// SPDX-License-Identifier: GPL-3.0-or-later

//! Private retained-history publication and restore helpers.

use super::{
    ComputedEvaluationAllocator, ComputedFeatureDocument, CoordinatorError, MutationOutcome,
    ReplayAction, RestoreCheckpoint, RetainedEditorCoordinator, RetainedSketchDocumentSession,
    SketchDocument, SketchLifecycleRevisionHighWater,
};

impl RetainedEditorCoordinator {
    pub(super) fn stage_feature_mutation_checkpoint(
        &self,
        features: &ComputedFeatureDocument,
    ) -> Result<RestoreCheckpoint, CoordinatorError> {
        checkpoint(&self.session, features, &self.computed_evaluation_allocator)
    }

    pub(super) fn record_feature_mutation(
        &mut self,
        next: RestoreCheckpoint,
        replay: ReplayAction,
    ) {
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.editor.invalidate_for_retained_state_change(true);
        self.clear_transient();
        self.reconcile_selection();
    }
}

pub(super) fn mutation_from<T: Clone>(
    outcome: &geosolve_sketch::RetainedDocumentTransactionOutcome<T>,
) -> MutationOutcome<T> {
    MutationOutcome {
        value: outcome.value().clone(),
        design: outcome.design_identity(),
        attempt: outcome.attempt_identity(),
        published_accepted: outcome.published_accepted_identity(),
    }
}

pub(super) fn checkpoint(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    evaluation_allocator: &ComputedEvaluationAllocator,
) -> Result<RestoreCheckpoint, CoordinatorError> {
    let (design_json, design_is_draft_v5) = checkpoint_document_to_json(session.design_document())?;
    let (accepted_json, accepted_is_draft_v5) = session.accepted_state().map_or_else(
        || Ok((None, false)),
        |accepted| {
            checkpoint_document_to_json(accepted.document())
                .map(|(json, is_draft)| (Some(json), is_draft))
        },
    )?;
    Ok(RestoreCheckpoint {
        design_json,
        design_is_draft_v5,
        accepted_json,
        accepted_is_draft_v5,
        accepted_belongs_to_current_design: session
            .accepted_state()
            .is_some_and(|accepted| accepted.design_identity() == session.design_identity()),
        revisions: session.revision_high_water(),
        sketch_identity_high_water: session.persistent_identity_high_water().clone(),
        feature_json: features.to_json()?,
        feature_lifecycle: features.lifecycle_high_water(),
        evaluation_allocator: evaluation_allocator.high_water(),
    })
}

pub(super) fn checkpoint_document_to_json(
    document: &SketchDocument,
) -> Result<(String, bool), geosolve_sketch::DocumentError> {
    match document.to_canonical_json() {
        Ok(json) => Ok((json, false)),
        Err(_) => document.to_draft_v5_json().map(|json| (json, true)),
    }
}

fn checkpoint_document_from_json(
    json: &str,
    is_draft_v5: bool,
) -> Result<SketchDocument, geosolve_sketch::DocumentError> {
    if is_draft_v5 {
        SketchDocument::from_draft_v5_json(json)
    } else {
        SketchDocument::from_json(json)
    }
}

pub(super) fn restore_sketch_checkpoint(
    current: &RetainedSketchDocumentSession,
    checkpoint: &RestoreCheckpoint,
    revisions: SketchLifecycleRevisionHighWater,
    accepted_restore: AcceptedCheckpointRestore,
) -> Result<RetainedSketchDocumentSession, CoordinatorError> {
    let high_water = current
        .persistent_identity_high_water()
        .merged(&checkpoint.sketch_identity_high_water)?;
    let mut design =
        checkpoint_document_from_json(&checkpoint.design_json, checkpoint.design_is_draft_v5)?;
    design.retain_persistent_identity_high_water(&high_water)?;
    let input = current.last_attempt().input();
    let request = input
        .candidate_request()
        .without_temporary_targets()
        .without_previous_state_preferences();
    let parameters = current.parameter_batch().clone();
    let snapshots = current.external_snapshot_set().clone();
    let mut restored = if let Some(json) = &checkpoint.accepted_json {
        let mut accepted = checkpoint_document_from_json(json, checkpoint.accepted_is_draft_v5)?;
        accepted.retain_persistent_identity_high_water(&high_water)?;
        let exact = if checkpoint.accepted_belongs_to_current_design {
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                design.clone(),
                accepted,
                revisions,
                parameters.clone(),
                snapshots.clone(),
                request,
                input.solver_config(),
            )
        } else {
            RetainedSketchDocumentSession::restore_design_with_accepted_and_inputs(
                design.clone(),
                accepted,
                revisions,
                parameters.clone(),
                snapshots.clone(),
                request,
                input.solver_config(),
            )
        };
        match (exact, accepted_restore) {
            (Ok(restored), _) => restored,
            (Err(error), AcceptedCheckpointRestore::RequireExact) => return Err(error.into()),
            (Err(_), AcceptedCheckpointRestore::PreferCurrentInputTruth) => {
                // Parameter values and external snapshots are host state rather than
                // sketch history. A historical accepted materialization can therefore
                // cease to certify after those inputs change. Restore the historical
                // design as a fresh attempt under the exact current inputs; it may
                // publish a newly accepted state or retain a typed input failure, but
                // the older accepted geometry must never masquerade as current.
                RetainedSketchDocumentSession::restore_design_with_inputs(
                    design,
                    revisions,
                    parameters,
                    snapshots,
                    request,
                    input.solver_config(),
                )?
            }
        }
    } else {
        RetainedSketchDocumentSession::restore_design_with_inputs(
            design,
            revisions,
            parameters,
            snapshots,
            request,
            input.solver_config(),
        )?
    };
    restored.retain_persistent_identity_high_water(&high_water)?;
    Ok(restored)
}

#[derive(Clone, Copy)]
pub(super) enum AcceptedCheckpointRestore {
    /// Persistence reload treats the stored accepted payload as a strict contract.
    RequireExact,
    /// History restores design intent under host inputs that are not historical state.
    PreferCurrentInputTruth,
}
