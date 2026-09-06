# 要件定義書

## はじめに

gijirec は Web 会議のリアルタイム文字起こしを行うローカルデスクトップアプリである。開発モード（`cargo tauri dev`）ではコンソールログによりキャプチャ・文字起こしの障害を調査できるが、配布用リリースビルド（`cargo tauri build` の実行ファイル）ではコンソールが利用できず、ログを参照して原因を切り分けられない。

本機能は、既存の observability 出力（キャプチャ・文字起こし・エディタの phase 遷移、エラーコード、メトリクス等）を、リリースビルド実行時に **明示的な CLI オプションを付けた場合のみ** ローカルファイル等に永続化し、運用者・開発者が不具合調査を自己完結で行えるようにする。オプション未指定の通常起動ではログを出力しない。Path D として brownfield 拡張であり、下游の `fix-release-transcribe` 等の障害切り分けを可能にする。

## スコープ境界

- **対象範囲**: リリースビルド向けのログ永続化（CLI オプションによる opt-in）、保存場所・取得手順の基本方針、既存 observability イベントの網羅、セキュリティ・プライバシー制約との整合
- **対象外**: クラウドへのログ送信、本格的な APM、利用者向けサポート UI、ログのリアルタイム画面表示
- **隣接システム・仕様への期待**: `audio-capture` / `whisper-transcribe` / `transcript-editor` で定義済みの observability イベント（phase 遷移、error code、buffer drop 等）を正本とし、本機能はそれらの永続化とリリースビルドでの可視化を担う。ログマスキング方針は開発時 observability と同一（capture / transcribe / editor の既存マスキング実装に整合）とし、要件 4 で永続化時の禁止・制限を明示する。実装の詳細（subscriber 構成、ローテーション方式等）は設計フェーズに委ねる。

## 要件

### 要件 1: リリースビルドでのログ永続化

**目的:** 開発者・運用者として、リリースビルド実行中にもアプリの動作ログを確認したい。その結果、コンソールが無い環境でも障害の切り分けができるようになる。

#### 受け入れ条件

1. When the application is started from a release build executable with the documented logging CLI option enabled, the gijirec application shall persist observability log output to a durable local file for the duration of the session.
2. When the application is started from a release build executable without the documented logging CLI option, the gijirec application shall not persist observability logs to files or emit log output to the console.
3. While the application is running in a release build with the logging CLI option enabled, the gijirec application shall record the same categories of observability events that are emitted to the development console (capture, transcribe, and editor domains).
4. While the application is running in development mode with console logging available, the gijirec application shall not require file-based log persistence for normal operation.
5. If log file persistence is enabled and fails at startup or during an active session, the gijirec application shall continue user-facing features without blocking and shall surface the persistence failure through any available diagnostic output.

### 要件 2: ログ保存場所と取得手順

**目的:** 開発者・運用者として、ログファイルの所在と収集方法を迷わず知りたい。その結果、不具合報告・二次調査の手順が標準化される。

#### 受け入れ条件

1. The gijirec application shall document the logging CLI option, default log storage location, and basic collection steps in operator-accessible documentation associated with this feature.
2. When a new application session begins in a release build with the logging CLI option enabled, the gijirec application shall write logs under a predictable directory within the application's local data area without requiring elevated privileges.
3. Where multiple application sessions occur with logging enabled, the gijirec application shall distinguish log output by session so that operators can identify logs from a specific run.
4. When an operator locates the log directory after a session started with the logging CLI option enabled, the gijirec application shall have written at least one log file containing entries from that session without requiring in-app UI navigation.

### 要件 3: 障害調査に必要なドメインイベントの記録

**目的:** 開発者として、キャプチャ・文字起こし・エディタの障害調査に必要なイベントをログから追跡したい。その結果、文字起こし不具合等の下游調査が可能になる。

#### 受け入れ条件

1. When capture phase transitions occur and release logging is enabled, the gijirec application shall include the phase name and associated error codes in the persisted release log.
2. When transcribe phase transitions or transcribe errors occur and release logging is enabled, the gijirec application shall include phase, error codes, and correlation identifiers in the persisted release log.
3. When editor save operations start or complete and release logging is enabled, the gijirec application shall include operation outcome indicators in the persisted release log without recording full transcript or handwritten body text.
4. If buffer drops, PCM sequence gaps, or inference latency anomalies are detected during capture or transcribe and release logging is enabled, the gijirec application shall record counts and timing metrics in the persisted release log.
5. Where release logging is enabled, the gijirec application shall default persisted release logs to phase transitions, warnings, and errors, and shall not include debug-level pipeline detail by default.

### 要件 4: セキュリティとプライバシー

**目的:** 利用者として、会議内容が不必要にログに残らないことを期待する。その結果、障害調査とプライバシー保護が両立する。

#### 受け入れ条件

1. The gijirec application shall not persist PCM audio samples, full transcript text, or microphone device display names in release log files.
2. The gijirec application shall apply the same log masking rules used for development observability to persisted release logs.
3. If internal diagnostic detail is recorded for troubleshooting, the gijirec application shall limit such detail to developer-oriented fields (error codes, correlation identifiers, counts, and timing) rather than user-generated content.
4. The gijirec application shall perform all release log persistence locally without requiring network connectivity.
5. Where release logging is enabled, the gijirec application shall not transmit log contents to external services.
