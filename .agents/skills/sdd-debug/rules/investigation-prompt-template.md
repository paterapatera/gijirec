# Investigation Prompt Template

Use when the user wants a **structured investigation prompt** before or instead of vague debugging chat.
Output: one investigation prompt block only (no commentary), unless the user asks for explanation.

Meta-prompts below are in **English** to save tokens when pasted into chat. Generated investigation prompts should be **Japanese** (readable for local review and handoff).

## When to use

- Starting a new debug session after manual smoke failure
- Resetting a loop of "still broken" / repeated fix attempts
- Handing off from one chat to another with consistent structure

Do not use as a substitute for running `sdd-debug` Phase 0 inside the agent.

---

## Short meta-prompt (daily use)

Fill and send (output: investigation prompt body only, in Japanese):

```
Fill and output one investigation prompt only (no commentary):

Feature:
Symptom:
Environment:
Logs/errors:
Related docs:
Constraints: no code until HIGH / one hypothesis / smoke after fix

Required structure: symptom class → env diff table (5–8 rows) → signal checklist (≤10 rows) → one hypothesis (CONFIDENCE) → one check step → user follow-ups (max 5)
Start with: "Run /sdd-debug from Phase 0 (no code changes)."
Use feature-specific terms from inputs and related docs.
Output in Japanese. ~200–400 words.
```

---

## Meta-prompt (full)

Fill `{...}` and send:

```
You write structured investigation prompts for software failures.
From the inputs below, produce exactly one "investigation maintenance prompt" for an implementer (AI) to run via `/sdd-debug`.

## Inputs
- Feature/area: {feature}
- Symptom (short): {symptom}
- Environment: {env, e.g. Windows release + --log / dev only / both}
- Known logs/errors (optional): {paste or none}
- Related docs (optional): {paths, e.g. docs/specs/<feature>/smoke-checklist.md}
- Constraints (optional): {default: no code changes first / one hypothesis at a time / smoke required after fix}

## Rules
1. Output the investigation prompt body only — no preamble or explanation.
2. Required structure:
   - Symptom class (non-functional / error / degraded) — pick one primary from inputs
   - Environment diff table (5–8 rows, SAME/DIFFERS/UNKNOWN)
   - Signal checklist from logs (≤10 rows, PASS/FAIL/MISSING)
   - One hypothesis (CONFIDENCE: HIGH/MEDIUM/LOW) + exactly one next check (prefer no code edit)
   - Up to 5 items to request from the user at end of round
3. Start fixes only when CONFIDENCE is HIGH.
4. Specify minimal post-fix smoke (build type, duration, input, success criteria).
5. Use feature-specific terms from inputs and related docs — not generic placeholders only.
6. ~200–400 words.

## Output format
---
{Investigation prompt for sdd-debug Phase 0. First line must say to run /sdd-debug starting at Phase 0 (no code changes).}
---

Output the investigation prompt in Japanese.
```

---

## Example (gijirec / fix-release-transcribe)

### Input

```
Feature: fix-release-transcribe
Symptom: no transcription on release manual smoke
Environment: Windows, dev OK, release + --log NG
Logs: enters transcribing but no UI blocks; CPU ~0%
Related docs: docs/specs/fix-release-transcribe/smoke-checklist.md
Constraints: no code first, one hypothesis at a time
```

### Generated prompt (reference output, Japanese)

```
/sdd-debug を実行し、Phase 0（コード変更禁止）から始めてください。

fix-release-transcribe の release 手動確認で文字起こしされない。

まずコードは変えず、次を順に出して:

1. 症状分類（機能しない / エラー / 遅い・品質悪い）— ログから主分類を1つ決める
2. dev vs release の差分表（ModelStore パス、setup/inject 順、モデルロード開始タイミング、block-appended ACL、TranscribeStallWatchdog 配線、worker 致命エラー surface、cmake/whisper /O2 フラグ）— 各行 SAME/DIFFERS/UNKNOWN
3. 貼った --log の信号チェックリスト（loading_model→ready→transcribing、whisper engine ready、inference_latency、block-appended、INFERENCE_FAILED/stall、モデルファイル存在とサイズ）— PASS/FAIL/MISSING
4. 上記から最も有力な仮説1つ（CONFIDENCE 付き）と、次に1つだけ試す確認手順（リポジトリ grep / ファイル存在 / ビルドログの /O2 確認など）
5. このラウンド終了時にユーザーへ求める追加情報（最大5項目）

修正は CONFIDENCE: HIGH になってから。
各修正後は release ビルドで30秒マイク入力スモーク（10秒以内に最初のブロックが出ること）を実行し、信号チェックリストを再評価する。unit test / verify だけでは完了としない。
```

---

## Integration note

When `sdd-debug` runs with a user-supplied prompt like the example:

1. Execute Phase 0 literally (tables + hypothesis).
2. Merge into `## Triage (Phase 0)` in the final output.
3. Proceed to Method 1–6 only after Phase 0 is complete.
