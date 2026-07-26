//! App-wide settings (theme, refresh interval). Persisted as `settings.json`
//! in the app config dir, same load/save pattern as `connections.rs` — no
//! keyring involved, nothing here is secret.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// "dark" | "light" | "system".
    pub theme: String,
    pub refresh_interval_secs: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".to_string(),
            refresh_interval_secs: 10,
        }
    }
}

fn store_file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &Path) -> Result<Settings, String> {
    let path = store_file(dir);
    if !path.exists() {
        return Ok(Settings::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save(dir: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(store_file(dir), raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), Settings::default());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            theme: "light".to_string(),
            refresh_interval_secs: 30,
        };
        save(dir.path(), &settings).unwrap();
        assert_eq!(load(dir.path()).unwrap(), settings);
    }
}
