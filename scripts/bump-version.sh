#!/usr/bin/env bash
# Open a pull request that bumps the workspace version.
#
# Usage: bump-version.sh <major|minor|patch|X.Y.Z> --issue <n> [--dry-run]
#
# Also rotates the CHANGELOG "Unreleased" section into the new version (see
# rotate-changelog.sh); the bump fails while it is empty.
#
# Runs `just check` and `just issues-check` on the bump commit, then pushes with
# --no-verify: the pre-push hook would repeat those checks while git holds the remote
# connection open, and a slow (cold cache) hook lets an idle SSH connection drop.
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

ROTATE_CHANGELOG="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/rotate-changelog.sh"
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
    die "Version bump must start from the 'main' branch (currently on '$BRANCH'). Set ALLOW_NON_MAIN=1 to override."
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

[[ "$NEW_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] || die "New version '$NEW_VERSION' is not X.Y.Z."
if (( BASH_REMATCH[1] < MAJOR
    || (BASH_REMATCH[1] == MAJOR && BASH_REMATCH[2] < MINOR)
    || (BASH_REMATCH[1] == MAJOR && BASH_REMATCH[2] == MINOR && BASH_REMATCH[3] <= PATCH) )); then
    die "New version $NEW_VERSION must be greater than the current version $CURRENT_VERSION."
fi

TAG="v$NEW_VERSION"
BUMP_BRANCH="chore/$ISSUE"
TITLE="chore: bump version to $NEW_VERSION (#$ISSUE)"
BODY="Bumps the workspace version from $CURRENT_VERSION to $NEW_VERSION. Merging this PR tags $TAG and publishes the release.

Closes #$ISSUE"

command -v gh >/dev/null 2>&1 || die "The GitHub CLI 'gh' is required to open the pull request."
gh auth status >/dev/null 2>&1 || die "'gh' is not authenticated. Run 'gh auth login' first."
if [ -n "$(git status --porcelain)" ]; then
    die "Working tree is not clean. Commit or stash changes before bumping version."
fi

git fetch --quiet --tags origin main
if [ "${ALLOW_NON_MAIN:-0}" != "1" ] && [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
    die "Local 'main' is not at origin/main. Update it first (git pull --ff-only)."
fi
if git rev-parse --verify "refs/tags/$TAG" >/dev/null 2>&1; then
    die "Tag '$TAG' already exists."
fi
if git rev-parse --verify "refs/heads/$BUMP_BRANCH" >/dev/null 2>&1; then
    die "Branch '$BUMP_BRANCH' already exists locally."
fi
if git ls-remote --exit-code --heads origin "$BUMP_BRANCH" >/dev/null 2>&1; then
    die "Branch '$BUMP_BRANCH' already exists on origin."
fi

# The release notes come from CHANGELOG.md: rotate a copy now so an empty Unreleased section,
# a leftover TODO(bump) marker or a missing link fails before anything changes.
RELEASE_DATE="$(date -u +%F)"
[ -f CHANGELOG.md ] || die "CHANGELOG.md not found."
ROTATION_PREVIEW="$(mktemp)"
trap 'rm -f "$ROTATION_PREVIEW"' EXIT
cp CHANGELOG.md "$ROTATION_PREVIEW"
"$ROTATE_CHANGELOG" "$NEW_VERSION" "$RELEASE_DATE" "$ROTATION_PREVIEW" || exit 1

if [ "$DRY_RUN" = "1" ]; then
    echo "Dry run: all preconditions pass, including the CHANGELOG rotation (cargo check, just check and just issues-check are not run), and nothing is changed."
    echo "Version: $CURRENT_VERSION -> $NEW_VERSION (tag $TAG is created after the PR merges)"
    echo "Branch:  $BUMP_BRANCH"
    echo "Commit:  $TITLE"
    printf 'PR title: %s\nPR body:\n%s\n' "$TITLE" "$BODY"
    exit 0
fi

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"

START_SHA="$(git rev-parse HEAD)"
# Roll back to the starting branch with a clean tree if a step before the push fails
cleanup() {
    rm -f Cargo.toml.tmp
    git restore --staged --worktree . 2>/dev/null || true
    if [ "$(git branch --show-current)" = "$BUMP_BRANCH" ]; then
        if [ -n "$BRANCH" ]; then
            git switch --quiet "$BRANCH" 2>/dev/null || true
        else
            git switch --quiet --detach "$START_SHA" 2>/dev/null || true
        fi
        git branch -D "$BUMP_BRANCH" >/dev/null 2>&1 || true
    fi
}
trap cleanup ERR

# Portable in-place replacement (works across GNU and BSD/macOS sed)
sed "/\[workspace\.package\]/,/^\[/ s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

git diff --quiet Cargo.toml && die "Cargo.toml has no [workspace.package] version line to update."

cargo check --workspace

# Release-file hook: steps that edit other tracked files for the release go here and add
# their files to RELEASE_FILES.
"$ROTATE_CHANGELOG" "$NEW_VERSION" "$RELEASE_DATE"
RELEASE_FILES=(Cargo.toml Cargo.lock CHANGELOG.md)

git switch -c "$BUMP_BRANCH"
git add "${RELEASE_FILES[@]}"
git commit -m "$TITLE" -m "Closes #$ISSUE"

# The pre-push hook's checks, run here on the commit being pushed so the push can skip
# the hook (a failure rolls back through the ERR trap).
echo "Running the pre-push checks on the bump commit: just check, just issues-check"
just check
just issues-check

trap - ERR

# Runs "$@" with its stdout untouched and its stderr shown live through tee, keeping a copy
# of the stderr in STEP_STDERR; returns the command's status. Call it only as an `if`
# condition, where errexit is off and a failing pipeline does not exit the script.
STEP_STDERR=""
run_capturing_stderr() {
    local log status
    log="$(mktemp)"
    { "$@" 2>&1 1>&3 3>&- | tee "$log" >&2 3>&-; status="${PIPESTATUS[0]}"; } 3>&1
    STEP_STDERR="$(cat "$log")"
    rm -f "$log"
    return "$status"
}

PR_CREATE_HINT="$(printf 'gh pr create --base main --head %q --title %q --body %q' "$BUMP_BRANCH" "$TITLE" "$BODY")"

if ! run_capturing_stderr git push --no-verify -u origin "$BUMP_BRANCH"; then
    {
        echo "Error: git push of $BUMP_BRANCH failed; no pull request was opened. git said:"
        printf '%s\n' "${STEP_STDERR:-(no output)}"
        echo "Fix the cause and retry with: git push --no-verify -u origin $BUMP_BRANCH && $PR_CREATE_HINT"
        echo "or discard the bump with: git switch $BRANCH && git branch -D $BUMP_BRANCH (and git push origin --delete $BUMP_BRANCH if it reached origin)"
    } >&2
    exit 1
fi

if ! run_capturing_stderr gh pr create --base main --head "$BUMP_BRANCH" --title "$TITLE" --body "$BODY"; then
    {
        echo "Error: $BUMP_BRANCH was pushed, but opening the pull request failed. gh said:"
        printf '%s\n' "${STEP_STDERR:-(no output)}"
        echo "Fix the cause and open it with: $PR_CREATE_HINT"
        echo "or discard the bump with: git switch $BRANCH && git branch -D $BUMP_BRANCH && git push origin --delete $BUMP_BRANCH"
    } >&2
    exit 1
fi

echo "Opened the bump PR on $BUMP_BRANCH. Merging it tags $TAG and publishes the release."
