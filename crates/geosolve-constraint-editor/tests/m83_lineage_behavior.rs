// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintActionRequest, ConstraintIntent, CurveNumericPropertyKind, DraftAuthoringInput,
    DraftInferenceInput, EditorEffect, EditorScene, GeometryToolVariant, Modifiers, PointerInput,
    RetainedEditorCoordinator, SelectionItem, Viewport, evaluate_lineage_session_cold,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentEdit, DocumentElementId, DocumentParameterKind,
    DocumentParameterTarget, DocumentSolveRequest, GeometryRole, HostActivationOverride,
    HostConfigurationActivation, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageMaterializationMap, LineageOutput, LineageOutputIdentity,
    LineageOutputIdentityFlow, LineageOutputKind, LineageReservation, LineageReservationKind,
    LineageSession, LineageStepState,
};

const POINTER_ID: u64 = 0x83_0001;

#[derive(Clone, Copy)]
enum Completion {
    FinalClick,
    ExplicitFinish,
}

#[derive(Clone, Copy)]
enum StartingFixture {
    Empty,
    ReusedPoint,
    TangentLine,
}

#[derive(Clone, Copy)]
struct RecipeCase {
    variant: GeometryToolVariant,
    clicks: &'static [[f64; 2]],
    completion: Completion,
    fixture: StartingFixture,
    regularized: bool,
    flip_branch: bool,
}

const RECIPE_CASES: [RecipeCase; 25] = [
    RecipeCase {
        variant: GeometryToolVariant::SketchPoint,
        clicks: &[[1.0, 1.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::Segment,
        clicks: &[[1.0, 1.0], [4.0, 2.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::ReusedPoint,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::Polyline,
        clicks: &[[1.0, 1.0], [4.0, 1.0], [4.0, 3.0]],
        completion: Completion::ExplicitFinish,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::MidpointLine,
        clicks: &[[1.0, 1.0], [4.0, 2.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::TwoPointAlignedRectangle,
        clicks: &[[1.0, 1.0], [5.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::ThreePointCornerRectangle,
        clicks: &[[1.0, 1.0], [5.0, 1.0], [5.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CenterRectangle,
        clicks: &[[3.0, 3.0], [5.0, 4.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: true,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::ThreePointCenterRectangle,
        clicks: &[[3.0, 3.0], [3.0, 5.0], [0.0, 5.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CenterRadiusCircle,
        clicks: &[[1.0, 1.0], [3.0, 1.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::TwoPointDiameterCircle,
        clicks: &[[1.0, 2.0], [5.0, 2.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::ThreePointCircle,
        clicks: &[[3.0, 2.0], [1.0, 4.0], [-1.0, 2.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CenterArc,
        clicks: &[[1.0, 1.0], [3.0, 1.0], [1.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: true,
    },
    RecipeCase {
        variant: GeometryToolVariant::ThreePointArc,
        clicks: &[[3.0, 2.0], [-1.0, 2.0], [1.0, 4.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::TangentArc,
        clicks: &[[2.0, 0.0], [3.0, 1.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::TangentLine,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CenterAxesEllipse,
        clicks: &[[1.0, 1.0], [5.0, 1.0], [1.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::AxisEndpointsEllipse,
        clicks: &[[5.0, 1.0], [-3.0, 1.0], [1.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CenterAxesEllipticalArc,
        clicks: &[[1.0, 1.0], [5.0, 1.0], [1.0, 3.0], [5.0, 1.0], [1.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: true,
    },
    RecipeCase {
        variant: GeometryToolVariant::AxisEndpointsEllipticalArc,
        clicks: &[[5.0, 1.0], [-3.0, 1.0], [1.0, 3.0], [5.0, 1.0], [1.0, 3.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::QuadraticBezier,
        clicks: &[[0.0, 0.0], [2.0, 3.0], [4.0, 0.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::CubicBezier,
        clicks: &[[0.0, 0.0], [1.0, 3.0], [3.0, 3.0], [4.0, 0.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::RationalQuadraticConic,
        clicks: &[[0.0, 0.0], [2.0, 3.0], [4.0, 0.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::Parabola,
        clicks: &[[0.0, 0.0], [0.0, 2.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::Hyperbola,
        clicks: &[[0.0, 0.0], [2.0, 0.0]],
        completion: Completion::FinalClick,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::OpenControlNurbs,
        clicks: &[[1.0, 1.0], [2.0, 3.0], [4.0, 3.0], [5.0, 1.0]],
        completion: Completion::ExplicitFinish,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
    RecipeCase {
        variant: GeometryToolVariant::PeriodicControlNurbs,
        clicks: &[[1.0, 1.0], [2.0, 3.0], [4.0, 3.0], [5.0, 1.0]],
        completion: Completion::ExplicitFinish,
        fixture: StartingFixture::Empty,
        regularized: false,
        flip_branch: false,
    },
];

type Manifest = (
    Vec<LineageOutput>,
    Vec<LineageOutputIdentity>,
    Vec<LineageReservation>,
);

fn fixture(kind: StartingFixture) -> SketchDocument {
    let mut document = SketchDocument::new(10.0).expect("sketch fixture");
    match kind {
        StartingFixture::Empty => {}
        StartingFixture::ReusedPoint => {
            document
                .add_point("reused recipe point", [1.0, 1.0])
                .expect("reused point");
        }
        StartingFixture::TangentLine => {
            let start = document
                .add_point("tangent source start", [0.0, 0.0])
                .expect("source start");
            let end = document
                .add_point("tangent source end", [2.0, 0.0])
                .expect("source end");
            document
                .add_curve(
                    "tangent source",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("tangent source line");
        }
    }
    document
}

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted retained fixture");
    RetainedEditorCoordinator::new(session).expect("retained coordinator")
}

fn scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted fixture");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        Viewport::new([1_000.0, 800.0], [1.0, 1.0], 50.0).expect("viewport"),
        0.25,
    )
    .expect("accepted editor scene")
    .with_retained_session(coordinator.session())
    .expect("authenticated editor scene")
}

fn authoring(regularized: bool, suppressed: bool) -> DraftAuthoringInput {
    DraftAuthoringInput {
        inference: DraftInferenceInput {
            suppressed,
            preferred_candidate: None,
        },
        regularized,
    }
}

fn press(
    coordinator: &mut RetainedEditorCoordinator,
    scene: &EditorScene,
    position: [f64; 2],
    regularized: bool,
    inference_suppressed: bool,
) -> Vec<EditorEffect> {
    coordinator.pointer_down_with_draft_authoring(
        scene,
        PointerInput {
            pointer_id: POINTER_ID,
            position: scene.viewport.model_to_screen(position),
            modifiers: Modifiers::default(),
        },
        authoring(regularized, inference_suppressed),
    )
}

fn terminal_effect(effects: Vec<EditorEffect>) -> EditorEffect {
    let mut terminals = effects.into_iter().filter(|effect| {
        matches!(
            effect,
            EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
        )
    });
    let terminal = terminals.next().expect("one terminal construction effect");
    assert!(terminals.next().is_none(), "only one terminal effect");
    terminal
}

fn action_schema(action: &LineageActionDefinition) -> Option<&str> {
    match action {
        LineageActionDefinition::ImportedBaseline { .. } => None,
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => Some(action.schema.as_str()),
    }
}

fn manifest(
    coordinator: &RetainedEditorCoordinator,
    owner: geosolve_sketch_lineage::LineageStepId,
) -> Manifest {
    let step = coordinator
        .lineage_document()
        .step(owner)
        .expect("lineage owner");
    (
        step.outputs.clone(),
        step.output_identities.clone(),
        step.reservations.clone(),
    )
}

fn assert_cold_matches(coordinator: &RetainedEditorCoordinator, solve: bool) {
    let session_json = coordinator
        .lineage_session_json()
        .expect("canonical lineage session");
    let session = LineageSession::from_session_json(&session_json).expect("strict lineage session");
    assert_eq!(coordinator.lineage_identity(), session.identity());
    assert_eq!(
        coordinator.history_len(),
        session.undo_len() + 1 + session.redo_len(),
        "the public history length must be derived from lineage"
    );
    assert_eq!(
        coordinator.history_cursor(),
        session.undo_len(),
        "the public history cursor must be the lineage Undo depth"
    );
    assert_eq!(coordinator.can_undo(), session.can_undo());
    assert_eq!(coordinator.can_redo(), session.can_redo());
    let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(&session_json)
        .expect("cold structural materialization");
    assert_eq!(
        cold.design_json(),
        coordinator.checkpoint().design_json(),
        "cold sketch materialization"
    );
    assert_eq!(
        cold.feature_json(),
        coordinator.checkpoint().feature_json(),
        "cold feature materialization"
    );
    let map_json =
        RetainedEditorCoordinator::lineage_materialization_map_json_for_session(&session_json)
            .expect("cold ownership map");
    RetainedEditorCoordinator::validate_lineage_materialization_map_json(&session_json, &map_json)
        .expect("authenticated cold ownership map");
    if solve {
        let evidence = evaluate_lineage_session_cold(&session)
            .expect("cold owning-domain materialization must validate");
        assert_eq!(evidence.lineage(), session.identity());
    }
}

fn author_recipe(
    case: RecipeCase,
) -> (
    RetainedEditorCoordinator,
    Vec<geosolve_sketch::DesignPointId>,
) {
    let document = fixture(case.fixture);
    let baseline_points = document
        .points()
        .iter()
        .map(|point| point.id)
        .collect::<Vec<_>>();
    let mut coordinator = coordinator(document);
    let scene = scene(&coordinator);
    let _ = coordinator
        .editor_mut()
        .activate_geometry_tool(case.variant);
    let inference_suppressed = !matches!(case.fixture, StartingFixture::ReusedPoint);

    let effects = match case.completion {
        Completion::FinalClick => {
            for &position in &case.clicks[..case.clicks.len() - 1] {
                let effects = press(
                    &mut coordinator,
                    &scene,
                    position,
                    case.regularized,
                    inference_suppressed,
                );
                assert!(effects.iter().all(|effect| !matches!(
                    effect,
                    EditorEffect::CommitConstruction { .. }
                        | EditorEffect::CommitConstructionPlan { .. }
                )));
            }
            if case.flip_branch {
                let _ = coordinator.editor_mut().flip_geometry_draft_branch();
            }
            press(
                &mut coordinator,
                &scene,
                *case.clicks.last().expect("terminal recipe click"),
                case.regularized,
                inference_suppressed,
            )
        }
        Completion::ExplicitFinish => {
            for &position in case.clicks {
                let effects = press(
                    &mut coordinator,
                    &scene,
                    position,
                    case.regularized,
                    inference_suppressed,
                );
                assert!(effects.iter().all(|effect| !matches!(
                    effect,
                    EditorEffect::CommitConstruction { .. }
                        | EditorEffect::CommitConstructionPlan { .. }
                )));
            }
            coordinator
                .editor_mut()
                .complete_draft(scene.design_identity)
        }
    };
    let outcome = coordinator
        .apply_editor_effect(&terminal_effect(effects))
        .expect("recipe publication")
        .expect("recipe mutation");
    assert!(outcome.published_accepted.is_some());

    (coordinator, baseline_points)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the closed 25-recipe matrix keeps exact authoring, ownership, deletion and history evidence row-local"
)]
fn m83_w1_all_geometry_recipes_materialize_and_keep_identity_through_delete_history() {
    assert_eq!(RECIPE_CASES.len(), GeometryToolVariant::ALL.len());
    assert_eq!(
        RECIPE_CASES
            .iter()
            .map(|case| case.variant)
            .collect::<Vec<_>>(),
        GeometryToolVariant::ALL
    );

    for case in RECIPE_CASES {
        let (mut coordinator, baseline_points) = author_recipe(case);
        let steps = coordinator.lineage_document().steps();
        assert_eq!(steps.len(), 2, "one semantic action for {:?}", case.variant);
        let owner = steps[1].id;
        assert_eq!(
            action_schema(&steps[1].action),
            Some(format!("geosolve.geometry.v1.{}", case.variant.key()).as_str()),
            "exact recipe schema for {:?}",
            case.variant
        );
        assert_eq!(steps[1].state, LineageStepState::Live);
        let owner_manifest = manifest(&coordinator, owner);
        let allocator = coordinator.lineage_document().allocator_high_water();
        let live_design = coordinator.checkpoint().design_json().to_owned();
        let live_map = LineageMaterializationMap::derive(coordinator.lineage_document())
            .expect("live recipe ownership map");
        let live_owned = live_map.owned_outputs(owner).to_vec();
        assert!(
            !live_owned.is_empty(),
            "owned output for {:?}",
            case.variant
        );
        assert!(
            live_owned.iter().all(|output| output.step == owner
                && output.document == coordinator.lineage_document().id())
        );
        if case.variant == GeometryToolVariant::Segment {
            assert!(
                steps[1].output_identities.iter().any(|identity| matches!(
                    identity.flow,
                    LineageOutputIdentityFlow::Aliased { .. }
                )),
                "snapped Segment must retain an explicit point alias"
            );
        }
        let created_curve_count = coordinator
            .session()
            .design_document()
            .curves()
            .iter()
            .filter(|curve| {
                !matches!(case.fixture, StartingFixture::TangentLine)
                    || curve.label != "tangent source"
            })
            .count();
        assert_eq!(
            steps[1]
                .outputs
                .iter()
                .filter(|output| output.kind == LineageOutputKind::GeometryRole)
                .count(),
            created_curve_count,
            "one stable logical geometry-role port per recipe curve for {:?}",
            case.variant
        );
        if matches!(
            case.variant,
            GeometryToolVariant::OpenControlNurbs | GeometryToolVariant::PeriodicControlNurbs
        ) {
            let expected_spans = coordinator
                .session()
                .design_document()
                .curves()
                .iter()
                .flat_map(|curve| match &curve.definition {
                    CurveDefinition::Nurbs { span_ids, .. } => span_ids
                        .iter()
                        .map(|span| format!("curve-span:{}/{span}", curve.id))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>();
            let actual_spans = steps[1]
                .reservations
                .iter()
                .filter(|reservation| reservation.kind == LineageReservationKind::CurveSpan)
                .map(|reservation| reservation.persistent_id.as_str().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(actual_spans, expected_spans, "persistent NURBS span ports");
            assert_eq!(
                steps[1]
                    .outputs
                    .iter()
                    .filter(|output| output.kind == LineageOutputKind::CurveSpan)
                    .count(),
                expected_spans.len()
            );
        }
        assert_cold_matches(&coordinator, true);

        let created_points = coordinator
            .session()
            .design_document()
            .points()
            .iter()
            .filter(|point| !baseline_points.contains(&point.id))
            .map(|point| point.id)
            .collect::<Vec<_>>();
        assert!(
            !created_points.is_empty(),
            "every current recipe owns at least one deletable point: {:?}",
            case.variant
        );
        coordinator
            .editor_mut()
            .set_selection(created_points.into_iter().map(SelectionItem::Point));
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("whole recipe deletion");
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_eq!(
            coordinator
                .lineage_document()
                .step(owner)
                .map(|step| step.state),
            Some(LineageStepState::Tombstoned),
            "whole-step tombstone for {:?}",
            case.variant
        );
        assert_eq!(manifest(&coordinator, owner), owner_manifest);
        assert_eq!(
            coordinator.lineage_document().allocator_high_water(),
            allocator
        );
        assert!(
            LineageMaterializationMap::derive(coordinator.lineage_document())
                .expect("deleted recipe ownership map")
                .live_owned_outputs(owner)
                .is_empty()
        );
        assert_eq!(
            LineageMaterializationMap::derive(coordinator.lineage_document())
                .expect("deleted recipe declarations")
                .owned_outputs(owner),
            live_owned
        );
        assert_cold_matches(&coordinator, false);

        coordinator.undo().expect("restore exact recipe owner");
        assert_eq!(
            coordinator
                .lineage_document()
                .step(owner)
                .map(|step| step.state),
            Some(LineageStepState::Live)
        );
        assert_eq!(manifest(&coordinator, owner), owner_manifest);
        assert_eq!(
            coordinator.lineage_document().allocator_high_water(),
            allocator
        );
        assert_eq!(coordinator.checkpoint().design_json(), live_design);
        assert_eq!(
            LineageMaterializationMap::derive(coordinator.lineage_document())
                .expect("restored recipe ownership map")
                .owned_outputs(owner),
            live_owned
        );
        assert_cold_matches(&coordinator, false);

        coordinator.redo().expect("repeat whole recipe deletion");
        assert_eq!(
            coordinator
                .lineage_document()
                .step(owner)
                .map(|step| step.state),
            Some(LineageStepState::Tombstoned)
        );
        assert_eq!(manifest(&coordinator, owner), owner_manifest);
        assert_eq!(
            coordinator.lineage_document().allocator_high_water(),
            allocator
        );
        assert_cold_matches(&coordinator, false);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one relation/dimension lifecycle regression keeps native source and owner evidence adjacent"
)]
fn m83_w2_relation_and_both_dimension_modes_keep_exact_owners_through_lifecycle() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let first = document.add_point("first", [0.0, 0.0]).expect("first");
    let second = document.add_point("second", [3.0, 1.0]).expect("second");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let mut coordinator = coordinator(document);
    let wrong_kind_before = coordinator
        .lineage_session_json()
        .expect("lineage before rejection");
    assert!(
        coordinator
            .apply_constraint_action_for(
                coordinator.session().design_identity(),
                &[SelectionItem::Point(first)],
                ConstraintActionRequest {
                    intent: ConstraintIntent::Horizontal,
                    label: "wrong-kind horizontal".into(),
                    contacts: Vec::new(),
                    relation: None,
                },
            )
            .is_err()
    );
    assert_eq!(
        coordinator
            .lineage_session_json()
            .expect("neutral rejection"),
        wrong_kind_before
    );

    let constraint = coordinator
        .apply_constraint_action_for(
            coordinator.session().design_identity(),
            &[SelectionItem::Curve(CurveSpan::line(line))],
            ConstraintActionRequest {
                intent: ConstraintIntent::Horizontal,
                label: "lineage horizontal".into(),
                contacts: Vec::new(),
                relation: None,
            },
        )
        .expect("horizontal relation")
        .value;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
    let driving = coordinator
        .add_segment_length_dimension(
            coordinator.session().design_identity(),
            DocumentDimensionMode::Driving,
            "lineage driving length",
        )
        .expect("driving length")
        .value;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
    let reference = coordinator
        .add_segment_length_dimension(
            coordinator.session().design_identity(),
            DocumentDimensionMode::Reference,
            "lineage reference length",
        )
        .expect("reference length")
        .value;

    let schemas = coordinator
        .lineage_document()
        .steps()
        .iter()
        .filter_map(|step| action_schema(&step.action).map(|schema| (schema.to_owned(), step.id)))
        .collect::<Vec<_>>();
    assert_eq!(
        schemas
            .iter()
            .map(|(schema, _)| schema.as_str())
            .collect::<Vec<_>>(),
        [
            "geosolve.constraint.v1.horizontal",
            "geosolve.dimension.v1.curve-length",
            "geosolve.dimension.v1.curve-length",
        ]
    );
    let owners = schemas.iter().map(|(_, owner)| *owner).collect::<Vec<_>>();
    let manifests = owners
        .iter()
        .map(|owner| manifest(&coordinator, *owner))
        .collect::<Vec<_>>();
    let allocator = coordinator.lineage_document().allocator_high_water();
    for owner in &owners {
        assert!(
            !LineageMaterializationMap::derive(coordinator.lineage_document())
                .expect("relation/dimension ownership map")
                .owned_outputs(*owner)
                .is_empty()
        );
    }
    let document = coordinator.session().design_document();
    let constraint_source = document
        .constraint(constraint)
        .expect("constraint")
        .source_id;
    let driving_dimension = document.dimension(driving).expect("driving dimension");
    let reference_dimension = document.dimension(reference).expect("reference dimension");
    assert_eq!(driving_dimension.mode, DocumentDimensionMode::Driving);
    assert_eq!(reference_dimension.mode, DocumentDimensionMode::Reference);
    let DocumentDimensionDefinition::CurveLength {
        target: driving_target,
        ..
    } = driving_dimension.definition
    else {
        panic!("expected curve-length dimension");
    };
    let DocumentDimensionDefinition::CurveLength {
        target: reference_target,
        ..
    } = reference_dimension.definition
    else {
        panic!("expected curve-length dimension");
    };
    let stable_native_ids = (
        constraint,
        constraint_source,
        driving,
        driving_dimension.source_id,
        driving_target,
        reference,
        reference_dimension.source_id,
        reference_target,
    );
    for (owner, expected_kind) in owners.iter().zip([
        LineageOutputKind::Constraint,
        LineageOutputKind::Dimension,
        LineageOutputKind::Dimension,
    ]) {
        let step = coordinator.lineage_document().step(*owner).expect("owner");
        assert!(
            step.outputs
                .iter()
                .any(|output| output.kind == expected_kind)
        );
        assert!(
            step.outputs
                .iter()
                .any(|output| output.kind == LineageOutputKind::Source),
            "constraint/dimension owner must publish its stable audit source"
        );
        assert!(
            step.reservations
                .iter()
                .any(|reservation| reservation.kind == LineageReservationKind::Source)
        );
    }
    assert_cold_matches(&coordinator, true);

    coordinator.editor_mut().set_selection([
        SelectionItem::Constraint(constraint),
        SelectionItem::Dimension(driving),
        SelectionItem::Dimension(reference),
    ]);
    coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), true)
        .expect("suppress relation and dimensions");
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(constraint)
            .expect("suppressed constraint")
            .suppressed
    );
    coordinator.editor_mut().set_selection([
        SelectionItem::Constraint(constraint),
        SelectionItem::Dimension(driving),
        SelectionItem::Dimension(reference),
    ]);
    coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), false)
        .expect("unsuppress relation and dimensions");
    assert_eq!(coordinator.lineage_document().steps().len(), 4);
    for ((owner, expected), actual) in owners
        .iter()
        .zip(&manifests)
        .zip(owners.iter().map(|owner| manifest(&coordinator, *owner)))
    {
        assert_eq!(actual, *expected, "stable owner manifest {owner}");
    }

    coordinator.editor_mut().set_selection([
        SelectionItem::Constraint(constraint),
        SelectionItem::Dimension(driving),
        SelectionItem::Dimension(reference),
    ]);
    coordinator
        .delete_selected(coordinator.session().design_identity())
        .expect("delete relation and both dimension owners");
    assert_eq!(coordinator.lineage_document().steps().len(), 4);
    assert!(owners.iter().all(|owner| {
        coordinator
            .lineage_document()
            .step(*owner)
            .map(|step| step.state)
            == Some(LineageStepState::Tombstoned)
    }));
    assert_cold_matches(&coordinator, false);

    coordinator.undo().expect("restore relation and dimensions");
    assert!(owners.iter().all(|owner| {
        coordinator
            .lineage_document()
            .step(*owner)
            .map(|step| step.state)
            == Some(LineageStepState::Live)
    }));
    let document = coordinator.session().design_document();
    let restored_driving = document.dimension(driving).expect("restored driving");
    let restored_reference = document.dimension(reference).expect("restored reference");
    assert_eq!(
        (
            constraint,
            document
                .constraint(constraint)
                .expect("restored constraint")
                .source_id,
            driving,
            restored_driving.source_id,
            match restored_driving.definition {
                DocumentDimensionDefinition::CurveLength { target, .. } => target,
                _ => panic!("restored driving kind"),
            },
            reference,
            restored_reference.source_id,
            match restored_reference.definition {
                DocumentDimensionDefinition::CurveLength { target, .. } => target,
                _ => panic!("restored reference kind"),
            },
        ),
        stable_native_ids
    );
    assert_eq!(
        coordinator.lineage_document().allocator_high_water(),
        allocator
    );
    assert_cold_matches(&coordinator, false);

    coordinator
        .redo()
        .expect("delete relation and dimensions again");
    assert!(owners.iter().all(|owner| {
        coordinator
            .lineage_document()
            .step(*owner)
            .map(|step| step.state)
            == Some(LineageStepState::Tombstoned)
    }));
    assert_eq!(
        coordinator.lineage_document().allocator_high_water(),
        allocator
    );
    assert_cold_matches(&coordinator, false);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact owner-rewrite regression keeps the three writable arc leaves and history evidence adjacent"
)]
fn m83_w3_curve_property_role_and_branch_rewrite_one_recipe_owner() {
    let case = RECIPE_CASES
        .iter()
        .copied()
        .find(|case| case.variant == GeometryToolVariant::CenterArc)
        .expect("Center Arc recipe case");
    let (mut coordinator, _) = author_recipe(RecipeCase {
        flip_branch: false,
        ..case
    });
    let owner = coordinator.lineage_document().steps()[1].id;
    let owner_manifest = manifest(&coordinator, owner);
    let allocator = coordinator.lineage_document().allocator_high_water();
    let initial_design = coordinator.checkpoint().design_json().to_owned();
    let curve = coordinator.session().design_document().curves()[0].id;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    coordinator
        .set_curve_numeric_property(
            coordinator.session().design_identity(),
            curve,
            CurveNumericPropertyKind::Radius,
            3.0,
        )
        .expect("rewrite recipe-owned radius");
    assert_eq!(
        coordinator.lineage_document().steps().len(),
        2,
        "radius owner"
    );
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    coordinator
        .set_curve_sweep(
            coordinator.session().design_identity(),
            curve,
            DocumentArcSweep::Clockwise,
        )
        .expect("rewrite recipe-owned sweep");
    assert_eq!(
        coordinator.lineage_document().steps().len(),
        2,
        "sweep owner"
    );
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    coordinator
        .toggle_selected_geometry_role(coordinator.session().design_identity())
        .expect("rewrite recipe-owned role");
    assert_eq!(coordinator.lineage_document().steps().len(), 2);
    assert_eq!(manifest(&coordinator, owner), owner_manifest);
    assert!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("recipe owner")
            .outputs
            .iter()
            .any(|output| {
                output.kind == LineageOutputKind::GeometryRole && output.reservation.is_none()
            })
    );
    assert_eq!(
        coordinator.lineage_document().allocator_high_water(),
        allocator
    );
    let edited_design = coordinator.checkpoint().design_json().to_owned();
    let edited = coordinator
        .session()
        .design_document()
        .curve(curve)
        .expect("edited arc");
    let CurveDefinition::CircularArc { radius, sweep, .. } = edited.definition else {
        panic!("expected circular arc");
    };
    assert_eq!(sweep, DocumentArcSweep::Clockwise);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .scalar(radius)
            .expect("edited radius")
            .value
            .to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .geometry_role(curve)
            .expect("edited role"),
        GeometryRole::Construction
    );
    assert_cold_matches(&coordinator, true);

    for _ in 0..3 {
        coordinator.undo().expect("undo owner rewrite");
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_eq!(manifest(&coordinator, owner), owner_manifest);
        assert_eq!(
            coordinator.lineage_document().allocator_high_water(),
            allocator
        );
    }
    assert_eq!(coordinator.checkpoint().design_json(), initial_design);
    assert_cold_matches(&coordinator, false);

    for _ in 0..3 {
        coordinator.redo().expect("redo owner rewrite");
        assert_eq!(coordinator.lineage_document().steps().len(), 2);
        assert_eq!(manifest(&coordinator, owner), owner_manifest);
        assert_eq!(
            coordinator.lineage_document().allocator_high_water(),
            allocator
        );
    }
    assert_eq!(coordinator.checkpoint().design_json(), edited_design);
    assert_cold_matches(&coordinator, false);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one typed association regression keeps declaration, binding, output and history evidence adjacent"
)]
fn m83_typed_parameter_associations_keep_logical_ports_through_cold_undo_redo() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let first = document.add_point("first", [0.0, 0.0]).expect("first");
    let second = document.add_point("second", [3.0, 0.0]).expect("second");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let mut coordinator = coordinator(document);
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
    let driving = coordinator
        .add_segment_length_dimension(
            coordinator.session().design_identity(),
            DocumentDimensionMode::Driving,
            "driving input",
        )
        .expect("driving dimension")
        .value;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(line))]);
    let reference = coordinator
        .add_segment_length_dimension(
            coordinator.session().design_identity(),
            DocumentDimensionMode::Reference,
            "reference output",
        )
        .expect("reference dimension")
        .value;

    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateParameter {
                label: "length input".into(),
                kind: DocumentParameterKind::Length,
            },
        )
        .expect("input parameter");
    let input = coordinator
        .session()
        .design_document()
        .parameters()
        .iter()
        .find(|parameter| parameter.label == "length input")
        .expect("input parameter identity")
        .id;
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::AddParameterBinding {
                parameter: input,
                target: DocumentParameterTarget::DrivingDimension(driving),
            },
        )
        .expect("parameter binding");
    let binding_owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("binding owner")
        .id;
    let binding_manifest = manifest(&coordinator, binding_owner);
    assert!(
        coordinator
            .lineage_document()
            .step(binding_owner)
            .expect("binding step")
            .outputs
            .iter()
            .any(|output| {
                output.kind == LineageOutputKind::ParameterBinding && output.reservation.is_none()
            })
    );

    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::CreateParameter {
                label: "length output".into(),
                kind: DocumentParameterKind::Length,
            },
        )
        .expect("output parameter");
    let output_parameter = coordinator
        .session()
        .design_document()
        .parameters()
        .iter()
        .find(|parameter| parameter.label == "length output")
        .expect("output parameter identity")
        .id;
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::AddParameterOutput {
                parameter: output_parameter,
                dimension: reference,
            },
        )
        .expect("parameter output");
    let output_owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("output owner")
        .id;
    let output_manifest = manifest(&coordinator, output_owner);
    assert!(
        coordinator
            .lineage_document()
            .step(output_owner)
            .expect("output step")
            .outputs
            .iter()
            .any(|output| {
                output.kind == LineageOutputKind::ParameterOutput && output.reservation.is_none()
            })
    );
    assert_cold_matches(&coordinator, false);

    coordinator.undo().expect("undo parameter output");
    assert!(coordinator.lineage_document().step(output_owner).is_none());
    assert_eq!(manifest(&coordinator, binding_owner), binding_manifest);
    assert_cold_matches(&coordinator, false);
    coordinator.redo().expect("redo parameter output");
    assert_eq!(manifest(&coordinator, output_owner), output_manifest);
    assert_cold_matches(&coordinator, false);
}

#[test]
fn m83_host_activation_rewrites_one_typed_logical_owner() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let first = document.add_point("first", [0.0, 0.0]).expect("first");
    let second = document.add_point("second", [3.0, 0.0]).expect("second");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let mut coordinator = coordinator(document);
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetHostConfigurationActivation {
                activation: HostConfigurationActivation::new(
                    1,
                    vec![HostActivationOverride::Inactive(DocumentElementId::Curve(
                        line,
                    ))],
                )
                .expect("activation"),
            },
        )
        .expect("host activation");
    let owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("activation owner")
        .id;
    let owner_manifest = manifest(&coordinator, owner);
    assert!(
        coordinator
            .lineage_document()
            .step(owner)
            .expect("activation step")
            .outputs
            .iter()
            .any(|output| {
                output.kind == LineageOutputKind::Activation && output.reservation.is_none()
            })
    );

    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetHostConfigurationActivation {
                activation: HostConfigurationActivation::new(2, Vec::new())
                    .expect("cleared activation"),
            },
        )
        .expect("rewrite host activation");
    assert_eq!(coordinator.lineage_document().steps().len(), 2);
    assert_eq!(manifest(&coordinator, owner), owner_manifest);
    assert_cold_matches(&coordinator, true);

    coordinator.undo().expect("undo activation rewrite");
    assert_eq!(manifest(&coordinator, owner), owner_manifest);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .host_configuration_activation()
            .expect("restored activation")
            .revision(),
        1
    );
    coordinator.redo().expect("redo activation rewrite");
    assert_eq!(manifest(&coordinator, owner), owner_manifest);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .host_configuration_activation()
            .expect("rewritten activation")
            .revision(),
        2
    );
    assert_cold_matches(&coordinator, false);
}
