// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::Cell;
use std::collections::BTreeSet;

use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, CellTarget, ComputedFeatureKind, ConstraintKind,
    DeletePolicy, DimensionKind, ExternalIntentKind, GeometryRecipeKind, IdentityTransitionKind,
    InputRole, InputSlot, IntentBootstrapObject, IntentEvaluation, IntentEvaluationFailure,
    IntentEvaluationFailureKind, IntentFieldKey, IntentGraphError, IntentIdentityFlow, IntentKey,
    IntentLiteral, IntentLiteralSchema, IntentNativeReservationKind, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchOperationKind, IntentPatchPolicy,
    IntentPlanDisposition, IntentPlanError, IntentPortKind, IntentPortRef, IntentPortRole,
    IntentPortSelector, IntentReservationState, IntentSession, IntentSessionId, IntentUnit,
    LeafField, LeafRef, MaterializationEvidence, NodeId, OperationKind, ParameterIntentKind,
    PatchPortRef, PortId,
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
fn closed_geometry_catalog_has_all_twenty_five_recipes() {
    assert_eq!(GeometryRecipeKind::ALL.len(), 25);
    assert_eq!(
        GeometryRecipeKind::ALL
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        25
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
    assert_eq!(GeometryRecipeKind::ALL.len(), 25);
    assert_eq!(ConstraintKind::ALL.len(), 35);
    assert_eq!(DimensionKind::ALL.len(), 8);
    assert_eq!(OperationKind::ALL.len(), 12);
    assert_eq!(ComputedFeatureKind::ALL.len(), 1);
    assert_eq!(AggregateKind::ALL.len(), 2);
    assert_eq!(ParameterIntentKind::ALL.len(), 3);
    assert_eq!(ExternalIntentKind::ALL.len(), 2);
    assert_eq!(BootstrapNativeKind::ALL.len(), 17);

    let cases = declaration_schema_cases();
    assert_eq!(cases.len(), 109);
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
    reason = "the exhaustive malformed-operand matrix is intentionally table driven"
)]
fn every_closed_declaration_rejects_malformed_operands_and_children_atomically() {
    let cases = declaration_schema_cases();
    let mut missing_operands = 0_usize;
    let mut choice_underflows = 0_usize;
    let mut choice_overflows = 0_usize;
    let mut contiguous_holes = 0_usize;
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
            assert!(
                matches!(
                    error,
                    IntentPlanError::Graph(IntentGraphError::MissingRequiredInput { .. })
                ),
                "{} non-contiguous {:?} input returned {error:?}",
                case.label,
                cardinality.role
            );
            contiguous_holes += 1;
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
    assert!(contiguous_holes > 0);
    assert_eq!(child_underflows, 5);
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
    reason = "the explicit 25-recipe table is the reviewed native-storage inventory"
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
                "domain",
                "domain_lower",
                "domain_upper",
                "domain_period",
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
