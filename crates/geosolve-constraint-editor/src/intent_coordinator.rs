// SPDX-License-Identifier: GPL-3.0-or-later

//! Single-history coordinator for projectional design intent.
//!
//! Durable mutation and Undo/Redo live exclusively in [`IntentSession`]. Native
//! retained sessions in this module are either an independently cold-rebuilt
//! accepted authority or gesture-local numerical continuation state; their own
//! command histories are never exposed or persisted.

use geosolve_sketch::{
    DesignPointId, DocumentDragLocalityPlan, DocumentSessionError, DocumentSolveRequest,
    OperationControl, OperationOutcome, RetainedSketchDocumentSession,
};
use geosolve_sketch_intent::{
    DeletePolicy, IntentAliasMap, IntentLiteral, IntentPatch, IntentPatchOperation,
    IntentPatchPlan, IntentPatchPolicy, IntentPlanDisposition, IntentPlanError, IntentSession,
    IntentSessionError, IntentSessionId, IntentSessionIdentity, IntentUnit, LeafRef, NodeId,
};
use thiserror::Error;

use crate::{
    ColdIntentMaterialization, ColdIntentMaterializer, IntentMaterializationError,
    IntentNativeWritableLeaf,
};

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

#[derive(Clone, Debug)]
struct AcceptedPointDragSample {
    preview: ProjectionalPointDragPreview,
    session: RetainedSketchDocumentSession,
}

#[derive(Clone, Debug)]
struct ProjectionalPointDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    point: DesignPointId,
    x: LeafRef,
    y: LeafRef,
    locality: DocumentDragLocalityPlan,
    origin: RetainedSketchDocumentSession,
    origin_position: [f64; 2],
    latest_request_id: Option<u64>,
    latest: Option<AcceptedPointDragSample>,
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
    accepted: Option<ColdIntentMaterialization>,
    point_drag: Option<ProjectionalPointDrag>,
}

impl ProjectionalIntentCoordinator {
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
            accepted,
            point_drag: None,
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
            accepted: Some(accepted),
            point_drag: None,
        })
    }

    #[must_use]
    pub const fn intent(&self) -> &IntentSession {
        &self.intent
    }

    /// Last independently accepted cold materialization. It remains available
    /// beneath a newer retained-failed intent transaction.
    #[must_use]
    pub const fn accepted_materialization(&self) -> Option<&ColdIntentMaterialization> {
        self.accepted.as_ref()
    }

    /// Native scene currently suitable for presentation. A valid pointer
    /// preview temporarily outranks the durable accepted scene.
    #[must_use]
    pub fn presentation_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.point_drag
            .as_ref()
            .and_then(|drag| drag.latest.as_ref())
            .map(|sample| &sample.session)
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
        self.cancel_point_drag();
        let (plan, materialized) = self.plan_patch(patch)?;
        self.commit_planned(plan, materialized)
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
    ) -> Result<(IntentPatchPlan, Option<ColdIntentMaterialization>), ProjectionalCoordinatorError>
    {
        let mut captured = None;
        let plan = self.intent.plan_patch(patch, |candidate| {
            let (evaluation, materialized) =
                self.materializer.evaluate_with_materialization(candidate);
            captured = materialized;
            evaluation
        })?;
        if plan.disposition() == IntentPlanDisposition::Accepted && captured.is_none() {
            return Err(ProjectionalCoordinatorError::MissingAcceptedMaterialization);
        }
        Ok((plan, captured))
    }

    fn commit_planned(
        &mut self,
        plan: IntentPatchPlan,
        materialized: Option<ColdIntentMaterialization>,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalCoordinatorError> {
        let disposition = plan.disposition();
        let aliases = plan.aliases().clone();
        let identity = self.intent.commit_plan(plan)?;
        if disposition == IntentPlanDisposition::Accepted {
            self.accepted = Some(
                materialized.ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?,
            );
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
        self.cancel_point_drag();
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
        self.accepted = accepted;
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
        if self.point_drag.is_some() {
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
            x,
            y,
            locality,
            origin: accepted.session.clone(),
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
            let mut candidate = drag.origin.clone();
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
                        session: candidate,
                    });
                    Ok(Some(preview))
                }
                OperationOutcome::Cancelled { .. } => {
                    Err(ProjectionalCoordinatorError::DragCancelled)
                }
                OperationOutcome::WorkExhausted { .. } => {
                    Err(ProjectionalCoordinatorError::DragWorkExhausted)
                }
                _ => Err(ProjectionalCoordinatorError::UnknownDragOutcome),
            }
        })();
        self.point_drag = Some(drag);
        result
    }

    /// Commits the newest exact accepted sample as one two-leaf instance patch.
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
        if latest.preview.request_id != request_id || drag.latest_request_id != Some(request_id) {
            return Err(ProjectionalCoordinatorError::StaleDragSample);
        }
        if pair_bits(latest.preview.accepted_position) == pair_bits(drag.origin_position) {
            return Err(ProjectionalCoordinatorError::DragDidNotMove);
        }
        let operations = [
            (drag.x, latest.preview.accepted_position[0]),
            (drag.y, latest.preview.accepted_position[1]),
        ]
        .into_iter()
        .map(|(leaf, value)| IntentPatchOperation::SetInstanceLeaf {
            leaf,
            value: IntentLiteral::Quantity {
                value,
                unit: IntentUnit::Length,
            },
        })
        .collect();
        let patch = IntentPatch::new(
            self.intent.identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        );
        let (plan, materialized) = self.plan_patch(patch)?;
        let materialized = materialized
            .as_ref()
            .ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?;
        let preview_document = latest
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::NoAcceptedDragSample)?
            .document()
            .to_draft_v5_json()?;
        let cold_document = materialized
            .session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalCoordinatorError::MissingAcceptedMaterialization)?
            .document()
            .to_draft_v5_json()?;
        if cold_document != preview_document {
            return Err(ProjectionalCoordinatorError::PreviewColdMismatch);
        }
        self.commit_planned(plan, Some(materialized.clone()))
    }

    /// Cancels any active gesture without changing intent, accepted authority,
    /// allocator, or history.
    pub fn cancel_point_drag(&mut self) {
        self.point_drag = None;
    }
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
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
    #[error("there is no independently accepted intent authority")]
    NoAcceptedAuthority,
    #[error("the restored native bootstrap does not match its accepted intent authority")]
    BootstrapAuthorityMismatch,
    #[error("a point drag is already active")]
    DragAlreadyActive,
    #[error("there is no active point drag")]
    NoActiveDrag,
    #[error("native point {point} has no writable projectional owner")]
    PointNotWritable { point: DesignPointId },
    #[error("native point {point} has split Cartesian ownership")]
    SplitPointOwnership { point: DesignPointId },
    #[error("pointer mismatch: expected {expected}, received {actual}")]
    PointerMismatch { expected: u64, actual: u64 },
    #[error("the point-drag route is stale")]
    StaleDragRoute,
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
    #[error("the accepted point-drag sample did not move the point")]
    DragDidNotMove,
    #[error("cold intent reconstruction differs from the exact accepted drag preview")]
    PreviewColdMismatch,
}

// Ensure temporary requests are never accidentally retained by helper changes.
const _: fn(DocumentSolveRequest) -> DocumentSolveRequest =
    DocumentSolveRequest::without_temporary_targets;
