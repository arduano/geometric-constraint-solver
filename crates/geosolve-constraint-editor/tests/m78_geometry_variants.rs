// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintEditor, ConstructionCommitPlan, ConstructionCommitToken, ConstructionPoint,
    ConstructionProposal, ConstructionRelationDefinition, ConstructionRelationProvenance,
    CoordinatorError, DraftAuthoringInput, DraftInferenceInput, DraftPointSlot, DraftSpanSlot,
    EditorEffect, EditorMutation, EditorScene, GeometryDraftIssue, GeometryDraftMeasurement,
    GeometryDraftStage, GeometryToolVariant, InferredRelation, Modifiers, PointerInput,
    RetainedEditorCoordinator, Viewport,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentCurveTrimView, DocumentEdit,
    DocumentEndpointRef, DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary,
    DocumentTrimParameter, FeatureEndpoint, GeometryRole, OperationControl, OperationOutcome,
    OperationStopReason, OperationWorkCounter, RetainedSketchDocumentSession, SketchDocument,
    SolverConfig, TangentOrientation,
};

const POINTER_ID: u64 = 0x7800;
const EPSILON: f64 = 1.0e-9;

fn authenticated_empty_scene() -> EditorScene {
    authenticated_scene(SketchDocument::new(10.0).expect("document"))
}

fn authenticated_scene(document: SketchDocument) -> EditorScene {
    authenticated_scene_with_viewport(
        document,
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 50.0).expect("viewport"),
    )
}

fn authenticated_scene_with_viewport(document: SketchDocument, viewport: Viewport) -> EditorScene {
    let session = RetainedSketchDocumentSession::new(
        document,
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
        viewport,
        0.25,
    )
    .expect("empty editor scene")
    .with_retained_session(&session)
    .expect("authenticated empty editor scene")
}

fn tangent_coordinator_fixture() -> (RetainedEditorCoordinator, EditorScene) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [2.0, 0.0]).expect("end");
    document
        .add_curve(
            "source line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("source line");
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
        Viewport::new([1_000.0, 800.0], [0.0, 0.0], 50.0).expect("viewport"),
        0.25,
    )
    .expect("scene")
    .with_retained_session(&session)
    .expect("authenticated scene");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let _ = coordinator
        .editor_mut()
        .activate_geometry_tool(GeometryToolVariant::TangentArc);
    (coordinator, scene)
}

fn coordinator_press(
    coordinator: &mut RetainedEditorCoordinator,
    scene: &EditorScene,
    position: [f64; 2],
) -> Vec<EditorEffect> {
    coordinator.pointer_down_with_draft_authoring(
        scene,
        PointerInput {
            pointer_id: POINTER_ID,
            position: scene.viewport.model_to_screen(position),
            modifiers: Modifiers::default(),
        },
        authoring(false),
    )
}

#[test]
#[allow(clippy::too_many_lines)]
fn m78_plan_provenance_orders_recipe_intent_and_shadows_ambient_direction() {
    let created = |curve_index| DraftSpanSlot::Created {
        curve_index,
        segment: 0,
    };
    let mut document = SketchDocument::new(10.0).expect("document");
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::RectangleLoop {
            points: vec![
                ConstructionPoint::New([0.0, 0.0]),
                ConstructionPoint::New([2.0, 0.0]),
                ConstructionPoint::New([2.0, 2.0]),
                ConstructionPoint::New([0.0, 2.0]),
                ConstructionPoint::New([1.0, 1.0]),
            ],
            corners: [0, 1, 2, 3],
            center: Some(4),
        },
        curve_roles: vec![
            GeometryRole::Profile,
            GeometryRole::Profile,
            GeometryRole::Profile,
            GeometryRole::Profile,
            GeometryRole::Construction,
        ],
        // Deliberately declare these out of lowering order. The ambient
        // Vertical targets a span whose direction is recipe-owned and must not
        // survive as duplicate/conflicting intent.
        relations: vec![
            ConstructionRelationDefinition::auto_inference(InferredRelation::Vertical {
                line: created(0),
            }),
            ConstructionRelationDefinition::recipe_regularization(InferredRelation::EqualLength {
                first: created(0),
                second: created(1),
            }),
            ConstructionRelationDefinition::recipe_intrinsic(InferredRelation::Horizontal {
                line: created(0),
            }),
        ],
    };

    let result = plan.apply(&mut document).expect("ordered recipe plan");
    assert_eq!(
        result
            .construction
            .curves
            .iter()
            .map(|curve| document.geometry_role(*curve).expect("created role"))
            .collect::<Vec<_>>(),
        plan.curve_roles
    );
    assert_eq!(
        result
            .constraints
            .iter()
            .map(|constraint| (constraint.relation_index, constraint.provenance))
            .collect::<Vec<_>>(),
        [
            (2, ConstructionRelationProvenance::RecipeIntrinsic),
            (1, ConstructionRelationProvenance::RecipeRegularization),
        ]
    );
    let labels = result
        .constraints
        .iter()
        .map(|constraint| {
            document
                .constraint(constraint.constraint)
                .expect("created constraint")
                .label
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            "recipe intrinsic horizontal",
            "recipe regularization equal length",
        ]
    );
    assert!(labels.iter().all(|label| !label.contains("auto")));

    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("retained document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted prepared input");
    let precedence_plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::Line {
            start: ConstructionPoint::New([0.0, 0.0]),
            end: ConstructionPoint::New([2.0, 0.0]),
        },
        curve_roles: vec![GeometryRole::Profile],
        relations: vec![
            ConstructionRelationDefinition::auto_inference(InferredRelation::Vertical {
                line: created(0),
            }),
            ConstructionRelationDefinition::recipe_intrinsic(InferredRelation::Horizontal {
                line: created(0),
            }),
        ],
    };
    let committed = coordinator
        .apply_construction_plan(&expected, &precedence_plan)
        .expect("recipe precedence must publish");
    assert!(committed.published_accepted.is_some());
    assert_eq!(committed.value.constraints.len(), 1);
    assert_eq!(
        committed.value.constraints[0].provenance,
        ConstructionRelationProvenance::RecipeIntrinsic
    );
}

#[test]
fn m78_oriented_rectangle_keeps_compatible_ambient_baseline_orientation() {
    let created = |curve_index| DraftSpanSlot::Created {
        curve_index,
        segment: 0,
    };
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::RectangleLoop {
            points: vec![
                ConstructionPoint::New([0.0, 0.0]),
                ConstructionPoint::New([2.0, 0.0]),
                ConstructionPoint::New([2.0, 1.0]),
                ConstructionPoint::New([0.0, 1.0]),
            ],
            corners: [0, 1, 2, 3],
            center: None,
        },
        curve_roles: vec![GeometryRole::Profile; 4],
        relations: vec![
            ConstructionRelationDefinition::recipe_intrinsic(InferredRelation::Perpendicular {
                first: created(0),
                second: created(1),
            }),
            ConstructionRelationDefinition::recipe_intrinsic(InferredRelation::Parallel {
                first: created(0),
                second: created(2),
            }),
            ConstructionRelationDefinition::recipe_intrinsic(InferredRelation::Parallel {
                first: created(1),
                second: created(3),
            }),
            ConstructionRelationDefinition::auto_inference(InferredRelation::Horizontal {
                line: created(0),
            }),
        ],
    };
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted prepared input");

    let committed = coordinator
        .apply_construction_plan(&expected, &plan)
        .expect("compatible oriented rectangle plan must publish");
    assert!(committed.published_accepted.is_some());
    assert_eq!(committed.value.constraints.len(), 4);
    assert_eq!(
        committed
            .value
            .constraints
            .iter()
            .filter(|constraint| {
                constraint.provenance == ConstructionRelationProvenance::AutoInference
            })
            .count(),
        1
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end Tangent Arc contract test audits source and created contact metadata together"
)]
fn m78_tangent_arc_requires_a_native_open_endpoint_and_commits_generic_tangency() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [2.0, 0.0]).expect("end");
    let line = document
        .add_curve(
            "source line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("source line");
    let scene = authenticated_scene(document.clone());
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);

    assert!(
        press(&mut editor, &scene, [1.0, 0.0], false).is_empty(),
        "an interior source point must not begin Tangent Arc"
    );
    assert!(
        press(&mut editor, &scene, [2.0, 0.0], false)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    assert!(
        press(&mut editor, &scene, [2.0, 0.0], false)
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. })),
        "a zero chord must stay correction-ready"
    );
    assert!(
        press(&mut editor, &scene, [3.0, 1.0e-9], false)
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::CommitConstructionPlan { .. })),
        "the tangent-line/infinite-radius limit must stay correction-ready"
    );

    let terminal = terminal_construction(&press(&mut editor, &scene, [3.0, 1.0], false));
    let plan = terminal
        .plan
        .expect("Tangent Arc owns an atomic relation plan");
    assert_eq!(
        plan.relations[0].provenance,
        ConstructionRelationProvenance::RecipeIntrinsic
    );
    let ConstructionProposal::CircularArc {
        center,
        start: arc_start,
        end: arc_end,
        sweep,
    } = &plan.proposal
    else {
        panic!("unexpected Tangent Arc proposal: {:?}", plan.proposal);
    };
    assert_point_close(construction_point_position(center), [2.0, 1.0]);
    assert_point_close(*arc_start, [2.0, 0.0]);
    assert_point_close(*arc_end, [3.0, 1.0]);
    assert_eq!(*sweep, DocumentArcSweep::CounterClockwise);
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [InferredRelation::CurveCurveTangency {
            first,
            second,
            orientation: TangentOrientation::Aligned,
        }] if *first == geosolve_constraint_editor::DraftContactDescriptor {
                span: DraftSpanSlot::Existing(CurveSpan { curve: line, segment: 0 }),
                domain: ContactDomain::Bounded { lower: 0.0, upper: 1.0 },
                parameter: 1.0,
                winding: 0,
                neighborhood: ContactNeighborhood::End,
            }
            && second.span == DraftSpanSlot::Created { curve_index: 0, segment: 0 }
            && second.domain == ContactDomain::Bounded { lower: 0.0, upper: 1.0 }
            && second.parameter.to_bits() == 0.0f64.to_bits()
            && second.winding == 0
            && second.neighborhood == ContactNeighborhood::Start
    ));

    let result = plan.apply(&mut document).expect("atomic tangent arc plan");
    assert_eq!(result.contacts.len(), 2);
    assert!(
        result
            .contacts
            .iter()
            .all(|contact| contact.relation_index == 0)
    );
    let created_arc = result.construction.curves[0];
    assert_eq!(
        document
            .contact(result.contacts[0].contact)
            .expect("source contact")
            .curve,
        CurveSpan::line(line)
    );
    assert_eq!(
        document
            .contact(result.contacts[1].contact)
            .expect("created contact")
            .curve,
        CurveSpan::line(created_arc)
    );
    assert_eq!(result.constraints.len(), 1);
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("tangent arc session");
    assert!(
        session
            .accepted_state_for_current_input()
            .expect("accepted tangent arc")
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= EPSILON)
    );
}

#[test]
fn m78_tangent_arc_analytic_construction_is_scale_safe() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).expect("scaled document");
        let start = document
            .add_point("scaled start", [0.0, 0.0])
            .expect("start");
        let end = document
            .add_point("scaled end", [2.0 * scale, 0.0])
            .expect("end");
        document
            .add_curve(
                "scaled source",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let scene = authenticated_scene_with_viewport(
            document,
            Viewport::new([1_000.0, 800.0], [scale, 0.0], 50.0 / scale).expect("scaled viewport"),
        );
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);
        let _ = press(&mut editor, &scene, [2.0 * scale, 0.0], false);
        let terminal =
            terminal_construction(&press(&mut editor, &scene, [3.0 * scale, scale], false));
        let ConstructionProposal::CircularArc { center, .. } = terminal.proposal else {
            panic!("unexpected scaled Tangent Arc proposal");
        };
        let center = construction_point_position(&center);
        assert_scalar_close(center[0] / scale, 2.0);
        assert_scalar_close(center[1] / scale, 1.0);
    }
}

#[test]
fn m78_tangent_arc_is_one_retained_history_step_and_round_trips_checkpoint() {
    let (mut coordinator, scene) = tangent_coordinator_fixture();
    let original_curves = coordinator.session().design_document().curves().len();
    let original_history = coordinator.history_len();
    let _ = coordinator_press(&mut coordinator, &scene, [2.0, 0.0]);
    let effects = coordinator_press(&mut coordinator, &scene, [3.0, 1.0]);
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("Tangent Arc commit effect");
    let token = match commit {
        EditorEffect::CommitConstructionPlan { token, .. } => *token,
        _ => unreachable!("filtered commit effect"),
    };
    let outcome = coordinator
        .apply_editor_effect(commit)
        .expect("atomic coordinator publication")
        .expect("retained mutation");
    let EditorMutation::InferredConstruction(result) = outcome.value else {
        panic!("expected inferred Tangent Arc construction");
    };
    assert_eq!(result.construction.curves.len(), 1);
    assert_eq!(result.contacts.len(), 2);
    assert_eq!(result.constraints.len(), 1);
    assert_eq!(coordinator.history_len(), original_history + 1);
    assert_eq!(
        coordinator.session().design_document().curves().len(),
        original_curves + 1
    );
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(result.constraints[0].constraint)
            .expect("tangency constraint")
            .definition,
        DocumentConstraintDefinition::CurveCurveTangency { .. }
    ));
    assert!(
        coordinator
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );

    let saved = coordinator
        .persistence_checkpoint()
        .expect("Tangent Arc checkpoint");
    let saved_design = saved.design_json().to_owned();
    coordinator.undo().expect("single-step Undo");
    assert_eq!(
        coordinator.session().design_document().curves().len(),
        original_curves
    );
    coordinator.redo().expect("single-step Redo");
    assert_eq!(
        coordinator.session().design_document().curves().len(),
        original_curves + 1
    );
    coordinator.undo().expect("Undo before reload");
    coordinator.reload(&saved).expect("checkpoint reload");
    assert_eq!(coordinator.history_len(), 1);
    assert_eq!(
        coordinator
            .persistence_checkpoint()
            .expect("restored checkpoint")
            .design_json(),
        saved_design
    );
    assert_eq!(
        coordinator.session().design_document().curves().len(),
        original_curves + 1
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn m78_tangent_arc_stale_plan_reauthenticates_a_moved_source_for_correction() {
    let (mut coordinator, scene) = tangent_coordinator_fixture();
    let _ = coordinator_press(&mut coordinator, &scene, [2.0, 0.0]);
    let effects = coordinator_press(&mut coordinator, &scene, [3.0, 1.0]);
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("Tangent Arc commit effect")
        .clone();
    let token = match &commit {
        EditorEffect::CommitConstructionPlan { token, .. } => *token,
        _ => unreachable!("filtered construction-plan effect"),
    };
    let expected = coordinator.session().design_identity();
    let source_curve = coordinator.session().design_document().curves()[0].id;
    let moved_endpoint = coordinator.session().design_document().points()[1].id;
    coordinator
        .apply_edit(
            expected,
            DocumentEdit::SetPointPosition {
                point: moved_endpoint,
                position: [2.0, 1.0],
            },
        )
        .expect("accepted source-endpoint edit");
    assert_eq!(
        coordinator.editor().pending_construction_commit_token(),
        Some(token),
        "the host must still be able to reject the now-stale publication"
    );
    let before_document = coordinator.session().design_document().clone();
    let before_design = coordinator.session().design_identity();
    let before_attempt = coordinator.session().last_attempt().identity();
    let before_accepted = coordinator.session().accepted_prepared_input();
    let before_history = (coordinator.history_len(), coordinator.history_cursor());
    let before_transcript = coordinator.transcript().len();
    let before_high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();
    let before_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("checkpoint before stale attempt")
        .clone();
    assert!(matches!(
        coordinator.apply_editor_effect(&commit),
        Err(CoordinatorError::StaleInferredConstructionInput)
    ));
    assert!(
        coordinator
            .acknowledge_construction_commit(token, false)
            .is_empty(),
        "rejection keeps the terminal preview visible for correction"
    );
    assert!(
        coordinator
            .editor()
            .pending_construction_commit_token()
            .is_none()
    );
    let status = coordinator
        .editor()
        .geometry_draft_status()
        .expect("stale rejection retains the Tangent Arc draft");
    assert_eq!(status.completed_stages, 1);
    assert_eq!(status.issue, Some(GeometryDraftIssue::ConstructionRejected));
    assert!(coordinator.current_problem_metadata().is_none());
    assert_eq!(coordinator.session().design_document(), &before_document);
    assert_eq!(coordinator.session().design_identity(), before_design);
    assert_eq!(
        coordinator.session().last_attempt().identity(),
        before_attempt
    );
    assert_eq!(
        coordinator.session().accepted_prepared_input(),
        before_accepted
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        before_history
    );
    assert_eq!(coordinator.transcript().len(), before_transcript);
    assert_eq!(
        coordinator.session().persistent_identity_high_water(),
        &before_high_water
    );
    let after_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("checkpoint after stale attempt");
    assert_eq!(
        after_checkpoint.sketch_identity_high_water(),
        before_checkpoint.sketch_identity_high_water()
    );
    assert_eq!(
        after_checkpoint.computed_evaluation_high_water(),
        before_checkpoint.computed_evaluation_high_water()
    );

    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted scene");
    let current_scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        scene.viewport,
        0.25,
    )
    .expect("rebased scene")
    .with_retained_session(coordinator.session())
    .expect("authenticated rebased scene");
    let corrected = [3.0, 2.0];
    assert!(
        coordinator
            .editor_mut()
            .pointer_move(
                &current_scene,
                PointerInput {
                    pointer_id: POINTER_ID,
                    position: current_scene.viewport.model_to_screen(corrected),
                    modifiers: Modifiers::default(),
                },
            )
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
    let corrected_effects = coordinator_press(&mut coordinator, &current_scene, corrected);
    let corrected_plan = corrected_effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { plan, .. } => Some(plan),
            _ => None,
        })
        .expect("corrected Tangent Arc plan");
    let ConstructionProposal::CircularArc {
        center,
        start,
        end,
        sweep,
    } = &corrected_plan.proposal
    else {
        panic!("wrong corrected Tangent Arc proposal");
    };
    assert_point_close(construction_point_position(center), [1.0, 3.0]);
    assert_point_close(*start, [2.0, 1.0]);
    assert_point_close(*end, corrected);
    assert_eq!(*sweep, DocumentArcSweep::CounterClockwise);
    assert!(matches!(
        corrected_plan.relation_payloads().as_slice(),
        [InferredRelation::CurveCurveTangency {
            first,
            second,
            orientation: TangentOrientation::Aligned,
        }] if first.span == DraftSpanSlot::Existing(CurveSpan {
                curve: source_curve,
                segment: 0,
            })
            && first.parameter.to_bits() == 1.0_f64.to_bits()
            && first.neighborhood == ContactNeighborhood::End
            && second.span == DraftSpanSlot::Created {
                curve_index: 0,
                segment: 0,
            }
            && second.parameter.to_bits() == 0.0_f64.to_bits()
            && second.neighborhood == ContactNeighborhood::Start
    ));
}

#[test]
fn m78_deleted_stale_dependency_stays_local_and_can_be_stepped_back() {
    let (mut coordinator, scene) = tangent_coordinator_fixture();
    let source_curve = coordinator.session().design_document().curves()[0].id;
    let _ = coordinator_press(&mut coordinator, &scene, [2.0, 0.0]);
    let effects = coordinator_press(&mut coordinator, &scene, [3.0, 1.0]);
    let commit = effects
        .iter()
        .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
        .expect("pending Tangent Arc plan")
        .clone();
    let token = match &commit {
        EditorEffect::CommitConstructionPlan { token, .. } => *token,
        _ => unreachable!("filtered construction-plan effect"),
    };
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Curve(source_curve),
            },
        )
        .expect("delete stale Tangent Arc dependency");
    assert!(matches!(
        coordinator.apply_editor_effect(&commit),
        Err(CoordinatorError::StaleInferredConstructionInput)
    ));
    assert!(
        coordinator
            .acknowledge_construction_commit(token, false)
            .is_empty()
    );

    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("accepted state after source deletion");
    let current_scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        scene.viewport,
        0.25,
    )
    .expect("scene after source deletion")
    .with_retained_session(coordinator.session())
    .expect("authenticated scene after source deletion");
    assert!(
        coordinator
            .editor_mut()
            .pointer_move(
                &current_scene,
                PointerInput {
                    pointer_id: POINTER_ID,
                    position: current_scene.viewport.model_to_screen([3.0, 2.0]),
                    modifiers: Modifiers::default(),
                },
            )
            .iter()
            .all(|effect| !matches!(
                effect,
                EditorEffect::CommitConstructionPlan { .. } | EditorEffect::PreviewConstruction(_)
            ))
    );
    assert_eq!(
        coordinator
            .editor()
            .geometry_draft_status()
            .expect("local rejected draft")
            .issue,
        Some(GeometryDraftIssue::ConstructionRejected)
    );
    let stepped_back = coordinator.editor_mut().step_back_draft();
    assert!(
        stepped_back
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    let reset = coordinator
        .editor()
        .geometry_draft_status()
        .expect("active Tangent Arc status after step-back");
    assert_eq!(reset.completed_stages, 0);
    assert_eq!(reset.issue, None);
    assert!(coordinator.current_problem_metadata().is_none());
}

#[test]
fn m78_positive_acknowledgement_requires_exact_retained_publication() {
    let (mut coordinator, scene) = tangent_coordinator_fixture();
    let original_document = coordinator.session().design_document().clone();
    let original_history = (coordinator.history_len(), coordinator.history_cursor());
    let _ = coordinator_press(&mut coordinator, &scene, [2.0, 0.0]);
    let effects = coordinator_press(&mut coordinator, &scene, [3.0, 1.0]);
    let token = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { token, .. } => Some(*token),
            _ => None,
        })
        .expect("pending Tangent Arc token");

    assert!(
        coordinator
            .acknowledge_construction_commit(token, true)
            .is_empty(),
        "an unbacked positive acknowledgement must behave as rejection"
    );
    assert!(
        coordinator
            .editor()
            .pending_construction_commit_token()
            .is_none()
    );
    assert_eq!(
        coordinator
            .editor()
            .geometry_draft_status()
            .expect("correction-ready draft")
            .issue,
        Some(GeometryDraftIssue::ConstructionRejected)
    );
    assert_eq!(coordinator.session().design_document(), &original_document);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        original_history
    );

    assert!(
        coordinator
            .editor_mut()
            .pointer_move(
                &scene,
                PointerInput {
                    pointer_id: POINTER_ID,
                    position: scene.viewport.model_to_screen([3.5, 1.5]),
                    modifiers: Modifiers::default(),
                },
            )
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
    );
}

#[test]
fn m78_controlled_publication_supplies_positive_acknowledgement_evidence() {
    let (mut coordinator, scene) = tangent_coordinator_fixture();
    let original_curves = coordinator.session().design_document().curves().len();
    let original_history = coordinator.history_len();
    let _ = coordinator_press(&mut coordinator, &scene, [2.0, 0.0]);
    let effects = coordinator_press(&mut coordinator, &scene, [3.0, 1.0]);
    let (expected, token, plan) = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan {
                expected,
                token,
                plan,
            } => Some((**expected, *token, plan.clone())),
            _ => None,
        })
        .expect("pending Tangent Arc plan");

    assert!(matches!(
        coordinator
            .apply_construction_plan_controlled(&expected, &plan, OperationControl::unlimited())
            .expect("controlled Tangent Arc publication"),
        OperationOutcome::Completed { .. }
    ));
    assert_eq!(
        coordinator.session().design_document().curves().len(),
        original_curves + 1
    );
    assert_eq!(coordinator.history_len(), original_history + 1);
    assert!(
        coordinator
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    assert!(
        coordinator
            .editor()
            .pending_construction_commit_token()
            .is_none()
    );
}

#[test]
fn m78_zero_budget_stops_large_proposal_before_transactional_lowering() {
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let expected = coordinator
        .session()
        .accepted_prepared_input()
        .expect("accepted input");
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::Polyline {
            points: (0..10_000)
                .map(|index| ConstructionPoint::New([f64::from(index), 0.0]))
                .collect(),
        },
        curve_roles: vec![GeometryRole::Profile],
        relations: Vec::new(),
    };
    let before_document = coordinator.session().design_document().clone();
    let before_input = coordinator.session().accepted_prepared_input();
    let before_history = (coordinator.history_len(), coordinator.history_cursor());
    let before_transcript = coordinator.transcript().len();
    let before_high_water = coordinator
        .session()
        .persistent_identity_high_water()
        .clone();
    let before_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("checkpoint")
        .clone();
    let mut control = OperationControl::unlimited();
    control.limits.document_validation_items = 0;

    let OperationOutcome::WorkExhausted { report } = coordinator
        .apply_construction_plan_controlled(&expected, &plan, control)
        .expect("controlled proposal")
    else {
        panic!("zero document-validation budget must stop proposal lowering");
    };
    assert_eq!(report.consumed.document_validation_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentValidationItems,
            checkpoint: geosolve_sketch::OperationCheckpoint::DocumentValidation,
        })
    );

    let mut lowering_control = OperationControl::unlimited();
    lowering_control.limits.document_validation_items = 1;
    lowering_control.limits.document_lowering_items = 0;
    let OperationOutcome::WorkExhausted { report } = coordinator
        .apply_construction_plan_controlled(&expected, &plan, lowering_control)
        .expect("lowering-controlled proposal")
    else {
        panic!("zero proposal-lowering budget must stop before candidate allocation");
    };
    assert_eq!(report.consumed.document_validation_items, 1);
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentLoweringItems,
            checkpoint: geosolve_sketch::OperationCheckpoint::DocumentLowering,
        })
    );
    assert_eq!(coordinator.session().design_document(), &before_document);
    assert_eq!(
        coordinator.session().accepted_prepared_input(),
        before_input
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        before_history
    );
    assert_eq!(coordinator.transcript().len(), before_transcript);
    assert_eq!(
        coordinator.session().persistent_identity_high_water(),
        &before_high_water
    );
    let after_checkpoint = coordinator
        .persistence_checkpoint()
        .expect("checkpoint after stopped proposal");
    assert_eq!(
        after_checkpoint.sketch_identity_high_water(),
        before_checkpoint.sketch_identity_high_water()
    );
    assert_eq!(
        after_checkpoint.computed_evaluation_high_water(),
        before_checkpoint.computed_evaluation_high_water()
    );
}

#[test]
fn m78_tangent_arc_resolves_whole_curve_start_and_multispan_end_orientation() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [[-2.0, 0.0], [0.0, 0.0], [0.0, 2.0]].map(|position| {
        document
            .add_point("polyline point", position)
            .expect("point")
    });
    let polyline = document
        .add_curve(
            "source polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .expect("polyline");
    let scene = authenticated_scene(document.clone());
    let cases = [
        (
            [-2.0, 0.0],
            [-3.0, 1.0],
            0,
            TangentOrientation::Opposed,
            [-2.0, 1.0],
            DocumentArcSweep::Clockwise,
        ),
        (
            [0.0, 2.0],
            [-1.0, 3.0],
            1,
            TangentOrientation::Aligned,
            [-1.0, 2.0],
            DocumentArcSweep::CounterClockwise,
        ),
    ];
    for (source, end, segment, orientation, expected_center, expected_sweep) in cases {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);
        let _ = press(&mut editor, &scene, source, false);
        let terminal = terminal_construction(&press(&mut editor, &scene, end, false));
        let plan = terminal.plan.expect("atomic Tangent Arc plan");
        let ConstructionProposal::CircularArc { center, sweep, .. } = &plan.proposal else {
            panic!("unexpected Tangent Arc proposal: {:?}", plan.proposal);
        };
        assert_point_close(construction_point_position(center), expected_center);
        assert_eq!(*sweep, expected_sweep);
        assert!(matches!(
            plan.relation_payloads().as_slice(),
            [InferredRelation::CurveCurveTangency {
                first,
                orientation: actual,
                ..
            }] if first.span == DraftSpanSlot::Existing(CurveSpan { curve: polyline, segment })
                && *actual == orientation
        ));
        assert_plan_solves_on_document(&plan, document.clone());
    }
}

#[test]
fn m78_tangent_arc_uses_the_last_semantic_span_of_an_open_nonlinear_curve() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let controls = [[-1.0, 0.0], [0.0, 2.0], [1.0, -1.0], [2.0, 2.0], [3.0, 0.0]].map(|position| {
        document
            .add_point("spline control", position)
            .expect("control")
    });
    let spline = document
        .add_curve(
            "open spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0],
                span_ids: vec![11, 17, 29],
                next_span_id: 30,
            },
        )
        .expect("spline");
    let endpoint = DocumentEndpointRef {
        curve: spline,
        endpoint: FeatureEndpoint::End,
    };
    let seed = document
        .curve_endpoint_contact_seed(endpoint)
        .expect("semantic End seed");
    let jet = document
        .evaluate_curve_jet(seed.support.span, seed.parameter)
        .expect("endpoint jet");
    let differential = jet.differential().expect("regular endpoint");
    let source = [jet.position.x, jet.position.y];
    let tangent = [differential.unit_tangent.x, differential.unit_tangent.y];
    let normal = [-tangent[1], tangent[0]];
    let target = [
        source[0] + tangent[0] + normal[0],
        source[1] + tangent[1] + normal[1],
    ];
    let scene = authenticated_scene(document.clone());
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);
    let _ = press(&mut editor, &scene, source, false);
    let terminal = terminal_construction(&press(&mut editor, &scene, target, false));
    let plan = terminal.plan.expect("nonlinear Tangent Arc plan");
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [InferredRelation::CurveCurveTangency {
            first,
            orientation: TangentOrientation::Aligned,
            ..
        }] if first.span == DraftSpanSlot::Existing(CurveSpan { curve: spline, segment: 29 })
            && first.parameter.to_bits() == 1.0f64.to_bits()
            && first.neighborhood == ContactNeighborhood::End
    ));
    assert_plan_solves_on_document(&plan, document);
}

#[test]
fn m78_tangent_arc_skips_ineligible_topology_and_rejects_shared_endpoint_ambiguity() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let closed_points = [[-4.0, -4.0], [-2.0, -4.0], [-3.0, -2.0]]
        .map(|position| document.add_point("closed point", position).expect("point"));
    let _closed = document
        .add_curve(
            "closed source",
            CurveDefinition::Polyline {
                points: closed_points.to_vec(),
                closed: true,
                branch_directions: vec![
                    [1.0, 0.0],
                    [-0.447_213_595_499_957_9, 0.894_427_190_999_915_9],
                    [-0.447_213_595_499_957_9, -0.894_427_190_999_915_9],
                ],
            },
        )
        .expect("closed polyline");
    let line_start = document
        .add_point("nearby line start", [0.05, 0.0])
        .expect("line start");
    let line_end = document
        .add_point("nearby line end", [2.05, 0.0])
        .expect("line end");
    let line = document
        .add_curve(
            "eligible line",
            CurveDefinition::Line {
                start: line_start,
                end: line_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let scene = authenticated_scene(document);
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);
    let _ = press(&mut editor, &scene, [0.05, 0.0], false);
    let terminal = terminal_construction(&press(&mut editor, &scene, [-0.95, 1.0], false));
    let plan = terminal
        .plan
        .expect("valid endpoint survives rejected closed topology");
    assert!(matches!(
        plan.relation_payloads().as_slice(),
        [InferredRelation::CurveCurveTangency { first, .. }]
            if first.span == DraftSpanSlot::Existing(CurveSpan { curve: line, segment: 0 })
    ));

    let mut shared = SketchDocument::new(10.0).expect("shared document");
    let first = shared.add_point("first", [0.0, 0.0]).expect("first");
    let junction = shared
        .add_point("shared junction", [2.0, 0.0])
        .expect("junction");
    let last = shared.add_point("last", [2.0, 2.0]).expect("last");
    shared
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start: first,
                end: junction,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    shared
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: junction,
                end: last,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    let scene = authenticated_scene(shared);
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::TangentArc);
    assert!(
        press(&mut editor, &scene, [2.0, 0.0], false).is_empty(),
        "a shared endpoint must not choose its support from persistent ID order"
    );
}

#[test]
fn m78_tangent_arc_requires_the_semantic_endpoint_to_be_visibly_untrimmed() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [2.0, 0.0]).expect("end");
    let line = document
        .add_curve(
            "trimmed line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let span = CurveSpan::line(line);
    document
        .replace_trim_views(
            span,
            vec![DocumentCurveTrimView {
                support: span,
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.25,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 1.0,
                    winding: 0,
                }),
            }],
        )
        .expect("trimmed visible interval");
    let scene = authenticated_scene(document);

    let mut hidden_start = ConstraintEditor::default();
    let _ = hidden_start.activate_geometry_tool(GeometryToolVariant::TangentArc);
    assert!(
        press(&mut hidden_start, &scene, [0.0, 0.0], false).is_empty(),
        "a support endpoint outside every painted interval is unavailable"
    );

    let mut visible_end = ConstraintEditor::default();
    let _ = visible_end.activate_geometry_tool(GeometryToolVariant::TangentArc);
    assert!(
        press(&mut visible_end, &scene, [2.0, 0.0], false)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_))),
        "the untrimmed semantic End remains eligible"
    );
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

fn press_with_inference(
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
                suppressed: false,
                preferred_candidate: None,
            },
            regularized: false,
        },
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
    assert_plan_solves_on_document(plan, SketchDocument::new(10.0).expect("document"));
}

fn assert_plan_solves_on_document(plan: &ConstructionCommitPlan, mut document: SketchDocument) {
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

fn authored_plan_with_existing_samples(
    variant: GeometryToolVariant,
    clicks: &[[f64; 2]],
) -> (SketchDocument, ConstructionCommitPlan) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let mut positions = Vec::<[f64; 2]>::new();
    for &position in clicks {
        if positions.iter().any(|existing| {
            existing[0].to_bits() == position[0].to_bits()
                && existing[1].to_bits() == position[1].to_bits()
        }) {
            continue;
        }
        document
            .add_point("existing recipe sample", position)
            .expect("sample point");
        positions.push(position);
    }
    let scene = authenticated_scene(document.clone());
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(variant);
    for &click in &clicks[..clicks.len() - 1] {
        let _ = press_with_inference(&mut editor, &scene, click);
    }
    let terminal = terminal_construction(&press_with_inference(
        &mut editor,
        &scene,
        *clicks.last().expect("terminal click"),
    ));
    (
        document,
        terminal.plan.expect("exact recipe owns atomic plan"),
    )
}

fn assert_created_incidence_plan(
    variant: GeometryToolVariant,
    clicks: &[[f64; 2]],
    expected_incidence: usize,
) {
    let (mut document, plan) = authored_plan_with_existing_samples(variant, clicks);
    let incidence = plan
        .relations
        .iter()
        .filter(|relation| {
            relation.provenance == ConstructionRelationProvenance::AutoInference
                && matches!(
                    relation.relation,
                    InferredRelation::PointOnCurve {
                        point: DraftPointSlot::Existing(_),
                        contact
                    } if contact.span == DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    }
                )
        })
        .count();
    assert_eq!(incidence, expected_incidence, "variant {variant:?}");
    let result = plan.apply(&mut document).expect("created-incidence plan");
    assert_eq!(result.contacts.len(), expected_incidence);
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("created-incidence session");
    assert!(
        session
            .accepted_state_for_current_input()
            .expect("created-incidence accepted state")
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
fn m78_fixed_recipe_status_publishes_live_pointer_measurements() {
    let scene = authenticated_empty_scene();

    let mut segment = ConstraintEditor::default();
    let _ = segment.activate_geometry_tool(GeometryToolVariant::Segment);
    let _ = press(&mut segment, &scene, [1.0, 1.0], false);
    let _ = move_pointer(&mut segment, &scene, [4.0, 5.0], false);
    let measurements = segment
        .geometry_draft_status()
        .expect("live Segment status")
        .measurements;
    assert!(measurements.iter().any(
        |measurement| matches!(measurement, GeometryDraftMeasurement::Length(value) if (*value - 5.0).abs() <= EPSILON)
    ));

    let mut circle = ConstraintEditor::default();
    let _ = circle.activate_geometry_tool(GeometryToolVariant::CenterRadiusCircle);
    let _ = press(&mut circle, &scene, [1.0, 1.0], false);
    let _ = move_pointer(&mut circle, &scene, [4.0, 1.0], false);
    let measurements = circle
        .geometry_draft_status()
        .expect("live Circle status")
        .measurements;
    assert!(measurements.iter().any(
        |measurement| matches!(measurement, GeometryDraftMeasurement::Radius(value) if (*value - 3.0).abs() <= EPSILON)
    ));
    assert!(measurements.iter().any(
        |measurement| matches!(measurement, GeometryDraftMeasurement::Diameter(value) if (*value - 6.0).abs() <= EPSILON)
    ));

    let mut rectangle = ConstraintEditor::default();
    let _ = rectangle.activate_geometry_tool(GeometryToolVariant::TwoPointAlignedRectangle);
    let _ = press(&mut rectangle, &scene, [1.0, 1.0], false);
    let _ = move_pointer(&mut rectangle, &scene, [5.0, 3.0], false);
    assert!(
        rectangle
            .geometry_draft_status()
            .expect("live Rectangle status")
            .measurements
            .iter()
            .any(|measurement| matches!(
                measurement,
                GeometryDraftMeasurement::WidthHeight { width, height }
                    if (*width - 4.0).abs() <= EPSILON && (*height - 2.0).abs() <= EPSILON
            ))
    );

    let mut ellipse = ConstraintEditor::default();
    let _ = ellipse.activate_geometry_tool(GeometryToolVariant::CenterAxesEllipse);
    let _ = press(&mut ellipse, &scene, [1.0, 1.0], false);
    let _ = press(&mut ellipse, &scene, [5.0, 1.0], false);
    let _ = move_pointer(&mut ellipse, &scene, [1.0, 3.0], false);
    assert!(ellipse
        .geometry_draft_status()
        .expect("live Ellipse status")
        .measurements
        .iter()
        .any(
            |measurement| matches!(measurement, GeometryDraftMeasurement::Ratio(value) if (*value - 0.5).abs() <= EPSILON)
        ));

    let mut arc = ConstraintEditor::default();
    let _ = arc.activate_geometry_tool(GeometryToolVariant::CenterArc);
    let _ = press(&mut arc, &scene, [1.0, 1.0], false);
    let _ = press(&mut arc, &scene, [3.0, 1.0], false);
    let _ = move_pointer(&mut arc, &scene, [1.0, 3.0], false);
    let measurements = arc
        .geometry_draft_status()
        .expect("live Arc status")
        .measurements;
    assert!(measurements.iter().any(
        |measurement| matches!(measurement, GeometryDraftMeasurement::Radius(value) if (*value - 2.0).abs() <= EPSILON)
    ));
    assert!(measurements.iter().any(|measurement| matches!(
        measurement,
        GeometryDraftMeasurement::AngleRadians(value)
            if (*value - std::f64::consts::FRAC_PI_2).abs() <= EPSILON
    )));
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
        [ConstructionRelationDefinition::recipe_intrinsic(
            InferredRelation::Midpoint {
                point: DraftPointSlot::Created { point_index: 0 },
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
            },
        )]
    );
    assert_plan_solves_with_finite_geometry(&plan);
}

#[test]
fn m78_exact_single_curve_variants_all_lower_through_atomic_plans() {
    let scene = authenticated_empty_scene();
    let cases = [
        (GeometryToolVariant::Segment, vec![[1.0, 1.0], [4.0, 2.0]]),
        (
            GeometryToolVariant::CenterRadiusCircle,
            vec![[1.0, 1.0], [3.0, 1.0]],
        ),
        (
            GeometryToolVariant::QuadraticBezier,
            vec![[0.0, 0.0], [2.0, 3.0], [4.0, 0.0]],
        ),
        (
            GeometryToolVariant::CubicBezier,
            vec![[0.0, 0.0], [1.0, 3.0], [3.0, 3.0], [4.0, 0.0]],
        ),
        (
            GeometryToolVariant::RationalQuadraticConic,
            vec![[0.0, 0.0], [2.0, 3.0], [4.0, 0.0]],
        ),
        (GeometryToolVariant::Parabola, vec![[0.0, 0.0], [0.0, 2.0]]),
        (GeometryToolVariant::Hyperbola, vec![[0.0, 0.0], [2.0, 0.0]]),
    ];

    for (variant, clicks) in cases {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        for &click in &clicks[..clicks.len() - 1] {
            let _ = press(&mut editor, &scene, click, false);
        }
        let terminal = terminal_construction(&press(
            &mut editor,
            &scene,
            *clicks.last().expect("terminal click"),
            false,
        ));
        let proposal_matches_variant = matches!(
            (variant, &terminal.proposal),
            (
                GeometryToolVariant::Segment,
                ConstructionProposal::Line { .. }
            ) | (
                GeometryToolVariant::CenterRadiusCircle,
                ConstructionProposal::Circle { .. }
            ) | (
                GeometryToolVariant::QuadraticBezier,
                ConstructionProposal::QuadraticBezier { .. }
            ) | (
                GeometryToolVariant::CubicBezier,
                ConstructionProposal::CubicBezier { .. }
            ) | (
                GeometryToolVariant::RationalQuadraticConic,
                ConstructionProposal::RationalQuadraticConic { .. }
            ) | (
                GeometryToolVariant::Parabola,
                ConstructionProposal::Parabola { .. }
            ) | (
                GeometryToolVariant::Hyperbola,
                ConstructionProposal::Hyperbola { .. }
            )
        );
        assert!(proposal_matches_variant, "wrong proposal for {variant:?}");
        let plan = terminal
            .plan
            .as_ref()
            .expect("exact recipe emits an atomic plan");
        assert_plan_solves_with_finite_geometry(plan);
    }
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
    let mut expected_roles = vec![GeometryRole::Profile; 4];
    if center.is_some() {
        expected_roles.push(GeometryRole::Construction);
    }
    assert_eq!(plan.curve_roles, expected_roles);
    assert_eq!(
        plan.relations.len(),
        case.ordinary_relation_count + usize::from(regularized)
    );
    assert!(
        plan.relations[..case.ordinary_relation_count]
            .iter()
            .all(|relation| relation.provenance == ConstructionRelationProvenance::RecipeIntrinsic)
    );
    assert_eq!(
        plan.relations
            .iter()
            .filter(|relation| {
                matches!(relation.relation, InferredRelation::EqualLength { .. })
            })
            .count(),
        usize::from(regularized)
    );
    assert_eq!(
        plan.relations
            .iter()
            .filter(|relation| relation.provenance
                == ConstructionRelationProvenance::RecipeRegularization)
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
    let plan = terminal
        .plan
        .as_ref()
        .expect("exact diameter recipe owns an atomic geometry-only plan");
    assert!(plan.relations.is_empty());
    assert_plan_solves_with_finite_geometry(plan);

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
    let plan = terminal
        .plan
        .as_ref()
        .expect("exact three-point recipe owns an atomic geometry-only plan");
    assert!(plan.relations.is_empty());
    assert_plan_solves_with_finite_geometry(plan);
}

#[test]
fn m78_three_point_circle_and_arc_are_translation_stable_at_large_coordinates() {
    const ORIGIN: f64 = 1.0e12;
    let viewport = Viewport::new([1_000.0, 800.0], [ORIGIN, ORIGIN], 50.0).expect("viewport");
    let scene =
        authenticated_scene_with_viewport(SketchDocument::new(10.0).expect("document"), viewport);
    for variant in [
        GeometryToolVariant::ThreePointCircle,
        GeometryToolVariant::ThreePointArc,
    ] {
        let mut editor = ConstraintEditor::default();
        let _ = editor.activate_geometry_tool(variant);
        let _ = press(&mut editor, &scene, [ORIGIN + 1.0, ORIGIN], false);
        let _ = press(&mut editor, &scene, [ORIGIN, ORIGIN + 1.0], false);
        let terminal =
            terminal_construction(&press(&mut editor, &scene, [ORIGIN - 1.0, ORIGIN], false));
        let (center, radius) = match &terminal.proposal {
            ConstructionProposal::Circle { center, radius } => {
                (construction_point_position(center), *radius)
            }
            ConstructionProposal::CircularArc { center, start, .. } => {
                let center = construction_point_position(center);
                (center, (start[0] - center[0]).hypot(start[1] - center[1]))
            }
            proposal => panic!("wrong translated {variant:?} proposal: {proposal:?}"),
        };
        assert_point_close(center, [ORIGIN, ORIGIN]);
        assert_scalar_close(radius, 1.0);
    }
}

#[test]
fn m78_created_curve_incidence_is_atomic_across_round_and_elliptic_recipes() {
    assert_created_incidence_plan(
        GeometryToolVariant::ThreePointCircle,
        &[[3.0, 1.0], [1.0, 3.0], [-1.0, 1.0]],
        3,
    );
    assert_created_incidence_plan(
        GeometryToolVariant::CenterArc,
        &[[1.0, 1.0], [3.0, 1.0], [1.0, 3.0]],
        2,
    );
    assert_created_incidence_plan(
        GeometryToolVariant::AxisEndpointsEllipse,
        &[[3.0, 1.0], [-1.0, 1.0], [1.0, 2.0]],
        1,
    );
    assert_created_incidence_plan(
        GeometryToolVariant::CenterAxesEllipticalArc,
        &[[1.0, 1.0], [3.0, 1.0], [1.0, 2.0], [3.0, 1.0], [1.0, 2.0]],
        2,
    );
}

#[test]
fn m78_created_curve_incidence_rejects_projected_off_support_points() {
    let mut document = SketchDocument::new(10.0).expect("document");
    for (label, position) in [
        ("center", [1.0, 1.0]),
        ("start", [3.0, 1.0]),
        ("off-support end", [1.0, 3.05]),
    ] {
        document.add_point(label, position).expect("sample point");
    }
    let scene = authenticated_scene(document.clone());
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::CenterArc);
    let _ = press_with_inference(&mut editor, &scene, [1.0, 1.0]);
    let _ = press_with_inference(&mut editor, &scene, [3.0, 1.0]);
    let rejected = press_with_inference(&mut editor, &scene, [1.0, 3.05]);
    assert!(rejected.iter().all(|effect| !matches!(
        effect,
        EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
    )));
    let status = editor
        .geometry_draft_status()
        .expect("off-support rejection keeps draft");
    assert_eq!(status.stage, GeometryDraftStage::End);
    assert_eq!(status.completed_stages, 2);
    assert_eq!(
        status.issue,
        Some(GeometryDraftIssue::IncompatibleConstraintIntent)
    );

    let corrected = terminal_construction(&press(&mut editor, &scene, [1.0, 3.0], false));
    let plan = corrected.plan.expect("corrected Center Arc plan");
    assert_eq!(
        plan.relations
            .iter()
            .filter(|relation| {
                matches!(relation.relation, InferredRelation::PointOnCurve { .. })
            })
            .count(),
        1,
        "only the exact snapped Start remains associative"
    );
    let result = plan.apply(&mut document).expect("corrected plan");
    assert_eq!(result.contacts.len(), 1);
}

#[test]
fn m78_axis_endpoint_elliptical_arc_does_not_promise_opposite_pole_incidence() {
    let mut document = SketchDocument::new(10.0).expect("document");
    document
        .add_point("opposite major pole", [-1.0, 1.0])
        .expect("opposite pole");
    let scene = authenticated_scene(document);
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::AxisEndpointsEllipticalArc);
    let _ = press(&mut editor, &scene, [3.0, 1.0], false);
    let opposite_pole = press_with_inference(&mut editor, &scene, [-1.0, 1.0]);
    assert!(
        opposite_pole
            .iter()
            .all(|effect| !matches!(effect, EditorEffect::DraftInferenceChanged(Some(_))))
    );
    for click in [[1.0, 2.0], [3.0, 1.0]] {
        let _ = press(&mut editor, &scene, click, false);
    }
    let terminal = terminal_construction(&press(&mut editor, &scene, [1.0, 2.0], false));
    let plan = terminal.plan.expect("axis-endpoint arc plan");
    assert!(matches!(
        plan.proposal,
        ConstructionProposal::AxisEndpointEllipticalArc { .. }
    ));
    assert!(plan.relations.is_empty());
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
    assert_eq!(
        three_point_arc
            .geometry_draft_status()
            .expect("3-Point Arc End stage")
            .stage,
        GeometryDraftStage::End
    );
    let _ = press(&mut three_point_arc, &scene, [-1.0, 2.0], false);
    assert_eq!(
        three_point_arc
            .geometry_draft_status()
            .expect("3-Point Arc Through stage")
            .stage,
        GeometryDraftStage::ThroughPoint
    );
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
    assert_eq!(
        status.issue,
        Some(GeometryDraftIssue::InvalidTerminalGeometry)
    );
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
    assert_eq!(status.issue, Some(GeometryDraftIssue::ConstructionRejected));

    let replacement = move_pointer(&mut midpoint, &scene, [5.0, 3.0], false);
    let replacement_endpoint = replacement.iter().find_map(|effect| match effect {
        EditorEffect::PreviewConstruction(
            geosolve_constraint_editor::ConstructionPreview::Complete {
                proposal: ConstructionProposal::MidpointLine { endpoint, .. },
                ..
            },
        ) => Some(endpoint),
        _ => None,
    });
    assert_point_close(
        construction_point_position(replacement_endpoint.expect("replacement preview")),
        [5.0, 3.0],
    );
    assert_eq!(
        midpoint
            .geometry_draft_status()
            .expect("valid replacement preview")
            .issue,
        None
    );
    let replacement = terminal_construction(&press(&mut midpoint, &scene, [5.0, 3.0], false));
    assert!(replacement.token.is_some());
}

#[test]
fn m78_finish_issue_is_local_and_clears_on_step_back_or_correction() {
    let scene = authenticated_empty_scene();
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::OpenControlNurbs);
    for position in [[1.0, 1.0], [2.0, 3.0]] {
        let _ = press(&mut editor, &scene, position, false);
    }

    assert!(editor.complete_draft(scene.design_identity).is_empty());
    assert_eq!(
        editor
            .geometry_draft_status()
            .expect("unfinished NURBS status")
            .issue,
        Some(GeometryDraftIssue::CannotFinish)
    );
    let _ = editor.step_back_draft();
    assert_eq!(
        editor
            .geometry_draft_status()
            .expect("stepped-back NURBS status")
            .issue,
        None
    );

    for position in [[2.0, 3.0], [4.0, 3.0], [5.0, 1.0]] {
        let _ = press(&mut editor, &scene, position, false);
    }
    assert!(editor.can_complete_draft());
    assert_eq!(
        editor
            .geometry_draft_status()
            .expect("corrected NURBS status")
            .issue,
        None
    );
}

#[test]
fn m78_one_sample_rejection_remains_a_draft_local_recoverable_issue() {
    let scene = authenticated_empty_scene();
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_geometry_tool(GeometryToolVariant::SketchPoint);
    let terminal = terminal_construction(&press(&mut editor, &scene, [2.0, 3.0], false));
    let token = terminal.token.expect("point construction token");
    assert!(
        editor
            .acknowledge_construction_commit(token, false)
            .is_empty()
    );
    assert_eq!(
        editor
            .geometry_draft_status()
            .expect("rejected point status")
            .issue,
        Some(GeometryDraftIssue::ConstructionRejected)
    );

    assert!(
        editor
            .escape_geometry_tool()
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    let status = editor
        .geometry_draft_status()
        .expect("first Escape retains exact point tool");
    assert_eq!(status.variant, GeometryToolVariant::SketchPoint);
    assert_eq!(status.issue, None);
}
