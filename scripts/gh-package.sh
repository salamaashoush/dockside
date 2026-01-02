#!/usr/bin/env bash
set -euo pipefail

# Package binaries for GitHub release
# Usage: ./scripts/gh-package.sh <version>

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
fi

echo "Packaging version: $VERSION"

# Create dist directory
DIST_DIR="dist"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Package macOS ARM64
if [[ -f "target/aarch64-apple-darwin/release/deckhand" ]]; then
    echo "Packaging macOS ARM64..."
    tar -czvf "$DIST_DIR/deckhand-v$VERSION-aarch64-apple-darwin.tar.gz" \
        -C target/aarch64-apple-darwin/release deckhand
fi

# Package macOS x64
if [[ -f "target/x86_64-apple-darwin/release/deckhand" ]]; then
    echo "Packaging macOS x64..."
    tar -czvf "$DIST_DIR/deckhand-v$VERSION-x86_64-apple-darwin.tar.gz" \
        -C target/x86_64-apple-darwin/release deckhand
fi

# Package universal binary
if [[ -f "target/universal/release/deckhand" ]]; then
    echo "Packaging macOS Universal..."
    tar -czvf "$DIST_DIR/deckhand-v$VERSION-universal-apple-darwin.tar.gz" \
        -C target/universal/release deckhand
fi

# Package current platform release (fallback)
if [[ -f "target/release/deckhand" ]]; then
    ARCH=$(uname -m)
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')

    if [[ "$ARCH" == "arm64" ]]; then
        ARCH="aarch64"
    fi

    if [[ "$OS" == "darwin" ]]; then
        TARGET="$ARCH-apple-darwin"
    else
        TARGET="$ARCH-unknown-linux-gnu"
    fi

    # Only package if cross-compiled version doesn't exist
    if [[ ! -f "$DIST_DIR/deckhand-v$VERSION-$TARGET.tar.gz" ]]; then
        echo "Packaging current platform ($TARGET)..."
        tar -czvf "$DIST_DIR/deckhand-v$VERSION-$TARGET.tar.gz" \
            -C target/release deckhand
    fi
fi

# Package macOS app bundle if exists
if [[ -d "target/release/Deckhand.app" ]]; then
    echo "Packaging macOS App Bundle..."
    (cd target/release && zip -r "../../$DIST_DIR/Deckhand-v$VERSION.app.zip" Deckhand.app)
fi

echo ""
echo "Packages created in $DIST_DIR/:"
ls -la "$DIST_DIR/"
