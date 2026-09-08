# Cursor Task bindings for sdd-orchestrate

Follow the already-loaded Cursor `sdd-orchestrate` SKILL.md. Do not `Read` that SKILL.md again if it is attached or inlined. Domain rules stay in `.agents/skills/sdd-orchestrate/`.

## Tool

Use the **Task** tool. Required every call: `description`, `prompt`. Fresh skill dispatch: `subagent_type`, `model`.

Do **not** `resume` a previous skill's `agent_id` onto a different skill. Re-runs after NO-GO / `fix` are also **fresh** (artifact-only).

**Model:** resolve once from [../model-pin.yaml](../model-pin.yaml). Use `roles.skill` if set, else `slug`. Do not parse `model-pin.md` for values. Do not hardcode slugs here.

## What stays in the parent

| Work | Who |
|------|-----|
| Resolve `<feature>`, flow, complexity tier, greenfield check | Parent |
| `[調整者]` steps (`spec.json` writes, guards, Terminal auto-approve) | Parent |
| Human `[GATE]` (`go` / `fix`) | Parent |
| Parse `VERDICT:` / Phase Gate `STATUS:` from report files | Parent |
| Rollback / 2× NO-GO stop | Parent |
| Path B (no orchestration) | Not this skill |

`verify-phase-gate` (タスク) and `verify-completion` (`FEATURE_GO`) are listed as `/sdd-*` steps in `flows.md` — dispatch them as skill Tasks below. Do not treat them as an excuse to inline other roles.

## Skill catalog

| Canonical step | Cursor entry to name in `prompt` | Notes |
|----------------|----------------------------------|-------|
| `/sdd-spec-quick` | `.agents/skills/sdd-spec-quick/SKILL.md` | S quick-path only. Do not also dispatch spec-requirements / design / tasks |
| `/sdd-spec-requirements` | `.agents/skills/sdd-spec-requirements/SKILL.md` | Child may nest explore for brownfield hints if Task is available |
| `/sdd-validate-requirements` | `.agents/skills/sdd-validate-requirements/SKILL.md` | Single Task. Optional `--only` as given. No sibling validate Tasks |
| `/sdd-spec-design` | `.cursor/skills/sdd-spec-design/SKILL.md` | Child owns explore gap/research per that binding |
| `/sdd-validate-design-qa` | `.agents/skills/sdd-validate-design-qa/SKILL.md` | Single Task. Optional `--only` as given |
| `/sdd-spec-tasks` | `.agents/skills/sdd-spec-tasks/SKILL.md` | |
| `/sdd-verify-phase-gate` | `.agents/skills/sdd-verify-phase-gate/SKILL.md` | Tasks terminal; skip for 要求/設計 when unified report already `STATUS: VERIFIED` |
| `/sdd-impl` | `.cursor/skills/sdd-impl/SKILL.md` | `実装のみ` only. Child owns implementer/reviewer/debugger Tasks |
| `/sdd-validate-impl` | `.agents/skills/sdd-validate-impl/SKILL.md` | |
| `/sdd-verify-completion` | `.agents/skills/sdd-verify-completion/SKILL.md` | After validate-impl GO (`FEATURE_GO`) |

Forbidden as orchestrator dispatches: `/sdd-discovery`, `bugbot`, `security-review`, `ci-investigator`. Do not launch `explore` from the orchestrator parent — spec-design / requirements children own research Tasks.

## Role table

| Role | When | `subagent_type` | `model` | `resume` | `run_in_background` |
|------|------|-----------------|---------|----------|---------------------|
| Skill dispatch | Each `/sdd-*` flow step that is not `[調整者]` / `[GATE]` | `generalPurpose` | pin | omit always | omit / false (serial steps). `true` only if Path D/E independent specs are explicitly parallel **and** each has its own checkout — default is one feature, serial |

Do not use `explore` for whole-skill dispatch (skills write artifacts). Fast models: only if `model-pin.yaml` allows them.

## Prompt envelope

Subagents **do not** see this conversation. `prompt` must be self-contained.

```text
You are a Cursor Task subagent. You have no parent chat history and no prior phase chat.
Execute exactly one skill for one feature. Artifact-only: trust docs/specs/<feature>/ and steering on disk, not implied chat decisions.

Feature: <feature>
Invocation: /sdd-<skill> <args including flags such as --auto --from-orchestrate --only …>
Workspace: <repo root>

1. Read this file first (Cursor binding if it exists, else canonical SKILL.md):
   <absolute or repo-relative path from the catalog>
2. Follow that skill fully. Load its rules only as that skill's Startup / Load order says.
3. You may nest Task calls if that skill's Cursor binding says so (spec-design explore, sdd-impl implementer).
4. Do NOT dispatch sibling orchestration skills. Do NOT set approvals.*.approved or ready_for_implementation (parent owns gates). You MAY write artifacts and generated flags the target skill requires.
5. Do NOT run /sdd-discovery. Do NOT start implementation unless this invocation is /sdd-impl.

Return exactly one block:
```

```text
## Skill Outcome
- SKILL: <name>
- STATUS: DONE | BLOCKED | FAILED
- VERDICT: <GO | NO-GO | MANUAL_VERIFY_REQUIRED | APPROVED | REJECTED | VERIFIED | NOT_VERIFIED | n/a>
- ARTIFACTS: <comma-separated paths written or updated>
- NOTES: <blockers, missing inputs, or one-line summary>
```

`description`: 3–5 words (e.g. `Generate requirements`, `Validate design`).

## Parse

- Prefer disk reports (`reviews/*-review.md`, `spec.json`, `design.md`) over the Outcome prose.
- Parse `VERDICT:` / Phase Gate `STATUS:` per `.agents/skills/sdd-validate-shared/contract.md` (read only when parsing).
- Missing Outcome block: one re-dispatch asking for `## Skill Outcome` only (fresh Task). Do not infer GO from chat.
- On `STATUS: BLOCKED` / `FAILED` or `VERDICT: NO-GO` → canonical `rules/rollback.md`. Do not auto-approve.

## Nested Task

`sdd-spec-design` and `sdd-impl` Cursor bindings launch child Tasks. The orchestrator waits for the **skill** Task to finish; it does not AwaitShell-poll children. If a skill Task cannot nest Task, it must follow that binding's in-process fallback and still return `## Skill Outcome`.

## Mention

In the parent user-visible reply, mention skill runs as `[Spec design](agent_id)` / `[Impl](agent_id)` etc.
