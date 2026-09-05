# Architecture Index

永続する境界・依存・ADR の索引。feature 削除後も残る。

## 読み方（必須）

1. **最初にこの index だけ**読む
2. 必要な path を選び、**そのファイルだけ** Read する
3. index に無いファイルを「念のため」開かない。**全量 Read 禁止**

## 配置

| Path | One-line purpose | Owners / Domains |
|------|------------------|------------------|
| `docs/architecture/boundaries.md` | 全体境界・依存方向の俯瞰 | cross-cutting |
| `docs/architecture/adr/` | 重要判断（ADR）。詳細は [adr/README.md](./adr/README.md) | — |

## 命名

- ADR: `ADR-NNNN-short-title.md`（例: `ADR-0001-use-postgres.md`）
- 1 判断 = 1 ファイル。既存 ADR 本文をマージ編集して履歴を消さない

## 禁止

- `docs/specs/{feature}/adr/` や feature 配下を **永続 architecture / ADR の正本にしない**
- 詳細契約を `boundaries.md` にコピペしない（契約は `docs/contracts/`）
- feature 配下の作業資料をここへ移さない（永続化するのは境界・ADR・contracts のみ）

## テンプレ

- 境界・ADR 雛形: `docs/settings/templates/architecture/`
