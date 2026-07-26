//! Saved Docker host connections. Thin adapter over the shared
//! `conn-manager` crate (issue #14): name/endpoint live in a JSON file in the
//! app config dir; any secret material (TLS client key, issue #7) lives only
//! in the OS keyring (Windows Credential Manager / macOS Keychain / Secret
//! Service) — never on disk, never logged.

use conn_manager::{Profile, ProfileStore};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "dockshell";

/// One saved connection = one Docker engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    /// `"local"` for the platform-default socket (named pipe on Windows,
    /// `/var/run/docker.sock` elsewhere), or a `tcp://host:port` URL for a
    /// remote engine.
    pub endpoint: String,
    /// When set, connect over mutual TLS: the client key PEM lives in the
    /// keyring (see `save`'s `secret` param), `client_cert_path` and
    /// `ca_cert_path` are plain paths to files already on disk.
    pub use_tls: bool,
    #[serde(default)]
    pub client_cert_path: Option<String>,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
}

impl Profile for ConnectionInfo {
    fn id(&self) -> &str {
        &self.id
    }
}

fn store(dir: &Path) -> ProfileStore {
    ProfileStore::new(dir.to_path_buf(), KEYRING_SERVICE)
}

pub fn load(dir: &Path) -> Result<Vec<ConnectionInfo>, String> {
    store(dir).load()
}

pub fn get(dir: &Path, id: &str) -> Result<ConnectionInfo, String> {
    store(dir).get(id)
}

/// `get_secret` needs no profile-store directory — the secret lives only in
/// the keyring — so it builds a `ProfileStore` with an unused, never-read dir.
pub fn get_secret(id: &str) -> Result<String, String> {
    store(&PathBuf::new()).get_secret(id)
}

/// Upsert a connection; `secret` is written to the keyring when provided.
pub fn save(dir: &Path, info: ConnectionInfo, secret: Option<String>) -> Result<(), String> {
    store(dir).save(info, secret)
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    store(dir).delete::<ConnectionInfo>(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(id: &str) -> ConnectionInfo {
        ConnectionInfo {
            id: id.into(),
            name: format!("conn {id}"),
            endpoint: "tcp://192.168.1.105:2375".into(),
            use_tls: false,
            client_cert_path: None,
            ca_cert_path: None,
        }
    }

    #[test]
    fn save_load_delete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), vec![]);

        save(dir.path(), conn("a"), None).unwrap();
        save(dir.path(), conn("b"), None).unwrap();
        assert_eq!(load(dir.path()).unwrap().len(), 2);
        assert_eq!(get(dir.path(), "a").unwrap().name, "conn a");

        // Upsert replaces, not duplicates.
        let mut edited = conn("a");
        edited.name = "renamed".into();
        save(dir.path(), edited, None).unwrap();
        let conns = load(dir.path()).unwrap();
        assert_eq!(conns.len(), 2);
        assert_eq!(get(dir.path(), "a").unwrap().name, "renamed");

        delete(dir.path(), "a").unwrap();
        assert_eq!(load(dir.path()).unwrap().len(), 1);
        assert!(get(dir.path(), "a").is_err());
    }

    #[test]
    fn unknown_connection_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(get(dir.path(), "nope").is_err());
    }

    #[test]
    fn tls_fields_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = conn("a");
        c.use_tls = true;
        c.client_cert_path = Some("/certs/cert.pem".into());
        c.ca_cert_path = Some("/certs/ca.pem".into());
        save(dir.path(), c.clone(), None).unwrap();
        assert_eq!(get(dir.path(), "a").unwrap(), c);
    }

    /// An existing `connections.json` written before TLS support (issue #7)
    /// has no `clientCertPath`/`caCertPath` keys at all — it must still load,
    /// defaulting the new fields to `None`, with no migration step.
    #[test]
    fn pre_tls_connections_json_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("connections.json"),
            r#"[{"id":"a","name":"old","endpoint":"tcp://192.168.1.105:2375","useTls":false}]"#,
        )
        .unwrap();
        let loaded = get(dir.path(), "a").unwrap();
        assert_eq!(loaded.client_cert_path, None);
        assert_eq!(loaded.ca_cert_path, None);
    }

    // Real OS keyring roundtrip. Ignored in CI (headless ubuntu has no Secret
    // Service); run locally with `cargo test -- --ignored`.
    #[test]
    #[ignore = "requires a real OS keyring; run locally with --ignored"]
    fn keyring_secret_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let id = "dockshell-test-keyring";
        save(dir.path(), conn(id), Some("s3cret".into())).unwrap();
        assert_eq!(get_secret(id).unwrap(), "s3cret");
        delete(dir.path(), id).unwrap();
        assert!(get_secret(id).is_err());
    }
}
