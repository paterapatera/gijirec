# ADR-0009: macOS スピーカー選択と ScreenCaptureKit の整合

- **Status**: Accepted
- **Date**: 2026-09-06
- **Feature**: audio-device-selection
- **Owners / Domains**: audio-device-selection, audio-capture

## Context

要件 1–3 は利用者がスピーカー（ループバック対象）を明示選択し、そのデバイスでキャプチャすることを求める。既存 ADR-0001 は macOS システム音声を ScreenCaptureKit（SCK）で取得する。SCK はハードウェア出力デバイスを指定する API を持たず、システム全体のオーディオミックスを返す。

## Decision

| OS | スピーカー一覧 | ループバック取得 |
|----|----------------|------------------|
| Windows | cpal `output_devices()` | 選択した出力デバイスへ `build_input_stream`（WASAPI ループバック） |
| macOS | cpal `output_devices()`（表示・選択 UI 用） | SCK システムミックス。**選択 `speaker_id` が OS 既定出力と一致している場合のみ**キャプチャ開始を許可 |

不一致時は `set_device_selection` およびキャプチャ `starting` preflight で `MACOS_OUTPUT_NOT_DEFAULT` を返し、利用者にシステム設定での出力変更または既定出力の選択を案内する。サイレントに別デバイスへフォールバックしない（要件 3.4, 4.2）。

マイク選択は両 OS で cpal 入力デバイス ID 指定（ADR-0001 のマイク経路を拡張）。

## Consequences

- Positive: 仮想デバイス不要。既存 SCK アダプタを維持し PCM 契約を破壊しない
- Negative / trade-offs: macOS では「スピーカー選択」が OS 出力ルーティングと連動する。Windows より操作ステップが増える可能性

## Alternatives considered

1. **Core Audio デバイス単位 TAP** — 低遅延だが長時間安定性・実装コストが高く、ADR-0001 で SCK を選択済み
2. **選択を UI 表示のみ（実際は常に既定）** — 要件 3.4 違反（サイレント不一致）
3. **アプリが OS 既定出力をプログラム変更** — 利用者のシステム設定を上書きし UX・権限リスクが大きい

## Notes

- Linux はサポート対象外（要件 6.3）
- 将来 macOS がデバイス指定 SCK を提供した場合、新 ADR で本制約を見直す
