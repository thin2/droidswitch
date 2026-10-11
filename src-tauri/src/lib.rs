mod commands;
pub mod db;
pub mod fs;
pub mod net;
pub mod permissions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::read_text_file,
            commands::write_text_file,
            commands::http_request
        ])
        .run(tauri::generate_context!())
        .expect("Droid Switch failed to start");
}
