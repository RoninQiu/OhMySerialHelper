# v1.2.0 终端数据录制功能 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 OhMySerial 添加"保存终端显示数据"功能——用户抓取串口日志时可一键录制到本地 .txt 文件，便于离线分析。录制包含终端显示的全部内容（RX + TX + 系统消息），重连后继续写入同一文件。

**Architecture:** 纯文本行由前端 Terminal 组件生成并通过 IPC 推送给 Rust 端 `Recorder`（字符串 sink）。Rust 端用 `BufWriter` 异步写盘，不阻塞 reader 线程。设置面板（Modal）配置默认保存路径和"每次弹文件对话框" toggle。Tauri dialog plugin 提供文件选择对话框。

**Tech Stack:** Rust `tauri-plugin-dialog = "2"`, Tauri 2.x IPC, React 18, TypeScript, Tailwind, xterm.js, Zustand, Vitest

---

## 用户已确认决策

| # | 决策 | 选项 |
|---|------|------|
| Q1 | 录制内容 | 终端渲染文本（HEX/TEXT 视图一致） |
| Q2 | 文件格式 | 纯文本行（去 ANSI） |
| Q3 | 触发 | 工具栏"⏺ 录制"按钮 |
| Q4 | 文件分卷 | 单文件不切，重连继续录制 |
| Q5 | 设置界面 | 新建 Modal 弹窗（HotkeyHelp 模式） |
| Q6 | 实现位置 | Rust 后端异步写盘 + IPC 控制 |
| Q7 | TX 录制 | 录（与终端 TX 行同步） |
| Q8 | 系统消息录制 | 录（设备断开/重连/系统提示） |
| Q9 | 重连 gap 注释 | 写一行 `#` 注释（含时长） |
| Q10 | 设置界面形态 | Modal 弹窗（`fixed inset-0 z-50`） |
| Q11 | ANSI 处理 | 写纯文本（去 ANSI 序列） |
| Q12 | 文件对话框触发 | 设置 toggle（每次弹 / 用默认） |
| Q13 | closePort 时 Recorder | 自动 stop + 提示用户 |
| Q14 | 文件头元数据 | 写一行 `# OhMySerial Capture — ...` |
| Q15 | 大文件警告 | > 500MB 时状态栏橙色警告 |
| Q16 | 磁盘写满处理 | 自动 stop + 推送 `recorder-error` 事件 |

---

## 文件格式规范

### 14.1 普通行（与终端显示一致，去 ANSI）

```
[14:35:12.456] ← [HEX] AA BB CC DD EE FF 11 22
[14:35:12.520] → [HEX] 01 02 03
[14:35:13.001] ← hello world
[14:35:13.500] [系统] 设备已断开: Broken pipe
```

**注意**：
- TEXT 视图时省略 `[HEX]` / `[TEXT]` 标记（终端本身不显示）
- HEX 视图时加 `[HEX]` 前缀便于 grep
- 文本视图原始字节直接写

### 14.2 注释行（重连 gap）

```
# [2026-06-22 14:35:12.456] 设备已断开: BrokenPipe
# [2026-06-22 14:35:47.123] 重连成功 (gap 34.667s)
```

### 14.3 文件头

```
# OhMySerial Capture — COM3 @ 115200 8N1 — 2026-06-22 14:35:12 (v1.2.0)
```

---

## 关键模块改造清单

| 模块 | 改动 | 文件 |
|------|------|------|
| 新增 | Rust Recorder 模块 | `src-tauri/src/recorder/mod.rs` |
| 扩展 | 5 个 IPC 命令 | `src-tauri/src/ipc/commands.rs` |
| 扩展 | SerialState 加 recorder 字段 | `src-tauri/src/ipc/commands.rs` |
| 扩展 | run_reader_loop 调 recorder.mark_event | `src-tauri/src/ipc/commands.rs` |
| 扩展 | schedule_reconnect 调 recorder.mark_event | `src-tauri/src/ipc/commands.rs` |
| 扩展 | AppConfig 加 2 字段 + CONFIG_VERSION → 2 | `src-tauri/src/config_impl.rs` |
| 新增 | tauri-plugin-dialog 依赖 | `src-tauri/Cargo.toml` |
| 扩展 | plugin 注册 + IPC handler | `src-tauri/src/lib.rs` |
| 扩展 | capabilities 加 dialog 权限 | `src-tauri/capabilities/default.json` |
| 新增 | recorderStore | `src/stores/recorderStore.ts` |
| 新增 | terminalFormat 工具 | `src/utils/terminalFormat.ts` |
| 扩展 | Terminal writeData 分支 invoke | `src/components/Terminal.tsx` |
| 扩展 | App 系统消息录制 | `src/App.tsx` |
| 扩展 | SerialToolbar 录制按钮 + ⚙ 按钮 | `src/components/SerialToolbar.tsx` |
| 扩展 | StatusBar REC 指示 | `src/components/StatusBar.tsx` |
| 新增 | SettingsPanel Modal | `src/components/SettingsPanel.tsx` |
| 扩展 | App 渲染 SettingsPanel | `src/App.tsx` |
| 扩展 | configStore 加 2 字段 | `src/stores/configStore.ts` |
| 扩展 | useConfigSync 加 recorderStore 订阅 | `src/hooks/useConfigSync.ts` |
| 新增 | @tauri-apps/plugin-dialog 依赖 | `package.json` |

---

## Commit 计划（4 个 TDD commit）

### Commit 1: `test(recorder): v1.2.0 录制功能单元测试套件`

**13 个测试全红（实现还没写）**：

- `src-tauri/src/recorder/mod.rs` 占位空模块
- `src-tauri/tests/recorder_integration.rs` 占位
- `src/stores/recorderStore.ts` 占位
- `src/utils/terminalFormat.ts` 占位
- `src/components/SettingsPanel.tsx` 占位
- `src/components/Terminal.tsx` / `src/App.tsx` / `src/components/SerialToolbar.tsx` / `src/components/StatusBar.tsx` 不动

#### Rust 单元测试（5 个，`src-tauri/src/recorder/mod.rs`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp_path(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("oh-my-serial-rec-{}-{}.txt", std::process::id(), name));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn start_recording_creates_empty_file() {
        let p = tmp_path("start");
        let rec = start_recording(p.clone()).unwrap();
        assert!(p.exists());
        assert_eq!(rec.bytes_written(), 0);
        rec.stop().unwrap();
    }

    #[test]
    fn write_line_accumulates_bytes() {
        let p = tmp_path("write");
        let mut rec = start_recording(p.clone()).unwrap();
        rec.write_line("[14:35:12.456] ← AA BB CC").unwrap();
        rec.write_line("[14:35:12.520] → 01 02").unwrap();
        assert!(rec.bytes_written() > 30);  // 至少两行长度
        rec.stop().unwrap();
        let mut content = String::new();
        std::fs::File::open(&p).unwrap().read_to_string(&mut content).unwrap();
        assert!(content.contains("← AA BB CC"));
        assert!(content.contains("→ 01 02"));
    }

    #[test]
    fn mark_event_adds_hash_prefix() {
        let p = tmp_path("event");
        let mut rec = start_recording(p.clone()).unwrap();
        rec.mark_event("设备已断开: BrokenPipe").unwrap();
        rec.stop().unwrap();
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.starts_with("# "));
        assert!(content.contains("设备已断开"));
    }

    #[test]
    fn stop_returns_correct_summary() {
        let p = tmp_path("stop");
        let mut rec = start_recording(p.clone()).unwrap();
        rec.write_line("hello").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let summary = rec.stop().unwrap();
        assert_eq!(summary.path, p);
        assert!(summary.bytes_written > 0);
        assert!(summary.duration_ms >= 10);
    }

    #[test]
    fn large_write_thousands_of_lines() {
        let p = tmp_path("large");
        let mut rec = start_recording(p.clone()).unwrap();
        for i in 0..10000 {
            rec.write_line(&format!("line {}", i)).unwrap();
        }
        let summary = rec.stop().unwrap();
        assert!(summary.bytes_written > 50000);
        let content = std::fs::read_to_string(&p).unwrap();
        assert_eq!(content.lines().count(), 10000);
    }
}
```

#### Rust 集成测试（3 个，`src-tauri/tests/recorder_integration.rs`）

```rust
// 注：实际测试用 mock 或实际 SerialState，由编写时根据 test harness 调整
#[test]
fn disconnect_then_reconnect_keeps_recording() { /* ... */ }

#[test]
fn close_port_stops_recording_automatically() { /* ... */ }

#[test]
fn concurrent_write_lines_no_data_loss() { /* ... */ }
```

#### 前端单元测试（5 个）

- `tests/frontend/recorderStore.test.ts`（2 测试）：start 设置 isRecording + stop 清空
- `tests/frontend/components/TerminalRecorder.test.tsx`（2 测试）：录制时 invoke / 非录制不 invoke
- `tests/frontend/components/SettingsPanel.test.tsx`（1 测试）：保存按钮调 configStore.save

**验证**：
```bash
cd src-tauri && cargo test recorder  # 5 个红
npm test -- recorderStore TerminalRecorder SettingsPanel  # 3 个红
```

---

### Commit 2: `feat(recorder): Rust Recorder 模块 + IPC 命令 + 重连透传`

**8 个 Rust 测试全绿**：

#### `src-tauri/src/recorder/mod.rs`（完整实现）

```rust
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

const BUF_CAPACITY: usize = 8 * 1024;

pub struct Recorder {
    writer: BufWriter<File>,
    bytes_written: u64,
    path: PathBuf,
    started_at: Instant,
}

#[derive(Debug)]
pub struct RecorderSummary {
    pub path: PathBuf,
    pub bytes_written: u64,
    pub duration_ms: u64,
}

pub fn start_recording(path: PathBuf) -> std::io::Result<Recorder> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    Ok(Recorder {
        writer: BufWriter::with_capacity(BUF_CAPACITY, file),
        bytes_written: 0,
        path,
        started_at: Instant::now(),
    })
}

impl Recorder {
    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.bytes_written += line.len() as u64 + 1;
        Ok(())
    }

    pub fn mark_event(&mut self, text: &str) -> std::io::Result<()> {
        let prefixed = format!("# {}", text);
        self.write_line(&prefixed)
    }

    pub fn write_header(&mut self, port: &str, baud: u32, db: u8, sb: u8, parity: &str) -> std::io::Result<()> {
        use std::time::SystemTime;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let line = format!(
            "# OhMySerial Capture — {} @ {} {} {} {} — {} (v1.2.0)",
            port, baud, db, sb, parity, now
        );
        self.write_line(&line)
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn stop(mut self) -> std::io::Result<RecorderSummary> {
        self.writer.flush()?;
        let duration_ms = self.started_at.elapsed().as_millis() as u64;
        Ok(RecorderSummary {
            path: self.path,
            bytes_written: self.bytes_written,
            duration_ms,
        })
    }
}
```

> **注意**：`chrono` 需要在 `Cargo.toml` 加依赖。如果不想引 chrono，用 `std::time::SystemTime` + 手算也行。

#### `src-tauri/src/ipc/commands.rs` 扩展

```rust
// SerialState 加字段
pub struct SerialState {
    // ... 现有字段
    pub recorder: Arc<Mutex<Option<crate::recorder::Recorder>>>,
}

// 5 个 IPC
#[tauri::command]
pub fn cmd_start_recording(path: String, state: State<'_, SerialState>) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if rec_guard.is_some() {
        return Err("已在录制中".to_string());
    }
    let mut rec = crate::recorder::start_recording(std::path::PathBuf::from(path))
        .map_err(|e| format!("创建录制文件失败: {e}"))?;
    // 写文件头
    rec.write_header(&state.last_port_name(), state.last_baud_rate(), /* ... */)
        .map_err(|e| format!("写文件头失败: {e}"))?;
    *rec_guard = Some(rec);
    Ok(())
}

#[tauri::command]
pub fn cmd_stop_recording(state: State<'_, SerialState>) -> Result<crate::recorder::RecorderSummary, String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    let rec = rec_guard.take().ok_or("未在录制")?;
    rec.stop().map_err(|e| format!("停止录制失败: {e}"))
}

#[tauri::command]
pub fn cmd_write_recorder_line(line: String, state: State<'_, SerialState>) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if let Some(rec) = rec_guard.as_mut() {
        rec.write_line(&line).map_err(|e| format!("写入失败: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_mark_recorder_event(text: String, state: State<'_, SerialState>) -> Result<(), String> {
    let mut rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    if let Some(rec) = rec_guard.as_mut() {
        rec.mark_event(&text).map_err(|e| format!("写入失败: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_is_recording(state: State<'_, SerialState>) -> Result<bool, String> {
    let rec_guard = state.recorder.lock().map_err(|e| e.to_string())?;
    Ok(rec_guard.is_some())
}

// SerialState Lite 也要带上 recorder 引用（用于 schedule_reconnect）
struct SerialStateLite {
    // ... 现有字段
    recorder: Arc<Mutex<Option<crate::recorder::Recorder>>>,
}
```

#### `run_reader_loop` 改造

在第 211 行（断线 log 处）加：
```rust
if let Ok(mut rg) = recorder.lock() {
    if let Some(rec) = rg.as_mut() {
        let _ = rec.mark_event(&format!("设备已断开: {}", e));
    }
}
```

#### `schedule_reconnect` 改造

成功回调处加：
```rust
if let Ok(mut rg) = recorder.lock() {
    if let Some(rec) = rg.as_mut() {
        let _ = rec.mark_event(&format!("重连成功 (gap {:.3}s)", gap_seconds));
    }
}
```

#### `cmd_close_port` 改造

```rust
#[tauri::command]
pub fn cmd_close_port(state: State<'_, SerialState>) -> Result<(), String> {
    // ... 现有逻辑
    // 新增：自动 stop 录制
    if let Ok(mut rec_guard) = state.recorder.lock() {
        if let Some(rec) = rec_guard.take() {
            let _ = rec.stop();  // 忽略错误，文件已 flush
            log::info!("[recorder] 串口关闭，自动停止录制");
        }
    }
    Ok(())
}
```

#### `src-tauri/src/config_impl.rs` 扩展

```rust
pub const CONFIG_VERSION: u32 = 2;

pub struct AppConfig {
    // ... 现有字段
    pub default_capture_path: String,
    pub prompt_save_dialog: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // ... 现有默认值
            default_capture_path: String::new(),
            prompt_save_dialog: true,
        }
    }
}

// migrate 函数（旧 v1 配置升级到 v2）
pub fn migrate_v1_to_v2(mut cfg: serde_json::Value) -> serde_json::Value {
    if !cfg.get("default_capture_path").is_some() {
        cfg["default_capture_path"] = serde_json::Value::String(String::new());
    }
    if !cfg.get("prompt_save_dialog").is_some() {
        cfg["prompt_save_dialog"] = serde_json::Value::Bool(true);
    }
    cfg
}
```

#### `src-tauri/Cargo.toml`

```toml
[dependencies]
tauri-plugin-dialog = "2"
```

#### `src-tauri/src/lib.rs`

```rust
.plugin(tauri_plugin_dialog::init())

.invoke_handler(tauri::generate_handler![
    // ... 现有
    ipc::commands::cmd_start_recording,
    ipc::commands::cmd_stop_recording,
    ipc::commands::cmd_write_recorder_line,
    ipc::commands::cmd_mark_recorder_event,
    ipc::commands::cmd_is_recording,
])
```

#### `src-tauri/capabilities/default.json`

```json
{
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "dialog:default",
    "dialog:allow-save",
    "dialog:allow-open"
  ]
}
```

**验证**：
```bash
cd src-tauri && cargo test recorder  # 8 个全绿
cd src-tauri && cargo build  # 无 warning
```

---

### Commit 3: `feat(recorder-ui): 工具栏按钮 + 状态栏 + Terminal 接线 + 系统消息`

**2 个前端测试绿**：

#### `src/stores/recorderStore.ts`

```ts
import { create } from "zustand";

export interface RecorderSummary {
  path: string;
  bytes_written: number;
  duration_ms: number;
}

interface RecorderState {
  isRecording: boolean;
  currentPath: string | null;
  bytesWritten: number;
  startedAt: number | null;
  startRecording: (path: string) => Promise<void>;
  stopRecording: () => Promise<RecorderSummary | null>;
  setBytesWritten: (n: number) => void;
}

export const useRecorderStore = create<RecorderState>((set, get) => ({
  isRecording: false,
  currentPath: null,
  bytesWritten: 0,
  startedAt: null,

  startRecording: async (path) => {
    if (get().isRecording) throw new Error("已在录制中");
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("cmd_start_recording", { path });
    set({ isRecording: true, currentPath: path, bytesWritten: 0, startedAt: Date.now() });
  },

  stopRecording: async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      const summary = await invoke<RecorderSummary>("cmd_stop_recording");
      set({ isRecording: false, currentPath: null, bytesWritten: 0, startedAt: null });
      return summary;
    } catch (e) {
      // 未在录制
      set({ isRecording: false, currentPath: null, bytesWritten: 0, startedAt: null });
      return null;
    }
  },

  setBytesWritten: (n) => set({ bytesWritten: n }),
}));
```

#### `src/utils/terminalFormat.ts`

```ts
import { decodeGBK } from "./encoding";
import { formatTimestamp, byteHex } from "../components/Terminal";

export type ViewMode = "text" | "hex";
export type Encoding = "utf8" | "gbk";
export type Direction = "rx" | "tx";

/** 生成去 ANSI 的纯文本行（不含 \n），由前端 Terminal + App 系统消息复用 */
export function formatLine(
  data: Uint8Array,
  direction: Direction,
  viewMode: ViewMode,
  encoding: Encoding,
  ts: Date = new Date(),
): string {
  const tsStr = formatTimestamp(ts);
  const arrow = direction === "rx" ? "←" : "→";

  if (viewMode === "hex") {
    const hex = Array.from(data, byteHex).join(" ");
    return `[${tsStr}] ${arrow} [HEX] ${hex}`;
  }

  // 文本视图
  const text = encoding === "gbk" ? decodeGBK(data) : new TextDecoder("utf-8").decode(data);
  return `[${tsStr}] ${arrow} ${text}`;
}
```

> **注意**：`formatTimestamp` / `byteHex` / `decodeGBK` 当前在 `Terminal.tsx` / `encoding.ts`。需要从 Terminal 移到 utils/terminalFormat.ts 并 re-export。

#### `src/components/Terminal.tsx` writeData 改造

```tsx
const writeData = useCallback(
  (data: Uint8Array, direction: Direction = "rx") => {
    const xterm = xtermRef.current;
    if (!xterm || data.length === 0) return;

    // 现有逻辑（写终端，保留 ANSI）
    const ts = formatTimestamp(new Date());
    // ... 现有代码

    // 新增：若在录制，同步推一行纯文本给 Rust Recorder
    if (useRecorderStore.getState().isRecording) {
      const line = formatLine(data, direction, viewMode, encoding);
      void import("@tauri-apps/api/core").then(({ invoke }) =>
        invoke("cmd_write_recorder_line", { line }).catch((e) => console.warn("recorder write failed:", e))
      );
    }
  },
  [viewMode, encoding],
);
```

#### `src/App.tsx` 系统消息录制

```tsx
// 在 listen("port-disconnected") 处加：
const unDisc = await listen<string>("port-disconnected", (event) => {
  useSerialStore.getState().setDisconnected(true);
  useSerialStore.setState({ isOpen: false });
  const msg = `\r\n[系统] 设备已断开: ${event.payload}\r\n`;
  terminalRef.current?.writeData(new TextEncoder().encode(msg));

  // 新增：录制系统消息
  if (useRecorderStore.getState().isRecording) {
    const line = `[${formatTimestamp(new Date())}] [系统] 设备已断开: ${event.payload}`;
    void import("@tauri-apps/api/core").then(({ invoke }) =>
      invoke("cmd_mark_recorder_event", { text: line })
    );
  }
});

// reconnect-status 同理
```

#### `src/components/SerialToolbar.tsx` 加按钮

```tsx
const isRecording = useRecorderStore((s) => s.isRecording);
const startRecording = useRecorderStore((s) => s.startRecording);
const stopRecording = useRecorderStore((s) => s.stopRecording);

const handleToggleRecording = async () => {
  try {
    if (isRecording) {
      const summary = await stopRecording();
      if (summary) {
        alert(`录制完成: ${(summary.bytes_written / 1024).toFixed(1)} KB`);
      }
    } else {
      // 弹文件对话框 or 用默认路径
      const cfg = useConfigStore.getState().config;
      let path: string;
      if (cfg.prompt_save_dialog || !cfg.default_capture_path) {
        const { save } = await import("@tauri-apps/plugin-dialog");
        path = await save({
          defaultPath: cfg.default_capture_path,
          filters: [{ name: "Text", extensions: ["txt"] }],
        });
        if (!path) return;  // 用户取消
      } else {
        path = `${cfg.default_capture_path}/capture-${Date.now()}.txt`;
      }
      await startRecording(path);
    }
  } catch (e) {
    console.error("toggle recording failed:", e);
  }
};

// 渲染
<button
  onClick={handleToggleRecording}
  className={isRecording ? "bg-red-500 text-white" : "bg-gray-200 dark:bg-gray-700"}
>
  {isRecording ? "⏹ 停止录制" : "⏺ 录制"}
</button>
```

#### `src/components/StatusBar.tsx` 加 REC 指示

```tsx
const isRecording = useRecorderStore((s) => s.isRecording);
const bytesWritten = useRecorderStore((s) => s.bytesWritten);

// 用 useRafValue 节流到 15Hz
const displayedBytes = useRafValue(bytesWritten);

// 渲染（TX/RX 行后）
{isRecording && (
  <span className="flex items-center gap-1 text-red-500" data-rec-indicator>
    <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
    REC {(displayedBytes / 1024).toFixed(1)} KB
  </span>
)}
{isRecording && displayedBytes > 500 * 1024 && (
  <span className="text-orange-500" title="录制文件超过 500MB">⚠</span>
)}
```

#### `package.json` 加依赖

```bash
npm install @tauri-apps/plugin-dialog
```

**验证**：
```bash
npm test -- recorderStore TerminalRecorder  # 4 个绿
npm run tsc  # 无 type 错误
```

---

### Commit 4: `feat(settings): SettingsPanel Modal + 默认路径/弹对话框 toggle`

**3 个前端测试绿**：

#### `src/stores/configStore.ts` 加 2 字段

```ts
export interface AppConfigFE {
  // ... 现有
  default_capture_path: string;
  prompt_save_dialog: boolean;
}

export const DEFAULT_CONFIG: AppConfigFE = {
  // ... 现有
  default_capture_path: "",
  prompt_save_dialog: true,
};

// loadFromBackend / save 自动通过 serialize 处理
```

#### `src/components/SettingsPanel.tsx`

照搬 HotkeyHelp 的 Modal 模式：

```tsx
import { useState } from "react";
import { useConfigStore } from "../stores/configStore";
import { useThemeClasses } from "../hooks/useThemeClasses";

export interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsPanel({ open, onClose }: SettingsPanelProps) {
  const t = useThemeClasses();
  const config = useConfigStore((s) => s.config);
  const setDefaultPath = (path: string) => useConfigStore.setState((s) => ({ config: { ...s.config, default_capture_path: path } }));
  const setPromptDialog = (v: boolean) => useConfigStore.setState((s) => ({ config: { ...s.config, prompt_save_dialog: v } }));
  const save = useConfigStore((s) => s.save);

  if (!open) return null;

  const handleBrowse = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({ directory: true, multiple: false, title: "选择默认保存目录" });
    if (path && typeof path === "string") setDefaultPath(path);
  };

  const handleSave = async () => {
    await save();
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
         onClick={onClose}>
      <div className={`rounded-lg shadow-2xl p-6 w-[480px] max-w-[90vw] ${t.bg.primary} border ${t.border.default}`}
           onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-semibold mb-4">⚙ 设置</h2>

        <div className="space-y-4">
          {/* 默认保存路径 */}
          <div>
            <label className="block text-sm font-medium mb-1">默认保存路径</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={config.default_capture_path}
                onChange={(e) => setDefaultPath(e.target.value)}
                placeholder="留空则每次录制时弹对话框"
                className={`flex-1 px-2 py-1 text-sm ${t.bg.tertiary} ${t.text.primary} rounded border ${t.border.input}`}
              />
              <button
                onClick={handleBrowse}
                className={`px-3 py-1 text-sm ${t.bg.tertiary} hover:opacity-80 rounded`}
              >
                浏览...
              </button>
            </div>
          </div>

          {/* 每次录制时弹文件对话框 */}
          <div>
            <label className="flex items-center gap-2 text-sm cursor-pointer">
              <input
                type="checkbox"
                checked={config.prompt_save_dialog}
                onChange={(e) => setPromptDialog(e.target.checked)}
              />
              每次录制时弹文件对话框
            </label>
            <p className={`text-xs ${t.text.muted} mt-1 ml-6`}>
              关闭时直接保存到上方默认路径
            </p>
          </div>
        </div>

        <div className="flex justify-end gap-2 mt-6">
          <button
            onClick={onClose}
            className={`px-4 py-2 text-sm ${t.bg.tertiary} hover:opacity-80 rounded`}
          >
            取消
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm bg-blue-500 hover:bg-blue-600 text-white rounded"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
```

#### `src/components/SerialToolbar.tsx` 加 ⚙ 按钮 + App.tsx 渲染 SettingsPanel

```tsx
// SerialToolbar.tsx
const [showSettings, setShowSettings] = useState(false);

<button
  onClick={() => setShowSettings(true)}
  className={`px-3 py-1 text-sm ${t.bg.tertiary} hover:opacity-80 rounded`}
  title="设置"
>
  ⚙
</button>

// App.tsx
const [showSettings, setShowSettings] = useState(false);
{showSettings && <SettingsPanel open={showSettings} onClose={() => setShowSettings(false)} />}
```

#### `src/hooks/useConfigSync.ts` 加 recorderStore 订阅

不需要新增，recorderStore 是 session-only 状态（不持久化），但 configStore 字段需要 subscribe 才能持久化。已有机制应该已覆盖，加验证测试。

**验证**：
```bash
npm test -- SettingsPanel  # 3 个绿
npm run tsc  # 无 type 错误
cd src-tauri && cargo test  # 全 51 个绿
```

---

## 端到端验证清单

完成后人工验证：

- [ ] 打开串口 → 点"⏺ 录制" → 弹文件对话框 → 选路径 → 按钮变"⏹ 停止录制"
- [ ] 状态栏出现红色 REC 指示 + 实时字节数
- [ ] RX 字节流 + TX 发送 都写入文件
- [ ] 关闭串口 → 自动停止录制 + 提示 summary
- [ ] 录制中设备断开 → 文件出现 `# [ts] 设备已断开` 注释
- [ ] 自动重连成功 → 文件出现 `# [ts] 重连成功 (gap Xs)` 注释
- [ ] 重连后 RX/TX 继续写入同一文件
- [ ] 打开 Settings → 默认路径可填 + 浏览按钮可点 + toggle 切换
- [ ] 保存 → 重启 app → 默认路径持久化
- [ ] toggle 关闭时直接保存到默认路径（不弹对话框）
- [ ] 大文件 > 500MB 时状态栏橙色警告
- [ ] 磁盘写满 → 自动停止 + 提示

---

## 文档同步

实施完成后：
- `CLAUDE.md` 加 v1.2.0 增量（4 commit 总结 + 决策点）
- `docs/releases/1.2.0.md` 写 release notes
- 走 `/release` skill 发版
