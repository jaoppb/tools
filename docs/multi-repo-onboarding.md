# Multi-Repository Organization Onboarding Guide

This guide walks through standardizing and onboarding an entire GitHub organization (e.g. `gatuno-manga` with repositories such as `web`, `api`, `app`, `infra`) to `tools`.

---

## 1. Overview & Architecture

Adopting `tools` across a multi-repository organization establishes consistent quality gates, PR merge standards, issue taxonomy, and milestone tracking across all repositories.

The onboarding workflow consists of two core phases:
1. **Declarative Repository Provisioning (`tools repo`)**: Reconciles GitHub repository merge settings, labels (`type:*`, `area:*`), and milestones to match `tools.toml`.
2. **Issue Taxonomy Migration (`tools issues migrate`)**: Audits existing unstandardized issues and migrates legacy labels (e.g., `bug`, `enhancement`, `documentation`) into canonical `type:*` and `area:*` labels in bulk.

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
unmanaged = "warn"                # labels on GitHub that are not declared: ignore | warn | error | prune
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

# Legacy label migrations mapping legacy names to canonical labels.
# There are no built-in defaults: only the mappings declared here are applied.
[github.labels.migrations]
bug = "type:fix"
fix = "type:fix"
enhancement = "type:feat"
feature = "type:feat"
documentation = "type:docs"
chore = "type:chore"
refactor = "type:refactor"
spike = "type:spike"
ui = "area:web"
backend = "area:api"

[github.milestones]
unmanaged = "error"              # the default: an undeclared milestone fails `repo check`
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
release = true
```

### Per-Repository Overrides

Subsystem-specific labels and milestones go in `[github.overrides.<repo>.labels]` and `[github.overrides.<repo>.milestones]`. `tools repo` resolves the shared declarations plus the overlay of each target repository; repositories without an overlay get the shared declarations only, and overlays of repositories that are not targeted are ignored.

```toml
[github.overrides.web.labels]
unmanaged = "error"              # overrides the shared value; omit to inherit it
deprecated = ["accessibility"]   # unioned with the shared list
items = [
    { name = "area:ssr", color = "2ea44f", description = "Server-side rendering" },
    { name = "type:feat", color = "00ff00", description = "Web-specific feature color" }, # same name: replaces the shared item
]

[github.overrides.web.labels.renames]
"old-name" = "area:ssr"          # merged with shared renames; overlay wins on conflict

[github.overrides.infra.milestones]
items = [{ title = "v1.0.0", description = "Infra GA", state = "open" }]
```

Merge rules: `items` override by key (`name` for labels, `title` for milestones) and otherwise append; `deprecated` is a union; `renames` and label `migrations` are merged key by key; `unmanaged` replaces the shared value only when set. The shared section is optional: a repository may declare its labels or milestones only through its overlay. `issues-check --migrate` reads only `migrations`: there is no built-in map, `renames` (which rename the label itself) are not applied to issues, and with no configuration found nothing is remapped. A key not recognised inside an overlay (for example a misspelled `unmanged`) is a configuration error, but an overlay keyed by a repository that is not targeted is ignored.

---

## 3. Provisioning Repositories (`tools repo`)

### Step 3.1: Check Configuration Drift
Run an audit across all configured repositories without modifying remote state:

```bash
just repo-check
# or with an organization override:
cargo run -q -p tools -- repo check --org gatuno-manga
```

### Undeclared labels and milestones

`tools.toml` is the source of truth, so a label or milestone that exists on GitHub but is not declared in `items` is drift: declare it, or list it under `deprecated` to have it removed. What `tools repo` does about it is set per kind with `unmanaged`, on `[github.labels]`, `[github.milestones]` and in an overlay:

| `unmanaged` | `check` | `sync` |
|-------------|---------|--------|
| `ignore` | says nothing | says nothing |
| `warn` (labels' default) | names each item, exit 0 | names each item, exit 0 |
| `error` (milestones' default) | names each item, exit 1 | applies everything else, names each item, then exits 1 |
| `prune` | reports the deletion as drift, exit 1 | deletes the item |

Each reported item is named with its fix, for example `milestone 'v0.4.0' exists on GitHub but is not declared in tools.toml (error); declare it under [github.milestones].items or add it to deprecated`. Open and closed milestones are treated alike. Milestones default to `error` because they are few and always intentional; labels default to `warn` because collaborators add them by hand.

`prune_unmanaged` no longer exists. A file that still sets it fails to load with a message naming its table; replace `true` with `unmanaged = "prune"` and `false` with `unmanaged = "warn"`.

### Step 3.2: Preview Reconciliation
`repo check` is the preview: it reports every addition, update or removal that `repo sync` would make (exit code 1 when there is any, and for an undeclared item whose `unmanaged` is `error`) and writes nothing. Run it as in Step 3.1.

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

## 4. Migrating Legacy Issues (`tools issues migrate`)

Once labels and settings are provisioned, legacy issues must be migrated to comply with the rule that every issue has exactly one `type:*` label and at least one `area:*` label.

### Step 4.1: Preview the Migration
`issues check` audits the issues and ends with the label conversions `issues migrate` would write; it never writes:

```bash
just issues-check
# or via cargo:
cargo run -q -p tools -- issues check
```

Example preview output:
```text
Issue migration plan (3 issues to update):
  web#12: -bug +type:fix +area:web
  api#4: -enhancement +type:feat +area:api
  infra#8: -documentation +type:docs +area:infra
Run `tools issues migrate` to execute these changes.
```

### Step 4.2: Issues Lacking Area or Type Labels
`issues migrate` only applies the configured label renames; it never invents a label. Issues that still lack a type or area after migration are reported by `tools issues check`; label them by hand (or with `gh issue edit --add-label`).

### Step 4.3: Apply Migrations
`issues migrate` always writes. Execute the migrations directly against the GitHub REST API:

```bash
just issues-migrate
# or via cargo:
cargo run -q -p tools -- issues migrate
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

### Composite GitHub Action (`jaoppb/tools`)
The recommended way to enforce repository and issue compliance across external repositories is using the official composite action `jaoppb/tools`. It downloads pre-compiled release binaries with SHA256 verification and automatic runner caching—avoiding full monorepo checkout or Cargo compilation overhead.

#### Option A: Automated Compliance Checks
Use built-in flags (`check-issues`, `check-repo`) to run audits directly within the action:

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
          version: 'latest' # or pin to a specific release tag such as 'v0.2.0'
          check-issues: 'true'
          check-repo: 'true'
          github-token: ${{ secrets.GITHUB_TOKEN }}
```

#### Option B: Setup CLI on PATH
Install tools onto `$GITHUB_PATH` and invoke them with custom arguments in downstream steps:

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

      - name: Setup Tools CLI
        uses: jaoppb/tools@v0.2.0
        with:
          version: 'latest'
          github-token: ${{ secrets.GITHUB_TOKEN }}

      - name: Verify Repository Settings Drift
        run: tools repo check

      - name: Verify Issue Workflow Rules
        run: tools issues check
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

#### Action Inputs Reference

| Input | Description | Default |
|-------|-------------|---------|
| `version` | Release version tag to install (e.g. `v0.2.0` or `latest`) | `latest` |
| `install-dir` | Target directory where the binary is placed | `${{ runner.temp }}/tools-bin` |
| `github-token` | GitHub token for API requests and issue auditing | `${{ github.token }}` |
| `cache` | Enable cross-run caching of the downloaded binary | `true` |
| `build-from-source` | Build `tools` with cargo from the action's checkout instead of downloading a release | `false` |
| `config-path` | `tools.toml` passed as `--config` to every command the action runs | `""` |
| `check-issues` | Run `tools issues check` immediately after install | `false` |
| `check-repo` | Run `tools repo check` immediately after install | `false` |
| `check-project` | Run `tools project check` immediately after install | `false` |
| `sync-project` | Run `tools project sync` (writes) immediately after install | `false` |
| `issues-args` | Extra CLI flags passed to `tools issues check` | `""` |
| `repo-args` | Extra CLI flags passed to `tools repo check` | `""` |
| `project-args` | Extra CLI flags passed to `tools project check` and `tools project sync` | `""` |

### Nix Flake Execution
For environments using Nix, the binary can be executed directly without local installation:

```bash
# Audit issue workflow rules
nix run github:jaoppb/tools -- issues check

# Audit repository drift
nix run github:jaoppb/tools -- repo check
```

### Standalone Script & Cargo Fallback
You can also install the binary directly via the installation script:

```bash
curl -sSL https://raw.githubusercontent.com/jaoppb/tools/main/scripts/install-tools.sh | bash
```

Or build from source using Cargo (requires a Rust toolchain):

```bash
cargo install --git https://github.com/jaoppb/tools tools

# A partial build carries only the groups you name
cargo install --git https://github.com/jaoppb/tools tools --no-default-features --features repo
```
