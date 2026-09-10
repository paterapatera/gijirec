# Sync Extensions (Extended Sync, Retention, Spec Cleanup)

Load during **Sync Mode** only. Do not apply during Bootstrap.

**Project-first**: Read loaded steering for Close / doc sync / completion sections before using conventions below.

---

## Extended Sync Scope Resolution

### Priority 1 — Project steering (if present)

Search loaded steering for sections whose title or purpose indicates:

- feature / implementation **close** workflow
- **doc sync scope** (table or bullet list of doc areas)
- **completion status** rules (what counts as "done")

Use only what the project defined. Do not invent project-specific paths beyond that section.

### Priority 2 — Convention-based fallback

If Priority 1 is missing or incomplete, probe the workspace (Glob / Read index files only; no full-tree read):

| If exists | Minimal check |
|-----------|----------------|
| Repository root `README.md` | Purpose, setup, quality commands still match project |
| `docs/architecture/` | Boundaries / ADR index not contradicting code |
| `docs/contracts/` + README index | New public surfaces indexed |
| `docs/settings/templates/` | Template shape still matches live docs |
| `docs/steering/roadmap.md` or `product.md` | Completion flags vs `tasks.md` completion when specs exist |

### Priority 3 — Opt-out

User passed `steering only` / `--steering-only` on invocation → skip Extended Sync targets, Spec knowledge retention, and Completed spec cleanup; run steering-only sync.

---

## Completion Status Rules (when `docs/specs/` exists)

- **Done** for a feature: every task in `docs/specs/<feature>/tasks.md` is complete per Task hierarchy completion below, and no `_Blocked:_`
- Do **not** mark incomplete from `spec.json` `phase` alone
- Do not add fields to `spec.json` that are not already used in that project

### Task hierarchy completion (tasks.md)

When judging whether a task line counts as complete:

1. Parse task lines matching checkbox + numbered prefix (e.g. `- [ ] 1. Title`, `- [x] 1.1 Subtask`)
2. A **parent** line (e.g. `[ ] 1. xxxxx`) counts as **complete** if **all direct children** at the next numbering level (e.g. `[x] 1.1 …`, `[x] 1.2 …`) are complete — **even when the parent checkbox is still `[ ]`**
3. Apply recursively: a child with its own sub-children (e.g. `1.1.1`) is complete only when that subtree is complete
4. **Leaf** tasks (no numbered sub-tasks under that line) require their own `[x]` to count as complete
5. A parent is **not** complete if any direct child subtree remains incomplete

**Example**: `[ ] 1. Batch API` with `[x] 1.1 Handler`, `[x] 1.2 Tests` → treat `1.` as complete for feature-done / deletion-candidate checks.

**Feature done**: All top-level task subtrees in `tasks.md` are complete and no `_Blocked:_` anywhere in the file.

---

## Spec Knowledge Retention

**When**: Sync step 6. Only for features judged **done** per Completion Status Rules.

**Purpose**: Before `docs/specs/<feature>/` is deleted, move durable knowledge from ephemeral spec files into steering.

### Check (JIT Read only)

1. Read `docs/specs/<feature>/tasks.md` → `## Implementation Notes` (and project-defined equivalent headings from steering, if any)
2. Extract cross-cutting durable knowledge: build/runtime pitfalls, external dependency constraints, non-obvious debug steps, warnings other features should reuse
3. For each item not already reflected in steering, append additively to the appropriate steering file:
   - Project steering Close / doc sync scope defines targets → use those
   - Else convention: technical constraints → `tech.md`; structure/boundaries → `structure.md`; verification → `testing.md`

### Do NOT

- Copy requirements, design, or task bodies into steering (ephemeral spec content)
- Delete spec directories in this step

### Retention complete

A feature has **retention complete** when every extracted Implementation Notes item is reflected in steering (or there were none to migrate).

---

## Completed Spec Cleanup

**When**: Sync step 7, after step 6. Skip when `--steering-only`.

**Purpose**: Integrate completed spec deletion into weekly `/sdd-steering` runs. **Always require human confirmation before delete** (except when zero candidates).

### Deletion candidate criteria

Convention defaults; project steering overrides if defined:

1. `tasks.md` satisfies Completion Status Rules (including Task hierarchy completion) and no `_Blocked:_`
2. **Retention complete** for that feature (step 6 finished; no pending Implementation Notes migration)
3. Delete only `docs/specs/<feature>/` — never `docs/architecture/`, `docs/contracts/`, `docs/architecture/adr/`, or other paths

### Flow

| Candidates | Action |
|------------|--------|
| **0** | Report "no deletion candidates". **Do not ask the user for confirmation** |
| **≥ 1** | List each candidate: feature name, path (`docs/specs/<feature>/`), one-line completion rationale. **Ask the user whether to delete** |
| User approves | Delete only approved `docs/specs/<feature>/` directories; **then** remove that name from `roadmap.md` (§ Roadmap prune); report results |
| User declines or no answer | Do not delete; report candidates and pending/declined status |

### Roadmap prune (post-deletion)

After approved deletion of `docs/specs/<feature>/`, if `docs/steering/roadmap.md` exists:

1. Remove that feature's line from `## Specs (dependency order)` and `## Existing Spec Updates` (checkbox form or `{name}[完了]：…`).
2. Remove the name from other specs' `Dependencies:`. If none remain, set `Dependencies: none`.
3. Do not leave a stub (`[x]`, `[完了]`) for the deleted spec.

Also drop any other roadmap names that have no `docs/specs/<name>/` (already-deleted leftovers). Preserve all other lines and sections.

### Do NOT

- Prompt for confirmation when candidate count is 0
- Delete before user approval
- Include features with incomplete retention in the candidate list
- Delete paths outside `docs/specs/<feature>/`
- Keep deleted spec names in `roadmap.md`
