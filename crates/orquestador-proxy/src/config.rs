//! Proxy configuration (`proxy.json`): parsing, validation and user lookup.

use std::path::Path;

use serde::Deserialize;

/// A client role. `viewer` may only issue safe methods; `editor` may issue all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Editor,
}

impl Role {
    /// Lowercase wire/audit representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Editor => "editor",
        }
    }
}

/// A configured client: who they are, the credential they present, and their role.
///
/// A user must carry at least one non-empty credential. `password_hash` is the
/// preferred Argon2id PHC string; `token` is the legacy plaintext secret kept
/// for back-compat with existing configs.
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub name: String,
    /// Legacy plaintext token. Optional since hashed users may omit it.
    #[serde(default)]
    pub token: Option<String>,
    /// Argon2id PHC string for the user's password.
    #[serde(default)]
    pub password_hash: Option<String>,
    pub role: Role,
}

/// Parsed `proxy.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Address the proxy binds, e.g. `0.0.0.0:4097`.
    pub listen: String,
    /// Upstream base URL, e.g. `http://127.0.0.1:4096`.
    pub upstream: String,
    /// Username used for the injected upstream Basic auth.
    pub upstream_username: String,
    /// Upstream password fallback (env `ORQUESTADOR_UPSTREAM_PASSWORD` wins).
    #[serde(default)]
    pub upstream_password: Option<String>,
    /// Known clients.
    pub users: Vec<User>,
}

/// Failure while reading or validating [`Config`].
#[derive(Debug)]
pub enum ConfigError {
    /// I/O failure reading the config file.
    Io(std::io::Error),
    /// JSON was syntactically invalid or missing a required field.
    Parse(serde_json::Error),
    /// JSON parsed but the values are unusable (empty fields, duplicate users…).
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "could not read config: {e}"),
            ConfigError::Parse(e) => write!(f, "invalid config JSON: {e}"),
            ConfigError::Invalid(msg) => write!(f, "invalid config: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            ConfigError::Invalid(_) => None,
        }
    }
}

impl Config {
    /// Parse and validate a config from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Config, ConfigError> {
        let config: Config = serde_json::from_str(json).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    /// Read, parse and validate a config file.
    pub fn load(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
        let raw = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Config::from_json_str(&raw)
    }

    /// Validate semantic invariants (non-empty fields, unique user names).
    pub fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |msg: String| Err(ConfigError::Invalid(msg));

        if self.listen.trim().is_empty() {
            return invalid("`listen` must not be empty".into());
        }
        if self.upstream.trim().is_empty() {
            return invalid("`upstream` must not be empty".into());
        }
        if self.upstream_username.trim().is_empty() {
            return invalid("`upstream_username` must not be empty".into());
        }
        if self.users.is_empty() {
            return invalid("`users` must contain at least one user".into());
        }

        for user in &self.users {
            if user.name.trim().is_empty() {
                return invalid("every user needs a non-empty `name`".into());
            }
            let has_token = user.token.as_deref().is_some_and(|token| !token.is_empty());
            let has_hash = user
                .password_hash
                .as_deref()
                .is_some_and(|hash| !hash.is_empty());
            if !has_token && !has_hash {
                return invalid(format!(
                    "user `{}` needs a non-empty `token` or `password_hash`",
                    user.name
                ));
            }
        }

        for (i, user) in self.users.iter().enumerate() {
            if self.users[..i].iter().any(|other| other.name == user.name) {
                return invalid(format!("duplicate user name: `{}`", user.name));
            }
        }

        Ok(())
    }

    /// Look up a configured user by exact name.
    pub fn lookup(&self, name: &str) -> Option<&User> {
        self.users.iter().find(|user| user.name == name)
    }
}

/// Choose the upstream password. The environment value (already read by the
/// caller) is preferred over the config field; an empty env value counts as
/// absent so a stray `VAR=` does not blank out a configured password.
pub fn pick_upstream_password(env: Option<String>, config: Option<String>) -> Option<String> {
    env.filter(|value| !value.is_empty()).or(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "listen": "0.0.0.0:4097",
        "upstream": "http://127.0.0.1:4096",
        "upstream_username": "opencode",
        "users": [
            { "name": "alice", "token": "change-me-1", "role": "editor" },
            { "name": "bob",   "token": "change-me-2", "role": "viewer" }
        ]
    }"#;

    #[test]
    fn parses_a_valid_config() {
        let cfg = Config::from_json_str(VALID).expect("valid config");
        assert_eq!(cfg.listen, "0.0.0.0:4097");
        assert_eq!(cfg.upstream, "http://127.0.0.1:4096");
        assert_eq!(cfg.upstream_username, "opencode");
        assert_eq!(cfg.upstream_password, None);
        assert_eq!(cfg.users.len(), 2);
        assert_eq!(cfg.users[0].role, Role::Editor);
        assert_eq!(cfg.users[1].role, Role::Viewer);
    }

    #[test]
    fn parses_optional_upstream_password() {
        let json = VALID.replace("\"users\"", "\"upstream_password\": \"pw\", \"users\"");
        let cfg = Config::from_json_str(&json).expect("valid config");
        assert_eq!(cfg.upstream_password.as_deref(), Some("pw"));
    }

    #[test]
    fn rejects_unknown_role() {
        let json = VALID.replace("editor", "admin");
        let err = Config::from_json_str(&json).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "got {err:?}");
    }

    #[test]
    fn rejects_missing_required_field() {
        let err = Config::from_json_str(r#"{ "listen": "0.0.0.0:1", "upstream": "http://x" }"#)
            .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "got {err:?}");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = Config::from_json_str("{ not json").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "got {err:?}");
    }

    #[test]
    fn rejects_empty_users() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": []
        }"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn rejects_duplicate_user_names() {
        let json = VALID.replace("\"bob\"", "\"alice\"");
        let err = Config::from_json_str(&json).unwrap_err();
        match err {
            ConfigError::Invalid(msg) => assert!(msg.contains("duplicate"), "got {msg}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_token() {
        let json = VALID.replace("change-me-1", "");
        let err = Config::from_json_str(&json).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn rejects_user_without_any_credential() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": [ { "name": "alice", "role": "editor" } ]
        }"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn parses_password_hash_without_token() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": [
                { "name": "alice",
                  "password_hash": "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
                  "role": "editor" }
            ]
        }"#;
        let cfg = Config::from_json_str(json).expect("password_hash-only user is valid");
        assert_eq!(cfg.users[0].name, "alice");
        assert!(cfg.users[0].password_hash.is_some());
        assert_eq!(cfg.users[0].token, None);
    }

    #[test]
    fn rejects_empty_password_hash_without_token() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": [ { "name": "alice", "password_hash": "", "role": "editor" } ]
        }"#;
        let err = Config::from_json_str(json).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn accepts_token_and_password_hash_together() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": [
                { "name": "alice", "token": "legacy",
                  "password_hash": "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA",
                  "role": "editor" }
            ]
        }"#;
        let cfg = Config::from_json_str(json).expect("both credentials are allowed");
        assert_eq!(cfg.users[0].token.as_deref(), Some("legacy"));
        assert!(cfg.users[0].password_hash.is_some());
    }

    #[test]
    fn rejects_empty_listen() {
        let json = VALID.replace("0.0.0.0:4097", "");
        let err = Config::from_json_str(&json).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn lookup_finds_and_misses() {
        let cfg = Config::from_json_str(VALID).unwrap();
        assert_eq!(cfg.lookup("alice").map(|u| u.role), Some(Role::Editor));
        assert!(cfg.lookup("nobody").is_none());
    }

    #[test]
    fn upstream_password_env_wins_over_config() {
        let picked =
            pick_upstream_password(Some("from-env".into()), Some("from-config".into()));
        assert_eq!(picked.as_deref(), Some("from-env"));
    }

    #[test]
    fn upstream_password_falls_back_to_config() {
        let picked = pick_upstream_password(None, Some("from-config".into()));
        assert_eq!(picked.as_deref(), Some("from-config"));
    }

    #[test]
    fn upstream_password_none_when_both_absent() {
        assert_eq!(pick_upstream_password(None, None), None);
    }
}
