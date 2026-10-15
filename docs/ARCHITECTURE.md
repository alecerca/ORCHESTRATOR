# Orquestador — Architecture

Orquestador is a Tauri 2 desktop control tower for a local `opencode serve`: the
headless Rust core (`orquestador-core`) owns all behavior, a thin Tauri shell
(`src-tauri`) bridges it over IPC, and a Vite + React webview renders it.

## Context

Orquestador drives a local `opencode serve` (default `http://127.0.0.1:4096`;
optional HTTP basic auth via `OPENCODE_SERVER_PASSWORD`, user `opencode`). It has
three layers with a hard boundary:

```
┌──────────────────────────── src-tauri (Tauri 2) ───────────────────────────┐
│  window "main" · AppBridge · 46 #[tauri::command]s · notify-rust            │
│  emits every CoreMessage to the webview as the event "core://message"       │
└───────────────▲───────────────────────────────────────────────┬────────────┘
        invoke()│ Tauri IPC                              emit() │ core://message
┌───────────────┴───────────────────────────────────────────────▼────────────┐
│  frontend — Vite + React 18 + TS strict + Monaco + useSyncExternalStore      │
└─────────────────────────────────────────────────────────────────────────────┘
                                ▲
┌───────────────────────────────┴─────────────────────────────────────────────┐
│  orquestador-core (headless; NO tauri/wry/webkit dependency)                 │
│                                                                              │
│   client.rs ──HTTP──▶ opencode serve      (typed reqwest, rustls)            │
│   events.rs  ──SSE──▶ GET /event          (hub: backoff, resync, bootstrap)  │
│        │                                                                     │
│        └──▶ state.rs  pure reducer  ──▶  CoreMessage broadcast               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Why the core is separate (the constraint that shapes everything)

`webkit2gtk-4.1` is now installed on this machine, but
`libayatana-appindicator3` is still absent and `sudo` still requires a password,
so the Tauri shell **has not been compiled in this environment**. The split
remains the right shape regardless: the core is a plain workspace member with
**zero** Tauri dependency and is verified headless (`cargo check`/`test`/`clippy`).
`src-tauri` is a standalone crate (empty `[workspace]` table) excluded from the
root workspace, so root `cargo` never tries to compile tauri/wry/webkit.

## Responsibilities

| Layer | Owns | Must never |
|---|---|---|
| `orquestador-core` | HTTP, SSE, serde types, session tree, status derivation, permission registry, notification *intent* (`Effect`), read-only git helpers, skill slugging/preamble, file-write helper | depend on Tauri; render UI; store messages/parts |
| `src-tauri` | Window, IPC commands (46), forwarding `CoreMessage`, showing notifications, server/proxy config persistence | hold business logic; build URLs |
| `frontend` | Rendering, local display cache, user gestures that call commands | be the source of truth; call the OpenCode server directly |

## Runtime sequence

1. Shell `setup` (`src-tauri/src/lib.rs`): read the persisted server config
   (`server.json`) or fall back to env/defaults → `AppBridge::new_with_config`
   → `subscribe()` the forwarder → `bootstrap()` (health, sessions, statuses,
   agents, pending permissions; create a root session if none) → `spawn_hub()`
   → `manage`.
2. The forwarder task sends every `CoreMessage` to the webview as
   `core://message`; each `Effect` also raises a desktop notification.
3. `EventHub::run()` loops until shutdown: connect `GET /event` →
   `set_connection(true)` → publish a snapshot → `resync()` → consume frames.
   Per frame it (a) broadcasts the raw event, (b) applies state-changing events
   to `AppState` and broadcasts a snapshot, (c) emits `Effect`s for
   finish/fail/limit/approval.
4. While connected the hub also reconciles statuses + pending permissions from
   plain HTTP every `RECONCILE_INTERVAL` (5 s), because SSE does not replay
   missed events, and reconnects if the stream stays silent for
   `STREAM_IDLE_TIMEOUT` (30 s) — a half-open/black-holed socket.
5. Webview: `invoke("get_state")` for the initial full state;
   `listen("core://message")` for live updates; all mutations go through
   commands.
6. On disconnect the hub backs off (`min(500ms·2^n, 30s)` + jitter < 250 ms,
   `attempt` capped at 6), resyncs on reconnect, and never crashes the app.

## Key decisions (rationale)

- **rustls over openssl** — `reqwest` with `default-features = false` +
  `rustls-tls`: no system TLS lib needed to build/test the core.
- **notify-rust in the shell, not core** — keeps the core headless; a missing
  D-Bus daemon only logs. Every `Effect` notifies, whether focused or not. No
  Tauri notification plugin.
- **`CoreMessage` envelope** — one event channel carries both high-frequency
  display data (`raw_event`) and debounced truth (`state_snapshot`).
- **Approvals render from the snapshot registry** — `ApprovalDialog` reads
  `AppStateSnapshot.permissions` (the core's pending registry) rather than only
  transient `effect` messages, so it is robust to approvals that arrive before
  the webview is listening. Responses are `once` / `always` / `reject`.
- **Sub-agent list from the snapshot** — `AgentPanel` derives the orchestrator
  and its sub-agents from the cached snapshot (bootstrap lists every session),
  which is inherently live; `list_children` stays bound and available. This
  avoids a redundant fetch loop for data already in the event stream.
- **No state framework** — `useSyncExternalStore` + a tiny hand-rolled store is
  enough and keeps the bundle small.
- **Monaco bundled locally** — offline, no CDN; workers via `?worker`.
- **Pane layout** wraps responsively (`flex: 1 1 340px`) instead of a fixed
  column count.
- **Theme** toggles via `data-theme` on `<html>` with matching CSS variables;
  persisted in `localStorage` and seeded from `prefers-color-scheme`.

### Skills (header panel)

The panel is **backend-aware**: it lists the connected backend's native skills **plus** the app's own.

- **App-owned storage**: app config dir (`app.path().app_config_dir()`) under `skills/<slug>.md`. Name = slugified filename stem; `save_skill` writes/overwrites, `delete_skill` removes. These are the only **editable** skills (`editable: true`, `source: "app"`).
- **Native (backend) skills**: read from the active backend (`AgentBackend::id()`), read-only (`editable: false`, `source` = backend id). Roots by backend: `opencode` → `~/.config/opencode/skills` + `~/.agents/skills`; `claude` → `~/.claude/skills`; `codex` → `~/.codex/skills`. A native skill is a directory `<root>/<name>/SKILL.md` (name = dir name) or, defensively, a file `<root>/<name>.md`. The app never mutates another tool's config.
- **Merge**: `list_skills` unions app + native, deduped by name (app shadows native; earlier root shadows later) and sorted.
- **Core**: `crates/orquestador-core/src/skills.rs` provides pure helpers `slugify`, `SkillInfo { name, path, source, editable }`, `merge_skills(app, native)`, and `build_preamble(&[(String, String)])` which emits a delimited `<skills>` block (empty content skipped). Unit-tested.
- **Shell**: Tauri commands `list_skills`/`read_skill`/`save_skill`/`delete_skill`. Both `send_message` and `send_message_async` take `app: AppHandle` + `skills: Option<Vec<String>>`; `apply_skills` resolves each selected name **app-first, then native roots**, and folds the content into `PromptBody.system` (appended; unreadable/missing skills are silently skipped). Never fails the send.
- **Frontend**: `skills.ts` (selection store persisted to `localStorage["orquestador.skills"]`, `useSyncExternalStore`-compatible), `skillDrafts.ts` (per-tab editor drafts with baseline-based dirtiness), `SkillEditor` + `SkillSavePrompt` components. TopBar renders the Skills dropdown (each item tagged with its `source`) + `+` (New/Import); the badge counts only **available** selections. Skill tabs (`kind: "skill"`) render in `PaneGrid`, are dropped on layout restore (`normalizeTab`), and closing a dirty skill tab prompts Save/Discard/Cancel (non-skill tabs unaffected). Composer/FollowUpInput pass selected skill names on every send; the Rust shell injects them as a hidden system block (no transcript pollution).

## Known limitations

Current, verified hardening gaps. `docs/AUDIT.md` carries the detail and evidence.

1. `security.csp` is `null` in `src-tauri/tauri.conf.json` — no defense-in-depth
   layer. Tightening it requires a running GUI to confirm Monaco workers still
   load; a workable policy would allow `'self'` plus `worker-src 'self' blob:`.
2. `useRawEvents` / `useLastMessage` in `frontend/src/store.ts` are unused, and
   `useRawEvents` builds a fresh array inside `getSnapshot`, violating the
   `useSyncExternalStore` caching contract.
3. The working-tree diff view (`frontend/src/components/DiffViewer.tsx`)
   refreshes by polling on an interval rather than reacting to incoming
   `session.diff` events, so the diff does not live-refresh from the event
   stream. (The git diff list, `DiffPanel.tsx`, likewise polls `git_status`.)
4. `AppBridge::bootstrap` runs via `block_on` inside the Tauri `setup`
   (`src-tauri/src/lib.rs`), so window creation waits for the initial health
   round-trip.
