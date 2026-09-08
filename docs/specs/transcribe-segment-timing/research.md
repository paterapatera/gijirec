# Gap Analysis: transcribe-segment-timing

## Summary

既存の Whisper ストリーミングパイプラインは `transcribe_worker.rs` の RMS フレーム VAD でエンドポイント検出を行い、`whisper_adapter.rs` で whisper.cpp 推論を実行する。セグメント区切りの主因は VAD 定数（特に `TRAILING_SILENCE_FRAMES` = 500 ms、`LONG_SILENCE_FRAMES` = 1.2 s）であり、Whisper 側の `single_segment` は意図的に `false`（日本語繰り返し回避）。

## Existing Implementation

| 領域 | 現状 | ギャップ |
|------|------|----------|
| VAD エンドポイント | `transcribe_worker.rs` 内 private const | ランタイム設定なし。チューニングは定数変更のみ |
| Whisper 推論 | `whisper_adapter.rs` — `single_segment=false`, `max_tokens=128`, dynamic `audio_ctx` | `entropy_thold` は steering に記載あるがコード未設定 |
| ブロック供給 | `BlockEmitter` → `TranscriptBlockBus` → Tauri event | 契約変更不要 |
| フロントエンド | `useTranscriptBlocks` がイベント購読 | 変更不要 |
| テスト | `transcribe_worker.rs` mod tests にエンドポイント検出テスト群 | 定数変更時に期待値更新が必要 |
| 手動検証 | `docs/manual/whisper-transcribe/performance-results.md` | ベースライン記録・調整後比較の追記が必要 |

## Key Parameters (baseline)

```
TRAILING_SILENCE_FRAMES = 5   (500 ms @ 100 ms/frame)
LONG_SILENCE_FRAMES     = 12  (1.2 s)
MIN_SPEECH_SAMPLES      = 16000 (1 s)
MAX_INFERENCE_WINDOW_SAMPLES = 160000 (10 s)
SILENCE_RMS_THRESHOLD   = 0.008
```

## Integration Points (unchanged)

- 入力: `PcmIngestConsumer` → rtrb (30 s ring) → `TranscribeWorker`
- 出力: `TranscriptSegmentSink::on_segment` → `BlockEmitter` → `whisper-transcribe://block-appended`
- 契約: `docs/contracts/whisper-transcribe-blocks.md`（5 s 遅延目標、追記のみ）

## Tuning Constraints (from steering)

- 1 軸ずつ変更、`bun run verify` + 実機確認
- `single_segment` / `entropy_thold` と窓長を同時変更しない
- revert 条件をメモしてから調整

## Recommended Approach

1. ベースライン計測（現行定数での区切り待ち時間）
2. `TRAILING_SILENCE_FRAMES` を 2〜3（200〜300 ms）に変更 → verify + 実機
3. 不足時のみ `LONG_SILENCE_FRAMES` を 6〜8（600〜800 ms）に変更
4. 品質問題（過剰分割・繰り返し）が出たら revert して別軸

## Risks

- 短い無音での過剰分割（句読点レベルの pause でブロック確定）
- 推論キューが skip-to-latest で古いウィンドウを捨てる頻度増加
- `stall_watchdog` の `SILENCE_RMS_THRESHOLD` 共有定数への影響なし（変更対象外）

---
_updated_at: 2026-09-08_
