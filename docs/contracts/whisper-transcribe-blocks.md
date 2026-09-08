# whisper-transcribe-blocks

- **Surface type**: Event / Data ownership
- **Owners / Domains**: whisper-transcribe
- **Related ADR**: docs/architecture/adr/ADR-0003-whisper-cpp-plus-streaming.md

## Purpose

gijirec Whisper Transcribe が下流（transcript-editor 等）へ供給するタイムスタンプ付きテキストブロックの形状と供給規約を定義する。

## Contract

### TranscriptBlock（論理ペイロード）

| フィールド | 型 | 制約 |
|-----------|-----|------|
| `block_id` | `string` | UUID v4。同一ブロックの再送時も同一 ID を維持 |
| `sequence` | `u64` | キャプチャセッション内で単調増加（欠番なし） |
| `text` | `string` | UTF-8。空文字列は発行しない |
| `start_timestamp_ms` | `u64` | キャプチャ開始基準の経過ミリ秒。対応 PCM の開始時刻と整合 |
| `language` | `string` | 推論で検出された言語コード（例: `ja`, `en`）。未検出時は `und` |

### 供給規約

| 項目 | 値 |
|------|-----|
| 供給方式 | **追記のみ** — 既発行ブロックの内容変更・撤回禁止 |
| 無音区間 | テキストブロックを発行しない |
| タイムスタンプ基準 | 上流 `PcmChunk.timestamp_ms` および `audio-capture-pcm` の時刻基準と整合 |
| 遅延目標 | バッチ窓終了から当該窓のブロックを **次推論サイクル完了まで** 供給。固定バッチ間隔は **30 秒**（前サイクル完了起点）。初版は間隔設定 UI なし |
| 保持 | メモリ上のリングバッファ最大 **500** ブロック（下流未消費時）。超過時は最古を破棄しメトリクス記録（転写テキストのディスク永続化は行わない） |

### Tauri イベント

#### `whisper-transcribe://block-appended`

```typescript
interface TranscriptBlockAppended {
  block: {
    block_id: string;
    sequence: number;
    text: string;
    start_timestamp_ms: number;
    language: string;
  };
  timestamp_ms: number; // イベント発行時刻（キャプチャ開始基準）
}
```

### 消費側インターフェース（Rust 内部）

```rust
/// 下流 crate / モジュールが実装する同期コールバック。
pub trait TranscriptBlockConsumer: Send + Sync {
    fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError>;
}
```

登録は `gijirec-presentation` の composition root 経由。v1 は単一消費者（transcript-editor 用バス）を想定。

### 禁止事項

- ブロック内容の事後変更・撤回（要件 3.5）
- 転写テキストの外部ネットワーク送信
- ユーザー明示操作なしのディスク永続化

## Non-goals

- 手動編集状態・部分ロック（transcript-editor の責務）
- Markdown 出力
- 話者ラベル付与

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-08 | 遅延目標を 30 秒固定バッチ方式に更新（VAD ストリーミング遅延目標を置換） | ADR-0012 |
| 2026-09-05 | 初版 — TranscriptBlock 形状・追記供給・Tauri イベント | ADR-0003 |

## Notes

- `block_id` は下流の差分同期キー。`sequence` は表示順序の正本
- ブロック形状変更は transcript-editor の Revalidation Trigger
