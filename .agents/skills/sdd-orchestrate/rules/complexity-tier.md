# Complexity Tier (S / M / L)

Compute the complexity tier **at orchestration start, immediately after routing** resolves the active flow and **before the first skill dispatch**. Write the result to `spec.json`, then load the matching flow variant in `flows.md`.

**Do not** change the `実装のみ` flow by tier. **Do not** read `requirements.md` / `design.md` for scoring — inputs are brief + metadata only.

## When to run

| Condition | Action |
| --------- | ------ |
| Active flow is `要求新規作成` (or Path D/E per-spec 要求新規作成) | Compute tier; select S / M / L variant |
| Active flow is `要求更新` / `設計更新` | Optional recompute on re-orchestrate; default to existing `complexity_tier` if present, else **L** |
| Active flow is `実装のみ` | **Skip** — do not alter the flow by tier |
| `spec.json` exists without `complexity_tier` | Treat as **L** (backward compatible) until recomputed |

Re-orchestration may recompute and overwrite `complexity_tier` / `complexity_score` / `complexity_rationale`.

## Inputs (brief + metadata only)

| Source | Required | Use |
| ------ | -------- | --- |
| `docs/specs/<feature>/brief.md` | **Yes** | Scope, greenfield/brownfield, tools/APIs, completeness, reference patterns |
| `docs/steering/roadmap.md` | If present | Upstream deps for the feature; Path D/E signals |
| `docs/specs/*/spec.json` | Count only | Metadata (spec count); do not read artifact bodies |
| User utterance | If present | Override keywords (`quick` / `lite` / `フル` / `full`) |

**Forbidden for scoring:** full reads of `requirements.md`, `design.md`, `tasks.md`, or review reports.

## Scoring

Start at **0**. Add/subtract per row. Sum determines the tier unless an override or force-L rule applies.

| Condition | Points |
| --------- | ------ |
| greenfield（brief Current State に実装なし / 緑地） | +0 |
| brownfield（既存コード拡張） | +2 |
| brief Scope In の箇条書き ≥ 8 | +2 |
| brief Scope In の箇条書き 4–7 | +1 |
| 推定 MCP ツール数 / 主要 API エンドポイント ≥ 5 | +2 |
| 推定ツール数 / 主要 API エンドポイント 3–4 | +1 |
| roadmap 上の upstream 依存 ≥ 1 | +2 |
| Path D/E（multi-spec） | +5（**強制 L**） |
| 参照実装・パターンが brief に明示 | −1 |
| brief が Problem / Approach / Scope In/Out / Constraints をすべて埋めている | −1 |

### Force L (before threshold)

Apply **L** immediately when any of:

- Path D/E (multi-spec / mixed roadmap) — score still recorded; tier is **L**
- User override:「フル」「full」
- Existing `spec.json` has no `complexity_tier` and this is a resume of an in-flight non-S/M flow — treat as **L** until recomputed for a fresh 要求新規作成

### User override

| User says | Effect |
| --------- | ------ |
| 「quick」「lite」 | Force **S** |
| 「フル」「full」 | Force **L** |

Override wins over the numeric score (except: never select S for Path D/E — Path D/E stays **L**).

## Tier thresholds

| Total score | Tier | Flow section |
| ----------- | ---- | ------------ |
| ≤ 1 | **S** | `flows.md` § 要求新規作成 (S) |
| 2–4 | **M** | `flows.md` § 要求新規作成 (M) |
| ≥ 5 or Path D/E | **L** | `flows.md` § 要求新規作成 (L) |

## Output (`spec.json`)

Orchestrator (`[調整者]`) merges these fields without removing existing keys:

```json
{
  "complexity_tier": "S",
  "complexity_score": 0,
  "complexity_rationale": "greenfield, 2 tools, no deps, complete brief"
}
```

| Field | Type | Meaning |
| ----- | ---- | ------- |
| `complexity_tier` | `"S"` \| `"M"` \| `"L"` | Selected flow variant |
| `complexity_score` | number | Numeric sum (record even when override/force applies) |
| `complexity_rationale` | string | Short human-readable reasons (factors + override if any) |

## Orchestrator procedure

After resolving the active flow (`routing.md` § Entry Contract), before the first skill dispatch:

1. Read this file (`rules/complexity-tier.md`)
2. If flow is `実装のみ` → skip; keep the 実装のみ section
3. Else score from `brief.md` (+ roadmap if present); apply force-L / user override
4. Write `complexity_tier` / `complexity_score` / `complexity_rationale` to `docs/specs/<feature>/spec.json` (create `spec.json` only if init already ran or will run via `/sdd-spec-requirements` Step 0; if pre-init, write immediately after Step 0 creates it, or stash the computed values and write on first `[調整者]` touch of `spec.json`)
5. Load the matching `flows.md` variant (`要求新規作成 (S|M|L)`). Missing `complexity_tier` on an existing spec → **L**

## Hard rules

- Do **not** pick tier **S** for Path D/E multi-spec flows
- Do **not** vary `実装のみ` by tier
- Do **not** score from requirements/design bodies
- Default without `complexity_tier` → **L** for **orchestration flow path** only (backward compatible)

## Scope note: missing tier vs `/sdd-impl` execution mode

| Layer | Missing `complexity_tier` means | Why |
| ----- | -------------------------------- | --- |
| **Orchestration** (this file, `routing.md`, `contract.md`) | Treat as **L** full-path for 要求/設計/タスク flows | Conservative: do not silently take quick-path on legacy specs |
| **`/sdd-impl` execution mode** | Task-count fallback: ≤3 → `direct`, ≤12 → `wave`, >12 → `strict` | Cost control at implement time; see `sdd-impl` Step 2 |

These are different decisions. Writing `complexity_tier` at orchestration entry (or before `実装のみ`) keeps them aligned; do not assume “missing → L” forces impl `strict`.

## Link to `/sdd-spec-design` discovery (improvement 08)

When `/sdd-spec-design` classifies scope scale, prefer `complexity_tier` over brief heuristics:

| `complexity_tier` | Scope scale (Axis B) | Typical discovery (greenfield) |
| ----------------- | -------------------- | ------------------------------ |
| **S** | **simple** | Minimal (`design-discovery-minimal.md`) |
| **M** | **standard** | Light |
| **L** | **complex** | Full |

Never treat tier **L** as simple. Brownfield / extension still run Gap (2.0) first; then Light (S/M) or Full (L) per the design skill mapping table.
