# whisper-transcribe 性能検証・手動テスト結果

設計 [Performance/Load](design.md) 項目 1–4、要件 **3.2, 7.1, 7.2**、および実音声 E2E（設計 E2E 5）。
10 分連続転写の実測記録および手動チェックリストを記録する。数値の捏造は禁止 — 実機計測後にのみ「合格 / 不合格」を記入する。

## 最新記録

| 項目 | 値 |
|------|-----|
| **記録日** | 2026-09-05 |
| **実行者** | CI / エージェント（Wave 23 タスク 8.4） |
| **結果** | **not executed in CI**（CI では未実施） |
| **環境** | Windows エージェント / CI（音声ハードウェア・実音声・10 分連続転写負荷計測不可） |
| **ビルド** | —（長時間計測未実行） |
| **OS / CPU / RAM** | — |

### 逸脱理由と対策

| 基準 | 判定 | 逸脱理由 | 対策 |
|------|------|----------|------|
| Performance/Load 1: 10 分連続転写 `transcribe_inference_latency_ms` p95 < 5000 ms (3.2) | **未計測** | CI / エージェント環境では実音声入力および長時間連続推論負荷計測が不可 | 開発者が実機でモデル取得済み環境にて 10 分間実音声を流し、ログの `transcribe_inference_latency_ms` の p95 値を記録する |
| Performance/Load 2: キャプチャ + 転写同時で追加 CPU 平均 < 25%（4 コア基準）・ピーク < 50% (7.2) | **未計測** | 同上 | タスクマネージャー / WPR（Windows）または Activity Monitor / Instruments（macOS）でプロファイル計測を実施 |
| Performance/Load 3: 常駐メモリ増分 < 400 MB（モデルロード後） (7.2) | **未計測** | 同上 | アプリ起動直後（Idle）とモデルロード後・10 分転写後のメモリ消費増分を記録 |
| Performance/Load 4: Web 会議アプリ並行で音声途切れなし (7.1) | **未計測** | 同上 | Teams / Zoom / Google Meet 等の通話中に文字起こしを同時実行し、音声品質低下がないことを主観 + 会議側ログで確認 |
| E2E 5 (手動): 3 s 発話 → 5 s 以内にブロック表示 (3.2) | **未計測** | 同上 | 実マイクから 3 秒間発話し、5 秒以内に UI 上に文字起こしテキストブロックが表示されるかを確認 |

### CI / 自動テストで継続している検証

実機長時間負荷の代替として、以下が自動テスト（Unit / Integration / UI）で保証されている:

| 種別 | 内容 |
|------|------|
| 単体 | `BlockEmitter`（単調 sequence, timestamp_ms 計算）、`PcmIngestConsumer`（非ブロッキング push）、`ModelStore`（SHA-256 検証） |
| 統合 | `transcribe_integration.rs`: 合成 PCM → ワーカー → ブロック emit、`PcmChunkBus` 経由のワーカー消費、キャプチャエラー連動、停止時ワーカースレッド完全 join |
| UI | `App.test.tsx`: `loading_model` + 進捗バー表示、`capturing` / `transcribing` 連動表示、エラー時 `action_ja` 表示 |
| リソース上限設計 | rtrb 30 秒（480,000 サンプル）上限、TranscriptBlockBus 500 件上限、PcmChunkBus 3 チャンク上限によりメモリ無制限増加を防止 |

---

## 実機計測記録テンプレート（実機計測後にコピーして追記）

```markdown
### 記録 YYYY-MM-DD（実機）

| 項目 | 値 |
|------|-----|
| 記録日 | YYYY-MM-DD |
| 実行者 | （名前） |
| 結果 | pass / fail |
| 環境 | Windows 11 / macOS 14.x 等 |
| ビルド | release / debug |
| OS / CPU / RAM | 例: Win11, 4C/8T, 16 GB |
| 使用モデル | ggml-small.bin (日本語/多言語) |

| 基準 | 測定値 | 合格 |
|------|--------|------|
| 10 分連続転写 `transcribe_inference_latency_ms` p95 | （ms） | yes / no |
| 追加 CPU 平均 / ピーク (%) | （% / %） | yes / no |
| 常駐メモリ増分 (MB) | （MB） | yes / no |
| Web 会議アプリ並行音声途切れ | （なし / あり） | yes / no |
| 3 s 発話 → 5 s 以内ブロック表示 | （秒） | yes / no |

備考: （逸脱時は原因と対策）
```
