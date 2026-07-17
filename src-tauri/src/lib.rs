pub mod commands;
pub mod connections;
pub mod docker;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::LogStreams::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::test_connection,
            commands::list_containers,
            commands::container_action,
            commands::start_log_stream,
            commands::stop_log_stream,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
