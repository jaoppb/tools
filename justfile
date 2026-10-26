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

# Audit GitHub repository settings against workflow rules
repo-check:
    cargo run -q -p repo-sync -- --check

# Synchronize GitHub repository settings to match tools.toml
repo-sync:
    cargo run -q -p repo-sync
