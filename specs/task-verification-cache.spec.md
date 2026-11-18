spec: task
name: "Verification cache (spike)"
tags: [performance, cache, spike]
max-rounds: 2
frozen: 20261117 plan-20261117-068
---

## Intent

Repeated verifications redo identical work: `agent-spec guard --spec-dir specs
--code .` over this repo's 27 specs takes 35.8-36.9 s three runs in a row on an
unchanged tree, and the dominant cost is the test layer spawning `cargo test
<filter>` once per scenario (measured: 216 filtered out per spawn, ~168 ms).
A cross-run memo of the verification report, keyed on the exact inputs, makes
an unchanged rerun near-instant without weakening the gate.

## Decisions

- Key = (spec bytes, resolved layer set, `--code` path, git state of the code
  tree, agent-spec version). Git state = HEAD commit + `git status --porcelain`
  + a content hash of every dirty path. No new dependency: hash with an inline
  FNV-1a over bytes; serialize the report with the existing serde_json.
- Cache location = `<code>/.agent-spec-cache/`, gitignored, one JSON file per
  key. Opt-in flag `--cache` on `lifecycle` (and therefore on `guard`).
- A hit MUST be byte-identical to the cold report for the same inputs; a hit
  also records `"cache": "hit"` in the report so provenance is never silent.
- A miss recomputes and writes; a write failure degrades to a miss with a loud
  note, never a silent pass.
- Invalidation is the whole point: spec edit, covered-code edit, test edit and
  layer-set change must all miss. Stale hits are the failure mode this spike
  exists to rule out.

## Boundaries

### Allowed Changes
- src/spec_gateway/cache.rs (new)
- src/spec_gateway/mod.rs
- src/spec_gateway/lifecycle.rs
- src/main.rs
- tests/cache.rs (new)
- .gitignore

### Forbidden
- Any new crate dependency (no sha2, no fs2, no tempfile)
- Weakening or skipping an existing verification layer
- Changing report content on a cold run (byte-identical output required)
- Making `--cache` the default in this spike

## Completion Criteria

Scenario: an unchanged tree hits the cache and returns the identical report
  Test: test_cache_hit_is_byte_identical_to_cold
  Given a lifecycle run with `--cache` that populated the cache
  When the same command runs again on the unchanged tree
  Then the second report is byte-identical except for the cache field
  And the second run is materially faster than the first

Scenario: editing the spec invalidates the entry
  Test: test_spec_edit_misses_cache
  Given a populated cache entry
  When one byte of the spec file changes
  Then the next run is a miss and recomputes

Scenario: editing covered code invalidates the entry
  Test: test_code_edit_misses_cache
  Given a populated cache entry
  When a dirty path under the code root changes
  Then the next run is a miss and recomputes

Scenario: editing a test invalidates the entry
  Test: test_test_file_edit_misses_cache
  Given a populated cache entry
  When a test file named by a `Test:` selector changes
  Then the next run is a miss and recomputes

Scenario: a different layer set never reuses another's entry
  Test: test_layer_set_is_part_of_key
  Given a populated cache entry for one `--layers` value
  When the same spec runs with a different `--layers` value
  Then it is a miss

Scenario: an unwritable cache degrades loudly, never silently passing
  Test: test_unwritable_cache_is_a_loud_miss
  Given a cache directory that cannot be written
  When a run with `--cache` executes
  Then the run still verifies, reports the write failure as a note
  And the verdict is never improved by the failure

Scenario: the guard workload improves end to end
  Test: test_guard_workload_speedup
  Given 27 specs under `specs/`
  When `guard --cache` runs twice
  Then the second run is at least 5x faster
  And both runs report the same pass/fail counts
