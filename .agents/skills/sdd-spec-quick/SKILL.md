---
name: sdd-spec-quick
description: Quick spec generation with interactive or automatic mode
---

# Quick Spec Generator

<instructions>
## CRITICAL: Automatic Mode Execution Rules

**If `--auto` flag is present in `$ARGUMENTS`, you are in AUTOMATIC MODE.**

In Automatic Mode:
- Execute ALL 4 phases in a continuous loop without stopping
- Display progress after each phase (e.g., "Phase 1/4 complete: spec initialized")
- IGNORE any "Next Step" messages from Phase 2-4 (they are for standalone usage)
- After Phase 4, run the final sanity review before exiting
- If `--from-orchestrate` is also present: follow Flag: `--from-orchestrate` (return for orchestrator Terminal auto-approve (S); no impl chain)
- Stop ONLY after the sanity review completes (and from-orchestrate extras if applicable) or if error occurs

---

## Core Task
Execute 4 spec phases sequentially. In automatic mode, execute all phases without stopping. In interactive mode, prompt user for approval between phases.

Before claiming quick generation is complete, run one lightweight sanity review over the generated requirements, design, and tasks. If the host supports fresh sub-agents, use one. Otherwise run the sanity review inline.

## Execution Steps

### Step 1: Parse Arguments and Initialize

Parse `$ARGUMENTS`:
- If contains `--auto`: **Automatic Mode** (execute all 4 phases)
- If contains `--from-orchestrate`: **From-Orchestrate Mode** (must be paired with `--auto`; see Flag section below)
- Otherwise: **Interactive Mode** (prompt at each phase)
- Extract description (remove `--auto` and `--from-orchestrate` flags if present)

Example:
```
"User profile with avatar upload --auto" → mode=automatic, description="User profile with avatar upload"
"feature-name --auto --from-orchestrate" → mode=from-orchestrate, description="feature-name"
"User profile feature" → mode=interactive, description="User profile feature"
```

Display mode banner and proceed to Step 2.

### Step 2: Execute Phase Loop

Execute these 4 phases in order:

---

#### Phase 1: Initialize Spec (inline — `sdd-spec-requirements` Step 0)

**Core Logic** (same as `/sdd-spec-requirements` Step 0; do not dispatch a separate init skill):

1. **Check for Brief**:
   - If `docs/specs/{feature-name}/brief.md` exists (created by `/sdd-discovery`), read it for discovery context
   - Use brief content as the project description instead of `$ARGUMENTS`

2. **Generate Feature Name**:
   - Convert description to kebab-case (reuse discovery directory when present)
   - Example: "User profile with avatar upload" → "user-profile-avatar-upload"
   - Keep name concise (2-4 words ideally)

3. **Check Uniqueness**:
   - Use Glob to check `docs/specs/*/`
   - If directory exists with only `brief.md` (no `spec.json`), use that directory (discovery created it)
   - Otherwise if feature name exists, append `-2`, `-3`, etc. (bare slug only; never invent numeric prefixes)

4. **Create Directory**:
   - Use Bash: `mkdir -p docs/specs/{feature-name}` (skip if already exists from discovery)

5. **Initialize Files from Templates** (`docs/settings/templates/specs/init.json`, `requirements-init.md`):

   Replace placeholders (`{{FEATURE_NAME}}`, `{{TIMESTAMP}}`, `{{PROJECT_DESCRIPTION}}`, language) and write:
   - `docs/specs/{feature-name}/spec.json`
   - `docs/specs/{feature-name}/requirements.md` (stub only — no EARS body)

6. **Output Progress**: "Phase 1/4 complete: Spec initialized at docs/specs/{feature-name}/"

**Automatic Mode**: IMMEDIATELY continue to Phase 2.

**Interactive Mode**: Prompt "Continue to requirements generation? (yes/no)"
- If "no": Stop, show current state
- If "yes": Continue to Phase 2

---

#### Phase 2: Generate Requirements

Invoke `/sdd-spec-requirements {feature-name}`.

Wait for completion. IGNORE any "Next Step" message (it is for standalone usage).

**Output Progress**: "Phase 2/4 complete: Requirements generated"

**Automatic Mode**: IMMEDIATELY continue to Phase 3.

**Interactive Mode**: Prompt "Continue to design generation? (yes/no)"
- If "no": Stop, show current state
- If "yes": Continue to Phase 3

---

#### Phase 3: Generate Design

Invoke `/sdd-spec-design {feature-name} -y`. The `-y` flag auto-approves requirements.

Wait for completion. IGNORE any "Next Step" message.

**Output Progress**: "Phase 3/4 complete: Design generated"

**Automatic Mode**: IMMEDIATELY continue to Phase 4.

**Interactive Mode**: Prompt "Continue to tasks generation? (yes/no)"
- If "no": Stop, show current state
- If "yes": Continue to Phase 4

---

#### Phase 4: Generate Tasks

Invoke `/sdd-spec-tasks {feature-name} -y`. The `-y` flag auto-approves requirements, design, and tasks.

Wait for completion.

**Output Progress**: "Phase 4/4 complete: Tasks generated"

#### Final Sanity Review

After Phase 4, run a lightweight sanity review before claiming completion.

- Review `requirements.md`, `design.md`, and `tasks.md` directly from disk. If `brief.md` exists, use it only as supporting context.
- Prefer a fresh review sub-agent when the host supports it. Pass only file paths and the review objective; the reviewer should read the generated files itself.
- Review focus:
  - Do requirements, design, and tasks tell a coherent story?
  - Are there obvious contradictions, missing prerequisites, or missing task coverage for required design work?
  - Are `_Depends:_`, `_Boundary:_`, and `(P)` markers plausible for implementation?
- If the review finds only task-plan-local issues, repair or update the generated `tasks.md` once, then re-run the sanity review.
- If the review finds a real requirements/design gap or contradiction, stop and report follow-up instead of claiming the quick spec is implementation-ready.

**All 4 phases plus sanity review complete.**

**If `--from-orchestrate`:** follow Flag: `--from-orchestrate` completion (optional unified validates, set `complexity_tier` if missing, do **not** approve or chain into `sdd-impl`). Return control to orchestrator for **Terminal auto-approve (S)**.

**Otherwise:** Output final completion summary (see Output Description section) and exit.

---

## Important Constraints

### Flag: --from-orchestrate

When present with `--auto`:

- Assume `brief.md` exists (discovery already ran).
- Run Phases 1–4 without interactive prompts.
- After Phase 4, run **lightweight sanity review** (existing final step).
- Then run **one** unified validate pass per phase if 05/06 implemented (`/sdd-validate-requirements`, `/sdd-validate-design-qa`); else run sanity review only.
- Set `spec.json` `complexity_tier` if missing (default `S` when invoked via this flag).
- Do **NOT** chain into `sdd-impl`.
- Do **not** set `approvals.*.approved` or `ready_for_implementation` — the orchestrator **[調整者]** owns those updates at Terminal auto-approve (S).

Terminal completion for `--from-orchestrate`:

- Do **not** open `[GATE] 仕様一式` for user approval
- Return control after artifact generation + sanity review (+ optional unified validates) succeed with all three `approvals.*.generated === true`
- Orchestrator **[調整者]** auto-sets all three `approvals.*.approved: true` and `ready_for_implementation: true`, emits PR Summary Output (`gates.md`), then ends orchestration

Without `--from-orchestrate`, keep the standalone Next Steps output below — do **not** overwrite with orchestrate PR Summary format.

### Error Handling
- Any phase failure stops the workflow
- Display error and current state
- Suggest manual recovery command

</instructions>

## Output Description

### Mode Banners

**Interactive Mode**:
```
Quick Spec Generation (Interactive Mode)

You will be prompted at each phase.
Note: Skips gap analysis and design validation.
```

**Automatic Mode**:
```
Quick Spec Generation (Automatic Mode)

All phases execute automatically without prompts.
Note: Skips optional validations (gap analysis, design review) and user approval prompts. Internal review gates still run.
Final sanity review still runs.
```

**From-Orchestrate Mode** (`--auto --from-orchestrate`):
```
Quick Spec Generation (From Orchestrate / S-tier quick-path)

All phases execute automatically. Sanity review (+ optional unified validates) runs.
Returns to orchestrator for Terminal auto-approve (S) — does not approve or start implementation.
```

### Intermediate Output

After each phase, show brief progress:
```
Spec initialized at docs/specs/{feature}/
Requirements generated → Continuing to design...
Design generated → Continuing to tasks...
```

### Final Completion Summary

**When `--from-orchestrate`:** note sanity review result and return control for orchestrator Terminal auto-approve (S) + PR Summary. Do **not** approve, do **not** emit PR Summary here, and do **not** suggest `/sdd-impl`.

**Otherwise** — provide output in the language specified in `spec.json`:

```
Quick Spec Generation Complete!

## Generated Files:
- docs/specs/{feature}/spec.json
- docs/specs/{feature}/requirements.md ({X} requirements)
- docs/specs/{feature}/design.md ({Y} components, {Z} endpoints)
- docs/specs/{feature}/tasks.md ({N} tasks)

Quick generation skipped:
- Separate gap step (gap is inline in `/sdd-spec-design` on brownfield; quick path skips it)

Sanity review: PASSED | FOLLOW-UP REQUIRED

## Next Steps:
1. Review generated specs (especially design.md)
2. Optional validation:
   - `/sdd-spec-design {feature}` - Re-run design (includes brownfield gap) if integration check needed
   - `/sdd-validate-design-qa {feature}` - Verify architecture quality
3. Start implementation: `/sdd-impl {feature}`

```

## Safety & Fallback

### Error Scenarios

**Template Missing**:
- Check `docs/settings/templates/specs/` exists
- Report specific missing file
- Exit with error

**Directory Creation Failed**:
- Check permissions
- Report error with path
- Exit with error

**Phase Execution Failed** (Phase 2-4):
- Stop workflow
- Show current state and completed phases
- Suggest: "Continue manually from `/sdd-spec-{next-phase} {feature}`"

**Sanity Review Failed**:
- Stop workflow
- Report the exact contradiction, missing prerequisite, or task-plan issue
- Suggest targeted follow-up with `/sdd-spec-design {feature}`, `/sdd-spec-tasks {feature}`, or manual edits depending on the finding

**User Cancellation** (Interactive Mode):
- Stop gracefully
- Show completed phases
- Suggest manual continuation
