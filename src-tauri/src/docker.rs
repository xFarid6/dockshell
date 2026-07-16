//! Thin wrapper around bollard: build a client from a saved connection and
//! expose the few operations the scaffold needs (list, start/stop/restart).

use bollard::query_parameters::{
    ListContainersOptionsBuilder, RestartContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::Docker;
use serde::Serialize;

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
