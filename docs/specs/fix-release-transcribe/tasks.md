# 実装計画

## 1. Foundation: composition 分割とアーキテクチャ正本

- [x] 1.1 compose の ModelStore 遅延注入構造への分割
  - `build_capture_stack()` から `dirs::data_local_dir` 依存を除去し、capture + transcribe wiring のみを Tauri setup 前に構築できる構成に分割する
  - setup 後に `app_data_dir` を受け取って `ModelStore` / `ModelOrchestrator` を注入する公開 API（例: `inject_model_stack`）を定義する
  - 単体テストで compose が `dirs::data_local_dir` を参照しないことを assert する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 5.3_
  - _Boundary: ReleaseComposeRoot_
  - _Design: D-ReleaseComposeRoot_
  - _Wave: 1_

- [x] 1.2 ADR-0008 と boundaries.md の境界記述更新
  - `ModelStore` の正本パスを Tauri `app_data_dir/models/` とする ADR-0008 を Accepted 状態で確定する
  - `docs/architecture/boundaries.md` に fix-release-transcribe の統合修正境界（ホスト composition / ACL / 停滞検知）を追記する
  - 設計の Persistent References と ADR 本文が整合していることを確認できる
  - _Requirements: 5.2, 5.3_
  - _Boundary: ReleaseComposeRoot_
  - _Design: D-ReleaseComposeRoot_
  - _Depends: 1.1_
  - _Wave: 2_

## 2. Core: app_data_dir 基準のモデルスタック（ReleaseComposeRoot）

- [x] 2.1 Tauri setup 内での ModelStore 注入とモデルロード開始
  - `lib.rs` setup 内で `app.path().app_data_dir()` を解決し、editor / release-logging と同一パスを `inject_model_stack` に渡す
  - 注入完了後に一度だけ `start_model_load_thread` を起動し、setup 前にモデルロードが走らないことを保証する
  - `ModelStore::model_path` が `{app_data_dir}/models/` を返す単体テストが pass する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 5.3_
  - _Boundary: ReleaseComposeRoot_
  - _Design: D-ReleaseComposeRoot, D-ModelStore_
  - _Depends: 1.1_
  - _Wave: 3_

- [x] 2.2 (P) 旧 Local パスからのモデル移行（存在時のみ）
  - 旧 `%LOCALAPPDATA%\gijirec\models\` にモデルが存在する場合、初回 setup で `app_data_dir/models/` へコピーする（存在時のみ、失敗時は再 DL フローへ委譲）
  - 移行成功後 `verify` が成功し transcribe phase が `ready` に遷移できる
  - 移行失敗時は `MODEL_NOT_FOUND` → 既存 DL フローで回復し、利用者向け error 通知が surface される
  - _Requirements: 2.1, 2.2, 2.3, 4.2, 5.3_
  - _Boundary: ModelStore_
  - _Design: D-ModelStore_
  - _Depends: 2.1_
  - _Wave: 4_

## 3. Core: Tauri イベント ACL（TranscribeAclGate）

- [x] 3.1 (P) block-appended listen 許可の追加
  - `permissions/allow-listen-transcribe-events.toml` に `whisper-transcribe://block-appended` を `[[permission.event.allow]]` として追加する
  - `capabilities/default.json` は既存 permission identifier 参照のまま変更不要であることを確認する
  - `event_permissions.rs` の CI gate が pass し、permission TOML に block-appended が含まれる
  - _Requirements: 1.1, 1.2, 3.1, 3.3, 5.1, 5.3_
  - _Boundary: TranscribeAclGate_
  - _Design: D-TranscribeAclGate_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Depends: 1.1_
  - _Wave: 5_

## 4. Core: 転写停滞ウォッチドッグ（TranscribeStallWatchdog）

- [x] 4.1 TranscribeStallWatchdog の実装
  - capture active + transcribing 中に 8 秒間ブロック未供給かつ入力あり条件（PCM RMS が VAD 閾値超過、または worker 推論試行 observability）を満たす場合に停滞を検知する
  - 純粋無音区間（VAD 無出力）は検知対象外とする
  - 発火時に `INFERENCE_FAILED` を `whisper-transcribe://error` で emit し、orchestrator 経由で phase を `error` に遷移する
  - 単体テストで閾値超過時の error emit と無音区間での非発火を検証する
  - _Requirements: 4.1, 4.3, 5.1_
  - _Boundary: TranscribeStallWatchdog_
  - _Design: D-TranscribeStallWatchdog_
  - _Contracts: docs/contracts/whisper-transcribe-status.md_
  - _Depends: 1.1_
  - _Wave: 6_

## 5. Integration: ライフサイクル連動と observability

- [x] 5.1 既存 TranscribeLifecycleHook へのウォッチドッグ連動
  - `gijirec-presentation/src/tauri/lifecycle.rs` を拡張し、capture transcribing 開始時にウォッチドッグを起動、停止時に停止する
  - `stall_watchdog.rs` を `transcribe/mod.rs` から export し、既存 lifecycle hook の責務分割を維持する
  - 既存 `transcribe_integration.rs` で deferred model inject 後も ready → transcribing 遷移が維持される
  - _Requirements: 1.1, 1.3, 4.3, 5.3_
  - _Boundary: TranscribeLifecycleHook_
  - _Design: D-TranscribeLifecycleHook, D-TranscribeStallWatchdog_
  - _Depends: 2.1, 4.1_
  - _Wave: 7_

- [x] 5.2 release ログ向け transcribe observability の整合
  - phase 遷移・error コード・停滞検知（`transcribe_stall_detected=true`）が `--log` 有効時に永続化ログへ記録される
  - 転写全文・PCM・デバイス表示名は release-logging 契約どおり記録しない
  - release ビルド + `--log` 起動後、モデルロード → ready → transcribing → block 配信の phase 系列がログから追跡できる
  - _Requirements: 4.4, 5.1_
  - _Boundary: TranscribeLifecycleHook_
  - _Contracts: docs/contracts/release-logging-persistence.md_
  - _Depends: 5.1_
  - _Wave: 8_

## 6. Validation: 単体・結合・release smoke

- [x] 6.1 ACL と compose 順序の回帰テスト
  - `event_permissions.rs` — block-appended が permission TOML に存在すること
  - setup 順序 mock — `app_data_dir` 注入前に model load が走らないこと
  - `TranscriptBlockBus` + RecordingEmitter — block publish が emit されること（既存テスト維持）
  - _Requirements: 1.1, 2.4, 3.1, 5.1, 5.3_
  - _Depends: 3.1, 5.1_
  - _Wave: 9_

- [x] 6.2 転写パイプライン結合テストの拡張
  - 既存 `transcribe_integration.rs` — inject 後 ready → transcribing → block 配信の一連フローが pass する
  - モデル取得失敗時に error phase + 利用者向け `message_ja` / `action_ja` が surface される
  - キャプチャ停止後 ready へ遷移し、既存ブロックが保持される
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.3, 3.1, 3.3, 4.1, 4.2, 5.1, 5.3_
  - _Depends: 5.1, 5.2_
  - _Wave: 10_

- [x]* 6.3 release EXE 手動 smoke チェックリスト
  - clean `gen` + `cargo tauri build` 後、EXE を `--log` 起動しモデル DL 完了 → ready をログと UI で確認する
  - キャプチャ開始 → 5 秒以内に `block-appended` でエディタ追記される
  - キャプチャ停止 → ready、既存ブロック保持。オフラインで転写継続を確認する
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 3.2, 3.3, 5.3_
  - _Depends: 6.2_
  - _Wave: 11_
