# Flow Routing

## Resolve Target Feature

Resolve `<feature>` **before** routing. Do not guess from chat history. Do not use the current git branch.

1. **`<feature>` is required** — first argument that is not a flow/tier override (`実装のみ`, `実装だけ`, `要求だけ更新`, `要求更新`, `設計だけ`, `設計のみ`, `設計更新`, `quick`, `lite`, `フル`, `full`).
2. **Stop and ask for a spec name** if no such argument is present.

Then continue with § Entry Contract using the resolved `<feature>`.

## Entry Contract (discovery runs standalone)

`/sdd-discovery` is **not** an orchestration step. It is run **standalone before** orchestration and has already produced `brief.md` (new specs), `roadmap.md` (when dependencies exist), and — for existing specs — `spec.json`. Orchestration is invoked with a required target `<feature>` plus optional explicit flow, and selects the active flow without a discovery Path signal:

1. **User-specified flow wins** — e.g.「要求だけ更新」「設計だけ」「実装だけ」.
2. **Else derive from `spec.json`** via § Spec State Hints:
   - `brief.md` exists, no `spec.json` → **要求新規作成** (start at `/sdd-spec-requirements`)
   - `approvals.requirements.generated` false → resume requirements / **要求更新**
   - requirements generated, `approvals.design.generated` false → **設計更新**
   - design generated, `approvals.tasks.generated` false → resume task generation (`/sdd-spec-tasks` … Terminal auto-approve)
   - `ready_for_implementation: true` → **実装のみ** (explicit request only)
3. **Neither `brief.md` nor `spec.json` exists** for the target → **stop**; instruct the user to run `/sdd-discovery` first. Do **not** auto-run discovery.

**Resume (new session):** A new chat on the **same Git checkout** with `/sdd-orchestrate <feature>` means the human accepted the previous phase artifacts. Start from the next incomplete phase based on `approvals.*.generated` and `ready_for_implementation` (same table as § Spec State Hints). Same-chat correction notes after Phase Handoff are not resume. Do not create a new worktree per phase.

Path B (no spec) is decided by discovery **before** orchestration and never enters an orchestration flow (see `flows.md` § Path B).

## Complexity Tier (orchestrator inline)

After resolving the active flow, before the first skill dispatch:

1. Read `rules/complexity-tier.md`
2. Compute tier from `brief.md` (+ roadmap if present)
3. Write `complexity_tier` / `complexity_score` / `complexity_rationale` to `spec.json`
4. Map tier → orchestration path (`flows.md` § Orchestration Paths by Tier), then load the matching flow variant (S / M / L suffix)

| Tier | Path | Flow section |
| ---- | ---- | ------------ |
| S | **quick-path** | `要求新規作成 (S)` → `/sdd-spec-quick --auto --from-orchestrate` |
| M | **standard-path** | `要求新規作成 (M)` → unified validates; 要求 / 設計 each → Phase terminal; タスクは再開後 |
| L | **full-path** | `要求新規作成 (L)` → full pipeline; 要求 / 設計 each → Phase terminal; タスクは再開後 |

User override: explicit「フル」「lite」「quick」でティア／経路を上書き可。
- 「quick」「lite」→ force S / **quick-path** (regardless of score)
- 「フル」「full」→ force L / **full-path**

**Do not** change `実装のみ` by tier. Existing specs without `complexity_tier` → treat as **L** for **orchestration path selection** only (not for `/sdd-impl` execution mode — that uses task-count fallback; see `complexity-tier.md` § Scope note). Path D/E → force **L** (never S / never quick-path).

## Determine Active Flow

Combine **`spec.json` state** (§ Spec State Hints) and **user override** to pick one flow:

| Condition | Flow |
| --------- | ---- |
| New spec / new requirements (`brief.md`, no `spec.json`) | 要求新規作成 |
| Existing spec, requirements change | 要求更新 |
| Requirements generated, design-only change | 設計更新 |
| No spec change, implementation only | 実装のみ |
| Path B (no spec, decided by discovery) | 直接実装 (outside orchestration) |

**User override wins** — e.g.「要求だけ更新」「実装だけ」.

## Modification Guard (implementation must be complete first)

**The orchestrator must not modify a spec whose implementation is not yet complete.** Before entering any flow that changes an existing spec (要求更新 / 設計更新, or a Path A extension into an existing spec), gate on that spec's implementation state.

Read `docs/specs/<feature>/spec.json` + `tasks.md`:

| State | Meaning | Orchestrator action |
| ----- | ------- | ------------------- |
| `ready_for_implementation: true` **and** `tasks.md` has any `[ ]` or `_Blocked:_` | implementation-ready but **not** complete | **Stop. Do not modify.** Prompt user to finish implementation first (explicit `実装のみ`). |
| `ready_for_implementation: true` **and** all `tasks.md` tasks `[x]`, no `_Blocked:_` | implementation complete | Modification allowed — proceed with 要求更新 / 設計更新 |
| `ready_for_implementation: false` | still in first-pass authoring (requirements/design/tasks not finished) | Not a modification of implemented-ready work — resume the initial flow normally |

- "Implementation complete" = `tasks.md` exists, every task `[x]`, none `_Blocked:_` (confirm via `/sdd-spec-status <feature>`; a prior `/sdd-validate-impl` GO is stronger evidence).
- On a blocked modification, report: which spec, its outstanding `[ ]` / `_Blocked:_` tasks, and instruct: complete implementation via an explicit `実装のみ` run, then re-request the change.
- **User override does not bypass this guard** unless the user explicitly acknowledges the incomplete implementation and insists on modifying anyway.
- Path B (直接実装) is unaffected — it has no spec.

## Upstream Dependency Guard (roadmap.md)

**The orchestrator must not start spec authoring for a downstream feature while its roadmap upstream dependencies have not finished task generation.** Before entering 要求新規作成 / 要求更新 / 設計更新 (and before each spec in Path D/E), gate on upstream readiness.

### When to run

| Flow | Timing |
| ---- | ------ |
| 要求新規作成 | At flow entry (target feature already identified from the invocation / `brief.md`), before first generation dispatch (`/sdd-spec-quick` on S, `/sdd-spec-requirements` on M/L) |
| 要求更新 / 設計更新 | After Modification Guard passes, before any generation or validate dispatch |
| Path D/E Multi-Spec | Before starting each spec's applicable flow (in roadmap dependency order) |

**Not applied** to 実装のみ, Path B, or features with no upstream dependencies.

### Parse dependencies

Read `docs/steering/roadmap.md` when it exists:

- `## Specs (dependency order)`
- `## Existing Spec Updates` (if present)

Find the line for the target feature; parse the `Dependencies:` field (comma-separated spec names; `none` = no deps).

| Condition | Action |
| --------- | ------ |
| `roadmap.md` missing | Pass (single-spec / no roadmap context) |
| Feature not listed in either section | Pass |
| `Dependencies: none` (or empty) | Pass |

### Upstream readiness (each listed dependency)

A dependency `<dep>` is **ready** if **any** of:

1. Its roadmap line is marked `[x]` (authoring completed for that spec), **or**
2. `docs/specs/<dep>/spec.json` has `approvals.tasks.generated === true` **and** `docs/specs/<dep>/tasks.md` exists with at least one task entry.

Otherwise **not ready** — including when `docs/specs/<dep>/` is missing or the upstream is still in requirements/design phase.

This matches `/sdd-verify-phase-gate <dep> tasks` generation criteria (upstream must complete before downstream starts).

### On block

**Stop.** Do not dispatch `/sdd-spec-quick`, `/sdd-spec-requirements`, `/sdd-spec-design`, `/sdd-spec-tasks`, or phase validates for the downstream feature.

Report to the user:

- Target feature and which upstream dep(s) are not ready
- Each blocking dep's status (`phase`, `approvals` from `spec.json`; optional `/sdd-spec-status <dep>`)
- Instruction: complete upstream through `/sdd-orchestrate <dep>` **in a checkout that will hold those artifacts**, then retry **this** spec in a checkout that already contains the upstream `spec.json` + `tasks.md` (typically: merge the upstream PR to the integration branch, then recreate or rebase this worktree onto that tip).
- **Do not** treat another worktree as visible. This guard reads only the current checkout. "Upstream finished next door" is not readiness.
- Independent specs (`Dependencies: none`) may run in parallel checkouts. A downstream spec must not start until every listed upstream is ready **here**.

**User override does not bypass this guard** unless the user explicitly acknowledges incomplete upstream specs and insists on proceeding anyway.

## Path → Flow

Reference mapping from the Path that `/sdd-discovery` determined **standalone** to the flow orchestration should be invoked with (Path itself is not re-derived inside orchestration — use § Entry Contract):

| Path | Route |
| ---- | ----- |
| **A** (existing spec sufficient) | 要求更新 \| 設計更新 \| 実装のみ — pick from user intent + `spec.json` approvals |
| **B** (no spec) | **直接実装** — never enter spec flow (handled outside orchestration) |
| **C** (new single spec) | 要求新規作成 |
| **D/E** (multi / mixed) | Run per-spec flows sequentially by dependency |

## Spec State Hints

Read `docs/specs/<feature>/spec.json` metadata only when routing:

| `approvals` state | Likely flow |
| ----------------- | ----------- |
| No spec / pre-init | 要求新規作成 (Path C+) |
| `approvals.requirements.generated` false | 要求更新 or resume requirements phase |
| requirements generated, design not | 設計更新 or resume design phase |
| design generated, tasks not | resume task generation (`/sdd-spec-tasks` … Terminal auto-approve) |
| `ready_for_implementation: true` | 実装のみ |

If orchestration was interrupted mid-flow, the next `/sdd-orchestrate <feature>` resumes from the next incomplete phase per `spec.json` `generated` flags and artifacts — artifact-only resume per `gates.md` § Resume.

## Execution Control

- Steps run **serial** by default.
- Design validate: single `/sdd-validate-design-qa` (Pass A qa→arch→sec serial inside one skill — no parallel `design.md` writes).
- All Pass A GO before Pass B final + inline phase-gate.
- Mid-flow user pivot → re-route; resume from required step.
- Status check → `/sdd-spec-status <feature>`.

## Path B vs 実装のみ

| | Path B 直接実装 | 実装のみ |
| - | --------------- | -------- |
| spec | none | existing |
| prerequisite | discovery Path B (standalone; never enters orchestration) | `ready_for_implementation: true` |
| implement | main context direct | `/sdd-impl` |
| verify | `/sdd-verify-completion` | `/sdd-impl` review + `/sdd-validate-impl` |
