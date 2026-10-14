# Installing Orquestador

Orquestador is a desktop app for **Windows, macOS and Linux**. It also ships a
headless CLI, **`orquestador-proxy`**. Choose one of the paths:

- **From a release** — fastest, no toolchain needed (available once the first
  release is published). Covers the GUI installers **and** the CLI binaries.
- **One-line script** — Linux/macOS (`install.sh`) and Windows
  (`install.ps1`); same release binaries, installed for you. Needs a published
  release. Add `--cli` (or `-Cli` on Windows) to install the CLI instead of the
  GUI.
- **Build from source** — the path that works today while releases are not
  published yet, or if you want to hack on the code.

## Install from a release

> **The first release is not published yet.** It will be built automatically
> by the release workflow (`.github/workflows/release.yml`) the first time a
> `v*` tag is pushed; the installers will appear on the GitHub
> [Releases page](https://github.com/alecerca/ORCHESTRATOR/releases).

Grab the installer for your platform from the GitHub **Releases** page.

| Platform | Artifact | Notes |
| --- | --- | --- |
| Windows 10/11 | `.msi` or `.exe` | WebView2 runtime is built into Windows 10/11. Unsigned builds trigger SmartScreen: *More info → Run anyway* |
| macOS | `.dmg` | Intel and Apple Silicon; right-click → *Open* on first launch to clear Gatekeeper |
| Linux | `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.AppImage` (any distro) | Make the AppImage executable with `chmod +x` |

No additional runtime needs to be installed: the app ships everything except
the OS webview it runs on.

Each release also carries the headless CLI, `orquestador-proxy`, as
architecture-specific binaries — `orquestador-proxy-linux-x86_64`,
`orquestador-proxy-macos-x86_64`, `orquestador-proxy-macos-aarch64` and
`orquestador-proxy-windows-x86_64.exe`. Download the one matching your OS and
architecture and put it on your `PATH`, or let a one-line script below do it.

## Install with the one-line script

Prerequisites: `curl` and a POSIX shell on Linux/macOS; Windows PowerShell on
Windows — nothing else. Like the release installers above, the scripts need a
**published release** to download from. Until the first release exists they
exit with a clear message pointing you back to this page.

### Linux / macOS

```sh
# GUI bundle: installs the AppImage under ~/.local/bin, or opens the .dmg
curl -fsSL https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.sh | sh
```

What it does, step by step (it prints each step as it goes):

1. Detects your OS (`Linux` or `Darwin`) and architecture (`x86_64` or
   `aarch64`); anything else exits with an error.
2. Asks the GitHub API for the latest release and picks the matching bundle by
   pattern — Linux: `*_amd64.AppImage` or `*_aarch64.AppImage` (`.deb`
   fallback), macOS: the `*_universal.dmg`. The architecture token is required
   on every platform, so a mismatched host fails with a clear message instead
   of installing the wrong architecture.
3. Installs:
   - **Linux** — the AppImage goes to `${XDG_BIN_HOME:-$HOME/.local/bin}/orquestador`
     (the directory is created if needed), and a desktop entry is written to
     `${XDG_DATA_HOME:-$HOME/.local/share}/applications/orquestador.desktop`
     so the app shows up in your application menu. If `~/.local/bin` is not
     on `PATH`, the script prints the line to add. With `--deb` (or when no
     AppImage is published), it installs the `.deb` via `dpkg` + a
     `apt-get -f install` fix-up — **only** when running as root or a
     non-interactive `sudo` is available; otherwise it prints the exact
     manual command instead of escalating privileges silently.
   - **macOS** — the `.dmg` is downloaded to `~/Downloads` and opened; drag
     Orquestador to *Applications* as usual.

Options:

```sh
sh install.sh --help            # usage
sh install.sh --version v0.1.0  # install a specific release tag
sh install.sh --cli             # install the CLI (orquestador-proxy), not the GUI
sh install.sh --deb             # Linux: prefer the .deb over the AppImage
```

(When piping via `curl`, pass options as `... | sh -s -- --cli`, or download
the file and run it.)

#### CLI install (Linux/macOS)

`--cli` downloads the `orquestador-proxy-<os>-<arch>` asset and installs it to
`${XDG_BIN_HOME:-$HOME/.local/bin}/orquestador-proxy` (no GUI bundle):

```sh
curl -fsSL https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.sh | sh -s -- --cli
```

### Windows (PowerShell)

The Windows installer is `scripts/install.ps1`. By default it downloads the
`.msi` (falling back to the `-setup.exe`), starts it, and lets the UAC prompt
install the GUI:

```powershell
iex "& { $(irm https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.ps1) }"
```

With `-Cli` it downloads `orquestador-proxy-windows-x86_64.exe` to
`%USERPROFILE%\.orquestador\bin\orquestador-proxy.exe` and prints the line to
add that directory to `PATH`:

```powershell
iex "& { $(irm https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.ps1) } -Cli"
```

Both forms accept `-Version v0.1.0` to install a specific release tag. Like the
other paths, the script needs a published release and points at
[build from source](#build-from-source) when none exists yet.

### Using the CLI (`orquestador-proxy`)

`orquestador-proxy` is the headless binary shipped next to the GUI. It has two
modes:

```sh
orquestador-proxy ./proxy.json                           # serve the role-enforcing proxy (default)
orquestador-proxy adduser <name> --role <editor|viewer>  # add a user (password read from stdin)
```

`serve` loads `proxy.json` (the path argument, or `ORQUESTADOR_PROXY_CONFIG`,
or `./proxy.json`) and runs the reverse proxy. `adduser` appends an
Argon2id-hashed user to `proxy.json` (use `--config <path>` to point elsewhere).
See [`COWORK.md`](COWORK.md) for the full role-enforcement setup.

### Uninstall

- **Linux (AppImage)** — delete the installed binary and the desktop entry:

  ```sh
  rm ~/.local/bin/orquestador
  rm "${XDG_DATA_HOME:-$HOME/.local/share}/applications/orquestador.desktop"
  ```

- **Linux (.deb)** — `sudo dpkg -r orquestador`.
- **macOS** — delete `Orquestador.app` from *Applications* and the `.dmg` in
  *Downloads*.
- **CLI (Linux/macOS)** — `rm ~/.local/bin/orquestador-proxy`.
- **CLI (Windows)** — delete `%USERPROFILE%\.orquestador\bin\orquestador-proxy.exe`
  and remove that directory from `PATH` if you added it.

## Build from source

### Prerequisites

| OS | Requirements |
| --- | --- |
| **All** | [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 20+ and npm 11+ |
| **Linux** | Tauri system deps: `webkit2gtk-4.1`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, plus `libssl-dev`, `build-essential`, `curl`, `wget`, `file`. Run `bash scripts/setup-system-deps.sh` — it prints the exact `apt` command for Debian/Ubuntu (Fedora/Arch equivalents: same packages under `webkit2gtk4.1`, `libayatana-appindicator`, `librsvg2`, `patchelf`) |
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

The bundled installers land in `src-tauri/target/release/bundle/` — **for the
OS the build runs on only** (Tauri does not cross-compile; the release
workflow builds each platform on its own runner):

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

- **The one-line script says there is no release yet** — the first release has
  not been published; there is nothing to download. Install
  [from source](#build-from-source) for now, or grab a release from the
  [Releases page](https://github.com/alecerca/ORCHESTRATOR/releases) once one
  is published.
- **The one-line script fails with a rate-limit message** — the GitHub API is
  throttling unauthenticated requests from your IP. Wait a bit and retry, or
  install [from a release](#install-from-a-release) / [from
  source](#build-from-source) manually.
- **Linux: the app window never opens / blank window** — the webkit2gtk-4.1
  library is missing or too old; re-run `scripts/setup-system-deps.sh`.
- **Linux: AppImage does not start** — some distros need `libfuse2` for older
  AppImage formats, or use `--appimage-extract-and-run`.
- **macOS: "unidentified developer"** — right-click the app → *Open* once;
  Gatekeeper remembers the exception.
- **Windows: SmartScreen warning** — unsigned builds trigger it; *More
  info → Run anyway*.
- **The app runs but shows no sessions** — the OpenCode server is not
  reachable at the configured URL, or the password is wrong. Check the server
  status in-app and `curl http://<url>/` from a terminal.
