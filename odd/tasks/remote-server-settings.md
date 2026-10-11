# Feature: Remote server settings (Track 2, easy half)

Roadmap item: **Settings: `base_url` + `password`** (`docs/ROADMAP.md` → Suggested
delivery order #1). First slice of the post-v1 roadmap.

## Objective

Let the user point Orquestador at any OpenCode server (local or remote) from a
Settings screen instead of relying on `OPENCODE_SERVER_*` environment variables,
and reconnect live without relaunching the app.

## Problem / why

- Today the server address is the hardcoded default `http://127.0.0.1:4096`
  (`crates/orquestador-core/src/client.rs`). Overriding it requires editing the
  environment and restarting.
- "Connect to another machine" (the cowork track) needs a UI + persistence so a
  non-technical user can type a host and password.

## Scope

In scope:

- Persist `{ base_url, username, password }` to `server.json` in the app config
  dir (backend-owned source of truth).
- Read it on startup and build the client from it; fall back to
  environment / defaults when the file is absent.
- Settings dialog (gear in the TopBar): edit, **Test connection** (probe
  `/global/health` on a throwaway client), and **Save** (persist + reconnect).
- Reconnect the live client + restart the SSE hub + clear stale state, without a
  relaunch.

Out of scope (later roadmap slices):

- Role-enforcing proxy / RBAC (Track 2 hard half).
- TLS provisioning (host-side concern; documented in the roadmap).
- `AgentBackend` trait (Track 3).

## Constraints

- Core crate stays headless: no Tauri dependency. Env parsing refactored into
  `ClientConfig::from_env_or_default()` so both core and shell reuse it.
- No secrets committed; the password lives in the app config dir (same as the
  skills store) and is never logged.
- Keep `cargo test --workspace`, `cargo test --manifest-path src-tauri/Cargo.toml`
  and the frontend typecheck/build green.

## Tasks

- [x] **T1 — Core env refactor.** Add `ClientConfig::from_env_or_default()` and
  make `OpenCodeClient::from_env()` delegate to it. (evidence: `cargo test
  --workspace` → 71 passed)
- [x] **T2 — Persistence module.** `src-tauri/src/server_config.rs`: load/save
  `server.json`, normalize base URL, collapse to `ClientConfig`. 4 unit tests.
  (evidence: `cargo test --manifest-path src-tauri/Cargo.toml` → 4 passed)
- [x] **T3 — Bridge reconnect.** `AppBridge`: client behind a sync `RwLock`,
  `new_with_config`, `reconnect` (stop hub, swap client, clear state, restart
  hub, bootstrap). (evidence: `cargo check --manifest-path src-tauri/Cargo.toml`
  clean)
- [x] **T4 — Commands + startup wiring.** `test_connection`,
  `get_server_config`, `set_server_config`; setup loads persisted config.
  (evidence: shell compiles)
- [x] **T5 — Frontend.** `serverConfig` store, `SettingsDialog`, gear button in
  `TopBar`, `commands.ts` bindings. (evidence: `npx tsc --noEmit` clean,
  `npx vite build` succeeds)

## Acceptance criteria

- Opening Settings shows the persisted config (or env/default on first run).
- "Test connection" reports the server version on success and an error otherwise.
- "Save" persists `server.json`, reconnects live, and the connection dot / session
  list reflect the new server without relaunch.

## Verification

- `cargo test --workspace` → 71 passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` → 4 passed.
- `cd frontend && npx tsc --noEmit` → clean.
- `cd frontend && npx vite build` → success (pre-existing Monaco chunk warning).
- Manual: run against a local `opencode serve`, change the URL, Test + Save.

## Delivery

- Branch: `feat/remote-server-settings` (from `main`).
- Work-unit commits: see git log on the branch.
