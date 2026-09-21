//! Target 03 - core_solver.
//!
//! Decode a FuzzInput and drive the raw-session core solver: build the matrix
//! fixture, solve with a bare RetainedSketchDocumentSession (no authoring
//! coordinator), and validate the solve diagnostics. This is the lowest-level
//! solver surface and the most direct place a convergence/rank/residual defect
//! can hide.
//!
//! Reject if the solve is accepted but the geometry is invalid, hard-validity
//! is not Valid, the hard residuals were not independently validated, or the
//! maximum normalized hard residual exceeds 1e-9. A rejected or unresolved
//! solve is a valid outcome.

#![no_main]

use fuzz::harness;
use fuzz::model::FuzzInput;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = FuzzInput::decode(data);
    if let Err(msg) = harness::run_core_solver(&input) {
        panic!("core_solver rejected: {msg}");
    }
});
