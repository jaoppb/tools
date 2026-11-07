# Workflow engine: config-driven rules, no cosmo assumptions

Status: grilled (spike #54 round 2), prototype pending. Target milestone: `v0.3.0` (spike + project-sync), `v0.4.0` (issues-check).

## Problem

`crates/core` still hardcodes the cosmo workflow. Findings from the audit (file:line as of `e5d233b`):

| Hardcoded | Where |
|-----------|-------|
| `type:` prefix, "exactly one" | `domain/facts/mod.rs:84`, `survey/issue/mod.rs:114`, `rules/labels/mod.rs:15` |
| `area:` ignoring `area_prefixes` | `domain/facts/mod.rs:92`, `domain/issue/migration.rs:90-91` |
| `epic` label (contradicted by `tools.toml` `deprecated`) | `domain/facts/mod.rs:108`, `parse/items/mod.rs:190` |
| epic > story > task, "task waits for story" | `domain/issue/mod.rs:168`, `domain/links/mod.rs:92`, `domain/target/mod.rs` |
| 5-state `Status` enum + aliases | `domain/issue/mod.rs:74-130`, `status_mapping.rs:10` |
| State machine and reasons | `domain/target/mod.rs`, `domain/plan/mod.rs:32-68` |
| Project field names `Status`/`Priority`/`Effort`, default Backlog | `parse/items/mod.rs:102,148,156`, `query/status_field.graphql:4` |
| Ready gate `Effort <= 8`, "split the story" | `domain/facts/mod.rs:~145,~187` |
| Release = `v<N>.<N>.<N>` regardless of `milestone_format` | `domain/audit/release/mod.rs:5-41`, `rules/release/mod.rs` |
| Violation text (`v0.1.0`, `type:`, `epic`, "AGENTS.md section 10") | `domain/audit/violation/mod.rs:143-221` |
| Legacy label migrations (bug→type:fix, …) always merged | `domain/repo/label.rs:46-66` |
| `CANONICAL_TYPES` for `--default-type` | `domain/issue/mod.rs:17-27`, `issues-check/src/cli.rs:267` |
| `Target::cosmo()` (`jaoppb/cosmo`, project 4) | `adapters/github/target/mod.rs:67-71` + behaviour tests |
| Dead `references`/`text` flags, unused `domain/audit/text` | `config/mod.rs:221-228` |
| "Cosmo" wording | `adapters/github/project/mod.rs:1`, `query/item_of_issue.graphql:1` |
| Type set duplicated in issue templates, AGENTS.md, CONTRIBUTING.md, README | `.github/ISSUE_TEMPLATE/*.yml`, docs |
| action.yml: no `config-path`, no project-sync step, `REPO` not an input | `action.yml`, `scripts/install-tools.sh:5` |

## Design

A declared vocabulary, a typed fact schema, and rules written in CEL. Inspired by Kubernetes admission policies (CEL + schema) and Jira workflows (statuses + ordered transitions).

```toml
[workflow]
extends  = "builtin:cosmo"         # or "builtin:tools"; omit for a blank slate
statuses = ["Backlog", "Ready", "In progress", "In review", "Done"]
default  = "Backlog"

[workflow.facts]                   # derived facts, named once
is_epic  = 'issue.labels.exists(l, l == "epic")'
role     = 'is_epic ? "epic" : (issue.parent != null && issue.parent.role == "story" ? "task" : "story")'
has_type = 'issue.labels.filter(l, l.startsWith("type:")).size() == 1'
triaged  = 'has_type && issue.milestone != null && issue.fields.Effort <= 8'

[[workflow.status]]                # first match wins, file order
id = "closed-is-done"
when = 'issue.state == "closed"'
set  = "Done"

[[workflow.status]]
id = "task-waits-for-story"
when = 'role == "task" && issue.parent.next in ["Backlog", "Ready"]'
set  = "Backlog"

[[workflow.audit]]                 # same language, same facts
id = "one-type-label"
when = '!has_type'
message = 'exactly one "type:" label is required'
```

## Decisions

- Scope is a fully pluggable workflow: statuses, hierarchy and transitions are data, not enums.
- Rules are a declarative list evaluated **first match wins, in file order**.
- Conditions are **CEL** expressions (`cel-interpreter`). The CEL syntax and the fact schema are part of the config contract and are versioned.
- Fact schema is the API: `issue.*`, `issue.parent.*`, `issue.prs`, `issue.fields.<Project field>`, `repo`, plus helpers like `release()`. Project field names stop being code; they are keys in `issue.fields`.
- `issue.parent.status` is the parent's current board column; `issue.parent.next` is the parent's status computed in this run. Both exist. Reading `next` forces dependency-ordered evaluation (parents first).
- Cross-repo parents are fetched on demand, parent chain only (no subtree). A repo the token cannot read is a hard error for that issue.
- Cycle detection runs in the engine before evaluation. An issue in a cycle gets no status change and a violation; therefore `parent.next` is never `null` for an evaluated issue.
- `[github.issues.rules]` toggles and `[github.project.status_mapping]` are **removed** in v0.3.0, replaced by `[workflow]`, with a one-shot config migration command.
- `extends`: local rules run first, builtin rules are appended. Shadow a builtin rule by reusing its `id`. **No removal mechanism**: if a preset does not fit, do not extend it.
- Two presets ship: `builtin:cosmo` (epic/story/task, parent required, membership, Ready gate) and `builtin:tools` (standalone issues, no parent, no membership).
- `--explain <issue>` prints every rule, its evaluated `when`, and which one fired. Non-negotiable; a rule list without it is undebuggable.
- Load-time validation: every expression compiles against the schema, every `set` names a declared status, unknown keys are errors with a TOML location.
- **Vocabulary is data, not code.** The engine never knows the words "type", "area" or "epic". `[workflow.params]` is an opaque TOML table (scalars/lists), merged local-over-preset and exposed to every CEL expression as `params`; Rust never reads specific keys. `builtin:cosmo` ships `params = { type_prefix = "type:", area_prefix = "area:", epic_label = "epic", effort_limit = 8 }` and facts built on them (`has_type = 'issue.labels.filter(l, l.startsWith(params.type_prefix)).size() == 1'`). A repo overrides a param without shadowing a rule.
- **Helpers are generic only** (string/version functions, fixed per schema version): `fold(s)` = lowercase + ascii-fold (deunicode), `release()`. No label-semantic helpers.
- **Schema version.** `[workflow] schema = 1`, optional, default 1. An unknown value is a load error with a TOML location. The prototype implements 1 only.
- **Keep status.** A rule says "no change" with the reserved `set = "@current"`, validated at load next to declared statuses (epic passthrough, no demotion of started cards).
- **Parent outside the Project.** A readable parent that is not a Project card is fetched and the rules apply to it (its `next` is computed). This intentionally diverges from today's "no card, no wait" (`domain/target` `story_has_started`); a test pins it.
- **builtin:cosmo gate is strict**: type, area, milestone, Priority and Effort required, Effort <= `params.effort_limit`. The oracle is `plan_with(strict_rules())`, not the default plan.
- **Missing field.** `issue.fields` holds only fields that are set; presets guard with `has(issue.fields.X)`.
- **Eval error.** A runtime error in a `when` freezes that issue: no change, a violation, the rule marked `error` in the trace. Never aborts the run, never falls through to later rules.
- **Reason = firing rule `id`.** The oracle compares `(card, destination)` only; old pair-derived reason text is not reproduced.
- **Prototype location.** Cargo feature `workflow` on `tools-core`, module `crates/core/src/workflow/` outside `domain` (domain stays std-only); `cel-interpreter` optional.
- Increment 1 = spike doc + engine driving **project-sync only**. issues-check keeps the current rule code until increment 2.

## Non-goals (v0.3.0)

- Audit rules in CEL (`[[workflow.audit]]`) — increment 2.
- Editing or creating Project fields; the engine reads fields, it does not provision them.
- Turing-complete or user-extensible functions in CEL; the helper set is fixed per schema version.
- Generating issue templates and docs from `tools.toml` — separate issue, not blocked on the engine.
- Removing or disabling individual preset rules.
- **Gaps** (`needs triage: no Priority`, `split the story`): not in the status rules; the prototype compares destinations only. Follow-up: audit-style rules or a per-rule gap list.

## Open questions

- **`extends` from a file/URL** (`extends = "./workflow.toml"`, `"github:org/repo/path"`) for org-wide policies. Deferred; builtin-only in v0.3.0.
- **Gap delivery.** Where triage gaps live once status rules own the gate (see non-goals). Decide in the audit design.
- Resolved in the grill: schema versioning, plan reasons, missing fields (see Decisions).

## Risks

- **Dependency-ordered evaluation across repos** is the hardest part: on-demand fetch, ordering, cycle exclusion, and the explain trace all interact. Prototype this first in the spike, against the existing `items_live.json` fixtures.
- **CEL in Rust**: `cel-interpreter` is less mature than cel-go. Verify macro support (`exists`, `filter`, `in`), error locations and performance on ~1k issues before committing.
- **Breaking v0.3.0** for every adopter of `[github.issues.rules]`. Mitigated by the migration command; still needs a loud changelog.
- **Preset drift.** `builtin:cosmo` must reproduce today's behaviour exactly; the existing behaviour tests are the oracle and must pass against the preset before the old code is deleted.

- **Strict preset is a behaviour change** for adopters running with `priority`/`effort` toggles off (the default today); the migration command and changelog must say so.
- **Existing code recurses without a cycle guard** (`target_with` -> `story_has_started`); a story cycle overflows the stack today. The engine's cycle exclusion fixes it, but the old path stays until deleted.

## Issue map

Spike / engine (`type:spike`, `area:core`, `v0.3.0`):
1. Spike: fact schema, CEL evaluation, dependency-ordered evaluation with cross-repo parents and cycle exclusion; prototype against fixtures.
2. feat: `[workflow]` config, CEL rules, `builtin:cosmo` and `builtin:tools` presets, load-time validation.
3. feat: project-sync driven by the workflow engine; `--explain`.
4. feat: config migration command for `[github.issues.rules]` and `status_mapping`.
5. feat (v0.4.0): issues-check driven by `[[workflow.audit]]`; delete `domain/audit/rules`, `violation` text, `Status` enum.

Independent of the engine (can start now):
6. chore: remove `Target::cosmo()`, "Cosmo" wording, dead `references`/`text` flags and `domain/audit/text`.
7. refactor: label `default_migrations` becomes config only (`[github.labels] migrations`), no implicit merge.
8. refactor: `--default-type` validates against `[github.labels.items]`, not `CANONICAL_TYPES`; migration uses configured prefixes.
9. feat: action.yml `config-path` input, project-sync step, `REPO` input.
10. chore: generate or check issue templates and docs label/milestone lists against `tools.toml`.
