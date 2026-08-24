// SPDX-License-Identifier: GPL-3.0-or-later

//! Release-only regression envelopes for M83's retained interaction paths.
//!
//! These are deliberately generous wall-clock ceilings rather than
//! microbenchmarks. They keep coalesced pointer-frame work separate from exact
//! durable publication and exercise only public editor/materialization APIs.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use geosolve_constraint_editor::{
    ColdIntentMaterializer, EditorEffect, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentNativeBinding, Modifiers,
    OffsetAuthoringOutcome, OffsetAuthoringState, OffsetAuthoringTarget, PickTolerance,
    PointerInput, ProjectionalEditorSession, ProjectionalIntentCoordinator, ScreenPoint,
    SelectionItem, Viewport,
};
use geosolve_sketch::{
    DocumentCurveControlId, DocumentCurveControlKind, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentId, DocumentProfileOffsetOperand, OperationControl, PersistentId,
    SketchHardValidity,
};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentKey,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSessionId,
    IntentUnit, LeafField, PatchPortRef,
};

const FRAME_WARMUPS: usize = 3;
const FRAME_SAMPLES: usize = 15;
const RELATION_HEAVY_SEGMENTS: usize = 40;

// These bounds are intentionally much wider than a frame budget. They catch
// algorithmic/replay regressions while tolerating shared-runner variance.
const CURVE_FRAME_P95_CEILING: Duration = Duration::from_millis(150);
const CURVE_TERMINAL_CEILING: Duration = Duration::from_secs(3);
const FILLET_FRAME_P95_CEILING: Duration = Duration::from_millis(250);
const FILLET_TERMINAL_CEILING: Duration = Duration::from_secs(4);
const OFFSET_FRAME_P95_CEILING: Duration = Duration::from_millis(400);
const OFFSET_TERMINAL_CEILING: Duration = Duration::from_secs(6);

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("bounded M83 fixture key")
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

const fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn port(alias: &str, role: IntentPortRole, index: u16) -> PatchPortRef {
    PatchPortRef::Alias {
        node: key(alias),
        selector: selector(role, index),
    }
}

fn create(alias: &str, draft: IntentNodeDraft) -> IntentPatchOperation {
    IntentPatchOperation::CreateNode {
        alias: key(alias),
        draft: Box::new(draft),
        cell: None,
    }
}

fn point(symbol: &str, position: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key(symbol),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        coordinate(position[1]),
    )
}

fn line(symbol: &str, start: &str, end: &str, direction: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(symbol),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        port(start, IntentPortRole::Primary, 0),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 1),
        port(end, IntentPortRole::Primary, 0),
    )
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point(direction),
    )
}

fn implicit_line(symbol: &str, start: [f64; 2], end: [f64; 2]) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key(symbol),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        coordinate(start[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        coordinate(start[1]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        coordinate(end[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        coordinate(end[1]),
    )
    .with_field(
        IntentFieldKey(key("branch_direction")),
        IntentLiteral::Point([1.0, 0.0]),
    )
}

fn quadratic(symbol: &str) -> IntentNodeDraft {
    IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::QuadraticBezier,
        },
        key(symbol),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::X,
        coordinate(0.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Start, 0),
        LeafField::Y,
        coordinate(0.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::X,
        coordinate(1.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Control, 0),
        LeafField::Y,
        coordinate(2.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::X,
        coordinate(3.0),
    )
    .with_instance_leaf(
        selector(IntentPortRole::End, 0),
        LeafField::Y,
        coordinate(0.0),
    )
}

fn coordinator(raw: u128, scale: f64) -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            scale,
        )
        .expect("M83 performance materializer"),
    )
    .expect("M83 performance coordinator")
}

fn apply_fixture(
    coordinator: &mut ProjectionalIntentCoordinator,
    operations: Vec<IntentPatchOperation>,
) -> geosolve_constraint_editor::ProjectionalPatchOutcome {
    coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        ))
        .expect("accepted M83 performance fixture")
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn assert_independently_valid(session: &ProjectionalEditorSession) {
    let materialization = session
        .coordinator()
        .accepted_materialization()
        .expect("accepted materialization");
    assert!(materialization.validation.hard_residuals_validated);
    assert!(
        materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{:#?}",
        materialization.validation
    );
    let accepted = materialization
        .session
        .accepted_state_for_current_input()
        .expect("current accepted native authority");
    let report = accepted.solve_result().unstable_core_report();
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(
        report.hard_residual_max.is_finite() && report.hard_residual_max <= 1.0e-9,
        "{report:#?}"
    );
    let diagnostics = accepted.diagnostics().solve.expect("solve diagnostics");
    assert_eq!(diagnostics.hard_validity, SketchHardValidity::Valid);
    assert!(diagnostics.hard_residuals_validated);
    assert!(
        diagnostics
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        "{diagnostics:#?}"
    );
    let document = accepted.document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    for curve in document.curves() {
        for span in document.curve_spans(curve.id).expect("valid curve spans") {
            for parameter in [0.0, 0.5, 1.0] {
                let jet = document
                    .evaluate_curve_jet(span, parameter)
                    .expect("finite accepted curve jet");
                assert!(
                    [
                        jet.position.x,
                        jet.position.y,
                        jet.first_derivative.x,
                        jet.first_derivative.y,
                        jet.second_derivative.x,
                        jet.second_derivative.y,
                    ]
                    .into_iter()
                    .all(f64::is_finite)
                );
            }
        }
    }
}

fn assert_performance_envelope(
    label: &str,
    samples: &[Duration],
    frame_ceiling: Duration,
    terminal: Duration,
    terminal_ceiling: Duration,
) {
    assert_eq!(samples.len(), FRAME_SAMPLES);
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let median = ordered[ordered.len() / 2];
    let p95_index = (ordered.len() * 95).div_ceil(100).saturating_sub(1);
    let p95 = ordered[p95_index];
    println!(
        "m83/{label}: frames={} median={:.3}ms p95={:.3}ms frame-ceiling={:.3}ms exact-terminal={:.3}ms terminal-ceiling={:.3}ms",
        samples.len(),
        median.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        frame_ceiling.as_secs_f64() * 1_000.0,
        terminal.as_secs_f64() * 1_000.0,
        terminal_ceiling.as_secs_f64() * 1_000.0,
    );
    assert!(
        p95 <= frame_ceiling,
        "M83 {label} pointer-frame p95 {p95:?} exceeded generous regression ceiling {frame_ceiling:?}"
    );
    assert!(
        terminal <= terminal_ceiling,
        "M83 {label} exact terminal {terminal:?} exceeded generous regression ceiling {terminal_ceiling:?}"
    );
}

fn relation_heavy_curve_fixture() -> (ProjectionalIntentCoordinator, DocumentCurveControlId) {
    let mut coordinator = coordinator(0x8300_9002, 1.0);
    let mut operations = Vec::with_capacity(RELATION_HEAVY_SEGMENTS * 2 + 3);
    for index in 0..RELATION_HEAVY_SEGMENTS {
        let line_alias = format!("stress_line_{index:02}");
        let relation_alias = format!("stress_horizontal_{index:02}");
        let y = f64::from(u32::try_from(index).expect("bounded fixture index")) * 0.2 + 5.0;
        operations.push(create(
            &line_alias,
            implicit_line(&format!("line_{index:02}"), [0.0, y], [2.0, y]),
        ));
        operations.push(create(
            &relation_alias,
            IntentNodeDraft::new(
                IntentNodeKind::Constraint {
                    constraint: ConstraintKind::Horizontal,
                },
                key(&format!("horizontal_{index:02}")),
            )
            .with_input(
                InputSlot::new(InputRole::Span, 0),
                port(&line_alias, IntentPortRole::Span, 0),
            ),
        ));
    }
    operations.extend([
        create("curve", quadratic("curve_main")),
        create("control_reference", point("control_reference", [0.0, 2.0])),
        create(
            "control_axis",
            IntentNodeDraft::new(
                IntentNodeKind::Constraint {
                    constraint: ConstraintKind::HorizontalPoints,
                },
                key("control_axis"),
            )
            .with_input(
                InputSlot::new(InputRole::Point, 0),
                port("curve", IntentPortRole::Control, 0),
            )
            .with_input(
                InputSlot::new(InputRole::Point, 1),
                port("control_reference", IntentPortRole::Primary, 0),
            ),
        ),
    ]);
    let outcome = apply_fixture(&mut coordinator, operations);
    let curve_port = outcome
        .aliases
        .port(&key("curve"), selector(IntentPortRole::Curve, 0))
        .expect("quadratic curve output");
    let IntentNativeBinding::Curve(curve) = coordinator
        .accepted_materialization()
        .expect("curve fixture authority")
        .ownership
        .port(curve_port)
        .expect("quadratic native binding")
    else {
        panic!("quadratic output must bind one native curve")
    };
    let control = coordinator
        .accepted_materialization()
        .expect("curve fixture authority")
        .session
        .accepted_state_for_current_input()
        .expect("curve fixture accepted state")
        .document()
        .curve_controls(curve)
        .expect("quadratic controls")
        .into_iter()
        .find(|candidate| {
            candidate.id.kind == DocumentCurveControlKind::ControlPoint { ordinal: 1 }
        })
        .expect("quadratic middle control")
        .id;
    (coordinator, control)
}

fn fillet_fixture() -> (ProjectionalEditorSession, Viewport) {
    let mut coordinator = coordinator(0x8300_9003, 10.0);
    apply_fixture(
        &mut coordinator,
        vec![
            create("start", point("fillet_start", [0.0, 0.0])),
            create("corner", point("fillet_corner", [4.0, 0.0])),
            create("end", point("fillet_end", [4.0, 4.0])),
            create("first", line("fillet_first", "start", "corner", [1.0, 0.0])),
            create("second", line("fillet_second", "corner", "end", [0.0, 1.0])),
        ],
    );
    (
        ProjectionalEditorSession::new(coordinator),
        Viewport::new([800.0, 600.0], [2.0, 2.0], 50.0).expect("fillet viewport"),
    )
}

fn complete_fillet_authoring(
    session: &mut ProjectionalEditorSession,
    viewport: Viewport,
) -> FeatureAuthoringState {
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        session
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..FeatureAuthoringOptions::default()
                },
                &[],
                key("Fillet performance"),
            )
            .expect("activate Fillet authoring"),
        FeatureAuthoringOutcome::ModeEntered(_) | FeatureAuthoringOutcome::Collecting { .. }
    ));
    let scene = session.scene(viewport, 0.5).expect("Fillet base scene");
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([3.0, 0.0]),
                PickTolerance::default(),
                key("Fillet performance"),
            )
            .expect("first Fillet support"),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    assert!(matches!(
        session
            .transact_feature_authoring_pick_at(
                &mut state,
                &scene,
                viewport.model_to_screen([4.0, 1.0]),
                PickTolerance::default(),
                key("Fillet performance"),
            )
            .expect("second Fillet support"),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    assert!(session.feature_authoring_preview_matches(&state));
    state
}

fn offset_fixture() -> (ProjectionalEditorSession, Viewport) {
    let mut coordinator = coordinator(0x8300_9004, 1.0);
    apply_fixture(
        &mut coordinator,
        vec![
            create("a", point("offset_a", [0.0, 0.0])),
            create("b", point("offset_b", [4.0, 0.0])),
            create("c", point("offset_c", [4.0, 3.0])),
            create("bottom", line("offset_bottom", "a", "b", [1.0, 0.0])),
            create("right", line("offset_right", "b", "c", [0.0, 1.0])),
        ],
    );
    (
        ProjectionalEditorSession::new(coordinator),
        Viewport::new([800.0, 600.0], [2.0, 1.5], 50.0).expect("Offset viewport"),
    )
}

fn complete_offset_authoring(session: &mut ProjectionalEditorSession) -> OffsetAuthoringState {
    let mut state = OffsetAuthoringState::default();
    assert!(matches!(
        session
            .activate_offset_authoring(&mut state)
            .expect("activate Offset authoring"),
        OffsetAuthoringOutcome::ModeEntered(_)
    ));
    for label in ["offset_bottom", "offset_right"] {
        let document = session
            .presentation_session()
            .expect("Offset native authority")
            .design_document();
        let curve = document
            .curves()
            .iter()
            .find(|curve| curve.label == label)
            .expect("Offset source curve");
        let span = document.curve_spans(curve.id).expect("Offset source span")[0];
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
    }
    assert!(matches!(
        state.set_distance(0.5),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ));
    assert!(
        session
            .refresh_offset_authoring_preview(&state, key("Offset performance"))
            .expect("initial Offset preview")
    );
    state
}

fn offset_drag_geometry(
    session: &ProjectionalEditorSession,
    dimension: DocumentDimensionId,
    viewport: Viewport,
) -> (ScreenPoint, [f64; 2]) {
    let document = session
        .presentation_session()
        .expect("Offset preview authority")
        .design_document();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &document
        .dimension(dimension)
        .expect("provisional Offset dimension")
        .definition
    else {
        panic!("provisional dimension must be Profile Offset")
    };
    let edge = match operand {
        DocumentProfileOffsetOperand::Face { outer, .. } => outer.edges[0],
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => chain.edges[0],
    };
    let source = document
        .evaluate_curve_jet(edge.source.curve, 0.5)
        .expect("Offset source jet");
    let target = document
        .evaluate_curve_jet(edge.target.curve, 0.5)
        .expect("Offset target jet");
    let normal = source
        .differential()
        .expect("regular Offset source")
        .left_normal;
    let separation = [
        target.position.x - source.position.x,
        target.position.y - source.position.y,
    ];
    let sign = if separation[0].mul_add(normal.x, separation[1] * normal.y) > 0.0 {
        1.0
    } else {
        -1.0
    };
    let scene = session.scene(viewport, 0.5).expect("Offset preview scene");
    let target_curve = scene
        .curves
        .iter()
        .find(|curve| curve.span == edge.target.curve)
        .expect("painted provisional Offset target");
    let first = target_curve
        .screen_polyline
        .first()
        .expect("Offset target polyline start");
    let last = target_curve
        .screen_polyline
        .last()
        .expect("Offset target polyline end");
    (
        ScreenPoint {
            x: 0.5 * (first.x + last.x),
            y: 0.5 * (first.y + last.y),
        },
        [normal.x * sign, normal.y * sign],
    )
}

#[test]
#[ignore = "release-only M83 interaction performance regression"]
fn relation_heavy_curve_control_separates_frames_from_exact_terminal_publication() {
    let (mut coordinator, control) = relation_heavy_curve_fixture();
    let expected = coordinator
        .accepted_materialization()
        .expect("curve fixture authority")
        .session
        .design_identity();
    let accepted_revision = coordinator
        .accepted_materialization()
        .expect("curve fixture authority")
        .session
        .accepted_state_for_current_input()
        .expect("curve fixture accepted state")
        .identity()
        .revision()
        .get();
    let history_before = coordinator.intent().undo_len();
    let identity_before = coordinator.intent().identity();
    let canonical_before = coordinator
        .intent()
        .to_canonical_json()
        .expect("canonical curve fixture");
    coordinator
        .begin_curve_control_drag(91, accepted_revision, expected, control)
        .expect("prepare exact curve-control route");

    let mut timings = Vec::with_capacity(FRAME_SAMPLES);
    let mut latest_request = 0_u64;
    for sample in 0..FRAME_WARMUPS + FRAME_SAMPLES {
        latest_request += 1;
        let sample = f64::from(u32::try_from(sample).expect("bounded frame sample"));
        let target = [1.05 + sample * 0.015, 2.0];
        let started = Instant::now();
        let preview = coordinator
            .preview_curve_control_drag(
                91,
                latest_request,
                expected,
                control,
                black_box(target),
                OperationControl::unlimited(),
            )
            .expect("curve-control pointer frame")
            .expect("accepted curve-control pointer frame");
        let elapsed = started.elapsed();
        assert!(preview.accepted_position.into_iter().all(f64::is_finite));
        assert_eq!(coordinator.intent().identity(), identity_before);
        assert_eq!(coordinator.intent().undo_len(), history_before);
        if sample >= f64::from(u32::try_from(FRAME_WARMUPS).unwrap()) {
            timings.push(elapsed);
        }
    }
    assert_eq!(
        coordinator
            .intent()
            .to_canonical_json()
            .expect("canonical transient curve fixture"),
        canonical_before
    );

    let terminal_started = Instant::now();
    let outcome = coordinator
        .finish_curve_control_drag(91, latest_request, expected, control)
        .expect("exact curve-control terminal publication")
        .expect("non-empty curve-control transaction");
    let terminal = terminal_started.elapsed();
    black_box(outcome);
    assert_eq!(coordinator.intent().undo_len(), history_before + 1);
    let session = ProjectionalEditorSession::new(coordinator);
    assert_independently_valid(&session);
    assert_performance_envelope(
        "relation-heavy-curve-control",
        &timings,
        CURVE_FRAME_P95_CEILING,
        terminal,
        CURVE_TERMINAL_CEILING,
    );
}

#[test]
#[ignore = "release-only M83 interaction performance regression"]
fn pre_apply_fillet_radius_separates_frames_from_exact_apply_publication() {
    let (mut session, viewport) = fillet_fixture();
    let mut state = complete_fillet_authoring(&mut session, viewport);
    let scene = session.scene(viewport, 0.5).expect("Fillet preview scene");
    let rail = scene.fillet_affordances[0].radius_rail;
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let canonical_before = session
        .coordinator()
        .intent()
        .to_canonical_json()
        .expect("canonical pre-Apply Fillet intent");
    session
        .pointer_down_feature_authoring_radius(
            &state,
            &scene,
            pointer(92, rail.screen_grip),
            PickTolerance::default(),
            key("Fillet performance"),
        )
        .expect("begin pre-Apply Fillet radius")
        .expect("Fillet authoring radius owner");
    let origin = viewport.screen_to_model(rail.screen_grip);

    let mut timings = Vec::with_capacity(FRAME_SAMPLES);
    let mut latest_target = rail.screen_grip;
    for sample in 0..FRAME_WARMUPS + FRAME_SAMPLES {
        let sample = f64::from(u32::try_from(sample).expect("bounded frame sample"));
        // Stay beyond the shared three-pixel movement threshold even on the
        // first warm-up sample.
        let displacement = 0.1 + sample * 0.0125;
        latest_target = viewport.model_to_screen([
            displacement.mul_add(rail.model_derivative[0], origin[0]),
            displacement.mul_add(rail.model_derivative[1], origin[1]),
        ]);
        let current_scene = session.scene(viewport, 0.5).expect("current Fillet scene");
        let started = Instant::now();
        let effects = session
            .pointer_move_feature_authoring_radius(
                &mut state,
                &current_scene,
                pointer(92, black_box(latest_target)),
            )
            .expect("pre-Apply Fillet radius frame");
        let elapsed = started.elapsed();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::PreviewComputedFeatureRadius { radius, .. } if radius.is_finite()
        )));
        assert_eq!(session.coordinator().intent().identity(), identity_before);
        assert_eq!(session.coordinator().intent().undo_len(), history_before);
        if sample >= f64::from(u32::try_from(FRAME_WARMUPS).unwrap()) {
            timings.push(elapsed);
        }
    }
    assert_eq!(
        session
            .coordinator()
            .intent()
            .to_canonical_json()
            .expect("canonical transient Fillet intent"),
        canonical_before
    );
    let release_scene = session.scene(viewport, 0.5).expect("Fillet release scene");
    assert!(
        session
            .pointer_up_feature_authoring_radius(
                &mut state,
                &release_scene,
                pointer(92, latest_target),
            )
            .expect("finish pre-Apply Fillet radius")
    );
    assert_eq!(session.coordinator().intent().undo_len(), history_before);

    let terminal_started = Instant::now();
    let outcome = session
        .apply_computed_fillet_preview(&mut state, key("Fillet performance"))
        .expect("exact Fillet Apply publication");
    let terminal = terminal_started.elapsed();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert_independently_valid(&session);
    assert!(
        session
            .coordinator()
            .accepted_materialization()
            .expect("accepted Fillet materialization")
            .validation
            .all_active_features_current
    );
    assert_performance_envelope(
        "pre-apply-fillet-radius",
        &timings,
        FILLET_FRAME_P95_CEILING,
        terminal,
        FILLET_TERMINAL_CEILING,
    );
}

#[test]
#[ignore = "release-only M83 interaction performance regression"]
fn pre_apply_offset_distance_separates_frames_from_exact_apply_publication() {
    let (mut session, viewport) = offset_fixture();
    let mut state = complete_offset_authoring(&mut session);
    let history_before = session.coordinator().intent().undo_len();
    let identity_before = session.coordinator().intent().identity();
    let canonical_before = session
        .coordinator()
        .intent()
        .to_canonical_json()
        .expect("canonical pre-Apply Offset intent");
    let dimension = session
        .offset_authoring_provisional_items()
        .iter()
        .find_map(|item| match item {
            SelectionItem::Dimension(dimension) => Some(*dimension),
            _ => None,
        })
        .expect("provisional Profile Offset dimension");
    let (press, derivative) = offset_drag_geometry(&session, dimension, viewport);
    let scene = session.scene(viewport, 0.5).expect("Offset preview scene");
    session
        .pointer_down_offset_authoring_distance(
            &state,
            &scene,
            pointer(93, press),
            key("Offset performance"),
        )
        .expect("begin pre-Apply Offset distance")
        .expect("Offset authoring distance owner");
    let origin = viewport.screen_to_model(press);

    let mut timings = Vec::with_capacity(FRAME_SAMPLES);
    let mut latest_target = press;
    for sample in 0..FRAME_WARMUPS + FRAME_SAMPLES {
        let sample = f64::from(u32::try_from(sample).expect("bounded frame sample"));
        // Stay beyond the shared three-pixel movement threshold even on the
        // first warm-up sample.
        let displacement = 0.1 + sample * 0.0125;
        latest_target = viewport.model_to_screen([
            displacement.mul_add(derivative[0], origin[0]),
            displacement.mul_add(derivative[1], origin[1]),
        ]);
        let current_scene = session.scene(viewport, 0.5).expect("current Offset scene");
        let started = Instant::now();
        let effects = session
            .pointer_move_offset_authoring_distance(
                &mut state,
                &current_scene,
                pointer(93, black_box(latest_target)),
            )
            .expect("pre-Apply Offset distance frame");
        let elapsed = started.elapsed();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EditorEffect::PreviewAcceptedProfileOffsetDistance { distance, .. }
                if distance.is_finite()
        )));
        assert_eq!(session.coordinator().intent().identity(), identity_before);
        assert_eq!(session.coordinator().intent().undo_len(), history_before);
        if sample >= f64::from(u32::try_from(FRAME_WARMUPS).unwrap()) {
            timings.push(elapsed);
        }
    }
    assert_eq!(
        session
            .coordinator()
            .intent()
            .to_canonical_json()
            .expect("canonical transient Offset intent"),
        canonical_before
    );
    let release_scene = session.scene(viewport, 0.5).expect("Offset release scene");
    assert!(
        session
            .pointer_up_offset_authoring_distance(
                &mut state,
                &release_scene,
                pointer(93, latest_target),
            )
            .expect("finish pre-Apply Offset distance")
    );
    assert_eq!(session.coordinator().intent().undo_len(), history_before);

    let terminal_started = Instant::now();
    let outcome = session
        .apply_profile_offset_preview(&mut state, key("Offset performance"))
        .expect("exact Offset Apply publication");
    let terminal = terminal_started.elapsed();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        session.coordinator().intent().undo_len(),
        history_before + 1
    );
    assert_independently_valid(&session);
    assert_performance_envelope(
        "pre-apply-offset-distance",
        &timings,
        OFFSET_FRAME_P95_CEILING,
        terminal,
        OFFSET_TERMINAL_CEILING,
    );
}
