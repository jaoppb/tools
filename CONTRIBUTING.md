# Contributing to Tools

Thank you for contributing to the `tools` monorepo. This document outlines the workflow, development environment setup, branching strategies, and commit conventions used across the project.

This repository enforces strict issue hygiene and commit conventions as documented in [AGENTS.md](AGENTS.md).

---

## Table of Contents

- [Code of Conduct & Principles](#code-of-conduct--principles)
- [Developer Setup & Tooling](#developer-setup--tooling)
  - [Prerequisites](#prerequisites)
  - [Initial Setup](#initial-setup)
  - [Git Hooks](#git-hooks)
- [Issue Workflow](#issue-workflow)
  - [Hierarchy & Milestones](#hierarchy--milestones)
  - [Label Taxonomy](#label-taxonomy)
  - [Issue Creation & Templates](#issue-creation--templates)
- [Branch & Commit Conventions](#branch--commit-conventions)
  - [Branch Naming](#branch-naming)
  - [Commit Messages](#commit-messages)
- [Pull Requests & Merge Policy](#pull-requests--merge-policy)
- [Workspace Verification & Recipes](#workspace-verification--recipes)

---

## Code of Conduct & Principles

- **Simplicity First:** Write the minimum code that cleanly solves the problem. Avoid speculative abstractions.
- **Surgical Changes:** Modify only what is necessary for the task at hand. Match existing style and preserve unrelated code/comments.
- **Fail Loud:** Do not swallow errors or ignore broken workflows. Enforce conventions deterministically via tooling.

---

## Developer Setup & Tooling

### Prerequisites

- **Rust:** Rust toolchain supporting edition 2024.
- **[Just](https://github.com/casey/just):** Command runner for project tasks.
- **[Lefthook](https://github.com/evilmartians/lefthook):** Git hooks manager.
- **[GitHub CLI (`gh`)](https://cli.github.com/):** For CLI operations requiring GitHub authentication.
- *(Optional)* **[Nix](https://nixos.org/):** A reproducible environment is available via `nix develop` (using `flake.nix`).

### Initial Setup

After cloning the repository, configure Git hooks:

```bash
just setup
```

### Git Hooks

Lefthook automates code quality checks:
- **`pre-commit`:** Runs `cargo fmt --all -- --check` and `cargo clippy --workspace -- -D warnings` whenever Rust or Cargo files are modified.
- **`pre-push`:** Executes full workspace verification (`just check`) and the issue audit (`just issues-check`).

Local overrides can be added to `lefthook-local.yml` (ignored by git).

---

## Issue Workflow

### Hierarchy & Milestones

- Open issues must belong to a target release milestone:
  - `v0.1.0`: Bootstrap (initial monorepo setup, extraction, tooling, Nix flake).
  - `v0.2.0`: Initial stable release (distribution, packaging, cross-repo adoption).
  - `v0.3.0`+: Post-release iterations and enhancements.
- Native GitHub sub-issues are supported when decomposing complex tasks into discrete steps.

### Label Taxonomy

Every open issue must strictly follow the label schema:
- **Exactly one `type:` label:**
  - `type:feat` — New behavior or capability
  - `type:fix` — Something is broken
  - `type:chore` — Tooling, CI, housekeeping, dependencies
  - `type:docs` — Documentation, guides, specifications
  - `type:refactor` — Restructuring without changing behavior
  - `type:spike` — Time-boxed research answering a question
- **At least one `area:` label:**
  - `area:core` — `crates/core` (domain models, API clients, config)
  - `area:project-sync` — `crates/project-sync` (Projects V2 status sync)
  - `area:issues-check` — `crates/issues-check` (workflow and integrity auditor)
  - `area:tooling` — Flake, Nix, Just, CI, Cargo workspace
  - `area:docs` — Documentation and workflow specifications

Run `just issues-check` locally to verify that all open issues conform to these rules.

### Issue Creation & Templates

File issues using the templates in `.github/ISSUE_TEMPLATE/`. Always select the appropriate issue type, assign the target milestone, and apply the relevant `area:` label.

---

## Branch & Commit Conventions

### Branch Naming

Branches must follow the pattern:
```
<type>/<issue>
```
Examples:
- `feat/12`
- `chore/1`
- `docs/31`
- `fix/42`

### Commit Messages

Commit titles must strictly conform to:
```
type: subject (#issue)
```

- **Intermediate commits:** Reference the issue using `Refs #issue`:
  ```
  docs: draft developer quickstart section (Refs #31)
  ```
- **Final commit on a branch:** Use GitHub closing keywords (`Closes #issue` or `Fixes #issue`) in the PR description and/or final commit body:
  ```
  docs: create repository README and overview guide (Closes #31)
  ```

---

## Pull Requests & Merge Policy

- **Merge Commits Only:** Squash and rebase merges are strictly disabled in repository settings. Every commit on your branch reaches the `main` branch through a merge commit.
- **PR Titles:** Follow the same format as commit titles (`type: subject (#issue)`).
- **PR Description:** Include `Closes #issue` to automatically close the associated issue upon merge.
- **CI Gates:** All PRs must pass `cargo fmt`, `cargo clippy`, `cargo test`, and `just arch-check`.

---

## Workspace Verification & Recipes

Use `just` recipes to validate changes before pushing:

```bash
# Workspace lints, formatting, and unit tests
just check

# Architecture boundaries lint via arch-lint
just arch-check

# Open issues audit against tools.toml rules
just issues-check

# Validate repository settings, labels, and milestones against tools.toml
just repo-check
```

For onboarding additional repositories or standardizing organizations with `tools.toml`, refer to the [Multi-Repository Organization Onboarding Guide](docs/multi-repo-onboarding.md).
