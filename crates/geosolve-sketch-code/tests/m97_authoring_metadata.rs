// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeSet;

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
    materialize_code_project_cold, materialize_code_project_incremental_for_structural_edit,
    required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

#[test]
fn m97_f003_source_relabel_preserves_independently_accepted_native_authority() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/m97-authoring-lifecycle.json")).unwrap();
    let initial: CompiledManagedSource =
        serde_json::from_value(fixture["steps"][0]["compiled"].clone()).unwrap();
    let renamed: CompiledManagedSource =
        serde_json::from_value(fixture["steps"][1]["compiled"].clone()).unwrap();
    let project = CodeProject::managed(ProjectKey("relabel".into()), initial).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let original = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x97_03),
        DocumentId(PersistentId::from_u128(0x97_03)),
        1.0,
    )
    .unwrap();
    let before = original
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let document = before
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    let geometry = serde_json::json!({"points":document.points(),"scalars":document.scalars(),"curves":document.curves()});
    let renamed_project = CodeProject::managed(ProjectKey("relabel".into()), renamed).unwrap();
    let (renamed, overlay) = materialize_code_project_incremental_for_structural_edit(
        &original,
        &renamed_project,
        &generated,
        &CodeInteractionOverlay::empty(),
    )
    .expect("a name-only change retains accepted native authority");
    assert_eq!(overlay, CodeInteractionOverlay::empty());
    let accepted = renamed
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(
        accepted.validation.hard_residuals_validated
            && accepted.validation.all_active_features_current
    );
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1e-9)
    );
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert_eq!(
        serde_json::json!({"points":document.points(),"scalars":document.scalars(),"curves":document.curves()}),
        geometry
    );
    assert_eq!(
        accepted.evidence, before.evidence,
        "renaming preserves independently validated mathematical evidence"
    );
    assert!(
        renamed
            .editor
            .coordinator()
            .intent()
            .organization()
            .node_names()
            .values()
            .any(|name| name.as_str() == "Main span")
    );
    assert!(
        !original
            .editor
            .coordinator()
            .intent()
            .organization()
            .node_names()
            .values()
            .any(|name| name.as_str() == "Main span"),
        "source edit borrows the prior accepted state"
    );
}
