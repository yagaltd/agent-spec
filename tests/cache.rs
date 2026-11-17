//! Pins for the opt-in verification cache (see
//! `specs/task-verification-cache.spec.md`).
//!
//! Every pin drives the real CLI binary, because the cache is an end-to-end
//! contract: key composition, hit/miss provenance, and cold-vs-warm output
//! identity all live at the process boundary.
//!
//! Fixtures are throwaway trees under the system temp dir. A fixture that must
//! be *cacheable* is a git repo: the key covers the git state of the code tree,
//! and a tree outside git has no state to key on.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_agent-spec");
const CACHE_DIR: &str = ".agent-spec-cache";

// ── fixture plumbing ────────────────────────────────────────────

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "agent-spec-cache-{tag}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed in {}: {}",
        repo.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `git init` + commit everything, with an identity that never touches the
/// user's config and never tries to sign.
fn git_init_and_commit(repo: &Path) {
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(
        repo,
        &[
            "-c",
            "user.email=cache-pin@example.com",
            "-c",
            "user.name=cache-pin",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    );
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().unwrap()
}

fn run_timed(args: &[&str]) -> (Output, Duration) {
    let started = Instant::now();
    let out = run(args);
    (out, started.elapsed())
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn report(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|err| {
        panic!(
            "lifecycle stdout is not JSON ({err})\nstdout:\n{}\nstderr:\n{}",
            stdout(out),
            stderr(out)
        )
    })
}

fn cache_status(out: &Output) -> String {
    let value = report(out);
    value
        .get("cache")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("report has no `cache` field: {value}"))
        .to_owned()
}

fn assert_cache(out: &Output, expected: &str, context: &str) {
    let actual = cache_status(out);
    assert_eq!(
        actual,
        expected,
        "{context}: expected cache={expected}, got {actual}\nstderr:\n{}",
        stderr(out)
    );
}

/// A task spec whose single scenario is bound to `test`. The `Then` text is
/// cosmetic — the verdict comes from the bound test.
fn spec_text(name: &str, scenario: &str, test: &str, then: &str) -> String {
    format!(
        r#"spec: task
name: "{name}"
tags: [fixture]
---

## Intent

Fixture spec for the verification-cache pins: the bound test decides the verdict.

## Decisions

- Bind every scenario to an existing test so the verdict is stable across runs.

## Boundaries

### Allowed Changes

- src/**

### Forbidden

- Do not add dependencies

## Completion Criteria

Scenario: {scenario}
  Test: {test}
  Given a fixture project
  When the bound test runs
  Then {then}
"#
    )
}

struct Fixture {
    root: PathBuf,
    spec: PathBuf,
    code: PathBuf,
}

impl Fixture {
    /// Spec + code root, no build manifest: the test layer has nothing to run,
    /// so a run costs milliseconds. The code root is a committed git repo.
    fn fast(tag: &str, scenario: &str, test: &str) -> Self {
        let root = temp_root(tag);
        let code = root.join("code");
        write_file(&code.join("notes.txt"), "fixture\n");
        git_init_and_commit(&code);
        let spec = root.join("specs/fixture.spec.md");
        write_file(
            &spec,
            &spec_text("cache-fixture", scenario, test, "it passes"),
        );
        Self { root, spec, code }
    }

    /// Spec + a real Rust crate so the test layer spawns `cargo test`.
    fn rust(tag: &str, test_fn: &str, test_body: &str) -> Self {
        let root = temp_root(tag);
        let code = root.join("code");
        // `target/` and `Cargo.lock` are produced by the verification run
        // itself; keeping them out of git keeps the key stable.
        write_file(&code.join(".gitignore"), "target/\nCargo.lock\n");
        write_file(
            &code.join("Cargo.toml"),
            "[package]\nname = \"cache-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        write_file(
            &code.join("src/lib.rs"),
            "pub fn ok() -> bool {\n    true\n}\n",
        );
        write_file(
            &code.join("tests/alpha.rs"),
            &format!("#[test]\nfn {test_fn}() {{\n    {test_body}\n}}\n"),
        );
        git_init_and_commit(&code);
        let spec = root.join("specs/fixture.spec.md");
        write_file(
            &spec,
            &spec_text("cache-fixture", "fixture scenario", test_fn, "it passes"),
        );
        Self { root, spec, code }
    }

    fn lifecycle_args(&self) -> Vec<String> {
        vec![
            "lifecycle".to_owned(),
            self.spec.display().to_string(),
            "--code".to_owned(),
            self.code.display().to_string(),
            "--format".to_owned(),
            "json".to_owned(),
            "--cache".to_owned(),
        ]
    }

    fn lifecycle(&self) -> Output {
        let args = self.lifecycle_args();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&refs)
    }

    fn cache_dir(&self) -> PathBuf {
        self.code.join(CACHE_DIR)
    }

    fn cache_entries(&self) -> usize {
        match fs::read_dir(self.cache_dir()) {
            Ok(entries) => entries.filter_map(Result::ok).count(),
            Err(_) => 0,
        }
    }
}

// ── completion criteria ─────────────────────────────────────────

#[test]
fn test_cache_hit_is_byte_identical_to_cold() {
    let fixture = Fixture::rust("hit-byte-identical", "test_ok", "assert!(true);");

    let (cold, cold_elapsed) = run_timed(
        &fixture
            .lifecycle_args()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert_cache(&cold, "miss", "first run with --cache");
    assert!(cold.status.success(), "cold run failed: {}", stderr(&cold));
    assert!(
        fixture.cache_entries() > 0,
        "cold run left no cache entry under {}",
        fixture.cache_dir().display()
    );

    let (warm, warm_elapsed) = run_timed(
        &fixture
            .lifecycle_args()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    assert_cache(&warm, "hit", "second run on the unchanged tree");
    assert!(warm.status.success(), "warm run failed: {}", stderr(&warm));

    // Byte identity: the only difference between the two reports is the
    // provenance field itself.
    let cold_text = stdout(&cold).replace("\"cache\": \"miss\"", "\"cache\": \"HIT\"");
    let warm_text = stdout(&warm).replace("\"cache\": \"hit\"", "\"cache\": \"HIT\"");
    assert_eq!(
        cold_text, warm_text,
        "warm report differs from the cold report beyond the cache field"
    );

    assert!(
        warm_elapsed * 5 <= cold_elapsed,
        "warm run should be materially faster: cold={cold_elapsed:?} warm={warm_elapsed:?}"
    );
}

#[test]
fn test_spec_edit_misses_cache() {
    let fixture = Fixture::fast("spec-edit", "fixture scenario", "test_ok");

    assert_cache(&fixture.lifecycle(), "miss", "initial run");

    let original = fs::read_to_string(&fixture.spec).unwrap();
    write_file(
        &fixture.spec,
        &original.replace("Fixture spec", "Fixtures spec"),
    );
    assert_cache(&fixture.lifecycle(), "miss", "after one byte of spec edit");

    // The edited spec caches on its own: the miss above came from the edit,
    // not from a permanently unstable key.
    assert_cache(&fixture.lifecycle(), "hit", "same edited spec again");
}

#[test]
fn test_code_edit_misses_cache() {
    let fixture = Fixture::fast("code-edit", "fixture scenario", "test_ok");
    let covered = fixture.code.join("src/lib.rs");
    write_file(&covered, "pub fn version() -> u32 {\n    1\n}\n");
    git(&fixture.code, &["add", "-A"]);
    git(
        &fixture.code,
        &[
            "-c",
            "user.email=cache-pin@example.com",
            "-c",
            "user.name=cache-pin",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "add covered code",
        ],
    );

    // Both runs see the same dirty path (` M src/lib.rs`); only its content
    // differs, so a hit here would mean the content hash is missing.
    write_file(&covered, "pub fn version() -> u32 {\n    2\n}\n");
    assert_cache(&fixture.lifecycle(), "miss", "first dirty-content run");

    write_file(&covered, "pub fn version() -> u32 {\n    3\n}\n");
    assert_cache(
        &fixture.lifecycle(),
        "miss",
        "dirty path content changed, status unchanged",
    );

    assert_cache(&fixture.lifecycle(), "hit", "unchanged dirty tree");
}

#[test]
fn test_test_file_edit_misses_cache() {
    let fixture = Fixture::rust("test-edit", "test_alpha", "assert!(true);");
    let test_file = fixture.code.join("tests/alpha.rs");

    // Run 1 and run 2 both see ` M tests/alpha.rs` — only the bytes differ.
    fs::write(
        &test_file,
        format!(
            "{}\n// first edit\n",
            fs::read_to_string(&test_file).unwrap()
        ),
    )
    .unwrap();
    assert_cache(&fixture.lifecycle(), "miss", "first edited test file");

    fs::write(
        &test_file,
        format!(
            "{}\n// second edit\n",
            fs::read_to_string(&test_file).unwrap()
        ),
    )
    .unwrap();
    assert_cache(
        &fixture.lifecycle(),
        "miss",
        "test file named by the Test: selector changed",
    );

    assert_cache(&fixture.lifecycle(), "hit", "unchanged test file");
}

#[test]
fn test_layer_set_is_part_of_key() {
    let fixture = Fixture::fast("layers", "fixture scenario", "test_ok");

    let with_layers = |layers: &str| {
        let mut args = fixture.lifecycle_args();
        args.push("--layers".to_owned());
        args.push(layers.to_owned());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&refs)
    };

    assert_cache(
        &with_layers("lint,test"),
        "miss",
        "first --layers lint,test run",
    );
    assert_cache(
        &with_layers("lint,test"),
        "hit",
        "same --layers value again",
    );
    assert_cache(&with_layers("test"), "miss", "different --layers value");
}

#[test]
fn test_unwritable_cache_is_a_loud_miss() {
    let fixture = Fixture::rust("unwritable", "test_fails", "assert!(false);");
    // A regular file where the cache directory belongs: every write fails, and
    // the failure must not be able to touch the verdict.
    write_file(&fixture.cache_dir(), "not a directory\n");

    let out = fixture.lifecycle();
    let report = report(&out);
    let diagnostic = stderr(&out);

    assert_cache(&out, "miss", "unwritable cache");
    assert!(
        diagnostic.contains("agent-spec cache:") && diagnostic.contains("failed to write"),
        "write failure must be reported loudly, stderr was:\n{diagnostic}"
    );
    assert!(
        !report["passed"].as_bool().unwrap_or(true),
        "a failing verification must stay failing: {report}"
    );
    assert_eq!(
        report["verification"]["summary"]["failed"], 1,
        "the run must still verify: {report}"
    );
    assert!(!out.status.success(), "run must exit non-zero");
}

#[test]
fn test_guard_workload_speedup() {
    // 27 specs, the same shape as `specs/` in this repo, each bound to the
    // fixture crate's `test_ok`. Guard spawns one `cargo test` per scenario,
    // which is the hot spot the cache exists to remove.
    let fixture = Fixture::rust("guard-workload", "test_ok", "assert!(true);");
    // The workload is exactly 27 specs, matching `specs/` in this repo.
    fs::remove_file(&fixture.spec).unwrap();
    let spec_dir = fixture.root.join("specs");
    for i in 0..27 {
        write_file(
            &spec_dir.join(format!("workload-{i:02}.spec.md")),
            &spec_text(
                &format!("workload-{i:02}"),
                "fixture scenario",
                "test_ok",
                "it passes",
            ),
        );
    }

    let spec_dir_arg = spec_dir.display().to_string();
    let code_arg = fixture.code.display().to_string();
    let args = [
        "guard",
        "--spec-dir",
        spec_dir_arg.as_str(),
        "--code",
        code_arg.as_str(),
        "--cache",
    ];

    let (cold, cold_elapsed) = run_timed(&args);
    assert!(
        cold.status.success(),
        "cold guard failed: {}",
        stderr(&cold)
    );
    assert!(
        stderr(&cold).contains("27 spec(s) passed"),
        "cold guard summary: {}",
        stderr(&cold)
    );

    let (warm, warm_elapsed) = run_timed(&args);
    assert!(
        warm.status.success(),
        "warm guard failed: {}",
        stderr(&warm)
    );
    assert!(
        stderr(&warm).contains("27 spec(s) passed"),
        "warm guard summary: {}",
        stderr(&warm)
    );

    assert!(
        warm_elapsed * 5 <= cold_elapsed,
        "guard workload must improve >=5x: cold={cold_elapsed:?} warm={warm_elapsed:?}\nwarm stderr:\n{}",
        stderr(&warm)
    );
}
