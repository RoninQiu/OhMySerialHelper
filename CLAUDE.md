# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> 最后更新：2026-06-03

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

### 数据流（双向）

```
MCU → serialport (Rust) → 后台读取线程 → 64KB RingBuffer → app.emit("serial-data") → 前端 listen → Xterm.js
                                  ↓ (断线)
                          app.emit("port-disconnected") → 前端 → UI 红色告警

前端 SendPanel/PresetPanel → sendData → invoke("cmd_write_data") → serialport
                    ↓ (轮询)
                invoke("cmd_queue_start_polling") → send-poller 后台线程 → 串口
```

**接收实现细节**：
- `cmd_open_port` 启动 `serial-reader` 后台线程（`std::thread::spawn`）
- 线程循环调用 `port.read()` → 写入 `Arc<Mutex<RingBuffer>>` 共享缓冲区
- 触发条件：满 4KB 或 16ms 定时器溢出（由读取线程用 `drain_all()` 批量取出）
- 通过 `AppHandle::emit("serial-data", Vec<u8>)` 推送到前端
- 前端 `App.tsx` 用 `listen("serial-data")` 订阅，写入 `Terminal` ref + 增加 rxBytes

**断线检测**：
- 分级策略：`NotConnected`/`BrokenPipe` 立即断线、`TimedOut` 静默、其他错误累计 3 次
- `app.emit("port-disconnected", ...)` 推送事件 → 前端 store `disconnected: true`

**发送实现细节**：
- 单次发送：`serialStore.sendData(data)` → `cmd_write_data` → serialport（乐观更新 txBytes，失败回滚）
- 队列轮询：`startPolling()` → `cmd_queue_start_polling` → `send-poller` 后台线程（独立于 reader）
- 互斥：`send-poller` 用 `port_handle.try_lock()` + 2ms 重试，与 reader 锁竞争窗口 < 1ms

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
| `serialStore` | `src/stores/serialStore.ts` | 串口连接状态（Zustand）+ sendData action + disconnected 字段 |
| `bufferStore` | `src/stores/bufferStore.ts` | 收发字节统计（rxBytes/txBytes/overflowCount） |
| `presetStore` | `src/stores/presetStore.ts` | 预设命令（localStorage 持久化，version: 2 + migrate）+ startPolling 调 IPC |
| `utils/hex` | `src/utils/hex.ts` | HEX 解析、CRC16、HexDump |
| `utils/encoding` | `src/utils/encoding.ts` | GBK/UTF-8 编解码 |
| `utils/format` | `src/utils/format.ts` | bytesToHuman 字节格式化（B/KB/MB/GB/TB） |
| `Terminal` | `src/components/Terminal.tsx` | Xterm.js 渲染组件 |
| `SerialToolbar` | `src/components/SerialToolbar.tsx` | 串口工具栏（三态连接指示灯） |
| `SendPanel` | `src/components/SendPanel.tsx` | 文本/HEX 发送面板（Enter 发送、Ctrl+Enter 换行） |
| `PresetPanel` | `src/components/PresetPanel.tsx` | 预设命令 CRUD + 快速发送 |
| `StatusBar` | `src/components/StatusBar.tsx` | 状态栏：连接状态 + TX/RX 字节 + 溢出提示 |

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

- **版本**: v0.1.0 (已发布) + v0.2.0 (接收打通) + v0.3.0 (发送闭环，断线检测，UI 完整化)
- **已完成**:
  - Task 1-11（基础框架、IPC、环形缓冲区、Xterm.js 组件）
  - Task 12（集成测试：17 个 Rust 测试 + 34 个前端测试 = **51 测试全部通过**）
  - v0.3.0 增量：
    - 断线检测（分级错误处理 + `port-disconnected` 事件 + 三态指示灯）
    - bufferStore 接入（乐观更新 + 失败回滚）
    - StatusBar 实时显示（连接状态 + TX/RX + 溢出）
    - SendPanel（文本/HEX 输入、Enter 发送）
    - PresetPanel（CRUD + localStorage 持久化 + 快速发送）
    - SendQueue IPC（7 个新命令 + send-poller 后台线程）
- **待完成**: Task 13（性能基准）、Task 14（主题切换、快捷键、文件日志）

## 已知问题

- `PreciseSender` 仍 `#[allow(dead_code)]`（设计预留，v0.4.0 合并到 SendQueue）
- **自动重连**（设备拔出后自动重连）尚未实现
- 自动重连握手协议/重发队列尚未设计