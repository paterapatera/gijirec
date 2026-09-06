# ADR-0008: ModelStore は Tauri app_data_dir を正本とする

- **Status**: Accepted
- **Date**: 2026-09-06
- **Feature**: fix-release-transcribe
- **Owners / Domains**: whisper-transcribe / fix-release-transcribe

## Context

Whisper モデルファイル（`kotoba-whisper-v2.2-ggml-q5_0.bin`）のローカル保存先は `ModelStore` が解決する。実装では composition root が `dirs::data_local_dir()/gijirec` を渡していたが、Tauri setup では `app.path().app_data_dir()`（identifier `com.gijirec.app`）を editor 設定・release 診断ログと共有している。Windows では `Local` と `Roaming` が分かれ、リリースビルドでモデル取得・検証・他機能のデータ所在が一致しない。

## Decision

- Whisper モデルの保存ルートは **Tauri `app.path().app_data_dir()`** を正本とする。
- パス: `{app_data_dir}/models/kotoba-whisper-v2.2-ggml-q5_0.bin`（`ModelStore::MODEL_FILENAME` 既存規約を維持）。
- `ModelStore` の初期化とモデルロード開始は **Tauri setup 完了後**（`app_data_dir` 解決後）に行う。
- `dirs::data_local_dir()` による独自 `gijirec` サブディレクトリは使用しない。

## Consequences

- Positive: editor / release-logging / モデルが同一 `app_data_dir` 配下に集約。運用ドキュメント（operations.md）と一致。
- Negative / trade-offs: 既存 dev 環境で `Local\gijirec` にのみモデルがある場合、初回は再取得または移行が必要（一回限り）。

## Alternatives considered

1. **`dirs::data_local_dir` を維持** — editor とログだけ Roaming、モデルだけ Local。不採用（運用・権限の二重管理）。
2. **モデルをバンドル同梱** — 配布サイズ増大。ADR-0004 の初回ダウンロード設計と矛盾。

## Notes

- 公開契約（`whisper-transcribe-blocks` / `whisper-transcribe-status`）のイベント形状は変更しない。
- Revalidation: Tauri identifier または `app_data_dir` 解決 API 変更時に本 ADR を再検証。
