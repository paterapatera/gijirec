# Brief: release-logging

## Trigger
`cargo tauri build` の実行ファイルで文字起こしが動かない。`cargo tauri dev` ではコンソールログで調査できるが、ビルド版ではログが見えず原因の切り分けができない。

## Problem
配布用ビルドではログ出力先がなく、キャプチャ・文字起こしの障害を自己完結で調査できない。

## Desired Outcome
ビルド済み実行ファイルでもログファイル等でログを確認でき、不具合調査が可能になる。

## Scope
- **In**: リリースビルド向けログ出力（ファイル書き出し等）、保存場所・取得手順の基本方針
- **Out**: クラウドへのログ送信、本格的な APM、ユーザー向けサポート UI

## Route
- **Path**: D
- **Rationale**: 既存 spec は実装完了済みのため、ビルド版ログ確認を独立 spec として切り出す。

## Approach
既存の tracing / observability 基盤を拡張し、リリースビルドでもファイル等にログを永続化する。

## Current State
開発時はコンソール出力のみ。`audio-capture` / `whisper-transcribe` / `transcript-editor` は実装済み。

## Upstream / Downstream
- Upstream: なし
- Downstream: fix-release-transcribe

## Constraints
モデル初回取得後はオフラインで動くこと。ログに機密会議内容を不必要に残さない設計を検討する。
