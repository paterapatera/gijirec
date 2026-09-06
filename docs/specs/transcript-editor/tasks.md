# 実装計画: transcript-editor

## 概要

上流 `whisper-transcribe://block-appended` を購読する二重 Slate.js エディタ（手動議事録＋ AI 転写）と部分ロックプラグインを TypeScript レイヤに新規追加し、保存・設定 I/O を Rust `SaveService` / `SettingsService` で実装する。Foundation → Core（境界別並列）→ Integration → Validation の順で実装する。

---

- [ ] 1. Foundation: 依存追加・骨格・テーマ基盤
- [x] 1.1 フロントエンド依存と shadcn / Tailwind スキャフォールド
  - `slate`、`slate-react`、`tailwindcss`、`class-variance-authority`、`clsx`、`tailwind-merge`、`@tauri-apps/plugin-dialog` を追加し、`components.json`（`@/` → `src/presentation`）と `tsconfig.json` のパスエイリアスを設定する
  - shadcn/ui コンポーネント（`button`、`switch`、`label`、`separator`、`alert`、`sonner`）を `src/presentation/components/ui/` に追加する
  - `tailwind.config.ts`、`postcss.config.js`、`src/presentation/lib/utils.ts`（`cn()`）を整備する
  - 完了時: `bun install` と `bun run typecheck` が通り、shadcn コンポーネントが import 可能である
  - _Requirements: 5.1, 8.5_
  - _Wave: 1_

- [x] 1.2 (P) エディタテーマ CSS 変数と globals 統合
  - `src/presentation/styles/globals.css` に shadcn セマンティックトークン（`--casal` プライマリ等）を定義する
  - `src/presentation/styles/editor-theme.css` に v1 パレット 8 色（`--jagged-ice`、`--hawkes-blue`、`--classic-rose`、`--plum` 等）を定義する
  - エントリ（`src/main.tsx` 等）で `globals.css` を import する
  - 完了時: AI パネル背景 `--jagged-ice`、手動パネル `--hawkes-blue` が CSS 変数として参照可能である
  - _Requirements: 2.1, 3.1_
  - _Boundary: EditorTheme_
  - _Wave: 2_

- [x] 1.3 (P) Rust editor モジュール骨格と Tauri 権限
  - `gijirec-domain/src/editor/`、`gijirec-application/src/editor/`、`gijirec-presentation/src/editor/` に `mod.rs` 空骨格を作成し各 `lib.rs` から公開する
  - `src-tauri/src/commands.rs` に editor コマンド登録のプレースホルダを追加する
  - Tauri capabilities に `fs`（保存先書込）、`dialog`（保存先選択）権限を明示追加する
  - `chrono` + `chrono-tz`（JST パス生成用）を application 層依存に追加する
  - 完了時: `cargo check` が 4 crate すべてで通り、editor モジュールがビルド対象に含まれる
  - _Requirements: 5.2, 6.1, 10.1_
  - _Wave: 3_

- [ ] 2. ドメイン層（TypeScript）: 転写型とエクスポート
- [x] 2.1 (P) 転写ドメイン型と契約ミラー
  - `TranscriptBlockView`（`blockId`、`sequence`、`text`、`startTimestampMs`、`language`、`displayText`）、`LockRange`、`EditorSettings` 契約ミラー型を `src/domain/transcript/types.ts` に定義する
  - `TranscriptBlockElement`、`CustomText`（`locked?` mark）を `src/domain/transcript/slateTypes.ts` に定義する
  - 完了時: 上流 `whisper-transcribe-blocks.md` フィールドと整合する型がコンパイルされ、domain レイヤが外側レイヤに依存しない
  - _Requirements: 1.1, 1.6, 2.1, 5.1, 8.4_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md, docs/contracts/transcript-editor-settings.md_
  - _Boundary: TranscriptDomainTypes_
  - _Design: D-Types_
  - _Depends: 1.1_
  - _Wave: 4_

- [x] 2.2 (P) Markdown / JSONL エクスポート純関数
  - `toAiMarkdown`（タイムスタンプなしプレーンテキスト）と `toJsonlRecords`（`block_id`、`sequence`、`text`、`start_timestamp_ms`、`language`）を `src/domain/transcript/export.ts` に実装する
  - ロック済み手動修正後テキストを `text` に反映する
  - 完了時: 空ドキュメントでも空文字列を返し、JSONL レコードが契約形状どおり生成されるユニットテストが通る
  - _Requirements: 1.6, 7.2, 7.5, 8.1, 8.2_
  - _Contracts: docs/contracts/transcript-editor-save.md_
  - _Boundary: TranscriptExport_
  - _Design: D-Export_
  - _Depends: 2.1_
  - _Wave: 5_

- [ ] 3. ドメイン層（Rust）: 保存・設定・エラー型
- [x] 3.1 (P) EditorSettings と Save リクエスト型
  - `EditorSettings`（`save_directory`、`export_jsonl_enabled`）を `gijirec-domain/src/editor/settings.rs` に serde 対応で定義する
  - `SaveTranscriptSessionRequest` / `SaveTranscriptSessionResult` / `AiTranscriptionJsonlRecord` を `gijirec-domain/src/editor/save.rs` に契約形状どおり定義する
  - 完了時: 契約フィールド制約を満たす型がコンパイルされ、JSON シリアライズが通る
  - _Requirements: 5.2, 6.1, 7.1, 7.2, 8.1_
  - _Contracts: docs/contracts/transcript-editor-save.md, docs/contracts/transcript-editor-settings.md_
  - _Boundary: EditorDomainTypes_
  - _Depends: 1.3_
  - _Wave: 6_

- [x] 3.2 (P) EditorError と利用者向けエラー変換
  - `EditorError` 内部列挙と `to_user_facing()` を `gijirec-domain/src/editor/error.rs` に実装する
  - 全 `EditorUserErrorCode`（`SAVE_DIRECTORY_NOT_SET`、`SAVE_DIRECTORY_UNAVAILABLE`、`SAVE_DIRECTORY_CREATE_FAILED`、`SAVE_FILE_WRITE_FAILED`、`SAVE_PARTIAL_FAILURE`、`SETTINGS_PERSIST_FAILED`、`INTERNAL`）を網羅し、`action_ja` が非空になる
  - 完了時: 各内部エラーが契約どおりの `EditorUserError` に変換されるユニットテストが通る
  - _Requirements: 5.4, 5.5, 6.3, 7.7, 9.1, 9.4_
  - _Contracts: docs/contracts/transcript-editor-status.md_
  - _Boundary: EditorErrors_
  - _Design: D-EditorError_
  - _Depends: 1.3_
  - _Wave: 7_

- [ ] 4. アプリケーション層（TypeScript）: ブロック状態と Slate プラグイン
- [x] 4.1 BlockReducer（追記のみ in-memory 状態）
  - `appendBlock(block)` で sequence 単調増加を期待し、既存ブロックの `text` / `blockId` を変更しない reducer を実装する
  - sequence 欠番を検出して `sequenceGapCount` を increment する
  - 上流停止後も状態を保持する（再起動時は空 — 要件 2.4）
  - 完了時: 追記のみ・既存 block 不変・gap カウントのユニットテストが通る
  - _Requirements: 1.1, 1.2, 1.4, 4.4, 9.2_
  - _Boundary: BlockReducer_
  - _Design: D-BlockReducer_
  - _Depends: 2.1_
  - _Wave: 8_

- [x] 4.2 (P) withAppendOnlyBlocks プラグイン
  - 新規 `transcript-block` 要素を `Editor.withoutNormalizing` でドキュメント末尾に insert のみ許可する
  - 上流由来操作での既存ノード `set_node` / `remove_node` を禁止する
  - 完了時: 末尾追記は成功し、既存ブロックへの上流由来削除が拒否されるユニットテストが通る
  - _Requirements: 1.2, 3.3, 4.1, 4.2_
  - _Boundary: WithAppendOnlyBlocks_
  - _Design: D-WithAppendOnlyBlocks_
  - _Depends: 2.1_
  - _Wave: 9_

- [x] 4.3 (P) LockManager と withLockedRanges プラグイン
  - 選択確定・直接入力時に `LockManager.lockSelection()` で `locked: true` mark を付与する
  - `editor.apply` で locked 範囲への上流由来 remove/replace を reject し、利用者の明示編集は許可する
  - `renderLeaf` で `--classic-rose` 背景 + `--plum` 下線を表示する
  - 完了時: ロック後の上流追記で locked テキストが不変であり、利用者の再編集が可能なテストが通る
  - _Requirements: 3.1, 3.2, 3.4, 3.5_
  - _Boundary: LockManager_
  - _Design: D-LockManager, D-WithLockedRanges_
  - _Depends: 2.1_
  - _Wave: 10_

- [x] 4.4 withStableSelection プラグイン
  - 追記時に編集中のカーソル位置・選択範囲を維持する selection ref 管理を実装する
  - scroll container に `overflow-anchor: auto` を適用する設計ノートを Ai エディタ側で消費可能にする
  - 完了時: 末尾追記後も非末尾編集位置の selection が保持されるユニットテストが通る
  - _Requirements: 4.1, 4.3_
  - _Boundary: WithStableSelection_
  - _Design: D-WithStableSelection_
  - _Depends: 4.2_
  - _Wave: 11_

- [ ] 5. アプリケーション層（Rust）: 設定永続化と保存 I/O
- [x] 5.1 (P) SettingsService
  - `app_data_dir` 配下 `editor-settings.json` の読み書きを `gijirec-application/src/editor/settings_service.rs` に実装する
  - `get` でデフォルト値（`save_directory: null`、`export_jsonl_enabled: false`）を返し、部分更新をサポートする
  - 永続化失敗を `SETTINGS_PERSIST_FAILED` に変換する
  - 完了時: 読み書きラウンドトリップとデフォルト値のユニットテストが通る
  - _Requirements: 5.2, 5.3, 8.4_
  - _Contracts: docs/contracts/transcript-editor-settings.md_
  - _Boundary: SettingsService_
  - _Design: D-SettingsService_
  - _Depends: 3.1, 3.2_
  - _Wave: 12_

- [x] 5.2 (P) SaveService
  - JST（`Asia/Tokyo`）で `{YYYY}/{MM}/{DD}/{hh}_{mm}_{ss}/` サブディレクトリを生成し、同一秒衝突時は `_001` インクリメントする
  - `handwriting.md`、`ai-transcription.md`、条件付き `ai-transcription.jsonl` を書込する
  - 基点ディレクトリの canonicalize + prefix 検証でパス traversal を拒否する
  - 部分失敗時は成功ファイルを残し `SAVE_PARTIAL_FAILURE` + `files_failed` を返す
  - 完了時: JST パス形式・同一秒 `_001`・traversal 拒否・部分失敗のユニットテストが通る
  - _Requirements: 5.4, 6.1, 6.2, 6.3, 7.1, 7.2, 7.7, 7.8, 10.2_
  - _Contracts: docs/contracts/transcript-editor-save.md_
  - _Boundary: SaveService_
  - _Design: D-SaveService_
  - _Depends: 3.1, 3.2_
  - _Wave: 13_

- [ ] 6. インフラストラクチャ・フック: IPC ラッパと状態接続
- [x] 6.1 editorCommands IPC ラッパ
  - `save_transcript_session`、`get_editor_settings`、`set_editor_settings`、`pick_save_directory` の invoke ラッパを `src/infrastructure/tauri/editorCommands.ts` に実装する
  - 契約型ミラーで request/response を型付けする
  - 完了時: injectable `invokeFn` でモック invoke が可能であり、型が契約と整合する
  - _Requirements: 2.4, 5.1, 7.3, 10.1_
  - _Contracts: docs/contracts/transcript-editor-save.md, docs/contracts/transcript-editor-settings.md_
  - _Boundary: EditorCommands_
  - _Depends: 2.1_
  - _Wave: 14_

- [x] 6.2 (P) useTranscriptBlocks フック
  - `whisper-transcribe://block-appended` を購読し、`BlockReducer.appendBlock` に接続する
  - 契約型ミラー `transcript-blocks.ts` を `presentation/hooks/` に配置する
  - v1 はマウント時 replay なし（セッション内メモリのみ）
  - テスト時は injectable `listenFn` で Tauri なし単体テスト可能にする
  - 完了時: モックイベント注入でブロックが reducer に追記されるフロント単体テストが通る
  - _Requirements: 1.1, 1.2, 1.3, 1.5_
  - _Contracts: docs/contracts/whisper-transcribe-blocks.md_
  - _Boundary: UseTranscriptBlocks_
  - _Design: D-UseTranscriptBlocks_
  - _Depends: 4.1, 6.1_
  - _Wave: 15_

- [x] 6.3 (P) useEditorSettings フック
  - 起動時 `get_editor_settings` で復元し、`set_editor_settings` / `pick_save_directory` で更新する
  - 契約型ミラー `editor-settings.ts` を配置する
  - 完了時: モック invoke で設定読込・部分更新・再起動復元がテストで検証される
  - _Requirements: 5.1, 5.2, 5.3, 8.4, 8.5_
  - _Contracts: docs/contracts/transcript-editor-settings.md_
  - _Boundary: UseEditorSettings_
  - _Design: D-UseEditorSettings_
  - _Depends: 6.1_
  - _Wave: 16_

- [x] 6.4 SaveOrchestrator
  - 保存開始時点で手動議事録・AI 転写をスナップショットし、`export.ts` で serialize して invoke する
  - `isSaving` フラグで二重保存を拒否する（2 回目は UI で無視）
  - スナップショット後到着ブロックは export に含めない
  - 保存処理中も上流 whisper-transcribe を停止しない
  - 完了時: スナップショット境界と `isSaving` ガードのユニットテストが通る
  - _Requirements: 2.4, 7.3, 7.4, 8.3, 9.2, 10.2_
  - _Contracts: docs/contracts/transcript-editor-save.md_
  - _Boundary: SaveOrchestrator_
  - _Design: D-SaveOrchestrator_
  - _Depends: 2.2, 6.1_
  - _Wave: 17_

- [ ] 7. プレゼンテーション層: Slate エディタと chrome
- [x] 7.1 (P) HandwritingEditor
  - 手動議事録用 Slate エディタを `src/presentation/components/HandwritingEditor.tsx` に実装する
  - 背景 `--hawkes-blue`、AI エディタとは独立した Slate インスタンス、`data-testid="handwriting-editor"`
  - `ref` で `Editor.string(editor, [])` によるプレーンテキスト export が可能
  - 完了時: 入力が即時反映され、自動ディスク書込が発生しない
  - _Requirements: 2.1, 2.2, 2.4, 10.2_
  - _Boundary: HandwritingEditor_
  - _Design: D-HandwritingEditor_
  - _Depends: 2.1, 1.2_
  - _Wave: 18_

- [x] 7.2 AiTranscriptEditor
  - `createAiTranscriptEditor()` factory で `withAppendOnlyBlocks` + `withLockedRanges` + `withStableSelection` を compose する
  - 背景 `--jagged-ice`、`data-testid="ai-transcript-editor"`、scroll container `overflow-y: auto; overflow-anchor: auto`
  - `blockId` / `startTimestampMs` を `transcript-block` 要素属性に保持する
  - 完了時: モックブロック追記で末尾 insert され、remount なしで表示が更新される
  - _Requirements: 1.1, 1.6, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3_
  - _Boundary: AiTranscriptEditor_
  - _Design: D-AiTranscriptEditor_
  - _Depends: 4.2, 4.3, 4.4_
  - _Wave: 19_

- [x] 7.3 (P) EditorToolbar
  - shadcn `Button`（保存 primary、保存先選択 outline）、`Switch` + `Label`（`export_jsonl_enabled`）を実装する
  - `pick_save_directory` は `useEditorSettings` 経由で呼び出す
  - 保存中は `Button` を `disabled` + loading 表示する
  - 完了時: 保存先未設定時に保存ボタン押下で Rust 側エラーが返り、JSONL 切替が設定に反映される
  - _Requirements: 5.1, 5.5, 8.5_
  - _Boundary: EditorToolbar_
  - _Design: D-EditorToolbar_
  - _Depends: 6.3_
  - _Wave: 20_

- [x] 7.4 (P) SaveResultToast
  - shadcn `Sonner` で保存成功時に出力パス、失敗時に `EditorUserError.message_ja` / `action_ja` を表示する
  - 完了時: 成功 toast に `output_directory`、失敗 toast に行動可能メッセージが含まれる
  - _Requirements: 7.6, 9.1_
  - _Contracts: docs/contracts/transcript-editor-status.md_
  - _Boundary: SaveResultToast_
  - _Design: D-SaveResultToast_
  - _Depends: 1.1_
  - _Wave: 21_

- [x] 7.5 TranscriptEditorView レイアウト
  - 二重エディタ（`Separator` 区切り）と `EditorToolbar` を統合する root コンポーネントを実装する
  - 手動議事録と AI 転写の同時編集が可能なレイアウトを提供する
  - 上流 `whisper-transcribe://error` 時も表示内容を保持し保存操作を可能にする
  - 完了時: 二重パネルが同時に操作可能であり、上流エラー後もエディタ内容が消えない
  - _Requirements: 1.3, 1.4, 2.3, 9.3_
  - _Boundary: TranscriptEditorView_
  - _Design: D-TranscriptEditorView_
  - _Depends: 7.1, 7.2, 7.3_
  - _Wave: 22_

- [x] 7.6 useSaveTranscript フック
  - `SaveOrchestrator` を呼び出し、結果を `SaveResultToast` に渡す保存操作フックを実装する
  - 完了時: モック invoke で保存成功・失敗・部分失敗の各シナリオがテストで検証される
  - _Requirements: 7.6, 9.1, 9.2_
  - _Boundary: UseSaveTranscript_
  - _Design: D-UseSaveTranscript_
  - _Depends: 6.4, 7.4_
  - _Wave: 23_

- [ ] 8. プレゼンテーション層（Rust）: Tauri コマンド
- [x] 8.1 editor Tauri コマンドハンドラ
  - `save_transcript_session`、`get_editor_settings`、`set_editor_settings`、`pick_save_directory` を `gijirec-presentation/src/editor/commands.rs` に実装する
  - `src-tauri/src/lib.rs` と `commands.rs` でコマンド登録・`SaveService` / `SettingsService` 注入を行う
  - ログターゲット `gijirec_editor`、転写テキスト全文・手動議事録全文のマスキングを設定する
  - 完了時: `cargo tauri dev` で editor invoke が応答し、設定 JSON が `app_data_dir` に永続化される
  - _Requirements: 5.2, 5.3, 7.1, 7.2, 8.1, 9.4, 10.1_
  - _Contracts: docs/contracts/transcript-editor-save.md, docs/contracts/transcript-editor-settings.md, docs/contracts/transcript-editor-status.md_
  - _Depends: 5.1, 5.2_
  - _Boundary: EditorCommandsRust_
  - _Wave: 24_

- [ ] 9. 統合結線: App 統合と境界ドキュメント
- [x] 9.1 App.tsx 統合
  - `TranscriptEditorView` を既存キャプチャ／文字起こしステータスパネルと統合する
  - ルートに `<Toaster />` を 1 つ配置する
  - `useTranscriptBlocks`、`useEditorSettings`、`useSaveTranscript` を接続する
  - 音声キャプチャ・Whisper 推論・クラウド同期・認証 UI は追加しない
  - 完了時: `bun run tauri dev` で二重エディタが表示され、上流ブロック追記・保存・設定が end-to-end で動作する
  - _Requirements: 1.3, 1.5, 2.3, 7.4, 9.3, 10.1, 10.3, 10.4_
  - _Depends: 6.2, 6.3, 7.5, 7.6, 8.1_
  - _Boundary: CompositionRoot_
  - _Wave: 25_

- [x] 9.2 boundaries.md 同期
  - `docs/architecture/boundaries.md` に transcript-editor 境界セクション（TS 編集 + Rust 保存 I/O、上流 block-appended 購読）を追加する
  - 完了時: boundaries.md に本 feature の所有境界と禁止事項が記載されている
  - _Requirements: 1.5, 3.5, 10.1_
  - _Depends: 9.1_
  - _Wave: 26_

- [ ] 10. 検証: テスト・性能・手動チェックリスト
- [x] 10.1 ユニットテスト（設計 Testing Strategy Unit 1–7）
  - `BlockReducer`（追記のみ、gap カウント）、`withLockedRanges`（ロック保護・利用者編集許可）、`toAiMarkdown` / `toJsonlRecords`（TS）、`EditorError::to_user_facing`（全 code）、`SaveService`（JST パス・衝突・traversal）、`SettingsService`（ラウンドトリップ）、`SaveOrchestrator`（`isSaving` ガード）を実装する
  - 完了時: 上記コンポーネントのユニットテストが `bun test` / `cargo test` で通る
  - _Requirements: 1.1, 1.2, 3.1, 3.4, 5.2, 6.1, 6.2, 7.5, 8.2, 9.1_
  - _Depends: 4.1, 4.3, 2.2, 3.2, 5.1, 5.2, 6.4_
  - _Wave: 27_

- [x] 10.2 統合テスト（設計 Integration 1–5）
  - mock `block-appended` → Ai エディタ末尾追記、ロック後追記で locked テキスト不変、invoke save → ファイル存在・内容一致、JSONL 有効/無効、保存中追加ブロック除外を検証する
  - 完了時: 統合テスト 5 件が `bun test` / `cargo test` で通る
  - _Requirements: 1.1, 3.1, 3.3, 7.1, 7.2, 7.3, 8.1, 8.3_
  - _Depends: 9.1_
  - _Wave: 28_

- [x] 10.3* E2E/UI テスト（設計 E2E 1–5）
  - 二重エディタ同時入力、選択ロック → 追記後 locked 保持、保存成功パス表示、保存先未設定通知、高頻度 mock 追記でカーソル維持を happy-dom + Slate で検証する
  - 完了時: フロント E2E テスト 5 件が `bun test` で通る（MVP 後に追加可能）
  - _Requirements: 2.3, 3.1, 4.3, 5.5, 7.6_
  - _Depends: 9.1_
  - _Wave: 29_

- [x] 10.4* 性能検証と手動チェックリスト
  - 500 ブロック追記 p95 < 16 ms、10 分相当 mock でメモリ増分 < 50 MB、保存 100 KB テキスト invoke + write < 500 ms を手動計測する
  - ログに転写全文・手動議事録全文が含まれないことを確認する
  - 完了時: 性能計測結果と pass/fail がチェックリストに記録されている
  - _Requirements: 4.4, 9.4, 10.1_
  - _Depends: 10.2_
  - _Wave: 30_

## Implementation Notes

- bun add は tailwindcss v4 を解決するため、shadcn の `tailwind.config.ts` 互換で v3.4.19 にピン留めした
- `bun run rust:typecheck` は Cursor の一時 `CARGO_TARGET_DIR` だと whisper-cpp-plus-sys の cmake が失敗する。`src-tauri/.cargo/config.toml` で `target` を固定し、ルートから `--manifest-path src-tauri/Cargo.toml` で実行する（`CARGO_TARGET_DIR=src-tauri/target` は `src-tauri/src-tauri/target` を誤生成するので不可）
- `saveTranscriptSession` の invoke は request をスプレッドしている。task 8.1 で Rust コマンド引数と合わせること
- 上流ブロック同期は `editor.applyUpstream(op)` 必須。直接 `apply` では locked 保護されない。task 7.2 で compose 時に接続すること
- `isAtDocumentEnd` は leaf offset とブロック文字数を比較している。ロックで split した末尾 leaf では誤判定しうる。task 7.2 で `Editor.end` ベースに直すこと

