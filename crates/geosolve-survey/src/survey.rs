// SPDX-License-Identifier: GPL-3.0-or-later
//! The residual-checked survey driver over `geosolve-constraint-editor`.
//!
//! A survey builds the [`MatrixFixture`] for a [`Family`] and [`FuzzVariant`],
//! drives the retained editor coordinator to author the subject, and hands the
//! solver diagnostics to [`SurveyOutcome::check`]. The check only trusts a
//! success-like status after independent residual validation: the accepted
//! state must be current, finite, hard-valid, and below the residual bound.
//!
//! This is the surface the fuzzing front-ends perturb. It mirrors the golden
//! authoring oracle's `survey_constraint`/`survey_dimension` core without the
//! full parity-manifest and definition-reconstruction validation, which the
//! golden test still owns.

use geosolve_constraint_editor::{
    AuthoringMutation, AuthoringOptions, AuthoringOutcome, AuthoringState, AuthoringTool,
    RetainedEditorCoordinator,
};
use geosolve_sketch::{
    DocumentAngleOrientation, DocumentDimensionMode, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SketchHardValidity, SolverConfig,
};

use crate::fixture::{MatrixFixture, constraint_operands, dimension_operands};
use crate::{Family, FuzzVariant};

/// Raw residual facts captured from one survey run.
///
/// A survey may legitimately fail to converge (rejected operands, unsatisfiable
/// subject); [`SurveyOutcome::check`] only treats such inputs as defects when
/// the solver reported success but independent validation disagrees.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct SurveyOutcome {
    pub family: Family,
    pub variant: FuzzVariant,
    pub accepted: bool,
    pub hard_validity: Option<SketchHardValidity>,
    pub hard_residuals_validated: bool,
    pub max_residual: Option<f64>,
    pub max_coordinate: Option<f64>,
    pub geometry_finite: bool,
    pub accepted_current: bool,
    pub error: Option<String>,
}

impl SurveyOutcome {
    /// A survey that failed to build, apply, or resolve a subject.
    pub fn with_error(family: Family, variant: FuzzVariant, error: impl Into<String>) -> Self {
        Self {
            family,
            variant,
            accepted: false,
            hard_validity: None,
            hard_residuals_validated: false,
            max_residual: None,
            max_coordinate: None,
            geometry_finite: false,
            accepted_current: false,
            error: Some(error.into()),
        }
    }

    /// Independent residual validation of a success-like solve.
    ///
    /// Returns `Ok(())` for a rejected input or a success that passes every
    /// residual check. Returns `Err` when the solver reports success but the
    /// geometry is non-finite, the accepted state is not current, the hard
    /// validity is not `Valid`, the residuals were not validated, or the
    /// maximum normalized hard residual exceeds the bound.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the solver reports success but the geometry is
    /// non-finite, the accepted state is not current, the hard validity is not
    /// `Valid`, the residuals were not validated, or the maximum normalized hard
    /// residual exceeds the bound.
    pub fn check(&self) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(format!("survey authoring/transaction failed: {error}"));
        }
        if !self.accepted {
            return Ok(());
        }
        if !self.geometry_finite || self.max_coordinate.is_some_and(|value| value > 1.0e300) {
            return Err(format!(
                "success reports accepted but geometry is invalid (finite={finite}, max_coordinate={coordinate:?})",
                finite = self.geometry_finite,
                coordinate = self.max_coordinate,
            ));
        }
        if self.hard_validity != Some(SketchHardValidity::Valid) {
            return Err(format!(
                "success reports accepted but hard_validity={validity:?}",
                validity = self.hard_validity,
            ));
        }
        if !self.hard_residuals_validated {
            return Err(
                "success reports accepted but the hard residuals were not independently validated"
                    .into(),
            );
        }
        if self
            .max_residual
            .is_none_or(|value| !(value.is_finite() && value <= 1.0e-9))
        {
            return Err(format!(
                "success reports accepted but the maximum normalized hard residual {residual:?} exceeds 1e-9",
                residual = self.max_residual,
            ));
        }
        if !self.accepted_current {
            return Err(
                "success reports accepted but the accepted state is not current for this input"
                    .into(),
            );
        }
        Ok(())
    }
}

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("survey parent document must be valid");
    RetainedEditorCoordinator::new(session).expect("survey coordinator")
}

fn geometry_finiteness(document: &SketchDocument) -> (bool, Option<f64>) {
    let mut finite = true;
    let mut max_coordinate = None;
    for point in document.points() {
        for value in point.position {
            if !value.is_finite() {
                finite = false;
            }
            max_coordinate = Some(max_coordinate.unwrap_or(0.0f64).max(value.abs()));
        }
    }
    for scalar in document.scalars() {
        if !scalar.value.is_finite() {
            finite = false;
        }
        max_coordinate = Some(max_coordinate.unwrap_or(0.0f64).max(scalar.value.abs()));
    }
    (finite, max_coordinate)
}

fn accepted_current(coordinator: &RetainedEditorCoordinator) -> bool {
    let Some(accepted) = coordinator.session().accepted_state_for_current_input() else {
        return false;
    };
    accepted.design_identity() == coordinator.session().design_identity()
}

fn authoring_application(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[geosolve_constraint_editor::AuthoringOperand],
    variant: FuzzVariant,
) -> Result<geosolve_constraint_editor::AuthoringApplication, String> {
    let mut state = AuthoringState::default();
    state.set_options(AuthoringOptions {
        tangent_orientation: crate::fixture::tangent_option(variant),
        curvature_relation: crate::fixture::curvature_option(variant),
        continuity: crate::fixture::continuity_option(variant),
        dimension_mode: DocumentDimensionMode::Driving,
        angle_orientation: DocumentAngleOrientation::CounterClockwise,
    });
    match state.activate(document, tool, operands) {
        AuthoringOutcome::Apply(application) => Ok(application),
        AuthoringOutcome::Warning(warning) => {
            Err(format!("authoring rejected operands: {warning:?}"))
        }
        other => Err(format!(
            "authoring did not complete with a usable application: {other:?}"
        )),
    }
}

/// Survey one family + variant through the retained editor coordinator.
#[must_use]
pub fn survey(subject: Family, variant: FuzzVariant) -> SurveyOutcome {
    let fixture = MatrixFixture::new(subject, variant);
    let document = fixture.document().clone();

    let (operands, tool): (
        Vec<geosolve_constraint_editor::AuthoringOperand>,
        AuthoringTool,
    ) = match subject {
        Family::Constraint { kind, intent } => (
            constraint_operands(kind, &fixture, variant),
            AuthoringTool::Constraint(intent),
        ),
        Family::Dimension(kind) => (
            dimension_operands(kind, &fixture, variant),
            AuthoringTool::Dimension(kind),
        ),
    };

    let application = match authoring_application(&document, tool, &operands, variant) {
        Ok(application) => application,
        Err(error) => return SurveyOutcome::with_error(subject, variant, error),
    };

    match subject {
        Family::Constraint { kind, .. } if application.resolved_constraint != Some(kind) => {
            return SurveyOutcome::with_error(
                subject,
                variant,
                format!(
                    "expected resolved constraint {kind:?}, got {:?}",
                    application.resolved_constraint,
                ),
            );
        }
        Family::Dimension(_) if application.resolved_constraint.is_some() => {
            return SurveyOutcome::with_error(
                subject,
                variant,
                "dimension application unexpectedly resolved a constraint".to_string(),
            );
        }
        _ => {}
    }

    let mut coordinator = coordinator(document);
    let identity = coordinator.session().design_identity();
    match coordinator.apply_authoring(identity, &application) {
        Ok(mutation) => {
            let accepted = published(&mutation)
                && coordinator
                    .session()
                    .latest_attempt_diagnostics()
                    .solve
                    .is_some_and(|diagnostic| diagnostic.accepted);
            let (finite, max_coordinate) =
                geometry_finiteness(coordinator.session().design_document());
            SurveyOutcome {
                family: subject,
                variant,
                accepted,
                hard_validity: attempt_hard_validity(&coordinator),
                hard_residuals_validated: attempt_hard_residuals_validated(&coordinator),
                max_residual: attempt_max_residual(&coordinator),
                max_coordinate,
                geometry_finite: finite,
                accepted_current: accepted_current(&coordinator),
                error: None,
            }
        }
        Err(error) => {
            SurveyOutcome::with_error(subject, variant, format!("apply_authoring failed: {error}"))
        }
    }
}

fn published(mutation: &AuthoringMutation) -> bool {
    match mutation {
        AuthoringMutation::Constraint(outcome) => outcome.published_accepted.is_some(),
        AuthoringMutation::Dimension(outcome) => outcome.published_accepted.is_some(),
    }
}

fn attempt_diagnostics(
    coordinator: &RetainedEditorCoordinator,
) -> Option<geosolve_sketch::SketchSolveDiagnostic> {
    coordinator.session().latest_attempt_diagnostics().solve
}

fn attempt_hard_validity(coordinator: &RetainedEditorCoordinator) -> Option<SketchHardValidity> {
    attempt_diagnostics(coordinator)
        .filter(|diagnostic| diagnostic.accepted)
        .map(|diagnostic| diagnostic.hard_validity)
}

fn attempt_hard_residuals_validated(coordinator: &RetainedEditorCoordinator) -> bool {
    attempt_diagnostics(coordinator).is_some_and(|diagnostic| diagnostic.hard_residuals_validated)
}

fn attempt_max_residual(coordinator: &RetainedEditorCoordinator) -> Option<f64> {
    attempt_diagnostics(coordinator)
        .and_then(|diagnostic| diagnostic.maximum_normalized_hard_residual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_survey_runs_without_reporting_false_convergence() {
        let variant = FuzzVariant::DETERMINISTIC;
        for family in &crate::FAMILIES {
            let outcome = survey(*family, variant);
            // A rejected or unresolved input is a valid outcome, not a defect.
            if let Err(message) = outcome.check() {
                // Only a genuine defect should surface; reject inputs are allowed.
                assert!(
                    outcome.accepted,
                    "non-accepted survey for {family:?} reported a defect: {message}"
                );
            }
        }
    }
}

#[cfg(test)]
mod oracle_check_tests {
    use super::*;

    /// A fully-accepted, all-good outcome: [`SurveyOutcome::check`] must return
    /// `Ok`. Each negative test below clones this and flips exactly one gate.
    fn accepted() -> SurveyOutcome {
        SurveyOutcome {
            family: crate::FAMILIES[0],
            variant: FuzzVariant::DETERMINISTIC,
            accepted: true,
            hard_validity: Some(SketchHardValidity::Valid),
            hard_residuals_validated: true,
            max_residual: Some(1.0e-12),
            max_coordinate: Some(1.0),
            geometry_finite: true,
            accepted_current: true,
            error: None,
        }
    }

    #[test]
    fn accepted_and_all_good_passes() {
        assert!(accepted().check().is_ok());
    }

    #[test]
    fn max_residual_at_bound_passes() {
        let mut o = accepted();
        o.max_residual = Some(1.0e-9); // `<= 1e-9` is the gate
        assert!(o.check().is_ok());
    }

    #[test]
    fn max_coordinate_at_bound_passes() {
        let mut o = accepted();
        o.max_coordinate = Some(1.0e300); // `> 1e300` is the gate
        assert!(o.check().is_ok());
    }

    #[test]
    fn rejected_but_bad_is_ok() {
        // A rejected input is a valid outcome, not a defect, even when every
        // other field looks wrong.
        let mut o = accepted();
        o.accepted = false;
        o.geometry_finite = false;
        o.hard_validity = None;
        o.hard_residuals_validated = false;
        o.max_residual = Some(2.0e-9);
        o.accepted_current = false;
        assert!(o.check().is_ok());
    }

    #[test]
    fn error_always_fails_even_when_rejected() {
        let mut o = accepted();
        o.accepted = false;
        o.error = Some("authoring failed".to_string());
        assert!(o.check().is_err());
    }

    #[test]
    fn non_finite_geometry_fails() {
        let mut o = accepted();
        o.geometry_finite = false;
        assert!(o.check().is_err());
    }

    #[test]
    fn overlarge_coordinate_fails() {
        let mut o = accepted();
        o.max_coordinate = Some(1.0e301);
        assert!(o.check().is_err());
    }

    #[test]
    fn hard_validity_missing_fails() {
        let mut o = accepted();
        o.hard_validity = None; // != Some(Valid)
        assert!(o.check().is_err());
    }

    #[test]
    fn residuals_not_validated_fails() {
        let mut o = accepted();
        o.hard_residuals_validated = false;
        assert!(o.check().is_err());
    }

    #[test]
    fn max_residual_missing_fails() {
        let mut o = accepted();
        o.max_residual = None;
        assert!(o.check().is_err());
    }

    #[test]
    fn max_residual_nan_fails() {
        let mut o = accepted();
        o.max_residual = Some(f64::NAN);
        assert!(o.check().is_err());
    }

    #[test]
    fn max_residual_over_bound_fails() {
        let mut o = accepted();
        o.max_residual = Some(2.0e-9); // `> 1e-9`
        assert!(o.check().is_err());
    }

    #[test]
    fn accepted_state_not_current_fails() {
        let mut o = accepted();
        o.accepted_current = false;
        assert!(o.check().is_err());
    }
}
