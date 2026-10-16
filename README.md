# Orquestador

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](docs/INSTALL.md)
[![Build from source](https://img.shields.io/badge/build%20from%20source-Rust%20%2B%20Tauri-1687B8)](docs/INSTALL.md)

**The desktop control tower for OpenCode and [gentle-ai](https://github.com/Gentleman-Programming/gentle-ai) orchestrator sub-agents.**

You run an OpenCode-based orchestrator and suddenly **five sub-agents are
working at once**. In the terminal you cannot see what each one is doing, and
to fix a single one you interrupt everything. Orquestador is a desktop GUI
that puts **every sub-agent in its own live pane**: you watch all of them, fix
just one, and the rest keep working.

> What a terminal sidebar can't do: live split panes, per-agent control, a real
> diff viewer, and a visual handle on the whole project.

## Demo

A 3-minute, 1080p60 walkthrough of the app.

<div align="center">
  <video src="https://github.com/user-attachments/assets/97e7b4b1-be6f-407a-bffe-911d73a3f416" controls playsinline width="900"></video>
</div>

If you prefer Spanish, see here:

<div align="center">
  <video src="https://github.com/user-attachments/assets/2f828948-d58f-4cfd-8338-1ed5df10b8df" controls playsinline width="900"></video>
</div>

Both renders are also available as direct links, for anyone who cannot play the
inline embed: [English](https://github.com/user-attachments/assets/97e7b4b1-be6f-407a-bffe-911d73a3f416) · [Spanish](https://github.com/user-attachments/assets/2f828948-d58f-4cfd-8338-1ed5df10b8df) — 1920×1080, 60 fps, 3 min.


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
  whitelist, live presence, a pinned project. Setup:
  [`docs/COWORK-SETUP.md`](docs/COWORK-SETUP.md).
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

**Review and control agent work**

- Diff tab: see every file your agents touched — modified, added, deleted or
  untracked — with status badges, and review each change in a Monaco diff
  viewer (base `HEAD` vs your working tree) before it lands.
- Discard a single file's changes with one click, behind a confirm dialog —
  undo one agent's edit without touching the rest of the tree.
- Save with `Ctrl/Cmd+S`, and never lose an edit: closing a file with unsaved
  changes prompts Save / Don't save / Cancel, and the tab shows a dirty dot.

**Workspace**

- Built-in Monaco editor: review the files your agents are touching and edit
  them without leaving the app.
- File tree and a git view (branches, checkout, commit staging, log) — the
  project lives in the same window, including a branch switcher in the status
  bar.
- Tab system with splits: watch several agents at once, or an agent and a file
  side by side. Tab-strip scroll arrows keep many open tabs reachable.

**Skills**

- Skills panel backed by the connected agent's skill roots: run your OpenCode
  skills with a click, and create app-level skills from the UI.

**Cowork & remote**

- Shared sessions with group roles, whitelist, presence and a pinned project
  ([setup guide](docs/COWORK-SETUP.md)).
- Connect the app to any OpenCode server — local, LAN or remote — from the
  server settings, no terminal needed.

## Installation

Orquestador is a desktop app for **Windows, macOS and Linux**, and it ships a
companion CLI, **`orquestador-proxy`**. Both come from the same
[GitHub Release](https://github.com/alecerca/ORCHESTRATOR/releases). Full
details, prerequisites and troubleshooting: [`docs/INSTALL.md`](docs/INSTALL.md).

**Desktop GUI — installer from a release** — grab the installer for your
platform from the
[Releases page](https://github.com/alecerca/ORCHESTRATOR/releases):

- **Windows** — `.msi` or `.exe` (WebView2 is built into Windows 10/11).
- **macOS** — one universal `.dmg` (Intel and Apple Silicon).
- **Linux** — `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE) or the portable
  `.AppImage`.

**CLI (`orquestador-proxy`) — one-line install** — `orquestador-proxy` is the
headless binary: it serves the role-enforcing proxy and mints users
(`adduser`). It is published for Linux/macOS/Windows from the same release, so
you install it with one command:

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.sh | sh -s -- --cli
```

```powershell
# Windows (PowerShell)
iex "& { $(irm https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.ps1) } -Cli"
```

The Linux/macOS script installs under `${XDG_BIN_HOME:-~/.local/bin}`; the
Windows script installs under `%USERPROFILE%\.orquestador\bin` and prints the
line to add that directory to `PATH`. Run the same scripts without `--cli` / `-Cli` to install
the GUI bundle instead. Options, uninstall and troubleshooting:
[`docs/INSTALL.md`](docs/INSTALL.md).

**Build from source** — for hacking on the code or a platform without a
prebuilt bundle; see [`docs/INSTALL.md`](docs/INSTALL.md) for the per-OS
prerequisites and the exact commands.

## Quick start

The short version for a user (full detail in [Installation](#installation) and
[`docs/INSTALL.md`](docs/INSTALL.md)):

1. **Install the desktop app** — from the
   [Releases page](https://github.com/alecerca/ORCHESTRATOR/releases) or the
   one-line script above.
2. **Give it a server.** Orquestador is a client and does nothing on its own:
   run `opencode serve` (default `http://127.0.0.1:4096`), or point the app at a
   local, LAN or remote server from Settings, or with the
   `OPENCODE_SERVER_BASE_URL` and `OPENCODE_SERVER_PASSWORD` environment
   variables.
3. **Open Orquestador** and pick your project. For shared or remote work,
   install the CLI companion `orquestador-proxy` (same script with `--cli` on
   Linux/macOS or `-Cli` on Windows) and follow
   [`docs/COWORK-SETUP.md`](docs/COWORK-SETUP.md).

## How it works

- **`crates/orquestador-core`** — Rust core: typed HTTP client, SSE event hub,
  session/state model. No Tauri dependency, so it compiles and tests headless
  (132 unit tests).
- **`src-tauri/`** — the Tauri 2 desktop shell (standalone workspace).
- **`frontend/`** — Vite + React + TypeScript + Monaco (editor and diff),
  bundled locally.

Verify headless with `bash scripts/verify-headless.sh`. Full architecture:
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Documentation

| Doc | What it covers |
| --- | --- |
| [`docs/INSTALL.md`](docs/INSTALL.md) | Install for Windows / macOS / Linux |
| [`docs/LAUNCH.md`](docs/LAUNCH.md) | Launch kit: demo and where the project is shared |
| [`docs/COWORK-SETUP.md`](docs/COWORK-SETUP.md) | Step-by-step Cowork walkthrough: project → server → users → proxy → teammates → connect |
| [`docs/COWORK.md`](docs/COWORK.md) | Cowork reference: roles, whitelist, presence, config and limits |
| [`docs/RUNBOOK.md`](docs/RUNBOOK.md) | First run, day-to-day operations |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Core, shell and frontend design |
| [`docs/PLAN.md`](docs/PLAN.md) | Master contract and feature plan |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Future directions |
| [`docs/AUDIT.md`](docs/AUDIT.md) | Independent audit results |
| [`SPEC.md`](SPEC.md) | Canonical specification |

## Development

See [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md): feature branches,
Conventional Commits with scopes, headless verification, and the review flow.

## License

MIT — see [`LICENSE`](LICENSE).
