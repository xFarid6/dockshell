//! Local `docker compose` CLI passthrough (issue #6).
//!
//! The Docker Engine API has no compose endpoints — compose is a
//! client-side concept — so rather than parse compose files and create
//! containers ourselves (a much larger project), this shells out to the
//! `docker compose` CLI. That only works when the daemon is reachable via
//! the local CLI, so remote connection profiles are rejected up front with
//! a clear message; full compose-file parsing is a possible follow-up.

use std::process::Stdio;

use serde::Serialize;
use tokio::process::{Child, Command};

use crate::connections::ConnectionInfo;

/// One line of `docker compose` output, tagged by which stream it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeLine {
    pub stream: String,
    pub message: String,
}

/// Compose only works against the engine the local CLI itself talks to —
/// there's no way to point `docker compose` at an arbitrary remote host.
pub fn ensure_local(info: &ConnectionInfo) -> Result<(), String> {
    if info.endpoint != "local" {
        return Err("compose requires a local connection".to_string());
    }
    Ok(())
}

fn build_compose_command(file: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("docker");
    cmd.arg("compose").arg("-f").arg(file).args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    cmd
}

/// Spawn `docker compose -f <file> <args>` with stdout/stderr piped so the
/// caller can forward output as it arrives.
pub fn spawn_compose(file: &str, args: &[&str]) -> Result<Child, String> {
    build_compose_command(file, args)
        .spawn()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn builds_the_up_command() {
        let cmd = build_compose_command("/stacks/app.yml", &["up", "-d"]);
        assert_eq!(cmd.as_std().get_program(), "docker");
        assert_eq!(
            args_of(&cmd),
            vec!["compose", "-f", "/stacks/app.yml", "up", "-d"]
        );
    }

    #[test]
    fn builds_the_down_command() {
        let cmd = build_compose_command("/stacks/app.yml", &["down"]);
        assert_eq!(
            args_of(&cmd),
            vec!["compose", "-f", "/stacks/app.yml", "down"]
        );
    }

    fn conn(endpoint: &str) -> ConnectionInfo {
        ConnectionInfo {
            id: "x".into(),
            name: "test".into(),
            endpoint: endpoint.into(),
            use_tls: false,
            client_cert_path: None,
            ca_cert_path: None,
        }
    }

    #[test]
    fn rejects_a_remote_connection() {
        let err = ensure_local(&conn("tcp://192.168.1.105:2375")).unwrap_err();
        assert!(err.contains("local connection"));
    }

    #[test]
    fn accepts_the_local_connection() {
        assert!(ensure_local(&conn("local")).is_ok());
    }
}
