# Dockside

Native desktop app for managing Docker containers, images, volumes, networks, and Kubernetes resources via Colima or native Docker.

Built with [GPUI](https://gpui.rs) for a fast, native experience on **macOS (Apple Silicon)** and **Linux (x86_64)**.

> Windows and Intel macOS are not supported.

## Features

- **Docker management**: containers, images, volumes, networks, compose
- **Kubernetes**: pods, services, deployments
- **Embedded terminal**: `docker exec`, container logs, `kubectl exec`
- **Image vulnerability scanning** via Trivy
- **Dockerfile linting** via Hadolint
- **Live stats**: CPU / memory / network / disk sparklines
- **Compose**: project-level start/stop/restart + `docker compose watch` streaming
- **Themes**: dozens of bundled themes, hot-reloaded
- **Settings**: theme, terminal font, refresh intervals, kubeconfig override, Colima defaults, …

## Requirements

- **macOS 13.0+** on Apple Silicon, or **Linux** with glibc 2.31+
- A Docker runtime:
  - macOS: [Colima](https://github.com/abiosoft/colima) (managed in-app)
  - Linux: native `dockerd` (or Colima)
- `docker` CLI on PATH
- Optional: `kubectl` for Kubernetes panels, `trivy` for image scanning, `hadolint` for Dockerfile linting

## Installation

### One-line install (latest release)

```bash
curl -fsSL https://raw.githubusercontent.com/salamaashoush/dockside/main/scripts/install.sh | bash
```

- macOS arm64 → `/Applications/Dockside.app` (quarantine xattr stripped)
- Linux x86_64 → `~/.local/bin/dockside` + themes in `~/.local/share/dockside` + `.desktop` entry + icon

Pin a version: `./scripts/install.sh v0.2.0`.

### Manual install

Download the latest archive from the [Releases page](https://github.com/salamaashoush/dockside/releases):

- **macOS arm64**: `Dockside-vX.Y.Z-macos-arm64.zip` → unzip and move `Dockside.app` to `/Applications`
- **Linux x86_64**: `Dockside-vX.Y.Z-linux-x86_64.tar.gz` → extract, copy `dockside` to a directory on PATH, copy `themes/` next to it (or set `DOCKSIDE_THEMES_DIR`)

## Build from source

```bash
git clone https://github.com/salamaashoush/dockside
cd dockside

# One-shot: install Rust + Zig 0.15.2 + system libs for your host
just install-build-deps     # or: bash scripts/install-deps.sh

# Run / build
just run                    # debug
just build-release          # release binary
just bundle                 # macOS .app bundle
```

`scripts/install-deps.sh` auto-detects:

- **macOS**: ensures `brew install pkg-config cmake`, runs `rustup`, downloads pinned Zig
- **Linux**: detects `apt-get` / `dnf` / `pacman` / `zypper` and installs gtk3 + webkit2gtk + libxdo + libappindicator + libxcb + libxkbcommon + openssl + vulkan + alsa development headers, plus Rust + Zig

## Usage

### Starting a Docker runtime

**macOS / Linux (Colima):**

```bash
colima start --cpu 4 --memory 8 --disk 60
# With Kubernetes
colima start --cpu 4 --memory 8 --disk 60 --kubernetes
```

Or via justfile: `just start-colima` / `just start-colima-k8s`.

**Linux (native Docker):** ensure `dockerd` is running and `/var/run/docker.sock` is reachable.

### Running Dockside

```bash
# Run from source
just run

# Run installed binary
dockside

# macOS bundle
open /Applications/Dockside.app
```

## Development

### Prerequisites

- Rust 1.91+
- [Zig](https://ziglang.org/) 0.15.2 (vendored Ghostty C core via `libghostty-vt-sys`)
- [just](https://github.com/casey/just) command runner

`just install-build-deps` installs all of the above.

### Common commands

```bash
just              # list recipes
just check        # fast compile check
just build        # debug build
just test         # run tests
just lint         # clippy
just fmt          # rustfmt
just ready        # run all CI checks
just watch        # cargo-watch loop
```

### Build / package

```bash
just build-release     # release binary
just build-native      # native CPU opts
just bundle            # macOS .app bundle
just install-app       # install bundle to /Applications
```

### Releasing

```bash
just bump patch|minor|major   # bumps Cargo.toml + lock
just release patch            # bump + changelog + commit + tag + push
```

A pushed `vX.Y.Z` tag triggers `.github/workflows/release.yml`, which:

1. Builds macOS arm64 + Linux x86_64
2. Bundles `Dockside.app` (macOS) and `Dockside-vX.Y.Z-linux-x86_64.tar.gz`
3. Generates release notes from `CHANGELOG.md`
4. Creates the GitHub release with both artifacts

## Project structure

```
dockside/
├── src/
│   ├── main.rs          # entry point + theme bootstrap
│   ├── app.rs           # main application
│   ├── platform/        # platform detection + runtime abstraction
│   ├── docker/          # Docker client and types
│   ├── colima/          # Colima client and machines
│   ├── kubernetes/      # kube client and types
│   ├── state/           # global app state (selection, settings, …)
│   ├── services/        # background tasks + dispatcher
│   ├── terminal/        # libghostty-backed terminal grid + log streams
│   └── ui/              # views: containers, images, volumes, networks,
│                        # pods, services, deployments, machines, compose,
│                        # activity monitor, settings
├── assets/              # icons, Info.plist
├── themes/              # bundled themes (.json)
├── scripts/
│   ├── install.sh       # end-user installer (macOS / Linux)
│   ├── install-deps.sh  # contributor build deps
│   ├── release.sh       # bump + changelog + tag + push
│   └── bump-version.sh  # Cargo.toml version bump
├── .github/workflows/   # CI + release pipelines
└── justfile             # task runner
```

## License

MIT
