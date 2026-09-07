# audio-capture-pcm

- **Surface type**: Data ownership / Event
- **Owners / Domains**: audio-capture
- **Related ADR**: docs/architecture/adr/ADR-0001-platform-audio-capture.md

## Purpose

gijirec Audio Capture が下流（whisper-transcribe 等）へ供給する正規化済み PCM チャンクの形状と供給規約を定義する。

## Contract

### PcmChunk（論理ペイロード）

| フィールド | 型 | 制約 |
|-----------|-----|------|
| `sequence` | `u64` | キャプチャ開始から単調増加。欠番なし |
| `sample_rate_hz` | `u32` | 固定 `16000` |
| `channels` | `u8` | 固定 `1`（モノラル） |
| `sample_format` | enum | 固定 `Int16Le` |
| `samples` | `i16[]` | 長さは `frame_count` と一致 |
| `frame_count` | `u32` | `samples.len()` |
| `timestamp_ms` | `u64` | キャプチャ開始基準の経過ミリ秒（`sequence` と整合） |

### 供給規約

| 項目 | 値 |
|------|-----|
| チャンク長 | **100 ms**（1600 サンプル @ 16 kHz） |
| 供給間隔 | 概ね 100 ms（ジッター ±20 ms 許容） |
| エンディアン | リトルエンディアン |
| バイト列サイズ | `frame_count * 2` |
| 停止時 | 部分チャンクは破棄。停止後に新規チャンクを発行しない |
| デバイス再選択 | `ChunkEmitter` を再生成せず `discard_partial_buffer` のみ行い `sequence` を継続する |

### 消費側インターフェース（Rust 内部）

```rust
/// 下流 crate が実装する同期コールバックまたは async channel 受信側。
pub trait PcmChunkConsumer: Send + Sync {
    fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError>;
}
```

登録は `gijirec-presentation` の composition root 経由。複数消費者は将来拡張とし、v1 は単一消費者（whisper-transcribe 用バス）を想定。

### Tauri イベント（デバッグ／将来 UI 用・v1 非必須）

| イベント名 | ペイロード | 備考 |
|-----------|-----------|------|
| `audio-capture://pcm-chunk` | **発行しない** | v1 は Rust 内部バスのみ。フロントへ PCM を送らない（7.2） |

## Non-goals

- Whisper 推論入力バッファの管理（whisper-transcribe の責務）
- 音声ファイルへの永続書き込み
- ネットワーク経由の PCM 送信

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-07 | デバイス再選択時の `sequence` 継続（`discard_partial_buffer`）を供給規約に追記 | audio-device-selection 完了昇格 |

## Notes

- ミキシング前の生ストリーム形状は本契約の対象外（infrastructure 内部）
- チャンク長変更は下流 whisper-transcribe の Revalidation Trigger
