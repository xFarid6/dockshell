//! Tauri commands — the IPC surface the Vue frontend calls via `invoke`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use futures_util::StreamExt;
use tauri::{async_runtime::JoinHandle, Emitter, Manager};

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

/// Tracks the in-flight log-follow task per container so a new "Logs" click
/// or a panel close can cancel the previous stream instead of leaking it.
#[derive(Default)]
pub struct LogStreams(Mutex<HashMap<String, JoinHandle<()>>>);

fn cancel_log_stream(streams: &LogStreams, container_id: &str) {
    if let Some(handle) = streams.0.lock().unwrap().remove(container_id) {
        handle.abort();
    }
}

/// Start following a container's logs; lines arrive on the frontend as
/// `log-line:{containerId}` events. Cancels any stream already running for
/// this container first.
#[tauri::command]
pub async fn start_log_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, LogStreams>,
    connection_id: String,
    container_id: String,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;

    cancel_log_stream(&state, &container_id);

    let event = format!("log-line:{container_id}");
    let app_handle = app.clone();
    let cid = container_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut stream = docker::stream_logs(&client, &cid);
        while let Some(item) = stream.next().await {
            match item {
                Ok(line) => {
                    let _ = app_handle.emit(&event, line);
                }
                Err(_) => break,
            }
        }
    });

    state.0.lock().unwrap().insert(container_id, handle);
    Ok(())
}

#[tauri::command]
pub fn stop_log_stream(
    state: tauri::State<'_, LogStreams>,
    container_id: String,
) -> Result<(), String> {
    cancel_log_stream(&state, &container_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stopping a container that never had a stream (or was already
    /// stopped) must be a no-op, not a panic — the frontend calls
    /// `stop_log_stream` unconditionally on unmount/container-switch even
    /// when it never confirmed a stream was running.
    #[test]
    fn cancel_log_stream_on_unknown_container_is_a_noop() {
        let streams = LogStreams::default();
        cancel_log_stream(&streams, "never-started");
        cancel_log_stream(&streams, "never-started"); // still a no-op
    }

    /// Calling stop twice in a row for a container that *was* streaming
    /// aborts the task once and is harmless the second time (mirrors the
    /// frontend's watch handler, which can fire stop for the same id more
    /// than once in edge cases).
    #[tokio::test]
    async fn cancel_log_stream_is_idempotent_after_a_real_stream() {
        let streams = LogStreams::default();
        let handle = tauri::async_runtime::spawn(async {
            std::future::pending::<()>().await;
        });
        streams.0.lock().unwrap().insert("c1".to_string(), handle);

        cancel_log_stream(&streams, "c1");
        assert!(streams.0.lock().unwrap().get("c1").is_none());

        // Second call: entry already gone, must not panic.
        cancel_log_stream(&streams, "c1");
    }
}
