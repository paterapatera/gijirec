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
| `whisper-transcribe`（将来） | `audio-capture` の `PcmChunk` 契約 | 下流は PCM バスのみ依存。キャプチャ実装に直接依存しない |

## audio-capture ドメイン境界

`docs/specs/audio-capture/` の設計に基づく。契約の正本は `docs/contracts/` を参照。

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
| `whisper-transcribe`（将来） | `PcmChunk` 契約のみ | キャプチャ実装 crate に直接依存しない |

## 境界メモ

- 契約面の正本は `docs/contracts/` の各ファイル
- 重要判断は `docs/architecture/adr/`
- PCM チャンク形状変更は `whisper-transcribe` の再検証トリガー
