# egui 原生 UI（地基 + 定时发送面板）实施计划

> **给 AI 执行者：** 按任务逐个执行，每步用复选框（`- [ ]`）追踪进度。
> 两种执行模式二选一：subagent-driven-development（每个任务派一个全新 subagent，
> 任务间做两段式评审，推荐）或 executing-plans（本会话内批量执行 + 检查点）。
> 无论哪种模式，都不要跳过「验证」步骤直接进行下一步。

**Goal:** 把 `egui-app/` 从能跑的 MVP 推进到「可当日常主力用」——先补地基（性能、GBK、重连显示、配置持久化），再用「定时发送」面板验证多面板交互模式。

**Architecture:** 三层不变式保持不变——`core::Backend`（tokio 后台）→ `backend_bridge.rs`（两个 tokio task 写 `SharedState`）→ `eframe::App::ui()`（egui 主线程读）。本轮新增的核心约定是**面板是纯函数**：面板读 `SharedState`、返回 `Option<PanelAction>`，绝不直接调 `backend`；所有副作用集中在 `app.rs` 的单个 `match` 里。配置走「内存持有完整 `AppConfig`，只覆盖 egui 拥有的 3 个字段，其余原样写回」，避免与 Tauri 版互相覆盖。

**Tech Stack:** Rust 2021 / egui + eframe 0.35 / `encoding_rs` 0.8 / `tokio` / `parking_lot` / `oh-my-serial-core`

**Spec:** [`docs/plans/2026-09-28-egui-native-ui-design.md`](2026-09-28-egui-native-ui-design.md) —— 执行者须同时阅读本计划与该 spec

---

## 全局约束

以下约束对本计划**每一个任务**都成立：

1. **`egui-app/` 不在版本控制内**（被 `.gitignore` 排除）。因此任务**不以 git commit 收尾**，而以「验证命令通过」收尾。不要试图 `git add egui-app/`，那会失败；也不要为了提交而改 `.gitignore`。
2. **本机环境：任何可执行文件名以 `egui.exe` 结尾的进程会被杀**（12/12 次观测吻合，结论已定案）。`Cargo.toml` 里主 bin 的名字是 `oms-native`，**任何时候不得改名或新增以 `egui` 结尾的 bin 名**。
3. **`egui-app/` 是独立 cargo workspace**（其 `Cargo.toml` 有 `[workspace]` 段），构建产物经 `egui-app/.cargo/config.toml` 落到仓库根 `target/`。所有 cargo 命令都在 `egui-app/` 目录下执行。
4. **`oh-my-serial-core` 是共享库**，被 Tauri 版共用。本计划**不改 core**。若发现必须改 core 才能完成，立即停下报告，不要自行修改。
5. **验证 GUI 程序存活用这套命令**（MSYS bash 直接 exec 会 `Permission denied`）：
   ```bash
   cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
   sleep 8
   tasklist | grep -ci oms-native    # 期望 ≥ 1
   taskkill //F //IM oms-native.exe
   ```
   构建与运行必须**分成两条命令**，不要用 `&&` 串联。
6. **`SharedState.ui` 的既有不变量**：`UiState` 由 egui 主线程独占读写，tokio task 不得写。新增面板状态一律放这里，不引入新锁。
7. **`SharedState` 中 task 可写的字段**仅限 `terminal` / `connection` / `reconnect` / `counters` / `ports`。Task 3 会给 `connection` 之外新增字段时，必须在字段注释里写明「谁写」。

---

## 文件结构

本计划完成后，`egui-app/src/` 的形态：

```
egui-app/src/
├── main.rs                  (改) 启动时装载配置，灌进 UiState
├── app.rs                   (改) 布局重构 + 副作用集中分发 + 10Hz 心跳
├── codec.rs                 (新) 编解码：decode/encode，UTF-8 与 GBK
├── state.rs                 (改) UiState 扩展：SideTab / QueueItem / ConfigSync 等
├── backend_bridge.rs        (改) push_rx 传编码；SendPreciseError 纠正 periodic_running
├── terminal.rs              (改) push_rx/push_tx 接 Encoding；移除一次性 repro 诊断依赖
├── config_sync.rs           (新) AppConfig 与 UiState 的双向桥 + 500ms debounce
└── ui/
    ├── mod.rs               (改) 声明新模块 + 统一 PanelAction
    ├── theme.rs             (改) 新增 success()
    ├── toolbar.rs           (改) 迁到统一 PanelAction
    ├── terminal.rs          (不改)
    ├── status_bar.rs        (改) 新增重连徽章
    ├── send_panel.rs        (改) 迁到统一 PanelAction + 按编码发送
    ├── reconnect.rs         (新) 重连徽章的状态机（脱离 egui 可单测）
    └── scheduled_panel.rs   (新) 队列轮询 + 周期发送
```

> **为什么 `config_sync.rs` 只放一处、且不在 `ui/` 下**：
> 它的全部内容（`merge_owned` / `apply_to_ui` / `ConfigSync::tick`）都是纯逻辑，
> 不碰任何 egui 类型，可以独立单测。调用点直接写在 `app.rs` 的 `ui()` 里
> （约 6 行），不值得为它单开一个 `ui/config_sync.rs`。
> `reconnect.rs` 同理——徽章的状态机可测，渲染在 `status_bar.rs` 里。

**删除：** `egui-app/tests/repro/`（3 个文件）+ `egui-app/Cargo.toml` 里对应的 3 个 `[[bin]]`。

---

## 批次 ① 地基

### Task 1：删除一次性诊断残留

`egui-app/` 此前暂停期间，为定位「egui 无痕退出」加了一批诊断代码。结论已查明是本机按映像名杀进程（见全局约束 2），诊断使命完成。这些代码现在有两个害处：`diag_write` **每帧** open/append/close 一个临时文件。

**Files:**
- Modify: `egui-app/src/app.rs`
- Modify: `egui-app/Cargo.toml`
- Delete: `egui-app/tests/repro/test_a_minimal.rs`、`test_b_apprefs.rs`、`test_c_egui_minimal_ui.rs`

**Interfaces:**
- Consumes: 无（首个任务）
- Produces: 干净的 `app.rs`，`eframe::App` 实现体只剩 `logic` / `ui` / `on_exit` 三个方法

- [ ] **Step 1：删除 `tick_diag` 方法与两个 static**

在 `egui-app/src/app.rs` 中删除整个第二个 `impl EguiApp` 块（`tick_diag` 所在的 `impl`）：

```rust
impl EguiApp {
    fn tick_diag(&self, frame: &eframe::Frame) {
        static FIRST_TICK: AtomicBool = AtomicBool::new(false);
        static TICK_COUNTER: AtomicU64 = AtomicU64::new(0);

        if !FIRST_TICK.swap(true, Ordering::Relaxed) {
            let info = frame.info();
            log::info!(
                "[egui] update() first tick — {:?} (cpu_usage={:?})",
                info.cpu_usage,
                info.cpu_usage
            );
        }

        let n = TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
        if n > 0 && n.is_power_of_two() {
            log::info!("[egui] still alive @ frame {} (power of 2)", n);
        }
    }
}
```

- [ ] **Step 2：删除随之失效的 import**

`egui-app/src/app.rs` 顶部的这一行不再被使用（`tick_diag` 是唯一使用者），删除它：

```rust
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
```

保留 `use crate::state::AppRefs;`、`use crate::ui::{send_panel, status_bar, terminal, theme, toolbar};`、`use egui::Context;` 这三行。

- [ ] **Step 3：删除 `logic()` 里的 `tick_diag` 调用**

把：

```rust
fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
    self.tick_diag(frame);
```

改为（Task 2 还会继续改这个方法，这里先只去掉诊断调用）：

```rust
fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
```

- [ ] **Step 4：删除 `ui()` 里的 `diag_write` 闭包与全部 7 处调用**

在 `ui()` 方法开头，删除这段（`let ctx = ui.ctx();` 保留）：

```rust
        // 诊断：直接写文件（Windows GUI 子系统无法重定向 stderr）
        let diag_path = std::env::temp_dir().join("oh-my-serial-egui").join("ui-diagnose.log");
        let diag_write = |msg: &str| {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&diag_path)
            {
                let _ = writeln!(f, "{} [ui] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg);
            }
        };

        diag_write("start");
```

然后删除 `ui()` 内全部 7 处 `diag_write(...)` 裸调用。它们都是独立成行的语句，删干净即可：

```rust
        diag_write("start");
        diag_write("after error_toast");
        diag_write("before toolbar");
        diag_write("after toolbar");
        diag_write("before status");
        diag_write("after status");
        diag_write("before central");
        diag_write("end");
```

- [ ] **Step 5：删除 repro 测试文件与对应 bin 声明**

删除整个目录：

```bash
cd eg-ui 2>/dev/null; rm -rf egui-app/tests/repro
```

（若上一行因目录名不匹配失败，直接 `rm -rf egui-app/tests/repro` 再删空的 `egui-app/tests`。）

在 `egui-app/Cargo.toml` 中删除这 3 段（保留主 bin `oms-native`）：

```toml
[[bin]]
name = "repro_test_a"
path = "tests/repro/test_a_minimal.rs"

[[bin]]
name = "repro_test_b"
path = "tests/repro/test_b_apprefs.rs"

[[bin]]
name = "repro_test_c"
path = "tests/repro/test_c_egui_minimal_ui.rs"
```

- [ ] **Step 6：验证编译与既有测试**

```bash
cd egui-app && cargo build --bin oms-native
```

Expected: 编译成功。**特别注意**：不得出现 `error[E0425]: cannot find function diag_write` 或 `cannot find function tick_diag`——若出现说明 Step 1/4 删漏了。

```bash
grep -rn "diag_write\|tick_diag" egui-app/src/
```

Expected: 无输出（grep 退出码 1）。

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 7 passed; 0 failed`（Task 1 不增删任何测试，数量应仍是 7）。

---

### Task 2：重绘策略改为事件驱动 + 10Hz 心跳

现状 `logic()` 每帧无条件 `ctx.request_repaint()` → 空闲时 60fps 空转。但 `backend_bridge.rs` 已在数据到达（`push_rx` 后）和事件到达（`apply_event` 后）各调了一次 `request_repaint()`，**真正需要重绘的时机已被覆盖**。

心跳（10Hz）不是可有可无的：重连倒计时「第 N 次尝试，X 秒后」和状态栏计时器需要持续走字，纯事件驱动会让倒计时卡住不动。

**Files:**
- Modify: `egui-app/src/app.rs`

**Interfaces:**
- Consumes: Task 1 清理后的 `app.rs`
- Produces: `ui()` 末尾有 `ctx.request_repaint_after(Duration::from_millis(100))`；`logic()` 不再无条件 repaint

- [ ] **Step 1：删除 `logic()` 里无条件 `request_repaint`**

把 `logic()` 整体替换为：

```rust
    /// eframe 0.35: `logic` 在每次 `ui` 之前调用
    ///
    /// 这里**不**调 `request_repaint()`——真正的重绘时机由 backend_bridge 在
    /// 数据/事件到达时触发，另有 `ui()` 末尾的 10Hz 心跳兜底。
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 启动期 一次性：刷新端口列表
        if self.refs.shared.ports.lock().is_empty() {
            self.refresh_ports();
        }

        let _ = ctx;
    }
```

- [ ] **Step 2：在 `ui()` 末尾加 10Hz 心跳**

在 `ui()` 的最后一行（`CentralPanel` 块之后、闭合花括号之前）加上：

```rust
        // 10Hz 心跳：让重连倒计时、状态栏计时器持续走字。
        // 有数据/事件时 backend_bridge 会立即 request_repaint()，不受此节流影响。
        ctx.request_repaint_after(Duration::from_millis(100));
```

（`ui()` 开头已有 `let ctx = ui.ctx();`，直接用 `ctx` 即可。）

- [ ] **Step 3：加 `Duration` import**

在 `egui-app/src/app.rs` 顶部加上：

```rust
use std::time::Duration;
```

- [ ] **Step 4：验证编译**

```bash
cd egui-app && cargo build --bin oms-native
```

Expected: 编译成功，无 unused variable / unused import 警告。

- [ ] **Step 5：实跑验证存活与响应**

```bash
cd egui-app && cargo build --bin oms-native && cd .. && cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
```

```bash
sleep 10 && tasklist | grep -ci oms-native
```

Expected: 输出 ≥ 1（程序存活 10 秒以上）。

```bash
taskkill //F //IM oms-native.exe
```

> 本任务的性能收益需要人工判断：Task 9 的手工验证清单第 1 项会复核。此处只需确认程序没崩。

---

### Task 3：GBK 编解码接线

toolbar 的 UTF-8/GBK 选择器**已经接好**（`ui/toolbar.rs` 里的 `ComboBox::from_label("编码")`，会写 `ui_state.encoding`），但 `terminal.rs` 的 `decode_text` 还在无脑 `String::from_utf8_lossy`——选了 GBK 也没用。

**涟漪**：`decode_text` 现在是自由函数，拿不到编码。新建 `codec.rs` 承担编解码，`push_rx`/`push_tx` 显式收 `Encoding` 参数。

**Files:**
- Create: `egui-app/src/codec.rs`
- Modify: `egui-app/src/state.rs`（`Encoding` 加 `as_str` / `from_label`）
- Modify: `egui-app/src/terminal.rs`（`push_rx`/`push_tx` 加 `Encoding` 参数，删除本地 `decode_text`）
- Modify: `egui-app/src/main.rs`（加 `mod codec;`）
- Modify: `egui-app/src/backend_bridge.rs`（`push_rx` 调用点）
- Modify: `egui-app/src/app.rs`（`push_tx` 调用点）
- Modify: `egui-app/src/ui/send_panel.rs`（`push_tx` 调用点 + 按编码发送）

**Interfaces:**
- Consumes: Task 1–2 的 `app.rs`
- Produces:
  - `codec::decode(bytes: &[u8], enc: Encoding) -> String`
  - `codec::encode(s: &str, enc: Encoding) -> Vec<u8>`
  - `Encoding::as_str(&self) -> &'static str`（`"utf-8"` / `"gbk"`）
  - `Encoding::from_label(s: &str) -> Encoding`
  - `TerminalBuffer::push_rx(&mut self, bytes: &[u8], enc: Encoding)`
  - `TerminalBuffer::push_tx(&mut self, bytes: &[u8], enc: Encoding)`
  - `send_panel::SendAction::SendText(String)` / `SendHex(String)` **签名不变**

- [ ] **Step 1：先写失败测试**

创建 `egui-app/src/codec.rs`，先只写测试（实现留空以确保编译失败）：

```rust
//! 编解码：UTF-8 与 GBK
//!
//! React 版用 npm 的 `iconv-lite` 做这件事；egui 版用 Rust 的 `encoding_rs`。
//! 同一份字节，按当前编码解释成显示文本；同一份文本，按当前编码变成发送字节。

use crate::state::Encoding;

/// 按编码把字节解码成显示文本。解码失败用 U+FFFD 替换（不 panic）。
pub fn decode(bytes: &[u8], enc: Encoding) -> String {
    match enc {
        Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::Gbk => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

/// 按编码把文本编码成待发送字节。无法编码的字符用 `?` 替换（不 panic）。
pub fn encode(s: &str, enc: Encoding) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => s.as_bytes().to_vec(),
        Encoding::Gbk => encoding_rs::GBK.encode(s).0.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "中文" 的 GBK 编码：D6D0 CEC4
    const CN_GBK: [u8; 4] = [0xD6, 0xD0, 0xCE, 0xC4];

    #[test]
    fn gbk_decode_chinese() {
        assert_eq!(decode(&CN_GBK, Encoding::Gbk), "中文");
    }

    #[test]
    fn utf8_decode_chinese() {
        assert_eq!(decode("中文".as_bytes(), Encoding::Utf8), "中文");
    }

    #[test]
    fn gbk_encode_chinese() {
        assert_eq!(encode("中文", Encoding::Gbk), CN_GBK.to_vec());
    }

    #[test]
    fn utf8_encode_chinese() {
        assert_eq!(encode("中文", Encoding::Utf8), "中文".as_bytes().to_vec());
    }

    #[test]
    fn wrong_encoding_does_not_panic() {
        // 用 UTF-8 解码器读 GBK 字节：出错也要给出可见文本，不能 panic
        let s = decode(&CN_GBK, Encoding::Utf8);
        assert!(!s.is_empty());
    }

    #[test]
    fn gbk_decode_invalid_bytes_falls_back() {
        // 孤立续字节，GBK 解码器会替换为 U+FFFD
        let s = decode(&[0x80, 0x80], Encoding::Gbk);
        assert!(!s.is_empty());
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(decode(&[], Encoding::Utf8), "");
        assert_eq!(decode(&[], Encoding::Gbk), "");
        assert!(encode("", Encoding::Gbk).is_empty());
    }

    #[test]
    fn roundtrip_gbk_ascii() {
        let s = "AT+GMR";
        assert_eq!(decode(&encode(s, Encoding::Gbk), Encoding::Gbk), s);
    }
}
```

- [ ] **Step 2：运行测试确认通过（此处实现已一并给出）**

```bash
cd egui-app && cargo test --bin oms-native codec::tests
```

Expected: `8 passed; 0 failed`。

> 如果 Step 1 你是「先只写测试再补实现」分两步做的，Step 1 的第一次运行应当 **编译失败**
> （`error[E0425]: cannot find function codec::decode`），确认后再补上 `decode`/`encode` 实现。

- [ ] **Step 3：`Encoding` 加字符串互转**

在 `egui-app/src/state.rs` 的 `impl Encoding` 块里追加两个方法：

```rust
    /// 与 `AppConfig.encoding` 的字符串形式（"utf-8" / "gbk"）互转
    pub fn as_str(&self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Gbk => "gbk",
        }
    }

    pub fn from_label(s: &str) -> Self {
        if s.eq_ignore_ascii_case("gbk") {
            Encoding::Gbk
        } else {
            Encoding::Utf8
        }
    }
```

- [ ] **Step 4：`terminal.rs` 接上编码**

在 `egui-app/src/terminal.rs` 顶部加 import：

```rust
use crate::codec;
use crate::state::Encoding;
```

把 `push_rx` 和 `push_tx` 两个方法的签名与函数体改为：

```rust
    pub fn push_rx(&mut self, bytes: &[u8], enc: Encoding) {
        if bytes.is_empty() {
            return;
        }
        self.push_line(TerminalLine {
            timestamp: now_hms_milli(),
            direction: Direction::Rx,
            bytes_len: bytes.len(),
            text: codec::decode(bytes, enc),
            hex: encode_hex(bytes),
        });
    }

    pub fn push_tx(&mut self, bytes: &[u8], enc: Encoding) {
        if bytes.is_empty() {
            return;
        }
        self.push_line(TerminalLine {
            timestamp: now_hms_milli(),
            direction: Direction::Tx,
            bytes_len: bytes.len(),
            text: codec::decode(bytes, enc),
            hex: encode_hex(bytes),
        });
    }
```

删除文件末尾的本地 `decode_text` 及其文档注释（已迁到 `codec.rs`）：

```rust
/// 解码为字符串：UTF-8 优先，失败逐字节替换为 U+FFFD 或 Latin-1 转义
///
/// MVP 范围：UTF-8 only。GBK 解码在 React 版有部分映射表实现，
/// egui 版先用 UTF-8 跑通 A/B 对比，GBK 是 v0.2 任务。
fn decode_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
```

- [ ] **Step 5：修 `terminal.rs` 里受影响的既有测试**

`push_rx` / `push_tx` 签名变了，该文件里 5 处测试的调用要加参数。把它们改为：

```rust
    #[test]
    fn push_rx_and_iter() {
        let mut buf = TerminalBuffer::new();
        buf.push_rx(b"hello", Encoding::Utf8);
        buf.push_rx(b"world", Encoding::Utf8);
        assert_eq!(buf.len(), 2);
        let v: Vec<&TerminalLine> = buf.iter().collect();
        assert_eq!(v[0].text, "hello");
        assert_eq!(v[1].text, "world");
    }
```

```rust
    #[test]
    fn drops_oldest_when_over_capacity() {
        let mut buf = TerminalBuffer::with_capacity(3);
        buf.push_rx(b"1", Encoding::Utf8);
        buf.push_rx(b"2", Encoding::Utf8);
        buf.push_rx(b"3", Encoding::Utf8);
        buf.push_rx(b"4", Encoding::Utf8);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.dropped(), 1);
        // 最新 3 行应在：2, 3, 4（1 被丢）
        let v: Vec<&TerminalLine> = buf.iter().collect();
        assert_eq!(v[0].text, "2");
        assert_eq!(v[2].text, "4");
    }
```

```rust
    #[test]
    fn empty_bytes_is_noop() {
        let mut buf = TerminalBuffer::new();
        buf.push_rx(b"", Encoding::Utf8);
        buf.push_tx(b"", Encoding::Utf8);
        assert_eq!(buf.len(), 0);
    }
```

```rust
    #[test]
    fn hex_pre_formatted_offline() {
        let mut buf = TerminalBuffer::new();
        buf.push_rx(&[0x01, 0x02, 0x03], Encoding::Utf8);
        let lines: Vec<_> = buf.iter().collect();
        assert_eq!(lines[0].hex, "01 02 03");
        assert_eq!(lines[0].text, "\u{1}\u{2}\u{3}");
        assert_eq!(lines[0].bytes_len, 3);
    }
```

再补一条覆盖「终端行按编码解码」的测试，追加到同一个 `mod tests` 里：

```rust
    #[test]
    fn terminal_line_decodes_by_encoding() {
        let mut buf = TerminalBuffer::new();
        buf.push_rx(&[0xD6, 0xD0, 0xCE, 0xC4], Encoding::Gbk);
        let lines: Vec<_> = buf.iter().collect();
        assert_eq!(lines[0].text, "中文");
        assert_eq!(lines[0].bytes_len, 4);
    }
```

- [ ] **Step 6：`main.rs` 注册模块**

在 `egui-app/src/main.rs` 的 `mod` 声明区加上：

```rust
mod codec;
```

- [ ] **Step 7：修 `backend_bridge.rs` 的调用点**

在 `egui-app/src/backend_bridge.rs` 的 `spawn_data_receiver` 里，把：

```rust
            {
                let mut term = shared.terminal.lock();
                term.push_rx(&bytes);
            }
```

改为：

```rust
            let enc = shared.ui.lock().encoding;
            {
                let mut term = shared.terminal.lock();
                term.push_rx(&bytes, enc);
            }
```

> **锁顺序说明**：`ui` 锁与 `terminal` 锁是先后获取、立即释放（`enc` 先取出再拿 terminal 锁），
> 不存在同时持有两把锁的时刻，不会有死锁风险。

- [ ] **Step 8：修 `app.rs` 的调用点**

在 `egui-app/src/app.rs` 的 `send_bytes` 方法里，把：

```rust
    pub fn send_bytes(&self, bytes: Vec<u8>, kind: &str) {
        match self.refs.backend.write_data(&bytes) {
            Ok(()) => {
                self.refs.shared.counters.add_tx(bytes.len());
                self.refs.shared.terminal.lock().push_tx(&bytes);
            }
```

改为：

```rust
    pub fn send_bytes(&self, bytes: Vec<u8>, kind: &str) {
        match self.refs.backend.write_data(&bytes) {
            Ok(()) => {
                self.refs.shared.counters.add_tx(bytes.len());
                let enc = self.refs.shared.ui.lock().encoding;
                self.refs.shared.terminal.lock().push_tx(&bytes, enc);
            }
```

- [ ] **Step 9：修 `send_panel.rs`（签名保持不变，只改内部编码路径）**

在 `egui-app/src/ui/send_panel.rs` 顶部加 import：

```rust
use crate::codec;
```

把 `show` 函数末尾的锁外写入块：

```rust
    // 在锁外写终端 + 计数（避免持锁期间 push）
    if !sent_text.is_empty() {
        let bytes = sent_text.as_bytes().to_vec();
        // 写入串口
        action = Some(SendAction::SendText(sent_text.clone()));
        // 本地回显到终端（TX 行）+ 计数（best-effort；tx_bytes 实际发送后再递增）
        shared.terminal.lock().push_tx(&bytes);
    }
```

改为：

```rust
    // 在锁外写终端 + 计数（避免持锁期间 push）
    if !sent_text.is_empty() {
        let enc = shared.ui.lock().encoding;
        let bytes = codec::encode(&sent_text, enc);
        // 写入串口
        action = Some(SendAction::SendText(sent_text.clone()));
        // 本地回显到终端（TX 行）
        shared.terminal.lock().push_tx(&bytes, enc);
    }
```

- [ ] **Step 10：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 16 passed; 0 failed`（7 原有 + 8 codec + 1 新增 terminal）。

```bash
grep -rn "push_rx(\|push_tx(" egui-app/src/
```

Expected: 每一处调用都有第二个 `Encoding` 参数。逐条核对：`terminal.rs` 测试内 5 处、`backend_bridge.rs` 1 处、`app.rs` 1 处、`send_panel.rs` 1 处。

---

### Task 4：重连状态徽章

`shared.reconnect: Mutex<Option<ReconnectEvent>>` 当前**只写不读**——`backend_bridge` 每次重连事件都往里塞，但 UI 一次都没画过。用户看到设备断开却没有任何反馈。

自动重连本身是白拿的：`core/src/backend.rs:805` 在 reader 线程清理时自己 `crate::config::load()` 读 `auto_reconnect`，不经过宿主。所以这里只缺显示。

**Files:**
- Create: `egui-app/src/ui/reconnect.rs`
- Modify: `egui-app/src/ui/mod.rs`
- Modify: `egui-app/src/state.rs`（`SharedState` 加 `reconnect_ok_until`）
- Modify: `egui-app/src/backend_bridge.rs`（`Succeeded` 时设置 3 秒窗口）
- Modify: `egui-app/src/ui/status_bar.rs`（渲染徽章）
- Modify: `egui-app/src/ui/theme.rs`（加 `success()`）

**Interfaces:**
- Consumes: Task 3 完成的代码
- Produces:
  - `ui::reconnect::Severity { Ok, Warn, Bad }`
  - `ui::reconnect::badge(re: &ReconnectEvent, ok_visible: bool) -> Option<(String, Severity)>`
  - `SharedState::reconnect_ok_until: Mutex<Option<Instant>>`（bridge 写、status_bar 读）
  - `theme::success() -> Color32`

- [ ] **Step 1：先写失败测试**

创建 `egui-app/src/ui/reconnect.rs`：

```rust
//! 重连状态徽章：把 `ReconnectEvent` 翻译成状态栏上的一行字
//!
//! 独立成文件是为了让这段状态机可以脱离 egui 单测——它只依赖
//! `ReconnectEvent` 的字段，不碰任何 UI 类型。

use crate::state::RECONNECT_OK_HOLD;
use egui::Color32;
use oh_my_serial_core::{ReconnectEvent, ReconnectPhase};

/// 徽章的严重级别，决定用哪个颜色
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Ok,
    Warn,
    Bad,
}

impl Severity {
    pub fn color(self) -> Color32 {
        match self {
            Severity::Ok => super::theme::success(),
            Severity::Warn => super::theme::warning(),
            Severity::Bad => super::theme::error(),
        }
    }
}

/// 把重连事件翻译成 `(文案, 级别)`。返回 `None` 表示不显示徽章。
///
/// `ok_visible` 由调用方根据「距重连成功是否已过 3 秒」决定——
/// 这样这段逻辑不依赖时钟，可直接单测。
pub fn badge(re: &ReconnectEvent, ok_visible: bool) -> Option<(String, Severity)> {
    match re.phase {
        // 刚触发、还没进入具体尝试：先不打扰用户
        ReconnectPhase::Started => None,
        ReconnectPhase::Attempt => Some((
            format!(
                "● 断开 · 重连 {}/{} · {}s 后重试",
                re.attempt,
                re.max_attempts,
                re.next_delay_ms / 1000
            ),
            Severity::Warn,
        )),
        // 重连成功后短暂显示，随后自动消失
        ReconnectPhase::Succeeded => {
            if ok_visible {
                Some(("● 已重连".to_string(), Severity::Ok))
            } else {
                None
            }
        }
        ReconnectPhase::Failed => {
            Some(("● 断开 · 重连失败".to_string(), Severity::Bad))
        }
        // 用户主动取消：连接已关闭，状态栏回到「○ 未连接」，无需额外徽章
        ReconnectPhase::Cancelled => None,
    }
}

/// 当前是否处于「重连成功后的展示窗口内」
pub fn ok_visible(until: Option<std::time::Instant>) -> bool {
    until.is_some_and(|t| std::time::Instant::now() < t)
}

/// 供 bridge 在收到 Succeeded 时计算窗口终点
pub fn ok_deadline() -> std::time::Instant {
    std::time::Instant::now() + RECONNECT_OK_HOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(phase: ReconnectPhase, attempt: u32, max: u32, delay_ms: u64) -> ReconnectEvent {
        ReconnectEvent {
            phase,
            attempt,
            max_attempts: max,
            next_delay_ms: delay_ms,
            message: String::new(),
        }
    }

    #[test]
    fn started_shows_nothing() {
        let e = ev(ReconnectPhase::Started, 0, 5, 1000);
        assert_eq!(badge(&e, false), None);
    }

    #[test]
    fn attempt_shows_countdown() {
        let e = ev(ReconnectPhase::Attempt, 2, 5, 5000);
        assert_eq!(
            badge(&e, false),
            Some(("● 断开 · 重连 2/5 · 5s 后重试".to_string(), Severity::Warn))
        );
    }

    #[test]
    fn attempt_truncates_delay_to_seconds() {
        let e = ev(ReconnectPhase::Attempt, 1, 5, 1999);
        let (text, _) = badge(&e, false).unwrap();
        assert!(text.contains("1s 后重试"), "实际: {text}");
    }

    #[test]
    fn succeeded_visible_then_gone() {
        let e = ev(ReconnectPhase::Succeeded, 1, 5, 0);
        assert_eq!(
            badge(&e, true),
            Some(("● 已重连".to_string(), Severity::Ok))
        );
        assert_eq!(badge(&e, false), None);
    }

    #[test]
    fn failed_is_permanent_bad() {
        let e = ev(ReconnectPhase::Failed, 5, 5, 0);
        // 无论 ok_visible 如何，失败都要一直显示
        assert_eq!(
            badge(&e, false),
            Some(("● 断开 · 重连失败".to_string(), Severity::Bad))
        );
    }

    #[test]
    fn cancelled_shows_nothing() {
        let e = ev(ReconnectPhase::Cancelled, 1, 5, 0);
        assert_eq!(badge(&e, false), None);
    }

    #[test]
    fn ok_visible_none_is_false() {
        assert!(!ok_visible(None));
    }
}
```

- [ ] **Step 2：运行测试**

```bash
cd egui-app && cargo test --bin oms-native reconnect::tests
```

Expected: **编译失败**，报 `cannot find function theme::success` 与 `cannot find constant state::RECONNECT_OK_HOLD`（下面两步会补上）。

- [ ] **Step 3：`state.rs` 加成功展示窗口常量**

在 `egui-app/src/state.rs` 顶部（import 之后）加上：

```rust
/// 重连成功后「● 已重连」徽章的展示时长
pub const RECONNECT_OK_HOLD: std::time::Duration = std::time::Duration::from_secs(3);
```

- [ ] **Step 4：`theme.rs` 加成功色**

在 `egui-app/src/ui/theme.rs` 追加：

```rust
/// 重连成功 / 正常状态：emerald-500
pub fn success() -> Color32 {
    Color32::from_rgb(16, 185, 129)
}
```

- [ ] **Step 5：`ui/mod.rs` 注册模块**

在 `egui-app/src/ui/mod.rs` 的模块声明里加一行（保持字母序，插在 `send_panel` 之后）：

```rust
pub mod reconnect;
```

- [ ] **Step 6：`SharedState` 加 `reconnect_ok_until` 字段**

在 `egui-app/src/state.rs` 的 `SharedState` 结构体里，`reconnect` 字段之后加：

```rust
    pub reconnect: Mutex<Option<ReconnectEvent>>,
    /// 「● 已重连」徽章的展示截止时刻。由 bridge 写，status_bar 读。
    pub reconnect_ok_until: Mutex<Option<std::time::Instant>>,
```

`SharedState` 派生了 `Default`，`Mutex<Option<Instant>>` 的 `Default` 存在，无需额外实现。

- [ ] **Step 7：`backend_bridge.rs` 在重连成功时设置窗口**

在 `egui-app/src/backend_bridge.rs` 的 `apply_event` 里，把 `BackendEvent::Reconnect(re) => {...}` 分支改为：

```rust
        BackendEvent::Reconnect(re) => {
            let msg = match re.phase {
                ReconnectPhase::Started => format!("[重连] 准备重连 ({}次)", re.max_attempts),
                ReconnectPhase::Attempt => format!(
                    "[重连] 第 {} 次尝试（{}秒后）",
                    re.attempt,
                    re.next_delay_ms / 1000
                ),
                ReconnectPhase::Succeeded => "[重连] 成功".to_string(),
                ReconnectPhase::Failed => "[重连] 失败，已放弃".to_string(),
                ReconnectPhase::Cancelled => "[重连] 已取消".to_string(),
            };
            shared.terminal.lock().push_system(msg);
            if re.phase == ReconnectPhase::Succeeded {
                *shared.reconnect_ok_until.lock() =
                    Some(std::time::Instant::now() + crate::state::RECONNECT_OK_HOLD);
            }
            *shared.reconnect.lock() = Some(re);
        }
```

- [ ] **Step 8：`status_bar.rs` 渲染徽章**

在 `egui-app/src/ui/status_bar.rs` 顶部加 import：

```rust
use super::reconnect;
```

把连接状态那个代码块（`ui.horizontal` 内的第一段）替换为：

```rust
        // 连接状态（重连徽章优先于常规文案）
        {
            let conn = shared.connection.lock();
            let re = shared.reconnect.lock().clone();
            let ok_until = *shared.reconnect_ok_until.lock();

            let badge = re
                .as_ref()
                .and_then(|r| reconnect::badge(r, reconnect::ok_visible(ok_until)));

            match badge {
                Some((text, sev)) => ui.colored_label(sev.color(), text),
                None if conn.is_open => {
                    let label = match (conn.open_port.as_deref(), conn.open_baud) {
                        (Some(name), baud) => format!("● {} @ {}", name, baud),
                        _ => "● 已连接".into(),
                    };
                    ui.colored_label(theme::tx_color(), label);
                    if conn.disconnected {
                        ui.colored_label(theme::warning(), "  ⚠ 设备已断开");
                    }
                }
                None => ui.colored_label(theme::text_secondary(), "○ 未连接"),
            }
        }
```

> `ReconnectEvent` 派生了 `Clone`，所以 `.clone()` 可用。

- [ ] **Step 9：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 23 passed; 0 failed`（16 + 7 个 reconnect 测试）。

---

### Task 5：配置持久化（与 Tauri 共用同一份 config.json）

**安全约束（本任务的核心）**：egui 与 Tauri 共用 `%APPDATA%/com.ohmyserial.app/config.json`。
若 egui 写回时把 Tauri 设的 `font_family` / `font_size` / `theme` 冲掉，用户会丢设置。

解法：**内存里持有完整的 `AppConfig`（`base`），每次写盘只覆盖 egui 拥有的 3 个字段**——
`last_port` / `baud_rate` / `encoding`（都是 toolbar 已经在编辑的）。
其余字段通过 `base.clone()` 天然保留。

`auto_reconnect` / `reconnect_max_attempts` **不需要** egui 持有：core 在 `backend.rs:805`
自己读配置，不经过宿主。

**Files:**
- Create: `egui-app/src/config_sync.rs`
- Modify: `egui-app/src/main.rs`
- Modify: `egui-app/src/state.rs`（`UiState` 加 `config: ConfigSync`）

**Interfaces:**
- Consumes: Task 3 的 `Encoding::as_str` / `Encoding::from_label`
- Produces:
  - `config_sync::merge_owned(base: &AppConfig, ui: &UiState) -> AppConfig`
  - `config_sync::apply_to_ui(cfg: &AppConfig, ui: &mut UiState)`
  - `config_sync::ConfigSync { base: AppConfig, pending_since: Option<Instant> }`
    - `ConfigSync::new(base: AppConfig) -> Self`
    - `ConfigSync::tick(&mut self, ui: &UiState, now: Instant) -> Option<AppConfig>`
  - `UiState::config: ConfigSync`
  - 常量 `config_sync::DEBOUNCE = Duration::from_millis(500)`

- [ ] **Step 1：先写失败测试**

创建 `egui-app/src/config_sync.rs`（测试与实现同文件，先写测试部分到文件末尾的 `mod tests`，实现见 Step 2）：

```rust
//! AppConfig ↔ UiState 桥接 + 500ms debounce 写盘
//!
//! **安全约束**：egui 与 Tauri 版共用同一份 `config.json`。若 egui 写回时
//! 覆盖了 Tauri 设的字段（字体、字号、主题…），用户会丢设置。
//! 因此 `ConfigSync` 持有**完整的** `AppConfig` 作为 `base`，
//! 每次写盘只在 base 之上覆盖 egui 真正拥有的 3 个字段，其余原样保留。
//!
//! egui 拥有的字段：last_port / baud_rate / encoding（都是 toolbar 在编辑的）。
//! `auto_reconnect` / `reconnect_max_attempts` 归 core 自己读（backend.rs:805），
//! egui 既不读也不写。

use crate::state::{Encoding, UiState};
use oh_my_serial_core::config::AppConfig;
use std::time::{Duration, Instant};

/// 设置变更到写盘的最小间隔
pub const DEBOUNCE: Duration = Duration::from_millis(500);

/// 把 UiState 里 egui 拥有的字段盖到 base 之上，其余字段原样保留
pub fn merge_owned(base: &AppConfig, ui: &UiState) -> AppConfig {
    let mut c = base.clone();
    c.last_port = ui.selected_port.clone();
    c.baud_rate = ui.baud_rate;
    c.encoding = ui.encoding.as_str().to_string();
    c
}

/// 把配置里 egui 拥有的字段灌进 UiState
pub fn apply_to_ui(cfg: &AppConfig, ui: &mut UiState) {
    ui.selected_port = cfg.last_port.clone();
    ui.baud_rate = cfg.baud_rate;
    ui.encoding = Encoding::from_label(&cfg.encoding);
}

pub struct ConfigSync {
    /// 上次写盘的完整配置；所有非 egui 字段靠它原样保留
    base: AppConfig,
    /// 首次检测到变更的时刻；None = 当前无待写变更
    pending_since: Option<Instant>,
}

impl ConfigSync {
    pub fn new(base: AppConfig) -> Self {
        Self {
            base,
            pending_since: None,
        }
    }

    /// 每帧调用。返回 `Some(cfg)` 表示此刻应该写盘。
    ///
    /// `now` 由调用方传入，便于单测控制时钟。
    pub fn tick(&mut self, ui: &UiState, now: Instant) -> Option<AppConfig> {
        let merged = merge_owned(&self.base, ui);

        if merged == self.base {
            // 变更已被撤销（如用户把波特率改回去了），取消待写
            self.pending_since = None;
            return None;
        }

        let since = *self.pending_since.get_or_insert(now);
        if now.duration_since(since) < DEBOUNCE {
            return None;
        }

        self.base = merged.clone();
        self.pending_since = None;
        Some(merged)
    }
}

impl Default for ConfigSync {
    fn default() -> Self {
        Self::new(AppConfig::default())
    }
}

impl std::fmt::Debug for ConfigSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigSync")
            .field("base", &self.base)
            .field("pending", &self.pending_since.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一份「Tauri 设过、egui 绝不能弄丢」的基线配置
    fn tauri_baseline() -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.font_family = "Consolas".to_string();
        cfg.font_size = 17;
        cfg.theme = "light".to_string();
        cfg.prompt_save_dialog = true;
        cfg.default_capture_path = r"D:\captures".to_string();
        cfg.auto_reconnect = false;
        cfg.reconnect_max_attempts = 9;
        cfg.buffer_size = 12345;
        cfg
    }

    #[test]
    fn merge_owned_preserves_every_foreign_field() {
        let base = tauri_baseline();
        let mut ui = UiState::default();
        ui.selected_port = Some("COM7".into());
        ui.baud_rate = 460800;
        ui.encoding = Encoding::Gbk;

        let merged = merge_owned(&base, &ui);

        // egui 拥有的 3 个字段确实变了
        assert_eq!(merged.last_port.as_deref(), Some("COM7"));
        assert_eq!(merged.baud_rate, 460800);
        assert_eq!(merged.encoding, "gbk");

        // 其余字段一个都不能动
        assert_eq!(merged.font_family, "Consolas");
        assert_eq!(merged.font_size, 17);
        assert_eq!(merged.theme, "light");
        assert!(merged.prompt_save_dialog);
        assert_eq!(merged.default_capture_path, r"D:\captures");
        assert!(!merged.auto_reconnect);
        assert_eq!(merged.reconnect_max_attempts, 9);
        assert_eq!(merged.buffer_size, 12345);
    }

    #[test]
    fn apply_to_ui_roundtrips_owned_fields() {
        let mut cfg = tauri_baseline();
        cfg.last_port = Some("COM3".into());
        cfg.baud_rate = 9600;
        cfg.encoding = "gbk".to_string();

        let mut ui = UiState::default();
        apply_to_ui(&cfg, &mut ui);

        assert_eq!(ui.selected_port.as_deref(), Some("COM3"));
        assert_eq!(ui.baud_rate, 9600);
        assert_eq!(ui.encoding, Encoding::Gbk);
    }

    #[test]
    fn apply_to_ui_unknown_encoding_falls_back_to_utf8() {
        let mut cfg = AppConfig::default();
        cfg.encoding = "big5".to_string();
        let mut ui = UiState::default();
        apply_to_ui(&cfg, &mut ui);
        assert_eq!(ui.encoding, Encoding::Utf8);
    }

    #[test]
    fn tick_does_not_write_before_debounce() {
        let mut cs = ConfigSync::new(tauri_baseline());
        let mut ui = UiState::default();
        ui.baud_rate = 230400;

        let t0 = Instant::now();
        assert!(cs.tick(&ui, t0).is_none(), "首次变更应只是开始计时");
        assert!(cs.tick(&ui, t0 + Duration::from_millis(499)).is_none());
        assert!(
            cs.tick(&ui, t0 + Duration::from_millis(500)).is_some(),
            "满 500ms 应触发写盘"
        );
    }

    #[test]
    fn tick_no_change_never_writes() {
        let mut cs = ConfigSync::new(tauri_baseline());
        let ui = UiState::default();
        let t0 = Instant::now();
        assert!(cs.tick(&ui, t0 + Duration::from_secs(60)).is_none());
    }

    #[test]
    fn tick_reverted_change_never_writes() {
        let mut cs = ConfigSync::new(tauri_baseline());
        let mut ui = UiState::default();
        let t0 = Instant::now();

        // 改一下再改回去
        ui.baud_rate = 230400;
        assert!(cs.tick(&ui, t0).is_none());
        ui.baud_rate = AppConfig::default().baud_rate;
        assert!(
            cs.tick(&ui, t0 + Duration::from_secs(10)).is_none(),
            "变更已撤销，不应写盘"
        );
    }

    #[test]
    fn after_write_second_change_gets_full_debounce() {
        let mut cs = ConfigSync::new(tauri_baseline());
        let mut ui = UiState::default();
        let t0 = Instant::now();

        ui.baud_rate = 230400;
        assert!(cs.tick(&ui, t0 + Duration::from_millis(500)).is_some());

        ui.baud_rate = 9600;
        assert!(
            cs.tick(&ui, t0 + Duration::from_millis(600)).is_none(),
            "第二次变更要重新计时"
        );
        assert!(cs.tick(&ui, t0 + Duration::from_millis(1100)).is_some());
    }

    #[test]
    fn written_config_keeps_foreign_fields() {
        let mut cs = ConfigSync::new(tauri_baseline());
        let mut ui = UiState::default();
        ui.baud_rate = 9600;

        let written = cs
            .tick(&ui, Instant::now() + Duration::from_millis(500))
            .expect("应写盘");
        assert_eq!(written.font_family, "Consolas");
        assert_eq!(written.theme, "light");
    }
}
```

- [ ] **Step 2：给 `UiState` 加 `config` 字段**

在 `egui-app/src/state.rs` 的 `UiState` 结构体末尾加：

```rust
    pub follow_tail: bool, // 是否自动滚到底部（终端新行）
    /// 配置持久化桥（持有完整 AppConfig，只覆盖 egui 拥有的字段）
    pub config: crate::config_sync::ConfigSync,
```

在 `impl Default for UiState` 里加对应初始化（放在 `follow_tail` 之后）：

```rust
            follow_tail: true,
            config: crate::config_sync::ConfigSync::default(),
```

- [ ] **Step 3：注册模块**

在 `egui-app/src/main.rs` 的 `mod` 声明区加上：

```rust
mod config_sync;
```

- [ ] **Step 4：启动时装载配置**

在 `egui-app/src/main.rs` 中，找到创建 `AppRefs`（`AppRefs::new()`）的位置，在其**之前**插入装载代码，并在 `AppRefs::new()` 之后把它灌进 UiState。

为清晰起见，把装载与灌入合并为一个函数，加到 `main.rs` 里：

```rust
/// 启动时读配置：先算出要注入 UiState 的 owned 字段，再在 AppRefs 建好后灌进去。
///
/// 顺序很重要——`AppRefs::new()` 会创建全新的 SharedState，
/// 在它之前无法写入，所以先 load 再 apply。
fn bootstrap_config(refs: &crate::state::AppRefs) {
    let cfg = oh_my_serial_core::config::load();
    {
        let mut ui = refs.shared.ui.lock();
        crate::config_sync::apply_to_ui(&cfg, &mut ui);
        ui.config = crate::config_sync::ConfigSync::new(cfg);
    }
    log::info!("[config] 已加载：{}", oh_my_serial_core::config::config_path().display());
}
```

然后在创建出 `refs` 之后、`AppRefs::new()` 返回值被使用之前，加一行调用：

```rust
    bootstrap_config(&refs);
```

> 若 `main.rs` 里 `refs` 的实际变量名不同，用它自己的名字。

- [ ] **Step 5：每帧 tick，触发写盘**

在 `egui-app/src/app.rs` 的 `ui()` 方法**最末尾**（Task 2 加的心跳那一行之后）加上：

```rust
        // 配置写盘：变更后静默 500ms 再落盘，避免拖动滑块时狂写
        let mut ui_state = self.refs.shared.ui.lock();
        if let Some(cfg) = ui_state.config.tick(&ui_state, std::time::Instant::now()) {
            drop(ui_state);
            if let Err(e) = oh_my_serial_core::config::save(&cfg) {
                log::warn!("[config] 保存失败: {e}");
            }
        }
```

> 这段有个 Rust 借用细节：`ui_state` 是 `MutexGuard`，`ui_state.config.tick(&ui_state, ..)`
> 同时可变借用了 guard 的字段、不可变借用了 guard 整体。`parking_lot::MutexGuard`
> 允许这种分裂借用（它实现了 `Deref`/`DerefMut`，借用检查器按字段拆分），
> **但若编译报错**，改用下面这个等价写法：
>
> ```rust
>         let now = std::time::Instant::now();
>         let to_save = {
>             let mut ui_state = self.refs.shared.ui.lock();
>             ui_state.config.tick(&ui_state, now)
>         };
>         if let Some(cfg) = to_save {
>             if let Err(e) = oh_my_serial_core::config::save(&cfg) {
>                 log::warn!("[config] 保存失败: {e}");
>             }
>         }
> ```

- [ ] **Step 6：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 31 passed; 0 failed`（23 + 8 个 config_sync 测试）。

- [ ] **Step 7：手工验证「不冲掉 Tauri 设置」**

这是本任务最重要的验收点，必须实测：

```bash
# 1) 备份现有配置
cp "$APPDATA/com.ohmyserial.app/config.json" /tmp/oms-config-backup.json 2>/dev/null || echo "无现存配置"
# 2) 打开 Tauri 版，改字体字号，保存 → 关闭
# 3) 启动 egui 版，随便动一下波特率再改回去（触发一次写盘），关闭
cat "$APPDATA/com.ohmyserial.app/config.json"
```

Expected: `font_family` / `font_size` / `theme` 仍是 Tauri 设的值，没被 egui 冲掉。

```bash
# 收尾还原
cp /tmp/oms-config-backup.json "$APPDATA/com.ohmyserial.app/config.json" 2>/dev/null || true
```

---

## 批次 ② 定时发送面板

> 到这里批次 ① 完成，`oms-native` 应能正常使用：**空闲 CPU 大幅下降、GBK 可用、
> 断线有重连提示、设置能持久化且不冲掉 Tauri**。
> 建议此时停下来跑一次实机验证再继续。

### Task 6：右栏标签页骨架 + 统一 PanelAction

现状终端与发送面板是在 `CentralPanel` 里用 `ui.horizontal` + `allocate_ui` 手搓的 70/30。
加队列、录制之后右栏会挤爆，必须先立骨架。

**核心约定（后续面板都照抄）**：
> **面板是纯函数** —— 读 `SharedState`、返回 `Option<PanelAction>`，**绝不直接调 `backend`**。
> 所有副作用集中在 `app.rs` 的单个 `match` 里。

**Files:**
- Modify: `egui-app/src/ui/mod.rs`
- Modify: `egui-app/src/state.rs`（加 `SideTab`、`QueueItem`、队列与周期发送的 UI 字段）
- Modify: `egui-app/src/ui/toolbar.rs`（迁到统一 Action）
- Modify: `egui-app/src/ui/send_panel.rs`（迁到统一 Action）
- Modify: `egui-app/src/app.rs`（布局重构 + 统一 match）

**Interfaces:**
- Consumes: Task 1–5 完成的代码
- Produces:
  - `ui::PanelAction { OpenPort, ClosePort, RefreshPorts, SendBytes { bytes: Vec<u8>, label: &'static str }, QueueChanged, QueueTogglePolling(bool), PeriodicToggle(bool) }`
  - `state::SideTab { Send, Scheduled, Record }`
  - `state::QueueItem { id: String, content: String, is_hex: bool, priority: u8, interval_ms: u64 }`
  - `UiState` 新字段：`active_tab`、`queue: Vec<QueueItem>`、`queue_seq: u64`、
    `periodic_running: bool`、`periodic_content: String`、`periodic_is_hex: bool`、
    `periodic_interval_ms: u64`、`periodic_confirm_until: Option<Instant>`
  - `toolbar::show(...) -> Option<PanelAction>`、`send_panel::show(...) -> Option<PanelAction>`

- [ ] **Step 1：`state.rs` 加标签枚举与队列条目类型**

在 `egui-app/src/state.rs` 中，`ViewMode` 定义之后追加：

```rust
/// 右侧栏的三个标签页
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SideTab {
    Send,
    Scheduled,
    Record,
}

impl SideTab {
    pub const ALL: [SideTab; 3] = [SideTab::Send, SideTab::Scheduled, SideTab::Record];

    pub fn label(self) -> &'static str {
        match self {
            SideTab::Send => "发送",
            SideTab::Scheduled => "定时发送",
            SideTab::Record => "录制",
        }
    }
}

/// 队列中的一条命令（UI 侧可编辑的真相；推给 core 前按 priority 降序）
#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub id: String,
    pub content: String,
    pub is_hex: bool,
    /// 高=255 / 中=128 / 低=0；core 的 queue_add 按此降序重排
    pub priority: u8,
    pub interval_ms: u64,
}

impl QueueItem {
    /// 三档优先级的中间档，新建条目的默认值
    pub const DEFAULT_PRIORITY: u8 = 128;

    pub fn new(id: String) -> Self {
        Self {
            id,
            content: String::new(),
            is_hex: false,
            priority: Self::DEFAULT_PRIORITY,
            interval_ms: 1000,
        }
    }

    pub fn priority_label(&self) -> &'static str {
        match self.priority {
            p if p >= 255 => "高",
            p if p >= 128 => "中",
            _ => "低",
        }
    }

    pub fn priority_value(label: &str) -> u8 {
        match label {
            "高" => 255,
            "低" => 0,
            _ => 128,
        }
    }
}
```

- [ ] **Step 2：`UiState` 加新字段**

在 `egui-app/src/state.rs` 的 `UiState` 结构体里，`config` 字段之后追加：

```rust
    /// 当前选中的右侧栏标签
    pub active_tab: SideTab,
    /// 队列条目（UI 侧可编辑的真相）
    pub queue: Vec<QueueItem>,
    /// id 生成计数器（单调递增，避免引入 uuid 依赖）
    pub queue_seq: u64,
    /// 周期发送是否在跑。core 侧状态不可信（见 backend.rs send-precise），
    /// 由 UI 自记 + SendPreciseError 事件纠正
    pub periodic_running: bool,
    pub periodic_content: String,
    pub periodic_is_hex: bool,
    pub periodic_interval_ms: u64,
    /// 「再点一次确认清空队列」的截止时刻
    pub periodic_confirm_until: Option<std::time::Instant>,
```

在 `impl Default for UiState` 里追加对应初值：

```rust
            active_tab: SideTab::Send,
            queue: Vec::new(),
            queue_seq: 0,
            periodic_running: false,
            periodic_content: String::new(),
            periodic_is_hex: false,
            periodic_interval_ms: 1000,
            periodic_confirm_until: None,
```

并把结构体上的 `#[allow(dead_code)]` 属性**删除**——本任务起这些字段陆续会被用到。
若删掉后出现 unused 警告，说明某个字段确实还没接上，保留该字段并加一行
`#[allow(dead_code)] // <字段名>：Task 7 接线` 到该字段上方。

- [ ] **Step 3：`ui/mod.rs` 定义统一 PanelAction**

把 `egui-app/src/ui/mod.rs` 替换为：

```rust
//! UI 组件模块
//!
//! **面板约定**：面板是纯函数——读 `SharedState`、返回 `Option<PanelAction>`，
//! **绝不直接调 `backend`**。所有副作用由 `app.rs` 在单个 `match` 里执行。

pub mod reconnect;
pub mod scheduled_panel;
pub mod send_panel;
pub mod status_bar;
pub mod terminal;
pub mod theme;
pub mod toolbar;

/// 面板向上层请求的动作。`app.rs` 统一 match 后执行副作用。
#[derive(Debug, Clone, PartialEq)]
pub enum PanelAction {
    OpenPort,
    ClosePort,
    RefreshPorts,
    /// 直接发送一段字节；`label` 用于失败时的终端提示
    SendBytes { bytes: Vec<u8>, label: &'static str },
    /// 队列内容变了，需要同步到 core（clear + 逐条 add）
    QueueChanged,
    /// 启停队列轮询
    QueueTogglePolling(bool),
    /// 启停周期发送
    PeriodicToggle(bool),
}
```

> `scheduled_panel` 模块在 Task 7 才创建。若本任务尚未创建该文件，先从上面列表里去掉那一行，
> Task 7 开头再加回。

- [ ] **Step 4：`toolbar.rs` 迁到统一 Action**

在 `egui-app/src/ui/toolbar.rs` 顶部把：

```rust
use crate::state::{Encoding, SharedState, UiState, ViewMode};
```

改为：

```rust
use super::PanelAction;
use crate::state::{Encoding, SharedState, UiState, ViewMode};
```

把签名：

```rust
pub fn show(ui: &mut Ui, shared: &SharedState, open_port: bool) -> Option<UiAction> {
    let mut action: Option<UiAction> = None;
```

改为：

```rust
pub fn show(ui: &mut Ui, shared: &SharedState, open_port: bool) -> Option<PanelAction> {
    let mut action: Option<PanelAction> = None;
```

把刷新端口：

```rust
        if ui.button("🔄 刷新").clicked() {
            action = Some(UiAction::RefreshPorts);
        }
```

改为：

```rust
        if ui.button("🔄 刷新").clicked() {
            action = Some(PanelAction::RefreshPorts);
        }
```

把打开/关闭串口按钮：

```rust
            if ui
                .add(egui::Button::new(label).fill(theme::accent()))
                .clicked()
            {
                action = Some(UiAction::TogglePort);
            }
```

改为：

```rust
            if ui
                .add(egui::Button::new(label).fill(theme::accent()))
                .clicked()
            {
                action = Some(PanelAction::open_or_close(open_port));
            }
```

其中 `if_open` 是 `PanelAction` 的小助手，在 `ui/mod.rs` 的 `impl PanelAction` 里加上：

```rust
impl PanelAction {
    /// 工具栏的「打开/关闭串口」按钮：已知当前是否已打开
    pub fn open_or_close(open: bool) -> Self {
        if open {
            PanelAction::ClosePort
        } else {
            PanelAction::OpenPort
        }
    }
}
```

最后删除文件末尾的旧枚举：

```rust
/// 工具栏返回的动作（None = 无动作）
pub enum UiAction {
    RefreshPorts,
    TogglePort,
}
```

- [ ] **Step 5：`send_panel.rs` 迁到统一 Action**

在 `egui-app/src/ui/send_panel.rs` 顶部把：

```rust
use crate::state::SharedState;
use egui::{Key, Ui};

pub enum SendAction {
    SendText(String),
    SendHex(String),
}
```

改为：

```rust
use super::PanelAction;
use crate::codec;
use crate::state::SharedState;
use egui::{Key, Ui};
```

把签名：

```rust
pub fn show(ui: &mut Ui, shared: &SharedState) -> Option<SendAction> {
    let mut action: Option<SendAction> = None;
    let mut sent_text = String::new();
```

改为：

```rust
pub fn show(ui: &mut Ui, shared: &SharedState) -> Option<PanelAction> {
    let mut action: Option<PanelAction> = None;
    let mut sent_text = String::new();
    let mut sent_hex = String::new();
```

在输入框之后、按钮行之前插入 HEX 输入框（原来 HEX 发送是靠 app.rs 里硬编码的
`pending_send_hex` 字段，UI 上没有入口，这次补上真实入口）：

```rust
        ui.add_space(2.0);
        let mut ui_state_hex = shared.ui.lock();
        let resp_hex = ui.add(
            egui::TextEdit::singleline(&mut ui_state_hex.pending_send_hex)
                .hint_text("HEX 发送，如 AA 01 或 0xAA 0x01")
                .desired_width(ui.available_width()),
        );
        if resp_hex.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            if !ui_state_hex.pending_send_hex.is_empty() {
                sent_hex = ui_state_hex.pending_send_hex.clone();
                ui_state_hex.pending_send_hex.clear();
            }
        }
        drop(ui_state_hex);
```

在按钮行之后追加 HEX 发送按钮：

```rust
        if ui.button("🔢 HEX 发送").clicked() && !shared.ui.lock().pending_send_hex.is_empty() {
            sent_hex = shared.ui.lock().pending_send_hex.clone();
            shared.ui.lock().pending_send_hex.clear();
        }
```

把锁外写入块替换为下面这版。**要点：面板只产出 `PanelAction`，不做 `push_tx`**——
回显由 `app.rs` 的 `EguiApp::send_bytes` 在真正写串口成功后统一执行。
若面板这里也回显，`app.rs` 会再回显一次，TX 行会重复。

```rust
    // 面板只产出动作，不做回显——回显由 app.rs 在写串口成功后统一执行
    if !sent_text.is_empty() {
        let enc = shared.ui.lock().encoding;
        let bytes = codec::encode(&sent_text, enc);
        action = Some(PanelAction::SendBytes { bytes, label: "发送" });
    }
    if !sent_hex.is_empty() {
        match parse_hex_input(&sent_hex) {
            Some(bytes) if !bytes.is_empty() => {
                action = Some(PanelAction::SendBytes {
                    bytes,
                    label: "HEX 发送",
                });
            }
            _ => {
                shared.ui.lock().error_msg = Some("HEX 解析失败".into());
            }
        }
    }

    action
```

同时删除文件顶部的 `#![allow(dead_code)] // MVP 范围仅文本发送；HEX 输入留待 v0.2`
（HEX 入口本轮接上了）。

- [ ] **Step 6：`app.rs` 布局重构**

把 `EguiApp::open_port` / `close_port` 之间的 `TogglePort` 处理改为展开的 `OpenPort` / `ClosePort`。
把 `ui()` 方法整体替换为：

```rust
    /// eframe 0.35: `ui` 替代了 0.30 的 `update`
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        // 错误提示
        if let Some(err) = self.refs.shared.ui.lock().error_msg.clone() {
            egui::Window::new("提示")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.colored_label(theme::error(), &err);
                    if ui.button("关闭").clicked() {
                        self.refs.shared.ui.lock().error_msg = None;
                    }
                });
        }

        egui::Panel::top("toolbar").show(ui, |ui| {
            if let Some(action) = toolbar::show(ui, &self.refs.shared, self.is_open()) {
                self.handle(action);
            }
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            status_bar::show(ui, &self.refs.shared);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            terminal::show(ui, &self.refs.shared);
        });

        egui::SidePanel::right("sidebar")
            .exact_width(400.0)
            .show(ctx, |ui| {
                self.show_sidebar(ui);
            });

        // 10Hz 心跳：让重连倒计时、状态栏计时器持续走字。
        // 有数据/事件时 backend_bridge 会立即 request_repaint()，不受此节流影响。
        ctx.request_repaint_after(Duration::from_millis(100));
    }
```

- [ ] **Step 7：`app.rs` 加侧栏渲染**

在 `impl EguiApp` 里（`send_bytes` 之后）加上：

```rust
    /// 右侧栏：顶部标签条 + 当前面板内容
    fn show_sidebar(&self, ui: &mut egui::Ui) {
        let tab = self.refs.shared.ui.lock().active_tab;

        ui.horizontal(|ui| {
            for t in SideTab::ALL {
                let sel = t == tab;
                if ui.selectable_label(sel, t.label()).clicked() {
                    self.refs.shared.ui.lock().active_tab = t;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match tab {
                SideTab::Send => {
                    if let Some(action) = send_panel::show(ui, &self.refs.shared) {
                        self.handle(action);
                    }
                }
                SideTab::Scheduled => {
                    if let Some(action) = scheduled_panel::show(ui, &self.refs.shared) {
                        self.handle(action);
                    }
                }
                SideTab::Record => {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("录制功能将在下一阶段提供")
                            .color(theme::text_secondary())
                            .italics(),
                    );
                }
            });
    }

    /// 唯一的副作用出口：面板动作在这里被翻译成 backend 调用
    fn handle(&self, action: PanelAction) {
        match action {
            PanelAction::RefreshPorts => self.refresh_ports(),
            PanelAction::OpenPort => self.open_port(),
            PanelAction::ClosePort => self.close_port(),
            PanelAction::SendBytes { bytes, label } => self.send_bytes(bytes, label),
            PanelAction::QueueChanged => self.queue_changed(),
            PanelAction::QueueTogglePolling(start) => self.set_queue_polling(start),
            PanelAction::PeriodicToggle(start) => self.set_periodic(start),
        }
    }
```

在 `egui-app/src/app.rs` 顶部把 use 改为：

```rust
use crate::state::{AppRefs, QueueItem, SideTab};
use crate::ui::{send_panel, scheduled_panel, status_bar, terminal, theme, toolbar, PanelAction};
```

- [ ] **Step 8：先建空的 `scheduled_panel` 占位**

创建 `egui-app/src/ui/scheduled_panel.rs`：

```rust
//! 定时发送面板：队列轮询 + 周期发送
//!
//! Task 6 先建骨架，Task 7 填队列，Task 8 填周期发送。
//! 本阶段返回 `None`——不产生任何动作。

use crate::state::SharedState;
use egui::Ui;
use super::PanelAction;

pub fn show(_ui: &mut Ui, _shared: &SharedState) -> Option<PanelAction> {
    None
}
```

- [ ] **Step 9：`app.rs` 加三个副作用方法**

在 `impl EguiApp` 里（`send_bytes` 之后）加上：

```rust
    /// 把 UI 侧队列全量同步到 core。
    ///
    /// core 的 `queue_add` 会按 priority 降序重排，所以这里也按降序推，
    /// 保证 UI 显示顺序 == 实际发送顺序。
    fn queue_changed(&self) {
        let items = self.sorted_queue();
        if let Err(e) = self.refs.backend.queue_clear() {
            self.set_error(format!("队列清空失败: {e}"));
            return;
        }
        for it in items {
            let enc = self.refs.shared.ui.lock().encoding;
            let bytes = match payload_bytes(&it.content, it.is_hex, enc) {
                Some(b) if !b.is_empty() => b,
                _ => continue, // 内容为空或 HEX 非法：跳过，不让一条坏数据毁掉整个队列
            };
            let cmd = oh_my_serial_core::SendCommand {
                id: it.id.clone(),
                content: bytes,
                priority: it.priority,
                interval_ms: it.interval_ms,
            };
            if let Err(e) = self.refs.backend.queue_add(cmd) {
                self.set_error(format!("队列添加失败: {e}"));
                return;
            }
        }
    }

    /// 按 priority 降序排列（core 的 queue_add 用同样的规则）
    fn sorted_queue(&self) -> Vec<QueueItem> {
        let mut v = self.refs.shared.ui.lock().queue.clone();
        v.sort_by(|a, b| b.priority.cmp(&a.priority));
        v
    }

    fn set_queue_polling(&self, start: bool) {
        let r = if start {
            self.refs.backend.queue_start_polling()
        } else {
            self.refs.backend.queue_stop_polling()
        };
        if let Err(e) = r {
            self.set_error(format!("{}队列轮询失败: {e}", if start { "启动" } else { "停止" }));
        }
    }

    fn set_periodic(&self, start: bool) {
        if start {
            let (content, is_hex) = {
                let ui = self.refs.shared.ui.lock();
                (ui.periodic_content.clone(), ui.periodic_is_hex)
            };
            let enc = self.refs.shared.ui.lock().encoding;
            let bytes = match payload_bytes(&content, is_hex, enc) {
                Some(b) if !b.is_empty() => b,
                _ => {
                    self.set_error("周期发送内容为空或 HEX 非法".into());
                    return;
                }
            };
            let interval = self.refs.shared.ui.lock().periodic_interval_ms.max(10);
            if let Err(e) = self.refs.backend.start_periodic_send(bytes, interval) {
                self.set_error(format!("启动周期发送失败: {e}"));
                return;
            }
            self.refs.shared.ui.lock().periodic_running = true;
        } else {
            if let Err(e) = self.refs.backend.stop_periodic_send() {
                self.set_error(format!("停止周期发送失败: {e}"));
                return;
            }
            self.refs.shared.ui.lock().periodic_running = false;
        }
    }

    fn set_error(&self, msg: String) {
        self.refs.shared.ui.lock().error_msg = Some(msg);
    }
```

在 `egui-app/src/app.rs` 末尾（所有 `impl` 之后）加自由函数：

```rust
/// 把面板里的一行输入转成待发送字节。HEX 非法或内容为空时返回 `None`。
fn payload_bytes(content: &str, is_hex: bool, enc: Encoding) -> Option<Vec<u8>> {
    if is_hex {
        crate::ui::send_panel::parse_hex_input(content).filter(|b| !b.is_empty())
    } else if content.is_empty() {
        None
    } else {
        Some(crate::codec::encode(content, enc))
    }
}
```

顶部 use 补上 `Encoding`：

```rust
use crate::state::{AppRefs, Encoding, QueueItem, SideTab};
```

- [ ] **Step 10：`backend_bridge.rs` 纠正 periodic_running**

在 `egui-app/src/backend_bridge.rs` 的 `apply_event` 里，把 `SendPreciseError` 分支：

```rust
        BackendEvent::SendPreciseError(e) => {
            shared
                .terminal
                .lock()
                .push_system(format!("[周期发送失败] {}", short_err(&e)));
        }
```

改为：

```rust
        BackendEvent::SendPreciseError(e) => {
            shared
                .terminal
                .lock()
                .push_system(format!("[周期发送失败] {}", short_err(&e)));
            // send-precise 线程写失败后直接 break，且 core 不会把 stop flag 置回 true
            // （backend.rs 的循环里没有 store(true)），所以 core 侧状态不可信。
            // 由这个事件把 UI 的乐观标志纠正回来，否则面板会一直显示「运行中」。
            shared.ui.lock().periodic_running = false;
        }
```

- [ ] **Step 11：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 31 passed; 0 failed`（本任务不增删测试）。

```bash
cd egui-app && cargo clippy --bin oms-native -- -D warnings
```

Expected: 无警告退出。若报 `needless_return`、`redundant_clone` 等，按提示修。

- [ ] **Step 12：实跑确认布局**

```bash
cd egui-app && cargo build --bin oms-native && cd .. && cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
```

```bash
sleep 8 && tasklist | grep -ci oms-native
```

Expected: ≥ 1。

肉眼确认：终端占满左侧，右侧 400px 栏有三个标签（发送 / 定时发送 / 录制），
点「定时发送」显示占位文案，点「录制」显示占位文案。

```bash
taskkill //F //IM oms-native.exe
```

---

### Task 7：队列轮询面板

**易错点回顾**（core 实际行为，与直觉不符）：
1. 队列**非消费** —— 且 poller 只反复发送 `commands.first()` 那一条（见下方更正）
2. `queue_add` 会按 priority 降序重排
3. `queue_status()` 不返回命令内容，UI 必须自己存一份

**Files:**
- Modify: `egui-app/src/ui/scheduled_panel.rs`
- Modify: `egui-app/src/state.rs`（`UiState` 加队列增删的纯逻辑方法）

**Interfaces:**
- Consumes: Task 6 的 `PanelAction::{QueueChanged, QueueTogglePolling}`、`UiState.queue`、`app.rs::sorted_queue`
- Produces: `UiState::add_queue_item(&mut self) -> String`（返回新条目 id）、`UiState::remove_queue_item(&mut self, id: &str)`

- [ ] **Step 1：先写失败测试**

在 `egui-app/src/state.rs` 的 `impl Default for UiState` **之后**追加一个 `#[cfg(test)]` 块
（state.rs 目前没有测试模块，新建一个）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ui() -> UiState {
        UiState::default()
    }

    #[test]
    fn add_queue_item_generates_unique_ascending_ids() {
        let mut s = ui();
        let a = s.add_queue_item();
        let b = s.add_queue_item();
        assert_ne!(a, b, "id 必须唯一");
        assert_eq!(s.queue.len(), 2);
        assert!(a < b, "id 应单调递增，便于排序: {a} vs {b}");
    }

    #[test]
    fn new_item_defaults_to_medium_priority_1s() {
        let mut s = ui();
        let id = s.add_queue_item();
        let it = s.queue.iter().find(|i| i.id == id).unwrap();
        assert_eq!(it.priority, QueueItem::DEFAULT_PRIORITY);
        assert_eq!(it.priority_label(), "中");
        assert_eq!(it.interval_ms, 1000);
        assert!(it.content.is_empty());
        assert!(!it.is_hex);
    }

    #[test]
    fn remove_queue_item_only_removes_target() {
        let mut s = ui();
        let a = s.add_queue_item();
        let b = s.add_queue_item();
        s.remove_queue_item(&a);
        assert_eq!(s.queue.len(), 1);
        assert_eq!(s.queue[0].id, b);
    }

    #[test]
    fn remove_unknown_id_is_noop() {
        let mut s = ui();
        s.add_queue_item();
        s.remove_queue_item("q999999");
        assert_eq!(s.queue.len(), 1);
    }

    #[test]
    fn remove_invalidates_confirm_gate() {
        // 队列被改动后，「确认清空」窗口必须作废，否则用户会基于过期认知点掉队列
        let mut s = ui();
        s.add_queue_item();
        s.periodic_confirm_until = Some(std::time::Instant::now());
        s.remove_queue_item("q0");
        assert!(s.periodic_confirm_until.is_none());
    }

    #[test]
    fn add_invalidates_confirm_gate() {
        let mut s = ui();
        s.periodic_confirm_until = Some(std::time::Instant::now());
        s.add_queue_item();
        assert!(s.periodic_confirm_until.is_none());
    }

    #[test]
    fn priority_label_value_roundtrip() {
        for label in ["高", "中", "低"] {
            let v = QueueItem::priority_value(label);
            assert_eq!(QueueItem { priority: v, ..QueueItem::new("x".into()) }.priority_label(), label);
        }
    }

    #[test]
    fn priority_value_defaults_to_medium() {
        assert_eq!(QueueItem::priority_value("不存在的档位"), 128);
    }
}
```

- [ ] **Step 2：运行测试**

```bash
cd egui-app && cargo test --bin oms-native state::tests
```

Expected: **编译失败**，报 `cannot find method add_queue_item` / `remove_queue_item`。

- [ ] **Step 3：`UiState` 加队列增删方法**

在 `egui-app/src/state.rs` 中，`impl Default for UiState` 之后追加：

```rust
impl UiState {
    /// 追加一条空队列条目，返回其 id
    pub fn add_queue_item(&mut self) -> String {
        self.queue_seq += 1;
        let id = format!("q{}", self.queue_seq);
        self.queue.push(QueueItem::new(id.clone()));
        self.periodic_confirm_until = None;
        id
    }

    /// 按 id 移除队列条目；队列内容变化时作废「确认清空」窗口
    pub fn remove_queue_item(&mut self, id: &str) {
        self.queue.retain(|i| i.id != id);
        self.periodic_confirm_until = None;
    }

    /// 清空队列并作废确认窗口（「全部清空」按钮用）
    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.periodic_confirm_until = None;
    }
}
```

- [ ] **Step 4：验证纯逻辑**

```bash
cd egui-app && cargo test --bin oms-native state::tests
```

Expected: `8 passed; 0 failed`。全量测试应为 `39 passed`。

- [ ] **Step 5：写队列面板 UI**

把 `egui-app/src/ui/scheduled_panel.rs` 整体替换为：

```rust
//! 定时发送面板：队列轮询 + 周期发送
//!
//! 队列语义（core 实际行为，勿凭直觉改）：
//! - **非消费**：`next_command()` 返回 `commands.first()` 且不移除，**也不推进游标**，
//!   所以轮询**只反复发送队首（priority 最高的）那一条**，其余命令一次都不会发出
//!   —— 不是「循环重发整个列表」。（2026-09-29 更正：本行原先写「循环重发整个列表」，
//!   是错的；真机上 3 条命令轮询时线缆上只出现队首那条。见文末「core 语义速查」）
//! - `interval_ms` 是该命令**发完后**的等待。
//! - `queue_add` 会按 `priority` 降序重排，所以 UI 也按降序显示与推送。
//!
//! `queue_status()` 只返回 `{count, is_polling}`，不返回命令内容，
//! 所以条目列表必须由 UI 自己持有（`UiState.queue`）。

use super::PanelAction;
use crate::state::{QueueItem, SharedState, SideTab};
use crate::ui::theme;
use egui::{DragValue, Ui};

pub fn show(ui: &mut Ui, shared: &SharedState, polling: bool) -> Option<PanelAction> {
    let mut action: Option<PanelAction> = None;

    // `polling` 由调用方从 core 的 `queue_status().is_polling` 读出传入：
    // SharedState 里没有 backend 引用，且这个值必须取 core 的真值
    // （轮询线程退出时 core 会正确置位，可信）
    show_queue_block(ui, shared, polling, &mut action);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    show_placeholder(ui);

    action
}

fn show_queue_block(
    ui: &mut Ui,
    shared: &SharedState,
    polling: bool,
    action: &mut Option<PanelAction>,
) {
    let mut ui_state = shared.ui.lock();

    let count = ui_state.queue.len();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("队列").strong());
        ui.label(format!("({} 条)", count));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !ui_state.queue.is_empty() && ui.button("全部清空").clicked() {
                ui_state.clear_queue();
                *action = Some(PanelAction::QueueChanged);
            }
        });
    });

    if ui_state.queue.is_empty() {
        ui.label(
            egui::RichText::new("（队列为空，点「+ 添加一行」加入命令）")
                .color(theme::text_secondary())
                .italics(),
        );
    }

    // 按 priority 降序渲染，与 core 的排序规则一致
    let mut order: Vec<usize> = (0..ui_state.queue.len()).collect();
    order.sort_by(|&a, &b| ui_state.queue[b].priority.cmp(&ui_state.queue[a].priority));

    let mut to_remove: Option<String> = None;
    for idx in order {
        let item = ui_state.queue[idx].clone();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("prio", item.id.as_str()))
                .selected_text(item.priority_label())
                .width(56.0)
                .show_ui(ui, |ui| {
                    for label in ["高", "中", "低"] {
                        if ui
                            .selectable_label(item.priority_label() == label, label)
                            .clicked()
                        {
                            ui_state.queue[idx].priority = QueueItem::priority_value(label);
                            *action = Some(PanelAction::QueueChanged);
                        }
                    }
                });

            if ui
                .add(
                    egui::SelectableLabel::new(
                        item.is_hex,
                        egui::RichText::new(if item.is_hex { "HEX" } else { "TXT" })
                            .small()
                            .monospace(),
                    ),
                )
                .clicked()
            {
                ui_state.queue[idx].is_hex = !item.is_hex;
                *action = Some(PanelAction::QueueChanged);
            }

            ui.add(
                egui::TextEdit::singleline(&mut ui_state.queue[idx].content)
                    .desired_width(f32::INFINITY)
                    .hint_text("命令内容"),
            );

            ui.add(DragValue::new(&mut ui_state.queue[idx].interval_ms).speed(10))
                .on_hover_text("该命令发送后的等待时长（ms）");
            ui.label("ms");

            if ui.small_button("✖").clicked() {
                to_remove = Some(item.id.clone());
            }
        });
    }

    if let Some(id) = to_remove {
        ui_state.remove_queue_item(&id);
        *action = Some(PanelAction::QueueChanged);
    }

    ui.horizontal(|ui| {
        if ui.button("+ 添加一行").clicked() {
            ui_state.add_queue_item();
            *action = Some(PanelAction::QueueChanged);
        }
    });

    ui.horizontal(|ui| {
        if polling {
            if ui.button("■ 停止轮询").clicked() {
                *action = Some(PanelAction::QueueTogglePolling(false));
            }
            ui.colored_label(theme::warning(), "轮询中…（反复发送队首那条）");
        } else {
            let periodic_busy = ui_state.periodic_running;
            let resp = ui
                .add_enabled(
                    !periodic_busy,
                    egui::Button::new("▶ 开始轮询").fill(theme::accent()),
                ))
                .on_hover_text(if periodic_busy {
                    "周期发送运行中，两者互斥：周期发送会清空队列"
                } else {
                    "按 priority 降序循环发送队列中的每条命令"
                });
            if resp.clicked() {
                // 队列为空时没有可发的东西，直接忽略
                if !ui_state.queue.is_empty() {
                    ui_state.periodic_confirm_until = None;
                    *action = Some(PanelAction::QueueChanged);
                    *action = Some(PanelAction::QueueTogglePolling(true));
                }
            }
        }
    });
}

/// 周期发送在 Task 8 实现，这里先留占位保持布局
fn show_placeholder(ui: &mut Ui) {
    ui.label(
        egui::RichText::new("周期发送：将在下一步实现")
            .color(theme::text_secondary())
            .italics(),
    );
}
```

同步修改 `egui-app/src/app.rs` 里 `show_sidebar` 的 `SideTab::Scheduled` 分支，
把调用改为三参数形式：

```rust
                SideTab::Scheduled => {
                    let polling = self
                        .refs
                        .backend
                        .queue_status()
                        .map(|s| s.is_polling)
                        .unwrap_or(false);
                    if let Some(action) = scheduled_panel::show(ui, &self.refs.shared, polling) {
                        self.handle(action);
                    }
                }
```

- [ ] **Step 6：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 39 passed; 0 failed`。

```bash
cd egui-app && cargo clippy --bin oms-native -- -D warnings
```

Expected: 无警告。

- [ ] **Step 7：实跑验证**

```bash
cd egui-app && cargo build --bin oms-native && cd .. && cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
```

```bash
sleep 8 && tasklist | grep -ci oms-native
```

肉眼确认：「定时发送」标签下能看到队列区块；添加 3 行、改优先级、切换 TXT/HEX、
改间隔 ms、删行都不 panic；「全部清空」生效。

```bash
taskkill //F //IM oms-native.exe
```

---

### Task 8：周期发送面板 + 二次确认门

**互斥约束**：core 的 `start_periodic_send` 内部会 `queue.clear()`，且与队列轮询共用
`port_handle`，两者互斥。且清队列是**破坏性**的——用户编辑的队列内容会永久丢失。

因此分两种处理：
- **破坏性**（启动周期发送）：二次确认，5 秒内再点一次才生效
- **非破坏性**（启动轮询）：直接置灰 + tooltip

**Files:**
- Modify: `egui-app/src/state.rs`（`UiState` 加确认门的查询/置位/取消方法 + 测试）
- Modify: `egui-app/src/ui/scheduled_panel.rs`

**Interfaces:**
- Consumes: Task 7 的 `scheduled_panel`、Task 6 的 `PanelAction::PeriodicToggle`、
  Task 7 已建立的 `UiState.periodic_confirm_until: Option<Instant>`
- Produces:
  - `state::CONFIRM_WINDOW: Duration`（5 秒）
  - `UiState::periodic_confirm_active(&self, now: Instant) -> bool`
  - `UiState::arm_periodic_confirm(&mut self, now: Instant)`
  - `UiState::disarm_periodic_confirm(&mut self)`
  - `UiState::periodic_confirm_remaining(&self, now: Instant) -> Duration`
  - `scheduled_panel::show_periodic(ui, shared, &mut action)`

- [ ] **Step 1：先写失败测试**

确认门的逻辑放在 `UiState` 上（而不是独立模块）——因为 Task 7 已经把
`periodic_confirm_until` 建在 `UiState` 里，拆到别处反而多一层间接。

在 `egui-app/src/state.rs` 已有的 `#[cfg(test)] mod tests` **末尾**追加：

```rust
    // ---- 周期发送的二次确认门 ----
    //
    // 场景：core 的 start_periodic_send 会调 queue.clear()，
    // 用户在队列里编辑的内容会被永久销毁。所以「启动周期发送」不能一点就生效。

    fn g_ui() -> UiState {
        UiState::default()
    }

    #[test]
    fn confirm_gate_fresh_is_inactive() {
        let s = g_ui();
        let now = std::time::Instant::now();
        assert!(!s.periodic_confirm_active(now));
        assert_eq!(s.periodic_confirm_remaining(now), CONFIRM_WINDOW);
    }

    #[test]
    fn confirm_gate_arm_activates() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        assert!(s.periodic_confirm_active(now));
    }

    #[test]
    fn confirm_gate_active_just_before_window() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        assert!(s.periodic_confirm_active(now + Duration::from_millis(4999)));
    }

    #[test]
    fn confirm_gate_expires_exactly_at_window() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        assert!(!s.periodic_confirm_active(now + CONFIRM_WINDOW));
    }

    #[test]
    fn confirm_gate_remaining_counts_down_never_negative() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        assert_eq!(s.periodic_confirm_remaining(now), CONFIRM_WINDOW);
        assert_eq!(
            s.periodic_confirm_remaining(now + Duration::from_secs(2)),
            Duration::from_secs(3)
        );
        assert_eq!(
            s.periodic_confirm_remaining(now + Duration::from_secs(99)),
            Duration::ZERO
        );
    }

    #[test]
    fn confirm_gate_disarm_clears() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        s.disarm_periodic_confirm();
        assert!(!s.periodic_confirm_active(now));
        assert!(!s.periodic_confirm_active(now + Duration::from_secs(10)));
    }

    #[test]
    fn confirm_gate_rearm_resets_clock() {
        let mut s = g_ui();
        let now = std::time::Instant::now();
        s.arm_periodic_confirm(now);
        s.arm_periodic_confirm(now + Duration::from_secs(3));
        assert!(s.periodic_confirm_active(now + Duration::from_secs(7)));
        assert!(!s.periodic_confirm_active(now + Duration::from_secs(8)));
    }
```

同时在 `egui-app/src/state.rs` 的测试模块顶部加两个 import（若尚未引入）：

```rust
use super::*;
use std::time::Duration;
```

- [ ] **Step 2：运行测试确认失败**

```bash
cd egui-app && cargo test --bin oms-native state::tests
```

Expected: **编译失败**，报 `cannot find method periodic_confirm_active` 等。

- [ ] **Step 3：实现确认门方法**

在 `egui-app/src/state.rs` 顶部加常量：

```rust
/// 周期发送「再点一次确认」的窗口时长
pub const CONFIRM_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);
```

在 `impl UiState` 里追加：

```rust
    /// 「启动周期发送」的确认窗口是否仍在有效期内
    pub fn periodic_confirm_active(&self, now: std::time::Instant) -> bool {
        self.periodic_confirm_until
            .is_some_and(|t| now.duration_since(t) < CONFIRM_WINDOW)
    }

    /// 进入待确认态（第一次点击，只 arm 不启动）
    pub fn arm_periodic_confirm(&mut self, now: std::time::Instant) {
        self.periodic_confirm_until = Some(now);
    }

    /// 取消待确认态
    pub fn disarm_periodic_confirm(&mut self) {
        self.periodic_confirm_until = None;
    }

    /// 剩余确认时间（已超时则为 0，不返回负值）
    pub fn periodic_confirm_remaining(&self, now: std::time::Instant) -> std::time::Duration {
        match self.periodic_confirm_until {
            Some(t) => CONFIRM_WINDOW.saturating_sub(now.duration_since(t)),
            None => CONFIRM_WINDOW,
        }
    }
```

- [ ] **Step 4：验证确认门逻辑**

```bash
cd egui-app && cargo test --bin oms-native state::tests
```

Expected: `16 passed; 0 failed`（Task 7 的 8 个 + 本任务 8 个）。

- [ ] **Step 5：实现周期发送面板**

把 `egui-app/src/ui/scheduled_panel.rs` 里的 `show_placeholder` 替换为下面的实现。
本文件已 `use super::PanelAction;`，本函数不需要新的 import：

```rust
/// 周期发送：单条 payload + 固定间隔。core 的 start_periodic_send 会清空队列，
/// 所以队列非空时必须二次确认。
fn show_periodic(ui: &mut Ui, shared: &SharedState, action: &mut Option<PanelAction>) {
    ui.label(egui::RichText::new("周期发送").strong());

    let now = std::time::Instant::now();
    let mut ui_state = shared.ui.lock();

    let running = ui_state.periodic_running;

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::SelectableLabel::new(
                    ui_state.periodic_is_hex,
                    egui::RichText::new(if ui_state.periodic_is_hex { "HEX" } else { "TXT" })
                        .small()
                        .monospace(),
                ),
            )
            .clicked()
        {
            ui_state.periodic_is_hex = !ui_state.periodic_is_hex;
        }
        ui.add(
            egui::TextEdit::singleline(&mut ui_state.periodic_content)
                .hint_text("周期发送的内容")
                .desired_width(f32::INFINITY),
        );
    });

    ui.horizontal(|ui| {
        ui.label("间隔");
        ui.add(DragValue::new(&mut ui_state.periodic_interval_ms)
            .speed(10)
            .clamp_range(10..=60_000))
            .on_hover_text("实际最小周期受波特率限制");
        ui.label("ms");
    });

    let queue_len = ui_state.queue.len();
    let confirming = ui_state.periodic_confirm_active(now);

    ui.horizontal(|ui| {
        if running {
            if ui.button("■ 停止").clicked() {
                ui_state.disarm_periodic_confirm();
                *action = Some(PanelAction::PeriodicToggle(false));
            }
            ui.colored_label(theme::tx_color(), "运行中…");
            return;
        }

        let text = if confirming {
            format!(
                "⚠ 再点一次确认清空队列（{} 条，{}s 内有效）",
                queue_len,
                ui_state.periodic_confirm_remaining(now).as_secs()
            )
        } else {
            "▶ 开始".to_string()
        };

        let btn = egui::Button::new(text).fill(if confirming {
            theme::warning()
        } else {
            theme::accent()
        });
        if ui.add(btn).clicked() {
            if confirming {
                ui_state.disarm_periodic_confirm();
                ui_state.queue.clear();
                *action = Some(PanelAction::QueueChanged);
                *action = Some(PanelAction::PeriodicToggle(true));
            } else if queue_len > 0 {
                // 队列非空：第一次点击只 arm，不启动
                ui_state.arm_periodic_confirm(now);
            } else {
                // 队列为空：直接启动
                *action = Some(PanelAction::PeriodicToggle(true));
            }
        }
    });

    if queue_len > 0 && !running && !confirming {
        ui.colored_label(
            theme::warning(),
            format!("⚠ 启动周期发送会清空当前队列（{} 条）", queue_len),
        );
    }
}
```

把 `show()` 里的 `show_placeholder(ui);` 改为 `show_periodic(ui, shared, &mut action);`，
并把 `show_periodic` 这个占位函数整个删掉。

- [ ] **Step 6：验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 47 passed; 0 failed`（39 + 8）。

```bash
cd egui-app && cargo clippy --bin oms-native -- -D warnings
```

Expected: 无警告。

- [ ] **Step 7：实跑验证确认门**

```bash
cd egui-app && cargo build --bin oms-native && cd .. && cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
```

```bash
sleep 8 && tasklist | grep -ci oms-native
```

肉眼确认：
1. 队列为空 → 点「▶ 开始」**立即**启动周期发送（不需确认）
2. 队列有 3 条 → 点「▶ 开始」按钮变橙色「⚠ 再点一次确认清空队列（3 条，4s 内有效）」，**倒计时在走**
3. 等 5 秒不点 → 按钮自动还原成「▶ 开始」，下方警示行仍在
4. 再点一次 → 按钮回「▶ 开始」，队列被清空，周期发送启动
5. 周期发送运行时 → 队列区的「▶ 开始轮询」置灰

```bash
taskkill //F //IM oms-native.exe
```

---

### Task 9：端到端手工验证与收尾

本任务不改代码，只跑验证清单。任何一项不通过，回到对应任务修。

**Files:** 无

**Interfaces:**
- Consumes: Task 1–8 的全部产出
- Produces: 一份实测结论

- [ ] **Step 1：全量自动验证**

```bash
cd egui-app && cargo test --bin oms-native
```

Expected: `test result: ok. 47 passed; 0 failed`。

```bash
cd egui-app && cargo clippy --bin oms-native -- -D warnings
```

Expected: 退出码 0，无警告。

```bash
cd egui-app && cargo build --release --bin oms-native
```

Expected: 编译成功（顺带验证 `opt-level = "z"` 下无问题）。

- [ ] **Step 2：确认共享库未被动过**

```bash
cd /e/RoninCode/Zed/OhMySerialHelper && git status --short core/ src-tauri/ src/
```

Expected: **无输出**。本轮所有改动都应限制在 `egui-app/`（不在版本控制内）
与 `docs/plans/`（已提交的文档）。

- [ ] **Step 3：实机验证清单**

准备：CH340 串口 TX-RX 短接，`OH_MY_SERIAL_TEST_PORT` 指向对应 COM 号。
启动：

```bash
cd egui-app && cd .. && cmd //c "target\debug\oms-native.exe" > oms.log 2>&1 &
```

逐项核对：

| # | 操作 | 期望 |
|---|---|---|
| 1 | 打开串口后静置 30 秒，任务管理器看 CPU | 明显低于改造前（无数据时约 10Hz 而非 60Hz） |
| 2 | 从短接口发数据回来 | 终端实时滚动，不卡顿 |
| 3 | 工具栏切 GBK，发中文，接收端回中文 | 终端正确显示中文，非 `?` |
| 4 | 拔掉 TX/RX | 状态栏出现橙色「● 断开 · 重连 1/5 · Ns 后重试」，N 逐秒递减 |
| 5 | 插回 TX/RX | 绿色「● 已重连」，约 3 秒后消失，恢复「● COMx @ baud」 |
| 6 | 关掉 app 重开 | 波特率、编码、上次端口被恢复 |
| 7 | Tauri 版改字体→保存→关；再开 egui 动一下设置→关；看 config.json | `font_family` / `font_size` / `theme` 未被 egui 冲掉 |
| 8 | 队列加 3 条（高/中/低优先级各一）→ 开始轮询 | 实际发送顺序为 高→中→低，与列表显示顺序一致 |
| 9 | 轮询运行中编辑某条的 interval | 改动在下一次同步后生效，不崩溃 |
| 10 | 队列 3 条 → 点周期发送「▶ 开始」 | 橙色确认态 + 倒计时；队列未被清空 |
| 11 | 5 秒后再点一次确认 | 队列被清空，周期发送启动 |
| 12 | 周期发送中点「▶ 开始轮询」 | 按钮置灰，tooltip 说明原因 |
| 13 | 周期发送中拔线（触发写失败） | 「运行中…」消失（Task 6 Step 10 的事件纠正生效） |
| 14 | 三个标签页来回切 | 状态各自保持，无 panic |

```bash
taskkill //F //IM oms-native.exe
```

- [ ] **Step 4：记录结论**

把实测结果写进 spec 文档末尾（`docs/plans/2026-09-28-egui-native-ui-design.md`），
在「风险与已知约束」之前插入一节「## 实施结果」，逐条记录第 3 步 14 项的通过/未通过，
未通过的注明原因与后续处理。这个文件在版本控制内，是唯一需要提交的产物：

```bash
cd /e/RoninCode/Zed/OhMySerialHelper
git add docs/plans/2026-09-28-egui-native-ui-design.md
git commit -m "docs: 记录 egui 地基 + 定时发送面板的实机验证结果"
```

---

## 附：core 语义速查（实施时随时对照）

| 事实 | 出处 | 含义 |
|---|---|---|
| `next_command()` 不移除元素 | `core/src/sender/queue.rs:53` | poller 只反复发 `commands.first()`，其余命令永不发出（原表写「循环重发整个列表」是错的，已更正） |
| `add()` 按 `Reverse(priority)` 排序 | `core/src/sender/queue.rs:27` | 优先级数值大的先发 |
| `queue_status()` 只给 count + is_polling | `core/src/backend.rs:472` | UI 必须自己存条目列表 |
| `start_periodic_send` 先 `queue.clear()` | `core/src/backend.rs:594` | 与队列轮询互斥，且清队列是破坏性的 |
| poller 线程退出时 `q.stop_polling()` | `core/src/backend.rs:558` | `is_polling` 真值可信 |
| send-precise 写失败直接 `break`，不置 stop flag | `core/src/backend.rs:630` | 周期发送状态不可信，须靠事件纠正 |
| `config::load()` 在 reader 清理时被 core 自己调用 | `core/src/backend.rs:805` | 自动重连对 egui 是白拿的，宿主无需接线 |
| `AppConfig` 派生 `PartialEq` | `core/src/config.rs:13` | `merge_owned` 的变更检测可直接用 `==` |
| `ReconnectEvent` 无 `PartialEq` | `core/src/backend.rs:71` | `badge()` 只消费字段，不需要比较事件 |
