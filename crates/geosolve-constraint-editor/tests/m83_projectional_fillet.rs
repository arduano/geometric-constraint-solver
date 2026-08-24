// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ActivePointerGestureKind, ColdIntentMaterializer, EditorEffect, EditorHoverTarget,
    FeatureAuthoringCandidate, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentNativeBinding, IntentNativeWritableLeaf,
    Modifiers, PickTolerance, PointerInput, ProjectionalEditorError, ProjectionalEditorSession,
    ProjectionalIntentCoordinator, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_features::{
    ComputedFeatureAuthoringSnapshot, ComputedFeatureDefinition, ComputedFeatureEvaluationState,
};
use geosolve_sketch_intent::{
    CellTarget, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey, IntentLiteral,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit,
    LeafField, PatchPortRef,
};

const DOCUMENT_RAW: u128 = 0x8300_f111_0000_0001;

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn point(name: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(name),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        coordinate(position[1]),
    )
}

fn point_alias(alias: &str) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(alias),
        selector: selector(IntentPortRole::Primary, 0),
    }
}

fn line(name: &str, start: &str, end: &str, direction: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(name),
    )
    .with_input(InputSlot::new(InputRole::Point, 0), point_alias(start))
    .with_input(InputSlot::new(InputRole::Point, 1), point_alias(end))
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point(direction),
    )
}

fn fixture() -> (ProjectionalEditorSession, Viewport) {
    let document = DocumentId(PersistentId::from_u128(DOCUMENT_RAW));
    let mut coordinator = ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(0x8300_f111),
        ColdIntentMaterializer::with_default_policy(document, 10.0).unwrap(),
    )
    .unwrap();
    coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("start"),
                    draft: Box::new(point("point.start", [0.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("corner"),
                    draft: Box::new(point("point.corner", [4.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("end"),
                    draft: Box::new(point("point.end", [4.0, 4.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("first"),
                    draft: Box::new(line("line.first", "start", "corner", [1.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("second"),
                    draft: Box::new(line("line.second", "corner", "end", [0.0, 1.0])),
                    cell: None,
                },
            ],
        ))
        .unwrap();
    (
        ProjectionalEditorSession::new(coordinator),
        Viewport::new([800.0, 600.0], [2.0, 2.0], 50.0).unwrap(),
    )
}

fn fillet_candidate(
    session: &ProjectionalEditorSession,
    viewport: Viewport,
) -> FeatureAuthoringCandidate {
    let native = session.coordinator().presentation_session().unwrap();
    let snapshot = ComputedFeatureAuthoringSnapshot::capture(native).unwrap();
    let scene = session.scene(viewport, 0.5).unwrap();
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(
            &snapshot,
            snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    assert!(matches!(
        state.pick_at(
            &snapshot,
            snapshot.sketch_document(),
            &scene,
            viewport.model_to_screen([3.0, 0.0]),
            PickTolerance::default(),
        ),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    match state.pick_at(
        &snapshot,
        snapshot.sketch_document(),
        &scene,
        viewport.model_to_screen([4.0, 1.0]),
        PickTolerance::default(),
    ) {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
        | FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => panic!("expected a complete Fillet candidate, got {other:?}"),
    }
}

fn create_fillet(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
) -> geosolve_sketch_intent::NodeId {
    let candidate = fillet_candidate(session, viewport);
    let expected_corner = candidate.persistent_corners()[0];
    let outcome = session
        .apply_computed_fillet(key("Fillet 1"), &candidate)
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let [feature] = accepted.features.features() else {
        panic!("one computed feature must be materialized")
    };
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    assert_eq!(fillet.corners.len(), 1);
    assert_eq!(fillet.corners[0].without_id(), expected_corner);
    let [evaluation] = accepted.computed.feature_evaluations() else {
        panic!("one computed feature evaluation must be published")
    };
    assert!(matches!(
        evaluation.state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(accepted.computed.edges().len(), 3);
    assert!(accepted.validation.all_active_features_current);
    assert_eq!(accepted.validation.feature_count, 1);
    assert_eq!(accepted.validation.computed_edge_count, 3);
    let node = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| matches!(node.kind, IntentNodeKind::ComputedFeature { .. }))
        .unwrap();
    let feature_port = node
        .port_by_selector(selector(IntentPortRole::Feature, 0))
        .unwrap()
        .as_ref(node.id);
    assert_eq!(
        accepted.ownership.port(feature_port),
        Some(IntentNativeBinding::ComputedFeature(feature.id))
    );
    let corner_port = node
        .ports
        .values()
        .find(|port| port.kind == geosolve_sketch_intent::IntentPortKind::FeatureCorner)
        .unwrap()
        .as_ref(node.id);
    assert_eq!(
        accepted.ownership.port(corner_port),
        Some(IntentNativeBinding::ComputedFeatureCorner(
            fillet.corners[0].id
        ))
    );
    assert_eq!(
        session.scene(viewport, 0.5).unwrap().computed_curves.len(),
        1
    );
    node.id
}

fn complete_projectional_fillet_authoring(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
) -> FeatureAuthoringState {
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        session
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..FeatureAuthoringOptions::default()
                },
                &[],
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).unwrap();
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([3.0, 0.0]),
                PickTolerance::default(),
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([4.0, 1.0]),
                PickTolerance::default(),
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    assert!(session.feature_authoring_preview_matches(&state));
    state
}

fn assert_independently_valid(session: &ProjectionalEditorSession) {
    let accepted = session.coordinator().accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert!(accepted.validation.all_active_features_current);
}

const fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers {
            shift: false,
            control: false,
            command: false,
        },
    }
}

#[test]
fn computed_fillet_is_exactly_cold_reconstructed_and_composed() {
    let (mut session, viewport) = fixture();
    create_fillet(&mut session, viewport);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let feature_json = accepted.features.to_json().unwrap();
    let ownership = accepted.ownership.clone();
    let validation = accepted.validation.clone();
    let intent = session.coordinator().intent().clone();

    let restored = ProjectionalEditorSession::restore(
        intent,
        DocumentId(PersistentId::from_u128(DOCUMENT_RAW)),
        10.0,
    )
    .unwrap();
    let cold = restored.coordinator().accepted_materialization().unwrap();
    assert_eq!(cold.features.to_json().unwrap(), feature_json);
    assert_eq!(cold.ownership, ownership);
    assert_eq!(cold.validation, validation);
    assert_eq!(cold.computed.edges().len(), 3);
    assert_eq!(
        restored.scene(viewport, 0.5).unwrap().computed_curves.len(),
        1
    );
}

#[test]
fn organization_reorder_does_not_reconstruct_or_renumber_computed_fillet() {
    let (mut session, viewport) = fixture();
    let node = create_fillet(&mut session, viewport);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let feature_json = accepted.features.to_json().unwrap();
    let computed_input = accepted.computed.input();
    let evidence = accepted.evidence.clone();
    let feature_id = accepted.features.features()[0].id;
    let ComputedFeatureDefinition::FilletSet(fillet) = &accepted.features.features()[0].definition;
    let corner_id = fillet.corners[0].id;

    let first = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateCell {
                alias: key("first-cell"),
                name: key("First cell"),
                before: None,
            }],
        ))
        .unwrap();
    assert_eq!(first.disposition, IntentPlanDisposition::OrganizationOnly);
    let first_cell = first.aliases.cell(&key("first-cell")).unwrap();
    let second = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateCell {
                alias: key("second-cell"),
                name: key("Second cell"),
                before: None,
            }],
        ))
        .unwrap();
    assert_eq!(second.disposition, IntentPlanDisposition::OrganizationOnly);
    let second_cell = second.aliases.cell(&key("second-cell")).unwrap();

    let moved = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::MoveDeclaration {
                node,
                cell: CellTarget::Stable { cell: second_cell },
                before: None,
            }],
        ))
        .unwrap();
    assert_eq!(moved.disposition, IntentPlanDisposition::OrganizationOnly);
    let default_cell = session.coordinator().intent().organization().cell_order()[0];
    let reordered = session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::ReorderCells {
                exact_order: vec![second_cell, default_cell, first_cell],
            }],
        ))
        .unwrap();
    assert_eq!(
        reordered.disposition,
        IntentPlanDisposition::OrganizationOnly
    );

    let after = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(after.features.to_json().unwrap(), feature_json);
    assert_eq!(after.computed.input(), computed_input);
    assert_eq!(after.evidence, evidence);
    let ComputedFeatureDefinition::FilletSet(fillet) = &after.features.features()[0].definition;
    assert_eq!(after.features.features()[0].id, feature_id);
    assert_eq!(fillet.corners[0].id, corner_id);
}

#[test]
fn delete_undo_redo_restore_the_same_feature_and_corner_ids() {
    let (mut session, viewport) = fixture();
    let node = create_fillet(&mut session, viewport);
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let feature_id = accepted.features.features()[0].id;
    let ComputedFeatureDefinition::FilletSet(fillet) = &accepted.features.features()[0].definition;
    let corner_id = fillet.corners[0].id;

    session.delete_declaration(node).unwrap();
    assert!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .features
            .features()
            .is_empty()
    );
    session.undo().unwrap().unwrap();
    let restored = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(restored.features.features()[0].id, feature_id);
    let ComputedFeatureDefinition::FilletSet(fillet) = &restored.features.features()[0].definition;
    assert_eq!(fillet.corners[0].id, corner_id);
    session.redo().unwrap().unwrap();
    assert!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .features
            .features()
            .is_empty()
    );
}

#[test]
fn rejected_radius_is_retained_over_the_exact_prior_computed_scene() {
    let (mut session, viewport) = fixture();
    create_fillet(&mut session, viewport);
    let prior = session.coordinator().accepted_materialization().unwrap();
    let feature = prior.features.features()[0].id;
    let prior_json = prior.features.to_json().unwrap();
    let prior_evidence = prior.evidence.clone();

    let outcome = session.edit_computed_fillet_radius(feature, 1.0e6).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::RetainedFailed);
    let retained = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(retained.features.to_json().unwrap(), prior_json);
    assert_eq!(retained.evidence, prior_evidence);
    assert_eq!(
        session.scene(viewport, 0.5).unwrap().computed_curves.len(),
        1
    );
    let node = session
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| matches!(node.kind, IntentNodeKind::ComputedFeature { .. }))
        .unwrap();
    let radius = node
        .fields
        .iter()
        .find(|(field, _)| field.0.as_str() == "radius")
        .map(|(_, value)| value)
        .unwrap();
    assert_eq!(
        radius,
        &IntentLiteral::Quantity {
            value: 1.0e6,
            unit: IntentUnit::Length,
        }
    );
    session.undo().unwrap().unwrap();
    let undone = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(undone.features.features()[0].id, feature);
    let ComputedFeatureDefinition::FilletSet(undone_fillet) =
        &undone.features.features()[0].definition;
    assert_eq!(undone_fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        session.scene(viewport, 0.5).unwrap().computed_curves.len(),
        1
    );
}

#[test]
fn radius_drag_previews_without_history_then_publishes_one_undoable_transaction() {
    let (mut session, viewport) = fixture();
    create_fillet(&mut session, viewport);
    let scene = session.scene(viewport, 0.5).unwrap();
    let rail = scene.fillet_affordances[0].radius_rail;
    let feature = rail.owner.feature;
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let evidence_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();

    session
        .pointer_down(&scene, pointer(71, rail.screen_grip))
        .unwrap();
    assert_eq!(
        session.editor().active_pointer_gesture().unwrap().kind,
        ActivePointerGestureKind::FilletRadius
    );
    let origin = viewport.screen_to_model(rail.screen_grip);
    let target = viewport.model_to_screen([
        0.25f64.mul_add(rail.model_derivative[0], origin[0]),
        0.25f64.mul_add(rail.model_derivative[1], origin[1]),
    ]);
    let effects = session.pointer_move(&scene, pointer(71, target)).unwrap();
    effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewComputedFeatureRadius { radius, .. } => Some(*radius),
            _ => None,
        })
        .expect("the finite same-branch sample must be independently accepted");
    let target = viewport.model_to_screen([
        0.4f64.mul_add(rail.model_derivative[0], origin[0]),
        0.4f64.mul_add(rail.model_derivative[1], origin[1]),
    ]);
    let radius = session
        .pointer_move(&scene, pointer(71, target))
        .unwrap()
        .into_iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewComputedFeatureRadius { radius, .. } => Some(radius),
            _ => None,
        })
        .expect("the newer independently accepted sample must replace the prior preview");
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    assert_eq!(
        preview_scene.computed_curves[0].radius.to_bits(),
        radius.to_bits()
    );

    let outcome = session
        .pointer_up(&preview_scene, pointer(71, target))
        .unwrap();
    assert_eq!(
        outcome.transaction.unwrap().disposition,
        IntentPlanDisposition::Accepted
    );
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let ComputedFeatureDefinition::FilletSet(fillet) =
        &accepted.features.feature(feature).unwrap().definition;
    assert_eq!(fillet.radius.to_bits(), radius.to_bits());
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);

    session.undo().unwrap().unwrap();
    let undone = session.coordinator().accepted_materialization().unwrap();
    let ComputedFeatureDefinition::FilletSet(fillet) =
        &undone.features.feature(feature).unwrap().definition;
    assert_eq!(fillet.radius.to_bits(), 1.0_f64.to_bits());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one rollback matrix keeps cancel, stale, invalid-domain, and non-finite terminal invariants contiguous"
)]
fn radius_drag_cancel_and_invalid_release_preserve_authority_and_history() {
    let (mut session, viewport) = fixture();
    create_fillet(&mut session, viewport);
    let scene = session.scene(viewport, 0.5).unwrap();
    let rail = scene.fillet_affordances[0].radius_rail;
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let evidence_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let origin = viewport.screen_to_model(rail.screen_grip);
    let valid = viewport.model_to_screen([
        0.2f64.mul_add(rail.model_derivative[0], origin[0]),
        0.2f64.mul_add(rail.model_derivative[1], origin[1]),
    ]);

    session
        .pointer_down(&scene, pointer(72, rail.screen_grip))
        .unwrap();
    session.pointer_move(&scene, pointer(72, valid)).unwrap();
    assert_ne!(
        session.scene(viewport, 0.5).unwrap().computed_curves[0]
            .radius
            .to_bits(),
        1.0_f64.to_bits()
    );
    assert!(
        session
            .cancel_interaction()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::RestoreComputedFeatureRadius { .. }))
    );
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session.scene(viewport, 0.5).unwrap().computed_curves[0]
            .radius
            .to_bits(),
        1.0_f64.to_bits()
    );

    let scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_down(&scene, pointer(73, rail.screen_grip))
        .unwrap();
    session.pointer_move(&scene, pointer(73, valid)).unwrap();
    assert!(matches!(
        session.pointer_up(&scene, pointer(73, valid)),
        Err(geosolve_constraint_editor::ProjectionalEditorError::FilletRadiusDragRouteMismatch)
    ));
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);

    let scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_down(&scene, pointer(74, rail.screen_grip))
        .unwrap();
    session.pointer_move(&scene, pointer(74, valid)).unwrap();
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    let invalid = viewport.model_to_screen([
        (-2.0f64).mul_add(rail.model_derivative[0], origin[0]),
        (-2.0f64).mul_add(rail.model_derivative[1], origin[1]),
    ]);
    assert!(
        session
            .pointer_move(&preview_scene, pointer(74, invalid))
            .unwrap()
            .is_empty()
    );
    let outcome = session
        .pointer_up(&preview_scene, pointer(74, invalid))
        .unwrap();
    assert!(outcome.transaction.is_none());
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );
    assert_eq!(
        session.scene(viewport, 0.5).unwrap().computed_curves[0]
            .radius
            .to_bits(),
        1.0_f64.to_bits()
    );

    let scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_down(&scene, pointer(75, rail.screen_grip))
        .unwrap();
    let nonfinite = ScreenPoint {
        x: f64::NAN,
        y: rail.screen_grip.y,
    };
    let outcome = session.pointer_up(&scene, pointer(75, nonfinite)).unwrap();
    assert!(outcome.transaction.is_none());
    assert!(session.editor().active_pointer_gesture().is_none());
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps cold preview, Apply, allocator high-water, Undo, and independent validation evidence contiguous"
)]
fn projectional_fillet_authoring_previews_applies_once_and_undo_restores_exact_base() {
    let (mut session, viewport) = fixture();
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let accepted_before = session.coordinator().accepted_materialization().unwrap();
    let ownership_before = accepted_before.ownership.clone();
    let evidence_before = accepted_before.evidence.clone();
    assert!(accepted_before.features.features().is_empty());
    let feature_allocator_before = accepted_before.features.allocator_high_water();
    let document_before = accepted_before.session.design_document().clone();
    let mut state = FeatureAuthoringState::default();

    assert!(matches!(
        session
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..FeatureAuthoringOptions::default()
                },
                &[],
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).unwrap();
    session
        .pointer_move_feature_authoring(
            &state,
            &scene,
            pointer(801, viewport.model_to_screen([3.0, 0.0])),
            PickTolerance::default(),
        )
        .unwrap();
    assert!(matches!(
        session.editor().hover_state().target,
        Some(EditorHoverTarget::Geometry(SelectionItem::Curve(_)))
    ));
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([3.0, 0.0]),
                PickTolerance::default(),
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    assert!(session.feature_authoring_preview_item().is_none());
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([4.0, 1.0]),
                PickTolerance::default(),
                key("Fillet 1"),
            )
            .unwrap(),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    assert!(session.feature_authoring_preview_matches(&state));
    assert!(matches!(
        session.feature_authoring_preview_item(),
        Some(SelectionItem::Feature(_))
    ));
    assert_eq!(session.coordinator().intent().identity(), identity_before);
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    assert_eq!(preview_scene.computed_curves.len(), 1);
    assert!(preview_scene.computed_curves[0].radius.is_finite());

    let outcome = session
        .apply_computed_fillet_preview(&mut state, key("Fillet 1"))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert!(state.active_tool().is_none());
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    let accepted = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(accepted.features.features().len(), 1);
    let removed_feature = accepted.features.features()[0].id;
    let ComputedFeatureDefinition::FilletSet(removed_fillet) =
        &accepted.features.features()[0].definition;
    let removed_corner = removed_fillet.corners[0].id;
    let intent_allocator_after_apply = session.coordinator().intent().allocator_high_water();
    assert_independently_valid(&session);

    session.undo().unwrap().unwrap();
    let undone = session.coordinator().accepted_materialization().unwrap();
    assert_eq!(undone.session.design_document(), &document_before);
    assert_eq!(undone.ownership.nodes, ownership_before.nodes);
    assert_eq!(undone.ownership.ports, ownership_before.ports);
    assert_eq!(undone.ownership.reservations, ownership_before.reservations);
    assert_eq!(
        undone.ownership.writable_leaves,
        ownership_before.writable_leaves
    );
    assert_eq!(undone.ownership.aggregates, ownership_before.aggregates);
    assert!(undone.features.features().is_empty());
    assert_eq!(
        undone.features.allocator_high_water(),
        feature_allocator_before,
        "Undo restores the semantic empty feature payload"
    );
    assert_eq!(
        session.coordinator().intent().allocator_high_water(),
        intent_allocator_after_apply,
        "Undo retains the durable intent allocator high-water without requiring feature revision reuse"
    );
    assert!(
        session
            .scene(viewport, 0.5)
            .unwrap()
            .computed_curves
            .is_empty()
    );
    assert_independently_valid(&session);

    let mut divergent = complete_projectional_fillet_authoring(&mut session, viewport);
    session
        .apply_computed_fillet_preview(&mut divergent, key("Fillet divergent"))
        .unwrap();
    let republished = session.coordinator().accepted_materialization().unwrap();
    let republished_feature = &republished.features.features()[0];
    let ComputedFeatureDefinition::FilletSet(republished_fillet) = &republished_feature.definition;
    assert!(republished_feature.id.raw() > removed_feature.raw());
    assert!(republished_fillet.corners[0].id.raw() > removed_corner.raw());
    assert_independently_valid(&session);
}

#[test]
fn projectional_fillet_authoring_radius_drag_is_history_free_until_apply() {
    let (mut session, viewport) = fixture();
    let mut state = complete_projectional_fillet_authoring(&mut session, viewport);
    let history_before = session.coordinator().intent().undo_len();
    let canonical_before = session.coordinator().intent().to_canonical_json().unwrap();
    let scene = session.scene(viewport, 0.5).unwrap();
    let rail = scene.fillet_affordances[0].radius_rail;
    assert!(
        session
            .pointer_down_feature_authoring_radius(
                &state,
                &scene,
                pointer(802, rail.screen_grip),
                PickTolerance::default(),
                key("Fillet 1"),
            )
            .unwrap()
            .is_some()
    );
    assert!(session.feature_authoring_radius_drag_active());
    assert_eq!(
        session.editor().active_pointer_gesture().unwrap().kind,
        ActivePointerGestureKind::FilletRadius
    );
    let origin = viewport.screen_to_model(rail.screen_grip);
    let target = viewport.model_to_screen([
        0.25f64.mul_add(rail.model_derivative[0], origin[0]),
        0.25f64.mul_add(rail.model_derivative[1], origin[1]),
    ]);
    let effects = session
        .pointer_move_feature_authoring_radius(&mut state, &scene, pointer(802, target))
        .unwrap();
    let radius = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::PreviewComputedFeatureRadius { radius, .. } => Some(*radius),
            _ => None,
        })
        .unwrap();
    assert_ne!(radius.to_bits(), 1.0_f64.to_bits());
    let preview_scene = session.scene(viewport, 0.5).unwrap();
    assert_eq!(
        preview_scene.computed_curves[0].radius.to_bits(),
        radius.to_bits()
    );
    assert!(
        session
            .pointer_up_feature_authoring_radius(&mut state, &preview_scene, pointer(802, target),)
            .unwrap()
    );
    assert!(!session.feature_authoring_radius_drag_active());
    assert!(session.feature_authoring_preview_matches(&state));
    assert_eq!(session.coordinator().intent().undo_len(), history_before);
    assert_eq!(
        session.coordinator().intent().to_canonical_json().unwrap(),
        canonical_before
    );

    session
        .apply_computed_fillet_preview(&mut state, key("Fillet 1"))
        .unwrap();
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    let accepted = session.coordinator().accepted_materialization().unwrap();
    let ComputedFeatureDefinition::FilletSet(fillet) = &accepted.features.features()[0].definition;
    assert_eq!(fillet.radius.to_bits(), radius.to_bits());
    assert_independently_valid(&session);
}

#[test]
fn projectional_fillet_invalid_stale_and_noop_transitions_are_atomic() {
    let (mut session, viewport) = fixture();
    let mut empty = FeatureAuthoringState::default();
    let canonical_empty = session.coordinator().intent().to_canonical_json().unwrap();
    assert!(matches!(
        session.apply_computed_fillet_preview(&mut empty, key("Fillet noop")),
        Err(ProjectionalEditorError::AuthoringCandidateIncomplete)
    ));
    assert_eq!(
        session.coordinator().intent().to_canonical_json().unwrap(),
        canonical_empty
    );

    let old_state = complete_projectional_fillet_authoring(&mut session, viewport);
    let start = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .points()
        .iter()
        .find(|point| point.label == "point.start")
        .unwrap()
        .id;
    let x_leaf = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .writable_leaf(IntentNativeWritableLeaf::PointX { point: start })
        .unwrap();
    session
        .apply_patch(IntentPatch::new(
            session.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::SetInstanceLeaf {
                leaf: x_leaf,
                value: coordinate(-1.0),
            }],
        ))
        .unwrap();
    let mut fresh_state = complete_projectional_fillet_authoring(&mut session, viewport);
    let canonical_before = session.coordinator().intent().to_canonical_json().unwrap();
    let evidence_before = session
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let preview_before = session.scene(viewport, 0.5).unwrap().computed_curves[0].clone();
    let fresh_before = fresh_state.clone();

    assert!(matches!(
        session
            .transact_feature_authoring_radius(&mut fresh_state, -1.0, key("Fillet 1"))
            .unwrap(),
        FeatureAuthoringOutcome::Warning(_)
    ));
    assert_eq!(fresh_state, fresh_before);
    assert!(session.feature_authoring_preview_matches(&fresh_state));

    let mut stale_state = old_state;
    assert!(matches!(
        session
            .transact_feature_authoring_radius(&mut stale_state, 0.75, key("Fillet 1"))
            .unwrap(),
        FeatureAuthoringOutcome::Warning(_)
    ));
    assert!(session.feature_authoring_preview_matches(&fresh_state));
    let preview_after = session.scene(viewport, 0.5).unwrap().computed_curves[0].clone();
    assert_eq!(preview_after, preview_before);
    assert_eq!(
        session.coordinator().intent().to_canonical_json().unwrap(),
        canonical_before
    );
    assert_eq!(
        session
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        evidence_before
    );
}
