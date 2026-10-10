//! Pure in-memory state: the session tree, per-agent status derivation and the
//! pending-approval registry.
//!
//! **Hard rule: no I/O in this module.** Everything here is a synchronous
//! reducer over already-decoded data so that it can be unit-tested without a
//! network, a runtime or a server (`cargo test -p orquestador-core`).
//! All HTTP lives in [`crate::client`]; all orchestration in [`crate::events`].

use crate::types::{Agent, Event, Permission, PermissionRequest, Session, SessionStatus, Todo};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Derived, UI facing status of a session (sub-agent).
///
/// Priority (highest first): `WaitingApproval` → `Error` → `Running` →
/// `Done` → `Idle` → `Pending`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentUiStatus {
    /// No status known yet (just created / before the first `/session/status`).
    Pending,
    /// Server says `idle` and the session never did anything.
    Idle,
    /// Server says `idle` after having been busy → the run completed.
    Done,
    /// Server says `busy` or `retry`.
    Running,
    /// A `permission.updated` is waiting for the human.
    WaitingApproval,
    /// A `session.error` arrived and no new run has started since.
    Error,
}

/// Desktop notification requested by the reducer. The Tauri shell decides
/// whether to actually show it (e.g. skip when the window is focused).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// A sub-agent finished its run.
    Finished { session_id: String, title: String },
    /// A sub-agent run failed.
    Failed {
        session_id: String,
        title: String,
        message: String,
    },
    /// A sub-agent is blocked waiting for approval.
    Approval {
        session_id: String,
        permission_id: String,
        title: String,
    },
}

/// Serializable mirror of [`AppState`] pushed to the webview.
///
/// Deliberately does **not** contain messages/parts: those stream at high
/// frequency and live only in the frontend display cache.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppStateSnapshot {
    pub connected: bool,
    pub server_version: Option<String>,
    pub root_session_id: Option<String>,
    /// Sorted newest first (deterministic — asserted by tests).
    pub sessions: Vec<SessionSnapshot>,
    /// Permissions still waiting for an answer.
    pub permissions: Vec<Permission>,
    pub agents: Vec<Agent>,
    pub todos: HashMap<String, Vec<Todo>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionSnapshot {
    pub session: Session,
    pub status: AgentUiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Raw server status, kept so the UI can render the `retry` countdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_status: Option<SessionStatus>,
    /// Epoch ms when the derived status last changed.
    ///
    /// Approximate for the first observation of a session: the true transition
    /// may have happened before the app started, so the current time is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_since: Option<i64>,
    /// Duration in ms of the previous run (set when leaving
    /// `Running`/`WaitingApproval`; `WaitingApproval` is part of the run).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_ms: Option<i64>,
}

/// Where a pending permission came from, so a successful `GET /permission`
/// refresh can tell a genuinely withdrawn entry from one that raced in over SSE
/// after the fetch was already in flight (see
/// [`AppState::apply_pending_permissions_at`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSource {
    /// Added by a `GET /permission` list fetch.
    List,
    /// Added by a `permission.updated` SSE event seen at this epoch ms.
    Event { seen_at_ms: i64 },
}

/// The single source of truth for everything the UI renders as *data*
/// (as opposed to the message stream, which is display-only).
#[derive(Debug, Default)]
pub struct AppState {
    connected: bool,
    server_version: Option<String>,
    root_session_id: Option<String>,
    sessions: HashMap<String, Session>,
    statuses: HashMap<String, SessionStatus>,
    /// Sessions that have been busy at least once since their last idle.
    active: HashSet<String>,
    /// Sessions that reached a clean idle after running (UI shows `Done`).
    finished: HashSet<String>,
    /// Sticky error message per session, cleared when a new run starts.
    errored: HashMap<String, String>,
    /// Permissions that have not been answered yet, keyed by permission id.
    pending: HashMap<String, Permission>,
    /// Origin / last-seen of every `pending` entry (see [`PendingSource`]).
    pending_source: HashMap<String, PendingSource>,
    todos: HashMap<String, Vec<Todo>>,
    agents: Vec<Agent>,
    /// Last derived status seen per session (to detect transitions).
    last_status: HashMap<String, AgentUiStatus>,
    /// Epoch ms when the derived status last changed (see [`SessionSnapshot`]).
    status_since: HashMap<String, i64>,
    /// Epoch ms when the current run window began (`Running`/`WaitingApproval`).
    run_started: HashMap<String, i64>,
    /// Duration in ms of the previous completed run, per session.
    last_run_ms: HashMap<String, i64>,
}

/// Is this derived status part of a run window? (`WaitingApproval` counts: the
/// agent is blocked mid-run, not finished.)
fn is_run_status(status: AgentUiStatus) -> bool {
    matches!(
        status,
        AgentUiStatus::Running | AgentUiStatus::WaitingApproval
    )
}

/// Wall clock in epoch ms. `AppState` is pure apart from this single read; the
/// `*_at` mutators take the timestamp explicitly so tests stay deterministic.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl AppState {
    // -- connection ---------------------------------------------------------

    pub fn set_connection(&mut self, connected: bool, version: Option<String>) {
        self.connected = connected;
        if version.is_some() {
            self.server_version = version;
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn set_root_session(&mut self, id: impl Into<String>) {
        self.root_session_id = Some(id.into());
    }

    pub fn root_session_id(&self) -> Option<&str> {
        self.root_session_id.as_deref()
    }

    // -- bootstrap (results of plain HTTP calls, no events involved) --------

    pub fn apply_sessions(&mut self, sessions: Vec<Session>) {
        self.apply_sessions_at(sessions, now_ms());
    }

    /// [`Self::apply_sessions`] with an explicit clock (tests).
    pub fn apply_sessions_at(&mut self, sessions: Vec<Session>, now_ms: i64) {
        for s in sessions {
            let id = s.id.clone();
            self.sessions.insert(id.clone(), s);
            self.observe(&id, now_ms);
        }
    }

    pub fn apply_statuses(&mut self, statuses: HashMap<String, SessionStatus>) {
        self.apply_statuses_at(statuses, now_ms());
    }

    /// [`Self::apply_statuses`] with an explicit clock (tests).
    pub fn apply_statuses_at(&mut self, statuses: HashMap<String, SessionStatus>, now_ms: i64) {
        for (id, st) in &statuses {
            if matches!(st, SessionStatus::Busy | SessionStatus::Retry { .. }) {
                self.active.insert(id.clone());
                self.finished.remove(id);
                self.errored.remove(id);
            }
        }
        let ids: Vec<String> = statuses.keys().cloned().collect();
        self.statuses.extend(statuses);
        for id in ids {
            self.observe(&id, now_ms);
        }
    }

    pub fn apply_agents(&mut self, agents: Vec<Agent>) {
        self.agents = agents;
    }

    /// Merge a completed `GET /permission` fetch into the registry, keyed by the
    /// same `per_…` id the reply call needs.
    ///
    /// A **successful list fetch is authoritative**: a tracked entry the response
    /// omits is dropped (answered elsewhere, withdrawn, or lost to a dead
    /// stream). An entry that arrived over `permission.updated` *after* the fetch
    /// started is the one exception — the response may simply not know about it
    /// yet — so the caller must pass the fetch start to
    /// [`Self::apply_pending_permissions_at`]; this convenience wrapper uses the
    /// current time and is meant for tests / best-effort callers.
    ///
    /// Removing an entry never emits an effect, and an id already tracked is left
    /// untouched (its richer payload preserved, its `Effect::Approval` not
    /// re-emitted), so an answered approval can never be resurrected. Returns one
    /// [`Effect::Approval`] per newly discovered permission.
    pub fn apply_pending_permissions(&mut self, requests: Vec<PermissionRequest>) -> Vec<Effect> {
        self.apply_pending_permissions_at(requests, now_ms())
    }

    /// [`Self::apply_pending_permissions`] with the explicit instant the fetch
    /// was sent (tests use fixed clocks, the event hub passes the real start).
    pub fn apply_pending_permissions_at(
        &mut self,
        requests: Vec<PermissionRequest>,
        fetched_at_ms: i64,
    ) -> Vec<Effect> {
        let listed: HashSet<&str> = requests.iter().map(|r| r.id.as_str()).collect();

        // Removals first: the list is authoritative for everything it could have
        // seen. An event-sourced entry newer than the fetch start may be missing
        // from the response purely because of the request/response race, so keep
        // it. No effect is emitted for a removal.
        let stale: Vec<String> = self
            .pending
            .keys()
            .filter(|id| !listed.contains(id.as_str()))
            .filter(|id| match self.pending_source.get(id.as_str()) {
                Some(PendingSource::Event { seen_at_ms }) => *seen_at_ms <= fetched_at_ms,
                Some(PendingSource::List) | None => true,
            })
            .cloned()
            .collect();
        for id in stale {
            if let Some(permission) = self.pending.remove(&id) {
                self.pending_source.remove(&id);
                self.observe(&permission.session_id, fetched_at_ms);
            }
        }

        let mut effects = Vec::new();
        for request in requests {
            let id = request.id.clone();
            if self.pending.contains_key(&id) {
                // Server-confirmed: it is safe to remove on a future fetch.
                self.pending_source.insert(id, PendingSource::List);
                continue;
            }
            let permission = request.into_permission();
            let session_id = permission.session_id.clone();
            effects.push(Effect::Approval {
                session_id: session_id.clone(),
                permission_id: permission.id.clone(),
                title: permission.title.clone(),
            });
            self.pending.insert(permission.id.clone(), permission);
            self.pending_source.insert(id, PendingSource::List);
            self.observe(&session_id, fetched_at_ms);
        }
        effects
    }

    /// Drop a permission from the registry because it is no longer answerable
    /// (the server answered `404` for it, or the reply succeeded). Emits no
    /// effects and returns `true` when the entry was present.
    pub fn remove_permission(&mut self, id: &str) -> bool {
        self.remove_permission_at(id, now_ms())
    }

    /// [`Self::remove_permission`] with an explicit clock (tests).
    pub fn remove_permission_at(&mut self, id: &str, now_ms: i64) -> bool {
        let Some(permission) = self.pending.remove(id) else {
            return false;
        };
        self.pending_source.remove(id);
        self.observe(&permission.session_id, now_ms);
        true
    }

    // -- reads --------------------------------------------------------------

    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values()
    }

    /// Direct children of `parent_id` (the sub-agents), newest first.
    pub fn children_of(&self, parent_id: &str) -> Vec<&Session> {
        let mut kids: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| s.parent_id.as_deref() == Some(parent_id))
            .collect();
        kids.sort_by(|a, b| b.time.created.cmp(&a.time.created).then(a.id.cmp(&b.id)));
        kids
    }

    /// Permissions still waiting for a human answer, oldest first.
    pub fn pending_permissions(&self) -> Vec<&Permission> {
        let mut list: Vec<&Permission> = self.pending.values().collect();
        list.sort_by_key(|p| p.time.created);
        list
    }

    pub fn has_pending_permission(&self, session_id: &str) -> bool {
        self.pending.values().any(|p| p.session_id == session_id)
    }

    /// Derived status for one session (see [`AgentUiStatus`]).
    pub fn agent_status(&self, session_id: &str) -> AgentUiStatus {
        if self.pending.values().any(|p| p.session_id == session_id) {
            return AgentUiStatus::WaitingApproval;
        }
        if let Some(msg) = self.errored.get(session_id) {
            if !msg.is_empty() {
                return AgentUiStatus::Error;
            }
        }
        match self.statuses.get(session_id) {
            Some(SessionStatus::Busy) | Some(SessionStatus::Retry { .. }) => AgentUiStatus::Running,
            _ if self.finished.contains(session_id) => AgentUiStatus::Done,
            Some(SessionStatus::Idle) => AgentUiStatus::Idle,
            None => {
                if self.sessions.contains_key(session_id) {
                    AgentUiStatus::Idle
                } else {
                    AgentUiStatus::Pending
                }
            }
        }
    }

    // -- reducer ------------------------------------------------------------

    /// Apply one decoded SSE event. Returns the side effects (desktop
    /// notifications) the caller should act on. State changes always happen;
    /// effects are derived from transitions so that repeats are harmless.
    pub fn apply_event(&mut self, event: &Event) -> Vec<Effect> {
        self.apply_event_at(event, now_ms())
    }

    /// [`Self::apply_event`] with an explicit clock (tests).
    pub fn apply_event_at(&mut self, event: &Event, now_ms: i64) -> Vec<Effect> {
        let mut effects = Vec::new();
        match event {
            Event::SessionCreated { properties } | Event::SessionUpdated { properties } => {
                let id = properties.info.id.clone();
                self.sessions.insert(id.clone(), properties.info.clone());
                self.observe(&id, now_ms);
            }
            Event::SessionDeleted { properties } => {
                let id = properties.info.id.clone();
                self.sessions.remove(&id);
                self.statuses.remove(&id);
                self.active.remove(&id);
                self.finished.remove(&id);
                self.errored.remove(&id);
                self.todos.remove(&id);
                self.pending.retain(|_, p| p.session_id != id);
                let live: HashSet<String> = self.pending.keys().cloned().collect();
                self.pending_source.retain(|pid, _| live.contains(pid));
                self.forget(&id);
                if self.root_session_id.as_deref() == Some(id.as_str()) {
                    self.root_session_id = None;
                }
            }
            Event::SessionStatus { properties } => {
                self.apply_status(&properties.session_id, &properties.status, &mut effects);
                self.observe(&properties.session_id, now_ms);
            }
            Event::SessionIdle { properties } => {
                self.on_idle(&properties.session_id, &mut effects);
                self.observe(&properties.session_id, now_ms);
            }
            Event::SessionError { properties } => {
                if let Some(id) = properties.session_id.as_deref() {
                    let message = properties
                        .error
                        .as_ref()
                        .and_then(|e| e.get("data"))
                        .and_then(|d| d.get("message"))
                        .and_then(|m| m.as_str())
                        .map(str::to_owned)
                        .unwrap_or_else(|| "session error".to_string());
                    // Only notify on the not-errored → errored transition so a
                    // burst of error events cannot spam notifications.
                    if !self.errored.contains_key(id) {
                        let title = self.title_of(id);
                        effects.push(Effect::Failed {
                            session_id: id.to_string(),
                            title,
                            message: message.clone(),
                        });
                    }
                    self.errored.insert(id.to_string(), message);
                    self.observe(id, now_ms);
                }
            }
            Event::PermissionUpdated { properties } => {
                let is_new = !self.pending.contains_key(&properties.id);
                self.pending
                    .insert(properties.id.clone(), properties.clone());
                // Record when the stream saw it: a later list fetch started
                // before this instant must not drop it as "withdrawn".
                self.pending_source.insert(
                    properties.id.clone(),
                    PendingSource::Event { seen_at_ms: now_ms },
                );
                if is_new {
                    effects.push(Effect::Approval {
                        session_id: properties.session_id.clone(),
                        permission_id: properties.id.clone(),
                        title: properties.title.clone(),
                    });
                }
                self.observe(&properties.session_id, now_ms);
            }
            Event::PermissionReplied { properties } => {
                self.pending.remove(&properties.permission_id);
                self.pending_source.remove(&properties.permission_id);
                self.observe(&properties.session_id, now_ms);
            }
            Event::TodoUpdated { properties } => {
                self.todos
                    .insert(properties.session_id.clone(), properties.todos.clone());
            }
            // Display-only events: they stream to the webview untouched.
            Event::MessageUpdated { .. }
            | Event::MessageRemoved { .. }
            | Event::PartUpdated { .. }
            | Event::PartRemoved { .. }
            | Event::SessionDiff { .. }
            | Event::FileEdited { .. }
            | Event::FileWatcher { .. }
            | Event::ServerConnected { .. }
            | Event::Unknown => {}
        }
        effects
    }

    fn apply_status(
        &mut self,
        session_id: &str,
        status: &SessionStatus,
        effects: &mut Vec<Effect>,
    ) {
        match status {
            SessionStatus::Busy | SessionStatus::Retry { .. } => {
                self.active.insert(session_id.to_string());
                self.finished.remove(session_id);
                // A new run supersedes the previous failure.
                self.errored.remove(session_id);
            }
            SessionStatus::Idle => self.on_idle(session_id, effects),
        }
        self.statuses.insert(session_id.to_string(), status.clone());
    }

    /// Shared by `session.idle` and `session.status{idle}`. Idempotent: the
    /// second call for the same run finds `active` empty and does nothing.
    fn on_idle(&mut self, session_id: &str, effects: &mut Vec<Effect>) {
        self.statuses
            .insert(session_id.to_string(), SessionStatus::Idle);
        if self.errored.remove(session_id).is_some() {
            // The failure was already reported; just clean up.
            self.active.remove(session_id);
            return;
        }
        if self.active.remove(session_id) {
            self.finished.insert(session_id.to_string());
            if self.sessions.contains_key(session_id) {
                effects.push(Effect::Finished {
                    session_id: session_id.to_string(),
                    title: self.title_of(session_id),
                });
            }
        }
    }

    /// Record the derived status transition for one session at `now_ms`.
    ///
    /// * `status_since` advances whenever the derived status changes;
    /// * the run window starts when the session enters `Running`/
    ///   `WaitingApproval` and ends when it leaves both, at which point
    ///   `last_run_ms` records the elapsed time.
    fn observe(&mut self, session_id: &str, now_ms: i64) {
        let status = self.agent_status(session_id);
        let prev = self.last_status.get(session_id).copied();
        if prev == Some(status) {
            return;
        }
        let was_in_run = matches!(prev, Some(s) if is_run_status(s));
        let now_in_run = is_run_status(status);
        if now_in_run && !was_in_run {
            self.run_started.insert(session_id.to_string(), now_ms);
        } else if was_in_run && !now_in_run {
            if let Some(start) = self.run_started.remove(session_id) {
                self.last_run_ms
                    .insert(session_id.to_string(), (now_ms - start).max(0));
            }
        }
        self.last_status.insert(session_id.to_string(), status);
        self.status_since.insert(session_id.to_string(), now_ms);
    }

    /// Drop every timer-tracking trace of a session (used on delete).
    fn forget(&mut self, session_id: &str) {
        self.last_status.remove(session_id);
        self.status_since.remove(session_id);
        self.run_started.remove(session_id);
        self.last_run_ms.remove(session_id);
    }

    fn title_of(&self, session_id: &str) -> String {
        self.sessions
            .get(session_id)
            .map(|s| {
                if s.title.trim().is_empty() {
                    "sub-agent".to_string()
                } else {
                    s.title.clone()
                }
            })
            .unwrap_or_else(|| "sub-agent".to_string())
    }

    // -- snapshot -----------------------------------------------------------

    pub fn snapshot(&self) -> AppStateSnapshot {
        let mut sessions: Vec<SessionSnapshot> = self
            .sessions
            .values()
            .map(|session| SessionSnapshot {
                session: session.clone(),
                status: self.agent_status(&session.id),
                error: self.errored.get(&session.id).cloned(),
                server_status: self.statuses.get(&session.id).cloned(),
                status_since: self.status_since.get(&session.id).copied(),
                last_run_ms: self.last_run_ms.get(&session.id).copied(),
            })
            .collect();
        // Newest first, id as tie-breaker → stable ordering for the UI + tests.
        sessions.sort_by(|a, b| {
            b.session
                .time
                .created
                .cmp(&a.session.time.created)
                .then(a.session.id.cmp(&b.session.id))
        });

        let mut permissions: Vec<Permission> = self.pending.values().cloned().collect();
        permissions.sort_by_key(|p| p.time.created);

        AppStateSnapshot {
            connected: self.connected,
            server_version: self.server_version.clone(),
            root_session_id: self.root_session_id.clone(),
            sessions,
            permissions,
            agents: self.agents.clone(),
            todos: self.todos.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PermissionRequest, SessionTime};

    fn session(id: &str, parent: Option<&str>, created: i64) -> Session {
        Session {
            id: id.into(),
            project_id: "prj".into(),
            directory: "/tmp".into(),
            parent_id: parent.map(str::to_owned),
            title: format!("title-{id}"),
            version: "1".into(),
            time: SessionTime {
                created,
                updated: created,
                compacting: None,
            },
            summary: None,
        }
    }

    fn event(json: &str) -> Event {
        serde_json::from_str(json).expect("event fixture parses")
    }

    /// A `permission.updated` SSE event for `id` in session `a`.
    fn permission_event(id: &str) -> Event {
        event(&format!(
            r#"{{"type":"permission.updated","properties":{{"id":"{id}","type":"bash","sessionID":"a","messageID":"m1","title":"Run","metadata":{{}},"time":{{"created":10}}}}}}"#
        ))
    }

    /// A `GET /permission` list entry for `id` in session `a`.
    fn permission_request(id: &str) -> PermissionRequest {
        PermissionRequest {
            id: id.into(),
            session_id: "a".into(),
            permission: "bash".into(),
            patterns: vec!["git commit -m x".into()],
            metadata: serde_json::json!({ "command": "git commit -m x" }),
            always: vec!["git commit *".into()],
            tool: None,
        }
    }

    #[test]
    fn session_tree_lists_children_newest_first() {
        let mut st = AppState::default();
        st.apply_sessions(vec![
            session("root", None, 1),
            session("child_a", Some("root"), 2),
            session("child_b", Some("root"), 3),
            session("other", None, 4),
        ]);
        let ids: Vec<&str> = st
            .children_of("root")
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(ids, ["child_b", "child_a"]);
        assert!(st.children_of("other").is_empty());
    }

    #[test]
    fn busy_then_idle_reports_finished_exactly_once() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);

        let busy = event(
            r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"busy"}}}"#,
        );
        assert!(st.apply_event(&busy).is_empty());
        assert_eq!(st.agent_status("a"), AgentUiStatus::Running);

        let idle = event(r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#);
        let effects = st.apply_event(&idle);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            Effect::Finished { ref session_id, ref title } if session_id == "a" && title == "title-a"
        ));
        assert_eq!(st.agent_status("a"), AgentUiStatus::Done);

        // A duplicate idle (status event + idle event) must NOT notify twice.
        let status_idle = event(
            r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"idle"}}}"#,
        );
        assert!(st.apply_event(&status_idle).is_empty());
        assert!(st.apply_event(&idle).is_empty());
    }

    #[test]
    fn a_session_that_never_ran_stays_idle_not_done() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("fresh", None, 1)]);
        assert_eq!(st.agent_status("fresh"), AgentUiStatus::Idle);
        st.apply_statuses(
            [("fresh".to_string(), SessionStatus::Idle)]
                .into_iter()
                .collect(),
        );
        assert_eq!(st.agent_status("fresh"), AgentUiStatus::Idle);
    }

    #[test]
    fn error_is_sticky_until_the_next_run_and_notifies_once() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_statuses(
            [("a".to_string(), SessionStatus::Busy)]
                .into_iter()
                .collect(),
        );

        let err = event(
            r#"{"type":"session.error","properties":{"sessionID":"a","error":{"name":"UnknownError","data":{"message":"kaput"}}}}"#,
        );
        let fx = st.apply_event(&err);
        assert_eq!(fx.len(), 1);
        assert!(matches!(&fx[0], Effect::Failed { message, .. } if message == "kaput"));
        assert_eq!(st.agent_status("a"), AgentUiStatus::Error);

        // Repeat of the same error → no second notification, still Error.
        assert!(st.apply_event(&err).is_empty());
        assert_eq!(st.agent_status("a"), AgentUiStatus::Error);

        // New run clears the sticky error.
        let busy = event(
            r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"busy"}}}"#,
        );
        st.apply_event(&busy);
        assert_eq!(st.agent_status("a"), AgentUiStatus::Running);

        // Finishing after an error run must not also shout "Finished".
        st.apply_event(&event(
            r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#,
        ));
        assert_eq!(st.agent_status("a"), AgentUiStatus::Done);
    }

    #[test]
    fn error_then_idle_does_not_emit_a_second_notification() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_event(&event(
            r#"{"type":"session.error","properties":{"sessionID":"a","error":{"data":{"message":"boom"}}}}"#,
        ));
        let fx = st.apply_event(&event(
            r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#,
        ));
        assert!(fx.is_empty(), "idle after error must stay quiet: {fx:?}");
        assert_eq!(st.agent_status("a"), AgentUiStatus::Idle);
    }

    #[test]
    fn pending_permission_wins_over_everything_and_notifies_once() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_statuses(
            [("a".to_string(), SessionStatus::Busy)]
                .into_iter()
                .collect(),
        );

        let upd = event(
            r#"{"type":"permission.updated","properties":{"id":"perm_1","type":"edit","sessionID":"a","messageID":"m1","title":"Edit file","metadata":{},"time":{"created":10}}}"#,
        );
        let fx = st.apply_event(&upd);
        assert_eq!(fx.len(), 1);
        assert!(
            matches!(&fx[0], Effect::Approval { permission_id, .. } if permission_id == "perm_1")
        );
        assert_eq!(st.agent_status("a"), AgentUiStatus::WaitingApproval);
        assert!(st.has_pending_permission("a"));

        // Same permission re-sent (server refresh) → no duplicate notification.
        assert!(st.apply_event(&upd).is_empty());

        // Replying clears it and falls back to Running (still busy).
        let replied = event(
            r#"{"type":"permission.replied","properties":{"sessionID":"a","permissionID":"perm_1","response":"once"}}"#,
        );
        st.apply_event(&replied);
        assert!(!st.has_pending_permission("a"));
        assert_eq!(st.agent_status("a"), AgentUiStatus::Running);
        assert!(st.pending_permissions().is_empty());
    }

    #[test]
    fn running_to_done_records_last_run_ms_and_advances_status_since() {
        let mut st = AppState::default();
        // First observation of the session: `status_since` = now (approximation).
        st.apply_sessions_at(vec![session("a", None, 1)], 1_000);
        let snap = st.snapshot();
        assert_eq!(snap.sessions[0].status_since, Some(1_000));
        assert_eq!(snap.sessions[0].last_run_ms, None);

        st.apply_event_at(
            &event(r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"busy"}}}"#),
            2_000,
        );
        let snap = st.snapshot();
        assert_eq!(snap.sessions[0].status, AgentUiStatus::Running);
        assert_eq!(snap.sessions[0].status_since, Some(2_000));

        st.apply_event_at(
            &event(r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#),
            2_500,
        );
        let snap = st.snapshot();
        assert_eq!(snap.sessions[0].status, AgentUiStatus::Done);
        assert_eq!(
            snap.sessions[0].status_since,
            Some(2_500),
            "status_since must advance on the transition"
        );
        assert_eq!(snap.sessions[0].last_run_ms, Some(500));
    }

    #[test]
    fn waiting_approval_counts_as_part_of_the_same_run_window() {
        let mut st = AppState::default();
        st.apply_sessions_at(vec![session("a", None, 1)], 1_000);
        st.apply_event_at(
            &event(r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"busy"}}}"#),
            2_000,
        );
        st.apply_event_at(
            &event(r#"{"type":"permission.updated","properties":{"id":"perm_1","type":"bash","sessionID":"a","messageID":"m1","title":"Run","metadata":{},"time":{"created":10}}}"#),
            2_200,
        );
        let snap = st.snapshot();
        assert_eq!(snap.sessions[0].status, AgentUiStatus::WaitingApproval);
        assert_eq!(snap.sessions[0].status_since, Some(2_200));

        st.apply_event_at(
            &event(r#"{"type":"permission.replied","properties":{"sessionID":"a","permissionID":"perm_1","response":"once"}}"#),
            2_300,
        );
        assert_eq!(st.agent_status("a"), AgentUiStatus::Running);

        st.apply_event_at(
            &event(r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#),
            2_600,
        );
        let snap = st.snapshot();
        assert_eq!(
            snap.sessions[0].last_run_ms,
            Some(600),
            "the run window starts when the session first became busy, WaitingApproval included"
        );
    }

    #[test]
    fn deleting_a_session_clears_timer_state() {
        let mut st = AppState::default();
        st.apply_sessions_at(vec![session("a", None, 1)], 1_000);
        st.apply_event_at(
            &event(r#"{"type":"session.status","properties":{"sessionID":"a","status":{"type":"busy"}}}"#),
            2_000,
        );
        st.apply_event_at(
            &event(r#"{"type":"session.idle","properties":{"sessionID":"a"}}"#),
            2_500,
        );
        assert_eq!(st.snapshot().sessions[0].last_run_ms, Some(500));

        st.apply_event_at(
            &event(r#"{"type":"session.deleted","properties":{"info":{"id":"a","projectID":"p","directory":"/d","title":"t","version":"1","time":{"created":1,"updated":2}}}}"#),
            3_000,
        );
        assert!(st.session("a").is_none());

        // Re-adding the same id must start a fresh timer, not inherit the old one.
        st.apply_sessions_at(vec![session("a", None, 1)], 4_000);
        let snap = st.snapshot();
        assert_eq!(snap.sessions[0].status_since, Some(4_000));
        assert_eq!(snap.sessions[0].last_run_ms, None);
    }

    #[test]
    fn apply_pending_permissions_surfaces_unknown_serverside_permissions_once() {
        use crate::types::PermissionRequest;

        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_statuses(
            [("a".to_string(), SessionStatus::Busy)]
                .into_iter()
                .collect(),
        );

        let request = || PermissionRequest {
            id: "per_1".into(),
            session_id: "a".into(),
            permission: "bash".into(),
            patterns: vec!["git commit -m x".into()],
            metadata: serde_json::json!({ "command": "git commit -m x" }),
            always: vec!["git commit *".into()],
            tool: None,
        };

        let fx = st.apply_pending_permissions(vec![request()]);
        assert_eq!(fx.len(), 1);
        assert!(
            matches!(&fx[0], Effect::Approval { permission_id, .. } if permission_id == "per_1")
        );
        assert_eq!(st.agent_status("a"), AgentUiStatus::WaitingApproval);
        assert_eq!(st.pending_permissions().len(), 1);

        // Re-applying the same server truth must NOT re-notify.
        assert!(st.apply_pending_permissions(vec![request()]).is_empty());
        assert_eq!(st.pending_permissions().len(), 1);
    }

    #[test]
    fn a_successful_list_fetch_removes_a_list_sourced_permission_that_is_gone() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_pending_permissions_at(vec![permission_request("per_1")], 1_000);
        assert_eq!(st.pending_permissions().len(), 1);

        // A later fetch (started at t=2000) no longer lists it.
        let fx = st.apply_pending_permissions_at(Vec::new(), 2_000);
        assert!(fx.is_empty(), "removals must not emit effects: {fx:?}");
        assert!(
            st.pending_permissions().is_empty(),
            "a permission the server no longer lists must be dropped"
        );
    }

    #[test]
    fn an_event_sourced_permission_older_than_the_fetch_is_removed_when_absent() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_event_at(&permission_event("per_old"), 1_000);
        assert_eq!(st.pending_permissions().len(), 1);

        // The fetch started at t=2000, AFTER the event was seen (t=1000), so a
        // response that omits it is authoritative: it is gone.
        let fx = st.apply_pending_permissions_at(Vec::new(), 2_000);
        assert!(fx.is_empty());
        assert!(st.pending_permissions().is_empty());
    }

    #[test]
    fn an_event_that_arrives_after_the_fetch_started_is_not_dropped() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        // The event raced in at t=3000, while the fetch was already in flight
        // (it started at t=2000), so the response cannot know about it.
        st.apply_event_at(&permission_event("per_race"), 3_000);

        let fx = st.apply_pending_permissions_at(Vec::new(), 2_000);
        assert!(fx.is_empty());
        assert_eq!(
            st.pending_permissions().len(),
            1,
            "an approval that raced in after the fetch began must survive"
        );
        assert_eq!(st.pending_permissions()[0].id, "per_race");
    }

    #[test]
    fn a_permission_still_listed_survives_the_fetch() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_pending_permissions_at(vec![permission_request("per_1")], 1_000);
        st.apply_pending_permissions_at(vec![permission_request("per_1")], 2_000);
        assert_eq!(st.pending_permissions().len(), 1);
    }

    #[test]
    fn a_fetch_removed_permission_is_not_resurrected_by_later_empty_fetches() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_pending_permissions_at(vec![permission_request("per_1")], 1_000);
        st.apply_pending_permissions_at(Vec::new(), 2_000);
        assert!(st.pending_permissions().is_empty());

        for t in [3_000, 4_000, 5_000] {
            assert!(st.apply_pending_permissions_at(Vec::new(), t).is_empty());
            assert!(st.pending_permissions().is_empty());
        }
    }

    #[test]
    fn a_replied_permission_is_not_resurrected_by_a_later_list_fetch_that_omits_it() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_event_at(&permission_event("perm_1"), 1_000);
        st.apply_event_at(
            &event(
                r#"{"type":"permission.replied","properties":{"sessionID":"a","permissionID":"perm_1","response":"once"}}"#,
            ),
            1_500,
        );
        assert!(st.pending_permissions().is_empty());

        // Reconcile started at 2000 sees nothing → still empty, no effects.
        assert!(st
            .apply_pending_permissions_at(Vec::new(), 2_000)
            .is_empty());
        assert!(st.pending_permissions().is_empty());
    }

    #[test]
    fn remove_permission_drops_it_locally_and_emits_no_effects() {
        let mut st = AppState::default();
        st.apply_sessions_at(vec![session("a", None, 1)], 500);
        st.apply_event_at(&permission_event("perm_1"), 1_000);
        assert_eq!(st.agent_status("a"), AgentUiStatus::WaitingApproval);

        // This is what the shell does on a successful reply or a 404.
        assert!(st.remove_permission_at("perm_1", 2_000));
        assert!(st.pending_permissions().is_empty());
        assert_eq!(st.agent_status("a"), AgentUiStatus::Idle);

        // Removing an unknown id is a harmless no-op.
        assert!(!st.remove_permission_at("ghost", 2_000));
    }

    #[test]
    fn replied_permission_is_not_resurrected_by_an_empty_reconcile() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1)]);
        st.apply_event(&event(
            r#"{"type":"permission.updated","properties":{"id":"perm_1","type":"bash","sessionID":"a","messageID":"m1","title":"Run","metadata":{},"time":{"created":10}}}"#,
        ));
        assert_eq!(st.pending_permissions().len(), 1);

        st.apply_event(&event(
            r#"{"type":"permission.replied","properties":{"sessionID":"a","permissionID":"perm_1","response":"once"}}"#,
        ));
        assert!(st.pending_permissions().is_empty());

        // A reconcile that sees no pending permissions must not resurrect it.
        assert!(st.apply_pending_permissions(Vec::new()).is_empty());
        assert!(st.pending_permissions().is_empty());
    }

    #[test]
    fn deleting_a_session_cleans_all_of_its_traces() {
        let mut st = AppState::default();
        st.apply_sessions(vec![
            session("root", None, 1),
            session("child", Some("root"), 2),
        ]);
        st.apply_statuses(
            [("child".to_string(), SessionStatus::Busy)]
                .into_iter()
                .collect(),
        );
        st.apply_event(&event(
            r#"{"type":"permission.updated","properties":{"id":"p","type":"bash","sessionID":"child","messageID":"m","title":"rm","metadata":{},"time":{"created":1}}}"#,
        ));
        st.set_root_session("root");
        assert_eq!(st.pending_permissions().len(), 1);

        st.apply_event(&event(
            r#"{"type":"session.deleted","properties":{"info":{"id":"root","projectID":"p","directory":"/d","title":"t","version":"1","time":{"created":1,"updated":2}}}}"#,
        ));
        assert!(st.session("root").is_none());
        assert_eq!(st.root_session_id(), None);
        // The child is orphaned but its permission is still shown (we must not
        // silently drop an unanswered approval).
        assert_eq!(st.pending_permissions().len(), 1);

        st.apply_event(&event(
            r#"{"type":"session.deleted","properties":{"info":{"id":"child","projectID":"p","directory":"/d","title":"t","version":"1","time":{"created":1,"updated":2}}}}"#,
        ));
        assert!(st.pending_permissions().is_empty());
    }

    #[test]
    fn bootstrap_applies_status_map_without_spurious_notifications() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("a", None, 1), session("b", None, 1)]);
        let mut map = HashMap::new();
        map.insert("a".to_string(), SessionStatus::Busy);
        map.insert("b".to_string(), SessionStatus::Idle);
        st.apply_statuses(map);
        assert_eq!(st.agent_status("a"), AgentUiStatus::Running);
        assert_eq!(st.agent_status("b"), AgentUiStatus::Idle);
    }

    #[test]
    fn snapshot_is_sorted_newest_first_and_carries_derived_status() {
        let mut st = AppState::default();
        st.apply_sessions(vec![session("old", None, 100), session("new", None, 200)]);
        st.apply_statuses(
            [("new".to_string(), SessionStatus::Busy)]
                .into_iter()
                .collect(),
        );
        st.apply_agents(vec![Agent {
            name: "build".into(),
            description: None,
            mode: "all".into(),
            built_in: true,
            color: None,
            model: None,
        }]);
        st.set_root_session("old");

        let snap = st.snapshot();
        let ids: Vec<&str> = snap
            .sessions
            .iter()
            .map(|s| s.session.id.as_str())
            .collect();
        assert_eq!(ids, ["new", "old"]);
        assert_eq!(snap.sessions[0].status, AgentUiStatus::Running);
        assert_eq!(snap.sessions[1].status, AgentUiStatus::Idle);
        assert_eq!(snap.root_session_id.as_deref(), Some("old"));
        assert!(!snap.connected);
        assert_eq!(snap.agents.len(), 1);

        // The snapshot must be JSON-serializable for the Tauri bridge.
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["sessions"][0]["session"]["id"], "new");
        assert_eq!(v["sessions"][0]["status"], "running");
    }

    #[test]
    fn unknown_event_types_are_ignored_safely() {
        let mut st = AppState::default();
        assert!(st.apply_event(&Event::Unknown).is_empty());
        assert!(st.apply_event(&Event::PartUpdated {
            properties: serde_json::from_str(
                r#"{"part":{"id":"p","sessionID":"s","messageID":"m","type":"text","text":"x"}}"#
            )
            .unwrap()
        })
        .is_empty());
    }
}
