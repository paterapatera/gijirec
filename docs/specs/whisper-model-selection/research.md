# Research & Design Decisions: whisper-model-selection

## Summary
- **Feature**: whisper-model-selection
- **Discovery Scope**: Extension (brownfield) — 完了済み whisper-transcribe のモデル取得・ロード経路を拡張
- **Key Findings**:
  - 現行実装は単一ファイル `kotoba-whisper-v2.2-ggml.bin`（FP16）固定。`ModelStore::MODEL_FILENAME` と `compose.rs` の `DEFAULT_WHISPER_MODEL_*` が正本
  - kenrouse 配布に 3 バリアントが同居（Q5_0 / Q8_0 / FP16）。各ファイル名・SHA-256 は ADR-0004 / 0010 / 0011 と HuggingFace LFS OID で整合
  - 永続化パターンは `transcript-editor-settings.md`（`app_data_dir` 配下 JSON + get/set command）が最も近い参照実装
  - `whisper-transcribe-status.md` のフェーズ列挙・`model-progress` イベントは変更不要（選択バリアントに対する取得・読み込みを既存イベントで表現可能）

## Gap Analysis (Step 2.0)

### Current State

| Asset | 状態 | ギャップ |
|-------|------|----------|
| `ModelStore` | 単一 `MODEL_FILENAME`、SHA-256 検証、legacy 移行 | バリアント別パス・検証・削除 API が必要 |
| `ModelDownloader` | URL + destination パスで汎用 DL | 変更不要（呼び出し側がバリアント別 URL/パスを渡す） |
| `ModelOrchestrator` | `ModelOrchestratorConfig { model_url, expected_sha256 }` 単一 | バリアントカタログ参照・選択変更・再取得フロー |
| `compose.rs` | FP16 の URL/SHA 定数のみ | 3 バリアント定数テーブル化 |
| フロント | `useTranscribeStatus` でフェーズ・進捗購読 | バリアント選択 UI + settings invoke ミラー |
| 永続化 | なし（常に FP16 固定） | `transcribe-settings.json` 新設 |

### Requirement-to-Asset Map

| 要件 | 既存 | ギャップ |
|------|------|----------|
| 1 選択 UI | なし | 新 UI コンポーネント + command |
| 2 取得・ロード・適用 | ModelOrchestrator 単一モデル | バリアント切替・次サイクル適用 |
| 3 状態表示 | whisper-transcribe-status 既存 | 変更不要（イベント再利用） |
| 4 永続化 | editor-settings パターン参照 | transcribe-settings 契約・サービス |
| 5 後方互換 | FP16 ファイル既存 | 初回既定 FP16、既存 `kotoba-whisper-v2.2-ggml.bin` を FP16 として認識 |

### Implementation Approach Options

**Option A — Extend ModelStore / ModelOrchestrator（推奨）**
- 単一モデル前提をバリアント ID 付き API に拡張
- ✅ 既存 DL・検証・フェーズイベントを再利用
- ❌ ModelStore / Orchestrator の責務が増える（許容範囲）

**Option B — バリアントごとに独立 ModelStore インスタンス**
- 3 並列ストア
- ❌ オーケストレータ複雑化、メモリ常駐リスク

**Option C — Hybrid（採用）**
- domain: `WhisperModelVariant` + `ModelVariantCatalog`
- infrastructure: `ModelStore` をバリアント対応に拡張
- application: `TranscribeSettingsService`（永続化）+ `ModelOrchestrator` 拡張
- presentation: Tauri commands + React 選択 UI

**Effort**: M（3–7 日） — 既存パターン踏襲、新契約 1 件
**Risk**: Low–Medium — 転写中切替のタイミング制御と既存 FP16 ファイル互換に注意

## Research Log

### kenrouse 配布ファイルと SHA-256
- **Context**: 各バリアントの取得 URL・整合性検証定数が必要
- **Sources Consulted**: HuggingFace `kenrouse/kotoba-whisper-v2.2-ggml` tree API、ADR-0004/0010/0011、`compose.rs`
- **Findings**:
  - Q5_0: `kotoba-whisper-v2.2-ggml-q5_0.bin` — SHA `4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658`
  - Q8_0: `kotoba-whisper-v2.2-ggml-q8_0.bin` — SHA `c4071b2f8f0129d463c6c7fd2e72c82f7276f9882a8f9cd9474e0c2b699100c4`
  - FP16: `kotoba-whisper-v2.2-ggml.bin` — SHA `eff70a8a236e731abba774ba71e1f6d0fce53302137208c32207e694e0bf4546`（現行 compose と一致）
- **Implications**: `ModelVariantCatalog` に filename / url / sha256 を集約。compose の単一定数はカタログへ移す

### 転写中バリアント切替
- **Context**: 要件 2.4 — 実行中推論を中断せず次サイクルから適用
- **Sources Consulted**: `structure.md`（30 s バッチサイクル）、`TranscribeWorker` 責務
- **Findings**: ワーカーはサイクル境界でモデルパスを再読込可能。切替要求は「次サイクル用 pending variant」として orchestrator が保持
- **Implications**: ホットリロードは不要。`loading_model` フェーズは新バリアント未取得時のみ

### 永続化と editor-settings パターン
- **Context**: 要件 4 — 再起動後復元、機微データ非含有
- **Sources Consulted**: `transcript-editor-settings.md`
- **Findings**: `{app_data_dir}/transcribe-settings.json`、部分更新 get/set、失敗時 FP16 フォールバック
- **Implications**: 新契約 `whisper-transcribe-settings.md` を作成

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks | Notes |
|--------|-------------|-----------|-------|-------|
| Catalog + extend orchestrator | バリアント定義を domain カタログに集約し既存 orchestrator を拡張 | 最小 diff、既存イベント再利用 | orchestrator 状態増 | **採用** |
| Per-variant orchestrator | バリアントごと独立 DL/ロード | 分離明確 | メモリ・複雑性 | 不採用 |
| Frontend-only selection | UI だけ変え URL をフロント保持 | 実装軽い | 契約・検証がフロント漏洩 | レイヤ違反で不採用 |

## Design Decisions

### Decision: バリアント別ファイルを同一 `models/` ディレクトリに共存
- **Context**: 複数バリアントのオフライン保持（要件 2.5）と既存 FP16 ファイル互換
- **Selected Approach**: `{app_data_dir}/models/<variant-filename>`。DL はバリアント単位。切替時は選択バリアントのみロード
- **Rationale**: ADR-0008 の保存先規約を維持。旧 FP16 利用者は追加取得不要
- **Trade-offs**: ディスク最大 ~2.8 GB（3 種すべて保持時）。v1 スコープ内

### Decision: フェーズイベント契約は変更しない
- **Context**: 要件 5.1 後方互換
- **Selected Approach**: `whisper-transcribe-status.md` は reference のみ。UI は既存 `useTranscribeStatus` を継続
- **Rationale**: `loading_model` / `model-progress` がバリアント取得を既に表現
- **Follow-up**: 設計で選択中バリアント表示は settings command のスナップショットで補完

### Decision: 新 ADR-0013 でユーザー選択を正式化
- **Context**: ADR-0011 Notes が「ユーザー向けモデル選択 UI — スコープ外」
- **Selected Approach**: 新 ADR で 3 バリアント選択を Accepted。ADR-0011 は FP16 既定のまま維持
- **Rationale**: append-only ADR 規約

## Risks & Mitigations
- **転写中切替のレース** — pending variant をサイクル開始前にのみ適用。ワーカー API で明示
- **永続化破損** — JSON パース失敗時 FP16 既定 + 日本語通知（要件 4.4）
- **SHA 不一致の旧ファイル** — 既存 `ModelCorrupt` フローで再取得促進

## References
- [kenrouse/kotoba-whisper-v2.2-ggml](https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml) — 配布ファイル一覧
- `docs/contracts/whisper-transcribe-status.md` — フェーズ・進捗イベント
- `docs/contracts/transcript-editor-settings.md` — 永続化パターン
- ADR-0008, ADR-0011 — 保存先・現行 FP16 既定
