// SPDX-License-Identifier: GPL-3.0-or-later

use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaScenarioKind, MAX_DOCUMENT_JSON_BYTES, SketchDocument, SketchDocumentSession,
    alpha_scenario,
};

const MUTATION_REPLACEMENTS: [u8; 8] = [b'0', b'9', b'-', b'{', b']', b'"', b' ', 0xff];

fn assert_success_is_valid(document: SketchDocument) {
    let canonical = document.to_canonical_json().unwrap();
    let reparsed = SketchDocument::from_json(&canonical).unwrap();
    assert_eq!(reparsed.to_canonical_json().unwrap(), canonical);

    if let Ok(session) = SketchDocumentSession::new(
        document,
        geosolve_sketch::DocumentSolveRequest::default(),
        SolverConfig::default(),
    ) {
        let accepted = session.accepted_result();
        assert!(accepted.accepted());
        let report = &accepted.accepted_view().unstable_core_report();
        assert_eq!(report.hard_validity, HardValidity::Valid);
        assert!(report.hard_residuals_validated);
        assert!(report.hard_residual_max.is_finite());
        assert!(report.hard_residual_max <= 1.0e-9);
        assert!(report.singular_values.iter().all(|value| value.is_finite()));
    }
}

fn mutate(seed: &[u8], case: usize) -> String {
    let mut bytes = seed.to_vec();
    let index = case.wrapping_mul(2_654_435_761) % bytes.len();
    bytes[index] = MUTATION_REPLACEMENTS[case % MUTATION_REPLACEMENTS.len()];
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn sketch_v1_v4_mutations_never_panic_or_publish_false_success() {
    let current = alpha_scenario(AlphaScenarioKind::A1, 1.0)
        .unwrap()
        .document
        .to_canonical_json()
        .unwrap();
    let mut old_value: serde_json::Value = serde_json::from_str(&current).unwrap();
    old_value.as_object_mut().unwrap().remove("trim_views");

    let mut seeds = vec![current];
    for version in [1, 2, 3] {
        old_value["version"] = version.into();
        seeds.push(serde_json::to_string(&old_value).unwrap());
    }

    for seed in seeds {
        for case in 0..256 {
            let input = mutate(seed.as_bytes(), case);
            let outcome = catch_unwind(AssertUnwindSafe(|| SketchDocument::from_json(&input)));
            let parsed = outcome.unwrap_or_else(|payload| {
                std::panic::resume_unwind(payload);
            });
            if let Ok(document) = parsed {
                assert_success_is_valid(document);
            }
        }
    }
}

#[test]
fn sketch_resource_and_extreme_numeric_inputs_fail_without_panic() {
    let oversized = " ".repeat(MAX_DOCUMENT_JSON_BYTES + 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| SketchDocument::from_json(&oversized)))
            .unwrap()
            .is_err()
    );

    let canonical = alpha_scenario(AlphaScenarioKind::M28TrimmedFillet, 1.0)
        .unwrap()
        .document
        .to_canonical_json()
        .unwrap();
    for replacement in ["1e9999", "-1e9999", "5e-324", "0"] {
        let input = canonical.replacen("1.0", replacement, 1);
        let parsed = catch_unwind(AssertUnwindSafe(|| SketchDocument::from_json(&input))).unwrap();
        if let Ok(document) = parsed {
            assert_success_is_valid(document);
        }
    }
}
