//! Saved Docker host connections. Name/endpoint live in a JSON file in the
//! app config dir; any secret material (TLS client key, issue #7) lives only
//! in the OS keyring (Windows Credential Manager / macOS Keychain / Secret
//! Service) — never on disk, never logged.
//!
//! Same pattern as proxmox-desktop's `connections.rs`, refactored to take the
//! store directory as a parameter so the file store is testable without a
//! Tauri `AppHandle`.

use serde::{Deserialize, Serialize};
use std::fs;
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

fn store_file(dir: &Path) -> PathBuf {
    dir.join("connections.json")
}

pub fn load(dir: &Path) -> Result<Vec<ConnectionInfo>, String> {
    let path = store_file(dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_all(dir: &Path, conns: &[ConnectionInfo]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(conns).map_err(|e| e.to_string())?;
    fs::write(store_file(dir), raw).map_err(|e| e.to_string())
}

pub fn get(dir: &Path, id: &str) -> Result<ConnectionInfo, String> {
    load(dir)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("unknown connection: {id}"))
}

fn secret_entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, id).map_err(|e| e.to_string())
}

pub fn get_secret(id: &str) -> Result<String, String> {
    secret_entry(id)?.get_password().map_err(|e| e.to_string())
}

/// Upsert a connection; `secret` is written to the keyring when provided.
pub fn save(dir: &Path, info: ConnectionInfo, secret: Option<String>) -> Result<(), String> {
    if let Some(s) = secret {
        secret_entry(&info.id)?
            .set_password(&s)
            .map_err(|e| e.to_string())?;
    }
    let mut conns = load(dir)?;
    match conns.iter_mut().find(|c| c.id == info.id) {
        Some(existing) => *existing = info,
        None => conns.push(info),
    }
    save_all(dir, &conns)
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    // Best effort — the entry may already be gone.
    if let Ok(entry) = secret_entry(id) {
        let _ = entry.delete_credential();
    }
    let mut conns = load(dir)?;
    conns.retain(|c| c.id != id);
    save_all(dir, &conns)
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
        fs::write(
            store_file(dir.path()),
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
