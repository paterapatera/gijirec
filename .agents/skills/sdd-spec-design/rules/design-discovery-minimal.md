# Minimal Discovery

Use when: greenfield + simple scope.

Steps:
1. Read brief.md Approach and Constraints — treat as binding.
2. If brief cites reference implementation (path or repo name), read at most 3 representative files (e.g. index, one tool, one error module) — no sub-agent.
3. If external API is named, verify endpoint paths via at most 1 WebFetch to official docs — no broad WebSearch.
4. Write condensed findings to research.md (optional, ≤ 40 lines) OR skip research.md and fold into design.md Overview.
5. Do NOT produce extension scenarios table, mermaid, or file tree unless requirements explicitly demand them.

Design document target length guidance: ≤ 150 lines for simple scope.
