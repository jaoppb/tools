#!/usr/bin/env bash
# Make the repository-relative links of Markdown read on stdin absolute, for GitHub release notes.
#
# Usage: release-notes-links.sh <tag> <owner/repo> < CHANGELOG.md > notes.md
#
# A release page resolves a relative link against /<owner>/<repo>/releases/tag/, so
# `[guide](docs/migrations/v0.3.0.md)` breaks there while it works in CHANGELOG.md's file view.
# Inline links `[text](path)` and reference definitions `[id]: path` whose target has no URL
# scheme and is not a bare `#anchor` become https://github.com/<owner>/<repo>/blob/<tag>/<path>
# (images `![alt](path)` use raw/ instead of blob/). A leading `/` means the repository root and a
# `?query` or `#fragment` suffix is kept. Fenced code blocks and `code spans` are left alone.
#
# Each rewritten path that does not exist under the current directory (the tagged checkout) gets
# a `::warning::` on stderr; the output is still written and the exit status stays 0.
set -euo pipefail

die() {
    echo "Error: $*" >&2
    exit 1
}

[ $# -eq 2 ] || die "Usage: $0 <tag> <owner/repo>"
TAG="$1"
REPO="$2"
[[ "$TAG" =~ ^[A-Za-z0-9._/-]+$ ]] || die "'$TAG' is not a valid tag."
[[ "$REPO" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]] || die "'$REPO' is not <owner>/<repo>."

paths="$(mktemp)"
trap 'rm -f "$paths"' EXIT

awk -v base="https://github.com/$REPO" -v tag="$TAG" -v paths="$paths" '
    # A target to rewrite: not empty, no scheme (http:, mailto:, ...), not //host, not #anchor.
    function relative(t) {
        return t != "" && t !~ /^[A-Za-z][A-Za-z0-9+.-]*:/ && t !~ /^\/\// && t !~ /^#/ && t !~ /^</
    }
    function absolute(t, kind,    p) {
        sub(/^(\.\/|\/)+/, "", t)
        p = t
        sub(/[?#].*$/, "", p)
        if (p != "") print p > paths
        return base "/" kind "/" tag "/" t
    }
    # Rewrite every inline link and image target in s, a stretch of text outside code spans.
    function inline(s,    out, pre, t, kind) {
        out = ""
        while (match(s, /\]\([^)[:space:]]*/)) {
            pre = substr(s, 1, RSTART + 1)
            t = substr(s, RSTART + 2, RLENGTH - 2)
            s = substr(s, RSTART + RLENGTH)
            if (relative(t)) {
                kind = "blob"
                # An image when the matching "[" opens with "!": find the last "[" before "](".
                if (match(pre, /!\[[^]]*\]\($/)) kind = "raw"
                t = absolute(t, kind)
            }
            out = out pre t
        }
        return out s
    }
    /^[[:space:]]*(```|~~~)/ { fenced = !fenced; print; next }
    fenced { print; next }
    # Reference definition: up to three spaces, [id]:, whitespace, target.
    match($0, /^ ? ? ?\[[^]]+\]:[ \t]+/) {
        head = substr($0, 1, RLENGTH)
        rest = substr($0, RLENGTH + 1)
        t = rest
        sub(/[ \t].*$/, "", t)
        if (relative(t)) {
            print head absolute(t, "blob") substr(rest, length(t) + 1)
            next
        }
    }
    {
        # Odd pieces lie outside `code spans`, even pieces inside.
        n = split($0, piece, "`")
        line = ""
        for (i = 1; i <= n; i++) {
            line = line (i > 1 ? "`" : "") (i % 2 ? inline(piece[i]) : piece[i])
        }
        print line
    }
'

while IFS= read -r p; do
    [ -e "$p" ] || echo "::warning::release notes link to '$p', which does not exist at $TAG" >&2
done < <(sort -u "$paths")
