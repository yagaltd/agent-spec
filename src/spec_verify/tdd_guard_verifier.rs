use crate::spec_core::{
    Evidence, ScenarioResult, SpecResult, StepVerdict, Verdict,
};
use crate::spec_verify::VerificationContext;
use crate::spec_verify::Verifier;

/// Verifier that runs `tdd-guard lint` as a verification layer.
/// Returns pass/fail for each scenario based on whether tdd-guard
/// reports any violations in the test files covering that scenario.
///
/// Enable with: `--layers lint,boundary,test,tdd-guard`
pub struct TddGuardVerifier;

impl Verifier for TddGuardVerifier {
    fn name(&self) -> &str {
        "tdd-guard"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        // Check if tdd-guard is available
        let tdd_guard_available = std::process::Command::new("tdd-guard")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());

        if !tdd_guard_available {
            return Ok(skip_all(
                &ctx.resolved_spec,
                "tdd-guard not installed — install with: npm install -g tdd-guard",
            ));
        }

        // Run tdd-guard lint
        let code_path = ctx
            .code_paths
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());

        let output = std::process::Command::new("tdd-guard")
            .args(["lint", "--src", &code_path, "--format", "json"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lint_result = parse_tdd_guard_output(&stdout);
                let exit_code = output.status.code().unwrap_or(1);

                Ok(ctx
                    .resolved_spec
                    .all_scenarios
                    .iter()
                    .map(|scenario| {
                        let selector = scenario
                            .test_selector
                            .as_ref()
                            .map(|s| s.filter.clone());

                        let related_violations =
                            find_related_violations(&selector, &lint_result);

                        let (verdict, evidence) = if exit_code == 0 {
                            (Verdict::Pass, vec![Evidence::AiAnalysis {
                                model: "tdd-guard".into(),
                                confidence: 1.0,
                                reasoning: "tdd-guard lint: all checks pass".into(),
                            }])
                        } else if related_violations.is_empty() {
                            (Verdict::Pass, vec![Evidence::AiAnalysis {
                                model: "tdd-guard".into(),
                                confidence: 0.8,
                                reasoning: format!(
                                    "tdd-guard lint: {} violations (none specific to this scenario)",
                                    lint_result.total_violations()
                                ),
                            }])
                        } else {
                            (Verdict::Fail, related_violations
                                .iter()
                                .map(|v| Evidence::AiAnalysis {
                                    model: "tdd-guard".into(),
                                    confidence: 1.0,
                                    reasoning: format!("{}:{} — {}", v.file, v.line, v.message),
                                })
                                .collect())
                        };

                        ScenarioResult {
                            scenario_name: scenario.name.clone(),
                            verdict,
                            step_results: vec![],
                            evidence,
                            duration_ms: 0,
                        }
                    })
                    .collect())
            }
            Err(e) => Ok(ctx
                .resolved_spec
                .all_scenarios
                .iter()
                .map(|scenario| ScenarioResult {
                    scenario_name: scenario.name.clone(),
                    verdict: Verdict::Uncertain,
                    step_results: vec![StepVerdict {
                        step_text: "tdd-guard execution failed".into(),
                        verdict: Verdict::Uncertain,
                        reason: e.to_string(),
                    }],
                    evidence: vec![Evidence::AiAnalysis {
                        model: "tdd-guard".into(),
                        confidence: 0.0,
                        reasoning: format!("tdd-guard failed to run: {e}"),
                    }],
                    duration_ms: 0,
                })
                .collect()),
        }
    }
}

fn skip_all(
    resolved: &crate::spec_core::ResolvedSpec,
    reason: &str,
) -> Vec<ScenarioResult> {
    resolved
        .all_scenarios
        .iter()
        .map(|scenario| ScenarioResult {
            scenario_name: scenario.name.clone(),
            verdict: Verdict::Skip,
            step_results: vec![StepVerdict {
                step_text: reason.into(),
                verdict: Verdict::Skip,
                reason: reason.into(),
            }],
            evidence: vec![],
            duration_ms: 0,
        })
        .collect()
}

#[derive(Default)]
struct TddGuardLintResult {
    checks: Vec<TddGuardCheck>,
}

struct TddGuardCheck {
    #[allow(dead_code)]
    rule: String,
    #[allow(dead_code)]
    status: String,
    violations: Vec<TddGuardViolation>,
}

#[derive(Clone)]
struct TddGuardViolation {
    file: String,
    line: u32,
    message: String,
}

impl TddGuardLintResult {
    fn total_violations(&self) -> usize {
        self.checks
            .iter()
            .map(|c| c.violations.len())
            .sum()
    }
}

fn parse_tdd_guard_output(json: &str) -> TddGuardLintResult {
    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return TddGuardLintResult::default(),
    };

    let mut result = TddGuardLintResult::default();

    if let Some(checks) = parsed.get("checks").and_then(|c| c.as_array()) {
        for check in checks {
            let rule = check
                .get("rule")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            let status = check
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            let violations = check
                .get("violations")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            Some(TddGuardViolation {
                                file: v.get("file")?.as_str()?.to_string(),
                                line: v.get("line")?.as_u64()? as u32,
                                message: v.get("message")?.as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            result.checks.push(TddGuardCheck {
                rule,
                status,
                violations,
            });
        }
    }

    result
}

fn find_related_violations(
    test_selector: &Option<String>,
    lint_result: &TddGuardLintResult,
) -> Vec<TddGuardViolation> {
    let selector = match test_selector {
        Some(s) => s,
        None => return vec![],
    };

    lint_result
        .checks
        .iter()
        .flat_map(|c| &c.violations)
        .filter(|v| {
            v.message.contains(selector.as_str()) || v.file.contains(selector.as_str())
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_output() {
        let result = parse_tdd_guard_output("");
        assert!(result.checks.is_empty());
    }

    #[test]
    fn parse_valid_output() {
        let json = r#"{
            "command": "lint",
            "exit_code": 1,
            "checks": [
                {
                    "rule": "no-skipped-tests",
                    "status": "fail",
                    "violations": [
                        {
                            "file": "tests/auth.test.ts",
                            "line": 5,
                            "message": "Skipped test: test_login"
                        }
                    ]
                },
                {
                    "rule": "no-internal-mocks",
                    "status": "pass",
                    "violations": []
                }
            ]
        }"#;

        let result = parse_tdd_guard_output(json);
        assert_eq!(result.checks.len(), 2);
        assert_eq!(result.checks[0].violations.len(), 1);
        assert_eq!(result.checks[0].violations[0].file, "tests/auth.test.ts");
        assert_eq!(result.total_violations(), 1);
    }

    #[test]
    fn find_related_violations_matches_selector() {
        let json = r#"{
            "command": "lint",
            "exit_code": 1,
            "checks": [
                {
                    "rule": "no-skipped-tests",
                    "status": "fail",
                    "violations": [
                        {
                            "file": "tests/auth.test.ts",
                            "line": 5,
                            "message": "Skipped test: test_login_flow"
                        }
                    ]
                }
            ]
        }"#;

        let result = parse_tdd_guard_output(json);
        let related = find_related_violations(&Some("test_login_flow".into()), &result);
        assert_eq!(related.len(), 1);

        let unrelated = find_related_violations(&Some("test_other".into()), &result);
        assert!(unrelated.is_empty());
    }
}
