#!/usr/bin/env bash
# Install build dependencies for Dockside contributors.
# Detects host OS and installs:
#   - Rust toolchain via rustup
#   - Zig 0.15.2 (required by libghostty-vt-sys)
#   - System libraries needed by gpui / gtk / webkit / vulkan
#
# macOS:    Apple Silicon only.
# Linux:    Debian/Ubuntu, Fedora, Arch, openSUSE supported.

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()   { printf "${BLUE}[INFO]${NC} %s\n" "$*"; }
ok()     { printf "${GREEN}[OK]${NC}   %s\n" "$*"; }
warn()   { printf "${YELLOW}[WARN]${NC} %s\n" "$*"; }
fatal()  { printf "${RED}[ERR]${NC}  %s\n" "$*" >&2; exit 1; }

need_cmd() { command -v "$1" >/dev/null 2>&1; }

ZIG_VERSION="0.15.2"

install_rust() {
  if need_cmd rustc; then
    ok "rustc already installed: $(rustc --version)"
    return
  fi
  info "Installing Rust via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
  ok "Rust installed"
}

install_zig_tarball() {
  local arch="$1" os="$2"
  local url="https://ziglang.org/download/${ZIG_VERSION}/zig-${os}-${arch}-${ZIG_VERSION}.tar.xz"
  local dest="$HOME/.local/share/zig-${ZIG_VERSION}"
  if [[ -x "$dest/zig" ]]; then
    ok "Zig already installed at $dest"
  else
    info "Downloading Zig from $url"
    mkdir -p "$dest"
    curl -fL "$url" | tar -xJ -C "$dest" --strip-components=1
    ok "Zig extracted to $dest"
  fi
  mkdir -p "$HOME/.local/bin"
  ln -sf "$dest/zig" "$HOME/.local/bin/zig"
  case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) warn "Add \$HOME/.local/bin to PATH to use zig: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
  esac
}

install_zig() {
  if need_cmd zig; then
    local current
    current="$(zig version 2>/dev/null || true)"
    if [[ "$current" == "$ZIG_VERSION" ]]; then
      ok "Zig $ZIG_VERSION already installed"
      return
    fi
    warn "Zig $current detected, project pins $ZIG_VERSION — installing pinned version side-by-side"
  fi

  local kernel arch
  kernel="$(uname -s)"
  arch="$(uname -m)"
  case "$kernel-$arch" in
    Darwin-arm64)         install_zig_tarball aarch64 macos ;;
    Linux-x86_64)         install_zig_tarball x86_64  linux ;;
    Linux-aarch64)        install_zig_tarball aarch64 linux ;;
    *) fatal "Unsupported platform for Zig install: $kernel-$arch" ;;
  esac
}

install_macos_libs() {
  if ! need_cmd brew; then
    fatal "Homebrew not found. Install from https://brew.sh first."
  fi
  brew update
  brew install pkg-config cmake
  ok "macOS build deps installed"
}

install_linux_libs() {
  local pkgs_apt=(
    build-essential pkg-config cmake curl ca-certificates
    libgtk-3-dev libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev
    libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
    libssl-dev libvulkan-dev libasound2-dev
  )
  local pkgs_dnf=(
    @development-tools pkgconf-pkg-config cmake curl ca-certificates
    gtk3-devel webkit2gtk4.1-devel libxdo-devel libappindicator-gtk3-devel
    libxcb-devel libxkbcommon-devel
    openssl-devel vulkan-loader-devel alsa-lib-devel
  )
  local pkgs_pacman=(
    base-devel pkgconf cmake curl ca-certificates
    gtk3 webkit2gtk-4.1 xdotool libappindicator-gtk3
    libxcb libxkbcommon openssl vulkan-icd-loader alsa-lib
  )
  local pkgs_zypper=(
    pattern:devel_basis pkg-config cmake curl ca-certificates
    gtk3-devel webkit2gtk3-devel xdotool-devel libappindicator3-devel
    libxcb-devel libxkbcommon-devel
    libopenssl-devel vulkan-devel alsa-devel
  )

  if need_cmd apt-get; then
    info "Installing Linux deps via apt-get..."
    sudo apt-get update
    sudo apt-get install -y "${pkgs_apt[@]}"
  elif need_cmd dnf; then
    info "Installing Linux deps via dnf..."
    sudo dnf install -y "${pkgs_dnf[@]}"
  elif need_cmd pacman; then
    info "Installing Linux deps via pacman..."
    sudo pacman -Syu --needed --noconfirm "${pkgs_pacman[@]}"
  elif need_cmd zypper; then
    info "Installing Linux deps via zypper..."
    sudo zypper install -y "${pkgs_zypper[@]}"
  else
    fatal "Unsupported Linux distro — install GTK3, WebKit2GTK 4.1, libxdo, libappindicator, libxcb, libxkbcommon, openssl, vulkan, alsa headers manually."
  fi
  ok "Linux build deps installed"
}

main() {
  case "$(uname -s)" in
    Darwin)
      [[ "$(uname -m)" == "arm64" ]] || fatal "Only Apple Silicon (arm64) is supported."
      install_macos_libs
      install_rust
      install_zig
      ;;
    Linux)
      install_linux_libs
      install_rust
      install_zig
      ;;
    *) fatal "Unsupported OS: $(uname -s)" ;;
  esac

  ok "All build dependencies installed."
  echo ""
  info "Next: cargo build --release"
}

main "$@"
