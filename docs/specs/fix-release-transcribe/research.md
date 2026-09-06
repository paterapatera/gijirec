# Research & Design Decisions: fix-release-transcribe

---
**Purpose**: brownfield ギャップ分析と Full discovery 成果を記録し、`design.md` の判断根拠とする。
---

## Summary

- **Feature**: `fix-release-transcribe`
- **Discovery Scope**: Brownfield 不具合修正（Path D）+ Full discovery（`complexity_tier: L`）
- **Key Findings**:
  - **モデル保存パス不整合**: `build_capture_stack()` が `dirs::data_local_dir()/gijirec` で `ModelStore` を初期化する一方、Tauri setup は `app.path().app_data_dir()`（identifier `com.gijirec.app` → Windows では `%APPDATA%\com.gijirec.app`）を editor / release-logging で使用。設計（whisper-transcribe D-ModelStore）の正本と乖離。
  - **Tauri ACL 欠落**: `whisper-transcribe://block-appended` が `allow-listen-transcribe-events.toml` に未登録。`event_permissions.rs` は登録を要求。リリース EXE ではカスタムイベント購読が拒否され、エディタへブロックが届かない可能性が高い（dev では gen / ビルド順序により症状が隠れるケースあり — Tauri issue #14310）。
  - **診断手段は整備済み**: 上流 `release-logging` により `--log` 起動で `gijirec_transcribe` の `transcribe_phase` / `error_code` を永続化可能。本修正の検証・切り分けに利用する。

## Gap Analysis（Step 2.0）

### Current State Investigation

| 領域 | 既存資産 | 備考 |
|------|----------|------|
| 文字起こしパイプライン | whisper-transcribe 実装完了（dev 動作確認済み） | orchestrator / worker / block bus / lifecycle hook |
| モデル I/O | `ModelStore`（`app_data_dir/models/` 想定）、`ModelDownloader`（reqwest rustls） | **実際の base は `dirs::data_local_dir`** |
| composition root | `src-tauri/src/compose.rs::build_capture_stack()` | `run()` 冒頭で Tauri setup 前に呼び出し |
| Tauri setup | `src-tauri/src/lib.rs` setup | `app_data_dir` で editor / logging。モデルロードは別スレッドで既存 `ModelOrchestrator` を使用 |
| イベント emit | `TauriTranscribeEventEmitter`, `TauriTranscriptBlockEventEmitter` | block は `emit_to("main", ...)` + `emit` フォールバック |
| ACL | `capabilities/default.json`, `permissions/allow-listen-*.toml` | transcribe: phase / progress / error のみ。**block-appended 欠落** |
| 診断ログ | `release-logging` 完了 | `--log` + `release-logging-persistence` 契約 |
| 統合テスト | `transcribe_integration.rs`, `event_permissions.rs` | リリース EXE 向け E2E は未整備 |

**命名・レイヤ規約**: 既存 4 crate + ホスト composition root。新規ドメイン型は不要。

### Requirement-to-Asset Map

| 要件 AC | 既存 | ギャップ |
|---------|------|----------|
| 1.1–1.4 リリース転写 | パイプライン実装済み | パス / ACL / 起動順序により release で機能不全 → **Constraint** |
| 2.1–2.4 モデル取得 | ModelOrchestrator 実装済み | 誤 `app_data` 基準で取得・検証 → **Missing（パス正本化）** |
| 3.1–3.3 フェーズ UI | `useTranscribeStatus` + events | ACL 不足で block / 一部 event 欠落の可能性 → **Constraint** |
| 4.1–4.3 障害通知 | `TranscribeUserError` 契約 | サイレント停止（transcribing 固定）の検知なし → **Missing（ウォッチドッグ）** |
| 4.4 ログ記録 | transcribe observability 実装済み | release `--log` 時に phase / error は記録可能。追加 observability は最小限で可 |
| 5.1–5.4 仕様維持 | whisper-transcribe 契約 | アルゴリズム変更なし — 本 spec はパリティ回復のみ |

### Implementation Approach Options

#### Option A: パス + ACL の最小修正（採用）
- setup 内で Tauri `app_data_dir` を `ModelStore` に注入し、モデルロードを setup 後に開始。
- `allow-listen-transcribe-events.toml` に `block-appended` を追加。
- **Trade-offs**: 最小 diff、契約形状変更なし。composition 順序の局所変更のみ。

#### Option B: モデルを Tauri resource bundle に同梱
- **Rejected**: モデル ~数百 MB。初回ダウンロード設計（ADR-0004）と矛盾。bundle サイズ爆発。

#### Option C: フロント polling に切替（invoke でブロック取得）
- **Rejected**: 新 IPC 契約が必要。既存 event 駆動設計と重複。

**Effort**: M（3–7 日）— 原因は限定的だが release 検証・ウォッチドッグ追加でテスト工数あり。  
**Risk**: Medium — release-only 症状の再現環境依存。`--log` と ACL テストで軽減。

## Research Log

### Tauri 2 イベント ACL（dev vs release）

- **Context**: brief の「イベント権限」仮説。release EXE で `listen()` が拒否される報告（Tauri #12325, #14310）。
- **Sources Consulted**: [Tauri capabilities](https://v2.tauri.app/security/capabilities/), [issue #14310](https://github.com/tauri-apps/tauri/issues/14310), `src-tauri/permissions/*.toml`, `event_permissions.rs`
- **Findings**:
  - カスタムイベント名は permission TOML で `[[permission.event.allow]]` 明示が必要。
  - `core:default` は `core:event:allow-listen` を含むが、**イベント名スコープ**は別途 allow-list。
  - `gen/` ディレクトリの stale 状態で dev 後の release ビルドが ACL 不整合になる事例あり。CI では clean build 前提。
- **Implications**: `block-appended` を transcribe permission に追加。`event_permissions.rs` を gate テストとして維持。release ビルド検証手順に clean `gen` を明記。

### モデルパス解決（dirs vs Tauri app_data_dir）

- **Context**: `compose.rs` L51–53 vs `lib.rs` setup L256–259。
- **Findings**:
  - `dirs::data_local_dir()/gijirec` → 例: `%LOCALAPPDATA%\gijirec`
  - Tauri `app_data_dir()` → 例: `%APPDATA%\com.gijirec.app`（operations.md と一致）
  - editor settings / release logs は Tauri 正本。モデルだけ別ディレクトリ。
  - dev で「動く」場合: 同一マシンに Local 側へモデルが既に存在、またはフェーズ表示のみ確認でブロック未到達に気づかない。
- **Implications**: ADR-0008 で Tauri `app_data_dir` をモデル保存の正本に固定。composition を setup 後モデルロードに遅延。

### release-logging による切り分け手順

- **Context**: 上流 release-logging 完了。Req 4.4。
- **Findings**:
  - `gijirec.exe --log` → `{app_data_dir}/logs/sessions/{run_session_id}/gijirec.log`
  - transcribe 行: `transcribe_phase`, `error_code`, `session_id`, latency / drop metrics
  - 禁止: 転写全文・PCM（契約準拠で既存 observability が遵守）
- **Implications**: 修正前ベースライン取得 → 修正後同一手順で phase 遷移と block 供給をログ相関。operations.md を参照（変更不要）。

### サイレント停止（Req 4.3）

- **Context**: `transcribing` だがブロック未供給が続くケース（ACL / worker 失敗）。
- **Findings**:
  - 現状: phase は `transcribing` でも block ゼロのまま UI は待機表示のみ。
  - whisper-transcribe 遅延目標: 5 秒（要件 1.2 / 3.2 参照）。
- **Implications**: capture active + transcribing + 連続入力で N 秒ブロック無し → recoverable error または UI 警告。N は whisper-transcribe と同値（5 秒 + マージン）。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Setup 後パス注入 | Tauri setup で ModelStore 再構成 | 単一正本、editor と整合 | composition 順序変更 | **採用** |
| dirs 維持 + symlink | Local と Roaming をリンク | 変更小 | OS 依存、権限複雑 | 不採用 |
| 二重パスフォールバック | 両方を探す | 移行容易 | 永続的な複雑性 | 不採用（一回限り移行で十分） |

## Design Decisions

### Decision: Tauri app_data_dir をモデル保存の正本に統一（ADR-0008）

- **Context**: Req 2.1–2.4、パス不整合
- **Selected Approach**: `ModelStore::new(app.path().app_data_dir())` を setup 内で行い、モデルロードスレッドを setup 後に開始。既存 Local パスにモデルがある場合は初回起動時に移行または再取得（実装タスクで選択）。
- **Rationale**: editor / logging と同一 OS ユーザーデータ領域。Tauri identifier 変更に追従。
- **Trade-offs**: 初回 release 起動で再ダウンロードの可能性（一回限り）。

### Decision: block-appended ACL 追加

- **Context**: Req 1.1, 3.1, transcript-editor 購読
- **Selected Approach**: `permissions/allow-listen-transcribe-events.toml` に `whisper-transcribe://block-appended` を追加。
- **Rationale**: 契約 `whisper-transcribe-blocks.md` で定義済みイベント。`event_permissions.rs` が回帰防止。

### Decision: 転写停滞ウォッチドッグ

- **Context**: Req 4.3
- **Selected Approach**: presentation 層で capture active + transcribing 中の最終 block 時刻を監視。閾値超過で `INFERENCE_FAILED` 相当のユーザー通知。
- **Rationale**: ACL 修正後も worker 失敗のサイレント継続を防ぐ。契約エラーコードを再利用。

## Risks & Mitigations

- **リリースのみ再現** — `--log` ベースライン + clean `gen` 後 `cargo tauri build` 検証手順をテスト計画に含める
- **モデル再ダウンロード** — 移行ロジックまたは利用者向けメッセージ（MODEL_NOT_FOUND → 再取得）
- **whisper-rs release リンク** — ローカル release ビルド smoke（モデルロード + 短 PCM）を統合テストに追加（`#[ignore]` ハードウェア向け）

## References

- `docs/specs/whisper-transcribe/design.md` — 参照パイプライン
- `docs/contracts/release-logging-persistence.md` — 診断ログ契約
- `docs/contracts/whisper-transcribe-blocks.md` — block-appended イベント
- `src-tauri/src/compose.rs`, `src-tauri/src/lib.rs` — パス不整合の実コード
- [Tauri issue #14310](https://github.com/tauri-apps/tauri/issues/14310) — release ACL / gen ディレクトリ
