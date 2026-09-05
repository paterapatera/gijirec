# QA Requirements Validate Checklist

## Mission

Requirements-phase testability review. Quality administrator role. Make every requirement verifiable before design starts. No user dialogue.

## In Scope

- AC verifiability: each acceptance criterion has an observable trigger and outcome
- Abnormal-flow / unwanted-behavior AC coverage per requirement
- Boundary conditions (limits, empty/zero, maximum, concurrency-visible effects) where user-observable
- NFR measurability: vague qualifiers normalized into concrete user- or operator-observable expectations
- AC overlap and coverage gaps within and across requirements
- Fix `requirements.md` directly when issues are local and unambiguous
- Record all autonomous decisions in report `## Decisions`; all edits as `## Reflected Fixes` rows

## Out of Scope

- Functional scope decisions and semantic ambiguity resolution (PO validate)
- Security failure modes and abuse cases (`/sdd-validate-requirements --only sec`)
- EARS mechanical syntax checks (`requirements-review-gate` in `/sdd-spec-requirements`)
- Design-level edge-case analysis (`/sdd-validate-design-qa`)
- User dialogue

## Checks

1. Every AC is verifiable: a tester could define a concrete observation (input/event → observable result) without guessing intent
2. Every requirement with a happy-path AC also covers its user-visible failure modes (invalid input, unavailable dependency, timeout, rejection) or documents why none apply
3. Boundary values that change behavior (limits, quotas, empty sets, size/time thresholds) have explicit ACs where the requirement implies them
4. NFR ACs are measurable: no bare "fast", "robust", "scalable", "user-friendly" — replace with concrete expectations when source material supports it, else flag as finding
5. No two ACs assert contradictory or duplicate obligations for the same trigger
6. Each AC outcome is observable to a user or operator — flag ACs verifiable only by inspecting internals (design smell in requirements)
7. Fixes preserve EARS keyword English (When, If, While, Where, shall) and numeric requirement/AC IDs

## Findings Severity

Use the shared scale in `../../sdd-validate-shared/contract.md` (Critical / Major / Minor):

- **Critical**: core AC untestable or a requirement has no verifiable AC at all → `NO-GO`
- **Major**: missing abnormal-flow AC or unmeasurable NFR → fix into `requirements.md` (list in `## Reflected Fixes`) or record risk acceptance in `## Decisions`
- **Minor**: wording/observability polish → fix inline if safe; note in `## Reflected Fixes`

## NO-GO Triggers

- A core requirement whose ACs cannot be made verifiable without new scope decisions (PO/user territory)
- Systematic absence of abnormal-flow ACs that cannot be patched locally without regeneration
- Fixing testability would require contradicting a PO decision in Pass A PO notes / `## Specialist Summaries` → PO
