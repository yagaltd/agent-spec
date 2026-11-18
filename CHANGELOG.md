## Unreleased

- **frozen specs: carve-out decision documented, flagged set marked** — docs
  + test only, no code change (the `boundaries-path` rule itself is
  untouched). 11 historical specs whose scenarios cite example paths outside
  their own Allowed Changes (16 findings, enumerated 2026-11-17) now carry a
  `frozen: 20261117 plan-20261117-068` frontmatter marker: frozen specs are
  a historical record; guard/lifecycle/boundaries-path apply to new and
  revised specs only. Semantics and the full list: `docs/frozen-specs.md`;
  the set is pinned by `tests/frozen_specs.rs`.

## v0.3.2 — boundaries lint + ledger check (2026-11-17)

- **lint(boundaries): criteria paths must resolve inside Allowed Changes** — the
  defect class that cost plan 064 four review rounds: a spec whose criterion
  depends on a path its own Boundaries forbid (e.g. criterion cites `docs/api.md`
  while Allowed lists only `src/**`) used to surface only during review, after the
  contract had already been executed against. The rule joins the existing lint
  pipeline (`spec_lint::LintPipeline::with_defaults`, no new subcommand): backticked
  tokens from Intent, Scenarios and Completion Criteria are path-like when they
  contain `/`, have no `://` scheme and no space; each must resolve against the
  spec's Allowed Changes entries (equal, descendant, or `**`/`*` glob cover).
  Unresolved paths are error-level findings naming the path and the criterion.
  Backticked commands (`cargo test`) and URLs are never flagged; only criteria
  text is scanned — the Allowed lines themselves never are.
- **plan-check validates `## Ledger Resolution`** — when a plan carries the
  section, every row's Disposition cell must be exactly one token:
  `done(<ref>)`, `closed(<reason>)` or `deferred(trigger: <condition>)`
  (whitespace-trimmed; prose after the token is malformed). Missing section is a
  no-op; malformed rows fail plan-check naming the Item cell and line number.

## v0.3.1 — verification cache (opt-in) + boundary fixes (2026-11-17)

- **`--cache` on `lifecycle` / `guard`** — cross-run memo of the whole verification report,
  keyed on spec bytes, layer set, `--ai-mode`, change paths, code root and git state (HEAD +
  `status --porcelain -uall` + a content hash of every dirty path), plus the crate version.
  Inline FNV-1a, no new dependency; entries under `<code>/.agent-spec-cache/` (gitignored).
  Measured on this repo: `guard --spec-dir specs` **57.4 s cold → 0.69 s warm (83x)**,
  28 specs either way; a single `lifecycle` 106-185x. A hit must be byte-identical to the cold
  report except `"cache": "hit"`, and an unwritable or non-git cache degrades to a loud miss —
  never a silent pass. Opt-in: a plain run creates no cache directory.
- **fix(boundaries): dotfiles are boundary paths** — `looks_like_path_boundary` dropped any
  pattern without a separator or a known suffix, so an explicitly allowed `.gitignore` became
  an automatic violation.
- **fix(boundaries): trailing annotations are prose** — `normalize_pattern` kept ` (new)`, so
  `src/x.rs (new)` could never match a change path. Both found by the cache spike verifying its
  own contract; pinned by three tests.

# Changelog

## v0.3.0 — lib/bin split + fuzz-driven correctness fixes (released 2026-11-17)

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
