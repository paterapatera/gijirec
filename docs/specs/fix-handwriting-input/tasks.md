# 実装計画

- [x] 1. テスト基盤: block 更新時の手入力保持検証
- [x] 1.1 block-appended イベントを合成して手入力テキストの変化を検証するテストヘルパーを追加する
  - `useTranscriptBlocks` の `listenFn` 注入パターンを利用し、block 追加をプログラム的に発火できる
  - テスト実行時に手入力エディタの DOM テキストが block 追加前後で同一であることを検証できる
  - _Requirements: 1.1, 3.1_
  - _Wave: 1_

- [x] 2. HandwritingEditor の再描画抑止と IME ガード
- [x] 2.1 HandwritingEditor を React.memo でラップし composition 状態を追跡する
  - `memo(forwardRef(...))` でコンポーネントをラップし、props 未変更時の再描画を抑止する
  - `onCompositionStart` / `onCompositionEnd` で `isComposing` ref を管理し、composition 中の Slate 内容を保護する
  - 既存の `getPlainText()` ref API が変更なく動作することを確認する
  - _Requirements: 1.2, 1.4, 2.1, 2.2, 2.3, 2.4_
  - _Boundary: HandwritingEditor_
  - _Design: D-HandwritingEditor_
  - _Wave: 2_

- [x] 3. AI 転写購読のサブツリー分離
- [x] 3.1 AiTranscriptPanel を新規作成し block 購読を局所化する
  - `useTranscriptBlocks` を Panel 内部に移動し、`AiTranscriptEditor` に `blocks` を渡す
  - Panel の state 更新が `TranscriptEditorView` 親を再描画しない構造になる
  - block-appended 発火時に AI エディタのみが更新されることを確認する
  - _Requirements: 1.1, 1.2, 1.3_
  - _Boundary: AiTranscriptPanel_
  - _Design: D-AiTranscriptPanel_
  - _Wave: 3_

- [x] 3.2 TranscriptEditorView から block 購読を除去し ref を安定化する
  - `useTranscriptBlocks` 呼び出しを除去し `AiTranscriptPanel` を配置する
  - `mergeRefs` を `useCallback` でメモ化し、HandwritingEditor の不要な ref 再割当を防ぐ
  - transcribe error 購読（要件 9.3）とツールバー配置が既存動作を維持する
  - _Requirements: 1.3, 3.4_
  - _Boundary: TranscriptEditorView_
  - _Design: D-TranscriptEditorView_
  - _Depends: 3.1_
  - _Wave: 4_

- [x] 4. 統合検証
- [x] 4.1 二重エディタ画面の統合テストで手入力保持を確認する
  - block-appended を連続発火させた後も手入力テキストが保持される
  - AI 転写エディタは block 追加に応じて更新される
  - 保存フロー（`getPlainText` → save）が既存動作を維持する
  - _Requirements: 1.1, 2.3, 3.1, 3.4_
  - _Depends: 2.1, 3.2_
  - _Wave: 5_

- [x] 5. 品質ゲート
- [x] 5.1 `bun run verify` を実行し全テスト・lint を通過する
  - `bun run verify` がエラーなく完了する
  - 新規・更新テストがすべてパスする
  - _Requirements: 3.2, 3.3_
  - _Depends: 4.1_
  - _Wave: 6_

- [x] 5.2* HandwritingEditor の composition イベント単体テストを追加する
  - composition 開始〜終了の合成イベントで Slate 内容が保持されることを検証する
  - _Requirements: 2.1, 2.2_
  - _Boundary: HandwritingEditor_
  - _Design: D-HandwritingEditor_
  - _Depends: 2.1_
  - _Wave: 6_

## Implementation Notes

- `TranscriptEditorView` の ref 安定化は `useCallback` + `externalHandwritingRef` パターンで実装（`exactOptionalPropertyTypes` 対応のため `AiTranscriptPanel` への ref は条件付き spread）
