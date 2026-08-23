// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentNativeBinding, ProjectionalCoordinatorError,
    ProjectionalIntentCoordinator,
};
use geosolve_sketch::{DocumentId, OperationControl, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::Node { role, index: 0 }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn point(position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("point.main"),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary),
        LeafField::Y,
        coordinate(position[1]),
    )
}

fn coordinator(raw: u128) -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn create_point(
    coordinator: &mut ProjectionalIntentCoordinator,
    position: [f64; 2],
) -> geosolve_sketch::DesignPointId {
    let patch = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point(position)),
            cell: None,
        }],
    );
    let outcome = coordinator.apply_patch(patch).unwrap();
    let port = outcome
        .aliases
        .port(&key("point"), selector(IntentPortRole::Primary))
        .unwrap();
    let IntentNativeBinding::Point(point) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
    else {
        panic!("primary point port must bind one native point")
    };
    point
}

fn accepted_position(
    coordinator: &ProjectionalIntentCoordinator,
    point: geosolve_sketch::DesignPointId,
) -> [f64; 2] {
    coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .point(point)
        .unwrap()
        .position
}

fn assert_pair(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

#[test]
fn one_intent_history_owns_patch_drag_undo_redo_and_cold_restore() {
    let mut coordinator = coordinator(0x8300_2001);
    let point = create_point(&mut coordinator, [1.0, 2.0]);
    assert_pair(accepted_position(&coordinator, point), [1.0, 2.0]);
    assert_eq!(coordinator.intent().history_projection().applied.len(), 1);

    let before_preview = coordinator.intent().identity();
    coordinator.begin_point_drag(7, point).unwrap();
    let preview = coordinator
        .preview_point_drag(7, 1, [4.0, -3.0], OperationControl::unlimited())
        .unwrap()
        .unwrap();
    assert_pair(preview.accepted_position, [4.0, -3.0]);
    assert_eq!(coordinator.intent().identity(), before_preview);
    assert_pair(
        coordinator
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .point(point)
            .unwrap()
            .position,
        [4.0, -3.0],
    );

    let outcome = coordinator.finish_point_drag(7, 1).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_pair(accepted_position(&coordinator, point), [4.0, -3.0]);
    assert_eq!(coordinator.intent().history_projection().applied.len(), 2);

    coordinator.undo().unwrap().unwrap();
    assert_pair(accepted_position(&coordinator, point), [1.0, 2.0]);
    coordinator.redo().unwrap().unwrap();
    assert_pair(accepted_position(&coordinator, point), [4.0, -3.0]);

    let json = coordinator.intent().to_canonical_json().unwrap();
    let intent = IntentSession::from_json(&json).unwrap();
    let restored = ProjectionalIntentCoordinator::restore(
        intent,
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(0x8300_2001_u128 << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_pair(accepted_position(&restored, point), [4.0, -3.0]);
    assert_eq!(
        restored.intent().to_canonical_json().unwrap(),
        coordinator.intent().to_canonical_json().unwrap()
    );
}

#[test]
fn retained_failure_and_organization_keep_the_exact_accepted_scene() {
    let mut coordinator = coordinator(0x8300_2002);
    let point = create_point(&mut coordinator, [2.0, 5.0]);
    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let node = *coordinator.intent().graph().nodes().keys().next().unwrap();

    let rename = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::RenameNode {
            node,
            name: key("Renamed point"),
        }],
    );
    assert_eq!(
        coordinator.apply_patch(rename).unwrap().disposition,
        IntentPlanDisposition::OrganizationOnly
    );
    assert_eq!(
        coordinator.accepted_materialization().unwrap().evidence,
        accepted_before
    );

    let retained_invalid = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key("annotation"),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Annotation,
                key("annotation.unsupported"),
            )),
            cell: None,
        }],
    );
    assert_eq!(
        coordinator
            .apply_patch(retained_invalid)
            .unwrap()
            .disposition,
        IntentPlanDisposition::RetainedFailed
    );
    assert_pair(accepted_position(&coordinator, point), [2.0, 5.0]);
    assert_eq!(
        coordinator.accepted_materialization().unwrap().evidence,
        accepted_before
    );

    coordinator.undo().unwrap().unwrap();
    assert_pair(accepted_position(&coordinator, point), [2.0, 5.0]);
    coordinator.redo().unwrap().unwrap();
    assert_pair(accepted_position(&coordinator, point), [2.0, 5.0]);
}

#[test]
fn point_preview_rejects_stale_samples_and_cancellation_adds_no_history() {
    let mut coordinator = coordinator(0x8300_2003);
    let point = create_point(&mut coordinator, [0.0, 0.0]);
    let history = coordinator.intent().history_projection();
    let identity = coordinator.intent().identity();
    coordinator.begin_point_drag(11, point).unwrap();
    coordinator
        .preview_point_drag(11, 3, [1.0, 1.0], OperationControl::unlimited())
        .unwrap();
    assert!(matches!(
        coordinator.preview_point_drag(11, 2, [2.0, 2.0], OperationControl::unlimited()),
        Err(ProjectionalCoordinatorError::StaleDragSample)
    ));
    coordinator.cancel_point_drag();
    assert_eq!(coordinator.intent().identity(), identity);
    assert_eq!(coordinator.intent().history_projection(), history);
    assert_pair(accepted_position(&coordinator, point), [0.0, 0.0]);
}

#[test]
fn deleting_one_retained_invalid_declaration_restores_accepted_current_authority() {
    let mut coordinator = coordinator(0x8300_2004);
    let point = create_point(&mut coordinator, [6.0, -2.0]);
    let patch = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key("invalid"),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Annotation,
                key("unsupported.annotation"),
            )),
            cell: None,
        }],
    );
    let invalid = coordinator
        .apply_patch(patch)
        .unwrap()
        .aliases
        .node(&key("invalid"))
        .unwrap();
    assert_eq!(coordinator.intent().graph().nodes().len(), 2);
    assert_pair(accepted_position(&coordinator, point), [6.0, -2.0]);

    let deleted = coordinator.delete_declaration(invalid).unwrap();
    assert_eq!(deleted.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(coordinator.intent().graph().nodes().len(), 1);
    assert_pair(accepted_position(&coordinator, point), [6.0, -2.0]);
    coordinator.undo().unwrap().unwrap();
    assert!(coordinator.intent().graph().node(invalid).is_some());
    assert_pair(accepted_position(&coordinator, point), [6.0, -2.0]);
}
