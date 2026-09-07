# Contract Standards

永続契約（`docs/contracts/`）のライフサイクルと、Rust / TypeScript へのミラー規約。feature spec や steering core とは役割を分ける。

## Philosophy

- **契約は feature を超えて残る** — spec をアーカイブしても下流が参照する面は残す
- **1 ファイル = 1 契約面** — API / Event / Data ownership のいずれか一つ
- **正本は一箇所** — 詳細のコピペを boundaries や steering に散らさない
- **型で同期** — 契約変更はコードミラーとテストをセットで更新

## What Belongs in `docs/contracts/`

| 契約面 | 例 | 所有者 |
|--------|-----|--------|
| Event | `audio-capture://phase-changed` | audio-capture |
| Data | `PcmChunk` 形状・供給規約 | audio-capture |
| Command | `get_capture_phase`、`get_transcribe_phase`、`get_transcribe_status`、`list_audio_devices`、`get_device_selection`、`set_device_selection`、`set_audio_device_ui_visible`、`save_transcript_session`、`get_editor_settings`、`set_editor_settings`、`pick_save_directory` | audio-capture / whisper-transcribe / audio-device-selection / transcript-editor |
| Data | リリース診断ログの保存場所・セッション ID・禁止フィールド | release-logging |

**入れないもの**: 実装手順、タスク分解、ADR 全文、UI モック、一時的な spike メモ。

## Naming & Layout

- **ファイル名**: `<domain>-<surface>.md`（kebab-case）
  - `audio-capture-status.md` — イベント
  - `audio-capture-pcm.md` — データ所有
  - `transcript-editor-save.md` / `transcript-editor-settings.md` / `transcript-editor-status.md` — 保存・設定 command とエラー形状
  - `audio-device-selection.md` — デバイス一覧・セッション選択 command / イベント
  - `release-logging-persistence.md` — リリース診断ログの永続化規約（cross-cutting）
- **イベント名**: `<domain>://<verb-or-noun>`（例: `audio-capture://error`）
- **index 必須**: 新規契約追加時は `docs/contracts/README.md` の Entries 行を更新（欠落禁止）

## Lifecycle

```
1. spec 設計で下書き（feature 内でも可）
2. 設計 GO 前に docs/contracts/ へ昇格
3. README Entries 更新
4. Rust domain + presentation + TS ミラーを実装
5. 契約テスト（マッピング・payload 形状）を追加
6. Changelog 行を契約ファイル末尾に追記
```

**禁止**:
- `docs/specs/{feature}/contracts/` を永続正本にしない
- `docs/architecture/boundaries.md` に契約詳細をコピペ
- 契約ファイル追加だけして index を更新しない

### 契約追加・変更時の同期先

新 command / イベントを `docs/contracts/` に足したら、同一 PR / 同一セッションで次も確認する:

- `docs/contracts/README.md` の Entries
- `docs/architecture/boundaries.md` の IPC 行
- `docs/steering/contracts.md`（本ファイル）の索引
- ルート `README.md`（利用者向けに影響する場合のみ）

steering だけ更新して boundaries / README を残さない。

## Code Mirroring

### Rust（正本に近い）

| 契約 | 実装場所 |
|------|----------|
| データ形状 | `gijirec-domain`（例: `PcmChunk`, `UserFacingError`, `TranscriptBlock`, `TranscribePhase`, `EditorSettings`, `EditorError`） |
| イベント定数・payload | `gijirec-presentation::tauri::events`（capture）、`gijirec-presentation::transcribe::event_emitter`（transcribe） |
| emit / command | `gijirec-presentation::tauri`（capture / device_selection）、`gijirec-presentation::editor`（save/settings）、`src-tauri/src/commands.rs`（ホスト登録） |
| 診断ログ永続化 | `src-tauri/src/logging/`（ホストのみ。`--log` 時 `app_data_dir/logs/`） |

domain に契約コメントで path を参照:

```rust
/// User-facing error payload per `docs/contracts/audio-capture-status.md`.
pub struct UserFacingError { ... }
```

### TypeScript（読み取り専用ミラー）

- **イベント／状態**: `src/presentation/hooks/{domain}-status.ts`（例: `capture-status.ts`、`transcribe-status.ts`、`transcript-blocks.ts`、`editor-settings.ts`）
- **Command 面**: `src/infrastructure/tauri/editorCommands.ts`（save / settings / pick directory）、`src/infrastructure/tauri/audioDeviceCommands.ts`（デバイス一覧・選択）。イベント購読ではなく invoke ラップ
- **内容**: イベント名定数 + interface（契約と同一フィールド名）
- **変換なし**: snake_case フィールド（`timestamp_ms`）は契約どおり維持。hook 内で camelCase に変換する場合は state 型のみ

```typescript
/** Contract types per `docs/contracts/audio-capture-status.md`. */
export const PHASE_CHANGED_EVENT = "audio-capture://phase-changed" as const;
```

### 跨境界ルール

- `src/` は `src-tauri/` を import しない — 契約型は TS 側にミラー
- 下流 spec（whisper-transcribe）は `PcmChunk` **契約** にのみ依存。キャプチャ実装 crate に直接依存しない
- v1: PCM は Rust 内部バスのみ。フロントへ `audio-capture://pcm-chunk` は発行しない

## Contract Document Structure

テンプレ: `docs/settings/templates/contracts/contract.md`

必須セクション:
- **Purpose** — 1 段落
- **Contract** — 型・列挙・禁止事項
- **Non-goals** — 境界外
- **Changelog** — 日付・変更・根拠

## Breaking Change Policy

| 変更種別 | 手順 |
|----------|------|
| フィールド追加（後方互換） | Changelog + 下流 spec で吸収可否を確認 |
| 列挙値追加 | domain マッピング + UI + 契約表を同時更新 |
| チャンク長・サンプルレート変更 | 下流 Revalidation Trigger（契約 Notes 参照） |
| イベント名変更 | 避ける。不可避なら ADR + 両言語ミラー同時 |

## Reading Discipline（エージェント・開発者共通）

1. 最初に `docs/contracts/README.md` の index **のみ**読む
2. 必要な 1 ファイルだけ Read
3. index に無いファイルを「念のため」全量開かない

## Related

| 文書 | 役割 |
|------|------|
| `docs/architecture/boundaries.md` | 誰が何を Own するか（契約詳細なし） |
| `docs/steering/structure.md` | レイヤとディレクトリパターン |
| `docs/steering/error-handling.md` | エラー契約の UI / ログ規約 |
| `docs/specs/{feature}/design.md` | feature 内の設計・シーケンス |

---
_updated_at: 2026-09-07（契約変更時の同期先を追記）_
_Document contract lifecycle and mirroring, not every field of every contract._
