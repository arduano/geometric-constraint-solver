// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionProposal, EditorEffect, EditorScene, Modifiers, PointerInput,
    RetainedEditorCoordinator, ScreenPoint, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentId,
    DocumentSolveRequest, PersistentId, RetainedSketchDocumentSession, SketchAcceptedDocumentState,
    SketchDocument, SolverConfig,
};

fn assert_rectangle_is_finite_and_has_four_dof(accepted: &SketchAcceptedDocumentState) {
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let rank = accepted.solve_result().unstable_core_report();
    assert!(rank.rank_is_valid);
    assert_eq!(rank.rank, 4);
    assert_eq!(rank.right_nullity, 4);
}

fn accepted_positions(
    coordinator: &RetainedEditorCoordinator,
    points: [DesignPointId; 4],
) -> [[f64; 2]; 4] {
    let document = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted rectangle")
        .document();
    points.map(|point| document.point(point).expect("rectangle point").position)
}

fn assert_positions_close(actual: [[f64; 2]; 4], expected: [[f64; 2]; 4]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 1.0e-9,
            "actual {actual:?}, expected {expected:?}"
        );
    }
}

fn visible_scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
    let visible_session = coordinator
        .solved_preview_session()
        .unwrap_or_else(|| coordinator.session());
    let accepted = visible_session
        .accepted_state_for_current_input()
        .expect("visible accepted rectangle");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        visible_session.design_document(),
        Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).expect("viewport"),
        0.5,
    )
    .expect("rectangle scene")
    .with_retained_session(visible_session)
    .expect("authenticated rectangle scene")
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end regression keeps topology, residual, projected resize, and construction/resize history evidence together"
)]
fn m72_interactive_rectangle_has_free_size_and_retained_history() {
    let document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7200_0002_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let baseline_history = coordinator.history_len();
    let expected = coordinator.session().design_identity();

    let outcome = coordinator
        .apply_construction(
            expected,
            &ConstructionProposal::Rectangle {
                first: [2.0, 3.0],
                second: [-1.0, -2.0],
            },
        )
        .expect("free rectangle construction");
    assert!(outcome.published_accepted.is_some());
    assert_eq!(outcome.value.points.len(), 4);
    assert_eq!(outcome.value.curves.len(), 4);
    assert!(outcome.value.scalars.is_empty());
    assert_eq!(coordinator.history_len(), baseline_history + 1);

    let points: [DesignPointId; 4] = outcome
        .value
        .points
        .clone()
        .try_into()
        .expect("four rectangle points");
    let curves: [_; 4] = outcome
        .value
        .curves
        .clone()
        .try_into()
        .expect("four rectangle curves");
    let document = coordinator.session().design_document();
    assert_eq!(document.points().len(), 4);
    assert_eq!(document.curves().len(), 4);
    assert_eq!(document.constraints().len(), 4);
    assert!(document.dimensions().is_empty());
    assert!(document.scalars().is_empty());
    assert!(document.contacts().is_empty());

    let expected_lines = [
        (points[0], points[1], [1.0, 0.0]),
        (points[1], points[2], [0.0, 1.0]),
        (points[2], points[3], [-1.0, 0.0]),
        (points[3], points[0], [0.0, -1.0]),
    ];
    for (curve, (expected_start, expected_end, expected_direction)) in
        curves.into_iter().zip(expected_lines)
    {
        assert!(matches!(
            document.curve(curve).expect("rectangle edge").definition,
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            } if start == expected_start
                && end == expected_end
                && branch_direction.map(f64::to_bits) == expected_direction.map(f64::to_bits)
        ));
    }
    let expected_constraints = [
        DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(curves[0]),
        },
        DocumentConstraintDefinition::Vertical {
            line: CurveSpan::line(curves[1]),
        },
        DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(curves[2]),
        },
        DocumentConstraintDefinition::Vertical {
            line: CurveSpan::line(curves[3]),
        },
    ];
    assert_eq!(
        document
            .constraints()
            .iter()
            .map(|constraint| constraint.definition.clone())
            .collect::<Vec<_>>(),
        expected_constraints
    );
    assert!(document.constraints().iter().all(|constraint| !matches!(
        constraint.definition,
        DocumentConstraintDefinition::FixedPoint { .. }
    )));

    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted original rectangle");
    assert_rectangle_is_finite_and_has_four_dof(accepted);
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[-1.0, -2.0], [2.0, -2.0], [2.0, 3.0], [-1.0, 3.0]],
    );

    let expected = coordinator.session().design_identity();
    let pointer_id = 0x7202;
    let scene = visible_scene(&coordinator);
    let press = scene
        .points
        .iter()
        .find(|point| point.id == points[0])
        .expect("visible bottom-left point")
        .screen_position;
    let _ = coordinator.pointer_down(&scene, pointer(pointer_id, press));
    let target = scene.viewport.model_to_screen([0.0, 0.0]);
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, target));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: requested_pointer,
            request_id,
            point,
            model_position,
        },
    ] = request.as_slice()
    else {
        panic!("rectangle resize did not request one projected move: {request:?}");
    };
    let preview = coordinator.resolve_projected_point_move(
        *requested_pointer,
        *request_id,
        *point,
        *model_position,
    );
    assert!(matches!(
        preview.as_slice(),
        [EditorEffect::PreviewPointMove { point, .. }] if *point == points[0]
    ));
    let release_scene = visible_scene(&coordinator);
    let release =
        coordinator
            .editor_mut()
            .pointer_up(&release_scene, expected, pointer(pointer_id, target));
    assert!(matches!(
        release.as_slice(),
        [EditorEffect::CommitPointMove { point, .. }] if *point == points[0]
    ));
    let resized = coordinator
        .apply_editor_effect(&release[0])
        .expect("commit rectangle resize")
        .expect("retained rectangle resize");
    assert!(resized.published_accepted.is_some());
    assert_eq!(coordinator.history_len(), baseline_history + 2);
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[0.0, 0.0], [2.0, 0.0], [2.0, 3.0], [0.0, 3.0]],
    );
    assert_rectangle_is_finite_and_has_four_dof(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted resized rectangle"),
    );

    coordinator.undo().expect("undo rectangle resize");
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[-1.0, -2.0], [2.0, -2.0], [2.0, 3.0], [-1.0, 3.0]],
    );
    coordinator.redo().expect("redo rectangle resize");
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[0.0, 0.0], [2.0, 0.0], [2.0, 3.0], [0.0, 3.0]],
    );

    coordinator.undo().expect("undo rectangle resize again");
    coordinator.undo().expect("undo rectangle construction");
    assert!(coordinator.session().design_document().points().is_empty());
    assert!(coordinator.session().design_document().curves().is_empty());
    assert!(
        coordinator
            .session()
            .design_document()
            .constraints()
            .is_empty()
    );

    coordinator.redo().expect("redo rectangle construction");
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[-1.0, -2.0], [2.0, -2.0], [2.0, 3.0], [-1.0, 3.0]],
    );
    coordinator.redo().expect("redo rectangle resize again");
    assert_positions_close(
        accepted_positions(&coordinator, points),
        [[0.0, 0.0], [2.0, 0.0], [2.0, 3.0], [0.0, 3.0]],
    );
}
