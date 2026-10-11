# AgentBackend trait (multi-backend seam)

## Objective

Introduce an `AgentBackend` abstraction in `orquestador-core` so the app can
drive more than one agent backend (OpenCode today; a CLI/other backend later)
without touching the event hub, the state reducer, or the Tauri shell.
Re-implement OpenCode behind the trait with **no behavior change**, and rewire
the hub/shell to `Arc<dyn AgentBackend>`.

## Problem

The core is hard-wired to one concrete type, `OpenCodeClient`, everywhere the
app talks to a backend: `bootstrap`, `EventHub` (its `client` field, the
per-connection loop, `reconcile_once`) and the Tauri shell (`AppBridge.client()`,
`reconnect`). Adding a second backend today means touching all of them. The
seam does not exist.

## Why

Roadmap Track 3 (`docs/ROADMAP.md`), item #2 in the suggested delivery order:
"`AgentBackend` trait + per-backend event adapters, OpenCode first; forces the
client seam clean." This is the prerequisite for a CLI backend and for the
role-enforcing proxy.

## Scope

In scope (core):

- `crates/orquestador-core/src/backend.rs` (new): the `AgentBackend` trait,
  `EventStream` alias, `BackendEvent` struct, `BACKEND_OPENCODE` const.
- `crates/orquestador-core/src/client.rs`: `impl AgentBackend for OpenCodeClient`
  (delegating to the existing inherent methods) plus a new `stream_events` that
  owns SSE framing; **move `decode_payload` here** with its unit tests.
- `crates/orquestador-core/src/events.rs`: `bootstrap`, `EventHub::new`,
  `EventHub.client`, the connection loop, `reconcile_once` move to
  `&dyn AgentBackend` / `Arc<dyn AgentBackend>`. The hub no longer frames SSE
  itself; it consumes `stream_events()`.
- `crates/orquestador-core/src/lib.rs`: re-export `AgentBackend`, `EventStream`,
  `BackendEvent`, `BACKEND_OPENCODE`.
- `Cargo.toml` + `crates/orquestador-core/Cargo.toml`: add `async-trait`.

In scope (shell):

- `src-tauri/src/bridge.rs`: `AppBridge` holds `Arc<StdRwLock<Arc<dyn AgentBackend>>>`;
  `client()` returns `Arc<dyn AgentBackend>`; `new_with_config`/`reconnect`
  build an `OpenCodeClient` behind the trait object.

Out of scope:

- The frontend (it never sees a backend type). No frontend files change.
- Actually implementing a second backend (CLI). This slice only proves the seam.
- Any behavior/payload change for OpenCode.
- `test_connection` in `commands.rs` stays a raw OpenCode HTTP probe.

## Constraints

- Crate is **edition 2021** → `async fn` in traits is not dyn-compatible; use
  `#[async_trait]` (add the dep) rather than hand-boxed futures.
- **No behavior change**: SSE framing/decoding, resync, reconcile, backoff,
  idle watchdog, and all `CoreMessage` output stay byte-for-byte identical for
  OpenCode. All existing tests must stay green (adapt only construction sites).
- The event stream item is `BackendEvent { event: Event, raw: Value }` — the raw
  JSON is still forwarded as `CoreMessage::RawEvent` for the UI message stream.
  (Document that a non-OpenCode backend must synthesize a compatible `raw`
  payload; evolving that is a later task.)
- Trait object behind `Arc`: `AgentBackend: Send + Sync`.

## Trait shape (reference)

```rust
#[async_trait]
pub trait AgentBackend: Send + Sync {
    fn id(&self) -> &'static str;

    // health / discovery
    async fn health(&self) -> Result<Health, CoreError>;
    async fn agents(&self) -> Result<Vec<Agent>, CoreError>;

    // sessions
    async fn list_sessions(&self) -> Result<Vec<Session>, CoreError>;
    async fn create_session(&self, body: &CreateSessionBody) -> Result<Session, CoreError>;
    async fn session_statuses(&self) -> Result<HashMap<String, SessionStatus>, CoreError>;
    async fn session(&self, id: &str) -> Result<Session, CoreError>;
    async fn session_children(&self, id: &str) -> Result<Vec<Session>, CoreError>;
    async fn delete_session(&self, id: &str) -> Result<bool, CoreError>;
    async fn rename_session(&self, id: &str, title: &str) -> Result<Session, CoreError>;
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
    async fn reply_permission(&self, request_id: &str, response: PermissionResponse, message: Option<&str>) -> Result<bool, CoreError>;
    async fn respond_permission(&self, id: &str, permission_id: &str, response: PermissionResponse, remember: Option<bool>) -> Result<bool, CoreError>;

    // files / diffs
    async fn diff(&self, id: &str, message_id: Option<&str>) -> Result<Vec<FileDiff>, CoreError>;
    async fn file_tree(&self, path: Option<&str>) -> Result<Vec<FileNode>, CoreError>;
    async fn file_content(&self, path: &str) -> Result<FileContent, CoreError>;
    async fn file_status(&self) -> Result<Vec<FileStatus>, CoreError>;
    async fn find_files(&self, query: &str, limit: Option<u32>) -> Result<Vec<String>, CoreError>;
    async fn find_symbols(&self, query: &str) -> Result<Vec<serde_json::Value>, CoreError>;
    async fn find_by_pattern(&self, pattern: &str) -> Result<Vec<String>, CoreError>;

    // events
    async fn stream_events(&self) -> Result<EventStream, CoreError>;
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<BackendEvent, CoreError>> + Send>>;

pub struct BackendEvent { pub event: Event, pub raw: Value }
```

Adjust exact file/query method names to whatever `client.rs` already exposes —
the trait must mirror the existing inherent surface 1:1 so delegation is trivial.

## Tasks

1. [ ] Add `async-trait` to workspace deps and to `crates/orquestador-core`.
2. [ ] `backend.rs`: trait + `EventStream` + `BackendEvent` + `BACKEND_OPENCODE`.
3. [ ] `client.rs`: `impl AgentBackend for OpenCodeClient` delegating via
       fully-qualified inherent calls; new `stream_events` that frames SSE with
       `eventsource()` and yields `BackendEvent`; move `decode_payload` (+ its unit
       tests) from `events.rs` to `client.rs`.
4. [ ] `events.rs`: `bootstrap(&dyn AgentBackend, ...)`, `EventHub.client:
       Arc<dyn AgentBackend>`, connection loop consumes `stream_events()`,
       `reconcile_once(&dyn AgentBackend, ...)`. Remove now-unused `eventsource`
       import locally.
5. [ ] `bridge.rs`: hold/return `Arc<dyn AgentBackend>`; wrap `OpenCodeClient`.
6. [ ] `lib.rs`: re-exports.
7. [ ] Add a `FakeBackend` (test-only) and one test driving `bootstrap` and/or
       the hub against it, proving the seam is backend-agnostic.
8. [ ] Run the full verification chain.

## Acceptance criteria

- `cargo test --workspace` — all green, including a new backend-agnostic test.
- `cargo check --manifest-path src-tauri/Cargo.toml` — clean.
- `cd frontend && npx tsc --noEmit` — clean (frontend untouched).
- No `OpenCodeClient` remains in the `bootstrap`/`EventHub`/`reconcile` signatures
  or in `AppBridge`'s stored/returned type.

## Verification

- `cargo test --workspace`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cd frontend && npx tsc --noEmit`

## Notes / risks

- `#[async_trait]` boxes every call — acceptable for a control-tower app; no hot loop.
- Module cycle `client.rs -> backend.rs` and `events.rs -> backend.rs` is fine
  (trait-only dependency, no item cycle).
- `test_connection` and any raw OpenCode HTTP probing stay outside the trait.

## Progress

- 2026-10-10: feature doc created; implementation delegated.
