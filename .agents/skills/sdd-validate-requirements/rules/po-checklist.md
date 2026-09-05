# PO Requirements Validate Checklist

## Mission

Post-generation semantic review and autonomous brush-up of `requirements.md`. Product Owner role.

## In Scope

- Semantic consistency across requirements and ACs
- Ambiguity resolution with documented assumptions
- Scope clarity (in/out of scope interpretation)
- Alignment with `brief.md` and steering product context
- Terminology continuity (`Boundary Candidates` → explicit boundaries)
- Fix `requirements.md` directly when issues are local and unambiguous
- Record all autonomous decisions in report `## Decisions`

## Out of Scope

- EARS mechanical syntax checks (`requirements-review-gate` in `/sdd-spec-requirements`)
- Testability deep-dive: abnormal-flow AC coverage, boundary values, NFR measurability (Pass A QA / `qa-requirements-checklist.md`)
- Security deep-dive (`/sdd-validate-requirements --only sec`)
- Gap-domain audit: brief traceability matrix, cross-spec consistency, template conformance (`/sdd-validate-requirements --only final`)
- User dialogue

## Checks

1. Every requirement objective is backed by ACs that express it; no orphan objectives, no AC with no parent intent (verifiability deep-dive is qa domain)
2. No contradictory ACs within or across requirements
3. Ambiguous terms resolved or flagged
4. Scope boundaries explicit where misread risk exists
5. Non-functional requirements stated at user-observable level (not implementation)
6. Requirement IDs and AC IDs follow project conventions
7. Changes to `requirements.md` preserve EARS keyword English (When, If, While, Where, shall)
8. Every `requirements.md` edit is recorded as a `## Reflected Fixes` row (finding → target section → summary) — free-text fix claims are treated as missing downstream

## Findings Severity

Use the shared scale in `../../sdd-validate-shared/contract.md` (Critical / Major / Minor):

- **Critical**: contradiction, missing core capability, scope hole → `NO-GO`
- **Major**: ambiguity requiring an assumption → fix into `requirements.md` (list in `## Reflected Fixes`) + record the assumption in `## Decisions`
- **Minor**: wording improvement → fix inline if safe; note in `## Reflected Fixes`

## NO-GO Triggers

- Unresolvable scope conflict between brief and requirements
- Missing core capability with no reasonable default
- Requirements structurally broken (cannot patch without regeneration)
