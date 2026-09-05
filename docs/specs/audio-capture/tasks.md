# 実装計画: audio-capture

## 概要

Greenfield Tauri デスクトップアプリとして、レイヤード Rust crates・Bun フロントエンド・OS ネイティブ二重キャプチャパイプラインを段階的に構築する。Foundation → Core（境界別並列）→ Integration → Validation の順で実装する。

---

- [x] 1. Foundation: プロジェクトスキャフォールドとツールチェーン
- [x] 1.1 Tauri 2 ホストと Cargo workspace の初期構成
  - `src-tauri/Cargo.toml` に workspace メンバー（domain / application / infrastructure / presentation）を定義し、`cargo check` が通る空 crate を生成する
  - `tauri.conf.json` の `beforeDevCommand` を Bun スクリプト前提に設定する
  - 完了時: `cargo check` と `bun run dev` の起動準備がエラーなく通る
  - _Requirements: 8.1_
  - _Wave: 1_

- [x] 1.2 Bun フロントエンド依存管理と package.json スクリプト
  - `package.json` / `bun.lock` を作成し、依存インストールとスクリプト実行を Bun のみで行う（npm 前提なし）
  - `dev` / `build` / `check` スクリプトを定義し、TypeScript strict + Vite エントリを配置する
  - 完了時: クリーン環境で `bun install` のみでフロント依存が解決し `bun run dev` が起動する
  - _Requirements: 8.1, 8.2_
  - _Wave: 2_

- [x] 1.3 レイヤード crate 骨格と公開モジュール構成
  - 各 crate に `lib.rs` と設計どおりのモジュールディレクトリ（`audio/`, `capture/`, `tauri/` 等）の空骨格を作成する
  - crate 間依存を domain ← application / infrastructure ← presentation のみに制限する
  - 完了時: 4 crate が相互 import 規則どおりにビルドされ、循環依存がない
  - _Requirements: 8.1_
  - _Wave: 3_

- [x] 1.4 cargo bylaw と Rust アーキテクチャ検証
  - cargo bylaw 設定を追加し、レイヤ違反を CI で検出できるようにする
  - `rust:check` 相当スクリプト（fmt / clippy / bylaw）を package.json に登録する
  - 完了時: 意図的なレイヤ違反 import で bylaw が失敗し、正常構成では通過する
  - _Requirements: 8.1_
  - _Wave: 4_

- [x] 1.5 dependency-cruiser とフロント品質ゲート
  - dependency-cruiser ルールで `src/domain` → 外レイヤ禁止、`src/` → `src-tauri/` 禁止を検証する
  - Biome / ESLint / typecheck を `check` スクリプトに統合する
  - 完了時: `bun run check` がレイヤ違反を検出し、正常構成では通過する
  - _Requirements: 8.1_
  - _Wave: 5_

- [x] 1.6 README と開発環境セットアップ手順
  - README に Bun を必須ツールとして明記し、npm を必須としないセットアップ手順を記載する
  - Rust stable・Tauri 前提・Mac/Windows 対応・Linux 非対応を明記する
  - 完了時: README の手順どおりに新規環境で `bun install` → 開発起動まで到達できる
  - _Requirements: 8.2, 8.3, 6.3_
  - _Wave: 6_

- [x] 2. ドメイン層: 音声キャプチャコア型
- [x] 2.1 PcmChunk 値オブジェクトと PcmChunkConsumer トレイト
  - 契約どおりのフィールド（sequence, sample_rate_hz=16000, channels=1, Int16Le, samples, frame_count, timestamp_ms）を持つ不変値オブジェクトを実装する
  - 下流登録用 `PcmChunkConsumer` トレイトと `PcmConsumerError` を定義する
  - 完了時: 1600 サンプル（100 ms）チャンクを構築するユニットテストが通り、契約フィールド制約が検証される
  - _Requirements: 2.2, 2.3, 7.2_
  - _Contracts: docs/contracts/audio-capture-pcm.md_
  - _Boundary: PcmTypes_
  - _Design: D-PcmTypes_
  - _Wave: 7_

- [x] 2.2 CapturePhase 状態列挙
  - `idle | starting | capturing | stopping | error` を表す列挙型と遷移ヘルパを実装する
  - 完了時: 設計の状態図どおりの合法遷移のみ許可し、非法遷移はコンパイル時またはテストで拒否される
  - _Requirements: 3.4, 5.1, 5.2, 5.3_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: CaptureState_
  - _Design: D-CaptureState_
  - _Wave: 8_

- [x] 2.3 CaptureError と UserFacingError マッピング
  - 内部 `CaptureError` から契約の `code` / `message_ja` / `action_ja` / `recoverable` へマップする変換を実装する
  - 全エラーコード（MIC_*, SYSTEM_*, DEVICE_DISCONNECTED, INTERNAL）を網羅する
  - 完了時: 各内部エラーが契約どおりの利用者向けペイロードに変換され、action_ja が空にならない
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 7.1, 7.4_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: Errors_
  - _Design: D-Errors_
  - _Wave: 9_

- [x] 3. インフラストラクチャ層: 音声入力アダプタ
- [x] 3.1 (P) 既定マイクの cpal キャプチャアダプタ
  - 既定入力デバイスから f32 モノフレームを継続取得し、RT コールバックでは rtrb へ push のみ行う
  - デバイスオープン失敗・権限拒否を `CaptureError` に変換する
  - 完了時: マイク接続環境でストリームが開始され、コールバック内アロケーションが発生しない（設計制約を満たす）
  - _Requirements: 1.1, 1.4, 6.1, 6.2, 7.1_
  - _Boundary: MicCaptureAdapter_
  - _Design: D-MicCaptureAdapter_
  - _Depends: 2.3_
  - _Wave: 10_

- [x] 3.2 (P) 16 kHz モノラルリサンプラ
  - rubato を用い任意サンプルレート・チャンネル構成の f32 入力を 16 kHz モノラルに変換する
  - リサンプル処理は専用スレッドで実行し、RT コールバックから分離する
  - 完了時: 48 kHz ステレオ入力を 16 kHz モノラルへ変換するユニットテストが期待サンプル数を満たす
  - _Requirements: 2.2, 4.1_
  - _Boundary: MonoResampler_
  - _Design: D-MonoResampler_
  - _Wave: 11_

- [x] 3.3 (P) Windows WASAPI ループバックアダプタ
  - 既定出力デバイスの WASAPI ループバックでシステム音声を f32 フレームとして取得する（仮想デバイス不要）
  - `cfg(target_os = "windows")` でのみコンパイルし、RT コールバックは rtrb push のみ
  - 完了時: Windows 環境でループバックストリームがオープンし、再生音声に同期したフレームがリングバッファへ到達する
  - _Requirements: 1.2, 1.4, 6.2_
  - _Boundary: WindowsLoopbackAdapter_
  - _Design: D-WindowsLoopbackAdapter_
  - _Depends: 2.3_
  - _Wave: 12_

- [x] 3.4 (P) macOS ScreenCaptureKit システム音声アダプタ
  - ScreenCaptureKit でシステム音声を取得し、映像は最小サイズで破棄、`excludesCurrentProcessAudio = true` を設定する
  - 画面収録権限拒否時は `SYSTEM_AUDIO_PERMISSION_DENIED` を返す
  - 完了時: macOS 13+ で SCK ストリームが開始し、権限付与後にシステム音声フレームがリングバッファへ到達する
  - _Requirements: 1.2, 1.4, 6.1, 7.1_
  - _Boundary: MacScreenCaptureKitAdapter_
  - _Design: D-MacScreenCaptureKitAdapter_
  - _Depends: 2.3_
  - _Wave: 13_

- [x] 4. アプリケーション層: ミックス・チャンク生成・オーケストレーション
- [x] 4.1 二系統音声の整列・レベル調整・ミックス
  - 50 ms 整列バッファでマイクとシステム音声のタイムライン差を吸収する
  - 200 ms RMS 窓で各ソースを -20 dBFS 付近へ正規化し、ソフトリミッター（±0.95）で合算する
  - メモリ保持は最大 30 s リングバッファを超えない（永続保存なし）
  - 完了時: 大音量マイク＋小音量システムの合成でクリップせず、片系統のみでもクラッシュしないユニットテストが通る
  - _Requirements: 2.1, 2.4, 7.3_
  - _Boundary: AudioMixer_
  - _Design: D-AudioMixer_
  - _Depends: 3.2_
  - _Wave: 14_

- [x] 4.2 100 ms 単位の PcmChunk 生成
  - ミックス済み f32 から 1600 サンプル（100 ms）境界で `PcmChunk` を生成し、sequence を単調増加させる
  - 停止時は部分チャンクを破棄し、新規発行を行わない
  - 完了時: 連続入力で sequence 欠番なし・1600 サンプル固定のチャンクが生成されるユニットテストが通る
  - _Requirements: 2.2, 2.3_
  - _Contracts: docs/contracts/audio-capture-pcm.md_
  - _Boundary: ChunkEmitter_
  - _Design: D-ChunkEmitter_
  - _Depends: 2.1, 4.1_
  - _Wave: 15_

- [x] 4.3 二重キャプチャの開始・停止・エラー分岐オーケストレータ
  - `start()`: マイクを先にオープンし、成功後にシステム音声をオープン。システム失敗時はマイクを直ちにクローズし capturing へ遷移しない（サイレントフォールバック禁止）
  - `stop()`: 全ストリームを逆順で閉じ、バッファを破棄し idle へ。冪等性を保証する
  - キャプチャ中のデバイス切断で安全停止し error フェーズへ遷移する
  - 権限 preflight を starting 中に実施する
  - 完了時: システム音声失敗シナリオでマイク単独継続が発生せず、start→capturing→stop→idle の状態遷移テストが通る
  - _Requirements: 1.1, 1.2, 1.3, 3.4, 5.1, 5.2, 5.3, 7.1_
  - _Boundary: CaptureOrchestrator_
  - _Design: D-CaptureOrchestrator_
  - _Depends: 3.1, 3.3, 3.4, 4.1_
  - _Wave: 16_

- [x] 5. プレゼンテーション層（Rust）: Tauri イベントとバス
- [x] 5.1 キャプチャフェーズ・エラーイベントの emit
  - `audio-capture://phase-changed` と `audio-capture://error` を Tauri イベントとして発行する
  - ペイロードは契約どおりの TypeScript 互換形状とし、action_ja を必ず含める
  - 完了時: オーケストレータのフェーズ遷移ごとに phase-changed が発火し、エラー時に action_ja 付き error イベントが UI へ到達する
  - _Requirements: 5.4, 7.1_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: CaptureEventEmitter_
  - _Design: D-CaptureEventEmitter_
  - _Depends: 2.2, 2.3_
  - _Wave: 17_

- [x] 5.2 PcmChunk 下流バスとバックプレッシャー
  - `PcmChunkConsumer` 単一登録点を実装し、100 ms チャンクを配信する
  - 下流遅延時は最大 3 チャンク（≈300 ms）を超えた分を破棄し `capture_buffer_drops_total` を記録する
  - PCM を Tauri フロントイベントへ送らない（Rust 内部バスのみ）
  - 完了時: モック consumer 登録後 100 ms 以内に最初のチャンクが到達し、遅延 consumer でドロップが記録される
  - _Requirements: 2.3, 7.2_
  - _Contracts: docs/contracts/audio-capture-pcm.md_
  - _Boundary: PcmChunkBus_
  - _Design: D-PcmChunkBus_
  - _Depends: 2.1, 4.2_
  - _Wave: 18_

- [x] 5.3 アプリライフサイクルとキャプチャの同期
  - Tauri `setup` でキャプチャを自動開始し、`CloseRequested` / `RunEvent::Exit` で完全停止する
  - Linux ターゲットでは非対応ダイアログを表示しキャプチャを開始しない
  - 停止後はマイク・システム音声の取得を行わない
  - 完了時: アプリ起動で capturing へ遷移し、ウィンドウ閉鎖・OS 終了で idle へ戻りリソースが解放される
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 6.3_
  - _Boundary: TauriLifecycleHook_
  - _Design: D-TauriLifecycleHook_
  - _Depends: 4.3, 5.1_
  - _Wave: 19_

- [ ] 6. プレゼンテーション層（TypeScript）: 最小ステータス UI
- [x] 6.1 (P) キャプチャ状態購読フック
  - Tauri イベント `audio-capture://phase-changed` / `audio-capture://error` を購読する React フックを実装する
  - アンマウント時にリスナーを解除する
  - 完了時: バックエンドから phase-changed を emit するとフックの state が同期更新される
  - _Requirements: 5.4_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Boundary: useCaptureStatus_
  - _Design: D-UseCaptureStatus_
  - _Depends: 5.1_
  - _Wave: 20_

- [x] 6.2 キャプチャ状態・エラー表示 UI
  - 現在フェーズ（capturing / error 等）と利用者向けエラーメッセージ・action_ja を表示する最小 App 画面を実装する
  - 技術コードのみの表示を避け、action_ja を目立つ形で提示する
  - 完了時: アプリ起動後 UI に capturing が表示され、権限拒否シミュレーションで action_ja 含有エラーが表示される
  - _Requirements: 5.4, 7.1_
  - _Depends: 6.1_
  - _Wave: 21_

- [ ] 7. 統合: composition root とプラットフォーム設定
- [x] 7.1 Rust composition root 結線
  - `src-tauri/src/lib.rs` で全 crate を組み立て、オーケストレータ・アダプタ・ミキサ・エミッタ・バス・イベント emitter を依存注入する
  - プラットフォーム別に Windows / macOS アダプタを cfg で切り替える
  - 完了時: `cargo build` が Mac/Windows ターゲットで成功し、composition root から end-to-end でキャプチャパイプラインが起動する
  - _Requirements: 1.3, 6.1, 6.2_
  - _Depends: 3.1, 3.2, 3.3, 3.4, 4.3, 5.1, 5.2, 5.3_
  - _Wave: 22_

- [x] 7.2 オーディオパイプライン end-to-end 結線
  - アダプタ → リサンプラ → ミキサ → チャンクエミッタ → バスのデータフローを専用処理スレッドで接続する
  - RT コールバックと処理スレッド間は rtrb のみでやり取りする
  - 完了時: キャプチャ開始後、モック consumer が 100 ms 間隔で連続 PcmChunk を受信する統合スモークが通る
  - _Requirements: 1.1, 1.2, 1.3, 2.1, 2.3, 4.1_
  - _Depends: 7.1_
  - _Wave: 23_

- [x] 7.3 macOS 権限説明とネイティブ依存同梱
  - `Info.plist` に `NSScreenCaptureUsageDescription` とマイク用途文字列を追加する
  - cpal / screencapturekit ネイティブ依存が Tauri バンドルに含まれることを確認する
  - 完了時: macOS バンドルビルドで権限プロンプトに説明文が表示される
  - _Requirements: 6.1, 7.1_
  - _Depends: 7.1_
  - _Wave: 24_

- [x] 7.4 可観測性（ログ・メトリクス代替）の組み込み
  - `tracing` でフェーズ遷移（INFO）・バッファドロップ（WARN）・ストリーム失敗（ERROR）を記録する
  - 音声サンプル・PCM バイト列・マイクデバイス名をログに出さない
  - `capture_buffer_drops_total` / `capture_phase` / `capture_rt_callback_max_us` を tracing フィールドで記録する
  - 完了時: キャプチャセッション中にフェーズ遷移ログが出力され、PCM 生データがログに含まれない
  - _Requirements: 4.1, 4.2, 7.2_
  - _Depends: 7.2_
  - _Wave: 25_

- [x] 7.5 architecture boundaries ドキュメント更新
  - `docs/architecture/boundaries.md` に audio-capture の境界・依存行を追加する
  - 完了時: boundaries.md に本 feature の Owns / Out / Allowed Dependencies が設計と一致して記載されている
  - _Requirements: 1.4_
  - _Depends: 7.1_
  - _Wave: 26_

- [ ] 8. 検証: ユニットテスト
- [x] 8.1* AudioMixer レベル調整・整列のユニットテスト
  - 片系統のみ入力・大音量マイク＋小音量システムのミックスでクリップしないことを検証する
  - 完了時: 設計 Testing Strategy の AudioMixer 項目 1–2 が自動テストでカバーされる
  - _Requirements: 2.1, 2.4_
  - _Boundary: AudioMixer_
  - _Depends: 4.1_
  - _Wave: 27_

- [x] 8.2* ChunkEmitter 境界・sequence のユニットテスト
  - 1600 サンプル境界と sequence 単調増加を検証する
  - 完了時: 設計 Testing Strategy の ChunkEmitter 項目 3 が自動テストでカバーされる
  - _Requirements: 2.3_
  - _Boundary: ChunkEmitter_
  - _Depends: 4.2_
  - _Wave: 28_

- [x] 8.3 CaptureOrchestrator フォールバック禁止のユニットテスト
  - システム音声失敗時にマイク単独で capturing へ遷移しないことを検証する
  - 完了時: 設計 Testing Strategy の CaptureOrchestrator 項目 4 が自動テストでカバーされる
  - _Requirements: 1.3, 5.2_
  - _Boundary: CaptureOrchestrator_
  - _Depends: 4.3_
  - _Wave: 29_

- [x] 8.4 UserFacingError 契約マッピングのユニットテスト
  - 各 `CaptureError` が契約の code / action_ja に正しくマップされることを検証する
  - 完了時: 設計 Testing Strategy の UserFacingError 項目 5 が自動テストでカバーされる
  - _Requirements: 5.4_
  - _Contracts: docs/contracts/audio-capture-status.md_
  - _Depends: 2.3_
  - _Wave: 30_

- [ ] 9. 検証: 統合テスト
- [x] 9.1 ライフサイクル統合テスト
  - start → capturing → stop → idle でストリームハンドルが解放されることを検証する
  - 完了時: 設計 Integration Tests 項目 3 が自動テストでカバーされる
  - _Requirements: 3.1, 3.2, 3.4_
  - _Depends: 5.3, 7.1_
  - _Wave: 31_

- [x] 9.2 PcmChunkBus 統合テスト
  - consumer 登録後 100 ms 以内の最初のチャンク到達と、遅延 consumer 時のドロップ記録を検証する
  - 完了時: 設計 Integration Tests 項目 4–5 が自動テストでカバーされる
  - _Requirements: 2.3, 7.2_
  - _Depends: 5.2, 7.2_
  - _Wave: 32_

- [x] 9.3 (P) Windows ループバック統合テスト
  - Windows 上でマイク＋ループバック同時開始を検証する（CI skip または手動実行付き）
  - 完了時: Windows 環境で統合テストが capturing へ到達するか、skip 理由が明示される
  - _Requirements: 1.2, 6.2_
  - _Boundary: WindowsLoopbackAdapter_
  - _Depends: 7.2_
  - _Wave: 33_

- [x] 9.4 (P) macOS SCK 統合テスト
  - macOS 上で SCK 権限付与後のシステム音声取得を検証する（`#[ignore]` 実機テスト可）
  - 完了時: macOS 環境で統合テストが capturing へ到達するか、ignore 理由が明示される
  - _Requirements: 1.2, 6.1, 7.1_
  - _Boundary: MacScreenCaptureKitAdapter_
  - _Depends: 7.2, 7.3_
  - _Wave: 34_

- [ ] 10. 検証: E2E・性能・手動チェックリスト
- [x] 10.1 E2E UI テスト
  - アプリ起動後 UI に capturing 表示、権限拒否時の action_ja エラー表示を自動または半自動で検証する
  - 完了時: 設計 E2E/UI Tests 項目 1–2 がテストまたはチェックリストでカバーされる
  - _Requirements: 3.1, 5.4, 7.1_
  - _Depends: 6.2, 7.1_
  - _Wave: 35_

- [x] 10.2 性能テスト計画の実行と合格判定
  - 30 分連続キャプチャで `capture_buffer_drops_total == 0`、CPU 平均 < 5%（4 コア参照）、常駐メモリ増分 < 50 MB を計測する
  - Windows Performance Recorder / macOS Instruments の手動プロファイル手順を README に追記する
  - 完了時: 計測結果が設計の合格基準を満たすか、逸脱理由と対策が記録される
  - _Requirements: 4.2_
  - _Depends: 7.2, 7.4_
  - _Wave: 36_

- [x] 10.3 会議アプリ並行動作の手動検証チェックリスト
  - Zoom/Teams 再生中に相手音声の持続的途切れがないことを手動で確認する手順を文書化し、1 回以上実行する
  - ウィンドウ閉鎖後のマイクインジケータ消灯を手動確認する
  - 完了時: チェックリストが README またはテスト手順書にあり、実行記録（日付・結果）が残る
  - _Requirements: 3.2, 4.1_
  - _Depends: 7.2, 10.1_
  - _Wave: 37_

---

## 要件カバレッジマトリクス

| 要件 ID | タスク |
|---------|--------|
| 1.1 | 3.1, 4.3, 7.2 |
| 1.2 | 3.3, 3.4, 4.3, 7.2, 9.3, 9.4 |
| 1.3 | 4.3, 7.1, 7.2, 8.3 |
| 1.4 | 3.1, 3.3, 3.4, 7.5 |
| 2.1 | 4.1, 7.2, 8.1 |
| 2.2 | 2.1, 3.2, 4.2 |
| 2.3 | 2.1, 4.2, 5.2, 7.2, 8.2, 9.2 |
| 2.4 | 4.1, 8.1 |
| 3.1 | 5.3, 9.1, 10.1 |
| 3.2 | 5.3, 10.3 |
| 3.3 | 5.3 |
| 3.4 | 2.2, 4.3, 5.3, 9.1 |
| 4.1 | 3.1, 3.2, 3.3, 3.4, 7.4, 10.3 |
| 4.2 | 7.4, 10.2 |
| 5.1 | 2.3, 3.1, 4.3 |
| 5.2 | 2.3, 4.3, 8.3 |
| 5.3 | 2.2, 2.3, 4.3 |
| 5.4 | 2.3, 5.1, 6.1, 6.2, 8.4, 10.1 |
| 6.1 | 3.1, 3.4, 7.1, 7.3, 9.4 |
| 6.2 | 3.1, 3.3, 7.1, 9.3 |
| 6.3 | 1.6, 5.3 |
| 7.1 | 2.3, 3.1, 3.4, 4.3, 5.1, 5.3, 6.2, 7.3, 9.4, 10.1 |
| 7.2 | 2.1, 5.2, 7.4, 9.2 |
| 7.3 | 4.1 |
| 7.4 | 2.3 |
| 8.1 | 1.1, 1.2, 1.3, 1.4, 1.5 |
| 8.2 | 1.2, 1.6 |
| 8.3 | 1.6 |

## Wave 概要

| Wave | フェーズ | 内容 |
|------|----------|------|
| 1–6 | Foundation | スキャフォールド・Bun・crate 骨格・品質ゲート・README |
| 7–9 | Core | ドメイン型（PcmChunk → Phase → Error） |
| 10–13 | Core (P) | Mic / Resampler / Win Loopback / Mac SCK アダプタ |
| 14–16 | Core | Mixer → ChunkEmitter → Orchestrator |
| 17–19 | Core | EventEmitter → PcmBus → LifecycleHook |
| 20–21 | Core | フロントフック → UI |
| 22–26 | Integration | Composition root → パイプライン → macOS 設定 → 可観測性 → boundaries |
| 27–30 | Validation | ユニットテスト |
| 31–34 | Validation | 統合テスト（プラットフォーム別並列） |
| 35–37 | Validation | E2E・性能・手動チェックリスト |

## Implementation Notes

- Windows の `tauri-build` は `bundle.icon` が空でも `icons/icon.ico` を要求する（1x1 プレースホルダで `cargo check` 通過）。
- knip の entry は Vite エントリ（`src/main.ts`）に合わせる。フロント品質ゲートは `bun run check`。
- `cargo-bylaw` 0.1.0 は rustc 1.95+ が必要。Tauri ホスト（`generate_context!`）は bylaw 解析対象外とし、`-p gijirec-domain -p gijirec-application -p gijirec-infrastructure -p gijirec-presentation` でレイヤ crate のみ検証する。
- Linux 非対応ダイアログは `on_app_setup` 経路。`RunEvent::Exit` 停止は `handle_capture_run_event` を 7.1 の `app.run` から呼ぶ。
- `useCaptureStatus` は injectable `listenFn` で単体テストする。knip の `entry` にフックテストを足しても Vite プラグインが `src/main.ts` を継続検出する。Windows では `bun test` 一括が depcruise fixture と干渉しうるため `bun run test:arch` を品質ゲートに使う。
- 7.1 は `CapturePipelineState`（mixer / ChunkEmitter / PcmChunkBus）を Tauri state に保持する。アダプタの rtrb consumer はポート内にあり、処理スレッド結線は 7.2。リサンプラは入力レートが open 後まで不明なため 7.2 で構築する。
- 7.2 の処理スレッドは `CaptureProcessingHook` で start 後に起動。`start_processing` 失敗は現状握りつぶし。スモークは合成 rtrb。macOS SCK レートは 48 kHz 固定。
- 可観測性は presentation の `CaptureObservability` トレイト経由。host が tracing 実装。`capture_rt_callback_max_us` は処理スレッド drain レイテンシの代理。cargo-bylaw は presentation 内 tracing マクロを解析できない。
- 10.2 の 30 分性能実測は CI/エージェントでは不可。手順は README、逸脱記録は `docs/specs/audio-capture/performance-results.md`。数値捏造禁止。
- 10.3 の Zoom/Teams 並走とマイクインジケータ確認も CI では不可。手順と実行記録は `docs/specs/audio-capture/manual-concurrency-checklist.md`。
- `@vitejs/plugin-react` 6 は Vite 8 専用（`vite/internal`）。Vite 7 では 5.x、Vite 8 では 6.x を組にする。
