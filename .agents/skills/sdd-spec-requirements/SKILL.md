---
name: sdd-spec-requirements
description: Initialize a spec if needed (Step 0), then generate comprehensive requirements for a specification
metadata:
  shared-rules: "ears-format.md, requirements-review-gate.md"
---


# Requirements Generation

<background_information>
- **Success Criteria**:
  - Ensure `spec.json` + stub `requirements.md` exist (Step 0 when missing)
  - Create complete requirements document aligned with steering context
  - Follow the project's EARS patterns and constraints for all acceptance criteria
  - Focus on core functionality without implementation details
  - Make inclusion/exclusion boundaries explicit when scope could otherwise be misread
  - Update metadata to track generation status
</background_information>

<instructions>
## Execution Steps

### Step 0: Initialize (if needed)

Skip this step when `docs/specs/$1/spec.json` already exists.

Otherwise, execute the init Step 0 logic inline:

1. **Check for Brief**: If `docs/specs/$1/brief.md` exists (created by `/sdd-discovery`), read it. Use it to pre-fill the project description and skip clarification questions the brief already answers.
2. **Clarify Intent**: The Project Description must contain three elements: (a) who has the problem, (b) current situation, (c) what should change. If `brief.md` covers these, continue. Otherwise ask the user before proceeding — do not fill gaps with assumptions. If neither `brief.md` nor `$ARGUMENTS` provides a project description, **stop and ask the user**.
3. **Naming**: Reuse the discovery directory name `$1` exactly when it already exists. If generating a name from `$ARGUMENTS` (no directory yet): short kebab-case `<concept-slug>` with optional numeric prefix `NNN-` (user-specified only; never invent a number). On user-specified number collision, stop and ask — do not renumber.
4. **Check Uniqueness**: If the directory already exists with only `brief.md` (no `spec.json`), use that directory.
5. **Create Directory**: `mkdir -p docs/specs/$1` if needed.
6. **Initialize from templates** (reuse existing paths — do not move templates):
   - Read `docs/settings/templates/specs/init.json`
   - Read `docs/settings/templates/specs/requirements-init.md`
   - Replace placeholders: `{{FEATURE_NAME}}`, `{{TIMESTAMP}}` (ISO 8601), `{{PROJECT_DESCRIPTION}}` (from brief or `$ARGUMENTS`), language (`ja` / detect from input, default `en`)
   - Write `spec.json` and stub `requirements.md` (requirements-init template only)
7. **Do NOT** generate full EARS requirements in this step. Do **NOT** set `approvals.requirements.generated: true`.

Then continue to Step 1 (Load Context).

1. **Load Context**:
   - Read `docs/specs/$1/spec.json` for language and metadata
   - Read `docs/specs/$1/brief.md` if it exists (discovery capture: Trigger, Problem, Desired Outcome, Scope, Route; optional Approach / Current State / deps / Constraints)
   - Read `docs/specs/$1/requirements.md` for project description
   - Core steering context: `product.md`, `tech.md`, `structure.md`
   - Additional steering files only when directly relevant to feature scope, user personas, business/domain rules, compliance/security constraints, operational constraints, or existing product boundaries
   - Relevant local agent skills or playbooks only when they clearly match the feature's host environment or use case and contain domain terminology or workflow rules that shape user-observable requirements

#### Load rules (persistent docs)

- Primary: this feature's `brief.md` / `requirements.md` (+ steering as above)
- `docs/architecture/**` / `docs/contracts/**`: **optional** related files only if needed for scope clarity — never glob-bulk-Read; path is always index → named file
- Do **not** author or merge contract bodies here; defer public-surface contracts to design
- Do not “read everything just in case”

2. **Read Guidelines**:
   - Read `rules/ears-format.md` from this skill's directory for EARS syntax rules
   - Read `rules/requirements-review-gate.md` from this skill's directory for pre-write review criteria
   - Read `docs/settings/templates/specs/requirements.md` for document structure

#### Parallel Research (sub-agent dispatch)

The following research areas are independent. Decide the optimal decomposition based on project complexity -- split, merge, add, or skip sub-agents as needed.

**In main context** (essential for requirements generation):
- Spec files: spec.json, brief.md, requirements.md (project description)
- EARS format rules, requirements review gate, requirements template
- Core steering: product.md, tech.md (directly inform scope and constraints)

**Delegate to sub-agent** (keeps exploration out of main context):
- **Codebase hints** (brownfield projects): Spawn a sub-agent to explore existing implementations that inform requirement scope. Ask it to summarize: (1) what already exists, (2) relevant interfaces/APIs, (3) patterns that new requirements should align with. Return a summary under 150 lines.
- **Domain research** (when external knowledge is needed): Spawn a sub-agent for web research on domain-specific requirements, standards, or best practices. Return a concise findings summary.
- **Additional steering and playbooks**: If many steering files or local agent playbooks exist, spawn a sub-agent to scan them and return only the sections relevant to this feature.

For greenfield projects with minimal codebase, skip sub-agent dispatch and load context directly. If multi-agent is not available, execute sequentially in main context.

After all research completes, synthesize findings in main context before generating requirements.

3. **Generate Requirements Draft**:
   - Create initial requirements draft based on project description
   - Group related functionality into logical requirement areas
   - Apply EARS format to all acceptance criteria
   - Use language specified in spec.json
   - Preserve terminology continuity across phases:
     - discovery = Scope In/Out (+ optional Approach; no EARS)
     - requirements = explicit inclusion/exclusion and adjacent expectations when needed
     - design = `Boundary Commitments`
     - tasks = `_Boundary:_`
   - If scope could be misread, add lightweight boundary context without introducing implementation or architecture ownership detail
   - Keep this as a draft until the review gate passes; do not write `requirements.md` yet

4. **Review Requirements Draft**:
   - Run the `Requirements Review Gate` from `rules/requirements-review-gate.md`
   - Review coverage, EARS compliance, ambiguity, adjacent expectations, and scope boundaries before finalizing
   - If issues are local to the draft, repair the requirements and review again
   - Keep the review bounded to at most 2 repair passes
   - If the draft exposes a real scope ambiguity or contradiction, stop and ask the user to clarify instead of writing guessed requirements

5. **Finalize and Update Metadata**:
   - Write `docs/specs/$1/requirements.md` only after the requirements review gate passes
   - Set `phase: "requirements-generated"`
   - Set `approvals.requirements.generated: true`
   - Update `updated_at` timestamp

## Discovery handoff

`brief.md` is a **capture artifact**, not a requirements substitute.

- If brief lacks detail, expand in requirements — do not send the user back to discovery unless Path or Scope In/Out is fundamentally wrong.
- Brownfield codebase research: run here (sub-agent), not in discovery.
- EARS / AC authoring happens only in this skill (after Step 0).
- Do not expect discovery briefs to contain Boundary Candidates tables, approach Pros/Cons, or viability notes — those were deferred by design.

## Important Constraints

### Requirements Scope: WHAT, not HOW
Requirements describe user-observable behavior, not implementation. Use this to decide what belongs here vs. in design:

**Ask the user about (requirements scope):**
- Functional scope — what is included and what is excluded
- User-observable behavior — "when X happens, what should the user see/experience?"
- Business rules and edge cases — limits, error conditions, special cases
- Non-functional requirements visible to users — response time expectations, availability, security level
- Adjacent expectations only when they change user-visible behavior or operator expectations — what this feature relies on, and what it explicitly does not own

**Do not ask about (design scope — defer to design phase):**
- Technology stack choices (database, framework, language)
- Architecture patterns (microservices, monolith, event-driven)
- API design, data models, internal component structure
- How to achieve non-functional requirements (caching strategy, scaling approach)
- Internal ownership mapping, component seams, or implementation boundaries that belong in design

**Litmus test**: If an EARS acceptance criterion can be written without mentioning any technology, it belongs in requirements. If it requires a technology choice, it belongs in design.

### Other Constraints
- Each requirement must be testable and unambiguous. If the project description leaves room for multiple interpretations on scope, behavior, or boundary conditions, ask the user to clarify before generating that requirement. Ask as many questions as needed; do not generate requirements that contain your own assumptions.
- Choose appropriate subject for EARS statements (system/service name for software)
- Requirement headings in requirements.md MUST include a leading numeric ID only (for example: "Requirement 1", "1.", "2 Feature ..."); do not use alphabetic IDs like "Requirement A".
- Step 0 must not generate EARS requirement bodies — only `spec.json` + stub `requirements.md` from `requirements-init.md`.
</instructions>

## Output Description
Provide output in the language specified in spec.json with:

1. **Generated Requirements Summary**: Brief overview of major requirement areas (3-5 bullets)
2. **Document Status**: Confirm requirements.md updated and spec.json metadata updated (note if Step 0 initialized the spec)
3. **Review Gate**: Confirm the requirements review gate passed
4. **Next Steps**: Guide user on how to proceed (approve and continue, or modify)

**Format Requirements**:
- Use Markdown headings for clarity
- Include file paths in code blocks
- Keep summary concise (under 300 words)

## Safety & Fallback

### Error Scenarios
- **Missing Project Description**: If neither `brief.md` nor `$ARGUMENTS` provides project description and `spec.json` is missing → stop and ask the user (same as Step 0). If stub `requirements.md` lacks project description after Step 0, ask user for feature details.
- **Step 0 must not** set `approvals.requirements.generated: true`.
- **Template Missing**: If template files don't exist, use inline fallback structure with warning; for Step 0 init templates missing, report error with specific path
- **Language Undefined**: Default to English (`en`) if spec.json doesn't specify language
- **Incomplete Requirements**: After generation, explicitly ask user if requirements cover all expected functionality
- **Steering Directory Empty**: Warn user that project context is missing and may affect requirement quality
- **Non-numeric Requirement Headings**: If existing headings do not include a leading numeric ID (for example, they use "Requirement A"), normalize them to numeric IDs and keep that mapping consistent (never mix numeric and alphabetic labels).
- **Scope Ambiguity Found During Requirements Review**: Stop execution, do not write a guessed `requirements.md`, and ask the user to clarify the missing or conflicting scope before re-running `/sdd-spec-requirements $1`
- **Ambiguous Feature Name (Step 0)**: If feature name generation is unclear, propose 2-3 options and ask user to select
- **Directory Conflict (Step 0)**: User-specified `NNN-<concept-slug>` collision → stop and ask; bare slug collision → reuse when same work, else choose a more specific slug

### Next Phase: Design Generation

**If Requirements Approved**:
- Review generated requirements at `docs/specs/$1/requirements.md`
- **Optional Gap Analysis** (for existing codebases):
  - Gap analysis runs automatically inside `/sdd-spec-design` on brownfield
  - Identifies existing components, integration points, and implementation strategy
  - Greenfield: skipped — do not dispatch a standalone gap step
- Then `/sdd-spec-design $1 -y` to proceed to design phase

**If Modifications Needed**:
- Provide feedback and re-run `/sdd-spec-requirements $1`
