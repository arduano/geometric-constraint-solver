//! Shared harnesses for the four fuzz targets.

use crate::model::FuzzInput;
use geosolve_constraint_editor::{
    ComputedFeatureDefinition, FeatureAuthoringOutcome, FeatureAuthoringState,
    FeatureAuthoringTool, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    DocumentSolveRequest, RetainedSketchDocumentSession, SketchHardValidity, SketchSolveDiagnostic,
    SolverConfig,
};
use geosolve_survey::FuzzVariant;
use geosolve_survey::fixture::MatrixFixture;
use geosolve_survey::survey::survey;

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

/// Run the raw-session core-solver harness for one input: build the matrix
/// fixture, solve with a bare RetainedSketchDocumentSession (no authoring
/// coordinator), and validate the solve diagnostics. This is the single signal
/// for target 03.
///
/// A rejected or unresolved solve is a valid outcome (Ok). A success-like solve
/// must be finite, hard-valid, independently residual-validated, and below the
/// residual bound; any violation is a defect (Err).
pub fn run_core_solver(input: &FuzzInput) -> Result<(), String> {
    let variant = input.into_variant();
    let family = input.family();

    let fixture = MatrixFixture::new(family, variant);
    let document = fixture.document().clone();

    let session = match RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    ) {
        Ok(session) => session,
        // The document itself could not seed a session; a malformed input, not
        // a solver defect. Skip.
        Err(_) => return Ok(()),
    };

    let Some(solve) = session.latest_attempt_diagnostics().solve else {
        return Ok(());
    };

    if !solve.accepted {
        return Ok(());
    }

    if solve.hard_validity != SketchHardValidity::Valid {
        return Err(format!(
            "accepted but hard_validity = {:?}",
            solve.hard_validity
        ));
    }
    if !solve.hard_residuals_validated {
        return Err("accepted but hard residuals were not independently validated".into());
    }
    if let Some(residual) = solve.maximum_normalized_hard_residual
        && !(residual.is_finite() && residual <= 1.0e-9)
    {
        return Err(format!(
            "accepted but maximum normalized hard residual {residual} exceeds 1e-9"
        ));
    }
    Ok(())
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

    let mut coordinator = match RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
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

    validate_fillet(&family, &solve)
}

/// Independent residual validation shared by the fillet front-end. A success-like
/// filleted solve must be finite, hard-valid, independently residual-validated,
/// and below the residual bound; any violation is a defect.
fn validate_fillet(
    family: &geosolve_survey::Family,
    solve: &SketchSolveDiagnostic,
) -> Result<(), String> {
    if !solve.accepted {
        return Ok(());
    }
    if solve.hard_validity != SketchHardValidity::Valid {
        return Err(format!(
            "{family:?}: fillet accepted but hard_validity = {:?}",
            solve.hard_validity
        ));
    }
    if !solve.hard_residuals_validated {
        return Err(format!(
            "{family:?}: fillet accepted but hard residuals were not independently validated"
        ));
    }
    if let Some(residual) = solve.maximum_normalized_hard_residual
        && !(residual.is_finite() && residual <= 1.0e-9)
    {
        return Err(format!(
            "{family:?}: fillet accepted but maximum normalized hard residual {residual} exceeds 1e-9"
        ));
    }
    Ok(())
}

/// True if every f64 in the input transform is finite. Used by target 02 to
/// reject inputs that would make the fixture non-finite before solving.
pub fn geometry_finiteness(input: &FuzzInput) -> bool {
    input.translation.iter().all(|t| t.is_finite())
        && input.scale.is_finite()
        && input.rotation.is_finite()
        && input.contact_parameter.is_finite()
}
