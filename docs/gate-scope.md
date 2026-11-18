# Gate scope — what guard scans, and why roadmap specs sit outside it

Decided: 2026-11-17 · pi-workflows plan `20261117-069`, task 1 · pinned by
`tests/multi_file_lint.rs` (`test_gate_scope_documented`).

## The decision

`agent-spec guard` scans the **top level of specs/** only. That top level is
the gate scope: the set of active contracts that pre-commit / CI runs must
pass before code lands. `specs/roadmap/` is **outside gate scope**.

Concretely (see `cmd_guard` in `src/main.rs`): guard reads `spec_dir` with a
single non-recursive `read_dir` and filters spec files — no directory
recursion. `specs/roadmap/` is a subdirectory, so its contents are never
picked up by a plain `guard --spec-dir specs` run.

## Rationale

Roadmap specs are **staged future work**, not active contracts. Gating the
present against specs whose scenarios have not executed yet would either
fail the gate forever or force roadmap prose to satisfy rules written for
executed contracts (see the related frozen-specs carve-out in
[docs/frozen-specs.md](frozen-specs.md): guard/lifecycle rules apply to new
and revised specs — frozen specs are historical record, and roadmap specs
are not-yet-live record).

They are not unsupervised, though: `agent-spec lint` covers them when the
files are **passed explicitly** —
`agent-spec lint specs/roadmap/*.spec.md` lints each one and, as of the
per-file continuation fix (v0.3.3), names *every* unparseable file with its
reason instead of stopping at the first one. Roadmap promotion
(`specs/task-stage-roadmap-specs.spec.md`) is the moment a spec crosses
into gate scope: it moves to the top level of `specs/` and from then on
guard runs it on every change.

## Why guard stays non-recursive

This is a deliberate scope boundary, not a missing feature: recursion would
put staged work into the CI gate with no opt-out. If a roadmap directory
ever needs gate coverage, the mechanism already exists — move the spec to
the top level (promotion), or pass the files to `lint`/`lifecycle`
explicitly.

## The evidence run ignores target-repo fixtures

Decided: 2026-11-17 · pi-workflows plan `20261117-070`, task 1 · pinned by
`spec_verify::bun_test_verifier::tests::test_guard_runner_excludes_fixtures`.

When guard/lifecycle collects test evidence, it invokes the target repo's
test runner — for Bun projects, `bun test` (resolved from the package.json
`test` script when it starts with `bun`). That invocation **ignores paths
matching `**/fixtures/**` by default**: bun runs with
`--path-ignore-patterns **/fixtures/**`, so fixture directories are never
loaded, let alone executed.

The reason is evidence hygiene, not gate leniency: fixture directories hold
static test data and fixture copies of real tests, and sweeping them made
guard's evidence stdout fill with pre-existing fixture failures from the
target repo (a fixture copy of a named test fails on every run and its
`(fail)` line lands in the collected evidence next to the real verdict).
Exit semantics are untouched — fail-closed stays fail-closed; the named
test still decides the scenario verdict exactly as before. The exclusion is
appended in `bun_evidence_command` (`src/spec_verify/bun_test_verifier.rs`)
so every caller in the verify pipeline gets the same default.
