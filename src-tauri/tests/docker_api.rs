//! Integration tests against a mock Docker Engine API (wiremock), same
//! approach as proxmox-desktop's `mock_api.rs`. No Docker install needed.

use dockshell_lib::connections::ConnectionInfo;
use dockshell_lib::docker;
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
