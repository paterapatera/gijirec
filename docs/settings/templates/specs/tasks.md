# Implementation Plan

## Task Format Template

Use whichever pattern fits the work breakdown:

### Major task only
- [ ] {{NUMBER}}. {{TASK_DESCRIPTION}}{{PARALLEL_MARK}}
  - {{DETAIL_ITEM_1}} *(Include details only when needed. If the task stands alone, omit bullet items.)*
  - _Requirements: {{REQUIREMENT_IDS}}_
  - _Wave: {{N}}_ *(Required when this major task is itself executable.)*

### Major + Sub-task structure
- [ ] {{MAJOR_NUMBER}}. {{MAJOR_TASK_SUMMARY}}
- [ ] {{MAJOR_NUMBER}}.{{SUB_NUMBER}} {{SUB_TASK_DESCRIPTION}}{{SUB_PARALLEL_MARK}}
  - {{DETAIL_ITEM_1}}
  - {{DETAIL_ITEM_2}}
  - {{OBSERVABLE_COMPLETION_ITEM}} *(At least one detail item should state the observable completion condition for this task.)*
  - _Requirements: {{REQUIREMENT_IDS}}_ *(IDs only; do not add descriptions or parentheses.)*
  - _Boundary: {{COMPONENT_NAMES}}_ *(Only for (P) tasks. Omit when scope is obvious.)*
  - _Design: D-{{COMPONENT_NAME}}_ *(Optional. Links to `#### Name {#D-Name}` in design.md for excerpt lookup; omit when Boundary name alone is enough.)*
  - _Depends: {{TASK_IDS}}_ *(Only for non-obvious cross-boundary dependencies. Most tasks omit this.)*
  - _Wave: {{N}}_ *(Required on every executable sub-task. Same N = same dispatch-batch candidate.)*

> **Parallel marker (`(P)` execution contract)**: Append ` (P)` only when `sdd-impl` may parallel-dispatch this task's Wave with other ready `(P)` peers (disjoint `_Boundary:_`, closed Depends, non-overlapping paths). Omit when unsafe or when running in `--sequential` mode. Never use `(P)` as an informational-only note.
>
> **Wave annotation**: Assign `_Wave: N_` so Foundation → Core → Integration → Validation increase by dependency. Keep Integration/Validation Waves separate from implementation Waves. `(P)` tasks with different `_Boundary:_` must not share a Wave number (enables cross-Wave parallel dispatch).
>
> **Optional test coverage**: When a sub-task is deferrable test work tied to acceptance criteria, mark the checkbox as `- [ ]*` and explain the referenced requirements in the detail bullets.
