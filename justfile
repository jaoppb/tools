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
