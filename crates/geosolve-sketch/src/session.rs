use geosolve_core::{
    AcceptedAuditPatch, HardValidity, SessionCoreRejection, SessionDomainRejection, SessionError,
    SessionPatch, SessionTransactionRejection, SolveSession, SolveTermination, SolverConfig,
};
use geosolve_geometry::{Point2, Vector2};
use thiserror::Error;

use crate::compiler::{
    CompiledSketch, ConicVectorRole, ReferenceDimensionValue, SketchGeometry, SketchSource,
    SketchSourceMapping, SolvedLatent, acceptance_solver_config, rejection_hard_validity,
};
use crate::{
    ArcId, CircleId, CircleTangencyMode, ConicId, ContactState, PointId, Sketch,
    SketchConstraintId, SketchDimensionId, SketchError, SketchSolveRequest, SketchSolveResult,
    SolveRejection,
};

/// One additive, non-structural edit supported by the M10 sketch session.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum SketchPatch {
    PointPosition {
        point: PointId,
        position: Point2<f64>,
    },
    CircleRadius {
        circle: CircleId,
        radius: f64,
    },
    ArcRadius {
        arc: ArcId,
        radius: f64,
    },
    ConicWeightedMiddle {
        conic: ConicId,
        weighted_middle: Vector2<f64>,
    },
    DimensionTarget {
        dimension: SketchDimensionId,
        target: f64,
    },
    ContactState {
        constraint: SketchConstraintId,
        state: ContactState,
    },
    CircleTangencyMode {
        constraint: SketchConstraintId,
        mode: CircleTangencyMode,
    },
    /// Only the target of the already compiled drag point may change.
    DragTarget {
        point: PointId,
        target: Point2<f64>,
    },
}

/// Revision-checked sketch transaction input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchSessionPatch {
    pub expected_revision: u64,
    pub edit: SketchPatch,
}

impl SketchSessionPatch {
    #[must_use]
    pub const fn new(expected_revision: u64, edit: SketchPatch) -> Self {
        Self {
            expected_revision,
            edit,
        }
    }
}

/// Session construction, patch validation, or explicit rebuild failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SketchSessionError {
    #[error(transparent)]
    Sketch(#[from] SketchError),
    #[error(transparent)]
    CoreSession(#[from] SessionError),
    #[error(transparent)]
    Core(#[from] geosolve_core::CoreError),
    #[error("stale sketch-session patch: expected revision {expected}, accepted revision {actual}")]
    StalePatch { expected: u64, actual: u64 },
    #[error("this edit changes the compiled request/topology shape and requires explicit rebuild")]
    RebuildRequired,
    #[error("initial sketch solve was rejected: {0:?}")]
    InitialRejected(SolveRejection),
    #[error("non-structural compilation changed retained core mappings")]
    MappingChanged,
    #[error("committed core transaction did not produce a complete sketch candidate")]
    MissingCandidate,
}

/// Domain-level revision counters for one accepted sketch session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SketchSessionRevisions {
    pub topology: u64,
    pub source: u64,
    pub state: u64,
    pub bound: u64,
}

/// Persistent accepted sketch plus retained core compilation/session state.
#[derive(Clone, Debug)]
pub struct SketchSession {
    sketch: Sketch,
    request: SketchSolveRequest,
    compiled: CompiledSketch,
    core: SolveSession,
    accepted_result: SketchSolveResult,
    revision: u64,
    revisions: SketchSessionRevisions,
    topology_compilations: u64,
    preference_targets: Vec<(PointId, Point2<f64>)>,
}

#[derive(Debug)]
struct CompleteSketchCandidate {
    sketch: Sketch,
    geometry: SketchGeometry,
    reference_values: Vec<ReferenceDimensionValue>,
    normalized_latents: Vec<SolvedLatent>,
    independent_hard_residual_max: f64,
}

enum LatentSynchronization {
    Unchanged,
    Committed,
    Rejected(Box<geosolve_core::SolveReport>, SolveRejection),
}

impl SketchSession {
    /// Builds the first accepted sketch/session revision.
    ///
    /// # Errors
    ///
    /// Returns a typed compile/core failure or the initial domain rejection.
    pub fn new(
        mut sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, SketchSessionError> {
        let config = acceptance_solver_config(config);
        let validation_sketch = sketch.clone();
        let mut compiled = sketch.compile(request)?;
        let mut core = SolveSession::new(compiled.problem().clone(), config)?;
        let complete = finalize_solved_candidate(&mut core, &compiled, &sketch, request)?
            .map_err(|(_, rejection)| SketchSessionError::InitialRejected(rejection))?;
        let independent_hard_residual_max = complete.independent_hard_residual_max;
        sketch = complete.sketch;
        compiled.replace_problem(core.problem().clone());
        let mut audit_refresh = AcceptedAuditPatch::new(core.revisions());
        copy_changed_constraint_audits(
            &core,
            &compiled,
            &validation_sketch,
            &sketch,
            request,
            &mut audit_refresh,
        )?;
        core.refresh_accepted_audit(audit_refresh)?;
        compiled.replace_problem(core.problem().clone());
        let report = core.report().clone();
        let accepted_result = SketchSolveResult {
            geometry: sketch.geometry(),
            display_audit: report.audit.clone(),
            reference_values: sketch.reference_values()?,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: Some(
                core.report()
                    .hard_residual_max
                    .max(independent_hard_residual_max),
            ),
        };
        let preference_targets = preference_targets(&validation_sketch, &compiled);
        Ok(Self {
            sketch,
            request,
            compiled,
            core,
            accepted_result,
            revision: 0,
            revisions: SketchSessionRevisions::default(),
            topology_compilations: 1,
            preference_targets,
        })
    }

    #[must_use]
    pub const fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    #[must_use]
    pub const fn request(&self) -> SketchSolveRequest {
        self.request
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn revisions(&self) -> SketchSessionRevisions {
        self.revisions
    }

    /// Number of accepted full topology compilations performed by this session.
    /// Non-structural patches leave this count unchanged.
    #[must_use]
    pub const fn topology_compilations(&self) -> u64 {
        self.topology_compilations
    }

    #[must_use]
    pub const fn accepted_result(&self) -> &SketchSolveResult {
        &self.accepted_result
    }

    #[must_use]
    pub fn source_mapping(&self, source: SketchSource) -> Option<&SketchSourceMapping> {
        self.accepted_result
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == source)
    }

    /// Returns the accepted core bound audit for one domain bound.
    #[must_use]
    pub fn bound_report(&self, bound: crate::SketchBound) -> Option<&geosolve_core::BoundReport> {
        let bound_id = self
            .compiled
            .bound_mappings()
            .iter()
            .find_map(|mapping| (mapping.bound == bound).then_some(mapping.bound_id))?;
        self.accepted_result
            .core_report
            .bounds
            .iter()
            .find(|report| report.bound_id == bound_id)
    }

    /// Returns accepted audit rows for one domain source.
    #[must_use]
    pub fn audit_source(
        &self,
        source: SketchSource,
    ) -> Option<&geosolve_core::AuditSourceSnapshot> {
        let source_id = self.source_mapping(source)?.core_source_id?;
        self.accepted_result
            .display_audit
            .sources
            .iter()
            .find(|audit| audit.source_id == source_id)
    }

    /// Applies one non-structural edit and atomically commits a complete solved sketch.
    ///
    /// # Errors
    ///
    /// Returns stale/preflight/compile failures before mutation. Numerical and
    /// domain failures are returned as rejected [`SketchSolveResult`] values.
    #[allow(clippy::too_many_lines)]
    pub fn apply_patch(
        &mut self,
        patch: SketchSessionPatch,
    ) -> Result<SketchSolveResult, SketchSessionError> {
        if patch.expected_revision != self.revision {
            return Err(SketchSessionError::StalePatch {
                expected: patch.expected_revision,
                actual: self.revision,
            });
        }

        let mut candidate_sketch = self.sketch.clone();
        let mut candidate_request = self.request;
        apply_domain_edit(&mut candidate_sketch, &mut candidate_request, patch.edit)?;
        candidate_sketch.preflight_segments()?;
        let candidate_preference_targets = preference_targets(&candidate_sketch, &self.compiled);

        let mut core_patch = SessionPatch::new(self.core.revisions());
        let mut replacement_sources = Vec::new();
        let mut replacement_source_labels = Vec::new();
        let mut bound_changed = false;
        match patch.edit {
            SketchPatch::PointPosition { point, .. } => {
                copy_point_value(&candidate_sketch, &self.compiled, point, &mut core_patch)?;
                push_unique(&mut replacement_sources, SketchSource::PreviousState(point));
            }
            SketchPatch::CircleRadius { circle, .. } => {
                copy_circle_radius_value(
                    &candidate_sketch,
                    &self.compiled,
                    circle,
                    &mut core_patch,
                )?;
            }
            SketchPatch::ArcRadius { arc, .. } => {
                copy_arc_radius_value(&candidate_sketch, &self.compiled, arc, &mut core_patch)?;
            }
            SketchPatch::ConicWeightedMiddle { conic, .. } => {
                copy_conic_weighted_middle_value(
                    &candidate_sketch,
                    &self.compiled,
                    conic,
                    &mut core_patch,
                )?;
            }
            SketchPatch::DimensionTarget { dimension, .. } => {
                push_unique(&mut replacement_sources, SketchSource::Dimension(dimension));
            }
            SketchPatch::ContactState { constraint, .. }
            | SketchPatch::CircleTangencyMode { constraint, .. } => {
                push_unique(
                    &mut replacement_sources,
                    SketchSource::Constraint(constraint),
                );
            }
            SketchPatch::DragTarget { point, .. } => {
                push_unique(&mut replacement_sources, SketchSource::DragTarget(point));
            }
        }

        for mapping in self.compiled.source_mappings() {
            let SketchSource::PreviousState(point) = mapping.source else {
                continue;
            };
            let Some(position) = candidate_sketch
                .point(point)
                .map(crate::SketchPoint::position)
            else {
                continue;
            };
            if self
                .preference_targets
                .iter()
                .find_map(|(id, target)| (*id == point).then_some(*target))
                .is_some_and(|target| point_bits(target) != point_bits(position))
            {
                push_unique(&mut replacement_sources, mapping.source);
            }
        }
        replacement_sources.retain(|source| {
            self.compiled
                .source_mappings()
                .iter()
                .any(|mapping| mapping.source == *source && mapping.core_source_id.is_some())
        });
        for source in &replacement_sources {
            if let Some(source_patch) =
                self.compiled
                    .source_patch(&candidate_sketch, candidate_request, *source)?
            {
                replacement_source_labels.push((*source, source_patch.source.label().to_owned()));
                bound_changed |= !source_patch.bounds.is_empty();
                for (variable, value) in source_patch.variable_values {
                    core_patch.set_variable_value(variable, value);
                }
                for (bound_id, bound) in source_patch.bounds {
                    core_patch.replace_bound(bound_id, bound);
                }
                core_patch.replace_source(source_patch.source_id, source_patch.source);
                for (residual_id, residual) in source_patch.residuals {
                    core_patch.replace_residual(residual_id, residual);
                }
            }
        }
        let mut domain_source_changed =
            edit_changes_source(patch.edit) || !replacement_sources.is_empty();

        let validation_compiled = self.compiled.clone();
        let validation_template = candidate_sketch.clone();
        let mut candidate_core = self.core.clone();
        let validation_tolerance = candidate_core.config().normalized_residual_tolerance;
        let (transaction, complete_candidate) =
            candidate_core.apply_with_output(core_patch, |problem, report| {
                complete_candidate_for_problem(
                    problem,
                    report,
                    &validation_compiled,
                    &validation_template,
                    candidate_request,
                    validation_tolerance,
                )
            })?;

        let rejection = transaction.rejection.map(|rejection| match rejection {
            SessionTransactionRejection::Domain(rejection) => rejection,
            SessionTransactionRejection::Core(rejection) => {
                map_core_rejection(&rejection, &transaction.report, self.core.config())
            }
            _ => {
                SolveRejection::IndependentValidationFailed("unknown core session rejection".into())
            }
        });
        if let Some(rejection) = rejection {
            let acceptance_max = crate::compiler::rejection_residual_max(&rejection)
                .map(|maximum| maximum.max(transaction.report.hard_residual_max));
            return Ok(SketchSolveResult {
                geometry: self.sketch.geometry(),
                display_audit: self.accepted_result.display_audit.clone(),
                reference_values: self.accepted_result.reference_values.clone(),
                source_mappings: self.accepted_result.source_mappings.clone(),
                bound_mappings: self.accepted_result.bound_mappings.clone(),
                core_report: transaction.report,
                rejection: Some(rejection),
                acceptance_hard_residual_max: acceptance_max,
            });
        }

        let _ = complete_candidate.ok_or(SketchSessionError::MissingCandidate)?;
        let complete = match finalize_solved_candidate(
            &mut candidate_core,
            &validation_compiled,
            &validation_template,
            candidate_request,
        )? {
            Ok(complete) => complete,
            Err((sync_report, rejection)) => {
                let acceptance_max = crate::compiler::rejection_residual_max(&rejection)
                    .map(|maximum| maximum.max(sync_report.hard_residual_max));
                return Ok(SketchSolveResult {
                    geometry: self.sketch.geometry(),
                    display_audit: self.accepted_result.display_audit.clone(),
                    reference_values: self.accepted_result.reference_values.clone(),
                    source_mappings: self.accepted_result.source_mappings.clone(),
                    bound_mappings: self.accepted_result.bound_mappings.clone(),
                    core_report: sync_report,
                    rejection: Some(rejection),
                    acceptance_hard_residual_max: acceptance_max,
                });
            }
        };
        let mut accepted_compiled = self.compiled.clone();
        for (source, label) in replacement_source_labels {
            accepted_compiled.replace_source_label(source, label)?;
        }
        accepted_compiled.replace_problem(candidate_core.problem().clone());
        let mut audit_refresh = AcceptedAuditPatch::new(candidate_core.revisions());
        let audit_changed = copy_changed_constraint_audits(
            &candidate_core,
            &accepted_compiled,
            &validation_template,
            &complete.sketch,
            candidate_request,
            &mut audit_refresh,
        )?;
        candidate_core.refresh_accepted_audit(audit_refresh)?;
        accepted_compiled.replace_problem(candidate_core.problem().clone());
        domain_source_changed |= audit_changed;
        let report = candidate_core.report().clone();
        let result = SketchSolveResult {
            geometry: complete.geometry,
            display_audit: report.audit.clone(),
            reference_values: complete.reference_values,
            source_mappings: accepted_compiled.source_mappings().to_vec(),
            bound_mappings: accepted_compiled.bound_mappings().to_vec(),
            acceptance_hard_residual_max: Some(
                report
                    .hard_residual_max
                    .max(complete.independent_hard_residual_max),
            ),
            core_report: report,
            rejection: None,
        };
        self.sketch = complete.sketch;
        self.request = candidate_request;
        self.compiled = accepted_compiled;
        self.core = candidate_core;
        self.revision = self.revision.saturating_add(1);
        self.revisions.state = self.revisions.state.saturating_add(1);
        if domain_source_changed {
            self.revisions.source = self.revisions.source.saturating_add(1);
        }
        if bound_changed {
            self.revisions.bound = self.revisions.bound.saturating_add(1);
        }
        self.preference_targets = candidate_preference_targets;
        self.accepted_result = result.clone();
        Ok(result)
    }

    /// Explicitly rebuilds request shape over the current accepted sketch.
    ///
    /// # Errors
    ///
    /// Returns a stale revision or any compile/solve/domain error from building
    /// the replacement request shape. The accepted session is retained.
    pub fn rebuild_request(
        &mut self,
        expected_revision: u64,
        request: SketchSolveRequest,
    ) -> Result<&SketchSolveResult, SketchSessionError> {
        self.rebuild(expected_revision, self.sketch.clone(), request)
    }

    /// Explicitly rebuilds changed topology/request shape as one clone-and-swap.
    ///
    /// # Errors
    ///
    /// Returns a stale revision or any compile/solve/domain error from building
    /// the replacement topology. The accepted session is retained.
    #[allow(clippy::needless_pass_by_value)]
    pub fn rebuild(
        &mut self,
        expected_revision: u64,
        sketch: Sketch,
        request: SketchSolveRequest,
    ) -> Result<&SketchSolveResult, SketchSessionError> {
        if expected_revision != self.revision {
            return Err(SketchSessionError::StalePatch {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let mut compiled = sketch.compile(request)?;
        let mut candidate_core = self.core.clone();
        candidate_core.rebuild(self.core.revisions(), compiled.problem().clone())?;
        let validation_sketch = sketch.clone();
        let complete = finalize_solved_candidate(&mut candidate_core, &compiled, &sketch, request)?
            .map_err(|(_, rejection)| SketchSessionError::InitialRejected(rejection))?;
        let independent_hard_residual_max = complete.independent_hard_residual_max;
        let accepted_sketch = complete.sketch;
        let reference_values = complete.reference_values;
        compiled.replace_problem(candidate_core.problem().clone());
        let mut audit_refresh = AcceptedAuditPatch::new(candidate_core.revisions());
        copy_changed_constraint_audits(
            &candidate_core,
            &compiled,
            &validation_sketch,
            &accepted_sketch,
            request,
            &mut audit_refresh,
        )?;
        candidate_core.refresh_accepted_audit(audit_refresh)?;
        compiled.replace_problem(candidate_core.problem().clone());
        let report = candidate_core.report().clone();
        let accepted_result = SketchSolveResult {
            geometry: accepted_sketch.geometry(),
            display_audit: report.audit.clone(),
            reference_values,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: Some(
                candidate_core
                    .report()
                    .hard_residual_max
                    .max(independent_hard_residual_max),
            ),
        };
        let preference_targets = preference_targets(&validation_sketch, &compiled);
        let rebuilt = Self {
            sketch: accepted_sketch,
            request,
            compiled,
            core: candidate_core,
            accepted_result,
            revision: self.revision.saturating_add(1),
            revisions: SketchSessionRevisions {
                topology: self.revisions.topology.saturating_add(1),
                source: self.revisions.source.saturating_add(1),
                state: self.revisions.state.saturating_add(1),
                bound: self.revisions.bound.saturating_add(1),
            },
            topology_compilations: self.topology_compilations.saturating_add(1),
            preference_targets,
        };
        *self = rebuilt;
        Ok(&self.accepted_result)
    }
}

fn preference_targets(sketch: &Sketch, compiled: &CompiledSketch) -> Vec<(PointId, Point2<f64>)> {
    compiled
        .source_mappings()
        .iter()
        .filter_map(|mapping| match mapping.source {
            SketchSource::PreviousState(point) => {
                sketch.point(point).map(|value| (point, value.position()))
            }
            _ => None,
        })
        .collect()
}

fn complete_candidate_for_problem(
    problem: &geosolve_core::Problem,
    report: &geosolve_core::SolveReport,
    compiled: &CompiledSketch,
    template: &Sketch,
    request: SketchSolveRequest,
    tolerance: f64,
) -> Result<CompleteSketchCandidate, SessionDomainRejection<SolveRejection>> {
    let mut candidate = compiled
        .solved_state_for_problem(problem, template)
        .map_err(|error| {
            session_domain_rejection(SolveRejection::IndependentValidationFailed(
                error.to_string(),
            ))
        })?;
    template.normalize_candidate_latents(&mut candidate);
    if let Some(segment) = template.first_flipped_segment(&candidate.geometry) {
        return Err(session_domain_rejection(
            SolveRejection::SegmentBranchFlipped(segment),
        ));
    }
    let independent_hard_residual_max = template
        .validate_m7_candidate(&candidate, tolerance)
        .map_err(session_domain_rejection)?;
    template
        .validate_drag_selected_span(request, &candidate)
        .map_err(session_domain_rejection)?;
    let mut complete = template.clone();
    complete.commit_solved_state(&candidate).map_err(|error| {
        session_domain_rejection(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ))
    })?;
    let reference_values = complete.reference_values().map_err(|error| {
        session_domain_rejection(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ))
    })?;
    if report.termination != SolveTermination::Converged {
        return Err(SessionDomainRejection::compatibility(
            SolveRejection::CoreTermination(report.termination),
        ));
    }
    Ok(CompleteSketchCandidate {
        geometry: complete.geometry(),
        sketch: complete,
        reference_values,
        normalized_latents: candidate.latents,
        independent_hard_residual_max,
    })
}

fn finalize_solved_candidate(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    template: &Sketch,
    request: SketchSolveRequest,
) -> Result<
    Result<CompleteSketchCandidate, (geosolve_core::SolveReport, SolveRejection)>,
    SketchSessionError,
> {
    for _ in 0..4 {
        let tolerance = core.config().normalized_residual_tolerance;
        let complete = match complete_candidate_for_problem(
            core.problem(),
            core.report(),
            compiled,
            template,
            request,
            tolerance,
        ) {
            Ok(complete) => complete,
            Err(rejection) => {
                let mut report = core.report().clone();
                report.hard_validity = rejection.hard_validity;
                return Ok(Err((report, rejection.reason)));
            }
        };
        match synchronize_accepted_latents(core, compiled, &complete.normalized_latents)? {
            LatentSynchronization::Unchanged => return Ok(Ok(complete)),
            LatentSynchronization::Committed => {}
            LatentSynchronization::Rejected(report, rejection) => {
                return Ok(Err((*report, rejection)));
            }
        }
    }
    let mut report = core.report().clone();
    report.termination = SolveTermination::Stalled;
    Ok(Err((
        report,
        SolveRejection::CoreTermination(SolveTermination::Stalled),
    )))
}

fn synchronize_accepted_latents(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    latents: &[SolvedLatent],
) -> Result<LatentSynchronization, SketchSessionError> {
    let mut patch = SessionPatch::new(core.revisions());
    let mut changed = false;
    for latent in latents {
        let variable = compiled
            .latent_variables()
            .iter()
            .find(|mapping| {
                mapping.constraint_id == latent.constraint_id && mapping.role == latent.role
            })
            .ok_or(SketchSessionError::MappingChanged)?
            .variable_id;
        let current = core
            .problem()
            .variable(variable)
            .ok_or(geosolve_core::CoreError::UnknownVariable(variable))?
            .value();
        let geosolve_core::VariableValue::Scalar(current) = current else {
            return Err(geosolve_core::CoreError::VariableKindMismatch {
                expected: geosolve_core::VariableKind::Scalar,
                actual: current.kind(),
            }
            .into());
        };
        if current.to_bits() != latent.value.to_bits() {
            patch.set_variable_value(variable, geosolve_core::VariableValue::Scalar(latent.value));
            changed = true;
        }
    }
    if !changed {
        return Ok(LatentSynchronization::Unchanged);
    }
    let transaction = core.apply(patch)?;
    if transaction.committed() {
        return Ok(LatentSynchronization::Committed);
    }
    let rejection = match transaction.rejection.as_ref() {
        Some(SessionTransactionRejection::Core(rejection)) => {
            map_core_rejection(rejection, &transaction.report, core.config())
        }
        Some(SessionTransactionRejection::Domain(never)) => match *never {},
        _ => SolveRejection::IndependentValidationFailed(
            "unknown core latent synchronization rejection".into(),
        ),
    };
    Ok(LatentSynchronization::Rejected(
        Box::new(transaction.report),
        rejection,
    ))
}

fn apply_domain_edit(
    sketch: &mut Sketch,
    request: &mut SketchSolveRequest,
    edit: SketchPatch,
) -> Result<(), SketchSessionError> {
    match edit {
        SketchPatch::PointPosition { point, position } => {
            sketch.set_point_position(point, position)?;
        }
        SketchPatch::CircleRadius { circle, radius } => sketch.set_circle_radius(circle, radius)?,
        SketchPatch::ArcRadius { arc, radius } => sketch.set_arc_radius(arc, radius)?,
        SketchPatch::ConicWeightedMiddle {
            conic,
            weighted_middle,
        } => sketch.set_conic_weighted_middle(conic, weighted_middle)?,
        SketchPatch::DimensionTarget { dimension, target } => {
            sketch.set_dimension_target(dimension, target)?;
        }
        SketchPatch::ContactState { constraint, state } => {
            sketch.set_contact_state(constraint, state)?;
        }
        SketchPatch::CircleTangencyMode { constraint, mode } => {
            sketch.set_circle_tangency_mode(constraint, mode)?;
        }
        SketchPatch::DragTarget { point, target } => {
            let Some(drag) = request.drag else {
                return Err(SketchSessionError::RebuildRequired);
            };
            if drag.point != point {
                return Err(SketchSessionError::RebuildRequired);
            }
            crate::model::validate_point(target, "drag target")?;
            request.drag = Some(crate::DragTarget { point, target });
        }
    }
    Ok(())
}

fn copy_point_value(
    sketch: &Sketch,
    compiled: &CompiledSketch,
    point: PointId,
    patch: &mut SessionPatch,
) -> Result<(), SketchSessionError> {
    let variable = compiled
        .variable_for_point(point)
        .ok_or(SketchError::UnknownPoint(point))?;
    let position = sketch
        .point(point)
        .ok_or(SketchError::UnknownPoint(point))?
        .position();
    patch.set_variable_value(
        variable,
        geosolve_core::VariableValue::Vec2([position.x, position.y]),
    );
    Ok(())
}

fn copy_circle_radius_value(
    sketch: &Sketch,
    compiled: &CompiledSketch,
    circle: CircleId,
    patch: &mut SessionPatch,
) -> Result<(), SketchSessionError> {
    let variable = compiled
        .variable_for_circle_radius(circle)
        .ok_or(SketchError::UnknownCircle(circle))?;
    let radius = sketch.circle_value(circle)?.radius();
    patch.set_variable_value(variable, geosolve_core::VariableValue::Scalar(radius));
    Ok(())
}

fn copy_arc_radius_value(
    sketch: &Sketch,
    compiled: &CompiledSketch,
    arc: ArcId,
    patch: &mut SessionPatch,
) -> Result<(), SketchSessionError> {
    let variable = compiled
        .variable_for_arc_radius(arc)
        .ok_or(SketchError::UnknownArc(arc))?;
    let radius = sketch.arc_value(arc)?.radius();
    patch.set_variable_value(variable, geosolve_core::VariableValue::Scalar(radius));
    Ok(())
}

fn copy_conic_weighted_middle_value(
    sketch: &Sketch,
    compiled: &CompiledSketch,
    conic: ConicId,
    patch: &mut SessionPatch,
) -> Result<(), SketchSessionError> {
    let variable = compiled
        .variable_for_conic_vector(conic, ConicVectorRole::WeightedMiddle)
        .ok_or(SketchError::UnknownConic(conic))?;
    let crate::ConicKind::RationalQuadratic {
        weighted_middle, ..
    } = sketch.conic_value(conic)?.kind()
    else {
        return Err(SketchError::InvalidConicScalarRole(conic).into());
    };
    patch.set_variable_value(
        variable,
        geosolve_core::VariableValue::Vec2([weighted_middle.x, weighted_middle.y]),
    );
    Ok(())
}

fn copy_changed_constraint_audits(
    core: &SolveSession,
    accepted: &CompiledSketch,
    before: &Sketch,
    after: &Sketch,
    request: SketchSolveRequest,
    patch: &mut AcceptedAuditPatch,
) -> Result<bool, SketchSessionError> {
    let mut changed = false;
    for mapping in accepted.source_mappings() {
        let SketchSource::Constraint(constraint) = mapping.source else {
            continue;
        };
        if before.contact_state(constraint).ok() == after.contact_state(constraint).ok() {
            continue;
        }
        let Some(refreshed) = accepted.source_patch(after, request, mapping.source)? else {
            continue;
        };
        let current_source = core
            .problem()
            .source(refreshed.source_id)
            .ok_or(geosolve_core::CoreError::UnknownSource(refreshed.source_id))?;
        let residual_changed = refreshed.residuals.iter().any(|(residual_id, residual)| {
            core.problem()
                .residual(*residual_id)
                .is_none_or(|current| current.audit_rows() != residual.audit_rows())
        });
        if current_source.label() == refreshed.source.label() && !residual_changed {
            continue;
        }
        changed = true;
        patch.replace_source(refreshed.source_id, refreshed.source);
        for (residual_id, residual) in refreshed.residuals {
            patch.replace_residual_rows(residual_id, residual.audit_rows().to_vec());
        }
    }
    Ok(changed)
}

const fn edit_changes_source(edit: SketchPatch) -> bool {
    matches!(
        edit,
        SketchPatch::DimensionTarget { .. }
            | SketchPatch::ContactState { .. }
            | SketchPatch::CircleTangencyMode { .. }
            | SketchPatch::DragTarget { .. }
    )
}

fn session_domain_rejection(rejection: SolveRejection) -> SessionDomainRejection<SolveRejection> {
    let hard_validity = rejection_hard_validity(&rejection);
    SessionDomainRejection::new(rejection, hard_validity)
}

fn map_core_rejection(
    rejection: &SessionCoreRejection,
    report: &geosolve_core::SolveReport,
    config: SolverConfig,
) -> SolveRejection {
    match rejection {
        SessionCoreRejection::BoundViolation(bound) => SolveRejection::BoundViolation(*bound),
        SessionCoreRejection::HardResidual { maximum, tolerance } => SolveRejection::HardResidual {
            maximum: *maximum,
            tolerance: *tolerance,
        },
        SessionCoreRejection::HardValidity(HardValidity::Invalid) => SolveRejection::HardResidual {
            maximum: report.hard_residual_max,
            tolerance: config.normalized_residual_tolerance,
        },
        SessionCoreRejection::HardValidity(_)
        | SessionCoreRejection::RankInvalid
        | SessionCoreRejection::EvaluationFailure
        | SessionCoreRejection::NonFiniteReport => {
            SolveRejection::CoreTermination(report.termination)
        }
        _ => SolveRejection::IndependentValidationFailed("unknown core session rejection".into()),
    }
}

fn point_bits(point: Point2<f64>) -> [u64; 2] {
    [point.x.to_bits(), point.y.to_bits()]
}

fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}
