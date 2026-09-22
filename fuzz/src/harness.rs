//! Shared harnesses for the four fuzz targets.

use crate::model::FuzzInput;
use geosolve_constraint_editor::{
    ComputedFeatureDefinition, FeatureAuthoringOutcome, FeatureAuthoringState,
    FeatureAuthoringTool, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    DesignPointId, DocumentSolveRequest, RetainedSketchDocumentSession, SketchHardValidity,
    SketchSolveDiagnostic, SolverConfig,
};
use geosolve_survey::{Family, FuzzVariant, fixture::MatrixFixture, survey::survey};

// ---------------------------------------------------------------------------
// Solver-config perturbation
// ---------------------------------------------------------------------------

/// FNV-1a over raw bytes: a cheap, order-sensitive mix used to seed a
/// deterministic, reproducible solver-config perturbation for one fuzz input.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Deterministic seed for this input's perturbed solver config: a hash of the
/// decoded input record, so the same input always maps to the same config and
/// the same solver result.
fn config_seed(input: &FuzzInput) -> u64 {
    let mut raw = [0u8; 64];
    let mut offset = 0usize;
    let put = |raw: &mut [u8], slot: &mut usize, bytes: &[u8]| {
        raw[*slot..*slot + bytes.len()].copy_from_slice(bytes);
        *slot += bytes.len();
    };
    put(&mut raw, &mut offset, &input.family.to_le_bytes());
    let translation_bytes = {
        let mut tb = [0u8; 16];
        tb[..8].copy_from_slice(&input.translation[0].to_le_bytes());
        tb[8..].copy_from_slice(&input.translation[1].to_le_bytes());
        tb
    };
    put(&mut raw, &mut offset, &translation_bytes);
    put(&mut raw, &mut offset, &input.scale.to_le_bytes());
    put(&mut raw, &mut offset, &input.rotation.to_le_bytes());
    put(
        &mut raw,
        &mut offset,
        &input.contact_parameter.to_le_bytes(),
    );
    put(
        &mut raw,
        &mut offset,
        &(input.reverse_spans as u8).to_le_bytes(),
    );
    put(
        &mut raw,
        &mut offset,
        &(input.swap_operands as u8).to_le_bytes(),
    );
    put(
        &mut raw,
        &mut offset,
        &(input.displaced as u8).to_le_bytes(),
    );
    put(&mut raw, &mut offset, &input.option_index.to_le_bytes());
    fnv1a(&raw[..offset])
}

/// A solver config perturbed deterministically from the input for this fuzz
/// target. The config stays within the solver's valid range and stresses the
/// numerics — iteration budget, residual/step/rank tolerances, and the
/// block-step cap — without ever producing an invalid policy. Damping values
/// are left at their defaults so the perturbation exercises convergence
/// *control* rather than destabilizing the Newton iteration itself.
pub fn perturbed_solver_config(input: &FuzzInput) -> SolverConfig {
    let seed = config_seed(input);
    // Iteration budget: 1..=1024. Low budgets force early rejection; high
    // budgets exercise the full Newton path.
    let max_iterations = 1 + (seed % 1024) as usize;
    // Residual tolerance: 1e-6..=1e-12, spanning the default 1e-9 and both
    // looser and tighter regimes.
    let residual_exp = 6 + ((seed >> 10) % 7) as i32; // 6..=12
    // Step tolerance: 1e-10..=1e-13.
    let step_exp = 10 + ((seed >> 20) % 4) as i32; // 10..=13
    // Rank tolerance: 1e-11..=1e-13, always tighter than residual tolerance.
    let rank_exp = 11 + ((seed >> 30) % 3) as i32; // 11..=13
    // Block-step cap: 0.05..=1.99.
    let block_step = 0.05 + ((seed >> 40) % 195) as f64 * 0.01;
    SolverConfig {
        max_iterations,
        normalized_residual_tolerance: 10.0_f64.powi(-residual_exp),
        normalized_step_tolerance: 10.0_f64.powi(-step_exp),
        rank_relative_tolerance: 10.0_f64.powi(-rank_exp),
        max_block_normalized_step: block_step,
        ..Default::default()
    }
}

/// The accepted document's point positions, keyed by point id.
type AcceptedPositions = Vec<(DesignPointId, [f64; 2])>;

/// The raw outcome of solving one matrix fixture: the latest solve diagnostic
/// plus the accepted document's point positions (empty if not accepted).
struct FixtureSolveOutcome {
    solve: SketchSolveDiagnostic,
    positions: AcceptedPositions,
}

/// Build the matrix fixture for one family + variant, solve it with a bare
/// session under the given solver config, and return the latest diagnostic plus
/// the accepted document's point positions. Returns None if the document could
/// not seed a session.
fn solve_fixture(
    family: Family,
    variant: FuzzVariant,
    config: &SolverConfig,
) -> Option<FixtureSolveOutcome> {
    let fixture = MatrixFixture::new(family, variant);
    let document = fixture.document().clone();
    let session =
        RetainedSketchDocumentSession::new(document, DocumentSolveRequest::default(), *config)
            .ok()?;
    let solve = session.latest_attempt_diagnostics().solve?;
    let positions = session
        .accepted_state()
        .map(|state| {
            state
                .document()
                .points()
                .iter()
                .map(|point| (point.id, point.position))
                .collect()
        })
        .unwrap_or_default();
    Some(FixtureSolveOutcome { solve, positions })
}

/// Solve the matrix fixture for one input under a perturbed solver config and
/// return its latest solve diagnostic (None if the document could not seed a
/// session). Deterministic: the same input always yields the same config and
/// diagnostic.
pub fn core_solve_diagnostic(input: &FuzzInput) -> Option<SketchSolveDiagnostic> {
    let variant = input.into_variant();
    let family = input.family();
    let config = perturbed_solver_config(input);
    let outcome = solve_fixture(family, variant, &config)?;
    Some(outcome.solve)
}

/// Solve the matrix fixture for one family + variant under the given solver
/// config, returning its latest solve diagnostic plus the accepted document's
/// point positions (empty if not accepted). Returns None if the document could
/// not seed a session. Used by target 02 to compare a base solve against its
/// `displaced`-perturbed neighbour under one identical config.
pub fn fixture_solve(
    family: Family,
    variant: FuzzVariant,
    config: &SolverConfig,
) -> Option<(SketchSolveDiagnostic, AcceptedPositions)> {
    let outcome = solve_fixture(family, variant, config)?;
    Some((outcome.solve, outcome.positions))
}

/// Run the survey() oracle for one input: decode, resolve family + variant,
/// call survey() and return its check() result. This is the single signal
/// for target 01.
///
/// check() returns Ok(()) for a rejected input or a success that passes every
/// independent residual validation, and Err(only) when the solver reports
/// success but the geometry is invalid — i.e. a genuine defect.
pub fn run_survey(input: &FuzzInput) -> Result<(), String> {
    let variant = input.into_variant();
    let outcome = survey(input.family(), variant);
    outcome.check()
}

/// Validate a converged core-solver result: a success-like solve must be
/// finite, hard-valid, independently residual-validated, and below the
/// residual bound the solver itself targeted; any violation is a defect.
///
/// `label` is an optional surface name; when non-empty the messages read
/// "`label`: accepted but ...", otherwise just "accepted but ...". Shared by
/// the core-solver, authoring, fillet, and random-document harnesses so a
/// success-like solve that fails any gate is caught identically everywhere.
fn validate_solve(
    config: &SolverConfig,
    solve: &SketchSolveDiagnostic,
    label: &str,
) -> Result<(), String> {
    let prefix = if label.is_empty() {
        String::new()
    } else {
        format!("{label}: ")
    };
    if solve.hard_validity != SketchHardValidity::Valid {
        return Err(format!(
            "{prefix}accepted but hard_validity = {:?}",
            solve.hard_validity
        ));
    }
    if !solve.hard_residuals_validated {
        return Err(format!(
            "{prefix}accepted but hard residuals were not independently validated"
        ));
    }
    let residual_bound = config.normalized_residual_tolerance;
    if let Some(residual) = solve.maximum_normalized_hard_residual
        && !(residual.is_finite() && residual <= residual_bound)
    {
        return Err(format!(
            "{prefix}accepted but maximum normalized hard residual {residual} exceeds the solver residual tolerance {residual_bound}"
        ));
    }
    Ok(())
}

/// Run the raw-session core-solver harness for one input: build the matrix
/// fixture, solve with a bare RetainedSketchDocumentSession (no authoring
/// coordinator), and validate the solve diagnostics. This is the single signal
/// for target 03.
///
/// A rejected or unresolved solve is a valid outcome (Ok). A success-like solve
/// must be finite, hard-valid, independently residual-validated, and below the
/// residual bound; any violation is a defect (Err).
pub fn run_core_solver(input: &FuzzInput) -> Result<(), String> {
    let Some(solve) = core_solve_diagnostic(input) else {
        return Ok(());
    };
    if !solve.accepted {
        return Ok(());
    }

    // The solver only reports accepted once it converged, so an accepted state
    // must be finite, hard-valid, independently residual-validated, and below
    // the residual tolerance the solver itself targeted. The bound is adaptive
    // to `normalized_residual_tolerance` so a relaxed tolerance admits a
    // correspondingly loose residual rather than flagging a false positive.
    let config = perturbed_solver_config(input);
    validate_solve(&config, &solve, "")
}

/// Solve an arbitrary document via the bare core-solver surface (target 03).
///
/// Builds a random `SketchDocument` (no authoring coordinator — just
/// `RetainedSketchDocumentSession`) from a fuzz input and validates its
/// diagnostics with the same accept-or-reject contract as the other surfaces,
/// labelled `"core-solver"`. Malformed documents are skipped, not crashes: a
/// decode/build failure or a session that cannot seed returns `Ok(())`.
pub fn run_random_core_solver(data: &[u8]) -> Result<(), String> {
    let document = match crate::random_doc::FuzzDocument::decode(data).build() {
        Some(document) => document,
        None => return Ok(()),
    };
    let config = SolverConfig::default();
    let session = match RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        config,
    ) {
        Ok(session) => session,
        Err(_) => return Ok(()),
    };
    let Some(solve) = session.latest_attempt_diagnostics().solve else {
        return Ok(());
    };
    validate_solve(&config, &solve, "core-solver")
}

/// Apply a deterministic neighbor perturbation to a variant's flags. Used by
/// target 02 to sweep flag neighbours of a decoded input.
pub fn perturb(mut variant: FuzzVariant) -> FuzzVariant {
    variant.displaced = !variant.displaced;
    variant
}

/// Apply a computed fillet at the fixture contact point through the editor
/// coordinator's feature-authoring path, then validate the same residual facts
/// as run_core_solver (target 04).
///
/// The fillet front-end layers a Fillet feature on top of the base document:
/// it activates the Fillet tool, obtains a computed candidate at the contact
/// point, stages a preview, and publishes the fillet. A rejected operand set,
/// a candidate with no preview, or a document that never gains a Fillet feature
/// is a valid outcome (`Ok(())`). Only a success-like solve on a document that
/// gained a *published* Fillet feature must be finite, hard-valid,
/// independently residual-validated, and below the residual bound; any
/// violation is a defect (`Err`).
pub fn run_fillet(input: &FuzzInput) -> Result<(), String> {
    let variant = input.into_variant();
    let family = input.family();

    let fixture = MatrixFixture::new(family, variant);
    let document = fixture.document().clone();

    let config = perturbed_solver_config(input);
    let mut coordinator = match RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(document, DocumentSolveRequest::default(), config)
            .map_err(|error| format!("failed to seed fillet session: {error}"))?,
    )
    .map_err(|error| format!("failed to build coordinator: {error}"))
    {
        Ok(coordinator) => coordinator,
        // A malformed base document is a bad input, not a solver defect.
        Err(_) => return Ok(()),
    };

    // Activate the Fillet tool and obtain a computed candidate at the contact
    // point. A candidate means the fixture gained a fillet-able corner.
    let contact = fixture.contact_point();
    let snapshot = match coordinator
        .feature_authoring_snapshot()
        .map_err(|error| format!("feature-authoring snapshot failed: {error}"))
    {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(()),
    };
    let mut state = FeatureAuthoringState::default();
    let candidate = match state.activate(
        &snapshot,
        snapshot.sketch_document(),
        FeatureAuthoringTool::Fillet,
        &[(SelectionItem::Point(contact), None)],
    ) {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => candidate,
        other => {
            eprintln!("[fillet] {family:?}: no contact candidate ({other:?})");
            return Ok(());
        }
    };

    // Stage the preview and publish the fillet feature.
    let identity = coordinator.feature_document().identity();
    let preview = match coordinator
        .prepare_feature_authoring_preview(identity, &candidate, "fuzz fillet")
        .map_err(|error| format!("fillet preview failed: {error}"))
    {
        Ok(preview) => preview,
        Err(_) => return Ok(()),
    };
    let mutation = match coordinator
        .apply_feature_authoring_preview(preview.token, &candidate)
        .map_err(|error| format!("fillet publication failed: {error}"))
    {
        Ok(mutation) => mutation,
        Err(_) => return Ok(()),
    };

    // Confirm the fillet feature actually published. A feature that is not a
    // FilletSet means the candidate did not materialize a fillet.
    let published = coordinator
        .feature_document()
        .feature(mutation.value)
        .is_some_and(|feature| {
            matches!(feature.definition, ComputedFeatureDefinition::FilletSet(_))
        });
    if !published {
        eprintln!("[fillet] {family:?}: feature published but is not a FilletSet");
        return Ok(());
    }

    // A fillet feature was published: validate the same residual facts as
    // run_core_solver.
    let solve = match coordinator.session().latest_attempt_diagnostics().solve {
        Some(solve) => solve,
        None => {
            return Err(format!(
                "{family:?}: fillet published but produced no solve diagnostics"
            ));
        }
    };

    validate_accepted(&family, &config, &solve)
}

/// Independent residual validation shared by the core-solver, fillet, and
/// fixture-perturbation harnesses. A success-like solve must be finite,
/// hard-valid, independently residual-validated, and below the residual bound
/// the solver itself targeted; any violation is a defect.
pub fn validate_accepted(
    family: &geosolve_survey::Family,
    config: &SolverConfig,
    solve: &SketchSolveDiagnostic,
) -> Result<(), String> {
    if !solve.accepted {
        return Ok(());
    }
    validate_solve(config, solve, &format!("{family:?} fillet"))
}

/// True if every f64 in the input transform is finite. Used by target 02 to
/// reject inputs that would make the fixture non-finite before solving.
pub fn geometry_finiteness(input: &FuzzInput) -> bool {
    input.translation.iter().all(|t| t.is_finite())
        && input.scale.is_finite()
        && input.rotation.is_finite()
        && input.contact_parameter.is_finite()
}
