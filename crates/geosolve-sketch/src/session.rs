use geosolve_core::{
    AcceptedAuditPatch, HardValidity, OperationCheckpoint, OperationControl, OperationController,
    OperationOutcome, OperationWorkCounter, SessionCoreRejection, SessionDomainRejection,
    SessionError, SessionPatch, SessionTransactionRejection, SolveSession, SolveTermination,
    SolverConfig,
};
use geosolve_geometry::{Point2, Vector2};
use thiserror::Error;

use crate::compiler::{
    CompiledSketch, ConicVectorRole, PreviousStateReference, ReferenceDimensionValue,
    SketchGeometry, SketchSource, SketchSourceMapping, SolvedLatent, acceptance_solver_config,
    rejection_hard_validity,
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
    ///
    /// Consecutive patches for that point belong to the request's current drag
    /// gesture. They advance the accepted numerical continuation state without
    /// recapturing `PreviousState` preference targets. Rebuilding the request to
    /// clear or replace the drag starts a new preference-target lifetime.
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
    #[error("drag locality planning is unavailable: {context}")]
    DragLocalityUnavailable { context: &'static str },
    #[error(
        "drag locality planning requires {active_tangent_dimensions} active tangent dimensions, \
         exceeding the interactive limit of {limit}"
    )]
    DragLocalityEnvelopeExceeded {
        active_tangent_dimensions: usize,
        limit: usize,
    },
    #[error(
        "drag locality planning requires {active_hard_rows} accepted hard rows, exceeding the \
         interactive limit of {limit}"
    )]
    DragLocalityRowEnvelopeExceeded {
        active_hard_rows: usize,
        limit: usize,
    },
    #[error(
        "drag locality planning found only {spanned} of {required} required mobility directions"
    )]
    DragLocalityIncomplete { required: usize, spanned: usize },
}

/// Domain-level revision counters for one accepted sketch session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SketchSessionRevisions {
    pub topology: u64,
    pub source: u64,
    pub state: u64,
    pub bound: u64,
}

/// Execution path that produced the current accepted runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchSessionExecutionKind {
    InitialSolve,
    /// Fresh independent validation of an already materialized state without
    /// projection, nonlinear iteration, or secondary-priority movement.
    NoMotionCertification,
    IncrementalUpdate,
    FullRebuild,
}

/// Bounded execution evidence for the current accepted runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchSessionExecutionSummary {
    pub kind: SketchSessionExecutionKind,
    pub component_count: usize,
    pub reused_component_count: usize,
    pub freshly_validated_hard_rows: bool,
    pub rank_valid: bool,
}

/// Numerical-rank authority supported by the production sketch envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchRankAuthority {
    /// Sparse storage/steps may execute, but rank remains authoritative dense SVD
    /// within the declared connected-component bound.
    BoundedDenseSvd,
}

/// Honest connected-component assessment for one accepted sketch runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchProductionScaleAssessment {
    pub authority: SketchRankAuthority,
    pub supported: bool,
    pub component_limit: usize,
    pub component_count: usize,
    pub maximum_active_rows: usize,
    pub maximum_active_tangent_dimensions: usize,
}

/// One accepted point selected to preserve a passive mechanism freedom during a drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SketchDragLocalityAnchor {
    pub(crate) point: PointId,
    /// Rank of this point's complete two-coordinate response in the accepted hard nullspace.
    pub(crate) mobility_rank: usize,
}

/// Exact-session-stamped ownership plan for one projected point drag.
///
/// The active point owns `active_rank` accepted hard-manifold directions. The anchors form a
/// deterministic complete greedy cover of the remaining visible point mobility. A caller must
/// reject this plan after any stamp changes. This runtime DTO owns rank and identity only;
/// persistent document hosts capture authoritative visible anchor targets at their lifecycle
/// boundary.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SketchDragLocalityPlan {
    pub(crate) point: PointId,
    pub(crate) hard_degrees_of_freedom: usize,
    pub(crate) active_rank: usize,
    pub(crate) passive_degrees_of_freedom: usize,
    pub(crate) anchors: Vec<SketchDragLocalityAnchor>,
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
    previous_state_reference: PreviousStateReference,
    last_execution: SketchSessionExecutionKind,
}

#[derive(Debug)]
struct CompleteSketchCandidate {
    sketch: Sketch,
    geometry: SketchGeometry,
    reference_values: Vec<ReferenceDimensionValue>,
    normalized_latents: Vec<SolvedLatent>,
    independent_hard_residual_max: f64,
}

struct DragLocalityCandidate {
    point: PointId,
    rows: Vec<Vec<f64>>,
    mobility_rank: usize,
    order: usize,
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
        sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, SketchSessionError> {
        let previous_state = PreviousStateReference::capture(&sketch);
        Self::new_with_previous_state_reference(sketch, request, config, &previous_state)
    }

    pub(crate) fn new_with_previous_state_reference(
        mut sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        previous_state: &PreviousStateReference,
    ) -> Result<Self, SketchSessionError> {
        let config = acceptance_solver_config(config);
        let validation_sketch = sketch.clone();
        let mut compiled = sketch.compile_with_previous_state_reference(request, previous_state)?;
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
            previous_state,
            &mut audit_refresh,
        )?;
        core.refresh_accepted_audit(audit_refresh)?;
        compiled.replace_problem(core.problem().clone());
        let report = core.report().clone();
        let geometry = sketch.geometry();
        let accepted_result = SketchSolveResult {
            attempted_geometry: Some(geometry.clone()),
            geometry,
            display_audit: report.audit.clone(),
            reference_values: sketch.reference_values()?,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            diagnostic_variable_owners: compiled.diagnostic_variable_owners(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: Some(
                core.report()
                    .hard_residual_max
                    .max(independent_hard_residual_max),
            ),
        };
        Ok(Self {
            sketch,
            request,
            compiled,
            core,
            accepted_result,
            revision: 0,
            revisions: SketchSessionRevisions::default(),
            topology_compilations: 1,
            previous_state_reference: previous_state.clone(),
            last_execution: SketchSessionExecutionKind::InitialSolve,
        })
    }

    /// Builds an accepted runtime by certifying the exact materialized sketch state.
    ///
    /// This path is reserved for publication of an independently accepted visible
    /// preview. It recompiles the publication request and rebuilds residual, rank,
    /// bound, diagnostic, and audit evidence, but it never projects or optimizes the
    /// candidate. Any fixed/alias/bound correction, nonzero secondary target, latent
    /// normalization, or domain correction rejects instead of moving the scene.
    pub(crate) fn certify_current_state_with_previous_state_reference_and_controller(
        mut sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        previous_state: &PreviousStateReference,
        controller: &mut OperationController,
    ) -> Result<Option<Self>, SketchSessionError> {
        let config = acceptance_solver_config(config);
        let validation_sketch = sketch.clone();
        let Some(mut compiled) = sketch.compile_with_previous_state_reference_and_controller(
            request,
            previous_state,
            controller,
        )?
        else {
            return Ok(None);
        };
        let Some(report) = compiled
            .problem()
            .certify_current_state_with_controller(config, controller)?
        else {
            return Ok(None);
        };

        let Some(complete) = complete_current_candidate_for_problem_controlled(
            compiled.problem(),
            &report,
            &compiled,
            &sketch,
            request,
            config.normalized_residual_tolerance,
            controller,
        )
        .map_err(|rejection| SketchSessionError::InitialRejected(rejection.reason))?
        else {
            return Ok(None);
        };
        let independent_hard_residual_max = complete.independent_hard_residual_max;
        sketch = complete.sketch;

        let mut core =
            SolveSession::from_accepted_report(compiled.problem().clone(), config, report)?;
        compiled.replace_problem(core.problem().clone());
        let Some(_) = refresh_changed_constraint_audits_controlled(
            &mut core,
            &compiled,
            &validation_sketch,
            &sketch,
            request,
            previous_state,
            controller,
        )?
        else {
            return Ok(None);
        };
        compiled.replace_problem(core.problem().clone());
        let report = core.report().clone();
        let geometry = sketch.geometry();
        let accepted_result = SketchSolveResult {
            attempted_geometry: Some(geometry.clone()),
            geometry,
            display_audit: report.audit.clone(),
            reference_values: complete.reference_values,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            diagnostic_variable_owners: compiled.diagnostic_variable_owners(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: Some(
                core.report()
                    .hard_residual_max
                    .max(independent_hard_residual_max),
            ),
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(Self {
            sketch,
            request,
            compiled,
            core,
            accepted_result,
            revision: 0,
            revisions: SketchSessionRevisions::default(),
            topology_compilations: 1,
            previous_state_reference: previous_state.clone(),
            last_execution: SketchSessionExecutionKind::NoMotionCertification,
        }))
    }

    /// Builds the first accepted sketch/session revision under operation control.
    ///
    /// Construction uses only scratch state. An interrupted outcome contains no
    /// partially constructed session.
    ///
    /// # Errors
    ///
    /// Returns a typed compile/core failure or the initial domain rejection.
    pub fn new_controlled(
        sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        control: geosolve_core::OperationControl,
    ) -> Result<geosolve_core::OperationOutcome<Self>, SketchSessionError> {
        let mut controller = geosolve_core::OperationController::new(control);
        let Some(session) = Self::new_with_controller(sketch, request, config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(session))
    }

    pub(crate) fn new_with_controller(
        mut sketch: Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        controller: &mut geosolve_core::OperationController,
    ) -> Result<Option<Self>, SketchSessionError> {
        let validation_sketch = sketch.clone();
        let previous_state = PreviousStateReference::capture(&validation_sketch);
        let Some(solve) = sketch.solve_with_previous_state_reference_and_controller(
            request,
            config,
            &previous_state,
            controller,
        )?
        else {
            return Ok(None);
        };
        let Some(session) = Self::from_accepted_solve_inner(
            sketch,
            &validation_sketch,
            request,
            config,
            solve,
            &previous_state,
            Some(controller),
        )?
        else {
            return Ok(None);
        };
        if controller
            .checkpoint(geosolve_core::OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(session))
    }

    pub(crate) fn from_accepted_solve_with_controller(
        sketch: Sketch,
        validation_sketch: &Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        solve: SketchSolveResult,
        previous_state: &PreviousStateReference,
        controller: &mut geosolve_core::OperationController,
    ) -> Result<Option<Self>, SketchSessionError> {
        Self::from_accepted_solve_inner(
            sketch,
            validation_sketch,
            request,
            config,
            solve,
            previous_state,
            Some(controller),
        )
    }

    fn from_accepted_solve_inner(
        sketch: Sketch,
        validation_sketch: &Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        solve: SketchSolveResult,
        previous_state: &PreviousStateReference,
        controller: Option<&mut geosolve_core::OperationController>,
    ) -> Result<Option<Self>, SketchSessionError> {
        if let Some(rejection) = solve.rejection {
            return Err(SketchSessionError::InitialRejected(rejection));
        }
        let config = acceptance_solver_config(config);
        let Some(mut compiled) = (if let Some(controller) = controller {
            sketch.compile_with_previous_state_reference_and_controller(
                request,
                previous_state,
                controller,
            )?
        } else {
            Some(sketch.compile_with_previous_state_reference(request, previous_state)?)
        }) else {
            return Ok(None);
        };
        let mut core = SolveSession::from_accepted_report(
            compiled.problem().clone(),
            config,
            solve.core_report,
        )?;
        compiled.replace_problem(core.problem().clone());
        let mut audit_refresh = AcceptedAuditPatch::new(core.revisions());
        copy_changed_constraint_audits(
            &core,
            &compiled,
            validation_sketch,
            &sketch,
            request,
            previous_state,
            &mut audit_refresh,
        )?;
        core.refresh_accepted_audit(audit_refresh)?;
        compiled.replace_problem(core.problem().clone());
        let report = core.report().clone();
        let geometry = sketch.geometry();
        let accepted_result = SketchSolveResult {
            attempted_geometry: Some(geometry.clone()),
            geometry,
            display_audit: report.audit.clone(),
            reference_values: sketch.reference_values()?,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            diagnostic_variable_owners: compiled.diagnostic_variable_owners(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: solve.acceptance_hard_residual_max,
        };
        Ok(Some(Self {
            sketch,
            request,
            compiled,
            core,
            accepted_result,
            revision: 0,
            revisions: SketchSessionRevisions::default(),
            topology_compilations: 1,
            previous_state_reference: previous_state.clone(),
            last_execution: SketchSessionExecutionKind::InitialSolve,
        }))
    }

    #[must_use]
    pub const fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    #[must_use]
    pub const fn request(&self) -> SketchSolveRequest {
        self.request
    }

    pub(crate) const fn previous_state_reference(&self) -> &PreviousStateReference {
        &self.previous_state_reference
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
    pub fn execution_summary(&self) -> SketchSessionExecutionSummary {
        SketchSessionExecutionSummary {
            kind: self.last_execution,
            component_count: self.accepted_result.core_report.component_solves.len(),
            reused_component_count: self
                .accepted_result
                .core_report
                .component_solves
                .iter()
                .filter(|component| component.reused)
                .count(),
            freshly_validated_hard_rows: self.accepted_result.core_report.hard_residuals_validated,
            rank_valid: self.accepted_result.core_report.rank_is_valid,
        }
    }

    /// Assesses the accepted runtime against the bounded production rank envelope.
    ///
    /// Sparse hard steps remain available inside this envelope. Numerical rank is
    /// deliberately not claimed as sparse-authoritative: dense SVD remains the
    /// oracle, bounded by the same safe-Rust controlled-kernel component limit.
    #[must_use]
    pub fn production_scale_assessment(&self) -> SketchProductionScaleAssessment {
        let maximum_active_rows = self
            .accepted_result
            .core_report
            .structural
            .component_summaries
            .iter()
            .map(|component| component.active_rows)
            .max()
            .unwrap_or(0);
        let maximum_active_tangent_dimensions = self
            .accepted_result
            .core_report
            .structural
            .component_summaries
            .iter()
            .map(|component| component.active_tangent_dimensions)
            .max()
            .unwrap_or(0);
        let component_limit = geosolve_core::CONTROLLED_DENSE_KERNEL_MAX_DIMENSION;
        SketchProductionScaleAssessment {
            authority: SketchRankAuthority::BoundedDenseSvd,
            supported: self.accepted_result.core_report.rank_is_valid
                && maximum_active_rows <= component_limit
                && maximum_active_tangent_dimensions <= component_limit,
            component_limit,
            component_count: self
                .accepted_result
                .core_report
                .structural
                .component_summaries
                .len(),
            maximum_active_rows,
            maximum_active_tangent_dimensions,
        }
    }

    /// Plans deterministic passive-freedom ownership for a projected point drag.
    ///
    /// The plan is derived from the freshly validated accepted hard-equality nullspace. Active
    /// bounds and their one-sided tangent cone remain core-owned solve state; the locality planner
    /// does not reinterpret them as equality directions. Candidate anchors are considered in point
    /// compilation order. At each step the planner chooses the point that adds the most uncovered
    /// nullspace rank, then the point with the lower total mobility rank, then the earlier point.
    /// This favors an independent one-DOF input over a dependent multi-DOF output when both cover
    /// the same passive direction.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for an unknown active-point mapping, non-finite or numerically
    /// ambiguous rank evidence, inconsistent accepted mappings, or an incomplete point-anchor
    /// cover. A mapped point that is fully fixed or eliminated receives a valid zero-DOF empty
    /// plan. Passive equality freedom visible only in scalars or contact parameters intentionally
    /// fails closed. No partial plan is returned.
    #[cfg(test)]
    pub(crate) fn drag_locality_plan(
        &self,
        point: PointId,
    ) -> Result<SketchDragLocalityPlan, SketchSessionError> {
        let mut controller =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::unlimited());
        self.drag_locality_plan_with_controller(point, &mut controller)?
            .ok_or(SketchSessionError::DragLocalityUnavailable {
                context: "uncontrolled locality planning was interrupted",
            })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn drag_locality_plan_with_controller(
        &self,
        point: PointId,
        controller: &mut geosolve_core::OperationController,
    ) -> Result<Option<SketchDragLocalityPlan>, SketchSessionError> {
        let point_variable = self
            .compiled
            .variable_for_point(point)
            .ok_or(SketchError::UnknownPoint(point))?;
        let Some((active_hard_rows, active_tangent_dimensions)) =
            self.core.accepted_hard_component_dimensions(point_variable)
        else {
            return Ok(Some(SketchDragLocalityPlan {
                point,
                hard_degrees_of_freedom: 0,
                active_rank: 0,
                passive_degrees_of_freedom: 0,
                anchors: Vec::new(),
            }));
        };
        validate_drag_locality_component_envelope(active_hard_rows, active_tangent_dimensions)?;
        let Some((component, nullspace)) = self
            .core
            .accepted_hard_component_nullspace_with_controller(point_variable, controller)?
        else {
            return Ok(None);
        };
        let Some(active_block) = component.tangent_blocks().iter().find(|block| {
            block.root == point_variable || block.alias_members.contains(&point_variable)
        }) else {
            return Ok(Some(SketchDragLocalityPlan {
                point,
                hard_degrees_of_freedom: 0,
                active_rank: 0,
                passive_degrees_of_freedom: 0,
                anchors: Vec::new(),
            }));
        };
        if active_block.kind != geosolve_core::VariableKind::Vec2
            || active_block.tangent_range.len() != 2
        {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "the active point does not have a two-coordinate tangent block",
            });
        }

        let tangent_dimensions = component
            .tangent_blocks()
            .last()
            .map_or(0, |block| block.tangent_range.end);
        validate_drag_locality_component_envelope(0, tangent_dimensions)?;
        let hard_degrees_of_freedom = nullspace.right_nullity;
        if nullspace.vectors.len() != hard_degrees_of_freedom {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "the accepted hard nullspace basis has inconsistent dimensions",
            });
        }
        if nullspace
            .vectors
            .iter()
            .any(|vector| vector.normalized_tangent.len() != tangent_dimensions)
        {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "the accepted hard nullspace does not match the tangent layout",
            });
        }

        let rank_tolerance = locality_rank_tolerance(
            self.core.config().rank_relative_tolerance,
            hard_degrees_of_freedom,
        )?;
        let active_rows = point_nullspace_response(&nullspace, active_block.tangent_range.clone())?;
        let mut covered =
            locality_row_basis(&active_rows, hard_degrees_of_freedom, rank_tolerance)?;
        let active_rank = covered.len();
        let passive_degrees_of_freedom = hard_degrees_of_freedom.saturating_sub(active_rank);
        if passive_degrees_of_freedom == 0 {
            return Ok(Some(SketchDragLocalityPlan {
                point,
                hard_degrees_of_freedom,
                active_rank,
                passive_degrees_of_freedom,
                anchors: Vec::new(),
            }));
        }

        let mut candidates = Vec::new();
        let mut component_points = Vec::new();
        for block in component.tangent_blocks() {
            for variable in std::iter::once(block.root).chain(block.alias_members.iter().copied()) {
                let Some((order, mapping)) = self.compiled.point_mapping_for_variable(variable)
                else {
                    continue;
                };
                component_points.push((
                    order,
                    mapping.point_id,
                    block.kind,
                    block.tangent_range.clone(),
                ));
            }
        }
        component_points.sort_by_key(|(order, _, _, _)| *order);
        for (order, candidate_point, kind, tangent_range) in component_points {
            if controller
                .charge(
                    geosolve_core::OperationWorkCounter::DocumentDependencyItems,
                    1,
                    geosolve_core::OperationCheckpoint::DocumentDependency,
                )
                .is_err()
            {
                return Ok(None);
            }
            if candidate_point == point {
                continue;
            }
            if kind != geosolve_core::VariableKind::Vec2 || tangent_range.len() != 2 {
                return Err(SketchSessionError::DragLocalityUnavailable {
                    context: "a candidate point has an invalid accepted tangent block",
                });
            }
            let rows = point_nullspace_response(&nullspace, tangent_range)?;
            let mobility_rank =
                locality_row_basis(&rows, hard_degrees_of_freedom, rank_tolerance)?.len();
            if mobility_rank == 0 {
                continue;
            }
            candidates.push(DragLocalityCandidate {
                point: candidate_point,
                rows,
                mobility_rank,
                order,
            });
        }

        let mut anchors = Vec::new();
        while covered.len() < hard_degrees_of_freedom {
            let mut best: Option<(usize, usize, usize, Vec<Vec<f64>>)> = None;
            for candidate in &candidates {
                if anchors
                    .iter()
                    .any(|anchor: &SketchDragLocalityAnchor| anchor.point == candidate.point)
                {
                    continue;
                }
                if controller
                    .charge(
                        geosolve_core::OperationWorkCounter::DocumentDependencyItems,
                        1,
                        geosolve_core::OperationCheckpoint::DocumentDependency,
                    )
                    .is_err()
                {
                    return Ok(None);
                }
                let expanded = extend_locality_row_basis(
                    &covered,
                    &candidate.rows,
                    hard_degrees_of_freedom,
                    rank_tolerance,
                )?;
                let gain = expanded.len().saturating_sub(covered.len());
                if gain == 0 {
                    continue;
                }
                let replace =
                    best.as_ref()
                        .is_none_or(|(best_gain, best_mobility_rank, best_order, _)| {
                            gain > *best_gain
                                || (gain == *best_gain
                                    && (candidate.mobility_rank < *best_mobility_rank
                                        || (candidate.mobility_rank == *best_mobility_rank
                                            && candidate.order < *best_order)))
                        });
                if replace {
                    best = Some((gain, candidate.mobility_rank, candidate.order, expanded));
                }
            }
            let Some((_, _, best_order, expanded)) = best else {
                return Err(SketchSessionError::DragLocalityIncomplete {
                    required: hard_degrees_of_freedom,
                    spanned: covered.len(),
                });
            };
            let candidate = candidates
                .iter()
                .find(|candidate| candidate.order == best_order)
                .expect("selected locality candidate retains its compile-order identity");
            anchors.push(SketchDragLocalityAnchor {
                point: candidate.point,
                mobility_rank: candidate.mobility_rank,
            });
            covered = expanded;
        }

        Ok(Some(SketchDragLocalityPlan {
            point,
            hard_degrees_of_freedom,
            active_rank,
            passive_degrees_of_freedom,
            anchors,
        }))
    }

    pub(crate) fn mark_full_rebuild(&mut self) {
        self.last_execution = SketchSessionExecutionKind::FullRebuild;
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

    /// Returns the accepted raw core bound audit for one domain bound.
    ///
    /// This is an explicitly unstable advanced-diagnostic seam. Stable document
    /// consumers should use [`crate::SketchDiagnosticSnapshot::bounds`].
    #[must_use]
    pub fn unstable_bound_report(
        &self,
        bound: crate::SketchBound,
    ) -> Option<&geosolve_core::BoundReport> {
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
    ///
    /// # Panics
    ///
    /// Panics only if the internal uncontrolled operation path reports interruption.
    #[allow(clippy::too_many_lines)]
    pub fn apply_patch(
        &mut self,
        patch: SketchSessionPatch,
    ) -> Result<SketchSolveResult, SketchSessionError> {
        self.apply_patch_inner(patch, None).map(|result| {
            result.expect("uncontrolled sketch session application cannot be interrupted")
        })
    }

    #[allow(clippy::too_many_lines)]
    fn apply_patch_inner(
        &mut self,
        patch: SketchSessionPatch,
        controller: Option<&mut OperationController>,
    ) -> Result<Option<SketchSolveResult>, SketchSessionError> {
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
        // A retained DragTarget request is one gesture, not a sequence of unrelated solves.
        // The accepted sketch/core state is still the continuation seed, but its solved
        // coordinates must never become new PreviousState intent. A request rebuild owns the
        // lifecycle boundary that clears or replaces the drag and captures a fresh reference.
        let candidate_previous_state = if same_drag_gesture(self.request, candidate_request) {
            self.previous_state_reference.clone()
        } else {
            PreviousStateReference::capture(&candidate_sketch)
        };

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
            let position = candidate_previous_state.point_position(point)?;
            if point_bits(self.previous_state_reference.point_position(point)?)
                != point_bits(position)
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
            if let Some(source_patch) = self.compiled.source_patch(
                &candidate_sketch,
                candidate_request,
                *source,
                &candidate_previous_state,
            )? {
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
        let domain_source_changed =
            edit_changes_source(patch.edit) || !replacement_sources.is_empty();

        self.finish_incremental_candidate(
            candidate_sketch,
            candidate_request,
            candidate_previous_state,
            core_patch,
            replacement_source_labels,
            bound_changed,
            domain_source_changed,
            controller,
        )
    }

    /// Applies a fully lowered scratch candidate while retaining compatible runtime
    /// identities, component caches, and symbolic storage.
    ///
    /// The caller supplies the exact source closure whose evaluator payloads may
    /// have changed. Every changed variable value is derived by comparing the
    /// scratch compilation with the retained compilation. A topology mismatch is
    /// reported explicitly so the document owner can take its full-rebuild path.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply_compatible_candidate(
        &mut self,
        candidate_sketch: Sketch,
        candidate_request: SketchSolveRequest,
        replacement_sources: &[SketchSource],
        previous_state: &PreviousStateReference,
        mut controller: Option<&mut OperationController>,
    ) -> Result<Option<SketchSolveResult>, SketchSessionError> {
        candidate_sketch.preflight_segments()?;
        let candidate_compiled = match controller.as_deref_mut() {
            Some(controller) => candidate_sketch
                .compile_with_previous_state_reference_and_controller(
                    candidate_request,
                    previous_state,
                    controller,
                )?,
            None => Some(
                candidate_sketch
                    .compile_with_previous_state_reference(candidate_request, previous_state)?,
            ),
        };
        let Some(candidate_compiled) = candidate_compiled else {
            return Ok(None);
        };
        if !self
            .compiled
            .has_compatible_runtime_topology(&candidate_compiled)
        {
            return Err(SketchSessionError::RebuildRequired);
        }

        let mut core_patch = SessionPatch::new(self.core.revisions());
        for variable in self.compiled.shape_variable_ids() {
            let retained = self
                .core
                .problem()
                .variable(variable)
                .ok_or(geosolve_core::CoreError::UnknownVariable(variable))?
                .value();
            let candidate = candidate_compiled
                .problem()
                .variable(variable)
                .ok_or(geosolve_core::CoreError::UnknownVariable(variable))?
                .value();
            if retained != candidate {
                core_patch.set_variable_value(variable, candidate);
            }
        }

        let mut replacement_sources = replacement_sources.to_vec();
        if let (Some(retained_drag), Some(candidate_drag)) =
            (self.request.drag, candidate_request.drag)
            && retained_drag.point == candidate_drag.point
            && point_bits(retained_drag.target) != point_bits(candidate_drag.target)
        {
            push_unique(
                &mut replacement_sources,
                SketchSource::DragTarget(candidate_drag.point),
            );
        }
        for mapping in self.compiled.source_mappings() {
            let SketchSource::PreviousState(point) = mapping.source else {
                continue;
            };
            let position = previous_state.point_position(point)?;
            if point_bits(self.previous_state_reference.point_position(point)?)
                != point_bits(position)
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

        let mut replacement_source_labels = Vec::new();
        let mut bound_changed = false;
        for source in &replacement_sources {
            if let Some(source_patch) = self.compiled.source_patch(
                &candidate_sketch,
                candidate_request,
                *source,
                previous_state,
            )? {
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
        self.finish_incremental_candidate(
            candidate_sketch,
            candidate_request,
            previous_state.clone(),
            core_patch,
            replacement_source_labels,
            bound_changed,
            !replacement_sources.is_empty(),
            controller,
        )
    }

    #[allow(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        clippy::too_many_lines
    )]
    fn finish_incremental_candidate(
        &mut self,
        candidate_sketch: Sketch,
        candidate_request: SketchSolveRequest,
        candidate_previous_state: PreviousStateReference,
        core_patch: SessionPatch,
        replacement_source_labels: Vec<(SketchSource, String)>,
        bound_changed: bool,
        mut domain_source_changed: bool,
        mut controller: Option<&mut OperationController>,
    ) -> Result<Option<SketchSolveResult>, SketchSessionError> {
        let validation_compiled = self.compiled.clone();
        let validation_template = candidate_sketch.clone();
        let mut candidate_core = self.core.clone();
        let validation_tolerance = candidate_core.config().normalized_residual_tolerance;
        let transaction = match controller.as_deref_mut() {
            Some(controller) => candidate_core.apply_with_output_controlled(
                core_patch,
                |problem, report| {
                    complete_candidate_for_problem(
                        problem,
                        report,
                        &validation_compiled,
                        &validation_template,
                        candidate_request,
                        validation_tolerance,
                    )
                },
                controller,
            )?,
            None => Some(
                candidate_core.apply_with_output(core_patch, |problem, report| {
                    complete_candidate_for_problem(
                        problem,
                        report,
                        &validation_compiled,
                        &validation_template,
                        candidate_request,
                        validation_tolerance,
                    )
                })?,
            ),
        };
        let Some((transaction, complete_candidate)) = transaction else {
            return Ok(None);
        };

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
            return Ok(Some(SketchSolveResult {
                geometry: self.sketch.geometry(),
                attempted_geometry: None,
                display_audit: self.accepted_result.display_audit.clone(),
                reference_values: self.accepted_result.reference_values.clone(),
                source_mappings: self.accepted_result.source_mappings.clone(),
                bound_mappings: self.accepted_result.bound_mappings.clone(),
                diagnostic_variable_owners: self.accepted_result.diagnostic_variable_owners.clone(),
                core_report: transaction.report,
                rejection: Some(rejection),
                acceptance_hard_residual_max: acceptance_max,
            }));
        }

        let _ = complete_candidate.ok_or(SketchSessionError::MissingCandidate)?;
        let complete = match finalize_solved_candidate_controlled(
            &mut candidate_core,
            &validation_compiled,
            &validation_template,
            candidate_request,
            controller.as_deref_mut(),
        )? {
            None => return Ok(None),
            Some(Ok(complete)) => complete,
            Some(Err((sync_report, rejection))) => {
                let acceptance_max = crate::compiler::rejection_residual_max(&rejection)
                    .map(|maximum| maximum.max(sync_report.hard_residual_max));
                return Ok(Some(SketchSolveResult {
                    geometry: self.sketch.geometry(),
                    attempted_geometry: None,
                    display_audit: self.accepted_result.display_audit.clone(),
                    reference_values: self.accepted_result.reference_values.clone(),
                    source_mappings: self.accepted_result.source_mappings.clone(),
                    bound_mappings: self.accepted_result.bound_mappings.clone(),
                    diagnostic_variable_owners: self
                        .accepted_result
                        .diagnostic_variable_owners
                        .clone(),
                    core_report: sync_report,
                    rejection: Some(rejection),
                    acceptance_hard_residual_max: acceptance_max,
                }));
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
            &candidate_previous_state,
            &mut audit_refresh,
        )?;
        candidate_core.refresh_accepted_audit(audit_refresh)?;
        accepted_compiled.replace_problem(candidate_core.problem().clone());
        domain_source_changed |= audit_changed;
        let report = candidate_core.report().clone();
        let result = SketchSolveResult {
            attempted_geometry: Some(complete.geometry.clone()),
            geometry: complete.geometry,
            display_audit: report.audit.clone(),
            reference_values: complete.reference_values,
            source_mappings: accepted_compiled.source_mappings().to_vec(),
            bound_mappings: accepted_compiled.bound_mappings().to_vec(),
            diagnostic_variable_owners: accepted_compiled.diagnostic_variable_owners(),
            acceptance_hard_residual_max: Some(
                report
                    .hard_residual_max
                    .max(complete.independent_hard_residual_max),
            ),
            core_report: report,
            rejection: None,
        };
        if let Some(controller) = controller
            && controller
                .checkpoint(OperationCheckpoint::BeforeCommit)
                .is_err()
        {
            return Ok(None);
        }
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
        self.previous_state_reference = candidate_previous_state;
        self.accepted_result = result.clone();
        self.last_execution = SketchSessionExecutionKind::IncrementalUpdate;
        Ok(Some(result))
    }

    /// Controlled counterpart to [`Self::apply_patch`].
    ///
    /// # Errors
    ///
    /// Returns the same patch and solve setup errors as [`Self::apply_patch`].
    pub fn apply_patch_controlled(
        &mut self,
        patch: SketchSessionPatch,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchSolveResult>, SketchSessionError> {
        if patch.expected_revision != self.revision {
            return Err(SketchSessionError::StalePatch {
                expected: patch.expected_revision,
                actual: self.revision,
            });
        }
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let mut candidate = self.clone();
        let Some(result) = candidate.apply_patch_inner(patch, Some(&mut controller))? else {
            return Ok(controller.outcome_unchecked());
        };
        if result.accepted() {
            if controller
                .checkpoint(OperationCheckpoint::BeforeCommit)
                .is_err()
            {
                return Ok(controller.outcome_unchecked());
            }
            *self = candidate;
        }
        Ok(controller.outcome(result))
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

    /// Controlled counterpart to [`Self::rebuild_request`].
    ///
    /// # Errors
    ///
    /// Returns the same rebuild errors as [`Self::rebuild_request`].
    pub fn rebuild_request_controlled(
        &mut self,
        expected_revision: u64,
        request: SketchSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchSolveResult>, SketchSessionError> {
        self.rebuild_controlled(expected_revision, self.sketch.clone(), request, control)
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
        let validation_sketch = sketch.clone();
        let previous_state = PreviousStateReference::capture(&validation_sketch);
        let mut compiled =
            sketch.compile_with_previous_state_reference(request, &previous_state)?;
        let mut candidate_core = self.core.clone();
        candidate_core.rebuild(self.core.revisions(), compiled.problem().clone())?;
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
            &previous_state,
            &mut audit_refresh,
        )?;
        candidate_core.refresh_accepted_audit(audit_refresh)?;
        compiled.replace_problem(candidate_core.problem().clone());
        let report = candidate_core.report().clone();
        let geometry = accepted_sketch.geometry();
        let accepted_result = SketchSolveResult {
            attempted_geometry: Some(geometry.clone()),
            geometry,
            display_audit: report.audit.clone(),
            reference_values,
            source_mappings: compiled.source_mappings().to_vec(),
            bound_mappings: compiled.bound_mappings().to_vec(),
            diagnostic_variable_owners: compiled.diagnostic_variable_owners(),
            core_report: report,
            rejection: None,
            acceptance_hard_residual_max: Some(
                candidate_core
                    .report()
                    .hard_residual_max
                    .max(independent_hard_residual_max),
            ),
        };
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
            previous_state_reference: previous_state,
            last_execution: SketchSessionExecutionKind::FullRebuild,
        };
        *self = rebuilt;
        Ok(&self.accepted_result)
    }

    /// Controlled counterpart to [`Self::rebuild`].
    ///
    /// # Errors
    ///
    /// Returns the same rebuild errors as [`Self::rebuild`].
    pub fn rebuild_controlled(
        &mut self,
        expected_revision: u64,
        sketch: Sketch,
        request: SketchSolveRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchSolveResult>, SketchSessionError> {
        if expected_revision != self.revision {
            return Err(SketchSessionError::StalePatch {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let mut controller = OperationController::new(control);
        self.rebuild_with_controller(sketch, request, &mut controller)
    }

    fn rebuild_with_controller(
        &mut self,
        mut sketch: Sketch,
        request: SketchSolveRequest,
        controller: &mut OperationController,
    ) -> Result<OperationOutcome<SketchSolveResult>, SketchSessionError> {
        if controller
            .checkpoint(OperationCheckpoint::DocumentLowering)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let validation_sketch = sketch.clone();
        let previous_state = PreviousStateReference::capture(&validation_sketch);
        let Some(solve) = sketch.solve_with_previous_state_reference_and_controller(
            request,
            self.core.config(),
            &previous_state,
            controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        if !solve.accepted() {
            return Ok(controller.outcome(solve));
        }
        let Some(mut rebuilt) = Self::from_accepted_solve_with_controller(
            sketch,
            &validation_sketch,
            request,
            self.core.config(),
            solve,
            &previous_state,
            controller,
        )?
        else {
            return Ok(controller.outcome_unchecked());
        };
        rebuilt.revision = self.revision.saturating_add(1);
        rebuilt.revisions = SketchSessionRevisions {
            topology: self.revisions.topology.saturating_add(1),
            source: self.revisions.source.saturating_add(1),
            state: self.revisions.state.saturating_add(1),
            bound: self.revisions.bound.saturating_add(1),
        };
        rebuilt.topology_compilations = self.topology_compilations.saturating_add(1);
        rebuilt.last_execution = SketchSessionExecutionKind::FullRebuild;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let result = rebuilt.accepted_result.clone();
        *self = rebuilt;
        Ok(controller.outcome(result))
    }
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
    complete_materialized_candidate_for_problem(candidate, report, template, request, tolerance)
}

fn complete_current_candidate_for_problem_controlled(
    problem: &geosolve_core::Problem,
    report: &geosolve_core::SolveReport,
    compiled: &CompiledSketch,
    template: &Sketch,
    request: SketchSolveRequest,
    tolerance: f64,
    controller: &mut OperationController,
) -> Result<Option<CompleteSketchCandidate>, SessionDomainRejection<SolveRejection>> {
    if controller
        .charge(
            OperationWorkCounter::DocumentLoweringItems,
            candidate_materialization_items(compiled),
            OperationCheckpoint::DocumentLowering,
        )
        .is_err()
    {
        return Ok(None);
    }
    let mut candidate = compiled
        .solved_state_for_problem(problem, template)
        .map_err(|error| {
            session_domain_rejection(SolveRejection::IndependentValidationFailed(
                error.to_string(),
            ))
        })?;
    if template.normalize_candidate_latents(&mut candidate) {
        return Err(session_domain_rejection(
            SolveRejection::IndependentValidationFailed(
                "no-motion certification would normalize a latent coordinate".into(),
            ),
        ));
    }
    if controller
        .charge(
            OperationWorkCounter::DocumentValidationItems,
            candidate_validation_items(compiled),
            OperationCheckpoint::BeforeFinalValidation,
        )
        .is_err()
    {
        return Ok(None);
    }
    let complete = complete_materialized_candidate_for_problem(
        candidate, report, template, request, tolerance,
    );
    if controller
        .checkpoint(OperationCheckpoint::AfterFinalValidation)
        .is_err()
    {
        return Ok(None);
    }
    complete.map(Some)
}

fn complete_materialized_candidate_for_problem(
    mut candidate: crate::compiler::SolvedSketchState,
    report: &geosolve_core::SolveReport,
    template: &Sketch,
    request: SketchSolveRequest,
    tolerance: f64,
) -> Result<CompleteSketchCandidate, SessionDomainRejection<SolveRejection>> {
    template
        .derive_curve_fillet_arcs(&mut candidate, tolerance)
        .map_err(session_domain_rejection)?;
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

fn candidate_materialization_items(compiled: &CompiledSketch) -> usize {
    [
        1,
        compiled.point_variables().len(),
        compiled.circle_radius_variables().len(),
        compiled.arc_radius_variables().len(),
        compiled.arc_angle_variables().len(),
        compiled.conic_scalar_variables().len(),
        compiled.conic_vector_variables().len(),
        compiled.nurbs_weight_variables().len(),
        compiled.latent_variables().len(),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

fn candidate_validation_items(compiled: &CompiledSketch) -> usize {
    candidate_materialization_items(compiled)
        .saturating_add(compiled.source_mappings().len())
        .saturating_add(compiled.bound_mappings().len())
}

type CandidateFinalization =
    Result<CompleteSketchCandidate, (geosolve_core::SolveReport, SolveRejection)>;

#[allow(clippy::result_large_err)]
fn finalize_solved_candidate(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    template: &Sketch,
    request: SketchSolveRequest,
) -> Result<CandidateFinalization, SketchSessionError> {
    finalize_solved_candidate_controlled(core, compiled, template, request, None)
        .map(|result| result.expect("uncontrolled candidate finalization cannot be interrupted"))
}

fn finalize_solved_candidate_controlled(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    template: &Sketch,
    request: SketchSolveRequest,
    mut controller: Option<&mut OperationController>,
) -> Result<Option<CandidateFinalization>, SketchSessionError> {
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
                return Ok(Some(Err((report, rejection.reason))));
            }
        };
        let synchronization = match controller.as_deref_mut() {
            Some(controller) => synchronize_accepted_latents_controlled(
                core,
                compiled,
                &complete.normalized_latents,
                controller,
            )?,
            None => Some(synchronize_accepted_latents(
                core,
                compiled,
                &complete.normalized_latents,
            )?),
        };
        let Some(synchronization) = synchronization else {
            return Ok(None);
        };
        match synchronization {
            LatentSynchronization::Unchanged => return Ok(Some(Ok(complete))),
            LatentSynchronization::Committed => {}
            LatentSynchronization::Rejected(report, rejection) => {
                return Ok(Some(Err((*report, rejection))));
            }
        }
    }
    let mut report = core.report().clone();
    report.termination = SolveTermination::Stalled;
    Ok(Some(Err((
        report,
        SolveRejection::CoreTermination(SolveTermination::Stalled),
    ))))
}

fn synchronize_accepted_latents(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    latents: &[SolvedLatent],
) -> Result<LatentSynchronization, SketchSessionError> {
    synchronize_accepted_latents_inner(core, compiled, latents, None)
        .map(|result| result.expect("uncontrolled latent synchronization cannot be interrupted"))
}

fn synchronize_accepted_latents_controlled(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    latents: &[SolvedLatent],
    controller: &mut OperationController,
) -> Result<Option<LatentSynchronization>, SketchSessionError> {
    synchronize_accepted_latents_inner(core, compiled, latents, Some(controller))
}

fn synchronize_accepted_latents_inner(
    core: &mut SolveSession,
    compiled: &CompiledSketch,
    latents: &[SolvedLatent],
    controller: Option<&mut OperationController>,
) -> Result<Option<LatentSynchronization>, SketchSessionError> {
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
        return Ok(Some(LatentSynchronization::Unchanged));
    }
    let transaction = match controller {
        Some(controller) => core
            .apply_with_output_controlled(
                patch,
                |_, _| Ok::<(), SessionDomainRejection<std::convert::Infallible>>(()),
                controller,
            )?
            .map(|(transaction, _)| transaction),
        None => Some(core.apply(patch)?),
    };
    let Some(transaction) = transaction else {
        return Ok(None);
    };
    if transaction.committed() {
        return Ok(Some(LatentSynchronization::Committed));
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
    Ok(Some(LatentSynchronization::Rejected(
        Box::new(transaction.report),
        rejection,
    )))
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
    previous_state: &PreviousStateReference,
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
        let Some(refreshed) =
            accepted.source_patch(after, request, mapping.source, previous_state)?
        else {
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

#[allow(clippy::too_many_arguments)]
fn refresh_changed_constraint_audits_controlled(
    core: &mut SolveSession,
    accepted: &CompiledSketch,
    before: &Sketch,
    after: &Sketch,
    request: SketchSolveRequest,
    previous_state: &PreviousStateReference,
    controller: &mut OperationController,
) -> Result<Option<bool>, SketchSessionError> {
    if controller
        .charge(
            OperationWorkCounter::DocumentLoweringItems,
            accepted.source_mappings().len().saturating_add(1),
            OperationCheckpoint::DocumentLowering,
        )
        .is_err()
    {
        return Ok(None);
    }
    let mut audit_refresh = AcceptedAuditPatch::new(core.revisions());
    let changed = copy_changed_constraint_audits(
        core,
        accepted,
        before,
        after,
        request,
        previous_state,
        &mut audit_refresh,
    )?;
    let audit_items = core
        .report()
        .audit
        .sources
        .iter()
        .fold(1usize, |items, source| {
            items.saturating_add(1).saturating_add(source.rows.len())
        });
    if controller
        .charge(
            OperationWorkCounter::DocumentValidationItems,
            audit_items,
            OperationCheckpoint::BeforeFinalValidation,
        )
        .is_err()
    {
        return Ok(None);
    }
    core.refresh_accepted_audit(audit_refresh)?;
    if controller
        .checkpoint(OperationCheckpoint::AfterFinalValidation)
        .is_err()
    {
        return Ok(None);
    }
    Ok(Some(changed))
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

fn same_drag_gesture(retained: SketchSolveRequest, candidate: SketchSolveRequest) -> bool {
    matches!(
        (retained.drag, candidate.drag),
        (Some(retained), Some(candidate)) if retained.point == candidate.point
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

fn locality_rank_tolerance(
    relative_tolerance: f64,
    dimensions: usize,
) -> Result<f64, SketchSessionError> {
    if !relative_tolerance.is_finite() || relative_tolerance <= 0.0 {
        return Err(SketchSessionError::DragLocalityUnavailable {
            context: "the accepted rank tolerance is invalid",
        });
    }
    Ok(relative_tolerance.max(
        64.0 * f64::EPSILON
            * f64::from(u32::try_from(dimensions.max(1)).map_err(|_| {
                SketchSessionError::DragLocalityUnavailable {
                    context: "the accepted hard nullity exceeds the supported locality envelope",
                }
            })?),
    ))
}

fn point_nullspace_response(
    nullspace: &geosolve_core::AcceptedNullspaceBasis,
    tangent_range: std::ops::Range<usize>,
) -> Result<Vec<Vec<f64>>, SketchSessionError> {
    let mut rows = Vec::with_capacity(tangent_range.len());
    for coordinate in tangent_range {
        let row = nullspace
            .vectors
            .iter()
            .map(|vector| vector.normalized_tangent.get(coordinate).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or(SketchSessionError::DragLocalityUnavailable {
                context: "a point response lies outside the accepted tangent layout",
            })?;
        if row.iter().any(|value| !value.is_finite()) {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "a point response contains non-finite mobility evidence",
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

fn locality_row_basis(
    rows: &[Vec<f64>],
    dimensions: usize,
    tolerance: f64,
) -> Result<Vec<Vec<f64>>, SketchSessionError> {
    extend_locality_row_basis(&[], rows, dimensions, tolerance)
}

fn extend_locality_row_basis(
    basis: &[Vec<f64>],
    rows: &[Vec<f64>],
    dimensions: usize,
    tolerance: f64,
) -> Result<Vec<Vec<f64>>, SketchSessionError> {
    let mut result = basis.to_vec();
    for row in rows {
        if row.len() != dimensions
            || row.iter().any(|value| !value.is_finite())
            || result.iter().any(|retained| retained.len() != dimensions)
        {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "mobility rank evidence has invalid dimensions or finiteness",
            });
        }
        let original_norm = finite_vector_norm(row)?;
        if original_norm <= tolerance {
            continue;
        }
        let mut candidate = row.clone();
        for _ in 0..2 {
            for retained in &result {
                let projection = dot_product(&candidate, retained)?;
                for (value, direction) in candidate.iter_mut().zip(retained) {
                    *value -= projection * direction;
                }
            }
        }
        let norm = finite_vector_norm(&candidate)?;
        let threshold = tolerance * original_norm.max(1.0);
        if norm <= threshold {
            continue;
        }
        if norm <= 100.0 * threshold {
            return Err(SketchSessionError::DragLocalityUnavailable {
                context: "point mobility rank is numerically ambiguous",
            });
        }
        for value in &mut candidate {
            *value /= norm;
        }
        result.push(candidate);
    }
    Ok(result)
}

fn finite_vector_norm(values: &[f64]) -> Result<f64, SketchSessionError> {
    let norm = values
        .iter()
        .fold(0.0_f64, |accumulator, value| accumulator.hypot(*value));
    if !norm.is_finite() {
        return Err(SketchSessionError::DragLocalityUnavailable {
            context: "mobility rank norm is non-finite",
        });
    }
    Ok(norm)
}

fn dot_product(first: &[f64], second: &[f64]) -> Result<f64, SketchSessionError> {
    if first.len() != second.len() {
        return Err(SketchSessionError::DragLocalityUnavailable {
            context: "mobility rank vectors have inconsistent dimensions",
        });
    }
    let value = first
        .iter()
        .zip(second)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if !value.is_finite() {
        return Err(SketchSessionError::DragLocalityUnavailable {
            context: "mobility rank projection is non-finite",
        });
    }
    Ok(value)
}

fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn validate_drag_locality_component_envelope(
    active_hard_rows: usize,
    active_tangent_dimensions: usize,
) -> Result<(), SketchSessionError> {
    let limit = geosolve_core::CONTROLLED_DENSE_KERNEL_MAX_DIMENSION;
    if active_hard_rows > limit {
        return Err(SketchSessionError::DragLocalityRowEnvelopeExceeded {
            active_hard_rows,
            limit,
        });
    }
    if active_tangent_dimensions > limit {
        return Err(SketchSessionError::DragLocalityEnvelopeExceeded {
            active_tangent_dimensions,
            limit,
        });
    }
    Ok(())
}

#[cfg(test)]
mod drag_locality_tests {
    use super::*;
    use crate::{AlphaScenarioIds, AlphaScenarioKind, alpha_scenario};

    fn runtime_session(
        kind: AlphaScenarioKind,
    ) -> (SketchSession, crate::DocumentRuntimeMap, AlphaScenarioIds) {
        let fixture = alpha_scenario(kind, 1.0).expect("sample");
        let lowered = fixture.document.lower().expect("lowered sample");
        let (sketch, mappings) = lowered.into_parts();
        let session =
            SketchSession::new(sketch, SketchSolveRequest::new(), SolverConfig::default())
                .expect("accepted runtime");
        (session, mappings, fixture.ids)
    }

    fn assert_exact_certification_exhausted_at(
        sketch: &Sketch,
        request: SketchSolveRequest,
        config: SolverConfig,
        previous_state: &PreviousStateReference,
        counter: OperationWorkCounter,
        limit: usize,
        checkpoint: OperationCheckpoint,
    ) {
        let mut limits = geosolve_core::OperationLimits::unlimited();
        match counter {
            OperationWorkCounter::DocumentLoweringItems => {
                limits.document_lowering_items = limit;
            }
            OperationWorkCounter::DocumentValidationItems => {
                limits.document_validation_items = limit;
            }
            _ => unreachable!("test only exercises document publication counters"),
        }
        let mut controller = OperationController::new(OperationControl::new(
            geosolve_core::CancellationToken::default(),
            limits,
        ));
        assert!(
            SketchSession::certify_current_state_with_previous_state_reference_and_controller(
                sketch.clone(),
                request,
                config,
                previous_state,
                &mut controller,
            )
            .expect("controlled exact certification")
            .is_none()
        );
        assert_eq!(
            controller.report().stopping_reason,
            Some(geosolve_core::OperationStopReason::WorkExhausted {
                counter,
                checkpoint,
            })
        );
    }

    #[test]
    fn exact_state_certification_accounts_for_every_post_core_stage() {
        let mut sketch = Sketch::new(1.0).expect("sketch");
        let point = sketch
            .add_point(Point2::new(2.0, -3.0))
            .expect("fixed point");
        sketch.add_fixed_point(point).expect("fixed source");
        let request = SketchSolveRequest::new().without_previous_state_preferences();
        let session =
            SketchSession::new(sketch, request, SolverConfig::default()).expect("accepted runtime");
        let sketch = session.sketch.clone();
        let previous_state = session.previous_state_reference.clone();
        let config = session.core.config();

        let mut prefix = OperationController::new(OperationControl::new(
            geosolve_core::CancellationToken::default(),
            geosolve_core::OperationLimits::default(),
        ));
        let compiled = sketch
            .compile_with_previous_state_reference_and_controller(
                request,
                &previous_state,
                &mut prefix,
            )
            .expect("prefix compile")
            .expect("prefix compile completed");
        compiled
            .problem()
            .certify_current_state_with_controller(config, &mut prefix)
            .expect("prefix certification")
            .expect("prefix certification completed");
        let prefix_work = prefix.report().consumed;

        let mut full = OperationController::new(OperationControl::unlimited());
        let certified =
            SketchSession::certify_current_state_with_previous_state_reference_and_controller(
                sketch.clone(),
                request,
                config,
                &previous_state,
                &mut full,
            )
            .expect("exact certification")
            .expect("exact certification completed");
        assert_eq!(
            certified.execution_summary().kind,
            SketchSessionExecutionKind::NoMotionCertification
        );
        let full_work = full.report().consumed;
        assert!(
            full_work.document_lowering_items
                >= prefix_work
                    .document_lowering_items
                    .saturating_add(candidate_materialization_items(&compiled))
                    .saturating_add(compiled.source_mappings().len())
                    .saturating_add(1)
        );
        assert!(
            full_work.document_validation_items
                > prefix_work
                    .document_validation_items
                    .saturating_add(candidate_validation_items(&compiled))
        );

        let cases = [
            (
                OperationWorkCounter::DocumentLoweringItems,
                prefix_work
                    .document_lowering_items
                    .saturating_add(candidate_materialization_items(&compiled))
                    .saturating_sub(1),
                OperationCheckpoint::DocumentLowering,
            ),
            (
                OperationWorkCounter::DocumentValidationItems,
                prefix_work
                    .document_validation_items
                    .saturating_add(candidate_validation_items(&compiled))
                    .saturating_sub(1),
                OperationCheckpoint::BeforeFinalValidation,
            ),
            (
                OperationWorkCounter::DocumentLoweringItems,
                full_work.document_lowering_items.saturating_sub(1),
                OperationCheckpoint::DocumentLowering,
            ),
            (
                OperationWorkCounter::DocumentValidationItems,
                full_work.document_validation_items.saturating_sub(1),
                OperationCheckpoint::BeforeFinalValidation,
            ),
        ];
        for (counter, limit, checkpoint) in cases {
            assert_exact_certification_exhausted_at(
                &sketch,
                request,
                config,
                &previous_state,
                counter,
                limit,
                checkpoint,
            );
        }
    }

    #[test]
    fn locality_envelope_guard_reports_row_and_tangent_overflow_without_a_large_solve() {
        let limit = geosolve_core::CONTROLLED_DENSE_KERNEL_MAX_DIMENSION;
        assert!(matches!(
            validate_drag_locality_component_envelope(limit + 1, 2),
            Err(SketchSessionError::DragLocalityRowEnvelopeExceeded {
                active_hard_rows,
                limit: actual_limit,
            }) if active_hard_rows == limit + 1 && actual_limit == limit
        ));
        assert!(matches!(
            validate_drag_locality_component_envelope(2, limit + 1),
            Err(SketchSessionError::DragLocalityEnvelopeExceeded {
                active_tangent_dimensions,
                limit: actual_limit,
            }) if active_tangent_dimensions == limit + 1 && actual_limit == limit
        ));
    }

    #[test]
    fn pantograph_locality_anchors_the_independent_opposite_arm() {
        let (session, mappings, AlphaScenarioIds::MotionPantograph(ids)) =
            runtime_session(AlphaScenarioKind::MotionPantograph)
        else {
            panic!("pantograph roles")
        };
        let input = mappings.runtime_point(ids.input).expect("runtime input");
        let guide = mappings.runtime_point(ids.guide).expect("runtime guide");

        let input_plan = session.drag_locality_plan(input).expect("input plan");
        assert_eq!(input_plan.hard_degrees_of_freedom, 2);
        assert_eq!(input_plan.active_rank, 1);
        assert_eq!(input_plan.passive_degrees_of_freedom, 1);
        assert_eq!(input_plan.anchors.len(), 1);
        assert_eq!(input_plan.anchors[0].point, guide);
        assert_eq!(input_plan.anchors[0].mobility_rank, 1);

        let guide_plan = session.drag_locality_plan(guide).expect("guide plan");
        assert_eq!(guide_plan.hard_degrees_of_freedom, 2);
        assert_eq!(guide_plan.active_rank, 1);
        assert_eq!(guide_plan.passive_degrees_of_freedom, 1);
        assert_eq!(guide_plan.anchors.len(), 1);
        assert_eq!(guide_plan.anchors[0].point, input);
        assert_eq!(guide_plan.anchors[0].mobility_rank, 1);
    }

    #[test]
    fn twin_roller_locality_anchors_only_the_other_center() {
        let (session, mappings, AlphaScenarioIds::MotionCam(ids)) =
            runtime_session(AlphaScenarioKind::MotionCam)
        else {
            panic!("cam roles")
        };
        let left = mappings
            .runtime_point(ids.left_center)
            .expect("runtime left center");
        let right = mappings
            .runtime_point(ids.right_center)
            .expect("runtime right center");

        let left_plan = session.drag_locality_plan(left).expect("left plan");
        assert_eq!(left_plan.hard_degrees_of_freedom, 2);
        assert_eq!(left_plan.active_rank, 1);
        assert_eq!(left_plan.passive_degrees_of_freedom, 1);
        assert_eq!(left_plan.anchors.len(), 1);
        assert_eq!(left_plan.anchors[0].point, right);
        assert_eq!(left_plan.anchors[0].mobility_rank, 1);

        let right_plan = session.drag_locality_plan(right).expect("right plan");
        assert_eq!(right_plan.hard_degrees_of_freedom, 2);
        assert_eq!(right_plan.active_rank, 1);
        assert_eq!(right_plan.passive_degrees_of_freedom, 1);
        assert_eq!(right_plan.anchors.len(), 1);
        assert_eq!(right_plan.anchors[0].point, left);
        assert_eq!(right_plan.anchors[0].mobility_rank, 1);
    }

    #[test]
    fn locality_cover_selects_two_independent_rank_one_anchors_across_scales() {
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let mut sketch = Sketch::new(scale).expect("sketch");
            let center = sketch
                .add_point(Point2::new(0.0, 0.0))
                .expect("shared center");
            let first = sketch
                .add_point(Point2::new(2.0 * scale, 0.0))
                .expect("first endpoint");
            let second = sketch
                .add_point(Point2::new(-scale, 3.0_f64.sqrt() * scale))
                .expect("second endpoint");
            let third = sketch
                .add_point(Point2::new(-scale, -3.0_f64.sqrt() * scale))
                .expect("third endpoint");
            sketch.add_fixed_point(center).expect("fixed center");
            for endpoint in [first, second, third] {
                sketch
                    .add_point_distance(
                        center,
                        endpoint,
                        2.0 * scale,
                        crate::DimensionMode::Driving,
                    )
                    .expect("radius constraint");
            }
            // These rows are deliberately redundant with the three radius targets. They join the
            // arms into one structural hard component without removing any angular freedom.
            sketch
                .add_equal_distance(center, first, center, second)
                .expect("first equal radius");
            sketch
                .add_equal_distance(center, second, center, third)
                .expect("second equal radius");

            let session =
                SketchSession::new(sketch, SketchSolveRequest::new(), SolverConfig::default())
                    .expect("accepted three-arm runtime");
            let plan = session.drag_locality_plan(first).expect("locality plan");
            assert_eq!(plan.hard_degrees_of_freedom, 3, "scale {scale:e}");
            assert_eq!(plan.active_rank, 1, "scale {scale:e}");
            assert_eq!(plan.passive_degrees_of_freedom, 2, "scale {scale:e}");
            assert_eq!(
                plan.anchors
                    .iter()
                    .map(|anchor| (anchor.point, anchor.mobility_rank))
                    .collect::<Vec<_>>(),
                vec![(second, 1), (third, 1)],
                "scale {scale:e}"
            );
        }
    }

    #[test]
    fn locality_rejects_components_beyond_the_interactive_row_envelope() {
        let limit = geosolve_core::CONTROLLED_DENSE_KERNEL_MAX_DIMENSION;
        let mut sketch = Sketch::new(1.0).expect("sketch");
        let point = sketch.add_point(Point2::new(0.0, 0.0)).expect("line start");
        let end = sketch.add_point(Point2::new(1.0, 0.0)).expect("line end");
        let segment = sketch.add_segment(point, end).expect("line segment");
        let hard_rows = limit + 1;
        for _ in 0..hard_rows {
            sketch.add_horizontal(segment).expect("horizontal");
        }
        let session =
            SketchSession::new(sketch, SketchSolveRequest::new(), SolverConfig::default())
                .expect("accepted oversized runtime");
        let active_rows = session.production_scale_assessment().maximum_active_rows;
        assert_eq!(active_rows, hard_rows);
        assert!(active_rows > limit);
        assert!(matches!(
            session.drag_locality_plan(point),
            Err(SketchSessionError::DragLocalityRowEnvelopeExceeded {
                active_hard_rows,
                limit: actual_limit,
            }) if active_hard_rows == active_rows && actual_limit == limit
        ));
    }

    #[test]
    fn controlled_locality_planning_ignores_disconnected_components_and_reports_local_work() {
        let mut sketch = Sketch::new(1.0).expect("sketch");
        let active = sketch
            .add_point(Point2::new(0.0, 0.0))
            .expect("active point");
        for index in 0..128 {
            sketch
                .add_point(Point2::new(f64::from(index), 10.0))
                .expect("disconnected point");
        }
        let session =
            SketchSession::new(sketch, SketchSolveRequest::new(), SolverConfig::default())
                .expect("accepted disconnected runtime");

        let mut limits = geosolve_core::OperationLimits::unlimited();
        limits.document_dependency_items = 1;
        let mut controller =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                geosolve_core::CancellationToken::default(),
                limits,
            ));
        let plan = session
            .drag_locality_plan_with_controller(active, &mut controller)
            .expect("local planning")
            .expect("completed planning");
        assert_eq!(plan.hard_degrees_of_freedom, 2);
        assert_eq!(plan.active_rank, 2);
        assert_eq!(plan.passive_degrees_of_freedom, 0);
        assert!(plan.anchors.is_empty());
        let report = controller.report();
        assert_eq!(report.consumed.component_linearizations, 1);
        assert_eq!(report.consumed.rank_kernels, 2);
        assert_eq!(report.consumed.dense_kernel_rows, 2);
        assert_eq!(report.consumed.dense_kernel_columns, 2);
        assert_eq!(report.consumed.document_dependency_items, 1);
        assert_eq!(report.stopping_reason, None);

        limits.document_dependency_items = 0;
        let mut controller =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                geosolve_core::CancellationToken::default(),
                limits,
            ));
        assert_eq!(
            session
                .drag_locality_plan_with_controller(active, &mut controller)
                .expect("controlled stop"),
            None
        );
        assert!(matches!(
            controller.report().stopping_reason,
            Some(geosolve_core::OperationStopReason::WorkExhausted {
                counter: geosolve_core::OperationWorkCounter::DocumentDependencyItems,
                checkpoint: geosolve_core::OperationCheckpoint::DocumentDependency,
            })
        ));
    }

    #[test]
    fn full_active_rank_short_circuits_large_connected_point_enumeration() {
        let mut sketch = Sketch::new(1.0).expect("sketch");
        let points = (0..64)
            .map(|index| {
                sketch
                    .add_point(Point2::new(f64::from(index), 0.0))
                    .expect("chain point")
            })
            .collect::<Vec<_>>();
        for pair in points.windows(2) {
            let segment = sketch.add_segment(pair[0], pair[1]).expect("chain segment");
            sketch.add_horizontal(segment).expect("horizontal chain");
            sketch
                .add_point_distance(pair[0], pair[1], 1.0, crate::DimensionMode::Driving)
                .expect("chain length");
        }
        let session =
            SketchSession::new(sketch, SketchSolveRequest::new(), SolverConfig::default())
                .expect("accepted connected runtime");

        // Fresh accepted-component validation charges 128 retained state-dependency items for
        // this chain and three items for each two-point hard residual. Give it exactly that
        // mandatory budget and no allowance for enumerating the 64 points afterward.
        let validation_dependency_items = 64 * 2 + 63 * 2 * 3;
        let mut limits = geosolve_core::OperationLimits::unlimited();
        limits.document_dependency_items = validation_dependency_items;
        let mut controller =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                geosolve_core::CancellationToken::default(),
                limits,
            ));
        let plan = session
            .drag_locality_plan_with_controller(points[0], &mut controller)
            .expect("local planning")
            .expect("completed planning");

        assert_eq!(plan.hard_degrees_of_freedom, 2);
        assert_eq!(plan.active_rank, 2);
        assert_eq!(plan.passive_degrees_of_freedom, 0);
        assert!(plan.anchors.is_empty());
        let report = controller.report();
        assert_eq!(
            report.consumed.document_dependency_items,
            validation_dependency_items
        );
        assert_eq!(report.stopping_reason, None);
    }
}
