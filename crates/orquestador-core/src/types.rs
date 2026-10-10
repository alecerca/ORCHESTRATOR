//! Serde shapes for the OpenCode server API.
//!
//! Extracted by the planner from the OpenAPI 3.1 document served at
//! `GET /doc` and from the generated SDK types referenced in `SPEC.md` §4.
//!
//! Conventions used here (IMPORTANT — the frontend mirrors these exactly):
//!
//! * Wire format is camelCase with **capitalised abbreviations**:
//!   `sessionID`, `parentID`, `messageID`, `permissionID`, `projectID`,
//!   `providerID`. `#[serde(rename_all = "camelCase")]` would produce
//!   `sessionId`, which is WRONG — every one of those fields is renamed
//!   explicitly.
//! * Unknown fields on the wire are ignored (serde default) so that a newer
//!   OpenCode server does not break deserialization.
//! * Optional fields are `Option<T>` + `#[serde(default)]`.
//! * All timestamps are epoch **milliseconds** (`number` in the OpenAPI spec).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Global / project
// ---------------------------------------------------------------------------

/// `GET /global/health`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Health {
    pub healthy: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectTime {
    pub created: i64,
    #[serde(default)]
    pub initialized: Option<i64>,
}

/// `GET /project`, `GET /project/current`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub worktree: String,
    #[serde(default, rename = "vcsDir")]
    pub vcs_dir: Option<String>,
    #[serde(default)]
    pub vcs: Option<String>,
    pub time: ProjectTime,
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionTime {
    pub created: i64,
    pub updated: i64,
    #[serde(default)]
    pub compacting: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub additions: i64,
    pub deletions: i64,
    pub files: i64,
    #[serde(default)]
    pub diffs: Option<Vec<FileDiff>>,
}

/// `GET /session`, `POST /session`, `GET /session/:id`, `GET /session/:id/children`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    #[serde(rename = "projectID")]
    pub project_id: String,
    pub directory: String,
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    pub title: String,
    pub version: String,
    pub time: SessionTime,
    #[serde(default)]
    pub summary: Option<SessionSummary>,
}

/// `GET /session/status` → `{ [sessionID]: SessionStatus }`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum SessionStatus {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "busy")]
    Busy,
    #[serde(rename = "retry")]
    Retry {
        attempt: u32,
        message: String,
        /// Epoch millis of the next attempt.
        next: i64,
    },
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserMessageTime {
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantMessageTime {
    pub created: i64,
    #[serde(default)]
    pub completed: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelRef {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache: TokenCache,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenCache {
    pub read: i64,
    pub write: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessagePath {
    pub cwd: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorData {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default, rename = "statusCode")]
    pub status_code: Option<u16>,
    #[serde(default, rename = "isRetryable")]
    pub is_retryable: Option<bool>,
}

/// Error payload carried by `AssistantMessage.error`.
/// The wire shape is `{ "name": "...", "data": { ... } }`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "name")]
pub enum MessageError {
    #[serde(rename = "ProviderAuthError")]
    ProviderAuth { data: ErrorData },
    #[serde(rename = "UnknownError")]
    Unknown { data: ErrorData },
    #[serde(rename = "MessageOutputLengthError")]
    OutputLength { data: ErrorData },
    #[serde(rename = "MessageAbortedError")]
    Aborted { data: ErrorData },
    #[serde(rename = "APIError")]
    Api { data: ErrorData },
}

impl MessageError {
    /// Human readable message, best effort (the UI shows this verbatim).
    pub fn message(&self) -> String {
        let data = match self {
            MessageError::ProviderAuth { data }
            | MessageError::Unknown { data }
            | MessageError::OutputLength { data }
            | MessageError::Aborted { data }
            | MessageError::Api { data } => data,
        };
        data.message
            .clone()
            .unwrap_or_else(|| self.name().to_string())
    }

    pub fn name(&self) -> &'static str {
        match self {
            MessageError::ProviderAuth { .. } => "ProviderAuthError",
            MessageError::Unknown { .. } => "UnknownError",
            MessageError::OutputLength { .. } => "MessageOutputLengthError",
            MessageError::Aborted { .. } => "MessageAbortedError",
            MessageError::Api { .. } => "APIError",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        time: UserMessageTime,
        #[serde(default)]
        agent: Option<String>,
        #[serde(default)]
        model: Option<ModelRef>,
        #[serde(default)]
        system: Option<String>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        time: AssistantMessageTime,
        #[serde(default)]
        error: Option<MessageError>,
        #[serde(rename = "parentID")]
        parent_id: String,
        #[serde(rename = "modelID")]
        model_id: String,
        #[serde(rename = "providerID")]
        provider_id: String,
        mode: String,
        path: MessagePath,
        #[serde(default)]
        summary: Option<bool>,
        cost: f64,
        tokens: TokenUsage,
        #[serde(default)]
        finish: Option<String>,
    },
}

impl Message {
    pub fn id(&self) -> &str {
        match self {
            Message::User { id, .. } | Message::Assistant { id, .. } => id,
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Message::User { session_id, .. } | Message::Assistant { session_id, .. } => session_id,
        }
    }

    pub fn role(&self) -> &'static str {
        match self {
            Message::User { .. } => "user",
            Message::Assistant { .. } => "assistant",
        }
    }

    pub fn created(&self) -> i64 {
        match self {
            Message::User { time, .. } => time.created,
            Message::Assistant { time, .. } => time.created,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Message::Assistant { error: Some(_), .. })
    }
}

/// `{ info: Message, parts: Part[] }` — every message-returning endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageBundle {
    pub info: Message,
    pub parts: Vec<Part>,
}

// ---------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status")]
pub enum ToolState {
    #[serde(rename = "pending")]
    Pending {
        input: serde_json::Value,
        #[serde(default)]
        raw: Option<String>,
    },
    #[serde(rename = "running")]
    Running {
        input: serde_json::Value,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        time: Option<ToolTime>,
    },
    #[serde(rename = "completed")]
    Completed {
        input: serde_json::Value,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        metadata: Option<serde_json::Value>,
        #[serde(default)]
        time: Option<ToolTime>,
    },
    #[serde(rename = "error")]
    Error {
        input: serde_json::Value,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        metadata: Option<serde_json::Value>,
        #[serde(default)]
        time: Option<ToolTime>,
    },
}

impl ToolState {
    pub fn status(&self) -> &'static str {
        match self {
            ToolState::Pending { .. } => "pending",
            ToolState::Running { .. } => "running",
            ToolState::Completed { .. } => "completed",
            ToolState::Error { .. } => "error",
        }
    }

    /// Short human label for the stream UI (`title` when known, else tool name).
    pub fn title(&self) -> Option<&str> {
        match self {
            ToolState::Running { title, .. } | ToolState::Completed { title, .. } => {
                title.as_deref()
            }
            ToolState::Pending { .. } | ToolState::Error { .. } => None,
        }
    }

    pub fn output(&self) -> Option<&str> {
        match self {
            ToolState::Completed { output, .. } => output.as_deref(),
            ToolState::Error { error, .. } => error.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolTime {
    pub start: i64,
    #[serde(default)]
    pub end: Option<i64>,
}

/// A message part. Tagged on `type`; every variant carries the same trio of
/// correlation ids (`id`, `sessionID`, `messageID`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Part {
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        text: String,
        #[serde(default)]
        synthetic: Option<bool>,
        #[serde(default)]
        ignored: Option<bool>,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        text: String,
        #[serde(default)]
        time: Option<ToolTime>,
    },
    #[serde(rename = "tool")]
    Tool {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        #[serde(rename = "callID")]
        call_id: String,
        tool: String,
        state: ToolState,
    },
    #[serde(rename = "step-start")]
    StepStart {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
    },
    #[serde(rename = "step-finish")]
    StepFinish {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        reason: String,
        #[serde(default)]
        cost: Option<f64>,
        #[serde(default)]
        tokens: Option<TokenUsage>,
    },
    #[serde(rename = "agent")]
    AgentSpawn {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        name: String,
    },
    #[serde(rename = "subtask")]
    Subtask {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        prompt: String,
        description: String,
        agent: String,
    },
    #[serde(rename = "file")]
    File {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        mime: String,
        url: String,
        #[serde(default)]
        filename: Option<String>,
    },
    #[serde(rename = "retry")]
    Retry {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        attempt: u32,
        #[serde(default)]
        error: Option<serde_json::Value>,
    },
    #[serde(rename = "patch")]
    Patch {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        hash: String,
        files: Vec<String>,
    },
    #[serde(rename = "snapshot")]
    Snapshot {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        snapshot: String,
    },
    #[serde(rename = "compaction")]
    Compaction {
        id: String,
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        auto: bool,
    },
    /// Any part type this build does not know about (forward compatible).
    #[serde(other)]
    Unknown,
}

impl Part {
    pub fn id(&self) -> Option<&str> {
        match self {
            Part::Unknown => None,
            Part::Text { id, .. }
            | Part::Reasoning { id, .. }
            | Part::Tool { id, .. }
            | Part::StepStart { id, .. }
            | Part::StepFinish { id, .. }
            | Part::AgentSpawn { id, .. }
            | Part::Subtask { id, .. }
            | Part::File { id, .. }
            | Part::Retry { id, .. }
            | Part::Patch { id, .. }
            | Part::Snapshot { id, .. }
            | Part::Compaction { id, .. } => Some(id),
        }
    }

    pub fn session_id(&self) -> Option<&str> {
        match self {
            Part::Unknown => None,
            Part::Text { session_id, .. }
            | Part::Reasoning { session_id, .. }
            | Part::Tool { session_id, .. }
            | Part::StepStart { session_id, .. }
            | Part::StepFinish { session_id, .. }
            | Part::AgentSpawn { session_id, .. }
            | Part::Subtask { session_id, .. }
            | Part::File { session_id, .. }
            | Part::Retry { session_id, .. }
            | Part::Patch { session_id, .. }
            | Part::Snapshot { session_id, .. }
            | Part::Compaction { session_id, .. } => Some(session_id),
        }
    }

    pub fn message_id(&self) -> Option<&str> {
        match self {
            Part::Unknown => None,
            Part::Text { message_id, .. }
            | Part::Reasoning { message_id, .. }
            | Part::Tool { message_id, .. }
            | Part::StepStart { message_id, .. }
            | Part::StepFinish { message_id, .. }
            | Part::AgentSpawn { message_id, .. }
            | Part::Subtask { message_id, .. }
            | Part::File { message_id, .. }
            | Part::Retry { message_id, .. }
            | Part::Patch { message_id, .. }
            | Part::Snapshot { message_id, .. }
            | Part::Compaction { message_id, .. } => Some(message_id),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Part::Text { .. } => "text",
            Part::Reasoning { .. } => "reasoning",
            Part::Tool { .. } => "tool",
            Part::StepStart { .. } => "step-start",
            Part::StepFinish { .. } => "step-finish",
            Part::AgentSpawn { .. } => "agent",
            Part::Subtask { .. } => "subtask",
            Part::File { .. } => "file",
            Part::Retry { .. } => "retry",
            Part::Patch { .. } => "patch",
            Part::Snapshot { .. } => "snapshot",
            Part::Compaction { .. } => "compaction",
            Part::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Permissions (approval prompts)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PermissionPattern {
    One(String),
    Many(Vec<String>),
}

/// `permission.updated` payload. While present in the pending registry the
/// agent is blocked waiting for the human.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permission {
    pub id: String,
    /// The permission kind (`edit`, `bash`, `webfetch`, `read`, ...). Renamed
    /// from the wire field `type` because `type` is a Rust keyword.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub pattern: Option<PermissionPattern>,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(default, rename = "callID")]
    pub call_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub time: PermissionTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionTime {
    pub created: i64,
}

/// Body of `POST /session/:id/permissions/:permissionID`.
///
/// ⚠️ SPEC §4 says `{ response, remember? }`; the OpenAPI document narrows
/// `response` to `"once" | "always" | "reject"` (not `allow`/`deny`).
/// `remember` is accepted here and omitted when `None`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionResponse {
    Once,
    Always,
    Reject,
}

impl PermissionResponse {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionResponse::Once => "once",
            PermissionResponse::Always => "always",
            PermissionResponse::Reject => "reject",
        }
    }
}

// ---------------------------------------------------------------------------
// Todos / files / agents
// ---------------------------------------------------------------------------

/// `GET /session/:id/todo`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Todo {
    pub id: String,
    pub content: String,
    /// `pending` | `in_progress` | `completed` | `cancelled`
    pub status: String,
    /// `high` | `medium` | `low`
    pub priority: String,
}

/// `GET /session/:id/diff` → `FileDiff[]`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDiff {
    pub file: String,
    pub before: String,
    pub after: String,
    pub additions: i64,
    pub deletions: i64,
}

/// `GET /file?path=` → `FileNode[]`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub absolute: String,
    #[serde(rename = "type")]
    pub kind: FileNodeKind,
    #[serde(default)]
    pub ignored: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileNodeKind {
    File,
    Directory,
}

/// `GET /file/content?path=` → `FileContent`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileContent {
    #[serde(rename = "type")]
    pub kind: FileContentKind,
    pub content: String,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default, rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileContentKind {
    Text,
    Binary,
}

/// `GET /file/status` → `File[]`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileStatus {
    pub path: String,
    pub added: i64,
    pub removed: i64,
    pub status: FileStatusKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileStatusKind {
    Added,
    Deleted,
    Modified,
}

/// `GET /agent` → `Agent[]` (only the fields the UI needs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub mode: String,
    #[serde(rename = "builtIn")]
    pub built_in: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub model: Option<ModelRef>,
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/// Body of `POST /session/:id/message` and `POST /session/:id/prompt_async`.
/// Only the text input part is produced by the composer (v1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptBody {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "messageID")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "noReply")]
    pub no_reply: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, bool>>,
    pub parts: Vec<InputPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InputPart {
    Text { text: String },
}

impl PromptBody {
    /// Convenience constructor for a plain-text instruction.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            message_id: None,
            model: None,
            agent: None,
            no_reply: None,
            system: None,
            tools: None,
            parts: vec![InputPart::Text { text: text.into() }],
        }
    }
}

/// Body of `POST /session/:id/command`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandBody {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Body of `POST /session`
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateSessionBody {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "parentID")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// SSE events
// ---------------------------------------------------------------------------

/// Every event produced by `GET /event`, tagged on `type`.
///
/// Only the events that drive Orquestador state are modelled; everything else
/// collapses into [`Event::Unknown`] so a server upgrade cannot break parsing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Event {
    // connection
    #[serde(rename = "server.connected")]
    ServerConnected { properties: serde_json::Value },
    // sessions
    #[serde(rename = "session.created")]
    SessionCreated { properties: SessionInfoProp },
    #[serde(rename = "session.updated")]
    SessionUpdated { properties: SessionInfoProp },
    #[serde(rename = "session.deleted")]
    SessionDeleted { properties: SessionInfoProp },
    #[serde(rename = "session.status")]
    SessionStatus { properties: SessionStatusProp },
    #[serde(rename = "session.idle")]
    SessionIdle { properties: SessionIdProp },
    #[serde(rename = "session.error")]
    SessionError { properties: SessionErrorProp },
    #[serde(rename = "session.diff")]
    SessionDiff { properties: SessionDiffProp },
    // messages / parts (display only — not mirrored into AppState)
    #[serde(rename = "message.updated")]
    MessageUpdated { properties: MessageInfoProp },
    #[serde(rename = "message.removed")]
    MessageRemoved { properties: SessionAndMessageProp },
    #[serde(rename = "message.part.updated")]
    PartUpdated { properties: PartUpdatedProp },
    #[serde(rename = "message.part.removed")]
    PartRemoved { properties: PartRemovedProp },
    // approvals
    #[serde(rename = "permission.updated")]
    PermissionUpdated { properties: Permission },
    #[serde(rename = "permission.replied")]
    PermissionReplied { properties: PermissionRepliedProp },
    // misc
    #[serde(rename = "todo.updated")]
    TodoUpdated { properties: TodoProp },
    #[serde(rename = "file.edited")]
    FileEdited { properties: FileEditedProp },
    #[serde(rename = "file.watcher.updated")]
    FileWatcher { properties: FileWatcherProp },
    /// Unmodelled event type — ignored by the reducer.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionInfoProp {
    pub info: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionIdProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionStatusProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionErrorProp {
    #[serde(default, rename = "sessionID")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionDiffProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub diff: Vec<FileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageInfoProp {
    pub info: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionAndMessageProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartUpdatedProp {
    pub part: Part,
    #[serde(default)]
    pub delta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartRemovedProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "partID")]
    pub part_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRepliedProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "permissionID")]
    pub permission_id: String,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub todos: Vec<Todo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEditedProp {
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileWatcherProp {
    pub file: String,
    pub event: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_JSON: &str = r#"{
        "id": "ses_abc",
        "projectID": "prj_1",
        "directory": "/home/dev/proj",
        "parentID": "ses_parent",
        "title": "Orchestrator",
        "version": "1.2.3",
        "time": { "created": 1728000000000, "updated": 1728000100000 }
    }"#;

    #[test]
    fn session_round_trips_with_opencode_field_names() {
        let s: Session = serde_json::from_str(SESSION_JSON).expect("session parses");
        assert_eq!(s.id, "ses_abc");
        assert_eq!(s.project_id, "prj_1");
        assert_eq!(s.parent_id.as_deref(), Some("ses_parent"));
        assert_eq!(s.time.created, 1728000000000);

        let out = serde_json::to_value(&s).unwrap();
        assert_eq!(out["projectID"], "prj_1");
        assert_eq!(out["parentID"], "ses_parent");
        assert!(
            out.get("sessionId").is_none(),
            "must not use naive camelCase"
        );
    }

    #[test]
    fn session_tolerates_unknown_fields() {
        let json = r#"{"id":"s","projectID":"p","directory":"/d","title":"t","version":"1",
                "time":{"created":1,"updated":2},"brandNewField":{"a":1}}"#;
        assert!(serde_json::from_str::<Session>(json).is_ok());
    }

    #[test]
    fn session_status_variants_parse() {
        let idle: SessionStatus = serde_json::from_str(r#"{"type":"idle"}"#).unwrap();
        assert_eq!(idle, SessionStatus::Idle);
        let busy: SessionStatus = serde_json::from_str(r#"{"type":"busy"}"#).unwrap();
        assert_eq!(busy, SessionStatus::Busy);
        let retry: SessionStatus = serde_json::from_str(
            r#"{"type":"retry","attempt":2,"message":"rate limited","next":9}"#,
        )
        .unwrap();
        assert!(matches!(retry, SessionStatus::Retry { attempt: 2, .. }));
        // round trip keeps the tag
        assert_eq!(
            serde_json::to_value(&busy).unwrap(),
            serde_json::json!({"type": "busy"})
        );
    }

    #[test]
    fn message_variants_parse_by_role() {
        let user = r#"{"id":"m1","sessionID":"s1","role":"user","time":{"created":1},
                       "agent":"build","model":{"providerID":"p","modelID":"m"}}"#;
        let m: Message = serde_json::from_str(user).unwrap();
        assert_eq!(m.role(), "user");
        assert_eq!(m.id(), "m1");

        let assistant = r#"{"id":"m2","sessionID":"s1","role":"assistant","time":{"created":1},
            "parentID":"","modelID":"m","providerID":"p","mode":"build",
            "path":{"cwd":"/d","root":"/d"},"cost":0.1,
            "tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}},
            "error":{"name":"APIError","data":{"message":"boom","statusCode":500,"isRetryable":false}}}"#;
        let m: Message = serde_json::from_str(assistant).unwrap();
        assert_eq!(m.role(), "assistant");
        assert!(m.is_error());
        match &m {
            Message::Assistant { error: Some(e), .. } => {
                assert_eq!(e.message(), "boom");
                assert_eq!(e.name(), "APIError");
            }
            other => panic!("expected assistant message, got {other:?}"),
        }
    }

    #[test]
    fn parts_parse_and_report_correlation_ids() {
        let text = r#"{"id":"p1","sessionID":"s1","messageID":"m1","type":"text","text":"hola"}"#;
        let p: Part = serde_json::from_str(text).unwrap();
        assert_eq!(p.kind(), "text");
        assert_eq!(p.id(), Some("p1"));
        assert_eq!(p.message_id(), Some("m1"));

        let tool = r#"{"id":"p2","sessionID":"s1","messageID":"m1","type":"tool","callID":"c1",
            "tool":"edit","state":{"status":"completed","input":{},"output":"ok","title":"Edit file",
            "metadata":{},"time":{"start":1,"end":2}}}"#;
        let p: Part = serde_json::from_str(tool).unwrap();
        assert_eq!(p.kind(), "tool");
        match &p {
            Part::Tool { state, tool, .. } => {
                assert_eq!(state.status(), "completed");
                assert_eq!(state.output(), Some("ok"));
                assert_eq!(tool, "edit");
            }
            other => panic!("expected tool part, got {other:?}"),
        }

        // Unknown part types must not blow up the stream.
        let weird = r#"{"id":"p3","sessionID":"s1","messageID":"m1","type":"hologram"}"#;
        let p: Part = serde_json::from_str(weird).unwrap();
        assert_eq!(p.kind(), "unknown");
    }

    #[test]
    fn permission_and_response_round_trip() {
        let json = r#"{"id":"perm_1","type":"edit","sessionID":"s1","messageID":"m1",
            "title":"Edit src/main.rs","metadata":{"filePath":"src/main.rs"},"time":{"created":1}}"#;
        let p: Permission = serde_json::from_str(json).unwrap();
        assert_eq!(p.kind, "edit");
        assert_eq!(p.session_id, "s1");

        assert_eq!(
            serde_json::to_value(PermissionResponse::Always).unwrap(),
            "always"
        );
        assert_eq!(
            serde_json::to_value(PermissionResponse::Reject).unwrap(),
            "reject"
        );
    }

    #[test]
    fn prompt_body_uses_opencode_wire_names() {
        let body = PromptBody::text("spawn 5 subagents");
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["parts"][0]["type"], "text");
        assert_eq!(v["parts"][0]["text"], "spawn 5 subagents");
        assert!(v.get("noReply").is_none(), "None fields must be omitted");
        assert!(v.get("messageID").is_none());
    }

    #[test]
    fn events_parse_and_unknown_events_are_tolerated() {
        let created = r#"{"type":"session.created","properties":{"info":{
            "id":"c1","projectID":"p","directory":"/d","title":"sub","version":"1",
            "time":{"created":1,"updated":2}}}}"#;
        let ev: Event = serde_json::from_str(created).unwrap();
        assert!(matches!(ev, Event::SessionCreated { .. }));

        let perm = r#"{"type":"permission.updated","properties":{"id":"perm_1","type":"bash",
            "sessionID":"s1","messageID":"m1","title":"Run rm","metadata":{},"time":{"created":1}}}"#;
        let ev: Event = serde_json::from_str(perm).unwrap();
        assert!(matches!(ev, Event::PermissionUpdated { .. }));

        let status =
            r#"{"type":"session.status","properties":{"sessionID":"s1","status":{"type":"busy"}}}"#;
        let ev: Event = serde_json::from_str(status).unwrap();
        assert!(matches!(
            ev,
            Event::SessionStatus {
                properties: SessionStatusProp {
                    status: SessionStatus::Busy,
                    ..
                }
            }
        ));

        let unknown = r#"{"type":"pty.created","properties":{"info":{"id":"x"}}}"#;
        let ev: Event = serde_json::from_str(unknown).unwrap();
        assert_eq!(ev, Event::Unknown);
    }

    #[test]
    fn message_bundle_shape_matches_history_endpoint() {
        let json = r#"{"info":{"id":"m1","sessionID":"s1","role":"user","time":{"created":1}},
            "parts":[{"id":"p1","sessionID":"s1","messageID":"m1","type":"text","text":"hi"}]}"#;
        let b: MessageBundle = serde_json::from_str(json).unwrap();
        assert_eq!(b.parts.len(), 1);
        let out = serde_json::to_value(&b).unwrap();
        assert_eq!(out["info"]["role"], "user");
        assert_eq!(out["parts"][0]["type"], "text");
    }
}
