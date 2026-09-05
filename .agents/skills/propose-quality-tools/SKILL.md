---
name: propose-quality-tools
description: >-
  Proposes a free/OSS quality toolchain (format, types, SOLID proxies,
  REP/CCP/CRP boundaries) for a language and scale (S|M|L). Use when the user
  invokes /propose-quality-tools or asks to propose quality/architecture
  tooling with a language and scale.
disable-model-invocation: true
---

# Propose Quality Tools

Propose a **free/OSS-only** toolchain and concrete settings for:

- Component cohesion: **REP**, **CCP**, **CRP**
- Class design: **SOLID**
- **Formatter** (+ supporting lint/typecheck)

Do **not** recommend paid products or paid SaaS tiers as primary options.

**Source of truth for matrices/configs:** [reference.md](reference.md). This file is the contract and workflow only. Principle→automation detail lives only in reference — do not duplicate long maps here.

## Scope

**In scope:** format, typecheck, SOLID *proxies* (complexity/size), dependency/boundary rules, unused/dead-code, duplication (L), CI `check` gates.

**Out of scope (do not require):** test runners, coverage thresholds, security scanners (SAST/deps), performance profilers, docs generators — mention only if the user asks.

**Language mode:** one primary `language` per invocation. Polyglot monorepos → emit **one proposal per language** (or ask which language first); share CI orchestration notes briefly, do not mash ecosystems into one Adopted table.

## Invocation

```text
/propose-quality-tools <language> <scale>
```

| Arg | Meaning | Examples |
|-----|---------|----------|
| `language` | Target language/ecosystem | see Supported languages |
| `scale` | Project size | `S` small, `M` medium, `L` large |

Normalize aliases: `ts`→`typescript`, `js`→`javascript`, `c#`/`dotnet`→`csharp`.

If language or scale is missing, ask once, then proceed.

### Supported languages

| Language | Notes |
|----------|--------|
| `typescript`, `javascript` | Full depth in reference (`javascript`: no `tsc`; see reference) |
| `python` | Full depth in reference |
| `go` | Full depth in reference |
| `java` | Full depth in reference |
| `csharp` | Full depth in reference |

### Unsupported languages

If the language is **not** in the table (e.g. `rust`, `ruby`, `php`, `kotlin`):

1. Say it is unsupported for a canonical stack.
2. Still propose by **role analogues** (format / type / SOLID proxies / boundaries / unused) using well-known OSS for that ecosystem.
3. Mark **Confidence: low** in the proposal header; do not invent a fake “official” matrix.
4. Prefer fewer tools; list what you could not verify (spot-check current docs/names before naming defaults).

## Scale definitions

| Scale | Rough size | Goal |
|-------|------------|------|
| **S** | scripts, small apps, few packages | Minimal friction; format + type + light lint |
| **M** | single service / modular app | Recommended default stack |
| **L** | monorepo, many modules, teams | Boundaries, dead-code, duplication, CI gates |

## Project shape

Before proposing a directory/boundary model, resolve **shape** (ask once if unclear; else infer from the repo):

| Shape | Typical signals | Boundary focus |
|-------|-----------------|----------------|
| `app` | deployable service/UI | layered dirs or packages (inner↛outer) |
| `library` | publishable package, no app shell | public API / `exports` / tests; avoid app DDD theater |
| `monorepo` | workspaces, multiple packages | package boundaries + workspace enforce |
| `scripts` | CLIs, glue, few modules | format + types; architecture optional |

Do **not** force `domain/application/infrastructure/presentation` onto `library` or `scripts`. Shape-specific folder sketches: [reference.md](reference.md).

## Hard rules

1. Prefer **few complementary tools** over overlapping suites.
2. CCP/CRP/REP/SOLID are **partially** automatable; tools give **signals/proxies**, never certification.
3. Split roles: **format** vs **type** vs **SOLID proxies** vs **architecture (REP/CCP/CRP/DIP)**.
4. Always return: adopted tools, rejected alternatives (with reason), config sketches, scripts/CI, growth path S→M→L.
5. Named tools in reference are **defaults**, not dogma — apply **conflict rules** below.
6. **Freshness:** before emitting config sketches, **always** spot-check keys/CLI against **current official docs** for every sketch you emit — do not skip because `Last verified` is recent. Prefer docs over stale reference. If docs diverge, use current docs, note the delta in Rollout, set **Confidence: low** (or keep normal only if the delta is trivial naming).
7. **Output language:** the emitted proposal (headings, tables, explanations, Rollout) MUST be **Japanese**. Keep tool names, CLIs, config keys, and code fences in their original (usually English) form.
8. Language details: [reference.md](reference.md). Examples: [examples.md](examples.md).

## Conflict rules (existing toolchain)

Inspect the repo (or user statements) before locking defaults:

| Situation | Action |
|-----------|--------|
| Formatter already standard (e.g. Prettier) | **Keep** it **or** propose a **migration** to the default; never dual-format. Document choice in Rejected/Rollout |
| Boundary tool already standard (Nx enforce, eslint-plugin-boundaries, ArchUnit, …) | Prefer **converge** on the existing tool; do not add a second primary boundary runner |
| Monorepo system present (Nx, Turborepo, pnpm workspaces, …) | Reuse its boundary/`exports` mechanisms; add dep-cruiser only if gaps remain |
| User demands paid SaaS (SonarCloud paid, etc.) | Refuse as primary; offer free/OSS alternatives; optional CE self-host only at L |
| Stack already matches a prior proposal / quality toolchain present | **Re-propose as diff:** Adopted = keep / add / replace; Rejected = remove or avoid; Rollout = migration only — do not rewrite a greenfield stack blindly |

## Workflow

Copy and track:

```text
Progress:
- [ ] 1. Parse language + scale (+ unsupported / polyglot path)
- [ ] 2. Resolve project shape (+ existing toolchain / re-propose?)
- [ ] 3. Select stack from reference (apply conflict rules)
- [ ] 4. Apply scale deltas
- [ ] 5. Freshness check (docs vs reference sketches)
- [ ] 6. Pre-emit self-check
- [ ] 7. Emit proposal (fixed output format)
- [ ] 8. Offer optional next step (write configs / install) — do not implement unless asked
```

### 1. Parse inputs

- Resolve language + scale; handle unsupported per section above.
- If the repo is clearly polyglot and the user did not pick a language, ask which language (or emit separate proposals).
- Note runtime (`bun`, `node`, `pnpm` workspaces) only when visible; otherwise stay ecosystem-generic.

### 2. Resolve shape

- Set `app` | `library` | `monorepo` | `scripts` (or mix: monorepo+library packages).
- Detect existing format/lint/boundary tools → conflict / re-propose mode.
- Choose boundary model from reference for that shape.

### 3. Select base stack

Use the language matrix in [reference.md](reference.md). Apply conflict rules.

**Role philosophy (all languages):**

| Concern | What to pick |
|---------|----------------|
| Format | One formatter (never two) |
| Types / contracts | Compiler or type checker in strict mode (`javascript`: JSDoc/`checkJs` or defer) |
| SOLID proxies | Complexity / size / cognitive-complexity rules |
| REP / CCP / CRP / DIP | Dependency/boundary rules + unused-export detection |
| CI | Single `check` = **non-destructive** gates including **format check** (never `--write` / auto-fix in CI) |

### 4. Apply scale deltas

| Scale | Include | Exclude / defer |
|-------|---------|-----------------|
| **S** | Formatter + typecheck + recommended lint | Architecture / dead-code optional; Sonar-like optional |
| **M** | Full recommended stack (format + type + SOLID lint + boundaries + unused) | Heavy platform (SonarQube CE); extra monorepo plugins unless shape needs them |
| **L** | M + stricter errors + duplication + monorepo boundaries if applicable + optional SonarQube Community self-host | Paid analyzers; duplicate formatters; duplicate boundary tools |

**Rejection defaults (unless conflict rules say keep existing):** second formatter; madge when a boundary tool exists; two ArchUnit-like tools together; Oxlint **and** full type-aware ESLint as equal primaries; paid cloud quality platforms.

### 5. Freshness check

- Read reference `Last verified` and sketches (`Last verified` is a freshness hint, **not** a skip gate).
- For **each** config sketch you will emit, confirm keys/CLI against current official docs (or well-known changelog) — always, including when `Last verified` is today.
- Extra care when the tool has had major releases since `Last verified`, or for unsupported/niche defaults.
- On conflict: **current docs win**; mention in Rollout; lower Confidence if the change is material.

### 6. Pre-emit self-check

Before emitting, verify:

- [ ] Exactly one formatter role filled
- [ ] Every adopted tool maps to a role (format / type / SOLID / arch / unused / dup)
- [ ] Rejected table non-empty when overlaps or paid alternatives exist
- [ ] Boundary model matches **shape** (not default app layers blindly)
- [ ] `check` includes format **check**, not format write
- [ ] No principle claimed as “certified”; 採用表は **シグナル（近似）** のみ
- [ ] Unsupported language **or** material docs drift ⇒ Confidence: low
- [ ] Freshness check done for emitted sketches
- [ ] Proposal body is Japanese (tool/CLI/config identifiers may stay English)

### 7. Emit proposal

Write the **entire proposal body in Japanese**. Use this exact structure (Japanese headings; English tool/CLI names OK):

```markdown
# 品質ツール提案

**言語:** <language>
**規模:** <S|M|L>
**形状:** <app|library|monorepo|scripts>
**モード:** <greenfield|re-propose-diff>
**前提:** 無料/OSS のみ。ツールは原則の近似シグナルであり、認定しない
**確信度:** <normal|low>   <!-- unsupported・強い推測・docs の実質乖離なら low -->

## 採用スタック
| 役割 | ツール | シグナル（近似） |
|------|--------|------------------|
| ... | ... | ... |

## 不採用
| ツール | 理由 |
|--------|------|
| ... | ... |

## ディレクトリ / 境界モデル
<形状に合ったフォルダ/パッケージと許可する依存方向>

## 設定スケッチ
<採用ツールの最小設定 — reference が古い場合は現行 docs 優先>

## スクリプト / CI
<コマンド: format（ローカルは write 可）, format-check, lint, typecheck, arch, dead-code, check>
<!-- check は破壊的でないゲートのみ。format-check を含める（--write 禁止） -->

## ロールアウト
1. まず warn → 次第に error へ
2. S→M または M→L で強化すること
3. 既存ツール差し替え時の移行メモ
4. reference との docs 差分（あれば）
```

Defaults per language live only in [reference.md](reference.md) — do not fork a second matrix or principle map here.

## What not to promise

- A single linter that “certifies SOLID” or “certifies CCP/CRP/REP”
- Paid tools (SonarCloud paid, NDepend commercial, etc.) as required
- Installing or writing repo configs unless the user explicitly asks after the proposal
- One mashed toolchain for a polyglot monorepo without per-language proposals
