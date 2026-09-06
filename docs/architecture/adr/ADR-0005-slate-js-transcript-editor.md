- **Status**: Accepted
- **Date**: 2026-09-06
- **Feature**: transcript-editor
- **Owners / Domains**: transcript-editor

## Context

transcript-editor は部分ロック付き二重エディタ（手動議事録 + AI ストリーミング転写）を新規実装する。brief および tech.md は Slate.js / Lexical 等のリッチテキストフレームワークを候補としている。要件 3（部分ロック）・4（追記のみ・レイアウト安定）・1.6（ブロック ID と表示内容の関連維持）がエディタ選定の主要制約である。

## Decision

**Slate.js**（`slate` + `slate-react`）を AI 転写エディタおよび手動議事録エディタのフレームワークとして採用する。

- AI 転写: カスタム `transcript-block` 要素（`blockId` / `startTimestampMs` メタデータ）+ `locked` テキスト mark + `withLockedRanges` / `withAppendOnlyBlocks` プラグイン
- 手動議事録: 独立 Slate エディタインスタンス（シンプルな paragraph / text モデル）
- 永続化・ファイル I/O は Rust Tauri コマンド（フロントはスナップショット送信のみ）

## Consequences

- Positive: カスタム `editor.apply` による範囲単位ロック・追記のみ更新が実装しやすい。転写編集の先行事例（slate-transcript-editor）と同系統。TypeScript strict との親和性が高い。
- Negative / trade-offs: Lexical よりプラグイン実装のボイラープレートが多い。React 19 との slate-react 互換は実装時に pin バージョン検証が必要。

## Alternatives considered

1. **Lexical** — Meta 主導で活発。部分 read-only は UneditablePlugin + 状態復元パターンが必要で、追記のみストリーミングとの統合が Slate より複雑。
2. **contenteditable 素の React** — 依存最小だが部分ロック・選択維持・追記安定性の自前実装コストが L tier に見合わない。
3. **slate-transcript-editor ライブラリ直接採用** — 音声 URL・話者分離・DPE 形式など本 spec スコープ外機能が多く、部分ロックモデルも異なる。

## Notes

- ブロック形状変更時は transcript-editor 全体の再検証トリガー（上流 `whisper-transcribe-blocks.md` Changelog 参照）
