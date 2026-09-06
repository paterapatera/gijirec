# transcript-editor-status

- **Surface type**: API
- **Owners / Domains**: transcript-editor
- **Related ADR**: docs/architecture/adr/ADR-0005-slate-js-transcript-editor.md

## Purpose

transcript-editor の保存・設定操作で利用者に返すエラー形状を定義する。Tauri invoke 応答および UI 表示の正本。

## Contract

### EditorUserError

```typescript
type EditorUserErrorCode =
  | "SAVE_DIRECTORY_NOT_SET"
  | "SAVE_DIRECTORY_UNAVAILABLE"
  | "SAVE_DIRECTORY_CREATE_FAILED"
  | "SAVE_FILE_WRITE_FAILED"
  | "SAVE_PARTIAL_FAILURE"
  | "SETTINGS_PERSIST_FAILED"
  | "INTERNAL";

interface EditorUserError {
  code: EditorUserErrorCode;
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}
```

### コード発火条件

| code | 発火条件 | recoverable |
|------|----------|-------------|
| `SAVE_DIRECTORY_NOT_SET` | 保存時 `save_directory === null` | true |
| `SAVE_DIRECTORY_UNAVAILABLE` | 基点ディレクトリ不存在または書込権限なし | true |
| `SAVE_DIRECTORY_CREATE_FAILED` | JST サブディレクトリ作成失敗 | true |
| `SAVE_FILE_WRITE_FAILED` | 全ファイル書込失敗 | true |
| `SAVE_PARTIAL_FAILURE` | 一部ファイルのみ書込成功 | true |
| `SETTINGS_PERSIST_FAILED` | 設定 JSON 書込失敗 | true |
| `INTERNAL` | 想定外エラー | false |

### 表示規約

- `message_ja`: 何が起きたか（短文）
- `action_ja`: 次に取れる行動（非空・必須）。例: 「別の保存先フォルダを選択してください」
- `code` は DOM / 利用者向けラベルに表示しない
- 転写テキスト全文・手動議事録全文を payload / ログに含めない

### Tauri イベント

v1 では保存・設定エラーは **invoke 応答内の `EditorUserError`** のみ。専用 error イベントは定義しない（whisper-transcribe 上流エラーは既存 `whisper-transcribe://error` を購読）。

## Non-goals

- whisper-transcribe / audio-capture のエラーコード（各 status 契約が所有）
- 自動リトライ

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-06 | 初版 — EditorUserError 列挙・表示規約 | ADR-0005 |

## Notes

- エラー変換は Rust domain `EditorError::to_user_facing()` に集約（steering error-handling 準拠）
