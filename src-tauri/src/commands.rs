//! Tauri commands — the IPC surface the Vue frontend calls via `invoke`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bollard::Docker;
use futures_util::StreamExt;
use tauri::{async_runtime::JoinHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as AsyncMutex;

use crate::connections::{self, ConnectionInfo};
use crate::docker::{
    self, ContainerDetail, ContainerInfo, ExecInput, ImageInfo, PortMapping, VolumeInfo,
};
use crate::settings::{self, Settings};

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

#[tauri::command]
pub async fn inspect_container(
    app: tauri::AppHandle,
    connection_id: String,
    container_id: String,
) -> Result<ContainerDetail, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::inspect_container(&client, &container_id).await
}

/// Create a container from an image and start it; returns the new
/// container's ID.
#[tauri::command]
pub async fn create_container(
    app: tauri::AppHandle,
    connection_id: String,
    image: String,
    name: Option<String>,
    ports: Vec<PortMapping>,
    env: Vec<String>,
) -> Result<String, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::create_and_start_container(&client, &image, name.as_deref(), &ports, &env).await
}

#[tauri::command]
pub async fn list_images(
    app: tauri::AppHandle,
    connection_id: String,
) -> Result<Vec<ImageInfo>, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::list_images(&client).await
}

#[tauri::command]
pub async fn remove_image(
    app: tauri::AppHandle,
    connection_id: String,
    image: String,
    force: bool,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::remove_image(&client, &image, force).await
}

/// Pull an image, forwarding layer-by-layer progress as `pull-progress:{image}`
/// events while the command is in flight; resolves once the pull completes.
#[tauri::command]
pub async fn pull_image(
    app: tauri::AppHandle,
    connection_id: String,
    image: String,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;

    let event = format!("pull-progress:{image}");
    let mut stream = docker::pull_image(&client, &image);
    while let Some(item) = stream.next().await {
        let progress = item?;
        let _ = app.emit(&event, progress);
    }
    Ok(())
}

#[tauri::command]
pub async fn list_volumes(
    app: tauri::AppHandle,
    connection_id: String,
) -> Result<Vec<VolumeInfo>, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::list_volumes(&client).await
}

#[tauri::command]
pub async fn remove_volume(
    app: tauri::AppHandle,
    connection_id: String,
    name: String,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;
    docker::remove_volume(&client, &name).await
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

/// Tracks the in-flight container-events task per connection so switching or
/// disconnecting cancels the previous subscription instead of leaking it.
#[derive(Default)]
pub struct EventStreams(Mutex<HashMap<String, JoinHandle<()>>>);

fn cancel_event_stream(streams: &EventStreams, connection_id: &str) {
    if let Some(handle) = streams.0.lock().unwrap().remove(connection_id) {
        handle.abort();
    }
}

/// Start following container lifecycle events for a connection; events arrive
/// on the frontend as `container-event:{connectionId}`. Cancels any
/// subscription already running for this connection first.
#[tauri::command]
pub async fn start_event_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, EventStreams>,
    connection_id: String,
) -> Result<(), String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;

    cancel_event_stream(&state, &connection_id);

    let event = format!("container-event:{connection_id}");
    let app_handle = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut stream = docker::stream_container_events(&client);
        while let Some(item) = stream.next().await {
            match item {
                Ok(evt) => {
                    let _ = app_handle.emit(&event, evt);
                }
                Err(_) => break,
            }
        }
    });

    state.0.lock().unwrap().insert(connection_id, handle);
    Ok(())
}

#[tauri::command]
pub fn stop_event_stream(
    state: tauri::State<'_, EventStreams>,
    connection_id: String,
) -> Result<(), String> {
    cancel_event_stream(&state, &connection_id);
    Ok(())
}

/// A running exec session: the engine client (for resize) and the stdin
/// writer (for keystrokes), plus the output-forwarding task so closing the
/// terminal tab can cancel it.
struct ExecSession {
    docker: Docker,
    input: Arc<AsyncMutex<ExecInput>>,
    task: JoinHandle<()>,
}

/// Tracks in-flight exec sessions by exec ID.
#[derive(Default)]
pub struct ExecSessions(Mutex<HashMap<String, ExecSession>>);

fn cancel_exec_session(sessions: &ExecSessions, exec_id: &str) {
    if let Some(session) = sessions.0.lock().unwrap().remove(exec_id) {
        session.task.abort();
    }
}

/// Exec `shell` into a running container with a TTY attached. Output arrives
/// on the frontend as `exec-output:{execId}` events; the returned exec ID is
/// the handle for subsequent input/resize/stop calls.
#[tauri::command]
pub async fn start_exec(
    app: tauri::AppHandle,
    state: tauri::State<'_, ExecSessions>,
    connection_id: String,
    container_id: String,
    shell: String,
) -> Result<String, String> {
    let info = connections::get(&store_dir(&app)?, &connection_id)?;
    let client = docker::client_for(&info)?;

    let (exec_id, mut output, input) = docker::start_exec(&client, &container_id, &shell).await?;

    let event = format!("exec-output:{exec_id}");
    let app_handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        while let Some(item) = output.next().await {
            match item {
                Ok(chunk) => {
                    let _ = app_handle.emit(&event, chunk);
                }
                Err(_) => break,
            }
        }
    });

    state.0.lock().unwrap().insert(
        exec_id.clone(),
        ExecSession {
            docker: client,
            input: Arc::new(AsyncMutex::new(input)),
            task,
        },
    );

    Ok(exec_id)
}

/// Write keystrokes (or a paste) into an exec session's stdin.
#[tauri::command]
pub async fn write_exec_input(
    state: tauri::State<'_, ExecSessions>,
    exec_id: String,
    data: String,
) -> Result<(), String> {
    let input = {
        let sessions = state.0.lock().unwrap();
        let session = sessions
            .get(&exec_id)
            .ok_or_else(|| format!("no exec session for {exec_id}"))?;
        session.input.clone()
    };
    let mut input = input.lock().await;
    input
        .write_all(data.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    input.flush().await.map_err(|e| e.to_string())
}

/// Resize an exec session's TTY to match the frontend terminal's dimensions.
#[tauri::command]
pub async fn resize_exec(
    state: tauri::State<'_, ExecSessions>,
    exec_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let docker = {
        let sessions = state.0.lock().unwrap();
        let session = sessions
            .get(&exec_id)
            .ok_or_else(|| format!("no exec session for {exec_id}"))?;
        session.docker.clone()
    };
    docker::resize_exec(&docker, &exec_id, cols, rows).await
}

#[tauri::command]
pub fn stop_exec(state: tauri::State<'_, ExecSessions>, exec_id: String) -> Result<(), String> {
    cancel_exec_session(&state, &exec_id);
    Ok(())
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    settings::load(&store_dir(&app)?)
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&store_dir(&app)?, &settings)
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

    /// Same idempotency guarantee as `stop_log_stream`: switching connections
    /// (or disconnecting) when no event stream was running must not panic.
    #[test]
    fn cancel_event_stream_on_unknown_connection_is_a_noop() {
        let streams = EventStreams::default();
        cancel_event_stream(&streams, "never-started");
        cancel_event_stream(&streams, "never-started"); // still a no-op
    }

    #[tokio::test]
    async fn cancel_event_stream_is_idempotent_after_a_real_stream() {
        let streams = EventStreams::default();
        let handle = tauri::async_runtime::spawn(async {
            std::future::pending::<()>().await;
        });
        streams.0.lock().unwrap().insert("c1".to_string(), handle);

        cancel_event_stream(&streams, "c1");
        assert!(streams.0.lock().unwrap().get("c1").is_none());

        // Second call: entry already gone, must not panic.
        cancel_event_stream(&streams, "c1");
    }

    fn dummy_exec_session() -> ExecSession {
        ExecSession {
            docker: Docker::connect_with_local_defaults().unwrap(),
            input: Arc::new(AsyncMutex::new(Box::pin(tokio::io::sink()))),
            task: tauri::async_runtime::spawn(async {
                std::future::pending::<()>().await;
            }),
        }
    }

    /// Same idempotency guarantee as `stop_log_stream`: closing a terminal
    /// tab that never started (or whose exec already ended) must not panic.
    #[test]
    fn cancel_exec_session_on_unknown_id_is_a_noop() {
        let sessions = ExecSessions::default();
        cancel_exec_session(&sessions, "never-started");
        cancel_exec_session(&sessions, "never-started"); // still a no-op
    }

    #[tokio::test]
    async fn cancel_exec_session_is_idempotent_after_a_real_session() {
        let sessions = ExecSessions::default();
        sessions
            .0
            .lock()
            .unwrap()
            .insert("e1".to_string(), dummy_exec_session());

        cancel_exec_session(&sessions, "e1");
        assert!(sessions.0.lock().unwrap().get("e1").is_none());

        // Second call: entry already gone, must not panic.
        cancel_exec_session(&sessions, "e1");
    }
}
