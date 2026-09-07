---
name: sdd-debug
description: Investigate implementation failures using root-cause-first debugging. Phase 0 triage (no code changes) before root-cause analysis. Use when blocked, verification fails, manual smoke fails, or repeated remediation does not converge.
metadata:
  shared-rules: "investigation-prompt-template.md"
---

# sdd-debug

<background_information>
This skill is for fresh-context root cause investigation. It combines local evidence, runtime/config inspection, and external documentation or issue research when available. It is not a patch generator for guess-first debugging.
</background_information>

## Investigation Prompt Template

For user-driven session bootstrap, load `rules/investigation-prompt-template.md` from this skill's directory.
Use it only when the user asks to generate an investigation prompt, or when inputs are too vague to start Phase 0.
Do not substitute Phase 0 execution with template generation alone.

<instructions>
## When to Use

- Implementer reports `BLOCKED`
- Reviewer rejection repeats after remediation
- Validation fails unexpectedly
- A task appears to conflict with runtime or platform reality
- The same failure survives more than one attempted fix
- Manual smoke / release verification fails (e.g. "works in dev, fails in release")
- User reports the same symptom after one or more fix attempts ("still broken", "still errors")

Do not use this skill to speculate about fixes before gathering evidence.

## Inputs

Provide:
- Exact failure symptom or blocker statement
- Error messages, stack trace, and failing command output
- Current `git diff` or summary of uncommitted failed changes
- Task brief: what was being built
- Reviewer feedback, if the failure came from review rejection
- Relevant spec file paths (`requirements.md`, `design.md`)
- Relevant requirement/design section numbers
- Relevant `## Implementation Notes`
- Runtime or environment constraints already known
- Build/run mode: dev | release | both
- Manual smoke result, if any (duration, input source, expected vs actual)
- Log excerpt with `--log` or equivalent (paste or file path)
- Smoke checklist path, if the feature has one (e.g. `docs/specs/<feature>/smoke-checklist.md`)

## Outputs

Return:
- `ROOT_CAUSE`
- `CATEGORY`
- `FIX_PLAN`
- `VERIFICATION`
- `NEXT_ACTION: RETRY_TASK | BLOCK_TASK | STOP_FOR_HUMAN`
- `CONFIDENCE: HIGH | MEDIUM | LOW`
- `NOTES`

Use the language specified in `spec.json`.

## Method

### 0. Phase 0 — Triage (mandatory, no code changes)

Run Phase 0 before reading code for fixes, proposing patches, or editing the repository.
If the user already supplied a structured investigation prompt (symptom class, diff table, signal checklist), validate and refine it; do not restart from zero.

#### 0.1 Symptom classification

Classify into exactly one primary bucket (secondary allowed in NOTES):

| Class | Meaning | Examples |
|-------|---------|----------|
| `NON_FUNCTIONAL` | Pipeline runs but user-visible outcome missing | No transcript blocks, no UI update |
| `ERROR_SURFACE` | User sees error toast / dialog / crash | "Restart required", INFERENCE_FAILED |
| `DEGRADED` | Works but slow, wrong quality, or partial | 10 min latency, garbage text, mic OK / speaker bad |

If classes mixed across rounds, state which class this round targets first.

#### 0.2 Environment diff table

Produce a table comparing the failing environment vs the known-good baseline (usually dev vs release, or before vs after a change).

Derive rows from, in order:

1. Feature spec (`requirements.md`, `design.md`, `smoke-checklist.md`)
2. Related ADRs / `docs/architecture/`
3. Repository config (`compose`, build flags, paths)

Typical row categories (include only what applies):

- Binary/build mode (dev / release / flags)
- Resource paths (models, app_data_dir, legacy paths)
- Startup / injection order (setup, inject, load thread)
- IPC / events (ACL, listen permissions, event names)
- Watchdogs / timeouts / error propagation
- Native build flags (e.g. cmake optimization)
- External dependencies (device, permissions, network)

Mark each row: `SAME` | `DIFFERS` | `UNKNOWN` (UNKNOWN must have a single check to resolve it).

#### 0.3 Signal checklist

From logs and observable behavior, check expected signals for this feature. Use spec smoke checklist when present.

Format:

| Signal | Expected | Observed | Verdict |
|--------|----------|----------|---------|
| ... | ... | ... | PASS / FAIL / MISSING |

Include phase transitions, worker ready, inference completion, domain events (e.g. block-appended), errors, and resource metrics (CPU/disk) when relevant.

Stop Phase 0 when every UNKNOWN in 0.2 has a check result OR is explicitly deferred with reason.

#### 0.4 Single hypothesis

State exactly one leading hypothesis:

- `HYPOTHESIS`: one sentence
- `CONFIDENCE`: HIGH | MEDIUM | LOW
- `FALSIFICATION`: what observation would disprove it
- `NEXT_CHECK`: one repo inspection or user action (no code edit)

**Code change gate**: Do not edit source files until `CONFIDENCE: HIGH` OR user explicitly overrides with documented risk.

LOW/MEDIUM: run `NEXT_CHECK` only; return Phase 0 summary and request more evidence.

#### 0.5 User follow-ups

End Phase 0 with up to 5 specific items for the user (log line ranges, rebuild command, file existence, Task Manager metrics, etc.).

#### Phase 0 output (prepend to Debug Report)

```md
## Triage (Phase 0)
- SYMPTOM_CLASS: NON_FUNCTIONAL | ERROR_SURFACE | DEGRADED
- ENV_DIFF: <markdown table>
- SIGNAL_CHECKLIST: <markdown table>
- HYPOTHESIS: <one sentence>
- CONFIDENCE: HIGH | MEDIUM | LOW
- NEXT_CHECK: <single step>
- USER_FOLLOW_UPS:
  1. ...
```

Then continue to Method steps 1–6. If Phase 0 already yielded HIGH confidence, step 4–5 may be abbreviated but must reference evidence from 0.3.

### 1. Read the Error Carefully
Extract:
- Exact error text
- Stack trace or failure location
- The command that produced the failure
- Whether the failure is deterministic or intermittent

### 2. Inspect Local Runtime and Repository State
Inspect the repository for local evidence:
- `package.json`, `pyproject.toml`, `go.mod`, `Makefile`, `README*`
- Build config
- `tsconfig` or equivalent language/runtime config
- Runtime-specific config
- Dependency versions and scripts
- Relevant changed files from `git diff`

### 3. Search the Web if Available
If web access is available, search:
- The exact error message
- The technology + symptom combination
- Official documentation
- Version-specific issue trackers or migration notes

Prefer:
- Official docs
- Official repos/issues
- Version-specific references
- Runtime-specific documentation

### 4. Classify the Root Cause
Use one category:
- `MISSING_DEPENDENCY`
- `RUNTIME_MISMATCH`
- `MODULE_FORMAT`
- `NATIVE_ABI`
- `CONFIG_GAP`
- `LOGIC_ERROR`
- `TASK_ORDERING_PROBLEM`
- `TASK_DECOMPOSITION_PROBLEM`
- `SPEC_CONFLICT`
- `EXTERNAL_DEPENDENCY`

### 5. Determine the Smallest Safe Next Action
Decide whether the issue can be fixed inside this repo by:
- Editing files
- Adjusting configuration
- Adding or correcting dependencies
- Restructuring code

Use `NEXT_ACTION: RETRY_TASK` when the issue is repo-fixable inside the current approved task plan.

### 6. Determine Whether the Task Plan Is Still Valid
Decide whether the current approved task plan is still safe to execute as written.

Prefer `NEXT_ACTION: STOP_FOR_HUMAN` when:
- A missing prerequisite task should exist before this one
- The current task is ordered incorrectly relative to unfinished work
- The current task boundary is wrong and should be split or merged
- The task is too large or ambiguous to fix safely inside the current implementation loop

Use `NEXT_ACTION: BLOCK_TASK` only when the current task should stop but the rest of the queue can still proceed safely.

Do not propose a brute-force code fix as a substitute for revising `tasks.md` or the approved plan.

## Critical Rule

Do not propose a multi-fix shotgun plan. Identify the root cause first, then produce the smallest plausible fix plan. If the true problem is a spec conflict or architecture problem, say so directly.

## Fix / Smoke Gate

After any fix (whether proposed in FIX_PLAN or applied by a follow-up implementer):

1. Run the feature's minimal smoke from spec checklist (or state a justified substitute).
2. Re-run the Phase 0 signal checklist; failing signals must be listed explicitly.
3. Do not close the debug loop on `bun run verify` (or unit tests) alone when the failure was manual/release smoke.

Record smoke in `VERIFICATION` with: build command, run command, duration, input, pass/fail per signal.

## Stop / Escalate

Use `NEXT_ACTION: STOP_FOR_HUMAN` when the blocker genuinely requires:
- Human product/requirements decision
- External credentials or inaccessible services
- Hardware or unavailable external systems
- Re-scoping due to spec/platform conflict

If the issue is fixable by repo changes inside the current task plan, do not escalate prematurely.

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "This probably just needs a quick patch" | Patch-first debugging creates rework. |
| "Let's try a few fixes" | Multi-fix guessing hides root cause. |
| "The spec is probably wrong, I'll adapt it" | Spec conflicts must be surfaced explicitly. |
| "The docs search is optional" | For runtime/dependency issues, docs and version issues often contain the shortest path to root cause. |

## Output Format

```md
## Triage (Phase 0)
- SYMPTOM_CLASS: NON_FUNCTIONAL | ERROR_SURFACE | DEGRADED
- ENV_DIFF: <table>
- SIGNAL_CHECKLIST: <table>
- HYPOTHESIS: <one sentence>
- CONFIDENCE: HIGH | MEDIUM | LOW
- NEXT_CHECK: <one step, no code unless HIGH>
- USER_FOLLOW_UPS:
  1. ...

## Debug Report
- ROOT_CAUSE: <1-2 sentence root cause; "investigating" if not yet HIGH>
- CATEGORY: MISSING_DEPENDENCY | RUNTIME_MISMATCH | MODULE_FORMAT | NATIVE_ABI | CONFIG_GAP | LOGIC_ERROR | TASK_ORDERING_PROBLEM | TASK_DECOMPOSITION_PROBLEM | SPEC_CONFLICT | EXTERNAL_DEPENDENCY
- FIX_PLAN:
  1. <empty or "none until CONFIDENCE HIGH" when not ready>
  2. ...
- VERIFICATION: <commands + manual smoke if release/runtime issue>
- NEXT_ACTION: RETRY_TASK | BLOCK_TASK | STOP_FOR_HUMAN
- CONFIDENCE: HIGH | MEDIUM | LOW
- NOTES: <context; secondary symptom class; deferred UNKNOWN rows>
```
</instructions>
