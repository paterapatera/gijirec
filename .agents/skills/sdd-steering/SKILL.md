---
name: sdd-steering
description: Manage docs/steering/ as persistent project knowledge; Sync includes Extended Sync, spec knowledge retention, and confirmed spec cleanup
metadata:
  shared-rules: "steering-principles.md, sync-extensions.md"
---


# SDD Steering Management

<background_information>
**Role**: Maintain `docs/steering/` as persistent project memory.

**Mission**:
- Bootstrap: Generate core steering from codebase (first-time)
- Sync: Keep steering and codebase aligned; Extended Sync for cross-doc drift; migrate durable spec notes; optional confirmed cleanup of completed specs
- Preserve: User customizations are sacred, updates are additive

**Success Criteria**:
- Steering captures patterns and principles, not exhaustive lists
- Code and doc drift detected and reported
- All `docs/steering/*.md` treated equally (core + custom)
- Completed spec knowledge retained in steering before spec directories are removed (with user confirmation)
</background_information>

<instructions>
## Scenario Detection

Check `docs/steering/` status:

**Bootstrap Mode**: Empty OR missing core files (product.md, tech.md, structure.md)  
**Sync Mode**: All core files exist

**Sync invocation flags** (parse from user message):

| Flag | Effect |
|------|--------|
| Default (no flag) | Full Sync: steering + Extended Sync + retention + spec cleanup |
| `steering only` / `--steering-only` | Steering-only sync: skip Extended Sync (steps 2–4 extended targets), Spec knowledge retention (step 6), Completed spec cleanup (step 7) |

---

## Bootstrap Flow

1. Load templates from `docs/settings/templates/steering/`
2. Analyze codebase (JIT):

#### Parallel Research

The following research areas are independent and can be executed in parallel:
1. **Product analysis**: README, package.json, documentation files for purpose, value, core capabilities
2. **Tech analysis**: Config files, dependencies, frameworks for technology patterns and decisions
3. **Structure analysis**: Directory tree, naming conventions, import patterns for organization

If multi-agent is enabled, spawn sub-agents for each area above. Otherwise execute sequentially.

After all parallel research completes, synthesize patterns for steering files.

3. Extract patterns (not lists):
   - Product: Purpose, value, core capabilities
   - Tech: Frameworks, decisions, conventions
   - Structure: Organization, naming, imports
4. Generate steering files (follow templates)
5. Load principles from `rules/steering-principles.md` from this skill's directory
6. Present summary for review

**Focus**: Patterns that guide decisions, not catalogs of files/dependencies.

---

## Sync Flow

1. Load all existing steering (`docs/steering/*.md`)
2. Load sync extensions from `rules/sync-extensions.md` in this skill's directory
3. **Resolve extended sync scope** (§ Extended Sync Scope Resolution in sync-extensions) — skip steps 3–4 extended targets and steps 6–7 when `--steering-only`
4. Analyze codebase for changes (JIT)
5. Detect drift (steering + resolved extended targets when not steering-only):
   - Steering ↔ code
   - Extended doc targets ↔ code (per resolved scope)
   - Completion metadata ↔ `docs/specs/<feature>/tasks.md` when specs exist (use Completion Status Rules + Task hierarchy completion)
6. Apply updates (additive, preserve user content)
7. **Spec knowledge retention** (§ Spec knowledge retention) — completed features only; skip when `--steering-only`
8. **Completed spec cleanup** (§ Completed spec cleanup) — list deletion candidates, confirm with user if any, delete on approval, then **prune those names from `roadmap.md`**; skip when `--steering-only`
9. Report per § Sync report (below)

**Update Philosophy**: Add, don't replace. Preserve user sections.

**Hard constraints** (never hardcode project-specific paths/commands in skill text; discover from steering or conventions):

- Extended Sync scope: project steering Close / doc sync sections first, then convention fallback in sync-extensions
- Spec deletion: `docs/specs/<feature>/` only; never without user approval when candidates ≥ 1; never prompt when candidates = 0
- Retention before deletion: features with pending Implementation Notes migration are not deletion candidates
- After approved spec deletion, remove that name from `docs/steering/roadmap.md` (own line and other specs' `Dependencies:`)

---

## Granularity Principle

From `rules/steering-principles.md` (in this skill's directory):

> "If new code follows existing patterns, steering shouldn't need updating."

Document patterns and principles, not exhaustive lists.

**Bad**: List every file in directory tree  
**Good**: Describe organization pattern with examples

</instructions>

## Tool guidance

- **Glob**: Find source/config files, `docs/specs/*/tasks.md`
- **Read**: Read steering, docs, configs, tasks.md
- **Grep**: Search patterns, `_Blocked:_`, Implementation Notes
- **Bash** with `ls` / `rm -rf`: Analyze structure; delete approved spec directories only in step 8 after user confirmation
- **AskQuestion** (or equivalent): Confirm spec deletion when candidates ≥ 1

**JIT Strategy**: Fetch when needed, not upfront.

## Output description

Chat summary only (files updated directly).

### Bootstrap:
```
✅ Steering Created

## Generated:
- product.md: [Brief description]
- tech.md: [Key stack]
- structure.md: [Organization]

Review and approve as Source of Truth.
```

### Sync:
```
✅ Steering Updated

## Changes:
- tech.md: React 18 → 19
- structure.md: Added API pattern

## Extended Sync
- Scope source: project steering § … | convention fallback | skipped (steering only)
- Targets checked: …
- Changes: … | none
- Spec knowledge retention: N migrated | none | skipped (steering only)
  - `<feature>`: <one-line summary> → `docs/steering/<file>.md`
- Completed spec cleanup:
  - Deletion candidates: N | 0 (no confirmation needed)
  - [if N > 0] Listed: `<feature>` (`docs/specs/<feature>/`), …
  - User confirmation: approved | declined | pending | n/a (0 candidates)
  - Deleted: `<feature>`, … | none
  - Roadmap prune: removed `<feature>` lines and `Dependencies:` refs | n/a (no roadmap / no deletions)

## Code Drift:
- Components not following import conventions

## Recommendations:
- Consider api-standards.md
```

## Examples

### Bootstrap
**Input**: Empty steering, React TypeScript project  
**Output**: 3 files with patterns - "Feature-first", "TypeScript strict", "React 19"

### Sync (full)
**Input**: Existing steering, completed feature with Implementation Notes, user did not pass `--steering-only`  
**Output**: Extended Sync summary, notes migrated to tech.md, one deletion candidate listed → user confirms → `docs/specs/<feature>/` removed and that name stripped from `roadmap.md`

### Sync (steering-only)
**Input**: `/sdd-steering --steering-only`  
**Output**: Steering drift updates only; Extended Sync / retention / cleanup marked skipped in summary

### Sync (parent task unchecked, children done)
**Input**: `tasks.md` has `[ ] 1. Parent` and `[x] 1.1`, `[x] 1.2` only  
**Output**: Feature treated as done for retention and deletion-candidate checks per Task hierarchy completion

## Safety & Fallback

- **Security**: Never include keys, passwords, secrets (see principles)
- **Uncertainty**: Report both states, ask user
- **Preservation**: Add rather than replace when in doubt
- **Spec deletion**: Never delete without user approval when there is at least one candidate; never delete outside `docs/specs/<feature>/`; after delete, remove that name from `roadmap.md`

## Notes

- All `docs/steering/*.md` loaded as project memory
- Templates, steering principles, and sync extensions are external for customization
- Focus on patterns, not catalogs
- "Golden Rule": New code following patterns shouldn't require steering updates
- Avoid documenting agent-specific tooling directories (e.g. `.agents/`, `.cursor/`, `.gemini/`, `.claude/`)
- `docs/settings/` content should NOT be documented in steering files (settings are metadata, not project knowledge)
- Light references to `docs/specs/` and `docs/steering/` are acceptable; avoid other `.kiro/` directories
- Weekly batch: one `/sdd-steering` run can cover Extended Sync → retention → confirmed spec cleanup
