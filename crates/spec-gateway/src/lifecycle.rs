use std::sync::Arc;

use std::path::{Path, PathBuf};

use spec_core::{LintReport, SpecResult, Verdict, VerificationReport};
use spec_lint::LintPipeline;
use spec_report::OutputFormat;
use spec_verify::{
    AiBackend, AiMode, AiVerifier, BoundariesVerifier, StructuralVerifier, TestVerifier,
    VerificationContext, Verifier,
    run_verification,
};

use crate::TaskContract;

/// The main entry point for agent-spec lifecycle integration.
///
/// Provides a simple API surface that an agent framework can call
/// at each lifecycle stage without understanding internal crates.
///
/// # Usage
///
/// ```rust,no_run
/// use spec_gateway::SpecGateway;
///
/// let gw = SpecGateway::load("task.spec").unwrap();
///
/// // 1. PLAN: Get spec context for agent prompt
/// let contract = gw.plan();
/// let prompt_fragment = contract.to_prompt();
///
/// // 2. GATE: Check spec quality before coding
/// let lint = gw.lint();
/// if lint.quality_score.overall < 0.6 {
///     panic!("spec quality too low, fix spec first");
/// }
///
/// // ... agent generates code ...
///
/// // 3. VERIFY: Check code against spec
/// let report = gw.verify("./src").unwrap();
///
/// // 4. DECIDE: Pass or retry?
/// if gw.is_passing(&report) {
///     println!("ready to merge");
/// } else {
///     let feedback = gw.failure_summary(&report);
///     println!("retry with feedback:\n{feedback}");
/// }
/// ```
pub struct SpecGateway {
    doc: spec_core::SpecDocument,
    resolved: spec_core::ResolvedSpec,
}

impl SpecGateway {
    /// Load a spec file from disk.
    pub fn load(spec_path: impl AsRef<Path>) -> SpecResult<Self> {
        let doc = spec_parser::parse_spec(spec_path.as_ref())?;
        let resolved = spec_parser::resolve_spec(doc.clone(), &[])?;
        Ok(Self { doc, resolved })
    }

    /// Load a spec from a string (for tests or inline specs).
    pub fn from_input(input: &str) -> SpecResult<Self> {
        let doc = spec_parser::parse_spec_from_str(input)?;
        let resolved = spec_parser::resolve_spec(doc.clone(), &[])?;
        Ok(Self { doc, resolved })
    }

    // ── Stage 1: PLAN ───────────────────────────────────────────

    /// Get the default planning contract for agent execution.
    pub fn plan(&self) -> TaskContract {
        TaskContract::from_resolved(&self.resolved)
    }

    /// Get the explicit task contract for agent execution.
    pub fn contract(&self) -> TaskContract {
        self.plan()
    }

    /// Legacy compatibility alias for older brief-based callers.
    #[allow(deprecated)]
    #[deprecated(note = "Use SpecGateway::plan()/contract() instead")]
    pub fn brief(&self) -> crate::SpecBrief {
        crate::SpecBrief::from_contract(&self.plan())
    }

    /// Get the full AST as JSON (for advanced agent use).
    pub fn ast_json(&self) -> String {
        serde_json::to_string_pretty(&self.doc).unwrap_or_default()
    }

    // ── Stage 2: GATE ───────────────────────────────────────────

    /// Run lint pipeline and return quality report.
    pub fn lint(&self) -> LintReport {
        let pipeline = LintPipeline::with_defaults();
        pipeline.run(&self.doc)
    }

    /// Check if spec quality meets a threshold.
    pub fn quality_gate(&self, min_score: f64) -> Result<LintReport, GateFailure> {
        let report = self.lint();
        if report.has_errors() || report.quality_score.overall < min_score {
            Err(GateFailure {
                actual_score: report.quality_score.overall,
                required_score: min_score,
                report,
            })
        } else {
            Ok(report)
        }
    }

    // ── Stage 3: VERIFY ─────────────────────────────────────────

    /// Verify code against this spec.
    pub fn verify(&self, code_path: impl AsRef<Path>) -> SpecResult<VerificationReport> {
        self.verify_with_ai_mode(code_path, AiMode::Off)
    }

    /// Verify code against this spec with an explicit change set for boundary checks.
    pub fn verify_with_changes(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_mode(code_path, change_paths, AiMode::Off)
    }

    /// Verify code against this spec with an explicit AI verification mode.
    pub fn verify_with_ai_mode(
        &self,
        code_path: impl AsRef<Path>,
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_mode(code_path, &[], ai_mode)
    }

    /// Verify code against this spec using a backend supplied by the embedding host.
    pub fn verify_with_ai_backend(
        &self,
        code_path: impl AsRef<Path>,
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.verify_with_changes_and_ai_backend(code_path, &[], backend)
    }

    /// Verify code against this spec with explicit change set and AI mode options.
    pub fn verify_with_changes_and_ai_mode(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            vec![code_path.as_ref().to_path_buf()],
            change_paths.to_vec(),
            ai_mode,
            AiVerifier::from_mode(ai_mode),
        )
    }

    /// Verify code against this spec with explicit change set and a host-supplied AI backend.
    pub fn verify_with_changes_and_ai_backend(
        &self,
        code_path: impl AsRef<Path>,
        change_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            vec![code_path.as_ref().to_path_buf()],
            change_paths.to_vec(),
            AiMode::External,
            AiVerifier::with_backend(backend),
        )
    }

    /// Verify against multiple code paths.
    pub fn verify_paths(&self, code_paths: &[PathBuf]) -> SpecResult<VerificationReport> {
        self.verify_paths_with_ai_mode(code_paths, AiMode::Off)
    }

    /// Verify against multiple code paths with an explicit change set for boundary checks.
    pub fn verify_paths_with_changes(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_mode(code_paths, change_paths, AiMode::Off)
    }

    /// Verify against multiple code paths with an explicit AI verification mode.
    pub fn verify_paths_with_ai_mode(
        &self,
        code_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_mode(code_paths, &[], ai_mode)
    }

    /// Verify against multiple code paths using a backend supplied by the embedding host.
    pub fn verify_paths_with_ai_backend(
        &self,
        code_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.verify_paths_with_changes_and_ai_backend(code_paths, &[], backend)
    }

    /// Verify against multiple code paths with explicit change set and AI mode options.
    pub fn verify_paths_with_changes_and_ai_mode(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
        ai_mode: AiMode,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            code_paths.to_vec(),
            change_paths.to_vec(),
            ai_mode,
            AiVerifier::from_mode(ai_mode),
        )
    }

    /// Verify against multiple code paths with explicit change set and a host-supplied AI backend.
    pub fn verify_paths_with_changes_and_ai_backend(
        &self,
        code_paths: &[PathBuf],
        change_paths: &[PathBuf],
        backend: Arc<dyn AiBackend>,
    ) -> SpecResult<VerificationReport> {
        self.run_verify(
            code_paths.to_vec(),
            change_paths.to_vec(),
            AiMode::External,
            AiVerifier::with_backend(backend),
        )
    }

    fn run_verify(
        &self,
        code_paths: Vec<PathBuf>,
        change_paths: Vec<PathBuf>,
        ai_mode: AiMode,
        ai: AiVerifier,
    ) -> SpecResult<VerificationReport> {
        let ctx = VerificationContext {
            code_paths,
            change_paths,
            ai_mode,
            resolved_spec: self.resolved.clone(),
        };

        let structural = StructuralVerifier;
        let boundaries = BoundariesVerifier;
        let test = TestVerifier;
        let verifiers: Vec<&dyn Verifier> = vec![&structural, &boundaries, &test, &ai];
        run_verification(&ctx, &verifiers)
    }

    // ── Stage 4: DECIDE ─────────────────────────────────────────

    /// Quick check: does the report have zero failures?
    pub fn is_passing(&self, report: &VerificationReport) -> bool {
        report.summary.total > 0
            && report.summary.failed == 0
            && report.summary.skipped == 0
            && report.summary.uncertain == 0
    }

    /// Generate a failure summary string suitable for feeding back
    /// to an agent as a retry prompt.
    pub fn failure_summary(&self, report: &VerificationReport) -> String {
        let mut out = String::new();
        out.push_str("## Verification Failed\n\n");
        out.push_str(&format!(
            "{} of {} scenarios are non-passing.\n\n",
            report.summary.failed + report.summary.skipped + report.summary.uncertain,
            report.summary.total,
        ));

        out.push_str("### Non-passing Scenarios\n\n");
        for result in &report.results {
            if result.verdict != Verdict::Pass {
                out.push_str(&format!("**{}**\n", result.scenario_name));
                out.push_str(&format!("- verdict: {:?}\n", result.verdict));
                for step in &result.step_results {
                    if step.verdict != Verdict::Pass {
                        out.push_str(&format!(
                            "- {:?}: {} ({})\n",
                            step.verdict, step.step_text, step.reason
                        ));
                    }
                }
                for ev in &result.evidence {
                    match ev {
                        spec_core::Evidence::CodeSnippet {
                            file,
                            line,
                            content,
                        } => {
                            out.push_str(&format!("  > {file}:{line}: `{content}`\n"));
                        }
                        spec_core::Evidence::TestOutput {
                            test_name, passed, ..
                        } => {
                            out.push_str(&format!("  > test `{test_name}`: passed={passed}\n"));
                        }
                        spec_core::Evidence::AiAnalysis {
                            model,
                            confidence,
                            reasoning,
                        } => {
                            out.push_str(&format!(
                                "  > ai `{model}`: confidence={confidence:.2}; {reasoning}\n"
                            ));
                        }
                        _ => {}
                    }
                }
                out.push('\n');
            }
        }

        out.push_str("### Action Required\n\n");
        out.push_str("Resolve the failures or missing verification coverage above and re-run verification.\n");

        out
    }

    /// Format verification report in the given format.
    pub fn format_report(&self, report: &VerificationReport, format: &str) -> String {
        let fmt = match format {
            "json" => OutputFormat::Json,
            "md" | "markdown" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        };
        spec_report::format_verification(report, &fmt)
    }

    /// Format lint report in the given format.
    pub fn format_lint_report(&self, report: &LintReport, format: &str) -> String {
        let fmt = match format {
            "json" => OutputFormat::Json,
            "md" | "markdown" => OutputFormat::Markdown,
            _ => OutputFormat::Text,
        };
        spec_report::format_lint(report, &fmt)
    }
}

/// Gate failure when spec quality is below threshold.
pub struct GateFailure {
    pub actual_score: f64,
    pub required_score: f64,
    pub report: LintReport,
}

impl std::fmt::Display for GateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error_count = self.report.error_count();
        if error_count > 0 {
            write!(
                f,
                "spec has {error_count} error-level lint issue(s) (quality {:.0}%, required {:.0}%)",
                self.actual_score * 100.0,
                self.required_score * 100.0,
            )
        } else {
            write!(
                f,
                "spec quality {:.0}% below required {:.0}% ({} issues)",
                self.actual_score * 100.0,
                self.required_score * 100.0,
                self.report.diagnostics.len(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use spec_verify::AiMode;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SAMPLE: &str = r#"spec: task
name: "测试任务"
tags: [test]
---

## 意图

实现一个简单的功能。

## 约束

### 禁止做
- 禁止使用 `.unwrap()`
- 禁止使用 `panic!`

### 必须做
- 所有错误必须返回 Result

## 已定决策

- 使用 thiserror 处理错误类型

## 验收标准

场景: 正常路径
  测试: test_full_lifecycle
  假设 输入有效
  当 调用函数
  那么 返回 Ok

场景: 错误路径
  测试: test_plan_returns_task_contract
  假设 输入无效
  当 调用函数
  那么 返回 Err

## 排除范围

- 日志系统
"#;

    #[test]
    fn test_full_lifecycle() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let contract = gw.plan();

        // Stage 1: PLAN
        assert_eq!(contract.name, "测试任务");
        assert_eq!(contract.intent, "实现一个简单的功能。");
        assert_eq!(contract.must, vec!["所有错误必须返回 Result"]);
        assert_eq!(
            contract.must_not,
            vec!["禁止使用 `.unwrap()`", "禁止使用 `panic!`"]
        );
        assert_eq!(contract.decisions, vec!["使用 thiserror 处理错误类型"]);
        assert!(contract.forbidden.is_empty());
        assert_eq!(contract.out_of_scope, vec!["日志系统"]);
        assert_eq!(contract.completion_criteria.len(), 2);

        let contract_prompt = contract.to_prompt();
        assert!(contract_prompt.contains("# Task Contract: 测试任务"));
        assert!(contract_prompt.contains("## Must"));
        assert!(contract_prompt.contains("## Must NOT"));
        assert!(contract_prompt.contains("## Boundaries"));
        assert!(contract_prompt.contains(".unwrap()"));
        assert!(contract_prompt.contains("## Completion Criteria"));

        // Stage 2: GATE
        let lint = gw.lint();
        assert!(lint.quality_score.overall > 0.0);

        let json = contract.to_json();
        assert!(json.contains("测试任务"));
    }

    #[test]
    fn test_plan_returns_task_contract() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let plan = gw.plan();
        let contract = gw.contract();

        assert_eq!(plan.name, contract.name);
        assert_eq!(plan.must, contract.must);
        assert_eq!(plan.must_not, contract.must_not);
        assert_eq!(plan.decisions, contract.decisions);
        assert_eq!(plan.forbidden, contract.forbidden);
        assert_eq!(plan.completion_criteria.len(), 2);
        assert!(plan.to_prompt().contains("# Task Contract: 测试任务"));
    }

    #[test]
    fn test_quality_gate() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();

        // Very low threshold should pass
        assert!(gw.quality_gate(0.0).is_ok());

        // Very high threshold should fail
        let result = gw.quality_gate(1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_quality_gate_fails_on_error_lint_issue() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "缺少测试绑定"
---

## 完成条件

场景: 缺少 selector
  假设 存在一个任务规格
  当 质量闸门检查该规格
  那么 质量闸门应失败
"#,
        )
        .unwrap();

        let result = gw.quality_gate(0.0);
        assert!(result.is_err());
        let failure = result.unwrap_err();
        assert!(failure.to_string().contains("error-level lint issue"));
        assert!(failure.report.has_errors());
    }

    #[test]
    fn test_contract_prompt_format() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let prompt = gw.plan().to_prompt();

        // Should be structured with clear sections
        assert!(prompt.contains("## Intent"));
        assert!(prompt.contains("## Must"));
        assert!(prompt.contains("## Must NOT"));
        assert!(prompt.contains("## Decisions"));
        assert!(prompt.contains("## Boundaries"));
        assert!(prompt.contains("## Completion Criteria"));
        assert!(prompt.contains("Scenario: 正常路径"));
        assert!(prompt.contains("Scenario: 错误路径"));
    }

    #[test]
    fn test_skip_is_not_passing() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let report = VerificationReport {
            spec_name: "测试任务".into(),
            results: vec![spec_core::ScenarioResult {
                scenario_name: "未验证场景".into(),
                verdict: Verdict::Skip,
                step_results: vec![spec_core::StepVerdict {
                    step_text: "等待 verifier".into(),
                    verdict: Verdict::Skip,
                    reason: "no verifier covered this step".into(),
                }],
                evidence: vec![],
                duration_ms: 0,
            }],
            summary: spec_core::VerificationSummary {
                total: 1,
                passed: 0,
                failed: 0,
                skipped: 1,
                uncertain: 0,
            },
        };

        assert!(!gw.is_passing(&report));

        let summary = gw.failure_summary(&report);
        assert!(summary.contains("non-passing"));
        assert!(summary.contains("Skip"));
    }

    #[test]
    fn test_load_resolves_inherited_constraints_from_spec_directory() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gateway-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        let project_path = root.join("project.spec");
        fs::write(
            &project_path,
            r#"spec: project
name: "项目规则"
---

## 约束

### 禁止做
- 禁止使用 `panic!`
"#,
        )
        .unwrap();

        let task_path = root.join("task.spec");
        fs::write(
            &task_path,
            r#"spec: task
name: "任务"
inherits: project
---

## 意图

实现功能。

## 约束

### 必须做
- 返回 Result

## 验收标准

场景: 正常路径
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#,
        )
        .unwrap();

        let gw = SpecGateway::load(&task_path).unwrap();
        let prompt = gw.plan().to_prompt();
        let contract = gw.contract();

        assert!(prompt.contains("禁止使用 `panic!`"));
        assert!(prompt.contains("返回 Result"));
        assert!(
            contract
                .must_not
                .contains(&"禁止使用 `panic!`".to_string())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_load_resolves_full_project_contract_from_spec_directory() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gateway-full-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        let project_path = root.join("project.spec");
        fs::write(
            &project_path,
            r#"spec: project
name: "项目规则"
---

## 约束

### 必须做
- 所有公共 API 返回结构化错误

### 禁止做
- 禁止使用 `panic!`

## 已定决策

- 使用 `thiserror` 统一错误类型
"#,
        )
        .unwrap();

        let task_path = root.join("task.spec");
        fs::write(
            &task_path,
            r#"spec: task
name: "任务"
inherits: project
---

## 意图

实现功能。

## 已定决策

- 返回值保持现有 JSON 格式

## 验收标准

场景: 正常路径
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#,
        )
        .unwrap();

        let gw = SpecGateway::load(&task_path).unwrap();
        let contract = gw.plan();
        let prompt = contract.to_prompt();

        assert!(
            contract
                .must
                .contains(&"所有公共 API 返回结构化错误".to_string())
        );
        assert!(contract.must_not.contains(&"禁止使用 `panic!`".to_string()));
        assert!(
            contract
                .decisions
                .contains(&"使用 `thiserror` 统一错误类型".to_string())
        );
        assert!(
            contract
                .decisions
                .contains(&"返回值保持现有 JSON 格式".to_string())
        );
        assert!(prompt.contains("所有公共 API 返回结构化错误"));
        assert!(prompt.contains("使用 `thiserror` 统一错误类型"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_task_contract_keeps_must_must_not_and_decisions_distinct() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "Contract fidelity"
---

## 意图

修正 Task Contract 语义。

## 约束

### 必须做
- 所有公共函数返回 `Result`

### 禁止做
- 禁止使用 `panic!`

## 已定决策

- 使用 `thiserror` 统一错误类型

## 验收标准

场景: 正常路径
  假设 合同包含多类约束
  当 gateway 构造 Task Contract
  那么 不同约束保持独立
"#,
        )
        .unwrap();

        let contract = gw.plan();
        let prompt = contract.to_prompt();

        assert_eq!(contract.must, vec!["所有公共函数返回 `Result`"]);
        assert_eq!(contract.must_not, vec!["禁止使用 `panic!`"]);
        assert_eq!(contract.decisions, vec!["使用 `thiserror` 统一错误类型"]);
        assert!(prompt.contains("## Must"));
        assert!(prompt.contains("## Must NOT"));
        assert!(prompt.contains("## Decisions"));
        assert!(prompt.contains("所有公共函数返回 `Result`"));
        assert!(prompt.contains("禁止使用 `panic!`"));
        assert!(prompt.contains("使用 `thiserror` 统一错误类型"));
    }

    #[allow(deprecated)]
    #[test]
    fn test_legacy_brief_stays_derived_from_contract() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let brief = gw.brief();
        let contract = gw.plan();

        assert_eq!(brief.name, contract.name);
        assert_eq!(brief.must, contract.must);
        assert!(
            brief
                .must_not
                .contains(&"禁止使用 `.unwrap()`".to_string())
        );
        assert_eq!(brief.decided, contract.decisions);
        assert_eq!(
            brief.scenario_names.len(),
            contract.completion_criteria.len()
        );
    }

    #[test]
    fn test_pass_plus_skip_is_not_passing() {
        let gw = SpecGateway::from_input(SAMPLE).unwrap();
        let report = VerificationReport {
            spec_name: "测试任务".into(),
            results: vec![
                spec_core::ScenarioResult {
                    scenario_name: "[structural] 禁止使用 `panic!`".into(),
                    verdict: Verdict::Pass,
                    step_results: vec![spec_core::StepVerdict {
                        step_text: "禁止使用 `panic!`".into(),
                        verdict: Verdict::Pass,
                        reason: "no violations found".into(),
                    }],
                    evidence: vec![],
                    duration_ms: 0,
                },
                spec_core::ScenarioResult {
                    scenario_name: "未验证场景".into(),
                    verdict: Verdict::Skip,
                    step_results: vec![spec_core::StepVerdict {
                        step_text: "等待 verifier".into(),
                        verdict: Verdict::Skip,
                        reason: "no verifier covered this step".into(),
                    }],
                    evidence: vec![],
                    duration_ms: 0,
                },
            ],
            summary: spec_core::VerificationSummary {
                total: 2,
                passed: 1,
                failed: 0,
                skipped: 1,
                uncertain: 0,
            },
        };

        assert!(!gw.is_passing(&report));

        let summary = gw.failure_summary(&report);
        assert!(summary.contains("verdict: Skip"));
    }

    #[test]
    fn test_verify_with_ai_mode_stub_marks_uncovered_scenarios_uncertain() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI skeleton"
---

## 完成条件

场景: 需要 AI 判断
  假设 存在一个未被机械 verifier 覆盖的场景
  当 gateway 使用 stub 模式验证
  那么 返回 uncertain
"#,
        )
        .unwrap();

        let report = gw.verify_with_ai_mode(".", AiMode::Stub).unwrap();
        assert_eq!(report.summary.uncertain, 1);
        assert_eq!(report.summary.skipped, 0);
        assert_eq!(report.results[0].verdict, Verdict::Uncertain);
        assert!(matches!(
            report.results[0].evidence.first(),
            Some(spec_core::Evidence::AiAnalysis { .. })
        ));
    }

    #[test]
    fn test_verify_default_keeps_uncovered_scenarios_skipped() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI skeleton"
---

## 完成条件

场景: 需要 AI 判断
  假设 存在一个未被机械 verifier 覆盖的场景
  当 gateway 使用默认模式验证
  那么 返回 skip
"#,
        )
        .unwrap();

        let report = gw.verify(".").unwrap();
        assert_eq!(report.summary.uncertain, 0);
        assert_eq!(report.summary.skipped, 1);
        assert_eq!(report.results[0].verdict, Verdict::Skip);
        assert!(report.results[0].evidence.is_empty());
    }

    struct HostBackend;

    impl AiBackend for HostBackend {
        fn name(&self) -> &str {
            "host-backend"
        }

        fn analyze(&self, _request: &spec_core::AiRequest) -> SpecResult<spec_core::AiDecision> {
            Ok(spec_core::AiDecision {
                model: self.name().into(),
                confidence: 0.88,
                verdict: Verdict::Uncertain,
                reasoning: "host backend supplied the analysis".into(),
            })
        }
    }

    #[test]
    fn test_load_resolves_org_project_task_chain() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-gw-org-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("org.spec"),
            r#"spec: org
name: "组织规则"
---

## Constraints

- No hardcoded credentials
"#,
        )
        .unwrap();

        fs::write(
            root.join("project.spec"),
            r#"spec: project
name: "项目规则"
inherits: org
---

## Constraints

### Must
- All public APIs return structured errors

### Must NOT
- 禁止使用 `panic!`

## Decisions

- Use thiserror for error types
"#,
        )
        .unwrap();

        fs::write(
            root.join("task.spec"),
            r#"spec: task
name: "任务"
inherits: project
---

## Intent

实现功能。

## Completion Criteria

Scenario: happy path
  Given valid input
  When function is called
  Then returns Ok
"#,
        )
        .unwrap();

        let gw = SpecGateway::load(root.join("task.spec")).unwrap();
        let contract = gw.plan();

        // Org-level constraint inherited through project
        assert!(
            contract.must.iter().any(|c| c.contains("No hardcoded credentials")),
            "should inherit org-level constraint, got: {:?}",
            contract.must
        );

        // Project-level constraints
        assert!(
            contract
                .must
                .iter()
                .any(|c| c.contains("All public APIs return structured errors")),
            "should inherit project-level must"
        );
        assert!(
            contract
                .must_not
                .iter()
                .any(|c| c.contains("禁止使用 `panic!`")),
            "should inherit project-level must-not"
        );

        // Project-level decisions
        assert!(
            contract
                .decisions
                .iter()
                .any(|d| d.contains("Use thiserror")),
            "should inherit project-level decisions"
        );

        // Near-layer overrides: closer rules should appear
        let prompt = contract.to_prompt();
        assert!(prompt.contains("No hardcoded credentials"));
        assert!(prompt.contains("禁止使用 `panic!`"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_verify_with_injected_ai_backend_uses_host_backend() {
        let gw = SpecGateway::from_input(
            r#"spec: task
name: "AI host backend"
---

## 完成条件

场景: 交给宿主 backend
  假设 宿主 agent 提供了自定义 backend
  当 gateway 执行验证
  那么 返回 backend 的分析结果
"#,
        )
        .unwrap();

        let report = gw.verify_with_ai_backend(".", Arc::new(HostBackend)).unwrap();
        assert_eq!(report.summary.uncertain, 1);
        assert_eq!(report.results[0].verdict, Verdict::Uncertain);
        assert!(matches!(
            report.results[0].evidence.first(),
            Some(spec_core::Evidence::AiAnalysis {
                model,
                confidence,
                reasoning,
            }) if model == "host-backend" && (*confidence - 0.88).abs() < f64::EPSILON && reasoning == "host backend supplied the analysis"
        ));
    }
}
