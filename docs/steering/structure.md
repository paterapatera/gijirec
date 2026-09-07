# Project Structure

## Organization Philosophy

**レイヤード・クリーンアーキテクチャ**を Rust と TypeScript の両面で採用する。ドメインが中心にあり、外側のレイヤは内側にのみ依存する。feature spec は `docs/specs/` で垂直に切り、実装は水平レイヤに配置する。

## Directory Patterns

### Feature Specs
**Location**: `docs/specs/{feature}/`  
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
| IPC 契約 | `docs/contracts/` | 新 command / イベント追加時 |

**完了判定**: `docs/specs/<feature>/tasks.md` 全 `[x]`、または該当コードの存在確認。**`spec.json` の `phase` だけで未完了と書かない**。Direct Implementation は設定ファイル（例: `tauri.conf.json`）を grep してから `[x]` にする。

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
_updated_at: 2026-09-07（Feature 完了クローズ・doc 同期スコープを追記）_
_Document patterns, not file trees. New files following patterns shouldn't require updates_
