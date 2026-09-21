// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared input model and residual-checked survey harness for `GeoSolve`'s core
//! authoring stack.
//!
//! This crate extracts the input model that the golden authoring oracle uses
//! (`crates/geosolve-constraint-editor/tests/golden_authoring_oracle.rs`) into a
//! reusable library so the fuzzing front-ends in `fuzz/` can drive the same
//! space of documents, authoring operands, and solver diagnostics without
//! duplicating the model. The golden test file remains the source of truth for
//! the exact 271-row fingerprint inventory.
//!
//! The model is a finite, fully-characterized input:
//!
//! ```text
//! Variant = translation[2], scale, rotation, contact_parameter,
//!           reverse_spans, swap_operands, displaced, option_index
//! Family  = 24 constraint kinds x {Lock, Coincident, Horizontal, Vertical,
//!           Parallel, Perpendicular, Equal, Midpoint, Symmetric, Tangent,
//!           Continuity, Concentric, Collinear} + 5 dimensions
//! ```
//!
//! A survey builds the fixture, runs the retained editor coordinator + solver,
//! and hands the diagnostics to [`survey::SurveyOutcome::check`], which only
//! trusts a success-like status after independent residual validation.
#![allow(
    clippy::too_many_lines,
    reason = "the fixture builder and authoring path are intentionally one reviewable unit"
)]

pub mod fixture;
pub mod survey;

use geosolve_constraint_editor::{ConstraintIntent, DimensionKind, ResolvedConstraintKind};

/// A minimal, fully-characterized variant of the golden oracle input.
///
/// The fields mirror the golden `Variant` exactly so that this crate can
/// reproduce the golden fingerprints byte-for-byte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuzzVariant {
    pub translation: [f64; 2],
    pub scale: f64,
    pub rotation: f64,
    pub reverse_spans: bool,
    pub swap_operands: bool,
    pub contact_parameter: f64,
    pub displaced: bool,
    pub option_index: u8,
}

/// The deterministic (identity) variant used by the golden reference rows.
impl FuzzVariant {
    pub const DETERMINISTIC: Self = Self {
        translation: [0.0, 0.0],
        scale: 1.0,
        rotation: 0.0,
        reverse_spans: false,
        swap_operands: false,
        contact_parameter: 0.5,
        displaced: false,
        option_index: 0,
    };

    /// The canonical little-endian byte footprint of a variant.
    pub const LEN: usize = 44;

    /// Encode the variant into its canonical little-endian byte layout.
    pub fn encode(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        let mut i = 0;
        for value in self.translation {
            out[i..i + 8].copy_from_slice(&value.to_le_bytes());
            i += 8;
        }
        out[i..i + 8].copy_from_slice(&self.scale.to_le_bytes());
        i += 8;
        out[i..i + 8].copy_from_slice(&self.rotation.to_le_bytes());
        i += 8;
        out[i] = u8::from(self.reverse_spans);
        i += 1;
        out[i] = u8::from(self.swap_operands);
        i += 1;
        out[i..i + 8].copy_from_slice(&self.contact_parameter.to_le_bytes());
        i += 8;
        out[i] = u8::from(self.displaced);
        i += 1;
        out[i] = self.option_index;
        out
    }

    /// Decode a variant from the trailing bytes of a fuzz input.
    ///
    /// Any bytes missing from a short input are treated as zeros, so the decode
    /// is total and the last 44 bytes always win.
    pub fn decode(bytes: &[u8]) -> Self {
        let mut buf = [0u8; Self::LEN];
        let n = bytes.len().min(Self::LEN);
        buf[Self::LEN - n..].copy_from_slice(&bytes[bytes.len() - n..]);
        let read_f64 = |i: usize| -> f64 {
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(&buf[i..i + 8]);
            f64::from_le_bytes(chunk)
        };
        let translation = [read_f64(0), read_f64(8)];
        let scale = read_f64(16);
        let rotation = read_f64(24);
        let reverse_spans = buf[32] != 0;
        let swap_operands = buf[33] != 0;
        let contact_parameter = read_f64(34);
        let displaced = buf[42] != 0;
        let option_index = buf[43];
        Self {
            translation,
            scale,
            rotation,
            reverse_spans,
            swap_operands,
            contact_parameter,
            displaced,
            option_index,
        }
    }

    /// A representative input fingerprint for the golden inventory.
    pub fn fingerprint(&self, family: &str) -> String {
        let mut bytes = Vec::with_capacity(family.len() + 8 * 6 + 4);
        bytes.extend_from_slice(family.as_bytes());
        bytes.push(0);
        for value in [
            self.translation[0],
            self.translation[1],
            self.scale,
            self.rotation,
            self.contact_parameter,
        ] {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&[
            u8::from(self.reverse_spans),
            u8::from(self.swap_operands),
            u8::from(self.displaced),
            self.option_index,
        ]);
        format!("input-{:016x}", fnv1a64(&bytes))
    }
}

/// The family of an authoring subject: a constraint kind with an intent, or a
/// dimension kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    Constraint {
        kind: ResolvedConstraintKind,
        intent: ConstraintIntent,
    },
    Dimension(DimensionKind),
}

impl Family {
    /// The stable identifier string used by the golden inventory.
    pub fn id(&self) -> &'static str {
        match self {
            Family::Constraint { kind, .. } => constraint_family_id(*kind),
            Family::Dimension(kind) => dimension_family_id(*kind),
        }
    }
}

/// The golden authoring inventory: every family the survey can perturb.
///
/// This is the public surface the fuzzing front-ends iterate. It mirrors the
/// golden oracle's fixed set of 29 families so the harness covers exactly the
/// constraints and dimensions the oracle validates.
pub const FAMILIES: [Family; 29] = [
    Family::Constraint {
        kind: ResolvedConstraintKind::FixedPoint,
        intent: ConstraintIntent::Lock,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::CoincidentPoints,
        intent: ConstraintIntent::Coincident,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::PointOnCurve,
        intent: ConstraintIntent::Coincident,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::CurveContact,
        intent: ConstraintIntent::Coincident,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::HorizontalLine,
        intent: ConstraintIntent::Horizontal,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::VerticalLine,
        intent: ConstraintIntent::Vertical,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::ParallelLines,
        intent: ConstraintIntent::Parallel,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::PerpendicularLines,
        intent: ConstraintIntent::Perpendicular,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::RadialLine,
        intent: ConstraintIntent::Perpendicular,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::EqualLength,
        intent: ConstraintIntent::Equal,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::EqualRadius,
        intent: ConstraintIntent::Equal,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::EqualCurvature,
        intent: ConstraintIntent::Equal,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::Midpoint,
        intent: ConstraintIntent::Midpoint,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::SymmetricAboutLine,
        intent: ConstraintIntent::Symmetric,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::CurveTangency,
        intent: ConstraintIntent::Tangent,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::EndpointContinuity,
        intent: ConstraintIntent::Continuity,
    },
    Family::Dimension(DimensionKind::PointDistance),
    Family::Dimension(DimensionKind::SegmentLength),
    Family::Dimension(DimensionKind::Radius),
    Family::Dimension(DimensionKind::Diameter),
    Family::Dimension(DimensionKind::OrientedAngle),
    // Append-only M71 inventory: preserve every historical family ordinal and
    // byte while adding the newly retained drafting relations.
    Family::Constraint {
        kind: ResolvedConstraintKind::HorizontalPoints,
        intent: ConstraintIntent::Horizontal,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::VerticalPoints,
        intent: ConstraintIntent::Vertical,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::ConcentricCurves,
        intent: ConstraintIntent::Concentric,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::CollinearSupports,
        intent: ConstraintIntent::Collinear,
    },
    // Append-only M74 inventory: intrinsic datums add operand families without
    // changing any historical family ordinal or input fingerprint.
    Family::Constraint {
        kind: ResolvedConstraintKind::CoincidentWithOrigin,
        intent: ConstraintIntent::Coincident,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::PointOnDatumAxis,
        intent: ConstraintIntent::Coincident,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::CollinearWithDatumAxis,
        intent: ConstraintIntent::Collinear,
    },
    Family::Constraint {
        kind: ResolvedConstraintKind::SymmetricAboutDatumAxis,
        intent: ConstraintIntent::Symmetric,
    },
];

const fn constraint_family_id(kind: ResolvedConstraintKind) -> &'static str {
    match kind {
        ResolvedConstraintKind::FixedPoint => "constraint.fixed-point",
        ResolvedConstraintKind::CoincidentWithOrigin => "constraint.coincident-with-origin",
        ResolvedConstraintKind::PointOnDatumAxis => "constraint.point-on-datum-axis",
        ResolvedConstraintKind::CoincidentPoints => "constraint.coincident-points",
        ResolvedConstraintKind::PointOnCurve => "constraint.point-on-curve",
        ResolvedConstraintKind::CurveContact => "constraint.curve-contact",
        ResolvedConstraintKind::HorizontalLine => "constraint.horizontal-line",
        ResolvedConstraintKind::VerticalLine => "constraint.vertical-line",
        ResolvedConstraintKind::HorizontalPoints => "constraint.horizontal-points",
        ResolvedConstraintKind::VerticalPoints => "constraint.vertical-points",
        ResolvedConstraintKind::ConcentricCurves => "constraint.concentric-curves",
        ResolvedConstraintKind::CollinearSupports => "constraint.collinear-supports",
        ResolvedConstraintKind::CollinearWithDatumAxis => "constraint.collinear-with-datum-axis",
        ResolvedConstraintKind::ParallelLines => "constraint.parallel-lines",
        ResolvedConstraintKind::PerpendicularLines => "constraint.perpendicular-lines",
        ResolvedConstraintKind::RadialLine => "constraint.radial-line",
        ResolvedConstraintKind::EqualLength => "constraint.equal-length",
        ResolvedConstraintKind::EqualRadius => "constraint.equal-radius",
        ResolvedConstraintKind::EqualCurvature => "constraint.equal-curvature",
        ResolvedConstraintKind::Midpoint => "constraint.midpoint",
        ResolvedConstraintKind::SymmetricAboutLine => "constraint.symmetric-about-line",
        ResolvedConstraintKind::SymmetricAboutDatumAxis => "constraint.symmetric-about-datum-axis",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_inventory_is_exhaustive_and_unique() {
        assert_eq!(FAMILIES.len(), 29);
        let mut seen: Vec<&str> = Vec::with_capacity(FAMILIES.len());
        for family in &FAMILIES {
            let id = family.id();
            assert!(
                !seen.contains(&id),
                "duplicate id {id} appears more than once in the inventory"
            );
            seen.push(id);
        }
    }

    #[test]
    fn encode_decode_roundtrip_is_exact() {
        let variant = FuzzVariant {
            translation: [1.5, -2.25],
            scale: 4.0,
            rotation: 0.7,
            reverse_spans: true,
            swap_operands: true,
            contact_parameter: 0.41,
            displaced: true,
            option_index: 5,
        };
        let encoded = variant.encode();
        assert_eq!(encoded.len(), FuzzVariant::LEN);
        assert_eq!(FuzzVariant::decode(&encoded), variant);
    }

    #[test]
    fn fingerprint_matches_golden_shape() {
        let fingerprint = FuzzVariant::DETERMINISTIC.fingerprint("constraint.fixed-point");
        assert!(
            fingerprint.starts_with("input-"),
            "unexpected {fingerprint}"
        );
    }
}
