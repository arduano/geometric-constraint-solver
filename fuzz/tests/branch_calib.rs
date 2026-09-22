//! Branch-flip budget calibration (Phase 2 of FUZZING-IMPROVEMENTS.md).
//!
//! Target 02 (`02_fixture_perturbation.rs`) flags a branch flip when a shared
//! point moves more than `PERTURBATION_BUDGET * scale` world units between the
//! base solve and the `displaced`-perturbed solve. That constant used to be an
//! unverified judgment ("8x margin"). This test *measures* the actual
//! displacement the perturbation produces across every family, a range of
//! scales (including the solver's magnitude extremes), and every perturbation
//! option, so the budget can be pinned to evidence instead of assumption.
//!
//! The signal is scale-invariant: the fixture scales uniformly with `scale`, so
//! `displacement / scale` is the quantity to track. We report its maximum over
//! the sweep and assert the budget sits comfortably above it.

use fuzz::harness;
use fuzz::model::FuzzInput;

use geosolve_sketch::DesignPointId;
use geosolve_survey::{FAMILIES, Family, FuzzVariant};

/// Scales sampled across the solver's normalized range (MIN_SCALE .. cap). The
/// fixture scales uniformly, so `displacement / scale` is scale-invariant; we
/// sweep the extremes to prove the constant holds there too.
const SCALES: [f64; 10] = [1e-3, 1e-2, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 1e3, 1e6];

/// Nearest-position displacement of the worst base point against the perturbed
/// point set — identical matching to the target 02 `branch_flip`, but reporting
/// the raw maximum rather than gating on the budget.
fn max_nearest_displacement(
    base: &[(DesignPointId, [f64; 2])],
    perturbed: &[(DesignPointId, [f64; 2])],
) -> f64 {
    base.iter()
        .map(|(_, base_point)| {
            perturbed
                .iter()
                .map(|(_, p)| {
                    let dx = p[0] - base_point[0];
                    let dy = p[1] - base_point[1];
                    (dx * dx + dy * dy).sqrt()
                })
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}

#[test]
fn measure_branch_flip_displacement() {
    let mut worst: Option<(Family, f64, u32, f64, f64)> = None; // family, scale, option, displacement, ratio
    let mut accepted_pairs = 0usize;
    let mut flagged = 0usize;
    let mut max_ratio = 0.0f64;
    let mut max_ratio_case = (0u32, 0.0f64, 0.0f64);

    for (family_index, family) in FAMILIES.iter().enumerate() {
        for scale in SCALES {
            for option_index in 0..24u32 {
                let base_variant = FuzzVariant {
                    scale,
                    option_index: option_index as u8,
                    ..FuzzVariant::DETERMINISTIC
                };
                let perturbed_variant = FuzzVariant {
                    scale,
                    option_index: option_index as u8,
                    displaced: true,
                    ..FuzzVariant::DETERMINISTIC
                };
                let base_bytes = FuzzInput::encode(family_index as u32, &base_variant);
                let base_input = FuzzInput::decode(&base_bytes);
                // Same config for both solves (only geometry differs), matching
                // the target's single-`config` setup.
                let config = harness::perturbed_solver_config(&base_input);

                let Some((base_diag, base_positions)) =
                    harness::fixture_solve(*family, base_variant, &config)
                else {
                    continue;
                };
                let Some((pert_diag, perturbed_positions)) =
                    harness::fixture_solve(*family, perturbed_variant, &config)
                else {
                    continue;
                };
                if !(base_diag.accepted && pert_diag.accepted) {
                    continue;
                }
                if base_positions.is_empty() || perturbed_positions.is_empty() {
                    continue;
                }

                accepted_pairs += 1;
                let displacement = max_nearest_displacement(&base_positions, &perturbed_positions);
                let ratio = displacement / scale;
                if ratio > max_ratio {
                    max_ratio = ratio;
                    max_ratio_case = (option_index, scale, displacement);
                    worst = Some((*family, scale, option_index, displacement, ratio));
                }
                // Count how often the *current* budget (4*scale) would fire, so
                // the calibration can show headroom.
                if displacement > 4.0 * scale {
                    flagged += 1;
                }
            }
        }
    }

    println!("branch-flip calibration: {accepted_pairs} accepted base+displaced pairs.");
    println!(
        "max displacement/scale = {:.4} (displacement {:.6}, scale {:.2}, option {})",
        max_ratio, max_ratio_case.2, max_ratio_case.1, max_ratio_case.0
    );
    if let Some((family, scale, option, displacement, ratio)) = worst {
        println!(
            "worst case: {family:?} scale={scale} option={option} displacement={displacement:.6} ratio={ratio:.4}"
        );
    }
    println!("displacements exceeding the current 4*scale budget: {flagged}");
    // The budget must clear the observed max stable displacement with margin.
    // The current constant is 4.0; require it to be at least 2x the worst
    // observed displacement/scale ratio (i.e. ratio <= 2.0), leaving room below
    // the macroscopic movement a genuine branch flip produces.
    assert!(
        4.0 >= 2.0 * max_ratio,
        "budget 4.0 is less than 2x the observed max ratio {max_ratio:.4}; re-evaluate the budget"
    );
}
