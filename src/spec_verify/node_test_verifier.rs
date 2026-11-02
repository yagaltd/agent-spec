use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::spec_core::{
    Evidence, ReviewMode, Scenario, ScenarioResult, SpecError, SpecResult, StepVerdict,
    TestSelector, Verdict,
};

use super::{VerificationContext, Verifier};

/// Verifier for Node.js projects that run their tests with `node --test`.
///
/// It invokes `node --test --test-name-pattern '<selector>'` in the project
/// root and parses the TAP output: a scenario passes iff its named test ran
/// and no `not ok` line appears for it. Failures map to the same outcome
/// shape that [`super::TestVerifier`] produces.
pub struct NodeTestVerifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingSource {
    ExplicitScenarioSelector,
    LegacyComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestBinding {
    pub(crate) selector: TestSelector,
    pub(crate) source: BindingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapOutcome {
    Passed,
    Failed,
    NotFound,
}

impl Verifier for NodeTestVerifier {
    fn name(&self) -> &str {
        "node-test"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let Some(project_root) = find_node_project_root(&ctx.code_paths) else {
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
            let output = Command::new("node")
                .arg("--test")
                .arg("--test-name-pattern")
                .arg(&binding.selector.filter)
                .current_dir(&project_root)
                .output()
                .map_err(|err| {
                    SpecError::Verification(format!("failed to run node --test: {err}"))
                })?;
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

            let tap = parse_tap_for_test(&combined, &binding.selector.filter);

            let verdict = match tap {
                TapOutcome::Passed => {
                    if scenario.review == ReviewMode::Human {
                        Verdict::PendingReview
                    } else {
                        Verdict::Pass
                    }
                }
                TapOutcome::Failed => Verdict::Fail,
                TapOutcome::NotFound => Verdict::Skip,
            };

            let reason = match (tap, binding.source) {
                (TapOutcome::Passed, BindingSource::ExplicitScenarioSelector) => {
                    format!("covered by explicit test `{selector_label}`")
                }
                (TapOutcome::Passed, BindingSource::LegacyComment) => {
                    format!("covered by legacy @spec test `{selector_label}`")
                }
                (TapOutcome::Failed, BindingSource::ExplicitScenarioSelector) => {
                    format!("explicit test `{selector_label}` failed")
                }
                (TapOutcome::Failed, BindingSource::LegacyComment) => {
                    format!("legacy @spec test `{selector_label}` failed")
                }
                (TapOutcome::NotFound, _) => {
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
                    passed: matches!(tap, TapOutcome::Passed),
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

/// Find the Node project root (directory containing `package.json`) by walking
/// up from each code path.
pub(crate) fn find_node_project_root(code_paths: &[PathBuf]) -> Option<PathBuf> {
    super::find_manifest_root(code_paths, "package.json")
}

/// True when the given project root declares a `node --test` script or the
/// code paths contain a `*.test.js` file.
pub(crate) fn is_node_test_project(root: &Path, code_paths: &[PathBuf]) -> bool {
    test_script_uses_node_test(root) || has_test_files(code_paths)
}

fn test_script_uses_node_test(root: &Path) -> bool {
    let Ok(content) = fs::read_to_string(root.join("package.json")) else {
        return false;
    };

    let parsed: Option<serde_json::Value> = serde_json::from_str(&content).ok();
    if let Some(json) = parsed {
        return json
            .get("scripts")
            .and_then(|scripts| scripts.get("test"))
            .and_then(|script| script.as_str())
            .is_some_and(|script| script.contains("node --test"));
    }

    // Fall back to a naive substring match for malformed JSON.
    content.contains("node --test")
}

fn has_test_files(code_paths: &[PathBuf]) -> bool {
    for path in code_paths {
        if path.is_file() {
            if is_node_test_file(path) {
                return true;
            }
        } else if path.is_dir() && dir_has_test_files(path) {
            return true;
        }
    }
    false
}

fn dir_has_test_files(dir: &Path) -> bool {
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
            if dir_has_test_files(&path) {
                return true;
            }
        } else if is_node_test_file(&path) {
            return true;
        }
    }

    false
}

fn is_node_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with(".test.js") || name.ends_with(".test.mjs") || name.ends_with(".test.cjs")
        })
}

pub(crate) fn collect_legacy_comment_bindings(
    code_paths: &[PathBuf],
) -> SpecResult<HashMap<String, String>> {
    let mut bindings = HashMap::new();
    let mut files = Vec::new();

    for path in code_paths {
        if path.is_file() {
            if is_js_file(path) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            collect_js_files(path, &mut files);
        }
    }

    for file in files {
        let content = fs::read_to_string(&file)?;
        for (scenario, test_name) in extract_bindings(&content) {
            bindings.entry(scenario).or_insert(test_name);
        }
    }

    Ok(bindings)
}

pub(crate) fn resolve_test_binding(
    scenario: &Scenario,
    legacy_bindings: &HashMap<String, String>,
) -> Option<TestBinding> {
    if let Some(selector) = scenario.test_selector.as_ref() {
        return Some(TestBinding {
            selector: selector.clone(),
            source: BindingSource::ExplicitScenarioSelector,
        });
    }

    legacy_bindings
        .get(&scenario.name)
        .map(|selector| TestBinding {
            selector: TestSelector::filter_only(selector.clone()),
            source: BindingSource::LegacyComment,
        })
}

fn is_js_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "mjs" | "cjs")))
}

fn collect_js_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && (name.starts_with('.') || name == "node_modules" || name == "target")
            {
                continue;
            }
            collect_js_files(&path, files);
        } else if is_js_file(&path) {
            files.push(path);
        }
    }
}

/// Extract `// @spec: <scenario>` → `test('<name>', ...)` bindings from JS.
fn extract_bindings(source: &str) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    let mut pending_specs = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        if let Some(spec_name) = trimmed
            .strip_prefix("// @spec:")
            .or_else(|| trimmed.strip_prefix("//@spec:"))
        {
            pending_specs.push(spec_name.trim().to_string());
            continue;
        }

        if let Some(test_name) = extract_test_call_name(trimmed) {
            for spec_name in pending_specs.drain(..) {
                bindings.push((spec_name, test_name.clone()));
            }
            continue;
        }

        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending_specs.clear();
        }
    }

    bindings
}

fn extract_test_call_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("test(")?;
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' && quote != '`' {
        return None;
    }
    let after_quote = &rest[quote.len_utf8()..];
    let end = after_quote.find(quote)?;
    let name = &after_quote[..end];
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Parse TAP output for the result of a single named test.
///
/// Returns `Passed` if the test ran and no `not ok` line appears, `Failed` if
/// a `not ok` line appears for the test, or `NotFound` if the test never ran.
fn parse_tap_for_test(output: &str, test_name: &str) -> TapOutcome {
    let mut found_passing = false;

    for line in output.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("not ok ")
            && tap_point_matches(rest, test_name)
        {
            return TapOutcome::Failed;
        } else if let Some(rest) = trimmed.strip_prefix("ok ")
            && tap_point_matches(rest, test_name)
        {
            found_passing = true;
        }
    }

    if found_passing {
        TapOutcome::Passed
    } else {
        TapOutcome::NotFound
    }
}

/// Match the name portion of a TAP test-point line (`1 - test_name`).
pub(crate) fn tap_point_matches(rest: &str, test_name: &str) -> bool {
    rest.find(" - ")
        .is_some_and(|idx| rest[idx + 3..].trim_end() == test_name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{TapOutcome, extract_bindings, extract_test_call_name, parse_tap_for_test};

    #[test]
    fn parses_passing_tap_point() {
        let tap = "TAP version 13\nok 1 - test_passing_case\n1..1\n";
        assert_eq!(parse_tap_for_test(tap, "test_passing_case"), TapOutcome::Passed);
    }

    #[test]
    fn parses_failing_tap_point() {
        let tap = "TAP version 13\nnot ok 1 - test_failing_case\n1..1\n";
        assert_eq!(parse_tap_for_test(tap, "test_failing_case"), TapOutcome::Failed);
    }

    #[test]
    fn reports_not_found_when_test_never_ran() {
        let tap = "TAP version 13\n1..0\nok 1 - cases.test.js\n1..1\n";
        assert_eq!(
            parse_tap_for_test(tap, "test_never_ran"),
            TapOutcome::NotFound
        );
    }

    #[test]
    fn extracts_js_spec_bindings() {
        let source = r#"
// @spec: 场景一
// @spec: 场景二
test('test_example', () => {});
"#;

        let bindings = extract_bindings(source);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0], ("场景一".to_string(), "test_example".to_string()));
        assert_eq!(bindings[1], ("场景二".to_string(), "test_example".to_string()));
    }

    #[test]
    fn ignores_js_comment_not_followed_by_a_test() {
        let source = "// @spec: 场景一\nconst x = 1;\n";
        assert!(extract_bindings(source).is_empty());
    }

    #[test]
    fn extracts_test_call_name_across_quote_styles() {
        assert_eq!(extract_test_call_name("test('a', () => {})"), Some("a".into()));
        assert_eq!(extract_test_call_name("test(\"b\", () => {})"), Some("b".into()));
        assert_eq!(extract_test_call_name("test(`c`, () => {})"), Some("c".into()));
        assert_eq!(extract_test_call_name("const x = 1;"), None);
    }
}
