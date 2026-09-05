# Light Discovery Process

## When to use (activation)

Run Light discovery when Step 2.1 maps to Light (or Integration-focused light):

| Codebase | Scope scale | Process |
| -------- | ----------- | ------- |
| greenfield | standard | Light |
| brownfield | simple | Gap (2.0) + Light |
| brownfield | standard | Gap + Light |
| extension | any | Gap + Integration-focused Light (this file) |

For **greenfield + simple**, use `design-discovery-minimal.md` instead — not this file.

## Objective
Quickly analyze integration requirements, patterns, and dependencies without full WebSearch / architecture-option sweeps.

## Reuse `research.md` from gap analysis

When `research.md` already contains Step 2.0 gap findings, prefer those sections for extension points and integration surfaces. Do not re-run a full codebase survey for the same questions — light Grep only for remaining gaps.

## Focused Discovery Steps

### 1. Extension / Integration Point Analysis
**Identify Integration Approach**:
- Locate existing extension points or interfaces (brownfield / extension)
- For greenfield standard: identify seams implied by brief Approach and steering `structure.md`
- Determine modification or create scope (files, components)
- Check for existing patterns to follow
- Identify backward compatibility requirements when extending

### 2. Dependency Check
**Verify Compatibility**:
- Check version compatibility of new dependencies
- Validate API contracts haven't changed
- Ensure no breaking changes in pipeline

### 3. Quick Technology Verification
**For New Libraries / Named APIs**:
- Prefer at most a few targeted WebFetch calls to official docs (avoid broad WebSearch sweeps)
- Verify basic usage patterns
- Check for known compatibility issues
- Confirm licensing compatibility
- Record key findings in `research.md` (technology alignment section) when material

### 4. Integration Risk Assessment
**Quick Risk Check**:
- Impact on existing functionality (brownfield)
- Performance implications
- Security considerations (do not drop PAT / origin / auth constraints even on simple scale)
- Testing requirements

### 5. Simple-scope lean output (when scale is simple under brownfield/extension)
When Axis B is **simple** (or `complexity_tier: S`):
- Prefer a short findings summary over a full research log rewrite
- Do not invent extension-scenario tables or multi-diagram architecture options
- Defer elaborate file trees to design.md size guards (top-level + `src/` only)

## When to Escalate to Full Discovery
Switch to full discovery if you find:
- Significant architectural changes needed
- Complex external service integrations / multi-service coupling
- Scope scale reclassified to **complex** (or `complexity_tier: L`)
- Unknown or poorly documented dependencies that block design

Do **not** escalate solely because the feature is greenfield.

## Output Requirements
- Clear integration / create approach (note boundary impacts in `research.md` when present)
- List of files/components to create or modify
- New dependencies with versions
- Integration risks and mitigations
- Testing focus areas
