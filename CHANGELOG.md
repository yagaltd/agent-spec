# Changelog

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
