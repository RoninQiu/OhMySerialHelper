# core 层定时发送/队列轮询修复：设计

> 日期: 2026-09-29
> 对应 issue: [#1](https://github.com/RoninQiu/OhMySerialHelper/issues/1)
> 状态: 设计已逐节确认，待实施

---

## 背景与动机

实现 egui 原生实验版的「定时发送」面板时，读 core 源码 + CH340 真机实测定位到三个缺陷。
**三个缺陷对正式版 Tauri 同样存在**——egui 只是第一个把队列面板做出来、从而把它们暴露出来。

决定性证据：正式版的 `presetStore.startPolling()` 是
`cmd_queue_clear` → 逐条 `cmd_queue_add`（所有 enabled 预设）→ `cmd_queue_start_polling`。
**原作者的意图明明白白是「轮流发送每一条预设，每条带自己的 intervalMs」**。
所以缺陷 1 不是产品决策，是实现漏了轮转。

---

## 关键事实（实施时必须遵守）

| 事实 | 出处 | 含义 |
|---|---|---|
| `next_command()` 返回 `first()` 且不移除 | `core/src/sender/queue.rs:53` | 队列非消费，但 poller 无游标 |
| poller 循环体内无 remove/pop/游标推进 | `core/src/backend.rs:503-560` | 只有 priority 最高那条会被发 |
| **「队列非消费」≠「轮流发整个列表」** | — | 本项目最初的设计文档写反了，实测才发现 |
| `SendQueue::clear()` 顺带清 `is_polling` | `core/src/sender/queue.rs:63-66` | 宿主每次同步队列都会踢掉 poller |
| `queue_start_polling` 的幂等守卫 | `core/src/backend.rs:490-491` | 旧 poller 还在 sleep 时重新 start 会静默早退 |
| reader **跨阻塞读持锁**，读超时 100ms | `core/src/backend.rs:703-708`、`:221` | 空闲线路上锁可用率仅 ~2% |
| `ErrorKind::TimedOut` 被当作「无数据」 | `core/src/backend.rs:745` | **缩短读超时是安全的**，不会误触发断线 |
| `cmd_queue_clear` 只在 `presetStore.startPolling` 用一次 | `src/stores/presetStore.ts:104` | 语义拆分后**宿主一行都不用改** |
| `cmd_queue_stop_polling` 才是停止路径 | `src/stores/presetStore.ts:124` | 同上 |

### 空闲线路上锁争用的算术

reader 每约 102ms 只留 2ms 无锁窗口（`sleep(2ms)`）⇒ 可用率 ~2%。
poller 旧预算 = 50 次 × 2ms ≈ 100ms，命中率 `1 - 0.98^50 ≈ 64%`
⇒ **约 1/3 概率启动即死**，且失败路径是 `break`（线程永久退出）。

---

## 已确认的四个决策

1. **范围**：三个缺陷 + `timed_tx_bytes`/`TxEcho` + `load_checked`，一次做完
2. **priority 语义**：按优先级**降序轮转一圈**（priority = 循环顺序，不是频率权重）
3. **缺陷 3 修法**：缩短读超时 100→10ms + 放大锁预算 + 争用不终止线程（方案 A）
4. **TxEcho**：全量发，靠 broadcast 通道自带背压
5. **precise sender 一并修**：与 poller 共用 worker 抽象，不留对称的坑

---

## P1 · 队列语义与游标（`core/src/sender/queue.rs`）

### 游标

```rust
pub struct SendQueue {
    commands: Vec<SendCommand>,  // 始终保持 priority 降序
    is_polling: bool,
    cursor: usize,               // 下一个要发的下标，读时按 len 回绕
}
```

拆成 `peek_next(&self)` 与 `advance(&mut self)` 两步，**而不是 `next_command()` 取即推进**。

理由：若取即推进，则锁争用时会推进了游标却不发送，等于悄悄跳过用户配的命令。
调试工具里静默丢命令比阻塞更糟。peek/advance 让「命令要么发了才推进」成为结构性保证。

代价：`SendQueue` 的 API 变两步，第三方调用方可能误用。但 core 只被本仓库两个宿主使用，
不存在外部消费者。

### 游标归零规则

**任何改变队列内容的操作都让游标归零**（一轮从头开始）：`add` / `remove` / `clear`。

理由：`add` 会重排（priority 降序），旧下标指向的是另一条命令；
`retain` 之后旧下标越界或错位。归零让行为可预测——「改了队列 → 下一条从优先级最高的开始」，
而不是悄悄换成某条中间的命令。

### `clear()` 语义拆分

```rust
/// 清空所有命令。**不影响轮询状态** —— 要停止请用 `stop_polling()`。
pub fn clear(&mut self) {
    self.commands.clear();
    self.cursor = 0;
}
```

这一改**顺带修好了正式版**：`presetStore.startPolling` 每次都先 `cmd_queue_clear`，
旧行为等于每次点「开始轮询」都把上一轮 poller 踢掉、紧接着的 `cmd_queue_start_polling`
又被幂等守卫吞掉——这就是「轮询刚开始就自己停了」的根因。

### 测试

`cursor_advances_and_wraps` / `cursor_starts_at_zero_after_add` / `cursor_clamps_after_remove`
/ `clear_does_not_stop_polling` / `empty_queue_next_command_is_none`

---

## P2 · poller 循环重写（`core/src/backend.rs`）

### 锁预算与争用分离

```rust
const WRITE_LOCK_BUDGET: Duration = Duration::from_millis(500);
const WRITE_LOCK_RETRY: Duration = Duration::from_millis(2);
const CONTENTION_BACKOFF: Duration = Duration::from_millis(10);

enum WriteOutcome {
    Sent(usize),
    /// 抢不到锁：**暂时争用**，不终止线程
    Contended,
    /// 真正的写失败（串口未打开 / IO 错误）
    Failed(std::io::Error),
}

fn write_with_retry(port_handle, payload, budget) -> WriteOutcome
```

**这是缺陷 3「静默死掉」的直接解法**：旧代码 `attempts > 50` 时构造一个假 IO 错误 `break`
掉线程，等于把争用当成写失败。现在争用只跳过本 tick。

### 循环骨架

```rust
let cmd = { let q = …; if !q.is_polling() { break } q.peek_next().cloned() };

match write_with_retry(&port_handle, &cmd.content, WRITE_LOCK_BUDGET) {
    WriteOutcome::Sent(n) => {
        { send_queue.lock()?.advance(); }     // ← 发了才推进
        timed_tx_bytes.fetch_add(n as u64, Ordering::Relaxed);
        let _ = event_tx.send(BackendEvent::TxEcho(cmd.content.clone()));
        sleep_interruptible(&gen, my_gen, Duration::from_millis(cmd.interval_ms));
    }
    WriteOutcome::Contended => {
        // 游标不动 → 下一 tick 重试**同一条**命令
        thread::sleep(CONTENTION_BACKOFF);
    }
    WriteOutcome::Failed(e) => {
        let _ = event_tx.send(BackendEvent::SendPollerError(e.to_string()));
        break;
    }
}
```

### 缩短读超时

`core/src/backend.rs:221` 的 `.timeout(Duration::from_millis(100))` → **10ms**。一处常量。

副作用是好的：RX 延迟从最坏 ~100ms 降到 ~12ms，锁可用率从 ~2% 升到 ~17%。

### `TxEcho` 与 `timed_tx_bytes`

```rust
pub enum BackendEvent {
    …
    /// core 直接写口的那两条路径（队列轮询 / 周期发送）写入成功。
    /// 面板触发的普通发送**不走这里**。
    TxEcho(Vec<u8>),
}

/// 只统计 poller 与 precise 的写入。与宿主自己的计数器天然不相交。
pub fn timed_tx_bytes(&self) -> u64
```

**为什么叫 `timed_tx_bytes` 而不是 `tx_bytes`**：避免与宿主同名的「用户手动发送计数」混淆。
两个计数器不相交，所以事件接收方直接把字节数并进现有计数器即可，**不需要任何额外管线**。

---

## P3 · 生命周期：代际号 + 可中断等待（`core/src/backend.rs`）

### 先说一个 issue #1 里没写的问题

`stop_polling()` 只是置位，**线程此刻躺在不可中断的 `thread::sleep(interval_ms)` 里**，
而 `interval_ms` 可配到 60000。

> **点了「停止轮询」之后，poller 线程最长 60 秒才真正退出。**

这不是竞态，是睡眠不可中断——它正是「立刻停止再开始 → 双 poller」窗口的来源。

### 可中断的间隔等待

```rust
fn sleep_interruptible(gen: &AtomicU64, my_gen: u64, total: Duration) {
    let slice = (total / 20).clamp(Duration::from_millis(20), Duration::from_millis(200));
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if gen.load(Ordering::SeqCst) != my_gen { return; }
        thread::sleep(slice.min(deadline.saturating_duration_since(Instant::now())));
    }
}
```

片长按 interval 的 1/20 算，钳在 20–200ms。**stop 的生效延迟从「最长 60 秒」降到「最长 200ms」。**

### 代际号取代 stop_flag

`polling_stop_flag: AtomicBool` 同时承担「该不该跑」和「有没有线程在跑」两个职责，是竞态来源。
拆成：

```rust
polling_generation: Arc<AtomicU64>,   // 每次 start/stop 自增
polling_running: Arc<AtomicBool>,     // 仅用于避免重复 spawn
```

### start / stop 新语义

```rust
pub fn queue_start_polling(&self) -> Result<(), SerialError> {
    let already = self.send_queue.lock()?.is_polling()
        && self.polling_running.load(Ordering::SeqCst);
    if already { return Ok(()); }

    let gen = self.polling_generation.fetch_add(1, Ordering::SeqCst) + 1;
    self.polling_running.store(true, Ordering::SeqCst);
    self.send_queue.lock()?.start_polling();
    spawn_poller(gen);
    Ok(())
}

pub fn queue_stop_polling(&self) -> Result<(), SerialError> {
    if let Ok(mut q) = self.send_queue.lock() { q.stop_polling(); }
    self.polling_generation.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
```

| 场景 | 行为 | 正确？ |
|---|---|---|
| 连按两次「开始」 | 第二次 `already` 为 true → 幂等返回 | ✓ 单线程 |
| 停止 → 立刻开始 | stop 使 gen+1、队列 `is_polling=false` → start 时 `already` 为 false，gen 再 +1 并 spawn；旧线程代号不符 → 退出 | ✓ **无双 poller** |
| 快速 stop/start 连打 | 每次 start 都 spawn、代号单调递增，只有最后一个能活 | ✓ 短暂多线程，各自最多活一片时长 |

### precise 一并修

`start_periodic_send` / `stop_periodic_send` 是同一套模式、同样的不可中断 sleep，
有一模一样的两个问题。egui 侧「周期发送启动前先 `queue_stop_polling()`」
正是会触发这个窗口的操作。

把 `sleep_interruptible` + 代际号抽成一个共用的 `InterruptibleWorker`，两者都用它。

---

## P4 · API 与宿主接线

### Tauri 侧（三处，都很浅）

**① `src-tauri/src/lib.rs`**
```rust
BackendEvent::TxEcho(bytes) => {
    let _ = app_handle.emit("tx-echo", serde_json::json!({ "bytes": bytes }));
}
```

**② `src/App.tsx`** —— `terminalRef` 与 `TerminalHandle.writeData(data, direction?)` 都已存在，
**不需要新组件 API**：
```ts
const unTxEcho = await listen<{ bytes: number[] }>("tx-echo", (event) => {
    const bytes = new Uint8Array(event.payload.bytes);
    useSerialStore.getState().noteTimedSend(bytes.length);
    terminalRef.current?.writeData(bytes, "tx");
});
unlistens.push(unTxEcho);
```

**③ `src/stores/serialStore.ts`** 加 `noteTimedSend(n: number)`，只做 `txBytes += n`。

**不动**既有的「乐观更新 + 失败回滚」——那条路径处理的是用户手动发送的失败，
与定时发送无关，混在一起会让两条路径的语义变浑。

### egui 侧（一处）

`egui-app/src/backend_bridge.rs` 的 `apply_event`：
```rust
BackendEvent::TxEcho(bytes) => {
    shared.counters.add_tx(bytes.len());
    let enc = shared.ui.lock().encoding;
    shared.terminal.lock().push_tx(&bytes, enc);
}
```
状态栏与终端因此自动正确，不需要把 backend 引用传进 UI。

### `config::load_checked()`

```rust
/// 与 `load()` 相同，但读取或解析失败时返回 `None`。
///
/// `load()` 分不清「用户没有配置」与「配置坏了」，
/// 贸然采用它的默认值会把用户设置整体重置。
pub fn load_checked() -> Option<AppConfig>
```

egui 侧接线：
- `ConfigSync` 记住启动装载时 `config.json` 的 mtime 与长度
- 写盘前比对，**变了**才 `load_checked()`：
  - `Some(cfg)` → 以它为新 base 再 merge（保住对方改动）
  - `None` → **跳过本次写盘**，往终端写一行说明

这把「并发覆盖」那条从文档免责声明变成真正修掉。

---

## 测试预期

| 仓库 | 现值 | 预期 |
|---|---|---|
| core | 58 | ~70（队列游标、poller 循环、`write_with_retry`、`load_checked`） |
| 前端 | 190 | ~192（`noteTimedSend` + `tx-echo` 监听） |
| egui-app | 85 | ~87（bridge TxEcho、config_sync mtime 守卫） |

29 个需要 CH340 的集成测试里，涉及预设轮询的会**从「恒过」变成「真的有意义」**。

---

## 兼容性

`clear()` 语义变更后 **Tauri 前端一行都不用改**——`presetStore.startPolling` 的
`clear → add × N → start` 序列在新语义下反而是正确用法。

**但要注意**：poller 正在跑时，「开始轮询」变成幂等空操作。
用户改完预设再点一次「开始」，core 认为已在跑就返回，不会重启。
这是合理的（队列已实时更新），但 UI 上要让用户知道「已经在轮询，改动即时生效」，
否则用户会以为「点了没反应」。实施时一并处理这条文案。
