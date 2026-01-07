#!/usr/bin/env bash
set -euo pipefail

# Release workflow - bumps version, generates changelog, commits and pushes
# CI will handle building and creating the GitHub release
#
# Usage: ./scripts/release.sh [major|minor|patch]

TYPE="${1:-patch}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check for required tools
check_requirements() {
    if ! command -v git-cliff &> /dev/null; then
        error "git-cliff is not installed. Install with: cargo install git-cliff"
    fi

    if ! git diff --quiet 2>/dev/null; then
        error "Working directory has uncommitted changes. Commit or stash them first."
    fi
}

# Get current version from Cargo.toml
get_version() {
    grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

# Get previous tag for changelog generation
get_previous_tag() {
    git describe --tags --abbrev=0 2>/dev/null || echo ""
}

# Main release process
main() {
    info "Starting release process (type: $TYPE)..."

    # Step 0: Check requirements
    check_requirements

    # Step 1: Bump version
    info "Step 1/5: Bumping version..."
    "$SCRIPT_DIR/bump-version.sh" "$TYPE"

    VERSION=$(get_version)
    TAG="v$VERSION"

    # Check if tag already exists
    if git rev-parse "$TAG" &> /dev/null; then
        error "Tag $TAG already exists. Choose a different version or delete the existing tag."
    fi

    success "Version bumped to $VERSION"

    # Step 2: Get previous tag for changelog range
    info "Step 2/5: Determining changelog range..."
    PREV_TAG=$(get_previous_tag)

    if [[ -n "$PREV_TAG" ]]; then
        info "Previous tag: $PREV_TAG"
        RANGE="$PREV_TAG..HEAD"
    else
        info "No previous tags found, generating full changelog"
        RANGE=""
    fi

    # Step 3: Generate changelog
    info "Step 3/5: Generating changelog..."

    if [[ -n "$RANGE" ]]; then
        git-cliff "$RANGE" --tag "$TAG" --prepend CHANGELOG.md
    else
        git-cliff --tag "$TAG" -o CHANGELOG.md
    fi

    success "Changelog updated"

    # Step 4: Commit changes
    info "Step 4/5: Committing changes..."
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): $TAG"

    # Create annotated tag
    git tag -a "$TAG" -m "Release $TAG"

    success "Created commit and tag $TAG"

    # Step 5: Push to remote
    info "Step 5/5: Pushing to remote..."
    git push origin main
    git push origin "$TAG"

    success "Pushed to remote"

    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  Release $TAG complete!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "CI will now build and create the GitHub release."
    echo "Watch progress at: https://github.com/salamaashoush/dockside/actions"
    echo ""
}

main
