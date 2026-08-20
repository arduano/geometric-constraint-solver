// SPDX-License-Identifier: GPL-3.0-or-later

//! Certified regular parallel curves used by revision-local computed features.

use std::f64::consts::TAU;

use geosolve_core::{
    OperationCheckpoint, OperationControl, OperationController, OperationWorkCounter,
};
use thiserror::Error;

use super::interval::Interval;
use super::pieces::{CurvePiece, PieceEvaluationError, piece_for_span};
use crate::document::document_arc_signed_sweep;
use crate::{CurveDefinition, CurveSpan, SketchDocument};

const DEFAULT_REGULARITY_MARGIN: f64 = 1.0e-8;
const DEFAULT_TANGENT_TOLERANCE_RADIANS: f64 = 2.0e-7;
const CERTIFICATION_CELLS: u32 = 4;

/// Explicit traversal of one complete native source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveOffsetTraversal {
    Forward,
    Reverse,
}

/// Deterministic approximation and certification policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveOffsetOptions {
    pub position_tolerance: f64,
    pub tangent_tolerance_radians: f64,
    pub regularity_margin: f64,
    pub max_depth: usize,
    pub max_patches: usize,
}

impl CurveOffsetOptions {
    #[must_use]
    pub fn for_model_scale(model_scale: f64) -> Self {
        Self {
            position_tolerance: 1.0e-8 * model_scale.abs().max(f64::MIN_POSITIVE),
            tangent_tolerance_radians: DEFAULT_TANGENT_TOLERANCE_RADIANS,
            regularity_margin: DEFAULT_REGULARITY_MARGIN,
            max_depth: 48,
            max_patches: 16_384,
        }
    }
}

/// One endpoint-Hermite cubic and its exact source-parameter provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveOffsetCubicPatch {
    pub source_parameters: [f64; 2],
    pub controls: [[f64; 2]; 4],
    /// Continuous position error over this patch.
    pub maximum_position_error: f64,
    /// Continuous `|d(Q-H)/ds|` bound for the patch-local parameter `s in [0, 1]`.
    /// Together with the exact endpoint interpolation this gives an error tube that shrinks to
    /// zero at both patch boundaries.
    pub maximum_local_derivative_error: f64,
    /// Continuous angular error between the mathematical parallel and fitted cubic tangents.
    pub maximum_tangent_error_radians: f64,
}

/// Exact analytic or interval-certified cubic representation of one parallel span.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveOffsetGeometry {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    CircularArc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep: f64,
        closed: bool,
    },
    CubicPatches(Vec<CurveOffsetCubicPatch>),
}

/// Independently derived regularity and approximation evidence for one parallel curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveOffsetCertificate {
    pub maximum_position_error: f64,
    pub maximum_tangent_error_radians: f64,
    pub minimum_regularity_factor: f64,
    pub subdivision_count: usize,
}

/// One complete regular parallel span.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveOffsetResult {
    pub source: CurveSpan,
    pub traversal: CurveOffsetTraversal,
    pub signed_distance: f64,
    pub geometry: CurveOffsetGeometry,
    pub certificate: CurveOffsetCertificate,
}

/// Typed fail-closed construction or certification failure.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CurveOffsetError {
    #[error("offset distance must be finite and nonzero")]
    InvalidDistance,
    #[error("curve-offset certification policy is invalid")]
    InvalidOptions,
    #[error("the native source span is missing or invalid")]
    InvalidSource,
    #[error("the source curve has a rational pole or non-finite enclosure")]
    InvalidGeometry,
    #[error("the source curve cannot be certified to have nonzero speed")]
    IrregularSource,
    #[error("the requested distance reaches or crosses the source evolute")]
    OffsetCusp,
    #[error("parallel-curve approximation tolerance could not be certified within depth {0}")]
    ApproximationToleranceUnmet(usize),
    #[error("parallel-curve output exceeds the configured patch limit {0}")]
    PatchLimitExceeded(usize),
}

/// Computes one regular parallel curve without adding solver variables or equations.
///
/// Exact line and circular carriers remain analytic. Other families are represented by a
/// deterministic endpoint-Hermite cubic chain. Source regularity is interval-certified; fit error
/// is bounded over deterministic interval subcells. The certificate integrates an interval
/// enclosure of the exact-minus-cubic second derivative from each subcell midpoint; it does not
/// infer a continuous bound from point samples.
///
/// # Errors
///
/// Returns a typed failure for invalid source/policy input, non-finite or irregular source
/// geometry, an offset cusp, an uncertifiable approximation, or deterministic work exhaustion.
///
/// # Panics
///
/// Panics only if the internally created unlimited operation controller reports an interruption.
pub fn compute_curve_offset(
    document: &SketchDocument,
    source: CurveSpan,
    traversal: CurveOffsetTraversal,
    signed_distance: f64,
    options: CurveOffsetOptions,
) -> Result<CurveOffsetResult, CurveOffsetError> {
    let mut controller = OperationController::new(OperationControl::unlimited());
    compute_curve_offset_with_controller(
        document,
        source,
        traversal,
        signed_distance,
        options,
        &mut controller,
    )
    .map(|result| result.expect("unlimited curve-offset evaluation cannot be interrupted"))
}

/// Computes one regular parallel curve under caller-owned cooperative operation control.
///
/// One `ProfileSubdivisions` unit authorizes the source span before any exact or adaptive offset
/// construction begins. Every actual adaptive subdivision charges one additional unit before
/// either recursive child is evaluated. A stopped controller returns `Ok(None)` and no partial
/// patch chain escapes.
///
/// # Errors
///
/// Returns the same typed geometry and certification failures as [`compute_curve_offset`].
pub fn compute_curve_offset_with_controller(
    document: &SketchDocument,
    source: CurveSpan,
    traversal: CurveOffsetTraversal,
    signed_distance: f64,
    options: CurveOffsetOptions,
    controller: &mut OperationController,
) -> Result<Option<CurveOffsetResult>, CurveOffsetError> {
    validate_inputs(signed_distance, options)?;
    let curve = document
        .curve(source.curve)
        .ok_or(CurveOffsetError::InvalidSource)?;
    document
        .evaluate_curve_jet(source, native_parameter_range(&curve.definition).0)
        .map_err(|_| CurveOffsetError::InvalidSource)?;
    let piece = piece_for_span(document, source).map_err(map_piece_error)?;
    let parameterization = PathParameterization::new(&curve.definition, traversal)?;
    if !charge_curve_offset_work(controller) {
        return Ok(None);
    }

    if matches!(
        curve.definition,
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
    ) {
        let (start, end) = exact_linear_offset(
            document,
            &curve.definition,
            source.segment,
            traversal,
            signed_distance,
        )?;
        return Ok(Some(CurveOffsetResult {
            source,
            traversal,
            signed_distance,
            geometry: CurveOffsetGeometry::Line { start, end },
            certificate: exact_certificate(1.0),
        }));
    }

    if let Some((center_id, source_sweep, closed)) =
        circular_definition(document, &curve.definition)
    {
        let center = document
            .point(center_id)
            .ok_or(CurveOffsetError::InvalidSource)?
            .position;
        let start = offset_sample(&piece, parameterization, 0.0, signed_distance)?;
        let radius = distance(start.position, center);
        if !radius.is_finite() || radius <= 0.0 || start.regularity <= options.regularity_margin {
            return Err(CurveOffsetError::OffsetCusp);
        }
        let sweep = match traversal {
            CurveOffsetTraversal::Forward => source_sweep,
            CurveOffsetTraversal::Reverse => -source_sweep,
        };
        return Ok(Some(CurveOffsetResult {
            source,
            traversal,
            signed_distance,
            geometry: CurveOffsetGeometry::CircularArc {
                center,
                radius,
                start_angle: (start.position[1] - center[1]).atan2(start.position[0] - center[0]),
                sweep,
                closed,
            },
            certificate: exact_certificate(start.regularity),
        }));
    }

    let mut state = ApproximationState {
        piece: &piece,
        parameterization,
        signed_distance,
        options,
        patches: Vec::new(),
        maximum_position_error: 0.0,
        maximum_tangent_error: 0.0,
        minimum_regularity: f64::INFINITY,
        subdivisions: 0,
    };
    let seeds: &[f64] = if matches!(curve.definition, CurveDefinition::Ellipse { .. }) {
        &[0.0, 0.25, 0.5, 0.75, 1.0]
    } else {
        &[0.0, 1.0]
    };
    for pair in seeds.windows(2) {
        if state
            .approximate(pair[0], pair[1], 0, controller)?
            .is_none()
        {
            return Ok(None);
        }
    }
    if state.patches.is_empty() || !state.minimum_regularity.is_finite() {
        return Err(CurveOffsetError::InvalidGeometry);
    }
    Ok(Some(CurveOffsetResult {
        source,
        traversal,
        signed_distance,
        geometry: CurveOffsetGeometry::CubicPatches(state.patches),
        certificate: CurveOffsetCertificate {
            maximum_position_error: state.maximum_position_error,
            maximum_tangent_error_radians: state.maximum_tangent_error,
            minimum_regularity_factor: state.minimum_regularity,
            subdivision_count: state.subdivisions,
        },
    }))
}

fn charge_curve_offset_work(controller: &mut OperationController) -> bool {
    controller
        .charge(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        )
        .is_ok()
}

fn exact_linear_offset(
    document: &SketchDocument,
    definition: &CurveDefinition,
    segment: u32,
    traversal: CurveOffsetTraversal,
    distance: f64,
) -> Result<([f64; 2], [f64; 2]), CurveOffsetError> {
    let segment = usize::try_from(segment).map_err(|_| CurveOffsetError::InvalidSource)?;
    let (start, end) = match definition {
        CurveDefinition::Line { start, end, .. } if segment == 0 => (*start, *end),
        CurveDefinition::Polyline { points, closed, .. } => {
            let start = *points.get(segment).ok_or(CurveOffsetError::InvalidSource)?;
            let end_index = if segment + 1 < points.len() {
                segment + 1
            } else if *closed && !points.is_empty() {
                0
            } else {
                return Err(CurveOffsetError::InvalidSource);
            };
            (start, points[end_index])
        }
        _ => return Err(CurveOffsetError::InvalidSource),
    };
    let mut start = document
        .point(start)
        .ok_or(CurveOffsetError::InvalidSource)?
        .position;
    let mut end = document
        .point(end)
        .ok_or(CurveOffsetError::InvalidSource)?
        .position;
    if traversal == CurveOffsetTraversal::Reverse {
        std::mem::swap(&mut start, &mut end);
    }
    let tangent = [end[0] - start[0], end[1] - start[1]];
    let speed = tangent[0].hypot(tangent[1]);
    if !speed.is_finite() || speed == 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let normal = [-tangent[1] / speed, tangent[0] / speed];
    let start = [
        distance.mul_add(normal[0], start[0]),
        distance.mul_add(normal[1], start[1]),
    ];
    let end = [
        distance.mul_add(normal[0], end[0]),
        distance.mul_add(normal[1], end[1]),
    ];
    if !start.into_iter().chain(end).all(f64::is_finite) {
        return Err(CurveOffsetError::InvalidGeometry);
    }
    Ok((start, end))
}

const fn exact_certificate(regularity: f64) -> CurveOffsetCertificate {
    CurveOffsetCertificate {
        maximum_position_error: 0.0,
        maximum_tangent_error_radians: 0.0,
        minimum_regularity_factor: regularity,
        subdivision_count: 0,
    }
}

fn validate_inputs(
    signed_distance: f64,
    options: CurveOffsetOptions,
) -> Result<(), CurveOffsetError> {
    if !signed_distance.is_finite() || signed_distance == 0.0 {
        return Err(CurveOffsetError::InvalidDistance);
    }
    if !options.position_tolerance.is_finite()
        || options.position_tolerance <= 0.0
        || !options.tangent_tolerance_radians.is_finite()
        || options.tangent_tolerance_radians <= 0.0
        || options.tangent_tolerance_radians >= std::f64::consts::FRAC_PI_2
        || !options.regularity_margin.is_finite()
        || options.regularity_margin <= 0.0
        || options.max_depth == 0
        || options.max_patches == 0
    {
        return Err(CurveOffsetError::InvalidOptions);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PathParameterization {
    offset: f64,
    rate: f64,
}

impl PathParameterization {
    fn new(
        definition: &CurveDefinition,
        traversal: CurveOffsetTraversal,
    ) -> Result<Self, CurveOffsetError> {
        let (lower, upper) = native_parameter_range(definition);
        let (offset, rate) = match traversal {
            CurveOffsetTraversal::Forward => (lower, upper - lower),
            CurveOffsetTraversal::Reverse => (upper, lower - upper),
        };
        if offset.is_finite() && rate.is_finite() && rate != 0.0 {
            Ok(Self { offset, rate })
        } else {
            Err(CurveOffsetError::InvalidSource)
        }
    }

    fn native(self, path_parameter: f64) -> f64 {
        self.rate.mul_add(path_parameter, self.offset)
    }

    fn native_interval(self, path: Interval) -> Interval {
        Interval::point(self.offset).add(path.mul(Interval::point(self.rate)))
    }
}

fn native_parameter_range(definition: &CurveDefinition) -> (f64, f64) {
    if matches!(
        definition,
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
    ) {
        (0.0, TAU)
    } else {
        (0.0, 1.0)
    }
}

fn circular_definition(
    document: &SketchDocument,
    definition: &CurveDefinition,
) -> Option<(crate::DesignPointId, f64, bool)> {
    match definition {
        CurveDefinition::Circle { center, .. } => Some((*center, TAU, true)),
        CurveDefinition::CircularArc {
            center,
            start_angle,
            end_angle,
            sweep,
            ..
        } => Some((
            *center,
            document_arc_signed_sweep(
                document.scalar(*start_angle)?.value,
                document.scalar(*end_angle)?.value,
                *sweep,
            )
            .ok()?,
            false,
        )),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct OffsetSample {
    position: [f64; 2],
    derivative: [f64; 2],
    regularity: f64,
}

fn offset_sample(
    piece: &CurvePiece,
    parameterization: PathParameterization,
    parameter: f64,
    distance: f64,
) -> Result<OffsetSample, CurveOffsetError> {
    let native = parameterization.native(parameter);
    let position = piece.point(native).map_err(map_piece_error)?;
    let native_tangent = piece.tangent(native).map_err(map_piece_error)?;
    let velocity = scale(native_tangent, parameterization.rate);
    let acceleration = piece
        .second_derivative(Interval::point(native))
        .map_err(map_piece_error)?;
    let acceleration = scale(
        [acceleration[0].midpoint(), acceleration[1].midpoint()],
        parameterization.rate.powi(2),
    );
    let speed = norm(velocity);
    if !speed.is_finite() || speed == 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let normal = [-velocity[1] / speed, velocity[0] / speed];
    let curvature = cross(velocity, acceleration) / speed.powi(3);
    let regularity = 1.0 - distance * curvature;
    let offset_position = add(position, scale(normal, distance));
    let derivative = scale(velocity, regularity);
    if !offset_position.into_iter().all(f64::is_finite)
        || !derivative.into_iter().all(f64::is_finite)
        || !regularity.is_finite()
    {
        return Err(CurveOffsetError::InvalidGeometry);
    }
    Ok(OffsetSample {
        position: offset_position,
        derivative,
        regularity,
    })
}

struct ApproximationState<'a> {
    piece: &'a CurvePiece,
    parameterization: PathParameterization,
    signed_distance: f64,
    options: CurveOffsetOptions,
    patches: Vec<CurveOffsetCubicPatch>,
    maximum_position_error: f64,
    maximum_tangent_error: f64,
    minimum_regularity: f64,
    subdivisions: usize,
}

impl ApproximationState<'_> {
    fn approximate(
        &mut self,
        start: f64,
        end: f64,
        depth: usize,
        controller: &mut OperationController,
    ) -> Result<Option<()>, CurveOffsetError> {
        let start_sample = offset_sample(
            self.piece,
            self.parameterization,
            start,
            self.signed_distance,
        )?;
        let end_sample =
            offset_sample(self.piece, self.parameterization, end, self.signed_distance)?;
        let length = end - start;
        let controls = [
            start_sample.position,
            add(
                start_sample.position,
                scale(start_sample.derivative, length / 3.0),
            ),
            add(
                end_sample.position,
                scale(end_sample.derivative, -length / 3.0),
            ),
            end_sample.position,
        ];
        let bounds = interval_bounds(
            self.piece,
            self.parameterization,
            self.signed_distance,
            start,
            end,
            controls,
        );
        let bounds = match bounds {
            Ok(bounds) => bounds,
            Err(CurveOffsetError::OffsetCusp) => return Err(CurveOffsetError::OffsetCusp),
            Err(error) if depth >= self.options.max_depth => return Err(error),
            Err(_) => {
                return self.subdivide(start, end, depth, controller);
            }
        };
        let coordinate_scale = controls
            .iter()
            .flatten()
            .fold(1.0_f64, |scale, value| scale.max(value.abs()));
        let position_tolerance = self
            .options
            .position_tolerance
            .max(256.0 * f64::EPSILON * coordinate_scale);
        if bounds.minimum_regularity <= self.options.regularity_margin
            || bounds.position_error > position_tolerance
            || bounds.tangent_error > self.options.tangent_tolerance_radians
        {
            if depth >= self.options.max_depth {
                return if bounds.minimum_regularity <= self.options.regularity_margin {
                    Err(CurveOffsetError::OffsetCusp)
                } else {
                    Err(CurveOffsetError::ApproximationToleranceUnmet(
                        self.options.max_depth,
                    ))
                };
            }
            return self.subdivide(start, end, depth, controller);
        }
        if self.patches.len() >= self.options.max_patches {
            return Err(CurveOffsetError::PatchLimitExceeded(
                self.options.max_patches,
            ));
        }
        self.maximum_position_error = self.maximum_position_error.max(bounds.position_error);
        self.maximum_tangent_error = self.maximum_tangent_error.max(bounds.tangent_error);
        self.minimum_regularity = self.minimum_regularity.min(bounds.minimum_regularity);
        self.patches.push(CurveOffsetCubicPatch {
            source_parameters: [
                self.parameterization.native(start),
                self.parameterization.native(end),
            ],
            controls,
            maximum_position_error: bounds.position_error,
            maximum_local_derivative_error: outward_product(bounds.derivative_error, length.abs()),
            maximum_tangent_error_radians: bounds.tangent_error,
        });
        Ok(Some(()))
    }

    fn subdivide(
        &mut self,
        start: f64,
        end: f64,
        depth: usize,
        controller: &mut OperationController,
    ) -> Result<Option<()>, CurveOffsetError> {
        if self.subdivisions >= self.options.max_patches {
            return Err(CurveOffsetError::PatchLimitExceeded(
                self.options.max_patches,
            ));
        }
        if !charge_curve_offset_work(controller) {
            return Ok(None);
        }
        self.subdivisions =
            self.subdivisions
                .checked_add(1)
                .ok_or(CurveOffsetError::PatchLimitExceeded(
                    self.options.max_patches,
                ))?;
        let middle = start + 0.5 * (end - start);
        if self
            .approximate(start, middle, depth + 1, controller)?
            .is_none()
        {
            return Ok(None);
        }
        self.approximate(middle, end, depth + 1, controller)
    }
}

#[derive(Clone, Copy, Debug)]
struct ApproximationBounds {
    position_error: f64,
    derivative_error: f64,
    tangent_error: f64,
    minimum_regularity: f64,
}

fn interval_bounds(
    piece: &CurvePiece,
    parameterization: PathParameterization,
    distance: f64,
    start: f64,
    end: f64,
    controls: [[f64; 2]; 4],
) -> Result<ApproximationBounds, CurveOffsetError> {
    let mut maximum_position_error = 0.0_f64;
    let mut maximum_derivative_error = 0.0_f64;
    let mut maximum_tangent_error = 0.0_f64;
    let mut minimum_regularity = f64::INFINITY;
    let patch_length = end - start;
    let patch_domain = Interval::checked(start, end).ok_or(CurveOffsetError::InvalidGeometry)?;
    let exact_third_derivative =
        offset_third_derivative_bounds(piece, parameterization, distance, patch_domain)?;
    let third_derivative_error = vector_norm_upper(subtract_intervals(
        exact_third_derivative,
        cubic_third_derivative(controls, patch_length),
    ))?;
    for cell in 0..CERTIFICATION_CELLS {
        // Pin the outer boundaries to the exact patch endpoints. Computing both through the
        // fractional partition can round the first boundary above `start` or the final boundary
        // below `end`, leaving a tiny interval that the otherwise-continuous certificate did not
        // cover.
        let cell_start = if cell == 0 {
            start
        } else {
            start + patch_length * (f64::from(cell) / f64::from(CERTIFICATION_CELLS))
        };
        let cell_end = if cell + 1 == CERTIFICATION_CELLS {
            end
        } else {
            start + patch_length * (f64::from(cell + 1) / f64::from(CERTIFICATION_CELLS))
        };
        let cell_middle = cell_start + 0.5 * (cell_end - cell_start);
        let half_cell_length = Interval::point(cell_end)
            .sub(Interval::point(cell_middle))
            .include(Interval::point(cell_middle).sub(Interval::point(cell_start)))
            .upper;
        let cell_domain =
            Interval::checked(cell_start, cell_end).ok_or(CurveOffsetError::InvalidGeometry)?;
        let exact_cell =
            offset_first_derivative_bounds(piece, parameterization, distance, cell_domain)?;
        let midpoint = Interval::point(cell_middle);
        let exact_midpoint =
            offset_second_derivative_bounds(piece, parameterization, distance, midpoint)?;
        let cubic_parameter = Interval::point(cell_middle)
            .sub(Interval::point(start))
            .div(Interval::point(patch_length))
            .ok_or(CurveOffsetError::InvalidGeometry)?;
        let (cubic_position, cubic_derivative) =
            cubic_interval_sample(controls, patch_length, cubic_parameter);

        let midpoint_position_error =
            vector_norm_upper(subtract_intervals(exact_midpoint.position, cubic_position))?;
        let midpoint_derivative_error = vector_norm_upper(subtract_intervals(
            exact_midpoint.first_derivative,
            cubic_derivative,
        ))?;
        let midpoint_second_derivative_error = vector_norm_upper(subtract_intervals(
            exact_midpoint.second_derivative,
            cubic_second_derivative(controls, patch_length, cubic_parameter),
        ))?;

        // For E = Q - H and every t in this subcell,
        //
        //   |E''(t)| <= |E''(m)| + |t-m| sup |E'''|
        //   |E'(t)|  <= |E'(m)|  + |t-m| sup |E''|
        //   |E(t)|   <= |E(m)|   + |t-m| sup |E'|.
        //
        // Every quantity on the right is outward-rounded interval evidence. This makes the
        // reported maxima continuous bounds over the whole cell, rather than sampled estimates.
        let second_derivative_error = outward_sum(
            midpoint_second_derivative_error,
            outward_product(half_cell_length, third_derivative_error),
        );
        let derivative_error = outward_sum(
            midpoint_derivative_error,
            outward_product(half_cell_length, second_derivative_error),
        );
        let position_error = outward_sum(
            midpoint_position_error,
            outward_product(half_cell_length, derivative_error),
        );

        let exact_speed_lower = vector_norm_lower(exact_cell.first_derivative)?;
        if exact_speed_lower <= 0.0 || derivative_error >= exact_speed_lower {
            return Err(CurveOffsetError::IrregularSource);
        }
        // If |H' - Q'| <= e < |Q'|, the angle between H' and Q' is at most asin(e/|Q'|).
        // x / sqrt(1-x^2) is a conservative algebraic upper bound for asin(x) on [0, 1),
        // allowing the certificate to remain outward-rounded without relying on libm rounding.
        let ratio = Interval::point(derivative_error)
            .div(Interval::point(exact_speed_lower))
            .ok_or(CurveOffsetError::IrregularSource)?;
        let complement = Interval::ONE.sub(ratio.square());
        let tangent_error = ratio
            .div(
                complement
                    .sqrt()
                    .filter(|value| value.lower > 0.0)
                    .ok_or(CurveOffsetError::IrregularSource)?,
            )
            .ok_or(CurveOffsetError::IrregularSource)?
            .upper;
        if !position_error.is_finite() || !tangent_error.is_finite() {
            return Err(CurveOffsetError::InvalidGeometry);
        }
        maximum_position_error = maximum_position_error.max(position_error);
        maximum_derivative_error = maximum_derivative_error.max(derivative_error);
        maximum_tangent_error = maximum_tangent_error.max(tangent_error);
        minimum_regularity = minimum_regularity.min(exact_cell.minimum_regularity);
    }
    Ok(ApproximationBounds {
        position_error: maximum_position_error,
        derivative_error: maximum_derivative_error,
        tangent_error: maximum_tangent_error,
        minimum_regularity,
    })
}

#[derive(Clone, Copy)]
struct OffsetFirstDerivativeBounds {
    first_derivative: [Interval; 2],
    minimum_regularity: f64,
}

#[derive(Clone, Copy)]
struct OffsetSecondDerivativeBounds {
    position: [Interval; 2],
    first_derivative: [Interval; 2],
    second_derivative: [Interval; 2],
}

fn offset_first_derivative_bounds(
    piece: &CurvePiece,
    parameterization: PathParameterization,
    distance: f64,
    path: Interval,
) -> Result<OffsetFirstDerivativeBounds, CurveOffsetError> {
    let native = parameterization.native_interval(path);
    let velocity = scale_intervals(
        piece.derivative(native).map_err(map_piece_error)?,
        parameterization.rate,
    );
    let acceleration = scale_intervals(
        piece.second_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(2),
    );
    let speed = vector_norm_interval(velocity)?;
    if speed.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let speed_cubed = speed.powi(3);
    let velocity_cross_acceleration = cross_intervals(velocity, acceleration);
    let curvature = velocity_cross_acceleration
        .div(speed_cubed)
        .ok_or(CurveOffsetError::IrregularSource)?;
    let regularity = Interval::ONE.sub(curvature.scale(distance));
    if regularity.upper <= 0.0 {
        return Err(CurveOffsetError::OffsetCusp);
    }
    if regularity.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }

    let first_derivative = scale_intervals_by_interval(velocity, regularity);
    if !first_derivative.into_iter().all(Interval::is_finite) {
        return Err(CurveOffsetError::InvalidGeometry);
    }
    Ok(OffsetFirstDerivativeBounds {
        first_derivative,
        minimum_regularity: regularity.lower,
    })
}

fn offset_second_derivative_bounds(
    piece: &CurvePiece,
    parameterization: PathParameterization,
    distance: f64,
    path: Interval,
) -> Result<OffsetSecondDerivativeBounds, CurveOffsetError> {
    let native = parameterization.native_interval(path);
    let source_position = piece.position(native).map_err(map_piece_error)?;
    let velocity = scale_intervals(
        piece.derivative(native).map_err(map_piece_error)?,
        parameterization.rate,
    );
    let acceleration = scale_intervals(
        piece.second_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(2),
    );
    let jerk = scale_intervals(
        piece.third_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(3),
    );
    let speed = vector_norm_interval(velocity)?;
    if speed.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let velocity_cross_acceleration = cross_intervals(velocity, acceleration);
    let curvature = velocity_cross_acceleration
        .div(speed.powi(3))
        .ok_or(CurveOffsetError::IrregularSource)?;
    let regularity = Interval::ONE.sub(curvature.scale(distance));
    if regularity.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let normal = [
        velocity[1]
            .neg()
            .div(speed)
            .ok_or(CurveOffsetError::IrregularSource)?,
        velocity[0]
            .div(speed)
            .ok_or(CurveOffsetError::IrregularSource)?,
    ];
    let position = [
        source_position.x.add(normal[0].scale(distance)),
        source_position.y.add(normal[1].scale(distance)),
    ];
    let first_derivative = scale_intervals_by_interval(velocity, regularity);
    let curvature_derivative = cross_intervals(velocity, jerk)
        .div(speed.powi(3))
        .ok_or(CurveOffsetError::IrregularSource)?
        .sub(
            velocity_cross_acceleration
                .mul(dot_intervals(velocity, acceleration))
                .scale(3.0)
                .div(speed.powi(5))
                .ok_or(CurveOffsetError::IrregularSource)?,
        );
    let second_derivative = add_intervals(
        scale_intervals_by_interval(acceleration, regularity),
        scale_intervals_by_interval(velocity, curvature_derivative.scale(-distance)),
    );
    if !position
        .into_iter()
        .chain(first_derivative)
        .chain(second_derivative)
        .all(Interval::is_finite)
    {
        return Err(CurveOffsetError::InvalidGeometry);
    }
    Ok(OffsetSecondDerivativeBounds {
        position,
        first_derivative,
        second_derivative,
    })
}

fn offset_third_derivative_bounds(
    piece: &CurvePiece,
    parameterization: PathParameterization,
    distance: f64,
    path: Interval,
) -> Result<[Interval; 2], CurveOffsetError> {
    let native = parameterization.native_interval(path);
    let velocity = scale_intervals(
        piece.derivative(native).map_err(map_piece_error)?,
        parameterization.rate,
    );
    let acceleration = scale_intervals(
        piece.second_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(2),
    );
    let jerk = scale_intervals(
        piece.third_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(3),
    );
    let snap = scale_intervals(
        piece.fourth_derivative(native).map_err(map_piece_error)?,
        parameterization.rate.powi(4),
    );
    let speed = vector_norm_interval(velocity)?;
    if speed.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let speed_cubed = speed.powi(3);
    let velocity_cross_acceleration = cross_intervals(velocity, acceleration);
    let curvature = velocity_cross_acceleration
        .div(speed_cubed)
        .ok_or(CurveOffsetError::IrregularSource)?;
    let regularity = Interval::ONE.sub(curvature.scale(distance));
    if regularity.lower <= 0.0 {
        return Err(CurveOffsetError::IrregularSource);
    }
    let velocity_dot_acceleration = dot_intervals(velocity, acceleration);
    let curvature_derivative = cross_intervals(velocity, jerk)
        .div(speed_cubed)
        .ok_or(CurveOffsetError::IrregularSource)?
        .sub(
            velocity_cross_acceleration
                .mul(velocity_dot_acceleration)
                .scale(3.0)
                .div(speed.powi(5))
                .ok_or(CurveOffsetError::IrregularSource)?,
        );
    let curvature_second_derivative = cross_intervals(acceleration, jerk)
        .add(cross_intervals(velocity, snap))
        .div(speed_cubed)
        .ok_or(CurveOffsetError::IrregularSource)?
        .sub(
            cross_intervals(velocity, jerk)
                .mul(velocity_dot_acceleration)
                .scale(6.0)
                .div(speed.powi(5))
                .ok_or(CurveOffsetError::IrregularSource)?,
        )
        .sub(
            velocity_cross_acceleration
                .mul(dot_intervals(acceleration, acceleration).add(dot_intervals(velocity, jerk)))
                .scale(3.0)
                .div(speed.powi(5))
                .ok_or(CurveOffsetError::IrregularSource)?,
        )
        .add(
            velocity_cross_acceleration
                .mul(velocity_dot_acceleration.square())
                .scale(15.0)
                .div(speed.powi(7))
                .ok_or(CurveOffsetError::IrregularSource)?,
        );
    let third_derivative = add_intervals(
        add_intervals(
            scale_intervals_by_interval(jerk, regularity),
            scale_intervals_by_interval(acceleration, curvature_derivative.scale(-2.0 * distance)),
        ),
        scale_intervals_by_interval(velocity, curvature_second_derivative.scale(-distance)),
    );
    third_derivative
        .into_iter()
        .all(Interval::is_finite)
        .then_some(third_derivative)
        .ok_or(CurveOffsetError::InvalidGeometry)
}

fn cubic_interval_sample(
    controls: [[f64; 2]; 4],
    parameter_length: f64,
    parameter: Interval,
) -> ([Interval; 2], [Interval; 2]) {
    let position = std::array::from_fn(|axis| {
        let controls = controls.map(|point| Interval::point(point[axis]));
        let first = interval_lerp(controls[0], controls[1], parameter);
        let second = interval_lerp(controls[1], controls[2], parameter);
        let third = interval_lerp(controls[2], controls[3], parameter);
        interval_lerp(
            interval_lerp(first, second, parameter),
            interval_lerp(second, third, parameter),
            parameter,
        )
    });
    let derivative_scale = Interval::point(3.0)
        .div(Interval::point(parameter_length))
        .expect("validated nonzero patch length");
    let derivative = std::array::from_fn(|axis| {
        let differences = [
            Interval::point(controls[1][axis]).sub(Interval::point(controls[0][axis])),
            Interval::point(controls[2][axis]).sub(Interval::point(controls[1][axis])),
            Interval::point(controls[3][axis]).sub(Interval::point(controls[2][axis])),
        ];
        interval_lerp(
            interval_lerp(differences[0], differences[1], parameter),
            interval_lerp(differences[1], differences[2], parameter),
            parameter,
        )
        .mul(derivative_scale)
    });
    (position, derivative)
}

fn cubic_second_derivative(
    controls: [[f64; 2]; 4],
    parameter_length: f64,
    parameter: Interval,
) -> [Interval; 2] {
    let length_squared = Interval::point(parameter_length).square();
    let derivative_scale = Interval::point(6.0)
        .div(length_squared)
        .expect("validated nonzero patch length");
    std::array::from_fn(|axis| {
        let first = Interval::point(controls[2][axis])
            .sub(Interval::point(controls[1][axis]).scale(2.0))
            .add(Interval::point(controls[0][axis]));
        let second = Interval::point(controls[3][axis])
            .sub(Interval::point(controls[2][axis]).scale(2.0))
            .add(Interval::point(controls[1][axis]));
        interval_lerp(first, second, parameter).mul(derivative_scale)
    })
}

fn cubic_third_derivative(controls: [[f64; 2]; 4], parameter_length: f64) -> [Interval; 2] {
    let derivative_scale = Interval::point(6.0)
        .div(Interval::point(parameter_length).powi(3))
        .expect("validated nonzero patch length");
    std::array::from_fn(|axis| {
        Interval::point(controls[3][axis])
            .sub(Interval::point(controls[2][axis]).scale(3.0))
            .add(Interval::point(controls[1][axis]).scale(3.0))
            .sub(Interval::point(controls[0][axis]))
            .mul(derivative_scale)
    })
}

fn interval_lerp(start: Interval, end: Interval, parameter: Interval) -> Interval {
    start.add(end.sub(start).mul(parameter))
}

fn vector_norm_interval(value: [Interval; 2]) -> Result<Interval, CurveOffsetError> {
    let squared = value[0].square().add(value[1].square());
    Interval::checked(squared.lower.max(0.0), squared.upper)
        .ok_or(CurveOffsetError::InvalidGeometry)?
        .sqrt()
        .ok_or(CurveOffsetError::InvalidGeometry)
}

fn vector_norm_upper(value: [Interval; 2]) -> Result<f64, CurveOffsetError> {
    Ok(vector_norm_interval(value)?.upper)
}

fn vector_norm_lower(value: [Interval; 2]) -> Result<f64, CurveOffsetError> {
    Ok(vector_norm_interval(value)?.lower)
}

fn outward_sum(first: f64, second: f64) -> f64 {
    Interval::point(first).add(Interval::point(second)).upper
}

fn outward_product(first: f64, second: f64) -> f64 {
    Interval::point(first).mul(Interval::point(second)).upper
}

fn map_piece_error(error: PieceEvaluationError) -> CurveOffsetError {
    match error {
        PieceEvaluationError::Pole | PieceEvaluationError::NonFinite => {
            CurveOffsetError::InvalidGeometry
        }
    }
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn scale(value: [f64; 2], factor: f64) -> [f64; 2] {
    [value[0] * factor, value[1] * factor]
}

fn norm(value: [f64; 2]) -> f64 {
    value[0].hypot(value[1])
}

fn distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    norm([first[0] - second[0], first[1] - second[1]])
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[1], -first[1] * second[0])
}

fn scale_intervals(value: [Interval; 2], factor: f64) -> [Interval; 2] {
    [value[0].scale(factor), value[1].scale(factor)]
}

fn scale_intervals_by_interval(value: [Interval; 2], factor: Interval) -> [Interval; 2] {
    [value[0].mul(factor), value[1].mul(factor)]
}

fn add_intervals(first: [Interval; 2], second: [Interval; 2]) -> [Interval; 2] {
    [first[0].add(second[0]), first[1].add(second[1])]
}

fn subtract_intervals(first: [Interval; 2], second: [Interval; 2]) -> [Interval; 2] {
    [first[0].sub(second[0]), first[1].sub(second[1])]
}

fn dot_intervals(first: [Interval; 2], second: [Interval; 2]) -> Interval {
    first[0].mul(second[0]).add(first[1].mul(second[1]))
}

fn cross_intervals(first: [Interval; 2], second: [Interval; 2]) -> Interval {
    first[0].mul(second[1]).sub(first[1].mul(second[0]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentArcSweep, DocumentBSplineForm, DocumentHyperbolaBranch, ScalarDomain, ScalarUnit,
    };

    fn scalar(
        document: &mut SketchDocument,
        label: &str,
        value: f64,
        unit: ScalarUnit,
        domain: ScalarDomain,
    ) -> crate::DesignScalarId {
        document.add_scalar(label, value, unit, domain).unwrap()
    }

    fn cubic_sample(controls: [[f64; 2]; 4], parameter: f64) -> ([f64; 2], [f64; 2]) {
        let difference =
            |first: [f64; 2], second: [f64; 2]| [first[0] - second[0], first[1] - second[1]];
        let complement = 1.0 - parameter;
        let position = add(
            add(
                scale(controls[0], complement.powi(3)),
                scale(controls[1], 3.0 * complement.powi(2) * parameter),
            ),
            add(
                scale(controls[2], 3.0 * complement * parameter.powi(2)),
                scale(controls[3], parameter.powi(3)),
            ),
        );
        let derivative = scale(
            add(
                add(
                    scale(difference(controls[1], controls[0]), complement.powi(2)),
                    scale(
                        difference(controls[2], controls[1]),
                        2.0 * complement * parameter,
                    ),
                ),
                scale(difference(controls[3], controls[2]), parameter.powi(2)),
            ),
            3.0,
        );
        (position, derivative)
    }

    #[track_caller]
    fn assert_independent_patch_samples(
        document: &SketchDocument,
        span: CurveSpan,
        signed_distance: f64,
        patches: &[CurveOffsetCubicPatch],
        label: &str,
    ) {
        let numerical_position =
            8_192.0 * f64::EPSILON * document.model_scale().abs().max(f64::MIN_POSITIVE);
        for pair in patches.windows(2) {
            assert_eq!(
                pair[0].source_parameters[1].to_bits(),
                pair[1].source_parameters[0].to_bits(),
                "{label} source intervals must be contiguous"
            );
            assert!(
                distance(pair[0].controls[3], pair[1].controls[0]) <= numerical_position,
                "{label} fitted patches must join exactly"
            );
        }
        for patch in patches {
            let native_rate = patch.source_parameters[1] - patch.source_parameters[0];
            assert!(native_rate.is_finite() && native_rate != 0.0, "{label}");
            for parameter in [0.0_f64, 0.25, 0.5, 0.75, 1.0] {
                let native_parameter = native_rate.mul_add(parameter, patch.source_parameters[0]);
                let jet = document.evaluate_curve_jet(span, native_parameter).unwrap();
                let traversal_velocity = [
                    jet.first_derivative.x * native_rate.signum(),
                    jet.first_derivative.y * native_rate.signum(),
                ];
                let source_speed = norm(traversal_velocity);
                assert!(source_speed.is_finite() && source_speed > 0.0, "{label}");
                let source_tangent = scale(traversal_velocity, source_speed.recip());
                let expected_position = add(
                    [jet.position.x, jet.position.y],
                    scale([-source_tangent[1], source_tangent[0]], signed_distance),
                );
                let (fitted_position, fitted_derivative) = cubic_sample(patch.controls, parameter);
                let fitted_speed = norm(fitted_derivative);
                assert!(fitted_speed.is_finite() && fitted_speed > 0.0, "{label}");
                let fitted_tangent = scale(fitted_derivative, fitted_speed.recip());
                let position_error = distance(fitted_position, expected_position);
                assert!(
                    position_error <= patch.maximum_position_error + numerical_position,
                    "{label} sampled position error {position_error:e} exceeds certified bound {:e}",
                    patch.maximum_position_error
                );
                let tangent_error = cross(source_tangent, fitted_tangent).abs().atan2(
                    source_tangent[0]
                        .mul_add(fitted_tangent[0], source_tangent[1] * fitted_tangent[1]),
                );
                assert!(
                    tangent_error <= patch.maximum_tangent_error_radians + 4_096.0 * f64::EPSILON,
                    "{label} sampled tangent error {tangent_error:e} exceeds certified bound {:e}",
                    patch.maximum_tangent_error_radians
                );
            }
        }
    }

    fn ellipse_document() -> (SketchDocument, CurveSpan) {
        let mut document = SketchDocument::new(10.0).unwrap();
        let center = document.add_point("center", [0.0, 0.0]).unwrap();
        let major = document.add_point("major", [4.0, 0.0]).unwrap();
        let ratio = document
            .add_scalar(
                "ratio",
                0.5,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
            )
            .unwrap();
        let curve = document
            .add_curve(
                "ellipse",
                CurveDefinition::Ellipse {
                    center,
                    major_axis_point: major,
                    minor_axis_ratio: ratio,
                },
            )
            .unwrap();
        (document, CurveSpan::line(curve))
    }

    #[test]
    fn ellipse_offset_is_deterministic_regular_and_endpoint_exact() {
        let (document, span) = ellipse_document();
        let options = CurveOffsetOptions::for_model_scale(document.model_scale());
        let first = compute_curve_offset(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            -0.25,
            options,
        )
        .unwrap();
        let second = compute_curve_offset(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            -0.25,
            options,
        )
        .unwrap();
        assert_eq!(first, second);
        let CurveOffsetGeometry::CubicPatches(patches) = &first.geometry else {
            panic!("ellipse offset must use certified cubic patches");
        };
        assert!(!patches.is_empty());
        assert!(first.certificate.minimum_regularity_factor > options.regularity_margin);
        assert!(first.certificate.maximum_position_error <= options.position_tolerance);
        assert!(
            first.certificate.maximum_tangent_error_radians <= options.tangent_tolerance_radians
        );
        assert!((patches.first().unwrap().controls[0][0] - 4.25).abs() <= 1.0e-12);
        assert!(patches.first().unwrap().controls[0][1].abs() <= 1.0e-12);
        let terminal = patches.last().unwrap().controls[3];
        assert!((terminal[0] - 4.25).abs() <= 1.0e-12);
        assert!(terminal[1].abs() <= 1.0e-12);
    }

    #[test]
    fn reverse_traversal_flips_the_left_offset_side() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [2.0, 0.0]).unwrap();
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let options = CurveOffsetOptions::for_model_scale(1.0);
        let forward = compute_curve_offset(
            &document,
            CurveSpan::line(curve),
            CurveOffsetTraversal::Forward,
            0.5,
            options,
        )
        .unwrap();
        let reverse = compute_curve_offset(
            &document,
            CurveSpan::line(curve),
            CurveOffsetTraversal::Reverse,
            0.5,
            options,
        )
        .unwrap();
        let CurveOffsetGeometry::Line {
            start: forward_start,
            end: forward_end,
        } = forward.geometry
        else {
            panic!("line remains exact");
        };
        let CurveOffsetGeometry::Line {
            start: reverse_start,
            end: reverse_end,
        } = reverse.geometry
        else {
            panic!("line remains exact");
        };
        for (actual, expected) in [
            (forward_start, [0.0, 0.5]),
            (forward_end, [2.0, 0.5]),
            (reverse_start, [2.0, -0.5]),
            (reverse_end, [0.0, -0.5]),
        ] {
            assert!(distance(actual, expected) <= 1.0e-14);
        }
    }

    #[test]
    fn line_circle_and_circular_arc_offsets_remain_exact_analytic_geometry() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let center = document.add_point("center", [1.0, 2.0]).unwrap();
        let radius = scalar(
            &mut document,
            "radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        let circle = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        let start_angle = scalar(
            &mut document,
            "start",
            0.0,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        let end_angle = scalar(
            &mut document,
            "end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        );
        let arc_radius = scalar(
            &mut document,
            "arc radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        );
        let arc = document
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius: arc_radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap();
        let options = CurveOffsetOptions::for_model_scale(1.0);

        let forward = compute_curve_offset(
            &document,
            CurveSpan::line(circle),
            CurveOffsetTraversal::Forward,
            0.5,
            options,
        )
        .unwrap();
        let reverse = compute_curve_offset(
            &document,
            CurveSpan::line(circle),
            CurveOffsetTraversal::Reverse,
            0.5,
            options,
        )
        .unwrap();
        let arc = compute_curve_offset(
            &document,
            CurveSpan::line(arc),
            CurveOffsetTraversal::Forward,
            0.5,
            options,
        )
        .unwrap();

        for (result, expected_radius, expected_sweep, expected_closed) in [
            (forward, 2.5, TAU, true),
            (reverse, 3.5, -TAU, true),
            (arc, 2.5, std::f64::consts::FRAC_PI_2, false),
        ] {
            let CurveOffsetGeometry::CircularArc {
                center: actual_center,
                radius,
                sweep,
                closed,
                ..
            } = result.geometry
            else {
                panic!("circular carriers must remain analytic");
            };
            assert!(distance(actual_center, [1.0, 2.0]) <= 1.0e-14);
            assert!((radius - expected_radius).abs() <= 1.0e-14);
            assert!((sweep - expected_sweep).abs() <= 1.0e-14);
            assert_eq!(closed, expected_closed);
            assert_eq!(
                result.certificate.maximum_position_error.to_bits(),
                0.0_f64.to_bits()
            );
            assert_eq!(
                result.certificate.maximum_tangent_error_radians.to_bits(),
                0.0_f64.to_bits()
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one family matrix keeps every built-in general-curve fixture and certificate assertion together"
    )]
    fn every_general_curve_family_is_certified_at_production_tolerance_across_scales_and_transforms()
     {
        for (scale_index, scale) in [1.0e-6_f64, 1.0, 1.0e6].into_iter().enumerate() {
            let rotation = [0.37_f64, -0.61, 1.13][scale_index];
            let translation =
                [[7.0, -5.0], [-4.0, 8.0], [11.0, 3.0]][scale_index].map(|value| value * scale);
            let (sin, cos) = rotation.sin_cos();
            let transform = |position: [f64; 2]| {
                [
                    scale.mul_add(cos.mul_add(position[0], -sin * position[1]), translation[0]),
                    scale.mul_add(sin.mul_add(position[0], cos * position[1]), translation[1]),
                ]
            };
            let mut document = SketchDocument::new(scale).unwrap();
            let point = |document: &mut SketchDocument, label, position| {
                document.add_point(label, transform(position)).unwrap()
            };
            let ratio = scalar(
                &mut document,
                "ratio",
                0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
            );
            let ellipse_center = point(&mut document, "ellipse center", [0.0, 0.0]);
            let ellipse_axis = point(&mut document, "ellipse axis", [3.0, 0.0]);
            let ellipse_start = scalar(
                &mut document,
                "ellipse start",
                -0.75,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let ellipse_end = scalar(
                &mut document,
                "ellipse end",
                0.9,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            );
            let full_ellipse_ratio = scalar(
                &mut document,
                "full ellipse ratio",
                0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
            );
            let ellipse = document
                .add_curve(
                    "ellipse",
                    CurveDefinition::Ellipse {
                        center: ellipse_center,
                        major_axis_point: ellipse_axis,
                        minor_axis_ratio: full_ellipse_ratio,
                    },
                )
                .unwrap();
            let elliptical_arc = document
                .add_curve(
                    "elliptical arc",
                    CurveDefinition::EllipticalArc {
                        center: ellipse_center,
                        major_axis_point: ellipse_axis,
                        minor_axis_ratio: ratio,
                        start_angle: ellipse_start,
                        end_angle: ellipse_end,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .unwrap();

            let rational_start = point(&mut document, "rational start", [0.0, 0.0]);
            let rational_end = point(&mut document, "rational end", [2.0, 0.0]);
            let rational_weight = scalar(
                &mut document,
                "rational weight",
                0.8,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: crate::MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            );
            let rational = document
                .add_curve(
                    "rational",
                    CurveDefinition::RationalQuadraticConic {
                        start: rational_start,
                        weighted_middle: transform([1.0, 1.5]).map(|value| 0.8 * value),
                        middle_weight: rational_weight,
                        end: rational_end,
                    },
                )
                .unwrap();

            let parabola_vertex = point(&mut document, "parabola vertex", [0.0, 0.0]);
            let parabola_focus = point(&mut document, "parabola focus", [0.5, 0.0]);
            let trim_start = scalar(
                &mut document,
                "trim start",
                -0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let trim_end = scalar(
                &mut document,
                "trim end",
                0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let parabola = document
                .add_curve(
                    "parabola",
                    CurveDefinition::ParabolaSegment {
                        vertex: parabola_vertex,
                        focus: parabola_focus,
                        trim_start,
                        trim_end,
                    },
                )
                .unwrap();

            let hyperbola_center = point(&mut document, "hyperbola center", [0.0, 0.0]);
            let hyperbola_axis = point(&mut document, "hyperbola axis", [2.0, 0.0]);
            let semi_conjugate = scalar(
                &mut document,
                "semi conjugate",
                scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            );
            let hyperbola_trim_start = scalar(
                &mut document,
                "hyperbola trim start",
                -0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let hyperbola_trim_end = scalar(
                &mut document,
                "hyperbola trim end",
                0.6,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
            );
            let hyperbola = document
                .add_curve(
                    "hyperbola",
                    CurveDefinition::HyperbolaSegment {
                        center: hyperbola_center,
                        transverse_axis_point: hyperbola_axis,
                        semi_conjugate,
                        branch: DocumentHyperbolaBranch::Positive,
                        trim_start: hyperbola_trim_start,
                        trim_end: hyperbola_trim_end,
                    },
                )
                .unwrap();

            let quadratic_controls = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]
                .map(|position| point(&mut document, "quadratic control", position));
            let quadratic = document
                .add_curve(
                    "quadratic",
                    CurveDefinition::QuadraticBezier {
                        controls: quadratic_controls,
                    },
                )
                .unwrap();
            let cubic_controls = [[0.0, 0.0], [0.7, 1.0], [1.3, -0.5], [2.0, 0.2]]
                .map(|position| point(&mut document, "cubic control", position));
            let cubic = document
                .add_curve(
                    "cubic",
                    CurveDefinition::CubicBezier {
                        controls: cubic_controls,
                    },
                )
                .unwrap();

            let spline_controls = [[0.0, 0.0], [1.0, 0.8], [2.0, 0.0]]
                .map(|position| point(&mut document, "spline control", position));
            let spline = document
                .add_curve(
                    "spline",
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Clamped,
                        degree: 2,
                        controls: spline_controls.to_vec(),
                        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap();
            let nurbs_weights = [1.0, 0.75, 1.0].map(|weight| {
                scalar(
                    &mut document,
                    "NURBS weight",
                    weight,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
            });
            let nurbs_controls = [[0.0, 0.0], [1.0, 0.8], [2.0, 0.0]]
                .map(|position| point(&mut document, "NURBS control", position));
            let nurbs = document
                .add_curve(
                    "NURBS",
                    CurveDefinition::Nurbs {
                        form: DocumentBSplineForm::Clamped,
                        degree: 2,
                        controls: nurbs_controls.to_vec(),
                        weights: nurbs_weights.to_vec(),
                        gauge_weight: nurbs_weights[0],
                        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                        span_ids: vec![0],
                        next_span_id: 1,
                    },
                )
                .unwrap();

            let options = CurveOffsetOptions::for_model_scale(document.model_scale());
            for (family_index, (label, curve)) in [
                ("ellipse", ellipse),
                ("elliptical arc", elliptical_arc),
                ("rational quadratic", rational),
                ("parabola", parabola),
                ("hyperbola", hyperbola),
                ("quadratic Bezier", quadratic),
                ("cubic Bezier", cubic),
                ("B-spline", spline),
                ("NURBS", nurbs),
            ]
            .into_iter()
            .enumerate()
            {
                let traversal = if (family_index + scale_index) % 3 == 0 {
                    CurveOffsetTraversal::Reverse
                } else {
                    CurveOffsetTraversal::Forward
                };
                let signed_distance = if (family_index + scale_index) % 2 == 0 {
                    0.05 * scale
                } else {
                    -0.05 * scale
                };
                let result = compute_curve_offset(
                    &document,
                    CurveSpan::line(curve),
                    traversal,
                    signed_distance,
                    options,
                )
                .unwrap_or_else(|error| {
                    panic!("{label} offset failed at scale {scale}: {error:?}")
                });
                let CurveOffsetGeometry::CubicPatches(patches) = result.geometry else {
                    panic!("{label} must use certified cubic output");
                };
                assert!(!patches.is_empty(), "{label}");
                assert!(
                    patches
                        .iter()
                        .flat_map(|patch| patch.controls.iter().flatten())
                        .all(|value| value.is_finite()),
                    "{label}"
                );
                assert_independent_patch_samples(
                    &document,
                    CurveSpan::line(curve),
                    signed_distance,
                    &patches,
                    label,
                );
                assert!(result.certificate.maximum_position_error <= options.position_tolerance);
                assert!(
                    result.certificate.maximum_tangent_error_radians
                        <= options.tangent_tolerance_radians
                );
                assert!(result.certificate.minimum_regularity_factor > options.regularity_margin);
            }
        }
    }

    #[test]
    fn ellipse_inward_offset_at_minimum_radius_rejects_a_cusp() {
        let (document, span) = ellipse_document();
        let error = compute_curve_offset(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            1.0,
            CurveOffsetOptions::for_model_scale(document.model_scale()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CurveOffsetError::OffsetCusp | CurveOffsetError::IrregularSource
        ));
    }

    #[test]
    fn patch_limit_rejects_without_a_coarse_fallback() {
        let (document, span) = ellipse_document();
        let mut options = CurveOffsetOptions::for_model_scale(document.model_scale());
        options.max_patches = 1;
        let error = compute_curve_offset(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            -0.25,
            options,
        )
        .unwrap_err();
        assert_eq!(error, CurveOffsetError::PatchLimitExceeded(1));
    }

    #[test]
    fn m82_f003_control_stops_before_curve_fitting_starts() {
        let (document, span) = ellipse_document();
        let mut options = CurveOffsetOptions::for_model_scale(document.model_scale());
        options.max_patches = 1;
        assert_eq!(
            compute_curve_offset(
                &document,
                span,
                CurveOffsetTraversal::Forward,
                -0.25,
                options,
            )
            .unwrap_err(),
            CurveOffsetError::PatchLimitExceeded(1),
            "the fixture must enter adaptive fitting when work is authorized"
        );

        let mut limits = geosolve_core::OperationLimits::unlimited();
        limits.profile_subdivisions = 0;
        let mut exhausted =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                geosolve_core::CancellationToken::default(),
                limits,
            ));
        assert!(
            compute_curve_offset_with_controller(
                &document,
                span,
                CurveOffsetTraversal::Forward,
                -0.25,
                options,
                &mut exhausted,
            )
            .unwrap()
            .is_none(),
            "zero work must stop before the adaptive fitter can report its patch-limit error"
        );
        let report = exhausted.report();
        assert_eq!(report.consumed.profile_subdivisions, 0);
        assert_eq!(
            report.stopping_reason,
            Some(geosolve_core::OperationStopReason::WorkExhausted {
                counter: geosolve_core::OperationWorkCounter::ProfileSubdivisions,
                checkpoint: geosolve_core::OperationCheckpoint::ProfileSubdivision,
            })
        );

        let (handle, token) = geosolve_core::cancellation_pair();
        handle.cancel();
        let mut cancelled =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                token,
                geosolve_core::OperationLimits::unlimited(),
            ));
        assert!(
            compute_curve_offset_with_controller(
                &document,
                span,
                CurveOffsetTraversal::Forward,
                -0.25,
                options,
                &mut cancelled,
            )
            .unwrap()
            .is_none(),
            "pre-existing cancellation must stop before fitting starts"
        );
        let report = cancelled.report();
        assert_eq!(report.consumed.profile_subdivisions, 0);
        assert_eq!(
            report.stopping_reason,
            Some(geosolve_core::OperationStopReason::Cancelled {
                checkpoint: geosolve_core::OperationCheckpoint::ProfileSubdivision,
            })
        );
    }

    #[test]
    fn m82_f003_subdivision_exhaustion_stops_before_recursive_child_work() {
        let (document, span) = ellipse_document();
        let options = CurveOffsetOptions::for_model_scale(document.model_scale());
        let unrestricted = compute_curve_offset(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            -0.25,
            options,
        )
        .unwrap();
        assert!(
            unrestricted.certificate.subdivision_count > 0,
            "the fixture must require at least one adaptive subdivision"
        );

        let mut unlimited =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::unlimited());
        let unlimited_result = compute_curve_offset_with_controller(
            &document,
            span,
            CurveOffsetTraversal::Forward,
            -0.25,
            options,
            &mut unlimited,
        )
        .unwrap()
        .expect("unlimited controlled fitting must complete");
        assert_eq!(unlimited_result, unrestricted);
        assert_eq!(
            unlimited.report().consumed.profile_subdivisions,
            unlimited_result.certificate.subdivision_count + 1,
            "completed accounting must remain one span plus every actual subdivision"
        );

        let mut limits = geosolve_core::OperationLimits::unlimited();
        limits.profile_subdivisions = 1;
        let mut controller =
            geosolve_core::OperationController::new(geosolve_core::OperationControl::new(
                geosolve_core::CancellationToken::default(),
                limits,
            ));
        assert!(
            compute_curve_offset_with_controller(
                &document,
                span,
                CurveOffsetTraversal::Forward,
                -0.25,
                options,
                &mut controller,
            )
            .unwrap()
            .is_none()
        );
        let report = controller.report();
        assert_eq!(
            report.consumed.profile_subdivisions, 1,
            "the initial span charge must be retained, while the rejected subdivision and its children do no work"
        );
        assert_eq!(
            report.stopping_reason,
            Some(geosolve_core::OperationStopReason::WorkExhausted {
                counter: geosolve_core::OperationWorkCounter::ProfileSubdivisions,
                checkpoint: geosolve_core::OperationCheckpoint::ProfileSubdivision,
            })
        );
    }
}
