# Task Generation Rules

## Core Principles

### 1. Natural Language Descriptions
Focus on capabilities and outcomes, not code structure.

**Describe**:
- What functionality to achieve
- Business logic and behavior
- Features and capabilities
- Domain language and concepts
- Data relationships and workflows

**Avoid**:
- File paths and directory structure
- Function/method names and signatures
- Type definitions and interfaces
- Class names and API contracts
- Specific data structures

**Rationale**: Implementation details (files, methods, types) are defined in design.md. Tasks describe the functional work to be done.

### 2. Task Ordering Principle

**Order implies dependency**: Task N implicitly depends on all tasks before it. This is the primary dependency mechanism.

**Tasks must follow this phase order**:
1. **Foundation**: Environment setup, test infrastructure, shared utilities, database schema, configuration
2. **Core**: Primary feature implementation (parallel-capable tasks grouped here)
3. **Integration**: Wiring components together, cross-boundary connections
4. **Validation**: E2E tests, edge cases, regression checks

**Rationale**: Foundation work unblocks everything else. Placing setup tasks early prevents downstream blocking. Core tasks can often run in parallel because foundation is already complete.

### 3. Task Integration & Progression

**Every task must**:
- Build on previous outputs (no orphaned code)
- Connect to the overall system (no hanging features)
- Progress incrementally (no big jumps in complexity)
- Respect architecture boundaries from design.md Boundary Map / Persistent References (and related `docs/architecture/**` paths when listed)
- Honor public-surface contracts from `docs/contracts/**` (via design.md Persistent References and task `_Contracts:` paths). Treat `design.md` contract text as excerpts/summaries only — not the long-lived authority
- Use major task summaries sparingly—omit detail bullets if the work is fully captured by child tasks.

**End with integration tasks** to wire everything together.

### 4. Dependency Declaration

**Default**: Sequential ordering handles most dependencies (task N depends on tasks before it).

**Explicit declaration required when**:
- A task depends on a specific task in a different major-task group (cross-boundary)
- The dependency is non-obvious from ordering alone
- A task can skip ahead of its position (declared via `(P)`) but still needs specific prior work

**Format**: `_Depends: 1.2, 2.3_` — placed alongside `_Requirements:_` in task detail sections.

**Do not over-annotate**: If a task simply depends on the task directly before it, ordering alone is sufficient.

### 5. Boundary Scope

**Each task should declare its component boundary** using design.md component/module names:
- `_Boundary: AuthService_` or `_Boundary: API Layer, UserRepository_`
- Helps validate parallel safety: tasks with non-overlapping boundaries are parallel candidates
- Helps agents understand scope: what to touch and what not to touch

**When to use**: Required for tasks marked `(P)` to validate parallel safety. Omit for sequential tasks where scope is obvious from the description.

**Boundary rule**:
- Each executable task should stay within a single responsibility boundary
- If work must cross boundaries, make it an explicit integration task rather than a normal implementation task
- Do not hide cross-boundary coordination inside a task that appears local

### 5.5 Wave Annotation (Phase order)

**Every executable sub-task must declare its phase wave** in the detail section:

```markdown
- _Wave: 1_
```

**Rules**:
- `_Wave:_` is a **phase-order** annotation (Foundation → Core → Integration → Validation). It is **not** the `sdd-impl` dispatch unit.
- **`sdd-impl` dispatches by major number** (`1`, `2`, …): all remaining executable work under major `N` goes to one implementer. Same Wave ≠ same implementer batch.
- Waves still increase in dependency order (later phases must not start before earlier phases they need)
- Integration and Validation phases get their own Waves **and their own majors** — do not mix them with Foundation/Core implementation under the same major
- `(P)` work with different `_Boundary:_` must be **different majors** (do not park them as sibling `N.M` under one parent) — so `sdd-impl` can parallel-dispatch those majors when dependencies are ready
- Sub-tasks under the same major share (or omit) `_Boundary:_` consistently; mixed-boundary `(P)` siblings under one major will **not** be split into parallel implementers

**How to assign Waves** (align with Task Ordering Principle):
1. Wave(s) for Foundation
2. Wave(s) for Core (split by **major** / boundary / dependency; `(P)` + different boundary → **separate majors**)
3. Separate Wave(s) **and majors** for Integration
4. Separate Wave(s) **and majors** for Validation

**Sizing vs dispatch**: Keep human-readable sub-tasks small (1–3 hours). Do **not** coarsen the hierarchy just to match dispatch — `sdd-impl` groups all `N.M` under major `N` into one implementer.

### 6. Flexible Task Sizing

**Guidelines**:
- **Major tasks**: As many sub-tasks as logically needed (group by cohesion)
- **Sub-tasks**: 1-3 hours each, 3-10 details per sub-task
- Balance between too granular and too broad
- Fine-grained sub-tasks and **major-level** dispatch coexist: size sub-tasks for human clarity; `sdd-impl` runs one implementer per major

**Don't force arbitrary numbers** - let logical grouping determine structure.

### 7. Requirements Mapping

**End each task detail section with**:
- `_Requirements: X.X, Y.Y_` listing **only numeric requirement IDs** (comma-separated). Never append descriptive text, parentheses, translations, or free-form labels.
- For cross-cutting requirements, list every relevant requirement ID. All requirements MUST have numeric IDs in requirements.md. If an ID is missing, stop and correct requirements.md before generating tasks.
- Reference components/interfaces from design.md when helpful; for public surfaces annotate `_Contracts: docs/contracts/<file>.md_` from Persistent References (path form; optional but recommended — authoritative detail lives in that contract file, not in design.md alone)

### 7.5 Observable Completion

**Each executable task must include at least one detail bullet that describes the observable completed state**:
- Phrase it as a deliverable, runtime behavior, persisted state, UI state, endpoint behavior, test result, or integration outcome
- Avoid vague bullets like "implement support", "wire things up", or "handle logic" unless paired with a concrete observable result
- Prefer making one detail bullet clearly answer: "What will be true when this task is done?"
- Keep this within the existing task body; do not add extra bookkeeping fields

### 8. Code-Only Focus

**Include ONLY**:
- Coding tasks (implementation)
- Testing tasks (unit, integration, E2E)
- Technical setup tasks (infrastructure, configuration)

**Exclude**:
- Deployment tasks
- Documentation tasks
- User testing
- Marketing/business activities

## Task Plan Review Gate

Before writing `tasks.md`, review the draft task plan and repair local issues until the plan passes or a true spec gap is discovered.

### Coverage Review

- Every requirement ID from `requirements.md` must appear in at least one task.
- Every design component, interface/contract, integration point, runtime prerequisite, and validation concern from `design.md` must be represented by at least one task.
- If coverage is missing because the task plan is incomplete, repair the draft tasks and review again.
- If coverage cannot be added cleanly because requirements or design are ambiguous, contradictory, or underspecified, stop and return to the requirements/design phase instead of papering over the gap in `tasks.md`.

### Executability Review

- Every sub-task must be executable as written, usually within 1-3 hours.
- Every sub-task must produce a verifiable deliverable (behavior, artifact, endpoint, UI state, config, migration, test, or integration result).
- Every executable sub-task must include at least one detail bullet that states the observable completion condition.
- Every executable sub-task must include a `_Wave: N_` annotation (detect missing Waves and add them before writing `tasks.md`).
- Split tasks that combine multiple independently verifiable outcomes.
- Split tasks that combine multiple responsibility boundaries unless they are explicit integration tasks.
- If many tasks require broad `_Boundary:_` scopes or repeated cross-boundary coordination, stop and return to design or roadmap decomposition instead of forcing the spec through task generation.
- Merge or collapse tasks that are too small, bookkeeping-only, or not meaningful execution units.
- Make implicit prerequisites explicit as preceding tasks.
- Re-check `_Depends:_`, `_Boundary:_`, `_Wave:_`, and `(P)` markers after edits so concurrency claims and **major** batches still match the design boundaries and dependency graph.
- Reject plans where Integration/Validation tasks share a **major** with Foundation/Core implementation, or where `(P)` tasks with different `_Boundary:_` share the same **major**.
- Reject `(P)` on any **major** that cannot safely parallel-dispatch at implementation time (shared incomplete Depends with a peer major, overlapping `_Boundary:_` / File Structure paths, or missing `_Boundary:_` when peer majors would run concurrently). Remove the marker or split into separate majors until the execution contract holds.

### Review Loop

- Run the review gate on the draft task plan before writing `tasks.md`.
- If issues are task-plan-local, repair the draft and re-run the review gate.
- Keep the loop bounded: no more than 2 review-and-repair passes before escalating a real spec gap.
- Write `tasks.md` only after the review gate passes.

### Optional Test Coverage Tasks

- When the design already guarantees functional coverage and rapid MVP delivery is prioritized, mark purely test-oriented follow-up work (e.g., baseline rendering/unit tests) as **optional** using the `- [ ]*` checkbox form.
- Only apply the optional marker when the sub-task directly references acceptance criteria from requirements.md in its detail bullets.
- Never mark implementation work or integration-critical verification as optional—reserve `*` for auxiliary/deferrable test coverage that can be revisited post-MVP.

## Task Hierarchy Rules

### Maximum 2 Levels
- **Level 1**: Major tasks (1, 2, 3, 4...)
- **Level 2**: Sub-tasks (1.1, 1.2, 2.1, 2.2...)
- **No deeper nesting** (no 1.1.1)
- If a major task would contain only a single actionable item, collapse the structure and promote the sub-task to the major level (e.g., replace `1.1` with `1.`).
- When a major task exists purely as a container, keep the checkbox description concise and avoid duplicating detailed bullets—reserve specifics for its sub-tasks.

### Sequential Numbering
- Major tasks MUST increment: 1, 2, 3, 4, 5...
- Sub-tasks reset per major task: 1.1, 1.2, then 2.1, 2.2...
- Never repeat major task numbers

### Parallel Analysis (default)
- Assume parallel analysis is enabled unless explicitly disabled (e.g. `--sequential` flag).
- **Policy: `(P)` is an execution contract** — it promises `sdd-impl` may parallel-dispatch this **major** with other ready `(P)` majors when boundaries, Depends, and paths are disjoint. Do not mark `(P)` for documentation-only or "looks independent" notes that cannot actually run in parallel.
- `(P)` means: this **major** has no dependency on its immediately preceding peer majors and **may be dispatched concurrently** with them at implementation time. Sub-tasks under the same major always share one implementer; do not use `(P)` on `N.M` siblings expecting them to run as separate agents.
- Identify **majors** that can run concurrently when **all** conditions hold:
  - No data dependency on other pending majors
  - No shared file or resource contention
  - No prerequisite review/approval from another major
  - `_Boundary:_` annotations confirm non-overlapping component scopes
- Foundation-phase majors are rarely `(P)` — they establish shared prerequisites.
- Core-phase majors are the primary candidates for `(P)` since foundation is already complete.
- Validate that identified parallel majors operate within separate boundaries defined in the Architecture Pattern & Boundary Map.
- Confirm API/event contracts from Persistent References / `_Contracts:` paths (`docs/contracts/**`) do not overlap in ways that cause conflicts; use design.md excerpts only as orientation.
- `(P)` majors with cross-boundary dependencies must declare `_Depends: X.X_` explicitly on the relevant children.
- Append `(P)` immediately after the **major** number for each parallel-capable major:
  - Example: `- [ ] 2. (P) Build auth service`
  - Put different-boundary `(P)` work in **different majors**, not as `(P)` siblings under one parent.
  - Skip marking container-only major lines if the parallel contract is already on that major's executable children consistently — prefer the major-level marker.
- If sequential mode is requested, omit `(P)` markers entirely.
- Explicitly call out dependencies that prevent `(P)` even when majors look similar. **Never attach `(P)` when parallel dispatch would be unsafe.**

### Checkbox Format
```markdown
- [ ] 1. Foundation: environment and test infrastructure setup
- [ ] 1.1 Sub-task description
  - Detail item 1
  - Detail item 2
  - Observable completion condition
  - _Requirements: X.X_
  - _Wave: 1_

- [ ] 2. (P) Core feature A
- [ ] 2.1 Sub-task description
  - Detail items...
  - Observable completion condition
  - _Requirements: Y.Y_
  - _Boundary: AuthService_
  - _Wave: 2_

- [ ] 3. (P) Core feature B
- [ ] 3.1 Sub-task description
  - Detail items...
  - Observable completion condition
  - _Requirements: Z.Z_
  - _Boundary: UserRepository_
  - _Wave: 2_

- [ ] 4. Integration and wiring
- [ ] 4.1 Sub-task description
  - Detail items...
  - Observable completion condition
  - _Depends: 2.1, 3.1_
  - _Requirements: W.W_
  - _Wave: 3_
```

## Requirements Coverage

**Mandatory Check**:
- ALL requirements from requirements.md MUST be covered
- Cross-reference every requirement ID with task mappings
- If gaps found: Return to requirements or design phase
- No requirement should be left without corresponding tasks

Use `N.M`-style numeric requirement IDs where `N` is the top-level requirement number from requirements.md (for example, Requirement 1 → 1.1, 1.2; Requirement 2 → 2.1, 2.2), and `M` is a local index within that requirement group.

Document any intentionally deferred requirements with rationale.
