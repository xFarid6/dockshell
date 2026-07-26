//! Thin wrapper around bollard: build a client from a saved connection and
//! expose the few operations the scaffold needs (list, start/stop/restart).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::models::MountPointTypeEnum;
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding as ModelPortBinding};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, EventsOptionsBuilder,
    InspectContainerOptions, InspectNetworkOptionsBuilder, ListContainersOptionsBuilder,
    ListImagesOptionsBuilder, ListNetworksOptionsBuilder, ListVolumesOptionsBuilder,
    LogsOptionsBuilder, PruneContainersOptionsBuilder, PruneImagesOptionsBuilder,
    PruneNetworksOptionsBuilder, PruneVolumesOptionsBuilder, RemoveImageOptionsBuilder,
    RemoveVolumeOptionsBuilder, ResizeExecOptionsBuilder, RestartContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWrite;

use crate::connections::{self, ConnectionInfo};

/// The client key PEM lives in the keyring, but bollard's TLS client-cert
/// resolver reads it from a file path on every handshake (`pool_max_idle_per_host(0)`
/// means that can be more than once per connection) — so we materialize it
/// to a private per-connection file each time we connect.
// ponytail: decrypted key cached on disk under the OS user profile rather
// than held only in memory; acceptable for v1 (no weaker than the ~/.docker
// convention Docker's own CLI uses) — revisit if bollard grows an in-memory
// client-cert resolver.
fn materialize_key_file(connection_id: &str) -> Result<PathBuf, String> {
    let key_pem = connections::get_secret(connection_id)
        .map_err(|e| format!("no client key stored for this connection: {e}"))?;
    let dir = std::env::temp_dir().join("dockshell-tls");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{connection_id}.pem"));
    fs::write(&path, key_pem).map_err(|e| e.to_string())?;
    Ok(path)
}

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
        return Docker::connect_with_local_defaults().map_err(|e| e.to_string());
    }
    if info.use_tls {
        let cert_path = info
            .client_cert_path
            .as_deref()
            .ok_or("TLS is enabled but no client certificate path is set")?;
        let ca_path = info
            .ca_cert_path
            .as_deref()
            .ok_or("TLS is enabled but no CA certificate path is set")?;
        let key_path = materialize_key_file(&info.id)?;
        return Docker::connect_with_ssl(
            &info.endpoint,
            &key_path,
            Path::new(cert_path),
            Path::new(ca_path),
            10,
            bollard::API_DEFAULT_VERSION,
        )
        .map_err(|e| e.to_string());
    }
    // tcp:// and http:// both accepted.
    Docker::connect_with_http(&info.endpoint, 10, bollard::API_DEFAULT_VERSION)
        .map_err(|e| e.to_string())
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

/// A host:container port mapping from the create/run dialog.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host: String,
    pub container: String,
}

/// Create a container from `image` and start it. `ports` maps host ports to
/// container ports (TCP only in v1); `env` is `KEY=VALUE` pairs. Returns the
/// new container's ID.
pub async fn create_and_start_container(
    docker: &Docker,
    image: &str,
    name: Option<&str>,
    ports: &[PortMapping],
    env: &[String],
) -> Result<String, String> {
    let mut exposed_ports = HashMap::new();
    let mut port_bindings = HashMap::new();
    for p in ports {
        let container_port = format!("{}/tcp", p.container);
        exposed_ports.insert(container_port.clone(), HashMap::new());
        port_bindings.insert(
            container_port,
            Some(vec![ModelPortBinding {
                host_ip: None,
                host_port: Some(p.host.clone()),
            }]),
        );
    }

    let body = ContainerCreateBody {
        image: Some(image.to_string()),
        env: Some(env.to_vec()),
        exposed_ports: Some(exposed_ports),
        host_config: Some(HostConfig {
            port_bindings: Some(port_bindings),
            ..Default::default()
        }),
        ..Default::default()
    };

    let opts = CreateContainerOptionsBuilder::new();
    let opts = match name {
        Some(n) => opts.name(n),
        None => opts,
    }
    .build();

    let created = docker
        .create_container(Some(opts), body)
        .await
        .map_err(|e| e.to_string())?;
    docker
        .start_container(&created.id, None::<StartContainerOptions>)
        .await
        .map_err(|e| e.to_string())?;
    Ok(created.id)
}

/// Container lifecycle actions the events stream forwards to the frontend;
/// other event types (image, network, volume, ...) are filtered out engine-side.
const CONTAINER_EVENT_ACTIONS: &[&str] = &[
    "start",
    "stop",
    "die",
    "destroy",
    "create",
    "rename",
    "health_status",
];

/// A container lifecycle event, forwarded to the frontend so the table can
/// refresh itself instead of waiting for a manual Refresh click.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerEvent {
    pub action: String,
    pub container_id: String,
    pub container_name: Option<String>,
}

/// Stream container start/stop/die/... events from the Engine's `/events`
/// endpoint until the caller drops the stream (e.g. aborts the task on
/// connection switch).
pub fn stream_container_events(
    docker: &Docker,
) -> impl Stream<Item = Result<ContainerEvent, String>> {
    let mut filters = HashMap::new();
    filters.insert("type".to_string(), vec!["container".to_string()]);
    filters.insert(
        "event".to_string(),
        CONTAINER_EVENT_ACTIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    let opts = EventsOptionsBuilder::new().filters(&filters).build();
    docker.events(Some(opts)).map(|item| {
        item.map_err(|e| e.to_string()).map(|msg| ContainerEvent {
            action: msg.action.unwrap_or_default(),
            container_id: msg
                .actor
                .as_ref()
                .and_then(|a| a.id.clone())
                .unwrap_or_default(),
            container_name: msg
                .actor
                .and_then(|a| a.attributes)
                .and_then(|attrs| attrs.get("name").cloned()),
        })
    })
}

/// What the frontend renders per image row.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

pub async fn list_images(docker: &Docker) -> Result<Vec<ImageInfo>, String> {
    let opts = ListImagesOptionsBuilder::new().build();
    let summaries = docker
        .list_images(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(summaries
        .into_iter()
        .map(|i| ImageInfo {
            id: i.id,
            tags: i.repo_tags,
            size: i.size,
            created: i.created,
        })
        .collect())
}

/// Remove an image by ID or `repo:tag`. `force` matches `docker rmi -f`.
pub async fn remove_image(docker: &Docker, image: &str, force: bool) -> Result<(), String> {
    let opts = RemoveImageOptionsBuilder::new().force(force).build();
    docker
        .remove_image(image, Some(opts), None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// What a prune call removed and how much disk space it freed. Networks
/// don't report reclaimed space (there's nothing to reclaim), so it's `None`
/// there.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneResult {
    pub deleted: Vec<String>,
    pub space_reclaimed: Option<i64>,
}

/// Remove stopped containers.
pub async fn prune_containers(docker: &Docker) -> Result<PruneResult, String> {
    let opts = PruneContainersOptionsBuilder::new().build();
    let r = docker
        .prune_containers(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(PruneResult {
        deleted: r.containers_deleted.unwrap_or_default(),
        space_reclaimed: r.space_reclaimed,
    })
}

/// Remove dangling (untagged) images.
pub async fn prune_images(docker: &Docker) -> Result<PruneResult, String> {
    let opts = PruneImagesOptionsBuilder::new().build();
    let r = docker
        .prune_images(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(PruneResult {
        deleted: r
            .images_deleted
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.deleted.or(d.untagged))
            .collect(),
        space_reclaimed: r.space_reclaimed,
    })
}

/// Remove volumes not referenced by any container.
pub async fn prune_volumes(docker: &Docker) -> Result<PruneResult, String> {
    let opts = PruneVolumesOptionsBuilder::new().build();
    let r = docker
        .prune_volumes(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(PruneResult {
        deleted: r.volumes_deleted.unwrap_or_default(),
        space_reclaimed: r.space_reclaimed,
    })
}

/// Remove networks not used by any container (never touches `bridge`/`host`/`none`).
pub async fn prune_networks(docker: &Docker) -> Result<PruneResult, String> {
    let opts = PruneNetworksOptionsBuilder::new().build();
    let r = docker
        .prune_networks(Some(opts))
        .await
        .map_err(|e| e.to_string())?;
    Ok(PruneResult {
        deleted: r.networks_deleted.unwrap_or_default(),
        space_reclaimed: None,
    })
}

/// One line of `docker pull` progress — layer id plus a human-readable status.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub id: Option<String>,
    pub status: String,
    pub progress: Option<String>,
}

/// Pull an image (`repo:tag` or `repo@digest`), streaming layer-by-layer
/// progress. Registry auth is out of scope for now — public images only.
pub fn pull_image(
    docker: &Docker,
    image: &str,
) -> impl Stream<Item = Result<PullProgress, String>> {
    let opts = CreateImageOptionsBuilder::new().from_image(image).build();
    docker.create_image(Some(opts), None, None).map(|item| {
        item.map(|info| PullProgress {
            id: info.id,
            status: info.status.unwrap_or_default(),
            progress: info.progress,
        })
        .map_err(|e| e.to_string())
    })
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

/// What the frontend renders per volume row.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
    pub labels: HashMap<String, String>,
    /// Names of containers with a volume mount pointing at this volume.
    pub used_by: Vec<String>,
}

/// List volumes, cross-referencing every container's mounts so the UI can
/// show which containers are using each volume without extra requests.
pub async fn list_volumes(docker: &Docker) -> Result<Vec<VolumeInfo>, String> {
    let opts = ListVolumesOptionsBuilder::new().build();
    let volumes = docker
        .list_volumes(Some(opts))
        .await
        .map_err(|e| e.to_string())?
        .volumes
        .unwrap_or_default();

    let containers = docker
        .list_containers(Some(ListContainersOptionsBuilder::new().all(true).build()))
        .await
        .map_err(|e| e.to_string())?;

    let mut used_by: HashMap<String, Vec<String>> = HashMap::new();
    for c in &containers {
        let name = c
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default();
        for m in c.mounts.as_deref().unwrap_or_default() {
            if m.typ == Some(MountPointTypeEnum::VOLUME) {
                if let Some(volume_name) = &m.name {
                    used_by
                        .entry(volume_name.clone())
                        .or_default()
                        .push(name.clone());
                }
            }
        }
    }

    Ok(volumes
        .into_iter()
        .map(|v| VolumeInfo {
            used_by: used_by.remove(&v.name).unwrap_or_default(),
            name: v.name,
            driver: v.driver,
            mountpoint: v.mountpoint,
            created: v.created_at.map(|d| d.to_string()).unwrap_or_default(),
            labels: v.labels,
        })
        .collect())
}

/// Remove a volume by name. Fails cleanly (with the engine's message, e.g.
/// "volume is in use") when containers still reference it.
pub async fn remove_volume(docker: &Docker, name: &str) -> Result<(), String> {
    let opts = RemoveVolumeOptionsBuilder::new().build();
    docker
        .remove_volume(name, Some(opts))
        .await
        .map_err(|e| e.to_string())
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

/// Networks Docker creates on every engine; not user-removable.
fn is_builtin_network(name: &str) -> bool {
    matches!(name, "bridge" | "host" | "none")
}

/// A container attached to a network, from the network's `Containers` map.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAttachment {
    pub container: String,
    pub ip: String,
}

/// What the frontend renders per network row.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub subnet: String,
    pub is_builtin: bool,
    pub attachments: Vec<NetworkAttachment>,
}

/// List networks, inspecting each one to pull its `Containers` map — the
/// list endpoint alone doesn't report attachments.
pub async fn list_networks(docker: &Docker) -> Result<Vec<NetworkInfo>, String> {
    let opts = ListNetworksOptionsBuilder::new().build();
    let networks = docker
        .list_networks(Some(opts))
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(networks.len());
    for n in networks {
        let name = n.name.unwrap_or_default();
        let detail = docker
            .inspect_network(&name, Some(InspectNetworkOptionsBuilder::new().build()))
            .await
            .map_err(|e| e.to_string())?;

        let attachments = detail
            .containers
            .unwrap_or_default()
            .into_values()
            .map(|c| NetworkAttachment {
                container: c.name.unwrap_or_default(),
                ip: c.ipv4_address.unwrap_or_default(),
            })
            .collect();

        let subnet = n
            .ipam
            .as_ref()
            .and_then(|i| i.config.as_ref())
            .and_then(|c| c.first())
            .and_then(|c| c.subnet.clone())
            .unwrap_or_default();

        out.push(NetworkInfo {
            id: n.id.unwrap_or_default(),
            is_builtin: is_builtin_network(&name),
            name,
            driver: n.driver.unwrap_or_default(),
            scope: n.scope.unwrap_or_default(),
            subnet,
            attachments,
        });
    }
    Ok(out)
}

/// Refuse to remove the built-in `bridge`/`host`/`none` networks — the
/// engine allows it but it breaks every container relying on them.
fn ensure_removable(name: &str) -> Result<(), String> {
    if is_builtin_network(name) {
        return Err(format!("cannot remove the built-in \"{name}\" network"));
    }
    Ok(())
}

pub async fn remove_network(docker: &Docker, name: &str) -> Result<(), String> {
    ensure_removable(name)?;
    docker.remove_network(name).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod network_tests {
    use super::*;

    #[test]
    fn rejects_removing_a_builtin_network() {
        for name in ["bridge", "host", "none"] {
            let err = ensure_removable(name).unwrap_err();
            assert!(err.contains(name));
        }
    }

    #[test]
    fn accepts_removing_a_user_network() {
        assert!(ensure_removable("my-net").is_ok());
    }
}

#[cfg(test)]
mod tls_tests {
    use super::*;

    fn tls_conn(cert: Option<&str>, ca: Option<&str>) -> ConnectionInfo {
        ConnectionInfo {
            id: "docker-rs-tls-test".into(),
            name: "remote".into(),
            endpoint: "tcp://192.168.1.105:2376".into(),
            use_tls: true,
            client_cert_path: cert.map(String::from),
            ca_cert_path: ca.map(String::from),
        }
    }

    #[test]
    fn requires_a_client_cert_path() {
        let err = client_for(&tls_conn(None, Some("/ca.pem"))).unwrap_err();
        assert!(err.contains("client certificate path"));
    }

    #[test]
    fn requires_a_ca_cert_path() {
        let err = client_for(&tls_conn(Some("/cert.pem"), None)).unwrap_err();
        assert!(err.contains("CA certificate path"));
    }

    // Exercises the real OS keyring (via connections::get_secret), same as
    // connections::tests::keyring_secret_roundtrip; run locally with --ignored.
    #[test]
    #[ignore = "requires a real OS keyring; run locally with --ignored"]
    fn materializes_the_key_from_the_keyring() {
        let dir = tempfile::tempdir().unwrap();
        let id = "dockshell-test-tls-key";
        let mut conn = tls_conn(Some("/cert.pem"), Some("/ca.pem"));
        conn.id = id.into();
        connections::save(dir.path(), conn, Some("-----BEGIN KEY-----\nabc\n".into())).unwrap();

        let path = materialize_key_file(id).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "-----BEGIN KEY-----\nabc\n"
        );

        connections::delete(dir.path(), id).unwrap();
    }
}
