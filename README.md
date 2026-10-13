# Orquestador

**The desktop control tower for OpenCode and [gentle-ai](https://github.com/Gentleman-Programming/gentle-ai) orchestrator sub-agents.**

You run an OpenCode-based orchestrator and suddenly **five sub-agents are
working at once**. In the terminal you cannot see what each one is doing, and
to fix a single one you interrupt everything. Orquestador is a desktop GUI
that puts **every sub-agent in its own live pane**: you watch all of them, fix
just one, and the rest keep working.

![Orquestador — main window](docs/screenshots/main-window.png)

> What a terminal sidebar can't do: live split panes, per-agent control, a real
> diff viewer, and a visual handle on the whole project.

## Why

Orquestador started as a personal convenience tool. Driving
[gentle-ai](https://github.com/Gentleman-Programming/gentle-ai) — OpenCode as
the orchestrator — from the terminal is powerful but rustic: the output of five
sub-agents scrolls past, there is no per-agent control, and even "which branch
is this agent on?" means another command. So I built a **control tower**: a
desktop window that shows the orchestrator session and every sub-agent, lets me
stop one, re-point it, rename it, and see the branch it is working on — and
that centralises the project (directory, files, git) in one place.

Two things turned it from a personal tool into something shareable:

- **Cowork** — shared orchestrator sessions for a small group: roles, a
  whitelist, live presence, a pinned project. Docs: [`docs/COWORK.md`](docs/COWORK.md).
- **Bring your own server** — the app connects to *your* OpenCode server. It
  does not need to be a process the app started: it can be a box on your LAN or
  a remote machine. Your agents, your machines, one control tower.

## Features

**Agent orchestration**

- Live pane per sub-agent, with status and a timer.
- Interrupt / re-instruct a single agent without touching the others.
- Rename an agent mid-flight; every agent shows the git branch it is working in.
- Composer for the orchestrator session, with approval prompts and desktop
  notifications.

**Workspace**

- Built-in Monaco editor with a diff viewer: review the files your agents are
  touching and edit them without leaving the app.
- File tree and a git view (branches, checkout, commit staging, log) — the
  project lives in the same window.
- Tab system with splits: watch several agents at once, or an agent and a file
  side by side.

**Skills**

- Skills panel backed by the connected agent's skill roots: run your OpenCode
  skills with a click, and create app-level skills from the UI.

**Cowork & remote**

- Shared sessions with group roles, whitelist, presence and a pinned project.
- Connect the app to any OpenCode server — local, LAN or remote — from the
  server settings, no terminal needed.

## Screenshots

Real captures are pending (this machine's display is not reachable from a shell;
see [`docs/screenshots/README.md`](docs/screenshots/README.md) for the capture
list). The README embeds them at:

- `docs/screenshots/main-window.png` — the control tower.
- `docs/screenshots/agent-panes.png` — per-agent panes with branch chips.
- `docs/screenshots/workspace.png` — editor, diff and git view.

## Installation

Orquestador is a desktop app for **Windows, macOS and Linux**.

**From a release** (once the first release is published): grab the installer
for your platform —

- **Windows** — `.msi` or `.exe` installer (WebView2 is built into
  Windows 10/11).
- **macOS** — `.dmg` (Intel and Apple Silicon).
- **Linux** — `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE) or the portable
  `.AppImage`.

**Build from source** — see [`docs/INSTALL.md`](docs/INSTALL.md) for the
per-OS prerequisites and the exact commands.

## Quick start

1. Make sure an OpenCode server is reachable: `opencode serve` (default
   `http://127.0.0.1:4096`). Point the app at your own server — local, LAN or
   remote — via the in-app server settings, or the `OPENCODE_SERVER_BASE_URL`
   and `OPENCODE_SERVER_PASSWORD` environment variables.
2. Install the desktop system dependencies — on Linux,
   `bash scripts/setup-system-deps.sh` prints the exact command; see
   [`docs/INSTALL.md`](docs/INSTALL.md) for Windows and macOS.
3. Run the app:

   ```sh
   npx @tauri-apps/cli dev
   ```

## How it works

- **`crates/orquestador-core`** — Rust core: typed HTTP client, SSE event hub,
  session/state model. No Tauri dependency, so it compiles and tests headless
  (77 unit tests).
- **`src-tauri/`** — the Tauri 2 desktop shell (standalone workspace).
- **`frontend/`** — Vite + React + TypeScript + Monaco (editor and diff),
  bundled locally.

Verify headless with `bash scripts/verify-headless.sh`. Full architecture:
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Documentation

| Doc | What it covers |
| --- | --- |
| [`docs/INSTALL.md`](docs/INSTALL.md) | Generic install for Windows / macOS / Linux |
| [`docs/LAUNCH.md`](docs/LAUNCH.md) | 90-second demo and where the project is shared |
| [`docs/COWORK.md`](docs/COWORK.md) | Shared sessions: roles, whitelist, presence |
| [`docs/RUNBOOK.md`](docs/RUNBOOK.md) | First run, day-to-day operations |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Core, shell and frontend design |
| [`docs/PLAN.md`](docs/PLAN.md) | Master contract and feature plan |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Future directions |
| [`docs/AUDIT.md`](docs/AUDIT.md) | Independent audit results |
| `SPEC.md` | Canonical specification |

## Development

See [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md): feature branches,
Conventional Commits with scopes, headless verification, and the review flow.

## License

MIT — see [`LICENSE`](LICENSE).