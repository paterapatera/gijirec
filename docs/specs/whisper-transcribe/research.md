# Research & Design Decisions: whisper-transcribe

## Summary
- **Feature**: whisper-transcribe
- **Discovery Scope**: New Feature（greenfield）— Full discovery（外部依存・ストリーミング STT パターン調査）
- **Key Findings**:
  - `whisper-cpp-plus`（whisper.cpp v1.8.6-stream-pcm ピン留め）は `WhisperStreamPcm` により 16 kHz PCM の逐次投入と VAD 駆動セグメント分割をネイティブサポートする
  - 上流 `PcmChunkBus` は v1 単一 consumer・最大 3 チャンク（~300 ms）のバックプレッシャー — 推論側は `on_pcm_chunk` を非ブロッキングに保つ必要がある
  - 下流 transcript-editor 向けには Tauri イベント＋ Rust 内部 `TranscriptBlockConsumer` トレイトの二層が、audio-capture の PCM 契約パターンと整合する

## Research Log

### whisper.cpp Rust バインディング選定
- **Context**: 要件 2（ローカル逐次・Python なし）と brief（whisper.cpp Rust バインディング）を満たすライブラリの比較
- **Sources Consulted**:
  - [whisper-cpp-plus (crates.io)](https://crates.io/crates/whisper-cpp-plus)
  - [operator-kit/whisper-cpp-plus-rs](https://github.com/operator-kit/whisper-cpp-plus-rs)
  - [yamabiko-whisper (docs.rs)](https://docs.rs/crate/yamabiko-whisper/latest)
- **Findings**:
  - `whisper-cpp-plus`: `WhisperStreamPcm` が stream-pcm.cpp の直接移植。固定ステップ（3–5 s）と Silero VAD 駆動の両モード。Metal / CUDA / OpenBLAS feature。`WhisperContext` は `Send + Sync`
  - `whisper-rs`（yamabiko 等の基盤）: 低レベル API。ストリーミングは自前実装が必要
  - `yamabiko-whisper`: LocalAgreement-2 + 必須 VAD。低遅延研究向きだが本 spec の 3–5 s ウィンドウ要件に対して過剰
- **Implications**: infrastructure 層に `whisper-cpp-plus` を採用。VAD 駆動で無音区間のブロック非生成（要件 3.3）を満たす

### ストリーミング遅延とウィンドウ設計
- **Context**: 要件 3.2（発話区間終了から 5 秒以内にテキスト供給）
- **Sources Consulted**: whisper-cpp-plus `WhisperStreamPcmConfig`（step_ms, length_ms, use_vad）
- **Findings**:
  - VAD 駆動モード: 発話終了（無音検出）で推論実行 → 区間終了から推論完了までが遅延の主因
  - 固定ステップ: `step_ms=3000` + overlap で最大 3 s 遅延、ただし無音でも空ブロックリスク
  - 推論は専用ワーカースレッドで実行し、PCM 受信は rtrb 経由で非ブロッキング化が必須
- **Implications**: デフォルト VAD 駆動 + 最大セグメント長 5 s キャップ。推論ワーカーは `PcmChunkConsumer::on_pcm_chunk` 外で動作

### モデル取得とオフライン運用
- **Context**: 要件 5（初回取得・オフライン推論）
- **Sources Consulted**: whisper.cpp 公式モデル配布（HuggingFace ggml）
- **Findings**:
  - `ggml-small.bin`（~466 MB）が多言語会議の精度/性能バランスとして妥当
  - 量子化版（`ggml-small-q5_0.bin`）でメモリ・CPU を ~30% 削減可能（要件 7 向け）
  - ダウンロードは HTTPS のみ。取得後はローカルパス参照のみ
- **Implications**: デフォルトモデル `ggml-small-q5_0.bin`。保存先は Tauri `app_data_dir/models/`

### 上流 PCM バス統合
- **Context**: 既存 `PcmChunkBus` 実装の制約確認
- **Sources Consulted**: `src-tauri/crates/gijirec-presentation/src/tauri/pcm_bus.rs`
- **Findings**:
  - `MAX_QUEUED_CHUNKS = 3`（~300 ms）。超過時は最古ドロップ
  - consumer 未登録時も publish 可能（ドロップ記録）
  - `PcmChunkConsumer::on_pcm_chunk` は同期呼び出し — 長時間ブロックはドロップを招く
- **Implications**: consumer 内は rtrb push のみ。推論は別スレッド。順序欠落（要件 1.2）は欠番を許容して継続

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| レイヤード拡張（採用） | 既存 4 crate に `transcribe/` モジュール追加 | steering 準拠、cargo bylaw 維持 | crate 肥大化 | audio-capture と同パターン |
| 独立 crate `gijirec-transcribe` | 推論を別 workspace メンバーに | 境界明確 | bylaw 更新・結線コスト | v1 では過剰 |
| フロント WASM whisper | UI 側推論 | 分離 | Python 不要だが性能・要件 6 違反 | 不採用 |

## Design Decisions

### Decision: whisper-cpp-plus 採用（VAD 駆動ストリーミング）
- **Context**: ローカル逐次 STT、3–5 s ウィンドウ、無音ブロック抑制
- **Alternatives Considered**:
  1. whisper-rs + 自前ストリーミング — 柔軟だが実装コスト大
  2. yamabiko-whisper — LocalAgreement-2 は本 spec スコープ外
- **Selected Approach**: `whisper-cpp-plus` の `WhisperStreamPcm`（VAD 駆動）。macOS は `metal` feature
- **Rationale**: stream-pcm.cpp 移植が要件 3.2/3.3 に直結。Python 不要
- **Trade-offs**: 新規依存（C++ ビルド）。fork ピン留めの追従コスト
- **Follow-up**: CI で Windows / macOS ビルド検証。実機性能は手動チェックリスト

### Decision: TranscriptBlock イベント＋内部 Consumer トレイト
- **Context**: 下流 transcript-editor への供給、追記のみ（要件 3.5）
- **Alternatives Considered**:
  1. Tauri イベントのみ — シンプルだが Rust 下流不可
  2. 共有メモリ — 過剰
- **Selected Approach**: `TranscriptBlockBus`（presentation）+ Tauri `whisper-transcribe://block-appended` イベント
- **Rationale**: audio-capture の `PcmChunkBus` / `PcmChunkConsumer` パターンをミラー
- **Trade-offs**: 二重配信パス。v1 は単一下流想定で許容

### Decision: 推論ワーカースレッド分離
- **Context**: 要件 7（会議アプリ並行）、PCM バス非ブロッキング
- **Selected Approach**: `on_pcm_chunk` は rtrb push のみ。`TranscribeWorker` が専用スレッドで VAD + 推論
- **Rationale**: 推論 1–3 s は RT パスに載せられない
- **Trade-offs**: スレッド間同期の複雑性。終了時 join 必須（要件 6.2）

## Risks & Mitigations
- **推論遅延が 5 s 超** — 量子化モデル + Metal/OpenBLAS。実機で step_ms / VAD 閾値チューニング
- **CPU 過負荷（要件 7）** — ワーカースレッド低優先度、無音時推論スキップ（VAD）
- **モデルダウンロード失敗** — リトライ UI、明確な `action_ja`（要件 5.4）
- **PCM ドロップによる欠落** — 欠番許容（要件 1.2）。メトリクス `transcribe_pcm_drops_observed` で監視

## References
- [whisper-cpp-plus](https://crates.io/crates/whisper-cpp-plus) — ストリーミング PCM API
- [audio-capture-pcm 契約](../../contracts/audio-capture-pcm.md) — 上流 PCM 形状
- [audio-capture-status 契約](../../contracts/audio-capture-status.md) — キャプチャフェーズ連携
