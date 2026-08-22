// SPDX-License-Identifier: GPL-3.0-or-later

//! Private retained-history publication and restore helpers.

use std::collections::BTreeMap;

use geosolve_sketch::ScalarValueEdit;

use super::lineage::{CoordinatorLineage, external_input_stamp};
use super::{
    ComputedEvaluationAllocator, ComputedFeatureDocument, ComputedFeatureEvaluationState,
    CoordinatorError, DocumentParameterTarget, MutationOutcome, OperationOutcome, ReplayAction,
    RestoreCheckpoint, RetainedEditorCoordinator, RetainedSketchDocumentSession, SketchDocument,
    SketchLifecycleRevisionHighWater, StagedSketchFeaturePublication, bounded_geometry_control,
    dimension_target_scalar, evaluate_computed_features,
    replace_session_accepted_from_lineage_evidence,
};

impl RetainedEditorCoordinator {
    /// Reifies every independently accepted projected value as retained design
    /// intent on scratch state. Lower-level sketch clients keep their ordinary
    /// single-point edit semantics; the lineage-authoritative workbench needs
    /// the complete accepted projection so every changed leaf can be routed
    /// back to its exact owner step and reproduced without a drag seed.
    pub(super) fn promote_direct_manipulation_projection(
        &self,
        candidate: &RetainedSketchDocumentSession,
    ) -> Result<RetainedSketchDocumentSession, CoordinatorError> {
        let accepted = candidate
            .accepted_state_for_current_input()
            .ok_or(CoordinatorError::DirectManipulationColdReproductionRejected)?;
        let accepted_document = accepted.document().clone();
        let mut promoted_design = accepted_document.clone();
        preserve_host_parameter_fallbacks(candidate.design_document(), &mut promoted_design)?;
        let input = candidate.last_attempt().input();
        let request = input.publication_request().without_temporary_targets();
        let promoted =
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                promoted_design,
                accepted_document,
                self.session.revision_high_water(),
                candidate.parameter_batch().clone(),
                candidate.latest_attempt_external_snapshot_set().clone(),
                request,
                input.solver_config(),
            )?;
        if promoted.design_identity() != candidate.design_identity()
            || promoted.last_attempt().identity() != candidate.last_attempt().identity()
            || promoted
                .accepted_state_for_current_input()
                .map(geosolve_sketch::SketchAcceptedDocumentState::identity)
                != Some(accepted.identity())
        {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }
        Ok(promoted)
    }

    /// Stages a computed-feature-only action against a canonical cold sketch.
    ///
    /// Feature intent does not alter retained sketch topology, but recording the
    /// action still performs a fresh strict lineage evaluation.  That evaluation
    /// may select a different valid solution for an underconstrained imported
    /// baseline, so its accepted evidence and the computed snapshot must publish
    /// together rather than leaving the pre-action live sketch cache in place.
    pub(super) fn stage_feature_mutation_publication(
        &self,
        features: &ComputedFeatureDocument,
        provisional_allocator: &ComputedEvaluationAllocator,
        provisional_snapshot: &super::ComputedFeatureSnapshot,
        replay: &ReplayAction,
    ) -> Result<StagedSketchFeaturePublication, CoordinatorError> {
        self.stage_sketch_feature_mutation_publication(
            self.session.clone(),
            features,
            provisional_allocator,
            provisional_snapshot,
            replay,
        )
    }

    /// Stages a sketch-changing action that also carries computed-feature
    /// state. Strict cold lineage evaluation owns accepted geometry. Exact
    /// provisional computed evidence is retained when its accepted sketch
    /// bytes already equal that authority; otherwise output and allocator are
    /// rebuilt from the canonical session. The action is then recorded a
    /// second time from the untouched live lineage so the final checkpoint and
    /// authority enter history atomically.
    pub(super) fn stage_sketch_feature_mutation_publication(
        &self,
        mut session: RetainedSketchDocumentSession,
        features: &ComputedFeatureDocument,
        provisional_allocator: &ComputedEvaluationAllocator,
        provisional_snapshot: &super::ComputedFeatureSnapshot,
        replay: &ReplayAction,
    ) -> Result<StagedSketchFeaturePublication, CoordinatorError> {
        let previous = checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?;
        let provisional = checkpoint(&session, features, provisional_allocator)?;
        let original_lineage = self.lineage.clone();
        let mut first_lineage = original_lineage.clone();
        let first_evaluation = first_lineage
            .record(
                replay,
                self.editor.geometry_tool_variant(),
                &previous,
                &provisional,
                session.parameter_batch(),
                session.external_snapshot_set(),
            )?
            .ok_or_else(|| {
                CoordinatorError::Lineage(
                    "accepted sketch-feature mutation produced rejected cold lineage evidence"
                        .into(),
                )
            })?;
        let provisional_accepted_matches_cold = provisional.accepted_belongs_to_current_design()
            && provisional.accepted_json() == Some(first_evaluation.accepted_sketch_json())
            && provisional.accepted_uses_draft_v5() == first_evaluation.accepted_uses_draft_v5();
        let (computed_evaluation_allocator, computed_snapshot) =
            if provisional_accepted_matches_cold {
                let accepted_input = session.accepted_prepared_input().ok_or_else(|| {
                    CoordinatorError::Lineage(
                        "accepted sketch-feature mutation lost its provisional accepted input"
                            .into(),
                    )
                })?;
                if provisional_snapshot.input().sketch != accepted_input
                    || provisional_snapshot.input().features != features.identity()
                {
                    return Err(CoordinatorError::Lineage(
                        "accepted sketch-feature mutation carried stale provisional computed output"
                            .into(),
                    ));
                }
                (provisional_allocator.clone(), provisional_snapshot.clone())
            } else {
                replace_session_accepted_from_lineage_evidence(
                    &mut session,
                    Some(&first_evaluation),
                )?;
                let mut allocator = self.computed_evaluation_allocator.clone();
                let evaluated = evaluate_computed_features(
                    &session,
                    features,
                    &mut allocator,
                    bounded_geometry_control(),
                )?;
                let OperationOutcome::Completed {
                    value: snapshot, ..
                } = evaluated
                else {
                    return Err(CoordinatorError::ComputedFeatureWorkStopped);
                };
                (allocator, snapshot)
            };
        let checkpoint = checkpoint(&session, features, &computed_evaluation_allocator)?;

        let mut lineage = original_lineage;
        let final_evaluation = lineage
            .record(
                replay,
                self.editor.geometry_tool_variant(),
                &previous,
                &checkpoint,
                session.parameter_batch(),
                session.external_snapshot_set(),
            )?
            .ok_or_else(|| {
                CoordinatorError::Lineage(
                    "canonical sketch-feature mutation became rejected during final lineage recording"
                        .into(),
                )
            })?;
        if final_evaluation.accepted_sketch_json() != first_evaluation.accepted_sketch_json()
            || final_evaluation.accepted_uses_draft_v5()
                != first_evaluation.accepted_uses_draft_v5()
        {
            return Err(CoordinatorError::Lineage(
                "canonical sketch-feature accepted geometry changed during final lineage recording"
                    .into(),
            ));
        }
        lineage
            .retain_computed_evaluation_high_water(checkpoint.computed_evaluation_high_water())?;
        Ok(StagedSketchFeaturePublication {
            lineage,
            session,
            computed_evaluation_allocator,
            computed_snapshot,
            checkpoint,
        })
    }

    /// Stages one direct-manipulation owner rewrite and proves it through a
    /// fresh owning-domain evaluation before any live coordinator field can be
    /// published. The proof intentionally allocates computed output only in a
    /// scratch allocator; rejection therefore cannot consume a live identity.
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic publication proof keeps host-input, cold sketch, accepted-witness and computed-feature authentication visibly ordered"
    )]
    pub(super) fn stage_direct_manipulation_lineage(
        &self,
        next: &RestoreCheckpoint,
        replay: &ReplayAction,
        candidate_session: &RetainedSketchDocumentSession,
    ) -> Result<Option<CoordinatorLineage>, CoordinatorError> {
        let previous = checkpoint(
            &self.session,
            &self.features,
            &self.computed_evaluation_allocator,
        )?;
        let mut lineage = self.lineage.clone();
        if !lineage.direct_manipulation_has_authored_change(&previous, next)? {
            return Ok(None);
        }
        let live_external_inputs = external_input_stamp(
            self.session.parameter_batch(),
            self.session.external_snapshot_set(),
        )?;
        let candidate_external_inputs = external_input_stamp(
            candidate_session.parameter_batch(),
            candidate_session.latest_attempt_external_snapshot_set(),
        )?;
        if live_external_inputs != candidate_external_inputs {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }
        let cold_evaluation = lineage.record_direct_manipulation(
            replay,
            self.editor.geometry_tool_variant(),
            &previous,
            next,
            candidate_session.parameter_batch(),
            candidate_session.latest_attempt_external_snapshot_set(),
        )?;
        if cold_evaluation.external_inputs() != &candidate_external_inputs {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }
        let materialized = lineage.materialize()?.into_restore_checkpoint();

        #[cfg(test)]
        if lineage.take_direct_reproduction_rejection_for_test() {
            return Err(CoordinatorError::DirectManipulationColdReproductionRejected);
        }

        let design = checkpoint_document_from_json(
            materialized.design_json(),
            materialized.design_uses_draft_v5(),
        )?;
        if next.accepted_json() != Some(cold_evaluation.accepted_sketch_json())
            || next.accepted_uses_draft_v5() != cold_evaluation.accepted_uses_draft_v5()
        {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }
        let accepted = current_accepted_document(next)?;
        let attempt_input = candidate_session.last_attempt().input();
        let request = attempt_input
            .candidate_request()
            .without_temporary_targets()
            .without_previous_state_preferences();
        let cold_session =
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                design,
                accepted,
                materialized.revisions(),
                candidate_session.parameter_batch().clone(),
                candidate_session
                    .latest_attempt_external_snapshot_set()
                    .clone(),
                request,
                attempt_input.solver_config(),
            )?;
        if cold_session.accepted_state_for_current_input().is_none() {
            return Err(CoordinatorError::DirectManipulationColdReproductionRejected);
        }

        let materialized_features =
            ComputedFeatureDocument::from_json(materialized.feature_json())?;
        let expected_features = ComputedFeatureDocument::from_json(next.feature_json())?;
        if materialized_features.id() != expected_features.id()
            || materialized_features.sketch_document() != expected_features.sketch_document()
            || materialized_features.features() != expected_features.features()
        {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }

        let mut scratch_allocator = ComputedEvaluationAllocator::from_high_water(
            materialized.computed_evaluation_high_water(),
        );
        let evaluated = evaluate_computed_features(
            &cold_session,
            &materialized_features,
            &mut scratch_allocator,
            bounded_geometry_control(),
        )?;
        let OperationOutcome::Completed {
            value: cold_features,
            ..
        } = evaluated
        else {
            return Err(CoordinatorError::DirectManipulationColdReproductionRejected);
        };
        if cold_features
            .feature_evaluations()
            .iter()
            .any(|evaluation| {
                matches!(
                    evaluation.state,
                    ComputedFeatureEvaluationState::Failed { .. }
                )
            })
        {
            return Err(CoordinatorError::DirectManipulationColdReproductionRejected);
        }

        #[cfg(test)]
        if lineage.take_direct_reproduction_mismatch_for_test() {
            return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
        }

        Ok(Some(lineage))
    }

    pub(super) fn publish_staged_feature_mutation(
        &mut self,
        lineage: CoordinatorLineage,
        next: RestoreCheckpoint,
        replay: ReplayAction,
    ) {
        self.lineage = lineage;
        self.history.truncate(self.history_cursor + 1);
        self.history.push(next);
        self.history_cursor += 1;
        self.transcript.push(replay);
        self.editor.invalidate_for_retained_state_change(true);
        self.clear_transient();
        self.reconcile_selection();
    }

    /// Atomically installs a feature-only publication after its sketch and
    /// computed output have both been rebuilt from strict-cold authority.
    pub(super) fn publish_staged_computed_feature_mutation(
        &mut self,
        staged: StagedSketchFeaturePublication,
        features: ComputedFeatureDocument,
        replay: ReplayAction,
    ) {
        self.session = staged.session;
        self.features = features;
        self.computed_evaluation_allocator = staged.computed_evaluation_allocator;
        self.computed_input = Some(staged.computed_snapshot.input());
        self.computed_snapshot = Some(staged.computed_snapshot);
        self.computed_evaluation_problem = None;
        self.publish_staged_feature_mutation(staged.lineage, staged.checkpoint, replay);
    }
}

/// Accepted documents contain effective host-parameter values, while the
/// retained design deliberately keeps the caller's local fallback values.
/// Direct-manipulation promotion may reify solver-projected geometry, but it
/// must not silently turn immutable host inputs into authored scalar edits.
fn preserve_host_parameter_fallbacks(
    retained: &SketchDocument,
    promoted: &mut SketchDocument,
) -> Result<(), CoordinatorError> {
    let mut fallbacks = BTreeMap::new();
    for binding in retained.parameter_bindings() {
        let scalar = match binding.target {
            DocumentParameterTarget::DrivingDimension(dimension) => {
                let definition = &retained
                    .dimension(dimension)
                    .ok_or(CoordinatorError::InvalidActionInput(
                        "host parameter targets a missing driving dimension",
                    ))?
                    .definition;
                dimension_target_scalar(definition)
            }
            DocumentParameterTarget::DimensionlessFixedScalar(property) => property.scalar,
            DocumentParameterTarget::Activation(_) => continue,
        };
        let fallback = retained
            .scalar(scalar)
            .ok_or(CoordinatorError::InvalidActionInput(
                "host parameter targets a missing fallback scalar",
            ))?
            .value;
        if promoted
            .scalar(scalar)
            .is_some_and(|value| value.value.to_bits() == fallback.to_bits())
        {
            continue;
        }
        if fallbacks
            .insert(scalar, fallback)
            .is_some_and(|previous| previous.to_bits() != fallback.to_bits())
        {
            return Err(CoordinatorError::InvalidActionInput(
                "host bindings disagree on one retained fallback scalar",
            ));
        }
    }
    if !fallbacks.is_empty() {
        let edits = fallbacks
            .into_iter()
            .map(|(scalar, value)| ScalarValueEdit::new(scalar, value))
            .collect::<Vec<_>>();
        promoted.set_scalar_values(&edits)?;
    }
    Ok(())
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

fn current_accepted_document(
    checkpoint: &RestoreCheckpoint,
) -> Result<SketchDocument, CoordinatorError> {
    if !checkpoint.accepted_belongs_to_current_design() {
        return Err(CoordinatorError::DirectManipulationColdReproductionMismatch);
    }
    checkpoint_document_from_json(
        checkpoint
            .accepted_json()
            .ok_or(CoordinatorError::DirectManipulationColdReproductionRejected)?,
        checkpoint.accepted_uses_draft_v5(),
    )
    .map_err(CoordinatorError::from)
}

pub(super) fn restore_sketch_checkpoint(
    current: &RetainedSketchDocumentSession,
    checkpoint: &RestoreCheckpoint,
    revisions: SketchLifecycleRevisionHighWater,
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
    let snapshots = current.latest_attempt_external_snapshot_set().clone();
    let mut restored = if let Some(json) = &checkpoint.accepted_json {
        let mut accepted = checkpoint_document_from_json(json, checkpoint.accepted_is_draft_v5)?;
        accepted.retain_persistent_identity_high_water(&high_water)?;
        if checkpoint.accepted_belongs_to_current_design {
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
        }?
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

/// Rebuilds one retained lineage history position without trusting the flat
/// accepted-document bytes embedded in any materialization checkpoint. The
/// current program is solved under the host's current inputs; if it rejects,
/// the lineage session's independently authenticated last-accepted program is
/// solved under its exact historical inputs and retained as distinct visible
/// authority.
#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed restore pipeline keeps lifecycle high-water, exact current authority, ordinary solve and historical fallback adjacent"
)]
pub(super) fn restore_sketch_checkpoint_from_lineage(
    current: &RetainedSketchDocumentSession,
    lineage: &CoordinatorLineage,
    checkpoint: &RestoreCheckpoint,
    revisions: SketchLifecycleRevisionHighWater,
    parameters: &geosolve_sketch::ParameterBatch,
    snapshots: &geosolve_sketch::ExternalSnapshotSet,
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
    let parameters = parameters.clone();
    let snapshots = snapshots.clone();
    let solver_config = input.solver_config();
    if let Some(evaluation) =
        lineage.reproduce_current_accepted_evaluation(&parameters, &snapshots)?
    {
        let mut accepted = checkpoint_document_from_json(
            evaluation.accepted_sketch_json(),
            evaluation.accepted_uses_draft_v5(),
        )?;
        accepted.retain_persistent_identity_high_water(&high_water)?;
        let revisions_already_retained =
            lifecycle_high_water_covers(current.revision_high_water(), revisions);
        if revisions_already_retained
            && current
                .accepted_state_for_current_input()
                .is_some_and(|current_accepted| {
                    current.design_document() == &design && current_accepted.document() == &accepted
                })
        {
            // Feature-only history keeps exact sketch lifecycle identities,
            // but only after the lineage owner has freshly authenticated the
            // complete accepted program and its exact host inputs above.
            let mut unchanged = current.clone();
            unchanged.retain_persistent_identity_high_water(&high_water)?;
            return Ok(unchanged);
        }
        let mut restored =
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                design,
                accepted,
                revisions,
                parameters,
                snapshots,
                request,
                solver_config,
            )?;
        restored.retain_persistent_identity_high_water(&high_water)?;
        return Ok(restored);
    }
    let exact = RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
        design.clone(),
        design.clone(),
        revisions,
        parameters.clone(),
        snapshots.clone(),
        request,
        solver_config,
    );
    let mut evaluated = exact.or_else(|_| {
        // Many ordinary retained programs intentionally store unsolved
        // authoring seeds. If exact no-optimization certification rejects,
        // solve those seeds normally. Direct-manipulation lineage instead
        // stores a complete accepted projection, so it takes the exact
        // branch above and cannot drift by a later solver ULP.
        RetainedSketchDocumentSession::restore_design_with_inputs(
            design.clone(),
            revisions,
            parameters.clone(),
            snapshots.clone(),
            request,
            solver_config,
        )
    })?;
    if let Some(evaluated_accepted) = evaluated.accepted_state_for_current_input() {
        let revisions_already_retained =
            lifecycle_high_water_covers(current.revision_high_water(), revisions);
        if revisions_already_retained
            && current
                .accepted_state_for_current_input()
                .is_some_and(|accepted| {
                    current.design_document() == evaluated.design_document()
                        && accepted.document() == evaluated_accepted.document()
                })
        {
            // Feature-only history is entitled to preserve exact sketch
            // identities and audit rows, but only after a fresh cold solve has
            // authenticated both retained and accepted documents. A forged
            // historical accepted cache therefore cannot enter this shortcut.
            let mut unchanged = current.clone();
            unchanged.retain_persistent_identity_high_water(&high_water)?;
            return Ok(unchanged);
        }
        evaluated.retain_persistent_identity_high_water(&high_water)?;
        return Ok(evaluated);
    }

    let Some((
        _accepted_checkpoint,
        accepted_inputs,
        _embedded_accepted_baseline,
        accepted_evidence,
    )) = lineage.materialize_last_accepted_with_inputs()?
    else {
        evaluated.retain_persistent_identity_high_water(&high_water)?;
        return Ok(evaluated);
    };
    lineage.validate_accepted_external_inputs(
        Some(accepted_inputs.parameters()),
        Some(accepted_inputs.snapshots()),
    )?;
    let mut accepted_design = checkpoint_document_from_json(
        accepted_evidence.accepted_sketch_json(),
        accepted_evidence.accepted_uses_draft_v5(),
    )?;
    accepted_design.retain_persistent_identity_high_water(&high_water)?;
    let mut restored =
        RetainedSketchDocumentSession::restore_design_with_accepted_and_distinct_inputs(
            design,
            accepted_design,
            revisions,
            parameters,
            snapshots,
            accepted_inputs.parameters().clone(),
            accepted_inputs.snapshots().clone(),
            request,
            solver_config,
        )?;
    restored.retain_persistent_identity_high_water(&high_water)?;
    Ok(restored)
}

fn lifecycle_high_water_covers(
    current: SketchLifecycleRevisionHighWater,
    required: SketchLifecycleRevisionHighWater,
) -> bool {
    current.design().get() >= required.design().get()
        && current.attempt().get() >= required.attempt().get()
        && match (current.accepted(), required.accepted()) {
            (_, None) => true,
            (Some(current), Some(required)) => current.get() >= required.get(),
            (None, Some(_)) => false,
        }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{DocumentParameterKind, DocumentParameterTarget, SketchDocument};

    use super::{dimension_target_scalar, preserve_host_parameter_fallbacks};

    #[test]
    fn direct_promotion_preserves_driving_dimension_fallback() {
        let mut retained = SketchDocument::new(8.0).expect("document");
        let rectangle = retained
            .add_rectangle("host-sized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = retained
            .add_parameter("host width", DocumentParameterKind::Length)
            .expect("parameter");
        retained
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("binding");
        let target = dimension_target_scalar(
            &retained
                .dimension(rectangle.dimensions[0])
                .expect("dimension")
                .definition,
        );
        let fallback = retained.scalar(target).expect("fallback scalar").value;
        let mut promoted = retained.clone();
        promoted
            .set_scalar_value(target, fallback + 2.0)
            .expect("effective host value");

        preserve_host_parameter_fallbacks(&retained, &mut promoted)
            .expect("preserve local fallback");

        assert_eq!(
            promoted
                .scalar(target)
                .expect("promoted scalar")
                .value
                .to_bits(),
            fallback.to_bits(),
            "accepted host input must not become retained authored intent"
        );
    }
}
