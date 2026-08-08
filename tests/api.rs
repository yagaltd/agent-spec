//! Public-API integration tests for the agent-spec library.
//!
//! These exercise the engine through the public API only (no CLI, no private
//! items): parse → AST, structural verify verdicts, lifecycle orchestration,
//! and dependency-skip / topological-ordering semantics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use agent_spec::spec_core::{ScenarioResult, VerificationReport};
use agent_spec::spec_gateway::{
    apply_dependency_skips, topological_sort_scenarios, SpecGateway,
};
use agent_spec::spec_parser::parse_spec_from_str;
use agent_spec::spec_verify::{
    run_verification, AiMode, StructuralVerifier, VerificationContext,
};

const SAMPLE_SPEC: &str = r#"spec: task
name: "refund"
tags: [payment]
---

## 意图

为支付网关添加退款功能。

## 约束

### 禁止

- 禁止使用 `panic!`

## 验收标准

场景: 退款成功
  测试: test_refund_ok
  假设 已支付订单
  当 发起全额退款
  那么 退款成功

场景: 退款拒绝
  前置: 退款成功
  测试: test_refund_rejected
  假设 订单已关闭
  当 发起退款
  那么 拒绝
"#;

/// Write a temp source file under a per-process unique dir; returns its path.
fn temp_file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent-spec-api-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

fn structural_verdict(gw: &SpecGateway, file: &PathBuf) -> agent_spec::spec_core::Verdict {
    let ctx = VerificationContext {
        code_paths: vec![file.clone()],
        change_paths: vec![file.clone()],
        ai_mode: AiMode::Off,
        resolved_spec: gw.resolved().clone(),
    };
    let report = run_verification(&ctx, &[&StructuralVerifier]).unwrap();
    let hits: Vec<&ScenarioResult> = report
        .results
        .iter()
        .filter(|r| r.scenario_name.starts_with("[structural]"))
        .collect();
    assert_eq!(hits.len(), 1, "expected exactly one structural scenario");
    hits[0].verdict
}

#[test]
fn parses_spec_into_ast_via_public_api() {
    let gw = SpecGateway::from_input(SAMPLE_SPEC).unwrap();
    let resolved = gw.resolved();
    assert_eq!(resolved.task.meta.name, "refund");
    assert_eq!(resolved.task.meta.tags, vec!["payment"]);

    let names: Vec<&str> = resolved
        .all_scenarios
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, vec!["退款成功", "退款拒绝"]);
    // Second scenario depends on the first — parser exposed the DepEdge.
    assert_eq!(resolved.all_scenarios[1].depends_on, vec!["退款成功"]);
}

#[test]
fn parses_spec_from_string_entry_point() {
    let doc = parse_spec_from_str(SAMPLE_SPEC).unwrap();
    assert_eq!(doc.meta.name, "refund");
}

#[test]
fn structural_verify_detects_violation_via_public_api() {
    let gw = SpecGateway::from_input(SAMPLE_SPEC).unwrap();
    let bad = temp_file("bad.rs", "fn f() { panic!(\"boom\"); }");
    assert_eq!(
        structural_verdict(&gw, &bad),
        agent_spec::spec_core::Verdict::Fail
    );
    let _ = fs::remove_file(&bad);
}

#[test]
fn structural_verify_passes_when_clean() {
    let gw = SpecGateway::from_input(SAMPLE_SPEC).unwrap();
    let clean = temp_file("ok.rs", "fn f() { let x = 1; }");
    assert_eq!(
        structural_verdict(&gw, &clean),
        agent_spec::spec_core::Verdict::Pass
    );
    let _ = fs::remove_file(&clean);
}

#[test]
fn gateway_lifecycle_orchestration_via_public_api() {
    let gw = SpecGateway::from_input(SAMPLE_SPEC).unwrap();

    // Lint layer through the gateway.
    let lint = gw.lint();
    assert_eq!(lint.spec_name, "refund");
    assert!(!lint.diagnostics.is_empty(), "lint should emit diagnostics");

    // Contract renderer.
    let contract = gw.contract();
    assert!(contract.intent.contains("退款"));

    // is_passing on a clean report vs a failing one.
    let pass_report = VerificationReport::from_results(
        "refund".into(),
        vec![ScenarioResult {
            scenario_name: "退款成功".into(),
            verdict: agent_spec::spec_core::Verdict::Pass,
            step_results: vec![],
            evidence: vec![],
            duration_ms: 0,
        }],
    );
    assert!(gw.is_passing(&pass_report));

    let fail_report = VerificationReport::from_results(
        "refund".into(),
        vec![ScenarioResult {
            scenario_name: "退款成功".into(),
            verdict: agent_spec::spec_core::Verdict::Fail,
            step_results: vec![],
            evidence: vec![],
            duration_ms: 0,
        }],
    );
    assert!(!gw.is_passing(&fail_report));
}

#[test]
fn dependency_skip_and_topological_order_via_public_api() {
    let gw = SpecGateway::from_input(SAMPLE_SPEC).unwrap();
    let scenarios = gw.resolved().all_scenarios.clone();

    // A fails, B passes → B must become Skip after dependency-skips applied.
    let mut report = VerificationReport::from_results(
        "refund".into(),
        vec![
            ScenarioResult {
                scenario_name: "退款成功".into(),
                verdict: agent_spec::spec_core::Verdict::Fail,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
            },
            ScenarioResult {
                scenario_name: "退款拒绝".into(),
                verdict: agent_spec::spec_core::Verdict::Pass,
                step_results: vec![],
                evidence: vec![],
                duration_ms: 0,
            },
        ],
    );
    apply_dependency_skips(&mut report, &scenarios);
    let b = report
        .results
        .iter()
        .find(|r| r.scenario_name == "退款拒绝")
        .unwrap();
    assert_eq!(b.verdict, agent_spec::spec_core::Verdict::Skip);
    assert_eq!(report.summary.skipped, 1);

    // Topological order keeps the dependency first.
    let order = topological_sort_scenarios(&scenarios);
    assert_eq!(order.len(), scenarios.len());
    let idx_a = scenarios.iter().position(|s| s.name == "退款成功").unwrap();
    let idx_b = scenarios.iter().position(|s| s.name == "退款拒绝").unwrap();
    let pos_a = order.iter().position(|&i| i == idx_a).unwrap();
    let pos_b = order.iter().position(|&i| i == idx_b).unwrap();
    assert!(pos_a < pos_b, "dependency must run before its dependent");
}
