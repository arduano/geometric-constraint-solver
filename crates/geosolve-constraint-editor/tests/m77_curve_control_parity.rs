// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConicConstructionOptions, ConstraintEditor, ConstructionPreview, ConstructionProposal,
    EditorEffect, EditorScene, EditorTool, Modifiers, PointerInput, RetainedEditorCoordinator,
    SceneCurveControl, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    AlphaScenarioKind, CurveDefinition, CurveId, CurveSpan, DesignScalarId, DocumentArcSweep,
    DocumentCurveControlId, DocumentCurveControlKind, DocumentCurveControlTarget,
    DocumentHyperbolaBranch, DocumentSolveRequest, FeatureEndpoint, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDesignIdentity, SketchDocument, SolverConfig, alpha_scenario,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn screen_point_is_finite(point: ScreenPoint) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn scene(coordinator: &RetainedEditorCoordinator, viewport: Viewport) -> EditorScene {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted M77 parity state");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.25,
    )
    .expect("M77 parity scene")
    .with_retained_session(coordinator.session())
    .expect("authenticated M77 parity scene")
}

fn selected_control_scene(
    coordinator: &mut RetainedEditorCoordinator,
    viewport: Viewport,
    curve: CurveId,
    kind: DocumentCurveControlKind,
) -> (EditorScene, SceneCurveControl) {
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    let mut current = scene(coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut current)
        .expect("selected parity control cage");
    let control = current
        .curve_controls
        .iter()
        .find(|control| control.id.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} control for {curve}"))
        .clone();
    (current, control)
}

fn prepared_preview_scene(
    coordinator: &RetainedEditorCoordinator,
    viewport: Viewport,
    interaction_revision: u64,
    interaction_design: SketchDesignIdentity,
) -> EditorScene {
    let source = coordinator
        .visible_preview_session()
        .expect("accepted parity preview session");
    let accepted = source
        .accepted_state_for_current_input()
        .expect("accepted parity preview state");
    let mut preview = EditorScene::from_accepted_for_design(
        interaction_revision,
        interaction_design,
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.25,
    )
    .expect("detached parity preview scene");
    coordinator
        .editor()
        .populate_curve_controls(&mut preview)
        .expect("preview parity control cage");
    preview
}

#[derive(Clone, Copy)]
struct CurvePreviewRequest {
    request_id: u64,
    expected: SketchDesignIdentity,
    control: DocumentCurveControlId,
    model_position: [f64; 2],
}

fn one_curve_preview_request(effects: &[EditorEffect]) -> CurvePreviewRequest {
    let [
        EditorEffect::RequestCurveControlPreview {
            request_id,
            expected,
            control,
            model_position,
            ..
        },
    ] = effects
    else {
        panic!("one curve-control preview request expected: {effects:?}")
    };
    CurvePreviewRequest {
        request_id: *request_id,
        expected: *expected,
        control: *control,
        model_position: *model_position,
    }
}

fn accept_curve_preview(
    coordinator: &mut RetainedEditorCoordinator,
    pointer_id: u64,
    request: CurvePreviewRequest,
) {
    assert!(matches!(
        coordinator
            .resolve_curve_control_preview(
                pointer_id,
                request.request_id,
                request.expected,
                request.control,
                request.model_position,
            )
            .as_slice(),
        [EditorEffect::PreviewCurveControl { control, .. }] if *control == request.control
    ));
}

fn release_curve_preview(
    coordinator: &mut RetainedEditorCoordinator,
    preview: &EditorScene,
    pointer_id: u64,
    position: ScreenPoint,
    expected: SketchDesignIdentity,
) {
    let release =
        coordinator
            .editor_mut()
            .pointer_up(preview, expected, pointer(pointer_id, position));
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        panic!("one exact curve-control commit expected: {release:?}")
    };
    coordinator
        .apply_editor_effect(commit)
        .expect("curve-control commit")
        .expect("changed curve-control mutation");
}

fn retained_scalar(coordinator: &RetainedEditorCoordinator, scalar: DesignScalarId) -> f64 {
    coordinator
        .session()
        .design_document()
        .scalar(scalar)
        .expect("retained parity scalar")
        .value
}

fn preview_scalar(coordinator: &RetainedEditorCoordinator, scalar: DesignScalarId) -> f64 {
    coordinator
        .visible_preview_session()
        .expect("visible parity preview")
        .accepted_state_for_current_input()
        .expect("accepted parity preview")
        .document()
        .scalar(scalar)
        .expect("preview parity scalar")
        .value
}

fn assert_close(actual: f64, expected: f64, context: &str) {
    let tolerance = 2.0e-11 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{context}: expected {expected}, got {actual}"
    );
}

fn assert_point_close(actual: [f64; 2], expected: [f64; 2], context: &str) {
    let scale = expected[0].abs().max(expected[1].abs()).max(1.0);
    let error = (actual[0] - expected[0]).hypot(actual[1] - expected[1]);
    assert!(
        error <= 2.0e-10 * scale,
        "{context}: expected {expected:?}, got {actual:?}, error={error:e}"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscreteCurveState {
    CircularArc(DocumentArcSweep),
    EllipticalArc(DocumentArcSweep),
    Hyperbola(DocumentHyperbolaBranch),
    Continuous,
}

fn discrete_curve_state(document: &SketchDocument, curve: CurveId) -> DiscreteCurveState {
    match &document
        .curve(curve)
        .expect("curve with retained discrete state")
        .definition
    {
        CurveDefinition::CircularArc { sweep, .. } => DiscreteCurveState::CircularArc(*sweep),
        CurveDefinition::EllipticalArc { sweep, .. } => DiscreteCurveState::EllipticalArc(*sweep),
        CurveDefinition::HyperbolaSegment { branch, .. } => DiscreteCurveState::Hyperbola(*branch),
        _ => DiscreteCurveState::Continuous,
    }
}

fn preview_discrete_curve_state(
    coordinator: &RetainedEditorCoordinator,
    curve: CurveId,
) -> DiscreteCurveState {
    let document = coordinator
        .visible_preview_session()
        .expect("visible parity preview")
        .accepted_state_for_current_input()
        .expect("accepted parity preview");
    discrete_curve_state(document.document(), curve)
}

fn expected_control_kinds(definition: &CurveDefinition) -> Vec<DocumentCurveControlKind> {
    use DocumentCurveControlKind as K;

    match definition {
        CurveDefinition::Line { .. } => vec![K::StartPoint, K::EndPoint],
        CurveDefinition::Polyline { points, closed, .. } => points
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if !closed && index == 0 {
                    K::StartPoint
                } else if !closed && index + 1 == points.len() {
                    K::EndPoint
                } else {
                    K::ControlPoint {
                        ordinal: u32::try_from(index).expect("bounded parity control ordinal"),
                    }
                }
            })
            .collect(),
        CurveDefinition::Circle { .. } => vec![K::Center, K::Radius],
        CurveDefinition::CircularArc { .. } => {
            vec![K::Center, K::Radius, K::TrimStart, K::TrimEnd]
        }
        CurveDefinition::QuadraticBezier { controls } => controls
            .iter()
            .enumerate()
            .map(|(index, _)| K::ControlPoint {
                ordinal: u32::try_from(index).expect("bounded parity control ordinal"),
            })
            .collect(),
        CurveDefinition::CubicBezier { controls } => controls
            .iter()
            .enumerate()
            .map(|(index, _)| K::ControlPoint {
                ordinal: u32::try_from(index).expect("bounded parity control ordinal"),
            })
            .collect(),
        CurveDefinition::Ellipse { .. } => vec![K::Center, K::MajorAxisPoint, K::MinorAxis],
        CurveDefinition::EllipticalArc { .. } => vec![
            K::Center,
            K::MajorAxisPoint,
            K::MinorAxis,
            K::TrimStart,
            K::TrimEnd,
        ],
        CurveDefinition::RationalQuadraticConic { .. } => {
            vec![K::StartPoint, K::RationalMiddle, K::EndPoint]
        }
        CurveDefinition::ParabolaSegment { .. } => {
            vec![K::Vertex, K::Focus, K::TrimStart, K::TrimEnd]
        }
        CurveDefinition::HyperbolaSegment { .. } => vec![
            K::Center,
            K::TransverseAxisPoint,
            K::ConjugateAxis,
            K::TrimStart,
            K::TrimEnd,
        ],
        CurveDefinition::BSpline { controls, .. } | CurveDefinition::Nurbs { controls, .. } => {
            controls
                .iter()
                .enumerate()
                .map(|(index, _)| K::ControlPoint {
                    ordinal: u32::try_from(index).expect("bounded parity control ordinal"),
                })
                .collect()
        }
    }
}

const fn curve_family_name(definition: &CurveDefinition) -> &'static str {
    match definition {
        CurveDefinition::Line { .. } => "line",
        CurveDefinition::Polyline { .. } => "polyline",
        CurveDefinition::Circle { .. } => "circle",
        CurveDefinition::CircularArc { .. } => "circular-arc",
        CurveDefinition::QuadraticBezier { .. } => "quadratic-bezier",
        CurveDefinition::CubicBezier { .. } => "cubic-bezier",
        CurveDefinition::Ellipse { .. } => "ellipse",
        CurveDefinition::EllipticalArc { .. } => "elliptical-arc",
        CurveDefinition::RationalQuadraticConic { .. } => "rational-quadratic",
        CurveDefinition::ParabolaSegment { .. } => "parabola",
        CurveDefinition::HyperbolaSegment { .. } => "hyperbola",
        CurveDefinition::BSpline { .. } => "b-spline",
        CurveDefinition::Nurbs { .. } => "nurbs",
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn every_curve_family_publishes_the_same_finite_control_catalog_on_native_and_wasm() {
    let fixture = alpha_scenario(AlphaScenarioKind::ProfileAllFamilies, 1.0)
        .expect("all-family M77 parity fixture");
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("all-family retained M77 session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("M77 coordinator");
    let viewport = Viewport::new([1_400.0, 900.0], [0.0, 0.0], 6.0).expect("M77 viewport");

    let curves = coordinator.session().design_document().curves().to_vec();
    let mut families = std::collections::BTreeSet::new();
    for curve in curves {
        families.insert(curve_family_name(&curve.definition));
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(curve.id))]);
        let mut current = scene(&coordinator, viewport);
        coordinator
            .editor()
            .populate_curve_controls(&mut current)
            .expect("family control cage");
        let actual = current
            .curve_controls
            .iter()
            .map(|control| {
                assert!(control.model_position.into_iter().all(f64::is_finite));
                assert!(screen_point_is_finite(control.screen_position));
                assert!(screen_point_is_finite(control.grip.center()));
                control.id.kind
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            expected_control_kinds(&curve.definition),
            "control catalog diverged for {}",
            curve.label,
        );
        assert!(current.curve_control_guides.iter().all(|guide| {
            guide.model_start.into_iter().all(f64::is_finite)
                && guide.model_end.into_iter().all(f64::is_finite)
                && screen_point_is_finite(guide.screen_start)
                && screen_point_is_finite(guide.screen_end)
        }));
    }
    assert_eq!(
        families,
        std::collections::BTreeSet::from([
            "b-spline",
            "circle",
            "circular-arc",
            "cubic-bezier",
            "ellipse",
            "elliptical-arc",
            "hyperbola",
            "line",
            "nurbs",
            "parabola",
            "polyline",
            "quadratic-bezier",
            "rational-quadratic",
        ]),
        "the closed alpha curve-family catalog changed",
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one native/WASM transaction keeps all four spatial stages and exact projected persistence together"
)]
fn m77_f013_elliptical_arc_authoring_uses_four_spatial_projected_stages() {
    let document = SketchDocument::new(10.0).expect("authoring document");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("authoring session");
    let coordinator = RetainedEditorCoordinator::new(session).expect("authoring coordinator");
    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let current = scene(&coordinator, viewport);
    let mut editor = ConstraintEditor::default();
    editor.activate_tool(EditorTool::EllipticalArc);
    editor
        .set_conic_options(ConicConstructionOptions {
            minor_axis_ratio: 0.5,
            arc_start: 2.4,
            arc_end: -2.3,
            arc_sweep: DocumentArcSweep::Clockwise,
            ..ConicConstructionOptions::default()
        })
        .expect("spatial arc options");

    let center = [0.4, -0.3];
    let major_axis = [4.4, -0.3];
    let raw_start = [2.4, 2.7];
    let raw_end = [-2.6, -1.3];
    let clicks = [center, major_axis, raw_start, raw_end];
    let mut terminal = Vec::new();
    for (index, model) in clicks.into_iter().enumerate() {
        let effects =
            editor.pointer_down(&current, pointer(8_000, viewport.model_to_screen(model)));
        let committed = effects.iter().any(|effect| {
            matches!(
                effect,
                EditorEffect::CommitConstruction { .. }
                    | EditorEffect::CommitConstructionPlan { .. }
            )
        });
        assert_eq!(
            committed,
            index == 3,
            "elliptical arc committed at spatial stage {}: {effects:?}",
            index + 1,
        );
        if matches!(index, 1 | 2) {
            let preview = effects.iter().find_map(|effect| match effect {
                EditorEffect::PreviewConstruction(preview) => Some(preview),
                _ => None,
            });
            let Some(ConstructionPreview::EllipticalArcSupport {
                support_points,
                trim_start,
                ..
            }) = preview
            else {
                panic!(
                    "missing support-ellipse preview at stage {}: {effects:?}",
                    index + 1
                )
            };
            assert!(support_points.len() >= 16);
            assert!(
                support_points
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert_eq!(trim_start.is_some(), index == 2);
        }
        if index == 3 {
            terminal = effects;
        }
    }

    let proposal = terminal
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstruction { proposal, .. } => Some(proposal.clone()),
            EditorEffect::CommitConstructionPlan { plan, .. } => Some(plan.proposal.clone()),
            _ => None,
        })
        .expect("terminal elliptical-arc proposal");
    let ConstructionProposal::EllipticalArc {
        center: center_operand,
        major_axis_point,
        minor_axis_ratio,
        start_angle,
        end_angle,
        sweep,
        ..
    } = proposal
    else {
        panic!("elliptical-arc proposal expected")
    };
    let expected_start = (3.0_f64 / 2.0).atan2(2.0 / 4.0);
    let expected_end = (-1.0_f64 / 2.0).atan2(-3.0 / 4.0);
    assert_close(minor_axis_ratio, 0.5, "minor-axis option");
    assert_close(start_angle, expected_start, "spatial start parameter");
    assert_close(end_angle, expected_end, "spatial end parameter");
    assert_eq!(sweep, DocumentArcSweep::Clockwise);
    assert!((start_angle - 2.4).abs() > 1.0e-3);
    assert!((end_angle + 2.3).abs() > 1.0e-3);

    let proposal = ConstructionProposal::EllipticalArc {
        center: center_operand,
        major_axis_point,
        minor_axis_ratio,
        start_angle,
        end_angle,
        sweep,
    };
    let mut constructed = SketchDocument::new(10.0).expect("constructed document");
    let result = proposal
        .apply(&mut constructed)
        .expect("spatial arc applies");
    assert_eq!(
        result.points.len(),
        2,
        "trim clicks are not persistent points"
    );
    assert_eq!(result.scalars.len(), 3);
    let controls = constructed
        .curve_controls(result.curves[0])
        .expect("constructed arc controls");
    for (kind, raw) in [
        (DocumentCurveControlKind::TrimStart, raw_start),
        (DocumentCurveControlKind::TrimEnd, raw_end),
    ] {
        let endpoint = controls
            .iter()
            .find(|control| control.id.kind == kind)
            .unwrap_or_else(|| panic!("missing {kind:?}"))
            .position;
        let normalized = [
            (endpoint[0] - center[0]) / 4.0,
            (endpoint[1] - center[1]) / 2.0,
        ];
        assert_close(
            normalized[0].mul_add(normalized[0], normalized[1] * normalized[1]),
            1.0,
            "projected endpoint lies on support ellipse",
        );
        let raw_normalized = [(raw[0] - center[0]) / 4.0, (raw[1] - center[1]) / 2.0];
        assert!(normalized[0] * raw_normalized[0] + normalized[1] * raw_normalized[1] > 0.0);
        assert_close(
            normalized[0] * raw_normalized[1] - normalized[1] * raw_normalized[0],
            0.0,
            "projected endpoint retains normalized radial direction",
        );
    }
    RetainedSketchDocumentSession::new(
        constructed,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("constructed arc independently accepts");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "native/WASM parity intentionally compares the same exact projection values in one closed lifecycle"
)]
fn prepared_size_preview_and_cancellation_are_native_wasm_identical() {
    let mut document = SketchDocument::new(6.0).expect("document");
    let center = document.add_point("center", [0.0, 0.0]).expect("center");
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("radius");
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .expect("circle");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained circle");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
    let viewport = Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport");
    let mut current = scene(&coordinator, viewport);
    coordinator
        .editor()
        .populate_curve_controls(&mut current)
        .expect("circle cage");
    let control = current
        .curve_controls
        .iter()
        .find(|control| control.id.kind == DocumentCurveControlKind::Radius)
        .expect("radius control")
        .clone();

    let pointer_id = 77;
    assert!(
        coordinator
            .pointer_down(&current, pointer(pointer_id, control.screen_position))
            .is_empty()
    );
    let moved = ScreenPoint {
        x: control.screen_position.x + 50.0,
        y: control.screen_position.y,
    };
    let request = coordinator
        .editor_mut()
        .pointer_move(&current, pointer(pointer_id, moved));
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
        panic!("one typed curve-control request expected: {request:?}")
    };
    assert_eq!(*model_position, [3.0, 0.0]);
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
            model_position: [3.0, 0.0],
        }]
    );
    assert_eq!(
        coordinator
            .visible_preview_session()
            .expect("accepted preview")
            .accepted_state_for_current_input()
            .expect("accepted preview state")
            .document()
            .scalar(radius)
            .expect("preview radius")
            .value,
        3.0,
    );
    let effects = coordinator.editor_mut().cancel();
    assert_eq!(effects, vec![EditorEffect::ClearCurveControlPreview]);
    for effect in effects {
        assert!(
            coordinator
                .apply_editor_effect(&effect)
                .expect("cancel")
                .is_none()
        );
    }
    assert!(coordinator.visible_preview_session().is_none());
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .expect("retained radius")
            .value,
        2.0,
    );
}

#[derive(Clone, Copy)]
struct TrimFamily {
    name: &'static str,
    curve: CurveId,
    start: DesignScalarId,
    end: DesignScalarId,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one native/WASM lifecycle matrix keeps all four trim-capable families and both endpoint identities explicit"
)]
fn changed_trim_gestures_preserve_discrete_state_and_round_trip_history_persistence() {
    let mut document = SketchDocument::new(6.0).expect("trim parity document");

    let circular_center = document
        .add_point("circular center", [-6.0, 0.0])
        .expect("circular center");
    let circular_radius = document
        .add_scalar(
            "circular radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("circular radius");
    let circular_start = document
        .add_scalar(
            "circular start",
            1.5,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .expect("circular start");
    let circular_end = document
        .add_scalar(
            "circular end",
            -0.5,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .expect("circular end");
    let circular = document
        .add_curve(
            "clockwise circular arc",
            CurveDefinition::CircularArc {
                center: circular_center,
                radius: circular_radius,
                start_angle: circular_start,
                end_angle: circular_end,
                sweep: DocumentArcSweep::Clockwise,
            },
        )
        .expect("circular arc");

    let elliptical_center = document
        .add_point("elliptical center", [0.0, 0.0])
        .expect("elliptical center");
    let elliptical_axis = document
        .add_point("elliptical axis", [3.0, 0.0])
        .expect("elliptical axis");
    let elliptical_ratio = document
        .add_scalar(
            "elliptical ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .expect("elliptical ratio");
    let elliptical_start = document
        .add_scalar(
            "elliptical start",
            0.2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .expect("elliptical start");
    let elliptical_end = document
        .add_scalar(
            "elliptical end",
            1.5,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .expect("elliptical end");
    let elliptical = document
        .add_curve(
            "counter-clockwise elliptical arc",
            CurveDefinition::EllipticalArc {
                center: elliptical_center,
                major_axis_point: elliptical_axis,
                minor_axis_ratio: elliptical_ratio,
                start_angle: elliptical_start,
                end_angle: elliptical_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .expect("elliptical arc");

    let parabola_vertex = document
        .add_point("parabola vertex", [6.0, 0.0])
        .expect("parabola vertex");
    let parabola_focus = document
        .add_point("parabola focus", [7.0, 0.0])
        .expect("parabola focus");
    let parabola_start = document
        .add_scalar(
            "parabola start",
            -1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("parabola start");
    let parabola_end = document
        .add_scalar(
            "parabola end",
            1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("parabola end");
    let parabola = document
        .add_curve(
            "parabola segment",
            CurveDefinition::ParabolaSegment {
                vertex: parabola_vertex,
                focus: parabola_focus,
                trim_start: parabola_start,
                trim_end: parabola_end,
            },
        )
        .expect("parabola segment");

    let hyperbola_center = document
        .add_point("hyperbola center", [12.0, 0.0])
        .expect("hyperbola center");
    let hyperbola_axis = document
        .add_point("hyperbola transverse axis", [14.0, 0.0])
        .expect("hyperbola transverse axis");
    let hyperbola_conjugate = document
        .add_scalar(
            "hyperbola conjugate",
            1.2,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("hyperbola conjugate");
    let hyperbola_start = document
        .add_scalar(
            "hyperbola start",
            -0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("hyperbola start");
    let hyperbola_end = document
        .add_scalar(
            "hyperbola end",
            0.9,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("hyperbola end");
    let hyperbola = document
        .add_curve(
            "negative hyperbola segment",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate: hyperbola_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .expect("hyperbola segment");

    let families = [
        TrimFamily {
            name: "circular arc",
            curve: circular,
            start: circular_start,
            end: circular_end,
        },
        TrimFamily {
            name: "elliptical arc",
            curve: elliptical,
            start: elliptical_start,
            end: elliptical_end,
        },
        TrimFamily {
            name: "parabola",
            curve: parabola,
            start: parabola_start,
            end: parabola_end,
        },
        TrimFamily {
            name: "hyperbola",
            curve: hyperbola,
            start: hyperbola_start,
            end: hyperbola_end,
        },
    ];

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained trim parity session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("trim parity coordinator");
    let viewport = Viewport::new([1_400.0, 900.0], [4.0, 0.0], 35.0).expect("trim parity viewport");
    let mut pointer_id = 7_700;

    for family in families {
        for (kind, endpoint, scalar, support_parameter) in [
            (
                DocumentCurveControlKind::TrimStart,
                FeatureEndpoint::Start,
                family.start,
                0.25,
            ),
            (
                DocumentCurveControlKind::TrimEnd,
                FeatureEndpoint::End,
                family.end,
                0.75,
            ),
        ] {
            pointer_id += 1;
            let context = format!("{} {kind:?}", family.name);
            let origin_value = retained_scalar(&coordinator, scalar);
            let origin_discrete =
                discrete_curve_state(coordinator.session().design_document(), family.curve);
            let target_jet = coordinator
                .session()
                .design_document()
                .evaluate_curve_jet(CurveSpan::line(family.curve), support_parameter)
                .unwrap_or_else(|error| panic!("{context} support evaluation failed: {error}"));
            let target = [target_jet.position.x, target_jet.position.y];
            let expected_value = coordinator
                .session()
                .design_document()
                .project_curve_trim_endpoint(family.curve, endpoint, target)
                .unwrap_or_else(|error| panic!("{context} projection failed: {error}"))
                .value;
            assert!(
                (expected_value - origin_value).abs() > 1.0e-9,
                "{context} must be a changed trim sample"
            );

            let (current, control) =
                selected_control_scene(&mut coordinator, viewport, family.curve, kind);
            assert_eq!(control.target, DocumentCurveControlTarget::Scalar(scalar));
            assert!(
                coordinator
                    .pointer_down(&current, pointer(pointer_id, control.screen_position))
                    .is_empty(),
                "{context} pointer-down"
            );
            let target_screen = viewport.model_to_screen(target);
            let request = one_curve_preview_request(
                &coordinator
                    .editor_mut()
                    .pointer_move(&current, pointer(pointer_id, target_screen)),
            );
            assert_eq!(request.control, control.id, "{context}");
            assert_point_close(request.model_position, target, &format!("{context} sample"));
            accept_curve_preview(&mut coordinator, pointer_id, request);
            assert_close(
                preview_scalar(&coordinator, scalar),
                expected_value,
                &format!("{context} preview scalar"),
            );
            assert_eq!(
                preview_discrete_curve_state(&coordinator, family.curve),
                origin_discrete,
                "{context} preview changed explicit sweep/branch"
            );

            let preview = prepared_preview_scene(
                &coordinator,
                viewport,
                current.accepted_revision,
                current.design_identity,
            );
            let before_history = coordinator.history_len();
            release_curve_preview(
                &mut coordinator,
                &preview,
                pointer_id,
                target_screen,
                current.design_identity,
            );
            assert_eq!(
                coordinator.history_len(),
                before_history + 1,
                "{context} must add exactly one history step"
            );
            assert_close(
                retained_scalar(&coordinator, scalar),
                expected_value,
                &format!("{context} committed scalar"),
            );
            assert_eq!(
                discrete_curve_state(coordinator.session().design_document(), family.curve),
                origin_discrete,
                "{context} commit changed explicit sweep/branch"
            );

            let checkpoint = coordinator
                .persistence_checkpoint()
                .unwrap_or_else(|error| panic!("{context} checkpoint failed: {error}"));
            coordinator
                .undo()
                .unwrap_or_else(|error| panic!("{context} Undo failed: {error}"));
            assert_close(
                retained_scalar(&coordinator, scalar),
                origin_value,
                &format!("{context} Undo scalar"),
            );
            assert_eq!(
                discrete_curve_state(coordinator.session().design_document(), family.curve),
                origin_discrete,
                "{context} Undo changed explicit sweep/branch"
            );
            coordinator
                .reload(&checkpoint)
                .unwrap_or_else(|error| panic!("{context} reload failed: {error}"));
            assert_close(
                retained_scalar(&coordinator, scalar),
                expected_value,
                &format!("{context} reloaded scalar"),
            );
            assert_eq!(
                discrete_curve_state(coordinator.session().design_document(), family.curve),
                origin_discrete,
                "{context} reload changed explicit sweep/branch"
            );
            let (_, recomputed) =
                selected_control_scene(&mut coordinator, viewport, family.curve, kind);
            assert_point_close(
                recomputed.model_position,
                target,
                &format!("{context} recomputed control"),
            );
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "ellipse rejection and hyperbola zero-crossing exercise deliberately different last-valid paths"
)]
fn size_domains_retain_and_publish_only_the_last_valid_native_wasm_sample() {
    let mut document = SketchDocument::new(6.0).expect("size-domain parity document");
    let ellipse_center = document
        .add_point("ellipse center", [0.0, 0.0])
        .expect("ellipse center");
    let ellipse_axis = document
        .add_point("ellipse axis", [4.0, 0.0])
        .expect("ellipse axis");
    let ellipse_ratio = document
        .add_scalar(
            "ellipse ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .expect("ellipse ratio");
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_axis,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .expect("ellipse");

    let hyperbola_center = document
        .add_point("hyperbola center", [8.0, 0.0])
        .expect("hyperbola center");
    let hyperbola_axis = document
        .add_point("hyperbola axis", [10.0, 0.0])
        .expect("hyperbola axis");
    let hyperbola_conjugate = document
        .add_scalar(
            "hyperbola conjugate",
            1.5,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("hyperbola conjugate");
    let hyperbola_start = document
        .add_scalar(
            "hyperbola start",
            -0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("hyperbola start");
    let hyperbola_end = document
        .add_scalar(
            "hyperbola end",
            0.9,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("hyperbola end");
    let hyperbola = document
        .add_curve(
            "negative hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate: hyperbola_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .expect("hyperbola");

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained size-domain parity session");
    let mut coordinator =
        RetainedEditorCoordinator::new(session).expect("size-domain parity coordinator");
    let viewport = Viewport::new([1_200.0, 800.0], [4.0, 0.0], 50.0).expect("size-domain viewport");

    let ellipse_pointer = 7_801;
    let (ellipse_scene, ellipse_control) = selected_control_scene(
        &mut coordinator,
        viewport,
        ellipse,
        DocumentCurveControlKind::MinorAxis,
    );
    assert_eq!(
        ellipse_control.target,
        DocumentCurveControlTarget::Scalar(ellipse_ratio)
    );
    assert!(
        coordinator
            .pointer_down(
                &ellipse_scene,
                pointer(ellipse_pointer, ellipse_control.screen_position)
            )
            .is_empty()
    );
    let valid_ellipse_target = [0.0, 3.0];
    let valid_ellipse_screen = viewport.model_to_screen(valid_ellipse_target);
    let valid_ellipse_request = one_curve_preview_request(&coordinator.editor_mut().pointer_move(
        &ellipse_scene,
        pointer(ellipse_pointer, valid_ellipse_screen),
    ));
    accept_curve_preview(&mut coordinator, ellipse_pointer, valid_ellipse_request);
    assert_close(
        preview_scalar(&coordinator, ellipse_ratio),
        0.75,
        "valid ellipse ratio preview",
    );

    let invalid_ellipse_target = [0.0, 5.0];
    let invalid_ellipse_screen = viewport.model_to_screen(invalid_ellipse_target);
    let invalid_ellipse_request =
        one_curve_preview_request(&coordinator.editor_mut().pointer_move(
            &ellipse_scene,
            pointer(ellipse_pointer, invalid_ellipse_screen),
        ));
    assert!(
        invalid_ellipse_request.request_id > valid_ellipse_request.request_id,
        "the out-of-domain ellipse sample must be independently considered"
    );
    assert!(
        coordinator
            .resolve_curve_control_preview(
                ellipse_pointer,
                invalid_ellipse_request.request_id,
                invalid_ellipse_request.expected,
                invalid_ellipse_request.control,
                invalid_ellipse_request.model_position,
            )
            .is_empty(),
        "ratio > 1 must not replace accepted preview geometry"
    );
    assert_close(
        preview_scalar(&coordinator, ellipse_ratio),
        0.75,
        "ellipse last-valid preview after domain rejection",
    );
    let ellipse_preview = prepared_preview_scene(
        &coordinator,
        viewport,
        ellipse_scene.accepted_revision,
        ellipse_scene.design_identity,
    );
    let ellipse_history = coordinator.history_len();
    release_curve_preview(
        &mut coordinator,
        &ellipse_preview,
        ellipse_pointer,
        invalid_ellipse_screen,
        ellipse_scene.design_identity,
    );
    assert_eq!(coordinator.history_len(), ellipse_history + 1);
    assert_close(
        retained_scalar(&coordinator, ellipse_ratio),
        0.75,
        "committed ellipse last-valid ratio",
    );
    let ellipse_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("ellipse checkpoint");
    coordinator.undo().expect("ellipse Undo");
    assert_close(
        retained_scalar(&coordinator, ellipse_ratio),
        0.5,
        "ellipse Undo",
    );
    coordinator
        .reload(&ellipse_checkpoint)
        .expect("ellipse checkpoint reload");
    assert_close(
        retained_scalar(&coordinator, ellipse_ratio),
        0.75,
        "ellipse checkpoint value",
    );
    let (_, recomputed_ellipse) = selected_control_scene(
        &mut coordinator,
        viewport,
        ellipse,
        DocumentCurveControlKind::MinorAxis,
    );
    assert_point_close(
        recomputed_ellipse.model_position,
        valid_ellipse_target,
        "recomputed ellipse minor-axis handle",
    );

    let hyperbola_pointer = 7_802;
    let branch = discrete_curve_state(coordinator.session().design_document(), hyperbola);
    assert_eq!(
        branch,
        DiscreteCurveState::Hyperbola(DocumentHyperbolaBranch::Negative)
    );
    let (hyperbola_scene, hyperbola_control) = selected_control_scene(
        &mut coordinator,
        viewport,
        hyperbola,
        DocumentCurveControlKind::ConjugateAxis,
    );
    assert_eq!(
        hyperbola_control.target,
        DocumentCurveControlTarget::Scalar(hyperbola_conjugate)
    );
    assert!(
        coordinator
            .pointer_down(
                &hyperbola_scene,
                pointer(hyperbola_pointer, hyperbola_control.screen_position)
            )
            .is_empty()
    );
    let valid_hyperbola_target = [8.0, 2.5];
    let valid_hyperbola_screen = viewport.model_to_screen(valid_hyperbola_target);
    let valid_hyperbola_request =
        one_curve_preview_request(&coordinator.editor_mut().pointer_move(
            &hyperbola_scene,
            pointer(hyperbola_pointer, valid_hyperbola_screen),
        ));
    accept_curve_preview(&mut coordinator, hyperbola_pointer, valid_hyperbola_request);
    assert_close(
        preview_scalar(&coordinator, hyperbola_conjugate),
        2.5,
        "valid hyperbola conjugate preview",
    );
    assert_eq!(
        preview_discrete_curve_state(&coordinator, hyperbola),
        branch
    );

    let zero_crossing = viewport.model_to_screen([8.0, 0.0]);
    assert!(
        coordinator
            .editor_mut()
            .pointer_move(&hyperbola_scene, pointer(hyperbola_pointer, zero_crossing))
            .is_empty(),
        "an exact zero rail sample must be filtered before a preview request"
    );
    assert_close(
        preview_scalar(&coordinator, hyperbola_conjugate),
        2.5,
        "hyperbola last-valid preview after zero crossing",
    );
    assert_eq!(
        preview_discrete_curve_state(&coordinator, hyperbola),
        branch
    );
    let hyperbola_preview = prepared_preview_scene(
        &coordinator,
        viewport,
        hyperbola_scene.accepted_revision,
        hyperbola_scene.design_identity,
    );
    let hyperbola_history = coordinator.history_len();
    release_curve_preview(
        &mut coordinator,
        &hyperbola_preview,
        hyperbola_pointer,
        zero_crossing,
        hyperbola_scene.design_identity,
    );
    assert_eq!(coordinator.history_len(), hyperbola_history + 1);
    assert_close(
        retained_scalar(&coordinator, hyperbola_conjugate),
        2.5,
        "committed hyperbola last-valid conjugate size",
    );
    assert_eq!(
        discrete_curve_state(coordinator.session().design_document(), hyperbola),
        branch
    );
    let hyperbola_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("hyperbola checkpoint");
    coordinator.undo().expect("hyperbola Undo");
    assert_close(
        retained_scalar(&coordinator, hyperbola_conjugate),
        1.5,
        "hyperbola Undo",
    );
    assert_eq!(
        discrete_curve_state(coordinator.session().design_document(), hyperbola),
        branch
    );
    coordinator
        .reload(&hyperbola_checkpoint)
        .expect("hyperbola checkpoint reload");
    assert_close(
        retained_scalar(&coordinator, hyperbola_conjugate),
        2.5,
        "hyperbola checkpoint value",
    );
    assert_eq!(
        discrete_curve_state(coordinator.session().design_document(), hyperbola),
        branch
    );
    let (_, recomputed_hyperbola) = selected_control_scene(
        &mut coordinator,
        viewport,
        hyperbola,
        DocumentCurveControlKind::ConjugateAxis,
    );
    assert_point_close(
        recomputed_hyperbola.model_position,
        valid_hyperbola_target,
        "recomputed hyperbola conjugate handle",
    );
}
