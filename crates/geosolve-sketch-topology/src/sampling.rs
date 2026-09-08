// SPDX-License-Identifier: GPL-3.0-or-later

//! Model-space polygonization of authenticated production fragments, never render tessellation.

use geosolve_sketch::{CurveDefinition, RetainedSketchDocumentSession, SketchDocument};
use thiserror::Error;

use crate::{
    TopologyOrientation, TopologyProductionProfile, TopologySourceProvenance, TopologyWire,
};

const MAX_VERTICES: usize = 65_536;
const MAX_EDGE_TESTS: usize = 2_000_000;

/// File/query-local region with implicitly closed, CCW outer and CW hole loops.
#[derive(Clone, Debug, PartialEq)]
pub struct SampledTopologyRegion {
    pub id: String,
    pub outer: Vec<[f64; 2]>,
    pub holes: Vec<Vec<[f64; 2]>>,
}

/// Sampling never publishes a partial or uncertified polygon set.
#[derive(Debug, Error)]
pub enum TopologySamplingError {
    #[error("production polygon sampling: {0}")]
    Rejected(String),
}

fn rejected(message: impl Into<String>) -> TopologySamplingError {
    TopologySamplingError::Rejected(message.into())
}

impl TopologyProductionProfile {
    /// Samples complete production wires from the exact current accepted document.
    ///
    /// Coordinates/error are in document model units. Lines (including polyline spans),
    /// circles and circular arcs are supported; other families fail explicitly. Circular
    /// fragment subdivisions use the conservative sagitta bound `r * angle² / 8`, with
    /// separate allowance for evaluated endpoint joins and floating-point roundoff.
    /// This bounds source-curve polygonization, not downstream Boolean/manufacturing error.
    ///
    /// # Errors
    /// Rejects stale/empty/unsupported input, unrepresentable error targets, exceeded work
    /// limits, nonfinite samples, broken joins or invalid sampled polygon topology.
    pub fn sample_polygons(
        &self,
        session: &RetainedSketchDocumentSession,
        max_chord_error: f64,
    ) -> Result<Vec<SampledTopologyRegion>, TopologySamplingError> {
        if !max_chord_error.is_finite() || max_chord_error <= 0.0 {
            return Err(rejected("max_chord_error must be finite and positive"));
        }
        self.validate_current(session)
            .map_err(|error| rejected(error.to_string()))?;
        if self.regions().is_empty() {
            return Err(rejected("no complete bounded production regions"));
        }
        let document = session
            .accepted_state_for_current_input()
            .ok_or_else(|| rejected("current accepted geometry required"))?
            .document();
        let mut vertices = 0;
        let mut edge_tests = 0;
        let mut result = Vec::new();
        for region in self.regions() {
            let wire = |id| {
                self.wires()
                    .iter()
                    .find(|wire| wire.id == id)
                    .ok_or_else(|| rejected("production region refers to a missing wire"))
            };
            let outer = sample_wire(
                document,
                wire(region.outer)?,
                max_chord_error,
                &mut vertices,
            )?;
            let holes = region
                .holes
                .iter()
                .map(|id| sample_wire(document, wire(*id)?, max_chord_error, &mut vertices))
                .collect::<Result<Vec<_>, _>>()?;
            validate_loop(&outer, true, &mut edge_tests)?;
            for (index, hole) in holes.iter().enumerate() {
                validate_loop(hole, false, &mut edge_tests)?;
                validate_separate(&outer, hole, &mut edge_tests)?;
                if !contains(&outer, hole[0]) {
                    return Err(rejected("sampled hole is outside its outer loop"));
                }
                for other in &holes[..index] {
                    validate_separate(hole, other, &mut edge_tests)?;
                    if contains(hole, other[0]) || contains(other, hole[0]) {
                        return Err(rejected("sampled holes overlap or nest"));
                    }
                }
            }
            result.push(SampledTopologyRegion {
                id: format!("region-{}", region.id.0),
                outer,
                holes,
            });
        }
        self.validate_current(session)
            .map_err(|error| rejected(error.to_string()))?;
        Ok(result)
    }
}

fn sample_wire(
    document: &SketchDocument,
    wire: &TopologyWire,
    error: f64,
    vertices: &mut usize,
) -> Result<Vec<[f64; 2]>, TopologySamplingError> {
    let mut points = Vec::new();
    for (index, fragment) in wire.fragments.iter().enumerate() {
        let TopologySourceProvenance::Native { support, .. } = fragment.source else {
            return Err(rejected("external source fragments are not supported"));
        };
        let curve = document
            .curve(support.curve)
            .ok_or_else(|| rejected("missing accepted curve"))?;
        let evaluate = |parameter| {
            document
                .evaluate_curve_jet(support, parameter)
                .map_err(|error| rejected(format!("curve {}: {error}", curve.label)))
        };
        let [start, end] = fragment.source_parameters;
        let end_jet = evaluate(end)?;
        let next = &wire.fragments[(index + 1) % wire.fragments.len()];
        let TopologySourceProvenance::Native {
            support: next_support,
            ..
        } = next.source
        else {
            return Err(rejected("external source fragments are not supported"));
        };
        let next_jet = document
            .evaluate_curve_jet(next_support, next.source_parameters[0])
            .map_err(|error| rejected(error.to_string()))?;
        let gap = (end_jet.position - next_jet.position).norm();
        if !gap.is_finite() || gap > error / 8.0 {
            return Err(rejected(
                "fragment endpoint join exceeds the requested sampling budget",
            ));
        }
        let steps = fragment_steps(document, fragment, error)?;
        *vertices +=
            usize::try_from(steps).map_err(|_| rejected("sampling count is unrepresentable"))?;
        if *vertices > MAX_VERTICES {
            return Err(rejected("sampling vertex limit exceeded"));
        }
        for step in 0..steps {
            let parameter = (end - start).mul_add(f64::from(step) / f64::from(steps), start);
            let position = evaluate(parameter)?.position;
            let point = [position.x, position.y];
            if !point.iter().all(|value| value.is_finite()) {
                return Err(rejected("nonfinite sampled position"));
            }
            points.push(point);
        }
    }
    let area = signed_area(&points);
    if !area.is_finite()
        || area == 0.0
        || (area > 0.0) != (wire.orientation == TopologyOrientation::CounterClockwise)
    {
        return Err(rejected(
            "sampled wire lost its certified production orientation",
        ));
    }
    Ok(points)
}

fn fragment_steps(
    document: &SketchDocument,
    fragment: &crate::TopologyFragment,
    error: f64,
) -> Result<u32, TopologySamplingError> {
    let TopologySourceProvenance::Native { support, .. } = fragment.source else {
        return Err(rejected("external source fragments are not supported"));
    };
    let curve = document
        .curve(support.curve)
        .ok_or_else(|| rejected("missing accepted curve"))?;
    let [start, end] = fragment.source_parameters;
    let start_jet = document
        .evaluate_curve_jet(support, start)
        .map_err(|error| rejected(error.to_string()))?;
    match curve.definition {
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => Ok(1),
        CurveDefinition::Circle { center, radius }
        | CurveDefinition::CircularArc { center, radius, .. } => {
            let radius = document
                .scalar(radius)
                .ok_or_else(|| rejected("missing radius"))?
                .value;
            let center = document
                .point(center)
                .ok_or_else(|| rejected("missing center"))?
                .position;
            let scale = center[0].abs().max(center[1].abs()).max(radius).max(1.0);
            let mut angle_scale = start.abs().max(end.abs()).max(1.0);
            if let CurveDefinition::CircularArc {
                start_angle,
                end_angle,
                ..
            } = curve.definition
            {
                for id in [start_angle, end_angle] {
                    angle_scale = angle_scale.max(
                        document
                            .scalar(id)
                            .ok_or_else(|| rejected("missing arc angle"))?
                            .value
                            .abs(),
                    );
                }
            }
            let roundoff = scale * angle_scale * 256.0 * f64::EPSILON;
            if !radius.is_finite() || radius <= 0.0 || roundoff > error / 8.0 {
                return Err(rejected(
                    "requested circular chord error is below representable precision",
                ));
            }
            // Circles use unwrapped angle; bounded arcs use a linear [0,1] parameter.
            // The accepted evaluator owns their sweep/branch and constant parameter speed.
            let speed = start_jet.first_derivative.norm() / radius;
            let angle = (end - start).abs() * speed;
            let max_angle = (error / radius).sqrt().min(std::f64::consts::FRAC_PI_2);
            let count = (angle / max_angle).ceil().max(1.0);
            if !count.is_finite() || count > f64::from(u32::try_from(MAX_VERTICES).unwrap()) {
                return Err(rejected("sampling vertex limit exceeded"));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                Ok(count as u32)
            }
        }
        _ => Err(rejected(format!(
            "unsupported curve family on `{}`; bake supports lines, circles and circular arcs",
            curve.label
        ))),
    }
}

fn signed_area(points: &[[f64; 2]]) -> f64 {
    let Some(origin) = points.first() else {
        return 0.0;
    };
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| cross(*origin, *a, *b))
        .sum::<f64>()
        * 0.5
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]).mul_add(c[1] - a[1], -(b[1] - a[1]) * (c[0] - a[0]))
}

fn edge_test(
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
    d: [f64; 2],
    count: &mut usize,
) -> Result<(), TopologySamplingError> {
    *count += 1;
    if *count > MAX_EDGE_TESTS {
        return Err(rejected("polygon validation work limit exceeded"));
    }
    // Axis separation is exact for the represented polygon coordinates.
    if (0..2).any(|axis| {
        a[axis].max(b[axis]) < c[axis].min(d[axis]) || c[axis].max(d[axis]) < a[axis].min(b[axis])
    }) {
        return Ok(());
    }
    let turns = [
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    ];
    if !turns.iter().all(|value| value.is_finite()) {
        return Err(rejected("unrepresentable polygon intersection test"));
    }
    if (turns[0].signum() != turns[1].signum() || turns[0] == 0.0 || turns[1] == 0.0)
        && (turns[2].signum() != turns[3].signum() || turns[2] == 0.0 || turns[3] == 0.0)
    {
        return Err(rejected(
            "sampled polygon edges intersect or touch; use a smaller chord error or repair the source",
        ));
    }
    Ok(())
}

#[allow(clippy::float_cmp)] // Reject exactly coincident represented vertices, not short valid edges.
fn validate_loop(
    points: &[[f64; 2]],
    ccw: bool,
    count: &mut usize,
) -> Result<(), TopologySamplingError> {
    let area = signed_area(points);
    if points.len() < 3 || !area.is_finite() || area == 0.0 || (area > 0.0) != ccw {
        return Err(rejected("invalid polygon loop size or winding"));
    }
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        if points[index] == points[next] {
            return Err(rejected("polygon has a zero-length edge"));
        }
        for other in index + 1..points.len() {
            let after = (other + 1) % points.len();
            if other != next && after != index {
                edge_test(
                    points[index],
                    points[next],
                    points[other],
                    points[after],
                    count,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_separate(
    a: &[[f64; 2]],
    b: &[[f64; 2]],
    count: &mut usize,
) -> Result<(), TopologySamplingError> {
    for i in 0..a.len() {
        for j in 0..b.len() {
            edge_test(
                a[i],
                a[(i + 1) % a.len()],
                b[j],
                b[(j + 1) % b.len()],
                count,
            )?;
        }
    }
    Ok(())
}

fn contains(points: &[[f64; 2]], point: [f64; 2]) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * ((point[1] - a[1]) / (b[1] - a[1])) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
