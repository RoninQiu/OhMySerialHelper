//! Tauri IPC commands — 全部为薄壳，桥接到 `oh_my_serial_core::Backend`
//!
//! v1.3.0 重构：
//! - 所有业务逻辑移到 core
//! - 这里只剩 3 件事：
//!   1. 把 `Channel<Vec<u8>>` ↔ `mpsc::Sender<Vec<u8>>` 桥接（forwarder task）
//!   2. 把 `BackendEvent` ↔ `app.emit("...")` 桥接（在 lib.rs setup 里订阅一次）
//!   3. 27 个 Tauri command 把参数解包 → Backend 方法 → 结果打包
//!
//! 性能：
//! - 数据通道保持 v0.6.0 的零拷贝设计（Tauri Channel），只是中间多一跳 mpsc
//! - 事件频率低（断线/重连状态变化），不影响性能

pub mod commands;
