---
name: sdd-design-html
description: >-
  Generates a static HTML preview of a spec's design.md (shadcn-html, boundary
  triad, architecture/flows, component index, traceability) to reduce
  understanding debt. Human review focuses on basic design (external +
  internal); physical/detailed design is collapsed. Use when the user invokes
  /sdd-design-html or asks to visualize, preview, or HTML-render design.md.
disable-model-invocation: true
---

# Design HTML Preview

Generate a **derived** static HTML view of `design.md`. Do not change the markdown. Do not dispatch other SDD skills. Do not touch `spec.json`.

Page order is **reviewer-first** (external → internal → collapsed physical), not the `design.md` section order.

**Visualization rules:** [viz-rules.md](viz-rules.md) (read before assembling sections).
**Shell:** [template.html](template.html). **Tokens / chrome:** [theme.css](theme.css).

## Invocation

```text
/sdd-design-html <feature>
```

`<feature>` is the directory name under `docs/specs/`. Output:

```text
docs/specs/<feature>/design.html
```

If `<feature>` is missing, stop and ask. Do not infer from chat history.

## Pins (do not use @latest)

| Asset | Pin |
| --- | --- |
| shadcn-html | `v0.7.13-alpha` |
| Lucide | `1.8.0` |
| Mermaid | `11.6.0` |

CDN base for components:

```text
https://cdn.jsdelivr.net/gh/codylindley/shadcn-html@v0.7.13-alpha/dist/
```

Graph / chart libraries other than Mermaid are forbidden (no Chart.js, ECharts, Cytoscape, D3). Allowed Mermaid types: `flowchart` / `graph`, `sequenceDiagram`, `stateDiagram` / `stateDiagram-v2`, `erDiagram`. Copy fences from the md; do not invent diagrams.

## Execution Steps

1. **Load**
   - Read `docs/specs/<feature>/spec.json` (language). If missing, infer `ja` vs `en` from `design.md` headings (`概要` / `境界` → `ja`, `Overview` / `Boundary` → `en`). Default `ja`.
   - Read `docs/specs/<feature>/design.md`. If missing, **stop**.
   - Optionally read `docs/specs/<feature>/requirements.md` (numeric IDs only, for coverage gaps). If missing, skip the gap row; do not stop.
   - Note whether `requirements.html` exists (header link only; do not read it).
   - Read [viz-rules.md](viz-rules.md) and [template.html](template.html) and [theme.css](theme.css).
   - At generation time, skim the pinned `component-skill.md` for any component whose markup you are unsure of (`card`, `badge`, `table`, `button`, `alert`, `separator`). Do not invent class names.

2. **Do not write** `design.md`, `requirements.md`, or `spec.json`.

3. **Parse** (do not invent content) per viz-rules heading aliases:
   - Overview, Goals, Non-Goals
   - Boundary triad + Revalidation Triggers
   - Architecture (pattern text + existing mermaid + Technology Stack)
   - Persistent References (Contracts / Architecture / ADRs; Mode)
   - System Flows (existing mermaid only)
   - Component summary table; API/Event contract tables (external); TypeScript / Implementation Notes (physical)
   - Requirements Traceability; compare IDs to `requirements.md` when present
   - Domain / Logical data (internal); Physical data (appendix)
   - Error Handling categories
   - File Structure, Observability, Testing, Operational Readiness (appendix)

4. **Assemble** sections per viz-rules. Copy [template.html](template.html):
   - Paste the full contents of [theme.css](theme.css) into `<style id="des-theme">` (replace `{{THEME_CSS}}`)
   - Replace `{{HTML_LANG}}` (`ja` or `en`), `{{FEATURE_NAME}}`, `{{GENERATED_AT}}` (human), `{{GENERATED_AT_ISO}}` (ISO 8601), `{{SOURCE_PATH}}` (`docs/specs/<feature>/design.md`)
   - Replace `{{REQ_MD_HREF}}` with `requirements.md` if that file exists, else empty (and delete the requirements source button)
   - Replace `{{REQ_HTML_HREF}}` with `requirements.html` if that file exists, else empty (and delete the requirements preview button)
   - Replace every `{{LABEL_*}}` from the table below (one language only)
   - Fill `{{SLOT_*}}` from the recipe comments in the template. Omit a slot (empty string) when viz-rules say skip. For stubs, also delete reviewer sections listed in viz-rules
   - **Delete** all `recipe:*` HTML comments from the output (they are authoring aids, not page content)
   - Leave no leftover `{{…}}` placeholders

5. **Write** `docs/specs/<feature>/design.html` (single file; CSS inlined; CDNs as in the template).

6. **Report** (language = spec language): output path, and which optional viz were omitted and why.

## Source of truth

| Artifact | Role |
| --- | --- |
| `design.md` | Canonical. Never edit from this skill. |
| `design.html` | Preview only. Regenerated from md. |
| `docs/contracts/**` | Authoritative long-lived contracts. HTML lists paths only. |

Header alert must state that HTML is **not** the source of truth.

## UI labels

Use the map matching `spec.json.language`. Do not mix.

| Key | ja | en |
| --- | --- | --- |
| `{{LABEL_TITLE}}` | 設計プレビュー | Design preview |
| `{{LABEL_NOT_CANONICAL}}` | 正本ではない。正本は design.md。公開契約の正本は docs/contracts。 | Not the source of truth. Canonical file is design.md. Public contracts live in docs/contracts. |
| `{{LABEL_SOURCE}}` | 正本 | Source |
| `{{LABEL_REQ_SOURCE}}` | 要件正本 | Requirements |
| `{{LABEL_REQ_PREVIEW}}` | 要件プレビュー | Requirements preview |
| `{{LABEL_TOC}}` | 目次 | Contents |
| `{{LABEL_LAYER_EXT}}` | 外部設計 | External design |
| `{{LABEL_LAYER_INT}}` | 内部設計 | Internal design |
| `{{LABEL_LAYER_PHY}}` | 詳細設計（参考） | Detailed design (reference) |
| `{{LABEL_LAYER_HINT}}` | 人間の確認対象は外部設計と内部設計。ファイル・型・物理データは畳んであります。 | Review external and internal design. File layout, types, and physical data are collapsed. |
| `{{LABEL_OVERVIEW}}` | 概要 | Overview |
| `{{LABEL_GOALS}}` | 目標と非目標 | Goals and non-goals |
| `{{LABEL_GOAL}}` | 目標 | Goals |
| `{{LABEL_NONGOAL}}` | 非目標 | Non-goals |
| `{{LABEL_BOUNDARY}}` | 責務境界 | Responsibility boundary |
| `{{LABEL_OWNS}}` | 本仕様が持つもの | This spec owns |
| `{{LABEL_OUT}}` | 境界外 | Out of boundary |
| `{{LABEL_DEPS}}` | 依存してよいもの | Allowed dependencies |
| `{{LABEL_REVAL}}` | 再検証トリガー | Revalidation triggers |
| `{{LABEL_ARCHITECTURE}}` | アーキテクチャ | Architecture |
| `{{LABEL_PATTERN}}` | 採用パターン | Pattern |
| `{{LABEL_STACK}}` | 技術スタック | Technology stack |
| `{{LABEL_CONTRACTS}}` | 公開面 | Public surface |
| `{{LABEL_NO_CONTRACT}}` | 契約変更なし | No contract changes |
| `{{LABEL_FLOWS}}` | システムフロー | System flows |
| `{{LABEL_COMPONENTS}}` | 部品インデックス | Component index |
| `{{LABEL_PUBLIC_IF}}` | 公開インタフェース | Public interfaces |
| `{{LABEL_TRACE}}` | 要件トレーサビリティ | Requirements traceability |
| `{{LABEL_GAP}}` | 設計に無い要件 | Requirements missing from design |
| `{{LABEL_GAP_HINT}}` | 次の要件IDは requirements.md にあるが、この設計に現れない。 | These requirement IDs are in requirements.md but not in this design. |
| `{{LABEL_DATA}}` | 論理データ | Logical data |
| `{{LABEL_ERRORS}}` | エラーの約束 | Error commitments |
| `{{LABEL_APPENDIX}}` | 詳細設計（参考） | Detailed design (reference) |
| `{{LABEL_FILES}}` | ファイル構成 | File structure |
| `{{LABEL_PHYSICAL}}` | 物理データ | Physical data |
| `{{LABEL_DETAIL}}` | 部品の詳細・型 | Component detail and types |
| `{{LABEL_OBS}}` | 可観測性 | Observability |
| `{{LABEL_TEST}}` | テスト戦略 | Testing strategy |
| `{{LABEL_OPS}}` | 運用準備 | Operational readiness |
| `{{LABEL_GENERATED}}` | 生成日時 | Generated |
| `{{LABEL_COL_LAYER}}` | 層 | Layer |
| `{{LABEL_COL_CHOICE}}` | 選択 | Choice |
| `{{LABEL_COL_ROLE}}` | 役割 | Role |
| `{{LABEL_COL_PATH}}` | パス | Path |
| `{{LABEL_COL_MODE}}` | モード | Mode |
| `{{LABEL_COL_NOTES}}` | メモ | Notes |
| `{{LABEL_COL_STATUS}}` | 状態 | Status |
| `{{LABEL_COL_COMPONENT}}` | 部品 | Component |
| `{{LABEL_COL_ANCHOR}}` | アンカー | Anchor |
| `{{LABEL_COL_DOMAIN}}` | 領域 | Domain |
| `{{LABEL_COL_INTENT}}` | 意図 | Intent |
| `{{LABEL_COL_REQ}}` | 要件 | Requirements |
| `{{LABEL_COL_DEPS}}` | 依存 | Dependencies |
| `{{LABEL_COL_CONTRACTS}}` | 契約 | Contracts |
| `{{LABEL_COL_SUMMARY}}` | 要約 | Summary |
| `{{LABEL_COL_IF}}` | インタフェース | Interfaces |
| `{{LABEL_COL_FLOW}}` | フロー | Flows |
| `{{LABEL_MODE_MODIFY}}` | 変更 | Modify |
| `{{LABEL_MODE_REF}}` | 参照 | Reference |
| `{{LABEL_CRIT_P0}}` | P0 | P0 |
| `{{LABEL_CRIT_P1}}` | P1 | P1 |
| `{{LABEL_CRIT_P2}}` | P2 | P2 |
| `{{LABEL_STUB}}` | 設計はまだ薄い | Design is still a stub |
| `{{LABEL_STUB_HINT}}` | 責務境界と部品が揃うまで、索引や図は出さない。 | Index and diagrams wait until boundary and components exist. |

## Markup contract

- Collapse: native `<details>` / `<summary>` (no accordion JS)
- Reviewer sections stay open. Physical/detailed blocks: `<details class="des-appendix">` **without** `open`
- Component index links: `href="#D-ComponentName"` matching `{#D-ComponentName}` anchors in the appendix
- Mode badges: `class="badge"` plus `data-mode="modify|reference"`. Visible text is `{{LABEL_MODE_*}}`
- Criticality badges: `class="badge"` plus `data-crit="p0|p1|p2"`
- Layer badges: `class="badge"` plus `data-layer="external|internal|physical"`
- Mermaid: `<pre class="mermaid">` inside `.des-diagram`
- No Tabs JS, no extra graph CDNs, no `class="dark"` on `<html>`
- Do not render EARS matrices (that is `requirements.html`)

## Constraints

- Paraphrase for tables; **do not add** components, edges, contracts, or requirement IDs that are not in the md
- Copy Mermaid from the md. Do not synthesize a second architecture graph
- Single HTML file; no build, no npm in the spec folder
- Stub md (no populated Boundary triad and no component summary): header, overview, not-canonical alert, plus `recipe:stub`. Skip index, diagrams, contracts, flows, trace, data, errors, appendix
- Orchestrator / other `/sdd-*` skills are out of scope

## Output to the user

Short:

1. Path of `design.html`
2. What was included vs omitted (and why, one line each)
