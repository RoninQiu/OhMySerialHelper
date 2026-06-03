mod error;
mod ipc;
mod sender;
mod serial;

pub use error::SerialError;
pub use ipc::{BufferStatus, SerialState};
pub use sender::{SendCommand, SendQueue};
pub use serial::port::{list_ports, PortInfo};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
        ])
        .setup(|_app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
