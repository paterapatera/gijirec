# audio-device-selection 性能検証結果

要件 **5.3**（選択変更 → `capturing` 復帰 **< 2 s**）。計測は `device_selection_restart_duration_ms`（tracing / observability）および統合テストの wall-clock で記録する。数値の捏造は禁止 — テスト実行後の実測値のみ記載する。

## 最新記録

| 項目 | 値 |
|------|-----|
| **記録日** | 2026-09-06 |
| **実行者** | CI / エージェント（Wave 26 タスク 9.5） |
| **結果** | **pass（合成 mock ポート自動テスト）** / **実機は未計測** |
| **環境** | Windows エージェント、`cargo test` debug ビルド |
| **テスト** | `device_selection_performance.rs` — `performance_selection_restart_under_two_seconds_mock_ports` |
| **ビルド** | debug (`target-integ`) |
| **OS / CPU / RAM** | Windows 10 (26200)、合成 rtrb ポート（ハードウェアなし） |

### Performance/Load 1 — 選択変更 → capturing 復帰（req 5.3）

| 指標 | 測定値 | 合格基準 | 判定 |
|------|--------|----------|------|
| `device_selection_restart_duration_ms`（observability / tracing 相当） | **0 ms** | < 2000 ms | **pass** |
| wall-clock（`set_device_selection` → `Capturing` + processing 再起動） | **0 ms** | < 2000 ms | **pass** |

**実行ログ（`cargo test --test device_selection_performance -- --nocapture`）:**

```
PERF device_selection_restart_duration_ms=0 wall_elapsed_ms=0
```

### 解釈

- **合成スタック（`SyntheticMicPort` / `SyntheticSystemPort` + `CapturePipelineState`）** では OS デバイス I/O がないため、再キャプチャはサブミリ秒で完了する。これは **回帰検知用の下限スモーク** であり、2 s 予算に対する十分なマージンを自動 assert する。
- **実ハードウェア（cpal / WASAPI / SCK）** ではデバイス open/close レイテンシ・OS スケジューリング・release ビルド差により数 ms〜数百 ms まで変動しうる。**最終的な 5.3 合格判定は実機計測が推奨**（下記手動テンプレート）。mock テスト合格 ≠ 全環境での性能保証ではない。

### CI / 自動テストで継続している検証

| 種別 | 内容 |
|------|------|
| 性能（mock） | `device_selection_performance.rs`: 再キャプチャ < 2 s、`device_selection_restart_duration_ms` 記録 |
| 観測性 | `device_selection_observability.rs`: tracing に `device_selection_restart_duration_ms` フィールド |
| 単体 | `service.rs`: `set_selection_emits_observability_ids_and_restart_duration` |
| 統合 | `device_selection_integration.rs` Tests 1–4: 選択変更 → capturing 復帰・PCM sequence 連続 |

### 実機計測が必要な項目（手動）

| 基準 | 判定 | 理由 |
|------|------|------|
| Performance/Load 1（実機 cpal / SCK） | **未計測** | CI エージェントに音声デバイスなし。release ビルドでの実デバイス open 時間は mock と異なる |

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
| ビルド | release |
| OS / CPU / RAM | 例: Win11, 4C/8T, 16 GB |

| 基準 | 測定値 | 合格 |
|------|--------|------|
| 選択変更 → capturing 復帰 wall-clock (ms) | （ms） | yes / no |
| ログ `device_selection_restart_duration_ms` | （ms） | yes / no |

手順:
1. `RUST_LOG=info` で release ビルド起動。
2. キャプチャ開始（`capturing`）。
3. UI または invoke でマイク／スピーカーを別デバイスに変更。
4. フェーズが `capturing` に戻るまでの時間をストップウォッチまたはログの `device_selection_restart_duration_ms` で記録。
5. 2000 ms 未満であることを確認。

備考: （逸脱時は原因と対策）
```

### 関連

- `src-tauri/tests/device_selection_observability.rs`（tracing フィールド検証）
- [docs/steering/testing.md](../../steering/testing.md)
