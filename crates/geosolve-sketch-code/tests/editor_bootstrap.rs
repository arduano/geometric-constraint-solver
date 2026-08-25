// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    EditorBootstrapDeclaration, KeyedReconcileState, ManagedValue, ProjectKey, SemanticSymbol,
    initialize_code_project_from_editor, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector,
    IntentSession, IntentSessionId, IntentUnit, LeafField, PatchPortRef,
};

fn length(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn gui_rectangle() -> (ProjectionalEditorSession, geosolve_sketch_intent::NodeId) {
    gui_rectangle_between([-8.0, 3.0], [52.0, 38.0])
}

fn gui_rectangle_between(
    first_position: [f64; 2],
    third_position: [f64; 2],
) -> (ProjectionalEditorSession, geosolve_sketch_intent::NodeId) {
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
    .with_instance_leaf(first, LeafField::X, length(first_position[0]))
    .with_instance_leaf(first, LeafField::Y, length(first_position[1]))
    .with_instance_leaf(third, LeafField::X, length(third_position[0]))
    .with_instance_leaf(third, LeafField::Y, length(third_position[1]));
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

fn gui_rectangle_with_diagonal() -> (
    ProjectionalEditorSession,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
) {
    gui_rectangle_with_diagonal_between([-8.0, 3.0], [52.0, 38.0])
}

fn gui_rectangle_with_diagonal_between(
    first_position: [f64; 2],
    third_position: [f64; 2],
) -> (
    ProjectionalEditorSession,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
) {
    let (mut editor, rectangle) = gui_rectangle_between(first_position, third_position);
    let rectangle_node = editor
        .coordinator()
        .intent()
        .graph()
        .node(rectangle)
        .unwrap();
    let corner = |index| {
        rectangle_node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index,
            })
            .unwrap()
            .as_ref(rectangle)
    };
    let alias = geosolve_sketch_intent::IntentKey::new("gui-diagonal").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        alias.clone(),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: corner(0) },
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        PatchPortRef::Stable { port: corner(2) },
    );
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
    let diagonal = outcome.aliases.node(&alias).unwrap();
    (editor, rectangle, diagonal)
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

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end F003 owner proves lexical source, typed parsing, independent validation, and exact shared native ownership"
)]
fn gui_rectangle_diagonal_projects_lexical_references_and_shared_native_ownership() {
    let (editor, rectangle, diagonal) = gui_rectangle_with_diagonal();
    let project = initialize_code_project_from_editor(
        &editor,
        ProjectKey("bootstrap-lexical-diagonal".into()),
        &[
            EditorBootstrapDeclaration::new(diagonal, SemanticSymbol("diagonal".into())),
            EditorBootstrapDeclaration::new(rectangle, SemanticSymbol("frame".into())),
        ],
    )
    .unwrap();
    let source = &project.managed.source;
    assert!(source.find("const frame =").unwrap() < source.find("const diagonal =").unwrap());
    assert!(source.contains("start: frame.corners.lowerLeft"));
    assert!(source.contains("end: frame.corners.upperRight"));
    assert!(!source.contains("\"declaration\""));
    assert!(!source.contains("output: [\"corners\""));

    let line = project
        .managed
        .program
        .declarations
        .iter()
        .find(|declaration| declaration.symbol.0 == "diagonal")
        .unwrap();
    let ManagedValue::Object(arguments) = &line.arguments else {
        panic!("managed line arguments must be an object")
    };
    assert!(matches!(arguments["start"], ManagedValue::Reference { .. }));
    assert!(matches!(arguments["end"], ManagedValue::Reference { .. }));

    let desired = required_generated_members(&project).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_f003_b007),
        DocumentId(PersistentId::from_u128(0x84_f003_b007)),
        1.0,
    )
    .unwrap();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert!(
        accepted
            .session
            .design_document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert_eq!(
        accepted.session.design_document().points().len(),
        4,
        "lexical line endpoints must alias rectangle corner points"
    );

    let graph = materialized.editor.coordinator().intent().graph();
    let rectangle_node = graph
        .nodes()
        .values()
        .find(|node| {
            matches!(
                node.kind,
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::TwoPointAlignedRectangle
                }
            )
        })
        .unwrap();
    let line_node = graph
        .nodes()
        .values()
        .find(|node| {
            matches!(
                node.kind,
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment
                }
            )
        })
        .unwrap();
    for (corner_index, role) in [(0, IntentPortRole::Start), (2, IntentPortRole::End)] {
        let rectangle_port = rectangle_node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: corner_index,
            })
            .unwrap()
            .as_ref(rectangle_node.id);
        let line_port = line_node
            .port_by_selector(IntentPortSelector::Node { role, index: 0 })
            .unwrap()
            .as_ref(line_node.id);
        assert_eq!(
            accepted.ownership.port(rectangle_port),
            accepted.ownership.port(line_port),
            "managed lexical dependency must retain exact shared native point ownership"
        );
    }
}

#[test]
fn reversed_gui_rectangle_projects_truthful_named_corner_roles() {
    let (editor, rectangle, diagonal) =
        gui_rectangle_with_diagonal_between([52.0, 38.0], [-8.0, 3.0]);
    let project = initialize_code_project_from_editor(
        &editor,
        ProjectKey("bootstrap-reversed-diagonal".into()),
        &[
            EditorBootstrapDeclaration::new(rectangle, SemanticSymbol("frame".into())),
            EditorBootstrapDeclaration::new(diagonal, SemanticSymbol("diagonal".into())),
        ],
    )
    .unwrap();
    assert!(project.managed.source.contains("lowerLeft: [-8.0, 3.0]"));
    assert!(project.managed.source.contains("upperRight: [52.0, 38.0]"));
    assert!(
        project
            .managed
            .source
            .contains("start: frame.corners.upperRight")
    );
    assert!(
        project
            .managed
            .source
            .contains("end: frame.corners.lowerLeft")
    );
}
