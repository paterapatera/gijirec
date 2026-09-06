# Research & Design Decisions: transcript-editor

## Summary
- **Feature**: transcript-editor
- **Discovery Scope**: New Feature（greenfield）/ Complex Integration（上流 whisper-transcribe + 二重 Slate エディタ + Rust 保存 I/O）
- **Key Findings**:
  - Slate.js は `editor.apply` オーバーライドと `locked` mark により部分ロック + 末尾追記のみ更新を自然に表現できる
  - 上流 `whisper-transcribe://block-appended` は既存 `TranscriptBlockBus` 経由でフロントへ到達 — 新規 Rust ブロック配信は不要
  - 保存 I/O は Rust 側に集約し、フロントはスナップショット送信のみ — セキュリティ・JST パス生成の一貫性

## Research Log

### エディタフレームワーク比較（Slate.js vs Lexical）
- **Context**: brief Approach「Slate.js や Lexical 等で部分ロックを制御」、要件 3・4 が核心
- **Sources Consulted**: Slate docs（Editable readOnly、editor.apply）、Lexical docs（Named Slots、UneditablePlugin）、Stack Overflow 事例、slate-transcript-editor（GitHub）
- **Findings**:
  - Lexical の部分 read-only は DecoratorNode / UneditablePlugin + EditorState 復元が必要で実装複雑度が高い
  - Slate は leaf mark（`locked: true`）+ `apply` フックで範囲保護が確立パターン
  - slate-transcript-editor はタイムコード整列向けで、リアルタイム追記 + 部分ロックモデルとは異なる
- **Implications**: ADR-0005 で Slate.js 採用。ブロック単位 `transcript-block` 要素 + `locked` mark アーキテクチャ

### 上流契約・既存 IPC パターン
- **Context**: whisper-transcribe 完了、フロントは `useTranscribeStatus` パターン確立
- **Sources Consulted**: `docs/contracts/whisper-transcribe-blocks.md`、`transcript_block_bus.rs`、`useTranscribeStatus.ts`
- **Findings**:
  - ブロックは Tauri イベント `whisper-transcribe://block-appended` で追記のみ配信
  - フロント契約ミラーは `src/presentation/hooks/` に配置、`listenFn` / `invokeFn` 注入でテスト
  - バックエンド 500 ブロックリング超過時は最古破棄 — フロントは sequence 欠番を許容しメトリクス記録
- **Implications**: `useTranscriptBlocks` を同パターンで追加。マウント時 replay は v1 対象外（イベントのみ）

### 保存・設定永続化
- **Context**: 要件 5–8 の JST サブディレクトリ・Markdown/JSONL 出力
- **Sources Consulted**: requirements.md、steering security.md、既存 Tauri commands パターン
- **Findings**:
  - 保存スナップショットは invoke 受信時点で確定（保存中ブロック除外）
  - 設定は app_data_dir JSON — 転写内容を含めない
  - 同一秒衝突は `_001` サフィックスで解決
- **Implications**: 3 契約ファイル（save / settings / status）を設計時に永続化

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Slate + TS domain + Rust save | 編集はフロント、I/O は Rust | steering レイヤ準拠、契約明確 | スナップショット同期の設計必要 | **採用** |
| Rust 側で全文保持 | バックエンドが編集状態を所有 | 単一ソース | ロック UI を Rust へ持込み不可 | 却下 |
| Lexical 双エディタ | Meta エコシステム | 活発な開発 | 部分ロック実装コスト | ADR-0005 で却下 |

## Design Decisions

### Decision: ブロック単位 Slate 要素 + locked mark
- **Context**: 要件 1.6（block_id 関連維持）、3（部分ロック）、4（追記安定）
- **Alternatives Considered**:
  1. 単一 paragraph へのテキスト連結 — block_id 喪失
  2. 外部 Map で offset 管理 — Slate 選択と同期困難
- **Selected Approach**: 各上流ブロックを `transcript-block` 要素として末尾追記。手動修正範囲に `locked` mark。JSONL  export は要素メタデータから生成
- **Rationale**: 追記は `insertNodes` at end のみ — 既存ノード不変で点滅・レイアウトシフト抑制
- **Trade-offs**: ブロック間の自然な連結表示は段落区切りになる（要件上問題なし — プレーンテキスト export）
- **Follow-up**: 高頻度追記時の selection 維持を E2E で検証

### Decision: 保存 I/O を Rust SaveService に集約
- **Context**: 要件 5–7、security（ローカル完結）
- **Selected Approach**: フロントが `save_transcript_session` invoke でスナップショット送信。Rust が JST パス生成・fs 書込
- **Rationale**: パス traversal 防止、JST 時刻の単一正本、エラー変換の domain 集約
- **Follow-up**: 部分失敗時の `files_written` / `files_failed` 契約テスト

### Decision: CSS カラートークン + shadcn/ui chrome
- **Context**: ユーザー指定デザイン制約 + shadcn/ui 採用指示
- **Selected Approach**: 提示パレットから v1 に必要な 8 色のみ定義。shadcn セマンティックトークンへ最小ブリッジ。Slate パネル・ロック装飾は `editor-theme.css`
- **Rationale**: アクセシビリティ付き chrome の一貫性を shadcn に委譲し、編集面は Slate 専任（ADR-0005 / ADR-0006）
- **Follow-up**: `bunx shadcn@latest init` + コンポーネント追加を Foundation タスクで実施

## Generalization（Synthesis）
- **TranscriptSnapshot** 抽象: 保存・将来 export で共通のスナップショット型（handwriting + ai blocks + locks）
- **EditorUserError**: whisper-transcribe と同型の `message_ja` / `action_ja` パターンを transcript-editor に一般化

## Build vs Adopt（Synthesis）
- **採用**: Slate.js（部分ロック）、shadcn/ui（アプリ chrome）、Tauri dialog（ディレクトリ選択）、既存 block-appended イベント
- **自前**: ロックプラグイン、追記安定化、JST サブディレクトリ生成（要件固有）

## Risks & Mitigations
- 高頻度 block 追記で selection 喪失 — `withStableSelection` + `overflow-anchor` + 実装テスト
- 500 ブロックリング破棄 — UI に欠落警告（sequence gap 検出）、保存前の利用者通知は v1 対象外
- slate-react と React 19 互換 — pin バージョン + CI typecheck

## References
- [Slate Editable readOnly](https://docs.slatejs.org/libraries/slate-react/editable) — 全体 readOnly（部分ロックはカスタム）
- [Lexical Named Slots](https://lexical.dev/docs/concepts/named-slots) — 部分編集の複雑さ参考
- `docs/contracts/whisper-transcribe-blocks.md` — 上流契約
- ADR-0005 — Slate.js 採用判断
- ADR-0006 — shadcn/ui chrome 採用判断
