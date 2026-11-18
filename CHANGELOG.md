## Unreleased

- **feat(plan-check): Sieve sections are validated now** — proposal plans
  now carry a validated forced-choice verdict, and a parked idea can never
  become a plan file. When a plan carries `## Sieve`, plan-check parses
  the Cell|Verdict table (header-driven, pinned 0/1 fallback, trimmed
  cells) plus the `Outcome:` line and checks the enums: V ∈
  nice-to-have/useful/valuable/must-have, R ∈
  trivial/contained/serious/severe, E ∈ xs/s/m/l, path ∈
  research/inspect/grill/none, Outcome ∈ propose/build-direct — all
  case-insensitive. `Outcome: park` (or any other non-enum) is an error
  naming the line and the rule — parked ideas are ledger rows, never
  plans; a malformed, unknown or missing verdict cell, a missing V/R/E/
  path row and a missing Outcome line are errors naming the cell/section
  and line. Plans without a Sieve section are a no-op — rejecting those
  stays forbidden (non-proposal plans are legal). Structure mirrors
  `validate_ledger_resolution`; pinned by the four `plan_check_*sieve*`
  tests in `src/main.rs`.

- **fix(boundaries): bare root filenames in Allowed lists are path
  boundaries now** — an entry like `README.md` was dropped as prose (no
  `/`, no `*`, not a dotfile, suffix not on a hardcoded list), so the
  exact file it named fell outside every allowed boundary and guard
  failed the change set with "not covered by any allowed boundary". The
  matcher rule is now: an Allowed entry is a path boundary iff it
  contains `/` or `*`, starts with a dot (0.3.1 dotfile behavior), or
  its final segment contains a dot — a file extension (README.md,
  package.json, Cargo.toml, Cargo.lock, vitest.config.ts). A bare word
  with no dot in any segment stays prose, extension-bearing entries
  anchor to the EXACT file (never a directory prefix), and the `./`
  prefix keeps working. Exit semantics untouched — fail-closed stays
  fail-closed. Pinned by the five named tests in
  `src/spec_verify/boundaries.rs`.

- **fix(guard): the evidence run no longer sweeps the target repo's
  fixtures** — guard/lifecycle invoked the target repo's test runner
  (`bun test`) as a full sweep, so `tests/fixtures/**` were loaded and
  run: fixture copies of real tests failed on every run and their
  `(fail)` lines polluted the collected evidence stdout with
  pre-existing failures from unrelated repos. The bun evidence run now
  passes `--path-ignore-patterns **/fixtures/**` by default, so fixture
  directories are never loaded. Exit semantics untouched. Documented in
  `docs/gate-scope.md`; pinned by
  `test_guard_runner_excludes_fixtures`.
- **fix(lint): a multi-file run no longer aborts on the first unparseable
  file** — `agent-spec lint a b c` used to propagate the first
  parse-failure with `?`, so files after the bad one were silently
  unprocessed and stderr suppression hid it (a combined run in plan-067
  under-reported and poisoned one durable count). Now every file is
  attempted: good files get their full report in the chosen format
  (text/json/md unchanged), parse failures are collected per file and
  named — path + reason — on stderr after the loop, with a processed/total
  summary; the run exits non-zero at the end if any lint errors, score
  misses, or parse failures occurred. Pinned by
  `tests/multi_file_lint.rs`. The gate-scope decision — guard scans the
  top level of `specs/` (active contracts); `specs/roadmap/` sits outside
  gate scope as staged future work, covered by explicit per-file lint — is
  documented in `docs/gate-scope.md`, cross-linked from
  `docs/frozen-specs.md` and pinned by a docs-reading test.

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
