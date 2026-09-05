# Maintain — propose-quality-tools

正本の鮮度を保つための運用メモ。契約は `SKILL.md`、行列・設定は `reference.md`。

## いつ更新するか

- 提案時に公式 docs とキー／CLI が食い違ったとき
- 既定ツールの major が変わったとき
- メンテ停止・後継が明確になったとき
- Supported に言語を追加するとき

## 何を直すか（優先順）

1. `reference.md` — Defaults / scale / scripts / config sketch / Rejected
2. `Last verified` をその日に bump（何を直したか括弧で一言）
3. 壊れやすい例だけ `examples.md`
4. 契約が変わるときだけ `SKILL.md`（役割・有料禁止・Freshness・出力テンプレ／日本語必須）

古いキーは残さない。docs を正とする。原則マップを SKILL に二重定義しない。

## 貼り付け用プロンプト

### 差分修正（いちばん使う）

```text
@.agents/skills/propose-quality-tools

propose-quality-tools の正本を現行公式 docs に合わせて更新してください。

対象: <language またはツール名。例: go / golangci-lint>
原因: <提案時の docs delta / major 変更 / メンテ停止>

ルール:
- 直すのは主に reference.md（Defaults / scale / scripts / config sketch / Rejected）
- 古いキーは残さない。docs を正とする
- Last verified を今日に bump（何を直したか括弧で一言）
- 契約（SKILL.md）は役割・有料禁止・Freshness・テンプレが変わらない限り触らない
- examples.md は壊れやすい例だけ最小更新
- 更新後、該当言語で /propose-quality-tools <lang> M をドライラン（提案のみ。実装・ファイル作成なし）
- 変更点を短く列挙して終える
```

### 年次棚卸し

```text
@.agents/skills/propose-quality-tools

Supported 全言語について、reference.md の Defaults と config sketch を現行公式 docs と突合してください。

- 不一致だけ修正（推測でツール総入れ替えしない）
- メンテ停止が明確なら Defaults 差し替え、または「既存収束のみ」
- Last verified を bump
- 提案の実装・インストールはしない
- 変更点と未着手（問題なし）を短く列挙
```

### 単言語スポットチェック（直さず確認のみ）

```text
@.agents/skills/propose-quality-tools

<language> の reference 既定と config sketch を現行公式 docs と突合し、差分だけ報告してください。
ファイルは変更しない。実装・インストールもしない。
```
