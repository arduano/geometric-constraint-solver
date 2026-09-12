// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    reason = "exact retained authority and fixture coordinates"
)]
use geosolve_constraint_editor::{EditorScene, Viewport};
use geosolve_sketch_code::{CodeOwnerAddress, CodeProject, CompiledManagedSource, ProjectKey};
use geosolve_sketch_engine::{
    EditableSession, MAX_POINT_GESTURE_SAMPLES, PointGestureSample, PointGestureTarget,
};

fn project(name: &str) -> String {
    let json = match name {
        "circle" => include_str!("fixtures/authoring-radius-2.json"),
        "larger" => include_str!("fixtures/authoring-radius-5.json"),
        "shared" => include_str!("fixtures/point-gesture-shared.json"),
        "rectangle" => include_str!("fixtures/point-gesture-rectangle.json"),
        "constrained" => include_str!("fixtures/point-gesture-constrained.json"),
        "computed" => include_str!("fixtures/point-gesture-computed.json"),
        "parameter" => include_str!("fixtures/point-gesture-parameter.json"),
        "suppressed" => include_str!("fixtures/point-gesture-suppressed.json"),
        _ => panic!("unknown fixture"),
    };
    CodeProject::managed(
        ProjectKey("retained-gesture".into()),
        CompiledManagedSource::from_json(json).unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap()
}
fn viewport() -> Viewport {
    Viewport::new([800.0, 600.0], [0.0, 0.0], 10.0).unwrap()
}
fn sample(sequence: u32, position: [f64; 2]) -> PointGestureSample {
    PointGestureSample { sequence, position }
}
fn assert_near(actual: [f64; 2], expected: [f64; 2]) {
    for axis in 0..2 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 1e-8,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn retained_point_frames_use_native_continuation_without_materialization_or_history() {
    let session = EditableSession::open(&project("circle"), None).unwrap();
    let before = session.state();
    let target = session.point_gesture_targets().unwrap().remove(0);
    let mut gesture = session
        .begin_point_gesture(target.target.clone(), 7, viewport())
        .unwrap();
    for sequence in 1..=12 {
        let position = [f64::from(sequence), f64::from(sequence) * 0.5];
        let frame = gesture.advance(7, sample(sequence, position)).unwrap();
        assert!(frame.accepted);
        assert_near(frame.accepted_position, position);
        assert_eq!(frame.work.native_preview_attempts(), 1);
        assert_eq!(frame.work.intent_materialization_attempts(), 0);
        assert_eq!(frame.work.history_publications(), 0);
        assert_eq!(session.state(), before);
    }
    let detached = EditorScene::from_detached_json(&gesture.scene_json().unwrap()).unwrap();
    assert!(detached.points.iter().any(|point| {
        (point.model_position[0] - 12.0).abs() < 1e-8
            && (point.model_position[1] - 6.0).abs() < 1e-8
    }));
    let terminal = gesture.finish(7).unwrap();
    assert_near(terminal.accepted_position(), [12.0, 6.0]);
    assert_eq!(terminal.command().target, target.target);
    assert_eq!(terminal.command().samples.len(), 12);
    let replay = session.replay_point_gesture(terminal.command()).unwrap();
    assert_eq!(replay.accepted_position(), terminal.accepted_position());
    assert_eq!(session.state(), before);
}

#[test]
fn malformed_routing_order_nonfinite_and_generation_never_consume_valid_samples() {
    let session = EditableSession::open(&project("circle"), None).unwrap();
    let before = session.state();
    let target = session.point_gesture_targets().unwrap().remove(0).target;
    let mut stale = target.clone();
    let PointGestureTarget::Point { address } = &mut stale else {
        panic!("point target expected")
    };
    address.owner.generation += 1;
    assert!(session.begin_point_gesture(stale, 7, viewport()).is_err());
    assert!(
        session
            .begin_point_gesture(target.clone(), 0, viewport())
            .is_err()
    );
    let mut gesture = session.begin_point_gesture(target, 7, viewport()).unwrap();
    assert!(gesture.advance(8, sample(1, [2.0, 0.0])).is_err());
    assert!(gesture.advance(7, sample(2, [2.0, 0.0])).is_err());
    assert!(gesture.advance(7, sample(1, [f64::NAN, 0.0])).is_err());
    assert_eq!(gesture.sample_count(), 0);
    assert_eq!(gesture.accepted_position(), [0.0, 0.0]);
    gesture.advance(7, sample(1, [2.0, 0.0])).unwrap();
    assert!(gesture.advance(7, sample(1, [10.0, 0.0])).is_err());
    assert_eq!(gesture.sample_count(), 1);
    assert_near(gesture.accepted_position(), [2.0, 0.0]);
    gesture.cancel();
    assert_eq!(session.state(), before);
}

#[test]
fn explicit_referenced_consumer_drag_detaches_without_moving_producer() {
    let session = EditableSession::open(&project("shared"), None).unwrap();
    let before = session.state();
    let handles = session.point_gesture_targets().unwrap();
    let target = handles.iter().find(|handle| matches!(&handle.target, PointGestureTarget::Point { address } if matches!(&address.owner.address, CodeOwnerAddress::DirectDeclaration { declaration } if declaration.0 == "consumer"))).unwrap().target.clone();
    let mut gesture = session.begin_point_gesture(target, 17, viewport()).unwrap();
    let frame = gesture.advance(17, sample(1, [10.0, 4.0])).unwrap();
    assert!(frame.accepted);
    assert_near(frame.accepted_position, [10.0, 4.0]);
    let scene = EditorScene::from_detached_json(&gesture.scene_json().unwrap()).unwrap();
    assert!(
        scene
            .points
            .iter()
            .any(|point| point.model_position == [0.0, 0.0])
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| (point.model_position[0] - 10.0).abs() < 1e-8)
    );
    let terminal = gesture.finish(17).unwrap();
    let replay = session.replay_point_gesture(terminal.command()).unwrap();
    assert_eq!(replay.accepted_position(), terminal.accepted_position());
    assert_eq!(session.state(), before);
}

#[test]
fn rectangle_corner_target_remains_explicit_and_replay_rejects_changed_basis_or_bounds() {
    let mut session = EditableSession::open(&project("rectangle"), None).unwrap();
    let handle = session
        .point_gesture_targets()
        .unwrap()
        .into_iter()
        .find(|handle| handle.position == [20.0, 10.0])
        .unwrap();
    // Named geometry exposes its authored oppositeCorner through the existing point
    // codec. Older/custom rectangle expansions retain the compound corner codec.
    assert!(
        matches!(&handle.target, PointGestureTarget::Point { address }
        if matches!(&address.owner.address, CodeOwnerAddress::DirectDeclaration { declaration }
            if declaration.0 == "box"))
    );
    let before = session.state();
    let mut gesture = session
        .begin_point_gesture(handle.target, 27, viewport())
        .unwrap();
    let frame = gesture.advance(27, sample(1, [25.0, 15.0])).unwrap();
    assert!(frame.accepted);
    assert_eq!(frame.work.intent_materialization_attempts(), 0);
    assert_eq!(frame.work.history_publications(), 0);
    let terminal = gesture.finish(27).unwrap();
    assert_eq!(session.state(), before);
    let mut oversized = terminal.command().clone();
    oversized.samples = vec![sample(1, [1.0, 1.0]); MAX_POINT_GESTURE_SAMPLES + 1];
    assert!(session.replay_point_gesture(&oversized).is_err());
    let mut missing = terminal.command().clone();
    missing.samples[0].sequence = 2;
    assert!(session.replay_point_gesture(&missing).is_err());
    let token = session.token().clone();
    session.apply_project(&token, &project("circle")).unwrap();
    let after = session.state();
    assert!(session.replay_point_gesture(terminal.command()).is_err());
    assert_eq!(session.state(), after);
}

#[test]
fn a_gesture_without_accepted_movement_has_no_terminal_and_cannot_change_server_state() {
    let session = EditableSession::open(&project("circle"), None).unwrap();
    let before = session.state();
    let target = session.point_gesture_targets().unwrap().remove(0).target;
    let gesture = session
        .begin_point_gesture(target.clone(), 37, viewport())
        .unwrap();
    assert!(gesture.finish(37).is_err());
    let mut gesture = session.begin_point_gesture(target, 38, viewport()).unwrap();
    let frame = gesture.advance(38, sample(1, [0.01, 0.01])).unwrap();
    assert!(!frame.accepted);
    assert_eq!(frame.work.native_preview_attempts(), 0);
    assert!(gesture.finish(38).is_err());
    assert_eq!(session.state(), before);
}

#[test]
fn constrained_bar_continuation_preserves_independent_distance_and_orientation_at_every_frame() {
    let session = EditableSession::open(&project("constrained"), None).unwrap();
    let before = session.state();
    let target = session
        .point_gesture_targets()
        .unwrap()
        .into_iter()
        .find(|handle| handle.position == [0.0, 0.0])
        .unwrap()
        .target;
    let mut gesture = session.begin_point_gesture(target, 47, viewport()).unwrap();
    for sequence in 1..=8 {
        let frame = gesture
            .advance(
                47,
                sample(sequence, [f64::from(sequence), f64::from(sequence)]),
            )
            .unwrap();
        assert!(frame.accepted);
        assert_eq!(frame.work.native_preview_attempts(), 1);
        assert_eq!(frame.work.intent_materialization_attempts(), 0);
        assert_eq!(frame.work.history_publications(), 0);
        let scene = EditorScene::from_detached_json(&gesture.scene_json().unwrap()).unwrap();
        let mut points = scene
            .points
            .iter()
            .map(|point| point.model_position)
            .collect::<Vec<_>>();
        points.sort_by(|first, second| first[0].total_cmp(&second[0]));
        assert_eq!(points.len(), 2);
        assert!((points[1][0] - points[0][0] - 20.0).abs() < 1e-8);
        assert!((points[1][1] - points[0][1]).abs() < 1e-8);
        assert_near(points[0], frame.accepted_position);
    }
    let terminal = gesture.finish(47).unwrap();
    let replay = session.replay_point_gesture(terminal.command()).unwrap();
    assert_eq!(replay.accepted_position(), terminal.accepted_position());
    assert_eq!(session.state(), before);
}

fn command(
    session: &EditableSession,
    origin: [f64; 2],
    target: [f64; 2],
) -> geosolve_sketch_engine::PointGestureCommand {
    let handle = session
        .point_gesture_targets()
        .unwrap()
        .into_iter()
        .find(|handle| handle.position == origin)
        .unwrap();
    let mut gesture = session
        .begin_point_gesture(handle.target, 61, viewport())
        .unwrap();
    for sequence in 1..=4 {
        let ratio = f64::from(sequence) / 4.0;
        let point = [
            origin[0] + (target[0] - origin[0]) * ratio,
            origin[1] + (target[1] - origin[1]) * ratio,
        ];
        gesture.advance(61, sample(sequence, point)).unwrap();
    }
    gesture.finish(61).unwrap().command().clone()
}
fn positions(result: &geosolve_sketch_engine::EngineAcceptedResult) -> Vec<[f64; 2]> {
    let mut points = result
        .geometry
        .points
        .iter()
        .map(|point| point.position)
        .collect::<Vec<_>>();
    points.sort_by(|left, right| {
        left[0]
            .total_cmp(&right[0])
            .then(left[1].total_cmp(&right[1]))
    });
    points
}

#[test]
fn server_point_commit_is_staged_cas_atomic_and_one_history_entry_with_exact_reconstruction() {
    for (name, origin, target) in [
        ("circle", [0.0, 0.0], [8.0, 4.0]),
        ("constrained", [0.0, 0.0], [8.0, 8.0]),
        ("rectangle", [20.0, 10.0], [25.0, 15.0]),
    ] {
        let json = project(name);
        let mut session = EditableSession::open(&json, None).unwrap();
        let before = session.state();
        let gesture = command(&session, origin, target);
        let prepared = session
            .prepare_point_gesture_commit(&gesture)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(session.state(), before);
        assert!(prepared.result().validation.hard_residuals_validated);
        let design = serde_json::to_string(prepared.design()).unwrap();
        let expected_positions = positions(prepared.result());
        let durable_project = session.export_project_json().unwrap();
        assert_eq!(durable_project, json);
        let restored = EditableSession::open(&durable_project, Some(&design)).unwrap();
        assert_eq!(
            restored.source_design_digest().unwrap(),
            prepared.source_design_digest()
        );
        if name == "rectangle" {
            // Source seeds stay exact; the two dependent rectangle corners may be
            // freshly recomputed within the existing 8-epsilon semantic seed cell.
            let cold = positions(&restored.state().result);
            assert_eq!(cold.len(), expected_positions.len());
            for point in &expected_positions {
                assert!(cold.iter().any(|candidate| (0..2).all(|axis| {
                    (candidate[axis] - point[axis]).abs() <= 8.0 * f64::EPSILON * 25.0
                })));
            }
        } else {
            assert_eq!(
                positions(&restored.state().result),
                expected_positions,
                "{name}: cold source/design reconstruction"
            );
        }
        let accepted = session.apply_point_gesture_commit(prepared).unwrap();
        assert_eq!(positions(accepted.result()), expected_positions);
        assert_eq!(session.export_project_json().unwrap(), durable_project);
        let cold_handles = restored.point_gesture_targets().unwrap();
        for handle in session.point_gesture_targets().unwrap() {
            assert_eq!(
                cold_handles
                    .iter()
                    .find(|cold| cold.target == handle.target)
                    .unwrap()
                    .position,
                handle.position,
                "{name}: every authored seed reconstructs exactly"
            );
        }
        assert_eq!(session.token().revision, before.token.revision + 1);
        let updated = session.state();
        assert!(session.commit_point_gesture(&gesture).is_err());
        assert_eq!(session.state(), updated);
        let token = session.token().clone();
        assert_eq!(session.undo(&token).unwrap().result(), &before.result);
        let token = session.token().clone();
        assert_eq!(session.redo(&token).unwrap().result(), accepted.result());
    }
}

#[test]
fn staged_point_commit_rejects_newer_session_and_preserves_its_complete_state() {
    let mut session = EditableSession::open(&project("circle"), None).unwrap();
    let gesture = command(&session, [0.0, 0.0], [5.0, 5.0]);
    let prepared = session.prepare_point_gesture_commit(&gesture).unwrap();
    let token = session.token().clone();
    session.apply_project(&token, &project("larger")).unwrap();
    let newer = session.state();
    assert!(session.apply_point_gesture_commit(prepared).is_err());
    assert_eq!(session.state(), newer);
}

#[test]
fn server_commit_preserves_detached_consumer_and_computed_fillet_current_geometry() {
    let json = project("shared");
    let mut session = EditableSession::open(&json, None).unwrap();
    let target = session.point_gesture_targets().unwrap().into_iter().find(|handle| matches!(&handle.target, PointGestureTarget::Point { address } if matches!(&address.owner.address, CodeOwnerAddress::DirectDeclaration { declaration } if declaration.0 == "consumer"))).unwrap().target;
    let mut gesture = session.begin_point_gesture(target, 63, viewport()).unwrap();
    gesture.advance(63, sample(1, [6.0, 4.0])).unwrap();
    let command = gesture.finish(63).unwrap().command().clone();
    let accepted = session.commit_point_gesture(&command).unwrap();
    assert_eq!(positions(accepted.result()), vec![[0.0, 0.0], [6.0, 4.0]]);
    let design = serde_json::to_string(&session.design()).unwrap();
    let restored = EditableSession::open(&json, Some(&design)).unwrap();
    assert_eq!(
        positions(&restored.state().result),
        positions(accepted.result())
    );

    let mut session = EditableSession::open(&project("computed"), None).unwrap();
    let before = session.state();
    assert!(!before.result.geometry.computed_edges.is_empty());
    let gesture = command_for_computed(&session);
    let prepared = session.prepare_point_gesture_commit(&gesture).unwrap();
    assert_eq!(session.state(), before);
    assert!(prepared.result().validation.all_active_features_current);
    assert!(!prepared.result().geometry.computed_edges.is_empty());
    let accepted = session.apply_point_gesture_commit(prepared).unwrap();
    assert!(accepted.result().validation.hard_residuals_validated);
    let token = session.token().clone();
    assert_eq!(session.undo(&token).unwrap().result(), &before.result);
}
fn command_for_computed(session: &EditableSession) -> geosolve_sketch_engine::PointGestureCommand {
    command(session, [20.0, 20.0], [22.0, 20.0])
}

#[test]
fn native_valid_but_infeasible_computed_terminal_cannot_publish_a_partial_scene() {
    let mut session = EditableSession::open(&project("computed"), None).unwrap();
    let before = session.state();
    let design = session.design();
    let mut bad = command_for_computed(&session);
    // The polyline remains native-valid, but its 1 mm outgoing leg cannot contain
    // the retained R2 Fillet with the original explicit normal sides/endpoints.
    bad.samples = vec![sample(1, [20.0, 1.0])];
    let native_terminal = session.replay_point_gesture(&bad).unwrap();
    assert_near(native_terminal.accepted_position(), [20.0, 1.0]);
    let failure = session.commit_point_gesture(&bad).unwrap_err().to_string();
    assert!(
        failure.contains("candidate evaluation rejected"),
        "{failure}"
    );
    assert_eq!(session.state(), before);
    assert_eq!(session.design(), design);
    assert!(!session.state().result.geometry.computed_edges.is_empty());
}

#[test]
fn trusted_latest_point_replay_preserves_scalar_edits_and_orders_same_point_writes() {
    let basis = EditableSession::open(&project("circle"), None).unwrap();
    let target = basis.point_gesture_targets().unwrap().remove(0).target;
    let mut gesture = basis
        .begin_point_gesture(target.clone(), 901, viewport())
        .unwrap();
    gesture.advance(901, sample(1, [12.0, 7.0])).unwrap();
    let terminal = gesture.finish(901).unwrap();
    let original_command = terminal.command().clone();
    let mut latest = EditableSession::open(&project("larger"), None).unwrap();
    assert!(
        latest
            .prepare_point_gesture_commit(&original_command)
            .is_err()
    );
    let mut first = latest
        .begin_point_gesture(
            latest.point_gesture_targets().unwrap().remove(0).target,
            902,
            viewport(),
        )
        .unwrap();
    first.advance(902, sample(1, [3.0, 4.0])).unwrap();
    let first = first.finish(902).unwrap();
    latest.commit_point_gesture(first.command()).unwrap();
    let before = latest.state();
    let (prepared, witness) = latest
        .prepare_point_gesture_replay(&basis, &original_command)
        .unwrap();
    assert_eq!(
        witness.required_stable_declarations,
        vec![geosolve_sketch_code::SemanticSymbol("bore".into())]
    );
    assert_eq!(latest.state(), before);
    assert!(prepared.result().validation.hard_residuals_validated);
    latest.apply_point_gesture_commit(prepared).unwrap();
    assert_near(
        latest.point_gesture_targets().unwrap().remove(0).position,
        [12.0, 7.0],
    );
    assert!(
        latest
            .accepted()
            .result()
            .geometry
            .scalars
            .iter()
            .any(|scalar| (scalar.value - 5.0).abs() < 1e-8)
    );
    assert_eq!(terminal.command(), &original_command);
    let accepted = latest.state();
    let mut forged = original_command.clone();
    let PointGestureTarget::Point { address } = &mut forged.target else {
        panic!()
    };
    address.owner.generation += 1;
    assert!(
        latest
            .prepare_point_gesture_replay(&basis, &forged)
            .is_err()
    );
    forged = original_command;
    forged.basis = "forged".into();
    assert!(
        latest
            .prepare_point_gesture_replay(&basis, &forged)
            .is_err()
    );
    assert_eq!(latest.state(), accepted);
}

#[test]
fn trusted_latest_point_replay_rejects_removed_source_owner_without_publication() {
    let basis = EditableSession::open(&project("circle"), None).unwrap();
    let mut gesture = basis
        .begin_point_gesture(
            basis.point_gesture_targets().unwrap().remove(0).target,
            903,
            viewport(),
        )
        .unwrap();
    gesture.advance(903, sample(1, [8.0, 4.0])).unwrap();
    let terminal = gesture.finish(903).unwrap();
    let latest = EditableSession::open(&project("rectangle"), None).unwrap();
    let before = latest.state();
    assert!(
        latest
            .prepare_point_gesture_replay(&basis, terminal.command())
            .is_err()
    );
    assert_eq!(latest.state(), before);
}

#[test]
fn latest_point_replay_retains_named_parameter_dependencies_with_distinct_lexical_names() {
    let basis = EditableSession::open(&project("parameter"), None).unwrap();
    let mut latest = EditableSession::open(&project("parameter"), None).unwrap();
    let mut gesture = basis
        .begin_point_gesture(
            basis.point_gesture_targets().unwrap().remove(0).target,
            905,
            viewport(),
        )
        .unwrap();
    gesture.advance(905, sample(1, [8.0, 4.0])).unwrap();
    let terminal = gesture.finish(905).unwrap();
    let before = latest.state();
    let (prepared, witness) = latest
        .prepare_point_gesture_replay(&basis, terminal.command())
        .unwrap();
    assert_eq!(
        witness
            .required_stable_declarations
            .iter()
            .map(|symbol| symbol.0.as_str())
            .collect::<Vec<_>>(),
        ["bore", "boreRadius"]
    );
    assert_eq!(latest.state(), before);
    assert!(prepared.result().validation.hard_residuals_validated);
    latest.apply_point_gesture_commit(prepared).unwrap();
    assert_near(
        latest.point_gesture_targets().unwrap().remove(0).position,
        [8.0, 4.0],
    );
    assert_eq!(
        latest.export_project_json().unwrap(),
        basis.export_project_json().unwrap()
    );
}

#[test]
fn explicitly_suppressed_geometry_has_no_advertised_point_gesture_targets() {
    let active = EditableSession::open(&project("circle"), None).unwrap();
    let target = active.point_gesture_targets().unwrap().remove(0).target;
    let suppressed = EditableSession::open(&project("suppressed"), None).unwrap();
    let before = suppressed.state();
    assert!(
        suppressed
            .accepted()
            .result()
            .validation
            .hard_residuals_validated
    );
    assert!(suppressed.accepted().result().geometry.points.is_empty());
    assert!(suppressed.point_gesture_targets().unwrap().is_empty());
    assert!(
        suppressed
            .begin_point_gesture(target, 1, viewport())
            .is_err()
    );
    assert_eq!(suppressed.state(), before);
    let restored = EditableSession::open(&project("circle"), None).unwrap();
    assert_eq!(
        restored.point_gesture_targets().unwrap().len(),
        active.point_gesture_targets().unwrap().len()
    );
}

#[test]
fn a_cold_polyline_corner_can_rotate_past_its_original_segment_hemisphere() {
    let json = CodeProject::managed(
        ProjectKey("cold-polyline-corner".into()),
        CompiledManagedSource::from_json(include_str!("fixtures/tool-operation-feature.json"))
            .unwrap(),
    )
    .unwrap()
    .to_canonical_json()
    .unwrap();
    // Exercise the observed workaround first, then the same corner gesture on
    // a cold document. Every leg remains nonzero throughout both paths.
    for move_endpoint_first in [true, false] {
        let mut session = EditableSession::open(&json, None).unwrap();
        if move_endpoint_first {
            let endpoint = command(&session, [220.0, 20.0], [222.0, 22.0]);
            session.commit_point_gesture(&endpoint).unwrap();
        }
        let before = session.state();
        let before_handles = session.point_gesture_targets().unwrap();
        let corner = before_handles
            .iter()
            .find(|handle| handle.position == [220.0, 0.0])
            .unwrap();
        let mut gesture = session
            .begin_point_gesture(corner.target.clone(), 71, viewport())
            .unwrap();
        for sequence in 1..=4 {
            let ratio = f64::from(sequence) / 4.0;
            let target = [220.0 - 5.0 * ratio, 21.0 * ratio];
            let frame = gesture.advance(71, sample(sequence, target)).unwrap();
            assert!(frame.accepted);
            assert_near(frame.accepted_position, target);
            let scene = EditorScene::from_detached_json(&gesture.scene_json().unwrap()).unwrap();
            assert!(
                scene
                    .points
                    .iter()
                    .all(|point| { point.model_position.into_iter().all(f64::is_finite) })
            );
        }
        let terminal = gesture.finish(71).unwrap();
        assert_near(terminal.accepted_position(), [215.0, 21.0]);
        assert_eq!(session.state(), before);
        let prepared = session
            .prepare_point_gesture_commit(terminal.command())
            .unwrap_or_else(|error| panic!("endpoint-first={move_endpoint_first}: {error}"));
        assert!(prepared.result().validation.hard_residuals_validated);
        assert_eq!(session.state(), before);
        let accepted = session.apply_point_gesture_commit(prepared).unwrap();
        for handle in session.point_gesture_targets().unwrap() {
            let expected = if handle.target == corner.target {
                [215.0, 21.0]
            } else {
                before_handles
                    .iter()
                    .find(|previous| previous.target == handle.target)
                    .unwrap()
                    .position
            };
            assert_near(handle.position, expected);
        }
        assert_eq!(session.export_project_json().unwrap(), json);
        let restored = EditableSession::open(
            &json,
            Some(&serde_json::to_string(&session.design()).unwrap()),
        )
        .unwrap();
        assert_eq!(
            positions(&restored.state().result),
            positions(accepted.result())
        );
        let token = session.token().clone();
        assert_eq!(session.undo(&token).unwrap().result(), &before.result);
        let token = session.token().clone();
        assert_eq!(session.redo(&token).unwrap().result(), accepted.result());
    }
}
