//! Target 01: survey() oracle over the full FAMILIES x option_index matrix.
//!
//! Decodes a FuzzInput from the trailing 51 bytes, resolves the family +
//! variant, calls survey() and returns its check() result. A genuine defect
//! (survey rejecting a golden input, or an accepted-but-invalid input) surfaces
//! as an Err and fails the run.

#![no_main]

use fuzz::harness;
use fuzz::model::FuzzInput;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = FuzzInput::decode(data);
    if let Err(msg) = harness::run_survey(&input) {
        panic!("survey({}) rejected: {msg}", input.family().id());
    }
});
