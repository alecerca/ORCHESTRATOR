# Installing Orquestador

Orquestador is a desktop app for **Windows, macOS and Linux**. Choose one of
the two paths:

- **From a release** — fastest, no toolchain needed (available once the first
  release is published).
- **Build from source** — current path while releases are not published yet,
  or if you want to hack on the code.

## Install from a release

Grab the installer for your platform from the GitHub **Releases** page.

| Platform | Artifact | Notes |
| --- | --- | --- |
| Windows 10/11 | `.msi` or `.exe` | WebView2 runtime is built into Windows 10/11 |
| macOS | `.dmg` | Intel and Apple Silicon; right-click → *Open* on first launch if Gatekeeper complains |
| Linux | `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.AppImage` (any distro) | Make the AppImage executable with `chmod +x` |

No additional runtime needs to be installed: the app ships everything except
the OS webview it runs on.

## Build from source

### Prerequisites

| OS | Requirements |
| --- | --- |
| **All** | [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 20+ and npm 11+ |
| **Linux** | Tauri system deps: `webkit2gtk-4.1`, `libappindicator`/`libayatana-appindicator`, `librsvg`, `patchelf`. Run `bash scripts/setup-system-deps.sh` — it prints the exact `apt` command for Debian/Ubuntu (Fedora/Arch equivalents: same packages under `webkit2gtk4.1`, `libayatana-appindicator`, `librsvg2`, `patchelf`) |
| **Windows** | [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 10/11) and the Microsoft C++ Build Tools (MSVC) — install via Visual Studio Build Tools with the "Desktop development with C++" workload |
| **macOS** | Xcode Command Line Tools (`xcode-select --install`) |

> The headless Rust core (`crates/orquestador-core`) does **not** need the
> Tauri system deps — those are only required to compile/run the desktop shell.

### Build

```sh
git clone https://github.com/alecerca/ORCHESTRATOR.git
cd ORCHESTRATOR
npm install --prefix frontend
npx @tauri-apps/cli build
```

The bundled installers land in `src-tauri/target/release/bundle/`:

- Linux: `deb/`, `rpm/`, `appimage/`
- Windows: `msi/`, `nsis/`
- macOS: `dmg/`, `macos/`

### Run in development

```sh
npm install --prefix frontend
npx @tauri-apps/cli dev
```

Opens the app window with Vite serving the frontend on `:1420`.

## Configuration

The app talks to an OpenCode server.

| Setting | Default | How to change it |
| --- | --- | --- |
| Server URL | `http://127.0.0.1:4096` | In-app server settings, or env `OPENCODE_SERVER_BASE_URL` |
| Server password | none | Env `OPENCODE_SERVER_PASSWORD` (HTTP basic auth, user `opencode`) |

The URL may point anywhere: localhost, another machine on the LAN, or a remote
server. Start a local server with `opencode serve`.

## Headless verification (no system deps needed)

```sh
bash scripts/verify-headless.sh
```

Runs the workspace checks (fmt, check, tests, clippy) and the frontend build
without touching the desktop shell.

## Troubleshooting

- **Linux: the app window never opens / blank window** — the webkit2gtk-4.1
  library is missing or too old; re-run `scripts/setup-system-deps.sh`.
- **Linux: AppImage does not start** — some distros need `libfuse2` for older
  AppImage formats, or use `--appimage-extract-and-run`.
- **macOS: "unidentified developer"** — right-click the app → *Open* once;
  Gatekeeper remembers the exception.
- **Windows: SmartScreen warning** — unsigned dev builds trigger it; *More
  info → Run anyway*.
- **The app runs but shows no sessions** — the OpenCode server is not
  reachable at the configured URL, or the password is wrong. Check the server
  status in-app and `curl http://<url>/` from a terminal.