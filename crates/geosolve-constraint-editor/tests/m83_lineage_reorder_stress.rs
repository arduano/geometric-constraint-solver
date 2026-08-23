// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintActionRequest, ConstraintIntent, ConstructionPoint, ConstructionProposal,
    EditorEffect, EditorScene, Modifiers, PointerInput, RetainedEditorCoordinator, ScreenPoint,
    SelectionItem, Viewport,
};
use geosolve_sketch::{
    CurveSpan, DocumentHyperbolaBranch, DocumentSolveRequest, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{LineageStep, LineageStepId};

fn empty_coordinator() -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted empty session");
    RetainedEditorCoordinator::new(session).expect("coordinator")
}

fn add_point(
    coordinator: &mut RetainedEditorCoordinator,
    position: [f64; 2],
) -> (geosolve_sketch::DesignPointId, LineageStepId) {
    let point = coordinator
        .apply_construction(
            coordinator.session().design_identity(),
            &ConstructionProposal::Point {
                point: ConstructionPoint::New(position),
            },
        )
        .expect("authored point")
        .value
        .points[0];
    let owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("point owner")
        .id;
    (point, owner)
}

fn assert_owner_manifest_unchanged(before: &LineageStep, after: &LineageStep) {
    assert_eq!(after.id, before.id);
    assert_eq!(after.key, before.key);
    assert_eq!(after.label, before.label);
    assert_eq!(after.outputs, before.outputs);
    assert_eq!(after.output_identities, before.output_identities);
    assert_eq!(after.reservations, before.reservations);
    assert_eq!(after.state, before.state);
}

fn step_order(coordinator: &RetainedEditorCoordinator) -> Vec<LineageStepId> {
    coordinator
        .lineage_document()
        .steps()
        .iter()
        .map(|step| step.id)
        .collect()
}

fn visible_scene(coordinator: &RetainedEditorCoordinator, viewport: Viewport) -> EditorScene {
    let visible = coordinator
        .solved_preview_session()
        .unwrap_or_else(|| coordinator.session());
    let accepted = visible
        .accepted_state_for_current_input()
        .expect("visible accepted lineage materialization");
    EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        accepted.design_identity(),
        accepted.document(),
        visible.design_document(),
        viewport,
        0.5,
    )
    .expect("finite lineage scene")
    .with_retained_session(visible)
    .expect("authenticated lineage scene")
}

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn accepted_positions(
    coordinator: &RetainedEditorCoordinator,
    points: [geosolve_sketch::DesignPointId; 2],
) -> [[u64; 2]; 2] {
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted sketch")
        .document();
    points.map(|point| {
        accepted
            .point(point)
            .expect("stable accepted point")
            .position
            .map(f64::to_bits)
    })
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one public stress keeps reorder, projected multi-owner rewrite, history, cold restore and identity evidence adjacent"
)]
fn reordered_recipe_owners_survive_projected_multi_owner_rewrite_history_and_cold_restore() {
    let mut coordinator = empty_coordinator();
    let baseline = coordinator.lineage_document().steps()[0].id;
    let (first, first_owner) = add_point(&mut coordinator, [0.0, 0.0]);
    let (second, second_owner) = add_point(&mut coordinator, [2.0, 1.0]);
    let relation = coordinator
        .apply_constraint_action_for(
            coordinator.session().design_identity(),
            &[SelectionItem::Point(first), SelectionItem::Point(second)],
            ConstraintActionRequest {
                intent: ConstraintIntent::Coincident,
                label: "shared projected position".into(),
                contacts: Vec::new(),
                relation: None,
            },
        )
        .expect("Coincident relation")
        .value;
    let relation_owner = coordinator.lineage_document().steps()[3].id;
    let first_before = coordinator
        .lineage_document()
        .step(first_owner)
        .expect("first owner")
        .clone();
    let second_before = coordinator
        .lineage_document()
        .step(second_owner)
        .expect("second owner")
        .clone();
    let relation_before = coordinator
        .lineage_document()
        .step(relation_owner)
        .expect("relation owner")
        .clone();
    let positions_before = accepted_positions(&coordinator, [first, second]);

    let reordered = coordinator
        .reorder_lineage_step(
            coordinator.lineage_identity(),
            second_owner,
            Some(first_owner),
        )
        .expect("independent recipe reorder");
    assert!(reordered.changed);
    assert!(reordered.accepted);
    assert!(!reordered.clamped);
    assert_eq!(
        step_order(&coordinator),
        vec![baseline, second_owner, first_owner, relation_owner]
    );
    assert_owner_manifest_unchanged(
        &first_before,
        coordinator
            .lineage_document()
            .step(first_owner)
            .expect("reordered first owner"),
    );
    assert_owner_manifest_unchanged(
        &second_before,
        coordinator
            .lineage_document()
            .step(second_owner)
            .expect("reordered second owner"),
    );

    let history_before_projection = (coordinator.history_len(), coordinator.history_cursor());
    let pointer_id = 0x83_71;
    let viewport = Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).expect("viewport");
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(first)]);
    let scene = visible_scene(&coordinator, viewport);
    let press = scene
        .points
        .iter()
        .find(|candidate| candidate.id == first)
        .expect("first point in accepted scene")
        .screen_position;
    assert!(
        coordinator
            .pointer_down(&scene, pointer(pointer_id, press))
            .is_empty()
    );
    let terminal = viewport.model_to_screen([4.0, 3.0]);
    let request = coordinator
        .editor_mut()
        .pointer_move(&scene, pointer(pointer_id, terminal));
    let [
        EditorEffect::RequestProjectedPointMove {
            pointer_id: requested_pointer,
            request_id,
            point: requested_point,
            model_position,
        },
    ] = request.as_slice()
    else {
        panic!("expected one terminal projected request: {request:?}")
    };
    let preview_effects = coordinator.resolve_projected_point_move(
        *requested_pointer,
        *request_id,
        *requested_point,
        *model_position,
    );
    assert!(matches!(
        preview_effects.as_slice(),
        [EditorEffect::PreviewPointMove { point, .. }] if *point == first
    ));
    assert!(
        coordinator
            .projected_drag_work_evidence()
            .is_some_and(|work| work.accepted),
        "projected work: {:?}",
        coordinator.projected_drag_work_evidence()
    );
    let projected = {
        let document = coordinator
            .solved_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
            .expect("accepted projected preview")
            .document();
        [first, second].map(|point| document.point(point).expect("projected point").position)
    };
    assert!(
        (projected[0][0] - projected[1][0]).hypot(projected[0][1] - projected[1][1]) <= 1.0e-9,
        "Coincident projection must solve both recipe owners: {projected:?}"
    );
    let preview_scene = visible_scene(&coordinator, viewport);
    let expected = coordinator.session().design_identity();
    let release = coordinator.editor_mut().pointer_up_current_sample(
        &preview_scene,
        expected,
        pointer(pointer_id, terminal),
    );
    let [
        commit @ EditorEffect::CommitPointMove {
            point: released_point,
            model_position,
            ..
        },
    ] = release.as_slice()
    else {
        panic!("strict terminal release did not publish one point commit: {release:?}")
    };
    assert_eq!(*released_point, first);
    assert_eq!(
        model_position.map(f64::to_bits),
        projected[0].map(f64::to_bits)
    );
    coordinator
        .apply_editor_effect(commit)
        .expect("projected release")
        .expect("one owner rewrite publication");
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (
            history_before_projection.0 + 1,
            history_before_projection.1 + 1,
        ),
        "strict terminal publication must add exactly one authoritative history position",
    );

    let first_after = coordinator
        .lineage_document()
        .step(first_owner)
        .expect("rewritten first owner")
        .clone();
    let second_after = coordinator
        .lineage_document()
        .step(second_owner)
        .expect("rewritten second owner")
        .clone();
    assert_owner_manifest_unchanged(&first_before, &first_after);
    assert_owner_manifest_unchanged(&second_before, &second_after);
    assert_ne!(first_after.action, first_before.action);
    assert_ne!(second_after.action, second_before.action);
    assert_eq!(
        coordinator
            .lineage_document()
            .step(relation_owner)
            .expect("unchanged relation owner"),
        &relation_before
    );
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(relation)
            .is_some(),
        "stable materialized relation identity must survive"
    );
    let positions_after = accepted_positions(&coordinator, [first, second]);
    assert_ne!(positions_after, positions_before);
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("strict terminal projection must remain accepted");
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
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );

    let rejection_lineage = coordinator.lineage_document().clone();
    let rejection_lineage_identity = coordinator.lineage_identity();
    let rejection_history = (coordinator.history_len(), coordinator.history_cursor());
    let rejection_accepted = coordinator.checkpoint().accepted_json().map(str::to_owned);
    let rejection_positions = accepted_positions(&coordinator, [first, second]);
    let rejected_pointer = pointer_id + 1;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(first)]);
    let rejection_scene = visible_scene(&coordinator, viewport);
    let rejection_press = rejection_scene
        .points
        .iter()
        .find(|candidate| candidate.id == first)
        .expect("rewritten point in accepted scene")
        .screen_position;
    assert!(
        coordinator
            .pointer_down(&rejection_scene, pointer(rejected_pointer, rejection_press),)
            .is_empty()
    );
    let valid_sample = viewport.model_to_screen([5.0, 4.0]);
    let valid_request = coordinator
        .editor_mut()
        .pointer_move(&rejection_scene, pointer(rejected_pointer, valid_sample));
    let [
        EditorEffect::RequestProjectedPointMove {
            request_id: valid_request_id,
            point: valid_point,
            model_position: valid_position,
            ..
        },
    ] = valid_request.as_slice()
    else {
        panic!("expected a valid pre-terminal request: {valid_request:?}")
    };
    assert!(matches!(
        coordinator
            .resolve_projected_point_move(
                rejected_pointer,
                *valid_request_id,
                *valid_point,
                *valid_position,
            )
            .as_slice(),
        [EditorEffect::PreviewPointMove { .. }]
    ));
    let rejected_sample = viewport.model_to_screen([6.0, 5.0]);
    let rejected_request = coordinator
        .editor_mut()
        .pointer_move(&rejection_scene, pointer(rejected_pointer, rejected_sample));
    let [
        EditorEffect::RequestProjectedPointMove {
            request_id: rejected_request_id,
            point: rejected_point,
            ..
        },
    ] = rejected_request.as_slice()
    else {
        panic!("expected one rejected terminal request: {rejected_request:?}")
    };
    assert!(
        coordinator
            .resolve_projected_point_move(
                rejected_pointer,
                *rejected_request_id,
                *rejected_point,
                [f64::NAN, 0.0],
            )
            .is_empty(),
        "non-finite terminal projection must not publish a preview",
    );
    let retained_scene = visible_scene(&coordinator, viewport);
    let expected = coordinator.session().design_identity();
    assert_eq!(
        coordinator.editor_mut().pointer_up_current_sample(
            &retained_scene,
            expected,
            pointer(rejected_pointer, rejected_sample),
        ),
        vec![EditorEffect::ClearPointPreview],
        "strict release must consume rejection instead of borrowing the older valid preview",
    );
    coordinator.clear_transient();
    assert_eq!(coordinator.lineage_document(), &rejection_lineage);
    assert_eq!(coordinator.lineage_identity(), rejection_lineage_identity);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        rejection_history,
    );
    assert_eq!(
        coordinator.checkpoint().accepted_json(),
        rejection_accepted.as_deref(),
    );
    assert_eq!(
        accepted_positions(&coordinator, [first, second]),
        rejection_positions,
    );
    assert!(coordinator.solved_preview_session().is_none());

    coordinator.undo().expect("Undo projected owner rewrite");
    assert_eq!(
        accepted_positions(&coordinator, [first, second]),
        positions_before
    );
    assert_eq!(
        step_order(&coordinator),
        vec![baseline, second_owner, first_owner, relation_owner]
    );
    coordinator.undo().expect("Undo reorder");
    assert_eq!(
        step_order(&coordinator),
        vec![baseline, first_owner, second_owner, relation_owner]
    );
    coordinator.redo().expect("Redo reorder");
    coordinator.redo().expect("Redo projected owner rewrite");
    assert_eq!(
        step_order(&coordinator),
        vec![baseline, second_owner, first_owner, relation_owner]
    );
    assert_eq!(
        accepted_positions(&coordinator, [first, second]),
        positions_after
    );

    let lineage_json = coordinator
        .lineage_session_json()
        .expect("reordered lineage session");
    let ledger_json = coordinator
        .lineage_host_input_ledger_json()
        .expect("reordered host-input ledger");
    let expected_steps = coordinator.lineage_document().steps().to_vec();
    let expected_history = (coordinator.history_len(), coordinator.history_cursor());
    let expected_accepted = coordinator.checkpoint().accepted_json().map(str::to_owned);
    let mut restored = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("same-document restore target");
    restored
        .restore_lineage_session_and_host_input_ledger_json(&lineage_json, &ledger_json)
        .expect("cold restore reordered projected lineage");
    assert_eq!(restored.lineage_document().steps(), expected_steps);
    assert_eq!(
        (restored.history_len(), restored.history_cursor()),
        expected_history
    );
    assert_eq!(
        restored.checkpoint().accepted_json(),
        expected_accepted.as_deref()
    );
    assert_eq!(
        accepted_positions(&restored, [first, second]),
        positions_after
    );
    assert!(
        restored
            .session()
            .design_document()
            .constraint(relation)
            .is_some()
    );
    restored.undo().expect("restored Undo projected rewrite");
    assert_eq!(
        accepted_positions(&restored, [first, second]),
        positions_before
    );
    restored.redo().expect("restored Redo projected rewrite");
    assert_eq!(
        accepted_positions(&restored, [first, second]),
        positions_after
    );
}

#[test]
fn reordered_explicit_branch_rewrites_the_same_stable_recipe_owner() {
    let mut coordinator = empty_coordinator();
    let (_, independent_owner) = add_point(&mut coordinator, [6.0, 2.0]);
    let hyperbola = coordinator
        .apply_construction(
            coordinator.session().design_identity(),
            &ConstructionProposal::Hyperbola {
                center: ConstructionPoint::New([0.0, 0.0]),
                transverse_axis_point: ConstructionPoint::New([2.5, 0.5]),
                semi_conjugate: 1.3,
                branch: DocumentHyperbolaBranch::Positive,
                trim_start: -1.1,
                trim_end: 1.4,
            },
        )
        .expect("authored hyperbola");
    let curve = hyperbola.value.curves[0];
    let branch_owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("hyperbola recipe owner")
        .id;
    let before = coordinator
        .lineage_document()
        .step(branch_owner)
        .expect("branch owner before reorder")
        .clone();

    let outcome = coordinator
        .reorder_lineage_step(
            coordinator.lineage_identity(),
            branch_owner,
            Some(independent_owner),
        )
        .expect("independent hyperbola reorder");
    assert!(outcome.changed);
    assert!(!outcome.clamped);
    assert_owner_manifest_unchanged(
        &before,
        coordinator
            .lineage_document()
            .step(branch_owner)
            .expect("reordered branch owner"),
    );

    coordinator.set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
    coordinator
        .set_curve_hyperbola_branch(
            coordinator.session().design_identity(),
            curve,
            DocumentHyperbolaBranch::Negative,
        )
        .expect("explicit branch rewrite after reorder");
    let after = coordinator
        .lineage_document()
        .step(branch_owner)
        .expect("branch owner after rewrite")
        .clone();
    assert_owner_manifest_unchanged(&before, &after);
    assert_ne!(after.action, before.action);
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .curve(curve)
            .expect("stable hyperbola ID")
            .definition,
        geosolve_sketch::CurveDefinition::HyperbolaSegment {
            branch: DocumentHyperbolaBranch::Negative,
            ..
        }
    ));

    coordinator.undo().expect("Undo branch rewrite");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(branch_owner)
            .expect("undone branch owner")
            .action,
        before.action
    );
    coordinator.redo().expect("Redo branch rewrite");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(branch_owner)
            .expect("redone branch owner"),
        &after
    );

    let lineage_json = coordinator
        .lineage_session_json()
        .expect("branch lineage session");
    let ledger_json = coordinator
        .lineage_host_input_ledger_json()
        .expect("branch host-input ledger");
    let mut restored = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("branch restore target");
    restored
        .restore_lineage_session_and_host_input_ledger_json(&lineage_json, &ledger_json)
        .expect("cold restore reordered branch lineage");
    assert_eq!(restored.lineage_document().step(branch_owner), Some(&after));
    assert_eq!(step_order(&restored), step_order(&coordinator));
}
