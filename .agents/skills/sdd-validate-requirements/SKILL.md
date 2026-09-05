---
name: sdd-validate-requirements
description: Unified autonomous requirements-phase validate (PO + QA + Sec + final gate + phase-gate). Semantic consistency, testability, security, reflection verification, gap-domain audit. Use after /sdd-spec-requirements in AI-DLC. No user dialogue. Supports --only po|qa|sec|final for partial re-runs.
metadata:
  shared-rules: "../sdd-validate-shared/contract.md, ../sdd-validate-shared/phase-gate.md"
---


# Validate Requirements (Unified)

<background_information>
Single-pass requirements-phase validate for AI-DLC (replaces separate po → qa → sec → ex → verify-phase-gate dispatches). Autonomous; no user dialogue. Writes `reviews/requirements-review.md` with one `VERDICT:` and inline `Phase Gate` status for the human approval gate.
</background_information>

<instructions>
## Inputs

- Feature: `$1`
- Optional: `--only po|qa|sec|final` (partial re-run; default = full Pass A→B→C)
- `docs/specs/$1/requirements.md`
- `docs/specs/$1/brief.md` (if exists)
- `docs/specs/$1/spec.json`
- Core steering: `docs/steering/product.md`, `tech.md`, `structure.md`
- `docs/steering/roadmap.md` if present (gap-domain cross-spec)
- `docs/settings/templates/specs/requirements.md` (template conformance)

## Execution (unified pass)

### Pass A — Specialist reviews (single context load)

Load **once**: `spec.json`, `requirements.md`, `brief.md` (if any), `product.md`, `tech.md`, `structure.md`, `../sdd-validate-shared/contract.md`.

Do **not** dispatch sibling validate skills between sub-passes. Keep checklists separate — read each in order.

1. **PO** — Read `rules/po-checklist.md`. Review & fix `requirements.md` when local and safe. Record working notes (Summary, Findings, Decisions, Reflected Fixes).
2. **QA** — Read `rules/qa-requirements-checklist.md`. Review & fix on the **updated** `requirements.md`. Append working notes.
3. **Sec** — Read `rules/sec-requirements-checklist.md`. Review & fix / adopt-defer on the updated `requirements.md`. Append working notes.

After each sub-pass, append rows to an in-memory `## Reflected Fixes` table with a **Pass** column (`PO` | `QA` | `Sec`).

If `--only po|qa|sec`: run only that specialist sub-pass (still load context once), then write `requirements-review.md` with that pass's summary and `VERDICT` for the pass — do not claim full Phase Gate `VERIFIED` unless Pass B also ran.

### Pass B — Final gate (same invocation)

Skip when `--only po|qa|sec` (unless `--only final`).

4. Read `rules/requirements-synthesis.md`.
5. **Reflection verification** — Verify all Reflected Fixes from Pass A against `requirements.md`; cross-check Decisions for contradictions (synthesis § Step 1). Prefer in-memory Pass A notes.
6. **Gap-domain audit** — all 8 domains per synthesis § Step 2.
7. **Self-repair** — Minor / unambiguous Major in **non-specialist** domains only (synthesis § Step 4). Specialist-domain defects → `VERDICT: NO-GO` with rollback target in Findings — do **not** self-repair specialist domains here.
8. **Phase gate check** — Inline `../sdd-validate-shared/phase-gate.md` § requirements (unified checks). Record results under `## Phase Gate`.

### Pass C — Write outputs

9. Write **only** `docs/specs/$1/reviews/requirements-review.md` (format below).
10. Do **NOT** write separate per-specialist report files.

### Verdict

- Single `VERDICT: GO | NO-GO | MANUAL_VERIFY_REQUIRED` at the end of `requirements-review.md`
- On `NO-GO`: name rollback target in Findings (`/sdd-spec-requirements` or specific pass: `po` / `qa` / `sec` / `final`)
- Orchestrator opens **[GATE] 要求** when `VERDICT: GO` **and** `Phase Gate` → `STATUS: VERIFIED` (no separate `/sdd-verify-phase-gate` dispatch in the flow)

## Report format (`reviews/requirements-review.md`)

```markdown
## Verdict
- VERDICT: GO

## Summary
...

## Findings
...

## Decisions
...

## Reflected Fixes
| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| PO-1 | ... | ... | PO |

## Specialist Summaries
### PO
（Summary + 主要 Decisions）
### QA
...
### Sec
...

## Gap-Domain Audit
（8 ドメイン表）

## 承認ゲートサマリ
### 検証済み観点
...
### 自己修復した事項
...
### 受容が必要な残リスク
...
### 人間判断が必要な未決事項
...

## Evidence
...

## Phase Gate
- STATUS: VERIFIED | NOT_VERIFIED | MANUAL_VERIFY_REQUIRED
- CHECKS: (inline phase-gate checklist results)
```

## Update mode

When `/sdd-orchestrate` runs 要求更新 (not new creation), scope to **changed requirements and ACs only** per contract Update Flows. Prefer `/sdd-validate-requirements $1 --only …` for targeted re-runs after a partial fix.

## Constraints

- Do not merge po/qa/sec checklists into one list — read them separately in Pass A.
- Do not insert other skill dispatches between Pass A sub-passes.
- Do not self-repair specialist-domain content in Pass B.
- Do not ask the user questions.
- Every `requirements.md` edit must appear as a `## Reflected Fixes` row with Pass label.
- Preserve EARS keyword English and numeric requirement/AC IDs.

## On NO-GO

Orchestrator rolls back per Findings (usually `/sdd-spec-requirements`, then re-run this unified skill). Specialist-domain defects may name `po` / `qa` / `sec` for `--only` re-run after a targeted `requirements.md` fix.
</instructions>

## Safety

- Missing `requirements.md` → stop: run `/sdd-spec-requirements $1` first.
- Missing `spec.json` → stop: run `/sdd-spec-requirements $1` first (Step 0 initializes).
- Pass B must not claim `Phase Gate STATUS: VERIFIED` if Pass A did not complete all three specialists.
