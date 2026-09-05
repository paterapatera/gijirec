# Phase Gate Verification (PHASE_GATE)

Read when executing `/sdd-verify-phase-gate` or when the orchestrator needs pre-implementation gate evidence.

## Claim Type Matrix

| Situation | Skill | Claim type |
| --------- | ----- | ---------- |
| 要求 / 設計 / タスクの機械 validate 完了後（人間承認前） | `/sdd-verify-phase-gate` | `PHASE_GATE` |
| `/sdd-impl` バッチ／複数タスク選択の完了ゲート（`[x]` 直前） | `/sdd-verify-completion` | `BATCH` |
| `/sdd-impl` 単一手動タスクの完了ゲート | `/sdd-verify-completion` | `TASK` |
| Path B 直接実装の完了 | `/sdd-verify-completion` | `FIX` or `TEST_OR_BUILD` |
| 全タスク完了 + `/sdd-validate-impl` GO 後 | `/sdd-verify-completion` | `FEATURE_GO` |

**Override**: 要求・設計のフェーズゲートは通常、統合 validate 内の `## Phase Gate` を使う。タスク、および再検証では `/sdd-verify-phase-gate`（`PHASE_GATE`）。`sdd-orchestrate/rules/gates.md` と `../sdd-validate-shared/contract.md` が正本。

`FEATURE_GO` はテストスイート・ランタイム smoke・統合評価が必要なため、実装前フェーズには不適切。

## Fresh Evidence

- Read files from disk in the current turn
- Parse `VERDICT:` line in each required `reviews/*.md`
- Do not infer GO from validate skill chat output alone

## Phase: `requirements`

| # | Check |
| - | ----- |
| 1 | `docs/specs/<feature>/requirements.md` exists and has requirement / AC content |
| 2 | `spec.json` → `approvals.requirements.generated === true` |
| 3 | `reviews/requirements-review.md` → `VERDICT: GO` |
| 4 | `reviews/requirements-review.md` → Phase Gate `STATUS: VERIFIED` |
| 5 | `approvals.requirements.approved === false` (not yet human-approved; gate is pre-approval) |

If `requirements-review.md` is absent (including specs with only old 4-file reports), result is **NOT_VERIFIED** — re-run `/sdd-validate-requirements` to generate the unified report.

Unified `/sdd-validate-requirements` performs these checks **inline** (Pass B step 8) and records results under `## Phase Gate`. Orchestrated 要求 flows do **not** dispatch `/sdd-verify-phase-gate` when the unified report already has `STATUS: VERIFIED`. Standalone `/sdd-verify-phase-gate <feature> requirements` remains available for debug / re-check.

## Phase: `design`

| # | Check |
| - | ----- |
| 1 | `docs/specs/<feature>/design.md` exists |
| 2 | `spec.json` → `approvals.design.generated === true` |
| 3 | `reviews/design-review.md` → `VERDICT: GO` |
| 4 | `reviews/design-review.md` → Phase Gate `STATUS: VERIFIED` |
| 5 | `approvals.design.approved === false` |

If `design-review.md` is absent (including specs with only old 4-file reports), result is **NOT_VERIFIED** — re-run `/sdd-validate-design-qa` to generate the unified report.

Unified `/sdd-validate-design-qa` performs these checks **inline** (Pass B step 8) and records results under `## Phase Gate`. Orchestrated 設計 flows do **not** dispatch `/sdd-verify-phase-gate` when the unified report already has `STATUS: VERIFIED`. Standalone `/sdd-verify-phase-gate <feature> design` remains available for debug / re-check.

## Phase: `tasks`

| # | Check |
| - | ----- |
| 1 | `docs/specs/<feature>/tasks.md` exists with at least one task entry |
| 2 | `spec.json` → `approvals.tasks.generated === true` |
| 3 | `approvals.tasks.approved === false` |
| 4 | No `_Blocked:_` tasks unless orchestrator is explicitly resuming blocked work |

Tasks phase has no `reviews/*.md` mechanical validates; generation + structure checks suffice.

## Verdict Mapping

| Result | Orchestrator action |
| ------ | ------------------- |
| `VERIFIED` | Proceed to human approval gate (`[GATE]`) |
| `NOT_VERIFIED` | Do not open human gate; fix or rollback |
| `MANUAL_VERIFY_REQUIRED` | Stop; report gaps to user |
