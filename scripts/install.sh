#!/usr/bin/env bash
# End-user install script — downloads the latest Dockside release for
# the host OS and installs it into either ~/.local or /Applications.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/salamaashoush/dockside/main/scripts/install.sh | bash
#   ./scripts/install.sh                # latest
#   ./scripts/install.sh v0.2.0         # specific version
#
# Supported targets: macOS arm64, Linux x86_64.

set -euo pipefail

REPO="salamaashoush/dockside"
VERSION="${1:-}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()  { printf "${BLUE}[INFO]${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}[OK]${NC}   %s\n" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC} %s\n" "$*"; }
fatal() { printf "${RED}[ERR]${NC}  %s\n" "$*" >&2; exit 1; }
need()  { command -v "$1" >/dev/null 2>&1 || fatal "$1 is required"; }

resolve_version() {
  if [[ -n "$VERSION" ]]; then
    echo "$VERSION"
    return
  fi
  need curl
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep -oE '"tag_name": *"v[^"]+"' \
    | head -n1 \
    | sed -E 's/.*"(v[^"]+)".*/\1/'
}

install_macos() {
  local version="$1"
  local arch
  arch="$(uname -m)"
  [[ "$arch" == "arm64" ]] || fatal "Only Apple Silicon (arm64) is supported on macOS."
  local asset="Dockside-${version}-macos-arm64.zip"
  local url="https://github.com/${REPO}/releases/download/${version}/${asset}"
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  info "Downloading $url"
  need curl
  curl -fL "$url" -o "$tmp/$asset"

  info "Extracting bundle"
  (cd "$tmp" && unzip -q "$asset")
  [[ -d "$tmp/Dockside.app" ]] || fatal "Bundle extraction failed: $tmp"

  local target="/Applications/Dockside.app"
  if [[ -d "$target" ]]; then
    info "Removing existing $target"
    rm -rf "$target"
  fi
  info "Installing to $target"
  mv "$tmp/Dockside.app" "$target"
  xattr -dr com.apple.quarantine "$target" 2>/dev/null || true
  ok "Installed Dockside ${version} to $target"
  echo ""
  info "Open from /Applications or run: open -a Dockside"
}

install_linux() {
  local version="$1"
  local arch
  arch="$(uname -m)"
  [[ "$arch" == "x86_64" ]] || fatal "Only x86_64 Linux is supported."
  local asset="Dockside-${version}-linux-x86_64.tar.gz"
  local url="https://github.com/${REPO}/releases/download/${version}/${asset}"
  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  info "Downloading $url"
  need curl
  curl -fL "$url" -o "$tmp/$asset"

  info "Extracting"
  tar -xzf "$tmp/$asset" -C "$tmp"

  local extracted
  extracted="$(find "$tmp" -mindepth 1 -maxdepth 1 -type d | head -n1)"
  [[ -n "$extracted" ]] || fatal "Tarball missing top-level directory"

  local bin_dir="$HOME/.local/bin"
  local share_dir="$HOME/.local/share/dockside"
  local app_dir="$HOME/.local/share/applications"
  local icon_dir="$HOME/.local/share/icons/hicolor/256x256/apps"
  mkdir -p "$bin_dir" "$share_dir" "$app_dir" "$icon_dir"

  info "Installing binary to $bin_dir/dockside"
  install -m 0755 "$extracted/dockside" "$bin_dir/dockside"

  info "Installing themes to $share_dir/themes"
  rm -rf "$share_dir/themes"
  cp -R "$extracted/themes" "$share_dir/themes"

  if [[ -f "$extracted/assets/icon.png" ]]; then
    cp "$extracted/assets/icon.png" "$icon_dir/dockside.png"
  fi

  cat > "$app_dir/dockside.desktop" <<EOF
[Desktop Entry]
Name=Dockside
Comment=Docker Management Desktop App
Exec=$bin_dir/dockside
Icon=dockside
Type=Application
Categories=Development;Utility;
EOF

  if need_cmd update-desktop-database; then
    update-desktop-database "$app_dir" >/dev/null 2>&1 || true
  fi

  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) warn "Add to PATH: export PATH=\"$bin_dir:\$PATH\"" ;;
  esac

  ok "Installed Dockside ${version} to $bin_dir/dockside"
  echo ""
  info "Run: dockside"
}

need_cmd() { command -v "$1" >/dev/null 2>&1; }

main() {
  local version
  version="$(resolve_version)"
  [[ -n "$version" ]] || fatal "Could not resolve a release version."
  info "Installing Dockside ${version}"

  case "$(uname -s)" in
    Darwin) install_macos "$version" ;;
    Linux)  install_linux  "$version" ;;
    *) fatal "Unsupported OS: $(uname -s)" ;;
  esac
}

main "$@"
