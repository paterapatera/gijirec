# Research & Design Decisions

## Summary

- **Feature**: fix-handwriting-input
- **Discovery Scope**: Brownfield / Simple Addition
- **Key Findings**:
  - `TranscriptEditorView` が `useTranscriptBlocks` で `block-appended` を購読し、更新のたびに全体が再描画される
  - `HandwritingEditor` は `React.memo` 未使用で、親再描画に連動する
  - `mergeRefs` が毎レンダーで新しい callback ref を生成し、memo 化を阻害する
  - IME composition ハンドラは現状未実装

## Gap Analysis

### Current State

| Asset | Location | Relevance |
|-------|----------|-----------|
| HandwritingEditor | `src/presentation/components/HandwritingEditor.tsx` | props なし Slate、memo なし |
| TranscriptEditorView | `src/presentation/components/TranscriptEditorView.tsx` | block 購読の親、再描画の起点 |
| AiTranscriptEditor | `src/presentation/components/AiTranscriptEditor.tsx` | `blocks` prop で上流同期（手入力とは独立） |
| useTranscriptBlocks | `src/presentation/hooks/useTranscriptBlocks.ts` | `whisper-transcribe://block-appended` 購読 |

### Requirement-to-Asset Map

| Requirement | Existing Asset | Gap |
|-------------|----------------|-----|
| 1.1–1.3 手入力独立性 | HandwritingEditor, TranscriptEditorView | Missing: 再描画ツリー分離 |
| 2.1–2.3 IME 安定性 | HandwritingEditor | Missing: composition ガード |
| 3.1–3.4 回帰防止 | 既存テスト | Missing: block 更新時の手入力保持テスト |

### Implementation Options

| Option | Description | Trade-offs |
|--------|-------------|------------|
| A: memo のみ | `React.memo` + ref 安定化 | 最小変更だが親 state 更新は残る |
| B: 購読分離 | block 購読を AI 側サブツリーへ移動 | 根本的な再描画抑止、推奨 |
| C: Hybrid | B + memo + IME composition ガード | 最も堅牢、本 spec で採用 |

### Effort & Risk

- **Effort**: S（1–3 日）— 既存パターンの拡張、IPC 変更なし
- **Risk**: Low — presentation 層のみ、契約面変更なし

## Research Log

### Re-render propagation path

- **Context**: brief の仮説をコードで確認
- **Sources Consulted**: HandwritingEditor.tsx, TranscriptEditorView.tsx, useTranscriptBlocks.ts
- **Findings**:
  - `block-appended` → `setState` → TranscriptEditorView 再描画 → HandwritingEditor 再描画
  - HandwritingEditor は blocks を受け取らないが親再描画で Slate DOM が churn
- **Implications**: 購読を AI サブツリーに移すのが第一手段

### IME composition in Slate

- **Context**: 要件 2 の実現手段
- **Sources Consulted**: Slate React Editable API
- **Findings**: `onCompositionStart` / `onCompositionEnd` で composition 状態を追跡可能。再描画抑止が主、composition ガードは補助
- **Implications**: 再描画分離後も composition ref ガードを追加し二重防御

## Design Decisions

### Decision: AI ブロック購読のサブツリー分離

- **Context**: 要件 1.2 — 親の AI 転写状態更新による再描画の影響を受けない
- **Alternatives Considered**:
  1. React.memo のみ
  2. 新規 `AiTranscriptPanel` で購読を局所化
- **Selected Approach**: `AiTranscriptPanel`（または同等のラッパー）が `useTranscriptBlocks` を保持し、`TranscriptEditorView` は toolbar + HandwritingEditor + Panel を静的に配置
- **Rationale**: 根本原因（親 state 更新）を除去
- **Trade-offs**: 1 ファイル追加 vs 確実な隔離

### Decision: HandwritingEditor の memo 化と ref 安定化

- **Context**: 要件 1.2, 3.1
- **Selected Approach**: `React.memo` でラップ、`mergeRefs` を `useCallback` で安定化
- **Rationale**: ツールバー props 変更時の不要な再描画も抑止

## Risks & Mitigations

- Slate の `initialValue` が再マウント時にリセットされる — 購読分離により再マウントを防止
- memo 化で ref 更新が阻害される — `useCallback` で ref callback を安定化
- テストで IME を完全再現困難 — composition イベントの合成テストで代替

## References

- `docs/specs/fix-handwriting-input/brief.md` — 問題定義とアプローチ
- `docs/steering/structure.md` — presentation 層パターン
