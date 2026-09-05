# Requirements Final Gate — Checklist

## Mission

Final requirements-phase gate for AI-DLC. Product Owner role. Autonomous; no user dialogue.

Goal: after this gate passes, the human approval gate should require only reading the report summary and accepting documented residual risks — no independent requirements re-review. To achieve that, this gate (a) verifies the specialist reviews were actually reflected, (b) audits every requirements-quality domain the specialists do **not** cover, and (c) self-repairs `requirements.md` for fixable findings instead of deferring them to the human.

## In Scope

- **Reflection verification** of Pass A specialist working notes (PO / QA / Sec) against final `requirements.md`
- **Gap-domain audit** — all requirements-quality angles outside the po/qa/sec scopes (see § Gap Domains)
- Brief → requirements traceability matrix (when `brief.md` exists)
- Self-repair of `requirements.md` for Minor and unambiguous Major findings
- Recording all decisions, self-repairs, and accepted residual risks in `## Decisions`
- Contributing to unified `reviews/requirements-review.md` (Pass B sections: Gap-Domain Audit, 承認ゲートサマリ, Phase Gate)

## Out of Scope

- Re-running the PO semantic/scope review (Pass A PO)
- Re-running the QA testability review (Pass A QA)
- Re-running the security review (Pass A Sec)
- EARS mechanical syntax checks (`requirements-review-gate` in `/sdd-spec-requirements`)
- Design-level audit (architecture, technology) — this gate judges requirements readiness only

## Preconditions (hard stop if unmet)

1. Pass A PO / QA / Sec completed in this invocation
2. `requirements.md` exists and reflects post-sec state

## Step 1 — Reflection Verification

1. From each specialist pass (working notes), extract: the `## Reflected Fixes` table, open items, deferred risks, and `## Decisions`.
2. **Verify every `## Reflected Fixes` row mechanically**: for each row (finding → requirements.md section → summary), open the named section of the final `requirements.md` and confirm the fix content exists. A claimed-but-missing fix is a **Critical** finding. A pass that edited `requirements.md` but has no `## Reflected Fixes` table (or only free-text fix claims) is itself a **Critical** finding — treat its fixes as unverified.
3. **Verify later edits did not invalidate earlier verdicts**: QA and Sec edit `requirements.md` after PO. Confirm their edits do not contradict a PO `## Decisions` entry, and Sec's edits do not undo a QA fix. A contradiction is **Critical**.
4. **Cross-check the three `## Decisions` sections** for contradictions with each other and with the final `requirements.md`. A contradiction is **Critical**.
5. Carry every deferred risk forward: each must appear in this gate's `## Decisions` as an explicitly accepted residual risk, or be resolved by self-repair.

## Step 2 — Gap Domains

Audit `requirements.md` against **every** domain below. Each domain gets an explicit pass/finding entry in Evidence — no domain may be silently skipped. If a domain is genuinely not applicable (e.g. no `brief.md` → no brief traceability), record `N/A` with a one-line reason.

1. **Brief traceability** — Build a matrix: every problem statement, scope decision, and boundary candidate in `brief.md` → the requirement/AC that covers it (or an explicit exclusion in scope boundaries). Include the matrix in the report Evidence. A brief scope decision with no requirement and no documented exclusion is **Critical**. Requirements with no brief backing feed the scope check (domain 7). No `brief.md` → `N/A`, and note the traceability source used instead (project description in `requirements.md`).
2. **Cross-spec consistency** — Against `docs/specs/roadmap.md` (if present): upstream dependencies' expectations are not contradicted; adjacent-system expectations in scope boundaries name the right specs; no obligation is claimed that an upstream spec owns. No roadmap → `N/A`.
3. **NFR completeness** — Performance, availability, capacity, and reliability expectations materially implied by `brief.md`, steering, or the feature's nature are present as user- or operator-observable ACs or explicitly excluded in scope boundaries. Missing material NFRs are findings; do not author NFR targets that require new scope decisions — that is a **NO-GO to `/sdd-spec-requirements`**. (Measurability of stated NFRs is qa domain — do not re-audit.)
4. **Operability expectations** — Operator-visible expectations (monitoring/alerting needs, manual intervention points, data retention/cleanup) implied by steering or the feature are stated at requirements level, or their absence is deliberate. Do not introduce design detail (no tooling or architecture choices).
5. **Compliance** — Regulatory/policy constraints from steering (`product.md` + any compliance steering) are reflected or their deviation documented. (Security-specific compliance is sec domain — verify sec addressed it rather than re-auditing.)
6. **Template conformance** — Structure matches `docs/settings/templates/specs/requirements.md`: introduction present; scope-boundary section present when adjacent systems/specs are touched; every requirement has 目的 and 受け入れ条件; numeric IDs only; language matches `spec.json` with EARS keywords in English.
7. **Scope fitness** — No gold-plating beyond `brief.md`/project description (requirement with no traceable origin) and no source scope silently dropped. Uses the matrix from domain 1 in both directions.
8. **Terminology & consistency** — Terms used consistently across requirements, scope boundaries, and ACs; consistent with steering vocabulary and upstream specs; no same-concept-different-name drift that would confuse design.

## Step 3 — Findings (no cap)

Record **all** findings with severity. Do not limit the count.

- **Critical**: claimed-but-missing specialist fix, cross-report contradiction, traceability break (uncovered brief scope decision), gap-domain hole that blocks design or invalidates a specialist GO → NO-GO unless self-repairable without touching specialist domains
- **Major**: gap-domain deficiency with a concrete fix, or residual risk needing explicit acceptance → self-repair or accept in `## Decisions`
- **Minor**: polish/consistency issue → self-repair silently allowed, but still record in `## Decisions`

## Step 4 — Self-Repair Policy

Fix findings in `requirements.md` directly rather than deferring them to the human, within these bounds:

- **Repair**: Minor findings, and Major findings whose fix is unambiguous from `brief.md`, steering, or the specialist reports (e.g. align a term with steering vocabulary, complete a scope-boundary entry the brief already decided, fix a heading ID, restore template structure without changing meaning).
- **Do not repair — NO-GO to `/sdd-spec-requirements`** when a fix needs a new scope or behavior decision (missing material NFR with no source to derive it from, uncovered brief scope decision with no obvious requirement shape): that content must be generated upstream and pass the po → qa → sec chain, not be authored at this gate.
- **Do not repair — NO-GO with named rollback target** when the fix would require re-analysis inside a specialist domain: functional scope/semantic ambiguity (`validate-requirements`), AC verifiability semantics (`validate-requirements-qa`), auth/PII/trust-boundary expectations (`validate-requirements-sec`). If a self-repair would even *touch* such an area, roll back instead.
- After repairing, re-check the edited sections for internal consistency and confirm no specialist-domain content changed. Every repair is listed as a `## Reflected Fixes` row (finding → requirements.md section → summary), with rationale in `## Decisions`.

## Step 5 — Report

Contribute Pass B sections to unified `reviews/requirements-review.md` (not a separate `requirements-final.md`): Gap-Domain Audit, 承認ゲートサマリ, Evidence (including brief→requirements matrix), and Phase Gate. Shared-contract fields (Verdict, Summary, Findings, Decisions, Reflected Fixes) are written once for the whole unified skill.

```markdown
## 承認ゲートサマリ
### 検証済み観点
（反映検証 + ギャップドメイン 1–8 の各結果を1行ずつ。N/A は理由付き）
### 自己修復した事項
（Pass B が requirements.md に加えた修正の一覧。なければ「なし」）
### 受容が必要な残リスク
（人間が受容判断すべきリスクのみ。各項目に根拠と却下時の影響。PO/QA/Sec の deferred リスクを含む）
### 人間判断が必要な未決事項
（理想は 0 件。0 件ならその旨を明記）
```

Evidence must include the brief→requirements traceability matrix (or its `N/A` rationale) and the per-domain check results.

## GO Criteria

- All three specialist passes completed successfully and every claimed fix verified present in `requirements.md`
- No unresolved cross-pass contradiction; later edits did not invalidate earlier decisions
- All 8 gap domains checked (pass, N/A with reason, or repaired)
- Traceability matrix has no uncovered brief scope decision
- All self-repairs applied, recorded, and re-checked for consistency
- Remaining open items are **only** residual risks explicitly listed for human acceptance

## NO-GO Triggers

- Any specialist pass incomplete or failing
- Claimed specialist fix absent from `requirements.md`
- Finding requires specialist re-analysis → name rollback: `po`, `qa`, or `sec` (or `/sdd-validate-requirements --only …`)
- Fix needs a new scope/behavior decision → name `/sdd-spec-requirements` as rollback target
- `requirements.md` contradicts adopted specialist decisions
- Cannot decide safely without user input → `MANUAL_VERIFY_REQUIRED`

## Update Flows

When the orchestrator indicates a requirements update (diff only): run reflection verification and gap domains only against changed requirements/ACs and anything they transitively touch; rebuild the traceability matrix only for changed entries; self-repair within the same diff scope. Do not re-audit unchanged domains or regenerate unrelated findings. The approval-gate summary still lists all 8 domains, marking unchanged ones as `unchanged (not re-audited)`.
