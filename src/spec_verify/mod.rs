mod ai_verifier;
mod bombadil_verifier;
mod boundaries;
pub mod bun_test_verifier;
pub mod complexity;
mod node_test_verifier;
mod structural;
pub mod tdd_guard_verifier;
mod test_verifier;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::spec_core::{
    ResolvedSpec, ScenarioResult, SpecResult, StepVerdict, Verdict, VerificationReport,
};

pub use ai_verifier::{AiBackend, AiVerifier, build_ai_request};
pub use bombadil_verifier::BombadilVerifier;
pub use boundaries::BoundariesVerifier;
// Single source of truth for boundary-pattern helpers (spec_gateway/plan.rs reuses them).
pub(crate) use boundaries::{looks_like_path_boundary, normalize_pattern, path_matches_pattern};
pub use bun_test_verifier::BunTestVerifier;
pub use complexity::ComplexityVerifier;
pub use node_test_verifier::NodeTestVerifier;
pub use structural::StructuralVerifier;
pub use tdd_guard_verifier::TddGuardVerifier;
pub use test_verifier::TestVerifier;

/// AI verifier mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    Off,
    Stub,
    External,
    /// Caller mode: emit AiRequests for the calling agent to resolve externally.
    Caller,
}

/// Context for verification.
pub struct VerificationContext {
    pub code_paths: Vec<PathBuf>,
    pub change_paths: Vec<PathBuf>,
    pub ai_mode: AiMode,
    pub resolved_spec: ResolvedSpec,
}

/// Which test runner should verify scenario selectors for the given code paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestVerifierKind {
    /// Rust project (`Cargo.toml` found) — use [`TestVerifier`].
    Rust,
    /// Node.js project using `node --test` — use [`NodeTestVerifier`].
    Node,
    /// Bun project (`bun test`) — use [`BunTestVerifier`].
    Bun,
    /// No test runner detected — scenarios fall through to skip.
    None,
}

/// Detect which test verifier to use from the code paths.
///
/// Priority: a `Cargo.toml` manifest selects the Rust verifier; a Bun marker
/// (lockfile, `bunfig.toml`, bun-flavored `package.json`, or `bun:test`
/// imports) selects the Bun verifier; otherwise a `package.json` with a
/// `node --test` script (or `*.test.js` files) selects the Node verifier;
/// otherwise no test verifier is selected.
pub fn detect_test_verifier(code_paths: &[PathBuf]) -> TestVerifierKind {
    if find_manifest_root(code_paths, "Cargo.toml").is_some() {
        return TestVerifierKind::Rust;
    }

    if bun_test_verifier::find_bun_project_root(code_paths).is_some() {
        return TestVerifierKind::Bun;
    }

    if let Some(root) = find_manifest_root(code_paths, "package.json")
        && node_test_verifier::is_node_test_project(&root, code_paths)
    {
        return TestVerifierKind::Node;
    }

    TestVerifierKind::None
}

/// Find the nearest ancestor directory (starting from each code path) that
/// contains the named manifest file (e.g. `Cargo.toml` or `package.json`).
pub(crate) fn find_manifest_root(code_paths: &[PathBuf], manifest_name: &str) -> Option<PathBuf> {
    for path in code_paths {
        let mut current = if path.is_file() {
            path.parent()?.to_path_buf()
        } else {
            path.clone()
        };

        loop {
            if current.join(manifest_name).is_file() {
                return Some(current);
            }
            if !current.pop() {
                break;
            }
        }
    }

    None
}

/// A test verifier that reports nothing, preserving the legacy skip behavior
/// when no test runner is detected.
#[derive(Debug, Clone, Copy)]
pub struct NoopVerifier;

impl Verifier for NoopVerifier {
    fn name(&self) -> &str {
        "noop"
    }

    fn verify(&self, _ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        Ok(Vec::new())
    }
}

/// Trait for scenario verifiers.
pub trait Verifier: Send + Sync {
    fn name(&self) -> &str;
    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>>;
}

/// Run verification with a set of verifiers.
pub fn run_verification(
    ctx: &VerificationContext,
    verifiers: &[&dyn Verifier],
) -> SpecResult<VerificationReport> {
    let mut all_results: Vec<ScenarioResult> = Vec::new();
    let mut covered_scenarios = HashSet::new();

    for verifier in verifiers {
        let results = verifier.verify(ctx)?;
        for result in results {
            if !covered_scenarios.insert(result.scenario_name.clone()) {
                if result.verdict != Verdict::Skip
                    && let Some(existing) = all_results
                        .iter_mut()
                        .find(|existing| existing.scenario_name == result.scenario_name)
                    && existing.verdict == Verdict::Skip
                {
                    *existing = result;
                }
                continue;
            }
            all_results.push(result);
        }
    }

    for scenario in &ctx.resolved_spec.all_scenarios {
        if covered_scenarios.contains(&scenario.name) {
            continue;
        }

        let step_results: Vec<StepVerdict> = scenario
            .steps
            .iter()
            .map(|step| StepVerdict {
                step_text: step.text.clone(),
                verdict: Verdict::Skip,
                reason: "no verifier covered this step".into(),
            })
            .collect();

        all_results.push(ScenarioResult {
            scenario_name: scenario.name.clone(),
            verdict: Verdict::Skip,
            step_results,
            evidence: Vec::new(),
            duration_ms: 0,
        });
    }

    Ok(VerificationReport::from_results(
        ctx.resolved_spec.task.meta.name.clone(),
        all_results,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use crate::spec_core::{
        ResolvedSpec, Scenario, ScenarioResult, Section, Span, SpecDocument, SpecLevel, SpecMeta,
        Step, StepKind, Verdict,
    };

    use super::{AiMode, VerificationContext, Verifier, run_verification};

    struct FirstVerifier;
    struct SecondVerifier;

    impl Verifier for FirstVerifier {
        fn name(&self) -> &str {
            "first"
        }

        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
            }])
        }
    }

    impl Verifier for SecondVerifier {
        fn name(&self) -> &str {
            "second"
        }

        fn verify(
            &self,
            _ctx: &VerificationContext,
        ) -> crate::spec_core::SpecResult<Vec<ScenarioResult>> {
            Ok(vec![ScenarioResult {
                scenario_name: "同一场景".into(),
                verdict: Verdict::Uncertain,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
            }])
        }
    }

    #[test]
    fn run_verification_keeps_first_result_for_same_scenario() {
        let scenario = Scenario {
            name: "同一场景".into(),
            steps: vec![Step {
                kind: StepKind::Given,
                text: "前置条件".into(),
                params: vec![],
                table: vec![],
                span: Span::line(1),
            }],
            test_selector: None,
            tags: vec![],
            review: Default::default(),
            mode: Default::default(),
            depends_on: vec![],
            span: Span::line(1),
        };
        let ctx = VerificationContext {
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
        };

        let first = FirstVerifier;
        let second = SecondVerifier;
        let report = run_verification(&ctx, &[&first, &second]).unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].verdict, Verdict::Pass);
    }
}
