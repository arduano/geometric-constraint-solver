// SPDX-License-Identifier: GPL-3.0-or-later

//! Single-history coordinator for projectional design intent.
//!
//! Durable mutation and Undo/Redo live exclusively in [`IntentSession`]. Native
//! retained sessions in this module are either an independently cold-rebuilt
//! accepted authority or gesture-local numerical continuation state; their own
//! command histories are never exposed or persisted.

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use geosolve_sketch::{
    CurveDefinition, CurveId, DesignPointId, DesignScalarId, DocumentCurveControlAvailability,
    DocumentCurveControlId, DocumentCurveControlProjection, DocumentCurveControlTarget,
    DocumentDragLocalityPlan, DocumentEdit, DocumentRationalConicControl, DocumentSessionError,
    DocumentSolveRequest, OperationControl, OperationOutcome, PreparedSketchOperation,
    PreparedSketchPatch, RetainedSketchDocumentSession, ScalarUnit, SketchDesignIdentity,
    SketchDocument,
};
use geosolve_sketch_intent::{
    DeletePolicy, GeometryRecipeKind, IntentAliasMap, IntentFieldKey, IntentKey, IntentLiteral,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPlan, IntentPatchPolicy,
    IntentPlanDisposition, IntentPlanError, IntentSession, IntentSessionError, IntentSessionId,
    IntentSessionIdentity, IntentUnit, LeafRef, NodeId,
};
use thiserror::Error;

use crate::{
    AuditedInteraction, ColdIntentMaterialization, ColdIntentMaterializer,
    IntentMaterializationError, IntentNativeWritableLeaf, InteractionWorkReceipt,
};

#[cfg(target_arch = "wasm32")]
type AcceptedMaterialization = Rc<ColdIntentMaterialization>;
#[cfg(not(target_arch = "wasm32"))]
type AcceptedMaterialization = Box<ColdIntentMaterialization>;

#[cfg(target_arch = "wasm32")]
fn retain_accepted(accepted: &AcceptedMaterialization) -> AcceptedMaterialization {
    Rc::clone(accepted)
}

#[cfg(not(target_arch = "wasm32"))]
fn retain_accepted(accepted: &AcceptedMaterialization) -> AcceptedMaterialization {
    Box::new(accepted.as_ref().clone())
}

#[cfg(target_arch = "wasm32")]
fn own_accepted(accepted: ColdIntentMaterialization) -> AcceptedMaterialization {
    Rc::new(accepted)
}

#[cfg(not(target_arch = "wasm32"))]
fn own_accepted(accepted: ColdIntentMaterialization) -> AcceptedMaterialization {
    Box::new(accepted)
}

#[cfg(target_arch = "wasm32")]
fn own_boxed_accepted(accepted: Box<ColdIntentMaterialization>) -> AcceptedMaterialization {
    Rc::from(accepted)
}

#[cfg(not(target_arch = "wasm32"))]
fn own_boxed_accepted(accepted: Box<ColdIntentMaterialization>) -> AcceptedMaterialization {
    accepted
}

fn accepted_authority_is_current(
    intent: &IntentSession,
    accepted: &ColdIntentMaterialization,
) -> bool {
    let semantic = intent.semantic_identity();
    intent.accepted().is_some_and(|authority| {
        authority.target == semantic
            && accepted.validation.semantic == semantic
            && accepted.ownership.semantic == semantic
            && accepted.evidence == authority.evidence
            && accepted
                .session
                .accepted_state_for_current_input()
                .is_some()
    })
}

/// Result of one committed projectional transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionalPatchOutcome {
    pub identity: IntentSessionIdentity,
    pub disposition: IntentPlanDisposition,
    pub aliases: IntentAliasMap,
}

/// Latest independently accepted point-drag preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionalPointDragPreview {
    pub request_id: u64,
    pub point: DesignPointId,
    pub accepted_position: [f64; 2],
}

/// One linear, independently accepted native point terminal delegated to an
/// outer semantic owner.
///
/// The proposal proves the exact pointer/request/Intent route and retains the
/// accepted native preview, but it grants no Intent publication authority by
/// itself. Consuming it never scans writable leaves, plans an Intent patch, or
/// appends nested history.
#[derive(Debug)]
pub struct DelegatedPointDragProposal {
    /// Exact Intent authority against which the native route was prepared.
    pub intent: IntentSessionIdentity,
    /// Pointer which exclusively owns this terminal.
    pub pointer_id: u64,
    /// Latest accepted pointer-frame request.
    pub request_id: u64,
    /// Exact native point authenticated at pointer-down.
    pub point: DesignPointId,
    /// Accepted native position witnessed by `terminal_session`.
    pub accepted_position: [f64; 2],
    terminal_session: Box<RetainedSketchDocumentSession>,
}

impl DelegatedPointDragProposal {
    /// Exact independently accepted native preview witnessed at pointer-up.
    #[must_use]
    pub fn terminal_session(&self) -> &RetainedSketchDocumentSession {
        &self.terminal_session
    }
}

/// Latest independently accepted selected-curve control preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionalCurveControlPreview {
    pub request_id: u64,
    pub control: DocumentCurveControlId,
    pub accepted_position: [f64; 2],
}

#[derive(Clone, Debug)]
struct AcceptedPointDragSample {
    preview: ProjectionalPointDragPreview,
    session: Box<RetainedSketchDocumentSession>,
}

#[derive(Clone, Debug)]
struct ProjectionalPointDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    point: DesignPointId,
    locality: DocumentDragLocalityPlan,
    origin: Box<RetainedSketchDocumentSession>,
    origin_position: [f64; 2],
    latest_request_id: Option<u64>,
    latest: Option<AcceptedPointDragSample>,
}

#[derive(Debug)]
struct ValidatedPointDragTerminal {
    intent: IntentSessionIdentity,
    point: DesignPointId,
    origin: Box<RetainedSketchDocumentSession>,
    preview: ProjectionalPointDragPreview,
    session: Box<RetainedSketchDocumentSession>,
}

#[derive(Clone, Copy, Debug)]
enum ProjectionalCurveControlRoute {
    Point {
        point: DesignPointId,
        x: LeafRef,
        y: LeafRef,
        origin: [f64; 2],
    },
    Scalar {
        scalar: DesignScalarId,
        leaf: LeafRef,
        unit: IntentUnit,
        origin: f64,
    },
    RationalMiddle {
        curve: CurveId,
        node: NodeId,
        weight: LeafRef,
        origin_weighted_middle: [f64; 2],
        origin_weight: f64,
    },
}

#[derive(Debug)]
struct AcceptedCurveControlSample {
    preview: ProjectionalCurveControlPreview,
    patch: Box<PreparedSketchPatch>,
    operations: Vec<IntentPatchOperation>,
    changed: bool,
}

#[derive(Debug)]
struct ProjectionalCurveControlDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    expected: SketchDesignIdentity,
    accepted_revision: u64,
    control: DocumentCurveControlId,
    route: ProjectionalCurveControlRoute,
    origin: Box<RetainedSketchDocumentSession>,
    latest_request_id: Option<u64>,
    latest: Option<AcceptedCurveControlSample>,
}

/// Projectional authority over one intent session and its accepted native scene.
///
/// The coordinator deliberately does not wrap `RetainedEditorCoordinator`: that
/// type owns the accepted M81 flat-workspace history. Reusing that durable
/// history here would duplicate Undo/Redo authority. Ordinary pointer previews
/// instead use history-discarded clones of the native retained sketch session.
#[derive(Debug)]
pub struct ProjectionalIntentCoordinator {
    intent: IntentSession,
    materializer: ColdIntentMaterializer,
    accepted: Option<AcceptedMaterialization>,
    point_drag: Option<ProjectionalPointDrag>,
    curve_control_drag: Option<ProjectionalCurveControlDrag>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedProjectionalTransaction {
    plan: IntentPatchPlan,
    materialization: Box<ColdIntentMaterialization>,
}

impl PreparedProjectionalTransaction {
    #[must_use]
    pub(crate) fn materialization(&self) -> &ColdIntentMaterialization {
        &self.materialization
    }
}

#[derive(Debug)]
enum PlannedProjectionalTransaction {
    Accepted(PreparedProjectionalTransaction),
    NonPublishing { plan: IntentPatchPlan },
}

impl PlannedProjectionalTransaction {
    fn into_accepted(
        self,
    ) -> Result<PreparedProjectionalTransaction, ProjectionalCoordinatorError> {
        match self {
            Self::Accepted(prepared) => Ok(prepared),
            Self::NonPublishing { .. } => {
                Err(ProjectionalCoordinatorError::MissingAcceptedMaterialization)
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static PLAN_PATCH_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_plan_patch_calls() {
    PLAN_PATCH_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn plan_patch_calls() -> usize {
    PLAN_PATCH_CALLS.with(std::cell::Cell::get)
}

impl ProjectionalIntentCoordinator {
    /// Forks the exact accepted projectional authority without replaying or
    /// re-solving it.
    ///
    /// The returned coordinator owns a history-free clone of current Intent,
    /// the already independently validated materialization and no disposable
    /// pointer state. This is the transactional starting point used by
    /// adjacent composition layers that own outer Undo/Redo and must prepare
    /// one new cold-validated patch without first cold-restoring the unchanged
    /// accepted graph.
    pub(crate) fn fork_accepted(&self) -> Result<Self, ProjectionalCoordinatorError> {
        let accepted = self
            .accepted
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if !accepted_authority_is_current(&self.intent, accepted) {
            return Err(ProjectionalCoordinatorError::NoAcceptedAuthority);
        }
        Ok(Self {
            intent: self.intent.delegated_checkpoint()?,
            materializer: self.materializer.clone(),
            accepted: Some(retain_accepted(accepted)),
            point_drag: None,
            curve_control_drag: None,
        })
    }

    /// Consumes one accepted coordinator and removes its nested Intent
    /// Undo/Redo while retaining the exact current graph, allocator and
    /// independently validated native authority.
    pub(crate) fn into_delegated_accepted(self) -> Result<Self, ProjectionalCoordinatorError> {
        let Self {
            intent,
            materializer,
            accepted,
            point_drag: _,
            curve_control_drag: _,
        } = self;
        let accepted = accepted.ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if !accepted_authority_is_current(&intent, &accepted) {
            return Err(ProjectionalCoordinatorError::NoAcceptedAuthority);
        }
        Ok(Self {
            intent: intent.into_delegated_checkpoint()?,
            materializer,
            accepted: Some(accepted),
            point_drag: None,
            curve_control_drag: None,
        })
    }

    /// Creates an empty projectional session.
    ///
    /// # Errors
    ///
    /// Returns an intent-session construction error for an invalid built-in key.
    pub fn empty(
        id: IntentSessionId,
        materializer: ColdIntentMaterializer,
    ) -> Result<Self, ProjectionalCoordinatorError> {
        Ok(Self {
            intent: IntentSession::with_id(id)?,
            materializer,
            accepted: None,
            point_drag: None,
            curve_control_drag: None,
        })
    }

    /// Restores canonical intent and cold-reconstructs its accepted authority.
    /// Persisted native artifacts are used only as authenticated comparison
    /// evidence and are never installed directly.
    ///
    /// # Errors
    ///
    /// Returns a materialization error if accepted authority cannot be
    /// independently reproduced exactly.
    pub fn restore(
        intent: IntentSession,
        materializer: ColdIntentMaterializer,
    ) -> Result<Self, ProjectionalCoordinatorError> {
        let accepted = intent
            .accepted()
            .map(|authority| materializer.materialize_accepted_authority(authority))
            .transpose()?;
        Ok(Self {
            intent,
            materializer,
            accepted: accepted.map(own_accepted),
            point_drag: None,
            curve_control_drag: None,
        })
    }

    /// Restores one exact pristine semantic session with independently
    /// materialized empty native acceptance and no manufactured history.
    ///
    /// This narrow initialization seam is intentionally separate from
    /// ordinary no-op patches, which remain rejected by the intent planner.
    ///
    /// # Errors
    ///
    /// Returns a typed materialization, intent, or authority mismatch when
    /// the session is not pristine or empty acceptance cannot be independently
    /// reconstructed and authenticated.
    pub fn restore_pristine_empty(
        mut intent: IntentSession,
        materializer: ColdIntentMaterializer,
    ) -> Result<Self, ProjectionalCoordinatorError> {
        let accepted = materializer.materialize_pristine_empty(&intent)?;
        intent.install_pristine_empty_acceptance(accepted.evidence.clone())?;
        let authority = intent
            .accepted()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if authority.target != accepted.validation.semantic
            || authority.evidence != accepted.evidence
            || accepted.ownership.semantic != authority.target
        {
            return Err(ProjectionalCoordinatorError::BootstrapAuthorityMismatch);
        }
        Ok(Self {
            intent,
            materializer,
            accepted: Some(own_accepted(accepted)),
            point_drag: None,
            curve_control_drag: None,
        })
    }

    /// Installs a strictly authenticated history-free native bootstrap.
    ///
    /// This is the narrow migration seam for an already restored flat
    /// workspace. The caller must first decode every bootstrap declaration,
    /// compare its native document with `accepted`, and independently validate
    /// the accepted solve. Interactive patches still return through the cold
    /// materializer; the supplied native session is never a second history
    /// authority.
    pub(crate) fn restore_authenticated_bootstrap(
        intent: IntentSession,
        materializer: ColdIntentMaterializer,
        accepted: ColdIntentMaterialization,
    ) -> Result<Self, ProjectionalCoordinatorError> {
        let authority = intent
            .accepted()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if authority.target != accepted.validation.semantic
            || authority.evidence != accepted.evidence
            || accepted.ownership.semantic != authority.target
        {
            return Err(ProjectionalCoordinatorError::BootstrapAuthorityMismatch);
        }
        Ok(Self {
            intent,
            materializer,
            accepted: Some(own_accepted(accepted)),
            point_drag: None,
            curve_control_drag: None,
        })
    }

    #[must_use]
    pub const fn intent(&self) -> &IntentSession {
        &self.intent
    }

    /// Last independently accepted cold materialization. It remains available
    /// beneath a newer retained-failed intent transaction.
    #[must_use]
    pub fn accepted_materialization(&self) -> Option<&ColdIntentMaterialization> {
        self.accepted.as_deref()
    }

    /// Native scene currently suitable for presentation. A valid pointer
    /// preview temporarily outranks the durable accepted scene.
    #[must_use]
    pub fn presentation_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.curve_control_drag
            .as_ref()
            .and_then(|drag| drag.latest.as_ref())
            .and_then(|sample| sample.patch.preview().accepted_session())
            .or_else(|| {
                self.point_drag
                    .as_ref()
                    .and_then(|drag| drag.latest.as_ref())
                    .map(|sample| sample.session.as_ref())
            })
            .or_else(|| self.accepted.as_ref().map(|accepted| &accepted.session))
    }

    /// Plans, independently materializes, and commits one exact-CAS patch.
    /// Accepted native state is installed only after the intent plan commits;
    /// retained failures and organization-only transactions preserve it.
    ///
    /// # Errors
    ///
    /// Returns the exact planning, materialization, or session publication error.
    pub fn apply_patch(
        &mut self,
        patch: IntentPatch,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.cancel_interaction();
        let planned = self.plan_patch(patch)?;
        self.commit_planned(planned)
    }

    /// Applies one independently materialized exact-CAS patch without
    /// constructing nested Intent Undo/Redo.
    ///
    /// Adjacent composite owners use this only over a history-free accepted
    /// fork while publishing the sole user-visible history row themselves.
    /// Native materialization, computed-feature evaluation, validation and
    /// accepted-authority publication are otherwise identical to
    /// [`Self::apply_patch`].
    pub(crate) fn apply_delegated_patch(
        &mut self,
        patch: IntentPatch,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.cancel_interaction();
        let mut work = InteractionWorkReceipt::default();
        let planned = self.plan_delegated_patch_with_work(patch, &mut work)?;
        self.commit_delegated_planned_with_work(planned, &mut work)
    }

    /// Applies one delegated source patch while carrying an independently
    /// accepted numerical continuation into the candidate's exact native
    /// certification. The continuation supplies no Intent operation, history
    /// row, or temporary drag request.
    pub(crate) fn apply_delegated_patch_with_accepted_continuation(
        &mut self,
        patch: IntentPatch,
        accepted_continuation: &geosolve_sketch::SketchDocument,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.cancel_interaction();
        let mut work = InteractionWorkReceipt::default();
        let planned =
            self.plan_patch_with_work_mode(patch, &mut work, true, Some(accepted_continuation))?;
        self.commit_delegated_planned_with_work(planned, &mut work)
    }

    /// Plans and independently cold-materializes one typed patch without
    /// publishing its intent plan, accepted authority, or history entry.
    ///
    /// This is the projectional property/authoring-preview seam. A retained
    /// failure returns `None`; an accepted result retains the exact-CAS plan
    /// together with its independently validated native materialization. The
    /// same opaque transaction can therefore be committed after terminal
    /// authentication without planning or cold-solving the candidate again.
    ///
    /// # Errors
    ///
    /// Returns the ordinary exact-CAS planning or cold-materialization error.
    pub(crate) fn prepare_patch_transaction_audited(
        &self,
        patch: IntentPatch,
    ) -> AuditedInteraction<
        Result<Option<PreparedProjectionalTransaction>, ProjectionalCoordinatorError>,
    > {
        let mut work = InteractionWorkReceipt::default();
        let outcome = match self.plan_patch_with_work(patch, &mut work) {
            Ok(PlannedProjectionalTransaction::Accepted(prepared)) => Ok(Some(prepared)),
            Ok(PlannedProjectionalTransaction::NonPublishing { .. }) => Ok(None),
            Err(error) => Err(error),
        };
        AuditedInteraction::new(outcome, work)
    }

    /// Publishes an exact previously prepared accepted transaction.
    ///
    /// The plan's normal exact-CAS base authentication rejects stale or
    /// foreign previews before either intent or native accepted authority can
    /// change. The native result was already independently validated while the
    /// plan was prepared, so successful publication performs no second solve.
    pub(crate) fn commit_prepared_transaction(
        &mut self,
        prepared: PreparedProjectionalTransaction,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.commit_prepared_transaction_audited(prepared)
            .into_outcome()
    }

    pub(crate) fn commit_prepared_transaction_audited(
        &mut self,
        prepared: PreparedProjectionalTransaction,
    ) -> AuditedInteraction<Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError>> {
        self.cancel_interaction();
        let mut work = InteractionWorkReceipt::default();
        let outcome = self.commit_planned_with_work(
            PlannedProjectionalTransaction::Accepted(prepared),
            &mut work,
        );
        AuditedInteraction::new(outcome, work)
    }

    /// Deletes one declaration and its exact Rust-computed dependent closure
    /// as a single accepted transaction. Callers never enumerate generated
    /// native objects or guess dependency ownership.
    ///
    /// # Errors
    ///
    /// Returns the ordinary graph, materialization, or publication error and
    /// leaves intent plus accepted scene unchanged on rejection.
    pub fn delete_declaration(
        &mut self,
        node: NodeId,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        let closure = self.intent.graph().dependent_closure([node])?;
        let policy = if closure.len() == 1 {
            DeletePolicy::RejectDependents
        } else {
            DeletePolicy::Cascade {
                exact_nodes: closure,
            }
        };
        self.apply_patch(IntentPatch::new(
            self.intent.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::DeleteNode { node, policy }],
        ))
    }

    fn plan_patch(
        &self,
        patch: IntentPatch,
    ) -> Result<PlannedProjectionalTransaction, ProjectionalCoordinatorError> {
        self.plan_patch_with_work(patch, &mut InteractionWorkReceipt::default())
    }

    fn plan_patch_with_work(
        &self,
        patch: IntentPatch,
        work: &mut InteractionWorkReceipt,
    ) -> Result<PlannedProjectionalTransaction, ProjectionalCoordinatorError> {
        self.plan_patch_with_work_mode(patch, work, false, None)
    }

    fn plan_delegated_patch_with_work(
        &self,
        patch: IntentPatch,
        work: &mut InteractionWorkReceipt,
    ) -> Result<PlannedProjectionalTransaction, ProjectionalCoordinatorError> {
        self.plan_patch_with_work_mode(patch, work, true, None)
    }

    fn plan_patch_with_work_mode(
        &self,
        patch: IntentPatch,
        work: &mut InteractionWorkReceipt,
        delegated: bool,
        accepted_continuation: Option<&geosolve_sketch::SketchDocument>,
    ) -> Result<PlannedProjectionalTransaction, ProjectionalCoordinatorError> {
        #[cfg(test)]
        PLAN_PATCH_CALLS.with(|calls| calls.set(calls.get() + 1));
        let mut captured = None;
        let evaluate = |candidate: &geosolve_sketch_intent::IntentCandidate| {
            let (evaluation, materialized, materialization_work) =
                if let Some(accepted_continuation) = accepted_continuation {
                    self.materializer
                        .evaluate_with_materialization_from_accepted_continuation_audited(
                            candidate,
                            accepted_continuation,
                        )
                } else {
                    self.materializer
                        .evaluate_with_materialization_audited(candidate)
                };
            work.merge(materialization_work);
            captured = materialized;
            evaluation
        };
        let plan = if delegated {
            self.intent.plan_delegated_patch(patch, evaluate)?
        } else {
            self.intent.plan_patch(patch, evaluate)?
        };
        if plan.disposition() == IntentPlanDisposition::Accepted {
            return Ok(PlannedProjectionalTransaction::Accepted(
                PreparedProjectionalTransaction {
                    plan,
                    materialization: Box::new(
                        captured
                            .ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?,
                    ),
                },
            ));
        }
        if captured.is_some() {
            return Err(ProjectionalCoordinatorError::UnexpectedNonPublishingMaterialization);
        }
        Ok(PlannedProjectionalTransaction::NonPublishing { plan })
    }

    fn commit_planned(
        &mut self,
        planned: PlannedProjectionalTransaction,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.commit_planned_with_work(planned, &mut InteractionWorkReceipt::default())
    }

    fn commit_planned_with_work(
        &mut self,
        planned: PlannedProjectionalTransaction,
        work: &mut InteractionWorkReceipt,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.commit_planned_with_work_mode(planned, work, true)
    }

    fn commit_delegated_planned_with_work(
        &mut self,
        planned: PlannedProjectionalTransaction,
        work: &mut InteractionWorkReceipt,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.commit_planned_with_work_mode(planned, work, false)
    }

    fn commit_planned_with_work_mode(
        &mut self,
        planned: PlannedProjectionalTransaction,
        work: &mut InteractionWorkReceipt,
        publish_history: bool,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        let (plan, materialization) = match planned {
            PlannedProjectionalTransaction::Accepted(PreparedProjectionalTransaction {
                plan,
                materialization,
            }) => (plan, Some(materialization)),
            PlannedProjectionalTransaction::NonPublishing { plan } => (plan, None),
        };
        let disposition = plan.disposition();
        let aliases = plan.aliases().clone();
        let identity = self.intent.commit_plan(plan)?;
        if publish_history {
            work.record_history_publication();
        }
        if let Some(materialization) = materialization {
            self.accepted = Some(own_boxed_accepted(materialization));
        }
        Ok(ProjectionalPatchOutcome {
            identity,
            disposition,
            aliases,
        })
    }

    /// Atomically restores the preceding intent transaction and independently
    /// cold-rebuilds its accepted scene. A failed rebuild changes nothing.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-materialization error.
    pub fn undo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalCoordinatorError> {
        self.restore_history_position(true)
    }

    /// Atomically restores the next intent transaction and independently
    /// cold-rebuilds its accepted scene. A failed rebuild changes nothing.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-materialization error.
    pub fn redo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalCoordinatorError> {
        self.restore_history_position(false)
    }

    fn restore_history_position(
        &mut self,
        undo: bool,
    ) -> Result<Option<IntentSessionIdentity>, ProjectionalCoordinatorError> {
        self.cancel_interaction();
        let mut staged = self.intent.clone();
        let moved = if undo { staged.undo()? } else { staged.redo()? };
        let Some(_) = moved else {
            return Ok(None);
        };
        let mut refreshed = None;
        let current_was_refreshed = staged.refresh_current_accepted_evidence(|candidate| {
            let (evaluation, materialized) =
                self.materializer.evaluate_with_materialization(candidate);
            refreshed = materialized;
            evaluation
        })?;
        let accepted = if current_was_refreshed {
            Some(refreshed.ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?)
        } else {
            staged
                .accepted()
                .map(|authority| self.materializer.materialize_accepted_authority(authority))
                .transpose()?
        };
        let identity = staged.identity();
        self.intent = staged;
        self.accepted = accepted.map(own_accepted);
        Ok(Some(identity))
    }

    /// Prepares one native point continuation route from the exact accepted
    /// reverse ownership map. Both Cartesian leaves must have the same logical
    /// point owner; fixed/unmapped points fail before a pointer sample is solved.
    ///
    /// # Errors
    ///
    /// Returns a typed route, point, locality, or gesture-state error.
    pub fn begin_point_drag(
        &mut self,
        pointer_id: u64,
        point: DesignPointId,
    ) -> Result<(), ProjectionalCoordinatorError> {
        if self.point_drag.is_some() || self.curve_control_drag.is_some() {
            return Err(ProjectionalCoordinatorError::DragAlreadyActive);
        }
        let accepted = self
            .accepted
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        let x = accepted
            .ownership
            .writable_leaf(IntentNativeWritableLeaf::PointX { point })
            .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?;
        let y = accepted
            .ownership
            .writable_leaf(IntentNativeWritableLeaf::PointY { point })
            .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?;
        if x.node != y.node || x.port != y.port {
            return Err(ProjectionalCoordinatorError::SplitPointOwnership { point });
        }
        let accepted_state = accepted
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        let origin_position = accepted_state
            .document()
            .point(point)
            .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?
            .position;
        let locality = accepted.session.drag_locality_plan(point)?;
        self.point_drag = Some(ProjectionalPointDrag {
            pointer_id,
            intent: self.intent.identity(),
            point,
            locality,
            origin: Box::new(accepted.session.clone()),
            origin_position,
            latest_request_id: None,
            latest: None,
        });
        Ok(())
    }

    /// Solves one coalesced pointer sample through native retained continuation.
    /// It performs no intent serialization, graph replay, history mutation, or
    /// workspace work. Rejected samples preserve the preceding valid preview.
    ///
    /// # Errors
    ///
    /// Returns a pointer, stale-route, native-session, or interruption error.
    pub fn preview_point_drag(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        target: [f64; 2],
        control: OperationControl,
    ) -> Result<Option<ProjectionalPointDragPreview>, ProjectionalCoordinatorError> {
        self.preview_point_drag_audited(pointer_id, request_id, target, control)
            .into_outcome()
    }

    pub(crate) fn preview_point_drag_audited(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        target: [f64; 2],
        control: OperationControl,
    ) -> AuditedInteraction<
        Result<Option<ProjectionalPointDragPreview>, ProjectionalCoordinatorError>,
    > {
        let mut work = InteractionWorkReceipt::default();
        let outcome =
            self.preview_point_drag_with_work(pointer_id, request_id, target, control, &mut work);
        AuditedInteraction::new(outcome, work)
    }

    fn preview_point_drag_with_work(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        target: [f64; 2],
        control: OperationControl,
        work: &mut InteractionWorkReceipt,
    ) -> Result<Option<ProjectionalPointDragPreview>, ProjectionalCoordinatorError> {
        if !target.iter().all(|value| value.is_finite()) {
            return Err(ProjectionalCoordinatorError::NonFiniteDragTarget);
        }
        let mut drag = self
            .point_drag
            .take()
            .ok_or(ProjectionalCoordinatorError::NoActiveDrag)?;
        let result = (|| {
            if drag.pointer_id != pointer_id {
                return Err(ProjectionalCoordinatorError::PointerMismatch {
                    expected: drag.pointer_id,
                    actual: pointer_id,
                });
            }
            if drag.intent != self.intent.identity() {
                return Err(ProjectionalCoordinatorError::StaleDragRoute);
            }
            if drag
                .latest_request_id
                .is_some_and(|latest| request_id <= latest)
            {
                return Err(ProjectionalCoordinatorError::StaleDragSample);
            }
            drag.latest_request_id = Some(request_id);
            let request = drag
                .origin
                .request()
                .without_temporary_targets()
                .with_drag(drag.point, target);
            let mut candidate = drag.origin.as_ref().clone();
            work.record_native_preview_attempt();
            let outcome = if let Some(previous) = &drag.latest {
                candidate.reattempt_from_accepted_preview_with_drag_locality_controlled(
                    candidate.design_identity(),
                    request,
                    &previous.session,
                    &drag.locality,
                    control,
                )?
            } else {
                candidate.reattempt_with_drag_locality_controlled(
                    candidate.design_identity(),
                    request,
                    &drag.locality,
                    control,
                )?
            };
            match outcome {
                OperationOutcome::Completed { .. } => {
                    let Some(accepted) = candidate.accepted_state_for_current_input() else {
                        return Ok(None);
                    };
                    let Some(point) = accepted.document().point(drag.point) else {
                        return Ok(None);
                    };
                    let accepted_position = point.position;
                    if !accepted_position.iter().all(|value| value.is_finite()) {
                        return Ok(None);
                    }
                    let preview = ProjectionalPointDragPreview {
                        request_id,
                        point: drag.point,
                        accepted_position,
                    };
                    drag.latest = Some(AcceptedPointDragSample {
                        preview,
                        session: Box::new(candidate),
                    });
                    Ok(Some(preview))
                }
                OperationOutcome::Cancelled { .. } | OperationOutcome::WorkExhausted { .. } => {
                    Ok(None)
                }
                _ => Err(ProjectionalCoordinatorError::UnknownDragOutcome),
            }
        })();
        self.point_drag = Some(drag);
        result
    }

    /// Commits the newest exact accepted sample as one reverse-bound instance patch.
    ///
    /// A constrained drag may move more than the clicked point (for example a
    /// horizontal length-driven segment may translate rigidly). Every native
    /// point/scalar change is projected back only through the accepted
    /// materialization's writable reverse map. Driving targets, fixed-target
    /// fields and explicit branch fields therefore cannot be rewritten merely
    /// because the native solver used them while resolving the gesture.
    /// The cold result must reproduce the complete preview document byte-for-byte
    /// before either intent or native authority is published.
    ///
    /// # Errors
    ///
    /// Returns a stale/missing sample, exact-reconstruction, or patch error.
    pub fn finish_point_drag(
        &mut self,
        pointer_id: u64,
        request_id: u64,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        self.finish_point_drag_audited(pointer_id, request_id)
            .into_outcome()
    }

    pub(crate) fn finish_point_drag_audited(
        &mut self,
        pointer_id: u64,
        request_id: u64,
    ) -> AuditedInteraction<Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError>> {
        let mut work = InteractionWorkReceipt::default();
        let outcome = self.finish_point_drag_with_work(pointer_id, request_id, &mut work);
        AuditedInteraction::new(outcome, work)
    }

    fn finish_point_drag_with_work(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        work: &mut InteractionWorkReceipt,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        let terminal = self.take_validated_point_drag_terminal(pointer_id, request_id)?;
        let ownership = &self
            .accepted
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?
            .ownership;
        let origin_document = terminal
            .origin
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?
            .document();
        let preview_document = terminal
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedDragSample)?
            .document();
        let operations =
            point_drag_intent_operations(ownership, origin_document, preview_document)?;
        if operations.is_empty() {
            return Err(ProjectionalCoordinatorError::DragDidNotMove);
        }
        let patch = IntentPatch::new(
            self.intent.identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        );
        let prepared = self.plan_patch_with_work(patch, work)?.into_accepted()?;
        let materialized = prepared.materialization();
        let preview_document = terminal
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedDragSample)?
            .document();
        let cold_document = materialized
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?
            .document();
        if !direct_manipulation_preview_matches_cold(
            &self.intent,
            ownership,
            preview_document,
            cold_document,
        ) {
            return Err(ProjectionalCoordinatorError::PreviewColdMismatch);
        }
        self.commit_planned_with_work(PlannedProjectionalTransaction::Accepted(prepared), work)
    }

    /// Consumes the exact latest native preview for an authenticated outer
    /// semantic owner without manufacturing a generic Intent transaction.
    pub(crate) fn finish_point_drag_delegated(
        &mut self,
        pointer_id: u64,
        request_id: u64,
    ) -> Result<DelegatedPointDragProposal, ProjectionalCoordinatorError> {
        let terminal = self.take_validated_point_drag_terminal(pointer_id, request_id)?;
        Ok(DelegatedPointDragProposal {
            intent: terminal.intent,
            pointer_id,
            request_id,
            point: terminal.point,
            accepted_position: terminal.preview.accepted_position,
            terminal_session: terminal.session,
        })
    }

    fn take_validated_point_drag_terminal(
        &mut self,
        pointer_id: u64,
        request_id: u64,
    ) -> Result<ValidatedPointDragTerminal, ProjectionalCoordinatorError> {
        let drag = self
            .point_drag
            .take()
            .ok_or(ProjectionalCoordinatorError::NoActiveDrag)?;
        if drag.pointer_id != pointer_id {
            return Err(ProjectionalCoordinatorError::PointerMismatch {
                expected: drag.pointer_id,
                actual: pointer_id,
            });
        }
        if drag.intent != self.intent.identity() {
            return Err(ProjectionalCoordinatorError::StaleDragRoute);
        }
        let latest = drag
            .latest
            .ok_or(ProjectionalCoordinatorError::NoAcceptedDragSample)?;
        if latest.preview.request_id != request_id
            || drag
                .latest_request_id
                .is_none_or(|latest_request_id| request_id > latest_request_id)
        {
            return Err(ProjectionalCoordinatorError::StaleDragSample);
        }
        if pair_bits(latest.preview.accepted_position) == pair_bits(drag.origin_position) {
            return Err(ProjectionalCoordinatorError::DragDidNotMove);
        }
        Ok(ValidatedPointDragTerminal {
            intent: drag.intent,
            point: drag.point,
            origin: drag.origin,
            preview: latest.preview,
            session: latest.session,
        })
    }

    /// Cancels any active gesture without changing intent, accepted authority,
    /// allocator, or history.
    pub fn cancel_point_drag(&mut self) {
        self.point_drag = None;
    }

    /// Prepares one non-point selected-curve control route from the exact
    /// accepted materialization. The route resolves the native target back to
    /// its sole writable instance leaf or rational-conic definition owner
    /// before any pointer-frame solve is allowed.
    ///
    /// # Errors
    ///
    /// Returns a typed stale scene, read-only/unowned target, or gesture-state
    /// error without changing intent or accepted authority.
    #[allow(
        clippy::too_many_lines,
        reason = "one closed reverse-ownership match keeps every native curve-control target explicit"
    )]
    pub fn begin_curve_control_drag(
        &mut self,
        pointer_id: u64,
        accepted_revision: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
    ) -> Result<(), ProjectionalCoordinatorError> {
        if self.point_drag.is_some() || self.curve_control_drag.is_some() {
            return Err(ProjectionalCoordinatorError::DragAlreadyActive);
        }
        let accepted = self
            .accepted
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if accepted.session.design_identity() != expected {
            return Err(ProjectionalCoordinatorError::StaleCurveControlRoute);
        }
        let accepted_state = accepted
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
        if accepted_state.identity().revision().get() != accepted_revision {
            return Err(ProjectionalCoordinatorError::StaleCurveControlRoute);
        }
        let view = accepted_state
            .document()
            .curve_controls(control.curve)?
            .into_iter()
            .find(|candidate| candidate.id == control)
            .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
        if !matches!(
            view.availability,
            DocumentCurveControlAvailability::Editable
        ) {
            return Err(ProjectionalCoordinatorError::CurveControlNotWritable { control });
        }
        let route = match view.target {
            DocumentCurveControlTarget::Point(point) => {
                let x = accepted
                    .ownership
                    .writable_leaf(IntentNativeWritableLeaf::PointX { point })
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                let y = accepted
                    .ownership
                    .writable_leaf(IntentNativeWritableLeaf::PointY { point })
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                if x.node != y.node || x.port != y.port {
                    return Err(ProjectionalCoordinatorError::SplitPointOwnership { point });
                }
                ProjectionalCurveControlRoute::Point {
                    point,
                    x,
                    y,
                    origin: view.position,
                }
            }
            DocumentCurveControlTarget::Scalar(scalar) => {
                let leaf = accepted
                    .ownership
                    .writable_leaf(IntentNativeWritableLeaf::ScalarValue { scalar })
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                let scalar_value = accepted_state
                    .document()
                    .scalar(scalar)
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                ProjectionalCurveControlRoute::Scalar {
                    scalar,
                    leaf,
                    unit: intent_unit(scalar_value.unit),
                    origin: scalar_value.value,
                }
            }
            DocumentCurveControlTarget::RationalMiddle { weight, .. } => {
                let weight_leaf = accepted
                    .ownership
                    .writable_leaf(IntentNativeWritableLeaf::ScalarValue { scalar: weight })
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                let node = exact_owned_curve_node(&accepted.ownership, control.curve)
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                let intent_node = self
                    .intent
                    .graph()
                    .node(node)
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?;
                if intent_node.kind
                    != (IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::RationalQuadraticConic,
                    })
                    || weight_leaf.node != node
                {
                    return Err(ProjectionalCoordinatorError::CurveControlNotWritable { control });
                }
                let CurveDefinition::RationalQuadraticConic {
                    weighted_middle,
                    middle_weight,
                    ..
                } = accepted
                    .session
                    .design_document()
                    .curve(control.curve)
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?
                    .definition
                else {
                    return Err(ProjectionalCoordinatorError::CurveControlNotWritable { control });
                };
                let origin_weight = accepted
                    .session
                    .design_document()
                    .scalar(middle_weight)
                    .ok_or(ProjectionalCoordinatorError::CurveControlNotWritable { control })?
                    .value;
                ProjectionalCurveControlRoute::RationalMiddle {
                    curve: control.curve,
                    node,
                    weight: weight_leaf,
                    origin_weighted_middle: weighted_middle,
                    origin_weight,
                }
            }
            _ => return Err(ProjectionalCoordinatorError::CurveControlNotWritable { control }),
        };
        self.curve_control_drag = Some(ProjectionalCurveControlDrag {
            pointer_id,
            intent: self.intent.identity(),
            expected,
            accepted_revision,
            control,
            route,
            origin: Box::new(accepted.session.clone()),
            latest_request_id: None,
            latest: None,
        });
        Ok(())
    }

    /// Solves one coalesced selected-curve control sample against the retained
    /// pointer-down native snapshot. No intent replay, serialization, history,
    /// or durable materialization occurs on this path. A rejected sample keeps
    /// the preceding accepted preview available for release.
    ///
    /// # Errors
    ///
    /// Returns a typed stale route/sample, inverse-projection, or bounded native
    /// operation error.
    pub fn preview_curve_control_drag(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        target: [f64; 2],
        operation_control: OperationControl,
    ) -> Result<Option<ProjectionalCurveControlPreview>, ProjectionalCoordinatorError> {
        self.preview_curve_control_drag_audited(
            pointer_id,
            request_id,
            expected,
            control,
            target,
            operation_control,
        )
        .into_outcome()
    }

    pub(crate) fn preview_curve_control_drag_audited(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        target: [f64; 2],
        operation_control: OperationControl,
    ) -> AuditedInteraction<
        Result<Option<ProjectionalCurveControlPreview>, ProjectionalCoordinatorError>,
    > {
        let mut work = InteractionWorkReceipt::default();
        let outcome = self.preview_curve_control_drag_with_work(
            pointer_id,
            request_id,
            expected,
            control,
            target,
            operation_control,
            &mut work,
        );
        AuditedInteraction::new(outcome, work)
    }

    #[allow(clippy::too_many_arguments)]
    fn preview_curve_control_drag_with_work(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        target: [f64; 2],
        operation_control: OperationControl,
        work: &mut InteractionWorkReceipt,
    ) -> Result<Option<ProjectionalCurveControlPreview>, ProjectionalCoordinatorError> {
        if !target.iter().all(|value| value.is_finite()) {
            return Err(ProjectionalCoordinatorError::NonFiniteDragTarget);
        }
        let mut drag = self
            .curve_control_drag
            .take()
            .ok_or(ProjectionalCoordinatorError::NoActiveCurveControlDrag)?;
        let result = (|| {
            authenticate_curve_control_drag(
                &drag,
                pointer_id,
                expected,
                control,
                self.intent.identity(),
            )?;
            if drag
                .latest_request_id
                .is_some_and(|latest| request_id <= latest)
            {
                return Err(ProjectionalCoordinatorError::StaleDragSample);
            }
            drag.latest_request_id = Some(request_id);
            let accepted = drag
                .origin
                .accepted_state_for_current_input()
                .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?;
            let Ok(projection) = accepted.document().project_curve_control(control, target) else {
                return Ok(None);
            };
            if !curve_control_projection_matches_route(projection, drag.route) {
                return Err(ProjectionalCoordinatorError::CurveControlRouteMismatch);
            }
            let edit = curve_control_projection_edit(projection)
                .ok_or(ProjectionalCoordinatorError::CurveControlRouteMismatch)?;
            work.record_native_preview_attempt();
            let Ok(outcome) = drag
                .origin
                .prepared_snapshot()
                .prepare(PreparedSketchOperation::Apply(edit))
                .execute(operation_control)
            else {
                return Ok(None);
            };
            let OperationOutcome::Completed { value: patch, .. } = outcome else {
                return Ok(None);
            };
            let preview_session = patch
                .preview()
                .accepted_session()
                .ok_or(ProjectionalCoordinatorError::NoAcceptedCurveControlSample)?;
            let preview_document = preview_session
                .accepted_state_for_current_input()
                .ok_or(ProjectionalCoordinatorError::NoAcceptedCurveControlSample)?
                .document();
            let accepted_position = preview_document
                .curve_controls(control.curve)?
                .into_iter()
                .find(|candidate| candidate.id == control)
                .map(|candidate| candidate.position)
                .filter(|position| position.iter().all(|value| value.is_finite()))
                .ok_or(ProjectionalCoordinatorError::NoAcceptedCurveControlSample)?;
            let (operations, changed) = curve_control_intent_operations(
                drag.route,
                patch.preview().design_document(),
                preview_document,
            )?;
            let preview = ProjectionalCurveControlPreview {
                request_id,
                control,
                accepted_position,
            };
            drag.latest = Some(AcceptedCurveControlSample {
                preview,
                patch: Box::new(patch),
                operations,
                changed,
            });
            Ok(Some(preview))
        })();
        self.curve_control_drag = Some(drag);
        result
    }

    /// Publishes the newest authenticated curve-control sample as one typed
    /// intent patch. The cold materialization must exactly reproduce the native
    /// preview document before either authority or history advances.
    ///
    /// # Errors
    ///
    /// Returns a typed stale/missing sample, cold mismatch, or patch failure.
    pub fn finish_curve_control_drag(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
    ) -> Result<Option<ProjectionalPatchOutcome>, ProjectionalCoordinatorError> {
        self.finish_curve_control_drag_audited(pointer_id, request_id, expected, control)
            .into_outcome()
    }

    pub(crate) fn finish_curve_control_drag_audited(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
    ) -> AuditedInteraction<Result<Option<ProjectionalPatchOutcome>, ProjectionalCoordinatorError>>
    {
        let mut work = InteractionWorkReceipt::default();
        let outcome = self.finish_curve_control_drag_with_work(
            pointer_id, request_id, expected, control, &mut work,
        );
        AuditedInteraction::new(outcome, work)
    }

    fn finish_curve_control_drag_with_work(
        &mut self,
        pointer_id: u64,
        request_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
        work: &mut InteractionWorkReceipt,
    ) -> Result<Option<ProjectionalPatchOutcome>, ProjectionalCoordinatorError> {
        let drag = self
            .curve_control_drag
            .take()
            .ok_or(ProjectionalCoordinatorError::NoActiveCurveControlDrag)?;
        authenticate_curve_control_drag(
            &drag,
            pointer_id,
            expected,
            control,
            self.intent.identity(),
        )?;
        let latest = drag
            .latest
            .ok_or(ProjectionalCoordinatorError::NoAcceptedCurveControlSample)?;
        if latest.preview.request_id != request_id
            || drag
                .latest_request_id
                .is_none_or(|latest_request_id| request_id > latest_request_id)
        {
            return Err(ProjectionalCoordinatorError::StaleDragSample);
        }
        if !latest.changed {
            return Ok(None);
        }
        let patch = IntentPatch::new(
            self.intent.identity(),
            IntentPatchPolicy::RequireAccepted,
            latest.operations,
        );
        let prepared = self.plan_patch_with_work(patch, work)?.into_accepted()?;
        let materialized = prepared.materialization();
        let ownership = &self
            .accepted
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedAuthority)?
            .ownership;
        let preview_patch = latest.patch.preview();
        let preview_document = preview_patch
            .accepted_document()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedCurveControlSample)?;
        let cold_document = materialized
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?
            .document();
        if !direct_manipulation_preview_matches_cold(
            &self.intent,
            ownership,
            preview_document,
            cold_document,
        ) {
            return Err(ProjectionalCoordinatorError::PreviewColdMismatch);
        }
        self.commit_planned_with_work(PlannedProjectionalTransaction::Accepted(prepared), work)
            .map(Some)
    }

    /// Cancels any prepared selected-curve control route without touching
    /// intent, accepted authority, native allocators, or history.
    pub fn cancel_curve_control_drag(&mut self) {
        self.curve_control_drag = None;
    }

    /// Exact pointer-down origin carried by the currently visible candidate.
    #[must_use]
    pub fn curve_control_preview_origin(
        &self,
    ) -> Option<(u64, SketchDesignIdentity, u64, [f64; 2])> {
        let drag = self.curve_control_drag.as_ref()?;
        let latest = drag.latest.as_ref()?;
        Some((
            drag.accepted_revision,
            drag.expected,
            latest.preview.request_id,
            latest.preview.accepted_position,
        ))
    }

    fn cancel_interaction(&mut self) {
        self.cancel_point_drag();
        self.cancel_curve_control_drag();
    }
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
}

/// Compares a retained direct-manipulation preview with its cold reconstruction.
///
/// Native continuation deliberately retains a line span's previous explicit
/// branch vector while its endpoints move. Higher-level recipes which do not
/// expose a branch field instead derive that same positive branch from their
/// current endpoints during every cold materialization. The two authorities
/// can therefore differ only by round-off in this recomputable metadata (for
/// example `[1, 0]` versus `[1, 7e-17]`) even though every accepted coordinate
/// and branch cell is identical.
///
/// Normalize only the closed schema-owned derived set in the canonical draft
/// comparison before requiring exact document equality. Segment and Midpoint
/// Line branches remain explicit intent fields and are never normalized away
/// here. A derived vector must also remain in the preview's same positive
/// branch cell; an actual branch flip still rejects.
fn direct_manipulation_preview_matches_cold(
    intent: &IntentSession,
    ownership: &crate::IntentMaterializationMap,
    preview: &SketchDocument,
    cold: &SketchDocument,
) -> bool {
    if preview == cold {
        return true;
    }
    let mut recomputable = std::collections::BTreeSet::new();
    for materialization in &ownership.nodes {
        let Some(node) = intent.graph().node(materialization.node) else {
            continue;
        };
        let IntentNodeKind::Geometry { recipe } = node.kind else {
            continue;
        };
        if !geometry_recipe_has_derived_line_branches(recipe) {
            continue;
        }
        for curve in materialization
            .owned
            .iter()
            .filter_map(|binding| match binding {
                crate::IntentNativeBinding::Curve(curve) => Some(*curve),
                _ => None,
            })
        {
            recomputable.insert(curve);
        }
    }
    preview.exact_except_recomputable_line_branches(cold, &recomputable)
}

const fn geometry_recipe_has_derived_line_branches(recipe: GeometryRecipeKind) -> bool {
    matches!(
        recipe,
        GeometryRecipeKind::Polyline
            | GeometryRecipeKind::TwoPointAlignedRectangle
            | GeometryRecipeKind::ThreePointCornerRectangle
            | GeometryRecipeKind::CenterRectangle
            | GeometryRecipeKind::ThreePointCenterRectangle
    )
}

#[cfg(test)]
mod direct_manipulation_comparison_tests {
    use super::*;

    #[test]
    fn only_schema_derived_line_recipes_enter_branch_normalization() {
        let derived = GeometryRecipeKind::ALL
            .into_iter()
            .filter(|recipe| geometry_recipe_has_derived_line_branches(*recipe))
            .collect::<Vec<_>>();
        assert_eq!(
            derived,
            vec![
                GeometryRecipeKind::Polyline,
                GeometryRecipeKind::TwoPointAlignedRectangle,
                GeometryRecipeKind::ThreePointCornerRectangle,
                GeometryRecipeKind::CenterRectangle,
                GeometryRecipeKind::ThreePointCenterRectangle,
            ]
        );
        assert!(!geometry_recipe_has_derived_line_branches(
            GeometryRecipeKind::Segment
        ));
        assert!(!geometry_recipe_has_derived_line_branches(
            GeometryRecipeKind::MidpointLine
        ));
    }
}

#[cfg(test)]
mod prepared_transaction_tests {
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_intent::{
        GeometryRecipeKind, IntentNodeDraft, IntentNodeKind, IntentPatchOperation, IntentPortRole,
        IntentPortSelector,
    };

    use super::*;

    fn coordinate(value: f64) -> IntentLiteral {
        IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        }
    }

    fn point_patch(
        coordinator: &ProjectionalIntentCoordinator,
        alias: &str,
        position: [f64; 2],
    ) -> IntentPatch {
        let selector = IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        };
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            IntentKey::new(alias).expect("test alias is valid"),
        )
        .with_instance_leaf(
            selector,
            geosolve_sketch_intent::LeafField::X,
            coordinate(position[0]),
        )
        .with_instance_leaf(
            selector,
            geosolve_sketch_intent::LeafField::Y,
            coordinate(position[1]),
        );
        IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: IntentKey::new(alias).expect("test alias is valid"),
                draft: Box::new(draft),
                cell: None,
            }],
        )
    }

    fn coordinator(raw: u128) -> ProjectionalIntentCoordinator {
        ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(raw),
            ColdIntentMaterializer::with_default_policy(
                DocumentId(PersistentId::from_u128(raw << 32)),
                1.0,
            )
            .expect("test materializer"),
        )
        .expect("test coordinator")
    }

    #[test]
    fn accepted_prepared_transaction_installs_its_exact_validated_materialization() {
        let mut coordinator = coordinator(0x8300_7001);
        PLAN_PATCH_CALLS.with(|calls| calls.set(0));
        let prepared = coordinator.prepare_patch_transaction_audited(point_patch(
            &coordinator,
            "prepared.point",
            [2.0, 3.0],
        ));
        assert_eq!(prepared.work.intent_materialization_attempts(), 1);
        assert_eq!(prepared.work.computed_evaluation_attempts(), 1);
        assert_eq!(prepared.work.history_publications(), 0);
        let prepared = prepared
            .outcome
            .expect("prepare")
            .expect("accepted transaction");
        assert_eq!(PLAN_PATCH_CALLS.with(std::cell::Cell::get), 1);
        let expected_target = prepared.plan.target();
        let expected_evidence = prepared.materialization().evidence.clone();
        assert!(coordinator.accepted_materialization().is_none());

        let committed = coordinator.commit_prepared_transaction_audited(prepared);
        assert_eq!(committed.work.intent_materialization_attempts(), 0);
        assert_eq!(committed.work.computed_evaluation_attempts(), 0);
        assert_eq!(committed.work.history_publications(), 1);
        let outcome = committed
            .outcome
            .expect("commit exact prepared transaction");

        assert_eq!(
            PLAN_PATCH_CALLS.with(std::cell::Cell::get),
            1,
            "committing a prepared preview must not plan or cold-materialize again"
        );
        assert_eq!(outcome.identity, expected_target);
        assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
        assert_eq!(coordinator.intent().undo_len(), 1);
        assert_eq!(
            coordinator
                .accepted_materialization()
                .expect("accepted materialization")
                .evidence,
            expected_evidence
        );
    }

    #[test]
    fn stale_prepared_transaction_cannot_replace_newer_intent_or_native_authority() {
        let mut coordinator = coordinator(0x8300_7002);
        let prepared = coordinator
            .prepare_patch_transaction_audited(point_patch(&coordinator, "stale.point", [2.0, 3.0]))
            .into_outcome()
            .expect("prepare")
            .expect("accepted transaction");
        coordinator
            .apply_patch(point_patch(&coordinator, "newer.point", [5.0, 7.0]))
            .expect("publish intervening mutation");
        let identity = coordinator.intent().identity();
        let evidence = coordinator
            .accepted_materialization()
            .expect("newer accepted authority")
            .evidence
            .clone();

        assert!(matches!(
            coordinator.commit_prepared_transaction(prepared),
            Err(ProjectionalCoordinatorError::Intent(
                IntentSessionError::StaleCas { .. }
            ))
        ));
        assert_eq!(coordinator.intent().identity(), identity);
        assert_eq!(
            coordinator
                .accepted_materialization()
                .expect("newer authority retained")
                .evidence,
            evidence
        );
    }
}

fn point_drag_intent_operations(
    ownership: &crate::IntentMaterializationMap,
    origin: &geosolve_sketch::SketchDocument,
    preview: &geosolve_sketch::SketchDocument,
) -> Result<Vec<IntentPatchOperation>, ProjectionalCoordinatorError> {
    let mut operations = Vec::new();
    for (native, leaf) in &ownership.writable_leaves {
        let (origin_value, preview_value, unit) = match *native {
            IntentNativeWritableLeaf::PointX { point } => (
                origin
                    .point(point)
                    .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?
                    .position[0],
                preview
                    .point(point)
                    .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?
                    .position[0],
                IntentUnit::Length,
            ),
            IntentNativeWritableLeaf::PointY { point } => (
                origin
                    .point(point)
                    .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?
                    .position[1],
                preview
                    .point(point)
                    .ok_or(ProjectionalCoordinatorError::PointNotWritable { point })?
                    .position[1],
                IntentUnit::Length,
            ),
            IntentNativeWritableLeaf::ScalarValue { scalar } => {
                let origin = origin
                    .scalar(scalar)
                    .ok_or(ProjectionalCoordinatorError::WritableScalarMissing { scalar })?;
                let preview = preview
                    .scalar(scalar)
                    .ok_or(ProjectionalCoordinatorError::WritableScalarMissing { scalar })?;
                if origin.unit != preview.unit {
                    return Err(ProjectionalCoordinatorError::WritableScalarUnitMismatch {
                        scalar,
                    });
                }
                (origin.value, preview.value, intent_unit(preview.unit))
            }
        };
        if origin_value.to_bits() != preview_value.to_bits() {
            operations.push(IntentPatchOperation::SetInstanceLeaf {
                leaf: *leaf,
                value: IntentLiteral::Quantity {
                    value: preview_value,
                    unit,
                },
            });
        }
    }
    Ok(operations)
}

const fn intent_unit(unit: ScalarUnit) -> IntentUnit {
    match unit {
        ScalarUnit::Length => IntentUnit::Length,
        ScalarUnit::Angle => IntentUnit::Angle,
        ScalarUnit::Parameter => IntentUnit::Dimensionless,
    }
}

fn exact_owned_curve_node(
    ownership: &crate::IntentMaterializationMap,
    curve: CurveId,
) -> Option<NodeId> {
    let mut owners = ownership.nodes.iter().filter_map(|node| {
        node.owned
            .contains(&crate::IntentNativeBinding::Curve(curve))
            .then_some(node.node)
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

fn authenticate_curve_control_drag(
    drag: &ProjectionalCurveControlDrag,
    pointer_id: u64,
    expected: SketchDesignIdentity,
    control: DocumentCurveControlId,
    intent: IntentSessionIdentity,
) -> Result<(), ProjectionalCoordinatorError> {
    if drag.pointer_id != pointer_id {
        return Err(ProjectionalCoordinatorError::PointerMismatch {
            expected: drag.pointer_id,
            actual: pointer_id,
        });
    }
    if drag.intent != intent || drag.expected != expected {
        return Err(ProjectionalCoordinatorError::StaleCurveControlRoute);
    }
    if drag.control != control {
        return Err(ProjectionalCoordinatorError::CurveControlRouteMismatch);
    }
    Ok(())
}

fn curve_control_projection_matches_route(
    projection: DocumentCurveControlProjection,
    route: ProjectionalCurveControlRoute,
) -> bool {
    match (projection, route) {
        (
            DocumentCurveControlProjection::Point { point: actual, .. },
            ProjectionalCurveControlRoute::Point {
                point: expected, ..
            },
        ) => actual == expected,
        (
            DocumentCurveControlProjection::Scalar { scalar: actual, .. },
            ProjectionalCurveControlRoute::Scalar {
                scalar: expected, ..
            },
        ) => actual == expected,
        (
            DocumentCurveControlProjection::RationalMiddle { curve: actual, .. },
            ProjectionalCurveControlRoute::RationalMiddle {
                curve: expected, ..
            },
        ) => actual == expected,
        _ => false,
    }
}

fn curve_control_projection_edit(
    projection: DocumentCurveControlProjection,
) -> Option<DocumentEdit> {
    match projection {
        DocumentCurveControlProjection::Point { point, position } => {
            Some(DocumentEdit::SetPointPosition { point, position })
        }
        DocumentCurveControlProjection::Scalar { scalar, value } => {
            Some(DocumentEdit::SetScalarValue { scalar, value })
        }
        DocumentCurveControlProjection::RationalMiddle { curve, control } => {
            let weighted_middle = match control {
                DocumentRationalConicControl::Euclidean { middle, weight } => {
                    [middle[0] * weight, middle[1] * weight]
                }
                DocumentRationalConicControl::Projective {
                    weighted_middle, ..
                } => weighted_middle,
                _ => return None,
            };
            weighted_middle
                .iter()
                .all(|value| value.is_finite())
                .then_some(DocumentEdit::SetConicWeightedMiddle {
                    curve,
                    weighted_middle,
                })
        }
        _ => None,
    }
}

fn curve_control_intent_operations(
    route: ProjectionalCurveControlRoute,
    design: &geosolve_sketch::SketchDocument,
    accepted: &geosolve_sketch::SketchDocument,
) -> Result<(Vec<IntentPatchOperation>, bool), ProjectionalCoordinatorError> {
    match route {
        ProjectionalCurveControlRoute::Point {
            point,
            x,
            y,
            origin,
        } => {
            let position = accepted
                .point(point)
                .ok_or(ProjectionalCoordinatorError::CurveControlRouteMismatch)?
                .position;
            Ok((
                [(x, position[0]), (y, position[1])]
                    .into_iter()
                    .map(|(leaf, value)| IntentPatchOperation::SetInstanceLeaf {
                        leaf,
                        value: IntentLiteral::Quantity {
                            value,
                            unit: IntentUnit::Length,
                        },
                    })
                    .collect(),
                pair_bits(position) != pair_bits(origin),
            ))
        }
        ProjectionalCurveControlRoute::Scalar {
            scalar,
            leaf,
            unit,
            origin,
        } => {
            let value = accepted
                .scalar(scalar)
                .ok_or(ProjectionalCoordinatorError::CurveControlRouteMismatch)?
                .value;
            Ok((
                vec![IntentPatchOperation::SetInstanceLeaf {
                    leaf,
                    value: IntentLiteral::Quantity { value, unit },
                }],
                value.to_bits() != origin.to_bits(),
            ))
        }
        ProjectionalCurveControlRoute::RationalMiddle {
            curve,
            node,
            weight,
            origin_weighted_middle,
            origin_weight,
        } => {
            let CurveDefinition::RationalQuadraticConic {
                weighted_middle,
                middle_weight,
                ..
            } = design
                .curve(curve)
                .ok_or(ProjectionalCoordinatorError::CurveControlRouteMismatch)?
                .definition
            else {
                return Err(ProjectionalCoordinatorError::CurveControlRouteMismatch);
            };
            let current_weight = design
                .scalar(middle_weight)
                .ok_or(ProjectionalCoordinatorError::CurveControlRouteMismatch)?
                .value;
            let middle_changed = pair_bits(weighted_middle) != pair_bits(origin_weighted_middle);
            let weight_changed = current_weight.to_bits() != origin_weight.to_bits();
            let mut operations = vec![IntentPatchOperation::SetDefinitionField {
                node,
                field: IntentFieldKey(
                    IntentKey::new("weighted_middle").expect("built-in intent field key is valid"),
                ),
                value: IntentLiteral::Point(weighted_middle),
            }];
            if weight_changed {
                operations.push(IntentPatchOperation::SetInstanceLeaf {
                    leaf: weight,
                    value: IntentLiteral::Quantity {
                        value: current_weight,
                        unit: IntentUnit::Dimensionless,
                    },
                });
            }
            Ok((operations, middle_changed || weight_changed))
        }
    }
}

/// Projectional coordinator failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionalCoordinatorError {
    #[error(transparent)]
    Plan(#[from] IntentPlanError),
    #[error(transparent)]
    Intent(#[from] IntentSessionError),
    #[error(transparent)]
    Graph(#[from] geosolve_sketch_intent::IntentGraphError),
    #[error(transparent)]
    Materialization(#[from] IntentMaterializationError),
    #[error(transparent)]
    Native(#[from] DocumentSessionError),
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error("an accepted plan omitted its independently validated native materialization")]
    MissingAcceptedMaterialization,
    #[error("a non-publishing plan unexpectedly produced native materialization")]
    UnexpectedNonPublishingMaterialization,
    #[error("there is no independently accepted intent authority")]
    NoAcceptedAuthority,
    #[error("the restored native bootstrap does not match its accepted intent authority")]
    BootstrapAuthorityMismatch,
    #[error("a point drag is already active")]
    DragAlreadyActive,
    #[error("there is no active point drag")]
    NoActiveDrag,
    #[error("there is no active selected-curve control drag")]
    NoActiveCurveControlDrag,
    #[error("native point {point} has no writable projectional owner")]
    PointNotWritable { point: DesignPointId },
    #[error("native point {point} has split Cartesian ownership")]
    SplitPointOwnership { point: DesignPointId },
    #[error("native writable scalar {scalar} is absent from one point-drag authority")]
    WritableScalarMissing { scalar: DesignScalarId },
    #[error("native writable scalar {scalar} changed unit during one point drag")]
    WritableScalarUnitMismatch { scalar: DesignScalarId },
    #[error("pointer mismatch: expected {expected}, received {actual}")]
    PointerMismatch { expected: u64, actual: u64 },
    #[error("the point-drag route is stale")]
    StaleDragRoute,
    #[error("the selected-curve control route is stale")]
    StaleCurveControlRoute,
    #[error("selected-curve control {control:?} has no exact writable projectional owner")]
    CurveControlNotWritable { control: DocumentCurveControlId },
    #[error("the selected-curve control projection differs from its prepared route")]
    CurveControlRouteMismatch,
    #[error("the point-drag sample is stale")]
    StaleDragSample,
    #[error("the point-drag target must be finite")]
    NonFiniteDragTarget,
    #[error("point-drag work was cancelled")]
    DragCancelled,
    #[error("point-drag work was exhausted")]
    DragWorkExhausted,
    #[error("the native point-drag operation returned an unknown outcome")]
    UnknownDragOutcome,
    #[error("no independently accepted point-drag sample is available")]
    NoAcceptedDragSample,
    #[error("no independently accepted selected-curve control sample is available")]
    NoAcceptedCurveControlSample,
    #[error("the accepted point-drag sample did not move the point")]
    DragDidNotMove,
    #[error("cold intent reconstruction differs from the exact accepted drag preview")]
    PreviewColdMismatch,
    #[error(transparent)]
    CurveControl(#[from] geosolve_sketch::DocumentCurveControlError),
}

// Ensure temporary requests are never accidentally retained by helper changes.
const _: fn(DocumentSolveRequest) -> DocumentSolveRequest =
    DocumentSolveRequest::without_temporary_targets;
