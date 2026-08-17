// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintEditor, ConstructionCommitPlan, ConstructionCommitToken, ConstructionPoint,
    ConstructionProposal, DraftAuthoringInput, DraftInferenceInput, DraftPointSlot, DraftSpanSlot,
    EditorEffect, EditorScene, GeometryDraftStage, GeometryToolVariant, InferredRelation,
    Modifiers, PointerInput, Viewport,
};
use geosolve_sketch::{
    DocumentArcSweep, DocumentBSplineForm, DocumentSolveRequest, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig,
};

const POINTER_ID: u64 = 0x7800;
const EPSILON: f64 = 1.0e-9;

fn authenticated_empty_scene() -> EditorScene {
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted empty session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted empty state");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 50.0).expect("viewport"),
        0.25,
    )
    .expect("empty editor scene")
    .with_retained_session(&session)
    .expect("authenticated empty editor scene")
}

fn authoring(regularized: bool) -> DraftAuthoringInput {
    DraftAuthoringInput {
        inference: DraftInferenceInput {
            suppressed: true,
            preferred_candidate: None,
        },
        regularized,
    }
}

fn press(
    editor: &mut ConstraintEditor,
    scene: &EditorScene,
    model_position: [f64; 2],
    regularized: bool,
) -> Vec<EditorEffect> {
    editor.pointer_down_with_draft_authoring(
        scene,
        PointerInput {
            pointer_id: POINTER_ID,
            position: scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        },
        authoring(regularized),
    )
}

fn move_pointer(
    editor: &mut ConstraintEditor,
    scene: &EditorScene,
    model_position: [f64; 2],
    regularized: bool,
) -> Vec<EditorEffect> {
    editor.pointer_move_with_draft_authoring(
        scene,
        PointerInput {
            pointer_id: POINTER_ID,
            position: scene.viewport.model_to_screen(model_position),
            modifiers: Modifiers::default(),
        },
        authoring(regularized),
    )
}

#[derive(Debug)]
struct TerminalConstruction {
    proposal: ConstructionProposal,
    plan: Option<ConstructionCommitPlan>,
    token: Option<ConstructionCommitToken>,
}

fn terminal_construction(effects: &[EditorEffect]) -> TerminalConstruction {
    let terminals = effects
        .iter()
        .filter_map(|effect| match effect {
            EditorEffect::CommitConstruction { proposal, .. } => Some(TerminalConstruction {
                proposal: proposal.clone(),
                plan: None,
                token: None,
            }),
            EditorEffect::CommitConstructionPlan { token, plan, .. } => {
                Some(TerminalConstruction {
                    proposal: plan.proposal.clone(),
                    plan: Some(plan.clone()),
                    token: Some(*token),
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [terminal] = terminals.as_slice() else {
        panic!("expected exactly one terminal construction effect: {effects:?}");
    };
    TerminalConstruction {
        proposal: terminal.proposal.clone(),
        plan: terminal.plan.clone(),
        token: terminal.token,
    }
}

fn construction_point_position(point: &ConstructionPoint) -> [f64; 2] {
    match point {
        ConstructionPoint::Existing { position, .. } | ConstructionPoint::New(position) => {
            *position
        }
    }
}

fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
    assert!(
        (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= EPSILON,
        "actual {actual:?}, expected {expected:?}"
    );
}

fn assert_scalar_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= EPSILON,
        "actual {actual}, expected {expected}"
    );
}

fn assert_proposal_applies_with_finite_geometry(proposal: &ConstructionProposal) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let result = proposal.apply(&mut document).expect("valid proposal");
    assert!(!result.curves.is_empty(), "recipe must create a curve");
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
    for curve in result.curves {
        for span in document.curve_spans(curve).expect("curve spans") {
            for interval in document.visible_intervals(span).expect("visible intervals") {
                let parameter = 0.5 * (interval.start + interval.end);
                let jet = document
                    .evaluate_curve_jet(span, parameter)
                    .expect("finite midpoint jet");
                assert!(jet.position.x.is_finite() && jet.position.y.is_finite());
            }
        }
    }
}

fn assert_plan_solves_with_finite_geometry(plan: &ConstructionCommitPlan) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let result = plan.apply(&mut document).expect("valid construction plan");
    assert!(!result.construction.curves.is_empty());
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
    .expect("recipe plan must solve");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted recipe plan");
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
            .is_some_and(|residual| residual.is_finite() && residual <= EPSILON)
    );
}

#[test]
fn m78_every_exact_variant_activates_with_semantic_initial_status() {
    use GeometryDraftStage as Stage;
    use GeometryToolVariant as Variant;

    let cases = [
        (Variant::SketchPoint, Stage::Point, Some(1)),
        (Variant::Segment, Stage::Start, Some(2)),
        (Variant::Polyline, Stage::Start, None),
        (Variant::MidpointLine, Stage::Center, Some(2)),
        (Variant::TwoPointAlignedRectangle, Stage::Corner, Some(2)),
        (Variant::ThreePointCornerRectangle, Stage::Corner, Some(3)),
        (Variant::CenterRectangle, Stage::Center, Some(2)),
        (Variant::ThreePointCenterRectangle, Stage::Center, Some(3)),
        (Variant::CenterRadiusCircle, Stage::Center, Some(2)),
        (
            Variant::TwoPointDiameterCircle,
            Stage::DiameterStart,
            Some(2),
        ),
        (Variant::ThreePointCircle, Stage::Start, Some(3)),
        (Variant::CenterArc, Stage::Center, Some(3)),
        (Variant::ThreePointArc, Stage::Start, Some(3)),
        (Variant::TangentArc, Stage::SourceEndpoint, Some(2)),
        (Variant::CenterAxesEllipse, Stage::Center, Some(3)),
        (
            Variant::AxisEndpointsEllipse,
            Stage::MajorAxisEndpoint,
            Some(3),
        ),
        (Variant::CenterAxesEllipticalArc, Stage::Center, Some(5)),
        (
            Variant::AxisEndpointsEllipticalArc,
            Stage::MajorAxisEndpoint,
            Some(5),
        ),
        (Variant::QuadraticBezier, Stage::Start, Some(3)),
        (Variant::CubicBezier, Stage::Start, Some(4)),
        (Variant::RationalQuadraticConic, Stage::Start, Some(3)),
        (Variant::Parabola, Stage::Vertex, Some(2)),
        (Variant::Hyperbola, Stage::Center, Some(2)),
        (Variant::OpenControlNurbs, Stage::ControlPoint, None),
        (Variant::PeriodicControlNurbs, Stage::ControlPoint, None),
    ];

    assert_eq!(cases.len(), GeometryToolVariant::ALL.len());
    let mut editor = ConstraintEditor::default();
    for (variant, stage, required_stages) in cases {
        let _ = editor.activate_geometry_tool(variant);
        assert_eq!(editor.geometry_tool_variant(), Some(variant));
        assert_eq!(editor.tool(), variant.editor_tool());
        let status = editor
            .geometry_draft_status()
            .expect("active exact tool has semantic status");
        assert_eq!(status.variant, variant);
        assert_eq!(status.stage, stage);
        assert_eq!(status.completed_stages, 0);
        assert_eq!(status.required_stages, required_stages);
        assert!(!status.can_finish);
        assert!(!status.regularized);
        assert!(status.measurements.is_empty());
    }
}

#[test]
fn m78_midpoint_line_is_one_line_with_an_atomic_midpoint_recipe() {
    let scene = authenticated_empty_scene();
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::MidpointLine);

    let first = press(&mut editor, &scene, [2.0, 3.0], false);
    assert!(
        first
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    let status = editor.geometry_draft_status().expect("midpoint draft");
    assert_eq!(status.stage, GeometryDraftStage::End);
    assert_eq!(status.completed_stages, 1);

    let terminal = terminal_construction(&press(&mut editor, &scene, [5.0, 7.0], false));
    let ConstructionProposal::MidpointLine {
        center,
        endpoint,
        opposite,
    } = &terminal.proposal
    else {
        panic!("wrong midpoint-line proposal: {:?}", terminal.proposal);
    };
    assert_point_close(construction_point_position(center), [2.0, 3.0]);
    assert_point_close(construction_point_position(endpoint), [5.0, 7.0]);
    assert_point_close(construction_point_position(opposite), [-1.0, -1.0]);

    let plan = terminal.plan.expect("midpoint relation is atomic");
    assert_eq!(
        plan.relations,
        [InferredRelation::Midpoint {
            point: DraftPointSlot::Created { point_index: 0 },
            line: DraftSpanSlot::Created {
                curve_index: 0,
                segment: 0,
            },
        }]
    );
    assert_plan_solves_with_finite_geometry(&plan);
}

#[derive(Clone, Copy)]
struct RectangleCase {
    variant: GeometryToolVariant,
    clicks: &'static [[f64; 2]],
    ordinary_corners: [[f64; 2]; 4],
    square_corners: [[f64; 2]; 4],
    center_position: Option<[f64; 2]>,
    ordinary_relation_count: usize,
}

const RECTANGLE_CASES: [RectangleCase; 4] = [
    RectangleCase {
        variant: GeometryToolVariant::TwoPointAlignedRectangle,
        clicks: &[[1.0, 1.0], [5.0, 3.0]],
        ordinary_corners: [[1.0, 1.0], [5.0, 1.0], [5.0, 3.0], [1.0, 3.0]],
        square_corners: [[1.0, 1.0], [5.0, 1.0], [5.0, 5.0], [1.0, 5.0]],
        center_position: None,
        ordinary_relation_count: 4,
    },
    RectangleCase {
        variant: GeometryToolVariant::ThreePointCornerRectangle,
        clicks: &[[1.0, 1.0], [5.0, 1.0], [5.0, 3.0]],
        ordinary_corners: [[1.0, 1.0], [5.0, 1.0], [5.0, 3.0], [1.0, 3.0]],
        square_corners: [[1.0, 1.0], [5.0, 1.0], [5.0, 5.0], [1.0, 5.0]],
        center_position: None,
        ordinary_relation_count: 3,
    },
    RectangleCase {
        variant: GeometryToolVariant::CenterRectangle,
        clicks: &[[3.0, 3.0], [5.0, 4.0]],
        ordinary_corners: [[5.0, 4.0], [1.0, 4.0], [1.0, 2.0], [5.0, 2.0]],
        square_corners: [[5.0, 5.0], [1.0, 5.0], [1.0, 1.0], [5.0, 1.0]],
        center_position: Some([3.0, 3.0]),
        ordinary_relation_count: 5,
    },
    RectangleCase {
        variant: GeometryToolVariant::ThreePointCenterRectangle,
        clicks: &[[3.0, 3.0], [3.0, 5.0], [0.0, 5.0]],
        ordinary_corners: [[0.0, 5.0], [6.0, 5.0], [6.0, 1.0], [0.0, 1.0]],
        square_corners: [[1.0, 5.0], [5.0, 5.0], [5.0, 1.0], [1.0, 1.0]],
        center_position: Some([3.0, 3.0]),
        ordinary_relation_count: 4,
    },
];

fn author_rectangle(case: RectangleCase, regularized: bool) -> TerminalConstruction {
    let scene = authenticated_empty_scene();
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(case.variant);
    let mut effects = Vec::new();
    for &position in case.clicks {
        effects = press(&mut editor, &scene, position, regularized);
    }
    terminal_construction(&effects)
}

fn assert_rectangle_case(case: RectangleCase, regularized: bool) {
    let terminal = author_rectangle(case, regularized);
    let ConstructionProposal::RectangleLoop {
        points,
        corners,
        center,
    } = &terminal.proposal
    else {
        panic!("wrong rectangle proposal: {:?}", terminal.proposal);
    };
    let actual_corners = corners.map(|index| construction_point_position(&points[index]));
    let expected_corners = if regularized {
        case.square_corners
    } else {
        case.ordinary_corners
    };
    for (actual, expected) in actual_corners.into_iter().zip(expected_corners) {
        assert_point_close(actual, expected);
    }
    assert_eq!(center.is_some(), case.center_position.is_some());
    if let (Some(center), Some(expected)) = (center, case.center_position) {
        assert_point_close(construction_point_position(&points[*center]), expected);
    }

    let side_vectors = actual_corners.map(|_| [0.0; 2]);
    let mut side_vectors = side_vectors;
    for index in 0..4 {
        let next = actual_corners[(index + 1) % 4];
        side_vectors[index] = [
            next[0] - actual_corners[index][0],
            next[1] - actual_corners[index][1],
        ];
        assert!(side_vectors[index][0].hypot(side_vectors[index][1]) > 0.0);
    }
    assert_scalar_close(
        side_vectors[0][0] * side_vectors[1][0] + side_vectors[0][1] * side_vectors[1][1],
        0.0,
    );
    assert_scalar_close(
        side_vectors[0][0] * side_vectors[2][1] - side_vectors[0][1] * side_vectors[2][0],
        0.0,
    );
    assert_scalar_close(
        side_vectors[1][0] * side_vectors[3][1] - side_vectors[1][1] * side_vectors[3][0],
        0.0,
    );
    if regularized {
        assert_scalar_close(
            side_vectors[0][0].hypot(side_vectors[0][1]),
            side_vectors[1][0].hypot(side_vectors[1][1]),
        );
    }

    let plan = terminal.plan.expect("rectangle relations are atomic");
    assert_eq!(
        plan.relations.len(),
        case.ordinary_relation_count + usize::from(regularized)
    );
    assert_eq!(
        plan.relations
            .iter()
            .filter(|relation| matches!(relation, InferredRelation::EqualLength { .. }))
            .count(),
        usize::from(regularized)
    );
    assert_plan_solves_with_finite_geometry(&plan);
}

#[test]
fn m78_all_rectangle_recipes_publish_expected_loops_and_atomic_square_relations() {
    for case in RECTANGLE_CASES {
        assert_rectangle_case(case, false);
        assert_rectangle_case(case, true);
    }
}

#[test]
fn m78_diameter_and_three_point_circles_have_analytic_centers() {
    let scene = authenticated_empty_scene();

    let mut diameter = ConstraintEditor::default();
    let _ = diameter.activate_geometry_tool(GeometryToolVariant::TwoPointDiameterCircle);
    let _ = press(&mut diameter, &scene, [1.0, 2.0], false);
    let terminal = terminal_construction(&press(&mut diameter, &scene, [5.0, 2.0], false));
    let ConstructionProposal::Circle { center, radius } = &terminal.proposal else {
        panic!("wrong diameter-circle proposal: {:?}", terminal.proposal);
    };
    assert_point_close(construction_point_position(center), [3.0, 2.0]);
    assert_scalar_close(*radius, 2.0);
    assert!(terminal.plan.is_none());
    assert_proposal_applies_with_finite_geometry(&terminal.proposal);

    let mut three_point = ConstraintEditor::default();
    let _ = three_point.activate_geometry_tool(GeometryToolVariant::ThreePointCircle);
    let _ = press(&mut three_point, &scene, [3.0, 2.0], false);
    let _ = press(&mut three_point, &scene, [1.0, 4.0], false);
    let terminal = terminal_construction(&press(&mut three_point, &scene, [-1.0, 2.0], false));
    let ConstructionProposal::Circle { center, radius } = &terminal.proposal else {
        panic!("wrong three-point-circle proposal: {:?}", terminal.proposal);
    };
    assert_point_close(construction_point_position(center), [1.0, 2.0]);
    assert_scalar_close(*radius, 2.0);
    assert!(terminal.plan.is_none());
    assert_proposal_applies_with_finite_geometry(&terminal.proposal);
}

#[test]
fn m78_center_and_three_point_arcs_preserve_explicit_sweep_semantics() {
    let scene = authenticated_empty_scene();

    let mut center_arc = ConstraintEditor::default();
    let _ = center_arc.activate_geometry_tool(GeometryToolVariant::CenterArc);
    let _ = press(&mut center_arc, &scene, [1.0, 1.0], false);
    let _ = press(&mut center_arc, &scene, [3.0, 1.0], false);
    assert_eq!(
        center_arc
            .geometry_draft_status()
            .expect("center arc status")
            .branch
            .sweep,
        Some(DocumentArcSweep::CounterClockwise)
    );
    assert!(
        center_arc
            .flip_geometry_draft_branch()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    let terminal = terminal_construction(&press(&mut center_arc, &scene, [1.0, 4.0], false));
    let ConstructionProposal::CircularArc {
        center,
        start,
        end,
        sweep,
    } = &terminal.proposal
    else {
        panic!("wrong center-arc proposal: {:?}", terminal.proposal);
    };
    assert_point_close(construction_point_position(center), [1.0, 1.0]);
    assert_point_close(*start, [3.0, 1.0]);
    assert_point_close(*end, [1.0, 3.0]);
    assert_eq!(*sweep, DocumentArcSweep::Clockwise);
    assert_proposal_applies_with_finite_geometry(&terminal.proposal);

    let mut three_point_arc = ConstraintEditor::default();
    let _ = three_point_arc.activate_geometry_tool(GeometryToolVariant::ThreePointArc);
    let _ = press(&mut three_point_arc, &scene, [3.0, 2.0], false);
    let _ = press(&mut three_point_arc, &scene, [-1.0, 2.0], false);
    let terminal = terminal_construction(&press(&mut three_point_arc, &scene, [1.0, 4.0], false));
    let ConstructionProposal::CircularArc {
        center,
        start,
        end,
        sweep,
    } = &terminal.proposal
    else {
        panic!("wrong three-point-arc proposal: {:?}", terminal.proposal);
    };
    assert_point_close(construction_point_position(center), [1.0, 2.0]);
    assert_point_close(*start, [3.0, 2.0]);
    assert_point_close(*end, [-1.0, 2.0]);
    assert_eq!(*sweep, DocumentArcSweep::CounterClockwise);
    assert_proposal_applies_with_finite_geometry(&terminal.proposal);
}

#[test]
fn m78_full_ellipse_recipes_derive_the_same_support_frame() {
    let scene = authenticated_empty_scene();

    let cases = [
        (
            GeometryToolVariant::CenterAxesEllipse,
            [[1.0, 1.0], [5.0, 1.0], [1.0, 3.0]],
            false,
        ),
        (
            GeometryToolVariant::AxisEndpointsEllipse,
            [[5.0, 1.0], [-3.0, 1.0], [1.0, 3.0]],
            true,
        ),
    ];
    for (variant, clicks, axis_endpoints) in cases {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        let _ = press(&mut editor, &scene, clicks[0], false);
        let _ = press(&mut editor, &scene, clicks[1], false);
        let terminal = terminal_construction(&press(&mut editor, &scene, clicks[2], false));
        let (center, major_axis_point, ratio) = match &terminal.proposal {
            ConstructionProposal::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } if !axis_endpoints => (
                construction_point_position(center),
                construction_point_position(major_axis_point),
                *minor_axis_ratio,
            ),
            ConstructionProposal::AxisEndpointEllipse {
                major_axis_point,
                center,
                minor_axis_ratio,
            } if axis_endpoints => (
                construction_point_position(center),
                construction_point_position(major_axis_point),
                *minor_axis_ratio,
            ),
            proposal => panic!("wrong ellipse proposal for {variant:?}: {proposal:?}"),
        };
        assert_point_close(center, [1.0, 1.0]);
        assert_point_close(major_axis_point, [5.0, 1.0]);
        assert_scalar_close(ratio, 0.5);
        assert_proposal_applies_with_finite_geometry(&terminal.proposal);
    }
}

#[test]
fn m78_elliptical_arc_recipes_project_spatial_trim_clicks_and_flip_sweep() {
    let scene = authenticated_empty_scene();
    let cases = [
        (
            GeometryToolVariant::CenterAxesEllipticalArc,
            [[1.0, 1.0], [5.0, 1.0], [1.0, 3.0], [5.0, 1.0], [1.0, 3.0]],
            false,
        ),
        (
            GeometryToolVariant::AxisEndpointsEllipticalArc,
            [[5.0, 1.0], [-3.0, 1.0], [1.0, 3.0], [5.0, 1.0], [1.0, 3.0]],
            true,
        ),
    ];
    for (variant, clicks, axis_endpoints) in cases {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        for &click in &clicks[..4] {
            let _ = press(&mut editor, &scene, click, false);
        }
        let status = editor
            .geometry_draft_status()
            .expect("elliptical-arc status");
        assert_eq!(status.stage, GeometryDraftStage::TrimEnd);
        assert_eq!(status.completed_stages, 4);
        assert_eq!(
            status.branch.sweep,
            Some(DocumentArcSweep::CounterClockwise)
        );
        assert!(
            editor
                .flip_geometry_draft_branch()
                .iter()
                .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
        );
        let terminal = terminal_construction(&press(&mut editor, &scene, clicks[4], false));
        let (center, major_axis_point, ratio, start, end, sweep) = match &terminal.proposal {
            ConstructionProposal::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } if !axis_endpoints => (
                construction_point_position(center),
                construction_point_position(major_axis_point),
                *minor_axis_ratio,
                *start_angle,
                *end_angle,
                *sweep,
            ),
            ConstructionProposal::AxisEndpointEllipticalArc {
                major_axis_point,
                center,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } if axis_endpoints => (
                construction_point_position(center),
                construction_point_position(major_axis_point),
                *minor_axis_ratio,
                *start_angle,
                *end_angle,
                *sweep,
            ),
            proposal => panic!("wrong elliptical-arc proposal for {variant:?}: {proposal:?}"),
        };
        assert_point_close(center, [1.0, 1.0]);
        assert_point_close(major_axis_point, [5.0, 1.0]);
        assert_scalar_close(ratio, 0.5);
        assert_scalar_close(start, 0.0);
        assert_scalar_close(end, std::f64::consts::FRAC_PI_2);
        assert_eq!(sweep, DocumentArcSweep::Clockwise);
        assert_proposal_applies_with_finite_geometry(&terminal.proposal);
    }
}

#[test]
fn m78_polyline_step_back_and_closure_reuse_the_first_control() {
    let scene = authenticated_empty_scene();
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::Polyline);
    let _ = press(&mut editor, &scene, [1.0, 1.0], false);
    let _ = press(&mut editor, &scene, [4.0, 1.0], false);
    let _ = press(&mut editor, &scene, [4.0, 3.0], false);
    let status = editor.geometry_draft_status().expect("polyline status");
    assert_eq!(status.completed_stages, 3);
    assert!(status.can_finish);

    assert!(
        editor
            .step_back_draft()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    let status = editor
        .geometry_draft_status()
        .expect("stepped-back polyline");
    assert_eq!(status.completed_stages, 2);
    assert!(status.can_finish);

    let _ = press(&mut editor, &scene, [1.0, 3.0], false);
    let terminal = terminal_construction(&press(&mut editor, &scene, [1.0, 1.0], false));
    let ConstructionProposal::PolylinePath { points, closed } = &terminal.proposal else {
        panic!("wrong closed-polyline proposal: {:?}", terminal.proposal);
    };
    assert!(*closed);
    assert_eq!(
        points.len(),
        3,
        "closure must not duplicate the first point"
    );
    for (point, expected) in points.iter().zip([[1.0, 1.0], [4.0, 1.0], [1.0, 3.0]]) {
        assert_point_close(construction_point_position(point), expected);
    }
    assert_proposal_applies_with_finite_geometry(&terminal.proposal);
}

#[test]
fn m78_open_and_periodic_nurbs_variants_override_legacy_form_state() {
    let scene = authenticated_empty_scene();
    let cases = [
        (
            GeometryToolVariant::OpenControlNurbs,
            DocumentBSplineForm::Periodic,
            DocumentBSplineForm::Clamped,
        ),
        (
            GeometryToolVariant::PeriodicControlNurbs,
            DocumentBSplineForm::Clamped,
            DocumentBSplineForm::Periodic,
        ),
    ];
    for (variant, configured_form, expected_form) in cases {
        let mut editor = ConstraintEditor::default();
        let mut options = editor.nurbs_options().clone();
        options.form = configured_form;
        editor.set_nurbs_options(options).expect("NURBS options");
        let _ = editor.activate_geometry_tool(variant);
        for position in [[1.0, 1.0], [2.0, 3.0], [4.0, 3.0], [5.0, 1.0]] {
            let _ = press(&mut editor, &scene, position, false);
        }
        let status = editor.geometry_draft_status().expect("NURBS status");
        assert_eq!(status.stage, GeometryDraftStage::ControlPoint);
        assert_eq!(status.completed_stages, 4);
        assert!(status.can_finish);

        let terminal = terminal_construction(&editor.complete_draft(scene.design_identity));
        let ConstructionProposal::Nurbs { controls, options } = &terminal.proposal else {
            panic!(
                "wrong NURBS proposal for {variant:?}: {:?}",
                terminal.proposal
            );
        };
        assert_eq!(controls.len(), 4);
        assert_eq!(options.form, expected_form);
        assert_eq!(options.degree, 3);
        assert_proposal_applies_with_finite_geometry(&terminal.proposal);
    }
}

#[test]
fn m78_invalid_terminal_sample_and_rejected_atomic_plan_remain_correction_ready() {
    let scene = authenticated_empty_scene();

    let mut circle = ConstraintEditor::default();
    let _ = circle.activate_geometry_tool(GeometryToolVariant::ThreePointCircle);
    let _ = press(&mut circle, &scene, [1.0, 1.0], false);
    let _ = press(&mut circle, &scene, [5.0, 1.0], false);
    let invalid = press(&mut circle, &scene, [3.0, 1.0], false);
    assert!(invalid.iter().all(|effect| !matches!(
        effect,
        EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
    )));
    let status = circle
        .geometry_draft_status()
        .expect("invalid terminal keeps prior valid draft");
    assert_eq!(status.stage, GeometryDraftStage::ThroughPoint);
    assert_eq!(status.completed_stages, 2);
    let corrected = terminal_construction(&press(&mut circle, &scene, [3.0, 3.0], false));
    assert!(matches!(
        corrected.proposal,
        ConstructionProposal::Circle { .. }
    ));

    let mut midpoint = ConstraintEditor::default();
    let _ = midpoint.activate_geometry_tool(GeometryToolVariant::MidpointLine);
    let _ = press(&mut midpoint, &scene, [2.0, 2.0], false);
    let terminal = terminal_construction(&press(&mut midpoint, &scene, [4.0, 2.0], false));
    let token = terminal.token.expect("atomic midpoint plan token");
    assert_eq!(midpoint.pending_construction_commit_token(), Some(token));
    assert!(
        midpoint
            .acknowledge_construction_commit(token, false)
            .is_empty()
    );
    assert_eq!(midpoint.pending_construction_commit_token(), None);
    let status = midpoint
        .geometry_draft_status()
        .expect("rejected terminal keeps correction-ready prefix");
    assert_eq!(status.stage, GeometryDraftStage::End);
    assert_eq!(status.completed_stages, 1);

    let replacement = move_pointer(&mut midpoint, &scene, [5.0, 3.0], false);
    assert!(replacement.iter().any(|effect| matches!(
        effect,
        EditorEffect::PreviewConstruction(
            geosolve_constraint_editor::ConstructionPreview::Complete {
                proposal: ConstructionProposal::MidpointLine { endpoint, .. },
                ..
            }
        ) if construction_point_position(endpoint) == [5.0, 3.0]
    )));
    let replacement = terminal_construction(&press(&mut midpoint, &scene, [5.0, 3.0], false));
    assert!(replacement.token.is_some());
}
