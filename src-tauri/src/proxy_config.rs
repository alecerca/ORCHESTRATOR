//! Host-side editing of the whitelist `proxy.json`.
//!
//! The host app never runs the proxy; it only reads and edits the owner's
//! whitelist file at a caller-supplied path. This module is a small,
//! self-contained mirror of the proxy's format (`name` + `role` + Argon2id
//! `password_hash`) so the desktop build does not depend on `orquestador-proxy`
//! (which would drag axum in). The raw JSON document is edited in place so
//! unrelated top-level fields and unknown per-user fields survive untouched.

use std::path::Path;

use argon2::password_hash::PasswordHasher;
use argon2::Argon2;
use serde::Serialize;
use serde_json::Value;

/// Roles the proxy enforces. Keep in sync with `crates/orquestador-proxy`.
const ROLES: [&str; 2] = ["editor", "viewer"];

/// A whitelist member as shown by the host console: identity and role only.
///
/// Credentials (the `password_hash` and any legacy `token`) are never exposed to
/// the frontend, so they are not part of this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProxyMember {
    pub name: String,
    pub role: String,
}

/// Read and parse the config document at `path`.
fn read_document(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("could not read proxy config {}: {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| format!("invalid proxy config JSON: {err}"))
}

/// Serialize `root` as pretty JSON and write it back over `path`.
fn write_document(path: &Path, root: &Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(root)
        .map_err(|err| format!("could not serialize proxy config: {err}"))?;
    std::fs::write(path, json)
        .map_err(|err| format!("could not write proxy config {}: {err}", path.display()))
}

/// Borrow the `users` array, erroring when the document has none.
fn users(root: &Value) -> Result<&[Value], String> {
    root.get("users")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "proxy config has no `users` array".to_string())
}

/// Mutably borrow the `users` array, erroring when the document has none.
fn users_mut(root: &mut Value) -> Result<&mut Vec<Value>, String> {
    root.get_mut("users")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "proxy config has no `users` array".to_string())
}

/// The `name` of a raw user object, when present and a string.
fn user_name(user: &Value) -> Option<&str> {
    user.get("name").and_then(Value::as_str)
}

/// Hash a password with Argon2id and default parameters. The random salt is
/// generated internally and embedded in the returned PHC string, so hashes are
/// byte-compatible with the proxy's verifier.
fn hash_password(password: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| format!("could not hash password: {err}"))
}

/// List every member's `name` + `role`, ignoring credentials.
pub fn list_members(path: &Path) -> Result<Vec<ProxyMember>, String> {
    let root = read_document(path)?;
    users(&root)?
        .iter()
        .map(|user| {
            let name = user_name(user)
                .ok_or_else(|| "proxy config user is missing a `name`".to_string())?;
            let role = user
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("proxy config user `{name}` is missing a `role`"))?;
            Ok(ProxyMember {
                name: name.to_string(),
                role: role.to_string(),
            })
        })
        .collect()
}

/// Append a member carrying an Argon2id `password_hash`, preserving the rest.
///
/// Rejects an empty name, an empty password, a duplicate name and an unknown
/// role. The document is only written after every check passes, so a rejected
/// call never mutates the file.
pub fn add_member(path: &Path, name: &str, role: &str, password: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("user name must not be empty".to_string());
    }
    if password.is_empty() {
        return Err("password must not be empty".to_string());
    }
    if !ROLES.contains(&role) {
        return Err(format!("unknown role: {role:?}"));
    }

    let mut root = read_document(path)?;
    if users_mut(&mut root)?
        .iter()
        .any(|user| user_name(user) == Some(name))
    {
        return Err(format!("user `{name}` already exists"));
    }

    let password_hash = hash_password(password)?;
    users_mut(&mut root)?.push(serde_json::json!({
        "name": name,
        "role": role,
        "password_hash": password_hash,
    }));

    write_document(path, &root)
}

/// Remove the member named `name`, erroring when no such member exists.
pub fn remove_member(path: &Path, name: &str) -> Result<(), String> {
    let mut root = read_document(path)?;
    let users = users_mut(&mut root)?;
    let before = users.len();
    users.retain(|user| user_name(user) != Some(name));
    if users.len() == before {
        return Err(format!("user `{name}` not found"));
    }
    write_document(path, &root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const BASE: &str = r#"{
        "listen": "0.0.0.0:4097",
        "upstream": "http://127.0.0.1:4096",
        "upstream_username": "opencode",
        "custom_top_level": "keep-me",
        "users": [
            { "name": "alice", "token": "change-me-1", "role": "editor", "note": "keep-me-too" }
        ]
    }"#;

    /// Write `json` to a fresh `proxy.json` inside a temp dir. The returned
    /// `TempDir` must stay alive for the duration of the test.
    fn config_file(json: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("proxy.json");
        std::fs::write(&path, json).expect("write fixture");
        (dir, path)
    }

    fn users_of(path: &Path) -> Vec<Value> {
        let raw = std::fs::read_to_string(path).expect("read config");
        let value: Value = serde_json::from_str(&raw).expect("parse config");
        value["users"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn add_then_list_round_trips() {
        let (_dir, path) = config_file(BASE);
        add_member(&path, "dave", "viewer", "hunter2").expect("add");
        let members = list_members(&path).expect("list");
        assert_eq!(members.len(), 2);
        assert_eq!(
            members
                .iter()
                .find(|m| m.name == "dave")
                .map(|m| m.role.as_str()),
            Some("viewer")
        );
        assert!(members.iter().any(|m| m.name == "alice"));
    }

    #[test]
    fn added_hash_is_an_argon2id_phc_string() {
        let (_dir, path) = config_file(BASE);
        add_member(&path, "dave", "editor", "hunter2").expect("add");
        let dave = users_of(&path)
            .into_iter()
            .find(|u| u["name"] == "dave")
            .expect("dave present");
        let hash = dave["password_hash"].as_str().expect("password_hash");
        assert!(hash.starts_with("$argon2id$"), "got {hash}");
        assert_eq!(dave["role"], "editor");
    }

    #[test]
    fn hash_uses_a_fresh_random_salt_per_call() {
        let (_dir, path) = config_file(BASE);
        add_member(&path, "dave", "editor", "hunter2").expect("add");
        let first = users_of(&path)
            .into_iter()
            .find(|u| u["name"] == "dave")
            .unwrap()["password_hash"]
            .clone();
        remove_member(&path, "dave").expect("remove");
        add_member(&path, "dave", "editor", "hunter2").expect("add again");
        let second = users_of(&path)
            .into_iter()
            .find(|u| u["name"] == "dave")
            .unwrap()["password_hash"]
            .clone();
        assert_ne!(first, second, "each hash must use a fresh random salt");
    }

    #[test]
    fn rejects_duplicate_name() {
        let (_dir, path) = config_file(BASE);
        let err = add_member(&path, "alice", "editor", "hunter2").unwrap_err();
        assert!(err.contains("already exists"), "got {err}");
    }

    #[test]
    fn remove_member_drops_the_user() {
        let (_dir, path) = config_file(BASE);
        remove_member(&path, "alice").expect("remove");
        assert!(list_members(&path).expect("list").is_empty());
    }

    #[test]
    fn remove_missing_user_errors() {
        let (_dir, path) = config_file(BASE);
        let err = remove_member(&path, "nobody").unwrap_err();
        assert!(err.contains("not found"), "got {err}");
    }

    #[test]
    fn never_writes_the_plaintext_password() {
        let (_dir, path) = config_file(BASE);
        add_member(&path, "dave", "editor", "sup3r-secret-pw").expect("add");
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("sup3r-secret-pw"),
            "plaintext password must never reach disk"
        );
        assert!(raw.contains("$argon2id$"), "only the PHC hash is stored");
    }

    #[test]
    fn preserves_unknown_fields() {
        let (_dir, path) = config_file(BASE);
        add_member(&path, "dave", "viewer", "hunter2").expect("add");
        let raw = std::fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(value["custom_top_level"], "keep-me");
        let alice = value["users"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["name"] == "alice")
            .expect("alice preserved");
        assert_eq!(alice["note"], "keep-me-too");
        assert_eq!(alice["token"], "change-me-1", "legacy token preserved");
    }

    #[test]
    fn rejects_empty_name_empty_password_and_unknown_role() {
        let (_dir, path) = config_file(BASE);
        assert!(add_member(&path, "  ", "editor", "hunter2").is_err());
        assert!(add_member(&path, "dave", "editor", "").is_err());
        assert!(add_member(&path, "dave", "admin", "hunter2").is_err());
        // None of the rejected calls may have written a user.
        assert_eq!(list_members(&path).expect("list").len(), 1);
    }

    #[test]
    fn missing_file_errors_instead_of_panicking() {
        let (_dir, path) = config_file(BASE);
        let missing = path.with_file_name("does-not-exist.json");
        assert!(list_members(&missing).is_err());
        assert!(add_member(&missing, "dave", "viewer", "hunter2").is_err());
        assert!(remove_member(&missing, "dave").is_err());
    }
}
