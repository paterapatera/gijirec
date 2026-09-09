# Implementation Plan

- [x] 1. 転写 ingest ゲイン定数とソフトリミッターの実装
  - `pcm_ingest_consumer.rs` に `TRANSCRIBE_INGEST_GAIN`（**1.25**）と `TRANSCRIBE_SOFT_LIMIT`（0.95）を追加し、i16→f32 変換後のサンプルループでゲイン適用＋ソフトリミットしてから rtrb に push する
  - ミキサー（`mixer.rs`）および `PcmChunkBus` の i16 ペイロードは変更しない
  - _Requirements: 1, 4_
  - _Boundary: PcmIngestConsumer_
  - _Design: D-PcmIngestConsumer_
  - _Wave: 1_

- [x] 2. ユニットテスト（ingest ゲイン）
  - 無音相当（RMS < 0.008）の入力がゲイン後も 0.008 未満であることを検証する
  - 代表入力（線形 RMS ≈ 0.007、約 −23 dBFS 相当）に ×1.45 適用後、RMS が 0.12〜0.13（−18〜−17 dBFS）付近に収まることを検証する
  - ピーク 0.9 相当の入力でソフトリミット後が 0.95 以下であることを検証する
  - _Requirements: 1, 2_
  - _Boundary: PcmIngestConsumer_
  - _Design: D-PcmIngestConsumer_
  - _Depends: 1_
  - _Wave: 2_

- [x] 3. 推論窓 RMS がゲイン後 PCM を反映することの確認
  - `transcribe_worker.rs` の window RMS 計測が rtrb 上の f32（ingest ゲイン後）に対して行われることをコードレビューで確認し、必要ならコメントを追加する（ロジック変更は不要な場合はコメントのみ）
  - _Requirements: 2, 3_
  - _Boundary: TranscribeWorkerRms_
  - _Design: D-TranscribeWorkerRms_
  - _Depends: 1_
  - _Wave: 2_

- [x] 4. 可観測性テストの更新
  - `src-tauri/tests/transcribe_observability.rs`（および関連 fixture）がゲイン適用後の RMS フィールドを引き続き検証できるよう、必要なら期待値を調整する
  - `transcribe_window_rms_dbfs` および ingest サマリフィールドが引き続き emit されることを確認する
  - _Requirements: 3_
  - _Depends: 1, 3_
  - _Wave: 3_

- [x] 5. 品質ゲートと手動検証
  - `bun run verify` が成功することを確認する
  - 快適 OS 音量で実機キャプチャし、`transcribe_window_rms_dbfs` が −18〜−17 dBFS 付近であることと転写精度の改善を手動確認する（手順を PR / メモに 1 行記載）
  - _Requirements: 3_
  - _Depends: 1, 2, 3, 4_
  - _Wave: 4_

## Implementation Notes

- `TRANSCRIBE_INGEST_GAIN` / `TRANSCRIBE_SOFT_LIMIT` を `pcm_ingest_consumer.rs` に集約（ゲイン **1.25**、実機ログに基づき 1.45 から調整）。RMS コールバックもゲイン後値を使用。
- 手動検証: 快適 OS 音量でキャプチャ → `20260908T235433Z` ログで 30s 窓 RMS 平均 **約 −17.1 dBFS**（要件 −18〜−17 付近）。
- 品質ゲート: `bun run verify` 成功（2026-09-09）。
