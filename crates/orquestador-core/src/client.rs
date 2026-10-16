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

use crate::backend::{AgentBackend, BackendEvent, EventStream, BACKEND_OPENCODE};
use crate::error::CoreError;
use crate::types::*;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Default base URL of `opencode serve`.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4096";
/// Default basic-auth user (`OPENCODE_SERVER_USERNAME` overrides it).
pub const DEFAULT_USERNAME: &str = "opencode";
/// Title given to the session the composer drives when none exists yet.
pub const DEFAULT_ROOT_TITLE: &str = "Orchestrator";
/// Request header that scopes a request to a project directory.
///
/// One `opencode serve` can serve several projects; sending this header on
/// **every** request (and on the SSE stream) is how the app selects which one to
/// work on without restarting the server.
pub const DIRECTORY_HEADER: &str = "x-opencode-directory";

/// Configuration for [`OpenCodeClient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub base_url: String,
    pub username: String,
    pub password: Option<String>,
    /// Absolute path of the project to scope every request to. `None` uses the
    /// server's own cwd project.
    pub directory: Option<String>,
    /// Optional display name used to attribute sessions this app creates
    /// (`[<member>] <title>`). `None` leaves newly created titles unattributed.
    pub member_name: Option<String>,
    pub request_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            username: DEFAULT_USERNAME.to_string(),
            password: None,
            directory: None,
            member_name: None,
            request_timeout: Duration::from_secs(30),
        }
    }
}

impl ClientConfig {
    /// Read the server configuration from the environment, falling back to the
    /// built-in defaults when a variable is missing or empty:
    ///
    /// * `OPENCODE_SERVER_BASE_URL` (optional, defaults to `127.0.0.1:4096`;
    ///   `OPENCODE_SERVER_URL` is accepted as a legacy alias),
    /// * `OPENCODE_SERVER_USERNAME` (optional, defaults to `opencode`),
    /// * `OPENCODE_SERVER_PASSWORD` (optional, enables basic auth).
    pub fn from_env_or_default() -> Self {
        let mut cfg = ClientConfig::default();
        let url = std::env::var("OPENCODE_SERVER_BASE_URL")
            .or_else(|_| std::env::var("OPENCODE_SERVER_URL"));
        if let Ok(url) = url {
            if !url.trim().is_empty() {
                cfg.base_url = url.trim().to_string();
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
        cfg
    }
}

/// Typed OpenCode server client. Cheap to clone (shares one connection pool).
#[derive(Debug, Clone)]
pub struct OpenCodeClient {
    http: reqwest::Client,
    /// Dedicated client for the long-lived SSE stream in [`crate::events`].
    ///
    /// `reqwest`'s [`reqwest::ClientBuilder::timeout`] is a TOTAL deadline
    /// ("applied from when the request starts connecting until the response
    /// body has finished"), so a client that sets one aborts `/event` after
    /// that long — the app logged an endless
    /// `event stream error: Transport error: error decoding response body`.
    /// This client therefore sets only `connect_timeout`; liveness is the
    /// event hub's job (the server heartbeats every 10s, see [`crate::events`]).
    stream_http: reqwest::Client,
    base_url: String,
    username: String,
    password: Option<String>,
    /// Project scope sent as [`DIRECTORY_HEADER`] on every request.
    directory: Option<String>,
    /// Optional display name used to attribute sessions this app creates.
    member_name: Option<String>,
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
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        // No total timeout: the SSE stream must outlive any request deadline.
        let stream_http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            http,
            stream_http,
            base_url,
            username: config.username,
            password: config.password,
            directory: config.directory,
            member_name: config.member_name,
        })
    }

    /// Build a client from the environment:
    /// `OPENCODE_SERVER_BASE_URL` (optional, defaults to `127.0.0.1:4096`;
    /// `OPENCODE_SERVER_URL` is accepted as a legacy alias),
    /// `OPENCODE_SERVER_PASSWORD` (optional, enables basic auth),
    /// `OPENCODE_SERVER_USERNAME` (optional, defaults to `opencode`).
    pub fn from_env() -> Result<Self, CoreError> {
        Self::new(ClientConfig::from_env_or_default())
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
        if let Some(directory) = &self.directory {
            req = req.header(DIRECTORY_HEADER, directory.as_str());
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

    /// `GET /orquestador/present` → online members of the shared workspace.
    ///
    /// Presence is **feature-detected, never assumed**: the route exists only
    /// behind `orquestador-proxy`, so talking to `opencode` directly answers
    /// `404`. Any failure — a non-success status, a transport error, or a body
    /// that does not decode — degrades to an empty list rather than an error, so
    /// a proxy-less setup shows "no presence" instead of breaking the app.
    pub async fn presence(&self) -> Result<Vec<PresenceEntry>, CoreError> {
        let res = match self
            .request(reqwest::Method::GET, "/orquestador/present")
            .send()
            .await
        {
            Ok(res) => res,
            Err(_) => return Ok(Vec::new()),
        };
        if !res.status().is_success() {
            return Ok(Vec::new());
        }
        Ok(res.json::<Vec<PresenceEntry>>().await.unwrap_or_default())
    }

    /// `GET /orquestador/whoami` → proxy reports the pinned `project` (or `null`).
    ///
    /// The method must **never** return an error. On transport error, non-2xx
    /// status, unparseable body, `null`, or a missing `project` field, degrade
    /// to `Ok(None)`. A `404` means "no proxy in front"; `401` means "no credentials".
    pub async fn workspace_scope(&self) -> Result<Option<String>, CoreError> {
        let res = match self
            .request(reqwest::Method::GET, "/orquestador/whoami")
            .send()
            .await
        {
            Ok(res) => res,
            Err(_) => return Ok(None),
        };
        if !res.status().is_success() {
            return Ok(None);
        }
        #[derive(serde::Deserialize)]
        struct Whoami {
            #[serde(default)]
            project: Option<Option<String>>,
        }
        match res.json::<Whoami>().await {
            Ok(w) => Ok(w.project.flatten()),
            Err(_) => Ok(None),
        }
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

    /// `GET /permission` → every pending approval across all sessions.
    ///
    /// This is the bootstrap/reconcile source of truth: unlike the
    /// `permission.updated` SSE event, it survives a stream that was down when
    /// the approval was requested.
    pub async fn pending_permissions(&self) -> Result<Vec<PermissionRequest>, CoreError> {
        self.get_json("/permission").await
    }

    /// `POST /permission/:requestID/reply` — the **request-scoped** reply route.
    ///
    /// This is the canonical route for the `per_…` ids returned by
    /// [`Self::pending_permissions`]. Its body uses the field name `reply` (not
    /// `response`) and is `additionalProperties: false`, so `message` is added
    /// only when present — a JSON `null` would be rejected.
    pub async fn reply_permission(
        &self,
        request_id: &str,
        response: PermissionResponse,
        message: Option<&str>,
    ) -> Result<bool, CoreError> {
        let path = format!("/permission/{}/reply", Self::seg(request_id));
        let mut body = serde_json::json!({ "reply": response.as_str() });
        if let Some(message) = message {
            body["message"] = serde_json::Value::String(message.to_string());
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

    /// `POST /session/:id/permissions/:permissionID`
    ///
    /// Routes by id form: a `per_…` id comes from [`Self::pending_permissions`]
    /// and belongs to the request-scoped `/permission/:id/reply` route (the
    /// session-scoped route answers it with `404`). The two routes also disagree
    /// on the body field name, so request ids are delegated to
    /// [`Self::reply_permission`]; the global request body carries no `remember`.
    pub async fn respond_permission(
        &self,
        id: &str,
        permission_id: &str,
        response: PermissionResponse,
        remember: Option<bool>,
    ) -> Result<bool, CoreError> {
        if permission_id.starts_with("per_") {
            return self.reply_permission(permission_id, response, None).await;
        }
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
        // Routed through `request()` so basic auth and the project scope header
        // are applied here exactly as on every other endpoint.
        let path = format!("/file?path={}", urlencoding_lite(path.unwrap_or("")));
        self.get_json(&path).await
    }

    /// `GET /file/content?path=` → `FileContent`
    pub async fn file_content(&self, path: &str) -> Result<FileContent, CoreError> {
        let path = format!("/file/content?path={}", urlencoding_lite(path));
        self.get_json(&path).await
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
    // Providers
    // -----------------------------------------------------------------------

    /// `GET /provider` → available providers, their models, the configured
    /// default model per provider, and the ids already authenticated.
    ///
    /// The typed [`ProviderList`] deliberately drops the credential-bearing
    /// `key`/`options` fields (see its docs); nothing secret leaves this call.
    pub async fn providers(&self) -> Result<ProviderList, CoreError> {
        self.get_json("/provider").await
    }

    /// `GET /provider/auth` → the supported auth methods per provider, keyed by
    /// provider id.
    pub async fn provider_auth(
        &self,
    ) -> Result<HashMap<String, Vec<ProviderAuthMethod>>, CoreError> {
        self.get_json("/provider/auth").await
    }

    // -----------------------------------------------------------------------
    // SSE
    // -----------------------------------------------------------------------

    /// `GET /event` — returns the raw response so the event hub can consume
    /// the byte stream (framing lives in [`crate::events`]).
    ///
    /// Uses the stream client (no total timeout): the response body stays open
    /// for as long as the server keeps it, which is the whole point of SSE.
    pub async fn event_stream(&self) -> Result<reqwest::Response, CoreError> {
        let mut req = self
            .stream_http
            .get(self.url("/event"))
            .header(reqwest::header::ACCEPT, "text/event-stream");
        if let Some(password) = &self.password {
            req = req.basic_auth(self.username.as_str(), Some(password));
        }
        if let Some(directory) = &self.directory {
            req = req.header(DIRECTORY_HEADER, directory.as_str());
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(CoreError::from_response(res).await);
        }
        Ok(res)
    }

    /// `GET /event` framed into decoded [`BackendEvent`]s.
    ///
    /// Owns the SSE framing that used to live in [`crate::events`]: the raw byte
    /// stream is parsed by `eventsource`, each `data` payload is decoded with
    /// [`decode_payload`], and the pair is yielded. A frame the server sends
    /// that does not decode to a typed event is skipped; a transport/parse
    /// error ends the stream with [`CoreError::Stream`] so the hub reconnects.
    pub async fn stream_events(&self) -> Result<EventStream, CoreError> {
        let response = self.event_stream().await?;
        let stream = response.bytes_stream().eventsource().filter_map(|frame| async move {
            let frame = match frame {
                Ok(frame) => frame,
                Err(e) => return Some(Err(CoreError::Stream(e.to_string()))),
            };
            let event_name = if frame.event.is_empty() {
                None
            } else {
                Some(frame.event.as_str())
            };
            let (event, raw) = decode_payload(&frame.data, event_name)?;
            Some(Ok(BackendEvent { event, raw }))
        });
        Ok(Box::pin(stream))
    }
}

/// Decode one SSE `data` payload.
///
/// Handles the three shapes observed/expected from the server:
/// 1. `{"type": "...", "properties": {...}}`          — plain event
/// 2. `{"directory": "...", "payload": {...event...}}` — `/global/event` wrapper
/// 3. bare JSON + an SSE `event:` line                 — fallback: synthesise `type`
///
/// Returns the typed event **and** the raw payload to forward to the webview.
pub fn decode_payload(data: &str, event_name: Option<&str>) -> Option<(Event, Value)> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut value: Value = serde_json::from_str(trimmed).ok()?;

    // Unwrap the /global/event envelope if present.
    if value.get("payload").map(Value::is_object).unwrap_or(false)
        && value.get("directory").is_some()
    {
        value = value.get("payload").cloned()?;
    }

    if !value.is_object() {
        return None;
    }

    // No `type` on the payload but the SSE `event:` line names it → synthesise.
    if value.get("type").is_none() {
        let name = event_name.filter(|n| !n.is_empty())?;
        let mut obj = serde_json::Map::new();
        obj.insert("type".to_string(), Value::String(name.to_string()));
        obj.insert("properties".to_string(), value);
        value = Value::Object(obj);
    }

    let typed = serde_json::from_value::<Event>(value.clone()).unwrap_or(Event::Unknown);
    Some((typed, value))
}

/// [`OpenCodeClient`] is the [`AgentBackend`] implementation for `opencode serve`.
///
/// Every method delegates to its inherent counterpart via a fully-qualified call
/// (`OpenCodeClient::health(self)`) so the inherent and trait methods, which
/// share names, never resolve ambiguously. `stream_events` is the one method
/// that lives only here and adds SSE framing on top of the raw `event_stream`.
#[async_trait]
impl AgentBackend for OpenCodeClient {
    fn id(&self) -> &'static str {
        BACKEND_OPENCODE
    }

    fn member_name(&self) -> Option<&str> {
        // Same effective-member chain as the webview (`identity.ts`):
        // explicit display name, else the configured username.
        self.member_name.as_deref().or(Some(self.username.as_str()))
    }

    async fn health(&self) -> Result<Health, CoreError> {
        OpenCodeClient::health(self).await
    }

    async fn agents(&self) -> Result<Vec<Agent>, CoreError> {
        OpenCodeClient::agents(self).await
    }

    async fn projects(&self) -> Result<Vec<Project>, CoreError> {
        OpenCodeClient::projects(self).await
    }

    async fn current_project(&self) -> Result<Project, CoreError> {
        OpenCodeClient::current_project(self).await
    }

    async fn presence(&self) -> Result<Vec<PresenceEntry>, CoreError> {
        OpenCodeClient::presence(self).await
    }

    async fn list_sessions(&self) -> Result<Vec<Session>, CoreError> {
        OpenCodeClient::list_sessions(self).await
    }

    async fn create_session(&self, body: &CreateSessionBody) -> Result<Session, CoreError> {
        OpenCodeClient::create_session(self, body).await
    }

    async fn session_statuses(&self) -> Result<HashMap<String, SessionStatus>, CoreError> {
        OpenCodeClient::session_statuses(self).await
    }

    async fn session(&self, id: &str) -> Result<Session, CoreError> {
        OpenCodeClient::session(self, id).await
    }

    async fn session_children(&self, id: &str) -> Result<Vec<Session>, CoreError> {
        OpenCodeClient::session_children(self, id).await
    }

    async fn delete_session(&self, id: &str) -> Result<bool, CoreError> {
        OpenCodeClient::delete_session(self, id).await
    }

    async fn rename_session(&self, id: &str, title: &str) -> Result<Session, CoreError> {
        OpenCodeClient::rename_session(self, id, title).await
    }

    async fn todos(&self, id: &str) -> Result<Vec<Todo>, CoreError> {
        OpenCodeClient::todos(self, id).await
    }

    async fn abort(&self, id: &str) -> Result<bool, CoreError> {
        OpenCodeClient::abort(self, id).await
    }

    async fn prompt(&self, id: &str, body: &PromptBody) -> Result<MessageBundle, CoreError> {
        OpenCodeClient::prompt(self, id, body).await
    }

    async fn prompt_async(&self, id: &str, body: &PromptBody) -> Result<(), CoreError> {
        OpenCodeClient::prompt_async(self, id, body).await
    }

    async fn messages(
        &self,
        id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<MessageBundle>, CoreError> {
        OpenCodeClient::messages(self, id, limit).await
    }

    async fn message(&self, id: &str, message_id: &str) -> Result<MessageBundle, CoreError> {
        OpenCodeClient::message(self, id, message_id).await
    }

    async fn command(&self, id: &str, body: &CommandBody) -> Result<MessageBundle, CoreError> {
        OpenCodeClient::command(self, id, body).await
    }

    async fn pending_permissions(&self) -> Result<Vec<PermissionRequest>, CoreError> {
        OpenCodeClient::pending_permissions(self).await
    }

    async fn reply_permission(
        &self,
        request_id: &str,
        response: PermissionResponse,
        message: Option<&str>,
    ) -> Result<bool, CoreError> {
        OpenCodeClient::reply_permission(self, request_id, response, message).await
    }

    async fn respond_permission(
        &self,
        id: &str,
        permission_id: &str,
        response: PermissionResponse,
        remember: Option<bool>,
    ) -> Result<bool, CoreError> {
        OpenCodeClient::respond_permission(self, id, permission_id, response, remember).await
    }

    async fn diff(&self, id: &str, message_id: Option<&str>) -> Result<Vec<FileDiff>, CoreError> {
        OpenCodeClient::diff(self, id, message_id).await
    }

    async fn file_tree(&self, path: Option<&str>) -> Result<Vec<FileNode>, CoreError> {
        OpenCodeClient::file_tree(self, path).await
    }

    async fn file_content(&self, path: &str) -> Result<FileContent, CoreError> {
        OpenCodeClient::file_content(self, path).await
    }

    async fn file_status(&self) -> Result<Vec<FileStatus>, CoreError> {
        OpenCodeClient::file_status(self).await
    }

    async fn find_files(&self, query: &str, limit: Option<u32>) -> Result<Vec<String>, CoreError> {
        OpenCodeClient::find_files(self, query, limit).await
    }

    async fn find_symbols(&self, query: &str) -> Result<Vec<Value>, CoreError> {
        OpenCodeClient::find_symbols(self, query).await
    }

    async fn find_by_pattern(&self, pattern: &str) -> Result<Vec<String>, CoreError> {
        OpenCodeClient::find_by_pattern(self, pattern).await
    }

    async fn stream_events(&self) -> Result<EventStream, CoreError> {
        OpenCodeClient::stream_events(self).await
    }
}

/// HTTP-status classification for [`CoreError`].
///
/// `CoreError` is the crate-wide error type; this helper sits next to the HTTP
/// layer that produces [`CoreError::Api`] so the shell (or any caller) can
/// recognize a `404` without string-matching the server message.
impl CoreError {
    /// True when the server answered `404` — the resource no longer exists.
    pub fn is_not_found(&self) -> bool {
        matches!(self, CoreError::Api { status: 404, .. })
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
    async fn directory_header_is_sent_when_a_directory_is_configured() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = OpenCodeClient::new(ClientConfig {
            base_url: server.base_url.clone(),
            directory: Some("/home/dev/proj".to_string()),
            ..ClientConfig::default()
        })
        .unwrap();
        let sessions: Vec<Session> = client.list_sessions().await.unwrap();
        assert!(sessions.is_empty());

        let req = server.request_text().await;
        assert!(
            req.to_lowercase()
                .contains("x-opencode-directory: /home/dev/proj"),
            "expected the project scope header, got: {req}"
        );
    }

    #[tokio::test]
    async fn directory_header_is_absent_when_no_directory_is_configured() {
        let server = MockServer::start("200 OK", r#"[]"#).await;
        let client = server.client(None).await; // directory defaults to None
        let _ = client.list_sessions().await.unwrap();

        let req = server.request_text().await;
        assert!(
            !req.to_lowercase().contains("x-opencode-directory"),
            "no scope header expected, got: {req}"
        );
    }

    #[tokio::test]
    async fn directory_header_is_sent_on_the_event_stream() {
        // The SSE stream is scoped the same way, so a project switch re-subscribes
        // a fresh stream that already targets the new directory.
        let server = MockServer::start("200 OK", "").await;
        let client = OpenCodeClient::new(ClientConfig {
            base_url: server.base_url.clone(),
            directory: Some("/home/dev/proj".to_string()),
            ..ClientConfig::default()
        })
        .unwrap();
        let _ = client.event_stream().await.unwrap();

        let req = server.request_text().await;
        assert!(req.starts_with("GET /event HTTP/1.1"), "got: {req}");
        assert!(
            req.to_lowercase()
                .contains("x-opencode-directory: /home/dev/proj"),
            "the SSE request must carry the project scope, got: {req}"
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
    async fn pending_permissions_hits_get_permission_and_parses_the_list() {
        let body = r#"[{"id":"per_1","sessionID":"ses_a","permission":"bash",
            "patterns":["rm -rf /"],"metadata":{"command":"rm -rf /"},
            "always":["rm *"],"tool":{"messageID":"m","callID":"c"}}]"#;
        let server = MockServer::start("200 OK", body).await;
        let client = server.client(None).await;
        let perms = client.pending_permissions().await.unwrap();
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].id, "per_1");
        assert_eq!(perms[0].session_id, "ses_a");
        assert_eq!(perms[0].permission, "bash");

        let req = server.request_text().await;
        assert!(req.starts_with("GET /permission HTTP/1.1"), "got: {req}");
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
    async fn reply_permission_posts_to_the_request_scoped_route_without_message() {
        // `GET /permission` returns `per_…` ids that belong to this route, whose
        // body uses `reply` (not `response`) and rejects extra properties, so
        // `message` must be omitted rather than sent as `null`.
        let server = MockServer::start("200 OK", "true").await;
        let client = server.client(None).await;
        let ok = client
            .reply_permission("per_126b00d0f001cyU", PermissionResponse::Once, None)
            .await
            .unwrap();
        assert!(ok);

        let req = server.request_text().await;
        assert!(
            req.starts_with("POST /permission/per_126b00d0f001cyU/reply HTTP/1.1"),
            "got: {req}"
        );
        assert!(req.contains("\"reply\":\"once\""), "got: {req}");
        assert!(
            !req.contains("message") && !req.contains("response"),
            "got: {req}"
        );
    }

    #[tokio::test]
    async fn reply_permission_includes_message_when_some() {
        let server = MockServer::start("200 OK", "true").await;
        let client = server.client(None).await;
        client
            .reply_permission("per_2", PermissionResponse::Reject, Some("not this time"))
            .await
            .unwrap();
        let req = server.request_text().await;
        assert!(req.contains("\"reply\":\"reject\""), "got: {req}");
        assert!(req.contains("\"message\":\"not this time\""), "got: {req}");
    }

    #[tokio::test]
    async fn respond_permission_routes_per_request_ids_to_the_reply_endpoint() {
        // A `per_…` id came from `GET /permission`; the session-scoped route
        // answers it with `404 Permission request not found`.
        let server = MockServer::start("200 OK", "true").await;
        let client = server.client(None).await;
        client
            .respond_permission("ses_1", "per_abc", PermissionResponse::Once, None)
            .await
            .unwrap();
        let req = server.request_text().await;
        assert!(
            req.starts_with("POST /permission/per_abc/reply HTTP/1.1"),
            "got: {req}"
        );
        assert!(req.contains("\"reply\":\"once\""), "got: {req}");
        assert!(!req.contains("\"response\""), "got: {req}");
        assert!(!req.contains("/session/"), "got: {req}");
    }

    #[test]
    fn is_not_found_classifies_http_404_without_string_matching() {
        assert!(CoreError::Api {
            status: 404,
            message: "Permission request not found: per_x".into(),
        }
        .is_not_found());
        assert!(!CoreError::Api {
            status: 500,
            message: "boom".into(),
        }
        .is_not_found());
        assert!(!CoreError::Config("nope".into()).is_not_found());
    }

    #[tokio::test]
    async fn reply_permission_surfaces_a_typed_404() {
        let server = MockServer::start(
            "404 Not Found",
            r#"{"message":"Permission request not found: per_x"}"#,
        )
        .await;
        let client = server.client(None).await;
        let err = client
            .reply_permission("per_x", PermissionResponse::Once, None)
            .await
            .unwrap_err();
        assert!(err.is_not_found(), "got: {err:?}");
        assert_eq!(err.kind(), "api");
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

    #[test]
    fn decode_handles_the_three_wire_shapes() {
        // 1. plain typed event
        let (typed, raw) = decode_payload(
            r#"{"type":"session.idle","properties":{"sessionID":"s"}}"#,
            None,
        )
        .expect("decodes");
        assert!(matches!(typed, Event::SessionIdle { .. }));
        assert_eq!(raw["type"], "session.idle");

        // 2. /global/event style envelope
        let (typed, raw) = decode_payload(
            r#"{"directory":"/proj","payload":{"type":"permission.updated","properties":{"id":"p","type":"edit","sessionID":"s","messageID":"m","title":"t","metadata":{},"time":{"created":1}}}}"#,
            None,
        )
        .expect("decodes");
        assert!(matches!(typed, Event::PermissionUpdated { .. }));
        assert_eq!(raw["type"], "permission.updated");
        assert!(raw.get("directory").is_none(), "envelope must be unwrapped");

        // 3. bare payload + SSE event name
        let (typed, raw) =
            decode_payload(r#"{"sessionID":"s"}"#, Some("session.idle")).expect("decodes");
        assert!(matches!(typed, Event::SessionIdle { .. }));
        assert_eq!(raw["type"], "session.idle");

        // Garbage and empty payloads are dropped, never panic.
        assert!(decode_payload("", None).is_none());
        assert!(decode_payload("{not json", None).is_none());
        assert!(decode_payload("42", None).is_none());
        assert!(decode_payload(r#"{"nope":1}"#, None).is_none());
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

    /// Serves one long-lived chunked SSE response, mirroring `opencode serve`:
    /// handshake immediately, then a frame after `gap`, then the connection
    /// stays open (frames every `gap`) until the client goes away.
    async fn spawn_slow_sse_server(gap: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                        cache-control: no-cache\r\ntransfer-encoding: chunked\r\n\r\n";
            if socket.write_all(head.as_bytes()).await.is_err() {
                return;
            }
            // Chunked framing: <hex-len>\r\n<payload>\r\n
            let chunk = |body: &str| format!("{:x}\r\n{}\r\n", body.len(), body);
            let frames = [
                "data: {\"type\":\"server.connected\",\"properties\":{}}\n\n",
                "data: {\"type\":\"server.heartbeat\",\"properties\":{}}\n\n",
            ];
            for body in frames {
                if socket.write_all(chunk(body).as_bytes()).await.is_err() {
                    return;
                }
                tokio::time::sleep(gap).await;
            }
            tokio::time::sleep(gap).await;
            let _ = socket.write_all(b"0\r\n\r\n").await;
        });
        format!("http://{addr}")
    }

    /// Regression: `reqwest`'s client-level `timeout` is a TOTAL deadline. It
    /// "is applied from when the request starts connecting until the response
    /// body has finished", so on the infinite `/event` stream it aborted the
    /// body after `request_timeout`, and the hub logged the endless
    /// `event stream error: Transport error: error decoding response body`.
    #[tokio::test]
    async fn event_stream_is_not_killed_by_the_request_timeout() {
        use eventsource_stream::Eventsource;
        use futures_util::StreamExt;

        let gap = Duration::from_millis(800);
        let base = spawn_slow_sse_server(gap).await;
        let client = OpenCodeClient::new(ClientConfig {
            base_url: base,
            request_timeout: Duration::from_millis(250),
            ..ClientConfig::default()
        })
        .unwrap();

        let response = client.event_stream().await.unwrap();
        let mut stream = response.bytes_stream().eventsource();

        let first = stream.next().await.expect("first frame").expect("no error");
        assert!(first.data.contains("server.connected"));

        // Arrives after `request_timeout` has elapsed since the request began.
        // With a total timeout on the stream client this read fails the same
        // way the app did on screen.
        let second = stream
            .next()
            .await
            .expect("second frame")
            .expect("no error");
        assert!(second.data.contains("server.heartbeat"));
    }

    #[tokio::test]
    async fn presence_hits_the_proxy_route_and_parses_the_list() {
        let body = r#"[{"name":"ana","role":"editor","idleMs":1200},
            {"name":"bob","role":"viewer","idleMs":0}]"#;
        let server = MockServer::start("200 OK", body).await;
        let client = server.client(None).await;
        let entries = client.presence().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "ana");
        assert_eq!(entries[0].role, "editor");
        assert_eq!(entries[0].idle_ms, 1200);
        assert_eq!(entries[1].role, "viewer");

        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /orquestador/present HTTP/1.1"),
            "got: {req}"
        );
    }

    #[tokio::test]
    async fn presence_degrades_to_empty_on_404_without_a_proxy() {
        // A direct `opencode serve` has no `/orquestador/…` route.
        let server = MockServer::start("404 Not Found", r#"{"message":"not found"}"#).await;
        let client = server.client(None).await;
        let entries = client.presence().await.expect("presence never errors");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn presence_degrades_to_empty_on_a_malformed_body() {
        let server = MockServer::start("200 OK", "{not json").await;
        let client = server.client(None).await;
        let entries = client.presence().await.expect("presence never errors");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn presence_degrades_to_empty_on_a_server_error() {
        let server = MockServer::start("500 Internal Server Error", r#"{"message":"boom"}"#).await;
        let client = server.client(None).await;
        let entries = client.presence().await.expect("presence never errors");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn presence_degrades_to_empty_when_the_connection_is_refused() {
        // No server behind the URL at all: a transport error, not an HTTP status.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let client = OpenCodeClient::new(ClientConfig {
            base_url: format!("http://{addr}"),
            ..ClientConfig::default()
        })
        .unwrap();
        let entries = client.presence().await.expect("presence never errors");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn workspace_scope_parses_the_pin() {
        let server = MockServer::start(
            "200 OK",
            r#"{"name":"alice","role":"editor","project":"/home/owner/team-app"}"#,
        )
        .await;
        let client = server.client(None).await;
        let scope = client.workspace_scope().await.expect("never errors");
        assert_eq!(scope.as_deref(), Some("/home/owner/team-app"));
        let req = server.request_text().await;
        assert!(
            req.starts_with("GET /orquestador/whoami HTTP/1.1"),
            "got: {req}"
        );
    }

    #[tokio::test]
    async fn workspace_scope_is_none_on_404() {
        let server = MockServer::start("404 Not Found", r#"{"message":"not found"}"#).await;
        let client = server.client(None).await;
        let scope = client.workspace_scope().await.expect("never errors");
        assert!(scope.is_none());
    }

    #[tokio::test]
    async fn workspace_scope_is_none_on_transport_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let client = OpenCodeClient::new(ClientConfig {
            base_url: format!("http://{addr}"),
            ..ClientConfig::default()
        })
        .unwrap();
        let scope = client.workspace_scope().await.expect("never errors");
        assert!(scope.is_none());
    }

    #[tokio::test]
    async fn workspace_scope_is_none_on_malformed_body() {
        let server = MockServer::start("200 OK", "{not json").await;
        let client = server.client(None).await;
        let scope = client.workspace_scope().await.expect("never errors");
        assert!(scope.is_none());
    }

    #[tokio::test]
    async fn workspace_scope_is_none_when_project_is_null() {
        let server = MockServer::start(
            "200 OK",
            r#"{"name":"alice","role":"editor","project":null}"#,
        )
        .await;
        let client = server.client(None).await;
        let scope = client.workspace_scope().await.expect("never errors");
        assert!(scope.is_none());
    }

    #[tokio::test]
    async fn provider_list_parses_and_ignores_credential_fields() {
        // `key`/`options` are present on the wire and deliberately NOT populated
        // here; the DTO must not model them, and serde must ignore the extras.
        let body = r#"{
            "all": [{
                "id": "anthropic",
                "name": "Anthropic",
                "source": "env",
                "env": ["ANTHROPIC_API_KEY"],
                "key": "",
                "options": {},
                "models": {
                    "claude-sonnet-4": {
                        "id": "claude-sonnet-4",
                        "providerID": "anthropic",
                        "name": "Claude Sonnet 4",
                        "family": "claude-sonnet"
                    }
                }
            }],
            "default": {"anthropic": "claude-sonnet-4"},
            "connected": ["anthropic"]
        }"#;
        let server = MockServer::start("200 OK", body).await;
        let client = server.client(None).await;
        let providers = client.providers().await.unwrap();

        assert_eq!(providers.all.len(), 1);
        let p = &providers.all[0];
        assert_eq!(p.id, "anthropic");
        assert_eq!(p.name, "Anthropic");
        assert_eq!(p.source, "env");
        assert_eq!(p.env, vec!["ANTHROPIC_API_KEY".to_string()]);
        let model = &p.models["claude-sonnet-4"];
        assert_eq!(model.id, "claude-sonnet-4");
        assert_eq!(model.name, "Claude Sonnet 4");
        assert_eq!(model.provider_id, "anthropic");
        assert_eq!(model.family.as_deref(), Some("claude-sonnet"));
        assert_eq!(
            providers
                .default_models
                .get("anthropic")
                .map(String::as_str),
            Some("claude-sonnet-4")
        );
        assert_eq!(providers.connected, vec!["anthropic".to_string()]);

        let req = server.request_text().await;
        assert!(req.starts_with("GET /provider HTTP/1.1"), "got: {req}");
    }

    #[tokio::test]
    async fn provider_auth_parses_the_method_map() {
        let body = r#"{
            "anthropic": [{"type": "api", "label": "API Key"}],
            "openai": [
                {"type": "oauth", "label": "ChatGPT Pro/Plus"},
                {"type": "api", "label": "API Key"}
            ]
        }"#;
        let server = MockServer::start("200 OK", body).await;
        let client = server.client(None).await;
        let methods = client.provider_auth().await.unwrap();

        assert_eq!(methods["anthropic"].len(), 1);
        assert_eq!(methods["anthropic"][0].auth_type, "api");
        assert_eq!(methods["anthropic"][0].label, "API Key");
        assert_eq!(methods["openai"].len(), 2);

        let req = server.request_text().await;
        assert!(req.starts_with("GET /provider/auth HTTP/1.1"), "got: {req}");
    }
}
