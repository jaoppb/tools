# Multi-Repository Organization Onboarding Guide

This guide walks through standardizing and onboarding an entire GitHub organization (e.g. `gatuno-manga` with repositories such as `web`, `api`, `app`, `infra`) to `tools`.

---

## 1. Overview & Architecture

Adopting `tools` across a multi-repository organization establishes consistent quality gates, PR merge standards, issue taxonomy, and milestone tracking across all repositories.

The onboarding workflow consists of two core phases:
1. **Declarative Repository Provisioning (`repo-sync`)**: Reconciles GitHub repository merge settings, labels (`type:*`, `area:*`), and milestones to match `tools.toml`.
2. **Issue Taxonomy Migration (`issues-check --migrate`)**: Audits existing unstandardized issues and migrates legacy labels (e.g., `bug`, `enhancement`, `documentation`) into canonical `type:*` and `area:*` labels in bulk.

---

## 2. Declarative Organization Configuration (`tools.toml`)

All repository settings, labels, and issue migration rules are configured declaratively in `tools.toml`:

```toml
[github.repository]
owner = "gatuno-manga"
name = "tools"

# Multi-repository organization targets
[github]
repositories = [
    "web",
    "api",
    "app",
    "infra",
]

[github.repository.settings]
allow_merge_commit = true
allow_squash_merge = false
allow_rebase_merge = false
delete_branch_on_merge = true
merge_commit_title = "PR_TITLE"
merge_commit_message = "PR_BODY"

[github.labels]
deprecated = [
    "bug",
    "enhancement",
    "documentation",
    "duplicate",
    "good first issue",
    "help wanted",
    "invalid",
    "question",
    "wontfix",
]
items = [
    { name = "type:feat", color = "1d76db", description = "New behaviour or capability" },
    { name = "type:fix", color = "d73a4a", description = "Something is broken" },
    { name = "type:chore", color = "8b949e", description = "Tooling, CI, housekeeping, dependencies" },
    { name = "type:docs", color = "0075ca", description = "Documentation, guides, specifications" },
    { name = "type:refactor", color = "5319e7", description = "Restructure without changing behaviour" },
    { name = "type:spike", color = "fbca04", description = "Time-boxed research that answers a question" },
    { name = "area:web", color = "2ea44f", description = "Frontend web application" },
    { name = "area:api", color = "0e8a16", description = "Backend API services" },
    { name = "area:app", color = "5319e7", description = "Mobile client applications" },
    { name = "area:infra", color = "0075ca", description = "Infrastructure and deployments" },
]

# Legacy label migrations mapping legacy names to canonical labels
[github.labels.migrations]
bug = "type:fix"
enhancement = "type:feat"
documentation = "type:docs"
ui = "area:web"
backend = "area:api"

[github.milestones]
items = [
    { title = "v0.1.0", description = "Initial milestone", state = "open" },
    { title = "v0.2.0", description = "Next milestone", state = "open" },
]

[github.issues.rules]
cycle = true
labels = true
membership = false
milestone = true
parent = false
references = false
release = true
text = false
```

---

## 3. Provisioning Repositories (`repo-sync`)

### Step 3.1: Check Configuration Drift
Run an audit across all configured repositories without modifying remote state:

```bash
just repo-check
# or with an organization override:
cargo run -p repo-sync -- --check --org gatuno-manga
```

### Step 3.2: Preview Reconciliation
Preview planned additions, updates, or removals:

```bash
cargo run -p repo-sync -- --dry-run
```

### Step 3.3: Apply Provisioning Across All Repositories
Reconcile settings, labels, and milestones across all member repositories:

```bash
just repo-bootstrap
# or targeting a specific organization:
just org-sync gatuno-manga
```

You can also scope synchronization to specific components if needed:
- `just settings-sync` — Repository merge settings only
- `just labels-sync` — Label taxonomy only
- `just milestones-sync` — Milestones only

---

## 4. Migrating Legacy Issues (`issues-check --migrate`)

Once labels and settings are provisioned, legacy issues must be migrated to comply with the rule that every issue has exactly one `type:*` label and at least one `area:*` label.

### Step 4.1: Dry-Run Migration Preview
Inspect existing issues across all target repositories and preview planned label conversions:

```bash
just issues-migrate
# or via cargo:
cargo run -p issues-check -- --migrate
```

Example preview output:
```text
Issue migration plan (3 issues to update):
  web#12: -bug +type:fix +area:web
  api#4: -enhancement +type:feat +area:api
  infra#8: -documentation +type:docs +area:infra
Run with --apply to execute these changes.
```

### Step 4.2: Handling Issues Lacking Area Labels
If issues have legacy type labels but no area indicator, provide a fallback default area:

```bash
cargo run -p issues-check -- --migrate --default-area web --repo web
```

### Step 4.3: Apply Migrations
Execute the migrations directly against the GitHub REST API:

```bash
just issues-migrate apply=true
# or via cargo:
cargo run -p issues-check -- --migrate --apply
```

Once complete, run the audit to verify all issues satisfy workflow rules:

```bash
just issues-check
```

---

## 5. Ongoing CI/CD Enforcement

To prevent configuration drift, incorporate validation into repository CI workflows and Git pre-push hooks:

### Lefthook Git Hooks
Run `just setup` locally to configure hooks:
- **`pre-commit`**: Validates code formatting and clippy lints.
- **`pre-push`**: Validates workspace checks (`just check`) and issue taxonomy (`just issues-check`).

### GitHub Actions Workflow
Add a workflow step to your repository's CI pipeline:

```yaml
name: Verify Compliance
on: [push, pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Verify Repository Settings Drift
        run: cargo run -p repo-sync -- --check
      - name: Verify Issue Workflow Rules
        run: cargo run -p issues-check
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```
