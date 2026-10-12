//! Pure core for the `adduser` CLI subcommand.
//!
//! The server owner adds collaborators without hand-editing JSON: this module
//! hashes a password with Argon2id (random salt, default parameters) and
//! appends the new user to the existing config document. The raw
//! [`serde_json::Value`] is edited in place so unknown fields and existing
//! users survive untouched. Plaintext passwords and hashes are never logged.

use argon2::password_hash::PasswordHasher;
use argon2::Argon2;
use serde_json::Value;

use crate::config::Role;

/// Failure while adding a user to a config document.
#[derive(Debug)]
pub enum AddUserError {
    /// The document was not valid JSON.
    Parse(serde_json::Error),
    /// The document is missing a `users` array.
    MissingUsers,
    /// The requested user name is empty.
    EmptyName,
    /// The requested password is empty.
    EmptyPassword,
    /// A user with the requested name already exists.
    Duplicate(String),
    /// Hashing the password failed.
    Hash(String),
}

impl std::fmt::Display for AddUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddUserError::Parse(e) => write!(f, "invalid config JSON: {e}"),
            AddUserError::MissingUsers => write!(f, "config has no `users` array"),
            AddUserError::EmptyName => write!(f, "user name must not be empty"),
            AddUserError::EmptyPassword => write!(f, "password must not be empty"),
            AddUserError::Duplicate(name) => write!(f, "user `{name}` already exists"),
            AddUserError::Hash(msg) => write!(f, "could not hash password: {msg}"),
        }
    }
}

impl std::error::Error for AddUserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AddUserError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

/// Parse `json`, append a new user carrying an Argon2id `password_hash`, and
/// return the updated document as pretty JSON.
///
/// Rejects an empty name, an empty password and a duplicate name. Existing
/// users and any unknown fields are preserved.
pub fn add_user_to_json(
    json: &str,
    name: &str,
    role: Role,
    password: &str,
) -> Result<String, AddUserError> {
    if name.trim().is_empty() {
        return Err(AddUserError::EmptyName);
    }
    if password.is_empty() {
        return Err(AddUserError::EmptyPassword);
    }

    let mut root: Value = serde_json::from_str(json).map_err(AddUserError::Parse)?;
    let users = root
        .get_mut("users")
        .and_then(Value::as_array_mut)
        .ok_or(AddUserError::MissingUsers)?;

    if users
        .iter()
        .any(|user| user.get("name").and_then(Value::as_str) == Some(name))
    {
        return Err(AddUserError::Duplicate(name.to_string()));
    }

    let password_hash = hash_password(password)?;
    users.push(serde_json::json!({
        "name": name,
        "password_hash": password_hash,
        "role": role.as_str(),
    }));

    serde_json::to_string_pretty(&root).map_err(AddUserError::Parse)
}

/// Hash a password with Argon2id and default parameters. The random salt is
/// generated internally and embedded in the returned PHC string.
fn hash_password(password: &str) -> Result<String, AddUserError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| AddUserError::Hash(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    const BASE: &str = r#"{
        "listen": "0.0.0.0:4097",
        "upstream": "http://127.0.0.1:4096",
        "upstream_username": "opencode",
        "custom_top_level": "keep-me",
        "users": [
            { "name": "alice", "token": "change-me-1", "role": "editor", "note": "keep-me-too" }
        ]
    }"#;

    #[test]
    fn adds_user_and_round_trips_through_config() {
        let updated = add_user_to_json(BASE, "dave", Role::Viewer, "hunter2").expect("added");
        let cfg = Config::from_json_str(&updated).expect("updated config is valid");

        assert_eq!(cfg.users.len(), 2);
        let dave = cfg.lookup("dave").expect("dave present");
        assert_eq!(dave.role, Role::Viewer);
        assert!(dave.password_hash.is_some());
        assert_eq!(dave.token, None);
        assert!(cfg.lookup("alice").is_some(), "existing user preserved");
    }

    #[test]
    fn appended_hash_verifies_the_password() {
        let updated = add_user_to_json(BASE, "dave", Role::Editor, "hunter2").expect("added");
        let cfg = Config::from_json_str(&updated).unwrap();

        assert!(crate::auth::resolve_user(&cfg, "dave", "hunter2").is_some());
        assert!(crate::auth::resolve_user(&cfg, "dave", "wrong").is_none());
        assert_eq!(cfg.lookup("dave").unwrap().role, Role::Editor);
    }

    #[test]
    fn preserves_unknown_fields() {
        let updated = add_user_to_json(BASE, "dave", Role::Viewer, "hunter2").unwrap();
        let value: Value = serde_json::from_str(&updated).unwrap();

        assert_eq!(value["custom_top_level"], "keep-me");
        let alice = value["users"]
            .as_array()
            .unwrap()
            .iter()
            .find(|user| user["name"] == "alice")
            .expect("alice present");
        assert_eq!(alice["note"], "keep-me-too");
    }

    #[test]
    fn uses_a_fresh_random_salt_per_call() {
        let first = add_user_to_json(BASE, "dave", Role::Editor, "hunter2").unwrap();
        let second = add_user_to_json(BASE, "dave", Role::Editor, "hunter2").unwrap();
        assert_ne!(first, second, "each hash must use a fresh random salt");
    }

    #[test]
    fn rejects_duplicate_name() {
        let err = add_user_to_json(BASE, "alice", Role::Editor, "hunter2").unwrap_err();
        assert!(
            matches!(err, AddUserError::Duplicate(ref name) if name == "alice"),
            "got {err:?}"
        );
    }

    #[test]
    fn rejects_empty_password() {
        let err = add_user_to_json(BASE, "dave", Role::Editor, "").unwrap_err();
        assert!(matches!(err, AddUserError::EmptyPassword), "got {err:?}");
    }

    #[test]
    fn rejects_empty_name() {
        let err = add_user_to_json(BASE, "  ", Role::Editor, "hunter2").unwrap_err();
        assert!(matches!(err, AddUserError::EmptyName), "got {err:?}");
    }

    #[test]
    fn rejects_missing_users_array() {
        let err =
            add_user_to_json(r#"{ "listen": "x" }"#, "dave", Role::Editor, "hunter2").unwrap_err();
        assert!(matches!(err, AddUserError::MissingUsers), "got {err:?}");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = add_user_to_json("{ not json", "dave", Role::Editor, "hunter2").unwrap_err();
        assert!(matches!(err, AddUserError::Parse(_)), "got {err:?}");
    }

    #[test]
    fn error_display_never_leaks_the_password_or_hash() {
        let err = add_user_to_json(BASE, "alice", Role::Editor, "topsecret").unwrap_err();
        let rendered = err.to_string();
        assert!(!rendered.contains("topsecret"), "got {rendered}");
        assert!(!rendered.contains("$argon2"), "got {rendered}");
    }
}
