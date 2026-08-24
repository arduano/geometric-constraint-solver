// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentGraphNodeKind, IntentInspectorEditTarget,
    IntentInspectorEditValue, IntentInspectorField, IntentNativeBinding, IntentRpcOutcome,
    IntentRpcRequest, IntentRpcSession, IntentRpcSuccess, MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES,
    MAX_INTENT_RPC_REQUEST_BYTES, ProjectionalEditorSession, ProjectionalIntentCoordinator,
    apply_intent_rpc_json_to_editor, apply_intent_rpc_to_editor,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_intent::{
    BootstrapNativeKind, CellTarget, GeometryRecipeKind, IntentBootstrapObject, IntentFieldKey,
    IntentIdentityFlow, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentOperationOutput, IntentOperationOutputKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit, LeafField,
    OperationKind, intent_content_digest,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

fn rpc() -> IntentRpcSession {
    IntentRpcSession::new(
        ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8300_4001),
            ColdIntentMaterializer::with_default_policy(
                DocumentId(PersistentId::from_u128(0x8300_4001_u128 << 32)),
                1.0,
            )
            .unwrap(),
        )
        .unwrap(),
    )
}

fn editor() -> ProjectionalEditorSession {
    ProjectionalEditorSession::new(
        ProjectionalIntentCoordinator::empty(
            IntentSessionId::from_raw(0x8300_4002),
            ColdIntentMaterializer::with_default_policy(
                DocumentId(PersistentId::from_u128(0x8300_4002_u128 << 32)),
                1.0,
            )
            .unwrap(),
        )
        .unwrap(),
    )
}

fn point() -> IntentNodeDraft {
    named_point("rpc.point", [1.0, 2.0])
}

fn named_point(symbol: &str, position: [f64; 2]) -> IntentNodeDraft {
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(symbol),
    )
    .with_instance_leaf(
        selector,
        LeafField::X,
        IntentLiteral::Quantity {
            value: position[0],
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        selector,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: position[1],
            unit: IntentUnit::Length,
        },
    )
}

fn invalid_segment() -> IntentNodeDraft {
    let start = IntentPortSelector::Node {
        role: IntentPortRole::Start,
        index: 0,
    };
    let end = IntentPortSelector::Node {
        role: IntentPortRole::End,
        index: 0,
    };
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("rpc.invalid.segment"),
    )
    .with_instance_leaf(
        start,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 0.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        start,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: 0.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        end,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 1.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        end,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: 0.0,
            unit: IntentUnit::Length,
        },
    )
    .with_field(
        geosolve_sketch_intent::IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point([f64::MAX, f64::MAX]),
    )
}

#[test]
fn rpc_graph_snapshot_exposes_actual_stable_output_ids_and_current_leaves() {
    let mut rpc = rpc();
    let response = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            rpc.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point()),
                cell: None,
            }],
        )),
    });
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("valid point patch must return a bounded receipt")
    };
    assert_eq!(receipt.identity, rpc.coordinator().intent().identity());
    let node = receipt
        .aliases
        .node(&key("point"))
        .expect("stable node alias");
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    let port = receipt
        .aliases
        .port(&key("point"), selector)
        .expect("stable point output alias");
    let snapshot = rpc.snapshot();
    let declaration = snapshot.graph.cells[0]
        .declarations
        .iter()
        .find(|declaration| declaration.node == node)
        .expect("queryable declaration");
    assert_eq!(declaration.symbol, key("rpc.point"));
    assert_eq!(declaration.name, key("rpc.point"));
    assert!(!declaration.suppressed);
    assert!(declaration.inputs.is_empty());
    assert!(declaration.definition_fields.is_empty());
    assert!(declaration.dependencies.is_empty());
    let output = declaration
        .descriptor
        .outputs
        .iter()
        .find(|output| output.port == port)
        .expect("actual stable output ID");
    assert_eq!(output.selector, selector);
    assert_eq!(output.writable, [LeafField::X, LeafField::Y]);
    assert!(matches!(output.flow, IntentIdentityFlow::Created { .. }));
    assert_eq!(declaration.instance_leaves.len(), 2);
    assert!(
        declaration
            .instance_leaves
            .iter()
            .all(|entry| entry.leaf.node == node && entry.leaf.port == port.port)
    );

    let encoded = serde_json::to_string(&snapshot.graph).expect("graph query serializes");
    let decoded = serde_json::from_str(&encoded).expect("graph query round trips");
    assert_eq!(snapshot.graph, decoded);
}

#[test]
#[allow(clippy::too_many_lines)]
fn rpc_graph_snapshot_preserves_child_topology_and_operation_output_spans() {
    let mut operation_outputs = Vec::new();
    operation_outputs
        .extend((0..4).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Point)));
    operation_outputs.extend((0..4).map(|_| IntentOperationOutput::curve(1)));
    operation_outputs.extend(
        (0..5).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Constraint)),
    );
    operation_outputs
        .extend((0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Scalar)));
    operation_outputs.extend(
        (0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Dimension)),
    );
    let polyline = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        key("rpc.polyline"),
    )
    .with_dynamic_children(3);
    let polyline_port_count = polyline
        .schema_generated_port_count()
        .expect("bounded polyline port shape");
    let rectangle = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::Rectangle,
        },
        key("rpc.rectangle"),
    )
    .with_field(
        IntentFieldKey(key("origin")),
        IntentLiteral::Point([1.0, 2.0]),
    )
    .with_field(
        IntentFieldKey(key("width")),
        IntentLiteral::Quantity {
            value: 4.0,
            unit: IntentUnit::Length,
        },
    )
    .with_field(
        IntentFieldKey(key("height")),
        IntentLiteral::Quantity {
            value: 3.0,
            unit: IntentUnit::Length,
        },
    )
    .with_field(
        IntentFieldKey(key("role")),
        IntentLiteral::Enum(key("profile")),
    )
    .with_operation_outputs(operation_outputs.clone());
    let rectangle_port_count = rectangle
        .schema_generated_port_count()
        .expect("bounded rectangle-operation port shape");

    let mut rpc = rpc();
    let response = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            rpc.coordinator().intent().identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("polyline"),
                    draft: Box::new(polyline),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("rectangle"),
                    draft: Box::new(rectangle),
                    cell: None,
                },
            ],
        )),
    });
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("valid child and operation-output declarations must remain queryable")
    };
    let polyline_node = receipt
        .aliases
        .node(&key("polyline"))
        .expect("polyline alias");
    let rectangle_node = receipt
        .aliases
        .node(&key("rectangle"))
        .expect("rectangle alias");

    let authoritative_polyline = rpc
        .coordinator()
        .intent()
        .graph()
        .node(polyline_node)
        .expect("authoritative polyline");
    let expected_children = authoritative_polyline
        .child_order
        .iter()
        .map(|child_id| {
            let child = &authoritative_polyline.children[child_id];
            (
                *child_id,
                child.schema,
                child
                    .ports
                    .iter()
                    .map(|port_id| authoritative_polyline.ports[port_id].as_ref(polyline_node))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_children.len(), 3);
    assert_eq!(authoritative_polyline.ports.len(), polyline_port_count);
    assert_eq!(
        rpc.coordinator()
            .intent()
            .graph()
            .node(rectangle_node)
            .expect("authoritative rectangle")
            .ports
            .len(),
        rectangle_port_count
    );

    let snapshot = rpc.snapshot();
    let declarations = &snapshot.graph.cells[0].declarations;
    let projected_polyline = declarations
        .iter()
        .find(|declaration| declaration.node == polyline_node)
        .expect("projected polyline");
    assert_eq!(projected_polyline.children.len(), expected_children.len());
    for (projected, (child, schema, ports)) in
        projected_polyline.children.iter().zip(expected_children)
    {
        assert_eq!(projected.child, child);
        assert_eq!(projected.schema, schema);
        assert_eq!(projected.ports, ports);
    }

    let projected_rectangle = declarations
        .iter()
        .find(|declaration| declaration.node == rectangle_node)
        .expect("projected rectangle operation");
    assert_eq!(projected_rectangle.operation_outputs, operation_outputs);
    assert_eq!(
        projected_rectangle
            .operation_outputs
            .iter()
            .filter(|output| output.kind == IntentOperationOutputKind::Curve)
            .map(|output| output.curve_span_count)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 1]
    );

    let encoded = serde_json::to_string(&snapshot.graph).expect("graph query serializes");
    let decoded = serde_json::from_str(&encoded).expect("graph query round trips");
    assert_eq!(snapshot.graph, decoded);
}

#[test]
fn rpc_graph_snapshot_compacts_large_bootstrap_payload_to_sha256_metadata() {
    const PAYLOAD_BYTES: usize = 512 * 1024;
    let payload = vec![0xa5; PAYLOAD_BYTES];
    let payload_sha256 = intent_content_digest(&payload);
    let codec = key("rpc-large-bootstrap-v1");
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Bootstrap {
            object: IntentBootstrapObject::new(BootstrapNativeKind::Point, codec.clone(), payload)
                .expect("bounded bootstrap payload"),
        },
        key("rpc.large.bootstrap"),
    );
    let mut rpc = rpc();
    let response = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            rpc.coordinator().intent().identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key("bootstrap"),
                draft: Box::new(draft),
                cell: None,
            }],
        )),
    });
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("unsupported host codec must remain queryable as retained intent")
    };
    assert_eq!(
        receipt.disposition,
        geosolve_sketch_intent::IntentPlanDisposition::RetainedFailed
    );
    let node = receipt
        .aliases
        .node(&key("bootstrap"))
        .expect("bootstrap alias");
    let snapshot = rpc.snapshot();
    let declaration = snapshot.graph.cells[0]
        .declarations
        .iter()
        .find(|declaration| declaration.node == node)
        .expect("retained bootstrap declaration");
    let IntentGraphNodeKind::Bootstrap { object } = &declaration.kind else {
        panic!("bootstrap kind metadata must stay explicit")
    };
    assert_eq!(object.kind, BootstrapNativeKind::Point);
    assert_eq!(object.codec, codec);
    assert_eq!(object.payload_bytes, PAYLOAD_BYTES as u64);
    assert_eq!(object.payload_sha256, payload_sha256);
    assert!(declaration.bootstrap_origin.is_none());

    let encoded = serde_json::to_string(&snapshot.graph).expect("bounded graph query serializes");
    assert!(!encoded.contains("\"payload\":"));
    assert!(encoded.contains(&format!("\"payload_bytes\":{PAYLOAD_BYTES}")));
    assert!(encoded.contains(&payload_sha256.to_string()));
    assert!(
        encoded.len() < 8 * 1024,
        "opaque payload size must not scale the stable query response"
    );
    let full_snapshot = serde_json::to_string(&snapshot).expect("full RPC snapshot serializes");
    assert!(!full_snapshot.contains("\"payload\":"));
    assert!(
        full_snapshot.len() < 32 * 1024,
        "workbench projection must not reintroduce opaque bootstrap bytes"
    );

    let inspector = rpc.apply(IntentRpcRequest::Inspector { node });
    let encoded_inspector = serde_json::to_string(&inspector).expect("Inspector RPC serializes");
    assert!(!encoded_inspector.contains("\"payload\":"));
    assert!(
        encoded_inspector.len() < 8 * 1024,
        "Inspector response must remain independent of bootstrap payload size"
    );

    let encoded_receipt = serde_json::to_string(&IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    })
    .expect("mutation receipt serializes");
    assert!(!encoded_receipt.contains("\"snapshot\":"));
    assert!(encoded_receipt.len() < 4 * 1024);
}

#[test]
fn native_and_json_rpc_share_exact_patch_history_and_projection() {
    let mut native = rpc();
    let patch = IntentPatch::new(
        native.coordinator().intent().identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::CreateNode {
            alias: key("point"),
            draft: Box::new(point()),
            cell: None,
        }],
    );
    let request = IntentRpcRequest::ApplyPatch {
        patch: Box::new(patch),
    };
    let request_json = serde_json::to_string(&request).unwrap();
    let response_json = native.apply_json(&request_json);
    let response: IntentRpcOutcome = serde_json::from_str(&response_json).unwrap();
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("valid typed patch must be accepted")
    };
    assert_eq!(
        receipt.disposition,
        geosolve_sketch_intent::IntentPlanDisposition::Accepted
    );
    assert_eq!(receipt.identity, native.coordinator().intent().identity());
    let snapshot = native.snapshot();
    assert_eq!(snapshot.identity, native.coordinator().intent().identity());
    assert_eq!(snapshot.projection.outline[0].declarations.len(), 1);
    assert_eq!(snapshot.projection.history.applied.len(), 1);
    assert!(snapshot.accepted_validation.is_some());

    let undo = native.apply_json(r#"{"method":"undo"}"#);
    assert!(!undo.contains("\"snapshot\":"));
    assert!(undo.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
    let undo: IntentRpcOutcome = serde_json::from_str(&undo).unwrap();
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::History { receipt },
    } = undo
    else {
        panic!("Undo must return a history result")
    };
    assert!(receipt.moved);
    assert_eq!(receipt.identity, native.coordinator().intent().identity());
    let snapshot = native.snapshot();
    assert!(snapshot.projection.outline[0].declarations.is_empty());
    assert_eq!(snapshot.projection.history.redoable.len(), 1);

    let redo = native.apply_json(r#"{"method":"redo"}"#);
    assert!(!redo.contains("\"snapshot\":"));
    assert!(redo.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
    let redo: IntentRpcOutcome = serde_json::from_str(&redo).unwrap();
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::History { receipt },
    } = redo
    else {
        panic!("Redo must return a history receipt")
    };
    assert!(receipt.moved);
    assert_eq!(receipt.identity, native.coordinator().intent().identity());
    assert_eq!(native.snapshot().projection.history.applied.len(), 1);
}

#[test]
fn source_token_mutation_returns_only_its_exact_post_commit_receipt() {
    let mut rpc = rpc();
    assert!(matches!(
        rpc.apply(IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                rpc.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: key("point"),
                    draft: Box::new(point()),
                    cell: None,
                }],
            )),
        }),
        IntentRpcOutcome::Success { .. }
    ));
    let before = rpc.coordinator().intent().identity();
    let token = rpc
        .snapshot()
        .projection
        .structured_source
        .tokens
        .into_iter()
        .find(|token| {
            matches!(
                token.target,
                geosolve_constraint_editor::IntentSourceTokenTarget::NodeName { .. }
            )
        })
        .expect("generated node-name token");
    let request = IntentRpcRequest::EditSourceToken {
        expected: Box::new(before),
        token: token.id,
        replacement: r#""RPC renamed point""#.to_owned(),
    };

    let encoded = rpc.apply_json(&serde_json::to_string(&request).unwrap());
    assert!(!encoded.contains("\"snapshot\":"));
    assert!(encoded.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
    let response: IntentRpcOutcome = serde_json::from_str(&encoded).unwrap();
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("valid source-token edit returns a patch receipt")
    };
    assert_ne!(receipt.identity, before);
    assert_eq!(receipt.identity, rpc.coordinator().intent().identity());
    assert_eq!(
        receipt.disposition,
        geosolve_sketch_intent::IntentPlanDisposition::OrganizationOnly
    );
    assert!(receipt.aliases.nodes.is_empty());
    assert!(receipt.aliases.ports.is_empty());
    assert!(receipt.aliases.cells.is_empty());
    assert_eq!(
        rpc.snapshot().projection.outline[0].declarations[0].name,
        key("RPC renamed point")
    );
}

#[test]
fn malformed_and_stale_rpc_are_fail_closed_and_preserve_authority() {
    let mut rpc = rpc();
    let identity = rpc.coordinator().intent().identity();
    let malformed = rpc.apply_json(r#"{"method":"execute_typescript","source":"solve()"}"#);
    let malformed: IntentRpcOutcome = serde_json::from_str(&malformed).unwrap();
    let IntentRpcOutcome::Failure { failure } = malformed else {
        panic!("unknown method must fail")
    };
    assert_eq!(failure.code, "invalid_request");
    assert_eq!(failure.identity, Some(identity));
    assert_eq!(rpc.coordinator().intent().identity(), identity);

    let unknown = rpc.apply(IntentRpcRequest::Inspector {
        node: geosolve_sketch_intent::NodeId::from_raw(99),
    });
    let IntentRpcOutcome::Success {
        value:
            IntentRpcSuccess::Inspector {
                identity: projected,
                inspector,
            },
    } = unknown
    else {
        panic!("unknown Inspector selection is a valid empty projection")
    };
    assert!(inspector.is_none());
    assert_eq!(*projected, identity);
    assert_eq!(rpc.coordinator().intent().identity(), identity);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact stale-token regression preserves source remapping, history and accepted evidence together"
)]
fn stale_source_token_after_reorder_preserves_identity_history_and_accepted_evidence() {
    let mut rpc = rpc();
    assert!(matches!(
        rpc.apply(IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                rpc.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![
                    IntentPatchOperation::CreateNode {
                        alias: key("first"),
                        draft: Box::new(named_point("rpc.first", [1.0, 2.0])),
                        cell: None,
                    },
                    IntentPatchOperation::CreateNode {
                        alias: key("second"),
                        draft: Box::new(named_point("rpc.second", [3.0, 4.0])),
                        cell: None,
                    },
                ],
            )),
        }),
        IntentRpcOutcome::Success { .. }
    ));

    let stale_snapshot = rpc.snapshot();
    let stale_token = stale_snapshot
        .projection
        .structured_source
        .tokens
        .iter()
        .find(|token| {
            matches!(
                token.target,
                geosolve_constraint_editor::IntentSourceTokenTarget::NodeName { .. }
            )
        })
        .expect("first source name token")
        .clone();
    let geosolve_constraint_editor::IntentSourceTokenTarget::NodeName { node: stale_owner } =
        stale_token.target
    else {
        unreachable!("filtered source token is a node name")
    };
    let default_cell = rpc.coordinator().intent().organization().cell_order()[0];
    assert!(matches!(
        rpc.apply(IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                rpc.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::MoveDeclaration {
                    node: stale_owner,
                    cell: CellTarget::Stable { cell: default_cell },
                    before: None,
                }],
            )),
        }),
        IntentRpcOutcome::Success { .. }
    ));
    let current_snapshot = rpc.snapshot();
    assert!(
        current_snapshot
            .projection
            .structured_source
            .tokens
            .iter()
            .find(|token| token.id == stale_token.id)
            .is_some_and(|token| {
                matches!(
                    token.target,
                    geosolve_constraint_editor::IntentSourceTokenTarget::NodeName { node }
                        if node != stale_owner
                )
            }),
        "the stale numeric token must now address the other declaration"
    );

    let identity_before = rpc.coordinator().intent().identity();
    let history_before = current_snapshot.projection.history;
    let accepted_evidence_before = rpc
        .coordinator()
        .accepted_materialization()
        .expect("accepted native authority")
        .evidence
        .clone();
    let stale_edit = IntentRpcRequest::EditSourceToken {
        expected: Box::new(stale_snapshot.identity),
        token: stale_token.id,
        replacement: r#""must not rename the other declaration""#.to_owned(),
    };
    let stale_edit: IntentRpcOutcome = serde_json::from_str(
        &rpc.apply_json(&serde_json::to_string(&stale_edit).expect("closed request serializes")),
    )
    .expect("closed response deserializes");
    assert!(matches!(
        stale_edit,
        IntentRpcOutcome::Failure { ref failure }
            if failure.code == "source_edit_rejected"
                && failure.identity == Some(identity_before)
    ));
    assert_eq!(rpc.coordinator().intent().identity(), identity_before);
    assert_eq!(rpc.snapshot().projection.history, history_before);
    assert_eq!(
        rpc.coordinator()
            .accepted_materialization()
            .expect("stale source rejection retains native authority")
            .evidence,
        accepted_evidence_before
    );
}

#[test]
fn retained_invalid_stale_and_oversized_rpc_requests_preserve_exact_authority() {
    let mut rpc = rpc();
    let pristine = rpc.coordinator().intent().identity();
    let accepted = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            pristine,
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point()),
                cell: None,
            }],
        )),
    });
    assert!(matches!(
        accepted,
        IntentRpcOutcome::Success {
            value: IntentRpcSuccess::Patch { ref receipt }
        } if receipt.disposition
            == geosolve_sketch_intent::IntentPlanDisposition::Accepted
    ));
    let accepted_identity = rpc.coordinator().intent().identity();
    let accepted_validation = rpc
        .coordinator()
        .accepted_materialization()
        .expect("accepted native authority")
        .validation
        .clone();

    let retained = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            accepted_identity,
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key("invalid"),
                draft: Box::new(invalid_segment()),
                cell: None,
            }],
        )),
    });
    assert!(matches!(
        retained,
        IntentRpcOutcome::Success {
            value: IntentRpcSuccess::Patch { ref receipt }
        } if receipt.disposition
            == geosolve_sketch_intent::IntentPlanDisposition::RetainedFailed
    ));
    let retained_identity = rpc.coordinator().intent().identity();
    assert_eq!(
        rpc.coordinator()
            .accepted_materialization()
            .expect("retained accepted authority")
            .validation,
        accepted_validation,
    );

    let stale = rpc.apply(IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            pristine,
            IntentPatchPolicy::RequireAccepted,
            Vec::new(),
        )),
    });
    assert!(matches!(
        stale,
        IntentRpcOutcome::Failure { ref failure }
            if failure.code == "patch_rejected" && failure.identity == Some(retained_identity)
    ));
    assert_eq!(rpc.coordinator().intent().identity(), retained_identity);

    let oversized = " ".repeat(MAX_INTENT_RPC_REQUEST_BYTES + 1);
    let response: IntentRpcOutcome = serde_json::from_str(&rpc.apply_json(&oversized)).unwrap();
    assert!(matches!(
        response,
        IntentRpcOutcome::Failure { ref failure }
            if failure.code == "request_too_large"
                && failure.identity == Some(retained_identity)
    ));
    assert_eq!(rpc.coordinator().intent().identity(), retained_identity);
    assert_eq!(
        rpc.coordinator()
            .accepted_materialization()
            .expect("oversize rejection retains native authority")
            .validation,
        accepted_validation,
    );
}

#[test]
fn excessive_schema_generated_alias_receipt_rejects_before_publication() {
    let mut rpc = rpc();
    let before = rpc.coordinator().intent().clone();
    let wide_operation = |symbol: &str| {
        IntentNodeDraft::new(
            IntentNodeKind::Operation {
                operation: OperationKind::ProfileOffset,
            },
            key(symbol),
        )
        .with_operation_outputs(vec![IntentOperationOutput::curve(u16::MAX)])
    };
    let request = IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            before.identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("first-operation"),
                    draft: Box::new(wide_operation("rpc.excessive.first")),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("second-operation"),
                    draft: Box::new(wide_operation("rpc.excessive.second")),
                    cell: None,
                },
            ],
        )),
    };

    let response = rpc.apply(request);
    assert!(matches!(
        response,
        IntentRpcOutcome::Failure { ref failure }
            if failure.code == "receipt_too_large"
                && failure.identity == Some(before.identity())
    ));
    assert_eq!(rpc.coordinator().intent(), &before);
    assert!(rpc.coordinator().accepted_materialization().is_none());
    let encoded = serde_json::to_string(&response).expect("bounded failure receipt serializes");
    assert!(encoded.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
}

#[test]
fn code_authored_rpc_continues_through_live_gui_projection_and_one_history() {
    let mut editor = editor();
    let request = IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("point"),
                draft: Box::new(point()),
                cell: None,
            }],
        )),
    };
    let response = apply_intent_rpc_to_editor(&mut editor, request);
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::Patch { receipt },
    } = response
    else {
        panic!("code-authored patch must publish through the live editor")
    };
    let node = receipt.aliases.node(&key("point")).unwrap();
    let port = receipt
        .aliases
        .port(
            &key("point"),
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
        )
        .unwrap();
    let IntentNativeBinding::Point(native_point) = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
    else {
        panic!("code-authored point must retain an editable native binding")
    };

    let projection = editor.workbench_projection();
    assert_eq!(projection.outline[0].declarations[0].node, node);
    assert!(editor.set_selected_declaration(Some(node)));
    let inspector = editor.selected_inspector(&projection).unwrap();
    let x = inspector
        .fields
        .iter()
        .find_map(|field| match field {
            IntentInspectorField::Instance { leaf, .. } if leaf.field == LeafField::X => {
                Some(*leaf)
            }
            _ => None,
        })
        .unwrap();
    editor
        .edit_inspector(
            &inspector,
            &IntentInspectorEditTarget::Instance { leaf: x },
            IntentInspectorEditValue::Literal {
                literal: IntentLiteral::Quantity {
                    value: 7.0,
                    unit: IntentUnit::Length,
                },
            },
        )
        .unwrap();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    assert_eq!(
        accepted.document().point(native_point).unwrap().position[0].to_bits(),
        7.0_f64.to_bits()
    );
    assert_eq!(editor.workbench_projection().history.applied.len(), 2);

    let rpc_undo = apply_intent_rpc_to_editor(&mut editor, IntentRpcRequest::Undo);
    assert!(matches!(
        rpc_undo,
        IntentRpcOutcome::Success {
            value: IntentRpcSuccess::History { ref receipt }
        } if receipt.moved
    ));
    assert_eq!(editor.workbench_projection().history.applied.len(), 1);
    assert_eq!(editor.workbench_projection().history.redoable.len(), 1);
    editor.redo().unwrap().unwrap();
    assert_eq!(editor.workbench_projection().history.applied.len(), 2);
    editor.undo().unwrap().unwrap();
    let rpc_redo = apply_intent_rpc_to_editor(&mut editor, IntentRpcRequest::Redo);
    assert!(matches!(
        rpc_redo,
        IntentRpcOutcome::Success {
            value: IntentRpcSuccess::History { ref receipt }
        } if receipt.moved
    ));
    assert_eq!(editor.workbench_projection().history.applied.len(), 2);
}

#[test]
fn live_editor_rpc_parse_resource_and_stale_failures_are_state_neutral() {
    let mut editor = editor();
    let stale_identity = editor.coordinator().intent().identity();
    let accepted = apply_intent_rpc_to_editor(
        &mut editor,
        IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                stale_identity,
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: key("point"),
                    draft: Box::new(point()),
                    cell: None,
                }],
            )),
        },
    );
    assert!(matches!(accepted, IntentRpcOutcome::Success { .. }));
    let before = editor.coordinator().intent().clone();
    let accepted_evidence = editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .evidence
        .clone();

    let malformed: IntentRpcOutcome = serde_json::from_str(&apply_intent_rpc_json_to_editor(
        &mut editor,
        r#"{"method":"execute_typescript","source":"solve()"}"#,
    ))
    .unwrap();
    assert!(matches!(
        malformed,
        IntentRpcOutcome::Failure { ref failure } if failure.code == "invalid_request"
    ));
    assert_eq!(editor.coordinator().intent(), &before);

    let oversized = " ".repeat(MAX_INTENT_RPC_REQUEST_BYTES + 1);
    let exhausted: IntentRpcOutcome =
        serde_json::from_str(&apply_intent_rpc_json_to_editor(&mut editor, &oversized)).unwrap();
    assert!(matches!(
        exhausted,
        IntentRpcOutcome::Failure { ref failure } if failure.code == "request_too_large"
    ));
    assert_eq!(editor.coordinator().intent(), &before);

    let stale = IntentRpcRequest::ApplyPatch {
        patch: Box::new(IntentPatch::new(
            stale_identity,
            IntentPatchPolicy::RequireAccepted,
            Vec::new(),
        )),
    };
    let stale: IntentRpcOutcome = serde_json::from_str(&apply_intent_rpc_json_to_editor(
        &mut editor,
        &serde_json::to_string(&stale).unwrap(),
    ))
    .unwrap();
    assert!(matches!(
        stale,
        IntentRpcOutcome::Failure { ref failure } if failure.code == "patch_rejected"
    ));
    assert_eq!(editor.coordinator().intent(), &before);
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .evidence,
        accepted_evidence
    );
}
