# Boundaries

<!-- Output: docs/architecture/boundaries.md（全体俯瞰のみ。巨大化したら分割） -->
<!-- Keep detailed contracts in docs/contracts/; do not paste them here -->

全体の境界と依存方向のみ。

## 依存方向

| From | To | Rule |
|------|-----|------|
| {{FROM}} | {{TO}} | {{RULE}} |

## {{DOMAIN}} ドメイン境界

<!-- Repeat per feature/domain. One section = Owns / Out / Allowed Dependencies / 依存方向 -->

`docs/specs/{{FEATURE}}/` の設計に基づく。契約の正本は `docs/contracts/` を参照。

### Owns（この Spec が所有）

| 領域 | コンポーネント / 成果物 |
|------|-------------------------|
| {{AREA}} | {{COMPONENTS}} |

### Out of Boundary（境界外）

| 領域 | 備考 |
|------|------|
| {{AREA}} | {{NOTE}} |

### Allowed Dependencies（許可依存）

| 種別 | 依存 |
|------|------|
| {{KIND}} | {{DEPS}} |

## 境界メモ

- {{BOUNDARY_NOTE}}
