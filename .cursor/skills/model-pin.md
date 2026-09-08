# Cursor SDD model pin

**Live config is only** [model-pin.yaml](model-pin.yaml). Bindings parse that file. **Do not parse this markdown as config.** There is no `roles` map until you add one in the yaml.

Default switch: edit `slug` in `model-pin.yaml`. Per-role exceptions: add `roles` there. Do not hardcode model names in SKILL.md or `cursor-bindings.md`.

| Field | Meaning |
|-------|---------|
| `slug` | Default `Task.model` for every **fresh** subagent |
| `fallback` | Used when the candidate slug is not in this session's available model list. `inherit` = parent model |
| `forbid_fast` | If true, never pass a slug whose name contains `fast` (case-insensitive) |
| `roles.<name>` | Optional override for that role. Missing keys use `slug` |

## Resolve (parent, before each fresh Task)

1. Read **[model-pin.yaml](model-pin.yaml)** only (once per parent invocation is enough). Ignore this `.md` file for values.
2. Candidate = `roles.<that role>` if that key is set and non-empty; otherwise `slug`.
3. If the candidate is not in this session's available model list → use `fallback`. Do **not** pick some other listed model.
4. If `forbid_fast` is true and the candidate contains `fast` → use `fallback` (or omit `model` if fallback is also Fast).
5. On `resume`: omit `model`. A pin change does not migrate in-flight sticky lineages.

## Per-role override

In `model-pin.yaml`, add `roles` and set **only** keys that should differ from `slug`. Example intent (not config): set `reviewer` to another listed slug; omit the rest.

| Key | Used by |
|-----|---------|
| `implementer` | `sdd-impl` implementer Task |
| `reviewer` | `sdd-impl` reviewer Task |
| `debugger` | `sdd-impl` debugger Task |
| `skill` | `sdd-orchestrate` whole-skill Task (`/sdd-spec-design` など) |
| `explore` | `sdd-spec-design` codebase / gap (`explore`) |
| `research` | `sdd-spec-design` external / API (`generalPurpose`) |

Use slugs from this session's available model list. Keys you omit keep `slug`.
