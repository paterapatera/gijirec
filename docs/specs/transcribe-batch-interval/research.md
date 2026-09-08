# Research & Design Decisions: transcribe-batch-interval

## Summary
- **Feature**: transcribe-batch-interval
- **Discovery Scope**: Brownfield extension（whisper-transcribe 推論スケジュール変更）
- **Key Findings**:
  - v1 実装は worktree 上の `src-tauri/` に存在。[Explore transcribe codebase gap](6435dd2d-b0e3-4fba-b30b-8b00bed24f83) により実コードパス・バッファ・ドロップ動作を確認
  - 現行は `PcmChunkBus` → `PcmIngestConsumer` → rtrb（480k = 30 s）→ `TranscribeWorker`（VAD + skip-to-latest + drop-oldest）→ `WhisperCppAdapter` → `BlockEmitter` → `TranscriptBlockBus`
  - 30 秒バッチ化の主戦場は `gijirec-infrastructure/.../transcribe_worker.rs` の窓切り出しと多層ドロップ除去。adapter / 下流はほぼ再利用
  - Req 2 の最大ギャップ: `skip-to-latest`（`take_window_from_state:417-422`）、`drain_consumer` の drop-oldest、`PcmChunkBus` 3 チャンク上限、rtrb 満杯時の ingest 失敗

## Research Log

### Step 2.0 Gap Analysis（brownfield）

- **Context**: 要件 1–6 が既存 whisper-transcribe 領域のスケジュール・バッファ設計変更を要求。`brief.md` Current State は v1 完了を明示
- **Sources Consulted**: 上記 + `src-tauri/crates/gijirec-infrastructure/src/transcribe/transcribe_worker.rs`、`whisper_adapter.rs`、`src-tauri/crates/gijirec-presentation/src/tauri/pcm_bus.rs`、`pcm_ingest_consumer.rs`、`src-tauri/src/compose.rs`、`block_emitter.rs`、`transcript_block_bus.rs`
- **Findings**:
  - **PATHS（実コード）**:
    - `gijirec-infrastructure/transcribe/transcribe_worker.rs` — VAD endpointing、`take_inference_window`、worker/drain スレッド
    - `gijirec-infrastructure/transcribe/whisper_adapter.rs` — `transcribe_pcm`（スケジューリングなし）
    - `gijirec-presentation/transcribe/pcm_ingest_consumer.rs` — rtrb push
    - `gijirec-presentation/src/tauri/pcm_bus.rs` — `MAX_QUEUED_CHUNKS = 3`
    - `gijirec-application/transcribe/block_emitter.rs` — sequence / 空スキップ
    - `gijirec-presentation/transcribe/transcript_block_bus.rs` — `block-appended`
    - `gijirec-presentation/transcribe/lifecycle_hook.rs` — キャプチャ連動停止
    - `src-tauri/src/compose.rs` — rtrb `480_000` 結線
    - `src/presentation/hooks/useTranscriptBlocks.ts` — フロント購読（変更不要）
  - **現行 VAD 定数**: max window **10 s**（160k samples）、buffer **20 s**（320k）、trailing silence 1.2 s / long 4.8 s
  - **意図的ドロップ（Req 2 と矛盾）**:
    - `PcmChunkBus`: 3 チャンク超で最古 drop
    - rtrb 満杯: ingest `Internal("rtrb buffer full")`
    - `drain_consumer`: `MAX_PCM_BUFFER_SAMPLES` 超で drop-oldest
    - `skip-to-latest`: 2× max window 超で最古 PCM 破棄
- **Implications**: 新規 crate モジュールより **`transcribe_worker.rs` 内リファクタ（Option A）** が最小差分。`WhisperCppAdapter` は 30 s 窓対応済み（`audio_ctx_for_pcm(480_000) == 1500`）

#### Requirement-to-Asset Map

| Req | 既存アセット | ギャップ |
|-----|-------------|---------|
| 1.1–1.4 | `transcribe_worker.rs` `take_inference_window` | **Missing** — 30 s 固定バッチ境界・サイクル完了起点タイマー |
| 2.1–2.4 | rtrb 480k、`PcmChunkBus`×3、skip-to-latest、drop-oldest | **Constraint** — 4 層の意図的 drop を除去・容量拡張が必要 |
| 3.1–3.5 | `BlockEmitter`, `TranscriptBlockBus`, `useTranscriptBlocks` | **Constraint** — 形状維持。`start_timestamp_ms` はバッチ窓起点へ |
| 4.1–4.3 | `lifecycle_hook.rs`, `orchestrator.rs`, `stall_watchdog.rs` | **Constraint** — ライフサイクル再利用。stall 閾値は要再調整 |
| 5.1–5.4 | — | スコープ外（変更禁止） |
| 6.1–6.3 | `transcribe_integration.rs` | **Missing** — 10 分 / backlog / timestamp 手動＋統合テスト |

#### Options

| Option | 概要 |
|--------|------|
| **A（推奨）** | `transcribe_worker.rs` — VAD `take_window_from_state` を 30 s バッチ境界に置換。skip-to-latest / drop-oldest 除去 |
| **B** | 新規 `BatchTranscribeWorker` — compose で差し替え。重複大 |
| **C** | `SchedulingMode` enum で VAD / Batch 共存 — rollback 用。初版は A で十分 |

- **EFFORT**: **M–L** — worker スケジュール + 3 層バッファ政策 + compose 定数 + 統合テスト
- **RISK**: **Medium–High** — v1 低遅延向け drop が深く組み込まれている。バックログ深さの過小見積が Req 2/6 失敗要因

### Step 2.1 Light Discovery — whisper-cpp-plus バッチ API

- **Context**: VAD ストリーミング（`WhisperStreamPcm`）から固定窓バッチへ切替時のアダプタ API
- **Sources Consulted**: ADR-0003、`tech.md`（whisper-cpp-plus 0.1 ピン留め）
- **Findings**:
  - ライブラリ選択（whisper-cpp-plus）は維持。変更は呼び出しモード（ストリーム → 窓単位 full transcribe）
  - モデル・スレッド上限（4）は変更しない（要件 5.2）
  - バッチ窓内の複数セグメントは adapter が返し、`BlockEmitter` が各セグメントを個別ブロック化可能
- **Implications**: infrastructure 層に `transcribe_pcm_window(samples, start_ms)` を追加。VAD による無音スキップはバッチ窓レベルで「テキスト空ならブロック不発行」（要件 3.4）に置換

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| A: Worker refactor | `transcribe_worker.rs` 内でスケジュール + バッファ修正 | 最小 diff、既存 drain/inference 再利用 | 単ファイルが肥大化しうる | **採用** |
| B: New worker module | 並行 worker | rollback 容易 | 重複・compose 複雑化 | 不採用 |
| C: Mode flag | VAD/Batch 共存 | A/B 切替 | 初版スコープ外 | 将来検討 |

## Design Decisions

### Decision: 30 秒固定バッチスケジュール（サイクル完了起点）

- **Context**: 要件 1.1 — 前回サイクル完了後約 30 秒で推論
- **Alternatives Considered**:
  1. キャプチャ開始からの壁時計 30 s — 推論遅延で窓が重なる
  2. VAD 区間 + 最大 30 s キャップ — スコープ外の複雑化
- **Selected Approach**: `transcribe_worker.rs` の `worker_loop` が前サイクル完了時刻を記録し、未処理 PCM ≥ 480_000 samples（30 s @ 16 kHz）かつ前サイクル完了から 30 s 経過後に `take_batch_window` を実行。バックログ時は連続サイクル（要件 6.3）
- **Rationale**: 要件 1.3（前サイクル以降の音声を含む）と CPU 安定性を両立
- **Trade-offs**: リアルタイム性は最大 30 s + 推論時間に低下。完全性・安定性を優先（要件 1.4）
- **Follow-up**: 実機で推論時間 > 30 s の場合のバックログ挙動を手動検証

### Decision: PCM 非破棄 — v1 ドロップ経路の除去と容量拡張

- **Context**: 要件 2.1–2.2。v1 は 4 層で意図的 drop
- **Selected Approach**:
  1. **除去**: `skip-to-latest`（`take_window_from_state`）、`drain_consumer` drop-oldest
  2. **拡張**: `MAX_PCM_BUFFER_SAMPLES` を 10 分相当以上（実機でキャリブレーション）、rtrb を compose で拡張検討
  3. **PcmChunkBus**: 長時間推論中 rtrb ブロック時の 3 チャンク cap を引き上げ（Research Needed）
- **Follow-up**: 最悪推論時間 × 10 分キャプチャで必要 rtrb + VecDeque 深さを実機計測

### Decision: 公開契約は形状維持・遅延目標のみ更新

- **Context**: 要件 3.1、既存 transcript-editor との整合
- **Selected Approach**: `whisper-transcribe://block-appended` ペイロード形状は変更しない。`whisper-transcribe-blocks.md` の遅延目標をバッチ方式に合わせて更新
- **Rationale**: 下流 Revalidation Trigger を最小化

## Risks & Mitigations

- **PCM バッファ溢れ** — 実機で 30 s 推論 worst-case を計測し rtrb / VecDeque / PcmChunkBus cap を決定
- **PcmChunkBus 3 チャンク cap** — 推論 > 300 ms で bus drop 発生しうる。cap 引き上げ or ingest ブロック戦略を実装時決定
- **stall_watchdog** — `INFERENCE_TIMEOUT=600s` が 30 s 窓に適合するか実機確認

## References

- `docs/architecture/boundaries.md` — whisper-transcribe 境界
- `docs/contracts/whisper-transcribe-blocks.md` — ブロック供給契約
- `docs/architecture/adr/ADR-0003-whisper-cpp-plus-streaming.md` — 現行ストリーミング判断（ライブラリ選択は維持）
- `docs/architecture/adr/ADR-0012-batch-inference-schedule.md` — 本 feature のスケジュール判断（新規）
