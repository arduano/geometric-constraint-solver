// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentNativeBinding, ProjectionalEditorSession,
    SelectionItem,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    EditorBootstrapDeclaration, EditorBootstrapError, ExpandedSemanticTarget, KeyedReconcileState,
    ManagedValue, ProjectKey, SemanticSymbol, initialize_code_project_from_editor,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSession,
    IntentSessionId, IntentUnit, LeafField, PatchPortRef,
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

fn literal_line(
    name: &str,
    start: [f64; 2],
    end: [f64; 2],
    direction: [f64; 2],
) -> IntentNodeDraft {
    let selector = |role| IntentPortSelector::Node { role, index: 0 };
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        geosolve_sketch_intent::IntentKey::new(name).unwrap(),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::X,
        length(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start),
        LeafField::Y,
        length(start[1]),
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(end[0]))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(end[1]))
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("branch_direction").unwrap()),
        IntentLiteral::Point(direction),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the GUI-authoring fixture keeps its two connected lines and accepted Fillet transaction together"
)]
fn gui_two_lines_with_fillet() -> (
    ProjectionalEditorSession,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
) {
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_f004)).unwrap();
    let mut editor = ProjectionalEditorSession::restore(
        intent,
        DocumentId(PersistentId::from_u128(0x84_f004)),
        10.0,
    )
    .unwrap();
    let first_alias = geosolve_sketch_intent::IntentKey::new("gui-first").unwrap();
    let second_alias = geosolve_sketch_intent::IntentKey::new("gui-second").unwrap();
    let first_outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: first_alias.clone(),
                draft: Box::new(literal_line(
                    "gui.first",
                    [0.0, 0.0],
                    [4.0, 0.0],
                    [1.0, 0.0],
                )),
                cell: None,
            }],
        ))
        .unwrap();
    let first = first_outcome.aliases.node(&first_alias).unwrap();
    let first_span = editor
        .coordinator()
        .intent()
        .graph()
        .node(first)
        .unwrap()
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .unwrap()
        .as_ref(first);
    let horizontal_alias = geosolve_sketch_intent::IntentKey::new("gui-horizontal").unwrap();
    let horizontal_outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: horizontal_alias.clone(),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Constraint {
                            constraint: ConstraintKind::Horizontal,
                        },
                        geosolve_sketch_intent::IntentKey::new("gui.horizontal").unwrap(),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        PatchPortRef::Stable { port: first_span },
                    ),
                ),
                cell: None,
            }],
        ))
        .unwrap();
    let horizontal = horizontal_outcome.aliases.node(&horizontal_alias).unwrap();
    let first_end = editor
        .coordinator()
        .intent()
        .graph()
        .node(first)
        .unwrap()
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        })
        .unwrap()
        .as_ref(first);
    let second_draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        geosolve_sketch_intent::IntentKey::new("gui.second").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: first_end },
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        LeafField::X,
        length(4.0),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        LeafField::Y,
        length(4.0),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("branch_direction").unwrap()),
        IntentLiteral::Point([0.0, 1.0]),
    );
    let second_outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: second_alias.clone(),
                draft: Box::new(second_draft),
                cell: None,
            }],
        ))
        .unwrap();
    let second = second_outcome.aliases.node(&second_alias).unwrap();
    let second_span = editor
        .coordinator()
        .intent()
        .graph()
        .node(second)
        .unwrap()
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .unwrap()
        .as_ref(second);
    let vertical_alias = geosolve_sketch_intent::IntentKey::new("gui-vertical").unwrap();
    let vertical_outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: vertical_alias.clone(),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Constraint {
                            constraint: ConstraintKind::Vertical,
                        },
                        geosolve_sketch_intent::IntentKey::new("gui.vertical").unwrap(),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        PatchPortRef::Stable { port: second_span },
                    ),
                ),
                cell: None,
            }],
        ))
        .unwrap();
    let vertical = vertical_outcome.aliases.node(&vertical_alias).unwrap();

    let accepted = editor.coordinator().accepted_materialization().unwrap();
    let IntentNativeBinding::Point(shared_point) = accepted.ownership.port(first_end).unwrap()
    else {
        panic!("shared line endpoint must materialize as a point")
    };
    let mut state = FeatureAuthoringState::default();
    let symbol = geosolve_sketch_intent::IntentKey::new("Fillet 1").unwrap();
    assert!(matches!(
        editor
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..FeatureAuthoringOptions::default()
                },
                &[(SelectionItem::Point(shared_point), None)],
                symbol.clone(),
            )
            .unwrap(),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    editor
        .apply_computed_fillet_preview(&mut state, symbol)
        .unwrap();
    let fillet = editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find_map(|node| {
            matches!(
                node.kind,
                IntentNodeKind::ComputedFeature {
                    feature: ComputedFeatureKind::FilletSet
                }
            )
            .then_some(node.id)
        })
        .unwrap();
    (editor, first, horizontal, second, vertical, fillet)
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
fn retained_failed_rebind_cannot_mix_current_intent_with_prior_accepted_geometry() {
    let (mut editor, rectangle, diagonal) = gui_rectangle_with_diagonal();
    let accepted_semantic = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .validation
        .semantic;
    let end = editor
        .coordinator()
        .intent()
        .graph()
        .node(diagonal)
        .unwrap()
        .inputs[&InputSlot::new(InputRole::Point, 1)];
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::RebindInput {
                node: diagonal,
                slot: InputSlot::new(InputRole::Point, 0),
                source: PatchPortRef::Stable { port: end },
            }],
        ))
        .unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::RetainedFailed);
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .validation
            .semantic,
        accepted_semantic,
    );
    assert_ne!(
        editor.coordinator().intent().semantic_identity(),
        accepted_semantic,
    );

    let error = initialize_code_project_from_editor(
        &editor,
        ProjectKey("retained-failed-rebind".into()),
        &[
            EditorBootstrapDeclaration::new(rectangle, SemanticSymbol("frame".into())),
            EditorBootstrapDeclaration::new(diagonal, SemanticSymbol("diagonal".into())),
        ],
    )
    .unwrap_err();
    assert_eq!(error, EditorBootstrapError::StaleAcceptedAuthority);
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
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end F004 owner keeps lexical parents, exact branch state, cold validation, and computed ownership contiguous"
)]
fn gui_two_line_fillet_projects_lexical_span_references_and_exact_branch_state() {
    let (editor, first, horizontal, second, vertical, fillet) = gui_two_lines_with_fillet();
    let original = editor
        .coordinator()
        .intent()
        .graph()
        .node(fillet)
        .unwrap()
        .clone();
    let project = initialize_code_project_from_editor(
        &editor,
        ProjectKey("bootstrap-lexical-fillet".into()),
        &[
            EditorBootstrapDeclaration::new(fillet, SemanticSymbol("fillet".into())),
            EditorBootstrapDeclaration::new(vertical, SemanticSymbol("vertical".into())),
            EditorBootstrapDeclaration::new(second, SemanticSymbol("second".into())),
            EditorBootstrapDeclaration::new(horizontal, SemanticSymbol("horizontal".into())),
            EditorBootstrapDeclaration::new(first, SemanticSymbol("first".into())),
        ],
    )
    .unwrap();
    let source = &project.managed.source;
    assert!(source.find("const first =").unwrap() < source.find("const fillet =").unwrap());
    assert!(source.find("const second =").unwrap() < source.find("const fillet =").unwrap());
    assert!(source.contains("const horizontal = $.constraint.horizontal"));
    assert!(source.contains("const vertical = $.constraint.vertical"));
    assert!(source.contains("curve: first.span"));
    assert!(source.contains("curve: second.span"));
    assert!(source.contains("const fillet = $.computed.filletSet"));
    assert!(source.contains("span: first.span"));
    assert!(source.contains("span: second.span"));
    assert!(source.contains("parents: ["));
    assert!(source.contains("neighborhood: {"));
    assert!(source.contains("periodicAnchor: null"));
    assert!(source.contains("endpointOrder:"));
    assert!(source.contains("sweep:"));
    assert!(!source.contains("corner_0000"));
    assert!(!source.contains("\"declaration\""));

    let managed_fillet = project
        .managed
        .program
        .declarations
        .iter()
        .find(|declaration| declaration.symbol.0 == "fillet")
        .unwrap();
    assert_eq!(managed_fillet.builder_path, ["computed", "filletSet"]);
    let ManagedValue::Object(arguments) = &managed_fillet.arguments else {
        panic!("managed Fillet arguments must be an object")
    };
    let ManagedValue::Array(corners) = &arguments["corners"] else {
        panic!("managed Fillet corners must be an array")
    };
    let [ManagedValue::Object(corner)] = corners.as_slice() else {
        panic!("fixture must project one Fillet corner")
    };
    let ManagedValue::Array(parents) = &corner["parents"] else {
        panic!("managed Fillet parents must be an array")
    };
    assert_eq!(parents.len(), 2);
    for parent in parents {
        let ManagedValue::Object(parent) = parent else {
            panic!("managed Fillet parent must be an object")
        };
        assert!(matches!(parent["span"], ManagedValue::Reference { .. }));
    }

    let desired = required_generated_members(&project).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_f004_b007),
        DocumentId(PersistentId::from_u128(0x84_f004_b007)),
        10.0,
    )
    .unwrap();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert_eq!(accepted.validation.feature_count, 1);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    let [accepted_feature] = accepted.features.features() else {
        panic!("cold replay must retain exactly one computed Fillet feature")
    };
    let ComputedFeatureDefinition::FilletSet(accepted_fillet) = &accepted_feature.definition;

    let rematerialized = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| {
            matches!(
                node.kind,
                IntentNodeKind::ComputedFeature {
                    feature: ComputedFeatureKind::FilletSet
                }
            )
        })
        .unwrap();
    assert_eq!(rematerialized.fields, original.fields);
    assert_eq!(rematerialized.suppressed, original.suppressed);
    assert_eq!(rematerialized.child_order.len(), original.child_order.len());
    assert!(
        accepted
            .ownership
            .node(rematerialized.id)
            .unwrap()
            .owned
            .iter()
            .any(|binding| matches!(binding, IntentNativeBinding::ComputedFeature(_)))
    );
    for (ordinal, corner) in accepted_fillet.corners.iter().enumerate() {
        let port = rematerialized
            .port_by_selector(IntentPortSelector::InitialChild {
                ordinal: u16::try_from(ordinal).unwrap(),
                role: IntentPortRole::FeatureCorner,
                index: 0,
            })
            .unwrap();
        assert_eq!(
            accepted.ownership.port(port.as_ref(rematerialized.id)),
            Some(IntentNativeBinding::ComputedFeatureCorner(corner.id)),
        );
    }
    for (index, symbol) in ["first", "second"].into_iter().enumerate() {
        let source = rematerialized
            .inputs
            .get(&InputSlot::new(
                InputRole::Span,
                u16::try_from(index).unwrap(),
            ))
            .unwrap();
        let source_node = materialized
            .editor
            .coordinator()
            .intent()
            .graph()
            .node(source.node)
            .unwrap();
        let ExpandedSemanticTarget::Declaration { alias, .. } =
            &materialized.expansion.semantic_outputs[symbol].target
        else {
            panic!("line output must retain its declaration owner")
        };
        assert_eq!(
            source_node.id,
            materialized.base_outcome.aliases.node(alias).unwrap()
        );
        assert_eq!(
            source_node.port(source.port).unwrap().selector,
            IntentPortSelector::Node {
                role: IntentPortRole::Span,
                index: 0,
            }
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
