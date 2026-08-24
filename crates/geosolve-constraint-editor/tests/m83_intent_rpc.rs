// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentInspectorEditTarget, IntentInspectorEditValue,
    IntentInspectorField, IntentNativeBinding, IntentRpcOutcome, IntentRpcRequest,
    IntentRpcSession, IntentRpcSuccess, MAX_INTENT_RPC_REQUEST_BYTES, ProjectionalEditorSession,
    ProjectionalIntentCoordinator, apply_intent_rpc_json_to_editor, apply_intent_rpc_to_editor,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSessionId,
    IntentUnit, LeafField,
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
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("rpc.point"),
    )
    .with_instance_leaf(
        selector,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 1.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        selector,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: 2.0,
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
        value:
            IntentRpcSuccess::Patch {
                disposition,
                snapshot,
                ..
            },
    } = response
    else {
        panic!("valid typed patch must be accepted")
    };
    assert_eq!(
        disposition,
        geosolve_sketch_intent::IntentPlanDisposition::Accepted
    );
    assert_eq!(snapshot.identity, native.coordinator().intent().identity());
    assert_eq!(snapshot.projection.outline[0].declarations.len(), 1);
    assert_eq!(snapshot.projection.history.applied.len(), 1);
    assert!(snapshot.accepted_validation.is_some());

    let undo = native.apply_json(r#"{"method":"undo"}"#);
    let undo: IntentRpcOutcome = serde_json::from_str(&undo).unwrap();
    let IntentRpcOutcome::Success {
        value: IntentRpcSuccess::History { moved, snapshot },
    } = undo
    else {
        panic!("Undo must return a history result")
    };
    assert!(moved);
    assert!(snapshot.projection.outline[0].declarations.is_empty());
    assert_eq!(snapshot.projection.history.redoable.len(), 1);
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
        value: IntentRpcSuccess::Inspector { inspector, .. },
    } = unknown
    else {
        panic!("unknown Inspector selection is a valid empty projection")
    };
    assert!(inspector.is_none());
    assert_eq!(rpc.coordinator().intent().identity(), identity);
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
            value: IntentRpcSuccess::Patch {
                disposition: geosolve_sketch_intent::IntentPlanDisposition::Accepted,
                ..
            }
        }
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
            value: IntentRpcSuccess::Patch {
                disposition: geosolve_sketch_intent::IntentPlanDisposition::RetainedFailed,
                ..
            }
        }
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
        value: IntentRpcSuccess::Patch { aliases, .. },
    } = response
    else {
        panic!("code-authored patch must publish through the live editor")
    };
    let node = aliases.node(&key("point")).unwrap();
    let port = aliases
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
            value: IntentRpcSuccess::History { moved: true, .. }
        }
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
            value: IntentRpcSuccess::History { moved: true, .. }
        }
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
