// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "exact prepared-patch identity and end-to-end gesture transactions are deliberately asserted in full"
)]

use geosolve_constraint_editor::{
    CoordinatorError, EditorEffect, EditorScene, Modifiers, PointerInput, ReplayAction,
    RetainedEditorCoordinator, SceneCurveControl, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DesignScalarId, DocumentArcSweep,
    DocumentCurveControlKind, DocumentCurveControlTarget, DocumentEdit, DocumentParameterKind,
    DocumentParameterTarget, DocumentRationalConicControl, DocumentScalarBranch,
    DocumentScalarPropertyRef, DocumentScalarUnit, DocumentSessionError, DocumentSolveRequest,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchLifecycleRevisionHighWater, SolverConfig,
};

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn base_scene(coordinator: &RetainedEditorCoordinator, viewport: Viewport) -> EditorScene {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .unwrap();
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.5,
    )
    .unwrap()
    .with_retained_session(coordinator.session())
    .unwrap()
}

fn prepared_preview_scene(
    coordinator: &RetainedEditorCoordinator,
    viewport: Viewport,
    interaction_revision: u64,
    interaction_design: geosolve_sketch::SketchDesignIdentity,
) -> EditorScene {
    let source = coordinator.visible_preview_session().unwrap();
    let accepted = source.accepted_state_for_current_input().unwrap();
    let mut scene = EditorScene::from_accepted_for_design(
        interaction_revision,
        interaction_design,
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.5,
    )
    .unwrap();
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .unwrap();
    scene
}

fn circle_fixture() -> (
    RetainedEditorCoordinator,
    EditorScene,
    SceneCurveControl,
    geosolve_sketch::DesignScalarId,
    Viewport,
) {
    let mut document = SketchDocument::new(6.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
    let mut scene = base_scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .unwrap();
    let control = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
        .unwrap()
        .clone();
    (coordinator, scene, control, radius, viewport)
}

fn rational_storage_bits(
    document: &SketchDocument,
    curve: CurveId,
    weight: DesignScalarId,
) -> ([u64; 2], u64) {
    let CurveDefinition::RationalQuadraticConic {
        weighted_middle, ..
    } = &document.curve(curve).unwrap().definition
    else {
        panic!("expected rational quadratic conic")
    };
    (
        weighted_middle.map(f64::to_bits),
        document.scalar(weight).unwrap().value.to_bits(),
    )
}

#[test]
fn unauthenticated_curve_control_callback_cannot_prepare_or_publish_a_preview() {
    let (mut coordinator, scene, control, radius, _) = circle_fixture();
    let before_input = coordinator.session().prepared_input();
    let effects = coordinator.resolve_curve_control_preview(
        7_001,
        9_001,
        scene.design_identity,
        control.id,
        [3.0, 0.0],
    );

    assert!(effects.is_empty());
    assert!(!coordinator.curve_control_preview_active());
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(coordinator.session().prepared_input(), before_input);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0
    );

    let pointer_id = 7_003;
    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected cancellable curve-control request: {request:?}");
    };
    for effect in coordinator.editor_mut().cancel() {
        coordinator.apply_editor_effect(&effect).unwrap();
    }
    assert!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                *request_id,
                *expected,
                *control,
                *model_position,
            )
            .is_empty()
    );
    assert!(!coordinator.curve_control_preview_active());
    assert!(coordinator.visible_preview_session().is_none());
}

#[test]
fn detached_or_mutated_same_design_scene_cannot_start_a_curve_control_gesture() {
    let (mut coordinator, scene, control, radius, _) = circle_fixture();
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .unwrap();
    let mut detached = EditorScene::from_accepted_for_design(
        scene.accepted_revision,
        scene.design_identity,
        accepted.document(),
        coordinator.session().design_document(),
        scene.viewport,
        0.5,
    )
    .unwrap();
    coordinator
        .editor()
        .populate_curve_controls(&mut detached)
        .unwrap();
    assert_eq!(detached.curve_controls, scene.curve_controls);
    assert!(
        coordinator
            .pointer_down(&detached, pointer(7_011, control.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    let detached_move = coordinator.editor_mut().pointer_move(
        &detached,
        pointer(
            7_011,
            ScreenPoint {
                x: control.screen_position.x + 50.0,
                y: control.screen_position.y,
            },
        ),
    );
    assert!(
        !detached_move
            .iter()
            .any(|effect| matches!(effect, EditorEffect::RequestCurveControlPreview { .. }))
    );

    let mut mutated = scene.clone();
    mutated
        .curve_controls
        .iter_mut()
        .find(|candidate| candidate.id == control.id)
        .unwrap()
        .accessible_name
        .push_str(" forged");
    assert!(
        coordinator
            .pointer_down(&mutated, pointer(7_012, control.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0
    );

    assert!(
        coordinator
            .pointer_down(&scene, pointer(7_013, control.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_some());
}

#[test]
fn detached_or_mutated_same_design_scene_cannot_start_a_curve_point_alias_gesture() {
    let (mut coordinator, scene, _, _, _) = circle_fixture();
    let before_history = coordinator.history_len();
    let alias = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Center)
        .unwrap()
        .clone();
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .unwrap();
    let mut detached = EditorScene::from_accepted_for_design(
        scene.accepted_revision,
        scene.design_identity,
        accepted.document(),
        coordinator.session().design_document(),
        scene.viewport,
        0.5,
    )
    .unwrap();
    coordinator
        .editor()
        .populate_curve_controls(&mut detached)
        .unwrap();
    assert_eq!(detached.curve_controls, scene.curve_controls);

    assert!(
        coordinator
            .pointer_down(&detached, pointer(7_021, alias.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    assert_eq!(coordinator.history_len(), before_history);
    let detached_move = coordinator.editor_mut().pointer_move(
        &detached,
        pointer(
            7_021,
            ScreenPoint {
                x: alias.screen_position.x + 50.0,
                y: alias.screen_position.y,
            },
        ),
    );
    assert!(
        !detached_move
            .iter()
            .any(|effect| matches!(effect, EditorEffect::RequestProjectedPointMove { .. }))
    );

    let DocumentCurveControlTarget::Point(alias_point) = alias.target else {
        panic!("center control must remain an ordinary point alias")
    };
    let mut mutated_point = scene.clone();
    mutated_point
        .points
        .iter_mut()
        .find(|candidate| candidate.id == alias_point)
        .unwrap()
        .model_position[0] += 1.0;
    assert!(
        coordinator
            .pointer_down(&mutated_point, pointer(7_022, alias.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    assert_eq!(coordinator.history_len(), before_history);

    let mut mutated_screen_point = scene.clone();
    mutated_screen_point
        .points
        .iter_mut()
        .find(|candidate| candidate.id == alias_point)
        .unwrap()
        .screen_position
        .x += 1.0;
    assert!(
        coordinator
            .pointer_down(&mutated_screen_point, pointer(7_023, alias.screen_position),)
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    assert_eq!(coordinator.history_len(), before_history);

    let mut mutated = scene.clone();
    mutated
        .curve_controls
        .iter_mut()
        .find(|candidate| candidate.id == alias.id)
        .unwrap()
        .accessible_name
        .push_str(" forged");
    assert!(
        coordinator
            .pointer_down(&mutated, pointer(7_024, alias.screen_position))
            .is_empty()
    );
    assert!(coordinator.editor().active_pointer_gesture().is_none());
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(coordinator.history_len(), before_history);

    assert!(
        coordinator
            .pointer_down(&scene, pointer(7_025, alias.screen_position))
            .is_empty()
    );
    assert_eq!(
        coordinator.editor().active_pointer_gesture(),
        Some(geosolve_constraint_editor::ActivePointerGesture {
            pointer_id: 7_025,
            kind: geosolve_constraint_editor::ActivePointerGestureKind::Point,
        })
    );
}

#[test]
fn foreign_out_of_order_and_current_invalid_samples_preserve_the_prior_valid_candidate() {
    let (mut coordinator, scene, control, radius, viewport) = circle_fixture();
    let pointer_id = 7_002;
    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));

    let first_position = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let first_request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, first_position));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id: first_request_id,
            expected,
            control: requested_control,
            model_position,
            ..
        },
    ] = first_request.as_slice()
    else {
        panic!("expected first curve-control request: {first_request:?}");
    };
    assert!(matches!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                *first_request_id,
                *expected,
                *requested_control,
                *model_position,
            )
            .as_slice(),
        [EditorEffect::PreviewCurveControl { .. }]
    ));
    assert_eq!(
        coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );

    assert!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id + 1,
                *first_request_id,
                *expected,
                *requested_control,
                [4.0, 0.0],
            )
            .is_empty()
    );
    assert_eq!(
        coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );

    let second_position = ScreenPoint {
        x: first_position.x + 25.0,
        y: first_position.y,
    };
    let second_request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, second_position));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id: second_request_id,
            expected,
            control: requested_control,
            ..
        },
    ] = second_request.as_slice()
    else {
        panic!("expected second curve-control request: {second_request:?}");
    };
    assert!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                *first_request_id,
                *expected,
                *requested_control,
                [4.0, 0.0],
            )
            .is_empty()
    );
    assert!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                *second_request_id,
                *expected,
                *requested_control,
                [0.0, 0.0],
            )
            .is_empty()
    );
    assert!(coordinator.curve_control_preview_active());
    assert_eq!(
        coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );

    let preview_scene = prepared_preview_scene(
        &coordinator,
        viewport,
        scene.accepted_revision,
        scene.design_identity,
    );
    assert_eq!(
        coordinator.editor_mut().pointer_up(
            &preview_scene,
            scene.design_identity,
            pointer(pointer_id, second_position),
        ),
        vec![EditorEffect::CommitCurveControl {
            expected: scene.design_identity,
            pointer_id,
            request_id: *first_request_id,
            control: *requested_control,
        }]
    );
}

#[test]
fn prepared_curve_control_preview_commits_exact_patch_as_one_history_step() {
    let (mut coordinator, scene, control, radius, viewport) = circle_fixture();
    let owner = control.owner;
    let origin_revision = scene.accepted_revision;
    let origin_design = scene.design_identity;
    let pointer_id = 77;
    assert!(
        coordinator
            .pointer_down(&scene, pointer(pointer_id, control.screen_position))
            .is_empty()
    );
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control: requested_control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected curve-control request: {request:?}");
    };
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *requested_control,
        *model_position,
    );
    assert!(matches!(
        acknowledgement.as_slice(),
        [EditorEffect::PreviewCurveControl { control, .. }] if *control == *requested_control
    ));
    assert!(coordinator.curve_control_preview_active());
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0,
        "the live design must remain unchanged before compare-and-swap release"
    );
    assert_eq!(
        coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );

    let preview_scene =
        prepared_preview_scene(&coordinator, viewport, origin_revision, origin_design);
    let release = coordinator.editor_mut().pointer_up(
        &preview_scene,
        origin_design,
        pointer(pointer_id, moved),
    );
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        panic!("expected exact prepared commit: {release:?}");
    };
    let before_history = coordinator.history_len();
    coordinator.apply_editor_effect(commit).unwrap().unwrap();
    assert_eq!(coordinator.history_len(), before_history + 1);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );
    coordinator.undo().unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0
    );
    coordinator.redo().unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );

    let saved = coordinator.persistence_checkpoint().unwrap();
    coordinator.undo().unwrap();
    coordinator.reload(&saved).unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0,
        "the ordinary checkpoint path must retain the committed scalar edit"
    );
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(owner)]);
    let mut reloaded_scene = base_scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut reloaded_scene)
        .unwrap();
    let reloaded_control = reloaded_scene
        .curve_controls
        .iter()
        .find(|candidate| candidate.id == *requested_control)
        .expect("recomputed radius control after reload");
    assert_eq!(reloaded_control.model_position, [3.0, 0.0]);
}

#[test]
fn curve_control_inverse_projection_uses_the_accepted_not_unsolved_design_geometry() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let mut accepted = document.clone();
    accepted.set_point_position(center, [1.0, 1.0]).unwrap();
    let session = RetainedSketchDocumentSession::restore_current_design_with_accepted(
        document,
        accepted,
        SketchLifecycleRevisionHighWater::from_raw(22, 22, Some(22)),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(
        session.design_document().point(center).unwrap().position,
        [0.0, 0.0]
    );
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .point(center)
            .unwrap()
            .position,
        [1.0, 1.0]
    );
    let probe = session
        .prepared_snapshot()
        .prepare(geosolve_sketch::PreparedSketchOperation::Apply(
            DocumentEdit::SetScalarValue {
                scalar: radius,
                value: 3.0,
            },
        ))
        .execute(geosolve_sketch::OperationControl::unlimited())
        .unwrap();
    let geosolve_sketch::OperationOutcome::Completed { value: probe, .. } = probe else {
        panic!("unlimited scalar-edit probe must complete")
    };
    assert!(
        probe.proposed_commit().accepted_state_identity().is_some(),
        "ordinary scalar editing must be independently accepted before testing coordinator projection"
    );

    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
    let mut scene = base_scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .unwrap();
    let radius_control = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
        .unwrap()
        .clone();
    assert_eq!(radius_control.model_position, [3.0, 1.0]);

    let pointer_id = 76;
    coordinator.pointer_down(&scene, pointer(pointer_id, radius_control.screen_position));
    let moved = viewport.model_to_screen([4.0, 1.0]);
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected accepted-domain request: {request:?}")
    };
    assert_eq!(*model_position, [4.0, 1.0]);
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    assert_eq!(
        acknowledgement,
        vec![EditorEffect::PreviewCurveControl {
            control: *control,
            model_position: [4.0, 1.0],
        }]
    );
    assert_eq!(
        coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .scalar(radius)
            .unwrap()
            .value,
        3.0
    );
}

#[test]
fn positive_negative_and_projective_rational_middle_gestures_commit_one_exact_history_step() {
    for (case, weight) in [("positive", 0.5), ("negative", -0.5), ("projective", 0.0)] {
        let mut document = SketchDocument::new(6.0).unwrap();
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [4.0, 0.0]).unwrap();
        let weight_id = document
            .add_scalar(
                "weight",
                weight,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            )
            .unwrap();
        let weighted_middle = if weight == 0.0 {
            [1.0, 2.0]
        } else {
            [weight, 2.0 * weight]
        };
        let curve = document
            .add_curve(
                format!("{case} rational"),
                CurveDefinition::RationalQuadraticConic {
                    start,
                    weighted_middle,
                    middle_weight: weight_id,
                    end,
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
        let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
        let mut scene = base_scene(&coordinator, viewport);
        coordinator
            .editor()
            .populate_curve_controls(&mut scene)
            .unwrap();
        let control = scene
            .curve_controls
            .iter()
            .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
            .unwrap()
            .clone();
        let origin = coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .unwrap();
        let target = [2.0, 3.0];
        let target_screen = viewport.model_to_screen(target);
        let pointer_id = 90;
        coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(pointer_id, target_screen));
        let [
            EditorEffect::RequestCurveControlPreview {
                request_id,
                expected,
                control,
                model_position,
                ..
            },
        ] = request.as_slice()
        else {
            panic!("{case} rational gesture did not request a preview: {request:?}")
        };
        assert_eq!(*model_position, target, "{case}");
        assert_eq!(
            coordinator.resolve_curve_control_preview(
                pointer_id,
                *request_id,
                *expected,
                *control,
                *model_position,
            ),
            vec![EditorEffect::PreviewCurveControl {
                control: *control,
                model_position: target,
            }],
            "{case}"
        );
        let preview_control = coordinator
            .visible_preview_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .rational_conic_control(curve)
            .unwrap();
        let expected_control = if weight == 0.0 {
            DocumentRationalConicControl::Projective {
                weighted_middle: target,
                weight,
            }
        } else {
            DocumentRationalConicControl::Euclidean {
                middle: target,
                weight,
            }
        };
        assert_eq!(preview_control, expected_control, "{case}");

        let preview_scene = prepared_preview_scene(
            &coordinator,
            viewport,
            scene.accepted_revision,
            scene.design_identity,
        );
        let release = coordinator.editor_mut().pointer_up(
            &preview_scene,
            scene.design_identity,
            pointer(pointer_id, target_screen),
        );
        let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
            panic!("{case} rational gesture did not release the prepared candidate: {release:?}")
        };
        let before_history = coordinator.history_len();
        coordinator.apply_editor_effect(commit).unwrap().unwrap();
        assert_eq!(coordinator.history_len(), before_history + 1, "{case}");
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .rational_conic_control(curve)
                .unwrap(),
            expected_control,
            "{case}"
        );
        coordinator.undo().unwrap();
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .rational_conic_control(curve)
                .unwrap(),
            origin,
            "{case} Undo"
        );
        coordinator.redo().unwrap();
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .rational_conic_control(curve)
                .unwrap(),
            expected_control,
            "{case} Redo"
        );
    }
}

#[test]
fn m77_f009_spatial_rational_middle_uses_effective_weight_without_mutating_fallback() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let domain = ScalarDomain::Bounded {
        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    };
    let weight = document
        .add_scalar("stored fallback weight", 0.5, ScalarUnit::Parameter, domain)
        .unwrap();
    let original_weighted_middle = [1.6, 2.4];
    let curve = document
        .add_curve(
            "host-weighted rational",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: original_weighted_middle,
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    let parameter = document
        .add_parameter("host weight", DocumentParameterKind::Dimensionless)
        .unwrap();
    document
        .add_parameter_binding(
            parameter,
            DocumentParameterTarget::DimensionlessFixedScalar(DocumentScalarPropertyRef {
                scalar: weight,
                unit: DocumentScalarUnit::Dimensionless,
                domain,
                branch: DocumentScalarBranch::Dimensionless,
            }),
        )
        .unwrap();
    let host_weight = 0.8;
    let batch = ParameterBatch::new(
        19,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Dimensionless(host_weight),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let replay_baseline = session.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);

    let fallback_bits = 0.5_f64.to_bits();
    let original_storage = (original_weighted_middle.map(f64::to_bits), fallback_bits);
    assert_eq!(
        rational_storage_bits(coordinator.session().design_document(), curve, weight),
        original_storage,
    );
    assert_eq!(
        coordinator.session().parameter_batch().entries()[0].value,
        ParameterValue::Dimensionless(host_weight),
        "the immutable host input owns the exact effective weight",
    );
    let accepted_effective_weight = coordinator
        .session()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .scalar(weight)
        .unwrap()
        .value;
    assert!(
        (accepted_effective_weight - host_weight).abs() <= 1.0e-9,
        "the independently accepted solve must satisfy the host weight",
    );

    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
    let mut scene = base_scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .unwrap();
    let control = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
        .unwrap()
        .clone();
    assert!((control.model_position[0] - 2.0).abs() <= 1.0e-9);
    assert!((control.model_position[1] - 3.0).abs() <= 1.0e-9);

    let target = [2.5, 3.5];
    let target_screen = viewport.model_to_screen(target);
    let pointer_id = 9_009;
    assert!(
        coordinator
            .pointer_down(&scene, pointer(pointer_id, control.screen_position))
            .is_empty()
    );
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, target_screen));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control: requested_control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("host-effective rational gesture did not request a preview: {request:?}")
    };
    assert!((model_position[0] - target[0]).abs() <= 1.0e-12);
    assert!((model_position[1] - target[1]).abs() <= 1.0e-12);
    let requested_middle = *model_position;
    let expected_weighted_middle = [
        accepted_effective_weight * requested_middle[0],
        accepted_effective_weight * requested_middle[1],
    ];
    let expected_storage = (expected_weighted_middle.map(f64::to_bits), fallback_bits);
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *requested_control,
        *model_position,
    );
    let [
        EditorEffect::PreviewCurveControl {
            control: acknowledged_control,
            model_position: acknowledged_position,
        },
    ] = acknowledgement.as_slice()
    else {
        panic!("host-effective rational gesture did not accept its preview: {acknowledgement:?}")
    };
    assert_eq!(*acknowledged_control, *requested_control);
    assert!((acknowledged_position[0] - target[0]).abs() <= 1.0e-9);
    assert!((acknowledged_position[1] - target[1]).abs() <= 1.0e-9);
    assert_eq!(
        rational_storage_bits(coordinator.session().design_document(), curve, weight),
        original_storage,
        "previews must not mutate the retained design",
    );
    let preview_session = coordinator.visible_preview_session().unwrap();
    assert_eq!(
        rational_storage_bits(preview_session.design_document(), curve, weight),
        expected_storage,
        "the prepared durable edit must retain the fallback before publication",
    );
    let preview = preview_session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let DocumentRationalConicControl::Euclidean {
        middle: preview_middle,
        weight: preview_weight,
    } = preview.rational_conic_control(curve).unwrap()
    else {
        panic!("nonzero host weight must retain Euclidean middle mode")
    };
    assert!((preview_middle[0] - target[0]).abs() <= 1.0e-9);
    assert!((preview_middle[1] - target[1]).abs() <= 1.0e-9);
    assert!((preview_weight - host_weight).abs() <= 1.0e-9);

    let preview_scene = prepared_preview_scene(
        &coordinator,
        viewport,
        scene.accepted_revision,
        scene.design_identity,
    );
    let release = coordinator.editor_mut().pointer_up(
        &preview_scene,
        scene.design_identity,
        pointer(pointer_id, target_screen),
    );
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        panic!("host-effective rational gesture did not release its prepared patch: {release:?}")
    };
    coordinator.apply_editor_effect(commit).unwrap().unwrap();
    assert_eq!(
        rational_storage_bits(coordinator.session().design_document(), curve, weight),
        expected_storage,
        "a spatial control edit must retain the stored fallback weight bit-for-bit",
    );
    let [action] = coordinator.transcript() else {
        panic!("one spatial gesture must record exactly one action")
    };
    assert!(matches!(
        action,
        ReplayAction::Edit {
            edit: DocumentEdit::SetConicWeightedMiddle {
                curve: action_curve,
                weighted_middle,
            },
            ..
        } if *action_curve == curve
            && weighted_middle.map(f64::to_bits) == expected_weighted_middle.map(f64::to_bits)
    ));
    let action = action.clone();
    let checkpoint = coordinator.persistence_checkpoint().unwrap();

    coordinator.undo().unwrap();
    assert_eq!(
        rational_storage_bits(coordinator.session().design_document(), curve, weight),
        original_storage,
    );
    coordinator.redo().unwrap();
    assert_eq!(
        rational_storage_bits(coordinator.session().design_document(), curve, weight),
        expected_storage,
    );

    let mut replayed = RetainedEditorCoordinator::new(replay_baseline.clone()).unwrap();
    replayed.replay(&action).unwrap();
    assert_eq!(
        rational_storage_bits(replayed.session().design_document(), curve, weight),
        expected_storage,
        "replay must retain the stored fallback weight",
    );
    let DocumentRationalConicControl::Euclidean {
        middle: replayed_middle,
        weight: replayed_weight,
    } = replayed
        .session()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .rational_conic_control(curve)
        .unwrap()
    else {
        panic!("replay changed the rational control mode")
    };
    assert!((replayed_middle[0] - target[0]).abs() <= 1.0e-9);
    assert!((replayed_middle[1] - target[1]).abs() <= 1.0e-9);
    assert!((replayed_weight - host_weight).abs() <= 1.0e-9);

    let mut restored = RetainedEditorCoordinator::new(replay_baseline).unwrap();
    restored.reload(&checkpoint).unwrap();
    assert_eq!(
        rational_storage_bits(restored.session().design_document(), curve, weight),
        expected_storage,
        "checkpoint restore must retain the stored fallback weight",
    );
    let DocumentRationalConicControl::Euclidean {
        middle: restored_middle,
        weight: restored_weight,
    } = restored
        .session()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .rational_conic_control(curve)
        .unwrap()
    else {
        panic!("checkpoint restore changed the rational control mode")
    };
    assert!((restored_middle[0] - target[0]).abs() <= 1.0e-9);
    assert!((restored_middle[1] - target[1]).abs() <= 1.0e-9);
    assert!((restored_weight - host_weight).abs() <= 1.0e-9);
}

#[test]
fn cancel_discards_prepared_curve_control_state_without_consuming_durable_allocators() {
    let (mut coordinator, scene, control, radius, _) = circle_fixture();
    let before_input = coordinator.session().prepared_input();
    let before_history = coordinator.history_len();
    let before_transcript = coordinator.transcript().len();
    let before_evaluation_high_water = coordinator
        .persistence_checkpoint()
        .unwrap()
        .computed_evaluation_high_water();
    let pointer_id = 78;

    assert!(
        coordinator
            .pointer_down(&scene, pointer(pointer_id, control.screen_position))
            .is_empty()
    );
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected curve-control request: {request:?}");
    };
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    assert!(matches!(
        acknowledgement.as_slice(),
        [EditorEffect::PreviewCurveControl { .. }]
    ));

    let cancellation = coordinator.editor_mut().cancel();
    assert_eq!(cancellation, vec![EditorEffect::ClearCurveControlPreview]);
    for effect in cancellation {
        assert!(coordinator.apply_editor_effect(&effect).unwrap().is_none());
    }

    assert!(!coordinator.curve_control_preview_active());
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(coordinator.session().prepared_input(), before_input);
    assert_eq!(coordinator.history_len(), before_history);
    assert_eq!(coordinator.transcript().len(), before_transcript);
    assert_eq!(
        coordinator
            .persistence_checkpoint()
            .unwrap()
            .computed_evaluation_high_water(),
        before_evaluation_high_water
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0
    );
}

#[test]
fn selection_replacement_immediately_revokes_the_prepared_curve_control_scene() {
    let (mut coordinator, scene, control, radius, _) = circle_fixture();
    let before_history = coordinator.history_len();
    let pointer_id = 7_804;
    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected curve-control request: {request:?}");
    };
    assert!(matches!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                *request_id,
                *expected,
                *control,
                *model_position,
            )
            .as_slice(),
        [EditorEffect::PreviewCurveControl { .. }]
    ));
    assert!(coordinator.visible_preview_session().is_some());

    coordinator.set_selection([]);

    assert!(coordinator.visible_preview_session().is_none());
    assert!(!coordinator.curve_control_preview_active());
    assert_eq!(coordinator.history_len(), before_history);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0,
        "selection replacement must restore the exact retained scene"
    );
    assert!(
        coordinator
            .editor_mut()
            .pointer_up(&scene, scene.design_identity, pointer(pointer_id, moved))
            .is_empty(),
        "the old pointer release must have no publication authority"
    );
}

#[test]
fn retained_state_change_rejects_an_older_prepared_curve_control_release() {
    let (mut coordinator, scene, control, radius, viewport) = circle_fixture();
    let pointer_id = 79;
    let origin_design = scene.design_identity;
    let origin_revision = scene.accepted_revision;

    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected curve-control request: {request:?}");
    };
    coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    let preview_scene =
        prepared_preview_scene(&coordinator, viewport, origin_revision, origin_design);
    let release = coordinator.editor_mut().pointer_up(
        &preview_scene,
        origin_design,
        pointer(pointer_id, moved),
    );
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        panic!("expected exact prepared commit: {release:?}");
    };

    coordinator
        .apply_edit(
            origin_design,
            DocumentEdit::SetScalarValue {
                scalar: radius,
                value: 2.5,
            },
        )
        .unwrap();
    let history_after_newer_edit = coordinator.history_len();
    assert!(matches!(
        coordinator.apply_editor_effect(commit),
        Err(CoordinatorError::Session(
            DocumentSessionError::StaleDesign { .. }
        ))
    ));
    assert_eq!(coordinator.history_len(), history_after_newer_edit);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.5
    );
    assert!(!coordinator.curve_control_preview_active());
}

#[test]
fn trim_drag_back_to_the_same_parameter_is_history_neutral() {
    let mut document = SketchDocument::new(6.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let start = document
        .add_scalar("start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let end = document
        .add_scalar(
            "end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(arc))]);
    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).unwrap();
    let mut scene = base_scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .unwrap();
    let control = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::TrimStart)
        .unwrap()
        .clone();
    let before_history = coordinator.history_len();
    let pointer_id = 80;

    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
    let radial_move = scene.viewport.model_to_screen([3.0, 0.0]);
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, radial_move));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected radial trim request: {request:?}");
    };
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    assert!(matches!(
        acknowledgement.as_slice(),
        [EditorEffect::PreviewCurveControl { .. }]
    ));
    assert!(!coordinator.curve_control_preview_active());
    assert!(coordinator.visible_preview_session().is_none());

    let release = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(pointer_id, radial_move),
    );
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        panic!("expected authenticated no-op release: {release:?}");
    };
    assert!(coordinator.apply_editor_effect(commit).unwrap().is_none());
    assert_eq!(coordinator.history_len(), before_history);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(start)
            .unwrap()
            .value,
        0.0
    );
}

#[test]
fn mismatched_release_revokes_the_non_authoritative_preview() {
    let (mut coordinator, scene, control, radius, viewport) = circle_fixture();
    let pointer_id = 81;
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    coordinator.pointer_down(&scene, pointer(pointer_id, control.screen_position));
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = request.as_slice()
    else {
        panic!("expected curve-control request: {request:?}");
    };
    coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    let preview_scene = prepared_preview_scene(
        &coordinator,
        viewport,
        scene.accepted_revision,
        scene.design_identity,
    );
    let release = coordinator.editor_mut().pointer_up(
        &preview_scene,
        scene.design_identity,
        pointer(pointer_id, moved),
    );
    let [
        EditorEffect::CommitCurveControl {
            expected,
            pointer_id,
            request_id,
            control,
        },
    ] = release.as_slice()
    else {
        panic!("expected exact prepared commit: {release:?}");
    };
    let mismatched = EditorEffect::CommitCurveControl {
        expected: *expected,
        pointer_id: *pointer_id,
        request_id: request_id + 1,
        control: *control,
    };
    let before_history = coordinator.history_len();
    assert!(matches!(
        coordinator.apply_editor_effect(&mismatched),
        Err(CoordinatorError::SolvedPreviewMismatch)
    ));
    assert!(!coordinator.curve_control_preview_active());
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(coordinator.history_len(), before_history);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .unwrap()
            .value,
        2.0
    );
}
