# Tools monorepo task recipes
#
# Every recipe except `default` runs only inside the flake devShell (`nix develop`, or direnv with
# `.envrc`), so local runs and CI use the same toolchain, pinned by flake.lock.

default:
    @just --list

# Fail unless running inside the flake devShell (flake.nix sets TOOLS_DEVSHELL)
[private]
_devshell:
    @[ -n "${TOOLS_DEVSHELL:-}" ] || { echo "error: run just inside the flake devShell: 'nix develop -c just <recipe>', or enter it with 'nix develop' or 'direnv allow'" >&2; exit 1; }

# Set up development environment and Git hooks
setup: _devshell
    lefthook install

# Run workspace checks, lints, and test suite
check: _devshell
    just fmt-check
    cargo check --workspace
    just lint
    cargo test --workspace
    just check-features

# Check Rust formatting
fmt-check: _devshell
    cargo fmt --all -- --check

# Lint the workspace with Clippy, denying warnings
lint: _devshell
    cargo clippy --workspace -- -D warnings

# Build every cargo feature group alone, and with none, to keep partial builds compiling
check-features: _devshell
    cargo clippy -p tools --no-default-features --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features issues --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features repo --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features project --all-targets -- -D warnings

# Run workspace architecture rules validation via arch-lint
arch-check: _devshell
    cargo test -p tools-architecture-tests

# Audit open GitHub issues against repository workflow rules
issues-check: _devshell
    cargo run -q -p tools -- issues check

# Audit GitHub repository settings, labels, and milestones against tools.toml
repo-check: _devshell
    cargo run -q -p tools -- repo check

# Synchronize all GitHub repository configuration (settings, labels, milestones) to match tools.toml
repo-sync: _devshell
    cargo run -q -p tools -- repo sync

# Synchronize GitHub repository settings only to match tools.toml
settings-sync: _devshell
    cargo run -q -p tools -- repo sync --settings

# Synchronize GitHub repository labels only to match tools.toml
labels-sync: _devshell
    cargo run -q -p tools -- repo sync --labels

# Synchronize GitHub repository milestones only to match tools.toml
milestones-sync: _devshell
    cargo run -q -p tools -- repo sync --milestones

# Bootstrap and reconcile all repository configuration (settings, labels, milestones)
repo-bootstrap: repo-sync

# Synchronize all GitHub repository configuration across an organization (or override owner)
org-sync org="": _devshell
    cargo run -q -p tools -- repo sync {{ if org != "" { "--org " + org } else { "" } }}

# Rewrite legacy issue labels to the configured taxonomy (writes; preview with `just issues-check`)
issues-migrate *args: _devshell
    cargo run -q -p tools -- issues migrate {{ args }}

# Print the planned Project Status moves without writing (exits 1 when there are any)
project-check: _devshell
    cargo run -q -p tools -- project check

# Write the planned Project Status moves
project-sync: _devshell
    cargo run -q -p tools -- project sync

# Open a PR bumping the workspace version (major, minor, patch, or X.Y.Z): `just bump-version minor --issue 90 [--dry-run]`
bump-version target *args: _devshell
    ./scripts/bump-version.sh {{target}} {{args}}
