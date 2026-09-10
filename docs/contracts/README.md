# Contracts Index

永続する契約面（API / イベント / データ所有）の索引。feature 削除後も残る。

## 読み方（必須）

1. **最初にこの index だけ**読む
2. 必要な path を選び、**そのファイルだけ** Read する
3. index に無いファイルを「念のため」開かない。**全量 Read 禁止**

## Entries

| ID / Path | One-line purpose | Owners / Domains |
|-----------|------------------|------------------|
| `audio-capture-pcm.md` | 下流向け 16 kHz モノラル PCM チャンク形状・供給規約 | audio-capture |
| `audio-capture-status.md` | キャプチャフェーズ・利用者向けエラー Tauri イベント | audio-capture |
| `audio-device-selection.md` | マイク／スピーカー一覧・セッション選択 Tauri command / イベント | audio-device-selection |
| `whisper-transcribe-blocks.md` | 下流向けタイムスタンプ付きテキストブロック形状・追記供給規約 | whisper-transcribe |
| `whisper-transcribe-status.md` | 文字起こしフェーズ・モデル進捗・利用者向けエラー Tauri イベント | whisper-transcribe |
| `whisper-transcribe-settings.md` | kotoba-whisper バリアント選択の永続化・Tauri command | whisper-model-selection |
| `transcript-editor-save.md` | 議事録保存コマンド・JST サブディレクトリ・Markdown/JSONL 出力形状 | transcript-editor |
| `transcript-editor-settings.md` | 保存先ディレクトリ・JSONL 出力設定の永続化 | transcript-editor |
| `transcript-editor-status.md` | 保存・設定操作の利用者向けエラー形状 | transcript-editor |
| `release-logging-persistence.md` | リリースビルド診断ログの保存場所・セッション ID・禁止フィールド | release-logging |
| `capture-audio-controls.md` | マイク ingest トグル・手動ゲイン・ingest 直前 dBFS メーター Tauri command / イベント | capture-audio-controls |

<!-- 例:
| contracts/billing-api.md | Billing HTTP API shape | billing |
| contracts/auth-session.md | Session cookie / token shape | auth |
-->

## 命名

- `<domain>-<surface>.md`（例: `billing-api.md`, `auth-session.md`）
- 1 ファイル = 1 契約面（API / イベント / データ所有のいずれか）

## 禁止

- `docs/specs/{feature}/contracts/` を **永続契約の正本にしない**
- 設計下書きを feature 内に置いてもよいが、GO 前に本ディレクトリへ書く
- `docs/architecture/boundaries.md` に契約詳細をコピペしない
- 契約ファイルを追加したのに **Entries を更新しない**（index 欠落禁止）

## テンプレ

- `docs/settings/templates/contracts/contract.md`
