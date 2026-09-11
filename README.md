# gijirec

Web 会議中にマイクとシステム音声を仮想オーディオデバイスなしで同時取り込みし、ローカル Whisper で約 30 秒間隔のバッチ文字起こし、その場で手動編集して Markdown（任意で JSONL）保存できる Tauri デスクトップアプリです。

## できること

- マイク＋スピーカー（ループバック）を 16 kHz モノラル PCM にリアルタイム合成
- 利用可能なマイク／スピーカーの一覧表示とセッション内デバイス選択（キャプチャ中の切り替え・再開）
- キャプチャ／文字起こしフェーズ状態の横並び表示
- マイク ingest ON/OFF（転写ミックスからの除外のみ）、転写 ingest 直前の dBFS メーター、手動ゲイン調整（0.25–4.0、既定 1.25）と現在値の数値表示
- whisper.cpp（`whisper-cpp-plus`）によるローカルバッチ文字起こし（約 30 秒間隔。ADR-0012）
- kotoba-whisper-v2.2 の量子化バリアント選択（Q5_0 / Q8_0 / FP16。ADR-0013）
- 手書きメモと AI 転写の二重エディタ（部分ロック、タイムスタンプ維持）
- 保存先ディレクトリへの Markdown / 任意 JSONL 出力
- モデル初回取得後はオフライン運用（クラウド STT・Python ランタイムなし）

## 対応プラットフォーム

| OS | サポート |
|----|----------|
| macOS 13+ | ✅ 対応 |
| Windows 10/11 | ✅ 対応 |
| Linux | ❌ **非対応**（起動・キャプチャは提供しません） |

## 必須ツール

本プロジェクトは **Bun** をフロントエンドのパッケージマネージャ兼スクリプト実行環境として使用します。**npm のインストールや `npm run` は不要**です。

| ツール | バージョン | 用途 |
|--------|------------|------|
| [Bun](https://bun.sh/) | >= 1.2 | 依存インストール・フロントスクリプト |
| [Rust](https://www.rust-lang.org/) | stable（`rust-toolchain.toml` 準拠） | Tauri バックエンド |
| [Tauri 2 前提条件](https://v2.tauri.app/start/prerequisites/) | — | OS ごとのビルド依存 |
| [cargo-tauri](https://v2.tauri.app/reference/cli/) | 2.x | デスクトップアプリの開発・ビルド |

Rust は `rustfmt` と `clippy` コンポーネントが必要です（`rust-toolchain.toml` で指定済み）。

```bash
# cargo-tauri CLI（初回のみ）
cargo install tauri-cli --version "^2"
```

## セットアップ

リポジトリをクローンしたあと、**Bun だけ**でフロントエンド依存を解決します。

```bash
bun install
```

npm や `package-lock.json` は使用しません。`bun.lock` がロックファイルです。

## 開発

### フロントエンドのみ（Vite 開発サーバー）

Tauri シェルなしで UI を確認する場合:

```bash
bun run dev
```

`http://localhost:1420` で Vite 開発サーバーが起動します。

### デスクトップアプリ（Tauri）

フルスタックで起動する場合:

```bash
cd src-tauri
cargo tauri dev
```

`tauri.conf.json` の `beforeDevCommand` により、起動前に自動で `bun run dev` が実行されます。

### リリースビルド診断ログ

配布用ビルドで障害調査が必要な場合のみ、起動時に `--log` を付けてください（通常起動ではログは残りません）。

```bash
# ビルド例（src-tauri から）
cargo tauri build
# 生成された実行ファイルを --log 付きで起動（OS ごとのパスは build 出力を参照）
```

詳細な保存場所・収集手順は [release-logging 運用手順](docs/manual/release-logging/operations.md) を参照。

## 品質チェック

```bash
# 完成判定（lint + test 一式。CI 相当の最終ゲート）
bun run verify

# 個別実行
# TypeScript: format / typecheck / lint / arch / knip / dup (TS threshold 0%)
bun run check

# フロント単体テスト（src/{presentation,application,domain,infrastructure}）
bun run test

# depcruise レイヤルールの fixture テスト
bun run test:arch

# Rust: fmt / check / clippy / bylaw / machete
bun run rust:check

# Rust: cargo test --workspace
bun run rust:test
```

| コマンド | 内容 |
|----------|------|
| `bun run verify` | **完成判定** — 下記の lint・テストをすべて実行 |
| `bun run check` | Biome・ESLint・TypeScript・dependency-cruiser・knip・jscpd（`dup:ts` + `dup:rust`） |
| `bun run test` | フロント4レイヤ配下のテスト（キャプチャ／文字起こし／エディタ／デバイス選択） |
| `bun run test:arch` | dependency-cruiser レイヤルールの fixture テスト |
| `bun run rust:check` | rustfmt・cargo check・clippy・cargo bylaw・cargo machete |
| `bun run rust:test` | Rust ワークスペースの `cargo test` |
| `bun run arch` | フロントエンドレイヤ依存検証（dependency-cruiser） |
| `bun run rust:arch` | Rust crate レイヤ依存検証（cargo bylaw） |

## 性能テスト（手動）

設計 Performance/Load 項目 1–3 および要件 4.2 の合格基準。30 分連続キャプチャの実測は **CI では実行しない**（ハードウェア・長時間計測が必要）。計測記録は [performance-results.md](docs/manual/audio-capture/performance-results.md) に残す。

### 合格基準（参照マシン: 4 コア / 16 GB）

| 項目 | 基準 | 確認方法 |
|------|------|----------|
| バッファドロップ | 30 分間 `capture_buffer_drops_total == 0` | キャプチャ終了後、ログに `pcm chunk bus dropped` の WARN が **0 件**（`capture_buffer_drops_total` フィールド）。下流 consumer 未登録のまま長時間 publish しないこと |
| CPU | キャプチャ中 **平均 < 5%**、**ピーク < 15%** | WPR / Instruments のプロセス CPU 使用率 |
| メモリ | キャプチャ開始前後の **常駐増分 < 50 MB** | タスクマネージャ / Activity Monitor またはプロファイラの Working Set / Resident Size |

### 共通準備

1. リリースまたは `--release` ビルドで Tauri アプリを起動する（デバッグビルドは CPU 比較の参考にならない）。
2. マイク・システム音声の権限を許可し、UI が `capturing` になることを確認する。
3. 30 分間、通常どおり Web 会議またはテスト用の音声再生を継続する。
4. 終了時に上記基準を記録し、[performance-results.md](docs/manual/audio-capture/performance-results.md) のテンプレート行を更新する。

ログでドロップを監視する例:

```bash
# Windows (PowerShell) / macOS — 別ターミナルで stderr をファイルにリダイレクトして起動した場合
RUST_LOG=gijirec_capture=info cargo tauri dev --manifest-path src-tauri/Cargo.toml
# 30 分後: "pcm chunk bus dropped" または capture_buffer_drops_total > 0 の WARN/ERROR が無いこと
```

### Windows — Windows Performance Recorder (WPR)

1. **管理者として** PowerShell を開き、利用可能なプロファイルを確認: `wpr -profiles`（`VirtualMemory` は存在しない）
2. 記録開始: `wpr -start CPU -start ResidentSet`（メモリは `ResidentSet` または後述のタスクマネージャーでも可）
3. gijirec を起動し、`capturing` 状態で **30 分** 計測する。
4. アプリを終了し、記録停止: `wpr -stop gijirec_perf.etl`
5. [Windows Performance Analyzer (WPA)](https://learn.microsoft.com/windows-hardware/test/wpt/windows-performance-analyzer)（Windows SDK / ADK の WPT に同梱）で `gijirec.exe` を開く。
6. **CPU Usage (Sampled)** で平均・ピークを読み取る（4 論理コア基準で 5% / 15% と比較）。
7. **Resident Set** / Working Set で 30 分前後の増分を確認する。

メモリだけタスクマネージャーで計測する場合: `gijirec.exe` の **作業セット** をキャプチャ開始直後と 30 分後にメモし、差分が 50 MB 未満か確認する。

### macOS — Instruments

1. Xcode の **Instruments** を開く。
2. **Time Profiler** テンプレートを選び、ターゲットに gijirec（`cargo tauri dev` または release `.app`）を指定する。
3. Record 開始 → `capturing` 確認 → **30 分** 継続 → Stop。
4. Time Profiler で gijirec プロセスの CPU 時間を確認（4 コア参照マシンで平均 5% / ピーク 15% 未満か）。
5. **Allocations** または Activity Monitor の **メモリ** で、キャプチャ前 idle と 30 分後の resident 増分が 50 MB 未満か確認する。

### 自動スモーク（CI で実行）

長時間実測の代替として、以下は CI / `bun run rust:check` で継続検証される:

- `PcmChunkBus` 統合テスト（バックプレッシャー・100 ms チャンク配信）
- 合成 rtrb による処理スレッド結線スモーク（`compose` 統合テスト）
- オーディオパイプライン単体テスト（mixer / ChunkEmitter / orchestrator）
- デバイス選択の統合・性能スモーク（`src-tauri/tests/device_selection_*.rs`）
- リリース診断ログの結合スモーク（`src-tauri/src/logging/`）

## 手動検証（会議アプリ並走・マイク解放）

要件 **3.2** / **4.1**。Zoom / Teams との並行実行中の相手音声途切れ、およびウィンドウ閉鎖後のマイクインジケータ消灯は **CI では実施しない**。手順・実行記録は [manual-concurrency-checklist.md](docs/manual/audio-capture/manual-concurrency-checklist.md) を参照（E2E 項目 1–2 は [e2e-checklist.md](docs/manual/audio-capture/e2e-checklist.md)）。

## プロジェクト構成

```
.
├── src/                  # TypeScript フロント（レイヤード）
│   ├── domain/
│   ├── application/
│   ├── infrastructure/
│   └── presentation/
├── src-tauri/            # Rust バックエンド（Tauri ホスト + logging）
│   ├── crates/           # gijirec-domain / application / infrastructure / presentation
│   └── src/              # composition root（compose, commands, logging）
├── docs/
│   ├── specs/            # 機能仕様（spec-driven development）
│   ├── steering/         # プロジェクト横断メモリ
│   ├── contracts/        # 永続 IPC 契約
│   └── architecture/     # 境界・ADR
└── package.json          # Bun スクリプト定義
```

機能仕様（v1 完了分）は `docs/steering/product.md` と `docs/architecture/boundaries.md`、手動検証・運用は `docs/manual/`、横断メモリは `docs/steering/`、IPC 契約は `docs/contracts/` です。新規 feature は `docs/specs/` に spec を作成する。

フロントエンドは `dependency-cruiser`、Rust は `cargo bylaw` でレイヤ依存を CI 検証します。`src/` から `src-tauri/` への直接 import は禁止です。

## ライセンス

（未定）
