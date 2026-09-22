//! Target 03 - core_solver (arbitrary documents).
//!
//! Solve arbitrary `SketchDocument`s via the bare core-solver surface: decode a
//! fuzz input into an arbitrary document (random points/curves/scalars), build
//! it, solve it with a bare RetainedSketchDocumentSession (no authoring
//! coordinator), and validate the solve diagnostics. This is target 03's
//! domain per `FUZZING.md §7` — arbitrary geometry over the bare core-solver
//! surface — and owns the convergence/rank/residual paths that the authoring-
//! oriented targets 01/04 do not exercise directly.
//!
//! Reject if the solve is accepted but the geometry is invalid, hard-validity
//! is not Valid, the hard residuals were not independently validated, or the
//! maximum normalized hard residual exceeds the solver residual tolerance.
//! Malformed documents are skipped (not crashes): a decode/build failure or a
//! session that cannot seed returns Ok, so these are bad inputs, not defects.

#![no_main]

use fuzz::harness;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Err(msg) = harness::run_random_core_solver(data) {
        panic!("core_solver rejected: {msg}");
    }
});
