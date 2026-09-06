# 実装計画

## 1. Foundation: ホスト logging 基盤の準備

- [x] 1.1 tracing-appender 依存追加と logging モジュール骨格
  - ホスト crate に `tracing-appender` 依存を追加し、release ログ永続化用の logging モジュールを新設する
  - 公開 API（release ログ設定、セッション ID 生成、永続化レイヤ初期化）のモジュール境界を定義し、ホスト起動コードから参照可能にする
  - logging モジュールがコンパイル・単体 import 可能な状態になる
  - _Requirements: 1.1_
  - _Boundary: ReleaseLogPersistence_
  - _Design: D-ReleaseLogPersistence_
  - _Wave: 1_

## 2. Core: CLI オプション解析（ReleaseLogCli）

- [x] 2.1 (P) `--log` フラグの起動時解析
  - アプリ起動の入口で CLI 引数を解析し、`--log` 指定時のみ release ビルドでファイル永続化を有効化する設定を生成する
  - release ビルドでオプション未指定の場合はファイル・コンソール出力を行わない設定になる
  - debug ビルドでは `--log` を無視し、開発コンソール出力のみとなる（ファイル永続化は行わない）
  - 未知の CLI オプションは無視し、将来の Tauri CLI 拡張と共存できる
  - 単体テストで `--log` あり/なし、debug/release ビルド設定の各組み合わせが期待どおり判定される
  - _Requirements: 1.1, 1.2, 1.4_
  - _Boundary: ReleaseLogCli_
  - _Design: D-ReleaseLogCli_
  - _Depends: 1.1_
  - _Wave: 2_

## 3. Core: セッション永続化（ReleaseLogPersistence）

- [x] 3.1 (P) 実行セッション ID 生成とログパス解決
  - UTC タイムスタンプと既存 capture セッション ID の suffix を組み合わせた `run_session_id` を生成する
  - ファイルシステム上安全な文字のみを使用し、セッションごとに一意のディレクトリ名になる
  - Tauri `app_data_dir` 配下に、契約どおりの相対パス（`logs/sessions/{run_session_id}/gijirec.log`）を解決する
  - 管理者権限を要せず OS 標準のユーザーデータ領域に保存できる
  - 単体テストで ID 形式、capture suffix の含有、禁止文字の排除が確認できる
  - _Requirements: 2.2, 2.3, 4.4_
  - _Boundary: ReleaseLogPersistence_
  - _Design: D-ReleaseLogPersistence_
  - _Contracts: docs/contracts/release-logging-persistence.md_
  - _Depends: 1.1_
  - _Wave: 4_

- [x] 3.2 セッションログファイル作成と latest-session ポインタ
  - `--log` 有効時のみセッションディレクトリを作成し、non-blocking ファイル appender で `gijirec.log` に追記する
  - セッション開始時に `logs/latest-session.txt` を最新の `run_session_id` で上書きする
  - ディレクトリ作成・appender 構築失敗時は初期化エラーを返し、呼び出し側が非ブロッキング degrade を選択できる
  - 単体テストでパス構築とセッション単一ファイル（ローテーションなし）設定が確認できる
  - _Requirements: 1.1, 1.5, 2.4_
  - _Boundary: ReleaseLogPersistence_
  - _Design: D-ReleaseLogPersistence_
  - _Contracts: docs/contracts/release-logging-persistence.md_
  - _Depends: 3.1_
  - _Wave: 5_

## 4. Integration: tracing subscriber 初期化（TracingInit）

- [x] 4.1 Registry 構成とビルドモード分岐
  - `EnvFilter` デフォルトを capture / transcribe / editor 各ドメイン info 以上に設定し、debug パイプライン詳細はデフォルトで除外する
  - debug ビルドは stdout への fmt 出力のみとし、`--log` は無視する
  - release ビルドで `--log` 未指定時は noop subscriber とし、ファイル・コンソール・`logs/` ディレクトリの副作用を一切発生させない
  - 既存の capture / transcribe / editor observability 登録フローを init 内で維持する
  - debug ビルドの単体テストで `logs/` ディレクトリが作成されないことを確認できる
  - _Requirements: 1.2, 1.3, 1.4, 3.5, 4.5_
  - _Boundary: TracingInit_
  - _Design: D-TracingInit_
  - _Depends: 2.1_
  - _Wave: 6_

- [x] 4.2 release ファイルレイヤ接続と WorkerGuard 寿命管理
  - Tauri setup 内で `app_data_dir` 解決後に file fmt layer を Registry に追加し、release + `--log` 時のみ永続化を有効化する
  - `WorkerGuard` を Tauri managed state で保持し、アプリ終了まで non-blocking writer が生存する
  - 永続化失敗時は tracing WARN（`release_log_persistence_failed=true`）と利用可能な diagnostic 出力で surface し、ユーザー向け機能はブロックしない
  - release + `--log` 起動後、既存 observability が emit する capture / transcribe / editor イベントが同一カテゴリでログファイルに記録される
  - _Requirements: 1.1, 1.3, 1.5_
  - _Boundary: TracingInit_
  - _Design: D-TracingInit_
  - _Depends: 3.2, 4.1_
  - _Wave: 7_

## 5. Operations documentation（OperationsDoc）

- [x] 5.1 (P) 運用手順ドキュメントの要件整合性検証
  - 既存の `operations.md` が `--log` CLI オプション、デフォルト保存場所、基本収集手順を記載していることを確認する（新規作成は不要）
  - 契約 `release-logging-persistence.md` と乖離がある場合のみ内容を更新する
  - 運用者がドキュメントのみでログ有効化から収集までの手順を完結できる状態になる
  - _Requirements: 2.1_
  - _Boundary: OperationsDoc_
  - _Design: D-OperationsDoc_
  - _Depends: 1.1_
  - _Wave: 3_

## 6. Validation: 単体・結合テスト

- [x] 6.1 release 設定の結合スモークテスト
  - release ビルド設定 + `--log` 有効時: セッション開始後に `gijirec.log` が存在し、capture phase 遷移行を含む
  - release ビルド設定 + オプション未指定時: `logs/sessions/` ディレクトリが作成されない
  - 複数セッション起動時: 各 `run_session_id` ごとに独立したログファイルが生成される
  - ログ行に transcribe の phase / error_code / session_id、editor save の outcome 指標、buffer drop / PCM gap / latency メトリクスが記録される
  - _Requirements: 1.1, 1.2, 1.3, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4_
  - _Depends: 4.2_
  - _Wave: 8_

- [x] 6.2 永続化失敗 degrade の結合テスト
  - 起動時: 書き込み不可ディレクトリでもアプリ setup が Err にならずユーザー機能が継続する
  - セッション中: ログ書き込み開始後のディスク満杯または権限喪失をシミュレートし、ユーザー機能が継続し `release_log_persistence_failed=true` WARN が surface される
  - _Requirements: 1.5_
  - _Depends: 4.2_
  - _Wave: 9_

- [x] 6.3 プライバシー・マスキング回帰検証
  - 既存 editor `save_log_fields_omit_markdown_bodies` 等の observability 単体テストが変更なしで pass する（presentation observability モジュールは変更しない）
  - release ログに PCM 音声、転写全文、マイク/スピーカーのデバイス表示名が含まれないことを確認する
  - 永続化はローカルファイルのみで、ネットワーク送信を行わない
  - _Requirements: 3.3, 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Depends: 6.1_
  - _Wave: 10_
