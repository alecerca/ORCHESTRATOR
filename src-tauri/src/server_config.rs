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
    /// Absolute path of the active project (`x-opencode-directory`). Absent in a
    /// `server.json` written before project switching existed; `#[serde(default)]`
    /// keeps those files loadable.
    #[serde(default)]
    pub directory: Option<String>,
}

/// Trim and drop a trailing slash so `http://host:4096/` matches the client's
/// URL builder.
pub fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Trim a directory and treat a blank value as "not set" (use the server's own
/// cwd project), matching how blank credentials collapse to defaults.
pub fn normalize_directory(dir: Option<String>) -> Option<String> {
    dir.map(|d| d.trim().to_string()).filter(|d| !d.is_empty())
}

/// The raw file to build a save on: the persisted one when present, otherwise
/// the effective environment / default configuration materialized into a file.
///
/// Persisting one field must never silently drop a server that was configured
/// only through the environment, so a partial save always starts from this.
pub fn effective_file(dir: &Path) -> ServerConfigFile {
    if let Some(file) = load_file(dir) {
        return file;
    }
    let cfg = ClientConfig::from_env_or_default();
    ServerConfigFile {
        base_url: Some(cfg.base_url),
        username: Some(cfg.username),
        password: cfg.password,
        directory: cfg.directory,
    }
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
        directory: normalize_directory(file.directory.clone()),
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
            directory: Some("   ".to_string()),
        });
        assert_eq!(cfg.base_url, DEFAULT_BASE_URL);
        assert_eq!(cfg.username, DEFAULT_USERNAME);
        assert!(cfg.password.is_none());
        assert!(
            cfg.directory.is_none(),
            "a blank directory means the server's own cwd project"
        );
    }

    #[test]
    fn to_client_config_honors_explicit_values() {
        let cfg = to_client_config(&ServerConfigFile {
            base_url: Some("http://host:4096/".to_string()),
            username: Some("alice".to_string()),
            password: Some("secret".to_string()),
            directory: Some("  /home/dev/proj  ".to_string()),
        });
        assert_eq!(cfg.base_url, "http://host:4096");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password.as_deref(), Some("secret"));
        assert_eq!(cfg.directory.as_deref(), Some("/home/dev/proj"));
    }

    #[test]
    fn old_server_json_without_directory_still_loads() {
        // Backward compatibility: a file written before project switching has no
        // `directory` key; it must still deserialize with `directory: None`.
        let dir = std::env::temp_dir().join(format!("orq-cfg-old-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE_NAME),
            r#"{"base_url":"http://old:1","username":"u","password":"p"}"#,
        )
        .unwrap();

        let cfg = load_from_dir(&dir).expect("old file must still load");
        assert_eq!(cfg.base_url, "http://old:1");
        assert!(cfg.directory.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("orq-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = ServerConfigFile {
            base_url: Some("http://example:1234".to_string()),
            username: Some("bob".to_string()),
            password: Some("pw".to_string()),
            directory: Some("/home/dev/proj".to_string()),
        };
        save_to_dir(&dir, &file).unwrap();
        let loaded = load_file(&dir).expect("file should load back");
        assert_eq!(loaded.base_url.as_deref(), Some("http://example:1234"));
        assert_eq!(loaded.username.as_deref(), Some("bob"));
        assert_eq!(loaded.password.as_deref(), Some("pw"));
        assert_eq!(loaded.directory.as_deref(), Some("/home/dev/proj"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
