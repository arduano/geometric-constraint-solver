// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    EditorEffect, EditorScene, EditorTool, Modifiers, PickTolerance, PointerInput,
    ProjectedDragWorkEvidence, RetainedEditorCoordinator, SelectionItem, Viewport,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactId, ContactSlot, CurveDefinition, CurveId,
    CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentDragLocalityPlan, MotionCamIds,
    MotionPantographIds, RetainedSketchDocumentSession, SketchAcceptedStateIdentity,
    SketchDesignIdentity, SketchDocument, SolverConfig, alpha_scenario,
};

const POSITION_TOLERANCE: f64 = 1.0e-8;

struct GestureCapture {
    baseline_design: SketchDesignIdentity,
    baseline_accepted: SketchAcceptedStateIdentity,
    published_design: SketchDesignIdentity,
    published_accepted: SketchAcceptedStateIdentity,
    locality: DocumentDragLocalityPlan,
    published_document: SketchDocument,
    accepted_json: Option<String>,
}

fn pointer(pointer_id: u64, position: geosolve_constraint_editor::ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn accepted_scene(coordinator: &RetainedEditorCoordinator, viewport: Viewport) -> EditorScene {
    let accepted = coordinator
        .session()
        .accepted_state()
        .expect("accepted state");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        coordinator.session().design_document(),
        viewport,
        0.25,
    )
    .expect("accepted editor scene")
}

fn assert_position_near(actual: [f64; 2], expected: [f64; 2], tolerance: f64, context: &str) {
    assert!(
        (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= tolerance,
        "{context}: expected={expected:?}, actual={actual:?}"
    );
}

fn assert_controlled_work_bounded(work: &ProjectedDragWorkEvidence) {
    let report = &work.operation;
    macro_rules! within {
        ($field:ident) => {
            assert!(
                report.consumed.$field <= report.configured.$field,
                "{} consumed {} above configured {}: {work:#?}",
                stringify!($field),
                report.consumed.$field,
                report.configured.$field,
            );
        };
    }

    // Every additive solve/rank/diagnostic counter used by ordinary pointer
    // publication has a finite synchronous-WASM ceiling.
    within!(document_validation_items);
    within!(document_dependency_items);
    within!(document_lowering_items);
    within!(nonlinear_iterations);
    within!(rejected_trials);
    within!(component_linearizations);
    within!(dense_kernel_work_units);
    within!(factorizations);
    within!(rank_kernels);
    within!(diagnostic_candidates);
    within!(diagnostic_trials);
    assert_ne!(report.configured.rank_kernels, usize::MAX);
    assert_ne!(report.configured.diagnostic_candidates, usize::MAX);
    assert_ne!(report.configured.diagnostic_trials, usize::MAX);
}

fn resolve_pointer_sample(
    coordinator: &mut RetainedEditorCoordinator,
    scene: &EditorScene,
    input: PointerInput,
) -> (
    DesignPointId,
    [f64; 2],
    Vec<EditorEffect>,
    ProjectedDragWorkEvidence,
) {
    let request = coordinator.editor_mut().pointer_move(scene, input);
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id,
            request_id,
            point,
            model_position,
        },
    ] = request.as_slice()
    else {
        panic!("one projected pointer request: {request:#?}");
    };
    assert_eq!(*pointer_id, input.pointer_id);
    let result =
        coordinator.resolve_projected_point_move(*pointer_id, *request_id, *point, *model_position);
    let work = coordinator
        .projected_drag_work_evidence()
        .expect("projected pointer work")
        .clone();
    (*point, *model_position, result, work)
}

fn dispatch_release(coordinator: &mut RetainedEditorCoordinator, effects: &[EditorEffect]) {
    assert_eq!(effects.len(), 2, "release effects: {effects:#?}");
    coordinator
        .apply_editor_effect(&effects[0])
        .expect("dispatch point release")
        .expect("retained point mutation");
    assert!(matches!(effects[1], EditorEffect::ClearPointPreview));
    assert!(
        coordinator
            .apply_editor_effect(&effects[1])
            .expect("dispatch release disposition")
            .is_none()
    );
}

#[derive(Clone)]
struct PassiveMotionCamState {
    center: [f64; 2],
    contact_metadata: Vec<ContactSlot>,
    contact_ids: [ContactId; 2],
    contact_parameters: [f64; 2],
    curve_definitions: [CurveDefinition; 3],
}

fn capture_passive_motion_cam_state(
    document: &SketchDocument,
    ids: MotionCamIds,
    passive_center: DesignPointId,
    passive_circle: CurveId,
) -> PassiveMotionCamState {
    let contact_ids = document
        .constraints()
        .iter()
        .find_map(|constraint| {
            let DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            } = constraint.definition
            else {
                return None;
            };
            let contacts = [first_contact, second_contact];
            contacts
                .iter()
                .any(|contact| {
                    document
                        .contact(*contact)
                        .is_some_and(|slot| slot.curve.curve == passive_circle)
                })
                .then_some(contacts)
        })
        .expect("passive roller tangency contacts");
    let contact_parameters = contact_ids.map(|contact| {
        document
            .scalar(
                document
                    .contact(contact)
                    .expect("passive contact")
                    .parameter,
            )
            .expect("passive contact parameter")
            .value
    });
    PassiveMotionCamState {
        center: document
            .point(passive_center)
            .expect("passive roller center")
            .position,
        contact_metadata: document.contacts().to_vec(),
        contact_ids,
        contact_parameters,
        curve_definitions: [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
            document
                .curve(curve)
                .expect("motion-cam curve")
                .definition
                .clone()
        }),
    }
}

fn assert_passive_motion_cam_state(
    document: &SketchDocument,
    ids: MotionCamIds,
    passive_center: DesignPointId,
    expected: &PassiveMotionCamState,
    context: &str,
) {
    assert_position_near(
        document
            .point(passive_center)
            .expect("passive roller center")
            .position,
        expected.center,
        POSITION_TOLERANCE,
        context,
    );
    assert_eq!(
        document.contacts(),
        expected.contact_metadata.as_slice(),
        "{context}: persistent contact branch metadata changed"
    );
    assert_eq!(
        [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
            document
                .curve(curve)
                .expect("motion-cam curve")
                .definition
                .clone()
        }),
        expected.curve_definitions,
        "{context}: persistent curve branch metadata changed"
    );
    for (contact, baseline) in expected
        .contact_ids
        .into_iter()
        .zip(expected.contact_parameters)
    {
        let value = document
            .scalar(
                document
                    .contact(contact)
                    .expect("passive contact")
                    .parameter,
            )
            .expect("passive contact parameter")
            .value;
        assert!(
            (value - baseline).abs() <= POSITION_TOLERANCE,
            "{context}: passive contact {contact:?} moved from {baseline} to {value}"
        );
    }
}

fn motion_cam_roller_center(parameter: f64) -> [f64; 2] {
    let tangent = [8.0, 8.0 - 16.0 * parameter];
    let tangent_norm = tangent[0].hypot(tangent[1]);
    [
        -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
        8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
    ]
}

#[allow(
    clippy::too_many_lines,
    reason = "the helper intentionally owns the complete public pointer-down, preview, exact-release, and publication boundary"
)]
fn perform_point_gesture(
    coordinator: &mut RetainedEditorCoordinator,
    viewport: Viewport,
    pointer_id: u64,
    point: DesignPointId,
    target: [f64; 2],
) -> GestureCapture {
    let baseline_design = coordinator.session().design_identity();
    let baseline_accepted = coordinator
        .session()
        .accepted_state()
        .expect("accepted gesture baseline")
        .identity();
    let start = coordinator
        .session()
        .accepted_state()
        .expect("accepted gesture baseline")
        .document()
        .point(point)
        .expect("gesture point")
        .position;
    let scene = accepted_scene(coordinator, viewport);
    let start_screen = viewport.model_to_screen(start);
    let target_screen = viewport.model_to_screen(target);
    let selection_effects = coordinator
        .editor_mut()
        .pointer_down(&scene, pointer(pointer_id, start_screen));
    assert_eq!(
        selection_effects,
        vec![EditorEffect::SelectionChanged(vec![SelectionItem::Point(
            point
        )])]
    );

    let request_effects = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, target_screen));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: requested_pointer,
            request_id,
            point: requested_point,
            model_position,
        },
    ] = request_effects.as_slice()
    else {
        panic!("one projected request: {request_effects:#?}");
    };
    assert_eq!(*requested_pointer, pointer_id);
    assert_eq!(*requested_point, point);
    assert_position_near(*model_position, target, 1.0e-12, "point target");

    let preview_effects = coordinator.resolve_projected_point_move(
        *requested_pointer,
        *request_id,
        *requested_point,
        *model_position,
    );
    assert!(matches!(
        preview_effects.as_slice(),
        [EditorEffect::PreviewPointMove {
            point: preview_point,
            ..
        }] if *preview_point == point
    ));
    let work = coordinator
        .projected_drag_work_evidence()
        .expect("projected work")
        .clone();
    assert_eq!(work.pointer_id, pointer_id);
    assert_eq!(work.point, point);
    assert_eq!(work.attempts, 1, "{work:#?}");
    assert!(
        !work.continued,
        "a released gesture must start fresh: {work:#?}"
    );
    assert!(work.accepted, "{work:#?}");
    assert_eq!(work.rejection_stage, None, "{work:#?}");
    assert!(work.operation.stopping_reason.is_none(), "{work:#?}");
    assert_controlled_work_bounded(&work);
    let locality = work
        .locality_plan()
        .expect("accepted gesture locality")
        .clone();
    assert_eq!(locality.design_identity(), baseline_design);
    assert_eq!(locality.accepted_state_identity(), baseline_accepted);
    assert_eq!(locality.point(), point);

    let preview_session = coordinator
        .visible_preview_session()
        .expect("visible accepted preview");
    let preview_json = preview_session
        .export_accepted_json()
        .expect("visible preview canonical JSON");
    let preview_document = preview_session
        .accepted_state()
        .expect("accepted preview state")
        .document()
        .clone();
    let preview_position = preview_document
        .point(point)
        .expect("preview gesture point")
        .position;
    let release_effects = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(pointer_id, target_screen),
    );
    assert!(matches!(
        release_effects.as_slice(),
        [
            EditorEffect::CommitPointMove {
                point: released_point,
                model_position,
                ..
            },
            EditorEffect::ClearPointPreview,
        ] if *released_point == point
            && model_position.map(f64::to_bits) == preview_position.map(f64::to_bits)
    ));
    dispatch_release(coordinator, &release_effects);

    let published = coordinator
        .session()
        .accepted_state()
        .expect("published gesture state");
    let published_design = coordinator.session().design_identity();
    let published_accepted = published.identity();
    assert_ne!(
        published_design, baseline_design,
        "release must publish a fresh retained design"
    );
    assert_ne!(
        published_accepted, baseline_accepted,
        "release must publish a fresh accepted state"
    );
    assert_eq!(
        published.document(),
        &preview_document,
        "release must publish the exact visible preview"
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("published canonical JSON"),
        preview_json,
        "release must publish the exact canonical preview bytes"
    );
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());

    GestureCapture {
        baseline_design,
        baseline_accepted,
        published_design,
        published_accepted,
        locality,
        published_document: preview_document,
        accepted_json: coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON"),
    }
}

fn pantograph_positions(document: &SketchDocument, ids: MotionPantographIds) -> [[f64; 2]; 5] {
    [
        document.point(ids.anchor).expect("anchor").position,
        document.point(ids.input).expect("input").position,
        document.point(ids.guide).expect("guide").position,
        document.point(ids.output).expect("output").position,
        document.point(ids.center).expect("center").position,
    ]
}

fn assert_pantograph_geometry(document: &SketchDocument, ids: MotionPantographIds) {
    let [anchor, input, guide, output, center] = pantograph_positions(document, ids);
    assert_position_near(anchor, [0.0, 0.0], POSITION_TOLERANCE, "fixed anchor");
    assert!(
        (input[0].hypot(input[1]) - 17.0_f64.sqrt()).abs() <= POSITION_TOLERANCE,
        "input arm lost its driving length: {input:?}"
    );
    assert!(
        (guide[0].hypot(guide[1]) - 10.0_f64.sqrt()).abs() <= POSITION_TOLERANCE,
        "guide arm lost its driving length: {guide:?}"
    );
    assert_position_near(
        output,
        [input[0] + guide[0], input[1] + guide[1]],
        POSITION_TOLERANCE,
        "pantograph closure",
    );
    assert_position_near(
        [2.0 * center[0], 2.0 * center[1]],
        output,
        POSITION_TOLERANCE,
        "pantograph midpoint",
    );
    assert!(
        input[0] * guide[1] - input[1] * guide[0] > 0.0,
        "pantograph left its positive assembly: input={input:?}, guide={guide:?}"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "release and cancellation must both prove that their already-issued queued samples cannot mutate any coordinator lifecycle surface"
)]
fn queued_projected_results_cannot_outlive_release_or_cancel_epochs() {
    let fixture =
        alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).expect("pantograph fixture");
    let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
        panic!("pantograph persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted pantograph session");
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 80.0).expect("viewport");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");

    let scene = accepted_scene(&coordinator, viewport);
    let start = scene
        .points
        .iter()
        .find(|point| point.id == ids.input)
        .expect("input scene point")
        .model_position;
    let start_screen = viewport.model_to_screen(start);
    coordinator
        .editor_mut()
        .pointer_down(&scene, pointer(700, start_screen));
    let first_request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(700, viewport.model_to_screen([4.0, 2.0])));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id,
            request_id,
            point,
            model_position,
        },
    ] = first_request.as_slice()
    else {
        panic!("first projected request: {first_request:#?}");
    };
    assert!(matches!(
        coordinator
            .resolve_projected_point_move(*pointer_id, *request_id, *point, *model_position)
            .as_slice(),
        [EditorEffect::PreviewPointMove { point: preview, .. }] if *preview == ids.input
    ));

    let queued_release_request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(700, viewport.model_to_screen([3.8, 2.2])));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: queued_pointer,
            request_id: queued_request,
            point: queued_point,
            model_position: queued_position,
        },
    ] = queued_release_request.as_slice()
    else {
        panic!("queued release request: {queued_release_request:#?}");
    };
    let release = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(700, viewport.model_to_screen([3.8, 2.2])),
    );
    dispatch_release(&mut coordinator, &release);
    let after_release = (
        coordinator.session().design_identity(),
        coordinator
            .session()
            .accepted_state()
            .expect("released accepted state")
            .identity(),
        coordinator
            .session()
            .export_accepted_json()
            .expect("released accepted JSON"),
        coordinator.history_len(),
        coordinator.history_cursor(),
        coordinator.transcript().len(),
    );
    assert!(
        coordinator
            .resolve_projected_point_move(
                *queued_pointer,
                *queued_request,
                *queued_point,
                *queued_position,
            )
            .is_empty(),
        "a queued result from the released epoch must be ignored"
    );
    assert_eq!(coordinator.session().design_identity(), after_release.0);
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("accepted state after stale release result")
            .identity(),
        after_release.1
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON after stale release result"),
        after_release.2
    );
    assert_eq!(coordinator.history_len(), after_release.3);
    assert_eq!(coordinator.history_cursor(), after_release.4);
    assert_eq!(coordinator.transcript().len(), after_release.5);
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());

    let scene = accepted_scene(&coordinator, viewport);
    let start = scene
        .points
        .iter()
        .find(|point| point.id == ids.guide)
        .expect("guide scene point")
        .model_position;
    coordinator
        .editor_mut()
        .pointer_down(&scene, pointer(701, viewport.model_to_screen(start)));
    let queued_cancel_request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(701, viewport.model_to_screen([1.3, 3.0])));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: queued_pointer,
            request_id: queued_request,
            point: queued_point,
            model_position: queued_position,
        },
    ] = queued_cancel_request.as_slice()
    else {
        panic!("queued cancel request: {queued_cancel_request:#?}");
    };
    let cancel = coordinator.editor_mut().cancel();
    assert!(
        cancel
            .iter()
            .any(|effect| matches!(effect, EditorEffect::CancelPointPreview))
    );
    for effect in &cancel {
        coordinator
            .apply_editor_effect(effect)
            .expect("dispatch cancellation");
    }
    let after_cancel = (
        coordinator.session().design_identity(),
        coordinator
            .session()
            .accepted_state()
            .expect("accepted state after cancel")
            .identity(),
        coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON after cancel"),
        coordinator.history_len(),
        coordinator.history_cursor(),
        coordinator.transcript().len(),
    );
    assert!(
        coordinator
            .resolve_projected_point_move(
                *queued_pointer,
                *queued_request,
                *queued_point,
                *queued_position,
            )
            .is_empty(),
        "a queued result from the cancelled epoch must be ignored"
    );
    assert_eq!(coordinator.session().design_identity(), after_cancel.0);
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("accepted state after stale cancel result")
            .identity(),
        after_cancel.1
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("accepted JSON after stale cancel result"),
        after_cancel.2
    );
    assert_eq!(coordinator.history_len(), after_cancel.3);
    assert_eq!(coordinator.history_cursor(), after_cancel.4);
    assert_eq!(coordinator.transcript().len(), after_cancel.5);
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "three complete alternating pointer gestures and their history round trip form one lifecycle regression"
)]
fn alternating_pantograph_gestures_recapture_locality_and_round_trip_history() {
    let fixture =
        alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).expect("pantograph fixture");
    let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
        panic!("pantograph persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted pantograph session");
    let initial_json = session
        .export_accepted_json()
        .expect("initial accepted JSON");
    let initial_document = session
        .accepted_state()
        .expect("initial accepted pantograph")
        .document()
        .clone();
    let initial_positions = pantograph_positions(&initial_document, ids);
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().activate_tool(EditorTool::Select);
    let viewport = Viewport::new([1000.0, 800.0], [2.5, 2.0], 90.0).expect("viewport");

    let first = perform_point_gesture(&mut coordinator, viewport, 201, ids.input, [4.0, 2.0]);
    assert_eq!(first.locality.anchors().len(), 1);
    assert_eq!(first.locality.anchors()[0].point(), ids.guide);
    assert_eq!(
        first.locality.anchors()[0].target().map(f64::to_bits),
        initial_positions[2].map(f64::to_bits)
    );
    assert_pantograph_geometry(&first.published_document, ids);
    let first_positions = pantograph_positions(&first.published_document, ids);
    assert_position_near(
        first_positions[2],
        initial_positions[2],
        POSITION_TOLERANCE,
        "first gesture passive guide",
    );
    let second = perform_point_gesture(&mut coordinator, viewport, 202, ids.guide, [1.4, 3.0]);
    assert_eq!(second.baseline_design, first.published_design);
    assert_eq!(second.baseline_accepted, first.published_accepted);
    assert_eq!(second.locality.anchors().len(), 1);
    assert_eq!(second.locality.anchors()[0].point(), ids.input);
    assert_eq!(
        second.locality.anchors()[0].target().map(f64::to_bits),
        first_positions[1].map(f64::to_bits),
        "guide gesture must capture the latest accepted input"
    );
    assert_pantograph_geometry(&second.published_document, ids);
    let second_positions = pantograph_positions(&second.published_document, ids);
    assert_position_near(
        second_positions[1],
        first_positions[1],
        POSITION_TOLERANCE,
        "second gesture passive input",
    );

    let third = perform_point_gesture(&mut coordinator, viewport, 203, ids.input, [3.6, 2.4]);
    assert_ne!(
        first.locality, third.locality,
        "the same active point in a new gesture must receive fresh stamped locality"
    );
    assert_eq!(third.baseline_design, second.published_design);
    assert_eq!(third.baseline_accepted, second.published_accepted);
    assert_eq!(third.locality.anchors().len(), 1);
    assert_eq!(third.locality.anchors()[0].point(), ids.guide);
    assert_eq!(
        third.locality.anchors()[0].target().map(f64::to_bits),
        second_positions[2].map(f64::to_bits),
        "third gesture must capture the latest accepted guide"
    );
    assert_pantograph_geometry(&third.published_document, ids);
    let third_positions = pantograph_positions(&third.published_document, ids);
    assert_position_near(
        third_positions[2],
        second_positions[2],
        POSITION_TOLERANCE,
        "third gesture passive guide",
    );

    assert_eq!(coordinator.history_len(), 4);
    assert_eq!(coordinator.history_cursor(), 3);
    for (cursor, expected) in [
        (2, &second.accepted_json),
        (1, &first.accepted_json),
        (0, &initial_json),
    ] {
        coordinator.undo().expect("undo pantograph gesture");
        assert_eq!(coordinator.history_cursor(), cursor);
        assert_eq!(
            &coordinator
                .session()
                .export_accepted_json()
                .expect("undone accepted JSON"),
            expected
        );
    }
    for (cursor, expected) in [
        (1, &first.accepted_json),
        (2, &second.accepted_json),
        (3, &third.accepted_json),
    ] {
        coordinator.redo().expect("redo pantograph gesture");
        assert_eq!(coordinator.history_cursor(), cursor);
        assert_eq!(
            &coordinator
                .session()
                .export_accepted_json()
                .expect("redone accepted JSON"),
            expected
        );
    }
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("redone pantograph")
            .document(),
        &third.published_document
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "circumference picking, continuation, passive-contact stability, exact release, and history form one end-to-end gesture"
)]
fn right_motion_cam_circumference_gesture_is_semantic_stable_and_undoable() {
    const POINTER_ID: u64 = 301;
    const PATH: [[f64; 2]; 2] = [[-0.04, 0.0], [-0.03, 0.025]];

    let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("motion-cam fixture");
    let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
        panic!("motion-cam persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted motion-cam session");
    let initial_json = session
        .export_accepted_json()
        .expect("initial accepted JSON");
    let initial_accepted = session.accepted_state().expect("accepted motion cam");
    let initial_document = initial_accepted.document().clone();
    let initial_design = session.design_identity();
    let initial_accepted_identity = initial_accepted.identity();
    let active_baseline = initial_document
        .point(ids.right_center)
        .expect("right roller center")
        .position;
    let passive_baseline = initial_document
        .point(ids.left_center)
        .expect("left roller center")
        .position;
    let contact_metadata = initial_document.contacts().to_vec();
    let passive_contacts = initial_document
        .constraints()
        .iter()
        .find_map(|constraint| {
            let DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            } = constraint.definition
            else {
                return None;
            };
            let contacts = [first_contact, second_contact];
            contacts
                .iter()
                .any(|contact| {
                    initial_document
                        .contact(*contact)
                        .is_some_and(|slot| slot.curve.curve == ids.left_circle)
                })
                .then_some(contacts)
        })
        .expect("left-roller tangency contacts");
    let passive_contact_values = passive_contacts.map(|contact| {
        initial_document
            .scalar(
                initial_document
                    .contact(contact)
                    .expect("passive contact")
                    .parameter,
            )
            .expect("passive contact parameter")
            .value
    });
    let curve_definitions = [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
        initial_document
            .curve(curve)
            .expect("motion-cam curve")
            .definition
            .clone()
    });

    let viewport = Viewport::new([1000.0, 700.0], [0.0, 2.0], 100.0).expect("viewport");
    let mut scene = EditorScene::from_accepted_for_design(
        initial_accepted.identity().revision().get(),
        session.design_identity(),
        &initial_document,
        session.design_document(),
        viewport,
        0.25,
    )
    .expect("initial motion-cam scene");
    let initial_scene_points = scene.points.clone();
    let initial_scene_curves = scene.curves.clone();
    let circumference_model = [active_baseline[0], active_baseline[1] + 1.0];
    let circumference_screen = viewport.model_to_screen(circumference_model);
    let expected_curve = SelectionItem::Curve(CurveSpan::line(ids.right_circle));
    assert_eq!(
        scene
            .hit_test(circumference_screen, PickTolerance::default())
            .expect("right circumference hit")
            .item,
        expected_curve
    );
    assert!(
        scene
            .annotation_hit_test(
                circumference_screen,
                PickTolerance::default(),
                &[],
                None,
                &[],
            )
            .is_none(),
        "gesture origin must exercise circumference geometry"
    );

    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().activate_tool(EditorTool::Select);
    assert_eq!(
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(POINTER_ID, circumference_screen)),
        vec![EditorEffect::SelectionChanged(vec![expected_curve])]
    );

    let mut frozen_locality = None;
    let mut final_pointer = circumference_screen;
    let mut final_preview_document = None;
    let mut final_preview_json = None;
    let mut final_scene_points = None;
    let mut final_scene_curves = None;

    for (index, delta) in PATH.into_iter().enumerate() {
        final_pointer = viewport.model_to_screen([
            circumference_model[0] + delta[0],
            circumference_model[1] + delta[1],
        ]);
        let request_effects = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(POINTER_ID, final_pointer));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request_effects.as_slice()
        else {
            panic!("one right-center projected request: {request_effects:#?}");
        };
        assert_eq!(*pointer_id, POINTER_ID);
        assert_eq!(
            *point, ids.right_center,
            "circumference must map to its semantic center"
        );
        assert_position_near(
            *model_position,
            [active_baseline[0] + delta[0], active_baseline[1] + delta[1]],
            1.0e-12,
            "preserved right-circumference pointer offset",
        );

        let preview_effects = coordinator.resolve_projected_point_move(
            *pointer_id,
            *request_id,
            *point,
            *model_position,
        );
        assert!(matches!(
            preview_effects.as_slice(),
            [EditorEffect::PreviewPointMove {
                point: preview_point,
                ..
            }] if *preview_point == ids.right_center
        ));
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("right-circle projected work");
        assert_eq!(work.attempts, 1, "{work:#?}");
        assert_eq!(work.continued, index > 0, "{work:#?}");
        assert!(work.accepted, "{work:#?}");
        assert_eq!(work.rejection_stage, None, "{work:#?}");
        assert!(work.operation.stopping_reason.is_none(), "{work:#?}");
        assert_controlled_work_bounded(work);
        let locality = work.locality_plan().expect("right-circle locality plan");
        assert_eq!(locality.design_identity(), initial_design);
        assert_eq!(
            locality.accepted_state_identity(),
            initial_accepted_identity
        );
        assert_eq!(locality.point(), ids.right_center);
        assert_eq!(locality.anchors().len(), 1);
        assert_eq!(locality.anchors()[0].point(), ids.left_center);
        assert_eq!(
            locality.anchors()[0].target().map(f64::to_bits),
            passive_baseline.map(f64::to_bits)
        );
        if let Some(expected) = &frozen_locality {
            assert_eq!(locality, expected, "continued locality changed");
        } else {
            frozen_locality = Some(locality.clone());
        }

        let preview_session = coordinator
            .visible_preview_session()
            .expect("visible right-circle preview");
        let preview = preview_session
            .accepted_state()
            .expect("accepted right-circle preview");
        let preview_document = preview.document();
        assert_position_near(
            preview_document
                .point(ids.left_center)
                .expect("passive left center")
                .position,
            passive_baseline,
            POSITION_TOLERANCE,
            "passive left center",
        );
        assert_eq!(
            preview_document.contacts(),
            contact_metadata.as_slice(),
            "persistent contact metadata changed during the gesture"
        );
        assert_eq!(
            [ids.cam, ids.left_circle, ids.right_circle].map(|curve| {
                preview_document
                    .curve(curve)
                    .expect("preview motion-cam curve")
                    .definition
                    .clone()
            }),
            curve_definitions,
            "curve branch definitions changed during the gesture"
        );
        for (contact, baseline) in passive_contacts.into_iter().zip(passive_contact_values) {
            let value = preview_document
                .scalar(
                    preview_document
                        .contact(contact)
                        .expect("preview passive contact")
                        .parameter,
                )
                .expect("preview passive parameter")
                .value;
            assert!(
                (value - baseline).abs() <= POSITION_TOLERANCE,
                "passive left contact {contact:?} moved from {baseline} to {value}"
            );
        }

        scene = EditorScene::from_accepted_for_design(
            preview.identity().revision().get(),
            preview_session.design_identity(),
            preview_document,
            preview_session.design_document(),
            viewport,
            0.25,
        )
        .expect("continued right-circle scene");
        final_scene_points = Some(scene.points.clone());
        final_scene_curves = Some(scene.curves.clone());
        final_preview_document = Some(preview_document.clone());
        final_preview_json = Some(
            preview_session
                .export_accepted_json()
                .expect("right-circle preview canonical JSON"),
        );
    }

    let final_preview_document = final_preview_document.expect("final right-circle preview");
    let final_preview_json = final_preview_json.expect("final right-circle preview JSON");
    let final_active = final_preview_document
        .point(ids.right_center)
        .expect("final right center")
        .position;
    let release_effects = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(POINTER_ID, final_pointer),
    );
    assert!(matches!(
        release_effects.as_slice(),
        [
            EditorEffect::CommitPointMove {
                point,
                model_position,
                ..
            },
            EditorEffect::ClearPointPreview,
        ] if *point == ids.right_center
            && model_position.map(f64::to_bits) == final_active.map(f64::to_bits)
    ));
    dispatch_release(&mut coordinator, &release_effects);

    let published = coordinator
        .session()
        .accepted_state()
        .expect("accepted right-circle release");
    assert_eq!(
        published.document(),
        &final_preview_document,
        "release must publish the exact visible right-circle preview"
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("published right-circle canonical JSON"),
        final_preview_json,
        "release must publish the exact right-circle preview bytes"
    );
    assert_position_near(
        published
            .document()
            .point(ids.left_center)
            .expect("published passive left center")
            .position,
        passive_baseline,
        POSITION_TOLERANCE,
        "published passive left center",
    );
    assert_eq!(published.document().contacts(), contact_metadata.as_slice());
    assert_eq!(
        coordinator
            .session()
            .last_attempt()
            .input()
            .publication_request()
            .drag,
        None
    );
    assert_eq!(coordinator.history_len(), 2);
    assert_eq!(coordinator.history_cursor(), 1);
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());
    let committed_json = coordinator
        .session()
        .export_accepted_json()
        .expect("committed accepted JSON");

    coordinator.undo().expect("undo right-circle gesture");
    assert_eq!(coordinator.history_cursor(), 0);
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("undone accepted JSON"),
        initial_json
    );
    let undone_scene = accepted_scene(&coordinator, viewport);
    assert_eq!(undone_scene.points, initial_scene_points);
    assert_eq!(undone_scene.curves, initial_scene_curves);

    coordinator.redo().expect("redo right-circle gesture");
    assert_eq!(coordinator.history_cursor(), 1);
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("redone accepted JSON"),
        committed_json
    );
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("redone right-circle state")
            .document(),
        &final_preview_document
    );
    let redone_scene = accepted_scene(&coordinator, viewport);
    assert_eq!(
        redone_scene.points,
        final_scene_points.expect("final scene points")
    );
    assert_eq!(
        redone_scene.curves,
        final_scene_curves.expect("final scene curves")
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the mirrored circumference lifecycle directly guards the formerly under-covered active/passive direction"
)]
fn left_motion_cam_circumference_release_is_stable_and_byte_exact() {
    const POINTER_ID: u64 = 302;
    const PATH: [[f64; 2]; 2] = [[0.04, 0.0], [0.03, 0.025]];

    let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("motion-cam fixture");
    let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
        panic!("motion-cam persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted motion-cam session");
    let initial_json = session
        .export_accepted_json()
        .expect("initial accepted JSON");
    let initial = session.accepted_state().expect("accepted motion cam");
    let initial_document = initial.document().clone();
    let initial_design = session.design_identity();
    let initial_accepted = initial.identity();
    let active_baseline = initial_document
        .point(ids.left_center)
        .expect("left roller center")
        .position;
    let passive_baseline = initial_document
        .point(ids.right_center)
        .expect("right roller center")
        .position;

    let viewport = Viewport::new([1000.0, 700.0], [0.0, 2.0], 100.0).expect("viewport");
    let mut scene = EditorScene::from_accepted_for_design(
        initial.identity().revision().get(),
        session.design_identity(),
        &initial_document,
        session.design_document(),
        viewport,
        0.25,
    )
    .expect("initial motion-cam scene");
    let circumference_model = [active_baseline[0], active_baseline[1] + 1.0];
    let circumference_screen = viewport.model_to_screen(circumference_model);
    let expected_curve = SelectionItem::Curve(CurveSpan::line(ids.left_circle));
    assert_eq!(
        scene
            .hit_test(circumference_screen, PickTolerance::default())
            .expect("left circumference hit")
            .item,
        expected_curve
    );

    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().activate_tool(EditorTool::Select);
    assert_eq!(
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(POINTER_ID, circumference_screen)),
        vec![EditorEffect::SelectionChanged(vec![expected_curve])]
    );

    let mut frozen_locality = None;
    let mut final_pointer = circumference_screen;
    let mut final_preview_document = None;
    let mut final_preview_json = None;
    for (index, delta) in PATH.into_iter().enumerate() {
        final_pointer = viewport.model_to_screen([
            circumference_model[0] + delta[0],
            circumference_model[1] + delta[1],
        ]);
        let request = coordinator
            .editor_mut()
            .pointer_move(&scene, pointer(POINTER_ID, final_pointer));
        let [
            EditorEffect::RequestProjectedPointMove {
                pointer_id,
                request_id,
                point,
                model_position,
            },
        ] = request.as_slice()
        else {
            panic!("one left-center projected request: {request:#?}");
        };
        assert_eq!(*point, ids.left_center);
        assert_position_near(
            *model_position,
            [active_baseline[0] + delta[0], active_baseline[1] + delta[1]],
            1.0e-12,
            "preserved left-circumference pointer offset",
        );
        assert!(matches!(
            coordinator
                .resolve_projected_point_move(
                    *pointer_id,
                    *request_id,
                    *point,
                    *model_position,
                )
                .as_slice(),
            [EditorEffect::PreviewPointMove { point: preview, .. }]
                if *preview == ids.left_center
        ));
        let work = coordinator
            .projected_drag_work_evidence()
            .expect("left-circle projected work");
        assert_eq!(work.attempts, 1, "{work:#?}");
        assert_eq!(work.continued, index > 0, "{work:#?}");
        assert!(work.accepted, "{work:#?}");
        assert_eq!(work.rejection_stage, None, "{work:#?}");
        assert_controlled_work_bounded(work);
        let locality = work.locality_plan().expect("left-circle locality plan");
        assert_eq!(locality.design_identity(), initial_design);
        assert_eq!(locality.accepted_state_identity(), initial_accepted);
        assert_eq!(locality.point(), ids.left_center);
        assert_eq!(locality.anchors().len(), 1);
        assert_eq!(locality.anchors()[0].point(), ids.right_center);
        assert_eq!(
            locality.anchors()[0].target().map(f64::to_bits),
            passive_baseline.map(f64::to_bits)
        );
        if let Some(expected) = &frozen_locality {
            assert_eq!(locality, expected, "continued left locality changed");
        } else {
            frozen_locality = Some(locality.clone());
        }

        let preview_session = coordinator
            .visible_preview_session()
            .expect("visible left-circle preview");
        let preview = preview_session
            .accepted_state()
            .expect("accepted left-circle preview");
        assert_position_near(
            preview
                .document()
                .point(ids.right_center)
                .expect("passive right center")
                .position,
            passive_baseline,
            POSITION_TOLERANCE,
            "passive right center",
        );
        final_preview_document = Some(preview.document().clone());
        final_preview_json = Some(
            preview_session
                .export_accepted_json()
                .expect("left-circle preview canonical JSON"),
        );
        scene = EditorScene::from_accepted_for_design(
            preview.identity().revision().get(),
            preview_session.design_identity(),
            preview.document(),
            preview_session.design_document(),
            viewport,
            0.25,
        )
        .expect("continued left-circle scene");
    }

    let final_preview_document = final_preview_document.expect("final left-circle preview");
    let final_preview_json = final_preview_json.expect("final left-circle preview JSON");
    let final_active = final_preview_document
        .point(ids.left_center)
        .expect("final left center")
        .position;
    let release = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(POINTER_ID, final_pointer),
    );
    assert!(matches!(
        release.as_slice(),
        [
            EditorEffect::CommitPointMove {
                point,
                model_position,
                ..
            },
            EditorEffect::ClearPointPreview,
        ] if *point == ids.left_center
            && model_position.map(f64::to_bits) == final_active.map(f64::to_bits)
    ));
    dispatch_release(&mut coordinator, &release);

    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("accepted left-circle release")
            .document(),
        &final_preview_document,
        "left release must publish the exact visible preview"
    );
    let committed_json = coordinator
        .session()
        .export_accepted_json()
        .expect("committed left-circle JSON");
    assert_eq!(
        committed_json, final_preview_json,
        "left release must publish the exact canonical preview bytes"
    );
    assert_position_near(
        coordinator
            .session()
            .accepted_state()
            .expect("accepted left-circle release")
            .document()
            .point(ids.right_center)
            .expect("published passive right center")
            .position,
        passive_baseline,
        POSITION_TOLERANCE,
        "published passive right center",
    );
    assert_eq!(coordinator.history_len(), 2);
    assert_eq!(coordinator.history_cursor(), 1);
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());

    coordinator.undo().expect("undo left-circle gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("undone left-circle JSON"),
        initial_json
    );
    coordinator.redo().expect("redo left-circle gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("redone left-circle JSON"),
        committed_json
    );
}

#[derive(Clone, Copy)]
struct MotionCamRejectionCase {
    pointer_id: u64,
    drag_left: bool,
    first_parameter: f64,
    recovery_parameter: f64,
    difficult_target: [f64; 2],
}

#[allow(
    clippy::too_many_lines,
    reason = "one helper owns the complete public circumference accept, reject, recovery, release, and history lifecycle"
)]
fn run_motion_cam_rejection_lifecycle(case: MotionCamRejectionCase) {
    let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).expect("motion-cam fixture");
    let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
        panic!("motion-cam persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted motion-cam session");
    let initial_json = session
        .export_accepted_json()
        .expect("initial accepted JSON");
    let initial_accepted = session.accepted_state().expect("accepted motion cam");
    let initial_document = initial_accepted.document().clone();
    let initial_design = session.design_identity();
    let initial_accepted_identity = initial_accepted.identity();
    let (active, active_circle, passive, passive_circle) = if case.drag_left {
        (
            ids.left_center,
            ids.left_circle,
            ids.right_center,
            ids.right_circle,
        )
    } else {
        (
            ids.right_center,
            ids.right_circle,
            ids.left_center,
            ids.left_circle,
        )
    };
    let active_baseline = initial_document
        .point(active)
        .expect("active roller center")
        .position;
    let passive_state =
        capture_passive_motion_cam_state(&initial_document, ids, passive, passive_circle);
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 2.0], 100.0).expect("viewport");
    let mut scene = EditorScene::from_accepted_for_design(
        initial_accepted.identity().revision().get(),
        session.design_identity(),
        &initial_document,
        session.design_document(),
        viewport,
        0.25,
    )
    .expect("initial motion-cam scene");
    let circumference_model = [active_baseline[0], active_baseline[1] + 1.0];
    let circumference_screen = viewport.model_to_screen(circumference_model);
    let expected_curve = SelectionItem::Curve(CurveSpan::line(active_circle));
    assert_eq!(
        scene
            .hit_test(circumference_screen, PickTolerance::default())
            .expect("active circumference hit")
            .item,
        expected_curve
    );

    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().activate_tool(EditorTool::Select);
    assert_eq!(
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(case.pointer_id, circumference_screen)),
        vec![EditorEffect::SelectionChanged(vec![expected_curve])]
    );

    let first_target = motion_cam_roller_center(case.first_parameter);
    let first_pointer = viewport.model_to_screen([first_target[0], first_target[1] + 1.0]);
    let (requested_point, requested_target, first_effects, first_work) = resolve_pointer_sample(
        &mut coordinator,
        &scene,
        pointer(case.pointer_id, first_pointer),
    );
    assert_eq!(requested_point, active);
    assert_position_near(
        requested_target,
        first_target,
        1.0e-12,
        "accepted circumference target",
    );
    assert!(matches!(
        first_effects.as_slice(),
        [EditorEffect::PreviewPointMove { point, .. }] if *point == active
    ));
    assert_eq!(first_work.attempts, 1, "{first_work:#?}");
    assert!(!first_work.continued, "{first_work:#?}");
    assert!(first_work.accepted, "{first_work:#?}");
    assert_eq!(first_work.rejection_stage, None, "{first_work:#?}");
    assert_controlled_work_bounded(&first_work);
    let frozen_locality = first_work
        .locality_plan()
        .expect("first roller locality")
        .clone();
    assert_eq!(frozen_locality.design_identity(), initial_design);
    assert_eq!(
        frozen_locality.accepted_state_identity(),
        initial_accepted_identity
    );
    assert_eq!(frozen_locality.point(), active);
    assert_eq!(frozen_locality.anchors().len(), 1);
    assert_eq!(frozen_locality.anchors()[0].point(), passive);
    assert_eq!(
        frozen_locality.anchors()[0].target().map(f64::to_bits),
        passive_state.center.map(f64::to_bits)
    );
    let (first_preview_identity, first_preview_document, first_preview_json) = {
        let preview_session = coordinator
            .visible_preview_session()
            .expect("first accepted visible preview");
        let preview = preview_session
            .accepted_state()
            .expect("first accepted preview");
        assert_position_near(
            preview
                .document()
                .point(active)
                .expect("active roller")
                .position,
            first_target,
            5.0e-8,
            "first accepted roller position",
        );
        assert_passive_motion_cam_state(
            preview.document(),
            ids,
            passive,
            &passive_state,
            "first accepted preview",
        );
        scene = EditorScene::from_accepted_for_design(
            preview.identity().revision().get(),
            preview_session.design_identity(),
            preview.document(),
            preview_session.design_document(),
            viewport,
            0.25,
        )
        .expect("first accepted preview scene");
        (
            preview.identity(),
            preview.document().clone(),
            preview_session
                .export_accepted_json()
                .expect("first preview JSON"),
        )
    };

    let difficult_pointer =
        viewport.model_to_screen([case.difficult_target[0], case.difficult_target[1] + 1.0]);
    let (requested_point, requested_target, difficult_effects, difficult_work) =
        resolve_pointer_sample(
            &mut coordinator,
            &scene,
            pointer(case.pointer_id, difficult_pointer),
        );
    assert_eq!(requested_point, active);
    assert_position_near(
        requested_target,
        case.difficult_target,
        1.0e-12,
        "difficult circumference target",
    );
    assert!(
        difficult_effects.is_empty(),
        "a rejected pointer sample must not replace the visible preview: {difficult_effects:#?}"
    );
    assert_eq!(difficult_work.attempts, 1, "{difficult_work:#?}");
    assert!(difficult_work.continued, "{difficult_work:#?}");
    assert!(!difficult_work.accepted, "{difficult_work:#?}");
    assert!(
        difficult_work.rejection_stage.is_some(),
        "{difficult_work:#?}"
    );
    assert_eq!(
        difficult_work
            .locality_plan()
            .expect("difficult roller locality"),
        &frozen_locality
    );
    assert_controlled_work_bounded(&difficult_work);
    {
        let retained_session = coordinator
            .visible_preview_session()
            .expect("retained visible preview after rejection");
        let retained = retained_session
            .accepted_state()
            .expect("retained accepted preview");
        assert_eq!(retained.identity(), first_preview_identity);
        assert_eq!(
            retained.document(),
            &first_preview_document,
            "rejection changed the complete visible preview"
        );
        assert_eq!(
            retained_session
                .export_accepted_json()
                .expect("retained preview JSON"),
            first_preview_json,
            "rejection changed the canonical visible preview bytes"
        );
        assert_passive_motion_cam_state(
            retained.document(),
            ids,
            passive,
            &passive_state,
            "rejected sample retention",
        );
    }

    let recovery_target = motion_cam_roller_center(case.recovery_parameter);
    let recovery_pointer = viewport.model_to_screen([recovery_target[0], recovery_target[1] + 1.0]);
    let (requested_point, requested_target, recovery_effects, recovery_work) =
        resolve_pointer_sample(
            &mut coordinator,
            &scene,
            pointer(case.pointer_id, recovery_pointer),
        );
    assert_eq!(requested_point, active);
    assert_position_near(
        requested_target,
        recovery_target,
        1.0e-12,
        "recovery circumference target",
    );
    assert!(matches!(
        recovery_effects.as_slice(),
        [EditorEffect::PreviewPointMove { point, .. }] if *point == active
    ));
    assert_eq!(recovery_work.attempts, 1, "{recovery_work:#?}");
    assert!(recovery_work.continued, "{recovery_work:#?}");
    assert!(recovery_work.accepted, "{recovery_work:#?}");
    assert_eq!(recovery_work.rejection_stage, None, "{recovery_work:#?}");
    assert_eq!(
        recovery_work
            .locality_plan()
            .expect("recovered roller locality"),
        &frozen_locality
    );
    assert_controlled_work_bounded(&recovery_work);
    let (recovered_document, recovered_json, recovered_active) = {
        let recovered_session = coordinator
            .visible_preview_session()
            .expect("recovered visible preview");
        let recovered = recovered_session
            .accepted_state()
            .expect("recovered accepted preview");
        let active_position = recovered
            .document()
            .point(active)
            .expect("recovered active roller")
            .position;
        assert_position_near(
            active_position,
            recovery_target,
            5.0e-8,
            "recovered active roller",
        );
        assert!(
            (active_position[0]
                - first_preview_document
                    .point(active)
                    .expect("first active roller")
                    .position[0])
                .hypot(
                    active_position[1]
                        - first_preview_document
                            .point(active)
                            .expect("first active roller")
                            .position[1],
                )
                <= 0.25,
            "recovery jumped away from the retained continuation"
        );
        assert_passive_motion_cam_state(
            recovered.document(),
            ids,
            passive,
            &passive_state,
            "recovered preview",
        );
        (
            recovered.document().clone(),
            recovered_session
                .export_accepted_json()
                .expect("recovered preview JSON"),
            active_position,
        )
    };

    let release = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(case.pointer_id, recovery_pointer),
    );
    assert!(matches!(
        release.as_slice(),
        [
            EditorEffect::CommitPointMove {
                point,
                model_position,
                ..
            },
            EditorEffect::ClearPointPreview,
        ] if *point == active
            && model_position.map(f64::to_bits) == recovered_active.map(f64::to_bits)
    ));
    dispatch_release(&mut coordinator, &release);
    let published = coordinator
        .session()
        .accepted_state()
        .expect("published recovered roller");
    assert_eq!(
        published.document(),
        &recovered_document,
        "release changed the recovered visible preview"
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("published recovered JSON"),
        recovered_json,
        "release changed the recovered canonical preview bytes"
    );
    assert_passive_motion_cam_state(
        published.document(),
        ids,
        passive,
        &passive_state,
        "published recovered preview",
    );
    assert_eq!(coordinator.history_len(), 2);
    assert_eq!(coordinator.history_cursor(), 1);
    assert!(coordinator.visible_preview_session().is_none());
    assert!(coordinator.projected_drag_work_evidence().is_none());

    coordinator.undo().expect("undo recovered roller gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("undone roller JSON"),
        initial_json
    );
    coordinator.redo().expect("redo recovered roller gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("redone roller JSON"),
        recovered_json
    );
}

#[test]
fn motion_cam_circumference_rejection_recovers_and_releases_exactly_in_both_directions() {
    for case in [
        MotionCamRejectionCase {
            pointer_id: 303,
            drag_left: true,
            first_parameter: 0.26,
            recovery_parameter: 0.28,
            difficult_target: [-8.0, 0.0],
        },
        MotionCamRejectionCase {
            pointer_id: 304,
            drag_left: false,
            first_parameter: 0.74,
            recovery_parameter: 0.72,
            difficult_target: [8.0, 0.0],
        },
    ] {
        run_motion_cam_rejection_lifecycle(case);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one helper owns a complete multi-sample boundary approach, reversal, exact release, and history lifecycle"
)]
fn run_pantograph_boundary_reversal(use_center: bool) {
    const OUTPUT_PATH: [[f64; 2]; 8] = [
        [5.3, 4.0],
        [5.6, 4.0],
        [5.85, 4.0],
        [6.0, 4.0],
        [5.85, 4.0],
        [5.6, 4.0],
        [5.3, 4.0],
        [5.0, 4.0],
    ];

    let fixture =
        alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).expect("pantograph fixture");
    let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
        panic!("pantograph persistent roles");
    };
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .expect("accepted pantograph session");
    let initial_json = session
        .export_accepted_json()
        .expect("initial pantograph JSON");
    let initial_accepted = session.accepted_state().expect("accepted pantograph");
    let initial_document = initial_accepted.document().clone();
    let initial_positions = pantograph_positions(&initial_document, ids);
    let initial_design = session.design_identity();
    let initial_accepted_identity = initial_accepted.identity();
    let branch_directions = ids.bars.map(|bar| {
        let CurveDefinition::Line {
            branch_direction, ..
        } = initial_document
            .curve(bar)
            .expect("pantograph bar")
            .definition
        else {
            panic!("pantograph bars are lines");
        };
        branch_direction
    });
    let active = if use_center { ids.center } else { ids.output };
    let pointer_id = if use_center { 306 } else { 305 };
    let viewport = Viewport::new([1000.0, 800.0], [2.5, 2.0], 90.0).expect("viewport");
    let mut scene = EditorScene::from_accepted_for_design(
        initial_accepted.identity().revision().get(),
        session.design_identity(),
        &initial_document,
        session.design_document(),
        viewport,
        0.25,
    )
    .expect("initial pantograph scene");
    let start = initial_document
        .point(active)
        .expect("pantograph active point")
        .position;
    let start_screen = viewport.model_to_screen(start);
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator.editor_mut().activate_tool(EditorTool::Select);
    assert_eq!(
        coordinator
            .editor_mut()
            .pointer_down(&scene, pointer(pointer_id, start_screen)),
        vec![EditorEffect::SelectionChanged(vec![SelectionItem::Point(
            active
        )])]
    );

    let mut previous_positions = initial_positions;
    let mut final_pointer = start_screen;
    let mut final_document = None;
    let mut final_json = None;
    for (index, expected_output) in OUTPUT_PATH.into_iter().enumerate() {
        let target = if use_center {
            [0.5 * expected_output[0], 0.5 * expected_output[1]]
        } else {
            expected_output
        };
        final_pointer = viewport.model_to_screen(target);
        let (requested_point, requested_target, effects, work) =
            resolve_pointer_sample(&mut coordinator, &scene, pointer(pointer_id, final_pointer));
        assert_eq!(requested_point, active);
        assert_position_near(
            requested_target,
            target,
            1.0e-12,
            "pantograph pointer target",
        );
        assert!(matches!(
            effects.as_slice(),
            [EditorEffect::PreviewPointMove { point, .. }] if *point == active
        ));
        assert_eq!(work.attempts, 1, "{work:#?}");
        assert_eq!(work.continued, index > 0, "{work:#?}");
        assert!(work.accepted, "{work:#?}");
        assert_eq!(work.rejection_stage, None, "{work:#?}");
        assert_controlled_work_bounded(&work);
        let locality = work.locality_plan().expect("pantograph locality");
        assert_eq!(locality.design_identity(), initial_design);
        assert_eq!(
            locality.accepted_state_identity(),
            initial_accepted_identity
        );
        assert_eq!(locality.point(), active);
        assert_eq!(locality.hard_degrees_of_freedom(), 2);
        assert_eq!(locality.active_rank(), 2);
        assert_eq!(locality.passive_degrees_of_freedom(), 0);
        assert!(locality.anchors().is_empty());

        let preview_session = coordinator
            .visible_preview_session()
            .expect("visible pantograph preview");
        let preview = preview_session
            .accepted_state()
            .expect("accepted pantograph preview");
        let document = preview.document();
        assert_position_near(
            document
                .point(active)
                .expect("active pantograph point")
                .position,
            target,
            POSITION_TOLERANCE,
            "active pantograph response",
        );
        assert_position_near(
            document
                .point(ids.output)
                .expect("pantograph output")
                .position,
            expected_output,
            POSITION_TOLERANCE,
            "pantograph output target",
        );
        assert_pantograph_geometry(document, ids);
        for (bar, expected) in ids.bars.into_iter().zip(branch_directions) {
            let CurveDefinition::Line {
                branch_direction, ..
            } = document.curve(bar).expect("pantograph bar").definition
            else {
                panic!("pantograph bars remain lines");
            };
            assert_eq!(
                branch_direction.map(f64::to_bits),
                expected.map(f64::to_bits),
                "ordinary boundary drag changed explicit pantograph branch state"
            );
        }
        let positions = pantograph_positions(document, ids);
        for (role, role_index) in [("input", 1), ("guide", 2)] {
            let step = (positions[role_index][0] - previous_positions[role_index][0])
                .hypot(positions[role_index][1] - previous_positions[role_index][1]);
            assert!(
                step <= 1.0,
                "{role} jumped while the output followed a short boundary step: \
                 previous={:?}, current={:?}",
                previous_positions[role_index],
                positions[role_index]
            );
        }
        previous_positions = positions;
        scene = EditorScene::from_accepted_for_design(
            preview.identity().revision().get(),
            preview_session.design_identity(),
            document,
            preview_session.design_document(),
            viewport,
            0.25,
        )
        .expect("continued pantograph scene");
        final_document = Some(document.clone());
        final_json = Some(
            preview_session
                .export_accepted_json()
                .expect("pantograph preview JSON"),
        );
    }

    let final_document = final_document.expect("final pantograph preview");
    let final_json = final_json.expect("final pantograph preview JSON");
    for (role, returned, initial) in ["anchor", "input", "guide", "output", "center"]
        .into_iter()
        .zip(pantograph_positions(&final_document, ids))
        .zip(initial_positions)
        .map(|((role, returned), initial)| (role, returned, initial))
    {
        assert_position_near(
            returned,
            initial,
            2.0e-7,
            &format!("reversed pantograph {role}"),
        );
    }
    let final_active = final_document
        .point(active)
        .expect("final pantograph active point")
        .position;
    let release = coordinator.editor_mut().pointer_up(
        &scene,
        scene.design_identity,
        pointer(pointer_id, final_pointer),
    );
    assert!(matches!(
        release.as_slice(),
        [
            EditorEffect::CommitPointMove {
                point,
                model_position,
                ..
            },
            EditorEffect::ClearPointPreview,
        ] if *point == active
            && model_position.map(f64::to_bits) == final_active.map(f64::to_bits)
    ));
    dispatch_release(&mut coordinator, &release);
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("published pantograph reversal")
            .document(),
        &final_document,
        "release changed the reversed visible pantograph preview"
    );
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("published pantograph JSON"),
        final_json,
        "release changed the reversed canonical pantograph bytes"
    );
    assert_eq!(coordinator.history_len(), 2);
    assert_eq!(coordinator.history_cursor(), 1);

    coordinator
        .undo()
        .expect("undo pantograph boundary gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("undone pantograph JSON"),
        initial_json
    );
    coordinator
        .redo()
        .expect("redo pantograph boundary gesture");
    assert_eq!(
        coordinator
            .session()
            .export_accepted_json()
            .expect("redone pantograph JSON"),
        final_json
    );
}

#[test]
fn pantograph_output_and_center_pointer_gestures_reverse_near_outer_boundary_exactly() {
    for use_center in [false, true] {
        run_pantograph_boundary_reversal(use_center);
    }
}
