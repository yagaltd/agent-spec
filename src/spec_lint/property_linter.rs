use crate::spec_core::{
    LintDiagnostic, Scenario, Section, Severity, Span, SpecDocument, StepKind,
};

use super::pipeline::SpecLinter;

/// Detects scenarios that look like property-based tests but may lack
/// a property framework binding. Warns if the scenario name suggests
/// property testing but the Test selector doesn't hint at a property framework.
pub struct PropertyTestLinter;

const PROPERTY_KEYWORDS: &[&str] = &[
    "for all",
    "for any",
    "any input",
    "any array",
    "any string",
    "any value",
    "any valid",
    "any sequence",
    "arbitrary",
    "property",
    "invariant",
    "round-trip",
    "roundtrip",
    "idempotent",
    "idempotency",
    "no crash",
    "no panic",
    "robustness",
    "order independent",
    "order-independent",
    "commutative",
    "symmetry",
    "shrinking",
    "always holds",
    "never crashes",
];

const PROPERTY_FRAMEWORK_HINTS: &[&str] = &[
    "fast-check",
    "proptest",
    "hypothesis",
    "rapid",
    "jqwik",
    "fc.assert",
    "fc.property",
    "fc.array",
    "fc.integer",
    "fc.string",
    "proptest!",
    "given_any",
    "for_all",
    "property_test",
    "arbtest",
];

impl SpecLinter for PropertyTestLinter {
    fn name(&self) -> &str {
        "property-test-strength"
    }

    fn lint(&self, doc: &SpecDocument) -> Vec<LintDiagnostic> {
        let mut diags = Vec::new();

        for section in &doc.sections {
            let scenarios = match section {
                Section::AcceptanceCriteria { scenarios, .. } => scenarios,
                _ => continue,
            };

            for scenario in scenarios {
                let looks_like_property = looks_like_property_test(scenario);

                if looks_like_property {
                    // Check if the test selector hints at a property framework
                    let has_framework_hint = scenario
                        .test_selector
                        .as_ref()
                        .map(|sel| {
                            let sel_lower = sel.filter.to_lowercase();
                            PROPERTY_FRAMEWORK_HINTS
                                .iter()
                                .any(|hint| sel_lower.contains(hint))
                        })
                        .unwrap_or(false);

                    // Also check step text for property framework keywords
                    let steps_hint_framework = scenario.steps.iter().any(|step| {
                        let text_lower = step.text.to_lowercase();
                        PROPERTY_FRAMEWORK_HINTS
                            .iter()
                            .any(|hint| text_lower.contains(hint))
                    });

                    if !has_framework_hint && !steps_hint_framework {
                        let scenario_name = &scenario.name;
                        diags.push(LintDiagnostic {
                            rule: "property-test-strength".into(),
                            severity: Severity::Warning,
                            message: format!(
                                "Scenario '{}' looks like a property-based test but has no property framework binding. \
                                 Consider using fast-check (TS), proptest (Rust), Hypothesis (Python), or rapid (Go).",
                                scenario_name
                            ),
                            span: scenario.span,
                            suggestion: Some(
                                "Add a property framework import and use property-based assertions instead of hardcoded examples.".into(),
                            ),
                        });
                    }
                }
            }
        }

        diags
    }
}

/// Check if a scenario looks like a property-based test
fn looks_like_property_test(scenario: &Scenario) -> bool {
    let name_lower = scenario.name.to_lowercase();

    // Check scenario name for property keywords
    for keyword in PROPERTY_KEYWORDS {
        if name_lower.contains(keyword) {
            return true;
        }
    }

    // Check step text for non-specific language ("any", "arbitrary", "all")
    let step_text: String = scenario
        .steps
        .iter()
        .map(|s| s.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let nonspecific_indicators = [
        "any valid",
        "any input",
        "any string",
        "any array",
        "any integer",
        "any sequence",
        "arbitrary input",
        "random input",
        "any byte",
        "any value",
        "for all",
    ];

    for indicator in &nonspecific_indicators {
        if step_text.contains(indicator) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_core::Step;

    fn make_scenario(name: &str, steps: Vec<(&str, &str)>, test_sel: Option<&str>) -> Scenario {
        Scenario {
            name: name.into(),
            steps: steps
                .into_iter()
                .map(|(kind, text)| Step {
                    kind: match kind {
                        "Given" => StepKind::Given,
                        "When" => StepKind::When,
                        "Then" => StepKind::Then,
                        _ => StepKind::Given,
                    },
                    text: text.into(),
                    params: vec![],
                    table: vec![],
                    span: Span::line(1),
                })
                .collect(),
            test_selector: test_sel.map(crate::spec_core::TestSelector::filter_only),
            tags: vec![],
            review: Default::default(),
            mode: Default::default(),
            depends_on: vec![],
            span: Span::line(1),
        }
    }

    #[test]
    fn detects_property_scenario_without_framework() {
        let scenario = make_scenario(
            "Sort is idempotent for all integer arrays",
            vec![
                ("Given", "any array of integers"),
                ("When", "sorted twice"),
                ("Then", "equals sorting once"),
            ],
            Some("test_sort_idempotent"),
        );
        let looks_property = looks_like_property_test(&scenario);
        assert!(looks_property, "should detect property test from name + steps");
    }

    #[test]
    fn no_warning_for_normal_scenario() {
        let scenario = make_scenario(
            "Calculate price with discount",
            vec![
                ("Given", "a base price of 100 and discount of 20"),
                ("When", "calculatePrice is called"),
                ("Then", "result is 80"),
            ],
            Some("test_calculate_price"),
        );
        let looks_property = looks_like_property_test(&scenario);
        assert!(!looks_property, "normal example test should not trigger");
    }

    #[test]
    fn no_warning_when_framework_present() {
        let scenario = make_scenario(
            "Sort is idempotent for all integer arrays",
            vec![
                ("Given", "any array of integers via fast-check"),
                ("When", "sorted twice"),
                ("Then", "equals sorting once"),
            ],
            Some("test_sort_idempotent_property"),
        );
        let looks_property = looks_like_property_test(&scenario);
        assert!(looks_property);
        // The steps contain "fast-check" so has_framework_hint should be true
        let steps_hint = scenario.steps.iter().any(|step| {
            let text_lower = step.text.to_lowercase();
            PROPERTY_FRAMEWORK_HINTS.iter().any(|hint| text_lower.contains(hint))
        });
        assert!(steps_hint, "step text should hint at property framework");
    }
}
