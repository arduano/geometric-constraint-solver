// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ActivePointerGesture, ActivePointerGestureKind, ConstraintEditor, EditorEffect,
    EditorHoverState, EditorHoverTarget, EditorScene, EditorTool, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, GeometryPickScope, Modifiers, PickTolerance,
    PointerInput, RetainedEditorCoordinator, SceneAnnotationGeometry, SceneAnnotationOccurrence,
    SceneAnnotationVisibility, SceneFilletHit, SceneGlyphMarker, ScreenPoint, SelectionItem,
    Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentEdit, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDatum, SketchDocument,
    SolverConfig,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

struct PointerParityFixture {
    coordinator: RetainedEditorCoordinator,
    base_scene: EditorScene,
    computed_scene: EditorScene,
    authoring: FeatureAuthoringState,
    first_span: CurveSpan,
    circle_span: CurveSpan,
    circle_center: geosolve_sketch::DesignPointId,
    overlap_point: geosolve_sketch::DesignPointId,
    overlap: ScreenPoint,
    owner: geosolve_constraint_editor::ComputedCornerRef,
}

fn pointer(pointer_id: u64, position: ScreenPoint, modifiers: Modifiers) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers,
    }
}

fn screen_distance(first: ScreenPoint, second: ScreenPoint) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

fn hover_change(
    target: Option<EditorHoverTarget>,
    context_owner: Option<SelectionItem>,
) -> Vec<EditorEffect> {
    vec![EditorEffect::HoverChanged(EditorHoverState {
        target,
        context_owner,
    })]
}

#[allow(
    clippy::too_many_lines,
    reason = "one accepted fixture gives every parity row identical native, computed, annotation, and datum overlap authority"
)]
fn parity_fixture() -> PointerParityFixture {
    let viewport = Viewport::new([800.0, 600.0], [2.0, 2.0], 50.0).expect("viewport");
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
    let end = document.add_point("end", [4.0, 4.0]).expect("end");
    let overlap_point = document
        .add_point("draggable Fillet overlap", [3.0, 0.0])
        .expect("overlap point");
    let circle_center = document
        .add_point("semantic circle center", [3.0, 2.0])
        .expect("circle center");
    let circle_radius = document
        .add_scalar(
            "semantic circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("circle radius");
    let first = document
        .add_curve(
            "horizontal parent",
            CurveDefinition::Line {
                start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let _second = document
        .add_curve(
            "vertical parent",
            CurveDefinition::Line {
                start: corner,
                end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    let circle_span = CurveSpan::line(
        document
            .add_curve(
                "semantic-center circle",
                CurveDefinition::Circle {
                    center: circle_center,
                    radius: circle_radius,
                },
            )
            .expect("circle"),
    );
    let first_span = CurveSpan::line(first);
    for label in [
        "horizontal",
        "horizontal duplicate 1",
        "horizontal duplicate 2",
        "horizontal duplicate 3",
        "horizontal duplicate 4",
        "horizontal duplicate 5",
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::Horizontal { line: first_span },
            )
            .expect("horizontal constraint");
    }

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted state");
    let base_scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.5,
    )
    .expect("base scene");

    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("feature-authoring snapshot");
    let accepted_document = snapshot.sketch_document().clone();
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(
            &snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    let first_pick = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &base_scene,
            viewport.model_to_screen([2.0, 0.0]),
            PickTolerance::default(),
            "pointer parity Fillet",
        )
        .expect("first Fillet support");
    assert!(matches!(
        first_pick.outcome,
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let second_pick = coordinator
        .transact_feature_authoring_pick_at(
            &mut authoring,
            &base_scene,
            viewport.model_to_screen([4.0, 2.0]),
            PickTolerance::default(),
            "pointer parity Fillet",
        )
        .expect("second Fillet support");
    assert!(matches!(
        second_pick.outcome,
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    let preview = coordinator
        .feature_authoring_preview()
        .expect("held Fillet preview");
    let preview_input = preview.metadata().input;
    let preview_snapshot = preview.snapshot().clone();
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted source");
    let mut computed_scene = EditorScene::from_accepted_with_computed(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        &coordinator
            .session()
            .accepted_prepared_input()
            .expect("accepted input"),
        &preview_input,
        &preview_snapshot,
        viewport,
        0.5,
    )
    .expect("computed scene");
    let owner = computed_scene
        .computed_curves
        .first()
        .expect("computed Fillet arc")
        .owner;
    coordinator
        .populate_computed_fillet_affordances(
            &mut computed_scene,
            &[SelectionItem::FeatureCorner(owner)],
            0.5,
        )
        .expect("Fillet radius affordance");
    let overlap = viewport.model_to_screen([3.0, 0.0]);
    assert!(matches!(
        computed_scene.resolve_fillet_hit(overlap, PickTolerance::default()),
        Some(SceneFilletHit::Radius {
            owner: resolved, ..
        }) if resolved == owner
    ));

    PointerParityFixture {
        coordinator,
        base_scene,
        computed_scene,
        authoring,
        first_span,
        circle_span,
        circle_center,
        overlap_point,
        overlap,
        owner,
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn context_corridor_reveals_only_and_never_becomes_a_click_target() {
    let fixture = parity_fixture();
    let line_item = SelectionItem::Curve(fixture.first_span);
    let (_leader_origin, marker_anchor) = fixture
        .base_scene
        .annotations
        .iter()
        .filter(|annotation| annotation.operands.contains(&line_item))
        .filter_map(|annotation| match &annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => markers
                .iter()
                .filter_map(|marker| marker.leader_from.map(|origin| (origin, marker.anchor)))
                .max_by(|first, second| {
                    screen_distance(first.0, first.1)
                        .total_cmp(&screen_distance(second.0, second.1))
                }),
            _ => None,
        })
        .max_by(|first, second| {
            screen_distance(first.0, first.1).total_cmp(&screen_distance(second.0, second.1))
        })
        .expect("displaced annotation leader");
    let tolerance = PickTolerance::default();
    let curve = fixture
        .base_scene
        .curves
        .iter()
        .find(|curve| curve.span == fixture.first_span)
        .expect("context curve");
    let first = curve.screen_polyline[0];
    let last = *curve.screen_polyline.last().expect("context curve end");
    let context_origin = (1..20)
        .map(|step| {
            let ratio = f64::from(step) / 20.0;
            ScreenPoint {
                x: (last.x - first.x).mul_add(ratio, first.x),
                y: (last.y - first.y).mul_add(ratio, first.y),
            }
        })
        .find(|position| {
            fixture
                .base_scene
                .hit_test(*position, tolerance)
                .is_some_and(|hit| hit.item == line_item)
                && !fixture.base_scene.annotations.iter().any(|annotation| {
                    annotation.is_visible(&[], Some(line_item), &[])
                        && annotation.hit_test(*position, tolerance.annotation_pixels)
                })
        })
        .expect("passive curve sample outside every revealed annotation");
    let corridor = (1..100)
        .map(|step| {
            let ratio = f64::from(step) / 100.0;
            ScreenPoint {
                x: (marker_anchor.x - context_origin.x).mul_add(ratio, context_origin.x),
                y: (marker_anchor.y - context_origin.y).mul_add(ratio, context_origin.y),
            }
        })
        .find(|position| {
            fixture.base_scene.hit_test(*position, tolerance).is_none()
                && !fixture.base_scene.annotations.iter().any(|annotation| {
                    annotation.is_visible(&[], Some(line_item), &[])
                        && annotation.hit_test(*position, tolerance.annotation_pixels)
                })
        })
        .expect("bounded reveal-only corridor sample");

    let mut editor = ConstraintEditor::default();
    assert_eq!(
        editor.pointer_move(
            &fixture.base_scene,
            pointer(1, context_origin, Modifiers::default()),
        ),
        hover_change(
            Some(EditorHoverTarget::Geometry(line_item)),
            Some(line_item),
        )
    );
    assert_eq!(
        editor.pointer_move(
            &fixture.base_scene,
            pointer(1, corridor, Modifiers::default()),
        ),
        hover_change(None, Some(line_item)),
    );
    assert!(
        editor
            .pointer_down(
                &fixture.base_scene,
                pointer(1, corridor, Modifiers::default()),
            )
            .is_empty()
    );
    assert!(editor.selection().is_empty());
    assert!(editor.active_pointer_gesture().is_none());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn first_sample_uses_prospective_geometry_context_for_hover_and_click() {
    let fixture = parity_fixture();
    let mut scene = fixture.base_scene.clone();
    let line_item = SelectionItem::Curve(fixture.first_span);
    let annotation_item = scene
        .annotations
        .iter()
        .filter(|annotation| annotation.operands.contains(&line_item))
        .map(|annotation| annotation.item)
        .min()
        .expect("contextual line annotation");
    scene
        .annotations
        .retain(|annotation| annotation.item == annotation_item);
    let position = scene.viewport.model_to_screen([1.0, 0.0]);
    scene.annotations[0].visibility = SceneAnnotationVisibility::Contextual;
    scene.annotations[0].geometry = SceneAnnotationGeometry::Label { anchor: position };
    assert_eq!(
        scene
            .hit_test(position, PickTolerance::default())
            .map(|hit| hit.item),
        Some(line_item),
        "the lower passive owner supplies prospective context",
    );
    let occurrence = SceneAnnotationOccurrence {
        item: annotation_item,
        marker_index: None,
    };
    let mut editor = ConstraintEditor::default();
    assert_eq!(
        editor.pointer_move(&scene, pointer(6, position, Modifiers::default()),),
        hover_change(Some(EditorHoverTarget::Annotation(occurrence)), None),
    );
    assert_eq!(
        editor.pointer_down(&scene, pointer(6, position, Modifiers::default()),),
        vec![EditorEffect::SelectionChanged(vec![annotation_item])],
    );
    assert_eq!(editor.selection(), &[annotation_item]);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn problem_forced_annotation_uses_the_same_move_and_down_wrapper_input() {
    let fixture = parity_fixture();
    let mut scene = fixture.base_scene.clone();
    let annotation_item = scene
        .annotations
        .iter()
        .map(|annotation| annotation.item)
        .min()
        .expect("contextual annotation");
    scene
        .annotations
        .retain(|annotation| annotation.item == annotation_item);
    let position = ScreenPoint { x: 40.0, y: 40.0 };
    scene.annotations[0].visibility = SceneAnnotationVisibility::Contextual;
    scene.annotations[0].geometry = SceneAnnotationGeometry::Label { anchor: position };
    assert!(
        scene.hit_test(position, PickTolerance::default()).is_none(),
        "the forced annotation sample must not inherit geometry or datum context",
    );
    let occurrence = SceneAnnotationOccurrence {
        item: annotation_item,
        marker_index: None,
    };

    let mut editor = ConstraintEditor::default();
    assert!(
        editor
            .pointer_move(&scene, pointer(7, position, Modifiers::default()))
            .is_empty(),
        "a contextual annotation is hidden without current problem ownership",
    );
    assert_eq!(
        editor.pointer_move_with_problem_items(
            &scene,
            pointer(7, position, Modifiers::default()),
            &[annotation_item],
        ),
        hover_change(Some(EditorHoverTarget::Annotation(occurrence)), None),
    );
    assert_eq!(
        editor.pointer_down_with_problem_items(
            &scene,
            pointer(7, position, Modifiers::default()),
            &[annotation_item],
        ),
        vec![EditorEffect::SelectionChanged(vec![annotation_item])],
    );
    assert_eq!(editor.selection(), &[annotation_item]);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn stored_point_and_semantic_center_precede_an_overlapping_annotation() {
    let fixture = parity_fixture();
    let mut scene = fixture.computed_scene.clone();
    scene.fillet_affordances.clear();
    for curve in &mut scene.computed_curves {
        curve.radius_rail = None;
    }
    let annotation_item = scene
        .annotations
        .iter()
        .map(|annotation| annotation.item)
        .min()
        .expect("overlap annotation");
    scene
        .annotations
        .retain(|annotation| annotation.item == annotation_item);
    scene.annotations[0].visibility = SceneAnnotationVisibility::Always;
    scene.annotations[0].geometry = SceneAnnotationGeometry::Label {
        anchor: fixture.overlap,
    };

    let point_item = SelectionItem::Point(fixture.overlap_point);
    let mut point_editor = ConstraintEditor::default();
    assert_eq!(
        point_editor.pointer_move(&scene, pointer(8, fixture.overlap, Modifiers::default()),),
        hover_change(
            Some(EditorHoverTarget::Geometry(point_item)),
            Some(point_item),
        ),
    );
    let _ = point_editor.pointer_down(&scene, pointer(8, fixture.overlap, Modifiers::default()));
    assert_eq!(point_editor.selection(), &[point_item]);
    assert_eq!(
        point_editor.active_pointer_gesture(),
        Some(ActivePointerGesture {
            pointer_id: 8,
            kind: ActivePointerGestureKind::Point,
        }),
    );

    scene
        .points
        .retain(|point| point.id != fixture.overlap_point);
    let center_item = SelectionItem::Curve(fixture.circle_span);
    assert_eq!(
        scene
            .curves
            .iter()
            .find(|curve| curve.span == fixture.circle_span)
            .and_then(|curve| curve.drag_handle_point),
        Some(fixture.circle_center),
        "the circle span must publish its semantic center as the drag owner",
    );
    let mut center_editor = ConstraintEditor::default();
    assert_eq!(
        center_editor.pointer_move(&scene, pointer(9, fixture.overlap, Modifiers::default()),),
        hover_change(
            Some(EditorHoverTarget::Geometry(center_item)),
            Some(center_item),
        ),
    );
    let _ = center_editor.pointer_down(&scene, pointer(9, fixture.overlap, Modifiers::default()));
    assert_eq!(center_editor.selection(), &[center_item]);
    assert_eq!(
        center_editor.active_pointer_gesture(),
        Some(ActivePointerGesture {
            pointer_id: 9,
            kind: ActivePointerGestureKind::Point,
        }),
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn passive_geometry_precedes_datum_and_datum_precedes_empty_canvas() {
    let fixture = parity_fixture();
    let mut scene = fixture.base_scene.clone();
    scene.annotations.clear();
    let geometry_position = scene.viewport.model_to_screen([1.0, 0.0]);
    let geometry_item = SelectionItem::Curve(fixture.first_span);
    let mut geometry_editor = ConstraintEditor::default();
    assert_eq!(
        geometry_editor.pointer_move(&scene, pointer(10, geometry_position, Modifiers::default()),),
        hover_change(
            Some(EditorHoverTarget::Geometry(geometry_item)),
            Some(geometry_item),
        ),
    );
    let _ =
        geometry_editor.pointer_down(&scene, pointer(10, geometry_position, Modifiers::default()));
    assert_eq!(geometry_editor.selection(), &[geometry_item]);

    let datum_position = scene.viewport.model_to_screen([7.0, 0.0]);
    let datum_item = SelectionItem::Datum(SketchDatum::XAxis);
    let mut datum_editor = ConstraintEditor::default();
    assert_eq!(
        datum_editor.pointer_move(&scene, pointer(11, datum_position, Modifiers::default()),),
        hover_change(
            Some(EditorHoverTarget::Geometry(datum_item)),
            Some(datum_item),
        ),
    );
    let _ = datum_editor.pointer_down(&scene, pointer(11, datum_position, Modifiers::default()));
    assert_eq!(datum_editor.selection(), &[datum_item]);

    let miss = scene.viewport.model_to_screen([7.0, 3.0]);
    let mut empty_editor = ConstraintEditor::default();
    assert!(
        empty_editor
            .pointer_move(&scene, pointer(12, miss, Modifiers::default()))
            .is_empty(),
    );
    assert!(
        empty_editor
            .pointer_down(&scene, pointer(12, miss, Modifiers::default()))
            .is_empty(),
    );
    assert!(empty_editor.selection().is_empty());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn fillet_radius_beats_overlapping_draggable_native_geometry_for_every_modifier() {
    let fixture = parity_fixture();
    let point_item = SelectionItem::Point(fixture.overlap_point);
    assert_eq!(
        fixture
            .computed_scene
            .hit_test(fixture.overlap, PickTolerance::default())
            .map(|hit| hit.item),
        Some(point_item),
        "the ordinary geometry surface must contain the draggable overlap point",
    );
    let mut without_point = fixture.computed_scene.clone();
    without_point
        .points
        .retain(|point| point.id != fixture.overlap_point);
    assert_eq!(
        without_point
            .native_authoring_hit_test(fixture.overlap, PickTolerance::default())
            .map(|hit| hit.item),
        Some(SelectionItem::Curve(fixture.first_span)),
        "the same radius sample must also overlap its passive native parent",
    );

    for (pointer_id, modifiers) in [
        (2, Modifiers::default()),
        (
            3,
            Modifiers {
                shift: true,
                control: true,
                command: true,
            },
        ),
    ] {
        let corner_item = SelectionItem::FeatureCorner(fixture.owner);
        let mut editor = ConstraintEditor::default();
        assert_eq!(
            editor.pointer_move(
                &fixture.computed_scene,
                pointer(pointer_id, fixture.overlap, modifiers),
            ),
            hover_change(
                Some(EditorHoverTarget::Geometry(corner_item)),
                Some(corner_item),
            )
        );
        assert_eq!(
            editor.pointer_down(
                &fixture.computed_scene,
                pointer(pointer_id, fixture.overlap, modifiers),
            ),
            vec![EditorEffect::SelectionChanged(vec![corner_item])],
        );
        assert_eq!(editor.selection(), &[corner_item]);
        assert_eq!(
            editor.active_pointer_gesture(),
            Some(ActivePointerGesture {
                pointer_id,
                kind: ActivePointerGestureKind::FilletRadius,
            })
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one parity row proves passive overlap priority and pointer-move state neutrality"
)]
fn annotation_precedes_passive_geometry_and_pointer_move_is_state_neutral() {
    let mut fixture = parity_fixture();
    let mut passive_scene = fixture.computed_scene.clone();
    passive_scene
        .points
        .retain(|point| point.id != fixture.overlap_point);
    passive_scene
        .curves
        .retain(|curve| curve.span != fixture.circle_span);
    passive_scene.fillet_affordances.clear();
    for curve in &mut passive_scene.computed_curves {
        curve.radius_rail = None;
    }
    let annotation_item = passive_scene
        .annotations
        .iter()
        .map(|annotation| annotation.item)
        .min()
        .expect("accepted annotation");
    passive_scene
        .annotations
        .retain(|annotation| annotation.item == annotation_item);
    passive_scene.annotations[0].visibility = SceneAnnotationVisibility::Always;
    passive_scene.annotations[0].geometry = SceneAnnotationGeometry::Label {
        anchor: fixture.overlap,
    };
    assert_eq!(
        passive_scene
            .native_authoring_hit_test(fixture.overlap, PickTolerance::default())
            .map(|hit| hit.item),
        Some(SelectionItem::Curve(fixture.first_span)),
    );
    assert!(
        passive_scene.computed_curves.iter().any(|curve| curve
            .screen_polyline
            .iter()
            .any(|point| screen_distance(*point, fixture.overlap) <= 1.0e-9)),
        "the annotation sample must overlap passive computed geometry",
    );
    assert!(passive_scene.datums.iter().any(|datum| {
        datum.datum == SketchDatum::XAxis
            && (datum.screen_start.y - fixture.overlap.y).abs() <= f64::EPSILON
    }));
    let occurrence = SceneAnnotationOccurrence {
        item: annotation_item,
        marker_index: None,
    };
    let mut annotation_editor = ConstraintEditor::default();
    let all_modifiers = Modifiers {
        shift: true,
        control: true,
        command: true,
    };
    assert_eq!(
        annotation_editor.pointer_move(&passive_scene, pointer(4, fixture.overlap, all_modifiers),),
        hover_change(Some(EditorHoverTarget::Annotation(occurrence)), None),
    );
    assert_eq!(
        annotation_editor.pointer_down(&passive_scene, pointer(4, fixture.overlap, all_modifiers),),
        vec![EditorEffect::SelectionChanged(vec![annotation_item])],
    );
    assert_eq!(annotation_editor.selection(), &[annotation_item]);
    assert!(annotation_editor.active_pointer_gesture().is_none());

    let retained_selection = vec![SelectionItem::Curve(fixture.first_span)];
    fixture
        .coordinator
        .editor_mut()
        .set_selection(retained_selection.iter().copied());
    let before_history = fixture.coordinator.history_len();
    let before_cursor = fixture.coordinator.history_cursor();
    let before_design = fixture.coordinator.session().design_identity();
    let before_accepted = fixture
        .coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted before move")
        .identity();
    let before_features = fixture.coordinator.feature_document().identity();
    let before_preview = fixture
        .coordinator
        .feature_authoring_preview()
        .expect("preview before move")
        .metadata()
        .clone();
    let before_authoring = fixture.authoring.clone();
    let before_branch_preview = fixture.coordinator.editor().fillet_branch_preview();
    let before_continuation = fixture
        .coordinator
        .editor()
        .computed_fillet_continuation_status()
        .cloned();
    assert_eq!(
        fixture.coordinator.editor_mut().pointer_move(
            &fixture.computed_scene,
            pointer(5, fixture.overlap, all_modifiers),
        ),
        hover_change(
            Some(EditorHoverTarget::Geometry(SelectionItem::FeatureCorner(
                fixture.owner,
            ))),
            Some(SelectionItem::FeatureCorner(fixture.owner)),
        ),
    );
    assert_eq!(fixture.coordinator.history_len(), before_history);
    assert_eq!(fixture.coordinator.history_cursor(), before_cursor);
    assert_eq!(
        fixture.coordinator.session().design_identity(),
        before_design
    );
    assert_eq!(
        fixture
            .coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted after move")
            .identity(),
        before_accepted,
    );
    assert_eq!(
        fixture.coordinator.feature_document().identity(),
        before_features
    );
    assert_eq!(
        fixture
            .coordinator
            .feature_authoring_preview()
            .expect("preview after move")
            .metadata(),
        &before_preview,
    );
    assert_eq!(fixture.authoring, before_authoring);
    assert_eq!(fixture.coordinator.editor().selection(), retained_selection);
    assert!(
        fixture
            .coordinator
            .editor()
            .active_pointer_gesture()
            .is_none()
    );
    assert_eq!(
        fixture.coordinator.editor().fillet_branch_preview(),
        before_branch_preview,
    );
    assert_eq!(
        fixture
            .coordinator
            .editor()
            .computed_fillet_continuation_status(),
        before_continuation.as_ref(),
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn exact_annotation_ties_ignore_scene_order_and_choose_the_first_occurrence() {
    let fixture = parity_fixture();
    let mut scene = fixture.base_scene.clone();
    let mut items = scene
        .annotations
        .iter()
        .map(|annotation| annotation.item)
        .collect::<Vec<_>>();
    items.sort_unstable();
    items.dedup();
    let items = items.into_iter().take(2).collect::<Vec<_>>();
    assert_eq!(items.len(), 2, "fixture must contain competing annotations");

    let probe = ScreenPoint { x: 40.0, y: 40.0 };
    let tolerance = PickTolerance::default().annotation_pixels;
    scene
        .annotations
        .retain(|annotation| items.contains(&annotation.item));
    for annotation in &mut scene.annotations {
        annotation.visibility = SceneAnnotationVisibility::Always;
        annotation.geometry = SceneAnnotationGeometry::Glyph {
            markers: vec![
                SceneGlyphMarker {
                    anchor: ScreenPoint {
                        x: probe.x - tolerance,
                        y: probe.y,
                    },
                    leader_from: None,
                },
                SceneGlyphMarker {
                    anchor: ScreenPoint {
                        x: probe.x + tolerance,
                        y: probe.y,
                    },
                    leader_from: None,
                },
            ],
        };
    }
    let expected_item = items[0];
    let expected_occurrence = SceneAnnotationOccurrence {
        item: expected_item,
        marker_index: Some(0),
    };

    for reverse_scene_order in [false, true] {
        let mut ordered_scene = scene.clone();
        if reverse_scene_order {
            ordered_scene.annotations.reverse();
        }
        let mut editor = ConstraintEditor::default();
        assert_eq!(
            editor.pointer_move(&ordered_scene, pointer(20, probe, Modifiers::default()),),
            hover_change(
                Some(EditorHoverTarget::Annotation(expected_occurrence)),
                None,
            ),
            "distance ties must resolve by semantic item and then marker occurrence",
        );
        let _ = editor.pointer_down(&ordered_scene, pointer(20, probe, Modifiers::default()));
        assert_eq!(
            editor.selection(),
            &[expected_item],
            "pointer-down must consume the same semantic tie winner as hover",
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn pointer_context_is_revoked_by_every_scene_or_input_owner_lifecycle() {
    let mut fixture = parity_fixture();
    let point_item = SelectionItem::Point(fixture.overlap_point);
    let expected = EditorHoverState {
        target: Some(EditorHoverTarget::Geometry(point_item)),
        context_owner: Some(point_item),
    };
    let cleared = hover_change(None, None);
    let input = pointer(21, fixture.overlap, Modifiers::default());
    let prime = |editor: &mut ConstraintEditor| {
        assert_eq!(
            editor.pointer_move(&fixture.base_scene, input),
            hover_change(
                Some(EditorHoverTarget::Geometry(point_item)),
                Some(point_item)
            ),
        );
        assert_eq!(editor.hover_state(), expected);
    };

    let mut tool_editor = ConstraintEditor::default();
    prime(&mut tool_editor);
    assert_eq!(tool_editor.activate_tool(EditorTool::Line), cleared);
    assert_eq!(tool_editor.hover_state(), EditorHoverState::default());

    let mut leave_editor = ConstraintEditor::default();
    prime(&mut leave_editor);
    assert_eq!(leave_editor.pointer_leave(), cleared);
    assert_eq!(leave_editor.hover_state(), EditorHoverState::default());

    let mut cancel_editor = ConstraintEditor::default();
    prime(&mut cancel_editor);
    assert_eq!(cancel_editor.cancel(), cleared);
    assert_eq!(cancel_editor.hover_state(), EditorHoverState::default());

    let mut camera_editor = ConstraintEditor::default();
    prime(&mut camera_editor);
    assert_eq!(camera_editor.invalidate_draft_inference(), cleared);
    assert_eq!(camera_editor.hover_state(), EditorHoverState::default());

    let mut policy_editor = ConstraintEditor::default();
    prime(&mut policy_editor);
    assert_eq!(
        policy_editor.set_geometry_pick_scope(GeometryPickScope::Profile),
        cleared,
    );
    assert_eq!(policy_editor.hover_state(), EditorHoverState::default());

    prime(fixture.coordinator.editor_mut());
    fixture
        .coordinator
        .apply_edit(
            fixture.coordinator.session().design_identity(),
            DocumentEdit::CreatePoint {
                label: "accepted scene replacement".into(),
                position: [8.0, 8.0],
            },
        )
        .expect("accepted scene replacement");
    assert_eq!(
        fixture.coordinator.editor().hover_state(),
        EditorHoverState::default(),
        "retained publication must not preserve hover from the replaced accepted scene",
    );
}
