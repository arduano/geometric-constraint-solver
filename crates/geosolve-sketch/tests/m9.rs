// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AuditEvaluationStatus, HardValidity, ResidualCategory, SecondaryStatus, SolveTermination,
    SolverConfig,
};
use geosolve_geometry::Point2;
use geosolve_sketch::{DimensionMode, Sketch, SketchSolveRequest, SketchSource, SolveRejection};

#[test]
fn domain_valid_secondary_iteration_limit_keeps_authoritative_hard_validity() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let point = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let retained = sketch.geometry();
    let request = SketchSolveRequest::default()
        .without_previous_state_preferences()
        .with_drag(point, Point2::new(1.0, 0.0));

    let result = sketch
        .solve(
            request,
            SolverConfig {
                max_iterations: 1,
                ..SolverConfig::default()
            },
        )
        .unwrap();

    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert_eq!(
        result.unstable_core_report().temporary_status,
        SecondaryStatus::IterationLimit
    );
    assert_eq!(
        result.rejection,
        Some(SolveRejection::CoreTermination(
            SolveTermination::IterationLimit
        ))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(sketch.geometry(), retained);
}

#[test]
fn branch_validation_precedes_compatibility_secondary_iteration_limit() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let end = sketch.add_point(Point2::new(4.0, 0.0)).unwrap();
    let dragged = sketch.add_point(Point2::new(0.0, 1.0)).unwrap();
    let segment = sketch.add_segment(start, end).unwrap();
    sketch.add_fixed_point(start).unwrap();
    sketch.add_horizontal(segment).unwrap();
    sketch
        .add_segment_length(segment, 4.0, DimensionMode::Driving)
        .unwrap();
    sketch
        .set_point_position(end, Point2::new(-4.0, 0.0))
        .unwrap();
    let retained = sketch.geometry();
    let request = SketchSolveRequest::default()
        .without_previous_state_preferences()
        .with_drag(dragged, Point2::new(2.0, 1.0));

    let result = sketch
        .solve(
            request,
            SolverConfig {
                max_iterations: 1,
                ..SolverConfig::default()
            },
        )
        .unwrap();

    assert_eq!(
        result.unstable_core_report().temporary_status,
        SecondaryStatus::IterationLimit
    );
    assert_eq!(
        result.unstable_core_report().termination,
        SolveTermination::IterationLimit
    );
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Invalid
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::SegmentBranchFlipped(segment))
    );
    assert_eq!(result.geometry, retained);
}

#[test]
#[allow(clippy::too_many_lines)]
fn tangent_distance_gradients_report_scale_invariant_numerical_dependence() {
    let mut classifications = Vec::new();
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let first_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let second_center = sketch.add_point(Point2::new(2.0 * scale, 0.0)).unwrap();
        let point = sketch.add_point(Point2::new(scale, 0.0)).unwrap();
        sketch.add_fixed_point(first_center).unwrap();
        sketch.add_fixed_point(second_center).unwrap();
        let first_distance = sketch
            .add_point_distance(first_center, point, scale, DimensionMode::Driving)
            .unwrap();
        let second_distance = sketch
            .add_point_distance(second_center, point, scale, DimensionMode::Driving)
            .unwrap();

        let result = sketch
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap();

        assert!(
            result.accepted(),
            "scale={scale:e}: {:#?}",
            result.rejection
        );
        assert_eq!(
            result.unstable_core_report().hard_validity,
            HardValidity::Valid
        );
        assert!(result.unstable_core_report().hard_residuals_validated);
        assert_eq!(
            result.unstable_core_report().hard_residual_max.to_bits(),
            0.0_f64.to_bits()
        );
        let solved = result.geometry.point(point).unwrap();
        assert!(solved.coords.iter().all(|value| value.is_finite()));
        assert!((solved.x / scale - 1.0).abs() <= 1.0e-12);
        assert!((solved.y / scale).abs() <= 1.0e-12);

        assert_eq!(result.unstable_core_report().rank, 1);
        assert_eq!(result.unstable_core_report().left_nullity, 1);
        assert_eq!(result.unstable_core_report().right_nullity, 1);
        assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 1);
        assert!(result.unstable_core_report().is_singular);
        assert!(!result.unstable_core_report().near_singular);
        let component = result
            .unstable_core_report()
            .component_solves
            .iter()
            .find(|component| {
                let structural = &result.unstable_core_report().structural.component_summaries
                    [component.component_index];
                structural.active_tangent_dimensions == 2 && structural.active_hard_rows == 2
            })
            .unwrap();
        assert!(component.rank_is_valid);
        assert_eq!(component.rank, 1);
        assert_eq!((component.left_nullity, component.right_nullity), (1, 1));
        assert!(component.is_singular);
        assert!(!component.near_singular);
        assert!(component.rank_machine_tolerance.is_finite());
        assert!(component.rank_machine_tolerance > 0.0);
        assert!(component.rank_threshold.is_finite());
        assert!(component.rank_threshold >= component.rank_machine_tolerance);
        assert!(component.sigma_max.is_finite());
        assert!((component.sigma_max - 2.0_f64.sqrt()).abs() <= 2.0e-15);

        for dimension in [first_distance, second_distance] {
            let mapping = result
                .source_mappings
                .iter()
                .find(|mapping| mapping.source == SketchSource::Dimension(dimension))
                .unwrap();
            assert_eq!(mapping.residual_ids.len(), 1);
            let source_id = mapping.core_source_id.unwrap();
            let audit_source = result
                .unstable_core_report()
                .audit
                .sources
                .iter()
                .find(|source| source.source_id == source_id)
                .unwrap();
            assert_eq!(audit_source.source_label, mapping.source_label);
            assert_eq!(audit_source.rows.len(), 1);
            let row = &audit_source.rows[0];
            assert_eq!(row.residual_id, mapping.residual_ids[0]);
            assert_eq!(row.category, ResidualCategory::Hard);
            assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
            assert_eq!(row.raw_residual.to_bits(), 0.0_f64.to_bits());
            assert_eq!(row.normalized_residual.to_bits(), 0.0_f64.to_bits());
            assert_eq!(row.incident_variables.len(), 2);
        }

        classifications.push((
            result.unstable_core_report().rank,
            result.unstable_core_report().left_nullity,
            result.unstable_core_report().right_nullity,
            result.unstable_core_report().is_singular,
            component.rank_machine_tolerance,
            component.rank_threshold,
        ));
    }

    for classification in &classifications[1..] {
        assert_eq!(
            (
                classification.0,
                classification.1,
                classification.2,
                classification.3,
            ),
            (
                classifications[0].0,
                classifications[0].1,
                classifications[0].2,
                classifications[0].3,
            )
        );
        assert_eq!(classification.4.to_bits(), classifications[0].4.to_bits());
        assert_eq!(classification.5.to_bits(), classifications[0].5.to_bits());
    }
}
