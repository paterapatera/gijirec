# Cursor Task bindings for sdd-spec-design

Follow the already-loaded Cursor `sdd-spec-design` SKILL.md. Do not `Read` that SKILL.md again if it is attached or inlined. Domain rules stay in `.agents/skills/sdd-spec-design/`.

## Tool

Use the **Task** tool. Required every call: `description`, `prompt`. Fresh research: `subagent_type`, `model`. Parallel gap+external: `run_in_background: true` on each call in **one** parent message.

Never `resume` research agents. Never `resume` an explore id onto a writer step.

**Model:** resolve once from [../model-pin.yaml](../model-pin.yaml). `explore` uses `roles.explore` if set, else `slug`. External research uses `roles.research` if set, else `slug`. Do not parse `model-pin.md` for values. Do not hardcode slugs here.

Put **thoroughness** in the explore `prompt` (not a Task JSON field): `quick` | `medium` | `very thorough`.

## When to dispatch

Read greenfield.md first. Ambiguous → not greenfield (run gap).

| Situation | Binding |
|-----------|---------|
| Greenfield | **No** gap Task. Skip to classify + discovery |
| Brownfield Step 2.0 | `explore` (codebase gap). Add `generalPurpose` only if external deps need docs not in-repo |
| Minimal (greenfield simple) | **No** Task |
| Light | Reuse `research.md` if present. Else optional `explore` for uncovered integration points only |
| Full | Reuse gap `research.md` for codebase. `generalPurpose` for external topics **not** already logged. Do not re-explore the same questions |
| Step 3 Synthesis and after | **No** Task |

Forbidden: `bugbot`, `security-review`, `ci-investigator`. Do not use `explore` to write `design.md`. Fast models: only if `model-pin.yaml` allows them.

If Task is unavailable (including nested under an orchestrator skill Task): run the same questions in-parent with Grep/Read/WebSearch. Still write `research.md` in the parent. Report that explore dispatch was skipped.

## Role table

| Role | When | `subagent_type` | Thoroughness (in prompt) | `model` | `run_in_background` |
|------|------|-----------------|--------------------------|---------|---------------------|
| Gap codebase | Brownfield 2.0 | `explore` | `medium`; `very thorough` if `complexity_tier` is `L` or scale is complex | pin | `true` if launched with external research in the same message |
| Gap / discovery external | Brownfield or Full/Light when APIs/docs are not in `research.md` | `generalPurpose` | n/a | pin | `true` if launched with explore in the same message |
| Leftover codebase | Light/Full and `research.md` does **not** cover the question | `explore` | `quick` or `medium` | pin | omit / false unless parallel with external |

## Prompt envelope

Subagents **do not** see this conversation. They must not write spec artifacts.

### Shared prefix

```text
You are a Cursor Task subagent. You have no parent chat history.
Return a findings summary only (max 150 lines). Do not write design.md, research.md, contracts, or spec.json.
Do not glob-bulk-Read docs/contracts/** or docs/architecture/**. Index → named paths only if a path is in the brief below.
```

### Explore (gap / codebase)

State thoroughness explicitly: `Thoroughness: medium` (or `quick` / `very thorough`).

Then include: feature name, `brief.md` Current State excerpt, requirement IDs in scope, questions to answer (current modules, patterns, integration surfaces, extend vs new), ignore globs if useful.

Ask for the gap-analysis checklist distilled: Requirement-to-Asset map tags, options A/B/C one-liners, effort/risk — **information over decisions**.

```text
## Findings summary
- AREA: gap-codebase
- KEY POINTS:
- PATHS: <files/modules>
- GAPS: Missing | Unknown | Constraint
- OPTIONS: A/B/C one line each
- EFFORT: S|M|L|XL — <one line>
- RISK: High|Medium|Low — <one line>
- RESEARCH NEEDED:
```

`description`: e.g. `Explore auth codebase`.

### External research (`generalPurpose`)

Include: stack from `tech.md` (short excerpt), specific APIs/libraries, questions. Instruct WebSearch/WebFetch; official docs over blogs.

```text
## Findings summary
- AREA: external
- KEY POINTS:
- SOURCES: <urls>
- CONSTRAINTS: versions, auth, rate limits
- RESEARCH NEEDED:
```

`description`: e.g. `Research billing API`.

## Parent after return

1. Parse `## Findings summary` only. If missing, one fresh re-dispatch asking for that block.
2. Parent writes/appends `docs/specs/<feature>/research.md` (canonical template). Do not let the subagent write it.
3. Synthesize in parent (canonical Step 3) — no Task.
4. Mention agents as `[Gap explore](agent_id)` / `[External research](agent_id)`.
