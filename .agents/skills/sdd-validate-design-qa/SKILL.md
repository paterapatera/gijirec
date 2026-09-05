---
name: sdd-validate-design-qa
description: Autonomous unified design validation (qa + arch + sec + final gate + phase-gate). Use in /sdd-orchestrate flows. Supports --only qa|arch|sec|final.
metadata:
  shared-rules: "../sdd-validate-shared/contract.md, ../sdd-validate-shared/phase-gate.md"
---


# Validate Design (Unified)

<background_information>
Single-pass design-phase validate for AI-DLC (replaces separate qa → arch → sec → ex → verify-phase-gate dispatches). Autonomous; no user dialogue. Writes `reviews/design-review.md` with one `VERDICT:` and inline `Phase Gate` status.
</background_information>

<instructions>
## Inputs

Authoritative review inputs for feature `$1`:

1. `docs/specs/$1/requirements.md`
2. `docs/specs/$1/design.md`
3. Paths listed in `design.md` **Persistent References** only (`Mode: modify` \| `reference`) — contracts / architecture / related ADR
4. Core steering: `docs/steering/tech.md`, `structure.md` (existing policy; do not broaden)

Also load (metadata / context, not full-project audit):

- Optional: `--only qa|arch|sec|final` (partial re-run; default = full Pass A→B→C)
- `docs/specs/$1/spec.json`
- `docs/specs/$1/research.md` if present
- `docs/steering/roadmap.md` or `docs/specs/roadmap.md` if present
- Security decisions: `reviews/requirements-review.md` when relevant

### Do **not** include as review inputs

- Unlisted files under `docs/contracts/**`
- All ADRs / full-project architecture audit

### Load rules (persistent docs)

| 資料 | このフェーズ |
|------|--------------|
| feature req/design/research | **主** |
| architecture / contracts / ADR | **主（関連）** — Persistent References paths only |

- Never glob-bulk-Read `docs/contracts/**` or `docs/architecture/**` or all ADRs
- Procedure: **Persistent References → those files only** (index only if a listed path needs discovery)
- Do not “read everything just in case”
- Do **not** run a full-project architecture audit every validate

### Review depth by change type (guideline)

| Change type | Review depth |
|-------------|--------------|
| Internal change, no contract surface | `design.md` + confirm **No contract changes**; minimal external contract Read |
| Non-breaking update to existing contracts | Listed related contracts only + normal validate |
| Boundary / breaking contract / new public surface | Related contracts + related ADR + Arch/Sec emphasis |

## Execution (unified pass)

### Pass A — Specialist reviews (single context load)

Load **once**: `spec.json`, `requirements.md`, `design.md`, `research.md` (if any), `tech.md`, `structure.md`, roadmap (if any), `../sdd-validate-shared/contract.md`, plus Persistent References paths only.

Do **not** dispatch sibling validate skills between sub-passes. Keep checklists separate — read each in order.

1. **QA** — Read `rules/qa-checklist.md`. Review edge cases / abnormal flows; fix `design.md` when local and safe. Record working notes.
2. **Arch** — Read `rules/arch-checklist.md`. Review SOLID/coupling/extensibility on the **updated** `design.md` against **referenced contracts only**. Treat external canonical vs design-summary contradiction as a Finding / NO-GO factor. `Mode: modify` path missing or stale → **NO-GO** or **Major**. Append working notes.
3. **Sec** — Read `rules/sec-design-checklist.md`. Threat model / data protection on the updated `design.md`. Append working notes.

After each sub-pass, append rows to an in-memory `## Reflected Fixes` table with a **Pass** column (`QA` | `Arch` | `Sec`).

If `--only qa|arch|sec`: run only that specialist sub-pass, then write `design-review.md` for that pass — do not claim full Phase Gate `VERIFIED` unless Pass B also ran.

### Pass B — Final gate (same invocation)

Skip when `--only qa|arch|sec` (unless `--only final`).

4. Read `rules/design-synthesis.md`.
5. **Reflection verification** — Verify all Reflected Fixes from Pass A against `design.md`; cross-check Decisions (synthesis § Step 1). Prefer in-memory Pass A notes.
6. **Gap-domain audit** — all 8 domains per synthesis § Step 2.
7. **Self-repair** — Minor / unambiguous Major in **non-specialist** domains only (synthesis § Step 4). Specialist-domain defects → `VERDICT: NO-GO` with rollback target — do **not** self-repair specialist domains here.
8. **Phase gate check** — Inline `../sdd-validate-shared/phase-gate.md` § design. Record under `## Phase Gate`.

### Pass C — Write outputs

9. Write **only** `docs/specs/$1/reviews/design-review.md` (format below).
10. Do **NOT** write separate per-specialist report files.

### Verdict

- Single `VERDICT: GO | NO-GO | MANUAL_VERIFY_REQUIRED` at the end of `design-review.md`
- On `NO-GO`: name rollback target in Findings (`/sdd-spec-design`, `qa` / `arch` / `sec` / `final`, or requirements phase)
- Orchestrator opens **[GATE] 設計** when `VERDICT: GO` **and** `Phase Gate` → `STATUS: VERIFIED` (no separate `/sdd-verify-phase-gate` for design in the flow)

## Report format (`reviews/design-review.md`)

```markdown
## Verdict
- VERDICT: GO

## Summary
...

## Reviewed Scope
- Reviewed contract paths: (Persistent References contracts read; or `none — No contract changes`)
- ADR paths: (ADRs read; or `none`)
- Contract sync: OK | MISSING | DRIFT

## Findings
...

## Decisions
...

## Reflected Fixes
| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | ... | ... | QA |

## Specialist Summaries
### QA
...
### Arch
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

**Reviewed Scope** (required, short):

- **Reviewed contract paths** — every contract path actually Read from Persistent References
- **ADR paths** — every ADR actually Read (empty/`none` if none)
- **Contract sync**:
  - `OK` — listed `Mode: modify` files exist and match design boundary/public-surface intent; `reference`-only / **No contract changes** when applicable
  - `MISSING` — a `Mode: modify` (or required new public-surface) path is absent
  - `DRIFT` — external canonical text contradicts the design summary / Boundary Commitments

`MISSING` or `DRIFT` is a Finding and may be **NO-GO** or **Major** (see arch-checklist).
## Update mode

When `/sdd-orchestrate` runs 要求更新 or 設計更新, scope to **changed design sections and linked requirements only**. Prefer `--only qa|arch|sec|final` for targeted re-runs.

## Constraints

- Do not merge qa/arch/sec checklists into one list.
- Do not insert other skill dispatches between Pass A sub-passes.
- Do not self-repair specialist-domain content in Pass B.
- Do not ask the user questions.
- Every `design.md` edit must appear as a `## Reflected Fixes` row with Pass label.

## On NO-GO

Orchestrator rolls back per Findings (usually `/sdd-spec-design`, then re-run this unified skill). Specialist-domain defects may name `qa` / `arch` / `sec` for `--only` re-run after a targeted fix.
</instructions>

## Safety

- Missing `design.md` → stop: run `/sdd-spec-design $1` first.
- Pass B must not claim `Phase Gate STATUS: VERIFIED` if Pass A did not complete all three specialists.
