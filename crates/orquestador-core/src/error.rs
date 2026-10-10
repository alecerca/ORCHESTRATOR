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

impl CoreError {
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
    fn json_source_errors_are_wrapped() {
        let e: CoreError = serde_json::from_str::<serde_json::Value>("{oops")
            .unwrap_err()
            .into();
        assert_eq!(e.kind(), "json");
    }
}
