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
| React hooks | `src/presentation/hooks/*.test.ts` | 同上 |
| アーキテクチャ検証 | `scripts/*.test.ts` | レイヤルールの fixture テスト |
| Rust ユニット | 各 crate の `#[cfg(test)] mod tests` | モジュール内 |
| Rust 統合 | `src-tauri/crates/*/tests/*.rs` | crate 外統合テスト |

`src/**/*.test.*` は `tsconfig.json` の `exclude` に入れ、型チェック対象外とする（本番ビルドに含めない）。

### 実行

```bash
# フロント（個別ファイル推奨 — 一括 bun test は depcruise fixture と干渉しうる）
bun test src/presentation/hooks/useCaptureStatus.test.ts
bun test src/presentation/App.test.tsx

# 品質ゲート（CI 相当）
bun run check          # test:arch（depcruise fixture）を含む
bun run rust:check     # fmt / clippy / bylaw / machete（cargo test は spec Validation で追加）
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

- **対象**: `useCaptureStatus`、`App` のフェーズ表示・エラー表示
- **依存**: Tauri を起動しない。`listenFn` / `invokeFn` を注入
- **DOM**: `happy-dom` + `@testing-library/react`（`test-setup.ts` で一度だけ登録）

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
| E2E | 実機キャプチャ、権限ダイアログ、ウィンドウ閉鎖後のマイク解放 | `docs/specs/audio-capture/e2e-checklist.md` |
| 性能 | 30 分連続キャプチャ、CPU / メモリ / バッファドロップ | `docs/specs/audio-capture/performance-results.md` |
| 並走 | Zoom / Teams との同時実行 | `docs/specs/audio-capture/manual-concurrency-checklist.md` |

**数値捏造禁止**。実測できない環境ではチェックリストを未完了のまま残す。

## Mocking Principles

- **モックする**: Tauri `listen` / `invoke`、OS 音声 API、Whisper 推論（将来）
- **モックしない**: テスト対象の hook / コンポーネント / domain 変換ロジック
- **ファクトリ**: 契約型（`CapturePhaseChanged`, `CaptureUserError`）はインラインで最小構成
- **クリーンアップ**: `afterEach(cleanup)`、hook テストは unmount で unlisten を検証

## Assertion Conventions

- UI は `data-testid` で要素を特定（クラス名のみに依存しない）
- 利用者向けエラーは `message_ja` と `action_ja` を別々に assert
- **技術コードを DOM に出さない** — `MIC_PERMISSION_DENIED` 等が `textContent` に含まれないことを確認
- Rust observability は PCM 生データがログ構造に混入しないことを assert

## Feature Spec との関係

- 各 spec の **Validation** フェーズでテストを追加。tasks.md の Testing Strategy 番号とコメントで要件を紐づける
- 新契約追加時は domain マッピングテスト + presentation hook テスト + 必要なら統合テストの 3 点セットを検討
- 詳細手順は spec 配下のチェックリスト、本 steering は横断パターンのみ

## Related

- エラー形状・UI 表示: `docs/steering/error-handling.md`
- 契約正本: `docs/steering/contracts.md`
- 品質ゲート一覧: `docs/steering/tech.md`

---
_updated_at: 2026-09-05_
_Focus on patterns and decisions. Tool-specific config lives in package.json / Cargo.toml._
