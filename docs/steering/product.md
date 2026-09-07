# Product Overview

gijirec は、Web 会議中にマイクとシステム音声を仮想オーディオデバイスなしで同時取り込みし、ローカル Whisper で低遅延に文字起こし、その場で手動編集して Markdown 保存できるデスクトップアプリ。

## Core Capabilities

1. **二重キャプチャ＋ミキシング** — マイクとスピーカー（システム音声）を 16kHz モノラル PCM にリアルタイム合成
2. **ローカル逐次文字起こし** — whisper.cpp による数秒遅延のストリーミングテキスト（タイムスタンプ付き）
3. **部分ロック付きエディタ** — 手動修正箇所を AI 上書きから保護し、タイムスタンプ構造を維持
4. **Markdown 出力** — 会議記録を `.md` として保存
5. **オフライン運用** — モデル初回取得後はインターネット不要

## Target Use Cases

- Web 会議のリアルタイム議事録作成（自分の発言と相手／PC 音声の両方を拾う）
- 仮想オーディオデバイス（BlackHole 等）を使いたくない／使えない環境での文字起こし
- クラウド STT に依存せず、ローカルで完結させたい会議・インタビュー記録

## Value Proposition

- **仮想デバイス不要** — OS ネイティブのループバック（Mac: ScreenCaptureKit 等 / Windows: WASAPI）でシステム音声を取得
- **低遅延・その場編集** — 録音後起こしではなく、発言から数秒以内にテキストが流れ、すぐ手直しできる
- **軽量・オフライン** — Python ランタイムやクラウド API に依存せず、会議の裏で OS を極端に重くしない
- **シンプルな起動・終了** — ダブルクリック起動、ウィンドウ閉じでキャプチャ・推論も完全停止

## Out of Scope

仮想オーディオデバイス前提の設計、クラウド音声認識、Python ランタイム、話者分離、Linux 対応。

## Related Docs

- 機能ロードマップと spec 依存順: `docs/steering/roadmap.md`
- 各機能の詳細: `docs/specs/{feature}/`

## Implementation Phasing

製品ビジョン全体に対し、実装は roadmap の spec 順に段階投入する。

表の「状態」列は `docs/steering/roadmap.md` と同期し、根拠は各 spec の `tasks.md` の `[x]` とする（`spec.json` の phase だけで未完了と判断しない）。

| Spec | 状態 | 備考 |
|------|------|------|
| audio-capture | 完了 | 二重キャプチャ・PCM ミックス・ライフサイクル・状態 UI |
| whisper-transcribe | 完了 | ローカル推論・モデル取得・フェーズ／進捗。ブロック供給は `whisper-transcribe://block-appended` |
| transcript-editor | 完了 | 二重エディタ・部分ロック・保存／設定 IPC |
| audio-device-selection | 完了 | マイク／スピーカー一覧・セッション選択・キャプチャ再開・`DeviceSelectorPanel` |
| release-logging | 完了 | リリースビルド `--log` 時の診断ログ永続化（`app_data_dir/logs/`） |
| fix-release-transcribe | 完了 | `app_data_dir` モデルパス（ADR-0008）、`block-appended` ACL、`TranscribeStallWatchdog`、compose 起動順序 |
| default-window-size | 完了 | `src-tauri/tauri.conf.json` — main ウィンドウ 1000×800 |

**現 UI の範囲**: キャプチャ／文字起こしフェーズ、モデル取得進捗、エラー表示（`message_ja` / `action_ja`）、マイク／スピーカー選択パネル、手書き＋AI 転写の二重エディタ、保存ツールバー・結果トースト。起動時ウィンドウは 1000×800。

---
_updated_at: 2026-09-07（完了状態ルールを追記）_
_Focus on patterns and purpose, not exhaustive feature lists_
