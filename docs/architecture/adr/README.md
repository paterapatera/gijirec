# ADR Index

Architecture Decision Records。重要な設計判断を永続化する。contracts / boundaries の「いま有効な正本」とは役割が異なる（理由・代替・置換履歴はここ）。

**1 判断 = 1 ファイル。本文のマージ編集で履歴を消さない。**

## 読み方（必須）

1. **最初にこの index だけ**読む
2. 設計/実装で必要な `ADR-NNNN-...` を **関連 1〜2 件**まで Read する
3. **全量 Read 禁止**。index に無いファイルを開かない。この README の Entries 表以外の本文をまとめて読まない
4. 実装の日常タスクでは ADR 不要（境界変更・契約破壊・技術選択のタスクのみ）

## いつ書くか

次のいずれかに該当したら **新規 ADR ファイル**を作る:

- 依存方向・所有境界の変更
- 公開契約の破壊的変更
- 採用/不採用の大きな技術選択（後から理由が必要）
- Revalidation Triggers に触れる判断

書かないもの:

- 局所的な実装詳細
- 後からコードとテストだけ見れば分かる命名

全判断を ADR 化する義務はない。

## 更新規則（Superseded）

- **本文のマージ編集で履歴を消さない**（既存 ADR へ判断を上書きマージしない）
- 判断を覆す手順:
  1. **新規 ADR** を採番して作成する
  2. 旧 ADR の Status を `Superseded by ADR-XXXX` に変更する（本文の Context/Decision/Consequences は消さない）
  3. **この README の Entries** に新 ADR を登録し、旧エントリの Status も `Superseded by ADR-XXXX` に更新する（index 欠落禁止）
  4. contracts / boundaries の更新とセットで、関連 ADR を design の Persistent References に載せる

**Index 同期（必須）**: ADR ファイルを追加・置換したら、必ず本 Entries を更新する。index に無い ADR を「念のため」開かせないため、ファイル作成と Entries 更新はセット。

## Entries

| ID / Path | One-line purpose | Owners / Domains | Status |
|-----------|------------------|------------------|--------|
| `ADR-0001-platform-audio-capture.md` | WASAPI ループバック + macOS ScreenCaptureKit による二重キャプチャ | audio-capture | Accepted |
| `ADR-0002-bun-frontend-toolchain.md` | Tauri ホストの Bun 採用（npm 非必須） | cross-cutting | Accepted |
| `ADR-0003-whisper-cpp-plus-streaming.md` | whisper-cpp-plus によるローカルストリーミング STT | whisper-transcribe | Superseded by ADR-0012 |
| `ADR-0012-batch-inference-schedule.md` | 30 秒固定バッチ推論スケジュール（VAD ストリーミングから移行） | transcribe-batch-interval | Accepted |
| `ADR-0004-whisper-model-kotoba.md` | デフォルト GGML モデルを kotoba-whisper-v2.2（Q5_0）に採用 | whisper-transcribe | Superseded by ADR-0010 |
| `ADR-0010-whisper-model-kotoba-q8.md` | デフォルト GGML モデルを kotoba-whisper-v2.2（Q8_0）に昇格 | whisper-transcribe | Superseded by ADR-0011 |
| `ADR-0011-whisper-model-kotoba-fp16.md` | デフォルト GGML モデルを kotoba-whisper-v2.2（FP16）に昇格 | whisper-transcribe | Accepted |
| `ADR-0005-slate-js-transcript-editor.md` | 部分ロック付き二重エディタに Slate.js を採用 | transcript-editor | Accepted |
| `ADR-0006-shadcn-ui-transcript-editor.md` | アプリ chrome（ツールバー・通知等）に shadcn/ui を採用 | transcript-editor | Accepted |
| `ADR-0007-release-file-logging.md` | release ビルド向け tracing-appender ファイル永続化 | release-logging | Accepted |
| `ADR-0008-model-store-app-data-dir.md` | Whisper ModelStore の保存先を Tauri app_data_dir に統一 | fix-release-transcribe | Accepted |
| `ADR-0009-macos-speaker-selection-strategy.md` | macOS スピーカー選択と SCK システムミックスの整合 | audio-device-selection | Accepted |
| `ADR-0013-whisper-model-variant-selection.md` | kotoba-whisper Q5_0 / Q8_0 / FP16 のユーザー選択 | whisper-model-selection | Accepted |

## 命名・採番

- `ADR-NNNN-short-title.md`（**ゼロ埋め 4 桁**。例: `ADR-0001-prefer-event-driven-billing.md`）
- 次番号 = Entries の最大 NNNN + 1（欠番を埋めない）
- テンプレ: `docs/settings/templates/architecture/adr.md`

## フォーマット（最小）

各 ADR は次を持つ（詳細はテンプレ）:

- Status: `Proposed` | `Accepted` | `Superseded by ADR-XXXX`
- Date / Feature
- Context / Decision / Consequences
- Alternatives considered（短く）

## 禁止

- `docs/specs/{feature}/adr/` を永続 ADR の正本にしない
- ADR 全文を implementer プロンプトへ常時注入しない
- 既存 ADR 本文へ新判断をマージ上書きしない
