# ADR Index

Architecture Decision Records。重要な設計判断を永続化する。contracts / boundaries の「いま有効な正本」とは役割が異なる（理由・代替・置換履歴はここ）。

**1 判断 = 1 ファイル。本文のマージ編集で履歴を消さない。**

## 読み方（必須）

1. **最初にこの index だけ**読む
2. 設計/実装で必要な `ADR-NNNN-...` を **関連 1〜2 件**まで Read する
3. **全量 Read 禁止**。index に無いファイルを開かない。この README の Entries 表以外の本文をまとめて読まない
4. 実装の日常タスクでは ADR 不要（境界変更・契約破壊・技術選択のタスクのみ）

## いつ書くか

次のいずれかに該当したら **新規 ADR ファイル**を作る:

- 依存方向・所有境界の変更
- 公開契約の破壊的変更
- 採用/不採用の大きな技術選択（後から理由が必要）
- Revalidation Triggers に触れる判断

書かないもの:

- 局所的な実装詳細
- 後からコードとテストだけ見れば分かる命名

全判断を ADR 化する義務はない。

## 更新規則（Superseded）

- **本文のマージ編集で履歴を消さない**（既存 ADR へ判断を上書きマージしない）
- 判断を覆す手順:
  1. **新規 ADR** を採番して作成する
  2. 旧 ADR の Status を `Superseded by ADR-XXXX` に変更する（本文の Context/Decision/Consequences は消さない）
  3. **この README の Entries** に新 ADR を登録し、旧エントリの Status も `Superseded by ADR-XXXX` に更新する（index 欠落禁止）
  4. contracts / boundaries の更新とセットで、関連 ADR を design の Persistent References に載せる

**Index 同期（必須）**: ADR ファイルを追加・置換したら、必ず本 Entries を更新する。index に無い ADR を「念のため」開かせないため、ファイル作成と Entries 更新はセット。

## Entries

| ID / Path | One-line purpose | Owners / Domains | Status |
|-----------|------------------|------------------|--------|
| _(none yet)_ | — | — | — |

## 命名・採番

- `ADR-NNNN-short-title.md`（**ゼロ埋め 4 桁**。例: `ADR-0001-prefer-event-driven-billing.md`）
- 次番号 = Entries の最大 NNNN + 1（欠番を埋めない）
- テンプレ: `docs/settings/templates/architecture/adr.md`

## フォーマット（最小）

各 ADR は次を持つ（詳細はテンプレ）:

- Status: `Proposed` | `Accepted` | `Superseded by ADR-XXXX`
- Date / Feature
- Context / Decision / Consequences
- Alternatives considered（短く）

## 禁止

- `docs/specs/{feature}/adr/` を永続 ADR の正本にしない
- ADR 全文を implementer プロンプトへ常時注入しない
- 既存 ADR 本文へ新判断をマージ上書きしない
