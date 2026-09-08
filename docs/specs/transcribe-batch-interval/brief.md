# Brief: transcribe-batch-interval

## Trigger
動作確認・改善依頼。そこまで高速なリアルタイム性は不要なので、推論を 30 秒ごとに実行したい。あわせて、音声が欠落しないようにしてほしい。

## Problem
現状は VAD 駆動の低遅延ストリーミング推論（`whisper-cpp-plus`、100 ms PCM チャンク配信）で、会議中の CPU 負荷や推論タイミングの不安定さが起き得る。推論間隔を延ばすと、バッファ／キュー設計が不十分な場合に音声欠落やタイムスタンプのずれが発生しうる。

## Desired Outcome
Whisper 推論は約 **30 秒間隔** で実行され、キャプチャ中の PCM は推論処理中も **欠落なく** 蓄積・処理される。転写ブロックは既存のエディタ連携（`block-appended` 等）と整合し、リアルタイム性より **完全性・安定性** を優先する。

## Scope
- **In**:
  - 推論トリガーを 30 秒間隔のバッチ方式に変更（固定 30 s）
  - 推論中も PCM チャンクを欠落させないバッファ／キュー設計
  - 既存の転写ブロック供給・タイムスタンプ・エディタ IPC との整合
  - 実機で音声欠落がないことを確認できる検証観点
- **Out**:
  - 推論間隔のユーザー設定 UI（初版は固定 30 s）
  - クラウド STT、モデル変更、話者分離
  - キャプチャ方式（cpal / loopback 等）そのものの変更

## Route
- **Path**: C
- **Rationale**: アーカイブ済み v1 を横断する単一の推論スケジュール変更で、新 spec として切り出すのが適切。

## Approach
VAD 駆動の逐次推論から、30 秒窓のバッチ推論へ切り替え。キャプチャ側は継続受信し、推論ワーカーがビジーでも rtrb 等でオーバーフロー／ドロップしないようバックプレッシャー付きバッファを維持する。

## Current State
v1 完了。`whisper-transcribe` はアーカイブ済み。`PcmChunkBus`（100 ms チャンク）、`WhisperCppAdapter`（VAD ストリーミング）、`TranscriptBlockBus` が稼働中。

## Upstream / Downstream
- **Upstream**: 音声キャプチャ＋PCM ミックス（実装済み `audio-capture` 領域）
- **Downstream**: 転写ブロック消費・エディタ（実装済み `transcript-editor` 領域）

## Constraints
仮想オーディオデバイス不要、オフライン運用、会議中に OS を極端に重くしない。テキスト追記時の激しいレイアウトシフト・点滅は起こさない（既存 product 制約を維持）。
