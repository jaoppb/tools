#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-}"
if [ -z "$TARGET" ]; then
    echo "Usage: $0 <major|minor|patch|X.Y.Z>" >&2
    exit 1
fi

BRANCH="$(git branch --show-current)"
if [ "$BRANCH" != "main" ] && [ "${ALLOW_NON_MAIN:-0}" != "1" ]; then
    echo "Error: Version bump must be performed on the 'main' branch (currently on '$BRANCH')." >&2
    echo "Set ALLOW_NON_MAIN=1 to override." >&2
    exit 1
fi

CURRENT_VERSION="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"
if [ -z "$CURRENT_VERSION" ] || [ "$CURRENT_VERSION" = "null" ]; then
    echo "Error: Could not determine current version from Cargo.toml via cargo metadata." >&2
    exit 1
fi

if [[ "$CURRENT_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(.*)$ ]]; then
    MAJOR="${BASH_REMATCH[1]}"
    MINOR="${BASH_REMATCH[2]}"
    PATCH="${BASH_REMATCH[3]}"
else
    echo "Error: Current version '$CURRENT_VERSION' does not follow semver format (X.Y.Z)." >&2
    exit 1
fi

case "$TARGET" in
    major)
        NEW_VERSION="$((MAJOR + 1)).0.0"
        ;;
    minor)
        NEW_VERSION="${MAJOR}.$((MINOR + 1)).0"
        ;;
    patch)
        NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))"
        ;;
    *)
        if [[ "$TARGET" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            NEW_VERSION="$TARGET"
        else
            echo "Error: Invalid target '$TARGET'. Expected 'major', 'minor', 'patch', or a semver string in format X.Y.Z (e.g., 1.2.3)." >&2
            exit 1
        fi
        ;;
esac

TAG="v$NEW_VERSION"
if git rev-parse --verify "refs/tags/$TAG" >/dev/null 2>&1; then
    echo "Error: Tag '$TAG' already exists." >&2
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    echo "Error: Working tree is not clean. Commit or stash changes before bumping version." >&2
    exit 1
fi

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

# Rollback changes if cargo check fails
cleanup() {
    rm -f Cargo.toml.tmp
    git restore Cargo.toml Cargo.lock 2>/dev/null || true
}
trap cleanup ERR

# Portable in-place replacement (works across GNU and BSD/macOS sed)
sed "/\[workspace\.package\]/,/^\[/ s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

cargo check --workspace

trap - ERR

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NEW_VERSION"
git tag -a "$TAG" -m "$TAG"
git push origin HEAD --follow-tags

echo "Successfully bumped version to $NEW_VERSION and pushed commit + tag $TAG"
