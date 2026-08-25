// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    EditorBootstrapDeclaration, KeyedReconcileState, ProjectKey, SemanticSymbol,
    initialize_code_project_from_editor, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSession,
    IntentSessionId, IntentUnit, LeafField,
};

fn length(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn gui_rectangle() -> (ProjectionalEditorSession, geosolve_sketch_intent::NodeId) {
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_b007)).unwrap();
    let mut editor = ProjectionalEditorSession::restore(
        intent,
        DocumentId(PersistentId::from_u128(0x84_b007)),
        1.0,
    )
    .unwrap();
    let first = IntentPortSelector::Node {
        role: IntentPortRole::Corner,
        index: 0,
    };
    let third = IntentPortSelector::Node {
        role: IntentPortRole::Corner,
        index: 2,
    };
    let alias = geosolve_sketch_intent::IntentKey::new("gui-frame").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
        },
        alias.clone(),
    )
    .with_instance_leaf(first, LeafField::X, length(-8.0))
    .with_instance_leaf(first, LeafField::Y, length(3.0))
    .with_instance_leaf(third, LeafField::X, length(52.0))
    .with_instance_leaf(third, LeafField::Y, length(38.0));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    let node = outcome.aliases.node(&alias).unwrap();
    (editor, node)
}

#[test]
fn accepted_gui_rectangle_initializes_truthful_managed_source_and_rematerializes() {
    let (editor, node) = gui_rectangle();
    let project = initialize_code_project_from_editor(
        &editor,
        ProjectKey("bootstrap-round-trip".into()),
        &[EditorBootstrapDeclaration::new(
            node,
            SemanticSymbol("frame".into()),
        )],
    )
    .unwrap();
    assert!(
        project
            .managed
            .source
            .contains("const frame = $.geometry.rectangle")
    );
    assert!(project.managed.source.contains("lowerLeft: [-8.0, 3.0]"));
    assert!(project.managed.source.contains("upperRight: [52.0, 38.0]"));
    assert!(project.custom_files.is_empty());
    assert!(project.artifacts.is_empty());

    let desired = required_generated_members(&project).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_b008),
        DocumentId(PersistentId::from_u128(0x84_b008)),
        1.0,
    )
    .unwrap();
    let document = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let positions = document
        .points()
        .iter()
        .map(|point| point.position)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 4);
    for expected in [[-8.0, 3.0], [52.0, 3.0], [52.0, 38.0], [-8.0, 38.0]] {
        let expected_bits = expected.map(f64::to_bits);
        assert!(
            positions
                .iter()
                .any(|actual| actual.map(f64::to_bits) == expected_bits)
        );
    }
}

#[test]
fn conversion_rejects_unsupported_gui_recipe_instead_of_inventing_history() {
    let (mut editor, _) = gui_rectangle();
    let alias = geosolve_sketch_intent::IntentKey::new("gui-point").unwrap();
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        alias.clone(),
    )
    .with_instance_leaf(selector, LeafField::X, length(1.0))
    .with_instance_leaf(selector, LeafField::Y, length(2.0));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    let point = outcome.aliases.node(&alias).unwrap();
    let error = initialize_code_project_from_editor(
        &editor,
        ProjectKey("unsupported".into()),
        &[EditorBootstrapDeclaration::new(
            point,
            SemanticSymbol("point".into()),
        )],
    )
    .unwrap_err();
    assert!(error.to_string().contains("unsupported recipe"));
}
