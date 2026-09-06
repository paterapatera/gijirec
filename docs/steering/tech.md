# Technology Stack

## Architecture

Tauri デスクトップアプリ。Rust バックエンドが OS ネイティブの音声キャプチャ・Whisper 推論を担い、TypeScript の Web UI がエディタ体験を提供する。

Rust 側は **レイヤードアーキテクチャ**（domain → application / infrastructure → presentation）で、依存方向を `cargo bylaw` で強制する。TypeScript 側も同じレイヤ分離を `dependency-cruiser` で強制する（`src/` は `src-tauri/` に依存しない）。

## Core Technologies

- **Frontend**: TypeScript（strict）、React 19、Vite 8、Tauri 2 IPC（`@tauri-apps/api`）
- **Backend**: Rust（edition 2024、stable toolchain）
- **Desktop Shell**: Tauri 2（`cargo tauri` CLI）
- **STT**: `whisper-cpp-plus` 0.1（ADR-0003）+ デフォルトモデル `kotoba-whisper-v2.2-ggml-q5_0.bin`（ADR-0004、kenrouse 配布）
- **Audio**: cpal（マイク / Windows ループバック）、screencapturekit（macOS システム音声）、rubato（リサンプル）、rtrb（スレッド間バッファ）— ADR-0001 準拠
- **Runtime**: Bun >= 1.2（フロントエンドパッケージマネージャ・スクリプト実行。npm 非前提）

## Key Libraries

影響する開発パターンに限定:

| 領域 | 技術 | 役割 |
|------|------|------|
| 音声キャプチャ | cpal、screencapturekit（macOS）、WASAPI loopback（Windows）、rubato、rtrb | マイク＋システム音声の二重取り込み・16 kHz ミックス |
| キャプチャ配信 | `PcmChunkBus`（presentation） | 100 ms チャンクの下流 consumer 向けバックプレッシャー付き配信 |
| 文字起こし | `whisper-cpp-plus`（`WhisperCppAdapter`） | VAD 駆動ストリーミング推論。専用ワーカースレッド + rtrb |
| 転写ブロック配信 | `TranscriptBlockBus`（presentation） | `whisper-transcribe://block-appended` で追記のみ配信 |
| マウント同期 | `TranscribeStatusCache`（presentation） | モデル取得中でもブロックしないフェーズ／進捗スナップショット |
| エディタ | Slate.js（編集面）+ shadcn/ui + Sonner | 部分ロック付き二重エディタ（ADR-0005 / ADR-0006） |
| 保存ダイアログ | `@tauri-apps/plugin-dialog` | 保存先ディレクトリ選択（`pick_save_directory`） |
| デバイス選択 | `DeviceSelectionService` + cpal 列挙 | セッション内マイク／スピーカー選択・キャプチャ再開（ADR-0009） |
| 診断ログ | `src-tauri/src/logging/`（`--log`） | リリースビルドのファイル永続化（ADR-0007）。`app_data_dir/logs/` |
| アーキテクチャ検証 | cargo bylaw、dependency-cruiser | レイヤ依存の自動チェック |

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

## Key Technical Decisions

| 判断 | 理由 |
|------|------|
| Tauri + Rust | OS ネイティブ音声 API への直接アクセス、単一バイナリ配布 |
| whisper.cpp（Python なし） | オフライン完結、ランタイム依存を最小化 |
| レイヤード crates / src 構造 | ドメイン境界をコンパイル時・CI 時に強制 |
| 仮想デバイス不使用 | ユーザー設定コストと環境依存を排除 |
| Mac / Windows のみ | 各 OS のループバック API を直接利用（Linux は Out） |
| Bun（npm 非前提） | Tauri 2 公式サポート、単一フロントツールチェーン（ADR-0002） |
| ModelStore は `app_data_dir`（ADR-0008） | モデル・editor 設定・release ログの保存先を Tauri 配下に統一 |

永続的な技術判断は `docs/architecture/adr/` に ADR として記録する。

---
_updated_at: 2026-09-07（Sync: fix-release-transcribe 完了・ADR-0008 を反映）_
_Document standards and patterns, not every dependency_
