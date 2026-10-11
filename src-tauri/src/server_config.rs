//! Persistence for the OpenCode server connection settings.
//!
//! The Settings dialog writes `server.json` into the app config directory via
//! the `set_server_config` command; the shell reads it back on startup so the
//! configured server is used without touching environment variables. When the
//! file is absent we fall back to the environment / built-in defaults.

use std::path::Path;

use orquestador_core::client::{ClientConfig, DEFAULT_BASE_URL, DEFAULT_USERNAME};
use serde::{Deserialize, Serialize};

/// File written next to the other app config.
pub const FILE_NAME: &str = "server.json";

/// On-disk shape (all fields optional so a partial file still deserializes).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerConfigFile {
    pub base_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Trim and drop a trailing slash so `http://host:4096/` matches the client's
/// URL builder.
pub fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Collapse the optional on-disk shape into a validated [`ClientConfig`].
pub fn to_client_config(file: &ServerConfigFile) -> ClientConfig {
    let base_url = file
        .base_url
        .as_deref()
        .map(normalize_base_url)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let username = file
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_USERNAME)
        .to_string();
    ClientConfig {
        base_url,
        username,
        password: file.password.clone().filter(|p| !p.is_empty()),
        ..ClientConfig::default()
    }
}

/// Read `server.json` from `dir`. Returns `None` when missing or malformed.
pub fn load_from_dir(dir: &Path) -> Option<ClientConfig> {
    load_file(dir).map(|file| to_client_config(&file))
}

/// Read the raw on-disk file (used by the settings command so the UI can show
/// exactly what is stored).
pub fn load_file(dir: &Path) -> Option<ServerConfigFile> {
    let raw = std::fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Write `server.json` into `dir`, creating the directory when needed.
pub fn save_to_dir(dir: &Path, file: &ServerConfigFile) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(file).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(dir.join(FILE_NAME), json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_url_trims_and_strips_trailing_slash() {
        assert_eq!(normalize_base_url("  http://x:1/ "), "http://x:1");
        assert_eq!(normalize_base_url("http://x:1"), "http://x:1");
        assert_eq!(normalize_base_url("   "), "");
    }

    #[test]
    fn to_client_config_falls_back_to_defaults_for_blank_fields() {
        let cfg = to_client_config(&ServerConfigFile {
            base_url: Some("  ".to_string()),
            username: Some("".to_string()),
            password: Some("".to_string()),
        });
        assert_eq!(cfg.base_url, DEFAULT_BASE_URL);
        assert_eq!(cfg.username, DEFAULT_USERNAME);
        assert!(cfg.password.is_none());
    }

    #[test]
    fn to_client_config_honors_explicit_values() {
        let cfg = to_client_config(&ServerConfigFile {
            base_url: Some("http://host:4096/".to_string()),
            username: Some("alice".to_string()),
            password: Some("secret".to_string()),
        });
        assert_eq!(cfg.base_url, "http://host:4096");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password.as_deref(), Some("secret"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("orq-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = ServerConfigFile {
            base_url: Some("http://example:1234".to_string()),
            username: Some("bob".to_string()),
            password: Some("pw".to_string()),
        };
        save_to_dir(&dir, &file).unwrap();
        let loaded = load_file(&dir).expect("file should load back");
        assert_eq!(loaded.base_url.as_deref(), Some("http://example:1234"));
        assert_eq!(loaded.username.as_deref(), Some("bob"));
        assert_eq!(loaded.password.as_deref(), Some("pw"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
