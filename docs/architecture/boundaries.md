# Boundaries

全体の境界と依存方向のみ。詳細契約は `docs/contracts/` を参照する。

## 依存方向

| From | To | Rule |
|------|-----|------|
| `src/presentation` | `src/application`, `src/infrastructure`（IPC アダプタのみ） | TS レイヤ一方向 |
| `src/application` | `src/domain` | ユースケースはドメイン型のみ |
| `src/infrastructure` | `src/domain` | アダプタはドメイン型を実装 |
| `src/*` | `src-tauri/*` | **禁止** — Tauri IPC 経由のみ |
| `gijirec-presentation` | `gijirec-application`, `gijirec-infrastructure` | Rust composition root |
| `gijirec-application` | `gijirec-domain` | オーケストレーション |
| `gijirec-infrastructure` | `gijirec-domain` | OS / cpal / SCK アダプタ |
| `gijirec-domain` | 他 crate | **禁止** |
| `whisper-transcribe` | `audio-capture` の `PcmChunk` 契約 | 下流は PCM バスのみ依存。キャプチャ実装に直接依存しない |

## audio-capture ドメイン境界

`docs/contracts/audio-capture-*.md` および ADR-0001 に基づく。契約の正本は `docs/contracts/` を参照。手動検証は `docs/manual/audio-capture/`。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| マイクおよびシステム音声の取得 | `MicCaptureAdapter`（cpal）、`WindowsLoopbackAdapter`（WASAPI loopback）、`MacScreenCaptureKitAdapter`（SCK） |
| リサンプル・レベル調整・ミックス | `MonoResamplerPipeline`、`DefaultAudioMixer` / `AudioMixer`、`ChunkEmitter` |
| PCM ストリーム生成と下流バス | `PcmChunk`、`PcmChunkBus`（契約: `audio-capture-pcm.md`） |
| キャプチャ開始・停止 | `DefaultCaptureOrchestrator`、Tauri lifecycle（起動開始・終了停止） |
| キャプチャフェーズと利用者向けエラー | `CapturePhase`、`CaptureError` → UI イベント（契約: `audio-capture-status.md`） |
| Tauri ホスト最小 UI | React キャプチャステータス表示、`useCaptureStatus` |
| ツールチェーン | Bun（`dev` / `build` / `check`） |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| 文字起こし推論・テキスト表示 | 下流 spec（`whisper-transcribe` 等）が所有 |
| モデルダウンロード、Markdown 出力 | 同上 |
| ユーザー認証・認可 | 本 spec の範囲外 |
| 音声データのディスク永続化 | PCM をファイル保存しない |
| 音声データの外部ネットワーク送信 | キャプチャ実装から外部送信しない |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| OS API | Windows WASAPI loopback（cpal）、macOS ScreenCaptureKit、マイク入力（cpal） |
| Rust crates | `cpal`、`rubato`、`rtrb`（`tokio` は将来の非同期境界用に許可） |
| デスクトップ | Tauri 2 lifecycle / events |
| フロント | Bun、Vite、React |
| 下流 | **なし**（先頭 spec）— 下流は `PcmChunk` 契約のみを消費 |
| 前提条件 | **仮想オーディオデバイス（BlackHole 等）は非前提** — 要件 1.4 に従い、OS 標準 API のみでマイク＋システム音声を取得（仮想デバイスを前提条件としない） |

### 依存方向（audio-capture 内）

| From | To | Rule |
|------|-----|------|
| `gijirec-presentation` | `gijirec-application`, `gijirec-infrastructure` | composition root |
| `gijirec-application` | `gijirec-domain` | オーケストレーション |
| `gijirec-infrastructure` | `gijirec-domain` | OS / cpal / SCK アダプタ |
| `gijirec-domain` | 他 crate | **禁止** |
| `whisper-transcribe` | `PcmChunk` 契約のみ | キャプチャ実装 crate に直接依存しない |

## audio-device-selection ドメイン境界

`docs/contracts/audio-device-selection.md` および ADR-0009 に基づく。`audio-capture` の拡張（Path D）。手動性能記録は `docs/manual/audio-device-selection/performance-results.md`。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| デバイス列挙・ホットプラグ通知 | `AudioDeviceEnumerator`（cpal 入出力一覧）、`audio-device-selection://devices-changed`（UI 表示中のみ） |
| セッション内選択状態 | `DeviceSelectionStore`（非永続、`DeviceSelection`） |
| 選択変更オーケストレーション | `DeviceSelectionService`（一覧更新・選択・再キャプチャ起動） |
| Tauri デバイス選択 IPC | `list_audio_devices` / `get_device_selection` / `set_device_selection` / `set_audio_device_ui_visible`、`audio-device-selection://selection-changed` / `devices-changed`（契約: `audio-device-selection.md`） |
| デバイス選択 UI | `DeviceSelectorPanel`、`useAudioDevices`（React） |
| 選択デバイスでのキャプチャ | `CaptureOrchestrator` 拡張（選択 ID 伝播・`restart_with_selection`） |
| 選択デバイス文脈の利用者向けエラー | `CaptureError` → `audio-capture://error`（`SELECTED_MIC_UNAVAILABLE` 等、`audio-capture-status.md` 拡張） |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| PCM 形状・ミキシング・チャンク生成 | audio-capture が所有（`PcmChunk` 形状は変更しない） |
| 選択のセッション跨ぎ永続化 | 要件スコープ外 |
| 文字起こし・エディタ・Markdown 保存 | 下流 spec |
| ユーザー認証・認可 | 本 spec の範囲外 |
| デバイス一覧・選択内容の外部ネットワーク送信 | **禁止** |
| 仮想オーディオデバイス作成 | product スコープ外 |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| 上流 | `audio-capture` の `CaptureOrchestrator`、各 Capture アダプタ、`TauriLifecycleHook` |
| OS API | cpal 入出力列挙・ストリーム、Windows WASAPI ループバック（ADR-0001）、macOS ScreenCaptureKit（ADR-0001, ADR-0009） |
| 契約 | `audio-capture-pcm.md`（参照のみ）、`audio-capture-status.md`（modify）、`audio-device-selection.md`（modify） |
| 下流 | **なし** — whisper-transcribe は PCM 契約のみ消費 |

### 依存方向（audio-device-selection 内）

| From | To | Rule |
|------|-----|------|
| `gijirec-presentation` | `gijirec-application`, `gijirec-infrastructure` | Tauri command / イベント |
| `gijirec-application` | `gijirec-domain` | `DeviceSelectionService` |
| `gijirec-infrastructure` | `gijirec-domain` | `AudioDeviceEnumerator` |
| `src/presentation` | `src/infrastructure`（Tauri invoke ミラー） | TS レイヤ |
| `audio-device-selection` | whisper-transcribe 実装 | **禁止** — PCM バスのみ下流 |

## whisper-transcribe ドメイン境界

`docs/contracts/whisper-transcribe-*.md` および ADR-0003 / ADR-0004 に基づく。手動性能記録は `docs/manual/whisper-transcribe/performance-results.md`。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| PCM 消費と推論ウィンドウ蓄積 | `PcmIngestConsumer`（`PcmChunkConsumer` 実装）、`rtrb::RingBuffer` |
| ローカル Whisper 推論 | `WhisperCppAdapter`（whisper-cpp-plus）、`TranscribeWorker` |
| テキストブロック生成と下流供給 | `TranscriptBlock`、`BlockEmitter`、`TranscriptBlockBus`（契約: `whisper-transcribe-blocks.md`） |
| 音声認識モデルの取得・検証 | `ModelStore`、`ModelDownloader` |
| 文字起こしフェーズと利用者向けエラー | `TranscribePhase`、`TranscribeError` → UI イベント（契約: `whisper-transcribe-status.md`） |
| 推論ライフサイクル | `TranscribeLifecycleHook`（キャプチャ連動・アプリ終了時完全停止） |
| 最小 UI | モデル取得進捗・文字起こしステータス表示、`useTranscribeStatus` |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| 音声キャプチャ・ミキシング | 上流 audio-capture が所有 |
| 手動編集 UI・部分ロック・Markdown 出力 | 下流 transcript-editor が所有 |
| 転写テキストの永続保存 | transcript-editor 以降 |
| 話者分離・クラウド STT | product スコープ外 |
| ユーザー認証・認可 | 本 spec の範囲外 |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| 上流契約 | `PcmChunk`（`audio-capture-pcm.md`）、`CapturePhase` イベント（`audio-capture-status.md`） |
| Rust crates | `whisper-cpp-plus`（ADR-0003）、`rtrb`（PCM ワーカー間バッファ） |
| デスクトップ | Tauri 2 lifecycle / events |
| フロント | Bun、Vite、React（ステータス UI のみ） |
| ネットワーク | モデル初回取得の HTTPS のみ（要件 9.1） |
| 下流 | **なし**（transcript-editor は `TranscriptBlock` 契約のみを消費） |

### 依存方向（whisper-transcribe 内）

| From | To | Rule |
|------|-----|------|
| `gijirec-presentation` | `gijirec-application`, `gijirec-infrastructure` | composition root |
| `gijirec-application` | `gijirec-domain` | オーケストレーション |
| `gijirec-infrastructure` | `gijirec-domain` | whisper.cpp / モデル I/O アダプタ |
| `gijirec-domain` | 他 crate | **禁止** |
| `gijirec-*` | `audio-capture` 実装 crate | **禁止** — `PcmChunk` 契約と Tauri イベントのみ |

## transcript-editor ドメイン境界

`docs/contracts/transcript-editor-*.md` および ADR-0005 / ADR-0006 に基づく。手動検証は `docs/manual/transcript-editor/validation-checklist.md`。

### Owns（この Spec が所有）

**責務分割**: 編集 UI とセッション内状態は **TypeScript**（`src/*`）が所有。Markdown / JSONL のファイル I/O と Tauri 保存コマンドは **Rust**（`gijirec-presentation` / `gijirec-application`）が所有。永続化は invoke スナップショット経由のみ。

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| 上流転写ブロックのリアルタイム表示（TS） | `useTranscriptBlocks`（`whisper-transcribe://block-appended` 購読）、AI 転写 Slate エディタ |
| 手動議事録エディタ（TS） | 独立 Slate エディタ、`HandwritingEditor` |
| 部分ロック（選択・入力箇所）（TS） | `withLockedRanges` プラグイン、`locked` mark |
| ストリーミング追記のレイアウト安定 | `withAppendOnlyBlocks`、スクロールアンカー CSS |
| 保存先設定・JSONL 出力設定 | `EditorSettings` 永続化（契約: `transcript-editor-settings.md`） |
| Markdown / JSONL ファイル出力（Rust） | `SaveService`、Tauri `save_transcript_session`（契約: `transcript-editor-save.md`） |
| 保存・設定エラー通知 | `EditorUserError`（契約: `transcript-editor-status.md`） |
| アプリ chrome（ツールバー・通知・設定 UI） | shadcn/ui コンポーネント、`EditorToolbar`、`SaveResultToast` |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| 音声キャプチャ・Whisper 推論・モデル取得 | 上流 spec が所有（要件 1.5） |
| 転写ブロック生成・供給規約 | whisper-transcribe が所有（本 spec は `block-appended` 購読のみ） |
| ロック状態・手動議事録・手動修正の上流返送 | whisper-transcribe へ送信**禁止**（要件 3.5） |
| 転写テキスト・手動議事録の外部ネットワーク送信 | **禁止**（要件 10.1） |
| 清書の自動マージ | product スコープ外 |
| クラウド同期・ユーザー認証 | product スコープ外 |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| 上流契約 | `TranscriptBlock` / `whisper-transcribe://block-appended`（`whisper-transcribe-blocks.md`） |
| 上流イベント | `whisper-transcribe://phase-changed`、`whisper-transcribe://error`（ステータス表示・保存継続判断） |
| フロント | Slate.js（編集面、ADR-0005）、shadcn/ui（chrome、ADR-0006）、React 19、Tailwind CSS、Tauri IPC |
| Rust | Tauri 2 command / dialog、std::fs（保存 I/O のみ） |
| ネットワーク | **なし** — 本 spec から外部送信禁止 |

### 依存方向（transcript-editor 内）

| From | To | Rule |
|------|-----|------|
| `src/presentation` | `src/application`, `src/infrastructure` | TS レイヤ一方向 |
| `src/application` | `src/domain` | ユースケースはドメイン型のみ |
| `src/infrastructure` | `src/domain` | Tauri invoke アダプタ |
| `src/*` | `src-tauri/*` 直接 import | **禁止** — IPC 経由のみ |
| `gijirec-presentation` | `gijirec-application`, `gijirec-domain` | 保存コマンド handler |
| `gijirec-application` | `gijirec-domain` | SaveService / SettingsService |
| `gijirec-*` | whisper-transcribe 実装 crate | **禁止** — Tauri イベントと契約型のみ |

## release-logging ドメイン境界

ADR-0007 および `docs/contracts/release-logging-persistence.md` に基づく。運用手順は `docs/manual/release-logging/operations.md`。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| リリースビルド診断ログのファイル永続化 | ホスト crate `logging/` モジュール、`init_tracing()` の Registry 構成、**`--log` CLI opt-in** |
| ログ保存場所・セッション識別 | `{app_data_dir}/logs/sessions/{run_session_id}/`、 `latest-session.txt` |
| 運用者向け収集手順 | `docs/manual/release-logging/operations.md` |
| 永続化失敗時の degrade | 非ブロッキング継続 + diagnostic surface |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| observability イベント定義・マスキング | 各ドメイン spec / presentation observability が正本。本 spec は subscriber で転写のみ |
| クラウド送信・APM・UI ログビューア | 要件スコープ外 |
| ログローテーション / 自動削除 | 設計委任・将来 spec 可 |
| フロントエンド IPC | ログ閲覧 API は提供しない |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| Rust crates | `tracing`, `tracing-subscriber`, `tracing-appender`（ホスト crate のみ） |
| Tauri | `app.path().app_data_dir()` |
| 上流 | 既存 `Tracing*Observability` 実装（変更せず再利用） |
| ネットワーク | **なし** |

### 依存方向（release-logging 内）

| From | To | Rule |
|------|-----|------|
| `src-tauri` ホスト | `gijirec-presentation` observability 登録 API | composition root のみ |
| `gijirec-presentation` | `tracing-appender` | **禁止** — bylaw / レイヤ分離 |
| 各ドメイン observability | ファイル I/O | **禁止** — subscriber 一元化 |

## fix-release-transcribe ドメイン境界

ADR-0008 およびホスト composition のリリースパリティ修正に基づく。公開契約形状は変更せず、リリースビルドでの動作等価性を回復する。手動 smoke は `docs/manual/fix-release-transcribe/smoke-checklist.md`。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| リリースビルドでの転写パイプライン動作回復 | ホスト `compose.rs` / `lib.rs` の setup 順序・`app_data_dir` 注入 |
| モデル保存パス正本化 | ADR-0008、`ModelStore` 初期化タイミング |
| Tauri イベント ACL 整合 | `permissions/allow-listen-transcribe-events.toml`（`block-appended` 含む） |
| 転写停滞検知 | `TranscribeStallWatchdog`（presentation または lifecycle 拡張） |
| リリース向け検証 | `event_permissions.rs` 拡張、`docs/manual/fix-release-transcribe/smoke-checklist.md` |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| whisper 推論アルゴリズム・モデル選定 | whisper-transcribe / ADR-0003, ADR-0004 |
| ログ永続化実装 | release-logging が所有 |
| 新規 IPC / 契約イベント形状 | 本 spec では既存契約のみ使用 |
| Linux 対応 | スコープ外 |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| 上流 | whisper-transcribe 全コンポーネント（変更はパリティ修正に限定） |
| 上流 spec | release-logging（`--log` 診断手順） |
| Tauri | `app.path().app_data_dir()`、capabilities / permissions |
| 契約 | `whisper-transcribe-blocks`, `whisper-transcribe-status`, `release-logging-persistence`（参照のみ） |

## 境界メモ

- 契約面の正本は `docs/contracts/` の各ファイル
- 重要判断は `docs/architecture/adr/`
- PCM チャンク形状変更は `whisper-transcribe` の再検証トリガー
- `TranscriptBlock` 形状変更は `transcript-editor` の再検証トリガー
- observability イベント形状または禁止フィールド方針の変更は `release-logging` の再検証トリガー
- v1 feature spec（`docs/specs/`）は 2026-09-07 に全件アーカイブ済み。境界・契約・手動検証は本ファイル / `docs/contracts/` / `docs/manual/` を正本とする
