# Parallel Task Analysis Rules

## Purpose
Provide a consistent way to identify implementation tasks that can be safely executed in parallel while generating `tasks.md`.

## Execution Contract

**Policy: `(P)` is an execution contract**, not an informational note. Marking `(P)` promises that `sdd-impl` **may** parallel-dispatch this **major** with other ready `(P)` majors when:
- `_Boundary:_` values differ (non-overlapping components)
- Dependencies are mutually closed (no shared incomplete `_Depends:_`)
- Planned change paths (File Structure / Boundary) do not overlap

If those conditions cannot hold, **do not** attach `(P)`.

## Relationship to Task Ordering

`(P)` means: this **major** has no dependency on its immediately preceding peer majors and **may be dispatched concurrently** with them at implementation time. The Task Ordering Principle (see tasks-generation.md) ensures Foundation-phase majors run first, making Core-phase majors the primary `(P)` candidates. Different-boundary `(P)` work must be **different majors** so parallel dispatch is across ready majors. Sub-tasks under one major always share one implementer.

## When to Consider Tasks Parallel
Only mark a **major** as parallel-capable when **all** of the following are true:

1. **No data dependency** on pending peer majors.
2. **No conflicting files or shared mutable resources** are touched.
3. **No prerequisite review/approval** from another major is required beforehand.
4. **Foundation work complete**: Environment/setup work needed by this major is already satisfied by earlier Foundation-phase majors.
5. **Non-overlapping boundaries**: `_Boundary:_` annotations confirm the majors operate on separate components.

## Marking Convention
- Append `(P)` immediately after the **major** numeric identifier for each qualifying major.
  - Example: `- [ ] 2. (P) Build background worker for emails`
- Put different-boundary `(P)` work in different majors. Do not mark sibling `N.M` under one parent as `(P)` expecting them to run as separate agents.
- If sequential execution is requested (e.g. via `--sequential` flag), omit `(P)` markers entirely.
- Keep `(P)` **outside** of checkbox brackets to avoid confusion with completion state.

## Grouping & Ordering Guidelines
- Keep work that belongs to the same theme and **same boundary** under the same major (one implementer).
- List obvious prerequisites or caveats in the detail bullets (e.g., "Requires schema migration from 1.2").
- When two majors look similar but are not parallel-safe, call out the blocking dependency explicitly.
- Prefer the `(P)` marker on the major line. Skip marking container-only majors only when the major-level contract is already clear from a single executable child.

## Quality Checklist
Before marking a major with `(P)`, ensure you have:

- Verified that running this major concurrently will not create merge or deployment conflicts.
- Confirmed `_Boundary:_` annotations show non-overlapping component scopes versus other `(P)` majors.
- Captured any shared state expectations in the detail bullets.
- Confirmed that the major can be tested independently of its `(P)` peers.
- Added `_Depends: X.X_` if this `(P)` major still requires specific prior work from a different major.
- Placed different-boundary `(P)` work in **distinct majors**.

If any check fails, **do not** mark the major with `(P)` and explain the dependency in the task details.
