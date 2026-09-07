# transcript-editor 性能・手動検証チェックリスト

検証日: 2026-09-06  
環境: Windows 10.0.26200, bun 1.3.8, Rust (workspace `src-tauri`。`src-tauri/.cargo/config.toml` で `target` 固定)

| # | 項目 | 目標 | 結果 | 状態 | 検証方法 |
|---|------|------|------|------|----------|
| 1 | 500 ブロック追記 p95 | < 16 ms | **p50 0.023 ms, p95 0.049 ms, max 0.165 ms** | **PASS** | `bun test src/application/transcript/blockAppend.perf.test.ts` |
| 2 | 10 分相当 mock メモリ増分 | < 50 MB | 未計測 | **MANUAL_SKIP** | 下記理由 |
| 3 | 保存 100 KB invoke + write | < 500 ms | **1 ms**（102 400 bytes） | **PASS** | `cargo test -p gijirec-application save_100kb_payload_completes_under_500ms`（`SaveService::save_at`、ディレクトリ作成＋書込込み） |
| 4 | ログに転写全文・手動議事録全文を含まない | 本文なし | テスト合格 | **PASS** | `cargo test -p gijirec-presentation save_log_fields_omit_markdown_bodies` |

## 詳細

### 1. 500 ブロック追記 p95（PASS）

- テスト: `src/application/transcript/blockAppend.perf.test.ts`
- 手法: `createAiTranscriptEditor()` + `applyUpstream(insert_node)` で 500 回末尾追記。各追記の `performance.now()` 差分から p95 を算出。
- 計測値（2026-09-06 実行）:
  - `block_count`: 500
  - `p50_ms`: 0.023
  - `p95_ms`: 0.049
  - `max_ms`: 0.165
  - `threshold_p95_ms`: 16
- 注: happy-dom + Slate ユニット microbench。実ブラウザ / Tauri WebView では異なる可能性あり。

### 2. 10 分相当 mock メモリ増分（MANUAL_SKIP）

**理由:** bun test / happy-dom では長時間セッション相当のメモリプロファイルを信頼できる形で取得できない。`performance.memory` は Node/Bun 非標準かつ GC タイミングに依存し、10 分連続 mock 追記のヒープ増分を安定再現する CI 向け手段がない。実機では DevTools Memory または `tauri dev` 長時間セッションでの手動計測が必要。

### 3. 保存 100 KB（PASS）

- テスト: `save_service.rs` → `save_100kb_payload_completes_under_500ms`
- 手法: 手動議事録 51 200 bytes + AI 転写 51 200 bytes（合計 102 400 bytes）を `SaveService::save_at` で保存。`Instant::elapsed()` で計測。
- 計測値（2026-09-06 実行）: `elapsed_ms=1`, `bytes=102400`, `threshold_ms=500`
- 注: Tauri invoke オーバーヘッドは含まない。`SaveService` 書込 I/O の下限性能。フル E2E は `tauri dev` 手動計測を推奨。

### 4. ログ本文除外（PASS）

- テスト: `observability.rs` → `save_log_fields_omit_markdown_bodies`
- `save_log_fields` が `handwriting_markdown_len` / `ai_transcription_markdown_len` / `jsonl_record_count` のみを出力し、`Debug` シリアライズに本文文字列を含まないことを検証。

## 実行コマンド（再現用）

```powershell
# 1. ブロック追記 microbench
bun test src/application/transcript/blockAppend.perf.test.ts

# 3–4. Rust 性能・ログフィールド（リポジトリルートから実行する場合）
# CARGO_TARGET_DIR はマニフェスト基準の相対パス。`src-tauri/target` と書くと src-tauri/src-tauri/target が誤生成される。
cargo test --manifest-path src-tauri/Cargo.toml -p gijirec-application save_100kb_payload_completes_under_500ms -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p gijirec-presentation save_log_fields_omit_markdown_bodies -- --nocapture
```

## サマリー

| 状態 | 件数 |
|------|------|
| PASS | 3 |
| MANUAL_SKIP | 1 |
| FAIL | 0 |

Rust typecheck の注意事項は [docs/steering/tech.md](../../steering/tech.md) の Toolchain gotchas を参照。
