# Research & Design Decisions: audio-device-selection

## Summary
- **Feature**: audio-device-selection
- **Discovery Scope**: Extension（audio-capture 拡張）/ Complex Integration — Gap + Full discovery
- **Key Findings**:
  - Windows は cpal の `output_devices()` から任意出力デバイスへ `build_input_stream`（`default_output_config`）で WASAPI ループバック可能。マイクは `input_devices()` + `Device` ID で選択可能
  - macOS マイクは cpal でデバイス選択可能。システム音声は ScreenCaptureKit がハードウェア出力単位の選択 API を持たず、システム全体ミックスを取得する（ADR-0009）
  - 既存 `CaptureOrchestrator` / アダプタ / PCM・ステータス契約を拡張するハイブリッド（Option C）が最小差分で要件を満たす

## Gap Analysis

### Current State Investigation

| 領域 | 既存実装（audio-capture 設計・契約） | 再利用可否 |
|------|--------------------------------------|------------|
| マイク取得 | `MicCaptureAdapter` — cpal 既定入力 | 拡張（デバイス ID 指定） |
| Windows ループバック | `WindowsLoopbackAdapter` — 既定出力 | 拡張（出力デバイス ID 指定） |
| macOS システム音声 | `MacScreenCaptureKitAdapter` — SCK 全体ミックス | 参照のみ（デバイス ID は preflight 用） |
| オーケストレーション | `CaptureOrchestrator` — 起動時自動開始 | 拡張（選択デバイス + 再開） |
| PCM 下流 | `PcmChunk` / `PcmChunkBus` | 変更なし |
| UI | `useCaptureStatus` — フェーズ・エラー | 拡張（デバイス選択パネル追加） |
| エラー契約 | `audio-capture-status.md` | 拡張（選択デバイス文脈のコード追加） |

**命名・レイヤ**: steering `structure.md` 準拠。Rust は `gijirec-{domain,application,infrastructure,presentation}`、TS は `src/{domain,application,infrastructure,presentation}`。Tauri IPC は presentation 境界。

### Requirement-to-Asset Map

| 要件 | 既存 | Gap |
|------|------|-----|
| 1.x 一覧取得 | cpal 列挙 API | **Missing** — `AudioDeviceEnumerator`、ホットプラグ監視、Tauri command |
| 2.x 選択 UI | なし | **Missing** — `DeviceSelectorPanel`、`useAudioDevices`、選択状態ストア |
| 3.x 選択キャプチャ | 既定デバイス二重キャプチャ | **Missing** — アダプタへのデバイス ID 伝播、再キャプチャ |
| 4.x エラー | 汎用 CaptureError | **Constraint** — 選択デバイス文脈の action_ja、サイレントフォールバック禁止は既存 5.2 と整合 |
| 5.x NFR | audio-capture 性能計画 | **Unknown** — 再開時間上限（設計で 2 s 目標） |
| 6.x プラットフォーム | Mac/Win 実装済み | **Constraint** — macOS スピーカー選択は ADR-0009 の制約付き |
| 7.x 権限 | 既存 preflight | **Constraint** — デバイス名のローカル限定表示（外部送信なし） |

### Implementation Approach Options

#### Option A: 既存コンポーネント拡張のみ
- `CaptureOrchestrator`、各アダプタ、`App.tsx` を直接拡張
- ✅ 差分最小、既存パターン踏襲
- ❌ オーケストレータと UI が肥大化

#### Option B: 新規コンポーネントのみ
- 独立 `DeviceSelectionService` + 新 UI、既存キャプチャは触らない
- ✅ 責務分離
- ❌ キャプチャ開始経路が二重化し、要件 3.4（サイレント切替禁止）の担保が難しい

#### Option C: ハイブリッド（推奨）
- **新規**: `DeviceSelectionStore`、`AudioDeviceEnumerator`、`DeviceSelectionCommands`、UI コンポーネント
- **拡張**: `CaptureOrchestrator::restart_with_devices`、`MicCaptureAdapter` / `WindowsLoopbackAdapter` のデバイス ID 受け取り
- ✅ 境界明確、PCM 契約非破壊、タスク分割可能

### Effort & Risk

| 項目 | 評価 | 理由 |
|------|------|------|
| Effort | **L**（1–2 週） | 2 OS × 2 デバイス種別、UI、再キャプチャ、ホットプラグ |
| Risk | **Medium** | macOS スピーカー選択のプラットフォーム制約、再キャプチャ中のギャップ |

## Research Log

### cpal デバイス列挙・選択（Windows / macOS マイク）
- **Context**: 要件 1, 2, 3 — 一覧と選択デバイスでのキャプチャ
- **Sources Consulted**: cpal docs.rs、`audio-capture` 設計、auricle-capture `enumerate`
- **Findings**:
  - `host.input_devices()` / `host.output_devices()` で列挙。`Device::name()` で表示名
  - Windows ループバック: 選択した **出力** デバイスに `build_input_stream` + `default_output_config()`
  - マイク: 選択した **入力** デバイスに `default_input_config()` + `build_input_stream`
  - デバイス ID は cpal `Device` の `id()` 文字列をセッション内安定キーとして使用
- **Implications**: `gijirec-infrastructure::audio::device_enumerator` に集約。domain に `AudioDeviceId` 値オブジェクト

### macOS スピーカー（ループバック）選択の制約
- **Context**: 要件 1.2, 2.3, 3.1 — SCK はハードウェア出力を指定できない
- **Sources Consulted**: Apple WWDC22 ScreenCaptureKit、mac-audio-recorder、Recall.ai ブログ
- **Findings**:
  - SCK はアプリ単位フィルタは可能だが、物理出力デバイス（ヘッドホン vs 内蔵）の選択 API はない
  - システム音声は OS がミックスしたストリームを取得
  - 選択スピーカーを実際の取得対象にするには、**OS 既定出力が選択デバイスと一致**している必要がある
- **Implications**: ADR-0009。不一致時はキャプチャ開始を拒否し `MACOS_OUTPUT_NOT_DEFAULT` で OS 設定変更を案内（サイレントフォールバック禁止）

### デバイスホットプラグと一覧更新
- **Context**: 要件 1.4
- **Sources Consulted**: cpal WASAPI default device monitoring、既存 audio-capture 5.3
- **Findings**:
  - cpal 0.16 は Windows でデバイス変更通知をサポート。macOS はポーリングまたは `notify` パターン
  - UI 表示中のみ一覧を再取得（要件 1.4）。バックグラウンド常時ポーリングは NFR 5.1 に反する可能性
- **Implications**: `DeviceSelectionService` が UI 可視フラグを受け取り、変更時に `audio-device-selection://devices-changed` を emit

### キャプチャ再開時間（要件 5.3）
- **Context**: 選択変更時の再キャプチャ
- **Findings**:
  - `stop` → ストリーム解放 → `start` の直列。audio-capture 既存停止は < 500 ms 想定
  - 再開合計 **2 s 以内**（設計 NFR）— 利用者が会議継続可能な上限
- **Implications**: 性能テストに再選択→`capturing` 復帰時間を追加

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| ハイブリッド拡張（採用） | 新規選択ドメイン + 既存キャプチャ拡張 | PCM 契約維持、タスク分割 | オーケストレータ API 拡張 | Option C |
| フロント Web API 列挙 | navigator.mediaDevices | ブラウザ標準 | Tauri ループバック非対応、OS 出力一覧不足 | 不採用 |
| 選択の永続化 | app_data JSON | 再起動後復元 | 要件スコープ外 | 不採用 |

## Design Decisions

### Decision: セッション内 `DeviceSelection` 状態（非永続）
- **Context**: 要件 2, brief Out（永続化除外）
- **Selected Approach**: `DeviceSelection { mic: Option<AudioDeviceId>, speaker: Option<AudioDeviceId> }`。`None` は OS 既定
- **Rationale**: 要件 2.5–2.6 の「未変更時は既定」に自然にマップ
- **Trade-offs**: 再起動で選択リセット（要件どおり）

### Decision: プラットフォーム別ループバック戦略（ADR-0009）
- **Context**: 要件 3, 6
- **Selected Approach**: Windows = 選択出力への cpal ループバック。macOS = SCK + 選択出力が OS 既定と一致することを preflight
- **Rationale**: 仮想デバイス不要・既存 ADR-0001 維持
- **Follow-up**: 実機で非既定出力選択時の UX 文言検証

### Decision: 汎用化 — `CaptureOrchestrator::restart_with_selection`
- **Context**: 合成レンズ — 将来のプリセット等に備え、現実装はデバイス ID のみ
- **Selected Approach**: `DeviceSelection` を引数に取る `start_with_selection` / `restart_with_selection`
- **Rationale**: 要件 3.3 の再開を単一経路に集約。サイレント切替防止（3.4）

## Risks & Mitigations
- **macOS スピーカー選択の期待ギャップ** — ADR-0009 + UI で「macOS はシステム既定出力と一致が必要」を明示
- **再キャプチャ中の PCM ギャップ** — `stopping` → `starting` フェーズを UI に表示。2 s 以内復帰を性能テストで検証
- **デバイス名の PII** — ログは ID のみ（`DEBUG` 限定で名前）。外部送信禁止（要件 7.2–3）

## References
- [cpal — Device enumeration](https://docs.rs/cpal/latest/cpal/) — 入出力デバイス列挙
- [ADR-0001](../architecture/adr/ADR-0001-platform-audio-capture.md) — 既存キャプチャ方式
- [audio-capture 設計](../audio-capture/design.md) — 拡張ベースライン
- Apple WWDC22 — ScreenCaptureKit（システム音声はデバイス非選択）
