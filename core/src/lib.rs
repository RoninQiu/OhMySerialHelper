//! OhMySerial 核心：UI-agnostic 串口业务逻辑
//!
//! v1.3.0 起从 src-tauri 拆分：
//! - `serial`：serialport 驱动 + RingBuffer
//! - `sender`：多命令发送队列（SendQueue）
//! - `recorder`：录制器（写入行到本地文件）
//! - `backend`：UI-agnostic 的 `Backend` 句柄（Tauri / egui 都用它通信）
//! - `config`：AppConfig 持久化（JSON 原子写）
//! - `error`：SerialError 类型
//! - `log_init`：日志系统初始化 + 日志行读取
//! - `fonts`：系统字体枚举（real-fonts feature）

pub mod backend;
pub mod config;
pub mod error;
pub mod fonts;
pub mod log_init;
pub mod recorder;
pub mod sender;
pub mod serial;

pub use backend::{
    Backend, BackendEvent, ConnectionStatus, OpenPortOptions, ReconnectEvent, ReconnectHandle,
    ReconnectPhase,
};
pub use config::AppConfig;
pub use error::SerialError;
pub use fonts::{list_mono_fonts, FontInfo};
pub use recorder::{start_recording, Recorder, RecorderSummary};
pub use sender::{SendCommand, SendQueue};
pub use serial::port::{list_ports, PortInfo};
pub use serial::ring_buffer::{BackpressureState, RingBuffer};
