# ADR-0015: キャプチャセッションを開始専用 IPC に縮小する

- **Status**: Accepted
- **Date**: 2026-09-19
- **Feature**: capture-session-toggle
- **Owners / Domains**: capture-session-toggle

## Context

Path A 要求更新により、停止ボタン・セッション途中停止・停止時即時転写フラッシュがスコープ外となった。進行中の実装と契約 `capture-session-toggle.md` は `set_capture_session_active` トグルと `stop_flush_in_progress` を前提としており、更新後 `requirements.md` と矛盾する。

## Decision

- 公開 IPC を **`start_capture_session`**（引数なし）に一本化し、停止系コマンドを提供しない。
- `CaptureSessionPhase` を **`idle` | `starting` | `active`** に縮小する（`stopped`/`stopping` および flush 進行フラグを削除）。
- セッション終了は利用者操作では行わず、**メインウィンドウ閉鎖**時の既存 `on_app_shutdown` lifecycle に委譲する。
- 30 秒バッチ転写は開始後アプリ終了まで継続；ユーザー向け停止 flush は行わない。

## Consequences

- Positive: 要件 2–3（停止非提供・継続）と契約が一致；UI を開始ボタン単体に簡素化できる。
- Negative / trade-offs: 進行中コード（`StopFlushCoordinator`、`set_capture_session_active`）は設計再生成に伴い削除・置換が必要。契約 Changelog で破壊的変更を明示。

## Alternatives considered

1. **`set_capture_session_active` を残し `active: false` を無視** — API が停止を示唆し要件と乖離するため不採用。
2. **停止 flush を shutdown のみに限定して UI フラグを維持** — ユーザー向け flush 可視化は要求外のため不採用。

## Notes

- `audio-capture-status.md` の起動時 non-auto-capture Changelog は維持（reference）。
