//! Thin glue between the headless [`orquestador_core`] and Tauri.
//!
//! It owns the three long-lived objects — the HTTP client, the shared
//! [`AppState`] and the broadcast channel of [`CoreMessage`]s — and hands them
//! out to the `#[tauri::command]`s in [`crate::commands`]. All real logic lives
//! in the core crate; nothing in here is unit-tested because compiling the
//! Tauri shell requires `webkit2gtk-4.1` (see `docs/PLAN.md`).

use std::sync::Arc;

use orquestador_core::client::ClientConfig;
use orquestador_core::events::{bootstrap, BootstrapOutcome, CoreMessage, EVENT_BUFFER};
use orquestador_core::state::AppStateSnapshot;
use orquestador_core::{AppState, EventHub, OpenCodeClient};
use tokio::sync::{broadcast, watch, RwLock};

/// Shared application handle stored in Tauri's state and cloned into every
/// command invocation.
pub struct AppBridge {
    client: Arc<OpenCodeClient>,
    state: Arc<RwLock<AppState>>,
    tx: broadcast::Sender<CoreMessage>,
    shutdown: watch::Sender<bool>,
}

impl AppBridge {
    /// Build the bridge from environment configuration
    /// (`OPENCODE_SERVER_BASE_URL`, `OPENCODE_SERVER_PASSWORD`).
    pub fn new() -> Self {
        let client = OpenCodeClient::from_env().unwrap_or_else(|_| {
            OpenCodeClient::new(ClientConfig::default())
                .expect("the default client configuration is always valid")
        });
        let (tx, _rx) = broadcast::channel(EVENT_BUFFER);
        let (shutdown, _rx) = watch::channel(false);
        Self {
            client: Arc::new(client),
            state: Arc::new(RwLock::new(AppState::default())),
            tx,
            shutdown,
        }
    }

    pub fn client(&self) -> Arc<OpenCodeClient> {
        Arc::clone(&self.client)
    }

    pub fn state(&self) -> Arc<RwLock<AppState>> {
        Arc::clone(&self.state)
    }

    /// Receiver for the webview/notification forwarder. Subscribe **before**
    /// [`spawn_hub`](Self::spawn_hub) so no message is missed.
    pub fn subscribe(&self) -> broadcast::Receiver<CoreMessage> {
        self.tx.subscribe()
    }

    /// Ask the SSE hub to stop (used on window close).
    pub fn request_shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Current derived state — safe to call at any time.
    pub async fn snapshot(&self) -> AppStateSnapshot {
        self.state.read().await.snapshot()
    }

    /// Push a fresh derived snapshot to every subscriber.
    ///
    /// The hub normally publishes on state-changing events; a command that
    /// mutates the registry itself (e.g. dropping a permission the server just
    /// answered) uses this so the webview updates without waiting for SSE.
    pub async fn push_snapshot(&self) {
        let snapshot = self.state.read().await.snapshot();
        let _ = self.tx.send(CoreMessage::StateSnapshot { snapshot });
    }

    /// One synchronous load (health + sessions + statuses + agents). Never
    /// panics; a dead server yields [`BootstrapOutcome::Disconnected`].
    pub async fn bootstrap(&self) -> BootstrapOutcome {
        bootstrap(&self.client, &self.state).await
    }

    /// Start the long-lived SSE connection on the Tauri async runtime.
    pub fn spawn_hub(&self) {
        let client = Arc::clone(&self.client);
        let state = Arc::clone(&self.state);
        let tx = self.tx.clone();
        let shutdown = self.shutdown.subscribe();
        tauri::async_runtime::spawn(async move {
            EventHub::new(client, state, tx, shutdown).run().await;
        });
    }
}

impl Default for AppBridge {
    fn default() -> Self {
        Self::new()
    }
}
