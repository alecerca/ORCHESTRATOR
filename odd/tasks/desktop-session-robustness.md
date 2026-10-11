# Desktop session robustness — feature document

- **Feature**: `desktop-session-robustness`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, branch `feat/session-warnings`, then `feat/project-switching`)
- **Created**: 2026-10-11
- **Delivery strategy**: `ask-on-risk` (default). One work-unit commit per slice, `--no-ff` merge to `main`.
- **Delivery forecast**: moderate-to-large across three slices; each slice is individually well under the ~400-line planning heuristic.
- **Depends on**: `AgentBackend`/`AppBridge` (done), `EventHub` reconnect (exists), `server.json` config (done).

## Objective

Make Orquestador behave like a **desktop app**, not a thin browser tab that
silently stops working. Three user-reported problems, in the user's own words:

1. **Dead server, silent app.** "Si yo cierro `opencode serve` y no cierro el
   orquestador … debería al menos salir un aviso" — currently the app keeps
   "running" and writes go nowhere with no warning.
2. **Usage limit, silent app.** "Si alcanzo mi límite de uso … se queda como
   running sin avisarme que alcanzé el límite y esperar a que se desbloquee" —
   a throttled/quota-blocked run stays visually "running" forever.
3. **Anchored to one directory.** "Está anclado a un directorio … que yo ejecute
   `opencode serve` de manera que se levante y pueda yo en el orquestador elegir
   qué proyecto tocar." One server, choose the project from the app.

## Verified facts (2026-10-11, live)

- A single `opencode serve` (pid 9257, `127.0.0.1:4096`) serves **multiple
  projects**: `GET /project` returned `global (/ )`, `LookingForMates_App`, and
  `orquestador`.
- **Per-request project scoping works on one server**: both
  `GET /project/current?directory=<abs>` and the header
  `x-opencode-directory: <abs>` returned the requested project; `GET /project/current`
  with no param returns the server's cwd project; `GET /session?directory=<abs>`
  returned that project's sessions. This is the basis for W3 — **no need to
  spawn/kill servers per project**.
- `ClientConfig { base_url, username, password, request_timeout }`
  (`crates/orquestador-core/src/client.rs:33`); basic auth applied per request
  (`client.rs:153-159`). **No directory/header concept exists today.**
- **Connection health already flows**: `AppStateSnapshot.connected` → top-bar
  dot; reconnect/backoff/30s idle watchdog live in `EventHub::run`
  (`crates/orquestador-core/src/events.rs:241-378`). Only *surfacing* is missing.
- **No limit handling**: any non-2xx becomes `CoreError::Api{status,message}`
  (`crates/orquestador-core/src/error.rs:55`); nothing distinguishes 429/402 or a
  "free tier / usage limit" message.
- **Closed enums**: `Effect` (`crates/orquestador-core/src/state.rs:38`) and
  `CoreMessage` (`events.rs:25`) each have 3 variants; adding a signal touches
  `state.rs`, `events.rs`, `frontend/src/components/NotificationsDrawer.tsx`,
  `src-tauri/src/notify.rs`, `frontend/src/types.ts` (`:99-113`).
- Verification chain: `cargo test --workspace` · `cargo check --manifest-path
  src-tauri/Cargo.toml` · `cd frontend && npx tsc --noEmit` · `npx vite build`.
  **No frontend test runner** → UI verify = tsc + build + manual (documented TDD
  exception). Core Rust logic **does** have a test runner → TDD applies.

## Slice A — Session warnings (W1 + W2) — `feat/session-warnings`

### Problem

The app shows no prominent warning when the server dies, and no warning when a
run is throttled/blocked by a usage limit; both leave the UI looking "running".

### Scope (in)

- A **connection banner**: when `connected === false`, show a clear, dismissible-
  free banner ("Sin conexión con `opencode serve` — reconectando…") instead of
  only the top-bar dot. Keep the composer honest about the state.
- A **limit signal**: classify limit conditions (HTTP 429/402, or message text
  matching limit/quota/rate/tier/usage) in the core error/event path and surface
  a distinct warning ("Límite de uso alcanzado — esperando para reintentar").
- Ensure a run blocked by a limit/send failure does **not** stay visually
  "running" — the pending/running state must clear or switch to an error/blocked
  state with the warning.
- Send-failure surfacing: when a send fails (any error), the user sees it.

### Scope (out)

- Auto-retry with backoff UI, countdown timers, or polling for unblock.
- Changing which provider/model is used (that is the future model-picker seam).
- Any `/project` or directory work (Slice B).

### Design decisions

- **Reuse the existing `connected` signal** for W1; do not invent a second
  liveness notion. If the worker finds `connected` is not actually toggled on
  stream loss, wire it in `EventHub` (that is in scope for W1).
- **One new `Effect` variant** for the limit warning (names chosen by the worker
  to fit the existing enum style); the connection banner is driven by the existing
  `connected` snapshot, not a new effect.
- **Classification is pure and unit-tested**: a `is_usage_limit(status, message)`
  helper in core with table tests (429 true, 402 true, 500 false, "free tier" /
  "usage limit" / "rate limit" true, unrelated false). TDD: RED first.
- Limit detection must be **defensive**: if the exact wire shape of a limit is
  unknown, classify conservatively (429/402 + keyword match) and never throw.

### Tasks

- [x] **A1 — Core: limit classification (TDD).** `is_usage_limit(status, message)`,
  `is_usage_limit_message(message)` and `CoreError::is_usage_limit()` in
  `crates/orquestador-core/src/error.rs` (case-insensitive keyword table;
  conservative 402/429). TDD RED → GREEN; table + `CoreError` tests added.
  Wired into `state.rs`: a limit `session.error` and a limit `retry` status emit a
  new `Effect::UsageLimit { session_id, title, message }` and clear the running
  state (`active`/`finished` removed, `errored` set → UI status `Error`), with
  dedup via `errored` so a burst does not spam. `apply_statuses_at` (
  bootstrap/reconcile) classifies limit retries too, so a snapshot never shows a
  blocked run as "running". Tests: limit error effect + clear, limit retry not
  running + dedup, non-limit retry stays Running, limit retry in a status map.
- [x] **A2 — Core: connection liveness.** Confirmed already correct:
  `EventHub::run` calls `mark_disconnected()` → `set_connection(false, None)` and
  publishes a snapshot when `connect_once` returns (covered by existing test
  `hub_survives_a_server_that_dies_mid_stream`). No `events.rs` change needed.
- [x] **A3 — Shell + frontend surfacing.** Added `Effect::UsageLimit` in
  `state.rs`; `src-tauri/src/notify.rs` `effect_kind`/`handle_effect` arms;
  `AppBridge::push_effect` (`bridge.rs`) so a send-time limit error travels the
  effect path; `commands.rs` classifies `send_message`/`send_message_async`
  errors and emits `Effect::UsageLimit` for limits. Frontend:
  `types.ts` `Effect` union, `NotificationsDrawer` `usage_limit` case,
  `.notification--usage_limit` style; new `components/ConnectionBanner.tsx`
  (gated on `storeReady` to avoid a startup flash) rendered in `App.tsx` with a
  `.connection-banner` style. Send failures were already surfaced by
  `Composer`/`FollowUpInput`; limit failures now additionally raise the distinct
  warning. **Deviation from doc:** banner copy is English, matching the existing
  English UI (`NotificationsDrawer`, `TopBar`), not the Spanish quoted from the
  user; the Spanish in this doc is the user's words, not the app's locale.
- [x] **A4 — Verify.** All four commands green (see Progress). Manual
  server-kill/limit checks remain for the reviewer.

## Slice B — Project switching (W3) — `feat/project-switching`

### Problem

The app is anchored to the server's cwd project. The user wants to run one
`opencode serve` and choose which project to work on from Orquestador.

### Scope (in)

- `ClientConfig.directory: Option<String>`; when set, every HTTP request **and**
  the SSE stream carry `x-opencode-directory: <abs>`. (Header form chosen so it
  applies uniformly, including the event stream, without per-endpoint query edits.)
- `AppBridge`: a method to set the active directory (reusing the existing
  reconnect/rebuild path), so switching is a deliberate, atomic re-scope.
- Tauri commands: `list_projects()` → `GET /project`; `current_project()` →
  `GET /project/current`; `set_project_directory(dir)` → persist + re-scope.
- Frontend: a **project picker** in the TopBar showing the current project
  (basename of `worktree`) with a dropdown of known projects from `GET /project`
  plus an "open folder…" action (native folder dialog). Persist the choice.
- Re-scope sessions/files readers to the selected directory.

### Scope (out)

- Spawning/managing the `opencode serve` process from the app.
- Multi-server / remote-project targets (that is the role proxy's territory).
- Changing session ownership/auth semantics.

### Design decisions

- **Header injection, one place**: add the header in the same request builder
  that adds basic auth (`client.rs:153-159`) and in the SSE stream request, so no
  endpoint is missed. One `directory` field drives all of it.
- **Switching = reconnect**: setting a directory rebuilds the client through the
  existing reconnect path (same mechanism already used for `server.json` changes)
  rather than mutating the live client — simpler and atomic.
- **Persistence**: store the selected directory alongside the existing server
  config (`server.json`) so it survives restarts; fall back to the server's
  current project when unset.
- **SSE risk**: verify the event stream is scoped by the directory header; if it
  is not, document the limitation and re-subscribe on switch. Do not assume.

### Tasks

- [ ] **B1 — Core: directory plumbing (TDD).** `ClientConfig.directory`; header
  in the request builder + SSE request; a unit test asserting the header is
  present/absent per config.
- [ ] **B2 — Shell: project commands.** `list_projects`, `current_project`,
  `set_project_directory`; wire through `AppBridge` with the reconnect path;
  persist to `server.json`.
- [ ] **B3 — Frontend: picker.** TopBar project control + dropdown + folder
  dialog + persistence + re-scope on change.
- [ ] **B4 — Verify.** Same chain as A4; manual: switch to `LookingForMates_App`
  and confirm sessions/files scope to it.

## Acceptance criteria

- **Slice A**: stopping `opencode serve` while the app is open raises a visible
  banner within a few seconds; restarting it clears the banner. A throttled/limit
  run raises a distinct warning and does not remain "running". Any send failure is
  visible.
- **Slice B**: from a single `opencode serve`, the user can switch the active
  project to any directory returned by `GET /project` (or chosen via the folder
  dialog); sessions and files reflect the selected project; the choice persists
  across restarts; switching does not require restarting the server.
- All verification commands pass; no regressions in the 77 existing core tests.

## Verification

- `cargo test --workspace`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cd frontend && npx tsc --noEmit`
- `cd frontend && npx vite build`
- Manual: server-kill banner (A), project switch (B).

## Progress

- 2026-10-11: Feature doc created. Live API validation done (`/project`,
  `?directory=`, `x-opencode-directory`). Slice A branched as
  `feat/session-warnings`; implementation delegated to a bounded writer.
- 2026-10-11: Slice A implemented (A1–A4). Core: `is_usage_limit` +
  `Effect::UsageLimit` + run-state clearing; shell: notify + `push_effect` +
  send-error classification; frontend: connection banner + limit notification.
  Verification: `cargo test --workspace` 118 passed / 0 failed (orquestador-core
  83, orquestador-proxy 30 + 5 integration); `cargo check --manifest-path
  src-tauri/Cargo.toml` clean; `npx tsc --noEmit` clean; `npx vite build` OK
  (pre-existing >500 kB chunk warning only). Commit id: _pending — terminal git
  action owned by the parent._
