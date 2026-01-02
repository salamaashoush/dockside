# Deckhand

A native macOS desktop application for managing Docker containers, images, volumes, networks, and Kubernetes resources via Colima.

Built with [GPUI](https://gpui.rs) for a fast, native experience.

## Features

- **Docker Management**: Containers, images, volumes, networks
- **Kubernetes Support**: Pods, services, deployments via Colima
- **Native Performance**: Built with Rust and GPUI framework
- **Real-time Updates**: Live status monitoring
- **Terminal Integration**: Exec into containers directly
- **Log Viewing**: Stream container and pod logs

## Requirements

- macOS 13.0+
- [Colima](https://github.com/abiosoft/colima) (container runtime)
- Docker CLI

## Installation

### Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/salamaashoush/deckhand/main/install.sh | bash
```

This will install Deckhand and optionally set up Docker and Colima if not already installed.

### Manual Installation

Download the latest release from the [Releases page](https://github.com/salamaashoush/deckhand/releases).

**macOS App Bundle:**
1. Download `Deckhand-vX.X.X.app.zip`
2. Extract and move to `/Applications`

**CLI Binary:**
1. Download `deckhand-vX.X.X-aarch64-apple-darwin.tar.gz` (Apple Silicon) or `deckhand-vX.X.X-x86_64-apple-darwin.tar.gz` (Intel)
2. Extract: `tar -xzf deckhand-*.tar.gz`
3. Move to PATH: `mv deckhand ~/.local/bin/`

### Build from Source

```bash
# Clone the repo
git clone https://github.com/salamaashoush/deckhand
cd deckhand

# Install dependencies
just install-deps

# Build and run
just run

# Or build release
just build-release

# Create app bundle
just bundle
```

## Usage

### Starting Colima

Before using Deckhand, start Colima:

```bash
# Standard Docker runtime
colima start --cpu 4 --memory 8 --disk 60

# With Kubernetes support
colima start --cpu 4 --memory 8 --disk 60 --kubernetes
```

Or use the justfile:

```bash
just start-colima      # Standard
just start-colima-k8s  # With Kubernetes
```

### Running Deckhand

```bash
# Run from source
just run

# Or run installed binary
deckhand

# Or open the app bundle
open /Applications/Deckhand.app
```

## Development

### Prerequisites

- Rust 1.85+
- [just](https://github.com/casey/just) command runner

### Setup

```bash
# Install development tools
just init

# Run in development mode
just run

# Run with debug logging
just run-debug

# Watch for changes
just watch
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
just build-macos-universal # Build universal binary
just bundle                # Create .app bundle
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
deckhand/
├── src/
│   ├── main.rs           # Entry point
│   ├── app.rs            # Main application
│   ├── docker/           # Docker client and types
│   ├── kubernetes/       # Kubernetes client and types
│   ├── state/            # Application state management
│   ├── services/         # Background services
│   ├── ui/               # UI components
│   │   ├── containers/   # Container views
│   │   ├── images/       # Image views
│   │   ├── pods/         # Pod views
│   │   ├── services/     # Service views
│   │   ├── deployments/  # Deployment views
│   │   └── ...
│   └── themes/           # Theme definitions
├── assets/               # App icons and resources
├── scripts/              # Build and release scripts
└── justfile              # Task runner commands
```

## License

MIT
