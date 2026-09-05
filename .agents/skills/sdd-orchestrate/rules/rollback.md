# Rollback

On `NO-GO` / `REJECTED`, roll back to the **generating step** for the failed check. Report: failure reason, impact scope, re-run steps.

| Failed check | Rollback to | Re-run from |
| ------------ | ----------- | ----------- |
| `/sdd-validate-requirements` (unified) | `/sdd-spec-requirements`; if Findings names `po`/`qa`/`sec` → fix `requirements.md` then `--only` that pass or full re-run | `/sdd-validate-requirements` |
| `/sdd-validate-design-qa` (unified) | `/sdd-spec-design`; if Findings names `qa`/`arch`/`sec` → fix `design.md` then `--only` that pass or full re-run; if Findings names a requirements defect → `/sdd-spec-requirements` (apply 要求 rollback-depth rule) | `/sdd-validate-design-qa`; requirements cause → `/sdd-validate-requirements` → design chain |
| `/sdd-impl` task review | that task's implementation | `/sdd-review` |
| `/sdd-validate-impl` | causing task or design | task → `/sdd-impl`; design cause → `/sdd-spec-design` onward |

## Phase gate failures (`/sdd-verify-phase-gate` or unified inline Phase Gate)

On `NOT_VERIFIED`, parse `GAPS` / Phase Gate `CHECKS` against `../sdd-validate-shared/phase-gate.md`. Do **not** open the human approval gate. On `MANUAL_VERIFY_REQUIRED`, stop and report gaps — rollback only if the user directs a fix path.

| Phase | Gap (checklist item) | Rollback to | Re-run from |
| ----- | -------------------- | ----------- | ----------- |
| `requirements` | missing / empty `requirements.md` | `/sdd-spec-requirements` | `/sdd-validate-requirements` |
| `requirements` | `approvals.requirements.generated !== true` | `/sdd-spec-requirements` | `/sdd-validate-requirements` |
| `requirements` | non-GO / missing `reviews/requirements-review.md` | `/sdd-spec-requirements` or `requirements.md` fix | `/sdd-validate-requirements` |
| `requirements` | Phase Gate not `VERIFIED` | fix gaps named in CHECKS | `/sdd-validate-requirements` or standalone `/sdd-verify-phase-gate` |
| `requirements` | `approvals.requirements.approved === true` | **[調整者]** re-apply 要求更新 approval invalidation (`flows.md`) | re-check Phase Gate |
| `design` | missing `design.md` | `/sdd-spec-design` | `/sdd-validate-design-qa` |
| `design` | `approvals.design.generated !== true` | `/sdd-spec-design` | `/sdd-validate-design-qa` |
| `design` | non-GO / missing `reviews/design-review.md` | `/sdd-spec-design` or `design.md` fix | `/sdd-validate-design-qa` |
| `design` | Phase Gate not `VERIFIED` | fix gaps named in CHECKS | `/sdd-validate-design-qa` or standalone `/sdd-verify-phase-gate` |
| `design` | `approvals.design.approved === true` | **[調整者]** set `approvals.design.approved: false` (and `approvals.tasks.approved: false`, `ready_for_implementation: false` if tasks were approved) | re-check Phase Gate |
| `tasks` | missing / empty `tasks.md` | `/sdd-spec-tasks` | `/sdd-verify-phase-gate` |
| `tasks` | `approvals.tasks.generated !== true` | `/sdd-spec-tasks` | `/sdd-verify-phase-gate` |
| `tasks` | `approvals.tasks.approved === true` | **[調整者]** set `approvals.tasks.approved: false`, `ready_for_implementation: false` | `/sdd-verify-phase-gate` |
| `tasks` | `_Blocked:_` tasks present | stop — report user | resolve blockers before re-gate |

## Rules

- Requirements change affecting design/impl → ask user rollback depth: 要求のみ / 設計まで / タスクまで
- **2 consecutive NO-GO** on same step → stop; seek user re-alignment
- Update flows: do **not** regenerate downstream artifacts unrelated to the change diff
