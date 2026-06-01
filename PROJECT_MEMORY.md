# OhMySerial 项目记忆与开发进度

> ⚠️ **重要**: 本文件是项目的长期记忆文档，用于在不同 LLM 之间切换时保持上下文连续性。请勿删除。

## 📌 项目基本信息

| 项目         | 值                                           |
| ------------ | -------------------------------------------- |
| **项目名称** | OhMySerial                                   |
| **版本**     | v0.1.0                                       |
| **定位**     | 面向工业控制的串口调试助手，现代化替代 SSCOM |
| **Git 仓库** | https://github.com/RoninQiu/OhMySerialHelper |
| **创建日期** | 2026-05-29                                   |
| **最后更新** | 2026-06-01                                   |

---

## 🛠 技术栈

| 层级 | 技术                                             |
| ---- | ------------------------------------------------ |
| 后端 | Rust + Tauri 2.x + Tokio + serialport            |
| 前端 | React 18 + TypeScript + Zustand + Tailwind CSS 3 |
| 终端 | Xterm.js 5.5 (WebGL 渲染)                        |
| 打包 | NSIS (Windows)                                   |
| 测试 | Vitest + jsdom                                   |

---

## ✅ 已完成任务 (11/11)

所有 Task 已完成并推送到 GitHub。

| Task    | 提交 Hash | 说明                                    |
| ------- | --------- | --------------------------------------- |
| Task 1  | `78d9261` | Tauri 项目初始化                        |
| Task 2  | `73fb575` | 错误类型定义 `SerialError`              |
| Task 3  | `93ceb5c` | 串口端口模块 `PortInfo`, `list_ports()` |
| Task 4  | `93ceb5c` | 环形缓冲区 `RingBuffer`                 |
| Task 5  | `e57e998` | IPC 命令层                              |
| Task 6  | `501c126` | 背压策略 `BackpressureState`            |
| Task 7  | `501c126` | 前端编码工具 `encoding.ts`, `hex.ts`    |
| Task 8  | `906efc4` | Zustand Store                           |
| Task 9  | `2509c75` | Xterm.js Terminal 和 SerialToolbar      |
| Task 10 | `b203729` | Rust 发送队列 `SendQueue`               |
| Task 11 | `2e3019f` | NSIS 打包配置                           |

**Tag**: `v0.1.0` 已推送到 origin/main

---

## 📁 项目结构

```
OhMySerialHelper/
├── src/                          # React 前端
│   ├── components/
│   │   ├── Terminal.tsx          # Xterm.js 终端组件
│   │   └── SerialToolbar.tsx     # 串口工具栏
│   ├── stores/
│   │   ├── serialStore.ts        # 串口状态管理
│   │   ├── bufferStore.ts        # 缓冲区统计
│   │   └── presetStore.ts        # 预设命令(localStorage持久化)
│   ├── utils/
│   │   ├── encoding.ts           # GBK/UTF-8 编解码
│   │   └── hex.ts               # HEX 转换、CRC16、HexDump
│   ├── App.tsx                  # 主应用组件
│   ├── main.tsx                 # 入口
│   └── index.css                # Tailwind CSS
├── src-tauri/                   # Rust 后端
│   ├── tauri.conf.json          # Tauri 配置
│   ├── Cargo.toml               # Rust 依赖
│   └── src/
│       ├── lib.rs               # Tauri Builder 入口
│       ├── main.rs              # main.rs (调用 lib::run)
│       ├── error.rs             # SerialError 枚举
│       ├── ipc/                 # IPC 命令层
│       │   ├── mod.rs
│       │   └── commands.rs      # 所有 Tauri commands
│       ├── sender/              # 发送队列模块
│       │   ├── mod.rs
│       │   ├── queue.rs         # SendQueue 实现
│       │   └── timer.rs         # PreciseSender (高精度定时)
│       └── serial/              # 串口驱动模块
│           ├── mod.rs
│           ├── port.rs          # PortInfo, list_ports(), guess_port_type()
│           └── ring_buffer.rs   # RingBuffer + BackpressureState
├── docs/plans/                  # 设计文档
│   ├── 2026-05-29-OhMySerial-design.md      # 架构设计
│   └── 2026-05-29-OhMySerial-implementation.md  # 实施计划
├── tests/frontend/              # 前端测试
│   ├── hex.test.ts              # 12 tests (hexToBytes, bytesToHex, etc.)
│   └── bufferStore.test.ts      # 6 tests
└── 配置文件
    ├── vite.config.ts           # Vite 配置
    ├── tailwind.config.js       # Tailwind 配置
    ├── postcss.config.js        # PostCSS 配置
    ├── tsconfig.json           # TypeScript 配置
    ├── vitest.config.ts        # Vitest 配置
    └── .gitattributes          # Line endings 规范化
```

---

## 🔧 Rust 后端核心模块

### error.rs - 错误类型

```rust
pub enum SerialError {
    OpenFailed(String),     // 串口打开失败
    PortLocked,             // 串口被占用
    SendTimeout,            // 发送超时
    ReceiveError(String),   // 接收错误
    BufferOverflow(usize),   // 缓冲区溢出，已丢弃字节数
    PortNotOpen,            // 串口未打开
    InvalidConfig(String),   // 配置无效
}
```

### ipc/commands.rs - IPC 命令

```rust
pub struct SerialState {
    pub ring_buffer: Mutex<RingBuffer>,  // 64KB 环形缓冲区
    pub port_handle: Mutex<Option<Box<dyn SerialPort>>>,
}

#[tauri::command]
pub fn cmd_list_ports() -> Result<Vec<PortInfo>, String>
    // 返回可用串口列表

#[tauri::command]
pub fn cmd_open_port(port_name, baud_rate, data_bits, stop_bits, parity) -> Result<(), String>
    // 打开串口

#[tauri::command]
pub fn cmd_close_port(state) -> Result<(), String>
    // 关闭串口

#[tauri::command]
pub fn cmd_read_buffer(state, len) -> Result<Vec<u8>, String>
    // 读取缓冲区数据

#[tauri::command]
pub fn cmd_write_data(state, data) -> Result<(), String>
    // 写入数据到串口

#[tauri::command]
pub fn cmd_get_buffer_status(state) -> Result<BufferStatus, String>
    // 获取缓冲区状态

#[derive(serde::Serialize)]
pub struct BufferStatus {
    pub data_len: usize,
    pub water_level: f32,
    pub backpressure: String,  // "Normal" | "Throttled" | "Overflow"
    pub overflow_count: usize,
}
```

### serial/port.rs - 串口端口

```rust
pub struct PortInfo {
    pub name: String,      // "COM3"
    pub port_type: String, // "CH340", "FTDI", "CP210x", "Unknown"
}

pub fn list_ports() -> Vec<PortInfo>
fn guess_port_type(name: &str) -> String
```

### serial/ring_buffer.rs - 环形缓冲区

```rust
pub struct RingBuffer {
    buf: Vec<u8>,
    capacity: usize,
    write_pos: usize,
    read_pos: usize,
    overflow_count: usize,
}

// 水位阈值
const WATER_LEVEL_LOW: f32 = 0.50;   // 恢复正常刷新率
const WATER_LEVEL_MID: f32 = 0.75;   // 降低刷新率
const WATER_LEVEL_HIGH: f32 = 0.90;   // 丢包警告

pub enum BackpressureState { Normal, Throttled, Overflow }

impl RingBuffer {
    pub fn backpressure_state() -> BackpressureState
    pub fn should_flush() -> bool  // 触发条件：满 4KB
    pub fn water_level() -> f32
}
```

### sender/queue.rs - 发送队列

```rust
pub struct SendCommand {
    pub id: String,
    pub content: Vec<u8>,
    pub priority: u8,      // 1-100, 越高越优先
    pub interval_ms: u64,
}

pub struct SendQueue {
    commands: Vec<SendCommand>,
    is_polling: bool,
}

impl SendQueue {
    pub fn add(cmd: SendCommand)   // 按优先级排序
    pub fn remove(id: &str)
    pub fn start_polling()
    pub fn stop_polling()
    pub fn next_command() -> Option<&SendCommand>
}
```

### sender/timer.rs - 高精度定时器

```rust
pub struct PreciseSender {
    is_running: bool,
}

impl PreciseSender {
    pub async fn start_sending(
        port_handle,
        payload: Vec<u8>,
        interval_ms: u64,
    )
    // 使用 tokio::time::interval + MissedTickBehavior::Delay
    // 实现高精度定时发送

    pub fn stop()
}
```

---

## 🎨 前端核心模块

### stores/serialStore.ts - 串口状态

```typescript
interface SerialState {
  isOpen: boolean;
  portName: string;
  baudRate: number;
  dataBits: 5 | 6 | 7 | 8;
  stopBits: 1 | 2;
  parity: "none" | "odd" | "even";
  dtr: boolean;
  rts: boolean;
  encoding: "utf8" | "gbk";

  openPort(portName: string, baudRate: number): Promise<void>;
  closePort(): Promise<void>;
  setDtr(enabled: boolean): Promise<void>;
  setRts(enabled: boolean): Promise<void>;
  setEncoding(encoding: "utf8" | "gbk"): void;
}
```

### stores/bufferStore.ts - 缓冲区统计

```typescript
type BufferSize = 1MB | 5MB | 10MB | 50MB

interface BufferState {
  bufferSize: BufferSize
  txBytes: number
  rxBytes: number
  overflowCount: number

  setBufferSize(size: BufferSize): void
  incrementTx(count: number): void
  incrementRx(count: number): void
  resetOverflow(): void
}

export const BUFFER_SIZES: BufferSize[] = [...]
```

### stores/presetStore.ts - 预设命令

```typescript
interface PresetCommand {
  id: string;
  name: string;
  content: string;
  type: "text" | "hex";
  priority: number; // 1-100
  enabled: boolean;
  intervalMs: number;
}
// 使用 zustand/middleware persist, 存储到 localStorage
```

### utils/encoding.ts - 编码工具

```typescript
export function decodeGBK(buffer: Uint8Array): string;
export function detectEncoding(buffer: Uint8Array): "utf8" | "gbk" | "latin1";
export function encodeGBK(text: string): Uint8Array;
```

### utils/hex.ts - HEX 工具

```typescript
export function hexToBytes(hex: string): Uint8Array;
export function bytesToHex(bytes: Uint8Array): string;
export function formatHexDump(bytes, bytesPerLine?, startOffset?): string;
export function extractPrintableAscii(bytes: Uint8Array): string;
export function crc16Modbus(data: Uint8Array): number;
export function isValidHex(hex: string): boolean;
```

---

## 🧪 测试状态

```bash
$ npm test

✓ tests/frontend/hex.test.ts (12 tests)
  - hexToBytes: parses spaced/compact hex, empty string, odd length
  - bytesToHex: converts to spaced hex, empty array, padding
  - formatHexDump: formats correctly, multi-line output
  - isValidHex: validates correct/invalid hex
  - crc16Modbus: calculates correctly

✓ tests/frontend/bufferStore.test.ts (6 tests)
  - initializes with default values
  - sets buffer size
  - increments tx/rx bytes
  - resets overflow count
  - BUFFER_SIZES contains expected sizes

Test Files: 2 passed
Tests: 18 passed
```

---

## ⏭ 后续任务 (未开始)

### Task 12: 集成测试

- 端到端串口通信测试
- 连接断开重连测试
- 数据收发完整性测试

### Task 13: 性能基准测试

- 高波特率 (921600) 压力测试
- 大数据量传输测试
- 内存使用监控

### Task 14: UI/UX 细节优化

- 预设命令面板完整实现
- 定时发送功能 UI
- 主题切换（深色/浅色）
- 快捷键支持

---

## 🔨 构建与运行

```bash
# 安装依赖
npm install

# 开发模式（需要先启动 Vite 再启动 Tauri）
npm run tauri dev

# 生产构建
npm run tauri build
# 产物位置: src-tauri/target/release/bundle/nsis/OhMySerial_0.1.0_x64-setup.exe

# 前端单独测试
npm test

# 前端构建 (dist/)
npx vite build
```

---

## ⚠️ 当前 UI 状态

### ✅ 已实现

- 基本布局框架（标题栏、工具栏、终端区、状态栏）
- Tailwind CSS 深色主题 (#1f2937 背景)
- Xterm.js 终端组件（欢迎信息、主题配置）
- 视图切换（文本/HEX）

### 🔄 待完成

- 串口连接功能与 Rust IPC 集成
- 实际数据收发通信
- 预设命令管理面板
- 定时发送配置 UI
- 流控 (DTR/RTS) 配置

---

## 💡 关键设计决策记录

### 1. 背压策略阈值

- 50% 以下: Normal (正常刷新率)
- 50-75%: Throttled (降低刷新率)
- 75-90%: Throttled (进一步降低)
- 90% 以上: Overflow (丢包警告)

### 2. 刷新触发条件

- 满 4KB 数据 OR
- 16ms 定时器溢出 (约 60Hz)

### 3. 缓冲区大小选项

- 1MB / 5MB / 10MB / 50MB 四档可选

### 4. Line Endings 配置

使用 `.gitattributes` 规范化所有源代码文件为 LF:

```
*.ts text eol=lf
*.tsx text eol=lf
*.rs text eol=lf
```

---

## 📝 已知问题

1. **Rust 部分 warnings**: `PreciseSender` 未被使用（设计预留）
2. **Rust 部分 warnings**: `WATER_LEVEL_LOW` 常量未使用
3. **前端**: SerialToolbar 中有未实现的 setBaudRate 引用

---

## 🔄 最近提交历史

| 提交      | 说明                                               |
| --------- | -------------------------------------------------- |
| `3dc383d` | feat: 实现基本 UI 组件和 Tailwind CSS              |
| `075e84e` | fix: 修复前端配置和依赖                            |
| `2e3019f` | feat: 配置 NSIS 单文件打包                         |
| `b203729` | feat: 实现 Rust 发送队列和高精度定时器             |
| `2509c75` | feat: 实现 Xterm.js Terminal 和 SerialToolbar 组件 |
| `906efc4` | feat: 实现 Zustand 状态管理                        |
| `501c126` | feat: 实现前端编码和 HEX 工具                      |
| `e57e998` | feat: 实现 IPC 命令层基础框架                      |

---

## 📚 参考文档

- [Tauri 2.x 文档](https://tauri.app/)
- [Xterm.js 文档](https://xtermjs.org/)
- [Zustand 文档](https://zustand.js.org/)
- [Tailwind CSS 文档](https://tailwindcss.com/)

---

**最后更新时间**: 2026-06-01
