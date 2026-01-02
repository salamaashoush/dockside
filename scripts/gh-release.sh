#!/usr/bin/env bash
set -euo pipefail

# Create GitHub release with packaged binaries
# Usage: ./scripts/gh-release.sh <version>

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
fi

# Ensure version has v prefix for tag
TAG="v$VERSION"

echo "Creating GitHub release for $TAG..."

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo "Error: GitHub CLI (gh) is not installed"
    echo "Install with: brew install gh"
    exit 1
fi

# Check if authenticated
if ! gh auth status &> /dev/null; then
    echo "Error: Not authenticated with GitHub CLI"
    echo "Run: gh auth login"
    exit 1
fi

# Check if tag exists
if ! git rev-parse "$TAG" &> /dev/null; then
    echo "Error: Tag $TAG does not exist"
    echo "Create it with: git tag -a $TAG -m \"Release $TAG\""
    exit 1
fi

# Check if dist directory exists and has files
DIST_DIR="dist"
if [[ ! -d "$DIST_DIR" ]] || [[ -z "$(ls -A "$DIST_DIR" 2>/dev/null)" ]]; then
    echo "Error: No packages found in $DIST_DIR/"
    echo "Run: just package $VERSION"
    exit 1
fi

# Generate release notes from changelog or commits
NOTES_FILE=$(mktemp)
trap "rm -f $NOTES_FILE" EXIT

if [[ -f "CHANGELOG.md" ]]; then
    # Extract notes for this version from changelog
    awk "/^## \[$VERSION\]|^## $VERSION/{found=1; next} /^## /{found=0} found" CHANGELOG.md > "$NOTES_FILE"
fi

# If no notes from changelog, use git log
if [[ ! -s "$NOTES_FILE" ]]; then
    echo "## What's Changed" > "$NOTES_FILE"
    echo "" >> "$NOTES_FILE"

    # Get previous tag
    PREV_TAG=$(git describe --tags --abbrev=0 "$TAG^" 2>/dev/null || echo "")

    if [[ -n "$PREV_TAG" ]]; then
        git log --pretty=format:"- %s" "$PREV_TAG..$TAG" >> "$NOTES_FILE"
    else
        git log --pretty=format:"- %s" "$TAG" >> "$NOTES_FILE"
    fi
fi

# Add installation instructions
cat >> "$NOTES_FILE" << 'EOF'

## Installation

### Quick Install (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/salamaashoush/deckhand/main/install.sh | bash
```

### Manual Installation

Download the appropriate binary for your platform:

| Platform | File |
|----------|------|
| macOS (Apple Silicon) | `deckhand-vX.X.X-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `deckhand-vX.X.X-x86_64-apple-darwin.tar.gz` |
| macOS (Universal) | `deckhand-vX.X.X-universal-apple-darwin.tar.gz` |
| macOS App Bundle | `Deckhand-vX.X.X.app.zip` |

### Build from Source

```bash
git clone https://github.com/sashoush/deckhand
cd deckhand
cargo build --release
```
EOF

echo ""
echo "Release notes:"
echo "=============="
cat "$NOTES_FILE"
echo ""
echo "=============="
echo ""

# Confirm before creating
read -p "Create release $TAG with these notes? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled"
    exit 0
fi

# Create the release
echo "Creating release..."

gh release create "$TAG" \
    --title "Deckhand $TAG" \
    --notes-file "$NOTES_FILE" \
    "$DIST_DIR"/*

echo ""
echo "Release created: https://github.com/salamaashoush/deckhand/releases/tag/$TAG"
