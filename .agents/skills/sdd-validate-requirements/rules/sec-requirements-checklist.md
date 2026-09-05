# Security Requirements Validate Checklist

## Mission

Requirements-phase security review. Security administrator role. No user dialogue.

## In Scope

- AuthN/AuthZ expectations at requirements level
- PII/sensitive data identification and classification; handling requirements
- Abuse cases: misuse of in-scope capabilities by untrusted or over-privileged actors
- Compliance constraints from steering
- Trust boundaries and data flows (conceptual)
- Security-relevant AC gaps
- Record recommendations with adopt/defer rationale in `## Decisions`; all edits as `## Reflected Fixes` rows

## Out of Scope

- Functional scope decisions (PO validate)
- Testability review: AC verifiability, NFR measurability (Pass A QA)
- Threat model detail at design level (`/sdd-validate-design-qa --only sec`)
- Implementation-specific controls (design phase)
- User dialogue

## Checks

1. Authentication/authorization needs stated or explicitly N/A with rationale
2. PII/sensitive data identified and classified (public / internal / sensitive / regulated); collection, use, retention expectations present
3. External integrations note trust boundary expectations
4. Unwanted-behavior ACs cover security-relevant failure modes where applicable
5. Abuse cases considered: for each capability, what happens when an untrusted actor misuses it (enumeration, injection of hostile input, quota/limit abuse) — expectations stated at user-observable level or explicitly deferred to design threat model with rationale
6. Steering security constraints reflected or deviation documented in `## Decisions`
7. Secrets/credentials never required in requirements text as literal values
8. Every `requirements.md` edit (adopted recommendation) is recorded as a `## Reflected Fixes` row — Pass B (`/sdd-validate-requirements --only final` or full run) mechanically verifies them; free-text fix claims are treated as missing

## Findings Severity

Use the shared scale in `../../sdd-validate-shared/contract.md` (Critical / Major / Minor):

- **Critical**: undocumented regulated/sensitive-data handling, critical auth gap, contradiction with steering security mandates → `NO-GO`
- **Major**: security-relevant AC gap or missing trust-boundary expectation → adopt (update `requirements.md`, list in `## Reflected Fixes`) or defer with risk acceptance in `## Decisions`
- **Minor**: clarification of security wording → fix inline if safe; note in `## Reflected Fixes`

## Decisions Format

For each security recommendation:

- **Recommendation**: what to add or clarify
- **Disposition**: adopted (requirements updated) | deferred (with risk acceptance reason)
- **Approval impact**: what fixing this in requirements commits downstream to

## NO-GO Triggers

- Undocumented handling of regulated/sensitive data
- Critical auth gap with no safe default assumption
- Contradiction with steering security mandates
