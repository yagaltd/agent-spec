# Changelog

## Unreleased

- **fix: `parse_trailing_number` multibyte panic** — `parse_trailing_number` sliced at
  `rfind(...) + 1`, which can land inside a multibyte UTF-8 char (e.g. `"中12"`) and
  panic. Found by the new fuzz target `fuzz_tool_output`; fixed by walking char
  boundaries from the end. Regression-pinned in `test_parse_trailing_number_multibyte_never_panics`.
- **fix: `node --test` now requests TAP explicitly** — `--test-reporter=tap
  --test-reporter-destination=stdout` is passed to `node --test`; Node ≥ 22/25 may not
  default to TAP for piped stdout, which made the Node verifier return `skip` instead of
  `pass`/`fail`.
- **fuzz scaffold** — `fuzz/` cargo-fuzz crate (separate workspace, never built by
  `cargo test`): `fuzz_spec_parse`, `fuzz_tool_output`, `fuzz_report_format`. Smoke-run
  60s/target on nightly, all green after the `parse_trailing_number` fix.
- **edge-case pins** — deterministic table test for all `spec_parser::keywords` matchers
  (empty/CRLF/whitespace/mixed-case/CJK/emoji/NUL-lossy/very-long inputs) and a formatter
  totality test (`format_verification`/`format_explain` never panic on a minimal report).
- **clippy**: replaced `unwrap()`s in plan-check regex compilation (deny
  `clippy::unwrap_used`); collapsed-`if`/derivable-`Default`/`ptr_arg` lint fixes;
  `cargo fmt` applied.

## v0.3.0 — lib/bin split

- **Library target added** — the spec parse / lint / verify engine is now
  importable as a Rust crate (`agent-spec` lib), so consumers (e.g.
  CognitiveOS v3) can call the engine directly instead of shelling out to the
  CLI. Public API: `spec_parser` (parse entry points), `spec_lint` (quality
  pipeline), `spec_verify` (Verifier trait, structural/boundary/test/ai layers,
  `run_verification`), `spec_gateway` (`SpecGateway`, `apply_dependency_skips`,
  `topological_sort_scenarios`), `spec_report` (formatters), `spec_core`
  (shared AST, verdicts, reports).
- **CLI unchanged** — the binary is now a thin wrapper; all subcommands keep
  byte-identical behavior. `apply_dependency_skips` and
  `topological_sort_scenarios` moved from the CLI crate root into
  `spec_gateway` (re-exported at `spec_gateway::` level).
- **Tests** — added `tests/api.rs`: public-API integration coverage (parse →
  AST, structural verify verdicts pass/fail, lifecycle orchestration,
  dependency-skip + topological ordering semantics). All prior tests retained.

## v0.2.7 — (previous)
