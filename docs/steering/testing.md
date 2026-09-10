# Testing Standards

gijirec のテスト方針。何をどこで検証し、何を CI に載せないかを統一する。

## Philosophy

- **振る舞いを検証** — 実装詳細や private API ではなく、契約・利用者体験をテストする
- **高速・決定的** — Tauri シェルや実デバイスなしで回せるテストを優先
- **層ごとに適切な粒度** — domain は純粋ユニット、presentation は IPC 境界の統合、E2E は手動
- **カバレッジ数値は追わない** — 契約上のクリティカルパスと品質ゲートで担保

## Organization

### 配置（co-located がデフォルト）

| 対象 | 場所 | 命名 |
|------|------|------|
| React コンポーネント | `src/presentation/**/*.test.tsx` | 実装ファイルと同階層 |
| 統合テスト（複数コンポーネント跨ぎ） | `src/presentation/integration/*.integration.test.tsx` | 配線・保存フローなど |
| React hooks | `src/presentation/hooks/*.test.ts` | 同上 |
| TS domain / application | `src/domain/**/*.test.ts`、`src/application/**/*.test.ts` | 同上（転写エクスポート・reducer・プラグイン） |
| TS infrastructure | `src/infrastructure/**/*.test.ts` | invoke ラッパ（editor / audio device） |
| アーキテクチャ検証 | `scripts/*.test.ts` | レイヤルールの fixture テスト |
| Rust ユニット | 各 crate の `#[cfg(test)] mod tests` | モジュール内 |
| Rust 統合 | `src-tauri/crates/*/tests/*.rs`、`src-tauri/tests/*.rs` | crate 外統合テスト（device selection 性能・observability・バッチ transcribe パイプライン・モデルバリアント切替・`capture_audio_controls_integration.rs` 含む） |

`src/**/*.test.*` は `tsconfig.json` の `exclude` に入れ、型チェック対象外とする（本番ビルドに含めない）。

### 実行

```bash
# 完成判定（lint + test 一式）
bun run verify

# 個別
# フロント（ルート一括 bun test は src-tauri の whisper fixture と scripts の depcruise fixture に当たる）
bun test src/presentation src/application src/domain src/infrastructure

# 品質ゲート（CI 相当の静的解析）
bun run check          # format / typecheck / lint / arch / knip
bun run test           # フロント4レイヤ（capture / transcribe / editor）
bun run test:arch      # depcruise レイヤルール fixture
bun run rust:check     # fmt / clippy / bylaw / machete
bun run rust:test      # cargo test --workspace
# compose 結線のみ: cargo test -p gijirec -- compose::
```

## Test Types

### Unit（Rust domain / application）

- **対象**: `PcmChunk` 制約、`CaptureError::to_user_facing()` マッピング、mixer / emitter の純粋ロジック
- **依存**: モックなしまたは crate 内スタブ
- **例**: 全 `UserFacingErrorCode` が非空 `message_ja` / `action_ja` を持つこと

### Integration（Rust presentation / 合成パイプライン）

- **対象**: `PcmChunkBus` バックプレッシャー、observability フック、合成 rtrb 結線スモーク
- **依存**: OS API はモック。`set_observability(RecordingObservability)` で tracing 代替
- **並列**: グローバル observability を使うテストは `Mutex` で直列化

### Component / Hook（TypeScript presentation）

- **対象**: キャプチャ／文字起こしフックと `App` のフェーズ表示。`AppStatusPanels` は `.phase-panels-row` 内の `capture-phase` / `transcribe-phase` と進捗・エラーの分離（`AppStatusPanels.test.tsx`）。エディタは二重エディタ・ツールバー・保存トースト・プラグイン。`ModelVariantSelector` は 3 選択肢・`loading_model` 中 disabled。`CaptureAudioControlsRow` は capturing / non-capturing の disabled・dBFS ラベル・`ingest-gain-value` 数値表示・invoke 呼び出し（`CaptureAudioControlsRow.test.tsx` / `CaptureAudioControlsRow.e2e.test.tsx`）
- **依存**: Tauri を起動しない。`listenFn` / `invokeFn` を注入
- **DOM**: `happy-dom` + `@testing-library/react`（`src/test-setup.ts` で一度だけ登録。全レイヤのテストから import 可）

```typescript
// パターン: mock listen + emit で Tauri イベントを再現
const { listenFn, emit } = createMockListen();
render(<App listenFn={listenFn} />);
emit(PHASE_CHANGED_EVENT, { phase: "capturing", timestamp_ms: 1 });
```

### Architecture（ツールチェーン）

- **dependency-cruiser**: `scripts/verify-depcruise-layers.test.ts` が一時 fixture でレイヤ違反を再現
- **cargo bylaw**: `bun run rust:arch` — レイヤ crate の依存方向
- **意図的違反テスト** — ルールが生きていることの証拠として維持

### E2E / Performance / Manual（CI 対象外）

| 種別 | 確認内容 | 記録先 |
|------|----------|--------|
| E2E | 実機キャプチャ、権限ダイアログ、ウィンドウ閉鎖後のマイク解放 | `docs/manual/audio-capture/e2e-checklist.md` |
| E2E（デバイス選択） | 既定表示・選択変更・empty state・`action_ja` | `App.device-selection.e2e.test.tsx`（モック IPC） |
| 性能 | 30 分連続キャプチャ、CPU / メモリ / バッファドロップ | `docs/manual/audio-capture/performance-results.md` |
| 性能（デバイス選択） | 選択変更 → capturing 復帰 < 2 s | `docs/manual/audio-device-selection/performance-results.md`（自動: `src-tauri/tests/device_selection_performance.rs`） |
| 性能（転写） | 10 分転写 latency・バッチ間隔（30 s）・停止後 flush 完全性 | `docs/manual/whisper-transcribe/performance-results.md` |
| 転写音量（ingest ゲイン） | 快適 OS 音量で `transcribe_window_rms_dbfs` が −18〜−17 dBFS 付近・転写精度の主観改善 | `docs/manual/whisper-transcribe/performance-results.md`（ingest ゲイン節） |
| 性能（エディタ） | 500 ブロック追記 p95、保存 100 KB、ログ本文除外 | `docs/manual/transcript-editor/validation-checklist.md` |
| 並走 | Zoom / Teams との同時実行 | `docs/manual/audio-capture/manual-concurrency-checklist.md` |
| リリース smoke | release EXE でキャプチャ→文字起こし→ブロック表示 | `docs/manual/fix-release-transcribe/smoke-checklist.md` |

### リリース vs dev パリティ（手動・CI 外）

release で文字起こししないとき、**コード変更前に**次を確認する:

- [ ] ModelStore が `app_data_dir`（ADR-0008）を参照しているか
- [ ] `compose` 起動順序・`block-appended` ACL・`TranscribeStallWatchdog` が有効か
- [ ] dev と同じ whisper 窓長・スレッド・`single_segment` / `entropy_thold` か
- [ ] `--log` 有無で挙動が変わるか（診断ログは `docs/manual/release-logging/operations.md`）

`bun run verify` は自動ゲート。**上記は release 実機確認**であり、verify 合格だけでは代替しない。

**数値捏造禁止**。実測できない環境ではチェックリストを未完了のまま残す。

## Mocking Principles

- **モックする**: Tauri `listen` / `invoke`、OS 音声 API、Whisper 推論、ディレクトリ選択ダイアログ
- **モックしない**: テスト対象の hook / コンポーネント / domain 変換ロジック
- **ファクトリ**: 契約型（`CapturePhaseChanged`, `CaptureUserError`）はインラインで最小構成
- **共有 invoke モック**: App 配線テスト向けに `src/presentation/testInvokeHelpers.ts`（`get_transcribe_settings` / `get_transcribe_status` 等）。エディタ統合は `components/transcriptEditorTestHelpers.ts`
- **クリーンアップ**: `afterEach(cleanup)`、hook テストは unmount で unlisten を検証

## Assertion Conventions

- UI は `data-testid` で要素を特定（クラス名のみに依存しない）
- 利用者向けエラーは `message_ja` と `action_ja` を別々に assert
- **技術コードを DOM に出さない** — `MIC_PERMISSION_DENIED` 等が `textContent` に含まれないことを確認
- Rust observability は PCM 生データがログ構造に混入しないことを assert

## Feature Spec との関係

- 各 spec の **Validation** フェーズでテストを追加。tasks.md の Testing Strategy 番号とコメントで要件を紐づける
- 新契約追加時は domain マッピングテスト + presentation hook テスト + 必要なら統合テストの 3 点セットを検討
- 詳細手順は `docs/manual/` のチェックリスト、本 steering は横断パターンのみ

## Related

- エラー形状・UI 表示: `docs/steering/error-handling.md`
- 契約正本: `docs/steering/contracts.md`
- 品質ゲート一覧: `docs/steering/tech.md`

---
_updated_at: 2026-09-10（AppStatusPanels / capture-gain-value-display テスト参照を追記）_
_Focus on patterns and decisions. Tool-specific config lives in package.json / Cargo.toml._
