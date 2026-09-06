# ADR-0006: transcript-editor のアプリ chrome に shadcn/ui を採用

- **Status**: Accepted
- **Date**: 2026-09-06
- **Feature**: transcript-editor
- **Owners / Domains**: transcript-editor

## Context

transcript-editor は二重 Slate エディタ（ADR-0005）に加え、保存・設定・通知・レイアウト分割などのアプリ chrome が必要である。初期設計は React コンポーネント + カスタム CSS（`editor-theme.css`）のみを想定していたが、利用者から shadcn/ui 採用が指示された。既存 gijirec フロントには shadcn が未導入のため、本 feature 投入時にホストアプリへ Tailwind + shadcn 基盤を追加する。

## Decision

**shadcn/ui** を transcript-editor の **アプリ chrome**（ツールバー、設定、保存結果通知、パネル区切り、エラー表示）に採用する。

- コンポーネント配置: `src/presentation/components/ui/`（shadcn CLI 出力先）
- ユーティリティ: `src/presentation/lib/utils.ts`（`cn()`）
- 設定: リポジトリルート `components.json`（`@/` → `src/presentation`）
- スタイル: `src/presentation/styles/globals.css` に Tailwind + shadcn セマンティックトークン。ユーザー提示パレットから **必要な色のみ** を `--primary` 等へブリッジし、Slate パネル背景は `editor-theme.css` で定義（未使用色の定義は不要）
- v1 で追加する shadcn コンポーネント: `button`, `switch`, `label`, `separator`, `alert`, `sonner`（保存結果 toast）
- **Slate エディタ本体**（AI 転写・手動議事録の編集面）は ADR-0005 のとおり Slate.js のまま。shadcn はエディタ contenteditable 内部には使わない

## Consequences

- Positive: アクセシビリティ付き Radix プリミティブ、一貫したボタン/トグル/通知 UX、将来のダークモードは CSS 変数差し替えで拡張しやすい
- Negative / trade-offs: ホストアプリ初の Tailwind + shadcn 導入コスト。`bun.lock` に Radix / CVA 依存が増える。Slate パネル色と shadcn トークンの二層管理が必要

## Alternatives considered

1. **カスタム CSS のみ継続** — 依存は少ないが、ツールバー/通知/フォームの一貫性を自前実装するコストが高い
2. **MUI / Chakra** — バンドルサイズと Tauri WebView でのスタイル競合リスク。プロジェクトに未採用
3. **shadcn で Slate も置換** — 不可。リッチテキスト編集は Slate 専任（ADR-0005）

## Notes

- shadcn / Tailwind メジャーアップグレード時は design の Revalidation Triggers を参照
- supply chain: `bun.lock` ピン + `bun run check` CI ゲート（steering security）
