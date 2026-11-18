# Frozen specs — semantics and marker

Decided: 2026-11-17 · pi-workflows plan `20261117-068`, task 2 · pinned by
`tests/frozen_specs.rs` (`test_frozen_specs_are_marked`).

## What "frozen" means

The `boundaries-path` lint (and the guard/lifecycle pipeline it runs inside)
applies to **new and revised specs only**. A spec whose scenarios cite example
paths outside its own Allowed Changes is a contract defect in a *living*
spec — in a spec whose scenarios already executed against historical code
trees, those citations are part of the record, not open defects.

Frozen specs are a **historical record**: their citation text is not rewritten
to satisfy the current lint, because the flagged set is frozen history. The
rule itself is untouched — frozen is a scope decision, not a carve-out in the
lint. Any spec that gets revised goes back under the normal rule in its
revised form.

## The marker

Each frozen spec carries one frontmatter line, the same form everywhere:

```text
frozen: 20261117 plan-20261117-068
```

- `20261117` — date of the carve-out decision (ISO 8601 basic format).
- `plan-20261117-068` — the plan that made the decision.

The parser ignores unknown frontmatter keys (`spec_parser::meta::parse_meta`),
so the marker is inert to parsing while remaining machine-checkable by grep.
Un-frozening a spec means rewriting its citations in a revision AND removing
the marker AND updating the hardcoded list in `tests/frozen_specs.rs` —
three deliberate edits, never a silent drift.

## The frozen set (enumerated 2026-11-17)

16 `boundaries-path` findings across 11 files, enumerated with per-file
`cargo run -- lint` runs. (Before v0.3.3, a combined `lint specs/*.md
specs/roadmap/*.md` run silently stopped at `specs/roadmap/README.md` — no
front-matter — so combined runs under-reported; the workaround was to
enumerate per file. v0.3.3 fixed the abort: combined runs now report every
good file and name every bad one at the end — see
[docs/gate-scope.md](gate-scope.md) for why the roadmap directory sits
outside guard's gate scope.)

| Spec | Findings |
|------|----------|
| `specs/task-add-behavior-completeness-linters.spec.md` | 1 |
| `specs/task-derive-change-set-from-staged-git-index.spec.md` | 1 |
| `specs/task-enforce-boundaries-with-explicit-change-set.spec.md` | 4 |
| `specs/task-phase2-run-history-and-vcs-context.spec.md` | 1 |
| `specs/task-support-change-scope-in-verify-and-lifecycle.spec.md` | 1 |
| `specs/task-support-git-worktree-change-scope.spec.md` | 1 |
| `specs/task-support-spec-md-extension.spec.md` | 1 |
| `specs/task-verification-cache.spec.md` | 1 |
| `specs/roadmap/task-phase2-run-history-and-vcs-context.spec.md` | 1 |
| `specs/roadmap/task-plan-command.spec.md` | 3 |
| `specs/roadmap/task-strengthen-rewrite-contract-authoring.spec.md` | 1 |
