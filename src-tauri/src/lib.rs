mod error;
mod serial;

pub use error::SerialError;
pub use serial::port::{list_ports, PortInfo};

pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
