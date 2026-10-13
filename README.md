# Orquestador

**The desktop control tower for orchestrator sub-agents.**

You run [gentle-ai](https://github.com/Gentleman-Programming/gentle-ai) (or any
OpenCode orchestrator) and suddenly five sub-agents are working at once. You
can't see what each one is doing, and to fix a single one you have to interrupt
everything.

Orquestador gives you back control: **every sub-agent live in its own pane**, a
message aimed at *just that one*, and a stop that leaves the rest running — plus
a Monaco editor with a diff viewer and desktop approval notifications. A desktop
GUI, not the terminal.

> What a terminal sidebar can't do: live split panes, per-agent control, and a
> real diff viewer.

## Features

- Live pane per sub-agent, with status and a timer.
- Interrupt / re-instruct a single agent without touching the others.
- Composer for the orchestrator session, with approval prompts and desktop
  notifications.
- Monaco editor with a diff viewer.
- Backend-aware skills panel (reads the connected agent's skill roots).

See [`docs/LAUNCH.md`](docs/LAUNCH.md) for the 90-second demo and where the
project is shared, and [`docs/COWORK.md`](docs/COWORK.md) to run the app for a
group — roles, whitelist, presence and the pinned project.

> **Status: implemented and audited.** The headless core is verified (77 unit
> tests), the frontend is complete and building, and an independent audit passed
> (`docs/AUDIT.md`). The Tauri shell is scaffolded and needs the system
> dependencies below to compile. See `docs/PLAN.md` (master contract),
> `docs/ARCHITECTURE.md`, `docs/ROADMAP.md` (future directions), `docs/AUDIT.md`,
> `docs/RUNBOOK.md` (first run), `docs/COWORK.md` (shared project),
> `docs/IMPLEMENTATION_NOTES.md`, and the canonical `SPEC.md`.

## Stack

- **Rust + Tauri 2** desktop shell (`src-tauri/`).
- **Rust core** `crates/orquestador-core` — typed HTTP client + SSE event hub +
  session/state model. **No Tauri dependency** so it compiles and tests headless.
- **TypeScript + Vite + React** frontend (`frontend/`) with **Monaco** (editor +
  diff) bundled locally.

## Requirements

- Rust 1.99+, Node 24+, npm 11+.
- A local OpenCode server: `opencode serve` (default `http://127.0.0.1:4096`).
- **To run the GUI only**: the Tauri system dependencies. Print the exact
  apt command with `scripts/setup-system-deps.sh` (run it as your user; an agent
  will not use `sudo`). They are **not** needed to build/test the core.

## Verify (headless — no system deps needed)

```sh
bash scripts/verify-headless.sh
```

It runs: `cargo metadata` (workspace), `cargo fmt --check`, `cargo check -p
orquestador-core`, `cargo test -p orquestador-core`, `cargo clippy
-p orquestador-core -- -D warnings`, then `npm install && npm run build` in
`frontend/`, plus the `todo!`/`unimplemented!` audit.

### Individually

```sh
cargo test -p orquestador-core          # core unit tests (client, SSE, state)
cargo clippy -p orquestador-core --all-targets -- -D warnings
cd frontend && npm install && npm run build
(cd src-tauri && cargo metadata --format-version 1 >/dev/null)   # manifest only
```

## Run the app (after installing system deps)

```sh
bash scripts/setup-system-deps.sh     # prints the sudo apt-get command
npx @tauri-apps/cli dev               # dev window + Vite on :1420
```

Configuration via environment: `OPENCODE_SERVER_BASE_URL`,
`OPENCODE_SERVER_PASSWORD` (optional HTTP basic auth, user `opencode`).

## Layout

```
crates/orquestador-core/   # all logic (headless, tested)
src-tauri/                 # Tauri shell (standalone workspace, needs webkit2gtk)
frontend/                  # Vite + React + TS + Monaco
scripts/                   # setup-system-deps.sh · install-deps.sh · verify-headless.sh
docs/                      # PLAN.md · ARCHITECTURE.md · ROADMAP.md · IMPLEMENTATION_PLAN.md
                           # IMPLEMENTATION_NOTES.md · AUDIT.md · RUNBOOK.md
```

New here? Follow **[`docs/RUNBOOK.md`](docs/RUNBOOK.md)** for the first run.
