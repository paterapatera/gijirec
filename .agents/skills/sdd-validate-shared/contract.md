# Validate Skill Contract (Shared)

All `/sdd-validate-*` skills in the AI-DLC flow share this contract. Read once per validate invocation.

## Autonomous Execution

- Do **not** ask the user questions during validate.
- Resolve ambiguity with reasonable assumptions; record them in `## Decisions`.
- If assumptions cannot be made safely → `VERDICT: NO-GO` or `MANUAL_VERIFY_REQUIRED`.
- Use the language specified in `spec.json`.

## Report Location

Write to `docs/specs/<feature>/reviews/<report>.md`:

| Skill | Report file |
| ----- | ----------- |
| `/sdd-validate-requirements` (unified) | `requirements-review.md` (**canonical**) |
| `/sdd-validate-design-qa` (unified) | `design-review.md` (**canonical**) |

Unified skills write **only** `*-review.md` on new runs. Do not write separate per-specialist report files.

Create `reviews/` if missing.

### Orchestrator parse rules (requirements)

- Require `reviews/requirements-review.md`: parse `VERDICT:` and `## Phase Gate` → `STATUS:`.
- Phase GO only when `VERDICT: GO` and Phase Gate `STATUS: VERIFIED`.
- Specs with only old 4-file reports (no `requirements-review.md`) are **NOT_VERIFIED** until re-validated with the unified skill.

### Orchestrator parse rules (design)

- Require `reviews/design-review.md`: parse `VERDICT:` and `## Phase Gate` → `STATUS:`.
- Phase GO only when `VERDICT: GO` and Phase Gate `STATUS: VERIFIED`.
- Specs with only old 4-file reports (no `design-review.md`) are **NOT_VERIFIED** until re-validated with the unified skill.

## Required Report Fields

```markdown
## Verdict
- VERDICT: GO | NO-GO | MANUAL_VERIFY_REQUIRED

## Summary
（2–3 文の要約）

## Findings
（重大度付き。NO-GO 時は修正指示を actionable に）

## Decisions
（自律的に確定した判断・前提・トレードオフ。承認ゲートでユーザーに報告する原文）

## Reflected Fixes
（対象成果物を編集した validate は必須。編集なしの場合は「なし」と明記）
| Finding | 対象セクション | 修正概要 |
| ------- | -------------- | -------- |

## Evidence
（参照したファイルパス・チェック項目。各チェック項目に pass / finding / N/A(理由付き) を明示 — 黙殺禁止）
```

Unified requirements reports additionally include `## Specialist Summaries`, `## Gap-Domain Audit`, `## 承認ゲートサマリ`, and `## Phase Gate` (see `/sdd-validate-requirements`). Unified design reports use the same extra sections (see `/sdd-validate-design-qa`).

The orchestrator parses **`VERDICT:`** (and for unified requirements/design, **Phase Gate `STATUS:`**) for mechanical gate decisions.

## Severity Vocabulary (shared)

All validates classify findings with the same scale:

- **Critical**: blocks the gate — requirement contradiction/traceability break, unreviewed sensitive path, claimed-but-missing fix → `NO-GO`
- **Major**: concrete deficiency — reflect a fix into the artifact (list in `## Reflected Fixes`) or record explicit risk acceptance in `## Decisions`
- **Minor**: polish/clarity — fix allowed, note in `## Reflected Fixes`; no gate impact

## Reflected Fixes Verifiability

Every fix a validate applies to `requirements.md` / `design.md` **must** appear as a `## Reflected Fixes` row (finding → target section → summary). Unified `/sdd-validate-requirements` Pass B verifies against the final `requirements.md`; unified `/sdd-validate-design-qa` Pass B verifies against the final `design.md`. Unverifiable free-text fix claims are treated as missing.

## Verdict Rules

| Verdict | When |
| ------- | ---- |
| `GO` | All in-scope checks pass; artifacts exist and are consistent |
| `NO-GO` | Blocking issues require regeneration or substantive fix |
| `MANUAL_VERIFY_REQUIRED` | Missing external info the agent cannot assume |

Before declaring `GO`, confirm fresh evidence: referenced files exist, edits are written, and content is internally consistent. At **orchestrated phase gates** (要求/設計/タスク):

- **Requirements (unified):** `/sdd-validate-requirements` embeds phase-gate checks; orchestrator does **not** dispatch `/sdd-verify-phase-gate` for requirements when `requirements-review.md` has `STATUS: VERIFIED`.
- **Design (unified):** `/sdd-validate-design-qa` embeds phase-gate checks; orchestrator does **not** dispatch `/sdd-verify-phase-gate` for design when `design-review.md` has `STATUS: VERIFIED`.
- **Tasks:** orchestrator still applies `/sdd-verify-phase-gate` (`PHASE_GATE`). See `phase-gate.md`.

`FEATURE_GO` is not used for pre-implementation gates.

## Phase Execution Order

**Requirements** (single skill): `/sdd-validate-requirements` (unified Pass A po→qa→sec + Pass B final + inline phase-gate). Optional `--only po|qa|sec|final` for partial re-runs.

**Design** (single skill): `/sdd-validate-design-qa` (unified Pass A qa→arch→sec + Pass B final + inline phase-gate). Optional `--only qa|arch|sec|final`.

Each specialist sub-pass reflects findings into its target artifact (`requirements.md` / `design.md`) before the next sub-pass. The next pass reads the **updated** artifact.

## Update Flows

When the orchestrator indicates an update flow (not new creation), scope work to changed sections only. Do not regenerate unrelated downstream artifacts. Prefer `--only` on the unified requirements/design skills for targeted re-validation.

## Context Loading (Minimal)

Always read:
- `docs/specs/<feature>/spec.json`
- Phase inputs listed in the invoking skill

Steering: start with `docs/steering/product.md`, `tech.md`, `structure.md`. Load additional steering only when directly relevant.

Do **not** re-read this contract or sibling validate skills unless the invoking skill directs it. Per-skill I/O: `phase-contracts.md` (read only when needed).

## spec.json Compatibility

- Missing `complexity_tier` (and related fields) on an existing spec → treat as **L** (**orchestration full-path only**). `/sdd-impl` execution mode still uses its task-count fallback when the field is absent. See `sdd-orchestrate/rules/complexity-tier.md` § Scope note.
- Do **not** change the existing `approvals` structure for backward compatibility.
