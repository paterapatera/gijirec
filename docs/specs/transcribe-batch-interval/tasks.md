# 実装計画

## 概要

VAD 駆動ストリーミングから **30 秒固定バッチ推論**へ切り替え、推論中も PCM を欠落させない。既存 `whisper-transcribe://block-appended` 供給規約を維持する。

## タスク

- [ ] 1. 基盤: PCM 非破棄バッファ基盤
- [x] 1.1 (P) compose 側 rtrb 容量をバックログ深さに応じて拡張する
  - 30 s 窓推論 worst-case とバックログ退避を見込んだ rtrb 容量に引き上げる
  - 満杯時に ingest が Internal エラーで落ちないことを確認できる
  - 関連する既存 Rust テストが通る状態になる
  - _Requirements: 2.1, 2.2, 5.1_
  - _Boundary: HostComposition_
  - _Wave: 1_

- [x] 1.2 (P) PcmChunkBus のキュー上限を引き上げ、長時間推論中のチャンク drop を防ぐ
  - `MAX_QUEUED_CHUNKS` を推論 worst-case / 100 ms チャンクで決定し定数化する
  - 溢れ時に最古 drop が発生しないことをテストまたは定数根拠で確認できる
  - _Requirements: 2.1, 2.2, 5.1_
  - _Boundary: PcmChunkBus_
  - _Design: D-PcmChunkBus_
  - _Wave: 2_

- [x] 1.3 (P) PcmIngestConsumer の rtrb push を非破棄戦略に変更する
  - rtrb 満杯時に Internal エラーを返さず、compose 側拡張容量と整合する
  - `on_pcm_chunk` 呼び出し中にサンプルが失われないことを確認できる
  - _Requirements: 2.1, 2.2_
  - _Boundary: PcmIngestConsumer_
  - _Design: D-PcmIngestConsumer_
  - _Depends: 1.1_
  - _Wave: 3_

- [ ] 2. コア: バッチ窓切り出しと drop 経路除去
- [x] 2.1 VecDeque 非破棄 drain と take_batch_window を実装する
  - `MAX_INFERENCE_WINDOW_SAMPLES = 480_000`、`MAX_PCM_BUFFER_SAMPLES` を 10 分相当以上に拡張する
  - drop-oldest と skip-to-latest を除去し、先頭 480k samples と `samples_before_buffer` を返す
  - 溢れ入力でも蓄積サンプル数が単調増加することを単体テストで確認できる
  - _Requirements: 1.1, 1.3, 1.4, 2.1, 2.2, 5.2_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 1.1, 1.2, 1.3_
  - _Wave: 4_

- [x] 2.2 worker_loop にサイクル完了起点 30 秒タイマーと初回起動条件を実装する
  - `BATCH_INTERVAL = 30 s`、前サイクル完了 + 30 s かつ未処理 ≥ 1 sample で推論を開始する
  - transcribing 開始時は 30 s 経過または未処理 PCM ≥ 480k で初回サイクルを起動する
  - タイミング条件を単体テストで検証できる
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 4.1_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 2.1_
  - _Wave: 5_

- [x] 2.3 バックログ連続サイクル・停止 flush・推論失敗継続を実装する
  - 未処理 PCM 残存時は 30 s 待機をスキップし連続サイクルを実行する
  - キャプチャ停止時に残 PCM を最終バッチとして flush する
  - 推論失敗時はログ後に次サイクルを継続し、キャプチャは停止しない
  - _Requirements: 2.3, 2.4, 6.3_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 2.2_
  - _Wave: 6_

- [x] 2.4 バッチ窓起点の転写ブロック供給と下流契約整合を実装する
  - `start_timestamp_ms` をバッチ窓先頭（`samples_before_buffer`）基準で算出する
  - 空転写はブロックを発行せず、既存 BlockEmitter の追記のみ供給を維持する
  - `sequence` 単調増加・欠番なしをテストで確認できる
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 5.3, 5.4_
  - _Boundary: TranscribeWorker, BlockEmitter_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Depends: 2.3_
  - _Wave: 7_

- [ ] 3. コア: ライフサイクルと可観測性
- [x] 3.1 (P) TranscribeLifecycleHook と stall_watchdog を 30 s バッチ向けに調整する
  - 既存 lifecycle flush パスが停止時最終バッチ処理と整合することを確認する
  - stall_watchdog 閾値を 30 s 窓向けに再調整する
  - ウィンドウ閉じでキャプチャおよび文字起こしが完全停止することを維持する
  - _Requirements: 2.3, 4.1, 4.3, 5.1, 5.2_
  - _Boundary: TranscribeLifecycleHook_
  - _Design: D-TranscribeLifecycleHook_
  - _Depends: 2.3_
  - _Wave: 8_

- [x] 3.2 (P) バッチサイクル可観測性メトリクスとログを追加する
  - `batch_cycle_started` / `batch_cycle_completed`、`transcribe_pcm_backlog_seconds`、`transcribe_rtrb_overflow_count` を記録する
  - PCM 全文・転写全文はログに出力しない
  - 手動検証で backlog と録音長を照合できる情報が tracing に出る
  - _Requirements: 6.1, 4.1, 5.4_
  - _Boundary: TranscribeWorker_
  - _Depends: 2.3_
  - _Wave: 9_

- [x] 4. 統合: 拡張バッファとバッチ worker の compose 結線を検証する
  - 合成 PCM → drain → batch worker → モック adapter の end-to-end が動作する
  - WhisperCppAdapter の API 形状およびモデル取得経路を変更しない
  - 音声キャプチャ方式・話者分離・クラウド STT に触れない
  - _Requirements: 1.1, 2.1, 4.2, 5.1, 5.2, 5.3, 5.4_
  - _Depends: 2.4, 3.1, 3.2_
  - _Wave: 10_

- [ ] 5. 検証: 自動テストと手動検証
- [x] 5.1 transcribe_integration.rs にバッチパイプライン統合テストを追加する
  - 合成 PCM → バッチ worker → モック adapter → `sequence` 欠番なしを確認する
  - 推論失敗注入後も次サイクルが実行されることを確認する
  - 停止 flush で残 PCM が処理されることを確認する
  - _Requirements: 2.3, 2.4, 3.1, 3.2, 6.3_
  - _Depends: 4_
  - _Wave: 11_

- [x] 5.2* take_batch_window・タイミング・タイムスタンプ単調性の追加ユニットテスト
  - design Testing Strategy の Unit Tests 節の残項目をカバーする
  - BlockEmitter 空テキスト入力でブロック不発行（3.4）を確認する
  - _Requirements: 1.1, 1.2, 3.2, 3.4, 6.2_
  - _Depends: 2.4_
  - _Wave: 12_

- [ ] 5.3 要件 6 の手動検証チェックリストを実機で実行する
  - 6.1: 10 分連続キャプチャ＋文字起こしで backlog と録音長を照合し欠落なしを確認する
  - 6.2: 連続発話中、各ブロックの `start_timestamp_ms` が直前バッチ境界から 30 s 以内であることを確認する
  - 6.3: CPU 負荷で推論遅延を発生させ、停止後に全期間がブロックでカバーされることを確認する
  - _Requirements: 6.1, 6.2, 6.3_
  - _Depends: 4, 5.1_
  - _Wave: 13_

## Implementation Notes

- compose テストは `-p gijirec -- compose::` で実行する（`-p gijirec-presentation` ではマッチしない）
- `gijirec-infrastructure` から `gijirec-application` への dev-dep は boundaries 違反。契約テストは `block_emitter.rs` 側に置く
- バッチ可観測性は worker コールバック → presentation `observability` → host tracing。`transcribe_rtrb_overflow_count` は ingest の共有 `AtomicU64` を worker がサイクル開始時に読む
