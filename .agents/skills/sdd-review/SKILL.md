---
name: sdd-review
description: Review a batch or task implementation against approved specs, task boundaries, and verification evidence. Use after an implementer finishes a batch/selection, after remediation, or before the completion gate that marks tasks [x].
---

# sdd-review

<background_information>
This skill performs adversarial judgment review for a **batch** (sdd-impl autonomous / wave / strict) or a **selection** (direct / manual / standalone). It verifies that the implementation is real, complete, bounded, aligned with approved requirements and design, and supported by mechanical verification evidence.

Boundary terminology continuity:
- discovery identifies `Boundary Candidates`
- design fixes `Boundary Commitments`
- tasks constrain execution with `_Boundary:_`
- review rejects concrete `Boundary Violations`
</background_information>

<instructions>
## When to Use

- After an implementer reports `READY_FOR_REVIEW` for a **batch** or **selection** (not once per unmarked sub-task inside an open batch)
- After remediation for a rejected review
- Before the caller runs the completion gate that marks those tasks `[x]`
- Before accepting a batch/selection into feature-level validation

**Batch-local vs selection/standalone:**

- **Batch-local** (`sdd-impl` wave/strict): one verdict for the whole batch. Parent already ran mechanical checks and passes `MECHANICAL_RESULTS` plus `## Spec Excerpts (authoritative for this batch)`. Adopt the mechanical baseline; concentrate on Judgment Checks unless results are missing or suspicious. Judge against **Spec Excerpts** (Requirements / Design / Contracts when present) — do **not** default to full-file Read of `requirements.md` / `design.md` / architecture.
- **Selection / direct** (`sdd-impl` direct or manual multi-task): same judgment once at selection end; parent should supply excerpts or the equivalent scoped sections used for Task Briefs.
- **Standalone task-local** (no parent mechanical / no excerpts): may run mechanical checks in-process and read only the **referenced** requirement/design sections (not the whole files by default).

Do not weaken Judgment Checks in any mode. Do not use this skill to invent missing requirements or silently reinterpret the spec.

## Inputs

Provide:
- Task ID list (batch/selection) or single task ID, plus exact task text from `tasks.md`
- Relevant requirement section numbers
- Relevant design section numbers
- Prefer `## Spec Excerpts (authoritative for this batch)` with `### Requirements`, `### Design`, and when related `### Contracts (authoritative for touched surfaces)` when available (authoritative for judgment)
- Spec file paths (`requirements.md`, `design.md`, optionally `tasks.md`, `docs/contracts/...`) as repository location only — **not** a directive to open them in full when excerpts are present
- The implementer's status report
- The task / batch `_Boundary:_` scope constraints
- Validation commands discovered by the controller
- Parent `MECHANICAL_RESULTS` when available (two-tier review)
- Per-task `FEATURE_FLAG: required | skipped` when provided
- Relevant steering excerpts when applicable
- Relevant `## Implementation Notes` entries when applicable
- Note `CONTRACTS_UPDATED` from the implementer Status Report when present (intentional contract path updates)

## Outputs

Return one of:
- `APPROVED`
- `REJECTED`

Also return:
- Mechanical results
- Findings with severity
- Required remediation
- One-sentence summary

Use the language specified in `spec.json`.

## First Action

Run `git diff` to inspect the actual code changes. If the diff is large or ambiguous, read the changed files directly. Do not trust the implementer report as source of truth.

## Core Principle

Read the **authoritative spec input** yourself (Spec Excerpts when provided; otherwise only the referenced sections — never default to full-file dumps). Read the diff yourself. Verify mechanically where possible (or adopt parent `MECHANICAL_RESULTS`). Reject on concrete failures rather than interpretive optimism.
The main review question is not just "does it work?" but "does it stay inside the approved responsibility boundary without hiding new coupling?"

## Spec Excerpts Policy

- **When Spec Excerpts are provided**: they are authoritative. Do **not** Read `requirements.md`, `design.md`, or `docs/architecture/**` in full. Do **not** bulk-scan contract/architecture trees. If a needed heading or contract path is missing, REJECT with REMEDIATION naming the exact missing path/heading(s) for parent re-excerpt — do not full-file load as recovery.
- **When Spec Excerpts are absent** (standalone): read only the cited section numbers / headings / contract paths needed for this review.
- Drift judgment uses Contracts excerpts + related contracts + executable contracts; architecture unread in full does not block that judgment.

## Mechanical Checks

When the caller already provides parent `MECHANICAL_RESULTS` (batch/selection two-tier review), adopt those results as the mechanical baseline and re-run only if missing or suspicious — do not require a mandatory full-suite re-execution. Judgment Checks below remain mandatory either way.

When no parent results are provided, run these checks and use the result as primary signal.

### 1. Regression Safety
- Adopt parent test results when present; otherwise run the project's canonical test suite using the validation commands discovered by the controller.
- If tests fail, reject.

### 2. No Residual Placeholder Markers
- Adopt parent TBD/TODO/FIXME summary when present; otherwise check changed files for `TBD`, `TODO`, `FIXME`, `HACK`, `XXX`.
- Reject if new placeholder markers were introduced without explicit task justification.

### 3. No Hardcoded Secrets
- Adopt parent secrets summary when present; otherwise check changed files for hardcoded secrets or credentials.
- Reject if concrete secret patterns are introduced.

### 4. Boundary Respect
- Adopt parent boundary result when present; otherwise compare changed files against the task / batch `_Boundary:_` scope.
- Reject if the change spills outside the approved boundary without explicit justification.
- Reject if the implementation introduces hidden cross-boundary coordination inside what should be a local task.

### 5. RED Phase Evidence
- Adopt parent RED-phase result when present; cross-check the implementer status report for `RED_PHASE_OUTPUT` on behavioral tasks.
- Reject if RED evidence is missing, empty, or unrelated to the task's acceptance criteria.
- When `FEATURE_FLAG` is `skipped`, do not REJECT solely for missing flag-only protocol steps.

### 6. Runtime-Sensitive Static Checks
- If the project already has lint or equivalent static analysis for the touched stack, run the relevant command for the task boundary (or adopt a parent result when provided).
- Pay attention to patterns that can survive typecheck/build yet fail at runtime: type-only imports used as values, missing namespace value imports for qualified-name access, unresolved globals, and newly introduced runtime-sensitive dependencies without matching boot/runtime handling.
- If no project lint command exists, perform a targeted diff-based spot check in the changed files for those patterns.
- Reject on concrete findings that create a realistic boot-time or module-load failure.

## Judgment Checks

### 7. Reality Check
- Confirm the implementation is real production code, not a placeholder, stub, fake path, or deferred-work shell.

### 8. Acceptance Criteria Coverage
- Read the task description(s) and confirm all aspects are implemented, not only the primary happy path.

### 9. Requirements Alignment
- Use `### Requirements` in Spec Excerpts when provided; otherwise read only the referenced sections in `requirements.md` (not the whole file by default).
- Confirm each requirement is satisfied by concrete observable behavior.
- Use original section numbers only.

### 10. Design Alignment
- Use `### Design` in Spec Excerpts when provided; otherwise read only the referenced sections in `design.md` (not the whole file by default).
- Confirm the implementation uses the prescribed structures, interfaces, and dependency direction.
- Reject silent substitutions for design-mandated choices.

### 10.25 Contract Drift
- Use `### Contracts (authoritative for touched surfaces)` in Spec Excerpts when provided. **"Drifted"** = contradicts Contracts excerpt I/O / invariants / forbidden dependencies; **or** new public surface / event / data ownership without matching `docs/contracts/` update; **or** executable contract still red while claiming complete.
- Treat confirmed contract drift as **Important** (required fix before acceptance). Reject unless the Status Report includes `CONTRACTS_UPDATED` with intentional paths and the diff shows those updates (or code was aligned to the contract). Do not require rewriting unrelated contracts.

### 10.5 Boundary Audit
- Compare the implementation against the design's boundary commitments and out-of-boundary statements (from excerpts or referenced sections).
- Reject if downstream-specific behavior is pushed into an upstream boundary for convenience.
- Reject if the implementation creates new hidden dependencies, shared ownership, or undeclared coupling across adjacent boundaries.
- Reject if a task that is not an explicit integration task now behaves like one.

### 11. Test Quality
- Confirm tests prove the required behavior rather than only scaffolding.
- Confirm tests would fail if the implementation were removed or broken.

### 12. Error Handling
- Confirm relevant failure paths are handled and not silently swallowed.

## Severity Model

Use:
- `Critical` for broken functionality, invalid verification, data loss, security risk, or major scope violation
- `Important` for required fixes before acceptance — including confirmed **contract drift** (Contracts excerpt contradiction, missing `docs/contracts/` update for a new public surface, or red executable contracts while claiming complete)
- `Suggestion` for non-blocking improvements
- `FYI` for informational notes

## Stop / Escalate

Escalate instead of papering over the issue when:
- The approved spec is ambiguous in a correctness-critical way
- The design conflicts with what is technically possible
- Required evidence cannot be gathered
- The implementation only works by silently deviating from approved scope
- Boundary ownership cannot be determined cleanly from requirements, design, and task scope

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| “Tests pass, so approve” | Passing tests do not prove spec compliance or boundary respect. |
| “The extra behavior is useful” | Extra behavior outside approved scope is still drift. |
| “The implementer said RED was done” | RED must be evidenced, not asserted. |
| “This gap is small enough to let through” | Real gaps must be rejected or escalated. |
| “I should open the whole design.md” | Prefer Spec Excerpts / cited sections; name missing headings instead of full-file load. |
| “Architecture was not fully read, so I cannot judge contracts” | Drift is judged from Contracts excerpts + related contracts + executable contracts. |

## Output Format

```md
## Review Verdict
- VERDICT: APPROVED | REJECTED
- TASK: <task-id or batch task-id list>
- MECHANICAL_RESULTS:
  - Tests: PASS | FAIL (command and exit code)
  - TBD/TODO grep: CLEAN | <count> matches
  - Secrets grep: CLEAN | <count> matches
  - Static checks: PASS | FAIL | SPOT_CHECKED
  - Boundary: WITHIN | <files outside boundary>
  - Boundary audit: CLEAN | <spillover / hidden dependency findings>
  - RED phase: VERIFIED | MISSING | N/A
- FINDINGS:
  1. <specific finding with exact files/spec refs>
- REMEDIATION: <mandatory if REJECTED; name missing excerpt headings when applicable>
- SUMMARY: <one sentence>
```

When parent `MECHANICAL_RESULTS` were provided, transcribe them into the verdict block; append notes only if you re-ran a check.
</instructions>
