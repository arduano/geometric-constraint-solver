//! Target 02 - fixture_perturbation.
//!
//! Decode a FuzzInput, solve the matrix fixture under a perturbed solver
//! config, then solve the `displaced`-perturbed neighbour under the SAME config
//! and compare. The perturbation is small (a few tenths of a fixture unit), so
//! a stable solver places the shared points at nearly the same world
//! coordinates. A shared point that moved far more than the perturbation
//! magnitude indicates the solver took a different solution branch — a defect
//! that a pure residual check would miss, because the flipped branch can still
//! be residual-valid.
//!
//! Reject if:
//!   * the perturbed solve is accepted but invalid/non-finite/invalid residuals;
//!   * either solve could not seed a session (a malformed input, not a defect);
//!   * both solves are accepted but a shared point moved more than the
//!     perturbation budget (a branch flip).
//!
//! A rejected solve is a valid outcome.

#![no_main]

use geosolve_sketch::DesignPointId;

use fuzz::harness;
use fuzz::model::FuzzInput;
use libfuzzer_sys::fuzz_target;

/// Perturbation budget for one shared point's world-space displacement. The
/// fixture perturbation shifts points by up to ~0.5 local units (<= ~0.5 *
/// `scale` world units); a branch flip moves a point by a macroscopic amount
/// (a large fraction of the ~10 * `scale` fixture). Budget = 4 * `scale` sits
/// comfortably between the two, with an 8x margin over the perturbation.
const PERTURBATION_BUDGET: f64 = 4.0;

/// Compare shared point positions between the base and perturbed solves.
///
/// Returns the base point with the largest displacement to its nearest
/// perturbed counterpart, if that displacement exceeds the perturbation budget.
///
/// Matching is by nearest world position, not by shared id. The `displaced`
/// perturbation is not always a pure id-preserving offset: for some families
/// (EndpointContinuity) it inserts a new point, which shifts every subsequent
/// id, and it can reorder the control list. A shared id can therefore refer to
/// a *different* physical point in the two documents — comparing by id then
/// reports a phantom branch flip (here `...9255` is bezier-2-end at `(4,-12)`
/// in the base but bezier-2-middle at `(2,-6)` in the perturbed document, yet
/// the solver moved neither point, both at residual 0.0). Nearest-position
/// matching compares each base point to the closest perturbed point, so it
/// tolerates the point set differing between documents while still flagging a
/// point with no nearby counterpart in the perturbed solve — the genuine
/// branch-flip signal.
fn branch_flip(
    base: &[(DesignPointId, [f64; 2])],
    perturbed: &[(DesignPointId, [f64; 2])],
    scale: f64,
) -> Option<(DesignPointId, f64)> {
    let budget = PERTURBATION_BUDGET * scale;
    let mut best: Option<(DesignPointId, f64)> = None;
    for (id, base_point) in base {
        let nearest = perturbed
            .iter()
            .map(|(_, p)| {
                let dx = p[0] - base_point[0];
                let dy = p[1] - base_point[1];
                (dx * dx + dy * dy).sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        if nearest > budget && best.is_none_or(|(_, d)| d < nearest) {
            best = Some((*id, nearest));
        }
    }
    best
}

fuzz_target!(|data: &[u8]| {
    let input = FuzzInput::decode(data);
    let family = input.family();
    let variant = input.into_variant();
    // One config for both solves: the only difference between the base and
    // perturbed geometry is the `displaced` perturbation, not the solver policy.
    let config = harness::perturbed_solver_config(&input);

    let (base_diagnostic, base_positions) = match harness::fixture_solve(family, variant, &config) {
        Some(value) => value,
        None => return,
    };
    let perturbed_variant = harness::perturb(variant);
    let (perturbed_diagnostic, perturbed_positions) =
        match harness::fixture_solve(family, perturbed_variant, &config) {
            Some(value) => value,
            None => return,
        };

    // The perturbed success must be finite, hard-valid, independently
    // residual-validated, and below the solver's residual tolerance.
    if let Err(msg) = harness::validate_accepted(&family, &config, &perturbed_diagnostic) {
        panic!("perturbed core-solve rejected: {msg}");
    }

    // Branch stability: if both solves are accepted, a shared point must not
    // move beyond the perturbation budget. A larger move means the perturbed
    // input flipped the solver onto a different solution branch.
    if base_diagnostic.accepted
        && perturbed_diagnostic.accepted
        && let Some((id, displacement)) =
            branch_flip(&base_positions, &perturbed_positions, variant.scale)
    {
        panic!(
            "{family:?}: branch flip - accepted point {id:?} moved {displacement} beyond the \
             {PERTURBATION_BUDGET}*scale perturbation budget"
        );
    }
});
