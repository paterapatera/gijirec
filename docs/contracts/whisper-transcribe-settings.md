# whisper-transcribe-settings

- **Surface type**: API / Data ownership
- **Owners / Domains**: whisper-model-selection
- **Related ADR**: docs/architecture/adr/ADR-0013-whisper-model-variant-selection.md

## Purpose

kotoba-whisper-v2.2 の量子化バリアント（Q5_0 / Q8_0 / FP16）選択の永続化形状と Tauri コマンド。モデル取得・フェーズイベントは `whisper-transcribe-status.md` を変更せず再利用する。

## Contract

### WhisperModelVariant

```typescript
/** kotoba-whisper-v2.2 の 3 バリアントのみ（要件 1.3–1.4） */
type WhisperModelVariant = "q5_0" | "q8_0" | "fp16";
```

| 値 | 表示ラベル（UI） | ローカルファイル名（`{app_data_dir}/models/`） |
|----|------------------|-----------------------------------------------|
| `q5_0` | Q5_0 | `kotoba-whisper-v2.2-ggml-q5_0.bin` |
| `q8_0` | Q8_0 | `kotoba-whisper-v2.2-ggml-q8_0.bin` |
| `fp16` | FP16 | `kotoba-whisper-v2.2-ggml.bin` |

配布元 URL（resolve/main）:

| 値 | URL |
|----|-----|
| `q5_0` | `https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q5_0.bin` |
| `q8_0` | `https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q8_0.bin` |
| `fp16` | `https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml.bin` |

SHA-256（検証用、実装定数の正本は Rust `ModelVariantCatalog`）:

| 値 | SHA-256 |
|----|---------|
| `q5_0` | `4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658` |
| `q8_0` | `c4071b2f8f0129d463c6c7fd2e72c82f7276f9882a8f9cd9474e0c2b699100c4` |
| `fp16` | `eff70a8a236e731abba774ba71e1f6d0fce53302137208c32207e694e0bf4546` |

### TranscribeSettings（永続化形状）

```typescript
interface TranscribeSettings {
  /** 選択中バリアント。未設定ファイルの論理既定は fp16（要件 4.3） */
  model_variant: WhisperModelVariant;
}
```

| フィールド | 制約 |
|-----------|------|
| `model_variant` | `q5_0` \| `q8_0` \| `fp16` のみ |

### 永続化

| 項目 | 値 |
|------|-----|
| 保存先 | Tauri `app_data_dir` 配下 `{app_identifier}/transcribe-settings.json` |
| 形式 | JSON（UTF-8） |
| 起動時 | `get_transcribe_settings` で復元し、選択バリアントのモデル取得・ロードを開始 |
| 既定 | ファイル不存在・`model_variant` 欠落時は **`fp16`**（既存利用者後方互換、ADR-0011） |
| マイグレーション | v1 は単一バージョン。将来フィールド追加時は後方互換デフォルト |

### Tauri コマンド

#### `get_transcribe_settings`

```typescript
// Request: なし
// Response:
interface GetTranscribeSettingsResponse {
  settings: TranscribeSettings;
  /** 各バリアントのローカルファイル存在（検証済みは問わず、ファイル存在のみ） */
  local_availability: Record<WhisperModelVariant, boolean>;
}
```

#### `set_transcribe_model_variant`

```typescript
interface SetTranscribeModelVariantRequest {
  model_variant: WhisperModelVariant;
}

interface SetTranscribeModelVariantResponse {
  settings: TranscribeSettings;
}
```

**`set_transcribe_model_variant` 挙動**:
1. 永続化（`transcribe-settings.json`）を更新
2. 選択バリアントがローカル未存在なら `ModelOrchestrator` が取得を開始し `loading_model` + `whisper-transcribe://model-progress` を発行（要件 2.1, 3.1）
3. ローカル存在なら追加 DL なしで読み込み（要件 2.2）
4. 転写実行中の場合、実行中サイクルは継続し **次バッチサイクル** から新バリアントを適用（要件 2.4）
5. 成功時 `whisper-transcribe://phase-changed` / `model-progress` は既存契約に従う

### Command エラー（invoke エラー payload）

| code | 条件 | message_ja 例 | action_ja 例 |
|------|------|---------------|--------------|
| `SETTINGS_PERSIST_FAILED` | JSON 書き込み失敗 | 設定の保存に失敗しました | アプリを再起動して再度お試しください |
| `INVALID_MODEL_VARIANT` | 列挙外の値 | 選択したモデルは利用できません | 一覧からモデルを選び直してください |
| `MODEL_DOWNLOAD_FAILED` | 取得失敗 | （`whisper-transcribe-status` の `TranscribeUserError` に委譲） | ネットワーク接続を確認し、アプリを再起動して再試行してください |

永続化**読み込み**失敗時は invoke エラーにせず、起動処理側で日本語通知 + **fp16 既定**で継続（要件 4.4）。通知は `whisper-transcribe://error` またはアプリ toast（実装は design に委譲）。

### 禁止事項

- 設定ファイルへの転写テキスト・PCM・認証情報の混入（要件 4.5）
- kotoba-whisper 以外のファミリ・量子化の受け付け
- 設定の外部ネットワーク同期

## Non-goals

- フェーズ列挙・`model-progress` イベント形状の変更（`whisper-transcribe-status.md` が所有）
- ハードウェア自動推奨
- 転写ブロック形状の変更

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-09 | 初版 — バリアント選択永続化・set/get command | ADR-0013 |

## Notes

- TypeScript ミラー: `src/infrastructure/tauri/transcribeSettingsCommands.ts`、`src/presentation/hooks/useTranscribeSettings.ts`
- バリアント表示ラベルは UI 層（Q5_0 / Q8_0 / FP16）。永続化値は snake_case 列挙
- 既存 `compose.rs` の `DEFAULT_WHISPER_MODEL_*` は `ModelVariantCatalog::fp16()` へ移行し、3 バリアント定義に統合
