# Implementation Plan

- [ ] 1. ベースライン計測と記録
  - 現行 `TRAILING_SILENCE_FRAMES`（5）・`LONG_SILENCE_FRAMES`（12）の値を `docs/manual/whisper-transcribe/performance-results.md` に記録する
  - 実機で 3 秒連続発話 → `block-appended` までの待ち時間を 3 回計測し中央値をベースラインとして記録する
  - _Requirements: 1.2, 3.3_
  - _Wave: 1_

- [ ] 2. VAD エンドポイント定数の第一軸調整
- [x] 2.1 `TRAILING_SILENCE_FRAMES` を 2 または 3 に変更する
  - `transcribe_worker.rs` の定数のみ変更し、`LONG_SILENCE_FRAMES` は触らない
  - `bun run verify` がパスすることを確認する
  - _Requirements: 1.1, 1.3, 1.4, 4.1_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 1_
  - _Wave: 2_

- [x] 2.2 エンドポイント検出ユニットテストの期待値を更新する
  - `mod tests` 内の trailing silence cut / long-pause 関連テストが新定数でパスする
  - `cargo test -p gijirec-infrastructure transcribe` がパスする
  - _Requirements: 3.2_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 2.1_
  - _Wave: 2_

- [ ] 3. 実機品質確認と第二軸判断
- [ ] 3.1 第一軸調整後の実機確認を行う
  - 3 秒連続日本語発話で過剰分割（1 発話が 2+ ブロック）がないことを確認する
  - 区切り待ち時間がベースライン比 50% 以上短縮されていることを確認する
  - 同一フレーズ 3 回連続出現がないことを確認する
  - _Requirements: 1.1, 2.2, 2.3, 3.4_
  - _Depends: 2.2_
  - _Wave: 3_

- [ ] 3.2 不足時のみ `LONG_SILENCE_FRAMES` を第二軸で調整する
  - 3.1 で 50% 短縮未達の場合のみ、12 → 6 または 8 に変更する（`TRAILING_SILENCE_FRAMES` は固定）
  - 品質問題が出た場合は revert し、調整結果を `performance-results.md` に記録する
  - _Requirements: 1.3, 2.1, 3.3_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 3.1_
  - _Wave: 3_

- [ ] 4. 最終検証と記録
  - `bun run verify` を実行し全チェックがパスする
  - 調整後の定数値・計測結果・品質確認結果を `performance-results.md` に追記する
  - `whisper-transcribe-blocks` 契約（空ブロックなし、追記のみ、5 秒遅延目標）を手動で確認する
  - _Requirements: 2.4, 3.1, 3.3, 3.4, 4.2, 4.3_
  - _Depends: 3.1, 3.2_
  - _Wave: 4_

## Implementation Notes

- `TRAILING_SILENCE_FRAMES` を 5 → 3 に変更（300 ms）。`cuts_utterance_at_trailing_silence` の残りサンプル期待値を `9_600 - TRAILING_SILENCE_FRAMES * FRAME_SAMPLES` に定数参照化。
- タスク 1 の定数記録・タスク 4 の `bun run verify` は完了。実機計測（タスク 1 タイミング / 3.1 / 3.2 / 4 手動 E2E）は CI 環境不可のため `performance-results.md` に未確認として記録済み。
