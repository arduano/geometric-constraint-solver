// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    reason = "override parity requires bit-exact authored and restored coordinates"
)]

use std::collections::BTreeSet;

use geosolve_constraint_editor::IntentNativeBinding;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeExpansionError, CodeProject, CodeProjectDemoId, ExpandedSemanticTarget,
    GeneratedMemberAddress, KeyedReconcileState, ManagedValue, MaterializedCodeProject,
    SketchCodeSession, bundled_code_project_demos, expand_code_project,
    materialize_code_project_cold, materialize_code_project_incremental,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentPortRef, IntentSession, IntentSessionId, NodeId};

fn rounded_project() -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .expect("rounded Polyline demo")
        .project()
}

fn rise_point() -> GeneratedMemberAddress {
    GeneratedMemberAddress::new("path", ["polyline", "vertex"], ["rise"], ["point"])
}

fn reconciled(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated member plan"),
            &BTreeSet::new(),
        )
        .expect("initial reconciliation")
        .into_staged()
}

fn accepted_point_position(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    address: &GeneratedMemberAddress,
    seed: u128,
) -> [f64; 2] {
    let materialized = materialize_code_project_cold(
        project,
        generated,
        IntentSessionId::from_raw(0x84_2000 + seed),
        DocumentId(PersistentId::from_u128(0x84_2000 + seed)),
        1.0,
    )
    .expect("cold native materialization");
    let provenance = materialized
        .expansion
        .generated_provenance
        .get(address)
        .expect("generated point provenance");
    let ExpandedSemanticTarget::Port { port } = &provenance.target else {
        panic!("generated Polyline vertex must lower to one ordinary point port")
    };
    let intent = materialized.editor.coordinator().intent();
    let node = intent
        .graph()
        .node_by_symbol(&port.alias)
        .expect("generated point declaration");
    let port = node
        .port_by_selector(port.selector)
        .expect("generated point port")
        .as_ref(node.id);
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted native authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    let IntentNativeBinding::Point(point) = accepted
        .ownership
        .port(port)
        .expect("generated point native ownership")
    else {
        panic!("generated Polyline point port must own a native point")
    };
    accepted
        .session
        .design_document()
        .point(point)
        .expect("accepted native point")
        .position
}

fn accepted_point_position_in(
    materialized: &MaterializedCodeProject,
    address: &GeneratedMemberAddress,
) -> [f64; 2] {
    let (_, port, IntentNativeBinding::Point(point)) =
        generated_native_identity(materialized, address)
    else {
        panic!("generated Polyline point must own one native point")
    };
    assert_eq!(port.kind, geosolve_sketch_intent::IntentPortKind::Point);
    materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .design_document()
        .point(point)
        .unwrap()
        .position
}

fn generated_native_identity(
    materialized: &MaterializedCodeProject,
    address: &GeneratedMemberAddress,
) -> (NodeId, IntentPortRef, IntentNativeBinding) {
    let provenance = materialized
        .expansion
        .generated_provenance
        .get(address)
        .unwrap();
    let ExpandedSemanticTarget::Port { port } = &provenance.target else {
        panic!("generated direct geometry must lower to one stable port")
    };
    let node = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&port.alias)
        .unwrap();
    let port = node
        .port_by_selector(port.selector)
        .unwrap()
        .as_ref(node.id);
    let native = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap();
    (node.id, port, native)
}

fn segment_branch_direction(
    materialized: &MaterializedCodeProject,
    address: &GeneratedMemberAddress,
) -> [f64; 2] {
    let (_, port, IntentNativeBinding::CurveSpan(span)) =
        generated_native_identity(materialized, address)
    else {
        panic!("generated segment must retain one stable native span")
    };
    assert_eq!(port.kind, geosolve_sketch_intent::IntentPortKind::CurveSpan);
    materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted native authority")
        .session
        .design_document()
        .curve_branch_direction(span)
        .expect("generated Polyline span must retain one explicit branch direction")
}

fn unit(vector: [f64; 2]) -> [f64; 2] {
    let norm = vector[0].hypot(vector[1]);
    [vector[0] / norm, vector[1] / norm]
}

fn assert_direction(actual: [f64; 2], expected: [f64; 2]) {
    assert!((actual[0] - expected[0]).abs() <= 1.0e-12);
    assert!((actual[1] - expected[1]).abs() <= 1.0e-12);
}

#[test]
fn point_override_moves_cold_native_geometry_and_reset_restores_code_position() {
    let project = rounded_project();
    let address = rise_point();
    let mut generated = reconciled(&project);
    assert_eq!(
        accepted_point_position(&project, &generated, &address, 0),
        [20.0, 0.0]
    );

    generated
        .set_override(
            &address,
            ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
        )
        .expect("supported point override");
    assert_eq!(
        accepted_point_position(&project, &generated, &address, 1),
        [22.0, 3.0]
    );

    assert!(generated.reset_to_code(&address).expect("reset override"));
    assert_eq!(
        accepted_point_position(&project, &generated, &address, 2),
        [20.0, 0.0]
    );
}

#[test]
fn unsupported_generated_output_override_rejects_before_native_publication() {
    let project = rounded_project();
    let mut generated = reconciled(&project);
    let span = generated
        .active()
        .keys()
        .find(|address| address.template == ["polyline", "segment"])
        .expect("generated Polyline span")
        .clone();
    generated
        .set_override(
            &span,
            ManagedValue::Array(vec![ManagedValue::Number(1.0), ManagedValue::Number(2.0)]),
        )
        .expect("generic ledger accepts bounded data before semantic lowering");
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_2100)).unwrap();
    assert!(matches!(
        expand_code_project(&project, &generated, intent.identity()),
        Err(CodeExpansionError::UnsupportedOverride { address })
            if address == span.display_path()
    ));
}

#[test]
fn point_override_warm_edit_changes_both_adjacent_branches_without_identity_churn() {
    let project = rounded_project();
    let address = rise_point();
    let start_span =
        GeneratedMemberAddress::new("path", ["polyline", "segment"], ["start"], ["span"]);
    let rise_span =
        GeneratedMemberAddress::new("path", ["polyline", "segment"], ["rise"], ["span"]);
    let base = reconciled(&project);
    let before = materialize_code_project_cold(
        &project,
        &base,
        IntentSessionId::from_raw(0x84_2300),
        DocumentId(PersistentId::from_u128(0x84_2300)),
        1.0,
    )
    .unwrap();
    let point_owner = generated_native_identity(&before, &address);
    let start_owner = generated_native_identity(&before, &start_span);
    let rise_owner = generated_native_identity(&before, &rise_span);
    let fillet_owners = before.host_outputs.clone();
    let feature_allocator = before
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .feature_lifecycle_high_water
        .allocator;
    assert_direction(segment_branch_direction(&before, &start_span), [1.0, 0.0]);
    assert_direction(
        segment_branch_direction(&before, &rise_span),
        unit([4.0, 12.0]),
    );

    let mut overridden = base.clone();
    overridden
        .set_override(
            &address,
            ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
        )
        .unwrap();
    let moved = materialize_code_project_incremental(&before, &project, &overridden).unwrap();
    assert_eq!(accepted_point_position_in(&moved, &address), [22.0, 3.0]);
    assert_eq!(generated_native_identity(&moved, &address), point_owner);
    assert_eq!(generated_native_identity(&moved, &start_span), start_owner);
    assert_eq!(generated_native_identity(&moved, &rise_span), rise_owner);
    assert_eq!(moved.host_outputs, fillet_owners);
    assert_eq!(
        moved
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .feature_lifecycle_high_water
            .allocator,
        feature_allocator,
    );
    assert_direction(
        segment_branch_direction(&moved, &start_span),
        unit([22.0, 3.0]),
    );
    assert_direction(
        segment_branch_direction(&moved, &rise_span),
        unit([2.0, 9.0]),
    );

    assert!(overridden.reset_to_code(&address).unwrap());
    let reset = materialize_code_project_incremental(&moved, &project, &overridden).unwrap();
    assert_eq!(accepted_point_position_in(&reset, &address), [20.0, 0.0]);
    assert_eq!(generated_native_identity(&reset, &address), point_owner);
    assert_eq!(generated_native_identity(&reset, &start_span), start_owner);
    assert_eq!(generated_native_identity(&reset, &rise_span), rise_owner);
    assert_eq!(reset.host_outputs, fillet_owners);
    assert_eq!(
        reset
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .feature_lifecycle_high_water
            .allocator,
        feature_allocator,
    );
    assert_direction(segment_branch_direction(&reset, &start_span), [1.0, 0.0]);
    assert_direction(
        segment_branch_direction(&reset, &rise_span),
        unit([4.0, 12.0]),
    );
}

#[test]
fn override_undo_redo_and_reload_preserve_geometry_and_generation_authority() {
    let project = rounded_project();
    let address = rise_point();
    let base = reconciled(&project);
    let identity = base.active()[&address];
    let initial_intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_2200)).unwrap();
    let initial_expansion =
        expand_code_project(&project, &base, initial_intent.identity()).unwrap();
    let mut session = SketchCodeSession::new_project(
        project.clone(),
        base,
        initial_expansion,
        serde_json::json!({"accepted": "code"}),
    )
    .unwrap();

    let override_value =
        ManagedValue::Array(vec![ManagedValue::Number(23.0), ManagedValue::Number(4.0)]);
    let mut overridden = session.snapshot().generated.clone();
    overridden
        .set_override(&address, override_value.clone())
        .unwrap();
    let override_intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_2201)).unwrap();
    let override_expansion =
        expand_code_project(&project, &overridden, override_intent.identity()).unwrap();
    let prepared = session
        .prepare_project_override(
            session.identity(),
            &address,
            override_value,
            override_expansion,
            serde_json::json!({"accepted": "override"}),
            "Move generated point",
        )
        .unwrap();
    session.apply_prepared(prepared).unwrap();
    assert_eq!(session.snapshot().generated.active()[&address], identity);
    assert_eq!(
        accepted_point_position(
            session.snapshot().accepted_code_project.as_ref().unwrap(),
            session.snapshot().accepted_generated.as_ref().unwrap(),
            &address,
            10,
        ),
        [23.0, 4.0]
    );

    let encoded = session.to_canonical_json().unwrap();
    let mut restored = SketchCodeSession::from_json(&encoded).unwrap();
    assert_eq!(restored.snapshot().generated.active()[&address], identity);
    assert_eq!(
        restored
            .snapshot()
            .generated
            .generation_high_water(&address),
        Some(identity.generation)
    );
    assert_eq!(
        accepted_point_position(
            restored.snapshot().accepted_code_project.as_ref().unwrap(),
            restored.snapshot().accepted_generated.as_ref().unwrap(),
            &address,
            11,
        ),
        [23.0, 4.0]
    );

    restored.undo().unwrap().expect("override Undo");
    assert_eq!(restored.snapshot().generated.active()[&address], identity);
    assert_eq!(
        accepted_point_position(
            restored.snapshot().accepted_code_project.as_ref().unwrap(),
            restored.snapshot().accepted_generated.as_ref().unwrap(),
            &address,
            12,
        ),
        [20.0, 0.0]
    );
    restored.redo().unwrap().expect("override Redo");
    assert_eq!(restored.snapshot().generated.active()[&address], identity);
    assert_eq!(
        accepted_point_position(
            restored.snapshot().accepted_code_project.as_ref().unwrap(),
            restored.snapshot().accepted_generated.as_ref().unwrap(),
            &address,
            13,
        ),
        [23.0, 4.0]
    );
}
