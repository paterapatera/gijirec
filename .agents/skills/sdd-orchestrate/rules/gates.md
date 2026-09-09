# Phase Gates

## Mechanical Gate (per validate/review)

Parse report files only — do not re-run analysis:

| Source | Parse field |
| ------ | ----------- |
| `reviews/*.md` from validate skills | `VERDICT: GO` \| `NO-GO` \| `MANUAL_VERIFY_REQUIRED` |
| `/sdd-review` | `APPROVED` \| `REJECTED` |
| `/sdd-validate-impl` | GO/NO-GO per that skill's output |

Report paths: see `../sdd-validate-shared/contract.md` (read only if parsing).

**VERDICT source (要求 / 設計):** unified `reviews/requirements-review.md` / `design-review.md` only — parse `VERDICT:` and `## Phase Gate` → `STATUS:`. Specs without the unified file are not GO (re-validate with the unified skill).

| Verdict | Orchestrator action |
| ------- | ------------------- |
| `GO` / `APPROVED` | Continue same-phase validates; when all pass → phase gate verification (below) |
| `NO-GO` / `REJECTED` | Rollback per `rollback.md` |
| `MANUAL_VERIFY_REQUIRED` | Stop; report to user until resolved |

## Phase Gate Verification (要求 / 設計 / タスク)

**要求 (unified):** After `/sdd-validate-requirements` writes `reviews/requirements-review.md` with `VERDICT: GO` and `## Phase Gate` → `STATUS: VERIFIED`, **Phase terminal** (handoff → end). Do **not** dispatch `/sdd-verify-phase-gate` for requirements in the orchestrated flow (standalone re-check still allowed).

**設計 (unified):** After `/sdd-validate-design-qa` writes `reviews/design-review.md` with `VERDICT: GO` and `## Phase Gate` → `STATUS: VERIFIED`, **Phase terminal** (handoff → end). Do **not** dispatch `/sdd-verify-phase-gate` for design in the orchestrated flow.

**タスク:** After generation, **before** terminal auto-approve:

1. Dispatch `/sdd-verify-phase-gate <feature> tasks`
2. Parse `STATUS: VERIFIED` | `NOT_VERIFIED` | `MANUAL_VERIFY_REQUIRED` (claim type `PHASE_GATE`)
3. Checklist: `../sdd-validate-shared/phase-gate.md`

| Result | Orchestrator action |
| ------ | ------------------- |
| `VERIFIED` | **Terminal auto-approve** (below) |
| `NOT_VERIFIED` | Rollback per `rollback.md` § Phase gate failures |
| `MANUAL_VERIFY_REQUIRED` | Stop; report gaps to user |

**Do not** use `/sdd-verify-completion` with `FEATURE_GO` for 要求 / 設計 / タスク — that claim type is for post-impl feature completion only. Inside `/sdd-impl`, use `BATCH` at each batch/selection completion gate (or `TASK` only for a single manual task); use `FEATURE_GO` only after `/sdd-validate-impl` GO at the end of `実装のみ`. Do **not** require verify-completion after every intermediate `APPROVED` while batch tasks remain unmarked.

## Phase Gate Table (`spec.json`)

| Phase | Pass condition | After readiness |
| ----- | -------------- | ------------------- |
| 要求 | `requirements.md` + `approvals.requirements.generated` + `/sdd-validate-requirements` GO + Phase Gate VERIFIED | **Phase terminal** → 次チャットで設計 |
| 設計 | `design.md` + `approvals.design.generated` + `/sdd-validate-design-qa` GO + Phase Gate VERIFIED | **Phase terminal** → 次チャットでタスク |
| タスク | `tasks.md` + `approvals.tasks.generated` + `/sdd-verify-phase-gate` VERIFIED | Set `ready_for_implementation: true` → **end orchestration (do not dispatch `/sdd-impl`)** |
| 仕様一式 (S) | all three `approvals.*.generated` + sanity review (or unified validates GO) | Set `ready_for_implementation: true` → **end orchestration** |
| 実装 | `/sdd-validate-impl` GO + `/sdd-verify-completion` (`FEATURE_GO`) VERIFIED | **End orchestration** — reached only via explicit `実装のみ` |

Requirements validate: single `/sdd-validate-requirements` (unified). Design validate: single `/sdd-validate-design-qa` (unified).

## Phase terminal（要求 / 設計）

After unified Phase Gate `VERIFIED`, **do not** wait for `go` and **do not** dispatch the next phase in this conversation:

1. `approvals.<phase>.generated` is already set by the generation skill
2. Emit **Phase Handoff**（below）
3. **Stop dispatching** — wait for the human, or end if they start a new chat

**Human review (no approval command):** The mechanical gate is not a substitute for a human reading the artifacts. After Handoff:

| User action | Meaning | Orchestrator action |
|-------------|---------|---------------------|
| Same chat: correction notes (natural language) | 問題あり | Stay in this phase. Apply notes → regenerate / re-validate. Do **not** start the next phase. No `fix` keyword required |
| New chat: `/sdd-orchestrate <feature>` | 問題なし（成果物を受け入れて次へ） | Resume next incomplete phase from disk (`routing.md`) |
| Same chat: asks to continue to the next phase | 拒否 | Tell them to open a **new** chat with `/sdd-orchestrate <feature>` (token hygiene) |

Do **not** prompt `Reply: go | fix`. Proceeding is the new-chat invoke; correcting is a same-chat reply.

Reason for the new chat: drop prior-phase conversation tokens. Same Git checkout. Do not create a new worktree per phase.

Path B: no `spec.json` gates — report changes at end only.

### Phase Handoff（必須フォーマット）

Emit one copy-friendly block. Language follows the spec (default ja). Distinct from **PR Summary Output** (tasks terminal).

**Required items:**

1. **終了したフェーズ** — `requirements` | `design`
2. **feature** — `<feature>`
3. **次にやること** — 成果物を確認する。問題があれば**このチャット**で修正指示。問題がなければ **同じ Git checkout** の新しいチャットで `/sdd-orchestrate <feature>`
4. **routing が選ぶ次フロー**:
   - 要求終了後 → 設計フェーズ
   - 設計終了後 → タスク生成
5. **読む成果物**（パス列挙）
6. **残リスク 1〜3 行** — 当該 unified review の受容残リスクから要約（再分析しない）
7. **禁止** — 「問題がない限り、このチャットの続きで次フェーズを続けないでください。次フェーズ用に新しい worktree を作らないでください」

**Template:**

````markdown
```markdown
## Phase Handoff

- **終了したフェーズ**: <requirements|design>
- **feature**: <feature>
- **次にやること**: 成果物を確認 → 問題があればこのチャットで修正指示 / 問題がなければ同じ checkout の新しいチャットで `/sdd-orchestrate <feature>`
- **routing が選ぶ次フロー**: <設計フェーズ | タスク生成>
- **読む成果物**:
  - `docs/specs/<feature>/spec.json`
  - `docs/specs/<feature>/requirements.md`
  - `docs/specs/<feature>/reviews/requirements-review.md`
  - （要求終了時・あれば）`docs/specs/<feature>/brief.md`
  - （設計終了時）`docs/specs/<feature>/design.md`
  - （設計終了時）`docs/specs/<feature>/reviews/design-review.md`
  - （設計終了時・あれば）`docs/specs/<feature>/research.md`
- **残リスク**:
  - <1〜3 lines from unified review>
- **禁止**: 問題がない限り、このチャットの続きで次フェーズを続けないでください。次フェーズ用に新しい worktree を作らないでください
```
````

### Exceptions（切断しない）

| ケース | 振る舞い |
|--------|----------|
| **S / quick-path** | 1 dispatch で要求+設計+タスク → Terminal auto-approve。Phase terminal なし |
| **フェーズ内**（生成 → validate → 機械ゲート前の修正往復） | **同一会話のまま** |
| **Phase Handoff 後の修正指示** | **同一会話のまま**（次フェーズへは進まない） |
| **validate NO-GO → rollback 再生成** | 同一フェーズ内。切らない |

## Resume

A new chat with `/sdd-orchestrate <feature>` on the **same Git checkout** means the human accepted the previous phase artifacts and wants the next incomplete phase (`routing.md` § Spec State Hints). **Artifact-only resume**: `spec.json` + `docs/specs/<feature>/` artifacts (and steering) only — not chat history or unwritten decisions. Same-chat correction notes after Handoff are **not** resume — stay in the current phase.

## Terminal auto-approve (タスク / 仕様一式)

After mechanical readiness (below), the orchestrator **auto-approves**:

**M/L — after tasks:**
1. `/sdd-spec-tasks` completed; `approvals.tasks.generated === true`
2. `/sdd-verify-phase-gate <feature> tasks` → `STATUS: VERIFIED`
3. **[調整者]** set `ready_for_implementation: true`, `phase: tasks-approved`
4. Emit **PR Summary Output**
5. End orchestration (do **not** dispatch `/sdd-impl`). After the PR Summary fence, emit the chat-only next-step line from § [AUTO] 仕様一式.

**S — after quick-path:**
1. `/sdd-spec-quick --auto --from-orchestrate` succeeded; all three `approvals.*.generated === true`
2. Sanity review (and optional unified validates) GO as required by quick-path contract
3. **[調整者]** set `ready_for_implementation: true`, `phase: tasks-approved`
4. Emit **PR Summary Output**
5. End orchestration (do **not** dispatch `/sdd-impl`). After the PR Summary fence, emit the chat-only next-step line from § [AUTO] 仕様一式.

User can still edit `tasks.md` / re-orchestrate later if needed. After PR Summary the same human review applies: same-chat correction notes stay in this phase; a new `/sdd-orchestrate <feature> 実装のみ` (or `/sdd-impl`) means they accepted the spec.

### Complexity tier (session)

| Tier | Notes |
| ---- | ----- |
| **S** | quick-path success → Terminal auto-approve (S) + PR Summary。Phase terminal なし |
| **M** | 要求 / 設計は各 Phase terminal; タスクは再開後に自動 |
| **L** | same as M |
| missing `complexity_tier` | Treat as L (backward compatible). |

`実装のみ`: end after `/sdd-verify-completion` (`FEATURE_GO`) VERIFIED.

## [AUTO] 仕様一式 (S-tier only)

Terminal auto-approve checklist covering requirements, design, and tasks.

Precondition:

- `spec.json` `approvals.*.generated === true` for all three phases
- Sanity review or unified validate reports `VERDICT: GO`

On terminal (**[調整者]**):

- `ready_for_implementation: true`
- `phase: tasks-approved`

Then: PR Summary Output → orchestration ends. Do **not** dispatch `/sdd-impl`. After the PR Summary fence, emit one chat-only next-step line (not inside the PR body): 実装は **同じ checkout** の新しいチャットで `/sdd-orchestrate <feature> 実装のみ` または `/sdd-impl <feature>`。roadmap 上の downstream spec は、この PR がマージ先に入ってからその先端の checkout で始める。

## PR Summary Output (タスク生成完了時)

After **terminal auto-approve** (`ready_for_implementation: true`), and **before** ending the orchestration, emit a Pull Request-ready summary so the user can copy & paste it directly into a PR (title + description).

**Format rules**

- Output as a single fenced ` ```markdown ` code block so it copies cleanly into a PR.
- Language follows the spec artifacts (default Japanese).
- Content is synthesized from the phase reports (`reviews/*.md` — especially `## Decisions` and 承認ゲートサマリ / `### 受容が必要な残リスク`), `brief.md` / `requirements.md`, `design.md`, and `tasks.md` — do not re-run analysis, only parse existing artifacts.
- **Two-layer layout:** タイトル / 概要 / 決定事項 / 残リスク are the default (always-visible) scan. 受け入れ確認 is the record layer — full transcription, but **collapsed** so it does not compete with the scan. Do not flatten all four into one long page.
- Keep the scan layer short. 概要 is a few sentences. 決定事項 / 残リスク cells are **one short sentence each** — do not paste full Decision paragraphs. Detail stays in the spec files.
- **Exception (record layer):** 受け入れ確認 transcribes every criterion from `requirements.md` — do not summarize or drop criteria (the spec directory may be deleted after implementation; this list is the surviving 物差し). Put it inside a closed `<details>` block. Use a **list**, never a table (EARS sentences wrap poorly in GitHub tables).

**Required content**

1. **タイトル** — one-line PR title (not a heading inside the body). Synthesize from `<feature>` + brief/requirements scope: short, imperative or noun-phrase, no trailing period. Prefer `<feature>: <要約>` (or repo PR title convention if one exists).
2. **概要** — what this spec/feature delivers (scope in a few sentences), spec path `docs/specs/<feature>/`.
3. **決定事項と理由 一覧** — a table of every key decision with its rationale, aggregated across phases:

   | 決定事項 | 理由 |
   | -------- | ---- |
   | <何を決めたか（短く）> | <なぜ（1文）> |

   Aggregate the same topics as the 決定事項サマリー (Scope / Requirements validates / Security validates / Supplements / Design / Tasks). One row per decision. Truncate; do not wrap a long EARS sentence or a full `## Decisions` bullet into a cell.
4. **残リスク** — residual risks accepted at 要求/設計 gates (and any still listed at terminal). Aggregate from unified reports' `### 受容が必要な残リスク` (and equivalent Decision bullets on deferred risks) in `reviews/requirements-review.md` / `reviews/design-review.md` (fall back to `*-final.md` / specialist reports if unified file is absent). Include risk + why accepted / deferred, **one short sentence per cell**. If none: write `なし` (no table).
5. **受け入れ確認** (collapsed) — wrap the block in `<details>` with `<summary>受け入れ確認（条件の転記。[ ] = 未確認）</summary>`. Default closed. Transcribe every numbered acceptance criterion from `requirements.md` (`#### 受け入れ条件` under each 要件). This is the planned 物差し only; implementation has not run.

   - **確認日 / 確認者 / 確認対象:** write `（実装後に記入）`. Do not invent a date, person, or PR number.
   - **List item:** `- [ ] **<要件番号>.<条件番号>** <criterion text as-is>` (e.g. `1.1`). Do not invent `REQ-*` aliases. Do not rewrite, merge, or omit. Include every criterion from every 要件.
   - Status is the checkbox only: `[ ]` = 未確認, `[x]` = 確認済み (filled at 実装後の受け入れ確認). Do **not** add nested `確認方法` / `結果` lines. Do **not** invent verification procedures. Do **not** mark `[x]` at this phase.
   - Do **not** use a markdown table for this section.

   One-line note after the list (fixed wording): `チェックを入れた項目が確認済み。確認者・確認日は実装後の受け入れ確認で更新する。この一覧は条件の転記のみ。`

**Template**

````markdown
```markdown
タイトル: <feature>: <要約>

## 概要

<feature が実現すること / スコープ>
Spec: `docs/specs/<feature>/`

## 決定事項と理由

| 決定事項 | 理由 |
| -------- | ---- |
| … | … |

## 残リスク

| リスク | 受容 / 延期の理由 |
| ------ | ---------------- |
| … | … |

<details>
<summary>受け入れ確認（条件の転記。[ ] = 未確認）</summary>

確認日: （実装後に記入）
確認者: （実装後に記入）
確認対象: （実装後に記入）

- [ ] **1.1** …

チェックを入れた項目が確認済み。確認者・確認日は実装後の受け入れ確認で更新する。この一覧は条件の転記のみ。
</details>
```
````

## Impl Phase Monitoring

Delegate to `/sdd-impl`; monitor stop conditions:

- All tasks `[x]` before `/sdd-validate-impl`
- `_Blocked:_` tasks → stop, report user
- Batch / selection loop: implement → parent mechanical → `/sdd-review` (judgment) → `/sdd-verify-completion` (`BATCH` / single-task `TASK`) before `[x]` (impl skill owns detail; execution mode `direct` / `wave` / `strict` from `complexity_tier`)

## Brownfield Option

`/sdd-spec-design` runs inline gap analysis on brownfield only (writes `research.md`). Greenfield skips gap — no separate gap dispatch.
