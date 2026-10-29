# Tools monorepo task recipes

default:
    @just --list

# Set up development environment and Git hooks
setup:
    lefthook install

# Run workspace checks, lints, and test suite
check:
    cargo fmt --all -- --check
    cargo check --workspace
    cargo clippy --workspace -- -D warnings
    cargo test --workspace

# Audit open GitHub issues against repository workflow rules
issues-check:
    cargo run -q -p issues-check

# Audit GitHub repository settings, labels, and milestones against tools.toml
repo-check:
    cargo run -q -p repo-sync -- --check

# Synchronize all GitHub repository configuration (settings, labels, milestones) to match tools.toml
repo-sync:
    cargo run -q -p repo-sync

# Synchronize GitHub repository settings only to match tools.toml
settings-sync:
    cargo run -q -p repo-sync -- --settings

# Synchronize GitHub repository labels only to match tools.toml
labels-sync:
    cargo run -q -p repo-sync -- --labels

# Synchronize GitHub repository milestones only to match tools.toml
milestones-sync:
    cargo run -q -p repo-sync -- --milestones

# Bootstrap and reconcile all repository configuration (settings, labels, milestones)
repo-bootstrap: repo-sync

# Bump workspace package version (major, minor, patch, or X.Y.Z), refresh Cargo.lock, commit, tag, and push
bump-version target:
    ./scripts/bump-version.sh {{target}}
