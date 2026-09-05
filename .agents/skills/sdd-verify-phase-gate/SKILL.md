---
name: sdd-verify-phase-gate
description: Verify spec phase gate readiness (要求/設計/タスク) with fresh artifact and VERDICT evidence. Use at AI-DLC pre-implementation phase gates instead of FEATURE_GO. Implementation completion uses /sdd-verify-completion with FEATURE_GO.
metadata:
  shared-rules: "../sdd-validate-shared/phase-gate.md"
---

# sdd-verify-phase-gate

<background_information>
Pre-implementation phase gates (requirements, design, tasks) must **not** use `FEATURE_GO` from `/sdd-verify-completion` — that claim type requires tests, runtime smoke, and integration evidence.

This skill verifies **artifact readiness** only: files exist, `VERDICT: GO` on required reports, and `spec.json` generation flags align.
</background_information>

<instructions>
## When to Use

- After all mechanical validates for a phase report `GO`, **before** the human approval gate（要求 / 設計）or **before** Terminal auto-approve（タスク）
- When `/sdd-orchestrate` reaches the tasks terminal step (`/sdd-verify-phase-gate <feature> tasks` → then auto-approve)
- **Requirements / design:** normally embedded in `/sdd-validate-requirements` / `/sdd-validate-design-qa` (`*-review.md` ## Phase Gate). Use this skill standalone for those phases only for debug / re-check
- **Do not** use for: per-task impl loop (`TASK`), Path B end (`FIX` / `TEST_OR_BUILD`), or post-impl feature completion (`FEATURE_GO`)

## Inputs

- Feature: `$1`
- Phase: `requirements` | `design` | `tasks` (from `$2` or active orchestrate gate)
- `docs/specs/$1/spec.json`

## Execution

1. Read `../sdd-validate-shared/phase-gate.md` (checklist for the phase).
2. Read listed artifacts from disk (**fresh** — do not trust conversation memory).
3. Run the phase checklist; record pass/fail per item in Evidence.
   - **要求 / 設計:** Require unified `reviews/*-review.md` (`VERDICT: GO` + Phase Gate `STATUS: VERIFIED`). Absence of the unified file → `NOT_VERIFIED` (re-run the unified validate skill).
4. Return result per Output Format below.

## Output

Return one of:
- `VERIFIED` — safe to proceed to human approval gate
- `NOT_VERIFIED` — missing artifact, non-GO verdict, or inconsistency
- `MANUAL_VERIFY_REQUIRED` — cannot determine without user input

```md
## Verification Result
- STATUS: VERIFIED | NOT_VERIFIED | MANUAL_VERIFY_REQUIRED
- CLAIM_TYPE: PHASE_GATE
- PHASE: requirements | design | tasks
- FEATURE: <feature>
- CLAIM: <e.g. "Requirements phase ready for human approval">
- EVIDENCE: <checklist items and file paths inspected>
- GAPS: <what failed or is missing>
- NOTES: <next action if not verified>
```

Use language from `spec.json`.

## On NOT_VERIFIED

Do not open the human approval gate. Re-run the failing validate or generation step per `sdd-orchestrate/rules/rollback.md` § Phase gate failures.
</instructions>

## Safety

- Missing `spec.json` → `NOT_VERIFIED`
- Unknown phase → stop; ask controller to pass `requirements`, `design`, or `tasks`
