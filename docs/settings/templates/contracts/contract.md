# {{DOMAIN}}-{{SURFACE}}

<!-- Output: docs/contracts/{{domain}}-{{surface}}.md -->
<!-- Register the path in docs/contracts/README.md index -->
<!-- Naming: <domain>-<surface>.md — one file = one contract surface -->

- **Surface type**: API | Event | Data ownership
- **Owners / Domains**: {{OWNERS}}
- **Related ADR**: {{ADR_PATH_OR_NONE}}

## Purpose

{{ONE_LINE_PURPOSE}}

## Contract

{{SHAPE_ENDPOINTS_EVENTS_SCHEMA_OR_OWNERSHIP}}

## Non-goals

- {{OUT_OF_SCOPE}}

## Changelog

<!-- Required for breaking or ownership-changing updates. Keep newest first. -->

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| {{YYYY-MM-DD}} | {{INITIAL_OR_DELTA}} | {{ADR_PATH_OR_NOTE}} |

## Notes

- feature 配下 (`docs/specs/{feature}/contracts/`) を永続契約の正本にしない
- AI は contracts index → 本ファイルのみ Read（全量禁止）
- 新規追加時は `docs/contracts/README.md` の Entries 行を必ず更新
- Command 面は Tauri invoke エラー payload を Contract 内に表で定義（例: `audio-device-selection.md`）
- TypeScript ミラー場所は Notes に 1 行で記載（例: `src/presentation/hooks/` または `src/infrastructure/tauri/`）
