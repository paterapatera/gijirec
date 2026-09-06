# transcript-editor-settings

- **Surface type**: API / Data ownership
- **Owners / Domains**: transcript-editor
- **Related ADR**: docs/architecture/adr/ADR-0005-slate-js-transcript-editor.md

## Purpose

transcript-editor の保存先ディレクトリおよび JSONL 出力設定の永続化形状と Tauri コマンドを定義する。

## Contract

### EditorSettings（永続化形状）

```typescript
interface EditorSettings {
  /** 保存基点ディレクトリの絶対パス。未設定は null */
  save_directory: string | null;
  /** タイムスタンプ付き JSONL 出力の有効/無効 */
  export_jsonl_enabled: boolean;
}
```

| フィールド | 制約 |
|-----------|------|
| `save_directory` | 絶対パス。存在しないパスも設定可（保存時に検証） |
| `export_jsonl_enabled` | デフォルト `false` |

### 永続化

| 項目 | 値 |
|------|-----|
| 保存先 | Tauri `app_data_dir` 配下 `{app_identifier}/editor-settings.json` |
| 形式 | JSON（UTF-8） |
| 起動時 | `get_editor_settings` で復元 |
| マイグレーション | v1 は単一バージョン。将来フィールド追加時は後方互換デフォルト |

### Tauri コマンド

#### `get_editor_settings`

```typescript
// Request: なし
// Response:
type GetEditorSettingsResponse = EditorSettings;
```

#### `set_editor_settings`

```typescript
interface SetEditorSettingsRequest {
  save_directory?: string | null;
  export_jsonl_enabled?: boolean;
}

type SetEditorSettingsResponse = EditorSettings;
```

- 部分更新可。未指定フィールドは既存値を維持
- 永続化失敗 → `SETTINGS_PERSIST_FAILED`

#### `pick_save_directory`

```typescript
// Request: なし
// Response: 利用者がダイアログで選択した絶対パス、キャンセル時 null
type PickSaveDirectoryResponse = string | null;
```

- OS ネイティブディレクトリ選択ダイアログ（Tauri dialog plugin）
- 選択結果は自動永続化しない — 利用者が `set_editor_settings` で確定

### 禁止事項

- 設定ファイルへの転写テキスト・手動議事録内容の混入
- 設定の外部ネットワーク同期

## Non-goals

- 保存ファイル I/O 本体（`transcript-editor-save.md`）
- エディタロック状態の永続化（セッション内メモリのみ）

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-06 | 初版 — EditorSettings 形状・永続化・設定コマンド | ADR-0005 |

## Notes

- 利用者向けエラーコード: `transcript-editor-status.md`
