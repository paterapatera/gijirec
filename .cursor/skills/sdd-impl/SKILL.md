---
name: sdd-impl
description: >-
  Cursor runtime for approved-task TDD implementation with Task subagent dispatch,
  sticky resume, and (P) parallel Waves. Use when the user invokes /sdd-impl,
  sdd-impl, implementation-only AI-DLC, or wave/strict batch implementation.
  Domain procedure stays in .agents/skills/sdd-impl; this skill is the Cursor
  Task binding. Target spec is the required first argument.
disable-model-invocation: true
---

# sdd-impl (Cursor runtime)

Cursor **binding**, not a second domain spec.

| Layer | Path | Role |
|-------|------|------|
| Domain (canonical) | `.agents/skills/sdd-impl/SKILL.md` | Modes, batch rules, mechanical checks, gates, commits |
| Templates | `.agents/skills/sdd-impl/templates/` | Implementer / reviewer / debugger prompts |
| Cursor runtime | this file + [cursor-bindings.md](cursor-bindings.md) | `Task` parameters, resume, parallel, prompt envelope |
| Model pin | [../model-pin.yaml](../model-pin.yaml) | Subagent `Task.model` (docs: [../model-pin.md](../model-pin.md)) |

This file governs dispatch. `.agents/skills/sdd-impl` is the procedure the parent Reads as a file — not a second attached Cursor skill. Do not @-attach or inline that SKILL.md as a catalog skill. Domain rules still come from the canonical SKILL.md. Do not rewrite those rules here.

## Load order

This SKILL.md is already in context when attached or inlined. Do **not** `Read` it again.

1. Read `.agents/skills/sdd-impl/SKILL.md` (Startup through the active mode). Skip orchestrator `rules/`.
2. Before the first subagent dispatch, read [cursor-bindings.md](cursor-bindings.md) and [../model-pin.yaml](../model-pin.yaml). Do not parse [../model-pin.md](../model-pin.md) for values.
3. Read a template only when constructing that role's `Task` prompt.

## Cursor overrides

Replace every canonical phrase like "spawn sub-agent" / "if the host supports resume" with this file.

- **`wave` / `strict`**: parent is controller only. Implementation, judgment review, and debug run in `Task` subagents. Do not fall back to in-parent implementation unless `Task` is unavailable.
- **`direct` / manual**: parent implements in this conversation. At selection end, still dispatch a **fresh** reviewer via `Task` (do not review your own code in-parent unless `Task` is unavailable).
- **Subagents have no parent history.** Always embed the template body plus Spec Excerpts in `prompt`. Do not rely on subagent skill auto-discovery of `sdd-review` / `sdd-debug`.
- **Do not poll** Task subagents with AwaitShell. Launch, continue or end the turn, then parse the return.
- Parent still owns: Spec Excerpts, `TEST_COMMANDS` / mechanical checks, `tasks.md` `[x]`, selective commits, `/sdd-validate-impl`, `FEATURE_GO`.

## Dispatch map (summary)

Full parameter table: [cursor-bindings.md](cursor-bindings.md).

| Role | `subagent_type` | `model` | Continuity |
|------|-----------------|---------|------------|
| Implementer | `generalPurpose` | pin (`slug`) | Fresh on first batch and after debug RETRY. `resume` same `agent_id` on happy-path next batch and remediation rounds 1–2 |
| Reviewer | `generalPurpose` | pin (`slug`) | Always fresh. Never `resume` an implementer or debugger id |
| Debugger | `generalPurpose` | pin (`slug`) | Always fresh. Never `resume` |

`model` comes from [../model-pin.yaml](../model-pin.yaml). Never copy a slug into this file. Do not use `bugbot`, `security-review`, `ci-investigator`, or `explore` in this loop.

`(P)` ready Waves: one parent message with multiple implementer `Task` calls (`run_in_background: true`). Separate lineage ids. Serial if boundaries, Depends, or paths overlap.

## Prompt construction

For each `Task`:

1. Read the matching template under `.agents/skills/sdd-impl/templates/`.
2. Build `prompt` as **template body + batch payload** (see bindings § Prompt envelope).
3. Set `description` to 3–5 words (e.g. `Implement batch 1.1`).
4. Tell the subagent the exact block to return (`## Status Report` / `## Review Verdict` / `## Debug Report`).
5. After return, parse **only** those structured fields (canonical parsing rules).

When mentioning a running or finished subagent in the parent reply, link `[Name](agent_id)`.
