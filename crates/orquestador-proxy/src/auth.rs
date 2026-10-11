//! HTTP Basic auth helpers: parsing the inbound header, constant-time token
//! comparison, credential resolution, and encoding the outbound upstream header.

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

/// Resolve a presented `(name, token)` to a configured [`User`], comparing the
/// token in constant time. Returns `None` for an unknown name or wrong token.
pub fn resolve_user<'a>(config: &'a Config, name: &str, token: &str) -> Option<&'a User> {
    let user = config.lookup(name)?;
    if constant_time_eq(user.token.as_bytes(), token.as_bytes()) {
        Some(user)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Role;

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
    fn encode_basic_is_decodable() {
        let encoded = encode_basic("opencode", "up-pw");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim_start_matches("Basic "))
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "opencode:up-pw");
    }
}
