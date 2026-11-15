#!/usr/bin/env bash
# Tests scripts/release-notes-links.sh on Markdown snippets, from a throwaway checkout directory.
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/release-notes-links.sh"
readonly SCRIPT
ROOT="$(mktemp -d)"
ROOT="$(cd "${ROOT}" && pwd -P)"
readonly ROOT
trap 'rm -rf "${ROOT}"' EXIT
mkdir -p "${ROOT}/docs/migrations"
touch "${ROOT}/docs/migrations/v0.3.0.md" "${ROOT}/README.md" "${ROOT}/logo.png"

failures=0
status=0
output=""
errors=""
readonly BLOB="https://github.com/acme/tools/blob/v1.2.3"

# Run the filter on stdin $1 with the arguments that follow, from the fake checkout.
run() {
  local input="$1"
  shift
  status=0
  output="$(cd "${ROOT}" && printf '%s\n' "${input}" | "${SCRIPT}" "$@" 2>"${ROOT}/stderr")" || status=$?
  errors="$(cat "${ROOT}/stderr")"
}

check() {
  local name="$1" ok="$2"
  if [[ "${ok}" == "yes" ]]; then
    echo "ok   ${name}"
  else
    echo "FAIL ${name} (status ${status}): ${output} ${errors}"
    failures=$((failures + 1))
  fi
}

# Rewrites $1 and expects $2 on stdout with status 0.
expect() {
  local name="$1" input="$2" want="$3"
  run "${input}" v1.2.3 acme/tools
  check "${name}" "$([[ ${status} -eq 0 && ${output} == "${want}" ]] && echo yes || echo no)"
}

expect "rewrites an inline link" \
  "> **Migration guide:** [v0.2.0 → v0.3.0](docs/migrations/v0.3.0.md)." \
  "> **Migration guide:** [v0.2.0 → v0.3.0](${BLOB}/docs/migrations/v0.3.0.md)."
expect "rewrites every link on a line" \
  "[a](README.md) and [b](./docs/migrations/v0.3.0.md)" \
  "[a](${BLOB}/README.md) and [b](${BLOB}/docs/migrations/v0.3.0.md)"
expect "treats a leading slash as the repository root" \
  "[a](/README.md)" "[a](${BLOB}/README.md)"
expect "keeps a fragment and a link title" \
  '[a](README.md#install "Install")' "[a](${BLOB}/README.md#install \"Install\")"
expect "rewrites a reference definition" \
  "[guide]: docs/migrations/v0.3.0.md" "[guide]: ${BLOB}/docs/migrations/v0.3.0.md"
expect "points an image at raw" \
  "![logo](logo.png)" "![logo](https://github.com/acme/tools/raw/v1.2.3/logo.png)"
expect "leaves absolute links alone" \
  "[k](https://keepachangelog.com/en/1.1.0/) [m](mailto:a@b.c) [p](//host/x)" \
  "[k](https://keepachangelog.com/en/1.1.0/) [m](mailto:a@b.c) [p](//host/x)"
expect "leaves anchors alone" "[top](#changelog)" "[top](#changelog)"
expect "leaves compare links alone" \
  "[0.3.0]: https://github.com/acme/tools/compare/v0.2.0...v0.3.0" \
  "[0.3.0]: https://github.com/acme/tools/compare/v0.2.0...v0.3.0"
# shellcheck disable=SC2016 # backticks are Markdown here
expect "leaves code spans alone" \
  'Links `docs/migrations/<version>.md` and `[x](y.md)`, then [a](README.md)' \
  "Links \`docs/migrations/<version>.md\` and \`[x](y.md)\`, then [a](${BLOB}/README.md)"
expect "leaves fenced code alone" \
  $'```md\n[x](y.md)\n```\n[a](README.md)' \
  $'```md\n[x](y.md)\n```\n'"[a](${BLOB}/README.md)"
expect "closes a fence only on its own marker" \
  $'~~~\n```\n[x](y.md)\n~~~\n[a](README.md)' \
  $'~~~\n```\n[x](y.md)\n~~~\n'"[a](${BLOB}/README.md)"
expect "closes a fence only on a run as long as the opener" \
  $'````\n```\n[x](y.md)\n````\n[a](README.md)' \
  $'````\n```\n[x](y.md)\n````\n'"[a](${BLOB}/README.md)"
expect "leaves footnotes alone" "[^1]: See the guide." "[^1]: See the guide."
expect "rewrites a reference definition without a space" \
  "[guide]:README.md" "[guide]:${BLOB}/README.md"
expect "leaves plain text alone" \
  "- \`tools repo check\` fails on drift (#1, #2)" "- \`tools repo check\` fails on drift (#1, #2)"

run "[a](README.md)" v1.2.3 acme/tools
check "no warning for an existing target" "$([[ -z ${errors} ]] && echo yes || echo no)"

run "[gone](docs/migrations/v9.9.9.md#top) [gone again](docs/migrations/v9.9.9.md)" v1.2.3 acme/tools
check "warns once on a missing target" \
  "$([[ ${status} -eq 0 && ${errors} == "::warning::"*"docs/migrations/v9.9.9.md"* && $(grep -c warning <<<"${errors}") -eq 1 ]] && echo yes || echo no)"
check "still rewrites a missing target" \
  "$([[ ${output} == *"${BLOB}/docs/migrations/v9.9.9.md#top"* ]] && echo yes || echo no)"

run "[a](README.md)" v1.2.3
check "refuses a missing argument" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"
run "[a](README.md)" v1.2.3 not-a-repo
check "refuses a malformed repository" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"
run "[a](README.md)" 'v1 2' acme/tools
check "refuses a malformed tag" "$([[ ${status} -ne 0 ]] && echo yes || echo no)"

echo "${failures} failed checks"
[[ ${failures} -eq 0 ]]
