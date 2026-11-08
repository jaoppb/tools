# Workflow engine: config-driven rules, no cosmo assumptions

Status: spike #54 done; the engine core, `[workflow]` config and presets landed in #56 (`crates/core/src/workflow/`, no feature gate); `tools project check|sync` runs on the engine since #57. Findings below. Target milestone: `v0.3.0` (spike + project-sync), `v0.4.0` (issues-check).

## Problem

`crates/core` still hardcodes the cosmo workflow. Findings from the audit (file:line as of `e5d233b`):

| Hardcoded | Where |
|-----------|-------|
| `type:` prefix, "exactly one" | `domain/facts/mod.rs:84`, `survey/issue/mod.rs:114`, `rules/labels/mod.rs:15` |
| `area:` ignoring `area_prefixes` | `domain/facts/mod.rs:92`, `domain/issue/migration.rs:90-91` |
| `epic` label (contradicted by `tools.toml` `deprecated`) | `domain/facts/mod.rs:108`, `parse/items/mod.rs:190` |
| epic > story > task, "task waits for story" | `domain/issue/mod.rs:168`, `domain/links/mod.rs:92`, `domain/target/mod.rs` |
| ~~5-state `Status` enum + aliases~~ done in #57: `StatusName`/`Statuses` value objects, `status_mapping` no longer read | `domain/issue/mod.rs:74-130`, `status_mapping.rs:10` |
| ~~State machine and reasons~~ done in #57: `domain/target` and `domain/plan` deleted | `domain/target/mod.rs`, `domain/plan/mod.rs:32-68` |
| Project field names `Status`/`Priority`/`Effort` (read by name) | `parse/items/mod.rs`, `query/status_field.graphql:4`; the default status is now `[workflow].default` (#57) |
| Ready gate `Effort <= 8`, "split the story" | `domain/facts/mod.rs:~145,~187` |
| Release = `v<N>.<N>.<N>` regardless of `milestone_format` | `domain/audit/release/mod.rs:5-41`, `rules/release/mod.rs` |
| Violation text (`v0.1.0`, `type:`, `epic`, "AGENTS.md section 10") | `domain/audit/violation/mod.rs:143-221` |
| Legacy label migrations (bug→type:fix, …) always merged | `domain/repo/label.rs:46-66` |
| ~~`CANONICAL_TYPES` for `--default-type`~~ flags removed in #62 | `domain/issue/mod.rs`, `crates/cli/src/issues/cli.rs` |
| ~~`Target::cosmo()` (`jaoppb/cosmo`, project 4)~~ done in #60 | `adapters/github/target/mod.rs:67-71` + behaviour tests |
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
- **Location.** Module `crates/core/src/workflow/` outside `domain` (domain stays std-only); `cel-interpreter` and `cel-parser` are regular dependencies. The `workflow` cargo feature of the prototype is gone (#56).
- **Config shape (#56).** `Config` holds the raw, serde-typed `[workflow]` block (`WorkflowConfig`, so `Config` keeps Clone/PartialEq; `Eq` was dropped because params may hold floats). `load_from_str`/`load_from_path` compile it and fail with `ConfigError::Workflow`; `Config::workflow()` returns the compiled `Workflow` (`None` without a block). Fields are `toml::Spanned`: undeclared status/default, unknown schema or preset, duplicate id, reserved or cyclic fact, bad param and CEL compile or unknown-name errors report `line N, column M` of the file. Errors inside a builtin preset carry no location. Unknown keys are plain TOML errors with their own location. `[github.issues.rules]` and `status_mapping` stay until #58.
- **Reference analysis (#56).** Load-time name checks parse each expression with `cel-parser` and walk the AST. Free identifiers are those not bound by an enclosing comprehension (scopes are respected, so a name bound in one macro does not excuse the same spelling elsewhere); operators and parser-expanded macros (`exists`, `all`, `filter`, `map`, `has`) are not function calls. Called functions must be built in or in the fixed helper set.
- **Frozen parent (#56).** A frozen parent (cycle, unreadable, eval error) freezes its whole subtree: the child gets no change and `Violation::ParentFrozen` naming it. Exposing partial facts was rejected: a frozen parent has no derived facts, any rule reading `issue.parent.<fact>` would error at run time anyway, and "no change" is the safe answer.
- **`ParentResolver` contract (#56).** Synchronous, object-safe (`&dyn`). `fetch` returns `Result<Subject, ResolveError>` with `NotFound`, `Forbidden` (token cannot read the repository) or `Failed(String)`. The engine asks once per issue number per run and caches the answer, errors included; only for parent-chain issues the Project lacks. Every error freezes the child (`Violation::ParentUnreadable { parent, error }`). The engine never retries; a transient failure costs that issue one run. Implementations must not panic.
- **`builtin:tools` (#56).** Same five statuses and `type_prefix`/`area_prefix`/`started` params as cosmo. Gate: exactly one type label, an area label, a milestone, all blockers closed. No Priority/Effort, parent, epic or membership; a stray parent link is ignored. Cosmo's closed/started/pull-request rules are otherwise identical. Tested with hand-built scenarios (`tests/behaviour/workflow/tools.rs`).
- **Project sync on the engine (#57).** `tools project check|sync` evaluates `Engine` over the Project snapshot; `domain/target`, `domain/plan` (`Moves`/`Gaps`/`Frozens`), the `Status` enum and `StatusMapping` are deleted. `[github.project.status_mapping]` is no longer read; board columns must equal `[workflow].statuses` exactly (trimmed, case-sensitive); #58 removes the key.
- **Value objects, not strings (#57).** `StatusName` exists only through `Statuses::parse` (the declared names plus the default); `RuleId` comes from the compiled block. A rule's `set` is `Set::Current` or `Set::Status(StatusName)`, checked at load. `Statuses::undeclared()` exists for readers that only need membership (the audit's `GhProject::new`), never for writing.
- **One decision per card (#57).** `Outcome::Move { from, to, rule } | Keep { rule: Option } | Frozen { cause }`, derived from the trace by `Decision::outcome()`. `Keep.rule` is `None` when no rule fired. Diagnostics (the `Explain` trace) are orthogonal. Summary line: `N kept, M moved, K frozen`; a move prints `<issue> A -> B (<rule-id>)`, a frozen card `<issue> frozen: <cause>`. Gaps are not printed (#59).
- **Exit codes (#57).** `check` exits 1 when any card moves **or** is frozen (an unreadable parent or a cycle needs a human); `sync` exits 0 after writing, frozen cards are reported. `check --explain <issue>` (`57` or `repo#57`) prints the `Explain` trace and exits 0. `--format json` is a follow-up.
- **No `[workflow]` block (#57).** `project check|sync` fail with a message naming what to add; there is no implicit preset. The target (owner/repo/project) is validated first.
- **GitHub `ParentResolver` (#57).** `GhProject` implements it: one GraphQL read of the issue in its own repository (`parent_issue.graphql`: labels, milestone, blockers, its own parent). A repository GitHub will not resolve ("Could not resolve to a Repository"), `FORBIDDEN`, "not accessible" and HTTP 403 map to `Forbidden`; an existing repository without that issue is `NotFound`; anything else is `Failed` with `gh`'s words. A parent outside the Project has no fields, no status and no pull requests. The items query now reads the parent's repository, so a cross-repo parent is no longer attributed to the child's repository.
- **`issue.prs` (#57).** The pulls query reads each pull request's number; `Snapshot` holds `Closes { pull, issue }` pairs and `issue.prs` is a list of `{number, repo}`. `closed_by_open_pr` is gone; the presets use `issue.prs.size() > 0`.
- **Parity oracle retired (#57).** The old state machine is deleted, so `tests/behaviour/workflow/parity.rs` pins the destinations it produced (recorded before deletion) as literal tables.

## Changelog: what moves under the strict `builtin:cosmo` gate (v0.3.0)

For adopters of `project-sync` with the old default `[github.issues.rules]` (priority and effort off). Interim home until a CHANGELOG exists.

- A card is **Ready only with Priority and Effort set** and Effort <= 8 (`params.effort_limit`); before, a card with type, area and milestone was Ready. Cards missing them stay in Backlog (no gap line is printed any more; `tools issues check` still reports them).
- A task whose **story is outside the Project** now waits for that story (fetched on demand); before, it was never held.
- A parent cycle, an unreadable parent or a parent in a repository the token cannot read **freezes** the card and exits 1 on `check`; before, a story cycle overflowed the stack.
- **Board columns must equal `[workflow].statuses`**; `status_mapping` aliases and case-insensitive matching are gone. An unknown column is an error naming it.
- Without a `[workflow]` block the command refuses to run.
- Reason text is the firing rule id (`gate-holds`), not "gate holds".

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

## Findings (spike #54)

**Verdict.** The design holds. `builtin:cosmo` reproduces the state machine's destination for every card of `items_live.json`, `items_edge.json` (also with every blocker open) and 19 + 6 hand-built transition and hierarchy scenarios, under `plan_with(strict_rules())`. Tests: `crates/core/tests/behaviour/workflow/`.

### Hypothesis 1: `cel-interpreter` 0.10 (confirmed, with gaps)

- Works: `exists`/`all`/`filter` macros, `has()`, ternary, `in`, `startsWith`, `size`, `null` comparison, custom functions (`fold`), child scopes. `default-features = false` is enough (no `regex`, no `chrono`).
- Compile errors are syntax only, with a position: `ERROR: <input>:1:33: Syntax error: missing ')'`. **There is no type checker**: `issue.fields.Effort <= "8"`, a missing key and an unknown identifier are *runtime* errors. The engine therefore validates names at load and treats runtime errors as a frozen issue.
- `Program::references()` lists free variables and functions, but (a) includes macro-bound names (`l`) and (b) lists operators as functions (`_&&_`, `!_`, `@in`). The prototype subtracted bound names by reading the identifier after `.exists(`-style openers. Replaced in #56 by an AST walk over `cel-parser` (see Decisions).
- Speed: 1000 issues, 12 facts and 7 rules each, parent chains included: **84 ms release, ~490 ms debug**. A child scope per issue over one shared root context matters (building `Context::default()` per issue was 2.7x slower in debug).
- Alternative (`rhai`, home-grown) not needed.

### Hypothesis 2: dependency-ordered evaluation (confirmed)

Algorithm, one pass, implemented in `workflow/engine.rs`:
1. Insert every Project card as a `Subject`.
2. For each card, walk its parent chain, fetching missing parents through `ParentResolver` (parent chain only, cached, a fetch error is stored, not raised). A repeat inside the current path records the whole loop as cyclic.
3. Evaluate each card with memoised recursion: the parent node first, then facts in dependency order (facts are sorted at load from `references()`; cycles between facts are a load error), then rules in order, first match wins. `issue.parent` is the parent node's whole `issue` value plus its derived facts and `next`.
4. A node is **frozen** (destination = current status, a `Violation`) when: it is on a cycle, its parent cannot be fetched, a fact or `when` errors, or **its parent is frozen** (new: a frozen parent has no derived facts to expose, and the safe answer is to change nothing). Never falls through to a later rule.
5. The trace (`Explain`) is a by-product of the same pass: every fact, every rule with its `when` source and one of `FIRED`/`no`/`ERROR`/`not reached`.

Sample, `items_live.json` `cosmo#20` (open blocker #76):

```
cosmo#20: Backlog -> Backlog
facts:
  has_area = true
  has_type = true
  triaged = true
  unblocked = false
  waits_for_parent = false
  gate_holds = false
  is_epic = false
rules:
  [no         ] epic-is-untouched: is_epic => @current
  [no         ] closed-is-done: issue.state == "closed" => Done
  [no         ] started-with-pull-request: issue.status in params.started && issue.prs.size() > 0 => In review
  [no         ] started-keeps-going: issue.status in params.started => In progress
  [FIRED      ] gate-closed: !gate_holds => Backlog
  [not reached] pull-request-open: issue.prs.size() > 0 => In review
  [not reached] gate-holds: true => Ready
```

### Hypothesis 3: `builtin:cosmo` parity (confirmed, three intentional differences)

The preset is `crates/core/src/workflow/presets/cosmo.toml`: 7 facts, 7 rules, params `type_prefix`, `area_prefix`, `epic_label`, `effort_limit`, `started`. Differences from the old code, each pinned by a test:
1. A readable parent outside the Project is fetched and applies (old: no card, no wait).
2. A parent cycle or an unreadable parent freezes the issue (old: stack overflow on a story cycle; no handling for unreadable).
3. The gate is the strict one (Priority and Effort required). The default `RulesConfig` toggles are gone.

Not reproduced, by decision: plan reason text (the firing rule id replaces it) and triage gaps.

### Fact schema v1

Every expression sees `issue`, `params`, `repo`, the derived facts by name, and the helper `fold(s)`.

| Name | Type | Notes |
|------|------|-------|
| `issue.number` | int | |
| `issue.state` | `"open"` / `"closed"` | not-planned counts as closed |
| `issue.status` | string | board column; the workflow `default` when the issue is not a card |
| `issue.labels` | list of string | raw names |
| `issue.milestone` | string or null | title |
| `issue.fields` | map | only fields that are set; `has(issue.fields.X)` guards |
| `issue.blockers` | list of `{number, state}` | cross-repo blockers as the adapter reads them |
| `issue.prs` | list of `{number, repo}` | the open pull requests that close the issue; `closed_by_open_pr` is gone (#57) |
| `issue.parent` | null or the parent's `issue` map | plus `next` and the parent's derived facts; recursive up the chain |
| `params.*` | scalar or list | opaque |
| `repo` | string | repository name |
| `fold(s)` | function | lowercase + ascii-fold |

`issue.next` exists only on a parent (the child's own `next` is the result being computed). Facts compile against these roots plus each other.

### Open questions, resolved or re-stated

- Resolved: schema versioning (`schema = 1`), plan reasons (rule id), missing fields (absent key).
- Resolved in #57: `issue.prs` is a list (PR numbers travel through `Snapshot` as `Closes`).
- Re-stated: `repo` is only a name today, not a map; gaps; `extends` from file/URL.
- Resolved in #56: a frozen parent freezes its whole subtree (see Decisions).

## Risks

- **Dependency-ordered evaluation across repos** is the hardest part: on-demand fetch, ordering, cycle exclusion, and the explain trace all interact. Prototype this first in the spike, against the existing `items_live.json` fixtures.
- **CEL in Rust**: `cel-interpreter` is less mature than cel-go. Verify macro support (`exists`, `filter`, `in`), error locations and performance on ~1k issues before committing.
- **Breaking v0.3.0** for every adopter of `[github.issues.rules]`. Mitigated by the migration command; still needs a loud changelog.
- **Preset drift.** `builtin:cosmo` must reproduce today's behaviour exactly; the existing behaviour tests are the oracle and must pass against the preset before the old code is deleted.

- **Strict preset is a behaviour change** for adopters running with `priority`/`effort` toggles off (the default today); the migration command and changelog must say so.
- **Existing code recurses without a cycle guard** (`target_with` -> `story_has_started`); a story cycle overflows the stack today. The engine's cycle exclusion fixes it, but the old path stays until deleted.

## Issue map

Spike / engine (`area:core`, `v0.3.0` unless noted):
1. ~~#54~~ spike, done: this document and the prototype (#55).
2. #56 feat: `[workflow]` config in `tools.toml`, `[workflow.params]`, both presets, location-bearing load errors, AST-based reference analysis, feature gate removed, `ParentResolver` contract. Done by this issue.
3. ~~#57~~ feat, done: project-sync driven by the engine: a `ParentResolver` backed by the GitHub adapter (cross-repo, parent chain only; every `ResolveError` freezes the child), `--explain <issue>` printing `Explain`, `Violation` reporting, Project statuses as strings instead of the `Status` enum. Decide what moves with the new strict gate (changelog).
4. #58 feat: config migration for `[github.issues.rules]` and `status_mapping`; map `priority`/`effort` toggles to the strict gate and `area_prefixes` to `params.area_prefix` (the toggles-off case needs a documented override or a rule shadow). Removes the old keys.
5. #59 feat (`v0.4.0`): issues-check driven by `[[workflow.audit]]`; decide where triage gaps live; delete `domain/audit/rules`, violation text, `Status` enum, `domain/plan` reasons.

Independent of the engine:
6. #60 chore: remove `Target::cosmo()`, "Cosmo" wording, dead `references`/`text` flags and `domain/audit/text`.
7. #61 refactor: label `default_migrations` becomes config only.
8. #62 refactor: `--default-type`/`--default-area` removed from `issues migrate` (the workflow rules cannot add labels); `CANONICAL_TYPES` gone.
9. #63 feat: action.yml `config-path` input, project-sync step, `REPO` input.
10. #64 chore: single source of truth for issue templates and docs label/milestone lists against `tools.toml`.
11. ~~#65~~ fix, done: story cycle overflows the stack in `domain/target` (the path was deleted by #57; the engine freezes cycles).
