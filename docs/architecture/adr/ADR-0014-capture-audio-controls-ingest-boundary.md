# ADR-0014: Capture audio controls ingest boundary

- **Status**: Accepted
- **Date**: 2026-09-10
- **Feature**: capture-audio-controls
- **Owners / Domains**: capture-audio-controls

## Context

`transcribe-volume-normalize` は `PcmIngestConsumer` に固定ゲイン（×1.25 + ソフトリミット 0.95）を単一定義した。新要件では (1) マイクを ingest ミックスから除外（OS ミュートではない）、(2) 手動ゲイン調整、(3) ingest 直前 dBFS 表示が必要。マイク除外を ingest 後で行うと PCM 契約を変えずに済むが、既にミキサーで合成済みのためキャプチャ処理でのゲートが必要。

## Decision

1. **マイク OFF** — `capture_processing` で `mic_ingest_enabled == false` のとき `mixer.push_mic` を呼ばない。ハードウェアキャプチャは継続するが転写 ingest ミックスにはマイクを含めない。
2. **手動ゲイン** — `transcribe-volume-normalize` の固定定数を削除し、セッション乗数 `manual_ingest_gain`（既定 1.25、`gain_user_adjusted` フラグ付き）に置換。ソフトリミット 0.95 は常時適用。
3. **dBFS メーター** — ingest 後 RMS を 1 Hz で `capture-audio-controls://ingest-level` に配信。生 PCM はフロントへ送らない。
4. **IPC** — 新契約 `capture-audio-controls.md`。信頼モデルは `audio-device-selection` 同等（ローカル単一ユーザー、capability 許可リスト、presentation 入力検証）。

## Consequences

- Positive: PCM 契約不変、既存転写パイプラインへの差分最小、未調整時は `transcribe-volume-normalize` と等価 loudness を維持
- Negative / trade-offs: `PcmIngestConsumer` の責務が増える（ゲイン + メーター集約）。所有は capture-audio-controls が明示する

## Alternatives considered

- **ingest 後にマイク成分を減算** — ミキサー出力は既に合成済みで不可
- **OS マイクミュート** — 要件外、他アプリへ影響
- **固定ゲインにユーザー乗算** — 未調整時に 1.25×1.25 となり要件 3.6 違反

## Notes

- 再キャプチャ（デバイス変更）時は `CaptureAudioControlsStore` をリセットしない（要件 4.2）
