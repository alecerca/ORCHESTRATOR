//! Backend-agnostic agent interface.
//!
//! The event hub and the Tauri shell talk to an agent through [`AgentBackend`]
//! rather than a concrete [`OpenCodeClient`](crate::client::OpenCodeClient). The
//! OpenCode implementation lives in [`crate::client`]; a second backend (a CLI,
//! a proxy) can be dropped in without touching the reducer or the shell.
//!
//! The trait mirrors the inherent `OpenCodeClient` surface 1:1 so its `impl` is
//! pure delegation, with one addition: [`AgentBackend::stream_events`] owns SSE
//! framing and yields decoded [`BackendEvent`]s, so the hub no longer knows the
//! streaming protocol.

use crate::error::CoreError;
use crate::types::*;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

/// Stable id of the built-in OpenCode backend.
pub const BACKEND_OPENCODE: &str = "opencode";

/// One decoded event from a backend's event stream.
///
/// * `event` is the typed event the state reducer consumes (see
///   [`crate::events::changes_state`]).
/// * `raw` is the untouched JSON forwarded to the webview as
///   [`CoreMessage::RawEvent`](crate::events::CoreMessage::RawEvent) for the
///   message/parts stream.
///
/// A non-OpenCode backend must synthesize a `raw` payload the UI understands;
/// evolving that shared shape is a later task.
#[derive(Debug, Clone)]
pub struct BackendEvent {
    pub event: Event,
    pub raw: Value,
}

/// A boxed, `Send` stream of decoded backend events.
pub type EventStream = Pin<Box<dyn futures_util::Stream<Item = Result<BackendEvent, CoreError>> + Send>>;

/// Everything the event hub and the shell need from an agent backend.
///
/// `async fn` in an edition-2021 trait is not dyn-compatible, so the methods are
/// `#[async_trait]`-desugared to boxed futures (acceptable here: no hot loop).
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Stable backend id (e.g. [`BACKEND_OPENCODE`]).
    fn id(&self) -> &'static str;

    /// Optional display name used to attribute sessions this app creates
    /// (`[<member>] <title>`). `None` leaves newly created titles unattributed.
    fn member_name(&self) -> Option<&str> {
        None
    }

    // health / discovery
    async fn health(&self) -> Result<Health, CoreError>;
    async fn agents(&self) -> Result<Vec<Agent>, CoreError>;

    // providers / models
    /// `GET /provider` — the providers the server knows, their models, the
    /// configured default model per provider, and the ids already authenticated.
    ///
    /// The typed [`ProviderList`] deliberately drops the credential-bearing
    /// `key`/`options` fields the raw response carries.
    async fn providers(&self) -> Result<ProviderList, CoreError>;

    /// `GET /provider/auth` — the auth methods each provider supports, keyed by
    /// provider id.
    async fn provider_auth(&self) -> Result<HashMap<String, Vec<ProviderAuthMethod>>, CoreError>;

    // projects (one server can serve several; the client scopes each request)
    async fn projects(&self) -> Result<Vec<Project>, CoreError>;
    async fn current_project(&self) -> Result<Project, CoreError>;

    /// Online members of the shared workspace (`GET /orquestador/present`).
    ///
    /// Presence is **feature-detected, never assumed**: only a backend behind
    /// `orquestador-proxy` has anyone to report, while a plain CLI backend (or a
    /// direct `opencode` connection) simply has nobody. The default is therefore
    /// an empty list — an absent presence surface must never be an error.
    async fn presence(&self) -> Result<Vec<PresenceEntry>, CoreError> {
        Ok(Vec::new())
    }

    /// The project the workspace is pinned to, or `None` when the app decides.
    async fn workspace_scope(&self) -> Result<Option<String>, CoreError> {
        Ok(None)
    }

    // sessions
    async fn list_sessions(&self) -> Result<Vec<Session>, CoreError>;
    async fn create_session(&self, body: &CreateSessionBody) -> Result<Session, CoreError>;
    async fn session_statuses(&self) -> Result<HashMap<String, SessionStatus>, CoreError>;
    async fn session(&self, id: &str) -> Result<Session, CoreError>;
    async fn session_children(&self, id: &str) -> Result<Vec<Session>, CoreError>;
    async fn delete_session(&self, id: &str) -> Result<bool, CoreError>;
    async fn rename_session(&self, id: &str, title: &str) -> Result<Session, CoreError>;
    /// Record `model` as the session's effective model (`POST
    /// /api/session/:id/model`), so a later prompt with no override keeps using
    /// it and a known baseline can restore the default.
    async fn set_session_model(&self, id: &str, model: &ModelRef) -> Result<(), CoreError>;
    async fn todos(&self, id: &str) -> Result<Vec<Todo>, CoreError>;
    async fn abort(&self, id: &str) -> Result<bool, CoreError>;

    // prompts / messages
    async fn prompt(&self, id: &str, body: &PromptBody) -> Result<MessageBundle, CoreError>;
    async fn prompt_async(&self, id: &str, body: &PromptBody) -> Result<(), CoreError>;
    async fn messages(&self, id: &str, limit: Option<u32>) -> Result<Vec<MessageBundle>, CoreError>;
    async fn message(&self, id: &str, message_id: &str) -> Result<MessageBundle, CoreError>;
    async fn command(&self, id: &str, body: &CommandBody) -> Result<MessageBundle, CoreError>;

    // permissions
    async fn pending_permissions(&self) -> Result<Vec<PermissionRequest>, CoreError>;
    async fn reply_permission(
        &self,
        request_id: &str,
        response: PermissionResponse,
        message: Option<&str>,
    ) -> Result<bool, CoreError>;
    async fn respond_permission(
        &self,
        id: &str,
        permission_id: &str,
        response: PermissionResponse,
        remember: Option<bool>,
    ) -> Result<bool, CoreError>;

    // files / diffs
    async fn diff(&self, id: &str, message_id: Option<&str>) -> Result<Vec<FileDiff>, CoreError>;
    async fn file_tree(&self, path: Option<&str>) -> Result<Vec<FileNode>, CoreError>;
    async fn file_content(&self, path: &str) -> Result<FileContent, CoreError>;
    async fn file_status(&self) -> Result<Vec<FileStatus>, CoreError>;
    async fn find_files(&self, query: &str, limit: Option<u32>) -> Result<Vec<String>, CoreError>;
    async fn find_symbols(&self, query: &str) -> Result<Vec<Value>, CoreError>;
    async fn find_by_pattern(&self, pattern: &str) -> Result<Vec<String>, CoreError>;

    // events
    async fn stream_events(&self) -> Result<EventStream, CoreError>;
}
