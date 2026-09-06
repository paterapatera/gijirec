# transcript-editor-save

- **Surface type**: API / Data ownership
- **Owners / Domains**: transcript-editor
- **Related ADR**: docs/architecture/adr/ADR-0005-slate-js-transcript-editor.md

## Purpose

transcript-editor が利用者の明示的保存操作で出力する Markdown / JSONL ファイルの形状、保存先ディレクトリ規約、および Tauri 保存コマンドを定義する。

## Contract

### 保存先ディレクトリ規約

| 項目 | 値 |
|------|-----|
| 基点 | 利用者設定の `save_directory`（`transcript-editor-settings.md`） |
| サブディレクトリ | `{save_directory}/{YYYY}/{MM}/{DD}/{hh}_{mm}_{ss}/` |
| 時刻基準 | 保存操作開始時刻の **日本標準時（JST, UTC+9）** |
| 同一秒衝突 | サフィックス `_001`, `_002`, … を `{hh}_{mm}_{ss}` 直後に付与して一意化 |
| 作成失敗 | ファイル出力を行わず、`SAVE_DIRECTORY_CREATE_FAILED` を返す |

### 出力ファイル

| ファイル | 内容 | 必須 |
|----------|------|------|
| `handwriting.md` | 保存開始時点の手動議事録（UTF-8 Markdown プレーンテキスト） | 常に |
| `ai-transcription.md` | 保存開始時点の AI 転写表示内容（ロック済み手動修正を含む、**タイムスタンプなし**プレーンテキスト） | 常に |
| `ai-transcription.jsonl` | 保存開始時点のブロック構造（1 行 1 JSON レコード、タイムスタンプ付き） | `export_jsonl_enabled === true` のみ |

### JSONL レコード形状

```typescript
interface AiTranscriptionJsonlRecord {
  block_id: string;
  sequence: number;
  text: string;
  start_timestamp_ms: number;
  language: string;
}
```

- `text` は当該ブロック表示領域の最終テキスト（利用者の手動修正・ロック後内容）
- 上流 `whisper-transcribe-blocks.md` の `TranscriptBlock` フィールドと整合

### 保存スナップショット規約

| 項目 | 値 |
|------|-----|
| スナップショット時点 | 保存コマンド受信時（Rust 側で JST サブディレクトリ名を確定した直後） |
| 保存中の上流ブロック | **含めない** — スナップショット以降に到着したブロックは当該保存成果物から除外 |
| 上流停止 | 保存処理中も whisper-transcribe の文字起こしを停止しない |
| 部分失敗 | 成功ファイルと失敗ファイルを区別して返却 |

### Tauri コマンド

#### `save_transcript_session`

```typescript
interface SaveTranscriptSessionRequest {
  /** 相関 ID（ログ用。転写全文を含めない） */
  session_id: string;
  /** 保存開始時点の手動議事録 Markdown */
  handwriting_markdown: string;
  /** 保存開始時点の AI 転写プレーンテキスト（タイムスタンプなし） */
  ai_transcription_markdown: string;
  /** export_jsonl_enabled 時のみ必須 */
  ai_transcription_jsonl?: AiTranscriptionJsonlRecord[];
}

interface SaveTranscriptSessionResult {
  success: boolean;
  /** 作成したサブディレクトリの絶対パス（成功時） */
  output_directory?: string;
  /** 書き込み成功ファイルの絶対パス一覧 */
  files_written?: string[];
  /** 部分失敗時の失敗ファイル */
  files_failed?: Array<{ path: string; reason_ja: string }>;
  /** 全体失敗時 */
  error?: EditorUserError;
}
```

- `save_directory` 未設定 → `SAVE_DIRECTORY_NOT_SET`（保存しない）
- 基点ディレクトリ不存在・書込不可 → `SAVE_DIRECTORY_UNAVAILABLE`
- 個別ファイル書込失敗 → `success: false` または部分成功（`files_written` + `files_failed`）

### 禁止事項

- 手動議事録と AI 転写の自動マージ単一ファイル生成
- 利用者明示操作なしのディスク書込
- 転写テキストの外部ネットワーク送信
- 保存処理による whisper-transcribe 停止

## Non-goals

- 手動編集 UI・部分ロックロジック（フロントエンド責務）
- 音声キャプチャ・Whisper 推論
- クラウド同期

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-06 | 初版 — 保存コマンド・JST サブディレクトリ・出力ファイル形状 | ADR-0005 |

## Notes

- 利用者向けエラーコード形状は `transcript-editor-status.md` を参照
- 上流ブロック契約: `docs/contracts/whisper-transcribe-blocks.md`
