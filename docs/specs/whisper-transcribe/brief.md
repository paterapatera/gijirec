# Brief: whisper-transcribe

## Trigger
ミックス済み音声を、数秒以内の遅延で画面に流すローカル文字起こしが必要。

## Problem
録音を後から起こす運用では、その場で内容を追えない。クラウドや Python 依存だとオフライン会議で使えない。

## Desired Outcome
3〜5 秒チャンクをローカル whisper.cpp に逐次投入し、発言から数秒以内にテキストがストリーミング追加される。各ブロックに音声開始タイムスタンプが紐づく。モデル取得後はオフラインで動く。終了時は推論プロセスも止まる。

## Scope
- **In**: ミックス PCM のチャンク投入、whisper.cpp（Rust バインディング、Python なし）、低遅延のテキストストリーム、ブロック単位タイムスタンプ、初回モデル取得後のオフライン推論、終了時の推論停止
- **Out**: 手動編集 UI、部分ロック、Markdown 出力、クラウド STT、話者分離

## Route
- **Path**: D
- **Rationale**: キャプチャとは別の推論パイプラインであり、後段エディタの入力になる。

## Approach
Python に依存せず、Rust バイナリ内の whisper.cpp で完結させる。

## Current State
緑地。`audio-capture` のミックス PCM を前提にする。

## Upstream / Downstream
- Upstream: audio-capture
- Downstream: transcript-editor

## Constraints
会議の裏で CPU・メモリが極端に重くならない。インターネットはモデル初回取得以外に不要。
