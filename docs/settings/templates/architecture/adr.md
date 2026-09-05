# ADR-{{NNNN}}: {{TITLE}}

<!-- Output: docs/architecture/adr/ADR-{{NNNN}}-{{slug}}.md -->
<!-- Register the path in docs/architecture/adr/README.md Entries -->
<!-- Numbering: zero-padded 4 digits (ADR-0001, ADR-0002, ...) -->

- **Status**: Proposed | Accepted | Superseded by ADR-XXXX
- **Date**: {{YYYY-MM-DD}}
- **Feature**: {{feature-id}}
  <!-- Keep this line after the feature directory is deleted -->
- **Owners / Domains**: {{OWNERS}}

## Context

{{WHY_THIS_DECISION_IS_NEEDED}}

## Decision

{{WHAT_WE_DECIDE}}

## Consequences

- Positive: {{POSITIVE}}
- Negative / trade-offs: {{TRADEOFFS}}

## Alternatives considered

{{SHORT_ALTERNATIVES}}

## Notes

- 1 判断 = 1 ファイル。既存本文をマージ編集して履歴を消さない
- 判断を覆すときは新 ADR を作り、旧 Status のみ `Superseded by ADR-XXXX` に変更する
- feature 配下を永続 ADR の正本にしない
