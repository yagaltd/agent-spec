use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::spec_core::{
    Evidence, ReviewMode, ScenarioResult, SpecError, SpecResult, StepVerdict, Verdict,
};

use super::node_test_verifier::{
    BindingSource, collect_legacy_comment_bindings, resolve_test_binding, tap_point_matches,
};
use super::{VerificationContext, Verifier};

/// Verifier for Bun projects that run their tests with `bun test`.
///
/// It invokes `bun test -t '<selector>'` in the project root and parses the
/// captured output: a scenario passes iff its named test ran (`(pass) <name>`
/// line, or a TAP `ok` point) and no `(fail)`/`not ok` line appears for it.
/// Failures map to the same outcome shape that [`super::NodeTestVerifier`]
/// produces.
pub struct BunTestVerifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BunOutcome {
    Passed,
    Failed,
    NotFound,
}

impl Verifier for BunTestVerifier {
    fn name(&self) -> &str {
        "bun-test"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let Some(project_root) = find_bun_project_root(&ctx.code_paths) else {
            return Ok(Vec::new());
        };

        let legacy_bindings = collect_legacy_comment_bindings(&ctx.code_paths)?;
        let mut results = Vec::new();

        for scenario in &ctx.resolved_spec.all_scenarios {
            let Some(binding) = resolve_test_binding(scenario, &legacy_bindings) else {
                continue;
            };

            let started = Instant::now();
            let selector_label = binding.selector.label();
            let (program, base_args) = bun_test_command(&project_root);
            let output = Command::new(&program)
                .args(&base_args)
                .args(["-t", &binding.selector.filter])
                .current_dir(&project_root)
                .output()
                .map_err(|err| SpecError::Verification(format!("failed to run bun test: {err}")))?;
            let duration_ms = started.elapsed().as_millis() as u64;

            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let combined = if stderr.trim().is_empty() {
                stdout.clone()
            } else if stdout.trim().is_empty() {
                stderr.clone()
            } else {
                format!("{stdout}\n{stderr}")
            };

            let outcome = parse_bun_output_for_test(&combined, &binding.selector.filter);

            let verdict = match outcome {
                BunOutcome::Passed => {
                    if scenario.review == ReviewMode::Human {
                        Verdict::PendingReview
                    } else {
                        Verdict::Pass
                    }
                }
                BunOutcome::Failed => Verdict::Fail,
                BunOutcome::NotFound => Verdict::Skip,
            };

            let reason = match (outcome, binding.source) {
                (BunOutcome::Passed, BindingSource::ExplicitScenarioSelector) => {
                    format!("covered by explicit test `{selector_label}`")
                }
                (BunOutcome::Passed, BindingSource::LegacyComment) => {
                    format!("covered by legacy @spec test `{selector_label}`")
                }
                (BunOutcome::Failed, BindingSource::ExplicitScenarioSelector) => {
                    format!("explicit test `{selector_label}` failed")
                }
                (BunOutcome::Failed, BindingSource::LegacyComment) => {
                    format!("legacy @spec test `{selector_label}` failed")
                }
                (BunOutcome::NotFound, _) => {
                    format!("test `{selector_label}` did not run")
                }
            };

            let step_results = scenario
                .steps
                .iter()
                .map(|step| StepVerdict {
                    step_text: step.text.clone(),
                    verdict,
                    reason: reason.clone(),
                })
                .collect();

            results.push(ScenarioResult {
                scenario_name: scenario.name.clone(),
                verdict,
                step_results,
                evidence: vec![Evidence::TestOutput {
                    test_name: selector_label,
                    stdout: combined,
                    passed: matches!(outcome, BunOutcome::Passed),
                    package: binding.selector.package.clone(),
                    level: binding.selector.level.clone(),
                    test_double: binding.selector.test_double.clone(),
                    targets: binding.selector.targets.clone(),
                }],
                duration_ms,
            });
        }

        Ok(results)
    }
}

/// Find the Bun project root by walking up from each code path.
///
/// A root qualifies when it contains a Bun lockfile/config marker (`bun.lock`,
/// `bun.lockb`, `bunfig.toml`), or a `package.json` whose test script /
/// dependency graph references bun, or — as a last resort — the code path
/// itself contains test files importing `bun:test`.
pub(crate) fn find_bun_project_root(code_paths: &[PathBuf]) -> Option<PathBuf> {
    for marker in ["bun.lock", "bun.lockb", "bunfig.toml"] {
        if let Some(root) = super::find_manifest_root(code_paths, marker) {
            return Some(root);
        }
    }

    if let Some(root) = super::find_manifest_root(code_paths, "package.json")
        && package_json_references_bun(&root)
    {
        return Some(root);
    }

    if let Some(root) = first_code_dir(code_paths)
        && dir_imports_bun_test(&root)
    {
        return Some(root);
    }

    None
}

/// Resolve the bun test invocation: prefer `scripts.test` from package.json
/// when it starts with `bun` (runtime-agnostic shape), else hardcode
/// `bun test`.
pub(crate) fn bun_test_command(root: &Path) -> (String, Vec<String>) {
    let script = fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|json| {
            json.get("scripts")
                .and_then(|scripts| scripts.get("test"))
                .and_then(|script| script.as_str())
                .map(str::to_owned)
        });

    if let Some(script) = script {
        let mut tokens = script.split_whitespace().map(str::to_owned);
        if tokens.next().as_deref() == Some("bun") {
            return ("bun".into(), tokens.collect());
        }
    }

    ("bun".into(), vec!["test".into()])
}

/// Parse captured bun test output for the result of a single named test.
///
/// Returns `Passed` when a `(pass) <name>` line (or a TAP `ok` point) names
/// the test, `Failed` when a `(fail) <name>` line (or `not ok` point) names
/// it, and `NotFound` when the filter matched no test at all.
pub(crate) fn parse_bun_output_for_test(output: &str, test_name: &str) -> BunOutcome {
    let mut found_passing = false;

    for line in output.lines() {
        let line = if line.contains('\x1b') {
            strip_ansi(line)
        } else {
            line.to_owned()
        };
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("(fail) ")
            && bun_line_names_test(rest, test_name)
        {
            return BunOutcome::Failed;
        } else if let Some(rest) = trimmed.strip_prefix("(pass) ")
            && bun_line_names_test(rest, test_name)
        {
            found_passing = true;
        } else if let Some(rest) = trimmed.strip_prefix("not ok ")
            && tap_point_matches(rest, test_name)
        {
            return BunOutcome::Failed;
        } else if let Some(rest) = trimmed.strip_prefix("ok ")
            && tap_point_matches(rest, test_name)
        {
            found_passing = true;
        }
    }

    if found_passing {
        BunOutcome::Passed
    } else {
        BunOutcome::NotFound
    }
}

fn first_code_dir(code_paths: &[PathBuf]) -> Option<PathBuf> {
    code_paths.iter().find_map(|path| {
        if path.is_dir() {
            Some(path.clone())
        } else {
            path.parent().map(Path::to_path_buf)
        }
    })
}

fn package_json_references_bun(root: &Path) -> bool {
    let Ok(content) = fs::read_to_string(root.join("package.json")) else {
        return false;
    };

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        if json
            .get("scripts")
            .and_then(|scripts| scripts.get("test"))
            .and_then(|script| script.as_str())
            .and_then(|script| script.split_whitespace().next())
            == Some("bun")
        {
            return true;
        }

        for key in [
            "dependencies",
            "devDependencies",
            "optionalDependencies",
            "peerDependencies",
        ] {
            if let Some(deps) = json.get(key).and_then(|deps| deps.as_object())
                && deps.keys().any(|name| is_bun_dependency(name))
            {
                return true;
            }
        }
        return false;
    }

    // Fall back to a naive substring match for malformed JSON.
    content.contains("bun test")
}

fn is_bun_dependency(name: &str) -> bool {
    name == "bun" || name == "bun-types" || name == "@types/bun" || name.starts_with("bun-")
}

fn dir_imports_bun_test(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && (name.starts_with('.') || name == "node_modules" || name == "target")
            {
                continue;
            }
            if dir_imports_bun_test(&path) {
                return true;
            }
        } else if is_bun_test_file(&path) {
            return true;
        }
    }

    false
}

fn is_bun_test_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !(name.contains(".test.") || name.contains(".spec.")) {
        return false;
    }
    fs::read_to_string(path)
        .map(|content| content.contains("bun:test"))
        .unwrap_or(false)
}

/// Match a `(pass|fail) <name>` line against the selector, allowing the
/// printed name to carry its describe prefix (`outer group > inner name`).
fn bun_line_names_test(rest: &str, test_name: &str) -> bool {
    let name = strip_duration_suffix(rest.trim_end());
    // Spec selectors may regex-escape parens for `bun test -t` (literal-paren
    // test names); correlate on the unescaped form so both sides line up.
    let wanted = test_name.replace("\\(", "(").replace("\\)", ")");
    name == wanted || name == test_name || name.ends_with(&format!(" > {wanted}"))
        || name.ends_with(&format!(" > {test_name}"))
}

/// Remove a trailing `[1.23ms]` style duration segment bun appends in some
/// output modes.
fn strip_duration_suffix(name: &str) -> &str {
    if let Some(open) = name.rfind(" [")
        && name.ends_with(']')
    {
        let inner = &name[open + 2..name.len() - 1];
        if inner.ends_with("ms") || inner.ends_with("\u{b5}s") || inner.ends_with('s') {
            return &name[..open];
        }
    }
    name
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for skipped in chars.by_ref() {
                if skipped.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::{TestVerifierKind, detect_test_verifier};
    use super::{
        BunOutcome, BunTestVerifier, VerificationContext, bun_test_command, find_bun_project_root,
        parse_bun_output_for_test,
    };
    use crate::spec_core::{
        ResolvedSpec, Scenario, Section, Span, SpecDocument, SpecLevel, SpecMeta, Step, StepKind,
        TestSelector, Verdict,
    };
    use crate::spec_verify::{AiMode, Verifier};

    fn temp_root(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-bun-{tag}-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn spec_with_selector(name: &str, filter: &str) -> VerificationContext {
        let scenario = Scenario {
            name: name.into(),
            steps: vec![Step {
                kind: StepKind::Given,
                text: "前置条件".into(),
                params: vec![],
                table: vec![],
                span: Span::line(1),
            }],
            test_selector: Some(TestSelector::filter_only(filter.to_string())),
            tags: vec![],
            review: Default::default(),
            mode: Default::default(),
            depends_on: vec![],
            span: Span::line(1),
        };
        VerificationContext {
            code_paths: vec![PathBuf::from(".")],
            change_paths: vec![],
            ai_mode: AiMode::Off,
            resolved_spec: ResolvedSpec {
                task: SpecDocument {
                    meta: SpecMeta {
                        level: SpecLevel::Task,
                        name: "test".into(),
                        inherits: None,
                        lang: vec![],
                        tags: vec![],
                        depends: vec![],
                        estimate: None,
                    },
                    sections: vec![Section::AcceptanceCriteria {
                        scenarios: vec![scenario.clone()],
                        span: Span::line(1),
                    }],
                    source_path: PathBuf::new(),
                },
                inherited_constraints: vec![],
                inherited_decisions: vec![],
                all_scenarios: vec![scenario],
            },
        }
    }

    // ── Detection ───────────────────────────────────────────────

    #[test]
    fn detects_bun_project_from_lockfile() {
        let root = temp_root("lock");
        fs::write(root.join("bun.lock"), "{}").unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn detects_bun_project_from_binary_lockfile() {
        let root = temp_root("lockb");
        fs::write(root.join("bun.lockb"), vec![0u8, 1, 2]).unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn detects_bun_project_from_bunfig() {
        let root = temp_root("fig");
        fs::write(root.join("bunfig.toml"), "[test]\n").unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn detects_bun_project_from_bun_test_script() {
        let root = temp_root("script");
        fs::write(
            root.join("package.json"),
            r#"{"scripts": {"test": "bun test"}}"#,
        )
        .unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn detects_bun_project_from_dependency_graph() {
        let root = temp_root("deps");
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies": {"bun-types": "1.0.0"}}"#,
        )
        .unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn detects_bun_project_from_test_files_importing_bun_test() {
        let root = temp_root("imports");
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests").join("unit.test.ts"),
            "import { test, expect } from \"bun:test\";\ntest(\"x\", () => {});\n",
        )
        .unwrap();
        assert_eq!(find_bun_project_root(std::slice::from_ref(&root)), Some(root));
    }

    #[test]
    fn ignores_node_test_project_without_bun_markers() {
        let root = temp_root("node");
        fs::write(
            root.join("package.json"),
            r#"{"scripts": {"test": "node --test"}}"#,
        )
        .unwrap();
        assert_eq!(find_bun_project_root(&[root]), None);
    }

    #[test]
    fn detection_prefers_bun_over_node_for_bun_scripts() {
        let root = temp_root("prefer");
        fs::write(
            root.join("package.json"),
            r#"{"scripts": {"test": "bun test"}}"#,
        )
        .unwrap();
        assert_eq!(detect_test_verifier(&[root]), TestVerifierKind::Bun);
    }

    #[test]
    fn detection_still_selects_rust_first() {
        let root = temp_root("rust");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert_eq!(detect_test_verifier(&[root]), TestVerifierKind::Rust);
    }

    // ── Command selection ───────────────────────────────────────

    #[test]
    fn falls_back_to_hardcoded_bun_test() {
        let root = temp_root("fallback");
        let (program, args) = bun_test_command(&root);
        assert_eq!(program, "bun");
        assert_eq!(args, vec!["test".to_string()]);
    }

    #[test]
    fn prefers_package_json_test_script_starting_with_bun() {
        let root = temp_root("cmdscript");
        fs::write(
            root.join("package.json"),
            r#"{"scripts": {"test": "bun test --preload ./preload.ts"}}"#,
        )
        .unwrap();
        let (program, args) = bun_test_command(&root);
        assert_eq!(program, "bun");
        assert_eq!(
            args,
            vec!["test".to_string(), "--preload".to_string(), "./preload.ts".to_string()]
        );
    }

    #[test]
    fn ignores_non_bun_test_script() {
        let root = temp_root("vitest");
        fs::write(
            root.join("package.json"),
            r#"{"scripts": {"test": "vitest run"}}"#,
        )
        .unwrap();
        let (program, args) = bun_test_command(&root);
        assert_eq!(program, "bun");
        assert_eq!(args, vec!["test".to_string()]);
    }

    // ── Output parsing ──────────────────────────────────────────

    #[test]
    fn parses_bun_pass_line() {
        let out = "bun test v1.3.11\n\nprobe.test.ts:\n(pass) alpha passes\n\n 1 pass\n 0 fail\n";
        assert_eq!(
            parse_bun_output_for_test(out, "alpha passes"),
            BunOutcome::Passed
        );
    }

    #[test]
    fn parses_bun_fail_line() {
        let out = "probe.test.ts:\n(pass) alpha passes\n(fail) beta fails\n 1 pass\n 1 fail\n";
        assert_eq!(
            parse_bun_output_for_test(out, "beta fails"),
            BunOutcome::Failed
        );
    }

    #[test]
    fn parses_nested_describe_names() {
        let out = "nested.test.ts:\n(pass) outer group > inner name\n 1 pass\n";
        assert_eq!(
            parse_bun_output_for_test(out, "inner name"),
            BunOutcome::Passed
        );
    }

    #[test]
    fn parses_pass_line_with_duration_suffix() {
        let out = "probe.test.ts:\n(pass) alpha passes [1.00ms]\n 1 pass\n";
        assert_eq!(
            parse_bun_output_for_test(out, "alpha passes"),
            BunOutcome::Passed
        );
    }

    #[test]
    fn reports_not_found_when_filter_matched_nothing() {
        let out = "probe.test.ts:\n\nerror: regex \"nope\" matched 0 tests. Searched 1 file\n";
        assert_eq!(parse_bun_output_for_test(out, "nope"), BunOutcome::NotFound);
    }

    #[test]
    fn parses_tap_style_points_too() {
        let out = "TAP version 13\nok 1 - alpha passes\n1..1\n";
        assert_eq!(
            parse_bun_output_for_test(out, "alpha passes"),
            BunOutcome::Passed
        );
        let out = "TAP version 13\nnot ok 1 - alpha passes\n1..1\n";
        assert_eq!(
            parse_bun_output_for_test(out, "alpha passes"),
            BunOutcome::Failed
        );
    }

    #[test]
    fn strips_ansi_coloring_before_matching() {
        let out = "probe.test.ts:\n(pass) \x1b[32malpha passes\x1b[0m\n 1 pass\n";
        assert_eq!(
            parse_bun_output_for_test(out, "alpha passes"),
            BunOutcome::Passed
        );
    }

    // ── Execution (real bun) ────────────────────────────────────

    #[test]
    fn executes_bun_scenario_pass_fail_and_skip() {
        if std::process::Command::new("bun").arg("--version").output().is_err() {
            // bun not installed in this environment; detection stays exercised.
            return;
        }

        let root = temp_root("exec");
        fs::write(
            root.join("probe.test.ts"),
            "import { test, expect } from \"bun:test\";\n\
             test(\"alpha passes\", () => { expect(1 + 1).toBe(2); });\n\
             test(\"beta fails\", () => { expect(1).toBe(2); });\n",
        )
        .unwrap();

        let mut pass = spec_with_selector("s-pass", "alpha passes");
        pass.code_paths = vec![root.clone()];
        let mut fail = spec_with_selector("s-fail", "beta fails");
        fail.code_paths = vec![root.clone()];
        let mut missing = spec_with_selector("s-missing", "nope");
        missing.code_paths = vec![root.clone()];

        let verifier = BunTestVerifier;
        assert_eq!(verifier.name(), "bun-test");

        let results = verifier.verify(&pass).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Pass);

        let results = verifier.verify(&fail).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Fail);

        let results = verifier.verify(&missing).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Skip);
    }

}

#[cfg(test)]
mod escaped_paren_tests {
    use super::*;

    #[test]
    fn escaped_selector_correlates_with_plain_paren_output() {
        // Spec selectors escape parens for the `bun test -t` regex; bun's
        // output line carries the plain name. Correlation must unescape.
        let line = "(pass) roleBody > real role file shapes stay clean (all four files)";
        assert!(bun_line_names_test(
            "roleBody > real role file shapes stay clean (all four files)",
            "real role file shapes stay clean \\(all four files\\)"
        ));
        assert!(bun_line_names_test(
            line.trim_start_matches("(pass) "),
            "real role file shapes stay clean \\(all four files\\)"
        ));
        assert!(!bun_line_names_test(
            "roleBody > unrelated test",
            "real role file shapes stay clean \\(all four files\\)"
        ));
    }
}
