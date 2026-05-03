#!/usr/bin/env bash
# End-user install script — downloads the latest Dockside release for
# the host OS and installs it. Artifacts are produced by cargo-packager
# in the release workflow.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/salamaashoush/dockside/main/scripts/install.sh | bash
#   ./scripts/install.sh                # latest
#   ./scripts/install.sh v0.3.1         # specific version
#
# Layouts produced by the release pipeline:
#   macOS arm64  →  Dockside-vX.Y.Z-macos-arm64.zip   (zipped .app)
#                +  Dockside_X.Y.Z_aarch64.dmg
#   Linux x86_64 →  Dockside_X.Y.Z_x86_64.AppImage
#                +  Dockside_X.Y.Z_amd64.deb         (Debian/Ubuntu)
#
# This script always grabs the .app zip on macOS and the AppImage on
# Linux for a uniform single-file install. Use the `.deb` directly via
# `dpkg -i` if you prefer distro integration.

set -euo pipefail

REPO="salamaashoush/dockside"
VERSION="${1:-}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()  { printf "${BLUE}[INFO]${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}[OK]${NC}   %s\n" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC} %s\n" "$*"; }
fatal() { printf "${RED}[ERR]${NC}  %s\n" "$*" >&2; exit 1; }
need()  { command -v "$1" >/dev/null 2>&1 || fatal "$1 is required"; }
need_cmd() { command -v "$1" >/dev/null 2>&1; }

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

# Strip leading "v" — cargo-packager artifact names use the bare version.
strip_v() { echo "${1#v}"; }

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

  local version_no_v
  version_no_v="$(strip_v "$version")"
  local asset="Dockside_${version_no_v}_x86_64.AppImage"
  local url="https://github.com/${REPO}/releases/download/${version}/${asset}"

  local bin_dir="$HOME/.local/bin"
  local app_dir="$HOME/.local/share/applications"
  local icon_dir="$HOME/.local/share/icons/hicolor/256x256/apps"
  mkdir -p "$bin_dir" "$app_dir" "$icon_dir"

  info "Downloading $url"
  need curl
  local target="$bin_dir/dockside"
  curl -fL "$url" -o "$target.tmp"
  chmod +x "$target.tmp"
  mv "$target.tmp" "$target"

  # Best-effort icon extraction. The AppImage has its own desktop file
  # and icon at AppDir root; we keep both for the system menu.
  if [[ -x "$target" ]]; then
    local extract_dir
    extract_dir="$(mktemp -d)"
    if (cd "$extract_dir" && "$target" --appimage-extract '*.png' >/dev/null 2>&1); then
      local png
      png="$(find "$extract_dir/squashfs-root" -maxdepth 2 -name '*.png' | head -n1 || true)"
      if [[ -n "$png" ]]; then
        cp "$png" "$icon_dir/dockside.png" || true
      fi
    fi
    rm -rf "$extract_dir"
  fi

  cat > "$app_dir/dockside.desktop" <<EOF
[Desktop Entry]
Name=Dockside
Comment=Docker Management Desktop App
Exec=$target
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

  ok "Installed Dockside ${version} to $target"
  echo ""
  info "Run: dockside  (or launch from your app menu)"
  echo ""
  info "First-time DNS setup:"
  echo "  1. Settings → Local DNS → Set up Local DNS"
  echo "     (one polkit prompt; installs the helper at /usr/local/libexec/,"
  echo "     registers the system resolver, trusts the local CA)."
  echo "  2. Settings → Local DNS → Drop port from URL"
  echo "     (optional; nftables redirect + systemd unit so http://name.dockside.test/"
  echo "     works without :47080)."
  echo ""
  if ! need_cmd nft; then
    warn "nftables not found — install before using 'Drop port from URL'."
    warn "  Arch: sudo pacman -S nftables    Debian/Ubuntu: sudo apt install nftables"
  fi
  if ! need_cmd pkexec; then
    warn "pkexec not found — the privileged-setup prompts will fail. Install polkit."
  fi

  echo ""
  info "Prefer a distro-native package?"
  info "  Debian/Ubuntu : sudo dpkg -i Dockside_${version_no_v}_amd64.deb"
  info "  Arch / CachyOS: download the matching PKGBUILD + .tar.gz, then 'makepkg -si'"
  info "  (Both drop the polkit policy automatically; no Set up prompt needed.)"
}

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
