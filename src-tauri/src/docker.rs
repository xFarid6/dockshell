//! Thin wrapper around bollard: build a client from a saved connection and
//! expose the few operations the scaffold needs (list, start/stop/restart).

use std::collections::HashMap;
use std::pin::Pin;

use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::query_parameters::{
    InspectContainerOptions, ListContainersOptionsBuilder, LogsOptionsBuilder,
    ResizeExecOptionsBuilder, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use futures_util::{Stream, StreamExt};
use serde::Serialize;
use tokio::io::AsyncWrite;

use crate::connections::ConnectionInfo;

/// What the frontend renders per container row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: Vec<String>,
}

pub fn client_for(info: &ConnectionInfo) -> Result<Docker, String> {
    if info.endpoint == "local" {
        Docker::connect_with_local_defaults().map_err(|e| e.to_string())
    } else {
        // tcp:// and http:// both accepted. TLS client certs are issue #7.
        Docker::connect_with_http(&info.endpoint, 10, bollard::API_DEFAULT_VERSION)
            .map_err(|e| e.to_string())
    }
}

pub async fn ping(docker: &Docker) -> Result<String, String> {
    let v = docker.version().await.map_err(|e| e.to_string())?;
    Ok(format!(
        "Docker {} (API {})",
        v.version.unwrap_or_default(),
        v.api_version.unwrap_or_default()
    ))
}

pub async fn list_containers(docker: &Docker) -> Result<Vec<ContainerInfo>, String> {
    let opts = ListContainersOptionsBuilder::new().all(true).build();
    let summaries = docker
        .list_containers(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(summaries
        .into_iter()
        .map(|c| ContainerInfo {
            id: c.id.unwrap_or_default(),
            // Docker returns names with a leading slash.
            name: c
                .names
                .unwrap_or_default()
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default(),
            image: c.image.unwrap_or_default(),
            state: c.state.map(|s| s.to_string()).unwrap_or_default(),
            status: c.status.unwrap_or_default(),
            ports: c
                .ports
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| {
                    p.public_port
                        .map(|pub_p| format!("{}:{}", pub_p, p.private_port))
                })
                .collect(),
        })
        .collect())
}

pub async fn container_action(docker: &Docker, id: &str, action: &str) -> Result<(), String> {
    match action {
        "start" => docker
            .start_container(id, None::<StartContainerOptions>)
            .await
            .map_err(|e| e.to_string()),
        "stop" => docker
            .stop_container(id, None::<StopContainerOptions>)
            .await
            .map_err(|e| e.to_string()),
        "restart" => docker
            .restart_container(id, None::<RestartContainerOptions>)
            .await
            .map_err(|e| e.to_string()),
        other => Err(format!("unknown container action: {other}")),
    }
}

/// A bind/volume mount attached to a container.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountInfo {
    pub source: String,
    pub destination: String,
    pub mode: String,
}

/// A host port bound to a container port (`"80/tcp"` etc).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortBinding {
    pub container_port: String,
    pub host_ip: String,
    pub host_port: String,
}

/// Everything `docker inspect` gives us that the detail panel renders.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub health: Option<String>,
    pub created: String,
    pub restart_policy: String,
    pub env: Vec<String>,
    pub labels: HashMap<String, String>,
    pub mounts: Vec<MountInfo>,
    pub ports: Vec<PortBinding>,
}

pub async fn inspect_container(docker: &Docker, id: &str) -> Result<ContainerDetail, String> {
    let info = docker
        .inspect_container(id, Some(InspectContainerOptions::default()))
        .await
        .map_err(|e| e.to_string())?;

    let config = info.config.unwrap_or_default();
    let host_config = info.host_config.unwrap_or_default();
    let state = info.state.unwrap_or_default();
    let network_settings = info.network_settings.unwrap_or_default();

    let ports = network_settings
        .ports
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(container_port, bindings)| {
            bindings
                .unwrap_or_default()
                .into_iter()
                .map(move |b| PortBinding {
                    container_port: container_port.clone(),
                    host_ip: b.host_ip.unwrap_or_default(),
                    host_port: b.host_port.unwrap_or_default(),
                })
        })
        .collect();

    let mounts = info
        .mounts
        .unwrap_or_default()
        .into_iter()
        .map(|m| MountInfo {
            source: m.source.unwrap_or_default(),
            destination: m.destination.unwrap_or_default(),
            mode: m.mode.unwrap_or_default(),
        })
        .collect();

    Ok(ContainerDetail {
        id: info.id.unwrap_or_default(),
        name: info
            .name
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string(),
        image: config.image.unwrap_or_default(),
        state: state.status.map(|s| s.to_string()).unwrap_or_default(),
        health: state.health.and_then(|h| h.status).map(|s| s.to_string()),
        created: info.created.unwrap_or_default(),
        restart_policy: host_config
            .restart_policy
            .and_then(|p| p.name)
            .map(|n| n.to_string())
            .unwrap_or_default(),
        env: config.env.unwrap_or_default(),
        labels: config.labels.unwrap_or_default(),
        mounts,
        ports,
    })
}

/// One line of container log output, tagged by which stream it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub stream: String,
    pub message: String,
}

impl From<LogOutput> for LogLine {
    fn from(output: LogOutput) -> Self {
        let (stream, bytes) = match output {
            LogOutput::StdOut { message } => ("stdout", message),
            LogOutput::StdErr { message } => ("stderr", message),
            LogOutput::StdIn { message } => ("stdin", message),
            LogOutput::Console { message } => ("console", message),
        };
        LogLine {
            stream: stream.to_string(),
            message: String::from_utf8_lossy(&bytes)
                .trim_end_matches('\n')
                .to_string(),
        }
    }
}

/// Follow a container's stdout/stderr, backfilling the last 500 lines first.
pub fn stream_logs(
    docker: &Docker,
    container_id: &str,
) -> impl Stream<Item = Result<LogLine, String>> + Send {
    let opts = LogsOptionsBuilder::new()
        .follow(true)
        .stdout(true)
        .stderr(true)
        .tail("500")
        .build();
    docker
        .logs(container_id, Some(opts))
        .map(|item| item.map(LogLine::from).map_err(|e| e.to_string()))
}

/// The write half of an attached exec session — one keystroke/paste at a
/// time from the frontend's xterm instance.
pub type ExecInput = Pin<Box<dyn AsyncWrite + Send>>;

/// A raw chunk of exec output, decoded as (possibly lossy) UTF-8.
///
/// Unlike [`LogLine`], this is *not* split or trimmed on newlines: with a TTY
/// attached, bollard's decoder hands back whatever bytes it just read off the
/// wire, and a terminal emulator needs those verbatim — including bare `\r`,
/// mid-escape-sequence fragments, and prompts with no trailing newline at
/// all — to render cursor movement, color, and line wrapping correctly.
fn exec_output_text(output: LogOutput) -> String {
    let bytes = match output {
        LogOutput::StdOut { message }
        | LogOutput::StdErr { message }
        | LogOutput::StdIn { message }
        | LogOutput::Console { message } => message,
    };
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Create an exec instance running `shell` with a TTY attached, and start it.
/// Returns the exec ID (used to key the session on the frontend/state side),
/// a stream of raw output chunks, and the writer for stdin.
pub async fn start_exec(
    docker: &Docker,
    container_id: &str,
    shell: &str,
) -> Result<
    (
        String,
        Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>,
        ExecInput,
    ),
    String,
> {
    let create_opts = CreateExecOptions {
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(vec![shell.to_string()]),
        ..Default::default()
    };
    let created = docker
        .create_exec(container_id, create_opts)
        .await
        .map_err(|e| e.to_string())?;

    let start_opts = StartExecOptions {
        tty: true,
        ..Default::default()
    };
    match docker
        .start_exec(&created.id, Some(start_opts))
        .await
        .map_err(|e| e.to_string())?
    {
        StartExecResults::Attached { output, input } => {
            let chunks = output.map(|item| item.map(exec_output_text).map_err(|e| e.to_string()));
            Ok((created.id, Box::pin(chunks), input))
        }
        StartExecResults::Detached => Err("exec started detached unexpectedly".to_string()),
    }
}

/// Resize the TTY of a running exec session (in character cells).
pub async fn resize_exec(
    docker: &Docker,
    exec_id: &str,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let opts = ResizeExecOptionsBuilder::new()
        .w(cols as i32)
        .h(rows as i32)
        .build();
    docker
        .resize_exec(exec_id, opts)
        .await
        .map_err(|e| e.to_string())
}
