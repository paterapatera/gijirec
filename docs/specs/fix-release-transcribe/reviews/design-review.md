## Verdict
- VERDICT: GO

## Summary

`fix-release-transcribe` の設計は Path D brownfield 不具合修正として、モデルパス正本化（ADR-0008）、Tauri ACL 追加、composition 起動順序修正、転写停滞ウォッチドッグの 4 点にスコープが明確に集中している。QA で停滞検知のフェーズ遷移・入力判定の曖昧さを是正し、Arch で steering 不整合（`whisper-rs` 表記）と既存ファイル配置の乖離を修正した。契約同期は OK、全 19 AC のトレーサビリティを確認。Phase Gate は VERIFIED。

## Reviewed Scope
- Reviewed contract paths: `docs/contracts/whisper-transcribe-blocks.md`, `docs/contracts/whisper-transcribe-status.md`, `docs/contracts/release-logging-persistence.md`
- ADR paths: `docs/architecture/adr/ADR-0008-model-store-app-data-dir.md`, `docs/architecture/adr/ADR-0003-whisper-cpp-plus-streaming.md`, `docs/architecture/adr/ADR-0004-whisper-model-kotoba.md`, `docs/architecture/adr/ADR-0007-release-file-logging.md`
- Contract sync: OK

## Findings

| ID | 重大度 | Pass | 内容 | 処置 |
| ---- | ------ | ---- | ---- | ---- |
| QA-1 | Major | QA | 停滞ウォッチドッグ発火時の phase 遷移が `error` / `ready` で未決定（要件 4.3 のユーザー可視失敗と契約整合が不明） | `error` + `INFERENCE_FAILED` に固定（Reflected Fixes） |
| QA-2 | Major | QA | 要件 4.3「continuous audible input」に対し `capturing` フェーズのみを proxy としていた（無音区間で誤検知リスク） | RMS 閾値 + worker observability による入力あり判定を明記（Reflected Fixes） |
| Arch-1 | Major | Arch | Technology Stack が `whisper-rs 0.16` と記載 — steering `tech.md` / ADR-0003 は `whisper-cpp-plus` | `whisper-cpp-plus 0.1` に修正（Reflected Fixes） |
| Arch-2 | Major | Arch | File Structure Plan が新規 `lifecycle_hook.rs` を想定 — brownfield では `tauri/lifecycle.rs` が既存フック | 既存 `lifecycle.rs` 拡張に修正（Reflected Fixes） |
| Arch-3 | Minor | Arch | Steering compliance が「`docs/steering/` 未整備」と記載 — `tech.md` / `structure.md` は存在 | steering 参照を更新（Reflected Fixes） |
| Sec-1 | — | Sec | 新規外部依存・認証面なし。モデル DL は既存 whisper-transcribe 委譲 | 追加修正不要 |
| Final-1 | — | Final | 反映修正 5 件を mechanical verification で確認 | 全件 present ✓ |

Critical / NO-GO トリガー: なし。

## Decisions

### QA
- **停滞検知の phase**: ウォッチドッグ発火時は `whisper-transcribe-status` 契約に従い `error` phase + `INFERENCE_FAILED` を正本とする。recoverable `ready` への直接遷移は採用しない（UI が error 表示で要件 4.3 を充足）。
- **入力あり proxy**: 初版は `capturing` + PCM RMS（VAD 閾値再利用）または worker 推論試行 observability の OR 条件。純粋無音は VAD 仕様どおりブロック不出力のため停滞検知対象外。

### Arch
- **既存 asset 再利用**: `TranscribeLifecycleHook` は `gijirec-presentation/src/tauri/lifecycle.rs` を拡張。新規 `lifecycle_hook.rs` は作成しない。
- **契約変更なし**: Persistent References はすべて `reference` または `modify`（boundaries / ADR-0008）で、イベント payload 形状は不変。ACL 行追加のみ。
- **Extension scenario 1**（新モデル追加 — Non-Goal）: whisper-transcribe / ADR-0004 が吸収。本 spec の境界・ファイルは変更不要。
- **Extension scenario 2**（Tauri `app_data_dir` API 変更）: Revalidation Triggers → ADR-0008 再検証。`ReleaseComposeRoot` / `ModelStore` 注入点のみ変更。

### Sec
- **AuthN/AuthZ**: N/A — ローカル単一利用者デスクトップ（requirements-review Sec 判断を継承）。
- **PII/ログ**: Observability は release-logging 禁止フィールドを遵守。`transcribe_stall_detected` は契約外 diagnostic フィールドとして許容（転写本文・PCM 不含）。
- **移行コピー**: Local → Roaming のモデル移行は同一ユーザーコンテキスト内のファイルコピー。新規信頼境界は追加しない（accepted risk）。

## Reflected Fixes
| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | Components / TranscribeStallWatchdog | 発火時 phase を `error` + `INFERENCE_FAILED` に固定 | QA |
| QA-2 | Components / TranscribeStallWatchdog | 入力あり判定を RMS + worker observability で具体化 | QA |
| Arch-1 | Technology Stack | `whisper-rs 0.16` → `whisper-cpp-plus 0.1（ADR-0003）` | Arch |
| Arch-2 | File Structure Plan / Modified Files | `lifecycle_hook.rs` → 既存 `tauri/lifecycle.rs` 拡張 | Arch |
| Arch-3 | Architecture Integration / Steering compliance | steering 未整備表記を `tech.md` / `structure.md` 参照に更新 | Arch |

## Specialist Summaries

### QA
**Summary**: 要件 4.1–4.4 の異常系（モデル失敗、ACL 拒否、転写停滞、ログ記録）は Error Handling / Observability / StallWatchdog でカバー。派生エッジケース（`app_data_dir` 解決失敗、setup 前モデルロード、移行失敗、ACL stale gen）も設計に記載あり。2 件の Major（停滞検知の phase 未定義・入力 proxy 不足）を修正。

**主要 Decisions**: 停滞検知は `error` + `INFERENCE_FAILED`。無音区間は検知対象外。

### Arch
**Summary**: レイヤ依存方向は維持。新規 `TranscribeStallWatchdog` は presentation 単一責務。`Mode: modify` の boundaries.md / ADR-0008 は存在し設計と整合。Anti-pattern スキャン: god object / circular dependency / ownership conflict なし。2 件 Major（STT 表記 drift、ファイル配置 drift）と 1 件 Minor を修正。

**主要 Decisions**: 既存 lifecycle.rs 拡張。契約 sync OK。

### Sec
**Summary**: 新規/changed surface は app_data_dir モデル保存、ACL listen 許可、移行コピー、diagnostic ログフィールド。認証不要。PII はローカル処理 + ログ禁止フィールド遵守。外部依存追加なし。

**主要 Decisions**: AuthN N/A。移行は同一ユーザーコンテキスト accepted risk。

## Gap-Domain Audit

| # | ドメイン | 結果 | 備考 |
| - | -------- | ---- | ---- |
| 1 | Requirements traceability | pass | 全 19 AC → 設計要素マッピング完了（Evidence マトリクス） |
| 2 | Non-functional (non-security) | pass | Performance N/A 明記。遅延は whisper-transcribe 5 秒窓参照 |
| 3 | Observability | pass | phase/error ログ、PII 禁止、stall diagnostic 記載 |
| 4 | Operability | pass | Deployment / Migration / Rollback 記載。feature flag 不要 |
| 5 | Testability | pass | unit / integration / release smoke で seams カバー |
| 6 | Compatibility | pass | No contract changes。既存イベント形状維持 |
| 7 | Scope fitness | pass | L tier、統合修正に限定。過剰抽象なし |
| 8 | Internal & external consistency | pass | steering / ADR / 契約と整合（修正後） |

## 承認ゲートサマリ

### 検証済み観点
- QA 異常系・エッジケースカバレッジ: pass（2 件修正反映済み）
- Arch SOLID / 契約同期 / ファイル配置: pass（3 件修正反映済み）
- Sec 威胁モデル / PII / 信頼境界: pass
- 反映検証: 5/5 fixes verified in design.md
- ギャップドメイン 1–8: 全 pass

### 自己修復した事項
Pass B（Final）による design.md 直接修正: なし（Pass A の 5 件修正で充足）。

### 受容が必要な残リスク
- **RMS 閾値による誤検知**: 環境ノイズが常時閾値超過する場合、停滞検知が早期発火する可能性。却下時: release smoke で実機調整が必要。
- **モデル移行の再 DL**: Local パスにのみモデルがある環境では初回 release 起動で再取得が発生（ADR-0008 trade-off）。却下時: 移行ロジックの実装タスクを優先。
- **Release-only 症状の検証**: ACL / composition 差分は clean `gen` + release build smoke に依存。却下時: CI optional ignore の手動チェックリストを必須化。

### 人間判断が必要な未決事項
0 件。

## Evidence

### 参照ファイル
- `docs/specs/fix-release-transcribe/spec.json` — phase: design-generated, approvals.design.generated: true
- `docs/specs/fix-release-transcribe/requirements.md`
- `docs/specs/fix-release-transcribe/design.md`（Pass A 修正後）
- `docs/specs/fix-release-transcribe/research.md`
- `docs/specs/fix-release-transcribe/reviews/requirements-review.md` — VERDICT: GO
- `docs/steering/tech.md`, `docs/steering/structure.md`
- Persistent References（contracts × 3, boundaries.md, ADR-0008, ADR-0003, ADR-0004, ADR-0007）

### Unwanted Behavior AC → Design Coverage

| AC | 設計カバレッジ |
| ---- | -------------- |
| 4.1 転写失敗通知 | Error Handling + `TranscribeEventEmitter` / status contract |
| 4.2 モデル失敗で error | `ModelOrchestrator` + Error Handling |
| 4.3 サイレント停止防止 | `TranscribeStallWatchdog`（修正後: RMS + 8s 閾値 → `error`） |
| 4.4 release ログ記録 | Observability + release-logging 契約参照 |

### Derived Edge Cases → Design Coverage

| Edge case | 設計カバレッジ |
| --------- | -------------- |
| `app_data_dir` 解決失敗 | Error Handling: setup エラー + INTERNAL |
| setup 前モデルロード race | ReleaseComposeRoot deferred inject + integration test 2 |
| ACL stale gen（dev→release） | Testing Strategy E2E: clean gen + release build |
| 移行コピー失敗 | Migration: MODEL_NOT_FOUND → 再 DL |
| 無音区間での誤停滞検知 | StallWatchdog: RMS + VAD 整合（修正後） |
| 二重ウォッチドッグ起動 | lifecycle start/stop 連動（idempotent 実装タスク） |

### Threat Table (STRIDE)

| # | Surface | Threat (STRIDE) | Impact | Mitigation / Accepted risk |
| - | ------- | --------------- | ------ | -------------------------- |
| 1 | `app_data_dir/models/` | Tampering (T) | モデル破損で転写不能 | SHA-256 検証（既存 ModelStore）→ MODEL_CORRUPT 通知 |
| 2 | HTTPS モデル DL | Spoofing (S) | 不正モデル取得 | SHA-256 + 既存 URL 定数（whisper-transcribe 委譲） |
| 3 | Tauri event ACL | Elevation (E) | 未許可 listen | 最小追加（block-appended のみ）+ event_permissions.rs gate |
| 4 | release ログ | Information disclosure (I) | 転写・PCM 漏洩 | release-logging 禁止フィールド + Observability セクション |
| 5 | モデル移行コピー | Information disclosure (I) | 他ユーザー読取 | OS ユーザーデータ ACL（accepted risk — Sec Decision） |

### Requirements → Design Traceability Matrix

| AC | 設計要素 |
| ---- | -------- |
| 1.1 | D-TranscribeLifecycleHook, D-TranscriptBlockBus, D-TranscribeAclGate |
| 1.2 | D-TranscribeWorker（既存 VAD ウィンドウ） |
| 1.3 | D-TranscribeOrchestrator, D-TranscribeLifecycleHook |
| 1.4 | D-ModelStore |
| 2.1 | D-ModelOrchestrator, D-ReleaseComposeRoot |
| 2.2 | D-ModelOrchestrator |
| 2.3 | D-TranscribeEventEmitter, Error Handling |
| 2.4 | D-TranscribeEventEmitter |
| 3.1 | D-TauriTranscribeEventEmitter |
| 3.2 | D-TauriTranscribeEventEmitter |
| 3.3 | D-TauriTranscribeEventEmitter |
| 4.1 | D-TranscribeEventEmitter, Error Handling |
| 4.2 | D-ModelOrchestrator |
| 4.3 | D-TranscribeStallWatchdog |
| 4.4 | Observability |
| 5.1 | D-TranscriptBlockBus |
| 5.2 | Boundary Commitments / Non-Goals |
| 5.3 | 統合修正全体 |
| 5.4 | Non-Goals |

未マップ AC: なし。

### チェック項目
- QA checklist 1–8: pass（修正後）
- Arch checklist 1–11: pass（contract sync OK、extension scenarios recorded）
- Sec checklist 1–10: pass
- Synthesis reflection verification: pass（5/5）
- Synthesis gap domains 1–8: pass

## Phase Gate
- STATUS: VERIFIED
- CHECKS:
  1. `docs/specs/fix-release-transcribe/design.md` 存在 — **pass**
  2. `spec.json` → `approvals.design.generated === true` — **pass** (`true`)
  3. `reviews/design-review.md` → `VERDICT: GO` — **pass**（本レポート）
  4. Phase Gate `STATUS: VERIFIED` — **pass**（本レポート）
  5. `approvals.design.approved === false` — **pass** (`false`)
