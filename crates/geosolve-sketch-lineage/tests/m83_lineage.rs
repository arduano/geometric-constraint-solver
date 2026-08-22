// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_lineage::{
    ImportedBaselineAction, ImportedBaselineEncoding, LineageAcceptedAuthority,
    LineageActionDefinition, LineageAllocatorHighWater, LineageAuxiliaryHighWater,
    LineageDeveloperKey, LineageDigest, LineageDocument, LineageDocumentError, LineageDocumentId,
    LineageEvaluationAttempt, LineageEvaluationDisposition, LineageEvaluationPolicy,
    LineageInputBinding, LineageLifecycleHighWater, LineageMutation, LineageOpaqueId,
    LineageOutput, LineageOutputId, LineageOutputIdentity, LineageOutputIdentityFlow,
    LineageOutputKind, LineageOutputRef, LineagePatch, LineageReservation, LineageReservationId,
    LineageReservationKind, LineageRevision, LineageSemanticKey, LineageSession, LineageStep,
    LineageStepId, LineageStepRewrite, MAX_LINEAGE_JSON_BYTES, VersionedActionPayload,
    lineage_content_digest,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TestSessionCheckpointWire {
    document: String,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<String>,
    last_accepted: Option<LineageAcceptedAuthority>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TestSessionWire {
    version: u32,
    document: String,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<String>,
    last_accepted: Option<LineageAcceptedAuthority>,
    undo: Vec<TestSessionCheckpointWire>,
    redo: Vec<TestSessionCheckpointWire>,
    lifecycle: LineageLifecycleHighWater,
    auxiliary_high_waters: BTreeMap<LineageSemanticKey, LineageAuxiliaryHighWater>,
    digest: LineageDigest,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TestSessionCanonicalPayload<'a> {
    version: u32,
    document: &'a str,
    latest_attempt: &'a Option<LineageEvaluationAttempt>,
    last_accepted_document: &'a Option<String>,
    last_accepted: &'a Option<LineageAcceptedAuthority>,
    undo: &'a [TestSessionCheckpointWire],
    redo: &'a [TestSessionCheckpointWire],
    lifecycle: LineageLifecycleHighWater,
    auxiliary_high_waters: &'a BTreeMap<LineageSemanticKey, LineageAuxiliaryHighWater>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TestDocumentWire {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: Vec<LineageStep>,
    digest: LineageDigest,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TestDocumentCanonicalPayload<'a> {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: &'a [LineageStep],
}

impl TestSessionWire {
    fn refresh_digest(&mut self) {
        let payload = TestSessionCanonicalPayload {
            version: self.version,
            document: &self.document,
            latest_attempt: &self.latest_attempt,
            last_accepted_document: &self.last_accepted_document,
            last_accepted: &self.last_accepted,
            undo: &self.undo,
            redo: &self.redo,
            lifecycle: self.lifecycle,
            auxiliary_high_waters: &self.auxiliary_high_waters,
        };
        self.digest = lineage_content_digest(&serde_json::to_vec(&payload).unwrap());
    }
}

impl TestDocumentWire {
    fn allocator_high_water(&self) -> LineageAllocatorHighWater {
        LineageAllocatorHighWater {
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
        }
    }

    fn refresh_digest(&mut self) {
        let payload = TestDocumentCanonicalPayload {
            version: self.version,
            document_id: self.document_id,
            revision: self.revision,
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
            evaluation_policy: self.evaluation_policy,
            steps: &self.steps,
        };
        self.digest = lineage_content_digest(&serde_json::to_vec(&payload).unwrap());
    }
}

fn developer_key(value: &str) -> LineageDeveloperKey {
    LineageDeveloperKey::new(value).unwrap()
}

fn semantic_key(value: &str) -> LineageSemanticKey {
    LineageSemanticKey::new(value).unwrap()
}

fn opaque_id(value: &str) -> LineageOpaqueId {
    LineageOpaqueId::new(value).unwrap()
}

fn output(id: u64, key: &str, kind: LineageOutputKind, reservation: Option<u64>) -> LineageOutput {
    LineageOutput {
        id: LineageOutputId::from_raw(id),
        key: semantic_key(key),
        kind,
        reservation: reservation.map(LineageReservationId::from_raw),
    }
}

fn reservation(
    id: u64,
    key: &str,
    kind: LineageReservationKind,
    persistent_id: &str,
) -> LineageReservation {
    LineageReservation {
        id: LineageReservationId::from_raw(id),
        key: semantic_key(key),
        kind,
        persistent_id: opaque_id(persistent_id),
    }
}

fn action(
    category: &str,
    schema: &str,
    inputs: Vec<LineageInputBinding>,
    parameters: BTreeMap<String, Value>,
) -> LineageActionDefinition {
    let action = VersionedActionPayload {
        schema: semantic_key(schema),
        version: 1,
        inputs,
        parameters,
    };
    match category {
        "geometry" => LineageActionDefinition::GeometryRecipe { action },
        "constraint" => LineageActionDefinition::Constraint { action },
        "dimension" => LineageActionDefinition::Dimension { action },
        "trim" => LineageActionDefinition::Trim { action },
        "parameter" => LineageActionDefinition::Parameter { action },
        "binding" => LineageActionDefinition::Binding { action },
        "external" => LineageActionDefinition::External { action },
        "operation" => LineageActionDefinition::Operation { action },
        "feature" => LineageActionDefinition::ComputedFeature { action },
        "annotation" => LineageActionDefinition::Annotation { action },
        _ => panic!("unknown test category"),
    }
}

fn geometry_step(
    step: u64,
    output_id: u64,
    reservation_id: u64,
    key: &str,
    output_kind: LineageOutputKind,
    reservation_kind: LineageReservationKind,
) -> LineageStep {
    LineageStep::new(
        LineageStepId::from_raw(step),
        developer_key(key),
        key,
        action(
            "geometry",
            "geosolve.recipe.test",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(
            output_id,
            "result",
            output_kind,
            Some(reservation_id),
        )],
        vec![reservation(
            reservation_id,
            "result",
            reservation_kind,
            &format!("native-{reservation_id}"),
        )],
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "test fixture keeps every identity and kind choice explicit"
)]
fn dependent_step(
    document: LineageDocumentId,
    step: u64,
    output_id: u64,
    reservation_id: u64,
    key: &str,
    category: &str,
    source_step: u64,
    source_output: u64,
    source_kind: LineageOutputKind,
    result_kind: LineageOutputKind,
    result_reservation_kind: LineageReservationKind,
) -> LineageStep {
    let input = LineageInputBinding {
        key: semantic_key("source"),
        kind: source_kind,
        source: LineageOutputRef {
            document,
            step: LineageStepId::from_raw(source_step),
            output: LineageOutputId::from_raw(source_output),
            kind: source_kind,
        },
    };
    LineageStep::new(
        LineageStepId::from_raw(step),
        developer_key(key),
        key,
        action(
            category,
            &format!("geosolve.{category}.test"),
            vec![input],
            BTreeMap::new(),
        ),
        vec![output(
            output_id,
            "result",
            result_kind,
            Some(reservation_id),
        )],
        vec![reservation(
            reservation_id,
            "result",
            result_reservation_kind,
            &format!("native-{reservation_id}"),
        )],
    )
}

fn continuation_step(
    document: LineageDocumentId,
    step: u64,
    output_id: u64,
    source_step: u64,
    source_output: u64,
    kind: LineageOutputKind,
) -> LineageStep {
    let source = LineageOutputRef {
        document,
        step: LineageStepId::from_raw(source_step),
        output: LineageOutputId::from_raw(source_output),
        kind,
    };
    LineageStep::new(
        LineageStepId::from_raw(step),
        developer_key(&format!("continuation-{step}")),
        format!("Continuation {step}"),
        action(
            "operation",
            "geosolve.operation.identity-continue",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(output_id, "continued", kind, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(output_id),
        flow: LineageOutputIdentityFlow::Continued { source },
    }])
}

fn retirement_step(
    document: LineageDocumentId,
    step: u64,
    output_id: u64,
    source_step: u64,
    source_output: u64,
    kind: LineageOutputKind,
) -> LineageStep {
    let source = LineageOutputRef {
        document,
        step: LineageStepId::from_raw(source_step),
        output: LineageOutputId::from_raw(source_output),
        kind,
    };
    LineageStep::new(
        LineageStepId::from_raw(step),
        developer_key(&format!("retirement-{step}")),
        format!("Retirement {step}"),
        action(
            "operation",
            "geosolve.operation.identity-retire",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(output_id, "retired", kind, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(output_id),
        flow: LineageOutputIdentityFlow::Retired { source },
    }])
}

fn insert(document: &mut LineageDocument, step: LineageStep) {
    let outcome = document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(step),
            }],
        ))
        .unwrap();
    assert!(outcome.changed);
}

fn three_step_document() -> LineageDocument {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x83));
    let id = document.id();
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "source",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    insert(
        &mut document,
        dependent_step(
            id,
            2,
            2,
            2,
            "horizontal",
            "constraint",
            1,
            1,
            LineageOutputKind::Curve,
            LineageOutputKind::Constraint,
            LineageReservationKind::Constraint,
        ),
    );
    insert(
        &mut document,
        geometry_step(
            3,
            3,
            3,
            "unrelated",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    document
}

fn insert_complete_supplemental_action_categories(
    document: &mut LineageDocument,
    id: LineageDocumentId,
) {
    for (step, key, category, output_kind, reservation_kind) in [
        (
            7,
            "trim-view",
            "trim",
            LineageOutputKind::TrimView,
            LineageReservationKind::TrimView,
        ),
        (
            8,
            "parameter",
            "parameter",
            LineageOutputKind::Parameter,
            LineageReservationKind::Parameter,
        ),
        (
            9,
            "binding",
            "binding",
            LineageOutputKind::ParameterBinding,
            LineageReservationKind::ParameterBinding,
        ),
        (
            10,
            "external",
            "external",
            LineageOutputKind::ExternalBinding,
            LineageReservationKind::ExternalBinding,
        ),
        (
            11,
            "annotation",
            "annotation",
            LineageOutputKind::Annotation,
            LineageReservationKind::Annotation,
        ),
    ] {
        insert(
            document,
            dependent_step(
                id,
                step,
                step,
                step,
                key,
                category,
                1,
                1,
                LineageOutputKind::Curve,
                output_kind,
                reservation_kind,
            ),
        );
    }
}

#[test]
fn canonical_round_trip_preserves_identity_and_generic_action_categories() {
    let mut document = three_step_document();
    let id = document.id();
    insert(
        &mut document,
        dependent_step(
            id,
            4,
            4,
            4,
            "distance",
            "dimension",
            3,
            3,
            LineageOutputKind::Point,
            LineageOutputKind::Dimension,
            LineageReservationKind::Dimension,
        ),
    );
    insert(
        &mut document,
        dependent_step(
            id,
            5,
            5,
            5,
            "offset",
            "operation",
            1,
            1,
            LineageOutputKind::Curve,
            LineageOutputKind::Operation,
            LineageReservationKind::Operation,
        ),
    );
    insert(
        &mut document,
        dependent_step(
            id,
            6,
            6,
            6,
            "fillets",
            "feature",
            1,
            1,
            LineageOutputKind::Curve,
            LineageOutputKind::Feature,
            LineageReservationKind::Feature,
        ),
    );
    insert_complete_supplemental_action_categories(&mut document, id);

    let canonical = document.to_canonical_json().unwrap();
    let restored = LineageDocument::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    assert_eq!(restored.identity(), document.identity());
    assert!(canonical.contains("\"revision\":\"000000000000000b\""));
    assert!(canonical.contains("\"kind\":\"computed_feature\""));
    for kind in ["trim", "parameter", "binding", "external", "annotation"] {
        assert!(canonical.contains(&format!("\"kind\":\"{kind}\"")));
    }

    let mut tampered: Value = serde_json::from_str(&canonical).unwrap();
    tampered["evaluation_policy"] = json!("dependency_local");
    assert!(matches!(
        LineageDocument::from_json(&serde_json::to_string(&tampered).unwrap()),
        Err(LineageDocumentError::DigestMismatch)
    ));
}

#[test]
fn imported_baseline_is_truthful_unique_first_root() {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8301));
    let baseline = LineageStep::new(
        LineageStepId::from_raw(1),
        developer_key("legacy"),
        "Imported baseline",
        LineageActionDefinition::ImportedBaseline {
            baseline: ImportedBaselineAction {
                encoding: ImportedBaselineEncoding::Opaque {
                    media_type: semantic_key("application/vnd.geosolve.workspace-draft"),
                    version: 5,
                },
                payload: "opaque-draft-v5-bytes".into(),
            },
        },
        vec![output(1, "legacy-point", LineageOutputKind::Point, Some(1))],
        vec![reservation(
            1,
            "legacy-point",
            LineageReservationKind::Point,
            "legacy-native-point",
        )],
    );
    insert(&mut document, baseline);
    insert(
        &mut document,
        geometry_step(
            2,
            2,
            2,
            "new-point",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    let json = document.to_canonical_json().unwrap();
    assert_eq!(
        LineageDocument::from_json(&json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        json
    );

    let mut invalid = LineageDocument::with_id(LineageDocumentId::from_raw(0x8302));
    insert(
        &mut invalid,
        geometry_step(
            1,
            1,
            1,
            "first",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    let late_baseline = LineageStep::new(
        LineageStepId::from_raw(2),
        developer_key("late-root"),
        "late",
        LineageActionDefinition::ImportedBaseline {
            baseline: ImportedBaselineAction {
                encoding: ImportedBaselineEncoding::Canonical {
                    schema: semantic_key("geosolve.sketch"),
                    version: 4,
                },
                payload: "{}".into(),
            },
        },
        Vec::new(),
        Vec::new(),
    );
    let before = invalid.identity();
    assert!(matches!(
        invalid.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(late_baseline),
            }],
        )),
        Err(LineageDocumentError::InvalidBaselinePosition { .. })
    ));
    assert_eq!(invalid.identity(), before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one rejection test verifies four related reference-integrity sentinels"
)]
fn wrong_forward_cross_document_and_cyclic_references_reject_atomically() {
    let mut forward = LineageDocument::with_id(LineageDocumentId::from_raw(0x8310));
    let document_id = forward.id();
    let consumer = dependent_step(
        document_id,
        1,
        1,
        1,
        "consumer",
        "constraint",
        2,
        2,
        LineageOutputKind::Curve,
        LineageOutputKind::Constraint,
        LineageReservationKind::Constraint,
    );
    let provider = geometry_step(
        2,
        2,
        2,
        "provider",
        LineageOutputKind::Curve,
        LineageReservationKind::Curve,
    );
    let before = forward.identity();
    assert!(matches!(
        forward.apply_patch(LineagePatch::new(
            before,
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(consumer),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(provider),
                },
            ],
        )),
        Err(LineageDocumentError::ForwardReference { .. })
    ));
    assert_eq!(forward.identity(), before);
    assert!(forward.steps().is_empty());

    let mut wrong_kind = LineageDocument::with_id(LineageDocumentId::from_raw(0x8311));
    insert(
        &mut wrong_kind,
        geometry_step(
            1,
            1,
            1,
            "curve",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    let mut wrong = dependent_step(
        wrong_kind.id(),
        2,
        2,
        2,
        "point-consumer",
        "constraint",
        1,
        1,
        LineageOutputKind::Point,
        LineageOutputKind::Constraint,
        LineageReservationKind::Constraint,
    );
    if let LineageActionDefinition::Constraint { action } = &mut wrong.action {
        action.inputs[0].source.kind = LineageOutputKind::Point;
    }
    assert!(matches!(
        wrong_kind.apply_patch(LineagePatch::new(
            wrong_kind.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(wrong),
            }],
        )),
        Err(LineageDocumentError::WrongOutputKind { .. })
    ));

    let mut cross = dependent_step(
        LineageDocumentId::from_raw(0xdead),
        2,
        2,
        2,
        "cross",
        "constraint",
        1,
        1,
        LineageOutputKind::Curve,
        LineageOutputKind::Constraint,
        LineageReservationKind::Constraint,
    );
    cross.id = LineageStepId::from_raw(2);
    assert!(matches!(
        wrong_kind.apply_patch(LineagePatch::new(
            wrong_kind.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(cross),
            }],
        )),
        Err(LineageDocumentError::CrossDocumentReference { .. })
    ));

    let mut cyclic = LineageDocument::with_id(LineageDocumentId::from_raw(0x8312));
    let id = cyclic.id();
    let first = dependent_step(
        id,
        1,
        1,
        1,
        "first",
        "operation",
        2,
        2,
        LineageOutputKind::Operation,
        LineageOutputKind::Operation,
        LineageReservationKind::Operation,
    );
    let second = dependent_step(
        id,
        2,
        2,
        2,
        "second",
        "operation",
        1,
        1,
        LineageOutputKind::Operation,
        LineageOutputKind::Operation,
        LineageReservationKind::Operation,
    );
    assert!(matches!(
        cyclic.apply_patch(LineagePatch::new(
            cyclic.identity(),
            vec![
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(first),
                },
                LineageMutation::Insert {
                    before: None,
                    step: Box::new(second),
                },
            ],
        )),
        Err(LineageDocumentError::DependencyCycle { .. })
    ));
}

#[test]
fn plain_tombstone_rejects_live_dependents_but_delete_subtree_owns_them() {
    let mut strict = three_step_document();
    let before = strict.identity();
    assert!(matches!(
        strict.apply_patch(LineagePatch::new(
            strict.identity(),
            vec![LineageMutation::Tombstone {
                step: LineageStepId::from_raw(1),
            }],
        )),
        Err(LineageDocumentError::LiveDependent {
            deleted,
            dependent,
        }) if deleted == LineageStepId::from_raw(1)
            && dependent == LineageStepId::from_raw(2)
    ));
    assert_eq!(strict.identity(), before);

    let mut subtree = three_step_document();
    let result = subtree
        .apply_patch(LineagePatch::new(
            subtree.identity(),
            vec![LineageMutation::DeleteSubtree {
                root: LineageStepId::from_raw(1),
            }],
        ))
        .unwrap();
    assert_eq!(
        result.tombstoned_steps,
        vec![LineageStepId::from_raw(1), LineageStepId::from_raw(2)]
    );
    assert_eq!(
        subtree.step(LineageStepId::from_raw(3)).unwrap().state,
        geosolve_sketch_lineage::LineageStepState::Live
    );
}

#[test]
fn rewrite_rebind_and_reorder_are_atomic_and_identity_preserving() {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8320));
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "first-point",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    insert(
        &mut document,
        geometry_step(
            2,
            2,
            2,
            "second-point",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    let document_id = document.id();
    insert(
        &mut document,
        dependent_step(
            document_id,
            3,
            3,
            3,
            "coincident",
            "constraint",
            1,
            1,
            LineageOutputKind::Point,
            LineageOutputKind::Constraint,
            LineageReservationKind::Constraint,
        ),
    );

    let ids_before = document.allocator_high_water();
    let mut parameters = BTreeMap::new();
    parameters.insert("x".into(), json!(42.0));
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![
                LineageMutation::Rewrite {
                    step: LineageStepId::from_raw(1),
                    replacement: Box::new(LineageStepRewrite {
                        label: "Moved first point".into(),
                        action: action("geometry", "geosolve.recipe.test", Vec::new(), parameters),
                    }),
                },
                LineageMutation::Rebind {
                    step: LineageStepId::from_raw(3),
                    input: semantic_key("source"),
                    target: document
                        .output_ref(LineageStepId::from_raw(2), LineageOutputId::from_raw(2))
                        .unwrap(),
                },
            ],
        ))
        .unwrap();
    assert_eq!(document.allocator_high_water(), ids_before);
    assert_eq!(
        document.step(LineageStepId::from_raw(1)).unwrap().outputs[0].id,
        LineageOutputId::from_raw(1)
    );

    let before_invalid_reorder = document.identity();
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before_invalid_reorder,
            vec![LineageMutation::Reorder {
                step: LineageStepId::from_raw(2),
                before: None,
            }],
        )),
        Err(LineageDocumentError::ForwardReference { .. })
    ));
    assert_eq!(document.identity(), before_invalid_reorder);
}

#[test]
fn session_undo_redo_never_reuses_abandoned_allocations() {
    let document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8330));
    let mut session = LineageSession::new(document).unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    let inserted_identity = session.identity();
    assert_eq!(session.undo_len(), 1);
    session.undo().unwrap().unwrap();
    assert!(session.document().steps().is_empty());
    assert_eq!(
        session.document().allocator_high_water().next_step_id,
        LineageStepId::from_raw(2)
    );
    assert_eq!(session.redo_len(), 1);

    let stale = LineagePatch::new(
        inserted_identity,
        vec![LineageMutation::SetEvaluationPolicy {
            policy: LineageEvaluationPolicy::DependencyLocal,
        }],
    );
    assert!(matches!(
        session.apply_patch(stale),
        Err(LineageDocumentError::StalePatch { .. })
    ));
    assert_eq!(session.redo_len(), 1);

    let no_op = session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::StrictChronological,
            }],
        ))
        .unwrap();
    assert!(!no_op.changed);
    assert_eq!(session.redo_len(), 1);

    session.redo().unwrap().unwrap();
    assert_eq!(session.document().steps()[0].id, LineageStepId::from_raw(1));
    session.undo().unwrap().unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    2,
                    2,
                    2,
                    "replacement",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    assert_eq!(session.document().steps()[0].id, LineageStepId::from_raw(2));
    assert_eq!(session.redo_len(), 0);
    assert!(session.lifecycle_high_water().revision.raw() >= 5);
}

#[test]
fn session_patch_rejects_abandoned_step_identity_rebinding_atomically() {
    let mut session = LineageSession::new(LineageDocument::with_id(LineageDocumentId::from_raw(
        0x83_301,
    )))
    .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    session.undo().unwrap().unwrap();
    let before = session.to_canonical_session_json().unwrap();

    assert!(matches!(
        session.apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    2,
                    2,
                    2,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        )),
        Err(LineageDocumentError::CrossHistoryIdentityRebinding {
            identity: "step",
            ..
        })
    ));
    assert_eq!(session.to_canonical_session_json().unwrap(), before);
}

#[derive(Clone, Copy)]
enum AbandonedIdentityKind {
    Step,
    Output,
    Reservation,
}

fn session_after_divergent_edit_clears_first_branch() -> LineageSession {
    let mut session = LineageSession::new(LineageDocument::with_id(LineageDocumentId::from_raw(
        0x83_302,
    )))
    .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "abandoned-first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    session.undo().unwrap().unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    2,
                    2,
                    2,
                    "divergent-second",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    assert_eq!(session.redo_len(), 0, "the first branch must be abandoned");
    assert_eq!(
        session.lifecycle_high_water().allocator.next_step_id,
        LineageStepId::from_raw(3)
    );
    session
}

fn replacement_reintroducing_abandoned_numeric_identity(
    session: &LineageSession,
    identity: AbandonedIdentityKind,
) -> LineageDocument {
    let mut replacement = session.document().clone();
    insert(
        &mut replacement,
        geometry_step(
            3,
            3,
            3,
            "new-third",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    let mut wire: TestDocumentWire =
        serde_json::from_str(&replacement.to_canonical_json().unwrap()).unwrap();
    let step = wire
        .steps
        .iter_mut()
        .find(|step| step.id == LineageStepId::from_raw(3))
        .unwrap();
    match identity {
        AbandonedIdentityKind::Step => {
            step.id = LineageStepId::from_raw(1);
            step.key = developer_key("reused-abandoned-step");
        }
        AbandonedIdentityKind::Output => {
            step.outputs[0].id = LineageOutputId::from_raw(1);
            step.output_identities[0].output = LineageOutputId::from_raw(1);
        }
        AbandonedIdentityKind::Reservation => {
            step.reservations[0].id = LineageReservationId::from_raw(1);
            step.outputs[0].reservation = Some(LineageReservationId::from_raw(1));
            step.output_identities[0].flow = LineageOutputIdentityFlow::Created {
                reservation: LineageReservationId::from_raw(1),
            };
        }
    }
    assert_eq!(
        wire.allocator_high_water(),
        replacement.allocator_high_water(),
        "the hostile replacement retains every lifecycle cursor"
    );
    wire.refresh_digest();
    LineageDocument::from_json(&serde_json::to_string(&wire).unwrap()).unwrap()
}

#[test]
fn reconcile_rejects_numeric_id_reuse_after_divergence_clears_redo_atomically() {
    for (kind, expected_identity) in [
        (AbandonedIdentityKind::Step, "step"),
        (AbandonedIdentityKind::Output, "output"),
        (AbandonedIdentityKind::Reservation, "reservation"),
    ] {
        let mut session = session_after_divergent_edit_clears_first_branch();
        let replacement = replacement_reintroducing_abandoned_numeric_identity(&session, kind);
        let before = session.to_canonical_session_json().unwrap();

        assert!(matches!(
            session.reconcile(session.identity(), replacement),
            Err(LineageDocumentError::CrossHistoryIdentityRebinding { identity, .. })
                if identity == expected_identity
        ));
        assert_eq!(
            session.to_canonical_session_json().unwrap(),
            before,
            "rejected {expected_identity} reuse must preserve exact session authority"
        );
    }
}

#[test]
fn session_auxiliary_high_water_is_canonical_monotonic_and_outside_history() {
    let document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8335));
    let mut session = LineageSession::new(document).unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    session.undo().unwrap().unwrap();
    let key = semantic_key("computed-evaluation-next-revision");
    let history = (session.undo_len(), session.redo_len(), session.identity());

    session
        .retain_auxiliary_high_water(key.clone(), LineageAuxiliaryHighWater::from_raw(41))
        .unwrap();
    session
        .retain_auxiliary_high_water(key.clone(), LineageAuxiliaryHighWater::from_raw(7))
        .unwrap();
    assert_eq!(
        session.auxiliary_high_water(&key),
        Some(LineageAuxiliaryHighWater::from_raw(41))
    );
    assert_eq!(
        (session.undo_len(), session.redo_len(), session.identity()),
        history,
        "lifecycle-only retention must not create history or clear Redo"
    );

    let canonical = session.to_canonical_session_json().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&canonical).unwrap()["auxiliary_high_waters"]["computed-evaluation-next-revision"],
        json!("0000000000000029")
    );
    let mut restored = LineageSession::from_session_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_session_json().unwrap(), canonical);
    restored.redo().unwrap().unwrap();
    assert_eq!(
        restored.auxiliary_high_water(&key),
        Some(LineageAuxiliaryHighWater::from_raw(41)),
        "Redo must not restore a lower lifecycle cursor"
    );
    restored.undo().unwrap().unwrap();
    assert_eq!(
        restored.auxiliary_high_water(&key),
        Some(LineageAuxiliaryHighWater::from_raw(41)),
        "Undo must not restore a lower lifecycle cursor"
    );
}

#[test]
fn complete_program_reconcile_is_exact_atomic_and_history_owned() {
    let mut current = LineageDocument::with_id(LineageDocumentId::from_raw(0x8331));
    insert(
        &mut current,
        geometry_step(
            1,
            1,
            1,
            "first",
            LineageOutputKind::Point,
            LineageReservationKind::Point,
        ),
    );
    let mut session = LineageSession::new(current).unwrap();
    let before = session.identity();
    let mut replacement = session.document().clone();
    replacement
        .apply_patch(LineagePatch::new(
            replacement.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::DependencyLocal,
            }],
        ))
        .unwrap();
    let replacement_identity = replacement.identity();
    assert_eq!(
        session.reconcile(before, replacement).unwrap(),
        replacement_identity
    );
    assert_eq!(session.undo_len(), 1);
    assert_eq!(session.redo_len(), 0);
    assert_eq!(
        session.document().evaluation_policy(),
        LineageEvaluationPolicy::DependencyLocal
    );
    session.undo().unwrap().unwrap();
    assert_eq!(
        session.document().evaluation_policy(),
        LineageEvaluationPolicy::StrictChronological
    );

    let history_before = (session.undo_len(), session.redo_len(), session.identity());
    let same_revision = session.document().clone();
    assert!(matches!(
        session.reconcile(session.identity(), same_revision),
        Err(LineageDocumentError::NonSequentialReplacementRevision { .. })
    ));
    assert_eq!(
        (session.undo_len(), session.redo_len(), session.identity()),
        history_before
    );

    let current_identity = session.identity();
    let mut lower_allocator = LineageDocument::with_id(LineageDocumentId::from_raw(0x8331));
    for policy in [
        LineageEvaluationPolicy::DependencyLocal,
        LineageEvaluationPolicy::StrictChronological,
        LineageEvaluationPolicy::DependencyLocal,
        LineageEvaluationPolicy::StrictChronological,
    ] {
        let outcome = lower_allocator
            .apply_patch(LineagePatch::new(
                lower_allocator.identity(),
                vec![LineageMutation::SetEvaluationPolicy { policy }],
            ))
            .unwrap();
        // The sequence above alternates from the persisted strict default.
        assert!(
            outcome.changed,
            "test policy replacement must advance every revision"
        );
    }
    while lower_allocator.revision().raw() < current_identity.revision.raw() + 1 {
        let policy = if lower_allocator.evaluation_policy()
            == LineageEvaluationPolicy::StrictChronological
        {
            LineageEvaluationPolicy::DependencyLocal
        } else {
            LineageEvaluationPolicy::StrictChronological
        };
        lower_allocator
            .apply_patch(LineagePatch::new(
                lower_allocator.identity(),
                vec![LineageMutation::SetEvaluationPolicy { policy }],
            ))
            .unwrap();
    }
    assert_eq!(
        lower_allocator.revision().raw(),
        current_identity.revision.raw() + 1
    );
    assert!(matches!(
        session.reconcile(current_identity, lower_allocator),
        Err(LineageDocumentError::AllocatorRegression {
            allocator: "step ID",
            ..
        })
    ));
    assert_eq!(
        (session.undo_len(), session.redo_len(), session.identity()),
        history_before
    );
}

#[derive(Clone, Copy)]
enum HistoricalIdentityRebinding {
    Step,
    Action,
    StepManifest,
    Output,
    Reservation,
    Persistent,
}

fn replacement_with_historical_identity_rebinding(
    document_id: LineageDocumentId,
    rebinding: HistoricalIdentityRebinding,
) -> LineageDocument {
    let mut step = geometry_step(
        1,
        1,
        1,
        "first",
        LineageOutputKind::Point,
        LineageReservationKind::Point,
    );
    match rebinding {
        HistoricalIdentityRebinding::Step => step.key = developer_key("rebound-step"),
        HistoricalIdentityRebinding::Action => {
            let LineageActionDefinition::GeometryRecipe { action } = &mut step.action else {
                panic!("test geometry action expected");
            };
            action.schema = semantic_key("geosolve.recipe.rebound");
        }
        HistoricalIdentityRebinding::StepManifest => {
            step.outputs
                .push(output(2, "extra-output", LineageOutputKind::Point, Some(2)));
            step.output_identities.push(LineageOutputIdentity {
                output: LineageOutputId::from_raw(2),
                flow: LineageOutputIdentityFlow::Created {
                    reservation: LineageReservationId::from_raw(2),
                },
            });
            step.reservations.push(reservation(
                2,
                "extra-output",
                LineageReservationKind::Point,
                "native-2",
            ));
        }
        HistoricalIdentityRebinding::Output => {
            step.outputs[0].key = semantic_key("rebound-output");
        }
        HistoricalIdentityRebinding::Reservation => {
            step.reservations[0].key = semantic_key("rebound-reservation");
        }
        HistoricalIdentityRebinding::Persistent => {
            step.reservations[0].persistent_id = opaque_id("rebound-persistent-id");
        }
    }

    let mut replacement = LineageDocument::with_id(document_id);
    insert(&mut replacement, step);
    replacement
        .apply_patch(LineagePatch::new(
            replacement.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::DependencyLocal,
            }],
        ))
        .unwrap();
    replacement
}

#[test]
fn reconcile_rejects_cross_history_identity_rebinding_atomically() {
    for (rebinding, expected_identity) in [
        (HistoricalIdentityRebinding::Step, "step"),
        (HistoricalIdentityRebinding::Action, "step action"),
        (HistoricalIdentityRebinding::StepManifest, "step"),
        (HistoricalIdentityRebinding::Output, "output"),
        (HistoricalIdentityRebinding::Reservation, "reservation"),
        (
            HistoricalIdentityRebinding::Persistent,
            "persistent identity",
        ),
    ] {
        let document_id = LineageDocumentId::from_raw(0x83_321);
        let mut current = LineageDocument::with_id(document_id);
        insert(
            &mut current,
            geometry_step(
                1,
                1,
                1,
                "first",
                LineageOutputKind::Point,
                LineageReservationKind::Point,
            ),
        );
        let mut session = LineageSession::new(current).unwrap();
        session
            .accept_current(
                session.identity(),
                Some(opaque_id("host-inputs-v1")),
                LineageDigest::from_bytes([0x21; 32]),
            )
            .unwrap();
        let before = session.to_canonical_session_json().unwrap();
        let replacement = replacement_with_historical_identity_rebinding(document_id, rebinding);

        assert!(matches!(
            session.reconcile(session.identity(), replacement),
            Err(LineageDocumentError::CrossHistoryIdentityRebinding { identity, .. })
                if identity == expected_identity
        ));
        assert_eq!(
            session.to_canonical_session_json().unwrap(),
            before,
            "rejected {expected_identity} rebinding must preserve complete authority"
        );
    }
}

#[test]
fn canonical_session_load_rejects_cross_history_identity_rebinding() {
    let document_id = LineageDocumentId::from_raw(0x83_322);
    let mut session = LineageSession::new(LineageDocument::with_id(document_id)).unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::DependencyLocal,
            }],
        ))
        .unwrap();

    let forged_document = replacement_with_historical_identity_rebinding(
        document_id,
        HistoricalIdentityRebinding::Action,
    );
    assert_eq!(forged_document.revision(), session.document().revision());
    let mut wire: TestSessionWire =
        serde_json::from_str(&session.to_canonical_session_json().unwrap()).unwrap();
    wire.document = forged_document.to_canonical_json().unwrap();
    wire.latest_attempt.as_mut().unwrap().target = forged_document.identity();
    wire.refresh_digest();
    let forged = serde_json::to_string(&wire).unwrap();

    assert!(matches!(
        LineageSession::from_session_json(&forged),
        Err(LineageDocumentError::CrossHistoryIdentityRebinding {
            identity: "step action",
            ..
        })
    ));
}

#[test]
fn net_no_op_mutation_batch_preserves_revision_history_redo_and_evaluation_evidence() {
    let mut session = LineageSession::new(LineageDocument::with_id(LineageDocumentId::from_raw(
        0x83_323,
    )))
    .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(geometry_step(
                    1,
                    1,
                    1,
                    "first",
                    LineageOutputKind::Point,
                    LineageReservationKind::Point,
                )),
            }],
        ))
        .unwrap();
    session
        .accept_current(
            session.identity(),
            Some(opaque_id("host-inputs-v1")),
            LineageDigest::from_bytes([0x23; 32]),
        )
        .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::DependencyLocal,
            }],
        ))
        .unwrap();
    session.undo().unwrap().unwrap();
    assert!(session.can_redo());

    let before = session.to_canonical_session_json().unwrap();
    let identity = session.identity();
    let latest_attempt = session.latest_attempt().cloned();
    let last_accepted = session.last_accepted().cloned();
    let history = (session.undo_len(), session.redo_len());
    let outcome = session
        .apply_patch(LineagePatch::new(
            identity,
            vec![
                LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::DependencyLocal,
                },
                LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::StrictChronological,
                },
            ],
        ))
        .unwrap();

    assert!(!outcome.changed);
    assert_eq!(outcome.identity, identity);
    assert_eq!(session.identity(), identity);
    assert_eq!((session.undo_len(), session.redo_len()), history);
    assert_eq!(session.latest_attempt(), latest_attempt.as_ref());
    assert_eq!(session.last_accepted(), last_accepted.as_ref());
    assert_eq!(session.to_canonical_session_json().unwrap(), before);
}

#[test]
fn decoding_and_generic_payloads_are_bounded() {
    let oversized = " ".repeat(MAX_LINEAGE_JSON_BYTES + 1);
    assert!(matches!(
        LineageDocument::from_json(&oversized),
        Err(LineageDocumentError::JsonResourceLimit { .. })
    ));

    let mut nested = Value::Null;
    for _ in 0..=geosolve_sketch_lineage::MAX_LINEAGE_PAYLOAD_DEPTH {
        nested = Value::Array(vec![nested]);
    }
    let mut parameters = BTreeMap::new();
    parameters.insert("nested".into(), nested);
    let step = LineageStep::new(
        LineageStepId::from_raw(1),
        developer_key("deep"),
        "deep",
        action("geometry", "geosolve.recipe.deep", Vec::new(), parameters),
        Vec::new(),
        Vec::new(),
    );
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x8340));
    let before = document.identity();
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(step),
            }],
        )),
        Err(LineageDocumentError::ResourceLimit {
            resource: "generic payload depth",
            ..
        })
    ));
    assert_eq!(document.identity(), before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the identity-transition scenario keeps allocation and dependency evidence explicit"
)]
fn explicit_identity_flow_is_dependency_checked_and_retired_ports_are_unavailable() {
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x83_100));
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "source",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    let source = document.output_ref(LineageStepId::from_raw(1), LineageOutputId::from_raw(1));
    let alias = LineageStep::new(
        LineageStepId::from_raw(2),
        developer_key("alias"),
        "Alias exact source",
        action(
            "operation",
            "geosolve.operation.identity-alias",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(2, "continued", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(2),
        flow: LineageOutputIdentityFlow::Aliased {
            source: source.unwrap(),
        },
    }]);
    insert(&mut document, alias);

    let retired = LineageStep::new(
        LineageStepId::from_raw(3),
        developer_key("retire"),
        "Retire exact alias",
        action(
            "operation",
            "geosolve.operation.identity-retire",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(3, "retired", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(3),
        flow: LineageOutputIdentityFlow::Retired {
            source: document
                .output_ref(LineageStepId::from_raw(2), LineageOutputId::from_raw(2))
                .unwrap(),
        },
    }]);
    insert(&mut document, retired);

    let dependent = dependent_step(
        document.id(),
        4,
        4,
        2,
        "invalid-retired-dependent",
        "constraint",
        3,
        3,
        LineageOutputKind::Curve,
        LineageOutputKind::Constraint,
        LineageReservationKind::Constraint,
    );
    let before = document.identity();
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(dependent),
            }],
        )),
        Err(LineageDocumentError::RetiredOutputReference {
            step,
            provider,
            output,
        }) if step == LineageStepId::from_raw(4)
            && provider == LineageStepId::from_raw(3)
            && output == LineageOutputId::from_raw(3)
    ));
    assert_eq!(document.identity(), before);

    let predecessor_dependent = dependent_step(
        document.id(),
        4,
        4,
        2,
        "invalid-retired-predecessor-dependent",
        "constraint",
        2,
        2,
        LineageOutputKind::Curve,
        LineageOutputKind::Constraint,
        LineageReservationKind::Constraint,
    );
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(predecessor_dependent),
            }],
        )),
        Err(LineageDocumentError::RetiredOutputReference {
            step,
            provider,
            output,
        }) if step == LineageStepId::from_raw(4)
            && provider == LineageStepId::from_raw(2)
            && output == LineageOutputId::from_raw(2)
    ));
    assert_eq!(document.identity(), before);

    let duplicate_retirement = LineageStep::new(
        LineageStepId::from_raw(4),
        developer_key("duplicate-retirement"),
        "Retire the consumed predecessor again",
        action(
            "operation",
            "geosolve.operation.identity-retire",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(4, "retired-again", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(4),
        flow: LineageOutputIdentityFlow::Retired {
            source: document
                .output_ref(LineageStepId::from_raw(1), LineageOutputId::from_raw(1))
                .unwrap(),
        },
    }]);
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(duplicate_retirement),
            }],
        )),
        Err(LineageDocumentError::RetiredOutputReference {
            step,
            provider,
            output,
        }) if step == LineageStepId::from_raw(4)
            && provider == LineageStepId::from_raw(1)
            && output == LineageOutputId::from_raw(1)
    ));
    assert_eq!(document.identity(), before);

    let outcome = document
        .apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::DeleteSubtree {
                root: LineageStepId::from_raw(1),
            }],
        ))
        .unwrap();
    assert_eq!(
        outcome.tombstoned_steps,
        vec![
            LineageStepId::from_raw(1),
            LineageStepId::from_raw(2),
            LineageStepId::from_raw(3),
        ]
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the regression keeps source, alias, successor, and duplicate-transition identities explicit"
)]
fn continuation_consumes_every_predecessor_port_from_the_previous_generation() {
    let id = LineageDocumentId::from_raw(0x83_101);
    let mut document = LineageDocument::with_id(id);
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "source",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    let source = document
        .output_ref(LineageStepId::from_raw(1), LineageOutputId::from_raw(1))
        .unwrap();
    let alias = LineageStep::new(
        LineageStepId::from_raw(2),
        developer_key("source-alias"),
        "Alias source",
        action(
            "operation",
            "geosolve.operation.identity-alias",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(2, "source-alias", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(2),
        flow: LineageOutputIdentityFlow::Aliased { source },
    }]);
    insert(&mut document, alias);

    let continued = LineageStep::new(
        LineageStepId::from_raw(3),
        developer_key("continued-source"),
        "Continue source",
        action(
            "operation",
            "geosolve.operation.identity-continue",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(3, "continued", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(3),
        flow: LineageOutputIdentityFlow::Continued { source },
    }]);
    insert(&mut document, continued);

    for (source_step, source_output, key) in [
        (1, 1, "consumed-source-input"),
        (2, 2, "consumed-alias-input"),
    ] {
        let before = document.identity();
        let dependent = dependent_step(
            id,
            4,
            4,
            2,
            key,
            "constraint",
            source_step,
            source_output,
            LineageOutputKind::Curve,
            LineageOutputKind::Constraint,
            LineageReservationKind::Constraint,
        );
        assert!(matches!(
            document.apply_patch(LineagePatch::new(
                before,
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(dependent),
                }],
            )),
            Err(LineageDocumentError::ConsumedOutputReference {
                step,
                provider,
                output,
                successor,
            }) if step == LineageStepId::from_raw(4)
                && provider == LineageStepId::from_raw(source_step)
                && output == LineageOutputId::from_raw(source_output)
                && successor.step == LineageStepId::from_raw(3)
                && successor.output == LineageOutputId::from_raw(3)
        ));
        assert_eq!(document.identity(), before);
    }

    let before = document.identity();
    let duplicate = LineageStep::new(
        LineageStepId::from_raw(4),
        developer_key("duplicate-continuation"),
        "Duplicate continuation",
        action(
            "operation",
            "geosolve.operation.identity-continue",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![output(4, "duplicate", LineageOutputKind::Curve, None)],
        Vec::new(),
    )
    .with_output_identities(vec![LineageOutputIdentity {
        output: LineageOutputId::from_raw(4),
        flow: LineageOutputIdentityFlow::Continued {
            source: document
                .output_ref(LineageStepId::from_raw(2), LineageOutputId::from_raw(2))
                .unwrap(),
        },
    }]);
    assert!(matches!(
        document.apply_patch(LineagePatch::new(
            before,
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(duplicate),
            }],
        )),
        Err(LineageDocumentError::ConsumedOutputReference { successor, .. })
            if successor.step == LineageStepId::from_raw(3)
    ));
    assert_eq!(document.identity(), before);
}

#[test]
fn suppressed_continuation_restores_referencable_predecessor() {
    let id = LineageDocumentId::from_raw(0x83_102);
    let mut document = LineageDocument::with_id(id);
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "source",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    insert(
        &mut document,
        continuation_step(id, 2, 2, 1, 1, LineageOutputKind::Curve),
    );
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::SetSuppressed {
                step: LineageStepId::from_raw(2),
                suppressed: true,
            }],
        ))
        .unwrap();

    insert(
        &mut document,
        continuation_step(id, 3, 3, 1, 1, LineageOutputKind::Curve),
    );
    assert!(document.validate().is_ok());
}

#[test]
fn tombstoned_continuation_restores_referencable_predecessor() {
    let id = LineageDocumentId::from_raw(0x83_103);
    let mut document = LineageDocument::with_id(id);
    insert(
        &mut document,
        geometry_step(
            1,
            1,
            1,
            "source",
            LineageOutputKind::Curve,
            LineageReservationKind::Curve,
        ),
    );
    insert(
        &mut document,
        continuation_step(id, 2, 2, 1, 1, LineageOutputKind::Curve),
    );
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Tombstone {
                step: LineageStepId::from_raw(2),
            }],
        ))
        .unwrap();

    insert(
        &mut document,
        continuation_step(id, 3, 3, 1, 1, LineageOutputKind::Curve),
    );
    assert!(document.validate().is_ok());
}

#[test]
fn inactive_retirement_restores_referencable_predecessor() {
    for (document_raw, tombstone) in [(0x83_104, false), (0x83_105, true)] {
        let id = LineageDocumentId::from_raw(document_raw);
        let mut document = LineageDocument::with_id(id);
        insert(
            &mut document,
            geometry_step(
                1,
                1,
                1,
                "source",
                LineageOutputKind::Curve,
                LineageReservationKind::Curve,
            ),
        );
        insert(
            &mut document,
            retirement_step(id, 2, 2, 1, 1, LineageOutputKind::Curve),
        );
        let lifecycle = if tombstone {
            LineageMutation::Tombstone {
                step: LineageStepId::from_raw(2),
            }
        } else {
            LineageMutation::SetSuppressed {
                step: LineageStepId::from_raw(2),
                suppressed: true,
            }
        };
        document
            .apply_patch(LineagePatch::new(document.identity(), vec![lifecycle]))
            .unwrap();

        insert(
            &mut document,
            continuation_step(id, 3, 3, 1, 1, LineageOutputKind::Curve),
        );
        assert!(document.validate().is_ok());
    }
}

#[test]
fn retained_failed_and_last_accepted_authority_round_trip_with_one_history() {
    let document = three_step_document();
    let mut session = LineageSession::new(document).unwrap();
    let accepted_identity = session.identity();
    let accepted_digest = LineageDigest::from_bytes([0x83; 32]);
    let external = Some(opaque_id("host-inputs-v1"));
    session
        .accept_current(accepted_identity, external.clone(), accepted_digest)
        .unwrap();

    let mut parameters = BTreeMap::new();
    parameters.insert("x".into(), json!(42.0));
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Rewrite {
                step: LineageStepId::from_raw(1),
                replacement: Box::new(LineageStepRewrite {
                    label: "edited source".into(),
                    action: action("geometry", "geosolve.recipe.test", Vec::new(), parameters),
                }),
            }],
        ))
        .unwrap();
    let rejected_identity = session.identity();
    assert_eq!(
        session.latest_attempt().unwrap().disposition,
        LineageEvaluationDisposition::Pending
    );
    session
        .reject_current(
            rejected_identity,
            external.clone(),
            semantic_key("domain.invalid-geometry"),
            vec![LineageStepId::from_raw(1)],
        )
        .unwrap();
    assert_eq!(session.undo_len(), 1);
    assert_eq!(session.last_accepted().unwrap().lineage, accepted_identity);
    assert_eq!(
        session.latest_attempt().unwrap().disposition,
        LineageEvaluationDisposition::Failed
    );

    let canonical = session.to_canonical_session_json().unwrap();
    let mut restored = LineageSession::from_session_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_session_json().unwrap(), canonical);
    assert_eq!(restored.identity(), rejected_identity);
    assert_eq!(restored.undo_len(), 1);
    assert_eq!(
        restored.last_accepted().unwrap().materialization_digest,
        accepted_digest
    );

    let undone = restored.undo().unwrap().unwrap();
    assert_ne!(undone, accepted_identity);
    assert_eq!(
        restored.latest_attempt().unwrap().disposition,
        LineageEvaluationDisposition::Pending
    );
    assert_eq!(restored.last_accepted().unwrap().lineage, accepted_identity);
    assert!(restored.can_redo());

    let history_before = (restored.undo_len(), restored.redo_len());
    assert!(matches!(
        restored.accept_current(rejected_identity, external, accepted_digest),
        Err(LineageDocumentError::StalePatch { .. })
    ));
    assert_eq!((restored.undo_len(), restored.redo_len()), history_before);

    let mut tampered: Value = serde_json::from_str(&canonical).unwrap();
    tampered["lifecycle"]["revision"] = json!("ffffffffffffffff");
    assert!(matches!(
        LineageSession::from_session_json(&serde_json::to_string(&tampered).unwrap()),
        Err(LineageDocumentError::DigestMismatch)
    ));
}

#[test]
fn untrusted_session_load_discards_current_and_historical_evaluation_authority() {
    let mut session = LineageSession::new(three_step_document()).unwrap();
    let external = Some(opaque_id("caller-asserted-inputs"));
    session
        .accept_current(
            session.identity(),
            external.clone(),
            LineageDigest::from_bytes([0x11; 32]),
        )
        .unwrap();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Rewrite {
                step: LineageStepId::from_raw(1),
                replacement: Box::new(LineageStepRewrite {
                    label: "edited source".into(),
                    action: action(
                        "geometry",
                        "geosolve.recipe.test",
                        Vec::new(),
                        BTreeMap::new(),
                    ),
                }),
            }],
        ))
        .unwrap();
    session
        .accept_current(
            session.identity(),
            external,
            LineageDigest::from_bytes([0x22; 32]),
        )
        .unwrap();
    session.undo().unwrap().unwrap();
    assert!(session.last_accepted().is_some());
    assert!(session.can_redo());

    session.discard_unverified_evaluation_authority();
    assert_eq!(
        session.latest_attempt().unwrap().disposition,
        LineageEvaluationDisposition::Pending
    );
    assert!(session.last_accepted().is_none());
    assert!(session.last_accepted_document().is_none());
    assert!(session.can_redo());

    session.redo().unwrap().unwrap();
    assert_eq!(
        session.latest_attempt().unwrap().disposition,
        LineageEvaluationDisposition::Pending
    );
    assert!(session.last_accepted().is_none());
    assert!(session.last_accepted_document().is_none());
    session.undo().unwrap().unwrap();
    assert!(session.last_accepted().is_none());
}
