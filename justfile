# Dockside - Docker & Kubernetes Desktop Manager
# justfile for development, building, and releasing

set shell := ["bash", "-cu"]
set dotenv-load

# Default recipe
default:
    @just --list

# ==================== Setup ====================

# Install all development tools
install-tools:
    @echo "Installing development tools..."
    cargo install cargo-binstall || true
    cargo binstall -y cargo-nextest
    cargo binstall -y cargo-watch
    cargo binstall -y cargo-audit
    cargo binstall -y cargo-deny
    cargo binstall -y cargo-outdated
    cargo binstall -y cargo-machete
    cargo binstall -y typos-cli
    cargo binstall -y git-cliff
    cargo binstall -y dprint
    @echo "Tools installed!"

# Initialize project (first time setup)
init: install-tools
    @echo "Project initialized!"

# ==================== Development ====================

# Run cargo check (fast compilation check)
check:
    cargo check

# Run the app in development mode
run:
    cargo run

# Run the app with RUST_LOG enabled
run-debug:
    RUST_LOG=debug cargo run

# Watch for changes and rebuild
watch:
    cargo watch -x run

# ==================== Testing ====================

# Run all tests
test:
    cargo nextest run

# Run tests with output
test-verbose:
    cargo nextest run --no-capture

# Run a specific test
test-one NAME:
    cargo nextest run {{NAME}}

# ==================== Linting & Formatting ====================

# Format code
fmt:
    cargo fmt

# Check formatting without changes
fmt-check:
    cargo fmt -- --check

# Run clippy lints
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Fix clippy warnings automatically
lint-fix:
    cargo clippy --fix --allow-dirty --allow-staged

# Spellcheck
typos:
    typos

# Fix typos automatically
typos-fix:
    typos -w

# Find unused dependencies
shear:
    cargo machete

# Full code quality check
fix: fmt lint-fix typos-fix
    @echo "All fixes applied!"

# ==================== Building ====================

# Build debug version
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Build with native CPU optimizations
build-native:
    RUSTFLAGS="-C target-cpu=native" cargo build --release

# Build small release (for distribution)
build-small:
    cargo build --profile release-small

# Build with debug info (for profiling)
build-debug:
    cargo build --profile release-with-debug

# ==================== macOS App Bundle ====================

# Generate app icon from source image
icon SOURCE:
    ./scripts/generate-icon.sh {{SOURCE}}

# Generate placeholder icon for development
icon-placeholder:
    ./scripts/generate-placeholder-icon.sh

# Create macOS app bundle
bundle: build-release
    @echo "Creating Dockside.app bundle..."
    @mkdir -p target/release/Dockside.app/Contents/MacOS
    @mkdir -p target/release/Dockside.app/Contents/Resources
    @cp target/release/dockside target/release/Dockside.app/Contents/MacOS/
    @cp assets/Info.plist target/release/Dockside.app/Contents/
    @cp assets/AppIcon.icns target/release/Dockside.app/Contents/Resources/ 2>/dev/null || echo "No icon found - run 'just icon-placeholder' first"
    @cp -R themes target/release/Dockside.app/Contents/Resources/
    @echo "Bundle created at target/release/Dockside.app"

# Install app to /Applications
install-app: bundle
    @echo "Installing Dockside.app to /Applications..."
    @rm -rf /Applications/Dockside.app
    @cp -R target/release/Dockside.app /Applications/
    @echo "Dockside.app installed to /Applications"

# ==================== Cross Compilation ====================

# Build for macOS ARM64 (Apple Silicon)
build-macos-arm64:
    cargo build --release --target aarch64-apple-darwin

# ==================== Security ====================

# Run security audit
audit:
    cargo audit

# Check dependencies for issues
deny:
    cargo deny check

# Full security check
security: audit
    @echo "Security checks passed!"

# ==================== Documentation ====================

# Generate documentation
doc:
    cargo doc --no-deps

# Open documentation in browser
doc-open:
    cargo doc --no-deps --open

# ==================== Release ====================

# Get current version from Cargo.toml
version:
    @grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'

# Bump version only (usage: just bump patch|minor|major)
bump TYPE:
    ./scripts/bump-version.sh {{TYPE}}

# Generate changelog for unreleased changes
changelog:
    git-cliff --unreleased

# Preview what the next release would look like
release-preview TYPE:
    #!/usr/bin/env bash
    ./scripts/bump-version.sh {{TYPE}} --dry-run 2>/dev/null || true
    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
    echo "Would release: v$VERSION"
    echo ""
    echo "Changelog preview:"
    git-cliff --unreleased --tag "v$VERSION"

# Create a new release (bumps version, updates changelog, commits, pushes)
# CI will automatically build and create GitHub release
release TYPE:
    ./scripts/release.sh {{TYPE}}

# ==================== CI ====================

# Run all CI checks (use before committing)
ready: fmt-check lint test
    @echo "All checks passed! Ready to commit."

# Quick check (faster than ready)
quick: check lint
    @echo "Quick checks passed!"

# ==================== Cleanup ====================

# Clean build artifacts
clean:
    cargo clean

# Clean and rebuild
rebuild: clean build

# ==================== Dependencies ====================

# Check for outdated dependencies
outdated:
    cargo outdated

# Update dependencies
update:
    cargo update

# ==================== Installation ====================

# Install the binary locally
install:
    cargo install --path .

# Uninstall the binary
uninstall:
    cargo uninstall dockside

# Install dependencies (Docker, Colima) on macOS
install-deps:
    @echo "Installing Docker and Colima..."
    @which brew > /dev/null || (echo "Homebrew not found. Please install it first." && exit 1)
    brew install docker docker-compose colima
    @echo "Dependencies installed!"
    @echo "Run 'colima start' to start the container runtime"

# Start Colima with default settings
start-colima:
    colima start --cpu 4 --memory 8 --disk 60

# Start Colima with Kubernetes
start-colima-k8s:
    colima start --cpu 4 --memory 8 --disk 60 --kubernetes
