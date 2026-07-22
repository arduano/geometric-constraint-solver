// SPDX-License-Identifier: GPL-3.0-or-later

use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_linkage::{
    PlanarDocumentId, PlanarLinkageDocument, PlanarLinkageSession, SpatialAssemblyDocument,
    SpatialAssemblyDocumentSession, SpatialAssemblySession, SpatialDocumentId, SpatialExampleKind,
    four_bar_open, spatial_example,
};

const MUTATION_REPLACEMENTS: [u8; 8] = [b'0', b'9', b'-', b'{', b']', b'"', b' ', 0xff];

fn mutate(seed: &[u8], case: usize) -> String {
    let mut bytes = seed.to_vec();
    let index = case.wrapping_mul(2_654_435_761) % bytes.len();
    bytes[index] = MUTATION_REPLACEMENTS[case % MUTATION_REPLACEMENTS.len()];
    String::from_utf8_lossy(&bytes).into_owned()
}

fn assert_planar_success_is_valid(document: PlanarLinkageDocument) {
    let canonical = document.to_json().unwrap();
    assert_eq!(
        PlanarLinkageDocument::from_json(&canonical)
            .unwrap()
            .to_json()
            .unwrap(),
        canonical
    );
    if let Ok(session) = PlanarLinkageSession::new(document, SolverConfig::default()) {
        let result = session.accepted_result();
        assert!(result.accepted());
        assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
        assert!(result.core_report.hard_residuals_validated);
        assert!(result.core_report.hard_residual_max.is_finite());
        assert!(result.core_report.hard_residual_max <= 1.0e-9);
        assert!(
            result
                .core_report
                .singular_values
                .iter()
                .all(|value| value.is_finite())
        );
    }
}

fn assert_spatial_success_is_valid(document: SpatialAssemblyDocument) {
    let canonical = document.to_json().unwrap();
    assert_eq!(
        SpatialAssemblyDocument::from_json(&canonical)
            .unwrap()
            .to_json()
            .unwrap(),
        canonical
    );
    if let Ok(session) = SpatialAssemblyDocumentSession::new(document, SolverConfig::default()) {
        let result = session.accepted_result();
        assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
        assert!(result.core_report.hard_residuals_validated);
        assert!(result.acceptance_hard_residual_max.is_finite());
        assert!(result.acceptance_hard_residual_max <= 1.0e-9);
        assert!(
            result
                .core_report
                .singular_values
                .iter()
                .all(|value| value.is_finite())
        );
        for body in &result.geometry.bodies {
            assert!(body.pose.ambient().iter().all(|value| value.is_finite()));
        }
    }
}

#[test]
fn planar_v1_mutations_never_panic_or_publish_false_success() {
    let (linkage, _) = four_bar_open().unwrap();
    let (document, _) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x2900), &linkage).unwrap();
    let canonical = document.to_json().unwrap();

    for case in 0..512 {
        let input = mutate(canonical.as_bytes(), case);
        let parsed = catch_unwind(AssertUnwindSafe(|| {
            PlanarLinkageDocument::from_json(&input)
        }))
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
        if let Ok(document) = parsed {
            assert_planar_success_is_valid(document);
        }
    }
}

#[test]
fn spatial_v1_mutations_never_panic_or_publish_false_success() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let runtime = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let persistent = SpatialAssemblyDocumentSession::from_accepted_session(
        SpatialDocumentId::from_u128(0x2901),
        &runtime,
    )
    .unwrap();
    let canonical = persistent.to_json().unwrap();

    for case in 0..512 {
        let input = mutate(canonical.as_bytes(), case);
        let parsed = catch_unwind(AssertUnwindSafe(|| {
            SpatialAssemblyDocument::from_json(&input)
        }))
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
        if let Ok(document) = parsed {
            assert_spatial_success_is_valid(document);
        }
    }
}
