//! Shared model types for the geosolve fuzzing corpus.
//!
//! `FuzzInput` is the 51-byte fuzzing record (see FUZZING.md input codec). It
//! is deliberately NOT `geosolve_survey::FuzzVariant::encode()`: the flag u8s
//! sit after `contact_parameter` and `option_index` is a u32, whereas the
//! variant codec places the flags after `rotation` and packs `option_index` as
//! a u8. Decoding is total — short inputs are left-padded with zeros.

use geosolve_survey::{Family, FuzzVariant};

/// Total length of a FuzzInput record in bytes.
pub const LEN: usize = 51;

/// Upper bound on a normalized translation component or scale factor. Kept
/// finite and well within the range the survey fixture can represent without
/// overflow: the fixture seeds `model_scale = 10.0 * scale`, so any scale at or
/// above `f64::MAX / 10` would overflow to Inf and fail `SketchDocument`
/// construction. This bound keeps every derived quantity finite and the
/// residual resolution meaningful.
pub const GEOMETRY_MAGNITUDE_CAP: f64 = 1e6;

/// Lower bound on a normalized scale factor. A scale below this is drowned by
/// the (up to `GEOMETRY_MAGNITUDE_CAP`) translation: the fixture's transform is
/// `translation + scale * R * p`, so a segment of length `scale * k` sitting on
/// an offset of ~1e6 rounds back to that offset and collapses to a zero-length
/// segment in f64 (rejected as degenerate). With the translation capped at
/// `GEOMETRY_MAGNITUDE_CAP`, this bound keeps the smallest scaled feature
/// (`~scale * 4`) comfortably above the f64 resolution (~2e-10 at 1e6).
pub const MIN_SCALE: f64 = 1e-3;

/// A 51-byte FuzzInput record.
///
/// Layout (FUZZING.md input codec):
///   family: u32 LE
///   translation[0..2], scale, rotation, contact_parameter: 5 x f64 bits LE
///   reverse_spans, swap_operands, displaced: u8 (flag bitfield, masked to 3 bits)
///   option_index: u32 LE
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuzzInput {
    pub family: u32,
    pub translation: [f64; 2],
    pub scale: f64,
    pub rotation: f64,
    pub contact_parameter: f64,
    pub reverse_spans: bool,
    pub swap_operands: bool,
    pub displaced: bool,
    pub option_index: u32,
}

impl FuzzInput {
    /// Decode a record from the trailing LEN bytes of `data`, left-padding
    /// short inputs with zeros. Decoding is total.
    pub fn decode(data: &[u8]) -> Self {
        let start = if data.len() >= LEN {
            data.len() - LEN
        } else {
            0
        };
        let buf = &data[start..];

        let mut b = [0u8; LEN];
        let fill = buf.len().min(LEN);
        b[LEN - fill..].copy_from_slice(&buf[..fill]);

        let family = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let translation = [
            f64::from_le_bytes(b[4..12].try_into().unwrap()),
            f64::from_le_bytes(b[12..20].try_into().unwrap()),
        ];
        let scale = f64::from_le_bytes(b[20..28].try_into().unwrap());
        let rotation = f64::from_le_bytes(b[28..36].try_into().unwrap());
        let contact_parameter = f64::from_le_bytes(b[36..44].try_into().unwrap());
        let reverse_spans = b[44] & 1 != 0;
        let swap_operands = b[45] & 2 != 0;
        let displaced = b[46] & 4 != 0;
        let option_index = u32::from_le_bytes([b[47], b[48], b[49], b[50]]);

        let mut input = FuzzInput {
            family,
            translation,
            scale,
            rotation,
            contact_parameter,
            reverse_spans,
            swap_operands,
            displaced,
            option_index,
        };
        input.normalize();
        input
    }

    /// Encode a record in the FuzzInput-specific layout (flags after
    /// contact_parameter, option_index as u32 LE).
    pub fn encode(family_index: u32, variant: &FuzzVariant) -> [u8; LEN] {
        let mut b = [0u8; LEN];
        b[0..4].copy_from_slice(&family_index.to_le_bytes());
        b[4..12].copy_from_slice(&variant.translation[0].to_le_bytes());
        b[12..20].copy_from_slice(&variant.translation[1].to_le_bytes());
        b[20..28].copy_from_slice(&variant.scale.to_le_bytes());
        b[28..36].copy_from_slice(&variant.rotation.to_le_bytes());
        b[36..44].copy_from_slice(&variant.contact_parameter.to_le_bytes());
        b[44] = (u8::from(variant.reverse_spans)
            | u8::from(variant.swap_operands) << 1
            | u8::from(variant.displaced) << 2)
            & 0x07;
        b[47..51].copy_from_slice(&(variant.option_index as u32).to_le_bytes());
        b
    }

    /// Clamp/normalize to well-formed geometry (FUZZING.md): non-finite
    /// translation -> 0.0, scale -> [MIN_SCALE, GEOMETRY_MAGNITUDE_CAP],
    /// rotation -> mod 2π, contact_parameter -> [0,1], option_index -> [0,24).
    ///
    /// Magnitudes are additionally capped at GEOMETRY_MAGNITUDE_CAP so that the
    /// fixture's `model_scale = 10.0 * scale` and its point offsets stay finite
    /// and solvable (an uncapped scale overflows `10 * scale` to Inf; an
    /// uncapped translation places points near f64::MAX, which the solver
    /// cannot resolve).
    fn normalize(&mut self) {
        for t in &mut self.translation {
            if !t.is_finite() {
                *t = 0.0;
            } else {
                *t = (*t).clamp(-GEOMETRY_MAGNITUDE_CAP, GEOMETRY_MAGNITUDE_CAP);
            }
        }
        // Keep the scale finite, positive, and within a non-degenerate
        // magnitude range. Below MIN_SCALE it is drowned by the translation and
        // collapses the fixture to zero-length segments; above the cap it
        // overflows `model_scale` and point offsets.
        self.scale = if self.scale.is_finite() {
            self.scale.abs().clamp(MIN_SCALE, GEOMETRY_MAGNITUDE_CAP)
        } else {
            MIN_SCALE
        };
        // `rem_euclid` propagates NaN (NaN.rem_euclid(x) == NaN), so an
        // unguarded rotation would leak a NaN into cos()/sin() and produce a
        // non-finite point position. Guard it like the other fields.
        if self.rotation.is_finite() {
            self.rotation = self.rotation.rem_euclid(std::f64::consts::TAU);
        } else {
            self.rotation = 0.0;
        }
        if !self.contact_parameter.is_finite() {
            self.contact_parameter = 0.5;
        } else {
            self.contact_parameter = self.contact_parameter.clamp(0.0, 1.0);
        }
        self.option_index %= 24;
    }

    /// The resolved family for this record (clamped into range).
    pub fn family(&self) -> Family {
        geosolve_survey::FAMILIES
            .get(self.family as usize)
            .copied()
            .unwrap_or(geosolve_survey::FAMILIES[0])
    }

    /// The variant implied by this record: the full perturbation, carrying the
    /// input's geometry (translation/scale/rotation/contact_parameter) plus its
    /// flags and option_index.
    pub fn into_variant(&self) -> FuzzVariant {
        FuzzVariant {
            translation: self.translation,
            scale: self.scale,
            rotation: self.rotation,
            contact_parameter: self.contact_parameter,
            reverse_spans: self.reverse_spans,
            swap_operands: self.swap_operands,
            displaced: self.displaced,
            option_index: self.option_index as u8,
        }
    }

    /// A copy of this input with a single flag flipped — the deterministic
    /// perturbation target 02 applies to sweep flag neighbours of a decoded
    /// input. Family and geometry are preserved.
    pub fn with_displaced(&self, displaced: bool) -> Self {
        let mut copy = *self;
        copy.displaced = displaced;
        copy
    }

    /// Deterministic golden seeds for a family: the DETERMINISTIC variant plus
    /// every flag pattern crossed with each of the 24 option indices.
    pub fn golden_seeds(family_index: u32) -> Vec<[u8; LEN]> {
        let mut out = Vec::new();
        out.push(FuzzInput::encode(family_index, &FuzzVariant::DETERMINISTIC));
        for option_index in 0..24u8 {
            for flags in 0..8u8 {
                let mut variant = FuzzVariant::DETERMINISTIC;
                variant.reverse_spans = flags & 1 != 0;
                variant.swap_operands = flags & 2 != 0;
                variant.displaced = flags & 4 != 0;
                variant.option_index = option_index;
                out.push(FuzzInput::encode(family_index, &variant));
            }
        }
        out
    }
}
