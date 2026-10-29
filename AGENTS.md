# Tools agent rules

This repository is a monorepo for developer tools, repository management utilities, and GitHub workflow automation (`project-sync`, `issues-check`). This file is the canonical rulebook for humans and agents.

## 1. Issue workflow

- **Hierarchy.** Standalone issues under release milestones. Native GitHub sub-issues are supported when breaking down a task into constituent steps.
- **Milestones.** Every open issue must belong to a release milestone:
  - `v0.1.0`: Bootstrap (initial monorepo setup, extraction from cosmo, tooling, Nix flake).
  - `v0.2.0`: Initial stable release (distribution, packaging, cross-repo adoption).
  - `v0.3.0`+: Post-release iterations and enhancements.
- **Labels.**
  - Exactly one `type:` label on every issue: `type:feat`, `type:fix`, `type:chore`, `type:docs`, `type:refactor`, or `type:spike`.
  - At least one `area:` label: `area:core`, `area:project-sync`, `area:issues-check`, `area:tooling`, or `area:docs`.
- **Creation and Triage.** File issues using the corresponding template from `.github/ISSUE_TEMPLATE/`. Upon creation or triage:
  - Select the template matching the issue type (which applies the `type:` label).
  - Set the appropriate `area:` label.
  - Assign the target milestone (`v0.1.0`, `v0.2.0`, etc.).
  - Link dependencies using native GitHub blocked-by links and list them under `Dependencies` if needed.
- **Branches, Commits, and Pull Requests.**
  - Branch naming: `<type>/<issue>` or `<type>/<issue>-<slug>` (e.g., `feat/7`, `chore/1-bootstrap-issue-workflow`, `feat/12-github-graphql-client`).
  - Commit title: `type: subject (#issue)`.
  - Pull request title: `type: subject (#issue)`.
  - Closing keywords: Use `Closes #issue` in the PR body and the branch's final commit. Use `Refs #issue` for intermediate commits.
  - Merge method: **Merge commit only**. Rebase and squash are disabled; every commit reaches `main`.

| Issue workflow rule | Enforcer | Status |
|---------------------|----------|--------|
| Labels (`type:*`, `area:*`) and milestones on open issues | `just issues-check` (also via Lefthook `pre-push`) | Enforced |
| Form structure | `.github/ISSUE_TEMPLATE/*.yml` with `blank_issues_enabled: false` | Enforced |
| Repository configuration (settings, labels, milestones) | `just repo-check` & PR review | Enforced |
| Workspace compilation, lints, tests | `just check` (also via Lefthook `pre-push`) | Enforced |
| Pre-commit formatting & lint validation | Lefthook `pre-commit` (`just setup`) | Enforced |

## 2. Developer setup & Git hooks

- **Lefthook.** Local Git hooks are managed by Lefthook to automate formatting, linting, and quality checks.
- **Setup.** Run `just setup` to configure Git hooks in your local clone.
- **Hooks:**
  - `pre-commit`: Validates Rust code formatting (`cargo fmt --all -- --check`) and workspace lints (`cargo clippy --workspace -- -D warnings`) when Rust or Cargo manifest files change.
  - `pre-push`: Runs full workspace validation (`just check`) and issue workflow audit (`just issues-check`).
  - Local custom overrides can be placed in `lefthook-local.yml` (ignored by git).
