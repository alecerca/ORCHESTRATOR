//! Method-based role gate.
//!
//! OpenCode's read endpoints are all `GET` (including the `/event` SSE stream);
//! writes are `POST` / `PATCH` / `PUT` / `DELETE`. A `viewer` may issue safe
//! methods only; an `editor` may issue any method.

use crate::config::Role;

/// Is `method` a safe, side-effect-free HTTP method (`GET`/`HEAD`/`OPTIONS`)?
pub fn is_safe_method(method: &str) -> bool {
    matches!(method.to_ascii_uppercase().as_str(), "GET" | "HEAD" | "OPTIONS")
}

/// May `role` issue `method`? `viewer` = safe methods; `editor` = everything.
pub fn authorize(method: &str, role: Role) -> bool {
    match role {
        Role::Editor => true,
        Role::Viewer => is_safe_method(method),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_can_read() {
        for m in ["GET", "HEAD", "OPTIONS"] {
            assert!(authorize(m, Role::Viewer), "viewer should allow {m}");
        }
    }

    #[test]
    fn viewer_cannot_write() {
        for m in ["POST", "PATCH", "PUT", "DELETE"] {
            assert!(!authorize(m, Role::Viewer), "viewer must not allow {m}");
        }
    }

    #[test]
    fn editor_can_do_everything() {
        for m in ["GET", "HEAD", "OPTIONS", "POST", "PATCH", "PUT", "DELETE"] {
            assert!(authorize(m, Role::Editor), "editor should allow {m}");
        }
    }

    #[test]
    fn method_match_is_case_insensitive() {
        assert!(authorize("get", Role::Viewer));
        assert!(!authorize("post", Role::Viewer));
    }

    #[test]
    fn unknown_method_is_not_safe() {
        assert!(!is_safe_method("TRACE"));
        assert!(!authorize("TRACE", Role::Viewer));
    }
}
