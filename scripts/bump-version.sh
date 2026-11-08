#!/usr/bin/env bash
# Open a pull request that bumps the workspace version.
#
# Usage: bump-version.sh <major|minor|patch|X.Y.Z> --issue <n> [--dry-run]
#
# Never pushes to main and never tags: the merged bump is tagged and released by
# .github/workflows/tag-release.yml.
set -euo pipefail

usage() {
    echo "Usage: $0 <major|minor|patch|X.Y.Z> --issue <n> [--dry-run]" >&2
    exit 1
}

die() {
    echo "Error: $*" >&2
    exit 1
}

TARGET=""
ISSUE=""
DRY_RUN=0
while [ $# -gt 0 ]; do
    case "$1" in
        --issue)
            [ $# -ge 2 ] || usage
            ISSUE="$2"
            shift 2
            ;;
        --issue=*)
            ISSUE="${1#--issue=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -*)
            usage
            ;;
        *)
            [ -z "$TARGET" ] || usage
            TARGET="$1"
            shift
            ;;
    esac
done
[ -n "$TARGET" ] || usage
[[ "$ISSUE" =~ ^[0-9]+$ ]] || die "--issue <n> is required and must be an issue number (file the bump issue first)."

BRANCH="$(git branch --show-current)"
if [ "$BRANCH" != "main" ] && [ "${ALLOW_NON_MAIN:-0}" != "1" ]; then
    echo "Error: Version bump must start from the 'main' branch (currently on '$BRANCH')." >&2
    echo "Set ALLOW_NON_MAIN=1 to override." >&2
    exit 1
fi

CURRENT_VERSION="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"
if [ -z "$CURRENT_VERSION" ] || [ "$CURRENT_VERSION" = "null" ]; then
    die "Could not determine current version from Cargo.toml via cargo metadata."
fi

if [[ "$CURRENT_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(.*)$ ]]; then
    MAJOR="${BASH_REMATCH[1]}"
    MINOR="${BASH_REMATCH[2]}"
    PATCH="${BASH_REMATCH[3]}"
else
    die "Current version '$CURRENT_VERSION' does not follow semver format (X.Y.Z)."
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
            die "Invalid target '$TARGET'. Expected 'major', 'minor', 'patch', or a semver string in format X.Y.Z (e.g., 1.2.3)."
        fi
        ;;
esac

TAG="v$NEW_VERSION"
BUMP_BRANCH="chore/$ISSUE"
TITLE="chore: bump version to $NEW_VERSION (#$ISSUE)"
BODY="Bumps the workspace version from $CURRENT_VERSION to $NEW_VERSION. Merging this PR tags $TAG and publishes the release.

Closes #$ISSUE"

if git rev-parse --verify "refs/tags/$TAG" >/dev/null 2>&1; then
    die "Tag '$TAG' already exists."
fi
if git rev-parse --verify "refs/heads/$BUMP_BRANCH" >/dev/null 2>&1; then
    die "Branch '$BUMP_BRANCH' already exists locally."
fi

if [ "$DRY_RUN" = "1" ]; then
    echo "Dry run: nothing is changed."
    echo "Version: $CURRENT_VERSION -> $NEW_VERSION (tag $TAG is created after the PR merges)"
    echo "Branch:  $BUMP_BRANCH"
    echo "Commit:  $TITLE"
    printf 'PR title: %s\nPR body:\n%s\n' "$TITLE" "$BODY"
    exit 0
fi

command -v gh >/dev/null 2>&1 || die "The GitHub CLI 'gh' is required to open the pull request."
gh auth status >/dev/null 2>&1 || die "'gh' is not authenticated. Run 'gh auth login' first."
if git ls-remote --exit-code --heads origin "$BUMP_BRANCH" >/dev/null 2>&1; then
    die "Branch '$BUMP_BRANCH' already exists on origin."
fi
if [ -n "$(git status --porcelain)" ]; then
    die "Working tree is not clean. Commit or stash changes before bumping version."
fi

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

# Rollback changes if a step before the commit fails
cleanup() {
    rm -f Cargo.toml.tmp
    git restore --staged --worktree . 2>/dev/null || true
}
trap cleanup ERR

# Portable in-place replacement (works across GNU and BSD/macOS sed)
sed "/\[workspace\.package\]/,/^\[/ s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

cargo check --workspace

# Release-file hook: steps that edit other tracked files for the release (for
# example rotating the CHANGELOG "Unreleased" section) go here and add their
# files to RELEASE_FILES.
RELEASE_FILES=(Cargo.toml Cargo.lock)

git switch -c "$BUMP_BRANCH"
git add "${RELEASE_FILES[@]}"
git commit -m "$TITLE" -m "Closes #$ISSUE"

trap - ERR

git push -u origin "$BUMP_BRANCH"
gh pr create --base main --head "$BUMP_BRANCH" --title "$TITLE" --body "$BODY"

echo "Opened the bump PR on $BUMP_BRANCH. Merging it tags $TAG and publishes the release."
