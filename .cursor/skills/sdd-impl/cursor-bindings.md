# Cursor Task bindings for sdd-impl

Follow the already-loaded Cursor `sdd-impl` SKILL.md. Do not `Read` that SKILL.md again if it is attached or inlined. Domain rules stay in `.agents/skills/sdd-impl/SKILL.md`.

## Tool

Use the **Task** tool. Required every call: `description`, `prompt`. Optional as specified below: `subagent_type`, `model`, `resume`, `run_in_background`.

Do **not** pass `model` when `resume` is set (prior model is reused).

**Model:** resolve once from [../model-pin.yaml](../model-pin.yaml). Use that value wherever this table says `pin`. Do not parse `model-pin.md` for values. Do not hardcode slugs here.

## Role table

| Role | When | `subagent_type` | `model` | `resume` | `run_in_background` |
|------|------|-----------------|---------|----------|---------------------|
| Implementer (fresh) | First batch of the run; any dispatch after debug `RETRY_TASK` | `generalPurpose` | pin | omit | `true` if this is one of several `(P)` implementers in the same parent message; else omit / false |
| Implementer (sticky) | Previous batch `APPROVED` → next batch; reviewer `REJECTED` rounds 1–2; mechanical FAIL remediation | `generalPurpose` | omit | previous implementer `agent_id` for **this lineage** | same as fresh |
| Reviewer | After parent mechanical checks all PASS | `generalPurpose` | pin | omit always | omit / false (one reviewer per completed batch) |
| Debugger | `BLOCKED`; unresolved `NEEDS_CONTEXT`; mechanical FAIL after review-round budget; `REJECTED` after 2 remediations | `generalPurpose` | pin | omit always | omit / false |

Forbidden in this loop: `bugbot`, `security-review`, `ci-investigator`, `explore`, `cursor-guide`. Fast models: only if `model-pin.yaml` allows them (`forbid_fast: false`).

If `resume` fails (agent gone / still running without `interrupt`): re-dispatch **fresh** with the canonical **pseudo-sticky** payload (changed paths, Implementation Notes, next-batch excerpts). Never start the next happy-path batch with an empty prompt.

## Continuity matrix

Canonical sticky/fresh table, bound to Task:

| Situation | Binding |
|-----------|---------|
| First batch, or after debug RETRY | New Task. Record returned `agent_id` as this lineage's implementer |
| Happy-path next batch | `resume: <implementer_id>` + next-batch envelope (still include Spec Excerpts; do not assume memory) |
| In-batch remediation (REJECTED / mechanical FAIL, rounds 1–2) | `resume: <implementer_id>` + feedback + `MECHANICAL_RESULTS` / Review Verdict |
| `(P)` parallel Waves | **One implementer id per Wave/batch**. Never resume lineage A onto lineage B |
| Reviewer | New Task every time. Do not resume implementer or debugger |
| Debugger | New Task every time. Do not resume anyone |
| Post-debug implementer | New Task (fresh). New lineage id |

Store in parent notes (not in `tasks.md`): `lineage → implementer_agent_id`. Discard verbose subagent prose after parsing; keep the one-line batch summary required by the canonical skill.

## `(P)` parallel recipe

When the canonical `(P)` contract holds (different `_Boundary:_`, closed Depends, non-overlapping paths):

1. One **parent message** with **multiple** implementer `Task` calls.
2. Each call: `run_in_background: true`, own prompt / excerpts / `description`.
3. Do not `resume` across those concurrent calls.
4. Do not poll with AwaitShell. Wait for Task completion notifications.
5. Per finished batch: parent mechanical → reviewer Task → selective commit (Wave order or completion order).
6. On git conflict or overlapping staged paths: **stop for human**. No force-merge, no `git add -A`.
7. After the parallel set finishes or aborts: re-read `tasks.md` before forming the next set.

If any `(P)` condition is unclear → serial (lowest ready Wave). Hosts that cannot run concurrent Tasks: serial, still honoring `(P)` as dispatch order — never informational-only.

## Prompt envelope

Subagents **do not** see this conversation. `prompt` must be self-contained.

### Shared prefix (every role)

```text
You are a Cursor Task subagent. You have no parent chat history.
Follow the protocol in the template below. Return only the required structured block at the end.
Do not update tasks.md. Do not create git commits.
Spec Excerpts below are authoritative. Do not Read requirements.md / design.md / docs/architecture/** in full.
```

Then append, in order:

1. **Template body** (full file):
   - Implementer → `.agents/skills/sdd-impl/templates/implementer-prompt.md`
   - Reviewer → `.agents/skills/sdd-impl/templates/reviewer-prompt.md`
   - Debugger → `.agents/skills/sdd-impl/templates/debugger-prompt.md`
2. **Batch payload** (canonical fields): feature name, ordered task ids/texts, `_Boundary:_`, Spec Excerpts (`### Requirements` / `### Design` / `### Contracts` when related), spec paths as location only, `FEATURE_FLAG`, parent validation commands, Implementation Notes as allowed.
3. **Role extras**:
   - Sticky implementer: previous-batch changed paths + one-line prior summary.
   - Remediation implementer: reviewer `REMEDIATION` or `MECHANICAL_RESULTS` failures.
   - Reviewer: parent `MECHANICAL_RESULTS` + implementer `## Status Report` (reference only) + `git diff` is the reviewer's first action.
   - Debugger: error / blocker, current `git diff`, reviewer findings if any. After debug RETRY, next implementer gets `FIX_PLAN`, `NOTES`, current `git diff` — not the failed implementer session.
4. **Return contract** (repeat the exact heading):
   - Implementer: `## Status Report` with `- STATUS: READY_FOR_REVIEW | BLOCKED | NEEDS_CONTEXT`
   - Reviewer: `## Review Verdict` with `- VERDICT: APPROVED | REJECTED`
   - Debugger: `## Debug Report` with `- NEXT_ACTION: RETRY_TASK | BLOCK_TASK | STOP_FOR_HUMAN`

Do **not** tell the subagent to "open the skill and figure it out" without embedding the template. Optional one-liner is allowed: `Governing protocol: sdd-review` / `sdd-debug` — still embed the template.

Line budgets and NEEDS_CONTEXT re-excerpt: follow the canonical skill. Re-dispatch once with named missing headings; prefer `resume` for that one-shot on implementer.

## Parent vs subagent ownership

| Work | Who |
|------|-----|
| Resolve `<feature>`, mode selection, Wave/batch formation, Spec Excerpts cut | Parent |
| TDD implementation, `RED_PHASE_OUTPUT`, in-boundary edits | Implementer Task |
| `TEST_COMMANDS`, TBD/secrets/boundary/RED mechanical checks | Parent (before reviewer) |
| Judgment review vs Spec Excerpts + `git diff` | Reviewer Task |
| Root-cause + `FIX_PLAN` | Debugger Task |
| Mark `[x]`, selective `git add <paths>`, commit, Implementation Notes | Parent |
| `/sdd-validate-impl` + `FEATURE_GO` | Parent (autonomous completion) |

Never give a subagent commit rights or `tasks.md` updates.

## Parse and mention

- Ignore prose outside the required block. Missing/ambiguous `STATUS` / `VERDICT` / `NEXT_ACTION` → one re-dispatch asking for the block only (implementer: prefer `resume`).
- In the parent user-visible reply, mention subagents as `[Implementer](agent_id)` / `[Reviewer](agent_id)` / `[Debugger](agent_id)`.

## `direct` / Task-unavailable fallback

- `direct` / manual: parent implements; still use a **fresh reviewer Task** at selection end when Task exists.
- If Task is unavailable: canonical Manual Mode / in-parent review. Report that Cursor subagent dispatch was skipped.
