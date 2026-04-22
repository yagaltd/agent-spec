# agent-spec

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **This is a fork of [ZhangHanDong/agent-spec](https://github.com/ZhangHanDong/agent-spec)** maintained by [yagaltd](https://github.com/yagaltd). It includes `tdd-guard` integration, `plan-check` validation, and pi-workflows compatibility.

`agent-spec` is an AI-native BDD/spec verification tool for task execution.

The core idea is simple:

- humans review the contract
- agents implement against the contract
- the machine verifies whether the code satisfies the contract

The primary planning surface is the **Task Contract**. The older `brief` view remains available as a compatibility alias, but new workflows should use `contract`.

## Task Contract

A task contract is a structured spec with four core parts:

- `Intent`: what to do, and why
- `Decisions`: technical choices that are already fixed
- `Boundaries`: what may change, and what must not change
- `Completion Criteria`: BDD scenarios that define deterministic pass/fail behavior

The DSL supports English and Chinese headings and step keywords.

## Example

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
```

Chinese authoring is also supported:

```spec
## 意图
## 已定决策
## 边界
## 完成条件

场景: 全额退款保持现有返回结构
  测试: test_refund_service_keeps_existing_success_payload
  假设 存在一笔金额为 "100.00" 元的已完成交易 "TXN-001"
  当 用户对 "TXN-001" 发起全额退款
  那么 响应状态码为 202
```

## Workflow

### 1. Author a task contract

Start from a template:

```bash
cargo run -q --bin agent-spec -- init --level task --lang en --name "User Registration API"
```

For rewrite/parity tasks, start from the parity-aware task template:

```bash
cargo run -q --bin agent-spec -- init --level task --template rewrite-parity --lang en --name "CLI Parity Contract"
```

Or study the examples in [`examples/`](examples).

### AI Agent Skills

This repo ships three agent skills under [`skills/`](skills):

- **`agent-spec-tool-first`**: the default integration path — tells the agent to use `agent-spec` as a CLI tool and drive tasks through `contract`, `lifecycle`, and `guard`.
- **`agent-spec-authoring`**: the authoring path — helps write or revise Task Contracts in the DSL.
- **`agent-spec-estimate`**: the estimation path — maps Task Contract elements (scenarios, decisions, boundaries) to round-based effort estimates.

For rewrite/parity work, the authoring path should explicitly bind observable behavior before coding:

- command x output mode
- local x remote
- warm cache x cold start
- success x partial failure x hard failure

See [`examples/rewrite-parity-contract.spec`](examples/rewrite-parity-contract.spec) for a concrete parity-oriented contract.

#### Install the CLI

```bash
cargo install --git https://github.com/yagaltd/agent-spec
```

#### One-line install (CLI + Claude Code skills)

```bash
./install-skills.sh
```

This installs the `agent-spec` CLI via `cargo install` (if not already present) and copies all three skills to `~/.claude/skills/`.

#### Manual install for Claude Code

```bash
# Copy to your global skills directory
cp -r skills/agent-spec-tool-first ~/.claude/skills/
cp -r skills/agent-spec-authoring ~/.claude/skills/
cp -r skills/agent-spec-estimate ~/.claude/skills/
```

Or symlink for auto-updates:

```bash
ln -s "$(pwd)/skills/agent-spec-tool-first" ~/.claude/skills/
ln -s "$(pwd)/skills/agent-spec-authoring" ~/.claude/skills/
ln -s "$(pwd)/skills/agent-spec-estimate" ~/.claude/skills/
```

#### Install for pi-workflows

agent-spec is a required dependency of [pi-workflows](https://github.com/yagaltd/pi-workflows). Install:

```bash
cargo install --git https://github.com/yagaltd/agent-spec
```

pi-workflows uses `agent-spec lifecycle` as the contract gate and `agent-spec guard` for repo-level boundary checks. The `--layers lint,boundary,test,tdd-guard` flag enables the full verification pipeline including test quality enforcement.

#### Install for Codex

The equivalent guidance for Codex lives in [`AGENTS.md`](AGENTS.md). Copy it to your project root:

```bash
cp AGENTS.md /path/to/your/project/
```

#### Install for Cursor

Copy [`.cursorrules`](.cursorrules) to your project root.

#### Workflow

1. Use `agent-spec-tool-first` to inspect the target spec and render `agent-spec contract`.
2. Run `agent-spec plan <spec> --code . --format prompt` to generate a self-contained implementation prompt with codebase context.
3. Implement code against the Contract + Plan.
4. Run `agent-spec lifecycle` for the task-level gate.
5. Run `agent-spec guard` for repo-level validation when needed.

Before step 2, if the task is a rewrite, migration, or parity effort, use the tool-first workflow to review which observable behaviors are still unbound. If stdout/stderr, `--json`, `-o/--output`, local/remote, cache state, or fallback order are only described in prose, go back to authoring mode and add scenarios first.

This keeps the main integration mode tool-first. Library embedding remains available for advanced Rust-host integration, but it is not the default path.

### 2. Render the contract for agent execution

```bash
cargo run -q --bin agent-spec -- contract specs/my-task.spec
```

Use `--format json` if another tool or agent runtime needs structured output.

### 2b. Generate plan context (Contract + Codebase + Task Sketch)

```bash
cargo run -q --bin agent-spec -- plan specs/my-task.spec --code .
```

`plan` outputs three blocks:

- **Contract** — the full task contract with inherited constraints
- **Codebase Context** — files in Allowed Changes paths with summaries, pub API signatures, and existing test functions
- **Task Sketch** — scenarios grouped by dependency order (topological sort) for implementation sequencing

Use `--format prompt` for a self-contained AI prompt (includes mandatory verification gate and execution protocol). Use `--format json` for machine-parseable output. Use `--depth full` to include pub API signatures in the codebase scan.

### 3. Run the full quality gate

```bash
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --format json
```

`lifecycle` runs:

- lint
- verification
- reporting

By default, verification includes the built-in structural, boundary, test, `tdd-guard`, Bombadil, AI-stub, and complexity layers. Optional external tools are skipped when they are not installed or when no matching scenario exists.

The run fails if:

- lint emits an `error`
- any scenario fails
- any scenario is still `skip` or `uncertain`
- the quality score is below `--min-score`

### 4. Use the repo-level guard

```bash
cargo run -q --bin agent-spec -- guard --spec-dir specs --code .
```

`guard` is intended for pre-commit / CI use. It lints all specs in `specs/` and verifies them against the current change set.

### 5. Validate a pi-workflows plan

`plan-check` validates a generated `plan.md` before execution:

```bash
cargo run -q --bin agent-spec -- plan-check plan.md
cargo run -q --bin agent-spec -- plan-check plan.md --format json
```

It checks that dependency blocks are parseable, dependency targets exist, circular dependencies are absent, bottleneck tags are valid, and contract status is internally consistent. This is useful as a fast gate before a workflow runtime starts dispatching agents.

### 6. Contract Acceptance (replaces Code Review)

```bash
cargo run -q --bin agent-spec -- explain specs/my-task.spec --code . --format markdown
```

`explain` renders a reviewer-friendly summary of the Contract + verification results. Use `--format markdown` for direct PR description paste. Use `--history` to include retry trajectory from run logs.

The reviewer judges two questions: (1) Is the Contract definition correct? (2) Did all verifications pass?

### 7. Stamp for traceability

```bash
cargo run -q --bin agent-spec -- stamp specs/my-task.spec --code . --dry-run
```

Outputs git trailers (`Spec-Name`, `Spec-Passing`, `Spec-Summary`) for the commit message. Currently only `--dry-run` is supported.

## Explicit Test Binding

Task-level scenarios should declare an explicit `Test:` / `测试:` selector.

```spec
Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
```

If package scoping matters, use the structured selector block:

```spec
Scenario: Duplicate email is rejected
  Test:
    Package: user-service
    Filter: test_register_api_rejects_duplicate_email
```

```spec
场景: 超限退款返回稳定错误码
  测试:
    包: refund-service
    过滤: test_refund_service_rejects_refund_exceeding_original_amount
```

This is the default quality rule for self-hosting and new task specs. The older `// @spec:` source annotation is still accepted as a compatibility fallback, but it should not be the primary authoring path.

## Property And External Verification

`agent-spec` can strengthen task contracts with optional property-test and external-verifier layers.

### Property-test linting

The linter warns when a scenario looks like a property-based test but the selector and step text do not bind it to a property framework. This catches contracts that say "for all inputs" but only point at example tests.

Good property-oriented scenarios should make the property binding visible:

```spec
Scenario: Sorting is idempotent for generated arrays
  Test: test_sort_idempotent_property
  Given arbitrary arrays generated with proptest
  When the array is sorted twice
  Then the second sort returns the same sequence
```

### `tdd-guard` layer

When `tdd-guard` is installed, lifecycle verification runs it as an external test-quality layer. It reports scenario-level failures when lint violations are related to the scenario's test selector.

Use it explicitly with a layer selection:

```bash
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --layers lint,boundary,test,tdd-guard
```

If `tdd-guard` is not installed, the layer returns skipped results instead of failing the whole command. This keeps `agent-spec` usable as a standalone tool while allowing stricter hosts, such as pi-workflows, to install and require `tdd-guard`.

### Bombadil layer

When `bombadil` is installed, scenarios tagged `bombadil`, `web-ui`, or `webui` can be verified through Bombadil property tests:

```spec
Scenario: Login form preserves validation messages
  tags: [web-ui, bombadil]
  Test: login_form_validation_property
  Given generated invalid login inputs
  When the form validates the input
  Then visible validation messages match the contract
```

Run it with:

```bash
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --layers lint,boundary,test,bombadil
```

Non-Bombadil scenarios are skipped by that layer. The normal test and boundary layers still cover ordinary task scenarios.

## Boundaries And Change Sets

`Boundaries` can contain both natural-language constraints and path constraints. Path-like entries are mechanically enforced against a change set.

Examples:

```spec
## Boundaries

### Allowed Changes
- crates/spec-parser/**
- crates/spec-gateway/src/lifecycle.rs

### Forbidden
- tests/golden/**
- docs/archive/**
```

The relevant commands accept repeatable `--change` flags:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --change crates/spec-parser/src/parser.rs
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change crates/spec-parser/src/parser.rs
```

Single-task commands also support optional VCS-backed change discovery:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --change-scope staged
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change-scope worktree
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --change-scope jj
```

Available scopes: `none` (default for verify/lifecycle), `staged`, `worktree`, `jj`.

When a `.jj/` directory is detected (even colocated with `.git/`), use `--change-scope jj` to discover changes via `jj diff --name-only`. The `stamp` command also outputs a `Spec-Change:` trailer with the jj change ID, and `explain --history` shows file-level diffs between adjacent runs via jj operation IDs.

## AI Verifier Skeleton

`agent-spec` now includes a minimal AI verifier surface intended to make `uncertain` results explicit and inspectable before a real model backend is wired in.

The relevant commands accept:

```bash
cargo run -q --bin agent-spec -- verify specs/my-task.spec --code . --ai-mode stub
cargo run -q --bin agent-spec -- lifecycle specs/my-task.spec --code . --ai-mode stub
```

Available modes:

- `off`: default, preserves the current mechanical-verifier-only behavior
- `stub`: turns otherwise-uncovered scenarios into `uncertain` results with `AiAnalysis` evidence
- `caller`: the calling Agent acts as the AI verifier (two-step protocol)

`caller` mode enables the Agent running `agent-spec` to also serve as the AI verifier. When `lifecycle --ai-mode caller` finds skipped scenarios, it writes `AiRequest` objects to `.agent-spec/pending-ai-requests.json`. The Agent reads the requests, analyzes each scenario, writes `ScenarioAiDecision` JSON, then calls `resolve-ai --decisions <file>` to merge decisions back into the report.

`stub` mode does not claim success. It is only a scaffold for:

- explicit `uncertain` semantics
- structured AI evidence in reports
- future integration of a real model-backed verifier

Internally, the AI layer now uses a pluggable backend shape:

- `AiRequest`: structured verifier input
- `AiDecision`: structured verifier output
- `AiBackend`: provider abstraction used by `AiVerifier`
- `StubAiBackend`: built-in backend for deterministic local behavior

No real model provider is wired in yet. The current value is that the contract/reporting surface is now stable enough to add a real backend later without redesigning the verification pipeline.

Provider selection and configuration are intentionally out of scope for `agent-spec` itself. The intended embedding model is:

- the host agent owns provider/model/auth/timeout policy
- the host agent injects an `AiBackend` into `spec-gateway`
- `agent-spec` stays focused on contracts, evidence, and verification semantics

`guard` resolves change paths in this order:

1. explicit `--change` arguments
2. auto-detected git changes according to `--change-scope`, if the current workspace is inside a git repo
3. an empty change set, if no git repo is available

`guard` defaults to `--change-scope staged`, which keeps pre-commit behavior stable.

If you want stronger boundary checks against the full current workspace, use:

```bash
cargo run -q --bin agent-spec -- guard --spec-dir specs --code . --change-scope worktree
```

`worktree` includes:

- staged files
- unstaged tracked changes
- untracked files

This makes `guard` practical for both pre-commit usage and broader local worktree validation without forcing users to enumerate changed files manually.

For consistency, `verify` and `lifecycle` use the same precedence when `--change-scope` is provided. The practical default is:

- `verify`: `none`
- `lifecycle`: `none`
- `guard`: `staged`

## Commands

| Command | Purpose |
|---------|---------|
| `parse` | Parse `.spec`/`.spec.md` files and show the AST |
| `lint` | Analyze spec quality (vague verbs, missing test selectors, coverage gaps) |
| `verify` | Verify code against a single spec |
| `contract` | Render the Task Contract view |
| `plan` | Generate plan context: Contract + Codebase scan + Task Sketch |
| `plan-check` | Validate generated plan.md structure before workflow execution |
| `lifecycle` | Run lint + verify + report (the main quality gate) |
| `guard` | Lint all specs and verify against the current change set |
| `explain` | Generate a human-readable contract review summary (Contract Acceptance) |
| `stamp` | Preview git trailers for a verified contract (`--dry-run`) |
| `resolve-ai` | Merge external AI decisions into a verification report (caller mode) |
| `checkpoint` | Preview VCS-aware checkpoint status |
| `graph` | Generate spec dependency graph (`--format dot` or `svg`) |
| `install-hooks` | Install git hooks for automatic checking |
| `measure-determinism` | [experimental] Measure contract verification variance |
| `brief` | Compatibility alias for `contract` |

## Examples

See [`examples/`](examples):

- [`examples/user-registration-contract.spec`](examples/user-registration-contract.spec)
- [`examples/refactor-payment-service.spec`](examples/refactor-payment-service.spec)
- [`examples/refund.spec`](examples/refund.spec)
- [`examples/no-unwrap.spec`](examples/no-unwrap.spec)

## Current Status

The current system is strongest when the contract can be checked by:

- explicit tests selected from `Completion Criteria`
- structural checks
- boundary checks against an explicit or staged change set
- optional property-test, `tdd-guard`, Bombadil, AI, and complexity verifier layers when the host project enables them

More advanced verifier layers can still be added, but the current model is already sufficient for self-hosting `agent-spec` with task contracts.

## Contributing

agent-spec is self-bootstrapping: the project uses itself to govern its own development. When you contribute, you follow the same Contract-driven workflow that agent-spec teaches.

### The contribution flow

Every change starts with a Task Contract. Before writing code, create a `.spec.md` file in `specs/` that defines what you're building — the intent, the technical decisions that are already fixed, the files you'll touch, and the BDD scenarios that define "done." Then implement against the Contract and verify with `lifecycle`. (Legacy `.spec` files are also supported.)

```bash
# 1. Create a task contract for your change
agent-spec init --level task --lang en --name "my-feature"
# Edit the generated spec: fill in Intent, Decisions, Boundaries, Completion Criteria

# 2. Check that the contract itself is well-written
agent-spec lint specs/my-feature.spec.md --min-score 0.7

# 3. Implement your change

# 4. Verify against the contract
agent-spec lifecycle specs/my-feature.spec.md --code . --change-scope worktree --format json

# 5. Run the repo-wide guard before committing
agent-spec guard --spec-dir specs --code .

# 6. Generate the PR description
agent-spec explain specs/my-feature.spec.md --code . --format markdown
```

The `guard` pre-commit hook is installed via `agent-spec install-hooks`. It checks all specs in `specs/` against your staged changes — your commit will be blocked if any contract fails.

### Project-level rules

The file `specs/project.spec` defines constraints that every task spec inherits. Read it before writing your first Contract — it tells you what the project enforces globally (e.g. "all public CLI behavior must have regression tests," "verification results must distinguish pass/fail/skip/uncertain").

### Roadmap specs

Future work lives in `specs/roadmap/`. These are real Task Contracts but they are not checked by the default `guard` run. When a roadmap spec is ready for implementation, promote it to the top-level `specs/` directory. See `specs/roadmap/README.md` for the promotion rule.

### Using AI agents to contribute

If you use Claude Code, Codex, Cursor, or another AI coding agent, install the skills from the [`skills/`](skills) directory (see [AI Agent Skills](#ai-agent-skills) above).

The `agent-spec-tool-first` skill tells the agent to read the Contract first, implement within its Boundaries, run `lifecycle` to verify, and retry on failure without modifying the spec. The `agent-spec-authoring` skill helps the agent draft or revise Task Contracts in the DSL. The `agent-spec-estimate` skill maps Contract elements to round-based effort estimates for sprint planning.

For agents without skill support, the project includes `AGENTS.md` (Codex), `.cursorrules` (Cursor), and `.aider.conf.yml` (Aider) with the essential command reference.

### What we review

Pull requests are evaluated through Contract Acceptance, not line-by-line code review. The reviewer checks two things: is the Contract definition correct (does it capture the right intent and edge cases), and did all verifications pass (lifecycle reports all-green). If both are yes, the PR is approved.

This means the quality of your Contract matters as much as the quality of your code. A well-written Contract with thorough exception-path scenarios is a stronger contribution than clever code with a thin spec.
