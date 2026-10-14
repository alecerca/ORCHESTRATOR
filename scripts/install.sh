#!/bin/sh
#
# Orquestador one-line installer (Linux/macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.sh | sh
#
# Detects your OS and architecture, downloads the matching bundle from the
# latest GitHub release, and installs it:
#   Linux   AppImage -> ${XDG_BIN_HOME:-$HOME/.local/bin}/orquestador
#                     (+ a .desktop entry under ${XDG_DATA_HOME:-$HOME/.local/share})
#           .deb     -> dpkg (only when root or non-interactive sudo is available)
#   macOS   .dmg     -> ~/Downloads, then opens it (drag to Applications)
#
# With --cli it installs the headless `orquestador-proxy` binary instead:
#   orquestador-proxy -> ${XDG_BIN_HOME:-$HOME/.local/bin}/orquestador-proxy
#
# Needs a published release: https://github.com/alecerca/ORCHESTRATOR/releases
# Full docs: docs/INSTALL.md
set -eu

REPO="alecerca/ORCHESTRATOR"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"

VERSION=""
USE_DEB=0
USE_CLI=0

info() { printf 'orquestador-install: %s\n' "$*" >&2; }
die()  { printf 'orquestador-install: error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Orquestador installer (Linux/macOS)

Usage:
  curl -fsSL https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.sh | sh
  sh install.sh [options]

Options:
  --version <tag>   Install a specific release tag (default: latest),
                    e.g. --version v0.1.0
  --cli             Install the CLI (orquestador-proxy) instead of the GUI
  --deb             Linux: install the .deb instead of the AppImage
  -h, --help        Show this help

What it does:
  Detects OS/arch, downloads the matching bundle from the latest GitHub
  release, and installs it:
    Linux   AppImage -> ~/.local/bin/orquestador (+ a .desktop menu entry);
            .deb     -> dpkg, only with root or non-interactive sudo
    macOS   .dmg     -> ~/Downloads, then opens it (drag to Applications)
  With --cli it installs the headless binary instead, to
    ~/.local/bin/orquestador-proxy.

This script needs a published release to download from:
  https://github.com/alecerca/ORCHESTRATOR/releases
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || die "--version needs a tag, e.g. --version v0.1.0"
      VERSION="$2"
      shift 2
      ;;
    --deb)
      USE_DEB=1
      shift
      ;;
    --cli)
      USE_CLI=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "unknown option: $1"
      ;;
  esac
done

# --- Detect OS and architecture ----------------------------------------------

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Linux)   os=linux ;;
  Darwin)  os=macos ;;
  *)       die "unsupported operating system '$os' (supported: Linux, macOS)" ;;
esac

case "$arch" in
  x86_64|amd64)  arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *)             die "unsupported architecture '$arch' (supported: x86_64, aarch64)" ;;
esac
info "detected: os=$os arch=$arch"

# --- Ask GitHub for the release ----------------------------------------------

if [ -n "$VERSION" ]; then
  api_url="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
else
  api_url="https://api.github.com/repos/$REPO/releases/latest"
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT INT TERM

info "fetching release info ($api_url)"
http_code=$(curl -sS -o "$tmpdir/release.json" -w '%{http_code}' "$api_url") \
  || die "could not reach the GitHub API — check your network connection"

case "$http_code" in
  200) ;;
  404)
    if [ -n "$VERSION" ]; then
      die "release '$VERSION' does not exist. See https://github.com/$REPO/releases"
    fi
    printf '%s\n' \
      "orquestador-install: there is no published release yet." \
      "" \
      "Until the first release is published there is nothing to download." \
      "Install from source instead:" \
      "  https://github.com/$REPO/blob/main/docs/INSTALL.md#build-from-source" \
      "or check the releases page later:" \
      "  https://github.com/$REPO/releases" >&2
    exit 1
    ;;
  403|429)
    printf '%s\n' \
      "orquestador-install: the GitHub API is rate-limiting this machine's IP" \
      "(unauthenticated requests are capped per hour). Wait a while and retry," \
      "or install manually:" \
      "  https://github.com/$REPO/releases" \
      "  https://github.com/$REPO/blob/main/docs/INSTALL.md" >&2
    exit 1
    ;;
  *)
    die "GitHub API returned HTTP $http_code — try again later"
    ;;
esac

tag=$(sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$tmpdir/release.json" | head -n 1)
[ -n "$tag" ] || die "could not read the release tag from the GitHub API response"
info "latest release: $tag"

# Collect every asset download URL from the API response (no jq needed).
grep -o '"browser_download_url":[[:space:]]*"[^"]*"' "$tmpdir/release.json" \
  | sed 's/.*"\(https[^"]*\)".*/\1/' > "$tmpdir/assets.txt" || true
[ -s "$tmpdir/assets.txt" ] || die "release $tag has no downloadable assets"

# Pick the first asset whose file name matches an extended regex.
pick() {
  pattern=$1
  while IFS= read -r url; do
    name=${url##*/}
    if printf '%s\n' "$name" | grep -qiE "$pattern"; then
      printf '%s\n' "$url"
      return 0
    fi
  done < "$tmpdir/assets.txt"
  return 1
}

# --- Select the asset for this platform --------------------------------------

if [ "$USE_CLI" = 1 ]; then
  # Stable CLI asset name: orquestador-proxy-<os>-<arch> (Linux/macOS here;
  # Windows is handled by install.ps1).
  cli_asset="orquestador-proxy-$os-$arch"
  asset_url=$(pick "^${cli_asset}\$") \
    || die "release $tag has no '$cli_asset' asset for $os/$arch"
elif [ "$os" = linux ]; then
  # Strict arch matching: require the arch token, never fall back to another
  # architecture's package.
  if [ "$arch" = x86_64 ]; then
    if [ "$USE_DEB" = 1 ]; then
      asset_url=$(pick '_amd64\.deb$') \
        || die "release $tag has no '_amd64.deb' asset for x86_64"
    else
      asset_url=$(pick '_amd64\.AppImage$') \
        || asset_url=$(pick '_amd64\.deb$') \
        || die "release $tag has no '_amd64.AppImage' or '_amd64.deb' asset for x86_64"
    fi
  else
    if [ "$USE_DEB" = 1 ]; then
      asset_url=$(pick '_aarch64\.deb$|_arm64\.deb$') \
        || die "release $tag has no '_aarch64.deb' or '_arm64.deb' asset for aarch64"
    else
      asset_url=$(pick '_aarch64\.AppImage$') \
        || asset_url=$(pick '_aarch64\.deb$|_arm64\.deb$') \
        || die "release $tag has no '_aarch64.AppImage', '_aarch64.deb' or '_arm64.deb' asset for aarch64"
    fi
  fi
else # macos
  # The .dmg must carry a matching architecture token (the release ships one
  # `universal` image). There is deliberately NO "any .dmg" fallback: that
  # would silently install the wrong architecture.
  if [ "$arch" = x86_64 ]; then
    asset_url=$(pick '(_x64|_x86_64|_universal)\.dmg$') \
      || die "release $tag has no universal or x86_64 .dmg asset for Intel Macs"
  else
    asset_url=$(pick '(_aarch64|_arm64|_universal)\.dmg$') \
      || die "release $tag has no universal or arm64 .dmg asset for Apple Silicon"
  fi
fi

asset_name=${asset_url##*/}
info "downloading $asset_name"
curl -fL -o "$tmpdir/$asset_name" "$asset_url" \
  || die "download failed: $asset_url"

# --- CLI mode ----------------------------------------------------------------

if [ "$USE_CLI" = 1 ]; then
  info "installing the CLI to $BIN_DIR/orquestador-proxy"
  mkdir -p "$BIN_DIR"
  cli_dest="$BIN_DIR/orquestador-proxy"
  # Write next to the target and rename: a failed or partial copy never
  # clobbers a working install.
  cp "$tmpdir/$asset_name" "$cli_dest.new" \
    || { rm -f "$cli_dest.new"; die "could not write to $BIN_DIR"; }
  chmod +x "$cli_dest.new"
  mv -f "$cli_dest.new" "$cli_dest"
  printf '%s\n' \
    "orquestador-install: installed orquestador-proxy $tag to $BIN_DIR/orquestador-proxy" \
    "" \
    "Serve (reads ./proxy.json by default):" \
    "  $BIN_DIR/orquestador-proxy [config.json]" \
    "Add a user (the password is read from stdin):" \
    "  $BIN_DIR/orquestador-proxy adduser <name> --role <editor|viewer>" >&2
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
      info "note: $BIN_DIR is not on your PATH. Add it to your shell profile:"
      info "  export PATH=\"$BIN_DIR:\$PATH\""
      ;;
  esac
  exit 0
fi

# --- Install -----------------------------------------------------------------

if [ "$os" = macos ]; then
  downloads_dir="$HOME/Downloads"
  mkdir -p "$downloads_dir"
  dmg_path="$downloads_dir/$asset_name"
  mv "$tmpdir/$asset_name" "$dmg_path"
  info "saved $dmg_path"
  info "opening the installer — drag 'Orquestador' into your Applications folder"
  if ! open "$dmg_path"; then
    info "could not open the .dmg automatically; double-click it at: $dmg_path"
  fi
  exit 0
fi

# Linux.
case "$asset_name" in
  *.deb)
    if [ "$USE_DEB" != 1 ] && ! command -v dpkg >/dev/null 2>&1; then
      die "release $tag has no AppImage and this system has no dpkg — install from source (docs/INSTALL.md)"
    fi
    if [ "$(id -u)" = "0" ]; then
      info "installing $asset_name with dpkg (running as root)"
      dpkg -i "$tmpdir/$asset_name" || apt-get -f install -y
    elif sudo -n true 2>/dev/null; then
      info "installing $asset_name with dpkg via sudo (non-interactive)"
      sudo dpkg -i "$tmpdir/$asset_name" || sudo apt-get -f install -y
    else
      cp "$tmpdir/$asset_name" "./$asset_name" \
        || die "could not save $asset_name here — re-run from a writable directory"
      printf '%s\n' \
        "orquestador-install: neither root nor a non-interactive sudo is" \
        "available, so nothing was installed automatically. The .deb was saved" \
        "as ./$asset_name — install it yourself with:" \
        "" \
        "  sudo dpkg -i ./$asset_name && sudo apt-get -f install" >&2
      exit 1
    fi
    info "done. Orquestador $tag is installed system-wide via dpkg;"
    info "start it from your application menu."
    exit 0
    ;;
  *)
    info "installing AppImage to $BIN_DIR/orquestador"
    mkdir -p "$BIN_DIR"
    app_dest="$BIN_DIR/orquestador"
    cp "$tmpdir/$asset_name" "$app_dest.new" \
      || { rm -f "$app_dest.new"; die "could not write to $BIN_DIR"; }
    chmod +x "$app_dest.new"
    mv -f "$app_dest.new" "$app_dest"

    apps_dir="$DATA_DIR/applications"
    mkdir -p "$apps_dir"
    info "writing desktop entry $apps_dir/orquestador.desktop"
    cat > "$apps_dir/orquestador.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Orquestador
Comment=Desktop control tower for OpenCode agents
Exec=$BIN_DIR/orquestador
Terminal=false
Categories=Development;IDE;
EOF
    ;;
esac

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    info "note: $BIN_DIR is not on your PATH. Add it to your shell profile:"
    info "  export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac

info "done. Orquestador $tag is installed."
info "start it from your application menu, or run: $BIN_DIR/orquestador"
info "it needs a reachable OpenCode server (opencode serve) — see docs/INSTALL.md"
