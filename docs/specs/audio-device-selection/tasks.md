# 実装計画: audio-device-selection

## 概要

既存 audio-capture を拡張し、cpal 列挙・セッション内デバイス選択・選択デバイスでの二重キャプチャ再開・Tauri IPC・選択 UI を段階的に実装する。Foundation → Core（境界別並列）→ Integration → Validation の順で進める。

---

- [x] 1. Foundation: ドメイン型と契約拡張
- [x] 1.1 AudioDeviceId / AudioDeviceInfo / DeviceSelection ドメイン型
  - `gijirec-domain` に cpal デバイス ID 文字列、`kind`（input/output）、`is_default` を持つ型と `DeviceSelection`（`Option` = OS 既定）を定義する
  - 完了時: 型のユニットテストで空 ID 拒否・`None` が既定解決可能であることが検証される
  - _Requirements: 1.3, 2.5, 2.6_
  - _Boundary: DeviceTypes_
  - _Design: D-DeviceTypes_
  - _Wave: 1_

- [x] 1.2 audio-capture-status 選択デバイス文脈エラーコード拡張
  - `SELECTED_MIC_UNAVAILABLE` / `SELECTED_SYSTEM_AUDIO_UNAVAILABLE` / `MACOS_OUTPUT_NOT_DEFAULT`（capture イベント側）を契約どおり `CaptureError` マッピングに追加する
  - 各コードに `message_ja` / `action_ja` / `recoverable` を付与する
  - 完了時: 内部エラーから契約ペイロードへの変換テストが全新規コードをカバーする
  - _Requirements: 4.1, 4.2, 4.4, 6.1_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: Errors_
  - _Depends: 1.1_
  - _Wave: 2_

- [x] 1.3 architecture boundaries ドキュメント更新
  - `docs/architecture/boundaries.md` に audio-device-selection の Owns / Out / Allowed Dependencies を設計どおり追記する
  - 完了時: boundaries.md の記載が design.md Boundary Commitments と一致する
  - _Requirements: 1.1_
  - _Depends: 1.1_
  - _Wave: 3_

- [x] 2. インフラストラクチャ層: デバイス列挙とアダプタ拡張
- [x] 2.1 (P) cpal 入出力デバイス列挙
  - `AudioDeviceEnumerator` で入力・出力デバイスを列挙し `Device::name()` をセッション `AudioDeviceId` および表示名として返す（cpal 0.16 は `Device::id()` 非公開）
  - 各 kind の OS 既定デバイスに `is_default: true` を付与する
  - 完了時: モック host で inputs/outputs が返り、既定フラグが 1 件ずつ立つユニットテストが通る
  - _Requirements: 1.1, 1.2, 1.3, 6.1, 6.2_
  - _Boundary: AudioDeviceEnumerator_
  - _Design: D-AudioDeviceEnumerator_
  - _Depends: 1.1_
  - _Wave: 4_

- [x] 2.2 (P) マイクキャプチャアダプタのデバイス ID 指定オープン
  - `MicCaptureAdapter` を拡張し、指定 `AudioDeviceId` で入力ストリームをオープンできるようにする（`None` は既定デバイス）
  - 存在しない ID・オープン失敗を `CaptureError` に変換する
  - 完了時: 有効 ID でストリーム開始、無効 ID で `SELECTED_MIC_UNAVAILABLE` 相当エラーが返る
  - _Requirements: 3.1, 3.4, 4.1, 6.1, 6.2, 7.1_
  - _Boundary: MicCaptureAdapter_
  - _Design: D-MicCaptureAdapter_
  - _Depends: 1.1, 1.2_
  - _Wave: 5_

- [x] 2.3 (P) Windows ループバックアダプタの出力デバイス ID 指定
  - `WindowsLoopbackAdapter` を拡張し、指定出力デバイス ID で WASAPI ループバックをオープンする（`None` は既定出力）
  - `cfg(target_os = "windows")` のみ。オープン失敗時は `SELECTED_SYSTEM_AUDIO_UNAVAILABLE` を返す
  - 完了時: Windows 環境で非既定出力デバイスへのループバック開始が可能（実機 `#[ignore]` テスト含む）
  - _Requirements: 3.1, 3.4, 4.2, 6.2_
  - _Boundary: WindowsLoopbackAdapter_
  - _Design: D-WindowsLoopbackAdapter_
  - _Depends: 1.1, 1.2_
  - _Wave: 6_

- [x] 3. アプリケーション層: 選択状態とサービス
- [x] 3.1 (P) セッション内 DeviceSelectionStore
  - `microphone_id` / `speaker_id` の `Option<AudioDeviceId>` を保持するスレッドセーフストアを実装する（永続化なし）
  - `get_selection()` / `update(selection)` を提供し、`None` は OS 既定として下流へ渡す
  - 完了時: `None` 選択が既定解決経路に渡されるユニットテストが通る
  - _Requirements: 2.2, 2.3, 2.5, 2.6_
  - _Boundary: DeviceSelectionStore_
  - _Design: D-DeviceSelectionStore_
  - _Depends: 1.1_
  - _Wave: 7_

- [x] 3.2 DeviceSelectionService（一覧・選択・ホットプラグ監視）
  - `list_devices()` で Enumerator を呼び出し空配列を許容する
  - `set_selection()` で ID 存在検証、macOS 非既定スピーカー preflight（`MACOS_OUTPUT_NOT_DEFAULT`）、同一選択 no-op、再開中の直列化（最新選択のみ再開）を実装する
  - `set_ui_visible(true/false)` で UI 可視時のみ ≥ 2 s 間隔のホットプラグ監視を行い `devices-changed` を emit する
  - `error` フェーズからの再選択で `restart_with_selection` を呼ぶ回復経路を含める
  - 完了時: 存在しない ID・macOS preflight・idempotent・直列化・error 回復のユニットテストが設計 Testing Strategy 1–6 をカバーする
  - _Requirements: 1.4, 1.5, 2.2, 2.3, 3.3, 4.5, 5.1, 6.1, 7.3_
  - _Boundary: DeviceSelectionService_
  - _Design: D-DeviceSelectionService_
  - _Depends: 2.1, 3.1, 4.1_
  - _Wave: 8_

- [x] 4. アプリケーション層: CaptureOrchestrator 拡張
- [x] 4.1 選択デバイスでの開始・再開オーケストレーション
  - `start_with_selection(sel)` / `restart_with_selection(sel)` を実装し、`None` を OS 既定に解決してマイク→システム音声の順でオープンする
  - `restart_with_selection`: stopping → 解放 → starting → 新デバイスでオープン。目標 < 2 s
  - スピーカー失敗時にマイク単独継続しない（サイレントフォールバック禁止）。切断時は `DEVICE_DISCONNECTED` で安全停止
  - macOS スピーカーは SCK システムミックス + OS 既定出力 preflight（ADR-0009）
  - 完了時: スピーカー失敗でマイク単独継続しないテスト、`error` フェーズから `capturing` 復帰テストが通る
  - _Requirements: 2.6, 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 5.2, 5.3, 6.1, 6.2, 7.1, 7.2, 7.5_
  - _Boundary: CaptureOrchestrator_
  - _Design: D-CaptureOrchestrator_
  - _Depends: 2.2, 2.3, 3.1_
  - _Wave: 9_

- [x] 5. プレゼンテーション層（Rust）: Tauri IPC
- [x] 5.1 デバイス選択 Tauri commands とイベント
  - `list_audio_devices` / `get_device_selection` / `set_device_selection` を実装し、契約どおりのペイロードとエラー（`INVALID_DEVICE`, `MACOS_OUTPUT_NOT_DEFAULT`, `INTERNAL`）を返す
  - `audio-device-selection://devices-changed` と `audio-device-selection://selection-changed` を emit する
  - 完了時: invoke 統合テストで一覧取得・選択反映・selection-changed 発火が確認される
  - _Requirements: 1.1, 1.2, 1.4, 2.2, 2.3, 2.4, 7.3_
  - _Contracts: docs/contracts/audio-device-selection.md_
  - _Boundary: DeviceSelectionCommands_
  - _Design: D-DeviceSelectionCommands_
  - _Depends: 3.2_
  - _Wave: 10_

- [x] 6. プレゼンテーション層（TypeScript）: デバイス選択 UI
- [x] 6.1 (P) 契約型ミラーと invoke ラッパ
  - `audio-device-types.ts` に契約型（`AudioDeviceInfo`, `DeviceSelection` 等）を定義する
  - `audioDeviceCommands.ts` に `list_audio_devices` / `get_device_selection` / `set_device_selection` の invoke ラッパを実装する
  - 完了時: TypeScript strict で型チェックが通り、invoke ラッパが契約フィールド名と一致する
  - _Requirements: 1.1, 1.2, 2.2, 2.3_
  - _Contracts: docs/contracts/audio-device-selection.md_
  - _Boundary: AudioDeviceTypes_
  - _Depends: 5.1_
  - _Wave: 11_

- [x] 6.2 (P) useAudioDevices フック
  - 一覧取得・現在選択・`devices-changed` / `selection-changed` イベント購読を行う React フックを実装する
  - マウント時 `set_ui_visible(true)`、アンマウント時 `false` を backend へ通知する
  - 完了時: バックエンドから selection-changed を emit するとフック state が同期更新される
  - _Requirements: 1.4, 2.4, 2.5, 5.1_
  - _Contracts: docs/contracts/audio-device-selection.md_
  - _Boundary: useAudioDevices_
  - _Design: D-useAudioDevices_
  - _Depends: 5.1, 6.1_
  - _Wave: 12_

- [x] 6.3 DeviceSelectorPanel 選択 UI
  - マイク・スピーカーの Select UI、現在値表示、候補ゼロ empty state、macOS スピーカー向けヘルプテキストを実装する
  - エラー表示時も UI を操作可能に維持し `action_ja` を表示する
  - 完了時: 起動直後に OS 既定が現在値表示され、候補ゼロ時 empty state、macOS 非既定選択時案内が表示される
  - _Requirements: 1.5, 2.1, 2.4, 2.5, 4.4, 4.5, 6.1_
  - _Boundary: DeviceSelectorPanel_
  - _Design: D-DeviceSelectorPanel_
  - _Depends: 6.2_
  - _Wave: 13_

- [x] 7. 統合: 結線とライフサイクル
- [x] 7.1 composition root への DeviceSelection 結線
  - `compose.rs` に `DeviceSelectionService`、Enumerator、Store、Commands を依存注入し Tauri command を登録する
  - `CaptureOrchestrator` 拡張と既存 audio-capture パイプラインを接続する
  - 完了時: `cargo build` が成功し、composition root から list/set_device_selection が invoke 可能
  - _Requirements: 1.1, 3.1, 6.1, 6.2_
  - _Depends: 3.2, 4.1, 5.1_
  - _Wave: 14_

- [x] 7.2 起動ライフサイクルと既定選択キャプチャ
  - `TauriLifecycleHook` を更新し、起動時 `get_selection()`（未操作 = 全 `None`）で `start_with_selection` を呼ぶ
  - UI 未操作時は OS 既定デバイス + 起動時自動キャプチャを維持する
  - Linux ターゲットでは cfg ガードで非対応のまま（6.3）
  - 完了時: アプリ起動で OS 既定デバイスの capturing が開始し、選択 UI 未操作でも既存ライフサイクルが維持される
  - _Requirements: 2.6, 3.1, 6.3, 7.1_
  - _Boundary: TauriLifecycleHook_
  - _Design: D-TauriLifecycleHook_
  - _Depends: 4.1, 7.1_
  - _Wave: 15_

- [x] 7.3 App.tsx への DeviceSelectorPanel 配置
  - 既存キャプチャステータス UI 近傍に `DeviceSelectorPanel` を配置し、利用者が見つけられる位置にする
  - 完了時: アプリ起動後、メイン画面からマイク・スピーカー選択 UI に到達できる
  - _Requirements: 2.1_
  - _Depends: 6.3, 7.1_
  - _Wave: 16_

- [x] 7.4 可観測性（選択変更・再開時間）
  - 選択変更（デバイス ID のみ INFO）、再キャプチャ開始/完了、`device_selection_restart_duration_ms` を tracing で記録する
  - デバイス名は DEBUG 限定、音声 PCM はログ禁止を維持する
  - 完了時: 選択変更セッションで ID のみ INFO ログが出力され、PCM 生データがログに含まれない
  - _Requirements: 5.3, 7.2, 7.3_
  - _Depends: 3.2, 4.1_
  - _Wave: 17_

- [x] 8. 検証: ユニットテスト
- [x] 8.1* DeviceSelectionService 検証・直列化・idempotent テスト
  - 存在しない ID、`MACOS_OUTPUT_NOT_DEFAULT`、同一選択 no-op、再開中直列化のテストを追加する
  - 完了時: 設計 Testing Strategy Unit Tests 1–4, 6 が自動テストでカバーされる
  - _Requirements: 2.2, 4.5, 6.1_
  - _Boundary: DeviceSelectionService_
  - _Depends: 3.2_
  - _Wave: 18_

- [x] 8.2* CaptureOrchestrator 選択再開・フォールバック禁止テスト
  - スピーカー失敗時マイク単独継続禁止、`error` フェーズからの復帰テストを追加する
  - 完了時: 設計 Testing Strategy Unit Tests 5–6 が自動テストでカバーされる
  - _Requirements: 3.4, 4.2, 4.5_
  - _Boundary: CaptureOrchestrator_
  - _Depends: 4.1_
  - _Wave: 19_

- [x] 8.3* AudioDeviceEnumerator 既定フラグテスト
  - モック host で `is_default` フラグと表示名が正しく返ることを検証する
  - 完了時: 設計 Testing Strategy Unit Test 7 が自動テストでカバーされる
  - _Requirements: 1.3, 2.5_
  - _Boundary: AudioDeviceEnumerator_
  - _Depends: 2.1_
  - _Wave: 20_

- [x] 8.4* DeviceSelectionStore 既定解決テスト
  - `None` 選択が OS 既定解決に渡されることを検証する
  - 完了時: 設計 Testing Strategy Unit Test 8 が自動テストでカバーされる
  - _Requirements: 2.5, 2.6_
  - _Boundary: DeviceSelectionStore_
  - _Depends: 3.1_
  - _Wave: 21_

- [x] 9. 検証: 統合・E2E・性能
- [x] 9.1 set_device_selection → capturing 復帰統合テスト
  - 選択変更後にフェーズ `capturing` に復帰し、選択マイク不存在で `SELECTED_MIC_UNAVAILABLE` が emit されることを検証する
  - 完了時: 設計 Testing Strategy Integration Tests 1–2 が自動テストでカバーされる
  - _Requirements: 3.3, 4.1_
  - _Depends: 7.1, 7.2_
  - _Wave: 22_

- [x] 9.2 UI 可視時のみ devices-changed 統合テスト
  - `set_ui_visible(true)` 時のみホットプラグイベントが発行されることを検証する
  - 完了時: 設計 Testing Strategy Integration Test 3 が自動テストでカバーされる
  - _Requirements: 1.4, 5.1_
  - _Depends: 3.2, 5.1_
  - _Wave: 23_

- [x] 9.3 再キャプチャ後 PcmChunk sequence 単調増加テスト
  - 選択変更後も `PcmChunk.sequence` が単調増加継続することを検証する
  - 完了時: 設計 Testing Strategy Integration Test 4 が自動テストでカバーされる
  - _Requirements: 3.2_
  - _Contracts: docs/contracts/audio-capture-pcm.md_
  - _Depends: 4.1, 7.1_
  - _Wave: 24_

- [x] 9.4* E2E/UI: 既定表示・選択変更・empty state・action_ja
  - 起動直後の既定表示、マイク変更後 capturing 継続、候補ゼロ empty state、エラーに action_ja 含有、macOS 非既定スピーカー案内を UI テストで検証する
  - 完了時: 設計 Testing Strategy E2E/UI Tests 1–5 がカバーされる（macOS 項目は該当 OS のみ）
  - _Requirements: 1.5, 2.5, 3.1, 3.3, 4.4, 6.1_
  - _Depends: 6.3, 7.2, 7.3_
  - _Wave: 25_

- [x] 9.5* 性能: 選択変更 → capturing 復帰 < 2 s
  - 選択変更から `capturing` 復帰までの時間を計測し 2 s 目標を検証する（実機ばらつきは手動確認を残す）
  - 完了時: 計測結果が `performance-results.md` または tracing ログに記録される
  - _Requirements: 5.3_
  - _Depends: 4.1, 7.4_
  - _Wave: 26_

## Implementation Notes

- cpal 0.16 は `Device::id()` 非公開。セッション `AudioDeviceId` は `Device::name()`（契約・設計に記載済み）。
- Wave 8（3.2 DeviceSelectionService）は Wave 9（4.1）に依存するため、Wave 番号順より Depends を優先した。
- ポートの `open_with_selection` はデフォルトで `open()` 委譲。実デバイス ID 結線は 7.1。
- 再キャプチャ時は `ChunkEmitter` を再生成せず `discard_partial_buffer` のみ行い sequence を継続する。
