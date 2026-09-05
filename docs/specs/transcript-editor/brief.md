# Brief: transcript-editor

## Trigger
AI 文字起こしをその場で人間が直せる編集面が、製品の最重要体験。

## Problem
流れてくる文字をすぐ直そうとすると、自動更新に上書きされる。細切れ点滅やレイアウトシフトで打てない。成果物を Markdown で残せない。

## Desired Outcome
リアルタイムにテキストが流れるエディタがあり、選択または入力した箇所は AI 上書きがロックされ手動が優先される。修正後もタイムスタンプ構造は残る。最終的に `.md` で保存できる。追加時に画面がガタつかない。

## Scope
- **In**: ストリーミング表示エディタ、選択／入力中の部分ロック、タイムスタンプ維持、Markdown エクスポート、追加時のレイアウト安定
- **Out**: 音声キャプチャ、Whisper 推論本体、仮想デバイス、クラウド同期

## Route
- **Path**: D
- **Rationale**: 部分ロック付き編集とエクスポートはキャプチャ／推論と別ドメイン。

## Approach
TypeScript の Web UI。Slate.js や Lexical 等で部分ロックを制御する（フレームワークは要求・設計で確定）。

## Current State
緑地。`whisper-transcribe` のテキスト＋タイムスタンプを前提にする。

## Upstream / Downstream
- Upstream: whisper-transcribe
- Downstream: なし

## Constraints
テキスト追加で激しいレイアウトシフトや点滅を起こさない。
