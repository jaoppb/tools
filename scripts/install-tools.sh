#!/usr/bin/env bash
set -euo pipefail

# Default configuration
REPO="${REPO:-jaoppb/tools}"

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
TOOLS_LIST="${2:-${TOOLS_LIST:-all}}"
DEST_DIR="${3:-${DEST_DIR:-}}"
CACHE_HIT="${CACHE_HIT:-false}"

if [ -z "$DEST_DIR" ]; then
    if [ -n "${RUNNER_TEMP:-}" ]; then
        DEST_DIR="${RUNNER_TEMP}/tools-bin"
    else
        DEST_DIR="${HOME}/.local/bin"
    fi
fi

mkdir -p "$DEST_DIR"

# Resolve list of tools
if [ "$TOOLS_LIST" = "all" ] || [ -z "$TOOLS_LIST" ]; then
    TOOLS=("issues-check" "repo-sync" "project-sync")
else
    IFS=',' read -r -a TOOLS <<< "$TOOLS_LIST"
fi

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
echo "  Tools: ${TOOLS[*]}"

# If cache-hit is true and binaries exist, verify and skip download
if [ "$CACHE_HIT" = "true" ]; then
    all_cached=true
    for tool in "${TOOLS[@]}"; do
        clean_tool="$(echo "$tool" | xargs)"
        [ -z "$clean_tool" ] && continue
        if [ ! -x "${DEST_DIR}/${clean_tool}${BIN_EXT}" ]; then
            all_cached=false
            break
        fi
    done
    if [ "$all_cached" = "true" ]; then
        echo "All requested tools found in cache hit. Skipping download."
        if [ -n "${GITHUB_OUTPUT:-}" ]; then
            echo "version=${TAG}" >> "$GITHUB_OUTPUT"
        fi
        exit 0
    fi
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# Download and install each tool
for tool in "${TOOLS[@]}"; do
    clean_tool="$(echo "$tool" | xargs)"
    [ -z "$clean_tool" ] && continue

    ASSET="${clean_tool}-${TAG}-${TARGET}.${EXT}"
    CHECKSUM_ASSET="${clean_tool}-${TAG}-${TARGET}.sha256"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
    CHECKSUM_URL="https://github.com/${REPO}/releases/download/${TAG}/${CHECKSUM_ASSET}"

    echo "Fetching ${clean_tool} (${ASSET})..."

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
        chmod +x "${DEST_DIR}/${clean_tool}${BIN_EXT}"
        echo "Successfully installed ${clean_tool} to ${DEST_DIR}/${clean_tool}${BIN_EXT}"
    else
        echo "Release asset download failed for ${clean_tool} (${ASSET}). Details:" >&2
        cat "$curl_err_file" >&2 || true
        echo "Attempting cargo fallback..." >&2

        fallback_success=false
        if command -v cargo >/dev/null 2>&1; then
            echo "Attempting cargo install --git fallback..."
            if cargo install --git "https://github.com/${REPO}" --tag "$TAG" "$clean_tool" --root "$TMP_DIR/cargo-root"; then
                cp "$TMP_DIR/cargo-root/bin/${clean_tool}${BIN_EXT}" "${DEST_DIR}/"
                chmod +x "${DEST_DIR}/${clean_tool}${BIN_EXT}"
                fallback_success=true
            fi
        fi

        if [ "$fallback_success" = "false" ]; then
            echo "Error: Failed to install '${clean_tool}'. Precompiled release asset '${ASSET}' was not accessible and cargo fallback was unavailable or failed." >&2
            exit 1
        fi
    fi
done

if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "version=${TAG}" >> "$GITHUB_OUTPUT"
fi

echo "Tools installation complete."
