use std::collections::BTreeMap;

use geosolve_core::{HardValidity, SolveTermination, SolverConfig};
use geosolve_geometry::Point2;
use thiserror::Error;

use crate::document::{
    ContactDefinition, ContactId, ContactStateEdit, CurveCurveFilletIds, CurveCurveFilletRequest,
    CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId, DocumentAngleOrientation,
    DocumentArcSweep, DocumentBSplineInsertion, DocumentBSplineSpanDirection,
    DocumentCircleTangencyMode, DocumentConstraintDefinition, DocumentConstraintId,
    DocumentCurveNormalSide, DocumentDimensionDefinition, DocumentDimensionId,
    DocumentDimensionMode, DocumentError, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentHyperbolaBranch, DocumentMirroredBSplineInsertion, DocumentNurbsInsertion,
    DocumentObjectId, DocumentSourceId, LineLineFilletIds, LineLineFilletRequest, MirroredCurveIds,
    PersistentId, RectangleIds, ScalarDomain, ScalarUnit, SketchDocument,
};
use crate::document_lowering::{DocumentRuntimeMap, RuntimeSource};
use crate::{
    SketchSession, SketchSessionError, SketchSolveRequest, SketchSolveResult, SketchSource,
    SolveRejection,
};

/// Persistent drag request lowered only after runtime IDs have been allocated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentDragTarget {
    pub point: DesignPointId,
    pub target: [f64; 2],
}

/// Per-solve interaction preferences expressed only in persistent IDs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentSolveRequest {
    pub drag: Option<DocumentDragTarget>,
    pub stability_target: Option<DocumentDragTarget>,
    pub previous_state_preferences: bool,
}

impl DocumentSolveRequest {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            drag: None,
            stability_target: None,
            previous_state_preferences: true,
        }
    }

    #[must_use]
    pub const fn without_previous_state_preferences(mut self) -> Self {
        self.previous_state_preferences = false;
        self
    }

    #[must_use]
    pub const fn with_drag(mut self, point: DesignPointId, target: [f64; 2]) -> Self {
        self.drag = Some(DocumentDragTarget { point, target });
        self
    }

    /// Adds a second compatible temporary target that keeps an unrelated point stable.
    #[must_use]
    pub const fn with_stability_target(mut self, point: DesignPointId, target: [f64; 2]) -> Self {
        self.stability_target = Some(DocumentDragTarget { point, target });
        self
    }
}

impl Default for DocumentSolveRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Separate attempted diagnostics and accepted state views for one document solve.
#[derive(Clone, Debug)]
pub struct DocumentSolveResult {
    attempted_solve: SketchSolveResult,
    accepted_view: SketchSolveResult,
    /// Runtime mappings for the accepted document state.
    mappings: DocumentRuntimeMap,
    /// Candidate mappings retained only for attempted diagnostics.
    attempted_mappings: DocumentRuntimeMap,
    attempted_sources: Vec<crate::SketchSourceMapping>,
    attempted_bound_mappings: Vec<crate::SketchBoundMapping>,
}

impl DocumentSolveResult {
    fn new(solve: SketchSolveResult, mappings: DocumentRuntimeMap) -> Self {
        Self {
            attempted_sources: solve.source_mappings.clone(),
            attempted_bound_mappings: solve.bound_mappings.clone(),
            attempted_mappings: mappings.clone(),
            accepted_view: solve.clone(),
            attempted_solve: solve,
            mappings,
        }
    }

    /// Returns the complete candidate attempt and its diagnostic mappings.
    #[must_use]
    pub const fn solve(&self) -> &SketchSolveResult {
        &self.attempted_solve
    }

    /// Returns the state/audit view retained by the document session.
    #[must_use]
    pub const fn accepted_view(&self) -> &SketchSolveResult {
        &self.accepted_view
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    /// Candidate remap used only to interpret an attempted solve's diagnostics.
    #[must_use]
    pub const fn attempted_mappings(&self) -> &DocumentRuntimeMap {
        &self.attempted_mappings
    }

    /// Candidate bound identities corresponding to `solve().core_report.bounds`.
    #[must_use]
    pub fn attempted_bound_mappings(&self) -> &[crate::SketchBoundMapping] {
        &self.attempted_bound_mappings
    }

    /// Returns one accepted reference-dimension measurement by persistent identity.
    #[must_use]
    pub fn accepted_reference_value(
        &self,
        document: &SketchDocument,
        dimension: DocumentDimensionId,
    ) -> Option<f64> {
        let source = document.dimension(dimension)?.source_id;
        let RuntimeSource::Dimension(runtime) = self.mappings.runtime_source(source)? else {
            return None;
        };
        self.accepted_view
            .reference_values
            .iter()
            .find_map(|value| (value.dimension_id == runtime).then_some(value.value))
    }

    #[must_use]
    pub fn accepted(&self) -> bool {
        self.attempted_solve.rejection.is_none()
    }

    /// Maps a runtime domain source from solver diagnostics back to persistent source identity.
    #[must_use]
    pub fn persistent_source(&self, source: SketchSource) -> Option<DocumentSourceId> {
        let runtime = match source {
            SketchSource::Constraint(id) => RuntimeSource::Constraint(id),
            SketchSource::Dimension(id) => RuntimeSource::Dimension(id),
            SketchSource::DragTarget(_) | SketchSource::PreviousState(_) => return None,
        };
        self.attempted_mappings
            .source_mappings()
            .iter()
            .find_map(|mapping| (mapping.runtime == Some(runtime)).then_some(mapping.source_id))
    }

    /// Maps a core source from a solve report back to persistent source identity.
    #[must_use]
    pub fn persistent_core_source(
        &self,
        source: geosolve_core::SourceConstraintId,
    ) -> Option<DocumentSourceId> {
        let runtime = self.attempted_sources.iter().find_map(|mapping| {
            (mapping.core_source_id == Some(source)).then_some(mapping.source)
        })?;
        self.persistent_source(runtime)
    }
}

/// One typed document edit. IDs for created objects are returned in [`DocumentCommandEffect`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentEdit {
    CreatePoint {
        label: String,
        position: [f64; 2],
    },
    CreateScalar {
        label: String,
        value: f64,
        unit: ScalarUnit,
        domain: ScalarDomain,
    },
    CreateCurve {
        label: String,
        definition: CurveDefinition,
    },
    CreateContact {
        label: String,
        definition: ContactDefinition,
    },
    CreateConstraint {
        label: String,
        definition: DocumentConstraintDefinition,
    },
    CreateDimension {
        label: String,
        definition: DocumentDimensionDefinition,
        mode: DocumentDimensionMode,
    },
    CreateRectangle {
        label: String,
        origin: [f64; 2],
        width: f64,
        height: f64,
    },
    CreateMirroredCurve {
        label: String,
        source_curve: CurveId,
        axis: CurveSpan,
    },
    CreateLineLineFillet {
        label: String,
        request: LineLineFilletRequest,
    },
    CreateCurveCurveFillet {
        label: String,
        request: CurveCurveFilletRequest,
    },
    SetPointPosition {
        point: DesignPointId,
        position: [f64; 2],
    },
    SetScalarValue {
        scalar: DesignScalarId,
        value: f64,
    },
    SetCurveBranch {
        curve: CurveSpan,
        direction: [f64; 2],
    },
    SetArcSweep {
        curve: CurveId,
        sweep: DocumentArcSweep,
    },
    SetLineLineFilletBranch {
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        second_side: DocumentCurveNormalSide,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    },
    SetCurveCurveFilletBranch {
        constraint: DocumentConstraintId,
        first_side: DocumentCurveNormalSide,
        first_trim_endpoint: DocumentFilletTrimEndpoint,
        second_side: DocumentCurveNormalSide,
        second_trim_endpoint: DocumentFilletTrimEndpoint,
        endpoint_order: DocumentFilletEndpointOrder,
        sweep: DocumentArcSweep,
    },
    SetConicWeightedMiddle {
        curve: CurveId,
        weighted_middle: [f64; 2],
    },
    SetHyperbolaBranch {
        curve: CurveId,
        branch: DocumentHyperbolaBranch,
    },
    InsertBSplineKnot {
        curve: CurveId,
        parameter: f64,
    },
    InsertMirroredBSplineKnot {
        label: String,
        source_curve: CurveId,
        mirrored_curve: CurveId,
        axis: CurveSpan,
        parameter: f64,
    },
    TransitionBSplineContact {
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    },
    InsertNurbsKnot {
        curve: CurveId,
        parameter: f64,
    },
    TransitionNurbsContact {
        contact: ContactId,
        direction: DocumentBSplineSpanDirection,
    },
    SetNurbsWeightGauge {
        curve: CurveId,
        gauge_weight: DesignScalarId,
    },
    SetContactStates {
        edits: Vec<ContactStateEdit>,
    },
    SetCircleTangencyBranch {
        constraint: DocumentConstraintId,
        mode: DocumentCircleTangencyMode,
        center_direction: [f64; 2],
    },
    SetDimensionMode {
        dimension: DocumentDimensionId,
        mode: DocumentDimensionMode,
    },
    SetOrientedAngleOrientation {
        dimension: DocumentDimensionId,
        orientation: DocumentAngleOrientation,
    },
    SetSourceSuppressed {
        source: DocumentSourceId,
        suppressed: bool,
    },
    Delete {
        object: DocumentObjectId,
    },
}

/// Revision-checked command input.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentCommand {
    pub expected_revision: u64,
    pub edit: DocumentEdit,
}

impl DocumentCommand {
    #[must_use]
    pub const fn new(expected_revision: u64, edit: DocumentEdit) -> Self {
        Self {
            expected_revision,
            edit,
        }
    }
}

/// Persistent identities affected by an accepted command.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DocumentCommandEffect {
    CreatedPoint(DesignPointId),
    CreatedScalar(DesignScalarId),
    CreatedCurve(crate::CurveId),
    CreatedContact(ContactId),
    CreatedConstraint(DocumentConstraintId),
    CreatedDimension(DocumentDimensionId),
    CreatedRectangle(Box<RectangleIds>),
    CreatedMirroredCurve(Box<MirroredCurveIds>),
    CreatedLineLineFillet(Box<LineLineFilletIds>),
    CreatedCurveCurveFillet(Box<CurveCurveFilletIds>),
    UpdatedPoint(DesignPointId),
    UpdatedScalar(DesignScalarId),
    UpdatedCurve(CurveId),
    UpdatedConicWeightedMiddle(CurveId),
    UpdatedHyperbolaBranch(CurveId),
    InsertedBSplineKnot(DocumentBSplineInsertion),
    InsertedMirroredBSplineKnot(Box<DocumentMirroredBSplineInsertion>),
    InsertedNurbsKnot(DocumentNurbsInsertion),
    UpdatedNurbsWeightGauge(CurveId),
    UpdatedContacts(Vec<ContactId>),
    UpdatedConstraint(DocumentConstraintId),
    UpdatedDimension(DocumentDimensionId),
    UpdatedSource(DocumentSourceId),
    Deleted(DocumentObjectId),
    Transaction(String),
    Imported,
    Undo,
    Redo,
}

/// Accepted IDs/value and command outcome from one atomic document transaction.
#[derive(Clone, Debug)]
pub struct DocumentTransactionOutcome<T> {
    pub value: Option<T>,
    pub outcome: DocumentCommandOutcome,
}

impl<T> DocumentTransactionOutcome<T> {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.value.is_some() && self.outcome.accepted()
    }
}

/// Accepted or rejected command attempt. Rejected attempts never mutate history.
#[derive(Clone, Debug)]
pub struct DocumentCommandOutcome {
    pub revision: u64,
    pub effect: Option<DocumentCommandEffect>,
    pub result: DocumentSolveResult,
}

impl DocumentCommandOutcome {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.effect.is_some() && self.result.accepted()
    }
}

/// Construction, command, history, or persistence failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DocumentSessionError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    SketchSession(#[from] SketchSessionError),
    #[error(transparent)]
    Sketch(#[from] crate::SketchError),
    #[error("stale document command: expected revision {expected}, accepted revision {actual}")]
    StaleCommand { expected: u64, actual: u64 },
    #[error("there is no accepted command to undo")]
    NothingToUndo,
    #[error("there is no accepted command to redo")]
    NothingToRedo,
    #[error("an accepted history snapshot unexpectedly failed to rebuild")]
    InvalidHistorySnapshot,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before: SketchDocument,
    after: SketchDocument,
    effect: DocumentCommandEffect,
}

/// Accepted persistent document plus solver session and accepted-only command history.
#[derive(Clone, Debug)]
pub struct SketchDocumentSession {
    document: SketchDocument,
    runtime: SketchSession,
    mappings: DocumentRuntimeMap,
    request: DocumentSolveRequest,
    config: SolverConfig,
    revision: u64,
    history: Vec<HistoryEntry>,
    history_cursor: usize,
    allocator_cursors: BTreeMap<crate::DocumentId, PersistentId>,
    span_allocator_cursors: BTreeMap<crate::DocumentId, BTreeMap<CurveId, u32>>,
}

impl SketchDocumentSession {
    /// Builds the first independently validated accepted document revision.
    ///
    /// # Errors
    ///
    /// Returns document/lowering/session errors or an initial solve rejection.
    pub fn new(
        document: SketchDocument,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentSessionError> {
        let lowered = document.lower()?;
        let (sketch, mappings) = lowered.into_parts();
        let runtime_request = lower_request(request, &mappings)?;
        let runtime = SketchSession::new(sketch, runtime_request, config)?;
        let mut document = document;
        document.project_accepted_state(runtime.sketch(), &mappings)?;
        let allocator_cursors = BTreeMap::from([(document.id(), document.allocator_cursor())]);
        let span_allocator_cursors =
            BTreeMap::from([(document.id(), document.spline_span_allocator_cursors())]);
        Ok(Self {
            document,
            runtime,
            mappings,
            request,
            config,
            revision: 0,
            history: Vec::new(),
            history_cursor: 0,
            allocator_cursors,
            span_allocator_cursors,
        })
    }

    #[must_use]
    pub const fn document(&self) -> &SketchDocument {
        &self.document
    }

    #[must_use]
    pub const fn runtime(&self) -> &SketchSession {
        &self.runtime
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    #[must_use]
    pub const fn request(&self) -> DocumentSolveRequest {
        self.request
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    #[must_use]
    pub const fn history_cursor(&self) -> usize {
        self.history_cursor
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history_cursor > 0
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history_cursor < self.history.len()
    }

    #[must_use]
    pub fn accepted_result(&self) -> DocumentSolveResult {
        DocumentSolveResult::new(
            self.runtime.accepted_result().clone(),
            self.mappings.clone(),
        )
    }

    /// Rebuilds a transient drag/request without adding a command-history entry.
    ///
    /// Rejected requests retain the prior accepted document, request, revision, and history.
    ///
    /// # Errors
    ///
    /// Returns a stale revision, persistent-ID mapping, lowering, or solver-start error.
    pub fn rebuild_request(
        &mut self,
        expected_revision: u64,
        request: DocumentSolveRequest,
    ) -> Result<DocumentSolveResult, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let attempt = attempt_document(&self.document, request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(result);
        };
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(result)
    }

    /// Applies one command by clone, solve, independent validation, and atomic swap.
    ///
    /// Numerical rejection is returned in the outcome and leaves accepted state/history unchanged.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, edit-validation, lowering, or solver-start error.
    pub fn apply(
        &mut self,
        command: DocumentCommand,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(command.expected_revision)?;
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let (effect, command_drag) = match command.edit {
            DocumentEdit::SetPointPosition { point, position } => {
                let mut target_candidate = candidate.clone();
                target_candidate.set_point_position(point, position)?;
                (
                    DocumentCommandEffect::UpdatedPoint(point),
                    Some(DocumentDragTarget {
                        point,
                        target: position,
                    }),
                )
            }
            edit => (apply_edit(&mut candidate, edit)?, None),
        };
        let attempt = attempt_document(&candidate, self.request, command_drag, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        };
        if let Some(drag) = command_drag
            && before
                .point(drag.point)
                .map(|point| point.position.map(f64::to_bits))
                == accepted_document
                    .point(drag.point)
                    .map(|point| point.position.map(f64::to_bits))
        {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        }
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(effect),
            result,
        })
    }

    /// Applies a compound document edit to one clone, solve, and history entry.
    ///
    /// The callback may use the public [`SketchDocument`] construction/editing API. Its
    /// returned value is published only when the complete candidate is independently accepted.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, invalid label/edit, lowering, or solver-start error.
    pub fn transact<T, F>(
        &mut self,
        expected_revision: u64,
        label: impl Into<String>,
        edit: F,
    ) -> Result<DocumentTransactionOutcome<T>, DocumentSessionError>
    where
        F: FnOnce(&mut SketchDocument) -> Result<T, DocumentError>,
    {
        self.check_revision(expected_revision)?;
        let label = label.into();
        if label.trim().is_empty() || label.len() > crate::MAX_LABEL_BYTES {
            return Err(DocumentError::InvalidField {
                field: "transaction label",
                message: format!("must contain 1..={} bytes", crate::MAX_LABEL_BYTES),
            }
            .into());
        }
        let before = self.document.clone();
        let mut candidate = before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let value = edit(&mut candidate)?;
        let attempt = attempt_document(&candidate, self.request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let effect = DocumentCommandEffect::Transaction(label);
        let Some((accepted_document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentTransactionOutcome {
                value: None,
                outcome: DocumentCommandOutcome {
                    revision: self.revision,
                    effect: None,
                    result,
                },
            });
        };
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: accepted_document.clone(),
            effect: effect.clone(),
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&accepted_document);
        self.commit(accepted_document, runtime, mappings);
        Ok(DocumentTransactionOutcome {
            value: Some(value),
            outcome: DocumentCommandOutcome {
                revision: self.revision,
                effect: Some(effect),
                result,
            },
        })
    }

    /// Restores the snapshot before the most recent accepted command.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, empty-history, or unexpected rebuild error.
    pub fn undo(
        &mut self,
        expected_revision: u64,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let index = self
            .history_cursor
            .checked_sub(1)
            .ok_or(DocumentSessionError::NothingToUndo)?;
        let mut candidate = self.history[index].before.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        self.history_cursor = index;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Undo),
            result: attempt.result,
        })
    }

    /// Reapplies the next accepted command snapshot.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, exhausted-redo, or unexpected rebuild error.
    pub fn redo(
        &mut self,
        expected_revision: u64,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let entry = self
            .history
            .get(self.history_cursor)
            .ok_or(DocumentSessionError::NothingToRedo)?;
        let mut candidate = entry.after.clone();
        self.advance_candidate_allocator(&mut candidate);
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let (document, runtime, mappings) = attempt
            .accepted
            .ok_or(DocumentSessionError::InvalidHistorySnapshot)?;
        self.history_cursor += 1;
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Redo),
            result: attempt.result,
        })
    }

    /// Imports a complete candidate atomically and records only an accepted import.
    ///
    /// # Errors
    ///
    /// Returns a stale-revision, JSON, validation, lowering, or solver-start error.
    pub fn import_json(
        &mut self,
        expected_revision: u64,
        json: &str,
    ) -> Result<DocumentCommandOutcome, DocumentSessionError> {
        self.check_revision(expected_revision)?;
        let mut candidate = SketchDocument::from_json(json)?;
        self.advance_candidate_allocator(&mut candidate);
        let before = self.document.clone();
        let request = DocumentSolveRequest {
            drag: None,
            ..self.request
        };
        let attempt = attempt_document(&candidate, request, None, self.config)?;
        let AttemptedDocument {
            accepted,
            mut result,
        } = attempt;
        let Some((document, runtime, mappings)) = accepted else {
            self.retain_accepted_view(&mut result);
            return Ok(DocumentCommandOutcome {
                revision: self.revision,
                effect: None,
                result,
            });
        };
        self.history.truncate(self.history_cursor);
        self.history.push(HistoryEntry {
            before,
            after: document.clone(),
            effect: DocumentCommandEffect::Imported,
        });
        self.history_cursor = self.history.len();
        self.record_allocator(&document);
        self.request = request;
        self.commit(document, runtime, mappings);
        Ok(DocumentCommandOutcome {
            revision: self.revision,
            effect: Some(DocumentCommandEffect::Imported),
            result,
        })
    }

    /// Exports the accepted document in canonical deterministic form.
    ///
    /// # Errors
    ///
    /// Returns a document validation or JSON serialization error.
    pub fn export_json(&self) -> Result<String, DocumentError> {
        self.document.to_canonical_json()
    }

    /// Returns the original effect at one accepted history position.
    #[must_use]
    pub fn history_effect(&self, index: usize) -> Option<&DocumentCommandEffect> {
        self.history.get(index).map(|entry| &entry.effect)
    }

    fn check_revision(&self, expected: u64) -> Result<(), DocumentSessionError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(DocumentSessionError::StaleCommand {
                expected,
                actual: self.revision,
            })
        }
    }

    fn commit(
        &mut self,
        document: SketchDocument,
        runtime: SketchSession,
        mappings: DocumentRuntimeMap,
    ) {
        self.document = document;
        self.runtime = runtime;
        self.mappings = mappings;
        self.revision = self.revision.saturating_add(1);
    }

    fn retain_accepted_view(&self, result: &mut DocumentSolveResult) {
        result
            .accepted_view
            .clone_from(self.runtime.accepted_result());
        result.mappings.clone_from(&self.mappings);
    }

    fn advance_candidate_allocator(&self, candidate: &mut SketchDocument) {
        if let Some(cursor) = self.allocator_cursors.get(&candidate.id()) {
            candidate.advance_allocator(*cursor);
        }
        if let Some(cursors) = self.span_allocator_cursors.get(&candidate.id()) {
            candidate.advance_spline_span_allocators(cursors);
        }
    }

    fn record_allocator(&mut self, document: &SketchDocument) {
        let cursor = document.allocator_cursor();
        self.allocator_cursors
            .entry(document.id())
            .and_modify(|retained| *retained = (*retained).max(cursor))
            .or_insert(cursor);
        let retained = self
            .span_allocator_cursors
            .entry(document.id())
            .or_default();
        for (curve, cursor) in document.spline_span_allocator_cursors() {
            retained
                .entry(curve)
                .and_modify(|value| *value = (*value).max(cursor))
                .or_insert(cursor);
        }
    }
}

struct AttemptedDocument {
    accepted: Option<(SketchDocument, SketchSession, DocumentRuntimeMap)>,
    result: DocumentSolveResult,
}

fn attempt_document(
    candidate: &SketchDocument,
    request: DocumentSolveRequest,
    command_drag: Option<DocumentDragTarget>,
    config: SolverConfig,
) -> Result<AttemptedDocument, DocumentSessionError> {
    let lowered = candidate.lower()?;
    let (mut sketch, mappings) = lowered.into_parts();
    let runtime_request = lower_request(request, &mappings)?;
    let attempted_request = lower_request(
        DocumentSolveRequest {
            drag: command_drag.or(request.drag),
            stability_target: request.stability_target,
            previous_state_preferences: command_drag.is_none()
                && request.previous_state_preferences,
        },
        &mappings,
    )?;
    let solve = sketch.solve(attempted_request, config)?;
    if solve.rejection.is_some() {
        return Ok(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        });
    }
    let runtime = SketchSession::new(sketch, runtime_request, config)?;
    let mut document = candidate.clone();
    if let Err(error) = document.project_accepted_state(runtime.sketch(), &mappings) {
        let mut solve = runtime.accepted_result().clone();
        solve.rejection = Some(SolveRejection::IndependentValidationFailed(
            error.to_string(),
        ));
        solve.acceptance_hard_residual_max = None;
        solve.core_report.hard_validity = HardValidity::Invalid;
        solve.core_report.termination = SolveTermination::Stalled;
        return Ok(AttemptedDocument {
            accepted: None,
            result: DocumentSolveResult::new(solve, mappings),
        });
    }
    let result = DocumentSolveResult::new(runtime.accepted_result().clone(), mappings.clone());
    Ok(AttemptedDocument {
        accepted: Some((document, runtime, mappings)),
        result,
    })
}

fn lower_request(
    request: DocumentSolveRequest,
    mappings: &DocumentRuntimeMap,
) -> Result<SketchSolveRequest, DocumentError> {
    let mut runtime = SketchSolveRequest::new();
    if !request.previous_state_preferences {
        runtime = runtime.without_previous_state_preferences();
    }
    if let Some(stability) = request.stability_target {
        let point = mappings
            .runtime_point(stability.point)
            .ok_or(DocumentError::UnknownId {
                kind: "stability target",
                id: stability.point.0,
            })?;
        runtime = runtime
            .with_stability_target(point, Point2::new(stability.target[0], stability.target[1]));
    }
    if let Some(drag) = request.drag {
        let point = mappings
            .runtime_point(drag.point)
            .ok_or(DocumentError::UnknownId {
                kind: "drag point",
                id: drag.point.0,
            })?;
        runtime = runtime.with_drag(point, Point2::new(drag.target[0], drag.target[1]));
    }
    Ok(runtime)
}

#[allow(clippy::too_many_lines)]
fn apply_edit(
    document: &mut SketchDocument,
    edit: DocumentEdit,
) -> Result<DocumentCommandEffect, DocumentError> {
    let effect = match edit {
        DocumentEdit::CreatePoint { label, position } => {
            DocumentCommandEffect::CreatedPoint(document.add_point(label, position)?)
        }
        DocumentEdit::CreateScalar {
            label,
            value,
            unit,
            domain,
        } => DocumentCommandEffect::CreatedScalar(document.add_scalar(label, value, unit, domain)?),
        DocumentEdit::CreateCurve { label, definition } => {
            DocumentCommandEffect::CreatedCurve(document.add_curve(label, definition)?)
        }
        DocumentEdit::CreateContact { label, definition } => {
            DocumentCommandEffect::CreatedContact(document.add_contact(label, definition)?)
        }
        DocumentEdit::CreateConstraint { label, definition } => {
            DocumentCommandEffect::CreatedConstraint(document.add_constraint(label, definition)?)
        }
        DocumentEdit::CreateDimension {
            label,
            definition,
            mode,
        } => DocumentCommandEffect::CreatedDimension(
            document.add_dimension(label, definition, mode)?,
        ),
        DocumentEdit::CreateRectangle {
            label,
            origin,
            width,
            height,
        } => DocumentCommandEffect::CreatedRectangle(Box::new(
            document.add_rectangle(&label, origin, width, height)?,
        )),
        DocumentEdit::CreateMirroredCurve {
            label,
            source_curve,
            axis,
        } => DocumentCommandEffect::CreatedMirroredCurve(Box::new(document.add_mirrored_curve(
            &label,
            source_curve,
            axis,
        )?)),
        DocumentEdit::CreateLineLineFillet { label, request } => {
            DocumentCommandEffect::CreatedLineLineFillet(Box::new(
                document.add_line_line_fillet(&label, request)?,
            ))
        }
        DocumentEdit::CreateCurveCurveFillet { label, request } => {
            DocumentCommandEffect::CreatedCurveCurveFillet(Box::new(
                document.add_curve_curve_fillet(&label, request)?,
            ))
        }
        DocumentEdit::SetPointPosition { point, position } => {
            document.set_point_position(point, position)?;
            DocumentCommandEffect::UpdatedPoint(point)
        }
        DocumentEdit::SetScalarValue { scalar, value } => {
            document.set_scalar_value(scalar, value)?;
            DocumentCommandEffect::UpdatedScalar(scalar)
        }
        DocumentEdit::SetCurveBranch { curve, direction } => {
            document.set_curve_branch(curve, direction)?;
            DocumentCommandEffect::UpdatedCurve(curve.curve)
        }
        DocumentEdit::SetArcSweep { curve, sweep } => {
            document.set_arc_sweep(curve, sweep)?;
            DocumentCommandEffect::UpdatedCurve(curve)
        }
        DocumentEdit::SetLineLineFilletBranch {
            constraint,
            first_side,
            second_side,
            endpoint_order,
            sweep,
        } => {
            document.set_line_line_fillet_branch(
                constraint,
                first_side,
                second_side,
                endpoint_order,
                sweep,
            )?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetCurveCurveFilletBranch {
            constraint,
            first_side,
            first_trim_endpoint,
            second_side,
            second_trim_endpoint,
            endpoint_order,
            sweep,
        } => {
            document.set_curve_curve_fillet_branch(
                constraint,
                first_side,
                first_trim_endpoint,
                second_side,
                second_trim_endpoint,
                endpoint_order,
                sweep,
            )?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetConicWeightedMiddle {
            curve,
            weighted_middle,
        } => {
            document.set_conic_weighted_middle(curve, weighted_middle)?;
            DocumentCommandEffect::UpdatedConicWeightedMiddle(curve)
        }
        DocumentEdit::SetHyperbolaBranch { curve, branch } => {
            document.set_hyperbola_branch(curve, branch)?;
            DocumentCommandEffect::UpdatedHyperbolaBranch(curve)
        }
        DocumentEdit::InsertBSplineKnot { curve, parameter } => {
            DocumentCommandEffect::InsertedBSplineKnot(
                document.insert_bspline_knot(curve, parameter)?,
            )
        }
        DocumentEdit::InsertMirroredBSplineKnot {
            label,
            source_curve,
            mirrored_curve,
            axis,
            parameter,
        } => DocumentCommandEffect::InsertedMirroredBSplineKnot(Box::new(
            document.insert_mirrored_bspline_knot(
                &label,
                source_curve,
                mirrored_curve,
                axis,
                parameter,
            )?,
        )),
        DocumentEdit::TransitionBSplineContact { contact, direction } => {
            document.transition_bspline_contact(contact, direction)?;
            DocumentCommandEffect::UpdatedContacts(vec![contact])
        }
        DocumentEdit::InsertNurbsKnot { curve, parameter } => {
            DocumentCommandEffect::InsertedNurbsKnot(document.insert_nurbs_knot(curve, parameter)?)
        }
        DocumentEdit::TransitionNurbsContact { contact, direction } => {
            document.transition_nurbs_contact(contact, direction)?;
            DocumentCommandEffect::UpdatedContacts(vec![contact])
        }
        DocumentEdit::SetNurbsWeightGauge {
            curve,
            gauge_weight,
        } => {
            document.set_nurbs_weight_gauge(curve, gauge_weight)?;
            DocumentCommandEffect::UpdatedNurbsWeightGauge(curve)
        }
        DocumentEdit::SetContactStates { edits } => {
            let contacts = edits.iter().map(|edit| edit.contact).collect();
            document.set_contact_states(&edits)?;
            DocumentCommandEffect::UpdatedContacts(contacts)
        }
        DocumentEdit::SetCircleTangencyBranch {
            constraint,
            mode,
            center_direction,
        } => {
            document.set_circle_tangency_branch(constraint, mode, center_direction)?;
            DocumentCommandEffect::UpdatedConstraint(constraint)
        }
        DocumentEdit::SetDimensionMode { dimension, mode } => {
            document.set_dimension_mode(dimension, mode)?;
            DocumentCommandEffect::UpdatedDimension(dimension)
        }
        DocumentEdit::SetOrientedAngleOrientation {
            dimension,
            orientation,
        } => {
            document.set_oriented_angle_orientation(dimension, orientation)?;
            DocumentCommandEffect::UpdatedDimension(dimension)
        }
        DocumentEdit::SetSourceSuppressed { source, suppressed } => {
            document.set_source_suppressed(source, suppressed)?;
            DocumentCommandEffect::UpdatedSource(source)
        }
        DocumentEdit::Delete { object } => {
            document.remove_with_owned_state(object)?;
            DocumentCommandEffect::Deleted(object)
        }
    };
    Ok(effect)
}
