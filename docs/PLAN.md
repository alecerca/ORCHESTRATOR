# Orquestador — Implementation Plan (PLANNER deliverable)

> Master contract for **AGENT 2 (IMPLEMENTER)**. Read `SPEC.md` first; this
> document refines it and resolves the `[PLANNER]` TODOs. Where this file and
> `SPEC.md` disagree, `SPEC.md` wins — report the conflict in
> `docs/IMPLEMENTATION_NOTES.md`.
>
> Companion docs: `docs/ARCHITECTURE.md` (diagrams), `docs/IMPLEMENTATION_PLAN.md`
> (the ordered task list extracted from §9 here). `PLAN.md` is the single source
> of truth; the other two are views over it.

---

## 1. Purpose & product recap

A desktop control tower for OpenCode sub-agents (Tauri 2 + TypeScript). One
window with: an **orchestrator composer**, an **agent panel** (sub-agent list
with status), **split panes** with each agent's live stream, **interrupt /
re-instruct**, **approval prompts** with desktop notifications, and an
**editor + diff viewer** (Monaco). Out of scope for v1: worktrees, issues/PRs,
multi-project switching, chat persistence.

Runtime target: a local `opencode serve` at `http://127.0.0.1:4096` (optional
HTTP basic auth via `OPENCODE_SERVER_PASSWORD`, user `opencode`).

---

## 2. Status snapshot (what this scaffold already contains)

Created by AGENT 1 and **verified green** (commands in §10):

| Area | State | Evidence |
|---|---|---|
| `Cargo.toml` workspace (core only) | done | `cargo metadata` → 1 member |
| `crates/orquestador-core` (types/error/client/state/events) | done + tested | `cargo test -p orquestador-core` → **37 passed** |
| `cargo clippy --all-targets -- -D warnings` | clean | — |
| `src-tauri` shell (bridge/commands/lib/notify/config) | scaffolded, **not compiled** (no webkit2gtk) | `cargo metadata` in `src-tauri` → resolves 487 pkgs |
| `frontend` (Vite+React+TS strict) | scaffold, builds | `npx tsc --noEmit` + `vite build` green |
| `scripts/setup-system-deps.sh` / `install-deps.sh` / `verify-headless.sh` | done | — |
| `docs/PLAN.md`, `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md` | this | — |
| `README.md` | stub (status: planning) | finalized by AGENT 3 |

**What AGENT 2 must build**: the real frontend UI (§8 + §9 steps S3–S11),
finish/verify `src-tauri` once webkit2gtk is installed, and write
`docs/IMPLEMENTATION_NOTES.md`. The core crate is considered complete — only
extend it if a UI need proves a gap (record it).

---

## 3. Repository layout

```
.
├── Cargo.toml                      # workspace: members = ["crates/*"] (src-tauri excluded)
├── Cargo.lock
├── SPEC.md                         # canonical spec (do not edit)
├── README.md                       # stub
├── odd/tasks/orquestador-dashboard.md   # feature doc (do not edit)
├── crates/
│   └── orquestador-core/           # ALL logic, no tauri dep, headless-tested
│       ├── Cargo.toml
│       └── src/{lib,types,error,client,state,events}.rs
├── src-tauri/                      # thin Tauri shell (own [workspace], NOT a root member)
│   ├── Cargo.toml                  # [workspace] empty table => standalone
│   ├── build.rs                    # tauri_build::build()
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/{icon.png,README.md}
│   └── src/{main,lib,bridge,commands,notify}.rs
├── frontend/                       # Vite + React + TS (strict) + Monaco
│   ├── package.json · tsconfig.json · tsconfig.node.json · vite.config.ts · index.html
│   └── src/{main.tsx,App.tsx,store.ts,types.ts,styles.css,vite-env.d.ts}
├── scripts/{setup-system-deps.sh,install-deps.sh,verify-headless.sh}
└── docs/{PLAN.md,ARCHITECTURE.md,IMPLEMENTATION_PLAN.md}
```

---

## 4. Architecture overview

```
┌──────────────────────────── src-tauri (Tauri 2) ───────────────────────────┐
│  lib.rs      Builder: manage(AppBridge), emit("core://message"), bootstrap  │
│  bridge.rs   owns Arc<OpenCodeClient> + Arc<RwLock<AppState>> + broadcast    │
│  commands.rs 21 #[tauri::command]s -> core (Result<T,String>)               │
│  notify.rs   Effect -> notify-rust (always notifies)                          │
└───────────────▲───────────────────────────────────────────────┬────────────┘
        invoke()│ Tauri IPC                              emit() │ "core://message"
┌───────────────┴───────────────────────────────────────────────▼────────────┐
│  frontend (Vite/React/TS) — useSyncExternalStore store, Monaco, panes        │
└─────────────────────────────────────────────────────────────────────────────┘
                                ▲
            orquestador-core (headless, no tauri)     │
  client.rs  ──HTTP──▶  opencode serve (127.0.0.1:4096)
  events.rs  ──SSE───▶  GET /event   →  AppState (state.rs, pure reducer)
                                         │ CoreMessage { raw_event | state_snapshot | effect }
                                         ▼ broadcast::Sender<CoreMessage>
```

**Data flow**

1. `AppBridge::bootstrap()` does a synchronous load (health, sessions, statuses,
   agents) and picks/creates the root session; `spawn_hub()` starts `EventHub`.
2. `EventHub` keeps one SSE connection: on each event it updates
   `AppState` (state-changing events) and broadcasts a `CoreMessage`:
   - `RawEvent{event}` — every SSE event verbatim (message/part streaming; display only),
   - `StateSnapshot{snapshot}` — after state-changing or `session.status` events,
   - `Effect{effect}` — a notification the shell should raise.
3. `src-tauri` forwards every `CoreMessage` to the webview (`core://message`) and
   turns `Effect` into a desktop notification.
4. The webview reads full state with the `get_state` command and listens to
   `core://message`; commands are the only way it mutates server state.

---

## 5. Build isolation (why `src-tauri` compiles alone)

`webkit2gtk-4.1` + `libayatana-appindicator3-dev` are missing and `sudo` needs a
password on the dev machine. Therefore:

- Root `Cargo.toml`: `members = ["crates/*"]` → the workspace contains **only**
  `orquestador-core`. Root `cargo check`/`test` never compiles Tauri.
- `src-tauri/Cargo.toml` has an **empty `[workspace]` table**, which makes it its
  own workspace root and excludes it from the parent. Verified: `cargo metadata`
  inside `src-tauri` lists exactly one member (`orquestador`) and resolves
  `tauri`, `tauri-build`, `notify-rust`, `orquestador-core`.
- The core uses `reqwest` with `default-features = false, features =
  ["json","stream","rustls-tls"]` → **rustls**, no openssl link dependency.
- `notify-rust` is a dependency of `src-tauri` (NOT of core), so the core stays
  pure and headless; notifications use the freedesktop D-Bus service (zbus, pure
  Rust). A missing daemon only logs a warning.

---

## 6. `orquestador-core` contract

### 6.1 Modules

| Module | Responsibility | I/O? |
|---|---|---|
| `types` | Serde shapes of every server object + SSE events. Wire-exact renames. | no |
| `error` | `CoreError` (`thiserror`) + `kind()` + manual `Serialize`. | no |
| `client` | `OpenCodeClient` + `ClientConfig`. **Only** place that builds URLs/HTTP. | yes (reqwest) |
| `state` | `AppState` pure reducer, `AgentUiStatus`, `AppStateSnapshot`, `Effect`. | no |
| `events` | `EventHub` (SSE + backoff + resync), `CoreMessage`, `bootstrap`. | yes (SSE) |

Re-exports (`lib.rs`): `OpenCodeClient`, `CoreError`, `EventHub`, `AppState`,
`DEFAULT_BASE_URL`. Also public: `client::{ClientConfig, DEFAULT_ROOT_TITLE}`,
`events::{CoreMessage, EVENT_BUFFER, BootstrapOutcome, bootstrap}`,
`state::{AppStateSnapshot, SessionSnapshot, AgentUiStatus, Effect}`.

### 6.2 `ClientConfig`

```rust
pub struct ClientConfig {
    pub base_url: String,          // default http://127.0.0.1:4096
    pub username: String,          // default "opencode"
    pub password: Option<String>,  // OPENCODE_SERVER_PASSWORD
    pub request_timeout: Duration, // default 30s
}
```
`OpenCodeClient::from_env()` reads `OPENCODE_SERVER_BASE_URL` and
`OPENCODE_SERVER_PASSWORD`. `OpenCodeClient::new(cfg)` is synchronous and
returns `Result<_, CoreError>`.

### 6.3 Exact API type shapes (wire)

Wire names use **capitalised abbreviations** — naive camelCase is wrong:

| Field | Wire |
|---|---|
| session id | `id` |
| project id | `projectID` |
| parent id | `parentID` |
| message id | `messageID` |
| permission id | `permissionID` (path param; body uses `id`) |
| model | `{ providerID, modelID }` (an **object**, not a string) |
| provider id | `providerID` |
| built in (agent) | `builtIn` |
| agent mode | `type` |

Key structs (see `types.rs` for the full list):

```
Session      { id, projectID, directory, parentID?, title, version,
               time:{created,updated,compacting?}, summary? }
SessionStatus = {type:"idle"} | {type:"busy"}
              | {type:"retry", attempt, message, next}
MessageBundle { info: Message, parts: Part[] }     // history + reply shape
Part (12 variants + Unknown)   ToolState · PermissionPattern
Permission   { id, type, pattern?, sessionID, messageID, callID?, title, metadata, time }
PermissionResponse = "once" | "always" | "reject"   // NOT allow/deny
Todo         { id, content, status, priority }
FileDiff · FileNode · FileContent · FileStatus
Agent        { name, description?, type, builtIn, color?, model? }
PromptBody   { messageID?, model?, agent?, noReply?, system?, tools?, parts:[InputPart] }
InputPart    = { type:"text", text }                // v1 composer sends text only
CreateSessionBody { parentID?, title? }
CommandBody  { command, arguments? }
Event (17 variants + Unknown, tagged on `type`)
```

**Resolved spec gaps** (record, do not "fix" in code):
1. Permission response body is `{ response: "once"|"always"|"reject", remember? }`;
   `SPEC.md` §4 just says `{response, remember?}` — value set clarified here.
   `remember` is omitted when `None`.
2. `POST /session/:id/message` body `model` is an **object** `{providerID,
   modelID}`, not a string.
3. Prefer `POST /session/:id/prompt_async` (returns 204, does not block) for the
   composer; `POST /session/:id/message` waits for the full reply and is exposed
   as `send_message` for callers that want the reply.

### 6.4 State machine (`state.rs`)

`AgentUiStatus` (serialized `snake_case`): `pending | idle | done | running |
waiting_approval | error`.

`AppState::agent_status(id)` precedence (highest first):

1. pending permission for the session → `waiting_approval`
2. sticky error present → `error`
3. server status `busy|retry` → `running`
4. session reached a clean idle after running (in `finished` set) → `done`
5. server status `idle` / known session with no activity → `idle`
6. unknown session → `pending`

Reducer rules: a new run (`busy`) clears the sticky error; `session.idle` after a
run emits `Effect::Finished` **once**; `session.error` emits `Effect::Failed`
once per distinct error; a new `permission.updated` emits `Effect::Approval`
once. `session.deleted` cascades (sessions/status/active/finished/errored/todos/
pending). `session.created|updated` upserts.

### 6.5 SSE hub (`events.rs`)

- `EventHub::run()` loops forever until shutdown; never returns `Err`.
- Per connection: `health()` (best effort) → `GET /event` → `set_connection(true)`
  → publish snapshot → `resync()` (list sessions + statuses) → consume frames.
- On EOF/error: `mark_disconnected()` (publish snapshot with `connected:false`),
  then backoff. A handshake resets the attempt counter.
- Backoff: `delay_for(attempt, jitter)` = `min(500ms << attempt, 30s)` + jitter
  < 250ms. `attempt` caps at 6 (30s).
- Frame decoding (`decode_payload`) handles: plain `data:` event, the
  `/global/event` `{directory, payload}` envelope, and a bare payload whose type
  comes from the SSE `event:` line. Unknown types become `Event::Unknown`.
- `bootstrap()` never panics; a stopped server yields
  `BootstrapOutcome::Disconnected{error}`; with no sessions it creates a root
  session titled `DEFAULT_ROOT_TITLE` ("Orchestrator").
- `CoreMessage` envelope (serialized `tag="kind"`):
  `{"kind":"raw_event","event":…}` |
  `{"kind":"state_snapshot","snapshot":…}` |
  `{"kind":"effect","effect":…}`. `Effect` serializes `tag="effect"`.

### 6.6 Notification policy

Core computes `Effect`; the **shell** always shows it (`src-tauri/src/notify.rs`),
regardless of whether the main window is focused. Do not send notifications for
message/part streaming.

---

## 7. Tauri shell (thin glue)

- `tauri.conf.json`: `productName "Orquestador"`, identifier
  `com.orquestador.app`, one window `label:"main"` 1440x900 (min 960x600),
  `frontendDist ../frontend/dist`, `devUrl http://localhost:1420`,
  `beforeDevCommand`/`beforeBuildCommand` run `npm --prefix ../frontend …`.
- `capabilities/default.json`: window `main`, permissions `["core:default"]`.
- `AppBridge` (`bridge.rs`): builds the client from env, owns the broadcast
  sender + shutdown watch, exposes `subscribe`, `snapshot`, `bootstrap`,
  `spawn_hub` (on `tauri::async_runtime`), `request_shutdown`.
- **Event name**: `core://message` (constant `CORE_EVENT`). Payload = the
  serialized `CoreMessage`.
- Window close (`WindowEvent::Destroyed`) → `bridge.request_shutdown()`.

### 7.1 Tauri command list (23)

All are `async fn(State<'_, AppBridge>, …) -> Result<T, String>`; errors are the
core `Display` string.

| Command | Args | Returns | Core call |
|---|---|---|---|
| `get_state` | — | `AppStateSnapshot` | `bridge.snapshot()` |
| `get_health` | — | `Health` | `health()` |
| `get_agents` | — | `Agent[]` | `agents()` |
| `session_statuses` | — | `{ [id]: SessionStatus }` | `session_statuses()` |
| `list_sessions` | — | `Session[]` | `list_sessions()` |
| `create_session` | `parent_id?`, `title?` | `Session` | `create_session()` |
| `list_children` | `session_id` | `Session[]` | `session_children()` |
| `delete_session` | `session_id` | `bool` | `delete_session()` |
| `rename_session` | `session_id`, `title` | `Session` | `rename_session()` |
| `get_todos` | `session_id` | `Todo[]` | `todos()` |
| `send_message` | `session_id`, `text`, `agent?` | `MessageBundle` | `prompt()` |
| `send_message_async` | `session_id`, `text`, `agent?` | `void` | `prompt_async()` |
| `send_command` | `session_id`, `command`, `arguments?` | `MessageBundle` | `command()` |
| `abort_session` | `session_id` | `bool` | `abort()` |
| `get_messages` | `session_id`, `limit?` | `MessageBundle[]` | `messages()` |
| `get_message` | `session_id`, `message_id` | `MessageBundle` | `message()` |
| `respond_permission` | `session_id`, `permission_id`, `response`, `remember?` | `bool` | `respond_permission()` |
| `get_diff` | `session_id`, `message_id?` | `FileDiff[]` | `diff()` |
| `list_files` | `path?` | `FileNode[]` | `file_tree()` |
| `read_file` | `path` | `FileContent` | `file_content()` |
| `file_status` | — | `FileStatus[]` | `file_status()` |
| `find_files` | `query`, `limit?` | `string[]` | `find_files()` |
| `find_symbols` | `query` | `unknown[]` | `find_symbols()` |

Arg names use snake_case; the JS side calls `invoke("send_message", { sessionId, text })`
— **Tauri 2 converts camelCase JS keys to the snake_case Rust params**
(`session_id` ⇄ `sessionId`). Keep this in mind for every command.

---

## 8. Frontend contract

### 8.1 Stack & decisions

- Vite + React 18 + TypeScript **strict** (`noUnusedLocals`, `noUnusedParameters`).
- `@tauri-apps/api` v2 (`invoke` from `.../core`, `listen` from `.../event`).
- Monaco: `monaco-editor` + `@monaco-editor/react`, **bundled locally** (no CDN):
  build workers with `?worker` imports and `loader.config({ monaco })`; Vite
  `worker.format = "es"` (already set). Worker files: `editor.worker`,
  `json.worker`, `ts.worker` (create the rest only if the language needs them).
- State: hand-rolled store on `useSyncExternalStore` (`src/store.ts`) — **no
  zustand/redux**. Core owns truth; the store caches the latest snapshot + the
  latest raw message.
- Dev server strictly on port 1420.

### 8.2 Wire types

`frontend/src/types.ts` mirrors the serde shapes (already written). Keep the
`CoreMessage` union `{kind:"raw_event"|"state_snapshot"|"effect"}` and `Effect`
`{effect:"finished"|"failed"|"approval"}` exactly.

### 8.3 Store contract (`store.ts`)

- `store.setState(snapshot)` — used after `invoke("get_state")`.
- `store.pushMessage(msg)` — folds `state_snapshot` into the cache, records
  `last_event` for the raw stream.
- Hooks: `useAppState(): AppStateSnapshot | null`, `useLastMessage(): CoreMessage | null`.
- Extend with: `rawEvents` ring buffer (cap ~500) + `useRawEvents(sessionId?)`,
  and selectors `useSessions()`, `usePendingPermissions()`. Keep it **small**.

### 8.4 Target component tree (AGENT 2 builds)

```
<App>
├── <TopBar>                         connected dot, server version, root session
├── <Workspace>                      CSS grid; collapsible panels
│   ├── <AgentPanel>                 sub-agents of root (list_children + live)
│   │   └── <AgentCard>              title, status chip, "open pane", abort btn
│   ├── <PaneGrid>                   split panes (1 / 2-up / grid), each:
│   │   ├── <AgentPane>              header(title,status) + <MessageStream>
│   │   │   ├── <MessageStream>      renders parts from raw events for its session
│   │   │   └── <FollowUpInput>      "re-instruct" -> send_message_async
│   │   └── <ApprovalDialog>         modal on Effect::approval (once/always/reject)
│   ├── <EditorPanel>                tabs: <FileTree> | <DiffViewer>
│   │   ├── <FileTree>               list_files/read_file
│   │   ├── <MonacoEditor>           open file content
│   │   └── <DiffViewer>             get_diff -> Monaco DiffEditor (per file)
│   ├── <NotificationsDrawer>        transient list of Effect events
│   └── <ActivityBar>                composer toggle, pane layout, theme
└── <Composer>                       terminal-style input -> send_message_async
```

Status chip colors map 1:1 to `AgentUiStatus`. All buttons that mutate server
state call commands and **do not** assume success (surface the `String` error).

### 8.5 Streaming strategy

- `get_messages({session_id, limit})` hydrates a pane's history on open.
- Afterwards, filter `rawEvents` whose payload is a `message.updated` /
  `part.updated` for that `sessionID` and render incrementally. Drop
  `part.removed`/`message.removed` accordingly.
- Keep the DOM windowed (render the last N parts or use a virtual list) — streams
  can be long.

---

## 9. Ordered implementation steps

Each step lists **files**, an **acceptance command**, and **depends on**. `S1`–`S2`
are already satisfied by the scaffold; AGENT 2 starts at `S3`.

| # | Step | Files | Acceptance | Depends |
|---|---|---|---|---|
| S1 | Confirm scaffold builds | — | `bash scripts/verify-headless.sh` (core tests + frontend build green) | — |
| S2 | Confirm manifest isolation | — | `(cd src-tauri && cargo metadata --format-version 1 >/dev/null)`; root `cargo metadata --no-deps` shows 1 member | — |
| S3 | Store + types hardening | `frontend/src/store.ts`, `types.ts` | `npm run typecheck` | S1 |
| S4 | Tauri command bindings | `frontend/src/commands.ts` (new) | `npm run typecheck` | S3 |
| S5 | `AgentPanel` + `AgentCard` | `frontend/src/components/AgentPanel.tsx`, `AgentCard.tsx` | `npm run build` | S4 |
| S6 | `Composer` | `frontend/src/components/Composer.tsx` | `npm run build` | S4 |
| S7 | `AgentPane` + `MessageStream` + split grid | `.../AgentPane.tsx`, `MessageStream.tsx`, `PaneGrid.tsx` | `npm run build` | S4,S5 |
| S8 | `ApprovalDialog` + notifications drawer | `.../ApprovalDialog.tsx`, `NotificationsDrawer.tsx` | `npm run build` | S4 |
| S9 | File tree + Monaco editor + workers | `.../FileTree.tsx`, `MonacoEditor.tsx`, `monaco/setup.ts` | `npm run build` | S4 |
| S10 | Diff viewer (Monaco DiffEditor) | `.../DiffViewer.tsx` | `npm run build` | S9 |
| S11 | App wiring, layout, styles | `App.tsx`, `styles.css`, `components/*` | `npm run typecheck && npm run build` | S5–S10 |
| S12 | Verify shell once deps are installed | `src-tauri/**` | `bash scripts/setup-system-deps.sh` then `cargo build` in `src-tauri` | S2, env |
| S13 | Write implementation notes | `docs/IMPLEMENTATION_NOTES.md` | file exists, commands + deviations recorded | S11 |

**Rules for every step**: no `todo!()`/`unimplemented!()` (audit gate §11); keep
TypeScript strict-clean; do not add a state framework; do not touch `SPEC.md`.

---

## 10. Verification matrix

| Check | Command | Status here |
|---|---|---|
| Workspace resolves | `cargo metadata --no-deps --format-version 1` | PASS (1 member) |
| Core compiles | `cargo check -p orquestador-core` | PASS |
| Core tests | `cargo test -p orquestador-core` | PASS (37) |
| Core lint | `cargo clippy -p orquestador-core --all-targets -- -D warnings` | PASS |
| Formatting | `cargo fmt --all -- --check` | PASS |
| Frontend install | `cd frontend && npm install` | PASS |
| Frontend types | `cd frontend && npx tsc --noEmit` | PASS |
| Frontend build | `cd frontend && npm run build` | PASS |
| Shell manifests | `cd src-tauri && cargo metadata --format-version 1` | PASS (no compile) |
| Shell compiles | `cd src-tauri && cargo build` | **BLOCKED** — needs `libwebkit2gtk-4.1-dev` (user runs `scripts/setup-system-deps.sh`) |
| GUI smoke | `npx @tauri-apps/cli dev` | **BLOCKED** — same |

`scripts/verify-headless.sh` runs the whole unblocked chain in one shot.

---

## 11. Stub policy & audit gate

- No `todo!()` / `unimplemented!()` may remain in `crates/` or `src-tauri/src/`.
- Audit command (must print nothing / exit 0):
  `grep -rnE 'todo!\(|unimplemented!\(' crates src-tauri/src`
- `console.log`/`eprintln!` allowed only for genuine error paths (prefix
  `[orquestador]`), never for state the UI already shows.

---

## 12. Risks & open questions

1. **Unverifiable shell** — `src-tauri` cannot be compiled here. It is written to
   compile but is unproven; the first `cargo build` after installing deps may
   need small fixes (import paths, Tauri 2 API names). Expected, not a defect.
2. **Tauri 2 arg casing** — JS must send camelCase keys for snake_case Rust
   params. Verify `respond_permission` (`permissionId`) and `send_message_async`
   on the first manual run.
3. **Monaco workers** — wrong worker wiring shows "Could not create web worker".
   Use `?worker` imports + `loader.config({ monaco })`; do not load from CDN.
4. **`remember` field** — accepted as optional; if the deployed server rejects
   it, omit it (send only `{response}`).
5. **Endpoint return shapes** — `POST /session/:id/command` is modelled as
   `MessageBundle`; `/find/symbol` as `unknown[]`. Confirm against a live server
   and adjust the client if needed.
6. **Notification daemon** — absent D-Bus only logs; approvals also appear in the
   in-app drawer, so nothing is lost.
7. **`/global/event` vs `/event`** — the hub supports both single-event and
   enveloped payloads; if the server ever changes framing, extend
   `decode_payload` (it is unit-tested).
8. **Base URL/auth** — confirm `OPENCODE_SERVER_BASE_URL` / `OPENCODE_SERVER_PASSWORD`
   behavior with the user's actual `opencode serve` invocation.

---

## 13. Handoff to AGENT 2 / AGENT 3

- AGENT 2: follow §9, then write `docs/IMPLEMENTATION_NOTES.md` (what was built,
  deviations from this plan, exact verification output, how to run).
- AGENT 3: audit against `SPEC.md` + this plan, re-run §10 checks headless, fix
  only clearly-bounded defects, write `docs/AUDIT.md`, finalize `README.md` +
  the "First run on this machine" RUNBOOK.
- Neither agent commits; the orchestrator commits conventional messages on `main`.

## 13. Skills (header panel)

- **Core**: `crates/orquestador-core/src/skills.rs` (slugify, `SkillInfo { name, path, source, editable }`, `merge_skills`, `build_preamble`) — pure, unit-tested. Module declared in `lib.rs`.
- **Shell (Tauri)**: commands `list_skills`, `read_skill`, `save_skill`, `delete_skill`; both `send_message` and `send_message_async` accept `skills: Option<Vec<String>>` and fold selected skills into `PromptBody.system` (appended, never replaces). Skills are **backend-aware**: the panel lists the connected backend's native skills (`AgentBackend::id()`; `opencode` → `~/.config/opencode/skills` + `~/.agents/skills`, `claude`/`codex` mapped) **plus** app-owned skills at `<app_config_dir>/skills/<slug>.md` (created on demand, the only editable ones). `apply_skills` resolves app-first then native; missing skills are skipped.
- **Frontend**: `frontend/src/skills.ts` (selected-skills store, persisted to `localStorage["orquestador.skills"]`, `useSyncExternalStore`-compatible), `frontend/src/skillDrafts.ts` (per-tab editor drafts with baseline-based dirtiness), `frontend/src/components/SkillEditor.tsx`, `frontend/src/components/SkillSavePrompt.tsx`, and `SkillInfo` type in `types.ts`. TopBar adds Skills dropdown + `+` (New skill / Import `.md`). Skill tabs are dropped on layout restore; closing a dirty skill tab prompts Save/Discard/Cancel (non-skill tabs unaffected). Composer/FollowUpInput pass selected skills on every send.

