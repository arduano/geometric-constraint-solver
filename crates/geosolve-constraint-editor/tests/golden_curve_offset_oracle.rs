// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the reviewed all-family Offset oracle keeps each exact public fixture and lifecycle leg in one isolated test binary"
)]

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_constraint_editor::{
    ComputedSceneState, EditorEffect, EditorScene, Modifiers, OffsetAuthoringApplyEffect,
    OffsetAuthoringOperand, OffsetAuthoringOutcome, OffsetAuthoringRoute, OffsetAuthoringState,
    OffsetAuthoringTarget, PointerInput, RetainedEditorCoordinator, ScreenPoint, SelectionItem,
    Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveOffsetGeometry, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentCurveControlAvailability,
    DocumentCurveControlTarget, DocumentHyperbolaBranch, DocumentId, DocumentSolveRequest,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, PersistentId, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedEdgeId, ComputedEdgeProvenance, ComputedFeatureEvaluationState,
    ComputedFeatureId,
};

const FAMILY: &str = "feature.curve-offset";
const TSV_HEADER: &str = "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint";

#[derive(Clone, Copy, Debug)]
enum GeometryCase {
    Line,
    Polyline,
    Circle,
    CircularArc,
    Ellipse,
    EllipticalArc,
    RationalQuadratic,
    Parabola,
    Hyperbola,
    QuadraticBezier,
    CubicBezier,
    BSplineClamped,
    BSplinePeriodic,
    NurbsClamped,
    NurbsPeriodic,
    MixedChain,
    Face,
    FaceWithHole,
}

#[derive(Clone, Copy, Debug)]
struct CaseDefinition {
    id: &'static str,
    geometry: GeometryCase,
}

const CASES: &[CaseDefinition] = &[
    CaseDefinition {
        id: "feature.curve-offset.authoring.line",
        geometry: GeometryCase::Line,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.polyline",
        geometry: GeometryCase::Polyline,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.circle",
        geometry: GeometryCase::Circle,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.circular-arc",
        geometry: GeometryCase::CircularArc,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.ellipse",
        geometry: GeometryCase::Ellipse,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.elliptical-arc",
        geometry: GeometryCase::EllipticalArc,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.rational-quadratic",
        geometry: GeometryCase::RationalQuadratic,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.parabola",
        geometry: GeometryCase::Parabola,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.hyperbola",
        geometry: GeometryCase::Hyperbola,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.quadratic-bezier",
        geometry: GeometryCase::QuadraticBezier,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.cubic-bezier",
        geometry: GeometryCase::CubicBezier,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.bspline-clamped",
        geometry: GeometryCase::BSplineClamped,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.bspline-periodic",
        geometry: GeometryCase::BSplinePeriodic,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.nurbs-clamped",
        geometry: GeometryCase::NurbsClamped,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.nurbs-periodic",
        geometry: GeometryCase::NurbsPeriodic,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.mixed-chain",
        geometry: GeometryCase::MixedChain,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.face",
        geometry: GeometryCase::Face,
    },
    CaseDefinition {
        id: "feature.curve-offset.authoring.face-with-hole",
        geometry: GeometryCase::FaceWithHole,
    },
];

#[derive(Clone, Debug)]
enum OperandSelection {
    Spans(Vec<CurveSpan>),
    FaceAt([f64; 2]),
}

#[derive(Clone, Debug)]
struct Fixture {
    document: SketchDocument,
    selection: OperandSelection,
    route: OffsetAuthoringRoute,
    distance: f64,
    viewport_center: [f64; 2],
    proxy_point: Option<DesignPointId>,
    /// Every computed-family row couples its chosen source control to one ordinary stored point.
    /// The proxy gesture must therefore use the normal constrained solve and move both owners.
    proxy_constraint_follower: Option<DesignPointId>,
}

#[derive(Clone, Debug)]
struct SemanticDefect {
    class: &'static str,
    message: String,
}

#[derive(Clone, Debug)]
struct Observation {
    input_fingerprint: String,
    outcome: Result<(), SemanticDefect>,
}

fn defect(class: &'static str, message: impl Into<String>) -> SemanticDefect {
    SemanticDefect {
        class,
        message: message.into(),
    }
}

#[test]
fn golden_curve_offset_oracle_inventory_and_tsv_schema_are_exhaustive() {
    assert_eq!(CASES.len(), 18);
    assert_eq!(TSV_HEADER.split('\t').count(), 6);
    let mut ids = CASES.iter().map(|case| case.id).collect::<Vec<_>>();
    assert!(ids.iter().all(|id| id.starts_with(FAMILY)));
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        CASES.len(),
        "Curve Offset case IDs must be unique"
    );
}

#[test]
fn golden_curve_offset_oracle_survey() {
    let selected = env::var("GEOSOLVE_GOLDEN_ORACLE_CASE");
    let output = env::var("GEOSOLVE_GOLDEN_ORACLE_OUTPUT");
    if selected.is_err() && output.is_err() {
        return;
    }
    let selected = selected.expect("GEOSOLVE_GOLDEN_ORACLE_CASE must accompany oracle output");
    let output = output.expect("GEOSOLVE_GOLDEN_ORACLE_OUTPUT must accompany oracle case");
    let case = CASES
        .iter()
        .find(|case| case.id == selected)
        .unwrap_or_else(|| panic!("unknown Curve Offset oracle case {selected}"));

    let row = match catch_unwind(AssertUnwindSafe(|| observe(*case))) {
        Ok(observation) => match &observation.outcome {
            Ok(()) => format!(
                "{}\t{FAMILY}\tPASS\t-\t-\t{}",
                case.id, observation.input_fingerprint
            ),
            Err(failure) => {
                let detail = format!(
                    "input={}; {}",
                    observation.input_fingerprint,
                    sanitize_tsv(&failure.message)
                );
                format!(
                    "{}\t{FAMILY}\tDEFECT\t-\t{}\t{:016x}:{detail}",
                    case.id,
                    failure.class,
                    fnv1a64(detail.as_bytes())
                )
            }
        },
        Err(payload) => {
            let detail = sanitize_tsv(&panic_payload(&payload));
            format!(
                "{}\t{FAMILY}\tPANIC\t-\ttest-panic\t{:016x}:{detail}",
                case.id,
                fnv1a64(detail.as_bytes())
            )
        }
    };

    let file = File::create(&output)
        .unwrap_or_else(|error| panic!("cannot create Curve Offset oracle TSV {output}: {error}"));
    let mut output = BufWriter::new(file);
    writeln!(output, "{TSV_HEADER}").expect("write Curve Offset oracle header");
    writeln!(output, "{row}").expect("write Curve Offset oracle row");
    output.flush().expect("flush Curve Offset oracle row");
}

fn observe(case: CaseDefinition) -> Observation {
    let fixture = build_fixture(case);
    let design_json = fixture
        .document
        .to_canonical_json()
        .expect("Curve Offset design JSON");
    let session = RetainedSketchDocumentSession::new(
        fixture.document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("golden Curve Offset accepted session");
    assert_current_accepted(&session);
    let accepted_json = session
        .export_accepted_json()
        .expect("export Curve Offset accepted JSON")
        .expect("Curve Offset accepted JSON");
    let input_fingerprint = input_fingerprint(&[
        &design_json,
        &accepted_json,
        case.id,
        match fixture.route {
            OffsetAuthoringRoute::NativeProfile => "native-profile",
            OffsetAuthoringRoute::ComputedCurve => "computed-curve",
        },
    ]);
    let mut coordinator =
        RetainedEditorCoordinator::new(session).expect("Curve Offset coordinator");
    let outcome = exercise_offset(case, &fixture, &design_json, &mut coordinator);
    Observation {
        input_fingerprint,
        outcome,
    }
}

fn exercise_offset(
    case: CaseDefinition,
    fixture: &Fixture,
    base_design_json: &str,
    coordinator: &mut RetainedEditorCoordinator,
) -> Result<(), SemanticDefect> {
    let base_feature_json = coordinator
        .feature_document()
        .to_json()
        .map_err(|error| defect("curve-offset.fixture", error.to_string()))?;
    let base_history = (coordinator.history_len(), coordinator.history_cursor());
    let viewport = Viewport::new([1_000.0, 760.0], fixture.viewport_center, 80.0)
        .map_err(|error| defect("curve-offset.scene", error.to_string()))?;

    let mut state = OffsetAuthoringState::default();
    if !matches!(
        coordinator.activate_offset_authoring(&mut state),
        Ok(OffsetAuthoringOutcome::ModeEntered(_))
    ) {
        return Err(defect(
            "curve-offset.authoring.activation",
            format!("{} did not activate over a complete operand index", case.id),
        ));
    }
    match &fixture.selection {
        OperandSelection::Spans(spans) => {
            for span in spans {
                if !matches!(
                    state.pick_target(OffsetAuthoringTarget::Span(*span)),
                    OffsetAuthoringOutcome::OperandChanged { .. }
                ) {
                    return Err(defect(
                        "curve-offset.authoring.collection",
                        format!("{} did not collect source span {span:?}", case.id),
                    ));
                }
            }
        }
        OperandSelection::FaceAt(position) => {
            let scene = compose_current_scene(coordinator, viewport, true)?;
            if !matches!(
                state.pick_at(
                    &scene,
                    viewport.model_to_screen(*position),
                    geosolve_constraint_editor::PickTolerance::default(),
                    geosolve_constraint_editor::GeometryInteractionPolicy::default(),
                ),
                OffsetAuthoringOutcome::OperandChanged { .. }
            ) {
                return Err(defect(
                    "curve-offset.authoring.collection",
                    format!("{} did not collect the bounded face", case.id),
                ));
            }
        }
    }
    if !matches!(
        state.set_distance(fixture.distance),
        OffsetAuthoringOutcome::DistanceChanged { distance, .. }
            if distance.to_bits() == fixture.distance.to_bits()
    ) {
        return Err(defect(
            "curve-offset.authoring.distance",
            format!("{} rejected its finite regular offset distance", case.id),
        ));
    }
    let candidate = state.candidate().ok_or_else(|| {
        defect(
            "curve-offset.authoring.route",
            format!("{} did not produce a complete candidate", case.id),
        )
    })?;
    if candidate.route != fixture.route || candidate.input != coordinator.session().prepared_input()
    {
        return Err(defect(
            "curve-offset.authoring.route",
            format!(
                "{} selected {:?}, expected {:?}, or lost prepared-input authority",
                case.id, candidate.route, fixture.route
            ),
        ));
    }
    let face_expected = matches!(
        case.geometry,
        GeometryCase::Circle
            | GeometryCase::Ellipse
            | GeometryCase::BSplinePeriodic
            | GeometryCase::NurbsPeriodic
            | GeometryCase::Face
            | GeometryCase::FaceWithHole
    );
    let operand_matches = match &candidate.operand {
        OffsetAuthoringOperand::Face { key, .. } => {
            face_expected
                && if matches!(case.geometry, GeometryCase::FaceWithHole) {
                    key.holes.len() == 1
                } else {
                    key.holes.is_empty()
                }
        }
        OffsetAuthoringOperand::OpenChain { spans, .. } => !face_expected && !spans.is_empty(),
    };
    if !operand_matches {
        return Err(defect(
            "curve-offset.authoring.operand",
            format!(
                "{} did not retain its exact chain/face/hole topology",
                case.id
            ),
        ));
    }

    let preview = coordinator
        .prepare_offset_authoring_preview(&state, format!("{} golden Offset", case.id))
        .map_err(|error| {
            defect(
                "curve-offset.authoring.preview",
                format!("{} preview was unavailable: {error}", case.id),
            )
        })?;
    if preview.route() != fixture.route
        || preview.base_input() != coordinator.session().prepared_input()
        || coordinator.feature_document().to_json().ok().as_deref()
            != Some(base_feature_json.as_str())
        || (coordinator.history_len(), coordinator.history_cursor()) != base_history
    {
        return Err(defect(
            "curve-offset.authoring.preview-authority",
            format!("{} preview mutated durable authority", case.id),
        ));
    }
    let preview_scene = compose_current_scene(coordinator, viewport, true)?;
    validate_complete_scene(
        &preview_scene,
        fixture.route == OffsetAuthoringRoute::ComputedCurve,
    )?;

    match fixture.route {
        OffsetAuthoringRoute::NativeProfile => exercise_native_publication(
            case,
            coordinator,
            &mut state,
            &preview,
            base_design_json,
            base_history,
            viewport,
        ),
        OffsetAuthoringRoute::ComputedCurve => exercise_computed_publication(
            case,
            fixture,
            coordinator,
            &mut state,
            &preview,
            base_design_json,
            base_history,
            viewport,
            preview_scene,
        ),
    }
}

fn exercise_native_publication(
    case: CaseDefinition,
    coordinator: &mut RetainedEditorCoordinator,
    state: &mut OffsetAuthoringState,
    preview: &geosolve_constraint_editor::OffsetAuthoringPreviewMetadata,
    _base_design_json: &str,
    base_history: (usize, usize),
    viewport: Viewport,
) -> Result<(), SemanticDefect> {
    let metadata = preview.native_profile().ok_or_else(|| {
        defect(
            "curve-offset.authoring.route",
            format!(
                "{} published computed metadata on the native route",
                case.id
            ),
        )
    })?;
    if metadata.source_spans.is_empty()
        || metadata.target_spans.len() != metadata.source_spans.len()
        || metadata.provisional_points.is_empty()
        || coordinator.visible_preview_session().is_none()
    {
        return Err(defect(
            "curve-offset.authoring.preview-authority",
            format!(
                "{} native preview omitted its exact source/target patch",
                case.id
            ),
        ));
    }
    let target_spans = metadata.target_spans.clone();
    let dimension = metadata.dimension;
    let distance = metadata.distance;
    let preview_document = coordinator
        .visible_preview_session()
        .map(|session| session.design_document().clone())
        .ok_or_else(|| {
            defect(
                "curve-offset.authoring.preview-authority",
                format!("{} native preview session disappeared", case.id),
            )
        })?;
    if &preview_document == coordinator.session().design_document() {
        return Err(defect(
            "curve-offset.authoring.preview-authority",
            format!(
                "{} native preview did not contain generated target geometry",
                case.id
            ),
        ));
    }

    let applied = coordinator
        .apply_offset_authoring_preview(state)
        .map_err(|error| {
            defect(
                "curve-offset.authoring.publication",
                format!("{} exact native preview did not publish: {error}", case.id),
            )
        })?;
    if !matches!(
        applied.value,
        OffsetAuthoringApplyEffect::NativeProfile(ref ids)
            if ids.dimension == dimension && ids.target == distance
    ) || applied.published_accepted.is_none()
        || (coordinator.history_len(), coordinator.history_cursor())
            != (base_history.0 + 1, base_history.1 + 1)
        || coordinator.session().design_document() != &preview_document
    {
        return Err(defect(
            "curve-offset.authoring.publication",
            format!(
                "{} did not atomically publish the exact native patch",
                case.id
            ),
        ));
    }
    assert_current_accepted(coordinator.session());
    let document = coordinator.session().design_document();
    if document.dimension(dimension).is_none()
        || target_spans
            .iter()
            .any(|span| document.curve(span.curve).is_none())
    {
        return Err(defect(
            "curve-offset.authoring.publication",
            format!(
                "{} lost native target or grouped constraint ownership",
                case.id
            ),
        ));
    }
    let scene = compose_current_scene(coordinator, viewport, true)?;
    validate_complete_scene(&scene, false)
}

fn exercise_computed_publication(
    case: CaseDefinition,
    fixture: &Fixture,
    coordinator: &mut RetainedEditorCoordinator,
    state: &mut OffsetAuthoringState,
    preview: &geosolve_constraint_editor::OffsetAuthoringPreviewMetadata,
    base_design_json: &str,
    base_history: (usize, usize),
    viewport: Viewport,
    preview_scene: EditorScene,
) -> Result<(), SemanticDefect> {
    let metadata = preview.computed_curve().ok_or_else(|| {
        defect(
            "curve-offset.authoring.route",
            format!(
                "{} published native metadata on the computed route",
                case.id
            ),
        )
    })?;
    let feature = metadata.feature;
    let preview_edges = validate_current_output(coordinator, feature, &metadata.generated_edges)?;
    if metadata.source_spans.is_empty()
        || coordinator.session().export_design_json().ok().as_deref() != Some(base_design_json)
    {
        return Err(defect(
            "curve-offset.authoring.preview-authority",
            format!(
                "{} computed preview changed native source geometry",
                case.id
            ),
        ));
    }

    let applied = coordinator
        .apply_offset_authoring_preview(state)
        .map_err(|error| {
            defect(
                "curve-offset.authoring.publication",
                format!(
                    "{} exact computed preview did not publish: {error}",
                    case.id
                ),
            )
        })?;
    if applied.value != OffsetAuthoringApplyEffect::ComputedCurve(feature)
        || (coordinator.history_len(), coordinator.history_cursor())
            != (base_history.0 + 1, base_history.1 + 1)
        || coordinator.transcript().len() != 1
        || coordinator.session().export_design_json().ok().as_deref() != Some(base_design_json)
        || coordinator.feature_document().feature(feature).is_none()
    {
        return Err(defect(
            "curve-offset.authoring.publication",
            format!(
                "{} did not publish exactly one generated-feature history step",
                case.id
            ),
        ));
    }
    let applied_edges = validate_current_output(coordinator, feature, &preview_edges)?;
    if applied_edges != preview_edges {
        return Err(defect(
            "curve-offset.authoring.publication",
            format!(
                "{} Apply reevaluated instead of publishing the held output",
                case.id
            ),
        ));
    }
    let proxy_point = fixture.proxy_point.ok_or_else(|| {
        defect(
            "curve-offset.proxy.coverage",
            format!("{} has no declared two-dimensional inverse proxy", case.id),
        )
    })?;
    let proxy_constraint_follower = fixture.proxy_constraint_follower.ok_or_else(|| {
        defect(
            "curve-offset.proxy.constraint-coverage",
            format!(
                "{} has no ordinary constrained source-control follower",
                case.id
            ),
        )
    })?;
    exercise_proxy_edit(
        case,
        coordinator,
        feature,
        &metadata.source_spans,
        proxy_point,
        proxy_constraint_follower,
        &applied_edges,
        base_history,
        viewport,
        preview_scene,
    )
}

fn exercise_proxy_edit(
    case: CaseDefinition,
    coordinator: &mut RetainedEditorCoordinator,
    feature: ComputedFeatureId,
    source_spans: &[CurveSpan],
    proxy_point: DesignPointId,
    proxy_constraint_follower: DesignPointId,
    applied_edges: &[ComputedEdgeId],
    base_history: (usize, usize),
    viewport: Viewport,
    mut scene: EditorScene,
) -> Result<(), SemanticDefect> {
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Feature(feature)]);
    let origin_design = coordinator.session().design_identity();
    let origin_position = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_point))
        .map(|point| point.position)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.source",
                format!("{} proxy source point is absent", case.id),
            )
        })?;
    let origin_follower_position = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| {
            accepted
                .document()
                .point(proxy_constraint_follower)
                .map(|point| point.position)
        })
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.constraint-coverage",
                format!("{} constrained proxy follower is absent", case.id),
            )
        })?;
    if origin_follower_position.map(f64::to_bits) != origin_position.map(f64::to_bits) {
        return Err(defect(
            "curve-offset.proxy.constraint-coverage",
            format!(
                "{} constrained proxy follower did not start coincident with its source",
                case.id
            ),
        ));
    }
    coordinator
        .editor()
        .populate_curve_controls(&mut scene)
        .map_err(|error| defect("curve-offset.proxy.scene", error.to_string()))?;
    let accepted_document = coordinator
        .session()
        .accepted_state_for_current_input()
        .map(geosolve_sketch::SketchAcceptedDocumentState::document)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.inventory",
                format!(
                    "{} has no Current accepted source for proxy inventory",
                    case.id
                ),
            )
        })?;
    let mut source_curves = source_spans
        .iter()
        .map(|span| span.curve)
        .collect::<Vec<_>>();
    source_curves.sort_unstable();
    source_curves.dedup();
    let mut expected_proxy_ids = Vec::new();
    for curve in source_curves {
        expected_proxy_ids.extend(
            accepted_document
                .curve_controls(curve)
                .map_err(|error| defect("curve-offset.proxy.inventory", error.to_string()))?
                .into_iter()
                .filter(|control| {
                    matches!(
                        control.availability,
                        DocumentCurveControlAvailability::Editable
                    ) && matches!(
                        control.target,
                        DocumentCurveControlTarget::Point(_)
                            | DocumentCurveControlTarget::RationalMiddle { .. }
                    )
                })
                .map(|control| control.id),
        );
    }
    expected_proxy_ids.sort_unstable();
    expected_proxy_ids.dedup();
    let mut actual_proxy_ids = scene
        .curve_controls
        .iter()
        .filter(|control| {
            control
                .offset_proxy
                .is_some_and(|proxy| proxy.feature == feature)
                && control.is_editable()
        })
        .map(|control| control.id)
        .collect::<Vec<_>>();
    actual_proxy_ids.sort_unstable();
    actual_proxy_ids.dedup();
    if actual_proxy_ids != expected_proxy_ids {
        return Err(defect(
            "curve-offset.proxy.inventory",
            format!(
                "{} exposed incomplete eligible two-dimensional proxy controls: expected {expected_proxy_ids:?}, actual {actual_proxy_ids:?}",
                case.id
            ),
        ));
    }
    let proxy = scene
        .curve_controls
        .iter()
        .find(|control| {
            control
                .offset_proxy
                .is_some_and(|proxy| proxy.feature == feature)
                && control.target == DocumentCurveControlTarget::Point(proxy_point)
                && control.is_editable()
        })
        .cloned()
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.coverage",
                format!(
                    "{} did not publish its declared source-owned proxy",
                    case.id
                ),
            )
        })?;
    if proxy.offset_proxy.is_none()
        || !proxy.model_position.into_iter().all(f64::is_finite)
        || proxy.model_position.map(f64::to_bits) == origin_position.map(f64::to_bits)
    {
        return Err(defect(
            "curve-offset.proxy.coverage",
            format!(
                "{} proxy was non-finite or painted on the source cage",
                case.id
            ),
        ));
    }

    let pointer_id = 82_000_u64 + case.geometry as u64;
    if !coordinator
        .pointer_down(&scene, pointer(pointer_id, proxy.screen_position))
        .is_empty()
    {
        return Err(defect(
            "curve-offset.proxy.gesture",
            format!("{} proxy press emitted an unexpected mutation", case.id),
        ));
    }
    let moved = ScreenPoint {
        x: proxy.screen_position.x + 8.0,
        y: proxy.screen_position.y - 5.0,
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
        return Err(defect(
            "curve-offset.proxy.gesture",
            format!(
                "{} proxy move did not request one source-control solve: {request:?}",
                case.id
            ),
        ));
    };
    if *control != proxy.id
        || *expected != origin_design
        || !model_position.iter().all(|value| value.is_finite())
        || model_position.map(f64::to_bits) == proxy.model_position.map(f64::to_bits)
    {
        return Err(defect(
            "curve-offset.proxy.inverse-map",
            format!(
                "{} proxy did not inverse-map to finite source-control input",
                case.id
            ),
        ));
    }
    let acknowledgement = coordinator.resolve_curve_control_preview(
        pointer_id,
        *request_id,
        *expected,
        *control,
        *model_position,
    );
    if !matches!(
        acknowledgement.as_slice(),
        [EditorEffect::PreviewCurveControl { control: accepted, .. }] if accepted == control
    ) || !coordinator.curve_control_preview_active()
    {
        return Err(defect(
            "curve-offset.proxy.solve",
            format!(
                "{} source-owned proxy edit was not independently accepted",
                case.id
            ),
        ));
    }
    let candidate_session = coordinator.visible_preview_session().ok_or_else(|| {
        defect(
            "curve-offset.proxy.solve",
            format!("{} accepted proxy edit has no prepared candidate", case.id),
        )
    })?;
    assert_current_accepted(candidate_session);
    let candidate_position = candidate_session
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_point))
        .map(|point| point.position)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.source",
                "candidate source point is absent",
            )
        })?;
    let movement_tolerance = 1.0e-10 * coordinator.session().design_document().model_scale();
    if (0..2)
        .any(|axis| (candidate_position[axis] - origin_position[axis]).abs() <= movement_tolerance)
    {
        return Err(defect(
            "curve-offset.proxy.inverse-map",
            format!(
                "{} accepted proxy edit did not move its owning source independently in both X and Y: {:?} -> {:?}",
                case.id, origin_position, candidate_position
            ),
        ));
    }
    let candidate_follower_position = candidate_session
        .accepted_state_for_current_input()
        .and_then(|accepted| {
            accepted
                .document()
                .point(proxy_constraint_follower)
                .map(|point| point.position)
        })
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.constraint-coverage",
                "candidate constrained proxy follower is absent",
            )
        })?;
    if (0..2)
        .any(|axis| (candidate_follower_position[axis] - candidate_position[axis]).abs() > 1.0e-9)
    {
        return Err(defect(
            "curve-offset.proxy.constraint-projection",
            format!(
                "{} proxy bypassed its ordinary Coincident constraint: source {:?}, follower {:?}",
                case.id, candidate_position, candidate_follower_position
            ),
        ));
    }

    let mut candidate_scene = compose_current_scene(coordinator, viewport, false)?;
    coordinator
        .editor()
        .populate_curve_controls(&mut candidate_scene)
        .map_err(|error| defect("curve-offset.proxy.scene", error.to_string()))?;
    coordinator
        .retain_curve_control_preview_interaction_origin(&mut candidate_scene)
        .map_err(|error| {
            defect(
                "curve-offset.proxy.scene-authority",
                format!(
                    "{} candidate scene lost pointer-down authority: {error}",
                    case.id
                ),
            )
        })?;
    validate_complete_scene(&candidate_scene, true)?;
    let release = coordinator.editor_mut().pointer_up(
        &candidate_scene,
        origin_design,
        pointer(pointer_id, moved),
    );
    let [commit @ EditorEffect::CommitCurveControl { .. }] = release.as_slice() else {
        return Err(defect(
            "curve-offset.proxy.commit",
            format!(
                "{} proxy release did not produce one exact commit: {release:?}",
                case.id
            ),
        ));
    };
    if coordinator
        .apply_editor_effect(commit)
        .map_err(|error| defect("curve-offset.proxy.commit", error.to_string()))?
        .is_none()
        || (coordinator.history_len(), coordinator.history_cursor())
            != (base_history.0 + 2, base_history.1 + 2)
        || coordinator.transcript().len() != 2
    {
        return Err(defect(
            "curve-offset.proxy.commit",
            format!(
                "{} proxy edit was not one durable source transaction",
                case.id
            ),
        ));
    }
    assert_current_accepted(coordinator.session());
    let committed_position = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_point))
        .map(|point| point.position)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.source",
                "committed source point is absent",
            )
        })?;
    if (0..2)
        .any(|axis| (committed_position[axis] - origin_position[axis]).abs() <= movement_tolerance)
    {
        return Err(defect(
            "curve-offset.proxy.commit",
            format!(
                "{} durable source did not retain both X/Y proxy motion",
                case.id,
            ),
        ));
    }
    let committed_follower_position = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| {
            accepted
                .document()
                .point(proxy_constraint_follower)
                .map(|point| point.position)
        })
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.constraint-coverage",
                "committed constrained proxy follower is absent",
            )
        })?;
    if (0..2)
        .any(|axis| (committed_follower_position[axis] - committed_position[axis]).abs() > 1.0e-9)
    {
        return Err(defect(
            "curve-offset.proxy.constraint-projection",
            format!(
                "{} durable proxy edit did not preserve its ordinary constraint",
                case.id
            ),
        ));
    }
    let regenerated = validate_current_output(coordinator, feature, &[])?;
    if regenerated.is_empty() || regenerated == applied_edges {
        return Err(defect(
            "curve-offset.proxy.regeneration",
            format!(
                "{} did not replace revision-local generated output",
                case.id
            ),
        ));
    }
    coordinator.undo().map_err(|error| {
        defect(
            "curve-offset.proxy.undo-redo",
            format!("{} proxy Undo failed: {error}", case.id),
        )
    })?;
    if (coordinator.history_len(), coordinator.history_cursor())
        != (base_history.0 + 2, base_history.1 + 1)
    {
        return Err(defect(
            "curve-offset.proxy.undo-redo",
            format!("{} proxy Undo changed the wrong history boundary", case.id),
        ));
    }
    assert_current_accepted(coordinator.session());
    let undone = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_point))
        .map(|point| point.position)
        .ok_or_else(|| defect("curve-offset.proxy.undo-redo", "Undo lost the source point"))?;
    let undone_follower = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_constraint_follower))
        .map(|point| point.position)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.undo-redo",
                "Undo lost the constrained follower",
            )
        })?;
    if undone.map(f64::to_bits) != origin_position.map(f64::to_bits)
        || undone_follower.map(f64::to_bits) != origin_follower_position.map(f64::to_bits)
        || validate_current_output(coordinator, feature, &[])?.is_empty()
    {
        return Err(defect(
            "curve-offset.proxy.undo-redo",
            format!(
                "{} proxy Undo did not restore complete source/output state",
                case.id
            ),
        ));
    }
    validate_complete_scene(&compose_current_scene(coordinator, viewport, true)?, true)?;

    coordinator.redo().map_err(|error| {
        defect(
            "curve-offset.proxy.undo-redo",
            format!("{} proxy Redo failed: {error}", case.id),
        )
    })?;
    if (coordinator.history_len(), coordinator.history_cursor())
        != (base_history.0 + 2, base_history.1 + 2)
    {
        return Err(defect(
            "curve-offset.proxy.undo-redo",
            format!("{} proxy Redo changed the wrong history boundary", case.id),
        ));
    }
    assert_current_accepted(coordinator.session());
    let redone = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_point))
        .map(|point| point.position)
        .ok_or_else(|| defect("curve-offset.proxy.undo-redo", "Redo lost the source point"))?;
    let redone_follower = coordinator
        .session()
        .accepted_state_for_current_input()
        .and_then(|accepted| accepted.document().point(proxy_constraint_follower))
        .map(|point| point.position)
        .ok_or_else(|| {
            defect(
                "curve-offset.proxy.undo-redo",
                "Redo lost the constrained follower",
            )
        })?;
    if redone.map(f64::to_bits) != committed_position.map(f64::to_bits)
        || redone_follower.map(f64::to_bits) != committed_follower_position.map(f64::to_bits)
        || validate_current_output(coordinator, feature, &[])?.is_empty()
    {
        return Err(defect(
            "curve-offset.proxy.undo-redo",
            format!(
                "{} proxy Redo did not restore complete source/output state",
                case.id
            ),
        ));
    }
    validate_complete_scene(&compose_current_scene(coordinator, viewport, true)?, true)?;
    Ok(())
}

fn compose_current_scene(
    coordinator: &RetainedEditorCoordinator,
    viewport: Viewport,
    authenticate: bool,
) -> Result<EditorScene, SemanticDefect> {
    let source = coordinator
        .visible_preview_session()
        .unwrap_or_else(|| coordinator.session());
    let accepted = source.accepted_state_for_current_input().ok_or_else(|| {
        defect(
            "curve-offset.scene-authority",
            "visible source is not current and accepted",
        )
    })?;
    let accepted_input = source.accepted_prepared_input().ok_or_else(|| {
        defect(
            "curve-offset.scene-authority",
            "visible source has no accepted prepared input",
        )
    })?;
    let scene = match coordinator.computed_scene_state() {
        ComputedSceneState::Current { expected, snapshot } => {
            EditorScene::from_accepted_with_computed(
                accepted.identity().revision().get(),
                source.design_identity(),
                accepted.document(),
                source.design_document(),
                &accepted_input,
                expected,
                snapshot,
                viewport,
                2.0,
            )
        }
        ComputedSceneState::Absent => EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            source.design_identity(),
            accepted.document(),
            source.design_document(),
            viewport,
            2.0,
        ),
        ComputedSceneState::Withheld => {
            return Err(defect(
                "curve-offset.scene-authority",
                "current computed presentation was withheld",
            ));
        }
    }
    .map_err(|error| defect("curve-offset.scene-authority", error.to_string()))?;
    if authenticate {
        scene
            .with_retained_session(source)
            .map_err(|error| defect("curve-offset.scene-authority", error.to_string()))
    } else {
        Ok(scene)
    }
}

fn validate_complete_scene(
    scene: &EditorScene,
    require_computed_offset: bool,
) -> Result<(), SemanticDefect> {
    let finite = scene.points.iter().all(|point| {
        point.model_position.into_iter().all(f64::is_finite)
            && point.screen_position.x.is_finite()
            && point.screen_position.y.is_finite()
    }) && scene.curves.iter().all(|curve| {
        !curve.screen_polyline.is_empty()
            && curve
                .screen_polyline
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
    }) && scene.computed_offset_curves.iter().all(|curve| {
        !curve.screen_polyline.is_empty()
            && (curve.screen_source_parameters.is_empty()
                || curve.screen_polyline.len() == curve.screen_source_parameters.len())
            && curve
                .screen_polyline
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
            && curve
                .screen_source_parameters
                .iter()
                .all(|parameter| parameter.is_finite())
    });
    if !finite || scene.curves.is_empty() {
        return Err(defect(
            "curve-offset.scene-completeness",
            "accepted native geometry was blank or non-finite",
        ));
    }
    if require_computed_offset && scene.computed_offset_curves.is_empty() {
        return Err(defect(
            "curve-offset.scene-completeness",
            "computed Offset presentation was absent while native geometry remained current",
        ));
    }
    if require_computed_offset
        && !scene
            .computed_offset_curves
            .iter()
            .any(|curve| !curve.screen_source_parameters.is_empty())
    {
        return Err(defect(
            "curve-offset.scene-correspondence",
            "computed Offset scene retained no finite source-owned parameter correspondence",
        ));
    }
    Ok(())
}

fn validate_current_output(
    coordinator: &RetainedEditorCoordinator,
    feature: ComputedFeatureId,
    expected_edges: &[ComputedEdgeId],
) -> Result<Vec<ComputedEdgeId>, SemanticDefect> {
    let snapshot = coordinator.computed_snapshot().ok_or_else(|| {
        defect(
            "curve-offset.authoring.current-authority",
            "current computed snapshot is absent",
        )
    })?;
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .ok_or_else(|| {
            defect(
                "curve-offset.authoring.current-authority",
                "feature-local evaluation is absent",
            )
        })?;
    let ComputedFeatureEvaluationState::Current {
        corner_edges,
        generated_edges,
    } = &evaluation.state
    else {
        return Err(defect(
            "curve-offset.authoring.current-authority",
            format!("feature was not Current: {:?}", evaluation.state),
        ));
    };
    if !corner_edges.is_empty()
        || generated_edges.is_empty()
        || (!expected_edges.is_empty() && generated_edges != expected_edges)
    {
        return Err(defect(
            "curve-offset.authoring.current-authority",
            "Current output did not retain one exact non-empty generated-edge set",
        ));
    }
    for edge_id in generated_edges {
        let edge = snapshot.edge(*edge_id).ok_or_else(|| {
            defect(
                "curve-offset.authoring.current-authority",
                "generated edge identity did not resolve in its owning snapshot",
            )
        })?;
        let ComputedEdgeProvenance::CurveOffset {
            owner,
            source_parameters,
            ..
        } = edge.provenance
        else {
            return Err(defect(
                "curve-offset.authoring.provenance",
                "generated edge did not carry Curve Offset provenance",
            ));
        };
        let ComputedEdgeGeometry::CurveOffset(geometry) = &edge.geometry else {
            return Err(defect(
                "curve-offset.authoring.geometry",
                "Curve Offset provenance did not carry Curve Offset geometry",
            ));
        };
        if owner != feature
            || !curve_offset_geometry_is_finite(geometry)
            || !curve_offset_source_correspondence_is_valid(geometry, source_parameters)
            || coordinator.selection_for_computed_edge(*edge_id)
                != Some(SelectionItem::Feature(feature))
        {
            return Err(defect(
                "curve-offset.authoring.geometry",
                "generated output was non-finite, lost honest source correspondence, or lost stable feature ownership",
            ));
        }
    }
    Ok(generated_edges.clone())
}

fn curve_offset_source_correspondence_is_valid(
    geometry: &CurveOffsetGeometry,
    source_parameters: Option<[f64; 2]>,
) -> bool {
    match (geometry, source_parameters) {
        // A junction-only connector is always a generated line and deliberately has no honest
        // inverse mapping onto either adjacent source span. Every source-derived analytic edge
        // must carry its traversal-correct native parameter endpoints.
        (CurveOffsetGeometry::Line { .. }, None) => true,
        (
            CurveOffsetGeometry::Line { .. } | CurveOffsetGeometry::CircularArc { .. },
            Some([start, end]),
        ) => start.is_finite() && end.is_finite() && start.to_bits() != end.to_bits(),
        (CurveOffsetGeometry::CubicPatches(patches), Some([start, end])) => {
            !patches.is_empty()
                && start.is_finite()
                && end.is_finite()
                && start.to_bits() != end.to_bits()
                && patches[0].source_parameters[0].to_bits() == start.to_bits()
                && patches
                    .last()
                    .is_some_and(|patch| patch.source_parameters[1].to_bits() == end.to_bits())
                && patches.windows(2).all(|pair| {
                    pair[0].source_parameters[1].to_bits() == pair[1].source_parameters[0].to_bits()
                })
        }
        // Arc and fitted output may never masquerade as a source-less connector.
        (CurveOffsetGeometry::CircularArc { .. } | CurveOffsetGeometry::CubicPatches(_), None) => {
            false
        }
    }
}

fn curve_offset_geometry_is_finite(geometry: &CurveOffsetGeometry) -> bool {
    match geometry {
        CurveOffsetGeometry::Line { start, end } => {
            start.iter().chain(end).all(|value| value.is_finite())
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => {
            center.iter().all(|value| value.is_finite())
                && radius.is_finite()
                && *radius > 0.0
                && start_angle.is_finite()
                && sweep.is_finite()
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            !patches.is_empty()
                && patches.iter().all(|patch| {
                    patch
                        .source_parameters
                        .iter()
                        .chain(patch.controls.iter().flatten())
                        .all(|value| value.is_finite())
                })
        }
    }
}

fn build_fixture(case: CaseDefinition) -> Fixture {
    let index = CASES
        .iter()
        .position(|candidate| candidate.id == case.id)
        .expect("known Curve Offset case");
    let mut document = SketchDocument::with_id(
        10.0,
        DocumentId(PersistentId::from_u128(
            0x676f_6c64_656e_5f6f_6666_0000_0000_u128 + index as u128 + 1,
        )),
    )
    .expect("golden Curve Offset document");
    match case.geometry {
        GeometryCase::Line => {
            let start = add_point(&mut document, "line start", [0.0, 0.0]);
            let end = add_point(&mut document, "line end", [4.0, 0.0]);
            let curve = add_line(&mut document, "line", start, end);
            fixture(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::NativeProfile,
                None,
            )
        }
        GeometryCase::Polyline => {
            let points = [
                add_point(&mut document, "polyline start", [0.0, 0.0]),
                add_point(&mut document, "polyline corner", [2.0, 0.0]),
                add_point(&mut document, "polyline end", [2.0, 2.0]),
            ];
            let curve = document
                .add_curve(
                    "polyline",
                    CurveDefinition::Polyline {
                        points: points.to_vec(),
                        closed: false,
                        branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
                    },
                )
                .expect("polyline");
            fixture(
                document,
                OperandSelection::Spans(vec![
                    CurveSpan { curve, segment: 0 },
                    CurveSpan { curve, segment: 1 },
                ]),
                OffsetAuthoringRoute::NativeProfile,
                None,
            )
        }
        GeometryCase::Circle => {
            let center = add_point(&mut document, "circle center", [0.0, 0.0]);
            let radius = add_scalar(
                &mut document,
                "circle radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            document
                .add_curve("circle", CurveDefinition::Circle { center, radius })
                .expect("circle");
            fixture(
                document,
                OperandSelection::FaceAt([0.0, 0.0]),
                OffsetAuthoringRoute::NativeProfile,
                None,
            )
        }
        GeometryCase::CircularArc => {
            let center = add_point(&mut document, "arc center", [0.0, 0.0]);
            let radius = add_scalar(
                &mut document,
                "arc radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let start_angle = finite_scalar(&mut document, "arc start", 0.0, ScalarUnit::Angle);
            let end_angle = finite_scalar(
                &mut document,
                "arc end",
                std::f64::consts::FRAC_PI_2,
                ScalarUnit::Angle,
            );
            let curve = document
                .add_curve(
                    "circular arc",
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .expect("circular arc");
            fixture(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::NativeProfile,
                None,
            )
        }
        GeometryCase::Ellipse => ellipse_fixture(document, true),
        GeometryCase::EllipticalArc => ellipse_fixture(document, false),
        GeometryCase::RationalQuadratic => {
            let start = add_point(&mut document, "rational start", [0.0, 0.0]);
            let end = add_point(&mut document, "rational end", [4.0, 0.0]);
            let middle_weight = add_scalar(
                &mut document,
                "rational weight",
                0.75,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            );
            let curve = document
                .add_curve(
                    "rational quadratic",
                    CurveDefinition::RationalQuadraticConic {
                        start,
                        weighted_middle: [1.5, 1.125],
                        middle_weight,
                        end,
                    },
                )
                .expect("rational quadratic");
            fixture(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::ComputedCurve,
                Some(start),
            )
        }
        GeometryCase::Parabola => {
            let vertex = add_point(&mut document, "parabola vertex", [0.0, 0.0]);
            let focus = add_point(&mut document, "parabola focus", [0.0, 1.0]);
            let trim_start = finite_scalar(
                &mut document,
                "parabola trim start",
                -0.75,
                ScalarUnit::Parameter,
            );
            let trim_end = finite_scalar(
                &mut document,
                "parabola trim end",
                0.75,
                ScalarUnit::Parameter,
            );
            let curve = document
                .add_curve(
                    "parabola",
                    CurveDefinition::ParabolaSegment {
                        vertex,
                        focus,
                        trim_start,
                        trim_end,
                    },
                )
                .expect("parabola");
            fixture_with_distance(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::ComputedCurve,
                Some(vertex),
                0.1,
                [0.0, 0.0],
            )
        }
        GeometryCase::Hyperbola => {
            let center = add_point(&mut document, "hyperbola center", [0.0, 0.0]);
            let transverse_axis_point = add_point(&mut document, "hyperbola axis", [2.0, 0.0]);
            let semi_conjugate = add_scalar(
                &mut document,
                "hyperbola conjugate",
                1.25,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let trim_start = finite_scalar(
                &mut document,
                "hyperbola trim start",
                -0.45,
                ScalarUnit::Parameter,
            );
            let trim_end = finite_scalar(
                &mut document,
                "hyperbola trim end",
                0.45,
                ScalarUnit::Parameter,
            );
            let curve = document
                .add_curve(
                    "hyperbola",
                    CurveDefinition::HyperbolaSegment {
                        center,
                        transverse_axis_point,
                        semi_conjugate,
                        branch: DocumentHyperbolaBranch::Positive,
                        trim_start,
                        trim_end,
                    },
                )
                .expect("hyperbola");
            fixture_with_distance(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::ComputedCurve,
                Some(transverse_axis_point),
                0.1,
                [2.0, 0.0],
            )
        }
        GeometryCase::QuadraticBezier => {
            let controls = [
                add_point(&mut document, "quadratic start", [0.0, 0.0]),
                add_point(&mut document, "quadratic middle", [2.0, 1.0]),
                add_point(&mut document, "quadratic end", [4.0, 0.0]),
            ];
            let curve = document
                .add_curve(
                    "quadratic Bezier",
                    CurveDefinition::QuadraticBezier { controls },
                )
                .expect("quadratic Bezier");
            fixture(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::ComputedCurve,
                Some(controls[1]),
            )
        }
        GeometryCase::CubicBezier => {
            let controls = [
                add_point(&mut document, "cubic start", [0.0, 0.0]),
                add_point(&mut document, "cubic first", [1.25, 0.8]),
                add_point(&mut document, "cubic second", [2.75, 0.8]),
                add_point(&mut document, "cubic end", [4.0, 0.0]),
            ];
            let curve = document
                .add_curve("cubic Bezier", CurveDefinition::CubicBezier { controls })
                .expect("cubic Bezier");
            fixture(
                document,
                OperandSelection::Spans(vec![CurveSpan::line(curve)]),
                OffsetAuthoringRoute::ComputedCurve,
                Some(controls[1]),
            )
        }
        GeometryCase::BSplineClamped => spline_fixture(document, false, false),
        GeometryCase::BSplinePeriodic => spline_fixture(document, false, true),
        GeometryCase::NurbsClamped => spline_fixture(document, true, false),
        GeometryCase::NurbsPeriodic => spline_fixture(document, true, true),
        GeometryCase::MixedChain => mixed_chain_fixture(document),
        GeometryCase::Face => face_fixture(document, false),
        GeometryCase::FaceWithHole => face_fixture(document, true),
    }
}

fn fixture(
    document: SketchDocument,
    selection: OperandSelection,
    route: OffsetAuthoringRoute,
    proxy_point: Option<DesignPointId>,
) -> Fixture {
    fixture_with_distance(document, selection, route, proxy_point, 0.2, [0.0, 0.0])
}

fn fixture_with_distance(
    mut document: SketchDocument,
    selection: OperandSelection,
    route: OffsetAuthoringRoute,
    proxy_point: Option<DesignPointId>,
    distance: f64,
    viewport_center: [f64; 2],
) -> Fixture {
    let proxy_constraint_follower = proxy_point.map(|point| {
        let position = document
            .point(point)
            .expect("declared Offset proxy source point")
            .position;
        let follower = document
            .add_point("Offset proxy constraint follower", position)
            .expect("finite Offset proxy constraint follower");
        document
            .add_constraint(
                "Offset proxy source coincidence",
                DocumentConstraintDefinition::Coincident {
                    first: point,
                    second: follower,
                },
            )
            .expect("ordinary Offset proxy source constraint");
        follower
    });
    Fixture {
        document,
        selection,
        route,
        distance,
        viewport_center,
        proxy_point,
        proxy_constraint_follower,
    }
}

fn ellipse_fixture(mut document: SketchDocument, full: bool) -> Fixture {
    let center = add_point(&mut document, "ellipse center", [0.0, 0.0]);
    let major_axis_point = add_point(
        &mut document,
        "ellipse axis",
        if full { [0.5, 0.0] } else { [3.0, 0.0] },
    );
    let minor_axis_ratio = add_scalar(
        &mut document,
        "ellipse ratio",
        0.75,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    let (definition, selection) = if full {
        (
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            },
            None,
        )
    } else {
        let start_angle = finite_scalar(&mut document, "ellipse arc start", 0.0, ScalarUnit::Angle);
        let end_angle = finite_scalar(
            &mut document,
            "ellipse arc end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
        );
        (
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            Some(()),
        )
    };
    let curve = document
        .add_curve(if full { "ellipse" } else { "elliptical arc" }, definition)
        .expect("ellipse family");
    fixture_with_distance(
        document,
        if selection.is_some() {
            OperandSelection::Spans(vec![CurveSpan::line(curve)])
        } else {
            OperandSelection::FaceAt([0.0, 0.0])
        },
        OffsetAuthoringRoute::ComputedCurve,
        Some(major_axis_point),
        if full { 0.02 } else { 0.1 },
        [0.0, 0.0],
    )
}

fn spline_fixture(mut document: SketchDocument, rational: bool, periodic: bool) -> Fixture {
    if periodic {
        let controls = [
            [20.0, 0.0],
            [21.5, -0.2],
            [22.0, 1.4],
            [20.5, 2.2],
            [19.2, 1.0],
        ]
        .into_iter()
        .enumerate()
        .map(|(index, position)| {
            add_point(
                &mut document,
                &format!("periodic control {index}"),
                position,
            )
        })
        .collect::<Vec<_>>();
        let definition = if rational {
            let weights = add_weights(&mut document, "periodic NURBS", controls.len());
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.clone(),
                gauge_weight: weights[0],
                weights,
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            }
        } else {
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.clone(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            }
        };
        document
            .add_curve(
                if rational {
                    "periodic NURBS"
                } else {
                    "periodic B-spline"
                },
                definition,
            )
            .expect("periodic spline");
        fixture_with_distance(
            document,
            OperandSelection::FaceAt([20.5, 1.0]),
            OffsetAuthoringRoute::ComputedCurve,
            Some(controls[2]),
            0.05,
            [20.5, 1.0],
        )
    } else {
        let controls = [[0.0, 0.0], [1.0, 1.0], [3.0, 1.0], [4.0, 0.0]]
            .into_iter()
            .enumerate()
            .map(|(index, position)| {
                add_point(&mut document, &format!("clamped control {index}"), position)
            })
            .collect::<Vec<_>>();
        let definition = if rational {
            let weights = add_weights(&mut document, "clamped NURBS", controls.len());
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.clone(),
                gauge_weight: weights[0],
                weights,
                knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 74,
            }
        } else {
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.clone(),
                knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 74,
            }
        };
        let curve = document
            .add_curve(
                if rational {
                    "clamped NURBS"
                } else {
                    "clamped B-spline"
                },
                definition,
            )
            .expect("clamped spline");
        fixture_with_distance(
            document,
            OperandSelection::Spans(vec![
                CurveSpan { curve, segment: 41 },
                CurveSpan { curve, segment: 73 },
            ]),
            OffsetAuthoringRoute::ComputedCurve,
            Some(controls[1]),
            0.1,
            [2.0, 0.5],
        )
    }
}

fn mixed_chain_fixture(mut document: SketchDocument) -> Fixture {
    let start = add_point(&mut document, "mixed start", [0.0, 0.0]);
    let join = add_point(&mut document, "mixed join", [2.0, 0.0]);
    let line = add_line(&mut document, "mixed line", start, join);
    let control = add_point(&mut document, "mixed control", [3.0, 0.8]);
    let end = add_point(&mut document, "mixed end", [4.0, 0.0]);
    let curve = document
        .add_curve(
            "mixed quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [join, control, end],
            },
        )
        .expect("mixed quadratic");
    fixture_with_distance(
        document,
        OperandSelection::Spans(vec![CurveSpan::line(line), CurveSpan::line(curve)]),
        OffsetAuthoringRoute::ComputedCurve,
        Some(control),
        0.1,
        [2.0, 0.0],
    )
}

fn face_fixture(mut document: SketchDocument, with_hole: bool) -> Fixture {
    let outer_center = add_point(&mut document, "outer ellipse center", [0.0, 0.0]);
    let outer_axis = add_point(&mut document, "outer ellipse axis", [3.5, 0.0]);
    let outer_ratio = add_scalar(
        &mut document,
        "outer ellipse ratio",
        0.7,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    );
    document
        .add_curve(
            "outer ellipse",
            CurveDefinition::Ellipse {
                center: outer_center,
                major_axis_point: outer_axis,
                minor_axis_ratio: outer_ratio,
            },
        )
        .expect("outer ellipse");
    if with_hole {
        let hole_center = add_point(&mut document, "circle hole center", [0.0, 0.0]);
        let hole_radius = add_scalar(
            &mut document,
            "circle hole radius",
            0.75,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        document
            .add_curve(
                "circle hole",
                CurveDefinition::Circle {
                    center: hole_center,
                    radius: hole_radius,
                },
            )
            .expect("circle hole");
    }
    fixture_with_distance(
        document,
        OperandSelection::FaceAt(if with_hole { [-2.0, 0.0] } else { [0.0, 0.0] }),
        OffsetAuthoringRoute::ComputedCurve,
        Some(outer_axis),
        0.05,
        [0.0, 0.0],
    )
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).expect("line start").position;
    let second = document.point(end).expect("line end").position;
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
        .expect("line")
}

fn add_point(document: &mut SketchDocument, label: &str, position: [f64; 2]) -> DesignPointId {
    document.add_point(label, position).expect("finite point")
}

fn finite_scalar(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
    unit: ScalarUnit,
) -> geosolve_sketch::DesignScalarId {
    add_scalar(document, label, value, unit, ScalarDomain::Finite)
}

fn add_scalar(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
    unit: ScalarUnit,
    domain: ScalarDomain,
) -> geosolve_sketch::DesignScalarId {
    document
        .add_scalar(label, value, unit, domain)
        .expect("finite scalar")
}

fn add_weights(
    document: &mut SketchDocument,
    label: &str,
    count: usize,
) -> Vec<geosolve_sketch::DesignScalarId> {
    (0..count)
        .map(|index| {
            add_scalar(
                document,
                &format!("{label} weight {index}"),
                1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
        })
        .collect()
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn assert_current_accepted(session: &RetainedSketchDocumentSession) {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("golden input must be current and accepted");
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );
}

fn input_fingerprint(parts: &[&str]) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
    format!("input-{:016x}", fnv1a64(&bytes))
}

fn sanitize_tsv(value: &str) -> String {
    value
        .chars()
        .map(|value| match value {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

fn panic_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".into()
    }
}
