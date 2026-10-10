#!/usr/bin/env bash
#
# Headless verification chain for this repository. Compiles and tests the core
# crate (no Tauri/system deps needed), then type-checks and builds the frontend.
# It deliberately does NOT touch `src-tauri` (missing webkit2gtk-4.1 here).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> cargo metadata (workspace resolves, no compile)"
cargo metadata --no-deps --format-version 1 >/dev/null

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo check -p orquestador-core"
cargo check -p orquestador-core

echo "==> cargo test -p orquestador-core"
cargo test -p orquestador-core

echo "==> cargo clippy -p orquestador-core --all-targets -- -D warnings"
cargo clippy -p orquestador-core --all-targets -- -D warnings

echo "==> stub audit (todo!/unimplemented! must be empty)"
if grep -rnE 'todo!\(|unimplemented!\(' crates src-tauri/src 2>/dev/null; then
  echo "FAIL: stubs found — implement them before shipping." >&2
  exit 1
fi

echo "==> frontend install + build"
cd "$ROOT/frontend"
npm install --no-audit --no-fund
npm run build

echo "OK — headless verification passed."
