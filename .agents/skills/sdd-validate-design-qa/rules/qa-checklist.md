# Design QA Validate Checklist

## Mission

Abnormal flows and edge-case coverage check. Quality administrator role. Reflect fixes into `design.md`.

Severity vocabulary, `## Reflected Fixes`, and per-check evidence discipline: shared contract (`../sdd-validate-shared/contract.md`). Every check below gets an explicit result — pass / finding / `N/A` with a one-line reason. No silent skips.

## In Scope

- Error paths, boundary conditions, empty/null states
- Concurrency/race scenarios
- Idempotency and retry behavior
- Input validation edge cases
- Failure-mode handling aligned with requirements Unwanted Behavior ACs
- Update `design.md` for gaps; list every edit in `## Reflected Fixes`

## Out of Scope

- Architecture/SOLID review (Pass A Arch / `arch-checklist.md`)
- Threat model (`/sdd-validate-design-qa --only sec`)
- User dialogue

## Edge-Case Derivation (do not rely on requirements alone)

Requirements ACs are the floor, not the ceiling. Derive additional edge cases independently with these techniques, then check the design against both sets:

1. **Input domain analysis** — for each external input: empty/null, boundary values, malformed, oversized, duplicate submissions
2. **State-transition enumeration** — for each stateful entity: list states, probe invalid/impossible transitions and re-entrant events
3. **Dependency failure modes** — for each external dependency: timeout, partial failure, out-of-order response, unavailability at startup
4. **Resource exhaustion** — pagination limits, queue/buffer growth, disk/memory ceilings, long-running operations
5. **Time & ordering** — clock skew, retries crossing state changes, concurrent writers to the same data

## Checks

1. Map each Unwanted Behavior AC to design coverage (table in Evidence)
2. Map derived edge cases (§ above) to design coverage; uncovered ones become findings
3. Happy path does not silently assume impossible preconditions
4. Error handling specified for every external dependency failure mode
5. State-driven requirements have invalid-transition handling
6. Concurrency/race handling stated for shared state and duplicate triggers — `N/A` only with reason (e.g. single-writer by design)
7. Pagination/limits/timeout edge cases addressed where inputs or NFRs imply them
8. Testing Strategy section covers edge-case verification for the failure modes found above (the design template always has this section — its absence is itself a finding)

## design.md Reflection

Before `GO`, apply actionable fixes to `design.md`:
- Add missing error/edge handling sections
- Clarify ambiguous behavior under failure
- Do not introduce architecture decisions outside design scope
- Record every edit as a `## Reflected Fixes` row (finding → design.md section → summary)

## Severity (per shared contract)

- **Critical**: user-facing failure mode with no design response; design contradicts requirements edge-case ACs
- **Major**: derived edge case uncovered with a concrete fix available → reflect it
- **Minor**: clarity/wording of failure behavior

## NO-GO Triggers

- Critical user-facing failure mode with no design response
- Design contradicts requirements edge-case ACs
- Repeated unfixable ambiguity after one repair pass
