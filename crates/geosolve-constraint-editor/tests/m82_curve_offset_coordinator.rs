// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringOutcome, AuthoringState, AuthoringTool, ComputedSceneState, ConstraintIntent,
    CoordinatorError, DisabledReason, EditorEffect, EditorScene, FeatureAuthoringCandidate,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringPreviewToken,
    FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarningKind,
    GeometryInteractionPolicy, Modifiers, OffsetAuthoringApplyEffect, OffsetAuthoringOutcome,
    OffsetAuthoringPreviewMetadata, OffsetAuthoringRoute, OffsetAuthoringState,
    OffsetAuthoringTarget, OffsetAuthoringWarningKind, PickTolerance, PointerInput, ReplayAction,
    RetainedEditorCoordinator, SelectionItem, Viewport,
};
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveOffsetGeometry, CurveSpan, DocumentArcSweep,
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentEdit, DocumentFaceOffsetDirection,
    DocumentLineSide, DocumentObjectId, DocumentSolveRequest, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedCurveOffsetChain, ComputedCurveOffsetDirectedSpan, ComputedCurveOffsetJunctionBranch,
    ComputedCurveOffsetJunctionProvenance, ComputedCurveOffsetOperand,
    ComputedCurveOffsetTerminalPolicy, ComputedCurveOffsetTraversal, ComputedEdgeGeometry,
    ComputedEdgeId, ComputedEdgeProvenance, ComputedFeatureEvaluationState, ComputedFeatureFailure,
    ComputedFeatureId, NativeCurveSpanSource,
};

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("current independently accepted sketch");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current accepted state");
    let report = accepted.solve_result().unstable_core_report();
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    RetainedEditorCoordinator::new(session).expect("retained coordinator")
}

fn activate_open_chain(
    coordinator: &mut RetainedEditorCoordinator,
    span: CurveSpan,
    distance: f64,
) -> OffsetAuthoringState {
    let mut state = OffsetAuthoringState::default();
    let entered = coordinator
        .activate_offset_authoring(&mut state)
        .expect("complete operand index");
    assert!(matches!(entered, OffsetAuthoringOutcome::ModeEntered(_)));
    assert!(matches!(
        state.pick_target(OffsetAuthoringTarget::Span(span)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(matches!(
        state.set_distance(distance),
        OffsetAuthoringOutcome::DistanceChanged {
            distance: accepted,
            ..
        } if accepted.to_bits() == distance.to_bits()
    ));
    state
}

fn quadratic_bezier_document() -> (SketchDocument, CurveSpan) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let controls = [
        document.add_point("start", [0.0, 0.0]).unwrap(),
        document.add_point("control", [2.0, 1.0]).unwrap(),
        document.add_point("end", [4.0, 0.0]).unwrap(),
    ];
    let curve = document
        .add_curve(
            "quadratic source",
            CurveDefinition::QuadraticBezier { controls },
        )
        .unwrap();
    (document, CurveSpan::line(curve))
}

fn assert_finite_offset_geometry(geometry: &CurveOffsetGeometry) {
    match geometry {
        CurveOffsetGeometry::Line { start, end } => {
            assert!(start.iter().chain(end).all(|value| value.is_finite()));
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => {
            assert!(center.iter().all(|value| value.is_finite()));
            assert!(radius.is_finite() && *radius > 0.0);
            assert!(start_angle.is_finite());
            assert!(sweep.is_finite());
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            assert!(!patches.is_empty());
            assert!(patches.iter().all(|patch| {
                patch
                    .source_parameters
                    .iter()
                    .chain(patch.controls.iter().flatten())
                    .all(|value| value.is_finite())
            }));
        }
    }
}

fn assert_current_curve_offset(
    coordinator: &RetainedEditorCoordinator,
    feature: ComputedFeatureId,
) -> Vec<ComputedEdgeId> {
    let snapshot = coordinator
        .computed_snapshot()
        .expect("complete current computed snapshot");
    assert_eq!(
        snapshot.input().sketch,
        coordinator.session().prepared_input()
    );
    if coordinator.feature_document().feature(feature).is_some() {
        assert_eq!(
            snapshot.input().features,
            coordinator.feature_document().identity()
        );
    }
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .expect("feature-local evaluation");
    let ComputedFeatureEvaluationState::Current {
        corner_edges,
        generated_edges,
    } = &evaluation.state
    else {
        panic!("Curve Offset must publish one complete Current evaluation: {evaluation:#?}")
    };
    assert!(corner_edges.is_empty());
    assert!(!generated_edges.is_empty());
    for edge_id in generated_edges {
        let edge = snapshot.edge(*edge_id).expect("current generated edge");
        assert!(matches!(
            edge.provenance,
            ComputedEdgeProvenance::CurveOffset { owner, .. } if owner == feature
        ));
        let ComputedEdgeGeometry::CurveOffset(geometry) = &edge.geometry else {
            panic!("Curve Offset provenance must carry Curve Offset geometry")
        };
        assert_finite_offset_geometry(geometry);
        assert_eq!(
            coordinator.selection_for_computed_edge(*edge_id),
            Some(SelectionItem::Feature(feature))
        );
    }
    generated_edges.clone()
}

fn assert_curve_offset_intent(
    coordinator: &RetainedEditorCoordinator,
    feature: ComputedFeatureId,
    distance: f64,
    side: DocumentLineSide,
) {
    let offset = coordinator
        .feature_document()
        .curve_offset(feature)
        .expect("persistent Curve Offset intent");
    assert_eq!(offset.distance.to_bits(), distance.to_bits());
    let ComputedCurveOffsetOperand::OpenChain {
        side: actual_side,
        chain,
    } = &offset.operand
    else {
        panic!("expected one open-chain operand")
    };
    assert_eq!(*actual_side, side);
    assert_eq!(chain.spans.len(), 1);
    assert!(chain.junctions.is_empty());
}

fn single_computed_operand(span: CurveSpan) -> ComputedCurveOffsetOperand {
    ComputedCurveOffsetOperand::OpenChain {
        side: DocumentLineSide::Left,
        chain: ComputedCurveOffsetChain {
            spans: vec![ComputedCurveOffsetDirectedSpan {
                source: NativeCurveSpanSource { span },
                traversal: ComputedCurveOffsetTraversal::Forward,
            }],
            junctions: Vec::new(),
            start_terminal: ComputedCurveOffsetTerminalPolicy::NormalTranslation,
            end_terminal: ComputedCurveOffsetTerminalPolicy::NormalTranslation,
        },
    }
}

fn published_quadratic_curve_offset(
    distance: f64,
) -> (RetainedEditorCoordinator, ComputedFeatureId, CurveSpan) {
    let (document, span) = quadratic_bezier_document();
    let mut coordinator = coordinator(document);
    let mut state = activate_open_chain(&mut coordinator, span, distance);
    let metadata = coordinator
        .prepare_offset_authoring_preview(&state, "Published Curve Offset")
        .expect("computed preview");
    let feature = metadata.computed_curve().expect("computed route").feature;
    let applied = coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("computed Apply");
    assert_eq!(
        applied.value,
        OffsetAuthoringApplyEffect::ComputedCurve(feature)
    );
    assert_current_curve_offset(&coordinator, feature);
    (coordinator, feature, span)
}

fn computed_offset_preview_scene(
    coordinator: &RetainedEditorCoordinator,
    viewport: Viewport,
    retain_interaction_origin: bool,
) -> EditorScene {
    let source = coordinator
        .visible_preview_session()
        .unwrap_or_else(|| coordinator.session());
    let accepted = source
        .accepted_state_for_current_input()
        .expect("current accepted Offset source");
    let accepted_input = source
        .accepted_prepared_input()
        .expect("accepted prepared input");
    let ComputedSceneState::Current { expected, snapshot } = coordinator.computed_scene_state()
    else {
        panic!("computed Offset preview must publish one Current scene")
    };
    let mut scene = EditorScene::from_accepted_with_computed(
        accepted.identity().revision().get(),
        source.design_identity(),
        accepted.document(),
        source.design_document(),
        &accepted_input,
        expected,
        snapshot,
        viewport,
        0.25,
    )
    .expect("exact computed Offset scene")
    .with_retained_session(source)
    .expect("authenticated current scene");
    if retain_interaction_origin {
        coordinator
            .retain_offset_distance_interaction_origin(&mut scene)
            .expect("live Offset distance origin");
    }
    scene
}

fn pointer(pointer_id: u64, position: geosolve_constraint_editor::ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .unwrap()
}

fn two_line_fillet_document() -> (
    SketchDocument,
    geosolve_sketch::DesignPointId,
    [CurveSpan; 2],
) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let corner = document.add_point("corner", [4.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 4.0]).unwrap();
    let first = add_line(&mut document, "first line", start, corner);
    let second = add_line(&mut document, "second line", corner, end);
    (
        document,
        corner,
        [CurveSpan::line(first), CurveSpan::line(second)],
    )
}

fn prepare_two_line_fillet(
    coordinator: &mut RetainedEditorCoordinator,
    corner: geosolve_sketch::DesignPointId,
) -> (
    FeatureAuthoringState,
    FeatureAuthoringCandidate,
    FeatureAuthoringPreviewToken,
) {
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("current feature-authoring snapshot");
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(
            &snapshot,
            snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert!(matches!(
        state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.5),
                ..FeatureAuthoringOptions::default()
            },
        ),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let transaction = coordinator
        .transact_feature_authoring_pick_items(
            &mut state,
            &[(SelectionItem::Point(corner), None)],
            "Two-line Fillet",
        )
        .expect("one-corner Fillet preview");
    let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = transaction.outcome else {
        panic!("point corner must produce one Fillet candidate")
    };
    let token = transaction
        .preview
        .expect("exact held Fillet preview")
        .token;
    (state, candidate, token)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one public retained-boundary regression keeps exact preview publication, generated ownership, edits, history and replay coherent"
)]
fn computed_curve_offset_preview_apply_edit_flip_undo_redo_and_replay_are_one_way_and_exact() {
    let (document, span) = quadratic_bezier_document();
    let mut coordinator = coordinator(document);
    let replay_session = coordinator.session().clone();
    let replay_features = coordinator.feature_document().clone();
    let base_design_json = coordinator.session().export_design_json().unwrap();
    let base_accepted_json = coordinator.session().export_accepted_json().unwrap();
    let base_feature_json = coordinator.feature_document().to_json().unwrap();
    let base_feature_identity = coordinator.feature_document().identity();
    let base_history = (coordinator.history_len(), coordinator.history_cursor());

    let mut state = activate_open_chain(&mut coordinator, span, 0.2);
    let candidate = state.candidate().expect("complete general-curve candidate");
    assert_eq!(candidate.route, OffsetAuthoringRoute::ComputedCurve);
    assert_eq!(candidate.input, coordinator.session().prepared_input());

    let metadata = coordinator
        .prepare_offset_authoring_preview(&state, "Quadratic Curve Offset")
        .expect("certified computed preview");
    assert_eq!(metadata.route(), OffsetAuthoringRoute::ComputedCurve);
    let computed_metadata = metadata
        .computed_curve()
        .expect("computed route metadata")
        .clone();
    assert_eq!(computed_metadata.source_spans, [span]);
    assert!(!computed_metadata.generated_edges.is_empty());
    assert_eq!(
        computed_metadata.input.sketch,
        coordinator.session().prepared_input()
    );
    assert_eq!(
        computed_metadata.input.features,
        computed_metadata.feature_identity
    );
    assert!(coordinator.offset_authoring_preview_matches(&state));
    let preview_edges = assert_current_curve_offset(&coordinator, computed_metadata.feature);
    assert_eq!(preview_edges, computed_metadata.generated_edges);

    assert_eq!(
        coordinator.session().export_design_json().unwrap(),
        base_design_json,
        "preview must not publish native sketch state"
    );
    assert_eq!(
        coordinator.session().export_accepted_json().unwrap(),
        base_accepted_json,
        "preview must not replace accepted native geometry"
    );
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        base_feature_json,
        "preview feature intent must remain provisional"
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        base_history,
        "preview must not add a history position"
    );
    assert!(coordinator.transcript().is_empty());

    let applied = coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("exact preview publication");
    assert_eq!(
        applied.value,
        OffsetAuthoringApplyEffect::ComputedCurve(computed_metadata.feature)
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (base_history.0 + 1, base_history.1 + 1),
        "Apply must add exactly one durable history step"
    );
    assert_eq!(coordinator.transcript().len(), 1);
    assert!(matches!(
        coordinator.transcript(),
        [ReplayAction::CreateComputedCurveOffset { distance, .. }]
            if distance.to_bits() == 0.2_f64.to_bits()
    ));
    assert_eq!(
        coordinator.session().export_design_json().unwrap(),
        base_design_json,
        "computed publication is one-way and must not create native geometry"
    );
    assert_eq!(
        coordinator.session().export_accepted_json().unwrap(),
        base_accepted_json
    );
    assert_eq!(
        coordinator.feature_document().identity(),
        computed_metadata.feature_identity,
        "Apply must publish the exact provisional feature document"
    );
    assert_eq!(
        coordinator.computed_snapshot().unwrap().input(),
        computed_metadata.input,
        "Apply must install the exact provisional evaluation snapshot"
    );
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.2,
        DocumentLineSide::Left,
    );
    let applied_edges = assert_current_curve_offset(&coordinator, computed_metadata.feature);
    assert_eq!(
        applied_edges, preview_edges,
        "Apply must install the exact evaluated preview rather than reevaluating it"
    );

    let before_distance = coordinator.feature_document().identity();
    coordinator
        .set_computed_curve_offset_distance(before_distance, computed_metadata.feature, 0.3)
        .expect("atomic distance edit");
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.3,
        DocumentLineSide::Left,
    );
    let distance_edges = assert_current_curve_offset(&coordinator, computed_metadata.feature);
    assert!(
        applied_edges
            .iter()
            .all(|edge| coordinator.selection_for_computed_edge(*edge).is_none()),
        "a new evaluation must revoke prior revision-local generated IDs"
    );
    assert_ne!(distance_edges, applied_edges);

    let before_flip = coordinator.feature_document().identity();
    coordinator
        .flip_computed_curve_offset_direction(before_flip, computed_metadata.feature)
        .expect("atomic side flip");
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.3,
        DocumentLineSide::Right,
    );
    let flipped_edges = assert_current_curve_offset(&coordinator, computed_metadata.feature);
    assert_ne!(flipped_edges, distance_edges);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (base_history.0 + 3, base_history.1 + 3)
    );
    assert_eq!(coordinator.transcript().len(), 3);
    let replay_actions = coordinator.transcript().to_vec();

    coordinator.undo().expect("Undo flip");
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.3,
        DocumentLineSide::Left,
    );
    assert_current_curve_offset(&coordinator, computed_metadata.feature);
    coordinator.undo().expect("Undo distance edit");
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.2,
        DocumentLineSide::Left,
    );
    assert_current_curve_offset(&coordinator, computed_metadata.feature);
    coordinator.undo().expect("Undo creation");
    assert!(
        coordinator
            .feature_document()
            .curve_offset(computed_metadata.feature)
            .is_none()
    );
    assert_eq!(coordinator.history_cursor(), base_history.1);
    assert_eq!(
        coordinator.session().export_design_json().unwrap(),
        base_design_json
    );
    assert_eq!(
        coordinator.session().export_accepted_json().unwrap(),
        base_accepted_json
    );

    coordinator.redo().expect("Redo creation");
    coordinator.redo().expect("Redo distance edit");
    coordinator.redo().expect("Redo flip");
    assert_curve_offset_intent(
        &coordinator,
        computed_metadata.feature,
        0.3,
        DocumentLineSide::Right,
    );
    assert_current_curve_offset(&coordinator, computed_metadata.feature);

    let mut replayed =
        RetainedEditorCoordinator::with_features(replay_session, replay_features).unwrap();
    assert_eq!(
        replayed.feature_document().identity(),
        base_feature_identity
    );
    for action in &replay_actions {
        replayed
            .replay(action)
            .expect("deterministic action replay");
    }
    assert_eq!(replayed.transcript(), replay_actions);
    assert_eq!(
        (replayed.history_len(), replayed.history_cursor()),
        (base_history.0 + 3, base_history.1 + 3)
    );
    assert_eq!(
        replayed.session().export_design_json().unwrap(),
        base_design_json
    );
    assert_eq!(
        replayed.session().export_accepted_json().unwrap(),
        base_accepted_json
    );
    assert_curve_offset_intent(
        &replayed,
        computed_metadata.feature,
        0.3,
        DocumentLineSide::Right,
    );
    assert_current_curve_offset(&replayed, computed_metadata.feature);
}

#[test]
fn rejected_computed_curve_offset_property_edit_is_exactly_state_neutral() {
    let (mut coordinator, feature, _) = published_quadratic_curve_offset(0.2);
    coordinator
        .flip_computed_curve_offset_direction(coordinator.feature_document().identity(), feature)
        .expect("the nearby right-side parallel remains regular");
    assert_curve_offset_intent(&coordinator, feature, 0.2, DocumentLineSide::Right);
    let current_edges = assert_current_curve_offset(&coordinator, feature);

    let feature_json = coordinator.feature_document().to_json().unwrap();
    let feature_identity = coordinator.feature_document().identity();
    let evaluation_high_water = coordinator.checkpoint().computed_evaluation_high_water();
    let prepared_input = coordinator.session().prepared_input();
    let history = (coordinator.history_len(), coordinator.history_cursor());
    let transcript = coordinator.transcript().to_vec();
    let snapshot_input = coordinator.computed_snapshot().unwrap().input();

    let error = coordinator
        .set_computed_curve_offset_distance(feature_identity, feature, 5.0)
        .expect_err("the right-side quadratic parallel crosses its curvature cusp");
    assert!(matches!(
        error,
        CoordinatorError::FeatureAuthoringPreviewRejected(
            ComputedFeatureFailure::OffsetCurveFailure { .. }
                | ComputedFeatureFailure::OffsetTopologyChange
        )
    ));
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        feature_json,
        "a rejected feature edit must not retain invalid intent"
    );
    assert_eq!(coordinator.feature_document().identity(), feature_identity);
    assert_eq!(
        coordinator.checkpoint().computed_evaluation_high_water(),
        evaluation_high_water,
        "discarded evaluation work must not consume durable generated-edge identity"
    );
    assert_eq!(coordinator.session().prepared_input(), prepared_input);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history
    );
    assert_eq!(coordinator.transcript(), transcript);
    assert_eq!(
        coordinator.computed_snapshot().unwrap().input(),
        snapshot_input
    );
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature),
        current_edges
    );
    assert_curve_offset_intent(&coordinator, feature, 0.2, DocumentLineSide::Right);
}

#[test]
fn rejected_computed_curve_offset_direction_flip_is_exactly_state_neutral() {
    let (document, span) = quadratic_bezier_document();
    let mut coordinator = coordinator(document);
    let mut state = activate_open_chain(&mut coordinator, span, 5.0);
    let feature = coordinator
        .prepare_offset_authoring_preview(&state, "Large left quadratic Offset")
        .expect("the outer quadratic parallel remains regular")
        .into_computed_curve()
        .expect("quadratic uses computed route")
        .feature;
    coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("publish outer quadratic Offset");
    let current_edges = assert_current_curve_offset(&coordinator, feature);
    assert_curve_offset_intent(&coordinator, feature, 5.0, DocumentLineSide::Left);

    let feature_json = coordinator.feature_document().to_json().unwrap();
    let feature_identity = coordinator.feature_document().identity();
    let evaluation_high_water = coordinator.checkpoint().computed_evaluation_high_water();
    let history = (coordinator.history_len(), coordinator.history_cursor());
    let transcript = coordinator.transcript().to_vec();
    let snapshot_input = coordinator.computed_snapshot().unwrap().input();

    let error = coordinator
        .flip_computed_curve_offset_direction(feature_identity, feature)
        .expect_err("the right quadratic parallel crosses its curvature cusp");
    assert!(matches!(
        error,
        CoordinatorError::FeatureAuthoringPreviewRejected(
            ComputedFeatureFailure::OffsetCurveFailure { .. }
                | ComputedFeatureFailure::OffsetTopologyChange
        )
    ));
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        feature_json
    );
    assert_eq!(coordinator.feature_document().identity(), feature_identity);
    assert_eq!(
        coordinator.checkpoint().computed_evaluation_high_water(),
        evaluation_high_water
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history
    );
    assert_eq!(coordinator.transcript(), transcript);
    assert_eq!(
        coordinator.computed_snapshot().unwrap().input(),
        snapshot_input
    );
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature),
        current_edges
    );
    assert_curve_offset_intent(&coordinator, feature, 5.0, DocumentLineSide::Left);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one public-boundary gesture regression keeps computed sampling, authority and atomic Apply evidence together"
)]
fn computed_curve_offset_distance_drag_keeps_last_valid_preview_and_exact_authority() {
    let (document, span) = quadratic_bezier_document();
    let mut coordinator = coordinator(document);
    let mut state = activate_open_chain(&mut coordinator, span, 0.2);
    let metadata = coordinator
        .prepare_offset_authoring_preview(&state, "Draggable quadratic Offset")
        .expect("computed preview")
        .into_computed_curve()
        .expect("computed route metadata");
    let feature = metadata.feature;
    let viewport = Viewport::new([800.0, 600.0], [2.0, 0.0], 80.0).expect("viewport");
    let preview_scene = computed_offset_preview_scene(&coordinator, viewport, false);
    let curve = preview_scene
        .computed_offset_curves
        .iter()
        .find(|curve| curve.owner == feature)
        .expect("generated computed Offset curve");
    let press = curve.screen_polyline[curve.screen_polyline.len() / 2];
    let history_before = (coordinator.history_len(), coordinator.history_cursor());
    let feature_json_before = coordinator.feature_document().to_json().unwrap();

    assert!(
        coordinator
            .hover_offset_authoring_distance(
                &mut state,
                &preview_scene,
                press,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            )
            .expect("computed target hover")
            .is_some()
    );
    assert!(
        coordinator
            .pointer_down_offset_authoring_distance(
                &mut state,
                &preview_scene,
                pointer(8201, press),
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            )
            .expect("computed target press")
            .is_some()
    );
    assert!(!coordinator.offset_authoring_preview_matches(&state));

    let press_model = preview_scene.viewport.screen_to_model(press);
    let valid_position = preview_scene
        .viewport
        .model_to_screen([press_model[0], press_model[1] + 0.15]);
    let valid_scene = computed_offset_preview_scene(&coordinator, viewport, true);
    let valid = coordinator
        .editor_mut()
        .pointer_move(&valid_scene, pointer(8201, valid_position));
    let [
        valid_effect @ EditorEffect::PreviewOffsetAuthoringDistance {
            authority,
            distance,
            ..
        },
    ] = valid.as_slice()
    else {
        panic!("one computed Offset distance request expected, got {valid:?}")
    };
    assert!(*distance > 0.2);

    let forged = EditorEffect::PreviewOffsetAuthoringDistance {
        gesture_epoch: match valid_effect {
            EditorEffect::PreviewOffsetAuthoringDistance { gesture_epoch, .. } => *gesture_epoch,
            _ => unreachable!(),
        },
        authority: geosolve_constraint_editor::OffsetAuthoringPreviewAuthority {
            token: authority.token + 1,
            ..*authority
        },
        distance: *distance,
    };
    assert!(matches!(
        coordinator.apply_offset_authoring_editor_effect(&mut state, &forged),
        Err(CoordinatorError::OffsetPreviewMismatch)
    ));
    assert_eq!(state.distance(), Some(0.2));
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        feature_json_before
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history_before
    );

    coordinator
        .apply_offset_authoring_editor_effect(&mut state, valid_effect)
        .expect("valid computed distance sample");
    let accepted_distance = state.distance().expect("last accepted distance");
    assert_eq!(accepted_distance.to_bits(), distance.to_bits());
    let accepted_edges = assert_current_curve_offset(&coordinator, feature);

    let current_scene = computed_offset_preview_scene(&coordinator, viewport, true);
    let invalid_position = current_scene
        .viewport
        .model_to_screen([press_model[0], press_model[1] - 1.0]);
    assert!(
        coordinator
            .editor_mut()
            .pointer_move(&current_scene, pointer(8201, invalid_position))
            .is_empty(),
        "a nonpositive distance is not an authorizable preview request"
    );
    assert_eq!(state.distance(), Some(accepted_distance));
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature),
        accepted_edges
    );

    let release_scene = computed_offset_preview_scene(&coordinator, viewport, true);
    let expected_design = coordinator.session().design_identity();
    let release = coordinator.editor_mut().pointer_up(
        &release_scene,
        expected_design,
        pointer(8201, invalid_position),
    );
    let [finish @ EditorEffect::FinishOffsetAuthoringDistance { .. }] = release.as_slice() else {
        panic!("one history-neutral finish expected, got {release:?}")
    };
    coordinator
        .apply_offset_authoring_editor_effect(&mut state, finish)
        .expect("release keeps the last valid computed preview");
    assert_eq!(state.distance(), Some(accepted_distance));
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature),
        accepted_edges
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history_before
    );

    let state_after_finish = state.clone();
    let feature_json_after_finish = coordinator.feature_document().to_json().unwrap();
    assert!(matches!(
        coordinator.apply_offset_authoring_editor_effect(&mut state, finish),
        Err(CoordinatorError::OffsetPreviewMismatch)
    ));
    assert_eq!(state, state_after_finish);
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        feature_json_after_finish
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history_before
    );

    let applied = coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("Apply exact last-valid computed candidate");
    assert_eq!(
        applied.value,
        OffsetAuthoringApplyEffect::ComputedCurve(feature)
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (history_before.0 + 1, history_before.1 + 1)
    );
    assert_curve_offset_intent(
        &coordinator,
        feature,
        accepted_distance,
        DocumentLineSide::Left,
    );
    assert_current_curve_offset(&coordinator, feature);
}

#[test]
fn source_failure_suppression_deletion_and_history_never_publish_partial_curve_offset_output() {
    let (mut coordinator, feature, span) = published_quadratic_curve_offset(0.2);
    let original_edges = assert_current_curve_offset(&coordinator, feature);

    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Curve(span.curve),
            },
        )
        .expect("accepted native source deletion");
    let failed_snapshot = coordinator
        .computed_snapshot()
        .expect("failed feature still has a complete evaluation snapshot");
    let failed = failed_snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .expect("failed feature disposition");
    assert!(
        matches!(
            &failed.state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::OffsetMissingSource { .. }
            }
        ),
        "{failed:#?}"
    );
    assert!(
        failed_snapshot.edges().is_empty(),
        "a failed one-feature evaluation must withhold every generated edge"
    );
    assert!(
        original_edges
            .iter()
            .all(|edge| coordinator.selection_for_computed_edge(*edge).is_none())
    );

    coordinator.undo().expect("Undo source deletion");
    assert_current_curve_offset(&coordinator, feature);

    let before_suppress = coordinator.feature_document().identity();
    coordinator
        .set_computed_feature_suppressed(before_suppress, feature, true)
        .expect("suppress Curve Offset");
    let suppressed_snapshot = coordinator
        .computed_snapshot()
        .expect("suppressed snapshot");
    let suppressed = suppressed_snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .expect("suppressed feature disposition");
    assert_eq!(suppressed.state, ComputedFeatureEvaluationState::Suppressed);
    assert!(suppressed_snapshot.edges().is_empty());

    coordinator.undo().expect("Undo suppression");
    assert_current_curve_offset(&coordinator, feature);
    coordinator.redo().expect("Redo suppression");
    assert_eq!(
        coordinator
            .computed_snapshot()
            .unwrap()
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Suppressed
    );
    coordinator.undo().expect("restore Current feature");
    assert_current_curve_offset(&coordinator, feature);

    let before_remove = coordinator.feature_document().identity();
    coordinator
        .remove_computed_feature(before_remove, feature)
        .expect("delete Curve Offset");
    assert!(coordinator.feature_document().feature(feature).is_none());
    assert!(coordinator.computed_snapshot().unwrap().edges().is_empty());
    coordinator.undo().expect("Undo Curve Offset deletion");
    assert_curve_offset_intent(&coordinator, feature, 0.2, DocumentLineSide::Left);
    assert_current_curve_offset(&coordinator, feature);
    coordinator.redo().expect("Redo Curve Offset deletion");
    assert!(coordinator.feature_document().feature(feature).is_none());
    assert!(coordinator.computed_snapshot().unwrap().edges().is_empty());
}

#[test]
fn active_computed_fillet_sources_are_excluded_but_native_fillet_topology_is_offset_eligible() {
    let (document, corner, source_spans) = two_line_fillet_document();

    let mut computed = coordinator(document.clone());
    let (_fillet_state, candidate, token) = prepare_two_line_fillet(&mut computed, corner);
    let feature = computed
        .apply_feature_authoring_preview(token, &candidate)
        .expect("computed Fillet publication")
        .value;
    assert!(computed.feature_document().feature(feature).is_some());
    let computed_features = computed.feature_document().to_json().unwrap();
    let computed_history = (computed.history_len(), computed.history_cursor());
    let computed_transcript = computed.transcript().to_vec();
    let mut excluded = OffsetAuthoringState::default();
    computed
        .activate_offset_authoring(&mut excluded)
        .expect("Offset activation over computed Fillet");
    for span in source_spans {
        assert!(matches!(
            excluded.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::Warning(ref warning)
                if warning.kind == OffsetAuthoringWarningKind::UnsupportedOperand
        ));
        assert!(excluded.operand().is_none());
    }
    assert_eq!(
        computed.feature_document().to_json().unwrap(),
        computed_features
    );
    assert_eq!(
        (computed.history_len(), computed.history_cursor()),
        computed_history
    );
    assert_eq!(computed.transcript(), computed_transcript);

    let mut native = coordinator(document);
    let (_fillet_state, candidate, token) = prepare_two_line_fillet(&mut native, corner);
    native
        .native_feature_authoring_availability(token, &candidate)
        .expect("ordinary line-line Fillet can publish natively");
    let published = native
        .apply_feature_authoring_native_profile(token, &candidate)
        .expect("native Fillet publication")
        .value;
    assert!(native.feature_document().features().is_empty());
    let mut native_offset = OffsetAuthoringState::default();
    native
        .activate_offset_authoring(&mut native_offset)
        .expect("Offset activation over native Fillet");
    for span in [
        CurveSpan::line(published.source_lines[0]),
        CurveSpan::line(published.arc),
        CurveSpan::line(published.source_lines[1]),
    ] {
        assert!(matches!(
            native_offset.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    let native_candidate = native_offset
        .candidate()
        .expect("shortened-line/arc/shortened-line chain");
    assert_eq!(native_candidate.route, OffsetAuthoringRoute::NativeProfile);
    assert_eq!(native_candidate.operand.span_count(), 3);
}

#[test]
fn m82_f001_computed_offset_preview_cold_evaluates_beside_an_unrelated_fillet() {
    let (mut document, corner, _) = two_line_fillet_document();
    let controls = [
        document.add_point("offset start", [8.0, 0.0]).unwrap(),
        document.add_point("offset control", [10.0, 1.0]).unwrap(),
        document.add_point("offset end", [12.0, 0.0]).unwrap(),
    ];
    let curve = document
        .add_curve(
            "unrelated quadratic source",
            CurveDefinition::QuadraticBezier { controls },
        )
        .unwrap();
    let span = CurveSpan::line(curve);
    let mut coordinator = coordinator(document);

    let (_fillet_state, fillet_candidate, token) =
        prepare_two_line_fillet(&mut coordinator, corner);
    let fillet = coordinator
        .apply_feature_authoring_preview(token, &fillet_candidate)
        .expect("computed Fillet publication")
        .value;
    let durable_identity = coordinator.feature_document().identity();
    assert!(
        !coordinator
            .computed_snapshot()
            .expect("current computed Fillet")
            .edges()
            .is_empty()
    );
    let durable_history = (coordinator.history_len(), coordinator.history_cursor());
    let durable_transcript = coordinator.transcript().to_vec();

    let mut offset = OffsetAuthoringState::default();
    coordinator
        .activate_offset_authoring(&mut offset)
        .expect("Offset activation beside an unrelated computed Fillet");
    assert!(matches!(
        offset.pick_target(OffsetAuthoringTarget::Span(span)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert_eq!(
        offset.candidate().expect("computed Offset candidate").route,
        OffsetAuthoringRoute::ComputedCurve
    );

    let preview = coordinator
        .prepare_offset_authoring_preview(&offset, "Cold independent Offset preview")
        .expect("a new feature must be evaluated cold, not continued from the Fillet snapshot");
    let computed = preview
        .computed_curve()
        .expect("computed Curve Offset metadata");
    assert_eq!(computed.source_spans, vec![span]);
    assert!(!computed.generated_edges.is_empty());
    assert_eq!(coordinator.feature_document().identity(), durable_identity);
    assert!(coordinator.feature_document().feature(fillet).is_some());
    assert!(
        coordinator
            .computed_snapshot()
            .expect("complete cold preview")
            .feature_evaluations()
            .iter()
            .any(|evaluation| {
                evaluation.feature == fillet
                    && matches!(
                        &evaluation.state,
                        geosolve_sketch_features::ComputedFeatureEvaluationState::Current { .. }
                    )
            })
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        durable_history
    );
    assert_eq!(coordinator.transcript(), durable_transcript);
}

#[test]
fn generated_curve_offset_edges_cannot_be_reused_as_native_authoring_operands() {
    let (coordinator, feature, _) = published_quadratic_curve_offset(0.5);
    let viewport = Viewport::new([800.0, 600.0], [2.0, 0.0], 80.0).unwrap();
    let scene = computed_offset_preview_scene(&coordinator, viewport, false);
    let generated = scene
        .computed_offset_curves
        .iter()
        .find(|curve| curve.owner == feature)
        .expect("published generated Offset edge");
    let generated_point = generated
        .screen_polyline
        .iter()
        .copied()
        .find(|position| {
            scene
                .native_authoring_hit_test(*position, PickTolerance::default())
                .is_none()
        })
        .expect("generated-only screen sample outside every native pick envelope");
    let feature_json = coordinator.feature_document().to_json().unwrap();
    let history = (coordinator.history_len(), coordinator.history_cursor());
    let transcript = coordinator.transcript().to_vec();

    let mut offset = OffsetAuthoringState::default();
    let mut coordinator = coordinator;
    coordinator
        .activate_offset_authoring(&mut offset)
        .expect("fresh Offset collector");
    assert!(matches!(
        offset.pick_at(
            &scene,
            generated_point,
            PickTolerance::default(),
            GeometryInteractionPolicy::default(),
        ),
        OffsetAuthoringOutcome::Warning(ref warning)
            if warning.kind == OffsetAuthoringWarningKind::NoTarget
    ));
    assert!(offset.operand().is_none());

    let mut relation = AuthoringState::default();
    assert!(matches!(
        relation.activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[],
        ),
        AuthoringOutcome::ModeEntered { .. }
    ));
    let before_relation = relation.clone();
    let relation_outcome = relation.pick_at_with_policy(
        coordinator.session().design_document(),
        &scene,
        generated_point,
        PickTolerance::default(),
        GeometryInteractionPolicy::default(),
    );
    assert!(
        matches!(
            relation_outcome,
        AuthoringOutcome::Warning(ref warning)
            if warning.reason == DisabledReason::WrongOperandKind
        ),
        "generated edge resolved as {relation_outcome:?}"
    );
    assert_eq!(relation, before_relation);

    assert!(matches!(
        coordinator.feature_authoring_picks_for_item(SelectionItem::Feature(feature), None),
        Err(CoordinatorError::FeatureAuthoringPick(ref warning))
            if *warning == FeatureAuthoringWarningKind::WrongOperandKind
    ));
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        feature_json
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history
    );
    assert_eq!(coordinator.transcript(), transcript);
}

#[test]
fn stale_computed_preview_cannot_overwrite_a_newer_feature_transaction() {
    let (document, span) = quadratic_bezier_document();
    let mut coordinator = coordinator(document);
    let mut state = activate_open_chain(&mut coordinator, span, 0.2);
    let held_preview = coordinator
        .prepare_offset_authoring_preview(&state, "Stale Curve Offset")
        .expect("held preview");
    assert_eq!(held_preview.route(), OffsetAuthoringRoute::ComputedCurve);
    let expected = coordinator.feature_document().identity();

    coordinator
        .replay(&ReplayAction::CreateComputedCurveOffset {
            expected,
            label: "Winning Curve Offset".into(),
            distance: 0.1,
            operand: single_computed_operand(span),
        })
        .expect("newer exact feature transaction");
    let winning_json = coordinator.feature_document().to_json().unwrap();
    let winning_history = (coordinator.history_len(), coordinator.history_cursor());
    let winning_transcript = coordinator.transcript().to_vec();
    let winner = coordinator.feature_document().features()[0].id;
    assert_curve_offset_intent(&coordinator, winner, 0.1, DocumentLineSide::Left);
    assert_current_curve_offset(&coordinator, winner);

    let rejected = coordinator.apply_offset_authoring_preview(&mut state);
    assert!(
        matches!(rejected, Err(CoordinatorError::OffsetPreviewMismatch)),
        "stale preview must lose exact feature CAS: {rejected:?}"
    );
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        winning_json
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        winning_history
    );
    assert_eq!(coordinator.transcript(), winning_transcript);
    assert_curve_offset_intent(&coordinator, winner, 0.1, DocumentLineSide::Left);
    assert_current_curve_offset(&coordinator, winner);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mixed-chain regression keeps both offset sides and exact miter provenance together"
)]
fn non_tangent_line_and_bezier_chain_trims_the_inner_miter_and_supports_both_sides() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("line start", [-2.0, 0.0]).unwrap();
    let join = document.add_point("shared join", [0.0, 0.0]).unwrap();
    let control = document.add_point("bezier control", [2.0, 1.0]).unwrap();
    let end = document.add_point("bezier end", [4.0, 2.0]).unwrap();
    let line = add_line(&mut document, "line", start, join);
    let bezier = document
        .add_curve(
            "quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [join, control, end],
            },
        )
        .unwrap();
    let spans = [CurveSpan::line(line), CurveSpan::line(bezier)];
    let mut coordinator = coordinator(document);
    let mut state = OffsetAuthoringState::default();
    assert!(matches!(
        coordinator.activate_offset_authoring(&mut state),
        Ok(OffsetAuthoringOutcome::ModeEntered(_))
    ));
    for span in spans {
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert!(matches!(
        state.set_distance(0.2),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    let candidate = state.candidate().expect("mixed open-chain candidate");
    assert_eq!(candidate.route, OffsetAuthoringRoute::ComputedCurve);

    let left = coordinator
        .prepare_offset_authoring_preview(&state, "Mixed Curve Offset")
        .expect("left preview")
        .into_computed_curve()
        .expect("computed route");
    let feature = left.feature;
    coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("left publication");
    let inner_edges = assert_current_curve_offset(&coordinator, feature);
    assert_eq!(
        inner_edges.len(),
        2,
        "the inner side trims both generated parallels without connector fragments"
    );
    let snapshot = coordinator.computed_snapshot().unwrap();
    let current_end = match &snapshot.edge(inner_edges[0]).unwrap().geometry {
        ComputedEdgeGeometry::CurveOffset(CurveOffsetGeometry::Line { end, .. }) => *end,
        geometry => panic!("expected exact leading line, got {geometry:?}"),
    };
    let next_start = match &snapshot.edge(inner_edges[1]).unwrap().geometry {
        ComputedEdgeGeometry::CurveOffset(CurveOffsetGeometry::CubicPatches(patches)) => {
            patches.first().unwrap().controls[0]
        }
        geometry => panic!("expected fitted trailing Bezier, got {geometry:?}"),
    };
    assert!(
        (current_end[0] - next_start[0]).abs() <= 1.0e-10
            && (current_end[1] - next_start[1]).abs() <= 1.0e-10,
        "trimmed inner miter must close on the generated curves: {current_end:?} vs {next_start:?}"
    );

    let assert_operand = |coordinator: &RetainedEditorCoordinator, side| {
        let offset = coordinator
            .feature_document()
            .curve_offset(feature)
            .expect("persistent mixed Offset");
        let ComputedCurveOffsetOperand::OpenChain {
            side: actual_side,
            chain,
        } = &offset.operand
        else {
            panic!("mixed operand must remain an open chain")
        };
        assert_eq!(*actual_side, side);
        assert_eq!(
            chain
                .spans
                .iter()
                .map(|span| span.source.span)
                .collect::<Vec<_>>(),
            spans
        );
        assert!(
            chain
                .spans
                .iter()
                .all(|span| span.traversal == ComputedCurveOffsetTraversal::Forward)
        );
        assert_eq!(chain.junctions.len(), 1);
        assert_eq!(
            chain.junctions[0].provenance,
            ComputedCurveOffsetJunctionProvenance::SharedPoint(join)
        );
        assert!(matches!(
            chain.junctions[0].branch,
            ComputedCurveOffsetJunctionBranch::Miter { .. }
        ));
        assert_eq!(
            chain.start_terminal,
            ComputedCurveOffsetTerminalPolicy::NormalTranslation
        );
        assert_eq!(
            chain.end_terminal,
            ComputedCurveOffsetTerminalPolicy::NormalTranslation
        );
    };
    assert_operand(&coordinator, DocumentLineSide::Left);

    coordinator
        .flip_computed_curve_offset_direction(coordinator.feature_document().identity(), feature)
        .expect("durable side flip");
    assert_operand(&coordinator, DocumentLineSide::Right);
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature).len(),
        4,
        "the outer side retains both source parallels plus its two authenticated miter connectors"
    );
}

fn assert_curved_miter_pair_supports_inner_trim_and_outer_connectors(
    document: SketchDocument,
    spans: [CurveSpan; 2],
    label: &str,
) {
    let mut coordinator = coordinator(document);
    let mut state = OffsetAuthoringState::default();
    coordinator
        .activate_offset_authoring(&mut state)
        .expect("complete curved-pair operand index");
    for span in spans {
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert!(matches!(
        state.set_distance(0.2),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert_eq!(
        state.candidate().expect("curved-pair candidate").route,
        OffsetAuthoringRoute::ComputedCurve
    );
    let feature = coordinator
        .prepare_offset_authoring_preview(&state, label)
        .expect("certified inner curved miter")
        .into_computed_curve()
        .expect("computed route")
        .feature;
    coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("publish inner curved miter");
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature).len(),
        2,
        "inner {label} miter must trim the two generated curves without connectors"
    );
    let offset = coordinator
        .feature_document()
        .curve_offset(feature)
        .expect("persistent curved pair");
    let ComputedCurveOffsetOperand::OpenChain { chain, .. } = &offset.operand else {
        panic!("curved pair must remain an open chain")
    };
    assert!(matches!(
        chain.junctions.as_slice(),
        [geosolve_sketch_features::ComputedCurveOffsetJunction {
            branch: ComputedCurveOffsetJunctionBranch::Miter { .. },
            ..
        }]
    ));

    coordinator
        .flip_computed_curve_offset_direction(coordinator.feature_document().identity(), feature)
        .expect("flip curved-pair side");
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature).len(),
        4,
        "outer {label} miter must retain two generated curves and two connectors"
    );
}

#[test]
fn cubic_to_cubic_inner_miter_is_certified_and_trimmed_on_both_curves() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("first start", [-4.0, 0.0]).unwrap();
    let first_control = document.add_point("first control", [-2.0, 0.0]).unwrap();
    let join = document.add_point("shared join", [0.0, 0.0]).unwrap();
    let second_control = document.add_point("second control", [2.0, 1.0]).unwrap();
    let end = document.add_point("second end", [4.0, 2.0]).unwrap();
    let first = document
        .add_curve(
            "first quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [start, first_control, join],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [join, second_control, end],
            },
        )
        .unwrap();
    assert_curved_miter_pair_supports_inner_trim_and_outer_connectors(
        document,
        [CurveSpan::line(first), CurveSpan::line(second)],
        "cubic-to-cubic",
    );
}

#[test]
fn circular_arc_to_cubic_inner_miter_is_certified_without_linearizing_the_arc() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("arc centre", [0.0, 2.0]).unwrap();
    let radius = document
        .add_scalar(
            "arc radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let start_angle = document
        .add_scalar(
            "arc start",
            -std::f64::consts::PI,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end_angle = document
        .add_scalar(
            "arc end",
            -std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "source arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let join = document.add_point("Bezier start", [0.0, 0.0]).unwrap();
    let control = document.add_point("Bezier control", [2.0, 1.0]).unwrap();
    let end = document.add_point("Bezier end", [4.0, 2.0]).unwrap();
    let bezier = document
        .add_curve(
            "quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [join, control, end],
            },
        )
        .unwrap();
    let arc_span = CurveSpan::line(arc);
    let arc_end = document
        .add_curve_contact(
            "owned arc end",
            arc_span,
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "arc-to-Bezier join",
            DocumentConstraintDefinition::PointOnCurve {
                point: join,
                contact: arc_end,
            },
        )
        .unwrap();
    assert_curved_miter_pair_supports_inner_trim_and_outer_connectors(
        document,
        [arc_span, CurveSpan::line(bezier)],
        "arc-to-cubic",
    );
}

#[test]
fn intrinsic_bspline_spans_publish_one_owned_tangent_junction() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
        .into_iter()
        .enumerate()
        .map(|(index, position)| {
            document
                .add_point(format!("control {index}"), position)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let spline = document
        .add_curve(
            "clamped spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls,
                knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 74,
            },
        )
        .unwrap();
    let spans = [
        CurveSpan {
            curve: spline,
            segment: 41,
        },
        CurveSpan {
            curve: spline,
            segment: 73,
        },
    ];
    let mut coordinator = coordinator(document);
    let mut state = OffsetAuthoringState::default();
    coordinator
        .activate_offset_authoring(&mut state)
        .expect("complete spline index");
    for span in spans {
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert!(matches!(
        state.set_distance(0.1),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert_eq!(
        state.candidate().expect("spline candidate").route,
        OffsetAuthoringRoute::ComputedCurve
    );
    let metadata = coordinator
        .prepare_offset_authoring_preview(&state, "Intrinsic spline Offset")
        .expect("complete spline preview")
        .into_computed_curve()
        .expect("computed route");
    let feature = metadata.feature;
    coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("spline Offset publication");
    let offset = coordinator
        .feature_document()
        .curve_offset(feature)
        .unwrap();
    let ComputedCurveOffsetOperand::OpenChain { chain, .. } = &offset.operand else {
        panic!("spline spans must persist as an open chain")
    };
    assert_eq!(
        chain
            .spans
            .iter()
            .map(|span| span.source.span)
            .collect::<Vec<_>>(),
        spans
    );
    assert_eq!(chain.junctions.len(), 1);
    assert_eq!(
        chain.junctions[0].provenance,
        ComputedCurveOffsetJunctionProvenance::IntrinsicSpanBoundary
    );
    assert_eq!(
        chain.junctions[0].branch,
        ComputedCurveOffsetJunctionBranch::Tangent
    );
    assert_current_curve_offset(&coordinator, feature);
}

#[test]
fn computed_holed_face_retains_exact_loops_and_both_offset_directions() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("shared centre", [0.0, 0.0]).unwrap();
    for (label, value) in [("outer circle", 4.0), ("inner circle", 2.0)] {
        let radius = document
            .add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        document
            .add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let mut coordinator = coordinator(document);
    let mut state = OffsetAuthoringState::default();
    coordinator
        .activate_offset_authoring(&mut state)
        .expect("complete holed-face index");
    let face = state
        .index()
        .unwrap()
        .faces()
        .iter()
        .find(|face| face.key.holes.len() == 1)
        .expect("annular face")
        .key
        .clone();
    let directed_span =
        |directed: &geosolve_sketch_topology::OffsetDirectedSpan| ComputedCurveOffsetDirectedSpan {
            source: NativeCurveSpanSource {
                span: directed.span,
            },
            traversal: match directed.traversal {
                geosolve_sketch_topology::OffsetTraversal::Forward => {
                    ComputedCurveOffsetTraversal::Forward
                }
                geosolve_sketch_topology::OffsetTraversal::Reverse => {
                    ComputedCurveOffsetTraversal::Reverse
                }
            },
        };
    let operand = ComputedCurveOffsetOperand::Face {
        direction: DocumentFaceOffsetDirection::Outward,
        outer: geosolve_sketch_features::ComputedCurveOffsetLoop {
            spans: face.outer.spans.iter().map(directed_span).collect(),
            junctions: Vec::new(),
        },
        holes: face
            .holes
            .iter()
            .map(|hole| geosolve_sketch_features::ComputedCurveOffsetLoop {
                spans: hole.spans.iter().map(directed_span).collect(),
                junctions: Vec::new(),
            })
            .collect(),
    };
    let expected = coordinator.feature_document().identity();
    coordinator
        .replay(&ReplayAction::CreateComputedCurveOffset {
            expected,
            label: "Holed computed face Offset".into(),
            distance: 0.1,
            operand,
        })
        .expect("computed holed-face publication");
    let feature = coordinator.feature_document().features()[0].id;
    let assert_direction = |coordinator: &RetainedEditorCoordinator, expected_direction| {
        let offset = coordinator
            .feature_document()
            .curve_offset(feature)
            .unwrap();
        let ComputedCurveOffsetOperand::Face {
            direction,
            outer,
            holes,
        } = &offset.operand
        else {
            panic!("annulus must persist as a face")
        };
        assert_eq!(*direction, expected_direction);
        assert_eq!(outer.spans.len(), 1);
        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].spans.len(), 1);
    };
    assert_direction(&coordinator, DocumentFaceOffsetDirection::Outward);
    assert_current_curve_offset(&coordinator, feature);

    coordinator
        .flip_computed_curve_offset_direction(coordinator.feature_document().identity(), feature)
        .expect("annulus direction flip");
    assert_direction(&coordinator, DocumentFaceOffsetDirection::Inward);
    assert_current_curve_offset(&coordinator, feature);
}

#[test]
fn general_curve_face_retains_exact_hole_and_both_offset_directions() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("shared centre", [0.0, 0.0]).unwrap();
    let major_axis_point = document
        .add_point("outer ellipse major", [5.0, 0.0])
        .unwrap();
    let minor_axis_ratio = document
        .add_scalar(
            "outer ellipse ratio",
            0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    document
        .add_curve(
            "outer ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            },
        )
        .unwrap();
    let hole_radius = document
        .add_scalar(
            "circular hole radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(
            "circular hole",
            CurveDefinition::Circle {
                center,
                radius: hole_radius,
            },
        )
        .unwrap();
    let mut coordinator = coordinator(document);
    let mut state = OffsetAuthoringState::default();
    coordinator
        .activate_offset_authoring(&mut state)
        .expect("complete general-face index");
    let face = state
        .index()
        .unwrap()
        .faces()
        .iter()
        .find(|face| face.key.holes.len() == 1)
        .expect("mixed ellipse/circle annulus")
        .key
        .clone();
    assert!(matches!(
        state.pick_target(OffsetAuthoringTarget::Face(face)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(matches!(
        state.set_distance(0.1),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert_eq!(
        state.candidate().expect("general face candidate").route,
        OffsetAuthoringRoute::ComputedCurve
    );
    let feature = coordinator
        .prepare_offset_authoring_preview(&state, "Elliptical face Offset")
        .expect("outward general-face preview")
        .into_computed_curve()
        .expect("computed face route")
        .feature;
    coordinator
        .apply_offset_authoring_preview(&mut state)
        .expect("publish outward general face");
    let outward = assert_current_curve_offset(&coordinator, feature);
    assert_eq!(outward.len(), 2, "outer plus one exact retained hole");
    let snapshot = coordinator.computed_snapshot().unwrap();
    assert!(outward.iter().any(|edge| matches!(
        &snapshot.edge(*edge).unwrap().geometry,
        ComputedEdgeGeometry::CurveOffset(CurveOffsetGeometry::CubicPatches(_))
    )));
    assert!(outward.iter().any(|edge| matches!(
        &snapshot.edge(*edge).unwrap().geometry,
        ComputedEdgeGeometry::CurveOffset(CurveOffsetGeometry::CircularArc { closed: true, .. })
    )));

    coordinator
        .flip_computed_curve_offset_direction(coordinator.feature_document().identity(), feature)
        .expect("flip general face direction");
    assert_eq!(
        assert_current_curve_offset(&coordinator, feature).len(),
        2,
        "inward general face must retain the same outer/hole cardinality"
    );
}

#[derive(Clone, Copy)]
enum NativeFixtureTarget {
    Span(CurveSpan),
    OnlyFace,
}

fn assert_native_route(document: SketchDocument, target: NativeFixtureTarget, label: &str) {
    let mut coordinator = coordinator(document);
    let base_features = coordinator.feature_document().to_json().unwrap();
    let base_history = (coordinator.history_len(), coordinator.history_cursor());
    let mut state = OffsetAuthoringState::default();
    assert!(matches!(
        coordinator.activate_offset_authoring(&mut state),
        Ok(OffsetAuthoringOutcome::ModeEntered(_))
    ));
    let target = match target {
        NativeFixtureTarget::Span(span) => OffsetAuthoringTarget::Span(span),
        NativeFixtureTarget::OnlyFace => {
            let faces = state.index().expect("operand index").faces();
            assert_eq!(faces.len(), 1, "{label}: expected one bounded face");
            OffsetAuthoringTarget::Face(faces[0].key.clone())
        }
    };
    assert!(matches!(
        state.pick_target(target),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ));
    assert!(matches!(
        state.set_distance(0.2),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert_eq!(
        state.candidate().expect("complete candidate").route,
        OffsetAuthoringRoute::NativeProfile,
        "{label}: native family must retain the M80 route"
    );
    let metadata = coordinator
        .prepare_offset_authoring_preview(&state, label)
        .expect("independently accepted native preview");
    assert!(matches!(
        metadata,
        OffsetAuthoringPreviewMetadata::NativeProfile(_)
    ));
    assert_eq!(metadata.route(), OffsetAuthoringRoute::NativeProfile);
    assert!(metadata.native_profile().is_some());
    assert!(metadata.computed_curve().is_none());
    assert_eq!(
        coordinator.feature_document().to_json().unwrap(),
        base_features,
        "{label}: native preview must not create Curve Offset intent"
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        base_history,
        "{label}: preview remains provisional"
    );
}

#[test]
fn line_circle_and_circular_arc_keep_the_exact_native_profile_offset_route() {
    let mut line_document = SketchDocument::new(10.0).unwrap();
    let line_start = line_document.add_point("line start", [0.0, 0.0]).unwrap();
    let line_end = line_document.add_point("line end", [4.0, 0.0]).unwrap();
    let line = line_document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: line_start,
                end: line_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    assert_native_route(
        line_document,
        NativeFixtureTarget::Span(CurveSpan::line(line)),
        "Line Profile Offset",
    );

    let mut circle_document = SketchDocument::new(10.0).unwrap();
    let circle_center = circle_document
        .add_point("circle center", [0.0, 0.0])
        .unwrap();
    let circle_radius = circle_document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    circle_document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    assert_native_route(
        circle_document,
        NativeFixtureTarget::OnlyFace,
        "Circle Profile Offset",
    );

    let mut arc_document = SketchDocument::new(10.0).unwrap();
    let arc_center = arc_document.add_point("arc center", [0.0, 0.0]).unwrap();
    let arc_radius = arc_document
        .add_scalar(
            "arc radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = arc_document
        .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = arc_document
        .add_scalar(
            "arc end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = arc_document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    assert_native_route(
        arc_document,
        NativeFixtureTarget::Span(CurveSpan::line(arc)),
        "Circular Arc Profile Offset",
    );
}
