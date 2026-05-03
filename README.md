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

- macOS arm64 → `/Applications/Dockside.app` (quarantine xattr stripped). The privileged `dockside-helper` lives inside `Contents/MacOS/` next to the main app — keep them together.
- Linux x86_64 → `~/.local/bin/dockside` (an AppImage with the helper bundled inside).

Pin a version: `./scripts/install.sh v0.3.1`.

### Manual install

All artifacts on the [Releases page](https://github.com/salamaashoush/dockside/releases) are produced by [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager):

- **macOS (Apple Silicon)** — `Dockside_X.Y.Z_aarch64.dmg` (drag to `/Applications`) or `Dockside-vX.Y.Z-macos-arm64.zip` (zipped `.app`).
- **Linux (x86_64) AppImage** — `Dockside_X.Y.Z_x86_64.AppImage`. `chmod +x`, run anywhere.
- **Linux (Debian / Ubuntu)** — `Dockside_X.Y.Z_amd64.deb`. `sudo dpkg -i …` auto-installs the polkit policy at `/usr/share/polkit-1/actions/dev.dockside.helper.policy`, so the in-app **Set up Local DNS** step is skipped.
- **Linux (Arch / CachyOS / Manjaro)** — `PKGBUILD` + matching `Dockside-X.Y.Z.tar.gz`. Drop both into the same directory and run `makepkg -si`. Same polkit policy install as `.deb`.

### First-time DNS setup

1. **Settings → Local DNS → Set up Local DNS** — one polkit / Touch ID prompt. Installs the helper to `/usr/local/libexec/dockside-helper` (AppImage / tarball) or recognises the package-installed copy at `/usr/bin/dockside-helper` (`.deb`), registers the system resolver for `*.dockside.test`, trusts the local root CA.
2. **Settings → Local DNS → Drop port from URL** *(optional)* — Linux installs an `nftables` NAT redirect plus a `dockside-port-redirect.service` systemd unit so `http://name.dockside.test/` works without a `:47080` suffix. macOS installs an equivalent `pf` redirect rule. Both persist across reboots and rebuilds.

## Build from source

```bash
git clone https://github.com/salamaashoush/dockside
cd dockside

# One-shot: install Rust + Zig 0.15.2 + system libs for your host
just install-build-deps     # or: bash scripts/install-deps.sh

# Run / build
just run                    # debug
just build-release          # release binary

# Native installers (cargo-packager). Install once: cargo install cargo-packager --locked
just package-macos          # Dockside.app + .dmg
just package-linux          # .deb + .AppImage
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
