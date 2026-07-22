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
    };
    let client = docker::client_for(&conn).unwrap();

    let (exec_id, mut output, _input) = docker::start_exec(&client, "abc123", "/bin/sh")
        .await
        .unwrap();
    assert_eq!(exec_id, "exec123");

    let chunk = output.next().await.unwrap().unwrap();
    assert_eq!(chunk, "/ # ");
}
