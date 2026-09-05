# SDD Skill Integration

Read on demand when routing, Path B, requirements init, or impl monitoring needs detail.

## Roles (dispatch only)

| Role | Responsibility | Key skills |
| ---- | -------------- | ---------- |
| 調整者 | Routing, gates, rollback | `/sdd-orchestrate` |
| プロダクトオーナー | Requirements, brush-up, AI-DLC requirements final gate | spec-requirements (includes init Step 0), validate-requirements (unified; `/sdd-discovery` is an external pre-step, not dispatched) |
| セキュリティ管理者 | Requirements/design security | via unified validate-requirements / validate-design-qa |
| 設計者 | Design, tasks, AI-DLC design final gate | spec-design (inline brownfield gap), validate-design-qa (unified), spec-tasks |
| アーキテクト管理者 | SOLID, coupling | via unified validate-design-qa |
| 品質管理者 | Testability, edge cases, integration | validate-requirements (unified), validate-design-qa (unified), validate-impl |
| 実装者 | TDD per tasks | `/sdd-impl` (or main context for Path B) |

## Spec Init

要求新規作成: `/sdd-spec-requirements` is the **first orchestration generation step** (`/sdd-discovery` already produced `brief.md` standalone). Step 0 initializes `spec.json` + stub `requirements.md` when missing; then generates EARS requirements.

| Skill | Timing | Output |
| ----- | ------ | ------ |
| `/sdd-discovery` | **Standalone pre-step (before orchestration)** | Path, `brief.md` (C/D/E), `roadmap.md` (when spec deps exist) |
| `/sdd-spec-requirements` | Orchestration entry (要求新規作成) | `spec.json` (if missing) + EARS `requirements.md` body |

**Skip Step 0** when `docs/specs/<feature>/spec.json` already exists — continue from Load Context / requirements generation.

## Path B (直接実装)

- Decided by `/sdd-discovery` **before** orchestration; Path B work never enters an orchestration flow.
- No spec create/update; do not enter spec flow.
- Implement in main context — **no** `/sdd-impl` (no approved tasks).
- Verify with `/sdd-verify-completion` (`FIX` or `TEST_OR_BUILD`).
- **Not used**: `spec.json` gates, `/sdd-impl`, `/sdd-validate-impl`, mandatory `/sdd-review`.

## Impl Monitoring (`/sdd-impl`)

Batch / selection loop (delegate to impl skill — detail in `sdd-impl`):

1. Form next Wave/batch (or `direct` selection) → implementer TDD → `READY_FOR_REVIEW`
2. Parent mechanical checks → on FAIL, remediate (no reviewer yet)
3. Judgment `/sdd-review` (batch/selection-local) → `APPROVED` / `REJECTED`
4. On `APPROVED`: `/sdd-verify-completion` once (`BATCH`, or `TASK` only for a single manual task) — **not** after every APPROVED when more tasks remain unmarked
5. Mark all batch/selection tasks `[x]` + selective commit
6. `REJECTED` / mechanical FAIL: max 2 remediation rounds → `/sdd-debug` (fresh) → `_Blocked:_` on persistent failure

Orchestrator stops if `_Blocked:_` remains or tasks incomplete before `/sdd-validate-impl`. Autonomous impl mode auto-runs validate-impl on completion, then `FEATURE_GO` verify-completion.

## Validate Skill Boundaries

| Skill | Phase | Role |
| ----- | ----- | ---- |
| `requirements-review-gate` (in spec-requirements) | Pre-write | EARS mechanical + draft quality |
| `/sdd-validate-requirements` | Post-write (unified) | PO+QA+Sec+final+phase-gate → `requirements-review.md` |
| `/sdd-spec-design` | Design generation | Inline gap (brownfield) + discovery + `design.md` |
| `/sdd-validate-design-qa` | Design (unified) | QA+Arch+Sec+final+phase-gate → `design-review.md` |
| `/sdd-validate-impl` | Post-impl | Cross-task integration |
| `/sdd-verify-phase-gate` | Pre-approval (タスク; 要求/設計は統合内 or standalone debug) | Artifact + `VERDICT` readiness (`PHASE_GATE`) |
| `/sdd-verify-completion` | Impl batch/selection gate / Path B / post-impl | Fresh evidence (`BATCH`, `TASK`, `FIX`, `TEST_OR_BUILD`, `FEATURE_GO`) |

Validate I/O detail: `../sdd-validate-shared/phase-contracts.md`.
