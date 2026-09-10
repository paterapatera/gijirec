# 実装計画: capture-audio-controls

## タスク一覧

- [x] 1. 基盤: ドメイン型・エラー契約・Tauri 権限の整備
- [x] 1.1 セッション音声制御のドメイン型とゲイン制約定数を定義する
  - `CaptureAudioControls`（`mic_ingest_enabled`、`manual_ingest_gain`、`gain_user_adjusted`）と `MIN_INGEST_GAIN`（0.25）・`MAX_INGEST_GAIN`（4.0）・`DEFAULT_INGEST_GAIN`（1.25）を domain 層に追加する
  - 範囲外・NaN・Inf のゲインはドメイン検証で拒否可能な形にし、契約 `capture-audio-controls.md` の型・制約表と一致させる
  - `cargo test` で定数値と不変条件のユニットテストが通る
  - _Requirements: 3.4, 3.6_
  - _Boundary: CaptureAudioControlsService_
  - _Contracts: docs/contracts/capture-audio-controls.md_
  - _Wave: 1_

- [x] 1.2 転写 ingest 音声源なしエラーコードを audio-capture 契約に接続する
  - `TRANSCRIBE_INGEST_NO_AUDIO_SOURCE` を domain / presentation のエラー列挙・日本語辞書（`message_ja` / `action_ja`）に追加し、既存 `audio-capture://error` イベント形状と整合させる
  - エラー payload が契約 `audio-capture-status.md` の表と一致し、既存エラー表示 UI がそのまま表示できる
  - _Requirements: 1.5_
  - _Boundary: CaptureAudioControlsService_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Wave: 1_

- [x] 1.3 Tauri capability 許可リストに音声制御コマンドを追加する
  - `allow-capture-audio-controls-commands.toml` を作成し、`get_capture_audio_controls` / `set_capture_audio_controls` を許可する
  - capability がアプリ manifest に組み込まれ、未許可 invoke が拒否される構成になる
  - _Requirements: 4.4_
  - _Wave: 1_

- [x] 2. (P) セッション音声制御ストアとサービスを実装する
- [x] 2.1 セッション内状態ストアと検証・適用ロジックを構築する
  - `CaptureAudioControlsStore`（`Arc<Mutex>`）でセッション中の 3 フィールドを保持し、デバイス再キャプチャ時もリセットしない
  - `CaptureAudioControlsService` が partial update を検証（ゲイン clamp 0.25–4.0、NaN/Inf → `INVALID_GAIN`）し、`manual_ingest_gain` 送信時は `gain_user_adjusted` を true にする（明示 false でリセット可）
  - 未調整セッションの既定 `manual_ingest_gain` は 1.25（`transcribe-volume-normalize` 等価）である
  - mic OFF 適用後に ingest 可能な音声源がない場合、`TRANSCRIBE_INGEST_NO_AUDIO_SOURCE` を発火する判定ロジックを持つ
  - 非 `capturing` 時も store は更新し `controls-changed` を emit するが、live ingest 適用は行わない（選択保持パターンと同型）
  - `cargo test` で gain clamp・`gain_user_adjusted` 遷移・既定 1.25・デバイス再開非リセットが検証される
  - _Requirements: 1.4, 1.5, 3.2, 3.4, 3.6, 4.2_
  - _Boundary: CaptureAudioControlsService_
  - _Contracts: docs/contracts/capture-audio-controls.md_
  - _Wave: 2_

- [x] 3. (P) キャプチャ processing 上のマイク ingest ゲートを実装する
- [x] 3.1 `mic_ingest_enabled` に応じてマイク PCM push をゲートする
  - `capture_processing` が `mic_ingest_enabled == false` のとき `push_mic` をスキップし、スピーカー／システム音声のみミキサーへ供給する
  - `mic_ingest_enabled == true` のとき既存二重キャプチャ・ミックス動作と同一の出力になる
  - ゲート状態は `Arc<AtomicBool>` 等で processing スレッドから読み取り、キャプチャ中のトグル変更が次チャンクから反映される
  - `cargo test` で mic ON/OFF 時のミキサー入力差分が検証される
  - _Requirements: 1.2, 1.3, 1.4_
  - _Boundary: CaptureProcessingGate_
  - _Wave: 2_

- [x] 4. (P) 転写 ingest consumer の動的ゲイン適用を実装する
- [x] 4.1 固定 ingest ゲインをセッション乗数に置換する
  - `PcmIngestConsumer` の固定 `TRANSCRIBE_INGEST_GAIN` を `set_ingest_gain_multiplier`（atomic、0.25–4.0）に置換し、ソフトリミット 0.95 を維持する
  - ゲイン変更は次 PCM チャンクから即時反映され、再起動を要求しない
  - chunk 処理後の RMS 計測を ingest ゲイン適用後信号に対して行い、下流メーター接続用コールバックを公開する
  - ユーザー未調整時の乗数 1.25 で既存転写音量と等価である
  - 既存 `transcribe-volume-normalize` 関連ユニットテストを更新し `cargo test` が通る
  - _Requirements: 2.1, 2.3, 3.2, 3.3, 3.6, 5.1_
  - _Boundary: PcmIngestConsumer_
  - _Wave: 2_

- [x] 5. ingest 直前 dBFS レベル emitter を実装する
- [x] 5.1 1 Hz dBFS メタデータイベントを集約・配信する
  - `IngestLevelEmitter` が `PcmIngestConsumer` の ingest 後 RMS を 1 秒窓で集約し、`20 * log10(rms)`（rms ≤ 0 は −120）で dBFS 変換する
  - キャプチャ `capturing` かつ ingest へ供給可能な間のみ `capture-audio-controls://ingest-level` を少なくとも 1 秒に 1 回 emit する
  - 非供給時は emit しない（誤解を招く固定レベルを送らない）
  - 生 PCM 配列をイベント・ログに含めない（dBFS メタデータのみ）
  - リソース圧迫時は emit 間隔を最大 2 秒まで延長し、転写 ingest 供給は継続する
  - `cargo test` で 1 秒窓集約・dBFS 変換・非 capturing 時スキップが検証される
  - _Depends: 4.1_
  - _Requirements: 2.1, 2.2, 2.4, 2.5, 5.1, 5.3_
  - _Boundary: IngestLevelEmitter_
  - _Contracts: docs/contracts/capture-audio-controls.md_
  - _Wave: 2_

- [x] 6. Tauri IPC コマンドとイベント emit を実装する
- [x] 6.1 音声制御 get/set コマンドとイベント配信を presentation 層に追加する
  - `get_capture_audio_controls` が `CaptureAudioControlsState`（`controls` + `ingest_level`）を返す
  - `set_capture_audio_controls` が partial update を `CaptureAudioControlsService` に委譲し、成功時 `capture-audio-controls://controls-changed` を emit する
  - `INVALID_GAIN` / `INTERNAL` の invoke エラー payload が契約表と一致する
  - `commands.rs` に `#[tauri::command]` を登録し、Step 1.3 の capability と接続する
  - presentation 層テストまたは統合スモークで get/set の往復が確認できる
  - _Depends: 2.1, 5.1_
  - _Requirements: 1.4, 1.5, 2.5, 3.2_
  - _Boundary: CaptureAudioControlsService_
  - _Contracts: docs/contracts/capture-audio-controls.md_
  - _Wave: 2_

- [x] 7. (P) フロントエンド IPC ラッパーと React フックを実装する
- [x] 7.1 契約型ミラーと invoke/listen フックを構築する
  - `capture-audio-controls-types.ts` が契約 `CaptureAudioControls` / `CaptureAudioControlsState` / イベント型をミラーする
  - `captureAudioControlsCommands.ts` が `get_capture_audio_controls` / `set_capture_audio_controls` をラップする
  - `useCaptureAudioControls` がマウント時 `get` 同期、`controls-changed` / `ingest-level` を購読し、phase に応じた disabled 状態を返す
  - injectable `invokeFn` / `listenFn` で Bun テストが Tauri なし実行可能である
  - _Requirements: 1.6, 2.4, 2.5, 3.5_
  - _Boundary: CaptureAudioControlsRow_
  - _Contracts: docs/contracts/capture-audio-controls.md_
  - _Wave: 2_

- [x] 8. キャプチャ音声制御 UI 行コンポーネントを実装する
- [x] 8.1 マイクトグル・dBFS メーター・ゲインスライダーを構築する
  - `CaptureAudioControlsRow` がマイク ON/OFF トグル、固定幅 dBFS メーター（例 `−18.2 dBFS` ラベル付き）、ゲインスライダー（0.25–4.0、step 0.05、中央 1.25）を提供する
  - `capturePhase !== 'capturing'` 時は全コントロール `disabled`、メーターは「—」または非活性表示とする
  - `ingest_level` が null のとき誤解を招く固定レベルを表示しない
  - ゲイン上下限到達時に `aria-live="polite"` で日本語ヒントを表示する
  - メーター固定幅により転写エディタ領域の目に見えるレイアウトシフトを発生させない
  - Bun テストで disabled 状態・dBFS ラベル・invoke 呼び出しが検証される
  - _Depends: 7.1_
  - _Requirements: 1.1, 1.6, 2.1, 2.3, 2.4, 3.1, 3.4, 3.5, 4.5, 5.2_
  - _Boundary: CaptureAudioControlsRow_
  - _Wave: 2_

- [x] 9. 統合: compose 結線とデバイス選択パネルへの組み込み
- [x] 9.1 バックエンド composition root で音声制御を結線する
  - `compose.rs` が `CaptureAudioControlsService` を `CaptureProcessingGate`・`PcmIngestConsumer`・`IngestLevelEmitter` に注入し、`capturing` 時の live apply が一連で動作する
  - サービス apply 時に mic ゲート・ingest ゲイン・ingest 源チェックが連動する
  - `docs/architecture/boundaries.md` に capture-audio-controls 境界節を追記する
  - compose 統合テスト（`cargo test -p gijirec -- compose::`）が既存とともに通る
  - _Depends: 2.1, 3.1, 4.1, 5.1, 6.1_
  - _Requirements: 1.2, 1.3, 1.4, 1.5, 3.2, 3.3, 4.2_
  - _Boundary: CaptureAudioControlsService, CaptureProcessingGate, PcmIngestConsumer, IngestLevelEmitter_
  - _Wave: 3_

- [x] 9.2 デバイス選択パネルに音声制御行を横並び統合する
  - `DeviceSelectorPanel` の既存 `<section>` 内に `CaptureAudioControlsRow` を配置し、デバイス選択項目と横並び（flex + wrap）にする
  - 既存 `CaptureErrorDisplay` / エラーパネル表示を維持し、キャプチャエラー時も制御 UI が矛盾しない
  - デバイス再選択・キャプチャ再開後もマイク ON/OFF と手動ゲインが保持される
  - 仮想オーディオデバイスや OS ミキサー変更を要求する UI・文言を追加しない
  - 実機またはスモークでキャプチャ設定領域の縦占有が最小化されていることを確認できる
  - _Depends: 8.1_
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Boundary: DeviceSelectorPanel_
  - _Wave: 3_

- [x] 10. 検証: 統合テスト・回帰・性能確認
- [x] 10.1 バックエンド統合テストで音声制御フローを検証する
  - `set_capture_audio_controls` → `PcmIngestConsumer` ゲイン反映の統合テストが通る
  - mic OFF + system 無効 → `TRANSCRIBE_INGEST_NO_AUDIO_SOURCE` 発火テストが通る
  - デバイス再選択後も controls 保持テストが通る
  - 非 `capturing` 時の store 更新が次回 `capturing` 開始時に ingest へ反映されるテストが通る
  - ingest-level 1 Hz が rtrb overflow を増加させない既存統合テスト拡張が通る
  - _Depends: 9.1, 9.2_
  - _Requirements: 1.2, 1.3, 1.4, 1.5, 1.6, 2.2, 3.2, 3.3, 3.6, 4.2, 5.1, 5.3_
  - _Wave: 4_

- [x] 10.2 フロントエンドと品質ゲートで回帰を確認する
  - `bun run verify` がエラーなく完了する
  - capturing 中のトグル・スライダー操作で invoke が呼ばれ、非 capturing 時は disabled であることをテストまたは手動スモークで確認できる
  - _Depends: 9.2_
  - _Requirements: 1.1, 1.6, 2.3, 3.1, 3.5, 4.1, 4.3, 4.4, 5.2_
  - _Wave: 4_

- [ ]* 10.3 オプション: UI スナップショットと E2E スモークを追加する
  - `CaptureAudioControlsRow` の capturing / non-capturing 表示差分のスナップショットまたは E2E スモークを追加する（MVP 後 defer 可）
  - テストが dBFS ラベル表示とメーター非活性表示を要件 2.3 / 2.4 に照合する
  - _Depends: 10.2_
  - _Requirements: 2.3, 2.4, 4.5, 5.2_
  - _Wave: 4_
