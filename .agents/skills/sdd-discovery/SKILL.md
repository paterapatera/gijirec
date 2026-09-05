---
name: sdd-discovery
description: Entry point for new work. Determines action path, refines ideas, writes brief.md, syncs roadmap.md for cross-spec dependencies. Run standalone before /sdd-orchestrate.
---


# Discovery

<background_information>
Discovery is **capture and route**, not requirements authoring. Default mode is Capture + Route (minutes: memo → Path → brief on disk). Workshop is exceptional (opt-in). Canonical AI-DLC pre-step before `/sdd-orchestrate` (includes roadmap sync).

- **Success Criteria**:
  - Correct Path (A–E) identified
  - User intent captured in `brief.md` (or capture log) on disk — not left in chat only
  - Cross-spec dependency edges reflected in `roadmap.md` when present (additive; never lose `[x]`)
  - Actionable next command suggested for a **new chat** (`/sdd-orchestrate` for Path A/C/D/E) — never chained in this conversation
  - User can finish Confirm with **はい** or a correction (no abort-via-いいえ; no “実行しない”)
  - Total discovery interaction kept minimal unless Workshop mode explicitly triggered
</background_information>

<instructions>

## Critical Constraints (read first)

- Discovery is **capture and route**, not requirements authoring.
- Default mode is Capture + Route; Workshop is exceptional.
- Never write EARS acceptance criteria in discovery.
- Never spawn viability / research sub-agents in Capture + Route mode.
- Never skip disk write (`brief.md`) for Path C/D/E after はい.
- Deep context loading (codebase exploration sub-agents) is deferred to requirements (brownfield) or design — not discovery.
- Never generate Pros/Cons approach comparison tables in Capture + Route.
- Do not invent Scope In/Out for vague requests — trigger Workshop instead.
- **Path classification is independent of roadmap presence.** A single-scope new spec that depends on an existing/other spec **stays Path C** and merely gains a roadmap line. Do **not** promote it to Path D/E — promotion implies multi-spec generation (per-spec `/sdd-orchestrate` in dependency order).
- **Never rewrite `roadmap.md` wholesale** — append/update only, preserving completed items and prior phases.
- **`roadmap.md` is the single dependency source.** Do not add machine-readable dependency fields to `brief.md`; briefs keep their prose `Upstream / Downstream` as human context only.
- **Confirm is はい or a correction.** Never offer いいえ as abort. Never ask whether to run `/sdd-orchestrate` now. Never chain it after discovery. 「はい」= write artifacts and stop.
- **Next chat needs the files on disk.** Uncommitted Discovery writes do not appear in a new Git worktree. If the next conversation will use a different checkout, commit first (Step 8). Do not start a downstream spec in a checkout that lacks upstream `tasks.md`.

## Step 1: Lightweight Scan

Gather **only metadata** to determine the action path. Do NOT read full file contents yet.

- **Specs inventory**: Scan `docs/specs/*/spec.json` for `name`, `phase` fields and `approvals` status. Note feature names and their current status.
- **Steering existence**: Check which files exist in `docs/steering/` (product.md, tech.md, structure.md, roadmap.md). Do NOT read their contents yet.
- **Roadmap check**: If `docs/steering/roadmap.md` exists, read it. This contains project-level context (approach, scope, constraints, spec list) from a previous discovery session. Use it to restore project context.
- **Top-level structure**: List the project root directory to note key directories and files. Do NOT recurse into subdirectories.

This step should consume minimal context. If `specs/` is empty and no steering exists, note "greenfield project" and move to Step 2.

## Step 2: Route (core thinking)

Based on the user's request and the metadata from Step 1, determine which path applies. **This is discovery's primary judgment.**

**Path A: Existing spec covers this**
- The request is an extension, enhancement, or fix within an existing spec's domain
- Every meaningful part of the request fits that same spec boundary
- Any remaining small follow-up work can be handled directly without creating a new spec

**Path B: No spec needed**
- The request is a bug fix, config change, simple refactor, or trivial addition
- No meaningful part of the request needs a new or updated spec boundary
- The request does not need to update an existing spec either

**Path C: New single-scope feature**
- The request is new, doesn't overlap with existing specs, and fits in one spec

**Path D: Multi-scope decomposition needed**
- The request spans multiple domains or would produce 20+ tasks in a single spec

**Path E: Mixed decomposition**
- The request contains a mix of: existing spec extensions, one or more new spec candidates, and optional direct-implementation work
- Use this path only when at least one genuinely new spec boundary is needed

If Path A vs C is ambiguous, ask the user **one question only** (e.g.「既存 spec X に入れる？新 spec？」). Do not start a question series.

## Step 3: Select Mode

Default: **Capture + Route**.

Use **Workshop** only when **ANY** of:

- User explicitly asks for comparison, exploration, or「一緒に考えて」「比較して」
- Complexity would be L-tier (score ≥ 5 / Path D/E force-L per `sdd-orchestrate` complexity-tier rules) **AND** brief lacks Scope In/Out
- Path D/E **AND** decomposition / dependency order is not yet stated
- Contradictions in user input that cannot be resolved with one question
- Request is too vague to Capture (e.g.「GitLab まわりをなんとかしたい」with no Problem / Outcome / boundary hint)

**Otherwise:** stay in Capture + Route — skip Workshop entirely (no sequential question series, no approach comparison, no viability sub-agent).

Completed or concrete briefs (Problem / Approach / Scope present) are the **normal** case for Capture + Route, not a special short-circuit.

## Step 4: Capture

**Do NOT spawn codebase sub-agents by default.**

Load only:

- User message / existing `brief.md` draft
- Step 1 metadata (spec list, steering existence)
- If Path A: target spec's `brief.md` or `requirements.md` **headings only** (not full read)
- If user cited a file from 動作確認: read that file / snippet only

### Load rules (persistent docs)

- Default: do **not** load `docs/architecture/**`, `docs/contracts/**`, or ADR
- Boundary exploration only: `docs/architecture/README.md` / `docs/contracts/README.md` (index) → **related files only**
- Never glob-bulk-Read `docs/contracts/**` or `docs/architecture/**`. Never “read everything just in case”
- Defer deep contract work to design

Transform into brief sections per **Minimal brief requirements** below.

### Input patterns

**A — 動作確認・改善メモ:** Capture Trigger (when / what / what happened), pain (1–2 sentences), Desired Outcome (1–2 sentences), touched files/modules/spec names if any, rough In/Out (~3 bullets).

**B — 新規構想が既に具体的:** Transcribe/reshape user text into brief sections. Ask only when there is a **contradiction**.

**C — 曖昧:** Do not invent detail — enter Workshop (Step 3).

If Approach is obvious from user text, copy 1–2 sentences. **Do NOT** generate alternative approaches.

Time budget: treat this step as **note-taking**, not analysis.

### Workshop mode (only when Step 3 selected Workshop)

Shrink of former deep dialogue — still produce the **minimal** brief template:

- Sequential questions: **max 3** — Problem / Desired Outcome / Boundary candidate (one)
- Approach comparison: **L-tier only**, max **2** approaches (no full Pros/Cons essay)
- Viability / research sub-agent: **forbidden** (defer to requirements or design research)
- Path D/E with unset decomposition: this is the **only** required Workshop thinking — propose dependency-ordered split, then Confirm

### Path early-stop after Capture

| Path | After Capture |
| ---- | ------------- |
| **A** | Update brief or record「既存 spec X に追記」; proceed Confirm → Write as needed → Next (new chat): `/sdd-orchestrate <feature>` (要求更新) |
| **B** | Do **not** force a spec. Optional memo under `docs/captures/` only. Recommend direct implementation |
| **C** | Confirm → Write `brief.md` → Next (new chat): `/sdd-orchestrate <feature>` |
| **D/E** | If decomposition unset → Workshop first. Else Confirm → Write brief(s) + `roadmap.md` → Next (new chat): `/sdd-orchestrate <first-ready-feature>` (and other parallel-ready specs by name) |

## Step 5: Confirm

Ask **one question about the brief only**. Do **not** mention `/sdd-orchestrate` in the question. Do **not** ask a second question about starting requirements now. Do **not** offer いいえ as a way to abort without writing.

Show Path, feature name, and Scope In/Out so **はい** can be answered without extra typing.

Question (use this wording):

「この brief でよいか？ はい = ファイルに書く。修正があれば内容を書いてください。」

Two valid replies only:

**はい / yes / OK / よい:**

- Proceed to Step 6 Write.
- 「はい」means **write artifacts and stop**. It does **not** mean run `/sdd-orchestrate` in this conversation.

**修正内容** (Path, Scope In/Out, naming, or other brief fields):

- Apply the correction, re-show the brief, and ask the same question once more.
- Do not reopen Workshop unless they ask to explore/compare.

If the user says いいえ / no with **no** correction: do **not** abort. Ask once what to change, then wait for はい or a correction.

Orchestrate is never started from discovery.

## Step 6: Write

**CRITICAL: You MUST write these files to disk BEFORE suggesting any next command. Conversation text does not survive session boundaries.**

### Spec naming convention (mandatory)

A spec directory is named `<concept-slug>` — a short kebab-case concept (e.g. `user-edit`, `user-list`) — with an **optional** numeric prefix:

- **User-specified number** → prefix as `NNN-<concept-slug>`, zero-padded to at least 3 digits (e.g. issue #42 → `042-user-edit`). If that exact directory already exists, **stop and ask the user** — do not silently renumber.
- **No number given** → use bare `<concept-slug>`. Do **not** invent a number.

The `<concept-slug>` is always **required**. Do not create the same concept both with and without a numeric prefix. Cite the full directory name consistently in `brief.md`, `roadmap.md`, and `## Upstream / Downstream`.

Template reference: `docs/settings/templates/specs/brief.md`.

### Minimal brief requirements (all Paths that write a brief)

**Required sections:**

```markdown
# Brief: <feature-name>

## Trigger
[動作確認・依頼・バグ報告など、きっかけを 1–3 文]

## Problem
[誰のどんな痛みか — 1–3 文]

## Desired Outcome
[完了時に何が真になるか — 1–3 文]

## Scope
- **In**: [箇条書き。詳細でなくてよい]
- **Out**: [箇条書き。明示的に除外]

## Route
- **Path**: A | B | C | D | E
- **Rationale**: [1 文でなぜその Path か]
```

**Optional** (write if known; otherwise leave for requirements):

```markdown
## Approach
[採用方針 1–2 文。複数案の比較表は書かない]

## Current State
[緑地 / 既存実装 / 関連 spec]

## Upstream / Downstream
[依存のメモ。roadmap 同期は Step 7 / E1]

## Constraints
[分かっている制約のみ]
```

**Do not write in discovery** (defer to requirements): EARS / AC, detailed Boundary Candidates lists, Pros/Cons approach tables, viability findings.

**Path C:** Write `docs/specs/<feature-name>/brief.md`.

**Path D:** Write `docs/steering/roadmap.md` and `docs/specs/<feature>/brief.md` for every feature under `## Specs (dependency order)`.

Roadmap structure:

```markdown
# Roadmap

## Overview
[Project goal — 1–2 paragraphs]

## Scope
- **In**: […]
- **Out**: […]

## Constraints
[…]

## Specs (dependency order)
- [ ] feature-a -- [one-line]. Dependencies: none
- [ ] feature-b -- [one-line]. Dependencies: feature-a
```

Do **not** require Approach Decision / Rejected alternatives / Boundary Strategy sections unless Workshop produced them; keep roadmap capture-thin.

**Path E:** Same as Path D, plus:

```markdown
## Existing Spec Updates
- [ ] existing-feature-a -- [extension]. Dependencies: none

## Direct Implementation Candidates
- [ ] small-item-a -- [why direct]
```

Path E rules:

- `## Specs (dependency order)` = **new specs only**
- Existing extensions → `## Existing Spec Updates`
- No-spec work → `## Direct Implementation Candidates`
- Write `brief.md` only for **new** specs under Specs

**Path A:** Update the existing spec's brief (or append a short Trigger/Problem note) when useful; do not create a duplicate spec directory.

**Path B:** Do not create `docs/specs/<feature>/`. Optional: `docs/captures/<slug>.md` with Trigger/Problem/Outcome only.

**Re-entry** (`roadmap.md` already exists): Write the next new spec's `brief.md`. Update `roadmap.md` if scope/ordering changed; preserve completed items and prior phases.

After writing, verify files exist by reading them back.

## Step 7 / E1: Dependency Resolution & Roadmap Sync

Run this **after** Step 6 has written `brief.md` (and, for D/E, `roadmap.md`), **before** Step 8. This step only ever **adds or updates** roadmap entries.

Dependency detection is **bidirectional**: an edge must be recorded whether it is declared by the dependent spec (via its `Upstream`) or by the dependency spec (via its `Downstream`), and regardless of the order the two specs were discovered in. A dependency edge is always written in the canonical direction `dependent → dependency` (i.e. "dependent has Dependencies: dependency").

1. **Collect dependency edges.** Build a set of candidate edges, each oriented as `dependent → dependency`:
   - **From this session's specs.** For each spec authored or updated in this session, read the `## Upstream / Downstream` section of its `brief.md`:
     - each `Upstream` item yields the edge `thisSpec → upstreamItem`;
     - each `Downstream` item yields the edge `downstreamItem → thisSpec`.
   - **Re-scan existing specs (reverse-order catch-up).** Read the `## Upstream / Downstream` section of **every** `docs/specs/<name>/brief.md` and add the edge `<name> → upstreamItem` for each of its `Upstream` items. This is essential: a spec discovered **earlier** may have declared an upstream that did not resolve to a spec at the time, but resolves now because **this** session created that spec. Without this re-scan, discovering the dependency spec *after* its dependent (e.g. `user-edit` first, then `user-list`) would leave the edge unrecorded.

2. **Resolve to spec names.** Keep only edges where **both** endpoints resolve to a **spec**:
   - matches an existing `docs/specs/<name>/` directory, **or**
   - is another new spec produced in this same session.

   Classify every **unresolved** endpoint before discarding it — never drop silently by default:
   - **Clearly external** (an external system, library, framework, service, or infrastructure): drop silently. These are not ordering dependencies.
   - **Spec-like but unresolved** (reads like a feature/spec reference — e.g. a kebab-case concept, a bare number, or a partial name — yet matches no `docs/specs/<name>/` directory and no in-session spec): **do not drop silently. Surface it to the user as a warning** and hold the edge. Report each occurrence with its source, e.g. "`docs/specs/<dependent>/brief.md` の Upstream/Downstream にある『<endpoint>』は spec 依存に見えますが `docs/specs/` のどのディレクトリにも一致しません。正しい spec ディレクトリ名(例 `001-user-edit`)へ直すか、外部依存であることを明示してください。" Ask the user to correct the brief (or confirm it is external) before finalizing the roadmap. Never fabricate or fuzzy-guess a spec name to force a match.

   De-duplicate resolved edges (the same edge may be declared from both directions).

3. **Trigger.** If **≥ 1 edge** survives resolution:
   1. If `docs/steering/roadmap.md` is missing, create a **minimal** roadmap using the template below.
   2. For each resolved edge `dependent → dependency`, ensure a line for the **dependent** spec exists under `## Specs (dependency order)` with `Dependencies:` listing all its resolved dependency specs (comma-separated). If the line already exists, **merge the new dependency into its `Dependencies:` field in place** — never duplicate the line, never drop dependencies already listed.
   3. **Back-fill direct dependency specs only** (do not recurse into transitive dependencies): for each resolved dependency spec, add a line if absent. Mark it `[x]` if `docs/specs/<dependency>/spec.json` has `approvals.tasks.generated: true` **and** `docs/specs/<dependency>/tasks.md` exists; otherwise `[ ]`.
   4. Preserve every existing roadmap line, its `[x]`/`[ ]` status, and all other sections.

4. **No dependency.** If no edge resolves to a spec-to-spec pair (external-only, or genuinely independent), do **nothing** extra — Path C stays brief-only, no roadmap created.

### Minimal roadmap template (Path C-origin, first creation)

Use this only when `roadmap.md` does not yet exist and a Path C/A dependency triggers creation. Heavier sections (`## Approach Decision`, `## Boundary Strategy`, etc.) are written only when Workshop produced multi-scope planning — do not fabricate them here.

```
# Roadmap

## Overview
Incrementally grown from single-spec discovery. Project-level planning: TBD.

## Specs (dependency order)
- [x] <upstream-spec> -- <one-line description>. Dependencies: none
- [ ] <target-spec> -- <one-line description>. Dependencies: <upstream-spec>
```

## Step 8 / E2: Next Step

Suggest the next command for a **new conversation** and **stop**.

- Do NOT automatically run `/sdd-orchestrate` or spec generation.
- Do NOT ask 「今実行するか」 or any yes/no about chaining. Discovery is finished.
- Phrase as: 別チャットで次を実行: `/sdd-orchestrate <feature>` (or the Path-specific command below). Always include `<feature>`.

### Git / checkout

Uncommitted files do not appear in a new Git worktree. If the next chat will run in a different checkout, **commit before leaving this one**.

| Situation | Where the Discovery commit lands |
| --------- | -------------------------------- |
| **Path C / A** (single spec) | Feature branch. Do not put a brief-only commit on the default integration branch (`main` / `develop`) unless the user asks. |
| **Path D/E** with ≥1 spec whose `Dependencies:` is `none` (parallel-ready) | **One commit on the merge target** (`main` / `develop`). Later checkouts branch from that tip. |
| **Path D/E** chained only (`a → b`) | One shared branch/checkout is enough until a spec needs its own PR. |

Then start **only specs that are ready in that checkout**: `Dependencies: none`, or every listed upstream already has `tasks.md` + `approvals.tasks.generated` here (usually after those PRs merge). Independent ready specs may use parallel checkouts. Do **not** open a downstream checkout from the Discovery tip and run `/sdd-orchestrate` there — the guard will fail or see stale upstreams.

Keep the same Git checkout across later phase chats for a given spec. Do not create a new worktree per requirements/design/tasks/impl phase.

| Path | Next command |
| ---- | ------------ |
| **A** | `/sdd-orchestrate <feature>`（要求更新 / 設計更新 as appropriate） |
| **B** | Direct implementation — no spec; do not force `sdd-spec-*` |
| **C** | Default: `/sdd-orchestrate <feature-name>` (orchestrator picks S/M/L path). Manual phase control: `/sdd-spec-requirements <feature-name>` (M/L only). Explicit fast: `/sdd-orchestrate <feature-name> quick` or `/sdd-spec-quick <feature-name> --auto` |
| **D** | `/sdd-orchestrate <first-ready-feature>` — first spec in roadmap order that is ready in this checkout. Name every parallel-ready spec the user may start (`/sdd-orchestrate <a>`, `/sdd-orchestrate <b>`). Downstream waits until upstream is merged into the checkout it will use. |
| **E** | Same as D for new specs; existing-spec updates: `/sdd-orchestrate <existing-feature>` separately |

If Step 7 / E1 created or updated `roadmap.md`, additionally note that the dependency was recorded so `/sdd-orchestrate` can enforce upstream readiness.

If the decomposition is only existing-spec updates + direct implementation, do **not** use Path E — prefer Path A or Path B guidance.

</instructions>

## Safety & Fallback

**Roadmap Already Exists (re-entry)**:

- Read `roadmap.md` to restore project context before Capture
- Determine next spec from completed status
- Write `brief.md` for the next spec only (just-in-time)
- Update `roadmap.md` if scope/ordering changed; append new phases — do not overwrite completed content

**E1 / dependency sync**:

- Path A/B still resolve upstreams: if a Path A extension references another spec as upstream, record it in `roadmap.md` the same way. Path B (no spec) never produces a roadmap line.
- If `brief.md` was not written (Path A/B skipped file writes), skip Step 7 / E1 unless the request itself resolves to a spec dependency worth recording; when in doubt, prefer recording in `roadmap.md`.
- Circular dependency detected while updating roadmap (target lists an upstream that transitively depends on the target): stop and report the cycle instead of writing it.
