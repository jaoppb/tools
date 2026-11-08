# Tools monorepo task recipes

# Recipes run only inside the flake devShell (`nix develop`, or direnv with `.envrc`), so local runs
# and CI use the same toolchain, pinned by flake.lock. `eager` makes just evaluate this guard before
# running any recipe, even under `set lazy`, so it covers every recipe, new ones included;
# `just --list` evaluates nothing and still works anywhere. flake.nix sets TOOLS_DEVSHELL.
eager _devshell := if env("TOOLS_DEVSHELL", "") == "" { error("run just inside the flake devShell: 'nix develop -c just <recipe>', or enter it with 'nix develop' or 'direnv allow'") } else { "" }

default:
    @just --list

# Set up development environment and Git hooks
setup:
    lefthook install

# Run workspace checks, lints, and test suite
check:
    just fmt-check
    cargo check --workspace
    just lint
    cargo test --workspace
    just check-features
    just workflows-check

# Lint the GitHub workflows and test the release scripts (tagging and CHANGELOG rotation)
workflows-check:
    actionlint
    shellcheck scripts/release-tag.sh scripts/tests/release-tag.test.sh scripts/rotate-changelog.sh scripts/tests/rotate-changelog.test.sh scripts/bump-version.sh
    scripts/tests/release-tag.test.sh
    scripts/tests/rotate-changelog.test.sh

# Check Rust formatting
fmt-check:
    cargo fmt --all -- --check

# Lint the workspace with Clippy, denying warnings
lint:
    cargo clippy --workspace -- -D warnings

# Build every cargo feature group alone, and with none, to keep partial builds compiling
check-features:
    cargo clippy -p tools --no-default-features --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features issues --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features repo --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features project --all-targets -- -D warnings
    cargo clippy -p tools --no-default-features --features changelog --all-targets -- -D warnings

# Run workspace architecture rules validation via arch-lint
arch-check:
    cargo test -p tools-architecture-tests

# Fail when the change since `base` touches crates/, action.yml, scripts/ or .github/workflows/ without CHANGELOG.md (waived by the skip-changelog label in `labels`, comma-separated)
changelog-check $base $labels="":
    cargo run -q -p tools -- changelog check --base "$base" --labels "$labels" --skip-label skip-changelog --changelog CHANGELOG.md --watch crates/ --watch action.yml --watch scripts/ --watch .github/workflows/

# Audit open GitHub issues against repository workflow rules
issues-check:
    cargo run -q -p tools -- issues check

# Audit GitHub repository settings, labels, and milestones against tools.toml
repo-check:
    cargo run -q -p tools -- repo check

# Synchronize all GitHub repository configuration (settings, labels, milestones) to match tools.toml
repo-sync:
    cargo run -q -p tools -- repo sync

# Synchronize GitHub repository settings only to match tools.toml
settings-sync:
    cargo run -q -p tools -- repo sync --settings

# Synchronize GitHub repository labels only to match tools.toml
labels-sync:
    cargo run -q -p tools -- repo sync --labels

# Synchronize GitHub repository milestones only to match tools.toml
milestones-sync:
    cargo run -q -p tools -- repo sync --milestones

# Bootstrap and reconcile all repository configuration (settings, labels, milestones)
repo-bootstrap: repo-sync

# Synchronize all GitHub repository configuration across an organization (or override owner)
org-sync org="":
    cargo run -q -p tools -- repo sync {{ if org != "" { "--org " + org } else { "" } }}

# Rewrite legacy issue labels to the configured taxonomy (writes; preview with `just issues-check`)
issues-migrate *args:
    cargo run -q -p tools -- issues migrate {{ args }}

# Print the planned Project Status moves without writing (exits 1 when there are any)
project-check:
    cargo run -q -p tools -- project check

# Write the planned Project Status moves
project-sync:
    cargo run -q -p tools -- project sync

# Open a PR bumping the workspace version (major, minor, patch, or X.Y.Z): `just bump-version minor --issue 90 [--dry-run]`
bump-version target *args:
    ./scripts/bump-version.sh {{target}} {{args}}
