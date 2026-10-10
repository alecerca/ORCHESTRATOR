# ORQUESTADOR — Canonical Project Spec (v1)

> Written by the orchestrator (2026-10-10). Every agent in the 3-agent pipeline reads THIS file first and treats it as the source of truth for product and architecture. Do NOT re-litigate decisions. Extend only where the spec leaves an explicit TODO (marked `[PLANNER]`, `[IMPLEMENTER]`, `[AUDITOR]`).

## 1. Product (what we build)

A desktop control tower for OpenCode sub-agents, provisionally named **orquestador**. Third-party inspiration: Orca — but WITHOUT worktree/issue complexity. What the user wants:

- One window with:
  1. **Orchestrator composer**: a terminal-style input where the user gives instructions to the orchestrator agent ("create 5 subagents, one does X..."). Visually a chat/terminal input that POSTs to `/session/:id/message`.
  2. **Agent panel**: all sub-agents spawned from the current session, each with: task (from the message that spawned it), live status (running / done / error / waiting-approval), and a DETAIL view (per-agent pane).
  3. **Split panes**: open several sub-agent panes side by side (split layout), each showing that agent's live message stream.
  4. **Interrupt / re-instruct**: from any agent pane, the user can `abort` the agent or send it a follow-up instruction.
  5. **Approval prompts**: when a sub-agent needs human approval (permission request), the app MUST surface it and let the user approve/deny (`POST /session/:id/permissions/:permissionID`), plus a desktop notification.
  6. **Editor + diff viewer**: file tree + code editor (Monaco) + a way to VIEW DIFFS of a session (`GET /session/:id/diff` → `FileDiff[]`). No git integration needed — the OpenCode server computes diffs.
  7. **Notifications**: desktop/system notifications when a sub-agent finishes, errors, or requests approval.
- **Explicitly OUT of scope (v1)**: worktree management, GitHub issues/PRs, multi-project switching, auth beyond basic-auth optional, chat history persistence.

### Who uses it
Solo developer, local machine, Linux (this env). Talks to a local `opencode serve` (default `http://127.0.0.1:4096`).

## 2. Fixed stack (decided — do not change)

- **Backend**: Rust, **Tauri 2** for the desktop shell.
- **Frontend**: TypeScript in the Tauri webview (Vite). Editor: **Monaco Editor**. Terminals/panes rendering: app UI (Monaco is the editor); the per-agent "panes" are live message streams rendered as UI, NOT raw `xterm` PTYs in v1 ([PLANNER] may add xterm later as a stretch; default = rendered stream).
- **HTTP + SSE**: Rust client for the OpenCode server API (`reqwest` + `tokio`; SSE via `eventsource-stream` or manual line parsing). Frontend talks to Rust via Tauri commands (no CORS pain).
- **Desktop notifications**: Tauri notification plugin or system `notify-rust`; decide in plan.
- **Language of all artifacts, code, comments, docs**: English.

## 3. Environment facts (this machine — verification must respect these)

- Rust 1.99, Node 24.21, npm 11.19 available.
- `openssl` dev libs: check. HTTP client must compile (`reqwest` features: `rustls-tls` preferred to avoid openssl system dep if possible; verify with `cargo check`).
- **`webkit2gtk-4.1` + appindicator NOT installed, sudo needs password** → **`cargo check/build` of the Tauri app crate FAILS here.** Consequences:
  - The Rust core (API client, session model, event hub) MUST live in a plain workspace member crate **without** `tauri`/`wry` deps, so it is fully `cargo test`-able headless. Name it `orquestador-core` (or better name per planner).
  - The Tauri shell crate is a SEPARATE crate (NOT a workspace default member) that depends on the core via `path`. Root `cargo check`/`cargo test` must succeed WITHOUT tauri deps compiled. Structure: workspace `members = ["./crates/*"]`; `src-tauri` crate excluded from the workspace (standalone Cargo.toml with its own `[workspace]` empty table) — verify this works.
  - Deliver a `scripts/setup-system-deps.sh` that prints/exports the apt command the user must run once (`libwebkit2gtk-4.1-dev` etc.) — do NOT run sudo yourselves.
- Verification chain (document in README): `cargo test` (workspace, headless) → frontend `npm run build` → optional `tauri build` (blocked until system deps; mark clearly).

## 4. OpenCode server API (source of truth for the client)

Base: `http://127.0.0.1:4096` (configurable). Optional basic auth via env `OPENCODE_SERVER_PASSWORD` (user: `opencode`).

- `GET /global/health` → `{ healthy, version }`
- `GET /project` · `GET /project/current`
- `GET /session` (list) · `POST /session` (`{ parentID?, title? }`) → Session
- `GET /session/status` → `{ [sessionID]: SessionStatus }`
- `GET /session/:id` · `DELETE /session/:id` · `PATCH /session/:id` (`{ title? }`)
- `GET /session/:id/children` → **sub-agents** (Session[])
- `GET /session/:id/todo` → Todo[]
- `POST /session/:id/abort` → **interrupt** (boolean)
- `POST /session/:id/message` body `{ messageID?, model?, agent?, noReply?, system?, tools?, parts }` → `{ info: Message, parts: Part[] }` (send instruction; waits for full reply)
- `POST /session/:id/prompt_async` (same body) → 204 (no wait)
- `GET /session/:id/message?limit=` → `{ info, parts }[]` (history)
- `GET /session/:id/message/:messageID`
- `POST /session/:id/command` (`{ command, arguments? }`)
- `POST /session/:id/permissions/:permissionID` body `{ response, remember? }` → **approval answers**
- `GET /session/:id/diff?messageID=` → `FileDiff[]` → **diff viewer data**
- `GET /file?path=` → FileNode[] · `GET /file/content?path=` → FileContent · `GET /file/status` → File[]
- `GET /find?pattern=` · `GET /find/file?query=` · `GET /find/symbol?query=`
- `GET /event` → **SSE stream, first event `server.connected`, then bus events** (this drives live updates)
- `GET /agent` → Agent[]
- `GET /doc` → OpenAPI spec (client could be generated from it; hand-written typed client is acceptable for v1)

Session types (`Session`, `Message`, `Part`, `SessionStatus`, `FileDiff`, `FileNode`, `FileContent`) — see `/doc` for exact schemas; the planner must extract the minimal set of types needed and write them into the core client.

## 5. Architecture contract (planner refines, implementer follows)

- **`orquestador-core`** (pure Rust, async tokio):
  - `client` module: typed API client (reqwest, rustls-tls), all endpoints above.
  - `events` module: SSE subscription, typed event models, reconnection with backoff.
  - `state` module: session tree (session → children), status derivation per agent (running/done/error/waiting-approval), permission-request registry.
- **UI (frontend/TypeScript)**:
  - Session/agent panel (tree of sessions, per-agent status chip).
  - Composer (orchestrator input).
  - Agent detail pane(s) with live stream + abort/follow-up buttons + approval dialog.
  - File tree + Monaco editor + diff panel.
  - Notification integration (Tauri `notification` command bridge).
  - State: plain stores (avoid heavy framework bloat; choice per planner — React is acceptable, vanilla/Preact fine too; keep it SMALL).
- **Tauri shell**: menu-less minimal window, resize, system tray optional. Commands bridge frontend ↔ `orquestador-core`.

## 6. Quality gates (each agent must run what applies and REPORT results honestly)

- `cargo fmt --check` (or run fmt) · `cargo clippy -- -D warnings` on core (or document why warnings allowed) · `cargo test` workspace.
- Frontend: `npm run build` (vite, tsc strict) — no runtime test infra required v1.
- Manual smoke impossible until system deps installed → AUDITOR must state explicitly what was verified headless vs what needs the user's `tauri dev` run (include a `RUNBOOK.md` section "First run on this machine").

## 7. Agent handoff contract (this pipeline)

- **Agent 1 (PLANNER)**: reads spec → produces `docs/PLAN.md` (architecture, module breakdown, exact API type shapes, Tauri command list, frontend component tree, ordered implementation steps with dependencies, verification commands, risks) + scaffolds the repo skeleton (Cargo workspace manifests, package.json, vite config, directory tree, .gitignore, README stub, scripts/setup-system-deps.sh) + `git init` (no commits needed from agents; orchestrator commits).
- **Agent 2 (IMPLEMENTER)**: reads `docs/PLAN.md` → implements everything → `cargo test` + `npm run build` must pass → writes `docs/IMPLEMENTATION_NOTES.md` (what was built, deviations, how to run, verification evidence).
- **Agent 3 (AUDITOR)**: reads spec + plan + code → adversarial review (security, correctness, UI consistency, spec compliance) → runs full verification headless → fixes ONLY clearly-bounded defects (small, safe fixes; otherwise document as findings) → writes `docs/AUDIT.md` (findings, evidence path:line, resolutions, what remains blocked by environment) + finalizes `README.md`/`RUNBOOK`.
- All agents: report results in a final structured summary. Files live in the project dir. NO commits by agents.

## 8. Decisions log

- 2026-10-10: Tauri approved by user (Monaco + stream UI, no worktree/issue features). Core/headless split mandated by missing webkit2gtk. Pipeline: planner → implementer → auditor, autonomous ("NO NECESITAS NADA DE MI").