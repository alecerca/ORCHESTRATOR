//! Thin glue between the headless [`orquestador_core`] and Tauri.
//!
//! It owns the three long-lived objects — the HTTP client, the shared
//! [`AppState`] and the broadcast channel of [`CoreMessage`]s — and hands them
//! out to the `#[tauri::command]`s in [`crate::commands`]. All real logic lives
//! in the core crate; nothing in here is unit-tested because compiling the
//! Tauri shell requires `webkit2gtk-4.1` (see `docs/PLAN.md`).

use std::sync::{Arc, Mutex, RwLock as StdRwLock};

use orquestador_core::client::ClientConfig;
use orquestador_core::events::{bootstrap, BootstrapOutcome, CoreMessage, EVENT_BUFFER};
use orquestador_core::state::AppStateSnapshot;
use orquestador_core::{AgentBackend, AppState, CoreError, EventHub, OpenCodeClient};
use tokio::sync::{broadcast, watch, RwLock};

/// Shared application handle stored in Tauri's state and cloned into every
/// command invocation.
///
/// The client sits behind a synchronous [`StdRwLock`] so commands can obtain a
/// cheap snapshot synchronously (`client()`), while the SSE hub keeps its own
/// owned client. `reconnect` swaps the client and restarts the hub so a change
/// to the server settings takes effect without relaunching the app.
pub struct AppBridge {
    client: Arc<StdRwLock<Arc<dyn AgentBackend>>>,
    state: Arc<RwLock<AppState>>,
    tx: broadcast::Sender<CoreMessage>,
    shutdown: Mutex<watch::Sender<bool>>,
}

impl AppBridge {
    /// Build the bridge from environment configuration
    /// (`OPENCODE_SERVER_BASE_URL`, `OPENCODE_SERVER_PASSWORD`).
    pub fn new() -> Self {
        let config = ClientConfig::from_env_or_default();
        Self::new_with_config(config)
    }

    /// Build the bridge from an explicit configuration (e.g. the persisted
    /// `server.json` written by the Settings dialog).
    pub fn new_with_config(config: ClientConfig) -> Self {
        let client = OpenCodeClient::new(config).unwrap_or_else(|_| {
            OpenCodeClient::new(ClientConfig::default())
                .expect("the default client configuration is always valid")
        });
        // Store behind the trait object so the shell never names a backend.
        let client: Arc<dyn AgentBackend> = Arc::new(client);
        let (tx, _rx) = broadcast::channel(EVENT_BUFFER);
        let (shutdown, _rx) = watch::channel(false);
        Self {
            client: Arc::new(StdRwLock::new(client)),
            state: Arc::new(RwLock::new(AppState::default())),
            tx,
            shutdown: Mutex::new(shutdown),
        }
    }

    /// Cheap synchronous snapshot of the current client. Commands call methods
    /// on the returned [`Arc`]; the inner backend is `Arc`-shared already.
    pub fn client(&self) -> Arc<dyn AgentBackend> {
        Arc::clone(&self.client.read().expect("client lock poisoned"))
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
        let _ = self.shutdown.lock().expect("shutdown lock poisoned").send(true);
    }

    /// Replace the client and (re)start the SSE hub against the new server.
    ///
    /// The previous hub is stopped, stale sessions from the old server are
    /// dropped, and a fresh hub is spawned. Returns the error string if the new
    /// configuration is invalid (nothing is swapped in that case).
    pub async fn reconnect(&self, config: ClientConfig) -> Result<(), CoreError> {
        let client: Arc<dyn AgentBackend> = Arc::new(OpenCodeClient::new(config)?);

        // Stop the current hub by flipping its watch flag.
        self.request_shutdown();

        // Swap in the new client + a brand-new shutdown channel for the next hub.
        *self.client.write().expect("client lock poisoned") = client;
        let (new_shutdown, _rx) = watch::channel(false);
        *self.shutdown.lock().expect("shutdown lock poisoned") = new_shutdown;

        // Drop sessions/statuses that belonged to the previous server.
        *self.state.write().await = AppState::default();

        self.spawn_hub();
        // Populate state immediately instead of waiting for the SSE handshake.
        let _ = self.bootstrap().await;
        Ok(())
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
        let client = self.client();
        bootstrap(&*client, &self.state).await
    }

    /// Start the long-lived SSE connection on the Tauri async runtime.
    pub fn spawn_hub(&self) {
        let client = self.client();
        let state = Arc::clone(&self.state);
        let tx = self.tx.clone();
        let shutdown = self
            .shutdown
            .lock()
            .expect("shutdown lock poisoned")
            .subscribe();
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
