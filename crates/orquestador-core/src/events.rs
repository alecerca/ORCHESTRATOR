//! SSE subscription against `GET /event`, reconnection with exponential
//! backoff, and the glue that turns events into [`AppState`] updates.
//!
//! This is the only module that owns a long-lived connection. Everything it
//! does is published on a [`broadcast::Sender<CoreMessage>`] so the (optional)
//! Tauri shell can bridge it to the webview without this crate knowing
//! anything about Tauri.

use crate::client::{OpenCodeClient, DEFAULT_ROOT_TITLE};
use crate::error::CoreError;
use crate::state::{AppState, AppStateSnapshot, Effect};
use crate::types::{Agent, CreateSessionBody, Event, Session};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch, RwLock};

/// Messages published by the event hub for whoever is listening
/// (the Tauri shell bridges these to `window.emit(...)`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreMessage {
    /// Lossless JSON of a single SSE event — display only (message/parts stream).
    RawEvent { event: Value },
    /// Full derived state after a state-changing event.
    StateSnapshot { snapshot: AppStateSnapshot },
    /// Something the user should be told about (desktop notification).
    Effect { effect: Effect },
}

/// Broadcast channel capacity. Lagging receivers just miss a snapshot and can
/// always re-read state via the `get_state` command.
pub const EVENT_BUFFER: usize = 512;

// ---------------------------------------------------------------------------
// Pure helpers (unit tested)
// ---------------------------------------------------------------------------

/// Exponential backoff with a hard 30s cap plus up to 250ms of jitter.
///
/// `attempt` 0 → ~0.5s, 1 → 1s, 2 → 2s, 3 → 4s, 4 → 8s, 5 → 16s, 6+ → 30s.
pub fn delay_for(attempt: u32, jitter_ms: u64) -> Duration {
    let shift = attempt.min(6);
    let base_ms = (500u64 << shift).min(30_000);
    Duration::from_millis(base_ms + (jitter_ms % 250))
}

fn jitter_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_millis()))
        .unwrap_or(0)
}

/// Does this event change [`AppState`]? (display-only events do not)
pub fn changes_state(event: &Event) -> bool {
    !matches!(
        event,
        Event::MessageUpdated { .. }
            | Event::MessageRemoved { .. }
            | Event::PartUpdated { .. }
            | Event::PartRemoved { .. }
            | Event::SessionDiff { .. }
            | Event::FileEdited { .. }
            | Event::FileWatcher { .. }
            | Event::ServerConnected { .. }
            | Event::Unknown
    )
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

/// Pick the session the composer drives: the most recently updated parentless
/// session. `None` means "there is nothing to reuse — create one".
pub fn pick_root(sessions: &[Session]) -> Option<String> {
    sessions
        .iter()
        .filter(|s| s.parent_id.is_none())
        .max_by_key(|s| (s.time.updated, s.time.created))
        .map(|s| s.id.clone())
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

/// Result of the initial synchronous load (no SSE involved yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    Connected {
        root_session_id: Option<String>,
        version: String,
    },
    Disconnected {
        error: String,
    },
}

/// Load health + sessions + statuses + agents and choose the root session.
///
/// Never panics and never propagates transport errors: a stopped
/// `opencode serve` must leave the app usable (it shows "disconnected" and the
/// event hub keeps retrying).
pub async fn bootstrap(client: &OpenCodeClient, state: &Arc<RwLock<AppState>>) -> BootstrapOutcome {
    let health = match client.health().await {
        Ok(h) => h,
        Err(e) => {
            state.write().await.set_connection(false, None);
            return BootstrapOutcome::Disconnected {
                error: e.to_string(),
            };
        }
    };

    let sessions = client.list_sessions().await.unwrap_or_default();
    let statuses = client.session_statuses().await.ok();
    let agents = client.agents().await.ok();

    // Reuse the newest root session; create one only when there is nothing.
    let root = match pick_root(&sessions) {
        Some(id) => Some(id),
        None => match client
            .create_session(&CreateSessionBody {
                parent_id: None,
                title: Some(DEFAULT_ROOT_TITLE.to_string()),
            })
            .await
        {
            Ok(s) => {
                state.write().await.apply_sessions(vec![s.clone()]);
                Some(s.id)
            }
            Err(e) => {
                state
                    .write()
                    .await
                    .set_connection(false, Some(health.version));
                return BootstrapOutcome::Disconnected {
                    error: e.to_string(),
                };
            }
        },
    };

    let mut st = state.write().await;
    st.apply_sessions(sessions);
    if let Some(statuses) = statuses {
        st.apply_statuses(statuses);
    }
    if let Some(agents) = agents {
        st.apply_agents(agents);
    }
    if let Some(root) = &root {
        st.set_root_session(root.clone());
    }
    st.set_connection(true, Some(health.version.clone()));
    drop(st);

    BootstrapOutcome::Connected {
        root_session_id: root,
        version: health.version,
    }
}

// ---------------------------------------------------------------------------
// The hub
// ---------------------------------------------------------------------------

/// Owns the SSE connection and keeps `AppState` up to date forever.
pub struct EventHub {
    client: Arc<OpenCodeClient>,
    state: Arc<RwLock<AppState>>,
    tx: broadcast::Sender<CoreMessage>,
    shutdown: watch::Receiver<bool>,
}

impl EventHub {
    pub fn new(
        client: Arc<OpenCodeClient>,
        state: Arc<RwLock<AppState>>,
        tx: broadcast::Sender<CoreMessage>,
        shutdown: watch::Receiver<bool>,
    ) -> Self {
        Self {
            client,
            state,
            tx,
            shutdown,
        }
    }

    /// Run until the shutdown flag flips. Never returns an error: failures are
    /// turned into backoff + reconnect, which is exactly what a control tower
    /// wants when `opencode serve` restarts underneath it.
    pub async fn run(mut self) {
        let mut attempt: u32 = 0;
        loop {
            if *self.shutdown.borrow() {
                break;
            }

            let saw_handshake = match self.connect_once().await {
                Ok(handshake) => handshake,
                Err(e) => {
                    eprintln!("[orquestador] event stream error: {e}");
                    false
                }
            };

            self.mark_disconnected().await;
            if *self.shutdown.borrow() {
                break;
            }

            // A clean handshake proves the server is alive → restart backoff.
            if saw_handshake {
                attempt = 0;
            }
            let delay = delay_for(attempt, jitter_ms());
            attempt = attempt.saturating_add(1);

            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = self.shutdown.changed() => break,
            }
        }
    }

    /// One connection: health → connect → resync → consume until EOF/error.
    /// Returns `true` when the stream delivered `server.connected`.
    async fn connect_once(&mut self) -> Result<bool, CoreError> {
        let version = self.client.health().await.ok().map(|h| h.version);
        let response = self.client.event_stream().await?;

        self.state.write().await.set_connection(true, version);
        self.publish_snapshot().await;
        self.resync().await;

        let mut stream = response.bytes_stream().eventsource();
        let mut saw_handshake = false;

        loop {
            let next = tokio::select! {
                item = stream.next() => item,
                _ = self.shutdown.changed() => break,
            };
            let Some(frame) = next else { break }; // stream ended (server gone)
            let frame = frame.map_err(|e| CoreError::Stream(e.to_string()))?;

            let event_name = if frame.event.is_empty() {
                None
            } else {
                Some(frame.event.as_str())
            };
            let Some((typed, raw)) = decode_payload(&frame.data, event_name) else {
                continue;
            };

            if matches!(typed, Event::ServerConnected { .. }) {
                saw_handshake = true;
            }

            let effects = {
                let mut state = self.state.write().await;
                state.apply_event(&typed)
            };

            // Raw first (so the UI sees the message before the derived state),
            // then the snapshot, then the effects.
            let _ = self.tx.send(CoreMessage::RawEvent { event: raw });
            if changes_state(&typed) {
                self.publish_snapshot().await;
            }
            for effect in effects {
                let _ = self.tx.send(CoreMessage::Effect { effect });
            }
        }

        Ok(saw_handshake)
    }

    /// Full re-read after (re)connecting: SSE does not replay missed events,
    /// so the only correct recovery is a resync.
    async fn resync(&self) {
        if let Ok(sessions) = self.client.list_sessions().await {
            self.state.write().await.apply_sessions(sessions);
        }
        if let Ok(statuses) = self.client.session_statuses().await {
            self.state.write().await.apply_statuses(statuses);
        }
        if let Ok(agents) = self.client.agents().await {
            let mut agents: Vec<Agent> = agents;
            agents.sort_by(|a, b| a.name.cmp(&b.name));
            self.state.write().await.apply_agents(agents);
        }
        self.publish_snapshot().await;
    }

    async fn mark_disconnected(&self) {
        self.state.write().await.set_connection(false, None);
        self.publish_snapshot().await;
    }

    async fn publish_snapshot(&self) {
        let snapshot = self.state.read().await.snapshot();
        let _ = self.tx.send(CoreMessage::StateSnapshot { snapshot });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionTime;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn backoff_grows_exponentially_and_caps_at_30s() {
        assert_eq!(delay_for(0, 0), Duration::from_millis(500));
        assert_eq!(delay_for(1, 0), Duration::from_millis(1_000));
        assert_eq!(delay_for(3, 0), Duration::from_millis(4_000));
        assert_eq!(delay_for(6, 0), Duration::from_millis(30_000));
        assert_eq!(delay_for(50, 0), Duration::from_millis(30_000));
        // jitter is bounded to <250ms
        assert_eq!(delay_for(0, 249), Duration::from_millis(749));
        assert_eq!(delay_for(0, 9_999), Duration::from_millis(749));
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

    #[test]
    fn only_state_changing_events_mark_the_snapshot_dirty() {
        assert!(!changes_state(&Event::Unknown));
        assert!(!changes_state(
            &decode_payload(r#"{"type":"message.part.updated","properties":{"part":{"id":"p","sessionID":"s","messageID":"m","type":"text","text":"x"}}}"#, None).unwrap().0
        ));
        assert!(changes_state(
            &decode_payload(r#"{"type":"session.status","properties":{"sessionID":"s","status":{"type":"busy"}}}"#, None).unwrap().0
        ));
        assert!(changes_state(
            &decode_payload(r#"{"type":"permission.updated","properties":{"id":"p","type":"edit","sessionID":"s","messageID":"m","title":"t","metadata":{},"time":{"created":1}}}"#, None).unwrap().0
        ));
    }

    #[test]
    fn pick_root_prefers_the_newest_parentless_session() {
        let mk = |id: &str, parent: Option<&str>, updated: i64| Session {
            id: id.into(),
            project_id: "p".into(),
            directory: "/d".into(),
            parent_id: parent.map(str::to_owned),
            title: id.into(),
            version: "1".into(),
            time: SessionTime {
                created: updated,
                updated,
                compacting: None,
            },
            summary: None,
        };
        let sessions = vec![
            mk("old_root", None, 10),
            mk("child", Some("old_root"), 99),
            mk("new_root", None, 20),
        ];
        assert_eq!(pick_root(&sessions).as_deref(), Some("new_root"));
        assert_eq!(pick_root(&[mk("child", Some("x"), 1)]), None);
        assert_eq!(pick_root(&[]), None);
    }

    #[test]
    fn core_message_serializes_with_a_kind_tag() {
        // The Tauri shell and the webview depend on this envelope shape.
        let snap = CoreMessage::StateSnapshot {
            snapshot: AppState::default().snapshot(),
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert_eq!(v["kind"], "state_snapshot");
        assert_eq!(v["snapshot"]["connected"], serde_json::json!(false));

        let raw = CoreMessage::RawEvent {
            event: serde_json::json!({"type": "session.idle", "properties": {}}),
        };
        assert_eq!(serde_json::to_value(&raw).unwrap()["kind"], "raw_event");

        let effect = CoreMessage::Effect {
            effect: Effect::Finished {
                session_id: "s".into(),
                title: "t".into(),
            },
        };
        let v = serde_json::to_value(&effect).unwrap();
        assert_eq!(v["kind"], "effect");
        assert_eq!(v["effect"]["effect"], "finished");
    }

    // -- end-to-end (scripted local server, no external network) -----------

    /// Serves canned responses for the handful of endpoints the hub touches,
    /// then stops after `max_connections`.
    async fn spawn_scripted_server(
        max_connections: usize,
        sse_body: String,
    ) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits_clone = hits.clone();

        tokio::spawn(async move {
            let mut served = 0usize;
            while served < max_connections {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                served += 1;
                let mut buf = vec![0u8; 8192];
                let mut read = 0usize;
                loop {
                    let n = socket.read(&mut buf[read..]).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    read += n;
                    if buf[..read].windows(4).any(|w| w == b"\r\n\r\n") || read == buf.len() {
                        break;
                    }
                }
                let head = String::from_utf8_lossy(&buf[..read]).to_string();
                let method = head.split_whitespace().next().unwrap_or("GET").to_string();
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .to_string();

                let (status, content_type, body) = match path.as_str() {
                    "/global/health" => (
                        "200 OK",
                        "application/json",
                        r#"{"healthy":true,"version":"9.9.9"}"#.to_string(),
                    ),
                    "/session" if method == "POST" => (
                        "200 OK",
                        "application/json",
                        concat!(
                            r#"{"id":"sess_new","projectID":"p","directory":"/d","#,
                            r#""title":"Orchestrator","version":"1","#,
                            r#""time":{"created":1,"updated":1}}"#
                        )
                        .to_string(),
                    ),
                    "/session" => ("200 OK", "application/json", "[]".to_string()),
                    "/session/status" => ("200 OK", "application/json", "{}".to_string()),
                    "/agent" => ("200 OK", "application/json", "[]".to_string()),
                    "/event" => ("200 OK", "text/event-stream", sse_body.clone()),
                    other => (
                        "404 Not Found",
                        "application/json",
                        format!(r#"{{"message":"no route for {other}"}}"#),
                    ),
                };

                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
                hits_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        (base, hits)
    }

    fn test_state() -> Arc<RwLock<AppState>> {
        Arc::new(RwLock::new(AppState::default()))
    }

    #[tokio::test]
    async fn bootstrap_connects_reuses_nothing_and_reports_disconnection() {
        let (base, _hits) = spawn_scripted_server(8, String::new()).await;
        let client = OpenCodeClient::new(crate::client::ClientConfig {
            base_url: base,
            ..Default::default()
        })
        .unwrap();
        let state = test_state();

        let outcome = bootstrap(&client, &state).await;
        match outcome {
            BootstrapOutcome::Connected { version, .. } => assert_eq!(version, "9.9.9"),
            other => panic!("expected Connected, got {other:?}"),
        }
        let snap = state.read().await.snapshot();
        assert!(snap.connected);
        assert_eq!(snap.server_version.as_deref(), Some("9.9.9"));
        assert_eq!(
            snap.root_session_id.as_deref(),
            Some("sess_new"),
            "with no sessions bootstrap must create the root session"
        );
    }

    #[tokio::test]
    async fn bootstrap_with_a_dead_server_never_panics() {
        // Port 1 on localhost refuses immediately.
        let client = OpenCodeClient::new(crate::client::ClientConfig {
            base_url: "http://127.0.0.1:1".into(),
            request_timeout: Duration::from_millis(500),
            ..Default::default()
        })
        .unwrap();
        let state = test_state();
        let outcome = bootstrap(&client, &state).await;
        assert!(
            matches!(outcome, BootstrapOutcome::Disconnected { .. }),
            "got {outcome:?}"
        );
        assert!(!state.read().await.snapshot().connected);
    }

    #[tokio::test]
    async fn hub_applies_events_and_publishes_snapshots() {
        let sse = concat!(
            "data: {\"type\":\"server.connected\",\"properties\":{}}\n\n",
            "data: {\"type\":\"session.created\",\"properties\":{\"info\":",
            "{\"id\":\"child_1\",\"projectID\":\"p\",\"directory\":\"/d\",",
            "\"parentID\":\"root_1\",\"title\":\"do the thing\",\"version\":\"1\",",
            "\"time\":{\"created\":1000,\"updated\":1000}}}}\n\n",
            "data: {\"type\":\"session.status\",\"properties\":",
            "{\"sessionID\":\"child_1\",\"status\":{\"type\":\"busy\"}}}\n\n",
            "data: {\"type\":\"permission.updated\",\"properties\":",
            "{\"id\":\"perm_9\",\"type\":\"edit\",\"sessionID\":\"child_1\",",
            "\"messageID\":\"m1\",\"title\":\"Edit src/lib.rs\",\"metadata\":{},",
            "\"time\":{\"created\":1001}}}\n\n",
        )
        .to_string();

        let (base, _hits) = spawn_scripted_server(64, sse).await;
        let client = Arc::new(
            OpenCodeClient::new(crate::client::ClientConfig {
                base_url: base,
                ..Default::default()
            })
            .unwrap(),
        );
        let state = test_state();
        let (tx, mut rx) = broadcast::channel::<CoreMessage>(super::EVENT_BUFFER);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let hub = EventHub::new(client, state.clone(), tx, shutdown_rx);
        tokio::spawn(hub.run());

        // Wait for a snapshot that contains the permission.
        let mut saw_raw = false;
        let snapshot = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match rx.recv().await {
                    Ok(CoreMessage::StateSnapshot { snapshot })
                        if !snapshot.permissions.is_empty() =>
                    {
                        break snapshot
                    }
                    Ok(CoreMessage::RawEvent { .. }) => saw_raw = true,
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(e) => panic!("broadcast closed: {e}"),
                }
            }
        })
        .await
        .expect("timed out waiting for the permission snapshot");

        assert_eq!(snapshot.permissions.len(), 1);
        assert_eq!(snapshot.permissions[0].id, "perm_9");
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session.id, "child_1");
        assert_eq!(
            snapshot.sessions[0].status,
            crate::state::AgentUiStatus::WaitingApproval
        );
        assert!(
            saw_raw,
            "raw events must be forwarded for the message stream"
        );

        // The session statuses endpoint was consulted during resync.
        let st = state.read().await;
        assert_eq!(
            st.sessions().count(),
            1,
            "resync + session.created should agree"
        );
        assert_eq!(
            st.agent_status("child_1"),
            crate::state::AgentUiStatus::WaitingApproval
        );
        drop(st);

        let _ = shutdown_tx.send(true);
    }

    #[tokio::test]
    async fn hub_survives_a_server_that_dies_mid_stream() {
        // One connection that immediately EOFs after `server.connected`.
        let sse = "data: {\"type\":\"server.connected\",\"properties\":{}}\n\n".to_string();
        let (base, _hits) = spawn_scripted_server(2, sse).await;
        let client = Arc::new(
            OpenCodeClient::new(crate::client::ClientConfig {
                base_url: base,
                ..Default::default()
            })
            .unwrap(),
        );
        let state = test_state();
        let (tx, mut rx) = broadcast::channel::<CoreMessage>(super::EVENT_BUFFER);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = tokio::spawn(EventHub::new(client, state.clone(), tx, shutdown_rx).run());

        // The hub must report "connected" at least once (the message is
        // buffered by the broadcast channel, so the window cannot be missed).
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match rx.recv().await {
                    Ok(CoreMessage::StateSnapshot { snapshot }) if snapshot.connected => break,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(e) => panic!("broadcast closed: {e}"),
                }
            }
        })
        .await
        .expect("hub never reported connected");

        // Shutdown must terminate the run loop even while it is backing off.
        let _ = shutdown_tx.send(true);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("hub task did not stop after shutdown")
            .expect("hub task panicked");
    }
}
