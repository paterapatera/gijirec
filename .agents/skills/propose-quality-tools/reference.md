# Reference — language matrices & configs

**Canonical matrices and config sketches.** Read when emitting concrete proposals. Do not duplicate long stacks into `SKILL.md`.

**Last verified:** 2026-08-12 (golangci-lint v2 sketch + Go Types role)  

If official docs disagree with a sketch below, **docs win** (see SKILL freshness check). Update this file when fixing drift; bump `Last verified`. Agents must still key-check emitted sketches against current docs even when this date is recent.

## Principle → automation map (signals only)

| Principle | Automate via (approx. signal) | Review still needed |
|-----------|-------------------------------|---------------------|
| SRP | complexity, max-lines/params, cognitive complexity | true single reason to change |
| OCP | layer boundaries, plugin ports | extension design |
| LSP | strict types, interface contracts | behavioral substitutability |
| ISP | small interfaces, unused members/exports | interface shape |
| DIP | forbidden deps (domain↛infra) | abstraction quality |
| REP | package/script isolation, versioned packages | release cohesion |
| CCP | same-change grouping via modules | change-reason analysis |
| CRP | unused public API, no fat util barrels | reuse coupling judgment |

Never claim tools “cover” or “certify” a principle — proposal column is **Signals (approx.)**.

## CI / `check` policy (all languages)

| Script | Purpose |
|--------|---------|
| `format` / equivalent | Local write / fix (developer machine only) |
| `format-check` | Non-destructive format verification for CI |
| `check` | All gates: `format-check` + type + lint + arch + unused (+ dup on L) |

Never put auto-write format inside `check`. Adapt names to ecosystem (`gofmt -l`, `ruff format --check`, `dotnet format --verify-no-changes`, etc.).

## Overlap avoidance

| Keep one of | Drop the other |
|-------------|----------------|
| Default formatter for the language | A second formatter |
| One primary boundary tool | madge + eslint-plugin-boundaries + ArchUnit\* as simultaneous defaults |
| Knip (TS) / ruff unused (Py) | ts-prune / duplicate dead-code runners alone |
| Type-aware SOLID lint primary | A weaker linter as *equal* primary for the same role |
| import-linter (Python) | Extra custom import graphs without need |

If the repo already standardized on the “drop” side, **keep existing** and reject the default instead (see SKILL conflict rules).

## Project shape — folder sketches

### `app` (layered service)

```text
src/
  domain/
  application/
  infrastructure/
  presentation/
scripts/          # no reverse deps from src
```

### `library` (publishable package)

```text
src/              # public API surface
  index.ts        # or pkg root
internal/         # or _internal — not exported
tests/
```

Boundaries: public entry ↔ internal; tests may import public API. No forced `presentation/`.

### `monorepo`

```text
packages/
  core/
  api/
  web/
```

Enforce package dependency directions (Nx enforce, package `exports`, or dep-cruiser across packages). **Polyglot:** one language matrix per package language — do not mix tool rows.

### `scripts`

```text
scripts/          # or tools/
```

Format + types + light lint; architecture optional.

---

## TypeScript / JavaScript

### Defaults (greenfield)

| Role | Default tool |
|------|----------------|
| Format | Biome (**v2+** sketches below) |
| Types (TypeScript) | `tsc --noEmit` + `strict` |
| Types (JavaScript) | `tsc --noEmit` + `checkJs` / JSDoc **or** defer type role and note gap |
| SOLID proxies | ESLint + typescript-eslint (type-checked) + eslint-plugin-sonarjs; for plain JS use ESLint + sonarjs without type-aware TS rules |
| Boundaries | dependency-cruiser |
| Unused | Knip |
| Duplication (L) | jscpd |

### Scale matrix

| Tool | S | M | L |
|------|---|---|---|
| Biome (or kept Prettier — one only) | ✅ | ✅ | ✅ |
| `tsc --noEmit` strict (TS) / checkJs (JS) | ✅ | ✅ | ✅ |
| ESLint + typescript-eslint (TS) | ✅ recommended | ✅ strictTypeChecked | ✅ + tighter thresholds |
| ESLint + sonarjs (JS; no type-aware TS) | ✅ | ✅ | ✅ |
| eslint-plugin-sonarjs | optional | ✅ | ✅ |
| dependency-cruiser | optional | ✅ | ✅ error-heavy |
| Knip | optional | ✅ | ✅ |
| jscpd | ❌ | optional | ✅ |
| SonarQube Community (self-host) | ❌ | ❌ | optional |
| Nx / workspace boundaries | ❌ | if monorepo | ✅ if monorepo |

### `package.json` scripts (M, TypeScript)

```json
{
  "scripts": {
    "format": "biome check --write .",
    "format-check": "biome check .",
    "lint": "eslint .",
    "typecheck": "tsc --noEmit",
    "arch": "depcruise src scripts --config .dependency-cruiser.cjs",
    "knip": "knip",
    "check": "npm run format-check && npm run typecheck && npm run lint && npm run arch && npm run knip"
  }
}
```

If keeping Prettier: `"format": "prettier --write ."`, `"format-check": "prettier --check ."`.

Adapt runner (`npm` / `pnpm` / `bun`) to the repo.

### `tsconfig.json` (core, TypeScript)

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "skipLibCheck": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "noEmit": true
  },
  "include": ["src/**/*", "scripts/**/*"]
}
```

### `biome.json` (Biome **v2+**)

```json
{
  "$schema": "https://biomejs.dev/schemas/2.0.0/schema.json",
  "assist": {
    "actions": {
      "source": {
        "organizeImports": "on"
      }
    }
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineWidth": 100
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "complexity": {
        "noExcessiveCognitiveComplexity": "off"
      }
    }
  },
  "files": {
    "includes": ["**", "!**/node_modules", "!**/dist", "!**/coverage"]
  }
}
```

Leave cognitive complexity to ESLint/sonarjs to avoid double reporting. If Biome major > sketch schema, run freshness check and prefer current docs / `biome migrate`.

### `eslint.config.js` (M, TypeScript)

```js
import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import sonarjs from "eslint-plugin-sonarjs";

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  sonarjs.configs.recommended,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "max-lines-per-function": ["warn", { max: 80, skipBlankLines: true, skipComments: true }],
      "max-params": ["warn", 4],
      complexity: ["warn", 12],
      "sonarjs/cognitive-complexity": ["warn", 15],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/consistent-type-imports": "error",
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  { ignores: ["node_modules/**", "dist/**", "coverage/**", "**/*.cjs"] },
);
```

**S:** use `recommendedTypeChecked` or non-type-aware recommended; raise thresholds.  
**L:** promote key `warn` → `error`; lower max-lines/complexity slightly if team agrees.  
**JavaScript:** drop type-aware `typescript-eslint` configs; keep ESLint + sonarjs size/complexity rules.

### `.dependency-cruiser.cjs` (M, `app` shape)

```js
/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-circular",
      severity: "error",
      from: {},
      to: { circular: true },
    },
    {
      name: "domain-no-outer",
      severity: "error",
      from: { path: "^src/domain" },
      to: { path: "^src/(infrastructure|presentation|application)" },
    },
    {
      name: "application-no-presentation",
      severity: "error",
      from: { path: "^src/application" },
      to: { path: "^src/presentation" },
    },
    {
      name: "application-no-infra-deep",
      severity: "warn",
      from: { path: "^src/application" },
      to: { path: "^src/infrastructure" },
    },
    {
      name: "scripts-isolated",
      severity: "warn",
      from: { path: "^src" },
      to: { path: "^scripts" },
    },
  ],
  options: {
    doNotFollow: { path: "node_modules" },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: "tsconfig.json" },
  },
};
```

For `library` shape: forbid `src`→deep coupling into test-only paths; enforce public entry vs `internal/` instead of app layers.

### `knip.json`

```json
{
  "entry": ["src/**/*.ts", "scripts/**/*.ts"],
  "project": ["src/**/*.ts", "scripts/**/*.ts"]
}
```

### Rejected by default (TS/JS) — greenfield

| Tool | Reason |
|------|--------|
| Prettier / dprint | Biome is default format (if Prettier already standard → keep Prettier, reject Biome) |
| madge | dependency-cruiser covers cycles/graphs |
| ArchUnitTS + eslint-plugin-boundaries | overlap with dependency-cruiser (unless already adopted) |
| ts-solid-linter | immature for primary gate |
| Oxlint as sole SOLID gate | weak type-aware depth vs typescript-eslint |
| Paid SonarCloud / CodeClimate | paid; CE only if L + self-host |

---

## Python

### Defaults (greenfield)

| Role | Default tool |
|------|----------------|
| Format + lint | Ruff |
| Types | mypy (strict) |
| Boundaries | import-linter (M/L) |
| Unused | Ruff unused; optional vulture (L) |
| Duplication (L) | jscpd or pylint duplicate checks sparingly |

### Scale matrix

| Tool | S | M | L |
|------|---|---|---|
| Ruff format + check | ✅ | ✅ | ✅ |
| mypy strict | ✅ | ✅ | ✅ + stricter overrides |
| import-linter | optional | ✅ | ✅ |
| vulture | ❌ | optional | optional |
| jscpd (or similar) | ❌ | optional | ✅ |
| SonarQube Community | ❌ | ❌ | optional |

### Scripts (M)

```toml
# pyproject.toml — script aliases via hatch/poetry/make; example Makefile targets:
# format:        ruff format .
# format-check:  ruff format --check .
# lint:          ruff check .
# typecheck:     mypy src
# arch:          lint-imports
# check:         format-check + lint + typecheck + arch
```

```bash
ruff format --check .
ruff check .
mypy src
lint-imports
```

`check` = all of the above (non-destructive). Local write: `ruff format .` and `ruff check --fix` only outside CI.

### Config sketches

**`pyproject.toml` (Ruff + mypy core):**

```toml
[tool.ruff]
line-length = 100
target-version = "py312"

[tool.ruff.lint]
select = ["E", "F", "I", "B", "UP", "SIM", "C90"]

[tool.ruff.lint.mccabe]
max-complexity = 12

[tool.mypy]
python_version = "3.12"
strict = true
warn_return_any = true
warn_unused_configs = true
mypy_path = "src"
packages = ["mypkg"]
```

**`importlinter` contracts (`app` shape):**

```ini
[importlinter]
root_package = mypkg

[importlinter:contract:layers]
name = Layered architecture
type = layers
layers =
    mypkg.presentation
    mypkg.application
    mypkg.domain
    mypkg.infrastructure
```

Adjust layer order to the team's dependency rule (inner packages must not import outer). For `library` shape: contracts on public package vs `_internal` instead of presentation.

### Rejected by default (Python)

| Tool | Reason |
|------|--------|
| Black + isort + flake8 combo | Ruff covers format+lint in one tool |
| Pylint as primary (S/M) | heavier; optional supplement only |
| Paid cloud quality platforms | free/OSS rule |
| Second import-graph tool beside import-linter | overlap |

---

## Go

### Defaults (greenfield)

| Role | Default tool |
|------|----------------|
| Format | `gofumpt` (or `gofmt` if already standard) — keep **outside** golangci `formatters` to avoid dual write |
| Types | `go build ./...` (compile gate; align Go version with `go.mod`) |
| Lint / SOLID proxies | `golangci-lint` **v2+** (incl. staticcheck, complexity) |
| Boundaries | package layout + `go-arch-lint` (M/L) |
| Unused | `unused` / staticcheck via golangci-lint |

### Scale matrix

| Tool | S | M | L |
|------|---|---|---|
| gofmt / gofumpt | ✅ | ✅ | ✅ |
| `go build ./...` | ✅ | ✅ | ✅ |
| golangci-lint | ✅ | ✅ | ✅ stricter |
| go-arch-lint | optional | ✅ | ✅ |
| dupl / similar | ❌ | optional | ✅ |
| SonarQube Community | ❌ | ❌ | optional |

### Scripts (M)

```bash
# format (local write)
gofumpt -w .

# format-check (CI)
test -z "$(gofumpt -l .)"

# typecheck
go build ./...

# lint + unused
golangci-lint run ./...

# arch
go-arch-lint check

# check
# format-check && go build ./... && golangci-lint run ./... && go-arch-lint check
```

### Config sketch — `.golangci.yml` (golangci-lint **v2+**)

```yaml
version: "2"
linters:
  default: standard
  enable:
    - errcheck
    - govet
    - staticcheck
    - unused
    - funlen
    - gocyclo
  settings:
    funlen:
      lines: 80
    gocyclo:
      min-complexity: 12
# Do not enable formatters.go* here when using standalone gofumpt/gofmt for the Format role.
# If golangci major > sketch, run freshness check / `golangci-lint migrate` and prefer current docs.
```

### Config sketch — `go-arch-lint` (`app` shape)

Keep packages like `internal/domain`, `internal/app`, `internal/infra`, `cmd/...` with rules: domain ↛ infra/cmd; app ↛ cmd UI wiring as agreed. Prefer current `.go-arch-lint.yml` schema from upstream docs if this sketch ages.

### Rejected by default (Go)

| Tool | Reason |
|------|--------|
| Second formatter (`gofmt` + `gofumpt` both writing) | one format tool |
| ESLint/dep-cruiser analogues from JS | wrong ecosystem |
| Paid SaaS quality clouds | free/OSS rule |

---

## Java

### Defaults (greenfield)

| Role | Default tool |
|------|----------------|
| Format | Spotless (google-java-format) |
| Types | `javac` via Gradle/Maven compile (treat warnings policy on L) |
| SOLID proxies | Checkstyle + PMD (+ SpotBugs optional) |
| Boundaries | ArchUnit tests (M/L) |
| Unused | SpotBugs / PMD unused + IDE; optional dedicated dead-code on L |
| Quality platform | SonarQube Community optional (L, self-host) |

### Scale matrix

| Tool | S | M | L |
|------|---|---|---|
| Spotless / google-java-format | ✅ | ✅ | ✅ |
| Compile / javac gates | ✅ | ✅ | ✅ + `-Werror` optional |
| Checkstyle / PMD | ✅ light | ✅ | ✅ stricter |
| SpotBugs | optional | ✅ | ✅ |
| ArchUnit | optional | ✅ | ✅ |
| SonarQube Community | ❌ | ❌ | optional |

### Scripts / Gradle (M)

```bash
./gradlew spotlessCheck   # format-check
./gradlew spotlessApply   # format write (local)
./gradlew check           # include test + ArchUnit + static analysis as configured
```

Ensure CI uses `spotlessCheck` (not `spotlessApply`). Wire ArchUnit under `test` or a dedicated task included in `check`.

### Config sketch — ArchUnit (idea)

```java
@AnalyzeClasses(packages = "com.example")
class ArchitectureTest {
  @ArchTest
  static final ArchRule domain_independent =
      noClasses().that().resideInAPackage("..domain..")
          .should().dependOnClassesThat()
          .resideInAnyPackage("..infrastructure..", "..presentation..");
}
```

### Rejected by default (Java)

| Tool | Reason |
|------|--------|
| Dual format plugins | one formatter |
| Paid SonarCloud / commercial analyzers as required | free/OSS; CE self-host only optional at L |
| JS boundary tools | wrong ecosystem |

---

## C#

### Defaults (greenfield)

| Role | Default tool |
|------|----------------|
| Format | `dotnet format` |
| Types | `dotnet build` (nullable enable / treat warnings as errors on L) |
| Analyzers / SOLID proxies | Roslyn analyzers / StyleCop Analyzers (OSS) |
| Boundaries | **ArchUnitNET** (M/L) — prefer over unmaintained NetArchTest |
| Unused | Roslyn IDE0005 / analyzers; optional dead-code on L |

If the repo already uses NetArchTest or NetArchTest.eNhancedEdition, **keep it** (conflict: converge). For greenfield, default ArchUnitNET; verify current NuGet package id before emit.

### Scale matrix

| Tool | S | M | L |
|------|---|---|---|
| `dotnet format` | ✅ | ✅ | ✅ |
| Roslyn / StyleCop Analyzers | ✅ | ✅ | ✅ stricter |
| ArchUnitNET | optional | ✅ | ✅ |
| SonarQube Community | ❌ | ❌ | optional |

### Scripts (M)

```bash
dotnet format                # write (local)
dotnet format --verify-no-changes   # format-check (CI)
dotnet build /p:TreatWarningsAsErrors=false
dotnet test                  # include ArchUnitNET test project
# check: format-check + build + test
```

### Config sketch — ArchUnitNET (`app` shape)

```csharp
// using static ArchUnitNET.Fluent.ArchRuleDefinition;
// Load Architecture once (ArchLoader / fixture) — verify against current docs.
[Fact]
public void Domain_DoesNotReference_Infrastructure()
{
    IArchRule rule = Types().That().ResideInNamespace("MyApp.Domain")
        .Should().NotDependOnAny(
            Types().That().ResideInNamespace("MyApp.Infrastructure"));

    rule.Check(Architecture);
}
```

Sketch is illustrative — **freshness-check** package ids and fluent API against current ArchUnitNET docs before emit.

### Rejected by default (C#)

| Tool | Reason |
|------|--------|
| NDepend commercial as required | paid |
| NetArchTest (BenMorris) as *new* greenfield default | effectively unmaintained; use only if already adopted or team standard |
| Second formatter beside `dotnet format` | overlap |
| JS/Python tools | wrong ecosystem |

---

## Rollout checklist (any language)

1. Add formatter; CI uses **format-check**, not write.
2. Turn on strict types (or document defer for plain JS).
3. Add complexity rules as **warn**.
4. Add boundary rules for the innermost layer / public API first (per shape).
5. Add unused-export / dead-code gate.
6. On L: duplication + monorepo boundaries; promote warns to errors.
7. Re-propose later as **diff** (keep/add/replace), not a second greenfield stack.
