# release-logging-persistence

- **Surface type**: Data ownership
- **Owners / Domains**: release-logging（cross-cutting 診断ログ）
- **Related ADR**: `docs/architecture/adr/ADR-0007-release-file-logging.md`

## Purpose

リリースビルド実行時に、gijirec がローカルに永続化する診断ログファイルの保存場所・セッション識別・禁止フィールドを定義する。下流の `fix-release-transcribe` 等がログ収集手順の正本として参照する。

## Contract

### 保存場所

| 項目 | 規約 |
|------|------|
| ルート | Tauri `app.path().app_data_dir()` 配下 |
| セッションログ | `{app_data_dir}/logs/sessions/{run_session_id}/gijirec.log` |
| 最新セッション参照 | `{app_data_dir}/logs/latest-session.txt`（1 行、UTF-8、`run_session_id` のみ） |
| 権限 | OS 標準のユーザーデータディレクトリ。管理者権限不要 |

### セッション識別

| フィールド | 形式 | 備考 |
|------------|------|------|
| `run_session_id` | `{YYYYMMDDTHHMMSSZ}-{capture_session_suffix}` | UTC ISO8601 基本形 + 既存 capture `session_id()` の suffix（例: `20260906T074500Z-capture-0`） |
| ログ行内 `session_id` | 既存 capture / transcribe observability の値 | transcribe イベントに含まれる相関 ID |

### ログ行形式

- **エンコーディング**: UTF-8
- **1 行 1 イベント**: `tracing-subscriber` fmt デフォルト（plain text、タイムスタンプ + level + target + fields）
- **デフォルト verbosity**: `EnvFilter` 未指定時 `gijirec_capture=info,gijirec_transcribe=info,gijirec_editor=info,info`（debug パイプライン詳細は除外）

### 記録対象イベント（カテゴリ）

release ログは開発コンソールと同一の observability イベントを記録する。

| Domain | Target | 代表イベント / フィールド |
|--------|--------|---------------------------|
| capture | `gijirec_capture` | `capture_phase`, `capture_buffer_drops_total`, `error_code`, `correlation_id`, `capture_rt_callback_max_us` |
| transcribe | `gijirec_transcribe` | `transcribe_phase`, `error_code`, `session_id`, gap/drop/latency メトリクス |
| editor | `gijirec_editor` | `editor_save_started`, `editor_save_completed`, `settings_updated`（本文長・件数のみ） |

### 禁止フィールド（永続化してはならない）

以下は release ログファイルに **含めてはならない**（Req 4.1–4.3）:

- PCM 音声サンプル、生 PCM バイト列
- 転写全文、手書き Markdown 本文、JSONL レコード本文
- マイク / スピーカーの **デバイス表示名**（OS が返す human-readable name）
- ネットワーク送信（本契約のログ内容を外部サービスへ送らない）

**許可される識別子**: abstract port 名（`mic`, `system`）、`error_code`、`session_id`、相関 ID、カウント、タイミング。

### ビルドモードと CLI オプション

| モード | CLI | ファイル永続化 | コンソール |
|--------|-----|----------------|------------|
| debug（`cargo tauri dev`） | （任意）`--log` は **無視** | **行わない** | stdout（現行どおり） |
| release | なし（デフォルト） | **行わない** | なし |
| release | `--log` | **行う** | なし |

判定: Rust `cfg!(not(debug_assertions))` **かつ** `--log` フラグ。

**正本オプション名**: `--log`（operations 文書と同一）

### 永続化失敗

- ディレクトリ作成・appender 初期化・セッション中の書き込み失敗時、アプリのユーザー向け機能は継続する。
- 失敗は利用可能な diagnostic 出力（stderr 等）と tracing WARN イベント（`release_log_persistence_failed=true`）で surface する。

## Non-goals

- クラウドログ送信、APM、リアルタイム UI 表示
- 自動ローテーション / 保持期間ポリシー（将来 spec で検討可）
- フロントエンドからのログ閲覧 API

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-06 | CLI opt-in — デフォルトはログ出力なし、`--log` 指定時のみ永続化 | 人間ゲート fix（プライバシー・通常利用でのログ残存回避） |
| 2026-09-06 | 初版 — 保存場所・セッション ID・禁止フィールド・ビルドモード | ADR-0007 |

## Notes

- マスキング実装の正本は `gijirec-presentation` の observability モジュール。本契約は永続化時の禁止事項を要求レベルで固定する。
- 運用者向け収集手順: `docs/specs/release-logging/operations.md`
