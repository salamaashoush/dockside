#!/usr/bin/env bash
set -euo pipefail

# Dockside Installer
# A native desktop application for managing Docker and Kubernetes

VERSION="${DOCKSIDE_VERSION:-latest}"
GITHUB_REPO="salamaashoush/dockside"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
ENABLE_K8S=false

print_banner() {
    echo ""
    echo -e "${BLUE}"
    echo "  ____             _        _     _      "
    echo " |  _ \  ___   ___| | _____(_) __| | ___ "
    echo " | | | |/ _ \ / __| |/ / __| |/ _\` |/ _ \\"
    echo " | |_| | (_) | (__|   <\__ \ | (_| |  __/"
    echo " |____/ \___/ \___|_|\_\___/_|\__,_|\___|"
    echo -e "${NC}"
    echo "  Docker & Kubernetes Desktop Manager"
    echo ""
}

info() { echo -e "${BLUE}INFO${NC} $1"; }
success() { echo -e "${GREEN}OK${NC} $1"; }
warn() { echo -e "${YELLOW}WARN${NC} $1"; }
error() { echo -e "${RED}ERROR${NC} $1"; exit 1; }

check_macos() {
    if [[ "$(uname -s)" != "Darwin" ]]; then
        error "Dockside is currently only available for macOS"
    fi

    if [[ "$(uname -m)" != "arm64" ]]; then
        error "Dockside requires Apple Silicon (M1/M2/M3/M4)"
    fi
}

command_exists() { command -v "$1" &> /dev/null; }

# Check brew in PATH or common locations
find_brew() {
    if command_exists brew; then
        echo "brew"
        return 0
    fi
    for path in "/opt/homebrew/bin/brew" "/usr/local/bin/brew"; do
        if [[ -x "$path" ]]; then
            echo "$path"
            return 0
        fi
    done
    return 1
}

install_homebrew() {
    if find_brew &>/dev/null; then
        success "Homebrew already installed"
        # Ensure brew is in PATH for this session
        if [[ -f "/opt/homebrew/bin/brew" ]]; then
            eval "$(/opt/homebrew/bin/brew shellenv)"
        fi
        return 0
    fi

    info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    if [[ -f "/opt/homebrew/bin/brew" ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    fi

    success "Homebrew installed"
}

install_docker() {
    if command_exists docker; then
        success "Docker CLI already installed"
        return 0
    fi

    info "Installing Docker CLI and Docker Compose..."
    brew install docker docker-compose
    success "Docker CLI installed"
}

install_colima() {
    if command_exists colima; then
        success "Colima already installed"
        return 0
    fi

    info "Installing Colima..."
    brew install colima
    success "Colima installed"
}

install_kubectl() {
    if command_exists kubectl; then
        success "kubectl already installed"
        return 0
    fi

    info "Installing kubectl..."
    brew install kubernetes-cli
    success "kubectl installed"
}

get_latest_version() {
    if [[ "$VERSION" == "latest" ]]; then
        info "Fetching latest version..."
        VERSION=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
        if [[ -z "$VERSION" ]]; then
            error "Failed to fetch latest version. Check your internet connection."
        fi
        info "Latest version: $VERSION"
    fi
}

install_dockside() {
    get_latest_version

    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/Dockside-$VERSION-macos-arm64.zip"

    info "Downloading Dockside $VERSION..."

    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/Dockside.zip"; then
        error "Failed to download Dockside. Check if the release exists."
    fi

    info "Installing to /Applications..."
    unzip -q "$TMP_DIR/Dockside.zip" -d "$TMP_DIR"

    if [[ -d "/Applications/Dockside.app" ]]; then
        rm -rf "/Applications/Dockside.app"
    fi

    mv "$TMP_DIR/Dockside.app" "/Applications/"
    success "Dockside.app installed to /Applications"
}

ask_kubernetes() {
    # Skip prompt if already specified via flag
    if [[ "$ENABLE_K8S" == "true" ]]; then
        return 0
    fi

    # Check if we have a TTY for interactive input
    if [[ ! -t 0 ]] && [[ ! -e /dev/tty ]]; then
        info "Non-interactive mode, skipping Kubernetes prompt"
        info "Use --with-kubernetes flag to enable Kubernetes"
        return 0
    fi

    echo ""
    echo -e "${BLUE}Do you want to enable Kubernetes?${NC}"
    echo "  This allows you to run and manage Kubernetes workloads locally."
    echo "  You can always enable it later from the Dockside app."
    echo ""
    # Read from /dev/tty to support curl | bash
    read -p "Enable Kubernetes? [y/N] " -n 1 -r < /dev/tty
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        ENABLE_K8S=true
    fi
}

start_colima() {
    if ! command_exists colima; then
        return 0
    fi

    # Check if Colima is already running
    if colima status &>/dev/null; then
        # Check if it has Kubernetes enabled
        if [[ "$ENABLE_K8S" == "true" ]]; then
            if ! colima status 2>/dev/null | grep -q "kubernetes.*enabled"; then
                warn "Colima is running but without Kubernetes"
                info "To enable Kubernetes, run: colima stop && colima start --kubernetes"
            else
                success "Colima is already running with Kubernetes"
            fi
        else
            success "Colima is already running"
        fi
        return 0
    fi

    info "Starting Colima with optimized defaults..."
    info "  CPU: 4 cores, Memory: 8GB, Disk: 60GB"

    local colima_args="--cpu 4 --memory 8 --disk 60"

    if [[ "$ENABLE_K8S" == "true" ]]; then
        info "  Kubernetes: enabled"
        colima_args="$colima_args --kubernetes"
    fi

    if ! colima start $colima_args; then
        error "Failed to start Colima"
    fi

    success "Colima started"
}

verify_docker() {
    info "Verifying Docker setup..."

    # Wait for Docker socket to be ready
    local retries=15
    while [[ $retries -gt 0 ]]; do
        if docker info &>/dev/null; then
            break
        fi
        sleep 1
        retries=$((retries - 1))
    done

    if ! docker info &>/dev/null; then
        warn "Docker is not responding. Make sure Colima is running."
        return 1
    fi

    success "Docker daemon is running"

    info "Running hello-world container to verify Docker..."
    if docker run --rm hello-world &>/dev/null; then
        success "Docker is working correctly"
    else
        warn "Failed to run hello-world container"
        return 1
    fi
}

verify_kubernetes() {
    if [[ "$ENABLE_K8S" != "true" ]]; then
        return 0
    fi

    info "Verifying Kubernetes setup..."

    # Wait for Kubernetes to be ready
    local retries=30
    while [[ $retries -gt 0 ]]; do
        if kubectl cluster-info &>/dev/null; then
            break
        fi
        sleep 2
        retries=$((retries - 1))
    done

    if ! kubectl cluster-info &>/dev/null; then
        warn "Kubernetes is not responding. It may still be starting up."
        return 1
    fi

    success "Kubernetes cluster is running"

    # Check if nodes are ready
    info "Waiting for Kubernetes node to be ready..."
    if kubectl wait --for=condition=Ready node --all --timeout=60s &>/dev/null; then
        success "Kubernetes node is ready"
    else
        warn "Kubernetes node is not ready yet"
    fi

    # Verify with a simple pod
    info "Running test pod to verify Kubernetes..."
    if kubectl run --rm -i --restart=Never --image=busybox test-pod -- echo "Kubernetes is working" &>/dev/null; then
        success "Kubernetes is working correctly"
    else
        # Cleanup in case it failed
        kubectl delete pod test-pod --ignore-not-found &>/dev/null || true
        warn "Failed to run test pod (this is normal if cluster is still initializing)"
    fi
}

print_next_steps() {
    echo ""
    echo -e "${GREEN}Installation complete!${NC}"
    echo ""
    echo "Open Dockside from Spotlight or run:"
    echo "  open /Applications/Dockside.app"
    echo ""
}

main() {
    print_banner

    SKIP_DEPS=false
    SKIP_APP=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-deps)
                SKIP_DEPS=true
                shift
                ;;
            --deps-only)
                SKIP_APP=true
                shift
                ;;
            --with-kubernetes)
                ENABLE_K8S=true
                shift
                ;;
            --version)
                shift
                VERSION="$1"
                shift
                ;;
            -h|--help)
                echo "Dockside Installer"
                echo ""
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --skip-deps       Skip installing Docker and Colima (app only)"
                echo "  --deps-only       Only install dependencies (no app)"
                echo "  --with-kubernetes Enable Kubernetes support"
                echo "  --version VER     Install specific version (default: latest)"
                echo "  -h, --help        Show this help message"
                echo ""
                echo "Examples:"
                echo "  curl -fsSL https://raw.githubusercontent.com/salamaashoush/dockside/main/install.sh | bash"
                echo "  ./install.sh --with-kubernetes"
                echo "  ./install.sh --version v0.1.0"
                echo ""
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done

    check_macos

    if [[ "$SKIP_DEPS" == "false" ]]; then
        echo ""
        info "Setting up Docker environment..."
        echo ""
        install_homebrew
        install_docker
        install_colima
        install_kubectl
        ask_kubernetes
        start_colima
        verify_docker
        verify_kubernetes
    fi

    if [[ "$SKIP_APP" == "false" ]]; then
        echo ""
        install_dockside
    fi

    print_next_steps
}

main "$@"
