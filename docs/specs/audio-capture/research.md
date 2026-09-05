# Research & Design Decisions: audio-capture

## Summary
- **Feature**: audio-capture
- **Discovery Scope**: New Feature（greenfield）/ Complex Integration — Full discovery
- **Key Findings**:
  - Windows システム音声は cpal が出力デバイスへの入力ストリームで WASAPI ループバックを自動有効化する
  - macOS システム音声は ScreenCaptureKit が正規手段。オーディオのみモードはなく 2×2 px ダミー映像が実務パターン
  - 二系統キャプチャはクロックが独立するため、サンプルカウント基準の整列バッファが必須

## Research Log

### Windows WASAPI ループバック（cpal）
- **Context**: 仮想デバイスなしでシステム音声を取得する方法
- **Sources Consulted**: cpal WASAPI backend ドキュメント、PR #478、auricle-capture crate
- **Findings**:
  - 既定出力デバイスに `build_input_stream` + `default_output_config()` を使用
  - ループバックは cpal が `AUDCLNT_STREAMFLAGS_LOOPBACK` を自動設定
  - リアルタイムコールバックは allocation-free + リングバッファが推奨
- **Implications**: `gijirec-infrastructure` に Windows 専用アダプタを配置

### macOS ScreenCaptureKit システム音声
- **Context**: cpal は macOS ループバック非対応
- **Sources Consulted**: screencapturekit-rs、DEV 記事（Alexander Mishin）、mac-audio-recorder
- **Findings**:
  - macOS 13+ で `capturesAudio = true`、画面収録権限が必要
  - 2×2 px / 1 fps の最小映像ストリームを破棄するパターンが標準
  - `excludesCurrentProcessAudio = true` で自アプリ音のフィードバックを防止
  - Core Audio TAP は低遅延だが長時間会議でクロック停止リスク
- **Implications**: `screencapturekit` crate を macOS ターゲット限定依存に追加（ADR-0001）

### ミキシング・レベル調整
- **Context**: 要件 2.4 — 入力レベル差への対応
- **Sources Consulted**: tauri-plugin-system-audio（参考）、second プロジェクトの mix 実装
- **Findings**:
  - f32 内部処理 → 各ソース RMS を 200 ms 窓で推定 → ターゲット -20 dBFS へゲイン調整 → ソフトリミッター → i16 量子化が STT 向け実績パターン
  - マイクとループバックを別々に取得し application 層でミックス（単系統フォールバック禁止は要件 5.2）
- **Implications**: `AudioMixer` を application 層に配置。アルゴリズム詳細は実装タスクで TDD

### Bun + Tauri 2 ツールチェーン
- **Context**: 要件 8
- **Sources Consulted**: Tauri v2 create-project ドキュメント、create-tauri-app README
- **Findings**: `bunx create-tauri-app`、`bun tauri dev/build` が公式サポート
- **Implications**: ADR-0002。package.json / tauri.conf.json を Bun 前提でスキャフォールド

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Hexagonal（採用） | domain + application + infrastructure + presentation | steering 準拠、cargo bylaw で強制可能 | 初期 crate 数 | gijirec-* crates |
| Tauri プラグイン単体 | tauri-plugin-system-audio | Windows 実装済み | macOS ループバックなし | 不採用 |
| フロント WebAudio ミックス | getUserMedia + 画面共有音声 | ブラウザ API | システム音声不安定、要件不適合 | 不採用 |

## Design Decisions

### Decision: プラットフォーム別システム音声アダプタ
- **Context**: 要件 1, 6
- **Alternatives**: 単一 cpal、tauri-plugin-system-audio、仮想デバイス
- **Selected Approach**: Windows cpal ループバック + macOS ScreenCaptureKit（ADR-0001）
- **Rationale**: 両 OS で仮想デバイス不要かつ要件を満たす唯一の組み合わせ
- **Trade-offs**: macOS 追加権限、クロック整列の複雑さ
- **Follow-up**: 実機で Zoom/Teams 並行時の CPU プロファイル

### Decision: 100 ms PCM チャンク供給
- **Context**: 要件 2.3（間隔は設計定義）
- **Alternatives**: 20 ms（フレーム単位）、500 ms、3 s（Whisper チャンクと同長）
- **Selected Approach**: 内部 20 ms フレーム、100 ms（1600 サンプル）で `PcmChunk` 発行
- **Rationale**: 下流バッファリングとレイテンシのバランス。Whisper 側でさらに集約可能
- **Trade-offs**: 下流が異なる間隔を望む場合は再サンプリングが必要
- **Follow-up**: whisper-transcribe 設計で消費バッファサイズを整合

### Decision: 汎用化 — `CaptureOrchestrator` + `PcmChunkConsumer`
- **Context**: 合成レンズ — 将来の複数下流を想定しつつ v1 は単一消費者
- **Selected Approach**: trait で下流登録。ミキサー出力は常に単一 PCM ストリーム
- **Rationale**: whisper-transcribe 以外の将来消費者に interface を一般化、実装は単一

## Risks & Mitigations
- **クロックドリフト** — 50 ms 整列リングバッファ + サンプルカウント基準タイムライン
- **macOS 権限拒否** — 起動時 preflight + `audio-capture-status` の action_ja 誘導（5.4, 7.1）
- **会議アプリ CPU 競合** — コールバック内アロケーション禁止、性能テスト計画で 5% CPU 上限を検証（4.2）
- **エコー／フィードバック** — v1 はソフトリミッターのみ。AEC は将来 spec（要件外）

## References
- [cpal WASAPI loopback](https://github.com/RustAudio/cpal/pull/478) — ループバック API パターン
- [screencapturekit-rs](https://doom-fish.github.io/screencapturekit-rs/screencapturekit/) — macOS SCK Rust バインディング
- [ScreenCaptureKit システム音声（DEV）](https://dev.to/alexander_mishin_1c72158e/how-i-record-system-audio-on-macos-without-a-virtual-driver-54e1) — ダミー映像パターン
- [Tauri 2 Create Project](https://v2.tauri.app/start/create-project/) — Bun サポート
