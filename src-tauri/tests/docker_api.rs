//! Integration tests against a mock Docker Engine API (wiremock), same
//! approach as proxmox-desktop's `mock_api.rs`. No Docker install needed.

use dockshell_lib::connections::ConnectionInfo;
use dockshell_lib::docker;
use futures_util::StreamExt;
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
