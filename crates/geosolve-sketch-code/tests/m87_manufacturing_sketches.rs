// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    KeyedReconcileState, ManagedControlEdit, ManagedControlEditBatch, ManagedSketchMutation,
    bundled_sample, expand_code_project, managed_control_manifest, materialize_code_project_cold,
    prepare_managed_control_mutation, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

#[test]
fn normalized_manufacturing_samples_cold_solve_and_prepare_source_owned_controls() {
    for (ordinal, key) in ["cnc-dogbone-coupon", "gridfinity-bin-section"]
        .into_iter()
        .enumerate()
    {
        let project = bundled_sample(key).expect("manufacturing sample").project();
        let desired = required_generated_members(&project).expect("generated inventory");
        let generated = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .expect("generated reconciliation")
            .into_staged();
        let seed = 0x90_50 + ordinal as u128;
        let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).unwrap();
        let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();
        let manifest = managed_control_manifest(&project, &expansion).unwrap();
        let control = manifest.editable().next().expect("editable source control");
        let mutation = prepare_managed_control_mutation(
            &project,
            &expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: control.token().unwrap().clone(),
                value: control.value.clone(),
            }]),
        )
        .expect("source-owned semantic mutation");
        assert!(
            matches!(mutation, ManagedSketchMutation::SetValues { ref values } if values.len() == 1)
        );

        let materialized = materialize_code_project_cold(
            &project,
            &generated,
            IntentSessionId::from_raw(seed),
            DocumentId(PersistentId::from_u128(seed)),
            1.0,
        )
        .expect("manufacturing sample cold materialization");
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted manufacturing authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
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
}

#[test]
fn gridfinity_uses_coordinate_datums_without_point_fixing_either_view() {
    let source = bundled_sample("gridfinity-bin-section")
        .expect("canonical Gridfinity sample")
        .managed_source();
    assert_eq!(
        source.matches("$.constraint.fixedCoordinate(").count(),
        2,
        "the section and linked plan each own one vertical datum"
    );
    assert_eq!(
        source.matches("$.constraint.fixedPoint(").count(),
        0,
        "both views remain width-editable through their dimension and symmetry relations"
    );
    assert!(source.contains("const sectionBaseYDatum = $.constraint.fixedCoordinate("));
    assert!(source.contains("const planAnchor = $.constraint.fixedCoordinate("));
    assert!(source.contains("const cavityFloorAtBase = $.constraint.horizontalPoints("));
    assert!(source.contains("const planMatchesSection = $.constraint.verticalPoints("));
    assert!(source.contains("const baseBottomWidth = $.dimension.curveLength("));
    assert!(source.contains("$.group(\"3U material section\""));
    assert!(source.contains("$.group(\"1 x 1 plan study\""));
}
