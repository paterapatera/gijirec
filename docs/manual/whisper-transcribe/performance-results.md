# whisper-transcribe 性能検証・手動テスト結果

10 分連続転写の実測記録および手動チェックリスト（要件 **3.2, 7.1, 7.2**、E2E: 3 s 発話 → 5 s 以内ブロック表示）。数値の捏造は禁止 — 実機計測後にのみ「合格 / 不合格」を記入する。

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
| 使用モデル | kotoba-whisper-v2.2-ggml-q5_0.bin |

| 基準 | 測定値 | 合格 |
|------|--------|------|
| 10 分連続転写 `transcribe_inference_latency_ms` p95 | （ms） | yes / no |
| 追加 CPU 平均 / ピーク (%) | （% / %） | yes / no |
| 常駐メモリ増分 (MB) | （MB） | yes / no |
| Web 会議アプリ並行音声途切れ | （なし / あり） | yes / no |
| 3 s 発話 → 5 s 以内ブロック表示 | （秒） | yes / no |

備考: （逸脱時は原因と対策）
```

---

## transcribe-segment-timing ベースライン（調整前）

| 項目 | 値 |
|------|-----|
| **記録日** | 2026-09-08 |
| **実行者** | SDD 実装エージェント |
| **spec** | `transcribe-segment-timing` |
| **TRAILING_SILENCE_FRAMES** | 5（500 ms @ 100 ms/frame） |
| **LONG_SILENCE_FRAMES** | 12（1.2 s @ 100 ms/frame） |
| **MIN_SPEECH_SAMPLES** | 16_000（1 s @ 16 kHz） |
| **FRAME_SAMPLES** | 1_600（100 ms @ 16 kHz） |

### 区切り待ち時間ベースライン（実機計測）

| 試行 | 発話区切り → block-appended（ms） |
|------|-----------------------------------|
| 1 | **未計測** |
| 2 | **未計測** |
| 3 | **未計測** |
| **中央値** | **未計測** |

> 実機計測は CI / エージェント環境では音声ハードウェアが利用不可のため未実施。開発者が 3 秒連続日本語発話で 3 回計測し中央値を追記する。

---

## transcribe-segment-timing 第一軸調整後（2026-09-08）

| 項目 | 旧値 | 新値 |
|------|------|------|
| **TRAILING_SILENCE_FRAMES** | 5（500 ms） | **3（300 ms）** |
| **LONG_SILENCE_FRAMES** | 12（1.2 s） | 12（変更なし） |
| **調整軸** | — | 第一軸のみ（`TRAILING_SILENCE_FRAMES`） |

### 自動検証結果

| 検証 | 結果 |
|------|------|
| `bun run verify` | **pass**（exit 0） |
| `cargo test -p gijirec-infrastructure transcribe` | **pass**（45 passed, 3 ignored） |
| trailing silence cut テスト | **pass**（期待値を定数参照に更新） |
| long-pause short speech テスト | **pass**（定数参照のため自動適合） |

### 実機品質確認（未実施 — 開発者追記待ち）

| 基準 | 判定 | 備考 |
|------|------|------|
| 3 秒連続日本語発話で過剰分割なし | **未確認** | タスク 3.1 |
| 区切り待ち 50% 以上短縮 | **未確認** | ベースライン中央値との比較が必要 |
| 同一フレーズ 3 回連続出現なし | **未確認** | タスク 3.1 |
| `LONG_SILENCE_FRAMES` 第二軸調整 | **未実施** | 3.1 で 50% 未達の場合のみ（タスク 3.2） |
| whisper-transcribe-blocks 契約手動確認 | **未確認** | 空ブロックなし・追記のみ・5 秒遅延目標 |

関連: [docs/steering/testing.md](../../steering/testing.md)（リリース vs dev パリティ）、[docs/manual/README.md](../README.md)
