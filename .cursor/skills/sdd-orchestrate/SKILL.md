---
name: sdd-orchestrate
description: >-
  Cursor runtime for AI-DLC orchestration. Routes flows, enforces phase gates,
  and dispatches role skills via Task without doing their work. Use when the
  user invokes /sdd-orchestrate, AI-DLC, spec/requirements/design updates, or
  implementation-only. Domain procedure stays in .agents/skills/sdd-orchestrate;
  this skill is the Cursor Task binding. Target spec is the required first argument.
disable-model-invocation: true
---

# sdd-orchestrate (Cursor runtime)

Cursor **binding**, not a second domain spec.

| Layer | Path | Role |
|-------|------|------|
| Domain (canonical) | `.agents/skills/sdd-orchestrate/SKILL.md` + `rules/` | Routing, flows, gates, rollback |
| Validate contract | `.agents/skills/sdd-validate-shared/` | `VERDICT:` / Phase Gate parse |
| Cursor runtime | this file + [cursor-bindings.md](cursor-bindings.md) | Skill → `Task` dispatch |
| Model pin | [../model-pin.yaml](../model-pin.yaml) | Subagent `Task.model` (docs: [../model-pin.md](../model-pin.md)) |

This file governs dispatch. `.agents/skills/sdd-orchestrate` is the procedure the parent Reads as a file — not a second attached Cursor skill. Do not @-attach or inline that SKILL.md as a catalog skill. Domain rules still come from the canonical SKILL.md and `rules/`. Do not rewrite those rules here.

## Load order

This SKILL.md is already in context when attached or inlined. Do **not** `Read` it again.

1. Read `.agents/skills/sdd-orchestrate/SKILL.md` Startup — then **only** the canonical files that step names (do not preload all `rules/`).
2. Before the first skill `Task`, read [cursor-bindings.md](cursor-bindings.md) and [../model-pin.yaml](../model-pin.yaml). Do not parse [../model-pin.md](../model-pin.md) for values.
3. When dispatching a skill that has a Cursor binding, the child reads that binding — not this orchestrator binding.

## Cursor overrides

Replace “Dispatch `/sdd-*`” with a **fresh** `Task` per [cursor-bindings.md](cursor-bindings.md).

- **Parent (調整者) only:** routing, `[調整者]` steps, human `[GATE]`, `spec.json` approvals / `ready_for_implementation`, Phase Handoff, PR Summary, rollback decisions.
- **Do not inline** requirements, design, tasks, validate, or impl work. One flow step → one skill `Task`.
- **Never** dispatch `/sdd-discovery`. Never chain generation flows into `/sdd-impl`.
- Unified validates stay **one** Task (`sdd-validate-requirements`, `sdd-validate-design-qa`). Do not split po/qa/sec or qa/arch/sec into sibling Tasks.
- Prefer `.cursor/skills/<skill>/SKILL.md` when it exists (`sdd-spec-design`, `sdd-impl`); else `.agents/skills/<skill>/SKILL.md`.
- Subagents have no parent history. Embed the envelope in `prompt`. Do not poll Task with AwaitShell.
- Artifact-only: pass feature name + flags + disk paths, not chat memory.

## Dispatch map (summary)

Full catalog: [cursor-bindings.md](cursor-bindings.md).

| Kind | `subagent_type` | `model` | Continuity |
|------|-----------------|---------|------------|
| Role skill (`/sdd-*` except 調整者-self) | `generalPurpose` | pin (`slug` or `roles.skill`) | Always fresh. Never `resume` across skills |

`model` comes from [../model-pin.yaml](../model-pin.yaml). Never copy a slug into this file.

When mentioning a running or finished subagent in the parent reply, link `[Name](agent_id)`.
