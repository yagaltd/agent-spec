# agent-spec

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **Fork of [ZhangHanDong/agent-spec](https://github.com/ZhangHanDong/agent-spec)** with `tdd-guard` integration, `plan-check` validation, and pi-workflows compatibility.

AI-native BDD/spec verification tool. You write the contract, the agent implements against it, the machine verifies the result.

## Install

```bash
cargo install --git https://github.com/yagaltd/agent-spec
```

Required by [pi-workflows](https://github.com/yagaltd/pi-workflows) as the contract verification gate.

Optional companion tools:
- [tdd-guard](https://github.com/yagaltd/tdd-guard) — test quality enforcement (recommended)
- [bombadil](https://github.com/antithesishq/bombadil) — property-based web UI testing

## Quick Start

```bash
# 1. Create a contract
agent-spec init --level task --lang en --name "User Registration API"
# Edit specs/user-registration-api.spec: fill in Intent, Decisions, Boundaries, Completion Criteria

# 2. Verify implementation against the contract
agent-spec lifecycle specs/user-registration-api.spec --code .

# 3. Check all contracts against current changes
agent-spec guard --spec-dir specs --code .
```

## Task Contract

A contract has four parts:

```spec
spec: task
name: "User Registration API"
tags: [api, contract]
---

## Intent

Implement a deterministic user registration API contract that an agent can code against
and a verifier can check with explicit test selectors.

## Decisions

- Use `POST /api/v1/users/register` as the only public entrypoint
- Persist a new user only after password hashing succeeds

## Boundaries

### Allowed Changes
- crates/api/**
- tests/integration/register_api.rs

### Forbidden
- Do not change the existing login endpoint contract
- Do not create a session during registration

## Completion Criteria

Scenario: Successful registration
  Test: test_register_api_returns_201_for_new_user
  Given no user with email "alice@example.com" exists
  When client submits the registration request:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  Then response status should be 201
  And response body should contain "user_id"

Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
  Given a user with email "alice@example.com" already exists
  When client submits a registration request with the same email
  Then response status should be 409
```

- **Intent**: what to do and why
- **Decisions**: technical choices already fixed
- **Boundaries**: which files may change, which must not
- **Completion Criteria**: BDD scenarios with explicit `Test:` selectors

Chinese headings are also supported (`## 意图`, `## 已定决策`, `## 边界`, `## 完成条件`, `场景:`, `测试:`).

## Commands

| Command | Purpose |
|---------|---------|
| `init` | Create a contract from template |
| `contract` | Render the contract for agent execution |
| `plan` | Generate plan context: contract + codebase scan + task sketch |
| `plan-check` | Validate `plan.md` structure (dependencies, bottleneck tags, contract status) |
| `lifecycle` | Run the full quality gate: lint + verify + report |
| `guard` | Verify all specs against current changes (pre-commit / CI) |
| `explain` | Generate human-readable contract review summary |

Use `--format json` on any command for machine-parseable output.

## Verification Layers

`lifecycle` runs multiple verification layers. Built-in layers always run. External layers activate when their tool is installed.

### Built-in layers

- **lint** — spec quality (vague verbs, missing test selectors, coverage gaps)
- **boundary** — verify changes stay within allowed paths
- **test** — verify test selectors exist and scenarios pass
- **complexity** — flag overly complex scenarios

### External layers

**tdd-guard** — test quality enforcement:

```bash
# Install tdd-guard, then lifecycle runs it automatically
agent-spec lifecycle specs/my-task.spec --code . --layers lint,boundary,test,tdd-guard
```

Checks: no skipped tests, no assertionless tests, no internal mocks, public interface only. See [yagaltd/tdd-guard](https://github.com/yagaltd/tdd-guard).

**bombadil** — property-based web UI testing:

```spec
Scenario: Login form preserves validation messages
  tags: [web-ui, bombadil]
  Test: login_form_validation_property
  Given generated invalid login inputs
  When the form validates the input
  Then visible validation messages match the contract
```

See [antithesishq/bombadil](https://github.com/antithesishq/bombadil).

If an external tool is not installed, its layer returns `skipped` — the command still succeeds. This keeps `agent-spec` usable standalone.

## Boundaries and Change Sets

Boundaries enforce which files may change. Path-like entries are mechanically checked:

```bash
# Verify against specific changed files
agent-spec lifecycle specs/my-task.spec --code . --change src/parser.rs

# Auto-detect changes from git
agent-spec lifecycle specs/my-task.spec --code . --change-scope worktree
agent-spec guard --spec-dir specs --code . --change-scope staged
```

Available scopes: `none` (default), `staged`, `worktree`, `jj`.

## Plan Validation

`plan-check` validates a pi-workflows `plan.md` before execution:

```bash
agent-spec plan-check plan.md
```

Checks: dependency blocks parseable, dependency targets exist, no circular dependencies, bottleneck tags valid, contract status consistent.

## Contract Acceptance

Replace line-by-line code review with contract acceptance:

```bash
agent-spec explain specs/my-task.spec --code . --format markdown
```

The reviewer answers two questions: (1) Is the contract correct? (2) Did all verifications pass?

## Explicit Test Binding

Every scenario must declare a `Test:` selector — the exact test function name:

```spec
Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
```

For package-scoped tests:

```spec
Scenario: Duplicate email is rejected
  Test:
    Package: user-service
    Filter: test_register_api_rejects_duplicate_email
```

## Property Tests

For property-based scenarios, make the property binding visible:

```spec
Scenario: Sorting is idempotent for generated arrays
  Test: test_sort_idempotent_property
  Given arbitrary arrays generated with proptest
  When the array is sorted twice
  Then the second sort returns the same sequence
```

## AI Verifier Modes

```bash
agent-spec lifecycle specs/my-task.spec --code . --ai-mode stub
```

| Mode | Behavior |
|------|----------|
| `off` | Default. Mechanical verification only. |
| `stub` | Mark uncovered scenarios as `uncertain` with structured evidence. |
| `caller` | Two-step protocol: agent writes AI decisions, then merges back. |

## Examples

See [`examples/`](examples):
- [`examples/user-registration-contract.spec`](examples/user-registration-contract.spec)
- [`examples/refactor-payment-service.spec`](examples/refactor-payment-service.spec)
- [`examples/refund.spec`](examples/refund.spec)
- [`examples/no-unwrap.spec`](examples/no-unwrap.spec)

## Contributing

See the [upstream contributing guide](https://github.com/ZhangHanDong/agent-spec#contributing). agent-spec is self-bootstrapping — contributions follow the same contract-driven workflow: write a `.spec.md`, implement, verify with `lifecycle`, run `guard` before committing.

## License

MIT
