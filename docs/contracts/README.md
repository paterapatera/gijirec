# Contracts Index

永続する契約面（API / イベント / データ所有）の索引。feature 削除後も残る。

## 読み方（必須）

1. **最初にこの index だけ**読む
2. 必要な path を選び、**そのファイルだけ** Read する
3. index に無いファイルを「念のため」開かない。**全量 Read 禁止**

## Entries

| ID / Path | One-line purpose | Owners / Domains |
|-----------|------------------|------------------|
| _(none yet)_ | — | — |

<!-- 例:
| contracts/billing-api.md | Billing HTTP API shape | billing |
| contracts/auth-session.md | Session cookie / token shape | auth |
-->

## 命名

- `<domain>-<surface>.md`（例: `billing-api.md`, `auth-session.md`）
- 1 ファイル = 1 契約面（API / イベント / データ所有のいずれか）

## 禁止

- `docs/specs/{feature}/contracts/` を **永続契約の正本にしない**
- 設計下書きを feature 内に置いてもよいが、GO 前に本ディレクトリへ書く
- `docs/architecture/boundaries.md` に契約詳細をコピペしない
- 契約ファイルを追加したのに **Entries を更新しない**（index 欠落禁止）

## テンプレ

- `docs/settings/templates/contracts/contract.md`
