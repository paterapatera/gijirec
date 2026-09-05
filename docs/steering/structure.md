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
**Purpose**: Web UI、Tauri IPC のフロント側（現状は `audio-capture` の状態表示）  
**Layers**（dependency-cruiser で強制）:
- `src/domain/` — ドメインモデル（外レイヤに依存しない）
- `src/application/` — ユースケース（domain のみ）
- `src/infrastructure/` — 外部アダプタ（domain のみ）
- `src/presentation/` — UI・composition root（`App.tsx`、契約ミラー用 hooks）

**Presentation パターン**: `docs/contracts/` のイベント／型を `presentation/hooks/` にミラーし、Tauri `listen` / `invoke` で購読。テスト時は `listenFn` を注入。

### Rust Backend
**Location**: `src-tauri/crates/`  
**Purpose**: 音声キャプチャ、Tauri コマンド／イベント（Whisper 推論は将来 crate 拡張）  
**Crates**（cargo bylaw で強制）:

| Crate | 依存可能 | 主なモジュール |
|-------|----------|----------------|
| `gijirec-domain` | なし（最内層） | `audio/`（PcmChunk, Phase, Error） |
| `gijirec-application` | domain | `capture/`（mixer, orchestrator, chunk_emitter） |
| `gijirec-infrastructure` | domain | `audio/`（mic, resampler, platform/*） |
| `gijirec-presentation` | domain, application, infrastructure | `tauri/`（commands, events, lifecycle, pcm_bus, observability） |

**Presentation パターン**: `gijirec-presentation::tauri` が composition root。Tauri state にパイプライン／ライフサイクルを保持し、契約イベント名（例: `audio-capture://phase-changed`）でフロントへ通知。

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

## Quality Scripts Mapping

| 対象 | コマンド | 検証内容 |
|------|----------|----------|
| TS 全体 | `bun run check` | format, types, lint, arch, test:arch, dead code |
| Rust 全体 | `bun run rust:check` | fmt, types, clippy, bylaw, dead code |

---
_updated_at: 2026-09-05（Sync: 契約ミラー・tauri モジュール・Bun コマンドを反映）_
_Document patterns, not file trees. New files following patterns shouldn't require updates_
