//! Fuzz target: report formatters on a minimally-valid report built from
//! pseudo-random fields (deterministic splitmix from input bytes).

#![no_main]

use agent_spec::spec_core::{
    Evidence, ScenarioResult, StepVerdict, Verdict, VerificationReport, VerificationSummary,
};
use agent_spec::spec_report::{ExplainInput, OutputFormat, format_explain, format_verification};
use libfuzzer_sys::fuzz_target;

const ALL_FORMATS: [OutputFormat; 5] = [
    OutputFormat::Text,
    OutputFormat::Json,
    OutputFormat::Markdown,
    OutputFormat::Compact,
    OutputFormat::Diagnostic,
];

fn verdict_of(seed: u64) -> Verdict {
    match seed % 5 {
        0 => Verdict::Pass,
        1 => Verdict::Fail,
        2 => Verdict::Skip,
        3 => Verdict::Uncertain,
        _ => Verdict::PendingReview,
    }
}

/// splitmix64 — deterministic pseudo-random field material.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fuzz_target!(|data: &[u8]| {
    let mut state: u64 = u64::from_le_bytes(
        data.iter()
            .take(8)
            .chain(std::iter::repeat(&0u8).take(8))
            .take(8)
            .copied()
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or([0u8; 8]),
    );

    let scenario_count = (splitmix(&mut state) % 8) as usize;
    let mut results = Vec::with_capacity(scenario_count);
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut uncertain = 0usize;
    let mut pending = 0usize;

    for i in 0..scenario_count {
        let verdict = verdict_of(splitmix(&mut state));
        match verdict {
            Verdict::Pass => passed += 1,
            Verdict::Fail => failed += 1,
            Verdict::Skip => skipped += 1,
            Verdict::Uncertain => uncertain += 1,
            Verdict::PendingReview => pending += 1,
        }

        let step_count = (splitmix(&mut state) % 4) as usize;
        let step_results: Vec<StepVerdict> = (0..step_count)
            .map(|j| StepVerdict {
                step_text: format!("step {i}.{j}"),
                verdict,
                reason: String::new(),
            })
            .collect();

        let evidence = if splitmix(&mut state) % 2 == 0 {
            vec![Evidence::TestOutput {
                test_name: format!("test_{i}"),
                stdout: "fuzz\noutput".to_string(),
                passed: verdict == Verdict::Pass,
                package: None,
                level: None,
                test_double: None,
                targets: None,
            }]
        } else {
            Vec::new()
        };

        results.push(ScenarioResult {
            scenario_name: format!("scenario {i}"),
            verdict,
            step_results,
            evidence,
            duration_ms: (splitmix(&mut state) % 10_000) as u64,
        });
    }

    let report = VerificationReport {
        spec_name: "fuzz".to_string(),
        results,
        summary: VerificationSummary {
            total: scenario_count,
            passed,
            failed,
            skipped,
            uncertain,
            pending_review: pending,
        },
    };
    let input = ExplainInput {
        name: "fuzz".to_string(),
        intent: String::new(),
        must: Vec::new(),
        must_not: Vec::new(),
        decisions: Vec::new(),
        allowed_changes: Vec::new(),
        forbidden: Vec::new(),
        out_of_scope: Vec::new(),
    };

    for format in &ALL_FORMATS {
        let _ = format_verification(&report, format);
        let _ = format_explain(&input, &report, format);
    }
});
