# audio-capture-status

- **Surface type**: Event
- **Owners / Domains**: audio-capture
- **Related ADR**: docs/architecture/adr/ADR-0001-platform-audio-capture.md

## Purpose

キャプチャライフサイクル状態と利用者向けエラー通知のイベント契約。フロントエンドおよび将来の診断 UI が購読する。

## Contract

### 状態列挙 `CapturePhase`

| 値 | 意味 |
|----|------|
| `idle` | 未開始または完全停止 |
| `starting` | デバイスオープン・権限確認中 |
| `capturing` | マイク＋システム音声の二重取得・ミックス稼働中 |
| `stopping` | リソース解放中 |
| `error` | 回復不能またはユーザー介入が必要な停止 |

### Tauri イベント

#### `audio-capture://phase-changed`

```typescript
interface CapturePhaseChanged {
  phase: "idle" | "starting" | "capturing" | "stopping" | "error";
  timestamp_ms: number;
}
```

#### `audio-capture://error`

```typescript
interface CaptureUserError {
  code:
    | "MIC_UNAVAILABLE"
    | "MIC_PERMISSION_DENIED"
    | "SYSTEM_AUDIO_UNAVAILABLE"
    | "SYSTEM_AUDIO_PERMISSION_DENIED"
    | "DEVICE_DISCONNECTED"
    | "INTERNAL";
  message_ja: string;       // 利用者向け短文
  action_ja: string;      // 次に取れる行動（5.4）
  recoverable: boolean;
}
```

| code | 発火条件 | action_ja 例 |
|------|---------|--------------|
| `MIC_UNAVAILABLE` | マイクデバイスなし（5.1） | マイク接続とシステム設定を確認してください |
| `MIC_PERMISSION_DENIED` | マイク権限拒否（7.1） | 設定 → プライバシー → マイクで gijirec を許可してください |
| `SYSTEM_AUDIO_UNAVAILABLE` | ループバック取得失敗（5.2） | 出力デバイスと OS バージョンを確認してください |
| `SYSTEM_AUDIO_PERMISSION_DENIED` | macOS 画面収録権限拒否（7.1） | 設定 → プライバシー → 画面とシステムオーディオ録音で許可してください |
| `DEVICE_DISCONNECTED` | キャプチャ中のデバイス切断（5.3） | デバイスを再接続してアプリを再起動してください |
| `INTERNAL` | 想定外（ログに詳細） | アプリを再起動してください。改善しない場合はログを共有してください |

### 禁止事項

- `SYSTEM_AUDIO_UNAVAILABLE` 時にマイクのみへサイレントフォールバックしない（5.2）
- 技術コードのみの通知（5.4 違反）

## Non-goals

- 文字起こし結果のイベント
- 認証・セッション状態

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-05 | 初版 — フェーズと利用者向けエラー | 要件 5, 7 |

## Notes

- `message_ja` / `action_ja` は presentation 層でローカライズ辞書から解決してもよいが、契約上は日本語文字列を必須とする（spec.language: ja）
