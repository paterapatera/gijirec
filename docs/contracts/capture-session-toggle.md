# capture-session-toggle

- **Surface type**: API / Event
- **Owners / Domains**: capture-session-toggle
- **Related ADR**: docs/architecture/adr/ADR-0015-capture-session-start-only.md

## Purpose

利用者が会議開始に合わせて明示的に**開始するだけ**のキャプチャセッション状態と排他制御の公開面。OS ストリームの `CapturePhase`（`audio-capture-status.md`）とは分離し、開始 UI と mount 同期の正本とする。セッション途中の停止・停止時転写フラッシュ・再開は提供しない（Path A 要求更新）。

## Contract

### 型定義

```typescript
/** 利用者向けセッション状態（開始 UI の根拠） */
type CaptureSessionPhase = "idle" | "starting" | "active";

interface CaptureSessionState {
  session_phase: CaptureSessionPhase;
  /**
   * 開始遷移処理中。true の間は開始操作を拒否（要件 2.6）。
   */
  transition_busy: boolean;
  /** 現在の audio-capture フェーズ（表示・デバッグ用ミラー。権威は audio-capture-status） */
  capture_phase: "idle" | "starting" | "capturing" | "stopping" | "error";
  timestamp_ms: number;
}
```

### セマンティクス

| `session_phase` | 意味 | 会議音声 ingest |
|-----------------|------|-----------------|
| `idle` | 未開始（待機）— 起動完了後の初期値 | **しない**（要件 1.1–1.2） |
| `starting` | 待機→進行中の遷移中 | まだしない／開始処理中 |
| `active` | 利用者が開始した後の安定状態 | **する**（capture が `capturing` になる） |

- 起動完了後の初期値は `idle`。`transition_busy=false`（要件 1.1）。
- `active` 到達後、利用者操作による `idle` への復帰はない。終了はメインウィンドウ閉鎖によるアプリ終了時の既存 lifecycle のみ（要件 3.1, 6.4）。
- `transition_busy` は `starting` の間、または同一開始要求の処理中に `true`。
- 停止時即時転写フラッシュ用フィールド・コマンドは**廃止**（要件スコープ外）。

### Tauri Commands

| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `get_capture_session_state` | なし | `CaptureSessionState` | `INTERNAL` |
| `start_capture_session` | なし | `CaptureSessionState` | `TRANSITION_BUSY`, `CAPTURE_START_FAILED`, `UNSUPPORTED_PLATFORM`, `INTERNAL` |

**`start_capture_session` 挙動**:

1. `session_phase === "active"` → 二重開始せず現在状態を返す（要件 2.5）。
2. `transition_busy` または `starting` → `TRANSITION_BUSY`（要件 2.6）。
3. 非対応 OS → `UNSUPPORTED_PLATFORM`（`message_ja` / `action_ja` は `audio-capture-status` と同型パターン）（要件 1.3）。
4. 内部で `CaptureOrchestrator::start_with_selection` + processing hooks（既存 lifecycle パス）。成功で `active`。
5. 開始失敗 → `CAPTURE_START_FAILED`、`session_phase` は `idle` 維持（要件 2.7）。
6. 成功時は `capture-session://state-changed` を emit。

| code | 条件 | recoverable |
|------|------|-------------|
| `TRANSITION_BUSY` | 開始遷移中の二重操作 | true |
| `CAPTURE_START_FAILED` | 開始できず capture が idle/error のまま | true |
| `UNSUPPORTED_PLATFORM` | Linux 等 | false |
| `INTERNAL` | 想定外 | true |

**非提供（破壊的削除）**: `set_capture_session_active` — 停止・トグル用途は要求外。実装移行期間のエイリアスは設けない。

### Tauri イベント

#### `capture-session://state-changed`

```typescript
interface CaptureSessionStateChanged {
  state: CaptureSessionState;
}
```

- `session_phase` または `transition_busy` の変化時に発行
- フロントは mount 時に `get_capture_session_state` で同期（既存 `get_capture_phase` パターンに倣う）

### Threat model

- ローカル単一ユーザー。AuthN/AuthZ なし（requirements-review Sec）。
- Tauri capability で `get_capture_session_state` / `start_capture_session` を許可リスト化。
- 他プロセスからの invoke 乱用は capability 境界で抑止；詳細は既存 Tauri ACL パターンに委譲（requirements-review Decisions）。
- ログに PCM・転写全文を含めない。

## Non-goals

- セッション途中停止・停止時 flush 進行フラグ
- `capture-audio-controls` の ingest トグル／ゲイン契約の変更
- 保存ファイル形式・`save_transcript_session` 形状の変更
- クラウド ASR
- PCM 保持上限到達時の自動キャプチャ停止の再設計（既存 product 安全装置）

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-19 | Path A — 開始専用 IPC（`start_capture_session`）、phase を `idle`/`starting`/`active` に縮小、`stop_flush_in_progress` と `set_capture_session_active` 削除 | ADR-0015 |
| 2026-09-19 | 初版 — セッション IPC・停止 flush 進行フラグ（**本日 Path A により置換**） | — |

## Notes

- TypeScript ミラー: `src/presentation/hooks/capture-session-types.ts`、`src/infrastructure/tauri/captureSessionCommands.ts`
- `CapturePhase` の権威契約は引き続き `audio-capture-status.md`
