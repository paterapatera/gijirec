# ADR-0007: release ビルド向け tracing ファイル永続化

- **Status**: Accepted
- **Date**: 2026-09-06
- **Feature**: release-logging
- **Owners / Domains**: release-logging（cross-cutting）

## Context

開発モードでは `tracing-subscriber` の stdout 出力でキャプチャ・文字起こし・エディタの observability を確認できる。リリースビルド（`cargo tauri build`）ではコンソールがなく、障害切り分けができない。既に各ドメインは trait ベース observability + ホスト側 `Tracing*Observability` で構造化イベントを発火しているが、subscriber は console のみ。

## Decision

1. **`tracing-appender`** をホスト crate（`src-tauri`）に追加し、`cfg!(not(debug_assertions))`（release ビルド）**かつ `--log` CLI オプション指定時のみ** non-blocking file layer を `tracing_subscriber::Registry` に載せる。オプション未指定の release 起動ではログ出力しない（noop subscriber）。
2. ログファイルは Tauri **`app_data_dir/logs/sessions/{run_session_id}/gijirec.log`** に書き込む。最新セッションは **`logs/latest-session.txt`** で参照する（`--log` 起動時のみ更新）。
3. **`WorkerGuard`** は Tauri managed state でプロセス寿命中保持し、異常終了時も flush を保証する。
4. presentation / application / infrastructure の observability trait とマスキング実装は **変更しない**。永続化は subscriber レイヤのみ。
5. 永続化失敗時はユーザー機能をブロックせず、diagnostic 出力 + WARN イベントで surface する。

## Consequences

- Positive: 既存 observability 投資を再利用。bylaw（presentation に tracing マクロ禁止）を維持。`fix-release-transcribe` がファイルログを参照可能。
- Negative / trade-offs: release ビルドでのみ検証しやすい。ローテーション未実装のため長時間実行でファイル肥大化の可能性（現 product スコープでは許容）。shared machine の ACL は OS 依存。

## Alternatives considered

- **常時 file（release 自動）**: 通常利用でもログが残りプライバシーリスク — **不採用**（人間ゲート fix で opt-in に変更）。
- **常時 file + stdout**: dev でもファイル生成 — 要件 1.4 に反するため不採用。
- **ドメイン別ログファイル**: マスキング・収集手順が複雑化 — 不採用。
- **カスタムログ crate**: tracing エコシステムから分離 — 不採用。

## Notes

- 契約正本: `docs/contracts/release-logging-persistence.md`
- Revalidation: observability イベント形状の破壊的変更、app_data_dir 解決タイミングの変更、禁止フィールド方針の変更
