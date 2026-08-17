// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

use geosolve_constraint_editor::{
    ConstructionCommitPlan, ConstructionPoint, ConstructionRelationDefinition,
    DraftCurveBranchCandidate, DraftCurveContact, DraftGuideClassification, DraftGuideGeometry,
    DraftInferenceBehavior, DraftInferenceCandidate, DraftInferenceCompleteness,
    DraftInferenceEngine, DraftInferenceFrame, DraftInferenceInput, DraftInferencePolicy,
    DraftInferenceRelation, DraftInferenceResolution, DraftInferenceSample, DraftInferenceStatus,
    DraftInferenceSubject, DraftPointSlot, DraftReferenceAnchor, DraftReferenceOrigin,
    DraftSpanSlot, EditorEffect, EditorMutation, EditorScene, EditorTool,
    GeometryInteractionPolicy, InferredRelation, Modifiers, PointerInput,
    RetainedEditorCoordinator, ScenePointRoleIncidence, ScreenPoint, Viewport,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DocumentConstraintDefinition, DocumentCoordinateAxis, DocumentId, DocumentSolveRequest,
    GeometryRole, PersistentId, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const GOLDEN_TRANSCRIPT: &[u8] = include_bytes!("fixtures/m70_transition_parity.golden.txt");

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn deterministic_fixture() -> (
    RetainedSketchDocumentSession,
    EditorScene,
    [geosolve_sketch::DesignPointId; 2],
    CurveSpan,
) {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7000_0070_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("deterministic document");
    let start = document
        .add_point("reference start", [-4.0, 1.0])
        .expect("reference start");
    let end = document
        .add_point("reference end", [4.0, 1.0])
        .expect("reference end");
    let reference = document
        .add_curve(
            "reference line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("reference line");
    for (label, point, target) in [
        ("fix reference start", start, [-4.0, 1.0]),
        ("fix reference end", end, [4.0, 1.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .expect("fixed reference endpoint");
    }
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained fixture");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted fixture");
    let scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([1_000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
        0.5,
    )
    .expect("scene")
    .with_retained_session(&session)
    .expect("bound scene");
    (session, scene, [start, end], CurveSpan::line(reference))
}

fn bits(value: f64) -> u64 {
    value.to_bits()
}

fn point_anchor(point: DesignPointId, position: [f64; 2]) -> DraftReferenceAnchor {
    DraftReferenceAnchor::PersistentPoint {
        point,
        model_position: position,
        role_incidence: ScenePointRoleIncidence {
            profile: true,
            construction: false,
        },
    }
}

fn midpoint_anchor(span: CurveSpan, position: [f64; 2]) -> DraftReferenceAnchor {
    DraftReferenceAnchor::Midpoint {
        span,
        model_position: position,
        affine_direction: [1.0, 0.0],
        role: GeometryRole::Profile,
        source_role: GeometryRole::Profile,
        origin: DraftReferenceOrigin::Native,
    }
}

fn curve_anchor(
    span: CurveSpan,
    position: [f64; 2],
    parameter: f64,
    branch: u32,
) -> DraftReferenceAnchor {
    DraftReferenceAnchor::CurvePoint {
        contact: DraftCurveContact {
            span,
            domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            parameter,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
        },
        branch_candidate: DraftCurveBranchCandidate::from_ordinal(branch),
        model_position: position,
        role: GeometryRole::Profile,
        source_role: GeometryRole::Profile,
        origin: DraftReferenceOrigin::Native,
    }
}

fn affine_anchor(span: CurveSpan, position: [f64; 2]) -> DraftReferenceAnchor {
    DraftReferenceAnchor::AffineSupport {
        contact: DraftCurveContact {
            span,
            domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            parameter: 0.5,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
        },
        model_position: position,
        affine_direction: [1.0, 0.0],
        role: GeometryRole::Profile,
        source_role: GeometryRole::Profile,
        origin: DraftReferenceOrigin::Native,
    }
}

fn inference_frame(
    scene: &EditorScene,
    sample: [f64; 2],
    span_start: Option<[f64; 2]>,
    anchors: Vec<DraftReferenceAnchor>,
) -> DraftInferenceFrame {
    inference_frame_for_subject(
        scene,
        sample,
        DraftInferenceSubject::PointOperand,
        span_start,
        anchors,
    )
}

fn inference_frame_for_subject(
    scene: &EditorScene,
    sample: [f64; 2],
    subject: DraftInferenceSubject,
    span_start: Option<[f64; 2]>,
    anchors: Vec<DraftReferenceAnchor>,
) -> DraftInferenceFrame {
    // Keep the byte-frozen M70 transcript scoped to the M70 candidate language.
    // M74 owns intrinsic-datum parity separately.
    let policy = GeometryInteractionPolicy {
        visibility: geosolve_constraint_editor::GeometryVisibility {
            reference_geometry: false,
            ..geosolve_constraint_editor::GeometryVisibility::default()
        },
        ..GeometryInteractionPolicy::default()
    };
    DraftInferenceFrame::from_scene(
        scene,
        policy,
        DraftInferenceSample {
            raw_screen_position: scene.viewport.model_to_screen(sample),
            subject,
            span_start,
        },
        anchors,
    )
}

fn m70_inference_engine() -> DraftInferenceEngine {
    // This byte-frozen fixture records M70, where remembered-point alignment
    // was deliberately visual tracking only. M71 has its own parity coverage
    // for the newly durable point-pair relation.
    let policy = DraftInferencePolicy {
        point_tracking: DraftInferenceBehavior::tracking_only(),
        ..DraftInferencePolicy::default()
    };
    DraftInferenceEngine::new(policy).expect("valid M70 inference policy")
}

fn push_span(transcript: &mut String, span: CurveSpan) {
    write!(transcript, "{}:{}", span.curve, span.segment).expect("string write");
}

fn push_contact(transcript: &mut String, contact: DraftCurveContact) {
    push_span(transcript, contact.span);
    match contact.domain {
        ContactDomain::SupportingLine => transcript.push_str("/line"),
        ContactDomain::Bounded { lower, upper } => {
            write!(
                transcript,
                "/bounded:{:016x}:{:016x}",
                bits(lower),
                bits(upper)
            )
            .expect("string write");
        }
        ContactDomain::Periodic { period } => {
            write!(transcript, "/periodic:{:016x}", bits(period)).expect("string write");
        }
    }
    write!(
        transcript,
        "/p:{:016x}/w:{}",
        bits(contact.parameter),
        contact.winding
    )
    .expect("string write");
    match contact.neighborhood {
        ContactNeighborhood::Interior => transcript.push_str("/interior"),
        ContactNeighborhood::Local { lower, upper } => {
            write!(
                transcript,
                "/local:{:016x}:{:016x}",
                bits(lower),
                bits(upper)
            )
            .expect("string write");
        }
        ContactNeighborhood::Start => transcript.push_str("/start"),
        ContactNeighborhood::End => transcript.push_str("/end"),
    }
}

fn push_relation(transcript: &mut String, relation: DraftInferenceRelation) {
    match relation {
        DraftInferenceRelation::CoincidentWithOrigin => transcript.push_str("datum-origin"),
        DraftInferenceRelation::PointOnDatumAxis { axis } => match axis {
            DocumentCoordinateAxis::X => transcript.push_str("datum-x-axis"),
            DocumentCoordinateAxis::Y => transcript.push_str("datum-y-axis"),
        },
        DraftInferenceRelation::PointIdentity { point } => {
            write!(transcript, "point:{point}").expect("string write");
        }
        DraftInferenceRelation::PointOnCurve { contact } => {
            transcript.push_str("curve:");
            push_contact(transcript, contact);
        }
        DraftInferenceRelation::PointOnCreatedCurve { point } => {
            write!(transcript, "created-curve:{point}").expect("string write");
        }
        DraftInferenceRelation::Midpoint { span } => {
            transcript.push_str("mid:");
            push_span(transcript, span);
        }
        DraftInferenceRelation::Horizontal => transcript.push_str("horizontal"),
        DraftInferenceRelation::Vertical => transcript.push_str("vertical"),
        DraftInferenceRelation::Parallel { reference } => {
            transcript.push_str("parallel:");
            push_span(transcript, reference);
        }
        DraftInferenceRelation::Perpendicular { reference } => {
            transcript.push_str("perpendicular:");
            push_span(transcript, reference);
        }
        DraftInferenceRelation::HorizontalPoints { reference } => {
            let _ = write!(transcript, "horizontal-points({reference})");
        }
        DraftInferenceRelation::VerticalPoints { reference } => {
            let _ = write!(transcript, "vertical-points({reference})");
        }
        DraftInferenceRelation::HorizontalPointToMidpoint { reference } => {
            transcript.push_str("horizontal-to-midpoint:");
            push_span(transcript, reference);
        }
        DraftInferenceRelation::VerticalPointToMidpoint { reference } => {
            transcript.push_str("vertical-to-midpoint:");
            push_span(transcript, reference);
        }
        DraftInferenceRelation::Concentric {
            reference,
            prospective_curve_index,
        } => {
            let _ = write!(
                transcript,
                "concentric({reference},created:{prospective_curve_index})"
            );
        }
        DraftInferenceRelation::Collinear { reference } => {
            transcript.push_str("collinear:");
            push_span(transcript, reference);
        }
    }
}

fn push_candidate(transcript: &mut String, candidate: &DraftInferenceCandidate) {
    write!(
        transcript,
        "{}@{:016x},{:016x}>{:016x},{:016x}/",
        candidate.id.get(),
        bits(candidate.raw_model_position[0]),
        bits(candidate.raw_model_position[1]),
        bits(candidate.adjusted_model_position[0]),
        bits(candidate.adjusted_model_position[1]),
    )
    .expect("string write");
    for (index, relation) in candidate.relations.iter().copied().enumerate() {
        if index > 0 {
            transcript.push('+');
        }
        push_relation(transcript, relation);
    }
    write!(
        transcript,
        "/rank:{}:{}:{}:{:016x}:{:016x}/refs:{}/guides:{}",
        u8::from(candidate.ranking.constraint_backed),
        candidate.ranking.persistent_relation_count,
        candidate.ranking.positional_geometry_role_priority,
        bits(candidate.ranking.distance_pixels),
        bits(candidate.ranking.angular_error_radians),
        candidate.references.len(),
        candidate.guides.len(),
    )
    .expect("string write");
}

fn push_resolution(transcript: &mut String, label: &str, resolution: &DraftInferenceResolution) {
    write!(transcript, "{label}=").expect("string write");
    match &resolution.status {
        DraftInferenceStatus::None => transcript.push_str("none"),
        DraftInferenceStatus::Resolved { candidate } => {
            write!(transcript, "resolved:{}", candidate.get()).expect("string write");
        }
        DraftInferenceStatus::Ambiguous { candidates } => {
            transcript.push_str("ambiguous:");
            for (index, candidate) in candidates.iter().enumerate() {
                if index > 0 {
                    transcript.push(',');
                }
                write!(transcript, "{}", candidate.get()).expect("string write");
            }
        }
        DraftInferenceStatus::Suppressed => transcript.push_str("suppressed"),
        DraftInferenceStatus::ResourceLimited => transcript.push_str("limited"),
        DraftInferenceStatus::StalePreferredCandidate { preferred } => {
            write!(transcript, "stale:{}", preferred.get()).expect("string write");
        }
    }
    match resolution.completeness {
        DraftInferenceCompleteness::Complete => transcript.push_str("/complete"),
        DraftInferenceCompleteness::CandidateLimit { required, limit } => {
            write!(transcript, "/candidate-limit:{required}:{limit}").expect("string write");
        }
        DraftInferenceCompleteness::SceneLimit(limit) => {
            write!(
                transcript,
                "/scene-limit:{}:{}",
                limit.required, limit.limit
            )
            .expect("string write");
        }
    }
    write!(
        transcript,
        "/raw:{:016x},{:016x}/adjusted:{:016x},{:016x}/candidates:",
        bits(resolution.raw_model_position[0]),
        bits(resolution.raw_model_position[1]),
        bits(resolution.adjusted_model_position[0]),
        bits(resolution.adjusted_model_position[1]),
    )
    .expect("string write");
    for (index, candidate) in resolution.candidates.iter().enumerate() {
        if index > 0 {
            transcript.push('|');
        }
        push_candidate(transcript, candidate);
    }
    write!(transcript, "/guides:{}", resolution.guides.len()).expect("string write");
    for guide in &resolution.guides {
        transcript.push('/');
        transcript.push_str(match guide.classification {
            DraftGuideClassification::TrackingOnly => "tracking",
            DraftGuideClassification::ConstraintBacked => "constraint",
        });
        match guide.geometry {
            DraftGuideGeometry::Point { position } => {
                write!(
                    transcript,
                    ":point:{:016x},{:016x}",
                    bits(position[0]),
                    bits(position[1])
                )
                .expect("string write");
            }
            DraftGuideGeometry::Segment { start, end } => {
                write!(
                    transcript,
                    ":segment:{:016x},{:016x}>{:016x},{:016x}",
                    bits(start[0]),
                    bits(start[1]),
                    bits(end[0]),
                    bits(end[1])
                )
                .expect("string write");
            }
        }
    }
    transcript.push('\n');
}

#[allow(
    clippy::too_many_lines,
    reason = "one shared native/WASM transcript intentionally covers every M70 transition family"
)]
fn inference_transition_transcript(
    scene: &EditorScene,
    reference_points: [DesignPointId; 2],
    reference: CurveSpan,
) -> String {
    let mut transcript = String::new();
    let resolve = |engine: &mut DraftInferenceEngine, sample, span_start, anchors, input| {
        engine
            .resolve(&inference_frame(scene, sample, span_start, anchors), input)
            .expect("deterministic inference transition")
    };

    let mut engine = m70_inference_engine();
    let identity = resolve(
        &mut engine,
        [-4.0, 1.0],
        None,
        vec![point_anchor(reference_points[0], [-4.0, 1.0])],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "identity", &identity);

    let mut engine = m70_inference_engine();
    let circle_through_point = engine
        .resolve(
            &inference_frame_for_subject(
                scene,
                [-4.0, 1.0],
                DraftInferenceSubject::CircleCircumference,
                None,
                vec![point_anchor(reference_points[0], [-4.0, 1.0])],
            ),
            DraftInferenceInput::default(),
        )
        .expect("deterministic circle-through-point transition");
    push_resolution(
        &mut transcript,
        "circle-through-point",
        &circle_through_point,
    );

    let mut engine = m70_inference_engine();
    let curve = resolve(
        &mut engine,
        [2.0, 1.0],
        None,
        vec![curve_anchor(reference, [2.0, 1.0], 0.75, 0)],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "curve", &curve);

    let mut engine = m70_inference_engine();
    let midpoint = resolve(
        &mut engine,
        [0.0, 1.0],
        None,
        vec![midpoint_anchor(reference, [0.0, 1.0])],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "midpoint", &midpoint);

    let mut engine = m70_inference_engine();
    let horizontal = resolve(
        &mut engine,
        [2.0, 0.05],
        Some([0.0, 0.0]),
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "horizontal", &horizontal);

    let mut engine = m70_inference_engine();
    let vertical = resolve(
        &mut engine,
        [0.05, 2.0],
        Some([0.0, 0.0]),
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "vertical", &vertical);

    let reference_anchor = affine_anchor(reference, [0.0, 1.0]);
    let mut engine = m70_inference_engine();
    engine
        .remember_reference(reference_anchor)
        .expect("parallel reference");
    let parallel = resolve(
        &mut engine,
        [2.0, 0.05],
        Some([0.0, 0.0]),
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "parallel", &parallel);

    let mut engine = m70_inference_engine();
    engine
        .remember_reference(reference_anchor)
        .expect("perpendicular reference");
    let perpendicular = resolve(
        &mut engine,
        [0.05, 2.0],
        Some([0.0, 0.0]),
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "perpendicular", &perpendicular);

    let mut engine = m70_inference_engine();
    engine
        .remember_reference(point_anchor(reference_points[0], [-4.0, 1.0]))
        .expect("tracking reference");
    let tracking = resolve(
        &mut engine,
        [-1.0, 1.05],
        None,
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "tracking", &tracking);

    let ambiguous_span = CurveSpan {
        curve: CurveId(PersistentId::from_u128(
            0x7000_0070_0000_0000_0000_0000_0000_00ff,
        )),
        segment: 0,
    };
    let mut engine = m70_inference_engine();
    let ambiguity = resolve(
        &mut engine,
        [2.0, 1.0],
        None,
        vec![
            curve_anchor(reference, [2.0, 1.0], 0.75, 0),
            curve_anchor(ambiguous_span, [2.0, 1.0], 0.75, 0),
        ],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "ambiguity", &ambiguity);

    let mut engine = m70_inference_engine();
    let wake = resolve(
        &mut engine,
        [-4.0, 1.0],
        None,
        vec![point_anchor(reference_points[0], [-4.0, 1.0])],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "wake", &wake);
    let suppressed = resolve(
        &mut engine,
        [-4.0, 1.0],
        None,
        vec![point_anchor(reference_points[0], [-4.0, 1.0])],
        DraftInferenceInput {
            suppressed: true,
            preferred_candidate: None,
        },
    );
    push_resolution(&mut transcript, "suppressed", &suppressed);
    let released = resolve(
        &mut engine,
        [-4.0, 1.0],
        None,
        vec![point_anchor(reference_points[0], [-4.0, 1.0])],
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "released", &released);
    let stale = resolve(
        &mut engine,
        [2.0, 1.0],
        None,
        vec![curve_anchor(reference, [2.0, 1.0], 0.75, 0)],
        DraftInferenceInput {
            suppressed: false,
            preferred_candidate: match wake.status {
                DraftInferenceStatus::Resolved { candidate } => Some(candidate),
                _ => panic!("wake candidate"),
            },
        },
    );
    push_resolution(&mut transcript, "stale", &stale);
    engine.clear_stage();
    let cleared = resolve(
        &mut engine,
        [-1.0, 1.05],
        None,
        Vec::new(),
        DraftInferenceInput::default(),
    );
    push_resolution(&mut transcript, "cleared", &cleared);

    transcript
}

#[allow(
    clippy::too_many_lines,
    reason = "one parity oracle keeps atomic publication, rejection, history and reload contiguous"
)]
fn transition_transcript() -> Vec<u8> {
    let (session, scene, reference_points, reference) = deterministic_fixture();
    let mut transcript = inference_transition_transcript(&scene, reference_points, reference);
    let reload_session = session.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let baseline_json = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("baseline canonical document");
    coordinator.editor_mut().activate_tool(EditorTool::Line);

    let midpoint = scene.viewport.model_to_screen([0.0, 1.0]);
    let first = coordinator.pointer_down(&scene, pointer(70, midpoint));
    assert!(first.iter().all(|effect| !matches!(
        effect,
        EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
    )));
    let near_normal = scene.viewport.model_to_screen([0.05, 4.0]);
    let second = coordinator.pointer_down(&scene, pointer(70, near_normal));
    let (token, plan, commit) = second
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { token, plan, .. } => {
                Some((*token, plan, effect))
            }
            _ => None,
        })
        .expect("midpoint-normal plan");
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [
            InferredRelation::Midpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Existing(midpoint_line),
            },
            InferredRelation::Perpendicular {
                first: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
                second: DraftSpanSlot::Existing(normal_line),
            },
        ] if *midpoint_line == reference && *normal_line == reference
    ));

    let committed = coordinator
        .apply_editor_effect(commit)
        .expect("atomic publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = committed.value else {
        panic!("expected inferred construction");
    };
    assert!(
        coordinator
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    write!(
        transcript,
        "preview=midpoint+perpendicular;commit=points:{},curves:{},contacts:{},constraints:{},history:{},cursor:{};",
        result.construction.points.len(),
        result.construction.curves.len(),
        result.contacts.len(),
        result.constraints.len(),
        coordinator.history_len(),
        coordinator.history_cursor(),
    )
    .expect("string write");
    let committed_json = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("committed canonical document");
    writeln!(transcript, "document={committed_json}").expect("string write");
    let checkpoint = coordinator
        .persistence_checkpoint()
        .expect("published checkpoint");

    coordinator.undo().expect("atomic undo");
    let document = coordinator.session().design_document();
    write!(
        transcript,
        "undo=points:{},curves:{},constraints:{},history:{},cursor:{},state_exact:{};",
        document.points().len(),
        document.curves().len(),
        document.constraints().len(),
        coordinator.history_len(),
        coordinator.history_cursor(),
        document
            .to_draft_v5_json()
            .expect("undo canonical document")
            == baseline_json,
    )
    .expect("string write");
    coordinator.redo().expect("atomic redo");
    let document = coordinator.session().design_document();
    write!(
        transcript,
        "redo=points:{},curves:{},constraints:{},history:{},cursor:{},state_exact:{};",
        document.points().len(),
        document.curves().len(),
        document.constraints().len(),
        coordinator.history_len(),
        coordinator.history_cursor(),
        document
            .to_draft_v5_json()
            .expect("redo canonical document")
            == committed_json,
    )
    .expect("string write");

    let mut reloaded = RetainedEditorCoordinator::new(reload_session).expect("reload coordinator");
    reloaded.reload(&checkpoint).expect("atomic reload");
    let document = reloaded.session().design_document();
    write!(
        transcript,
        "reload=points:{},curves:{},constraints:{},history:{},cursor:{},state_exact:{};",
        document.points().len(),
        document.curves().len(),
        document.constraints().len(),
        reloaded.history_len(),
        reloaded.history_cursor(),
        document
            .to_draft_v5_json()
            .expect("reload canonical document")
            == committed_json,
    )
    .expect("string write");

    let before_design = reloaded.session().design_identity();
    let before_history = (reloaded.history_len(), reloaded.history_cursor());
    let created = DraftSpanSlot::Created {
        curve_index: 0,
        segment: 0,
    };
    let redundant = ConstructionCommitPlan {
        proposal: geosolve_constraint_editor::ConstructionProposal::Line {
            start: ConstructionPoint::New([0.0, 5.0]),
            end: ConstructionPoint::New([2.0, 5.0]),
        },
        curve_roles: vec![GeometryRole::Profile],
        relations: vec![
            ConstructionRelationDefinition::auto_inference(InferredRelation::Horizontal {
                line: created,
            }),
            ConstructionRelationDefinition::auto_inference(InferredRelation::Parallel {
                first: created,
                second: DraftSpanSlot::Existing(reference),
            }),
        ],
    };
    let expected = reloaded
        .session()
        .accepted_prepared_input()
        .expect("reloaded prepared input");
    let rejection = reloaded.apply_construction_plan(&expected, &redundant);
    assert!(
        matches!(
            &rejection,
            Err(geosolve_constraint_editor::CoordinatorError::RedundantInferredConstruction { .. })
        ),
        "unexpected rejection result: {rejection:?}"
    );
    let unchanged = reloaded.session().design_identity() == before_design
        && (reloaded.history_len(), reloaded.history_cursor()) == before_history;
    writeln!(transcript, "reject=redundant,state_unchanged:{unchanged}").expect("string write");
    transcript.into_bytes()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn m70_transition_oracle_matches_the_cross_target_golden_bytes() {
    let transcript = transition_transcript();
    assert_eq!(
        transcript,
        GOLDEN_TRANSCRIPT,
        "{}",
        String::from_utf8_lossy(&transcript)
    );
}
