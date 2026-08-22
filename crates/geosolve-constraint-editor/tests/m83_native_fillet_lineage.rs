// SPDX-License-Identifier: GPL-3.0-or-later

//! Focused owning-boundary regressions for native-Fillet lineage replay.

use std::collections::BTreeSet;

use geosolve_constraint_editor::{
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    RetainedEditorCoordinator, SelectionItem, evaluate_lineage_session_cold,
    evaluate_lineage_session_cold_with_inputs,
};
use geosolve_sketch::{
    CurveDefinition, CurveId, DesignPointId, DocumentConstraintDefinition, DocumentEdit,
    DocumentNativeLineFilletIds, DocumentParameterKind, DocumentParameterTarget,
    DocumentSolveRequest, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageEvaluationPolicy, LineageMaterializationMap, LineageMutation,
    LineagePatch, LineageSession, LineageStepId, LineageStepRewrite, VersionedActionPayload,
    lineage_content_digest,
};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
struct LineCorner {
    corner: DesignPointId,
    lines: [CurveId; 2],
}

fn add_line_corner(document: &mut SketchDocument, label: &str, origin: [f64; 2]) -> LineCorner {
    let outer = document
        .add_point(format!("{label}.outer"), origin)
        .expect("first outer point");
    let corner = document
        .add_point(format!("{label}.corner"), [origin[0] + 4.0, origin[1]])
        .expect("corner point");
    let second_outer = document
        .add_point(
            format!("{label}.second-outer"),
            [origin[0] + 4.0, origin[1] + 4.0],
        )
        .expect("second outer point");
    let first = document
        .add_curve(
            format!("{label}.first"),
            CurveDefinition::Line {
                start: outer,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first source line");
    let second = document
        .add_curve(
            format!("{label}.second"),
            CurveDefinition::Line {
                start: corner,
                end: second_outer,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second source line");
    LineCorner {
        corner,
        lines: [first, second],
    }
}

fn two_corner_document() -> (SketchDocument, [LineCorner; 2]) {
    let mut document = SketchDocument::new(20.0).expect("native-Fillet document");
    let first = add_line_corner(&mut document, "first", [0.0, 0.0]);
    let second = add_line_corner(&mut document, "second", [10.0, 0.0]);
    (document, [first, second])
}

fn retained(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .expect("accepted native-Fillet fixture")
}

fn publish_native_fillet(
    coordinator: &mut RetainedEditorCoordinator,
    corner: DesignPointId,
    radius: f64,
    label: &str,
) -> DocumentNativeLineFilletIds {
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("native-Fillet authoring snapshot");
    let accepted = snapshot.sketch_document().clone();
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(&snapshot, &accepted, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert!(matches!(
        authoring.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(radius),
                ..FeatureAuthoringOptions::default()
            },
        ),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let transaction = coordinator
        .transact_feature_authoring_pick_items(
            &mut authoring,
            &[(SelectionItem::Point(corner), None)],
            label,
        )
        .expect("native-Fillet candidate transaction");
    let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = transaction.outcome else {
        panic!("line-line corner must produce a native-Fillet preview");
    };
    let preview = transaction.preview.expect("held native-Fillet preview");
    coordinator
        .native_feature_authoring_availability(preview.token, &candidate)
        .expect("native-Fillet publication availability");
    coordinator
        .apply_feature_authoring_native_profile(preview.token, &candidate)
        .expect("native-Fillet publication")
        .value
}

fn assert_ids_exist(coordinator: &RetainedEditorCoordinator, ids: &DocumentNativeLineFilletIds) {
    let document = coordinator.session().design_document();
    assert_eq!(
        ids.source_lines
            .map(|curve| document.curve(curve).is_some()),
        [true; 2]
    );
    assert!(document.point(ids.removed_corner).is_none());
    assert_eq!(
        ids.contact_points
            .map(|point| document.point(point).is_some()),
        [true; 2]
    );
    assert!(document.point(ids.center).is_some());
    assert!(document.curve(ids.arc).is_some());
    assert_eq!(
        ids.contacts
            .map(|contact| document.contact(contact).is_some()),
        [true; 2]
    );
    assert_eq!(
        ids.tangencies
            .map(|constraint| document.constraint(constraint).is_some()),
        [true; 2]
    );
    assert!(document.dimension(ids.radius_dimension).is_some());
    for scalar in [
        ids.radius,
        ids.start_angle,
        ids.end_angle,
        ids.contact_parameters[0],
        ids.contact_parameters[1],
        ids.radius_target,
    ] {
        assert!(document.scalar(scalar).is_some());
    }
}

fn assert_finite_accepted(coordinator: &RetainedEditorCoordinator) {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted native-Fillet authority");
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| { point.position.into_iter().all(f64::is_finite) })
    );
    assert!(
        accepted
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    let report = accepted.solve_result().unstable_core_report();
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
}

fn created_point_ids(ids: &DocumentNativeLineFilletIds) -> BTreeSet<DesignPointId> {
    ids.contact_points.into_iter().chain([ids.center]).collect()
}

fn assert_created_ids_are_disjoint(
    first: &DocumentNativeLineFilletIds,
    second: &DocumentNativeLineFilletIds,
) {
    assert!(created_point_ids(first).is_disjoint(&created_point_ids(second)));
    assert_ne!(first.arc, second.arc);
    let first_scalars = BTreeSet::from([
        first.radius,
        first.start_angle,
        first.end_angle,
        first.contact_parameters[0],
        first.contact_parameters[1],
        first.radius_target,
    ]);
    let second_scalars = BTreeSet::from([
        second.radius,
        second.start_angle,
        second.end_angle,
        second.contact_parameters[0],
        second.contact_parameters[1],
        second.radius_target,
    ]);
    assert!(first_scalars.is_disjoint(&second_scalars));
    assert!(BTreeSet::from(first.contacts).is_disjoint(&BTreeSet::from(second.contacts)));
    assert!(BTreeSet::from(first.tangencies).is_disjoint(&BTreeSet::from(second.tangencies)));
    assert_ne!(first.radius_dimension, second.radius_dimension);
}

fn native_fillet_step(session: &LineageSession) -> LineageStepId {
    session
        .document()
        .steps()
        .iter()
        .find_map(|step| match &step.action {
            LineageActionDefinition::Operation { action }
                if action.schema.as_str()
                    == "geosolve.document-edit.v1.create-prepared-native-line-fillet-geometry" =>
            {
                Some(step.id)
            }
            _ => None,
        })
        .expect("native-Fillet lineage owner")
}

fn action_payload_mut(action: &mut LineageActionDefinition) -> &mut VersionedActionPayload {
    let LineageActionDefinition::Operation { action } = action else {
        panic!("native-Fillet owner must be an Operation action");
    };
    action
}

fn refresh_compiled_intent_digest(payload: &mut VersionedActionPayload) {
    let intent = payload
        .parameters
        .get("authored_intent")
        .expect("authored native-Fillet intent")
        .clone();
    let owner_fields = payload
        .parameters
        .get("authored_owner_fields")
        .expect("native-Fillet owner fields")
        .clone();
    let digest = lineage_content_digest(
        &serde_json::to_vec(&json!({
            "authored_intent": intent,
            "authored_owner_fields": owner_fields,
        }))
        .expect("canonical hostile intent bytes"),
    );
    let compiled = payload
        .parameters
        .get_mut("compiled_materialization")
        .and_then(Value::as_object_mut)
        .expect("compiled native-Fillet materialization");
    compiled.insert(
        "intent_digest".into(),
        serde_json::to_value(digest).expect("hostile intent digest"),
    );
}

#[derive(Clone, Copy, Debug)]
enum PreparedForgery {
    Request,
    Plan,
    ReservedIds,
    Topology,
}

fn forge_prepared_fillet(
    coordinator: &RetainedEditorCoordinator,
    forgery: PreparedForgery,
) -> (LineageSession, LineageStepId) {
    let mut session = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("canonical native-Fillet lineage"),
    )
    .expect("decoded native-Fillet lineage");
    let owner = native_fillet_step(&session);
    let step = session.document().step(owner).expect("native owner");
    let mut action = step.action.clone();
    let label = step.label.clone();
    let payload = action_payload_mut(&mut action);
    let prepared = payload
        .parameters
        .get_mut("authored_intent")
        .and_then(|intent| {
            intent.pointer_mut("/body/CreatePreparedNativeLineFilletGeometry/prepared")
        })
        .expect("prepared native-Fillet intent");
    match forgery {
        PreparedForgery::Request => {
            prepared["request"]["radius"] = json!(0.75);
        }
        PreparedForgery::Plan => {
            prepared["accepted_line_tangents"][0] = json!([0.6, 0.8]);
        }
        PreparedForgery::ReservedIds => {
            prepared["expected_ids"]["arc"] = prepared["expected_ids"]["source_lines"][0].clone();
        }
        PreparedForgery::Topology => {
            prepared["corner"] = prepared["expected_ids"]["contact_points"][0].clone();
        }
    }
    refresh_compiled_intent_digest(payload);
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            vec![LineageMutation::Rewrite {
                step: owner,
                replacement: Box::new(LineageStepRewrite { label, action }),
            }],
        ))
        .expect("structurally valid hostile native-Fillet rewrite");
    (session, owner)
}

#[test]
fn consecutive_native_fillets_replay_as_distinct_accepted_lineage_transitions() {
    let (document, corners) = two_corner_document();
    let mut coordinator = RetainedEditorCoordinator::new(retained(document)).expect("coordinator");
    let base_history = (coordinator.history_len(), coordinator.history_cursor());
    let first = publish_native_fillet(&mut coordinator, corners[0].corner, 0.5, "first native");
    let second = publish_native_fillet(&mut coordinator, corners[1].corner, 0.75, "second native");

    assert_eq!(first.source_lines, corners[0].lines);
    assert_eq!(second.source_lines, corners[1].lines);
    assert_created_ids_are_disjoint(&first, &second);
    assert_ids_exist(&coordinator, &first);
    assert_ids_exist(&coordinator, &second);
    assert_finite_accepted(&coordinator);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (base_history.0 + 2, base_history.1 + 2)
    );

    let strict = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("consecutive native-Fillet lineage"),
    )
    .expect("strict lineage");
    let strict_evidence =
        evaluate_lineage_session_cold(&strict).expect("strict consecutive-Fillet replay");
    assert_eq!(strict_evidence.validated_prefix_count(), 3);

    let mut local = strict.clone();
    local
        .apply_patch(LineagePatch::new(
            local.identity(),
            vec![LineageMutation::SetEvaluationPolicy {
                policy: LineageEvaluationPolicy::DependencyLocal,
            }],
        ))
        .expect("dependency-local policy");
    let local_evidence =
        evaluate_lineage_session_cold(&local).expect("local consecutive-Fillet replay");
    assert_eq!(
        local_evidence.sketch_digest(),
        strict_evidence.sketch_digest()
    );
    assert_eq!(
        local_evidence.feature_digest(),
        strict_evidence.feature_digest()
    );
    let strict_map = LineageMaterializationMap::derive(strict.document())
        .expect("strict consecutive-Fillet ownership");
    let local_map = LineageMaterializationMap::derive(local.document())
        .expect("local consecutive-Fillet ownership");
    assert_eq!(local_map.bindings(), strict_map.bindings());
    assert_eq!(local_map.reverse_bindings(), strict_map.reverse_bindings());
    assert_eq!(
        local_map.declared_reverse_bindings(),
        strict_map.declared_reverse_bindings()
    );
    assert_eq!(
        local_map.logical_reverse_bindings(),
        strict_map.logical_reverse_bindings()
    );

    coordinator.undo().expect("Undo second native Fillet");
    assert_ids_exist(&coordinator, &first);
    assert!(
        coordinator
            .session()
            .design_document()
            .curve(second.arc)
            .is_none()
    );
    assert_finite_accepted(&coordinator);
    coordinator.redo().expect("Redo second native Fillet");
    assert_ids_exist(&coordinator, &first);
    assert_ids_exist(&coordinator, &second);
    assert_finite_accepted(&coordinator);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one host-only authority case keeps action bytes, history, high-water, fallback and cold accepted geometry adjacent"
)]
fn host_only_radius_inputs_after_native_fillet_keep_action_and_identity_exact() {
    let (document, corners) = two_corner_document();
    let mut coordinator = RetainedEditorCoordinator::new(retained(document)).expect("coordinator");
    let ids = publish_native_fillet(&mut coordinator, corners[0].corner, 0.5, "host native");
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateParameter {
                label: "host native radius".into(),
                kind: DocumentParameterKind::Length,
            },
        )
        .expect("radius parameter");
    let parameter = coordinator
        .session()
        .design_document()
        .parameters()
        .iter()
        .find(|parameter| parameter.label == "host native radius")
        .expect("radius parameter identity")
        .id;
    let binding = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::AddParameterBinding {
                parameter,
                target: DocumentParameterTarget::DrivingDimension(ids.radius_dimension),
            },
        )
        .expect("native radius host binding");
    assert!(
        binding.published_accepted.is_none(),
        "the new binding has no host value until the host-only replacement"
    );
    let native_owner = native_fillet_step(
        &LineageSession::from_session_json(&coordinator.lineage_session_json().expect("lineage"))
            .expect("decoded lineage"),
    );
    let action_before = coordinator
        .lineage_document()
        .step(native_owner)
        .expect("native owner")
        .clone();
    let lineage_before = coordinator.lineage_identity();
    let history_before = (coordinator.history_len(), coordinator.history_cursor());
    let high_water_before = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();

    let current_batch = ParameterBatch::new(
        2,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Length(0.8),
        }],
    )
    .expect("replacement host radius");
    let outcome = coordinator
        .replace_parameter_batch(
            coordinator.session().design_identity(),
            current_batch.clone(),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        )
        .expect("host-only radius update");
    assert!(outcome.published_accepted.is_some());
    assert_eq!(coordinator.lineage_identity(), lineage_before);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history_before,
        "host-only inputs must not create user lineage history"
    );
    assert_eq!(
        coordinator
            .lineage_document()
            .step(native_owner)
            .expect("unchanged native owner"),
        &action_before
    );
    assert_eq!(
        coordinator.session().persistent_identity_high_water(),
        &high_water_before
    );
    assert_ids_exist(&coordinator, &ids);
    assert_finite_accepted(&coordinator);
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("host-current native Fillet");
    let accepted_radius = accepted
        .document()
        .scalar(ids.radius)
        .expect("accepted solved radius")
        .value;
    assert!(
        (accepted_radius - 0.8).abs() <= 1.0e-8,
        "host radius={accepted_radius}"
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(ids.radius_target)
            .expect("retained radius fallback")
            .value
            .to_bits(),
        0.5_f64.to_bits(),
        "host-only effective input must not overwrite the authored fallback"
    );

    let lineage = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("host-current lineage"),
    )
    .expect("decoded host-current lineage");
    let cold = evaluate_lineage_session_cold_with_inputs(
        &lineage,
        &current_batch,
        &geosolve_sketch::ExternalSnapshotSet::default(),
    )
    .expect("cold replay under the current host-only radius");
    assert_eq!(cold.validated_prefix_count(), 4);
    let lineage_json = coordinator
        .lineage_session_json()
        .expect("host-current lineage JSON");
    let ledger_json = coordinator
        .lineage_host_input_ledger_json()
        .expect("host-current input ledger");
    let cold = RetainedEditorCoordinator::lineage_cold_current_accepted_evidence_checkpoint(
        &lineage_json,
        Some(&ledger_json),
        &current_batch,
        &geosolve_sketch::ExternalSnapshotSet::default(),
    )
    .expect("cold host-only accepted authority");
    let live_accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("live host-only accepted authority");
    let live_accepted_json = if cold.accepted_uses_draft_v5() {
        live_accepted.document().to_draft_v5_json()
    } else {
        live_accepted.document().to_canonical_json()
    }
    .expect("encode live host-only accepted authority");
    assert_eq!(
        cold.accepted_json(),
        Some(live_accepted_json.as_str()),
        "host-only publication must expose the exact strict-cold accepted geometry"
    );
}

#[test]
fn forged_native_fillet_request_plan_ids_and_topology_never_replace_accepted_authority() {
    let (document, corners) = two_corner_document();
    let mut coordinator = RetainedEditorCoordinator::new(retained(document)).expect("coordinator");
    let ids = publish_native_fillet(&mut coordinator, corners[0].corner, 0.5, "forgery native");
    assert_ids_exist(&coordinator, &ids);
    assert_finite_accepted(&coordinator);
    let live_lineage = coordinator
        .lineage_session_json()
        .expect("live native-Fillet lineage");
    let accepted_json = coordinator
        .session()
        .export_accepted_json()
        .expect("accepted native-Fillet document");
    let history = (coordinator.history_len(), coordinator.history_cursor());
    let high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();

    for forgery in [
        PreparedForgery::Request,
        PreparedForgery::Plan,
        PreparedForgery::ReservedIds,
        PreparedForgery::Topology,
    ] {
        let (hostile, owner) = forge_prepared_fillet(&coordinator, forgery);
        let accepted_before = hostile
            .last_accepted()
            .cloned()
            .expect("retained genuine accepted authority");
        let accepted_document_before = hostile
            .last_accepted_document()
            .cloned()
            .expect("retained genuine accepted lineage");
        let failure = evaluate_lineage_session_cold(&hostile)
            .expect_err("forged prepared native Fillet must fail closed");
        assert_eq!(failure.code(), "sketch_evaluation_error", "{forgery:?}");
        assert_eq!(failure.failed_step(), Some(owner), "{forgery:?}");
        assert_eq!(hostile.last_accepted(), Some(&accepted_before));
        assert_eq!(
            hostile.last_accepted_document(),
            Some(&accepted_document_before)
        );
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged live lineage"),
            live_lineage,
            "{forgery:?} changed the live retained coordinator"
        );
        assert_eq!(
            coordinator.session().export_accepted_json().unwrap(),
            accepted_json,
            "{forgery:?} changed accepted geometry"
        );
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            history
        );
        assert_eq!(
            coordinator.session().persistent_identity_high_water(),
            &high_water
        );
        assert_finite_accepted(&coordinator);
    }
}

#[test]
fn abandoned_native_fillet_ids_remain_retired_after_undo_and_divergence() {
    let (document, corners) = two_corner_document();
    let mut coordinator = RetainedEditorCoordinator::new(retained(document)).expect("coordinator");
    let first = publish_native_fillet(&mut coordinator, corners[0].corner, 0.5, "abandoned native");
    let committed_high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();
    coordinator.undo().expect("Undo first native Fillet");
    assert_eq!(
        coordinator.session().persistent_identity_high_water(),
        &committed_high_water,
        "Undo must retain every native-Fillet reservation"
    );
    assert!(coordinator.can_redo());

    let second = publish_native_fillet(
        &mut coordinator,
        corners[1].corner,
        0.75,
        "divergent native",
    );
    assert!(!coordinator.can_redo(), "the new Fillet must abandon Redo");
    assert_created_ids_are_disjoint(&first, &second);
    assert!(
        coordinator
            .session()
            .design_document()
            .curve(first.arc)
            .is_none(),
        "the abandoned Fillet must not remain materialized"
    );
    assert_ids_exist(&coordinator, &second);
    assert_eq!(
        committed_high_water
            .merged(coordinator.session().persistent_identity_high_water())
            .expect("same sketch namespace"),
        *coordinator.session().persistent_identity_high_water(),
        "the divergent branch must remain above every abandoned reservation"
    );
    assert_finite_accepted(&coordinator);

    let lineage = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("divergent native-Fillet lineage"),
    )
    .expect("decoded divergent lineage");
    let cold = evaluate_lineage_session_cold(&lineage)
        .expect("cold replay after native-Fillet history divergence");
    assert_eq!(cold.validated_prefix_count(), 2);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact restore plus Undo/Redo authority regression keeps the accepted native-Fillet bytes and lifecycle assertions adjacent"
)]
fn rejected_current_restore_and_history_publish_exact_older_accepted_native_fillet() {
    let (document, corners) = two_corner_document();
    let initial = retained(document);
    let mut coordinator =
        RetainedEditorCoordinator::new(initial.clone()).expect("live coordinator");
    let ids = publish_native_fillet(&mut coordinator, corners[0].corner, 0.5, "fallback native");
    let center = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted native Fillet")
        .document()
        .point(ids.center)
        .expect("accepted Fillet center")
        .position;
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateConstraint {
                label: "accepted center anchor".into(),
                definition: DocumentConstraintDefinition::FixedPoint {
                    point: ids.center,
                    target: center,
                },
            },
        )
        .expect("accepted center anchor");
    let accepted_before = coordinator
        .session()
        .export_accepted_json()
        .expect("accepted export")
        .expect("accepted native-Fillet bytes");
    let rejected = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateConstraint {
                label: "conflicting center anchor".into(),
                definition: DocumentConstraintDefinition::FixedPoint {
                    point: ids.center,
                    target: [center[0] + 1.0, center[1] + 1.0],
                },
            },
        )
        .expect("structurally valid conflicting anchor");
    assert!(rejected.published_accepted.is_none());
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("retained accepted export")
            .expect("older accepted bytes"),
        accepted_before
    );
    let mut expected_restored_accepted =
        SketchDocument::from_json(&accepted_before).expect("accepted native-Fillet document");
    expected_restored_accepted
        .retain_persistent_identity_high_water(
            coordinator.session().persistent_identity_high_water(),
        )
        .expect("accepted document retains rejected-action high-water");
    let expected_restored_accepted = expected_restored_accepted
        .to_canonical_json()
        .expect("canonical restored accepted bytes");

    let lineage_json = coordinator
        .lineage_session_json()
        .expect("rejected lineage session");
    let mut restored = RetainedEditorCoordinator::new(initial).expect("restore target");
    restored
        .restore_lineage_session_json(&lineage_json)
        .expect("cold rejected-current restore");
    assert_eq!(
        restored
            .session()
            .export_accepted_json()
            .expect("restored accepted export")
            .expect("restored older accepted bytes"),
        expected_restored_accepted,
        "restore may advance only allocator high-water over the exact cold-authenticated native-Fillet bytes"
    );
    assert!(
        restored
            .session()
            .accepted_state_for_current_input()
            .is_none()
    );
    assert_ids_exist(&restored, &ids);

    coordinator.undo().expect("Undo rejected anchor");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("Undo accepted export")
            .expect("Undo accepted bytes"),
        expected_restored_accepted
    );
    assert_finite_accepted(&coordinator);
    coordinator.redo().expect("Redo rejected anchor");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("Redo accepted export")
            .expect("Redo older accepted bytes"),
        expected_restored_accepted,
        "Redo must retain the exact authenticated fallback plus monotonic high-water rather than a second solve"
    );
    assert!(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .is_none()
    );
}
