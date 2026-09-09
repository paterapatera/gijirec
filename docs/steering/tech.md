# Technology Stack

## Architecture

Tauri デスクトップアプリ。Rust バックエンドが OS ネイティブの音声キャプチャ・Whisper 推論を担い、TypeScript の Web UI がエディタ体験を提供する。

Rust 側は **レイヤードアーキテクチャ**（domain → application / infrastructure → presentation）で、依存方向を `cargo bylaw` で強制する。TypeScript 側も同じレイヤ分離を `dependency-cruiser` で強制する（`src/` は `src-tauri/` に依存しない）。

## Core Technologies

- **Frontend**: TypeScript（strict）、React 19、Vite 8、Tauri 2 IPC（`@tauri-apps/api`）
- **Backend**: Rust（edition 2024、stable toolchain）
- **Desktop Shell**: Tauri 2（`cargo tauri` CLI）
- **STT**: `whisper-cpp-plus` 0.1（ADR-0003）+ kotoba-whisper-v2.2 の Q5_0 / Q8_0 / FP16 選択（ADR-0013。論理既定 FP16 / ADR-0011）
- **Audio**: cpal（マイク / Windows ループバック）、screencapturekit（macOS システム音声）、rubato（リサンプル）、rtrb（スレッド間バッファ）— ADR-0001 準拠
- **Runtime**: Bun >= 1.2（フロントエンドパッケージマネージャ・スクリプト実行。npm 非前提）

## Key Libraries

影響する開発パターンに限定:

| 領域 | 技術 | 役割 |
|------|------|------|
| 音声キャプチャ | cpal、screencapturekit（macOS）、WASAPI loopback（Windows）、rubato、rtrb | マイク＋システム音声の二重取り込み・16 kHz ミックス |
| キャプチャ配信 | `PcmChunkBus`（presentation） | 100 ms チャンクの下流 consumer 向けバックプレッシャー付き配信 |
| 文字起こし | `whisper-cpp-plus`（`WhisperCppAdapter`） | 30 秒固定バッチ推論（ADR-0012）。専用ワーカースレッド + rtrb。推論中も PCM 非破棄蓄積 |
| モデルバリアント | `ModelVariantCatalog` + `ModelOrchestrator` | Q5_0 / Q8_0 / FP16 の path / URL / SHA-256 正本。`transcribe-settings.json` で永続化（ADR-0013） |
| 転写ブロック配信 | `TranscriptBlockBus`（presentation） | `whisper-transcribe://block-appended` で追記のみ配信 |
| マウント同期 | `TranscribeStatusCache`（presentation） | モデル取得中でもブロックしないフェーズ／進捗スナップショット |
| エディタ | Slate.js（編集面）+ shadcn/ui + Sonner | 部分ロック付き二重エディタ（ADR-0005 / ADR-0006） |
| 保存ダイアログ | `@tauri-apps/plugin-dialog` | 保存先ディレクトリ選択（`pick_save_directory`） |
| デバイス選択 | `DeviceSelectionService` + cpal 列挙 | セッション内マイク／スピーカー選択・キャプチャ再開（ADR-0009） |
| 診断ログ | `src-tauri/src/logging/`（`--log`） | リリースビルドのファイル永続化（ADR-0007）。`app_data_dir/logs/` |
| アーキテクチャ検証 | cargo bylaw、dependency-cruiser | レイヤ依存の自動チェック |

### Whisper 推論（バッチスケジュール）

- スケジュール: 前サイクル完了から **30 秒**（`BATCH_INTERVAL`）。未処理 PCM ≥ 480k samples でも起動。バックログ残存時は連続サイクル（ADR-0012）
- 窓長: `MAX_INFERENCE_WINDOW_SAMPLES = 480_000`（30 s @ 16 kHz）。停止時は残 PCM を最終バッチ flush
- **転写 ingest ゲイン**: `PcmIngestConsumer` で `TRANSCRIBE_INGEST_GAIN = 1.25` + `TRANSCRIBE_SOFT_LIMIT = 0.95`（ミキサー非変更。推論窓 −18〜−17 dBFS 目標）。定数は `pcm_ingest_consumer.rs` に単一定義（1.45 から実機ログで調整済み）
- **RMS 可観測性**: `transcribe_window_rms_dbfs` / ingest サマリ RMS はゲイン後 PCM（rtrb 上 f32）を反映。worker の window RMS 計測も ingest 後サンプルに対して行う
- 選択バリアント: `q5_0` / `q8_0` / `fp16`（契約: `whisper-transcribe-settings.md`）。論理既定は FP16（既存 `kotoba-whisper-v2.2-ggml.bin` を追加 DL なしで互換）
- 転写中切替: `pending_variant` を次バッチサイクル（`on_batch_cycle_started`）で適用。同一バリアント再選択は no-op（永続化のみ）
- 可観測性: `batch_cycle_started` / `batch_cycle_completed`、`transcribe_pcm_backlog_seconds`、`transcribe_rtrb_overflow_count`（ingest 共有 `AtomicU64` を worker がサイクル開始時に読む。PCM 全文・転写全文はログに出さない）
- スレッド数: CPU コア数に応じた動的設定、**上限 4**
- VAD 区切り定数（`TRAILING_SILENCE_FRAMES` 等）はユニットテスト用レガシー経路に残存。本番は `take_batch_window` 経路

### Whisper 推論パラメータ（レガシー VAD 経路の調整時）

- 窓長・スレッド・VAD は **1 軸ずつ** 変更し、`bun run verify` + 実機で確認してから次へ
- 繰り返し発話・区切り不良は、窓長変更と `single_segment` / `entropy_thold` を同時に変えない
- 大きく戻す前に revert 条件をメモする（調整セッションで全 revert が起きやすい）

## Development Standards

### Type Safety

- TypeScript `strict: true` + `noUncheckedIndexedAccess`、`exactOptionalPropertyTypes`
- ESLint `strictTypeChecked` + `@typescript-eslint/no-explicit-any: error`
- `consistent-type-imports` で型 import を分離

### Code Quality

- **Format**: Biome（`bun run format`）— フロントエンドのみ、`src-tauri` は除外
- **Lint**: ESLint + sonarjs（フロント）、Clippy `-D clippy::all`（Rust）
- **Complexity**: 関数 80 行・パラメータ 4 個・認知複雑度 15 を warn 上限
- **Dead code**: knip（TS）、cargo machete（Rust）

### Architecture Enforcement

```bash
bun run arch          # dependency-cruiser（TS レイヤ）
bun run rust:arch     # cargo bylaw（Rust レイヤ）
```

### Testing

- **Frontend**: Bun 組み込みテスト（`bun:test`）+ happy-dom + Testing Library。フックは injectable `listenFn` / `invokeFn` で Tauri なし単体テスト
- **Rust**: crate 内ユニットテスト、presentation の統合テスト（合成 rtrb・パイプラインスモーク）
- **品質ゲート**: `bun run verify` が完成判定（`check` + `test` + `test:arch` + `rust:check` + `rust:test`）。`bun run check` は format / typecheck / lint / arch（depcruise）/ knip。`bun run test` は `src/{presentation,application,domain,infrastructure}`。`bun run test:arch` は `scripts/verify-depcruise-layers.test.ts`。長時間性能・E2E は手動チェックリスト（CI 対象外）
- **方針**: 契約形状は `docs/contracts/` を正本とし、feature spec の Validation フェーズでテストを追加

## Development Environment

### Required Tools

- [Bun](https://bun.sh/) >= 1.2（依存インストール・フロントスクリプト。`bun.lock` がロックファイル）
- Rust stable（`rust-toolchain.toml` 準拠、rustfmt / clippy 付き）
- [cargo-tauri](https://v2.tauri.app/reference/cli/) 2.x
- cargo-bylaw、cargo-machete（`rust:arch` / `rust:dead-code` 用）

### Common Commands

```bash
# 完成判定（lint + test 一式）
bun run verify

# Frontend quality gate
bun run check          # format + typecheck + lint + arch + knip
bun run test           # src/{presentation,application,domain,infrastructure}
bun run test:arch      # depcruise layer fixture

# Rust quality gate
bun run rust:check     # fmt + check + clippy + bylaw + machete
bun run rust:test      # cargo test --workspace

# Desktop dev
cd src-tauri && cargo tauri dev   # beforeDevCommand で bun run dev を自動実行

# Individual
bun run typecheck
bun run rust:typecheck
```

### Toolchain & build gotchas

実装で繰り返し遭遇するビルド・品質ゲートの注意点:

- **Windows `tauri-build`**: `bundle.icon` が空でも `icons/icon.ico` を要求する（1×1 プレースホルダで `cargo check` 通過）
- **knip entry**: Vite エントリ（`src/main.ts`）に合わせる。フロント品質ゲートは `bun run check`
- **cargo-bylaw 0.1.0**: rustc **1.95+** が必要。Tauri ホスト（`generate_context!`）は bylaw 解析対象外。`-p gijirec-domain -p gijirec-application -p gijirec-infrastructure -p gijirec-presentation` でレイヤ crate のみ検証
- **Windows テスト**: `bun test` 一括が depcruise fixture と干渉しうるため、arch fixture は **`bun run test:arch`** を品質ゲートに使う
- **Vite + React プラグイン**: `@vitejs/plugin-react` **6** は Vite **8** 専用（`vite/internal`）。Vite 7 では 5.x、Vite 8 では 6.x を組にする
- **tailwindcss**: shadcn の `tailwind.config.ts` 互換のため **v3.4.19** にピン留め（`bun add` が v4 を解決しうる）
- **Rust typecheck / test**: Cursor の一時 `CARGO_TARGET_DIR` だと whisper-cpp-plus-sys の cmake が失敗する。`src-tauri/.cargo/config.toml` で `target` を固定し、ルートから `--manifest-path src-tauri/Cargo.toml` で実行。**`CARGO_TARGET_DIR=src-tauri/target` は `src-tauri/src-tauri/target` を誤生成するので不可**
- **compose 統合テスト**: `cargo test -p gijirec -- compose::`（`-p gijirec-presentation` ではマッチしない）
- **契約テスト配置**: `gijirec-infrastructure` → `gijirec-application` の dev-dep は bylaw 違反。ブロック供給の契約テストは `block_emitter.rs` 側に置く

## Key Technical Decisions

| 判断 | 理由 |
|------|------|
| Tauri + Rust | OS ネイティブ音声 API への直接アクセス、単一バイナリ配布 |
| whisper.cpp（Python なし） | オフライン完結、ランタイム依存を最小化 |
| レイヤード crates / src 構造 | ドメイン境界をコンパイル時・CI 時に強制 |
| 仮想デバイス不使用 | ユーザー設定コストと環境依存を排除 |
| Mac / Windows のみ | 各 OS のループバック API を直接利用（Linux は Out） |
| Bun（npm 非前提） | Tauri 2 公式サポート、単一フロントツールチェーン（ADR-0002） |
| ModelStore は `app_data_dir`（ADR-0008） | モデル・editor / transcribe 設定・release ログの保存先を Tauri 配下に統一 |
| 3 バリアントユーザー選択（ADR-0013） | 精度・速度・メモリのトレードオフを利用者が選択。フェーズイベント形状は変更しない |

永続的な技術判断は `docs/architecture/adr/` に ADR として記録する。

---
_updated_at: 2026-09-10（transcribe-volume-normalize / ingest ゲインを反映）_
_Document standards and patterns, not every dependency_
