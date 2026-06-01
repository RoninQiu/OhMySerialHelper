# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> 最后更新：2026-06-01

## 项目概述

OhMySerial 是一款面向工业控制的串口调试助手，使用 Tauri 2.x + Rust 后端 + React 前端构建。

## 构建与运行

```bash
# 安装依赖
npm install

# 开发模式（前后端联动）
npm run tauri dev

# 生产构建
npm run tauri build
# 产物位置: src-tauri/target/release/bundle/nsis/OhMySerial_0.1.0_x64-setup.exe

# 前端单元测试
npm test

# 前端独立构建
npx vite build
```

## 架构概览

### 数据流（接收路径）

```
MCU → serialport (Rust) → 后台读取线程 → 64KB RingBuffer → app.emit("serial-data") → 前端 listen → Xterm.js
```

**实现细节**：
- `cmd_open_port` 启动 `serial-reader` 后台线程（`std::thread::spawn`）
- 线程循环调用 `port.read()` → 写入 `Arc<Mutex<RingBuffer>>` 共享缓冲区
- 触发条件：满 4KB 或 16ms 定时器溢出（由读取线程用 `drain_all()` 批量取出）
- 通过 `AppHandle::emit("serial-data", Vec<u8>)` 推送到前端
- 前端 `App.tsx` 用 `listen("serial-data")` 订阅，写入 `Terminal` ref

**关键设计**：`Arc<Mutex<>>` 跨线程共享缓冲区 + `Arc<AtomicBool>` stop_flag 控制线程退出。

### Rust 后端模块

| 模块 | 文件 | 职责 |
|------|------|------|
| `serial/port` | `src-tauri/src/serial/port.rs` | 串口枚举、CH340/FTDI/CP210x 识别 |
| `serial/ring_buffer` | `src-tauri/src/serial/ring_buffer.rs` | 64KB 环形队列、背压策略、水位检测 |
| `ipc/commands` | `src-tauri/src/ipc/commands.rs` | Tauri IPC 命令（打开/关闭/读写串口） |
| `sender/queue` | `src-tauri/src/sender/queue.rs` | 发送队列（优先级排序） |
| `sender/timer` | `src-tauri/src/sender/timer.rs` | 高精度定时器（tokio::time::interval） |

### 前端模块

| 模块 | 文件 | 职责 |
|------|------|------|
| `serialStore` | `src/stores/serialStore.ts` | 串口连接状态（Zustand） |
| `bufferStore` | `src/stores/bufferStore.ts` | 收发字节统计 |
| `presetStore` | `src/stores/presetStore.ts` | 预设命令（localStorage 持久化） |
| `utils/hex` | `src/utils/hex.ts` | HEX 解析、CRC16、HexDump |
| `utils/encoding` | `src/utils/encoding.ts` | GBK/UTF-8 编解码 |
| `Terminal` | `src/components/Terminal.tsx` | Xterm.js 渲染组件 |
| `SerialToolbar` | `src/components/SerialToolbar.tsx` | 串口工具栏 |

### 背压策略

- `water_level < 50%`: Normal，正常刷新
- `water_level 50-75%`: Throttled，降低刷新率
- `water_level > 90%`: Overflow，丢包并通知前端

### Tauri 2.x Capabilities 权限

`src-tauri/capabilities/default.json` 必须显式声明前端可用权限：

- `core:default` — 基础命令调用
- `core:event:allow-listen` / `core:event:allow-unlisten` — 监听后端事件

**坑**：Tauri 2.x 默认拒绝所有事件/命令，capabilities 为空时 `listen()` 静默失败（数据"收不到"但无报错）。

## 当前状态

- **版本**: v0.1.0 (已发布) + v0.2.0 增量（数据接收打通，未发布）
- **已完成**: Task 1-11（基础框架、IPC、环形缓冲区、Xterm.js 组件）+ **后台读取线程 + 事件推送链路**
- **待完成**: Task 12（集成测试）、Task 13（性能基准测试）、Task 14（UI/UX 优化）

## 已知问题

- `PreciseSender` 已实现但未被 IPC 调用（设计预留，Task 14 集成时移除 `#[allow(dead_code)]`）
- `SendQueue` 优先级排序已实现但尚未接入轮询任务
- 数据接收已可用，但**自动重连 / 断线检测**尚未实现（设备拔出时 UI 不会自动感知）