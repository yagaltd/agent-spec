# Commands

## Core flow

```bash
cargo run -q --bin agent-spec -- contract specs/task.spec
cargo run -q --bin agent-spec -- lifecycle specs/task.spec --code .
cargo run -q --bin agent-spec -- guard --spec-dir specs --code .
```

## Change sets

Use explicit paths when the user already knows the relevant change set:

```bash
cargo run -q --bin agent-spec -- lifecycle specs/task.spec --code . --change crates/spec-parser/src/parser.rs
```

Use git-backed change discovery when the user wants boundary checking against repo state:

```bash
cargo run -q --bin agent-spec -- verify specs/task.spec --code . --change-scope staged
cargo run -q --bin agent-spec -- lifecycle specs/task.spec --code . --change-scope worktree
cargo run -q --bin agent-spec -- guard --spec-dir specs --code . --change-scope worktree
```

Defaults:

- `verify`: `--change-scope none`
- `lifecycle`: `--change-scope none`
- `guard`: `--change-scope staged`

## AI mode

```bash
cargo run -q --bin agent-spec -- verify specs/task.spec --code . --ai-mode stub
```

Use `stub` only when the user explicitly wants AI-style `uncertain` evidence scaffolding. It is not a passing mode.
