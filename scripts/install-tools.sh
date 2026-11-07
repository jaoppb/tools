#!/usr/bin/env bash
set -euo pipefail

# Default configuration
REPO="${REPO:-jaoppb/tools}"
BIN="tools"
# FROM_SOURCE=true builds the checked-out repository (SOURCE_DIR) instead of downloading a release.
FROM_SOURCE="${FROM_SOURCE:-false}"
SOURCE_DIR="${SOURCE_DIR:-${ACTION_PATH:-.}}"

# GitHub API / Download Authorization Headers
AUTH_HEADER=()
if [ -n "${GITHUB_TOKEN:-}" ]; then
    AUTH_HEADER=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
elif [ -n "${GH_TOKEN:-}" ]; then
    AUTH_HEADER=(-H "Authorization: Bearer ${GH_TOKEN}")
fi

# Resolve version tag from GitHub Releases
resolve_version() {
    local req="${1:-latest}"
    if [ "$FROM_SOURCE" = "true" ]; then
        echo "source"
        return
    fi
    if [ "$req" = "latest" ] || [ -z "$req" ]; then
        local latest_tag=""
        if command -v gh >/dev/null 2>&1; then
            latest_tag="$(gh release view -R "$REPO" --json tagName -q .tagName 2>/dev/null || true)"
        fi
        if [ -z "$latest_tag" ]; then
            latest_tag="$(curl -fsSL ${AUTH_HEADER[@]+"${AUTH_HEADER[@]}"} "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n 1 | sed -E 's/.*"tag_name": "([^"]+)".*/\1/' || true)"
        fi
        if [ -z "$latest_tag" ]; then
            echo "Error: Could not resolve latest release version from GitHub API for '$REPO'." >&2
            exit 1
        fi
        echo "$latest_tag"
    else
        # Ensure leading 'v'
        if [[ "$req" =~ ^v ]]; then
            echo "$req"
        else
            echo "v$req"
        fi
    fi
}

# Subcommand: --resolve-version
if [ "${1:-}" = "--resolve-version" ]; then
    TAG="$(resolve_version "${2:-${TOOLS_VERSION:-latest}}")"
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
        echo "version=${TAG}" >> "$GITHUB_OUTPUT"
    fi
    echo "$TAG"
    exit 0
fi

# Precedence: positional arguments override environment variables
REQUESTED_VERSION="${1:-${TOOLS_VERSION:-latest}}"
DEST_DIR="${2:-${DEST_DIR:-}}"
CACHE_HIT="${CACHE_HIT:-false}"

if [ -z "$DEST_DIR" ]; then
    if [ -n "${RUNNER_TEMP:-}" ]; then
        DEST_DIR="${RUNNER_TEMP}/tools-bin"
    else
        DEST_DIR="${HOME}/.local/bin"
    fi
fi

mkdir -p "$DEST_DIR"

# Detect Operating System and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        OS_FAMILY="unknown-linux-gnu"
        EXT="tar.gz"
        BIN_EXT=""
        ;;
    Darwin)
        OS_FAMILY="apple-darwin"
        EXT="tar.gz"
        BIN_EXT=""
        ;;
    CYGWIN*|MINGW*|MSYS*|Windows_NT)
        OS_FAMILY="pc-windows-msvc"
        EXT="zip"
        BIN_EXT=".exe"
        ;;
    *)
        echo "Error: Unsupported operating system '$OS'" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)
        NORM_ARCH="x86_64"
        ;;
    aarch64|arm64)
        NORM_ARCH="aarch64"
        ;;
    *)
        echo "Error: Unsupported runner architecture '$ARCH'" >&2
        exit 1
        ;;
esac

TARGET="${NORM_ARCH}-${OS_FAMILY}"
TAG="$(resolve_version "$REQUESTED_VERSION")"

echo "Tools installer configuration:"
echo "  Repository: $REPO"
echo "  Resolved Version: $TAG"
echo "  Target: $TARGET"
echo "  Destination: $DEST_DIR"

# If cache-hit is true and the binary exists, skip the download
if [ "$CACHE_HIT" = "true" ] && [ -x "${DEST_DIR}/${BIN}${BIN_EXT}" ]; then
    echo "${BIN} found in cache hit. Skipping download."
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
        echo "version=${TAG}" >> "$GITHUB_OUTPUT"
    fi
    exit 0
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

install_from_source() {
    echo "Building ${BIN} from ${SOURCE_DIR}..."
    cargo install --path "${SOURCE_DIR}/crates/cli" --root "$TMP_DIR/cargo-root" --locked
    cp "$TMP_DIR/cargo-root/bin/${BIN}${BIN_EXT}" "${DEST_DIR}/"
    chmod +x "${DEST_DIR}/${BIN}${BIN_EXT}"
    echo "Successfully installed ${BIN} to ${DEST_DIR}/${BIN}${BIN_EXT}"
}

if [ "$FROM_SOURCE" = "true" ]; then
    install_from_source
    if [ -n "${GITHUB_OUTPUT:-}" ]; then
        echo "version=${TAG}" >> "$GITHUB_OUTPUT"
    fi
    echo "Tools installation complete."
    exit 0
fi

ASSET="${BIN}-${TAG}-${TARGET}.${EXT}"
CHECKSUM_ASSET="${BIN}-${TAG}-${TARGET}.sha256"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/${CHECKSUM_ASSET}"

echo "Fetching ${BIN} (${ASSET})..."

curl_err_file="${TMP_DIR}/curl_error.log"
download_success=false
if curl -fsSL ${AUTH_HEADER[@]+"${AUTH_HEADER[@]}"} -o "${TMP_DIR}/${ASSET}" "$DOWNLOAD_URL" 2>"$curl_err_file" && \
   curl -fsSL ${AUTH_HEADER[@]+"${AUTH_HEADER[@]}"} -o "${TMP_DIR}/${CHECKSUM_ASSET}" "$CHECKSUM_URL" 2>>"$curl_err_file"; then
    download_success=true
fi

if [ "$download_success" = "true" ]; then
    echo "Verifying SHA256 checksum for ${ASSET}..."
    (
        cd "$TMP_DIR"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c "${CHECKSUM_ASSET}"
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 -c "${CHECKSUM_ASSET}"
        else
            echo "Warning: sha256sum/shasum not found. Skipping checksum verification." >&2
        fi
    )

    echo "Extracting ${ASSET} to ${DEST_DIR}..."
    if [ "$EXT" = "tar.gz" ]; then
        tar -xzf "${TMP_DIR}/${ASSET}" -C "$DEST_DIR"
    elif [ "$EXT" = "zip" ]; then
        unzip -q -o "${TMP_DIR}/${ASSET}" -d "$DEST_DIR"
    fi
    chmod +x "${DEST_DIR}/${BIN}${BIN_EXT}"
    echo "Successfully installed ${BIN} to ${DEST_DIR}/${BIN}${BIN_EXT}"
else
    echo "Release asset download failed for ${BIN} (${ASSET}). Details:" >&2
    cat "$curl_err_file" >&2 || true
    echo "Attempting cargo fallback..." >&2

    fallback_success=false
    if command -v cargo >/dev/null 2>&1; then
        echo "Attempting cargo install --git fallback..."
        if cargo install --git "https://github.com/${REPO}" --tag "$TAG" "$BIN" --root "$TMP_DIR/cargo-root"; then
            cp "$TMP_DIR/cargo-root/bin/${BIN}${BIN_EXT}" "${DEST_DIR}/"
            chmod +x "${DEST_DIR}/${BIN}${BIN_EXT}"
            fallback_success=true
        fi
    fi

    if [ "$fallback_success" = "false" ]; then
        echo "Error: Failed to install '${BIN}'. Precompiled release asset '${ASSET}' was not accessible and cargo fallback was unavailable or failed." >&2
        exit 1
    fi
fi

if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "version=${TAG}" >> "$GITHUB_OUTPUT"
fi

echo "Tools installation complete."
