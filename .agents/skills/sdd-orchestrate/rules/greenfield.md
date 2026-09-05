# Greenfield Detection

A feature is **greenfield** when ALL hold:

1. `brief.md` § Current State states no implementation / 緑地 / greenfield, OR
2. No production source under project src/ (excluding docs/, .agents/, templates), AND
3. No `docs/specs/<feature>/design.md` from a prior implementation cycle

If ambiguous → not greenfield (safer to run gap).

Orchestrator and spec-design MUST read this before any gap or codebase sub-agent dispatch.
