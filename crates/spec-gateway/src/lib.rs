#![warn(clippy::all)]
#![deny(unsafe_code)]

//! spec-gateway: Embeddable facade for agent-spec.
//!
//! This crate provides a single-call API for AI agents to:
//! 1. Load and lint a spec before coding starts
//! 2. Verify code against a spec after coding completes
//! 3. Get structured JSON results for decision-making
//!
//! # Agent Lifecycle Integration
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Agent Task Lifecycle                        │
//! │                                                               │
//! │  1. PLAN    ─── load_spec()      → TaskContract (context)     │
//! │  2. GATE    ─── lint_spec()      → QualityScore (go/no-go)    │
//! │  3. CODE    ─── (agent writes code)                           │
//! │  4. VERIFY  ─── verify_spec()    → VerificationReport         │
//! │  5. DECIDE  ─── is_passing()     → bool (merge or retry)      │
//! │                                                               │
//! │  If FAIL:   ─── failure_summary()→ String (retry prompt)      │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod brief;
mod lifecycle;

#[allow(deprecated)]
pub use brief::SpecBrief;
pub use brief::TaskContract;
pub use lifecycle::SpecGateway;
pub use spec_verify::{AiBackend, AiMode};
