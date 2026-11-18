//! Pins for multi-file lint continuation (pi-workflows plan 20261117-069,
//! task 1).
//!
//! The defect: `lint a b c` aborted on the first file that failed to parse
//! (`?` propagation), so files after the bad one were silently unprocessed —
//! a combined run under-reported, which poisoned a durable count in plan-067.
//! The fix: per-file result collection — every processable file is reported,
//! every unparseable file is named with its reason, and the run exits
//! non-zero at the end if anything failed.
//!
//! Every pin drives the real CLI binary (end-to-end contract at the process
//! boundary, same shape as `tests/cache.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_agent-spec");

// ── fixture plumbing ────────────────────────────────────────────

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "agent-spec-multi-lint-{tag}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
}

/// A minimal, lint-clean task spec with a distinct frontmatter name.
fn valid_spec(name: &str) -> String {
    format!(
        r#"spec: task
name: "{name}"
tags: [lint]

---

## Intent

A valid spec used to pin multi-file lint continuation.

## Completion Criteria

Scenario: it works
  Test: test_{name}_works
  Given the fixture spec
  When lint runs over it
  Then a full report is emitted
"#
    )
}

fn run_lint(files: &[&Path], extra: &[&str]) -> Output {
    Command::new(BIN)
        .arg("lint")
        .args(files)
        .args(extra)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

// ── pins ────────────────────────────────────────────────────────

#[test]
fn test_multi_file_lint_continues_past_parse_failures() {
    let dir = temp_root("continue");
    let good_a = dir.join("valid_a.spec.md");
    let bad = dir.join("no-frontmatter.md");
    let good_b = dir.join("valid_b.spec.md");
    write_file(&good_a, &valid_spec("alpha"));
    write_file(&bad, "just prose, no front-matter separator\n");
    write_file(&good_b, &valid_spec("beta"));

    let out = run_lint(&[&good_a, &bad, &good_b], &[]);

    // Both valid files received full reports — the run did not stop at the
    // unparseable file in the middle.
    let out_text = stdout(&out);
    assert!(
        out_text.contains("Spec: alpha"),
        "first valid file must still get its report, stdout:\n{out_text}"
    );
    assert!(
        out_text.contains("Spec: beta"),
        "valid file AFTER the bad one must still get its report, stdout:\n{out_text}"
    );

    // The bad file is named, not silently skipped.
    let err_text = stderr(&out);
    assert!(
        err_text.contains("no-frontmatter.md"),
        "the unparseable file must be named, stderr:\n{err_text}"
    );

    // Per the fixed contract: any parse failure fails the run at the end
    // (non-zero exit), even though all processable files were reported.
    assert!(
        !out.status.success(),
        "a run with a parse failure must exit non-zero (after reporting every good file)"
    );

    // Extend-don't-break: the JSON format still emits one report per good
    // file instead of aborting at the bad one.
    let out_json = run_lint(&[&good_a, &bad, &good_b], &["--format", "json"]);
    let json_text = stdout(&out_json);
    assert!(
        json_text.contains("\"alpha\"") && json_text.contains("\"beta\""),
        "json format must report both good files, stdout:\n{json_text}"
    );
}

#[test]
fn test_multi_file_lint_names_all_bad_files() {
    let dir = temp_root("all-bad");
    let good = dir.join("valid.spec.md");
    let bad_one = dir.join("bad-no-separator.md");
    let bad_two = dir.join("bad-level.spec.md");
    write_file(&good, &valid_spec("gamma"));
    write_file(&bad_one, "no separator here either\n");
    // Distinct parse failure: unknown spec level in frontmatter.
    write_file(
        &bad_two,
        "spec: bogus\nname: \"broken\"\n---\n\n## Intent\n\nbroken meta\n",
    );

    let out = run_lint(&[&bad_one, &good, &bad_two], &[]);

    assert!(
        !out.status.success(),
        "a run with unparseable files must exit non-zero"
    );

    let err_text = stderr(&out);
    assert!(
        err_text.contains("bad-no-separator.md"),
        "first bad file must be named, stderr:\n{err_text}"
    );
    assert!(
        err_text.contains("missing front-matter separator"),
        "first bad file's reason must be given, stderr:\n{err_text}"
    );
    assert!(
        err_text.contains("bad-level.spec.md"),
        "second bad file must ALSO be named (no first-error abort), stderr:\n{err_text}"
    );
    assert!(
        err_text.contains("unknown spec level: bogus"),
        "second bad file's reason must be given, stderr:\n{err_text}"
    );

    // The good file sandwiched between the bad ones still gets its report.
    let out_text = stdout(&out);
    assert!(
        out_text.contains("Spec: gamma"),
        "valid file must still be reported, stdout:\n{out_text}"
    );
}

#[test]
fn test_gate_scope_documented() {
    let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/gate-scope.md");
    let doc = fs::read_to_string(&doc_path).unwrap_or_else(|e| {
        panic!("docs/gate-scope.md must exist (gate-scope decision is pinned by this test): {e}")
    });

    // Guard scope: the top level of specs/ only (active contracts).
    assert!(
        doc.contains("top level of specs/"),
        "the doc must state the guard's top-level scope"
    );

    // Roadmap exclusion, stated explicitly.
    assert!(
        doc.contains("specs/roadmap/"),
        "the doc must name the roadmap directory as outside the gate scope"
    );
    assert!(
        doc.contains("outside gate scope"),
        "the doc must state the exclusion in so many words"
    );

    // Rationale: roadmap specs are staged future work; lint still covers them
    // when passed explicitly.
    assert!(
        doc.contains("staged future work"),
        "the doc must give the rationale (staged future work)"
    );
    assert!(
        doc.contains("passed explicitly"),
        "the doc must state that explicit per-file lint enumeration still covers roadmap specs"
    );

    // Cross-link: the frozen-specs doc is the related decision record.
    assert!(
        doc.contains("frozen-specs.md"),
        "the doc must cross-link docs/frozen-specs.md"
    );
}
