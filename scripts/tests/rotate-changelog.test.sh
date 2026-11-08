#!/usr/bin/env bash
# Tests scripts/rotate-changelog.sh against throwaway CHANGELOG files.
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/rotate-changelog.sh"
readonly SCRIPT
ROOT="$(mktemp -d)"
ROOT="$(cd "${ROOT}" && pwd -P)"
readonly ROOT
trap 'rm -rf "${ROOT}"' EXIT

failures=0
cases=0
status=0
output=""
file=""

# A fresh CHANGELOG whose Unreleased body is $1.
setup() {
  cases=$((cases + 1))
  file="${ROOT}/case${cases}.md"
  {
    printf '# Changelog\n\n## [Unreleased]\n\n%s\n\n## [0.2.0] - 2026-11-06\n\n- old (#1)\n\n' "$1"
    printf '[Unreleased]: https://example.com/r/compare/v0.2.0...HEAD\n'
    printf '[0.2.0]: https://example.com/r/compare/v0.1.0...v0.2.0\n'
  } >"${file}"
}

run() {
  status=0
  output="$("${SCRIPT}" "$@" 2>&1)" || status=$?
}

check() {
  local name="$1" ok="$2"
  if [[ "${ok}" == "yes" ]]; then
    echo "ok   ${name}"
  else
    echo "FAIL ${name} (status ${status}): ${output}"
    failures=$((failures + 1))
  fi
}

has() { grep -qxF -- "$1" "${file}" && echo yes || echo no; }

setup "- Added a thing (#2, #3)"
run 0.3.0 2026-11-09 "${file}"
check "rotates a filled Unreleased" "$([[ ${status} -eq 0 ]] && echo yes || echo no)"
check "adds the dated heading" "$(has '## [0.3.0] - 2026-11-09')"
check "keeps an empty Unreleased on top" "$(awk '/^## /{print; exit}' "${file}" | grep -qxF '## [Unreleased]' && echo yes || echo no)"
check "puts the entries under the new version" "$(awk '/^## \[0.3.0\]/{f=1} /^## \[0.2.0\]/{f=0} f && /Added a thing/{ok=1} END{print ok ? "yes" : "no"}' "${file}")"
check "moves the Unreleased link" "$(has '[Unreleased]: https://example.com/r/compare/v0.3.0...HEAD')"
check "adds the version link from the previous tag" "$(has '[0.3.0]: https://example.com/r/compare/v0.2.0...v0.3.0')"
check "keeps older sections" "$(has '## [0.2.0] - 2026-11-06')"

setup ""
before="$(cat "${file}")"
run 0.3.0 2026-11-09 "${file}"
check "fails on an empty Unreleased" "$([[ ${status} -ne 0 && ${output} == *empty* ]] && echo yes || echo no)"
check "leaves the file untouched when empty" "$([[ "$(cat "${file}")" == "${before}" ]] && echo yes || echo no)"

setup "<!-- a comment only -->"
run 0.3.0 2026-11-09 "${file}"
check "a comment alone is empty" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"

setup "<!-- TODO(bump): resolve -->
- entry (#2)"
run 0.3.0 2026-11-09 "${file}"
check "fails while TODO(bump) is present" "$([[ ${status} -ne 0 && ${output} == *TODO* ]] && echo yes || echo no)"

setup "- entry (#2)"
run 0.2.0 2026-11-09 "${file}"
check "refuses an existing version" "$([[ ${status} -ne 0 && ${output} == *already* ]] && echo yes || echo no)"

setup "- entry (#2)"
run 1.0 2026-11-09 "${file}"
check "refuses a malformed version" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"
run 0.3.0 09-11-2026 "${file}"
check "refuses a malformed date" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"

setup "- entry (#2)"
sed -i '/^\[Unreleased\]:/d' "${file}"
run 0.3.0 2026-11-09 "${file}"
check "fails without the Unreleased compare link" "$([[ ${status} -ne 0 && ${output} == *link* ]] && echo yes || echo no)"

run 0.3.0 2026-11-09 "${ROOT}/missing.md"
check "fails on a missing file" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"

echo "${cases} cases, ${failures} failed checks"
[[ ${failures} -eq 0 ]]
