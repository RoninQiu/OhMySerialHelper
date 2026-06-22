mod config_impl;
mod error;
mod fonts;
mod ipc;
mod log_init;
#[doc(hidden)]
pub mod recorder;
mod sender;
mod serial;

pub use error::SerialError;
pub use fonts::{list_mono_fonts, FontInfo};
pub use ipc::{BufferStatus, SerialState};
pub use recorder::{Recorder, RecorderSummary};
pub use sender::{SendCommand, SendQueue};
pub use serial::port::{list_ports, PortInfo};
pub use serial::ring_buffer::RingBuffer;
pub use config_impl::AppConfig;

/// 暴露 config 子模块给 integration tests 使用
#[doc(hidden)]
pub mod config {
    pub use crate::config_impl::*;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志系统
    log_init::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ipc::commands::SerialState::default())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::cmd_list_ports,
            ipc::commands::cmd_open_port,
            ipc::commands::cmd_close_port,
            ipc::commands::cmd_read_buffer,
            ipc::commands::cmd_write_data,
            ipc::commands::cmd_get_buffer_status,
            ipc::commands::cmd_get_connection_status,
            ipc::commands::cmd_queue_add,
            ipc::commands::cmd_queue_remove,
            ipc::commands::cmd_queue_clear,
            ipc::commands::cmd_queue_start_polling,
            ipc::commands::cmd_queue_stop_polling,
            ipc::commands::cmd_queue_status,
            ipc::commands::cmd_start_periodic_send,
            ipc::commands::cmd_stop_periodic_send,
            ipc::commands::cmd_get_log_dir,
            ipc::commands::cmd_read_log_lines,
            ipc::commands::cmd_open_log_dir,
            ipc::commands::cmd_load_config,
            ipc::commands::cmd_save_config,
            ipc::commands::cmd_cancel_reconnect,
            ipc::commands::cmd_list_fonts,
            // v1.2.0 录制功能
            ipc::commands::cmd_start_recording,
            ipc::commands::cmd_stop_recording,
            ipc::commands::cmd_write_recorder_line,
            ipc::commands::cmd_mark_recorder_event,
            ipc::commands::cmd_is_recording,
        ])
        .setup(|_app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
