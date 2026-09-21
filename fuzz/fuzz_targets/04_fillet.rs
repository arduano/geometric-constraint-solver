//! Target 04 - fillet.
//!
//! Decode a FuzzInput and drive the fillet front-end: build the matrix fixture,
//! layer a computed Fillet feature on top of the base document through the
//! editor coordinator's feature-authoring path (activate the Fillet tool,
//! obtain a computed candidate at the fixture contact point, stage a preview,
//! and publish the fillet), then validate the solve diagnostics.
//!
//! A rejected operand set, a candidate with no preview, or a document that
//! never gains a published Fillet feature is a valid outcome. Only a success-like
//! solve on a document that gained a published Fillet feature is checked: reject
//! if the geometry is invalid, hard-validity is not Valid, the hard residuals
//! were not independently validated, or the maximum normalized hard residual
//! exceeds 1e-9.

#![no_main]

use fuzz::harness;
use fuzz::model::FuzzInput;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = FuzzInput::decode(data);
    if let Err(msg) = harness::run_fillet(&input) {
        panic!("fillet rejected: {msg}");
    }
});
