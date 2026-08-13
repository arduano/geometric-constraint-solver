// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AuthoringOutcome, AuthoringState, AuthoringTool, ComputedConstructionFragmentId,
    ComputedConstructionFragmentProvenance, ComputedCornerRef, ComputedEvaluationRevision,
    ComputedFeatureCornerId, ComputedFeatureId, ComputedSourceInterval, ConstraintEditor,
    ConstraintIntent, ConstructionPoint, ConstructionProposal, EditorEffect, EditorHoverTarget,
    EditorScene, EditorTool, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    GeometryInteractionPolicy, GeometryPickScope, GeometryRoleSelectionState, GeometryVisibility,
    Modifiers, NativeCurveSpanSource, PickTolerance, PointerInput, RetainedEditorCoordinator,
    SceneCurveOrigin, SceneGeometryHit, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentFilletTrimEndpoint, DocumentObjectId, DocumentSolveRequest, GeometryRole,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_features::ComputedFeatureAuthoringSnapshot;

fn accepted_scene(document: &SketchDocument, viewport: Viewport) -> EditorScene {
    let session = RetainedSketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
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
    .expect("editor scene")
    .with_retained_session(&session)
    .expect("bound editor scene")
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
    role: GeometryRole,
) -> (CurveSpan, [geosolve_sketch::DesignPointId; 2]) {
    let start_id = document
        .add_point(format!("{label} start"), start)
        .expect("point");
    let end_id = document
        .add_point(format!("{label} end"), end)
        .expect("point");
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    let curve = document
        .add_curve_with_role(
            label,
            CurveDefinition::Line {
                start: start_id,
                end: end_id,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
            role,
        )
        .expect("line");
    (CurveSpan::line(curve), [start_id, end_id])
}

fn policy(scope: GeometryPickScope) -> GeometryInteractionPolicy {
    GeometryInteractionPolicy {
        scope,
        visibility: GeometryVisibility::default(),
    }
}

fn policy_with_visibility(
    scope: GeometryPickScope,
    explicit_construction: bool,
    implicit_construction: bool,
) -> GeometryInteractionPolicy {
    GeometryInteractionPolicy {
        scope,
        visibility: GeometryVisibility {
            explicit_construction,
            implicit_construction,
        },
    }
}

fn apply_policy(editor: &mut ConstraintEditor, value: GeometryInteractionPolicy) {
    let _ = editor.set_geometry_pick_scope(value.scope);
    let _ = editor.set_geometry_visibility(value.visibility);
}

fn point_canvas_path(
    scene: &EditorScene,
    point: geosolve_sketch::DesignPointId,
    position: ScreenPoint,
    value: GeometryInteractionPolicy,
) -> (bool, bool, bool) {
    let mut editor = ConstraintEditor::default();
    apply_policy(&mut editor, value);
    let input = PointerInput {
        pointer_id: 41,
        position,
        modifiers: Modifiers::default(),
    };
    let _ = editor.pointer_move(scene, input);
    let hovered = editor.hover_state().target.map(EditorHoverTarget::item)
        == Some(SelectionItem::Point(point));
    let _ = editor.pointer_down(scene, input);
    let selected = editor.selection() == [SelectionItem::Point(point)];
    let drag_effects = editor.pointer_move(
        scene,
        PointerInput {
            position: ScreenPoint {
                x: position.x + 4.0,
                ..position
            },
            ..input
        },
    );
    let dragged = drag_effects.iter().any(|effect| {
        matches!(
            effect,
            EditorEffect::RequestProjectedPointMove { point: candidate, .. } if *candidate == point
        )
    });
    (hovered, selected, dragged)
}

fn curve_canvas_path(
    scene: &EditorScene,
    span: CurveSpan,
    position: ScreenPoint,
    value: GeometryInteractionPolicy,
) -> (bool, bool, Option<SceneCurveOrigin>) {
    let mut editor = ConstraintEditor::default();
    apply_policy(&mut editor, value);
    let input = PointerInput {
        pointer_id: 44,
        position,
        modifiers: Modifiers::default(),
    };
    let _ = editor.pointer_move(scene, input);
    let hovered = editor.hover_state().target.map(EditorHoverTarget::item)
        == Some(SelectionItem::Curve(span));
    let _ = editor.pointer_down(scene, input);
    (
        hovered,
        editor.selection() == [SelectionItem::Curve(span)],
        editor.curve_pick_origin(span),
    )
}

fn line_start_snap(
    scene: &EditorScene,
    position: ScreenPoint,
    value: GeometryInteractionPolicy,
) -> ConstructionPoint {
    let mut editor = ConstraintEditor::default();
    apply_policy(&mut editor, value);
    let _ = editor.activate_tool(EditorTool::Line);
    let _ = editor.pointer_down(
        scene,
        PointerInput {
            pointer_id: 42,
            position,
            modifiers: Modifiers::default(),
        },
    );
    let effects = editor.pointer_down(
        scene,
        PointerInput {
            pointer_id: 42,
            position: ScreenPoint {
                x: position.x + 30.0,
                y: position.y + 30.0,
            },
            modifiers: Modifiers::default(),
        },
    );
    let (start, token) = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstruction {
                proposal: ConstructionProposal::Line { start, .. },
                ..
            } => Some((*start, None)),
            EditorEffect::CommitConstructionPlan { token, plan, .. } => match &plan.proposal {
                ConstructionProposal::Line { start, .. } => Some((*start, Some(*token))),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or_else(|| panic!("line draft should commit after its second point: {effects:?}"));
    if let Some(token) = token {
        assert!(
            editor
                .acknowledge_construction_commit(token, true)
                .iter()
                .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
        );
    }
    start
}

fn assert_point_scope_matrix(
    scene: &EditorScene,
    point: geosolve_sketch::DesignPointId,
    position: ScreenPoint,
    role: GeometryRole,
) {
    for scope in [
        GeometryPickScope::All,
        GeometryPickScope::Profile,
        GeometryPickScope::Construction,
    ] {
        let expected = scope == GeometryPickScope::All
            || matches!(
                (scope, role),
                (GeometryPickScope::Profile, GeometryRole::Profile)
                    | (GeometryPickScope::Construction, GeometryRole::Construction)
            );
        assert_eq!(
            point_canvas_path(scene, point, position, policy(scope)),
            (expected, expected, expected),
            "hover, selection and drag ownership diverged for {role:?} in {scope:?}"
        );
        let snap = line_start_snap(scene, position, policy(scope));
        assert_eq!(
            matches!(snap, ConstructionPoint::Existing { id, .. } if id == point),
            expected,
            "snap admission diverged for {role:?} in {scope:?}"
        );
    }
}

fn horizontal_authoring_pick(
    document: &SketchDocument,
    scene: &EditorScene,
    position: ScreenPoint,
    value: GeometryInteractionPolicy,
) -> HorizontalAuthoringPick {
    let mut authoring = AuthoringState::default();
    assert!(matches!(
        authoring.activate(
            document,
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[],
        ),
        AuthoringOutcome::ModeEntered { .. }
    ));
    match authoring.pick_at_with_policy(document, scene, position, PickTolerance::default(), value)
    {
        AuthoringOutcome::Apply(application) => match application.operands[0].item {
            SelectionItem::Curve(span) => HorizontalAuthoringPick::Applied(span),
            item => panic!("Horizontal resolved a non-curve operand: {item:?}"),
        },
        AuthoringOutcome::Collecting { .. } => HorizontalAuthoringPick::PointPrefix,
        AuthoringOutcome::Warning(_) => HorizontalAuthoringPick::Unavailable,
        outcome => panic!("unexpected Horizontal authoring outcome: {outcome:?}"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HorizontalAuthoringPick {
    Applied(CurveSpan),
    PointPrefix,
    Unavailable,
}

struct ImplicitFragmentFixture {
    document: SketchDocument,
    scene: EditorScene,
    span: CurveSpan,
    position: ScreenPoint,
    origin: SceneCurveOrigin,
}

fn implicit_fragment_fixture() -> ImplicitFragmentFixture {
    let mut document = SketchDocument::new(1.0).expect("document");
    let (span, _) = line(
        &mut document,
        "source",
        [0.0, 0.0],
        [4.0, 0.0],
        GeometryRole::Profile,
    );
    let viewport = Viewport::new([800.0, 500.0], [2.0, 0.0], 50.0).expect("viewport");
    let mut scene = accepted_scene(&document, viewport);
    let interval = ComputedSourceInterval {
        start: 0.5,
        end: 1.0,
    };
    let origin = SceneCurveOrigin::FilletDiscarded {
        fragment: ComputedConstructionFragmentId {
            evaluation: ComputedEvaluationRevision::from_raw(3),
            ordinal: 0,
        },
        source: NativeCurveSpanSource { span },
        interval,
        provenance: ComputedConstructionFragmentProvenance {
            owner: ComputedCornerRef {
                feature: ComputedFeatureId::from_raw(1),
                corner: ComputedFeatureCornerId::from_raw(2),
            },
            endpoint: DocumentFilletTrimEndpoint::End,
            base_interval: ComputedSourceInterval {
                start: 0.0,
                end: 1.0,
            },
        },
    };
    let native = scene.curves.first_mut().expect("native source");
    let mut discarded = native.clone();
    native.screen_polyline = vec![
        viewport.model_to_screen([0.0, 0.0]),
        viewport.model_to_screen([2.0, 0.0]),
    ];
    native.screen_parameters = vec![0.0, 0.5];
    discarded.screen_polyline = vec![
        viewport.model_to_screen([2.0, 0.0]),
        viewport.model_to_screen([4.0, 0.0]),
    ];
    discarded.screen_parameters = vec![0.5, 1.0];
    discarded.role = GeometryRole::Construction;
    discarded.source_role = GeometryRole::Profile;
    discarded.origin = origin;
    scene.curves.push(discarded);
    ImplicitFragmentFixture {
        document,
        scene,
        span,
        position: viewport.model_to_screen([3.0, 0.0]),
        origin,
    }
}

struct FilletAuthoringFixture {
    snapshot: ComputedFeatureAuthoringSnapshot,
    document: SketchDocument,
    scene: EditorScene,
    viewport: Viewport,
}

fn fillet_authoring_fixture(source_role: GeometryRole, implicit: bool) -> FilletAuthoringFixture {
    let mut document = SketchDocument::new(1.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("point");
    let corner = document.add_point("corner", [2.0, 0.0]).expect("point");
    let end = document.add_point("end", [2.0, 2.0]).expect("point");
    let first = document
        .add_curve_with_role(
            "first",
            CurveDefinition::Line {
                start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
            source_role,
        )
        .expect("curve");
    let second = document
        .add_curve_with_role(
            "second",
            CurveDefinition::Line {
                start: corner,
                end,
                branch_direction: [0.0, 1.0],
            },
            source_role,
        )
        .expect("curve");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("authoring snapshot");
    let accepted = snapshot.sketch_document().clone();
    let viewport = Viewport::new([800.0, 500.0], [1.0, 1.0], 50.0).expect("viewport");
    let mut scene = EditorScene::from_accepted_for_design(
        snapshot.accepted_state_identity().revision().get(),
        snapshot.sketch_input().design_identity(),
        &accepted,
        coordinator.session().design_document(),
        viewport,
        0.25,
    )
    .expect("scene");
    if implicit {
        for (ordinal, span) in [CurveSpan::line(first), CurveSpan::line(second)]
            .into_iter()
            .enumerate()
        {
            let curve = scene
                .curves
                .iter_mut()
                .find(|curve| curve.span == span)
                .expect("source curve");
            curve.role = GeometryRole::Construction;
            curve.origin = SceneCurveOrigin::FilletDiscarded {
                fragment: ComputedConstructionFragmentId {
                    evaluation: ComputedEvaluationRevision::from_raw(8),
                    ordinal: u32::try_from(ordinal).expect("bounded ordinal"),
                },
                source: NativeCurveSpanSource { span },
                interval: ComputedSourceInterval {
                    start: 0.0,
                    end: 1.0,
                },
                provenance: ComputedConstructionFragmentProvenance {
                    owner: ComputedCornerRef {
                        feature: ComputedFeatureId::from_raw(7),
                        corner: ComputedFeatureCornerId::from_raw(
                            u64::try_from(ordinal + 1).expect("bounded corner"),
                        ),
                    },
                    endpoint: DocumentFilletTrimEndpoint::End,
                    base_interval: ComputedSourceInterval {
                        start: 0.0,
                        end: 1.0,
                    },
                },
            };
        }
    }
    FilletAuthoringFixture {
        snapshot,
        document: accepted,
        scene,
        viewport,
    }
}

fn fillet_authoring_completes(
    source_role: GeometryRole,
    implicit: bool,
    value: GeometryInteractionPolicy,
) -> bool {
    let FilletAuthoringFixture {
        snapshot,
        document,
        scene,
        viewport,
    } = fillet_authoring_fixture(source_role, implicit);
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    let first = state.pick_at_with_policy(
        &snapshot,
        &document,
        &scene,
        viewport.model_to_screen([1.0, 0.0]),
        PickTolerance::default(),
        value,
    );
    if !matches!(first, FeatureAuthoringOutcome::Collecting { .. }) {
        return false;
    }
    matches!(
        state.pick_at_with_policy(
            &snapshot,
            &document,
            &scene,
            viewport.model_to_screen([2.0, 1.0]),
            PickTolerance::default(),
            value,
        ),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    )
}

fn assert_display_visibility_is_scope_independent(
    profile: &geosolve_constraint_editor::SceneCurve,
    construction: &geosolve_constraint_editor::SceneCurve,
) {
    assert!(construction.is_visible(policy(GeometryPickScope::Profile)));
    assert!(profile.is_visible(policy(GeometryPickScope::Construction)));
    assert!(!construction.is_interactive(policy(GeometryPickScope::Profile)));
    assert!(!profile.is_interactive(policy(GeometryPickScope::Construction)));
    let hidden = GeometryInteractionPolicy {
        scope: GeometryPickScope::All,
        visibility: GeometryVisibility {
            explicit_construction: false,
            implicit_construction: true,
        },
    };
    assert!(!construction.is_visible(hidden));
    assert!(profile.is_visible(hidden));
}

#[test]
fn scene_roles_and_shared_point_incidence_drive_all_three_pick_scopes() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let profile_start = document
        .add_point("profile start", [-2.0, 0.0])
        .expect("point");
    let shared = document.add_point("shared", [0.0, 0.0]).expect("point");
    let construction_end = document.add_point("guide end", [2.0, 0.0]).expect("point");
    let profile = document
        .add_curve_with_role(
            "profile",
            CurveDefinition::Line {
                start: profile_start,
                end: shared,
                branch_direction: [1.0, 0.0],
            },
            GeometryRole::Profile,
        )
        .expect("profile line");
    let construction = document
        .add_curve_with_role(
            "guide",
            CurveDefinition::Line {
                start: shared,
                end: construction_end,
                branch_direction: [1.0, 0.0],
            },
            GeometryRole::Construction,
        )
        .expect("construction line");
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    let scene = accepted_scene(&document, viewport);

    let profile_curve = scene
        .curves
        .iter()
        .find(|curve| curve.span == CurveSpan::line(profile))
        .expect("profile scene curve");
    assert_eq!(profile_curve.role, GeometryRole::Profile);
    let construction_curve = scene
        .curves
        .iter()
        .find(|curve| curve.span == CurveSpan::line(construction))
        .expect("construction scene curve");
    assert_eq!(construction_curve.role, GeometryRole::Construction);
    assert_eq!(construction_curve.source_role, GeometryRole::Construction);
    assert_eq!(construction_curve.origin, SceneCurveOrigin::Native);
    assert_display_visibility_is_scope_independent(profile_curve, construction_curve);

    let shared_point = scene
        .points
        .iter()
        .find(|point| point.id == shared)
        .expect("shared scene point");
    assert!(shared_point.role_incidence.profile);
    assert!(shared_point.role_incidence.construction);
    let guide_point = scene
        .points
        .iter()
        .find(|point| point.id == construction_end)
        .expect("construction-only point");
    assert!(!guide_point.role_incidence.profile);
    assert!(guide_point.role_incidence.construction);

    let shared_screen = viewport.model_to_screen([0.0, 0.0]);
    for scope in [GeometryPickScope::Profile, GeometryPickScope::Construction] {
        assert_eq!(
            scene
                .hit_test_with_policy(shared_screen, PickTolerance::default(), policy(scope))
                .map(|hit| hit.item),
            Some(SelectionItem::Point(shared))
        );
    }
    let guide_screen = viewport.model_to_screen([2.0, 0.0]);
    assert!(
        scene
            .hit_test_with_policy(
                guide_screen,
                PickTolerance::default(),
                policy(GeometryPickScope::Profile),
            )
            .is_none()
    );
    assert_eq!(
        scene
            .hit_test_with_policy(
                guide_screen,
                PickTolerance::default(),
                policy(GeometryPickScope::Construction),
            )
            .map(|hit| hit.item),
        Some(SelectionItem::Point(construction_end))
    );
    let hidden = GeometryInteractionPolicy {
        scope: GeometryPickScope::All,
        visibility: GeometryVisibility {
            explicit_construction: false,
            implicit_construction: true,
        },
    };
    assert!(
        scene
            .hit_test_with_policy(guide_screen, PickTolerance::default(), hidden)
            .is_none()
    );
}

#[test]
fn free_profile_and_explicit_construction_points_share_one_canvas_path_matrix() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let free = document.add_point("free", [-3.0, 0.0]).expect("point");
    let (_, guide_points) = line(
        &mut document,
        "guide",
        [3.0, 0.0],
        [5.0, 0.0],
        GeometryRole::Construction,
    );
    let guide = guide_points[0];
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    let scene = accepted_scene(&document, viewport);
    let free_scene = scene
        .points
        .iter()
        .find(|point| point.id == free)
        .expect("free point scene");
    assert_eq!(
        free_scene.role_incidence,
        geosolve_constraint_editor::ScenePointRoleIncidence {
            profile: true,
            construction: false,
        },
        "a role-neutral free point participates as Profile without persistent point role state"
    );
    assert!(free_scene.is_interactive(policy(GeometryPickScope::Profile)));
    assert!(!free_scene.is_interactive(policy(GeometryPickScope::Construction)));
    assert_point_scope_matrix(
        &scene,
        free,
        viewport.model_to_screen([-3.0, 0.0]),
        GeometryRole::Profile,
    );
    assert_point_scope_matrix(
        &scene,
        guide,
        viewport.model_to_screen([3.0, 0.0]),
        GeometryRole::Construction,
    );

    let hidden = policy_with_visibility(GeometryPickScope::All, false, true);
    let guide_position = viewport.model_to_screen([3.0, 0.0]);
    assert_eq!(
        point_canvas_path(&scene, guide, guide_position, hidden),
        (false, false, false)
    );
    assert!(matches!(
        line_start_snap(&scene, guide_position, hidden),
        ConstructionPoint::New(_)
    ));

    let mut editor = ConstraintEditor::default();
    let down = PointerInput {
        pointer_id: 43,
        position: guide_position,
        modifiers: Modifiers::default(),
    };
    let _ = editor.pointer_down(&scene, down);
    assert!(editor.pointer_move(
        &scene,
        PointerInput {
            position: ScreenPoint {
                x: guide_position.x + 4.0,
                ..guide_position
            },
            ..down
        }
    ).iter().any(|effect| matches!(effect, EditorEffect::RequestProjectedPointMove { point, .. } if *point == guide)));
    assert_eq!(
        editor.set_geometry_pick_scope(GeometryPickScope::Profile),
        vec![EditorEffect::ClearPointPreview]
    );
    assert!(editor.active_pointer_gesture().is_none());
    assert_eq!(editor.selection(), [SelectionItem::Point(guide)]);
}

#[test]
fn all_scope_prefers_profile_only_inside_the_one_pixel_cross_role_band() {
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    for (separation, expected_role) in [
        (0.000, GeometryRole::Profile),
        (0.018, GeometryRole::Profile),
        (0.040, GeometryRole::Construction),
    ] {
        let mut document = SketchDocument::new(1.0).expect("document");
        let (profile, _) = line(
            &mut document,
            "profile",
            [-4.0, 0.0],
            [4.0, 0.0],
            GeometryRole::Profile,
        );
        let (construction, _) = line(
            &mut document,
            "construction",
            [-4.0, separation],
            [4.0, separation],
            GeometryRole::Construction,
        );
        let scene = accepted_scene(&document, viewport);
        let hit = scene
            .hit_test_with_policy(
                viewport.model_to_screen([0.0, separation]),
                PickTolerance::default(),
                policy(GeometryPickScope::All),
            )
            .expect("overlapping line hit");
        let expected = match expected_role {
            GeometryRole::Profile => profile,
            GeometryRole::Construction => construction,
        };
        assert_eq!(hit.item, SelectionItem::Curve(expected));
    }
}

#[test]
fn implicit_fillet_fragment_hit_retains_provenance_and_selects_its_native_source() {
    let fixture = implicit_fragment_fixture();
    let hit = fixture
        .scene
        .hit_test_with_policy(
            fixture.position,
            PickTolerance::default(),
            policy(GeometryPickScope::Construction),
        )
        .expect("implicit construction hit");
    assert_eq!(hit.item, SelectionItem::Curve(fixture.span));
    assert_eq!(hit.curve_parameter, Some(0.75));
    assert!(matches!(
        hit.geometry,
        Some(SceneGeometryHit::NativeCurve {
            role: GeometryRole::Construction,
            source_role: GeometryRole::Profile,
            origin: picked,
        }) if picked == fixture.origin
    ));
    for (scope, expected) in [
        (GeometryPickScope::All, true),
        (GeometryPickScope::Profile, false),
        (GeometryPickScope::Construction, true),
    ] {
        assert_eq!(
            curve_canvas_path(
                &fixture.scene,
                fixture.span,
                fixture.position,
                policy(scope),
            ),
            (expected, expected, expected.then_some(fixture.origin),)
        );
        assert_eq!(
            horizontal_authoring_pick(
                &fixture.document,
                &fixture.scene,
                fixture.position,
                policy(scope),
            ),
            if expected {
                HorizontalAuthoringPick::Applied(fixture.span)
            } else {
                HorizontalAuthoringPick::Unavailable
            }
        );
    }
    let hidden = policy_with_visibility(GeometryPickScope::Construction, true, false);
    assert_eq!(
        curve_canvas_path(&fixture.scene, fixture.span, fixture.position, hidden,),
        (false, false, None)
    );
    assert_eq!(
        horizontal_authoring_pick(&fixture.document, &fixture.scene, fixture.position, hidden,),
        HorizontalAuthoringPick::Unavailable
    );
}

#[test]
fn implicit_origin_selection_routes_dimension_and_delete_to_the_complete_native_curve() {
    let fixture = implicit_fragment_fixture();
    let selected_coordinator = || {
        let session = RetainedSketchDocumentSession::new(
            fixture.document.clone(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let _ = coordinator
            .editor_mut()
            .set_geometry_pick_scope(GeometryPickScope::Construction);
        let _ = coordinator.editor_mut().pointer_down(
            &fixture.scene,
            PointerInput {
                pointer_id: 45,
                position: fixture.position,
                modifiers: Modifiers::default(),
            },
        );
        assert_eq!(
            coordinator.editor().selection(),
            [SelectionItem::Curve(fixture.span)]
        );
        assert_eq!(
            coordinator.editor().curve_pick_origin(fixture.span),
            Some(fixture.origin)
        );
        coordinator
    };

    let mut dimension_coordinator = selected_coordinator();
    let expected = dimension_coordinator.session().design_identity();
    let dimension = dimension_coordinator
        .add_selected_dimension(expected, DocumentDimensionMode::Reference, "source length")
        .expect("implicit-origin selection admits the native span dimension")
        .value;
    assert!(matches!(
        dimension_coordinator
            .session()
            .design_document()
            .dimension(dimension)
            .expect("created dimension")
            .definition,
        DocumentDimensionDefinition::CurveLength { curve, .. } if curve == fixture.span
    ));

    let mut delete_coordinator = selected_coordinator();
    let expected = delete_coordinator.session().design_identity();
    let deleted = delete_coordinator
        .delete_selected(expected)
        .expect("implicit-origin selection deletes the native source")
        .value;
    assert!(deleted.contains(&DocumentObjectId::Curve(fixture.span.curve)));
    assert!(
        delete_coordinator
            .session()
            .design_document()
            .curve(fixture.span.curve)
            .is_none()
    );
}

#[test]
fn compatibility_aware_constraint_authoring_obeys_role_and_visibility_matrix() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let (profile, _) = line(
        &mut document,
        "profile",
        [-2.0, -1.0],
        [2.0, -1.0],
        GeometryRole::Profile,
    );
    let (construction, _) = line(
        &mut document,
        "construction",
        [-2.0, 1.0],
        [2.0, 1.0],
        GeometryRole::Construction,
    );
    let profile_point = document
        .add_point("profile overlap", [0.0, -1.0])
        .expect("point");
    let construction_point = document
        .add_point("construction overlap", [0.0, 1.0])
        .expect("point");
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    let scene = accepted_scene(&document, viewport);
    for (span, point, model_position, role, excluded_pick) in [
        (
            profile,
            profile_point,
            [0.0, -1.0],
            GeometryRole::Profile,
            HorizontalAuthoringPick::Unavailable,
        ),
        (
            construction,
            construction_point,
            [0.0, 1.0],
            GeometryRole::Construction,
            HorizontalAuthoringPick::PointPrefix,
        ),
    ] {
        let position = viewport.model_to_screen(model_position);
        assert_eq!(
            scene
                .native_authoring_hit_test(position, PickTolerance::default())
                .map(|hit| hit.item),
            Some(SelectionItem::Point(point)),
            "the compatibility fallback fixture requires the wrong-kind point to win raw picking"
        );
        for scope in [
            GeometryPickScope::All,
            GeometryPickScope::Profile,
            GeometryPickScope::Construction,
        ] {
            let expected = scope == GeometryPickScope::All
                || matches!(
                    (scope, role),
                    (GeometryPickScope::Profile, GeometryRole::Profile)
                        | (GeometryPickScope::Construction, GeometryRole::Construction)
                );
            assert_eq!(
                horizontal_authoring_pick(&document, &scene, position, policy(scope)),
                if expected {
                    HorizontalAuthoringPick::Applied(span)
                } else {
                    excluded_pick
                },
                "Horizontal authoring path diverged for {role:?} in {scope:?}"
            );
        }
    }
    assert_eq!(
        horizontal_authoring_pick(
            &document,
            &scene,
            viewport.model_to_screen([0.0, 1.0]),
            policy_with_visibility(GeometryPickScope::All, false, true),
        ),
        HorizontalAuthoringPick::PointPrefix
    );
}

#[test]
fn computed_fillet_authoring_obeys_profile_explicit_and_implicit_scope_matrix() {
    for (role, scope, expected) in [
        (GeometryRole::Profile, GeometryPickScope::All, true),
        (GeometryRole::Profile, GeometryPickScope::Profile, true),
        (
            GeometryRole::Profile,
            GeometryPickScope::Construction,
            false,
        ),
        (GeometryRole::Construction, GeometryPickScope::All, true),
        (
            GeometryRole::Construction,
            GeometryPickScope::Profile,
            false,
        ),
        (
            GeometryRole::Construction,
            GeometryPickScope::Construction,
            true,
        ),
    ] {
        assert_eq!(
            fillet_authoring_completes(role, false, policy(scope)),
            expected,
            "explicit {role:?} Fillet authoring diverged in {scope:?}"
        );
    }
    assert!(!fillet_authoring_completes(
        GeometryRole::Construction,
        false,
        policy_with_visibility(GeometryPickScope::All, false, true),
    ));

    for (scope, expected) in [
        (GeometryPickScope::All, true),
        (GeometryPickScope::Profile, false),
        (GeometryPickScope::Construction, true),
    ] {
        assert_eq!(
            fillet_authoring_completes(GeometryRole::Profile, true, policy(scope)),
            expected,
            "implicit Construction Fillet authoring diverged in {scope:?}"
        );
    }
    assert!(!fillet_authoring_completes(
        GeometryRole::Profile,
        true,
        policy_with_visibility(GeometryPickScope::Construction, true, false),
    ));
}

#[test]
fn role_aware_construction_and_selected_curve_toggle_are_atomic_and_undoable() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let proposal = ConstructionProposal::Line {
        start: ConstructionPoint::New([0.0, 0.0]),
        end: ConstructionPoint::New([2.0, 0.0]),
    };
    let first = proposal
        .apply_with_role(&mut document, GeometryRole::Construction)
        .expect("construction line")
        .curves[0];
    assert_eq!(
        document.geometry_role(first),
        Some(GeometryRole::Construction)
    );
    let second = ConstructionProposal::Line {
        start: ConstructionPoint::New([0.0, 1.0]),
        end: ConstructionPoint::New([2.0, 1.0]),
    }
    .apply(&mut document)
    .expect("profile line")
    .curves[0];
    assert_eq!(document.geometry_role(second), Some(GeometryRole::Profile));

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().set_selection([
        SelectionItem::Curve(CurveSpan::line(first)),
        SelectionItem::Curve(CurveSpan::line(second)),
    ]);
    assert_eq!(
        coordinator.selected_geometry_role_state(),
        Some(GeometryRoleSelectionState::Mixed)
    );
    let expected = coordinator.session().design_identity();
    coordinator
        .toggle_selected_geometry_role(expected)
        .expect("one atomic toggle");
    assert_eq!(
        coordinator.session().design_document().geometry_role(first),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .geometry_role(second),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        coordinator.selected_geometry_role_state(),
        Some(GeometryRoleSelectionState::Construction)
    );
    let expected = coordinator.session().design_identity();
    coordinator
        .toggle_selected_geometry_role(expected)
        .expect("all-Construction selection toggles to Profile");
    assert_eq!(
        coordinator.session().design_document().geometry_role(first),
        Some(GeometryRole::Profile)
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .geometry_role(second),
        Some(GeometryRole::Profile)
    );
    assert_eq!(
        coordinator.selected_geometry_role_state(),
        Some(GeometryRoleSelectionState::Profile)
    );
    coordinator.undo().expect("undo role batch");
    assert_eq!(
        coordinator.session().design_document().geometry_role(first),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .geometry_role(second),
        Some(GeometryRole::Construction)
    );
    coordinator.undo().expect("undo mixed role batch");
    assert_eq!(
        coordinator.session().design_document().geometry_role(first),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .geometry_role(second),
        Some(GeometryRole::Profile)
    );
}

#[test]
fn drawing_role_is_frozen_into_the_commit_effect() {
    let document = SketchDocument::new(1.0).expect("document");
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    let scene = accepted_scene(&document, viewport);
    let mut editor = ConstraintEditor::default();
    editor.set_authoring_geometry_role(GeometryRole::Construction);
    editor.activate_tool(EditorTool::Line);
    let first = viewport.model_to_screen([-1.0, 0.0]);
    let second = viewport.model_to_screen([1.0, 0.0]);
    let _ = editor.pointer_down(
        &scene,
        PointerInput {
            pointer_id: 9,
            position: first,
            modifiers: Modifiers::default(),
        },
    );
    let effects = editor.pointer_down(
        &scene,
        PointerInput {
            pointer_id: 9,
            position: second,
            modifiers: Modifiers::default(),
        },
    );
    let token = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { token, plan, .. }
                if plan.role == GeometryRole::Construction =>
            {
                Some(*token)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("construction-role plan expected: {effects:?}"));
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
    assert!(
        editor
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
}

#[test]
fn geometry_policy_change_cancels_an_incomplete_geometry_draft() {
    let document = SketchDocument::new(1.0).expect("document");
    let viewport = Viewport::new([800.0, 500.0], [0.0, 0.0], 50.0).expect("viewport");
    let scene = accepted_scene(&document, viewport);
    let mut editor = ConstraintEditor::default();
    let _ = editor.activate_tool(EditorTool::Line);
    let first = viewport.model_to_screen([-1.0, 0.0]);
    let second = viewport.model_to_screen([1.0, 0.0]);
    let first_effects = editor.pointer_down(
        &scene,
        PointerInput {
            pointer_id: 10,
            position: first,
            modifiers: Modifiers::default(),
        },
    );
    assert!(first_effects.is_empty());
    assert_eq!(
        editor.set_geometry_pick_scope(GeometryPickScope::Profile),
        vec![EditorEffect::ClearConstructionPreview]
    );
    assert!(
        editor
            .pointer_down(
                &scene,
                PointerInput {
                    pointer_id: 10,
                    position: second,
                    modifiers: Modifiers::default(),
                },
            )
            .is_empty()
    );
    let effects = editor.pointer_down(
        &scene,
        PointerInput {
            pointer_id: 10,
            position: first,
            modifiers: Modifiers::default(),
        },
    );
    let token = effects
        .iter()
        .find_map(|effect| match effect {
            EditorEffect::CommitConstructionPlan { token, .. } => Some(*token),
            _ => None,
        })
        .unwrap_or_else(|| panic!("restarted draft should publish one plan: {effects:?}"));
    assert!(
        editor
            .acknowledge_construction_commit(token, true)
            .iter()
            .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
    );
}
