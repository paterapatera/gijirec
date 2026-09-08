# ADR-0012: 30 秒固定バッチ推論スケジュール

- **Status**: Accepted
- **Date**: 2026-09-08
- **Feature**: transcribe-batch-interval
- **Owners / Domains**: whisper-transcribe

## Context

gijirec の文字起こしは ADR-0003 に基づき VAD 駆動の低遅延ストリーミング（`WhisperStreamPcm`）で稼働している。会議中の CPU 負荷と推論タイミングの不安定さが問題となり、リアルタイム性より完全性・安定性を優先して推論間隔を約 30 秒のバッチ実行へ切り替える必要がある（transcribe-batch-interval 要件 1, 2, 4）。

上流 `PcmChunk`（100 ms）と下流 `TranscriptBlock` 追記供給（`whisper-transcribe://block-appended`）は既存契約を維持する。音声キャプチャ方式・モデル・話者分離は変更しない（要件 5）。

## Decision

文字起こし推論を **固定 30 秒バッチ方式** に切り替える。

- **スケジュール**: 前回推論サイクル完了後、約 **30 秒** 経過（または未処理 PCM が 30 秒分に達した時点）で次サイクルを開始。初版はユーザー設定 UI なし（要件 1.2）
- **PCM 蓄積**: `PcmIngestConsumer` は推論中も rtrb / `BatchWindowAccumulator` へ非破棄で蓄積（要件 2）
- **推論 API**: `WhisperCppAdapter` は VAD ストリーミングモードではなく、蓄積窓単位のバッチ transcribe API を使用。窓内でテキストが空の場合はブロックを発行しない（要件 3.4）
- **停止時**: キャプチャ停止後、保持中 PCM を最終バッチで処理（要件 2.3）
- **失敗時**: 単一サイクル失敗でキャプチャを止めず後続サイクルを継続（要件 2.4）
- **契約**: `whisper-transcribe-blocks.md` の遅延目標をバッチ方式に更新。イベント形状は変更しない

`whisper-cpp-plus` ライブラリ選択（ADR-0003）は維持する。変更はスケジュールと呼び出しモードのみ。

## Consequences

- Positive: 推論 CPU スパイクの間隔が予測可能になり、会議中の OS 負荷が緩和される（要件 4.1）
- Positive: 推論中も PCM を欠落させない設計と整合し、長時間会議の完全性が向上（要件 2, 6）
- Negative / trade-offs: 転写テキストの表示遅延は最大 30 秒 + 推論時間に増加。product の「数秒遅延」表現は本変更後はバッチ前提に更新が必要
- Negative / trade-offs: ADR-0003 の VAD ストリーミング前提は本 ADR のスケジュール判断で実質置換。ライブラリ ADR 本文は履歴として残し、Status は `Superseded by ADR-0012` とする

## Alternatives considered

1. **VAD + 最大 30 s キャップ** — 低遅延とバッチの折衷だが、トリガーが不安定で要件 1 の「固定 30 秒」を満たしにくい
2. **壁時計 30 s タイマー（サイクル完了無視）** — 推論遅延時に窓が重複または欠落しうる
3. **ユーザー設定可能な間隔** — 初版スコープ外（要件 1.2）

## Notes

- 実機検証: 10 分連続キャプチャで音声欠落なし（要件 6.1）
- 判断を覆す場合は新 ADR を作成し、本 ADR の Status を `Superseded by ADR-XXXX` に変更する
