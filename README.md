# Orquestador

Desktop control tower for [OpenCode](https://opencode.ai) sub-agents: a
composer for the orchestrator session, a live panel of sub-agents with status,
split panes with each agent's message stream, interrupt / re-instruct, approval
prompts with desktop notifications, and a Monaco editor with a diff viewer.

> **Status: implemented and audited.** The headless core is verified (39 unit
> tests), the frontend is complete and building, and an independent audit passed
> (`docs/AUDIT.md`). The Tauri shell is scaffolded and needs the system
> dependencies below to compile. See `docs/PLAN.md` (master contract),
> `docs/ARCHITECTURE.md`, `docs/AUDIT.md`, `docs/RUNBOOK.md` (first run),
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
docs/                      # PLAN.md · ARCHITECTURE.md · IMPLEMENTATION_PLAN.md
                           # IMPLEMENTATION_NOTES.md · AUDIT.md · RUNBOOK.md
```

New here? Follow **[`docs/RUNBOOK.md`](docs/RUNBOOK.md)** for the first run.
