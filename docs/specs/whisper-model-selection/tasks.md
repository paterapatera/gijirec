# Implementation Plan

## whisper-model-selection

- [x] 1. Foundation: ドメインカタログと設定永続化
- [x] 1.1 kotoba-whisper-v2.2 の 3 バリアント列挙とカタログ正本を定義する
  - `WhisperModelVariant` を Q5_0 / Q8_0 / FP16 の 3 値のみに固定し、serde 値は契約どおり `q5_0` / `q8_0` / `fp16` とする
  - 各バリアントの filename / HTTPS URL / SHA-256 を `docs/contracts/whisper-transcribe-settings.md` と一致させ、`default()` は FP16 を返す
  - 列挙外の値を受け付けない API 形状にし、将来追加は Revalidation Triggers に従う
  - 完了時: 3 バリアントのメタデータが単一カタログから参照可能で、契約表と byte 一致する単体テストが通る
  - _Requirements: 1.3, 1.4, 2.1, 2.2_
  - _Boundary: ModelVariantCatalog_
  - _Design: D-ModelVariantCatalog_
  - _Contracts: docs/contracts/whisper-transcribe-settings.md_
  - _Wave: 1_

- [x] 1.2 転写設定の読み書きサービスを実装する
  - `{app_data_dir}/transcribe-settings.json` に `model_variant` のみを UTF-8 JSON で保存・復元する
  - ファイル不存在・欠落・破損 JSON 時は FP16 既定で起動継続し、日本語通知経路を記録する（invoke エラーにしない）
  - 保存失敗時は `SETTINGS_PERSIST_FAILED` を返せるエラー型を定義し、転写本文・PCM・認証情報を永続化しない
  - 完了時: 欠落ファイルで fp16 が復元され、破損 JSON で既定 + エラー記録、正常保存で再起動後も選択が保持される
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Boundary: TranscribeSettingsService_
  - _Design: D-TranscribeSettingsService_
  - _Contracts: docs/contracts/whisper-transcribe-settings.md_
  - _Wave: 2_

- [x] 2. Core: バリアント対応モデルストア
- [x] 2.1 バリアント別パス解決・検証と既存 FP16 ファイル互換を実装する
  - `model_path(variant)` で 3 ファイル名を解決し、SHA-256 検証をバリアントごとに行う
  - 既存 `kotoba-whisper-v2.2-ggml.bin` を FP16 として追加取得なし利用できる（rename 不要）
  - legacy `%LOCALAPPDATA%/gijirec/models/` 移行を fp16 パスに適用し、取得済みファイルはオフラインで再利用可能にする
  - 完了時: 各バリアントの期待パスが解決され、既存 FP16 ファイルのみ存在する環境で verify が成功し DL 不要となる
  - _Requirements: 2.2, 2.5, 5.4_
  - _Boundary: ModelStore_
  - _Design: D-ModelStore_
  - _Depends: 1.1_
  - _Wave: 3_

- [x] 3. Core: オーケストレーションとサイクル境界切替
- [x] 3.1 選択バリアントの ensure フロー（取得・検証・ロード）を拡張する
  - ローカル未存在時のみ `ModelDownloader` で取得し、`loading_model` / `model-progress` / `ready` / `error` を既存イベント契約どおり発行する
  - ローカル検証済み存在時は追加 DL を呼ばず読み込み、同一バリアント再選択は no-op（永続化のみ）とする
  - DL / verify 失敗時は日本語 `message_ja` / `action_ja` 付きエラーを発行し、HTTPS-only + SHA-256 検証を維持する
  - 観測用に `model_variant_selected` / `model_variant_applied` ログと `transcribe_active_model_variant` メトリクスを追加する
  - 完了時: 未取得バリアント選択で DL→ready、既存ファイル選択で DL 未呼び出し、失敗時に error イベントが観測できる
  - _Requirements: 2.1, 2.3, 2.6, 3.1, 3.2, 3.3, 3.4_
  - _Boundary: ModelOrchestrator_
  - _Design: D-ModelOrchestrator_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Depends: 2.1, 1.1_
  - _Wave: 4_

- [x] 3.2 転写中バリアント切替を次バッチサイクル境界で適用する
  - `active_variant` と `pending_variant` を管理し、転写実行中の切替は現サイクル完了まで defer する
  - `TranscribeWorker` の `on_batch_cycle_started`（既存 observability 経路）から `try_apply_pending_variant` を呼び、新モデルパスを worker に供給する
  - アイドル / ready 時の切替は即時 reload し、実行中推論を中断しない
  - 完了時: 転写中に別バリアントを選んでも現バッチは旧モデル、次サイクルから新パスが使われる統合テストが通る
  - _Requirements: 2.4_
  - _Boundary: TranscribeWorker_
  - _Design: D-ModelOrchestrator_
  - _Depends: 3.1_
  - _Wave: 5_

- [x] 4. Core: Tauri 設定コマンド
- [x] 4.1 get / set 転写設定コマンドを公開する
  - `get_transcribe_settings` が `settings` と各バリアントの `local_availability` を返す
  - `set_transcribe_model_variant` が永続化後に `ModelOrchestrator::apply_variant` を呼び、列挙外は `INVALID_MODEL_VARIANT` とする
  - 保存失敗時は `SETTINGS_PERSIST_FAILED` を invoke エラーとして返す
  - 完了時: フロントから invoke 可能で、契約の request/response 形状とエラーコードが一致する
  - _Requirements: 1.1, 1.2, 4.1_
  - _Boundary: settings_commands_
  - _Design: D-TranscribeSettingsService_
  - _Contracts: docs/contracts/whisper-transcribe-settings.md_
  - _Depends: 1.2, 3.1_
  - _Wave: 6_

- [x] 5. Core: フロントエンド設定層と選択 UI
- [x] 5.1 (P) 転写設定 invoke ラッパーと React フックを追加する
  - `transcribeSettingsCommands.ts` で契約どおりの型と invoke を提供する
  - `useTranscribeSettings` が起動時取得・バリアント変更・`local_availability` 状態を保持する
  - 完了時: マウント時に settings が復元され、set 後に hook 状態が応答と同期される
  - _Requirements: 1.2, 4.1, 4.2_
  - _Boundary: useTranscribeSettings_
  - _Design: D-useTranscribeSettings_
  - _Contracts: docs/contracts/whisper-transcribe-settings.md_
  - _Depends: 4.1_
  - _Wave: 8_

- [x] 5.2 3 バリアント選択 UI を実装する
  - Q5_0 / Q8_0 / FP16 の 3 選択肢のみ提示し、現在選択中バリアントをラベルで明示する
  - `loading_model` 中は選択を disabled にし、`useTranscribeStatus` のフェーズ表示と整合させる
  - kotoba-whisper 以外・他量子化・自動推奨 UI を含めない
  - 完了時: UI に 3 選択肢と現在値が表示され、取得中は操作不可になる
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 3.2_
  - _Boundary: ModelVariantSelector_
  - _Design: D-ModelVariantSelector_
  - _Depends: 5.1_
  - _Wave: 9_

- [x] 6. Integration: 起動配線と UI 配置
- [x] 6.1 (P) compose 起動時のカタログ注入と設定復元を配線する
  - `compose.rs` の FP16 単一定数を `ModelVariantCatalog` 3 定義へ移行する
  - Tauri setup 後に settings 復元 → orchestrator 初期化 → 選択バリアントの ensure を開始する
  - `inject_model_stack` 経路で `app_data_dir` 注入を維持する
  - 完了時: 初回起動で fp16 既定 ensure、保存済み選択で該当バリアント ensure が開始される
  - _Requirements: 4.2, 4.3, 5.4_
  - _Boundary: compose_
  - _Depends: 1.2, 3.1, 4.1_
  - _Wave: 7_

- [x] 6.2 設定パネルへ選択 UI を配置し既存転写フローを維持する
  - `ModelVariantSelector` を DeviceSelectorPanel / 設定と同階層に配置する
  - マウント時 `useTranscribeStatus` 同期と `whisper-transcribe://*` 購読を変更せず、キャプチャ→転写ブロック追記が動作する
  - クラウド STT や他モデルファミリ切替 UI を追加しない
  - 完了時: バリアント選択導入後も主要フローとマウント時状態同期が従来どおり動作する
  - _Requirements: 5.1, 5.2, 5.3_
  - _Boundary: App_
  - _Depends: 5.2, 6.1_
  - _Wave: 10_

- [x] 7. Validation: テストと回帰確認
- [x] 7.1 ドメイン・インフラ・設定・オーケストレータの単体テストを追加する
  - カタログ 3 エントリの filename/SHA 整合、各バリアント path 解決、FP16 既存ファイル互換
  - settings 欠落→default fp16、破損 JSON→default + エラー記録
  - orchestrator: ローカル存在時 DL 未呼び出し、同一 variant no-op
  - 完了時: design Testing Strategy の Unit Tests 節の観点が cargo test で緑になる
  - _Requirements: 2.2, 4.3, 4.4, 5.4_
  - _Depends: 3.1, 1.2, 2.1_
  - _Wave: 11_

- [x] 7.2 コマンド・イベント・サイクル境界の統合テストを追加する
  - `set_transcribe_model_variant` → `loading_model` → `ready` イベント系列（モック HTTP）
  - 起動時 settings 復元 → 正しい variant で orchestrator 初期化
  - 転写中 variant 変更 → 現サイクル完了後に worker が新パスを使用
  - 完了時: design Testing Strategy の Integration Tests 節が cargo test で緑になる
  - _Requirements: 2.1, 2.4, 4.2, 5.1_
  - _Depends: 6.1, 3.2, 4.1_
  - _Wave: 12_

- [x] 7.3* (P) 選択 UI と主要フローの E2E スモークを追加する
  - 3 選択肢表示と現在選択ラベル（要件 1.1, 1.2）
  - バリアント変更後の次バッチ転写継続スモーク
  - マウント時 `get_transcribe_status` 同期が従来どおり（要件 5.1）
  - 完了時: E2E または UI スモークで上記が自動確認できる（MVP 後 defer 可）
  - _Requirements: 1.1, 1.2, 5.1, 5.2_
  - _Depends: 6.2_
  - _Wave: 13_
