// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic, equation-free drafting operations over public sketch APIs.
//!
//! This crate never owns solver state or residual formulas. It prepares immutable
//! proposals from a complete stamped sketch snapshot and applies them only through
//! the retained document transaction boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use geosolve_sketch::{
    ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition, CurveId, CurveSpan,
    DesignPointId, DocumentArcSweep, DocumentConstraintDefinition, DocumentCurveContinuity,
    DocumentCurveTrimView, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentDirectedProfileOffsetCurve, DocumentEdit, DocumentElementId, DocumentError,
    DocumentFaceOffsetDirection, DocumentLineSide, DocumentOffsetTraversal,
    DocumentPreparedProfileOffsetGeometry, DocumentProfileOffsetCreationJunction,
    DocumentProfileOffsetCreationOperand, DocumentProfileOffsetCreationPath,
    DocumentProfileOffsetCreationRequest, DocumentProfileOffsetEdgePair,
    DocumentProfileOffsetJunction, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetOperand, DocumentProfileOffsetTurn,
    DocumentTrimBoundary, DocumentTrimParameter, GeometryRole, MirroredCurveIds,
    OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationWorkCounter, PreparedSketchInput, RectangleIds, RetainedDocumentTransactionOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchAcceptedStateIdentity,
    SketchDesignIdentity, SketchDocument, SketchMaterializationIdentityKind,
    SketchMaterializationIdentityReservation,
};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetEndpointRef, OffsetEndpointRole, OffsetFaceKey, OffsetJoinOwner,
    OffsetOperandEligibility, OffsetOperandIndex, OffsetOperandIneligibility, OffsetTraversal,
};
use thiserror::Error;

mod profile_offset;

use profile_offset::{ProfileOffsetPlanFailure, plan_profile_offset_operand};

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

/// One code-owned segment in an equation-free operation-output path.
///
/// Field spellings are owned by this crate rather than inferred from labels or persistent IDs.
/// Indices identify intrinsically ordered results such as polygon vertices and pattern instances.
/// Source-keyed segments retain the exact source point, curve, span, or Profile Offset junction
/// that owns a dynamic member, so inserting or reordering unrelated inputs cannot silently relabel
/// an existing output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationOutputPathSegment {
    Field(&'static str),
    Index(usize),
    SourceControl {
        curve: CurveId,
        point: DesignPointId,
        ordinal: usize,
    },
    SourceCurve(CurveId),
    SourceSpan(CurveSpan),
    SourceJunction(DocumentProfileOffsetJunctionOwner),
}

/// Stable semantic ownership of one native identity generated by an operation.
///
/// The native identity kind remains independently authenticated. This role describes why the
/// identity exists without exposing solver equations or relying on its allocation ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOperationOutputRole {
    Geometry,
    ContactParameter,
    Contact,
    Constraint,
    DimensionTarget,
    Dimension,
    Parameter,
    ExternalBinding,
}

/// One authenticated native output slot generated by an existing sketch operation.
///
/// Slots are ordered by the operation's ordinary persistent allocation order. Constraint and
/// dimension slots each own their immediately following audit-source identity atomically. `path`
/// and `role` are generated by the native operation and remain stable across reserved replay even
/// though the substituted persistent IDs differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchOperationOutputSlot {
    pub ordinal: usize,
    pub kind: SketchMaterializationIdentityKind,
    /// Exact number of native spans exposed by a generated curve. Non-curve
    /// outputs always carry zero. Intent replay needs this cardinality before
    /// it can reserve the operation node's stable span ports.
    pub curve_span_count: u16,
    pub path: Vec<SketchOperationOutputPathSegment>,
    pub role: SketchOperationOutputRole,
}

/// Equation-free native output inventory prepared from one exact operation proposal.
///
/// This plan is generated by the Rust operation implementation; callers provide only matching
/// typed reservations and cannot replace the operation's output manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchOperationOutputPlan {
    pub kind: SketchOperationKind,
    pub slots: Vec<SketchOperationOutputSlot>,
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
    output_plan: SketchOperationOutputPlan,
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

    /// Returns the authenticated native-output inventory generated by this exact proposal.
    #[must_use]
    pub const fn output_plan(&self) -> &SketchOperationOutputPlan {
        &self.output_plan
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

    /// Applies this proposal while consuming exact, already-retired native reservations.
    ///
    /// The reservation sequence must match [`Self::output_plan`] in category and order. The
    /// operation still executes through its ordinary public document constructors; the sketch
    /// document merely substitutes the supplied never-reused identities for that scoped atomic
    /// action. A reservation mismatch, stale proposal, or incomplete consumption changes no live
    /// session state. As with [`Self::apply`], an ordinary solve rejection remains retained design
    /// intent, while Profile Offset requires fresh accepted publication and rejects atomically.
    ///
    /// # Errors
    ///
    /// Returns a typed output-plan mismatch, stale input, deterministic replay, document, or
    /// session error.
    pub fn apply_with_reserved_outputs(
        &self,
        session: &mut RetainedSketchDocumentSession,
        reservations: &[SketchMaterializationIdentityReservation],
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
        self.validate_reserved_outputs(reservations)?;
        if self.requires_accepted_publication() {
            let mut candidate = session.clone();
            let outcome =
                self.apply_retained_with_reserved_outputs(&mut candidate, reservations)?;
            if outcome.published_accepted_identity().is_none() {
                return Err(SketchOperationApplyError::ProfileOffsetRejected);
            }
            *session = candidate;
            return Ok(outcome);
        }
        self.apply_retained_with_reserved_outputs(session, reservations)
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

    /// Controlled counterpart to [`Self::apply_with_reserved_outputs`].
    ///
    /// Cancellation or deterministic work exhaustion consumes no reservation and changes no
    /// session state. A completed action has the same exact reservation authentication and
    /// publication semantics as the uncontrolled method.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::apply_with_reserved_outputs`].
    pub fn apply_controlled_with_reserved_outputs(
        &self,
        session: &mut RetainedSketchDocumentSession,
        reservations: &[SketchMaterializationIdentityReservation],
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
        self.validate_reserved_outputs(reservations)?;
        if self.requires_accepted_publication() {
            let mut candidate = session.clone();
            let outcome = self.apply_retained_controlled_with_reserved_outputs(
                &mut candidate,
                reservations,
                control,
            )?;
            if let OperationOutcome::Completed { value, .. } = &outcome {
                if value.published_accepted_identity().is_none() {
                    return Err(SketchOperationApplyError::ProfileOffsetRejected);
                }
                *session = candidate;
            }
            return Ok(outcome);
        }
        self.apply_retained_controlled_with_reserved_outputs(session, reservations, control)
    }

    const fn requires_accepted_publication(&self) -> bool {
        matches!(&self.plan, PlannedOperation::ProfileOffset { .. })
    }

    fn validate_reserved_outputs(
        &self,
        reservations: &[SketchMaterializationIdentityReservation],
    ) -> Result<(), SketchOperationApplyError> {
        let actual = reservations
            .iter()
            .copied()
            .map(SketchMaterializationIdentityReservation::kind)
            .collect::<Vec<_>>();
        let expected = self
            .output_plan
            .slots
            .iter()
            .map(|slot| slot.kind)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(SketchOperationApplyError::ReservedOutputMismatch { expected, actual });
        }
        Ok(())
    }

    fn apply_retained(
        &self,
        session: &mut RetainedSketchDocumentSession,
    ) -> Result<
        RetainedDocumentTransactionOutcome<SketchOperationApplication>,
        SketchOperationApplyError,
    > {
        let expected_application = self.expected.clone();
        let expected_output_plan = self.output_plan.clone();
        let plan = self.plan.clone();
        Ok(
            session.transact(self.input.design_identity(), move |document| {
                let planned_application = plan.apply(document)?;
                if planned_application.application != expected_application
                    || planned_application.output_plan != expected_output_plan
                {
                    return Err(DocumentError::InvalidField {
                        field: "operation proposal replay",
                        message:
                            "same stamped document did not reproduce the prepared identity and semantic-output maps"
                                .into(),
                    });
                }
                Ok(planned_application.application)
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
        let expected_output_plan = self.output_plan.clone();
        let plan = self.plan.clone();
        Ok(session.transact_controlled(
            self.input.design_identity(),
            move |document| {
                let planned_application = plan.apply(document)?;
                if planned_application.application != expected_application
                    || planned_application.output_plan != expected_output_plan
                {
                    return Err(DocumentError::InvalidField {
                        field: "operation proposal replay",
                        message:
                            "same stamped document did not reproduce the prepared identity and semantic-output maps"
                                .into(),
                    });
                }
                Ok(planned_application.application)
            },
            control,
        )?)
    }

    fn apply_retained_with_reserved_outputs(
        &self,
        session: &mut RetainedSketchDocumentSession,
        reservations: &[SketchMaterializationIdentityReservation],
    ) -> Result<
        RetainedDocumentTransactionOutcome<SketchOperationApplication>,
        SketchOperationApplyError,
    > {
        let expected_application = self.expected.clone();
        let output_plan = self.output_plan.clone();
        let plan = self.plan.clone();
        let reservations = reservations.to_vec();
        Ok(
            session.transact(self.input.design_identity(), move |document| {
                let planned_application = document
                    .apply_reserved_materialization_action(&reservations, move |candidate| {
                        plan.apply(candidate)
                    })?;
                validate_reserved_application(
                    &planned_application,
                    &expected_application,
                    &output_plan,
                )?;
                Ok(planned_application.application)
            })?,
        )
    }

    fn apply_retained_controlled_with_reserved_outputs(
        &self,
        session: &mut RetainedSketchDocumentSession,
        reservations: &[SketchMaterializationIdentityReservation],
        control: OperationControl,
    ) -> Result<
        OperationOutcome<RetainedDocumentTransactionOutcome<SketchOperationApplication>>,
        SketchOperationApplyError,
    > {
        let expected_application = self.expected.clone();
        let output_plan = self.output_plan.clone();
        let plan = self.plan.clone();
        let reservations = reservations.to_vec();
        Ok(session.transact_controlled(
            self.input.design_identity(),
            move |document| {
                let planned_application = document
                    .apply_reserved_materialization_action(&reservations, move |candidate| {
                        plan.apply(candidate)
                    })?;
                validate_reserved_application(
                    &planned_application,
                    &expected_application,
                    &output_plan,
                )?;
                Ok(planned_application.application)
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
    #[error("reserved operation outputs do not match the authenticated output plan")]
    ReservedOutputMismatch {
        expected: Vec<SketchMaterializationIdentityKind>,
        actual: Vec<SketchMaterializationIdentityKind>,
    },
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannedOperationOutput {
    element: DocumentElementId,
    path: Vec<SketchOperationOutputPathSegment>,
    role: SketchOperationOutputRole,
}

#[derive(Clone, Debug, PartialEq)]
struct PlannedOperationApplication {
    application: SketchOperationApplication,
    output_plan: SketchOperationOutputPlan,
}

#[derive(Clone, Debug)]
struct PolygonOutputIds {
    points: Vec<DesignPointId>,
    curves: Vec<CurveId>,
}

#[derive(Clone, Debug)]
struct SlotOutputIds {
    centers: [DesignPointId; 2],
    boundary_points: [DesignPointId; 4],
    edges: [CurveId; 2],
    arcs: [SlotArcOutputIds; 2],
    joins: Vec<(
        geosolve_sketch::ContactId,
        geosolve_sketch::DocumentConstraintId,
    )>,
    fixed: Vec<geosolve_sketch::DocumentConstraintId>,
}

#[derive(Clone, Copy, Debug)]
struct SlotArcOutputIds {
    radius: geosolve_sketch::DesignScalarId,
    start_angle: geosolve_sketch::DesignScalarId,
    end_angle: geosolve_sketch::DesignScalarId,
    curve: CurveId,
}

#[derive(Clone, Debug)]
struct PatternCopyOutputIds {
    instance: usize,
    source: CurveId,
    source_controls: Vec<DesignPointId>,
    controls: Vec<DesignPointId>,
    curve: CurveId,
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
            if accepted.document != snapshot.design
                && (!point_defined_curve_is_unchanged(
                    &snapshot.design,
                    &accepted.document,
                    *source,
                )? || !point_defined_curve_is_unchanged(
                    &snapshot.design,
                    &accepted.document,
                    axis.curve,
                )?)
            {
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
            if accepted.document != snapshot.design
                && sources.iter().copied().try_fold(false, |changed, source| {
                    Ok::<_, DocumentError>(
                        changed
                            || !point_defined_curve_is_unchanged(
                                &snapshot.design,
                                &accepted.document,
                                source,
                            )?,
                    )
                })?
            {
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
    let planned_application = plan.apply(&mut scratch)?;
    let expected = planned_application.application;
    let output_plan = planned_application.output_plan;
    scratch.validate()?;
    Ok(SketchOperationResult::Proposed(Box::new(
        SketchOperationProposal {
            input: snapshot.input,
            accepted,
            request,
            source_free_role,
            plan,
            expected,
            output_plan,
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
    ) -> Result<PlannedOperationApplication, DocumentError> {
        let before = document_elements(document);
        let (kind, mut explicit, outputs) = match self {
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
                (*kind, vec![change], Vec::new())
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
                    Vec::new(),
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
                    mirror_outputs(&ids),
                )
            }
            Self::Chamfer(plan) => {
                let (changes, outputs) = apply_chamfer(document, plan)?;
                (SketchOperationKind::Chamfer, changes, outputs)
            }
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
                    fillet_outputs(&ids),
                )
            }
            Self::Rectangle {
                label,
                origin,
                width,
                height,
                role,
            } => {
                let ids =
                    document.add_rectangle_with_role(label, *origin, *width, *height, *role)?;
                (
                    SketchOperationKind::Rectangle,
                    Vec::new(),
                    rectangle_outputs(&ids),
                )
            }
            Self::Polygon {
                label,
                points,
                role,
            } => {
                let ids = apply_polygon(document, label, points, *role)?;
                (
                    SketchOperationKind::RegularPolygon,
                    Vec::new(),
                    polygon_outputs(&ids),
                )
            }
            Self::Slot(plan) => {
                let ids = apply_slot(document, plan)?;
                (
                    SketchOperationKind::Slot,
                    Vec::new(),
                    slot_outputs(document, &ids),
                )
            }
            Self::Pattern {
                label,
                sources,
                instances,
                step,
            } => {
                let mut copies = Vec::new();
                for instance_index in 1..*instances {
                    let instance = small_index(instance_index);
                    let offset = [step[0] * instance, step[1] * instance];
                    for (source_ordinal, source) in sources.iter().copied().enumerate() {
                        let (source_controls, controls, curve) = copy_point_defined_curve(
                            document,
                            &format!("{label}.instance_{instance}.source_{}", source_ordinal + 1),
                            source,
                            offset,
                        )?;
                        copies.push(PatternCopyOutputIds {
                            instance: instance_index,
                            source,
                            source_controls,
                            controls,
                            curve,
                        });
                    }
                }
                (
                    SketchOperationKind::LinearPattern,
                    sources
                        .iter()
                        .map(|source| SketchOperationIdentityChange::Retained((*source).into()))
                        .collect(),
                    pattern_outputs(&copies),
                )
            }
            Self::ProfileOffset { prepared, sources } => {
                let ids = document.create_prepared_profile_offset_geometry(prepared.clone())?;
                (
                    SketchOperationKind::ProfileOffset,
                    sources
                        .iter()
                        .map(|source| SketchOperationIdentityChange::Retained(source.curve.into()))
                        .collect(),
                    profile_offset_outputs(document, ids)?,
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
        let application = SketchOperationApplication {
            kind,
            identity_changes: explicit,
        };
        let output_plan = operation_output_plan(document, &application, outputs)?;
        Ok(PlannedOperationApplication {
            application,
            output_plan,
        })
    }
}

const fn field(name: &'static str) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::Field(name)
}

const fn index(value: usize) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::Index(value)
}

const fn source_control(
    curve: CurveId,
    point: DesignPointId,
    ordinal: usize,
) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::SourceControl {
        curve,
        point,
        ordinal,
    }
}

const fn source_curve(value: CurveId) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::SourceCurve(value)
}

const fn source_span(value: CurveSpan) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::SourceSpan(value)
}

const fn source_junction(
    value: DocumentProfileOffsetJunctionOwner,
) -> SketchOperationOutputPathSegment {
    SketchOperationOutputPathSegment::SourceJunction(value)
}

fn semantic_output(
    element: impl Into<DocumentElementId>,
    role: SketchOperationOutputRole,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    PlannedOperationOutput {
        element: element.into(),
        path,
        role,
    }
}

fn geometry_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::Geometry, path)
}

fn contact_parameter_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::ContactParameter, path)
}

fn contact_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::Contact, path)
}

fn constraint_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::Constraint, path)
}

fn dimension_target_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::DimensionTarget, path)
}

fn dimension_output(
    element: impl Into<DocumentElementId>,
    path: Vec<SketchOperationOutputPathSegment>,
) -> PlannedOperationOutput {
    semantic_output(element, SketchOperationOutputRole::Dimension, path)
}

fn nested_path(
    prefix: &[SketchOperationOutputPathSegment],
    suffix: impl IntoIterator<Item = SketchOperationOutputPathSegment>,
) -> Vec<SketchOperationOutputPathSegment> {
    prefix.iter().copied().chain(suffix).collect()
}

fn mirror_outputs(ids: &MirroredCurveIds) -> Vec<PlannedOperationOutput> {
    ids.point_pairs
        .iter()
        .enumerate()
        .map(|(ordinal, (source, mirrored))| {
            geometry_output(
                *mirrored,
                vec![
                    field("controls"),
                    source_control(ids.source_curve, *source, ordinal),
                ],
            )
        })
        .chain(std::iter::once(geometry_output(
            ids.mirrored_curve,
            vec![field("curves"), source_curve(ids.source_curve)],
        )))
        .chain(
            ids.symmetry_constraints
                .iter()
                .zip(ids.point_pairs.iter())
                .enumerate()
                .map(|(ordinal, (constraint, (source, _)))| {
                    constraint_output(
                        *constraint,
                        vec![
                            field("symmetryConstraints"),
                            source_control(ids.source_curve, *source, ordinal),
                        ],
                    )
                }),
        )
        .collect()
}

fn fillet_outputs(ids: &geosolve_sketch::CurveCurveFilletIds) -> Vec<PlannedOperationOutput> {
    vec![
        geometry_output(ids.center, vec![field("center")]),
        geometry_output(ids.radius, vec![field("radius")]),
        geometry_output(ids.start_angle, vec![field("startAngle")]),
        geometry_output(ids.end_angle, vec![field("endAngle")]),
        geometry_output(ids.arc, vec![field("arc")]),
        contact_parameter_output(
            ids.contact_parameters[0],
            vec![field("parents"), field("first"), field("parameter")],
        ),
        contact_output(
            ids.contacts[0],
            vec![field("parents"), field("first"), field("contact")],
        ),
        contact_parameter_output(
            ids.contact_parameters[1],
            vec![field("parents"), field("second"), field("parameter")],
        ),
        contact_output(
            ids.contacts[1],
            vec![field("parents"), field("second"), field("contact")],
        ),
        constraint_output(ids.constraint, vec![field("association")]),
        dimension_target_output(
            ids.radius_target,
            vec![field("radiusDimension"), field("target")],
        ),
        dimension_output(
            ids.radius_dimension,
            vec![field("radiusDimension"), field("dimension")],
        ),
    ]
}

fn rectangle_outputs(ids: &RectangleIds) -> Vec<PlannedOperationOutput> {
    let mut outputs = ["bottomLeft", "bottomRight", "topRight", "topLeft"]
        .into_iter()
        .zip(ids.points)
        .map(|(name, point)| geometry_output(point, vec![field("corners"), field(name)]))
        .collect::<Vec<_>>();
    outputs.extend(
        ["bottom", "right", "top", "left"]
            .into_iter()
            .zip(ids.curves)
            .map(|(name, curve)| geometry_output(curve, vec![field("edges"), field(name)])),
    );
    outputs.extend(
        [
            "anchor",
            "bottomHorizontal",
            "rightVertical",
            "topHorizontal",
            "leftVertical",
        ]
        .into_iter()
        .zip(ids.constraints)
        .map(|(name, constraint)| {
            constraint_output(constraint, vec![field("constraints"), field(name)])
        }),
    );
    for (name, target, dimension) in [
        ("width", ids.targets[0], ids.dimensions[0]),
        ("height", ids.targets[1], ids.dimensions[1]),
    ] {
        outputs.extend([
            dimension_target_output(
                target,
                vec![field("dimensions"), field(name), field("target")],
            ),
            dimension_output(
                dimension,
                vec![field("dimensions"), field(name), field("dimension")],
            ),
        ]);
    }
    outputs
}

fn polygon_outputs(ids: &PolygonOutputIds) -> Vec<PlannedOperationOutput> {
    ids.points
        .iter()
        .enumerate()
        .map(|(ordinal, point)| geometry_output(*point, vec![field("vertices"), index(ordinal)]))
        .chain(
            ids.curves.iter().enumerate().map(|(ordinal, curve)| {
                geometry_output(*curve, vec![field("edges"), index(ordinal)])
            }),
        )
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    reason = "the arguments are the exact native identities produced by one closed Chamfer operation"
)]
fn chamfer_outputs(
    document: &SketchDocument,
    first_point: DesignPointId,
    second_point: DesignPointId,
    edge: CurveId,
    contacts: [geosolve_sketch::ContactId; 2],
    constraints: [geosolve_sketch::DocumentConstraintId; 2],
    targets: [geosolve_sketch::DesignScalarId; 2],
    dimensions: [geosolve_sketch::DocumentDimensionId; 2],
) -> Vec<PlannedOperationOutput> {
    let parameters = contacts.map(|contact| {
        document
            .contact(contact)
            .expect("new Chamfer contact must be present")
            .parameter
    });
    vec![
        geometry_output(first_point, vec![field("endpoints"), field("first")]),
        geometry_output(second_point, vec![field("endpoints"), field("second")]),
        geometry_output(edge, vec![field("edge")]),
        contact_parameter_output(
            parameters[0],
            vec![field("parents"), field("first"), field("parameter")],
        ),
        contact_output(
            contacts[0],
            vec![field("parents"), field("first"), field("contact")],
        ),
        contact_parameter_output(
            parameters[1],
            vec![field("parents"), field("second"), field("parameter")],
        ),
        contact_output(
            contacts[1],
            vec![field("parents"), field("second"), field("contact")],
        ),
        constraint_output(
            constraints[0],
            vec![field("parents"), field("first"), field("constraint")],
        ),
        constraint_output(
            constraints[1],
            vec![field("parents"), field("second"), field("constraint")],
        ),
        dimension_target_output(
            targets[0],
            vec![field("distances"), field("first"), field("target")],
        ),
        dimension_target_output(
            targets[1],
            vec![field("distances"), field("second"), field("target")],
        ),
        dimension_output(
            dimensions[0],
            vec![field("distances"), field("first"), field("dimension")],
        ),
        dimension_output(
            dimensions[1],
            vec![field("distances"), field("second"), field("dimension")],
        ),
    ]
}

fn slot_outputs(document: &SketchDocument, ids: &SlotOutputIds) -> Vec<PlannedOperationOutput> {
    let mut outputs = ["first", "second"]
        .into_iter()
        .zip(ids.centers)
        .map(|(name, point)| geometry_output(point, vec![field("centers"), field(name)]))
        .collect::<Vec<_>>();
    outputs.extend(
        ["topFirst", "topSecond", "bottomSecond", "bottomFirst"]
            .into_iter()
            .zip(ids.boundary_points)
            .map(|(name, point)| {
                geometry_output(point, vec![field("boundaryPoints"), field(name)])
            }),
    );
    outputs.extend(
        ["top", "bottom"]
            .into_iter()
            .zip(ids.edges)
            .map(|(name, curve)| geometry_output(curve, vec![field("edges"), field(name)])),
    );
    for (name, arc) in ["right", "left"].into_iter().zip(ids.arcs) {
        let prefix = [field("arcs"), field(name)];
        outputs.extend([
            geometry_output(arc.radius, nested_path(&prefix, [field("radius")])),
            geometry_output(arc.start_angle, nested_path(&prefix, [field("startAngle")])),
            geometry_output(arc.end_angle, nested_path(&prefix, [field("endAngle")])),
            geometry_output(arc.curve, nested_path(&prefix, [field("curve")])),
        ]);
    }
    let join_names = ["topRight", "bottomRight", "bottomLeft", "topLeft"];
    for (name, (contact, constraint)) in join_names.into_iter().zip(ids.joins.iter().copied()) {
        let prefix = [field("joins"), field(name)];
        let parameter = document
            .contact(contact)
            .expect("new Slot contact must be present")
            .parameter;
        outputs.extend([
            contact_parameter_output(parameter, nested_path(&prefix, [field("parameter")])),
            contact_output(contact, nested_path(&prefix, [field("contact")])),
            constraint_output(constraint, nested_path(&prefix, [field("constraint")])),
        ]);
    }
    let fixed_names = [
        "firstCenter",
        "secondCenter",
        "topFirst",
        "topSecond",
        "bottomSecond",
        "bottomFirst",
    ];
    outputs.extend(fixed_names.into_iter().zip(ids.fixed.iter().copied()).map(
        |(name, constraint)| {
            constraint_output(constraint, vec![field("fixedConstraints"), field(name)])
        },
    ));
    outputs
}

fn pattern_outputs(copies: &[PatternCopyOutputIds]) -> Vec<PlannedOperationOutput> {
    let mut outputs = Vec::new();
    for copy in copies {
        let prefix = [
            field("instances"),
            index(copy.instance),
            field("sources"),
            source_curve(copy.source),
        ];
        outputs.extend(
            copy.controls
                .iter()
                .zip(&copy.source_controls)
                .enumerate()
                .map(|(ordinal, (point, source))| {
                    geometry_output(
                        *point,
                        nested_path(
                            &prefix,
                            [
                                field("controls"),
                                source_control(copy.source, *source, ordinal),
                            ],
                        ),
                    )
                }),
        );
        outputs.push(geometry_output(
            copy.curve,
            nested_path(&prefix, [field("curve")]),
        ));
    }
    outputs
}

fn profile_offset_outputs(
    document: &SketchDocument,
    ids: geosolve_sketch::DocumentProfileOffsetIds,
) -> Result<Vec<PlannedOperationOutput>, DocumentError> {
    let dimension = document
        .dimension(ids.dimension)
        .ok_or_else(|| operation_replay_error("new Profile Offset dimension is missing"))?;
    let DocumentDimensionDefinition::ProfileOffset { target, operand } = &dimension.definition
    else {
        return Err(operation_replay_error(
            "new Profile Offset dimension has the wrong definition",
        ));
    };
    if *target != ids.target {
        return Err(operation_replay_error(
            "new Profile Offset dimension changed its distance target",
        ));
    }
    let mut outputs = Vec::new();
    match operand {
        DocumentProfileOffsetOperand::Face { outer, holes, .. } => {
            profile_offset_path_outputs(
                document,
                &[field("operand"), field("outer")],
                &outer.edges,
                &outer.junctions,
                true,
                &mut outputs,
            )?;
            for hole in holes {
                let Some(first) = hole.edges.first() else {
                    return Err(operation_replay_error(
                        "Profile Offset generated an empty face hole",
                    ));
                };
                profile_offset_path_outputs(
                    document,
                    &[
                        field("operand"),
                        field("holes"),
                        source_span(first.source.curve),
                    ],
                    &hole.edges,
                    &hole.junctions,
                    true,
                    &mut outputs,
                )?;
            }
        }
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => {
            profile_offset_path_outputs(
                document,
                &[field("operand"), field("chain")],
                &chain.edges,
                &chain.junctions,
                false,
                &mut outputs,
            )?;
        }
    }
    outputs.extend([
        dimension_target_output(ids.target, vec![field("distance"), field("target")]),
        dimension_output(ids.dimension, vec![field("distance"), field("dimension")]),
    ]);
    Ok(outputs)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive native-output dispatcher keeps Profile Offset source ownership auditable"
)]
fn profile_offset_path_outputs(
    document: &SketchDocument,
    prefix: &[SketchOperationOutputPathSegment],
    edges: &[DocumentProfileOffsetEdgePair],
    junctions: &[DocumentProfileOffsetJunction],
    closed: bool,
    outputs: &mut Vec<PlannedOperationOutput>,
) -> Result<(), DocumentError> {
    let mut boundary_points = BTreeMap::new();
    for (ordinal, edge) in edges.iter().enumerate() {
        if let Some((start, end)) = directed_line_points(document, edge.target)? {
            let end_ordinal = if closed {
                (ordinal + 1) % edges.len()
            } else {
                ordinal + 1
            };
            for (point, boundary) in [(start, ordinal), (end, end_ordinal)] {
                let boundary = profile_offset_boundary_segment(edges, junctions, closed, boundary)?;
                if boundary_points
                    .insert(point, boundary)
                    .is_some_and(|prior| prior != boundary)
                {
                    return Err(operation_replay_error(
                        "Profile Offset repeats one boundary point at different semantic paths",
                    ));
                }
            }
        }
    }
    outputs.extend(boundary_points.into_iter().map(|(point, boundary)| {
        geometry_output(point, nested_path(prefix, [field("boundaries"), boundary]))
    }));

    for edge in edges {
        let edge_prefix = nested_path(prefix, [field("edges"), source_span(edge.source.curve)]);
        let target = document
            .curve(edge.target.curve.curve)
            .ok_or_else(|| operation_replay_error("Profile Offset target curve is missing"))?;
        match &target.definition {
            CurveDefinition::Line { .. } => {}
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => outputs.extend([
                geometry_output(*center, nested_path(&edge_prefix, [field("center")])),
                geometry_output(*radius, nested_path(&edge_prefix, [field("radius")])),
                geometry_output(
                    *start_angle,
                    nested_path(&edge_prefix, [field("startAngle")]),
                ),
                geometry_output(*end_angle, nested_path(&edge_prefix, [field("endAngle")])),
            ]),
            CurveDefinition::Circle { center, radius } => outputs.extend([
                geometry_output(*center, nested_path(&edge_prefix, [field("center")])),
                geometry_output(*radius, nested_path(&edge_prefix, [field("radius")])),
            ]),
            _ => {
                return Err(operation_replay_error(
                    "Profile Offset generated an unsupported target curve family",
                ));
            }
        }
        outputs.push(geometry_output(
            edge.target.curve.curve,
            nested_path(&edge_prefix, [field("curve")]),
        ));
    }
    for junction in junctions {
        let DocumentProfileOffsetJunctionOwner::Constraint(constraint) = junction.target_owner
        else {
            continue;
        };
        let definition = &document
            .constraint(constraint)
            .ok_or_else(|| operation_replay_error("Profile Offset junction owner is missing"))?
            .definition;
        let DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        } = definition
        else {
            return Err(operation_replay_error(
                "Profile Offset junction owner has the wrong definition",
            ));
        };
        let junction_prefix = nested_path(
            prefix,
            [field("junctions"), source_junction(junction.source_owner)],
        );
        for (name, contact) in [("incoming", *first_contact), ("outgoing", *second_contact)] {
            let parameter = document
                .contact(contact)
                .ok_or_else(|| {
                    operation_replay_error("Profile Offset junction contact is missing")
                })?
                .parameter;
            outputs.extend([
                contact_parameter_output(
                    parameter,
                    nested_path(&junction_prefix, [field(name), field("parameter")]),
                ),
                contact_output(
                    contact,
                    nested_path(&junction_prefix, [field(name), field("contact")]),
                ),
            ]);
        }
        outputs.push(constraint_output(
            constraint,
            nested_path(&junction_prefix, [field("constraint")]),
        ));
    }
    Ok(())
}

fn profile_offset_boundary_segment(
    edges: &[DocumentProfileOffsetEdgePair],
    junctions: &[DocumentProfileOffsetJunction],
    closed: bool,
    boundary: usize,
) -> Result<SketchOperationOutputPathSegment, DocumentError> {
    if closed {
        let junction = junctions
            .get((boundary + junctions.len() - 1) % junctions.len())
            .ok_or_else(|| operation_replay_error("Profile Offset closed junction is missing"))?;
        return Ok(source_junction(junction.source_owner));
    }
    if boundary == 0 {
        return Ok(field("start"));
    }
    if boundary == edges.len() {
        return Ok(field("end"));
    }
    let junction = junctions
        .get(boundary - 1)
        .ok_or_else(|| operation_replay_error("Profile Offset open junction is missing"))?;
    Ok(source_junction(junction.source_owner))
}

fn directed_line_points(
    document: &SketchDocument,
    curve: DocumentDirectedProfileOffsetCurve,
) -> Result<Option<(DesignPointId, DesignPointId)>, DocumentError> {
    let target = document
        .curve(curve.curve.curve)
        .ok_or_else(|| operation_replay_error("Profile Offset target curve is missing"))?;
    let CurveDefinition::Line { start, end, .. } = &target.definition else {
        return Ok(None);
    };
    Ok(Some(match curve.traversal {
        DocumentOffsetTraversal::Forward => (*start, *end),
        DocumentOffsetTraversal::Reverse => (*end, *start),
    }))
}

fn apply_polygon(
    document: &mut SketchDocument,
    label: &str,
    positions: &[[f64; 2]],
    role: GeometryRole,
) -> Result<PolygonOutputIds, DocumentError> {
    let points = positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            document.add_point(format!("{label}.point_{}", index + 1), *position)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut curves = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        curves.push(document.add_curve_with_role(
            format!("{label}.edge_{}", index + 1),
            CurveDefinition::Line {
                start: points[index],
                end: points[next],
                branch_direction: direction(positions[index], positions[next])?,
            },
            role,
        )?);
    }
    Ok(PolygonOutputIds { points, curves })
}

#[allow(clippy::too_many_lines)]
fn apply_chamfer(
    document: &mut SketchDocument,
    plan: &ChamferPlan,
) -> Result<
    (
        Vec<SketchOperationIdentityChange>,
        Vec<PlannedOperationOutput>,
    ),
    DocumentError,
> {
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
    let first_dimension = document.add_dimension(
        format!("{}.first_distance_dimension", plan.label),
        DocumentDimensionDefinition::PointDistance {
            first: plan.corner,
            second: first_point,
            target: first_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    let second_dimension = document.add_dimension(
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
    Ok((
        vec![
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
        ],
        chamfer_outputs(
            document,
            first_point,
            second_point,
            chamfer,
            [first_contact, second_contact],
            [first_owner, second_owner],
            [first_target, second_target],
            [first_dimension, second_dimension],
        ),
    ))
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

#[allow(
    clippy::too_many_lines,
    reason = "one atomic Slot expansion keeps its exact output identity allocation order auditable"
)]
fn apply_slot(
    document: &mut SketchDocument,
    plan: &SlotPlan,
) -> Result<SlotOutputIds, DocumentError> {
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
    let top = document.add_curve_with_role(
        format!("{}.top", plan.label),
        CurveDefinition::Line {
            start: boundary_points[0],
            end: boundary_points[1],
            branch_direction: axis,
        },
        plan.role,
    )?;
    let bottom = document.add_curve_with_role(
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
    let mut joins = Vec::with_capacity(4);
    for (index, (point, curve, parameter, neighborhood)) in [
        (
            boundary_points[1],
            right.curve,
            0.0,
            ContactNeighborhood::Start,
        ),
        (
            boundary_points[2],
            right.curve,
            1.0,
            ContactNeighborhood::End,
        ),
        (
            boundary_points[3],
            left.curve,
            0.0,
            ContactNeighborhood::Start,
        ),
        (
            boundary_points[0],
            left.curve,
            1.0,
            ContactNeighborhood::End,
        ),
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
        let constraint = document.add_constraint(
            format!("{}.join_{}", plan.label, index + 1),
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )?;
        joins.push((contact, constraint));
    }
    let mut fixed = Vec::with_capacity(6);
    for (index, point) in centers.into_iter().chain(boundary_points).enumerate() {
        let target = document.point(point).expect("new slot point").position;
        fixed.push(document.add_constraint(
            format!("{}.fixed_{}", plan.label, index + 1),
            DocumentConstraintDefinition::FixedPoint { point, target },
        )?);
    }
    Ok(SlotOutputIds {
        centers,
        boundary_points,
        edges: [top, bottom],
        arcs: [right, left],
        joins,
        fixed,
    })
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
) -> Result<SlotArcOutputIds, DocumentError> {
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
    let curve = document.add_curve_with_role(
        format!("{label}.arc"),
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        },
        role,
    )?;
    Ok(SlotArcOutputIds {
        radius,
        start_angle,
        end_angle,
        curve,
    })
}

fn copy_point_defined_curve(
    document: &mut SketchDocument,
    label: &str,
    source: CurveId,
    offset: [f64; 2],
) -> Result<(Vec<DesignPointId>, Vec<DesignPointId>, CurveId), DocumentError> {
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
    let curve = document.add_curve_with_role(format!("{label}.curve"), copied_definition, role)?;
    Ok((controls, copied, curve))
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

fn point_defined_curve_is_unchanged(
    design: &SketchDocument,
    accepted: &SketchDocument,
    curve: CurveId,
) -> Result<bool, DocumentError> {
    let design_curve = design.curve(curve).ok_or_else(|| unknown_curve(curve))?;
    let Some(accepted_curve) = accepted.curve(curve) else {
        return Ok(false);
    };
    if design_curve != accepted_curve
        || design.geometry_role(curve) != accepted.geometry_role(curve)
    {
        return Ok(false);
    }
    let controls = point_defined_controls(&design_curve.definition).ok_or_else(|| {
        DocumentError::InvalidField {
            field: "operation curve copy",
            message: "source family has no exact point-defined comparison".into(),
        }
    })?;
    Ok(controls.into_iter().all(|point| {
        design
            .point(point)
            .zip(accepted.point(point))
            .is_some_and(|(design, accepted)| design == accepted)
    }))
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

#[allow(
    clippy::too_many_lines,
    reason = "native identity pairing, semantic roles, paths, and curve-span cardinality are authenticated together"
)]
fn operation_output_plan(
    document: &SketchDocument,
    application: &SketchOperationApplication,
    outputs: Vec<PlannedOperationOutput>,
) -> Result<SketchOperationOutputPlan, DocumentError> {
    let mut generated = application
        .identity_changes
        .iter()
        .filter_map(|change| match change {
            SketchOperationIdentityChange::Proposed(element) => Some(*element),
            SketchOperationIdentityChange::Retained(_)
            | SketchOperationIdentityChange::Replaced(_)
            | SketchOperationIdentityChange::Split { .. } => None,
        })
        .collect::<Vec<_>>();
    generated.sort_unstable_by_key(|element| element.persistent_id());
    if generated.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(operation_replay_error(
            "operation reported one generated identity more than once",
        ));
    }

    let output_count = outputs.len();
    let output_by_element = outputs
        .into_iter()
        .map(|output| (output.element, output))
        .collect::<BTreeMap<_, _>>();
    if output_by_element.len() != output_count {
        return Err(operation_replay_error(
            "operation repeats one generated identity in its semantic output plan",
        ));
    }

    let mut slots = Vec::new();
    let mut index = 0;
    while index < generated.len() {
        let element = generated[index];
        let kind = match element {
            DocumentElementId::Point(_) => SketchMaterializationIdentityKind::Point,
            DocumentElementId::Scalar(_) => SketchMaterializationIdentityKind::Scalar,
            DocumentElementId::Curve(_) => SketchMaterializationIdentityKind::Curve,
            DocumentElementId::Contact(_) => SketchMaterializationIdentityKind::Contact,
            DocumentElementId::Constraint(_) => {
                require_generated_source_pair(&generated, index, "constraint")?;
                index += 1;
                SketchMaterializationIdentityKind::Constraint
            }
            DocumentElementId::Dimension(_) => {
                require_generated_source_pair(&generated, index, "dimension")?;
                index += 1;
                SketchMaterializationIdentityKind::Dimension
            }
            DocumentElementId::Parameter(_) => SketchMaterializationIdentityKind::Parameter,
            DocumentElementId::ExternalBinding(_) => {
                SketchMaterializationIdentityKind::ExternalBinding
            }
            DocumentElementId::Document(_) | DocumentElementId::Source(_) => {
                return Err(operation_replay_error(
                    "generated source identities must immediately follow their constraint or dimension owner",
                ));
            }
            _ => {
                return Err(operation_replay_error(
                    "operation generated an unsupported persistent output category",
                ));
            }
        };
        let output = output_by_element.get(&element).ok_or_else(|| {
            operation_replay_error("operation generated an identity without a semantic output role")
        })?;
        if !output_role_accepts_kind(output.role, kind) {
            return Err(operation_replay_error(
                "operation semantic output role disagrees with its native identity kind",
            ));
        }
        if output.path.is_empty() {
            return Err(operation_replay_error(
                "operation semantic output path must not be empty",
            ));
        }
        let curve_span_count = match element {
            DocumentElementId::Curve(curve) => u16::try_from(document.curve_spans(curve)?.len())
                .map_err(|_| operation_replay_error("operation curve span count exceeds limits"))?,
            _ => 0,
        };
        if (kind == SketchMaterializationIdentityKind::Curve) != (curve_span_count > 0) {
            return Err(operation_replay_error(
                "operation curve output has an invalid native span count",
            ));
        }
        slots.push(SketchOperationOutputSlot {
            ordinal: slots.len(),
            kind,
            curve_span_count,
            path: output.path.clone(),
            role: output.role,
        });
        index += 1;
    }
    if slots
        .iter()
        .enumerate()
        .any(|(index, slot)| slots[..index].iter().any(|prior| prior.path == slot.path))
    {
        return Err(operation_replay_error(
            "operation generated one semantic output path more than once",
        ));
    }
    if output_by_element.len() != slots.len() {
        return Err(operation_replay_error(
            "operation semantic outputs are missing or repeat one generated identity",
        ));
    }
    Ok(SketchOperationOutputPlan {
        kind: application.kind,
        slots,
    })
}

const fn output_role_accepts_kind(
    role: SketchOperationOutputRole,
    kind: SketchMaterializationIdentityKind,
) -> bool {
    match role {
        SketchOperationOutputRole::Geometry => matches!(
            kind,
            SketchMaterializationIdentityKind::Point
                | SketchMaterializationIdentityKind::Scalar
                | SketchMaterializationIdentityKind::Curve
        ),
        SketchOperationOutputRole::ContactParameter
        | SketchOperationOutputRole::DimensionTarget => {
            matches!(kind, SketchMaterializationIdentityKind::Scalar)
        }
        SketchOperationOutputRole::Contact => {
            matches!(kind, SketchMaterializationIdentityKind::Contact)
        }
        SketchOperationOutputRole::Constraint => {
            matches!(kind, SketchMaterializationIdentityKind::Constraint)
        }
        SketchOperationOutputRole::Dimension => {
            matches!(kind, SketchMaterializationIdentityKind::Dimension)
        }
        SketchOperationOutputRole::Parameter => {
            matches!(kind, SketchMaterializationIdentityKind::Parameter)
        }
        SketchOperationOutputRole::ExternalBinding => {
            matches!(kind, SketchMaterializationIdentityKind::ExternalBinding)
        }
    }
}

fn require_generated_source_pair(
    generated: &[DocumentElementId],
    owner_index: usize,
    owner_kind: &'static str,
) -> Result<(), DocumentError> {
    let owner = generated[owner_index].persistent_id().as_u128();
    let expected_source = owner.checked_add(1).ok_or(DocumentError::IdExhausted)?;
    let Some(DocumentElementId::Source(source)) = generated.get(owner_index + 1).copied() else {
        return Err(operation_replay_error(format!(
            "generated {owner_kind} is missing its immediately following audit source"
        )));
    };
    if source.0.as_u128() != expected_source {
        return Err(operation_replay_error(format!(
            "generated {owner_kind} audit source is not owner-adjacent"
        )));
    }
    Ok(())
}

fn validate_reserved_application(
    actual: &PlannedOperationApplication,
    expected: &SketchOperationApplication,
    output_plan: &SketchOperationOutputPlan,
) -> Result<(), DocumentError> {
    let actual_retained = actual
        .application
        .identity_changes
        .iter()
        .filter(|change| !matches!(change, SketchOperationIdentityChange::Proposed(_)))
        .cloned()
        .collect::<Vec<_>>();
    let expected_retained = expected
        .identity_changes
        .iter()
        .filter(|change| !matches!(change, SketchOperationIdentityChange::Proposed(_)))
        .cloned()
        .collect::<Vec<_>>();
    if actual.application.kind != expected.kind || actual_retained != expected_retained {
        return Err(operation_replay_error(
            "reserved replay changed the authenticated retained/replaced identity disposition",
        ));
    }
    if actual.output_plan != *output_plan {
        return Err(operation_replay_error(
            "reserved replay changed the authenticated generated-output semantic plan",
        ));
    }
    Ok(())
}

fn operation_replay_error(message: impl Into<String>) -> DocumentError {
    DocumentError::InvalidField {
        field: "operation proposal replay",
        message: message.into(),
    }
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
