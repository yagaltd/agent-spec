use crate::spec_core::{
    Evidence, ScenarioResult, SpecResult, StepVerdict, Verdict,
};
use crate::spec_verify::VerificationContext;
use crate::spec_verify::Verifier;

/// Verifier that runs Bombadil for web UI property-based testing.
/// Only activates when:
/// 1. `bombadil` is installed
/// 2. The spec has scenarios tagged with `bombadil` or `web-ui`
///
/// Enable with: `--layers lint,boundary,test,bombadil`
pub struct BombadilVerifier;

impl Verifier for BombadilVerifier {
    fn name(&self) -> &str {
        "bombadil"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        // Check if bombadil is available
        let bombadil_available = std::process::Command::new("bombadil")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());

        if !bombadil_available {
            return Ok(skip_all(
                &ctx.resolved_spec,
                "bombadil not installed — optional for web UI testing",
            ));
        }

        // Find scenarios tagged for bombadil or web-ui
        let bombadil_scenarios: Vec<_> = ctx
            .resolved_spec
            .all_scenarios
            .iter()
            .filter(|s| {
                s.tags.iter().any(|t| {
                    let t_lower = t.to_lowercase();
                    t_lower == "bombadil" || t_lower == "web-ui" || t_lower == "webui"
                })
            })
            .collect();

        if bombadil_scenarios.is_empty() {
            return Ok(skip_all(
                &ctx.resolved_spec,
                "no bombadil/web-ui tagged scenarios",
            ));
        }

        // Run bombadil for each tagged scenario
        let code_path = ctx
            .code_paths
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());

        let mut results = Vec::new();

        for scenario in &bombadil_scenarios {
            let spec_file = find_bombadil_spec(&code_path, &scenario.name);

            match spec_file {
                Some(spec_path) => {
                    let output = std::process::Command::new("bombadil")
                        .args(["test", &code_path, &spec_path, "--exit-on-violation"])
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .output();

                    match output {
                        Ok(output) => {
                            let exit_code = output.status.code().unwrap_or(1);
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                            results.push(ScenarioResult {
                                scenario_name: scenario.name.clone(),
                                verdict: if exit_code == 0 {
                                    Verdict::Pass
                                } else {
                                    Verdict::Fail
                                },
                                step_results: vec![],
                                evidence: if exit_code == 0 {
                                    vec![Evidence::AiAnalysis {
                                        model: "bombadil".into(),
                                        confidence: 1.0,
                                        reasoning: "bombadil: all properties hold".into(),
                                    }]
                                } else {
                                    vec![Evidence::AiAnalysis {
                                        model: "bombadil".into(),
                                        confidence: 1.0,
                                        reasoning: format!(
                                            "bombadil violations:\n{}\n{}",
                                            stdout, stderr
                                        ),
                                    }]
                                },
                                duration_ms: 0,
                            });
                        }
                        Err(e) => {
                            results.push(ScenarioResult {
                                scenario_name: scenario.name.clone(),
                                verdict: Verdict::Uncertain,
                                step_results: vec![StepVerdict {
                                    step_text: "bombadil execution failed".into(),
                                    verdict: Verdict::Uncertain,
                                    reason: e.to_string(),
                                }],
                                evidence: vec![Evidence::AiAnalysis {
                                    model: "bombadil".into(),
                                    confidence: 0.0,
                                    reasoning: format!("bombadil failed: {e}"),
                                }],
                                duration_ms: 0,
                            });
                        }
                    }
                }
                None => {
                    results.push(ScenarioResult {
                        scenario_name: scenario.name.clone(),
                        verdict: Verdict::Skip,
                        step_results: vec![StepVerdict {
                            step_text: "no bombadil spec file found".into(),
                            verdict: Verdict::Skip,
                            reason: format!(
                                "no .spec.ts file found for scenario '{}' in {}",
                                scenario.name, code_path
                            ),
                        }],
                        evidence: vec![],
                        duration_ms: 0,
                    });
                }
            }
        }

        // Add skip results for non-bombadil scenarios
        for scenario in &ctx.resolved_spec.all_scenarios {
            if !bombadil_scenarios.iter().any(|bs| bs.name == scenario.name) {
                results.push(ScenarioResult {
                    scenario_name: scenario.name.clone(),
                    verdict: Verdict::Skip,
                    step_results: vec![StepVerdict {
                        step_text: "not a bombadil scenario".into(),
                        verdict: Verdict::Skip,
                        reason: "scenario not tagged with bombadil/web-ui".into(),
                    }],
                    evidence: vec![],
                    duration_ms: 0,
                });
            }
        }

        Ok(results)
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

/// Look for a bombadil spec file that matches the scenario name
fn find_bombadil_spec(code_path: &str, scenario_name: &str) -> Option<String> {
    let slug = scenario_name
        .to_lowercase()
        .replace(' ', "-")
        .replace('_', "-");

    let candidates = [
        format!("{code_path}/bombadil/{slug}.spec.ts"),
        format!("{code_path}/specs/{slug}.bombadil.spec.ts"),
        format!("{code_path}/tests/{slug}.spec.ts"),
        format!("{code_path}/e2e/{slug}.spec.ts"),
    ];

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.clone());
        }
    }

    None
}
