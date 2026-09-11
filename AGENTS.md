# Agent Instructions

正本: `docs/steering/tech.md`

- **完成判定**: `bun run verify`（完了・`FEATURE_GO` は exit 0 のみ）
- **修正ループ**: `bun run verify:agent` → 失敗は `PRIMARY_FAILURE` / `.verify-agent/last-report.json` → `--step <failedStep>` で再実行 → 最後に `verify`

```bash
bun run verify:agent
bun run verify:agent -- --step rust:lint
bun run verify:agent -- --from test
```

`verify:agent` PASS だけで完成宣言しない。`.verify-agent/` / `.jscpd-report/` はコミットしない。
