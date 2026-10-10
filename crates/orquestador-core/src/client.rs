//! Typed async HTTP client for the OpenCode server (`opencode serve`).
//!
//! Design rules (see `docs/PLAN.md` §3):
//!
//! * No `tauri` dependency — this crate is headless and fully unit-testable.
//! * `reqwest` with `default-features = false` + `rustls-tls`: no openssl
//!   system dependency at link time.
//! * Every endpoint from `SPEC.md` §4 lives here and ONLY here; nothing else
//!   in the codebase builds a URL by hand.
//! * Optional HTTP basic auth (`OPENCODE_SERVER_PASSWORD`, user `opencode`).

use crate::error::CoreError;
use crate::types::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

/// Default base URL of `opencode serve`.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4096";
/// Default basic-auth user (`OPENCODE_SERVER_USERNAME` overrides it).
pub const DEFAULT_USERNAME: &str = "opencode";
/// Title given to the session the composer drives when none exists yet.
pub const DEFAULT_ROOT_TITLE: &str = "Orchestrator";

/// Configuration for [`OpenCodeClient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub base_url: String,
    pub username: String,
    pub password: Option<String>,
    pub request_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            username: DEFAULT_USERNAME.to_string(),
            password: None,
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// Typed OpenCode server client. Cheap to clone (shares one connection pool).
#[derive(Debug, Clone)]
pub struct OpenCodeClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: Option<String>,
}

impl OpenCodeClient {
    /// Build a client from explicit configuration.
    pub fn new(config: ClientConfig) -> Result<Self, CoreError> {
        let base_url = config.base_url.trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(CoreError::Config("base_url must not be empty".into()));
        }
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err(CoreError::Config(format!(
                "base_url must start with http:// or https:// (got {base_url})"
            )));
        }
        let http = reqwest::Client::builder()
            .timeout(config.request_timeout)
            // The SSE stream must not be killed by the request timeout; the
            // hub sets its own read/idle policy on top of this.
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            http,
            base_url,
            username: config.username,
            password: config.password,
        })
    }

    /// Build a client from the environment:
    /// `OPENCODE_SERVER_BASE_URL` (optional, defaults to `127.0.0.1:4096`;
    /// `OPENCODE_SERVER_URL` is accepted as a legacy alias),
    /// `OPENCODE_SERVER_PASSWORD` (optional, enables basic auth),
    /// `OPENCODE_SERVER_USERNAME` (optional, defaults to `opencode`).
    pub fn from_env() -> Result<Self, CoreError> {
        let mut cfg = ClientConfig::default();
        let url = std::env::var("OPENCODE_SERVER_BASE_URL")
            .or_else(|_| std::env::var("OPENCODE_SERVER_URL"));
        if let Ok(url) = url {
            if !url.trim().is_empty() {
                cfg.base_url = url;
            }
        }
        if let Ok(user) = std::env::var("OPENCODE_SERVER_USERNAME") {
            if !user.trim().is_empty() {
                cfg.username = user;
            }
        }
        cfg.password = std::env::var("OPENCODE_SERVER_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty());
        Self::new(cfg)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn has_auth(&self) -> bool {
        self.password.is_some()
    }

    // -----------------------------------------------------------------------
    // Transport helpers
    // -----------------------------------------------------------------------

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self.http.request(method, self.url(path));
        if let Some(password) = &self.password {
            req = req.basic_auth(self.username.as_str(), Some(password));
        }
        req
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, CoreError> {
        let res = self.request(reqwest::Method::GET, path).send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<T>().await?)
    }

    async fn send_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &B,
    ) -> Result<T, CoreError> {
        let res = self.request(method, path).json(body).send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<T>().await?)
    }

    /// Sends a body and only checks the status code (endpoints returning `204`).
    async fn send_ignored<B: Serialize + ?Sized>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &B,
    ) -> Result<(), CoreError> {
        let res = self.request(method, path).json(body).send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(())
    }

    /// Percent-encode a single path segment (unreserved chars only).
    /// Session/message ids are `ses_…`/`msg_…` today, but never trust that.
    fn seg(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        for b in raw.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Global / project
    // -----------------------------------------------------------------------

    /// `GET /global/health`
    pub async fn health(&self) -> Result<Health, CoreError> {
        self.get_json("/global/health").await
    }

    /// `GET /project`
    pub async fn projects(&self) -> Result<Vec<Project>, CoreError> {
        self.get_json("/project").await
    }

    /// `GET /project/current`
    pub async fn current_project(&self) -> Result<Project, CoreError> {
        self.get_json("/project/current").await
    }

    /// `GET /agent`
    pub async fn agents(&self) -> Result<Vec<Agent>, CoreError> {
        self.get_json("/agent").await
    }

    // -----------------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------------

    /// `GET /session`
    pub async fn list_sessions(&self) -> Result<Vec<Session>, CoreError> {
        self.get_json("/session").await
    }

    /// `POST /session` — `{ parentID?, title? }`
    pub async fn create_session(&self, body: &CreateSessionBody) -> Result<Session, CoreError> {
        self.send_json(reqwest::Method::POST, "/session", body)
            .await
    }

    /// `GET /session/status` → map keyed by session id.
    pub async fn session_statuses(&self) -> Result<HashMap<String, SessionStatus>, CoreError> {
        self.get_json("/session/status").await
    }

    /// `GET /session/:id`
    pub async fn session(&self, id: &str) -> Result<Session, CoreError> {
        self.get_json(&format!("/session/{}", Self::seg(id))).await
    }

    /// `DELETE /session/:id`
    pub async fn delete_session(&self, id: &str) -> Result<bool, CoreError> {
        let path = format!("/session/{}", Self::seg(id));
        let res = self.request(reqwest::Method::DELETE, &path).send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<bool>().await.unwrap_or(true))
    }

    /// `PATCH /session/:id` — `{ title }`
    pub async fn rename_session(&self, id: &str, title: &str) -> Result<Session, CoreError> {
        let path = format!("/session/{}", Self::seg(id));
        let body = serde_json::json!({ "title": title });
        self.send_json(reqwest::Method::PATCH, &path, &body).await
    }

    /// `GET /session/:id/children` — **the sub-agents of a session**.
    pub async fn session_children(&self, id: &str) -> Result<Vec<Session>, CoreError> {
        self.get_json(&format!("/session/{}/children", Self::seg(id)))
            .await
    }

    /// `GET /session/:id/todo`
    pub async fn todos(&self, id: &str) -> Result<Vec<Todo>, CoreError> {
        self.get_json(&format!("/session/{}/todo", Self::seg(id)))
            .await
    }

    /// `POST /session/:id/abort` → `bool`
    pub async fn abort(&self, id: &str) -> Result<bool, CoreError> {
        let path = format!("/session/{}/abort", Self::seg(id));
        let res = self
            .request(reqwest::Method::POST, &path)
            .json(&serde_json::json!({}))
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<bool>().await.unwrap_or(true))
    }

    // -----------------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------------

    /// `POST /session/:id/message` — **waits for the full assistant reply**.
    /// Can block for minutes; prefer [`Self::prompt_async`] from the UI.
    pub async fn prompt(&self, id: &str, body: &PromptBody) -> Result<MessageBundle, CoreError> {
        let path = format!("/session/{}/message", Self::seg(id));
        self.send_json(reqwest::Method::POST, &path, body).await
    }

    /// `POST /session/:id/prompt_async` → `204`, does not wait.
    pub async fn prompt_async(&self, id: &str, body: &PromptBody) -> Result<(), CoreError> {
        let path = format!("/session/{}/prompt_async", Self::seg(id));
        self.send_ignored(reqwest::Method::POST, &path, body).await
    }

    /// `GET /session/:id/message?limit=` — history, oldest first.
    pub async fn messages(
        &self,
        id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<MessageBundle>, CoreError> {
        let mut path = format!("/session/{}/message", Self::seg(id));
        if let Some(limit) = limit {
            path.push_str(&format!("?limit={limit}"));
        }
        self.get_json(&path).await
    }

    /// `GET /session/:id/message/:messageID`
    pub async fn message(&self, id: &str, message_id: &str) -> Result<MessageBundle, CoreError> {
        let path = format!(
            "/session/{}/message/{}",
            Self::seg(id),
            Self::seg(message_id)
        );
        self.get_json(&path).await
    }

    /// `POST /session/:id/command` → the assistant reply the command produced.
    pub async fn command(&self, id: &str, body: &CommandBody) -> Result<MessageBundle, CoreError> {
        let path = format!("/session/{}/command", Self::seg(id));
        self.send_json(reqwest::Method::POST, &path, body).await
    }

    // -----------------------------------------------------------------------
    // Permissions (approval prompts)
    // -----------------------------------------------------------------------

    /// `POST /session/:id/permissions/:permissionID`
    pub async fn respond_permission(
        &self,
        id: &str,
        permission_id: &str,
        response: PermissionResponse,
        remember: Option<bool>,
    ) -> Result<bool, CoreError> {
        let path = format!(
            "/session/{}/permissions/{}",
            Self::seg(id),
            Self::seg(permission_id)
        );
        // `remember` is optional and MUST be omitted when absent (PLAN §6.3);
        // a JSON `null` would make strict servers reject the body.
        let mut body = serde_json::json!({ "response": response.as_str() });
        if let Some(remember) = remember {
            body["remember"] = serde_json::Value::Bool(remember);
        }
        let res = self
            .request(reqwest::Method::POST, &path)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<bool>().await.unwrap_or(true))
    }

    // -----------------------------------------------------------------------
    // Diffs / files
    // -----------------------------------------------------------------------

    /// `GET /session/:id/diff?messageID=` → `FileDiff[]`
    pub async fn diff(
        &self,
        id: &str,
        message_id: Option<&str>,
    ) -> Result<Vec<FileDiff>, CoreError> {
        let mut path = format!("/session/{}/diff", Self::seg(id));
        if let Some(mid) = message_id {
            path.push_str(&format!("?messageID={}", Self::seg(mid)));
        }
        self.get_json(&path).await
    }

    /// `GET /file?path=` → directory listing (`path` empty = project root).
    ///
    /// The `path` query key is mandatory: the server answers `400 BadRequest`
    /// ("Missing key at [path]") when it is absent, so we always send it, even
    /// for the project root (empty value).
    pub async fn file_tree(&self, path: Option<&str>) -> Result<Vec<FileNode>, CoreError> {
        let url = format!(
            "{}/file?path={}",
            self.base_url,
            urlencoding_lite(path.unwrap_or(""))
        );
        let mut req = self.http.get(&url);
        if let Some(password) = &self.password {
            req = req.basic_auth(self.username.as_str(), Some(password));
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<Vec<FileNode>>().await?)
    }

    /// `GET /file/content?path=` → `FileContent`
    pub async fn file_content(&self, path: &str) -> Result<FileContent, CoreError> {
        let url = format!(
            "{}/file/content?path={}",
            self.base_url,
            urlencoding_lite(path)
        );
        let mut req = self.http.get(&url);
        if let Some(password) = &self.password {
            req = req.basic_auth(self.username.as_str(), Some(password));
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res.json::<FileContent>().await?)
    }

    /// `GET /file/status` → tracked-file status.
    pub async fn file_status(&self) -> Result<Vec<FileStatus>, CoreError> {
        self.get_json("/file/status").await
    }

    /// `GET /find/file?query=` → matching paths.
    pub async fn find_files(
        &self,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<String>, CoreError> {
        let mut path = format!("/find/file?query={}", urlencoding_lite(query));
        if let Some(limit) = limit {
            path.push_str(&format!("&limit={limit}"));
        }
        self.get_json(&path).await
    }

    /// `GET /find/symbol?query=` → matching symbols.
    pub async fn find_symbols(&self, query: &str) -> Result<Vec<serde_json::Value>, CoreError> {
        let path = format!("/find/symbol?query={}", urlencoding_lite(query));
        self.get_json(&path).await
    }

    /// `GET /find?pattern=` → files matching a glob pattern.
    pub async fn find_by_pattern(&self, pattern: &str) -> Result<Vec<String>, CoreError> {
        let path = format!("/find?pattern={}", urlencoding_lite(pattern));
        self.get_json(&path).await
    }

    // -----------------------------------------------------------------------
    // SSE
    // -----------------------------------------------------------------------

    /// `GET /event` — returns the raw response so the event hub can consume
    /// the byte stream (framing lives in [`crate::events`]).
    pub async fn event_stream(&self) -> Result<reqwest::Response, CoreError> {
        let mut req = self
            .http
            .get(self.url("/event"))
            .header(reqwest::header::ACCEPT, "text/event-stream");
        if let Some(password) = &self.password {
            req = req.basic_auth(self.username.as_str(), Some(password));
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res)
    }
}

/// Minimal query/form percent-encoding (RFC 3986 unreserved chars kept as is).
fn urlencoding_lite(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests — a hand rolled HTTP/1.1 mock keeps the core free of dev-only crates.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves exactly one request with a canned raw HTTP response, then closes.
    fn http_response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    struct MockServer {
        base_url: String,
        request: tokio::task::JoinHandle<String>,
    }

    impl MockServer {
        async fn start(status: &str, body: &str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let raw = http_response(status, body);
            let request = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut read = 0usize;
                // Read until the client stops sending for a short idle window,
                // so the request BODY is captured too (headers + body often
                // arrive in separate segments; the old loop stopped at the
                // header terminator and lost the body).
                loop {
                    match tokio::time::timeout(
                        Duration::from_millis(50),
                        socket.read(&mut buf[read..]),
                    )
                    .await
                    {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => read += n,
                        Ok(Err(_)) => break,
                        Err(_) => break, // idle: request is fully buffered
                    }
                    if read == buf.len() {
                        break;
                    }
                }
                let _ = socket.write_all(raw.as_bytes()).await;
                let _ = socket.shutdown().await;
                String::from_utf8_lossy(&buf[..read]).to_string()
            });
            Self {
                base_url: format!("http://{addr}"),
                request,
            }
        }

        async fn client(&self, password: Option<&str>) -> OpenCodeClient {
            OpenCodeClient::new(ClientConfig {
                base_url: self.base_url.clone(),
                password: password.map(str::to_owned),
                ..ClientConfig::default()
            })
            .unwrap()
        }

        async fn request_text(self) -> String {
            self.request.await.unwrap()
        }
    }

    #[tokio::test]
    async fn health_hits_the_right_path_and_parses_the_body() {
        let server = MockServer::start("200 OK", r#"{"healthy":true,"version":"1.9.0"}"#).await;
        let client = server.client(None).await;
        let health = client.health().await.unwrap();
        assert!(health.healthy);
        assert_eq!(health.version, "1.9.0");

        let req = server.request_text().await;
        assert!(req.starts_with("GET /global/health HTTP/1.1"), "got: {req}");
        assert!(
            !req.to_lowercase().contains("authorization:"),
            "no auth header expected"
        );
    }

    #[tokio::test]
    async fn basic_auth_header_is_sent_when_a_password_is_configured() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(Some("s3cret")).await;
        let sessions: Vec<Session> = client.list_sessions().await.unwrap();
        assert!(sessions.is_empty());

        let req = server.request_text().await;
        assert!(req.starts_with("GET /session HTTP/1.1"), "got: {req}");
        assert!(
            req.to_lowercase().contains("authorization: basic "),
            "expected basic auth, got: {req}"
        );
    }

    #[tokio::test]
    async fn session_ids_are_percent_encoded_in_the_path() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(None).await;
        let _ = client.session_children("ses a/b").await.unwrap();
        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /session/ses%20a%2Fb/children HTTP/1.1"),
            "got: {req}"
        );
    }

    #[tokio::test]
    async fn non_success_status_becomes_a_typed_api_error() {
        let server = MockServer::start("404 Not Found", r#"{"message":"session not found"}"#).await;
        let client = server.client(None).await;
        let err = client.session("missing").await.unwrap_err();
        match &err {
            CoreError::Api { status, message } => {
                assert_eq!(*status, 404);
                assert_eq!(message, "session not found");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
        assert_eq!(err.kind(), "api");
    }

    #[tokio::test]
    async fn statuses_deserialize_from_the_id_keyed_map() {
        let body = r#"{"ses_a":{"type":"busy"},"ses_b":{"type":"retry","attempt":1,"message":"x","next":5}}"#;
        let server = MockServer::start("200 OK", body).await;
        let client = server.client(None).await;
        let map = client.session_statuses().await.unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["ses_a"], SessionStatus::Busy);
        assert!(matches!(
            map["ses_b"],
            SessionStatus::Retry { attempt: 1, .. }
        ));
    }

    #[tokio::test]
    async fn respond_permission_omits_remember_when_none() {
        // PLAN §6.3: the `remember` field MUST be omitted when `None` so a
        // server that does not know the field never sees `"remember": null`.
        let server = MockServer::start("200 OK", "true").await;
        let client = server.client(None).await;
        client
            .respond_permission("ses_1", "perm_1", PermissionResponse::Once, None)
            .await
            .unwrap();
        let req = server.request_text().await;
        assert!(
            req.starts_with("POST /session/ses_1/permissions/perm_1 HTTP/1.1"),
            "got: {req}"
        );
        assert!(req.contains("\"response\":\"once\""), "got: {req}");
        assert!(
            !req.contains("remember"),
            "None must omit the field, got: {req}"
        );
    }

    #[tokio::test]
    async fn respond_permission_includes_remember_when_set() {
        let server = MockServer::start("200 OK", "true").await;
        let client = server.client(None).await;
        client
            .respond_permission("ses_1", "perm_2", PermissionResponse::Always, Some(true))
            .await
            .unwrap();
        let req = server.request_text().await;
        assert!(req.contains("\"remember\":true"), "got: {req}");
    }

    #[tokio::test]
    async fn client_rejects_a_malformed_base_url() {
        let err = OpenCodeClient::new(ClientConfig {
            base_url: "not-a-url".into(),
            ..ClientConfig::default()
        })
        .unwrap_err();
        assert_eq!(err.kind(), "config");
    }

    #[test]
    fn urlencoding_lite_keeps_slashes_and_encodes_spaces() {
        assert_eq!(urlencoding_lite("src/main.rs"), "src/main.rs");
        assert_eq!(urlencoding_lite("a b+c"), "a%20b%2Bc");
    }

    #[tokio::test]
    async fn file_tree_always_sends_the_path_query_key() {
        // The server rejects a missing `path` key with 400 "Missing key at [path]".
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(None).await;
        let nodes = client.file_tree(None).await.unwrap();
        assert!(nodes.is_empty());

        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /file?path= HTTP/1.1"),
            "root listing must include the path key, got: {req}"
        );
    }

    #[tokio::test]
    async fn file_tree_treats_an_empty_path_as_the_project_root() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(None).await;
        let _ = client.file_tree(Some("")).await.unwrap();

        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /file?path= HTTP/1.1"),
            "empty path must still send the key, got: {req}"
        );
    }

    #[tokio::test]
    async fn file_tree_percent_encodes_the_path_value() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(None).await;
        let _ = client.file_tree(Some("src/a b.rs")).await.unwrap();

        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /file?path=src/a%20b.rs HTTP/1.1"),
            "got: {req}"
        );
    }
}
