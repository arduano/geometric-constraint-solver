// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic, equation-free drafting operations over public sketch APIs.
//!
//! This crate never owns solver state or residual formulas. It prepares immutable
//! proposals from a complete stamped sketch snapshot and applies them only through
//! the retained document transaction boundary.

use std::collections::BTreeSet;
use std::sync::Arc;

use geosolve_sketch::{
    ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition, CurveId, CurveSpan,
    DesignPointId, DocumentArcSweep, DocumentConstraintDefinition, DocumentCurveContinuity,
    DocumentCurveTrimView, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentDirectedProfileOffsetCurve, DocumentEdit, DocumentElementId, DocumentError,
    DocumentFaceOffsetDirection, DocumentLineSide, DocumentOffsetTraversal,
    DocumentPreparedProfileOffsetGeometry, DocumentProfileOffsetCreationJunction,
    DocumentProfileOffsetCreationOperand, DocumentProfileOffsetCreationPath,
    DocumentProfileOffsetCreationRequest, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetTurn, DocumentTrimBoundary,
    DocumentTrimParameter, GeometryRole, OperationCheckpoint, OperationControl,
    OperationController, OperationOutcome, OperationWorkCounter, PreparedSketchInput,
    RetainedDocumentTransactionOutcome, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
    SketchAcceptedStateIdentity, SketchDesignIdentity, SketchDocument,
};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetEndpointEligibility, OffsetEndpointRef, OffsetEndpointRole,
    OffsetFaceKey, OffsetJoinOwner, OffsetOperandEligibility, OffsetOperandIndex,
    OffsetOperandIneligibility, OffsetTraversal,
};
use thiserror::Error;

const PARAMETER_EPSILON: f64 = 1.0e-12;
const MAX_PATTERN_INSTANCES: usize = 256;
const MAX_POLYGON_SIDES: usize = 256;
const MAX_PROFILE_OFFSET_SPANS: usize = 256;
const PROFILE_OFFSET_TANGENT_CROSS_TOLERANCE: f64 = 1.0e-9;

/// Exact retained side selected when one support is split into two visible pieces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitRetainedPiece {
    Before,
    After,
}

/// Side retained by a one-parameter trim operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrimRetainedSide {
    Before,
    After,
}

/// One endpoint of a directed line span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEndpoint {
    Start,
    End,
}

/// One exact authenticated operand for the topology-preserving Profile Offset operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SketchProfileOffsetOperand {
    Face {
        key: OffsetFaceKey,
        direction: DocumentFaceOffsetDirection,
    },
    OpenChain {
        spans: Vec<OffsetDirectedSpan>,
        side: DocumentLineSide,
    },
}

/// Closed sketch-operation request surface.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationRequest {
    Split {
        support: CurveSpan,
        parameter: f64,
        retained: SplitRetainedPiece,
    },
    Break {
        support: CurveSpan,
        start: f64,
        end: f64,
        retained: SplitRetainedPiece,
    },
    Trim {
        support: CurveSpan,
        parameter: f64,
        retained: TrimRetainedSide,
    },
    ExtendLineToLine {
        line: CurveSpan,
        endpoint: LineEndpoint,
        target: CurveSpan,
    },
    Mirror {
        label: String,
        source: CurveId,
        axis: CurveSpan,
    },
    Chamfer {
        label: String,
        first: CurveSpan,
        second: CurveSpan,
        first_distance: f64,
        second_distance: f64,
    },
    AssociativeFillet {
        label: String,
        request: CurveCurveFilletRequest,
    },
    Rectangle {
        label: String,
        origin: [f64; 2],
        width: f64,
        height: f64,
    },
    RegularPolygon {
        label: String,
        center: [f64; 2],
        radius: f64,
        sides: usize,
        rotation: f64,
    },
    Slot {
        label: String,
        first_center: [f64; 2],
        second_center: [f64; 2],
        radius: f64,
    },
    LinearPattern {
        label: String,
        sources: Vec<CurveId>,
        instances: usize,
        step: [f64; 2],
    },
    ProfileOffset {
        label: String,
        distance: f64,
        operand: SketchProfileOffsetOperand,
        operand_index: Arc<OffsetOperandIndex>,
    },
}

impl SketchOperationRequest {
    #[must_use]
    pub const fn kind(&self) -> SketchOperationKind {
        match self {
            Self::Split { .. } => SketchOperationKind::Split,
            Self::Break { .. } => SketchOperationKind::Break,
            Self::Trim { .. } => SketchOperationKind::Trim,
            Self::ExtendLineToLine { .. } => SketchOperationKind::Extend,
            Self::Mirror { .. } => SketchOperationKind::Mirror,
            Self::Chamfer { .. } => SketchOperationKind::Chamfer,
            Self::AssociativeFillet { .. } => SketchOperationKind::AssociativeFillet,
            Self::Rectangle { .. } => SketchOperationKind::Rectangle,
            Self::RegularPolygon { .. } => SketchOperationKind::RegularPolygon,
            Self::Slot { .. } => SketchOperationKind::Slot,
            Self::LinearPattern { .. } => SketchOperationKind::LinearPattern,
            Self::ProfileOffset { .. } => SketchOperationKind::ProfileOffset,
        }
    }
}

/// Stable operation classification for host presentation and evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchOperationKind {
    Split,
    Break,
    Trim,
    Extend,
    Mirror,
    Chamfer,
    AssociativeFillet,
    Rectangle,
    RegularPolygon,
    Slot,
    LinearPattern,
    ProfileOffset,
}

/// Complete immutable document/accepted-state input used to prepare one proposal.
#[derive(Clone, Debug)]
pub struct SketchOperationSnapshot {
    input: PreparedSketchInput,
    design: SketchDocument,
    accepted: Option<AcceptedOperationSnapshot>,
}

#[derive(Clone, Debug)]
struct AcceptedOperationSnapshot {
    identity: SketchAcceptedStateIdentity,
    current_publication: bool,
    design: SketchDesignIdentity,
    document: SketchDocument,
}

impl SketchOperationSnapshot {
    /// Captures immutable operation input without changing solver lifecycle state.
    #[must_use]
    pub fn capture(session: &RetainedSketchDocumentSession) -> Self {
        let current_accepted = session
            .accepted_state_for_current_input()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
        Self {
            input: session.prepared_input(),
            design: session.design_document().clone(),
            accepted: session
                .accepted_state()
                .map(|accepted| AcceptedOperationSnapshot {
                    identity: accepted.identity(),
                    current_publication: current_accepted == Some(accepted.identity()),
                    design: accepted.design_identity(),
                    document: accepted.document().clone(),
                }),
        }
    }

    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn design_document(&self) -> &SketchDocument {
        &self.design
    }

    /// Turns this immutable snapshot into a worker-movable operation job.
    #[must_use]
    pub fn prepare(self, request: SketchOperationRequest) -> PreparedSketchOperation {
        self.prepare_with_geometry_role(request, GeometryRole::Profile)
    }

    /// Turns this snapshot into an operation job with an explicit role for source-free output.
    ///
    /// The requested role applies to `Rectangle`, `RegularPolygon`, and `Slot` output. Geometry
    /// derived from existing curves retains its source-driven role policy instead: copies inherit
    /// their source, while multi-source output is Construction when any parent is Construction.
    #[must_use]
    pub fn prepare_with_geometry_role(
        self,
        request: SketchOperationRequest,
        source_free_role: GeometryRole,
    ) -> PreparedSketchOperation {
        PreparedSketchOperation {
            snapshot: self,
            request,
            source_free_role,
        }
    }
}

/// Immutable operation job. Hosts may move it to a native worker or execute it
/// synchronously in single-threaded WASM.
#[derive(Debug)]
pub struct PreparedSketchOperation {
    snapshot: SketchOperationSnapshot,
    request: SketchOperationRequest,
    source_free_role: GeometryRole,
}

impl PreparedSketchOperation {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.snapshot.input
    }

    #[must_use]
    pub const fn request(&self) -> &SketchOperationRequest {
        &self.request
    }

    /// Returns the requested role for source-free output geometry.
    #[must_use]
    pub const fn source_free_geometry_role(&self) -> GeometryRole {
        self.source_free_role
    }

    /// Executes against captured scratch state and never mutates a live session.
    ///
    /// # Errors
    ///
    /// Returns a typed invalid-request or public-document construction error.
    pub fn execute(
        self,
        control: OperationControl,
    ) -> Result<OperationOutcome<SketchOperationResult>, SketchOperationError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        if controller
            .charge(
                OperationWorkCounter::DocumentValidationItems,
                1,
                OperationCheckpoint::DocumentValidation,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let operand_count = request_operand_count(&self.request);
        if controller
            .charge(
                OperationWorkCounter::DocumentDependencyItems,
                operand_count,
                OperationCheckpoint::DocumentDependency,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let result = build_result(&self.snapshot, self.request, self.source_free_role)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(result))
    }
}

/// Typed non-proposal outcome for an otherwise valid request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchOperationUnsupported {
    pub kind: SketchOperationKind,
    pub reason: SketchOperationUnsupportedReason,
}

/// Exact unsupported-capability classification. Unsupported exact transforms are
/// never approximated.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationUnsupportedReason {
    CurveFamily {
        curve: CurveId,
        operation: &'static str,
    },
    PeriodicMultiInterval {
        support: CurveSpan,
    },
    ProfileOffsetFace {
        key: OffsetFaceKey,
        reasons: Vec<OffsetOperandIneligibility>,
    },
    ProfileOffsetSpan {
        span: CurveSpan,
        reasons: Vec<OffsetOperandIneligibility>,
    },
    ProfileOffsetPeriodicChain {
        span: CurveSpan,
    },
}

/// Typed incomplete input-state outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchOperationIncomplete {
    pub kind: SketchOperationKind,
    pub reason: SketchOperationIncompleteReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationIncompleteReason {
    AcceptedStateRequired,
    AcceptedStateForDifferentDesign,
    AcceptedStateForDifferentInput,
    AcceptedGeometryDiffersFromDesign,
    ParameterNotInsideOneVisibleInterval,
    LinesDoNotShareOneEndpoint,
    ParallelLines,
    IntersectionDoesNotExtendSelectedEndpoint,
    ProfileOffsetIndexForDifferentInput,
    ProfileOffsetIndexForDifferentAcceptedState,
    ProfileOffsetFaceMissing {
        key: OffsetFaceKey,
    },
    ProfileOffsetSpanMissing {
        span: CurveSpan,
    },
    ProfileOffsetEmptyChain,
    ProfileOffsetSpanLimitExceeded,
    ProfileOffsetDuplicateSpan {
        span: CurveSpan,
    },
    ProfileOffsetDisconnectedJoin {
        incoming: CurveSpan,
        outgoing: CurveSpan,
    },
    ProfileOffsetBranchedJoin {
        endpoint: OffsetEndpointRef,
    },
    ProfileOffsetClosedChain,
    ProfileOffsetDegenerateJunction {
        incoming: CurveSpan,
        outgoing: CurveSpan,
    },
}

/// Completed preparation result.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum SketchOperationResult {
    Proposed(Box<SketchOperationProposal>),
    Unsupported(SketchOperationUnsupported),
    Incomplete(SketchOperationIncomplete),
}

/// Explicit identity/provenance disposition published with one proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationIdentityChange {
    Retained(DocumentElementId),
    Replaced(DocumentElementId),
    Split {
        source: CurveId,
        retained: SplitRetainedPiece,
        visible_piece_count: usize,
    },
    Proposed(DocumentElementId),
}

/// Deterministic public-document result of applying one proposal.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchOperationApplication {
    pub kind: SketchOperationKind,
    pub identity_changes: Vec<SketchOperationIdentityChange>,
}

/// Non-mutating proposal prepared from one exact input stamp.
#[derive(Clone, Debug)]
pub struct SketchOperationProposal {
    input: PreparedSketchInput,
    accepted: Option<SketchAcceptedStateIdentity>,
    request: SketchOperationRequest,
    source_free_role: GeometryRole,
    plan: PlannedOperation,
    expected: SketchOperationApplication,
}

impl SketchOperationProposal {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> Option<SketchAcceptedStateIdentity> {
        self.accepted
    }

    #[must_use]
    pub const fn request(&self) -> &SketchOperationRequest {
        &self.request
    }

    /// Returns the requested role for source-free output geometry.
    #[must_use]
    pub const fn source_free_geometry_role(&self) -> GeometryRole {
        self.source_free_role
    }

    #[must_use]
    pub const fn expected_application(&self) -> &SketchOperationApplication {
        &self.expected
    }

    /// Returns the authenticated atomic document edit for a prepared Profile Offset.
    ///
    /// The caller must prepare this edit against [`Self::input`]. Once that prepared sketch job
    /// completes, its exact `PreparedSketchPatch` is the sole preview/commit authority; the edit
    /// must not be rebuilt from UI state before publication. Other operation kinds return `None`.
    #[must_use]
    pub fn profile_offset_document_edit(&self) -> Option<DocumentEdit> {
        match &self.plan {
            PlannedOperation::ProfileOffset { prepared, .. } => {
                Some(DocumentEdit::CreatePreparedProfileOffsetGeometry {
                    prepared: Box::new(prepared.clone()),
                })
            }
            PlannedOperation::Visibility { .. }
            | PlannedOperation::ExtendLine { .. }
            | PlannedOperation::Mirror { .. }
            | PlannedOperation::Chamfer(_)
            | PlannedOperation::Fillet { .. }
            | PlannedOperation::Rectangle { .. }
            | PlannedOperation::Polygon { .. }
            | PlannedOperation::Slot(_)
            | PlannedOperation::Pattern { .. } => None,
        }
    }

    /// Applies through the normal retained document transaction after exact-input
    /// compare-and-swap validation.
    ///
    /// A solve rejection normally remains an ordinary retained-design attempt. Profile Offset is
    /// stricter: its topology-preserving contract requires a newly accepted state, so a rejected
    /// attempt returns [`SketchOperationApplyError::ProfileOffsetRejected`] without changing the
    /// live session.
    ///
    /// # Errors
    ///
    /// Returns a stale-input, deterministic-replay, document, or session error.
    pub fn apply(
        &self,
        session: &mut RetainedSketchDocumentSession,
    ) -> Result<
        RetainedDocumentTransactionOutcome<SketchOperationApplication>,
        SketchOperationApplyError,
    > {
        let actual = session.prepared_input();
        if actual != self.input {
            return Err(SketchOperationApplyError::StaleInput {
                expected: Box::new(self.input),
                actual: Box::new(actual),
            });
        }
        if self.requires_accepted_publication() {
            let mut candidate = session.clone();
            let outcome = self.apply_retained(&mut candidate)?;
            if outcome.published_accepted_identity().is_none() {
                return Err(SketchOperationApplyError::ProfileOffsetRejected);
            }
            *session = candidate;
            return Ok(outcome);
        }
        self.apply_retained(session)
    }

    /// Controlled counterpart to [`Self::apply`].
    ///
    /// Exact-input compare-and-swap is checked before work begins. Cancellation
    /// or deterministic work exhaustion returns a stopped [`OperationOutcome`]
    /// and leaves the retained session unchanged; a completed outcome has the
    /// same retained-design and independent-publication semantics as
    /// [`Self::apply`].
    ///
    /// # Errors
    ///
    /// Returns the same stale-input, deterministic-replay, document, or session
    /// errors as [`Self::apply`].
    pub fn apply_controlled(
        &self,
        session: &mut RetainedSketchDocumentSession,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<SketchOperationApplication>>,
        SketchOperationApplyError,
    > {
        let actual = session.prepared_input();
        if actual != self.input {
            return Err(SketchOperationApplyError::StaleInput {
                expected: Box::new(self.input),
                actual: Box::new(actual),
            });
        }
        if self.requires_accepted_publication() {
            let mut candidate = session.clone();
            let outcome = self.apply_retained_controlled(&mut candidate, control)?;
            if let OperationOutcome::Completed { value, .. } = &outcome {
                if value.published_accepted_identity().is_none() {
                    return Err(SketchOperationApplyError::ProfileOffsetRejected);
                }
                *session = candidate;
            }
            return Ok(outcome);
        }
        self.apply_retained_controlled(session, control)
    }

    const fn requires_accepted_publication(&self) -> bool {
        matches!(&self.plan, PlannedOperation::ProfileOffset { .. })
    }

    fn apply_retained(
        &self,
        session: &mut RetainedSketchDocumentSession,
    ) -> Result<
        RetainedDocumentTransactionOutcome<SketchOperationApplication>,
        SketchOperationApplyError,
    > {
        let expected_application = self.expected.clone();
        let plan = self.plan.clone();
        Ok(
            session.transact(self.input.design_identity(), move |document| {
                let application = plan.apply(document)?;
                if application != expected_application {
                    return Err(DocumentError::InvalidField {
                        field: "operation proposal replay",
                        message:
                            "same stamped document did not reproduce the prepared identity map"
                                .into(),
                    });
                }
                Ok(application)
            })?,
        )
    }

    fn apply_retained_controlled(
        &self,
        session: &mut RetainedSketchDocumentSession,
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<SketchOperationApplication>>,
        SketchOperationApplyError,
    > {
        let expected_application = self.expected.clone();
        let plan = self.plan.clone();
        Ok(session.transact_controlled(
            self.input.design_identity(),
            move |document| {
                let application = plan.apply(document)?;
                if application != expected_application {
                    return Err(DocumentError::InvalidField {
                        field: "operation proposal replay",
                        message:
                            "same stamped document did not reproduce the prepared identity map"
                                .into(),
                    });
                }
                Ok(application)
            },
            control,
        )?)
    }
}

/// Preparation failure before a proposal exists.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SketchOperationError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error("invalid {field}: {message}")]
    InvalidRequest {
        field: &'static str,
        message: &'static str,
    },
}

/// Proposal-application failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SketchOperationApplyError {
    #[error("stale sketch-operation proposal input")]
    StaleInput {
        expected: Box<PreparedSketchInput>,
        actual: Box<PreparedSketchInput>,
    },
    #[error("Profile Offset did not produce an independently accepted state")]
    ProfileOffsetRejected,
    #[error(transparent)]
    Session(#[from] geosolve_sketch::DocumentSessionError),
}

#[derive(Clone, Debug)]
enum PlannedOperation {
    Visibility {
        kind: SketchOperationKind,
        support: CurveSpan,
        views: Vec<DocumentCurveTrimView>,
        retained: Option<SplitRetainedPiece>,
    },
    ExtendLine {
        curve: CurveId,
        point: DesignPointId,
        position: [f64; 2],
    },
    Mirror {
        label: String,
        source: CurveId,
        axis: CurveSpan,
    },
    Chamfer(ChamferPlan),
    Fillet {
        label: String,
        request: CurveCurveFilletRequest,
    },
    Rectangle {
        label: String,
        origin: [f64; 2],
        width: f64,
        height: f64,
        role: GeometryRole,
    },
    Polygon {
        label: String,
        points: Vec<[f64; 2]>,
        role: GeometryRole,
    },
    Slot(SlotPlan),
    Pattern {
        label: String,
        sources: Vec<CurveId>,
        instances: usize,
        step: [f64; 2],
    },
    ProfileOffset {
        prepared: DocumentPreparedProfileOffsetGeometry,
        sources: Vec<CurveSpan>,
    },
}

#[derive(Clone, Debug)]
struct ChamferPlan {
    label: String,
    first: CurveSpan,
    second: CurveSpan,
    corner: DesignPointId,
    first_position: [f64; 2],
    second_position: [f64; 2],
    first_parameter: f64,
    second_parameter: f64,
    first_keep_start: bool,
    second_keep_start: bool,
    first_distance: f64,
    second_distance: f64,
}

#[derive(Clone, Debug)]
struct SlotPlan {
    label: String,
    first_center: [f64; 2],
    second_center: [f64; 2],
    radius: f64,
    role: GeometryRole,
}

#[allow(clippy::too_many_lines)]
fn build_result(
    snapshot: &SketchOperationSnapshot,
    request: SketchOperationRequest,
    source_free_role: GeometryRole,
) -> Result<SketchOperationResult, SketchOperationError> {
    let kind = request.kind();
    let (plan, accepted) = match &request {
        SketchOperationRequest::Split {
            support,
            parameter,
            retained,
        } => {
            ensure_finite(*parameter, "split parameter")?;
            if is_full_periodic_support(&snapshot.design, *support)? {
                return Ok(unsupported(
                    kind,
                    SketchOperationUnsupportedReason::PeriodicMultiInterval { support: *support },
                ));
            }
            let intervals = snapshot.design.visible_intervals(*support)?;
            let Some(index) = containing_interval(&intervals, *parameter) else {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
                ));
            };
            let selected = intervals[index];
            if !strictly_inside(*parameter, selected.start, selected.end) {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
                ));
            }
            let mut views = intervals
                .iter()
                .map(|interval| fixed_view(*support, interval.start, interval.end))
                .collect::<Vec<_>>();
            views.splice(
                index..=index,
                [
                    fixed_view(*support, selected.start, *parameter),
                    fixed_view(*support, *parameter, selected.end),
                ],
            );
            (
                PlannedOperation::Visibility {
                    kind,
                    support: *support,
                    views,
                    retained: Some(*retained),
                },
                None,
            )
        }
        SketchOperationRequest::Break {
            support,
            start,
            end,
            retained,
        } => {
            ensure_finite(*start, "break start")?;
            ensure_finite(*end, "break end")?;
            if start >= end {
                return Err(SketchOperationError::InvalidRequest {
                    field: "break interval",
                    message: "start must be less than end",
                });
            }
            if is_full_periodic_support(&snapshot.design, *support)? {
                return Ok(unsupported(
                    kind,
                    SketchOperationUnsupportedReason::PeriodicMultiInterval { support: *support },
                ));
            }
            let intervals = snapshot.design.visible_intervals(*support)?;
            let Some(index) = intervals
                .iter()
                .position(|interval| interval.start < *start && *end < interval.end)
            else {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
                ));
            };
            let selected = intervals[index];
            let mut views = intervals
                .iter()
                .map(|interval| fixed_view(*support, interval.start, interval.end))
                .collect::<Vec<_>>();
            views.splice(
                index..=index,
                [
                    fixed_view(*support, selected.start, *start),
                    fixed_view(*support, *end, selected.end),
                ],
            );
            (
                PlannedOperation::Visibility {
                    kind,
                    support: *support,
                    views,
                    retained: Some(*retained),
                },
                None,
            )
        }
        SketchOperationRequest::Trim {
            support,
            parameter,
            retained,
        } => {
            ensure_finite(*parameter, "trim parameter")?;
            if is_full_periodic_support(&snapshot.design, *support)? {
                return Ok(unsupported(
                    kind,
                    SketchOperationUnsupportedReason::PeriodicMultiInterval { support: *support },
                ));
            }
            let intervals = snapshot.design.visible_intervals(*support)?;
            let Some(index) = containing_interval(&intervals, *parameter) else {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
                ));
            };
            let selected = intervals[index];
            if !strictly_inside(*parameter, selected.start, selected.end) {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
                ));
            }
            let retained_piece = match retained {
                TrimRetainedSide::Before => SplitRetainedPiece::Before,
                TrimRetainedSide::After => SplitRetainedPiece::After,
            };
            let retained_intervals = match retained {
                TrimRetainedSide::Before => intervals[..=index]
                    .iter()
                    .enumerate()
                    .map(|(current, interval)| {
                        fixed_view(
                            *support,
                            interval.start,
                            if current == index {
                                *parameter
                            } else {
                                interval.end
                            },
                        )
                    })
                    .collect(),
                TrimRetainedSide::After => intervals[index..]
                    .iter()
                    .enumerate()
                    .map(|(current, interval)| {
                        fixed_view(
                            *support,
                            if current == 0 {
                                *parameter
                            } else {
                                interval.start
                            },
                            interval.end,
                        )
                    })
                    .collect(),
            };
            (
                PlannedOperation::Visibility {
                    kind,
                    support: *support,
                    views: retained_intervals,
                    retained: Some(retained_piece),
                },
                None,
            )
        }
        SketchOperationRequest::ExtendLineToLine {
            line,
            endpoint,
            target,
        } => {
            let Some(accepted) = accepted_for_design(snapshot) else {
                return Ok(missing_accepted(snapshot, kind));
            };
            let (point, position) =
                match plan_line_extension(&accepted.document, *line, *endpoint, *target) {
                    Ok(plan) => plan,
                    Err(GeometryPlanFailure::Unsupported(curve)) => {
                        return Ok(unsupported(
                            kind,
                            SketchOperationUnsupportedReason::CurveFamily {
                                curve,
                                operation: "extend",
                            },
                        ));
                    }
                    Err(GeometryPlanFailure::Incomplete(reason)) => {
                        return Ok(incomplete(kind, reason));
                    }
                };
            (
                PlannedOperation::ExtendLine {
                    curve: line.curve,
                    point,
                    position,
                },
                Some(accepted.identity),
            )
        }
        SketchOperationRequest::Mirror {
            label,
            source,
            axis,
        } => {
            let Some(accepted) = accepted_for_design(snapshot) else {
                return Ok(missing_accepted(snapshot, kind));
            };
            if accepted.document != snapshot.design {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::AcceptedGeometryDiffersFromDesign,
                ));
            }
            if !is_point_defined_mirror_family(&snapshot.design, *source)? {
                return Ok(unsupported(
                    kind,
                    SketchOperationUnsupportedReason::CurveFamily {
                        curve: *source,
                        operation: "mirror",
                    },
                ));
            }
            (
                PlannedOperation::Mirror {
                    label: label.clone(),
                    source: *source,
                    axis: *axis,
                },
                Some(accepted.identity),
            )
        }
        SketchOperationRequest::Chamfer {
            label,
            first,
            second,
            first_distance,
            second_distance,
        } => {
            ensure_positive(*first_distance, "first chamfer distance")?;
            ensure_positive(*second_distance, "second chamfer distance")?;
            let Some(accepted) = accepted_for_design(snapshot) else {
                return Ok(missing_accepted(snapshot, kind));
            };
            let plan = match plan_chamfer(
                &accepted.document,
                label,
                *first,
                *second,
                *first_distance,
                *second_distance,
            ) {
                Ok(plan) => plan,
                Err(GeometryPlanFailure::Unsupported(curve)) => {
                    return Ok(unsupported(
                        kind,
                        SketchOperationUnsupportedReason::CurveFamily {
                            curve,
                            operation: "chamfer",
                        },
                    ));
                }
                Err(GeometryPlanFailure::Incomplete(reason)) => {
                    return Ok(incomplete(kind, reason));
                }
            };
            (PlannedOperation::Chamfer(plan), Some(accepted.identity))
        }
        SketchOperationRequest::AssociativeFillet { label, request } => (
            PlannedOperation::Fillet {
                label: label.clone(),
                request: *request,
            },
            None,
        ),
        SketchOperationRequest::Rectangle {
            label,
            origin,
            width,
            height,
        } => {
            ensure_pair(*origin, "rectangle origin")?;
            ensure_positive(*width, "rectangle width")?;
            ensure_positive(*height, "rectangle height")?;
            (
                PlannedOperation::Rectangle {
                    label: label.clone(),
                    origin: *origin,
                    width: *width,
                    height: *height,
                    role: source_free_role,
                },
                None,
            )
        }
        SketchOperationRequest::RegularPolygon {
            label,
            center,
            radius,
            sides,
            rotation,
        } => {
            ensure_pair(*center, "polygon center")?;
            ensure_positive(*radius, "polygon radius")?;
            ensure_finite(*rotation, "polygon rotation")?;
            if !(3..=MAX_POLYGON_SIDES).contains(sides) {
                return Err(SketchOperationError::InvalidRequest {
                    field: "polygon sides",
                    message: "must be in 3..=256",
                });
            }
            let points = (0..*sides)
                .map(|index| {
                    let angle = *rotation
                        + std::f64::consts::TAU * small_index(index) / small_index(*sides);
                    [
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    ]
                })
                .collect();
            (
                PlannedOperation::Polygon {
                    label: label.clone(),
                    points,
                    role: source_free_role,
                },
                None,
            )
        }
        SketchOperationRequest::Slot {
            label,
            first_center,
            second_center,
            radius,
        } => {
            ensure_pair(*first_center, "slot first center")?;
            ensure_pair(*second_center, "slot second center")?;
            ensure_positive(*radius, "slot radius")?;
            if squared_distance(*first_center, *second_center) <= f64::EPSILON {
                return Err(SketchOperationError::InvalidRequest {
                    field: "slot centers",
                    message: "centers must be distinct",
                });
            }
            (
                PlannedOperation::Slot(SlotPlan {
                    label: label.clone(),
                    first_center: *first_center,
                    second_center: *second_center,
                    radius: *radius,
                    role: source_free_role,
                }),
                None,
            )
        }
        SketchOperationRequest::LinearPattern {
            label,
            sources,
            instances,
            step,
        } => {
            ensure_pair(*step, "pattern step")?;
            if sources.is_empty() {
                return Err(SketchOperationError::InvalidRequest {
                    field: "pattern sources",
                    message: "must not be empty",
                });
            }
            if !(2..=MAX_PATTERN_INSTANCES).contains(instances) {
                return Err(SketchOperationError::InvalidRequest {
                    field: "pattern instances",
                    message: "must be in 2..=256",
                });
            }
            let Some(accepted) = accepted_for_design(snapshot) else {
                return Ok(missing_accepted(snapshot, kind));
            };
            if accepted.document != snapshot.design {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::AcceptedGeometryDiffersFromDesign,
                ));
            }
            for source in sources {
                if !is_point_defined_pattern_family(&snapshot.design, *source)? {
                    return Ok(unsupported(
                        kind,
                        SketchOperationUnsupportedReason::CurveFamily {
                            curve: *source,
                            operation: "linear pattern",
                        },
                    ));
                }
            }
            (
                PlannedOperation::Pattern {
                    label: label.clone(),
                    sources: sources.clone(),
                    instances: *instances,
                    step: *step,
                },
                Some(accepted.identity),
            )
        }
        SketchOperationRequest::ProfileOffset {
            label,
            distance,
            operand,
            operand_index,
        } => {
            ensure_positive(*distance, "profile offset distance")?;
            let Some(accepted) = accepted_for_design(snapshot) else {
                return Ok(missing_accepted(snapshot, kind));
            };
            if operand_index.input() != snapshot.input {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ProfileOffsetIndexForDifferentInput,
                ));
            }
            if operand_index.accepted_state_identity() != accepted.identity {
                return Ok(incomplete(
                    kind,
                    SketchOperationIncompleteReason::ProfileOffsetIndexForDifferentAcceptedState,
                ));
            }
            let (creation_operand, sources) =
                match plan_profile_offset_operand(&accepted.document, operand_index, operand) {
                    Ok(plan) => plan,
                    Err(ProfileOffsetPlanFailure::Unsupported(reason)) => {
                        return Ok(unsupported(kind, reason));
                    }
                    Err(ProfileOffsetPlanFailure::Incomplete(reason)) => {
                        return Ok(incomplete(kind, reason));
                    }
                };
            let request = DocumentProfileOffsetCreationRequest {
                label: label.clone(),
                distance: *distance,
                operand: creation_operand,
            };
            let prepared = accepted.document.prepare_profile_offset_geometry(request)?;
            (
                PlannedOperation::ProfileOffset { prepared, sources },
                Some(accepted.identity),
            )
        }
    };

    let mut scratch = snapshot.design.clone();
    let expected = plan.apply(&mut scratch)?;
    scratch.validate()?;
    Ok(SketchOperationResult::Proposed(Box::new(
        SketchOperationProposal {
            input: snapshot.input,
            accepted,
            request,
            source_free_role,
            plan,
            expected,
        },
    )))
}

fn accepted_for_design(snapshot: &SketchOperationSnapshot) -> Option<&AcceptedOperationSnapshot> {
    snapshot.accepted.as_ref().filter(|accepted| {
        accepted.design == snapshot.input.design_identity() && accepted.current_publication
    })
}

fn missing_accepted(
    snapshot: &SketchOperationSnapshot,
    kind: SketchOperationKind,
) -> SketchOperationResult {
    let reason = match snapshot.accepted.as_ref() {
        None => SketchOperationIncompleteReason::AcceptedStateRequired,
        Some(accepted) if accepted.design != snapshot.input.design_identity() => {
            SketchOperationIncompleteReason::AcceptedStateForDifferentDesign
        }
        Some(_) => SketchOperationIncompleteReason::AcceptedStateForDifferentInput,
    };
    incomplete(kind, reason)
}

fn unsupported(
    kind: SketchOperationKind,
    reason: SketchOperationUnsupportedReason,
) -> SketchOperationResult {
    SketchOperationResult::Unsupported(SketchOperationUnsupported { kind, reason })
}

fn incomplete(
    kind: SketchOperationKind,
    reason: SketchOperationIncompleteReason,
) -> SketchOperationResult {
    SketchOperationResult::Incomplete(SketchOperationIncomplete { kind, reason })
}

impl PlannedOperation {
    #[allow(clippy::too_many_lines)]
    fn apply(
        &self,
        document: &mut SketchDocument,
    ) -> Result<SketchOperationApplication, DocumentError> {
        let before = document_elements(document);
        let (kind, mut explicit) = match self {
            Self::Visibility {
                kind,
                support,
                views,
                retained,
            } => {
                document.replace_trim_views(*support, views.clone())?;
                let change = retained.map_or_else(
                    || SketchOperationIdentityChange::Retained(support.curve.into()),
                    |retained| SketchOperationIdentityChange::Split {
                        source: support.curve,
                        retained,
                        visible_piece_count: views.len(),
                    },
                );
                (*kind, vec![change])
            }
            Self::ExtendLine {
                curve,
                point,
                position,
            } => {
                document.set_point_position(*point, *position)?;
                (
                    SketchOperationKind::Extend,
                    vec![
                        SketchOperationIdentityChange::Retained((*curve).into()),
                        SketchOperationIdentityChange::Replaced((*point).into()),
                    ],
                )
            }
            Self::Mirror {
                label,
                source,
                axis,
            } => {
                let ids = document.add_mirrored_curve(label, *source, *axis)?;
                (
                    SketchOperationKind::Mirror,
                    vec![SketchOperationIdentityChange::Retained((*source).into())]
                        .into_iter()
                        .chain(
                            ids.point_pairs
                                .iter()
                                .map(|(_, point)| {
                                    SketchOperationIdentityChange::Proposed((*point).into())
                                })
                                .chain(std::iter::once(SketchOperationIdentityChange::Proposed(
                                    ids.mirrored_curve.into(),
                                )))
                                .chain(ids.symmetry_constraints.iter().map(|constraint| {
                                    SketchOperationIdentityChange::Proposed((*constraint).into())
                                })),
                        )
                        .collect(),
                )
            }
            Self::Chamfer(plan) => (SketchOperationKind::Chamfer, apply_chamfer(document, plan)?),
            Self::Fillet { label, request } => {
                let ids = document.add_curve_curve_fillet(label, *request)?;
                (
                    SketchOperationKind::AssociativeFillet,
                    vec![
                        SketchOperationIdentityChange::Retained(request.first.curve.curve.into()),
                        SketchOperationIdentityChange::Retained(request.second.curve.curve.into()),
                        SketchOperationIdentityChange::Proposed(ids.arc.into()),
                        SketchOperationIdentityChange::Proposed(ids.constraint.into()),
                    ],
                )
            }
            Self::Rectangle {
                label,
                origin,
                width,
                height,
                role,
            } => {
                document.add_rectangle_with_role(label, *origin, *width, *height, *role)?;
                (SketchOperationKind::Rectangle, Vec::new())
            }
            Self::Polygon {
                label,
                points,
                role,
            } => {
                apply_polygon(document, label, points, *role)?;
                (SketchOperationKind::RegularPolygon, Vec::new())
            }
            Self::Slot(plan) => {
                apply_slot(document, plan)?;
                (SketchOperationKind::Slot, Vec::new())
            }
            Self::Pattern {
                label,
                sources,
                instances,
                step,
            } => {
                for instance in 1..*instances {
                    let instance = small_index(instance);
                    let offset = [step[0] * instance, step[1] * instance];
                    for (source_ordinal, source) in sources.iter().copied().enumerate() {
                        copy_point_defined_curve(
                            document,
                            &format!("{label}.instance_{instance}.source_{}", source_ordinal + 1),
                            source,
                            offset,
                        )?;
                    }
                }
                (
                    SketchOperationKind::LinearPattern,
                    sources
                        .iter()
                        .map(|source| SketchOperationIdentityChange::Retained((*source).into()))
                        .collect(),
                )
            }
            Self::ProfileOffset { prepared, sources } => {
                document.create_prepared_profile_offset_geometry(prepared.clone())?;
                (
                    SketchOperationKind::ProfileOffset,
                    sources
                        .iter()
                        .map(|source| SketchOperationIdentityChange::Retained(source.curve.into()))
                        .collect(),
                )
            }
        };
        let after = document_elements(document);
        for element in after.difference(&before) {
            if !explicit
                .iter()
                .any(|change| identity_change_mentions(change, *element))
            {
                explicit.push(SketchOperationIdentityChange::Proposed(*element));
            }
        }
        Ok(SketchOperationApplication {
            kind,
            identity_changes: explicit,
        })
    }
}

fn apply_polygon(
    document: &mut SketchDocument,
    label: &str,
    positions: &[[f64; 2]],
    role: GeometryRole,
) -> Result<(), DocumentError> {
    let points = positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            document.add_point(format!("{label}.point_{}", index + 1), *position)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        document.add_curve_with_role(
            format!("{label}.edge_{}", index + 1),
            CurveDefinition::Line {
                start: points[index],
                end: points[next],
                branch_direction: direction(positions[index], positions[next])?,
            },
            role,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn apply_chamfer(
    document: &mut SketchDocument,
    plan: &ChamferPlan,
) -> Result<Vec<SketchOperationIdentityChange>, DocumentError> {
    let role = if [plan.first.curve, plan.second.curve]
        .into_iter()
        .any(|curve| document.geometry_role(curve) == Some(GeometryRole::Construction))
    {
        GeometryRole::Construction
    } else {
        GeometryRole::Profile
    };
    let first_point = document.add_point(
        format!("{}.first_endpoint", plan.label),
        plan.first_position,
    )?;
    let second_point = document.add_point(
        format!("{}.second_endpoint", plan.label),
        plan.second_position,
    )?;
    let chamfer = document.add_curve_with_role(
        format!("{}.edge", plan.label),
        CurveDefinition::Line {
            start: first_point,
            end: second_point,
            branch_direction: direction(plan.first_position, plan.second_position)?,
        },
        role,
    )?;
    let first_contact = document.add_curve_contact(
        format!("{}.first_contact", plan.label),
        plan.first,
        plan.first_parameter,
        0,
        local_neighborhood(plan.first_parameter),
        None,
    )?;
    let second_contact = document.add_curve_contact(
        format!("{}.second_contact", plan.label),
        plan.second,
        plan.second_parameter,
        0,
        local_neighborhood(plan.second_parameter),
        None,
    )?;
    let first_owner = document.add_constraint(
        format!("{}.first_on_parent", plan.label),
        DocumentConstraintDefinition::PointOnCurve {
            point: first_point,
            contact: first_contact,
        },
    )?;
    let second_owner = document.add_constraint(
        format!("{}.second_on_parent", plan.label),
        DocumentConstraintDefinition::PointOnCurve {
            point: second_point,
            contact: second_contact,
        },
    )?;
    let first_target = document.add_scalar(
        format!("{}.first_distance", plan.label),
        plan.first_distance,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let second_target = document.add_scalar(
        format!("{}.second_distance", plan.label),
        plan.second_distance,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        format!("{}.first_distance_dimension", plan.label),
        DocumentDimensionDefinition::PointDistance {
            first: plan.corner,
            second: first_point,
            target: first_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    document.add_dimension(
        format!("{}.second_distance_dimension", plan.label),
        DocumentDimensionDefinition::PointDistance {
            first: plan.corner,
            second: second_point,
            target: second_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    document.replace_trim_views(
        plan.first,
        vec![contact_trim_view(
            plan.first,
            plan.first_keep_start,
            first_owner,
            first_contact,
        )],
    )?;
    document.replace_trim_views(
        plan.second,
        vec![contact_trim_view(
            plan.second,
            plan.second_keep_start,
            second_owner,
            second_contact,
        )],
    )?;
    Ok(vec![
        SketchOperationIdentityChange::Split {
            source: plan.first.curve,
            retained: if plan.first_keep_start {
                SplitRetainedPiece::Before
            } else {
                SplitRetainedPiece::After
            },
            visible_piece_count: 1,
        },
        SketchOperationIdentityChange::Split {
            source: plan.second.curve,
            retained: if plan.second_keep_start {
                SplitRetainedPiece::Before
            } else {
                SplitRetainedPiece::After
            },
            visible_piece_count: 1,
        },
        SketchOperationIdentityChange::Proposed(chamfer.into()),
    ])
}

fn contact_trim_view(
    support: CurveSpan,
    keep_start: bool,
    owner: geosolve_sketch::DocumentConstraintId,
    contact: geosolve_sketch::ContactId,
) -> DocumentCurveTrimView {
    let contact = DocumentTrimBoundary::ConstraintContact { owner, contact };
    if keep_start {
        DocumentCurveTrimView {
            support,
            start: fixed_boundary(0.0),
            end: contact,
        }
    } else {
        DocumentCurveTrimView {
            support,
            start: contact,
            end: fixed_boundary(1.0),
        }
    }
}

fn apply_slot(document: &mut SketchDocument, plan: &SlotPlan) -> Result<(), DocumentError> {
    let axis = direction(plan.first_center, plan.second_center)?;
    let normal = [-axis[1], axis[0]];
    let top_first = add(plan.first_center, scale(normal, plan.radius));
    let top_second = add(plan.second_center, scale(normal, plan.radius));
    let bottom_second = add(plan.second_center, scale(normal, -plan.radius));
    let bottom_first = add(plan.first_center, scale(normal, -plan.radius));
    let centers = [
        document.add_point(format!("{}.first_center", plan.label), plan.first_center)?,
        document.add_point(format!("{}.second_center", plan.label), plan.second_center)?,
    ];
    let boundary_points = [
        document.add_point(format!("{}.top_first", plan.label), top_first)?,
        document.add_point(format!("{}.top_second", plan.label), top_second)?,
        document.add_point(format!("{}.bottom_second", plan.label), bottom_second)?,
        document.add_point(format!("{}.bottom_first", plan.label), bottom_first)?,
    ];
    document.add_curve_with_role(
        format!("{}.top", plan.label),
        CurveDefinition::Line {
            start: boundary_points[0],
            end: boundary_points[1],
            branch_direction: axis,
        },
        plan.role,
    )?;
    document.add_curve_with_role(
        format!("{}.bottom", plan.label),
        CurveDefinition::Line {
            start: boundary_points[2],
            end: boundary_points[3],
            branch_direction: [-axis[0], -axis[1]],
        },
        plan.role,
    )?;
    let right = add_slot_arc(
        document,
        &format!("{}.right", plan.label),
        centers[1],
        plan.radius,
        normal[1].atan2(normal[0]),
        (-normal[1]).atan2(-normal[0]),
        DocumentArcSweep::Clockwise,
        plan.role,
    )?;
    let left = add_slot_arc(
        document,
        &format!("{}.left", plan.label),
        centers[0],
        plan.radius,
        (-normal[1]).atan2(-normal[0]),
        normal[1].atan2(normal[0]),
        DocumentArcSweep::Clockwise,
        plan.role,
    )?;
    for (index, (point, curve, parameter, neighborhood)) in [
        (boundary_points[1], right, 0.0, ContactNeighborhood::Start),
        (boundary_points[2], right, 1.0, ContactNeighborhood::End),
        (boundary_points[3], left, 0.0, ContactNeighborhood::Start),
        (boundary_points[0], left, 1.0, ContactNeighborhood::End),
    ]
    .into_iter()
    .enumerate()
    {
        let contact = document.add_curve_contact(
            format!("{}.join_contact_{}", plan.label, index + 1),
            CurveSpan::line(curve),
            parameter,
            0,
            neighborhood,
            None,
        )?;
        document.add_constraint(
            format!("{}.join_{}", plan.label, index + 1),
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )?;
    }
    for (index, point) in centers.into_iter().chain(boundary_points).enumerate() {
        let target = document.point(point).expect("new slot point").position;
        document.add_constraint(
            format!("{}.fixed_{}", plan.label, index + 1),
            DocumentConstraintDefinition::FixedPoint { point, target },
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_slot_arc(
    document: &mut SketchDocument,
    label: &str,
    center: DesignPointId,
    radius: f64,
    start: f64,
    end: f64,
    sweep: DocumentArcSweep,
    role: GeometryRole,
) -> Result<CurveId, DocumentError> {
    let radius = document.add_scalar(
        format!("{label}.radius"),
        radius,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let start_angle = document.add_scalar(
        format!("{label}.start_angle"),
        start,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let end_angle = document.add_scalar(
        format!("{label}.end_angle"),
        end,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    document.add_curve_with_role(
        format!("{label}.arc"),
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        },
        role,
    )
}

fn copy_point_defined_curve(
    document: &mut SketchDocument,
    label: &str,
    source: CurveId,
    offset: [f64; 2],
) -> Result<CurveId, DocumentError> {
    let role = document
        .geometry_role(source)
        .ok_or_else(|| unknown_curve(source))?;
    let definition = document
        .curve(source)
        .ok_or_else(|| unknown_curve(source))?
        .definition
        .clone();
    let controls =
        point_defined_controls(&definition).ok_or_else(|| DocumentError::InvalidField {
            field: "pattern source",
            message: "source family has no exact point-defined copy".into(),
        })?;
    let copied = controls
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let position = document
                .point(*point)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "pattern source point",
                    message: "source control is missing".into(),
                })?
                .position;
            document.add_point(
                format!("{label}.point_{}", index + 1),
                add(position, offset),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let copied_definition = remap_point_defined_curve(&definition, &copied)?;
    document.add_curve_with_role(format!("{label}.curve"), copied_definition, role)
}

fn remap_point_defined_curve(
    definition: &CurveDefinition,
    controls: &[DesignPointId],
) -> Result<CurveDefinition, DocumentError> {
    Ok(match definition {
        CurveDefinition::Line {
            branch_direction, ..
        } => CurveDefinition::Line {
            start: controls[0],
            end: controls[1],
            branch_direction: *branch_direction,
        },
        CurveDefinition::Polyline {
            closed,
            branch_directions,
            ..
        } => CurveDefinition::Polyline {
            points: controls.to_vec(),
            closed: *closed,
            branch_directions: branch_directions.clone(),
        },
        CurveDefinition::QuadraticBezier { .. } => CurveDefinition::QuadraticBezier {
            controls: controls
                .try_into()
                .map_err(|_| invalid_copy_shape("quadratic Bezier"))?,
        },
        CurveDefinition::CubicBezier { .. } => CurveDefinition::CubicBezier {
            controls: controls
                .try_into()
                .map_err(|_| invalid_copy_shape("cubic Bezier"))?,
        },
        CurveDefinition::BSpline {
            form,
            degree,
            knots,
            span_ids,
            next_span_id,
            ..
        } => CurveDefinition::BSpline {
            form: *form,
            degree: *degree,
            controls: controls.to_vec(),
            knots: knots.clone(),
            span_ids: span_ids.clone(),
            next_span_id: *next_span_id,
        },
        _ => return Err(invalid_copy_shape("unsupported curve family")),
    })
}

fn invalid_copy_shape(family: &str) -> DocumentError {
    DocumentError::InvalidField {
        field: "operation curve copy",
        message: format!("invalid {family} control topology"),
    }
}

fn point_defined_controls(definition: &CurveDefinition) -> Option<Vec<DesignPointId>> {
    match definition {
        CurveDefinition::Line { start, end, .. } => Some(vec![*start, *end]),
        CurveDefinition::Polyline { points, .. } => Some(points.clone()),
        CurveDefinition::QuadraticBezier { controls } => Some(controls.to_vec()),
        CurveDefinition::CubicBezier { controls } => Some(controls.to_vec()),
        CurveDefinition::BSpline { controls, .. } => Some(controls.clone()),
        _ => None,
    }
}

enum ProfileOffsetPlanFailure {
    Unsupported(SketchOperationUnsupportedReason),
    Incomplete(SketchOperationIncompleteReason),
}

fn plan_profile_offset_operand(
    document: &SketchDocument,
    index: &OffsetOperandIndex,
    operand: &SketchProfileOffsetOperand,
) -> Result<(DocumentProfileOffsetCreationOperand, Vec<CurveSpan>), ProfileOffsetPlanFailure> {
    match operand {
        SketchProfileOffsetOperand::Face { key, direction } => {
            let candidate = index.face(key).ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetFaceMissing { key: key.clone() },
                )
            })?;
            if let Some(reasons) = disabled_offset_reasons(&candidate.eligibility) {
                return Err(ProfileOffsetPlanFailure::Unsupported(
                    SketchOperationUnsupportedReason::ProfileOffsetFace {
                        key: key.clone(),
                        reasons,
                    },
                ));
            }
            let span_count = key.outer.spans.len()
                + key.holes.iter().map(|hole| hole.spans.len()).sum::<usize>();
            if span_count == 0 || span_count > MAX_PROFILE_OFFSET_SPANS {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetSpanLimitExceeded,
                ));
            }
            let mut seen = BTreeSet::new();
            let outer =
                plan_profile_offset_path(document, index, &key.outer.spans, true, &mut seen)?;
            let holes = key
                .holes
                .iter()
                .map(|hole| plan_profile_offset_path(document, index, &hole.spans, true, &mut seen))
                .collect::<Result<Vec<_>, _>>()?;
            let sources = seen.into_iter().collect();
            Ok((
                DocumentProfileOffsetCreationOperand::Face {
                    direction: *direction,
                    outer,
                    holes,
                },
                sources,
            ))
        }
        SketchProfileOffsetOperand::OpenChain { spans, side } => {
            if spans.is_empty() {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetEmptyChain,
                ));
            }
            if spans.len() > MAX_PROFILE_OFFSET_SPANS {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetSpanLimitExceeded,
                ));
            }
            for directed in spans {
                let candidate = index.span(directed.span).ok_or({
                    ProfileOffsetPlanFailure::Incomplete(
                        SketchOperationIncompleteReason::ProfileOffsetSpanMissing {
                            span: directed.span,
                        },
                    )
                })?;
                if let Some(reasons) = disabled_offset_reasons(&candidate.eligibility) {
                    return Err(ProfileOffsetPlanFailure::Unsupported(
                        SketchOperationUnsupportedReason::ProfileOffsetSpan {
                            span: directed.span,
                            reasons,
                        },
                    ));
                }
                if candidate.periodic {
                    return Err(ProfileOffsetPlanFailure::Unsupported(
                        SketchOperationUnsupportedReason::ProfileOffsetPeriodicChain {
                            span: directed.span,
                        },
                    ));
                }
            }
            let mut seen = BTreeSet::new();
            let chain = plan_profile_offset_path(document, index, spans, false, &mut seen)?;
            if spans.len() > 1
                && index
                    .adjacent_endpoints(directed_offset_endpoint(spans[0], true))
                    .any(|endpoint| {
                        endpoint == directed_offset_endpoint(spans[spans.len() - 1], false)
                    })
            {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetClosedChain,
                ));
            }
            let sources = seen.into_iter().collect();
            Ok((
                DocumentProfileOffsetCreationOperand::OpenChain { side: *side, chain },
                sources,
            ))
        }
    }
}

fn disabled_offset_reasons(
    eligibility: &OffsetOperandEligibility,
) -> Option<Vec<OffsetOperandIneligibility>> {
    match eligibility {
        OffsetOperandEligibility::Eligible => None,
        OffsetOperandEligibility::Disabled { reasons } => Some(reasons.clone()),
    }
}

fn plan_profile_offset_path(
    document: &SketchDocument,
    index: &OffsetOperandIndex,
    spans: &[OffsetDirectedSpan],
    closed: bool,
    seen: &mut BTreeSet<CurveSpan>,
) -> Result<DocumentProfileOffsetCreationPath, ProfileOffsetPlanFailure> {
    let mut edges = Vec::with_capacity(spans.len());
    for directed in spans {
        if !seen.insert(directed.span) {
            return Err(ProfileOffsetPlanFailure::Incomplete(
                SketchOperationIncompleteReason::ProfileOffsetDuplicateSpan {
                    span: directed.span,
                },
            ));
        }
        edges.push(DocumentDirectedProfileOffsetCurve {
            curve: directed.span,
            traversal: document_offset_traversal(directed.traversal),
        });
    }
    let junction_count = if closed && !is_periodic_offset_path(index, spans) {
        spans.len()
    } else {
        spans.len().saturating_sub(1)
    };
    let mut junctions = Vec::with_capacity(junction_count);
    for current in 0..junction_count {
        let next = (current + 1) % spans.len();
        let incoming_endpoint = directed_offset_endpoint(spans[current], false);
        let outgoing_endpoint = directed_offset_endpoint(spans[next], true);
        if !closed {
            ensure_offset_endpoint_not_branched(index, incoming_endpoint)?;
            ensure_offset_endpoint_not_branched(index, outgoing_endpoint)?;
        }
        let adjacency = find_offset_adjacency(index, incoming_endpoint, outgoing_endpoint)
            .ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                        incoming: spans[current].span,
                        outgoing: spans[next].span,
                    },
                )
            })?;
        let owner = adjacency
            .owners
            .first()
            .copied()
            .map(document_profile_offset_owner)
            .ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                        incoming: spans[current].span,
                        outgoing: spans[next].span,
                    },
                )
            })?;
        let branch = profile_offset_junction_branch(
            document,
            spans[current],
            spans[next],
            adjacency
                .owners
                .iter()
                .copied()
                .any(|owner| offset_join_is_explicitly_tangent(document, owner)),
        )
        .ok_or_else(|| {
            ProfileOffsetPlanFailure::Incomplete(
                SketchOperationIncompleteReason::ProfileOffsetDegenerateJunction {
                    incoming: spans[current].span,
                    outgoing: spans[next].span,
                },
            )
        })?;
        junctions.push(DocumentProfileOffsetCreationJunction {
            source_owner: owner,
            branch,
        });
    }
    Ok(DocumentProfileOffsetCreationPath { edges, junctions })
}

fn is_periodic_offset_path(index: &OffsetOperandIndex, spans: &[OffsetDirectedSpan]) -> bool {
    spans.len() == 1
        && index
            .span(spans[0].span)
            .is_some_and(|candidate| candidate.periodic)
}

fn ensure_offset_endpoint_not_branched(
    index: &OffsetOperandIndex,
    endpoint: OffsetEndpointRef,
) -> Result<(), ProfileOffsetPlanFailure> {
    let branched = index
        .span(endpoint.span)
        .and_then(|span| {
            span.endpoints
                .iter()
                .find(|candidate| candidate.endpoint == endpoint)
        })
        .is_some_and(|candidate| {
            matches!(
                candidate.eligibility,
                OffsetEndpointEligibility::Branched { .. }
            )
        });
    if branched {
        Err(ProfileOffsetPlanFailure::Incomplete(
            SketchOperationIncompleteReason::ProfileOffsetBranchedJoin { endpoint },
        ))
    } else {
        Ok(())
    }
}

fn find_offset_adjacency(
    index: &OffsetOperandIndex,
    first: OffsetEndpointRef,
    second: OffsetEndpointRef,
) -> Option<&geosolve_sketch_topology::OffsetEndpointAdjacency> {
    let endpoints = if first < second {
        [first, second]
    } else {
        [second, first]
    };
    index
        .adjacencies()
        .iter()
        .find(|adjacency| adjacency.endpoints == endpoints)
}

const fn document_profile_offset_owner(
    owner: OffsetJoinOwner,
) -> DocumentProfileOffsetJunctionOwner {
    match owner {
        OffsetJoinOwner::SharedPoint(point) => {
            DocumentProfileOffsetJunctionOwner::SharedPoint(point)
        }
        OffsetJoinOwner::Constraint(constraint) => {
            DocumentProfileOffsetJunctionOwner::Constraint(constraint)
        }
    }
}

fn offset_join_is_explicitly_tangent(document: &SketchDocument, owner: OffsetJoinOwner) -> bool {
    let OffsetJoinOwner::Constraint(owner) = owner else {
        return false;
    };
    document.constraint(owner).is_some_and(|constraint| {
        matches!(
            &constraint.definition,
            DocumentConstraintDefinition::LineCircleTangency { .. }
                | DocumentConstraintDefinition::CircleArcTangency { .. }
                | DocumentConstraintDefinition::LineCurveTangency { .. }
                | DocumentConstraintDefinition::CurveCurveTangency { .. }
                | DocumentConstraintDefinition::EndpointContinuity {
                    continuity: DocumentCurveContinuity::G1
                        | DocumentCurveContinuity::G2
                        | DocumentCurveContinuity::ParametricC2 { .. },
                    ..
                }
        )
    })
}

fn profile_offset_junction_branch(
    document: &SketchDocument,
    incoming: OffsetDirectedSpan,
    outgoing: OffsetDirectedSpan,
    explicitly_tangent: bool,
) -> Option<DocumentProfileOffsetJunctionBranch> {
    let incoming_tangent = directed_offset_tangent(document, incoming, false)?;
    let outgoing_tangent = directed_offset_tangent(document, outgoing, true)?;
    let cross_value = cross(incoming_tangent, outgoing_tangent);
    let alignment = incoming_tangent[0].mul_add(
        outgoing_tangent[0],
        incoming_tangent[1] * outgoing_tangent[1],
    );
    if explicitly_tangent || cross_value.abs() <= PROFILE_OFFSET_TANGENT_CROSS_TOLERANCE {
        (alignment > 0.0).then_some(DocumentProfileOffsetJunctionBranch::Tangent)
    } else {
        Some(DocumentProfileOffsetJunctionBranch::Miter {
            turn: if cross_value.is_sign_positive() {
                DocumentProfileOffsetTurn::Left
            } else {
                DocumentProfileOffsetTurn::Right
            },
        })
    }
}

fn directed_offset_tangent(
    document: &SketchDocument,
    directed: OffsetDirectedSpan,
    at_start: bool,
) -> Option<[f64; 2]> {
    let parameter = match (directed.traversal, at_start) {
        (OffsetTraversal::Forward, true) | (OffsetTraversal::Reverse, false) => 0.0,
        (OffsetTraversal::Forward, false) | (OffsetTraversal::Reverse, true) => 1.0,
    };
    let differential = document
        .evaluate_curve_jet(directed.span, parameter)
        .ok()?
        .differential()
        .ok()?;
    let sign = match directed.traversal {
        OffsetTraversal::Forward => 1.0,
        OffsetTraversal::Reverse => -1.0,
    };
    Some([
        differential.unit_tangent.x * sign,
        differential.unit_tangent.y * sign,
    ])
}

const fn document_offset_traversal(traversal: OffsetTraversal) -> DocumentOffsetTraversal {
    match traversal {
        OffsetTraversal::Forward => DocumentOffsetTraversal::Forward,
        OffsetTraversal::Reverse => DocumentOffsetTraversal::Reverse,
    }
}

const fn directed_offset_endpoint(
    directed: OffsetDirectedSpan,
    at_start: bool,
) -> OffsetEndpointRef {
    let endpoint = match (directed.traversal, at_start) {
        (OffsetTraversal::Forward, true) | (OffsetTraversal::Reverse, false) => {
            OffsetEndpointRole::Start
        }
        (OffsetTraversal::Forward, false) | (OffsetTraversal::Reverse, true) => {
            OffsetEndpointRole::End
        }
    };
    OffsetEndpointRef {
        span: directed.span,
        endpoint,
    }
}

fn plan_chamfer(
    document: &SketchDocument,
    label: &str,
    first: CurveSpan,
    second: CurveSpan,
    first_distance: f64,
    second_distance: f64,
) -> Result<ChamferPlan, GeometryPlanFailure> {
    let first_endpoints =
        line_endpoint_ids(document, first).ok_or(GeometryPlanFailure::Unsupported(first.curve))?;
    let second_endpoints = line_endpoint_ids(document, second)
        .ok_or(GeometryPlanFailure::Unsupported(second.curve))?;
    let shared = [first_endpoints.0, first_endpoints.1]
        .into_iter()
        .filter(|candidate| *candidate == second_endpoints.0 || *candidate == second_endpoints.1)
        .collect::<Vec<_>>();
    let [corner] = shared.as_slice() else {
        return Err(GeometryPlanFailure::Incomplete(
            SketchOperationIncompleteReason::LinesDoNotShareOneEndpoint,
        ));
    };
    let first_other = if first_endpoints.0 == *corner {
        first_endpoints.1
    } else {
        first_endpoints.0
    };
    let second_other = if second_endpoints.0 == *corner {
        second_endpoints.1
    } else {
        second_endpoints.0
    };
    let corner_position = point_position(document, *corner);
    let first_other_position = point_position(document, first_other);
    let second_other_position = point_position(document, second_other);
    let first_length = squared_distance(corner_position, first_other_position).sqrt();
    let second_length = squared_distance(corner_position, second_other_position).sqrt();
    if first_distance >= first_length || second_distance >= second_length {
        return Err(GeometryPlanFailure::Incomplete(
            SketchOperationIncompleteReason::ParameterNotInsideOneVisibleInterval,
        ));
    }
    let first_fraction = first_distance / first_length;
    let second_fraction = second_distance / second_length;
    let first_position = lerp(corner_position, first_other_position, first_fraction);
    let second_position = lerp(corner_position, second_other_position, second_fraction);
    let first_corner_is_start = first_endpoints.0 == *corner;
    let second_corner_is_start = second_endpoints.0 == *corner;
    Ok(ChamferPlan {
        label: label.to_owned(),
        first,
        second,
        corner: *corner,
        first_position,
        second_position,
        first_parameter: if first_corner_is_start {
            first_fraction
        } else {
            1.0 - first_fraction
        },
        second_parameter: if second_corner_is_start {
            second_fraction
        } else {
            1.0 - second_fraction
        },
        first_keep_start: !first_corner_is_start,
        second_keep_start: !second_corner_is_start,
        first_distance,
        second_distance,
    })
}

fn plan_line_extension(
    document: &SketchDocument,
    line: CurveSpan,
    endpoint: LineEndpoint,
    target: CurveSpan,
) -> Result<(DesignPointId, [f64; 2]), GeometryPlanFailure> {
    let source =
        line_endpoint_ids(document, line).ok_or(GeometryPlanFailure::Unsupported(line.curve))?;
    let target_endpoints = line_endpoint_ids(document, target)
        .ok_or(GeometryPlanFailure::Unsupported(target.curve))?;
    let first = point_position(document, source.0);
    let second = point_position(document, source.1);
    let target_first = point_position(document, target_endpoints.0);
    let target_second = point_position(document, target_endpoints.1);
    let source_direction = subtract(second, first);
    let target_direction = subtract(target_second, target_first);
    let denominator = cross(source_direction, target_direction);
    let characteristic = squared_norm(source_direction)
        .sqrt()
        .max(squared_norm(target_direction).sqrt())
        .max(1.0);
    if denominator.abs() <= f64::EPSILON * characteristic * characteristic * 16.0 {
        return Err(GeometryPlanFailure::Incomplete(
            SketchOperationIncompleteReason::ParallelLines,
        ));
    }
    let parameter = cross(subtract(target_first, first), target_direction) / denominator;
    let extends = match endpoint {
        LineEndpoint::Start => parameter < -PARAMETER_EPSILON,
        LineEndpoint::End => parameter > 1.0 + PARAMETER_EPSILON,
    };
    if !extends {
        return Err(GeometryPlanFailure::Incomplete(
            SketchOperationIncompleteReason::IntersectionDoesNotExtendSelectedEndpoint,
        ));
    }
    let position = add(first, scale(source_direction, parameter));
    Ok((
        match endpoint {
            LineEndpoint::Start => source.0,
            LineEndpoint::End => source.1,
        },
        position,
    ))
}

enum GeometryPlanFailure {
    Unsupported(CurveId),
    Incomplete(SketchOperationIncompleteReason),
}

fn line_endpoint_ids(
    document: &SketchDocument,
    span: CurveSpan,
) -> Option<(DesignPointId, DesignPointId)> {
    let curve = document.curve(span.curve)?;
    match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => Some((*start, *end)),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = usize::try_from(span.segment).ok()?;
            let first = *points.get(index)?;
            let second = if index + 1 < points.len() {
                points[index + 1]
            } else if *closed {
                points[0]
            } else {
                return None;
            };
            Some((first, second))
        }
        _ => None,
    }
}

fn is_point_defined_mirror_family(
    document: &SketchDocument,
    curve: CurveId,
) -> Result<bool, DocumentError> {
    let definition = &document
        .curve(curve)
        .ok_or_else(|| unknown_curve(curve))?
        .definition;
    Ok(matches!(
        definition,
        CurveDefinition::Line { .. }
            | CurveDefinition::Polyline { .. }
            | CurveDefinition::QuadraticBezier { .. }
            | CurveDefinition::CubicBezier { .. }
            | CurveDefinition::BSpline { .. }
    ))
}

fn is_point_defined_pattern_family(
    document: &SketchDocument,
    curve: CurveId,
) -> Result<bool, DocumentError> {
    is_point_defined_mirror_family(document, curve)
}

fn is_full_periodic_support(
    document: &SketchDocument,
    support: CurveSpan,
) -> Result<bool, DocumentError> {
    let curve = document
        .curve(support.curve)
        .ok_or_else(|| unknown_curve(support.curve))?;
    document.curve_spans(support.curve)?;
    Ok(matches!(
        curve.definition,
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
    ))
}

fn document_elements(document: &SketchDocument) -> BTreeSet<DocumentElementId> {
    let mut elements = BTreeSet::new();
    elements.insert(DocumentElementId::Document(document.id()));
    elements.extend(
        document
            .points()
            .iter()
            .map(|point| DocumentElementId::Point(point.id)),
    );
    elements.extend(
        document
            .scalars()
            .iter()
            .map(|scalar| DocumentElementId::Scalar(scalar.id)),
    );
    elements.extend(
        document
            .curves()
            .iter()
            .map(|curve| DocumentElementId::Curve(curve.id)),
    );
    elements.extend(
        document
            .contacts()
            .iter()
            .map(|contact| DocumentElementId::Contact(contact.id)),
    );
    elements.extend(
        document
            .constraints()
            .iter()
            .map(|constraint| DocumentElementId::Constraint(constraint.id)),
    );
    elements.extend(
        document
            .dimensions()
            .iter()
            .map(|dimension| DocumentElementId::Dimension(dimension.id)),
    );
    elements.extend(
        document
            .sources()
            .map(|source| DocumentElementId::Source(source.id)),
    );
    elements
}

fn identity_change_mentions(
    change: &SketchOperationIdentityChange,
    element: DocumentElementId,
) -> bool {
    match change {
        SketchOperationIdentityChange::Retained(id)
        | SketchOperationIdentityChange::Replaced(id)
        | SketchOperationIdentityChange::Proposed(id) => *id == element,
        SketchOperationIdentityChange::Split { source, .. } => {
            DocumentElementId::Curve(*source) == element
        }
    }
}

fn fixed_view(support: CurveSpan, start: f64, end: f64) -> DocumentCurveTrimView {
    DocumentCurveTrimView {
        support,
        start: fixed_boundary(start),
        end: fixed_boundary(end),
    }
}

fn fixed_boundary(parameter: f64) -> DocumentTrimBoundary {
    DocumentTrimBoundary::Fixed(DocumentTrimParameter {
        parameter,
        winding: 0,
    })
}

fn containing_interval(
    intervals: &[geosolve_sketch::DocumentVisibleCurveInterval],
    parameter: f64,
) -> Option<usize> {
    intervals
        .iter()
        .position(|interval| interval.start <= parameter && parameter <= interval.end)
}

fn strictly_inside(parameter: f64, start: f64, end: f64) -> bool {
    parameter > start + PARAMETER_EPSILON && parameter < end - PARAMETER_EPSILON
}

fn local_neighborhood(parameter: f64) -> ContactNeighborhood {
    let radius = 0.2_f64.min(parameter * 0.5).min((1.0 - parameter) * 0.5);
    ContactNeighborhood::Local {
        lower: parameter - radius,
        upper: parameter + radius,
    }
}

fn request_operand_count(request: &SketchOperationRequest) -> usize {
    match request {
        SketchOperationRequest::Split { .. }
        | SketchOperationRequest::Break { .. }
        | SketchOperationRequest::Trim { .. }
        | SketchOperationRequest::Mirror { .. } => 1,
        SketchOperationRequest::ExtendLineToLine { .. }
        | SketchOperationRequest::Chamfer { .. }
        | SketchOperationRequest::AssociativeFillet { .. } => 2,
        SketchOperationRequest::Rectangle { .. }
        | SketchOperationRequest::RegularPolygon { .. }
        | SketchOperationRequest::Slot { .. } => 0,
        SketchOperationRequest::LinearPattern { sources, .. } => sources.len(),
        SketchOperationRequest::ProfileOffset { operand, .. } => match operand {
            SketchProfileOffsetOperand::Face { key, .. } => {
                key.outer.spans.len() + key.holes.iter().map(|hole| hole.spans.len()).sum::<usize>()
            }
            SketchProfileOffsetOperand::OpenChain { spans, .. } => spans.len(),
        },
    }
}

fn ensure_finite(value: f64, field: &'static str) -> Result<(), SketchOperationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SketchOperationError::InvalidRequest {
            field,
            message: "must be finite",
        })
    }
}

fn ensure_positive(value: f64, field: &'static str) -> Result<(), SketchOperationError> {
    ensure_finite(value, field)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(SketchOperationError::InvalidRequest {
            field,
            message: "must be positive",
        })
    }
}

fn ensure_pair(value: [f64; 2], field: &'static str) -> Result<(), SketchOperationError> {
    ensure_finite(value[0], field)?;
    ensure_finite(value[1], field)
}

fn small_index(index: usize) -> f64 {
    f64::from(u32::try_from(index).expect("operation counts are bounded to u32"))
}

fn point_position(document: &SketchDocument, point: DesignPointId) -> [f64; 2] {
    document
        .point(point)
        .expect("validated curve point must exist")
        .position
}

fn direction(first: [f64; 2], second: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    let delta = subtract(second, first);
    let norm = squared_norm(delta).sqrt();
    if !norm.is_finite() || norm <= f64::MIN_POSITIVE {
        return Err(DocumentError::InvalidField {
            field: "operation line direction",
            message: "endpoints must define a finite nonzero direction".into(),
        });
    }
    Ok([delta[0] / norm, delta[1] / norm])
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

fn scale(vector: [f64; 2], factor: f64) -> [f64; 2] {
    [vector[0] * factor, vector[1] * factor]
}

fn lerp(first: [f64; 2], second: [f64; 2], amount: f64) -> [f64; 2] {
    add(first, scale(subtract(second, first), amount))
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

fn squared_norm(vector: [f64; 2]) -> f64 {
    vector[0].mul_add(vector[0], vector[1] * vector[1])
}

fn squared_distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    squared_norm(subtract(first, second))
}

fn unknown_curve(curve: CurveId) -> DocumentError {
    DocumentError::InvalidField {
        field: "operation curve",
        message: format!("unknown curve {curve}"),
    }
}
