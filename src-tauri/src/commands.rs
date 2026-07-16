//! Tauri commands — the IPC surface the Vue frontend calls via `invoke`.

use std::path::PathBuf;
use tauri::Manager;

use crate::connections::{self, ConnectionInfo};
use crate::docker::{self, ContainerInfo};

fn store_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_connections(app: tauri::AppHandle) -> Result<Vec<ConnectionInfo>, String> {
    connections::load(&store_dir(&app)?)
}

#[tauri::command]
pub fn save_connection(
    app: tauri::AppHandle,
    info: ConnectionInfo,
    secret: Option<String>,
) -> Result<(), String> {
    connections::save(&store_dir(&app)?, info, secret)
}

#[tauri::command]
pub fn delete_connection(app: tauri::AppHandle, id: String) -> Result<(), String> {
    connections::delete(&store_dir(&app)?, &id)
}

/// Ping the engine and report its version — the "test connection" button.
#[tauri::command]
pub async fn test_connection(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let info = connections::get(&store_dir(&app)?, &id)?;
    let client = docker::client_for(&info)?;
    docker::ping(&client).await
}

#[tauri::command]
pub async fn list_containers(
    app: tauri::AppHandle,
    connection_id: String,
) -> Result<Vec<ContainerInfo>, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::list_containers(&client).await
}

#[tauri::command]
pub async fn container_action(
    app: tauri::AppHandle,
    connection_id: String,
    container_id: String,
    action: String,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::container_action(&client, &container_id, &action).await
}
