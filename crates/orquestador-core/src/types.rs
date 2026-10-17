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
use std::collections::{HashMap, HashSet};

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
    /// The effective model this session runs, as the server reports it. This is
    /// the only authoritative source of the model the session uses (the
    /// app-level config carries none); a `None` here means the session has no
    /// recorded model yet. The wire spells the model id `id` (NOT `modelID`),
    /// which [`ModelRef`] maps in via a serde alias.
    #[serde(default)]
    pub model: Option<ModelRef>,
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
    /// The model id. Emitted as `modelID` (the agent/prompt spelling); *also*
    /// accepted as `id`, which is how a `GET /session` row spells the effective
    /// model it recorded.
    #[serde(rename = "modelID", alias = "id")]
    pub model_id: String,
}

// ---------------------------------------------------------------------------
// Providers (OpenCode `GET /provider`)
// ---------------------------------------------------------------------------

/// `GET /provider` → everything the UI needs to list providers and their models.
///
/// SECURITY: the live `/provider` response embeds credential material inside
/// each provider's `key` and `options` (and, per model, `options` / `headers`).
/// Those fields are deliberately **not** modelled here, so they can never be
/// deserialized, logged, `Debug`-printed, or forwarded to the webview. serde
/// ignores the unknown wire fields (crate convention) while the typed fields
/// stay credential-free.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderList {
    pub all: Vec<Provider>,
    /// Configured default model per provider (`providerID` → `modelID`).
    #[serde(default, rename = "default")]
    pub default_models: HashMap<String, String>,
    /// Provider ids that are already authenticated — the per-provider auth status.
    #[serde(default)]
    pub connected: Vec<String>,
}

impl ProviderList {
    /// True when a provider can serve a model right now: it is already
    /// authenticated (a member of `connected` from `GET /provider`).
    ///
    /// A provider that is not connected cannot serve a model this app sends, so
    /// its models must be marked rather than offered as ordinary choices. The
    /// separate `has_auth` signal only tells the UI whether a sign-in path
    /// exists; it never makes an unauthenticated provider usable.
    pub fn is_usable(&self, provider_id: &str) -> bool {
        self.connected.iter().any(|id| id == provider_id)
    }

    /// Stamp every provider with `usable` (from `connected`) and `has_auth`
    /// (whether `auth_provider_ids` — the keys of `GET /provider/auth` — lists
    /// it), so the webview gets the classification without re-deriving the rule.
    pub fn classify_usability<'a, I>(&mut self, auth_provider_ids: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        let auth: HashSet<&str> = auth_provider_ids.into_iter().collect();
        let connected: HashSet<&str> = self.connected.iter().map(String::as_str).collect();
        for provider in &mut self.all {
            provider.usable = connected.contains(provider.id.as_str());
            provider.has_auth = auth.contains(provider.id.as_str());
        }
    }

    /// Stamp every provider with `offers_api_key`: whether `GET /provider/auth`
    /// lists an `api`-type method for it, so the picker knows which unusable
    /// providers can be connected by entering a key.
    ///
    /// This is deliberately narrower than `has_auth`: an OAuth-only provider has
    /// auth but no key entry, so it must not advertise one.
    pub fn classify_api_key_auth(&mut self, methods: &HashMap<String, Vec<ProviderAuthMethod>>) {
        for provider in &mut self.all {
            provider.offers_api_key = methods
                .get(&provider.id)
                .is_some_and(|entries| entries.iter().any(ProviderAuthMethod::is_api_key));
        }
    }
}

/// One provider. `key` and `options` are intentionally omitted — see
/// [`ProviderList`].
///
/// `Eq` is intentionally not derived: models carry `f64` pricing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub source: String,
    /// Environment variable **names** this provider can read a credential from
    /// (e.g. `ANTHROPIC_API_KEY`) — names only, never values.
    #[serde(default)]
    pub env: Vec<String>,
    /// Models offered by this provider, keyed by model id.
    #[serde(default)]
    pub models: HashMap<String, ProviderModel>,
    /// Whether this provider declares at least one auth method in
    /// `GET /provider/auth`. Computed by the shell; **not** part of the
    /// `/provider` wire payload (see [`ProviderList::classify_usability`]).
    #[serde(default, skip_deserializing, rename = "hasAuth")]
    pub has_auth: bool,
    /// Whether this provider can serve a model right now (it is `connected`).
    /// Computed by the shell; **not** part of the `/provider` wire payload.
    #[serde(default, skip_deserializing)]
    pub usable: bool,
    /// Whether `GET /provider/auth` lists an `api`-type method for this
    /// provider — i.e. the user can connect it by entering an API key. False
    /// for a provider that offers only OAuth (or no auth method at all).
    /// Computed by the shell; **not** part of the `/provider` wire payload.
    #[serde(default, skip_deserializing, rename = "offersApiKey")]
    pub offers_api_key: bool,
}

/// Per-million-token pricing for a [`ProviderModel`]. Every component zero (and
/// no `:free`/`-free` id suffix) means the model is free.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache: CacheCost,
}

/// The nested `cache` pricing of a [`ModelCost`]. Modelled as its own object to
/// match the wire shape rather than being flattened.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CacheCost {
    pub read: f64,
    pub write: f64,
}

/// One model offered by a provider. Only display/identity and safe pricing
/// fields are modelled; the model-level `options` and `headers` (which may carry
/// credentials) are deliberately omitted — see [`ProviderList`].
///
/// `Eq` is intentionally not derived: `cost` holds `f64` values.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(default)]
    pub family: Option<String>,
    /// Token pricing, `None` when the catalogue omits it (a model with no cost
    /// data is not free unless its id carries the explicit suffix).
    #[serde(default)]
    pub cost: Option<ModelCost>,
    /// `"active"` | `"deprecated"` in the catalogue.
    #[serde(default)]
    pub status: Option<String>,
}

impl ProviderModel {
    /// True when this model costs nothing: every `cost` component is zero, or
    /// its id carries the explicit `:free` / `-free` suffix the catalogue uses.
    ///
    /// Single source of truth for the `free` tag; the computed flag is
    /// serialized by the [`Serialize`] impl below.
    pub fn is_free(&self) -> bool {
        let zero_cost = self.cost.as_ref().is_some_and(|cost| {
            cost.input == 0.0
                && cost.output == 0.0
                && cost.cache.read == 0.0
                && cost.cache.write == 0.0
        });
        zero_cost || self.id.ends_with(":free") || self.id.ends_with("-free")
    }
}

/// Serializes the wire-visible fields plus the computed `free` flag, so the
/// webview never re-derives the rule.
impl Serialize for ProviderModel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ProviderModel", 7)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("providerID", &self.provider_id)?;
        state.serialize_field("family", &self.family)?;
        state.serialize_field("cost", &self.cost)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("free", &self.is_free())?;
        state.end()
    }
}

/// `GET /provider/auth` → one supported auth method per provider.
///
/// `type` is `"oauth"` or `"api"`. `prompts` are the UI prompt definitions the
/// server expects for an interactive login flow; they are kept as opaque JSON
/// (they never carry credential *values*).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAuthMethod {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub label: String,
    #[serde(default)]
    pub prompts: Vec<serde_json::Value>,
}

impl ProviderAuthMethod {
    /// True when this method lets the user supply an API key directly
    /// (`type: "api"`), as opposed to an OAuth sign-in (`type: "oauth"`).
    pub fn is_api_key(&self) -> bool {
        self.auth_type == "api"
    }
}

// ---------------------------------------------------------------------------
// Provider OAuth connect (T5)
// ---------------------------------------------------------------------------

/// Response of `POST /provider/:providerID/oauth/authorize`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAuthAuthorization {
    pub url: String,
    /// `"auto"` → open the URL in a browser and re-poll the auth status;
    /// `"code"` → the server expects a pasted authorization code back.
    pub method: String,
    pub instructions: String,
}

// ---------------------------------------------------------------------------
// Config read / patch (T6)
// ---------------------------------------------------------------------------

/// Read-only view of `GET /config`.
///
/// SECURITY: same convention as [`ProviderList`] — `provider`, `server`,
/// `command`, … are credential-bearing or unbounded sections and are
/// deliberately **not** modelled. The server may send them; serde drops them,
/// so they can never be deserialized, Debug-printed, or forwarded to the
/// webview. Only the safe scalars below are kept.
///
/// Wire names differ per field (`logLevel` camelCase, `small_model` /
/// `default_agent` snake_case) — the renames mirror the upstream schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpencodeConfig {
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default, rename = "logLevel")]
    pub log_level: Option<String>,
    #[serde(default)]
    pub snapshot: Option<bool>,
    #[serde(default)]
    pub share: Option<String>,
    #[serde(default)]
    pub autoshare: Option<bool>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub small_model: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub instructions: Option<Vec<String>>,
}

/// Body of `PATCH /config` — the **allowlist** of scalar fields this app is
/// willing to change. Every field omits itself when `None`, so a partial patch
/// serializes to exactly the touched scalars; the server merges the patch into
/// the existing config (upstream `mergeDeep`), so omitted fields are preserved
/// rather than replaced. Credential-bearing sections are not expressible here
/// at all — they cannot ride along even by accident.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "logLevel")]
    pub log_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autoshare: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
}

// ---------------------------------------------------------------------------
// MCP status (T7)
// ---------------------------------------------------------------------------

/// One entry of `GET /mcp` — status of a named MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum McpStatus {
    Connected,
    Disabled,
    Failed {
        error: String,
    },
    NeedsAuth,
    NeedsClientRegistration {
        error: String,
    },
    /// A future server status — tolerated so one unknown server cannot break
    /// parsing of the whole map.
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// Slash commands (T9)
// ---------------------------------------------------------------------------

/// One entry of `GET /command`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Restrict the command to one agent (e.g. `plan`); None → all agents.
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// `"command" | "skill" | ...` — where the definition came from.
    #[serde(default)]
    pub source: Option<String>,
    pub template: String,
    #[serde(default)]
    pub hints: Vec<String>,
}

// ---------------------------------------------------------------------------
// Clarifications (T11 — question batches)
// ---------------------------------------------------------------------------

/// One selectable option of a clarification question.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// One question inside a pending batch. The whole batch is presented at once
/// (one pause, one reply covering every question). `multiple` marks
/// multi-select; `custom` (absent → server default `true`) whether a free-text
/// answer is allowed. `recommended` does not exist on the wire today
/// (opencode 1.18.35) — modelled for a future revision and rendered only when
/// present; the UI never invents a preselection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionInfo {
    pub question: String,
    pub header: String,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
}

/// The tool call that triggered a batch (message/call correlation).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionToolRef {
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
}

/// One pending clarification batch — both the `GET /question` list entry and
/// the `properties` payload of `question.asked` / `question.v2.asked`
/// (identical shapes on the wire).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub questions: Vec<QuestionInfo>,
    #[serde(default)]
    pub tool: Option<QuestionToolRef>,
}

/// `properties` of `question.replied` / `question.v2.replied`. One answer
/// entry per question, in question order; a multi-select answer holds several
/// labels; an unanswered/partial question carries `[]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionRepliedProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "requestID")]
    pub request_id: String,
    pub answers: Vec<Vec<String>>,
}

/// `properties` of `question.rejected` / `question.v2.rejected`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionRejectedProp {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "requestID")]
    pub request_id: String,
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

/// `GET /permission` → one pending approval across all sessions.
///
/// This is a **different wire shape** from the `permission.updated` payload
/// modelled by [`Permission`]: the list endpoint carries `permission` /
/// `patterns` / `always`, nests the message id under `tool`, and has neither a
/// `title` nor a timestamp. It is modelled faithfully here rather than forced
/// into [`Permission`]; [`Self::into_permission`] performs the fold for the
/// in-memory registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRequest {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// Permission kind (`bash`, `edit`, `external_directory`, ...).
    pub permission: String,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub always: Vec<String>,
    #[serde(default)]
    pub tool: Option<PermissionToolRef>,
}

/// The `tool` sub-object of a [`PermissionRequest`] (optional on the wire).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionToolRef {
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
}

impl PermissionRequest {
    /// Fold this list entry into the `permission.updated`-shaped registry type.
    ///
    /// The list endpoint has no `title` and no `time`, so the kind is used as a
    /// short title and `time.created` is `0` (these entries therefore sort
    /// before SSE-discovered ones, whose real timestamps are positive).
    pub fn into_permission(self) -> Permission {
        let pattern = match self.patterns.as_slice() {
            [] => None,
            [only] => Some(PermissionPattern::One(only.clone())),
            many => Some(PermissionPattern::Many(many.to_vec())),
        };
        let (message_id, call_id) = match &self.tool {
            Some(tool) => (tool.message_id.clone(), Some(tool.call_id.clone())),
            None => (String::new(), None),
        };
        let title = if self.permission.trim().is_empty() {
            "permission".to_string()
        } else {
            self.permission.clone()
        };
        Permission {
            id: self.id,
            kind: self.permission,
            pattern,
            session_id: self.session_id,
            message_id,
            call_id,
            title,
            metadata: self.metadata,
            time: PermissionTime { created: 0 },
        }
    }
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

/// `GET /orquestador/present` entry: one member of the shared workspace.
///
/// Presence is a proxy-native concept (see the cowork design): the app only
/// receives this shape when it talks to the server through `orquestador-proxy`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresenceEntry {
    pub name: String,
    pub role: String,
    #[serde(rename = "idleMs")]
    pub idle_ms: u64,
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
    // clarifications (T11) — v1 and v2 wire revisions share the shapes
    #[serde(rename = "question.asked")]
    QuestionAsked { properties: QuestionRequest },
    #[serde(rename = "question.v2.asked")]
    QuestionV2Asked { properties: QuestionRequest },
    #[serde(rename = "question.replied")]
    QuestionReplied { properties: QuestionRepliedProp },
    #[serde(rename = "question.v2.replied")]
    QuestionV2Replied { properties: QuestionRepliedProp },
    #[serde(rename = "question.rejected")]
    QuestionRejected { properties: QuestionRejectedProp },
    #[serde(rename = "question.v2.rejected")]
    QuestionV2Rejected { properties: QuestionRejectedProp },
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
    fn prompt_body_serializes_model_when_present() {
        let mut body = PromptBody::text("hello");
        body.model = Some(ModelRef {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet".to_string(),
        });
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["model"]["providerID"], "anthropic");
        assert_eq!(v["model"]["modelID"], "claude-3-5-sonnet");
    }

    #[test]
    fn prompt_body_omits_model_when_none() {
        let body = PromptBody::text("hello");
        let v = serde_json::to_value(&body).unwrap();
        assert!(v.get("model").is_none(), "model must be absent when None");
    }

    #[test]
    fn permission_request_parses_the_list_payload_and_folds_into_a_permission() {
        // Shape captured live from `GET /permission` on 2026-10-10.
        let json = r#"{
            "id":"per_126b00d0f001cyU1H4us7xUdRg",
            "sessionID":"ses_edb2968abffe",
            "permission":"bash",
            "patterns":["git add a","git commit -m x"],
            "metadata":{"command":"git add a && git commit -m x"},
            "always":["git add *","git commit *"],
            "tool":{"messageID":"msg_1","callID":"call_1"}
        }"#;
        let req: PermissionRequest = serde_json::from_str(json).expect("list payload parses");
        assert_eq!(req.id, "per_126b00d0f001cyU1H4us7xUdRg");
        assert_eq!(req.session_id, "ses_edb2968abffe");
        assert_eq!(req.permission, "bash");
        assert_eq!(req.patterns.len(), 2);
        assert_eq!(req.always.len(), 2);
        assert_eq!(req.tool.as_ref().unwrap().message_id, "msg_1");

        let p = req.into_permission();
        assert_eq!(p.id, "per_126b00d0f001cyU1H4us7xUdRg");
        assert_eq!(p.kind, "bash");
        assert_eq!(p.session_id, "ses_edb2968abffe");
        assert_eq!(p.message_id, "msg_1");
        assert_eq!(p.call_id.as_deref(), Some("call_1"));
        assert_eq!(
            p.pattern,
            Some(PermissionPattern::Many(vec![
                "git add a".into(),
                "git commit -m x".into()
            ]))
        );
    }

    #[test]
    fn permission_request_without_tool_or_patterns_folds_safely() {
        let json = r#"{"id":"per_x","sessionID":"ses_y","permission":"external_directory",
            "patterns":["/tmp/*"],"metadata":{},"always":["/tmp/*"]}"#;
        let req: PermissionRequest = serde_json::from_str(json).expect("parses");
        assert!(req.tool.is_none());
        let p = req.into_permission();
        assert_eq!(p.pattern, Some(PermissionPattern::One("/tmp/*".into())));
        assert_eq!(p.message_id, "");
        assert_eq!(p.title, "external_directory");
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

    // -- provider cost / status / free (T11) --------------------------------

    /// A model carrying the nested `cost` object and its `status`, as `GET
    /// /provider` sends it (captured live: `cost.cache` is a nested object).
    const MODEL_JSON: &str = r#"{
        "id": "anthropic/claude-fable-5",
        "name": "Claude Fable 5",
        "providerID": "anthropic",
        "family": "claude",
        "status": "deprecated",
        "cost": { "input": 3, "output": 15, "cache": { "read": 0.3, "write": 1.25 } }
    }"#;

    #[test]
    fn provider_model_parses_nested_cost_and_status() {
        let model: ProviderModel = serde_json::from_str(MODEL_JSON).expect("model parses");
        let cost = model.cost.expect("cost is modelled");
        assert_eq!(cost.input, 3.0);
        assert_eq!(cost.output, 15.0);
        assert_eq!(cost.cache.read, 0.3);
        assert_eq!(cost.cache.write, 1.25);
        assert_eq!(model.status.as_deref(), Some("deprecated"));
    }

    #[test]
    fn provider_model_is_free_from_zero_cost_or_id_suffix() {
        // Every cost component zero → free, regardless of the id.
        let zero: ProviderModel = serde_json::from_str(
            r#"{"id":"a/zero","name":"Zero","providerID":"a","status":"active",
                "cost":{"input":0,"output":0,"cache":{"read":0,"write":0}}}"#,
        )
        .unwrap();
        assert!(zero.is_free(), "all-zero cost must be free");

        // Any non-zero component (including the nested cache) → not free.
        let paid_input: ProviderModel = serde_json::from_str(
            r#"{"id":"a/paid","name":"Paid","providerID":"a","status":"active",
                "cost":{"input":1,"output":0,"cache":{"read":0,"write":0}}}"#,
        )
        .unwrap();
        assert!(!paid_input.is_free(), "non-zero input must not be free");

        let paid_cache: ProviderModel = serde_json::from_str(
            r#"{"id":"a/paid-cache","name":"Paid Cache","providerID":"a","status":"active",
                "cost":{"input":0,"output":0,"cache":{"read":0,"write":2}}}"#,
        )
        .unwrap();
        assert!(
            !paid_cache.is_free(),
            "non-zero nested cache cost must not be free"
        );

        // The explicit `:free` / `-free` id suffixes mark a model free even when
        // the catalogue reports a non-zero cost.
        let suffix_colon: ProviderModel = serde_json::from_str(
            r#"{"id":"a/x:free","name":"X","providerID":"a","status":"active",
                "cost":{"input":9,"output":9,"cache":{"read":9,"write":9}}}"#,
        )
        .unwrap();
        assert!(suffix_colon.is_free(), "`:free` id suffix must be free");

        let suffix_dash: ProviderModel = serde_json::from_str(
            r#"{"id":"a/y-free","name":"Y","providerID":"a","status":"active",
                "cost":{"input":9,"output":9,"cache":{"read":9,"write":9}}}"#,
        )
        .unwrap();
        assert!(suffix_dash.is_free(), "`-free` id suffix must be free");
    }

    #[test]
    fn provider_model_serializes_the_free_flag_for_the_webview() {
        let zero: ProviderModel = serde_json::from_str(
            r#"{"id":"a/zero","name":"Zero","providerID":"a","status":"active",
                "cost":{"input":0,"output":0,"cache":{"read":0,"write":0}}}"#,
        )
        .unwrap();
        let out = serde_json::to_value(&zero).unwrap();
        assert_eq!(out["free"], serde_json::json!(true));
        assert_eq!(out["cost"]["cache"]["read"], serde_json::json!(0.0));
        assert_eq!(out["providerID"], "a");
    }

    // -- provider usability (T11) -------------------------------------------

    const PROVIDER_LIST_JSON: &str = r#"{
        "all": [
            {"id":"opencode","name":"OpenCode","source":"custom","env":[],"models":{}},
            {"id":"anthropic","name":"Anthropic","source":"env","env":[],"models":{}},
            {"id":"openai","name":"OpenAI","source":"env","env":[],"models":{}},
            {"id":"nearai","name":"NearAI","source":"env","env":[],"models":{}}
        ],
        "default": {},
        "connected": ["opencode", "anthropic"]
    }"#;

    #[test]
    fn provider_list_classifies_usable_providers_from_connected_and_auth() {
        let mut list: ProviderList =
            serde_json::from_str(PROVIDER_LIST_JSON).expect("provider list parses");

        // Usability comes from `connected`: an unauthenticated provider cannot
        // serve a model this app sends, even when a sign-in path exists.
        assert!(list.is_usable("opencode"));
        assert!(list.is_usable("anthropic"));
        assert!(
            !list.is_usable("openai"),
            "has-auth is not enough on its own"
        );
        assert!(!list.is_usable("nearai"));

        // `classify_usability` stamps both signals per provider so the webview
        // gets them without re-deriving the rule.
        list.classify_usability(["openai"].into_iter());
        let by_id = |id: &str| list.all.iter().find(|p| p.id == id).unwrap();
        assert!(by_id("opencode").usable);
        assert!(!by_id("opencode").has_auth);
        assert!(by_id("anthropic").usable);
        assert!(!by_id("openai").usable);
        assert!(by_id("openai").has_auth);
        assert!(!by_id("nearai").usable);
        assert!(!by_id("nearai").has_auth);
    }

    #[test]
    fn provider_list_classifies_api_key_support_from_auth_methods() {
        let mut list: ProviderList =
            serde_json::from_str(PROVIDER_LIST_JSON).expect("provider list parses");

        let method = |kind: &str| ProviderAuthMethod {
            auth_type: kind.to_string(),
            label: format!("{kind} method"),
            prompts: Vec::new(),
        };
        let methods: HashMap<String, Vec<ProviderAuthMethod>> = HashMap::from([
            // Offers an API key alongside OAuth.
            ("openai".to_string(), vec![method("oauth"), method("api")]),
            // API key only.
            ("anthropic".to_string(), vec![method("api")]),
            // OAuth only — has auth, but no key entry.
            ("opencode".to_string(), vec![method("oauth")]),
            // Declares auth but with no methods (defensive).
            ("nearai".to_string(), Vec::new()),
        ]);

        list.classify_api_key_auth(&methods);
        let by_id = |id: &str| list.all.iter().find(|p| p.id == id).unwrap();
        assert!(by_id("openai").offers_api_key, "api among oauth counts");
        assert!(by_id("anthropic").offers_api_key);
        assert!(
            !by_id("opencode").offers_api_key,
            "an oauth-only provider must not advertise a key entry"
        );
        assert!(!by_id("nearai").offers_api_key);
    }

    #[test]
    fn provider_auth_method_is_api_key_only_for_the_api_type() {
        let api = ProviderAuthMethod {
            auth_type: "api".to_string(),
            label: "API Key".to_string(),
            prompts: Vec::new(),
        };
        let oauth = ProviderAuthMethod {
            auth_type: "oauth".to_string(),
            label: "ChatGPT Pro/Plus".to_string(),
            prompts: Vec::new(),
        };
        assert!(api.is_api_key());
        assert!(!oauth.is_api_key());
    }

    #[test]
    fn provider_serializes_offers_api_key_in_camel_case() {
        // The picker reads `provider.offersApiKey`; lock the serialized name so
        // a field rename cannot silently strand the connect affordance.
        let provider: Provider = serde_json::from_value(serde_json::json!({
            "id": "anthropic",
            "name": "Anthropic",
            "source": "env",
            "models": {},
        }))
        .expect("a minimal provider parses");
        let value = serde_json::to_value(&provider).expect("provider serializes");
        let object = value.as_object().expect("provider serializes to an object");
        assert!(
            object.contains_key("offersApiKey"),
            "the field must serialize as `offersApiKey` for the frontend"
        );
        assert!(
            !object.contains_key("offers_api_key"),
            "the snake_case name must never reach the wire"
        );
        assert_eq!(
            object
                .get("offersApiKey")
                .and_then(serde_json::Value::as_bool),
            Some(false),
            "an unclassified provider defaults to offering no key entry"
        );
    }

    // -- session effective model (carried from the wire) --------------------

    /// A `GET /session` row as the server sends it. The effective model lives
    /// under `model` with the field name `id`, NOT `modelID`.
    const SESSION_WITH_MODEL_JSON: &str = r#"{
        "id": "ses_1",
        "projectID": "prj",
        "directory": "/d",
        "title": "t",
        "version": "1",
        "time": { "created": 1, "updated": 2 },
        "model": { "id": "claude-fable-5", "providerID": "anthropic" }
    }"#;

    #[test]
    fn session_maps_the_wire_model_into_the_model_ref() {
        let session: Session =
            serde_json::from_str(SESSION_WITH_MODEL_JSON).expect("session parses");
        let model = session.model.expect("the effective model is carried");
        assert_eq!(model.provider_id, "anthropic");
        assert_eq!(model.model_id, "claude-fable-5");
    }

    #[test]
    fn session_without_a_model_yields_none() {
        let json = r#"{
            "id": "ses_2", "projectID": "prj", "directory": "/d",
            "title": "t", "version": "1",
            "time": { "created": 1, "updated": 2 }
        }"#;
        let session: Session = serde_json::from_str(json).expect("session parses");
        assert_eq!(session.model, None);
    }

    #[test]
    fn session_model_round_trips_between_wire_and_canonical_spelling() {
        let session: Session =
            serde_json::from_str(SESSION_WITH_MODEL_JSON).expect("session parses");
        let model = session.model.clone().expect("model present");
        assert_eq!(
            session,
            serde_json::from_value::<Session>(serde_json::to_value(&session).unwrap()).unwrap()
        );
        // Canonical out: serialization emits `modelID`, and it parses back.
        let out = serde_json::to_value(&model).unwrap();
        assert_eq!(out["modelID"], "claude-fable-5");
        assert_eq!(out["providerID"], "anthropic");
        assert_eq!(serde_json::from_value::<ModelRef>(out).unwrap(), model);
        // Wire in: the same value also parses from the server's `id` spelling.
        let wire = serde_json::json!({ "id": "claude-fable-5", "providerID": "anthropic" });
        assert_eq!(serde_json::from_value::<ModelRef>(wire).unwrap(), model);
    }

    // -- questions (T11: batch clarifications) ------------------------------

    /// One `question.asked` frame, both wire revisions (v1 and v2 carry the
    /// same `properties` shape).
    fn question_asked_event(kind: &str) -> Event {
        serde_json::from_str(&format!(
            r#"{{
                "type": "{kind}",
                "properties": {{
                    "id": "que_1",
                    "sessionID": "ses_a",
                    "questions": [
                        {{
                            "question": "Which environment?",
                            "header": "Environment",
                            "options": [
                                {{"label": "staging", "description": "Staging cluster"}},
                                {{"label": "prod", "description": "Production"}}
                            ],
                            "multiple": true,
                            "custom": false
                        }},
                        {{
                            "question": "Anything else to add?",
                            "header": "Notes",
                            "options": []
                        }}
                    ],
                    "tool": {{"messageID": "msg_1", "callID": "call_1"}}
                }}
            }}"#
        ))
        .expect("question.asked event parses")
    }

    #[test]
    fn question_asked_events_parse_in_both_wire_revisions() {
        for kind in ["question.asked", "question.v2.asked"] {
            let event = question_asked_event(kind);
            let properties = match &event {
                Event::QuestionAsked { properties } | Event::QuestionV2Asked { properties } => {
                    properties
                }
                other => panic!("expected a question.asked variant, got {other:?}"),
            };
            assert_eq!(properties.id, "que_1");
            assert_eq!(properties.session_id, "ses_a");
            assert_eq!(properties.questions.len(), 2, "the whole batch is kept");

            let first = &properties.questions[0];
            assert_eq!(first.header, "Environment");
            assert_eq!(first.options[0].label, "staging");
            assert_eq!(first.multiple, Some(true));
            assert_eq!(first.custom, Some(false));
            // No `recommended` exists on the wire today (opencode 1.18): the
            // field is modelled for a future revision and must default to None.
            assert_eq!(first.recommended, None);
            assert_eq!(first.custom.unwrap_or(true), false);

            let second = &properties.questions[1];
            assert_eq!(second.multiple, None);
            // Omitted `custom` means free text is allowed (server default true).
            assert_eq!(second.custom.unwrap_or(true), true);

            let tool = properties.tool.as_ref().expect("tool ref is carried");
            assert_eq!(tool.message_id, "msg_1");
            assert_eq!(tool.call_id, "call_1");
        }
    }

    #[test]
    fn question_replied_carries_answers_and_rejected_carries_only_ids() {
        let replied: Event = serde_json::from_str(
            r#"{"type":"question.replied","properties":{"sessionID":"ses_a","requestID":"que_1","answers":[["staging"],["note"]]}}"#,
        )
        .expect("question.replied parses");
        match replied {
            Event::QuestionReplied { properties } => {
                assert_eq!(properties.request_id, "que_1");
                assert_eq!(
                    properties.answers,
                    vec![vec!["staging".to_string()], vec!["note".to_string()]]
                );
            }
            other => panic!("expected QuestionReplied, got {other:?}"),
        }

        let v2_replied: Event = serde_json::from_str(
            r#"{"type":"question.v2.replied","properties":{"sessionID":"ses_a","requestID":"que_1","answers":[["a","b"]]}}"#,
        )
        .expect("question.v2.replied parses");
        assert!(matches!(
            v2_replied,
            Event::QuestionV2Replied { properties } if properties.answers[0].len() == 2
        ));

        for kind in ["question.rejected", "question.v2.rejected"] {
            let rejected: Event = serde_json::from_str(&format!(
                r#"{{"type":"{kind}","properties":{{"sessionID":"ses_a","requestID":"que_1"}}}}"#
            ))
            .expect("question.rejected parses");
            match rejected {
                Event::QuestionRejected { properties }
                | Event::QuestionV2Rejected { properties } => {
                    assert_eq!(properties.request_id, "que_1");
                    assert_eq!(properties.session_id, "ses_a");
                }
                other => panic!("expected a rejected variant, got {other:?}"),
            }
        }
    }

    #[test]
    fn question_info_round_trips_without_inventing_a_recommended_choice() {
        let wire = r#"{
            "question": "Pick one",
            "header": "Pick",
            "options": [{"label": "a", "description": "first"}]
        }"#;
        let info: QuestionInfo = serde_json::from_str(wire).expect("parses");
        assert_eq!(info.recommended, None);
        let out = serde_json::to_value(&info).unwrap();
        assert_eq!(out["recommended"], serde_json::Value::Null);
        // The options keep their required `description` for the UI.
        assert_eq!(out["options"][0]["description"], "first");
    }
}
