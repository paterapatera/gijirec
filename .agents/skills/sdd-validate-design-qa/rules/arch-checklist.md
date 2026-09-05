# Design Architecture Validate Checklist

## Mission

SOLID, loose coupling, and extensibility simulation. Architect administrator role. Runs after QA; reads QA-updated `design.md`.

Severity vocabulary, `## Reflected Fixes`, and per-check evidence discipline: shared contract (`../sdd-validate-shared/contract.md`). Every check below gets an explicit result — pass / finding / `N/A` with a one-line reason. No silent skips.

## In Scope

- Single Responsibility, dependency direction, interface boundaries — on **design boundaries + Persistent References contracts only**
- Coupling/cohesion assessment against referenced contracts (not the full contracts tree)
- Data ownership and consistency boundaries
- Reuse of existing assets (no reinvention of shared components/utilities/patterns)
- File Structure Plan: component-to-file mapping granularity
- Alignment with steering `tech.md` / `structure.md`
- Extension scenario walkthrough with explicit pass criteria
- Contract sync vs Persistent References (`Mode: modify` / `reference`)
- Reflect architectural fixes into `design.md`; list every edit in `## Reflected Fixes`

## Out of Scope

- Edge-case checklist (`/sdd-validate-design-qa`)
- Security threat modeling (`/sdd-validate-design-qa --only sec`)
- User dialogue
- Bulk Read of unlisted `docs/contracts/**`, all ADRs, or full-project architecture audit

## Anti-Pattern Scan

Explicitly scan the design for each anti-pattern and record the result:

1. **God object/module** — one component owning unrelated responsibilities
2. **Circular dependency** — components or layers referencing each other, directly or transitively
3. **Leaky abstraction** — interface exposing implementation details (storage schema, vendor types) to consumers
4. **Shared mutable state without a single owner** — two components writing the same data with no arbitration
5. **Data ownership conflict** — design's data ownership contradicts the spec's Boundary Commitments or another spec's ownership (roadmap-dependent specs)
6. **Speculative abstraction** — interfaces/adapters that exist only for hypothetical future scope with no requirement backing

## Checks

1. Components/modules have clear responsibilities; no god objects
2. Dependencies point inward; no forbidden layer violations per steering
3. Interfaces stable; implementations swappable where requirements demand flexibility
4. Shared state minimized; boundaries explicit; every mutable datum has exactly one owning component
5. Data model transactional/consistency boundaries match component boundaries (no transaction spanning ownership seams without an explicit pattern)
6. Complexity proportionate to requirements: every component, interface, and layer traces to a requirement or an explicit extensibility decision — anything else is a finding (speculative abstraction)
7. **Reuse of existing assets**: for each new component/utility/pattern the design introduces, confirm no functionally equivalent asset already exists — check steering `structure.md` shared locations, other specs in `docs/specs/roadmap.md`, and (brownfield) the gap analysis / discovery findings in `research.md`. Reinvention without a recorded justification is a **Major** finding: either switch the design to the existing asset or record the justification in `design.md` and the report `## Decisions`. Ownership *conflict* is anti-pattern 5; this check catches *duplication without conflict*.
8. **File Structure Plan mapping**: the component-to-file mapping keeps one clear responsibility per file — no god file absorbing multiple unrelated components, no single component smeared across unrelated files, shared files identified as intentional seams (not accidental hotspots). A clean component diagram does not pass this check by itself; judge the physical layout.
9. **Important decision → ADR**: If the design changes dependency direction/ownership boundaries, breaks a public contract, makes a major adopt/reject tech choice, or touches Revalidation Triggers, Persistent References must list a corresponding new/related ADR under `docs/architecture/adr/` (or an explicit justification that no ADR is needed). Missing ADR for such a decision is a **Major** finding. Do not require ADRs for local naming or implementation detail. Read only listed / related ADRs — never all ADRs.
10. **Contract sync (Persistent References)**: For every Persistent References path with `Mode: modify`, confirm the file exists and already reflects this feature's boundary / public-surface change. Record Reviewed contract paths and ADR paths read for the unified report `## Reviewed Scope`. Set **Contract sync**:
    - `OK` — modify paths present and aligned; or reference-only / **No contract changes** when the design states no surface change
    - `MISSING` — `Mode: modify` (or required new public-surface) path absent → **NO-GO** or **Major**
    - `DRIFT` — external canonical contradicts design summary / Boundary Commitments → Finding / **NO-GO** factor
11. Extension simulation — walk through **two** scenarios:
   - one plausible additive change derived from requirements Non-Goals or roadmap
   - one change to an external dependency (version bump, API change, replacement)

   **Pass criteria** (record per scenario; optimistic prose without these is not a pass):
   - the change stays within component boundaries the design already declares — name which components/files absorb it
   - no contract change is forced on consumers unrelated to the change
   - no dependency direction violation is needed to implement it

## design.md Reflection

Before `GO`:
- Split conflated responsibilities in design sections
- Add boundary/commitment clarifications
- Document dependency direction and data ownership where unclear
- Record every edit as a `## Reflected Fixes` row (finding → design.md section → summary)

## Severity (per shared contract)

- **Critical**: fundamental layering violation; unresolvable data ownership conflict; coupling that blocks a stated extensibility requirement; external canonical vs design summary contradiction that invalidates the architecture (`Contract sync: DRIFT` when blocking)
- **Major**: anti-pattern with a concrete fix (split, invert dependency, assign owner); reinvention of an existing asset without justification; god file / smeared component in the File Structure Plan; important boundary/contract/tech decision without ADR → reflect it; `Mode: modify` path missing or stale (`Contract sync: MISSING`) when a concrete fix path exists
- **Minor**: naming/boundary wording clarity

## NO-GO Triggers

- Fundamental layering violation
- Tight coupling that blocks stated extensibility requirements
- Design introduces ownership conflicts with requirements boundaries or dependent specs
- `Contract sync: MISSING` or `DRIFT` that leaves public surface / boundary authority unresolved
