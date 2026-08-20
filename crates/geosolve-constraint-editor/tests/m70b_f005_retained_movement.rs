// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ComputedCornerRef, ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeature,
    ComputedFeatureCornerId, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureId,
    ComputedFilletCorner, ComputedSceneState, CoordinatorError, EditorEffect, EditorProblemScope,
    EditorScene, Modifiers, NativeCurveSpanSource, PointerInput, ProjectedDragRejectionStage,
    ReplayAction, RetainedEditorCoordinator, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveId, DesignPointId, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentEdit, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentSolveRequest,
    ParameterBatch, PersistentId, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};

const PAYLOAD_FINGERPRINT: &str = "4228:0823d31f269300af";
const STALE_CIRCLE_CERTIFICATE_UPPER: f64 = 7.857_323_073_392_596;

const SKETCH_JSON: &str = concat!(
    r#"{"version":4,"id":"7653a0003fed873aee16ee394279fe5e","next_id":"7653a0003fed873aee16ee394279fe65","model_scale":10.0,"points":["#,
    r#"{"id":"7653a0003fed873aee16ee394279fe5f","label":"draft point","position":[0.16002449354493023,1.9065418176251467]},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe62","label":"draft point","position":[-2.6404041434913528,2.0437056692350866]},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe63","label":"draft point","position":[1.371638516099403,4.855564627238864]}],"#,
    r#""scalars":[{"id":"7653a0003fed873aee16ee394279fe60","label":"radius","value":2.201783656372145,"unit":"length","domain":{"kind":"positive"}}],"#,
    r#""curves":[{"id":"7653a0003fed873aee16ee394279fe61","label":"circle","definition":{"kind":"circle","center":"7653a0003fed873aee16ee394279fe5f","radius":"7653a0003fed873aee16ee394279fe60"}},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe64","label":"line","definition":{"kind":"line","start":"7653a0003fed873aee16ee394279fe62","end":"7653a0003fed873aee16ee394279fe63","branch_direction":[0.9748804436785523,0.22272880490208083]}}],"#,
    r#""contacts":[],"trim_views":[],"constraints":[],"dimensions":[],"source_order":[]}"#,
);

const FEATURE_JSON: &str = concat!(
    r#"{"version":1,"document_id":"1136cf735081f15888738f4d370b9b2d","sketch_document":"7653a0003fed873aee16ee394279fe5e","revision":7,"next_feature_id":"0000000000000002","next_corner_id":"0000000000000002","features":["#,
    r#"{"id":"0000000000000001","label":"Fillet 1","suppressed":false,"definition":{"kind":"fillet_set","radius":1.0,"corners":["#,
    r#"{"id":"0000000000000001","first":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe61","segment":0}},"picked_parameter":0.01630131737160223,"winding":1,"neighborhood":{"local":{"lower":4.959571177211237,"upper":7.857323073392596}},"normal_side":"right","retained_endpoint":"end","periodic_anchor":{"parameter":3.1578939709613953,"winding":0}},"#,
    r#""second":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe64","segment":0}},"picked_parameter":0.6995120213306758,"winding":0,"neighborhood":"interior","normal_side":"left","retained_endpoint":"start","periodic_anchor":null},"#,
    r#""endpoint_order":"first_then_second","sweep":"counter_clockwise"}]}}],"digest":"df8408ece03aa63593d91056ed1d09592f4f1f2654cb2616f205be04cb217081"}"#,
);

#[derive(Clone, Copy, Debug, PartialEq)]
struct BranchSemantics {
    first_source: NativeCurveSpanSource,
    first_normal_side: DocumentCurveNormalSide,
    first_retained_endpoint: DocumentFilletTrimEndpoint,
    second_source: NativeCurveSpanSource,
    second_normal_side: DocumentCurveNormalSide,
    second_retained_endpoint: DocumentFilletTrimEndpoint,
    endpoint_order: DocumentFilletEndpointOrder,
    sweep: DocumentArcSweep,
}

impl From<ComputedFilletCorner> for BranchSemantics {
    fn from(corner: ComputedFilletCorner) -> Self {
        Self {
            first_source: corner.first.source,
            first_normal_side: corner.first.normal_side,
            first_retained_endpoint: corner.first.retained_endpoint,
            second_source: corner.second.source,
            second_normal_side: corner.second.normal_side,
            second_retained_endpoint: corner.second.retained_endpoint,
            endpoint_order: corner.endpoint_order,
            sweep: corner.sweep,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PersistentFeatureSemantics {
    feature: ComputedFeatureId,
    label: String,
    suppressed: bool,
    radius: f64,
    corner: ComputedFilletCorner,
}

#[derive(Clone, Copy, Debug)]
struct ArcSignature {
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    circle_parameter: f64,
    line_parameter: f64,
    circle_contact: [f64; 2],
    line_contact: [f64; 2],
}

struct Fixture {
    coordinator: RetainedEditorCoordinator,
    owner: ComputedCornerRef,
    line_start: DesignPointId,
    line_end: DesignPointId,
    circle_center: [f64; 2],
    circle_source: NativeCurveSpanSource,
    line_source: NativeCurveSpanSource,
    branch: BranchSemantics,
    stale_seed_barrier_angle: f64,
}

#[derive(Debug, PartialEq)]
struct CoordinatorFingerprint {
    design: String,
    accepted: Option<String>,
    design_identity: geosolve_sketch::SketchDesignIdentity,
    attempt_identity: geosolve_sketch::SketchAttemptIdentity,
    feature_document: String,
    feature_identity: geosolve_sketch_features::ComputedFeatureDocumentIdentity,
    computed_scene: String,
    checkpoint_design: String,
    checkpoint_accepted: Option<String>,
    checkpoint_features: String,
    checkpoint_revisions: String,
    checkpoint_sketch_high_water: String,
    checkpoint_feature_high_water: String,
    checkpoint_evaluation_high_water: String,
    history_len: usize,
    history_cursor: usize,
    can_undo: bool,
    can_redo: bool,
    transcript: Vec<ReplayAction>,
}

fn coordinator_fingerprint(coordinator: &RetainedEditorCoordinator) -> CoordinatorFingerprint {
    let checkpoint = coordinator
        .persistence_checkpoint()
        .expect("complete coordinator checkpoint fingerprint");
    CoordinatorFingerprint {
        design: coordinator.session().export_design_json().unwrap(),
        accepted: coordinator.session().export_accepted_json().unwrap(),
        design_identity: coordinator.session().design_identity(),
        attempt_identity: coordinator.session().last_attempt().identity(),
        feature_document: coordinator.feature_document().to_json().unwrap(),
        feature_identity: coordinator.feature_document().identity(),
        computed_scene: format!("{:?}", coordinator.computed_scene_state()),
        checkpoint_design: checkpoint.design_json().to_owned(),
        checkpoint_accepted: checkpoint.accepted_json().map(str::to_owned),
        checkpoint_features: checkpoint.feature_json().to_owned(),
        checkpoint_revisions: format!("{:?}", checkpoint.revisions()),
        checkpoint_sketch_high_water: format!("{:?}", checkpoint.sketch_identity_high_water()),
        checkpoint_feature_high_water: format!("{:?}", checkpoint.feature_lifecycle_high_water()),
        checkpoint_evaluation_high_water: format!(
            "{:?}",
            checkpoint.computed_evaluation_high_water()
        ),
        history_len: coordinator.history_len(),
        history_cursor: coordinator.history_cursor(),
        can_undo: coordinator.can_undo(),
        can_redo: coordinator.can_redo(),
        transcript: coordinator.transcript().to_vec(),
    }
}

fn exact_documents() -> (SketchDocument, ComputedFeatureDocument) {
    let document = SketchDocument::from_json(SKETCH_JSON).expect("payload sketch JSON");
    assert_eq!(
        document.to_canonical_json().expect("canonical sketch"),
        SKETCH_JSON,
        "{PAYLOAD_FINGERPRINT}: exact sketch fixture drifted"
    );
    let features =
        ComputedFeatureDocument::from_json(FEATURE_JSON).expect("payload feature sidecar JSON");
    assert_eq!(
        features.to_json().expect("canonical feature sidecar"),
        FEATURE_JSON,
        "{PAYLOAD_FINGERPRINT}: exact feature fixture drifted"
    );
    (document, features)
}

fn fixture_from_documents(document: SketchDocument, features: ComputedFeatureDocument) -> Fixture {
    let feature = &features.features()[0];
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
        panic!("expected FilletSet feature");
    };
    let corner = fillet.corners[0];
    let owner = ComputedCornerRef {
        feature: feature.id,
        corner: corner.id,
    };
    let circle_source = corner.first.source;
    let line_source = corner.second.source;
    let (line_start, line_end) = match &document
        .curve(line_source.span.curve)
        .expect("line source")
        .definition
    {
        CurveDefinition::Line { start, end, .. } => (*start, *end),
        other => panic!("payload affine source is not a line: {other:?}"),
    };
    let circle_center_id = match &document
        .curve(circle_source.span.curve)
        .expect("circle source")
        .definition
    {
        CurveDefinition::Circle { center, .. } => *center,
        other => panic!("payload periodic source is not a circle: {other:?}"),
    };
    let circle_center = document
        .point(circle_center_id)
        .expect("circle center")
        .position;
    let seed_total =
        corner.first.picked_parameter + f64::from(corner.first.winding) * std::f64::consts::TAU;
    let seed_tangent = document
        .evaluate_curve_jet(circle_source.span, seed_total)
        .expect("persisted circle seed")
        .first_derivative;
    let stale_seed_barrier_angle = seed_tangent.y.atan2(seed_tangent.x);
    assert!(
        (85.0_f64.to_radians()..95.0_f64.to_radians()).contains(&stale_seed_barrier_angle),
        "the exact payload's stale certificate barrier should be near the 90-degree mark"
    );

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained payload sketch");
    let coordinator =
        RetainedEditorCoordinator::with_features(session, features).expect("composite coordinator");

    Fixture {
        coordinator,
        owner,
        line_start,
        line_end,
        circle_center,
        circle_source,
        line_source,
        branch: corner.into(),
        stale_seed_barrier_angle,
    }
}

fn exact_fixture() -> Fixture {
    let (document, features) = exact_documents();
    fixture_from_documents(document, features)
}

fn exact_fixture_with_unrelated_failed_feature() -> (Fixture, ComputedFeatureId) {
    let (document, mut features) = exact_documents();
    let ComputedFeatureDefinition::FilletSet(current) = &features.features()[0].definition else {
        panic!("expected current FilletSet feature");
    };
    let mut missing = current.corners[0].without_id();
    missing.first.source.span.curve = CurveId(PersistentId::from_u128(0xf005_0001));
    missing.second.source.span.curve = CurveId(PersistentId::from_u128(0xf005_0002));
    let failed = features
        .create_fillet_set("intentional unrelated missing source", 1.0, vec![missing])
        .expect("structurally valid missing-source feature");
    (fixture_from_documents(document, features), failed)
}

fn persistent_feature_semantics(
    coordinator: &RetainedEditorCoordinator,
    owner: ComputedCornerRef,
) -> PersistentFeatureSemantics {
    let feature = coordinator
        .feature_document()
        .feature(owner.feature)
        .expect("persistent feature identity");
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
        panic!("expected persistent FilletSet feature");
    };
    let corner = *fillet
        .corners
        .iter()
        .find(|corner| corner.id == owner.corner)
        .expect("persistent corner identity");
    assert_eq!(fillet.corners.len(), 1);
    PersistentFeatureSemantics {
        feature: feature.id,
        label: feature.label.clone(),
        suppressed: feature.suppressed,
        radius: fillet.radius,
        corner,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, context: &str) {
    assert!(
        actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= tolerance,
        "{context}: expected {expected:.16}, got {actual:.16}"
    );
}

fn assert_signature_close(actual: ArcSignature, expected: ArcSignature, context: &str) {
    for axis in 0..2 {
        assert_close(actual.center[axis], expected.center[axis], 2.0e-8, context);
        assert_close(
            actual.circle_contact[axis],
            expected.circle_contact[axis],
            2.0e-8,
            context,
        );
        assert_close(
            actual.line_contact[axis],
            expected.line_contact[axis],
            2.0e-8,
            context,
        );
    }
    for (actual, expected) in [
        (actual.radius, expected.radius),
        (actual.start_angle, expected.start_angle),
        (actual.end_angle, expected.end_angle),
        (actual.circle_parameter, expected.circle_parameter),
        (actual.line_parameter, expected.line_parameter),
    ] {
        assert_close(actual, expected, 2.0e-8, context);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exact computed-scene assertion keeps branch, geometry, residual and trimming invariants together"
)]
fn assert_current(
    coordinator: &RetainedEditorCoordinator,
    owner: ComputedCornerRef,
    branch: BranchSemantics,
) -> ArcSignature {
    let semantics = persistent_feature_semantics(coordinator, owner);
    assert_eq!(semantics.feature, owner.feature);
    assert_eq!(semantics.corner.id, owner.corner);
    assert_eq!(BranchSemantics::from(semantics.corner), branch);
    assert!(!semantics.suppressed);
    assert_eq!(semantics.radius.to_bits(), 1.0_f64.to_bits());

    let (expected, snapshot) = match coordinator.computed_scene_state() {
        ComputedSceneState::Current { expected, snapshot } => (*expected, snapshot),
        state => panic!(
            "{PAYLOAD_FINGERPRINT}: persistent Fillet was not Current after source movement: {state:?}"
        ),
    };
    assert_eq!(snapshot.input(), expected);
    assert_eq!(expected.features, coordinator.feature_document().identity());
    let visible_session = coordinator
        .solved_preview_session()
        .unwrap_or_else(|| coordinator.session());
    assert_eq!(
        visible_session.accepted_prepared_input(),
        Some(expected.sketch)
    );

    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == owner.feature)
        .expect("stable feature evaluation identity");
    let ComputedFeatureEvaluationState::Current { corner_edges, .. } = &evaluation.state else {
        panic!(
            "{PAYLOAD_FINGERPRINT}: persistent Fillet evaluation failed: {:?}",
            evaluation.state
        );
    };
    assert!(matches!(corner_edges.as_slice(), [(corner, _)] if *corner == owner.corner));

    let edge = snapshot
        .fillet_arc_edge(owner)
        .expect("same persistent corner must own one generated arc");
    let ComputedEdgeProvenance::FilletArc {
        owner: edge_owner,
        sources,
    } = edge.provenance
    else {
        panic!("persistent corner did not publish Fillet provenance");
    };
    assert_eq!(edge_owner, owner);
    assert_eq!(sources, [branch.first_source, branch.second_source]);
    let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
        panic!("persistent corner did not publish circular-arc geometry");
    };
    assert_eq!(arc.sweep, branch.sweep);
    assert_eq!(arc.contacts[0].source, branch.first_source);
    assert_eq!(arc.contacts[1].source, branch.second_source);
    assert!(
        arc.center
            .into_iter()
            .chain([arc.radius, arc.start_angle, arc.end_angle])
            .chain(arc.contacts.iter().flat_map(|contact| {
                [
                    contact.parameter,
                    contact.total_parameter,
                    contact.position[0],
                    contact.position[1],
                ]
            }))
            .all(f64::is_finite),
        "generated Fillet geometry must remain finite"
    );

    let accepted = visible_session
        .accepted_state_for_current_input()
        .expect("current accepted source movement");
    for contact in arc.contacts {
        let jet = accepted
            .document()
            .evaluate_curve_jet(contact.source.span, contact.total_parameter)
            .expect("source contact jet");
        let incidence =
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1]);
        assert!(
            incidence <= 2.0e-8,
            "source incidence residual {incidence:.12e}"
        );
        let radial = [
            arc.center[0] - contact.position[0],
            arc.center[1] - contact.position[1],
        ];
        let tangent = [jet.first_derivative.x, jet.first_derivative.y];
        let normalized_tangency = tangent[0].mul_add(radial[0], tangent[1] * radial[1]).abs()
            / (tangent[0].hypot(tangent[1]) * radial[0].hypot(radial[1]));
        assert!(
            normalized_tangency <= 2.0e-8,
            "normalized tangency residual {normalized_tangency:.12e}"
        );
    }

    assert_eq!(
        snapshot.source_fragment_edges(branch.first_source).count(),
        0,
        "the closed circle must remain a full native source"
    );
    assert_eq!(
        snapshot
            .source_construction_fragments(branch.first_source)
            .count(),
        0,
        "the closed circle must not acquire a discarded complement"
    );
    assert_eq!(
        snapshot.source_fragment_edges(branch.second_source).count(),
        1,
        "the open line must retain exactly one visible side"
    );
    assert_eq!(snapshot.replaced_sources(), &[branch.second_source]);

    ArcSignature {
        center: arc.center,
        radius: arc.radius,
        start_angle: arc.start_angle,
        end_angle: arc.end_angle,
        circle_parameter: arc.contacts[0].total_parameter,
        line_parameter: arc.contacts[1].total_parameter,
        circle_contact: arc.contacts[0].position,
        line_contact: arc.contacts[1].position,
    }
}

fn assert_current_with_unrelated_failure(
    coordinator: &RetainedEditorCoordinator,
    owner: ComputedCornerRef,
    branch: BranchSemantics,
    failed_feature: ComputedFeatureId,
    expected_feature: &ComputedFeature,
    expected_failure: &ComputedFeatureFailure,
) -> ArcSignature {
    let signature = assert_current(coordinator, owner, branch);
    assert_eq!(
        coordinator.feature_document().feature(failed_feature),
        Some(expected_feature),
        "unrelated failed intent must not be rewritten while another feature re-anchors"
    );
    let ComputedSceneState::Current { snapshot, .. } = coordinator.computed_scene_state() else {
        panic!("mixed Current/Failed evaluation must remain globally publishable");
    };
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == failed_feature)
        .expect("unrelated failed evaluation identity");
    assert!(matches!(
        &evaluation.state,
        ComputedFeatureEvaluationState::Failed { failure } if failure == expected_failure
    ));
    let ComputedFeatureDefinition::FilletSet(fillet) = &expected_feature.definition else {
        panic!("expected failed FilletSet feature");
    };
    for corner in &fillet.corners {
        assert!(
            snapshot
                .fillet_arc_edge(ComputedCornerRef {
                    feature: failed_feature,
                    corner: corner.id,
                })
                .is_none(),
            "failed feature must not publish generated geometry"
        );
    }
    signature
}

fn visible_scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
    let visible_session = coordinator
        .solved_preview_session()
        .unwrap_or_else(|| coordinator.session());
    let accepted = visible_session
        .accepted_state_for_current_input()
        .expect("visible accepted movement state");
    let (expected, snapshot) = match coordinator.computed_scene_state() {
        ComputedSceneState::Current { expected, snapshot } => (*expected, snapshot),
        state => panic!("{PAYLOAD_FINGERPRINT}: visible computed scene is {state:?}"),
    };
    EditorScene::from_accepted_with_computed(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        visible_session.design_document(),
        &visible_session
            .accepted_prepared_input()
            .expect("visible accepted input"),
        &expected,
        snapshot,
        Viewport::new([900.0, 700.0], [0.0, 0.0], 50.0).expect("movement viewport"),
        0.5,
    )
    .expect("exact current movement scene")
}

fn commit_position(
    coordinator: &mut RetainedEditorCoordinator,
    point: DesignPointId,
    position: [f64; 2],
) {
    let history_before = coordinator.history_len();
    let transcript_before = coordinator.transcript().len();
    let expected = coordinator.session().design_identity();
    let outcome = coordinator
        .apply_edit(expected, DocumentEdit::SetPointPosition { point, position })
        .expect("accepted retained source edit");
    assert!(outcome.published_accepted.is_some());
    assert_eq!(coordinator.history_len(), history_before + 1);
    assert_eq!(coordinator.transcript().len(), transcript_before + 1);
}

fn resolve_projected_sample(
    coordinator: &mut RetainedEditorCoordinator,
    pointer_id: u64,
    point: DesignPointId,
    model_position: [f64; 2],
) -> (ScreenPoint, Vec<EditorEffect>) {
    let scene = visible_scene(coordinator);
    let screen_position = scene.viewport.model_to_screen(model_position);
    let request = coordinator.editor_mut().pointer_move(
        &scene,
        PointerInput {
            pointer_id,
            position: screen_position,
            modifiers: Modifiers::default(),
        },
    );
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: requested_pointer,
            request_id,
            point: requested_point,
            model_position: requested_position,
        },
    ] = request.as_slice()
    else {
        panic!("projected sample did not request exactly one move: {request:?}");
    };
    assert_eq!(*requested_pointer, pointer_id);
    assert_eq!(*requested_point, point);
    let effects = coordinator.resolve_projected_point_move(
        *requested_pointer,
        *request_id,
        *requested_point,
        *requested_position,
    );
    (screen_position, effects)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact projected gesture freezes sibling-preview provenance, cardinal continuity and durable replay together"
)]
fn m70b_f005_projected_gesture_crosses_cardinal_mark_and_commits_exact_branch() {
    let mut fixture = exact_fixture();
    let initial_feature_identity = fixture.coordinator.feature_document().identity();
    let initial_history = fixture.coordinator.history_len();
    let initial_scene = visible_scene(&fixture.coordinator);
    let press = initial_scene
        .points
        .iter()
        .find(|point| point.id == fixture.line_start)
        .expect("movable line endpoint in exact scene")
        .screen_position;
    let pointer_id = 0xf005;
    let pointer = |position| PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    };
    let expected_design = fixture.coordinator.session().design_identity();
    let _ = fixture
        .coordinator
        .pointer_down(&initial_scene, pointer(press));

    let original_start = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    // Moving this one ordinary endpoint down carries the accepted circle
    // contact from 93.17 degrees through the stale 90.19-degree certificate
    // edge and the true 90-degree cardinal point while the line contact stays
    // strictly inside the finite segment. This is the exact movement seam from
    // the UAT report, without conflating it with a genuine segment-end escape.
    let vertical_offsets = [
        -0.25, -0.5, -0.7, -0.745_14, -0.75, -0.8, -1.0, -0.8, -0.5, -0.25, 0.0,
    ];
    let mut previous_parameter: Option<f64> = None;
    let mut observed_parameter_range = (f64::INFINITY, f64::NEG_INFINITY);
    let mut last_screen = press;
    for (sample, offset) in vertical_offsets.into_iter().enumerate() {
        let target = [original_start[0], original_start[1] + offset];
        let scene = visible_scene(&fixture.coordinator);
        last_screen = scene.viewport.model_to_screen(target);
        let request = fixture
            .coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(last_screen));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id: requested_pointer,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("sample {sample} did not request one projected move: {request:?}");
        };
        assert_eq!(*requested_pointer, pointer_id);
        assert_eq!(*point, fixture.line_start);
        let effects = fixture.coordinator.resolve_projected_point_move(
            *requested_pointer,
            *request_id,
            *point,
            *model_position,
        );
        assert!(matches!(
            effects.as_slice(),
            [EditorEffect::PreviewPointMove { point, .. }] if *point == fixture.line_start
        ));
        let current = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
        assert!(
            0.0 < current.line_parameter && current.line_parameter < 1.0,
            "sample {sample} escaped the finite line while exercising the cardinal seam"
        );
        observed_parameter_range.0 = observed_parameter_range.0.min(current.circle_parameter);
        observed_parameter_range.1 = observed_parameter_range.1.max(current.circle_parameter);
        if let Some(previous) = previous_parameter {
            assert!(
                (current.circle_parameter - previous).abs() < 0.75,
                "sample {sample} hopped from circle parameter {previous:.16} to {:.16}",
                current.circle_parameter
            );
        }
        previous_parameter = Some(current.circle_parameter);
    }
    assert!(
        observed_parameter_range.0 < std::f64::consts::TAU + std::f64::consts::FRAC_PI_2
            && observed_parameter_range.1 > STALE_CIRCLE_CERTIFICATE_UPPER,
        "the gesture must cross both the true cardinal and stale numeric certificate edge: {observed_parameter_range:?}"
    );

    let release_scene = visible_scene(&fixture.coordinator);
    let release = fixture.coordinator.editor_mut().pointer_up(
        &release_scene,
        expected_design,
        pointer(last_screen),
    );
    assert!(matches!(
        release.as_slice(),
        [EditorEffect::CommitPointMove { point, .. }] if *point == fixture.line_start
    ));
    let mutation = fixture
        .coordinator
        .apply_editor_effect(&release[0])
        .expect("commit exact projected movement")
        .expect("point-move mutation");
    assert!(mutation.published_accepted.is_some());
    let committed = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    assert_eq!(fixture.coordinator.history_len(), initial_history + 1);
    assert_ne!(
        fixture.coordinator.feature_document().identity(),
        initial_feature_identity,
        "mouse-up must durably re-anchor the accepted contact frame"
    );
    let [action] = fixture.coordinator.transcript() else {
        panic!("one gesture must record exactly one replay action");
    };
    assert!(matches!(
        action,
        ReplayAction::Edit {
            computed_features: Some(_),
            ..
        }
    ));
    let action = action.clone();
    let checkpoint = fixture
        .coordinator
        .persistence_checkpoint()
        .expect("projected movement checkpoint");

    fixture.coordinator.undo().expect("undo projected movement");
    assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    fixture.coordinator.redo().expect("redo projected movement");
    assert_signature_close(
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch),
        committed,
        "projected movement Undo/Redo",
    );

    let mut replayed = exact_fixture().coordinator;
    replayed.replay(&action).expect("projected movement replay");
    assert_signature_close(
        assert_current(&replayed, fixture.owner, fixture.branch),
        committed,
        "projected movement transcript replay",
    );

    let mut restored = exact_fixture().coordinator;
    restored
        .reload(&checkpoint)
        .expect("projected movement checkpoint restore");
    assert_signature_close(
        assert_current(&restored, fixture.owner, fixture.branch),
        committed,
        "projected movement cold restore",
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the real gesture must keep one complete valid transaction through a blocked sample, reverse recovery and commit"
)]
fn m70b_f005_projected_gesture_blocks_true_barrier_and_recovers_in_reverse() {
    // This target collapses the finite line onto a short segment above the
    // circle. The persisted branch genuinely has no in-segment root; unlike a
    // cardinal certificate seam, this is a real motion barrier.
    let mut barrier_probe = exact_fixture();
    commit_position(
        &mut barrier_probe.coordinator,
        barrier_probe.line_start,
        [0.0, 5.0],
    );
    let ComputedSceneState::Current { snapshot, .. } =
        barrier_probe.coordinator.computed_scene_state()
    else {
        panic!("barrier probe must complete as an attributed feature failure");
    };
    assert!(matches!(
        snapshot.feature_evaluations()[0].state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::NoLocalRoot { .. }
        }
    ));

    let mut fixture = exact_fixture();
    let initial_history = fixture.coordinator.history_len();
    let scene = visible_scene(&fixture.coordinator);
    let press = scene
        .points
        .iter()
        .find(|point| point.id == fixture.line_start)
        .expect("movable line endpoint")
        .screen_position;
    let original = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    let expected_design = fixture.coordinator.session().design_identity();
    let pointer_id = 0xf005_0002;
    fixture.coordinator.pointer_down(
        &scene,
        PointerInput {
            pointer_id,
            position: press,
            modifiers: Modifiers::default(),
        },
    );

    let (_, accepted_effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [original[0], original[1] - 0.5],
    );
    assert!(matches!(
        accepted_effects.as_slice(),
        [EditorEffect::PreviewPointMove { point, .. }] if *point == fixture.line_start
    ));
    let last_valid = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    let last_valid_accepted = fixture
        .coordinator
        .solved_preview_session()
        .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
        .expect("last valid accepted source preview")
        .identity();
    let last_valid_computed = match fixture.coordinator.computed_scene_state() {
        ComputedSceneState::Current { expected, snapshot } => {
            assert_eq!(*expected, snapshot.input());
            snapshot.input()
        }
        state => panic!("last valid computed preview is not Current: {state:?}"),
    };

    let (_, blocked_effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [0.0, 5.0],
    );
    assert!(
        blocked_effects.is_empty(),
        "a genuine computed barrier must not replace the last complete preview"
    );
    let blocked_work = fixture
        .coordinator
        .projected_drag_work_evidence()
        .expect("blocked sample work evidence");
    assert!(!blocked_work.accepted);
    assert_eq!(
        blocked_work.rejection_stage,
        Some(ProjectedDragRejectionStage::PreviewPublication)
    );
    assert_eq!(
        fixture
            .coordinator
            .solved_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
            .expect("blocked sample retains accepted source preview")
            .identity(),
        last_valid_accepted
    );
    assert!(matches!(
        fixture.coordinator.computed_scene_state(),
        ComputedSceneState::Current { expected, snapshot }
            if *expected == last_valid_computed && snapshot.input() == last_valid_computed
    ));
    assert_signature_close(
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch),
        last_valid,
        "blocked sample retains last complete computed scene",
    );
    let problems = fixture.coordinator.computed_feature_problems();
    let [problem] = problems.as_slice() else {
        panic!("one newly blocked Fillet must publish one targeted limit cue");
    };
    assert_eq!(
        problem.feature,
        Some(fixture.owner.feature),
        "blocked problems: {problems:?}"
    );
    assert_eq!(problem.corners, vec![fixture.owner.corner]);
    let mut expected_sources = vec![fixture.circle_source, fixture.line_source];
    expected_sources.sort_unstable();
    assert_eq!(problem.sources, expected_sources);
    assert_eq!(problem.scope, EditorProblemScope::Targeted);
    assert!(
        problem.message.starts_with("Parent limit:")
            || problem.message.starts_with("Fillet movement limit:")
    );

    let reverse_target = [original[0], original[1] - 0.25];
    let (reverse_screen, reverse_effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        reverse_target,
    );
    let [
        EditorEffect::PreviewPointMove {
            point,
            model_position: recovered_position,
        },
    ] = reverse_effects.as_slice()
    else {
        panic!("reverse sample did not recover one projected preview: {reverse_effects:?}");
    };
    assert_eq!(*point, fixture.line_start);
    assert!(fixture.coordinator.computed_feature_problems().is_empty());
    let recovered = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    assert!(
        (recovered.circle_parameter - last_valid.circle_parameter).abs() < 0.75,
        "reverse recovery must continue the same explicit circle root"
    );

    let release_scene = visible_scene(&fixture.coordinator);
    let release = fixture.coordinator.editor_mut().pointer_up(
        &release_scene,
        expected_design,
        PointerInput {
            pointer_id,
            position: reverse_screen,
            modifiers: Modifiers::default(),
        },
    );
    let [
        EditorEffect::CommitPointMove {
            point,
            model_position,
            ..
        },
    ] = release.as_slice()
    else {
        panic!("recovered gesture did not produce one transactional point commit: {release:?}");
    };
    assert_eq!(*point, fixture.line_start);
    for axis in 0..2 {
        assert_close(
            model_position[axis],
            recovered_position[axis],
            1.0e-10,
            "recovered gesture releases its exact accepted projection",
        );
    }
    fixture
        .coordinator
        .apply_editor_effect(&release[0])
        .expect("commit recovered sample")
        .expect("point mutation");
    assert_eq!(fixture.coordinator.history_len(), initial_history + 1);
    assert_signature_close(
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch),
        recovered,
        "recovered sample commit",
    );
}

#[test]
fn m70b_f005_release_after_blocked_sample_commits_only_last_complete_preview() {
    let mut fixture = exact_fixture();
    let initial_history = fixture.coordinator.history_len();
    let scene = visible_scene(&fixture.coordinator);
    let press = scene
        .points
        .iter()
        .find(|point| point.id == fixture.line_start)
        .expect("movable line endpoint")
        .screen_position;
    let expected_design = fixture.coordinator.session().design_identity();
    let pointer_id = 0xf005_0003;
    fixture.coordinator.pointer_down(
        &scene,
        PointerInput {
            pointer_id,
            position: press,
            modifiers: Modifiers::default(),
        },
    );
    let original = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    let (_, valid_effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [original[0], original[1] - 0.5],
    );
    let [
        EditorEffect::PreviewPointMove {
            point,
            model_position: valid_position,
        },
    ] = valid_effects.as_slice()
    else {
        panic!("valid sample did not publish one projected point: {valid_effects:?}");
    };
    assert_eq!(*point, fixture.line_start);
    let valid_signature = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);

    let (blocked_screen, blocked_effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [0.0, 5.0],
    );
    assert!(blocked_effects.is_empty());
    assert_eq!(fixture.coordinator.computed_feature_problems().len(), 1);
    assert_signature_close(
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch),
        valid_signature,
        "terminal blocked sample retains complete scene",
    );

    let release_scene = visible_scene(&fixture.coordinator);
    let release = fixture.coordinator.editor_mut().pointer_up(
        &release_scene,
        expected_design,
        PointerInput {
            pointer_id,
            position: blocked_screen,
            modifiers: Modifiers::default(),
        },
    );
    let [
        EditorEffect::CommitPointMove {
            point,
            model_position,
            ..
        },
    ] = release.as_slice()
    else {
        panic!("terminal blocked sample did not commit the retained preview: {release:?}");
    };
    assert_eq!(*point, fixture.line_start);
    assert_eq!(
        model_position.map(f64::to_bits),
        valid_position.map(f64::to_bits)
    );
    fixture
        .coordinator
        .apply_editor_effect(&release[0])
        .expect("commit retained valid preview")
        .expect("point mutation");
    assert_eq!(fixture.coordinator.history_len(), initial_history + 1);
    assert!(fixture.coordinator.computed_feature_problems().is_empty());
    assert_signature_close(
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch),
        valid_signature,
        "terminal blocked sample commits retained complete scene",
    );
}

#[test]
fn m70b_f005_first_blocked_sample_releases_without_mutation() {
    let mut fixture = exact_fixture();
    let before = coordinator_fingerprint(&fixture.coordinator);
    let scene = visible_scene(&fixture.coordinator);
    let press = scene
        .points
        .iter()
        .find(|point| point.id == fixture.line_start)
        .expect("movable line endpoint")
        .screen_position;
    let expected_design = fixture.coordinator.session().design_identity();
    let pointer_id = 0xf005_0004;
    fixture.coordinator.pointer_down(
        &scene,
        PointerInput {
            pointer_id,
            position: press,
            modifiers: Modifiers::default(),
        },
    );
    let (blocked_screen, effects) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [0.0, 5.0],
    );
    assert!(effects.is_empty());
    assert_eq!(fixture.coordinator.computed_feature_problems().len(), 1);
    let release_scene = visible_scene(&fixture.coordinator);
    let release = fixture.coordinator.editor_mut().pointer_up(
        &release_scene,
        expected_design,
        PointerInput {
            pointer_id,
            position: blocked_screen,
            modifiers: Modifiers::default(),
        },
    );
    assert_eq!(release, vec![EditorEffect::ClearPointPreview]);
    fixture.coordinator.clear_transient();
    let mut after = coordinator_fingerprint(&fixture.coordinator);
    // Rejected bounded evaluation may consume a never-reused generated-edge
    // revision, but it must not alter any durable or publishable scene state.
    after.checkpoint_evaluation_high_water = before.checkpoint_evaluation_high_water.clone();
    assert_eq!(after, before);
    assert!(fixture.coordinator.computed_feature_problems().is_empty());
}

#[test]
fn m70b_f005_drag_limit_merges_with_unrelated_retained_failure() {
    let (mut fixture, failed_feature) = exact_fixture_with_unrelated_failed_feature();
    let initial_problems = fixture.coordinator.computed_feature_problems();
    assert_eq!(initial_problems.len(), 1);
    assert_eq!(initial_problems[0].feature, Some(failed_feature));
    let scene = visible_scene(&fixture.coordinator);
    let press = scene
        .points
        .iter()
        .find(|point| point.id == fixture.line_start)
        .expect("movable line endpoint")
        .screen_position;
    let pointer_id = 0xf005_0005;
    fixture.coordinator.pointer_down(
        &scene,
        PointerInput {
            pointer_id,
            position: press,
            modifiers: Modifiers::default(),
        },
    );
    let original = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    let (_, valid) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [original[0], original[1] - 0.5],
    );
    assert!(matches!(
        valid.as_slice(),
        [EditorEffect::PreviewPointMove { .. }]
    ));
    let (_, blocked) = resolve_projected_sample(
        &mut fixture.coordinator,
        pointer_id,
        fixture.line_start,
        [0.0, 5.0],
    );
    assert!(blocked.is_empty());
    let problems = fixture.coordinator.computed_feature_problems();
    assert_eq!(problems.len(), 2);
    assert_eq!(problems[0].feature, Some(failed_feature));
    assert_eq!(
        problems[1].feature,
        Some(fixture.owner.feature),
        "blocked problems: {problems:?}"
    );
    assert!(
        problems[1].message.starts_with("Parent limit:")
            || problems[1].message.starts_with("Fillet movement limit:")
    );
    assert!(
        problems
            .iter()
            .all(|problem| problem.scope == EditorProblemScope::Targeted)
    );
    fixture.coordinator.clear_transient();
    assert_eq!(
        fixture.coordinator.computed_feature_problems(),
        initial_problems
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact retained regression keeps movement, commit, history, replay and cold-restore evidence together"
)]
fn m70b_f005_retained_line_circle_motion_reanchors_replays_and_restores() {
    let mut fixture = exact_fixture();
    let initial = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    assert_close(
        initial.circle_parameter,
        7.909_322_804_062_922,
        2.0e-8,
        "payload-derived initial circle contact",
    );

    let barrier = fixture.stale_seed_barrier_angle;
    let angles = [
        35.0_f64.to_radians(),
        70.0_f64.to_radians(),
        90.0_f64.to_radians(),
        barrier - 1.0e-6,
        barrier,
        barrier + 1.0e-6,
        100.0_f64.to_radians(),
        115.0_f64.to_radians(),
        123.0_f64.to_radians(),
        115.0_f64.to_radians(),
        100.0_f64.to_radians(),
        barrier + 1.0e-6,
        barrier,
        barrier - 1.0e-6,
        90.0_f64.to_radians(),
        70.0_f64.to_radians(),
        35.0_f64.to_radians(),
    ];
    let mut previous_stable_parameter: Option<f64> = None;
    let mut first_35_degree_signature = None;
    let initial_history_len = fixture.coordinator.history_len();

    for (step, angle) in angles.into_iter().enumerate() {
        let direction = [angle.cos(), angle.sin()];
        let start_position = [
            fixture.circle_center[0] - 5.0 * direction[0],
            fixture.circle_center[1] - 5.0 * direction[1],
        ];
        let end_position = [
            fixture.circle_center[0] + 5.0 * direction[0],
            fixture.circle_center[1] + 5.0 * direction[1],
        ];

        commit_position(&mut fixture.coordinator, fixture.line_start, start_position);
        assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
        commit_position(&mut fixture.coordinator, fixture.line_end, end_position);
        let current = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);

        if let Some(previous) = previous_stable_parameter {
            assert!(
                (current.circle_parameter - previous).abs() < 0.75,
                "step {step} hopped roots from circle parameter {previous:.16} to {:.16}",
                current.circle_parameter
            );
        }
        previous_stable_parameter = Some(current.circle_parameter);
        if step == 0 {
            first_35_degree_signature = Some(current);
        }
    }

    let final_signature = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    assert_signature_close(
        final_signature,
        first_35_degree_signature.expect("outbound 35-degree sample"),
        "returning through the stale barrier must recover the same 35-degree branch",
    );
    assert_eq!(
        fixture.coordinator.history_len(),
        initial_history_len + angles.len() * 2
    );
    assert_eq!(fixture.coordinator.transcript().len(), angles.len() * 2);
    assert!(fixture.coordinator.transcript().iter().all(|action| {
        matches!(
            action,
            ReplayAction::Edit {
                computed_features: Some(_),
                ..
            }
        )
    }));

    let final_design_json = fixture
        .coordinator
        .session()
        .export_design_json()
        .expect("final canonical sketch");
    let final_feature_json = fixture
        .coordinator
        .feature_document()
        .to_json()
        .expect("final canonical feature sidecar");
    let final_semantics = persistent_feature_semantics(&fixture.coordinator, fixture.owner);
    let transcript = fixture.coordinator.transcript().to_vec();
    let saved = fixture
        .coordinator
        .persistence_checkpoint()
        .expect("durable final checkpoint");
    assert_eq!(
        fixture.coordinator.checkpoint().design_json(),
        final_design_json
    );
    assert_eq!(
        fixture.coordinator.checkpoint().feature_json(),
        final_feature_json
    );
    assert_eq!(saved.design_json(), final_design_json);
    assert_eq!(saved.feature_json(), final_feature_json);

    fixture
        .coordinator
        .undo()
        .expect("undo final committed endpoint edit");
    assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    fixture
        .coordinator
        .redo()
        .expect("redo final committed endpoint edit");
    let redone = assert_current(&fixture.coordinator, fixture.owner, fixture.branch);
    assert_eq!(
        fixture.coordinator.session().export_design_json().unwrap(),
        final_design_json
    );
    assert_eq!(
        persistent_feature_semantics(&fixture.coordinator, fixture.owner),
        final_semantics
    );
    assert_signature_close(redone, final_signature, "Undo/Redo final geometry");

    let mut replayed = exact_fixture().coordinator;
    for action in &transcript {
        replayed
            .replay(action)
            .expect("deterministic movement replay");
        assert_current(&replayed, fixture.owner, fixture.branch);
    }
    let replayed_signature = assert_current(&replayed, fixture.owner, fixture.branch);
    assert_eq!(
        replayed.session().export_design_json().unwrap(),
        final_design_json
    );
    assert_eq!(
        replayed.feature_document().to_json().unwrap(),
        final_feature_json
    );
    assert_eq!(
        persistent_feature_semantics(&replayed, fixture.owner),
        final_semantics
    );
    assert_signature_close(replayed_signature, final_signature, "transcript replay");

    let mut restored = exact_fixture().coordinator;
    restored.reload(&saved).expect("cold composite restore");
    let restored_signature = assert_current(&restored, fixture.owner, fixture.branch);
    assert_eq!(
        restored.session().export_design_json().unwrap(),
        final_design_json
    );
    assert_eq!(
        persistent_feature_semantics(&restored, fixture.owner),
        final_semantics
    );
    assert_signature_close(
        restored_signature,
        final_signature,
        "cold checkpoint restore",
    );

    assert_eq!(fixture.circle_source, fixture.branch.first_source);
    assert_eq!(fixture.line_source, fixture.branch.second_source);
    assert_eq!(fixture.owner.feature.raw(), 1);
    assert_eq!(fixture.owner.corner, ComputedFeatureCornerId::from_raw(1));
}

#[test]
fn m70b_f005_recorded_reanchor_cannot_be_transplanted_to_another_native_edit() {
    let mut recorded = exact_fixture();
    let original = recorded
        .coordinator
        .session()
        .design_document()
        .point(recorded.line_start)
        .expect("payload line start")
        .position;
    let expected = recorded.coordinator.session().design_identity();
    recorded
        .coordinator
        .apply_edit(
            expected,
            DocumentEdit::SetPointPosition {
                point: recorded.line_start,
                position: [original[0], original[1] - 0.75],
            },
        )
        .expect("legitimate movement recording");
    let [
        ReplayAction::Edit {
            expected,
            computed_features: Some(transition),
            ..
        },
    ] = recorded.coordinator.transcript()
    else {
        panic!("legitimate source movement did not record its exact re-anchor transition");
    };

    // The transition's private fields prevent direct branch mutation, but replay
    // must also reject transplanting that authentic transition onto a different
    // native edit. Otherwise the sidecar contact frame is not authenticated as
    // a derivative of the edit in the same replay action.
    let transplanted = ReplayAction::Edit {
        expected: *expected,
        edit: DocumentEdit::SetPointPosition {
            point: recorded.line_start,
            position: [original[0], original[1] - 0.65],
        },
        computed_features: Some(transition.clone()),
    };
    let mut replayed = exact_fixture().coordinator;
    let before = coordinator_fingerprint(&replayed);
    let error = replayed
        .replay(&transplanted)
        .expect_err("a recorded re-anchor belongs to exactly one native edit");
    assert!(matches!(
        error,
        CoordinatorError::StaleComputedFeatureCandidate
    ));
    assert_eq!(
        coordinator_fingerprint(&replayed),
        before,
        "rejected replay must preserve design, acceptance, computed output, allocators, history and transcript atomically"
    );

    // Host inputs do not advance the design identity, so edit/feature identity
    // checks alone cannot authenticate this transition. The recorded prepared
    // input must reject replay after even an otherwise-empty parameter input
    // revision changes.
    let legitimate = recorded.coordinator.transcript()[0].clone();
    let mut changed_input = exact_fixture().coordinator;
    changed_input
        .replace_parameter_batch(
            changed_input.session().design_identity(),
            ParameterBatch::new(1, Vec::new()).expect("empty replacement parameter batch"),
            DocumentSolveRequest::default(),
        )
        .expect("replace host parameter input without changing design identity");
    assert_eq!(changed_input.session().design_identity(), *expected);
    let before = coordinator_fingerprint(&changed_input);
    let error = changed_input
        .replay(&legitimate)
        .expect_err("a recorded re-anchor belongs to one exact durable host input");
    assert!(matches!(
        error,
        CoordinatorError::StaleComputedFeatureCandidate
    ));
    assert_eq!(
        coordinator_fingerprint(&changed_input),
        before,
        "host-input mismatch must reject before native geometry or computed authority changes"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "mixed-disposition movement, replay and reload form one retained transaction contract"
)]
fn m70b_f005_current_reanchor_preserves_unrelated_failed_set_through_replay_and_reload() {
    let (mut fixture, failed_feature) = exact_fixture_with_unrelated_failed_feature();
    let expected_failed_feature = fixture
        .coordinator
        .feature_document()
        .feature(failed_feature)
        .expect("persistent unrelated failed feature")
        .clone();
    let expected_failure = match fixture.coordinator.computed_scene_state() {
        ComputedSceneState::Current { snapshot, .. } => snapshot
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == failed_feature)
            .and_then(|evaluation| match &evaluation.state {
                ComputedFeatureEvaluationState::Failed { failure } => Some(failure.clone()),
                _ => None,
            })
            .expect("unrelated feature must begin Failed"),
        state => panic!("mixed fixture did not publish a complete scene: {state:?}"),
    };
    assert!(matches!(
        expected_failure,
        ComputedFeatureFailure::MissingSource { .. }
    ));
    let initial = assert_current_with_unrelated_failure(
        &fixture.coordinator,
        fixture.owner,
        fixture.branch,
        failed_feature,
        &expected_failed_feature,
        &expected_failure,
    );
    let original = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    let initial_feature_identity = fixture.coordinator.feature_document().identity();

    let mut previous_parameter = initial.circle_parameter;
    let mut final_signature = initial;
    for offset in [-0.70, -0.745_14, -0.75, -0.80] {
        commit_position(
            &mut fixture.coordinator,
            fixture.line_start,
            [original[0], original[1] + offset],
        );
        final_signature = assert_current_with_unrelated_failure(
            &fixture.coordinator,
            fixture.owner,
            fixture.branch,
            failed_feature,
            &expected_failed_feature,
            &expected_failure,
        );
        assert!(
            (final_signature.circle_parameter - previous_parameter).abs() < 0.75,
            "mixed-disposition continuation hopped to another circle root"
        );
        previous_parameter = final_signature.circle_parameter;
    }
    assert!(initial.circle_parameter > 7.857_323_073_392_596);
    assert!(final_signature.circle_parameter < 7.857_323_073_392_596);
    assert_ne!(
        fixture.coordinator.feature_document().identity(),
        initial_feature_identity,
        "current branch contact frame must be durably re-anchored"
    );
    let persisted = persistent_feature_semantics(&fixture.coordinator, fixture.owner);
    let persisted_circle_parameter = persisted.corner.first.picked_parameter
        + f64::from(persisted.corner.first.winding) * std::f64::consts::TAU;
    assert_close(
        persisted_circle_parameter,
        final_signature.circle_parameter,
        2.0e-8,
        "durable current contact re-anchor",
    );

    let final_feature_json = fixture.coordinator.feature_document().to_json().unwrap();
    let final_current_semantics = persisted;
    let transcript = fixture.coordinator.transcript().to_vec();
    let checkpoint = fixture.coordinator.persistence_checkpoint().unwrap();

    let (mut replayed, replayed_failed) = exact_fixture_with_unrelated_failed_feature();
    assert_eq!(replayed_failed, failed_feature);
    for action in &transcript {
        replayed.coordinator.replay(action).expect("mixed replay");
        assert_current_with_unrelated_failure(
            &replayed.coordinator,
            replayed.owner,
            replayed.branch,
            failed_feature,
            &expected_failed_feature,
            &expected_failure,
        );
    }
    assert_eq!(
        replayed.coordinator.feature_document().to_json().unwrap(),
        final_feature_json
    );
    assert_signature_close(
        assert_current_with_unrelated_failure(
            &replayed.coordinator,
            replayed.owner,
            replayed.branch,
            failed_feature,
            &expected_failed_feature,
            &expected_failure,
        ),
        final_signature,
        "mixed transcript replay",
    );

    let (mut restored, restored_failed) = exact_fixture_with_unrelated_failed_feature();
    assert_eq!(restored_failed, failed_feature);
    restored
        .coordinator
        .reload(&checkpoint)
        .expect("mixed checkpoint restore");
    assert_eq!(
        persistent_feature_semantics(&restored.coordinator, restored.owner),
        final_current_semantics,
        "restore may rebase lifecycle revisions but not current feature semantics"
    );
    assert_signature_close(
        assert_current_with_unrelated_failure(
            &restored.coordinator,
            restored.owner,
            restored.branch,
            failed_feature,
            &expected_failed_feature,
            &expected_failure,
        ),
        final_signature,
        "mixed checkpoint restore",
    );
}

#[test]
fn m70b_f005_only_exact_current_computed_input_has_affordance_or_mutation_authority() {
    let mut fixture = exact_fixture();
    let action_items = [SelectionItem::FeatureCorner(fixture.owner)];
    let mut current_scene = visible_scene(&fixture.coordinator);
    fixture
        .coordinator
        .populate_computed_fillet_affordances(&mut current_scene, &action_items, 0.5)
        .expect("exact current scene has affordance authority");

    let stale_scene = current_scene;
    let original = fixture
        .coordinator
        .session()
        .design_document()
        .point(fixture.line_start)
        .expect("payload line start")
        .position;
    commit_position(
        &mut fixture.coordinator,
        fixture.line_start,
        [original[0], original[1] - 0.75],
    );
    let before_stale_scene = coordinator_fingerprint(&fixture.coordinator);
    let mut stale_scene = stale_scene;
    assert!(matches!(
        fixture.coordinator.populate_computed_fillet_affordances(
            &mut stale_scene,
            &action_items,
            0.5,
        ),
        Err(CoordinatorError::StaleComputedFeatureCandidate)
    ));
    assert_eq!(
        coordinator_fingerprint(&fixture.coordinator),
        before_stale_scene
    );

    let origin = fixture.coordinator.computed_evaluation_input().unwrap();
    let preview_input = fixture
        .coordinator
        .preview_computed_fillet_radius_exact(origin, fixture.owner.feature, 0.9)
        .expect("authentic radius preview")
        .input();
    let mut detached_preview_scene = visible_scene(&fixture.coordinator);
    fixture.coordinator.clear_computed_feature_preview();
    let before_detached_preview = coordinator_fingerprint(&fixture.coordinator);
    assert!(matches!(
        fixture.coordinator.populate_computed_fillet_affordances(
            &mut detached_preview_scene,
            &action_items,
            0.5,
        ),
        Err(CoordinatorError::StaleComputedFeatureCandidate)
    ));
    assert!(matches!(
        fixture.coordinator.set_computed_fillet_radius_exact(
            preview_input,
            fixture.owner.feature,
            0.9,
        ),
        Err(CoordinatorError::StaleComputedFeatureCandidate)
    ));
    assert_eq!(
        coordinator_fingerprint(&fixture.coordinator),
        before_detached_preview
    );

    let mut recovered_current = visible_scene(&fixture.coordinator);
    fixture
        .coordinator
        .populate_computed_fillet_affordances(&mut recovered_current, &action_items, 0.5)
        .expect("current scene regains authority after preview clear");
}
