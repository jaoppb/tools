#!/usr/bin/env bash
# Tests scripts/release-tag.sh against throwaway repositories: a bare `origin` and a
# clone where each case commits Cargo.toml revisions and runs the script.
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/release-tag.sh"
readonly SCRIPT
ROOT="$(mktemp -d)"
readonly ROOT
trap 'rm -rf "${ROOT}"' EXIT

export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.com
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.com
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

failures=0
cases=0
status=0
out=""
output=""

# A fresh origin and clone per case; the clone is the working directory.
setup() {
  local dir="${ROOT}/case${cases}"
  git init --quiet --bare --initial-branch=main "${dir}/origin.git"
  git clone --quiet "${dir}/origin.git" "${dir}/work" 2>/dev/null
  cd "${dir}/work"
  : >"${dir}/output"
  export GITHUB_OUTPUT="${dir}/output"
}

# Commits a Cargo.toml with the given workspace version ('' omits the key) and prints
# the commit. Extra arguments become extra manifest lines, so a commit can change the
# file without changing the version.
commit_version() {
  local version="$1"
  shift
  {
    echo '[workspace]'
    echo 'members = []'
    echo
    echo '[workspace.package]'
    [[ -n ${version} ]] && echo "version = \"${version}\""
    echo 'edition = "2024"'
    local line
    for line in "$@"; do echo "${line}"; done
  } >Cargo.toml
  git add Cargo.toml
  git commit --quiet --allow-empty -m "version ${version:-none}"
  git push --quiet origin HEAD:main
  git rev-parse HEAD
}

run() {
  status=0
  out="$("${SCRIPT}" "$@" 2>&1)" || status=$?
  output="$(cat "${GITHUB_OUTPUT}")"
}

remote_tag() { git ls-remote --tags origin "refs/tags/$1^{}" | cut -f1; }

fail() {
  echo "FAIL [${current}]: $*"
  echo "  status=${status} output='${output}'"
  local line
  while IFS= read -r line; do echo "  | ${line}"; done <<<"${out}"
  failures=$((failures + 1))
}

expect_status() { [[ ${status} -eq $1 ]] || fail "expected exit $1"; }
expect_output() { [[ ${output} == "$1" ]] || fail "expected GITHUB_OUTPUT '$1'"; }
expect_log() { grep -qF -- "$1" <<<"${out}" || fail "expected log containing '$1'"; }
expect_tag_at() { [[ "$(remote_tag "$1")" == "$2" ]] || fail "expected $1 at $2"; }
expect_no_tag() { [[ -z "$(git ls-remote --tags origin "refs/tags/$1")" ]] || fail "expected no tag $1"; }

case_() {
  cases=$((cases + 1))
  current="$1"
  setup
}

# --- push -------------------------------------------------------------------------

case_ "push: a version bump is tagged and released"
before="$(commit_version 0.2.0)"
after="$(commit_version 0.3.0)"
run push "${before}" "${after}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${after}"
[[ "$(git cat-file -t refs/tags/v0.3.0)" == tag ]] || fail "expected an annotated tag"

case_ "push: a multi-commit push is compared end to end and tags its head"
before="$(commit_version 0.2.0)"
commit_version 0.3.0 >/dev/null
after="$(commit_version 0.3.0 '# later edit')"
run push "${before}" "${after}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${after}"

case_ "push: Cargo.toml changed without a version change never tags a missing tag"
before="$(commit_version 0.3.0)"
after="$(commit_version 0.3.0 '# dependency bump')"
run push "${before}" "${after}"
expect_status 0
expect_output ""
expect_no_tag v0.3.0
expect_log "although tag v0.3.0 is missing"

case_ "push: an unchanged version with its tag present does nothing"
before="$(commit_version 0.3.0)"
git tag -a v0.3.0 -m v0.3.0 && git push --quiet origin v0.3.0
after="$(commit_version 0.3.0 '# dependency bump')"
run push "${before}" "${after}"
expect_status 0
expect_output ""
expect_tag_at v0.3.0 "${before}"
expect_log "nothing to release"

case_ "push: an all-zero before (new branch) never tags"
after="$(commit_version 0.3.0)"
run push 0000000000000000000000000000000000000000 "${after}"
expect_status 0
expect_output ""
expect_no_tag v0.3.0
expect_log "::warning::Cannot compare"

case_ "push: an unknown before (rewritten history) never tags"
after="$(commit_version 0.3.0)"
run push 1111111111111111111111111111111111111111 "${after}"
expect_status 0
expect_output ""
expect_no_tag v0.3.0
expect_log "::warning::Cannot compare"

case_ "push: a before without a workspace version counts as a change"
before="$(commit_version '')"
after="$(commit_version 0.3.0)"
run push "${before}" "${after}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${after}"

case_ "push: a head without a workspace version never tags"
before="$(commit_version 0.3.0)"
after="$(commit_version '')"
run push "${before}" "${after}"
expect_status 0
expect_output ""
expect_log "has no [workspace.package].version"

case_ "push: a pre-release version is not tagged"
before="$(commit_version 0.2.0)"
after="$(commit_version 0.3.0-rc.1)"
run push "${before}" "${after}"
expect_status 0
expect_output ""
expect_no_tag v0.3.0-rc.1
expect_log "not a plain release version"

case_ "push: a changed version whose tag already exists is left alone"
first="$(commit_version 0.3.0)"
git tag -a v0.3.0 -m v0.3.0 && git push --quiet origin v0.3.0
before="$(commit_version 0.2.0)"
after="$(commit_version 0.3.0)"
run push "${before}" "${after}"
expect_status 0
expect_output ""
expect_tag_at v0.3.0 "${first}"
expect_log "already exists"

case_ "push: a failed tag query on origin fails without tagging"
before="$(commit_version 0.3.0)"
after="$(commit_version 0.4.0)"
git remote set-url origin "${ROOT}/missing.git"
run push "${before}" "${after}"
expect_status 1
expect_output ""
expect_log "Could not query tags on origin"
[[ -z "$(git tag --list v0.4.0)" ]] || fail "expected no local tag v0.4.0"

case_ "push: a failed tag query on origin fails even when the version is unchanged"
before="$(commit_version 0.4.0)"
after="$(commit_version 0.4.0 '# dependency bump')"
git remote set-url origin "${ROOT}/missing.git"
run push "${before}" "${after}"
expect_status 1
expect_log "Could not query tags on origin"

# --- dispatch ---------------------------------------------------------------------

case_ "dispatch: a missing tag defaults to the dispatched commit"
commit_version 0.2.0 >/dev/null
head="$(commit_version 0.3.0)"
run dispatch v0.3.0 "" "${head}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${head}"

case_ "dispatch: a missing tag is created at an explicit older ref"
bump="$(commit_version 0.3.0)"
head="$(commit_version 0.3.0 '# dependency bump')"
run dispatch v0.3.0 "${bump}" "${head}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${bump}"

case_ "dispatch: a ref may be a branch name known only as a remote-tracking ref"
bump="$(commit_version 0.3.0)"
git push --quiet origin HEAD:release
git update-ref refs/remotes/origin/release "${bump}"
head="$(commit_version 0.3.1)"
run dispatch v0.3.0 release "${head}"
expect_status 0
expect_tag_at v0.3.0 "${bump}"

case_ "dispatch: a ref whose version differs from the tag fails without tagging"
head="$(commit_version 0.2.0)"
run dispatch v0.3.0 "" "${head}"
expect_status 1
expect_output ""
expect_no_tag v0.3.0
expect_log "does not match v0.3.0"

case_ "dispatch: an unresolvable ref fails without tagging"
head="$(commit_version 0.3.0)"
run dispatch v0.3.0 no-such-ref "${head}"
expect_status 1
expect_no_tag v0.3.0
expect_log "Cannot resolve 'no-such-ref'"

case_ "dispatch: a malformed tag fails"
head="$(commit_version 0.3.0)"
run dispatch 0.3.0 "" "${head}"
expect_status 1
expect_no_tag 0.3.0
expect_log "does not match 'v<major>.<minor>.<patch>'"

case_ "dispatch: an existing tag without ref is released as it is"
bump="$(commit_version 0.3.0)"
git tag -a v0.3.0 -m v0.3.0 && git push --quiet origin v0.3.0
head="$(commit_version 0.3.1)"
run dispatch v0.3.0 "" "${head}"
expect_status 0
expect_output "tag=v0.3.0"
expect_tag_at v0.3.0 "${bump}"
expect_log "already exists"

case_ "dispatch: an existing lightweight tag with its own commit as ref is released"
bump="$(commit_version 0.3.0)"
git tag v0.3.0 && git push --quiet origin v0.3.0
run dispatch v0.3.0 "${bump}" "${bump}"
expect_status 0
expect_output "tag=v0.3.0"

case_ "dispatch: an existing annotated tag with its commit as ref is released"
bump="$(commit_version 0.3.0)"
git tag -a v0.3.0 -m v0.3.0 && git push --quiet origin v0.3.0
run dispatch v0.3.0 "${bump}" "${bump}"
expect_status 0
expect_output "tag=v0.3.0"

case_ "dispatch: an existing tag is never moved to another ref"
bump="$(commit_version 0.3.0)"
git tag -a v0.3.0 -m v0.3.0 && git push --quiet origin v0.3.0
head="$(commit_version 0.3.0 '# dependency bump')"
run dispatch v0.3.0 "${head}" "${head}"
expect_status 1
expect_output ""
expect_tag_at v0.3.0 "${bump}"
expect_log "a tag is never moved"

case_ "dispatch: a failed tag query on origin fails without releasing"
head="$(commit_version 0.3.0)"
git remote set-url origin "${ROOT}/missing.git"
run dispatch v0.3.0 "" "${head}"
expect_status 1
expect_output ""
expect_log "Could not query tags on origin"

# --- usage ------------------------------------------------------------------------

case_ "usage: unknown or incomplete commands exit 2"
run frobnicate
expect_status 2
run push only-one
expect_status 2

echo "${cases} cases, ${failures} failures"
[[ ${failures} -eq 0 ]]
