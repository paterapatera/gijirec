# Design Security Validate Checklist

## Mission

Threat model, credentials, and PII at design level. Security administrator role. Runs after arch; reads arch-updated `design.md`.

Severity vocabulary, `## Reflected Fixes`, and per-check evidence discipline: shared contract (`../sdd-validate-shared/contract.md`). Every check below gets an explicit result — pass / finding / `N/A` with a one-line reason. No silent skips.

## In Scope

- Threat enumeration for new/changed surfaces (mandatory threat table)
- AuthN/AuthZ design vs requirements
- Secrets management approach (no literal secrets in design)
- PII storage, transit, logging, retention design — including the design's Observability section (PII in logs/metrics)
- Migration/rollback data protection (design's Operational Readiness section)
- Availability abuse: DoS, rate limiting, resource-exhaustion abuse
- Audit logging of security-relevant events
- Dependency/supply-chain exposure of new external dependencies
- Input validation and output encoding at boundaries
- Reflect security fixes into `design.md`; list every edit in `## Reflected Fixes`

## Out of Scope

- QA edge-case checklist (`/sdd-validate-design-qa`)
- SOLID/architecture review (Pass A Arch)
- User dialogue

## Threat Table (mandatory format)

Enumerate threats per STRIDE against each new/changed surface. Include the table in the report Evidence; free-text threat narration alone is a finding against this review itself.

| # | Surface | Threat (STRIDE) | Impact | Mitigation in design / Accepted risk (Decision ref) |
| - | ------- | --------------- | ------ | --------------------------------------------------- |

Every row must end in a design mitigation (with section ref) or an accepted-risk entry in `## Decisions`. Rows with neither are **Critical**.

## Checks

1. Trust boundaries diagrammed or described; data classification noted
2. Authentication/session/token flow defined where required
3. Authorization checks at correct layer; no security-by-obscurity
4. PII fields identified; encryption/masking/logging rules stated — **explicitly review the Observability section**: no PII in logs/metrics without a masking rule
5. External calls: TLS, credential injection, least privilege
6. DoS/abuse: rate limiting, quotas, or an explicit accepted risk for every publicly reachable or unauthenticated surface
7. Audit logging: security-relevant events (authn failures, authz denials, privilege changes, data export) are logged per design, tamper-considerations noted
8. Migration/rollback paths (Operational Readiness section) do not expose or orphan sensitive data; rollback does not resurrect revoked access
9. New external dependencies: supply-chain note (source trust, pinning/lockfile expectation, permission scope) or `N/A` with reason
10. Threat table complete (§ above); all requirements-sec decisions honored

## design.md Reflection

Before `GO`:
- Add missing security sections/controls
- Clarify data handling at boundaries (including logging/migration paths)
- Document deferred risks with rationale in report `## Decisions`
- Record every edit as a `## Reflected Fixes` row (finding → design.md section → summary)

## Severity (per shared contract)

- **Critical**: sensitive data path undocumented; threat with neither mitigation nor accepted-risk decision; contradiction with requirements-sec decisions
- **Major**: missing control with a concrete design fix (masking rule, rate limit, audit event) → reflect it
- **Minor**: wording/classification clarity

## NO-GO Triggers

- Sensitive data path undocumented
- Critical threat without mitigation or accepted-risk decision
- Design contradicts requirements-sec decisions
