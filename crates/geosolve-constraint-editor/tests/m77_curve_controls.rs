// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ActivePointerGestureKind, ConicConstructionOptions, ConstraintEditor, ConstructionProposal,
    EditorEffect, EditorHoverTarget, EditorTool, Modifiers, PickTolerance, PointerInput,
    SceneCurveControlGripGeometry, SceneCurveControlGuideKind, SceneCurveControlHit,
    SceneCurveControlInteraction, SceneCurveControlRole, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DocumentArcSweep, DocumentBSplineForm,
    DocumentCurveControlKind, DocumentHyperbolaBranch, DocumentRationalConicControlMode,
    DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
};

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn accepted_scene(document: &SketchDocument) -> geosolve_constraint_editor::EditorScene {
    accepted_scene_with_viewport(
        document,
        Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
    )
}

fn accepted_scene_with_viewport(
    document: &SketchDocument,
    viewport: Viewport,
) -> geosolve_constraint_editor::EditorScene {
    #[allow(clippy::default_trait_access)]
    let session = RetainedSketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .expect("retained session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted state");
    geosolve_constraint_editor::EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        session.design_document(),
        viewport,
        0.5,
    )
    .expect("scene")
    .with_retained_session(&session)
    .expect("authenticated scene")
}

fn assert_grip_outline_hit(
    scene: &geosolve_constraint_editor::EditorScene,
    control: &geosolve_constraint_editor::SceneCurveControl,
    sample: ScreenPoint,
    expected: bool,
    label: &str,
) {
    let tolerance = PickTolerance {
        point_pixels: 0.0,
        curve_pixels: 0.0,
        annotation_pixels: 0.0,
    };
    let actual = scene
        .curve_control_hit_test(sample, tolerance)
        .is_some_and(|hit| hit.control() == control.id);
    assert_eq!(actual, expected, "{label}: sample={sample:?}");
}

fn circle_document() -> (SketchDocument, CurveId, geosolve_sketch::DesignPointId) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let center = document.add_point("center", [0.0, 0.0]).expect("center");
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("radius");
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .expect("circle");
    (document, circle, center)
}

#[test]
fn selected_only_circle_cage_owns_exact_hover_click_and_point_alias() {
    let (document, circle, center) = circle_document();
    let span = CurveSpan::line(circle);
    let mut scene = accepted_scene(&document);
    let mut editor = ConstraintEditor::default();

    editor
        .populate_curve_controls(&mut scene)
        .expect("empty selection cage");
    assert!(scene.curve_controls.is_empty());

    editor.set_selection([SelectionItem::Curve(span)]);
    editor
        .populate_curve_controls(&mut scene)
        .expect("selected cage");
    assert_eq!(scene.curve_controls.len(), 2);
    let radius = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
        .expect("radius control");
    assert_eq!(radius.role, SceneCurveControlRole::Size);
    assert!(radius.rail.is_some());
    assert_eq!(radius.accessible_name, "Radius — circle");
    assert!(matches!(
        scene.curve_control_hit_test(radius.screen_position, PickTolerance::default()),
        Some(SceneCurveControlHit::Direct { control, owner, .. })
            if control == radius.id && owner == span
    ));

    let hover = editor.pointer_move(&scene, pointer(41, radius.screen_position));
    assert!(matches!(
        hover.as_slice(),
        [EditorEffect::HoverChanged(state)]
            if state.target == Some(EditorHoverTarget::CurveControl {
                control: radius.id,
                owner: span,
            })
    ));
    assert!(
        editor
            .pointer_down(&scene, pointer(41, radius.screen_position))
            .is_empty()
    );
    assert_eq!(
        editor.active_pointer_gesture().map(|gesture| gesture.kind),
        Some(ActivePointerGestureKind::CurveControl)
    );
    assert_eq!(editor.selection(), [SelectionItem::Curve(span)]);
    assert!(
        editor
            .pointer_up(
                &scene,
                scene.design_identity,
                pointer(41, radius.screen_position)
            )
            .is_empty()
    );

    let center_control = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Center)
        .expect("center alias");
    assert!(
        editor
            .pointer_down(&scene, pointer(42, center_control.screen_position))
            .is_empty()
    );
    assert_eq!(
        editor.active_pointer_gesture().map(|gesture| gesture.kind),
        Some(ActivePointerGestureKind::Point)
    );
    assert_eq!(editor.selection(), [SelectionItem::Curve(span)]);
    assert_eq!(editor.hovered(), Some(SelectionItem::Curve(span)));
    assert!(
        editor
            .pointer_up(
                &scene,
                scene.design_identity,
                pointer(42, center_control.screen_position),
            )
            .is_empty()
    );
    assert!(document.point(center).is_some());

    editor.set_selection([SelectionItem::Curve(span), SelectionItem::Point(center)]);
    editor
        .populate_curve_controls(&mut scene)
        .expect("multiple selection clears cage");
    assert!(scene.curve_controls.is_empty());
    editor.set_selection([SelectionItem::Curve(span)]);
    editor.activate_tool(EditorTool::Line);
    editor
        .populate_curve_controls(&mut scene)
        .expect("tool switch clears cage");
    assert!(scene.curve_controls.is_empty());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one zoom-independent geometry matrix keeps all three published grip primitives explicit"
)]
fn published_circle_square_and_diamond_grip_outlines_are_the_exact_zero_fringe_hit_geometry() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let circle_center = document
        .add_point("circle center", [0.0, 0.0])
        .expect("circle center");
    let circle_radius = document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("circle radius");
    let circle = document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .expect("circle");
    let rational_start = document
        .add_point("rational start", [5.0, 0.0])
        .expect("rational start");
    let rational_end = document
        .add_point("rational end", [9.0, 0.0])
        .expect("rational end");
    let rational_weight = document
        .add_scalar(
            "rational weight",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .expect("rational weight");
    let rational = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [3.5, 1.5],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .expect("rational");

    for zoom in [5.0, 50.0, 500.0] {
        let viewport = Viewport::new([1000.0, 700.0], [4.0, 1.0], zoom).expect("outline viewport");
        let mut scene = accepted_scene_with_viewport(&document, viewport);
        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
        editor
            .populate_curve_controls(&mut scene)
            .expect("circle grip catalog");
        let circle_grip = scene
            .curve_controls
            .iter()
            .find(|control| control.id.kind == DocumentCurveControlKind::Center)
            .expect("published circle grip")
            .clone();
        let SceneCurveControlGripGeometry::Circle {
            center,
            radius_pixels,
        } = circle_grip.grip
        else {
            panic!("stored point alias did not publish a circle grip")
        };
        for (name, dy, expected) in [
            ("circle inside", radius_pixels * 0.75, true),
            ("circle boundary", radius_pixels, true),
            ("circle outside", radius_pixels + 0.25, false),
        ] {
            assert_grip_outline_hit(
                &scene,
                &circle_grip,
                ScreenPoint {
                    x: center.x,
                    y: center.y + dy,
                },
                expected,
                &format!("{name} at zoom {zoom}"),
            );
        }

        let diamond = scene
            .curve_controls
            .iter()
            .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
            .expect("published diamond grip")
            .clone();
        let SceneCurveControlGripGeometry::Diamond {
            center,
            radius_pixels,
        } = diamond.grip
        else {
            panic!("radius did not publish a diamond grip")
        };
        for (name, diagonal, expected) in [
            ("diamond inside", radius_pixels * 0.2, true),
            ("diamond boundary", radius_pixels * 0.5, true),
            ("diamond outside", radius_pixels * 0.5 + 0.25, false),
        ] {
            assert_grip_outline_hit(
                &scene,
                &diamond,
                ScreenPoint {
                    x: center.x + diagonal,
                    y: center.y + diagonal,
                },
                expected,
                &format!("{name} at zoom {zoom}"),
            );
        }

        editor.set_selection([SelectionItem::Curve(CurveSpan::line(rational))]);
        editor
            .populate_curve_controls(&mut scene)
            .expect("rational grip catalog");
        let square = scene
            .curve_controls
            .iter()
            .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
            .expect("published square grip")
            .clone();
        let SceneCurveControlGripGeometry::Square {
            center,
            half_extent_pixels,
        } = square.grip
        else {
            panic!("ordinary rational middle did not publish a square grip")
        };
        for (name, diagonal, expected) in [
            ("square inside", half_extent_pixels * 0.75, true),
            ("square boundary", half_extent_pixels, true),
            ("square outside", half_extent_pixels + 0.25, false),
        ] {
            assert_grip_outline_hit(
                &scene,
                &square,
                ScreenPoint {
                    x: center.x + diagonal,
                    y: center.y + diagonal,
                },
                expected,
                &format!("{name} at zoom {zoom}"),
            );
        }
    }
}

#[test]
fn stale_exact_curve_span_cannot_publish_hit_or_edit_controls() {
    let (document, circle, _) = circle_document();
    let stale_circle_span = CurveSpan {
        curve: circle,
        segment: 1,
    };
    assert!(
        !document
            .curve_spans(circle)
            .unwrap()
            .contains(&stale_circle_span)
    );
    let mut scene = accepted_scene(&document);
    let mut editor = ConstraintEditor::default();
    editor.set_selection([SelectionItem::Curve(stale_circle_span)]);
    editor
        .populate_curve_controls(&mut scene)
        .expect("stale circle span is a non-authoritative selection");
    assert!(scene.curve_controls.is_empty());
    assert!(scene.curve_control_guides.is_empty());

    let would_be_radius = scene.viewport.model_to_screen([2.0, 0.0]);
    assert!(!matches!(
        editor.pointer_move(&scene, pointer(51, would_be_radius)).as_slice(),
        [EditorEffect::HoverChanged(state)]
            if matches!(state.target, Some(EditorHoverTarget::CurveControl { .. }))
    ));
    editor.pointer_down(&scene, pointer(51, would_be_radius));
    assert_ne!(
        editor.active_pointer_gesture().map(|gesture| gesture.kind),
        Some(ActivePointerGestureKind::CurveControl)
    );

    let mut spline_document = SketchDocument::new(10.0).expect("spline document");
    let controls = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
        .map(|position| spline_document.add_point("control", position).unwrap());
    let spline = spline_document
        .add_curve(
            "spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
                span_ids: vec![17, 29],
                next_span_id: 30,
            },
        )
        .expect("spline");
    let retired_span = CurveSpan {
        curve: spline,
        segment: 23,
    };
    assert!(
        !spline_document
            .curve_spans(spline)
            .unwrap()
            .contains(&retired_span)
    );
    let mut spline_scene = accepted_scene(&spline_document);
    editor.set_selection([SelectionItem::Curve(retired_span)]);
    editor
        .populate_curve_controls(&mut spline_scene)
        .expect("retired spline span is a non-authoritative selection");
    assert!(spline_scene.curve_controls.is_empty());
    assert!(spline_scene.curve_control_guides.is_empty());
}

#[test]
fn size_rail_preserves_grab_offset_threshold_last_valid_and_exact_release_request() {
    let (document, circle, _) = circle_document();
    let span = CurveSpan::line(circle);
    let mut scene = accepted_scene(&document);
    let mut editor = ConstraintEditor::default();
    editor.set_selection([SelectionItem::Curve(span)]);
    editor
        .populate_curve_controls(&mut scene)
        .expect("selected cage");
    let radius = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
        .expect("radius")
        .clone();
    let press = ScreenPoint {
        x: radius.screen_position.x,
        y: radius.screen_position.y + 4.0,
    };
    editor.pointer_down(&scene, pointer(71, press));

    assert!(
        editor
            .pointer_move(
                &scene,
                pointer(
                    71,
                    ScreenPoint {
                        x: press.x + 2.0,
                        y: press.y,
                    },
                ),
            )
            .is_empty()
    );
    let moved = ScreenPoint {
        x: press.x + 5.0,
        y: press.y + 7.0,
    };
    let request = editor.pointer_move(&scene, pointer(71, moved));
    let [
        EditorEffect::RequestCurveControlPreview {
            pointer_id,
            request_id,
            expected,
            control,
            model_position,
        },
    ] = request.as_slice()
    else {
        panic!("curve-control request expected: {request:?}")
    };
    assert_eq!(*pointer_id, 71);
    assert_eq!(*expected, scene.design_identity);
    assert_eq!(*control, radius.id);
    assert!((model_position[0] - 2.1).abs() <= 1.0e-12);
    assert_eq!(model_position[1].to_bits(), 0.0f64.to_bits());

    assert_eq!(
        editor.curve_control_preview_result(
            71,
            *request_id,
            *expected,
            *control,
            Some(*model_position),
        ),
        vec![EditorEffect::PreviewCurveControl {
            control: *control,
            model_position: *model_position,
        }]
    );

    let invalid = ScreenPoint {
        x: press.x - 110.0,
        y: press.y,
    };
    assert!(editor.pointer_move(&scene, pointer(71, invalid)).is_empty());
    assert_eq!(
        editor.pointer_up(&scene, scene.design_identity, pointer(71, invalid)),
        vec![EditorEffect::CommitCurveControl {
            expected: scene.design_identity,
            pointer_id: 71,
            request_id: *request_id,
            control: *control,
        }]
    );
}

fn rational_scene(weight: f64) -> (SketchDocument, CurveId) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [2.0, 0.0]).expect("end");
    let weight = document
        .add_scalar(
            "weight",
            weight,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: geosolve_sketch::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .expect("weight");
    let curve = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [1.0, 2.0],
                middle_weight: weight,
                end,
            },
        )
        .expect("rational");
    (document, curve)
}

#[test]
fn zero_weight_scene_uses_explicit_projective_vector_and_its_painted_guide_hits() {
    let (document, curve) = rational_scene(0.0);
    let mut scene = accepted_scene(&document);
    let mut editor = ConstraintEditor::default();
    editor.set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    editor
        .populate_curve_controls(&mut scene)
        .expect("projective cage");
    let middle = scene
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
        .expect("middle");
    assert_eq!(middle.role, SceneCurveControlRole::ProjectiveVector);
    assert_eq!(middle.accessible_name, "Projective middle Qh — rational");
    assert!(matches!(
        middle.target,
        geosolve_sketch::DocumentCurveControlTarget::RationalMiddle {
            mode: DocumentRationalConicControlMode::Projective,
            ..
        }
    ));
    assert!(
        scene
            .curve_control_guides
            .iter()
            .all(|guide| guide.kind != SceneCurveControlGuideKind::ControlPolygon)
    );
    assert_eq!(
        scene
            .curve_control_guides
            .iter()
            .filter(|guide| guide.kind == SceneCurveControlGuideKind::ProjectiveVector)
            .count(),
        1
    );
    let half_vector = scene.viewport.model_to_screen([0.5, 1.0]);
    assert!(matches!(
        scene.curve_control_hit_test(half_vector, PickTolerance::default()),
        Some(SceneCurveControlHit::Direct { control, .. }) if control == middle.id
    ));
}

#[test]
fn rational_construction_click_is_p1_for_nonzero_weight_and_qh_tip_at_zero() {
    let document = SketchDocument::new(10.0).expect("document");
    let scene = accepted_scene(&document);
    for (weight, expected_middle) in [(2.0, [2.0, 4.0]), (-0.5, [-0.5, -1.0]), (0.0, [3.0, 2.0])] {
        let mut editor = ConstraintEditor::default();
        editor
            .set_conic_options(ConicConstructionOptions {
                middle_weight: weight,
                ..ConicConstructionOptions::default()
            })
            .expect("options");
        editor.activate_tool(EditorTool::RationalQuadraticConic);
        let mut effects = Vec::new();
        for model in [[-2.0, 0.0], [1.0, 2.0], [2.0, 0.0]] {
            effects =
                editor.pointer_down(&scene, pointer(91, scene.viewport.model_to_screen(model)));
        }
        let proposal = effects.iter().find_map(|effect| match effect {
            EditorEffect::CommitConstruction { proposal, .. } => Some(proposal),
            EditorEffect::CommitConstructionPlan { plan, .. } => Some(&plan.proposal),
            _ => None,
        });
        assert!(
            matches!(
                proposal,
                Some(ConstructionProposal::RationalQuadraticConic {
                    weighted_middle,
                    middle_weight,
                    ..
                }) if weighted_middle.map(f64::to_bits) == expected_middle.map(f64::to_bits)
                    && middle_weight.to_bits() == weight.to_bits()
            ),
            "unexpected rational construction for weight {weight}: {proposal:?}"
        );

        if weight == 0.0 {
            let proposal = proposal.expect("zero-weight proposal").clone();
            let mut constructed = SketchDocument::new(10.0).expect("constructed document");
            let result = proposal.apply(&mut constructed).expect("zero-weight conic");
            let curve = result.curves[0];
            let middle = constructed
                .curve_controls(curve)
                .expect("zero-weight controls")
                .into_iter()
                .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
                .expect("projective middle");
            assert_eq!(
                middle.position.map(f64::to_bits),
                [1.0, 2.0].map(f64::to_bits),
                "the later Qh vector tip must coincide with the construction click",
            );
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one headless gallery freezes every family-specific M77 cage topology"
)]
fn advanced_family_cages_publish_exact_controls_guides_and_point_alias_ownership() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let add_points = |document: &mut SketchDocument, positions: &[[f64; 2]]| {
        positions
            .iter()
            .map(|position| document.add_point("control", *position).unwrap())
            .collect::<Vec<_>>()
    };
    let ratio_domain = ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    };

    let ellipse_points = add_points(&mut document, &[[0.0, 0.0], [3.0, 0.0]]);
    let ratio = document
        .add_scalar("ratio", 0.5, ScalarUnit::Parameter, ratio_domain)
        .unwrap();
    let ellipse_start = document
        .add_scalar("start", 0.2, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let ellipse_end = document
        .add_scalar("end", 1.4, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let elliptical_arc = document
        .add_curve(
            "elliptical arc",
            CurveDefinition::EllipticalArc {
                center: ellipse_points[0],
                major_axis_point: ellipse_points[1],
                minor_axis_ratio: ratio,
                start_angle: ellipse_start,
                end_angle: ellipse_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();

    let parabola_points = add_points(&mut document, &[[6.0, 0.0], [7.0, 0.0]]);
    let parabola_start = document
        .add_scalar("start", -0.8, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let parabola_end = document
        .add_scalar("end", 1.1, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let parabola = document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex: parabola_points[0],
                focus: parabola_points[1],
                trim_start: parabola_start,
                trim_end: parabola_end,
            },
        )
        .unwrap();

    let hyperbola_points = add_points(&mut document, &[[12.0, 0.0], [14.0, 0.5]]);
    let conjugate = document
        .add_scalar("conjugate", 1.5, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let hyperbola_start = document
        .add_scalar("start", -0.7, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let hyperbola_end = document
        .add_scalar("end", 0.9, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let hyperbola = document
        .add_curve(
            "hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_points[0],
                transverse_axis_point: hyperbola_points[1],
                semi_conjugate: conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .unwrap();

    let cubic_points = add_points(
        &mut document,
        &[[18.0, 0.0], [19.0, 2.0], [21.0, 2.0], [22.0, 0.0]],
    );
    let cubic = document
        .add_curve(
            "cubic",
            CurveDefinition::CubicBezier {
                controls: cubic_points.clone().try_into().unwrap(),
            },
        )
        .unwrap();
    let knots = vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0];
    let bspline = document
        .add_curve(
            "B-spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: cubic_points.clone(),
                knots: knots.clone(),
                span_ids: vec![31, 37],
                next_span_id: 38,
            },
        )
        .unwrap();
    let weights = [1.0, 0.8, 1.2, 1.0]
        .map(|value| {
            document
                .add_scalar(
                    "weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .to_vec();
    let nurbs = document
        .add_curve(
            "NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: cubic_points,
                gauge_weight: weights[0],
                weights,
                knots,
                span_ids: vec![41, 43],
                next_span_id: 44,
            },
        )
        .unwrap();
    document.validate().unwrap();

    let mut scene = accepted_scene(&document);
    let mut editor = ConstraintEditor::default();
    for (curve, expected_controls, expected_guides) in [
        (
            elliptical_arc,
            5,
            vec![
                SceneCurveControlGuideKind::PrincipalAxis,
                SceneCurveControlGuideKind::MinorAxisSpoke,
                SceneCurveControlGuideKind::SizeRail,
            ],
        ),
        (parabola, 4, vec![SceneCurveControlGuideKind::FocusAxis]),
        (
            hyperbola,
            5,
            vec![
                SceneCurveControlGuideKind::PrincipalAxis,
                SceneCurveControlGuideKind::ConjugateAxisSpoke,
                SceneCurveControlGuideKind::SizeRail,
            ],
        ),
    ] {
        editor.set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
        editor.populate_curve_controls(&mut scene).unwrap();
        assert_eq!(
            scene.curve_controls.len(),
            expected_controls,
            "curve {curve}"
        );
        let actual = scene
            .curve_control_guides
            .iter()
            .map(|guide| guide.kind)
            .collect::<Vec<_>>();
        for expected in expected_guides {
            assert!(actual.contains(&expected), "curve {curve}: {actual:?}");
        }
        assert!(scene.curve_controls.iter().all(|control| {
            control.model_position.into_iter().all(f64::is_finite)
                && control.screen_position.x.is_finite()
                && control.screen_position.y.is_finite()
        }));
    }

    for curve in [cubic, bspline, nurbs] {
        let span = document.curve_spans(curve).unwrap()[0];
        editor.set_selection([SelectionItem::Curve(span)]);
        editor.populate_curve_controls(&mut scene).unwrap();
        assert_eq!(scene.curve_controls.len(), 4, "curve {curve}");
        assert!(scene.curve_controls.iter().all(|control| matches!(
            control.interaction,
            SceneCurveControlInteraction::PointAlias(_)
        )));
        assert_eq!(
            scene
                .curve_control_guides
                .iter()
                .filter(|guide| guide.kind == SceneCurveControlGuideKind::ControlPolygon)
                .count(),
            3,
            "curve {curve}"
        );
    }
}
