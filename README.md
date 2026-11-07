# Tools

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Rust: 2024 Edition](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](Cargo.toml)

A monorepo for developer tools, declarative repository management utilities, and GitHub workflow automation.

---

## Table of Contents

- [Overview & Purpose](#overview--purpose)
- [Monorepo Architecture & Crates](#monorepo-architecture--crates)
  - [Architecture Diagram](#architecture-diagram)
  - [Crates Overview](#crates-overview)
    - [crates/core (tools-core)](#cratescore-tools-core)
    - [crates/cli (tools)](#cratescli-tools)
- [Declarative Configuration (tools.toml)](#declarative-configuration-toolstoml)
  - [Repository Settings](#repository-settings)
  - [Labels Schema](#labels-schema)
  - [Milestones Definition](#milestones-definition)
  - [Issue Workflow Rules](#issue-workflow-rules)
- [Developer Quickstart & Tooling](#developer-quickstart--tooling)
  - [Prerequisites](#prerequisites)
  - [Local Environment Setup](#local-environment-setup)
  - [Development Recipes](#development-recipes)
- [Packaging, Installation & GitHub Action](#packaging-installation--github-action)
  - [Composite GitHub Action](#composite-github-action)
  - [Binary Releases & Installation Script](#binary-releases--installation-script)
  - [Nix Flake Packages](#nix-flake-packages)
- [Contributing & Guidelines](#contributing--guidelines)

---

## Overview & Purpose

The `tools` monorepo provides a unified, reliable suite of Rust CLIs and core domain libraries designed to automate and enforce engineering standards across GitHub repositories and organizations.

Key capabilities include:
- **Declarative Repository Synchronization:** Reconcile repository settings (merge options, branch deletion), label taxonomies, and milestone definitions against a single source of truth (`tools.toml`).
- **Issue Workflow Auditing:** Enforce structural integrity on open issues, including strict label taxonomies (`type:*` and `area:*`), mandatory milestone assignments, cyclic dependency detection, and automated migration from legacy labels.
- **GitHub Projects V2 Synchronization:** Map and synchronize project board statuses, dependency blockers, and native sub-issue hierarchies.

---

## Monorepo Architecture & Crates

### Architecture Diagram

The monorepo follows a modular hexagonal/ports-and-adapters architecture where domain logic and external integrations reside in `tools-core`, consumed by focused command-line tools:

```mermaid
flowchart TD
    Config["tools.toml\n(Declarative Config)"] --> Core["crates/core\n(tools-core)"]

    subgraph CoreComponents ["tools-core"]
        ConfigLoader["Config Parser & Loader"]
        DomainModels["Domain Models & Rules"]
        GitHubAdapters["GitHub REST & GraphQL Adapters"]
    end

    Core --> CoreComponents

    Tools["crates/cli\n(tools CLI: issues, repo, project)"] --> Core

    Tools --> GitHubAPI["GitHub API\n(REST & GraphQL)"]
```

### Crates Overview

#### [crates/core](crates/core) (`tools-core`)
Shared foundation library providing domain models, adapters, and configuration parsing.
- **`domain`:** Pure business logic and models representing repository settings, label sets, milestones, issue hierarchies, and validation rules.
- **`config`:** Strongly typed deserialization, defaults, and validation for `tools.toml`.
- **`adapters::github`:** High-performance GitHub REST and GraphQL API clients wrapping `gh` CLI commands and native queries.
- **`application`:** Orchestration logic for auditing, plan generation, and synchronization execution.

#### [crates/cli](crates/cli) (`tools`)
One binary, `tools <group> <verb>`. Read-only verbs are always `check`; verbs that write are `sync` or `migrate`. `--config`, `--owner` (alias `--org`) and `--repo` are global flags accepted on every verb.

Each group is a cargo feature (`issues`, `repo`, `project`), all on by default, so a build can carry only what it needs:

```bash
cargo build -p tools --no-default-features --features repo
```

Exit codes: `0` clean (or `--help`), `1` a `check` found something to fix, `2` the run could not complete.

**`tools repo`**: declarative repository configuration. Audits or reconciles GitHub settings, labels, and milestones across single repositories or entire organizations.
```bash
tools repo check                 # drift report, writes nothing (exit 1 on drift)
tools repo sync                  # reconcile everything configured in tools.toml
tools repo sync --settings       # or --labels, --milestones, --project
```

**`tools issues`**: workflow integrity and rule auditor for GitHub issues.
```bash
tools issues check               # milestones, taxonomy, blockers; also previews pending label migrations (exit 1 if any)
tools issues migrate             # rewrite legacy labels (e.g. `bug`, `enhancement`) to the configured taxonomy
tools issues migrate --default-area core --default-type chore
```

**`tools project`**: GitHub Projects V2 board Status synchronization from issue states, blocked-by relationships, and native sub-issues.
```bash
tools project check              # print the planned moves and frozen cards, write nothing (exit 1 when there are any)
tools project check --explain 57 # the facts and rules behind one issue (`57` or `repo#57`)
tools project sync               # write the moves
```

Both need a `[workflow]` block in `tools.toml` (for example `extends = "builtin:cosmo"`); the board's Status columns must match its `statuses`. See [the workflow engine spec](docs/workflow-engine-spec.md).

During development use `cargo run -q -p tools -- <group> <verb>`, or the `just` recipes below.

---

## Declarative Configuration (`tools.toml`)

All repository configurations and workflow rules are governed declaratively by [`tools.toml`](tools.toml) in the repository root.

### Repository Settings
Defines target repository identity and merge button constraints:
```toml
[github.repository]
owner = "jaoppb"
name = "tools"

[github.repository.settings]
allow_merge_commit = true
allow_squash_merge = false
allow_rebase_merge = false
delete_branch_on_merge = true
merge_commit_title = "PR_TITLE"
merge_commit_message = "PR_BODY"
```

### Labels Schema
Specifies canonical labels and marks deprecated legacy labels for automated cleanup or migration. The issue templates in `.github/ISSUE_TEMPLATE/` and the label lists in `AGENTS.md` and `CONTRIBUTING.md` must agree with it; `tools repo check` verifies this offline:
```toml
[github.labels]
deprecated = [
    "bug",
    "enhancement",
    "documentation",
]
items = [
    { name = "type:feat", color = "1d76db", description = "New behaviour or capability" },
    { name = "type:fix", color = "d73a4a", description = "Something is broken" },
    { name = "type:chore", color = "8b949e", description = "Tooling, CI, housekeeping, dependencies" },
    { name = "type:docs", color = "0075ca", description = "Documentation, guides, specifications" },
    { name = "area:core", color = "2ea44f", description = "crates/core: domain models, github client, config" },
    { name = "area:tooling", color = "2ea44f", description = "Flake, Nix, Just, CI, Cargo workspace" },
]
```

### Milestones Definition
Manages release milestones and supports automated renames:
```toml
[github.milestones]
renames = { "v0" = "v0.1.0", "v1" = "v0.2.0" }
items = [
    { title = "v0.1.0", description = "Bootstrap: issue workflow, repository setup, and tooling extraction", state = "closed" },
    { title = "v0.2.0", description = "Initial release: stable CLI distribution and packaging", state = "closed" },
    { title = "v0.3.0", description = "Cleanup and refactoring of hardcoded logic", state = "open" },
]
```

### Issue Workflow Rules
Enforces issue invariants during `just issues-check`:
```toml
[github.issues.rules]
cycle = true       # Prohibit cyclic dependency chains
labels = true      # Require exactly one type:* label and >=1 area:* label
milestone = true   # Require every open issue to be assigned a milestone
release = true     # Enforce milestone format conventions
```

---

## Developer Quickstart & Tooling

### Prerequisites

- **Rust**: Rust toolchain supporting edition 2024.
- **[Just](https://github.com/casey/just)**: Command runner for workspace tasks.
- **[Lefthook](https://github.com/evilmartians/lefthook)**: Fast, multi-language Git hooks manager.
- **[GitHub CLI (`gh`)](https://cli.github.com/)**: Authenticated CLI used by adapters to communicate with GitHub.
- *(Optional)* **[Nix](https://nixos.org/)**: A reproducible developer environment is provided via [`flake.nix`](flake.nix) (`nix develop`).

### Local Environment Setup

1. Clone the repository and enter the workspace:
   ```bash
   git clone https://github.com/jaoppb/tools.git
   cd tools
   ```
2. Initialize developer tooling and Git hooks:
   ```bash
   just setup
   ```
   This configures Lefthook pre-commit (formatting and clippy lints) and pre-push hooks (workspace tests and issue checks).

### Development Recipes

Workspace recipes are managed with `just`:

| Recipe | Description | Invocation |
|--------|-------------|------------|
| **Validation & Checks** | | |
| `just check` | Runs formatting checks, workspace type-check, Clippy lints, and unit tests | `just check` |
| `just arch-check` | Validates architecture boundary rules via `arch-lint` | `just arch-check` |
| `just check-features` | Builds each feature group (`issues`, `repo`, `project`) alone, and none, with Clippy; part of `just check` | `just check-features` |
| `just issues-check` | Audits open GitHub issues against workflow rules in `tools.toml` | `just issues-check` |
| `just repo-check` | Verifies GitHub repository settings, labels, and milestones against `tools.toml` | `just repo-check` |
| **Repository Synchronization** | | |
| `just repo-sync` | Reconciles repository settings, labels, and milestones to match `tools.toml` | `just repo-sync` |
| `just settings-sync` | Synchronizes repository merge settings only | `just settings-sync` |
| `just labels-sync` | Synchronizes repository labels only | `just labels-sync` |
| `just milestones-sync`| Synchronizes repository milestones only | `just milestones-sync` |
| `just repo-bootstrap` | Reconciles repository configuration (alias for `repo-sync`) | `just repo-bootstrap` |
| `just org-sync` | Synchronizes configuration across an organization | `just org-sync org="my-org"` |
| **Migration & Releases** | | |
| `just issues-migrate` | Rewrites legacy labels to the configured taxonomy (writes; `just issues-check` previews) | `just issues-migrate --default-area core --default-type chore` |
| `just project-check` | Prints the planned Project Status moves, writes nothing | `just project-check` |
| `just project-sync` | Writes the planned Project Status moves | `just project-sync` |
| `just bump-version` | Bumps workspace version, updates `Cargo.lock`, tags, and commits | `just bump-version patch` |

---

## Packaging, Installation & GitHub Action

Pre-compiled binary releases, composite GitHub Actions, and Nix packages enable direct consumption in external repositories and CI pipelines without Rust toolchain setup or compilation overhead.

### Composite GitHub Action

To enforce workflow integrity in another repository's GitHub Actions workflow, use the official composite action `jaoppb/tools`:

```yaml
name: Verify Compliance
on: [push, pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      issues: read
      pull-requests: read
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Run Tools Compliance Checks
        uses: jaoppb/tools@v0.2.0
        with:
          version: 'latest'
          check-issues: 'true'
          check-repo: 'true'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

Alternatively, omit the `check-*` and `sync-project` inputs to install `tools` onto `$GITHUB_PATH` for customized command execution:

```yaml
      - name: Setup Tools CLI
        uses: jaoppb/tools@v0.2.0

      - name: Run Audit
        run: tools issues check
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

#### Action inputs

| Input | Default | Description |
|-------|---------|-------------|
| `version` | `latest` | Release tag of the binary to install. |
| `build-from-source` | `false` | Build `tools` with cargo from the action's own checkout instead of downloading a release (needs a Rust toolchain). |
| `repo` | `jaoppb/tools` | `owner/name` repository whose releases provide the binary (use it for a fork). |
| `config-path` | _(empty)_ | `tools.toml` passed as `--config` to every command the action runs. Empty means upward search from the working directory. |
| `check-issues` / `issues-args` | `false` / _(empty)_ | Run `tools issues check` and pass extra arguments. |
| `check-repo` / `repo-args` | `false` / _(empty)_ | Run `tools repo check` and pass extra arguments. |
| `check-project` / `project-args` | `false` / _(empty)_ | Run `tools project check` (read-only; fails when moves are pending) and pass extra arguments. |
| `sync-project` | `false` | Run `tools project sync` (writes); `project-args` applies to it too. |
| `github-token` | `${{ github.token }}` | Token for release downloads and the tools. |

Release binaries are named `tools-<tag>-<target>`, so `version` must name a release published after this change (earlier tags only carry the old binaries); use `build-from-source` before then. Since v0.3.0 the input names follow the commands: `tools` (the binary list) is gone, `issues-check-args`, `repo-sync-args` and `project-sync-args` became `issues-args`, `repo-args` and `project-args`, and `project-sync-args: '--dry-run'` is now `check-project: 'true'`.

`sync-project` writes. Moving cards in a GitHub Project (v2) needs a personal access token or GitHub App token with project write access passed as `github-token`; the default `github.token` cannot write Projects v2. `tools project` fails when no project number is configured (`[github.project]` in `tools.toml` or `--project`), so leave `check-project` and `sync-project` off in repositories without a project.

```yaml
      - name: Sync Project Status
        uses: jaoppb/tools@v0.2.0
        with:
          config-path: 'config/tools.toml'
          sync-project: 'true'
          github-token: ${{ secrets.PROJECT_TOKEN }}
```

### Binary Releases & Installation Script

Pre-compiled binaries for Linux (`x86_64`, `aarch64`), macOS (`x86_64`, `apple-silicon`), and Windows (`x86_64`) are published with SHA256 checksums on [GitHub Releases](https://github.com/jaoppb/tools/releases).

Install directly via curl:

```bash
curl -sSL https://raw.githubusercontent.com/jaoppb/tools/main/scripts/install-tools.sh | bash
```

Or install a specific version, or build the checked-out source:

```bash
TOOLS_VERSION=v0.2.0 ./scripts/install-tools.sh
FROM_SOURCE=true ./scripts/install-tools.sh
```

### Nix Flake Packages

The `tools` binary is the flake's default package in [`flake.nix`](flake.nix):

```bash
# Run directly without local installation
nix run github:jaoppb/tools -- issues check
nix run github:jaoppb/tools -- repo check
nix run github:jaoppb/tools -- project check

# Enter development shell with all utilities
nix develop github:jaoppb/tools
```

---

## Contributing & Guidelines

For detailed contribution instructions, branch naming conventions, commit formats, pull request standards, and multi-repo onboarding guides, please refer to:

- [CONTRIBUTING.md](CONTRIBUTING.md) — Contributor onboarding, branch/commit naming, PR policy, and setup.
- [AGENTS.md](AGENTS.md) — Canonical rules for human and agent workflows in this repository.
- [docs/multi-repo-onboarding.md](docs/multi-repo-onboarding.md) — Multi-repository and organization-wide setup guide.
