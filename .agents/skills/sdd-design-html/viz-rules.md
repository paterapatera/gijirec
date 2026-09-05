# Visualization rules

Read this before filling [template.html](template.html). Prefer omission over a diagram or table that invents edges, components, or coverage.

Human review target: **basic design** = external + internal. Physical/detailed design is parsed and placed in closed `<details class="des-appendix">`. Do not drop it from the file; do not put it above the fold.

## Heading aliases

Match the first heading that exists. English template headings are the default (`/sdd-spec-design` output). Japanese aliases are for hand-written md.

| Slot | English | Japanese |
| --- | --- | --- |
| Overview | `## Overview` | `## 概要` |
| Goals | `### Goals` | `### 目標` |
| Non-Goals | `### Non-Goals` | `### 非目標` |
| Boundary | `## Boundary Commitments` | `## 境界の約束` / `## 責務境界` |
| Owns | `### This Spec Owns` | `### 本仕様が持つもの` / `### 本仕様の責務` |
| Out | `### Out of Boundary` | `### 境界外` / `### Out of Boundary` |
| Deps | `### Allowed Dependencies` | `### 依存してよいもの` / `### 許可する依存` |
| Reval | `### Revalidation Triggers` | `### 再検証トリガー` |
| Architecture | `## Architecture` | `## アーキテクチャ` |
| Stack | `### Technology Stack` | `### 技術スタック` |
| Persistent | `## Persistent References` | `## 永続参照` |
| Files | `## File Structure Plan` | `## ファイル構成計画` |
| Flows | `## System Flows` | `## システムフロー` |
| Trace | `## Requirements Traceability` | `## 要件トレーサビリティ` |
| Components | `## Components and Interfaces` | `## コンポーネントとインタフェース` |
| Data | `## Data Models` | `## データモデル` |
| Domain | `### Domain Model` | `### ドメインモデル` |
| Logical | `### Logical Data Model` | `### 論理データモデル` |
| Physical | `### Physical Data Model` | `### 物理データモデル` |
| Errors | `## Error Handling` | `## エラー処理` |
| Observability | `## Observability` | `## 可観測性` |
| Testing | `## Testing Strategy` | `## テスト戦略` |
| Ops | `## Operational Readiness` | `## 運用準備` |

Ignore template purpose/warning blockquotes at the top of `design.md`.

## Layer map (where each parsed section goes)

**External (open)** — Overview, Goals / Non-Goals, Boundary triad, Persistent References, System Flows, public API/Event/Batch tables from component details, Error Handling categories.

**Internal (open)** — Architecture (diagram + pattern + stack), Revalidation Triggers, component index, Requirements Traceability, Domain / Logical data.

**Physical (closed appendix)** — File Structure Plan, Physical Data Model, TypeScript / service signatures / Implementation Notes, Observability, Testing Strategy, Operational Readiness, Supporting References.

## Always emit (if the source section exists and is non-empty)

### Overview

Purpose / Users / Impact paragraphs. Drop empty template placeholders (`[specific value]`, empty `_Gap analysis_` lines may stay if present in the md).

### Goals / Non-Goals cards

Two `card`s. Drop a card whose bullet list is missing or empty. Drop the whole section if both are empty.

### Boundary triad

Three `card`s: owns / out / allowed dependencies. Drop a card whose source bullets are missing or empty. Drop the whole section if all three are empty.

Do not invent adjacent systems or owners.

### Revalidation triggers

Document-level list or table from that subsection. Omit if missing or placeholder-only.

### Architecture

- Copy every fenced `mermaid` in the Architecture section (allowed types only). Skip a fence whose type is not allowed.
- Pattern / boundary bullets: keep as a short list; do not restate the diagram.
- Technology Stack: emit rows whose Choice cell is non-empty. Drop entirely empty layers.

### Persistent References

One table per non-empty group (Contracts / Architecture / ADRs). Mode cell → badge `data-mode="modify|reference"`. If the md states **No contract changes** (any language equivalent), show that as an alert and still list reference rows if present.

Do not fetch or embed contract file bodies.

### Component index

One table from the summary table under Components (Component, Anchor, Domain/Layer, Intent, Req Coverage, Key Dependencies, Contracts). If there is no summary table, build rows from `#### Name {#D-Name}` headings using only Intent / Requirements / Dependencies fields that exist — do not invent Intent.

Anchor column links to `#D-Name` in the appendix when a detail block exists; otherwise plain text.

### Requirements traceability

Copy the traceability table if present. If the section is omitted in the md (simple feature), do **not** invent a table from the component index.

**Coverage gap (optional):** when `requirements.md` exists, collect numeric requirement IDs (heading numbers such as `1`, `1.1`, `2`). An ID is **covered** if it appears anywhere in `design.md`. List uncovered IDs in `recipe:gap`. Omit the gap alert when `requirements.md` is missing or every ID is covered. Do not flag IDs that are only in the design.

### Logical data

Domain Model + Logical Data Model prose/lists, plus any mermaid in those subsections (`erDiagram` / `flowchart` / `stateDiagram`). Omit Physical here.

### Error commitments

Error Strategy + Error Categories only. Omit per-method recovery and long process-flow mermaid unless it already exists (then copy it).

## Emit only when warranted

### System Flows

Copy mermaid fences in System Flows (`sequenceDiagram`, `flowchart`, `stateDiagram`, `erDiagram`). Omit the section if there are zero fences and no non-placeholder prose.

Do not stitch a feature-level sequence from components.

### Public interfaces (external)

From each component detail block, copy **only**:

- API Contract tables
- Event Contract / Batch Contract bullet lists
- A one-line Intent

Do **not** copy TypeScript `interface` / `type` blocks, Preconditions/Postconditions, or Implementation Notes into this section (those go to the appendix).

Omit the whole public-interface section if no API/Event/Batch content exists.

### Appendix (always closed)

Emit a closed `<details class="des-appendix">` for each of the following that exists in the md:

- File Structure Plan (directory tree + modified files)
- Physical Data Model
- Per-component remaining detail (TypeScript, Implementation Notes, State Management). Preserve `{#D-Name}` as `id="D-Name"` on the heading wrapper
- Observability
- Testing Strategy
- Operational Readiness

If none of these exist, omit the appendix shell.

## Page-level structure (order)

Follow this order in the HTML (reviewer-first, not `design.md` section order). TOC is a **linear** list of the same ids, each with a layer badge. Do not group the TOC by layer — that would jump around the page.

1. Header (title, links to `design.md` and optional requirements files, generated timestamp, not-canonical alert, layer hint)
2. TOC (same order as sections below; omit skipped ids)
3. Overview (external)
4. Goals / Non-Goals (external)
5. Boundary triad (external)
6. Revalidation triggers (internal; kept next to boundary)
7. Architecture (internal)
8. Public surface / Persistent References (external)
9. System Flows (external)
10. Public interfaces (external)
11. Component index (internal)
12. Traceability + optional gap alert (internal)
13. Logical data (internal)
14. Error commitments (external)
15. Appendix (physical, closed)
16. Footer (source path)

No Tabs JS. Layer badges are legend only, not a filter UI.

## Forbidden

- Inventing components, actors, edges, requirement IDs, or contract paths
- Re-rendering EARS / AC matrices from `requirements.md`
- Pie/bar charts, heatmaps, term clouds, network graphs, extra graph CDNs
- Mermaid types other than `flowchart` / `graph`, `sequenceDiagram`, `stateDiagram` / `stateDiagram-v2`, `erDiagram`
- Synthesizing a component graph from Dependencies (Architecture mermaid is the structure diagram)
- Empty sections with placeholder lorem or “TBD” diagrams
- Changing `design.md`
- Putting File Structure, TypeScript, or Physical Data above the fold

## Stub documents

Treat as stub when **both**:

- Boundary triad would be empty (missing section or only template placeholders)
- No component summary table and no `#### … {#D-…}` detail heading

Emit header, overview, not-canonical alert, and `recipe:stub`. Skip TOC groups for index/diagrams/appendix.
