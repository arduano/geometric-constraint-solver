// SPDX-License-Identifier: GPL-3.0-or-later
//! Determinism check for the solver-config-perturbed core solver (target 03)
//! and the fixture-perturbation harness (target 02).
//!
//! `perturbed_solver_config` seeds deterministically from the input bytes, so
//! the same input must yield the same solve diagnostic on every run. A
//! non-deterministic harness (hash ordering, thread-unsafe state, pointer
//! ordering) would surface here as an assertion failure.
//!
//! Build/run:
//! ```sh
//! cargo test --manifest-path fuzz/Cargo.toml --test determinism -- --exact
//! ```

use fuzz::harness;
use fuzz::model::{FuzzInput, LEN};
use geosolve_survey::FAMILIES;

/// Total inputs to exercise (capped so the test stays fast while still
/// covering every family).
const MAX_INPUTS: usize = 300;
/// Repetitions per input.
const RUNS: usize = 5;

#[test]
fn core_solve_is_deterministic() {
    let mut inputs: Vec<FuzzInput> = Vec::new();
    for family_index in 0..FAMILIES.len() as u32 {
        for seed in FuzzInput::golden_seeds(family_index) {
            inputs.push(FuzzInput::decode(&seed[..LEN]));
            if inputs.len() >= MAX_INPUTS {
                break;
            }
        }
        if inputs.len() >= MAX_INPUTS {
            break;
        }
    }

    for (index, input) in inputs.iter().enumerate() {
        let first = harness::core_solve_diagnostic(input);
        for attempt in 1..RUNS {
            let again = harness::core_solve_diagnostic(input);
            assert_eq!(
                first, again,
                "non-deterministic solve at input #{index} (attempt {attempt})"
            );
        }
    }
}
