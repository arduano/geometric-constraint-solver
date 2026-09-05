// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{
    ColdIntentMaterializer, EditorEffect, IntentNativeBinding, Modifiers, PointerInput,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{DesignPointId, DocumentId, OperationControl, PersistentId};
use geosolve_sketch_code::{
    CodeOwnerAddress, CodePointEdit, CodeProject, CompiledManagedSource, ExpandedCodeProject,
    ExpandedPort, ExpandedWritablePoint, KeyedReconcileState, ManagedPathSegment, ProjectKey,
    SemanticOutputPath, bundled_sample, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{
    IntentPlanDisposition, IntentPortKind, IntentSession, IntentSessionId,
};
use serde::Deserialize;

const MECHANISMS: [&str; 4] = [
    "theo-jansen-leg",
    "whitworth-quick-return",
    "peaucellier-linkage",
    "five-stage-scissor-lift",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleWitnesses {
    format: String,
    #[serde(rename = "representative_edit")]
    _representative_edit: serde_json::Value,
    #[serde(rename = "secondary_edit")]
    _secondary_edit: serde_json::Value,
    drags: Vec<DragWitness>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DragWitness {
    name: String,
    declaration: String,
    output_path: Vec<String>,
    target: [f64; 2],
}

fn generated(project: &geosolve_sketch_code::CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated member reconciliation")
        .into_staged()
}

fn witness_path(witness: &DragWitness) -> SemanticOutputPath {
    SemanticOutputPath(
        witness
            .output_path
            .iter()
            .cloned()
            .map(ManagedPathSegment::Field)
            .collect(),
    )
}

fn witness_writable_point<'a>(
    expansion: &'a ExpandedCodeProject,
    witness: &DragWitness,
    context: &str,
) -> &'a ExpandedWritablePoint {
    let path = witness_path(witness);
    let matches = expansion
        .writable_points
        .iter()
        .filter(|point| {
            let CodePointEdit::Point { address } = &point.edit else {
                return false;
            };
            matches!(
                &address.owner.address,
                CodeOwnerAddress::DirectDeclaration { declaration }
                    if declaration.0 == witness.declaration
            ) && address.output == path
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{context}: witness must resolve to exactly one source-backed point"
    );
    matches[0]
}

fn native_point(
    coordinator: &ProjectionalIntentCoordinator,
    handle: &ExpandedPort,
    context: &str,
) -> DesignPointId {
    assert_eq!(handle.kind, IntentPortKind::Point, "{context}: point port");
    let node = coordinator
        .intent()
        .graph()
        .node_by_symbol(&handle.alias)
        .unwrap_or_else(|| panic!("{context}: expanded point alias is absent from Intent"));
    let port = node
        .port_by_selector(handle.selector)
        .unwrap_or_else(|| panic!("{context}: expanded point selector is absent from Intent"));
    assert_eq!(port.kind, handle.kind, "{context}: point port kind");
    match coordinator
        .accepted_materialization()
        .unwrap_or_else(|| panic!("{context}: accepted materialization"))
        .ownership
        .port(port.as_ref(node.id))
        .unwrap_or_else(|| panic!("{context}: native point binding"))
    {
        IntentNativeBinding::Point(point) => point,
        binding => panic!("{context}: witness resolved to {binding:?}, not a point"),
    }
}

fn point_position(
    coordinator: &ProjectionalIntentCoordinator,
    point: DesignPointId,
    context: &str,
) -> [f64; 2] {
    coordinator
        .accepted_materialization()
        .and_then(|accepted| accepted.session.accepted_state_for_current_input())
        .and_then(|accepted| accepted.document().point(point))
        .unwrap_or_else(|| panic!("{context}: accepted point"))
        .position
}

fn accepted_document_json(coordinator: &ProjectionalIntentCoordinator, context: &str) -> String {
    coordinator
        .accepted_materialization()
        .and_then(|accepted| accepted.session.accepted_state_for_current_input())
        .unwrap_or_else(|| panic!("{context}: accepted document"))
        .document()
        .to_draft_v5_json()
        .unwrap_or_else(|error| panic!("{context}: encode accepted document: {error}"))
}

fn pair_bits(point: [f64; 2]) -> [u64; 2] {
    point.map(f64::to_bits)
}

fn assert_accepted(
    coordinator: &ProjectionalIntentCoordinator,
    expected_raw_dof: usize,
    expected_effective_dof: usize,
    context: &str,
) {
    let materialized = coordinator
        .accepted_materialization()
        .unwrap_or_else(|| panic!("{context}: accepted materialization"));
    assert!(
        materialized.validation.hard_residuals_validated,
        "{context}: independent hard-residual validation"
    );
    assert!(
        materialized.validation.all_active_features_current,
        "{context}: every active computed feature is Current"
    );
    assert!(
        materialized
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
        "{context}: finite normalized hard residual <= 1e-9"
    );

    let accepted = materialized
        .session
        .accepted_state_for_current_input()
        .unwrap_or_else(|| panic!("{context}: accepted state belongs to current input"));
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .flat_map(|point| point.position)
            .chain(
                accepted
                    .document()
                    .scalars()
                    .iter()
                    .map(|scalar| scalar.value),
            )
            .all(f64::is_finite),
        "{context}: finite accepted geometry"
    );
    let diagnostics = accepted.diagnostics();
    assert_eq!(
        diagnostics
            .rank
            .and_then(|rank| rank.numerical_right_nullity),
        Some(expected_raw_dof),
        "{context}: numerical right nullity"
    );
    let mobility = diagnostics
        .mobility
        .unwrap_or_else(|| panic!("{context}: mobility diagnostics"));
    assert_eq!(
        mobility.equality_degrees_of_freedom,
        Some(expected_raw_dof),
        "{context}: equality DOF"
    );
    assert_eq!(
        mobility.bidirectional_bounded_degrees_of_freedom,
        Some(expected_effective_dof),
        "{context}: bidirectional bounded DOF"
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete retained gesture keeps preview, commit, independent validity, Undo, Redo and cold-restore evidence adjacent"
)]
fn exercise_witness(sample_key: &str, witness: &DragWitness, seed: u128) {
    let sample = bundled_sample(sample_key).expect("registered mechanism sample");
    let project = sample.project();
    let reconciliation = generated(&project);
    let document = DocumentId(PersistentId::from_u128(seed));
    let intent_session = IntentSessionId::from_raw(seed ^ 0x9200_0000);
    let materialized =
        materialize_code_project_cold(&project, &reconciliation, intent_session, document, 1.0)
            .unwrap_or_else(|error| panic!("{sample_key}/{}: {error}", witness.name));
    let context = format!("{sample_key}/{}", witness.name);
    let writable = witness_writable_point(&materialized.expansion, witness, &context);

    // Start from the exact ordinary managed materialization, but exercise the
    // gesture on the public retained coordinator boundary requested by M92-C3.
    let initial_intent = materialized
        .editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .unwrap_or_else(|error| panic!("{context}: encode initial Intent: {error}"));
    let mut coordinator = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(&initial_intent)
            .unwrap_or_else(|error| panic!("{context}: decode initial Intent: {error}")),
        ColdIntentMaterializer::with_default_policy(document, 1.0)
            .unwrap_or_else(|error| panic!("{context}: cold materializer: {error}")),
    )
    .unwrap_or_else(|error| panic!("{context}: restore managed coordinator: {error}"));
    let point = native_point(&coordinator, &writable.handle, &context);
    let expected_raw_dof = sample.expected.numerical_right_nullity();
    let expected_effective_dof = sample.expected.bidirectional_bounded_degrees_of_freedom();
    assert_accepted(
        &coordinator,
        expected_raw_dof,
        expected_effective_dof,
        &format!("{context} initial"),
    );

    let graph_before = coordinator.intent().graph().clone();
    let document_before = accepted_document_json(&coordinator, &format!("{context} initial"));
    let position_before = point_position(&coordinator, point, &context);
    assert!(
        witness.target.into_iter().all(f64::is_finite),
        "{context}: finite witness target"
    );

    let pointer_id = u64::try_from(seed).expect("bounded witness seed");
    coordinator
        .begin_point_drag(pointer_id, point)
        .unwrap_or_else(|error| panic!("{context}: begin drag: {error}"));
    let preview = coordinator
        .preview_point_drag(pointer_id, 1, witness.target, OperationControl::unlimited())
        .unwrap_or_else(|error| panic!("{context}: preview drag: {error}"))
        .unwrap_or_else(|| panic!("{context}: witness produced no accepted preview"));
    assert!(
        preview.accepted_position.into_iter().all(f64::is_finite),
        "{context}: finite preview terminal"
    );
    assert_ne!(
        pair_bits(preview.accepted_position),
        pair_bits(position_before),
        "{context}: witness must move its declared point"
    );
    assert_eq!(
        coordinator.intent().graph(),
        &graph_before,
        "{context}: preview cannot change explicit branch/contact authority"
    );

    let outcome = coordinator
        .finish_point_drag(pointer_id, 1)
        .unwrap_or_else(|error| panic!("{context}: finish drag: {error}"));
    assert_eq!(
        outcome.disposition,
        IntentPlanDisposition::Accepted,
        "{context}: terminal publication"
    );
    assert_eq!(
        coordinator.intent().graph(),
        &graph_before,
        "{context}: terminal cannot change explicit branch/contact authority"
    );
    let terminal_position = point_position(&coordinator, point, &context);
    assert_eq!(
        pair_bits(terminal_position),
        pair_bits(preview.accepted_position),
        "{context}: committed point is the exact accepted preview"
    );
    assert_accepted(
        &coordinator,
        expected_raw_dof,
        expected_effective_dof,
        &format!("{context} terminal"),
    );
    let document_terminal = accepted_document_json(&coordinator, &format!("{context} terminal"));
    assert_ne!(
        document_terminal, document_before,
        "{context}: terminal must publish a changed accepted document"
    );
    let terminal_intent = coordinator
        .intent()
        .to_canonical_json()
        .unwrap_or_else(|error| panic!("{context}: encode terminal Intent: {error}"));

    coordinator
        .undo()
        .unwrap_or_else(|error| panic!("{context}: Undo: {error}"))
        .unwrap_or_else(|| panic!("{context}: missing Undo receipt"));
    assert_eq!(
        coordinator.intent().graph(),
        &graph_before,
        "{context}: Undo branch/contact authority"
    );
    assert_eq!(
        accepted_document_json(&coordinator, &format!("{context} Undo")),
        document_before,
        "{context}: Undo restores the exact accepted document"
    );
    assert_eq!(
        pair_bits(point_position(&coordinator, point, &context)),
        pair_bits(position_before),
        "{context}: Undo restores exact point position"
    );
    assert_accepted(
        &coordinator,
        expected_raw_dof,
        expected_effective_dof,
        &format!("{context} Undo"),
    );

    coordinator
        .redo()
        .unwrap_or_else(|error| panic!("{context}: Redo: {error}"))
        .unwrap_or_else(|| panic!("{context}: missing Redo receipt"));
    assert_eq!(
        coordinator.intent().graph(),
        &graph_before,
        "{context}: Redo branch/contact authority"
    );
    assert_eq!(
        accepted_document_json(&coordinator, &format!("{context} Redo")),
        document_terminal,
        "{context}: Redo restores the exact accepted document"
    );
    assert_eq!(
        pair_bits(point_position(&coordinator, point, &context)),
        pair_bits(terminal_position),
        "{context}: Redo restores exact terminal point"
    );
    assert_accepted(
        &coordinator,
        expected_raw_dof,
        expected_effective_dof,
        &format!("{context} Redo"),
    );

    let restored = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(&terminal_intent)
            .unwrap_or_else(|error| panic!("{context}: decode terminal Intent: {error}")),
        ColdIntentMaterializer::with_default_policy(document, 1.0)
            .unwrap_or_else(|error| panic!("{context}: restore materializer: {error}")),
    )
    .unwrap_or_else(|error| panic!("{context}: cold restore terminal: {error}"));
    assert_eq!(
        restored.intent().to_canonical_json().unwrap(),
        terminal_intent,
        "{context}: canonical persistence restore"
    );
    assert_eq!(
        restored.intent().graph(),
        &graph_before,
        "{context}: restored branch/contact authority"
    );
    assert_eq!(
        accepted_document_json(&restored, &format!("{context} restored")),
        document_terminal,
        "{context}: restore reproduces the exact accepted document"
    );
    assert_eq!(
        pair_bits(point_position(&restored, point, &context)),
        pair_bits(terminal_position),
        "{context}: restore reproduces exact terminal point"
    );
    assert_accepted(
        &restored,
        expected_raw_dof,
        expected_effective_dof,
        &format!("{context} restored"),
    );
}

#[test]
fn every_mechanism_drag_witness_is_retained_reversible_and_cold_restorable() {
    let mut witness_count = 0_usize;
    for (sample_index, sample_key) in MECHANISMS.into_iter().enumerate() {
        let sample = bundled_sample(sample_key).expect("registered mechanism sample");
        let witnesses: SampleWitnesses = serde_json::from_str(sample.witnesses_json())
            .unwrap_or_else(|error| panic!("{sample_key}: parse witnesses: {error}"));
        assert_eq!(
            witnesses.format, "geosolve-sample-witnesses-v1",
            "{sample_key}: witness format"
        );
        assert!(
            !witnesses.drags.is_empty(),
            "{sample_key}: mechanism must declare an intended drag"
        );
        for (drag_index, witness) in witnesses.drags.iter().enumerate() {
            let sample_ordinal = u128::try_from(sample_index).expect("bounded sample index");
            let drag_ordinal = u128::try_from(drag_index).expect("bounded drag index");
            let seed = 0x9230_0000 + sample_ordinal * 0x100 + drag_ordinal + 1;
            exercise_witness(sample_key, witness, seed);
            witness_count += 1;
        }
    }
    assert!(witness_count >= MECHANISMS.len());
}

fn bounded_pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn bounded_viewport(editor: &ProjectionalEditorSession) -> Viewport {
    let document = editor
        .presentation_session()
        .unwrap()
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let mut lower = [f64::INFINITY; 2];
    let mut upper = [f64::NEG_INFINITY; 2];
    for point in document.points() {
        for axis in 0..2 {
            lower[axis] = lower[axis].min(point.position[axis]);
            upper[axis] = upper[axis].max(point.position[axis]);
        }
    }
    let scale =
        (800.0 / (upper[0] - lower[0]).max(1.0)).min(500.0 / (upper[1] - lower[1]).max(1.0));
    Viewport::new(
        [1000.0, 700.0],
        [lower[0].midpoint(upper[0]), lower[1].midpoint(upper[1])],
        scale,
    )
    .unwrap()
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "keeps all twelve real pointer frames and their independent acceptance checks together"
)]
fn bounded_preview_frames(
    editor: &mut ProjectionalEditorSession,
    point: DesignPointId,
    viewport: Viewport,
    target: [f64; 2],
    pointer_id: u64,
    expected_dof: usize,
    context: &str,
) -> [f64; 2] {
    let intent_before = editor.coordinator().intent().to_canonical_json().unwrap();
    let origin = point_position(editor.coordinator(), point, context);
    let start = viewport.model_to_screen(origin);
    let end = viewport.model_to_screen(target);
    assert!(
        (end.x - start.x).hypot(end.y - start.y) > 6.0,
        "{context}: witness must exceed the ordinary pointer threshold"
    );
    let scene = editor.scene(viewport, 0.25).unwrap();
    editor
        .pointer_down(&scene, bounded_pointer(pointer_id, start))
        .unwrap_or_else(|error| panic!("{context}: pointer down: {error}"));
    assert_eq!(
        editor.editor().selection(),
        &[SelectionItem::Point(point)],
        "{context}: the normal scene picker must select the declared driver"
    );
    let mut attempts = 0;
    let mut rejected = Vec::new();
    let mut terminal = origin;
    for step in 1..=12 {
        let fraction = f64::from(step) / 12.0;
        let cursor = ScreenPoint {
            x: (end.x - start.x).mul_add(fraction, start.x),
            y: (end.y - start.y).mul_add(fraction, start.y),
        };
        let before = editor
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .clone();
        let scene = editor.scene(viewport, 0.25).unwrap();
        let audited = editor.pointer_move_audited(&scene, bounded_pointer(pointer_id, cursor));
        assert_eq!(
            audited.work.intent_materialization_attempts(),
            0,
            "{context}"
        );
        assert_eq!(audited.work.history_publications(), 0, "{context}");
        assert!(audited.work.native_preview_attempts() <= 1, "{context}");
        let effects = audited
            .outcome
            .unwrap_or_else(|error| panic!("{context}: pointer move {step}: {error}"));
        let preview = effects.iter().find_map(|effect| match effect {
            EditorEffect::PreviewPointMove {
                point: moved,
                model_position,
            } if *moved == point => Some(*model_position),
            _ => None,
        });
        let state = editor
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap();
        if audited.work.native_preview_attempts() == 1 {
            attempts += 1;
            if let Some(preview) = preview {
                assert_eq!(
                    pair_bits(state.document().point(point).unwrap().position),
                    pair_bits(preview),
                    "{context}: accepted effect and authoritative preview agree"
                );
                terminal = preview;
            } else {
                rejected.push(step);
                assert_eq!(
                    state.document(),
                    &before,
                    "{context}: rejected frame retention"
                );
            }
        }
        assert!(state.solve_result().rejection.is_none(), "{context}");
        assert!(
            state
                .solve_result()
                .acceptance_hard_residual_max
                .is_some_and(|value| value.is_finite() && value <= 1.0e-9),
            "{context}: independently validated Hard residuals"
        );
        assert!(
            state
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .chain(state.document().scalars().iter().map(|scalar| scalar.value))
                .all(f64::is_finite),
            "{context}: finite accepted preview"
        );
        assert_eq!(
            state.diagnostics().rank.unwrap().numerical_right_nullity,
            Some(expected_dof),
            "{context}: preview mobility"
        );
        let mobility = state.diagnostics().mobility.unwrap();
        assert_eq!(mobility.equality_degrees_of_freedom, Some(expected_dof));
        assert_eq!(
            mobility.bidirectional_bounded_degrees_of_freedom,
            Some(expected_dof),
            "{context}: ordinary witnesses stay inside their usable stroke"
        );
        assert_eq!(
            editor.coordinator().intent().to_canonical_json().unwrap(),
            intent_before,
            "{context}: preview preserves complete intent/history/branch state"
        );
    }
    assert!(
        attempts > 0,
        "{context}: no native preview request crossed the pointer boundary"
    );
    assert!(
        rejected.is_empty(),
        "{context}: default bounded policy rejected frames {rejected:?} of {attempts} attempts"
    );
    assert!(
        distance(terminal, origin) > 1.0e-6,
        "{context}: driver never moved"
    );
    assert!(
        distance(terminal, target) < distance(origin, target),
        "{context}: constrained driver must approach its cursor target"
    );
    terminal
}

#[allow(
    clippy::too_many_lines,
    reason = "keeps the bounded gesture, exact terminal, Undo/Redo and cold reload together"
)]
fn exercise_bounded_witness(sample_key: &str, witness: &DragWitness, seed: u128) {
    let context = format!("{sample_key}/{}", witness.name);
    let sample = bundled_sample(sample_key).unwrap();
    let project = sample.project();
    let document_id = DocumentId(PersistentId::from_u128(seed));
    let materialized = materialize_code_project_cold(
        &project,
        &generated(&project),
        IntentSessionId::from_raw(seed),
        document_id,
        1.0,
    )
    .unwrap_or_else(|error| panic!("{context}: materialization: {error}"));
    let writable = witness_writable_point(&materialized.expansion, witness, &context);
    // This constructor selects the production default bounded geometry policy.
    // No copied limit constants or unlimited preview controls enter this test.
    let mut editor = ProjectionalEditorSession::restore(
        IntentSession::from_json(
            &materialized
                .editor
                .coordinator()
                .intent()
                .to_canonical_json()
                .unwrap(),
        )
        .unwrap(),
        document_id,
        1.0,
    )
    .unwrap();
    let point = native_point(editor.coordinator(), &writable.handle, &context);
    let viewport = bounded_viewport(&editor);
    let before = accepted_document_json(editor.coordinator(), &context);
    let undo_before = editor.coordinator().intent().undo_len();
    let graph_before = editor.coordinator().intent().graph().clone();
    let pointer_id = u64::try_from(seed).unwrap();
    let terminal = bounded_preview_frames(
        &mut editor,
        point,
        viewport,
        witness.target,
        pointer_id,
        sample.expected.numerical_right_nullity(),
        &context,
    );
    let scene = editor.scene(viewport, 0.25).unwrap();
    let committed = editor.pointer_up_audited(
        &scene,
        bounded_pointer(pointer_id, viewport.model_to_screen(witness.target)),
    );
    assert_eq!(
        committed.work.history_publications(),
        1,
        "{context}: one gesture transaction"
    );
    let committed = committed.outcome.unwrap();
    assert_eq!(
        committed.transaction.unwrap().disposition,
        IntentPlanDisposition::Accepted,
        "{context}: exact terminal publication"
    );
    assert_eq!(
        pair_bits(point_position(editor.coordinator(), point, &context)),
        pair_bits(terminal),
        "{context}: commit preserves the last accepted preview"
    );
    assert_eq!(editor.coordinator().intent().undo_len(), undo_before + 1);
    assert_eq!(editor.coordinator().intent().graph(), &graph_before);
    assert_accepted(
        editor.coordinator(),
        sample.expected.numerical_right_nullity(),
        sample.expected.bidirectional_bounded_degrees_of_freedom(),
        &context,
    );
    let after = accepted_document_json(editor.coordinator(), &context);
    editor.undo().unwrap().unwrap();
    assert_eq!(
        accepted_document_json(editor.coordinator(), &context),
        before
    );
    editor.redo().unwrap().unwrap();
    assert_eq!(
        accepted_document_json(editor.coordinator(), &context),
        after
    );
    let persisted = editor.coordinator().intent().to_canonical_json().unwrap();
    let restored = ProjectionalEditorSession::restore(
        IntentSession::from_json(&persisted).unwrap(),
        document_id,
        1.0,
    )
    .unwrap();
    assert_eq!(
        restored.coordinator().intent().to_canonical_json().unwrap(),
        persisted
    );
    assert_eq!(
        accepted_document_json(restored.coordinator(), &context),
        after
    );
    assert_accepted(
        restored.coordinator(),
        sample.expected.numerical_right_nullity(),
        sample.expected.bidirectional_bounded_degrees_of_freedom(),
        &context,
    );
}

#[test]
fn every_mechanism_witness_accepts_default_bounded_pointer_frames() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (sample_index, key) in MECHANISMS.into_iter().enumerate() {
        let witnesses =
            serde_json::from_str::<SampleWitnesses>(bundled_sample(key).unwrap().witnesses_json());
        let witnesses = match witnesses {
            Ok(witnesses)
                if witnesses.format == "geosolve-sample-witnesses-v1"
                    && !witnesses.drags.is_empty() =>
            {
                witnesses
            }
            other => {
                failures.push(format!("{key}: invalid witness inventory: {other:?}"));
                continue;
            }
        };
        for (drag_index, witness) in witnesses.drags.iter().enumerate() {
            checked += 1;
            let seed = 0x9294_0000
                + u128::try_from(sample_index).unwrap() * 0x100
                + u128::try_from(drag_index).unwrap();
            let result = std::panic::catch_unwind(|| exercise_bounded_witness(key, witness, seed));
            if let Err(panic) = result {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| {
                        panic
                            .downcast_ref::<&str>()
                            .map(|message| (*message).to_owned())
                    })
                    .unwrap_or_else(|| "non-text panic".into());
                eprintln!("FAIL bounded {key}/{}: {message}", witness.name);
                failures.push(format!("{key}/{}: {message}", witness.name));
            } else {
                eprintln!("PASS bounded {key}/{}", witness.name);
            }
        }
    }
    assert!(
        checked >= MECHANISMS.len(),
        "complete public mechanism inventory"
    );
    assert!(
        failures.is_empty(),
        "bounded pointer checklist:\n{}",
        failures.join("\n")
    );
}

fn named_point(
    coordinator: &ProjectionalIntentCoordinator,
    expansion: &ExpandedCodeProject,
    declaration: &str,
) -> DesignPointId {
    let witness = DragWitness {
        name: declaration.into(),
        declaration: declaration.into(),
        output_path: vec!["point".into()],
        target: [0.0, 0.0],
    };
    native_point(
        coordinator,
        &witness_writable_point(expansion, &witness, declaration).handle,
        declaration,
    )
}

fn distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - second[0]).hypot(first[1] - second[1])
}

#[test]
fn jansen_foot_is_not_a_rigid_rocker_about_the_frame_pivot() {
    let sample = bundled_sample("theo-jansen-leg").unwrap();
    let project = sample.project();
    let materialized = materialize_code_project_cold(
        &project,
        &generated(&project),
        IntentSessionId::from_raw(0x9291),
        DocumentId(PersistentId::from_u128(0x9291)),
        1.0,
    )
    .unwrap();
    let expansion = materialized.expansion.clone();
    let mut retained = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(
            &materialized
                .editor
                .coordinator()
                .intent()
                .to_canonical_json()
                .unwrap(),
        )
        .unwrap(),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(0x9291)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap();
    let coordinator = &mut retained;
    let crank = named_point(coordinator, &expansion, "crankPin");
    let ground = named_point(coordinator, &expansion, "groundPivot");
    let frame = named_point(coordinator, &expansion, "framePivot");
    let foot = named_point(coordinator, &expansion, "foot");
    let origin = point_position(coordinator, ground, "ground");
    let start = point_position(coordinator, crank, "crank");
    let radius = distance(origin, start);
    let angle = (start[1] - origin[1]).atan2(start[0] - origin[0]);
    let mut radii = Vec::new();
    for step in 1..=16 {
        let theta = angle + f64::from(step) * 0.025;
        let target = [
            origin[0] + radius * theta.cos(),
            origin[1] + radius * theta.sin(),
        ];
        coordinator.begin_point_drag(1, crank).unwrap();
        let preview = coordinator
            .preview_point_drag(1, 1, target, OperationControl::unlimited())
            .unwrap()
            .unwrap();
        assert!(distance(preview.accepted_position, target) < 1.0e-5);
        coordinator.finish_point_drag(1, 1).unwrap();
        assert_accepted(coordinator, 1, 1, "Jansen local trajectory");
        radii.push(distance(
            point_position(coordinator, foot, "foot"),
            point_position(coordinator, frame, "frame"),
        ));
    }
    let minimum = radii.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = radii.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "Jansen foot/frame radii: {radii:?}; spread {}",
        maximum - minimum
    );
    assert!(
        maximum - minimum > 0.1,
        "a walking-leg output must articulate independently of a rigid frame rocker"
    );
}

// Model-unit design checks are independent of the sample's manifest and of the
// solver's success status. Every trajectory uses ordinary retained previews.
#[allow(
    clippy::too_many_lines,
    reason = "retained trajectory, validation, history and durable evidence share one exact authority"
)]
fn trace_mechanism(
    sample_key: &str,
    driver: &str,
    names: &[&str],
    targets: &[[f64; 2]],
    dof: usize,
) -> Vec<Vec<[f64; 2]>> {
    let project = if sample_key == "twin-roller-bezier-cam" {
        // Catalog curation must not discard the independent tangent/locality regression.
        let compiled = CompiledManagedSource::from_json(include_str!(
            "fixtures/twin-roller-bezier-cam/sketch.compiled.json"
        ))
        .expect("private cam compiler envelope");
        compiled
            .validate_input_source(include_str!("fixtures/twin-roller-bezier-cam/sketch.ts"))
            .expect("private cam source authority");
        CodeProject::managed(
            ProjectKey("regression-twin-roller-bezier-cam".into()),
            compiled,
        )
        .expect("private cam project")
    } else {
        bundled_sample(sample_key).unwrap().project()
    };
    let document = DocumentId(PersistentId::from_u128(0x9292));
    let materialized = materialize_code_project_cold(
        &project,
        &generated(&project),
        IntentSessionId::from_raw(0x9292),
        document,
        1.0,
    )
    .unwrap();
    let expansion = materialized.expansion;
    let mut coordinator = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(
            &materialized
                .editor
                .coordinator()
                .intent()
                .to_canonical_json()
                .unwrap(),
        )
        .unwrap(),
        ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
    )
    .unwrap();
    let point = named_point(&coordinator, &expansion, driver);
    let ids = names
        .iter()
        .map(|name| named_point(&coordinator, &expansion, name))
        .collect::<Vec<_>>();
    let graph = coordinator.intent().graph().clone();
    let before = accepted_document_json(&coordinator, sample_key);
    let mut frames = Vec::new();
    let mut evidence = Vec::new();
    coordinator.begin_point_drag(1, point).unwrap();
    for (index, target) in targets.iter().copied().enumerate() {
        let request = u64::try_from(index + 1).unwrap();
        let preview = coordinator
            .preview_point_drag(1, request, target, OperationControl::unlimited())
            .unwrap_or_else(|error| panic!("{sample_key} sample {index}: {error}"))
            .unwrap_or_else(|| panic!("{sample_key} sample {index}: no accepted preview"));
        let tracking = distance(preview.accepted_position, target);
        eprintln!(
            "{sample_key} sample {index}: target {target:?}, accepted {:?}, tracking {tracking}",
            preview.accepted_position
        );
        assert!(
            tracking < 1.0e-5,
            "{sample_key} sample {index}: reachable driver must track within 1e-5 mm, actual {tracking}"
        );
        let state = coordinator
            .presentation_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap();
        let result = state.solve_result();
        assert!(
            result.rejection.is_none(),
            "{sample_key}: accepted preview rejection"
        );
        assert!(
            result
                .acceptance_hard_residual_max
                .is_some_and(|v| v.is_finite() && v <= 1.0e-9)
        );
        assert!(
            state
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .chain(state.document().scalars().iter().map(|s| s.value))
                .all(f64::is_finite)
        );
        let diagnostics = state.diagnostics();
        assert_eq!(diagnostics.rank.unwrap().numerical_right_nullity, Some(dof));
        assert_eq!(
            diagnostics
                .mobility
                .unwrap()
                .bidirectional_bounded_degrees_of_freedom,
            Some(dof)
        );
        assert_eq!(
            coordinator.intent().graph(),
            &graph,
            "{sample_key}: explicit branch/contact graph"
        );
        let positions = ids
            .iter()
            .map(|id| state.document().point(*id).unwrap().position)
            .collect::<Vec<_>>();
        evidence.push(serde_json::json!({"index": index, "target": target, "names": names, "points": positions, "residual": result.acceptance_hard_residual_max, "document": serde_json::from_str::<serde_json::Value>(&state.document().to_draft_v5_json().unwrap()).unwrap()}));
        frames.push(positions);
    }
    coordinator
        .finish_point_drag(1, u64::try_from(targets.len()).unwrap())
        .unwrap();
    assert_accepted(&coordinator, dof, dof, "trajectory terminal");
    let terminal = accepted_document_json(&coordinator, sample_key);
    let persisted = coordinator.intent().to_canonical_json().unwrap();
    coordinator.undo().unwrap().unwrap();
    assert_eq!(accepted_document_json(&coordinator, sample_key), before);
    coordinator.redo().unwrap().unwrap();
    assert_eq!(accepted_document_json(&coordinator, sample_key), terminal);
    let restored = ProjectionalIntentCoordinator::restore(
        IntentSession::from_json(&persisted).unwrap(),
        ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
    )
    .unwrap();
    assert_eq!(accepted_document_json(&restored, sample_key), terminal);
    if let Ok(directory) = std::env::var("M92_MECHANISM_EVIDENCE") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            format!("{directory}/{sample_key}-{driver}-trajectory.json"),
            serde_json::to_string(&evidence).unwrap(),
        )
        .unwrap();
    }
    frames
}

fn crank_cycle(origin: [f64; 2], radius: f64, initial: f64) -> Vec<[f64; 2]> {
    (1..=72)
        .chain((1..72).rev())
        .map(|step| {
            let angle = initial + f64::from(step) * std::f64::consts::TAU / 72.0;
            [
                origin[0] + radius * angle.cos(),
                origin[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

#[test]
fn whitworth_completes_a_crank_cycle_and_has_one_quick_return_stroke() {
    let frames = trace_mechanism(
        "whitworth-quick-return",
        "crankPin",
        &[
            "crankPivot",
            "rockerPivot",
            "crankPin",
            "rockerEnd",
            "ramPin",
        ],
        &crank_cycle([0.0, 0.0], 8.0_f64.sqrt(), std::f64::consts::FRAC_PI_4),
        1,
    );
    let mut xs = Vec::new();
    for frame in &frames {
        assert!((distance(frame[0], frame[2]) - 8.0_f64.sqrt()).abs() < 1.0e-6);
        assert!((distance(frame[1], frame[3]) - 117.0_f64.sqrt()).abs() < 1.0e-6);
        assert!((distance(frame[3], frame[4]) - 8.0).abs() < 1.0e-6);
        assert!((frame[4][1] - 6.0).abs() < 1.0e-8);
        xs.push(frame[4][0]);
    }
    let cycle = &xs[..72];
    let maximum = cycle
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    let minimum = cycle
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    assert!(
        maximum.1 - minimum.1 > 10.0,
        "useful ram stroke exceeds 10 mm"
    );
    let half = maximum.0.abs_diff(minimum.0);
    let longer = half.max(72 - half);
    let shorter = half.min(72 - half);
    assert!(
        (45..=52).contains(&longer) && (20..=27).contains(&shorter),
        "quick-return crank intervals: {longer} vs {shorter} five-degree steps"
    );
    for i in 0..71 {
        assert!(
            (xs[i] - xs[142 - i]).abs() < 1.0e-5,
            "reverse follows same assembly"
        );
    }
}

fn signed_area(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

#[test]
fn jansen_complete_cycle_has_a_level_stance_lifted_return_and_same_assembly_in_reverse() {
    let frames = trace_mechanism(
        "theo-jansen-leg",
        "crankPin",
        &[
            "groundPivot",
            "framePivot",
            "crankPin",
            "upperKnee",
            "lowerKnee",
            "upperAnkle",
            "lowerAnkle",
            "foot",
        ],
        &crank_cycle([0.0, 0.0], 15.0, 0.0),
        1,
    );
    let lengths = [
        (0, 2, 15.0),
        (1, 3, 41.5),
        (2, 3, 50.0),
        (1, 4, 39.3),
        (2, 4, 61.9),
        (1, 5, 40.1),
        (3, 5, 55.8),
        (5, 6, 39.4),
        (4, 6, 36.7),
        (4, 7, 49.0),
        (6, 7, 65.7),
    ];
    for frame in &frames {
        assert_eq!(pair_bits(frame[0]), pair_bits([0.0, 0.0]));
        assert_eq!(pair_bits(frame[1]), pair_bits([-38.0, -7.8]));
        for (a, b, length) in lengths {
            assert!((distance(frame[a], frame[b]) - length).abs() < 1.0e-6);
        }
        for (a, b, c, sign) in [
            (1, 2, 3, 1.0),
            (1, 2, 4, -1.0),
            (1, 3, 5, 1.0),
            (5, 4, 6, -1.0),
            (4, 6, 7, 1.0),
        ] {
            assert!(
                signed_area(frame[a], frame[b], frame[c]) * sign > 1.0,
                "explicit assembled triangle never flips or approaches a toggle"
            );
        }
    }
    let cycle = &frames[..72];
    let ymin = cycle.iter().map(|p| p[7][1]).fold(f64::INFINITY, f64::min);
    let ymax = cycle
        .iter()
        .map(|p| p[7][1])
        .fold(f64::NEG_INFINITY, f64::max);
    let stance = cycle
        .iter()
        .filter(|p| p[7][1] < ymin + 1.0)
        .map(|p| p[7][0])
        .collect::<Vec<_>>();
    let xmin = stance.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = stance.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "Jansen stance {} degrees, horizontal travel {}, return lift {}",
        stance.len() * 5,
        xmax - xmin,
        ymax - ymin
    );
    assert!(
        stance.len() >= 30,
        "at least 150 crank degrees form the 1 mm stance band"
    );
    assert!(
        xmax - xmin > 55.0,
        "stance covers more than 55 mm horizontally"
    );
    assert!(
        ymax - ymin > 20.0,
        "return lifts more than 20 mm above stance"
    );
    for i in 0..71 {
        for (&forward, &reverse) in frames[i].iter().zip(&frames[142 - i]) {
            assert!(distance(forward, reverse) < 1.0e-5);
        }
    }
}

#[test]
fn peaucellier_output_obeys_the_inversion_identity_and_straight_line_in_both_directions() {
    // |OP| |OQ| = long_radius^2 - rhombus_side^2 = 16; the
    // radius-4 driver circle passes through O, hence Q.x = 16/(2*4)=2.
    let angles = (1..=9)
        .map(|i| 90.0 + f64::from(i) * 2.5)
        .chain((0..9).rev().map(|i| 90.0 + f64::from(i) * 2.5))
        .chain((1..=16).map(|i| 90.0 - f64::from(i) * 2.5))
        .chain((1..16).rev().map(|i| 90.0 - f64::from(i) * 2.5));
    let targets = angles
        .map(|a| {
            let t = a.to_radians();
            [4.0 + 4.0 * t.cos(), 4.0 * t.sin()]
        })
        .collect::<Vec<_>>();
    let frames = trace_mechanism(
        "peaucellier-linkage",
        "point3PeaucellierCircularInputP",
        &[
            "point1PeaucellierFixedOriginO",
            "point2PeaucellierInputCenterS",
            "point3PeaucellierCircularInputP",
            "point4PeaucellierStraightLineOutputQ",
            "point5PeaucellierShoulderB",
            "point6PeaucellierShoulderD",
        ],
        &targets,
        1,
    );
    let mut ys = Vec::new();
    for f in frames {
        assert!((distance(f[0], f[2]) * distance(f[0], f[3]) - 16.0).abs() < 1.0e-6);
        assert!((f[3][0] - 2.0).abs() < 1.0e-7);
        assert!(signed_area(f[0], f[2], f[4]) > 0.1 && signed_area(f[0], f[2], f[5]) < -0.1);
        for (a, b, l) in [
            (0, 4, 5.0),
            (0, 5, 5.0),
            (4, 2, 3.0),
            (2, 5, 3.0),
            (5, 3, 3.0),
            (3, 4, 3.0),
            (1, 2, 4.0),
        ] {
            assert!((distance(f[a], f[b]) - l).abs() < 1.0e-6);
        }
        ys.push(f[3][1]);
    }
    assert!(
        ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - ys.iter().copied().fold(f64::INFINITY, f64::min)
            > 2.0
    );
}

#[test]
fn five_scissor_stages_extend_together_and_reverse_without_lateral_drift() {
    // Width w and stage rise h satisfy w^2+h^2=10^2; all left
    // endpoints stay on x=-4 and total top rise is exactly 5h.
    let heights = (1..=12)
        .map(|i| 6.0 + f64::from(i) * 0.25)
        .chain((0..12).rev().map(|i| 6.0 + f64::from(i) * 0.25))
        .chain((1..=12).map(|i| 6.0 - f64::from(i) * 0.25))
        .chain((1..12).rev().map(|i| 6.0 - f64::from(i) * 0.25));
    let targets = heights.map(|h| [-4.0, 5.0 * h]).collect::<Vec<_>>();
    let names = (0..=5)
        .flat_map(|level| {
            [
                format!("point{}TowerLevel{level}Left", level * 2 + 1),
                format!("point{}TowerLevel{level}Right", level * 2 + 2),
            ]
        })
        .collect::<Vec<_>>();
    let refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let frames = trace_mechanism(
        "five-stage-scissor-lift",
        "point11TowerLevel5Left",
        &refs,
        &targets,
        1,
    );
    for f in frames {
        let rise = f[10][1] / 5.0;
        let width = f[1][0] + 4.0;
        assert!((width.hypot(rise) - 10.0).abs() < 1.0e-7);
        for level in 0..=5 {
            assert!((f[2 * level][0] + 4.0).abs() < 1.0e-7);
            assert!((f[2 * level + 1][0] - f[1][0]).abs() < 1.0e-7);
            assert!(
                (f[2 * level][1] - f64::from(u32::try_from(level).unwrap()) * rise).abs() < 1.0e-7
            );
            assert!((f[2 * level][1] - f[2 * level + 1][1]).abs() < 1.0e-7);
        }
    }
}

#[test]
fn cam_followers_keep_tangent_offset_and_passive_follower_locality() {
    for (driver, other, start, direction) in [
        (
            "point4LeftRollerCenter",
            "point5RightRollerCenter",
            0.25,
            1.0,
        ),
        (
            "point5RightRollerCenter",
            "point4LeftRollerCenter",
            0.75,
            -1.0,
        ),
    ] {
        let parameters = (1..=20)
            .map(|i| start + direction * f64::from(i) * 0.01)
            .chain(
                (0..20)
                    .rev()
                    .map(|i| start + direction * f64::from(i) * 0.01),
            )
            .chain((1..=15).map(|i| start - direction * f64::from(i) * 0.01))
            .chain(
                (1..15)
                    .rev()
                    .map(|i| start - direction * f64::from(i) * 0.01),
            );
        let targets = parameters
            .map(|t| {
                // Exact normal offset of C(t)=(-4+8t,8t(1-t)), radius 1.
                let slope = 1.0 - 2.0 * t;
                let norm = slope.hypot(1.0);
                [
                    -4.0 + 8.0 * t - slope / norm,
                    8.0 * t * (1.0 - t) + 1.0 / norm,
                ]
            })
            .collect::<Vec<_>>();
        let frames = trace_mechanism(
            "twin-roller-bezier-cam",
            driver,
            &[driver, other, "point1CamQ0", "point2CamQ1", "point3CamQ2"],
            &targets,
            2,
        );
        let passive = frames[0][1];
        for (f, target) in frames.iter().zip(targets) {
            assert!(distance(f[0], target) < 1.0e-5);
            assert!(
                distance(f[1], passive) < 1.0e-8,
                "the other 1-DOF follower must remain still"
            );
            assert_eq!(pair_bits(f[2]), pair_bits([-4.0, 0.0]));
            assert!(distance(f[3], [0.0, 4.0]) < 1.0e-9);
            assert_eq!(pair_bits(f[4]), pair_bits([4.0, 0.0]));
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "each bounded stroke retains its preview and reverse recovery in one lifecycle"
)]
fn bounded_mechanism_strokes_stop_before_degeneracy_and_recover_in_reverse() {
    for (key, driver) in [
        ("peaucellier-linkage", "point3PeaucellierCircularInputP"),
        ("five-stage-scissor-lift", "point11TowerLevel5Left"),
    ] {
        let project = bundled_sample(key).unwrap().project();
        let document = DocumentId(PersistentId::from_u128(0x9293));
        let materialized = materialize_code_project_cold(
            &project,
            &generated(&project),
            IntentSessionId::from_raw(0x9293),
            document,
            1.0,
        )
        .unwrap();
        let expansion = materialized.expansion;
        let mut coordinator = ProjectionalIntentCoordinator::restore(
            IntentSession::from_json(
                &materialized
                    .editor
                    .coordinator()
                    .intent()
                    .to_canonical_json()
                    .unwrap(),
            )
            .unwrap(),
            ColdIntentMaterializer::with_default_policy(document, 1.0).unwrap(),
        )
        .unwrap();
        let point = named_point(&coordinator, &expansion, driver);
        let graph = coordinator.intent().graph().clone();
        let circle = |degrees: f64| {
            let t = degrees.to_radians();
            [4.0 + 4.0 * t.cos(), 4.0 * t.sin()]
        };
        let (near, bound, outside, reverse) = if key == "peaucellier-linkage" {
            (circle(110.0), circle(115.0), circle(125.0), circle(105.0))
        } else {
            (
                [-4.0, 45.0],
                [-4.0, 99.0_f64.sqrt() * 5.0],
                [-4.0, 55.0],
                [-4.0, 43.0],
            )
        };
        coordinator.begin_point_drag(1, point).unwrap();
        let mut last = bound;
        for (index, target) in [near, bound, outside, reverse].into_iter().enumerate() {
            let preview = coordinator
                .preview_point_drag(
                    1,
                    u64::try_from(index + 1).unwrap(),
                    target,
                    OperationControl::unlimited(),
                )
                .unwrap();
            let Some(preview) = preview else {
                assert_eq!(
                    index, 2,
                    "{key}: reachable bound target {index} was rejected"
                );
                let state = coordinator
                    .presentation_session()
                    .unwrap()
                    .accepted_state_for_current_input()
                    .unwrap();
                assert!(distance(state.document().point(point).unwrap().position, last) < 1e-8);
                continue;
            };
            let expected = if index == 2 { bound } else { target };
            assert!(
                distance(preview.accepted_position, expected) < 1e-5,
                "{key}: bounded preview {index} {:?} versus {expected:?}",
                preview.accepted_position
            );
            let state = coordinator
                .presentation_session()
                .unwrap()
                .accepted_state_for_current_input()
                .unwrap();
            assert!(state.solve_result().rejection.is_none());
            assert!(
                state
                    .solve_result()
                    .acceptance_hard_residual_max
                    .is_some_and(|r| r.is_finite() && r <= 1e-9)
            );
            let mobility = state
                .diagnostics()
                .mobility
                .unwrap()
                .bidirectional_bounded_degrees_of_freedom;
            // Merely targeting the mathematical endpoint can leave a tiny
            // interior slack. Pushing beyond it must activate the bound.
            if index != 1 {
                assert_eq!(
                    mobility,
                    Some(usize::from(index != 2)),
                    "{key} sample {index}"
                );
            }
            assert_eq!(coordinator.intent().graph(), &graph);
            if index == 2 {
                assert!(
                    distance(last, preview.accepted_position) < 1e-7,
                    "unreachable drag retains the boundary geometry"
                );
            }
            last = preview.accepted_position;
        }
        coordinator.finish_point_drag(1, 4).unwrap();
        assert_accepted(&coordinator, 1, 1, "recovered bounded mechanism");
    }
}
