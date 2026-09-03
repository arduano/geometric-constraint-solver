// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::Cell;
use std::collections::BTreeSet;

use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, CellTarget, ComputedFeatureKind, ConstraintKind,
    DeletePolicy, DimensionKind, ExternalIntentKind, GeometryRecipeKind, IdentityTransitionKind,
    InputRole, InputSlot, IntentBootstrapObject, IntentDefinitionFieldDescriptor,
    IntentEditClassification, IntentEvaluation, IntentEvaluationFailure,
    IntentEvaluationFailureKind, IntentFieldChoices, IntentFieldDefault, IntentFieldKey,
    IntentGraphError, IntentIdentityFlow, IntentKey, IntentLiteral, IntentLiteralSchema,
    IntentNativeReservationKind, IntentNodeDraft, IntentNodeKind, IntentOperationOutput,
    IntentOperationOutputKind, IntentOutputDescriptor, IntentPatch, IntentPatchOperation,
    IntentPatchOperationKind, IntentPatchPolicy, IntentPlanDisposition, IntentPlanError,
    IntentPortKind, IntentPortRef, IntentPortRole, IntentPortSelector, IntentProjectionPath,
    IntentReservationState, IntentSession, IntentSessionId, IntentSessionIdentity, IntentUnit,
    LeafField, LeafRef, MAX_INTENT_PROJECTION_PATH_SEGMENTS, MaterializationEvidence, NodeId,
    OperationKind, ParameterIntentKind, PatchPortRef, PortId,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn point_draft(name: &str) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(name),
    )
}

fn point_selector() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    }
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn identity_result_selector() -> IntentPortSelector {
    IntentPortSelector::Node {
        role: IntentPortRole::Result,
        index: 0,
    }
}

fn alias_point(node: &str) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: point_selector(),
    }
}

fn alias_port(node: &str, role: IntentPortRole, index: u16) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(node),
        selector: IntentPortSelector::Node { role, index },
    }
}

fn bootstrap_draft(kind: BootstrapNativeKind, name: &str) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Bootstrap {
            object: IntentBootstrapObject::new(
                kind,
                key("geosolve-flat-object-v1"),
                format!("{kind:?}:{name}").into_bytes(),
            )
            .unwrap(),
        },
        key(name),
    )
}

fn accepted(candidate: &geosolve_sketch_intent::IntentCandidate) -> IntentEvaluation {
    IntentEvaluation::Accepted {
        evidence: MaterializationEvidence::new_host_artifacts(
            candidate.external_inputs().identity(),
            format!("materialized:{:?}", candidate.semantic_identity()).into_bytes(),
            b"owners".to_vec(),
            b"host-validation-artifact".to_vec(),
        )
        .unwrap(),
    }
}

#[derive(Clone, Debug)]
struct DeclarationSchemaCase {
    label: String,
    kind: IntentNodeKind,
    dynamic_children: u16,
}

fn declaration_schema_cases() -> Vec<DeclarationSchemaCase> {
    let mut cases = Vec::new();
    let mut push = |label: String, kind: IntentNodeKind| {
        let dynamic_children = kind.schema(0).minimum_children;
        cases.push(DeclarationSchemaCase {
            label,
            kind,
            dynamic_children,
        });
    };

    for recipe in GeometryRecipeKind::ALL {
        push(
            format!("geometry.{recipe:?}"),
            IntentNodeKind::Geometry { recipe },
        );
    }
    for constraint in ConstraintKind::ALL {
        push(
            format!("constraint.{constraint:?}"),
            IntentNodeKind::Constraint { constraint },
        );
    }
    for dimension in DimensionKind::ALL {
        push(
            format!("dimension.{dimension:?}"),
            IntentNodeKind::Dimension { dimension },
        );
    }
    for operation in OperationKind::ALL {
        push(
            format!("operation.{operation:?}"),
            IntentNodeKind::Operation { operation },
        );
    }
    for feature in ComputedFeatureKind::ALL {
        push(
            format!("computed_feature.{feature:?}"),
            IntentNodeKind::ComputedFeature { feature },
        );
    }
    for aggregate in AggregateKind::ALL {
        push(
            format!("aggregate.{aggregate:?}"),
            IntentNodeKind::Aggregate { aggregate },
        );
    }
    for parameter in ParameterIntentKind::ALL {
        push(
            format!("parameter.{parameter:?}"),
            IntentNodeKind::Parameter { parameter },
        );
    }
    for external in ExternalIntentKind::ALL {
        push(
            format!("external.{external:?}"),
            IntentNodeKind::External { external },
        );
    }
    for object in BootstrapNativeKind::ALL {
        push(
            format!("bootstrap.{object:?}"),
            IntentNodeKind::Bootstrap {
                object: IntentBootstrapObject::new(
                    object,
                    key("geosolve-flat-object-v1"),
                    format!("schema:{object:?}").into_bytes(),
                )
                .unwrap(),
            },
        );
    }
    push("annotation".to_owned(), IntentNodeKind::Annotation);
    for transition in [
        IdentityTransitionKind::Alias,
        IdentityTransitionKind::Continue,
        IdentityTransitionKind::Retire,
    ] {
        push(
            format!("identity.{transition:?}"),
            IntentNodeKind::Identity {
                transition,
                port_kind: IntentPortKind::Point,
            },
        );
    }
    cases
}

fn field_descriptor(
    kind: &IntentNodeKind,
    dynamic_children: u16,
    name: &str,
) -> IntentDefinitionFieldDescriptor {
    kind.field_descriptors(dynamic_children)
        .into_iter()
        .find(|descriptor| descriptor.schema.field.0.as_str() == name)
        .unwrap_or_else(|| panic!("{kind:?} has no {name} descriptor"))
}

fn closed_choices(values: &[&str]) -> IntentFieldChoices {
    IntentFieldChoices::Closed(values.iter().copied().map(key).collect())
}

fn literal_for_schema(schema: IntentLiteralSchema) -> IntentLiteral {
    match schema {
        IntentLiteralSchema::Boolean => IntentLiteral::Boolean(false),
        IntentLiteralSchema::Integer => IntentLiteral::Integer(0),
        IntentLiteralSchema::Natural => IntentLiteral::Natural(1),
        IntentLiteralSchema::Text => IntentLiteral::Text(key("schema-text")),
        IntentLiteralSchema::Enum => IntentLiteral::Enum(key("schema-enum")),
        IntentLiteralSchema::Point => IntentLiteral::Point([1.0, 2.0]),
        IntentLiteralSchema::Quantity(unit) => IntentLiteral::Quantity { value: 1.0, unit },
    }
}

fn wrong_literal_for_schema(schema: IntentLiteralSchema) -> IntentLiteral {
    if schema == IntentLiteralSchema::Boolean {
        IntentLiteral::Text(key("wrong-literal"))
    } else {
        IntentLiteral::Boolean(false)
    }
}

fn different_unit(unit: IntentUnit) -> IntentUnit {
    match unit {
        IntentUnit::Length => IntentUnit::Angle,
        IntentUnit::Angle => IntentUnit::Dimensionless,
        IntentUnit::Dimensionless => IntentUnit::Length,
    }
}

fn leaf_projection_key(field: LeafField) -> IntentKey {
    key(match field {
        LeafField::X => "x",
        LeafField::Y => "y",
        LeafField::Value => "value",
        LeafField::Angle => "angle",
        LeafField::Weight => "weight",
        LeafField::Parameter => "parameter",
    })
}

fn assert_semantic_path(path: &IntentProjectionPath, expected: &serde_json::Value) {
    let actual = serde_json::to_value(path).expect("semantic path serializes");
    assert_eq!(&actual, expected);
}

fn assert_output_paths_form_an_unambiguous_tree(outputs: &[IntentOutputDescriptor]) {
    for (index, output) in outputs.iter().enumerate() {
        for other in outputs.iter().skip(index + 1) {
            assert!(
                !projection_path_is_strict_prefix(&output.path, &other.path)
                    && !projection_path_is_strict_prefix(&other.path, &output.path),
                "stable output paths must form an unambiguous tree: {:?} and {:?}",
                output.path,
                other.path,
            );
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one audit helper proves the complete input, definition, output, and writable-leaf path bijection"
)]
fn assert_descriptor_target_paths_are_bijective(node: &geosolve_sketch_intent::IntentNode) {
    let descriptor = node.descriptor();

    assert_eq!(descriptor.inputs.len(), node.inputs.len());
    assert_eq!(
        descriptor
            .inputs
            .iter()
            .map(|input| input.slot)
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.inputs.len(),
        "each canonical input slot must appear once"
    );
    assert_eq!(
        descriptor
            .inputs
            .iter()
            .map(|input| input.path.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.inputs.len(),
        "each canonical input slot must have one unique semantic path"
    );
    assert!(
        descriptor
            .inputs
            .iter()
            .all(|input| node.inputs.contains_key(&input.slot))
    );

    assert_eq!(descriptor.fields.len(), descriptor.schema.fields.len());
    assert_eq!(
        descriptor
            .fields
            .iter()
            .map(|field| field.schema.field.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.fields.len(),
        "each canonical definition field must appear once"
    );
    assert_eq!(
        descriptor
            .fields
            .iter()
            .map(|field| field.path.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.fields.len(),
        "each canonical definition field must have one unique semantic path"
    );

    assert_eq!(descriptor.outputs.len(), node.ports.len());
    assert_eq!(
        descriptor
            .outputs
            .iter()
            .map(|output| output.port)
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.outputs.len(),
        "each canonical stable port must appear once"
    );
    assert_eq!(
        descriptor
            .outputs
            .iter()
            .map(|output| output.path.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        descriptor.outputs.len(),
        "each stable output must have one unique semantic path"
    );
    assert_output_paths_form_an_unambiguous_tree(&descriptor.outputs);

    let instance_leaf_count = descriptor
        .outputs
        .iter()
        .map(|output| output.writable.len())
        .sum::<usize>();
    let instance_paths = descriptor
        .outputs
        .iter()
        .flat_map(|output| {
            output.writable.iter().map(|field| {
                if output.writable.len() == 1 {
                    output.path.clone()
                } else {
                    output.path.clone().with_field(leaf_projection_key(*field))
                }
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        instance_paths.len(),
        instance_leaf_count,
        "each canonical writable leaf must have one unique semantic path"
    );

    for path in descriptor
        .inputs
        .iter()
        .map(|input| &input.path)
        .chain(descriptor.fields.iter().map(|field| &field.path))
        .chain(descriptor.outputs.iter().map(|output| &output.path))
        .chain(instance_paths.iter())
    {
        let projected = serde_json::to_string(path).expect("semantic path serializes");
        assert!(
            !projected.contains(":0000"),
            "pseudo slot leaked: {projected}"
        );
        assert!(
            !projected.contains("corner_0000"),
            "flattened corner field leaked: {projected}"
        );
        assert!(
            !projected.contains("node:"),
            "raw node key leaked: {projected}"
        );
    }
}

fn projection_path_is_strict_prefix(
    prefix: &IntentProjectionPath,
    path: &IntentProjectionPath,
) -> bool {
    prefix.segments().len() < path.segments().len()
        && path.segments().starts_with(prefix.segments())
}

fn dummy_input(kind: &IntentNodeKind, slot: InputSlot) -> PatchPortRef {
    let port_kind = slot.role.expected_kind().unwrap_or(match kind {
        IntentNodeKind::Identity { port_kind, .. } => *port_kind,
        _ => IntentPortKind::Point,
    });
    PatchPortRef::Stable {
        port: IntentPortRef {
            node: NodeId::from_raw(0xfeed_0000 + slot.role as u64),
            port: PortId::from_raw(1 + u64::from(slot.index) + (slot.role as u64) * 0x1_0000),
            kind: port_kind,
        },
    }
}

fn minimal_schema_draft(case: &DeclarationSchemaCase, symbol: &str) -> IntentNodeDraft {
    let schema = case.kind.schema(case.dynamic_children);
    let mut draft = IntentNodeDraft::new(case.kind.clone(), key(symbol))
        .with_dynamic_children(case.dynamic_children);
    for cardinality in &schema.inputs {
        for index in 0..cardinality.minimum {
            let slot = InputSlot::new(cardinality.role, index);
            draft = draft.with_input(slot, dummy_input(&case.kind, slot));
        }
    }
    for choice in &schema.input_choices {
        let mut actual = choice
            .alternatives
            .iter()
            .filter(|slot| draft.inputs.contains_key(slot))
            .count();
        for slot in &choice.alternatives {
            if actual >= usize::from(choice.minimum) {
                break;
            }
            if !draft.inputs.contains_key(slot) {
                draft = draft.with_input(*slot, dummy_input(&case.kind, *slot));
                actual += 1;
            }
        }
    }
    if matches!(
        case.kind,
        IntentNodeKind::Bootstrap {
            object: IntentBootstrapObject {
                kind: BootstrapNativeKind::SemanticSource,
                ..
            }
        }
    ) {
        let slot = InputSlot::new(InputRole::Catalog, 0);
        draft = draft.with_input(slot, dummy_input(&case.kind, slot));
    }
    for field in schema.fields.iter().filter(|field| field.required) {
        draft = draft.with_field(field.field.clone(), literal_for_schema(field.literal));
    }
    draft
}

fn atomic_schema_rejection(label: &str, draft: IntentNodeDraft) -> IntentPlanError {
    let session = IntentSession::with_id(IntentSessionId::from_raw(0x83f0)).unwrap();
    let identity = session.identity();
    let allocator = session.allocator_high_water();
    let evaluated = Cell::new(false);
    let patch = IntentPatch::new(
        identity,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("declaration"),
            draft: Box::new(draft),
            cell: None,
        }],
    );
    let result = session.plan_patch(patch, |candidate| {
        evaluated.set(true);
        accepted(candidate)
    });
    assert!(!evaluated.get(), "{label} reached the materializer");
    assert_eq!(session.identity(), identity, "{label} changed identity");
    assert_eq!(
        session.allocator_high_water(),
        allocator,
        "{label} changed allocator high-water"
    );
    match result {
        Ok(_) => panic!("{label} unexpectedly planned"),
        Err(error) => error,
    }
}

fn create_point(
    session: &mut IntentSession,
    alias: &str,
) -> (NodeId, geosolve_sketch_intent::IntentPortRef) {
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key(alias),
            draft: Box::new(point_draft(alias)),
            cell: None,
        }],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let node = plan.aliases().node(&key(alias)).unwrap();
    let port = plan.aliases().port(&key(alias), point_selector()).unwrap();
    session.commit_plan(plan).unwrap();
    (node, port)
}

#[test]
fn multi_root_delete_rejects_unauthenticated_root_sets_atomically() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83ca_5cad)).unwrap();
    let (root, root_port) = create_point(&mut session, "root");
    let segment_patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("dependent-segment"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("dependent.segment"),
                )
                .with_input(
                    InputSlot::new(InputRole::Point, 0),
                    PatchPortRef::Stable { port: root_port },
                ),
            ),
            cell: None,
        }],
    );
    let segment_plan = session.plan_patch(segment_patch, accepted).unwrap();
    let dependent = segment_plan
        .aliases()
        .node(&key("dependent-segment"))
        .unwrap();
    session.commit_plan(segment_plan).unwrap();
    let (other_root, _) = create_point(&mut session, "other-root");

    let canonical_before = session.to_canonical_json().unwrap();
    let identity_before = session.identity();
    let allocator_before = session.allocator_high_water();
    let unknown = NodeId::from_raw(0xdead_beef);

    let missing_addressed_root = IntentPatch::new(
        identity_before,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::DeleteNode {
            node: root,
            policy: DeletePolicy::CascadeRoots {
                exact_roots: BTreeSet::from([other_root]),
                exact_nodes: BTreeSet::from([other_root]),
            },
        }],
    );
    assert!(matches!(
        session.plan_patch(missing_addressed_root, |_| {
            panic!("an unauthenticated root set must not reach evaluation")
        }),
        Err(IntentPlanError::CascadeRootMissing { node }) if node == root
    ));

    let unknown_root = IntentPatch::new(
        identity_before,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::DeleteNode {
            node: root,
            policy: DeletePolicy::CascadeRoots {
                exact_roots: BTreeSet::from([root, unknown]),
                exact_nodes: BTreeSet::from([root, dependent, unknown]),
            },
        }],
    );
    assert!(matches!(
        session.plan_patch(unknown_root, |_| {
            panic!("an unknown root must not reach evaluation")
        }),
        Err(IntentPlanError::Graph(IntentGraphError::UnknownNode(node))) if node == unknown
    ));

    let incomplete_closure = IntentPatch::new(
        identity_before,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::DeleteNode {
            node: root,
            policy: DeletePolicy::CascadeRoots {
                exact_roots: BTreeSet::from([root, other_root]),
                exact_nodes: BTreeSet::from([root, other_root]),
            },
        }],
    );
    assert!(matches!(
        session.plan_patch(incomplete_closure, |_| {
            panic!("a stale dependent closure must not reach evaluation")
        }),
        Err(IntentPlanError::CascadeMismatch { expected, actual })
            if expected == BTreeSet::from([root, dependent, other_root])
                && actual == BTreeSet::from([root, other_root])
    ));

    assert_eq!(session.to_canonical_json().unwrap(), canonical_before);
    assert_eq!(session.identity(), identity_before);
    assert_eq!(session.allocator_high_water(), allocator_before);
}

#[test]
fn closed_geometry_catalog_has_all_twenty_seven_recipes() {
    assert_eq!(GeometryRecipeKind::ALL.len(), 27);
    assert_eq!(
        GeometryRecipeKind::ALL
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        27
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one fixture proves both typed aggregate producers and their Profile Offset consumers"
)]
fn ordered_span_aggregates_supply_typed_chain_and_profile_offset_operands() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8334)).unwrap();
    let segment = |symbol: &str| {
        IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            key(symbol),
        )
    };
    let circle = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key("profile.circle"),
    );
    let chain = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        key("offset.chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias_port("first", IntentPortRole::Span, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        alias_port("second", IntentPortRole::Span, 0),
    );
    let profile = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::ClosedProfile,
        },
        key("offset.profile"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias_port("circle", IntentPortRole::Span, 0),
    );
    let chain_offset = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset,
        },
        key("offset.open-chain"),
    )
    .with_input(
        InputSlot::new(InputRole::Chain, 0),
        alias_port("chain", IntentPortRole::Chain, 0),
    )
    .with_field(
        IntentFieldKey(key("distance")),
        IntentLiteral::Quantity {
            value: 0.5,
            unit: IntentUnit::Length,
        },
    );
    let profile_dimension = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::ProfileOffset,
        },
        key("offset.closed-profile.distance"),
    )
    .with_input(
        InputSlot::new(InputRole::Profile, 0),
        alias_port("profile", IntentPortRole::Profile, 0),
    )
    .with_field(
        IntentFieldKey(key("source_traversal")),
        IntentLiteral::Enum(key("forward")),
    )
    .with_field(
        IntentFieldKey(key("target_traversal")),
        IntentLiteral::Enum(key("forward")),
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("first"),
                draft: Box::new(segment("span.first")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("second"),
                draft: Box::new(segment("span.second")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("circle"),
                draft: Box::new(circle),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("chain"),
                draft: Box::new(chain),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("profile"),
                draft: Box::new(profile),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("chain-offset"),
                draft: Box::new(chain_offset),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("profile-dimension"),
                draft: Box::new(profile_dimension),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let aliases = plan.aliases().clone();
    let first_span = aliases
        .port(
            &key("first"),
            IntentPortSelector::Node {
                role: IntentPortRole::Span,
                index: 0,
            },
        )
        .unwrap();
    let second_span = aliases
        .port(
            &key("second"),
            IntentPortSelector::Node {
                role: IntentPortRole::Span,
                index: 0,
            },
        )
        .unwrap();
    let chain_port = aliases
        .port(
            &key("chain"),
            IntentPortSelector::Node {
                role: IntentPortRole::Chain,
                index: 0,
            },
        )
        .unwrap();
    let profile_port = aliases
        .port(
            &key("profile"),
            IntentPortSelector::Node {
                role: IntentPortRole::Profile,
                index: 0,
            },
        )
        .unwrap();
    let chain_id = aliases.node(&key("chain")).unwrap();
    let profile_id = aliases.node(&key("profile")).unwrap();
    let chain_offset_id = aliases.node(&key("chain-offset")).unwrap();
    let profile_dimension_id = aliases.node(&key("profile-dimension")).unwrap();
    session.commit_plan(plan).unwrap();

    assert_eq!(chain_port.kind, IntentPortKind::Chain);
    assert_eq!(profile_port.kind, IntentPortKind::Profile);
    let chain_node = session.graph().node(chain_id).unwrap();
    assert!(chain_node.reservations.is_empty());
    assert_eq!(
        chain_node.inputs[&InputSlot::new(InputRole::Span, 0)],
        first_span
    );
    assert_eq!(
        chain_node.inputs[&InputSlot::new(InputRole::Span, 1)],
        second_span
    );
    let profile_node = session.graph().node(profile_id).unwrap();
    assert!(profile_node.reservations.is_empty());
    assert_eq!(
        session.graph().node(chain_offset_id).unwrap().inputs[&InputSlot::new(InputRole::Chain, 0)],
        chain_port
    );
    assert_eq!(
        session.graph().node(profile_dimension_id).unwrap().inputs
            [&InputSlot::new(InputRole::Profile, 0)],
        profile_port
    );

    // Unlike declaration presentation order, indexed span order is part of
    // the aggregate definition and therefore changes semantic identity.
    let before_reorder = session.semantic_identity();
    let reorder = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::RebindInput {
                node: chain_id,
                slot: InputSlot::new(InputRole::Span, 0),
                source: PatchPortRef::Stable { port: second_span },
            },
            IntentPatchOperation::RebindInput {
                node: chain_id,
                slot: InputSlot::new(InputRole::Span, 1),
                source: PatchPortRef::Stable { port: first_span },
            },
        ],
    );
    let plan = session.plan_patch(reorder, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    assert_ne!(session.semantic_identity(), before_reorder);
    assert_eq!(
        session.graph().node(chain_id).unwrap().inputs[&InputSlot::new(InputRole::Span, 0)],
        second_span
    );

    let semantic = session.semantic_identity();
    let rename = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::RenameNode {
            node: chain_id,
            name: key("Renamed open chain"),
        }],
    );
    let plan = session
        .plan_patch(rename, |_| panic!("display names must not materialize"))
        .unwrap();
    session.commit_plan(plan).unwrap();
    assert_eq!(session.semantic_identity(), semantic);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-driven oracle reviews every closed declaration schema"
)]
fn every_closed_declaration_schema_is_unique_coherent_and_minimally_admitted() {
    assert_eq!(GeometryRecipeKind::ALL.len(), 27);
    assert_eq!(ConstraintKind::ALL.len(), 35);
    assert_eq!(DimensionKind::ALL.len(), 8);
    assert_eq!(OperationKind::ALL.len(), 12);
    assert_eq!(ComputedFeatureKind::ALL.len(), 1);
    assert_eq!(AggregateKind::ALL.len(), 2);
    assert_eq!(ParameterIntentKind::ALL.len(), 3);
    assert_eq!(ExternalIntentKind::ALL.len(), 2);
    assert_eq!(BootstrapNativeKind::ALL.len(), 17);

    let cases = declaration_schema_cases();
    assert_eq!(cases.len(), 111);
    assert_eq!(
        cases
            .iter()
            .map(|case| case.label.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        cases.len()
    );

    for (index, case) in cases.iter().enumerate() {
        let schema = case.kind.schema(case.dynamic_children);
        let descriptors = case.kind.field_descriptors(case.dynamic_children);
        assert_eq!(
            descriptors
                .iter()
                .map(|descriptor| descriptor.schema.clone())
                .collect::<Vec<_>>(),
            schema.fields,
            "{} descriptor drifted from validation schema",
            case.label
        );
        assert!(descriptors.iter().all(|descriptor| {
            descriptor.edit == IntentEditClassification::Definition
                && matches!(
                    (&descriptor.schema.literal, &descriptor.choices),
                    (IntentLiteralSchema::Enum, IntentFieldChoices::Closed(_))
                        | (_, IntentFieldChoices::NotApplicable)
                )
                && matches!(
                    (&descriptor.schema.required, &descriptor.default),
                    (true, IntentFieldDefault::Required)
                        | (
                            false,
                            IntentFieldDefault::Literal(_)
                                | IntentFieldDefault::Contextual
                                | IntentFieldDefault::Conditional
                        )
                )
        }));
        for descriptor in &descriptors {
            if let IntentFieldChoices::Closed(values) = &descriptor.choices {
                assert!(!values.is_empty(), "{} has empty choices", case.label);
                assert_eq!(
                    values.iter().collect::<BTreeSet<_>>().len(),
                    values.len(),
                    "{} repeats choices for {:?}",
                    case.label,
                    descriptor.schema.field
                );
                if let IntentFieldDefault::Literal(IntentLiteral::Enum(value)) = &descriptor.default
                {
                    assert!(
                        values.contains(value),
                        "{} default for {:?} is outside its closed choices",
                        case.label,
                        descriptor.schema.field
                    );
                }
            }
        }
        assert!(
            schema.minimum_children <= schema.maximum_children,
            "{} has reversed child bounds",
            case.label
        );
        assert!(
            (schema.minimum_children..=schema.maximum_children).contains(&case.dynamic_children),
            "{} fixture children are outside its schema",
            case.label
        );

        let roles = schema
            .inputs
            .iter()
            .map(|cardinality| cardinality.role)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            roles.len(),
            schema.inputs.len(),
            "{} repeats an input role",
            case.label
        );
        for cardinality in &schema.inputs {
            assert!(
                cardinality.minimum <= cardinality.maximum,
                "{} has reversed {:?} input bounds",
                case.label,
                cardinality.role
            );
        }

        for choice in &schema.input_choices {
            assert!(
                choice.minimum <= choice.maximum
                    && usize::from(choice.maximum) <= choice.alternatives.len(),
                "{} has invalid choice bounds",
                case.label
            );
            assert_eq!(
                choice
                    .alternatives
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len(),
                choice.alternatives.len(),
                "{} repeats a choice alternative",
                case.label
            );
            for alternative in &choice.alternatives {
                let cardinality = schema
                    .inputs
                    .iter()
                    .find(|cardinality| cardinality.role == alternative.role)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} choice refers to undeclared role {:?}",
                            case.label, alternative.role
                        )
                    });
                assert!(
                    alternative.index < cardinality.maximum,
                    "{} choice refers outside {:?} bounds",
                    case.label,
                    alternative.role
                );
            }
        }

        assert_eq!(
            schema
                .fields
                .iter()
                .map(|field| field.field.clone())
                .collect::<BTreeSet<_>>()
                .len(),
            schema.fields.len(),
            "{} repeats a definition field",
            case.label
        );

        // Real stable sources are deliberately not fabricated by this schema
        // oracle. Reaching dependency resolution proves the complete minimal
        // declaration passed structural validation; input-free declarations
        // proceed all the way to the materializer closure.
        let session = IntentSession::with_id(IntentSessionId::from_raw(
            0x8400 + u128::try_from(index).unwrap(),
        ))
        .unwrap();
        let identity = session.identity();
        let allocator = session.allocator_high_water();
        let draft = minimal_schema_draft(case, &format!("schema-minimal-{index:03}"));
        let has_inputs = !draft.inputs.is_empty();
        let evaluated = Cell::new(false);
        let patch = IntentPatch::new(
            identity,
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("declaration"),
                draft: Box::new(draft),
                cell: None,
            }],
        );
        let result = session.plan_patch(patch, |candidate| {
            evaluated.set(true);
            accepted(candidate)
        });
        if has_inputs {
            assert!(
                matches!(
                    result,
                    Err(IntentPlanError::Graph(IntentGraphError::UnknownNode(_)))
                ),
                "{} did not reach dependency resolution: {result:?}",
                case.label
            );
            assert!(
                !evaluated.get(),
                "{} evaluated with dummy inputs",
                case.label
            );
        } else {
            assert!(
                result.is_ok(),
                "{} rejected its minimal declaration: {result:?}",
                case.label
            );
            assert!(evaluated.get(), "{} did not reach evaluation", case.label);
        }
        assert_eq!(session.identity(), identity);
        assert_eq!(session.allocator_high_water(), allocator);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive omission inventory keeps every contextual and conditional declaration field visibly reviewed"
)]
fn descriptor_omission_categories_are_an_explicit_closed_inventory() {
    let mut contextual = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    for case in declaration_schema_cases() {
        for descriptor in case.kind.field_descriptors(case.dynamic_children) {
            let identity = format!("{}.{}", case.label, descriptor.schema.field.0.as_str());
            match descriptor.default {
                IntentFieldDefault::Contextual => {
                    contextual.insert(identity);
                }
                IntentFieldDefault::Conditional => {
                    conditional.insert(identity);
                }
                IntentFieldDefault::Required | IntentFieldDefault::Literal(_) => {}
            }
        }
    }

    assert_eq!(
        contextual,
        [
            "geometry.Segment.branch_direction",
            "geometry.Polyline.branch_direction_0000",
            "geometry.Polyline.branch_direction_0001",
            "geometry.MidpointLine.branch_direction",
            "geometry.ThreePointCenterRectangle.side_midpoint",
            "constraint.FixedPoint.target",
            "constraint.EndpointContinuity.first_rate",
            "constraint.EndpointContinuity.second_rate",
            "computed_feature.FilletSet.name",
            "annotation.offset",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        conditional,
        [
            "dimension.ProfileOffset.direction",
            "dimension.ProfileOffset.side",
            "operation.AssociativeFillet.first_local_lower",
            "operation.AssociativeFillet.first_local_upper",
            "operation.AssociativeFillet.first_anchor_parameter",
            "operation.AssociativeFillet.first_anchor_winding",
            "operation.AssociativeFillet.second_local_lower",
            "operation.AssociativeFillet.second_local_upper",
            "operation.AssociativeFillet.second_anchor_parameter",
            "operation.AssociativeFillet.second_anchor_winding",
            "operation.ProfileOffset.direction",
            "operation.ProfileOffset.side",
            "operation.ProfileOffset.first_traversal",
            "geometry.TangentArc.source_support",
            "geometry.TangentArc.source_range_lower",
            "geometry.TangentArc.source_range_upper",
            "constraint.PointOnCurve.contact_support",
            "constraint.PointOnCurve.contact_range_lower",
            "constraint.PointOnCurve.contact_range_upper",
            "constraint.LineCurveTangency.contact_support",
            "constraint.LineCurveTangency.contact_range_lower",
            "constraint.LineCurveTangency.contact_range_upper",
            "constraint.CurveDirection.contact_support",
            "constraint.CurveDirection.contact_range_lower",
            "constraint.CurveDirection.contact_range_upper",
            "constraint.LineCircleTangency.first_contact_support",
            "constraint.LineCircleTangency.first_contact_range_lower",
            "constraint.LineCircleTangency.first_contact_range_upper",
            "constraint.LineCircleTangency.second_contact_support",
            "constraint.LineCircleTangency.second_contact_range_lower",
            "constraint.LineCircleTangency.second_contact_range_upper",
            "constraint.CircleArcTangency.first_contact_support",
            "constraint.CircleArcTangency.first_contact_range_lower",
            "constraint.CircleArcTangency.first_contact_range_upper",
            "constraint.CircleArcTangency.second_contact_support",
            "constraint.CircleArcTangency.second_contact_range_lower",
            "constraint.CircleArcTangency.second_contact_range_upper",
            "constraint.CurveCurveContact.first_contact_support",
            "constraint.CurveCurveContact.first_contact_range_lower",
            "constraint.CurveCurveContact.first_contact_range_upper",
            "constraint.CurveCurveContact.second_contact_support",
            "constraint.CurveCurveContact.second_contact_range_lower",
            "constraint.CurveCurveContact.second_contact_range_upper",
            "constraint.CurveCurveTangency.first_contact_support",
            "constraint.CurveCurveTangency.first_contact_range_lower",
            "constraint.CurveCurveTangency.first_contact_range_upper",
            "constraint.CurveCurveTangency.second_contact_support",
            "constraint.CurveCurveTangency.second_contact_range_lower",
            "constraint.CurveCurveTangency.second_contact_range_upper",
            "constraint.EqualCurvature.first_contact_support",
            "constraint.EqualCurvature.first_contact_range_lower",
            "constraint.EqualCurvature.first_contact_range_upper",
            "constraint.EqualCurvature.second_contact_support",
            "constraint.EqualCurvature.second_contact_range_lower",
            "constraint.EqualCurvature.second_contact_range_upper",
            "constraint.EndpointContinuity.first_contact_support",
            "constraint.EndpointContinuity.first_contact_range_lower",
            "constraint.EndpointContinuity.first_contact_range_upper",
            "constraint.EndpointContinuity.second_contact_support",
            "constraint.EndpointContinuity.second_contact_range_lower",
            "constraint.EndpointContinuity.second_contact_range_upper",
            "constraint.LineLineFillet.first_contact_support",
            "constraint.LineLineFillet.first_contact_range_lower",
            "constraint.LineLineFillet.first_contact_range_upper",
            "constraint.LineLineFillet.second_contact_support",
            "constraint.LineLineFillet.second_contact_range_lower",
            "constraint.LineLineFillet.second_contact_range_upper",
            "constraint.CurveCurveFillet.first_contact_support",
            "constraint.CurveCurveFillet.first_contact_range_lower",
            "constraint.CurveCurveFillet.first_contact_range_upper",
            "constraint.CurveCurveFillet.second_contact_support",
            "constraint.CurveCurveFillet.second_contact_range_lower",
            "constraint.CurveCurveFillet.second_contact_range_upper",
            "computed_feature.FilletSet.corner_0000_first_local_lower",
            "computed_feature.FilletSet.corner_0000_first_local_upper",
            "computed_feature.FilletSet.corner_0000_first_anchor_parameter",
            "computed_feature.FilletSet.corner_0000_first_anchor_winding",
            "computed_feature.FilletSet.corner_0000_second_local_lower",
            "computed_feature.FilletSet.corner_0000_second_local_upper",
            "computed_feature.FilletSet.corner_0000_second_anchor_parameter",
            "computed_feature.FilletSet.corner_0000_second_anchor_winding",
            "external.Binding.topology_digest",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one reviewed table freezes every family-specific native omission fallback"
)]
fn descriptor_static_defaults_match_the_native_declaration_fallbacks() {
    let defaults = [
        (
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::RationalQuadraticConic,
            },
            0,
            "weighted_middle",
            IntentLiteral::Point([1.0, 1.0]),
        ),
        (
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::OpenControlNurbs,
            },
            4,
            "degree",
            IntentLiteral::Natural(3),
        ),
        (
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::PeriodicControlNurbs,
            },
            4,
            "gauge_index",
            IntentLiteral::Natural(0),
        ),
        (
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::FixedCoordinate,
            },
            0,
            "target",
            IntentLiteral::Quantity {
                value: 0.0,
                unit: IntentUnit::Length,
            },
        ),
        (
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::CircleCircleTangency,
            },
            0,
            "center_direction",
            IntentLiteral::Point([1.0, 0.0]),
        ),
        (
            IntentNodeKind::Dimension {
                dimension: DimensionKind::SupportingLineOffset,
            },
            0,
            "side",
            IntentLiteral::Enum(key("left")),
        ),
        (
            IntentNodeKind::Dimension {
                dimension: DimensionKind::ExactTranslatedSegmentOffset,
            },
            0,
            "orientation",
            IntentLiteral::Enum(key("same")),
        ),
    ];
    for (kind, dynamic_children, field, expected) in defaults {
        assert_eq!(
            field_descriptor(&kind, dynamic_children, field).default,
            IntentFieldDefault::Literal(expected),
            "{kind:?}.{field}"
        );
    }

    let contact_defaults = [
        (
            ConstraintKind::PointOnCurve,
            "contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::LineCurveTangency,
            "contact",
            0.0,
            "bounded",
            "start",
            "aligned",
        ),
        (
            ConstraintKind::LineCircleTangency,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::LineCircleTangency,
            "second_contact",
            0.0,
            "periodic",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::CircleArcTangency,
            "first_contact",
            0.0,
            "periodic",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::CircleArcTangency,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::CurveCurveContact,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::CurveCurveContact,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::CurveCurveTangency,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::CurveCurveTangency,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "aligned",
        ),
        (
            ConstraintKind::CurveDirection,
            "contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::EqualCurvature,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::EqualCurvature,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::EndpointContinuity,
            "first_contact",
            1.0,
            "bounded",
            "end",
            "none",
        ),
        (
            ConstraintKind::EndpointContinuity,
            "second_contact",
            0.0,
            "bounded",
            "start",
            "none",
        ),
        (
            ConstraintKind::LineLineFillet,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::LineLineFillet,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::CurveCurveFillet,
            "first_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
        (
            ConstraintKind::CurveCurveFillet,
            "second_contact",
            0.5,
            "bounded",
            "interior",
            "none",
        ),
    ];
    for (constraint, prefix, parameter, _domain, neighborhood, orientation) in contact_defaults {
        let kind = IntentNodeKind::Constraint { constraint };
        assert_eq!(
            field_descriptor(&kind, 0, &format!("{prefix}_parameter")).default,
            IntentFieldDefault::Literal(IntentLiteral::Quantity {
                value: parameter,
                unit: IntentUnit::Dimensionless,
            }),
            "{constraint:?}.{prefix}.parameter"
        );
        assert_eq!(
            field_descriptor(&kind, 0, &format!("{prefix}_winding")).default,
            IntentFieldDefault::Literal(IntentLiteral::Integer(0)),
            "{constraint:?}.{prefix}.winding"
        );
        for (suffix, expected) in [("neighborhood", neighborhood), ("orientation", orientation)] {
            assert_eq!(
                field_descriptor(&kind, 0, &format!("{prefix}_{suffix}")).default,
                IntentFieldDefault::Literal(IntentLiteral::Enum(key(expected))),
                "{constraint:?}.{prefix}.{suffix}"
            );
        }
        for (suffix, value) in [("neighborhood_lower", 0.0), ("neighborhood_upper", 1.0)] {
            assert_eq!(
                field_descriptor(&kind, 0, &format!("{prefix}_{suffix}")).default,
                IntentFieldDefault::Literal(IntentLiteral::Quantity {
                    value,
                    unit: IntentUnit::Dimensionless,
                }),
                "{constraint:?}.{prefix}.{suffix}"
            );
        }
    }
}

#[test]
fn descriptor_branch_choice_vocabularies_are_exact_and_profile_offset_is_driving_only() {
    let cases: Vec<(IntentNodeKind, u16, &str, &[&str])> = vec![
        (
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::PointOnCurve,
            },
            0,
            "contact_orientation",
            &["none", "unoriented", "aligned", "opposed"],
        ),
        (
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::CircleArcTangency,
            },
            0,
            "side",
            &["outside_arc", "inside_arc"],
        ),
        (
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::CurveDirection,
            },
            0,
            "relation",
            &["tangent", "normal"],
        ),
        (
            IntentNodeKind::Dimension {
                dimension: DimensionKind::ProfileOffset,
            },
            0,
            "mode",
            &["driving"],
        ),
        (
            IntentNodeKind::Dimension {
                dimension: DimensionKind::ProfileOffset,
            },
            0,
            "source_traversal",
            &["forward", "reverse"],
        ),
        (
            IntentNodeKind::Operation {
                operation: OperationKind::AssociativeFillet,
            },
            0,
            "first_neighborhood",
            &["interior", "local", "start", "end"],
        ),
        (
            IntentNodeKind::Operation {
                operation: OperationKind::ProfileOffset,
            },
            0,
            "direction",
            &["outward", "inward"],
        ),
        (
            IntentNodeKind::ComputedFeature {
                feature: ComputedFeatureKind::FilletSet,
            },
            1,
            "corner_0000_endpoint_order",
            &["first_then_second", "second_then_first"],
        ),
        (
            IntentNodeKind::Parameter {
                parameter: ParameterIntentKind::Parameter,
            },
            0,
            "kind",
            &["length", "angle", "dimensionless", "activation"],
        ),
        (
            IntentNodeKind::External {
                external: ExternalIntentKind::Binding,
            },
            0,
            "feature_kind",
            &["point", "line_segment"],
        ),
    ];
    for (kind, dynamic_children, field, expected) in cases {
        assert_eq!(
            field_descriptor(&kind, dynamic_children, field).choices,
            closed_choices(expected),
            "{kind:?}.{field}"
        );
    }
}

#[test]
fn profile_offset_traversals_stay_named_fields_while_tangent_arc_source_is_nested_contact_state() {
    let profile_offset = IntentNodeKind::Dimension {
        dimension: DimensionKind::ProfileOffset,
    };
    assert_semantic_path(
        &field_descriptor(&profile_offset, 0, "source_traversal").path,
        &serde_json::json!(["sourceTraversal"]),
    );
    assert_semantic_path(
        &field_descriptor(&profile_offset, 0, "target_traversal").path,
        &serde_json::json!(["targetTraversal"]),
    );

    let tangent_arc = IntentNodeKind::Geometry {
        recipe: GeometryRecipeKind::TangentArc,
    };
    assert_semantic_path(
        &field_descriptor(&tangent_arc, 0, "source_parameter").path,
        &serde_json::json!(["source", "contact", "parameter"]),
    );
    assert_semantic_path(
        &field_descriptor(&tangent_arc, 0, "source_neighborhood").path,
        &serde_json::json!(["source", "contact", "neighborhood", "kind"]),
    );
}

#[test]
fn low_control_nurbs_require_an_explicit_admissible_degree() {
    for (index, (recipe, dynamic_children, degree)) in [
        (GeometryRecipeKind::OpenControlNurbs, 2, 1),
        (GeometryRecipeKind::OpenControlNurbs, 3, 2),
        (GeometryRecipeKind::PeriodicControlNurbs, 3, 2),
    ]
    .into_iter()
    .enumerate()
    {
        let kind = IntentNodeKind::Geometry { recipe };
        let descriptor = field_descriptor(&kind, dynamic_children, "degree");
        assert!(descriptor.schema.required);
        assert_eq!(descriptor.default, IntentFieldDefault::Required);

        let case = DeclarationSchemaCase {
            label: format!("low-control.{recipe:?}"),
            kind: kind.clone(),
            dynamic_children,
        };
        let mut omitted = minimal_schema_draft(&case, &format!("nurbs-omitted-{index}"));
        omitted.fields.remove(&IntentFieldKey(key("degree")));
        assert!(matches!(
            atomic_schema_rejection(&case.label, omitted),
            IntentPlanError::Graph(IntentGraphError::MissingRequiredDefinitionField { .. })
        ));

        let session = IntentSession::with_id(IntentSessionId::from_raw(
            0x83d0_0000 + u128::try_from(index).unwrap(),
        ))
        .unwrap();
        let explicit = IntentNodeDraft::new(kind, key(&format!("nurbs-explicit-{index}")))
            .with_dynamic_children(dynamic_children)
            .with_field(
                IntentFieldKey(key("degree")),
                IntentLiteral::Natural(degree),
            );
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("declaration"),
                draft: Box::new(explicit),
                cell: None,
            }],
        );
        assert!(session.plan_patch(patch, accepted).is_ok());
    }

    for recipe in [
        GeometryRecipeKind::OpenControlNurbs,
        GeometryRecipeKind::PeriodicControlNurbs,
    ] {
        let descriptor = field_descriptor(&IntentNodeKind::Geometry { recipe }, 4, "degree");
        assert!(!descriptor.schema.required);
        assert_eq!(
            descriptor.default,
            IntentFieldDefault::Literal(IntentLiteral::Natural(3))
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive malformed-operand matrix is intentionally table driven"
)]
fn every_closed_declaration_rejects_malformed_operands_and_children_atomically() {
    let cases = declaration_schema_cases();
    let mut missing_operands = 0_usize;
    let mut choice_underflows = 0_usize;
    let mut choice_overflows = 0_usize;
    let mut sparse_geometry_point_aliases = 0_usize;
    let mut child_underflows = 0_usize;

    for (index, case) in cases.iter().enumerate() {
        let schema = case.kind.schema(case.dynamic_children);
        for cardinality in schema
            .inputs
            .iter()
            .filter(|cardinality| cardinality.minimum > 0)
        {
            let mut draft = minimal_schema_draft(case, &format!("missing-input-{index:03}"));
            let slot = InputSlot::new(cardinality.role, cardinality.minimum - 1);
            draft.inputs.remove(&slot);
            let error = atomic_schema_rejection(&case.label, draft);
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::MissingRequiredInput { .. })
                ),
                "{} missing {:?} returned {error:?}",
                case.label,
                cardinality.role
            );
            missing_operands += 1;
        }

        for choice in &schema.input_choices {
            if choice.minimum > 0 {
                let mut draft = minimal_schema_draft(case, &format!("choice-under-{index:03}"));
                for slot in &choice.alternatives {
                    draft.inputs.remove(slot);
                }
                let error = atomic_schema_rejection(&case.label, draft);
                assert!(
                    matches!(
                        error,
                        IntentPlanError::Graph(IntentGraphError::InputChoiceCardinality { .. })
                    ),
                    "{} choice underflow returned {error:?}",
                    case.label
                );
                choice_underflows += 1;
            }
            if usize::from(choice.maximum) < choice.alternatives.len() {
                let mut draft = minimal_schema_draft(case, &format!("choice-over-{index:03}"));
                for slot in choice
                    .alternatives
                    .iter()
                    .take(usize::from(choice.maximum) + 1)
                {
                    draft.inputs.insert(*slot, dummy_input(&case.kind, *slot));
                }
                let error = atomic_schema_rejection(&case.label, draft);
                assert!(
                    matches!(
                        error,
                        IntentPlanError::Graph(IntentGraphError::InputChoiceCardinality { .. })
                    ),
                    "{} choice overflow returned {error:?}",
                    case.label
                );
                choice_overflows += 1;
            }
        }

        for cardinality in schema
            .inputs
            .iter()
            .filter(|cardinality| cardinality.minimum == 0 && cardinality.maximum >= 2)
        {
            let mut draft = minimal_schema_draft(case, &format!("input-hole-{index:03}"));
            let slot = InputSlot::new(cardinality.role, 1);
            draft.inputs.insert(slot, dummy_input(&case.kind, slot));
            let error = atomic_schema_rejection(&case.label, draft);
            if (matches!(case.kind, IntentNodeKind::Geometry { .. })
                && cardinality.role == InputRole::Point)
                || (matches!(
                    case.kind,
                    IntentNodeKind::Operation {
                        operation: OperationKind::ProfileOffset
                    }
                ) && cardinality.role == InputRole::Profile)
            {
                // The schema intentionally admits this sparse authored slot;
                // the dummy source then fails at ordinary dependency lookup.
                assert!(
                    matches!(
                        error,
                        IntentPlanError::Graph(IntentGraphError::UnknownNode(_))
                    ),
                    "{} sparse {:?} input returned {error:?}",
                    case.label,
                    cardinality.role
                );
                sparse_geometry_point_aliases += 1;
            } else {
                assert!(
                    matches!(
                        error,
                        IntentPlanError::Graph(IntentGraphError::MissingRequiredInput { .. })
                    ),
                    "{} non-contiguous {:?} input returned {error:?}",
                    case.label,
                    cardinality.role
                );
            }
        }

        if !matches!(case.kind, IntentNodeKind::Bootstrap { .. }) {
            let mut draft = minimal_schema_draft(case, &format!("extra-input-{index:03}"));
            let slot = if let Some(cardinality) = schema.inputs.first() {
                InputSlot::new(cardinality.role, cardinality.maximum)
            } else {
                let role = match case.kind {
                    IntentNodeKind::Geometry { .. }
                    | IntentNodeKind::Constraint { .. }
                    | IntentNodeKind::Dimension { .. }
                    | IntentNodeKind::Operation { .. }
                    | IntentNodeKind::ComputedFeature { .. }
                    | IntentNodeKind::Aggregate { .. }
                    | IntentNodeKind::Annotation => InputRole::Point,
                    IntentNodeKind::Parameter { .. } => InputRole::Scalar,
                    IntentNodeKind::External { .. } => InputRole::External,
                    IntentNodeKind::Identity { .. } => InputRole::Identity,
                    IntentNodeKind::Bootstrap { .. } => unreachable!(),
                };
                InputSlot::new(role, 0)
            };
            draft.inputs.insert(slot, dummy_input(&case.kind, slot));
            let error = atomic_schema_rejection(&case.label, draft);
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::UnexpectedInputSlot { .. })
                ),
                "{} extra input returned {error:?}",
                case.label
            );
        }

        if schema.minimum_children > 0 {
            let invalid_case = DeclarationSchemaCase {
                label: case.label.clone(),
                kind: case.kind.clone(),
                dynamic_children: schema.minimum_children - 1,
            };
            let draft = minimal_schema_draft(&invalid_case, &format!("child-under-{index:03}"));
            let error = atomic_schema_rejection(&case.label, draft);
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::ChildCardinality { .. })
                ),
                "{} child underflow returned {error:?}",
                case.label
            );
            child_underflows += 1;
        }
    }

    assert!(missing_operands > 0);
    assert!(choice_underflows > 0);
    assert!(choice_overflows > 0);
    assert!(sparse_geometry_point_aliases > 0);
    assert_eq!(child_underflows, 7);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive definition-field matrix is intentionally table driven"
)]
fn every_closed_declaration_rejects_malformed_definition_fields_atomically() {
    let cases = declaration_schema_cases();
    let mut required_fields = 0_usize;
    let mut typed_fields = 0_usize;
    let mut quantity_fields = 0_usize;

    for (index, case) in cases.iter().enumerate() {
        let schema = case.kind.schema(case.dynamic_children);
        for field in schema.fields.iter().filter(|field| field.required) {
            let mut draft = minimal_schema_draft(case, &format!("missing-field-{index:03}"));
            draft.fields.remove(&field.field);
            let error = atomic_schema_rejection(&case.label, draft);
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::MissingRequiredDefinitionField { .. })
                ),
                "{} missing {:?} returned {error:?}",
                case.label,
                field.field
            );
            required_fields += 1;
        }

        let mut unknown = minimal_schema_draft(case, &format!("unknown-field-{index:03}"));
        unknown.fields.insert(
            IntentFieldKey(key("schema.unknown")),
            IntentLiteral::Boolean(false),
        );
        let error = atomic_schema_rejection(&case.label, unknown);
        assert!(
            matches!(
                error,
                IntentPlanError::Graph(IntentGraphError::UnknownDefinitionField { .. })
            ),
            "{} unknown field returned {error:?}",
            case.label
        );

        for field in &schema.fields {
            let mut draft = minimal_schema_draft(case, &format!("wrong-field-{index:03}"));
            draft
                .fields
                .insert(field.field.clone(), wrong_literal_for_schema(field.literal));
            let error = atomic_schema_rejection(&case.label, draft);
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::DefinitionFieldLiteralMismatch { .. })
                ),
                "{} wrong {:?} literal returned {error:?}",
                case.label,
                field.field
            );
            typed_fields += 1;

            if let IntentLiteralSchema::Quantity(unit) = field.literal {
                let mut draft = minimal_schema_draft(case, &format!("wrong-unit-{index:03}"));
                draft.fields.insert(
                    field.field.clone(),
                    IntentLiteral::Quantity {
                        value: 1.0,
                        unit: different_unit(unit),
                    },
                );
                let error = atomic_schema_rejection(&case.label, draft);
                assert!(
                    matches!(
                        error,
                        IntentPlanError::Graph(
                            IntentGraphError::DefinitionFieldLiteralMismatch { .. }
                        )
                    ),
                    "{} wrong {:?} unit returned {error:?}",
                    case.label,
                    field.field
                );
                quantity_fields += 1;
            }
        }
    }

    assert!(required_fields > 0);
    assert!(typed_fields > 0);
    assert!(quantity_fields > 0);
}

#[test]
fn unordered_patch_permutations_allocate_identical_nodes_ports_and_bytes() {
    fn operations_with_aliases(
        left: &str,
        right: &str,
        relation_alias: &str,
    ) -> Vec<IntentPatchOperation> {
        let relation = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::Coincident,
            },
            key("Coincident"),
        )
        .with_input(InputSlot::new(InputRole::Point, 0), alias_point(left))
        .with_input(InputSlot::new(InputRole::Point, 1), alias_point(right));
        vec![
            IntentPatchOperation::CreateNode {
                alias: key(relation_alias),
                draft: Box::new(relation),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key(right),
                draft: Box::new(point_draft("B")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key(left),
                draft: Box::new(point_draft("A")),
                cell: None,
            },
        ]
    }

    fn operations() -> Vec<IntentPatchOperation> {
        operations_with_aliases("a", "b", "constraint")
    }

    let id = IntentSessionId::from_raw(0x8301);
    let mut first = IntentSession::with_id(id).unwrap();
    let mut second = IntentSession::with_id(id).unwrap();
    let mut renamed_aliases = IntentSession::with_id(id).unwrap();
    let first_patch = IntentPatch::new(
        first.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        operations(),
    );
    let mut reversed = operations();
    reversed.reverse();
    let second_patch = IntentPatch::new(
        second.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        reversed,
    );
    let renamed_alias_patch = IntentPatch::new(
        renamed_aliases.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        operations_with_aliases("temporary-left", "temporary-right", "temporary-relation"),
    );
    let first_plan = first.plan_patch(first_patch, accepted).unwrap();
    let second_plan = second.plan_patch(second_patch, accepted).unwrap();
    let renamed_alias_plan = renamed_aliases
        .plan_patch(renamed_alias_patch, accepted)
        .unwrap();
    assert_eq!(first_plan.aliases(), second_plan.aliases());
    assert_eq!(first_plan.target(), second_plan.target());
    assert_eq!(first_plan.target(), renamed_alias_plan.target());
    assert_eq!(first_plan.token(), second_plan.token());
    first.commit_plan(first_plan).unwrap();
    second.commit_plan(second_plan).unwrap();
    renamed_aliases.commit_plan(renamed_alias_plan).unwrap();
    assert_eq!(
        first.to_canonical_json().unwrap(),
        second.to_canonical_json().unwrap()
    );
    assert_eq!(
        first.to_canonical_json().unwrap(),
        renamed_aliases.to_canonical_json().unwrap()
    );
    assert_eq!(
        first.graph().canonical_schedule().unwrap(),
        vec![
            NodeId::from_raw(1),
            NodeId::from_raw(2),
            NodeId::from_raw(3)
        ]
    );
}

#[test]
fn atomic_forward_alias_cycle_rejects_without_allocating_or_calling_materializer() {
    let session = IntentSession::with_id(IntentSessionId::from_raw(0x8302)).unwrap();
    let identity = session.identity();
    let allocator = session.allocator_high_water();
    let identity_draft = |name: &str, source: &str| {
        IntentNodeDraft::new(
            IntentNodeKind::Identity {
                transition: IdentityTransitionKind::Alias,
                port_kind: IntentPortKind::Point,
            },
            key(name),
        )
        .with_input(
            InputSlot::new(InputRole::Identity, 0),
            PatchPortRef::Alias {
                node: key(source),
                selector: identity_result_selector(),
            },
        )
    };
    let patch = IntentPatch::new(
        identity,
        IntentPatchPolicy::RetainFailedIntent,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("a"),
                draft: Box::new(identity_draft("A", "b")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("b"),
                draft: Box::new(identity_draft("B", "a")),
                cell: None,
            },
        ],
    );
    assert!(matches!(
        session.plan_patch(patch, |_| panic!("cycle must reject before evaluation")),
        Err(IntentPlanError::Graph(
            IntentGraphError::DependencyCycle { .. }
        ))
    ));
    assert_eq!(session.identity(), identity);
    assert_eq!(session.allocator_high_water(), allocator);
}

#[test]
fn two_continuations_of_one_generation_are_a_rejected_fork() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8303)).unwrap();
    let (_, point) = create_point(&mut session, "point");
    let continuation = |name: &str| {
        IntentNodeDraft::new(
            IntentNodeKind::Identity {
                transition: IdentityTransitionKind::Continue,
                port_kind: IntentPortKind::Point,
            },
            key(name),
        )
        .with_input(
            InputSlot::new(InputRole::Identity, 0),
            PatchPortRef::Stable { port: point },
        )
    };
    let before = session.identity();
    let patch = IntentPatch::new(
        before,
        IntentPatchPolicy::RetainFailedIntent,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("first"),
                draft: Box::new(continuation("First")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("second"),
                draft: Box::new(continuation("Second")),
                cell: None,
            },
        ],
    );
    assert!(matches!(
        session.plan_patch(patch, |_| panic!("fork must reject before evaluation")),
        Err(IntentPlanError::Graph(
            IntentGraphError::IdentityFork { .. }
        ))
    ));
    assert_eq!(session.identity(), before);
}

#[test]
fn reference_to_a_retired_identity_rejects_atomically() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8304)).unwrap();
    let (_, point) = create_point(&mut session, "point");
    let retire = IntentNodeDraft::new(
        IntentNodeKind::Identity {
            transition: IdentityTransitionKind::Retire,
            port_kind: IntentPortKind::Point,
        },
        key("Retire"),
    )
    .with_input(
        InputSlot::new(InputRole::Identity, 0),
        PatchPortRef::Stable { port: point },
    );
    let constraint = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::FixedPoint,
        },
        key("Fixed"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: point },
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("retire"),
                draft: Box::new(retire),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("dependent"),
                draft: Box::new(constraint),
                cell: None,
            },
        ],
    );
    assert!(matches!(
        session.plan_patch(patch, |_| panic!("retired reference is structural")),
        Err(IntentPlanError::Graph(
            IntentGraphError::RetiredReference { .. }
        ))
    ));
}

#[test]
fn organization_and_instance_revisions_are_independent() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8305)).unwrap();
    let (point, port) = create_point(&mut session, "point");
    let baseline = session.identity();
    let rename = IntentPatch::new(
        baseline,
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::RenameNode {
            node: point,
            name: key("Renamed point"),
        }],
    );
    let plan = session
        .plan_patch(rename, |_| panic!("organization-only edits do not solve"))
        .unwrap();
    assert_eq!(plan.target().graph, baseline.graph);
    assert_eq!(plan.target().instance, baseline.instance);
    assert_ne!(plan.target().organization, baseline.organization);
    session.commit_plan(plan).unwrap();

    let renamed = session.identity();
    let leaf = LeafRef {
        node: point,
        port: port.port,
        field: LeafField::X,
    };
    let edit = IntentPatch::new(
        renamed,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::SetInstanceLeaf {
            leaf,
            value: IntentLiteral::Quantity {
                value: 12.5,
                unit: IntentUnit::Length,
            },
        }],
    );
    let plan = session.plan_patch(edit, accepted).unwrap();
    assert_eq!(plan.target().graph, renamed.graph);
    assert_ne!(plan.target().instance, renamed.instance);
    assert_eq!(plan.target().organization, renamed.organization);
    session.commit_plan(plan).unwrap();
}

#[test]
fn failed_explicit_intent_retains_previous_accepted_scene_and_undo_restores_it() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8306)).unwrap();
    create_point(&mut session, "accepted");
    let accepted_before = session.accepted().unwrap().clone();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key("failed"),
            draft: Box::new(point_draft("Failed")),
            cell: None,
        }],
    );
    let plan = session
        .plan_patch(patch, |candidate| IntentEvaluation::Failed {
            failure: IntentEvaluationFailure {
                kind: IntentEvaluationFailureKind::SolverRejected,
                failed_nodes: candidate.diff().created_nodes.clone(),
                diagnostic: key("no-valid-solution"),
            },
        })
        .unwrap();
    assert_eq!(
        plan.disposition(),
        geosolve_sketch_intent::IntentPlanDisposition::RetainedFailed
    );
    session.commit_plan(plan).unwrap();
    assert_eq!(session.graph().nodes().len(), 2);
    assert_eq!(session.accepted(), Some(&accepted_before));
    assert_eq!(
        session.latest_attempt().unwrap().disposition,
        geosolve_sketch_intent::IntentAttemptDisposition::RetainedFailed
    );
    session.undo().unwrap().unwrap();
    assert_eq!(session.graph().nodes().len(), 1);
    let restored = session.accepted().unwrap();
    assert_eq!(restored.target, session.semantic_identity());
    assert_eq!(restored.graph, *session.graph());
    assert_eq!(restored.instance, *session.instance());
    assert_eq!(restored.evidence, accepted_before.evidence);
}

#[test]
fn require_accepted_failure_and_transient_work_publish_nothing() {
    for kind in [
        IntentEvaluationFailureKind::SolverRejected,
        IntentEvaluationFailureKind::Cancelled,
        IntentEvaluationFailureKind::Exhausted,
        IntentEvaluationFailureKind::Stale,
    ] {
        let session =
            IntentSession::with_id(IntentSessionId::from_raw(0x8310 + kind as u128)).unwrap();
        let before = session.identity();
        let patch = IntentPatch::new(
            before,
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point_draft("Point")),
                cell: None,
            }],
        );
        let result = session.plan_patch(patch, |candidate| IntentEvaluation::Failed {
            failure: IntentEvaluationFailure {
                kind,
                failed_nodes: candidate.diff().created_nodes.clone(),
                diagnostic: key("rejected"),
            },
        });
        assert!(result.is_err());
        assert_eq!(session.identity(), before);
        assert!(session.graph().nodes().is_empty());
        assert_eq!(session.undo_len(), 0);
    }
}

#[test]
fn undo_redo_restore_exact_ids_while_divergence_never_reuses_them() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8307)).unwrap();
    let (first, first_port) = create_point(&mut session, "first");
    assert_eq!(first, NodeId::from_raw(1));
    session.undo().unwrap().unwrap();
    assert!(session.graph().nodes().is_empty());
    session.redo().unwrap().unwrap();
    assert!(session.graph().nodes().contains_key(&first));
    assert!(session.graph().port(first_port).is_some());

    session.undo().unwrap().unwrap();
    let (second, second_port) = create_point(&mut session, "second");
    assert_eq!(second, NodeId::from_raw(2));
    assert_ne!(first_port.port, second_port.port);
    assert_eq!(session.redo_len(), 0);
}

#[test]
fn exact_cas_plan_and_canonical_session_round_trip_are_authenticated() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8308)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point_draft("Point")),
            cell: None,
        }],
    );
    let stale_plan = session.plan_patch(patch.clone(), accepted).unwrap();
    let current_plan = session.plan_patch(patch, accepted).unwrap();
    session.commit_plan(current_plan).unwrap();
    assert!(session.commit_plan(stale_plan).is_err());

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(restored.identity(), session.identity());
    assert!(matches!(
        IntentSession::from_json(&format!(" {canonical}")),
        Err(geosolve_sketch_intent::IntentSessionError::NonCanonicalJson)
    ));

    let mut tampered: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    tampered["revision"] = serde_json::json!("00000000000000ff");
    assert!(IntentSession::from_json(&serde_json::to_string(&tampered).unwrap()).is_err());
}

#[test]
fn ordinary_geometry_reuse_is_a_schema_derived_alias_without_duplicate_native_identity() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8320)).unwrap();
    let (_, existing) = create_point(&mut session, "existing");
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("Segment"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: existing },
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("segment"),
            draft: Box::new(draft),
            cell: None,
        }],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let node = plan.aliases().node(&key("segment")).unwrap();
    let start = plan
        .aliases()
        .port(
            &key("segment"),
            IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            },
        )
        .unwrap();
    let end = plan
        .aliases()
        .port(
            &key("segment"),
            IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            },
        )
        .unwrap();
    session.commit_plan(plan).unwrap();

    let segment = session.graph().node(node).unwrap();
    assert_eq!(
        segment.port(start.port).unwrap().flow,
        IntentIdentityFlow::Aliased { source: existing }
    );
    assert!(segment.port(start.port).unwrap().writable.is_empty());
    assert!(matches!(
        segment.port(end.port).unwrap().flow,
        IntentIdentityFlow::Created { .. }
    ));
    assert_eq!(
        segment
            .reservations
            .values()
            .filter(|reservation| reservation.kind == IntentNativeReservationKind::Point)
            .count(),
        1
    );
}

#[test]
fn concrete_descriptor_projects_the_allocated_output_and_edit_contract() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_38)).unwrap();
    let (node, primary) = create_point(&mut session, "descriptor.point");
    let declaration = session.graph().node(node).unwrap();
    let descriptor = declaration.descriptor();

    assert_eq!(descriptor.schema, declaration.kind.schema(0));
    assert_eq!(
        descriptor.suppression_edit,
        IntentEditClassification::Definition
    );
    assert_eq!(
        descriptor.input_edit,
        IntentEditClassification::InputBinding
    );
    assert_eq!(descriptor.name_edit, IntentEditClassification::Organization);
    let output = descriptor
        .outputs
        .iter()
        .find(|output| output.port == primary)
        .expect("primary point output");
    assert_eq!(output.writable, vec![LeafField::X, LeafField::Y]);
    assert_eq!(output.native, Some(IntentNativeReservationKind::Point));
    assert_eq!(output.edit, IntentEditClassification::Instance);
    assert!(matches!(
        output.flow,
        IntentIdentityFlow::Created { reservation }
            if declaration.reservations.contains_key(&reservation)
    ));
    assert_semantic_path(&output.path, &serde_json::json!(["point"]));
    assert_descriptor_target_paths_are_bijective(declaration);
}

#[test]
fn semantic_projection_path_deserialization_enforces_its_public_shape_and_bound() {
    let valid: IntentProjectionPath =
        serde_json::from_str(r#"["corners",1,"parents",0,"parameter"]"#).unwrap();
    assert_semantic_path(
        &valid,
        &serde_json::json!(["corners", 1, "parents", 0, "parameter"]),
    );

    for malformed in ["[]", "[0,\"point\"]", "[\"\"]"] {
        assert!(
            serde_json::from_str::<IntentProjectionPath>(malformed).is_err(),
            "malformed semantic path was admitted: {malformed}"
        );
    }
    let oversized = serde_json::Value::Array(
        std::iter::repeat_n(
            serde_json::Value::String("nested".to_owned()),
            MAX_INTENT_PROJECTION_PATH_SEGMENTS + 1,
        )
        .collect(),
    );
    assert!(serde_json::from_value::<IntentProjectionPath>(oversized).is_err());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused output-shape fixture compares singleton and paired named contact objects"
)]
fn singleton_and_paired_contacts_are_named_objects_with_typed_leaves() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_51)).unwrap();
    let point_on_curve = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::PointOnCurve,
        },
        key("semantic.point-on-curve"),
    )
    .with_input(InputSlot::new(InputRole::Point, 0), alias_point("point"))
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias_port("first", IntentPortRole::Span, 0),
    );
    let curve_contact = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::CurveCurveContact,
        },
        key("semantic.curve-contact"),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        alias_port("first", IntentPortRole::Span, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        alias_port("second", IntentPortRole::Span, 0),
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point_draft("semantic.contact-point")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("first"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("semantic.first-span"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("second"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("semantic.second-span"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("singleton"),
                draft: Box::new(point_on_curve),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("repeated"),
                draft: Box::new(curve_contact),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let singleton = plan.aliases().node(&key("singleton")).unwrap();
    let repeated = plan.aliases().node(&key("repeated")).unwrap();
    session.commit_plan(plan).unwrap();

    let singleton = session.graph().node(singleton).unwrap().descriptor();
    assert_output_paths_form_an_unambiguous_tree(&singleton.outputs);
    let singleton_contact = singleton
        .outputs
        .iter()
        .find(|output| output.selector == selector(IntentPortRole::Contact, 0))
        .unwrap();
    let singleton_parameter = singleton
        .outputs
        .iter()
        .find(|output| output.selector == selector(IntentPortRole::Parameter, 0))
        .unwrap();
    assert_semantic_path(
        &singleton_contact.path,
        &serde_json::json!(["contact", "contact"]),
    );
    assert_semantic_path(
        &singleton_parameter.path,
        &serde_json::json!(["contact", "parameter"]),
    );

    let repeated = session.graph().node(repeated).unwrap().descriptor();
    assert_output_paths_form_an_unambiguous_tree(&repeated.outputs);
    for (index, side) in [(0_u16, "first"), (1, "second")] {
        let contact = repeated
            .outputs
            .iter()
            .find(|output| output.selector == selector(IntentPortRole::Contact, index))
            .unwrap();
        let parameter = repeated
            .outputs
            .iter()
            .find(|output| output.selector == selector(IntentPortRole::Parameter, index))
            .unwrap();
        assert_semantic_path(
            &contact.path,
            &serde_json::json!(["contacts", side, "contact"]),
        );
        assert_semantic_path(
            &parameter.path,
            &serde_json::json!(["contacts", side, "parameter"]),
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one integrated semantic fixture reviews fixed inputs, repeated children, NURBS controls, and nested Fillet parents together"
)]
fn semantic_descriptor_uses_named_fields_and_real_arrays_for_repeated_structure() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_50)).unwrap();
    let mut operations = vec![
        IntentPatchOperation::CreateNode {
            alias: key("start"),
            draft: Box::new(point_draft("semantic.start")),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("end"),
            draft: Box::new(point_draft("semantic.end")),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("segment"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("semantic.segment"),
                )
                .with_input(InputSlot::new(InputRole::Point, 0), alias_point("start"))
                .with_input(InputSlot::new(InputRole::Point, 1), alias_point("end")),
            ),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("polyline"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Polyline,
                    },
                    key("semantic.polyline"),
                )
                .with_dynamic_children(3),
            ),
            cell: None,
        },
        IntentPatchOperation::CreateNode {
            alias: key("nurbs"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::OpenControlNurbs,
                    },
                    key("semantic.nurbs"),
                )
                .with_dynamic_children(4),
            ),
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
    let fillet_kind = IntentNodeKind::ComputedFeature {
        feature: ComputedFeatureKind::FilletSet,
    };
    let mut fillet =
        IntentNodeDraft::new(fillet_kind.clone(), key("semantic.fillet")).with_dynamic_children(2);
    for index in 0..4_u16 {
        fillet = fillet.with_input(
            InputSlot::new(InputRole::Span, index),
            alias_port(&format!("parent-{index}"), IntentPortRole::Span, 0),
        );
    }
    for schema in fillet_kind.schema(2).fields {
        if schema.required {
            fillet = fillet.with_field(schema.field, literal_for_schema(schema.literal));
        }
    }
    operations.push(IntentPatchOperation::CreateNode {
        alias: key("fillet"),
        draft: Box::new(fillet),
        cell: None,
    });

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
    let segment = plan.aliases().node(&key("segment")).unwrap();
    let polyline = plan.aliases().node(&key("polyline")).unwrap();
    let nurbs = plan.aliases().node(&key("nurbs")).unwrap();
    let fillet = plan.aliases().node(&key("fillet")).unwrap();
    session.commit_plan(plan).unwrap();

    for node in session.graph().nodes().values() {
        assert_descriptor_target_paths_are_bijective(node);
    }

    let segment = session.graph().node(segment).unwrap().descriptor();
    assert_semantic_path(&segment.inputs[0].path, &serde_json::json!(["start"]));
    assert_semantic_path(&segment.inputs[1].path, &serde_json::json!(["end"]));

    let polyline = session.graph().node(polyline).unwrap().descriptor();
    let first_vertex = polyline
        .outputs
        .iter()
        .find(|output| {
            output.selector
                == IntentPortSelector::InitialChild {
                    ordinal: 0,
                    role: IntentPortRole::Corner,
                    index: 0,
                }
        })
        .unwrap();
    assert_semantic_path(
        &first_vertex.path,
        &serde_json::json!(["vertices", 0, "position"]),
    );

    let nurbs = session.graph().node(nurbs).unwrap().descriptor();
    let second_weight = nurbs
        .outputs
        .iter()
        .find(|output| {
            output.selector
                == IntentPortSelector::InitialChild {
                    ordinal: 1,
                    role: IntentPortRole::Target,
                    index: 0,
                }
        })
        .unwrap();
    assert_semantic_path(
        &second_weight.path,
        &serde_json::json!(["controls", 1, "weight"]),
    );

    let fillet = session.graph().node(fillet).unwrap().descriptor();
    let fourth_parent = fillet
        .inputs
        .iter()
        .find(|input| input.slot == InputSlot::new(InputRole::Span, 3))
        .unwrap();
    assert_semantic_path(
        &fourth_parent.path,
        &serde_json::json!(["corners", 1, "parents", 1]),
    );
    let second_corner_parameter = fillet
        .fields
        .iter()
        .find(|field| field.schema.field.0.as_str() == "corner_0001_second_parameter")
        .unwrap();
    assert_semantic_path(
        &second_corner_parameter.path,
        &serde_json::json!(["corners", 1, "parents", 1, "parameter"]),
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one inventory test keeps native operation handles and logical-only ports visibly distinct"
)]
fn authored_curve_handles_and_logical_ports_never_masquerade_as_native_sketch_ids() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8321)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("circle"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::ThreePointCircle,
                    },
                    key("Three-point circle"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("operation"),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Operation {
                            operation: OperationKind::Rectangle,
                        },
                        key("Rectangle operation"),
                    )
                    .with_field(
                        IntentFieldKey(key("origin")),
                        IntentLiteral::Point([0.0, 0.0]),
                    )
                    .with_field(
                        IntentFieldKey(key("width")),
                        IntentLiteral::Quantity {
                            value: 2.0,
                            unit: IntentUnit::Length,
                        },
                    )
                    .with_field(
                        IntentFieldKey(key("height")),
                        IntentLiteral::Quantity {
                            value: 1.0,
                            unit: IntentUnit::Length,
                        },
                    )
                    .with_field(
                        IntentFieldKey(key("role")),
                        IntentLiteral::Enum(key("profile")),
                    ),
                ),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("annotation"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Annotation,
                    key("Annotation"),
                )),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let circle = plan.aliases().node(&key("circle")).unwrap();
    let operation = plan.aliases().node(&key("operation")).unwrap();
    let annotation = plan.aliases().node(&key("annotation")).unwrap();
    session.commit_plan(plan).unwrap();

    let circle = session.graph().node(circle).unwrap();
    for index in 0..3 {
        let handle = circle
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Control,
                index,
            })
            .unwrap();
        assert_eq!(handle.kind, IntentPortKind::HandlePoint);
        assert_eq!(handle.flow, IntentIdentityFlow::OwnedLogical);
        assert!(handle.writable.is_empty());
    }
    assert_eq!(
        circle
            .reservations
            .values()
            .map(|reservation| reservation.kind)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            IntentNativeReservationKind::Point,
            IntentNativeReservationKind::Scalar,
            IntentNativeReservationKind::Curve,
        ])
    );
    let span = circle
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .unwrap();
    assert_eq!(span.flow, IntentIdentityFlow::OwnedLogical);

    for node in [operation, annotation] {
        let node = session.graph().node(node).unwrap();
        assert!(node.reservations.is_empty());
        assert!(
            node.ports
                .values()
                .all(|port| port.flow == IntentIdentityFlow::OwnedLogical)
        );
    }
}

#[test]
fn operation_output_shape_round_trips_with_stable_native_and_span_ports() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_2101)).unwrap();
    let outputs = vec![
        IntentOperationOutput::native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(2),
        IntentOperationOutput::native(IntentOperationOutputKind::Constraint),
        IntentOperationOutput::native(IntentOperationOutputKind::Dimension),
    ];
    let patch = operation_output_shape_patch(session.identity(), outputs.clone());
    let plan = session.plan_patch(patch, accepted).unwrap();
    let operation = plan.aliases().node(&key("operation")).unwrap();
    session.commit_plan(plan).unwrap();

    let node = session.graph().node(operation).unwrap();
    assert_eq!(node.operation_outputs, outputs);
    assert_eq!(
        node.ports
            .values()
            .filter(|port| matches!(
                port.selector,
                IntentPortSelector::Node {
                    role: IntentPortRole::Result,
                    ..
                }
            ))
            .count(),
        4
    );
    assert_eq!(
        node.ports
            .values()
            .filter(|port| matches!(
                port.selector,
                IntentPortSelector::Node {
                    role: IntentPortRole::Span,
                    ..
                }
            ))
            .count(),
        2
    );
    assert_eq!(
        node.reservations
            .values()
            .filter(|reservation| matches!(
                reservation.kind,
                IntentNativeReservationKind::ConstraintSource
                    | IntentNativeReservationKind::DimensionSource
            ))
            .count(),
        2
    );

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(
        restored.graph().node(operation).unwrap().operation_outputs,
        outputs
    );
}

fn operation_output_shape_patch(
    session_id: IntentSessionIdentity,
    outputs: Vec<IntentOperationOutput>,
) -> IntentPatch {
    IntentPatch::new(
        session_id,
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("source"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("source"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("axis"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("axis"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("operation"),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Operation {
                            operation: OperationKind::Mirror,
                        },
                        key("operation"),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Curve, 0),
                        alias_port("source", IntentPortRole::Curve, 0),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        alias_port("axis", IntentPortRole::Span, 0),
                    )
                    .with_operation_outputs(outputs),
                ),
                cell: None,
            },
        ],
    )
}

#[test]
fn malformed_operation_output_shapes_reject_before_identity_allocation() {
    let cases = [
        (
            "non-operation output",
            point_draft("point").with_operation_outputs(vec![IntentOperationOutput::native(
                IntentOperationOutputKind::Point,
            )]),
        ),
        (
            "zero-span curve",
            IntentNodeDraft::new(
                IntentNodeKind::Operation {
                    operation: OperationKind::Rectangle,
                },
                key("rectangle"),
            )
            .with_operation_outputs(vec![IntentOperationOutput::curve(0)]),
        ),
        (
            "span count on point",
            IntentNodeDraft::new(
                IntentNodeKind::Operation {
                    operation: OperationKind::Rectangle,
                },
                key("rectangle"),
            )
            .with_operation_outputs(vec![IntentOperationOutput {
                kind: IntentOperationOutputKind::Point,
                curve_span_count: 1,
            }]),
        ),
        (
            "edit-only output",
            IntentNodeDraft::new(
                IntentNodeKind::Operation {
                    operation: OperationKind::Split,
                },
                key("split"),
            )
            .with_operation_outputs(vec![IntentOperationOutput::native(
                IntentOperationOutputKind::Point,
            )]),
        ),
    ];
    for (label, draft) in cases {
        assert!(matches!(
            atomic_schema_rejection(label, draft),
            IntentPlanError::Graph(IntentGraphError::InvalidPortSchema { .. })
        ));
    }
}

#[test]
fn contact_constraints_and_dimensions_generate_complete_native_reservation_pairs() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8322)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point_draft("Point")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("curve"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("Curve"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("constraint"),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Constraint {
                            constraint: ConstraintKind::PointOnCurve,
                        },
                        key("Point on curve"),
                    )
                    .with_input(InputSlot::new(InputRole::Point, 0), alias_point("point"))
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        alias_port("curve", IntentPortRole::Span, 0),
                    ),
                ),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("dimension"),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Dimension {
                            dimension: DimensionKind::CurveLength,
                        },
                        key("Curve length"),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        alias_port("curve", IntentPortRole::Span, 0),
                    ),
                ),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let constraint = plan.aliases().node(&key("constraint")).unwrap();
    let dimension = plan.aliases().node(&key("dimension")).unwrap();
    session.commit_plan(plan).unwrap();

    let constraint = session.graph().node(constraint).unwrap();
    let kinds = constraint
        .reservations
        .values()
        .map(|reservation| reservation.kind)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        kinds,
        BTreeSet::from([
            IntentNativeReservationKind::Contact,
            IntentNativeReservationKind::Scalar,
            IntentNativeReservationKind::Constraint,
            IntentNativeReservationKind::ConstraintSource,
        ])
    );
    let constraint_owner = constraint
        .reservations
        .values()
        .find(|reservation| reservation.kind == IntentNativeReservationKind::Constraint)
        .unwrap();
    let constraint_source = constraint.reservations[&constraint_owner.paired_with.unwrap()];
    assert_eq!(
        constraint_source.kind,
        IntentNativeReservationKind::ConstraintSource
    );
    assert_eq!(constraint_source.paired_with, Some(constraint_owner.id));

    let dimension = session.graph().node(dimension).unwrap();
    let dimension_owner = dimension
        .reservations
        .values()
        .find(|reservation| reservation.kind == IntentNativeReservationKind::Dimension)
        .unwrap();
    let dimension_source = dimension.reservations[&dimension_owner.paired_with.unwrap()];
    assert_eq!(
        dimension_source.kind,
        IntentNativeReservationKind::DimensionSource
    );
    assert_eq!(dimension_source.paired_with, Some(dimension_owner.id));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one normalized-bootstrap fixture keeps every admitted flat object and dependency visible"
)]
fn normalized_bootstrap_is_per_object_typed_and_canonical_without_an_aggregate_baseline() {
    let mut drafts = vec![
        (
            "document",
            bootstrap_draft(BootstrapNativeKind::Document, "Document"),
        ),
        (
            "point",
            bootstrap_draft(BootstrapNativeKind::Point, "Point"),
        ),
        (
            "scalar",
            bootstrap_draft(BootstrapNativeKind::Scalar, "Scalar"),
        ),
        (
            "catalog",
            bootstrap_draft(BootstrapNativeKind::SemanticCatalog, "Semantic catalog"),
        ),
        (
            "external",
            bootstrap_draft(BootstrapNativeKind::ExternalBinding, "External binding"),
        ),
    ];
    drafts.extend([
        (
            "curve",
            bootstrap_draft(BootstrapNativeKind::Curve, "Curve")
                .with_input(
                    InputSlot::new(InputRole::Point, 0),
                    alias_port("point", IntentPortRole::Primary, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Scalar, 0),
                    alias_port("scalar", IntentPortRole::Target, 0),
                ),
        ),
        (
            "semantic-source",
            bootstrap_draft(BootstrapNativeKind::SemanticSource, "Semantic source").with_input(
                InputSlot::new(InputRole::Catalog, 0),
                alias_port("catalog", IntentPortRole::Catalog, 0),
            ),
        ),
        (
            "parameter",
            bootstrap_draft(BootstrapNativeKind::Parameter, "Parameter").with_input(
                InputSlot::new(InputRole::Scalar, 0),
                alias_port("scalar", IntentPortRole::Target, 0),
            ),
        ),
        (
            "contact",
            bootstrap_draft(BootstrapNativeKind::Contact, "Contact")
                .with_input(
                    InputSlot::new(InputRole::Curve, 0),
                    alias_port("curve", IntentPortRole::Curve, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Scalar, 0),
                    alias_port("scalar", IntentPortRole::Target, 0),
                ),
        ),
        (
            "trim-view",
            bootstrap_draft(BootstrapNativeKind::CurveTrimView, "Trim view").with_input(
                InputSlot::new(InputRole::Curve, 0),
                alias_port("curve", IntentPortRole::Curve, 0),
            ),
        ),
        (
            "geometry-role",
            bootstrap_draft(BootstrapNativeKind::GeometryRole, "Geometry role").with_input(
                InputSlot::new(InputRole::Curve, 0),
                alias_port("curve", IntentPortRole::Curve, 0),
            ),
        ),
        (
            "feature",
            bootstrap_draft(BootstrapNativeKind::ComputedFeature, "Computed feature").with_input(
                InputSlot::new(InputRole::Curve, 0),
                alias_port("curve", IntentPortRole::Curve, 0),
            ),
        ),
        (
            "constraint",
            bootstrap_draft(BootstrapNativeKind::Constraint, "Constraint")
                .with_input(
                    InputSlot::new(InputRole::Point, 0),
                    alias_port("point", IntentPortRole::Primary, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Contact, 0),
                    alias_port("contact", IntentPortRole::Contact, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Source, 0),
                    alias_port("semantic-source", IntentPortRole::Source, 0),
                ),
        ),
        (
            "dimension",
            bootstrap_draft(BootstrapNativeKind::Dimension, "Dimension")
                .with_input(
                    InputSlot::new(InputRole::Curve, 0),
                    alias_port("curve", IntentPortRole::Curve, 0),
                )
                .with_input(
                    InputSlot::new(InputRole::Scalar, 0),
                    alias_port("scalar", IntentPortRole::Target, 0),
                ),
        ),
        (
            "parameter-binding",
            bootstrap_draft(BootstrapNativeKind::ParameterBinding, "Parameter binding").with_input(
                InputSlot::new(InputRole::Parameter, 0),
                alias_port("parameter", IntentPortRole::Parameter, 0),
            ),
        ),
        (
            "parameter-output",
            bootstrap_draft(BootstrapNativeKind::ParameterOutput, "Parameter output").with_input(
                InputSlot::new(InputRole::Parameter, 0),
                alias_port("parameter", IntentPortRole::Parameter, 0),
            ),
        ),
        (
            "annotation",
            bootstrap_draft(
                BootstrapNativeKind::AnnotationPlacement,
                "Annotation placement",
            )
            .with_input(
                InputSlot::new(InputRole::Dimension, 0),
                alias_port("dimension", IntentPortRole::Dimension, 0),
            ),
        ),
    ]);
    assert_eq!(drafts.len(), BootstrapNativeKind::ALL.len());

    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8323)).unwrap();
    let operations = drafts
        .into_iter()
        .map(|(alias, draft)| IntentPatchOperation::CreateNode {
            alias: key(alias),
            draft: Box::new(draft),
            cell: None,
        })
        .collect();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let catalog = plan.aliases().node(&key("catalog")).unwrap();
    let semantic_source = plan.aliases().node(&key("semantic-source")).unwrap();
    session.commit_plan(plan).unwrap();

    let kinds = session
        .graph()
        .nodes()
        .values()
        .map(|node| match &node.kind {
            IntentNodeKind::Bootstrap { object } => object.kind,
            _ => panic!("bootstrap transaction emitted a non-bootstrap declaration"),
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(kinds, BootstrapNativeKind::ALL.into_iter().collect());
    assert_eq!(
        session.graph().nodes().len(),
        BootstrapNativeKind::ALL.len()
    );

    let catalog_port = session
        .graph()
        .node(catalog)
        .unwrap()
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Catalog,
            index: 0,
        })
        .unwrap();
    assert_eq!(catalog_port.kind, IntentPortKind::SemanticCatalog);
    assert!(matches!(
        catalog_port.flow,
        IntentIdentityFlow::Created { .. }
    ));
    assert_eq!(
        session.graph().node(semantic_source).unwrap().inputs
            [&InputSlot::new(InputRole::Catalog, 0)],
        catalog_port.as_ref(catalog)
    );
    let semantic_reservations = session
        .graph()
        .nodes()
        .values()
        .flat_map(|node| node.reservations.values().map(|value| value.kind))
        .filter(|kind| {
            matches!(
                kind,
                IntentNativeReservationKind::SemanticCatalog
                    | IntentNativeReservationKind::SemanticSource
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        semantic_reservations,
        BTreeSet::from([
            IntentNativeReservationKind::SemanticCatalog,
            IntentNativeReservationKind::SemanticSource,
        ])
    );

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(restored.identity(), session.identity());
}

#[test]
fn bootstrap_membership_uses_one_logical_document_root_without_native_aliasing() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_b001)).unwrap();
    let root = bootstrap_draft(BootstrapNativeKind::Document, "document");
    let point = bootstrap_draft(BootstrapNativeKind::Point, "point").with_input(
        InputSlot::new(InputRole::Identity, 0),
        alias_port("document", IntentPortRole::Result, 0),
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("document"),
                draft: Box::new(root),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    session.commit_initialization_plan(plan).unwrap();

    assert_eq!(session.graph().nodes().len(), 2);
    assert_eq!(session.undo_len(), 0);
    assert_eq!(session.redo_len(), 0);
    assert!(session.undo().unwrap().is_none());
    let point = session
        .graph()
        .nodes()
        .values()
        .find(|node| node.symbol.as_str() == "point")
        .unwrap();
    let source = point.inputs[&InputSlot::new(InputRole::Identity, 0)];
    assert_eq!(source.kind, IntentPortKind::Collection);
    let primary = point
        .port_by_selector(point_selector())
        .expect("bootstrap point output");
    assert!(matches!(primary.flow, IntentIdentityFlow::Created { .. }));
}

#[test]
#[allow(clippy::too_many_lines)]
fn bootstrap_point_ejection_is_an_in_place_atomic_typed_transition() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_b003)).unwrap();
    let root = bootstrap_draft(BootstrapNativeKind::Document, "document");
    let point = bootstrap_draft(BootstrapNativeKind::Point, "point").with_input(
        InputSlot::new(InputRole::Identity, 0),
        alias_port("document", IntentPortRole::Result, 0),
    );
    let alias = point_draft("dependent")
        .with_input(InputSlot::new(InputRole::Point, 0), alias_point("point"));
    let initialization = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("dependent"),
                draft: Box::new(alias),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("document"),
                draft: Box::new(root),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(initialization, accepted).unwrap();
    let aliases = plan.aliases().clone();
    session.commit_initialization_plan(plan).unwrap();
    let point = aliases.nodes[&key("point")];
    let dependent = aliases.nodes[&key("dependent")];
    let original = session.graph().node(point).unwrap().clone();
    let primary = original.port_by_selector(point_selector()).unwrap().clone();
    let IntentIdentityFlow::Created { reservation } = primary.flow else {
        panic!("bootstrap Point must own its native reservation")
    };
    let dependent_input =
        session.graph().node(dependent).unwrap().inputs[&InputSlot::new(InputRole::Point, 0)];
    let allocator = session.allocator_high_water();
    let reservations = session.reservations().clone();

    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::EjectBootstrapPoint { node: point }],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    assert_eq!(
        plan.descriptor().operation_kinds,
        [IntentPatchOperationKind::EjectBootstrapPoint]
    );
    session.commit_plan(plan).unwrap();

    let ejected = session.graph().node(point).unwrap();
    assert_eq!(ejected.id, original.id);
    assert_eq!(
        ejected.port_by_selector(point_selector()).unwrap(),
        &primary
    );
    assert_eq!(ejected.reservations, original.reservations);
    assert!(matches!(
        ejected.kind,
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint
        }
    ));
    assert_eq!(
        ejected.bootstrap_origin.as_ref(),
        match &original.kind {
            IntentNodeKind::Bootstrap { object } => Some(object),
            _ => None,
        }
    );
    assert!(ejected.inputs.is_empty());
    assert_eq!(session.allocator_high_water(), allocator);
    assert_eq!(session.reservations(), &reservations);
    assert_eq!(
        session.reservations().entries()[&reservation].state,
        IntentReservationState::Declared
    );
    assert_eq!(
        session.graph().node(dependent).unwrap().inputs[&InputSlot::new(InputRole::Point, 0)],
        dependent_input
    );
    assert_eq!(session.undo_len(), 1);

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert!(session.undo().unwrap().is_some());
    assert_eq!(session.graph().node(point).unwrap(), &original);
    assert_eq!(session.allocator_high_water(), allocator);
    assert_eq!(
        session.reservations().entries()[&reservation].state,
        IntentReservationState::Declared
    );
    assert!(session.redo().unwrap().is_some());
    assert!(
        session
            .graph()
            .node(point)
            .unwrap()
            .bootstrap_origin
            .is_some()
    );
}

#[test]
fn bootstrap_point_ejection_rejects_unsupported_and_incomplete_nodes_atomically() {
    let mut supported = IntentSession::with_id(IntentSessionId::from_raw(0x83_b004)).unwrap();
    let root = bootstrap_draft(BootstrapNativeKind::Document, "document");
    let point = bootstrap_draft(BootstrapNativeKind::Point, "point").with_input(
        InputSlot::new(InputRole::Identity, 0),
        alias_port("document", IntentPortRole::Result, 0),
    );
    let initialization = IntentPatch::new(
        supported.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("document"),
                draft: Box::new(root),
                cell: None,
            },
        ],
    );
    let plan = supported.plan_patch(initialization, accepted).unwrap();
    let aliases = plan.aliases().clone();
    supported.commit_initialization_plan(plan).unwrap();
    let before = supported.to_canonical_json().unwrap();
    let unsupported = IntentPatch::new(
        supported.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::EjectBootstrapPoint {
            node: aliases.nodes[&key("document")],
        }],
    );
    assert!(matches!(
        supported.plan_patch(unsupported, |_| panic!(
            "unsupported ejection must not evaluate"
        )),
        Err(IntentPlanError::Graph(
            IntentGraphError::UnsupportedBootstrapEjection { .. }
        ))
    ));
    assert_eq!(supported.to_canonical_json().unwrap(), before);

    let mut incomplete = IntentSession::with_id(IntentSessionId::from_raw(0x83_b005)).unwrap();
    let initialization = IntentPatch::new(
        incomplete.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(bootstrap_draft(BootstrapNativeKind::Point, "point")),
            cell: None,
        }],
    );
    let plan = incomplete.plan_patch(initialization, accepted).unwrap();
    let point = plan.aliases().nodes[&key("point")];
    incomplete.commit_initialization_plan(plan).unwrap();
    let before = incomplete.to_canonical_json().unwrap();
    let patch = IntentPatch::new(
        incomplete.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::EjectBootstrapPoint { node: point }],
    );
    assert!(matches!(
        incomplete.plan_patch(patch, |_| panic!("incomplete ejection must not evaluate")),
        Err(IntentPlanError::Graph(
            IntentGraphError::IncompleteBootstrapEjection { .. }
        ))
    ));
    assert_eq!(incomplete.to_canonical_json().unwrap(), before);
}

#[test]
fn initialization_publication_rejects_a_nonempty_session() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_b002)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("first"),
            draft: Box::new(point_draft("first")),
            cell: None,
        }],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("second"),
            draft: Box::new(point_draft("second")),
            cell: None,
        }],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    assert!(session.commit_initialization_plan(plan).is_err());
    assert_eq!(session.graph().nodes().len(), 1);
}

#[test]
fn pristine_empty_acceptance_is_history_free_exact_and_canonical() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_b003)).unwrap();
    let evidence = MaterializationEvidence::new_host_artifacts(
        session.external_inputs().identity(),
        b"accepted-empty-sketch".to_vec(),
        b"empty-ownership".to_vec(),
        b"independently-validated-empty".to_vec(),
    )
    .unwrap();
    let before = session.identity();
    let installed = session
        .install_pristine_empty_acceptance(evidence.clone())
        .unwrap();

    assert_ne!(installed, before);
    assert!(session.graph().nodes().is_empty());
    assert_eq!(session.undo_len(), 0);
    assert_eq!(session.redo_len(), 0);
    assert!(session.undo().unwrap().is_none());
    assert_eq!(session.accepted().unwrap().evidence, evidence);
    assert!(session.install_pristine_empty_acceptance(evidence).is_err());

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(restored.identity(), session.identity());
    assert!(restored.graph().nodes().is_empty());
    assert_eq!(restored.undo_len(), 0);
}

#[test]
fn pristine_empty_acceptance_rejects_wrong_inputs_and_retained_state() {
    let mut wrong_inputs = IntentSession::with_id(IntentSessionId::from_raw(0x83_b004)).unwrap();
    let other = IntentSession::with_id(IntentSessionId::from_raw(0x83_b005)).unwrap();
    let mut evidence = MaterializationEvidence::new_host_artifacts(
        other.external_inputs().identity(),
        b"empty".to_vec(),
        b"owners".to_vec(),
        b"validated".to_vec(),
    )
    .unwrap();
    // Session IDs are deliberately absent from external-input identity, so
    // authenticate a genuinely different opaque input stamp.
    evidence.external_inputs.revision = geosolve_sketch_intent::ExternalInputRevision::from_raw(1);
    assert!(
        wrong_inputs
            .install_pristine_empty_acceptance(evidence)
            .is_err()
    );

    let mut nonempty = IntentSession::with_id(IntentSessionId::from_raw(0x83_b006)).unwrap();
    let patch = IntentPatch::new(
        nonempty.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point_draft("point")),
            cell: None,
        }],
    );
    let plan = nonempty.plan_patch(patch, accepted).unwrap();
    nonempty.commit_plan(plan).unwrap();
    let evidence = MaterializationEvidence::new_host_artifacts(
        nonempty.external_inputs().identity(),
        b"empty".to_vec(),
        b"owners".to_vec(),
        b"validated".to_vec(),
    )
    .unwrap();
    assert!(
        nonempty
            .install_pristine_empty_acceptance(evidence)
            .is_err()
    );
}

#[test]
fn direct_acceptance_seams_reject_forged_evidence_atomically() {
    let mut pristine = IntentSession::with_id(IntentSessionId::from_raw(0x83_b007)).unwrap();
    let mut forged_empty = MaterializationEvidence::new_host_artifacts(
        pristine.external_inputs().identity(),
        b"empty".to_vec(),
        b"owners".to_vec(),
        b"validated".to_vec(),
    )
    .unwrap();
    forged_empty.ownership.push(0x83);
    let pristine_identity = pristine.identity();
    assert!(
        pristine
            .install_pristine_empty_acceptance(forged_empty)
            .is_err()
    );
    assert_eq!(pristine.identity(), pristine_identity);
    assert!(pristine.accepted().is_none());

    let mut current = IntentSession::with_id(IntentSessionId::from_raw(0x83_b008)).unwrap();
    let patch = IntentPatch::new(
        current.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point_draft("point")),
            cell: None,
        }],
    );
    let plan = current.plan_patch(patch, accepted).unwrap();
    current.commit_plan(plan).unwrap();
    let before = current.to_canonical_json().unwrap();
    assert!(
        current
            .refresh_current_accepted_evidence(|candidate| {
                let IntentEvaluation::Accepted { mut evidence } = accepted(candidate) else {
                    unreachable!()
                };
                evidence.host_validation.push(0x83);
                IntentEvaluation::Accepted { evidence }
            })
            .is_err()
    );
    assert_eq!(current.to_canonical_json().unwrap(), before);
}

#[test]
fn semantic_source_bootstrap_requires_its_typed_catalog_before_materialization() {
    let session = IntentSession::with_id(IntentSessionId::from_raw(0x8324)).unwrap();
    let identity = session.identity();
    let allocator = session.allocator_high_water();
    let patch = IntentPatch::new(
        identity,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("source"),
            draft: Box::new(bootstrap_draft(
                BootstrapNativeKind::SemanticSource,
                "Orphan source",
            )),
            cell: None,
        }],
    );
    assert!(matches!(
        session.plan_patch(patch, |_| panic!("missing catalog is structural")),
        Err(IntentPlanError::Graph(
            IntentGraphError::MissingRequiredInput { .. }
        ))
    ));
    assert_eq!(session.identity(), identity);
    assert_eq!(session.allocator_high_water(), allocator);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the explicit 27-recipe table is the reviewed native-storage inventory"
)]
fn every_geometry_recipe_has_the_reviewed_native_and_logical_storage_inventory() {
    // recipe, dynamic children, native points, native scalars, native curves,
    // native contacts, logical Cartesian handles.
    let expected = [
        (GeometryRecipeKind::SketchPoint, 0, 1, 0, 0, 0, 0),
        (GeometryRecipeKind::Segment, 0, 2, 0, 1, 0, 0),
        (GeometryRecipeKind::Polyline, 3, 3, 0, 1, 0, 0),
        (GeometryRecipeKind::MidpointLine, 0, 3, 0, 1, 0, 0),
        (
            GeometryRecipeKind::TwoPointAlignedRectangle,
            0,
            4,
            0,
            4,
            0,
            0,
        ),
        (
            GeometryRecipeKind::ThreePointCornerRectangle,
            0,
            4,
            0,
            4,
            0,
            0,
        ),
        (GeometryRecipeKind::CenterRectangle, 0, 5, 0, 5, 0, 0),
        (
            GeometryRecipeKind::ThreePointCenterRectangle,
            0,
            5,
            0,
            5,
            0,
            1,
        ),
        (GeometryRecipeKind::CenterRadiusCircle, 0, 1, 1, 1, 0, 1),
        (GeometryRecipeKind::TwoPointDiameterCircle, 0, 1, 1, 1, 0, 2),
        (GeometryRecipeKind::ThreePointCircle, 0, 1, 1, 1, 0, 3),
        (GeometryRecipeKind::CenterArc, 0, 1, 3, 1, 0, 3),
        (GeometryRecipeKind::ThreePointArc, 0, 1, 3, 1, 0, 3),
        (GeometryRecipeKind::TangentArc, 0, 1, 5, 1, 2, 3),
        (GeometryRecipeKind::CenterAxesEllipse, 0, 2, 1, 1, 0, 1),
        (GeometryRecipeKind::AxisEndpointsEllipse, 0, 2, 1, 1, 0, 2),
        (
            GeometryRecipeKind::CenterAxesEllipticalArc,
            0,
            2,
            3,
            1,
            0,
            3,
        ),
        (
            GeometryRecipeKind::AxisEndpointsEllipticalArc,
            0,
            2,
            3,
            1,
            0,
            4,
        ),
        (GeometryRecipeKind::QuadraticBezier, 0, 3, 0, 1, 0, 0),
        (GeometryRecipeKind::CubicBezier, 0, 4, 0, 1, 0, 0),
        (GeometryRecipeKind::RationalQuadraticConic, 0, 2, 1, 1, 0, 1),
        (GeometryRecipeKind::Parabola, 0, 2, 2, 1, 0, 0),
        (GeometryRecipeKind::Hyperbola, 0, 2, 3, 1, 0, 0),
        (GeometryRecipeKind::OpenControlBSpline, 4, 4, 0, 1, 0, 0),
        (GeometryRecipeKind::PeriodicControlBSpline, 4, 4, 0, 1, 0, 0),
        (GeometryRecipeKind::OpenControlNurbs, 4, 4, 4, 1, 0, 0),
        (GeometryRecipeKind::PeriodicControlNurbs, 4, 4, 4, 1, 0, 0),
    ];
    assert_eq!(expected.len(), GeometryRecipeKind::ALL.len());

    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8325)).unwrap();
    let operations = expected
        .iter()
        .enumerate()
        .map(|(index, (recipe, children, ..))| {
            let mut draft = IntentNodeDraft::new(
                IntentNodeKind::Geometry { recipe: *recipe },
                key(&format!("Recipe {index:02}")),
            )
            .with_dynamic_children(*children);
            if *recipe == GeometryRecipeKind::TangentArc {
                draft = draft.with_input(
                    InputSlot::new(InputRole::Span, 0),
                    alias_port("tangent-source", IntentPortRole::Span, 0),
                );
            }
            IntentPatchOperation::CreateNode {
                alias: key(&format!("recipe-{index:02}")),
                draft: Box::new(draft),
                cell: None,
            }
        })
        .chain(std::iter::once(IntentPatchOperation::CreateNode {
            alias: key("tangent-source"),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                key("Tangent source"),
            )),
            cell: None,
        }))
        .collect();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let aliases = plan.aliases().clone();
    session.commit_plan(plan).unwrap();

    for (index, (recipe, _, points, scalars, curves, contacts, handles)) in
        expected.into_iter().enumerate()
    {
        let node = session
            .graph()
            .node(aliases.node(&key(&format!("recipe-{index:02}"))).unwrap())
            .unwrap();
        assert_eq!(
            node.kind,
            IntentNodeKind::Geometry { recipe },
            "recipe {recipe:?}"
        );
        let reservation_count = |kind| {
            node.reservations
                .values()
                .filter(|reservation| reservation.kind == kind)
                .count()
        };
        assert_eq!(
            reservation_count(IntentNativeReservationKind::Point),
            points,
            "point inventory for {recipe:?}"
        );
        assert_eq!(
            reservation_count(IntentNativeReservationKind::Scalar),
            scalars,
            "scalar inventory for {recipe:?}"
        );
        assert_eq!(
            reservation_count(IntentNativeReservationKind::Curve),
            curves,
            "curve inventory for {recipe:?}"
        );
        assert_eq!(
            reservation_count(IntentNativeReservationKind::Contact),
            contacts,
            "contact inventory for {recipe:?}"
        );
        let intrinsic_relations = match recipe {
            GeometryRecipeKind::MidpointLine | GeometryRecipeKind::TangentArc => 1,
            GeometryRecipeKind::TwoPointAlignedRectangle
            | GeometryRecipeKind::ThreePointCenterRectangle => 4,
            GeometryRecipeKind::ThreePointCornerRectangle => 3,
            GeometryRecipeKind::CenterRectangle => 5,
            _ => 0,
        };
        assert_eq!(
            reservation_count(IntentNativeReservationKind::Constraint),
            intrinsic_relations,
            "intrinsic relation inventory for {recipe:?}"
        );
        assert_eq!(
            reservation_count(IntentNativeReservationKind::ConstraintSource),
            intrinsic_relations,
            "intrinsic relation-source inventory for {recipe:?}"
        );
        assert_eq!(
            node.ports
                .values()
                .filter(|port| port.kind == IntentPortKind::HandlePoint)
                .count(),
            handles,
            "logical handle inventory for {recipe:?}"
        );
        assert!(
            node.ports
                .values()
                .filter(|port| port.kind == IntentPortKind::HandlePoint)
                .all(|port| port.writable.is_empty()),
            "derived handles must not expose generic instance leaves for {recipe:?}"
        );
        assert_eq!(
            node.ports
                .values()
                .map(|port| port.selector)
                .collect::<BTreeSet<_>>()
                .len(),
            node.ports.len(),
            "port selectors must be unique for {recipe:?}"
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle test keeps every compound recipe's field-dependent topology together"
)]
fn compound_recipe_fields_generate_exact_topology_without_hidden_native_points() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8325_0001)).unwrap();
    let rectangle = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::ThreePointCenterRectangle,
        },
        key("regularized rectangle"),
    )
    .with_field(
        IntentFieldKey(key("regularized")),
        IntentLiteral::Boolean(true),
    )
    .with_field(
        IntentFieldKey(key("side_midpoint")),
        IntentLiteral::Point([0.0, 1.0]),
    );
    let closed_polyline = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        key("closed polyline"),
    )
    .with_dynamic_children(4)
    .with_field(IntentFieldKey(key("closed")), IntentLiteral::Boolean(true));
    let quadratic_nurbs = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::OpenControlNurbs,
        },
        key("quadratic NURBS"),
    )
    .with_dynamic_children(5)
    .with_field(IntentFieldKey(key("degree")), IntentLiteral::Natural(2))
    .with_field(
        IntentFieldKey(key("gauge_index")),
        IntentLiteral::Natural(1),
    );
    let periodic_nurbs = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::PeriodicControlNurbs,
        },
        key("periodic NURBS"),
    )
    .with_dynamic_children(5)
    .with_field(IntentFieldKey(key("degree")), IntentLiteral::Natural(2));
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        [
            ("rectangle", rectangle),
            ("polyline", closed_polyline),
            ("open-nurbs", quadratic_nurbs),
            ("periodic-nurbs", periodic_nurbs),
        ]
        .into_iter()
        .map(|(alias, draft)| IntentPatchOperation::CreateNode {
            alias: key(alias),
            draft: Box::new(draft),
            cell: None,
        })
        .collect(),
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let aliases = plan.aliases().clone();
    session.commit_plan(plan).unwrap();

    let rectangle = session
        .graph()
        .node(aliases.node(&key("rectangle")).unwrap())
        .unwrap();
    let reservation_count = |kind| {
        rectangle
            .reservations
            .values()
            .filter(|reservation| reservation.kind == kind)
            .count()
    };
    assert_eq!(reservation_count(IntentNativeReservationKind::Point), 5);
    assert_eq!(
        reservation_count(IntentNativeReservationKind::Constraint),
        5
    );
    assert_eq!(
        reservation_count(IntentNativeReservationKind::ConstraintSource),
        5
    );
    assert_eq!(
        rectangle
            .ports
            .values()
            .filter(|port| port.kind == IntentPortKind::HandlePoint)
            .count(),
        1,
        "the side midpoint remains logical-only"
    );
    assert!(rectangle.reservations.values().all(|reservation| {
        !matches!(reservation.kind, IntentNativeReservationKind::Constraint)
            || reservation.paired_with.is_some()
    }));

    let span_count = |alias: &str| {
        session
            .graph()
            .node(aliases.node(&key(alias)).unwrap())
            .unwrap()
            .ports
            .values()
            .filter(|port| port.kind == IntentPortKind::CurveSpan)
            .count()
    };
    assert_eq!(span_count("polyline"), 4);
    assert_eq!(span_count("open-nurbs"), 3);
    assert_eq!(span_count("periodic-nurbs"), 5);
}

#[test]
fn contact_bearing_relations_retain_complete_explicit_contact_cell_fields() {
    let contact_cases = [
        (ConstraintKind::PointOnCurve, 1_usize),
        (ConstraintKind::LineCurveTangency, 1),
        (ConstraintKind::CurveDirection, 1),
        (ConstraintKind::LineCircleTangency, 2),
        (ConstraintKind::CircleArcTangency, 2),
        (ConstraintKind::CurveCurveContact, 2),
        (ConstraintKind::CurveCurveTangency, 2),
        (ConstraintKind::EqualCurvature, 2),
        (ConstraintKind::EndpointContinuity, 2),
        (ConstraintKind::LineLineFillet, 2),
        (ConstraintKind::CurveCurveFillet, 2),
    ];
    for (constraint, contact_count) in contact_cases {
        let schema = IntentNodeKind::Constraint { constraint }.schema(0);
        let expected_prefixes = if contact_count == 1 {
            vec!["contact"]
        } else {
            vec!["first_contact", "second_contact"]
        };
        for prefix in expected_prefixes {
            for suffix in [
                "parameter",
                "winding",
                "support",
                "range_lower",
                "range_upper",
                "neighborhood",
                "neighborhood_lower",
                "neighborhood_upper",
                "orientation",
            ] {
                let name = format!("{prefix}_{suffix}");
                assert!(
                    schema
                        .fields
                        .iter()
                        .any(|field| field.field.0.as_str() == name),
                    "{constraint:?} omits {name}"
                );
            }
        }
        let node = IntentNodeKind::Constraint { constraint };
        let expected_reservations = 2 * contact_count + 2;
        let session =
            IntentSession::with_id(IntentSessionId::from_raw(0x8325_1000 + constraint as u128))
                .unwrap();
        let case = DeclarationSchemaCase {
            label: format!("contact.{constraint:?}"),
            kind: node,
            dynamic_children: 0,
        };
        let draft = minimal_schema_draft(&case, "contact-cell");
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("contact-cell"),
                draft: Box::new(draft),
                cell: None,
            }],
        );
        let result = session.plan_patch(patch, accepted);
        assert!(
            matches!(
                result,
                Err(IntentPlanError::Graph(IntentGraphError::UnknownNode(_)))
            ),
            "{constraint:?} did not reach dependency resolution: {result:?}"
        );
        assert_eq!(
            node_port_reservation_count_for_test(constraint, contact_count),
            expected_reservations
        );
    }
}

fn node_port_reservation_count_for_test(constraint: ConstraintKind, contact_count: usize) -> usize {
    let mut session =
        IntentSession::with_id(IntentSessionId::from_raw(0x8325_2000 + constraint as u128))
            .unwrap();
    let mut operations = Vec::new();
    for index in 0..2 {
        operations.push(IntentPatchOperation::CreateNode {
            alias: key(&format!("point-{index}")),
            draft: Box::new(point_draft(&format!("point {index}"))),
            cell: None,
        });
        operations.push(IntentPatchOperation::CreateNode {
            alias: key(&format!("curve-{index}")),
            draft: Box::new(IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                key(&format!("curve {index}")),
            )),
            cell: None,
        });
    }
    let case = DeclarationSchemaCase {
        label: format!("contact.{constraint:?}"),
        kind: IntentNodeKind::Constraint { constraint },
        dynamic_children: 0,
    };
    let schema = case.kind.schema(0);
    let mut draft = IntentNodeDraft::new(case.kind, key("contact relation"));
    for cardinality in schema.inputs {
        for index in 0..cardinality.minimum {
            let source = match cardinality.role {
                InputRole::Point => alias_port(
                    &format!("point-{}", index.min(1)),
                    IntentPortRole::Primary,
                    0,
                ),
                InputRole::Curve => {
                    alias_port(&format!("curve-{}", index.min(1)), IntentPortRole::Curve, 0)
                }
                InputRole::Span => {
                    alias_port(&format!("curve-{}", index.min(1)), IntentPortRole::Span, 0)
                }
                _ => return 2 * contact_count + 2,
            };
            draft = draft.with_input(InputSlot::new(cardinality.role, index), source);
        }
    }
    for field in schema.fields.iter().filter(|field| field.required) {
        draft = draft.with_field(field.field.clone(), literal_for_schema(field.literal));
    }
    operations.push(IntentPatchOperation::CreateNode {
        alias: key("relation"),
        draft: Box::new(draft),
        cell: None,
    });
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let node = plan.aliases().node(&key("relation")).unwrap();
    session.commit_plan(plan).unwrap();
    session.graph().node(node).unwrap().reservations.len()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle fixture proves both successful alias rebind and atomic cycle rejection"
)]
fn rebind_updates_schema_alias_flow_and_rejects_a_new_identity_cycle_atomically() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8326)).unwrap();
    let alias_draft = |name: &str, source: &str| {
        IntentNodeDraft::new(
            IntentNodeKind::Identity {
                transition: IdentityTransitionKind::Alias,
                port_kind: IntentPortKind::Point,
            },
            key(name),
        )
        .with_input(InputSlot::new(InputRole::Identity, 0), alias_point(source))
    };
    let segment = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("Segment"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        alias_port("left-alias", IntentPortRole::Result, 0),
    );
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("left"),
                draft: Box::new(point_draft("Left")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("right"),
                draft: Box::new(point_draft("Right")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("left-alias"),
                draft: Box::new(alias_draft("Left alias", "left")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("right-alias"),
                draft: Box::new(alias_draft("Right alias", "right")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("segment"),
                draft: Box::new(segment),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let left_alias = plan.aliases().node(&key("left-alias")).unwrap();
    let right_alias = plan.aliases().node(&key("right-alias")).unwrap();
    let right_alias_port = plan
        .aliases()
        .port(&key("right-alias"), identity_result_selector())
        .unwrap();
    let segment = plan.aliases().node(&key("segment")).unwrap();
    let segment_start = plan
        .aliases()
        .port(
            &key("segment"),
            IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            },
        )
        .unwrap();
    session.commit_plan(plan).unwrap();

    let reservations_before = session.graph().node(segment).unwrap().reservations.clone();
    let rebind = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::RebindInput {
            node: segment,
            slot: InputSlot::new(InputRole::Point, 0),
            source: PatchPortRef::Stable {
                port: right_alias_port,
            },
        }],
    );
    let plan = session.plan_patch(rebind, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    assert_eq!(
        session
            .graph()
            .node(segment)
            .unwrap()
            .port(segment_start.port)
            .unwrap()
            .flow,
        IntentIdentityFlow::Aliased {
            source: right_alias_port
        }
    );
    assert_eq!(
        session.graph().node(segment).unwrap().reservations,
        reservations_before,
        "rebind must neither orphan nor allocate native identity"
    );

    let before_cycle = session.identity();
    let allocator = session.allocator_high_water();
    let left_alias_port = session
        .graph()
        .node(left_alias)
        .unwrap()
        .port_by_selector(identity_result_selector())
        .unwrap()
        .as_ref(left_alias);
    let cycle = IntentPatch::new(
        before_cycle,
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::RebindInput {
                node: left_alias,
                slot: InputSlot::new(InputRole::Identity, 0),
                source: PatchPortRef::Stable {
                    port: right_alias_port,
                },
            },
            IntentPatchOperation::RebindInput {
                node: right_alias,
                slot: InputSlot::new(InputRole::Identity, 0),
                source: PatchPortRef::Stable {
                    port: left_alias_port,
                },
            },
        ],
    );
    assert!(matches!(
        session.plan_patch(cycle, |_| panic!("dependency cycle is structural")),
        Err(IntentPlanError::Graph(
            IntentGraphError::DependencyCycle { .. }
        ))
    ));
    assert_eq!(session.identity(), before_cycle);
    assert_eq!(session.allocator_high_water(), allocator);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused lifecycle keeps exact ledger metadata and every disposition transition together"
)]
fn durable_reservation_ledger_tracks_pairs_suppression_delete_undo_redo_and_reload() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8330)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("curve"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment,
                    },
                    key("ledger.curve"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("constraint"),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Constraint {
                            constraint: ConstraintKind::Horizontal,
                        },
                        key("constraint.horizontal"),
                    )
                    .with_input(
                        InputSlot::new(InputRole::Span, 0),
                        alias_port("curve", IntentPortRole::Span, 0),
                    ),
                ),
                cell: None,
            },
        ],
    );
    let plan = session
        .plan_patch(patch, |candidate| {
            assert_eq!(
                candidate.semantic_identity().reservations,
                candidate.reservations().identity()
            );
            assert!(candidate.reservations().entries().values().all(|record| {
                record.state == IntentReservationState::Declared
                    && candidate.graph().node(record.owner_node).is_some()
            }));
            accepted(candidate)
        })
        .unwrap();
    let node = plan.aliases().node(&key("constraint")).unwrap();
    session.commit_plan(plan).unwrap();

    let node_reservations = session.graph().node(node).unwrap().reservations.clone();
    assert!(!node_reservations.is_empty());
    for reservation in node_reservations.values() {
        let record = &session.reservations().entries()[&reservation.id];
        assert_eq!(record.kind, reservation.kind);
        assert_eq!(record.owner_node, node);
        assert_eq!(record.paired_with, reservation.paired_with);
        assert_eq!(record.state, IntentReservationState::Declared);
        let owning_port = session
            .graph()
            .node(node)
            .unwrap()
            .port(record.owner_port)
            .unwrap();
        assert_eq!(
            owning_port.flow,
            IntentIdentityFlow::Created {
                reservation: reservation.id
            }
        );
    }
    let constraint_reservation = node_reservations
        .values()
        .find(|reservation| reservation.kind == IntentNativeReservationKind::Constraint)
        .unwrap();
    let source = &session.reservations().entries()[&constraint_reservation.paired_with.unwrap()];
    assert_eq!(source.kind, IntentNativeReservationKind::ConstraintSource);
    assert_eq!(source.paired_with, Some(constraint_reservation.id));

    let suppress = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::SetSuppressed {
            node,
            suppressed: true,
        }],
    );
    let before_suppress_revision = session.reservations().revision();
    let plan = session.plan_patch(suppress, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    assert_eq!(
        session.reservations().revision().raw(),
        before_suppress_revision.raw() + 1
    );
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Suppressed
    }));

    session.undo().unwrap().unwrap();
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Declared
    }));
    session.redo().unwrap().unwrap();
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Suppressed
    }));

    let unsuppress = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::SetSuppressed {
            node,
            suppressed: false,
        }],
    );
    let plan = session.plan_patch(unsuppress, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    let delete = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::DeleteNode {
            node,
            policy: DeletePolicy::RejectDependents,
        }],
    );
    let plan = session.plan_patch(delete, accepted).unwrap();
    session.commit_plan(plan).unwrap();
    assert!(session.graph().node(node).is_none());
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Tombstoned
    }));

    session.undo().unwrap().unwrap();
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Declared
    }));
    session.redo().unwrap().unwrap();
    assert!(node_reservations.keys().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Tombstoned
    }));

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(restored.reservations(), session.reservations());
    assert_eq!(restored.semantic_identity(), session.semantic_identity());
}

#[test]
fn retained_failure_and_divergent_history_preserve_tombstones_and_accepted_authority() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8331)).unwrap();
    let (_, accepted_point) = create_point(&mut session, "accepted.point");
    let accepted_before = session.accepted().unwrap().clone();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::CreateNode {
            alias: key("failed"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Dimension {
                        dimension: DimensionKind::PointDistance,
                    },
                    key("failed.dimension"),
                )
                .with_input(
                    InputSlot::new(InputRole::Point, 0),
                    PatchPortRef::Stable {
                        port: accepted_point,
                    },
                )
                .with_input(
                    InputSlot::new(InputRole::Point, 1),
                    PatchPortRef::Stable {
                        port: accepted_point,
                    },
                ),
            ),
            cell: None,
        }],
    );
    let plan = session
        .plan_patch(patch, |candidate| IntentEvaluation::Failed {
            failure: IntentEvaluationFailure {
                kind: IntentEvaluationFailureKind::SolverRejected,
                failed_nodes: candidate.diff().created_nodes.clone(),
                diagnostic: key("incompatible-explicit-intent"),
            },
        })
        .unwrap();
    let failed = plan.aliases().node(&key("failed")).unwrap();
    session.commit_plan(plan).unwrap();
    let failed_reservations = session
        .graph()
        .node(failed)
        .unwrap()
        .reservations
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(failed_reservations.iter().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Declared
    }));
    assert_eq!(session.accepted(), Some(&accepted_before));
    assert!(failed_reservations.iter().all(|reservation| {
        !accepted_before
            .reservations
            .entries()
            .contains_key(reservation)
    }));

    session.undo().unwrap().unwrap();
    assert!(failed_reservations.iter().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Tombstoned
    }));
    assert_eq!(
        session.accepted().unwrap().target,
        session.semantic_identity()
    );
    assert_eq!(
        session.accepted().unwrap().reservations,
        *session.reservations()
    );

    let allocator_before_divergence = session.allocator_high_water();
    let (replacement, _) = create_point(&mut session, "replacement.point");
    assert_eq!(session.redo_len(), 0);
    assert!(replacement.raw() >= allocator_before_divergence.next_node.raw());
    assert!(failed_reservations.iter().all(|reservation| {
        session.reservations().entries()[reservation].state == IntentReservationState::Tombstoned
            && reservation.raw() < session.allocator_high_water().next_reservation.raw()
    }));
}

#[test]
fn developer_symbols_are_unique_and_organization_edits_are_nonsemantic() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8332)).unwrap();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("a"),
                draft: Box::new(point_draft("point.alpha").with_display_name(key("Point"))),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("b"),
                draft: Box::new(point_draft("point.beta").with_display_name(key("Point"))),
                cell: None,
            },
        ],
    );
    let plan = session.plan_patch(patch, accepted).unwrap();
    let alpha = plan.aliases().node(&key("a")).unwrap();
    session.commit_plan(plan).unwrap();
    assert_eq!(
        session
            .graph()
            .node_by_symbol(&key("point.alpha"))
            .unwrap()
            .id,
        alpha
    );
    assert!(session.graph().node_by_symbol(&key("Point")).is_none());

    let semantic = session.semantic_identity();
    let accepted_authority = session.accepted().unwrap().clone();
    let create_cell = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateCell {
            alias: key("group"),
            name: key("Group"),
            before: None,
        }],
    );
    let plan = session
        .plan_patch(create_cell, |_| panic!("organization must not materialize"))
        .unwrap();
    let group = plan.aliases().cell(&key("group")).unwrap();
    session.commit_plan(plan).unwrap();
    let organize = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::RenameNode {
                node: alpha,
                name: key("Renamed display point"),
            },
            IntentPatchOperation::MoveDeclaration {
                node: alpha,
                cell: CellTarget::Stable { cell: group },
                before: None,
            },
        ],
    );
    let plan = session
        .plan_patch(organize, |_| panic!("organization must not materialize"))
        .unwrap();
    session.commit_plan(plan).unwrap();
    assert_eq!(session.semantic_identity(), semantic);
    assert_eq!(session.accepted(), Some(&accepted_authority));
    assert_eq!(
        session
            .graph()
            .node_by_symbol(&key("point.alpha"))
            .unwrap()
            .id,
        alpha
    );

    let duplicate_session = IntentSession::with_id(IntentSessionId::from_raw(0x8333)).unwrap();
    let before = duplicate_session.identity();
    let allocator = duplicate_session.allocator_high_water();
    let duplicate = IntentPatch::new(
        before,
        IntentPatchPolicy::RequireAccepted,
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("left"),
                draft: Box::new(point_draft("duplicate.symbol").with_display_name(key("Left"))),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("right"),
                draft: Box::new(point_draft("duplicate.symbol").with_display_name(key("Right"))),
                cell: None,
            },
        ],
    );
    assert!(matches!(
        duplicate_session.plan_patch(duplicate, |_| panic!("duplicate symbols are structural")),
        Err(IntentPlanError::Graph(IntentGraphError::DuplicateSymbol(symbol)))
            if symbol == key("duplicate.symbol")
    ));
    assert_eq!(duplicate_session.identity(), before);
    assert_eq!(duplicate_session.allocator_high_water(), allocator);
}

#[test]
fn logical_handles_reject_generic_instance_storage_before_materialization() {
    let session = IntentSession::with_id(IntentSessionId::from_raw(0x8334)).unwrap();
    let before = session.identity();
    let allocator = session.allocator_high_water();
    let handle = IntentPortSelector::Node {
        role: IntentPortRole::Control,
        index: 0,
    };
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::ThreePointCircle,
        },
        key("circle.three-point"),
    )
    .with_instance_leaf(
        handle,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 1.0,
            unit: IntentUnit::Length,
        },
    );
    let patch = IntentPatch::new(
        before,
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("circle"),
            draft: Box::new(draft),
            cell: None,
        }],
    );
    assert!(matches!(
        session.plan_patch(patch, |_| panic!("handle state is structurally invalid")),
        Err(IntentPlanError::Graph(
            IntentGraphError::UnknownWritableLeaf { .. }
        ))
    ));
    assert_eq!(session.identity(), before);
    assert_eq!(session.allocator_high_water(), allocator);
}

#[test]
fn deterministic_history_projection_moves_descriptors_through_undo_redo() {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8335)).unwrap();
    let (point, _) = create_point(&mut session, "history.point");
    let rename = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::RenameNode {
            node: point,
            name: key("History point renamed"),
        }],
    );
    let plan = session
        .plan_patch(rename, |_| panic!("organization must not materialize"))
        .unwrap();
    assert_eq!(
        plan.descriptor().disposition,
        IntentPlanDisposition::OrganizationOnly
    );
    assert_eq!(
        plan.descriptor().operation_kinds,
        vec![IntentPatchOperationKind::RenameNode]
    );
    session.commit_plan(plan).unwrap();

    let projection = session.history_projection();
    assert_eq!(projection.applied.len(), 2);
    assert!(projection.redoable.is_empty());
    assert_eq!(
        projection.applied[0].operation_kinds,
        vec![IntentPatchOperationKind::CreateNode]
    );
    assert_eq!(
        projection.applied[1].affected_nodes,
        BTreeSet::from([point])
    );
    let canonical = session.to_canonical_json().unwrap();
    assert!(!canonical.contains("timestamp"));
    assert!(!canonical.contains("wall_clock"));
    assert!(!canonical.contains("created_at"));
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.history_projection(), projection);

    session.undo().unwrap().unwrap();
    let undone = session.history_projection();
    assert_eq!(undone.applied.len(), 1);
    assert_eq!(undone.redoable.len(), 1);
    assert_eq!(
        undone.redoable[0].operation_kinds,
        vec![IntentPatchOperationKind::RenameNode]
    );
    session.redo().unwrap().unwrap();
    assert_eq!(session.history_projection(), projection);
}

#[test]
#[allow(clippy::too_many_lines)]
fn hostile_stable_rebind_references_fail_closed_without_materialization_or_state_change() {
    #[derive(Clone, Copy)]
    enum ExpectedError {
        UnknownNode(NodeId),
        UnknownPort { node: NodeId, port: PortId },
        KindMismatch,
    }

    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8336)).unwrap();
    let (source_node, source_port) = create_point(&mut session, "stable.source");
    let alias_patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("stable-alias"),
            draft: Box::new(
                IntentNodeDraft::new(
                    IntentNodeKind::Identity {
                        transition: IdentityTransitionKind::Alias,
                        port_kind: IntentPortKind::Point,
                    },
                    key("stable.alias"),
                )
                .with_input(
                    InputSlot::new(InputRole::Identity, 0),
                    PatchPortRef::Stable { port: source_port },
                ),
            ),
            cell: None,
        }],
    );
    let plan = session.plan_patch(alias_patch, accepted).unwrap();
    let alias_node = plan.aliases().node(&key("stable-alias")).unwrap();
    session.commit_plan(plan).unwrap();

    let unknown_node = NodeId::from_raw(u64::MAX);
    let unknown_port = PortId::from_raw(u64::MAX);
    let cases = [
        (
            "unknown node",
            IntentPortRef {
                node: unknown_node,
                ..source_port
            },
            ExpectedError::UnknownNode(unknown_node),
        ),
        (
            "unknown port on a live node",
            IntentPortRef {
                port: unknown_port,
                ..source_port
            },
            ExpectedError::UnknownPort {
                node: source_node,
                port: unknown_port,
            },
        ),
        (
            "caller-stamped kind mismatch",
            IntentPortRef {
                kind: IntentPortKind::Scalar,
                ..source_port
            },
            ExpectedError::KindMismatch,
        ),
    ];

    let identity = session.identity();
    let canonical = session.to_canonical_json().unwrap();
    let allocator = session.allocator_high_water();
    for (label, source, expected) in cases {
        let evaluated = Cell::new(false);
        let patch = IntentPatch::new(
            identity,
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::RebindInput {
                node: alias_node,
                slot: InputSlot::new(InputRole::Identity, 0),
                source: PatchPortRef::Stable { port: source },
            }],
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.plan_patch(patch, |candidate| {
                evaluated.set(true);
                accepted(candidate)
            })
        }));
        let error = result
            .unwrap_or_else(|_| panic!("{label} panicked instead of returning a typed error"))
            .expect_err("hostile reference unexpectedly planned");
        assert!(!evaluated.get(), "{label} reached the materializer");
        match (expected, error) {
            (
                ExpectedError::UnknownNode(expected),
                IntentPlanError::Graph(IntentGraphError::UnknownNode(actual)),
            ) => assert_eq!(actual, expected, "{label}"),
            (
                ExpectedError::UnknownPort {
                    node: expected_node,
                    port: expected_port,
                },
                IntentPlanError::Graph(IntentGraphError::UnknownPort {
                    node: actual_node,
                    port: actual_port,
                }),
            ) => {
                assert_eq!(actual_node, expected_node, "{label}");
                assert_eq!(actual_port, expected_port, "{label}");
            }
            (
                ExpectedError::KindMismatch,
                IntentPlanError::Graph(IntentGraphError::InputKindMismatch {
                    expected: IntentPortKind::Point,
                    actual: IntentPortKind::Scalar,
                    ..
                }),
            ) => {}
            (_, actual) => panic!("{label} returned unexpected error: {actual:?}"),
        }
        assert_eq!(session.identity(), identity, "{label} changed identity");
        assert_eq!(
            session.to_canonical_json().unwrap(),
            canonical,
            "{label} changed canonical state"
        );
        assert_eq!(
            session.allocator_high_water(),
            allocator,
            "{label} changed allocator high-water"
        );
    }
}
