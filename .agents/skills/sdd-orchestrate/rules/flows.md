# Flow Step Sequences

**Orchestration scope ends at task generation.** The generation flows (要求新規作成 / 要求更新 / 設計更新) terminate at **Terminal auto-approve** (M/L: tasks; S: 仕様一式). After mechanical readiness, the orchestrator auto-approves, emits the **PR Summary Output** (`gates.md` § PR Summary Output), and **ends the orchestration** — it must **not** dispatch `/sdd-impl` or any implementation step. Implementation is run separately (explicit `実装のみ` invocation only).

**Entry precondition (discovery is not an orchestration step).** `/sdd-discovery` is run **standalone before** orchestration and has already produced `brief.md` (for new specs) / `roadmap.md` (when dependencies exist) and, for existing specs, `spec.json`. Orchestration is invoked with a required target `<feature>` plus optional explicit flow, and selects the active flow per `routing.md` § Entry Contract. If neither `brief.md` nor `spec.json` exists for the target, **stop** and instruct the user to run `/sdd-discovery` first (do not auto-run discovery).

Load **only** the section matching the active flow and complexity tier (`要求新規作成 (S|M|L)`, etc.). After each validate step: phase gate must be verified — for **要求**, via unified `/sdd-validate-requirements` (`requirements-review.md`); for **設計**, via unified `/sdd-validate-design-qa` (`design-review.md`). On `VERIFIED` for 要求 / 設計 (M/L): **Phase terminal** — handoff → end. Do **not** continue numbered steps below the boundary in the same conversation. Resume in a **new chat** on the **same Git checkout** with `/sdd-orchestrate <feature>`. For **タスク** terminal: `/sdd-verify-phase-gate <feature> tasks` then set `ready_for_implementation: true` (**Terminal auto-approve**). S-tier quick-path uses sanity review (and optional unified validates) then **Terminal auto-approve (S)**.

**Session boundary (M/L):** After 要求 or 設計 is mechanically ready, emit Phase Handoff and **stop**. No `go` wait. Human reviews artifacts: same-chat correction notes stay in this phase; a new `/sdd-orchestrate <feature>` means proceed. (S quick-path is the exception per `gates.md` § Exceptions.)

`[調整者]` steps are orchestrator-only (not skill dispatches). Orchestrator updates `spec.json` directly — including `complexity_tier` / `complexity_score` / `complexity_rationale` at flow entry (`routing.md` § Complexity Tier).

## Orchestration Paths by Tier

After complexity tier is computed (`routing.md` § Complexity Tier):

| Tier | Path name | Behavior |
|------|-----------|----------|
| S | **quick-path** | Delegate to `/sdd-spec-quick <feature> --auto --from-orchestrate` |
| M | **standard-path** | Unified validates; 要求 / 設計 each → Phase terminal; タスクは再開後に自動 |
| L | **full-path** | Current 要求新規作成 (L) — all steps; 要求 / 設計 each → Phase terminal; タスクは再開後に自動 |

User override `quick` → force quick-path regardless of score.
User override `full` → force full-path.

## 要求新規作成 (L)

_Precondition_: `/sdd-discovery` (Path C/D/E) already ran standalone; `brief.md` exists at `docs/specs/<feature>/`. Selected when `complexity_tier` is **L** (score ≥ 5, Path D/E, user「フル」/「full」, or missing `complexity_tier` on resume).

**Path**: full-path. 要求 / 設計は機械ゲート通過後に Phase terminal。タスクは再開後に Terminal auto-approve。

**Greenfield**: Never run a standalone gap step. spec-design Step 2.0 auto-skips.
**Brownfield**: Gap runs inside spec-design only (07).

1. **[調整者] Upstream dependency guard** — verify roadmap upstream deps are task-generation complete (`routing.md` § Upstream Dependency Guard). If not ready, **stop** before requirements.
2. `/sdd-spec-requirements <feature>` (initializes `spec.json` if missing — Step 0)
3. `/sdd-validate-requirements <feature>` (unified: po+qa+sec+ex+phase-gate → `reviews/requirements-review.md`)
4. **[調整者] Phase terminal (要求)** — handoff → end。同一フロー内で `/sdd-spec-design` に進まない
── session boundary ──（再開後のフローで実行）
5. `/sdd-spec-design <feature>` (inline brownfield gap analysis; greenfield skips gap)
6. `/sdd-validate-design-qa <feature>` (unified: qa+arch+sec+ex+phase-gate → `reviews/design-review.md`)
7. **[調整者] Phase terminal (設計)** — handoff → end。同一フロー内で `/sdd-spec-tasks` に進まない
── session boundary ──（再開後のフローで実行）
8. `/sdd-spec-tasks <feature>`
9. `/sdd-verify-phase-gate <feature> tasks`（未実施なら）
10. **[調整者] Terminal auto-approve** — set `ready_for_implementation: true` → PR Summary Output（`gates.md`）→ end（実装工程には進まない）

## 要求新規作成 (S)

_Precondition_: same as (L); selected when `complexity_tier` is **S** (score ≤ 1, or user「quick」/「lite」). **Never** for Path D/E.

**Path**: quick-path. Terminal auto-approve (S) after quick-path success.

**Greenfield**: Never run a standalone gap step. spec-design Step 2.0 auto-skips.
**Brownfield**: Gap runs inside spec-design only (07).

1. **[調整者] Upstream dependency guard** — verify roadmap upstream deps are task-generation complete (`routing.md` § Upstream Dependency Guard). If not ready, **stop** before generation.
2. `/sdd-spec-quick <feature> --auto --from-orchestrate` — generates requirements + design + tasks; runs sanity review (and optional unified validates). Do **not** dispatch individual `spec-requirements` / `validate-*` / `spec-design` / `spec-tasks` separately.
3. **[調整者] Terminal auto-approve (S)** — set `ready_for_implementation: true` → PR Summary Output（`gates.md`）→ end（実装工程には進まない）

## 要求新規作成 (M)

_Precondition_: same as (L); selected when `complexity_tier` is **M** (score 2–4).

**Path**: standard-path（same as L）. 要求 / 設計は Phase terminal。タスクは再開後。

**Greenfield**: Never run a standalone gap step. spec-design Step 2.0 auto-skips.
**Brownfield**: Gap runs inside spec-design only (07).

1. **[調整者] Upstream dependency guard** — verify roadmap upstream deps are task-generation complete (`routing.md` § Upstream Dependency Guard). If not ready, **stop** before requirements.
2. `/sdd-spec-requirements <feature>` (initializes `spec.json` if missing — Step 0)
3. `/sdd-validate-requirements <feature>` (unified: po+qa+sec+ex+phase-gate)
4. **[調整者] Phase terminal (要求)** — handoff → end。同一フロー内で `/sdd-spec-design` に進まない
── session boundary ──（再開後のフローで実行）
5. `/sdd-spec-design <feature>` (inline brownfield gap; greenfield skips)
6. `/sdd-validate-design-qa <feature>` (unified: qa+arch+sec+ex+phase-gate)
7. **[調整者] Phase terminal (設計)** — handoff → end。同一フロー内で `/sdd-spec-tasks` に進まない
── session boundary ──（再開後のフローで実行）
8. `/sdd-spec-tasks <feature>`
9. `/sdd-verify-phase-gate <feature> tasks`（未実施なら）
10. **[調整者] Terminal auto-approve** — set `ready_for_implementation: true` → PR Summary Output（`gates.md`）→ end（実装工程には進まない）

## 要求更新

_Precondition_: `/sdd-discovery` already ran standalone and confirmed this is an update to existing spec `<feature>` (`spec.json` exists).

1. **[調整者] Modification guard** — verify the target spec's implementation is complete (`routing.md` § Modification Guard). If it is implementation-ready but not complete (`ready_for_implementation: true` with `[ ]` / `_Blocked:_` tasks), **stop** and prompt the user to finish implementation first (explicit `実装のみ`). Do not proceed to the next step.
2. **[調整者] Upstream dependency guard** — verify roadmap upstream deps are task-generation complete (`routing.md` § Upstream Dependency Guard). If not ready, **stop** before generation or validate.
3. **[調整者]** Invalidate implementation readiness in `docs/specs/<feature>/spec.json` **before** generation or validate:
   - `ready_for_implementation: false`
   - Update `updated_at`
   - Do **not** clear `generated` flags — existing artifacts remain until regenerated in later steps
4. `/sdd-spec-requirements <feature>` (diff only)
5. `/sdd-validate-requirements <feature>` (unified; diff only — optional `--only po|qa|sec|final`)
6. **[調整者] Phase terminal (要求)** — handoff → end。同一フロー内で `/sdd-spec-design` に進まない
── session boundary ──（再開後のフローで実行）
7. `/sdd-spec-design <feature>` (requirements diff only)
8. `/sdd-validate-design-qa <feature>` (unified; diff only — optional `--only qa|arch|sec|final`)
9. **[調整者] Phase terminal (設計)** — handoff → end。同一フロー内で `/sdd-spec-tasks` に進まない
── session boundary ──（再開後のフローで実行）
10. `/sdd-spec-tasks <feature>` (diff only)
11. `/sdd-verify-phase-gate <feature> tasks`（未実施なら）
12. **[調整者] Terminal auto-approve** — set `ready_for_implementation: true` → PR Summary Output（`gates.md`）→ end（実装工程には進まない）

## 設計更新

_Precondition_: `/sdd-discovery` already ran standalone and confirmed this is a design-only change to existing spec `<feature>` (`spec.json` exists, `approvals.requirements.generated: true`).

1. **[調整者] Modification guard** — verify the target spec's implementation is complete (`routing.md` § Modification Guard). If it is implementation-ready but not complete, **stop** and prompt the user to finish implementation first (explicit `実装のみ`). Do not proceed.
2. **[調整者] Upstream dependency guard** — verify roadmap upstream deps are task-generation complete (`routing.md` § Upstream Dependency Guard). If not ready, **stop** before generation or validate.
3. `/sdd-spec-design <feature>`
4. `/sdd-validate-design-qa <feature>` (unified; diff only — optional `--only qa|arch|sec|final`)
5. **[調整者] Phase terminal (設計)** — handoff → end。同一フロー内で `/sdd-spec-tasks` に進まない
── session boundary ──（再開後のフローで実行）
6. `/sdd-spec-tasks <feature>` (diff only)
7. `/sdd-verify-phase-gate <feature> tasks`（未実施なら）
8. **[調整者] Terminal auto-approve** — set `ready_for_implementation: true` → PR Summary Output（`gates.md`）→ end（実装工程には進まない）

## 実装のみ

**Enter only on an explicit user request for implementation** (e.g.「実装だけ」). This flow is never reached automatically from a task-generation flow — Terminal auto-approve does not chain into it.

_Precondition_: `/sdd-discovery` already ran standalone (Path A → impl only); `spec.json` exists.

1. Verify `ready_for_implementation: true` — stop if false
2. `/sdd-impl <feature>` — impl selects execution mode `direct` / `wave` / `strict` from `spec.json` `complexity_tier` (or task-count fallback); see `sdd-impl` Step 2
3. `/sdd-validate-impl <feature>`
4. `/sdd-verify-completion <feature>` (`FEATURE_GO`) → end

## Path B 直接実装

Path B is decided by `/sdd-discovery` **before** orchestration. When discovery returns Path B, the work does **not** enter orchestration at all — it is implemented directly in main context. This section documents that boundary; orchestration itself has no Path B flow to run.

1. Implement in main context (no `/sdd-impl`)
2. `/sdd-verify-completion` (claim `FIX` or `TEST_OR_BUILD`)
3. Report changes; user confirmation → end

**Do not use**: `spec.json` gates, `/sdd-impl`, `/sdd-validate-impl`, mandatory `/sdd-review`.

## Path D/E Multi-Spec

_Precondition_: `/sdd-discovery` already ran standalone and produced `roadmap.md` + `brief.md` for each spec.

Each conversation still takes **one** `<feature>`. Independent specs (`Dependencies: none`) may run in **parallel checkouts**. A downstream spec must run in a checkout that already contains upstream task-generation artifacts (typically after those PRs merge to the integration branch). Do not start `/sdd-orchestrate <downstream>` from a Discovery-only tip.

For each spec in roadmap dependency order (when its checkout is ready):

1. **[調整者] Upstream dependency guard** for that spec (`routing.md` § Upstream Dependency Guard). If not ready, **stop** — do not start this spec's flow. Readiness is this checkout only.
2. Force `complexity_tier: L` for each spec (Path D/E). Run the full applicable flow above (`要求新規作成 (L)` / 要求更新 / 設計更新 as appropriate). Never select 要求新規作成 (S) for multi-spec.
