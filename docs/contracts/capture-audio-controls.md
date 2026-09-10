# capture-audio-controls

- **Surface type**: API / Event
- **Owners / Domains**: capture-audio-controls
- **Related ADR**: docs/architecture/adr/ADR-0014-capture-audio-controls-ingest-boundary.md

## Purpose

転写 ingest 直前のマイク ON/OFF（ingest ミックス除外のみ）、手動 ingest ゲイン、ingest 直前 dBFS レベル表示の Tauri IPC 契約。生 PCM はフロントへ送らない（メタデータのみ）。

## Contract

### 型定義

```typescript
/** 転写 ingest ミックスへのマイク供給（OS ミュートではない） */
interface CaptureAudioControls {
  /** true = マイクを ingest ミックスに含める。既定 true */
  mic_ingest_enabled: boolean;
  /**
   * ingest 直前の線形ゲイン乗数。既定 1.25（`transcribe-volume-normalize` 等価）。
   * ソフトリミット 0.95 は Rust 側で常時適用。
   */
  manual_ingest_gain: number;
  /** セッション中にユーザーがゲインを操作したら true（要件 3.6 の判定用） */
  gain_user_adjusted: boolean;
}

/** メーター非活性時は level_dbfs を省略 */
interface IngestLevelSnapshot {
  level_dbfs: number;
  timestamp_ms: number;
}

interface CaptureAudioControlsState {
  controls: CaptureAudioControls;
  /** capturing かつ ingest へ供給可能なときのみ。それ以外は null */
  ingest_level: IngestLevelSnapshot | null;
}
```

### ゲイン制約

| 項目 | 値 |
|------|-----|
| `manual_ingest_gain` 最小 | `0.25` |
| `manual_ingest_gain` 最大 | `4.0` |
| 既定（未調整） | `1.25` |
| ソフトリミット | `0.95`（実装固定、IPC から変更不可） |
| dBFS 算出 | ingest 後 RMS → `20 * log10(rms)`、rms ≤ 0 は `−120` |

### Tauri Commands

| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `get_capture_audio_controls` | なし | `CaptureAudioControlsState` | なし |
| `set_capture_audio_controls` | `Partial<CaptureAudioControls>`（送信フィールドのみ更新） | `CaptureAudioControlsState` | `INVALID_GAIN`, `INTERNAL` |

**`set_capture_audio_controls` 挙動**:
1. 送信フィールドを検証（`manual_ingest_gain` は有限数かつ 0.25–4.0）
2. `gain_user_adjusted` は `manual_ingest_gain` が送信されたとき `true` に設定（明示 `false` 送信でリセット可）
3. セッション状態を更新し、キャプチャ processing / `PcmIngestConsumer` に反映
4. キャプチャが `capturing` のとき即時反映（再起動不要、要件 1.4 / 3.2）
5. `capture-audio-controls://controls-changed` を emit
6. mic OFF 適用後に ingest 可能な音声源がない場合、`audio-capture://error`（`TRANSCRIBE_INGEST_NO_AUDIO_SOURCE`）を発行（要件 1.5）

**非キャプチャ時**（`idle` / `starting` / `stopping` / `error`）:
- invoke は成功するが UI は無効表示（要件 1.6 / 3.5）。ingest パスへは変更を適用しない。

### Tauri イベント

#### `capture-audio-controls://controls-changed`

```typescript
interface CaptureAudioControlsChanged {
  controls: CaptureAudioControls;
  timestamp_ms: number;
}
```

#### `capture-audio-controls://ingest-level`

キャプチャが `capturing` かつ ingest へ供給可能な間、**少なくとも 1 秒に 1 回**（実装は 1 Hz 集約を推奨）。生 PCM は含めない。

```typescript
interface IngestLevelChanged {
  level_dbfs: number;
  timestamp_ms: number;
}
```

非供給時（非キャプチャ、ingest 無効、音声源なし）は emit しない（要件 2.4）。

### Command エラー（invoke エラー payload）

| code | 条件 | message_ja 例 | action_ja 例 |
|------|------|---------------|--------------|
| `INVALID_GAIN` | 範囲外・NaN・Inf | ゲインの値が不正です | スライダーを中央付近に戻して再度お試しください |
| `INTERNAL` | 内部エラー | 音声設定の更新に失敗しました | アプリを再起動してください |

### Threat model（audio-device-selection 同等）

- ローカル単一ユーザー。追加 AuthN/AuthZ なし
- Tauri capability `allow-capture-audio-controls-commands` で command を許可リスト化
- presentation 層で入力検証・clamp。ログに PCM / 転写全文を出さない
- フロントへは dBFS メタデータのみ（要件 2.5）

## Non-goals

- OS レベルマイクミュート・システムミキサー操作
- 生 PCM のフロント配信
- 設定のディスク永続化（セッション内のみ）
- キャプチャ段ミキサー（−20 dBFS 目標）の変更
- 自動 AGC

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-10 | 初版 — マイク ingest トグル・手動ゲイン・dBFS メーター IPC | ADR-0014 |

## Notes

- TypeScript ミラー: `src/infrastructure/tauri/captureAudioControlsCommands.ts`、`src/presentation/hooks/capture-audio-controls-types.ts`
- UI 配置: `DeviceSelectorPanel` 内横並び（要件 4.1 / 4.5）
- ingest ゲイン実装正本: `PcmIngestConsumer`（`transcribe-volume-normalize` 固定定数は本契約の既定乗数に置換）
