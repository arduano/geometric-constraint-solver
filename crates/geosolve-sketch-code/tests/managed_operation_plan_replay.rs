// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::PreparedIntentOperationOutputRole;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeCompositionError, CodeInteractionOverlay, CodeProject, CompiledManagedSource,
    ExpandedCodeProject, KeyedReconcileState, MaterializedCodeProject, ProjectKey,
    materialize_code_project_cold, materialize_code_project_incremental_for_structural_edit,
    rehydrate_materialized_code_project, required_generated_members,
};
use geosolve_sketch_intent::{
    IntentKey, IntentOperationOutputKind, IntentSessionId, OperationKind, intent_content_digest,
};

const PROFILE_OFFSET: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
);
const PROFILE_OFFSET_REORDERED: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-reordered.json"
);
const OPERATION_CATALOG: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-operation-catalog.json"
);

fn materialized_from_with_project(
    envelope: &str,
    project: &str,
    session: u128,
) -> MaterializedCodeProject {
    let compiled = CompiledManagedSource::from_json(envelope).expect("compiled managed source");
    let project = CodeProject::managed(ProjectKey(project.into()), compiled)
        .expect("operation replay project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated reconciliation")
        .into_staged();
    materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(session),
        DocumentId(PersistentId::from_u128(session)),
        1.0,
    )
    .expect("cold operation materialization")
}

fn materialized_from(envelope: &str, session: u128) -> MaterializedCodeProject {
    materialized_from_with_project(envelope, "managed-v3-operation-catalog", session)
}

fn materialized(session: u128) -> MaterializedCodeProject {
    let materialized = materialized_from(PROFILE_OFFSET, session);
    assert_eq!(materialized.expansion.operation_plans.len(), 1);
    materialized
}

fn refresh_expansion_digest(expansion: &mut ExpandedCodeProject) {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
        &expansion.operation_plans,
    ))
    .expect("expansion digest payload");
    expansion.digest = intent_content_digest(&bytes).to_string();
}

fn assert_native_plan_tamper_rejected(
    session: u128,
    mutate: impl FnOnce(&mut ExpandedCodeProject),
) {
    let materialized = materialized(session);
    let mut expansion = materialized.expansion.clone();
    mutate(&mut expansion);
    refresh_expansion_digest(&mut expansion);
    assert!(matches!(
        rehydrate_materialized_code_project(Box::new(materialized.editor), expansion),
        Err(CodeCompositionError::RehydratedOperationPlanMismatch { .. })
    ));
}

#[test]
fn retained_profile_offset_plan_rehydrates_after_native_reauthentication() {
    let materialized = materialized(0x89_0f00);

    rehydrate_materialized_code_project(Box::new(materialized.editor), materialized.expansion)
        .expect("native-authenticated warm rehydration");
}

#[test]
fn profile_offset_structural_edit_plans_against_pristine_cold_authority() {
    let previous = materialized(0x90_0f10);
    let compiled = CompiledManagedSource::from_json(PROFILE_OFFSET_REORDERED)
        .expect("reordered Profile Offset compiler envelope");
    let project = CodeProject::managed(ProjectKey("operation-plan-replay".into()), compiled)
        .expect("reordered Profile Offset project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("reordered member reconciliation")
        .into_staged();

    let (candidate, retained_overlay) = materialize_code_project_incremental_for_structural_edit(
        &previous,
        &project,
        &generated,
        &CodeInteractionOverlay::empty(),
    )
    .expect("an operation-bearing source reorder must use isolated pristine planning authority");

    assert_eq!(retained_overlay, CodeInteractionOverlay::empty());
    assert_eq!(candidate.expansion.operation_plans.len(), 1);
    let accepted = candidate
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("structural edit retains complete accepted native authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn retained_multi_operation_plan_rehydrates_in_exact_declaration_order() {
    let materialized =
        materialized_from_with_project(OPERATION_CATALOG, "operation-plan-replay", 0x90_0f00);
    assert_eq!(materialized.expansion.operation_plans.len(), 12);
    let declaration_order = materialized
        .expansion
        .operation_plans
        .iter()
        .map(|retained| {
            let current = retained
                .prefix_aliases
                .last()
                .expect("operation planning prefix includes its owner");
            materialized
                .expansion
                .declaration_for_alias(current)
                .expect("planning alias has declaration provenance")
                .0
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declaration_order,
        [
            "split",
            "breakCurve",
            "trim",
            "extend",
            "mirror",
            "chamfer",
            "associativeFillet",
            "rectangle",
            "regularPolygon",
            "slot",
            "linearPattern",
            "profileOffset",
        ]
    );
    for pair in materialized.expansion.operation_plans.windows(2) {
        assert!(
            pair[1].prefix_aliases.starts_with(&pair[0].prefix_aliases),
            "each native plan retains the exact cumulative source-order prefix"
        );
        assert!(pair[1].prefix_aliases.len() > pair[0].prefix_aliases.len());
    }

    rehydrate_materialized_code_project(Box::new(materialized.editor), materialized.expansion)
        .expect("all retained native operation plans reauthenticate in source order");
}

#[test]
fn restored_operation_plan_rejects_reordered_rows() {
    let materialized = materialized_from(OPERATION_CATALOG, 0x90_0f01);
    let mut expansion = materialized.expansion.clone();
    expansion.operation_plans.swap(0, 1);
    refresh_expansion_digest(&mut expansion);
    assert!(matches!(
        rehydrate_materialized_code_project(Box::new(materialized.editor), expansion),
        Err(CodeCompositionError::RehydratedOperationPlanMismatch { .. })
    ));
}

#[test]
fn restored_operation_plan_rejects_missing_extra_and_mutated_rows() {
    assert_native_plan_tamper_rejected(0x89_0f01, |expansion| {
        expansion.operation_plans.clear();
    });
    assert_native_plan_tamper_rejected(0x89_0f02, |expansion| {
        let extra = expansion.operation_plans[0].clone();
        expansion.operation_plans.push(extra);
    });
    assert_native_plan_tamper_rejected(0x89_0f03, |expansion| {
        expansion.operation_plans[0].symbol = IntentKey::new("wrongOperation").unwrap();
    });
    assert_native_plan_tamper_rejected(0x89_0f04, |expansion| {
        expansion.operation_plans[0].plan.operation = OperationKind::Mirror;
    });
    assert_native_plan_tamper_rejected(0x89_0f05, |expansion| {
        expansion.operation_plans[0].plan.outputs.pop();
    });
    assert_native_plan_tamper_rejected(0x89_0f06, |expansion| {
        expansion.operation_plans[0].plan.outputs[0].path.clear();
    });
    assert_native_plan_tamper_rejected(0x89_0f07, |expansion| {
        let output = &mut expansion.operation_plans[0].plan.outputs[0];
        output.output.kind = match output.output.kind {
            IntentOperationOutputKind::Point => IntentOperationOutputKind::Scalar,
            _ => IntentOperationOutputKind::Point,
        };
    });
    assert_native_plan_tamper_rejected(0x89_0f08, |expansion| {
        let output = expansion.operation_plans[0]
            .plan
            .outputs
            .iter_mut()
            .find(|output| output.output.kind == IntentOperationOutputKind::Curve)
            .expect("Profile Offset owns a curve output");
        output.output.curve_span_count = output.output.curve_span_count.saturating_add(1);
    });
    assert_native_plan_tamper_rejected(0x89_0f09, |expansion| {
        let output = &mut expansion.operation_plans[0].plan.outputs[0];
        output.role = match output.role {
            PreparedIntentOperationOutputRole::Geometry => {
                PreparedIntentOperationOutputRole::Contact
            }
            _ => PreparedIntentOperationOutputRole::Geometry,
        };
    });
    assert_native_plan_tamper_rejected(0x89_0f0a, |expansion| {
        *expansion.operation_plans[0]
            .prefix_aliases
            .last_mut()
            .expect("Profile Offset planning owner") =
            IntentKey::new("wrongPlanningAlias").unwrap();
    });
}
