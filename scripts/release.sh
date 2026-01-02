#!/usr/bin/env bash
set -euo pipefail

# Full release workflow
# Usage: ./scripts/release.sh [major|minor|patch]

TYPE="${1:-patch}"

echo "Starting release process..."

# 1. Bump version
./scripts/bump-version.sh "$TYPE"

# Get new version
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Releasing version: $VERSION"

# 2. Generate changelog
if command -v git-cliff &> /dev/null; then
    echo "Generating changelog..."
    git-cliff --tag "v$VERSION" -o CHANGELOG.md
fi

# 3. Commit changes
echo "Committing version bump..."
git add Cargo.toml Cargo.lock CHANGELOG.md 2>/dev/null || git add Cargo.toml Cargo.lock
git commit -m "chore: release v$VERSION"

# 4. Create tag
echo "Creating tag v$VERSION..."
git tag -a "v$VERSION" -m "Release v$VERSION"

# 5. Push to remote
echo "Pushing to remote..."
git push origin main
git push origin "v$VERSION"

echo ""
echo "Release v$VERSION created!"
echo ""
echo "Next steps:"
echo "  1. Build release: just build-release"
echo "  2. Package: just package $VERSION"
echo "  3. Create GitHub release: just gh-release $VERSION"
echo ""
echo "Or run: just release-all $TYPE"
