// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless interaction adapter for projectional design intent.
//!
//! This adapter deliberately owns no [`RetainedEditorCoordinator`]. Durable
//! geometry, accepted-scene publication and Undo/Redo remain exclusively in
//! [`ProjectionalIntentCoordinator`]; [`ConstraintEditor`] contributes only
//! disposable selection, hover and pointer-gesture state.

use std::sync::Arc;

use geosolve_sketch::{
    CurveId, DesignPointId, DocumentCurveControlId, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentId, GeometryRole, OperationControl, OperationOutcome,
    PreparedSketchInput, RetainedSketchDocumentSession, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE,
    SketchDesignIdentity, SketchHardValidity,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureAuthoringSnapshot, ComputedFeatureDefinition,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
};
use geosolve_sketch_intent::{
    IntentFieldKey, IntentKey, IntentLiteral, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPlanDisposition, IntentSession, IntentSessionIdentity, NodeId,
    OperationKind,
};
use geosolve_sketch_topology::{OffsetOperandRequest, PreparedOffsetOperandQuery};
use thiserror::Error;

use crate::intent_bootstrap::{
    decode_flat_intent_bootstrap_prefix, flat_intent_bootstrap_prefix_materialization_map,
};
use crate::{
    AuthoringApplication, AuthoringState, ColdIntentMaterialization, ColdIntentMaterializer,
    ConstraintEditor, EditorEffect, EditorError, EditorScene, FeatureAuthoringCandidate,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    GeometryRoleSelectionState, IntentBootstrapError, IntentInspectorEditError,
    IntentInspectorEditTarget, IntentInspectorEditValue, IntentInspectorProjection,
    IntentNativeBinding, IntentSourceEditError, IntentSourceTokenId, IntentValidationEvidence,
    IntentWorkbenchProjection, Modifiers, OffsetAuthoringCandidate, OffsetAuthoringOutcome,
    OffsetAuthoringState, PickTolerance, PointerInput, ProfileOffsetDirectionState,
    ProjectionalAuthoringError, ProjectionalCoordinatorError, ProjectionalFilletAuthoringError,
    ProjectionalIntentCoordinator, ProjectionalPatchOutcome, ProjectionalProfileOffsetError,
    SelectionItem, Viewport, decode_flat_intent_bootstrap,
    flat_intent_bootstrap_materialization_map, projectional_application_patch,
    projectional_construction_patch, projectional_fillet_patch, projectional_fillet_radius_patch,
    projectional_profile_offset_delete_node_patch, projectional_profile_offset_delete_patch,
    projectional_profile_offset_direction_patch, projectional_profile_offset_distance_patch,
    projectional_profile_offset_patch,
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

#[derive(Clone, Debug)]
struct ProjectionalFilletAuthoringPreview {
    intent: IntentSessionIdentity,
    candidate: FeatureAuthoringCandidate,
    materialization: ColdIntentMaterialization,
    feature: geosolve_sketch_features::ComputedFeatureId,
}

#[derive(Clone, Debug)]
struct ProjectionalProfileOffsetAuthoringPreview {
    intent: IntentSessionIdentity,
    candidate: OffsetAuthoringCandidate,
    materialization: ColdIntentMaterialization,
    provisional_items: Vec<SelectionItem>,
    dimension: DocumentDimensionId,
}

#[derive(Clone, Debug)]
struct ActiveFilletAuthoringRadiusDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    expected: geosolve_sketch_features::ComputedFeatureEvaluationInput,
    feature: geosolve_sketch_features::ComputedFeatureId,
    origin_state: FeatureAuthoringState,
    origin_preview: ProjectionalFilletAuthoringPreview,
    latest_radius: Option<f64>,
    symbol: IntentKey,
}

#[derive(Clone, Debug)]
struct ActiveProfileOffsetAuthoringDistanceDrag {
    pointer_id: u64,
    intent: IntentSessionIdentity,
    expected: PreparedSketchInput,
    dimension: DocumentDimensionId,
    origin_state: OffsetAuthoringState,
    origin_preview: ProjectionalProfileOffsetAuthoringPreview,
    latest_distance: Option<f64>,
    symbol: IntentKey,
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
    fillet_authoring_preview: Option<ProjectionalFilletAuthoringPreview>,
    profile_offset_authoring_preview: Option<ProjectionalProfileOffsetAuthoringPreview>,
    fillet_authoring_radius_drag: Option<ActiveFilletAuthoringRadiusDrag>,
    profile_offset_authoring_distance_drag: Option<ActiveProfileOffsetAuthoringDistanceDrag>,
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
            fillet_authoring_preview: None,
            profile_offset_authoring_preview: None,
            fillet_authoring_radius_drag: None,
            profile_offset_authoring_distance_drag: None,
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

    /// Native session which owns the currently rendered projectional scene,
    /// including history-free property or authoring previews.
    #[must_use]
    pub fn presentation_session(&self) -> Option<&RetainedSketchDocumentSession> {
        self.fillet_radius_drag
            .as_ref()
            .and_then(|drag| drag.latest.as_ref())
            .or_else(|| {
                self.profile_offset_distance_drag
                    .as_ref()
                    .and_then(|drag| drag.latest.as_ref())
            })
            .or_else(|| {
                self.fillet_authoring_preview
                    .as_ref()
                    .map(|preview| &preview.materialization)
            })
            .or_else(|| {
                self.profile_offset_authoring_preview
                    .as_ref()
                    .map(|preview| &preview.materialization)
            })
            .map(|materialization| &materialization.session)
            .or_else(|| self.coordinator.presentation_session())
    }

    /// Builds the durable editor-owned Outline/source/History projection.
    ///
    /// Pointer-frame methods never call this function; presentation adapters
    /// rebuild these DTOs only at a durable render boundary.
    #[must_use]
    pub fn workbench_projection(&self) -> IntentWorkbenchProjection {
        IntentWorkbenchProjection::from_session(self.coordinator.intent())
    }

    /// Currently selected logical declaration.
    ///
    /// A unique native canvas/tree pick is projected back to this same stable
    /// owner. Outline/source selection clears native selection, so an older
    /// declaration can never remain the implicit mutation target after the
    /// user visibly selects a different sketch item.
    #[must_use]
    pub const fn selected_declaration(&self) -> Option<NodeId> {
        self.selected_declaration
    }

    fn scene_materialization(
        &self,
    ) -> Result<
        (
            &ColdIntentMaterialization,
            &RetainedSketchDocumentSession,
            bool,
        ),
        ProjectionalEditorError,
    > {
        let accepted = self
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
        let authoring_preview = self
            .fillet_authoring_preview
            .as_ref()
            .map(|preview| &preview.materialization)
            .or_else(|| {
                self.profile_offset_authoring_preview
                    .as_ref()
                    .map(|preview| &preview.materialization)
            });
        let materialization = property_preview.or(authoring_preview).unwrap_or(accepted);
        let session = self
            .presentation_session()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        Ok((materialization, session, property_preview.is_some()))
    }

    /// Selects one stable declaration for Outline/source/Inspector projection.
    ///
    /// Missing or deleted identities clear the logical selection. A private
    /// one-consumer Profile Offset aggregate resolves to its visible operation
    /// owner. Selecting a declaration clears any competing native selection.
    /// This is presentation state and never appends intent history.
    pub fn set_selected_declaration(&mut self, node: Option<NodeId>) -> bool {
        self.selected_declaration = node.and_then(|node| self.visible_declaration_owner(node));
        if self.selected_declaration.is_some() {
            self.editor.set_selection([]);
        }
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
        let (materialization, session, property_preview_active) = self.scene_materialization()?;
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
        if !property_preview_active {
            self.attach_computed_fillet_radius_rails(&mut scene, session, materialization)?;
        }
        if property_preview_active && let Some(drag) = self.fillet_radius_drag.as_ref() {
            scene.set_computed_fillet_interaction_origin(drag.expected)?;
        }
        if property_preview_active && let Some(drag) = self.profile_offset_distance_drag.as_ref() {
            scene.set_accepted_offset_distance_interaction_origin(&drag.expected)?;
        }
        if let Some(drag) = self.fillet_authoring_radius_drag.as_ref() {
            scene.set_computed_fillet_interaction_origin(drag.expected)?;
        }
        if let Some(drag) = self.profile_offset_authoring_distance_drag.as_ref() {
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

    /// Captures the exact current native boundary used by projectional
    /// computed-Fillet collection.
    ///
    /// # Errors
    ///
    /// Returns a typed accepted-scene or feature-snapshot failure.
    pub fn feature_authoring_snapshot(
        &self,
    ) -> Result<ComputedFeatureAuthoringSnapshot, ProjectionalEditorError> {
        let accepted = self
            .coordinator
            .accepted_materialization()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        ComputedFeatureAuthoringSnapshot::capture(&accepted.session)
            .map_err(|error| ProjectionalEditorError::ComputedScene(error.to_string()))
    }

    /// Enters projectional computed-Fillet collection from exact accepted
    /// native selection and prepares a history-free preview when the
    /// preselection already forms a complete candidate.
    ///
    /// # Errors
    ///
    /// Returns a snapshot or cold-preview failure without changing the
    /// collector or replacing a prior valid preview.
    pub fn activate_feature_authoring(
        &mut self,
        state: &mut FeatureAuthoringState,
        tool: FeatureAuthoringTool,
        options: FeatureAuthoringOptions,
        selection: &[(SelectionItem, Option<f64>)],
        symbol: IntentKey,
    ) -> Result<FeatureAuthoringOutcome, ProjectionalEditorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = snapshot.sketch_document().clone();
        let mut trial = state.clone();
        let _ = trial.activate(&snapshot, &document, tool, &[]);
        let options_outcome = trial.set_options(&snapshot, options);
        if matches!(options_outcome, FeatureAuthoringOutcome::Warning(_)) {
            return Ok(options_outcome);
        }
        let outcome = if selection.is_empty() {
            options_outcome
        } else {
            trial.pick_items(&snapshot, &document, selection)
        };
        self.finish_feature_authoring_transition(state, trial, outcome, symbol)
    }

    /// Resolves one Fillet canvas pick through the shared native authoring
    /// owner, preparing the exact typed intent preview before publishing the
    /// collector transition.
    ///
    /// # Errors
    ///
    /// Returns a snapshot, stale-scene or cold-preview failure state-neutrally.
    pub fn transact_feature_authoring_pick_at(
        &mut self,
        state: &mut FeatureAuthoringState,
        scene: &EditorScene,
        position: crate::ScreenPoint,
        tolerance: PickTolerance,
        symbol: IntentKey,
    ) -> Result<FeatureAuthoringOutcome, ProjectionalEditorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = snapshot.sketch_document().clone();
        let mut trial = state.clone();
        let outcome = trial.pick_at_with_policy(
            &snapshot,
            &document,
            scene,
            position,
            tolerance,
            self.editor.geometry_interaction_policy(),
        );
        self.finish_feature_authoring_transition(state, trial, outcome, symbol)
    }

    /// Resolves one semantic tree/keyboard Fillet pick through the same
    /// history-free preview boundary as canvas input.
    ///
    /// # Errors
    ///
    /// Returns a snapshot or cold-preview failure state-neutrally.
    pub fn transact_feature_authoring_pick_items(
        &mut self,
        state: &mut FeatureAuthoringState,
        items: &[(SelectionItem, Option<f64>)],
        symbol: IntentKey,
    ) -> Result<FeatureAuthoringOutcome, ProjectionalEditorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let document = snapshot.sketch_document().clone();
        let mut trial = state.clone();
        let outcome = trial.pick_items(&snapshot, &document, items);
        self.finish_feature_authoring_transition(state, trial, outcome, symbol)
    }

    /// Continues the complete projectional Fillet candidate to one absolute
    /// radius and replaces its preview only after cold validation succeeds.
    ///
    /// # Errors
    ///
    /// Returns a snapshot or cold-preview failure state-neutrally.
    pub fn transact_feature_authoring_radius(
        &mut self,
        state: &mut FeatureAuthoringState,
        radius: f64,
        symbol: IntentKey,
    ) -> Result<FeatureAuthoringOutcome, ProjectionalEditorError> {
        let snapshot = self.feature_authoring_snapshot()?;
        let mut trial = state.clone();
        let outcome = trial.continue_radius_absolute(&snapshot, radius);
        self.finish_feature_authoring_transition(state, trial, outcome, symbol)
    }

    /// Publishes the exact Fillet item an unchanged press would consume.
    /// This hover path changes no intent, preview or authoring candidate.
    ///
    /// # Errors
    ///
    /// Returns a current-snapshot failure.
    pub fn pointer_move_feature_authoring(
        &mut self,
        state: &FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        tolerance: PickTolerance,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        if let Some((_, item)) =
            self.feature_authoring_radius_hit(state, scene, input.position, tolerance)?
        {
            return Ok(self.editor.set_authoring_hover_target(Some(item)));
        }
        let snapshot = self.feature_authoring_snapshot()?;
        let target = state.hover_item_at_with_policy(
            &snapshot,
            snapshot.sketch_document(),
            scene,
            input.position,
            tolerance,
            self.editor.geometry_interaction_policy(),
        );
        Ok(self.editor.set_authoring_hover_target(target))
    }

    fn feature_authoring_radius_hit(
        &self,
        state: &FeatureAuthoringState,
        scene: &EditorScene,
        position: crate::ScreenPoint,
        tolerance: PickTolerance,
    ) -> Result<
        Option<(geosolve_sketch_features::ComputedCornerRef, SelectionItem)>,
        ProjectionalEditorError,
    > {
        if !self.feature_authoring_preview_matches(state) {
            return Ok(None);
        }
        let preview = self
            .fillet_authoring_preview
            .as_ref()
            .ok_or(ProjectionalEditorError::AuthoringPreviewIdentityMismatch)?;
        if scene.computed_input != Some(preview.materialization.computed.input()) {
            return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
        }
        let feature = preview
            .materialization
            .features
            .feature(preview.feature)
            .ok_or(ProjectionalEditorError::AuthoringPreviewIdentityMismatch)?;
        let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
        Ok(fillet.corners.iter().find_map(|corner| {
            let owner = geosolve_sketch_features::ComputedCornerRef {
                feature: preview.feature,
                corner: corner.id,
            };
            self.editor
                .feature_radius_hover_item(scene, position, owner, tolerance)
                .map(|item| (owner, item))
        }))
    }

    /// Starts a history-free radius gesture only when the press independently
    /// hits the exact cold-materialized Fillet candidate currently rendered.
    /// A non-radius press is returned to the ordinary native operand collector.
    ///
    /// # Errors
    ///
    /// Rejects stale preview, scene or gesture provenance without changing
    /// intent, history, accepted authority or the collector.
    pub fn pointer_down_feature_authoring_radius(
        &mut self,
        state: &FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        tolerance: PickTolerance,
        symbol: IntentKey,
    ) -> Result<Option<Vec<EditorEffect>>, ProjectionalEditorError> {
        let Some((owner, _)) =
            self.feature_authoring_radius_hit(state, scene, input.position, tolerance)?
        else {
            return Ok(None);
        };
        if self.fillet_authoring_radius_drag.is_some() {
            return Ok(Some(Vec::new()));
        }
        let preview = self
            .fillet_authoring_preview
            .clone()
            .ok_or(ProjectionalEditorError::AuthoringPreviewIdentityMismatch)?;
        let effects = self
            .editor
            .pointer_down_feature_radius(scene, input, owner, tolerance)
            .ok_or(ProjectionalEditorError::FilletRadiusDragRouteMismatch)?;
        let route = self
            .editor
            .prepared_feature_radius_drag_route()
            .ok_or(ProjectionalEditorError::FilletRadiusDragRouteMismatch)?;
        if route.pointer_id != input.pointer_id
            || route.expected != preview.materialization.computed.input()
            || route.feature != preview.feature
            || route.origin_radius.to_bits() != preview.candidate.radius().to_bits()
        {
            let _ = self.editor.cancel();
            return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
        }
        self.fillet_authoring_radius_drag = Some(ActiveFilletAuthoringRadiusDrag {
            pointer_id: input.pointer_id,
            intent: self.coordinator.intent().identity(),
            expected: route.expected,
            feature: route.feature,
            origin_state: state.clone(),
            origin_preview: preview,
            latest_radius: None,
            symbol,
        });
        Ok(Some(effects))
    }

    /// Advances a live pre-Apply Fillet radius gesture by cold-materializing
    /// the entire typed candidate. Accepted intent and history remain untouched.
    ///
    /// # Errors
    ///
    /// Rejects stale/out-of-order effects or an invalid continuation while
    /// retaining the prior last-valid candidate and preview.
    pub fn pointer_move_feature_authoring_radius(
        &mut self,
        state: &mut FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let active = self
            .fillet_authoring_radius_drag
            .clone()
            .ok_or(ProjectionalEditorError::MissingFilletRadiusDragRoute)?;
        if active.pointer_id != input.pointer_id
            || active.intent != self.coordinator.intent().identity()
        {
            return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
        }
        let effects = self.editor.pointer_move(scene, input);
        let mut presentation = Vec::new();
        for effect in effects {
            match effect {
                EditorEffect::PreviewComputedFeatureRadius {
                    expected,
                    feature,
                    radius,
                } => {
                    if expected != active.expected || feature != active.feature {
                        return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
                    }
                    let snapshot = self.feature_authoring_snapshot()?;
                    let mut trial = active.origin_state.clone();
                    let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } =
                        trial.continue_radius_absolute(&snapshot, radius)
                    else {
                        return Err(ProjectionalEditorError::AuthoringPreviewRejected);
                    };
                    let preview = self.prepare_computed_fillet_authoring_preview(
                        active.symbol.clone(),
                        &candidate,
                    )?;
                    if preview.feature != feature
                        || preview.candidate != candidate
                        || !self
                            .editor
                            .accept_computed_feature_radius_preview(&expected, feature, radius)
                    {
                        return Err(ProjectionalEditorError::FilletRadiusPreviewMismatch);
                    }
                    self.fillet_authoring_preview = Some(preview);
                    *state = trial;
                    if let Some(drag) = self.fillet_authoring_radius_drag.as_mut() {
                        drag.latest_radius = Some(radius);
                    }
                    presentation.push(EditorEffect::PreviewComputedFeatureRadius {
                        expected,
                        feature,
                        radius,
                    });
                }
                EditorEffect::RestoreComputedFeatureRadius { .. }
                | EditorEffect::ClearComputedFeaturePreview => {
                    self.restore_feature_authoring_radius_drag(state);
                    presentation.push(EditorEffect::ClearComputedFeaturePreview);
                }
                other => presentation.push(other),
            }
        }
        Ok(presentation)
    }

    /// Finishes a pre-Apply Fillet radius gesture without publishing intent.
    /// Apply remains the only durable feature transaction.
    ///
    /// # Errors
    ///
    /// Rejects a foreign pointer or terminal sample and restores the exact
    /// pointer-down collector/preview checkpoint.
    pub fn pointer_up_feature_authoring_radius(
        &mut self,
        state: &mut FeatureAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<bool, ProjectionalEditorError> {
        let active = self
            .fillet_authoring_radius_drag
            .clone()
            .ok_or(ProjectionalEditorError::MissingFilletRadiusDragRoute)?;
        if active.pointer_id != input.pointer_id
            || active.intent != self.coordinator.intent().identity()
        {
            self.restore_feature_authoring_radius_drag(state);
            return Err(ProjectionalEditorError::FilletRadiusDragRouteMismatch);
        }
        let effects = self.editor.pointer_up(scene, scene.design_identity, input);
        let committed = effects.iter().any(|effect| {
            matches!(
                effect,
                EditorEffect::CommitComputedFeatureRadius {
                    expected,
                    feature,
                    radius,
                } if *expected == active.expected
                    && *feature == active.feature
                    && active.latest_radius.is_some_and(|latest| latest.to_bits() == radius.to_bits())
            )
        });
        if committed && self.feature_authoring_preview_matches(state) {
            self.fillet_authoring_radius_drag = None;
            return Ok(true);
        }
        self.restore_feature_authoring_radius_drag(state);
        Ok(false)
    }

    /// Cancels a live pre-Apply Fillet radius gesture and restores its exact
    /// history-free pointer-down state.
    pub fn cancel_feature_authoring_radius_drag(
        &mut self,
        state: &mut FeatureAuthoringState,
    ) -> Vec<EditorEffect> {
        let effects = self.editor.cancel();
        self.restore_feature_authoring_radius_drag(state);
        effects
    }

    fn restore_feature_authoring_radius_drag(&mut self, state: &mut FeatureAuthoringState) {
        if let Some(active) = self.fillet_authoring_radius_drag.take() {
            *state = active.origin_state;
            self.fillet_authoring_preview = Some(active.origin_preview);
        }
    }

    /// Whether projectional Fillet authoring currently owns the captured
    /// pointer as a history-free radius gesture.
    #[must_use]
    pub const fn feature_authoring_radius_drag_active(&self) -> bool {
        self.fillet_authoring_radius_drag.is_some()
    }

    /// Whether the complete collector candidate is exactly the cold preview
    /// currently rendered by [`Self::scene`].
    #[must_use]
    pub fn feature_authoring_preview_matches(&self, state: &FeatureAuthoringState) -> bool {
        let FeatureAuthoringOutcome::Apply(candidate) = state.apply() else {
            return false;
        };
        self.fillet_authoring_preview
            .as_ref()
            .is_some_and(|preview| {
                preview.intent == self.coordinator.intent().identity()
                    && preview.candidate == candidate
            })
    }

    /// Stable computed owner used only to paint the current provisional
    /// Fillet result as selected candidate geometry.
    #[must_use]
    pub fn feature_authoring_preview_item(&self) -> Option<SelectionItem> {
        self.fillet_authoring_preview
            .as_ref()
            .filter(|preview| preview.intent == self.coordinator.intent().identity())
            .map(|preview| SelectionItem::Feature(preview.feature))
    }

    fn finish_feature_authoring_transition(
        &mut self,
        state: &mut FeatureAuthoringState,
        trial: FeatureAuthoringState,
        outcome: FeatureAuthoringOutcome,
        symbol: IntentKey,
    ) -> Result<FeatureAuthoringOutcome, ProjectionalEditorError> {
        match &outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. } => {
                let preview = self.prepare_computed_fillet_authoring_preview(symbol, candidate)?;
                self.profile_offset_authoring_preview = None;
                self.fillet_authoring_preview = Some(preview);
            }
            FeatureAuthoringOutcome::ModeEntered(_)
            | FeatureAuthoringOutcome::Collecting { .. }
            | FeatureAuthoringOutcome::CandidateCleared(_)
            | FeatureAuthoringOutcome::ModeExited => {
                self.fillet_authoring_preview = None;
            }
            FeatureAuthoringOutcome::NoNativeHit(_)
            | FeatureAuthoringOutcome::Warning(_)
            | FeatureAuthoringOutcome::Inactive => return Ok(outcome),
            FeatureAuthoringOutcome::Apply(_) => {}
        }
        *state = trial;
        Ok(outcome)
    }

    fn prepare_computed_fillet_authoring_preview(
        &self,
        symbol: IntentKey,
        candidate: &FeatureAuthoringCandidate,
    ) -> Result<ProjectionalFilletAuthoringPreview, ProjectionalEditorError> {
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
        let translated = projectional_fillet_patch(
            self.coordinator.intent().identity(),
            self.coordinator.intent(),
            &accepted.ownership,
            accepted_input,
            accepted_state.identity(),
            symbol,
            candidate,
        )?;
        let materialization = self
            .coordinator
            .preview_patch_materialization(translated.patch)?
            .ok_or(ProjectionalEditorError::AuthoringPreviewRejected)?;
        let features = materialization
            .features
            .features()
            .iter()
            .filter(|feature| accepted.features.feature(feature.id).is_none())
            .map(|feature| feature.id)
            .collect::<Vec<_>>();
        let [feature] = features.as_slice() else {
            return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
        };
        Ok(ProjectionalFilletAuthoringPreview {
            intent: self.coordinator.intent().identity(),
            candidate: candidate.clone(),
            materialization,
            feature: *feature,
        })
    }

    /// Activates native Profile Offset from the exact current accepted
    /// topology snapshot. No partial operand index is installed.
    ///
    /// # Errors
    ///
    /// Returns topology capture, bounded-work or accepted-authority failure.
    pub fn activate_offset_authoring(
        &mut self,
        state: &mut OffsetAuthoringState,
    ) -> Result<OffsetAuthoringOutcome, ProjectionalEditorError> {
        let accepted = self
            .coordinator
            .accepted_materialization()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let query =
            PreparedOffsetOperandQuery::capture(&accepted.session, OffsetOperandRequest::default())
                .map_err(|error| ProjectionalEditorError::OffsetAuthoring(error.to_string()))?;
        let OperationOutcome::Completed { value, .. } = query
            .execute(crate::coordinator::bounded_geometry_control())
            .map_err(|error| ProjectionalEditorError::OffsetAuthoring(error.to_string()))?
        else {
            return Err(ProjectionalEditorError::OffsetAuthoring(
                "topology capture exhausted its bounded work envelope".into(),
            ));
        };
        let index = value.operand_index.map(Arc::new).ok_or_else(|| {
            ProjectionalEditorError::OffsetAuthoring(
                "topology capture did not produce a complete operand index".into(),
            )
        })?;
        let model_scale = accepted.session.design_document().model_scale();
        self.clear_authoring_previews();
        Ok(state.activate(index, model_scale))
    }

    /// Cold-materializes the exact current Profile Offset candidate without
    /// publishing intent or history. A failed replacement leaves the last
    /// valid preview intact.
    ///
    /// # Errors
    ///
    /// Returns stale/incomplete topology or ordinary cold-materialization
    /// failure without changing accepted authority.
    pub fn refresh_offset_authoring_preview(
        &mut self,
        state: &OffsetAuthoringState,
        symbol: IntentKey,
    ) -> Result<bool, ProjectionalEditorError> {
        if state.candidate().is_none() {
            self.profile_offset_authoring_preview = None;
            return Ok(false);
        }
        let preview = self.prepare_profile_offset_authoring_preview(state, symbol)?;
        self.fillet_authoring_preview = None;
        self.profile_offset_authoring_preview = Some(preview);
        Ok(true)
    }

    fn prepare_profile_offset_authoring_preview(
        &self,
        state: &OffsetAuthoringState,
        symbol: IntentKey,
    ) -> Result<ProjectionalProfileOffsetAuthoringPreview, ProjectionalEditorError> {
        let candidate = state
            .candidate()
            .ok_or(ProjectionalEditorError::AuthoringCandidateIncomplete)?;
        let accepted = self
            .coordinator
            .accepted_materialization()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let translated = projectional_profile_offset_patch(
            self.coordinator.intent().identity(),
            self.coordinator.intent(),
            &accepted.ownership,
            &accepted.session,
            state,
            symbol,
        )?;
        let materialization = self
            .coordinator
            .preview_patch_materialization(translated.patch)?
            .ok_or(ProjectionalEditorError::AuthoringPreviewRejected)?;
        let provisional_items = provisional_items(&accepted.ownership, &materialization.ownership);
        if provisional_items.is_empty() {
            return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
        }
        let dimension = provisional_profile_offset_dimension(&materialization, &provisional_items)?;
        Ok(ProjectionalProfileOffsetAuthoringPreview {
            intent: self.coordinator.intent().identity(),
            candidate,
            materialization,
            provisional_items,
            dimension,
        })
    }

    /// Applies one numeric Profile Offset authoring edit atomically with its
    /// exact cold preview. Invalid/rejected replacements leave both collector
    /// and last-valid preview unchanged.
    ///
    /// # Errors
    ///
    /// Returns an exact translation or materialization failure without
    /// changing intent, accepted authority, history or collector state.
    pub fn transact_offset_authoring_distance(
        &mut self,
        state: &mut OffsetAuthoringState,
        distance: f64,
        symbol: IntentKey,
    ) -> Result<OffsetAuthoringOutcome, ProjectionalEditorError> {
        let mut trial = state.clone();
        let outcome = trial.set_distance(distance);
        match &outcome {
            OffsetAuthoringOutcome::DistanceChanged { .. } => {
                let preview = trial
                    .candidate()
                    .map(|_| self.prepare_profile_offset_authoring_preview(&trial, symbol))
                    .transpose()?;
                self.fillet_authoring_preview = None;
                self.profile_offset_authoring_preview = preview;
                *state = trial;
            }
            OffsetAuthoringOutcome::Warning(_)
            | OffsetAuthoringOutcome::Inactive
            | OffsetAuthoringOutcome::ModeEntered(_)
            | OffsetAuthoringOutcome::HoverChanged(_)
            | OffsetAuthoringOutcome::OperandChanged { .. }
            | OffsetAuthoringOutcome::ApplyRequested(_)
            | OffsetAuthoringOutcome::ModeExited => {}
        }
        Ok(outcome)
    }

    /// Starts a history-free distance gesture when a press hits the exact
    /// provisional Profile Offset target or annotation currently rendered.
    ///
    /// # Errors
    ///
    /// Rejects stale preview/scene provenance without changing durable state.
    pub fn pointer_down_offset_authoring_distance(
        &mut self,
        state: &OffsetAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
        symbol: IntentKey,
    ) -> Result<Option<Vec<EditorEffect>>, ProjectionalEditorError> {
        if !self.offset_authoring_preview_matches(state) {
            return Ok(None);
        }
        if self.profile_offset_authoring_distance_drag.is_some() {
            return Ok(Some(Vec::new()));
        }
        let preview = self
            .profile_offset_authoring_preview
            .clone()
            .ok_or(ProjectionalEditorError::AuthoringPreviewIdentityMismatch)?;
        let Some(effects) = self
            .editor
            .pointer_down_accepted_offset_distance(scene, input)
        else {
            return Ok(None);
        };
        let route = self
            .editor
            .prepared_accepted_offset_distance_drag_route()
            .ok_or(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch)?;
        let expected = preview
            .materialization
            .session
            .accepted_prepared_input()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        if route.pointer_id != input.pointer_id
            || route.expected != expected
            || route.dimension != preview.dimension
            || route.origin_distance.to_bits() != preview.candidate.distance.to_bits()
        {
            let _ = self.editor.cancel();
            return Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch);
        }
        self.profile_offset_authoring_distance_drag =
            Some(ActiveProfileOffsetAuthoringDistanceDrag {
                pointer_id: input.pointer_id,
                intent: self.coordinator.intent().identity(),
                expected,
                dimension: route.dimension,
                origin_state: state.clone(),
                origin_preview: preview,
                latest_distance: None,
                symbol,
            });
        Ok(Some(effects))
    }

    /// Advances a live pre-Apply Profile Offset distance gesture through the
    /// same typed patch and cold-materialization boundary used by numeric edit.
    ///
    /// # Errors
    ///
    /// Rejects stale or invalid samples while retaining the prior last-valid
    /// preview and every durable authority unchanged.
    pub fn pointer_move_offset_authoring_distance(
        &mut self,
        state: &mut OffsetAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let active = self
            .profile_offset_authoring_distance_drag
            .clone()
            .ok_or(ProjectionalEditorError::MissingProfileOffsetDistanceDragRoute)?;
        if active.pointer_id != input.pointer_id
            || active.intent != self.coordinator.intent().identity()
        {
            return Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch);
        }
        let effects = self.editor.pointer_move(scene, input);
        let mut presentation = Vec::new();
        for effect in effects {
            match effect {
                EditorEffect::PreviewAcceptedProfileOffsetDistance {
                    expected,
                    dimension,
                    distance,
                } => {
                    if expected != active.expected || dimension != active.dimension {
                        return Err(
                            ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch,
                        );
                    }
                    let mut trial = active.origin_state.clone();
                    if !matches!(
                        trial.set_distance(distance),
                        OffsetAuthoringOutcome::DistanceChanged { distance: accepted, .. }
                            if accepted.to_bits() == distance.to_bits()
                    ) {
                        return Err(ProjectionalEditorError::AuthoringPreviewRejected);
                    }
                    let preview = self
                        .prepare_profile_offset_authoring_preview(&trial, active.symbol.clone())?;
                    if preview.dimension != dimension
                        || !self
                            .editor
                            .accept_profile_offset_distance_preview(&expected, dimension, distance)
                    {
                        return Err(ProjectionalEditorError::ProfileOffsetDistancePreviewMismatch);
                    }
                    self.profile_offset_authoring_preview = Some(preview);
                    *state = trial;
                    if let Some(drag) = self.profile_offset_authoring_distance_drag.as_mut() {
                        drag.latest_distance = Some(distance);
                    }
                    presentation.push(EditorEffect::PreviewAcceptedProfileOffsetDistance {
                        expected,
                        dimension,
                        distance,
                    });
                }
                EditorEffect::ClearAcceptedProfileOffsetPreview => {
                    self.restore_offset_authoring_distance_drag(state);
                    presentation.push(EditorEffect::ClearAcceptedProfileOffsetPreview);
                }
                other => presentation.push(other),
            }
        }
        Ok(presentation)
    }

    /// Finishes a pre-Apply Profile Offset distance gesture without publishing
    /// a declaration or history entry.
    ///
    /// # Errors
    ///
    /// Rejects a foreign pointer or terminal sample and restores the exact
    /// pointer-down collector/preview checkpoint.
    pub fn pointer_up_offset_authoring_distance(
        &mut self,
        state: &mut OffsetAuthoringState,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<bool, ProjectionalEditorError> {
        let active = self
            .profile_offset_authoring_distance_drag
            .clone()
            .ok_or(ProjectionalEditorError::MissingProfileOffsetDistanceDragRoute)?;
        if active.pointer_id != input.pointer_id
            || active.intent != self.coordinator.intent().identity()
        {
            self.restore_offset_authoring_distance_drag(state);
            return Err(ProjectionalEditorError::ProfileOffsetDistanceDragRouteMismatch);
        }
        let effects = self.editor.pointer_up(scene, scene.design_identity, input);
        let committed = effects.iter().any(|effect| {
            matches!(
                effect,
                EditorEffect::CommitAcceptedProfileOffsetDistance {
                    expected,
                    dimension,
                    distance,
                } if *expected == active.expected
                    && *dimension == active.dimension
                    && active.latest_distance.is_some_and(|latest| latest.to_bits() == distance.to_bits())
            )
        });
        if committed && self.offset_authoring_preview_matches(state) {
            self.profile_offset_authoring_distance_drag = None;
            return Ok(true);
        }
        self.restore_offset_authoring_distance_drag(state);
        Ok(false)
    }

    /// Cancels a live pre-Apply Profile Offset distance gesture and restores
    /// its exact history-free pointer-down state.
    pub fn cancel_offset_authoring_distance_drag(
        &mut self,
        state: &mut OffsetAuthoringState,
    ) -> Vec<EditorEffect> {
        let effects = self.editor.cancel();
        self.restore_offset_authoring_distance_drag(state);
        effects
    }

    fn restore_offset_authoring_distance_drag(&mut self, state: &mut OffsetAuthoringState) {
        if let Some(active) = self.profile_offset_authoring_distance_drag.take() {
            *state = active.origin_state;
            self.profile_offset_authoring_preview = Some(active.origin_preview);
        }
    }

    /// Whether projectional Offset authoring currently owns the captured
    /// pointer as a history-free distance gesture.
    #[must_use]
    pub const fn offset_authoring_distance_drag_active(&self) -> bool {
        self.profile_offset_authoring_distance_drag.is_some()
    }

    /// Whether Apply would publish the exact Profile Offset candidate currently
    /// rendered by [`Self::scene`].
    #[must_use]
    pub fn offset_authoring_preview_matches(&self, state: &OffsetAuthoringState) -> bool {
        self.profile_offset_authoring_preview
            .as_ref()
            .zip(state.candidate().as_ref())
            .is_some_and(|(preview, candidate)| {
                preview.intent == self.coordinator.intent().identity()
                    && preview.candidate == *candidate
            })
    }

    /// Exact native items introduced only by the current history-free Profile
    /// Offset candidate.
    #[must_use]
    pub fn offset_authoring_provisional_items(&self) -> &[SelectionItem] {
        self.profile_offset_authoring_preview
            .as_ref()
            .filter(|preview| preview.intent == self.coordinator.intent().identity())
            .map_or(&[], |preview| preview.provisional_items.as_slice())
    }

    /// Clears only computed-Fillet/Profile-Offset authoring previews. Accepted
    /// intent, history, evidence and the reusable collectors are unchanged.
    pub fn clear_authoring_previews(&mut self) {
        let _ = self.editor.cancel();
        self.fillet_authoring_radius_drag = None;
        self.profile_offset_authoring_distance_drag = None;
        self.fillet_authoring_preview = None;
        self.profile_offset_authoring_preview = None;
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
        self.clear_authoring_previews();
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

    /// Applies only the exact computed-Fillet candidate currently rendered by
    /// the history-free projectional preview.
    ///
    /// # Errors
    ///
    /// Rejects incomplete/stale collector state or a preview mismatch without
    /// changing intent, accepted authority, history or collector state.
    pub fn apply_computed_fillet_preview(
        &mut self,
        state: &mut FeatureAuthoringState,
        symbol: IntentKey,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let FeatureAuthoringOutcome::Apply(candidate) = state.apply() else {
            return Err(ProjectionalEditorError::AuthoringCandidateIncomplete);
        };
        if !self.feature_authoring_preview_matches(state) {
            return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
        }
        let outcome = self.apply_computed_fillet(symbol, &candidate)?;
        if outcome.disposition == IntentPlanDisposition::Accepted {
            let _ = state.publication_succeeded();
        }
        Ok(outcome)
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

    /// Applies only the exact native Profile Offset candidate currently
    /// rendered by the history-free projectional preview.
    ///
    /// # Errors
    ///
    /// Rejects incomplete/stale collector state or a preview mismatch without
    /// changing intent, accepted authority, history or collector state.
    pub fn apply_profile_offset_preview(
        &mut self,
        state: &mut OffsetAuthoringState,
        symbol: IntentKey,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        if state.candidate().is_none() {
            return Err(ProjectionalEditorError::AuthoringCandidateIncomplete);
        }
        if !self.offset_authoring_preview_matches(state) {
            return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
        }
        self.apply_profile_offset(state, symbol)
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

    /// Explicitly promotes one exact historical native Point declaration into
    /// the supported typed Sketch Point recipe.
    ///
    /// The declaration, output port, native reservation and point identity are
    /// retained in place, so downstream dependencies require no rewriting.
    /// The exact bootstrap payload remains sealed reconstruction provenance.
    ///
    /// # Errors
    ///
    /// Rejects a missing, non-Point, already-ejected, suppressed, malformed,
    /// incompletely owned or ambiguous declaration, or ordinary projectional
    /// publication failure. Rejection changes no durable session state.
    pub fn eject_bootstrap_point(
        &mut self,
        node: NodeId,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let patch = {
            let accepted = self
                .coordinator
                .accepted_materialization()
                .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
            crate::intent_bootstrap::bootstrap_point_ejection_patch(
                self.coordinator.intent(),
                &accepted.ownership,
                accepted.session.design_document(),
                node,
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

    /// Deletes the one declaration addressed by the coherent logical/native
    /// selection projection.
    ///
    /// Profile Offset uses its declaration-level multi-root closure so private
    /// Profile/OpenChain operands are removed even when the latest explicit
    /// intent is retained-invalid and absent from accepted native ownership.
    /// Every other declaration uses the ordinary exact dependent closure.
    ///
    /// # Errors
    ///
    /// Rejects empty or ambiguous selection, protected/malformed declaration
    /// state, or ordinary planning/materialization/publication failure without
    /// changing intent, accepted authority, selection, or history.
    pub fn delete_selected_declaration(
        &mut self,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        let node = self
            .selected_declaration
            .ok_or(ProjectionalEditorError::MissingDeleteSelection)?;
        let is_profile_offset = matches!(
            self.coordinator
                .intent()
                .graph()
                .node(node)
                .map(|node| &node.kind),
            Some(IntentNodeKind::Operation {
                operation: OperationKind::ProfileOffset
            })
        );
        if !is_profile_offset {
            return self.delete_declaration(node);
        }
        let patch = projectional_profile_offset_delete_node_patch(self.coordinator.intent(), node)?;
        self.apply_patch(patch)
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
        self.clear_authoring_previews();
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
        self.clear_authoring_previews();
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
        self.project_native_selection_to_declaration();
    }

    /// Applies one transient selection click without touching intent or history.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: Modifiers) {
        self.editor.select_item(item, modifiers);
        self.project_native_selection_to_declaration();
    }

    /// Returns the aggregate persistent role of the currently selected
    /// projectional geometry declarations.
    ///
    /// Repeated spans of one native curve and several output curves owned by
    /// one recipe count once. A selected curve whose exact intent owner does
    /// not expose the closed `role` field is reported as unsupported rather
    /// than being mistaken for the role used by future authoring.
    ///
    /// # Errors
    ///
    /// Returns an ownership error when a selected accepted curve has no one
    /// exact role-bearing declaration in the accepted materialization.
    pub fn selected_geometry_role_state(
        &self,
    ) -> Result<Option<GeometryRoleSelectionState>, ProjectionalEditorError> {
        let owners = self.selected_geometry_role_owners()?;
        let roles = owners
            .into_iter()
            .map(|node| {
                self.coordinator
                    .intent()
                    .graph()
                    .node(node)
                    .ok_or(ProjectionalEditorError::GeometryRoleOwnerMismatch)
                    .and_then(node_geometry_role)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut roles = roles.into_iter();
        let Some(first) = roles.next() else {
            return Ok(None);
        };
        if roles.all(|role| role == first) {
            return Ok(Some(match first {
                GeometryRole::Profile => GeometryRoleSelectionState::Profile,
                GeometryRole::Construction => GeometryRoleSelectionState::Construction,
            }));
        }
        Ok(Some(GeometryRoleSelectionState::Mixed))
    }

    /// Atomically toggles every selected projectional geometry declaration.
    /// An all-Construction selection becomes Profile; Profile or mixed
    /// selections become Construction, matching the accepted flat workbench.
    ///
    /// # Errors
    ///
    /// Rejects empty, protected, ambiguous, or non-role-bearing selection and
    /// ordinary exact-CAS/materialization failures without changing intent.
    pub fn toggle_selected_geometry_role(
        &mut self,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        if self
            .editor
            .selection()
            .iter()
            .any(|item| matches!(item, SelectionItem::Datum(_)))
        {
            return Err(ProjectionalEditorError::ProtectedGeometryRoleSelection);
        }
        let owners = self.selected_geometry_role_owners()?;
        if owners.is_empty() {
            return Err(ProjectionalEditorError::MissingGeometryRoleSelection);
        }
        let state = self
            .selected_geometry_role_state()?
            .ok_or(ProjectionalEditorError::MissingGeometryRoleSelection)?;
        let target = match state {
            GeometryRoleSelectionState::Construction => GeometryRole::Profile,
            GeometryRoleSelectionState::Profile | GeometryRoleSelectionState::Mixed => {
                GeometryRole::Construction
            }
        };
        let value = IntentLiteral::Enum(
            IntentKey::new(match target {
                GeometryRole::Profile => "profile",
                GeometryRole::Construction => "construction",
            })
            .map_err(|_| ProjectionalEditorError::InvalidBuiltInGeometryRoleSchema)?,
        );
        let field = IntentFieldKey(
            IntentKey::new("role")
                .map_err(|_| ProjectionalEditorError::InvalidBuiltInGeometryRoleSchema)?,
        );
        let patch = IntentPatch::new(
            self.coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            owners
                .into_iter()
                .map(|node| IntentPatchOperation::SetDefinitionField {
                    node,
                    field: field.clone(),
                    value: value.clone(),
                })
                .collect(),
        );
        self.apply_patch(patch)
    }

    fn selected_geometry_role_owners(
        &self,
    ) -> Result<std::collections::BTreeSet<NodeId>, ProjectionalEditorError> {
        let curves = self
            .editor
            .selection()
            .iter()
            .filter_map(|item| match item {
                SelectionItem::Curve(span) => Some(span.curve),
                SelectionItem::Point(_)
                | SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        if curves.is_empty() {
            return Ok(std::collections::BTreeSet::new());
        }
        let accepted = self
            .coordinator
            .accepted_materialization()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        curves
            .into_iter()
            .map(|curve| exact_role_owner(self.coordinator.intent(), &accepted.ownership, curve))
            .collect()
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
        self.project_native_selection_to_declaration();
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

    fn project_native_selection_to_declaration(&mut self) {
        let selection = self.editor.selection();
        if selection.is_empty() {
            self.selected_declaration = None;
            return;
        }
        let Some(accepted) = self.coordinator.accepted_materialization() else {
            self.selected_declaration = None;
            return;
        };
        let mut owners = std::collections::BTreeSet::new();
        for item in selection {
            let item_owners = accepted
                .ownership
                .nodes
                .iter()
                .filter(|owner| {
                    owner
                        .owned
                        .iter()
                        .any(|binding| native_binding_selects_item(*binding, *item))
                })
                .map(|owner| owner.node)
                .collect::<std::collections::BTreeSet<_>>();
            let mut item_owners = item_owners.into_iter();
            let Some(owner) = item_owners.next() else {
                self.selected_declaration = None;
                return;
            };
            if item_owners.next().is_some() {
                self.selected_declaration = None;
                return;
            }
            owners.insert(owner);
        }
        let mut owners = owners.into_iter();
        self.selected_declaration = owners
            .next()
            .filter(|_| owners.next().is_none())
            .and_then(|node| self.visible_declaration_owner(node));
    }

    fn visible_declaration_owner(&self, node: NodeId) -> Option<NodeId> {
        let graph = self.coordinator.intent().graph();
        let declaration = graph.node(node)?;
        if !matches!(declaration.kind, IntentNodeKind::Aggregate { .. }) {
            return Some(node);
        }
        let consumers = graph
            .nodes()
            .values()
            .filter(|candidate| candidate.dependencies().contains(&node))
            .collect::<Vec<_>>();
        let [consumer] = consumers.as_slice() else {
            return Some(node);
        };
        matches!(
            consumer.kind,
            IntentNodeKind::Operation {
                operation: OperationKind::ProfileOffset
            }
        )
        .then_some(consumer.id)
        .or(Some(node))
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

fn native_binding_selects_item(binding: IntentNativeBinding, item: SelectionItem) -> bool {
    match (binding, item) {
        (IntentNativeBinding::Point(candidate), SelectionItem::Point(point)) => candidate == point,
        (IntentNativeBinding::Curve(candidate), SelectionItem::Curve(span)) => {
            candidate == span.curve
        }
        (IntentNativeBinding::CurveSpan(candidate), SelectionItem::Curve(span)) => {
            candidate.curve == span.curve
        }
        (IntentNativeBinding::Constraint(candidate), SelectionItem::Constraint(constraint)) => {
            candidate == constraint
        }
        (IntentNativeBinding::Dimension(candidate), SelectionItem::Dimension(dimension)) => {
            candidate == dimension
        }
        (IntentNativeBinding::ComputedFeature(candidate), SelectionItem::Feature(feature)) => {
            candidate == feature
        }
        (IntentNativeBinding::ComputedFeature(candidate), SelectionItem::FeatureCorner(corner)) => {
            candidate == corner.feature
        }
        (
            IntentNativeBinding::ComputedFeatureCorner(candidate),
            SelectionItem::FeatureCorner(corner),
        ) => candidate == corner.corner,
        (
            IntentNativeBinding::Scalar(_)
            | IntentNativeBinding::Contact(_)
            | IntentNativeBinding::Source(_)
            | IntentNativeBinding::Parameter(_)
            | IntentNativeBinding::ExternalBinding(_)
            | IntentNativeBinding::Logical(_),
            _,
        )
        | (
            _,
            SelectionItem::Datum(_)
            | SelectionItem::Point(_)
            | SelectionItem::Curve(_)
            | SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_),
        ) => false,
    }
}

fn node_geometry_role(
    node: &geosolve_sketch_intent::IntentNode,
) -> Result<GeometryRole, ProjectionalEditorError> {
    let role = IntentFieldKey(
        IntentKey::new("role")
            .map_err(|_| ProjectionalEditorError::InvalidBuiltInGeometryRoleSchema)?,
    );
    if !node
        .kind
        .schema(u16::try_from(node.children.len()).unwrap_or(u16::MAX))
        .fields
        .iter()
        .any(|field| field.field == role)
    {
        return Err(ProjectionalEditorError::UnsupportedGeometryRoleOwner);
    }
    match node.fields.get(&role) {
        None => Ok(GeometryRole::Profile),
        Some(IntentLiteral::Enum(value)) if value.as_str() == "profile" => {
            Ok(GeometryRole::Profile)
        }
        Some(IntentLiteral::Enum(value)) if value.as_str() == "construction" => {
            Ok(GeometryRole::Construction)
        }
        Some(_) => Err(ProjectionalEditorError::InvalidGeometryRoleOwner),
    }
}

fn exact_role_owner(
    intent: &IntentSession,
    ownership: &crate::IntentMaterializationMap,
    curve: CurveId,
) -> Result<NodeId, ProjectionalEditorError> {
    let owners = ownership
        .nodes
        .iter()
        .filter(|owner| owner.owned.contains(&IntentNativeBinding::Curve(curve)))
        .map(|owner| owner.node)
        .collect::<std::collections::BTreeSet<_>>();
    let mut owners = owners.into_iter();
    let owner = owners
        .next()
        .ok_or(ProjectionalEditorError::GeometryRoleOwnerMismatch)?;
    if owners.next().is_some() {
        return Err(ProjectionalEditorError::GeometryRoleOwnerMismatch);
    }
    let node = intent
        .graph()
        .node(owner)
        .ok_or(ProjectionalEditorError::GeometryRoleOwnerMismatch)?;
    node_geometry_role(node)?;
    Ok(owner)
}

fn provisional_items(
    accepted: &crate::IntentMaterializationMap,
    preview: &crate::IntentMaterializationMap,
) -> Vec<SelectionItem> {
    let accepted = accepted
        .nodes
        .iter()
        .flat_map(|node| node.owned.iter().copied())
        .chain(accepted.ports.iter().map(|(_, binding)| *binding))
        .collect::<std::collections::BTreeSet<_>>();
    let mut items = preview
        .nodes
        .iter()
        .flat_map(|node| node.owned.iter().copied())
        .chain(preview.ports.iter().map(|(_, binding)| *binding))
        .filter(|binding| !accepted.contains(binding))
        .filter_map(|binding| match binding {
            IntentNativeBinding::Point(point) => Some(SelectionItem::Point(point)),
            IntentNativeBinding::CurveSpan(span) => Some(SelectionItem::Curve(span)),
            IntentNativeBinding::Constraint(constraint) => {
                Some(SelectionItem::Constraint(constraint))
            }
            IntentNativeBinding::Dimension(dimension) => Some(SelectionItem::Dimension(dimension)),
            IntentNativeBinding::ComputedFeature(feature) => Some(SelectionItem::Feature(feature)),
            IntentNativeBinding::ComputedFeatureCorner(_)
            | IntentNativeBinding::Scalar(_)
            | IntentNativeBinding::Curve(_)
            | IntentNativeBinding::Contact(_)
            | IntentNativeBinding::Source(_)
            | IntentNativeBinding::Parameter(_)
            | IntentNativeBinding::ExternalBinding(_)
            | IntentNativeBinding::Logical(_) => None,
        })
        .collect::<Vec<_>>();
    items.sort_unstable();
    items.dedup();
    items
}

fn provisional_profile_offset_dimension(
    preview: &ColdIntentMaterialization,
    items: &[SelectionItem],
) -> Result<DocumentDimensionId, ProjectionalEditorError> {
    let dimensions = items
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Dimension(dimension) => Some(*dimension),
            SelectionItem::Point(_)
            | SelectionItem::Curve(_)
            | SelectionItem::Constraint(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        })
        .filter(|dimension| {
            preview
                .session
                .design_document()
                .dimension(*dimension)
                .is_some_and(|dimension| {
                    matches!(
                        dimension.definition,
                        DocumentDimensionDefinition::ProfileOffset { .. }
                    )
                })
        })
        .collect::<Vec<_>>();
    let [dimension] = dimensions.as_slice() else {
        return Err(ProjectionalEditorError::AuthoringPreviewIdentityMismatch);
    };
    Ok(*dimension)
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
    #[error("select one exact Design declaration or owned sketch item to delete")]
    MissingDeleteSelection,
    #[error("no projectional curve with an editable persistent role is selected")]
    MissingGeometryRoleSelection,
    #[error("intrinsic reference geometry has a protected role")]
    ProtectedGeometryRoleSelection,
    #[error("a selected curve does not have one exact projectional role owner")]
    GeometryRoleOwnerMismatch,
    #[error("the selected declaration does not expose an editable geometry role")]
    UnsupportedGeometryRoleOwner,
    #[error("the selected declaration contains an invalid geometry role")]
    InvalidGeometryRoleOwner,
    #[error("the built-in geometry-role schema is invalid")]
    InvalidBuiltInGeometryRoleSchema,
    #[error("the accepted scene does not match its retained native authority")]
    SceneAuthorityMismatch,
    #[error("computed scene evaluation rejected: {0}")]
    ComputedScene(String),
    #[error("computed scene evaluation stopped before publication")]
    ComputedSceneStopped,
    #[error("the projectional authoring candidate is incomplete")]
    AuthoringCandidateIncomplete,
    #[error("the projectional authoring preview was independently rejected")]
    AuthoringPreviewRejected,
    #[error("the projectional authoring preview does not match its typed candidate identity")]
    AuthoringPreviewIdentityMismatch,
    #[error("Profile Offset authoring is unavailable: {0}")]
    OffsetAuthoring(String),
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
