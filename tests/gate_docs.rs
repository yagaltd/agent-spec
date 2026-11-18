//! Pins for the docs pages that document gate commands.
//!
//! Docs drift silently: a page that stops matching the command it documents
//! is worse than no page. This pin asserts the *presence* of the documented
//! contract surface — the four validated sections, the gate option flags,
//! the ledger doctrine and real error shapes — in `docs/plan-check.md`.
//! The reviewer checks the wording against `cmd_plan_check` and its
//! validators in `src/main.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

const PAGE: &str = "docs/plan-check.md";

fn read_docs_page() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(PAGE);
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "docs page must exist and be readable: {} ({e})",
            path.display()
        )
    })
}

#[test]
fn plan_check_docs_page_documents_sections() {
    let docs = read_docs_page();

    // All four validated sections are documented by name.
    for section in ["Dependencies", "Ledger Resolution", "Bottleneck", "Sieve"] {
        assert!(
            docs.contains(section),
            "docs must document the {section} section by name"
        );
    }

    // The gate option flags appear. plan-check itself takes only a positional
    // plan path and --format; --spec-dir/--code/--change/--change-scope/
    // --min-score/--cache belong to the repo-level gates (guard, lifecycle,
    // verify, lint) that the docs page cross-references truthfully.
    for flag in [
        "--spec-dir",
        "--code",
        "--change",
        "--change-scope",
        "--min-score",
        "--cache",
    ] {
        assert!(docs.contains(flag), "docs must mention the {flag} flag");
    }

    // The ledger doctrine: parked ideas are ledger rows, never plans.
    assert!(
        docs.contains("parked ideas are ledger rows"),
        "docs must state the parked-ideas-are-ledger-rows rule"
    );

    // Real error shapes, quoted from the validators — not paraphrases.
    for error_shape in [
        "invalid Outcome",
        "expected one of: propose, build-direct",
        "expected done(<ref>), closed(<reason>) or deferred(trigger: <condition>)",
        "Unknown bottleneck tag",
        "Circular dependency detected",
        "not found in spec map",
    ] {
        assert!(
            docs.contains(error_shape),
            "docs must quote the real error shape: {error_shape}"
        );
    }
}
