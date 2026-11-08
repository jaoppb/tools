#!/usr/bin/env bash
# Move the CHANGELOG "Unreleased" section under a new version heading.
#
# Usage: rotate-changelog.sh <X.Y.Z> <YYYY-MM-DD> [file]   (file defaults to CHANGELOG.md)
#
# Fails, leaving the file untouched, when Unreleased is missing or empty, still has a
# TODO(bump) marker, or the version already has a section. Also moves the compare links:
# `[Unreleased]` now compares from the new tag and `[X.Y.Z]` compares from the previous one.
set -euo pipefail

die() {
    echo "Error: $*" >&2
    exit 1
}

[ $# -ge 2 ] && [ $# -le 3 ] || die "Usage: $0 <X.Y.Z> <YYYY-MM-DD> [file]"
VERSION="$1"
DATE="$2"
FILE="${3:-CHANGELOG.md}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "'$VERSION' is not X.Y.Z."
[[ "$DATE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || die "'$DATE' is not YYYY-MM-DD."
[ -f "$FILE" ] || die "$FILE not found."

grep -qxF "## [Unreleased]" "$FILE" || die "$FILE has no '## [Unreleased]' heading."
if grep -qF "## [$VERSION]" "$FILE"; then
    die "$FILE already has a section for $VERSION."
fi

# The Unreleased body: everything between its heading and the next version heading or link block.
body="$(awk '
    /^## \[Unreleased\]$/ { inside = 1; next }
    inside && (/^## \[/ || /^\[[^]]+\]: /) { exit }
    inside { print }
' "$FILE")"
if grep -qF "TODO(bump)" <<<"$body"; then
    die "Unreleased still has a TODO(bump) marker: resolve it first."
fi
# Content is anything but blank lines and one-line HTML comments.
if ! grep -qvE '^[[:space:]]*$|^[[:space:]]*<!--.*-->[[:space:]]*$' <<<"$body"; then
    die "Unreleased is empty: nothing to release."
fi

previous="$(sed -nE 's|^\[Unreleased\]: .*/compare/v([0-9]+\.[0-9]+\.[0-9]+)\.\.\.HEAD$|\1|p' "$FILE")"
[ -n "$previous" ] || die "$FILE has no '[Unreleased]: <repo>/compare/vX.Y.Z...HEAD' link."
repo_url="$(sed -nE 's|^\[Unreleased\]: (.*)/compare/.*$|\1|p' "$FILE")"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
awk -v version="$VERSION" -v date="$DATE" -v previous="$previous" -v url="$repo_url" '
    /^## \[Unreleased\]$/ { print; print ""; print "## [" version "] - " date; next }
    /^\[Unreleased\]: / {
        print "[Unreleased]: " url "/compare/v" version "...HEAD"
        print "[" version "]: " url "/compare/v" previous "...v" version
        next
    }
    { print }
' "$FILE" >"$tmp"
cat "$tmp" >"$FILE"
