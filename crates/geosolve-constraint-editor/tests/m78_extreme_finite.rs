// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintEditor, ConstructionCommitPlan, ConstructionPoint, ConstructionProposal,
    DraftAuthoringInput, DraftInferenceInput, EditorEffect, EditorScene, GeometryDraftMeasurement,
    GeometryToolVariant, Modifiers, PointerInput, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentSolveRequest,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};

const POINTER_ID: u64 = 0x78f1;

fn authenticated_scene(viewport: Viewport) -> EditorScene {
    authenticated_scene_for_document(SketchDocument::new(10.0).expect("document"), viewport)
}

fn authenticated_scene_for_document(document: SketchDocument, viewport: Viewport) -> EditorScene {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted state");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        session.design_document(),
        viewport,
        0.25,
    )
    .expect("scene")
    .with_retained_session(&session)
    .expect("authenticated scene")
}

fn authenticated_scene_pair(first: Viewport, second: Viewport) -> [EditorScene; 2] {
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted state");
    [first, second].map(|viewport| {
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.25,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("authenticated scene")
    })
}

fn press(
    editor: &mut ConstraintEditor,
    scene: &EditorScene,
    model_position: [f64; 2],
) -> Vec<EditorEffect> {
    editor.pointer_down_with_draft_authoring(
        scene,
        PointerInput {
            pointer_id: POINTER_ID,
            position: scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        },
        DraftAuthoringInput {
            inference: DraftInferenceInput {
                suppressed: true,
                preferred_candidate: None,
            },
            regularized: false,
        },
    )
}

fn terminal_proposal(effects: &[EditorEffect]) -> ConstructionProposal {
    let proposals = effects
        .iter()
        .filter_map(|effect| match effect {
            EditorEffect::CommitConstruction { proposal, .. } => Some(proposal.clone()),
            EditorEffect::CommitConstructionPlan { plan, .. } => Some(plan.proposal.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [proposal] = proposals.as_slice() else {
        panic!("expected exactly one terminal construction effect: {effects:?}");
    };
    proposal.clone()
}

fn terminal_plan(effects: &[EditorEffect]) -> ConstructionCommitPlan {
    let plans = effects
        .iter()
        .filter_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { plan, .. } => Some(plan.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!("expected exactly one terminal construction plan: {effects:?}");
    };
    plan.clone()
}

fn assert_plan_solves_with_finite_geometry(plan: &ConstructionCommitPlan) {
    let mut document = SketchDocument::new(10.0).expect("document");
    plan.apply(&mut document).expect("finite plan lowering");
    assert!(
        document
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("finite plan must solve and independently validate");
    let accepted = session
        .accepted_state_for_current_input()
        .unwrap_or_else(|| {
            panic!(
                "finite plan must publish an accepted state: {plan:?}; failure={:?}",
                session.last_attempt().failure()
            )
        });
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

fn point_position(point: &ConstructionPoint) -> [f64; 2] {
    match point {
        ConstructionPoint::Existing { position, .. } | ConstructionPoint::New(position) => {
            *position
        }
    }
}

fn assert_scalar_relative(actual: f64, expected: f64) {
    assert!(actual.is_finite(), "actual value must be finite: {actual}");
    if expected == 0.0 {
        assert_eq!(actual.abs().to_bits(), 0.0f64.to_bits(), "expected zero");
    } else {
        assert!(
            (actual / expected - 1.0).abs() <= 1.0e-12,
            "actual {actual}, expected {expected}"
        );
    }
}

fn assert_point_relative(actual: [f64; 2], expected: [f64; 2]) {
    assert_scalar_relative(actual[0], expected[0]);
    assert_scalar_relative(actual[1], expected[1]);
}

#[test]
fn three_point_circle_and_arc_keep_representable_extreme_finite_centers() {
    let scene = authenticated_scene(
        Viewport::new([1_000.0, 800.0], [5.0e307, 0.0], 4.0e-306).expect("viewport"),
    );
    let clicks = [[0.0, 0.0], [1.0e308, 0.0], [5.0e307, 2.0e307]];

    for variant in [
        GeometryToolVariant::ThreePointCircle,
        GeometryToolVariant::ThreePointArc,
    ] {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        let _ = press(&mut editor, &scene, clicks[0]);
        let _ = press(&mut editor, &scene, clicks[1]);
        let effects = press(&mut editor, &scene, clicks[2]);
        let proposal = terminal_proposal(&effects);
        let (center, radius) = match &proposal {
            ConstructionProposal::Circle { center, radius } => (point_position(center), *radius),
            ConstructionProposal::CircularArc { center, start, .. } => {
                let center = point_position(center);
                (center, (start[0] - center[0]).hypot(start[1] - center[1]))
            }
            proposal => panic!("wrong extreme-finite {variant:?} proposal: {proposal:?}"),
        };
        assert_point_relative(center, [5.0e307, -5.25e307]);
        assert_scalar_relative(radius, 7.25e307);
        assert!(
            [
                center[0] - radius,
                center[0] + radius,
                center[1] - radius,
                center[1] + radius,
            ]
            .into_iter()
            .all(f64::is_finite),
            "the extreme fixture must retain a renderable finite supporting circle"
        );
        let status = editor
            .geometry_draft_status()
            .expect("terminal extreme-finite status");
        assert!(
            status
                .measurements
                .iter()
                .all(|measurement| match measurement {
                    GeometryDraftMeasurement::Length(value)
                    | GeometryDraftMeasurement::Radius(value)
                    | GeometryDraftMeasurement::Diameter(value)
                    | GeometryDraftMeasurement::AngleRadians(value)
                    | GeometryDraftMeasurement::Ratio(value) => value.is_finite(),
                    GeometryDraftMeasurement::WidthHeight { width, height } => {
                        width.is_finite() && height.is_finite()
                    }
                    GeometryDraftMeasurement::ControlCount(_) => true,
                    _ => false,
                })
        );
        assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));
    }
}

#[test]
fn diagonal_extreme_three_point_recipes_publish_locally_valid_geometry() {
    let viewport = Viewport::new([1_000.0, 800.0], [0.0, 0.0], 4.0e-306).expect("viewport");
    let scene = authenticated_scene(viewport);
    let samples = [
        [-6.5e307, -6.5e307],
        [6.5e307, 6.5e307],
        [-6.5e307, 6.5e307],
    ];
    for variant in [
        GeometryToolVariant::ThreePointCircle,
        GeometryToolVariant::ThreePointArc,
    ] {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        let _ = press(&mut editor, &scene, samples[0]);
        let _ = press(&mut editor, &scene, samples[1]);
        let effects = press(&mut editor, &scene, samples[2]);
        let proposal = terminal_proposal(&effects);
        let center = match &proposal {
            ConstructionProposal::Circle { center, .. }
            | ConstructionProposal::CircularArc { center, .. } => point_position(center),
            proposal => panic!("wrong diagonal-extreme proposal: {proposal:?}"),
        };
        assert!(center.into_iter().all(|coordinate| {
            coordinate.is_finite() && coordinate.abs() <= 8.0 * f64::EPSILON
        }));
        assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));
    }
}

#[test]
fn translated_tangent_arc_retains_the_requested_endpoint() {
    let viewport = Viewport::new([1_000.0, 800.0], [1.0e16, 2.0], 50.0).expect("viewport");
    let mut source_document = SketchDocument::new(10.0).expect("source document");
    let source_start = source_document
        .add_point("source start", [1.0e16 - 2.0, 0.0])
        .expect("source start");
    let source_end = source_document
        .add_point("source end", [1.0e16, 0.0])
        .expect("source end");
    source_document
        .add_curve(
            "source line",
            CurveDefinition::Line {
                start: source_start,
                end: source_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("source line");
    let tangent_scene = authenticated_scene_for_document(source_document.clone(), viewport);
    let mut tangent = ConstraintEditor::default();
    let _ = tangent.activate_geometry_tool(GeometryToolVariant::TangentArc);
    let requested_end = [1.0e16 + 4.0, 4.0];
    let _ = press(&mut tangent, &tangent_scene, [1.0e16, 0.0]);
    let effects = press(&mut tangent, &tangent_scene, requested_end);
    let plan = terminal_plan(&effects);
    let ConstructionProposal::CircularArc { center, sweep, .. } = &plan.proposal else {
        panic!("wrong translated Tangent Arc proposal: {:?}", plan.proposal);
    };
    assert_point_relative(point_position(center), [1.0e16, 4.0]);
    assert_eq!(*sweep, DocumentArcSweep::CounterClockwise);

    let mut candidate = source_document;
    let result = plan.apply(&mut candidate).expect("finite Tangent Arc plan");
    let created = result.construction.curves[0];
    let retained_end = candidate
        .evaluate_curve_jet(CurveSpan::line(created), 1.0)
        .expect("retained Tangent Arc endpoint")
        .position;
    let endpoint_error =
        (retained_end.x - requested_end[0]).hypot(retained_end.y - requested_end[1]);
    assert!(endpoint_error <= 1.0e-12);
    let session = RetainedSketchDocumentSession::new(
        candidate,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("extreme Tangent Arc must solve and validate");
    assert!(
        session
            .accepted_state_for_current_input()
            .expect("accepted Tangent Arc")
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn unrepresentable_derived_circle_incidence_stays_a_local_draft_issue() {
    let viewport = Viewport::new([1_000.0, 800.0], [1.0e16, 2.0], 50.0).expect("viewport");
    let scene = authenticated_scene(viewport);
    let mut circle = ConstraintEditor::default();
    let _ = circle.activate_geometry_tool(GeometryToolVariant::ThreePointCircle);
    let _ = press(&mut circle, &scene, [1.0e16, 0.0]);
    let _ = press(&mut circle, &scene, [1.0e16 + 2.0, 0.0]);
    let effects = press(&mut circle, &scene, [1.0e16, 4.0]);
    assert!(
        effects
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );
    assert_eq!(
        circle
            .geometry_draft_status()
            .expect("correction-ready circle")
            .issue,
        Some(geosolve_constraint_editor::GeometryDraftIssue::InvalidTerminalGeometry)
    );

    let mut source_document = SketchDocument::new(10.0).expect("source document");
    let source_start = source_document
        .add_point("source start", [1.0e16, -2.0])
        .expect("source start");
    let source_end = source_document
        .add_point("source end", [1.0e16, 0.0])
        .expect("source end");
    source_document
        .add_curve(
            "source line",
            CurveDefinition::Line {
                start: source_start,
                end: source_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("source line");
    let tangent_scene = authenticated_scene_for_document(source_document, viewport);
    let mut tangent = ConstraintEditor::default();
    let _ = tangent.activate_geometry_tool(GeometryToolVariant::TangentArc);
    let _ = press(&mut tangent, &tangent_scene, [1.0e16, 0.0]);
    let effects = press(&mut tangent, &tangent_scene, [1.0e16 + 2.0, 4.0]);
    assert!(
        effects
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
    );
    assert_eq!(
        tangent
            .geometry_draft_status()
            .expect("correction-ready Tangent Arc")
            .issue,
        Some(geosolve_constraint_editor::GeometryDraftIssue::InvalidTerminalGeometry)
    );
}

#[test]
fn large_finite_circle_omits_an_unrepresentable_status_diameter() {
    let scene = authenticated_scene(
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 4.0e-306).expect("viewport"),
    );
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::CenterRadiusCircle);
    let _ = press(&mut editor, &scene, [0.0, 0.0]);
    let effects = press(&mut editor, &scene, [1.0e308, 0.0]);
    let status = editor
        .geometry_draft_status()
        .expect("terminal large-circle status");

    assert!(status.measurements.iter().any(|measurement| {
        matches!(measurement, GeometryDraftMeasurement::Radius(radius) if radius.to_bits() == 1.0e308f64.to_bits())
    }));
    assert!(
        status
            .measurements
            .iter()
            .all(|measurement| { !matches!(measurement, GeometryDraftMeasurement::Diameter(_)) })
    );
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one extreme-finite regression audits every midpoint/reflection geometry recipe"
)]
fn midpoint_and_reflection_recipes_avoid_representable_overflow() {
    let horizontal_scene = authenticated_scene(
        Viewport::new([1_000.0, 800.0], [1.0e308, 0.0], 1.0e-305).expect("viewport"),
    );

    let mut diameter = ConstraintEditor::default();
    let _ = diameter.activate_geometry_tool(GeometryToolVariant::TwoPointDiameterCircle);
    let _ = press(&mut diameter, &horizontal_scene, [8.0e307, 0.0]);
    let effects = press(&mut diameter, &horizontal_scene, [1.2e308, 0.0]);
    let proposal = terminal_proposal(&effects);
    let ConstructionProposal::Circle { center, radius } = &proposal else {
        panic!("wrong diameter-circle proposal: {proposal:?}");
    };
    assert_point_relative(point_position(center), [1.0e308, 0.0]);
    assert_scalar_relative(*radius, 2.0e307);
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));

    let mut midpoint_line = ConstraintEditor::default();
    let _ = midpoint_line.activate_geometry_tool(GeometryToolVariant::MidpointLine);
    let _ = press(&mut midpoint_line, &horizontal_scene, [1.0e308, 0.0]);
    let effects = press(&mut midpoint_line, &horizontal_scene, [8.0e307, 0.0]);
    let proposal = terminal_proposal(&effects);
    let ConstructionProposal::MidpointLine { opposite, .. } = &proposal else {
        panic!("wrong midpoint-line proposal: {proposal:?}");
    };
    assert_point_relative(point_position(opposite), [1.2e308, 0.0]);
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));

    let axis_clicks = [[8.0e307, 0.0], [1.2e308, 0.0], [1.0e308, 1.0e307]];
    let mut ellipse = ConstraintEditor::default();
    let _ = ellipse.activate_geometry_tool(GeometryToolVariant::AxisEndpointsEllipse);
    let _ = press(&mut ellipse, &horizontal_scene, axis_clicks[0]);
    let _ = press(&mut ellipse, &horizontal_scene, axis_clicks[1]);
    let effects = press(&mut ellipse, &horizontal_scene, axis_clicks[2]);
    let proposal = terminal_proposal(&effects);
    let ConstructionProposal::AxisEndpointEllipse {
        center,
        minor_axis_ratio,
        ..
    } = &proposal
    else {
        panic!("wrong axis-endpoint ellipse proposal: {proposal:?}");
    };
    assert_point_relative(point_position(center), [1.0e308, 0.0]);
    assert_scalar_relative(*minor_axis_ratio, 0.5);
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));

    let mut arc = ConstraintEditor::default();
    let _ = arc.activate_geometry_tool(GeometryToolVariant::AxisEndpointsEllipticalArc);
    for click in axis_clicks {
        let _ = press(&mut arc, &horizontal_scene, click);
    }
    let _ = press(&mut arc, &horizontal_scene, [8.0e307, 0.0]);
    let effects = press(&mut arc, &horizontal_scene, [1.0e308, -1.0e307]);
    let proposal = terminal_proposal(&effects);
    let ConstructionProposal::AxisEndpointEllipticalArc { center, .. } = &proposal else {
        panic!("wrong axis-endpoint elliptical-arc proposal: {proposal:?}");
    };
    assert_point_relative(point_position(center), [1.0e308, 0.0]);
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));

    let centered_scene = authenticated_scene(
        Viewport::new([1_000.0, 800.0], [1.0e308, 1.0e308], 1.0e-305).expect("viewport"),
    );
    for (variant, clicks) in [
        (
            GeometryToolVariant::CenterRectangle,
            vec![[1.0e308, 1.0e308], [8.0e307, 8.0e307]],
        ),
        (
            GeometryToolVariant::ThreePointCenterRectangle,
            vec![[1.0e308, 1.0e308], [1.0e308, 8.0e307], [8.0e307, 8.0e307]],
        ),
    ] {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        for click in &clicks[..clicks.len() - 1] {
            let _ = press(&mut editor, &centered_scene, *click);
        }
        let effects = press(
            &mut editor,
            &centered_scene,
            *clicks.last().expect("terminal click"),
        );
        let proposal = terminal_proposal(&effects);
        let ConstructionProposal::RectangleLoop { points, .. } = proposal else {
            panic!("wrong extreme-finite rectangle proposal for {variant:?}");
        };
        assert!(
            points
                .iter()
                .all(|point| point_position(point).into_iter().all(f64::is_finite)),
            "{variant:?} must not publish non-finite derived corners"
        );
        assert!(points.iter().any(|point| {
            let position = point_position(point);
            (position[0] / 1.2e308 - 1.0).abs() <= 1.0e-12
                && (position[1] / 1.2e308 - 1.0).abs() <= 1.0e-12
        }));
        assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));
    }
}

#[test]
fn center_arc_normalizes_direction_before_applying_an_extreme_radius() {
    let [wide_scene, detail_scene] = authenticated_scene_pair(
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 1.0e-198).expect("wide viewport"),
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 1.0e202).expect("detail viewport"),
    );

    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::CenterArc);
    let _ = press(&mut editor, &wide_scene, [0.0, 0.0]);
    let _ = press(&mut editor, &wide_scene, [1.0e200, 0.0]);
    let effects = press(&mut editor, &detail_scene, [0.0, 1.0e-200]);
    let proposal = terminal_proposal(&effects);
    let ConstructionProposal::CircularArc { end, .. } = proposal else {
        panic!("wrong center-arc proposal");
    };
    assert_point_relative(end, [0.0, 1.0e200]);
    assert_plan_solves_with_finite_geometry(&terminal_plan(&effects));

    // Both zoom levels retain exactly the same authenticated scene authority.
    assert_eq!(wide_scene.accepted_revision, detail_scene.accepted_revision);
    assert_eq!(wide_scene.design_identity, detail_scene.design_identity);
}
