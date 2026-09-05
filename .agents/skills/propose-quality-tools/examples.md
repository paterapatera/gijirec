# Examples

最終提案の見出し・説明は **日本語**（ツール名・CLI・設定キーは英語のまま）。

## Example A — TypeScript medium (app)

**Input:** `/propose-quality-tools typescript M`  
**形状:** `app`（推論または明示）  
**モード:** `greenfield`

**採用:**

| 役割 | ツール | シグナル（近似） |
|------|--------|------------------|
| Format | Biome (v2+) | スタイル一貫性 |
| Types | `tsc` strict | LSP / DIP 契約（近似） |
| SOLID proxies | ESLint + typescript-eslint + sonarjs | サイズ/複雑度 ≈ SRP シグナル |
| Boundaries | dependency-cruiser | DIP / CCP / REP シグナル |
| Unused | Knip | CRP シグナル |

**不採用:** Prettier、madge、ArchUnitTS、有料 SonarCloud。

**スクリプト / CI:** `check` 内で `format-check && typecheck && lint && arch && knip`。ローカルの `format` は write 可。

---

## Example B — TypeScript small

**Input:** `/propose-quality-tools ts S`

**採用:** Biome + `tsc` + ESLint recommended（type-aware は任意）。

**後回し:** dependency-cruiser、Knip、jscpd。

**補足:** レイヤ/パッケージが現れたら M へ上げる。

---

## Example C — TypeScript large monorepo

**Input:** `/propose-quality-tools typescript L`  
**形状:** `monorepo`

**採用:** M 相当フル + 厳格化 + jscpd + workspace 境界（Nx enforce または package `exports`）+ 任意で SonarQube Community 自前ホスト。

**不採用:** 第2フォーマッタ、重複する境界リンタ。

**Polyglot 注意:** パッケージに Python/Go もある場合は **言語ごとに別呼び出し**。1 テーブルに混ぜない。

---

## Example D — Python medium (full shape)

**Input:** `/propose-quality-tools python M`  
**形状:** `app`  
**モード:** `greenfield`

**採用:**

| 役割 | ツール | シグナル（近似） |
|------|--------|------------------|
| Format + lint | Ruff | スタイル; 複雑度 (C90) ≈ SRP シグナル |
| Types | mypy strict | LSP / DIP 契約（近似） |
| Boundaries | import-linter | DIP / CCP / REP シグナル |
| Unused | Ruff unused | CRP シグナル |

**不採用:**

| ツール | 理由 |
|--------|------|
| Black + isort + flake8 | Ruff が三者を代替 |
| 有料 SonarCloud | 無料/OSS のみ |
| 第2の import-graph ツール | import-linter と重複 |

**ディレクトリ / 境界モデル:** `mypkg.domain` ← `application` ← 外側。infrastructure/presentation は内向き依存のみ（import-linter 契約）。

**設定スケッチ:** `pyproject.toml` の Ruff+mypy、import-linter の layers — reference 参照。

**スクリプト / CI:**

```text
format        → ruff format .
format-check  → ruff format --check .
lint          → ruff check .
typecheck     → mypy src
arch          → lint-imports
check         → format-check && lint && typecheck && arch
```

**ロールアウト:** 複雑度は warn → error。L へ上げるとき vulture/jscpd を追加。

---

## Example E — Missing args

**Input:** `/propose-quality-tools`

**挙動:** `language` と `scale`（`S`|`M`|`L`）を一度だけ聞く（境界モデルを誤推測しそうなら形状も）。その後提案。

---

## Example F — Existing Prettier + ESLint (conflict)

**Input:** 「Prettier と ESLint 済み。`/propose-quality-tools typescript M`」  
**形状:** `app`  
**モード:** `re-propose-diff`

**採用:**

| 役割 | ツール | シグナル（近似） |
|------|--------|------------------|
| Format | **Prettier 維持**（既に標準） | スタイル |
| Types | `tsc` strict | 型 |
| SOLID proxies | 既存 ESLint → 必要なら typescript-eslint type-checked + sonarjs を拡張 | 複雑度シグナル |
| Boundaries | dependency-cruiser（Nx/boundaries で未カバーなら） | DIP / CCP |
| Unused | Knip | CRP |

**不採用:**

| ツール | 理由 |
|--------|------|
| Biome（第2フォーマッタとして） | 二重フォーマット禁止。移行は後で任意 |
| madge | dependency-cruiser 採用時 |

**ロールアウト:** A: Prettier 維持 / B: 専用変更で Biome へ移行 — 併用しない。`check` は `prettier --check`。

---

## Example G — Unsupported language (rust)

**Input:** `/propose-quality-tools rust M`

**挙動:**

- rust は正準マトリクスの **未対応** と明記。
- docs/名前を軽く確認したうえで役割アナロジーを提案し、**確信度: low**。
- スキル公式の既定スタックだと主張しない。
- 未検証の点を列挙。

---

## Example H — Library shape (TypeScript)

**Input:** 「npm ライブラリ。`/propose-quality-tools typescript M`」  
**形状:** `library`

**採用:** Biome（または既存 Prettier）、`tsc` strict、ESLint+sonarjs、公開エントリ中心の Knip、`src` vs `internal` 向け dependency-cruiser（`presentation/` は不要）。

**不採用:** `domain/application/infrastructure/presentation` のアプリ用レイヤ強制。

**境界モデル:** 公開 exports / entrypoints ↔ internal。テストは公開 API を import。

---

## Example I — C# medium (ArchUnitNET default)

**Input:** `/propose-quality-tools csharp M`  
**形状:** `app`

**採用:** `dotnet format` + Roslyn/StyleCop + ArchUnitNET（+ ビルドを型ゲート）。

**不採用:** NDepend 商用; NetArchTest を *新規* greenfield 既定にしない（リポジトリ既存なら維持可）。

**補足:** 設定スケッチ出力前に ArchUnitNET API を freshness-check。

---

## Example J — Polyglot monorepo

**Input:** 「TS + Python の monorepo。`/propose-quality-tools`」

**挙動:** 先に言語を聞く **または** 2 本提案（typescript M + python M）し、共有 CI メモのみ。Biome と Ruff を言語分割なしで 1 つの採用表に混ぜない。
