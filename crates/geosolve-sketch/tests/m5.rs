#![allow(clippy::too_many_lines)]

use geosolve_core::{
    AuditEvaluationStatus, AuditRowSnapshot, AuditSnapshot, AuditSourceSnapshot, CoreError,
    EvaluationErrorCategory, HardValidity, ResidualCategory, SolveTermination, SolverConfig,
    SourceConstraintId, VariableValue,
};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ConflictingRectangleIds, CoordinateAxis, DimensionKind, DimensionMode, Sketch, SketchError,
    SketchSolveRequest, SketchSource, SolveRejection, UnderconstrainedTriangleIds,
    conflicting_rectangle, redundant_rectangle, underconstrained_triangle,
};

const TOLERANCE: f64 = 1.0e-9;

fn assert_point(actual: Point2<f64>, expected: Point2<f64>, scale: f64, tolerance: f64) {
    let error = (actual - expected).norm() / scale;
    assert!(
        error <= tolerance,
        "actual={actual:?} expected={expected:?} normalized error={error:e}"
    );
}

fn assert_length(first: Point2<f64>, second: Point2<f64>, expected: f64, scale: f64) {
    let error = ((second - first).norm() - expected).abs() / scale;
    assert!(error <= TOLERANCE, "length error={error:e}");
}

fn assert_accepted(result: &geosolve_sketch::SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
    assert_eq!(result.display_audit, result.core_report.audit);
}

fn display_row<'a>(
    result: &'a geosolve_sketch::SketchSolveResult,
    source_label_fragment: &str,
) -> &'a AuditRowSnapshot {
    audit_row(&result.display_audit, source_label_fragment)
}

fn audit_row<'a>(audit: &'a AuditSnapshot, source_label_fragment: &str) -> &'a AuditRowSnapshot {
    &audit
        .sources
        .iter()
        .find(|source| source.source_label.contains(source_label_fragment))
        .unwrap_or_else(|| panic!("missing display source containing {source_label_fragment:?}"))
        .rows[0]
}

fn display_source(
    result: &geosolve_sketch::SketchSolveResult,
    source: SketchSource,
) -> &AuditSourceSnapshot {
    let core_source_id = result
        .source_mappings
        .iter()
        .find_map(|mapping| {
            (mapping.source == source)
                .then_some(mapping.core_source_id)
                .flatten()
        })
        .unwrap_or_else(|| panic!("missing core source mapping for {source:?}"));
    result
        .display_audit
        .sources
        .iter()
        .find(|item| item.source_id == core_source_id)
        .unwrap_or_else(|| panic!("missing display audit source for {source:?}"))
}

fn incident_point(row: &AuditRowSnapshot, index: usize) -> Point2<f64> {
    let VariableValue::Vec2([x, y]) = row.incident_variables[index].value else {
        panic!("expected Vec2 incident variable: {row:#?}")
    };
    Point2::new(x, y)
}

fn mapped_sketch_sources(
    result: &geosolve_sketch::SketchSolveResult,
    core_sources: &[SourceConstraintId],
) -> Vec<SketchSource> {
    core_sources
        .iter()
        .map(|core_source| {
            result
                .source_mappings
                .iter()
                .find_map(|mapping| {
                    (mapping.core_source_id == Some(*core_source)).then_some(mapping.source)
                })
                .unwrap_or_else(|| panic!("missing sketch mapping for {core_source:?}"))
        })
        .collect()
}

fn scaled_triangle(
    scale: f64,
) -> Result<(Sketch, UnderconstrainedTriangleIds), geosolve_sketch::SketchError> {
    let mut sketch = Sketch::new(scale)?;
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0))?;
    let b = sketch.add_named_point("B", Point2::new(4.0 * scale, 0.0))?;
    let c = sketch.add_named_point("C", Point2::new(2.2 * scale, 2.0 * scale))?;
    let ab = sketch.add_named_segment("AB", a, b)?;
    let fixed_a = sketch.add_fixed_point(a)?;
    let horizontal_ab = sketch.add_horizontal(ab)?;
    let length_ab = sketch.add_segment_length(ab, 4.0 * scale, DimensionMode::Driving)?;
    let distance_ac = sketch.add_point_distance(a, c, 3.0 * scale, DimensionMode::Driving)?;
    Ok((
        sketch,
        UnderconstrainedTriangleIds {
            a,
            b,
            c,
            ab,
            fixed_a,
            horizontal_ab,
            length_ab,
            distance_ac,
        },
    ))
}

fn scaled_rectangle(
    scale: f64,
    conflicting: bool,
) -> Result<(Sketch, ConflictingRectangleIds), SketchError> {
    let (mut sketch, ids) = if conflicting {
        conflicting_rectangle()?
    } else {
        redundant_rectangle()?
    };
    sketch.set_model_scale(scale)?;
    sketch.set_point_position(ids.a, Point2::new(0.0, 0.0))?;
    sketch.set_point_position(ids.b, Point2::new(4.0 * scale, 0.0))?;
    sketch.set_point_position(ids.c, Point2::new(4.0 * scale, 3.0 * scale))?;
    sketch.set_point_position(ids.d, Point2::new(0.0, 3.0 * scale))?;
    sketch.set_dimension_target(ids.width_4, 4.0 * scale)?;
    sketch.set_dimension_target(
        ids.width_5,
        if conflicting {
            5.0 * scale
        } else {
            4.0 * scale
        },
    )?;
    Ok((sketch, ids))
}

#[test]
fn s1_initial_solve_has_one_dof_and_the_explicit_rightward_branch() {
    let (mut sketch, ids) = underconstrained_triangle().unwrap();
    let branch = sketch.segment(ids.ab).unwrap().branch();
    assert_point(
        Point2::from(branch.reference_direction()),
        Point2::new(1.0, 0.0),
        1.0,
        0.0,
    );

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&result);
    assert_eq!(result.core_report.local_degrees_of_freedom, 1);
    assert_point(
        result.geometry.point(ids.b).unwrap(),
        Point2::new(4.0, 0.0),
        1.0,
        TOLERANCE,
    );
    assert_length(
        result.geometry.point(ids.a).unwrap(),
        result.geometry.point(ids.c).unwrap(),
        3.0,
        1.0,
    );
    assert!(sketch.segment_branch_is_preserved(ids.ab).unwrap());
    assert!(
        result
            .core_report
            .audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .all(|row| !row.template.trim().is_empty()
                && !row.bindings.is_empty()
                && row.scale.is_finite()
                && row.scale > 0.0)
    );
}

#[test]
fn s1_drag_projects_multiple_targets_and_release_preserves_the_last_state() {
    let (mut sketch, ids) = underconstrained_triangle().unwrap();
    let initial = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&initial);

    for target in [
        Point2::new(0.5, 5.0),
        Point2::new(-4.0, 1.5),
        Point2::new(3.0, -2.0),
    ] {
        let result = sketch
            .solve(
                SketchSolveRequest::default().with_drag(ids.c, target),
                SolverConfig::default(),
            )
            .unwrap();
        assert_accepted(&result);
        let expected = Point2::from(target.coords.normalize() * 3.0);
        let solved = result.geometry.point(ids.c).unwrap();
        // The current priority solver reaches this stationary point to about
        // 3e-8 model scales when lower-priority previous-state rows are present.
        assert_point(solved, expected, 1.0, 5.0e-8);
        assert_length(result.geometry.point(ids.a).unwrap(), solved, 3.0, 1.0);

        let attained_error = (solved - target).norm();
        let known_minimum = (target.coords.norm() - 3.0).abs();
        assert!((attained_error - known_minimum).abs() <= 5.0e-8);
        let temporary = result
            .core_report
            .priority_solves
            .iter()
            .find(|priority| priority.category == ResidualCategory::Temporary)
            .unwrap();
        assert_eq!(temporary.termination, SolveTermination::Converged);
    }

    let accepted_c = sketch.point(ids.c).unwrap().position();
    let released = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&released);
    assert_point(
        released.geometry.point(ids.c).unwrap(),
        accepted_c,
        1.0,
        2.0e-10,
    );
    assert!(sketch.segment_branch_is_preserved(ids.ab).unwrap());
}

#[test]
fn s1_is_scale_invariant_with_identical_classification_branch_and_source_order() {
    let mut baseline_sources = None;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut sketch, ids) = scaled_triangle(scale).unwrap();
        let result = sketch
            .solve(
                SketchSolveRequest::default()
                    .with_drag(ids.c, Point2::new(-1.5 * scale, 4.0 * scale)),
                SolverConfig::default(),
            )
            .unwrap();
        assert_accepted(&result);
        assert_eq!(result.core_report.local_degrees_of_freedom, 1);
        assert_eq!(result.core_report.rank, 3);
        assert_point(
            result.geometry.point(ids.b).unwrap(),
            Point2::new(4.0 * scale, 0.0),
            scale,
            2.0e-9,
        );
        let expected_direction = Point2::new(-1.5, 4.0).coords.normalize() * 3.0 * scale;
        assert_point(
            result.geometry.point(ids.c).unwrap(),
            Point2::from(expected_direction),
            scale,
            5.0e-8,
        );
        assert!(sketch.segment_branch_is_preserved(ids.ab).unwrap());

        let sources: Vec<_> = result
            .source_mappings
            .iter()
            .map(|mapping| mapping.source)
            .collect();
        if let Some(expected) = &baseline_sources {
            assert_eq!(&sources, expected);
        } else {
            baseline_sources = Some(sources);
        }
    }
}

#[test]
fn s2_compiles_deterministically_and_maps_only_both_widths_as_conflicting() {
    let (mut sketch, ids) = conflicting_rectangle().unwrap();
    let (same_scene, same_ids) = conflicting_rectangle().unwrap();
    assert_eq!(ids, same_ids);
    for (point, expected) in [
        (ids.a, Point2::new(0.0, 0.0)),
        (ids.b, Point2::new(4.0, 0.0)),
        (ids.c, Point2::new(4.0, 3.0)),
        (ids.d, Point2::new(0.0, 3.0)),
    ] {
        assert_eq!(sketch.point(point).unwrap().position(), expected);
    }
    for (segment, expected) in [
        (ids.ab, (ids.a, ids.b)),
        (ids.bc, (ids.b, ids.c)),
        (ids.cd, (ids.c, ids.d)),
        (ids.da, (ids.d, ids.a)),
    ] {
        let segment = sketch.segment(segment).unwrap();
        assert_eq!((segment.start(), segment.end()), expected);
    }
    assert!(matches!(
        sketch.dimension(ids.width_4).unwrap().kind(),
        DimensionKind::SegmentLength { segment, target }
            if segment == ids.ab && (target - 4.0).abs() <= f64::EPSILON
    ));
    assert!(matches!(
        sketch.dimension(ids.width_5).unwrap().kind(),
        DimensionKind::SegmentLength { segment, target }
            if segment == ids.ab && (target - 5.0).abs() <= f64::EPSILON
    ));
    assert_eq!(
        sketch.dimension(ids.width_4).unwrap().mode(),
        DimensionMode::Driving
    );
    assert_eq!(
        sketch.dimension(ids.width_5).unwrap().mode(),
        DimensionMode::Driving
    );

    let request = SketchSolveRequest::default().without_previous_state_preferences();
    let first_compile = sketch.compile(request).unwrap();
    let second_compile = sketch.compile(request).unwrap();
    let same_scene_compile = same_scene.compile(request).unwrap();
    assert_eq!(
        first_compile.point_variables(),
        second_compile.point_variables()
    );
    assert_eq!(
        first_compile.source_mappings(),
        second_compile.source_mappings()
    );
    assert_eq!(
        first_compile.source_mappings(),
        same_scene_compile.source_mappings()
    );
    assert_eq!(
        first_compile.problem().audit_rows().unwrap(),
        second_compile.problem().audit_rows().unwrap()
    );

    let retained = sketch.geometry();
    let result = sketch.solve(request, SolverConfig::default()).unwrap();
    assert!(!result.accepted());
    assert_ne!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.geometry, retained);
    assert_eq!(sketch.geometry(), retained);
    assert!(
        result
            .geometry
            .points
            .iter()
            .all(|point| point.position.x.is_finite() && point.position.y.is_finite())
    );
    assert!(
        result
            .core_report
            .accepted_state
            .ambient()
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max > TOLERANCE);
    assert_eq!(
        mapped_sketch_sources(&result, &result.core_report.conflicting_sources),
        vec![
            SketchSource::Dimension(ids.width_4),
            SketchSource::Dimension(ids.width_5),
        ]
    );
    for (dimension, expected_raw) in [(ids.width_4, 0.0), (ids.width_5, -1.0)] {
        let source = display_source(&result, SketchSource::Dimension(dimension));
        assert!(source.annotations.conflicting);
        assert!(!source.annotations.redundant);
        assert!(!source.annotations.singular);
        assert_eq!(source.rows.len(), 1);
        let row = &source.rows[0];
        assert!(row.annotations.conflicting);
        assert!(!row.annotations.redundant);
        assert!(!row.annotations.singular);
        assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
        assert_eq!(row.evaluation_error, None);
        assert!((row.raw_residual - expected_raw).abs() <= f64::EPSILON);
        assert!((row.normalized_residual - expected_raw).abs() <= f64::EPSILON);
        assert_point(
            incident_point(row, 0),
            result.geometry.point(ids.a).unwrap(),
            1.0,
            0.0,
        );
        assert_point(
            incident_point(row, 1),
            result.geometry.point(ids.b).unwrap(),
            1.0,
            0.0,
        );
    }
    for constraint in [
        ids.fixed_a,
        ids.horizontal_ab,
        ids.horizontal_cd,
        ids.vertical_bc,
        ids.vertical_da,
    ] {
        let source = display_source(&result, SketchSource::Constraint(constraint));
        assert!(!source.annotations.conflicting);
        assert!(source.rows.iter().all(|row| !row.annotations.conflicting));
    }
}

#[test]
fn s2_equal_width_variant_marks_only_the_second_width_redundant() {
    let (mut sketch, ids) = redundant_rectangle().unwrap();
    let result = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();
    assert_accepted(&result);
    assert!(result.core_report.conflicting_sources.is_empty());
    assert_eq!(
        mapped_sketch_sources(&result, &result.core_report.redundant_sources),
        vec![SketchSource::Dimension(ids.width_5)]
    );
    assert_eq!(
        mapped_sketch_sources(
            &result,
            &result.core_report.sources_containing_redundant_rows,
        ),
        vec![SketchSource::Dimension(ids.width_5)]
    );
}

#[test]
fn s2_conflict_and_redundancy_diagnostics_are_scale_invariant() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut conflicting, conflict_ids) = scaled_rectangle(scale, true).unwrap();
        let conflict = conflicting
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap();
        assert!(!conflict.accepted());
        assert_ne!(
            conflict.core_report.termination,
            SolveTermination::Converged
        );
        assert!(conflict.core_report.hard_residuals_validated);
        assert!(conflict.core_report.hard_residual_max > TOLERANCE);
        assert_eq!(
            mapped_sketch_sources(&conflict, &conflict.core_report.conflicting_sources),
            vec![
                SketchSource::Dimension(conflict_ids.width_4),
                SketchSource::Dimension(conflict_ids.width_5),
            ]
        );

        let (mut redundant, redundant_ids) = scaled_rectangle(scale, false).unwrap();
        let redundancy = redundant
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap();
        assert_accepted(&redundancy);
        assert!(redundancy.core_report.conflicting_sources.is_empty());
        assert_eq!(
            mapped_sketch_sources(&redundancy, &redundancy.core_report.redundant_sources),
            vec![SketchSource::Dimension(redundant_ids.width_5)]
        );
    }
}

#[test]
fn every_compiled_residual_has_an_analytic_jacobian_with_exact_source_mapping() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(3.0, 1.0)).unwrap();
    let c = sketch.add_named_point("C", Point2::new(-1.0, 2.0)).unwrap();
    let d = sketch.add_named_point("D", Point2::new(2.0, 4.0)).unwrap();
    let ab = sketch.add_named_segment("AB", a, b).unwrap();
    let cd = sketch.add_named_segment("CD", c, d).unwrap();
    sketch
        .add_fixed_point_at(a, Point2::new(0.5, -0.25))
        .unwrap();
    sketch
        .add_fixed_coordinate(b, CoordinateAxis::X, 2.5)
        .unwrap();
    sketch.add_coincident(b, c).unwrap();
    sketch.add_horizontal(ab).unwrap();
    sketch.add_vertical(cd).unwrap();
    sketch
        .add_point_distance(a, d, 4.5, DimensionMode::Driving)
        .unwrap();
    sketch
        .add_segment_length(cd, 3.25, DimensionMode::Driving)
        .unwrap();
    let reference = sketch
        .add_point_distance(a, b, 9.0, DimensionMode::Reference)
        .unwrap();

    let compiled = sketch
        .compile(SketchSolveRequest::default().with_drag(d, Point2::new(5.0, -3.0)))
        .unwrap();
    let check = compiled.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        check.all_within(1.0e-6),
        "max relative error={:e}: {check:#?}",
        check.max_relative_error()
    );
    assert_eq!(compiled.point_variables().len(), 4);
    for mapping in compiled.source_mappings() {
        if let Some(source_id) = mapping.core_source_id {
            assert_eq!(mapping.residual_ids.len(), 1);
            assert_eq!(
                compiled
                    .problem()
                    .residual(mapping.residual_ids[0])
                    .unwrap()
                    .source(),
                source_id
            );
            assert_eq!(
                compiled.problem().source(source_id).unwrap().label(),
                mapping.source_label
            );
        } else {
            assert_eq!(mapping.source, SketchSource::Dimension(reference));
            assert!(mapping.residual_ids.is_empty());
        }
    }
}

#[test]
fn fixed_point_and_coordinate_exact_and_perturbed_fixtures_recover() {
    for initial in [Point2::new(2.0, -1.0), Point2::new(8.0, 6.0)] {
        let mut sketch = Sketch::new(3.0).unwrap();
        let point = sketch.add_named_point("P", initial).unwrap();
        sketch
            .add_fixed_point_at(point, Point2::new(2.0, -1.0))
            .unwrap();
        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        assert_eq!(
            compiled
                .problem()
                .structural_summary()
                .unwrap()
                .eliminated_rows,
            2
        );
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_point(
            result.geometry.point(point).unwrap(),
            Point2::new(2.0, -1.0),
            3.0,
            TOLERANCE,
        );
    }

    for initial in [Point2::new(2.0, 7.0), Point2::new(-4.0, 7.0)] {
        let mut sketch = Sketch::new(2.0).unwrap();
        let point = sketch.add_named_point("P", initial).unwrap();
        sketch
            .add_fixed_coordinate(point, CoordinateAxis::X, 2.0)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_point(
            result.geometry.point(point).unwrap(),
            Point2::new(2.0, 7.0),
            2.0,
            2.0e-9,
        );
        assert_eq!(result.core_report.local_degrees_of_freedom, 1);
    }
}

#[test]
fn fixed_point_recovery_is_similarity_metamorphic() {
    let target = Point2::new(1.0, -2.0);
    let initial = Point2::new(4.0, 3.0);
    for (scale, angle, translation) in [
        (1.0e-6, -0.4, [3.0e-6, 7.0e-6]),
        (1.0, 0.8, [12.0, -9.0]),
        (1.0e6, 1.3, [-2.0e6, 5.0e6]),
    ] {
        let transformed_target = transform(target, scale, angle, translation);
        let mut sketch = Sketch::new(scale).unwrap();
        let point = sketch
            .add_named_point("P", transform(initial, scale, angle, translation))
            .unwrap();
        sketch
            .add_fixed_point_at(point, transformed_target)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_point(
            result.geometry.point(point).unwrap(),
            transformed_target,
            scale,
            TOLERANCE,
        );
    }
}

#[test]
fn coincident_horizontal_and_vertical_exact_and_perturbed_fixtures_recover() {
    for second in [Point2::new(1.0, 2.0), Point2::new(5.0, 6.0)] {
        let mut sketch = Sketch::new(4.0).unwrap();
        let a = sketch.add_named_point("A", Point2::new(1.0, 2.0)).unwrap();
        let b = sketch.add_named_point("B", second).unwrap();
        sketch.add_coincident(a, b).unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_point(
            result.geometry.point(a).unwrap(),
            result.geometry.point(b).unwrap(),
            4.0,
            TOLERANCE,
        );
        assert_eq!(result.core_report.local_degrees_of_freedom, 2);
    }

    for (end, vertical) in [
        (Point2::new(4.0, 1.0), false),
        (Point2::new(4.0, 3.0), false),
        (Point2::new(1.0, 4.0), true),
        (Point2::new(3.0, 4.0), true),
    ] {
        let mut sketch = Sketch::new(3.0).unwrap();
        let a = sketch.add_named_point("A", Point2::new(1.0, 1.0)).unwrap();
        let b = sketch.add_named_point("B", end).unwrap();
        let segment = sketch.add_named_segment("AB", a, b).unwrap();
        if vertical {
            sketch.add_vertical(segment).unwrap();
        } else {
            sketch.add_horizontal(segment).unwrap();
        }
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        let solved_a = result.geometry.point(a).unwrap();
        let solved_b = result.geometry.point(b).unwrap();
        if vertical {
            assert!((solved_b.x - solved_a.x).abs() / 3.0 <= TOLERANCE);
        } else {
            assert!((solved_b.y - solved_a.y).abs() / 3.0 <= TOLERANCE);
        }
        assert!(sketch.segment_branch_is_preserved(segment).unwrap());
    }
}

#[test]
fn point_distance_and_segment_length_exact_and_perturbed_fixtures_recover() {
    for initial_length in [4.0, 2.5] {
        let mut sketch = Sketch::new(4.0).unwrap();
        let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
        let b = sketch
            .add_named_point("B", Point2::new(initial_length, 0.0))
            .unwrap();
        sketch
            .add_point_distance(a, b, 4.0, DimensionMode::Driving)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_length(
            result.geometry.point(a).unwrap(),
            result.geometry.point(b).unwrap(),
            4.0,
            4.0,
        );
        assert_eq!(result.core_report.local_degrees_of_freedom, 3);
    }

    for initial_length in [4.0, 2.5] {
        let mut sketch = Sketch::new(4.0).unwrap();
        let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
        let b = sketch
            .add_named_point("B", Point2::new(initial_length, 0.0))
            .unwrap();
        let segment = sketch.add_named_segment("AB", a, b).unwrap();
        sketch
            .add_segment_length(segment, 4.0, DimensionMode::Driving)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_length(
            result.geometry.point(a).unwrap(),
            result.geometry.point(b).unwrap(),
            4.0,
            4.0,
        );
        assert!(sketch.segment_branch_is_preserved(segment).unwrap());
    }
}

fn transform(point: Point2<f64>, scale: f64, angle: f64, translation: [f64; 2]) -> Point2<f64> {
    let cosine = angle.cos();
    let sine = angle.sin();
    Point2::new(
        scale * (cosine * point.x - sine * point.y) + translation[0],
        scale * (sine * point.x + cosine * point.y) + translation[1],
    )
}

#[test]
fn distance_recovery_is_translation_rotation_and_scale_metamorphic() {
    let base_a = Point2::new(1.0, -2.0);
    let base_b = Point2::new(3.0, 1.0);
    for (scale, angle, translation) in [
        (1.0, 0.0, [0.0, 0.0]),
        (1.0, 0.7, [20.0, -11.0]),
        (1.0e-6, -0.3, [-4.0e-6, 7.0e-6]),
        (1.0e6, 1.2, [3.0e6, -2.0e6]),
    ] {
        let mut sketch = Sketch::new(scale).unwrap();
        let a = sketch
            .add_named_point("A", transform(base_a, scale, angle, translation))
            .unwrap();
        let b = sketch
            .add_named_point("B", transform(base_b, scale, angle, translation))
            .unwrap();
        sketch
            .add_point_distance(a, b, 4.0 * scale, DimensionMode::Driving)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_length(
            result.geometry.point(a).unwrap(),
            result.geometry.point(b).unwrap(),
            4.0 * scale,
            scale,
        );
        assert_eq!(result.core_report.local_degrees_of_freedom, 3);
    }
}

#[test]
fn ninety_degree_rotation_maps_horizontal_recovery_to_vertical_recovery() {
    let mut horizontal = Sketch::new(1.0).unwrap();
    let ha = horizontal
        .add_named_point("A", Point2::new(1.0, 2.0))
        .unwrap();
    let hb = horizontal
        .add_named_point("B", Point2::new(5.0, 4.0))
        .unwrap();
    let hs = horizontal.add_named_segment("AB", ha, hb).unwrap();
    horizontal.add_horizontal(hs).unwrap();
    let horizontal_result = horizontal
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&horizontal_result);

    let rotate = |point: Point2<f64>| Point2::new(-point.y, point.x);
    let mut vertical = Sketch::new(1.0).unwrap();
    let va = vertical
        .add_named_point("A", rotate(Point2::new(1.0, 2.0)))
        .unwrap();
    let vb = vertical
        .add_named_point("B", rotate(Point2::new(5.0, 4.0)))
        .unwrap();
    let vs = vertical.add_named_segment("AB", va, vb).unwrap();
    vertical.add_vertical(vs).unwrap();
    let vertical_result = vertical
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&vertical_result);
    assert_point(
        vertical_result.geometry.point(va).unwrap(),
        rotate(horizontal_result.geometry.point(ha).unwrap()),
        1.0,
        2.0e-9,
    );
    assert_point(
        vertical_result.geometry.point(vb).unwrap(),
        rotate(horizontal_result.geometry.point(hb).unwrap()),
        1.0,
        2.0e-9,
    );
}

#[test]
fn horizontal_and_vertical_recovery_are_translation_and_scale_metamorphic() {
    for (scale, translation) in [
        (1.0e-6, [4.0e-6, -7.0e-6]),
        (1.0, [13.0, -5.0]),
        (1.0e6, [-3.0e6, 8.0e6]),
    ] {
        let mut horizontal = Sketch::new(scale).unwrap();
        let ha = horizontal
            .add_named_point(
                "A",
                transform(Point2::new(1.0, 2.0), scale, 0.0, translation),
            )
            .unwrap();
        let hb = horizontal
            .add_named_point(
                "B",
                transform(Point2::new(5.0, 4.0), scale, 0.0, translation),
            )
            .unwrap();
        let segment = horizontal.add_named_segment("AB", ha, hb).unwrap();
        horizontal.add_horizontal(segment).unwrap();
        let result = horizontal
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        let solved_a = result.geometry.point(ha).unwrap();
        let solved_b = result.geometry.point(hb).unwrap();
        assert!((solved_b.y - solved_a.y).abs() / scale <= TOLERANCE);
        assert!(!horizontal.segment_has_enforced_branch(segment).unwrap());

        let mut vertical = Sketch::new(scale).unwrap();
        let va = vertical
            .add_named_point(
                "A",
                transform(Point2::new(1.0, 2.0), scale, 0.0, translation),
            )
            .unwrap();
        let vb = vertical
            .add_named_point(
                "B",
                transform(Point2::new(3.0, 6.0), scale, 0.0, translation),
            )
            .unwrap();
        let segment = vertical.add_named_segment("AB", va, vb).unwrap();
        vertical.add_vertical(segment).unwrap();
        let result = vertical
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        let solved_a = result.geometry.point(va).unwrap();
        let solved_b = result.geometry.point(vb).unwrap();
        assert!((solved_b.x - solved_a.x).abs() / scale <= TOLERANCE);
        assert!(!vertical.segment_has_enforced_branch(segment).unwrap());
    }
}

#[test]
fn coincidence_and_fixed_coordinate_recovery_transform_consistently() {
    for (scale, angle, translation) in [
        (1.0, 0.0, [0.0, 0.0]),
        (1.0, 0.8, [12.0, -7.0]),
        (1.0e-6, -0.4, [3.0e-6, 5.0e-6]),
        (1.0e6, 1.1, [-2.0e6, 4.0e6]),
    ] {
        let mut sketch = Sketch::new(scale).unwrap();
        let first = sketch
            .add_named_point(
                "A",
                transform(Point2::new(1.0, 2.0), scale, angle, translation),
            )
            .unwrap();
        let second = sketch
            .add_named_point(
                "B",
                transform(Point2::new(5.0, 6.0), scale, angle, translation),
            )
            .unwrap();
        sketch.add_coincident(first, second).unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        let expected = transform(Point2::new(3.0, 4.0), scale, angle, translation);
        assert_point(
            result.geometry.point(first).unwrap(),
            expected,
            scale,
            2.0e-8,
        );
        assert_point(
            result.geometry.point(second).unwrap(),
            expected,
            scale,
            2.0e-8,
        );
    }

    let mut fixed_x = Sketch::new(1.0).unwrap();
    let point_x = fixed_x.add_named_point("P", Point2::new(5.0, 7.0)).unwrap();
    fixed_x
        .add_fixed_coordinate(point_x, CoordinateAxis::X, 2.0)
        .unwrap();
    let x_result = fixed_x
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&x_result);

    let mut rotated = Sketch::new(1.0).unwrap();
    let point_y = rotated
        .add_named_point("P", Point2::new(-7.0, 5.0))
        .unwrap();
    rotated
        .add_fixed_coordinate(point_y, CoordinateAxis::Y, 2.0)
        .unwrap();
    let y_result = rotated
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&y_result);
    let x_solved = x_result.geometry.point(point_x).unwrap();
    assert_point(
        y_result.geometry.point(point_y).unwrap(),
        Point2::new(-x_solved.y, x_solved.x),
        1.0,
        2.0e-9,
    );

    for (scale, translation) in [
        (1.0e-6, [4.0e-6, -3.0e-6]),
        (1.0, [20.0, -8.0]),
        (1.0e6, [-5.0e6, 9.0e6]),
    ] {
        let mut sketch = Sketch::new(scale).unwrap();
        let initial = Point2::new(translation[0] + 7.0 * scale, translation[1] - 3.0 * scale);
        let target_x = translation[0] + 2.0 * scale;
        let point = sketch.add_named_point("P", initial).unwrap();
        sketch
            .add_fixed_coordinate(point, CoordinateAxis::X, target_x)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_point(
            result.geometry.point(point).unwrap(),
            Point2::new(target_x, initial.y),
            scale,
            2.0e-9,
        );
    }
}

#[test]
fn segment_length_recovery_is_similarity_metamorphic() {
    let first = Point2::new(1.0, -2.0);
    let second = Point2::new(3.0, 1.0);
    for (scale, angle, translation) in [
        (1.0e-6, -0.6, [5.0e-6, 2.0e-6]),
        (1.0, 0.9, [11.0, -7.0]),
        (1.0e6, 1.4, [-4.0e6, 3.0e6]),
    ] {
        let mut sketch = Sketch::new(scale).unwrap();
        let a = sketch
            .add_named_point("A", transform(first, scale, angle, translation))
            .unwrap();
        let b = sketch
            .add_named_point("B", transform(second, scale, angle, translation))
            .unwrap();
        let segment = sketch.add_named_segment("AB", a, b).unwrap();
        sketch
            .add_segment_length(segment, 4.0 * scale, DimensionMode::Driving)
            .unwrap();
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_length(
            result.geometry.point(a).unwrap(),
            result.geometry.point(b).unwrap(),
            4.0 * scale,
            scale,
        );
        assert!(!sketch.segment_has_enforced_branch(segment).unwrap());
    }
}

#[test]
fn driving_reference_toggle_preserves_id_and_order_but_changes_rows_and_dof() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(3.0, 0.0)).unwrap();
    let segment = sketch.add_named_segment("AB", a, b).unwrap();
    sketch.add_fixed_point(a).unwrap();
    let dimension = sketch
        .add_segment_length(segment, 4.0, DimensionMode::Driving)
        .unwrap();

    let driving_compile = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    assert_eq!(driving_compile.problem().audit_rows().unwrap().len(), 3);
    let driving_order: Vec<_> = driving_compile
        .source_mappings()
        .iter()
        .map(|mapping| mapping.source)
        .collect();
    let driving = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&driving);
    assert_eq!(driving.core_report.local_degrees_of_freedom, 1);

    sketch
        .set_dimension_mode(dimension, DimensionMode::Reference)
        .unwrap();
    assert_eq!(
        sketch.dimension(dimension).unwrap().mode(),
        DimensionMode::Reference
    );
    let reference_compile = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    assert_eq!(reference_compile.problem().audit_rows().unwrap().len(), 2);
    let reference_order: Vec<_> = reference_compile
        .source_mappings()
        .iter()
        .map(|mapping| mapping.source)
        .collect();
    assert_eq!(driving_order, reference_order);
    let mapping = reference_compile
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Dimension(dimension))
        .unwrap();
    assert!(mapping.core_source_id.is_none());
    assert!(mapping.residual_ids.is_empty());

    let reference = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&reference);
    assert_eq!(reference.core_report.local_degrees_of_freedom, 2);
    assert_eq!(reference.reference_values.len(), 1);
    assert_eq!(reference.reference_values[0].dimension_id, dimension);
    assert!((reference.reference_values[0].value - 4.0).abs() <= 4.0e-9);

    sketch
        .set_dimension_mode(dimension, DimensionMode::Driving)
        .unwrap();
    assert_eq!(
        sketch.dimension(dimension).unwrap().mode(),
        DimensionMode::Driving
    );
}

#[test]
fn point_distance_toggle_preserves_its_domain_id_and_reports_reference_value() {
    let mut sketch = Sketch::new(5.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(3.0, 4.0)).unwrap();
    let dimension = sketch
        .add_point_distance(a, b, 5.0, DimensionMode::Driving)
        .unwrap();
    let driving = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let driving_mapping = driving
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Dimension(dimension))
        .unwrap();
    assert!(driving_mapping.core_source_id.is_some());

    sketch
        .set_dimension_mode(dimension, DimensionMode::Reference)
        .unwrap();
    sketch.set_dimension_target(dimension, 9.0).unwrap();
    let reference = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&reference);
    assert_eq!(
        sketch.dimension(dimension).unwrap().mode(),
        DimensionMode::Reference
    );
    assert_eq!(reference.reference_values[0].dimension_id, dimension);
    assert!((reference.reference_values[0].value - 5.0).abs() <= 5.0e-9);
    let reference_mapping = reference
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::Dimension(dimension))
        .unwrap();
    assert!(reference_mapping.core_source_id.is_none());
    assert_eq!(
        reference_mapping.source_label,
        "dimension 1: reference measurement of distance A-B"
    );
    assert!(!reference_mapping.source_label.contains('9'));
}

#[test]
fn invalid_scale_stale_ids_and_nonfinite_edits_are_rejected_without_mutation() {
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            Sketch::new(scale),
            Err(SketchError::InvalidModelScale(_))
        ));
    }

    let mut sketch = Sketch::new(2.0).unwrap();
    let point = sketch.add_point(Point2::new(1.0, 2.0)).unwrap();
    let original = sketch.geometry();
    assert!(matches!(
        sketch.set_model_scale(f64::NEG_INFINITY),
        Err(SketchError::InvalidModelScale(_))
    ));
    assert!((sketch.model_scale() - 2.0).abs() <= f64::EPSILON);
    assert!(matches!(
        sketch.set_point_position(point, Point2::new(f64::NAN, 0.0)),
        Err(SketchError::NonFinitePoint { .. })
    ));
    assert_eq!(sketch.geometry(), original);

    let stale = sketch.add_point(Point2::new(8.0, 9.0)).unwrap();
    sketch.remove_point(stale).unwrap();
    assert!(matches!(
        sketch.set_point_position(stale, Point2::new(0.0, 0.0)),
        Err(SketchError::UnknownPoint(id)) if id == stale
    ));
    assert!(matches!(
        sketch.compile(SketchSolveRequest::default().with_drag(stale, Point2::new(0.0, 0.0))),
        Err(SketchError::UnknownPoint(id)) if id == stale
    ));

    let other = sketch.add_point(Point2::new(4.0, 2.0)).unwrap();
    let dimension = sketch
        .add_point_distance(point, other, 3.0, DimensionMode::Reference)
        .unwrap();
    sketch.remove_dimension(dimension).unwrap();
    assert!(matches!(
        sketch.set_dimension_mode(dimension, DimensionMode::Driving),
        Err(SketchError::UnknownDimension(id)) if id == dimension
    ));
}

#[test]
fn invalid_constraint_and_dimension_construction_paths_are_explicit() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(2.0, 0.0)).unwrap();
    let coincident_position = sketch.add_named_point("C", Point2::new(0.0, 0.0)).unwrap();

    assert!(matches!(
        sketch.add_fixed_point_at(a, Point2::new(f64::NAN, 0.0)),
        Err(SketchError::NonFinitePoint { .. })
    ));
    assert!(matches!(
        sketch.add_fixed_coordinate(a, CoordinateAxis::X, f64::INFINITY),
        Err(SketchError::NonFiniteValue { .. })
    ));
    assert_eq!(sketch.add_coincident(a, a), Err(SketchError::RepeatedPoint));
    assert_eq!(
        sketch.add_segment(a, a),
        Err(SketchError::DegenerateSegment)
    );
    assert_eq!(
        sketch.add_segment(a, coincident_position),
        Err(SketchError::DegenerateSegment)
    );
    assert_eq!(
        sketch.add_point_distance(a, coincident_position, 1.0, DimensionMode::Driving),
        Err(SketchError::DegenerateDistance)
    );

    let stale_segment = sketch.add_named_segment("stale", a, b).unwrap();
    sketch.remove_segment(stale_segment).unwrap();
    assert!(matches!(
        sketch.add_horizontal(stale_segment),
        Err(SketchError::UnknownSegment(id)) if id == stale_segment
    ));
    assert!(matches!(
        sketch.add_vertical(stale_segment),
        Err(SketchError::UnknownSegment(id)) if id == stale_segment
    ));

    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            sketch.add_point_distance(a, b, value, DimensionMode::Driving),
            Err(SketchError::InvalidDimensionValue(_))
        ));
    }
    assert_eq!(
        sketch.add_point_distance(a, a, 1.0, DimensionMode::Driving),
        Err(SketchError::RepeatedPoint)
    );
    let stale_point = sketch
        .add_named_point("stale point", Point2::new(8.0, 9.0))
        .unwrap();
    sketch.remove_point(stale_point).unwrap();
    assert!(matches!(
        sketch.add_point_distance(a, stale_point, 1.0, DimensionMode::Driving),
        Err(SketchError::UnknownPoint(id)) if id == stale_point
    ));

    let segment = sketch.add_named_segment("AB", a, b).unwrap();
    for value in [0.0, -1.0, f64::NEG_INFINITY, f64::NAN] {
        assert!(matches!(
            sketch.add_segment_length(segment, value, DimensionMode::Driving),
            Err(SketchError::InvalidDimensionValue(_))
        ));
    }
    sketch.set_point_position(b, Point2::new(0.0, 0.0)).unwrap();
    assert_eq!(
        sketch.add_horizontal(segment),
        Err(SketchError::DegenerateSegment)
    );
    assert_eq!(
        sketch.add_vertical(segment),
        Err(SketchError::DegenerateSegment)
    );
    assert_eq!(
        sketch.add_segment_length(segment, 1.0, DimensionMode::Driving),
        Err(SketchError::DegenerateSegment)
    );
}

#[test]
fn zero_length_distance_derivative_is_invalid_and_retains_geometry() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(1.0, 0.0)).unwrap();
    sketch
        .add_point_distance(a, b, 2.0, DimensionMode::Driving)
        .unwrap();
    sketch.set_point_position(b, Point2::new(0.0, 0.0)).unwrap();
    let retained = sketch.geometry();
    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    assert!(matches!(
        compiled.problem().check_jacobians(1.0e-5),
        Err(CoreError::CategorizedEvaluation {
            category: EvaluationErrorCategory::Nondifferentiable,
            ..
        })
    ));

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(!result.accepted());
    assert_eq!(
        result.rejection,
        Some(SolveRejection::CoreTermination(
            SolveTermination::InvalidGeometry
        ))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(sketch.geometry(), retained);
    let row = display_row(&result, "distance A-B");
    assert_point(
        incident_point(row, 0),
        result.geometry.point(a).unwrap(),
        1.0,
        0.0,
    );
    assert_point(
        incident_point(row, 1),
        result.geometry.point(b).unwrap(),
        1.0,
        0.0,
    );
    assert!((row.raw_residual + 2.0).abs() <= f64::EPSILON);
    assert!((row.normalized_residual + 2.0).abs() <= f64::EPSILON);
    assert_eq!(row.evaluation_status, AuditEvaluationStatus::Failed);
    assert_eq!(
        row.evaluation_error_category,
        Some(EvaluationErrorCategory::Nondifferentiable)
    );
    assert!(
        row.evaluation_error
            .as_deref()
            .unwrap()
            .contains("distance derivative")
    );
    let attempted_row = audit_row(&result.core_report.audit, "distance A-B");
    assert_eq!(
        row.annotations.conflicting,
        attempted_row.annotations.conflicting
    );
    assert_eq!(
        attempted_row.evaluation_status,
        AuditEvaluationStatus::Failed
    );
}

#[test]
fn explicit_segment_branch_rejects_a_converged_flipped_root_without_committing() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(4.0, 0.0)).unwrap();
    let segment = sketch.add_named_segment("AB", a, b).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch.add_horizontal(segment).unwrap();
    sketch
        .add_segment_length(segment, 4.0, DimensionMode::Driving)
        .unwrap();
    assert!(sketch.segment_has_enforced_branch(segment).unwrap());

    sketch
        .set_point_position(b, Point2::new(-3.0, 0.0))
        .unwrap();
    assert!(!sketch.segment_branch_is_preserved(segment).unwrap());
    let retained = sketch.geometry();
    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.core_report.hard_validity, HardValidity::Invalid);
    assert!(result.core_report.hard_residuals_validated);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::SegmentBranchFlipped(segment))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(sketch.geometry(), retained);
    let row = display_row(&result, "length AB");
    assert_point(
        incident_point(row, 0),
        result.geometry.point(a).unwrap(),
        4.0,
        0.0,
    );
    assert_point(
        incident_point(row, 1),
        result.geometry.point(b).unwrap(),
        4.0,
        0.0,
    );
    assert!((row.raw_residual + 1.0).abs() <= f64::EPSILON);
    assert!((row.normalized_residual + 0.25).abs() <= f64::EPSILON);
    assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
    assert_eq!(row.evaluation_error, None);
    assert!(!row.annotations.conflicting);
    let attempted_row = audit_row(&result.core_report.audit, "length AB");
    assert!(attempted_row.raw_residual.abs() <= TOLERANCE);
    assert_point(
        incident_point(attempted_row, 1),
        Point2::new(-4.0, 0.0),
        4.0,
        TOLERANCE,
    );

    sketch.reselect_segment_branch(segment).unwrap();
    assert!(sketch.segment_branch_is_preserved(segment).unwrap());
    let accepted = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&accepted);
}

#[test]
fn length_only_segment_can_rotate_through_a_full_turn_without_branch_rejection() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(2.0, 0.0)).unwrap();
    let segment = sketch.add_named_segment("AB", a, b).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch
        .add_segment_length(segment, 2.0, DimensionMode::Driving)
        .unwrap();
    assert!(!sketch.segment_has_enforced_branch(segment).unwrap());

    let mut crossed_initial_half_plane = false;
    for step in 0..=16 {
        let angle = f64::from(step) * std::f64::consts::TAU / 16.0;
        let target = Point2::new(2.0 * angle.cos(), 2.0 * angle.sin());
        let result = sketch
            .solve(
                SketchSolveRequest::default().with_drag(b, target),
                SolverConfig::default(),
            )
            .unwrap();
        assert_accepted(&result);
        assert_point(result.geometry.point(b).unwrap(), target, 2.0, 5.0e-8);
        if !sketch.segment_branch_is_preserved(segment).unwrap() {
            crossed_initial_half_plane = true;
        }
    }
    assert!(crossed_initial_half_plane);
    assert_point(
        sketch.point(b).unwrap().position(),
        Point2::new(2.0, 0.0),
        2.0,
        5.0e-8,
    );
}
