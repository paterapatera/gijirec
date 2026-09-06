# 要件定義書

## はじめに

gijirec は Web 会議のリアルタイム文字起こしを行うローカルデスクトップアプリである。開発モード（`cargo tauri dev`）では文字起こしが正常に動作するが、配布用リリースビルド（`cargo tauri build` の実行ファイル）では文字起こしパイプラインが機能せず、配布版が実用にならない。

本機能は、リリースビルド特有の原因（モデルパス、リソース同梱、イベント権限、パス解決等）を特定・修正し、release ビルドでも dev と同様に文字起こしが動作するようにする。Path D として brownfield 不具合修正であり、上流の `release-logging` で取得したログを手がかりに dev / release の差分を切り分ける。既存の文字起こし仕様（低遅延・オフライン・タイムスタンプ付きブロック）を変えない。

## スコープ境界

- **対象範囲**: リリースビルドでの文字起こしパイプライン全体の動作回復（モデル取得・準備、キャプチャ連動の開始／停止、転写ブロックのエディタ配信、利用者向けエラー通知）、開発モードとの動作等価性の保証
- **対象外**: 文字起こしアルゴリズムの変更、新モデル対応、Linux 対応、クラウド STT、ログ永続化機能そのもの（`release-logging` が担う）
- **隣接システム・仕様への期待**: `whisper-transcribe` で定義済みのフェーズ遷移、転写ブロック契約、利用者向けエラー形式（`message_ja` / `action_ja`）を正本とし、本機能はリリースビルドでの同等動作を回復する。障害切り分け用のログ出力は上流 `release-logging` が提供し、本機能はそのログを活用可能な観測ポイントを欠落させない。永続化ログの内容は `release-logging` のプライバシー制約（転写全文・PCM・デバイス表示名の非記録）に整合する。`audio-capture` はキャプチャフェーズイベントにより文字起こし開始の前提条件を供給する。

## 要件

### 要件 1: リリースビルドでの文字起こし動作

**目的:** 利用者として、配布版アプリでも会議中のリアルタイム文字起こしを利用したい。その結果、開発モード以外でも実用的な議事録作成ができるようになる。

#### 受け入れ条件

1. When the user starts audio capture in a release build executable and the whisper model is available, the gijirec application shall transition the transcribe phase to transcribing and deliver timestamped transcript blocks to the editor.
2. While audio capture is active in a release build executable with continuous audible input that would produce transcript blocks in development mode under equivalent conditions, the gijirec application shall append new transcript blocks to the editor within the latency window defined by whisper-transcribe requirement 3 acceptance criterion 2.
3. When the user stops audio capture in a release build executable, the gijirec application shall stop transcription and return the transcribe phase to ready without losing blocks already delivered to the editor.
4. The gijirec application shall perform all transcription in a release build executable locally without requiring network connectivity after the whisper model is available on disk.

### 要件 2: リリースビルドでのモデル取得と準備

**目的:** 利用者として、配布版アプリ初回起動時もモデル取得から文字起こし準備完了まで迷わず進めたい。その結果、リリースビルドでも初回セットアップ後に文字起こしを開始できるようになる。

#### 受け入れ条件

1. When the application starts in a release build executable without a locally available whisper model, the gijirec application shall download and verify the default whisper model and expose acquisition progress to the user.
2. When model acquisition completes successfully in a release build executable, the gijirec application shall transition the transcribe phase to ready.
3. If model acquisition or verification fails in a release build executable, the gijirec application shall transition the transcribe phase to error and surface a user-facing error with Japanese message and recommended action.
4. While the transcribe phase is ready in a release build executable, the gijirec application shall not require a development console to confirm model readiness.

### 要件 3: リリースビルドでのフェーズ・進捗表示

**目的:** 利用者として、文字起こしの現在状態とモデル取得の進捗を画面上で把握したい。その結果、リリースビルドでも待機・エラー・稼働中を開発モードと同様に判断できるようになる。

#### 受け入れ条件

1. When transcribe phase transitions occur in a release build executable, the gijirec application shall update the same user-visible transcribe status indicators used in development mode.
2. While the whisper model is being downloaded or verified in a release build executable, the gijirec application shall display model acquisition progress to the user.
3. When transcription is active in a release build executable, the gijirec application shall indicate the transcribing phase to the user through the established transcribe status UI.

### 要件 4: リリースビルドでの障害通知

**目的:** 利用者として、文字起こしが失敗または停止した場合に理由と対処を知りたい。その結果、コンソールが無い配布版でも自己解決または報告が可能になる。

#### 受け入れ条件

1. If transcription cannot start or continue due to a failure in a release build executable, the gijirec application shall surface a user-facing error with Japanese message and recommended action consistent with the whisper-transcribe user error contract.
2. If the whisper model cannot be loaded or prepared in a release build executable, the gijirec application shall transition the transcribe phase to error and notify the user rather than remaining in a non-transcribing state without user-visible explanation.
3. If transcription remains in the transcribing phase without delivering new transcript blocks beyond the latency window defined by whisper-transcribe requirement 3 acceptance criterion 2 while capture is active and continuous audible input is present in a release build executable, the gijirec application shall surface a user-visible failure indication or recoverable error rather than failing silently indefinitely.
4. Where release logging is enabled per the release-logging specification, the gijirec application shall record transcribe phase transitions and error codes in persisted logs sufficient for operators to distinguish failure points without accessing a development console.

### 要件 5: 既存文字起こし仕様の維持

**目的:** 利用者として、不具合修正後も従来の文字起こし体験が変わらないことを期待する。その結果、エディタ・保存フローとの既存連携が維持される。

#### 受け入れ条件

1. The gijirec application shall preserve append-only, timestamped transcript block delivery semantics defined by whisper-transcribe when running as a release build executable.
2. The gijirec application shall not change transcription algorithms, default model selection, or supported language configuration as part of this feature.
3. Where audio capture availability and model readiness are equivalent between development and release builds, the gijirec application shall deliver timestamped transcript blocks to the editor and surface the same transcribe phase transitions and user-facing errors as in development mode for the same user actions and audio input.
4. The gijirec application shall not introduce Linux support or new whisper model variants as part of this feature.
