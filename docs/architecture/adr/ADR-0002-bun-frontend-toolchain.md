# ADR-0002: フロントエンドパッケージマネージャに Bun を採用

- **Status**: Accepted
- **Date**: 2026-09-05
- **Feature**: audio-capture
- **Owners / Domains**: cross-cutting / audio-capture（ホストアプリ全体）

## Context

要件 8 は Tauri ホストの依存インストールとスクリプト実行に Bun を用い、npm を必須としないことを義務付ける。steering `tech.md` は Node.js 20.11+ を記載しているが、本 spec の要件が優先される。

## Decision

- `package.json` のスクリプトは `bun run` / `bun` で実行可能とする
- ロックファイルは `bun.lock` を正本とする
- `tauri.conf.json` の `beforeDevCommand` / `beforeBuildCommand` は `bun run dev` / `bun run build` を参照する
- CI と README は Bun 公式インストール手順を必須とし、npm install / npm run を前提にしない

## Consequences

- Positive: 単一ツールチェーン。Tauri 2 / create-tauri-app が Bun を公式サポート
- Negative / trade-offs: steering `tech.md` の Node/npm 記述は後続で更新が必要。チームは Bun 1.x をインストールする

## Alternatives considered

1. **npm / pnpm 継続** — 要件 8 違反
2. **pnpm のみ** — 要件 8 違反

## Notes

- Rust 側ツールチェーン（cargo）は変更なし
