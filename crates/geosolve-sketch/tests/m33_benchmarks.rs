// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "../benches/support/m33_representative.rs"]
mod representative;

use representative::{PreparedWorkload, WORKLOADS, WorkloadSize, workloads};

#[test]
fn representative_documents_have_exactly_six_deterministic_keys_and_shapes() {
    assert_eq!(WORKLOADS.len(), 6);
    assert_eq!(
        WORKLOADS.map(representative::WorkloadKind::key),
        [
            "connected",
            "disconnected",
            "spline_heavy",
            "parameter_heavy",
            "external_reference",
            "activation_heavy",
        ]
    );
    for kind in WORKLOADS {
        let first = representative::build_workload(kind, WorkloadSize::Representative);
        let second = representative::build_workload(kind, WorkloadSize::Representative);
        assert_eq!(first.document_signature(), second.document_signature());
        assert_eq!(
            first.document_signature(),
            representative::expected_representative_signature(kind).document
        );
        assert_eq!(
            first.document.to_canonical_json().unwrap(),
            second.document.to_canonical_json().unwrap()
        );
        assert!(
            kind.shape_name()
                .contains("workload-shape proxy (not an API)")
                || matches!(
                    kind,
                    representative::WorkloadKind::Connected
                        | representative::WorkloadKind::Disconnected
                        | representative::WorkloadKind::SplineHeavy
                )
        );
    }
}

#[test]
fn smoke_shapes_compile_solve_edit_diagnose_and_profile() {
    for definition in workloads(WorkloadSize::Smoke) {
        let key = definition.kind.key();
        let prepared = PreparedWorkload::prepare(definition);
        assert_eq!(prepared.definition.kind.key(), key);
        assert!(
            prepared
                .accepted
                .accepted_result()
                .accepted_view()
                .accepted()
        );
        assert_eq!(
            prepared.profile_options,
            geosolve_sketch::VisualProfileOptions::default()
        );
        assert!(prepared.signature.document.canonical_bytes > 0);
        assert!(prepared.signature.solve.tangent_dimensions > 0);
        assert_eq!(
            prepared.signature.profile.status,
            geosolve_sketch::VisualProfileStatus::Complete
        );
    }
}
