mod error;

pub use error::SerialError;

pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            log::info!("OhMySerial starting...");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
