#!/usr/bin/env bash
#
# One-time system dependencies for compiling/running the Tauri 2 shell.
#
# This script only PRINTS the command. It never executes sudo: the elevated
# command must be run by the user (an agent is not allowed to use sudo).
#
# It detects the distro by which package manager is installed:
#   dnf  -> Fedora   (also RHEL/CentOS/Rocky/Alma)
#   apt-get -> Debian/Ubuntu family
#
# Why these packages: `webkit2gtk-4.1` and the AppIndicator are required by
# `wry`/`tauri`. They are missing on stock machines, which is exactly why the
# whole application logic lives in the `orquestador-core` crate (no Tauri dep)
# and is verified headless. Only `src-tauri` needs these system packages.
set -euo pipefail

if command -v dnf >/dev/null 2>&1; then
  distro="fedora"
elif command -v apt-get >/dev/null 2>&1; then
  distro="debian"
else
  distro="unknown"
fi

case "$distro" in
  fedora)
    cat <<'EOF'
Install these packages (Fedora) once, as your regular user with sudo:

  sudo dnf groupinstall "Development Tools"
  sudo dnf install -y \
    curl wget file openssl-devel gtk3-devel \
    webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf

Note: if `libappindicator-gtk3-devel` is not found on your Fedora release,
install `libayatana-appindicator-gtk3-devel` instead (both exist on Fedora 44).

Then verify the Tauri shell compiles / runs:

  cd src-tauri
  cargo build            # first compile is slow; proves the toolchain is ready
  # and from the repository root, the full dev app:
  npx @tauri-apps/cli dev

Notes:
  * `openssl-devel` is only needed for native TLS builds; the core crate uses
    rustls, so it is optional in practice.
  * If `tauri dev` cannot find the notification daemon, desktop notifications
    are skipped with a warning (see src-tauri/src/notify.rs); the app still runs.
EOF
    ;;
  debian)
    cat <<'EOF'
Install these packages (Debian/Ubuntu) once, as your regular user with sudo:

  sudo apt-get update && sudo apt-get install -y \
    build-essential curl wget file libssl-dev libgtk-3-dev \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf

Notes:
  * `libssl-dev` is only needed for native TLS builds; the core crate uses
    rustls, so it is optional in practice.
  * If `tauri dev` cannot find the notification daemon, desktop notifications
    are skipped with a warning (see src-tauri/src/notify.rs); the app still runs.
EOF
    ;;
  unknown)
    cat <<'EOF'
Could not detect a supported package manager (dnf or apt-get).

Install the Tauri v2 system dependencies for your distro yourself
(https://v2.tauri.app/start/prerequisites/): webkit2gtk-4.1 (devel), gtk3
(devel), AppIndicator (gtk3), librsvg (devel), patchelf, plus a C toolchain.

On Arch:       webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg patchelf
On openSUSE:   webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel
               librsvg-devel patchelf
EOF
    ;;
esac