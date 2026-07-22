pub mod commands;
pub mod connections;
pub mod docker;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::LogStreams::default())
        .manage(commands::EventStreams::default())
        .manage(commands::ExecSessions::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::test_connection,
            commands::list_containers,
            commands::container_action,
            commands::inspect_container,
            commands::create_container,
            commands::list_images,
            commands::remove_image,
            commands::pull_image,
            commands::start_log_stream,
            commands::stop_log_stream,
            commands::start_event_stream,
            commands::stop_event_stream,
            commands::start_exec,
            commands::write_exec_input,
            commands::resize_exec,
            commands::stop_exec,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
