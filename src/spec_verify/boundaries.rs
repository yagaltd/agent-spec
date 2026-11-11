use std::path::{Path, PathBuf};

use crate::spec_core::{
    BoundaryCategory, Evidence, ScenarioResult, SpecResult, StepVerdict, Verdict,
};

use super::{VerificationContext, Verifier};

/// Mechanical verifier for file-level task boundaries.
///
/// This verifier only runs when callers provide an explicit `change_paths` set.
/// It does not infer diffs from VCS state.
pub struct BoundariesVerifier;

impl Verifier for BoundariesVerifier {
    fn name(&self) -> &str {
        "boundaries"
    }

    fn verify(&self, ctx: &VerificationContext) -> SpecResult<Vec<ScenarioResult>> {
        let (allowed, forbidden) = collect_path_boundaries(&ctx.resolved_spec.task.sections);
        if ctx.change_paths.is_empty() || (allowed.is_empty() && forbidden.is_empty()) {
            return Ok(Vec::new());
        }

        let workspace_root =
            find_workspace_root(&ctx.code_paths).or_else(|| find_workspace_root(&ctx.change_paths));
        let changes = normalize_change_paths(&ctx.change_paths, workspace_root.as_deref());
        if changes.is_empty() {
            return Ok(Vec::new());
        }

        let mut step_results = Vec::new();
        let mut evidence = Vec::new();
        let mut has_failure = false;

        for change in changes {
            if let Some(pattern) = forbidden
                .iter()
                .find(|pattern| path_matches_pattern(pattern, &change))
            {
                has_failure = true;
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Fail,
                    reason: format!("matches forbidden boundary `{pattern}`"),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: pattern.clone(),
                    matched: true,
                    locations: vec![change],
                });
                continue;
            }

            if let Some(pattern) = allowed
                .iter()
                .find(|pattern| path_matches_pattern(pattern, &change))
            {
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Pass,
                    reason: format!("matches allowed boundary `{pattern}`"),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: pattern.clone(),
                    matched: true,
                    locations: vec![change],
                });
                continue;
            }

            if !allowed.is_empty() {
                has_failure = true;
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Fail,
                    reason: "not covered by any allowed boundary".into(),
                });
                evidence.push(Evidence::PatternMatch {
                    pattern: "<allowed-boundaries>".into(),
                    matched: false,
                    locations: vec![change],
                });
            } else {
                step_results.push(StepVerdict {
                    step_text: change.clone(),
                    verdict: Verdict::Pass,
                    reason: "no allow-list declared; change accepted because it is not forbidden"
                        .into(),
                });
            }
        }

        Ok(vec![ScenarioResult {
            scenario_name: "[boundaries] explicit change set respects declared paths".into(),
            verdict: if has_failure {
                Verdict::Fail
            } else {
                Verdict::Pass
            },
            step_results,
            evidence,
            duration_ms: 0,
        }])
    }
}

fn collect_path_boundaries(sections: &[crate::spec_core::Section]) -> (Vec<String>, Vec<String>) {
    let mut allowed = Vec::new();
    let mut forbidden = Vec::new();

    for section in sections {
        if let crate::spec_core::Section::Boundaries { items, .. } = section {
            for item in items {
                if !looks_like_path_boundary(&item.text) {
                    continue;
                }

                match item.category {
                    BoundaryCategory::Allow => allowed.push(normalize_pattern(&item.text)),
                    BoundaryCategory::Deny | BoundaryCategory::General => {
                        forbidden.push(normalize_pattern(&item.text))
                    }
                }
            }
        }
    }

    (allowed, forbidden)
}

pub(crate) fn looks_like_path_boundary(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('*')
        || trimmed.ends_with(".rs")
        || trimmed.ends_with(".ts")
        || trimmed.ends_with(".js")
        || trimmed.ends_with(".py")
        || trimmed.ends_with(".spec")
        || trimmed.ends_with(".spec.md")
}

fn normalize_change_paths(paths: &[PathBuf], workspace_root: Option<&Path>) -> Vec<String> {
    let mut changes = Vec::new();

    for path in paths {
        let normalized = normalize_path(path, workspace_root);
        if !normalized.is_empty() && !changes.iter().any(|item| item == &normalized) {
            changes.push(normalized);
        }
    }

    changes
}

fn normalize_path(path: &Path, workspace_root: Option<&Path>) -> String {
    let candidate = workspace_root
        .and_then(|root| strip_root_prefix(path, root))
        .unwrap_or_else(|| path.to_path_buf());

    candidate
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

/// Strip `root` from `path`, comparing canonical forms when the two live in
/// different frames — e.g. guard's relative `.` code root against the ABSOLUTE
/// change paths git worktree scopes produce. A bare `strip_prefix` can never
/// bridge that mismatch and would leak `home/aurel/...` into the change set.
fn strip_root_prefix(path: &Path, root: &Path) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }

    let canonical_root = root.canonicalize().ok()?;
    if path.is_absolute() {
        path.strip_prefix(&canonical_root)
            .ok()
            .map(Path::to_path_buf)
    } else {
        let absolute = std::env::current_dir().ok()?.join(path);
        absolute
            .strip_prefix(&canonical_root)
            .ok()
            .map(Path::to_path_buf)
    }
}

/// Normalize a boundary bullet into a path glob pattern.
///
/// Decorated bullets (`` - `src/**` — description ``) must yield only the
/// backticked span; trimming backticks at the ends alone would keep the
/// description inside the pattern. The first span wins when several exist.
pub(crate) fn normalize_pattern(pattern: &str) -> String {
    let base = match pattern.find('`') {
        Some(start) => match pattern[start + 1..].find('`') {
            Some(end) => &pattern[start + 1..start + 1 + end],
            // Unbalanced backtick: keep the legacy end-trimming behavior.
            None => pattern,
        },
        None => pattern,
    };

    base.trim()
        .trim_matches('`')
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string()
}

pub(crate) fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let path_segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    match_segments(&pattern_segments, &path_segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        return (0..=path.len()).any(|index| match_segments(&pattern[1..], &path[index..]));
    }

    if path.is_empty() {
        return false;
    }

    segment_matches(pattern[0], path[0]) && match_segments(&pattern[1..], &path[1..])
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return pattern == segment;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut cursor = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if index == 0 && anchored_start {
            if !segment[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }

        if let Some(found) = segment[cursor..].find(part) {
            cursor += found + part.len();
        } else {
            return false;
        }
    }

    if anchored_end && let Some(last_part) = parts.iter().rev().find(|part| !part.is_empty()) {
        return segment.ends_with(last_part);
    }

    true
}

fn find_workspace_root(paths: &[PathBuf]) -> Option<PathBuf> {
    for path in paths {
        let mut current = if path.is_file() {
            path.parent()?.to_path_buf()
        } else {
            path.clone()
        };

        loop {
            // Git WORKTREES carry `.git` as a FILE (gitdir pointer), regular
            // repos as a directory — accept both. `Cargo.toml` still selects
            // Rust workspace roots; first ancestor match wins.
            if current.join(".git").exists() || current.join("Cargo.toml").is_file() {
                return Some(current);
            }
            if !current.pop() {
                break;
            }
        }
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::spec_core::{ResolvedSpec, Scenario, SpecLevel, SpecMeta, SpecResult, Verdict};

    use super::{BoundariesVerifier, path_matches_pattern};
    use crate::spec_verify::{AiMode, VerificationContext, Verifier};

    fn make_resolved_spec(input: &str) -> SpecResult<ResolvedSpec> {
        let doc = crate::spec_parser::parse_spec_from_str(input)?;
        Ok(ResolvedSpec {
            task: doc,
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::<Scenario>::new(),
        })
    }

    #[test]
    fn matches_double_star_path_patterns() {
        assert!(path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-parser/src/parser.rs"
        ));
        assert!(path_matches_pattern("specs/**", "specs/task.spec"));
        assert!(!path_matches_pattern(
            "crates/spec-parser/**",
            "crates/spec-gateway/src/lib.rs"
        ));
    }

    #[test]
    fn test_boundaries_verifier_accepts_changes_within_allowed_paths() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## 边界

### 允许修改
- crates/spec-parser/**

### 禁止做
- crates/spec-gateway/**
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-parser/src/parser.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Pass);
    }

    #[test]
    fn test_boundaries_verifier_rejects_change_outside_allowed_paths() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## Boundaries

### Allowed Changes
- crates/spec-parser/**
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-gateway/src/lifecycle.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Fail);
        assert!(
            results[0].step_results[0]
                .reason
                .contains("not covered by any allowed boundary")
        );
    }

    #[test]
    fn test_boundaries_verifier_rejects_change_matching_forbidden_boundary() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "边界"
---

## 边界

### 允许修改
- crates/spec-gateway/**

### 禁止做
- crates/spec-gateway/src/lib.rs
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("crates/spec-gateway/src/lib.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Fail);
        assert!(
            results[0].step_results[0]
                .reason
                .contains("matches forbidden boundary")
        );
    }

    // ---- regression: git WORKTREES carry `.git` as a FILE, not a dir ----

    fn temp_root(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("agent-spec-wsroot-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn normalize_path_strips_absolute_paths_against_relative_root() {
        // guard runs `--code .` (relative root) but receives ABSOLUTE worktree
        // change paths; only the canonical frames can bridge that mismatch.
        let cwd = std::env::current_dir().unwrap();
        let absolute_change = cwd.join("Cargo.toml");
        assert_eq!(
            super::normalize_path(&absolute_change, Some(Path::new("."))),
            "Cargo.toml"
        );
    }

    #[test]
    fn find_workspace_root_detects_git_file_marker_without_cargo_toml() {
        let root = temp_root("git-file");
        std::fs::create_dir_all(root.join("a/b/repo/src")).unwrap();
        std::fs::write(root.join("a/b/repo/.git"), "gitdir: /elsewhere").unwrap();
        std::fs::write(root.join("a/b/repo/src/sort.rs"), "fn main() {}").unwrap();

        let detected = super::find_workspace_root(&[root.join("a/b/repo/src/sort.rs")]);
        assert_eq!(detected.as_deref(), Some(root.join("a/b/repo").as_path()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn normalize_change_paths_makes_git_file_root_changes_repo_relative() {
        let root = temp_root("git-file-rel");
        std::fs::create_dir_all(root.join("repo/src")).unwrap();
        std::fs::write(root.join("repo/.git"), "gitdir: /elsewhere").unwrap();
        std::fs::write(root.join("repo/src/sort.rs"), "fn main() {}").unwrap();

        let change = root.join("repo/src/sort.rs");
        let workspace_root = super::find_workspace_root(std::slice::from_ref(&change));
        let changes =
            super::normalize_change_paths(std::slice::from_ref(&change), workspace_root.as_deref());

        assert_eq!(changes, vec!["src/sort.rs".to_string()]);
        assert!(
            !changes[0].contains("home/aurel"),
            "absolute path leaked into change set: {}",
            changes[0]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- regression: decorated bullets keep the description inside the pattern ----

    #[test]
    fn normalize_pattern_extracts_first_backtick_span() {
        assert_eq!(
            super::normalize_pattern("`src/**` — all source changes"),
            "src/**"
        );
        assert_eq!(super::normalize_pattern("src/**"), "src/**");
        assert_eq!(super::normalize_pattern("`a/b` x `c`"), "a/b");
    }

    #[test]
    fn decorated_backtick_bullet_is_recognized_by_boundaries_verifier() {
        let resolved = make_resolved_spec(
            r#"spec: task
name: "probe"
---

## Boundaries

### Allowed Changes
- `src/**` — all source changes
"#,
        )
        .unwrap();

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: vec![PathBuf::from("src/sort.rs")],
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verdict, Verdict::Pass);
    }

    #[test]
    fn verifier_skips_when_no_explicit_change_paths_are_provided() {
        let resolved = ResolvedSpec {
            task: crate::spec_core::SpecDocument {
                meta: SpecMeta {
                    level: SpecLevel::Task,
                    name: "边界".into(),
                    inherits: None,
                    lang: vec![],
                    tags: vec![],
                    depends: vec![],
                    estimate: None,
                },
                sections: vec![],
                source_path: PathBuf::new(),
            },
            inherited_constraints: Vec::new(),
            inherited_decisions: Vec::new(),
            all_scenarios: Vec::new(),
        };

        let verifier = BoundariesVerifier;
        let results = verifier
            .verify(&VerificationContext {
                code_paths: vec![PathBuf::from(".")],
                change_paths: Vec::new(),
                ai_mode: AiMode::Off,
                resolved_spec: resolved,
            })
            .unwrap();

        assert!(results.is_empty());
    }
}
