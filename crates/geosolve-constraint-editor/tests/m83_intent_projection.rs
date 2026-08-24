// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentInspectorEditError, IntentInspectorEditTarget,
    IntentInspectorEditValue, IntentInspectorField, IntentNativeBinding, IntentSourceEditError,
    IntentSourceTokenTarget, IntentWorkbenchProjection, ProjectionalIntentCoordinator,
};
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_intent::{
    CellTarget, GeometryRecipeKind, InputRole, InputSlot, IntentKey, IntentLiteral,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit, LeafField, PatchPortRef,
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
    named_point("point.main", position)
}

fn named_point(symbol: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(symbol),
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

fn circle(radius: f64) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key("circle.main"),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::X,
        coordinate(0.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Center),
        LeafField::Y,
        coordinate(0.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Target),
        LeafField::Value,
        coordinate(radius),
    )
}

fn coordinator() -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(0x8300_3001),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(0x8300_3001_u128 << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn assert_point(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

#[test]
fn outline_inspector_source_and_history_project_one_stable_declaration() {
    let mut coordinator = coordinator();
    let create = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point([1.25, -2.5])),
            cell: None,
        }],
    );
    let outcome = coordinator.apply_patch(create).unwrap();
    let node = outcome.aliases.node(&key("point")).unwrap();
    let point_port = outcome
        .aliases
        .port(&key("point"), selector(IntentPortRole::Primary))
        .unwrap();
    let IntentNativeBinding::Point(native_point) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(point_port)
        .unwrap()
    else {
        panic!("point output must bind one native point")
    };

    let projection = IntentWorkbenchProjection::from_session(coordinator.intent());
    assert_eq!(projection.identity, coordinator.intent().identity());
    assert_eq!(projection.outline.len(), 1);
    assert_eq!(projection.outline[0].declarations.len(), 1);
    assert_eq!(projection.outline[0].declarations[0].node, node);
    assert_eq!(projection.history.applied.len(), 1);
    assert_eq!(projection.history.redoable.len(), 0);
    assert_eq!(
        projection.structured_source,
        IntentWorkbenchProjection::from_session(coordinator.intent()).structured_source
    );
    for token in &projection.structured_source.tokens {
        assert!(token.start < token.end);
        assert!(token.end <= projection.structured_source.text.len());
        assert!(
            projection
                .structured_source
                .text
                .is_char_boundary(token.start)
        );
        assert!(
            projection
                .structured_source
                .text
                .is_char_boundary(token.end)
        );
    }

    let inspector = projection.inspector(coordinator.intent(), node).unwrap();
    assert_eq!(inspector.node, node);
    assert_eq!(
        inspector
            .fields
            .iter()
            .filter(|field| matches!(field, IntentInspectorField::Instance { .. }))
            .count(),
        2
    );
    assert!(inspector.fields.iter().all(|field| {
        matches!(field, IntentInspectorField::Definition { .. })
            || matches!(
                field,
                IntentInspectorField::Instance {
                    value: Some(IntentLiteral::Quantity {
                        unit: IntentUnit::Length,
                        ..
                    }),
                    ..
                }
            )
    }));
    let accepted = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    assert_point(
        accepted.document().point(native_point).unwrap().position,
        [1.25, -2.5],
    );
}

#[test]
fn recognized_source_edit_uses_typed_patch_and_organization_stays_nonsemantic() {
    let mut coordinator = coordinator();
    let create = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point([3.0, 4.0])),
            cell: None,
        }],
    );
    let node = coordinator
        .apply_patch(create)
        .unwrap()
        .aliases
        .node(&key("point"))
        .unwrap();
    let accepted_before = coordinator
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();
    let source = IntentWorkbenchProjection::from_session(coordinator.intent()).structured_source;
    let name = source
        .tokens
        .iter()
        .find(|token| token.target == IntentSourceTokenTarget::NodeName { node })
        .unwrap();
    let rename = source
        .patch_for_edit(coordinator.intent(), name.id, "\"Readable point\"")
        .unwrap();
    let renamed = coordinator.apply_patch(rename).unwrap();
    assert_eq!(
        renamed.disposition,
        geosolve_sketch_intent::IntentPlanDisposition::OrganizationOnly
    );
    assert_eq!(
        coordinator.accepted_materialization().unwrap().evidence,
        accepted_before
    );

    let create_cell = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateCell {
            alias: key("layout"),
            name: key("Layout only"),
            before: None,
        }],
    );
    let cell = coordinator
        .apply_patch(create_cell)
        .unwrap()
        .aliases
        .cell(&key("layout"))
        .unwrap();
    let move_node = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::MoveDeclaration {
            node,
            cell: CellTarget::Stable { cell },
            before: None,
        }],
    );
    coordinator.apply_patch(move_node).unwrap();
    assert_eq!(
        coordinator.accepted_materialization().unwrap().evidence,
        accepted_before
    );
    let projection = IntentWorkbenchProjection::from_session(coordinator.intent());
    assert_eq!(projection.outline.len(), 2);
    assert_eq!(
        projection.outline[1].declarations[0].name.as_str(),
        "Readable point"
    );

    assert_eq!(
        source.patch_for_edit(coordinator.intent(), name.id, "\"stale\""),
        Err(IntentSourceEditError::StaleProjection)
    );
}

#[test]
fn source_and_inspector_project_exact_stable_input_bindings_after_rebind() {
    let mut coordinator = coordinator();
    let start_slot = InputSlot::new(InputRole::Point, 0);
    let segment = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("bound.segment"),
    )
    .with_input(
        start_slot,
        PatchPortRef::Alias {
            node: key("first"),
            selector: selector(IntentPortRole::Primary),
        },
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, coordinate(5.0))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, coordinate(0.0));
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("first"),
                    draft: Box::new(named_point("bound.first", [0.0, 0.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("second"),
                    draft: Box::new(named_point("bound.second", [2.0, 1.0])),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("segment"),
                    draft: Box::new(segment),
                    cell: None,
                },
            ],
        ))
        .unwrap();
    let first = outcome
        .aliases
        .port(&key("first"), selector(IntentPortRole::Primary))
        .unwrap();
    let second = outcome
        .aliases
        .port(&key("second"), selector(IntentPortRole::Primary))
        .unwrap();
    let segment = outcome.aliases.node(&key("segment")).unwrap();

    let before = IntentWorkbenchProjection::from_session(coordinator.intent());
    let first_binding = format!(
        "\"{start_slot}\": {}",
        serde_json::to_string(&first).unwrap()
    );
    assert!(before.structured_source.text.contains(&first_binding));
    assert_eq!(
        before
            .inspector(coordinator.intent(), segment)
            .unwrap()
            .inputs,
        vec![(start_slot, first)]
    );

    coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::RebindInput {
                node: segment,
                slot: start_slot,
                source: PatchPortRef::Stable { port: second },
            }],
        ))
        .unwrap();
    let after = IntentWorkbenchProjection::from_session(coordinator.intent());
    let second_binding = format!(
        "\"{start_slot}\": {}",
        serde_json::to_string(&second).unwrap()
    );
    assert_ne!(after.structured_source.text, before.structured_source.text);
    assert!(after.structured_source.text.contains(&second_binding));
    assert!(!after.structured_source.text.contains(&first_binding));
    assert_eq!(
        after
            .inspector(coordinator.intent(), segment)
            .unwrap()
            .inputs,
        vec![(start_slot, second)]
    );
}

#[test]
fn instance_source_token_commits_through_native_materialization_once() {
    let mut coordinator = coordinator();
    let create = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point([1.0, 2.0])),
            cell: None,
        }],
    );
    let outcome = coordinator.apply_patch(create).unwrap();
    let point_port = outcome
        .aliases
        .port(&key("point"), selector(IntentPortRole::Primary))
        .unwrap();
    let IntentNativeBinding::Point(native_point) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(point_port)
        .unwrap()
    else {
        panic!("point output must bind one native point")
    };
    let source = IntentWorkbenchProjection::from_session(coordinator.intent()).structured_source;
    let x = source
        .tokens
        .iter()
        .find(|token| {
            matches!(
                token.target,
                IntentSourceTokenTarget::Instance {
                    leaf: geosolve_sketch_intent::LeafRef {
                        field: LeafField::X,
                        ..
                    }
                }
            )
        })
        .unwrap();
    let replacement = serde_json::to_string(&coordinate(8.5)).unwrap();
    let patch = source
        .patch_for_edit(coordinator.intent(), x.id, &replacement)
        .unwrap();
    coordinator.apply_patch(patch).unwrap();
    let accepted = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    assert_point(
        accepted.document().point(native_point).unwrap().position,
        [8.5, 2.0],
    );
    assert_eq!(coordinator.intent().history_projection().applied.len(), 2);
}

#[test]
fn inspector_edits_authenticate_schema_coordinates_and_retain_invalid_intent() {
    let mut coordinator = coordinator();
    let create = IntentPatch::new(
        coordinator.intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point([1.0, 2.0])),
            cell: None,
        }],
    );
    let outcome = coordinator.apply_patch(create).unwrap();
    let node = outcome.aliases.node(&key("point")).unwrap();
    let projection = IntentWorkbenchProjection::from_session(coordinator.intent());
    let inspector = projection.inspector(coordinator.intent(), node).unwrap();
    let leaf = inspector
        .fields
        .iter()
        .find_map(|field| match field {
            IntentInspectorField::Instance { leaf, .. } if leaf.field == LeafField::X => {
                Some(*leaf)
            }
            _ => None,
        })
        .unwrap();
    let patch = inspector
        .patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: coordinate(7.0),
            },
        )
        .unwrap();
    coordinator.apply_patch(patch).unwrap();
    assert_eq!(coordinator.intent().history_projection().applied.len(), 2);

    assert_eq!(
        inspector.patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: coordinate(8.0),
            },
        ),
        Err(IntentInspectorEditError::StaleProjection)
    );

    let current = IntentWorkbenchProjection::from_session(coordinator.intent());
    let current = current.inspector(coordinator.intent(), node).unwrap();
    assert_eq!(
        current.patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Suppressed,
            IntentInspectorEditValue::Literal {
                literal: IntentLiteral::Boolean(true),
            },
        ),
        Err(IntentInspectorEditError::TargetValueMismatch)
    );
    assert_eq!(
        current.patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: IntentLiteral::Boolean(true),
            },
        ),
        Err(IntentInspectorEditError::InvalidLiteral)
    );
}

#[test]
fn inspector_value_leaf_preserves_its_projected_quantity_unit() {
    let mut coordinator = coordinator();
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("circle"),
                draft: Box::new(circle(2.0)),
                cell: None,
            }],
        ))
        .unwrap();
    let node = outcome.aliases.node(&key("circle")).unwrap();
    let projection = IntentWorkbenchProjection::from_session(coordinator.intent());
    let inspector = projection.inspector(coordinator.intent(), node).unwrap();
    let leaf = inspector
        .fields
        .iter()
        .find_map(|field| match field {
            IntentInspectorField::Instance {
                leaf,
                value:
                    Some(IntentLiteral::Quantity {
                        unit: IntentUnit::Length,
                        ..
                    }),
                ..
            } if leaf.field == LeafField::Value => Some(*leaf),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        inspector.patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: IntentLiteral::Quantity {
                    value: 3.5,
                    unit: IntentUnit::Angle,
                },
            },
        ),
        Err(IntentInspectorEditError::InvalidLiteral)
    );
    let patch = inspector
        .patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: coordinate(3.5),
            },
        )
        .unwrap();
    coordinator.apply_patch(patch).unwrap();
    let document = coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .design_document();
    let CurveDefinition::Circle { radius, .. } = document.curves()[0].definition else {
        panic!("fixture must remain a circle");
    };
    assert_eq!(
        document.scalar(radius).unwrap().value.to_bits(),
        3.5_f64.to_bits()
    );
}
