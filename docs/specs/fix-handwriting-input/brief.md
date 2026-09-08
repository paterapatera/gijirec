# Brief: fix-handwriting-input

## Trigger
動作確認中、手入力エリア（HandwritingEditor）で日本語 IME 入力時に不具合を確認した。

## Problem
手入力中に文字が勝手に消えたり、変換確定前に変換が決定されてしまう。親の AI 転写更新による再描画が手入力 Slate に波及している可能性がある。

## Desired Outcome
手入力エリアは AI 転写の更新で再描画されず、IME 変換中も入力内容が保持される。通常の日本語入力が安定してできる。

## Scope
- **In**: HandwritingEditor の再描画抑止（memo / 更新ツリー分離）、IME composition との競合防止、回帰防止
- **Out**: AiTranscriptEditor の IME 問題（別 issue）、Rust バックエンド、新 UI 機能、Linux 対応

## Route
- **Path**: C
- **Rationale**: v1 spec はアーカイブ済みのため、`fix-release-transcribe` と同様に focused fix 用の新 spec を起票する。

## Approach
手入力パネルは upstream（AI 転写ブロック）と独立しているため、**再描画自体を抑止・分離する**ことを第一手段とする。`React.memo` や購読の局所化で親更新の波及を止め、必要なら IME composition 中の追加ガードを要件で具体化する。

## Current State
`HandwritingEditor` は props なしの Slate 実装。ブロック購読は `TranscriptEditorView` 親で行い、AI 側更新のたびに子ツリー全体が re-render される構造。transcript-editor は実装完了・アーカイブ済み。

## Upstream / Downstream
- **Upstream**: transcript-editor（概念上・完了・アーカイブ済み。`docs/specs/` には存在しない）
- **Downstream**: none

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で OS 全体を極端に重くしない。テキスト追加時に激しいレイアウトシフトや点滅を起こさない。
