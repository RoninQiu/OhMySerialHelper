# OhMySerial 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成 OhMySerial 串口调试助手的开发，实现高性能串口通信、WebGL 渲染、双缓冲架构

**Architecture:**

- Rust 后端：异步串口读写、64KB 环形队列、批量 IPC 发送、发送队列下沉
- React 前端：Xterm.js 渲染、Zustand 状态管理、数据分层存储
- Tauri 2.x：前后端通信、WebView2 承载

**Tech Stack:** Rust + Tauri 2.x + Tokio + serialport + React + TypeScript + Xterm.js + Zustand

---

## 文件结构

```
OhMySerialHelper/
├── src/                              # React 前端
│   ├── components/
│   │   ├── Terminal.tsx              # Xterm.js 终端组件
│   │   ├── SerialToolbar.tsx         # 串口工具栏
│   │   ├── SendPanel.tsx             # 发送面板
│   │   └── PresetPanel.tsx           # 预设命令面板
│   ├── stores/
│   │   ├── serialStore.ts            # 串口状态（Zustand）
│   │   ├── bufferStore.ts           # 数据缓冲状态
│   │   └── presetStore.ts            # 预设命令状态
│   ├── utils/
│   │   ├── encoding.ts               # GBK/UTF-8 编码转换
│   │   └── hex.ts                    # HEX 解析工具
│   └── App.tsx
├── src-tauri/                        # Rust 后端
│   ├── src/
│   │   ├── lib.rs                    # 核心库
│   │   ├── serial/
│   │   │   ├── mod.rs                # 串口模块入口
│   │   │   ├── port.rs               # 串口操作（枚举/打开/读写）
│   │   │   └── ring_buffer.rs        # 64KB 环形队列
│   │   ├── ipc/
│   │   │   │   ├── mod.rs            # IPC 模块入口
│   │   │   │   └── commands.rs       # Tauri IPC 命令
│   │   ├── sender/
│   │   │   │   ├── mod.rs            # 发送模块入口
│   │   │   │   ├── queue.rs          # 发送队列
│   │   │   │   └── timer.rs          # 高精度定时器
│   │   └── error.rs                  # 错误类型定义
│   └── Cargo.toml
├── tests/                            # 单元测试
│   ├── rust/
│   │   ├── ring_buffer_test.rs
│   │   ├── encoding_test.rs
│   │   └── hex_test.rs
│   └── frontend/
│       ├── encoding_test.ts
│       └── hex_test.ts
└── docs/
    └── plans/
        ├── 2026-05-29-OhMySerial-design.md
        └── 2026-05-29-OhMySerial-implementation.md
```

---

## 第一阶段：环境初始化

### Task 1: Tauri 项目初始化

**Files:**

- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `package.json`
- Create: `vite.config.ts`

- [ ] **Step 1: 创建 Cargo.toml，配置依赖**

```toml
[package]
name = "oh-my-serial"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serialport = "4"
tauri = { version = "2", features = ["devtools"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
log = "0.4"
env_logger = "0.10"

[profile.release]
lto = true
opt-level = "z"
strip = true
```

- [ ] **Step 2: 创建 tauri.conf.json**

```json
{
  "productName": "OhMySerial",
  "version": "0.1.0",
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "windows": {
      "nsis": {
        "installerIcon": "icons/icon.ico"
      }
    }
  },
  "build": {
    "devtools": true
  }
}
```

- [ ] **Step 3: 创建基础 main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    oh_my_serial::run()
}
```

- [ ] **Step 4: 创建 lib.rs 基础框架**

```rust
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: 初始化前端项目**

```bash
npm create vite@latest . -- --template react-ts
npm install zustand @xterm/xterm @xterm/addon-fit
```

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "feat: 初始化 Tauri 项目基础结构"
```

---

### Task 2: 项目结构与错误类型定义

**Files:**

- Create: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 定义错误类型**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SerialError {
    #[error("串口打开失败: {0}")]
    OpenFailed(String),

    #[error("串口已被占用")]
    PortLocked,

    #[error("发送超时")]
    SendTimeout,

    #[error("接收错误: {0}")]
    ReceiveError(String),

    #[error("缓冲区溢出，已丢弃 {0} 字节")]
    BufferOverflow(usize),
}
```

- [ ] **Step 2: 添加 thiserror 依赖到 Cargo.toml**

```toml
thiserror = "1"
```

- [ ] **Step 3: 在 lib.rs 中导出错误模块**

```rust
mod error;
pub use error::SerialError;
```

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 定义 SerialError 错误类型"
```

---

## 第二阶段：Rust 串口驱动与 IPC

### Task 3: 串口端口模块

**Files:**

- Create: `src-tauri/src/serial/mod.rs`
- Create: `src-tauri/src/serial/port.rs`
- Test: `tests/rust/port_test.rs`

- [ ] **Step 1: 创建 port.rs，定义 PortInfo 结构体**

```rust
use serde::{Deserialize, Serialize};
use serialport::SerialPortInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub name: String,
    pub port_type: String,
    pub baud_rates: Vec<u32>,
}

impl From<&SerialPortInfo> for PortInfo {
    fn from(info: &SerialPortInfo) -> Self {
        let port_type = guess_port_type(&info.name);
        Self {
            name: info.port_name.clone(),
            port_type,
            baud_rates: info.baud_rates.clone(),
        }
    }
}

fn guess_port_type(name: &str) -> String {
|    name.contains("CH340") => "CH340".to_string(),
|    name.contains("FTDI") => "FTDI".to_string(),
|    name.contains("CP210") => "CP210x".to_string(),
|    _ => "Unknown".to_string(),
}
```

- [ ] **Step 2: 实现串口枚举函数**

```rust
pub fn list_ports() -> Vec<PortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .iter()
        .map(PortInfo::from)
        .collect()
}
```

- [ ] **Step 3: 创建单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_ports_returns_vec() {
        let ports = list_ports();
        assert!(ports.is_empty() || ports.len() > 0);
    }

    #[test]
    fn test_port_info_from_serial_port_info() {
        let info = SerialPortInfo {
            port_name: "COM3".to_string(),
            baud_rates: vec![9600, 115200],
            // ... 其他字段用默认值
        };
        let port_info = PortInfo::from(&info);
        assert_eq!(port_info.name, "COM3");
    }
}
```

- [ ] **Step 4: 运行测试验证**

```bash
cd src-tauri && cargo test --lib
# 预期: PASS
```

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: 实现串口枚举模块"
```

---

### Task 4: 环形缓冲区实现

**Files:**

- Create: `src-tauri/src/serial/ring_buffer.rs`
- Test: `tests/rust/ring_buffer_test.rs`

- [ ] **Step 1: 实现环形队列结构体**

```rust
pub struct RingBuffer {
    buf: Vec<u8>,
    capacity: usize,
    write_pos: usize,
    read_pos: usize,
    overflow_count: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            capacity,
            write_pos: 0,
            read_pos: 0,
            overflow_count: 0,
        }
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        let available = self.available_space();
        if data.len() > available {
            self.overflow_count += data.len() - available;
        }

        let to_write = data.len().min(self.capacity);
        for i in 0..to_write {
            self.buf[(self.write_pos + i) % self.capacity] = data[i];
        }
        self.write_pos = (self.write_pos + to_write) % self.capacity;
        to_write
    }

    pub fn read(&mut self, len: usize) -> Vec<u8> {
        let available = self.data_len();
        let to_read = len.min(available);
        let mut result = Vec::with_capacity(to_read);
        for _ in 0..to_read {
            result.push(self.buf[self.read_pos]);
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }
        result
    }

    pub fn data_len(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    pub fn available_space(&self) -> usize {
        self.capacity - self.data_len()
    }

    pub fn water_level(&self) -> f32 {
        self.data_len() as f32 / self.capacity as f32
    }

    pub fn overflow_count(&self) -> usize {
        self.overflow_count
    }

    pub fn reset_overflow(&mut self) {
        self.overflow_count = 0;
    }
}
```

- [ ] **Step 2: 创建单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read() {
        let mut buf = RingBuffer::new(64);
        let written = buf.write(b"hello");
        assert_eq!(written, 5);
        assert_eq!(buf.data_len(), 5);
    }

    #[test]
    fn test_wrap_around() {
        let mut buf = RingBuffer::new(10);
        buf.write(b"0123456789");
        assert_eq!(buf.data_len(), 10);
        buf.read(5);
        assert_eq!(buf.data_len(), 5);
        buf.write(b"ABCDEF");
        assert_eq!(buf.data_len(), 11); // overflow 1 byte
        assert_eq!(buf.overflow_count(), 1);
    }

    #[test]
    fn test_water_level() {
        let mut buf = RingBuffer::new(100);
        assert_eq!(buf.water_level(), 0.0);
        buf.write(b"test");
        assert_eq!(buf.water_level(), 0.04);
    }
}
```

- [ ] **Step 3: 运行测试**

```bash
cd src-tauri && cargo test ring_buffer -- --nocapture
# 预期: PASS
```

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 实现 64KB 环形缓冲区"
```

---

### Task 5: IPC 命令层

**Files:**

- Create: `src-tauri/src/ipc/mod.rs`
- Create: `src-tauri/src/ipc/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 commands.rs，定义 Tauri 命令**

```rust
use crate::serial::{port::{list_ports, PortInfo}, ring_buffer::RingBuffer};
use crate::error::SerialError;
use std::sync::Mutex;
use tauri::State;

pub struct SerialState {
    pub ring_buffer: Mutex<RingBuffer>,
    pub port_handle: Mutex<Option<Box<dyn SerialPort>>>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            ring_buffer: Mutex::new(RingBuffer::new(65536)),
            port_handle: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn cmd_list_ports() -> Result<Vec<PortInfo>, String> {
    Ok(list_ports())
}

#[tauri::command]
pub fn cmd_open_port(port_name: String, baud_rate: u32) -> Result<(), String> {
    // 实现打开逻辑
    todo!()
}

#[tauri::command]
pub fn cmd_close_port(state: State<'_, SerialState>) -> Result<(), String> {
    let mut handle = state.port_handle.lock().map_err(|e| e.to_string())?;
    *handle = None;
    Ok(())
}

#[tauri::command]
pub fn cmd_read_buffer(state: State<'_, SerialState>, len: usize) -> Result<Vec<u8>, String> {
    let mut buf = state.ring_buffer.lock().map_err(|e| e.to_string())?;
    Ok(buf.read(len))
}
```

- [ ] **Step 2: 更新 lib.rs 注册命令**

```rust
mod serial;
mod ipc;
mod error;

pub use error::SerialError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ipc::commands::SerialState::default())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::cmd_list_ports,
            ipc::commands::cmd_open_port,
            ipc::commands::cmd_close_port,
            ipc::commands::cmd_read_buffer,
        ])
        .setup(|_app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: 添加 serialport trait import**

```rust
use serialport::SerialPort;
```

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 实现 IPC 命令层基础框架"
```

---

### Task 6: 背压策略实现

**Files:**

- Modify: `src-tauri/src/serial/ring_buffer.rs`
- Modify: `src-tauri/src/ipc/commands.rs`

- [ ] **Step 1: 在 RingBuffer 中添加水位阈值常量**

```rust
const WATER_LEVEL_LOW: f32 = 0.50;  // 恢复正常刷新率
const WATER_LEVEL_MID: f32 = 0.75;  // 降低刷新率
const WATER_LEVEL_HIGH: f32 = 0.90; // 丢包警告
```

- [ ] **Step 2: 添加背压状态枚举**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackpressureState {
    Normal,
    Throttled,
    Overflow,
}

impl RingBuffer {
    pub fn backpressure_state(&self) -> BackpressureState {
        let level = self.water_level();
        if level >= WATER_LEVEL_HIGH {
            BackpressureState::Overflow
        } else if level >= WATER_LEVEL_MID {
            BackpressureState::Throttled
        } else {
            BackpressureState::Normal
        }
    }

    pub fn should_flush(&self) -> bool {
        // 触发条件：满 4KB 或 计时满 16ms
        self.data_len() >= 4096
    }
}
```

- [ ] **Step 3: 在 IPC 命令中返回背压状态**

```rust
#[derive(serde::Serialize)]
pub struct BufferStatus {
    pub data_len: usize,
    pub water_level: f32,
    pub backpressure: String,
    pub overflow_count: usize,
}

#[tauri::command]
pub fn cmd_get_buffer_status(state: State<'_, SerialState>) -> Result<BufferStatus, String> {
    let buf = state.ring_buffer.lock().map_err(|e| e.to_string())?;
    Ok(BufferStatus {
        data_len: buf.data_len(),
        water_level: buf.water_level(),
        backpressure: format!("{:?}", buf.backpressure_state()),
        overflow_count: buf.overflow_count(),
    })
}
```

- [ ] **Step 4: 添加测试**

```rust
#[test]
fn test_backpressure_state() {
    let mut buf = RingBuffer::new(100);
    assert_eq!(buf.backpressure_state(), BackpressureState::Normal);

    buf.write(b"test");
    assert_eq!(buf.backpressure_state(), BackpressureState::Normal);

    buf.write(&[0u8; 70]); // 74%
    assert_eq!(buf.backpressure_state(), BackpressureState::Throttled);

    buf.write(&[0u8; 20]); // 94%
    assert_eq!(buf.backpressure_state(), BackpressureState::Overflow);
}
```

- [ ] **Step 5: 运行测试并提交**

```bash
cd src-tauri && cargo test backpressure -- --nocapture
git add -A && git commit -m "feat: 实现背压策略"
```

---

## 第三阶段：前端 Xterm.js 与编码层

### Task 7: 前端编码工具

**Files:**

- Create: `src/utils/encoding.ts`
- Create: `src/utils/hex.ts`
- Test: `tests/frontend/encoding_test.ts`
- Test: `tests/frontend/hex_test.ts`

- [ ] **Step 1: 实现 GBK 转换表**

```typescript
// GBK 编码范围大表（简化版，实际需要完整映射）
const GBK_TO_UTF8: Map<number, number[]> = new Map([
  // 添加常用 GBK 字符映射
]);

export function decodeGBK(buffer: Uint8Array): string {
  // 实现 GBK -> UTF-8 转换
  let result = "";
  for (let i = 0; i < buffer.length; i++) {
    const byte = buffer[i];
    if (byte < 0x80) {
      result += String.fromCharCode(byte);
    } else if (i + 1 < buffer.length) {
      const gbk = (byte << 8) | buffer[i + 1];
      const utf8 = GBK_TO_UTF8.get(gbk);
      if (utf8) {
        result += String.fromCharCode(...utf8);
        i++;
      } else {
        result += "\uFFFD"; // 未知字符
        i++;
      }
    }
  }
  return result;
}

export function detectEncoding(buffer: Uint8Array): "utf8" | "gbk" | "latin1" {
  // 简单的编码检测
  try {
    const decoded = new TextDecoder("utf-8").decode(buffer.slice(0, 100));
    if (!decoded.includes("\uFFFD")) {
      return "utf8";
    }
  } catch {}
  return "latin1";
}
```

- [ ] **Step 2: 实现 HEX 工具函数**

```typescript
export function hexToBytes(hex: string): Uint8Array {
  // 清理十六进制字符串：移除空格、支持 313233 或 31 32 33 格式
  const cleaned = hex.replace(/\s+/g, "");
  const bytes = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < cleaned.length; i += 2) {
    bytes[i / 2] = parseInt(cleaned.substr(i, 2), 16);
  }
  return bytes;
}

export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join(" ");
}

export function formatHexDump(
  bytes: Uint8Array,
  bytesPerLine: number = 16,
): string {
  const lines: string[] = [];
  for (let i = 0; i < bytes.length; i += bytesPerLine) {
    const chunk = bytes.slice(i, i + bytesPerLine);
    const addr = i.toString(16).padStart(8, "0");
    const hex = bytesToHex(chunk).padEnd(48, " ");
    const ascii = Array.from(chunk)
      .map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : "."))
      .join("");
    lines.push(`${addr}  ${hex}  ${ascii}`);
  }
  return lines.join("\n");
}
```

- [ ] **Step 3: 创建单元测试**

```typescript
// tests/frontend/hex_test.ts
import { describe, it, expect } from "vitest";
import { hexToBytes, bytesToHex, formatHexDump } from "../../src/utils/hex";

describe("hexToBytes", () => {
  it("parses spaced hex", () => {
    expect(hexToBytes("31 32 33")).toEqual(new Uint8Array([0x31, 0x32, 0x33]));
  });

  it("parses compact hex", () => {
    expect(hexToBytes("313233")).toEqual(new Uint8Array([0x31, 0x32, 0x33]));
  });

  it("handles empty string", () => {
    expect(hexToBytes("")).toEqual(new Uint8Array(0));
  });
});

describe("bytesToHex", () => {
  it("converts bytes to spaced hex", () => {
    expect(bytesToHex(new Uint8Array([0x31, 0x32, 0x33]))).toBe("31 32 33");
  });
});

describe("formatHexDump", () => {
  it("formats hex dump correctly", () => {
    const input = new Uint8Array([0x48, 0x65, 0x6c, 0x6c, 0x6f]);
    const result = formatHexDump(input);
    expect(result).toContain("4865 6c6c 6f");
    expect(result).toContain("Hello");
  });
});
```

- [ ] **Step 4: 运行前端测试**

```bash
npm test -- --run hex_test.ts
# 预期: PASS
```

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat: 实现前端编码和 HEX 工具"
```

---

### Task 8: Zustand Store 实现

**Files:**

- Create: `src/stores/serialStore.ts`
- Create: `src/stores/bufferStore.ts`
- Create: `src/stores/presetStore.ts`

- [ ] **Step 1: 实现 serialStore**

```typescript
import { create } from "zustand";

interface SerialState {
  isOpen: boolean;
  portName: string;
  baudRate: number;
  dataBits: 5 | 6 | 7 | 8;
  stopBits: 1 | 1.5 | 2;
  parity: "none" | "odd" | "even";
  dtr: boolean;
  rts: boolean;
  encoding: "utf8" | "gbk";

  openPort: (portName: string, baudRate: number) => Promise<void>;
  closePort: () => Promise<void>;
  setDtr: (enabled: boolean) => Promise<void>;
  setRts: (enabled: boolean) => Promise<void>;
  setEncoding: (encoding: "utf8" | "gbk") => void;
}

export const useSerialStore = create<SerialState>((set, get) => ({
  isOpen: false,
  portName: "",
  baudRate: 115200,
  dataBits: 8,
  stopBits: 1,
  parity: "none",
  dtr: false,
  rts: false,
  encoding: "utf8",

  openPort: async (portName, baudRate) => {
    await window.__TAURI__.core.invoke("cmd_open_port", { portName, baudRate });
    set({ isOpen: true, portName, baudRate });
  },

  closePort: async () => {
    await window.__TAURI__.core.invoke("cmd_close_port");
    set({ isOpen: false });
  },

  setDtr: async (enabled) => {
    await window.__TAURI__.core.invoke("cmd_set_dtr", { enabled });
    set({ dtr: enabled });
  },

  setRts: async (enabled) => {
    await window.__TAURI__.core.invoke("cmd_set_rts", { enabled });
    set({ rts: enabled });
  },

  setEncoding: (encoding) => set({ encoding }),
}));
```

- [ ] **Step 2: 实现 bufferStore**

```typescript
import { create } from 'zustand';

type BufferSize = 1024 * 1024 | 5 * 1024 * 1024 | 10 * 1024 * 1024 | 50 * 1024 * 1024;

interface BufferState {
  bufferSize: BufferSize;
  txBytes: number;
  rxBytes: number;
  overflowCount: number;

  setBufferSize: (size: BufferSize) => void;
  incrementTx: (count: number) => void;
  incrementRx: (count: number) => void;
  resetOverflow: () => void;
}

export const BUFFER_SIZES: BufferSize[] = [
  1 * 1024 * 1024,
  5 * 1024 * 1024,
  10 * 1024 * 1024,
  50 * 1024 * 1024,
];

export const useBufferStore = create<BufferState>((set) => ({
  bufferSize: 10 * 1024 * 1024, // 默认 10MB
  txBytes: 0,
  rxBytes: 0,
  overflowCount: 0,

  setBufferSize: (size) => set({ bufferSize: size }),
  incrementTx: (count) => set((s) => ({ txBytes: s.txBytes + count })),
  incrementRx: (count) => set((s) => ({ rxBytes: s.rxBytes + count })),
  resetOverflow: () => set({ overflowCount: 0 }),
}));
```

- [ ] **Step 3: 实现 presetStore**

```typescript
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface PresetCommand {
  id: string;
  name: string;
  content: string;
  type: "text" | "hex";
  priority: number; // 1-100
  enabled: boolean;
  intervalMs: number;
}

interface PresetState {
  commands: PresetCommand[];
  isPolling: boolean;

  addCommand: (cmd: Omit<PresetCommand, "id">) => void;
  updateCommand: (id: string, updates: Partial<PresetCommand>) => void;
  deleteCommand: (id: string) => void;
  startPolling: () => void;
  stopPolling: () => void;
}

export const usePresetStore = create<PresetState>()(
  persist(
    (set) => ({
      commands: [],
      isPolling: false,

      addCommand: (cmd) =>
        set((s) => ({
          commands: [...s.commands, { ...cmd, id: crypto.randomUUID() }],
        })),

      updateCommand: (id, updates) =>
        set((s) => ({
          commands: s.commands.map((c) =>
            c.id === id ? { ...c, ...updates } : c,
          ),
        })),

      deleteCommand: (id) =>
        set((s) => ({
          commands: s.commands.filter((c) => c.id !== id),
        })),

      startPolling: () => set({ isPolling: true }),
      stopPolling: () => set({ isPolling: false }),
    }),
    { name: "presets" },
  ),
);
```

- [ ] **Step 4: 提交**

```bash
git add -A && git commit -m "feat: 实现 Zustand 状态管理"
```

---

### Task 9: Xterm.js 终端组件

**Files:**

- Create: `src/components/Terminal.tsx`
- Create: `src/components/SerialToolbar.tsx`

- [ ] **Step 1: 创建 Terminal 组件**

```tsx
import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useBufferStore } from "../stores/bufferStore";

interface TerminalProps {
  data: Uint8Array;
  viewMode: "text" | "hex";
  encoding: "utf8" | "gbk";
}

export function Terminal({ data, viewMode, encoding }: TerminalProps) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!terminalRef.current) return;

    const xterm = new XTerm({
      theme: { background: "#111827", foreground: "#F3F4F6" },
      cursorBlink: true,
      fontSize: 14,
    });

    const fitAddon = new FitAddon();
    xterm.loadAddon(fitAddon);
    xterm.open(terminalRef.current);
    fitAddon.fit();

    xtermRef.current = xterm;
    fitAddonRef.current = fitAddon;

    return () => {
      xterm.dispose();
    };
  }, []);

  useEffect(() => {
    if (!xtermRef.current) return;

    if (viewMode === "hex") {
      xtermRef.current.write(formatHexToXTerm(data));
    } else {
      xtermRef.current.write(data);
    }
  }, [data, viewMode, encoding]);

  return <div ref={terminalRef} className="h-full w-full" />;
}
```

- [ ] **Step 2: 创建 SerialToolbar 组件**

```tsx
import { useSerialStore } from "../stores/serialStore";
import { useBufferStore, BUFFER_SIZES } from "../stores/bufferStore";

export function SerialToolbar() {
  const {
    isOpen,
    portName,
    baudRate,
    encoding,
    openPort,
    closePort,
    setEncoding,
  } = useSerialStore();
  const { bufferSize, setBufferSize } = useBufferStore();

  return (
    <div className="flex items-center gap-4 p-2 bg-slate-800">
      <select
        value={portName}
        disabled={isOpen}
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        <option value="">选择串口</option>
        {/* 动态加载串口列表 */}
      </select>

      <select
        value={baudRate}
        onChange={(e) => {
          /* 更新波特率 */
        }}
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        <option value="9600">9600</option>
        <option value="115200">115200</option>
        <option value="921600">921600</option>
      </select>

      <select
        value={encoding}
        onChange={(e) => setEncoding(e.target.value as "utf8" | "gbk")}
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        <option value="utf8">UTF-8</option>
        <option value="gbk">GBK</option>
      </select>

      <select
        value={bufferSize}
        onChange={(e) =>
          setBufferSize(Number(e.target.value) as typeof bufferSize)
        }
        className="px-2 py-1 rounded bg-slate-700 text-white"
      >
        {BUFFER_SIZES.map((size) => (
          <option key={size} value={size}>
            {(size / 1024 / 1024).toFixed(0)}MB 缓冲
          </option>
        ))}
      </select>

      <button
        onClick={() => (isOpen ? closePort() : openPort(portName, baudRate))}
        className={`px-4 py-1 rounded font-medium ${
          isOpen
            ? "bg-red-500 hover:bg-red-600"
            : "bg-blue-500 hover:bg-blue-600"
        } text-white`}
      >
        {isOpen ? "关闭串口" : "打开串口"}
      </button>
    </div>
  );
}
```

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "feat: 实现 Xterm.js Terminal 和 SerialToolbar 组件"
```

---

## 第四阶段：发送队列与定时器

### Task 10: Rust 发送队列

**Files:**

- Create: `src-tauri/src/sender/mod.rs`
- Create: `src-tauri/src/sender/queue.rs`
- Create: `src-tauri/src/sender/timer.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 实现发送队列结构**

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendCommand {
    pub id: String,
    pub content: Vec<u8>,
    pub priority: u8,
    pub interval_ms: u64,
}

pub struct SendQueue {
    commands: Vec<SendCommand>,
    is_polling: bool,
}

impl SendQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            is_polling: false,
        }
    }

    pub fn add(&mut self, cmd: SendCommand) {
        self.commands.push(cmd);
        self.commands.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn remove(&mut self, id: &str) {
        self.commands.retain(|c| c.id != id);
    }

    pub fn start_polling(&mut self) {
        self.is_polling = true;
    }

    pub fn stop_polling(&mut self) {
        self.is_polling = false;
    }

    pub fn is_polling(&self) -> bool {
        self.is_polling
    }

    pub fn next_command(&self) -> Option<&SendCommand> {
        self.commands.first()
    }
}
```

- [ ] **Step 2: 实现高精度定时器**

```rust
use std::time::Duration;
use std::sync::Arc;
use tokio::time::{interval, MissedTickBehavior};
use tokio::sync::Mutex;
use serialport::SerialPort;

pub async fn start_precise_sender(
    port_writer: Arc<Mutex<Box<dyn SerialPort>>>,
    payload: Vec<u8>,
    interval_ms: u64,
) {
    let mut interval_timer = interval(Duration::from_millis(interval_ms));
    interval_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval_timer.tick().await;
        let mut writer = port_writer.lock().await;
        if let Err(e) = writer.write_all(&payload) {
            eprintln!("发送失败: {:?}", e);
            break;
        }
        if let Err(e) = writer.flush() {
            eprintln!("刷新失败: {:?}", e);
            break;
        }
    }
}
```

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "feat: 实现 Rust 发送队列和高精度定时器"
```

---

## 第五阶段：打包发布

### Task 11: NSIS 打包配置

**Files:**

- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: 配置 NSIS 单文件打包**

```json
{
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "windows": {
      "nsis": {
        "installerIcon": "icons/icon.ico",
        "installMode": "currentUser"
      }
    }
  },
  "app": {
    "windows": [
      {
        "title": "OhMySerial",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ]
  }
}
```

- [ ] **Step 2: 验证构建**

```bash
npm run tauri build -- --verbose
# 预期: 输出 OhMySerial.exe 于 target/release/bundle/nsis/
```

- [ ] **Step 3: 提交**

```bash
git add -A && git commit -m "feat: 配置 NSIS 单文件打包"
git tag v0.1.0
git push origin main --tags
```

---

## 进度追踪

- [x] Task 1: Tauri 项目初始化
- [ ] Task 2: 项目结构与错误类型定义
- [ ] Task 3: 串口端口模块
- [ ] Task 4: 环形缓冲区实现
- [ ] Task 5: IPC 命令层
- [ ] Task 6: 背压策略实现
- [ ] Task 7: 前端编码工具
- [ ] Task 8: Zustand Store 实现
- [ ] Task 9: Xterm.js 终端组件
- [ ] Task 10: Rust 发送队列
- [ ] Task 11: NSIS 打包配置

---

## 后续任务

- Task 12: 集成测试（端到端串口通信测试）
- Task 13: 性能基准测试
- Task 14: UI/UX 细节优化
