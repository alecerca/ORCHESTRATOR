//! HTTP Basic auth helpers: parsing the inbound header, constant-time token
//! comparison, credential resolution, and encoding the outbound upstream header.

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::PasswordVerifier;
use argon2::Argon2;
use base64::Engine as _;

use crate::config::{Config, User};

/// Decode a `Authorization: Basic base64(name:token)` header value.
///
/// Returns `(name, token)`. The scheme is matched case-insensitively; malformed
/// base64, non-UTF-8 payloads, a missing `:` separator, or a non-Basic scheme
/// yield `None`.
pub fn parse_basic(header_value: &str) -> Option<(String, String)> {
    let value = header_value.trim();
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (name, token) = text.split_once(':')?;
    Some((name.to_string(), token.to_string()))
}

/// Encode `Basic base64(username:password)` for the injected upstream auth.
pub fn encode_basic(username: &str, password: &str) -> String {
    let raw = format!("{username}:{password}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    )
}

/// Compare two byte strings without an early-exit on the secret bytes.
///
/// The length difference is folded into the accumulator rather than causing an
/// early return; the running XOR makes the comparison time independent of where
/// the first differing byte is.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff: u8 = 0;
    let max = a.len().max(b.len());
    for i in 0..max {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    // Fold the length difference in without truncation or an early return.
    let mut len_diff = a.len() ^ b.len();
    while len_diff != 0 {
        diff |= (len_diff & 0xff) as u8;
        len_diff >>= 8;
    }
    diff == 0
}

/// Verify a presented secret against a stored Argon2id PHC string.
///
/// A malformed or unsupported hash yields `false` rather than an error, so an
/// unusable stored credential simply fails closed.
fn verify_password_hash(hash: &str, secret: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok()
}

/// Resolve a presented `(name, secret)` to a configured [`User`].
///
/// When the user carries a `password_hash` it is verified with Argon2id;
/// otherwise the legacy `token` is compared in constant time. Returns `None`
/// for an unknown name or a wrong secret. Secrets are never logged or echoed.
pub fn resolve_user<'a>(config: &'a Config, name: &str, secret: &str) -> Option<&'a User> {
    let user = config.lookup(name)?;

    if let Some(hash) = user.password_hash.as_deref() {
        return verify_password_hash(hash, secret).then_some(user);
    }
    if let Some(token) = user.token.as_deref() {
        return constant_time_eq(token.as_bytes(), secret.as_bytes()).then_some(user);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Role;
    use argon2::password_hash::PasswordHasher as _;

    /// Hash a password independently of `resolve_user`, so verification is
    /// exercised against a real Argon2id PHC string rather than a fixture.
    fn hash(password: &str) -> String {
        Argon2::default()
            .hash_password(password.as_bytes())
            .expect("hash")
            .to_string()
    }

    fn config() -> Config {
        Config::from_json_str(
            r#"{
                "listen": "0.0.0.0:4097",
                "upstream": "http://127.0.0.1:4096",
                "upstream_username": "opencode",
                "users": [
                    { "name": "alice", "token": "s3cret-a", "role": "editor" },
                    { "name": "bob",   "token": "s3cret-b", "role": "viewer" }
                ]
            }"#,
        )
        .expect("valid config")
    }

    #[test]
    fn parses_basic_credentials() {
        let header = encode_basic("alice", "s3cret-a");
        // encode_basic is under test too, so compare against a known-good literal:
        let literal = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("alice:s3cret-a")
        );
        assert_eq!(literal, header);
        assert_eq!(
            parse_basic(&header),
            Some(("alice".to_string(), "s3cret-a".to_string()))
        );
    }

    #[test]
    fn scheme_is_case_insensitive() {
        let header = format!(
            "basic {}",
            base64::engine::general_purpose::STANDARD.encode("bob:s3cret-b")
        );
        assert_eq!(
            parse_basic(&header),
            Some(("bob".to_string(), "s3cret-b".to_string()))
        );
    }

    #[test]
    fn splits_on_first_colon_only() {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("bob:a:b:c")
        );
        assert_eq!(
            parse_basic(&header),
            Some(("bob".to_string(), "a:b:c".to_string()))
        );
    }

    #[test]
    fn rejects_non_basic_scheme() {
        assert_eq!(parse_basic("Bearer abc"), None);
    }

    #[test]
    fn rejects_missing_separator() {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("nocolon")
        );
        assert_eq!(parse_basic(&header), None);
    }

    #[test]
    fn rejects_invalid_base64() {
        assert_eq!(parse_basic("Basic !!!not-base64!!!"), None);
    }

    #[test]
    fn rejects_empty_header() {
        assert_eq!(parse_basic(""), None);
        assert_eq!(parse_basic("Basic"), None);
    }

    #[test]
    fn constant_time_eq_matches_and_differs() {
        assert!(constant_time_eq(b"same", b"same"));
        assert!(!constant_time_eq(b"same", b"diff"));
        assert!(!constant_time_eq(b"short", b"longer"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn resolve_user_accepts_matching_credentials() {
        let cfg = config();
        let user = resolve_user(&cfg, "alice", "s3cret-a").expect("alice");
        assert_eq!(user.role, Role::Editor);
    }

    #[test]
    fn resolve_user_rejects_wrong_token() {
        let cfg = config();
        assert!(resolve_user(&cfg, "alice", "wrong").is_none());
    }

    #[test]
    fn resolve_user_rejects_unknown_name() {
        let cfg = config();
        assert!(resolve_user(&cfg, "nobody", "s3cret-a").is_none());
    }

    #[test]
    fn resolve_user_accepts_correct_password() {
        let json = format!(
            r#"{{
                "listen": "0.0.0.0:4097",
                "upstream": "http://127.0.0.1:4096",
                "upstream_username": "opencode",
                "users": [
                    {{ "name": "dave", "password_hash": "{}", "role": "viewer" }}
                ]
            }}"#,
            hash("hunter2")
        );
        let cfg = Config::from_json_str(&json).expect("valid config");
        let user = resolve_user(&cfg, "dave", "hunter2").expect("correct password");
        assert_eq!(user.role, Role::Viewer);
    }

    #[test]
    fn resolve_user_rejects_wrong_password() {
        let json = format!(
            r#"{{
                "listen": "0.0.0.0:4097",
                "upstream": "http://127.0.0.1:4096",
                "upstream_username": "opencode",
                "users": [
                    {{ "name": "dave", "password_hash": "{}", "role": "viewer" }}
                ]
            }}"#,
            hash("hunter2")
        );
        let cfg = Config::from_json_str(&json).expect("valid config");
        assert!(resolve_user(&cfg, "dave", "wrong").is_none());
    }

    #[test]
    fn password_hash_takes_precedence_over_token() {
        let json = format!(
            r#"{{
                "listen": "0.0.0.0:4097",
                "upstream": "http://127.0.0.1:4096",
                "upstream_username": "opencode",
                "users": [
                    {{ "name": "erin", "token": "legacy-token", "password_hash": "{}", "role": "editor" }}
                ]
            }}"#,
            hash("real-password")
        );
        let cfg = Config::from_json_str(&json).expect("valid config");
        assert!(resolve_user(&cfg, "erin", "real-password").is_some());
        // The legacy token must not authenticate once a hash is present.
        assert!(resolve_user(&cfg, "erin", "legacy-token").is_none());
    }

    #[test]
    fn resolve_user_rejects_malformed_password_hash() {
        let json = r#"{
            "listen": "0.0.0.0:4097",
            "upstream": "http://127.0.0.1:4096",
            "upstream_username": "opencode",
            "users": [ { "name": "dave", "password_hash": "not-a-phc-string", "role": "viewer" } ]
        }"#;
        let cfg = Config::from_json_str(json).expect("shape is valid");
        assert!(resolve_user(&cfg, "dave", "hunter2").is_none());
    }

    #[test]
    fn encode_basic_is_decodable() {
        let encoded = encode_basic("opencode", "up-pw");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim_start_matches("Basic "))
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "opencode:up-pw");
    }
}
