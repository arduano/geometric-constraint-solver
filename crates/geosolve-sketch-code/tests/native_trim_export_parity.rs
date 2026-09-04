// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentCurveTrimView, DocumentId, DocumentTrimBoundary,
    DocumentTrimParameter, PersistentId, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchDocument,
};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
    export_sketch_document_to_managed_source, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSessionId, OperationKind};

const FIXED_TRIM_ENVELOPE: &str = include_str!("fixtures/native-fixed-trim-export.json");

fn fixed_trim_document() -> (SketchDocument, CurveSpan) {
    let mut document = SketchDocument::new(1.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let curve = document
        .add_curve(
            "support",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let support = CurveSpan::line(curve);
    document
        .replace_trim_views(
            support,
            vec![DocumentCurveTrimView {
                support,
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.25,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.75,
                    winding: 0,
                }),
            }],
        )
        .unwrap();
    (document, support)
}

#[test]
fn fixed_native_trim_export_cold_materializes_the_same_visible_interval() {
    let (native, native_support) = fixed_trim_document();
    let native_intervals = native.visible_intervals(native_support).unwrap();
    let exported = export_sketch_document_to_managed_source(&native).unwrap();
    let compiled = CompiledManagedSource::from_json(FIXED_TRIM_ENVELOPE)
        .expect("pinned compiler envelope for the native export");
    compiled
        .validate_input_source(&exported)
        .expect("fixture authenticates the exact current exporter bytes");
    assert_eq!(compiled.normalized_source, exported);

    let project = CodeProject::managed(ProjectKey("native-fixed-trim-export".into()), compiled)
        .expect("authenticated managed project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x91_7a1_u128),
        DocumentId(PersistentId::from_u128(0x91_7a1_u128)),
        native.model_scale(),
    )
    .expect("native-exported fixed trim cold materializes");

    assert_eq!(
        materialized
            .expansion
            .operation_plans
            .iter()
            .map(|operation| operation.plan.operation)
            .collect::<Vec<_>>(),
        [OperationKind::Trim, OperationKind::Trim],
    );
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold materialization owns accepted native authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| {
                value.is_finite() && value <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            })
    );

    let managed = accepted.session.design_document();
    let [managed_curve] = managed.curves() else {
        panic!("managed export must retain exactly one support curve");
    };
    let managed_intervals = managed
        .visible_intervals(CurveSpan::line(managed_curve.id))
        .expect("managed visible interval");
    assert_eq!(managed.trim_views().len(), 1);
    assert_eq!(managed_intervals.len(), 1);
    assert!(
        managed_intervals[0]
            .start
            .total_cmp(&native_intervals[0].start)
            .is_eq()
    );
    assert!(
        managed_intervals[0]
            .end
            .total_cmp(&native_intervals[0].end)
            .is_eq()
    );
    assert_eq!(
        managed_intervals[0].start_boundary,
        native_intervals[0].start_boundary,
    );
    assert_eq!(
        managed_intervals[0].end_boundary,
        native_intervals[0].end_boundary,
    );
}
