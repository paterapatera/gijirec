# ADR-0001: プラットフォーム別システム音声キャプチャ

- **Status**: Accepted
- **Date**: 2026-09-05
- **Feature**: audio-capture
- **Owners / Domains**: audio-capture

## Context

要件 1 は仮想オーディオデバイスなしでマイクとシステム音声を同時取得することを義務付ける。Mac / Windows で API が異なり、cpal 単体では macOS ループバックをカバーしない。

## Decision

| OS | マイク | システム音声 |
|----|--------|-------------|
| Windows | cpal 入力ストリーム（既定入力デバイス） | cpal で既定**出力**デバイスに入力ストリームを構築（WASAPI `AUDCLNT_STREAMFLAGS_LOOPBACK` 自動適用） |
| macOS | cpal 入力ストリーム（既定入力デバイス） | `screencapturekit` crate 経由の ScreenCaptureKit オーディオ（2×2 px / 1 fps のダミー映像を破棄、`capturesAudio = true`, `excludesCurrentProcessAudio = true`） |

両ストリームは infrastructure 層で取得し、application 層の `AudioMixer` で単一タイムラインに整列・ミックスする。下流へは `docs/contracts/audio-capture-pcm.md` の形状で供給する。

## Consequences

- Positive: 仮想デバイス不要。steering の技術方針と一致
- Negative / trade-offs: macOS は画面収録権限が必要（マイク権限に加え）。二系統のクロック差により整列バッファ（設計で ~50 ms）が必要。Windows は既定出力ミックス全体のみ取得（アプリ単位の分離は v1 対象外）

## Alternatives considered

1. **tauri-plugin-system-audio 採用** — Windows のみ完全。macOS ループバック未対応のため不採用
2. **macOS Core Audio TAP** — 低遅延だが HFP / クロック停止で無音化する報告が多く、長時間会議向けに ScreenCaptureKit を選択
3. **仮想デバイス（BlackHole 等）** — 要件 1.4 で明示除外

## Notes

- Linux はサポート対象外（要件 6.3）
- 依存方向: infrastructure → 外部 crate / OS API のみ。domain は OS 非依存
