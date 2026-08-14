// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

use geosolve_constraint_editor::{
    AuthoringMutation, AuthoringOperand, AuthoringOutcome, AuthoringState, AuthoringTool,
    ConstraintIntent, ConstructionCommitPlan, ConstructionPoint, DraftCurveSlot,
    DraftInferenceEngine, DraftInferenceFrame, DraftInferenceInput, DraftInferenceLimits,
    DraftInferenceRelation, DraftInferenceSample, DraftInferenceSceneInputCollection,
    DraftInferenceStatus, DraftInferenceSubject, DraftLineSupportSlot, DraftPointSlot,
    DraftReferenceAnchor, DraftReferenceOrigin, DraftSpanSlot, EditorScene,
    GeometryInteractionPolicy, InferredRelation, ResolvedConstraintKind, RetainedEditorCoordinator,
    ScenePointRoleIncidence, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition,
    DocumentDirectionSense, DocumentId, DocumentSolveRequest, GeometryRole, PersistentId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const GOLDEN_TRANSCRIPT: &[u8] = include_bytes!("fixtures/m71_transition_parity.golden.txt");

struct Fixture {
    session: RetainedSketchDocumentSession,
    scene: EditorScene,
    points: [DesignPointId; 6],
    lines: [CurveSpan; 2],
    circles: [CurveSpan; 2],
}

fn fixture() -> Fixture {
    let mut document = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(
            0x7100_0071_0000_0000_0000_0000_0000_0001,
        )),
    )
    .expect("document");
    let points = [
        [-4.0, 0.0],
        [-2.0, 0.0],
        [2.0, 0.0],
        [4.0, 0.0],
        [-3.0, 3.0],
        [3.0, 3.0],
    ]
    .map(|position| document.add_point("M71 point", position).expect("point"));
    let lines = [(points[0], points[1]), (points[2], points[3])].map(|(start, end)| {
        CurveSpan::line(
            document
                .add_curve(
                    "M71 line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        )
    });
    let circles = [(points[4], 1.0), (points[5], 2.0)].map(|(center, value)| {
        let radius = document
            .add_scalar(
                "M71 radius",
                value,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("radius");
        CurveSpan::line(
            document
                .add_curve("M71 circle", CurveDefinition::Circle { center, radius })
                .expect("circle"),
        )
    });
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted state");
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
    .expect("authenticated scene");
    Fixture {
        session,
        scene,
        points,
        lines,
        circles,
    }
}

fn point_anchor(point: DesignPointId, model_position: [f64; 2]) -> DraftReferenceAnchor {
    DraftReferenceAnchor::PersistentPoint {
        point,
        model_position,
        role_incidence: ScenePointRoleIncidence {
            profile: true,
            construction: false,
        },
    }
}

fn affine_anchor(span: CurveSpan, model_position: [f64; 2]) -> DraftReferenceAnchor {
    use geosolve_constraint_editor::{DraftCurveContact, DraftReferenceAnchor};
    use geosolve_sketch::{ContactDomain, ContactNeighborhood};

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
        model_position,
        affine_direction: [1.0, 0.0],
        role: GeometryRole::Profile,
        source_role: GeometryRole::Profile,
        origin: DraftReferenceOrigin::Native,
    }
}

fn midpoint_anchor(span: CurveSpan, model_position: [f64; 2]) -> DraftReferenceAnchor {
    DraftReferenceAnchor::Midpoint {
        span,
        model_position,
        affine_direction: [1.0, 0.0],
        role: GeometryRole::Profile,
        source_role: GeometryRole::Profile,
        origin: DraftReferenceOrigin::Native,
    }
}

fn push_inference(transcript: &mut String, label: &str, relation: DraftInferenceRelation) {
    write!(transcript, "{label}=").expect("string write");
    push_inference_relation(transcript, relation);
    transcript.push('\n');
}

fn push_inference_bundle(
    transcript: &mut String,
    label: &str,
    relations: &[DraftInferenceRelation],
) {
    write!(transcript, "{label}=").expect("string write");
    for (index, relation) in relations.iter().copied().enumerate() {
        if index > 0 {
            transcript.push('+');
        }
        push_inference_relation(transcript, relation);
    }
    transcript.push('\n');
}

fn push_inference_relation(transcript: &mut String, relation: DraftInferenceRelation) {
    match relation {
        DraftInferenceRelation::HorizontalPoints { reference } => {
            write!(transcript, "horizontal-points:{reference}").expect("string write");
        }
        DraftInferenceRelation::VerticalPoints { reference } => {
            write!(transcript, "vertical-points:{reference}").expect("string write");
        }
        DraftInferenceRelation::HorizontalPointToMidpoint { reference } => {
            write!(
                transcript,
                "horizontal-to-midpoint:{}:{}",
                reference.curve, reference.segment
            )
            .expect("string write");
        }
        DraftInferenceRelation::VerticalPointToMidpoint { reference } => {
            write!(
                transcript,
                "vertical-to-midpoint:{}:{}",
                reference.curve, reference.segment
            )
            .expect("string write");
        }
        DraftInferenceRelation::Collinear { reference } => {
            write!(
                transcript,
                "collinear:{}:{}",
                reference.curve, reference.segment
            )
            .expect("string write");
        }
        DraftInferenceRelation::Concentric {
            reference,
            prospective_curve_index,
        } => {
            write!(
                transcript,
                "concentric:{reference}:created:{prospective_curve_index}"
            )
            .expect("string write");
        }
        DraftInferenceRelation::Horizontal => transcript.push_str("horizontal"),
        DraftInferenceRelation::Vertical => transcript.push_str("vertical"),
        other => panic!("unexpected M71 inference relation: {other:?}"),
    }
}

fn resolve_relation(
    scene: &EditorScene,
    engine: &mut DraftInferenceEngine,
    sample: [f64; 2],
    subject: DraftInferenceSubject,
    span_start: Option<[f64; 2]>,
    anchors: Vec<DraftReferenceAnchor>,
) -> DraftInferenceRelation {
    resolve_relations(scene, engine, sample, subject, span_start, anchors)
        .into_iter()
        .next()
        .expect("resolved relation")
}

fn resolve_relations(
    scene: &EditorScene,
    engine: &mut DraftInferenceEngine,
    sample: [f64; 2],
    subject: DraftInferenceSubject,
    span_start: Option<[f64; 2]>,
    anchors: Vec<DraftReferenceAnchor>,
) -> Vec<DraftInferenceRelation> {
    let scene_inputs = scene.draft_inference_scene_inputs(
        scene.viewport.model_to_screen(sample),
        subject,
        DraftInferenceLimits::default(),
    );
    let scene_inputs = match scene_inputs {
        DraftInferenceSceneInputCollection::Complete(inputs) => inputs,
        DraftInferenceSceneInputCollection::ResourceLimited(evidence) => {
            panic!("M71 fixture exceeded scene resources: {evidence:?}")
        }
    };
    let anchors = if anchors.is_empty() {
        scene_inputs.anchors
    } else {
        anchors
    };
    let resolution = engine
        .resolve(
            &DraftInferenceFrame::from_scene_with_semantic_centers(
                scene,
                GeometryInteractionPolicy::default(),
                DraftInferenceSample {
                    raw_screen_position: scene.viewport.model_to_screen(sample),
                    subject,
                    span_start,
                },
                anchors,
                scene_inputs.semantic_centers,
            ),
            DraftInferenceInput::default(),
        )
        .expect("inference resolution");
    let DraftInferenceStatus::Resolved { candidate } = resolution.status else {
        panic!("M71 candidate did not resolve: {resolution:?}");
    };
    resolution
        .candidates
        .iter()
        .find(|value| value.id == candidate)
        .expect("resolved candidate")
        .relations
        .clone()
}

fn apply_authoring(
    coordinator: &mut RetainedEditorCoordinator,
    intent: ConstraintIntent,
    expected: ResolvedConstraintKind,
    operands: &[SelectionItem],
) -> geosolve_sketch::DocumentConstraintId {
    let mut authoring = AuthoringState::default();
    let operands = operands
        .iter()
        .copied()
        .map(AuthoringOperand::selected)
        .collect::<Vec<_>>();
    let AuthoringOutcome::Apply(application) = authoring.activate(
        coordinator.session().design_document(),
        AuthoringTool::Constraint(intent),
        &operands,
    ) else {
        panic!("{intent:?} did not produce an application");
    };
    assert_eq!(application.resolved_constraint, Some(expected));
    let AuthoringMutation::Constraint(outcome) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .expect("authoring publication")
    else {
        panic!("constraint mutation expected");
    };
    assert!(outcome.published_accepted.is_some());
    outcome.value
}

#[allow(
    clippy::too_many_lines,
    reason = "one compact native/WASM transcript keeps M71 inference, authoring and retained reload contiguous"
)]
fn transition_transcript() -> Vec<u8> {
    let Fixture {
        session,
        scene,
        points,
        lines,
        circles,
    } = fixture();
    let mut transcript = String::new();

    let mut engine = DraftInferenceEngine::default();
    engine
        .remember_reference(point_anchor(points[0], [-4.0, 0.0]))
        .expect("remember point");
    push_inference(
        &mut transcript,
        "remembered-horizontal",
        resolve_relation(
            &scene,
            &mut engine,
            [-1.0, 0.05],
            DraftInferenceSubject::PointOperand,
            None,
            Vec::new(),
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    engine
        .remember_reference(point_anchor(points[0], [-4.0, 0.0]))
        .expect("remember point");
    push_inference(
        &mut transcript,
        "remembered-vertical",
        resolve_relation(
            &scene,
            &mut engine,
            [-3.95, 2.0],
            DraftInferenceSubject::PointOperand,
            None,
            Vec::new(),
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    let axis_reference = point_anchor(points[0], [-4.0, 0.0]);
    engine
        .remember_reference(axis_reference)
        .expect("remember bundled point axis");
    push_inference_bundle(
        &mut transcript,
        "remembered-axis-bundle",
        &resolve_relations(
            &scene,
            &mut engine,
            [0.04, 0.05],
            DraftInferenceSubject::PointOperand,
            Some([0.0, -4.0]),
            vec![axis_reference],
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    let midpoint = midpoint_anchor(lines[0], [-3.0, 0.0]);
    engine
        .remember_reference(midpoint)
        .expect("remember midpoint");
    push_inference(
        &mut transcript,
        "remembered-midpoint-horizontal",
        resolve_relation(
            &scene,
            &mut engine,
            [0.0, 0.05],
            DraftInferenceSubject::PointOperand,
            None,
            vec![midpoint],
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    let midpoint = midpoint_anchor(lines[0], [-3.0, 0.0]);
    engine
        .remember_reference(midpoint)
        .expect("remember midpoint");
    push_inference(
        &mut transcript,
        "remembered-midpoint-vertical",
        resolve_relation(
            &scene,
            &mut engine,
            [-2.95, 2.0],
            DraftInferenceSubject::PointOperand,
            None,
            vec![midpoint],
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    engine
        .remember_reference(affine_anchor(lines[0], [-3.0, 0.0]))
        .expect("remember support");
    push_inference(
        &mut transcript,
        "remembered-collinear",
        resolve_relation(
            &scene,
            &mut engine,
            [1.0, 0.05],
            DraftInferenceSubject::PointOperand,
            Some([-1.0, 0.0]),
            Vec::new(),
        ),
    );

    let mut engine = DraftInferenceEngine::default();
    push_inference(
        &mut transcript,
        "accepted-center",
        resolve_relation(
            &scene,
            &mut engine,
            [-2.95, 3.0],
            DraftInferenceSubject::CenteredPointOperand {
                prospective_curve_index: 0,
            },
            None,
            Vec::new(),
        ),
    );

    let baseline = session
        .design_document()
        .to_draft_v5_json()
        .expect("baseline draft v5");
    let reload_session = session.clone();
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let concentric = apply_authoring(
        &mut coordinator,
        ConstraintIntent::Concentric,
        ResolvedConstraintKind::ConcentricCurves,
        &circles.map(SelectionItem::Curve),
    );
    let collinear = apply_authoring(
        &mut coordinator,
        ConstraintIntent::Collinear,
        ResolvedConstraintKind::CollinearSupports,
        &lines.map(SelectionItem::Curve),
    );
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(concentric)
            .expect("concentric")
            .definition,
        DocumentConstraintDefinition::Concentric { .. }
    ));
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(collinear)
            .expect("collinear")
            .definition,
        DocumentConstraintDefinition::Collinear { .. }
    ));
    let presentation_scene = {
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted presentation state");
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            scene.viewport,
            0.5,
        )
        .expect("presentation scene")
    };
    for id in [concentric, collinear] {
        let entry = presentation_scene
            .constraint_entries
            .iter()
            .find(|entry| entry.id == id)
            .expect("M71 constraint entry");
        writeln!(
            transcript,
            "entry={}:{}:{:?}:operands:{}:suppressed:{}",
            entry.id,
            entry.label,
            entry.glyph,
            entry.operands.len(),
            entry.suppressed
        )
        .expect("string write");
    }
    writeln!(
        transcript,
        "explicit=concentric:{concentric},collinear:{collinear},history:{},cursor:{}",
        coordinator.history_len(),
        coordinator.history_cursor()
    )
    .expect("string write");

    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let plan = ConstructionCommitPlan {
        proposal: geosolve_constraint_editor::ConstructionProposal::Point {
            point: ConstructionPoint::New([-1.0, 0.2]),
        },
        role: GeometryRole::Profile,
        relations: vec![
            InferredRelation::HorizontalPoints {
                first: DraftPointSlot::Created { point_index: 0 },
                second: DraftPointSlot::Existing(points[0]),
            },
            InferredRelation::VerticalPoints {
                first: DraftPointSlot::Created { point_index: 0 },
                second: DraftPointSlot::Existing(points[0]),
            },
        ],
    };
    let committed = coordinator
        .apply_construction_plan(&expected, &plan)
        .expect("inferred point-pair publication");
    assert!(committed.published_accepted.is_some());
    let durable = committed
        .value
        .constraints
        .iter()
        .map(|created| {
            &coordinator
                .session()
                .design_document()
                .constraint(created.constraint)
                .expect("durable inferred constraint")
                .definition
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        durable.as_slice(),
        [
            DocumentConstraintDefinition::HorizontalPoints { .. },
            DocumentConstraintDefinition::VerticalPoints { .. }
        ]
    ));
    writeln!(
        transcript,
        "atomic=points:{},constraints:{},sources:{}+{}",
        committed.value.construction.points.len(),
        committed.value.constraints.len(),
        committed.value.constraints[0].source,
        committed.value.constraints[1].source
    )
    .expect("string write");

    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let midpoint_plan = ConstructionCommitPlan {
        proposal: geosolve_constraint_editor::ConstructionProposal::Point {
            point: ConstructionPoint::New([-2.8, 0.2]),
        },
        role: GeometryRole::Profile,
        relations: vec![
            InferredRelation::HorizontalPointToMidpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Existing(lines[0]),
            },
            InferredRelation::VerticalPointToMidpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Existing(lines[0]),
            },
        ],
    };
    let midpoint_committed = coordinator
        .apply_construction_plan(&expected, &midpoint_plan)
        .expect("both midpoint axes publication");
    assert!(midpoint_committed.published_accepted.is_some());
    let midpoint_definitions = midpoint_committed
        .value
        .constraints
        .iter()
        .map(|created| {
            &coordinator
                .session()
                .design_document()
                .constraint(created.constraint)
                .expect("durable midpoint-axis constraint")
                .definition
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        midpoint_definitions.as_slice(),
        [
            DocumentConstraintDefinition::HorizontalPointToMidpoint { .. },
            DocumentConstraintDefinition::VerticalPointToMidpoint { .. }
        ]
    ));
    writeln!(
        transcript,
        "atomic-midpoint=points:{},constraints:{},sources:{}+{}",
        midpoint_committed.value.construction.points.len(),
        midpoint_committed.value.constraints.len(),
        midpoint_committed.value.constraints[0].source,
        midpoint_committed.value.constraints[1].source
    )
    .expect("string write");

    let created_circle = ConstructionCommitPlan {
        proposal: geosolve_constraint_editor::ConstructionProposal::Circle {
            center: ConstructionPoint::New([-3.0, 3.0]),
            radius: 0.5,
        },
        role: GeometryRole::Profile,
        relations: vec![InferredRelation::Concentric {
            first: DraftCurveSlot::Created { curve_index: 0 },
            second: DraftCurveSlot::Existing(circles[0].curve),
        }],
    };
    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let created_circle = coordinator
        .apply_construction_plan(&expected, &created_circle)
        .expect("prospective concentric publication");
    assert!(created_circle.published_accepted.is_some());
    writeln!(
        transcript,
        "prospective-circle=curves:{},constraints:{}",
        created_circle.value.construction.curves.len(),
        created_circle.value.constraints.len()
    )
    .expect("string write");

    let created_line = ConstructionCommitPlan {
        proposal: geosolve_constraint_editor::ConstructionProposal::Line {
            start: ConstructionPoint::New([6.0, 0.0]),
            end: ConstructionPoint::New([8.0, 0.0]),
        },
        role: GeometryRole::Profile,
        relations: vec![InferredRelation::Collinear {
            first: DraftLineSupportSlot {
                span: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
                direction: DocumentDirectionSense::Reverse,
            },
            second: DraftLineSupportSlot {
                span: DraftSpanSlot::Existing(lines[0]),
                direction: DocumentDirectionSense::Forward,
            },
        }],
    };
    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let created_line = coordinator
        .apply_construction_plan(&expected, &created_line)
        .expect("prospective collinear publication");
    assert!(created_line.published_accepted.is_some());
    writeln!(
        transcript,
        "prospective-line=curves:{},constraints:{}",
        created_line.value.construction.curves.len(),
        created_line.value.constraints.len()
    )
    .expect("string write");

    let current = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .expect("current draft v5");
    let checkpoint = coordinator
        .persistence_checkpoint()
        .expect("persistence checkpoint");
    assert!(checkpoint.design_uses_draft_v5());
    assert!(checkpoint.accepted_uses_draft_v5());
    assert_eq!(checkpoint.design_json(), current);
    coordinator.undo().expect("undo");
    let after_undo = coordinator.session().design_document().constraints().len();
    coordinator.redo().expect("redo");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("redo draft v5"),
        current
    );
    writeln!(
        transcript,
        "history=undo-constraints:{after_undo},redo-exact:true,cursor:{}",
        coordinator.history_cursor()
    )
    .expect("string write");

    let mut reloaded = RetainedEditorCoordinator::new(reload_session).expect("reload coordinator");
    reloaded.reload(&checkpoint).expect("checkpoint reload");
    let restored = reloaded.session().design_document();
    let retained_planar_count = restored
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::HorizontalPoints { .. }
                    | DocumentConstraintDefinition::VerticalPoints { .. }
                    | DocumentConstraintDefinition::HorizontalPointToMidpoint { .. }
                    | DocumentConstraintDefinition::VerticalPointToMidpoint { .. }
                    | DocumentConstraintDefinition::Concentric { .. }
                    | DocumentConstraintDefinition::Collinear { .. }
            )
        })
        .count();
    writeln!(
        transcript,
        "reload=exact:{},accepted:{},retained-planar:{retained_planar_count},source-order:{}",
        restored.to_draft_v5_json().expect("reload draft v5") == current,
        reloaded
            .session()
            .accepted_state_for_current_input()
            .is_some(),
        restored.source_order().len()
    )
    .expect("string write");
    writeln!(transcript, "baseline-changed:{}", baseline != current).expect("string write");
    transcript.into_bytes()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn m71_transition_oracle_matches_the_cross_target_golden_bytes() {
    let transcript = transition_transcript();
    assert_eq!(
        transcript,
        GOLDEN_TRANSCRIPT,
        "{}",
        String::from_utf8_lossy(&transcript)
    );
}
