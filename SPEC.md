# Orquestador — Product spec

Orquestador is a desktop control tower for local OpenCode sub-agents. This file
is the source of truth for **product scope** and for the **OpenCode server API
surface** the client implements. The detailed technical contract (module
boundaries, wire types, Tauri command list, frontend contract) lives in
`docs/PLAN.md`; the layered overview lives in `docs/ARCHITECTURE.md`.

## 1. Product (what we build)

A desktop control tower for OpenCode sub-agents, named **orquestador**.
Third-party inspiration: Orca — but WITHOUT worktree/issue complexity. What the
user wants:

1. **Orchestrator composer**: a terminal-style input where the user instructs
   the orchestrator agent ("create 5 sub-agents, one does X…"). Sends via
   `POST /session/:id/prompt_async` (non-blocking); `POST /session/:id/message`
   (blocking, waits for the reply) remains available for callers that want it.
2. **Agent panel**: every sub-agent spawned from the root session, each with its
   task, live status (pending / idle / running / done / error /
   waiting-approval), and a detail pane.
3. **Split panes**: several sub-agent panes side by side, each showing that
   agent's live message stream.
4. **Interrupt / re-instruct**: from any pane, `abort` the agent
   (`POST /session/:id/abort`) or send a follow-up instruction.
5. **Approval prompts**: when a sub-agent needs human approval, the app surfaces
   it and lets the user answer `once` / `always` / `reject`. Replies route by id
   form: request ids (`per_…`) go to `POST /permission/:requestID/reply`, session
   permission ids go to `POST /session/:id/permissions/:permissionID`. A desktop
   notification accompanies each request.
6. **Editor + diff viewer**: file tree + code editor (Monaco) + a git
   **working-tree** diff viewer (diff list, per-file diff, commit files, branch
   listing/checkout, discard file). The OpenCode `GET /session/:id/diff`
   endpoint is implemented in the core client, but the UI currently renders git
   working-tree diffs instead.
7. **Notifications**: desktop notifications when a sub-agent finishes, errors,
   hits a usage limit, or requests approval.
8. **Cowork**: optional shared mode through the `orquestador-proxy`
   auth/whitelist proxy (roles, presence, member management) — see
   `docs/COWORK.md`.

- **Explicitly OUT of scope (v1)**: worktree management, GitHub issues/PRs,
  multi-project switching, chat history persistence.

### Who uses it

A solo developer or a small team. It is a cross-platform desktop app (Linux,
macOS, Windows); it talks to a local `opencode serve` (default
`http://127.0.0.1:4096`). A single user runs it directly; a team can share one
server through the Cowork proxy (`docs/COWORK.md`), which adds optional HTTP
basic auth and per-member roles on top of the plain server.

## 2. Fixed stack (decided)

- **Backend**: Rust, **Tauri 2** for the desktop shell.
- **Frontend**: TypeScript in the Tauri webview (Vite), strict mode. Editor:
  **Monaco Editor** (bundled locally, no CDN). Agent "panes" are live message
  streams rendered as UI, NOT `xterm` PTYs.
- **HTTP + SSE**: Rust client for the OpenCode server API (`reqwest` + `tokio`,
  rustls; SSE via `eventsource-stream`). The frontend talks to Rust via Tauri
  commands (no CORS).
- **Desktop notifications**: `notify-rust` called from the shell; the core emits
  notification *intents* (`Effect`) and the shell always shows them. No Tauri
  notification plugin.
- **Language of all artifacts, code, comments, docs**: English.

## 3. Build & environment contract

- The Rust core (API client, session model, event hub, git/skills helpers) lives
  in a plain workspace member crate **without** `tauri`/`wry` dependencies, so
  it is fully `cargo test`-able headless.
- The Tauri shell is a **separate** crate (standalone `Cargo.toml` with an empty
  `[workspace]` table) excluded from the root workspace, so root
  `cargo check`/`cargo test` succeed without compiling tauri/wry/webkit.
  Workspace: `members = ["crates/*"]`.
- `reqwest` uses `default-features = false` + `rustls-tls`, so there is no
  openssl system link dependency.
- The Tauri shell additionally needs the system webview + indicator libraries
  (on Debian/Ubuntu: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`).
  `scripts/setup-system-deps.sh` prints the apt command the user runs once; it
  never runs `sudo` itself. On the current Linux dev machine `webkit2gtk-4.1` is
  present but `libayatana-appindicator3` is not and `sudo` needs a password, so
  the shell has **not** been compiled here.
- Verification chain: `cargo fmt --all -- --check` → `cargo clippy` →
  `cargo test` (workspace, headless) → frontend `npm run build` → optional
  `npx @tauri-apps/cli build` / `dev` (requires the system deps). See
  `scripts/verify-headless.sh` and `docs/RUNBOOK.md`.

## 4. OpenCode server API (source of truth for the client)

Base: `http://127.0.0.1:4096` (configurable). Optional HTTP basic auth via
`OPENCODE_SERVER_PASSWORD` (user `opencode`). When a project directory is
configured, every request and the SSE stream send the `x-opencode-directory`
header so one server can serve several projects.

- `GET /global/health` → `{ healthy, version }`
- `GET /project` · `GET /project/current`
- `GET /session` (list) · `POST /session` (`{ parentID?, title? }`) → Session
- `GET /session/status` → `{ [sessionID]: SessionStatus }`
- `GET /session/:id` · `DELETE /session/:id` · `PATCH /session/:id` (`{ title? }`)
- `GET /session/:id/children` → **sub-agents** (Session[])
- `GET /session/:id/todo` → Todo[]
- `POST /session/:id/abort` → **interrupt** (boolean)
- `POST /session/:id/message` body `{ messageID?, model?, agent?, noReply?, system?, tools?, parts }` → `{ info, parts }` (waits for the full reply). `model` is an **object** `{ providerID, modelID }`, not a string.
- `POST /session/:id/prompt_async` (same body) → 204 (no wait)
- `GET /session/:id/message?limit=` → `{ info, parts }[]` (history)
- `GET /session/:id/message/:messageID`
- `POST /session/:id/command` (`{ command, arguments? }`)
- `GET /permission` → PermissionRequest[] (pending requests, global)
- `POST /permission/:requestID/reply` body `{ reply, message? }` (**approval answer** for request ids; `reply` = `once` | `always` | `reject`; `message` is sent only when present)
- `POST /session/:id/permissions/:permissionID` body `{ response, remember? }` (**approval answer** for session permission ids; `remember` is omitted when absent)
- `GET /session/:id/diff?messageID=` → FileDiff[] (implemented in the core client; the UI uses git working-tree diffs instead — see §1)
- `GET /file?path=` → FileNode[] · `GET /file/content?path=` → FileContent · `GET /file/status` → File[]
- `GET /find?pattern=` · `GET /find/file?query=` · `GET /find/symbol?query=`
- `GET /event` → **SSE stream**, first event `server.connected`, then bus events
- `GET /agent` → Agent[]
- `GET /doc` → OpenAPI spec (reference; the client is hand-written)

Cowork proxy extensions (served by `orquestador-proxy`):

- `GET /orquestador/present`
- `GET /orquestador/whoami`

Session types (`Session`, `Message`, `Part`, `SessionStatus`, `FileDiff`,
`FileNode`, `FileContent`) — the core client declares the minimal set it needs in
`crates/orquestador-core/src/types.rs`. Wire names use capitalised abbreviations
(`sessionID`, `parentID`, `messageID`, `permissionID`, `projectID`, `providerID`).

## 5. Architecture contract

- **`orquestador-core`** (pure Rust, async tokio):
  - `client` module: typed API client (reqwest, rustls-tls); the ONLY place that
    builds URLs — every endpoint above lives here.
  - `events` module: SSE subscription, typed event models, reconnection with
    backoff, periodic HTTP reconcile, idle watchdog.
  - `state` module: session tree, per-agent status derivation, sticky-error
    tracking, permission-request registry.
  - `types`/`error`/`backend`/`files`/`git`/`skills` helpers.
- **UI (frontend/TypeScript)**:
  - Session/agent panel (tree of sessions, per-agent status chip).
  - Composer (orchestrator input).
  - Agent detail pane(s) with live stream + abort/follow-up buttons + approval
    dialog.
  - File tree + Monaco editor + git diff panel.
  - Notification integration (via Rust `Effect`s, surfaced by the shell).
  - State: a small hand-rolled store on `useSyncExternalStore` (no heavy state
    framework).
- **Tauri shell**: minimal window, 46 commands bridging frontend ↔
  `orquestador-core`, notification bridge.

## 6. Quality gates

- `cargo fmt --all -- --check` · `cargo clippy --all-targets -- -D warnings`
  (core + proxy) · `cargo test` (workspace, headless).
- Frontend: `npm run build` (Vite + `tsc` strict) — no runtime test infra in v1.
- The Tauri shell and GUI smoke run are **not verifiable in this environment**
  until the system deps are installed; any verification report must state
  explicitly what was checked headless (core tests, frontend build, command
  count) vs what requires the user's `npx @tauri-apps/cli dev` run. See the
  "First run on this machine" section of `docs/RUNBOOK.md`.
