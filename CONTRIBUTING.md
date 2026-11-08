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
- [Releasing](#releasing)
- [Workspace Verification & Recipes](#workspace-verification--recipes)

---

## Code of Conduct & Principles

- **Simplicity First:** Write the minimum code that cleanly solves the problem. Avoid speculative abstractions.
- **Surgical Changes:** Modify only what is necessary for the task at hand. Match existing style and preserve unrelated code/comments.
- **Fail Loud:** Do not swallow errors or ignore broken workflows. Enforce conventions deterministically via tooling.

---

## Developer Setup & Tooling

### Prerequisites

- **[Nix](https://nixos.org/) with flakes enabled (required):** the `flake.nix` devShell provides the Rust toolchain, `just`, Lefthook and the GitHub CLI (`gh`) at the versions pinned by `flake.lock`. CI runs every `just` job in the same devShell, so `just check` means the same thing locally and in CI. Every `just` recipe except `just --list` fails outside it.
- *(Optional)* **[direnv](https://direnv.net/):** enters the devShell automatically through `.envrc` (`direnv allow`).

### Initial Setup

After cloning the repository, enter the devShell and configure Git hooks:

```bash
nix develop        # or `direnv allow` once
just setup
```

To run one recipe without entering the devShell: `nix develop -c just check`.

### Git Hooks

Lefthook automates code quality checks:
- **`pre-commit`:** Runs `just fmt-check` and `just lint` (`cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`) whenever Rust or Cargo files are modified.
- **`pre-push`:** Executes full workspace verification (`just check`) and the issue audit (`just issues-check`).

The hooks call `just`, so they run only inside the devShell: commit and push from a shell where it is entered (or direnv is allowed). Outside it they fail with the command to use.

Local overrides can be added to `lefthook-local.yml` (ignored by git).

---

## Issue Workflow

### Hierarchy & Milestones

- Open issues must belong to a target release milestone. The milestones are declared in `tools.toml` `[github.milestones]`.
- Native GitHub sub-issues are supported when decomposing complex tasks into discrete steps.

### Label Taxonomy

Every open issue must strictly follow the label schema (declared in `tools.toml` with the issue templates under `[github.issue_templates]`; `just repo-check` fails when this list or the templates drift from them; list each label as a bullet that starts with its backticked name):
<!-- labels:start -->
- **Exactly one `type:` label:**
  - `type:feat` — New behavior or capability
  - `type:fix` — Something is broken
  - `type:chore` — Tooling, CI, housekeeping, dependencies
  - `type:docs` — Documentation, guides, specifications
  - `type:refactor` — Restructuring without changing behavior
  - `type:spike` — Time-boxed research answering a question
- **At least one `area:` label:**
  - `area:core` — `crates/core` (domain models, API clients, config)
  - `area:project-sync` — `tools project` in `crates/cli` (Projects V2 status sync)
  - `area:issues-check` — `tools issues` in `crates/cli` (workflow and integrity auditor)
  - `area:tooling` — Flake, Nix, Just, CI, Cargo workspace
  - `area:docs` — Documentation and workflow specifications
<!-- labels:end -->

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

## Releasing

Releases never touch `main` directly:

1. File a `type:chore` issue for the bump (template, area, milestone).
2. From an up-to-date `main`, run `just bump-version <major|minor|patch|X.Y.Z> --issue <n>` (add `--dry-run` to preview the version, branch, and PR text without changing anything). It needs an authenticated `gh`, creates `chore/<n>`, commits `chore: bump version to X.Y.Z (#n)`, pushes the branch, and opens the PR.
3. Merge the PR with a merge commit. The `Tag Release` workflow then tags the merge commit `vX.Y.Z` and calls the `Release` workflow, which validates the tag against the `Cargo.toml` version and publishes the GitHub Release with binaries. Pushing a `vX.Y.Z` tag by hand still triggers `Release` directly.

If `Release` fails after the tag exists, use "Re-run failed jobs" on the same `Tag Release` run (a new run sees the tag and does nothing). If the `tag` job itself failed, re-run it: the tag job tags the current `Cargo.toml` version whenever its tag is missing, so otherwise the next unrelated change to the root `Cargo.toml` on `main` would tag and release that commit under the bumped version.

---

## Workspace Verification & Recipes

Use `just` recipes to validate changes before pushing:

```bash
# Workspace lints, formatting, unit tests, and the partial feature builds
just check

# Each cargo feature group (issues, repo, project) alone, and none
just check-features

# Architecture boundaries lint via arch-lint
just arch-check

# Open issues audit against tools.toml rules
just issues-check

# Validate repository settings, labels, and milestones against tools.toml
just repo-check
```

For onboarding additional repositories or standardizing organizations with `tools.toml`, refer to the [Multi-Repository Organization Onboarding Guide](docs/multi-repo-onboarding.md).
