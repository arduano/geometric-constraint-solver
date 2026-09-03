// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{IntentNativeBinding, IntentNativeWritableLeaf};
use geosolve_sketch::{
    ContactAdmissibleRange, DocumentElementId, DocumentId, PersistentId,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchBoundStatus,
};
use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
    materialize_code_project_cold, materialize_code_project_incremental_for_structural_edit,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentPortRole, IntentPortSelector, IntentSessionId, NodeId};

const BASE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-base.json"
);
const LIMITED: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-limited.json"
);

fn project(envelope: &str) -> CodeProject {
    CodeProject::managed(
        ProjectKey("m91-contact-range-continuation".into()),
        CompiledManagedSource::from_json(envelope).expect("compiled managed source"),
    )
    .expect("managed project")
}

fn node_by_symbol(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    symbol: &str,
) -> NodeId {
    let alias = materialized
        .expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| (declaration.0 == symbol).then_some(alias))
        .expect("managed declaration alias");
    materialized
        .base_outcome
        .aliases
        .node(alias)
        .expect("managed declaration node")
}

fn binding(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    symbol: &str,
    role: IntentPortRole,
) -> IntentNativeBinding {
    let node = node_by_symbol(materialized, symbol);
    let intent = materialized.editor.coordinator().intent();
    let port = intent
        .graph()
        .node(node)
        .unwrap()
        .port_by_selector(IntentPortSelector::Node { role, index: 0 })
        .unwrap()
        .as_ref(node);
    materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .expect("native binding")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one managed end-to-end regression keeps source compilation, structural continuation, independent solve validation, and writable-leaf preservation together"
)]
fn managed_range_source_edit_continues_to_the_bound_and_preserves_unrelated_geometry() {
    let base_project = project(BASE);
    let reconciliation = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&base_project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let before = materialize_code_project_cold(
        &base_project,
        &reconciliation,
        IntentSessionId::from_raw(0x91_0c01),
        DocumentId(PersistentId::from_u128(0x91_0c01)),
        1.0,
    )
    .expect("base project accepted");
    let IntentNativeBinding::Contact(contact_id) =
        binding(&before, "contact", IntentPortRole::Contact)
    else {
        panic!("point-on-curve owns one contact");
    };
    let IntentNativeBinding::Point(unrelated_id) =
        binding(&before, "unrelated", IntentPortRole::Primary)
    else {
        panic!("unrelated sketch point owns one point");
    };
    let accepted_before = before
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .clone();
    let unrelated_before = accepted_before.point(unrelated_id).unwrap().position;
    let contact_before = accepted_before.contact(contact_id).unwrap();
    assert!(
        accepted_before
            .scalar(contact_before.parameter)
            .unwrap()
            .value
            > 0.5
    );

    let limited_project = project(LIMITED);
    let limited_reconciliation = reconciliation
        .plan(
            required_generated_members(&limited_project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let (after, retained_overlay) = materialize_code_project_incremental_for_structural_edit(
        &before,
        &limited_project,
        &limited_reconciliation,
        &CodeInteractionOverlay::empty(),
    )
    .expect("range-only managed source edit accepted");
    assert_eq!(retained_overlay, CodeInteractionOverlay::empty());

    let accepted = after
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap();
    let contact = accepted.document().contact(contact_id).unwrap();
    assert_eq!(
        contact.admissible_range,
        Some(ContactAdmissibleRange {
            lower: 0.0,
            upper: 0.5,
        })
    );
    assert_eq!(
        accepted
            .document()
            .scalar(contact.parameter)
            .unwrap()
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        accepted
            .document()
            .point(unrelated_id)
            .unwrap()
            .position
            .map(f64::to_bits),
        unrelated_before.map(f64::to_bits)
    );
    let diagnostics = accepted.diagnostics();
    let solve = diagnostics.solve.as_ref().unwrap();
    assert!(solve.accepted && solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
    );
    let bound = diagnostics
        .bounds
        .iter()
        .find(|bound| bound.target == DocumentElementId::Contact(contact_id))
        .unwrap();
    assert_eq!(bound.status, SketchBoundStatus::ActiveUpper);
    assert!(
        after
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .writable_leaf(IntentNativeWritableLeaf::PointX {
                point: unrelated_id,
            })
            .is_some()
    );
}
