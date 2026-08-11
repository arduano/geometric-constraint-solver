// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::too_many_lines,
    reason = "the exhaustive golden family inventory is intentionally one reviewable oracle"
)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_constraint_editor::{
    AuthoringApplication, AuthoringMutation, AuthoringOperand, AuthoringOptions, AuthoringOutcome,
    AuthoringState, AuthoringTool, ConstraintIntent, DimensionKind, DimensionTargetDisplayUnit,
    DimensionTargetMetadata, ResolvedConstraintKind, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId,
    DesignScalarId, DocumentAngleOrientation, DocumentConstraintDefinition,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentId, DocumentSolveRequest, PersistentId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SketchHardValidity,
    SolverConfig, TangentOrientation,
};
use proptest::prelude::{Strategy, any};
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};

const BASE_SEED_HEX: &str = "aa6ab88cc8aa4878c51d78db3d1b993355406fce8c6c42353a850c05696c2edd";
const BASE_SEED: [u8; 32] = [
    0xaa, 0x6a, 0xb8, 0x8c, 0xc8, 0xaa, 0x48, 0x78, 0xc5, 0x1d, 0x78, 0xdb, 0x3d, 0x1b, 0x99, 0x33,
    0x55, 0x40, 0x6f, 0xce, 0x8c, 0x6c, 0x42, 0x35, 0x3a, 0x85, 0x0c, 0x05, 0x69, 0x6c, 0x2e, 0xdd,
];
const SEEDED_VARIANTS: u32 = 8;
const MAX_SHRINK_ITERS: u32 = 512;
const TSV_HEADER: &str = "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint";

const CONSTRAINT_KINDS: [ResolvedConstraintKind; 16] = [
    ResolvedConstraintKind::FixedPoint,
    ResolvedConstraintKind::CoincidentPoints,
    ResolvedConstraintKind::PointOnCurve,
    ResolvedConstraintKind::CurveContact,
    ResolvedConstraintKind::HorizontalLine,
    ResolvedConstraintKind::VerticalLine,
    ResolvedConstraintKind::ParallelLines,
    ResolvedConstraintKind::PerpendicularLines,
    ResolvedConstraintKind::RadialLine,
    ResolvedConstraintKind::EqualLength,
    ResolvedConstraintKind::EqualRadius,
    ResolvedConstraintKind::EqualCurvature,
    ResolvedConstraintKind::Midpoint,
    ResolvedConstraintKind::SymmetricAboutLine,
    ResolvedConstraintKind::CurveTangency,
    ResolvedConstraintKind::EndpointContinuity,
];

const DIMENSION_KINDS: [DimensionKind; 5] = [
    DimensionKind::PointDistance,
    DimensionKind::SegmentLength,
    DimensionKind::Radius,
    DimensionKind::Diameter,
    DimensionKind::OrientedAngle,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FamilySubject {
    Constraint {
        kind: ResolvedConstraintKind,
        intent: ConstraintIntent,
    },
    Dimension(DimensionKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OracleFamily {
    id: &'static str,
    subject: FamilySubject,
}

const FAMILIES: [OracleFamily; 21] = [
    constraint_family(ResolvedConstraintKind::FixedPoint, ConstraintIntent::Lock),
    constraint_family(
        ResolvedConstraintKind::CoincidentPoints,
        ConstraintIntent::Coincident,
    ),
    constraint_family(
        ResolvedConstraintKind::PointOnCurve,
        ConstraintIntent::Coincident,
    ),
    constraint_family(
        ResolvedConstraintKind::CurveContact,
        ConstraintIntent::Coincident,
    ),
    constraint_family(
        ResolvedConstraintKind::HorizontalLine,
        ConstraintIntent::Horizontal,
    ),
    constraint_family(
        ResolvedConstraintKind::VerticalLine,
        ConstraintIntent::Vertical,
    ),
    constraint_family(
        ResolvedConstraintKind::ParallelLines,
        ConstraintIntent::Parallel,
    ),
    constraint_family(
        ResolvedConstraintKind::PerpendicularLines,
        ConstraintIntent::Perpendicular,
    ),
    constraint_family(
        ResolvedConstraintKind::RadialLine,
        ConstraintIntent::Perpendicular,
    ),
    constraint_family(ResolvedConstraintKind::EqualLength, ConstraintIntent::Equal),
    constraint_family(ResolvedConstraintKind::EqualRadius, ConstraintIntent::Equal),
    constraint_family(
        ResolvedConstraintKind::EqualCurvature,
        ConstraintIntent::Equal,
    ),
    constraint_family(ResolvedConstraintKind::Midpoint, ConstraintIntent::Midpoint),
    constraint_family(
        ResolvedConstraintKind::SymmetricAboutLine,
        ConstraintIntent::Symmetric,
    ),
    constraint_family(
        ResolvedConstraintKind::CurveTangency,
        ConstraintIntent::Tangent,
    ),
    constraint_family(
        ResolvedConstraintKind::EndpointContinuity,
        ConstraintIntent::Continuity,
    ),
    dimension_family(DimensionKind::PointDistance),
    dimension_family(DimensionKind::SegmentLength),
    dimension_family(DimensionKind::Radius),
    dimension_family(DimensionKind::Diameter),
    dimension_family(DimensionKind::OrientedAngle),
];

const fn constraint_family(kind: ResolvedConstraintKind, intent: ConstraintIntent) -> OracleFamily {
    OracleFamily {
        id: constraint_family_id(kind),
        subject: FamilySubject::Constraint { kind, intent },
    }
}

const fn dimension_family(kind: DimensionKind) -> OracleFamily {
    OracleFamily {
        id: dimension_family_id(kind),
        subject: FamilySubject::Dimension(kind),
    }
}

const fn constraint_family_id(kind: ResolvedConstraintKind) -> &'static str {
    match kind {
        ResolvedConstraintKind::FixedPoint => "constraint.fixed-point",
        ResolvedConstraintKind::CoincidentPoints => "constraint.coincident-points",
        ResolvedConstraintKind::PointOnCurve => "constraint.point-on-curve",
        ResolvedConstraintKind::CurveContact => "constraint.curve-contact",
        ResolvedConstraintKind::HorizontalLine => "constraint.horizontal-line",
        ResolvedConstraintKind::VerticalLine => "constraint.vertical-line",
        ResolvedConstraintKind::ParallelLines => "constraint.parallel-lines",
        ResolvedConstraintKind::PerpendicularLines => "constraint.perpendicular-lines",
        ResolvedConstraintKind::RadialLine => "constraint.radial-line",
        ResolvedConstraintKind::EqualLength => "constraint.equal-length",
        ResolvedConstraintKind::EqualRadius => "constraint.equal-radius",
        ResolvedConstraintKind::EqualCurvature => "constraint.equal-curvature",
        ResolvedConstraintKind::Midpoint => "constraint.midpoint",
        ResolvedConstraintKind::SymmetricAboutLine => "constraint.symmetric-about-line",
        ResolvedConstraintKind::CurveTangency => "constraint.curve-tangency",
        ResolvedConstraintKind::EndpointContinuity => "constraint.endpoint-continuity",
    }
}

const fn dimension_family_id(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::PointDistance => "dimension.point-distance",
        DimensionKind::SegmentLength => "dimension.segment-length",
        DimensionKind::Radius => "dimension.radius",
        DimensionKind::Diameter => "dimension.diameter",
        DimensionKind::OrientedAngle => "dimension.oriented-angle",
    }
}

const fn dimension_label(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::PointDistance => "Point distance",
        DimensionKind::SegmentLength => "Segment length",
        DimensionKind::Radius => "Radius",
        DimensionKind::Diameter => "Diameter",
        DimensionKind::OrientedAngle => "Oriented angle",
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Variant {
    translation: [f64; 2],
    scale: f64,
    rotation: f64,
    reverse_spans: bool,
    swap_operands: bool,
    contact_parameter: f64,
    displaced: bool,
    option_index: u8,
}

impl Variant {
    const DETERMINISTIC: Self = Self {
        translation: [0.0, 0.0],
        scale: 1.0,
        rotation: 0.0,
        reverse_spans: false,
        swap_operands: false,
        contact_parameter: 0.5,
        displaced: false,
        option_index: 0,
    };
}

fn tangent_option(variant: Variant) -> TangentOrientation {
    if variant.option_index.is_multiple_of(2) {
        TangentOrientation::Aligned
    } else {
        TangentOrientation::Opposed
    }
}

fn curvature_option(variant: Variant) -> DocumentCurveCurvatureRelation {
    match variant.option_index % 3 {
        0 => DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
        1 => DocumentCurveCurvatureRelation::Signed,
        2 => DocumentCurveCurvatureRelation::MagnitudeSameSign,
        _ => unreachable!("modulo three"),
    }
}

fn continuity_option(variant: Variant) -> DocumentCurveContinuity {
    match variant.option_index % 4 {
        0 => DocumentCurveContinuity::G1,
        1 => DocumentCurveContinuity::G0,
        2 => DocumentCurveContinuity::G2,
        3 => DocumentCurveContinuity::ParametricC2 {
            first_rate: if variant.swap_operands { 2.0 } else { 1.0 },
            second_rate: if variant.swap_operands { 1.0 } else { 2.0 },
        },
        _ => unreachable!("modulo four"),
    }
}

fn variant_strategy() -> impl Strategy<Value = Variant> {
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
                Variant {
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

#[derive(Clone, Debug)]
struct SemanticDefect {
    class: &'static str,
    message: String,
}

type OracleResult<T = ()> = Result<T, SemanticDefect>;

fn defect(class: &'static str, message: impl Into<String>) -> SemanticDefect {
    SemanticDefect {
        class,
        message: message.into(),
    }
}

#[derive(Clone, Debug)]
struct SurveyRow {
    case_id: String,
    family: &'static str,
    status: &'static str,
    finding_id: &'static str,
    failure_class: String,
    fingerprint: String,
}

impl SurveyRow {
    fn pass(case_id: String, family: &'static str, variant: Variant) -> Self {
        Self {
            case_id,
            family,
            status: "PASS",
            finding_id: "-",
            failure_class: "-".into(),
            fingerprint: input_fingerprint(family, variant),
        }
    }

    fn failed(
        case_id: String,
        family: &'static str,
        variant: Variant,
        failure: &SemanticDefect,
    ) -> Self {
        let detail = format!("variant={variant:?}; {}", failure.message);
        let hash = fnv1a64(detail.as_bytes());
        Self {
            case_id,
            family,
            status: "DEFECT",
            finding_id: "-",
            failure_class: failure.class.into(),
            fingerprint: format!("{hash:016x}:{}", sanitize_tsv(&detail)),
        }
    }

    fn panicked(case_id: String, family: &'static str, variant: Variant, message: &str) -> Self {
        let detail = format!("variant={variant:?}; {}", sanitize_tsv(message));
        let hash = fnv1a64(detail.as_bytes());
        Self {
            case_id,
            family,
            status: "PANIC",
            finding_id: "-",
            failure_class: "test-panic".into(),
            fingerprint: format!("{hash:016x}:{detail}"),
        }
    }

    fn write_to(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}",
            sanitize_tsv(&self.case_id),
            sanitize_tsv(self.family),
            self.status,
            self.finding_id,
            sanitize_tsv(&self.failure_class),
            sanitize_tsv(&self.fingerprint),
        )
    }
}

fn input_fingerprint(family: &str, variant: Variant) -> String {
    let mut bytes = Vec::with_capacity(family.len() + 8 * 6 + 4);
    bytes.extend_from_slice(family.as_bytes());
    bytes.push(0);
    for value in [
        variant.translation[0],
        variant.translation[1],
        variant.scale,
        variant.rotation,
        variant.contact_parameter,
    ] {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&[
        u8::from(variant.reverse_spans),
        u8::from(variant.swap_operands),
        u8::from(variant.displaced),
        variant.option_index,
    ]);
    format!("input-{:016x}", fnv1a64(&bytes))
}

fn sanitize_tsv(value: &str) -> String {
    value
        .chars()
        .map(|value| match value {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
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

#[test]
fn golden_oracle_inventory_and_tsv_schema_are_exhaustive() {
    assert_eq!(CONSTRAINT_KINDS.len(), 16);
    assert_eq!(DIMENSION_KINDS.len(), 5);
    assert_eq!(
        FAMILIES.len(),
        CONSTRAINT_KINDS.len() + DIMENSION_KINDS.len()
    );
    assert_eq!(TSV_HEADER.split('\t').count(), 6);
    assert_eq!(SEEDED_VARIANTS, 8);
    assert_eq!(MAX_SHRINK_ITERS, 512);
    assert_eq!(BASE_SEED_HEX.len(), 64);
    let decoded_seed = std::array::from_fn(|index| {
        let bytes = BASE_SEED_HEX.as_bytes();
        (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1])
    });
    assert_eq!(BASE_SEED, decoded_seed);

    let ids = FAMILIES
        .iter()
        .map(|family| family.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), FAMILIES.len());
    for kind in CONSTRAINT_KINDS {
        assert!(FAMILIES.iter().any(|family| {
            family.id == constraint_family_id(kind)
                && matches!(
                    family.subject,
                    FamilySubject::Constraint {
                        kind: candidate,
                        ..
                    } if candidate == kind
                )
        }));
    }
    for kind in DIMENSION_KINDS {
        assert!(FAMILIES.iter().any(|family| {
            family.id == dimension_family_id(kind)
                && family.subject == FamilySubject::Dimension(kind)
        }));
    }

    let mut bytes = Vec::new();
    SurveyRow::pass(
        "constraint.fixed-point.deterministic".into(),
        FAMILIES[0].id,
        Variant::DETERMINISTIC,
    )
    .write_to(&mut bytes)
    .expect("in-memory TSV row");
    let row = String::from_utf8(bytes).expect("UTF-8 TSV");
    assert_eq!(row.trim_end().split('\t').count(), 6);
}

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("BASE_SEED_HEX must use lowercase hexadecimal"),
    }
}

#[test]
fn golden_oracle_family_survey() {
    let selected = env::var("GEOSOLVE_GOLDEN_ORACLE_FAMILY");
    let output = env::var("GEOSOLVE_GOLDEN_ORACLE_OUTPUT");
    let selected_case = env::var("GEOSOLVE_GOLDEN_ORACLE_CASE");
    if selected.is_err() && output.is_err() && selected_case.is_err() {
        return;
    }
    let selected = selected.expect("GEOSOLVE_GOLDEN_ORACLE_FAMILY must accompany oracle output");
    let output = output.expect("GEOSOLVE_GOLDEN_ORACLE_OUTPUT must accompany oracle family");
    let selected_case =
        selected_case.expect("GEOSOLVE_GOLDEN_ORACLE_CASE must accompany oracle family");
    let family = FAMILIES
        .iter()
        .copied()
        .find(|family| family.id == selected)
        .unwrap_or_else(|| panic!("unknown golden oracle family: {selected}"));

    let file = File::create(&output)
        .unwrap_or_else(|error| panic!("cannot create oracle TSV {output}: {error}"));
    let mut output = BufWriter::new(file);
    writeln!(output, "{TSV_HEADER}").expect("write oracle TSV header");

    let row = if selected_case == "deterministic" {
        survey_deterministic(family)
    } else if let Some(index) = selected_case.strip_prefix("seed-") {
        let variant_index = index
            .parse::<u32>()
            .unwrap_or_else(|error| panic!("invalid golden oracle case {selected_case}: {error}"));
        assert!(
            variant_index < SEEDED_VARIANTS,
            "golden oracle case index is outside 0..{SEEDED_VARIANTS}: {selected_case}"
        );
        survey_seeded(family, variant_index)
    } else {
        panic!("unknown golden oracle case: {selected_case}");
    };
    row.write_to(&mut output).expect("write oracle row");
    output.flush().expect("flush complete oracle TSV");
}

fn survey_deterministic(family: OracleFamily) -> SurveyRow {
    let case_id = format!("{}.deterministic", family.id);
    let variant = effective_variant(family, Variant::DETERMINISTIC, true);
    match catch_unwind(AssertUnwindSafe(|| survey_one(family, variant, true))) {
        Ok(Ok(())) => SurveyRow::pass(case_id, family.id, variant),
        Ok(Err(failure)) => SurveyRow::failed(case_id, family.id, variant, &failure),
        Err(payload) => SurveyRow::panicked(case_id, family.id, variant, &panic_payload(&payload)),
    }
}

fn survey_seeded(family: OracleFamily, variant_index: u32) -> SurveyRow {
    let seed = oracle_seed(family.id, variant_index);
    let config = Config {
        cases: 1,
        max_shrink_iters: MAX_SHRINK_ITERS,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &seed));
    let last_failure = RefCell::new(None::<(Variant, SemanticDefect)>);
    let last_variant = RefCell::new(Variant::DETERMINISTIC);
    let result = catch_unwind(AssertUnwindSafe(|| {
        runner.run(&variant_strategy(), |mut variant| {
            variant.reverse_spans = variant_index & 1 != 0;
            variant.displaced = variant_index & 2 != 0;
            variant.swap_operands = variant_index & 4 != 0;
            variant.option_index = u8::try_from(variant_index).expect("eight option variants");
            if matches!(
                family.subject,
                FamilySubject::Constraint {
                    kind: ResolvedConstraintKind::EndpointContinuity,
                    ..
                }
            ) && variant_index == 3
            {
                // Retain one pre-satisfied unequal-rate Parametric-C2 witness; seed-07
                // remains its displaced, operand-swapped recovery counterpart.
                variant.displaced = false;
            }
            let variant = effective_variant(family, variant, false);
            *last_variant.borrow_mut() = variant;
            match survey_one(family, variant, false) {
                Ok(()) => Ok(()),
                Err(failure) => {
                    *last_failure.borrow_mut() = Some((variant, failure.clone()));
                    Err(TestCaseError::fail(format!(
                        "{}: {}",
                        failure.class, failure.message
                    )))
                }
            }
        })
    }));
    let case_id = format!("{}.seed-{variant_index:02}", family.id);
    match result {
        Ok(Ok(())) => SurveyRow::pass(case_id, family.id, last_variant.into_inner()),
        Ok(Err(_)) => {
            let (variant, failure) = last_failure
                .into_inner()
                .expect("proptest error without semantic failure is a harness defect");
            SurveyRow::failed(case_id, family.id, variant, &failure)
        }
        Err(payload) => SurveyRow::panicked(
            case_id,
            family.id,
            last_variant.into_inner(),
            &panic_payload(&payload),
        ),
    }
}

fn oracle_seed(family: &str, variant_index: u32) -> [u8; 32] {
    let mut seed = BASE_SEED;
    let mut state =
        fnv1a64(family.as_bytes()) ^ u64::from(variant_index).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for chunk in seed.chunks_exact_mut(8) {
        state = splitmix64(state);
        let original = u64::from_le_bytes(chunk.try_into().expect("eight-byte seed chunk"));
        chunk.copy_from_slice(&(original ^ state).to_le_bytes());
    }
    seed
}

const fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    state ^ (state >> 31)
}

fn panic_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".into()
    }
}

fn survey_one(family: OracleFamily, variant: Variant, compare_preselection: bool) -> OracleResult {
    let variant = effective_variant(family, variant, compare_preselection);
    let fixture = MatrixFixture::new(family.subject, variant);
    match family.subject {
        FamilySubject::Constraint { kind, intent } => {
            survey_constraint(kind, intent, &fixture, variant, compare_preselection)
        }
        FamilySubject::Dimension(kind) => {
            survey_dimension(kind, &fixture, variant, compare_preselection)
        }
    }
}

fn effective_variant(
    family: OracleFamily,
    mut variant: Variant,
    compare_preselection: bool,
) -> Variant {
    if matches!(
        family.subject,
        FamilySubject::Constraint {
            kind: ResolvedConstraintKind::HorizontalLine | ResolvedConstraintKind::VerticalLine,
            ..
        }
    ) {
        variant.rotation = 0.0;
    }
    if !compare_preselection
        && matches!(
            family.subject,
            FamilySubject::Constraint {
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

#[derive(Clone, Copy)]
struct Transform {
    translation: [f64; 2],
    scale: f64,
    cosine: f64,
    sine: f64,
}

impl Transform {
    fn new(variant: Variant) -> Self {
        Self {
            translation: variant.translation,
            scale: variant.scale,
            cosine: variant.rotation.cos(),
            sine: variant.rotation.sin(),
        }
    }

    fn point(self, value: [f64; 2]) -> [f64; 2] {
        [
            self.translation[0] + self.scale * value[0].mul_add(self.cosine, -value[1] * self.sine),
            self.translation[1] + self.scale * value[0].mul_add(self.sine, value[1] * self.cosine),
        ]
    }
}

struct MatrixFixture {
    document: SketchDocument,
    transform: Transform,
    points: [DesignPointId; 6],
    coincident: [DesignPointId; 2],
    contact_point: DesignPointId,
    line_midpoint: DesignPointId,
    lines: [CurveSpan; 2],
    circles: [CurveSpan; 2],
    circle_radii: [DesignScalarId; 2],
    beziers: [CurveSpan; 2],
    overlapping_line: CurveSpan,
    radial_line: CurveSpan,
    horizontal_contact_parameter: f64,
    overlap_contact_parameter: f64,
    radial_parameter: f64,
    original_points: Vec<(DesignPointId, [f64; 2])>,
    original_scalars: Vec<(DesignScalarId, f64)>,
    pre_satisfied: bool,
}

impl MatrixFixture {
    fn new(subject: FamilySubject, variant: Variant) -> Self {
        let transform = Transform::new(variant);
        let displaced_kind = match subject {
            FamilySubject::Constraint { kind, .. } if variant.displaced => Some(kind),
            FamilySubject::Constraint { .. } | FamilySubject::Dimension(_) => None,
        };
        let displaced = |kind| displaced_kind == Some(kind);
        let mut document = SketchDocument::with_id(
            10.0 * variant.scale,
            DocumentId(PersistentId::from_u128(0x4d37_3042_4831_4155_5448_4f52)),
        )
        .expect("oracle document");
        let p = variant.contact_parameter;
        let intersection_x = -2.0 + 4.0 * p;
        let primary_end = [
            2.0,
            if displaced(ResolvedConstraintKind::HorizontalLine) {
                0.3
            } else {
                0.0
            },
        ];
        let cross_x_offset = if displaced(ResolvedConstraintKind::VerticalLine)
            || displaced(ResolvedConstraintKind::PerpendicularLines)
        {
            0.3
        } else {
            0.0
        };
        let cross_y_offset = if displaced(ResolvedConstraintKind::CurveContact) {
            0.2
        } else {
            0.0
        };
        let symmetric_y_offset = if displaced(ResolvedConstraintKind::SymmetricAboutLine) {
            0.3
        } else {
            0.0
        };
        let points = [
            add_point(&mut document, transform, "a", [-2.0, 0.0]),
            add_point(&mut document, transform, "b", primary_end),
            add_point(
                &mut document,
                transform,
                "c",
                [intersection_x, -2.0 + cross_y_offset],
            ),
            add_point(
                &mut document,
                transform,
                "d",
                [intersection_x + cross_x_offset, 2.0 + cross_y_offset],
            ),
            add_point(&mut document, transform, "e", [intersection_x - 3.0, 3.0]),
            add_point(
                &mut document,
                transform,
                "f",
                [intersection_x + 3.0, 3.0 + symmetric_y_offset],
            ),
        ];
        let first_line = add_line_from_points(
            &mut document,
            "primary line",
            points[0],
            points[1],
            variant.reverse_spans,
        );
        let second_line = add_line_from_points(
            &mut document,
            "cross line",
            points[2],
            points[3],
            variant.reverse_spans,
        );
        let first_radius = document
            .add_scalar(
                "first radius",
                variant.scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("first radius");
        let first_circle = CurveSpan::line(
            document
                .add_curve(
                    "first circle",
                    CurveDefinition::Circle {
                        center: points[4],
                        radius: first_radius,
                    },
                )
                .expect("first circle"),
        );
        let second_radius = document
            .add_scalar(
                "second radius",
                variant.scale
                    * if displaced(ResolvedConstraintKind::EqualRadius) {
                        1.2
                    } else {
                        1.0
                    },
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("second radius");
        let second_circle = CurveSpan::line(
            document
                .add_curve(
                    "second circle",
                    CurveDefinition::Circle {
                        center: points[5],
                        radius: second_radius,
                    },
                )
                .expect("second circle"),
        );
        let contact_point = add_point(
            &mut document,
            transform,
            "contact point",
            [
                intersection_x,
                if displaced(ResolvedConstraintKind::PointOnCurve) {
                    0.25
                } else {
                    0.0
                },
            ],
        );
        let line_midpoint = add_point(
            &mut document,
            transform,
            "line midpoint",
            [
                0.0,
                if displaced(ResolvedConstraintKind::Midpoint) {
                    0.25
                } else {
                    0.0
                },
            ],
        );
        let coincident = [
            add_point(&mut document, transform, "coincident a", [5.0, 5.0]),
            add_point(
                &mut document,
                transform,
                "coincident b",
                [
                    5.0 + if displaced(ResolvedConstraintKind::CoincidentPoints) {
                        0.25
                    } else {
                        0.0
                    },
                    5.0,
                ],
            ),
        ];
        let first_bezier_controls = [
            add_point(&mut document, transform, "bezier 1 start", [-4.0, -4.0]),
            add_point(&mut document, transform, "bezier 1 middle", [-2.0, -2.0]),
            add_point(&mut document, transform, "bezier seam", [0.0, -4.0]),
        ];
        let second_bezier_start = if displaced(ResolvedConstraintKind::EndpointContinuity) {
            add_point(
                &mut document,
                transform,
                "displaced bezier 2 start",
                [0.2, -4.1],
            )
        } else {
            first_bezier_controls[2]
        };
        let endpoint_continuity = matches!(
            subject,
            FamilySubject::Constraint {
                kind: ResolvedConstraintKind::EndpointContinuity,
                ..
            }
        );
        let parametric_c2 = endpoint_continuity
            && matches!(
                continuity_option(variant),
                DocumentCurveContinuity::ParametricC2 { .. }
            );
        let second_end = if parametric_c2 {
            [2.0, -7.0]
        } else if endpoint_continuity && continuity_option(variant) == DocumentCurveContinuity::G2 {
            [4.0, -12.0]
        } else {
            [4.0, -4.0]
        };
        let second_middle = if parametric_c2 {
            [1.0, -5.0]
        } else {
            [
                2.0,
                if displaced(ResolvedConstraintKind::EqualCurvature) {
                    -6.4
                } else {
                    -6.0
                },
            ]
        };
        let mut second_bezier_controls = [
            second_bezier_start,
            add_point(&mut document, transform, "bezier 2 middle", second_middle),
            add_point(&mut document, transform, "bezier 2 end", second_end),
        ];
        if matches!(
            subject,
            FamilySubject::Constraint {
                kind: ResolvedConstraintKind::EqualCurvature,
                ..
            }
        ) && curvature_option(variant) != DocumentCurveCurvatureRelation::MagnitudeOppositeSign
        {
            second_bezier_controls.reverse();
        }
        if endpoint_continuity && variant.option_index >= 4 {
            second_bezier_controls.reverse();
        }
        let beziers = [first_bezier_controls, second_bezier_controls].map(|controls| {
            CurveSpan::line(
                document
                    .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
                    .expect("quadratic Bezier"),
            )
        });
        let tangent_shift = if displaced(ResolvedConstraintKind::CurveTangency) {
            0.2
        } else {
            0.0
        };
        let overlap_start = add_point(&mut document, transform, "overlap a", [-2.0, tangent_shift]);
        let overlap_end = add_point(
            &mut document,
            transform,
            "overlap b",
            [
                2.0 + if displaced(ResolvedConstraintKind::EqualLength) {
                    0.5
                } else {
                    0.0
                },
                tangent_shift
                    + if displaced(ResolvedConstraintKind::ParallelLines) {
                        0.3
                    } else {
                        0.0
                    },
            ],
        );
        let overlap_reverse = variant.reverse_spans
            ^ (matches!(
                subject,
                FamilySubject::Constraint {
                    kind: ResolvedConstraintKind::CurveTangency,
                    ..
                }
            ) && tangent_option(variant) == TangentOrientation::Opposed);
        let overlapping_line = add_line_from_points(
            &mut document,
            "overlapping line",
            overlap_start,
            overlap_end,
            overlap_reverse,
        );
        let radial_base = [intersection_x - 3.0, 3.0];
        let radial_normal_offset = if displaced(ResolvedConstraintKind::RadialLine) {
            0.25
        } else {
            0.0
        };
        let radial_start = add_point(
            &mut document,
            transform,
            "radial segment start",
            [radial_base[0] + 2.0, radial_base[1] + radial_normal_offset],
        );
        let radial_end = add_point(
            &mut document,
            transform,
            "radial segment end",
            [radial_base[0] + 3.0, radial_base[1] + radial_normal_offset],
        );
        let radial_line = add_line_from_points(
            &mut document,
            "external radial segment",
            radial_start,
            radial_end,
            variant.reverse_spans,
        );
        let horizontal_contact_parameter = if variant.reverse_spans { 1.0 - p } else { p };
        let overlap_contact_parameter = if overlap_reverse { 1.0 - p } else { p };
        let [radial_start, radial_end] =
            line_points(&document, radial_line).expect("radial fixture line");
        let radial_center = document.point(points[4]).expect("radial center").position;
        let radial_direction = [
            radial_end[0] - radial_start[0],
            radial_end[1] - radial_start[1],
        ];
        let radial_offset = [
            radial_center[0] - radial_start[0],
            radial_center[1] - radial_start[1],
        ];
        let radial_length = radial_direction[0].hypot(radial_direction[1]);
        let radial_unit = [
            radial_direction[0] / radial_length,
            radial_direction[1] / radial_length,
        ];
        let radial_parameter = radial_offset[0]
            .mul_add(radial_unit[0], radial_offset[1] * radial_unit[1])
            / radial_length;
        let original_points = document
            .points()
            .iter()
            .map(|point| (point.id, point.position))
            .collect();
        let original_scalars = document
            .scalars()
            .iter()
            .map(|scalar| (scalar.id, scalar.value))
            .collect();
        Self {
            document,
            transform,
            points,
            coincident,
            contact_point,
            line_midpoint,
            lines: [first_line, second_line],
            circles: [first_circle, second_circle],
            circle_radii: [first_radius, second_radius],
            beziers,
            overlapping_line,
            radial_line,
            horizontal_contact_parameter,
            overlap_contact_parameter,
            radial_parameter,
            original_points,
            original_scalars,
            pre_satisfied: displaced_kind.is_none()
                || displaced_kind == Some(ResolvedConstraintKind::FixedPoint),
        }
    }
}

fn add_point(
    document: &mut SketchDocument,
    transform: Transform,
    label: &str,
    position: [f64; 2],
) -> DesignPointId {
    document
        .add_point(label, transform.point(position))
        .unwrap_or_else(|error| panic!("add oracle point {label}: {error}"))
}

fn add_line_from_points(
    document: &mut SketchDocument,
    label: &str,
    first: DesignPointId,
    second: DesignPointId,
    reverse: bool,
) -> CurveSpan {
    let [start, end] = if reverse {
        [second, first]
    } else {
        [first, second]
    };
    let start_position = document.point(start).expect("line start point").position;
    let end_position = document.point(end).expect("line end point").position;
    let delta = [
        end_position[0] - start_position[0],
        end_position[1] - start_position[1],
    ];
    let length = delta[0].hypot(delta[1]);
    CurveSpan::line(
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .unwrap_or_else(|error| panic!("add oracle line {label}: {error}")),
    )
}

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("oracle parent document must be valid");
    RetainedEditorCoordinator::new(session).expect("oracle coordinator")
}

fn constraint_operands(
    kind: ResolvedConstraintKind,
    fixture: &MatrixFixture,
    variant: Variant,
) -> Vec<AuthoringOperand> {
    let selected = |item| AuthoringOperand::selected(item);
    let picked = |item, parameter| AuthoringOperand::picked(item, Some(parameter));
    let p = fixture.horizontal_contact_parameter;
    let mut operands = match kind {
        ResolvedConstraintKind::FixedPoint => {
            vec![selected(SelectionItem::Point(fixture.points[0]))]
        }
        ResolvedConstraintKind::CoincidentPoints => fixture
            .coincident
            .map(SelectionItem::Point)
            .map(selected)
            .to_vec(),
        ResolvedConstraintKind::PointOnCurve => vec![
            selected(SelectionItem::Point(fixture.contact_point)),
            picked(SelectionItem::Curve(fixture.lines[0]), p),
        ],
        ResolvedConstraintKind::CurveContact => vec![
            picked(SelectionItem::Curve(fixture.lines[0]), p),
            picked(SelectionItem::Curve(fixture.lines[1]), 0.5),
        ],
        ResolvedConstraintKind::HorizontalLine => {
            vec![selected(SelectionItem::Curve(fixture.lines[0]))]
        }
        ResolvedConstraintKind::VerticalLine => {
            vec![selected(SelectionItem::Curve(fixture.lines[1]))]
        }
        ResolvedConstraintKind::ParallelLines | ResolvedConstraintKind::EqualLength => vec![
            selected(SelectionItem::Curve(fixture.lines[0])),
            selected(SelectionItem::Curve(fixture.overlapping_line)),
        ],
        ResolvedConstraintKind::PerpendicularLines => fixture
            .lines
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
        ResolvedConstraintKind::RadialLine => vec![
            picked(SelectionItem::Curve(fixture.circles[0]), 0.0),
            picked(
                SelectionItem::Curve(fixture.radial_line),
                variant.contact_parameter,
            ),
        ],
        ResolvedConstraintKind::EqualRadius => fixture
            .circles
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
        ResolvedConstraintKind::EqualCurvature => fixture
            .beziers
            .map(|span| picked(SelectionItem::Curve(span), 0.5))
            .to_vec(),
        ResolvedConstraintKind::Midpoint => vec![
            selected(SelectionItem::Point(fixture.line_midpoint)),
            selected(SelectionItem::Curve(fixture.lines[0])),
        ],
        ResolvedConstraintKind::SymmetricAboutLine => vec![
            selected(SelectionItem::Point(fixture.points[4])),
            selected(SelectionItem::Point(fixture.points[5])),
            selected(SelectionItem::Curve(fixture.lines[1])),
        ],
        ResolvedConstraintKind::CurveTangency => vec![
            picked(SelectionItem::Curve(fixture.lines[0]), p),
            picked(
                SelectionItem::Curve(fixture.overlapping_line),
                fixture.overlap_contact_parameter,
            ),
        ],
        ResolvedConstraintKind::EndpointContinuity => vec![
            picked(SelectionItem::Curve(fixture.beziers[0]), 1.0),
            picked(
                SelectionItem::Curve(fixture.beziers[1]),
                if variant.option_index >= 4 { 1.0 } else { 0.0 },
            ),
        ],
    };
    if variant.swap_operands {
        match kind {
            ResolvedConstraintKind::CoincidentPoints
            | ResolvedConstraintKind::PointOnCurve
            | ResolvedConstraintKind::CurveContact
            | ResolvedConstraintKind::ParallelLines
            | ResolvedConstraintKind::PerpendicularLines
            | ResolvedConstraintKind::RadialLine
            | ResolvedConstraintKind::EqualLength
            | ResolvedConstraintKind::EqualRadius
            | ResolvedConstraintKind::EqualCurvature
            | ResolvedConstraintKind::Midpoint
            | ResolvedConstraintKind::CurveTangency
            | ResolvedConstraintKind::EndpointContinuity => operands.reverse(),
            ResolvedConstraintKind::SymmetricAboutLine => operands.swap(0, 1),
            ResolvedConstraintKind::FixedPoint
            | ResolvedConstraintKind::HorizontalLine
            | ResolvedConstraintKind::VerticalLine => {}
        }
    }
    operands
}

fn dimension_operands(
    kind: DimensionKind,
    fixture: &MatrixFixture,
    variant: Variant,
) -> Vec<AuthoringOperand> {
    let selected = |item| AuthoringOperand::selected(item);
    let mut operands = match kind {
        DimensionKind::PointDistance => fixture.points[0..2]
            .iter()
            .copied()
            .map(SelectionItem::Point)
            .map(selected)
            .collect(),
        DimensionKind::SegmentLength => {
            vec![selected(SelectionItem::Curve(fixture.lines[0]))]
        }
        DimensionKind::Radius => {
            vec![selected(SelectionItem::Curve(fixture.circles[0]))]
        }
        DimensionKind::Diameter => {
            vec![selected(SelectionItem::Curve(fixture.circles[1]))]
        }
        DimensionKind::OrientedAngle => fixture
            .lines
            .map(SelectionItem::Curve)
            .map(selected)
            .to_vec(),
    };
    if variant.swap_operands
        && matches!(
            kind,
            DimensionKind::PointDistance | DimensionKind::OrientedAngle
        )
    {
        operands.reverse();
    }
    operands
}

fn authoring_application(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
    options: AuthoringOptions,
    compare_preselection: bool,
) -> OracleResult<AuthoringApplication> {
    let preselected = if compare_preselection {
        let mut authoring = AuthoringState::default();
        authoring.set_options(options);
        match authoring.activate(document, tool, operands) {
            AuthoringOutcome::Apply(application) => Some(application),
            other => {
                return Err(defect(
                    "authoring.preselection",
                    format!("complete compatible preselection returned {other:?}"),
                ));
            }
        }
    } else {
        None
    };

    let mut authoring = AuthoringState::default();
    authoring.set_options(options);
    if !matches!(
        authoring.activate(document, tool, &[]),
        AuthoringOutcome::ModeEntered { .. }
    ) {
        return Err(defect(
            "authoring.mode-entry",
            "empty selection did not enter repeated authoring mode",
        ));
    }
    let mut application = None;
    for (index, operand) in operands.iter().copied().enumerate() {
        let outcome = authoring.pick(document, operand);
        if index + 1 == operands.len() {
            let AuthoringOutcome::Apply(candidate) = outcome else {
                return Err(defect(
                    "authoring.completion",
                    format!("terminal pick returned {outcome:?}"),
                ));
            };
            application = Some(candidate);
        } else if !matches!(outcome, AuthoringOutcome::Collecting { .. }) {
            return Err(defect(
                "authoring.prefix",
                format!("valid pending prefix returned {outcome:?}"),
            ));
        }
    }
    let application = application.ok_or_else(|| {
        defect(
            "authoring.completion",
            "operand sequence produced no application",
        )
    })?;
    if preselected
        .as_ref()
        .is_some_and(|value| value != &application)
    {
        return Err(defect(
            "authoring.path-parity",
            format!(
                "preselection {:?} differs from repeated-pick {application:?}",
                preselected.expect("checked Some")
            ),
        ));
    }
    Ok(application)
}

fn survey_constraint(
    kind: ResolvedConstraintKind,
    intent: ConstraintIntent,
    fixture: &MatrixFixture,
    variant: Variant,
    compare_preselection: bool,
) -> OracleResult {
    let operands = constraint_operands(kind, fixture, variant);
    let options = AuthoringOptions {
        tangent_orientation: tangent_option(variant),
        curvature_relation: curvature_option(variant),
        continuity: continuity_option(variant),
        dimension_mode: DocumentDimensionMode::Driving,
        angle_orientation: DocumentAngleOrientation::CounterClockwise,
    };
    let mut coordinator = coordinator(fixture.document.clone());
    let application = authoring_application(
        coordinator.session().design_document(),
        AuthoringTool::Constraint(intent),
        &operands,
        options,
        compare_preselection,
    )?;
    if application.resolved_constraint != Some(kind) {
        return Err(defect(
            "authoring.resolution",
            format!(
                "expected {kind:?}, got {:?}",
                application.resolved_constraint
            ),
        ));
    }
    let history_before = (coordinator.history_len(), coordinator.history_cursor());
    let mutation = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .map_err(|error| defect("authoring.transaction", error.to_string()))?;
    let AuthoringMutation::Constraint(outcome) = mutation else {
        return Err(defect(
            "authoring.mutation-kind",
            "constraint application returned a dimension mutation",
        ));
    };
    if outcome.published_accepted.is_none() {
        return Err(defect(
            "solver.rejected",
            format!(
                "constraint retained a rejected attempt: {:?}",
                coordinator.session().latest_attempt_diagnostics().solve
            ),
        ));
    }
    if (coordinator.history_len(), coordinator.history_cursor())
        != (history_before.0 + 1, history_before.1 + 1)
    {
        return Err(defect(
            "lifecycle.history",
            "accepted constraint did not add exactly one history checkpoint",
        ));
    }
    validate_current_acceptance(&coordinator)?;
    validate_constraint_definition(
        kind,
        fixture,
        variant,
        &operands,
        outcome.value,
        coordinator.session().design_document(),
    )?;
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "accepted publication is not current"))?
        .document();
    validate_finite_geometry(accepted)?;
    validate_constraint_geometry(kind, accepted, outcome.value, fixture)?;
    validate_no_move_witness(fixture, accepted)?;
    validate_protected_geometry(fixture, accepted, variant.scale)
}

fn survey_dimension(
    kind: DimensionKind,
    fixture: &MatrixFixture,
    variant: Variant,
    compare_preselection: bool,
) -> OracleResult {
    let operands = dimension_operands(kind, fixture, variant);
    let options = AuthoringOptions {
        dimension_mode: DocumentDimensionMode::Driving,
        ..AuthoringOptions::default()
    };
    let mut coordinator = coordinator(fixture.document.clone());
    let precreate_document = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "pre-create geometry is not current"))?
        .document();
    let initial_measurement = measure_dimension_operands(
        kind,
        &operands,
        precreate_document,
        options.angle_orientation,
    )?;
    let initial_tolerance = dimension_tolerance(kind, precreate_document);
    let application = authoring_application(
        coordinator.session().design_document(),
        AuthoringTool::Dimension(kind),
        &operands,
        options,
        compare_preselection,
    )?;
    if application.resolved_constraint.is_some() {
        return Err(defect(
            "authoring.resolution",
            "dimension application unexpectedly published a constraint resolution",
        ));
    }
    let history_before = (coordinator.history_len(), coordinator.history_cursor());
    let mutation = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .map_err(|error| defect("dimension.create", error.to_string()))?;
    let AuthoringMutation::Dimension(outcome) = mutation else {
        return Err(defect(
            "authoring.mutation-kind",
            "dimension application returned a constraint mutation",
        ));
    };
    if outcome.published_accepted.is_none() {
        return Err(defect(
            "solver.rejected",
            "dimension creation retained a rejected attempt",
        ));
    }
    let dimension = outcome.value;
    validate_current_acceptance(&coordinator)?;
    validate_dimension_definition(
        kind,
        &operands,
        dimension,
        coordinator.session().design_document(),
    )?;
    let original_metadata = coordinator
        .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
        .ok_or_else(|| {
            defect(
                "dimension.metadata",
                "created dimension has no target metadata",
            )
        })?;
    validate_dimension_metadata(
        kind,
        dimension,
        coordinator.session().design_document(),
        original_metadata,
    )?;
    if (original_metadata.value - initial_measurement).abs() > initial_tolerance {
        return Err(defect(
            "dimension.initial-target",
            format!(
                "authored target {} differs from pre-create accepted measurement {initial_measurement}",
                original_metadata.value
            ),
        ));
    }
    let accepted_created = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "created dimension is not current"))?
        .document();
    validate_finite_geometry(accepted_created)?;
    validate_dimension_geometry(kind, dimension, accepted_created, original_metadata.value)?;
    validate_no_move_witness(fixture, accepted_created)?;

    let edited_display = if kind == DimensionKind::OrientedAngle {
        (original_metadata.display_value - 12.0).max(15.0)
    } else {
        original_metadata.display_value * 1.05
    };
    let edit = coordinator
        .set_dimension_display_target(
            coordinator.session().design_identity(),
            dimension,
            edited_display,
        )
        .map_err(|error| defect("dimension.edit", error.to_string()))?;
    if edit.published_accepted.is_none() {
        return Err(defect(
            "solver.rejected",
            "valid dimension target edit retained a rejected attempt",
        ));
    }
    validate_current_acceptance(&coordinator)?;
    let edited_metadata = coordinator
        .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
        .ok_or_else(|| defect("dimension.metadata", "edited dimension disappeared"))?;
    validate_dimension_metadata(
        kind,
        dimension,
        coordinator.session().design_document(),
        edited_metadata,
    )?;
    if (edited_metadata.display_value - edited_display).abs() > 1.0e-9 {
        return Err(defect(
            "dimension.edit",
            format!(
                "requested display target {edited_display}, persisted {}",
                edited_metadata.display_value
            ),
        ));
    }
    let edited_bits = edited_metadata.value.to_bits();
    validate_dimension_definition(
        kind,
        &operands,
        dimension,
        coordinator.session().design_document(),
    )?;
    let accepted_edited = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "edited dimension is not current"))?
        .document();
    validate_finite_geometry(accepted_edited)?;
    validate_dimension_geometry(kind, dimension, accepted_edited, edited_metadata.value)?;
    coordinator
        .undo()
        .map_err(|error| defect("dimension.undo", error.to_string()))?;
    validate_current_acceptance(&coordinator)?;
    let undo_metadata = coordinator
        .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
        .ok_or_else(|| defect("dimension.undo", "undo did not retain created dimension ID"))?;
    validate_dimension_metadata(
        kind,
        dimension,
        coordinator.session().design_document(),
        undo_metadata,
    )?;
    if undo_metadata.value.to_bits() != original_metadata.value.to_bits() {
        return Err(defect(
            "dimension.undo",
            "undo did not restore the exact original target",
        ));
    }
    validate_dimension_definition(
        kind,
        &operands,
        dimension,
        coordinator.session().design_document(),
    )?;
    let accepted_undo = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "undone dimension is not current"))?
        .document();
    validate_finite_geometry(accepted_undo)?;
    validate_dimension_geometry(kind, dimension, accepted_undo, undo_metadata.value)?;
    coordinator
        .redo()
        .map_err(|error| defect("dimension.redo", error.to_string()))?;
    validate_current_acceptance(&coordinator)?;
    let redo_metadata = coordinator
        .dimension_target_metadata_for(&[SelectionItem::Dimension(dimension)])
        .ok_or_else(|| defect("dimension.redo", "redo did not retain dimension ID"))?;
    validate_dimension_metadata(
        kind,
        dimension,
        coordinator.session().design_document(),
        redo_metadata,
    )?;
    if redo_metadata.value.to_bits() != edited_bits {
        return Err(defect(
            "dimension.redo",
            "redo did not restore the exact edited target",
        ));
    }
    validate_dimension_definition(
        kind,
        &operands,
        dimension,
        coordinator.session().design_document(),
    )?;
    if (coordinator.history_len(), coordinator.history_cursor())
        != (history_before.0 + 2, history_before.1 + 2)
    {
        return Err(defect(
            "lifecycle.history",
            "create/edit/undo/redo did not retain the expected history shape",
        ));
    }
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "redone dimension is not current"))?
        .document();
    validate_finite_geometry(accepted)?;
    validate_dimension_geometry(kind, dimension, accepted, redo_metadata.value)?;
    validate_protected_geometry(fixture, accepted, variant.scale)
}

fn validate_dimension_metadata(
    kind: DimensionKind,
    dimension: geosolve_sketch::DocumentDimensionId,
    document: &SketchDocument,
    metadata: DimensionTargetMetadata,
) -> OracleResult {
    let stored = document
        .dimension(dimension)
        .ok_or_else(|| defect("dimension.metadata", "selected dimension disappeared"))?;
    let expected_scalar = dimension_target_scalar(&stored.definition);
    let (expected_unit, expected_display_unit) = if kind == DimensionKind::OrientedAngle {
        (ScalarUnit::Angle, DimensionTargetDisplayUnit::AcuteDegrees)
    } else {
        (ScalarUnit::Length, DimensionTargetDisplayUnit::ModelUnits)
    };
    let expected_display_value = if kind == DimensionKind::OrientedAngle {
        let line_angle = metadata.value.rem_euclid(std::f64::consts::PI);
        line_angle
            .min(std::f64::consts::PI - line_angle)
            .to_degrees()
    } else {
        metadata.value
    };
    let display_matches = if kind == DimensionKind::OrientedAngle {
        (metadata.display_value - expected_display_value).abs()
            <= 1.0e-10 * expected_display_value.abs().max(1.0)
    } else {
        metadata.display_value.to_bits() == expected_display_value.to_bits()
    };
    let display_domain_valid = if kind == DimensionKind::OrientedAngle {
        metadata.display_value >= 0.0
    } else {
        metadata.display_value > 0.0
    };
    if metadata.dimension != dimension
        || metadata.scalar != expected_scalar
        || metadata.mode != DocumentDimensionMode::Driving
        || metadata.unit != expected_unit
        || metadata.display_unit != expected_display_unit
        || !metadata.value.is_finite()
        || metadata.value <= 0.0
        || !metadata.display_value.is_finite()
        || !display_domain_valid
        || !display_matches
    {
        return Err(defect(
            "dimension.metadata",
            format!(
                "expected active {expected_unit:?}/{expected_display_unit:?} target metadata, got {metadata:?}"
            ),
        ));
    }
    Ok(())
}

fn measure_dimension_operands(
    kind: DimensionKind,
    operands: &[AuthoringOperand],
    document: &SketchDocument,
    orientation: DocumentAngleOrientation,
) -> OracleResult<f64> {
    let items = operands
        .iter()
        .map(|operand| operand.item)
        .collect::<Vec<_>>();
    match (kind, items.as_slice()) {
        (
            DimensionKind::PointDistance,
            [SelectionItem::Point(first), SelectionItem::Point(second)],
        ) => Ok(distance(
            document
                .point(*first)
                .ok_or_else(|| defect("geometry.missing", "distance first point disappeared"))?
                .position,
            document
                .point(*second)
                .ok_or_else(|| defect("geometry.missing", "distance second point disappeared"))?
                .position,
        )),
        (DimensionKind::SegmentLength, [SelectionItem::Curve(curve)]) => {
            line_length(document, *curve)
        }
        (DimensionKind::Radius, [SelectionItem::Curve(curve)]) => {
            circle_radius(document, curve.curve)
        }
        (DimensionKind::Diameter, [SelectionItem::Curve(curve)]) => {
            Ok(2.0 * circle_radius(document, curve.curve)?)
        }
        (
            DimensionKind::OrientedAngle,
            [SelectionItem::Curve(first), SelectionItem::Curve(second)],
        ) => Ok(oriented_line_angle(document, *first, *second, orientation)?
            .rem_euclid(std::f64::consts::TAU)),
        _ => Err(defect(
            "geometry.oracle",
            format!("unexpected operands for {kind:?}: {items:?}"),
        )),
    }
}

fn validate_current_acceptance(coordinator: &RetainedEditorCoordinator) -> OracleResult {
    let diagnostics = coordinator.session().latest_attempt_diagnostics();
    let solve = diagnostics.solve.ok_or_else(|| {
        defect(
            "solver.diagnostics",
            "accepted attempt has no solve diagnostics",
        )
    })?;
    if !solve.accepted
        || solve.hard_validity != SketchHardValidity::Valid
        || !solve.hard_residuals_validated
        || !solve
            .maximum_normalized_hard_residual
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    {
        return Err(defect(
            "solver.validation",
            format!("success lacks independent hard validation: {solve:?}"),
        ));
    }
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .ok_or_else(|| defect("lifecycle.authority", "no exact-current accepted state"))?;
    if accepted.design_identity() != coordinator.session().design_identity()
        || accepted.originating_attempt() != coordinator.session().last_attempt().identity()
    {
        return Err(defect(
            "lifecycle.identity",
            "accepted/design/attempt identities are incoherent",
        ));
    }
    Ok(())
}

fn validate_constraint_definition(
    kind: ResolvedConstraintKind,
    fixture: &MatrixFixture,
    variant: Variant,
    operands: &[AuthoringOperand],
    constraint: geosolve_sketch::DocumentConstraintId,
    document: &SketchDocument,
) -> OracleResult {
    let stored = document
        .constraint(constraint)
        .ok_or_else(|| defect("persistence.missing", "created constraint ID is absent"))?;
    if stored.suppressed || stored.label != kind.label() {
        return Err(defect(
            "persistence.constraint",
            format!(
                "unexpected label/suppression for {kind:?}: label={:?}, suppressed={}",
                stored.label, stored.suppressed
            ),
        ));
    }
    let definition = &stored.definition;
    let items = operands.iter().map(|value| value.item).collect::<Vec<_>>();
    let point = |index: usize| match items[index] {
        SelectionItem::Point(point) => Some(point),
        _ => None,
    };
    let span = |index: usize| match items[index] {
        SelectionItem::Curve(span) => Some(span),
        _ => None,
    };
    let exact = match (kind, definition) {
        (
            ResolvedConstraintKind::FixedPoint,
            DocumentConstraintDefinition::FixedPoint {
                point: stored,
                target,
            },
        ) => {
            *stored == point(0).expect("fixed operand")
                && target.map(f64::to_bits)
                    == document
                        .point(*stored)
                        .expect("fixed point")
                        .position
                        .map(f64::to_bits)
        }
        (
            ResolvedConstraintKind::CoincidentPoints,
            DocumentConstraintDefinition::Coincident { first, second },
        ) => Some(*first) == point(0) && Some(*second) == point(1),
        (
            ResolvedConstraintKind::PointOnCurve,
            DocumentConstraintDefinition::PointOnCurve {
                point: stored,
                contact,
            },
        ) => {
            let curve_index = usize::from(point(0).is_some());
            Some(*stored)
                == items.iter().find_map(|item| match item {
                    SelectionItem::Point(point) => Some(*point),
                    _ => None,
                })
                && validate_contact(
                    document,
                    *contact,
                    span(curve_index).expect("point-on-curve span"),
                    ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    fixture.horizontal_contact_parameter,
                    bounded_neighborhood(fixture.horizontal_contact_parameter),
                    None,
                )
        }
        (
            ResolvedConstraintKind::CurveContact,
            DocumentConstraintDefinition::CurveCurveContact {
                first_contact,
                second_contact,
            },
        ) => validate_curve_pair_contacts(
            document,
            [*first_contact, *second_contact],
            operands,
            None,
            false,
        ),
        (
            ResolvedConstraintKind::HorizontalLine,
            DocumentConstraintDefinition::Horizontal { line },
        ) => Some(*line) == span(0),
        (ResolvedConstraintKind::VerticalLine, DocumentConstraintDefinition::Vertical { line }) => {
            Some(*line) == span(0)
        }
        (
            ResolvedConstraintKind::ParallelLines,
            DocumentConstraintDefinition::Parallel { first, second },
        )
        | (
            ResolvedConstraintKind::PerpendicularLines,
            DocumentConstraintDefinition::Perpendicular { first, second },
        )
        | (
            ResolvedConstraintKind::EqualLength,
            DocumentConstraintDefinition::EqualLength { first, second },
        ) => Some(*first) == span(0) && Some(*second) == span(1),
        (
            ResolvedConstraintKind::RadialLine,
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        ) => {
            *point == fixture.points[4]
                && validate_contact(
                    document,
                    *contact,
                    fixture.radial_line,
                    ContactDomain::SupportingLine,
                    fixture.radial_parameter,
                    ContactNeighborhood::Interior,
                    None,
                )
        }
        (
            ResolvedConstraintKind::EqualRadius,
            DocumentConstraintDefinition::EqualRadius { first, second },
        ) => {
            span(0).map(|value| value.curve) == Some(*first)
                && span(1).map(|value| value.curve) == Some(*second)
        }
        (
            ResolvedConstraintKind::EqualCurvature,
            DocumentConstraintDefinition::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            },
        ) => {
            *relation == curvature_option(variant)
                && validate_curve_pair_contacts(
                    document,
                    [*first_contact, *second_contact],
                    operands,
                    None,
                    false,
                )
        }
        (
            ResolvedConstraintKind::Midpoint,
            DocumentConstraintDefinition::Midpoint {
                point: stored,
                line,
            },
        ) => {
            Some(*stored)
                == items.iter().find_map(|item| match item {
                    SelectionItem::Point(point) => Some(*point),
                    _ => None,
                })
                && Some(*line)
                    == items.iter().find_map(|item| match item {
                        SelectionItem::Curve(span) => Some(*span),
                        _ => None,
                    })
        }
        (
            ResolvedConstraintKind::SymmetricAboutLine,
            DocumentConstraintDefinition::SymmetricAboutLine {
                first,
                second,
                line,
            },
        ) => Some(*first) == point(0) && Some(*second) == point(1) && Some(*line) == span(2),
        (
            ResolvedConstraintKind::CurveTangency,
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            },
        ) => validate_curve_pair_contacts(
            document,
            [*first_contact, *second_contact],
            operands,
            Some(tangent_option(variant)),
            false,
        ),
        (
            ResolvedConstraintKind::EndpointContinuity,
            DocumentConstraintDefinition::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            },
        ) => {
            *continuity == continuity_option(variant)
                && validate_curve_pair_contacts(
                    document,
                    [*first_contact, *second_contact],
                    operands,
                    None,
                    true,
                )
        }
        _ => false,
    };
    if !exact {
        return Err(defect(
            "persistence.definition",
            format!("{kind:?} persisted unexpected definition {definition:?}"),
        ));
    }
    Ok(())
}

fn validate_curve_pair_contacts(
    document: &SketchDocument,
    contacts: [ContactId; 2],
    operands: &[AuthoringOperand],
    tangent_orientation: Option<TangentOrientation>,
    endpoints: bool,
) -> bool {
    for (contact, operand) in contacts.into_iter().zip(operands) {
        let SelectionItem::Curve(span) = operand.item else {
            return false;
        };
        let parameter = operand.curve_parameter.unwrap_or(0.5);
        let neighborhood = if endpoints {
            if parameter.to_bits() == 0.0_f64.to_bits() {
                ContactNeighborhood::Start
            } else {
                ContactNeighborhood::End
            }
        } else {
            ContactNeighborhood::Interior
        };
        if !validate_contact(
            document,
            contact,
            span,
            ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            parameter,
            neighborhood,
            tangent_orientation,
        ) {
            return false;
        }
    }
    true
}

fn validate_contact(
    document: &SketchDocument,
    contact: ContactId,
    span: CurveSpan,
    domain: ContactDomain,
    parameter: f64,
    neighborhood: ContactNeighborhood,
    tangent_orientation: Option<TangentOrientation>,
) -> bool {
    let Some(contact) = document.contact(contact) else {
        return false;
    };
    let Some(parameter_value) = document.scalar(contact.parameter) else {
        return false;
    };
    contact.curve == span
        && contact.domain == domain
        && contact.winding == 0
        && contact.neighborhood == neighborhood
        && contact.tangent_orientation == tangent_orientation
        && parameter_value.value.to_bits() == parameter.to_bits()
        && parameter_value.unit == ScalarUnit::Parameter
        && parameter_value.domain == scalar_domain_for_contact(domain)
}

const fn scalar_domain_for_contact(domain: ContactDomain) -> ScalarDomain {
    match domain {
        ContactDomain::Bounded { lower, upper } => ScalarDomain::Bounded { lower, upper },
        ContactDomain::Periodic { period } => ScalarDomain::Periodic { period },
        ContactDomain::SupportingLine => ScalarDomain::Finite,
    }
}

fn bounded_neighborhood(parameter: f64) -> ContactNeighborhood {
    if parameter.to_bits() == 0.0_f64.to_bits() {
        ContactNeighborhood::Start
    } else if parameter.to_bits() == 1.0_f64.to_bits() {
        ContactNeighborhood::End
    } else {
        ContactNeighborhood::Interior
    }
}

fn validate_dimension_definition(
    kind: DimensionKind,
    operands: &[AuthoringOperand],
    dimension: geosolve_sketch::DocumentDimensionId,
    document: &SketchDocument,
) -> OracleResult {
    let stored = document
        .dimension(dimension)
        .ok_or_else(|| defect("persistence.missing", "created dimension ID is absent"))?;
    if stored.mode != DocumentDimensionMode::Driving
        || stored.suppressed
        || stored.label != dimension_label(kind)
    {
        return Err(defect(
            "persistence.dimension",
            format!(
                "created dimension did not persist with exact active Driving metadata: {stored:?}"
            ),
        ));
    }
    let target = dimension_target_scalar(&stored.definition);
    let target = document
        .scalar(target)
        .ok_or_else(|| defect("persistence.dimension", "target scalar is absent"))?;
    let expected_unit = if kind == DimensionKind::OrientedAngle {
        ScalarUnit::Angle
    } else {
        ScalarUnit::Length
    };
    if target.unit != expected_unit
        || target.domain != ScalarDomain::Positive
        || !target.value.is_finite()
        || target.value <= 0.0
    {
        return Err(defect(
            "persistence.dimension",
            format!("invalid target scalar metadata {target:?}"),
        ));
    }
    let items = operands.iter().map(|value| value.item).collect::<Vec<_>>();
    let exact = match (&stored.definition, kind) {
        (
            DocumentDimensionDefinition::PointDistance { first, second, .. },
            DimensionKind::PointDistance,
        ) => items == [SelectionItem::Point(*first), SelectionItem::Point(*second)],
        (DocumentDimensionDefinition::CurveLength { curve, .. }, DimensionKind::SegmentLength) => {
            items == [SelectionItem::Curve(*curve)]
        }
        (DocumentDimensionDefinition::Radius { curve, .. }, DimensionKind::Radius)
        | (DocumentDimensionDefinition::Diameter { curve, .. }, DimensionKind::Diameter) => {
            items == [SelectionItem::Curve(CurveSpan::line(*curve))]
        }
        (
            DocumentDimensionDefinition::OrientedAngle {
                first,
                second,
                orientation,
                ..
            },
            DimensionKind::OrientedAngle,
        ) => {
            *orientation == DocumentAngleOrientation::CounterClockwise
                && items == [SelectionItem::Curve(*first), SelectionItem::Curve(*second)]
        }
        _ => false,
    };
    if !exact {
        return Err(defect(
            "persistence.dimension",
            format!(
                "{kind:?} persisted unexpected definition {:?}",
                stored.definition
            ),
        ));
    }
    Ok(())
}

const fn dimension_target_scalar(definition: &DocumentDimensionDefinition) -> DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. } => *target,
    }
}

fn validate_dimension_geometry(
    kind: DimensionKind,
    dimension: geosolve_sketch::DocumentDimensionId,
    document: &SketchDocument,
    expected_target: f64,
) -> OracleResult {
    let stored = document
        .dimension(dimension)
        .ok_or_else(|| defect("geometry.missing", "accepted dimension disappeared"))?;
    let persisted_target = document
        .scalar(dimension_target_scalar(&stored.definition))
        .ok_or_else(|| defect("geometry.missing", "accepted dimension target disappeared"))?
        .value;
    if persisted_target.to_bits() != expected_target.to_bits() {
        return Err(defect(
            "geometry.target-authority",
            format!(
                "accepted target {persisted_target} differs from persisted editor target {expected_target}"
            ),
        ));
    }
    let measurement = match &stored.definition {
        DocumentDimensionDefinition::PointDistance { first, second, .. } => distance(
            document
                .point(*first)
                .ok_or_else(|| defect("geometry.missing", "distance first point disappeared"))?
                .position,
            document
                .point(*second)
                .ok_or_else(|| defect("geometry.missing", "distance second point disappeared"))?
                .position,
        ),
        DocumentDimensionDefinition::CurveLength { curve, .. } => line_length(document, *curve)?,
        DocumentDimensionDefinition::Radius { curve, .. } => circle_radius(document, *curve)?,
        DocumentDimensionDefinition::Diameter { curve, .. } => {
            2.0 * circle_radius(document, *curve)?
        }
        DocumentDimensionDefinition::OrientedAngle {
            first,
            second,
            orientation,
            ..
        } => {
            let signed = oriented_line_angle(document, *first, *second, *orientation)?;
            signed
                + ((expected_target - signed) / std::f64::consts::TAU).round()
                    * std::f64::consts::TAU
        }
        DocumentDimensionDefinition::SupportingLineOffset { .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { .. } => {
            return Err(defect(
                "geometry.oracle",
                format!("unexpected non-authoring dimension definition for {kind:?}"),
            ));
        }
    };
    let tolerance = dimension_tolerance(kind, document);
    if !measurement.is_finite() || (measurement - expected_target).abs() > tolerance {
        return Err(defect(
            "geometry.dimension-postcondition",
            format!(
                "{kind:?} accepted measurement {measurement} for target {expected_target} (tolerance {tolerance})"
            ),
        ));
    }
    Ok(())
}

fn dimension_tolerance(kind: DimensionKind, document: &SketchDocument) -> f64 {
    if kind == DimensionKind::OrientedAngle {
        2.0e-8
    } else {
        2.0e-8 * document.model_scale()
    }
}

fn oriented_line_angle(
    document: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
    orientation: DocumentAngleOrientation,
) -> OracleResult<f64> {
    let [first_start, first_end] = line_points(document, first)?;
    let [second_start, second_end] = line_points(document, second)?;
    let first = [first_end[0] - first_start[0], first_end[1] - first_start[1]];
    let second = [
        second_end[0] - second_start[0],
        second_end[1] - second_start[1],
    ];
    Ok(match orientation {
        DocumentAngleOrientation::CounterClockwise => {
            cross(first, second).atan2(dot(first, second))
        }
        DocumentAngleOrientation::Clockwise => (-cross(first, second)).atan2(dot(first, second)),
    })
}

fn validate_finite_geometry(document: &SketchDocument) -> OracleResult {
    if document
        .points()
        .iter()
        .any(|point| !point.position.into_iter().all(f64::is_finite))
        || document
            .scalars()
            .iter()
            .any(|scalar| !scalar.value.is_finite())
    {
        return Err(defect(
            "geometry.non-finite",
            "accepted document contains NaN or infinity",
        ));
    }
    for curve in document.curves() {
        let spans = document
            .curve_spans(curve.id)
            .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
        for span in spans {
            for parameter in [0.0, 0.5, 1.0] {
                let jet = document
                    .evaluate_curve_jet(span, parameter)
                    .map_err(|error| {
                        defect(
                            "geometry.evaluation",
                            format!("curve {span:?} at {parameter}: {error}"),
                        )
                    })?;
                if !jet
                    .position
                    .iter()
                    .chain(jet.first_derivative.iter())
                    .chain(jet.second_derivative.iter())
                    .all(|value| value.is_finite())
                {
                    return Err(defect(
                        "geometry.non-finite",
                        format!("curve {span:?} produced a non-finite jet"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_no_move_witness(fixture: &MatrixFixture, accepted: &SketchDocument) -> OracleResult {
    if !fixture.pre_satisfied {
        return Ok(());
    }
    let tolerance = 2.0e-8 * fixture.transform.scale.max(1.0);
    for (point, expected) in &fixture.original_points {
        let actual = accepted
            .point(*point)
            .ok_or_else(|| defect("geometry.missing", format!("point {point} disappeared")))?
            .position;
        if distance(actual, *expected) > tolerance {
            return Err(defect(
                "geometry.unintended-movement",
                format!(
                    "pre-satisfied authoring moved point {point} from {expected:?} to {actual:?}"
                ),
            ));
        }
    }
    for (scalar, expected) in &fixture.original_scalars {
        let actual = accepted
            .scalar(*scalar)
            .ok_or_else(|| defect("geometry.missing", format!("scalar {scalar} disappeared")))?
            .value;
        if (actual - expected).abs() > tolerance {
            return Err(defect(
                "geometry.unintended-movement",
                format!(
                    "pre-satisfied authoring moved scalar {scalar} from {expected} to {actual}"
                ),
            ));
        }
    }
    Ok(())
}

fn validate_protected_geometry(
    fixture: &MatrixFixture,
    document: &SketchDocument,
    scale: f64,
) -> OracleResult {
    for radius in fixture.circle_radii {
        let value = document
            .scalar(radius)
            .ok_or_else(|| defect("geometry.missing", "protected radius disappeared"))?
            .value;
        if !value.is_finite() || value < 0.5 * scale {
            return Err(defect(
                "geometry.collapse",
                format!("protected radius collapsed to {value}"),
            ));
        }
    }
    for span in [fixture.lines[0], fixture.lines[1], fixture.overlapping_line] {
        let [start, end] = line_points(document, span)?;
        if distance(start, end) < scale {
            return Err(defect(
                "geometry.collapse",
                format!("protected line {span:?} collapsed"),
            ));
        }
    }
    Ok(())
}

fn validate_constraint_geometry(
    kind: ResolvedConstraintKind,
    document: &SketchDocument,
    constraint: geosolve_sketch::DocumentConstraintId,
    fixture: &MatrixFixture,
) -> OracleResult {
    let definition = &document
        .constraint(constraint)
        .ok_or_else(|| defect("geometry.missing", "accepted constraint disappeared"))?
        .definition;
    let tolerance = 2.0e-8 * fixture.transform.scale.max(1.0);
    let valid = match definition {
        DocumentConstraintDefinition::FixedPoint { point, target } => {
            distance(
                document.point(*point).expect("fixed point").position,
                *target,
            ) <= tolerance
        }
        DocumentConstraintDefinition::Coincident { first, second } => {
            distance(
                document.point(*first).expect("first point").position,
                document.point(*second).expect("second point").position,
            ) <= tolerance
        }
        DocumentConstraintDefinition::Horizontal { line } => {
            let [a, b] = line_points(document, *line)?;
            (a[1] - b[1]).abs() <= tolerance
        }
        DocumentConstraintDefinition::Vertical { line } => {
            let [a, b] = line_points(document, *line)?;
            (a[0] - b[0]).abs() <= tolerance
        }
        DocumentConstraintDefinition::Parallel { first, second } => {
            normalized_dot_or_cross(document, *first, *second, true)?.abs() <= 2.0e-8
        }
        DocumentConstraintDefinition::Perpendicular { first, second } => {
            normalized_dot_or_cross(document, *first, *second, false)?.abs() <= 2.0e-8
        }
        DocumentConstraintDefinition::EqualLength { first, second } => {
            let first = line_length(document, *first)?;
            let second = line_length(document, *second)?;
            (first - second).abs() <= tolerance
        }
        DocumentConstraintDefinition::EqualRadius { first, second } => {
            let first = circle_radius(document, *first)?;
            let second = circle_radius(document, *second)?;
            (first - second).abs() <= tolerance
        }
        DocumentConstraintDefinition::Midpoint { point, line } => {
            let [a, b] = line_points(document, *line)?;
            let midpoint = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            distance(
                document.point(*point).expect("midpoint point").position,
                midpoint,
            ) <= tolerance
        }
        DocumentConstraintDefinition::SymmetricAboutLine {
            first,
            second,
            line,
        } => {
            let first = document.point(*first).expect("symmetric first").position;
            let second = document.point(*second).expect("symmetric second").position;
            let midpoint = [(first[0] + second[0]) * 0.5, (first[1] + second[1]) * 0.5];
            let [a, b] = line_points(document, *line)?;
            let direction = [b[0] - a[0], b[1] - a[1]];
            let offset = [midpoint[0] - a[0], midpoint[1] - a[1]];
            let across = [second[0] - first[0], second[1] - first[1]];
            cross(direction, offset).abs() <= tolerance * vector_length(direction)
                && dot(direction, across).abs() <= tolerance * vector_length(direction)
        }
        DocumentConstraintDefinition::PointOnCurve { point, contact } => {
            let contact_position = document
                .evaluate_contact_jet(*contact)
                .map_err(|error| defect("geometry.evaluation", error.to_string()))?
                .position;
            distance(
                document
                    .point(*point)
                    .expect("point-on-curve point")
                    .position,
                [contact_position[0], contact_position[1]],
            ) <= tolerance
        }
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        } => contact_distance(document, *first_contact, *second_contact)? <= tolerance,
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        } => {
            let orientation = document
                .contact(*first_contact)
                .and_then(|contact| contact.tangent_orientation)
                .ok_or_else(|| {
                    defect(
                        "persistence.contact",
                        "tangency lost its explicit orientation",
                    )
                })?;
            let tangent_dot = contact_tangent_dot(document, *first_contact, *second_contact)?;
            contact_distance(document, *first_contact, *second_contact)? <= tolerance
                && contact_tangent_cross(document, *first_contact, *second_contact)?.abs() <= 2.0e-8
                && match orientation {
                    TangentOrientation::Aligned => tangent_dot > 0.0,
                    TangentOrientation::Opposed => tangent_dot < 0.0,
                }
        }
        DocumentConstraintDefinition::EqualCurvature {
            first_contact,
            second_contact,
            relation,
        } => {
            let first = document
                .measure_curve_contact(
                    *first_contact,
                    geosolve_sketch::DocumentCurveMeasurementKind::SignedCurvature,
                )
                .map_err(|error| defect("geometry.measurement", error.to_string()))?;
            let second = document
                .measure_curve_contact(
                    *second_contact,
                    geosolve_sketch::DocumentCurveMeasurementKind::SignedCurvature,
                )
                .map_err(|error| defect("geometry.measurement", error.to_string()))?;
            match relation {
                DocumentCurveCurvatureRelation::Signed => (first - second).abs() <= tolerance,
                DocumentCurveCurvatureRelation::MagnitudeSameSign => {
                    (first.abs() - second.abs()).abs() <= tolerance
                        && first.signum() == second.signum()
                }
                DocumentCurveCurvatureRelation::MagnitudeOppositeSign => {
                    (first.abs() - second.abs()).abs() <= tolerance
                        && first.signum() != second.signum()
                }
            }
        }
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            continuity,
        } => {
            let first = document
                .evaluate_contact_jet(*first_contact)
                .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
            let second = document
                .evaluate_contact_jet(*second_contact)
                .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
            let first_sign = endpoint_path_sign(document, *first_contact, true)?;
            let second_sign = endpoint_path_sign(document, *second_contact, false)?;
            let position_delta = first.position - second.position;
            let position_valid = position_delta[0].abs().max(position_delta[1].abs())
                / document.model_scale()
                <= 2.0e-8;
            match continuity {
                DocumentCurveContinuity::G0 => position_valid,
                DocumentCurveContinuity::G1 | DocumentCurveContinuity::G2 => {
                    let first_unit = first
                        .differential()
                        .map_err(|error| defect("geometry.measurement", error.to_string()))?
                        .unit_tangent
                        * first_sign;
                    let second_unit = second
                        .differential()
                        .map_err(|error| defect("geometry.measurement", error.to_string()))?
                        .unit_tangent
                        * second_sign;
                    let tangent_valid =
                        (first_unit[0] * second_unit[1] - first_unit[1] * second_unit[0]).abs()
                            <= 2.0e-8
                            && first_unit.dot(&second_unit) > 0.0;
                    let curvature_valid = if *continuity == DocumentCurveContinuity::G2 {
                        let first_curvature = document
                            .measure_curve_contact(
                                *first_contact,
                                geosolve_sketch::DocumentCurveMeasurementKind::SignedCurvature,
                            )
                            .map_err(|error| defect("geometry.measurement", error.to_string()))?;
                        let second_curvature = document
                            .measure_curve_contact(
                                *second_contact,
                                geosolve_sketch::DocumentCurveMeasurementKind::SignedCurvature,
                            )
                            .map_err(|error| defect("geometry.measurement", error.to_string()))?;
                        ((first_curvature * first_sign - second_curvature * second_sign)
                            * document.model_scale())
                        .abs()
                            <= 2.0e-8
                    } else {
                        true
                    };
                    position_valid && tangent_valid && curvature_valid
                }
                DocumentCurveContinuity::ParametricC2 {
                    first_rate,
                    second_rate,
                } => {
                    let first_path = first.first_derivative * first_sign;
                    let second_path = second.first_derivative * second_sign;
                    let first_velocity = first_path * *first_rate;
                    let second_velocity = second_path * *second_rate;
                    let first_acceleration = first.second_derivative * first_rate.powi(2);
                    let second_acceleration = second.second_derivative * second_rate.powi(2);
                    let velocity_delta = first_velocity - second_velocity;
                    let acceleration_delta = first_acceleration - second_acceleration;
                    position_valid
                        && first_rate.is_finite()
                        && *first_rate > 0.0
                        && second_rate.is_finite()
                        && *second_rate > 0.0
                        && velocity_delta[0].abs().max(velocity_delta[1].abs())
                            / document.model_scale()
                            <= 2.0e-8
                        && acceleration_delta[0].abs().max(acceleration_delta[1].abs())
                            / document.model_scale()
                            <= 2.0e-8
                }
            }
        }
        other => {
            return Err(defect(
                "geometry.oracle",
                format!("no golden semantic oracle for {kind:?}: {other:?}"),
            ));
        }
    };
    if !valid {
        return Err(defect(
            "geometry.postcondition",
            format!("{kind:?} accepted without satisfying its public geometric postcondition"),
        ));
    }
    Ok(())
}

fn endpoint_path_sign(
    document: &SketchDocument,
    contact: ContactId,
    incoming: bool,
) -> OracleResult<f64> {
    let neighborhood = document
        .contact(contact)
        .ok_or_else(|| defect("geometry.missing", "continuity contact disappeared"))?
        .neighborhood;
    match (incoming, neighborhood) {
        (true, ContactNeighborhood::Start) | (false, ContactNeighborhood::End) => Ok(-1.0),
        (true, ContactNeighborhood::End) | (false, ContactNeighborhood::Start) => Ok(1.0),
        (_, ContactNeighborhood::Interior | ContactNeighborhood::Local { .. }) => Err(defect(
            "persistence.contact",
            "endpoint continuity persisted an interior neighborhood",
        )),
    }
}

fn line_points(document: &SketchDocument, span: CurveSpan) -> OracleResult<[[f64; 2]; 2]> {
    let curve = document
        .curve(span.curve)
        .ok_or_else(|| defect("geometry.missing", format!("line {span:?} disappeared")))?;
    let CurveDefinition::Line { start, end, .. } = curve.definition else {
        return Err(defect(
            "geometry.fixture",
            format!("expected line span, got {:?}", curve.definition),
        ));
    };
    Ok([
        document.point(start).expect("line start").position,
        document.point(end).expect("line end").position,
    ])
}

fn line_length(document: &SketchDocument, span: CurveSpan) -> OracleResult<f64> {
    let [a, b] = line_points(document, span)?;
    Ok(distance(a, b))
}

fn circle_radius(document: &SketchDocument, curve: geosolve_sketch::CurveId) -> OracleResult<f64> {
    let definition = &document
        .curve(curve)
        .ok_or_else(|| defect("geometry.missing", format!("circle {curve} disappeared")))?
        .definition;
    let CurveDefinition::Circle { radius, .. } = definition else {
        return Err(defect("geometry.fixture", "expected a circle"));
    };
    Ok(document.scalar(*radius).expect("circle radius").value)
}

fn normalized_dot_or_cross(
    document: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
    use_cross: bool,
) -> OracleResult<f64> {
    let [a, b] = line_points(document, first)?;
    let [c, d] = line_points(document, second)?;
    let first = [b[0] - a[0], b[1] - a[1]];
    let second = [d[0] - c[0], d[1] - c[1]];
    let denominator = vector_length(first) * vector_length(second);
    Ok(if use_cross {
        cross(first, second) / denominator
    } else {
        dot(first, second) / denominator
    })
}

fn contact_distance(
    document: &SketchDocument,
    first: ContactId,
    second: ContactId,
) -> OracleResult<f64> {
    let first = document
        .evaluate_contact_jet(first)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    let second = document
        .evaluate_contact_jet(second)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    Ok((first.position - second.position).norm())
}

fn contact_tangent_cross(
    document: &SketchDocument,
    first: ContactId,
    second: ContactId,
) -> OracleResult<f64> {
    let first = document
        .evaluate_contact_jet(first)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    let second = document
        .evaluate_contact_jet(second)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    let denominator = first.first_derivative.norm() * second.first_derivative.norm();
    Ok((first.first_derivative[0] * second.first_derivative[1]
        - first.first_derivative[1] * second.first_derivative[0])
        / denominator)
}

fn contact_tangent_dot(
    document: &SketchDocument,
    first: ContactId,
    second: ContactId,
) -> OracleResult<f64> {
    let first = document
        .evaluate_contact_jet(first)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    let second = document
        .evaluate_contact_jet(second)
        .map_err(|error| defect("geometry.evaluation", error.to_string()))?;
    let denominator = first.first_derivative.norm() * second.first_derivative.norm();
    Ok(first.first_derivative.dot(&second.first_derivative) / denominator)
}

fn distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - second[0]).hypot(first[1] - second[1])
}

fn vector_length(value: [f64; 2]) -> f64 {
    value[0].hypot(value[1])
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[0], first[1] * second[1])
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[1], -first[1] * second[0])
}
