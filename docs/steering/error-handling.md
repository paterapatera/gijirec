# Error Handling Standards

gijirec のエラー分類・変換・表示・ログの横断ルール。HTTP API ではなく **Tauri IPC + ローカルデスクトップ** 向け。

## Philosophy

- **内部と利用者向けを分離** — Rust domain に技術詳細、UI には日本語の説明と次のアクション
- **契約で安定化** — イベント payload の `code` は列挙固定。文言は `message_ja` / `action_ja`
- **境界で変換** — infrastructure で `CaptureError` を捕捉し、presentation で `UserFacingError` に変換して emit
- **fail closed** — 権限不足・デバイス不可時にサイレントフォールバックしない（例: システム音声のみ諦めてマイクだけ続行しない）

## Error Layers

```
OS / cpal / SCK
    ↓
CaptureError          (gijirec-domain — 内部列挙)
    ↓ to_user_facing()
UserFacingError       (domain — 契約 payload 形状)
    ↓ emit via Tauri
CaptureUserError      (フロント型ミラー — audio-capture://error)
    ↓
UI (message_ja + action_ja)
```

| 層 | 型 | 利用者に見えるか | ログに載せるか |
|----|-----|------------------|----------------|
| Internal | `CaptureError::Internal { detail }` | いいえ（`INTERNAL` + 汎用文言のみ） | はい（detail 含む） |
| Contract | `UserFacingError` / `CaptureUserError` | はい | code のみ（PCM・デバイス名は出さない） |
| Emit | `EmitError` | いいえ | はい（開発者向け） |

## Canonical User-Facing Shape

契約正本: `docs/contracts/audio-capture-status.md`

```typescript
interface CaptureUserError {
  code: "MIC_UNAVAILABLE" | "MIC_PERMISSION_DENIED" | ...;
  message_ja: string;   // 何が起きたか（短文）
  action_ja: string;    // 次に取れる行動（必須・非空）
  recoverable: boolean; // ユーザー操作で回復しうるか
}
```

### code 追加ルール

1. `docs/contracts/{domain}-status.md` に列挙と発火条件を追記
2. `UserFacingErrorCode`（Rust）と TS 型ミラーを同期
3. `CaptureError::to_user_facing()` にマッピング + ユニットテスト
4. `action_ja` は OS 設定パスを含む具体的な一文（権限系）

### recoverable の目安

| recoverable | 例 |
|-------------|-----|
| `true` | 権限拒否、デバイス未接続、一時的な unavailable |
| `false` | `INTERNAL` — 再起動・ログ共有を促す |

## Domain Mapping（Rust）

変換は **domain** の `CaptureError::to_user_facing()` に集約。presentation は変換ロジックを重複させない。

```rust
// infrastructure: OS 失敗 → CaptureError
// presentation events: CaptureError → emit_error → UserFacingError payload
let facing = error.to_user_facing();
CaptureUserErrorPayload {
    code: facing.code.as_str().to_string(),
    message_ja: facing.message_ja,
    action_ja: facing.action_ja,
    recoverable: facing.recoverable,
}
```

**禁止**: UI 向けイベントに `detail` やスタックトレースを含める。デバイス固有名称をそのまま `message_ja` に埋め込む。

## UI Presentation

- エラーパネルは `role="alert"`、`aria-live="polite"` で状態と併用
- `message_ja` と `action_ja` を **別要素** で表示（`error-message` / `error-action`）
- `code` は開発者向け。DOM テキストや利用者向けラベルに出さない
- `recoverable: true` でも自動リトライは v1 では行わない（ユーザーが設定を直す前提）

## Logging & Observability

- **tracing ターゲット**: `gijirec_capture`、`gijirec_transcribe`、`gijirec_editor`、`gijirec_device`
- **presentation 層**: `CaptureObservability` トレイト経由。マクロは host（`run()`）側で実装（bylaw 対策）
- **ログに含める**: phase 遷移、`capture_buffer_drops_total`、error code、correlation `session_id`
- **ログに含めない**: PCM サンプル配列、会議内容、マイクデバイス表示名の生文字列

```rust
// 良い例: ポート名 + CaptureError 列挙
log_stream_open_failure("mic", &CaptureError::MicUnavailable, session_id);

// 悪い例: デバイス列挙結果をそのまま info! に流す
```

## Propagation Rules（feature 横断）

capture と transcribe で同パターンを踏襲:

| 層 | 責務 |
|----|------|
| infrastructure | 外部失敗を domain の typed error にラップ |
| domain | `to_user_facing()` または同等の変換。文言の正本 |
| presentation | Tauri event / command の emit。変換は domain に委譲 |
| `src/presentation` hooks | 契約型ミラー + 購読。ビジネスロジックなし |

**Transcribe 固有**: `TranscribeError::to_user_facing()` → `whisper-transcribe://error`。上流キャプチャ `error` は `UPSTREAM_CAPTURE_ERROR` で伝播。

**Editor 固有**: `EditorError::to_user_facing()`。保存／設定失敗はイベントではなく command 結果の `error` フィールド。UI は `SaveResultToast`（Sonner）で `message_ja` / `action_ja` を出す。

**Device selection 固有**: 選択デバイス不可は `audio-capture://error` 経由（`SELECTED_MIC_UNAVAILABLE` / `SELECTED_SYSTEM_AUDIO_UNAVAILABLE` / `MACOS_OUTPUT_NOT_DEFAULT`）。command 側の `INVALID_DEVICE` は `set_device_selection` 失敗時。サイレントフォールバック禁止（別デバイスへ自動切替しない）。

## Retry

- **音声キャプチャ**: リアルタイムコールバック内での自動リトライなし。失敗は phase `error` + イベント
- **Whisper**: 推論キューはワーカースレッド内で処理。失敗は phase `error` + `whisper-transcribe://error`
- **保存**: `SaveOrchestrator` の `isSaving` ガード。進行中の二重保存はしない。部分失敗は `SAVE_PARTIAL_FAILURE`
- **Tauri emit 失敗**: `EmitError` をログ。UI には既に phase が `error` なら二重通知しない

## Testing Requirements

- 全 `CaptureError` 変種が契約 `code` にマップされること（domain テスト）
- 各 code で `action_ja` が非空（domain テスト）
- UI が `message_ja` / `action_ja` を表示し、code を DOM に出さない（presentation テスト）
- observability が PCM ダンプ風文字列を記録しない（統合テスト）

詳細: `docs/steering/testing.md`

## Related

- 契約: `docs/contracts/audio-capture-status.md`
- セキュリティ（ログ・データ）: `docs/steering/security.md`
- 境界（外部送信なし）: `docs/architecture/boundaries.md`

---
_updated_at: 2026-09-07（Sync: デバイス選択エラー・gijirec_device tracing を反映）_
_Focus on patterns and decisions, not every error variant._
