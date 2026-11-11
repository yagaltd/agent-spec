//! Fuzz target: external tool output parsers (tdd-guard JSON, bun TAP-ish
//! output, git numstat, complexity ratio extraction).

#![no_main]

use agent_spec::spec_verify::bun_test_verifier::{BunOutcome, parse_bun_output_for_test};
use agent_spec::spec_verify::complexity::{
    parse_first_number, parse_line_ratio, parse_numstat_output, parse_trailing_number,
};
use agent_spec::spec_verify::tdd_guard_verifier::parse_tdd_guard_output;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let output = String::from_utf8_lossy(data);

    // tdd-guard JSON parse: must be total.
    let _result = parse_tdd_guard_output(&output);

    // bun output parse + matching against an arbitrary test name.
    let test_name = output.lines().next().unwrap_or("fuzz_test");
    match parse_bun_output_for_test(&output, test_name) {
        BunOutcome::Passed | BunOutcome::Failed | BunOutcome::NotFound => {}
    }

    // complexity / numstat parsers: must be total.
    let _ = parse_line_ratio(&output);
    let _ = parse_trailing_number(&output);
    let _ = parse_first_number(&output);
    let _ = parse_numstat_output(&output);
});
