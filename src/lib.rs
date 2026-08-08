//! agent-spec library: the spec parse / lint / verify engine.
//!
//! The crate is split into a library (this file — the engine, importable by
//! other crates such as CognitiveOS v3) and a thin CLI binary (`src/main.rs`).
//! The CLI keeps byte-identical behavior for every subcommand; the engine
//! behind it is the public API below.
//!
//! Public API surface:
//! - [`spec_parser`] — `.spec` parsing to an AST ([`spec_core::Spec`]),
//!   string and file entry points, reference resolution.
//! - [`spec_lint`] — lint pipeline (quality smells, property linters).
//! - [`spec_verify`] — verification layers (lint/boundary/test/ai) and the
//!   `Verifier` trait.
//! - [`spec_gateway`] — lifecycle orchestration ([`SpecGateway`]),
//!   plan/brief generation.
//! - [`spec_report`] — output formatters (text/json/md).
//! - [`spec_core`] — shared AST, verdicts, reports.
//! - [`vcs`] — git/jj change-scope helpers used by `guard`.

// Mirrors the pre-split bin crate (main.rs had a crate-wide `#![allow(dead_code)]`):
// the engine retains internal helpers reachable only through test-gated re-exports
// (e.g. `spec_lint::cross_check` is `#[cfg(test)] pub use`-d). Keep this posture until
// those helpers gain real consumers, rather than hiding new dead code piecemeal.
#![allow(dead_code)]

pub mod spec_core;
pub mod spec_gateway;
pub mod spec_lint;
pub mod spec_parser;
pub mod spec_report;
pub mod spec_verify;
pub mod vcs;

// Re-exports for ergonomic `use agent_spec::{parse_spec, ...};` access.
pub use spec_gateway::SpecGateway;
pub use spec_parser::{parse_spec, parse_spec_from_str, resolve_spec};
pub use spec_verify::{run_verification, AiMode, Verifier};
