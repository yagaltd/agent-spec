---
name: agent-spec-tool-first
description: Tool-first workflow for projects that use agent-spec Task Contracts. Use when Claude Code needs to inspect `specs/*.spec`, render a task contract with `agent-spec contract`, implement code against the contract, run `agent-spec lifecycle` or `agent-spec guard`, interpret non-passing verification results, or work in a contract-driven code review flow.
---

# Agent Spec Tool First

Use `agent-spec` as a CLI tool first. Prefer the command surface over embedding APIs unless the user is explicitly working on library integration.

## Workflow

1. Find the relevant spec.
   Read `specs/project.spec` first when it exists.
   Then read the target task spec in `specs/`.

2. Render the execution contract.
   Run `agent-spec contract <task.spec>` before coding.
   Treat the rendered Task Contract as the source of truth for implementation boundaries and completion criteria.

3. Implement against the contract.
   Follow `Intent`, `Decisions`, `Boundaries`, and `Completion Criteria`.
   Do not treat prose outside the contract as higher priority than the contract without surfacing the conflict.

4. Verify with the CLI.
   Use `agent-spec lifecycle <task.spec> --code <path>` for the main quality gate.
   Use `agent-spec guard --spec-dir specs --code <path>` when the user wants repo-level checking.

5. Interpret failures correctly.
   `fail`, `skip`, and `uncertain` are all non-passing.
   Read the failure summary and the per-scenario evidence before changing code.

## Command Selection

- Use `agent-spec contract` to prepare execution context for the task.
- Use `agent-spec lifecycle` for task-level verification after edits.
- Use `agent-spec guard` for repo-wide validation or pre-commit style checks.
- Use `agent-spec verify` only when the user explicitly wants raw verification output without the full lifecycle gate.
- Use `agent-spec lint` when the issue is about spec quality rather than code behavior.

Read [references/commands.md](references/commands.md) when you need concrete command patterns or need to choose between `--change`, `--change-scope`, or `--ai-mode`.

## Guardrails

- Prefer CLI commands over calling `spec-gateway` directly.
- Prefer `contract` over the legacy `brief` alias.
- Treat `guard` as `tool-first` default for repo-wide checks.
- Use `--change-scope worktree` only when the user wants workspace-wide boundary checking; otherwise preserve the staged/default behavior.
- Use `--ai-mode stub` only when the user explicitly wants `uncertain` AI evidence scaffolding.

## Escalation

Switch to library integration only when the task is about embedding `agent-spec` into another Rust agent runtime, testing `spec-gateway`, or injecting a host AI backend. In those cases, read the repo code directly rather than forcing the CLI path.
