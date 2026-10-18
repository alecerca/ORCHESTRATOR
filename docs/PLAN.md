# Orquestador — Technical design reference

Detailed technical contract for the Orquestador desktop app: the module
boundaries, the exact wire shapes of the OpenCode server API, the Tauri command
surface, and the frontend contract. The product/API spec lives in `SPEC.md`; the
layered overview lives in `docs/ARCHITECTURE.md`.

## 1. Purpose & product recap

A desktop control tower for OpenCode sub-agents (Tauri 2 + TypeScript). One
window with: an **orchestrator composer**, an **agent panel** (sub-agent list
with status), **split panes** with each agent's live stream, **interrupt /
re-instruct**, **approval prompts** with desktop notifications, an **editor +
git diff viewer** (Monaco), and — for shared use — a **Cowork** mode that talks
to the `orquestador-proxy` auth/whitelist proxy (see `docs/COWORK.md`). Out of
scope for v1: worktrees, issues/PRs, multi-project switching, chat persistence.

Runtime target: a local `opencode serve` at `http://127.0.0.1:4096` (optional
HTTP basic auth via `OPENCODE_SERVER_PASSWORD`, user `opencode`).

## 2. Scope & status

The core and proxy crates are implemented and headless-verified
(`cargo test` + `cargo clippy` clean; `cargo fmt --check` currently reports
toolchain-drift diffs — see §9). The Tauri shell and the frontend are fully
written and the shell compiles in this environment (see §5); `src-tauri` is
cross-read against the frontend contract, not executed here. The project is
published on GitHub Releases.

## 3. Repository layout

```
.
├── Cargo.toml                      # workspace: members = ["crates/*"] (src-tauri excluded)
├── Cargo.lock
├── SPEC.md                         # product + API spec
├── README.md
├── crates/
│   ├── orquestador-core/           # ALL logic, no tauri dep, headless-tested
│   │   ├── Cargo.toml
│   │   └── src/{lib,types,error,client,state,events,backend,files,git,skills}.rs
│   └── orquestador-proxy/          # Cowork auth proxy CLI (roles, whitelist, presence)
│       ├── Cargo.toml
│       └── src/{main,lib,auth,authorize,config,proxy,adduser}.rs
├── src-tauri/                      # thin Tauri shell (own [workspace], NOT a root member)
│   ├── Cargo.toml                  # [workspace] empty table => standalone
│   ├── build.rs                    # tauri_build::build()
│   ├── tauri.conf.json · capabilities/default.json · icons/
│   └── src/{main,lib,bridge,commands,notify,proxy_config,server_config}.rs
├── frontend/                       # Vite + React + TS (strict) + Monaco
│   ├── package.json · tsconfig.json · tsconfig.node.json · vite.config.ts · index.html
│   └── src/{main.tsx,App.tsx,store.ts,types.ts,commands.ts,styles.css,…}
│       └── components/*.tsx        # TopBar, AgentPanel, PaneGrid, EditorPanel, …
├── scripts/{setup-system-deps.sh,install-deps.sh,install.sh,install.ps1,verify-headless.sh}
└── docs/{ARCHITECTURE,AUDIT,CONTRIBUTING,COWORK-SETUP,COWORK,INSTALL,LAUNCH,PLAN,ROADMAP,RUNBOOK}.md
```

## 4. Architecture overview

```
┌──────────────────────────── src-tauri (Tauri 2) ───────────────────────────┐
│  lib.rs      Builder: manage(AppBridge), emit("core://message"), bootstrap  │
│  bridge.rs   owns Arc<dyn AgentBackend> + Arc<RwLock<AppState>> + broadcast  │
│  commands.rs 46 #[tauri::command]s -> core (Result<T,String>)               │
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

1. `bootstrap()` does a synchronous load (health, sessions, statuses, agents,
   pending permissions) and picks/creates the root session; `spawn_hub()` starts
   `EventHub`.
2. `EventHub` keeps one SSE connection: on each event it updates `AppState`
   (state-changing events) and broadcasts a `CoreMessage`:
   - `RawEvent{event}` — every SSE event verbatim (message/part streaming; display only),
   - `StateSnapshot{snapshot}` — after state-changing or `session.status` events,
   - `Effect{effect}` — a notification the shell should raise.
3. `src-tauri` forwards every `CoreMessage` to the webview (`core://message`) and
   turns `Effect` into a desktop notification.
4. The webview reads full state with the `get_state` command and listens to
   `core://message`; commands are the only way it mutates server state.

## 5. Build isolation (why `src-tauri` compiles alone)

The shell compiles in this environment. The isolation is structural, not
incidental:

- Root `Cargo.toml`: `members = ["crates/*"]` → the workspace contains **only**
  `orquestador-core` and `orquestador-proxy`. Root `cargo check`/`test` never
  compiles Tauri.
- `src-tauri/Cargo.toml` has an **empty `[workspace]` table**, which makes it its
  own workspace root and excludes it from the parent. Verified: `cargo metadata`
  inside `src-tauri` lists exactly one member (`orquestador`) and resolves
  `tauri`, `tauri-build`, `notify-rust`, `orquestador-core`.
- The core uses `reqwest` with `default-features = false, features =
  ["json","stream","rustls-tls"]` → **rustls**, no openssl link dependency.
- `notify-rust` is a dependency of `src-tauri` (NOT of core), so the core stays
  pure and headless; a missing D-Bus daemon only logs a warning.

## 6. `orquestador-core` contract

### 6.1 Modules

| Module | Responsibility | I/O? |
|---|---|---|
| `types` | Serde shapes of every server object + SSE events. Wire-exact renames. | no |
| `error` | `CoreError` (`thiserror`) + `kind()` + helpers (`is_not_found`, `is_usage_limit`). | no |
| `client` | `OpenCodeClient` + `ClientConfig` + `decode_payload`. **Only** place that builds URLs/HTTP. | yes (reqwest) |
| `state` | `AppState` pure reducer, `AgentUiStatus`, `AppStateSnapshot`, `Effect`. | no |
| `events` | `EventHub` (SSE + backoff + resync + reconcile), `CoreMessage`, `bootstrap`. | yes (SSE) |
| `backend` | `AgentBackend` trait + `BackendEvent`/`EventStream` — the seam that keeps the hub/shell backend-agnostic. | no |
| `files` | `write_text_file` — validated repository-relative write helper. | yes (fs) |
| `git` | Read-only git introspection (`branch_for_dir`, `overview`, `commit_files`, `branches`, `status`, `show`) + the single mutating `checkout`/`discard_file`. | yes (`git`) |
| `skills` | Pure `slugify`, `SkillInfo`, `merge_skills`, `build_preamble`. | no |

Re-exports (`lib.rs`): `OpenCodeClient`, `CoreError`, `EventHub`, `AppState`,
`DEFAULT_BASE_URL`. Also public: `backend::{AgentBackend, BackendEvent,
EventStream, BACKEND_OPENCODE}`, `skills::{build_preamble, slugify, SkillInfo}`.

### 6.2 `ClientConfig`

```rust
pub struct ClientConfig {
    pub base_url: String,          // default http://127.0.0.1:4096
    pub username: String,          // default "opencode"
    pub password: Option<String>,  // OPENCODE_SERVER_PASSWORD
    pub directory: Option<String>, // x-opencode-directory scope; None = server cwd
    pub member_name: Option<String>, // optional attribution name for created sessions
    pub request_timeout: Duration, // default 30s
}
```

`ClientConfig::from_env_or_default()` reads `OPENCODE_SERVER_BASE_URL`
(`OPENCODE_SERVER_URL` accepted as a legacy alias), `OPENCODE_SERVER_USERNAME`
(optional, default `opencode`), and `OPENCODE_SERVER_PASSWORD` (optional, enables
basic auth). `OpenCodeClient::new(cfg)` is synchronous and returns
`Result<_, CoreError>`. Every request (and the SSE stream) sends
`x-opencode-directory` when `directory` is set, which is how one server can
serve several projects without a restart.

### 6.3 Exact API type shapes (wire)

Wire names use **capitalised abbreviations** — naive camelCase is wrong:

| Field | Wire |
|---|---|
| session id | `id` |
| project id | `projectID` |
| parent id | `parentID` |
| message id | `messageID` |
| permission id | `permissionID` / request id `per_…` |
| model | `{ providerID, modelID }` (an **object**, not a string) |
| built in (agent) | `builtIn` |
| agent mode | `type` |
| project scope header | `x-opencode-directory` |

Key structs (see `types.rs` for the full list):

```
Session      { id, projectID, directory, parentID?, title, version,
               time:{created,updated,compacting?}, summary? }
SessionStatus = {type:"idle"} | {type:"busy"}
              | {type:"retry", attempt, message, next}
MessageBundle { info: Message, parts: Part[] }     // history + reply shape
Part (12 variants + Unknown)   ToolState · PermissionPattern
Permission     { id, type, pattern?, sessionID, messageID, callID?, title, metadata, time }
PermissionRequest { id, sessionID, permission, patterns, metadata, always, tool? }
PermissionResponse = "once" | "always" | "reject"   // NOT allow/deny
Todo         { id, content, status, priority }
FileDiff · FileNode · FileContent · FileStatus · Project · PresenceEntry
Agent        { name, description?, type, builtIn, color?, model? }
PromptBody   { messageID?, model?, agent?, noReply?, system?, tools?, parts:[InputPart] }
InputPart    = { type:"text", text }                // v1 composer sends text only
CreateSessionBody { parentID?, title? }
CommandBody  { command, arguments? }
Event (17 variants + Unknown, tagged on `type`)
```

**Wire notes** (record, do not "fix" in code):

1. Permission replies vary by id form and route:
   - a **request id** (`per_…`, from `GET /permission` or `permission.updated`)
     answers `POST /permission/:requestID/reply` with body `{ reply, message? }`
     (`reply` is `once`/`always`/`reject`; `message` is added only when present,
     because the route rejects extra properties and a JSON `null`);
   - a **session permission id** answers
     `POST /session/:id/permissions/:permissionID` with body
     `{ response, remember? }`. `remember` is **omitted** when `None` — a JSON
     `null` would make strict servers reject the body. `OpenCodeClient::respond_permission`
     routes automatically on the `per_` prefix.
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
once. A usage/rate limit is surfaced as `Effect::UsageLimit` (distinct from
`Failed`) and leaves the "running" state, whether it arrives as a
`session.error` or a `retry` status. `session.deleted` cascades
(sessions/status/active/finished/errored/todos/pending/root). `session.created|updated`
upserts. The snapshot also carries timing (`status_since`, `last_run_ms` derivable
from the run window, `WaitingApproval` included).

### 6.5 SSE hub (`events.rs`)

- `EventHub::run()` loops forever until shutdown; never returns `Err`.
- Per connection: `health()` (best effort) → `stream_events()` →
  `set_connection(true)` → publish snapshot → `resync()` (list sessions +
  statuses) → consume frames.
- The consume loop also runs two `select!` arms: a periodic HTTP **reconcile**
  (`RECONCILE_INTERVAL` = 5 s: statuses + pending permissions, because SSE does
  not replay), and a **idle watchdog** (`STREAM_IDLE_TIMEOUT` = 30 s) that breaks
  a half-open stream so the outer loop reconnects.
- On EOF/error: `mark_disconnected()` (publish snapshot with `connected:false`),
  then backoff. A `server.connected` handshake resets the attempt counter.
- Backoff: `delay_for(attempt, jitter)` = `min(500ms << attempt, 30s)` + jitter
  < 250ms. `attempt` caps at 6 (30s).
- Frame decoding (`client::decode_payload`) handles: plain `{"type","properties"}`
  event, the `/global/event` `{directory, payload}` envelope, and a bare payload
  whose type comes from the SSE `event:` line. Unknown types become
  `Event::Unknown`.
- `bootstrap()` never panics; a stopped server yields
  `BootstrapOutcome::Disconnected{error}`; with no sessions it creates a root
  session titled `[<member>] Orchestrator` (`attributed_title`). Bootstrap also
  reconciles `GET /permission`, remembering the fetch start so a `permission.updated`
  that races in mid-fetch is not dropped.
- `CoreMessage` envelope (serialized `tag="kind"`):
  `{"kind":"raw_event","event":…}` |
  `{"kind":"state_snapshot","snapshot":…}` |
  `{"kind":"effect","effect":…}`. `Effect` serializes `tag="effect"`.

### 6.6 Notification policy

Core computes `Effect`; the **shell** always shows it (`src-tauri/src/notify.rs`,
`notify-rust`), regardless of whether the main window is focused. Do not send
notifications for message/part streaming.

## 7. Tauri shell (thin glue)

- `tauri.conf.json`: `productName "Orquestador"`, identifier
  `com.orquestador.app`, one window `label:"main"` 1440x900 (min 960x600),
  `frontendDist ../frontend/dist`, `devUrl http://localhost:1420`,
  `beforeDevCommand`/`beforeBuildCommand` run `npm --prefix ../frontend …`.
- `capabilities/default.json`: window `main`, permissions `["core:default"]`.
- `AppBridge` (`bridge.rs`): builds the client from persisted config or env,
  owns `Arc<dyn AgentBackend>` + `Arc<RwLock<AppState>>` + the broadcast sender +
  shutdown watch; exposes `subscribe`, `snapshot`, `bootstrap`, `spawn_hub` (on
  `tauri::async_runtime`), `reconnect`, `push_snapshot`, `push_effect`,
  `request_shutdown`.
- **Event name**: `core://message` (constant `CORE_EVENT`). Payload = the
  serialized `CoreMessage`.
- Window close (`WindowEvent::Destroyed`) → `bridge.request_shutdown()`.

### 7.1 Tauri command list (46)

Every command is `async fn` returning `Result<T, String>` unless noted; errors
are the core `Display` string. `bridge: State<'_, AppBridge>` and
`app: AppHandle` parameters are injected by Tauri and are not listed below; the
**Args** column lists only the caller-supplied arguments (snake_case Rust — the
JS side sends camelCase keys).

| Command | Args | Returns | Core call |
|---|---|---|---|
| `get_state` | — | `AppStateSnapshot` | `bridge.snapshot()` |
| `get_health` | — | `Health` | `client().health()` |
| `test_connection` | `base_url`, `username?`, `password?` | `Health` | `OpenCodeClient::new(cfg).health()` |
| `get_server_config` | — | `ServerConfigDto` | `server_config::load_from_dir` |
| `set_server_config` | `base_url`, `username?`, `password?`, `member_name?`, `proxy_config_path?` | `ServerConfigDto` | save + `bridge.reconnect` |
| `list_projects` | — | `Project[]` | `client().projects()` |
| `current_project` | — | `Project` | `client().current_project()` |
| `set_project_directory` | `directory?` | `ServerConfigDto` | save + `bridge.reconnect` |
| `get_agents` | — | `Agent[]` | `client().agents()` |
| `get_presence` | — | `PresenceEntry[]` | `client().presence()` |
| `get_workspace_scope` | — | `string?` | `client().workspace_scope()` |
| `session_statuses` | — | `{ [id]: SessionStatus }` | `client().session_statuses()` |
| `list_sessions` | — | `Session[]` | `client().list_sessions()` |
| `create_session` | `parent_id?`, `title?` | `Session` | `client().create_session()` |
| `list_children` | `session_id` | `Session[]` | `client().session_children()` |
| `delete_session` | `session_id` | `bool` | `client().delete_session()` |
| `rename_session` | `session_id`, `title` | `Session` | `client().rename_session()` |
| `get_todos` | `session_id` | `Todo[]` | `client().todos()` |
| `send_message` | `session_id`, `text`, `agent?`, `skills?` | `MessageBundle` | `client().prompt()` + `apply_skills` |
| `send_message_async` | `session_id`, `text`, `agent?`, `skills?` | `void` | `client().prompt_async()` + `apply_skills` |
| `abort_session` | `session_id` | `bool` | `client().abort()` |
| `get_messages` | `session_id`, `limit?` | `MessageBundle[]` | `client().messages()` |
| `get_message` | `session_id`, `message_id` | `MessageBundle` | `client().message()` |
| `send_command` | `session_id`, `command`, `arguments?` | `MessageBundle` | `client().command()` |
| `respond_permission` | `session_id`, `permission_id`, `response`, `remember?` | `bool` | `client().respond_permission()` |
| `list_files` | `path?` | `FileNode[]` | `client().file_tree()` |
| `read_file` | `path` | `FileContent` | `client().file_content()` |
| `file_status` | — | `FileStatus[]` | `client().file_status()` |
| `find_files` | `query`, `limit?` | `string[]` | `client().find_files()` |
| `find_symbols` | `query` | `unknown[]` | `client().find_symbols()` |
| `list_skills` | — | `SkillInfo[]` | `merge_skills(app_skill_list, native_skill_list)` |
| `read_skill` | `name` | `string` | `resolve_skill_path` + read |
| `save_skill` | `name`, `content` | `SkillInfo` | write `<slug>.md` |
| `delete_skill` | `name` | `bool` | remove `<slug>.md` |
| `get_proxy_members` | `path` | `ProxyMemberDto[]` | `proxy_config::list_members` |
| `add_proxy_member` | `path`, `name`, `role`, `password` | `void` | `proxy_config::add_member` |
| `remove_proxy_member` | `path`, `name` | `void` | `proxy_config::remove_member` |
| `git_branch` | `directory` | `string?` | `git::branch_for_dir` |
| `git_log` | `limit?` | `RepoLog` | `git::overview` |
| `git_commit_files` | `hash` | `CommitFile[]` | `git::commit_files` |
| `git_branches` | — | `Branch[]` | `git::branches` |
| `git_checkout` | `name` | `void` | `git::checkout` |
| `git_status` | — | `Change[]` | `git::status` |
| `git_show_file` | `path` | `HeadFile` | `git::show` |
| `git_discard_file` | `path` | `void` | `git::discard_file` |
| `write_file` | `path`, `content` | `void` | `files::write_text_file` |

`git_branch` is the one command that returns `Option<String>` directly (never an
error). Arg names use snake_case; the JS side calls
`invoke("send_message", { sessionId, text })` — **Tauri 2 converts camelCase JS
keys to the snake_case Rust params** (`session_id` ⇄ `sessionId`). Keep this in
mind for every command.

## 8. Frontend contract

### 8.1 Stack & decisions

- Vite + React 18 + TypeScript **strict** (`noUnusedLocals`, `noUnusedParameters`).
- `@tauri-apps/api` v2 (`invoke` from `.../core`, `listen` from `.../event`).
- Monaco: `monaco-editor` + `@monaco-editor/react`, **bundled locally** (no CDN):
  build workers with `?worker` imports and `loader.config({ monaco })` in
  `monaco/setup.ts`; Vite `worker.format = "es"`. `main.tsx` imports the setup
  module before `App`.
- State: hand-rolled store on `useSyncExternalStore` (`src/store.ts`) — **no
  zustand/redux**. The core owns truth; the store caches the latest snapshot, a
  bounded raw-event ring buffer (cap 500), and a per-session reducer over
  `message.*` / `message.part.*` events.
- Dev server strictly on port 1420.

### 8.2 Wire types

`frontend/src/types.ts` mirrors the serde shapes (capitalised abbreviations:
`sessionID`, `parentID`, `messageID`, `permissionID`, `projectID`, `providerID`).
Keep the `CoreMessage` union `{kind:"raw_event"|"state_snapshot"|"effect"}` and
`Effect` `{effect:"finished"|"failed"|"approval"|"usage_limit"}` exactly.

### 8.3 Store contract (`store.ts`)

- `store` folds `state_snapshot` into the cached snapshot and `raw_event` into
  the raw ring buffer + per-session stream reducer; derived arrays are cached and
  only invalidated on mutation so `getSnapshot` returns stable references.
- Hooks: `useAppState()`, `useSessions()`, `usePendingPermissions()`,
  `useSessionStream(sessionId)`. `useLastMessage()` and `useRawEvents()` exist
  but are currently **unused** (see §11 / `docs/AUDIT.md`).
- Keep it **small**.

### 8.4 Commands (`commands.ts`)

Typed 1:1 `invoke` wrappers for all 46 Tauri commands, using camelCase argument
keys. Server settings live in `serverConfig.ts`; the effective member identity in
`identity.ts`; the selected skills store in `skills.ts` (`localStorage`).

### 8.5 Component tree (high level)

```
<App>
├── <TopBar>               connected dot, server version, root picker, identity/cowork, skills
├── <Workspace>            resizable panels (agent panel · panes · editor)
│   ├── <AgentPanel>/<AgentCard>/<StatusChip>   sub-agents of the root (live snapshot)
│   ├── <PaneGrid>         split panes: <AgentPane> → <MessageStream> + <FollowUpInput>,
│   │                      <ApprovalDialog>, skill tabs, diff tabs
│   ├── <EditorPanel>      tabs: <FileTree>/<FileViewer>/<MonacoEditor>,
│   │                      <DiffPanel> → git <DiffViewer>, <GitViewer>
│   ├── <NotificationsDrawer>   transient Effect list
│   └── <ActivityBar>      composer toggle, pane layout, theme
├── <Composer>             terminal-style input → send_message_async
└── dialogs                Settings, ProjectPicker, JoinWorkspace, Members, Welcome
```

Status chip colors map 1:1 to `AgentUiStatus`. All buttons that mutate server
state call commands and **do not** assume success (surface the `String` error).

### 8.6 Streaming strategy

- `get_messages({session_id, limit})` hydrates a pane's history on open.
- Afterwards, `useSessionStream(sessionId)` folds `raw_event` frames whose
  payload is a `message.updated` / `part.updated` for that `sessionID` and
  renders incrementally. `part.removed`/`message.removed` are dropped.
- Keep the DOM windowed (render the last N parts or use a virtual list) — streams
  can be long.

## 9. Verification matrix

| Check | Command | Status here |
|---|---|---|
| Workspace resolves | `cargo metadata --no-deps --format-version 1` | PASS (2 members) |
| Core compiles | `cargo check -p orquestador-core` | PASS |
| Core tests | `cargo test -p orquestador-core` | PASS (132) |
| Proxy tests | `cargo test -p orquestador-proxy` | PASS (74: 56 unit + 18 integration) |
| Core lint | `cargo clippy --workspace --all-targets -- -D warnings` | PASS (clean) |
| Formatting | `cargo fmt --all -- --check` | **FAIL** — the local rustfmt would reformat several files (`backend.rs`, `client.rs`, `events.rs`, `skills.rs`, `authorize.rs`, `config.rs`); toolchain drift, not a logic defect |
| Frontend types | `cd frontend && npx tsc --noEmit` | PASS (exit 0) |
| Frontend build | `cd frontend && npm run build` | not run in this pass (writes `frontend/dist`) |
| Tauri commands | `grep -c '#\[tauri::command\]' src-tauri/src/commands.rs` | 46 |
| Shell manifests | `cd src-tauri && cargo metadata --format-version 1` | PASS (no compile) |
| Shell compiles | `cd src-tauri && cargo build` | **NOT RUN** — `libayatana-appindicator3` absent + sudo needs a password; compile `webkit2gtk-4.1` is now present |
| GUI smoke | `npx @tauri-apps/cli dev` | **NOT RUN** — same |

`scripts/verify-headless.sh` runs the whole unblocked chain in one shot.

## 10. Stub policy & audit gate

- No `todo!()` / `unimplemented!()` may remain in **production** code in
  `crates/` or `src-tauri/src/`. The test scaffolding is exempt: the `#[cfg(test)]`
  `FakeBackend` in `crates/orquestador-core/src/events.rs` uses `unimplemented!()`
  for backend methods its tests do not exercise. Scope the audit to non-test
  code, e.g. run `grep -rnE 'todo!\(|unimplemented!\(' crates src-tauri/src`
  and confirm every hit falls inside a `#[cfg(test)]` module (in this tree the
  only hits are in `events.rs`'s test module).
- `console.log`/`eprintln!` allowed only for genuine error paths (prefix
  `[orquestador]`), never for state the UI already shows.

## 11. Risks & open questions

1. **Shell build verification (Resolved 2026-10-15)** — The shell now compiles in
   this environment, and the release workflow built GUI bundles on ubuntu,
   windows and macos for `v0.1.0`/`v0.2.0`. The original concern: `src-tauri` was
   unproven here; the first `cargo build` after installing deps may need small
   fixes (import paths, Tauri 2 API names). Expected, not a defect.
2. **Tauri 2 arg casing** — JS must send camelCase keys for snake_case Rust
   params. Verify `respond_permission` (`permissionId`) and `send_message_async`
   on the first manual run.
3. **Monaco workers** — wrong worker wiring shows "Could not create web worker".
   Use `?worker` imports + `loader.config({ monaco })`; do not load from CDN.
4. **Endpoint return shapes** — `POST /session/:id/command` is modelled as
   `MessageBundle`; `/find/symbol` as `unknown[]`. Confirm against a live server
   and adjust the client if needed.
5. **Notification daemon** — absent D-Bus only logs; approvals also appear in the
   in-app drawer, so nothing is lost.
6. **`/global/event` vs `/event`** — the hub supports both single-event and
   enveloped payloads; if the server ever changes framing, extend
   `decode_payload` (it is unit-tested).
7. **Base URL/auth** — confirm `OPENCODE_SERVER_BASE_URL` / `OPENCODE_SERVER_PASSWORD`
   behavior with the user's actual `opencode serve` invocation.

## 12. Skills (header panel)

- **Core**: `crates/orquestador-core/src/skills.rs` (slugify, `SkillInfo { name, path, source, editable }`, `merge_skills`, `build_preamble`) — pure, unit-tested. Module declared in `lib.rs`.
- **Shell (Tauri)**: commands `list_skills`, `read_skill`, `save_skill`, `delete_skill`; both `send_message` and `send_message_async` accept `skills: Option<Vec<String>>` and fold selected skills into `PromptBody.system` (appended, never replaces). Skills are **backend-aware**: the panel lists the connected backend's native skills (`AgentBackend::id()`; `opencode` → `~/.config/opencode/skills` + `~/.agents/skills`, `claude`/`codex` mapped) **plus** app-owned skills at `<app_config_dir>/skills/<slug>.md` (created on demand, the only editable ones). `apply_skills` resolves app-first then native; missing skills are skipped.
- **Frontend**: `frontend/src/skills.ts` (selected-skills store, persisted to `localStorage["orquestador.skills"]`, `useSyncExternalStore`-compatible), `frontend/src/skillDrafts.ts` (per-tab editor drafts with baseline-based dirtiness), `frontend/src/components/SkillEditor.tsx`, `frontend/src/components/SkillSavePrompt.tsx`, and `SkillInfo` type in `types.ts`. TopBar adds Skills dropdown + `+` (New skill / Import `.md`). Skill tabs are dropped on layout restore; closing a dirty skill tab prompts Save/Discard/Cancel (non-skill tabs unaffected). Composer/FollowUpInput pass selected skills on every send.
