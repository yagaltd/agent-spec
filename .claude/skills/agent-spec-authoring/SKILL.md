---
name: agent-spec-authoring
description: Authoring workflow for agent-spec Task Contracts and self-hosting specs. Use when Claude Code needs to create or update `.spec` files, rewrite `Intent`/`Decisions`/`Boundaries`/`Completion Criteria`, add explicit `Test:` or `测试:` selectors, tighten deterministic acceptance criteria, or maintain `specs/project.spec` and task-level self-hosting specs.
---

# Agent Spec Authoring

Write contracts that are easy for humans to review and easy for `agent-spec` to verify mechanically.

## Workflow

1. Read the project rule.
   Read `specs/project.spec` first when it exists.

2. Study nearby examples.
   Read sibling task specs in `specs/` and any relevant files in `examples/`.

3. Write or update the contract.
   Use the Task Contract shape:
   `Intent`
   `Decisions`
   `Boundaries`
   `Completion Criteria`
   Optional: `Out of Scope`

4. Make the contract mechanically useful.
   Add explicit `Test:` / `测试:` selectors for every task scenario.
   Use structured selector blocks with `Package` / `Filter` when package scoping matters.
   Write path-like boundaries when the rule should be machine-enforced.
   Keep completion criteria deterministic and specific.

5. Validate the contract.
   Run `agent-spec contract` to inspect the rendered execution surface.
   Run `agent-spec lifecycle` after edits if the repo already has matching tests.

Read [references/patterns.md](references/patterns.md) when you need authoring patterns or selector/boundary examples.

## Authoring Rules

- Keep `Intent` focused on what to do and why.
- Put already-fixed technical choices in `Decisions`, not in vague prose.
- Put allowed files/modules and forbidden moves in `Boundaries`.
- Write `Completion Criteria` as BDD scenarios with deterministic pass/fail meaning.
- Use step tables for structured inputs instead of inventing custom prose blocks.
- Keep provider/model/auth details out of the contract unless they are domain requirements of the product being built.

## Self-Hosting Rules

When authoring specs for the `agent-spec` project itself:

- Put task specs under `specs/`
- Update tests when DSL or verification behavior changes
- Preserve the four verdicts: `pass`, `fail`, `skip`, `uncertain`
- Do not let a task spec rely on implicit test-name matching when explicit selectors are required

## Escalation

Switch to the `agent-spec-tool-first` skill after the contract is drafted and the task moves into implementation and verification.
