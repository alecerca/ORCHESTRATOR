# RUNBOOK — Orquestador first run

> Operational guide to verify, launch, and troubleshoot Orquestador.
> For the architecture see `docs/ARCHITECTURE.md`; for the audit see
> `docs/AUDIT.md`. All commands assume the repository root unless noted.

## 0. Prerequisites

- Rust `1.99+`, Node `24+`, npm `11+`.
- A running OpenCode server: `opencode serve` (default `http://127.0.0.1:4096`).
- **GUI only**: the Tauri system libraries (see step 3). They are **not** needed
  to build or test the core.

## 1. Verify the project headless (no system deps)

```sh
bash scripts/verify-headless.sh
```

Expected tail: `OK — headless verification passed.` This runs the workspace
metadata check, `cargo fmt --check`, `cargo check`/`test`/`clippy` on
`orquestador-core`, the `todo!`/`unimplemented!` audit, then `npm install &&
npm run build` in `frontend/`. It deliberately skips `src-tauri`.

Run the pieces individually if you prefer:

```sh
cargo test -p orquestador-core
cargo clippy --workspace --all-targets -- -D warnings
(cd frontend && npm install && npm run typecheck && npm run build)
```

## 2. Start the OpenCode server

Orquestador is a client — it does nothing without a server:

```sh
opencode serve            # listens on http://127.0.0.1:4096
```

If your server uses HTTP basic auth, note the password for step 4.

## 3. Install the system dependencies (once, GUI only)

The `src-tauri` crate needs `webkit2gtk-4.1` and an ayatana appindicator. Print
the exact command and run it yourself (an agent will not use `sudo`):

```sh
bash scripts/setup-system-deps.sh      # prints the apt-get line
# then, as your user:
sudo apt-get update && sudo apt-get install -y \
  build-essential curl wget file libssl-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

First compile of the shell is slow:

```sh
(cd src-tauri && cargo build)
```

## 4. Configure (optional)

| Variable | Purpose | Default |
|---|---|---|
| `OPENCODE_SERVER_BASE_URL` | Server base URL | `http://127.0.0.1:4096` |
| `OPENCODE_SERVER_PASSWORD` | Enables HTTP basic auth (user `opencode`) | unset |
| `OPENCODE_SERVER_USERNAME` | Basic-auth user | `opencode` |

`OPENCODE_SERVER_URL` is still accepted as a legacy alias for the base URL.

> The password is read from the environment at runtime, kept only in memory,
> and never sent to the webview. Do not put it in a committed `.env`.

## 5. Run the app

```sh
npx @tauri-apps/cli dev      # dev window + Vite on :1420
```

First-run smoke test:

1. The top bar shows the connection status; a green/connected state means the
   SSE stream to the server is live.
2. Send an instruction from the composer → it goes to the root session via
   `prompt_async` and appears in the orchestrator stream.
3. When a sub-agent needs approval, the Approval dialog appears and (if a
   notification daemon is present) a desktop notification fires.
4. Open the editor to browse the file tree and view a session diff.

## 6. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `tauri dev` fails building `src-tauri` (webkit2gtk/appindicator) | System deps missing | Run step 3. |
| Top bar stays disconnected | No server, or wrong URL/auth | Start `opencode serve`; check `OPENCODE_SERVER_BASE_URL`/`OPENCODE_SERVER_PASSWORD` (step 4). |
| No desktop notifications | No notification daemon | Harmless — the app logs a warning and continues (`src-tauri/src/notify.rs`); in-app effects still work. |
| Composer is disabled | The root session is unknown | Wait for the initial connect / press reconnect; the composer enables once the root session is resolved. |
| `npm run build` warns about a >500 kB chunk | Monaco is large by design | Expected; not an error. |

## 7. Re-verify after changes

After any code change, re-run step 1. Structural changes to `src-tauri` require
step 3's toolchain and a `tauri dev` smoke test.
