---
name: sdd-spec-design
description: Create comprehensive technical design for a specification. Runs inline brownfield gap analysis before discovery.
metadata:
  shared-rules: "design-principles.md, design-discovery-full.md, design-discovery-light.md, design-discovery-minimal.md, design-synthesis.md, design-review-gate.md, gap-analysis.md"
disable-model-invocation: true
---


# Technical Design Generator

<background_information>
- **Success Criteria**:
  - All requirements mapped to technical components with clear interfaces
  - The design makes responsibility boundaries explicit enough to guide task generation and review
  - Persistent public-surface contracts written/merged under `docs/contracts/**` (and related `docs/architecture/**`) at design time, with Persistent References in `design.md`
  - Brownfield gap analysis (`research.md`) completed when needed — skipped on greenfield
  - Appropriate architecture discovery and research completed without duplicating gap codebase survey
  - Design aligns with steering context and existing patterns
  - Visual diagrams included for complex architectures
</background_information>

<instructions>
## Execution Steps

### Step 1: Load Context

**Read all necessary context**:
- `docs/specs/$1/spec.json`, `requirements.md`, `design.md` (if exists)
- `docs/specs/$1/brief.md` (if exists — Current State / brownfield signals)
- `docs/specs/$1/research.md` (if exists — prior gap analysis or discovery log)
- Core steering context: `product.md`, `tech.md`, `structure.md`
- Additional steering files only when directly relevant to requirement coverage, architecture boundaries, integrations, runtime prerequisites, security/performance constraints, or team conventions that affect implementation readiness
- `docs/settings/templates/specs/design.md` for document structure
- Read `rules/design-principles.md` from this skill's directory for design principles
- `docs/settings/templates/specs/research.md` for discovery / gap log structure

#### Load rules (persistent docs)

| 資料 | このフェーズ |
|------|--------------|
| feature req/design/research | **主** |
| architecture / contracts | **主（関連のみ）** — Step 4: index → related Read → merge/create |
| ADR | 関連 **1–2**（境界・破壊的契約・重要判断時）。追記型; 全件禁止 |

- Never glob-bulk-Read `docs/contracts/**` or `docs/architecture/**`
- Procedure: **index → Persistent References / named related paths → those files only**
- Do not “read everything just in case”. Extra persistent excerpts for parent orchestration: aim **~80–150 lines** total; if over, cut paths / shrink scope
- Whole architecture diagrams only when this feature changes boundaries

**Validate requirements readiness**:
- Verify `approvals.requirements.generated === true` in spec.json (stop if false, see Safety & Fallback)

**Artifact-only resume**: 前のチャット履歴・口頭の合意・未書き込みの決定を前提にしない。フェーズの入力は上記 Load Context の成果物（および steering）のみ。チャットにしかない意図が必要なら、生成前に成果物へ書いてから続行する（勝手に補完しない）。要求の曖昧さを会話記憶で埋めない。不足なら `requirements.md` の修正をユーザーに求め、設計を進めない。

### Step 2: Discovery & Analysis

**Critical: This phase ensures design is based on complete, accurate information.**

#### Step 2.0: Gap Analysis (brownfield only)

Read `../sdd-orchestrate/rules/greenfield.md` before any gap or codebase sub-agent dispatch.

Determine **greenfield** per that rule. If ambiguous → treat as **not greenfield** (safer to run gap).

Otherwise determine **brownfield** when **any** of:

- `brief.md` Current State indicates existing implementation / extension, or
- `requirements.md` describes extending an existing system, or
- Lightweight Grep finds feature-related code in the codebase

If **greenfield**:

- **Skip gap analysis entirely.** Do **not** spawn gap-analysis / codebase survey sub-agents.
- Do **not** write `research.md` solely for gap analysis. (Optional later: external API research log only if full discovery needs it.)
- When writing `design.md` (Step 7), include this one line in Overview:
  `_Gap analysis: skipped (greenfield per brief Current State)._`
- Proceed to Step 2.1 Classify Feature Type.

If **brownfield**:

- Read `rules/gap-analysis.md` from this skill's directory.
- Execute the gap analysis framework (may use sub-agents per gap-analysis rules — codebase analysis, external deps when needed).
- Write `docs/specs/$1/research.md` following `docs/settings/templates/specs/research.md` (append with `---` if the file already exists).
- Then proceed to Step 2.1; treat `research.md` as discovery input.

#### Step 2.1: Classify Feature Type (2 axes)

**Do not** treat greenfield as Full discovery by default. Classify on two axes, then select discovery from the mapping table.

**Axis A — Codebase** (from Step 2.0 / brief Current State):

| Label | When |
| ----- | ---- |
| **greenfield** | No existing implementation; Step 2.0 skipped |
| **brownfield** | Extending existing system; Step 2.0 ran |
| **extension** | Existing *spec* / feature extension (update flow or brief says extend a generated spec) |

**Axis B — Scope scale** (from brief; `spec.json` `complexity_tier` overrides when present):

| Scale | Brief heuristic |
| ----- | --------------- |
| **simple** | Scope In bullets ≤ 5 **and** primary external APIs/tools ≤ 3 **and** brief cites a reference implementation **or** fixed stack |
| **standard** | Scope In 6–10 **or** tools/APIs 4–8 |
| **complex** | Scope In > 10 **or** tools/APIs > 8 **or** multi-service |

`complexity_tier` priority (never downgrade L to simple): **S → simple**, **M → standard**, **L → complex**.

1. **Map classification → discovery process**:

| Codebase | Scale | Discovery |
| -------- | ----- | --------- |
| greenfield | simple | **Minimal** — `rules/design-discovery-minimal.md` |
| greenfield | standard | **Light** — `rules/design-discovery-light.md` |
| greenfield | complex | **Full** — `rules/design-discovery-full.md` |
| brownfield | simple | Gap (2.0) + **Light** |
| brownfield | standard | Gap + **Light** |
| brownfield | complex | Gap + **Full** |
| extension | any | Gap + **Integration-focused** (light) |

**Deleted mapping**: `New Feature (greenfield) → Full discovery required`.

Hard rules:
- Do **not** drop security requirements (PAT, origin checks, etc.) from design because scale is simple
- Do **not** reclassify `complexity_tier: L` as simple
- When brownfield/`research.md` exists: **Do NOT** repeat the same codebase survey in full discovery — reference `research.md`

2. **Execute the selected discovery process**:

   **Minimal** (greenfield + simple):
   - Read and execute `rules/design-discovery-minimal.md`
   - No WebSearch; no full sub-agent; optional ≤1 Grep on a cited reference path

   **Light** (greenfield standard; brownfield simple/standard; extension):
   - Read and execute `rules/design-discovery-light.md`
   - Prefer `research.md` gap findings for integration points; light Grep only for gaps not already documented

   **Full** (greenfield complex; brownfield complex):
   - Read and execute `rules/design-discovery-full.md`
   - When `research.md` exists from Step 2.0: reuse it (see discovery-full reuse rule)
   - Conduct thorough research using WebSearch/WebFetch for gaps **not** covered in research.md

#### Parallel Research (sub-agent dispatch)

The following research areas are independent and can be dispatched as **sub-agents**. The agent should decide the optimal decomposition based on feature complexity — split, merge, add, or skip sub-agents as needed. Each sub-agent returns a **findings summary** (not raw data) to keep the main context clean for synthesis.

**Typical research areas** (adjust as appropriate):
- **Codebase analysis**: Skip if Step 2.0 already produced `research.md` covering the same questions; otherwise existing architecture patterns, integration points, code conventions
- **External research**: Dependencies, APIs, latest best practices (only topics not already in research.md)
- **Context loading** (usually main context): Steering files, design principles, discovery rules, templates

For **Minimal** discovery (and greenfield with gap skipped at simple scale), skip sub-agent dispatch entirely — pattern check in main context only.

After all findings return, synthesize in main context before proceeding.

3. **Retain Discovery Findings for Step 3**:
   - External API contracts and constraints
   - Technology decisions with rationale
   - Existing patterns to follow or extend (from gap / research.md when present)
   - Integration points and dependencies
   - Identified risks and mitigation strategies
   - Boundary candidates, out-of-boundary decisions, and likely revalidation triggers

4. **Persist Findings to Research Log**:
   - Create or update `docs/specs/$1/research.md` using the shared template
   - **Greenfield**: only write/update `research.md` if discovery produced material external research worth logging; do not invent a gap analysis section
   - **Brownfield**: research.md already has gap analysis from Step 2.0 — append discovery/synthesis topics without wiping gap findings
   - Summarize discovery scope and key findings (Summary section)
   - Record investigations in Research Log topics with sources and implications
   - Document architecture pattern evaluation, design decisions, and risks using the template sections
   - Use the language specified in spec.json when writing or updating `research.md`

### Step 3: Synthesis

**Apply design synthesis to discovery findings before writing.**

- Read and apply `rules/design-synthesis.md` from this skill's directory
- This step requires the full picture from discovery — do not parallelize or delegate to sub-agents
- Record synthesis outcomes (generalizations found, build-vs-adopt decisions, simplifications) in `research.md` when a research log exists or is warranted

**Size guards when scope scale is simple**:
- Omit "Extension scenarios" table unless requirements mention future extension
- File structure plan: list top-level + `src/` only (no per-file comments)
- One mermaid diagram maximum
- Target `design.md` length guidance: ≤ 150 lines (security/auth requirements must still be present)

### Step 4: Persist / Merge External Contracts (required before design draft)

**Do this before drafting `design.md`. Persistent contracts are written at design time — there is no post-implementation contract-creation phase.**

1. **Read indexes only** (mandatory):
   - `docs/contracts/README.md`
   - `docs/architecture/README.md`
   - Do **not** bulk-read every file under those trees

2. **Read only related files**: From the indexes, open existing contract / architecture files that touch this feature's Boundary Commitments or public surfaces. Skip unrelated entries.

3. **Merge or create when the public surface / boundary changes**:
   - **contracts** (`docs/contracts/`): Diff-merge into existing files; create new `<domain>-<surface>.md` when no file exists. Use `docs/settings/templates/contracts/contract.md` for new files. Update `docs/contracts/README.md` Entries when adding a path.
   - **architecture/boundaries** (`docs/architecture/boundaries.md` and related): Update **only** related sections. Do not rewrite unrelated sections.
   - **Important decisions (ADR)**: When any of the following apply, create a **new** ADR under `docs/architecture/adr/` (template: `docs/settings/templates/architecture/adr.md`; numbering in `docs/architecture/adr/README.md`):
     - Dependency direction or ownership-boundary change
     - Breaking change to a public contract
     - Major tech adopt/reject that will need rationale later
     - Decision that touches Revalidation Triggers
     - Do **not** ADR local implementation detail or naming that code+tests alone explain
     - **Append-only**: never merge/overwrite a new decision into an existing ADR body. To reverse a decision → new ADR + set old Status to `Superseded by ADR-XXXX`
     - **Index update (mandatory)**: Register every new ADR in `docs/architecture/adr/README.md` Entries (path, one-line purpose, owners/domains, Status). When superseding, update the old entry's Status to `Superseded by ADR-XXXX` as well as the old ADR file header. Never leave an ADR file that is missing from the index.
     - Load only related ADR(s) from the index (**1–2 max**); do not bulk-read all ADR bodies
   - **Merge rules**:

   | Target | Action |
   | ------ | ------ |
   | contracts | Read existing; diff-update. Destructive changes must be explicit via ADR or a **Changelog** section in the contract. New paths must appear in `docs/contracts/README.md` Entries |
   | architecture diagrams / boundaries | Diff-merge related sections only. Whole-file rewrite forbidden |
   | ADR | New file only (do not merge bodies; Superseded replaces via new ADR). New/superseded ADRs must update `docs/architecture/adr/README.md` Entries |

   On conflict: do not casually delete prior definitions; record the change rationale in an ADR or the contract's Changelog.

4. **Internal-only / no surface change**: If this is a pure internal refactor with no contract-surface change, skip create/modify. The draft may use `Mode: reference` only and must state **No contract changes** in Persistent References notes.

5. **Prepare Persistent References** for the design draft (required section — see template). Every `Mode: modify` path listed must already be updated on disk before the review gate.

### Step 5: Generate Design Draft

1. **Load Design Template and Rules**:
   - Read `docs/settings/templates/specs/design.md` for structure
   - Read `rules/design-principles.md` from this skill's directory for principles

2. **Generate Design Draft**:
   - **Follow specs/design.md template structure and generation instructions strictly**
   - **Boundary-first requirement**: Before expanding supporting sections, make the boundary explicit. The draft must clearly define what this spec owns, what it does not own, which dependencies are allowed, and what changes would require downstream revalidation.
   - **Persistent References (required)**: Include the Persistent References section (after Architecture or near Boundary Commitments). List every related `docs/contracts/**`, `docs/architecture/**`, and ADR path with `Mode: modify | reference`. New public surfaces without a `docs/contracts/` file are not allowed.
   - **Integrate all discovery findings and synthesis outcomes**: Use researched information (APIs, patterns, technologies) and synthesis decisions (generalizations, build-vs-adopt, simplifications) throughout component definitions, architecture decisions, and integration points — including Step 2.0 gap findings when present
   - **File Structure Plan** (required): Populate the File Structure Plan section with concrete file paths and responsibilities. Analyze the codebase to determine which files need to be created vs. modified. Each file must have one clear responsibility. This section directly drives task `_Boundary:_` annotations and implementation Task Briefs — vague file structures produce vague implementations.
   - **Testing Strategy**: Derive test items from requirements' acceptance criteria, not generic patterns. Each test item should reference specific components and behaviors from this design. E2E paths must map to the critical user flows identified in requirements. Avoid vague entries like "test login works" -- instead specify what is being verified and why it matters.
   - **Observability & Operational Readiness** (required): Populate logging (with explicit PII/secret masking rules), metrics, alerts, debuggability, and the performance/deployment-rollback/migration subsections. Use explicit `N/A — <reason>` instead of omitting items. These sections are reviewed by the security validate and verified (not generated) by the AI-DLC final design gate — leaving them out forces validate-phase rollbacks.
   - If existing design.md found in Step 1, use it as reference context (merge mode)
   - Apply design rules: Type Safety, Visual Communication, Formal Tone
   - Use language specified in spec.json
   - Keep this as a draft until the review gate passes; do not write `design.md` yet

### Step 6: Review Design Draft

- Read and apply `rules/design-review-gate.md` from this skill's directory
- Verify requirements coverage, architecture readiness, implementation executability, and Persistent References / external contract updates before finalizing the design
- If issues are local to the draft (or to a `Mode: modify` contract/architecture file from Step 4), repair and review again
- Keep the review bounded to at most 2 repair passes
- If the draft exposes a real requirements/design gap, stop and return to requirements clarification instead of papering over it in `design.md`

### Step 7: Finalize Design Document

1. **Write Final Design, Persistent Artifacts, and Research Log**:
   - Confirm every `Mode: modify` path from Persistent References is already updated (Step 4)
   - Write `docs/specs/$1/design.md` only after the design review gate passes
   - On greenfield (Step 2.0 skipped): ensure Overview includes `_Gap analysis: skipped (greenfield per brief Current State)._` and do **not** create gap-only `research.md`
   - Persist any `research.md` updates that support the finalized design (brownfield gap + discovery; greenfield only if a research log was created)
   - Do not create empty gap-only `research.md` on greenfield
   - Do **not** invent a post-implementation "create contracts" skill or phase — contracts/architecture/ADR updates happen here

2. **Update Metadata** in spec.json:
   - Set `phase: "design-generated"`
   - Set `approvals.design.generated: true`
   - Update `updated_at` timestamp

## Critical Constraints
 - **Type Safety**:
   - Enforce strong typing aligned with the project's technology stack.
   - For statically typed languages, define explicit types/interfaces and avoid unsafe casts.
   - For TypeScript, never use `any`; prefer precise types and generics.
   - For dynamically typed languages, provide type hints/annotations where available (e.g., Python type hints) and validate inputs at boundaries.
   - Document public interfaces and contracts clearly to ensure cross-component type safety. Authoritative long-lived contracts live in `docs/contracts/**`; `design.md` holds excerpts + Persistent References.
- **Requirements Traceability IDs**: Use numeric requirement IDs only (e.g. "1.1", "1.2", "3.1", "3.3") exactly as defined in requirements.md. Do not invent new IDs or use alphabetic labels.
- **Persistent contracts at design time**: Index → related Read → merge/create → Persistent References in draft → review gate → write `design.md`. Never defer contract creation to a post-implementation phase. Never bulk-read all of `docs/contracts/` or `docs/architecture/`.
- **In-flight parallel specs**: When `roadmap.md` lists other incomplete specs, diff-merge only contract/architecture files this spec's Boundary Commitments own. If another in-flight spec already owns the same public surface, **stop and ask** — do not rewrite that file wholesale.
- **Greenfield**: never run gap-analysis sub-agents; never invent gap `research.md` content.
- **Brownfield**: gap runs once in Step 2.0; discovery must reuse `research.md` instead of duplicating codebase survey.
</instructions>

## Output Description

**Command execution output** (separate from design.md content):

Provide brief summary in the language specified in spec.json:

1. **Status**: Confirm design document generated at `docs/specs/$1/design.md`
2. **Gap Analysis**: brownfield completed / greenfield skipped
3. **Discovery Type**: Which discovery process was executed (full/light/minimal)
4. **Persistent Contracts**: Which `docs/contracts/**` / `docs/architecture/**` / ADR paths were created or modified (or **No contract changes**)
5. **Key Findings**: 2-3 critical insights from `research.md` (if any) that shaped the design
6. **Review Gate**: Confirm the design review gate passed
7. **Next Action**: Continue to `/sdd-validate-design-qa $1` (orchestrated) or review `design.md`
8. **Research Log**: Confirm `research.md` updated, or note that none was needed (greenfield)

**Format**: Concise Markdown (under 200 words) - this is the command output, NOT the design document itself

**Note**: The actual design document follows `docs/settings/templates/specs/design.md` structure.

## Safety & Fallback

### Error Scenarios

**Requirements Not Generated**:
- **Stop Execution**: Cannot proceed without generated requirements
- **User Message**: "Requirements not yet generated. Run requirements phase first."
- **Suggested Action**: "Run `/sdd-spec-requirements $1` first"

**Missing Requirements**:
- **Stop Execution**: Requirements document must exist
- **User Message**: "No requirements.md found at `docs/specs/$1/requirements.md`"
- **Suggested Action**: "Run `/sdd-spec-requirements $1` to generate requirements first"

**Template Missing**:
- **User Message**: "Template file missing at `docs/settings/templates/specs/design.md`"
- **Suggested Action**: "Check repository setup or restore template file"
- **Fallback**: Use inline basic structure with warning

**Steering Context Missing**:
- **Warning**: "Steering directory empty or missing - design may not align with project standards"
- **Proceed**: Continue with generation but note limitation in output

**Invalid Requirement IDs**:
  - **Stop Execution**: If requirements.md is missing numeric IDs or uses non-numeric headings (for example, "Requirement A"), stop and instruct the user to fix requirements.md before continuing.

**Spec Gap Found During Design Review**:
- **Stop Execution**: Do not write a patched-over `design.md`
- **User Message**: "Design review found a real spec gap or ambiguity that must be resolved before design can be finalized."
- **Suggested Action**: Clarify or fix `requirements.md`, then re-run `/sdd-spec-design $1`

### Next Phase: Task Generation

**If Design generated**:
- Review generated design at `docs/specs/$1/design.md`
- Then `/sdd-spec-tasks $1` to generate implementation tasks

**If Modifications Needed**:
- Provide feedback and re-run `/sdd-spec-design $1`
- Existing design used as reference (merge mode)
