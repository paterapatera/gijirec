# Project Structure

## Organization Philosophy

**レイヤード・クリーンアーキテクチャ**を Rust と TypeScript の両面で採用する。ドメインが中心にあり、外側のレイヤは内側にのみ依存する。feature spec は `docs/specs/` で垂直に切り、実装は水平レイヤに配置する。

## Directory Patterns

### Feature Specs
**Location**: `docs/specs/{feature}/`（新規 feature 用。v1 完了分は 2026-09-07 にアーカイブ削除済み）  
**Purpose**: 機能単位の要求・設計・タスク（audio-capture, whisper-transcribe, transcript-editor 等）  
**Naming**: kebab-case の機能名

### Steering（本ディレクトリ）
**Location**: `docs/steering/`  
**Purpose**: プロジェクト全体の永続メモリ（product, tech, structure, roadmap）  
**Note**: spec 横断の原則のみ。feature 詳細は spec 配下

### Persistent Architecture
**Location**: `docs/architecture/`, `docs/contracts/`  
**Purpose**: 境界・依存方向（boundaries）、ADR、永続契約面  
**Rule**: feature 配下の作業資料をここへ移さない。index から必要ファイルだけ Read する

### TypeScript Frontend
**Location**: `src/`  
**Purpose**: Web UI、Tauri IPC のフロント側（キャプチャ・文字起こし状態＋転写エディタ）  
**Layers**（dependency-cruiser で強制）:
- `src/domain/` — ドメインモデル（外レイヤに依存しない）。転写は `domain/transcript/`（型・Markdown/JSONL エクスポート）
- `src/application/` — ユースケース（domain のみ）。転写は `application/transcript/`（blockReducer、Slate プラグイン、saveOrchestrator）
- `src/infrastructure/` — 外部アダプタ（domain のみ）。`infrastructure/tauri/editorCommands.ts` が保存／設定 invoke をラップ。`infrastructure/tauri/audioDeviceCommands.ts` がデバイス一覧・選択 invoke をラップ
- `src/presentation/` — UI・composition root（`App.tsx`、hooks、`components/` の二重エディタ・`DeviceSelectorPanel` と chrome）

**二重エディタ再描画分離**: `TranscriptEditorView` は block 購読を持たず、`AiTranscriptPanel` 内で `useTranscriptBlocks` を局所化する。`block-appended` 更新は AI 側のみ再描画し、手入力 `HandwritingEditor` へ波及しない。`HandwritingEditor` は `React.memo` + IME `composition` イベントガード。親からの ref は `useCallback` + `externalHandwritingRef` で安定化（`exactOptionalPropertyTypes` 対応のため `AiTranscriptPanel` への ref は条件付き spread）。

**Presentation パターン**: `docs/contracts/` のイベント／型を `presentation/hooks/` にミラーし、Tauri `listen` / `invoke` で購読。マウント時は `get_capture_phase` / `get_transcribe_status` / `get_editor_settings` / `get_device_selection` で同期。テスト時は `listenFn` / `invokeFn` を注入。command ミラーは hooks ではなく `infrastructure/tauri/{editorCommands,audioDeviceCommands}.ts`。

### Rust Backend
**Location**: `src-tauri/crates/`  
**Purpose**: 音声キャプチャ・Whisper 推論・Tauri コマンド／イベント  
**Crates**（cargo bylaw で強制）:

| Crate | 依存可能 | 主なモジュール |
|-------|----------|----------------|
| `gijirec-domain` | なし（最内層） | `audio/`（`device.rs` 含む）、`transcribe/`、`editor/`（EditorSettings, Save リクエスト, EditorError） |
| `gijirec-application` | domain | `capture/`、`device_selection/`（DeviceSelectionStore, DeviceSelectionService）、`transcribe/`、`editor/`（SettingsService, SaveService — ファイル I/O はここ。infrastructure には editor アダプタを置かない） |
| `gijirec-infrastructure` | domain | `audio/`（`device_enumerator`、マイク／ループバックのデバイス ID 指定。editor なし）、`transcribe/`（Whisper / モデル取得） |
| `gijirec-presentation` | domain, application, infrastructure | `tauri/`（capture、`device_selection`）、`transcribe/`、`editor/`（command 実装・observability） |

**Presentation パターン**: `gijirec-presentation` が composition root。`tauri/`（capture / device_selection）/ `transcribe/` / `editor/` が各ドメインの Tauri 境界。`src-tauri/src/compose.rs` と `commands.rs` がホスト側で結線。契約イベント（`audio-capture://…`、`whisper-transcribe://…`、`audio-device-selection://…`）と editor / device command でフロントと同期。

**ホスト横断**: `src-tauri/src/logging/` がリリース診断ログ（`--log`、ADR-0007）。レイヤ crate 外の composition 専用。

**IPC 同期パターン**: モデル取得など長時間処理中に orchestrator ロックを避けるため、`TranscribeStatusCache` がフェーズ／進捗スナップショットを保持し、`get_transcribe_phase` / `get_transcribe_status` でマウント時同期する。エディタ設定は `get_editor_settings` / `set_editor_settings`（`app_data_dir/editor-settings.json`）。デバイス選択は `get_device_selection` / `list_audio_devices`（セッション内のみ永続化なし）。保存は command 往復（イベントではない）。

**キャプチャパイプライン（composition）**: `CapturePipelineState`（mixer / `ChunkEmitter` / `PcmChunkBus`）を Tauri state に保持。アダプタの rtrb consumer はポート内にあり、処理スレッド結線は `CaptureProcessingHook`（start 後起動）。リサンプラは入力レートが open 後まで不明なため processing スレッド起動時に構築。macOS SCK は 48 kHz 固定。可観測性は presentation の `CaptureObservability` トレイト経由（host が tracing 実装）。`capture_rt_callback_max_us` は処理スレッド drain レイテンシの代理。Linux 非対応は `on_app_setup` でダイアログ。`RunEvent::Exit` 停止は `handle_capture_run_event` を `app.run` から呼ぶ。

**デバイス再選択**: 再キャプチャ時は `ChunkEmitter` を再生成せず `discard_partial_buffer` のみ行い `sequence` を継続する（`audio-capture-pcm` の単調増加・欠番なし）。

**転写ワーカー（バッチ）**: `TranscribeWorker` は `take_batch_window` で先頭 480k samples を非破棄切り出し、`BATCH_INTERVAL`（30 s）起点でサイクル実行。停止時 flush・推論失敗時は次サイクル継続。可観測性は worker コールバック → presentation `observability` → host tracing。

**転写エディタ（上流同期）**: AI 転写ブロックの上流同期は `editor.applyUpstream(op)` 必須。直接 `Editor.apply` では locked 範囲保護されない（`withLockedRanges`）。末尾判定は `Editor.end` ベース（`withStableSelection`）。

## Naming Conventions

- **Rust crates**: `gijirec-{layer}`（domain, application, infrastructure, presentation）
- **Spec directories**: kebab-case（`audio-capture`）
- **ADR files**: `ADR-NNNN-short-title.md`（ゼロ埋め 4 桁）
- **Contract files**: `{domain}-{surface}.md`
- **Functions / variables**: Rust は snake_case、TypeScript は camelCase
- **Unused bindings**: `_` プレフィックスで明示的に無視

## Import Organization

TypeScript は Biome の `organizeImports` を有効化。レイヤ越えの import は dependency-cruiser で検出する。

```typescript
// 契約型のミラー（docs/contracts/ を正本）
import type { CapturePhaseChanged } from "./capture-status";
import { PHASE_CHANGED_EVENT } from "./capture-status";

// Tauri IPC（presentation 層のみ）
import { listen } from "@tauri-apps/api/event";
```

**Cross-boundary rules**:
- `src/` → `src-tauri/` 禁止（フロントは IPC 経由のみ）
- `src/domain/` → application / infrastructure / presentation 禁止
- `src/application/` → presentation 禁止

Rust は crate 間の `path` 依存のみ。presentation が composition root。

## Code Organization Principles

1. **ドメイン中心** — ビジネスルールは domain crate / `src/domain` に集約
2. **アダプタ分離** — OS API・whisper.cpp・ファイル I/O は infrastructure
3. **spec 駆動** — 新機能は `docs/specs/{feature}/` から着手し、roadmap の依存順に従う
4. **契約の正本** — API / イベント形状は `docs/contracts/` に永続化（feature 内は下書き可）
5. **境界変更は ADR** — レイヤ依存や技術選択の変更は `docs/architecture/adr/` に記録

## Feature 完了クローズ

spec の `tasks.md` が全 `[x]` になったら、次を **同一作業単位** で行う（途中で steering だけ止めない）。

1. `bun run verify`
2. ドキュメント同期（下記スコープを一括照合）
3. `roadmap.md` / `product.md` の状態更新
4. コミット（Conventional Commits・説明は日本語）
5. 必要なら main 向けスカッシュ

### ドキュメント同期スコープ

| 領域 | 正本 | 同期時に見るもの |
|------|------|------------------|
| 横断メモリ | `docs/steering/` | product / roadmap / structure / contracts 等 |
| 境界・ADR | `docs/architecture/` | `boundaries.md`、関連 ADR |
| 契約テンプレ | `docs/settings/templates/` | 新パターンの反映 |
| 利用者向け | ルート `README.md` | できること・構成・verify 説明 |
| 手動検証・運用 | `docs/manual/` | E2E・性能実測・リリースログ収集手順 |
| IPC 契約 | `docs/contracts/` | 新 command / イベント追加時 |

**完了判定**: `docs/specs/<feature>/tasks.md` 全 `[x]`、または該当コードの存在確認。**`spec.json` の `phase` だけで未完了と書かない**。Direct Implementation は設定ファイル（例: `tauri.conf.json`）を grep してから `[x]` にする。

### Spec ライフサイクル（完了 feature の整理）

feature 完了後、spec ディレクトリを削除する前に次を行う（削除は **人間が週次** で実施可）:

1. **Implementation Notes 昇格** — 恒久パターンを `docs/steering/` / `docs/contracts/` / `docs/architecture/` / `README.md` に移す
2. **手動ドキュメント移設** — チェックリスト・運用手順を `docs/manual/` に移し、参照を更新（spec 配下に残さない）
3. **steering 同期** — 人間が `/sdd-steering` で横断照合（週次運用可。自動 dispatch は不要）
4. **`bun run verify`**
5. **spec 削除** — 人間が `docs/specs/<feature>/` を削除し、`product.md` / `README.md` の spec 参照を整理

長時間性能・E2E・並走検証の記録先は [docs/manual/README.md](../manual/README.md)。

## Quality Scripts Mapping

| 対象 | コマンド | 検証内容 |
|------|----------|----------|
| **完成判定** | `bun run verify` | 下記 lint・テスト一式 |
| TS 全体 | `bun run check` | format, types, lint, arch, dead code |
| TS テスト | `bun run test` | フロント4レイヤ（capture / transcribe / editor） |
| TS arch fixture | `bun run test:arch` | dependency-cruiser レイヤルールの回帰テスト |
| Rust 全体 | `bun run rust:check` | fmt, types, clippy, bylaw, dead code |
| Rust テスト | `bun run rust:test` | `cargo test --workspace` |

---
_updated_at: 2026-09-09（二重エディタ再描画分離・バッチ転写ワーカーを追記）_
_Document patterns, not file trees. New files following patterns shouldn't require updates_
