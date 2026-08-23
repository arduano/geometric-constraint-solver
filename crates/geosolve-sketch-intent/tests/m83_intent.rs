// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch_intent::{
    BootstrapNativeKind, ConstraintKind, DimensionKind, GeometryRecipeKind, IdentityTransitionKind,
    InputRole, InputSlot, IntentBootstrapObject, IntentEvaluation, IntentEvaluationFailure,
    IntentEvaluationFailureKind, IntentGraphError, IntentIdentityFlow, IntentKey, IntentLiteral,
    IntentNativeReservationKind, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPlanError, IntentPortKind, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentUnit, LeafField, LeafRef,
    MaterializationEvidence, NodeId, OperationKind, PatchPortRef,
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
        evidence: MaterializationEvidence::new_independently_validated(
            candidate.external_inputs().identity(),
            format!("materialized:{:?}", candidate.semantic_identity()).into_bytes(),
            b"owners".to_vec(),
            b"independent-residual-validation".to_vec(),
            true,
        )
        .unwrap(),
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
fn unordered_patch_permutations_allocate_identical_nodes_ports_and_bytes() {
    fn operations() -> Vec<IntentPatchOperation> {
        let relation = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::Coincident,
            },
            key("Coincident"),
        )
        .with_input(InputSlot::new(InputRole::Point, 0), alias_point("a"))
        .with_input(InputSlot::new(InputRole::Point, 1), alias_point("b"));
        vec![
            IntentPatchOperation::CreateNode {
                alias: key("constraint"),
                draft: Box::new(relation),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("b"),
                draft: Box::new(point_draft("B")),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("a"),
                draft: Box::new(point_draft("A")),
                cell: None,
            },
        ]
    }

    let id = IntentSessionId::from_raw(0x8301);
    let mut first = IntentSession::with_id(id).unwrap();
    let mut second = IntentSession::with_id(id).unwrap();
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
    let first_plan = first.plan_patch(first_patch, accepted).unwrap();
    let second_plan = second.plan_patch(second_patch, accepted).unwrap();
    assert_eq!(first_plan.aliases(), second_plan.aliases());
    assert_eq!(first_plan.target(), second_plan.target());
    assert_eq!(first_plan.token(), second_plan.token());
    first.commit_plan(first_plan).unwrap();
    second.commit_plan(second_plan).unwrap();
    assert_eq!(
        first.to_canonical_json().unwrap(),
        second.to_canonical_json().unwrap()
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
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Operation {
                        operation: OperationKind::ProfileOffset,
                    },
                    key("Profile offset"),
                )),
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
                alias: key("constraint"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Constraint {
                        constraint: ConstraintKind::PointOnCurve,
                    },
                    key("Point on curve"),
                )),
                cell: None,
            },
            IntentPatchOperation::CreateNode {
                alias: key("dimension"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Dimension {
                        dimension: DimensionKind::CurveLength,
                    },
                    key("Curve length"),
                )),
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
        (GeometryRecipeKind::TangentArc, 0, 1, 3, 1, 0, 3),
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
        .map(
            |(index, (recipe, children, ..))| IntentPatchOperation::CreateNode {
                alias: key(&format!("recipe-{index:02}")),
                draft: Box::new(
                    IntentNodeDraft::new(
                        IntentNodeKind::Geometry { recipe: *recipe },
                        key(&format!("Recipe {index:02}")),
                    )
                    .with_dynamic_children(*children),
                ),
                cell: None,
            },
        )
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
        assert_eq!(
            node.ports
                .values()
                .filter(|port| port.kind == IntentPortKind::HandlePoint)
                .count(),
            handles,
            "logical handle inventory for {recipe:?}"
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
