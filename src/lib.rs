//! agent-spec library surface.
//!
//! The crate was binary-only; this lib target exposes the (already
//! cleanly separated) spec modules for in-process use — e.g. CognitiveOS's
//! mechanical verifier calls `spec_parser::parse_spec_from_str` on contract
//! strings without a `.spec` file round-trip. The CLI (`src/main.rs`) is
//! unchanged behavior-wise: it now consumes this lib instead of declaring
//! the modules itself.

#![warn(clippy::all)]
#![deny(unsafe_code)]

pub mod spec_core;
pub mod spec_gateway;
pub mod spec_lint;
pub mod spec_parser;
pub mod spec_report;
pub mod spec_verify;
pub mod vcs;

/// Apply dependency skips: for each scenario with depends_on, if any dependency
/// has a non-pass verdict, override this scenario's verdict to Skip.
pub fn apply_dependency_skips(
    report: &mut crate::spec_core::VerificationReport,
    scenarios: &[crate::spec_core::Scenario],
) {
    use std::collections::HashMap;

    // Build name -> verdict map from current results (owned keys to avoid borrow conflict)
    let verdict_map: HashMap<String, crate::spec_core::Verdict> = report
        .results
        .iter()
        .map(|r| (r.scenario_name.clone(), r.verdict))
        .collect();

    // Build name -> depends_on map from scenarios (owned keys)
    let deps_map: HashMap<String, Vec<String>> = scenarios
        .iter()
        .filter(|s| !s.depends_on.is_empty())
        .map(|s| (s.name.clone(), s.depends_on.clone()))
        .collect();

    // For each result, check if any dependency failed
    for result in &mut report.results {
        if let Some(deps) = deps_map.get(&result.scenario_name) {
            let failed_deps: Vec<&str> = deps
                .iter()
                .filter(|dep| {
                    verdict_map
                        .get(dep.as_str())
                        .is_none_or(|v| *v != crate::spec_core::Verdict::Pass)
                })
                .map(|d| d.as_str())
                .collect();

            if !failed_deps.is_empty() {
                result.verdict = crate::spec_core::Verdict::Skip;
                let dep_names = failed_deps.join(", ");
                result
                    .evidence
                    .push(crate::spec_core::Evidence::PatternMatch {
                        pattern: "dependency-skip".into(),
                        matched: true,
                        locations: vec![format!("dependency failed: {dep_names}")],
                    });
            }
        }
    }

    // Recompute summary
    let total = report.results.len();
    let passed = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Pass)
        .count();
    let failed = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Fail)
        .count();
    let skipped = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Skip)
        .count();
    let uncertain = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::Uncertain)
        .count();
    let pending_review = report
        .results
        .iter()
        .filter(|r| r.verdict == crate::spec_core::Verdict::PendingReview)
        .count();
    report.summary = crate::spec_core::VerificationSummary {
        total,
        passed,
        failed,
        skipped,
        uncertain,
        pending_review,
    };
}

/// Sort scenarios by topological order based on depends_on.
/// Returns indices in execution order. Scenarios without dependencies preserve
/// their original order relative to each other.
#[allow(dead_code)]
pub fn topological_sort_scenarios(scenarios: &[crate::spec_core::Scenario]) -> Vec<usize> {
    use std::collections::{HashMap, VecDeque};

    let name_to_idx: HashMap<&str, usize> = scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.as_str(), i))
        .collect();

    // Build in-degree and adjacency
    let mut in_degree = vec![0usize; scenarios.len()];
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; scenarios.len()];

    for (i, s) in scenarios.iter().enumerate() {
        for dep in &s.depends_on {
            if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                in_degree[i] += 1;
                dependents[dep_idx].push(i);
            }
        }
    }

    // Kahn's algorithm with stable ordering
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut order = Vec::with_capacity(scenarios.len());
    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        let mut next: Vec<usize> = dependents[idx]
            .iter()
            .filter_map(|&dep_idx| {
                in_degree[dep_idx] -= 1;
                if in_degree[dep_idx] == 0 {
                    Some(dep_idx)
                } else {
                    None
                }
            })
            .collect();
        // Sort to preserve original order among siblings
        next.sort();
        for n in next {
            queue.push_back(n);
        }
    }

    order
}