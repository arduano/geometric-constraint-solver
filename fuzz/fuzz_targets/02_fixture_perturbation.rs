//! Target 02 - fixture_perturbation.
//!
//! Decode a FuzzInput, apply a deterministic flag perturbation (flip
//! `displaced`), and re-run the core-solver harness on the perturbed input.
//! This exercises solver behaviour at neighbour inputs around a decoded point:
//! a defect surfaces when a perturbed input flips a branch and the solver
//! reports a success that fails residual validation.
//!
//! Reject if the perturbed solve is accepted but invalid/non-finite/invalid
//! residuals. A rejected perturbed input is a valid outcome.

#![no_main]

use fuzz::harness;
use fuzz::model::FuzzInput;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = FuzzInput::decode(data);
    let perturbed = input.with_displaced(!input.displaced);
    if let Err(msg) = harness::run_core_solver(&perturbed) {
        panic!("perturbed core-solve rejected: {msg}");
    }
});
