# audio-capture 性能テスト結果

30 分連続キャプチャの実測記録（要件 **4.2**）。数値の捏造は禁止 — 実測後にのみ「合格 / 不合格」を記入する。

## 最新記録

| 項目 | 値 |
|------|-----|
| **記録日** | 2026-09-05 |
| **実行者** | CI / エージェント（Wave 36 タスク 10.2 ドキュメント整備） |
| **結果** | **not executed in CI**（CI では未実施） |
| **環境** | Windows エージェント / CI（音声ハードウェア・30 分常時キャプチャ不可） |
| **ビルド** | —（長時間計測未実行） |
| **OS / CPU / RAM** | — |

### 逸脱理由と対策

| 基準 | 判定 | 逸脱理由 | 対策 |
|------|------|----------|------|
| Performance/Load 1: 30 分 `capture_buffer_drops_total == 0` | **未計測** | CI / エージェント環境では 30 分の実音声キャプチャ実測不可（マイク・ループバック / SCK デバイス不在、無人長時間実行不可） | 開発者が Windows / macOS 実機で [README 性能テスト手順](../../../README.md#性能テスト手動) に従い 30 分計測後、下記テンプレート行を更新する |
| Performance/Load 2: CPU 平均 < 5%、ピーク < 15% | **未計測** | 同上 | WPR（Windows）または Instruments（macOS）でプロファイルし結果を記録 |
| Performance/Load 3: 常駐メモリ増分 < 50 MB | **未計測** | 同上 | 同上 |

### CI で継続している自動検証

長時間負荷の代替として、以下は PR / CI で実行される（合格 ≠ 30 分性能合格）:

| 種別 | 内容 |
|------|------|
| 単体 | `AudioMixer` / `ChunkEmitter` / `CaptureOrchestrator` / `UserFacingError` |
| 統合 | `PcmChunkBus`（100 ms チャンク・ドロップ記録）、ライフサイクル、合成 rtrb 処理スレッド |
| 観測性 | `capture_buffer_drops_total` を WARN ログで出力（7.4）。実機 30 分後にログ件数 0 を確認 |

---

## 実測記録テンプレート（実機計測後にコピーして追記）

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

| 基準 | 測定値 | 合格 |
|------|--------|------|
| 30 分 `capture_buffer_drops_total` | （ログ WARN 件数 or 0） | yes / no |
| CPU 平均 / ピーク (%) | （WPR / Instruments） | yes / no |
| メモリ増分 (MB) | （開始 idle → 30 分後） | yes / no |

備考: （逸脱時は原因と対策）
```

### ドロップ確認チェックリスト

1. `RUST_LOG=gijirec_capture=info` で起動。
2. 30 分間 `capturing` を維持。
3. 終了後、ログに `pcm chunk bus dropped` および `capture_buffer_drops_total` > 0 の WARN が **ない** こと。
4. 下流 `PcmChunkConsumer` を v1 では登録しない構成の場合、publish のみではドロップが増えうる — 性能試験は **本番同等**（consumer 未登録の dev スモークではない）で行う。

### 関連ドキュメント

- [README — 性能テスト（手動）](../../../README.md#性能テスト手動)
- [docs/manual/README.md](../README.md)
