#![allow(clippy::many_single_char_names, clippy::too_many_lines)]

use std::f64::consts::{FRAC_PI_2, PI};

use geosolve_core::{HardValidity, SolveReport, SolveTermination, SolverConfig, VariableValue};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    AngleOrientation, ArcSweep, CENTER_DIRECTION_COSINE_MARGIN, CenterDirectionBranch,
    CircleContainment, CircleTangencyMode, ContactState, DimensionMode, LatentVariableRole,
    LineParameterDomain, LineSide, Sketch, SketchError, SketchSolveRequest, SketchSource,
    SolveRejection,
};

const TOLERANCE: f64 = 1.0e-9;

fn solve(sketch: &mut Sketch) -> geosolve_sketch::SketchSolveResult {
    sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap()
}

fn solve_with(sketch: &mut Sketch, config: SolverConfig) -> geosolve_sketch::SketchSolveResult {
    sketch.solve(SketchSolveRequest::default(), config).unwrap()
}

fn assert_accepted(result: &geosolve_sketch::SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(
        result.unstable_core_report().termination,
        SolveTermination::Converged
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert!(result.unstable_core_report().hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
}

fn transform(point: Point2<f64>, scale: f64, angle: f64, offset: [f64; 2]) -> Point2<f64> {
    let (sine, cosine) = angle.sin_cos();
    Point2::new(
        scale * (cosine * point.x - sine * point.y) + offset[0],
        scale * (sine * point.x + cosine * point.y) + offset[1],
    )
}

fn unit(first: Point2<f64>, second: Point2<f64>) -> [f64; 2] {
    let direction = second - first;
    let length = direction.norm();
    [direction.x / length, direction.y / length]
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

fn assert_fd(sketch: &Sketch) {
    let compiled = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let report = compiled.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        report.all_within(1.0e-6),
        "max FD error={:e}: {report:#?}",
        report.max_relative_error()
    );
}

#[allow(clippy::float_cmp)]
fn assert_complete_analysis_matches(actual: &SolveReport, direct: &SolveReport) {
    assert_eq!(actual.termination, direct.termination);
    assert_eq!(actual.accepted_state, direct.accepted_state);
    assert_eq!(
        actual.hard_residuals_validated,
        direct.hard_residuals_validated
    );
    assert_eq!(actual.hard_residual_max, direct.hard_residual_max);
    assert_eq!(actual.hard_residual_l2, direct.hard_residual_l2);
    assert_eq!(actual.rank_is_valid, direct.rank_is_valid);
    assert_eq!(actual.rank, direct.rank);
    assert_eq!(
        actual.local_degrees_of_freedom,
        direct.local_degrees_of_freedom
    );
    assert_eq!(actual.is_singular, direct.is_singular);
    assert_eq!(actual.rank_threshold, direct.rank_threshold);
    assert_eq!(actual.singular_values, direct.singular_values);
    assert_eq!(actual.conflicting_sources, direct.conflicting_sources);
    assert_eq!(actual.redundant_sources, direct.redundant_sources);
    assert_eq!(
        actual.sources_containing_redundant_rows,
        direct.sources_containing_redundant_rows
    );
    assert_eq!(actual.redundant_rows, direct.redundant_rows);
    assert_eq!(actual.singular_rows, direct.singular_rows);
    assert_eq!(actual.structural, direct.structural);
    assert_eq!(actual.audit, direct.audit);
    assert_eq!(actual.component_solves.len(), direct.component_solves.len());
    for (actual, direct) in actual.component_solves.iter().zip(&direct.component_solves) {
        assert_eq!(actual.component_index, direct.component_index);
        assert_eq!(actual.pattern_signature, direct.pattern_signature);
        assert_eq!(actual.termination, direct.termination);
        assert_eq!(
            actual.hard_residuals_validated,
            direct.hard_residuals_validated
        );
        assert_eq!(actual.hard_residual_max, direct.hard_residual_max);
        assert_eq!(actual.rank_is_valid, direct.rank_is_valid);
        assert_eq!(actual.rank, direct.rank);
        assert_eq!(
            actual.local_degrees_of_freedom,
            direct.local_degrees_of_freedom
        );
        assert_eq!(actual.is_singular, direct.is_singular);
        assert_eq!(actual.rank_threshold, direct.rank_threshold);
        assert_eq!(actual.singular_values, direct.singular_values);
    }
}

#[test]
fn normalized_direction_rows_reject_short_nonzero_false_roots_and_reparameterize() {
    let angular_error = 1.0e-4;
    let short = 1.0e-12;

    for (perpendicular, angle) in [(false, angular_error), (true, FRAC_PI_2 + angular_error)] {
        let mut sketch = Sketch::new(1.0).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(short, 0.0)).unwrap();
        let c = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let d = sketch
            .add_point(Point2::new(short * angle.cos(), short * angle.sin()))
            .unwrap();
        let first = sketch.add_segment(a, b).unwrap();
        let second = sketch.add_segment(c, d).unwrap();
        for point in [a, b, c, d] {
            sketch.add_fixed_point(point).unwrap();
        }
        let constraint = if perpendicular {
            sketch.add_perpendicular(first, second).unwrap()
        } else {
            sketch.add_parallel(first, second).unwrap()
        };
        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(constraint))
            .unwrap();
        let row = compiled
            .problem()
            .audit_rows()
            .unwrap()
            .into_iter()
            .find(|row| Some(row.source_id) == mapping.core_source_id)
            .unwrap();
        assert!((row.scale - 1.0).abs() <= f64::EPSILON);
        assert!(row.template.contains("unit_direction"));
        let result = solve(&mut sketch);
        assert!(!result.accepted());
        assert!(result.unstable_core_report().hard_residual_max > TOLERANCE);
    }

    for length in [1.0e-12, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(1.0).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(length, 0.0)).unwrap();
        let c = sketch.add_point(Point2::new(-3.0 * length, 2.0)).unwrap();
        let d = sketch.add_point(Point2::new(5.0 * length, 2.0)).unwrap();
        let first = sketch.add_segment(a, b).unwrap();
        let second = sketch.add_segment(c, d).unwrap();
        for point in [a, b, c, d] {
            sketch.add_fixed_point(point).unwrap();
        }
        sketch.add_parallel(first, second).unwrap();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        assert!(
            cross(
                unit(a_position(&result, a), a_position(&result, b)),
                unit(a_position(&result, c), a_position(&result, d))
            )
            .abs()
                <= TOLERANCE
        );
    }
}

fn a_position(
    result: &geosolve_sketch::SketchSolveResult,
    point: geosolve_sketch::PointId,
) -> Point2<f64> {
    result.geometry.point(point).unwrap()
}

#[test]
fn symmetry_and_line_tangency_use_unit_directions_for_short_lines() {
    let short = 1.0e-12;
    let error = 1.0e-4;

    let mut symmetry = Sketch::new(1.0).unwrap();
    let l0 = symmetry.add_point(Point2::new(0.0, 0.0)).unwrap();
    let l1 = symmetry.add_point(Point2::new(short, 0.0)).unwrap();
    let first = symmetry.add_point(Point2::new(0.0, 1.0)).unwrap();
    let second = symmetry.add_point(Point2::new(error, -1.0)).unwrap();
    let line = symmetry.add_segment(l0, l1).unwrap();
    for point in [l0, l1, first, second] {
        symmetry.add_fixed_point(point).unwrap();
    }
    let source = symmetry
        .add_symmetric_about_line(first, second, line)
        .unwrap();
    let compiled = symmetry
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(source))
        .unwrap();
    let rows: Vec<_> = compiled
        .problem()
        .audit_rows()
        .unwrap()
        .into_iter()
        .filter(|row| Some(row.source_id) == mapping.core_source_id)
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| (row.scale - 1.0).abs() <= f64::EPSILON)
    );
    assert!(rows.iter().all(|row| row.template.contains("unit_line")));
    let result = solve(&mut symmetry);
    assert!(!result.accepted());
    assert!(result.unstable_core_report().hard_residual_max > TOLERANCE);

    let angle = -FRAC_PI_2 + error;
    let radial = [angle.cos(), angle.sin()];
    let contact = Point2::new(0.5 * short, 0.0);
    let center_position = Point2::new(contact.x - radial[0], contact.y - radial[1]);
    let mut tangent = Sketch::new(1.0).unwrap();
    let a = tangent.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = tangent.add_point(Point2::new(short, 0.0)).unwrap();
    let center = tangent.add_point(center_position).unwrap();
    let line = tangent.add_segment(a, b).unwrap();
    let circle = tangent.add_circle(center, 1.0).unwrap();
    for point in [a, b, center] {
        tangent.add_fixed_point(point).unwrap();
    }
    tangent
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let source = tangent
        .add_line_circle_tangency(
            line,
            circle,
            LineParameterDomain::BoundedSegment,
            LineSide::Left,
            0.5,
            angle,
        )
        .unwrap();
    let compiled = tangent
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(source))
        .unwrap();
    let alignment = compiled
        .problem()
        .audit_rows()
        .unwrap()
        .into_iter()
        .filter(|row| Some(row.source_id) == mapping.core_source_id)
        .nth(2)
        .unwrap();
    assert!((alignment.scale - 1.0).abs() <= f64::EPSILON);
    assert!(alignment.template.contains("unit_line_direction"));
    let result = solve(&mut tangent);
    assert!(!result.accepted());
    assert!(result.unstable_core_report().hard_residual_max > TOLERANCE);

    for half_length in [0.5e-12, 0.5, 0.5e6] {
        let mut reparameterized = Sketch::new(1.0).unwrap();
        let a = reparameterized
            .add_point(Point2::new(-half_length, 0.0))
            .unwrap();
        let b = reparameterized
            .add_point(Point2::new(half_length, 0.0))
            .unwrap();
        let center = reparameterized.add_point(Point2::new(0.0, 1.0)).unwrap();
        let first = reparameterized.add_point(Point2::new(0.5, 2.0)).unwrap();
        let second = reparameterized.add_point(Point2::new(0.5, -2.0)).unwrap();
        let line = reparameterized.add_segment(a, b).unwrap();
        let circle = reparameterized.add_circle(center, 1.0).unwrap();
        for point in [a, b, center, first, second] {
            reparameterized.add_fixed_point(point).unwrap();
        }
        reparameterized
            .add_circle_radius(circle, 1.0, DimensionMode::Driving)
            .unwrap();
        reparameterized
            .add_symmetric_about_line(first, second, line)
            .unwrap();
        reparameterized
            .add_line_circle_tangency(
                line,
                circle,
                LineParameterDomain::SupportingLine,
                LineSide::Left,
                0.5,
                -FRAC_PI_2,
            )
            .unwrap();
        let result = solve(&mut reparameterized);
        assert_accepted(&result);
    }
}

#[test]
fn every_candidate_segment_is_validated_and_midpoint_collapse_is_transactional() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let midpoint = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let segment = sketch.add_segment(a, b).unwrap();
    sketch.add_midpoint(midpoint, segment).unwrap();
    sketch.set_point_position(b, Point2::new(0.0, 0.0)).unwrap();
    sketch
        .set_point_position(midpoint, Point2::new(0.0, 0.0))
        .unwrap();
    let retained = sketch.geometry();
    assert!(matches!(
        sketch.solve(SketchSolveRequest::default(), SolverConfig::default()),
        Err(SketchError::InvalidSegmentEntity(id)) if id == segment
    ));
    assert_eq!(sketch.geometry(), retained);

    let mut isolated = Sketch::new(1.0).unwrap();
    let a = isolated.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = isolated.add_point(Point2::new(1.0, 0.0)).unwrap();
    let segment = isolated.add_segment(a, b).unwrap();
    isolated
        .set_point_position(b, Point2::new(0.0, 0.0))
        .unwrap();
    assert!(matches!(
        isolated.solve(SketchSolveRequest::default(), SolverConfig::default()),
        Err(SketchError::InvalidSegmentEntity(id)) if id == segment
    ));

    let mut induced = Sketch::new(1.0).unwrap();
    let a = induced.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = induced.add_point(Point2::new(2.0, 0.0)).unwrap();
    let midpoint = induced.add_point(Point2::new(1.0, 0.0)).unwrap();
    let segment = induced.add_segment(a, b).unwrap();
    induced.add_fixed_point(a).unwrap();
    induced
        .add_fixed_point_at(b, Point2::new(0.0, 0.0))
        .unwrap();
    induced.add_midpoint(midpoint, segment).unwrap();
    let retained = induced.geometry();
    let result = solve(&mut induced);
    assert_eq!(
        result.unstable_core_report().termination,
        SolveTermination::Converged
    );
    assert_eq!(
        result.rejection,
        Some(SolveRejection::DegenerateSegment(segment))
    );
    assert_eq!(result.geometry, retained);
}

#[test]
fn point_on_line_and_midpoint_hold_all_point_references() {
    for midpoint in [false, true] {
        let mut sketch = Sketch::new(1.0).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
        let point = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
        let segment = sketch.add_segment(a, b).unwrap();
        let constraint = if midpoint {
            sketch.add_midpoint(point, segment).unwrap()
        } else {
            sketch
                .add_point_on_line(point, segment, LineParameterDomain::BoundedSegment, 0.5)
                .unwrap()
        };
        for referenced in [a, b, point] {
            assert_eq!(
                sketch.remove_point(referenced),
                Err(SketchError::PointInUse(referenced))
            );
        }
        sketch.remove_constraint(constraint).unwrap();
        sketch.remove_segment(segment).unwrap();
        sketch.remove_point(point).unwrap();
        assert!(matches!(
            sketch.set_point_position(point, Point2::new(0.0, 0.0)),
            Err(SketchError::UnknownPoint(id)) if id == point
        ));
    }
}

#[test]
fn bounded_roundoff_is_clamped_audited_and_real_escape_rejects_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let endpoint_offset = 4.0 * f64::EPSILON;
        let mut sketch = Sketch::new(scale).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(scale, 0.0)).unwrap();
        let point = sketch
            .add_point(Point2::new(scale * (1.0 + endpoint_offset), 0.0))
            .unwrap();
        let segment = sketch.add_segment(a, b).unwrap();
        for fixed in [a, b, point] {
            sketch.add_fixed_point(fixed).unwrap();
        }
        let contact = sketch
            .add_point_on_line(point, segment, LineParameterDomain::BoundedSegment, 0.9)
            .unwrap();
        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let latent_variable = compiled
            .latent_variables()
            .iter()
            .find(|mapping| {
                mapping.constraint_id == contact
                    && mapping.role == LatentVariableRole::LineParameter
            })
            .unwrap()
            .variable_id;
        let config = SolverConfig {
            normalized_residual_tolerance: 32.0 * f64::EPSILON,
            normalized_step_tolerance: f64::EPSILON,
            ..SolverConfig::default()
        };
        let result = solve_with(&mut sketch, config);
        assert!(result.accepted(), "{:#?}", result.rejection);
        assert_eq!(
            sketch.contact_state(contact).unwrap(),
            ContactState::PointOnLine { parameter: 1.0 }
        );
        let mapping = result
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(contact))
            .unwrap();
        let source = result
            .display_audit
            .sources
            .iter()
            .find(|source| Some(source.source_id) == mapping.core_source_id)
            .unwrap();
        assert!(source.rows.iter().all(|row| {
            row.bindings
                .iter()
                .any(|binding| binding.name == "warm-start parameter" && binding.value == "1")
        }));
        assert!(source.rows.iter().all(|row| {
            row.incident_variables.iter().any(|variable| {
                variable.variable_id == latent_variable
                    && variable.value == VariableValue::Scalar(1.0)
            })
        }));
        let normalized_geometry_error =
            (result.geometry.point(point).unwrap() - result.geometry.point(b).unwrap()).norm()
                / scale;
        assert!(normalized_geometry_error <= config.normalized_residual_tolerance);
        let mut direct = sketch.clone();
        let direct_result = solve_with(&mut direct, config);
        assert!(direct_result.accepted(), "{:#?}", direct_result.rejection);
        assert_complete_analysis_matches(
            result.unstable_core_report(),
            direct_result.unstable_core_report(),
        );
        assert_eq!(result.display_audit, direct_result.display_audit);

        let endpoint_angle = PI * (1.0 + endpoint_offset);
        let mut arc_sketch = Sketch::new(scale).unwrap();
        let center = arc_sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let point = arc_sketch
            .add_point(Point2::new(
                scale * endpoint_angle.cos(),
                scale * endpoint_angle.sin(),
            ))
            .unwrap();
        let arc = arc_sketch
            .add_arc(center, scale, 0.0, PI, ArcSweep::CounterClockwise)
            .unwrap();
        arc_sketch.add_fixed_point(center).unwrap();
        arc_sketch.add_fixed_point(point).unwrap();
        arc_sketch
            .add_arc_radius(arc, scale, DimensionMode::Driving)
            .unwrap();
        let arc_contact = arc_sketch.add_point_on_arc(point, arc, 0.9).unwrap();
        let result = solve_with(&mut arc_sketch, config);
        assert!(result.accepted(), "{:#?}", result.rejection);
        assert_eq!(
            arc_sketch.contact_state(arc_contact).unwrap(),
            ContactState::PointOnArc {
                span_parameter: 1.0
            }
        );
        let clamped_arc_error = (result.geometry.point(point).unwrap()
            - result.geometry.arc(arc).unwrap().evaluate(1.0).unwrap())
        .norm()
            / scale;
        assert!(clamped_arc_error <= config.normalized_residual_tolerance);

        let mut escaped = Sketch::new(scale).unwrap();
        let a = escaped.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = escaped.add_point(Point2::new(scale, 0.0)).unwrap();
        let point = escaped
            .add_point(Point2::new(scale * (1.0 + 1.0e-4), 0.0))
            .unwrap();
        let segment = escaped.add_segment(a, b).unwrap();
        for fixed in [a, b, point] {
            escaped.add_fixed_point(fixed).unwrap();
        }
        let contact = escaped
            .add_point_on_line(point, segment, LineParameterDomain::BoundedSegment, 0.9)
            .unwrap();
        let retained = escaped.geometry();
        let result = solve(&mut escaped);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::ContactParameterOutOfDomain(contact))
        );
        assert_eq!(result.geometry, retained);

        let escape_angle = PI * (1.0 + 1.0e-4);
        let mut arc_escape = Sketch::new(scale).unwrap();
        let center = arc_escape.add_point(Point2::new(0.0, 0.0)).unwrap();
        let point = arc_escape
            .add_point(Point2::new(
                scale * escape_angle.cos(),
                scale * escape_angle.sin(),
            ))
            .unwrap();
        let arc = arc_escape
            .add_arc(center, scale, 0.0, PI, ArcSweep::CounterClockwise)
            .unwrap();
        arc_escape.add_fixed_point(center).unwrap();
        arc_escape.add_fixed_point(point).unwrap();
        arc_escape
            .add_arc_radius(arc, scale, DimensionMode::Driving)
            .unwrap();
        let contact = arc_escape.add_point_on_arc(point, arc, 0.9).unwrap();
        let result = solve(&mut arc_escape);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::ContactParameterOutOfDomain(contact))
        );
    }
}

#[test]
fn center_direction_near_orthogonal_is_ambiguous_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let normalized_x = 0.5e-9;
        let normalized_y = (1.0_f64 - normalized_x * normalized_x).sqrt();
        let mut sketch = Sketch::new(scale).unwrap();
        let first_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let second_center = sketch
            .add_point(Point2::new(normalized_x * scale, normalized_y * scale))
            .unwrap();
        let first = sketch.add_circle(first_center, 0.5 * scale).unwrap();
        let second = sketch.add_circle(second_center, 0.5 * scale).unwrap();
        sketch.add_fixed_point(first_center).unwrap();
        sketch.add_fixed_point(second_center).unwrap();
        sketch
            .add_circle_radius(first, 0.5 * scale, DimensionMode::Driving)
            .unwrap();
        sketch
            .add_circle_radius(second, 0.5 * scale, DimensionMode::Driving)
            .unwrap();
        let tangency = sketch
            .add_circle_circle_tangency(
                first,
                second,
                CircleTangencyMode::External,
                CenterDirectionBranch::positive_x(),
            )
            .unwrap();
        let result = solve(&mut sketch);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::CenterDirectionFlipped(tangency))
        );
    }

    for scale in [1.0e-6, 1.0, 1.0e6] {
        let feature = scale * 1.0e-12;
        let mut sketch = Sketch::new(scale).unwrap();
        let first_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let second_center = sketch.add_point(Point2::new(feature, 0.0)).unwrap();
        let first = sketch.add_circle(first_center, 0.4 * feature).unwrap();
        let second = sketch.add_circle(second_center, 0.6 * feature).unwrap();
        sketch.add_fixed_point(first_center).unwrap();
        sketch.add_fixed_point(second_center).unwrap();
        sketch
            .add_circle_radius(first, 0.4 * feature, DimensionMode::Driving)
            .unwrap();
        sketch
            .add_circle_radius(second, 0.6 * feature, DimensionMode::Driving)
            .unwrap();
        sketch
            .add_circle_circle_tangency(
                first,
                second,
                CircleTangencyMode::External,
                CenterDirectionBranch::positive_x(),
            )
            .unwrap();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        let cosine = CenterDirectionBranch::positive_x()
            .direction_cosine(
                result.geometry.point(first_center).unwrap(),
                result.geometry.point(second_center).unwrap(),
            )
            .unwrap();
        assert!((cosine - 1.0).abs() <= f64::EPSILON);
    }

    for residual_tolerance in [1.0e-12, 1.0e-3] {
        let direction_cosine = 0.5 * CENTER_DIRECTION_COSINE_MARGIN;
        let direction_sine = (1.0 - direction_cosine * direction_cosine).sqrt();
        let mut sketch = Sketch::new(1.0).unwrap();
        let first_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let second_center = sketch
            .add_point(Point2::new(direction_cosine, direction_sine))
            .unwrap();
        let first = sketch.add_circle(first_center, 0.4).unwrap();
        let second = sketch.add_circle(second_center, 0.6).unwrap();
        sketch.add_fixed_point(first_center).unwrap();
        sketch.add_fixed_point(second_center).unwrap();
        sketch
            .add_circle_radius(first, 0.4, DimensionMode::Driving)
            .unwrap();
        sketch
            .add_circle_radius(second, 0.6, DimensionMode::Driving)
            .unwrap();
        let tangency = sketch
            .add_circle_circle_tangency(
                first,
                second,
                CircleTangencyMode::External,
                CenterDirectionBranch::positive_x(),
            )
            .unwrap();
        let config = SolverConfig {
            normalized_residual_tolerance: residual_tolerance,
            ..SolverConfig::default()
        };
        let result = solve_with(&mut sketch, config);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::CenterDirectionFlipped(tangency))
        );
    }
}

#[test]
fn periodic_contact_audit_reports_committed_scalar_on_the_retained_branch() {
    let retained_angle = PI - 0.1;
    let solved_angle = PI + 0.1;
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let point = sketch
        .add_point(Point2::new(
            2.0 * solved_angle.cos(),
            2.0 * solved_angle.sin(),
        ))
        .unwrap();
    let circle = sketch.add_circle(center, 2.0).unwrap();
    sketch.add_fixed_point(center).unwrap();
    sketch.add_fixed_point(point).unwrap();
    sketch
        .add_circle_radius(circle, 2.0, DimensionMode::Driving)
        .unwrap();
    let contact = sketch
        .add_point_on_circle(point, circle, retained_angle)
        .unwrap();
    let compiled = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let angle_variable = compiled
        .latent_variables()
        .iter()
        .find(|mapping| {
            mapping.constraint_id == contact && mapping.role == LatentVariableRole::CircleAngle
        })
        .unwrap()
        .variable_id;
    let result = solve(&mut sketch);
    assert_accepted(&result);
    let ContactState::PointOnCircle { angle } = sketch.contact_state(contact).unwrap() else {
        panic!("wrong periodic contact state")
    };
    assert!((angle - solved_angle).abs() <= TOLERANCE);
    assert!((angle - retained_angle).abs() < 0.5);
    let mapping = result
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(contact))
        .unwrap();
    let source = result
        .display_audit
        .sources
        .iter()
        .find(|source| Some(source.source_id) == mapping.core_source_id)
        .unwrap();
    assert!(source.rows.iter().all(|row| {
        row.bindings.iter().any(|binding| {
            binding.name == "warm-start angle" && binding.value == retained_angle.to_string()
        })
    }));
    assert!(source.rows.iter().all(|row| {
        row.incident_variables.iter().any(|variable| {
            variable.variable_id == angle_variable
                && matches!(variable.value, VariableValue::Scalar(value) if (value - angle).abs() <= f64::EPSILON)
        })
    }));
}

#[test]
fn point_on_line_and_arc_have_split_similarity_recovery_oracles() {
    for (scale, angle, offset) in [
        (1.0e-6, -0.4, [3.0e-6, 7.0e-6]),
        (1.0, 0.7, [11.0, -5.0]),
        (1.0e6, 1.2, [-4.0e6, 2.0e6]),
    ] {
        let mut line_sketch = Sketch::new(scale).unwrap();
        let a = line_sketch
            .add_point(transform(Point2::new(0.0, 0.0), scale, angle, offset))
            .unwrap();
        let b = line_sketch
            .add_point(transform(Point2::new(2.0, 0.0), scale, angle, offset))
            .unwrap();
        let point = line_sketch
            .add_point(transform(Point2::new(0.7, 0.4), scale, angle, offset))
            .unwrap();
        let line = line_sketch.add_segment(a, b).unwrap();
        line_sketch.add_fixed_point(a).unwrap();
        line_sketch.add_fixed_point(b).unwrap();
        let contact = line_sketch
            .add_point_on_line(point, line, LineParameterDomain::BoundedSegment, 0.35)
            .unwrap();
        assert_fd(&line_sketch);
        let result = solve(&mut line_sketch);
        assert_accepted(&result);
        let axis = unit(a_position(&result, a), a_position(&result, b));
        let offset_to_point = a_position(&result, point) - a_position(&result, a);
        assert!(cross(axis, [offset_to_point.x, offset_to_point.y]).abs() / scale <= TOLERANCE);
        let ContactState::PointOnLine { parameter } = line_sketch.contact_state(contact).unwrap()
        else {
            panic!("wrong point-on-line state")
        };
        assert!((0.0..=1.0).contains(&parameter));

        let mut arc_sketch = Sketch::new(scale).unwrap();
        let center_position = transform(Point2::new(0.0, 0.0), scale, angle, offset);
        let center = arc_sketch.add_point(center_position).unwrap();
        let point = arc_sketch
            .add_point(transform(Point2::new(0.4, 1.7), scale, angle, offset))
            .unwrap();
        let arc = arc_sketch
            .add_arc(
                center,
                2.0 * scale,
                angle,
                angle + PI,
                ArcSweep::CounterClockwise,
            )
            .unwrap();
        arc_sketch.add_fixed_point(center).unwrap();
        arc_sketch
            .add_arc_radius(arc, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        let contact = arc_sketch.add_point_on_arc(point, arc, 0.45).unwrap();
        assert_fd(&arc_sketch);
        let result = solve(&mut arc_sketch);
        assert_accepted(&result);
        let ContactState::PointOnArc { span_parameter } =
            arc_sketch.contact_state(contact).unwrap()
        else {
            panic!("wrong point-on-arc state")
        };
        let solved = a_position(&result, point);
        let direct = result
            .geometry
            .arc(arc)
            .unwrap()
            .evaluate(span_parameter)
            .unwrap();
        assert!((solved - direct).norm() / scale <= TOLERANCE);
        assert!(((solved - center_position).norm() - 2.0 * scale).abs() / scale <= TOLERANCE);
    }
}

#[test]
fn segment_relations_midpoint_symmetry_and_angle_have_split_similarity_oracles() {
    for (scale, angle, offset) in [
        (1.0e-6, -0.3, [2.0e-6, 5.0e-6]),
        (1.0, 0.8, [9.0, -6.0]),
        (1.0e6, 1.1, [-3.0e6, 4.0e6]),
    ] {
        for perpendicular in [false, true] {
            let mut sketch = Sketch::new(scale).unwrap();
            let a = sketch
                .add_point(transform(Point2::new(0.0, 0.0), scale, angle, offset))
                .unwrap();
            let b = sketch
                .add_point(transform(Point2::new(2.0, 0.0), scale, angle, offset))
                .unwrap();
            let c = sketch
                .add_point(transform(Point2::new(0.0, 1.0), scale, angle, offset))
                .unwrap();
            let base_end = if perpendicular {
                Point2::new(0.3, 2.7)
            } else {
                Point2::new(1.7, 1.3)
            };
            let d = sketch
                .add_point(transform(base_end, scale, angle, offset))
                .unwrap();
            let first = sketch.add_segment(a, b).unwrap();
            let second = sketch.add_segment(c, d).unwrap();
            for fixed in [a, b, c] {
                sketch.add_fixed_point(fixed).unwrap();
            }
            if perpendicular {
                sketch.add_perpendicular(first, second).unwrap();
            } else {
                sketch.add_parallel(first, second).unwrap();
            }
            sketch
                .add_segment_length(second, 2.0 * scale, DimensionMode::Driving)
                .unwrap();
            assert_fd(&sketch);
            let result = solve(&mut sketch);
            assert_accepted(&result);
            let first_axis = unit(a_position(&result, a), a_position(&result, b));
            let second_axis = unit(a_position(&result, c), a_position(&result, d));
            let oracle = if perpendicular {
                dot(first_axis, second_axis)
            } else {
                cross(first_axis, second_axis)
            };
            assert!(oracle.abs() <= TOLERANCE);
        }

        let mut equal = Sketch::new(scale).unwrap();
        let a = equal
            .add_point(transform(Point2::new(0.0, 0.0), scale, angle, offset))
            .unwrap();
        let b = equal
            .add_point(transform(Point2::new(2.0, 0.0), scale, angle, offset))
            .unwrap();
        let c = equal
            .add_point(transform(Point2::new(0.0, 1.0), scale, angle, offset))
            .unwrap();
        let d = equal
            .add_point(transform(Point2::new(0.8, 1.4), scale, angle, offset))
            .unwrap();
        let first = equal.add_segment(a, b).unwrap();
        let second = equal.add_segment(c, d).unwrap();
        for fixed in [a, b, c] {
            equal.add_fixed_point(fixed).unwrap();
        }
        equal.add_equal_segment_length(first, second).unwrap();
        assert_fd(&equal);
        let result = solve(&mut equal);
        assert_accepted(&result);
        let first_length = (a_position(&result, b) - a_position(&result, a)).norm();
        let second_length = (a_position(&result, d) - a_position(&result, c)).norm();
        assert!((first_length - second_length).abs() / scale <= TOLERANCE);

        let mut midpoint = Sketch::new(scale).unwrap();
        let a = midpoint
            .add_point(transform(Point2::new(0.0, 0.0), scale, angle, offset))
            .unwrap();
        let b = midpoint
            .add_point(transform(Point2::new(4.0, 2.0), scale, angle, offset))
            .unwrap();
        let point = midpoint
            .add_point(transform(Point2::new(1.0, 0.2), scale, angle, offset))
            .unwrap();
        let segment = midpoint.add_segment(a, b).unwrap();
        midpoint.add_fixed_point(a).unwrap();
        midpoint.add_fixed_point(b).unwrap();
        midpoint.add_midpoint(point, segment).unwrap();
        assert_fd(&midpoint);
        let result = solve(&mut midpoint);
        assert_accepted(&result);
        let direct =
            Point2::from((a_position(&result, a).coords + a_position(&result, b).coords) * 0.5);
        assert!((a_position(&result, point) - direct).norm() / scale <= TOLERANCE);

        let mut symmetry = Sketch::new(scale).unwrap();
        let l0 = symmetry
            .add_point(transform(Point2::new(-2.0, 0.0), scale, angle, offset))
            .unwrap();
        let l1 = symmetry
            .add_point(transform(Point2::new(2.0, 0.0), scale, angle, offset))
            .unwrap();
        let first = symmetry
            .add_point(transform(Point2::new(1.0, 2.0), scale, angle, offset))
            .unwrap();
        let second = symmetry
            .add_point(transform(Point2::new(0.4, -1.0), scale, angle, offset))
            .unwrap();
        let line = symmetry.add_segment(l0, l1).unwrap();
        for fixed in [l0, l1, first] {
            symmetry.add_fixed_point(fixed).unwrap();
        }
        symmetry
            .add_symmetric_about_line(first, second, line)
            .unwrap();
        assert_fd(&symmetry);
        let result = solve(&mut symmetry);
        assert_accepted(&result);
        let axis = unit(a_position(&result, l0), a_position(&result, l1));
        let normal = [-axis[1], axis[0]];
        let pair = a_position(&result, second) - a_position(&result, first);
        let pair_midpoint = Point2::from(
            (a_position(&result, first).coords + a_position(&result, second).coords) * 0.5,
        );
        let midpoint_offset = pair_midpoint - a_position(&result, l0);
        assert!(dot(axis, [pair.x, pair.y]).abs() / scale <= TOLERANCE);
        assert!(dot(normal, [midpoint_offset.x, midpoint_offset.y]).abs() / scale <= TOLERANCE);

        let mut angle_sketch = Sketch::new(scale).unwrap();
        let o = angle_sketch
            .add_point(transform(Point2::new(0.0, 0.0), scale, angle, offset))
            .unwrap();
        let x = angle_sketch
            .add_point(transform(Point2::new(1.0, 0.0), scale, angle, offset))
            .unwrap();
        let p = angle_sketch
            .add_point(transform(Point2::new(0.3, 0.8), scale, angle, offset))
            .unwrap();
        let first = angle_sketch.add_segment(o, x).unwrap();
        let second = angle_sketch.add_segment(o, p).unwrap();
        angle_sketch.add_fixed_point(o).unwrap();
        angle_sketch.add_fixed_point(x).unwrap();
        angle_sketch
            .add_segment_length(second, scale, DimensionMode::Driving)
            .unwrap();
        angle_sketch
            .add_oriented_angle(
                first,
                second,
                FRAC_PI_2,
                AngleOrientation::CounterClockwise,
                DimensionMode::Driving,
            )
            .unwrap();
        assert_fd(&angle_sketch);
        let result = solve(&mut angle_sketch);
        assert_accepted(&result);
        let first_axis = unit(a_position(&result, o), a_position(&result, x));
        let second_axis = unit(a_position(&result, o), a_position(&result, p));
        assert!(dot(first_axis, second_axis).abs() <= TOLERANCE);
        assert!(cross(first_axis, second_axis) > 0.0);
    }
}

#[test]
fn equal_radius_and_all_radius_dimension_forms_have_scale_oracles() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut equal = Sketch::new(scale).unwrap();
        let first_center = equal.add_point(Point2::new(0.0, 0.0)).unwrap();
        let second_center = equal.add_point(Point2::new(4.0 * scale, 0.0)).unwrap();
        let first = equal.add_circle(first_center, 2.0 * scale).unwrap();
        let second = equal.add_circle(second_center, 1.2 * scale).unwrap();
        equal
            .add_circle_radius(first, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        equal.add_equal_circle_radius(first, second).unwrap();
        assert_fd(&equal);
        let result = solve(&mut equal);
        assert_accepted(&result);
        assert!(
            (result.geometry.circle(first).unwrap().radius
                - result.geometry.circle(second).unwrap().radius)
                .abs()
                / scale
                <= TOLERANCE
        );

        for diameter in [false, true] {
            let mut circle_sketch = Sketch::new(scale).unwrap();
            let center = circle_sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
            let circle = circle_sketch.add_circle(center, 0.8 * scale).unwrap();
            if diameter {
                circle_sketch
                    .add_circle_diameter(circle, 4.0 * scale, DimensionMode::Driving)
                    .unwrap();
            } else {
                circle_sketch
                    .add_circle_radius(circle, 2.0 * scale, DimensionMode::Driving)
                    .unwrap();
            }
            assert_fd(&circle_sketch);
            let result = solve(&mut circle_sketch);
            assert_accepted(&result);
            assert!(
                (result.geometry.circle(circle).unwrap().radius - 2.0 * scale).abs() / scale
                    <= TOLERANCE
            );

            let mut arc_sketch = Sketch::new(scale).unwrap();
            let center = arc_sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
            let arc = arc_sketch
                .add_arc(center, 0.8 * scale, 0.0, PI, ArcSweep::CounterClockwise)
                .unwrap();
            if diameter {
                arc_sketch
                    .add_arc_diameter(arc, 4.0 * scale, DimensionMode::Driving)
                    .unwrap();
            } else {
                arc_sketch
                    .add_arc_radius(arc, 2.0 * scale, DimensionMode::Driving)
                    .unwrap();
            }
            assert_fd(&arc_sketch);
            let result = solve(&mut arc_sketch);
            assert_accepted(&result);
            assert!(
                (result.geometry.arc(arc).unwrap().radius - 2.0 * scale).abs() / scale <= TOLERANCE
            );
        }
    }
}

#[test]
fn circle_tangency_literal_matrix_covers_every_mode_and_similarity() {
    #[derive(Clone, Copy)]
    struct Case {
        mode: CircleTangencyMode,
        first_radius: f64,
        second_radius: f64,
        center_distance: f64,
    }

    let cases = [
        Case {
            mode: CircleTangencyMode::External,
            first_radius: 2.0,
            second_radius: 1.0,
            center_distance: 3.0,
        },
        Case {
            mode: CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond,
            },
            first_radius: 2.0,
            second_radius: 0.75,
            center_distance: 1.25,
        },
        Case {
            mode: CircleTangencyMode::Internal {
                containment: CircleContainment::SecondContainsFirst,
            },
            first_radius: 0.75,
            second_radius: 2.0,
            center_distance: 1.25,
        },
    ];
    let similarities: [(f64, f64, [f64; 2]); 3] = [
        (1.0e-6, -0.45, [3.0e-6, -2.0e-6]),
        (1.0, 0.75, [11.0, -7.0]),
        (1.0e6, 1.25, [-4.0e6, 5.0e6]),
    ];

    for case in cases {
        for (scale, rotation, translation) in similarities {
            let (sine, cosine) = rotation.sin_cos();
            let branch = CenterDirectionBranch::new([cosine, sine]).unwrap();
            let first_position = Point2::new(translation[0], translation[1]);
            for perturbed in [false, true] {
                let axial = case.center_distance * scale * if perturbed { 1.35 } else { 1.0 };
                let transverse = if perturbed { 0.2 * scale } else { 0.0 };
                let second_position = Point2::new(
                    first_position.x + axial * cosine - transverse * sine,
                    first_position.y + axial * sine + transverse * cosine,
                );
                let mut sketch = Sketch::new(scale).unwrap();
                let first_center = sketch.add_point(first_position).unwrap();
                let second_center = sketch.add_point(second_position).unwrap();
                let first = sketch
                    .add_circle(first_center, case.first_radius * scale)
                    .unwrap();
                let second = sketch
                    .add_circle(second_center, case.second_radius * scale)
                    .unwrap();
                sketch.add_fixed_point(first_center).unwrap();
                sketch
                    .add_circle_radius(first, case.first_radius * scale, DimensionMode::Driving)
                    .unwrap();
                sketch
                    .add_circle_radius(second, case.second_radius * scale, DimensionMode::Driving)
                    .unwrap();
                sketch
                    .add_circle_circle_tangency(first, second, case.mode, branch)
                    .unwrap();
                assert_fd(&sketch);
                let result = solve(&mut sketch);
                assert_accepted(&result);
                let solved_first = result.geometry.circle(first).unwrap();
                let solved_second = result.geometry.circle(second).unwrap();
                let distance = (solved_second.center - solved_first.center).norm();
                assert!(
                    (distance - case.center_distance * scale).abs() / scale <= TOLERANCE,
                    "mode={:?} perturbed={perturbed} distance={distance:e}",
                    case.mode
                );
                let cosine = branch
                    .direction_cosine(solved_first.center, solved_second.center)
                    .unwrap();
                assert!(cosine > CENTER_DIRECTION_COSINE_MARGIN);
                match case.mode {
                    CircleTangencyMode::External => {
                        assert!(
                            (distance - solved_first.radius - solved_second.radius).abs() / scale
                                <= TOLERANCE
                        );
                    }
                    CircleTangencyMode::Internal {
                        containment: CircleContainment::FirstContainsSecond,
                    } => {
                        assert!(solved_first.radius > solved_second.radius);
                        assert!(
                            (distance - (solved_first.radius - solved_second.radius)).abs() / scale
                                <= TOLERANCE
                        );
                    }
                    CircleTangencyMode::Internal {
                        containment: CircleContainment::SecondContainsFirst,
                    } => {
                        assert!(solved_second.radius > solved_first.radius);
                        assert!(
                            (distance - (solved_second.radius - solved_first.radius)).abs() / scale
                                <= TOLERANCE
                        );
                    }
                }
            }
        }
    }

    let mut invalid = Sketch::new(1.0).unwrap();
    let first_center = invalid.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second_center = invalid.add_point(Point2::new(0.0, 0.0)).unwrap();
    let first = invalid.add_circle(first_center, 1.0).unwrap();
    let second = invalid.add_circle(second_center, 1.0).unwrap();
    invalid.add_fixed_point(first_center).unwrap();
    invalid.add_fixed_point(second_center).unwrap();
    let tangency = invalid
        .add_circle_circle_tangency(
            first,
            second,
            CircleTangencyMode::External,
            CenterDirectionBranch::positive_x(),
        )
        .unwrap();
    let result = solve(&mut invalid);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::CenterDirectionFlipped(tangency))
    );
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Invalid
    );
}

#[test]
fn segment_variant_invalid_checklist_is_literal_and_preflighted() {
    #[derive(Clone, Copy, Debug)]
    enum Variant {
        PointOnLine,
        Parallel,
        Perpendicular,
        EqualLength,
        Midpoint,
        Symmetry,
        LineCircleTangency,
        OrientedAngle,
    }

    let checklist = [
        Variant::PointOnLine,
        Variant::Parallel,
        Variant::Perpendicular,
        Variant::EqualLength,
        Variant::Midpoint,
        Variant::Symmetry,
        Variant::LineCircleTangency,
        Variant::OrientedAngle,
    ];
    assert_eq!(checklist.len(), 8);

    for variant in checklist {
        let mut sketch = Sketch::new(2.0).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
        let c = sketch.add_point(Point2::new(0.0, 2.0)).unwrap();
        let d_position = if matches!(variant, Variant::Perpendicular) {
            Point2::new(0.0, 4.0)
        } else {
            Point2::new(2.0, 2.0)
        };
        let d = sketch.add_point(d_position).unwrap();
        let point = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
        let reflected = sketch.add_point(Point2::new(1.0, -1.0)).unwrap();
        let first = sketch.add_segment(a, b).unwrap();
        let second = sketch.add_segment(c, d).unwrap();
        match variant {
            Variant::PointOnLine => {
                sketch
                    .add_point_on_line(point, first, LineParameterDomain::BoundedSegment, 0.5)
                    .unwrap();
            }
            Variant::Parallel => {
                sketch.add_parallel(first, second).unwrap();
            }
            Variant::Perpendicular => {
                sketch.add_perpendicular(first, second).unwrap();
            }
            Variant::EqualLength => {
                sketch.add_equal_segment_length(first, second).unwrap();
            }
            Variant::Midpoint => {
                sketch.add_midpoint(point, first).unwrap();
            }
            Variant::Symmetry => {
                let upper = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
                sketch
                    .add_symmetric_about_line(upper, reflected, first)
                    .unwrap();
            }
            Variant::LineCircleTangency => {
                let center = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
                let circle = sketch.add_circle(center, 1.0).unwrap();
                sketch
                    .add_line_circle_tangency(
                        first,
                        circle,
                        LineParameterDomain::BoundedSegment,
                        LineSide::Left,
                        0.5,
                        -FRAC_PI_2,
                    )
                    .unwrap();
            }
            Variant::OrientedAngle => {
                sketch
                    .add_oriented_angle(
                        first,
                        second,
                        FRAC_PI_2,
                        AngleOrientation::CounterClockwise,
                        DimensionMode::Driving,
                    )
                    .unwrap();
            }
        }
        sketch.set_point_position(b, Point2::new(0.0, 0.0)).unwrap();
        assert!(
            matches!(
                sketch.compile(SketchSolveRequest::default()),
                Err(SketchError::InvalidSegmentEntity(id)) if id == first
            ),
            "variant={variant:?}"
        );
    }
}

#[test]
fn exact_constraint_checklist_is_literal_and_independent() {
    #[derive(Clone, Copy, Debug)]
    enum Case {
        PointOnLine,
        PointOnCircle,
        Parallel,
        Perpendicular,
        EqualLength,
        EqualRadius,
        Midpoint,
        Symmetry,
        LineCircleTangency,
        OrientedAngle,
    }

    let checklist = [
        Case::PointOnLine,
        Case::PointOnCircle,
        Case::Parallel,
        Case::Perpendicular,
        Case::EqualLength,
        Case::EqualRadius,
        Case::Midpoint,
        Case::Symmetry,
        Case::LineCircleTangency,
        Case::OrientedAngle,
    ];
    assert_eq!(checklist.len(), 10);

    for case in checklist {
        let mut sketch = Sketch::new(2.0).unwrap();
        let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
        let c = sketch.add_point(Point2::new(0.0, 2.0)).unwrap();
        let d = sketch
            .add_point(
                if matches!(case, Case::Perpendicular | Case::OrientedAngle) {
                    Point2::new(0.0, 4.0)
                } else {
                    Point2::new(2.0, 2.0)
                },
            )
            .unwrap();
        let point = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
        let first = sketch.add_segment(a, b).unwrap();
        let second = sketch.add_segment(c, d).unwrap();
        let mut coordinate_oracle = None;
        match case {
            Case::PointOnLine => {
                sketch
                    .add_point_on_line(point, first, LineParameterDomain::BoundedSegment, 0.5)
                    .unwrap();
            }
            Case::PointOnCircle => {
                let angle = 0.4_f64;
                let radius = 1.5;
                let expected = Point2::new(radius * angle.cos(), 2.0 + radius * angle.sin());
                let on_circle = sketch.add_point(expected).unwrap();
                let circle = sketch.add_circle(c, radius).unwrap();
                sketch
                    .add_circle_radius(circle, radius, DimensionMode::Driving)
                    .unwrap();
                sketch
                    .add_point_on_circle(on_circle, circle, angle)
                    .unwrap();
                sketch.add_fixed_point(on_circle).unwrap();
                coordinate_oracle = Some((on_circle, expected));
            }
            Case::Parallel => {
                sketch.add_parallel(first, second).unwrap();
            }
            Case::Perpendicular => {
                sketch.add_perpendicular(first, second).unwrap();
            }
            Case::EqualLength => {
                sketch.add_equal_segment_length(first, second).unwrap();
            }
            Case::EqualRadius => {
                let first_circle = sketch.add_circle(a, 1.5).unwrap();
                let second_circle = sketch.add_circle(c, 1.5).unwrap();
                sketch
                    .add_equal_circle_radius(first_circle, second_circle)
                    .unwrap();
            }
            Case::Midpoint => {
                sketch.add_midpoint(point, first).unwrap();
            }
            Case::Symmetry => {
                let upper = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
                let lower = sketch.add_point(Point2::new(1.0, -1.0)).unwrap();
                sketch
                    .add_symmetric_about_line(upper, lower, first)
                    .unwrap();
                sketch.add_fixed_point(upper).unwrap();
                sketch.add_fixed_point(lower).unwrap();
            }
            Case::LineCircleTangency => {
                let center = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
                let circle = sketch.add_circle(center, 1.0).unwrap();
                sketch
                    .add_line_circle_tangency(
                        first,
                        circle,
                        LineParameterDomain::BoundedSegment,
                        LineSide::Left,
                        0.5,
                        -FRAC_PI_2,
                    )
                    .unwrap();
                sketch.add_fixed_point(center).unwrap();
                sketch
                    .add_circle_radius(circle, 1.0, DimensionMode::Driving)
                    .unwrap();
            }
            Case::OrientedAngle => {
                sketch
                    .add_oriented_angle(
                        first,
                        second,
                        FRAC_PI_2,
                        AngleOrientation::CounterClockwise,
                        DimensionMode::Driving,
                    )
                    .unwrap();
            }
        }
        for fixed in [a, b, c, d, point] {
            sketch.add_fixed_point(fixed).unwrap();
        }
        assert_fd(&sketch);
        let result = solve(&mut sketch);
        assert_accepted(&result);
        if let Some((point, expected)) = coordinate_oracle {
            assert!((result.geometry.point(point).unwrap() - expected).norm() <= f64::EPSILON);
        }
    }
}
