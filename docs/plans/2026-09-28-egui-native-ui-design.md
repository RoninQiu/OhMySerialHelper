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

1. **队列是非消费的**。`next_command()` 返回 `commands.first()` 且**不移除**，
   所以轮询是**循环重发整个列表**，不是消费到空。`interval_ms` 是该命令**发完后**的等待时长。
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
轮询线程只读不删，故编辑期间同步是安全的。列表变更或开始轮询时调用。

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

### 3.2 周期发送区块

内容输入框 + HEX/TXT 切换 + 间隔 `DragValue`（10..=60000ms）+ 启停按钮。
编码同 SendPanel。间隔下限提示受波特率实际约束。

**启动前置校验**：内容为空、或 HEX 解析结果为空字节时，置 `error_msg` 且**不启动**。
间隔为 0 时把输入钳到 10ms 下限。

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
9. 轮询中编辑队列 → 同步生效
10. 队列非空时点周期发送 → 按钮变二次确认态
11. 周期发送运行中 → 「开始轮询」置灰
12. 拔线使周期发送写失败 → 面板「运行中」消失（§3.4）

---

## 风险与已知约束

| 风险 | 缓解 |
|---|---|
| **本机按映像名杀进程**：`[[bin]]` 名若以 `egui.exe` 结尾必被杀 | 已改名为 `oms-native`。后续新增 bin 不得使用含 `egui` 结尾的名字 |
| 共享 config 被两个 UI 互相覆盖 | §2.5：内存持完整 `AppConfig`，只改自己管的字段 |
| 10fps 心跳导致倒计时观感偏慢 | 100ms 粒度对秒级倒计时足够；若觉得迟钝再降到 50ms |
| egui-app 无 CI，改坏不会在 PR 里暴露 | 核心逻辑（排序/解码/确认窗口）写单测兜底；依赖 CH340 手工验证 |
| 录制面板的原生文件对话框依赖未解决 | 本轮不涉及；下一轮需引入 `rfd`，届时再评估 |
