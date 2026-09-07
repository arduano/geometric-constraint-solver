// SPDX-License-Identifier: GPL-3.0-or-later

//! Finite line/arc boundary qualification after native and computed composition.
//! This module creates no equations or geometry and uses no presentation samples.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{CurveDefinition, CurveSpan, DocumentArcSweep, SketchDocument};
use geosolve_sketch_features::{ComputedCircularArc, ComputedEdgeGeometry, NativeCurveSpanSource};
use geosolve_sketch_intent::IntentAliasMap;

use super::{CodeCompositionError, CodeHostRequest, expect_span};

const MAX_BOUNDARY_EDGES: usize = 512;
type Point = [f64; 2];

#[derive(Clone, Copy, Debug)]
enum Shape {
    Line,
    Arc {
        center: Point,
        radius: f64,
        start_angle: f64,
        sweep: f64,
    },
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    start: Point,
    end: Point,
    shape: Shape,
}

pub(super) fn validate(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    requests: &[CodeHostRequest],
) -> Result<(), CodeCompositionError> {
    for request in requests {
        let CodeHostRequest::ChannelBoundaryCheck {
            output,
            supports,
            expected_components,
            expected_open_ends,
            suppressed,
            ..
        } = request
        else {
            continue;
        };
        if *suppressed {
            continue;
        }
        let failure = |diagnostic: String| CodeCompositionError::InvalidChannelBoundary {
            member: output.display_path(),
            diagnostic,
        };
        if supports.is_empty() || supports.len() > MAX_BOUNDARY_EDGES {
            return Err(failure(
                "native support count is outside the channel bound".into(),
            ));
        }
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or(CodeCompositionError::BaseNotAccepted)?;
        // Native fragments and computed contacts must use the same current
        // accepted solve. Retained design coordinates remain initialization
        // seeds after dimension edits and cannot qualify the published boundary.
        let state = accepted
            .session
            .accepted_state_for_current_input()
            .ok_or(CodeCompositionError::BaseNotAccepted)?;
        let document = state.document();
        let mut sources = BTreeSet::new();
        for support in supports {
            let reference = aliases
                .port(&support.alias, support.selector)
                .ok_or_else(|| failure("channel support alias is unavailable".into()))?;
            let binding = accepted
                .ownership
                .port(reference)
                .ok_or_else(|| failure("channel support has no accepted native binding".into()))?;
            let span = expect_span(binding, &support.alias.to_string())?;
            if !sources.insert(NativeCurveSpanSource { span }) {
                return Err(failure("channel repeats a native boundary support".into()));
            }
        }
        let mut edges = Vec::new();
        for source in &sources {
            if accepted.computed.replaced_sources().contains(source) {
                for edge in accepted.computed.source_fragment_edges(*source) {
                    let ComputedEdgeGeometry::NativeSourceFragment { source, interval } =
                        edge.geometry
                    else {
                        return Err(failure(
                            "channel source replacement is not a native fragment".into(),
                        ));
                    };
                    edges.push(
                        native_edge(document, source.span, interval.start, interval.end)
                            .map_err(&failure)?,
                    );
                }
            } else {
                let intervals = document
                    .visible_intervals(source.span)
                    .map_err(|error| failure(error.to_string()))?;
                for interval in intervals {
                    edges.push(
                        native_edge(document, source.span, interval.start, interval.end)
                            .map_err(&failure)?,
                    );
                }
            }
        }
        for edge in accepted.computed.edges() {
            if let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry {
                let included = arc
                    .contacts
                    .map(|contact| sources.contains(&contact.source));
                if included == [true, true] {
                    edges.push(computed_edge(arc).map_err(&failure)?);
                } else if included.contains(&true) {
                    return Err(failure(
                        "channel Fillet also consumes an outside boundary".into(),
                    ));
                }
            }
        }
        qualify(
            &edges,
            usize::from(*expected_components),
            usize::from(*expected_open_ends),
        )
        .map_err(failure)?;
    }
    Ok(())
}

fn native_edge(
    document: &SketchDocument,
    span: CurveSpan,
    lower: f64,
    upper: f64,
) -> Result<Edge, String> {
    let at = |parameter| {
        document
            .evaluate_curve_jet(span, parameter)
            .map_err(|error| error.to_string())
    };
    let first = at(lower)?;
    let last = at(upper)?;
    let start = [first.position.x, first.position.y];
    let end = [last.position.x, last.position.y];
    let curve = document
        .curve(span.curve)
        .ok_or("channel support disappeared")?;
    let shape = match curve.definition {
        CurveDefinition::Line { .. } => Shape::Line,
        CurveDefinition::CircularArc { center, radius, .. } => {
            let center = document
                .point(center)
                .ok_or("channel arc center disappeared")?
                .position;
            let radius = document
                .scalar(radius)
                .ok_or("channel arc radius disappeared")?
                .value;
            let radial = subtract(start, center);
            let tangent = [first.first_derivative.x, first.first_derivative.y];
            let sweep = cross(radial, tangent).signum() * norm(tangent) * (upper - lower) / radius;
            Shape::Arc {
                center,
                radius,
                start_angle: radial[1].atan2(radial[0]),
                sweep,
            }
        }
        _ => {
            return Err("channel boundary must contain only finite lines and circular arcs".into());
        }
    };
    checked_edge(Edge { start, end, shape })
}

fn computed_edge(arc: &ComputedCircularArc) -> Result<Edge, String> {
    let sweep = match arc.sweep {
        DocumentArcSweep::CounterClockwise => (arc.end_angle - arc.start_angle).rem_euclid(TAU),
        DocumentArcSweep::Clockwise => -(arc.start_angle - arc.end_angle).rem_euclid(TAU),
    };
    let sample = |angle: f64| {
        [
            arc.center[0] + arc.radius * angle.cos(),
            arc.center[1] + arc.radius * angle.sin(),
        ]
    };
    checked_edge(Edge {
        start: sample(arc.start_angle),
        end: sample(arc.end_angle),
        shape: Shape::Arc {
            center: arc.center,
            radius: arc.radius,
            start_angle: arc.start_angle,
            sweep,
        },
    })
}

fn checked_edge(edge: Edge) -> Result<Edge, String> {
    if !edge.start.into_iter().chain(edge.end).all(f64::is_finite) {
        return Err("channel boundary contains non-finite endpoints".into());
    }
    match edge.shape {
        Shape::Line if norm(subtract(edge.end, edge.start)) > 0.0 => Ok(edge),
        Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
        } if center
            .into_iter()
            .chain([radius, start_angle, sweep])
            .all(f64::is_finite)
            && radius > 0.0
            && sweep.abs() > 0.0
            && sweep.abs() < TAU =>
        {
            Ok(edge)
        }
        _ => Err("channel boundary contains degenerate or non-finite geometry".into()),
    }
}

fn qualify(
    edges: &[Edge],
    expected_components: usize,
    expected_open_ends: usize,
) -> Result<(), String> {
    if edges.is_empty() || edges.len() > MAX_BOUNDARY_EDGES {
        return Err("composed boundary edge count is outside the channel bound".into());
    }
    let (edges, epsilon) = normalized_boundary(edges)?;
    qualify_normalized(&edges, epsilon, expected_components, expected_open_ends)
}

fn normalized_boundary(edges: &[Edge]) -> Result<(Vec<Edge>, f64), String> {
    let origin = edges[0].start;
    let extent = edges
        .iter()
        .map(|edge| {
            let chord_extent =
                norm(subtract(edge.start, origin)).max(norm(subtract(edge.end, origin)));
            match edge.shape {
                Shape::Line => chord_extent,
                Shape::Arc { center, radius, .. } => {
                    chord_extent.max(norm(subtract(center, origin))).max(radius)
                }
            }
        })
        .fold(0.0, f64::max);
    let magnitude = edges
        .iter()
        .flat_map(|edge| edge.start.into_iter().chain(edge.end))
        .map(f64::abs)
        .fold(0.0, f64::max);
    // Endpoint association must accommodate independently validated native/host
    // residual roundoff. Nearer-than-resolution non-endpoint contacts reject;
    // this never changes the solver's independent hard-residual tolerance.
    let epsilon = 1.0e-9_f64.max((magnitude / extent) * 64.0 * f64::EPSILON);
    if !extent.is_finite() || extent <= 0.0 || !epsilon.is_finite() || epsilon >= 1.0 {
        return Err("channel boundary exceeds finite geometric resolution".into());
    }
    let point = |point: Point| {
        [
            (point[0] - origin[0]) / extent,
            (point[1] - origin[1]) / extent,
        ]
    };
    let normalized = edges
        .iter()
        .map(|edge| {
            let shape = match edge.shape {
                Shape::Line => Shape::Line,
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                } => Shape::Arc {
                    center: point(center),
                    radius: radius / extent,
                    start_angle,
                    sweep,
                },
            };
            checked_edge(Edge {
                start: point(edge.start),
                end: point(edge.end),
                shape,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((normalized, epsilon))
}

fn qualify_normalized(
    edges: &[Edge],
    epsilon: f64,
    expected_components: usize,
    expected_open_ends: usize,
) -> Result<(), String> {
    let mut vertices: Vec<Point> = Vec::new();
    let mut incidence: Vec<Vec<usize>> = Vec::new();
    let mut ends = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        let mut pair = [0; 2];
        for (endpoint, point) in [edge.start, edge.end].into_iter().enumerate() {
            let matching = vertices
                .iter()
                .enumerate()
                .filter(|(_, candidate)| near(**candidate, point, epsilon))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let vertex = match matching.as_slice() {
                [] => {
                    vertices.push(point);
                    incidence.push(Vec::new());
                    vertices.len() - 1
                }
                [vertex] => *vertex,
                _ => return Err("channel endpoint association is ambiguous".into()),
            };
            incidence[vertex].push(index);
            pair[endpoint] = vertex;
        }
        if pair[0] == pair[1] {
            return Err("channel edge collapses within boundary resolution".into());
        }
        ends.push(pair);
    }
    if incidence.iter().any(|edges| edges.len() > 2) {
        return Err("channel boundary branches or touches itself".into());
    }
    let open_ends = incidence.iter().filter(|edges| edges.len() == 1).count();
    if open_ends != expected_open_ends {
        return Err(format!(
            "channel boundary has {open_ends} open ends, expected {expected_open_ends}"
        ));
    }
    let mut visited = BTreeSet::new();
    let mut components = 0;
    for index in 0..edges.len() {
        if visited.contains(&index) {
            continue;
        }
        components += 1;
        let mut pending = vec![index];
        while let Some(index) = pending.pop() {
            if !visited.insert(index) {
                continue;
            }
            for endpoint in ends[index] {
                pending.extend(incidence[endpoint].iter().copied());
            }
        }
    }
    if components != expected_components {
        return Err(format!(
            "channel boundary has {components} components, expected {expected_components}"
        ));
    }
    for (index, first) in edges.iter().enumerate() {
        for second in &edges[index + 1..] {
            intersections(*first, *second, epsilon)?;
        }
    }
    Ok(())
}

fn intersections(first: Edge, second: Edge, epsilon: f64) -> Result<(), String> {
    match (first.shape, second.shape) {
        (Shape::Line, Shape::Line) => line_line(first, second, epsilon),
        (Shape::Line, Shape::Arc { .. }) => line_arc(first, second, epsilon),
        (Shape::Arc { .. }, Shape::Line) => line_arc(second, first, epsilon),
        (Shape::Arc { .. }, Shape::Arc { .. }) => arc_arc(first, second, epsilon),
    }
}

fn contact(first: Edge, second: Edge, point: Point, epsilon: f64) -> Result<(), String> {
    let endpoint = |edge: Edge| near(point, edge.start, epsilon) || near(point, edge.end, epsilon);
    if endpoint(first) && endpoint(second) {
        Ok(())
    } else {
        Err("channel boundary intersects or touches itself away from a shared endpoint".into())
    }
}

fn line_line(first: Edge, second: Edge, epsilon: f64) -> Result<(), String> {
    let a = subtract(first.end, first.start);
    let b = subtract(second.end, second.start);
    let delta = subtract(second.start, first.start);
    let denominator = cross(a, b);
    if denominator == 0.0 {
        if cross(delta, a).abs() > epsilon * norm(a) {
            return Ok(());
        }
        let length = norm(a);
        let unit = scale(a, 1.0 / length);
        let t0 = dot(delta, unit);
        let t1 = dot(subtract(second.end, first.start), unit);
        let lower = 0.0_f64.max(t0.min(t1));
        let upper = length.min(t0.max(t1));
        if upper < lower - epsilon {
            return Ok(());
        }
        if upper > lower + epsilon {
            return Err("channel boundary contains overlapping line fragments".into());
        }
        contact(
            first,
            second,
            add(first.start, scale(unit, lower.midpoint(upper))),
            epsilon,
        )
    } else {
        let t = cross(delta, b) / denominator;
        let u = cross(delta, a) / denominator;
        if t < -epsilon / norm(a)
            || t > 1.0 + epsilon / norm(a)
            || u < -epsilon / norm(b)
            || u > 1.0 + epsilon / norm(b)
        {
            return Ok(());
        }
        contact(first, second, add(first.start, scale(a, t)), epsilon)
    }
}

fn line_arc(line: Edge, arc: Edge, epsilon: f64) -> Result<(), String> {
    let Shape::Arc { center, radius, .. } = arc.shape else {
        unreachable!()
    };
    let vector = subtract(line.end, line.start);
    let length = norm(vector);
    let unit = scale(vector, 1.0 / length);
    let delta = subtract(center, line.start);
    let along = dot(delta, unit);
    let perpendicular = cross(delta, unit).abs();
    if perpendicular > radius + epsilon {
        return Ok(());
    }
    let squared = radius * radius - perpendicular * perpendicular;
    let distance = squared.max(0.0).sqrt();
    for t in [along - distance, along + distance] {
        if t >= -epsilon && t <= length + epsilon {
            let point = add(line.start, scale(unit, t));
            if on_arc(arc, point, epsilon) {
                contact(line, arc, point, epsilon)?;
            }
        }
    }
    Ok(())
}

fn arc_arc(first: Edge, second: Edge, epsilon: f64) -> Result<(), String> {
    let Shape::Arc {
        center: a,
        radius: ra,
        ..
    } = first.shape
    else {
        unreachable!()
    };
    let Shape::Arc {
        center: b,
        radius: rb,
        ..
    } = second.shape
    else {
        unreachable!()
    };
    let delta = subtract(b, a);
    let distance = norm(delta);
    if distance <= epsilon && (ra - rb).abs() <= epsilon {
        for point in [
            first.start,
            first.end,
            arc_midpoint(first),
            second.start,
            second.end,
            arc_midpoint(second),
        ] {
            if on_arc(first, point, epsilon) && on_arc(second, point, epsilon) {
                contact(first, second, point, epsilon)?;
            }
        }
        return Ok(());
    }
    if distance > ra + rb + epsilon || distance < (ra - rb).abs() - epsilon || distance == 0.0 {
        return Ok(());
    }
    let along = (ra * ra - rb * rb + distance * distance) / (2.0 * distance);
    let height = (ra * ra - along * along).max(0.0).sqrt();
    let unit = scale(delta, 1.0 / distance);
    let base = add(a, scale(unit, along));
    for sign in [-1.0, 1.0] {
        let point = add(base, scale([-unit[1], unit[0]], sign * height));
        if on_arc(first, point, epsilon) && on_arc(second, point, epsilon) {
            contact(first, second, point, epsilon)?;
        }
    }
    Ok(())
}

fn on_arc(arc: Edge, point: Point, epsilon: f64) -> bool {
    let Shape::Arc {
        center,
        radius,
        start_angle,
        sweep,
    } = arc.shape
    else {
        unreachable!()
    };
    let radial = subtract(point, center);
    if (norm(radial) - radius).abs() > epsilon {
        return false;
    }
    if near(point, arc.start, epsilon) || near(point, arc.end, epsilon) {
        return true;
    }
    let angle = radial[1].atan2(radial[0]);
    let travelled = if sweep > 0.0 {
        (angle - start_angle).rem_euclid(TAU)
    } else {
        (start_angle - angle).rem_euclid(TAU)
    };
    travelled <= sweep.abs() + epsilon / radius
}

fn arc_midpoint(arc: Edge) -> Point {
    let Shape::Arc {
        center,
        radius,
        start_angle,
        sweep,
    } = arc.shape
    else {
        unreachable!()
    };
    let angle = start_angle + sweep / 2.0;
    add(center, [radius * angle.cos(), radius * angle.sin()])
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
fn subtract(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale(a: Point, value: f64) -> Point {
    [a[0] * value, a[1] * value]
}
fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn norm(a: Point) -> f64 {
    a[0].hypot(a[1])
}
fn near(a: Point, b: Point, epsilon: f64) -> bool {
    norm(subtract(a, b)) <= epsilon
}

#[cfg(test)]
mod tests {
    use super::{Edge, Shape, intersections, qualify};
    use std::f64::consts::PI;

    fn line(start: [f64; 2], end: [f64; 2]) -> Edge {
        Edge {
            start,
            end,
            shape: Shape::Line,
        }
    }
    fn arc(center: [f64; 2], radius: f64, start_angle: f64, sweep: f64) -> Edge {
        let at = |angle: f64| {
            [
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]
        };
        Edge {
            start: at(start_angle),
            end: at(start_angle + sweep),
            shape: Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
            },
        }
    }

    #[test]
    fn exact_intersections_reject_crossings_touches_and_overlaps() {
        let cases = [
            (
                line([0.0, 0.0], [10.0, 10.0]),
                line([0.0, 10.0], [10.0, 0.0]),
            ),
            (line([0.0, 0.0], [10.0, 0.0]), line([5.0, 0.0], [15.0, 0.0])),
            (line([0.0, 3.0], [25.0, 3.0]), arc([5.0, 5.0], 3.0, PI, PI)),
            (line([-5.0, 3.0], [5.0, 3.0]), arc([0.0, 0.0], 3.0, 0.0, PI)),
            (arc([0.0, 0.0], 3.0, 0.0, PI), arc([4.0, 0.0], 3.0, 0.0, PI)),
            (
                arc([0.0, 0.0], 3.0, 0.0, PI),
                arc([0.0, 0.0], 3.0, PI / 2.0, PI),
            ),
        ];
        for (first, second) in cases {
            assert!(
                intersections(first, second, 1.0e-9).is_err(),
                "{first:?}, {second:?}"
            );
            assert!(
                intersections(second, first, 1.0e-9).is_err(),
                "symmetric pair"
            );
        }
    }

    #[test]
    fn endpoint_joins_and_separate_nested_arc_boundaries_remain_valid() {
        assert!(
            intersections(
                line([0.0, 0.0], [3.0, 0.0]),
                arc([3.0, 3.0], 3.0, -PI / 2.0, PI / 2.0),
                1.0e-9
            )
            .is_ok()
        );
        assert!(
            intersections(
                arc([0.0, 0.0], 2.0, 0.0, PI),
                arc([0.0, 0.0], 3.0, 0.0, PI),
                1.0e-9
            )
            .is_ok()
        );
        let capsule = [
            line([0.0, 3.0], [20.0, 3.0]),
            arc([20.0, 0.0], 3.0, -PI / 2.0, PI),
            line([0.0, -3.0], [20.0, -3.0]),
            arc([0.0, 0.0], 3.0, PI / 2.0, PI),
        ];
        for factor in [1.0e-6, 1.0, 1.0e6, 1.0e100] {
            let transformed = capsule.map(|edge| {
                let point =
                    |point: [f64; 2]| [(point[0] + 100.0) * factor, (point[1] - 80.0) * factor];
                let shape = match edge.shape {
                    Shape::Line => Shape::Line,
                    Shape::Arc {
                        center,
                        radius,
                        start_angle,
                        sweep,
                    } => Shape::Arc {
                        center: point(center),
                        radius: radius * factor,
                        start_angle,
                        sweep,
                    },
                };
                Edge {
                    start: point(edge.start),
                    end: point(edge.end),
                    shape,
                }
            });
            assert!(qualify(&transformed, 1, 0).is_ok(), "scale {factor}");
        }
        assert!(qualify(&capsule, 2, 0).is_err());
        assert!(qualify(&capsule[..3], 1, 0).is_err());
        assert!(qualify(&capsule[..3], 1, 2).is_ok());
    }
}
