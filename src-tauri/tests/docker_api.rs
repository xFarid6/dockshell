//! Integration tests against a mock Docker Engine API (wiremock), same
//! approach as proxmox-desktop's `mock_api.rs`. No Docker install needed.

use dockshell_lib::connections::ConnectionInfo;
use dockshell_lib::docker;
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn conn_for(server: &MockServer) -> ConnectionInfo {
    ConnectionInfo {
        id: "test".into(),
        name: "mock engine".into(),
        endpoint: server.uri(),
        use_tls: false,
        client_cert_path: None,
        ca_cert_path: None,
    }
}

#[tokio::test]
async fn lists_containers_from_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Id": "abc123",
                "Names": ["/portainer"],
                "Image": "portainer/portainer-ce:latest",
                "State": "running",
                "Status": "Up 3 days",
                "Ports": [{"PrivatePort": 9000, "PublicPort": 9000, "Type": "tcp"}]
            }
        ])))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let containers = docker::list_containers(&client).await.unwrap();

    assert_eq!(containers.len(), 1);
    let c = &containers[0];
    assert_eq!(c.id, "abc123");
    assert_eq!(c.name, "portainer"); // leading slash stripped
    assert_eq!(c.image, "portainer/portainer-ce:latest");
    assert_eq!(c.state, "running");
    assert_eq!(c.ports, vec!["9000:9000"]);
}

#[tokio::test]
async fn container_start_stop_restart() {
    let server = MockServer::start().await;
    for verb in ["start", "stop", "restart"] {
        Mock::given(method("POST"))
            .and(path_regex(format!(
                r"^(/v[0-9.]+)?/containers/abc123/{verb}$"
            )))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
    }

    let client = docker::client_for(&conn_for(&server)).unwrap();
    for action in ["start", "stop", "restart"] {
        docker::container_action(&client, "abc123", action)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn unknown_action_is_rejected_without_a_request() {
    let server = MockServer::start().await;
    let client = docker::client_for(&conn_for(&server)).unwrap();
    assert!(docker::container_action(&client, "abc123", "explode")
        .await
        .is_err());
}

#[tokio::test]
async fn ping_reports_engine_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/version$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Version": "27.3.1",
            "ApiVersion": "1.47"
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let msg = docker::ping(&client).await.unwrap();
    assert!(msg.contains("27.3.1"));
}

/// Docker's log endpoint multiplexes stdout/stderr with an 8-byte header per
/// frame: [stream type, 0, 0, 0, big-endian u32 length], then that many
/// payload bytes. See bollard's `NewlineLogOutputDecoder`.
fn log_frame(stream_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + payload.len());
    buf.push(stream_type);
    buf.extend_from_slice(&[0, 0, 0]);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

#[tokio::test]
async fn streams_container_logs_from_engine_api() {
    let server = MockServer::start().await;

    let mut body = log_frame(1, b"starting up\n");
    body.extend(log_frame(2, b"a warning\n"));

    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/abc123/logs$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let lines: Vec<_> = docker::stream_logs(&client, "abc123")
        .take(2)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|line| line.unwrap())
        .collect();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].stream, "stdout");
    assert_eq!(lines[0].message, "starting up");
    assert_eq!(lines[1].stream, "stderr");
    assert_eq!(lines[1].message, "a warning");
}

/// A container can write arbitrary bytes to stdout (binary output, a
/// truncated multi-byte UTF-8 sequence, etc.) — the log line must render as
/// the lossy replacement character rather than dropping the line or
/// panicking on the invalid sequence.
#[tokio::test]
async fn handles_invalid_utf8_in_log_message() {
    let server = MockServer::start().await;

    // 0xFF and 0xFE are never valid UTF-8 bytes on their own.
    let body = log_frame(1, &[0xFF, 0xFE, b'o', b'k', b'\n']);

    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/abc123/logs$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let lines: Vec<_> = docker::stream_logs(&client, "abc123")
        .take(1)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|line| line.unwrap())
        .collect();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].stream, "stdout");
    // Invalid bytes become U+FFFD; the trailing "ok" survives intact.
    assert!(lines[0].message.contains('\u{FFFD}'));
    assert!(lines[0].message.ends_with("ok"));
}

#[tokio::test]
async fn creates_and_starts_a_container_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/create$"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "Id": "newid123",
            "Warnings": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/newid123/start$"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let ports = vec![docker::PortMapping {
        host: "8080".into(),
        container: "80".into(),
    }];
    let env = vec!["FOO=bar".to_string()];
    let id = docker::create_and_start_container(&client, "nginx:latest", Some("web"), &ports, &env)
        .await
        .unwrap();
    assert_eq!(id, "newid123");

    let requests = server.received_requests().await.unwrap();
    let create_req = requests
        .iter()
        .find(|r| r.url.path().ends_with("/containers/create"))
        .expect("create request recorded");
    assert!(create_req
        .url
        .query()
        .unwrap_or_default()
        .contains("name=web"));

    let body: serde_json::Value = serde_json::from_slice(&create_req.body).unwrap();
    assert_eq!(body["Image"], "nginx:latest");
    assert_eq!(body["Env"], serde_json::json!(["FOO=bar"]));
    assert!(body["ExposedPorts"]["80/tcp"].is_object());
    let bindings = &body["HostConfig"]["PortBindings"]["80/tcp"][0];
    assert_eq!(bindings["HostPort"], "8080");

    let start_req = requests
        .iter()
        .find(|r| r.url.path().ends_with("/containers/newid123/start"));
    assert!(start_req.is_some());
}

#[tokio::test]
async fn inspects_container_from_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/abc123/json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Id": "abc123",
            "Created": "2026-07-01T12:00:00Z",
            "Name": "/portainer",
            "Image": "sha256:deadbeef",
            "State": {
                "Status": "running",
                "Health": { "Status": "healthy" }
            },
            "HostConfig": {
                "RestartPolicy": { "Name": "unless-stopped" }
            },
            "Mounts": [
                { "Source": "/data", "Destination": "/var/lib/portainer", "Mode": "rw" }
            ],
            "Config": {
                "Image": "portainer/portainer-ce:latest",
                "Env": ["FOO=bar"],
                "Labels": { "com.example": "1" }
            },
            "NetworkSettings": {
                "Ports": {
                    "9000/tcp": [{ "HostIp": "0.0.0.0", "HostPort": "9000" }]
                }
            }
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let detail = docker::inspect_container(&client, "abc123").await.unwrap();

    assert_eq!(detail.id, "abc123");
    assert_eq!(detail.name, "portainer"); // leading slash stripped
    assert_eq!(detail.image, "portainer/portainer-ce:latest");
    assert_eq!(detail.state, "running");
    assert_eq!(detail.health.as_deref(), Some("healthy"));
    assert_eq!(detail.restart_policy, "unless-stopped");
    assert_eq!(detail.env, vec!["FOO=bar".to_string()]);
    assert_eq!(detail.labels.get("com.example"), Some(&"1".to_string()));
    assert_eq!(detail.mounts.len(), 1);
    assert_eq!(detail.mounts[0].source, "/data");
    assert_eq!(detail.mounts[0].destination, "/var/lib/portainer");
    assert_eq!(detail.mounts[0].mode, "rw");
    assert_eq!(detail.ports.len(), 1);
    assert_eq!(detail.ports[0].container_port, "9000/tcp");
    assert_eq!(detail.ports[0].host_ip, "0.0.0.0");
    assert_eq!(detail.ports[0].host_port, "9000");
    assert!(detail.created.starts_with("2026-07-01"));
}

#[tokio::test]
async fn lists_images_from_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/images/json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Id": "sha256:abc123",
                "ParentId": "",
                "RepoTags": ["alpine:latest"],
                "RepoDigests": [],
                "Created": 1751328000i64,
                "Size": 7500000i64,
                "SharedSize": 0,
                "Labels": {},
                "Containers": -1
            }
        ])))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let images = docker::list_images(&client).await.unwrap();

    assert_eq!(images.len(), 1);
    assert_eq!(images[0].id, "sha256:abc123");
    assert_eq!(images[0].tags, vec!["alpine:latest".to_string()]);
    assert_eq!(images[0].size, 7500000);
    assert_eq!(images[0].created, 1751328000);
}

#[tokio::test]
async fn removes_an_image_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path_regex(r"^(/v[0-9.]+)?/images/alpine:latest$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "Deleted": "sha256:abc123" }
        ])))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    docker::remove_image(&client, "alpine:latest", false)
        .await
        .unwrap();
}

#[tokio::test]
async fn prunes_containers_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/prune$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ContainersDeleted": ["abc123", "def456"],
            "SpaceReclaimed": 4096i64
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let result = docker::prune_containers(&client).await.unwrap();
    assert_eq!(
        result.deleted,
        vec!["abc123".to_string(), "def456".to_string()]
    );
    assert_eq!(result.space_reclaimed, Some(4096));
}

#[tokio::test]
async fn prunes_images_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/images/prune$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ImagesDeleted": [{ "Deleted": "sha256:abc123" }, { "Untagged": "old:tag" }],
            "SpaceReclaimed": 8192i64
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let result = docker::prune_images(&client).await.unwrap();
    assert_eq!(
        result.deleted,
        vec!["sha256:abc123".to_string(), "old:tag".to_string()]
    );
    assert_eq!(result.space_reclaimed, Some(8192));
}

#[tokio::test]
async fn prunes_volumes_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/volumes/prune$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "VolumesDeleted": ["data"],
            "SpaceReclaimed": 1024i64
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let result = docker::prune_volumes(&client).await.unwrap();
    assert_eq!(result.deleted, vec!["data".to_string()]);
    assert_eq!(result.space_reclaimed, Some(1024));
}

/// Networks have no `SpaceReclaimed` field in the Engine API response —
/// there's no disk space to reclaim by removing a network.
#[tokio::test]
async fn prunes_networks_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/networks/prune$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "NetworksDeleted": ["custom-net"]
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let result = docker::prune_networks(&client).await.unwrap();
    assert_eq!(result.deleted, vec!["custom-net".to_string()]);
    assert_eq!(result.space_reclaimed, None);
}

/// The Engine API streams `/images/create` as newline-delimited JSON objects
/// (bollard decodes with `JsonLineDecoder`), one per progress update.
#[tokio::test]
async fn streams_pull_progress_from_engine_api() {
    let server = MockServer::start().await;
    let body = concat!(
        r#"{"status":"Pulling from library/alpine","id":"latest"}"#,
        "\n",
        r#"{"status":"Downloading","progress":"[====>   ] 1MB/2MB","id":"a1b2c3"}"#,
        "\n",
    );

    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/images/create$"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let updates: Vec<_> = docker::pull_image(&client, "alpine:latest")
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|u| u.unwrap())
        .collect();

    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].status, "Pulling from library/alpine");
    assert_eq!(updates[0].id.as_deref(), Some("latest"));
    assert_eq!(updates[1].status, "Downloading");
    assert_eq!(updates[1].progress.as_deref(), Some("[====>   ] 1MB/2MB"));
    assert_eq!(updates[1].id.as_deref(), Some("a1b2c3"));
}

#[tokio::test]
async fn streams_container_events_from_engine_api() {
    let server = MockServer::start().await;
    let body = concat!(
        r#"{"Type":"container","Action":"start","Actor":{"ID":"abc123","Attributes":{"name":"web"}},"time":1}"#,
        "\n",
        r#"{"Type":"container","Action":"die","Actor":{"ID":"def456","Attributes":{"name":"db"}},"time":2}"#,
        "\n",
    );

    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/events$"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let events: Vec<_> = docker::stream_container_events(&client)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|e| e.unwrap())
        .collect();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].action, "start");
    assert_eq!(events[0].container_id, "abc123");
    assert_eq!(events[0].container_name.as_deref(), Some("web"));
    assert_eq!(events[1].action, "die");
    assert_eq!(events[1].container_id, "def456");

    let requests = server.received_requests().await.unwrap();
    let events_req = requests
        .iter()
        .find(|r| r.url.path().ends_with("/events"))
        .expect("events request recorded");
    let query = events_req.url.query().unwrap_or_default();
    assert!(query.contains("filters="));
}

#[tokio::test]
async fn lists_volumes_and_cross_references_container_mounts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/volumes$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Volumes": [
                {
                    "Name": "data",
                    "Driver": "local",
                    "Mountpoint": "/var/lib/docker/volumes/data/_data",
                    "CreatedAt": "2026-07-01T12:00:00Z",
                    "Labels": {},
                    "Options": {},
                    "Scope": "local"
                },
                {
                    "Name": "unused",
                    "Driver": "local",
                    "Mountpoint": "/var/lib/docker/volumes/unused/_data",
                    "CreatedAt": "2026-07-02T12:00:00Z",
                    "Labels": {},
                    "Options": {},
                    "Scope": "local"
                }
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/containers/json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Id": "abc123",
                "Names": ["/portainer"],
                "Image": "portainer/portainer-ce:latest",
                "State": "running",
                "Status": "Up 3 days",
                "Mounts": [
                    { "Type": "volume", "Name": "data", "Source": "/var/lib/docker/volumes/data/_data" },
                    { "Type": "bind", "Source": "/etc/hosts" }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let volumes = docker::list_volumes(&client).await.unwrap();

    assert_eq!(volumes.len(), 2);
    let data = volumes.iter().find(|v| v.name == "data").unwrap();
    assert_eq!(data.driver, "local");
    assert_eq!(data.mountpoint, "/var/lib/docker/volumes/data/_data");
    assert_eq!(data.used_by, vec!["portainer".to_string()]);
    let unused = volumes.iter().find(|v| v.name == "unused").unwrap();
    assert!(unused.used_by.is_empty());
}

#[tokio::test]
async fn removes_a_volume_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path_regex(r"^(/v[0-9.]+)?/volumes/data$"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    docker::remove_volume(&client, "data").await.unwrap();
}

#[tokio::test]
async fn remove_volume_surfaces_in_use_conflict() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path_regex(r"^(/v[0-9.]+)?/volumes/data$"))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "message": "volume is in use - [abc123]"
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let err = docker::remove_volume(&client, "data").await.unwrap_err();
    assert!(err.contains("in use"));
}

#[tokio::test]
async fn resizes_exec_tty_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"^(/v[0-9.]+)?/exec/exec123/resize$"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    docker::resize_exec(&client, "exec123", 120, 40)
        .await
        .unwrap();
}

/// wiremock can't hijack a connection the way a real HTTP upgrade needs, so
/// exercising `start_exec`'s attach flow means speaking raw HTTP ourselves:
/// reply to `POST .../containers/{id}/exec` with a fixed exec ID, then to
/// `POST .../exec/{id}/start` with a 101 upgrade followed by the raw
/// (non-multiplexed, since `tty: true`) output bytes a real engine would
/// send.
async fn spawn_fake_exec_daemon(exec_id: &'static str, raw_output: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let request_line = request.lines().next().unwrap_or_default();

                if request_line.contains("/start") {
                    let header = "HTTP/1.1 101 UPGRADED\r\n\
                         Content-Type: application/vnd.docker.raw-stream\r\n\
                         Connection: Upgrade\r\n\
                         Upgrade: tcp\r\n\r\n";
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(raw_output).await;
                } else {
                    let body = format!("{{\"Id\":\"{exec_id}\"}}");
                    let response = format!(
                        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            });
        }
    });

    format!("http://{addr}")
}

#[tokio::test]
async fn streams_tty_exec_output_over_the_upgraded_connection() {
    // No trailing newline, like a shell prompt — a terminal must still see
    // it immediately rather than have it withheld pending a newline that
    // may never come.
    let endpoint = spawn_fake_exec_daemon("exec123", b"/ # ").await;
    let conn = ConnectionInfo {
        id: "test".into(),
        name: "fake exec daemon".into(),
        endpoint,
        use_tls: false,
        client_cert_path: None,
        ca_cert_path: None,
    };
    let client = docker::client_for(&conn).unwrap();

    let (exec_id, mut output, _input) = docker::start_exec(&client, "abc123", "/bin/sh")
        .await
        .unwrap();
    assert_eq!(exec_id, "exec123");

    let chunk = output.next().await.unwrap().unwrap();
    assert_eq!(chunk, "/ # ");
}

#[tokio::test]
async fn lists_networks_with_attachments_from_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/networks$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "Id": "b1",
                "Name": "bridge",
                "Driver": "bridge",
                "Scope": "local",
                "IPAM": { "Config": [{ "Subnet": "172.17.0.0/16" }] }
            },
            {
                "Id": "n1",
                "Name": "app-net",
                "Driver": "bridge",
                "Scope": "local",
                "IPAM": { "Config": [{ "Subnet": "172.20.0.0/16" }] }
            }
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/networks/bridge$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Id": "b1",
            "Name": "bridge",
            "Driver": "bridge",
            "Scope": "local",
            "Containers": {}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^(/v[0-9.]+)?/networks/app-net$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Id": "n1",
            "Name": "app-net",
            "Driver": "bridge",
            "Scope": "local",
            "Containers": {
                "abc123": { "Name": "portainer", "IPv4Address": "172.20.0.2/16" }
            }
        })))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    let networks = docker::list_networks(&client).await.unwrap();

    assert_eq!(networks.len(), 2);
    let bridge = networks.iter().find(|n| n.name == "bridge").unwrap();
    assert!(bridge.is_builtin);
    assert!(bridge.attachments.is_empty());
    let app_net = networks.iter().find(|n| n.name == "app-net").unwrap();
    assert!(!app_net.is_builtin);
    assert_eq!(app_net.subnet, "172.20.0.0/16");
    assert_eq!(app_net.attachments.len(), 1);
    assert_eq!(app_net.attachments[0].container, "portainer");
    assert_eq!(app_net.attachments[0].ip, "172.20.0.2/16");
}

#[tokio::test]
async fn removes_a_network_via_engine_api() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path_regex(r"^(/v[0-9.]+)?/networks/app-net$"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = docker::client_for(&conn_for(&server)).unwrap();
    docker::remove_network(&client, "app-net").await.unwrap();
}

#[tokio::test]
async fn remove_network_rejects_a_builtin_network_without_calling_the_engine() {
    // No mocks registered — a call reaching the (mock) server would 404 and
    // this would fail, proving the built-in guard runs before any request.
    let server = MockServer::start().await;
    let client = docker::client_for(&conn_for(&server)).unwrap();

    let err = docker::remove_network(&client, "bridge").await.unwrap_err();
    assert!(err.contains("bridge"));
}
