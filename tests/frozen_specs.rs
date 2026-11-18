//! Pins for the frozen-spec carve-out decision (pi-workflows plan
//! 20261117-068, task 2, decided 2026-11-17).
//!
//! The `boundaries-path` lint flags 11 historical specs whose scenarios cite
//! example paths outside their own Allowed Changes (16 findings total; the
//! flagged set is frozen history — enumerated 2026-11-17 with per-file
//! `cargo run -- lint` runs, because a combined run silently stops at
//! `specs/roadmap/README.md`, which has no front-matter). Those specs are
//! marked, not rewritten: see `docs/frozen-specs.md` for the semantics.
//!
//! The list below is hardcoded on purpose: the flagged set does not grow on
//! its own, and un-flagging a spec (rewriting its citations) must be a
//! deliberate edit to this list, not a silent test relaxation.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

/// The frozen marker every flagged spec carries in its frontmatter:
/// date + plan id of the decision.
const FROZEN_MARKER: &str = "frozen: 20261117 plan-20261117-068";

/// Specs the `boundaries-path` rule flags, enumerated 2026-11-17
/// (plan-20261117-068 task 2).
const FROZEN_SPECS: &[&str] = &[
    "specs/task-add-behavior-completeness-linters.spec.md",
    "specs/task-derive-change-set-from-staged-git-index.spec.md",
    "specs/task-enforce-boundaries-with-explicit-change-set.spec.md",
    "specs/task-phase2-run-history-and-vcs-context.spec.md",
    "specs/task-support-change-scope-in-verify-and-lifecycle.spec.md",
    "specs/task-support-git-worktree-change-scope.spec.md",
    "specs/task-support-spec-md-extension.spec.md",
    "specs/task-verification-cache.spec.md",
    "specs/roadmap/task-phase2-run-history-and-vcs-context.spec.md",
    "specs/roadmap/task-plan-command.spec.md",
    "specs/roadmap/task-strengthen-rewrite-contract-authoring.spec.md",
];

#[test]
fn test_frozen_specs_are_marked() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for rel in FROZEN_SPECS {
        let path = root.join(rel);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("frozen spec unreadable: {rel}: {e}"));
        assert!(
            text.contains(FROZEN_MARKER),
            "frozen marker `{FROZEN_MARKER}` missing from {rel}"
        );
    }
}
