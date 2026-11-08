# Changelog

All notable changes to this project are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/).

Every pull request that changes config, the CLI, the action, the release flow or behaviour adds an entry under `Unreleased` (one bullet, ending `(#issue, #PR)`), or carries the `skip-changelog` label. `just bump-version` moves `Unreleased` under the new version heading, and the Release workflow publishes that section as the GitHub release notes. Each version section links its migration guide, `docs/migrations/<version>.md`.

## [Unreleased]

> **Release-day break.** `jaoppb/tools@v0.2.0` defaults to `version: latest`. Once v0.3.0 is published it resolves to a release that has no `issues-check`, `repo-sync` or `project-sync` assets, and the install step fails. Pin `version: v0.2.0` to stay on v0.2.0, or move to `jaoppb/tools@v0.3.0` with `version: v0.3.0`. There are no compatibility shims.
>
> **Migration guide:** [v0.2.0 → v0.3.0](docs/migrations/v0.3.0.md).

### Breaking

- The three binaries `issues-check`, `repo-sync` and `project-sync` are replaced by one `tools <group> <verb>` CLI (`tools issues check|migrate`, `tools repo check|sync`, `tools project check|sync`, `tools config check|migrate`). Read-only verbs are always `check` and exit 1 when something is left to fix; `--dry-run` is gone except on `tools config migrate`. Release assets are one `tools-vX.Y.Z-<target>` archive and `scripts/install-tools.sh` installs only `tools` (its second argument is now the install directory) (#66, #73)
- Composite action: `tools` is removed, `issues-check-args` is now `issues-args`, `repo-sync-args` is now `repo-args`, and `version` must name a release that ships `tools` (#63, #66, #68, #73)
- `[github.project.status_mapping]` is removed and fails to load; board columns must equal `[workflow].statuses` exactly (trimmed, case-sensitive). `tools config migrate` rewrites the file (#57, #58, #84, #90)
- `tools project check|sync` refuse to run without a `[workflow]` block; there is no implicit preset (#57, #84)
- The `builtin:cosmo` gate is strict: a card is Ready only with Priority and Effort set (Effort at most `params.effort_limit`, 8), so adopters of the old default rules (Priority and Effort off) must keep their gate with a `triaged` fact. Run `tools project check` before the first `sync` (#57, #84)
- `references` and `text` in `[github.issues.rules]`, and any misspelled key, now fail to load (#60, #72)
- `prune_unmanaged` is replaced by `unmanaged = "ignore" | "warn" | "error" | "prune"` on `[github.labels]` and `[github.milestones]`, and unknown keys in those tables are rejected. Milestones default to `error`: every GitHub milestone missing from `[github.milestones].items` fails `tools repo check` (#76, #86)
- Label migrations come only from `[github.labels.migrations]`: the built-in legacy map is gone, and `tools issues check` exits 1 while a migration is pending (#61, #77)
- `--default-type` and `--default-area` are removed from `tools issues migrate` (#62, #85)

### Added

- `[workflow]` in `tools.toml`: statuses, facts and rules as CEL expressions evaluated first match wins, with the `builtin:cosmo` and `builtin:tools` presets, local `params`, `facts` and `[[workflow.status]]` rules overriding the preset, and AST-based analysis of every expression at load (#54, #55, #56, #70)
- `tools project check --explain <issue>` prints the facts and rules behind one card; `check` lines read `<issue> A -> B (<rule-id>)` or `<issue> frozen: <cause>`, then `N kept, M moved, K frozen` (#57, #84)
- `tools config check|migrate` (always built) rewrites a v0.2.0 `tools.toml` into `[workflow]` keeping comments; idempotent, with `--dry-run` (#58, #90)
- Action inputs `config-path`, `repo`, `check-project`, `sync-project`, `project-args` and `build-from-source` (#63, #66, #68, #73)
- `[github.issue_templates]` maps each issue template to its label and title prefix, and `tools repo check` starts with an offline check of the templates and of the label lists in `AGENTS.md` and `CONTRIBUTING.md` (#64, #69)
- `unmanaged` severities report undeclared labels and milestones by name (#76, #86)
- Release flow: `just bump-version` opens a PR instead of pushing to `main`, and the merged bump is tagged by the `Tag Release` workflow only when the push changed the workspace version (#52, #89, #91, #100)
- `CHANGELOG.md`, the `skip-changelog` label and `tools changelog check`, which fails a pull request that touches `crates/`, `action.yml`, `scripts/` or `.github/workflows/` without a changelog entry; the bump rotates `Unreleased` and the Release workflow publishes the section as release notes (#83)
- Migration guides `docs/migrations/v0.2.0.md` and `v0.3.0.md` (#79, #80, #87, #96)

### Changed

- `tools repo check`'s offline template and doc checks are opt-in: doc label lists are read only from the files named in `[github.labels].docs` (default empty, paths relative to `tools.toml`, rejected in `[github.overrides.<repo>.labels]`), and templates only when `[github.issue_templates]` is declared. The step runs only on `check` of the configured repository alone that covers labels, so a repository without `AGENTS.md` or `CONTRIBUTING.md` no longer fails, and a one-entry `[github].repositories` list now skips it (#93, #103)
- A task whose story is not on the Project waits for that story (fetched on demand); before it was never held (#57, #84)
- A param in `[workflow.params]` must keep the TOML type of the preset value, with a load error naming line and column; `area_prefix` is a list, and `tools config check` reports a single value and `migrate` wraps it (#58, #90, #92, #99)
- `[github.issues.rules]` is deprecated: `tools issues check` still reads it and prints a warning, `tools project` no longer does (#58, #90)
- The `tools.toml` repository schema is decoupled from the domain repository types (#97, #102)
- CI jobs run through the flake devShell, the toolchain pinned by `flake.lock` (#94, #98)
- `cel-interpreter` 0.10 and `cel-parser` 0.10 are replaced by `cel` 0.15 (#92, #99)
- Cosmo-specific code and wording removed from the core (`Target::cosmo`, dead rule flags) (#60, #72)

### Fixed

- A parent cycle froze nothing and overflowed the stack; the cycle and every card below a frozen parent are now frozen, reported, and exit 1 on `check` (#65, #67)
- An expression that crashes the interpreter or ranges over a non-list freezes its card instead of aborting `tools project check|sync` (#92, #99)
- `Tag Release` tags only when the push changed the workspace version (#91, #100)

## [0.2.0] - 2026-11-06

Migration guide: [v0.1.0 → v0.2.0](docs/migrations/v0.2.0.md).

### Added

- Packaged CLI binaries, the composite GitHub Action (`jaoppb/tools`) and `scripts/install-tools.sh` (#39)
- Configurable milestone format and phase naming in the audit rules (#38)
- Configurable, accent-insensitive area label prefixes (#32)
- Per-repository label and milestone overrides (#34)
- Multiple `--repo` arguments and organization targets in `repo-sync` (#37)
- Repository-qualified issue identity, cross-repository blockers and organization-level Projects V2 (#10, #11, #12)
- Configurable Project status field mapping, with Priority and Effort optional (#13)
- Organization-wide label and repository migration, and a default-type fallback for unlabeled issues in `issues-check --migrate` (#14, #33)
- Repository README and contributing guide (#31)

### Changed

- ADR and SPDD reference checks are removed from the core audit (#36)

### Fixed

- Nested comments and reviews are paginated by the repository text reader (#35)

## [0.1.0] - 2026-10-30

Migration guide: none, this is the first release.

### Added

- `issues-check`, `repo-sync` and `project-sync` binaries: the issue workflow audit, declarative repository and Project bootstrapping (settings, labels, milestones) and Project status sync (#4)
- Renaming of labels, milestones and Project names (#16)
- Issue templates, the PR template, merge-commit settings, Lefthook hooks, CI and the release workflow (#1, #3, #6, #7, #19)
- Workspace architecture rules with arch-lint (#5)

[Unreleased]: https://github.com/jaoppb/tools/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jaoppb/tools/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jaoppb/tools/releases/tag/v0.1.0
