// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::ComputedFeatureDefinition;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, FeatureKind, KeyedReconcileState, ManagedPathSegment,
    ProjectKey, SemanticOutputPath, SemanticSymbol, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

const SOURCE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-fillet.sketch.ts");
const ENVELOPE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-fillet.json");

#[test]
fn compiled_v3_named_fillet_set_cold_replays_keyed_members_and_valid_branch_state() {
    let compiled = CompiledManagedSource::from_json(ENVELOPE)
        .expect("authenticated V3 direct-Fillet compiler envelope");
    compiled
        .validate_input_source(SOURCE)
        .expect("direct-Fillet source digest");
    let project = CodeProject::managed(ProjectKey("managed-v3-direct-fillet".into()), compiled)
        .expect("authenticated direct-Fillet project");
    let desired = required_generated_members(&project).expect("direct-Fillet member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("direct-Fillet member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x90f1_11e7),
        DocumentId(PersistentId::from_u128(0x90f1_11e7)),
        1.0,
    )
    .expect("named FilletSet must cold materialize");

    let path = |segments: &[&str]| {
        SemanticOutputPath(
            segments
                .iter()
                .map(|segment| {
                    segment.strip_prefix('@').map_or_else(
                        || ManagedPathSegment::Field((*segment).into()),
                        |member| ManagedPathSegment::Member {
                            member: member.into(),
                        },
                    )
                })
                .collect(),
        )
    };
    let expected_paths = BTreeSet::from([
        path(&["fillets"]),
        path(&["fillets", "@firstSecond", "arc"]),
        path(&["fillets", "@firstSecond", "corner"]),
        path(&["fillets", "@secondThird", "arc"]),
        path(&["fillets", "@secondThird", "corner"]),
    ]);
    let actual_paths = materialized
        .expansion
        .semantic_outputs
        .values()
        .filter(|output| output.reference.declaration == SemanticSymbol("round".into()))
        .map(|output| output.reference.output.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_paths, expected_paths);
    assert!(
        materialized
            .expansion
            .semantic_outputs
            .values()
            .any(|output| {
                output.reference.declaration == SemanticSymbol("round".into())
                    && output.reference.expected_kind == FeatureKind::CurveSpan
            })
    );

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold FilletSet owns accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let features = accepted.features.features();
    assert_eq!(features.len(), 1);
    let ComputedFeatureDefinition::FilletSet(fillet) = &features[0].definition;
    assert_eq!(fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(fillet.corners.len(), 2);
    assert!(
        accepted
            .session
            .design_document()
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
}
