# Dockside

A native cross-platform desktop application for managing Docker containers, images, volumes, networks, and Kubernetes resources via Colima or native Docker.

Built with [GPUI](https://gpui.rs) for a fast, native experience on macOS, Linux, and Windows.

## Features

- **Docker Management**: Containers, images, volumes, networks
- **Kubernetes Support**: Pods, services, deployments
- **Cross-Platform**: macOS (Colima), Linux (native Docker / Colima), Windows (Docker in WSL2)
- **Native Performance**: Built with Rust and GPUI framework
- **Real-time Updates**: Live status monitoring
- **Terminal Integration**: Exec into containers directly
- **Log Viewing**: Stream container and pod logs

## Requirements

- macOS 13.0+, Linux (glibc 2.31+), or Windows 10/11
- Docker runtime:
  - **macOS**: [Colima](https://github.com/abiosoft/colima)
  - **Linux**: Native Docker daemon or Colima
  - **Windows**: Docker running inside WSL2 (exposed via TCP)
- Docker CLI

## Installation

### Quick Install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/salamaashoush/dockside/main/install.sh | bash
```

This will install Dockside and optionally set up Docker and Colima if not already installed.

### Manual Installation

Download the latest release from the [Releases page](https://github.com/salamaashoush/dockside/releases).

**macOS App Bundle:**
1. Download `Dockside-vX.X.X.app.zip`
2. Extract and move to `/Applications`

**macOS CLI Binary:**
1. Download `dockside-vX.X.X-aarch64-apple-darwin.tar.gz` (Apple Silicon) or `dockside-vX.X.X-x86_64-apple-darwin.tar.gz` (Intel)
2. Extract: `tar -xzf dockside-*.tar.gz`
3. Move to PATH: `mv dockside ~/.local/bin/`

**Linux:**
1. Download `dockside-vX.X.X-x86_64-unknown-linux-gnu.tar.gz`
2. Extract: `tar -xzf dockside-*.tar.gz`
3. Move to PATH: `mv dockside ~/.local/bin/`

**Windows:**
1. Download `dockside-vX.X.X-x86_64-pc-windows-msvc.zip`
2. Extract and run `dockside.exe`

### Build from Source

```bash
git clone https://github.com/salamaashoush/dockside
cd dockside

# Install dependencies
just install-deps

# Build and run
just run

# Or build release
just build-release

# Create app bundle (macOS)
just bundle
```

## Usage

### Starting a Docker Runtime

**macOS / Linux (Colima):**

```bash
colima start --cpu 4 --memory 8 --disk 60
# With Kubernetes
colima start --cpu 4 --memory 8 --disk 60 --kubernetes
```

Or via justfile: `just start-colima` / `just start-colima-k8s`.

**Linux (native Docker):** ensure `dockerd` is running and `/var/run/docker.sock` is accessible.

**Windows (WSL2):** install a WSL2 distro, install Docker inside it, and expose the daemon over TCP. The setup dialog walks you through this on first launch.

### Running Dockside

```bash
# Run from source
just run

# Or run installed binary
dockside

# Or open the app bundle (macOS)
open /Applications/Dockside.app
```

## Development

### Prerequisites

- Rust 1.91+
- [just](https://github.com/casey/just) command runner

### Setup

```bash
just init       # Install development tools
just run        # Run in development mode
just run-debug  # Run with debug logging
just watch      # Watch for changes
```

### Commands

```bash
just              # List all commands
just check        # Fast compilation check
just build        # Build debug version
just test         # Run tests
just lint         # Run clippy
just fmt          # Format code
just ready        # Run all CI checks
```

### Building

```bash
just build-release         # Build release binary
just build-native          # Build with native CPU opts
just build-macos-universal # Build universal binary (macOS)
just bundle                # Create .app bundle (macOS)
just install-app           # Install to /Applications
```

### Releasing

```bash
just bump patch|minor|major  # Bump version
just release patch           # Create release tag
just release-all patch       # Full release workflow
```

## Project Structure

```
dockside/
├── src/
│   ├── main.rs           # Entry point
│   ├── app.rs            # Main application
│   ├── platform/         # Platform detection + Docker runtime abstraction
│   ├── docker/           # Docker client and types
│   ├── colima/           # Colima client and machine types
│   ├── kubernetes/       # Kubernetes client and types
│   ├── state/            # Application state management
│   ├── services/         # Background services
│   ├── ui/               # UI components
│   │   ├── containers/   # Container views
│   │   ├── images/       # Image views
│   │   ├── pods/         # Pod views
│   │   ├── services/     # Service views
│   │   ├── deployments/  # Deployment views
│   │   ├── machines/     # Machine + host views
│   │   └── ...
│   └── themes/           # Theme definitions
├── assets/               # App icons and resources
├── scripts/              # Build and release scripts
└── justfile              # Task runner commands
```

## License

MIT
