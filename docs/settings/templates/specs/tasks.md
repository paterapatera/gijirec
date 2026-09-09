# Implementation Plan

## Task Format Template

Use whichever pattern fits the work breakdown:

### Major task only
- [ ] {{NUMBER}}. {{TASK_DESCRIPTION}}{{PARALLEL_MARK}}
  - {{DETAIL_ITEM_1}} *(Include details only when needed. If the task stands alone, omit bullet items.)*
  - _Requirements: {{REQUIREMENT_IDS}}_
  - _Wave: {{N}}_ *(Required when this major task is itself executable. Phase order only; `sdd-impl` dispatches by major number.)*

### Major + Sub-task structure
- [ ] {{MAJOR_NUMBER}}. {{MAJOR_TASK_SUMMARY}}{{PARALLEL_MARK}}
- [ ] {{MAJOR_NUMBER}}.{{SUB_NUMBER}} {{SUB_TASK_DESCRIPTION}}
  - {{DETAIL_ITEM_1}}
  - {{DETAIL_ITEM_2}}
  - {{OBSERVABLE_COMPLETION_ITEM}} *(At least one detail item should state the observable completion condition for this task.)*
  - _Requirements: {{REQUIREMENT_IDS}}_ *(IDs only; do not add descriptions or parentheses.)*
  - _Boundary: {{COMPONENT_NAMES}}_ *(Only for (P) tasks. Omit when scope is obvious.)*
  - _Design: D-{{COMPONENT_NAME}}_ *(Optional. Links to `#### Name {#D-Name}` in design.md for excerpt lookup; omit when Boundary name alone is enough.)*
  - _Depends: {{TASK_IDS}}_ *(Only for non-obvious cross-boundary dependencies. Most tasks omit this.)*
  - _Wave: {{N}}_ *(Required on every executable sub-task. Phase order only — `sdd-impl` dispatches by major, not by Wave.)*

> **Parallel marker (`(P)` execution contract)**: Append ` (P)` on the **major** when `sdd-impl` may parallel-dispatch this major with other ready `(P)` majors (disjoint `_Boundary:_`, closed Depends, non-overlapping paths). Put different-boundary `(P)` work in different majors. Omit when unsafe or when running in `--sequential` mode. Never use `(P)` as an informational-only note. Sub-task `(P)` does **not** spawn a separate implementer.
>
> **Wave annotation**: Assign `_Wave: N_` so Foundation → Core → Integration → Validation increase by dependency. Keep Integration/Validation in their own majors. `(P)` work with different `_Boundary:_` must be different majors (enables cross-major parallel dispatch). Wave is not the dispatch unit.
>
> **Optional test coverage**: When a sub-task is deferrable test work tied to acceptance criteria, mark the checkbox as `- [ ]*` and explain the referenced requirements in the detail bullets.
