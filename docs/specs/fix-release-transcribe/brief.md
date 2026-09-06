# Brief: fix-release-transcribe

## Trigger
`cargo tauri dev` では文字起こしが動作するが、`cargo tauri build` の実行ファイルでは文字起こしされない。

## Problem
リリースビルドで文字起こしパイプラインが機能せず、配布版が実用にならない。

## Desired Outcome
release ビルドでも dev と同様に文字起こしが動作する。

## Scope
- **In**: release ビルド特有の原因（モデルパス、リソース同梱、イベント権限、パス解決等）の特定と修正
- **Out**: 文字起こしアルゴリズムの変更、新モデル対応、Linux 対応

## Route
- **Path**: D
- **Rationale**: 既存 `whisper-transcribe` は実装完了済みのため、release 固有の不具合修正を独立 spec として切り出す。

## Approach
`release-logging` で取得したログを手がかりに、dev / release の差分（パス・バンドル・権限）を切り分けて修正する。

## Current State
`whisper-transcribe` は dev 環境で動作確認済み。release ビルドでは文字起こしが発生しない。

## Upstream / Downstream
- Upstream: release-logging
- Downstream: なし

## Constraints
既存の文字起こし仕様（低遅延・オフライン・タイムスタンプ付きブロック）を変えない。
