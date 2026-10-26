# Tools monorepo task recipes

default:
    @just --list

# Run workspace checks, lints, and test suite
check:
    cargo check --workspace
    cargo clippy --workspace -- -D warnings
    cargo test --workspace

# Audit open GitHub issues against repository workflow rules
issues-check:
    cargo run -q -p issues-check

# Audit GitHub repository settings, labels, and milestones against tools.toml
repo-check:
    cargo run -q -p repo-sync -- --check

# Synchronize GitHub repository settings to match tools.toml
repo-sync:
    cargo run -q -p repo-sync -- --settings

# Synchronize GitHub repository labels to match tools.toml
labels-sync:
    cargo run -q -p repo-sync -- --labels

# Synchronize GitHub repository milestones to match tools.toml
milestones-sync:
    cargo run -q -p repo-sync -- --milestones

# Bootstrap and reconcile all repository configuration (settings, labels, milestones)
repo-bootstrap:
    cargo run -q -p repo-sync
