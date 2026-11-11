//! Fuzz target: arbitrary bytes through the spec parser.
//!
//! Exercises the bilingual keyword matchers, param extraction, and the
//! top-level `parse_spec_from_str` entry point on lossy-decoded input.

#![no_main]

use agent_spec::spec_parser::keywords::{
    extract_params, match_depends_field, match_mode_field, match_review_field,
    match_scenario_header, match_scenario_tags, match_section_header, match_step_keyword,
    match_test_selector, match_test_selector_field,
};
use agent_spec::spec_parser::parse_spec_from_str;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);

    // Individual matchers must be total.
    let _ = match_step_keyword(&input);
    let _ = match_section_header(&input);
    let _ = match_scenario_header(&input);
    let _ = match_test_selector(&input);
    let _ = match_scenario_tags(&input);
    let _ = match_test_selector_field(&input);
    let _ = match_review_field(&input);
    let _ = match_mode_field(&input);
    let _ = match_depends_field(&input);
    let _ = extract_params(&input);

    // Full parse must return a Result, never panic.
    let _ = parse_spec_from_str(&input);
});
