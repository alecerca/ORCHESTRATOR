# Orquestador — Architecture

> View over `docs/PLAN.md` (the single source of truth). Read `SPEC.md` first.
> This file exists because the feature document
> (`odd/tasks/orquestador-dashboard.md`) names it as a deliverable; the
> authoritative details live in `PLAN.md`.

## Context

Orquestador is a local desktop app that drives a local `opencode serve`
(`http://127.0.0.1:4096`). It has three layers with a hard boundary:

```
┌──────────────────────────── src-tauri (Tauri 2) ───────────────────────────┐
│  window "main" · AppBridge · 23 #[tauri::command]s · notify-rust            │
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

`webkit2gtk-4.1` and `libayatana-appindicator3-dev` are missing and `sudo` needs
a password, so the Tauri crate cannot compile on the dev machine. The core is a
plain workspace member with **zero** Tauri dependency and is verified headless
(`cargo check`/`test`/`clippy`). `src-tauri` is a standalone crate (empty
`[workspace]` table) excluded from the root workspace.

## Responsibilities

| Layer | Owns | Must never |
|---|---|---|
| `orquestador-core` | HTTP, SSE, serde types, session tree, status derivation, permission registry, notification *intent* (`Effect`) | depend on Tauri; render UI; store messages/parts |
| `src-tauri` | Window, IPC commands, forwarding `CoreMessage`, showing notifications | hold business logic; build URLs |
| `frontend` | Rendering, local display cache, user gestures that call commands | be the source of truth; call the OpenCode server directly |

## Runtime sequence

1. Shell `setup`: `AppBridge::new()` → subscribe → `bootstrap()` (health,
   sessions, statuses, agents; create root if none) → `spawn_hub()` → `manage`.
2. `EventHub` connects `GET /event`; on each frame it (a) broadcasts the raw
   event, (b) applies state-changing events to `AppState` and broadcasts a
   snapshot, (c) emits `Effect`s for finish/fail/approval.
3. Webview: `invoke("get_state")` for initial full state; `listen("core://message")`
   for live updates; all mutations go through commands.
4. On disconnect the hub backs off (`min(500ms·2^n, 30s)` + jitter), resyncs on
   reconnect, and never crashes the app.

## Key decisions (rationale)

- **rustls over openssl** — no system TLS lib needed to build/test the core.
- **notify-rust in the shell, not core** — keeps the core headless; a missing
  D-Bus daemon only logs. Suppress when focused except approvals.
- **`CoreMessage` envelope** — one event channel carries both high-frequency
  display data (`raw_event`) and debounced truth (`state_snapshot`).
- **No state framework** — `useSyncExternalStore` + a tiny store is enough and
  keeps the bundle small (SPEC §5 asks for "small").
- **Monaco bundled locally** — offline, no CDN; workers via `?worker`.
