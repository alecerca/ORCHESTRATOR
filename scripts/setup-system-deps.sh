#!/usr/bin/env bash
#
# One-time system dependencies for compiling/running the Tauri 2 shell.
#
# This script only PRINTS the command. Do NOT run it with elevated privileges
# from an agent: `sudo` needs a password and must be run by the user.
#
# Why: `webkit2gtk-4.1` and the ayatana appindicator are required by `wry`/
# `tauri`. They are missing on this machine, which is exactly why the whole
# application logic lives in the `orquestador-core` crate (no Tauri dep) and is
# verified headless. Only `src-tauri` needs these packages.
set -euo pipefail

cat <<'EOF'
Install these packages (Debian/Ubuntu) once, as your regular user with sudo:

  sudo apt-get update && sudo apt-get install -y \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf

Then verify the Tauri shell compiles / runs:

  cd src-tauri
  cargo build            # first compile is slow; proves the toolchain is ready
  # and from the repository root, the full dev app:
  npx @tauri-apps/cli dev

Notes:
  * `libssl-dev` is only needed for native TLS builds; the core crate already
    uses rustls, so it is optional in practice.
  * On Fedora/Arch the packages have different names (webkit2gtk4.1-devel,
    libappindicator-gtk3-devel, ...). Adjust accordingly.
  * If `tauri dev` cannot find the notification daemon, desktop notifications
    are skipped with a warning (see src-tauri/src/notify.rs); the app still runs.
EOF
