//! Random-document generator for target 03's bare core-solver surface.
//!
//! `FUZZING.md §7` documents target 03 as "arbitrary `SketchDocument` (random
//! points/curves/scalars) via `RetainedSketchDocumentSession`". This module
//! decodes a variable-length record into an arbitrary document and builds it;
//! the harness then solves it with a bare session (no authoring coordinator)
//! and validates the diagnostics. Arbitrary geometry over the bare core-solver
//! surface is the coverage this target owns — convergence/rank/residual paths
//! that the authoring-oriented targets 01/04 do not exercise directly.
//!
//! Decoding is **total** (short inputs left-pad with zeros; magnitudes are
//! clamped finite and within the solver's resolvable range, mirroring
//! `model.rs`). Building is **defensive**: curves that cannot be satisfied
//! (not enough referenced points, no radius scalar) are skipped rather than
//! panicking, so a malformed document is a bad input, not a harness crash.

use geosolve_sketch::{
    CurveDefinition, DesignPointId, SketchDocument, ScalarDomain, ScalarUnit,
};

use crate::model::{GEOMETRY_MAGNITUDE_CAP, MIN_SCALE};

/// Upper bounds on how many entities the generator emits. Kept small so a
/// single solve stays cheap while still producing non-trivial constraint
/// graphs (curves reference random existing points, so shared ids form
/// coincidence constraints).
const MAX_POINTS: usize = 12;
const MAX_SCALARS: usize = 6;
const MAX_CURVES: usize = 12;

/// Curve kind tags accepted by the record.
const CURVE_LINE: u8 = 0;
const CURVE_CIRCLE: u8 = 1;
const CURVE_BEZIER: u8 = 2;

/// A decoded arbitrary document, ready to build.
#[derive(Clone, Debug)]
pub struct FuzzDocument {
    pub model_scale: f64,
    pub points: Vec<[f64; 2]>,
    pub scalars: Vec<f64>,
    /// Each curve is `(kind, ref0, ref1, ref2)`: raw point/scalar indices that
    /// the builder clamps into range, so every reference is always valid.
    pub curves: Vec<(u8, u8, u8, u8)>,
}

/// A fixed persistent namespace for the generated document. The exact id is
/// irrelevant — it only has to be a non-zero, stable identity so every solve
/// of one document shares a namespace.
const DOCUMENT_ID: u128 = 0x47_55_5f_42_52_4f_43;

impl FuzzDocument {
    /// Decode a record from a fuzz input. The record layout (all fields
    /// optional; missing bytes read as zero):
    ///
    /// ```text
    /// model_scale: f64
    /// point_count: u8 (& 0x0F)
    /// points:     point_count × (x: f64, y: f64)
    /// scalar_count: u8 (& 0x07)
    /// scalars:    scalar_count × f64
    /// curve_count: u8 (& 0x0F)
    /// curves:     curve_count × (kind: u8, ref0: u8, ref1: u8, ref2: u8)
    /// ```
    pub fn decode(data: &[u8]) -> Self {
        // Left-pad into a fixed buffer so reads are total; short inputs read
        // as zeros (model_scale 0.0 -> clamped, points at origin, etc.).
        let mut buf = [0u8; 512];
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        let mut cur = Cursor { buf, pos: 0 };

        let model_scale = cur.f64();
        let point_count = (cur.byte() & 0x0F) as usize;
        let mut points = Vec::with_capacity(point_count.min(MAX_POINTS));
        for _ in 0..point_count {
            points.push([cur.f64(), cur.f64()]);
        }
        let scalar_count = (cur.byte() & 0x07) as usize;
        let mut scalars = Vec::with_capacity(scalar_count.min(MAX_SCALARS));
        for _ in 0..scalar_count {
            scalars.push(cur.f64());
        }
        let curve_count = (cur.byte() & 0x0F) as usize;
        let mut curves = Vec::with_capacity(curve_count.min(MAX_CURVES));
        for _ in 0..curve_count {
            curves.push((cur.byte(), cur.byte(), cur.byte(), cur.byte()));
        }

        FuzzDocument {
            model_scale,
            points,
            scalars,
            curves,
        }
    }

    /// Build the decoded document. Returns `None` only if the document cannot
    /// seed at all (non-positive/nonfinite scale after clamping, which the
    /// clamps below prevent, or no points to reference) — a bad input, never a
    /// panic. Curves that cannot be satisfied are skipped.
    pub fn build(&self) -> Option<SketchDocument> {
        let scale = clamp_magnitude(self.model_scale, MIN_SCALE, GEOMETRY_MAGNITUDE_CAP);
        let mut doc = match SketchDocument::with_id(scale, document_id()) {
            Ok(doc) => doc,
            Err(_) => return None,
        };

        // Points first; every curve references an already-added point id.
        let mut ids: Vec<DesignPointId> = Vec::new();
        for (i, p) in self.points.iter().enumerate() {
            let x = clamp_magnitude(p[0], -GEOMETRY_MAGNITUDE_CAP, GEOMETRY_MAGNITUDE_CAP);
            let y = clamp_magnitude(p[1], -GEOMETRY_MAGNITUDE_CAP, GEOMETRY_MAGNITUDE_CAP);
            let label = format!("p{i}");
            if let Ok(id) = doc.add_point(label, [x, y]) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            return None;
        }

        // Positive-domain radius scalars, mirroring the survey fixture.
        let mut radius_ids: Vec<_> = Vec::new();
        for (i, s) in self.scalars.iter().enumerate() {
            let value = clamp_magnitude(*s, MIN_SCALE, GEOMETRY_MAGNITUDE_CAP);
            let label = format!("r{i}");
            if let Ok(id) = doc.add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            {
                radius_ids.push(id);
            }
        }

        // Curves. References are clamped into range, so each is always valid.
        for (kind, r0, r1, r2) in &self.curves {
            if ids.is_empty() {
                break;
            }
            let a = ids[(*r0 as usize) % ids.len()];
            match *kind {
                CURVE_LINE => {
                    if ids.len() >= 2 {
                        let b = ids[(*r1 as usize) % ids.len()];
                        // branch_direction is a semantic hint; [1, 0] is always
                        // valid and never affects the solved positions.
                        let _ = doc.add_curve(
                            "l",
                            CurveDefinition::Line {
                                start: a,
                                end: b,
                                branch_direction: [1.0, 0.0],
                            },
                        );
                    }
                }
                CURVE_CIRCLE => {
                    if !radius_ids.is_empty() {
                        let radius = radius_ids[(*r1 as usize) % radius_ids.len()];
                        let _ = doc.add_curve(
                            "c",
                            CurveDefinition::Circle {
                                center: a,
                                radius,
                            },
                        );
                    }
                }
                CURVE_BEZIER
                    if ids.len() >= 3 => {
                        let b = ids[(*r1 as usize) % ids.len()];
                        let c = ids[(*r2 as usize) % ids.len()];
                        let _ = doc.add_curve(
                            "b",
                            CurveDefinition::QuadraticBezier {
                                controls: [a, b, c],
                            },
                        );
                    }
                _ => {}
            }
        }

        Some(doc)
    }
}

/// The document's persistent identity namespace.
fn document_id() -> geosolve_sketch::DocumentId {
    geosolve_sketch::DocumentId(
        geosolve_sketch::PersistentId::from_u128(DOCUMENT_ID),
    )
}

/// Clamp `value` into `[lo, hi]`, mapping any non-finite input to `lo`.
fn clamp_magnitude(value: f64, lo: f64, hi: f64) -> f64 {
    if value.is_finite() && value >= lo && value <= hi {
        value
    } else if value.is_infinite() && value > 0.0 {
        hi
    } else {
        lo
    }
}

/// A fixed-buffer byte cursor for total decoding.
struct Cursor {
    buf: [u8; 512],
    pos: usize,
}

impl Cursor {
    fn f64(&mut self) -> f64 {
        if self.pos + 8 <= self.buf.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&self.buf[self.pos..self.pos + 8]);
            self.pos += 8;
            f64::from_le_bytes(b)
        } else {
            self.pos = self.buf.len();
            0.0
        }
    }

    fn byte(&mut self) -> u8 {
        let b = self.buf.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }
}
