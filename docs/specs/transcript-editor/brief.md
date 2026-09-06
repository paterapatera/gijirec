# Brief: transcript-editor

## Trigger
AI 文字起こしをその場で人間が直せる編集面が、製品の最重要体験。

## Problem
流れてくる文字をすぐ直そうとすると、自動更新に上書きされる。細切れ点滅やレイアウトシフトで打てない。成果物を Markdown で残せない。

## Desired Outcome
ユーザーは手動で議事録を取り、AI はリアルタイム文字起こしを行う。リアルタイムにテキストが流れるエディタがあり、選択または入力した箇所は AI 上書きがロックされ手動が優先される。保存時に `handwriting.md`（手動議事録）と `ai-transcription.md`（AI 文字起こし・テキストのみ）の 2 ファイルを生成できる。オプションで `ai-transcription.jsonl`（タイムスタンプ付き）も取得可能。保存先ディレクトリは設定でき、次回起動時も保持される。追加時に画面がガタつかない。

手動議事録は内容は正確だが荒い、AI 議事録は誤字が多いが細かいという特徴があり、2 ファイルを組み合わせて議事録の清書をする想定。

## Scope
- **In**: ストリーミング表示エディタ、選択／入力中の部分ロック、手動議事録エディタ、`handwriting.md` / `ai-transcription.md` 出力（AI 側はテキストのみ・タイムスタンプ不要）、オプション `ai-transcription.jsonl`（タイムスタンプ付き）、保存先ディレクトリ設定（永続化）、日時ベースのサブディレクトリ構成（`{base}/{YYYY}/{MM}/{DD}/{hh}_{mm}_{ss}/`、JST 基準）、保存中も文字起こし継続、追加時のレイアウト安定
- **Out**: 音声キャプチャ、Whisper 推論本体、仮想デバイス、クラウド同期、清書の自動マージ（ユーザーが手動で行う）

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
