# Research & Design Decisions: capture-audio-controls

## Summary
- **Feature**: capture-audio-controls
- **Discovery Scope**: Brownfield extension（`audio-device-selection` + `transcribe-volume-normalize`）
- **Key Findings**:
  - マイク OFF は `capture_processing.rs` で `push_mic` をゲートするのが最小差分。ミキサーは単一トラックでも出力可能（`can_emit_at` が `mic_active || sys_active` を許容）
  - 手動ゲインは `PcmIngestConsumer` の固定 `TRANSCRIBE_INGEST_GAIN` をセッション乗数に置換し、ソフトリミット 0.95 は維持（`transcribe-volume-normalize` の実質的な上書き）
  - dBFS メーターは ingest 後 RMS を 1 Hz で Tauri イベント配信。生 PCM はフロントへ送らない

## Gap Analysis（Step 2.0）

### Requirement-to-Asset Map

| 要件 | 既存資産 | ギャップ |
|------|----------|----------|
| 1.1–1.4 マイク ON/OFF | `DefaultAudioMixer`、`capture_processing` | **Missing** — ingest ミックスからマイク除外フラグなし |
| 1.5 音声源なし | `audio-capture://error` | **Missing** — mic OFF + system 無効の専用コード |
| 1.6 非キャプチャ時 UI 無効 | `useCaptureStatus` | **Constraint** — 既存 phase パターンを再利用 |
| 2.1–2.5 dBFS 表示 | `PcmIngestConsumer` chunk RMS、`rms_to_dbfs` | **Missing** — フロント向けメタデータ IPC・1 Hz 配信 |
| 3.1–3.6 手動ゲイン | `pcm_ingest_consumer.rs` 固定 ×1.25 | **Missing** — 動的ゲイン・ユーザー未調整フラグ |
| 3.4 クリッピング防止 | ソフトリミット 0.95 既存 | **Unknown** — 上下限具体値は設計で決定 |
| 4.1–4.5 UI 整合 | `DeviceSelectorPanel` | **Missing** — トグル・メーター・スライダー・横並び |
| 4.2 再開時状態保持 | `DeviceSelectionService` セッション状態 | **Missing** — 制御状態ストアと再キャプチャ連携 |
| 5.1–5.3 性能 | `PcmChunkBus` バックプレッシャー | **Constraint** — メーターは集約配信で負荷抑制 |

### Implementation Options

| Option | 概要 | 採否 |
|--------|------|------|
| A: 既存拡張 | `DeviceSelectorPanel` + `PcmIngestConsumer` + `capture_processing` ゲート | **推奨** |
| B: 新ミキサー | speaker-only ミキサー実装を分離 | 過剰 — 既存 `can_emit_at` で単一トラック対応済み |
| C: フロント側ゲイン | Web Audio で増幅 | 要件違反 — ingest 直前は Rust 側 |

### Effort / Risk
- **Effort**: M（3–7 日）— 既存 IPC・ingest パターン踏襲、UI 追加が主
- **Risk**: Low–Medium — 境界は明確。リスクは再キャプチャ時の状態同期とメーター負荷（1 Hz 集約で低減）

## Research Log

### Mic OFF の適用位置
- **Context**: 要件は OS ミュートではなく ingest ミックス除外
- **Sources**: `mixer.rs` `emit_next_sample` / `can_emit_at`、`capture_processing.rs` `push_mic`
- **Findings**: ミキサー出力は `PcmChunkBus` → `PcmIngestConsumer` へ。マイク除外はキャプチャ処理で `push_mic` をスキップするのが PCM 契約を変えず最小
- **Implications**: `audio-capture-pcm.md` は変更なし。`CaptureAudioControlsStore` を processing スレッドが読む

### transcribe-volume-normalize 置換方針
- **Context**: 要件 3.6 と requirements review の残リスク
- **Sources**: `pcm_ingest_consumer.rs`、`transcribe-volume-normalize` 完了 spec
- **Findings**: 固定定数は単一箇所。動的 `ingest_gain_multiplier`（既定 1.25）+ 既存 `soft_limit(0.95)` で等価性を保証可能
- **Implications**: `TRANSCRIBE_INGEST_GAIN` 定数は削除しセッション状態へ。未調整時は 1.25 固定

### dBFS メーター配信
- **Context**: 要件 2.2（≥1 Hz）、2.5（生 PCM 禁止）
- **Sources**: `transcribe_observability.rs` `rms_to_dbfs`、`on_pcm_rms` callback
- **Findings**: chunk 単位 RMS は既存。1 秒窓で集約しイベント emit がバックプレッシャー最小
- **Implications**: 新イベント `capture-audio-controls://ingest-level`、キャプチャ中かつ ingest 有効時のみ

### IPC 信頼モデル
- **Context**: Sec deferred — audio-device-selection 同等
- **Sources**: `audio-device-selection.md`、`security.md`
- **Findings**: ローカル単一ユーザー、Tauri capability 許可リスト、presentation 層で入力検証
- **Implications**: 新 permission ファイル、gain 範囲 clamp、NaN/Inf 拒否

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks | Notes |
|--------|-------------|-----------|-------|-------|
| Session store + IPC | `DeviceSelectionStore` 同型のセッション状態 | 既存パターン一致、再起動で既定復帰 | 再キャプチャ同期が必要 | **採用** |
| 永続化 JSON | transcribe-settings 型 | 再起動後も維持 | 要件外、スコープ膨張 | 不採用 |
| PcmChunk 分岐 | mic/system 別バス | 細粒度制御 | PCM 契約変更、下流影響大 | 不採用 |

## Design Decisions

### Decision: Mic ingest gating at capture processing
- **Context**: 要件 1.2、OS ミュートとの境界
- **Alternatives**: OS ミュート、ingest 後ゼロ埋め、別ミキサー
- **Selected**: `mic_ingest_enabled == false` 時 `capture_processing` で `push_mic` をスキップ
- **Rationale**: ミキサーは system-only 出力を既にサポート。PCM 形状不変
- **Trade-offs**: マイクハードウェアは稼働継続（キャプチャパイプラインは二重取得のまま）— 要件どおり
- **Follow-up**: mic OFF + system 無効時の `TRANSCRIBE_INGEST_NO_AUDIO_SOURCE` 検知

### Decision: Replace fixed ingest gain with session multiplier
- **Context**: 要件 3.6、`transcribe-volume-normalize` 連続性
- **Alternatives**: 乗算オーバーレイ（1.25 × user）、別 DSP チェーン
- **Selected**: `PcmIngestConsumer` の単一乗数（既定 1.25、`gain_user_adjusted` フラグ付き）
- **Rationale**: 既存テスト・性能結果をそのまま維持。実装差分最小
- **Trade-offs**: `transcribe-volume-normalize` の定数は設計上 supersede（ADR-0014）
- **Follow-up**: 既存ユニットテストを動的ゲインに更新

### Decision: Gain limits 0.25–4.0 with soft limit 0.95
- **Context**: 要件 3.4（クリッピング防止）
- **Alternatives**: dB スライダー、無制限ゲイン
- **Selected**: 線形乗数 0.25–4.0、常時ソフトリミット 0.95、境界で UI フィードバック
- **Rationale**: ミキサー `MAX_GAIN = 4.0` と整合。過大増幅を抑止しつつ −18〜−17 dBFS 調整余地を残す
- **Trade-offs**: 極端な入力ではソフトリミット後も歪みうる — 上限で実用上十分
- **Follow-up**: スライダー端での `aria-live` ヒント

### Decision: Generalization — CaptureAudioControls aggregate
- **Context**: マイクトグル・ゲイン・メーターは同一セッション状態
- **Selected**: 単一 `CaptureAudioControls` 型と `CaptureAudioControlsService`
- **Rationale**: device selection と同型の IPC・再キャプチャ連携を一括管理

## Risks & Mitigations
- 再キャプチャで mic/gain 状態がリセット — `DeviceSelectionService.restart` 前後で store を保持（要件 4.2）
- メーター更新が UI を圧迫 — 1 Hz 集約、キャプチャ外は emit 停止（要件 5.2）
- rtrb バックプレッシャー — ingest パスは既存と同じ。メーターは callback 内集計のみ

## References
- `docs/contracts/audio-device-selection.md` — IPC パターン
- `docs/contracts/whisper-transcribe-settings.md` — セッション/永続化の分離参考
- `src-tauri/crates/gijirec-presentation/src/transcribe/pcm_ingest_consumer.rs` — ingest ゲイン実装
- `src-tauri/src/capture_processing.rs` — ミキサー結線
- `docs/specs/capture-audio-controls/reviews/requirements-review.md` — Phase Gate GO
