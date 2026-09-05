# whisper-transcribe-status

- **Surface type**: Event
- **Owners / Domains**: whisper-transcribe
- **Related ADR**: docs/architecture/adr/ADR-0003-whisper-cpp-plus-streaming.md

## Purpose

文字起こしライフサイクル状態、モデル取得進捗、利用者向けエラー通知のイベント契約。フロントエンドおよび将来の診断 UI が購読する。

## Contract

### 状態列挙 `TranscribePhase`

| 値 | 意味 |
|----|------|
| `idle` | 未開始または完全停止 |
| `loading_model` | モデル初回取得またはローカル読み込み中 |
| `ready` | モデル利用可能、キャプチャ待ち |
| `transcribing` | キャプチャ中かつ逐次推論稼働中 |
| `stopping` | 推論ワーカー停止・リソース解放中 |
| `error` | 回復不能またはユーザー介入が必要な停止 |

### Tauri イベント

#### `whisper-transcribe://phase-changed`

```typescript
interface TranscribePhaseChanged {
  phase: "idle" | "loading_model" | "ready" | "transcribing" | "stopping" | "error";
  timestamp_ms: number;
}
```

#### `whisper-transcribe://model-progress`

```typescript
interface ModelDownloadProgress {
  bytes_downloaded: number;
  bytes_total: number | null; // 不明時は null
  percent: number | null;     // 0–100。算出不能時は null
  status: "downloading" | "verifying" | "complete" | "failed";
}
```

`loading_model` フェーズ中に発行。`complete` 後は `ready` へ遷移。

#### `whisper-transcribe://error`

```typescript
interface TranscribeUserError {
  code:
    | "MODEL_DOWNLOAD_FAILED"
    | "MODEL_CORRUPT"
    | "MODEL_NOT_FOUND"
    | "INFERENCE_FAILED"
    | "UPSTREAM_CAPTURE_ERROR"
    | "INTERNAL";
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}
```

| code | 発火条件 | action_ja 例 |
|------|---------|--------------|
| `MODEL_DOWNLOAD_FAILED` | 初回モデル取得失敗（5.4） | ネットワーク接続を確認し、アプリを再起動して再試行してください |
| `MODEL_CORRUPT` | ローカルモデル読み込み不能（5.5） | 設定からモデルを再取得してください |
| `MODEL_NOT_FOUND` | モデルパス不在かつ取得不可 | アプリを再起動し、モデル取得を完了してください |
| `INFERENCE_FAILED` | 回復不能な推論エラー（8.1） | アプリを再起動してください。改善しない場合はモデル再取得を試してください |
| `UPSTREAM_CAPTURE_ERROR` | 上流 audio-capture が `error` へ遷移（8.3） | キャプチャエラーを解消後、文字起こしは自動再開します |
| `INTERNAL` | 想定外 | アプリを再起動してください。改善しない場合はログを共有してください |

### 禁止事項

- 技術コードのみの通知（8.2 違反）
- エラー通知・ログへの転写テキスト全文・PCM 生データの含有（8.4, 9.1）
- 認証・セッション状態の提供（9.3）

## Non-goals

- キャプチャフェーズイベント（`audio-capture-status` が所有）
- 編集ロック状態

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-05 | 初版 — フェーズ・モデル進捗・利用者向けエラー | 要件 5, 6, 8 |

## Notes

- `message_ja` / `action_ja` は日本語文字列を必須（spec.language: ja）
- 上流 `audio-capture://phase-changed` の `error` 受信時は新規 PCM 処理を停止し `UPSTREAM_CAPTURE_ERROR` を発行可能
