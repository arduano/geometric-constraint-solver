// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentInspectorEditError, IntentInspectorEditTarget,
    IntentInspectorEditValue, IntentInspectorField, IntentInspectorInput, IntentNativeBinding,
    IntentProjectedPortReference, IntentSourceEditError, IntentSourceTokenTarget,
    IntentWorkbenchProjection, ProjectionalIntentCoordinator,
};
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_intent::{
    AggregateKind, CellTarget, ComputedFeatureKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentEvaluation, IntentKey, IntentLiteral, IntentLiteralSchema, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
    IntentPortSelector, IntentProjectionPath, IntentSession, IntentSessionId, IntentUnit,
    LeafField, MaterializationEvidence, PatchPortRef,
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

fn literal_for_schema(schema: IntentLiteralSchema) -> IntentLiteral {
    match schema {
        IntentLiteralSchema::Boolean => IntentLiteral::Boolean(false),
        IntentLiteralSchema::Integer => IntentLiteral::Integer(0),
        IntentLiteralSchema::Natural => IntentLiteral::Natural(3),
        IntentLiteralSchema::Text => IntentLiteral::Text(key("semantic-text")),
        IntentLiteralSchema::Enum => IntentLiteral::Enum(key("semantic-enum")),
        IntentLiteralSchema::Point => IntentLiteral::Point([1.0, 2.0]),
        IntentLiteralSchema::Quantity(unit) => IntentLiteral::Quantity { value: 1.0, unit },
    }
}

fn accepted(candidate: &geosolve_sketch_intent::IntentCandidate) -> IntentEvaluation {
    IntentEvaluation::Accepted {
        evidence: MaterializationEvidence::new_host_artifacts(
            candidate.external_inputs().identity(),
            format!("semantic-source:{:?}", candidate.semantic_identity()).into_bytes(),
            b"semantic-source-owners".to_vec(),
            b"semantic-source-validation".to_vec(),
        )
        .unwrap(),
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
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end projection fixture proves the same semantic binding before and after an exact rebind"
)]
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
    let start_path = IntentProjectionPath::field(key("start"));
    let first_reference = IntentProjectedPortReference {
        declaration: key("bound.first"),
        output: IntentProjectionPath::field(key("point")),
        kind: first.kind,
    };
    let first_binding = format!(
        "start: {}",
        serde_json::to_string(&first_reference).unwrap()
    );
    assert!(before.structured_source.text.contains(&first_binding));
    assert!(!before.structured_source.text.contains("point:0000"));
    assert!(!before.structured_source.text.contains("node:"));
    assert!(!before.structured_source.text.contains("\"node\":"));
    assert!(!before.structured_source.text.contains("\"port\":"));
    assert_eq!(
        before
            .inspector(coordinator.intent(), segment)
            .unwrap()
            .inputs,
        vec![IntentInspectorInput {
            path: start_path.clone(),
            source: first_reference,
        }]
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
    let second_reference = IntentProjectedPortReference {
        declaration: key("bound.second"),
        output: IntentProjectionPath::field(key("point")),
        kind: second.kind,
    };
    let second_binding = format!(
        "start: {}",
        serde_json::to_string(&second_reference).unwrap()
    );
    assert_ne!(after.structured_source.text, before.structured_source.text);
    assert!(after.structured_source.text.contains(&second_binding));
    assert!(!after.structured_source.text.contains(&first_binding));
    assert_eq!(
        after
            .inspector(coordinator.intent(), segment)
            .unwrap()
            .inputs,
        vec![IntentInspectorInput {
            path: start_path,
            source: second_reference,
        }]
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
fn nurbs_controls_project_as_nested_instance_arrays_without_selector_strings() {
    let mut coordinator = coordinator();
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::OpenControlNurbs,
        },
        key("nurbs.main"),
    )
    .with_dynamic_children(4);
    for (ordinal, position) in [[0.0, 0.0], [1.0, 1.0], [2.0, 1.0], [3.0, 0.0]]
        .into_iter()
        .enumerate()
    {
        let ordinal = u16::try_from(ordinal).unwrap();
        let control = IntentPortSelector::InitialChild {
            ordinal,
            role: IntentPortRole::Control,
            index: 0,
        };
        let weight = IntentPortSelector::InitialChild {
            ordinal,
            role: IntentPortRole::Target,
            index: 0,
        };
        draft = draft
            .with_instance_leaf(control, LeafField::X, coordinate(position[0]))
            .with_instance_leaf(control, LeafField::Y, coordinate(position[1]))
            .with_instance_leaf(
                weight,
                LeafField::Weight,
                IntentLiteral::Quantity {
                    value: 1.0,
                    unit: IntentUnit::Dimensionless,
                },
            );
    }
    coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("nurbs"),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();

    let source = IntentWorkbenchProjection::from_session(coordinator.intent()).structured_source;
    assert!(source.text.contains("controls: ["));
    assert!(source.text.contains("position: {"));
    assert!(source.text.contains("weight:"));
    assert!(!source.text.contains("child:0000"));
    assert!(!source.text.contains("node:control"));
    assert!(!source.text.contains("node:target"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one source fixture joins sparse recipe operands, aggregate spans, and a genuine two-corner Fillet to prove their nested array composition"
)]
fn structured_source_composes_sparse_recipe_aggregate_and_two_corner_fillet_arrays() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_3020)).unwrap();
    let point_ref = |node: &str| PatchPortRef::Alias {
        node: key(node),
        selector: selector(IntentPortRole::Primary),
    };
    let span_ref = |node: &str| PatchPortRef::Alias {
        node: key(node),
        selector: selector(IntentPortRole::Span),
    };

    let polyline_vertex = |ordinal| IntentPortSelector::InitialChild {
        ordinal,
        role: IntentPortRole::Corner,
        index: 0,
    };
    let polyline = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        key("semantic.polyline"),
    )
    .with_dynamic_children(3)
    .with_input(InputSlot::new(InputRole::Point, 0), point_ref("first"))
    .with_input(InputSlot::new(InputRole::Point, 2), point_ref("third"))
    .with_instance_leaf(polyline_vertex(1), LeafField::X, coordinate(1.0))
    .with_instance_leaf(polyline_vertex(1), LeafField::Y, coordinate(0.5));

    let nurbs_kind = IntentNodeKind::Geometry {
        recipe: GeometryRecipeKind::OpenControlNurbs,
    };
    let mut nurbs = IntentNodeDraft::new(nurbs_kind.clone(), key("semantic.nurbs"))
        .with_dynamic_children(4)
        .with_input(InputSlot::new(InputRole::Point, 0), point_ref("first"))
        .with_input(InputSlot::new(InputRole::Point, 2), point_ref("third"));
    for field in nurbs_kind.schema(4).fields {
        if field.required {
            nurbs = nurbs.with_field(field.field, literal_for_schema(field.literal));
        }
    }

    let aggregate = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("semantic.chain"),
    )
    .with_input(InputSlot::new(InputRole::Span, 0), span_ref("parent-0"))
    .with_input(InputSlot::new(InputRole::Span, 1), span_ref("parent-1"));

    let fillet_kind = IntentNodeKind::ComputedFeature {
        feature: ComputedFeatureKind::FilletSet,
    };
    let mut fillet =
        IntentNodeDraft::new(fillet_kind.clone(), key("semantic.fillet")).with_dynamic_children(2);
    for index in 0..4_u16 {
        fillet = fillet.with_input(
            InputSlot::new(InputRole::Span, index),
            span_ref(&format!("parent-{index}")),
        );
    }
    for field in fillet_kind.schema(2).fields {
        if field.required {
            fillet = fillet.with_field(field.field, literal_for_schema(field.literal));
        }
    }

    let mut operations = vec![
        IntentPatchOperation::CreateNode {
            alias: key("first"),
            draft: Box::new(named_point("semantic.first", [0.0, 0.0])),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("third"),
            draft: Box::new(named_point("semantic.third", [2.0, 1.0])),
            cell: None,
        },
    ];
    for index in 0..4_u16 {
        operations.push(IntentPatchOperation::CreateNode {
            alias: key(&format!("parent-{index}")),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                key(&format!("semantic.parent{index}")),
            )),
            cell: None,
        });
    }
    operations.extend([
        IntentPatchOperation::CreateNode {
            alias: key("polyline"),
            draft: Box::new(polyline),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("nurbs"),
            draft: Box::new(nurbs),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("aggregate"),
            draft: Box::new(aggregate),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("fillet"),
            draft: Box::new(fillet),
            cell: None,
        },
    ]);
    let plan = session
        .plan_patch(
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                operations,
            ),
            accepted,
        )
        .unwrap();
    session.commit_plan(plan).unwrap();

    let source = IntentWorkbenchProjection::from_session(&session).structured_source;
    let declaration = |symbol: &str| {
        let marker = format!("symbol: \"{symbol}\"");
        let start = source.text.find(&marker).unwrap();
        let value = &source.text[start..];
        let end = value.find("\n        },").unwrap();
        &value[..end]
    };
    let polyline = declaration("semantic.polyline");
    assert_eq!(polyline.matches("vertices: [").count(), 2, "{polyline}");
    assert_eq!(polyline.matches("position: {").count(), 1, "{polyline}");
    assert_eq!(
        polyline.matches("\n              null,").count(),
        2,
        "{polyline}"
    );
    let nurbs = declaration("semantic.nurbs");
    assert!(nurbs.contains("controls: ["));
    assert!(nurbs.contains("\n              null,"), "{nurbs}");
    let aggregate = declaration("semantic.chain");
    assert_eq!(aggregate.matches("spans: [").count(), 1, "{aggregate}");
    assert!(aggregate.contains("semantic.parent0"), "{aggregate}");
    assert!(aggregate.contains("semantic.parent1"), "{aggregate}");
    let fillet = declaration("semantic.fillet");
    assert_eq!(fillet.matches("corners: [").count(), 2, "{fillet}");
    assert_eq!(fillet.matches("parents: [").count(), 4, "{fillet}");
    assert_eq!(fillet.matches("endpointOrder:").count(), 2, "{fillet}");
    assert_eq!(fillet.matches("sweep:").count(), 2, "{fillet}");
    for index in 0..4_u16 {
        assert!(
            fillet.contains(&format!("semantic.parent{index}")),
            "{fillet}"
        );
    }
    assert!(!fillet.contains("corner_0000"));
    assert!(!fillet.contains("corner_0001"));
    assert!(!source.text.contains("point:0000"));
    assert!(!source.text.contains("span:0000"));
    assert!(!source.text.contains("child:0000"));
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
fn inspector_identity_rejects_an_otherwise_unchanged_view_after_unrelated_mutation() {
    let mut coordinator = coordinator();
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("selected"),
                draft: Box::new(named_point("selected.point", [1.0, 2.0])),
                cell: None,
            }],
        ))
        .unwrap();
    let node = outcome.aliases.node(&key("selected")).unwrap();
    let inspector = IntentWorkbenchProjection::from_session(coordinator.intent())
        .inspector(coordinator.intent(), node)
        .unwrap();
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

    coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("unrelated"),
                draft: Box::new(named_point("unrelated.point", [8.0, 9.0])),
                cell: None,
            }],
        ))
        .unwrap();
    assert_eq!(
        inspector.patch_for_edit(
            coordinator.intent(),
            &IntentInspectorEditTarget::Instance { leaf },
            IntentInspectorEditValue::Literal {
                literal: coordinate(3.0),
            },
        ),
        Err(IntentInspectorEditError::StaleProjection),
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
