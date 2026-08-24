// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless interaction adapter for projectional design intent.
//!
//! This adapter deliberately owns no [`RetainedEditorCoordinator`]. Durable
//! geometry, accepted-scene publication and Undo/Redo remain exclusively in
//! [`ProjectionalIntentCoordinator`]; [`ConstraintEditor`] contributes only
//! disposable selection, hover and pointer-gesture state.

use geosolve_sketch::{
    DesignPointId, DocumentCurveControlId, DocumentDimensionDefinition, DocumentDimensionId,
    DocumentId, OperationControl, PreparedSketchInput, RetainedSketchDocumentSession,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchDesignIdentity, SketchHardValidity,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureAuthoringSnapshot, ComputedFeatureDefinition,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
};
use geosolve_sketch_intent::{
    IntentKey, IntentPatch, IntentPlanDisposition, IntentSession, IntentSessionIdentity, NodeId,
};
use thiserror::Error;

use crate::intent_bootstrap::{
    decode_flat_intent_bootstrap_prefix, flat_intent_bootstrap_prefix_materialization_map,
};
use crate::{
    AuthoringApplication, AuthoringState, ColdIntentMaterialization, ColdIntentMaterializer,
    ConstraintEditor, EditorEffect, EditorError, EditorScene, FeatureAuthoringCandidate,
    IntentBootstrapError, IntentInspectorEditError, IntentInspectorEditTarget,
    IntentInspectorEditValue, IntentInspectorProjection, IntentSourceEditError,
    IntentSourceTokenId, IntentValidationEvidence, IntentWorkbenchProjection, Modifiers,
    OffsetAuthoringState, PickTolerance, PointerInput, ProfileOffsetDirectionState,
    ProjectionalAuthoringError, ProjectionalCoordinatorError, ProjectionalFilletAuthoringError,
    ProjectionalIntentCoordinator, ProjectionalPatchOutcome, ProjectionalProfileOffsetError,
    SelectionItem, Viewport, decode_flat_intent_bootstrap,
    flat_intent_bootstrap_materialization_map, projectional_application_patch,
    projectional_construction_patch, projectional_fillet_patch, projectional_fillet_radius_patch,
    projectional_profile_offset_delete_patch, projectional_profile_offset_direction_patch,
    projectional_profile_offset_distance_patch, projectional_profile_offset_patch,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActivePointDrag {
    pointer_id: u64,
    point: DesignPointId,
    latest_request_id: Option<u64>,
    latest_position: Option<[f64; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveCurveControlDrag {
    pointer_id: u64,
    expected: SketchDesignIdentity,
    control: DocumentCurveControlId,
    latest_request_id: Option<u64>,
    latest_position: Option<[f64; 2]>,
}

#[derive(Debug)]
struct ActiveFilletRadiusDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    feature: geosolve_sketch_features::ComputedFeatureId,
    latest_radius: Option<f64>,
    latest: Option<ColdIntentMaterialization>,
}

#[derive(Debug)]
struct ActiveProfileOffsetDistanceDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    expected: PreparedSketchInput,
    dimension: DocumentDimensionId,
    latest_distance: Option<f64>,
    latest: Option<ColdIntentMaterialization>,
}

/// Result of one terminal pointer sample.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionalEditorPointerOutcome {
    /// Presentation-only effects already reflected in the embedded editor.
    pub effects: Vec<EditorEffect>,
    /// The sole durable transaction, present only for an accepted moved drag.
    pub transaction: Option<ProjectionalPatchOutcome>,
}

/// Result of one authenticated terminal geometry-authoring publication.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionalEditorConstructionOutcome {
    /// Draft/inference presentation effects emitted by the accepted acknowledgement.
    pub effects: Vec<EditorEffect>,
    /// The one durable transaction appended to the sole intent history.
    pub transaction: ProjectionalPatchOutcome,
}

/// Projectional intent plus disposable headless interaction state.
///
/// Pointer motion solves against retained native continuation owned by the
/// projectional coordinator. It does not serialize or replay intent and cannot
/// append history. A matching pointer-up converts the newest accepted sample
/// into exactly one instance patch.
#[derive(Debug)]
pub struct ProjectionalEditorSession {
    coordinator: ProjectionalIntentCoordinator,
    editor: ConstraintEditor,
    selected_declaration: Option<NodeId>,
    point_drag: Option<ActivePointDrag>,
    curve_control_drag: Option<ActiveCurveControlDrag>,
    fillet_radius_drag: Option<ActiveFilletRadiusDrag>,
    profile_offset_distance_drag: Option<ActiveProfileOffsetDistanceDrag>,
    preview_control: OperationControl,
}

impl ProjectionalEditorSession {
    /// Creates a headless projectional interaction session.
    #[must_use]
    pub fn new(coordinator: ProjectionalIntentCoordinator) -> Self {
        Self::with_editor_and_control(
            coordinator,
            ConstraintEditor::default(),
            crate::coordinator::bounded_geometry_control(),
        )
    }

    /// Restores canonical intent through an independently cold-rebuilt native
    /// authority and installs no second durable coordinator or history.
    ///
    /// # Errors
    ///
    /// Returns a typed materializer/session/authentication failure. Persisted
    /// flat geometry is intentionally not accepted through this constructor.
    pub fn restore(
        intent: IntentSession,
        document: DocumentId,
        model_scale: f64,
    ) -> Result<Self, ProjectionalEditorError> {
        let materializer = ColdIntentMaterializer::with_default_policy(document, model_scale)
            .map_err(ProjectionalCoordinatorError::from)?;
        Ok(Self::new(ProjectionalIntentCoordinator::restore(
            intent,
            materializer,
        )?))
    }

    /// Activates one strictly decoded, history-free flat bootstrap over its
    /// already restored and independently accepted native scene.
    ///
    /// This migration seam intentionally accepts only a current native solve
    /// whose per-object sketch and computed-feature declarations authenticate
    /// the complete restored sidecar. Later ordinary declarations use the
    /// separate cold bootstrap-prefix path; failing closed here prevents a
    /// legacy scene from being installed with incomplete visual or ownership
    /// authority.
    ///
    /// # Errors
    ///
    /// Returns a typed bootstrap, native-authority, computed-feature, or
    /// independent-validation mismatch without installing partial state.
    pub fn restore_native_bootstrap(
        intent: IntentSession,
        native: RetainedSketchDocumentSession,
    ) -> Result<Self, ProjectionalEditorError> {
        let decoded = decode_flat_intent_bootstrap(&intent)?;
        if native.design_document() != &decoded.document {
            return Err(ProjectionalEditorError::BootstrapDocumentMismatch);
        }
        let accepted = native
            .accepted_state_for_current_input()
            .ok_or(ProjectionalEditorError::BootstrapCurrentAcceptanceRequired)?;
        let solve = accepted
            .diagnostics()
            .solve
            .ok_or(ProjectionalEditorError::BootstrapValidationMissing)?;
        if !solve.accepted
            || solve.hard_validity != SketchHardValidity::Valid
            || !solve.hard_residuals_validated
            || solve.maximum_normalized_hard_residual.is_some_and(|value| {
                !value.is_finite() || value > SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            })
        {
            return Err(ProjectionalEditorError::BootstrapValidationRejected);
        }
        let authority = intent
            .accepted()
            .ok_or(ProjectionalEditorError::BootstrapCurrentAcceptanceRequired)?;
        if authority.evidence.materialization
            != accepted
                .document()
                .to_draft_v5_json()
                .map_err(ProjectionalCoordinatorError::from)?
                .into_bytes()
        {
            return Err(ProjectionalEditorError::BootstrapDocumentMismatch);
        }
        let semantic = authority.target;
        let mut ownership = flat_intent_bootstrap_materialization_map(&intent)?;
        let computed = crate::intent_computed::materialize_computed_features(
            intent.graph(),
            decoded.document.id(),
            &native,
            &mut ownership,
            Some((&decoded.features, decoded.feature_lifecycle_high_water)),
        )
        .map_err(crate::IntentMaterializationError::from)
        .map_err(ProjectionalCoordinatorError::from)?;
        let materialization = ColdIntentMaterialization {
            session: native,
            features: computed.features.clone(),
            feature_lifecycle_high_water: computed.feature_lifecycle_high_water,
            computed: computed.snapshot.clone(),
            computed_evaluation_high_water: computed.evaluation_high_water,
            ownership,
            validation: IntentValidationEvidence {
                semantic,
                document: decoded.document.id(),
                point_count: decoded.document.points().len(),
                curve_count: decoded.document.curves().len(),
                constraint_count: decoded.document.constraints().len(),
                hard_residuals_validated: solve.hard_residuals_validated,
                maximum_normalized_hard_residual: solve.maximum_normalized_hard_residual,
                feature_document: computed.features.id(),
                feature_revision: computed.features.revision(),
                feature_digest: computed.features.digest(),
                feature_count: computed.features.features().len(),
                computed_edge_count: computed.snapshot.edges().len(),
                all_active_features_current: true,
            },
            evidence: authority.evidence.clone(),
        };
        let materializer = ColdIntentMaterializer::with_default_policy(
            decoded.document.id(),
            decoded.document.model_scale(),
        )
        .and_then(|materializer| {
            materializer.with_authenticated_bootstrap(&decoded, materialization.ownership.clone())
        })
        .map_err(ProjectionalCoordinatorError::from)?;
        Ok(Self::new(
            ProjectionalIntentCoordinator::restore_authenticated_bootstrap(
                intent,
                materializer,
                materialization,
            )?,
        ))
    }

    /// Cold-restores a canonical workspace whose immutable historical
    /// bootstrap prefix is followed by ordinary projectional declarations.
    ///
    /// Stored flat scene bytes are not installed. They remain host comparison
    /// evidence while the returned coordinator independently reconstructs its
    /// accepted authority from the typed prefix and declaration DAG.
    ///
    /// # Errors
    ///
    /// Returns a strict prefix, seed-authentication, cold-materialization, or
    /// accepted-evidence mismatch without installing partial state.
    pub fn restore_with_bootstrap_prefix(
        intent: IntentSession,
    ) -> Result<Self, ProjectionalEditorError> {
        let decoded = decode_flat_intent_bootstrap_prefix(&intent)?;
        let ownership = flat_intent_bootstrap_prefix_materialization_map(&intent)?;
        let materializer = ColdIntentMaterializer::with_default_policy(
            decoded.document.id(),
            decoded.document.model_scale(),
        )
        .and_then(|materializer| materializer.with_authenticated_bootstrap(&decoded, ownership))
        .map_err(ProjectionalCoordinatorError::from)?;
        Ok(Self::new(ProjectionalIntentCoordinator::restore(
            intent,
            materializer,
        )?))
    }

    /// Creates a session with explicit transient editor and solve-work policy.
    #[must_use]
    pub const fn with_editor_and_control(
        coordinator: ProjectionalIntentCoordinator,
        editor: ConstraintEditor,
        preview_control: OperationControl,
    ) -> Self {
        Self {
            coordinator,
            editor,
            selected_declaration: None,
            point_drag: None,
            curve_control_drag: None,
            fillet_radius_drag: None,
            profile_offset_distance_drag: None,
            preview_control,
        }
    }

    /// Canonical durable authority.
    #[must_use]
    pub const fn coordinator(&self) -> &ProjectionalIntentCoordinator {
        &self.coordinator
    }

    /// Disposable selection, hover and gesture state.
    #[must_use]
    pub const fn editor(&self) -> &ConstraintEditor {
        &self.editor
    }

    /// Mutable disposable interaction state for presentation adapters.
    ///
    /// Durable effects emitted here must still return through the typed methods
    /// on this session; this accessor grants no document or history authority.
    pub fn editor_mut(&mut self) -> &mut ConstraintEditor {
        &mut self.editor
    }

    /// Builds the durable editor-owned Outline/source/History projection.
    ///
    /// Pointer-frame methods never call this function; presentation adapters
    /// rebuild these DTOs only at a durable render boundary.
    #[must_use]
    pub fn workbench_projection(&self) -> IntentWorkbenchProjection {
        IntentWorkbenchProjection::from_session(self.coordinator.intent())
    }

    /// Currently selected logical declaration, independent of canvas picks.
    #[must_use]
    pub const fn selected_declaration(&self) -> Option<NodeId> {
        self.selected_declaration
    }

    /// Selects one stable declaration for Outline/source/Inspector projection.
    ///
    /// Missing or deleted identities clear the logical selection. This is
    /// presentation state and never appends intent history.
    pub fn set_selected_declaration(&mut self, node: Option<NodeId>) -> bool {
        self.selected_declaration =
            node.filter(|node| self.coordinator.intent().graph().node(*node).is_some());
        self.selected_declaration.is_some() == node.is_some()
    }

    /// Builds the schema-derived Inspector for the selected declaration.
    #[must_use]
    pub fn selected_inspector(
        &self,
        projection: &IntentWorkbenchProjection,
    ) -> Option<IntentInspectorProjection> {
        projection.inspector(self.coordinator.intent(), self.selected_declaration?)
    }

    /// Builds the currently presentable native scene.
    ///
    /// A valid point preview temporarily outranks the durable accepted scene.
    /// Retained invalid intent continues to display the prior accepted scene.
    ///
    /// # Errors
    ///
    /// Returns a typed no-authority or scene-composition failure.
    pub fn scene(
        &self,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<EditorScene, ProjectionalEditorError> {
        let accepted_materialization = self
            .coordinator
            .accepted_materialization()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let property_preview = self
            .fillet_radius_drag
            .as_ref()
            .and_then(|drag| drag.latest.as_ref())
            .or_else(|| {
                self.profile_offset_distance_drag
                    .as_ref()
                    .and_then(|drag| drag.latest.as_ref())
            });
        let materialization = property_preview.unwrap_or(accepted_materialization);
        let session = property_preview
            .map_or_else(
                || self.coordinator.presentation_session(),
                |preview| Some(&preview.session),
            )
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let accepted_input = session
            .accepted_prepared_input()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let mut transient_computed = None;
        let computed = if materialization.computed.input().sketch == accepted_input {
            &materialization.computed
        } else {
            let mut allocator = ComputedEvaluationAllocator::from_high_water(
                materialization.computed_evaluation_high_water,
            );
            let outcome = ComputedFeatureEvaluationSnapshot::capture(
                session,
                &materialization.features,
                ComputedFeatureEvaluationPolicy::default(),
            )
            .map_err(|error| ProjectionalEditorError::ComputedScene(error.to_string()))?
            .prepare(&mut allocator)
            .map_err(|error| ProjectionalEditorError::ComputedScene(error.to_string()))?
            .execute(OperationControl::unlimited())
            .map_err(|error| ProjectionalEditorError::ComputedScene(error.to_string()))?;
            let geosolve_sketch::OperationOutcome::Completed {
                value: computed, ..
            } = outcome
            else {
                return Err(ProjectionalEditorError::ComputedSceneStopped);
            };
            transient_computed.insert(computed)
        };
        let mut scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            &accepted_input,
            &computed.input(),
            computed,
            viewport,
            chord_tolerance_pixels,
        )?;
        if !scene.update_annotation_values(accepted) {
            return Err(ProjectionalEditorError::SceneAuthorityMismatch);
        }
        scene.apply_annotation_layout(&self.editor.annotation_layout_for_scene());
        let mut scene = scene.with_retained_session(session)?;
        self.editor.populate_curve_controls(&mut scene)?;
        if property_preview.is_none() {
            self.attach_computed_fillet_radius_rails(&mut scene, session, materialization)?;
        }
        if property_preview.is_some()
            && let Some(drag) = self.fillet_radius_drag.as_ref()
        {
            scene.set_computed_fillet_interaction_origin(drag.expected)?;
        }
        if property_preview.is_some()
            && let Some(drag) = self.profile_offset_distance_drag.as_ref()
        {
            scene.set_accepted_offset_distance_interaction_origin(&drag.expected)?;
        }
        if let Some((accepted_revision, expected, request_id, model_position)) =
            self.coordinator.curve_control_preview_origin()
        {
            scene.set_curve_control_interaction_origin(
                accepted_revision,
                expected,
                request_id,
                model_position,
            );
        }
        Ok(scene)
    }

    fn attach_computed_fillet_radius_rails(
        &self,
        scene: &mut EditorScene,
        session: &RetainedSketchDocumentSession,
        materialization: &ColdIntentMaterialization,
    ) -> Result<(), ProjectionalEditorError> {
        if materialization.features.features().is_empty() {
            return Ok(());
        }
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(session)
            .map_err(|error| ProjectionalEditorError::ComputedScene(error.to_string()))?;
        for feature in materialization.features.features() {
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
            let mut affected_owners = fillet
                .corners
                .iter()
                .map(|corner| geosolve_sketch_features::ComputedCornerRef {
                    feature: feature.id,
                    corner: corner.id,
                })
                .collect::<Vec<_>>();
            affected_owners.sort_unstable();
            for corner in &fillet.corners {
                let outcome = snapshot.continue_fillet_corner(
                    corner.without_id(),
                    fillet.radius,
                    fillet.radius,
                    ComputedFeatureEvaluationPolicy::default(),
                    self.preview_control.clone(),
                );
                let Ok(geosolve_sketch::OperationOutcome::Completed {
                    value: continuation,
                    ..
                }) = outcome
                else {
                    continue;
                };
                scene.attach_computed_fillet_radius_rail(
                    geosolve_sketch_features::ComputedCornerRef {
                        feature: feature.id,
                        corner: corner.id,
                    },
                    continuation.sensitivity.center_derivative,
                    affected_owners.clone(),
                )?;
            }
        }
        Ok(())
    }

    /// Applies a typed durable patch through the sole intent history.
    ///
    /// # Errors
    ///
    /// Returns the ordinary projectional planning/materialization error.
    pub fn apply_patch(
        &mut self,
        patch: IntentPatch,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        self.cancel_interaction();
        let outcome = self.coordinator.apply_patch(patch)?;
        if outcome.disposition == IntentPlanDisposition::Accepted {
            self.clear_transient_selection();
        }
        self.reconcile_declaration_selection();
        Ok(outcome)
    }

    /// Publishes one grouped computed-Fillet candidate as a single typed
    /// declaration through the sole intent history.
    ///
    /// # Errors
    ///
    /// Rejects stale preview stamps, incomplete logical ownership, invalid
    /// branch metadata, or ordinary planning/materialization failures.
    pub fn apply_computed_fillet(
        &mut self,
        symbol: IntentKey,
        candidate: &FeatureAuthoringCandidate,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            let accepted_state = accepted
                .session
                .accepted_state_for_current_input()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            let accepted_input = accepted
                .session
                .accepted_prepared_input()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_fillet_patch(
                self.coordinator.intent().identity(),
                self.coordinator.intent(),
                &accepted.ownership,
                accepted_input,
                accepted_state.identity(),
                symbol,
                candidate,
            )?
            .patch
        };
        self.apply_patch(patch)
    }

    /// Edits one stable computed-Fillet radius. A geometrically invalid value
    /// remains as retained intent over the prior accepted scene, matching the
    /// projectional Inspector contract.
    ///
    /// # Errors
    ///
    /// Rejects stale ownership, nonpositive/nonfinite input, a non-Fillet
    /// feature, or ordinary projectional patch failures.
    pub fn edit_computed_fillet_radius(
        &mut self,
        feature: geosolve_sketch_features::ComputedFeatureId,
        radius: f64,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_fillet_radius_patch(
                self.coordinator.intent(),
                &accepted.ownership,
                feature,
                radius,
            )?
        };
        self.apply_patch(patch)
    }

    /// Publishes one complete topology-authenticated native Profile Offset as
    /// aggregate operands plus one operation declaration through the sole
    /// intent history.
    ///
    /// Native operation preparation authenticates the exact output inventory
    /// before any intent reservation is created. A successful publication
    /// clears the consumed operand while retaining the collector's distance
    /// memory; the host may reactivate it with a fresh topology index.
    ///
    /// # Errors
    ///
    /// Rejects stale/incomplete collection, topology or native preparation,
    /// missing logical source ownership, or ordinary projectional publication
    /// failure without changing the collector or history.
    pub fn apply_profile_offset(
        &mut self,
        state: &mut OffsetAuthoringState,
        symbol: IntentKey,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        self.cancel_interaction();
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_profile_offset_patch(
                self.coordinator.intent().identity(),
                self.coordinator.intent(),
                &accepted.ownership,
                &accepted.session,
                state,
                symbol,
            )?
            .patch
        };
        let outcome = self.apply_patch(patch)?;
        if outcome.disposition == IntentPlanDisposition::Accepted {
            state.clear_after_apply();
        }
        Ok(outcome)
    }

    /// Edits the positive distance property owned by one accepted native
    /// Profile Offset declaration.
    ///
    /// # Errors
    ///
    /// Rejects invalid distance, stale/ambiguous ownership, a non-Offset
    /// declaration, or ordinary projectional publication failure.
    pub fn edit_profile_offset_distance(
        &mut self,
        dimension: DocumentDimensionId,
        distance: f64,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_profile_offset_distance_patch(
                self.coordinator.intent(),
                &accepted.ownership,
                dimension,
                distance,
            )?
        };
        self.apply_patch(patch)
    }

    /// Edits the explicit direction property owned by one accepted native
    /// Profile Offset declaration without changing operand topology.
    ///
    /// # Errors
    ///
    /// Rejects a face/chain direction-family mismatch, stale ownership, or
    /// ordinary projectional publication failure.
    pub fn edit_profile_offset_direction(
        &mut self,
        dimension: DocumentDimensionId,
        direction: ProfileOffsetDirectionState,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_profile_offset_direction_patch(
                self.coordinator.intent(),
                &accepted.ownership,
                dimension,
                direction,
            )?
        };
        self.apply_patch(patch)
    }

    /// Deletes the operation which owns one accepted native Profile Offset
    /// dimension plus its exact downstream dependency closure in one history
    /// transaction.
    ///
    /// # Errors
    ///
    /// Rejects stale/ambiguous ownership or a changed dependency closure
    /// without publishing a partial deletion.
    pub fn delete_profile_offset(
        &mut self,
        dimension: DocumentDimensionId,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_profile_offset_delete_patch(
                self.coordinator.intent(),
                &accepted.ownership,
                dimension,
            )?
        };
        self.apply_patch(patch)
    }

    /// Applies one complete contextual relation or dimension application
    /// through the sole typed intent history.
    ///
    /// Applicability and explicit branch defaults are resolved by the same
    /// native authoring owner as the flat coordinator. Operands are rebound
    /// only through the exact accepted reverse-ownership map, and a valid but
    /// unsolved declaration remains retained above the prior accepted scene.
    ///
    /// # Errors
    ///
    /// Returns a stale application/ownership, native authoring, planning,
    /// materialization or publication error without a partial transaction.
    pub fn apply_authoring_application(
        &mut self,
        application: &AuthoringApplication,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        self.cancel_interaction();
        let translated = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            let accepted_state = accepted
                .session
                .accepted_state_for_current_input()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            projectional_application_patch(
                self.coordinator.intent().identity(),
                self.coordinator.intent(),
                &accepted.ownership,
                accepted.session.design_document(),
                accepted_state.document(),
                application,
            )?
        };
        let outcome = self.coordinator.apply_patch(translated.patch)?;
        if outcome.disposition == IntentPlanDisposition::Accepted {
            self.clear_transient_selection();
        }
        self.reconcile_declaration_selection();
        Ok(outcome)
    }

    /// Applies one exact tokenized construction terminal through intent.
    ///
    /// The pending editor-owned token authenticates its prepared native input,
    /// complete plan, and exact geometry variant before translation. A valid
    /// plan is lowered into one typed intent patch and therefore one history
    /// entry. Translation, stale-authority, or materialization failure is
    /// acknowledged as rejected so the editor restores its correction-ready
    /// draft; a foreign or substituted envelope never consumes the genuine
    /// pending token.
    ///
    /// # Errors
    ///
    /// Returns a typed envelope, authority, translation, planning, solve, or
    /// publication failure. No accepted native scene or intent history changes
    /// when an error is returned.
    pub fn apply_construction_editor_effect(
        &mut self,
        effect: &EditorEffect,
    ) -> Result<ProjectionalEditorConstructionOutcome, ProjectionalEditorError> {
        let EditorEffect::CommitConstructionPlan {
            expected,
            token,
            plan,
        } = effect
        else {
            return Err(ProjectionalEditorError::UnexpectedConstructionEffect);
        };
        let Some(variant) =
            self.editor
                .authenticated_construction_commit_variant(*token, expected.as_ref(), plan)
        else {
            return Err(ProjectionalEditorError::ConstructionCommitMismatch);
        };

        let translated = {
            let Some(accepted) = self.coordinator.accepted_materialization() else {
                self.reject_construction_commit(*token);
                return Err(ProjectionalEditorError::ConstructionInputMismatch);
            };
            if accepted.session.accepted_prepared_input().as_ref() != Some(expected.as_ref()) {
                self.reject_construction_commit(*token);
                return Err(ProjectionalEditorError::ConstructionInputMismatch);
            }
            match projectional_construction_patch(
                self.coordinator.intent().identity(),
                self.coordinator.intent(),
                &accepted.ownership,
                variant,
                plan,
            ) {
                Ok(translated) => translated,
                Err(error) => {
                    self.reject_construction_commit(*token);
                    return Err(error.into());
                }
            }
        };

        // Stage the success acknowledgement before the durable commit. This
        // makes the remaining state swap infallible if native materialization
        // succeeds, while a materialization error can still reject the live
        // pending draft without having changed intent history.
        let mut accepted_editor = self.editor.clone();
        let effects = accepted_editor.acknowledge_construction_commit(*token, true);
        if accepted_editor
            .pending_construction_commit_token()
            .is_some()
        {
            return Err(ProjectionalEditorError::ConstructionAcknowledgementMismatch);
        }
        let transaction = match self.coordinator.apply_patch(translated.patch) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.reject_construction_commit(*token);
                return Err(error.into());
            }
        };
        self.editor = accepted_editor;
        self.point_drag = None;
        self.curve_control_drag = None;
        self.reconcile_declaration_selection();
        Ok(ProjectionalEditorConstructionOutcome {
            effects,
            transaction,
        })
    }

    /// Deletes one declaration plus its exact dependent closure through one
    /// intent transaction.
    ///
    /// # Errors
    ///
    /// Returns the ordinary dependency/materialization/publication error.
    pub fn delete_declaration(
        &mut self,
        node: NodeId,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        self.cancel_interaction();
        let outcome = self.coordinator.delete_declaration(node)?;
        self.clear_transient_selection();
        self.reconcile_declaration_selection();
        Ok(outcome)
    }

    /// Applies one recognized structured-source token as an ordinary typed
    /// exact-CAS patch. Source text is never executed.
    ///
    /// # Errors
    ///
    /// Returns a stale/invalid token or the ordinary projectional patch error.
    pub fn edit_source_token(
        &mut self,
        projection: &IntentWorkbenchProjection,
        token: IntentSourceTokenId,
        replacement: &str,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = projection.structured_source.patch_for_edit(
            self.coordinator.intent(),
            token,
            replacement,
        )?;
        self.apply_patch(patch)
    }

    /// Applies one schema-generated Inspector edit through the ordinary typed
    /// exact-CAS patch vocabulary and sole intent history.
    ///
    /// # Errors
    ///
    /// Returns a stale Inspector coordinate, a typed-value mismatch, or the
    /// ordinary projectional planning/materialization error.
    pub fn edit_inspector(
        &mut self,
        inspector: &IntentInspectorProjection,
        target: &IntentInspectorEditTarget,
        value: IntentInspectorEditValue,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = inspector.patch_for_edit(self.coordinator.intent(), target, value)?;
        self.apply_patch(patch)
    }

    /// Steps the sole intent history backward.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-reconstruction error without changing state.
    pub fn undo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalEditorError> {
        self.cancel_interaction();
        let moved = self.coordinator.undo()?;
        if moved.is_some() {
            self.clear_transient_selection();
            self.reconcile_declaration_selection();
        }
        Ok(moved)
    }

    /// Steps the sole intent history forward.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-reconstruction error without changing state.
    pub fn redo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalEditorError> {
        self.cancel_interaction();
        let moved = self.coordinator.redo()?;
        if moved.is_some() {
            self.clear_transient_selection();
            self.reconcile_declaration_selection();
        }
        Ok(moved)
    }

    /// Replaces transient selection without touching intent or history.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        self.editor.set_selection(selection);
    }

    /// Applies one transient selection click without touching intent or history.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: Modifiers) {
        self.editor.select_item(item, modifiers);
    }

    /// Publishes the exact compatible native item that the current relation or
    /// dimension authoring state would consume on pointer-down.
    ///
    /// This is presentation-only hover state. It reads the same currently
    /// presentable accepted document and geometry-interaction policy as the
    /// flat coordinator, and it never changes intent, accepted evidence, or
    /// history.
    pub fn pointer_move_authoring(
        &mut self,
        state: &AuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        tolerance: PickTolerance,
    ) -> Vec<EditorEffect> {
        let target = self.coordinator.presentation_session().and_then(|session| {
            state.hover_item_at_with_policy(
                session.design_document(),
                scene,
                input.position,
                tolerance,
                self.editor.geometry_interaction_policy(),
            )
        });
        self.editor.set_authoring_hover_target(target)
    }

    /// Starts one headless pointer gesture. Selection changes are transient.
    ///
    /// # Errors
    ///
    /// Returns a typed ownership/continuation error when the selected point is
    /// not a writable intent output. The rejected route is cancelled before a
    /// move frame can request native work.
    #[allow(
        clippy::too_many_lines,
        reason = "one pointer-down authentication table keeps all mutually exclusive projectional mutation routes explicit"
    )]
    pub fn pointer_down(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let effects = self
            .editor
            .pointer_down_accepted_offset_distance(scene, input)
            .unwrap_or_else(|| self.editor.pointer_down(scene, input));
        let point_route = self.editor.prepared_point_drag_route();
        let curve_route = self.editor.prepared_curve_control_drag_route();
        let fillet_route = self.editor.prepared_feature_radius_drag_route();
        let offset_route = self.editor.prepared_accepted_offset_distance_drag_route();
        let route_count = [
            point_route.is_some(),
            curve_route.is_some(),
            fillet_route.is_some(),
            offset_route.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if route_count > 1 {
            self.cancel_direct_manipulation();
            let _ = self.editor.cancel();
            return Err(ProjectionalEditorError::AmbiguousDirectManipulationRoute);
        }
        if let Some(route) = point_route {
            if self.curve_control_drag.is_some() {
                self.cancel_direct_manipulation();
                let _ = self.editor.cancel();
                return Err(ProjectionalEditorError::PointDragRouteMismatch);
            }
            if let Some(active) = self.point_drag {
                if active.pointer_id != route.pointer_id || active.point != route.point {
                    self.cancel_point_drag();
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::PointDragRouteMismatch);
                }
            } else {
                if let Err(error) = self
                    .coordinator
                    .begin_point_drag(route.pointer_id, route.point)
                {
                    let _ = self.editor.cancel();
                    return Err(error.into());
                }
                self.point_drag = Some(ActivePointDrag {
                    pointer_id: route.pointer_id,
                    point: route.point,
                    latest_request_id: None,
                    latest_position: None,
                });
            }
        } else if let Some(route) = curve_route {
            if self.point_drag.is_some() {
                self.cancel_direct_manipulation();
                let _ = self.editor.cancel();
                return Err(ProjectionalEditorError::CurveControlDragRouteMismatch);
            }
            if let Some(active) = self.curve_control_drag {
                if active.pointer_id != route.pointer_id
                    || active.expected != route.expected
                    || active.control != route.control
                {
                    self.cancel_curve_control_drag();
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::CurveControlDragRouteMismatch);
                }
            } else {
                if let Err(error) = self.coordinator.begin_curve_control_drag(
                    route.pointer_id,
                    route.accepted_revision,
                    route.expected,
                    route.control,
                ) {
                    let _ = self.editor.cancel();
                    return Err(error.into());
                }
                self.curve_control_drag = Some(ActiveCurveControlDrag {
                    pointer_id: route.pointer_id,
                    expected: route.expected,
                    control: route.control,
                    latest_request_id: None,
                    latest_position: None,
                });
            }
        } else if let Some(route) = fillet_route {
            if let Some(active) = self.fillet_radius_drag.as_ref() {
                if active.pointer_id != route.pointer_id
                    || active.expected != route.expected
                    || active.feature != route.feature
                {
                    self.cancel_fillet_radius_drag();
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
                }
            } else {
                let accepted = self
                    .coordinator
                    .accepted_materialization()
                    .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
                let origin_radius = accepted
                    .features
                    .feature(route.feature)
                    .map(|feature| match &feature.definition {
                        ComputedFeatureDefinition::FilletSet(fillet) => fillet.radius,
                    })
                    .filter(|radius| radius.to_bits() == route.origin_radius.to_bits());
                if accepted.computed.input() != route.expected || origin_radius.is_none() {
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
                }
                projectional_fillet_radius_patch(
                    self.coordinator.intent(),
                    &accepted.ownership,
                    route.feature,
                    route.origin_radius,
                )?;
                self.fillet_radius_drag = Some(ActiveFilletRadiusDrag {
                    pointer_id: route.pointer_id,
                    intent: self.coordinator.intent().identity(),
                    expected: route.expected,
                    feature: route.feature,
                    latest_radius: None,
                    latest: None,
                });
            }
        } else if let Some(route) = offset_route {
            if let Some(active) = self.profile_offset_distance_drag.as_ref() {
                if active.pointer_id != route.pointer_id
                    || active.expected != route.expected
                    || active.dimension != route.dimension
                {
                    self.cancel_profile_offset_distance_drag();
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch);
                }
            } else {
                let accepted = self
                    .coordinator
                    .accepted_materialization()
                    .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
                let current_input = accepted
                    .session
                    .accepted_prepared_input()
                    .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
                let origin_distance = accepted
                    .session
                    .design_document()
                    .dimension(route.dimension)
                    .and_then(|dimension| match dimension.definition {
                        DocumentDimensionDefinition::ProfileOffset { target, .. } => accepted
                            .session
                            .design_document()
                            .scalar(target)
                            .map(|scalar| scalar.value),
                        _ => None,
                    })
                    .filter(|distance| distance.to_bits() == route.origin_distance.to_bits());
                if current_input != route.expected || origin_distance.is_none() {
                    let _ = self.editor.cancel();
                    return Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch);
                }
                projectional_profile_offset_distance_patch(
                    self.coordinator.intent(),
                    &accepted.ownership,
                    route.dimension,
                    route.origin_distance,
                )?;
                self.profile_offset_distance_drag = Some(ActiveProfileOffsetDistanceDrag {
                    pointer_id: route.pointer_id,
                    intent: self.coordinator.intent().identity(),
                    expected: route.expected,
                    dimension: route.dimension,
                    latest_distance: None,
                    latest: None,
                });
            }
        } else {
            self.cancel_direct_manipulation();
        }
        Ok(effects)
    }

    /// Resolves one pointer frame through native retained continuation.
    ///
    /// The returned effects are presentation signals only. The intent identity
    /// and history are unchanged for every successful or rejected frame.
    ///
    /// # Errors
    ///
    /// Returns a typed route or native preview failure.
    pub fn pointer_move(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let effects = self.editor.pointer_move(scene, input);
        self.resolve_pointer_frame(effects)
    }

    /// Finishes a pointer gesture and publishes at most one intent transaction.
    ///
    /// # Errors
    ///
    /// Returns a typed stale-route, preview, or exact cold-reconstruction error.
    #[allow(
        clippy::too_many_lines,
        reason = "one terminal dispatcher proves every direct-manipulation route can publish at most one transaction"
    )]
    pub fn pointer_up(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<ProjectionalEditorPointerOutcome, ProjectionalEditorError> {
        if !input.position.x.is_finite() || !input.position.y.is_finite() {
            if self
                .editor
                .active_pointer_gesture()
                .is_some_and(|gesture| gesture.pointer_id == input.pointer_id)
            {
                self.cancel_direct_manipulation();
                return Ok(ProjectionalEditorPointerOutcome {
                    effects: self.editor.cancel(),
                    transaction: None,
                });
            }
            return Ok(ProjectionalEditorPointerOutcome {
                effects: Vec::new(),
                transaction: None,
            });
        }
        let expected = self
            .curve_control_drag
            .map_or(scene.design_identity, |drag| drag.expected);
        let effects = self.editor.pointer_up(scene, expected, input);
        let mut presentation = Vec::new();
        let mut transaction = None;
        for effect in effects {
            match effect {
                EditorEffect::CommitPointMove {
                    point,
                    model_position,
                    ..
                } => {
                    let Some(drag) = self.point_drag.take() else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingPointDragRoute);
                    };
                    if drag.pointer_id != input.pointer_id || drag.point != point {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::PointDragRouteMismatch);
                    }
                    let Some(request_id) = drag.latest_request_id else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingAcceptedPointSample);
                    };
                    let Some(accepted_position) = drag.latest_position else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingAcceptedPointSample);
                    };
                    if accepted_position.map(f64::to_bits) != model_position.map(f64::to_bits) {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::PointPreviewMismatch);
                    }
                    transaction = Some(
                        self.coordinator
                            .finish_point_drag(input.pointer_id, request_id)?,
                    );
                }
                EditorEffect::ClearPointPreview => {
                    self.cancel_point_drag();
                    presentation.push(EditorEffect::ClearPointPreview);
                }
                EditorEffect::CommitCurveControl {
                    expected,
                    pointer_id,
                    request_id,
                    control,
                } => {
                    let Some(drag) = self.curve_control_drag.take() else {
                        self.coordinator.cancel_curve_control_drag();
                        return Err(ProjectionalEditorError::MissingCurveControlDragRoute);
                    };
                    if drag.pointer_id != pointer_id
                        || drag.pointer_id != input.pointer_id
                        || drag.expected != expected
                        || drag.control != control
                    {
                        self.coordinator.cancel_curve_control_drag();
                        return Err(ProjectionalEditorError::CurveControlDragRouteMismatch);
                    }
                    if drag.latest_request_id != Some(request_id) || drag.latest_position.is_none()
                    {
                        self.coordinator.cancel_curve_control_drag();
                        return Err(ProjectionalEditorError::MissingAcceptedCurveControlSample);
                    }
                    transaction = self
                        .coordinator
                        .finish_curve_control_drag(pointer_id, request_id, expected, control)?;
                }
                EditorEffect::ClearCurveControlPreview => {
                    self.cancel_curve_control_drag();
                    presentation.push(EditorEffect::ClearCurveControlPreview);
                }
                EditorEffect::CommitComputedFeatureRadius {
                    expected,
                    feature,
                    radius,
                } => {
                    let Some(drag) = self.fillet_radius_drag.take() else {
                        return Err(ProjectionalEditorError::MissingFilletRadiusDragRoute);
                    };
                    let latest_input = drag.latest.as_ref().map(|latest| latest.computed.input());
                    if drag.pointer_id != input.pointer_id
                        || drag.intent != self.coordinator.intent().identity()
                        || drag.expected != expected
                        || drag.feature != feature
                        || drag
                            .latest_radius
                            .is_none_or(|accepted| accepted.to_bits() != radius.to_bits())
                        || drag.latest.is_none()
                        || scene.computed_input != latest_input
                    {
                        return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
                    }
                    transaction = Some(self.edit_computed_fillet_radius(feature, radius)?);
                }
                EditorEffect::ClearComputedFeaturePreview => {
                    self.cancel_fillet_radius_drag();
                    presentation.push(EditorEffect::ClearComputedFeaturePreview);
                }
                EditorEffect::CommitAcceptedProfileOffsetDistance {
                    expected,
                    dimension,
                    distance,
                } => {
                    let Some(drag) = self.profile_offset_distance_drag.take() else {
                        return Err(ProjectionalEditorError::MissingProfileOffsetDistanceDragRoute);
                    };
                    let latest_input = drag
                        .latest
                        .as_ref()
                        .and_then(|latest| latest.session.accepted_prepared_input());
                    if drag.pointer_id != input.pointer_id
                        || drag.intent != self.coordinator.intent().identity()
                        || drag.expected != expected
                        || drag.dimension != dimension
                        || drag
                            .latest_distance
                            .is_none_or(|accepted| accepted.to_bits() != distance.to_bits())
                        || drag.latest.is_none()
                        || scene.authenticated_prepared_input() != latest_input
                    {
                        return Err(
                            ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch,
                        );
                    }
                    transaction = Some(self.edit_profile_offset_distance(dimension, distance)?);
                }
                EditorEffect::ClearAcceptedProfileOffsetPreview => {
                    self.cancel_profile_offset_distance_drag();
                    presentation.push(EditorEffect::ClearAcceptedProfileOffsetPreview);
                }
                effect => presentation.push(effect),
            }
        }
        self.cancel_direct_manipulation();
        Ok(ProjectionalEditorPointerOutcome {
            effects: presentation,
            transaction,
        })
    }

    /// Cancels every active pointer/draft interaction without changing intent.
    pub fn cancel_interaction(&mut self) -> Vec<EditorEffect> {
        self.cancel_direct_manipulation();
        self.editor.cancel()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one preview dispatcher keeps transient acknowledgement and history-free failure behavior auditable"
    )]
    fn resolve_pointer_frame(
        &mut self,
        effects: Vec<EditorEffect>,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let mut presentation = Vec::new();
        for effect in effects {
            match effect {
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                } => {
                    self.authenticate_point_drag(pointer_id, point)?;
                    let preview = self.coordinator.preview_point_drag(
                        pointer_id,
                        request_id,
                        model_position,
                        self.preview_control.clone(),
                    )?;
                    let accepted_position = preview.map(|preview| preview.accepted_position);
                    if let Some(drag) = self.point_drag.as_mut() {
                        drag.latest_request_id = Some(request_id);
                        if accepted_position.is_some() {
                            drag.latest_position = accepted_position;
                        }
                    }
                    presentation.extend(self.editor.projected_drag_result(
                        pointer_id,
                        request_id,
                        point,
                        accepted_position,
                    ));
                }
                EditorEffect::ClearPointPreview => {
                    self.cancel_point_drag();
                    presentation.push(EditorEffect::ClearPointPreview);
                }
                EditorEffect::RequestCurveControlPreview {
                    pointer_id,
                    request_id,
                    expected,
                    control,
                    model_position,
                } => {
                    self.authenticate_curve_control_drag(pointer_id, expected, control)?;
                    let preview = self.coordinator.preview_curve_control_drag(
                        pointer_id,
                        request_id,
                        expected,
                        control,
                        model_position,
                        self.preview_control.clone(),
                    )?;
                    let accepted_position = preview.map(|preview| preview.accepted_position);
                    if let Some(preview) = preview
                        && let Some(drag) = self.curve_control_drag.as_mut()
                    {
                        drag.latest_request_id = Some(preview.request_id);
                        drag.latest_position = Some(preview.accepted_position);
                    }
                    presentation.extend(self.editor.curve_control_preview_result(
                        pointer_id,
                        request_id,
                        expected,
                        control,
                        accepted_position,
                    ));
                }
                EditorEffect::ClearCurveControlPreview => {
                    self.cancel_curve_control_drag();
                    presentation.push(EditorEffect::ClearCurveControlPreview);
                }
                EditorEffect::PreviewComputedFeatureRadius {
                    expected,
                    feature,
                    radius,
                } => {
                    self.authenticate_fillet_radius_drag(&expected, feature)?;
                    let patch = {
                        let accepted = self
                            .coordinator
                            .accepted_materialization()
                            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
                        projectional_fillet_radius_patch(
                            self.coordinator.intent(),
                            &accepted.ownership,
                            feature,
                            radius,
                        )?
                    };
                    if let Some(preview) = self.coordinator.preview_patch_materialization(patch)? {
                        if !self
                            .editor
                            .accept_computed_feature_radius_preview(&expected, feature, radius)
                        {
                            self.cancel_fillet_radius_drag();
                            return Err(ProjectionalEditorError::FilletRadiusPreviewMismatch);
                        }
                        let Some(drag) = self.fillet_radius_drag.as_mut() else {
                            return Err(ProjectionalEditorError::MissingFilletRadiusDragRoute);
                        };
                        drag.latest_radius = Some(radius);
                        drag.latest = Some(preview);
                        presentation.push(EditorEffect::PreviewComputedFeatureRadius {
                            expected,
                            feature,
                            radius,
                        });
                    }
                }
                EditorEffect::ClearComputedFeaturePreview
                | EditorEffect::RestoreComputedFeatureRadius { .. } => {
                    self.cancel_fillet_radius_drag();
                    presentation.push(EditorEffect::ClearComputedFeaturePreview);
                }
                EditorEffect::PreviewAcceptedProfileOffsetDistance {
                    expected,
                    dimension,
                    distance,
                } => {
                    self.authenticate_profile_offset_distance_drag(&expected, dimension)?;
                    let patch = {
                        let accepted = self
                            .coordinator
                            .accepted_materialization()
                            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
                        projectional_profile_offset_distance_patch(
                            self.coordinator.intent(),
                            &accepted.ownership,
                            dimension,
                            distance,
                        )?
                    };
                    if let Some(preview) = self.coordinator.preview_patch_materialization(patch)? {
                        if !self
                            .editor
                            .accept_profile_offset_distance_preview(&expected, dimension, distance)
                        {
                            self.cancel_profile_offset_distance_drag();
                            return Err(
                                ProjectionalEditorError::ProfileOffsetDistancePreviewMismatch,
                            );
                        }
                        let Some(drag) = self.profile_offset_distance_drag.as_mut() else {
                            return Err(
                                ProjectionalEditorError::MissingProfileOffsetDistanceDragRoute,
                            );
                        };
                        drag.latest_distance = Some(distance);
                        drag.latest = Some(preview);
                        presentation.push(EditorEffect::PreviewAcceptedProfileOffsetDistance {
                            expected,
                            dimension,
                            distance,
                        });
                    }
                }
                EditorEffect::ClearAcceptedProfileOffsetPreview => {
                    self.cancel_profile_offset_distance_drag();
                    presentation.push(EditorEffect::ClearAcceptedProfileOffsetPreview);
                }
                effect => presentation.push(effect),
            }
        }
        Ok(presentation)
    }

    fn authenticate_point_drag(
        &mut self,
        pointer_id: u64,
        point: DesignPointId,
    ) -> Result<(), ProjectionalEditorError> {
        if let Some(drag) = self.point_drag {
            if drag.pointer_id == pointer_id && drag.point == point {
                return Ok(());
            }
            self.cancel_point_drag();
            return Err(ProjectionalEditorError::PointDragRouteMismatch);
        }
        Err(ProjectionalEditorError::MissingPointDragRoute)
    }

    fn cancel_point_drag(&mut self) {
        self.point_drag = None;
        self.coordinator.cancel_point_drag();
    }

    fn authenticate_curve_control_drag(
        &mut self,
        pointer_id: u64,
        expected: SketchDesignIdentity,
        control: DocumentCurveControlId,
    ) -> Result<(), ProjectionalEditorError> {
        if let Some(drag) = self.curve_control_drag {
            if drag.pointer_id == pointer_id && drag.expected == expected && drag.control == control
            {
                return Ok(());
            }
            self.cancel_curve_control_drag();
            return Err(ProjectionalEditorError::CurveControlDragRouteMismatch);
        }
        Err(ProjectionalEditorError::MissingCurveControlDragRoute)
    }

    fn cancel_curve_control_drag(&mut self) {
        self.curve_control_drag = None;
        self.coordinator.cancel_curve_control_drag();
    }

    fn authenticate_fillet_radius_drag(
        &mut self,
        expected: &geosolve_sketch_features::ComputedFeatureEvaluationInput,
        feature: geosolve_sketch_features::ComputedFeatureId,
    ) -> Result<(), ProjectionalEditorError> {
        if self.fillet_radius_drag.as_ref().is_some_and(|drag| {
            drag.intent == self.coordinator.intent().identity()
                && drag.expected == *expected
                && drag.feature == feature
        }) {
            return Ok(());
        }
        self.cancel_fillet_radius_drag();
        Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch)
    }

    fn cancel_fillet_radius_drag(&mut self) {
        self.fillet_radius_drag = None;
    }

    fn authenticate_profile_offset_distance_drag(
        &mut self,
        expected: &PreparedSketchInput,
        dimension: DocumentDimensionId,
    ) -> Result<(), ProjectionalEditorError> {
        if self
            .profile_offset_distance_drag
            .as_ref()
            .is_some_and(|drag| {
                drag.intent == self.coordinator.intent().identity()
                    && drag.expected == *expected
                    && drag.dimension == dimension
            })
        {
            return Ok(());
        }
        self.cancel_profile_offset_distance_drag();
        Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch)
    }

    fn cancel_profile_offset_distance_drag(&mut self) {
        self.profile_offset_distance_drag = None;
    }

    fn cancel_direct_manipulation(&mut self) {
        self.cancel_point_drag();
        self.cancel_curve_control_drag();
        self.cancel_fillet_radius_drag();
        self.cancel_profile_offset_distance_drag();
    }

    fn reject_construction_commit(&mut self, token: crate::ConstructionCommitToken) {
        let prepared_input = self
            .coordinator
            .accepted_materialization()
            .and_then(|accepted| accepted.session.accepted_prepared_input());
        let _ = self.editor.acknowledge_construction_commit_for_input(
            token,
            false,
            prepared_input.as_ref(),
            true,
        );
    }

    fn clear_transient_selection(&mut self) {
        let _ = self.editor.cancel();
        self.editor.set_selection([]);
    }

    fn reconcile_declaration_selection(&mut self) {
        if self
            .selected_declaration
            .is_some_and(|node| self.coordinator.intent().graph().node(node).is_none())
        {
            self.selected_declaration = None;
        }
    }
}

/// Projectional headless interaction failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionalEditorError {
    #[error(transparent)]
    Coordinator(#[from] ProjectionalCoordinatorError),
    #[error(transparent)]
    Scene(#[from] EditorError),
    #[error(transparent)]
    SourceEdit(#[from] IntentSourceEditError),
    #[error(transparent)]
    InspectorEdit(#[from] IntentInspectorEditError),
    #[error(transparent)]
    Bootstrap(#[from] IntentBootstrapError),
    #[error(transparent)]
    Authoring(#[from] ProjectionalAuthoringError),
    #[error(transparent)]
    FilletAuthoring(#[from] ProjectionalFilletAuthoringError),
    #[error(transparent)]
    ProfileOffset(#[from] ProjectionalProfileOffsetError),
    #[error("the editor effect is not a terminal construction plan")]
    UnexpectedConstructionEffect,
    #[error("the construction token, plan, input, or active geometry variant is not current")]
    ConstructionCommitMismatch,
    #[error("the construction plan targets a stale accepted native input")]
    ConstructionInputMismatch,
    #[error("the authenticated construction acknowledgement did not consume its pending token")]
    ConstructionAcknowledgementMismatch,
    #[error("there is no independently accepted projectional scene")]
    NoAcceptedAuthority,
    #[error("the accepted scene does not match its retained native authority")]
    SceneAuthorityMismatch,
    #[error("computed scene evaluation rejected: {0}")]
    ComputedScene(String),
    #[error("computed scene evaluation stopped before publication")]
    ComputedSceneStopped,
    #[error("flat bootstrap activation currently requires a current accepted native design")]
    BootstrapCurrentAcceptanceRequired,
    #[error("flat bootstrap declarations and the restored native document disagree")]
    BootstrapDocumentMismatch,
    #[error("flat bootstrap accepted state omitted independent solve validation evidence")]
    BootstrapValidationMissing,
    #[error("flat bootstrap accepted state failed independent solve validation")]
    BootstrapValidationRejected,
    #[error("the terminal point sample has no prepared projectional drag route")]
    MissingPointDragRoute,
    #[error("the terminal point sample does not match its prepared drag route")]
    PointDragRouteMismatch,
    #[error("the terminal point sample has no independently accepted preview")]
    MissingAcceptedPointSample,
    #[error("the terminal editor point differs from the accepted native preview")]
    PointPreviewMismatch,
    #[error("the editor prepared both point and selected-curve mutation routes for one press")]
    AmbiguousDirectManipulationRoute,
    #[error("the terminal selected-curve sample has no prepared projectional route")]
    MissingCurveControlDragRoute,
    #[error("the terminal selected-curve sample does not match its prepared route")]
    CurveControlDragRouteMismatch,
    #[error("the terminal selected-curve sample has no independently accepted preview")]
    MissingAcceptedCurveControlSample,
    #[error("the terminal Fillet-radius sample has no prepared projectional route")]
    MissingFilletRadiusDragRoute,
    #[error("the Fillet-radius sample does not match its prepared projectional route")]
    FilletRadiusDragRouteMismatch,
    #[error("the independently accepted Fillet-radius sample does not match editor state")]
    FilletRadiusPreviewMismatch,
    #[error("the terminal Profile Offset sample has no prepared projectional route")]
    MissingProfileOffsetDistanceDragRoute,
    #[error("the Profile Offset sample does not match its prepared projectional route")]
    ProfileOffsetDistanceDragRouteMismatch,
    #[error("the independently accepted Profile Offset sample does not match editor state")]
    ProfileOffsetDistancePreviewMismatch,
}
