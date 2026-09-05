---
name: sdd-req-html
description: >-
  Generates a static HTML preview of a spec's requirements.md (shadcn-html,
  EARS matrices, decision tables, Mermaid) to reduce understanding debt.
  Use when the user invokes /sdd-req-html or asks to visualize, preview, or
  HTML-render requirements.md.
disable-model-invocation: true
---

# Requirements HTML Preview

Generate a **derived** static HTML view of `requirements.md`. Do not change the markdown. Do not dispatch other SDD skills. Do not touch `spec.json`.

**Visualization rules:** [viz-rules.md](viz-rules.md) (read before assembling sections).
**Shell:** [template.html](template.html). **Tokens / chrome:** [theme.css](theme.css).

## Invocation

```text
/sdd-req-html <feature>
```

`<feature>` is the directory name under `docs/specs/`. Output:

```text
docs/specs/<feature>/requirements.html
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

Graph / chart libraries other than Mermaid are forbidden (no Chart.js, ECharts, Cytoscape, D3). Mermaid diagrams: `flowchart` and `stateDiagram` only.

## Execution Steps

1. **Load**
   - Read `docs/specs/<feature>/spec.json` (language). If missing, infer `ja` vs `en` from `requirements.md` headings (`要件` → `ja`, `Requirement` → `en`). Default `ja`.
   - Read `docs/specs/<feature>/requirements.md`. If missing, **stop**.
   - Read [viz-rules.md](viz-rules.md) and [template.html](template.html) and [theme.css](theme.css).
   - At generation time, skim the pinned `component-skill.md` for any component whose markup you are unsure of (`card`, `badge`, `table`, `button`, `alert`, `separator`). Do not invent class names.

2. **Do not write** `requirements.md` or `spec.json`.

3. **Parse** (do not invent content)
   - Introduction (`## はじめに` / `## Introduction`)
   - Optional scope (`対象範囲` / `対象外` / `隣接…` or In / Out / Adjacent)
   - Each requirement: numeric heading → `id="req-N"`; purpose/user-story; numbered EARS ACs
   - Classify each AC: `When` | `If` | `While` | `Where` | `shall` (see viz-rules)

4. **Assemble** sections per viz-rules. Copy [template.html](template.html):
   - Paste the full contents of [theme.css](theme.css) into `<style id="req-theme">` (replace `{{THEME_CSS}}`)
   - Replace `{{HTML_LANG}}` (`ja` or `en`), `{{FEATURE_NAME}}`, `{{GENERATED_AT}}` (human), `{{GENERATED_AT_ISO}}` (ISO 8601), `{{SOURCE_PATH}}` (`docs/specs/<feature>/requirements.md`)
   - Replace every `{{LABEL_*}}` from the table below (one language only)
   - Fill `{{SLOT_*}}` from the recipe comments in the template. Omit a slot (empty string) when viz-rules say skip. For stubs, also delete the `#index` section from the shell
   - **Delete** all `recipe:*` HTML comments from the output (they are authoring aids, not page content)
   - Leave no leftover `{{…}}` placeholders

5. **Write** `docs/specs/<feature>/requirements.html` (single file; CSS inlined; CDNs as in the template).

6. **Report** (language = spec language): output path, and which optional viz were omitted and why.

## Source of truth

| Artifact | Role |
| --- | --- |
| `requirements.md` | Canonical. Never edit from this skill. |
| `requirements.html` | Preview only. Regenerated from md. |

Header alert must state that HTML is **not** the source of truth.

## UI labels

Use the map matching `spec.json.language`. Do not mix.

| Key | ja | en |
| --- | --- | --- |
| `{{LABEL_TITLE}}` | 要件プレビュー | Requirements preview |
| `{{LABEL_NOT_CANONICAL}}` | 正本ではない。正本は requirements.md。 | Not the source of truth. Canonical file is requirements.md. |
| `{{LABEL_SOURCE}}` | 正本 | Source |
| `{{LABEL_TOC}}` | 目次 | Contents |
| `{{LABEL_INTRO}}` | はじめに | Introduction |
| `{{LABEL_SCOPE}}` | スコープ境界 | Scope boundary |
| `{{LABEL_IN}}` | 対象範囲 | In scope |
| `{{LABEL_OUT}}` | 対象外 | Out of scope |
| `{{LABEL_ADJ}}` | 隣接システム・仕様への期待 | Adjacent expectations |
| `{{LABEL_INDEX}}` | 要件インデックス | Requirement index |
| `{{LABEL_EXCEPTIONS}}` | 例外カタログ | Exception catalog |
| `{{LABEL_STORY}}` | 目的 | Purpose |
| `{{LABEL_ROLE}}` | 役割 | Role |
| `{{LABEL_CAPABILITY}}` | やりたいこと | Capability |
| `{{LABEL_BENEFIT}}` | 得られる結果 | Benefit |
| `{{LABEL_AC}}` | 受け入れ条件 | Acceptance criteria |
| `{{LABEL_DECISION}}` | 決定表 | Decision table |
| `{{LABEL_FLOW}}` | 流れ | Flow |
| `{{LABEL_STATE}}` | 状態 | States |
| `{{LABEL_GENERATED}}` | 生成日時 | Generated |
| `{{LABEL_COL_ID}}` | ID | ID |
| `{{LABEL_COL_PURPOSE}}` | 目的 | Purpose |
| `{{LABEL_COL_AC_COUNT}}` | AC数 | ACs |
| `{{LABEL_COL_TYPES}}` | 種別 | Types |
| `{{LABEL_COL_NUM}}` | # | # |
| `{{LABEL_COL_SUBJECT}}` | 主体 | Subject |
| `{{LABEL_COL_COND}}` | 条件 | Condition |
| `{{LABEL_COL_TYPE}}` | 種別 | Type |
| `{{LABEL_COL_RESPONSE}}` | 応答 | Response |
| `{{LABEL_COL_REQ}}` | 要件 | Requirement |
| `{{LABEL_PATTERN}}` | パターン | Pattern |
| `{{LABEL_Y}}` | Y | Y |
| `{{LABEL_N}}` | N | N |
| `{{LABEL_NA}}` | — | — |
| `{{LABEL_MARK}}` | ○ | ○ |
| `{{LABEL_LEGEND}}` | 動きの種類 | When this applies |
| `{{LABEL_EARS_WHEN}}` | 発生時 | When it happens |
| `{{LABEL_EARS_IF}}` | 異常時 | If it goes wrong |
| `{{LABEL_EARS_WHILE}}` | 継続中 | While this continues |
| `{{LABEL_EARS_WHERE}}` | 任意 | If this feature exists |
| `{{LABEL_EARS_SHALL}}` | 常時 | Always |
| `{{LABEL_EARS_WHEN_HINT}}` | 送信する・承認するなど、何かが起こったときの動き | What the system does after an event |
| `{{LABEL_EARS_IF_HINT}}` | エラーや、入力が正しくないとき | Errors or invalid input |
| `{{LABEL_EARS_WHILE_HINT}}` | 承認待ちなど、その状態が続く間の動き | Behavior while a situation lasts |
| `{{LABEL_EARS_WHERE_HINT}}` | オプションの機能を使う場合だけ | Only when an optional feature is on |
| `{{LABEL_EARS_SHALL_HINT}}` | 条件に関係なく、いつも守ること | Always true, no extra condition |

## Markup contract

- Decision cells: `Y` → `<span class="req-dt-y">Y</span>`, `N` → `<span class="req-dt-n">N</span>`, `—` → `<span class="req-dt-na">—</span>`, action fires → `<span class="req-dt-mark">○</span>` (never `X`)
- Collapse: native `<details>` / `<summary>` (no accordion JS)
- Requirement blocks: `<details class="req-block" id="req-N" open>`
- Badges: `class="badge"` plus `data-ears="when|if|while|where|shall"` (styles in theme.css). Visible text is `{{LABEL_EARS_*}}` — never show raw `When` / `If` / `While` / `Where` / `shall` on the page. Classification still uses those English keywords in the markdown.
- Mermaid: `<pre class="mermaid">` inside `.req-diagram`
- No Tabs JS, no extra graph CDNs, no `class="dark"` on `<html>`

## Constraints

- Paraphrase AC text for tables/diagrams; **do not add behavior** that is not in the md
- Numeric requirement IDs only → `id="req-N"`
- Single HTML file; no build, no npm in the spec folder
- Stub md (no numbered EARS ACs): header, intro, not-canonical alert, plus `recipe:stub`. Skip index, matrix, diagrams, catalog, and the `#index` shell section. Localize the stub description (ja: 受け入れ条件はまだない。 / en: ACs are not in this document yet.)
- Orchestrator / other `/sdd-*` skills are out of scope

## Output to the user

Short:

1. Path of `requirements.html`
2. What was included vs omitted (and why, one line each)
