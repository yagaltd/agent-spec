# plan-check — validating plan structure

Decided: 2026-11-17 · pi-workflows plan `20261117-074`, task 3 · pinned by
`tests/gate_docs.rs` (`plan_check_docs_page_documents_sections`); the behavior
itself is pinned by the `plan_check_*` tests in `src/main.rs`.

`agent-spec plan-check` is the CLI gate that validates a plan.md file for
structural correctness. It is documentation of existing behavior: everything
below is verifiable against `cmd_plan_check`, `check_plan`,
`parse_dependencies`, `validate_ledger_resolution`, `valid_disposition`,
`validate_sieve`, `extract_task_ids` and `detect_cycle` in `src/main.rs`.

## The command

```bash
agent-spec plan-check <PLAN> [--format text|json]
```

- `<PLAN>` — positional path to the plan file to validate.
- `--format` — `text` (default) or `json`.

### Output

Text format (header rule is 40 `=`):

```text
plan-check: .workflows/plan.md
========================================
✅ Plan is valid
Tasks: 3
Dependencies: 2
```

Problems are listed between the verdict and the counts, one per line:
`  ERROR: <message>` for errors, `  WARN:  <message>` for warnings.

JSON format emits one object with the fields `plan` (the path as given),
`valid` (bool), `errors` (array), `warnings` (array), `tasks` (task IDs found
in the spec map) and `dependencies` (each edge rendered `from → to`).

### Exit codes

| Exit | When |
|---|---|
| `0` | the plan is valid (no errors) |
| `1` | the plan has at least one error — stderr shows `error: plan validation failed` |
| `1` | the plan file is missing or unreadable — stderr shows `error: <io error>` |

Warnings never change the exit code: a plan with warnings but no errors is
valid.

## What it validates

`valid` is exactly "the error list is empty" (`check_plan`). Errors and
warnings come from four validated areas plus one consistency check. When a
section is absent from the plan, its check is a no-op — absence never fails.

### 1. Dependencies (optional — absent means no edges, no errors)

The `## Dependencies` block declares the task DAG one line per task:

```markdown
## Dependencies

TASK_1: []
TASK_2: [TASK_1]
TASK_3: [TASK_1, TASK_2]
```

Grammar (`parse_dependencies`): each line is `TASK_<id>: [TASK_A, TASK_B]` —
comma-separated targets, an empty `[]` means no dependencies. Task IDs are
collected from the spec map: `### TASK 1` headings (normalized to `TASK_1`)
plus any `TASK_N:` dependency lines (`extract_task_ids`).

- ERROR — unknown target:
  `Dependency target 'TASK_9' not found in spec map — referenced by TASK_2`
- ERROR — cycle (`detect_cycle`, DFS):
  `Circular dependency detected: TASK_1 → TASK_2 → TASK_1`
- WARN — unknown source:
  `Dependency source 'TASK_0' not found in spec map`

### 2. Ledger Resolution (optional — a plan without the section is a no-op)

The `## Ledger Resolution` section (`validate_ledger_resolution`, pi-workflows
ledger doctrine) is a table whose rows carry a Disposition cell:

```markdown
## Ledger Resolution

| # | Item | Disposition | Note |
|---|---|---|---|
| 1 | docs debt | done(TASK_2) | budgets land |
| 2 | bombadil gap | closed(accepted-gap DEC-008) | nothing to do |
| 3 | later | deferred(trigger: revisit at v1.0) | wait |
```

- The header row is located by its cells: the column whose cell reads
  `Disposition` (case-insensitive) is the Disposition column and the column
  reading `Item` is the Item column. Fallback template positions: Item = 2nd
  column, Disposition = 3rd. Separator rows (`|---|`) and non-table prose are
  skipped.
- The Disposition cell must be exactly one token (`valid_disposition`), no
  prose tail, kinds case-sensitive:
  - `done(<ref>)` — reference non-empty
  - `closed(<reason>)` — reason non-empty
  - `deferred(trigger: <condition>)` — the literal `trigger: ` prefix with a
    non-empty condition
- Errors name the Item cell and the 1-based line:
  - `ledger row 'bad row' (line 6): invalid disposition 'finished(T9)' — expected done(<ref>), closed(<reason>) or deferred(trigger: <condition>)`
  - `ledger row 'truncated' (line 4): missing Disposition cell`

### 3. Bottleneck (a whole-plan tag scan, not a section)

Every `**Bottleneck**:` tag in the plan is checked against the fixed enum:

```markdown
- **Bottleneck**: 🔴 BLOCKING — waves 1–2
```

An emoji (`🔴 🟡 🔵 🟠 ⚪`) between the colon and the tag is optional and
stripped before matching (`check_plan`). The tag must begin with one of:
`BLOCKING`, `RISKY`, `TIME-CONSUMING`, `VERIFICATION-HEAVY`, `STANDARD`
(case-sensitive).

- WARN — unknown tags are warnings, never gate failures:
  `Unknown bottleneck tag: 'CRITICAL' — expected one of: BLOCKING, RISKY, TIME-CONSUMING, VERIFICATION-HEAVY, STANDARD`
- No `**Bottleneck**:` tags in the plan → no warnings.

### 4. Sieve (optional — a plan without the section is a no-op; non-proposal plans stay legal)

The `## Sieve` section (`validate_sieve`, pi-workflows plan `20261117-073`
normative grammar) is a forced-choice Cell|Verdict table plus an Outcome line:

```markdown
## Sieve

| Cell | Verdict |
|---|---|
| V value | valuable |
| R risk | contained |
| E effort | s |
| path | none |

Outcome: propose
```

- The header row is located by its `Cell` / `Verdict` cells
  (case-insensitive); fallback template positions: Cell = 1st column,
  Verdict = 2nd.
- Each row's Cell is matched by its first word (`V`, `R`, `E` or `path`,
  case-insensitive), and the table needs one row for each of the four kinds.
- Verdict enums (trimmed, case-insensitive):

  | Cell | Allowed verdicts |
  |---|---|
  | V | `nice-to-have`, `useful`, `valuable`, `must-have` |
  | R | `trivial`, `contained`, `serious`, `severe` |
  | E | `xs`, `s`, `m`, `l` |
  | path | `research`, `inspect`, `grill`, `none` |

- The `Outcome:` line (key case-insensitive, value trimmed and
  case-insensitive) must be `propose` or `build-direct`. `park` — or any
  other non-enum — is an error:

  `invalid Outcome 'park' (line 10) — expected one of: propose, build-direct; parked ideas are ledger rows, never plans`

- All Sieve errors quote the offending cell and the 1-based line:
  - `sieve row 'V value' (line 5): invalid verdict 'must' — expected one of: nice-to-have, useful, valuable, must-have`
  - `sieve row 'future' (line 5): unknown Cell — expected V, R, E or path`
  - `sieve row 'path' (line 8): missing Verdict cell`
  - `Sieve section (line 2): missing E row — the Cell|Verdict table needs one row for each of V, R, E, path`
  - `Sieve section (line 2): missing Outcome line — expected 'Outcome: propose' or 'Outcome: build-direct'`

### Additional check: contract-status consistency

Beyond the four areas, `check_plan` counts `Contract status … WRITTEN`
mentions and `Contract … specs/` spec-path mentions across the whole plan and
warns when the former outnumbers the latter:

`3 tasks marked WRITTEN but only 2 have spec file paths`

### Section scoping

`## Ledger Resolution` and `## Sieve` are found by their heading line and end
at the next `## ` heading; text outside such a section is never validated by
that section's rules. Concretely: prose mentioning `Outcome: park` without a
`## Sieve` heading is legal (`plan_check_ignores_missing_sieve`), and a plan
with neither section is valid (`plan_check_ignores_missing_section`).

## plan-check and the repo-level gates

plan-check validates one plan file and needs no repo context: its only inputs
are the positional `<PLAN>` path and `--format`. The option flags known from
the other gates are **not** plan-check flags — they belong to the commands
that gate code changes against contracts (`src/main.rs`, `Commands` enum):

| Flag | Defined by | Default |
|---|---|---|
| `--spec-dir` | `guard`, `graph` | `specs` |
| `--code` | `verify`, `lifecycle` (required); `guard`, `explain`, `stamp`, `resolve-ai`, `plan`, `measure-determinism` | `.` where defaulted |
| `--change` | `verify`, `lifecycle`, `guard` | none (repeatable) |
| `--change-scope` | `verify`/`lifecycle` (`none`), `guard` (`staged`) | per command |
| `--min-score` | `lint` (`0.0`), `lifecycle`, `guard` (`0.6`) | per command |
| `--cache` | `lifecycle`, `guard` | off |

A pi-workflows wave therefore gates plan and contracts in two commands:

```bash
agent-spec plan-check .workflows/plan.md
agent-spec guard --spec-dir .workflows/specs --code . --change-scope worktree --min-score 0.7
```

## Related

- [gate-scope.md](gate-scope.md) — what `guard` scans and why roadmap specs
  sit outside the gate scope.
- Ledger doctrine: parked ideas are ledger rows, never plans — that is why
  `Outcome: park` is an error and why the ledger Disposition grammar has no
  `parked` token (a parked idea is tracked with `deferred(trigger: …)`).
