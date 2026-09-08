---
name: sdd-spec-design
description: >-
  Cursor runtime for technical design generation. Uses explore Tasks for
  brownfield gap and codebase research, then synthesizes design in the parent.
  Use when the user invokes /sdd-spec-design, design generation, gap analysis,
  or after approved requirements in AI-DLC. Domain procedure stays in
  .agents/skills/sdd-spec-design; this skill is the Cursor Task binding.
  Target spec is the required first argument.
disable-model-invocation: true
---

# sdd-spec-design (Cursor runtime)

Cursor **binding**, not a second domain spec.

| Layer | Path | Role |
|-------|------|------|
| Domain (canonical) | `.agents/skills/sdd-spec-design/SKILL.md` + `rules/` | Gap, discovery, synthesis, contracts, draft |
| Greenfield rule | `.agents/skills/sdd-orchestrate/rules/greenfield.md` | Ban gap Tasks on greenfield |
| Cursor runtime | this file + [cursor-bindings.md](cursor-bindings.md) | `explore` / research `Task` |
| Model pin | [../model-pin.yaml](../model-pin.yaml) | Subagent `Task.model` (docs: [../model-pin.md](../model-pin.md)) |

This file governs dispatch. `.agents/skills/sdd-spec-design` is the procedure the parent Reads as a file — not a second attached Cursor skill. Do not @-attach or inline that SKILL.md as a catalog skill. Domain rules still come from the canonical SKILL.md. Do not rewrite those rules here.

## Load order

This SKILL.md is already in context when attached or inlined. Do **not** `Read` it again.

1. Read `.agents/skills/sdd-spec-design/SKILL.md` through Step 1, then follow that skill's later steps.
2. Before **any** gap or codebase `Task`, read `.agents/skills/sdd-orchestrate/rules/greenfield.md`.
3. Before the first research `Task`, read [cursor-bindings.md](cursor-bindings.md) and [../model-pin.yaml](../model-pin.yaml). Do not parse [../model-pin.md](../model-pin.md) for values.
4. Read `rules/gap-analysis.md` / discovery-*.md only when the canonical step requires them.

## Cursor overrides

Replace “spawn gap / codebase / research sub-agent” with [cursor-bindings.md](cursor-bindings.md).

- **Greenfield:** no gap `Task`, no gap-only `research.md`. One Overview line as canonical.
- **Minimal** discovery: no research `Task`. Optional ≤1 Grep in parent.
- **Brownfield Step 2.0:** `explore` for codebase gap. Optional `generalPurpose` for external deps. Parent writes `research.md`.
- **Light / Full:** do **not** re-`explore` the same codebase questions already in `research.md`. Extra `explore` only for uncovered paths; external research via `generalPurpose` + WebSearch/WebFetch.
- **Synthesis, contracts, draft, review gate, `design.md`:** parent only. Never Task.
- Subagents return a **findings summary** (≤150 lines), not raw dumps. They do not write `design.md` / `research.md` / contracts.
- Do not poll Task with AwaitShell.

## Dispatch map (summary)

Full table: [cursor-bindings.md](cursor-bindings.md).

| Role | `subagent_type` | `model` | Continuity |
|------|-----------------|---------|------------|
| Gap / leftover codebase | `explore` | pin (`slug` or `roles.explore`) | Always fresh |
| External / API research | `generalPurpose` | pin (`slug` or `roles.research`) | Always fresh |

`model` comes from [../model-pin.yaml](../model-pin.yaml). Never copy a slug into this file.

When mentioning a running or finished subagent in the parent reply, link `[Name](agent_id)`.
