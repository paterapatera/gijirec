# 実装計画: whisper-transcribe

## 概要

既存 4 crate に `transcribe/` モジュールを追加し、上流 audio-capture の PCM 契約を消費してローカル whisper.cpp 推論・タイムスタンプ付きテキストブロック供給を実現する。Foundation → Core（境界別並列）→ Integration → Validation の順で実装する。

---

- [x] 1. Foundation: 依存追加とアーキテクチャ骨格
- [x] 1.1 whisper-cpp-plus・rtrb・HTTP クライアント依存と transcribe モジュール骨格
  - `gijirec-infrastructure/Cargo.toml` に `whisper-cpp-plus`（macOS は `metal` feature）、`rtrb`、モデル取得用 HTTP クライアント（`reqwest` または `ureq`）を追加する
  - 4 crate に `transcribe/mod.rs` 空骨格を作成し、各 `lib.rs` から公開する
  - Tauri capabilities / permissions にモデル取得（HTTPS）と文字起こしイベント emit に必要な権限を追加する
  - 完了時: `cargo check` が 4 crate すべてで通り、transcribe モジュールがビルド対象に含まれる
  - _Requirements: 2.3, 5.1, 9.1_
  - _Wave: 1_

- [x] 1.2 Application port トレイト定義
  - `TranscribeWorkerPort`（`spawn` / `stop_and_join`）と `WhisperContextPort`（`load_model`）を application 層に定義する
  - infrastructure 具象への直接依存が application 層に存在しないことを cargo bylaw で確認できる
  - 完了時: port トレイトがコンパイルされ、presentation composition root から注入可能な型として公開される
  - _Requirements: 2.1, 6.1, 6.4, 6.5_
  - _Depends: 1.1_
  - _Wave: 2_

- [x] 2. ドメイン層: 文字起こしコア型
- [x] 2.1 (P) TranscriptBlock 値オブジェクトと TranscriptBlockConsumer トレイト
  - 契約どおりのフィールド（`block_id`, `sequence`, `text`, `start_timestamp_ms`, `language`）を持つ不変値オブジェクトを実装する
  - 下流登録用 `TranscriptBlockConsumer` トレイトと `TranscriptConsumerError` を定義する
  - 推論ワーカーから application 層へセグメントを渡す `TranscriptSegmentSink` トレイトを domain 層に定義する（cargo bylaw 準拠の層間結線用）
  - 完了時: 契約フィールド制約を満たすブロックを構築するユニットテストが通り、空文字列ブロックの構築が拒否される
  - _Requirements: 3.5, 4.1, 4.2, 4.3, 9.2_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Boundary: TranscriptBlockTypes_
  - _Design: D-TranscriptBlock_
  - _Depends: 1.1_
  - _Wave: 3_

- [x] 2.2 (P) TranscribePhase 状態列挙
  - `idle | loading_model | ready | transcribing | stopping | error` を表す列挙型と遷移ヘルパを実装する
  - 完了時: 設計の状態図どおりの合法遷移のみ許可し、非法遷移はテストで拒否される
  - _Requirements: 5.1, 6.1, 6.4_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Boundary: TranscribeState_
  - _Design: D-TranscribePhase_
  - _Depends: 1.1_
  - _Wave: 4_

- [x] 2.3 TranscribeError と UserFacingTranscribeError マッピング
  - 内部 `TranscribeError` から契約の `code` / `message_ja` / `action_ja` / `recoverable` へマップする `to_user_facing()` を実装する
  - 全エラーコード（`MODEL_DOWNLOAD_FAILED`, `MODEL_CORRUPT`, `MODEL_NOT_FOUND`, `INFERENCE_FAILED`, `UPSTREAM_CAPTURE_ERROR`, `INTERNAL`）を網羅する
  - 完了時: 各内部エラーが契約どおりの利用者向けペイロードに変換され、`action_ja` が空にならない
  - _Requirements: 5.4, 5.5, 8.1, 8.2, 8.4_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Boundary: TranscribeErrors_
  - _Design: D-TranscribeError_
  - _Depends: 2.2_
  - _Wave: 5_

- [ ] 3. インフラストラクチャ層: Whisper 推論とモデル I/O
- [x] 3.1 (P) WhisperCppAdapter（WhisperContextPort 実装）
  - `whisper-cpp-plus` の `WhisperContext` をロードし、`WhisperStreamPcm`（VAD 駆動、`length_ms=5000`）で推論するラッパを実装する
  - モデルロード失敗を `MODEL_CORRUPT` に変換し、クラウド送信・Python ランタイムを要求しない
  - 完了時: ローカルモデルパスからコンテキストがロードされ、合成 PCM 入力でテキストセグメントが返るスモークテストが通る
  - _Requirements: 2.1, 2.2, 2.3, 5.3_
  - _Boundary: WhisperCppAdapter_
  - _Design: D-WhisperCppAdapter_
  - _Depends: 1.1, 1.2, 2.3_
  - _Wave: 6_

- [x] 3.2 (P) ModelStore（ローカルモデルパス・整合性検証）
  - Tauri `app_data_dir/models/kotoba-whisper-v2.2-ggml-q5_0.bin` のパス解決と SHA-256 整合性検証を実装する
  - 破損・読み込み不能ファイルを `MODEL_CORRUPT` に変換し、部分ダウンロードファイルは削除する
  - 完了時: 存在/不存在/破損ファイルの 3 パターンを検出するユニットテストが通る
  - _Requirements: 5.2, 5.5, 9.1_
  - _Boundary: ModelStore_
  - _Design: D-ModelStore_
  - _Depends: 1.1, 2.3_
  - _Wave: 7_

- [x] 3.3 (P) ModelDownloader（HTTPS モデル取得）
  - HuggingFace（`kenrouse/kotoba-whisper-v2.2-ggml`）から `kotoba-whisper-v2.2-ggml-q5_0.bin` を HTTPS（TLS 1.2+）で取得し、進捗コールバックを発火する
  - 取得失敗時は部分ファイルを削除し `MODEL_DOWNLOAD_FAILED` を返す
  - 完了時: モック HTTP サーバで進捗コールバック系列（`downloading` → `verifying` → `complete`）が期待どおり発火する
  - _Requirements: 5.1, 5.4, 9.1_
  - _Boundary: ModelDownloader_
  - _Design: D-ModelDownloader_
  - _Depends: 1.1, 2.3_
  - _Wave: 8_

- [x] 3.4 TranscribeWorker（推論ワーカー本体）
  - 専用スレッドで rtrb から PCM を drain し、VAD 駆動推論を実行する（スレッド優先度 `BelowNormal`）
  - 推論セグメントは domain 層の `TranscriptSegmentSink` コールバックへ渡す（application の `BlockEmitter` への直接依存は composition root で結線）
  - 空テキスト・空白のみセグメントは sink へ渡さず、推論レイテンシを `transcribe_inference_latency_ms` メトリクスに記録する
  - 停止シグナル受信後は進行中推論を完了または中断し、コンテキストを drop してスレッドを join する
  - 完了時: 合成 PCM 入力でセグメントが sink コールバックへ渡され、停止後にワーカースレッドが残存しないことをテストで確認できる
  - _Requirements: 2.1, 3.2, 3.3, 6.5, 7.1_
  - _Boundary: TranscribeWorker_
  - _Design: D-TranscribeWorker_
  - _Depends: 1.1, 2.3, 3.1_
  - _Wave: 12_

- [ ] 4. アプリケーション層: オーケストレーションとブロック生成
- [x] 4.1 BlockEmitter（推論結果 → TranscriptBlock 変換）
  - whisper セグメントを `TranscriptBlock` に変換し、`sequence` 単調増加と `start_timestamp_ms`（`PcmChunk.timestamp_ms` 整合）を付与する
  - 空テキスト・空白のみセグメントは発行せず、既発行ブロックの変更・撤回を行わない（追記のみ）
  - キャプチャセッション境界で `timestamp_ms` 基準がリセットされ、`sequence` はセッション内で単調増加する
  - 完了時: 空テキスト非発行・sequence 単調増加・timestamp_ms 計算のユニットテストが通る
  - _Requirements: 2.4, 3.1, 3.3, 3.5, 4.1, 4.2, 4.3_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Boundary: BlockEmitter_
  - _Design: D-BlockEmitter_
  - _Depends: 2.1_
  - _Wave: 9_

- [x] 4.2 ModelOrchestrator（モデル存在確認・取得オーケストレーション）
  - 起動時に `ModelStore` でローカルモデル存在を確認し、未取得時は `ModelDownloader` で取得を開始する
  - オフライン初回起動時は `MODEL_NOT_FOUND` を返し、取得完了後は `ready` フェーズへ遷移可能にする
  - 完了時: モデル存在/不存在/取得失敗の各シナリオで期待フェーズとエラーコードが返るユニットテストが通る
  - _Requirements: 5.1, 5.2, 5.4, 5.5_
  - _Boundary: ModelOrchestrator_
  - _Design: D-ModelOrchestrator_
  - _Depends: 2.3, 3.2, 3.3_
  - _Wave: 10_

- [x] 4.3 DefaultTranscribeOrchestrator（フェーズ管理・開始/停止ゲート）
  - `ready` + 上流 `capturing` のときのみ `transcribing` へ遷移し、`TranscribeWorkerPort::spawn` でワーカーを起動する
  - `stop` で `stopping` → ワーカー join → `ready`/`idle` へ遷移し、回復不能エラーで `error` へ遷移して新規ブロック供給を停止する
  - `ensure_model` で `ModelOrchestrator` を呼び出し、モデル未取得時は `loading_model` フェーズへ遷移する
  - 完了時: capturing + ready で start、idle で stop、推論失敗で error への遷移がユニットテストで検証される
  - _Requirements: 6.1, 6.4, 8.1_
  - _Boundary: TranscribeOrchestrator_
  - _Design: D-TranscribeOrchestrator_
  - _Depends: 1.2, 2.2, 2.3, 4.2_
  - _Wave: 11_

- [ ] 5. プレゼンテーション層（Rust）: PCM 受信・イベント・ライフサイクル
- [x] 5.1 PcmIngestConsumer（PcmChunkConsumer 実装）
  - `PcmChunkBus` からの同期コールバックで PCM を rtrb に非ブロッキング push するのみ（推論・I/O を実行しない）
  - 順序欠落（sequence 欠番）を検出しても処理継続し、欠番を `transcribe_pcm_sequence_gaps` メトリクスに記録する
  - PCM をファイル・ネットワークへ送らない（音声キャプチャのデバイス取得・ミキシング・権限要求は所有しない）
  - 完了時: push のみで即時 return し、欠番検出後も次チャンクを受理するユニットテストが通る
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 7.1_
  - _Contracts: docs/contracts/audio-capture-pcm.md_
  - _Boundary: PcmIngestConsumer_
  - _Design: D-PcmIngestConsumer_
  - _Depends: 1.1_
  - _Wave: 13_

- [x] 5.2 TranscriptBlockBus（下流ブロック配信）
  - `TranscriptBlock` を単一下流 consumer と Tauri `whisper-transcribe://block-appended` へ追記のみ配信する
  - メモリリング最大 500 ブロック。超過時は最古破棄し `transcribe_block_buffer_drops` メトリクスを記録する
  - 転写テキストを外部ネットワークへ送信せず、ユーザー明示操作なしのディスク永続化を行わない
  - 完了時: ブロック追記で Tauri イベントが emit され、501 件目で最古ブロックが破棄されることをテストで確認できる
  - _Requirements: 3.1, 3.5, 9.2, 9.4_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Boundary: TranscriptBlockBus_
  - _Design: D-TranscriptBlockBus_
  - _Depends: 2.1_
  - _Wave: 14_

- [x] 5.3 TranscribeEventEmitter（UI 向けイベント emit）
  - `whisper-transcribe://phase-changed`、`whisper-transcribe://model-progress`、`whisper-transcribe://error` を Tauri へ emit する
  - エラー通知に `action_ja` を必ず含め、転写テキスト全文・PCM 生データをペイロードに含めない
  - 完了時: 各フェーズ遷移とモデル進捗・エラー code が契約形状どおり emit される統合テストが通る
  - _Requirements: 5.1, 5.4, 5.5, 8.1, 8.2, 8.4_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Boundary: TranscribeEventEmitter_
  - _Design: D-TranscribeEventEmitter_
  - _Depends: 2.2, 2.3_
  - _Wave: 15_

- [x] 5.4 TranscribeLifecycleHook（キャプチャ・アプリ終了連動）
  - `CaptureProcessingHook::on_capture_started` → `TranscribeOrchestrator::start`（モデル ready 時）
  - `on_capture_stopping` / アプリ終了（`RunEvent::Exit`）→ `stop` + ワーカー `join`（最大 5 s）。タイムアウト時は WARN ログ + 強制中断
  - `audio-capture://phase-changed` の `error` 購読 → 新規 PCM 処理停止、`UPSTREAM_CAPTURE_ERROR` 発行。`capturing` 復帰で再開
  - 完了時: キャプチャ開始で transcribing へ、停止・終了でワーカー join 完了しバックグラウンドスレッドが残存しない
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 8.3_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: TranscribeLifecycleHook_
  - _Design: D-TranscribeLifecycleHook_
  - _Depends: 4.3, 5.3_
  - _Wave: 16_

- [ ] 6. 統合結線: composition root と観測性
- [x] 6.1 composition root 結線と観測性設定
  - `gijirec-presentation::lib.rs` で `PcmChunkBus::register` に `PcmIngestConsumer` を登録し、presentation 層の port アダプタ（`TranscribeWorkerPortAdapter`, `WhisperContextPortAdapter`）が infrastructure 具象をラップして `DefaultTranscribeOrchestrator` に注入する
  - `BlockEmitter` を `TranscriptSegmentSink` として `TranscribeWorker` に結線し、`TranscriptBlockBus`・`TranscribeEventEmitter`・`TranscribeLifecycleHook` を lifecycle に接続する
  - `boundaries.md` の whisper-transcribe セクションを `PcmIngestConsumer` + rtrb 表記に同期する
  - ログターゲット `gijirec_transcribe`、転写テキスト・PCM・モデル URL トークンのマスキング、`session_id` 付与を設定する
  - 完了時: `cargo tauri dev` でアプリ起動し、PCM バス登録・フェーズイベント・モデル取得フローがエラーなく初期化される
  - _Requirements: 1.1, 2.1, 6.1, 6.2, 6.3, 6.5, 8.3, 8.4, 9.1_
  - _Depends: 3.4, 4.1, 4.2, 4.3, 5.1, 5.2, 5.3, 5.4_
  - _Boundary: CompositionRoot_
  - _Wave: 17_

- [ ] 7. フロントエンド: 文字起こしステータス UI
- [x] 7.1 useTranscribeStatus フック
  - `whisper-transcribe://phase-changed`、`model-progress`、`error` を購読し、契約ミラー型で React 状態を更新する
  - テスト時は injectable `listenFn` で Tauri なし単体テスト可能にする
  - 完了時: モックイベント注入で phase / progress / error 状態が期待どおり更新されるフロント単体テストが通る
  - _Requirements: 5.1, 8.2_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Boundary: UseTranscribeStatus_
  - _Design: D-UseTranscribeStatus_
  - _Depends: 6.1_
  - _Wave: 18_

- [x] 7.2 App.tsx 文字起こしステータス・モデル進捗表示
  - 既存キャプチャステータス表示に文字起こしフェーズ・モデル取得進捗バー・エラーメッセージ（`action_ja` 含む）を統合する
  - 手動編集 UI・部分ロック・Markdown 出力・認証 UI は追加しない
  - 完了時: アプリ起動で `loading_model` 進捗が表示され、キャプチャ中は `transcribing`、モデル破損時はエラーと `action_ja` が画面に表示される
  - _Requirements: 3.4, 4.4, 5.1, 5.4, 5.5, 6.1, 9.3_
  - _Depends: 7.1_
  - _Wave: 19_

- [x] 8. 検証: テスト・性能・手動チェックリスト
- [x] 8.1 ユニットテスト（設計 Testing Strategy Unit 1–6）
  - `BlockEmitter`（空テキスト非発行、sequence、timestamp_ms）、`TranscribeError::to_user_facing`（全 code + action_ja 非空）、`ModelStore`（存在/不存在/破損）、`PcmIngestConsumer`（push のみ即時 return）、`TranscribeOrchestrator`（capturing + ready で start）、`ModelOrchestrator`（オフライン初回 `MODEL_NOT_FOUND`）を実装する
  - 完了時: 上記 6 コンポーネントのユニットテストが `cargo test` で通る
  - _Requirements: 1.2, 4.1, 4.3, 5.4, 5.5, 6.1, 6.4, 8.2_
  - _Depends: 2.3, 3.2, 4.1, 4.2, 4.3, 5.1, 5.4_
  - _Wave: 20_

- [x] 8.2 統合テスト（設計 Integration 1–5）
  - 合成 PCM → ブロック emit（モック WhisperAdapter）、`PcmChunkBus` 登録 → consumer 経由ワーカー到達、キャプチャ `error` → 推論停止 → `capturing` 復帰再開、`stop` → ワーカー join 完了、モデルダウンロードモック → `model-progress` 系列を検証する
  - 完了時: presentation crate の統合テスト 5 件が `cargo test` で通る
  - _Requirements: 1.1, 2.1, 3.3, 5.1, 6.5, 8.3_
  - _Depends: 6.1_
  - _Wave: 21_

- [x] 8.3 E2E/UI テスト（設計 E2E 1–4）
  - アプリ起動 → モデル未取得時 `loading_model` 表示と進捗バー、キャプチャ中 → `transcribing` 表示、ウィンドウ閉鎖 → 5 秒以内プロセス終了・推論スレッド残存なし、モデル破損ファイル → エラーメッセージと `action_ja` 表示を自動テストで検証する
  - 完了時: フロント E2E テスト 4 件が `bun test` で通る
  - _Requirements: 5.1, 5.5, 6.1, 6.2, 6.5_
  - _Depends: 7.2_
  - _Wave: 22_

- [x] 8.4 性能検証と performance-results.md 作成
  - 10 分連続転写で `transcribe_inference_latency_ms` p95 < 5000 ms、キャプチャ + 転写同時で追加 CPU 平均 < 25%（4 コア）・ピーク < 50%、常駐メモリ増分 < 400 MB、Web 会議アプリ並行で音声途切れなしを手動計測する
  - 3 s 発話 → 5 s 以内にブロック表示（設計 E2E 5）を手動チェックリストに含め、`docs/specs/whisper-transcribe/performance-results.md` に結果を記録する
  - 完了時: `performance-results.md` に各合格基準の計測値と pass/fail が記載されている
  - _Requirements: 3.2, 7.1, 7.2_
  - _Depends: 8.2_
  - _Wave: 23_

- [x] 8.5 スコープ外機能の非実装確認
  - 手動編集 UI・部分ロック・Markdown 出力（3.4, 4.4）、音声キャプチャ所有（1.4）、ユーザー認証・認可（9.3）が本 spec スコープ外として未実装であることをコードベース走査で確認する
  - 完了時: 上記機能に該当する UI・API・永続化が存在しないことをチェックリストで記録できる
  - _Requirements: 1.4, 3.4, 4.4, 9.3_
  - _Depends: 7.2_
  - _Wave: 24_
