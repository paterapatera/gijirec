---
name: sdd-impl
description: Implement tasks using TDD with subagent dispatch. Requires `ready_for_implementation: true`. Autonomous runs dispatch one implementer per major number (`1`, `2`, …), not per `N.M`. Manual selection still uses the given task ids. Target spec is the required first argument.
disable-model-invocation: true
---


# sdd-impl Skill

<background_information>
You operate with two layers of mode:

**Invocation** (from arguments):
- **Autonomous** (no task numbers): Run all pending tasks under the selected execution mode below
- **Manual** (task numbers provided): Execute selected tasks in the main context (`direct`-leaning; see Execution Mode Selection)

**Execution mode** (from `complexity_tier` / task-count / user override — see Step 2):
- **`direct`**: Parent (or a single agent) implements sequentially; reviewer once at selection/feature end — do not spawn per-task implementer×reviewer pairs
- **`wave`**: Default for tier M — parent is controller only (does not implement the batch); one implementer per **major** (`1`, `2`, …), sticky on happy path, two-tier review per major; `(P)` may run ready majors concurrently
- **`strict`**: Tier L / user-forced — same as `wave`, plus Integration/Validation majors stay solo and split a major earlier when excerpts/change-set are tight; failure-path fresh agents stay mandatory

- **Success Criteria**:
  - All tests written before implementation code
  - Code passes all tests with no regressions
  - Tasks marked as completed in tasks.md only after batch/selection `sdd-verify-completion` returns `VERIFIED`
  - Implementation aligns with design and requirements
  - Independent reviewer approves each batch (or selection/feature end under `direct`) before the completion gate
  - Feature success reported only after Step 4 `FEATURE_GO` verification
  - Terminal `/sdd-validate-impl` + `FEATURE_GO` remain for all execution modes (including `direct` / tier S)
</background_information>

<instructions>

## Startup: Resolve Target Feature

Do this **before** Step 1. Do **not** guess from chat history. Do **not** pick a spec by scanning `docs/specs/` or choosing the only `ready_for_implementation` spec. Do **not** load orchestrator routing files; follow the rules below.

**Classify each argument token** (skip mode overrides: `strict`, `wave`, `direct`, `full`):

| Token shape | Meaning |
| ----------- | ------- |
| Task-id list: only `N` or `N.M` ids separated by commas and/or spaces (e.g. `1.1`, `1,2,3`, `1.1,1.2`) | **Manual invocation** selection — not a feature name |
| Any other remaining token | **Explicit `<feature>`** (first such token wins) |

**Resolve `<feature>`:**

1. **Explicit `<feature>` is required.** First remaining token that is not a task-id list.
2. **Stop and ask for a spec name** if no such token is present. Do not use the git branch. Do not guess from chat history. Do not scan `docs/specs/`.
3. After resolve, if `docs/specs/<feature>/spec.json` is missing → **stop**; report the attempted name. Do not scan other specs.

Use the resolved `<feature>` for all later steps (context paths, `spec.json`, `/sdd-validate-impl <feature>`). Do **not** pass a task-id list as the feature name.

Examples: `/sdd-impl 001-login` (autonomous); `/sdd-impl 001-login 1.1,1.2` (manual). `/sdd-impl` and `/sdd-impl 1.1,1.2` (no feature) → stop.

## Step 1: Gather Context

If steering/spec context is already available from conversation, skip redundant file reads.
Otherwise, load all necessary context:
- `docs/specs/{feature}/spec.json`, `requirements.md`, `design.md`, `tasks.md`
- Core steering context: `product.md`, `tech.md`, `structure.md`
- Additional steering files only when directly relevant to the selected task's boundary, runtime prerequisites, integrations, domain rules, security/performance constraints, or team conventions that affect implementation or validation
- Relevant local agent skills or playbooks only when they clearly match the task's host environment or use case; read the specific artifact(s) you need, not entire directories

### Load rules (persistent docs)

| 資料 | このフェーズ |
|------|--------------|
| feature req/design/tasks | **主（抜粋）** — parent injects batch excerpts; do not default to full-file Read for subagents |
| contracts | **主（関連）** — task `_Contracts:` paths and/or Persistent References for touched surfaces; include as excerpt |
| architecture | **関連のみ** — boundary/dependency-direction slices only when the batch changes them |
| ADR | 境界変更時のみ（関連 1–2） |

- Never glob-bulk-Read `docs/contracts/**` or `docs/architecture/**`
- Procedure: **index (if needed) → Persistent References / `_Contracts:` / task-declared paths → those files only**
- Do not “read everything just in case”
- Extra persistent excerpts (contracts/architecture/ADR beyond feature excerpts): aim **~80–150 lines** total; if over, cut paths or split **within** the major

### Parallel Research

The following research areas are independent and can be executed in parallel:
1. **Spec context loading**: spec.json, requirements.md, design.md, tasks.md
2. **Steering, playbooks, & patterns**: Core steering, task-relevant extra steering, matching local agent skills/playbooks, and existing code patterns

After all parallel research completes, synthesize implementation brief before starting.

### Preflight

**Validate readiness**:
- Verify `ready_for_implementation: true` in spec.json (stop if not, see Safety & Fallback)

**Discover validation commands**:
- Inspect repository-local sources of truth in this order: project scripts/manifests (`package.json`, `pyproject.toml`, `go.mod`, `Cargo.toml`, app manifests), task runners (`Makefile`, `justfile`), CI/workflow files, existing e2e/integration configs, then `README*`
- Derive a canonical validation set for this repo: `TEST_COMMANDS`, `BUILD_COMMANDS`, and `SMOKE_COMMANDS`
- Prefer commands already used by repo automation over ad hoc shell pipelines
- For `SMOKE_COMMANDS`, choose the lightest trustworthy runtime-liveness check for the app shape (for example: root URL load, Electron launch, CLI `--help`, service health endpoint, mobile simulator/e2e harness if one already exists)
- Keep the full command set in the parent context, and pass only the task-relevant subset to implementer and reviewer sub-agents

**Establish repo baseline**:
- Run `git status --porcelain` and note any pre-existing uncommitted changes

## Step 2: Select Tasks & Determine Mode

**Parse arguments**:
- `<feature>` is already resolved in Startup (explicit argument). Do not re-parse `$1` as the feature name.
- If a task-id list is present in the arguments (see Startup): **manual invocation** (`direct`-leaning)
- If no task numbers: **autonomous invocation** (all pending tasks; execution mode from selection below)

**Build task queue**:
- Read tasks.md. **Dispatch unit** (autonomous `wave` / `strict`) is the **major number** (`1`, `2`, …) — one implementer for all remaining executable work under that major. Do **not** spawn one implementer per `1.1` / `1.2`.
- Executable work items remain `X.Y` sub-tasks (or a collapsed executable major `N.` with no children). Container-only major headers are not themselves executed; their children are.
- Skip tasks with `_Blocked:_` annotation
- For each selected task, check `_Depends:_` annotations -- verify referenced tasks are `[x]`
- If prerequisites incomplete, execute them first or warn the user
- Use `_Boundary:_` annotations to understand the task's component scope
- Count **executable sub-tasks** in the queue (incomplete `X.Y` or executable majors that are not `_Blocked:_` and whose `_Depends:_` are satisfied or will be run in this invocation) — used for mode fallback only, not as the dispatch grain

### Execution Mode Selection

Do **not** reinvent tier scoring. Prefer `spec.json` `complexity_tier` written by `sdd-orchestrate` (`rules/complexity-tier.md`). Key name: **`complexity_tier`** (`"S"` | `"M"` | `"L"`).

**Default selection** (first match wins after overrides):

1. **User override** (highest priority): explicit requests → force that execution mode; do not recompute from tier.
   - 「strict で」/ `strict` / `full` → **`strict`**
   - 「全部サブエージェントで」/ `wave` / 「wave で」 → **`wave`** (unless they also said strict)
   - 「direct で」/ `direct` / 「親だけで」 → **`direct`**
2. **Manual invocation** (task-id list in the arguments) → **`direct`** (main-context sequential; same as Manual Mode below).
3. Read `docs/specs/{feature}/spec.json` → `complexity_tier`:
   - `"S"` → **`direct`**
   - `"M"` → **`wave`**
   - `"L"` → **`strict`**
4. **Missing `complexity_tier`**: estimate from executable sub-task count in the queue (this is **impl execution mode only** — distinct from orchestration treating missing tier as flow-path **L**; see `sdd-orchestrate/rules/complexity-tier.md` § Scope note):
   - ≤ 3 → **`direct`**
   - ≤ 12 → **`wave`**
   - \> 12 → **`strict`**

**Do not break existing short-circuits**: Path B direct implementation (no `/sdd-impl`) stays outside this skill. Quality gates are **not** dropped for tier S / `direct` — selection/feature-end review + Step 4 terminal validate remain.

**Report** (exactly one line at run start, before first implementer dispatch):
`Execution mode: <direct|wave|strict> (reason: <tier S|M|L | task-count=N | manual | user override>)`

### Mode behavior summary

| Mode | When (default) | Behavior |
|------|----------------|----------|
| `direct` | tier **S**, or executable sub-tasks ≤ 3, or manual invocation | Parent (or single agent) implements sequentially. No per-task fresh implementer×reviewer pairs. Reviewer **once** at selection end (manual) or after all pending tasks / feature end (autonomous `direct`). Then deferred `sdd-verify-completion` + Step 4 validate. |
| `wave` | tier **M**, or missing-tier with 4–12 tasks | Full **major-number** dispatch: parent is controller only (does not implement the batch); one implementer per major (`1`, `2`, …), sticky happy path, parent mechanical + judgment reviewer per major, `(P)` parallel across ready majors when the contract holds. |
| `strict` | tier **L**, or missing-tier with \> 12 tasks, or user「strict」 | Same as `wave`, plus: Integration / Validation **majors** stay solo (do not parallel them with implementation majors); split a major earlier when change-set or Spec Excerpts budget is tight. Failure-path **fresh** debugger / post-debug implementer stays mandatory (unchanged). Terminal validate unchanged. |

## Step 3: Execute Implementation

If execution mode is **`direct`**, use **Manual Mode** below (even when invocation was autonomous — treat the full pending queue as the selection). If **`wave`** or **`strict`**, use **Autonomous Mode** with the batch-definition deltas noted under `strict`.

### Autonomous Mode (sub-agent dispatch — `wave` / `strict`)

**Batch definition rules** (parent builds the next batch from `tasks.md`):

1. Consider only incomplete executable work (`X.Y` sub-tasks, or a collapsed executable major `N.` with no children).
2. Skip tasks with `_Blocked:_`.
3. Do not include tasks whose `_Depends:_` are unsatisfied.
4. **Major number is the dispatch unit**: Form the next batch from the **lowest incomplete major number** whose remaining executable tasks have satisfied dependencies. The batch is **all** remaining incomplete executable work under that major (e.g. major `1` → `1.1`, `1.2`, `1.3` together; or executable `1.` alone). Do **not** spawn a separate implementer for each `N.M`. Do **not** mix different majors in one batch.
5. **`_Wave:_` does not form the dispatch batch.** It remains a phase-order annotation (Foundation → Core → Integration → Validation). Ignore Wave when choosing which major to dispatch. If a major must be split (excerpt budget / change-set too large), split **within** that major in listed order (optionally grouping by `_Wave:_` inside the major); never pull another major into the same implementer.
6. Split a major into sequential sub-batches of the **same** major only when:
   - Spec Excerpts would exceed the line budget (see Pre-dispatch Spec Excerpts)
   - The estimated change set is too large for one implementer
   - Under `strict`, an Integration / Validation major must stay **solo** (do not mix those tasks with other work in the same implementer, and do not parallel that major with implementation majors)
7. README / pure-documentation-only tasks may ride along in the same major. There is **no** sub-task count cap of 4/2 as the default split.
8. Do **not** split a major solely to parallelize its `(P)` sub-tasks. Intra-major `(P)` children run in **one** implementer, in listed order.

**`(P)` execution contract** (policy: promote markers to conditional parallel dispatch — not informational):

`(P)` means the parent **may** dispatch that **major** in parallel with other ready `(P)` majors when **all** of the following hold:
- **Different `_Boundary:_`** for the majors as wholes (the set of boundaries in each major do not overlap)
- **Closed dependencies**: no shared incomplete `_Depends:_` between the candidate majors (each major's Depends are already `[x]` or closed within that major)
- **Non-overlapping change paths**: planned paths inferred from File Structure / `_Boundary:_` / task bodies do not overlap
- **Marker**: the major itself is marked `(P)`, **or** every remaining executable child under the major is marked `(P)`

**Parallel dispatch**: When multiple incomplete majors are dependency-ready, `(P)`-eligible, and pass the conditions above, the parent may dispatch those **major batches** concurrently (one implementer per major). If any condition fails or collision risk is unclear → fall back to **serial** (lowest ready major first). Never unconditional parallel that ignores path/Depends conflicts. Never split a major into parallel implementers just because its children have `(P)`.

**Merge / commit under parallel**: Parent still owns commits. Prefer selective staging per completed major, committing in major-number order or completion order. On git conflict or overlapping staged paths → **stop and escalate to human**; do not force-merge or `git add -A`.

**Iteration discipline**: Default is ONE major per iteration (form major batch → one implementer → parent mechanical checks → (pass) one batch reviewer → commit → re-read tasks.md → next major). When the `(P)` execution contract allows parallel majors, one iteration may run multiple such major cycles concurrently; each major still gets its own implementer → mechanical → reviewer → selective commit. After any parallel set finishes (or aborts on conflict), re-read `tasks.md` before forming the next set.

**Context management** (parent vs implementer — keep these distinct):
- **Parent controller**: At the start of each iteration, re-read `tasks.md` and determine the next **major** batch (or parallel-ready `(P)` major set) as ordered task-ID list(s) under that major. Do NOT rely on accumulated memory of previous iterations for batch selection. After completing each iteration, the parent may discard verbose status/reviewer reports and retain only a one-line summary per major (e.g., "major 1 [1.1, 1.2, 1.3]: APPROVED, 5 files changed").
- **Sticky implementer**: On the success path, the implementer may retain major-to-major implementation knowledge **within that major lineage**. Parent discarding verbose reports does **not** require discarding sticky implementer context, and does **not** require an unconditional fresh implementer each major. Concurrent `(P)` majors use separate implementer lineages.

### Agent continuity (sticky vs fresh)

| Situation | Agent | Reason |
|-----------|-------|--------|
| Batch success continuation (previous APPROVED → next batch) | **sticky**: resume the same implementer, OR parent re-dispatches with previous-batch one-line summary + Implementation Notes + next-major excerpt only | Reduce cold start on happy path |
| In-batch REMEDIATION (reviewer REJECTED rounds 1–2) | Prefer **same implementer** with feedback re-sent (fresh not required) | Keep fix context |
| BLOCKED / unresolved NEEDS_CONTEXT / REJECTED → debug | **Always fresh** debugger | Cut pollution (unchanged) |
| RETRY after debug | **Always fresh** implementer (unchanged) | Cut pollution |
| Reviewer | Independent per batch (judgment-only after parent mechanical). Sticky not required | Independence |

**Sticky fallback** (hosts that cannot resume / continue the same agent): parent must still pass a **pseudo-sticky** payload — never a bare empty fresh start when continuing after APPROVED:
- File paths changed in the previous major
- Related rows from `## Implementation Notes`
- Next major task texts and `_Boundary:_` scope
- Spec excerpts for the next major (see Pre-dispatch Spec Excerpts below)

### Pre-dispatch Spec Excerpts (parent, before each major dispatch)

Do **not** rely on subagents reading `requirements.md` / `design.md` in full. Feature acceptance criteria and feature-local design live in those files; **authoritative long-lived public-surface contracts live in `docs/contracts/**`** (boundaries/ADR in `docs/architecture/**`). The parent cuts batch-needed fragments from the correct authorities and embeds them in the prompt.

Before each implementer / reviewer / debugger dispatch for a batch, the parent:

1. Aggregate `_Requirements:` IDs from every task in the batch.
2. From `requirements.md`, extract only the referenced acceptance criteria (e.g., the cited conditions under requirement 3 among 1–7 — not the whole file).
3. From optional `_Design: D-Name_` annotations and/or `_Boundary:_` / task bodies, extract the matching design component section(s), plus the minimal cross-cutting slices needed for this batch (dependency-direction block, Error mapping for that boundary, File Structure lines for relevant paths). **Prefer stable anchors**: resolve `_Design:` or Boundary name to `{#D-...}` / `D-...` headings first (grep/search); if no hit, fall back to heading-text partial match; if still unresolved → treat as **NEEDS_CONTEXT**. Do **not** include Overview, Non-Goals, or unrelated component sections.
4. **Related contracts excerpt** (persistent): Prefer task `_Contracts: docs/contracts/<file>.md_` paths (optional annotation; not required); else Persistent References (`Mode: modify` or Boundary-related `reference`) for surfaces this batch touches. Embed under `### Contracts (authoritative for touched surfaces)`. Architecture/ADR only when the batch changes boundaries or dependency direction (minimal block).
5. **Line-budget / split trigger**: Prefer keeping the **whole major** together. If feature Spec Excerpts would exceed roughly **250–400 lines**, or added persistent excerpts exceed roughly **80–150 lines**, **split within that major** (sequential sub-batches of the same major) — do not paste full specs or bulk-read contract trees to stay under budget, and do not mix another major in to fill budget.
6. Embed excerpts in a fixed prompt section (authoritative for the batch):

```
## Spec Excerpts (authoritative for this batch)
### Requirements
<extracted acceptance criteria with source section numbers>
### Design
<extracted component + minimal cross-cutting slices with source section numbers>
### Contracts (authoritative for touched surfaces)
<related contract excerpts or short full files, with paths>
```

**Authority**: Spec Excerpts are the authoritative input for implementer / reviewer / debugger. Spec file paths may be included as repository location only; do **not** instruct subagents to open and read those files as the primary workflow.

**Contract drift (ズレ) — parent must enforce via templates**:

"Drifted" means any of: (1) implementation contradicts provided Contracts excerpt I/O / invariants / forbidden dependencies; (2) new public surface / event / data ownership without matching `docs/contracts/` update; (3) executable contract (types / OpenAPI / contract tests) still red while claiming complete. Architecture full Read is **not** required to judge drift when excerpts + related contracts + executable contracts are present.

When the implementer detects drift, it must update in place: align code to contract **or** intentionally update the contract (decide from Task Brief / Requirements excerpts); update `docs/contracts/<file>` (breaking → ADR per append-only rules; record in contract Changelog when applicable); if creating a **new** contract or ADR file, register it in `docs/contracts/README.md` or `docs/architecture/adr/README.md` Entries; append path to `design.md` Persistent References if the feature still exists and the path is missing; report `CONTRACTS_UPDATED: <paths>` in the Status Report (include index README paths when Entries changed). Do not rewrite unrelated contracts. Do not invent a post-implementation "write all contracts" phase.

**Subagent load limits**: Default — do **not** glob-bulk-Read `docs/architecture/**`. On excerpt gaps, subagents must `NEEDS_CONTEXT` / `MISSING` with the missing **path or heading** (not self-load full files).

**NEEDS_CONTEXT re-excerpt**: If a subagent returns `NEEDS_CONTEXT` / `MISSING` naming a missing heading or path, the parent may extract that heading/path and re-dispatch **once** with additional excerpts (same one-shot re-dispatch rule). Do not tell the subagent to load the full file.

Do **not** introduce `design/` multi-file splits or inject steering wholesale — only short task-relevant steering snippets already allowed elsewhere.

**Parent is controller, not implementer.** When execution mode is `wave` or `strict`, the parent MUST dispatch implementer and reviewer as separate agents for each batch. Do not implement, edit production code, or write tests in the parent context. Do not skip dispatch because the parent could finish the work, the batch looks small, or a separate agent would use a similar model. "The parent can implement this" is not the same as multi-agent being unavailable.

For each batch:

**a) Dispatch implementer** (one implementer per **major** batch):
- Read `templates/implementer-prompt.md` from this skill's directory
- Run **Pre-dispatch Spec Excerpts** for this batch, then construct a prompt by combining the template with batch-specific context:
  - Ordered list of task IDs/descriptions and shared boundary scope
  - `## Spec Excerpts (authoritative for this batch)` with `### Requirements`, `### Design`, and when related `### Contracts (authoritative for touched surfaces)` (Requirements/Design always; Contracts when related)
  - Spec file paths as repository location only (not a directive to read them in full)
  - Exact requirement and design section numbers each task in the batch must satisfy (using source numbering, NOT invented `REQ-*` aliases) — these must match what appears in the excerpts
  - Task-relevant steering context (short) and parent-discovered validation commands (tests/build/smoke as relevant)
  - Whether each task is behavioral or non-behavioral
  - Per task (or batch): `FEATURE_FLAG: required | skipped` — parent judges using the Feature Flag Protocol apply conditions below; if `tasks.md` or design excerpts require a flag → `required`
  - **Previous learnings**: Include any `## Implementation Notes` entries from tasks.md that are relevant to this batch's boundary or dependencies (e.g., "better-sqlite3 requires separate rebuild for Electron"). This prevents the same mistakes from recurring.
- The implementer builds Task Briefs from the **Spec Excerpts** (acceptance criteria, completion definition, design constraints, verification method) for the batch tasks, then implements them in order with TDD. Default: do **not** Read `requirements.md` / `design.md` in full.
- **Continuity**:
  - First batch of the run, or any dispatch after debug RETRY → spawn a **fresh** implementer
  - After a previous batch APPROVED → **sticky**: resume the same implementer when the host supports it; otherwise re-dispatch with the sticky-fallback payload above (pseudo-sticky). Do **not** require an unconditional empty fresh implementer on the success path

**b) Handle implementer status**:
- Parse implementer status only from the exact `## Status Report` block and `- STATUS:` field.
- If `STATUS` is missing, ambiguous, or replaced with prose, re-dispatch the implementer once requesting the exact structured status block only (prefer same implementer / sticky when available). Do NOT proceed to review without a parseable `READY_FOR_REVIEW | BLOCKED | NEEDS_CONTEXT` value.
- When present, note `CONTRACTS_UPDATED: <paths>` for the reviewer (intentional contract updates in this batch); stage those paths with the batch commit if APPROVED.
- **READY_FOR_REVIEW** → proceed to **parent mechanical checks** (step c), then reviewer only if those pass
- **BLOCKED** → dispatch debug subagent (see section below); do NOT immediately skip
- **NEEDS_CONTEXT** → extract the named missing path(s)/heading(s) from `MISSING`, append those excerpts once, and re-dispatch (prefer same implementer); if still unresolved → dispatch debug subagent

**c) Parent mechanical checks** (batch end, **before** reviewer dispatch):

After the implementer returns `READY_FOR_REVIEW`, the parent controller runs these checks itself and records `MECHANICAL_RESULTS`. Do **not** dispatch the reviewer until all pass.

1. **Regression**: Run the parent-held `TEST_COMMANDS` (task-relevant subset is allowed). Record each command, exit code, and a short pass/fail summary.
2. **TBD/TODO/FIXME**: Grep changed files for `TBD|TODO|FIXME|HACK|XXX` (same intent as the reviewer template). Record CLEAN or match count / samples.
3. **Secrets**: Grep changed files for hardcoded secret patterns (e.g. `password=`, `api_key=`, `secret=`, `token=`, case-insensitive). Record CLEAN or match count / samples.
4. **Boundary**: Run `git diff --name-only` and compare against every `_Boundary:_` in the batch. Record WITHIN or files outside boundary.
5. **RED phase**: For behavioral tasks in the batch, confirm the implementer Status Report includes non-empty `RED_PHASE_OUTPUT` (required whether `FEATURE_FLAG` is `required` or `skipped`). Record VERIFIED | MISSING | N/A.

**If any mechanical check FAILs**:
- Do **not** call the reviewer.
- Treat as **REJECTED-equivalent remediation**: send the `MECHANICAL_RESULTS` (and concrete failures) back to the implementer (prefer same / sticky implementer).
- Count this remediations round toward the same **Bounded Review Rounds** limit as reviewer `REJECTED` (max 2 implementer re-dispatch rounds, then debug on the next failure).

**If all mechanical checks PASS** → proceed to step d with the recorded `MECHANICAL_RESULTS`.

**d) Dispatch reviewer** (one judgment review per batch):

- Call the reviewer **once for the whole batch** (not once per task).
- Read `templates/reviewer-prompt.md` from this skill's directory
- Construct a review prompt with:
  - Parent `MECHANICAL_RESULTS` (commands, exit codes, grep summaries) — the reviewer may **trust these as the mechanical baseline and must not re-run the full suite by default**; re-run only if results are missing or look suspicious
  - The batch task list (ordered IDs) and relevant spec section numbers
  - The same `## Spec Excerpts (authoritative for this batch)` used for the implementer (Requirements + Design + Contracts when present); paths may appear as location only
  - The implementer's status report (for reference only — reviewer must verify judgment items independently)
  - Batch `_Boundary:_` scope and task-relevant validation command names (for context; mechanical execution already done by parent)
  - Per-task `FEATURE_FLAG: required | skipped` (when `skipped`, missing flag-only steps is not a REJECT reason; `RED_PHASE_OUTPUT` for behavioral tasks still required)
- The reviewer must apply the `sdd-review` protocol for **batch-local judgment review** (spec alignment, test quality, implementation reality), with mechanical checks satisfied via parent `MECHANICAL_RESULTS` unless re-run is warranted. Default: compare judgment against **Spec Excerpts**, not a full Read of `requirements.md` / `design.md`.
- The reviewer sub-agent runs `git diff` itself as the primary source of truth for judgment.
- Spawn a reviewer sub-agent with this prompt (independent per batch; sticky not required)

**e) Handle reviewer verdict**:
- Parse reviewer verdict only from the exact `## Review Verdict` block and `- VERDICT:` field.
- If `VERDICT` is missing, ambiguous, or replaced with prose, re-dispatch the reviewer once requesting the exact structured verdict only. Do NOT mark batch tasks complete, commit, or continue to the next batch without a parseable `APPROVED | REJECTED` value.
- **APPROVED** → proceed to **batch completion gate** (step e2). Do **not** call `sdd-verify-completion` on APPROVED alone, and do **not** mark any batch task `[x]` yet.
- **REJECTED (round 1-2)** → re-send review feedback to the **same** implementer when possible (sticky remediation; fresh not required); otherwise re-dispatch with feedback plus the sticky-fallback payload. After the implementer returns `READY_FOR_REVIEW` again, re-run **parent mechanical checks** before the next reviewer call. Do **not** require a full `sdd-verify-completion` on each remediation — parent mechanical re-run is enough until the batch completion gate.
- **REJECTED (round 3)** → dispatch debug subagent (see section below)

**e2) Batch completion gate** (once per batch, immediately before marking all tasks `[x]`):

Apply `sdd-verify-completion` **exactly once** for the whole batch (claim type: `BATCH` / multi-`TASK`). Do **not** call it per sub-task checkbox, and do **not** treat implementer or reviewer prose as sufficient evidence by itself.

Evidence for this gate:
- Parent `MECHANICAL_RESULTS` already recorded for this batch (test commands, exit codes, TBD/Secrets/Boundary/RED summaries)
- Reviewer `APPROVED` from the exact `## Review Verdict` block

**If `VERIFIED`** → mark **all** tasks in the batch `[x]` in tasks.md, then perform one selective git commit for the batch (step f).

**If `NOT_VERIFIED` or `MANUAL_VERIFY_REQUIRED`** → do **not** treat the batch as complete: do not mark any batch task `[x]`, do not commit as success, and either remediate (send gaps back to the implementer; counts toward Bounded Review Rounds) or BLOCK affected task(s) if the gate cannot be satisfied.

**f) Commit** (parent-only, selective staging; one commit per verified batch):
- Stage only the files actually changed for this batch, plus tasks.md
- **NEVER** use `git add -A` or `git add .`
- Use `git add <file1> <file2> ...` with explicit file paths
- Commit message format: `feat(<feature-name>): <major N summary spanning task IDs>`

**g) Record learnings**:
- If this batch revealed cross-cutting insights, append a one-line note to the `## Implementation Notes` section at the bottom of tasks.md

**h) Debug subagent** (triggered by BLOCKED, NEEDS_CONTEXT unresolved, mechanical FAIL after review-round budget exhausted, or REJECTED after 2 remediation rounds):

The debug subagent runs in a **fresh context** — it receives only the error information, not the failed implementation history. This avoids the context pollution that causes infinite retry loops.

- Read `templates/debugger-prompt.md` from this skill's directory
- Construct a debug prompt with:
  - The error description / blocker reason / reviewer rejection findings
  - `git diff` of the current uncommitted changes
  - The batch task descriptions and relevant spec section numbers
  - `## Spec Excerpts (authoritative for this batch)` (same batch excerpts; plus any one-shot additional excerpts if NEEDS_CONTEXT named missing headings)
  - Spec paths as repository location only; optional short extra context slices only when needed for the failure (still not full-file dumps)
- The debugger must apply the `sdd-debug` protocol to this failure investigation.
- Preserve rich failure context: error output, reviewer findings, current `git diff`, task/spec refs, and any relevant Implementation Notes.
- When available, the debugger should inspect runtime/config state and use web or official documentation research to validate root-cause hypotheses before proposing a fix plan.
- Spawn a fresh sub-agent with this prompt

**Handle debug report**:
- Parse `NEXT_ACTION` from the debug report's exact structured field.
- If `NEXT_ACTION: STOP_FOR_HUMAN` → append `_Blocked: <ROOT_CAUSE>_` to tasks.md, stop the feature run, and report that human review is required before continuing
- If `NEXT_ACTION: BLOCK_TASK` → append `_Blocked: <ROOT_CAUSE>_` to tasks.md, skip to next batch
- If `NEXT_ACTION: RETRY_TASK` → preserve the current worktree; do NOT reset or discard unrelated changes. Spawn a **new** (fresh) implementer sub-agent with the debug report's `FIX_PLAN`, `NOTES`, and the current `git diff`, and require it to repair the batch with explicit edits only
  - If the new implementer succeeds (READY_FOR_REVIEW → parent mechanical PASS → reviewer APPROVED) → normal flow
  - If the new implementer also fails → repeat debug cycle (max 2 debug rounds total). After 2 failed debug rounds → append `_Blocked: debug attempted twice, still failing — <ROOT_CAUSE>_` to tasks.md, skip
- **Max 2 debug rounds per batch**. Each round: fresh debug subagent → fresh implementer. If still failing after 2 rounds, the affected task(s) are blocked.
- Record debug findings in `## Implementation Notes` (this helps subsequent batches avoid the same issue)

**`(P)` markers**: See **`(P)` execution contract** under Batch definition rules. Parallel is **major-to-major**, not `N.M`-to-`N.M`. Hosts that cannot run concurrent sub-agents still honor the contract by processing ready `(P)` majors **serially** (never inventing informational-only semantics).

**Fallback**: Fall back to `direct` (Manual Mode) only when the host cannot spawn a separate agent at all. If the host can spawn isolated agents, `wave` / `strict` remain dispatch modes. Do not fall back because the parent could implement the batch, the work looks small, or the parent already uses a capable coding model.

### Manual Mode / `direct` (main context)

Used for **manual invocation**, execution mode **`direct`** (tier S or ≤3 tasks), and the host-cannot-spawn fallback above. Execute the selected / pending queue with the same TDD + mechanical + judgment loop. Do **not** dispatch a fresh implementer and reviewer per sub-task. `sdd-verify-completion` is **deferred** to selection completion — not required after every remediation, and not required merely because a task was `APPROVED`.

Reviewer cadence under `direct`: run judgment review **once** after all tasks in the selection/queue are implemented (or once if only one task) — not once per sub-task. Parent mechanical checks still run before that reviewer call.

**Spec scoping under `direct`**: The parent *is* the implementer, so there is no subagent prompt — but apply the **same excerpt discipline** as Pre-dispatch Spec Excerpts: for each task (or for the whole selection before coding), extract only the referenced `_Requirements:` criteria, matching design sections (prefer `{#D-...}` / `_Design:` / Boundary anchors), and related `### Contracts (authoritative for touched surfaces)` via `_Contracts:` or Persistent References. Keep the working brief roughly within the **250–400 line** budget; if larger, implement in smaller slices. Do **not** load entire `requirements.md` / `design.md` / architecture as the default. Apply the same Contract Drift definition and in-place update (including `CONTRACTS_UPDATED` in the status/notes when contracts change). If a subagent reviewer is used at selection end, pass those excerpts as `## Spec Excerpts (authoritative for this batch)`.

For each selected task:

**1. Build Task Brief**:
Before writing any code, from the scoped excerpts (not full-file dumps) clarify:
- What observable behaviors must be true when done (acceptance criteria)
- What files/functions/tests must exist (completion definition)
- What technical decisions to follow from design excerpts (design constraints)
- How to confirm the task works (verification method)

**2. Execute TDD cycle** (Kent Beck's RED → GREEN → REFACTOR):
- **RED**: Write test for the next small piece of functionality based on the acceptance criteria. Test should fail.
- **GREEN**: Implement simplest solution to make test pass, following the design constraints.
- **REFACTOR**: Improve code structure, remove duplication. All tests must still pass.
- **VERIFY**: All tests pass (new and existing), no regressions. Confirm verification method passes.
- **Per-task mechanical (lightweight)**: Optionally re-run the task-relevant subset of `TEST_COMMANDS` after each task; remediate in-context on failure. Do **not** spawn a fresh implementer or reviewer per sub-task under `direct`.
- **Defer mark**: Do **not** mark `[x]` yet if more selected tasks remain; continue to the next selected task.

**2b. Selection / feature-end review** (once after all selected/queued tasks are implemented — not once per sub-task):
- Run the full **parent mechanical checks** (Regression / TBD / Secrets / Boundary / RED) across the selection's changes. If any FAIL, remediate in-context (count toward the review-round budget) and do not proceed to judgment.
- When mechanical checks PASS, apply `sdd-review` judgment review **once** for the whole selection: if the host supports fresh subagents, use a fresh reviewer with `MECHANICAL_RESULTS` and the selection's Spec Excerpts; otherwise perform judgment in the main context using the `sdd-review` protocol against the same scoped excerpts. Do NOT proceed to the completion gate until the verdict is parseably `APPROVED`. Do **not** call `sdd-verify-completion` here.
- Keep recorded `MECHANICAL_RESULTS` + `APPROVED` for the completion gate.

**3. Selection completion gate** (after selection/feature-end review is `APPROVED`):
- **One selected task**: apply `sdd-verify-completion` once at that task's end (claim type: `TASK`), then mark it `[x]`.
- **Multiple selected tasks**: apply `sdd-verify-completion` **once** for the whole selection (claim type: `BATCH` / multi-`TASK`) using parent mechanical evidence + reviewer `APPROVED` for those tasks; then mark all selected tasks `[x]`.
- **If `NOT_VERIFIED` or `MANUAL_VERIFY_REQUIRED`**: do not mark any remaining incomplete selected tasks `[x]`; remediate or stop with gaps.

## Step 4: Final Validation

**Autonomous invocation** (`wave`, `strict`, or autonomous `direct` completing the full pending queue):
- After all tasks complete, run `/sdd-validate-impl <feature>` (resolved name) as a GO/NO-GO gate — **required for every execution mode**, including tier S / `direct` (do not skip quality gates for S)
- If validation returns GO → before reporting feature success, apply `sdd-verify-completion` to the feature-level claim using the validation result and fresh supporting evidence
- If validation returns NO-GO:
  - Fix only concrete findings from the validation report
  - Cap remediation at 3 rounds; if still NO-GO, stop and report remaining findings
- If validation returns MANUAL_VERIFY_REQUIRED → stop and report the missing verification step

**Manual invocation** (task-number selection only):
- Suggest running `/sdd-validate-impl <feature>` (resolved name) but do not auto-execute

## Feature Flag Protocol

Feature Flag Protocol is **opt-in**. The parent passes `FEATURE_FLAG: required | skipped` per batch / task.

**Required** only when **all** of the following hold (or an explicitly documented equivalent):

- The change is **brownfield**: it alters behavior on an existing production / user-facing path
- Without a flag, an intermediate state would be exposed to users, **or** the design requires a flag as the rollback unit

If `tasks.md` or design excerpts require a flag → `required`.

**When `FEATURE_FLAG: required`**, enforce RED → GREEN with a feature flag:

1. **Add flag** (OFF by default): Introduce a toggle appropriate to the codebase (env var, config constant, boolean, conditional)
2. **RED -- flag OFF**: Write tests for the new behavior. Run tests → must FAIL. If tests pass with flag OFF, the tests are not testing the right thing. Rewrite.
3. **GREEN -- flag ON + implement**: Enable the flag, write implementation. Run tests → must PASS.
4. **Remove flag**: Make the code unconditional. Run tests → must still PASS.

**When `FEATURE_FLAG: skipped`**, use a standard RED → GREEN → REFACTOR cycle (no flag add/toggle/remove). Typical skip cases:

- greenfield (new package / new binary / unpublished feature)
- pure refactoring, configuration, or documentation
- internal module additions where design does not require a flag
- tasks with no behavioral change

**RED evidence**: For behavioral tasks, keep `RED_PHASE_OUTPUT` (failing tests before implementation) whether the flag is `required` or `skipped`. Do not REJECT solely for missing flag-only steps when `skipped`.

</instructions>

## Critical Constraints
- **Execution Mode Selection**: Choose `direct` / `wave` / `strict` per Step 2 (user override → manual → `complexity_tier` → task-count fallback). Report the chosen mode and reason in one line at run start. Autonomous `wave` / `strict` dispatch by **major number**, not per `N.M`. Tier S / `direct` must not default to per-task fresh implementer×reviewer.
- **Dispatch Required (`wave` / `strict`)**: Parent is controller only. MUST spawn implementer and reviewer as separate agents per major batch. Do not implement the batch in the parent context. Fall back to `direct` only when the host cannot spawn a separate agent — not because the parent could do the work faster.
- **Strict Handoff Parsing**: Never infer implementer `STATUS` or reviewer `VERDICT` from surrounding prose; only the exact structured fields count
- **Parent Spec Excerpts**: Before each major dispatch, inject `## Spec Excerpts (authoritative for this batch)` with `### Requirements`, `### Design`, and when related `### Contracts (authoritative for touched surfaces)`; if combined excerpts exceed ~250–400 lines, split **within** the major. Under `direct`, apply the same excerpt discipline in-context (do not full-file dump). Do not make full-file Read of requirements/design/architecture the default for subagents. Do not split design into multi-file layouts under this skill.
- **Contract Drift**: Detect from Contracts excerpts + related contracts + executable contracts (not architecture full Read). On drift: align code or update contracts intentionally; register new contract/ADR paths in the corresponding README Entries; report `CONTRACTS_UPDATED: <paths>` in the implementer Status Report; stage those paths with the batch commit. Do not rewrite unrelated contracts.
- **`(P)` Execution Contract**: `(P)` authorizes conditional parallel **major** dispatch when boundaries, Depends, and paths are disjoint; otherwise serial. Never treat `(P)` as informational-only. Never split a major into parallel implementers for its `(P)` children. On merge conflict under parallel → stop for human. Applies to `wave` / `strict` only.
- **Sticky on Happy Path**: Under `wave` / `strict`, after APPROVED prefer sticky / resume (or pseudo-sticky fallback) for the next **major**; do not treat unconditional fresh implementer as required on the success path. Under parallel majors, sticky applies per-major lineage, not across concurrent majors
- **Fresh on Failure / Debug**: Under `wave` / `strict` (including `strict` / tier L): debugger is always fresh; implementer after debug RETRY is always fresh; max 2 debug rounds unchanged
- **Deferred Verify-Completion**: Call `sdd-verify-completion` once per batch (or once per `direct`/manual selection) immediately before marking tasks `[x]`, plus once at feature end for `FEATURE_GO`. Do not require it after every APPROVED, every sub-task checkbox, or every remediation. Subagent self-reports are never sufficient evidence alone.
- **Terminal Validate Retained**: Autonomous completion always runs Step 4 `/sdd-validate-impl` + `FEATURE_GO` for `direct`, `wave`, and `strict` alike — never drop end gates for tier S
- **No Destructive Reset**: Never use `git checkout .`, `git reset --hard`, or similar destructive rollback inside the implementation loop
- **Selective Staging**: NEVER use `git add -A` or `git add .`; always stage explicit file paths
- **Bounded Review Rounds**: Max 2 implementer re-dispatch rounds per mechanical FAIL or reviewer REJECTED, then debug
- **Two-Tier Review**: Parent runs mechanical checks before reviewer; on mechanical FAIL skip reviewer and remediate; reviewer focuses on judgment using parent `MECHANICAL_RESULTS`
- **Bounded Debug**: Max 2 debug rounds per batch (debug + re-implementation per round); if still failing → BLOCKED
- **Bounded Remediation**: Cap final-validation remediation at 3 rounds

## Output Description

**Start of run**: One line — selected execution mode and reason (see Execution Mode Selection).

**`wave` / `strict`**: For each **major** (including each major in a parallel `(P)` set), report: major number and task ID list, implementer status, parent mechanical summary, reviewer verdict, batch verify-completion status, files changed, commit hash. After all majors: final validation result and feature `FEATURE_GO` verification.

**`direct` / manual**: Tasks executed with test results; selection verify-completion status. Status of completed/remaining tasks. Autonomous `direct` also reports Step 4 validation + `FEATURE_GO`.

**Format**: Concise, in the language specified in spec.json.

## Safety & Fallback

### Error Scenarios

**Cannot Resolve Feature**:
- **Stop Execution**: No explicit `<feature>`, or `docs/specs/<feature>/spec.json` is missing
- **Suggested Action**: Pass the spec name (`/sdd-impl <feature>`). Do not infer from chat, git branch, or scan `docs/specs/`

**Not Ready for Implementation or Missing Spec Files**:
- **Stop Execution**: All spec files must exist and `ready_for_implementation` must be true
- **Suggested Action**: "Complete previous phases: `/sdd-spec-requirements`, `/sdd-spec-design`, `/sdd-spec-tasks`"

**Test Failures**:
- **Stop Implementation**: Fix failing tests before continuing
- **Action**: Debug and fix, then re-run

**All Tasks Blocked**:
- Stop and report all blocked tasks with reasons; human review needed

**Spec Conflicts with Reality**:
- Block the task with `_Blocked: <reason>_` -- do not silently work around it

**Upstream Ownership Detected**:
- If review, debug, or validation shows that the root cause belongs to an upstream, foundation, shared-platform, or dependency spec, do not patch around it inside the downstream feature
- Route the fix back to the owning upstream spec, keep the downstream task blocked until that contract is repaired, and re-run validation/smoke for dependent specs after the upstream fix lands

**Task Plan Invalidated During Implementation**:
- If debug returns `NEXT_ACTION: STOP_FOR_HUMAN` because of task ordering, boundary, or decomposition problems, stop and return for human review of `tasks.md` or the approved plan instead of forcing a code workaround

**Session Interrupted**:
- Safe to re-run `/sdd-impl <feature>` — completed tasks are already `[x]` in tasks.md and committed to git
- The controller re-reads tasks.md on each iteration, so it will pick up where it left off automatically
