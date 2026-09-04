// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{
    ColdIntentMaterializer, IntentNativeBinding, ProjectionalIntentCoordinator,
};
use geosolve_sketch::{DesignPointId, DocumentId, OperationControl, PersistentId};
use geosolve_sketch_code::{
    CodeOwnerAddress, CodePointEdit, ExpandedCodeProject, ExpandedPort, ExpandedWritablePoint,
    KeyedReconcileState, ManagedPathSegment, SemanticOutputPath, bundled_sample,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{
    IntentPlanDisposition, IntentPortKind, IntentSession, IntentSessionId,
};
use serde::Deserialize;

const MECHANISMS: [&str; 5] = [
    "theo-jansen-leg",
    "whitworth-quick-return",
    "twin-roller-bezier-cam",
    "peaucellier-linkage",
    "five-stage-scissor-lift",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleWitnesses {
    format: String,
    #[serde(rename = "representative_edit")]
    _representative_edit: serde_json::Value,
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
