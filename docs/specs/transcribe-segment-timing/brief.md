# Brief: transcribe-segment-timing

## Trigger
動作確認中、AI 転写の「言葉の区切り」が遅く感じ、もっと早くブロックが確定してほしい。

## Problem
発話の区切りまで待つ時間が長く、テキストが流れる体感が遅い。会議中のリアルタイム性が損なわれる。

## Desired Outcome
言葉（発話区切り）の確定タイミングが現状の **おおよそ半分** の待ち時間になる。区切りが早まり、転写テキストがより速く UI に現れる。

## Scope
- **In**
  - Whisper ストリーミング推論の区切りタイミング調整（VAD / 無音判定 / 窓長など、現行パラメータのチューニング）
  - 目標: 現状比 ~50% の区切り待ち時間
  - 品質劣化（過剰分割・繰り返し・欠落）が許容範囲内かの確認
- **Out**
  - モデル変更（kotoba-whisper 以外への切替）
  - エディタ UI 変更
  - 手入力エリア（`fix-handwriting-input`）
  - Linux 対応
  - ユーザー向け設定 UI（今回は定数チューニングのみ）

## Route
- **Path**: C
- **Rationale**: whisper-transcribe 領域のチューニング要求。v1 spec はアーカイブ済みのため、単一 scope の新 spec として起票する。

## Approach
現行の VAD / 区切り関連パラメータをベースライン計測し、**1 軸ずつ** 半分方向に調整（`docs/steering/tech.md` の Whisper 調整ルールに従う）。実機で区切り速度と品質のトレードオフを確認する。

## Current State
whisper-transcribe は完了・アーカイブ済み。`whisper-cpp-plus` による VAD 駆動ストリーミング。区切り不良は窓長・`single_segment` / `entropy_thold` 等と関連（steering 記載）。

## Upstream / Downstream
- **Upstream**: whisper-transcribe（概念上・完了・アーカイブ済み。`docs/specs/` には存在しない）
- **Downstream**: none

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で OS 全体を極端に重くしない。モデル初回取得後はオフラインで全機能が動く。Whisper パラメータは 1 軸ずつ変更し、`bun run verify` + 実機確認する。
