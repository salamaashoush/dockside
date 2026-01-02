#!/usr/bin/env bash
set -euo pipefail

# Deckhand Installer
# A native desktop application for managing Docker and Kubernetes

VERSION="${DECKHAND_VERSION:-latest}"
INSTALL_DIR="${DECKHAND_INSTALL_DIR:-$HOME/.local/bin}"
GITHUB_REPO="salamaashoush/deckhand"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_banner() {
    echo ""
    echo -e "${BLUE}"
    echo "  ____            _    _                     _ "
    echo " |  _ \  ___  ___| | _| |__   __ _ _ __   __| |"
    echo " | | | |/ _ \/ __| |/ / '_ \ / _\` | '_ \ / _\` |"
    echo " | |_| |  __/ (__|   <| | | | (_| | | | | (_| |"
    echo " |____/ \___|\___|_|\_\_| |_|\__,_|_| |_|\__,_|"
    echo -e "${NC}"
    echo "  Docker & Kubernetes Desktop Manager"
    echo ""
}

info() {
    echo -e "${BLUE}INFO${NC} $1"
}

success() {
    echo -e "${GREEN}SUCCESS${NC} $1"
}

warn() {
    echo -e "${YELLOW}WARN${NC} $1"
}

error() {
    echo -e "${RED}ERROR${NC} $1"
    exit 1
}

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Darwin)
            OS="apple-darwin"
            ;;
        Linux)
            OS="unknown-linux-gnu"
            ;;
        *)
            error "Unsupported operating system: $OS"
            ;;
    esac

    case "$ARCH" in
        arm64|aarch64)
            ARCH="aarch64"
            ;;
        x86_64)
            if [[ "$OS" == "apple-darwin" ]]; then
                error "Intel Macs are not supported. Deckhand requires Apple Silicon (M1/M2/M3)."
            fi
            ARCH="x86_64"
            ;;
        *)
            error "Unsupported architecture: $ARCH"
            ;;
    esac

    PLATFORM="${ARCH}-${OS}"
    info "Detected platform: $PLATFORM"
}

# Check if a command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Install Homebrew if not present (macOS only)
install_homebrew() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        return 0
    fi

    if command_exists brew; then
        info "Homebrew already installed"
        return 0
    fi

    info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # Add to PATH for Apple Silicon
    if [[ -f "/opt/homebrew/bin/brew" ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    fi

    success "Homebrew installed"
}

# Install Docker CLI
install_docker() {
    if command_exists docker; then
        info "Docker CLI already installed"
        return 0
    fi

    info "Installing Docker CLI..."

    case "$(uname -s)" in
        Darwin)
            brew install docker docker-compose
            ;;
        Linux)
            if command_exists apt-get; then
                sudo apt-get update
                sudo apt-get install -y docker.io docker-compose
                # Add user to docker group
                sudo usermod -aG docker "$USER" 2>/dev/null || true
            elif command_exists dnf; then
                sudo dnf install -y docker docker-compose
                sudo systemctl enable docker
                sudo systemctl start docker
                sudo usermod -aG docker "$USER" 2>/dev/null || true
            elif command_exists pacman; then
                sudo pacman -S --noconfirm docker docker-compose
                sudo systemctl enable docker
                sudo systemctl start docker
                sudo usermod -aG docker "$USER" 2>/dev/null || true
            else
                error "Unsupported Linux distribution. Please install Docker manually."
            fi
            warn "You may need to log out and back in for docker group permissions to take effect"
            ;;
    esac

    success "Docker CLI installed"
}

# Install Colima (macOS container runtime)
install_colima() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        info "Colima is macOS-only (Linux runs Docker natively), skipping..."
        return 0
    fi

    if command_exists colima; then
        info "Colima already installed"
        return 0
    fi

    info "Installing Colima..."
    brew install colima
    success "Colima installed"
}

# Get latest version from GitHub
get_latest_version() {
    if [[ "$VERSION" == "latest" ]]; then
        info "Fetching latest version..."
        VERSION=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
        if [[ -z "$VERSION" ]]; then
            error "Failed to fetch latest version"
        fi
        info "Latest version: $VERSION"
    fi
}

# Download and install Deckhand
install_deckhand() {
    get_latest_version
    detect_platform

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Download URL
    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/deckhand-$VERSION-$PLATFORM.tar.gz"

    info "Downloading Deckhand $VERSION..."
    info "URL: $DOWNLOAD_URL"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    # Download
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/deckhand.tar.gz"; then
        error "Failed to download Deckhand. Check if the release exists."
    fi

    # Extract
    info "Extracting..."
    tar -xzf "$TMP_DIR/deckhand.tar.gz" -C "$TMP_DIR"

    # Install binary
    if [[ -f "$TMP_DIR/deckhand" ]]; then
        mv "$TMP_DIR/deckhand" "$INSTALL_DIR/deckhand"
        chmod +x "$INSTALL_DIR/deckhand"
    else
        error "Binary not found in archive"
    fi

    success "Deckhand installed to $INSTALL_DIR/deckhand"
}

# Install macOS app bundle
install_macos_app() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        return 0
    fi

    get_latest_version

    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/Deckhand-$VERSION.app.zip"

    info "Checking for macOS app bundle..."

    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    if curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/Deckhand.app.zip" 2>/dev/null; then
        info "Installing macOS app bundle..."
        unzip -q "$TMP_DIR/Deckhand.app.zip" -d "$TMP_DIR"

        if [[ -d "$TMP_DIR/Deckhand.app" ]]; then
            # Remove old version if exists
            rm -rf "/Applications/Deckhand.app"
            mv "$TMP_DIR/Deckhand.app" "/Applications/"
            success "Deckhand.app installed to /Applications"
        fi
    else
        info "No app bundle available, CLI binary installed instead"
    fi
}

# Add to PATH
setup_path() {
    SHELL_NAME=$(basename "$SHELL")

    case "$SHELL_NAME" in
        bash)
            RC_FILE="$HOME/.bashrc"
            [[ "$(uname -s)" == "Darwin" ]] && RC_FILE="$HOME/.bash_profile"
            ;;
        zsh)
            RC_FILE="$HOME/.zshrc"
            ;;
        fish)
            RC_FILE="$HOME/.config/fish/config.fish"
            ;;
        *)
            RC_FILE="$HOME/.profile"
            ;;
    esac

    # Check if already in PATH
    if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
        return 0
    fi

    # Check if already in RC file
    if [[ -f "$RC_FILE" ]] && grep -q "$INSTALL_DIR" "$RC_FILE"; then
        return 0
    fi

    info "Adding $INSTALL_DIR to PATH in $RC_FILE"

    if [[ "$SHELL_NAME" == "fish" ]]; then
        echo "fish_add_path $INSTALL_DIR" >> "$RC_FILE"
    else
        echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$RC_FILE"
    fi

    warn "Please restart your shell or run: source $RC_FILE"
}

# Verify installation
verify_installation() {
    if [[ -x "$INSTALL_DIR/deckhand" ]]; then
        success "Installation verified!"
        echo ""
        if [[ "$(uname -s)" == "Darwin" ]]; then
            if [[ -d "/Applications/Deckhand.app" ]]; then
                echo "Open Deckhand from /Applications or run 'deckhand' from terminal"
            else
                echo "Run 'deckhand' to start the application"
            fi
        else
            echo "Run 'deckhand' to start the application"
            echo ""
            echo "Note: Make sure Docker is running:"
            echo "  sudo systemctl start docker"
        fi
        echo ""
    else
        error "Installation verification failed"
    fi
}

# Start Colima if not running
start_colima() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        return 0
    fi

    if ! command_exists colima; then
        return 0
    fi

    if colima status &>/dev/null; then
        info "Colima is already running"
        return 0
    fi

    echo ""
    read -p "Would you like to start Colima now? [y/N] " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        info "Starting Colima..."
        colima start --cpu 4 --memory 8 --disk 60
        success "Colima started"
    else
        echo ""
        echo "To start Colima later, run:"
        echo "  colima start --cpu 4 --memory 8 --disk 60"
        echo ""
        echo "Or with Kubernetes:"
        echo "  colima start --cpu 4 --memory 8 --disk 60 --kubernetes"
        echo ""
    fi
}

# Main installation flow
main() {
    print_banner

    # Parse arguments
    SKIP_DEPS=false
    BINARY_ONLY=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-deps)
                SKIP_DEPS=true
                shift
                ;;
            --binary-only)
                BINARY_ONLY=true
                shift
                ;;
            --version)
                shift
                VERSION="$1"
                shift
                ;;
            --install-dir)
                shift
                INSTALL_DIR="$1"
                shift
                ;;
            -h|--help)
                echo "Deckhand Installer"
                echo ""
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --skip-deps       Skip installing Docker and Colima"
                echo "  --binary-only     Only install the CLI binary (no app bundle)"
                echo "  --version VER     Install specific version (default: latest)"
                echo "  --install-dir DIR Install to specific directory"
                echo "  -h, --help        Show this help message"
                echo ""
                echo "Environment variables:"
                echo "  DECKHAND_VERSION     Version to install"
                echo "  DECKHAND_INSTALL_DIR Installation directory"
                echo ""
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done

    # Install dependencies
    if [[ "$SKIP_DEPS" == "false" ]]; then
        if [[ "$(uname -s)" == "Darwin" ]]; then
            install_homebrew
        fi
        install_docker
        install_colima
    fi

    # Install Deckhand
    install_deckhand

    # Install macOS app bundle
    if [[ "$BINARY_ONLY" == "false" ]]; then
        install_macos_app
    fi

    # Setup PATH
    setup_path

    # Verify
    verify_installation

    # Offer to start Colima
    if [[ "$SKIP_DEPS" == "false" ]]; then
        start_colima
    fi

    echo ""
    success "Deckhand installation complete!"
    echo ""
}

main "$@"
