# egui 原生 UI 设计：地基 + 定时发送面板

> 日期: 2026-09-28
> 状态: 设计已确认，待实施
> 关联代码: `egui-app/`（本地实验目录，**不在版本控制内**）

---

## 背景

OhMySerial 当前发布形态是 Tauri 2.x + React。仓库里另有一个纯 egui（eframe）实现 `egui-app/`，
用于 A/B 对比「无 WebView2 的原生渲染」是否可行。

`egui-app` 此前处于暂停状态。暂停原因已查明，**与代码缺陷无关**：

- 本机环境存在「任何可执行文件名以 `egui.exe` 结尾的进程会被杀」的规则（12/12 次观测吻合）
- 团队早期记录的「eframe 0.35 event loop bug」是误诊，已更正到 `.agents/memory.md`

解法是把 `[[bin]]` 名字从 `oh-my-serial-egui` 改成 **`oms-native`**，绕开命名规则。
另已补上 CJK 字体注册与终端虚拟化，`oms-native.exe` 实测存活 12s、705 帧（≈59fps）、无 panic。

本设计在此基础上继续推进，让 egui 版本达到「可当日常主力用」的状态。

---

## 目标与范围

### 本轮（垂直切片）

分两批交付。第一批是地基，第二批用「定时发送」面板验证多面板交互模式。

| 批次 | 内容 |
|---|---|
| **① 地基** | 清诊断残留、重绘节流、GBK 编码接线、重连状态展示、配置持久化 |
| **② 定时发送** | 右栏标签页骨架 + 统一 PanelAction + 队列轮询面板 + 周期发送面板 |

**为什么先做定时发送**：本项目最大的不确定性不是工作量，而是「egui 里怎么组织多面板交互」。
现有 1270 行只有单个右侧面板，标签页、面板间状态同步全是空白。
定时发送是第一个需要设计交互形态的功能，用它趟出模式，比三条线并行铺开再一起返工便宜。

### 下一轮

**录制面板**（开始/停止、路径、状态栏 REC 指示），照搬本轮趟出的面板模式。
有原生文件对话框依赖需要解决（core 无文件对话框能力，预计引入 `rfd`）。

### 本轮明确不做

| 项 | 原因 |
|---|---|
| 字体选择 UI | `font_size` / `font_family` 配置字段本轮只读不写，避免覆盖 Tauri 侧设置 |
| 预设命令面板 | Tauri 版是 localStorage，egui 侧需要另设计持久化方案 |
| 日志面板 | core 有 `log_init` 读日志行能力，UI 另排 |
| 自动打开上次端口 | 只预选中下拉框里的端口，仍需手动点连接 |

---

## 现状

`egui-app/` 共 1270 行 Rust，独立 cargo workspace（`egui-app/.cargo/config.toml` 设
`target-dir = "../target"` 与仓库根共用产物），**被 `.gitignore` 排除**。

### 已有

| 模块 | 行数 | 职责 |
|---|---|---|
| `main.rs` | 121 | 入口，runtime + bridge 启动 |
| `app.rs` | 204 | `eframe::App` 实现，布局与副作用分发 |
| `state.rs` | 133 | `SharedState` 跨线程状态 |
| `backend_bridge.rs` | 115 | Backend → SharedState 的两个 tokio 任务 |
| `terminal.rs` | 241 | 终端缓冲（含虚拟化行访问） |
| `ui/{terminal,toolbar,send_panel,status_bar,theme}.rs` | 449 | 各面板 |

功能：串口工具栏（端口/波特率/开关/刷新）、终端（文本+HEX 双视图、跟随尾部、已虚拟化）、
发送面板（文本+HEX）、状态栏（收发字节）、主题、错误弹窗、中文 fallback 字体。

### 依赖现状

`egui-app/Cargo.toml` 已含 `encoding_rs = "0.8"`（**未接线**）、`tokio`、`parking_lot`、`chrono`。
本轮不新增依赖。

### core 已提供但 UI 未用的 API

`queue_add` / `queue_remove` / `queue_clear` / `queue_status` / `queue_start_polling` /
`queue_stop_polling` / `start_periodic_send` / `stop_periodic_send` / `config::load` / `config::save`。
另有事件 `BackendEvent::SendPollerError` / `SendPreciseError`（bridge 已处理，但只打日志）。

---

## 关键约束（core 语义，实现时必须遵守）

以下四条是从 `core/src/backend.rs` 与 `core/src/sender/queue.rs` 读出的实际行为，
与直觉不符，容易写错：

1. **队列是非消费的，且轮询只反复发送队首那一条。**
   `next_command()` 返回 `commands.first()` 且**不移除**，
   而 `backend.rs` 的 poller 循环体内**没有任何 remove / pop / 游标推进**。
   所以轮询期间被反复发送的**只有 priority 最高的那一条**，
   **队列里其余命令永远不会被发出**。
   `interval_ms` 是该命令**发完后**的等待时长。

   > ⚠️ **实施时更正（2026-09-28）**：本文原先写的是「循环重发整个列表，不是消费到空」，
   > 那是错的推断——非消费 ≠ 轮流发整个列表。实施者读 core 源码时发现；
   > controller 复核 `core/src/sender/queue.rs:53` 与 `core/src/backend.rs:503-560`
   > （循环体内 remove/pop/游标推进 grep 零命中）后确认。
   > **这是 core 层行为，正式版 Tauri 同样如此**——「加 N 条命令 + 开始轮询」这种用法，
   > 在任何应用里都只有第 1 条会被发出。core 不在本轮修改范围内，但需上报。
2. **`queue_add` 会重排**。`sort_by_key(Reverse(priority))` —— 优先级降序，数值大的先发。
3. **周期发送会清空队列**。`start_periodic_send` 内部先 `queue.clear()`，
   与队列轮询**互斥**（共用 `port_handle`）。
4. **`queue_status()` 不返回命令内容**，只给 `{count, is_polling}`。
   UI 要显示/编辑队列，core 侧读不到明细。

### core 语义的两个不对称点

| 功能 | core 状态是否可信 | 原因 |
|---|---|---|
| 队列轮询 | ✅ 可信 | 线程退出时会 `q.stop_polling()` + 置 flag，`queue_status().is_polling` 准确 |
| 周期发送 | ❌ **不可信** | `send-precise` 线程写失败时直接 `break`，**没有把 stop flag 置回 true**；core 无 `is_periodic_running()` 查询接口 |

→ 周期发送的运行状态必须由 **UI 侧自记 + 事件纠正**，见 §3.4。

---

## 设计 · 1：右栏骨架与面板通信模式

这是要趟出来的核心，下一轮录制面板照抄。

### 1.1 结构改动

现状是终端与发送面板在 `CentralPanel` 内用 `ui.horizontal` + `allocate_ui` 手搓 70/30。改为：

```
CentralPanel        → 只放 terminal::show()
SidePanel::right    → 固定宽 400px
  ├─ 顶部标签条: [发送] [定时发送] [录制]
  └─ 当前面板内容（ScrollArea::vertical 可滚）
```

「录制」标签本轮**先占位**（显示「下一阶段」），使标签栏形态一次做对，下一轮直接填内容。

`UiState` 新增 `active_tab: SideTab { Send, Scheduled, Record }`。

### 1.2 面板通信模式（核心决策）

现状每面板自定义 Action 枚举（`UiAction`、`SendAction`），app.rs 分别 match，面板一多就散。
改为**统一 Action 枚举**，定义在 `ui/mod.rs`：

```rust
pub enum PanelAction {
    OpenPort,
    ClosePort,
    RefreshPorts,
    SendBytes { bytes: Vec<u8>, label: &'static str },
    QueueChanged,             // 队列内容变了 → 需同步到 core
    QueueTogglePolling(bool),
    PeriodicToggle(bool),
}
```

**不变式：面板是纯函数** —— 读 `SharedState`，返回 `Option<PanelAction>`，**绝不直接调 `backend`**。
所有副作用（写串口、起停线程）集中在 `app.rs` 的单个 `match` 中。

这与现有 `send_panel::show` 的写法一致，只是把 Action 提升为共享类型。
加面板不需要改 app.rs 的结构，只是 match 多一个分支。

### 1.3 状态归属

新面板状态（队列条目、周期参数、确认窗口）**继续放 `UiState`**（egui 主线程独占、task 不写）。
不引入新锁，不破坏 `state.rs` 已声明的不变量。

### 1.4 队列影子副本

`UiState.queue: Vec<QueueItem>` 是 UI 的可编辑真相：

```rust
pub struct QueueItem {
    pub id: String,       // 见下方 id 生成规则
    pub content: String,  // 文本/HEX 输入原文
    pub is_hex: bool,
    pub priority: u8,     // 0 / 128 / 255
    pub interval_ms: u64,
}
```

**id 生成规则**：`format!("q{}", n)`，`n` 取自 `UiState` 里的单调递增 `u64` 计数器。
**不引入 `uuid` 依赖** —— `core::SendCommand.id` 只是 `String`，egui 侧唯一要求是彼此不重复。

存 `String + is_hex` 而非 `Vec<u8>`：面板要显示原文，重新编码时按当前 `UiState.encoding` 转字节，
与 SendPanel 走同一条路径。

**排序一致性**：保存时按优先级降序推给 core，显示时也按降序。
两者顺序必须永远一致，否则用户看到的第 1 条不是实际先发的那条。

**同步**：`queue_changed()` 执行 `queue_clear()` + 逐条 `queue_add()`。
列表变更或开始轮询时调用。

> ⚠ **更正（2026-09-29 最终评审）**：本节原先写「轮询线程只读不删，故编辑期间同步是安全的」，
> **这是错的**。轮询线程确实只读不删，但 `queue_changed()` 的第一步 `queue_clear()`
> 会调 core 的 `SendQueue::clear()`，而它**顺带把 `is_polling` 一起置为 `false`**
> （`core/src/sender/queue.rs`）。poller 每轮开头就判 `if !q.is_polling() { break; }`
> （`core/src/backend.rs`），于是**任何一次编辑都会在下一轮把轮询静默停掉**——
> 用户只是把第 1 行的间隔从 2500 改成 5000，轮询就停了，而且没有任何解释。
>
> 因此实现改为：**轮询中把队列区块整体置灰**（`add_enabled_ui(false, …)`），
> 并显示「轮询中不可编辑，请先点「■ 停止轮询」再改」；「■ 停止轮询」保持可用。
> 不能用「先 stop 再 start 自动续跑」绕过——旧 poller 仍在 `sleep(cmd.interval_ms)`
> （默认 1000ms，上限 60000ms）里，`start` 会把 stop flag 复位成 false，旧线程醒来
> 看到 `flag = false` + `is_polling = true` 会**继续跑**，于是两个 poller 同时发队首
> = 双倍发送。core 提供 join / 代际号之前这条路不安全。

---

## 设计 · 2：地基

### 2.1 删除诊断残留

定位「egui 无痕退出」时添加的代码，结论已查明，使命完成：

| 删除对象 | 位置 |
|---|---|
| `diag_write` 闭包 + 7 处逐帧调用 | `app.rs` `ui()` —— 每帧 open/append/close 一个临时文件 |
| `tick_diag()` + `FIRST_TICK`/`TICK_COUNTER` static + 调用 | `app.rs` |
| `on_exit()` 中的诊断日志 | `app.rs` |
| `tests/repro/test_{a_minimal,b_apprefs,c_egui_minimal_ui}.rs` | 一次性排查产物 |
| `Cargo.toml` 中对应的 3 个 `[[bin]]` | 必须与文件同时删，否则 cargo 报路径缺失 |

保留：`backend_bridge.rs` 已用 `log::info!` 写正经日志，不动。

### 2.2 重绘策略

现状 `logic()` 每帧无条件 `ctx.request_repaint()` → 空闲时 60fps 空转。
但 `backend_bridge.rs:31,48` 已在数据到达与事件到达时调 `request_repaint()`，真正需要重绘的时机是覆盖到的。

改为：

- 删除 `logic()` 中无条件 `request_repaint()`
- `ui()` 末尾加 `ctx.request_repaint_after(Duration::from_millis(100))`

效果：空闲降到 10fps；有数据/事件时 bridge 立即唤醒，不受心跳限制。

**心跳是必需的**：重连倒计时「第 N 次尝试，X 秒后」与状态栏计时器需要持续走字，
纯事件驱动会让倒计时卡住。

### 2.3 GBK 编码接线

toolbar 的 UTF-8/GBK 选择器**已接好**（`ui/toolbar.rs:88-96`，写 `ui_state.encoding`），
只是 `terminal.rs:169` 的 `decode_text` 还在无脑 `String::from_utf8_lossy`。

单点改动：

```
当前:  String::from_utf8_lossy(bytes)
改为:  match encoding {
          Utf8 => String::from_utf8_lossy(bytes).into_owned(),
          Gbk  => encoding_rs::GBK.decode(bytes).0.into_owned(),   // 解码错误 → U+FFFD
       }
```

**涟漪**：`decode_text` 现在是自由函数，拿不到 `SharedState`。
需改为显式传入 `Encoding`，`push_rx` / `push_tx` 签名跟着带 `Encoding`。
这是本节唯一有涟漪的改动。

反向（发送）同理：`SendPanel` 的 `String::into_bytes()` 换成按当前编码 encode。
UI 无需改动（选择器已存在）。

### 2.4 重连状态展示

`shared.reconnect: Mutex<Option<ReconnectEvent>>` 当前**只写不读**。状态栏补上：

| 状态 | 展示 |
|---|---|
| 已断开，重连中 | 橙色 `● 断开 · 重连 2/5 · 5s 后重试` |
| 重连成功 | 绿色 `● 已重连`，3 秒后自动消失 |
| 重连放弃 | 红色 `● 断开 · 重连失败` |

倒计时数字靠 §2.2 的 10Hz 心跳刷新。

### 2.5 配置持久化

**关键约束：Tauri 与 egui 共用同一份 `config.json`**
（`%APPDATA%/com.ohmyserial.app/config.json`，由 `core::config::config_path()` 决定）。

> **egui 内存中持有完整的 `AppConfig`，只修改自己管的字段，其余原样写回。**

否则 egui 会把 Tauri 设的 `font_family` / `font_size` 冲掉 —— 共享配置最容易出的事故。

- **读**：`config::load()` → 灌进 `UiState`
- **egui 只拥有 3 个字段**：`last_port` / `baud_rate` / `encoding`（都是 toolbar 已经在编辑的）。
  其余字段（`theme` / `font_size` / `font_family` / `default_capture_path` /
  `auto_reconnect` / `reconnect_max_attempts` …）**不灌进 `UiState`，也不写回**，
  由 §2.5 的 `merge_owned(base, ui)` 天然原样保留。

  > 注：`auto_reconnect` / `reconnect_max_attempts` 由 **core 自己**在
  > `backend.rs:805` 读配置（`crate::config::load()`），不经过宿主。
  > 所以自动重连对 egui 是白拿的，egui 无需（也不应）持有这两个字段。
- **不做**：自动打开上次端口

`config::load()` / `save()` 本身已足够稳（文件缺失或损坏回落默认值 + `log::warn!`，
`save` 是 tmp+rename 原子写），**不改 core**。

---

## 设计 · 3：定时发送面板

标签页内部上下两块。

### 3.1 队列区块

```rust
pub struct QueueItem {
    pub id: String,
    pub content: String,
    pub is_hex: bool,
    pub priority: u8,        // 0 / 128 / 255
    pub interval_ms: u64,
}
```

每行：优先级 ComboBox / 内容（单行截断，等宽）/ 间隔 `DragValue` / HEX-TXT 切换 / 删除。

**优先级用三档 ComboBox（高=255 / 中=128 / 低=0）**，不用裸 0-255 数字 ——
内部仍存 `u8` 保持与 core 兼容，但用户看到的是能懂的东西。新条目默认「中」。

操作：新增（追加空行）、删除（UI 删 + `queue_remove`）、编辑（标记 dirty → `queue_changed()`）、
开始轮询（`queue_changed()` + `queue_start_polling()`）、停止（`queue_stop_polling()`）。

**轮询中整个区块置灰**（见 §1.4 的更正）：`add_enabled_ui(false, …)` 包住标题行（含「全部清空」）、
每一行的控件和「+ 添加一行」，并显示一行原因；「■ 停止轮询」在块外，必须保持可用。

### 3.2 周期发送区块

内容输入框 + HEX/TXT 切换 + 间隔 `DragValue`（10..=60000ms）+ 启停按钮。
编码同 SendPanel。间隔下限提示受波特率实际约束。

**启动前置校验**：内容为空、或 HEX 解析结果为空字节时，**「▶ 开始」按钮直接置灰**并给出原因
（文案：「周期发送内容为空或 HEX 非法：先填入有效内容，再启动」），间隔为 0 时把输入钳到 10ms 下限。

> ⚠ **更正（2026-09-29 最终评审）**：原先只写「置 `error_msg` 且不启动」是不够的——
> 校验发生在 `app.rs::set_periodic`，而**面板在二次确认通过时就已经 `clear_queue()`**，
> 于是「确认 → 队列被清 → 才发现内容非法 → 启动失败」是一条真实的数据丢失路径
> （实施结果的问题 2 就是在真机上走到的）。按钮置灰让这条路**不可能被走到**：
> 判据与 `app.rs` 的 `payload_bytes` 完全一致，面板放行 ⇒ app 那边必然校验通过。

### 3.3 互斥处理

core 层面两个功能互斥（§关键约束 3），UI 分两种情况处理：

**破坏性场景 —— 启动周期发送会清空用户队列（永久销毁编辑内容）**

「▶ 开始周期发送」做成**二次确认**：队列非空时，第一次点击把按钮变为
`⚠ 再点一次确认清空队列（N 条）`，5 秒内第二次点击才生效，超时自动还原。
状态存 `UiState.confirm_until: Option<Instant>`。轻量、无模态、不会误点丢数据。

**非破坏性场景 —— 启动轮询不破坏任何东西，只是 core 层面不能共存**

「▶ 开始轮询」在周期发送运行时**直接置灰** + tooltip「请先停止周期发送」。
不需要二次确认。

### 3.4 周期发送运行状态

因 core 侧状态不可信（§关键约束，不对称点）：

- UI 侧自记 `UiState.periodic_running: bool`
- **必须靠事件纠正**：`backend_bridge.rs` 处理 `BackendEvent::SendPreciseError` 时，
  除打日志外还要把该标志清为 `false`

否则一次串口写失败后，面板会一直显示「运行中」而实际线程早死了。

队列轮询无此问题，`queue_status().is_polling` 每帧读一次即可。

### 3.5 面板形态

```
┌ 定时发送 ─────────────────────────┐
│ 队列 (3 条)            [全部清空] │
│ [高▾] [TXT] AT+GMR   1000ms [×]  │
│ [中▾] [HEX] AA 01      500ms [×]  │
│ [低▾] [TXT] AT+RST     2000ms [×]  │
│ [+ 添加一行]                      │
│ [▶ 开始轮询]  轮询中… [■ 停止]     │
├───────────────────────────────────┤
│ 周期发送                          │
│ [AT+CPIN?              ] [HEX]   │
│ 间隔 [1000] ms                    │
│ [▶ 开始]  运行中… [■ 停止]        │
└───────────────────────────────────┘
```

全部收在 `ScrollArea::vertical()` 内，队列条目多了能滚。

---

## 数据流

```
core::Backend (tokio 后台线程)
   │  mpsc::Sender<Vec<u8>> 数据
   │  broadcast::Sender<BackendEvent> 事件
   ▼
backend_bridge.rs (2 个 tokio task)
   │  写 SharedState.{terminal,connection,reconnect,counters}
   │  ctx.request_repaint()          ← 事件驱动重绘（§2.2）
   ▼
SharedState (parking_lot::Mutex)
   │
   ▼
eframe::App::ui()  ──→  面板纯函数（读 SharedState）
   │  返回 Option<PanelAction>
   ▼
app.rs 单个 match ──→ 调 core::Backend 副作用
   │
   └──→ queue_changed() ──→ queue_clear() + queue_add() 逐条（§1.4）
```

配置流：`config::load()` → UiState →（每帧 diff，500ms debounce）→ `config::save()`（§2.5）

---

## 错误处理

沿用现有 `UiState.error_msg` + `egui::Window` 模态，不新增机制。

| 场景 | 处理 |
|---|---|
| 打开/关闭串口失败 | `error_msg`，已有 |
| HEX 解析失败 | `error_msg`，已有 |
| `queue_*` / `start_periodic_send` 返回 `Err` | 转 `error_msg` |
| 轮询写入失败 | `BackendEvent::SendPollerError` → bridge 打日志 + 终端系统行（已有），`is_polling` 由 core 自动转 false |
| 周期发送写入失败 | `BackendEvent::SendPreciseError` → bridge 打日志 + 终端系统行（已有）+ **清 `periodic_running`**（新增，§3.4） |
| 配置文件损坏 | `config::load()` 内部回落默认值 + `log::warn!`（core 已处理） |

---

## 测试策略

`egui-app/` 不在版本控制内，因此**无 CI**。测试靠本地 `cargo test` + 手工验证。

### 可自动化的纯函数

新增单测（放 egui-app 内，`cargo test` 手动跑）：

| 函数 | 断言点 |
|---|---|
| `decode_text(bytes, Encoding::Gbk)` | GBK 双字节序列正确解码；非法序列回落 U+FFFD 不 panic |
| 队列排序 | 降序推送后顺序与 UI 显示顺序一致 |
| `parse_hex_input` | 已有，保持 |
| 二次确认窗口 | 超时后按钮文案还原，不触发启动 |

### 手工验证清单（需真实串口，CH340 TX-RX 短接）

1. 空闲时 CPU 占用较改动前明显下降（§2.2）
2. 串口收发正常，终端不卡顿（验证 10fps 心跳不影响响应）
3. GBK 设备返回中文，终端正常显示；切回 UTF-8 行为与改动前一致
4. 拔线 → 状态栏出现橙色重连提示，倒计时逐秒递减
5. 插回 → 绿色「已重连」，3 秒后消失
6. 重启 app，baud / encoding / theme / 录制路径设置被正确恢复
7. 在 Tauri 版改字体设置 → 启动 egui → egui 写盘后 **Tauri 的字体设置未被冲掉**（§2.5 关键约束）
8. 队列添加 3 条 → 开始轮询 → 观察发送顺序符合优先级降序
9. 轮询中编辑队列 → 同步生效（**已作废**：轮询中禁止编辑，见问题清单第 8 条）
10. 队列非空时点周期发送 → 按钮变二次确认态
11. 周期发送运行中 → 「开始轮询」置灰
12. 拔线使周期发送写失败 → 面板「运行中」消失（§3.4）

---

## 实施结果

> 本节由 Task 9（端到端验证与收尾）写入，记录 2026-09-29 在**本机实机**跑出的结果。
> 验证手段：accesskit（eframe 已启用 `accesskit` feature）通过 UI Automation 读整棵
> 控件树 —— 按钮/标签/计数器/`enabled` 状态都是**读回来的**，不是推断的；
> 交互用 UIA 的 Invoke/Toggle/RangeValue 模式驱动真实运行中的进程；
> 文本输入用 `SendInput` 的 Unicode 路径；弹窗是否展开用窗口像素差分判定。

### A. 自动化验证

| 命令 | 结果 |
|---|---|
| `cargo test --bin oms-native` | **退出码 0**，`test result: ok. 67 passed; 0 failed; 0 ignored`（7.63s） |
| `cargo clippy --bin oms-native --tests -- -D warnings` | **退出码 0**，0 警告（touch 全部 `src/**/*.rs` 后强制重跑，非缓存结果） |
| `cargo build --release --bin oms-native` | **退出码 0**，`Finished release profile [optimized] in 1m 29s`，产物 `target/release/oms-native.exe` 7,305,216 字节 |

`--tests` 这一道门**实测不是假绿**（负向对照）：往 `src/state.rs` 的 `#[cfg(test)] mod tests`
里临时塞一个 `let mut x = 5;`（unused_mut）后——

- `cargo clippy --bin oms-native --tests -- -D warnings` → `error: variable does not need to be mutable`，**退出码 101**
- `cargo clippy --bin oms-native -- -D warnings` → **退出码 0**

带 `--tests` 能抓住测试代码里的 lint，不带则完全看不见 `#[cfg(test)]` 模块（Ruling 23 成立）。
临时改动已 `git checkout` 还原，工作区干净。

### B. 共享库未被动过

`git status --short core/ src-tauri/ src/` → **无输出**。本轮改动只落在 `egui-app/`（独立仓库，
最终提交仍为 `8fc2395`，`git status` 干净）与本文件的文档改动上。

### C. 实机验证清单（14 项）

环境前置事实（与 `AGENTS.md` 记载**不符**，实测为准）：

- **COM5 不是 CH340，是 FTDI FT232**（`FTDIBUS\VID_0403+PID_6001+A50285BIA`）。
  TX-RX **确实短接**：独立探针发 7 字节收回同样 7 字节。
  COM1 是主板口、未短接（发 7 收 0），故所有回环验证都在 COM5 上做。
- 本机桌面在 **RDP 层之后**（前台窗口是 `mstsc` 的 `TscShellContainerClass`）。
  后果：`keybd_event` / `mouse_event` / `SendKeys` 的 **VK 路径全部被吞**，
  `PostMessage` 的鼠标消息也不被 winit 采纳。唯一走得通的输入是
  **`SendInput` 的 `KEYEVENTF_UNICODE` 路径**（纯按键事件）与 **UIA 模式调用**。
  清单里凡依赖指针点击或 VK 键盘的项，都是先用这两条路替代后完成的。

| # | 操作 | 结果 | 证据 / 现象 |
|---|---|---|---|
| 1 | 空闲 30s 看 CPU | **通过** | 改造前基线（`74bcb66`，`logic()` 里无条件 `ctx.request_repaint()`）：30.156s CPU / 30s = **100.52% 单核**；当前（事件驱动 + 10Hz 心跳）：1.516s / 30s = **5.05% 单核**。**约 20 倍下降** |
| 2 | 从短接口发数据回来 | **通过** | 文本 `HELLO9` 发送后 `↓ TX 16→22 B`、`↑ RX 16→22 B`，终端渲染出该行（时间戳 + `→`/`←`） |
| 3 | 切 GBK 发中文，接收端回中文 | **通过**（离线推理无法证伪，故用线缆字节数证） | `中文` 在 `encoding=GBK` 下发出 **+4 字节**、收回 +4 字节，终端正确显示 `中文`（GBK 每字 2 字节；UTF-8 会是 6）。发送框旁的「N 字节」是 Rust `String::len()`（UTF-8 长度，恒为 6），不能用来判编码——真正判别的是 `↓TX`/`↑RX` 计数器，它统计的是**编码后的字节数** |
| 4 | 拔掉 TX/RX → 橙色重连徽章 | **无法验证** | 短接是 USB 模块上的物理跳线，无法自动断开；模拟设备移除需要管理员权限（`Disable-PnpDevice`），当前进程非管理员。逻辑本身有 7 个 `ui/reconnect.rs` 单测覆盖 |
| 5 | 插回 TX/RX → 绿色「已重连」 | **无法验证** | 同第 4 项 |
| 6 | 关掉 app 重开 → 恢复设置 | **通过** | 预置 `last_port=COM5 / baud_rate=57600 / encoding=GBK` 后重启，工具栏读回 `端口='COM5' 波特率='57600' 编码='GBK'`，状态栏 `● COM5 @ 57600` |
| 7 | Tauri 字段未被 egui 冲掉 | **通过** | 11 个非自有字段预置为特征值（`font_family=T9-Guard-Font`、`font_size=37`、`theme=dark`、`default_capture_path=D:\t9-guard`、`buffer_size=33333`、`auto_reconnect=false`、`reconnect_max_attempts=9`、`prompt_save_dialog=true` 等）。触发 egui 写盘（见下）后回读：**12 个非自有字段 0 处改动**，只有自有字段 `encoding` 由 `"GBK"` 规范化为 `"gbk"` |
| 8 | 队列 3 条 → 开始轮询 → 顺序 | **部分通过** | UI 侧全通（加行→`(3 条)`、内容为空时轮询门置灰并给出原因、填入内容后门放开、行 TXT/HEX 可切、间隔可改、`✖` 可删、`全部清空` 可用）。**但实际发出的只有队首那一条**：3 条内容分别为 `R1Z/R2Z/R3Z` 时，线缆上反复出现的只有 `R1Z`，`R2Z`/`R3Z` **一次都没出现**——**在真机上复现了「关键约束 1」的 core 缺陷**（详见下方问题 1） |
| 9 | 轮询中编辑 interval | **原判「通过」不成立，已改为「禁止编辑」** | 当时只读了「进程 `responding=True`、无 panic」，没有检查**轮询是否还在跑**——而真实行为是：编辑触发 `queue_changed()` → `queue_clear()` → core 的 `SendQueue::clear()` 顺带把 `is_polling` 置 false → poller 下一轮退出，**轮询被静默停掉**（见 §1.4 更正与问题清单第 8 条）。现在改为：轮询中队列区块整体置灰 + 「轮询中不可编辑，请先点「■ 停止轮询」再改」，「■ 停止轮询」保持可用；`ui::scheduled_panel::tests::queue_editing_is_disabled_while_polling` 用「轮询中整列点击不能改变队列长度、不能产出 `QueueChanged`」钉住这条（对照组证明扫法本身能点中「+ 添加一行」） |
| 10 | 队列 3 条 → 点「▶ 开始」 | **通过** | 按钮文案变为 `⚠ 再点一次确认清空队列（3 条，4s 内有效）`（橙色确认态），2.2s 后读数变 `2s`（倒计时在走），队列仍是 `(3 条)` 未被清空 |
| 11 | 5 秒后再点确认 | **通过** | 5.6s 后按钮**自动还原**为「▶ 开始」（无卡死态）；过期后再点只重新 arm（`4s 内有效`）；在窗口内再点一次 → 队列 `(3 条) → (0 条)` 且尝试启动周期发送 |
| 12 | 周期发送中「开始轮询」置灰 | **通过** | 周期发送运行中（`运行中…` + `■ 停止` 同时出现，线缆上 `PERIOD` 周期性回显），`▶ 开始轮询` 为 `enabled=False`，禁用原因正文显示「周期发送运行中，两者互斥：周期发送会清空队列」 |
| 13 | 周期发送中拔线 | **无法验证** | 同第 4 项（无物理断线条件）。`ui()` 的事件纠正路径未在真机触发 |
| 14 | 三个标签页来回切 | **通过** | 发送→定时发送→录制 各切 2 轮（共 6 次），每次面板标记各自正确（`0 字节` / `队列` / `录制功能将在下一阶段提供`），无 panic、无挂起 |

**用户报告的 Critical（工具栏三个下拉点击卡死）已复验：通过。**
对 端口 / 波特率 / 编码 三个 `ComboBox` 各做一次 UIA `Invoke`（等价于展开下拉）：

| 下拉 | Invoke 是否在 15s 内返回 | 弹窗区域像素变化 | 进程状态 |
|---|---|---|---|
| 端口 | 是 | 5852 | responding=True，线程数不变 |
| 波特率 | 是 | 6919 | responding=True，线程数不变 |
| 编码 | 是 | 4137 | responding=True，线程数不变 |

这个判据对本缺陷是**完备**的：死锁点就在弹窗内容闭包里的重入取锁，所以
「Invoke 及时返回（闭包没阻塞）」＋「弹窗区域确实画出了上千像素（闭包真的执行了）」
两条同时成立，就排除了该死锁。三个下拉全程 `GetFocus` 与 UIA 调用都正常返回，
无挂起。用户此前也已实机确认。

### 本轮发现、判定不在本轮修的问题

1. **core 的队列轮询只反复发送 priority 最高的那一条**（`core/src/sender/queue.rs:53`
   的 `first()` 配合 `core/src/backend.rs:503-560` 循环体内无 remove/pop/游标推进）。
   本轮**在真机上复现**：3 条不同内容的命令轮询时，只有队首那条上线缆，
   其余命令一次都没发出。**这是 core 层行为，正式版 Tauri 同样如此** ——
   「加 N 条命令 + 开始轮询」这种用法在两个应用里都只有第 1 条会发出去。
   egui 侧 UI 文案已如实写为「轮询中…（反复发送队首那条）」。
2. **确认门通过后若 `start_periodic_send` 失败（如内容非法/串口未打开），队列内容已丢失。**
   实测路径：队列 3 条 → 点两次确认 → 队列立刻变成 `(0 条)`，随后启动失败并弹出
   「周期发送内容为空或 HEX 非法」。确认门的语义是「用户明确同意销毁队列」，
   属知情同意的设计取舍，但触发条件不限于串口已打开，用户可能没有预期。
   **→ 已修（内容非法这条支路）**：内容为空 / HEX 非法时「▶ 开始」直接置灰并给出原因，
   这条路走不到确认门就不会清队列（见 §3.2 的更正）。串口未打开时仍会走到
   「队列已清 → 启动失败」，但那是用户在**内容合法**的情况下明确同意销毁队列后的失败。
3. **工具栏的开关按钮只在串口关闭时渲染**（`ui/toolbar.rs` 的
   `if !open_port { ... }`），所以界面上**没有关闭串口**的入口，
   只能靠拔线或退出程序。`PanelAction::ClosePort` 因此没有产出点。
   实测确认：串口打开后工具栏那一格显示的是 `已打开` 文本，没有按钮。
   **→ 已修**：按钮在两种状态下都渲染（关闭态「▶ 打开串口」/ 打开态「■ 关闭串口」，
   配色区分 accent / error），整行探针也扩成 `open_port = false` 与 `true` 各扫一遍；
   `ui::toolbar::tests::open_close_button_click_returns_matching_action` 钉住
   「关闭态扫得到 `OpenPort` 且扫不到 `ClosePort`，打开态反之」。
4. **`egui-app/` 无 CI 覆盖**：该目录被主仓库 `.gitignore:76` 排除、也不在根 workspace 的
   `members` 里，它的 85 个测试与 clippy 只能本地跑。
5. **（本轮新发现，core 层）polling / 周期发送的写入可能被读线程饿死。**
   `serial-reader` 线程**跨阻塞读持有 `port_handle`**（`core/src/backend.rs:703-708`），
   端口读超时是 **100ms**（`backend.rs:221`）；而 send-poller 与 send-precise 用的是
   `try_lock` 重试 **50 × 2ms ≈ 100ms** 的预算（`backend.rs:522-542`、`616-632`）。
   两个数字相等，于是读线程占满一个读超时时，发送线程刚好耗尽预算、
   报 `无法获取串口锁`，然后 `break` **退出线程**——轮询/周期发送会静默停掉。
   实测出现一次：轮询启动后终端打出 `[轮询写入失败] 无法获取串口锁`。
   测试路径能缓解（把队列间隔调大、避免连续回环流量），但不是修复。
   属 **core 层行为，正式版 Tauri 同样如此**。
6. **（本轮新发现，集成层）轮询与周期发送写出的字节不进 `↓ TX` 计数器、也不产生 TX 回显。**
   `app.rs::send_bytes` 才会计数并回显，而 poller / precise sender 直接在 core 里写口。
   于是这两条路径下 `↓ TX` 不动、终端只有 `←` 没有 `→`，
   用户会以为没发出去（本轮据此先误判过一次「周期发送没工作」，实际线缆上有周期性回显）。
   **→ 本轮只加缓解**：启动轮询 / 启动周期发送时往终端打**一行**系统提示
   （「[提示] …的字节由 core 直接写口：不计入 ↓TX、也没有 TX 回显（线缆上确实在发）」）。
   真正的修法要给 core 加字节计数 API（或让 core 把「已写出的字节」当事件推出来），
   不在本轮范围。
7. **（工具链观察）本机 `egui-app/Cargo.toml` 的 `Src` 之外还有两个残留 worktree**
   （`OhMySerialHelper/.t9-oldwt` 与 `%TEMP%/oms-t9/oldwt`，均 detached 在 `74bcb66`），
   前者会让主仓库 `git status` 多出一条未跟踪项。已删除并 prune，见收尾说明。
8. **（最终评审发现，core 层）`SendQueue::clear()` 会顺带把 `is_polling` 置为 `false`。**
   `core/src/sender/queue.rs` 的 `clear()` 里除了 `commands.clear()` 还有
   `is_polling = false`；而 poller 每轮开头判 `if !q.is_polling() { break; }`。
   于是任何「先 clear 再 add」的同步动作（egui 的 `queue_changed()` 就是全量
   `queue_clear()` + 逐条 `queue_add()`）都会**把正在跑的轮询停掉**。
   这条先后造成两个后果：① 设计与实现文档里「轮询中编辑是安全的」是错的（§1.4 已更正）；
   ② UI 侧必须禁止轮询中编辑（本轮的修法），因为 core 不提供 join / 代际号，
   「先 stop 再 start」会让旧线程和新线程同时发队首（双倍发送）。
   同一族的第二个坑：`queue_start_polling()` 的幂等守卫是
   `if !polling_stop_flag { return Ok(()) }`，而旧 poller 可能还在
   `sleep(cmd.interval_ms)`（默认 1000ms、可设到 60000ms）里，stop flag 仍是 false ——
   此时用户点「开始轮询」**什么都不会发生，连错误都不弹**。属 core 层行为，
   正式版 Tauri 同样如此。

---

## 风险与已知约束

| 风险 | 缓解 |
|---|---|
| **本机按映像名杀进程**：`[[bin]]` 名若以 `egui.exe` 结尾必被杀 | 已改名为 `oms-native`。后续新增 bin 不得使用含 `egui` 结尾的名字 |
| 共享 config 被两个 UI 互相覆盖 | **单进程场景**由 §2.5 的 `merge_owned(base, ui)` 保证不冲掉对方字段（egui 只盖 `last_port` / `baud_rate` / `encoding` 三个）。**并发打开（Tauri 与 egui 同时运行）或用户手改 config.json 时这条缓解不覆盖**：`base` 只在启动时装载一次，对方改了 egui 不拥有的字段之后，egui 的下一次写盘仍会拿**旧的 base** 把对方改动**静默覆盖**。⚠ 已知未修，见下方说明 |
| ↳ 为什么没加 mtime/size 守卫 | 看似低成本的「写盘前发现文件被改过就重新 `load()`」在这里**不安全**：core 的 `config::load()` 在文件损坏/读失败时回落 `AppConfig::default()` 且**无法从签名上区分成功与回落**（返回 `AppConfig` 而不是 `Result`），采纳一份「读到默认值」的 base 会把用户在 Tauri 侧的全部设置整体重置 —— 比「可能覆盖对方一处改动」更糟。要真修必须改 core（给 `load()` 一个能表达失败的入口）或让 egui 自己解析 JSON，两者都超出本轮范围（且 `oh-my-serial-core` 是共用库，动它要同步评估 Tauri 侧）。**本轮只改文档陈述，不假装已缓解** |
| 10fps 心跳导致倒计时观感偏慢 | 100ms 粒度对秒级倒计时足够；若觉得迟钝再降到 50ms |
| egui-app 无 CI，改坏不会在 PR 里暴露 | 核心逻辑（排序/解码/确认窗口）写单测兜底；依赖 CH340 手工验证 |
| 录制面板的原生文件对话框依赖未解决 | 本轮不涉及；下一轮需引入 `rfd`，届时再评估 |
