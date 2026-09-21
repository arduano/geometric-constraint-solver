// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden parity: pin `geosolve-survey` to the golden authoring oracle TSV.
//!
//! The golden authoring oracle
//! (`crates/geosolve-constraint-editor/tests/golden_authoring_oracle.rs`) enumerates
//! a fixed inventory of 271 cases and records, for each, a canonical input
//! fingerprint. This test reconstructs the same survey inputs from [`FuzzVariant`],
//! runs [`survey`], and asserts the computed fingerprint matches the golden one for
//! every case the survey model can reproduce: the 261 constraint + dimension cases.
//!
//! The remaining 10 cases (`feature.fillet`, `scene-authority`) use different input
//! models (fillet authoring hashes sketch/feature JSON documents; scene-authority
//! rows carry a plain placeholder), so they cannot be reproduced by the
//! `FuzzVariant` model. They are presence- and shape-checked so the corpus cannot
//! silently drift out from under the survey crate.

use std::cell::RefCell;
use std::fs;

use geosolve_constraint_editor::ResolvedConstraintKind;
use geosolve_survey::survey::survey;
use geosolve_survey::{FAMILIES, Family, FuzzVariant};
use proptest::prelude::{Strategy, any};
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};

/// The golden corpus this crate pins against.
///
/// A byte-for-byte copy of
/// `crates/geosolve-constraint-editor/tests/fixtures/golden_authoring_scene_oracle.golden.tsv`
/// (the authoritative source the survey crate cannot itself reach).
const GOLDEN_TSV: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/golden_authoring_scene_oracle.golden.tsv"
);

/// Every row the golden oracle records.
const EXPECTED_ROW_COUNT: usize = 271;

/// Rows the `FuzzVariant` model can reproduce (29 families x 9 cases).
const EXPECTED_REPRODUCIBLE_COUNT: usize = 261;

/// A single golden oracle row, reduced to the fields this test reasons about.
struct GoldenRow {
    case_id: String,
    family: String,
    status: String,
    fingerprint: String,
}

/// Parse the golden TSV (header skipped) into rows.
fn read_golden_rows() -> Vec<GoldenRow> {
    let text = fs::read_to_string(GOLDEN_TSV)
        .unwrap_or_else(|error| panic!("cannot read golden TSV {GOLDEN_TSV}: {error}"));
    let mut rows = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line_number == 0 || line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let case_id = fields.next().expect("case_id").to_owned();
        let family = fields.next().expect("family").to_owned();
        let status = fields.next().expect("status").to_owned();
        let _finding_id = fields.next().expect("finding_id").to_owned();
        let _failure_class = fields.next().expect("failure_class").to_owned();
        let fingerprint = fields.next().expect("fingerprint").to_owned();
        debug_assert!(
            fields.next().is_none(),
            "golden TSV row {line_number} has an unexpected trailing field"
        );
        rows.push(GoldenRow {
            case_id,
            family,
            status,
            fingerprint,
        });
    }
    rows
}

const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

const fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    state ^ (state >> 31)
}

const BASE_SEED: [u8; 32] = [
    0xaa, 0x6a, 0xb8, 0x8c, 0xc8, 0xaa, 0x48, 0x78, 0xc5, 0x1d, 0x78, 0xdb, 0x3d, 0x1b, 0x99, 0x33,
    0x55, 0x40, 0x6f, 0xce, 0x8c, 0x6c, 0x42, 0x35, 0x3a, 0x85, 0x0c, 0x05, 0x69, 0x6c, 0x2e, 0xdd,
];

/// Derive the `ChaCha` seed for a seeded golden case, mirroring the golden oracle.
fn oracle_seed(family: &str, variant_index: u32) -> [u8; 32] {
    let mut seed = BASE_SEED;
    let mut state =
        fnv1a64(family.as_bytes()) ^ u64::from(variant_index).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for chunk in seed.as_chunks_mut::<8>().0 {
        state = splitmix64(state);
        let original = u64::from_le_bytes(*chunk);
        chunk.copy_from_slice(&(original ^ state).to_le_bytes());
    }
    seed
}

/// The perturbation strategy the golden oracle draws seeded cases from.
fn variant_strategy() -> impl Strategy<Value = FuzzVariant> {
    (
        -8_i32..=8,
        -8_i32..=8,
        0_u8..3,
        0_u8..7,
        any::<bool>(),
        any::<bool>(),
        0_u8..7,
        any::<bool>(),
    )
        .prop_map(
            |(tx, ty, scale, rotation, reverse_spans, swap_operands, contact, displaced)| {
                let scale = [0.25, 1.0, 4.0][usize::from(scale)];
                FuzzVariant {
                    translation: [f64::from(tx) * scale, f64::from(ty) * scale],
                    scale,
                    rotation: [-0.7, -0.35, 0.0, 0.2, 0.45, 0.8, 1.1][usize::from(rotation)],
                    reverse_spans,
                    swap_operands,
                    contact_parameter: 0.25 + f64::from(contact) * 0.08,
                    displaced,
                    option_index: 0,
                }
            },
        )
}

/// The golden oracle's effective-variant adjustment.
///
/// Axis-aligned line/point subjects pin `rotation` to zero. Point-on-curve
/// subjects remap `contact_parameter` from the option/reverse-spans pair, but only
/// when `compare_preselection` is false (the seeded survey path; the deterministic
/// reference path preserves the raw contact parameter).
fn effective_variant(
    family: Family,
    variant: FuzzVariant,
    compare_preselection: bool,
) -> FuzzVariant {
    let mut variant = variant;
    let is_axis_aligned = matches!(
        family,
        Family::Constraint {
            kind: ResolvedConstraintKind::HorizontalLine
                | ResolvedConstraintKind::VerticalLine
                | ResolvedConstraintKind::HorizontalPoints
                | ResolvedConstraintKind::VerticalPoints,
            ..
        }
    );
    if is_axis_aligned {
        variant.rotation = 0.0;
    }
    if !compare_preselection
        && matches!(
            family,
            Family::Constraint {
                kind: ResolvedConstraintKind::PointOnCurve,
                ..
            }
        )
    {
        variant.contact_parameter = match variant.option_index {
            0 => {
                if variant.reverse_spans {
                    1.0
                } else {
                    0.0
                }
            }
            1 => {
                if variant.reverse_spans {
                    0.0
                } else {
                    1.0
                }
            }
            _ => variant.contact_parameter,
        };
    }
    variant
}

/// Reconstruct the effective variant for a seeded golden case.
///
/// Mirrors the golden oracle's `survey_seeded`: a single `ChaCha` RNG draw from
/// [`variant_strategy`] (no shrinking, since every golden case passes), then the
/// index-derived flags, the endpoint-continuity witness tweak, and finally the
/// `compare_preselection == false` effective adjustment.
fn seeded_variant(family: Family, index: u32) -> FuzzVariant {
    let seed = oracle_seed(family.id(), index);
    let config = Config {
        cases: 1,
        max_shrink_iters: 512,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &seed));
    let is_endpoint_continuity = matches!(
        family,
        Family::Constraint {
            kind: ResolvedConstraintKind::EndpointContinuity,
            ..
        }
    );
    let last = RefCell::new(FuzzVariant::DETERMINISTIC);
    let result = runner.run(
        &variant_strategy(),
        |mut variant: FuzzVariant| -> Result<(), TestCaseError> {
            variant.reverse_spans = index & 1 != 0;
            variant.displaced = index & 2 != 0;
            variant.swap_operands = index & 4 != 0;
            variant.option_index = u8::try_from(index).expect("variant index fits u8");
            if is_endpoint_continuity && index == 3 {
                // Retain one pre-satisfied unequal-rate Parametric-C2 witness.
                variant.displaced = false;
            }
            *last.borrow_mut() = effective_variant(family, variant, false);
            Ok(())
        },
    );
    assert!(
        result.is_ok(),
        "golden seeded case {index} for {family:?} failed to draw"
    );
    last.into_inner()
}

/// Reconstruct the effective variant for a reproducible golden `case_id`.
fn reconstruct_variant(family: Family, case_id: &str) -> FuzzVariant {
    let prefix = format!("{}.", family.id());
    if let Some(rest) = case_id.strip_prefix(&prefix) {
        if let Some(index_str) = rest.strip_prefix("seed-") {
            let index: u32 = index_str
                .parse()
                .unwrap_or_else(|error| panic!("invalid seeded golden case_id {case_id}: {error}"));
            return seeded_variant(family, index);
        }
        if rest == "deterministic" {
            return effective_variant(family, FuzzVariant::DETERMINISTIC, true);
        }
    }
    panic!("golden case_id {case_id} is not a reproducible FuzzVariant case");
}

/// Pin the survey crate to the golden TSV: reproduce every `FuzzVariant` case and
/// presence-check every remaining case so the corpus cannot drift.
#[test]
fn golden_parity_matches_the_golden_tsv() {
    let rows = read_golden_rows();
    assert_eq!(
        rows.len(),
        EXPECTED_ROW_COUNT,
        "golden TSV row count is {}, expected {EXPECTED_ROW_COUNT}",
        rows.len()
    );

    let mut problems = Vec::new();
    let mut reproducible = 0usize;

    for row in &rows {
        let Some(family) = FAMILIES
            .iter()
            .copied()
            .find(|candidate| candidate.id() == row.family)
        else {
            // feature.fillet / scene-authority: different input models.
            if row.status != "PASS" {
                problems.push(format!(
                    "case_id={} non-reproducible row has non-PASS status {}",
                    row.case_id, row.status
                ));
            }
            if !row.fingerprint.starts_with("input-") && row.fingerprint != "ok" {
                problems.push(format!(
                    "case_id={} unexpected fingerprint {:#?} (expected input-… or ok)",
                    row.case_id, row.fingerprint
                ));
            }
            continue;
        };

        reproducible += 1;
        let variant = reconstruct_variant(family, &row.case_id);
        let outcome = survey(family, variant);
        if let Err(message) = outcome.check()
            && outcome.accepted
        {
            problems.push(format!(
                "case_id={} survey reported a false convergence: {message}",
                row.case_id
            ));
        }
        let actual = variant.fingerprint(&row.family);
        if actual != row.fingerprint {
            problems.push(format!(
                "case_id={} fingerprint mismatch: expected {} got {}",
                row.case_id, row.fingerprint, actual
            ));
        }
    }

    assert_eq!(
        reproducible, EXPECTED_REPRODUCIBLE_COUNT,
        "expected {EXPECTED_REPRODUCIBLE_COUNT} FuzzVariant-reproducible rows, found {reproducible}"
    );
    assert!(
        problems.is_empty(),
        "golden parity problems:\n{}",
        problems.join("\n")
    );
}
