//! Single error type for the whole core crate.

use serde::Serialize;

/// Every fallible core operation returns [`CoreError`].
///
/// It implements [`Serialize`] by hand so that Tauri commands can return it
/// straight to the webview as `{ kind, message }` without dragging
/// non-serializable sources (`reqwest::Error`) across the IPC boundary.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON (de)serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Non-2xx response. Carries the status and whatever the server said.
    #[error("server returned HTTP {status}: {message}")]
    Api { status: u16, message: String },

    /// The SSE stream broke (parse error, dropped connection mid-frame).
    #[error("event stream error: {0}")]
    Stream(String),

    /// Background event hub is not running (not started / shutting down).
    #[error("event hub is not running")]
    HubStopped,

    /// Base URL / auth / arguments are unusable.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// Explicit timeout waiting for the server.
    #[error("operation timed out")]
    Timeout,
}

/// Keywords that mark a server message as a usage/rate limit (matched
/// case-insensitively). Kept small and conservative on purpose.
const USAGE_LIMIT_KEYWORDS: [&str; 6] = [
    "free tier",
    "usage limit",
    "rate limit",
    "rate-limit",
    "quota",
    "too many requests",
];

/// Does this server message describe a usage/rate limit, regardless of status?
///
/// Pure and case-insensitive: providers word this differently, so this matches
/// a small keyword set and can never panic.
pub fn is_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    USAGE_LIMIT_KEYWORDS
        .iter()
        .any(|keyword| lower.contains(keyword))
}

/// Classify an HTTP status + message as a usage/rate limit.
///
/// Conservative on purpose: only the unambiguous statuses (`402 Payment
/// Required`, `429 Too Many Requests`) or a limit keyword in the message count,
/// so an unrelated `500` is never mislabelled.
pub fn is_usage_limit(status: u16, message: &str) -> bool {
    matches!(status, 402 | 429) || is_usage_limit_message(message)
}

impl CoreError {
    /// True when the failure is a usage/rate limit (HTTP 402/429 or a
    /// limit/quota/rate message), so the shell can warn distinctly.
    pub fn is_usage_limit(&self) -> bool {
        match self {
            CoreError::Api { status, message } => is_usage_limit(*status, message),
            CoreError::Stream(message) => is_usage_limit_message(message),
            other => is_usage_limit_message(&other.to_string()),
        }
    }

    /// Stable machine-readable discriminator used by the frontend.
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::Http(_) => "http",
            CoreError::Json(_) => "json",
            CoreError::Api { .. } => "api",
            CoreError::Stream(_) => "stream",
            CoreError::HubStopped => "hub_stopped",
            CoreError::Config(_) => "config",
            CoreError::Timeout => "timeout",
        }
    }

    /// Build an [`CoreError::Api`] from a non-success response, falling back to
    /// the status text when the body is not JSON with a `message`/`error` key.
    pub async fn from_response(res: reqwest::Response) -> CoreError {
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("message")
                    .or_else(|| v.get("error"))
                    .and_then(|m| m.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    format!("HTTP {status}")
                } else {
                    trimmed.chars().take(300).collect()
                }
            });
        CoreError::Api { status, message }
    }
}

impl Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = serializer.serialize_struct("CoreError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_to_a_stable_shape_for_the_webview() {
        let err = CoreError::Config("bad url".into());
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["kind"], "config");
        assert_eq!(v["message"], "invalid configuration: bad url");

        let err = CoreError::Api {
            status: 404,
            message: "not found".into(),
        };
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["kind"], "api");
        assert!(v["message"].as_str().unwrap().contains("404"));
    }

    #[test]
    fn usage_limit_detection_is_conservative_and_case_insensitive() {
        // Status alone is enough for the unambiguous limit codes.
        assert!(is_usage_limit(429, ""));
        assert!(is_usage_limit(402, "payment required"));

        // Unrelated statuses are NOT limits…
        assert!(!is_usage_limit(500, "internal server error"));
        assert!(!is_usage_limit(200, "all good"));
        assert!(!is_usage_limit(404, "not found"));

        // …unless the message names a limit (any case).
        assert!(is_usage_limit(500, "Free Tier limit reached"));
        assert!(is_usage_limit(0, "usage limit exceeded"));
        assert!(is_usage_limit(400, "Rate Limit"));
        assert!(is_usage_limit(400, "rate-limit exceeded"));
        assert!(is_usage_limit(400, "monthly quota exhausted"));
        assert!(is_usage_limit(400, "Too Many Requests"));

        // An unrelated message on a limit-ready status still counts (status).
        assert!(is_usage_limit(429, "slow down please"));

        // And an unrelated message never does.
        assert!(!is_usage_limit(400, "invalid model id"));
        assert!(!is_usage_limit_message("invalid model id"));
    }

    #[test]
    fn core_error_classifies_usage_limits() {
        assert!(CoreError::Api {
            status: 429,
            message: "slow down".into(),
        }
        .is_usage_limit());
        assert!(CoreError::Api {
            status: 400,
            message: "free tier exhausted".into(),
        }
        .is_usage_limit());
        assert!(!CoreError::Api {
            status: 500,
            message: "boom".into(),
        }
        .is_usage_limit());
        assert!(CoreError::Stream("rate limit hit on the stream".into()).is_usage_limit());
        assert!(!CoreError::Config("bad url".into()).is_usage_limit());
    }

    #[test]
    fn json_source_errors_are_wrapped() {
        let e: CoreError = serde_json::from_str::<serde_json::Value>("{oops")
            .unwrap_err()
            .into();
        assert_eq!(e.kind(), "json");
    }
}
