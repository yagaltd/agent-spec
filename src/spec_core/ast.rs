use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Spec hierarchy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpecLevel {
    Org,
    Project,
    Task,
}

/// Language used in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Zh,
    En,
}

/// Front-matter metadata of a spec file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecMeta {
    pub level: SpecLevel,
    pub name: String,
    pub inherits: Option<String>,
    pub lang: Vec<Lang>,
    pub tags: Vec<String>,
    /// Spec-level dependencies: names of other specs this spec depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    /// Estimated effort (e.g., "0.5d", "2d", "1w").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
}

/// A parsed .spec document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDocument {
    pub meta: SpecMeta,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

/// A top-level section in the spec body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Section {
    Intent {
        content: String,
        span: Span,
    },
    Constraints {
        items: Vec<Constraint>,
        span: Span,
    },
    Decisions {
        items: Vec<String>,
        span: Span,
    },
    Boundaries {
        items: Vec<Boundary>,
        span: Span,
    },
    AcceptanceCriteria {
        scenarios: Vec<Scenario>,
        span: Span,
    },
    OutOfScope {
        items: Vec<String>,
        span: Span,
    },
}

/// A single constraint line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub text: String,
    pub category: ConstraintCategory,
    pub span: Span,
}

/// Constraint categories matching the DSL sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintCategory {
    Must,
    MustNot,
    Decided,
    General,
}

/// A task boundary item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boundary {
    pub text: String,
    pub category: BoundaryCategory,
    pub span: Span,
}

/// Boundary categories for task contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryCategory {
    Allow,
    Deny,
    General,
}

/// Scenario execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioMode {
    #[default]
    Standard,
    Optimize,
}

impl ScenarioMode {
    pub fn is_standard(&self) -> bool {
        *self == Self::Standard
    }
}

/// Review mode for a scenario: whether it needs human review after passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ReviewMode {
    #[default]
    Auto,
    Human,
}

impl ReviewMode {
    pub fn is_auto(&self) -> bool {
        *self == Self::Auto
    }
}

/// A BDD scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_selector: Option<TestSelector>,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "ReviewMode::is_auto")]
    pub review: ReviewMode,
    #[serde(default, skip_serializing_if = "ScenarioMode::is_standard")]
    pub mode: ScenarioMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub span: Span,
}

impl Scenario {
    /// Returns `true` if this scenario is marked as critical — either via a
    /// `critical` tag or a `(critical)` / `（critical）` name suffix (case-insensitive).
    pub fn is_critical(&self) -> bool {
        let has_tag = self.tags.iter().any(|t| t.eq_ignore_ascii_case("critical"));
        if has_tag {
            return true;
        }
        let lower = self.name.to_lowercase();
        lower.ends_with("(critical)") || lower.ends_with("（critical）")
    }

    /// Returns the scenario name with any trailing `(critical)` / `（critical）`
    /// suffix stripped, suitable for display purposes.
    pub fn display_name(&self) -> &str {
        let name = self.name.trim_end();
        // Try ASCII parentheses first
        if let Some(idx) = name.rfind('(') {
            let suffix = &name[idx..];
            if suffix.to_lowercase() == "(critical)" {
                return name[..idx].trim_end();
            }
        }
        // Try fullwidth parentheses
        if let Some(idx) = name.rfind('（') {
            let suffix = &name[idx..];
            if suffix.to_lowercase() == "（critical）" {
                return name[..idx].trim_end();
            }
        }
        name
    }
}

/// Structured test selector for binding a scenario to test execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSelector {
    pub filter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_double: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<String>,
}

impl TestSelector {
    pub fn filter_only(filter: impl Into<String>) -> Self {
        Self {
            filter: filter.into(),
            package: None,
            level: None,
            test_double: None,
            targets: None,
        }
    }

    pub fn label(&self) -> String {
        match &self.package {
            Some(package) => format!("{package}::{}", self.filter),
            None => self.filter.clone(),
        }
    }
}

/// BDD step keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    Given,
    When,
    Then,
    And,
    But,
}

/// A single Given/When/Then step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub text: String,
    pub params: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table: Vec<Vec<String>>,
    pub span: Span,
}

/// Source location span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl Span {
    pub fn new(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Self {
        Self {
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    pub fn line(line: usize) -> Self {
        Self {
            start_line: line,
            end_line: line,
            start_col: 0,
            end_col: 0,
        }
    }
}

/// A resolved spec with inherited constraints merged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSpec {
    pub task: SpecDocument,
    pub inherited_constraints: Vec<Constraint>,
    pub inherited_decisions: Vec<String>,
    pub all_scenarios: Vec<Scenario>,
}
