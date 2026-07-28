#![allow(clippy::too_many_lines)]

use std::f64::consts::{FRAC_PI_2, PI};

use geosolve_core::{
    AuditEvaluationStatus, EvaluationErrorCategory, HardValidity, SolveTermination, SolverConfig,
};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ArcCircleTangencySide, ArcId, ArcSweep, CircleId, ContactState, DimensionKind, DimensionMode,
    LatentVariableRole, PointId, Sketch, SketchConstraintId, SketchError, SketchSolveRequest,
    SketchSource, SolveRejection,
};

const TOLERANCE: f64 = 1.0e-9;

#[derive(Clone, Copy)]
struct FixtureIds {
    arc_center: PointId,
    circle_center: PointId,
    circle: CircleId,
    arc: ArcId,
    tangency: SketchConstraintId,
}

fn fixture(side: ArcCircleTangencySide, perturbed: bool) -> (Sketch, FixtureIds) {
    let mut sketch = Sketch::new(2.0).unwrap();
    let arc_center = sketch
        .add_named_point("arc center", Point2::new(0.0, 0.0))
        .unwrap();
    let center_x = match side {
        ArcCircleTangencySide::OutsideArc => 3.0,
        ArcCircleTangencySide::InsideArc => 1.0,
    };
    let circle_center = sketch
        .add_named_point("circle center", Point2::new(center_x, 0.0))
        .unwrap();
    let circle = sketch
        .add_named_circle(
            "free-radius circle",
            circle_center,
            if perturbed { 0.72 } else { 1.0 },
        )
        .unwrap();
    let arc = sketch
        .add_named_arc(
            "broad arc",
            arc_center,
            2.0,
            -3.0 * PI / 4.0,
            3.0 * PI / 4.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(arc_center).unwrap();
    sketch
        .add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let exact_circle_angle = match side {
        ArcCircleTangencySide::OutsideArc => PI,
        ArcCircleTangencySide::InsideArc => 0.0,
    };
    let tangency = sketch
        .add_circle_arc_tangency(
            circle,
            arc,
            side,
            if perturbed { 0.46 } else { 0.5 },
            if perturbed {
                exact_circle_angle + 0.18
            } else {
                exact_circle_angle
            },
        )
        .unwrap();
    (
        sketch,
        FixtureIds {
            arc_center,
            circle_center,
            circle,
            arc,
            tangency,
        },
    )
}

fn solve(sketch: &mut Sketch) -> geosolve_sketch::SketchSolveResult {
    sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap()
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
    assert_eq!(result.display_audit, result.unstable_core_report().audit);
}

fn contact(sketch: &Sketch, constraint: SketchConstraintId) -> (f64, f64) {
    let ContactState::CircleArcTangency {
        arc_span_parameter,
        circle_angle,
    } = sketch.contact_state(constraint).unwrap()
    else {
        panic!("wrong contact state")
    };
    (arc_span_parameter, circle_angle)
}

fn assert_tangent_geometry(
    sketch: &Sketch,
    result: &geosolve_sketch::SketchSolveResult,
    ids: FixtureIds,
    scale: f64,
) {
    let solved_circle = result.geometry.circle(ids.circle).unwrap();
    let solved_arc = result.geometry.arc(ids.arc).unwrap();
    let (span, angle) = contact(sketch, ids.tangency);
    assert!((0.0..=1.0).contains(&span));
    let circle_contact = solved_circle.evaluate(angle).unwrap();
    let arc_contact = solved_arc.evaluate(span).unwrap();
    assert!((circle_contact - arc_contact).norm() / scale <= TOLERANCE);
    let center_distance = (solved_circle.center - solved_arc.center).norm();
    assert!(
        (solved_circle.radius - (center_distance - solved_arc.radius).abs()).abs() / scale
            <= TOLERANCE
    );
}

#[test]
fn exact_outside_and_inside_fixtures_converge_with_two_local_dof_and_no_circle_driver() {
    for side in [
        ArcCircleTangencySide::OutsideArc,
        ArcCircleTangencySide::InsideArc,
    ] {
        let (mut sketch, ids) = fixture(side, false);
        assert_eq!(sketch.circle_arc_tangency_side(ids.tangency).unwrap(), side);
        assert_eq!(
            sketch.constraint(ids.tangency).unwrap().kind(),
            geosolve_sketch::SketchConstraintKind::CircleArcTangency {
                circle: ids.circle,
                arc: ids.arc,
                side,
                arc_span_parameter: 0.5,
                circle_angle: if side == ArcCircleTangencySide::OutsideArc {
                    PI
                } else {
                    0.0
                },
            }
        );

        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        assert_eq!(compiled.circle_radius_variables().len(), 1);
        assert_eq!(compiled.arc_radius_variables().len(), 1);
        assert_eq!(sketch.dimensions().count(), 1);
        assert!(matches!(
            sketch.dimensions().next().unwrap().1.kind(),
            DimensionKind::ArcRadius { arc, target: 2.0 } if arc == ids.arc
        ));
        assert!(
            compiled
                .problem()
                .audit_rows()
                .unwrap()
                .iter()
                .all(|row| !row.template.contains("circle.radius - target"))
        );

        let result = solve(&mut sketch);
        assert_accepted(&result);
        assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 2);
        assert_eq!(
            result.geometry.point(ids.arc_center).unwrap(),
            Point2::new(0.0, 0.0)
        );
        let expected_center = if side == ArcCircleTangencySide::OutsideArc {
            Point2::new(3.0, 0.0)
        } else {
            Point2::new(1.0, 0.0)
        };
        assert!(
            (result.geometry.point(ids.circle_center).unwrap() - expected_center).norm()
                <= TOLERANCE
        );
        assert!((result.geometry.circle(ids.circle).unwrap().radius - 1.0).abs() <= TOLERANCE);
        let (span, angle) = contact(&sketch, ids.tangency);
        let expected_angle = if side == ArcCircleTangencySide::OutsideArc {
            PI
        } else {
            0.0
        };
        assert!((span - 0.5).abs() <= TOLERANCE);
        assert!((angle - expected_angle).abs() <= TOLERANCE);
        assert_tangent_geometry(&sketch, &result, ids, 2.0);
    }
}

#[test]
fn perturbed_radius_and_contacts_recover_without_a_circle_radius_equation() {
    for side in [
        ArcCircleTangencySide::OutsideArc,
        ArcCircleTangencySide::InsideArc,
    ] {
        let (mut sketch, ids) = fixture(side, true);
        let initial_center = sketch.point(ids.circle_center).unwrap().position();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 2);
        assert!(
            (result.geometry.point(ids.circle_center).unwrap() - initial_center).norm() <= 5.0e-9
        );
        assert!((result.geometry.circle(ids.circle).unwrap().radius - 1.0).abs() <= 5.0e-9);
        assert_tangent_geometry(&sketch, &result, ids, 2.0);
    }

    let (mut periodic, ids) = fixture(ArcCircleTangencySide::OutsideArc, true);
    periodic
        .set_contact_state(
            ids.tangency,
            ContactState::CircleArcTangency {
                arc_span_parameter: 0.46,
                circle_angle: 3.0 * PI + 0.18,
            },
        )
        .unwrap();
    let result = solve(&mut periodic);
    assert_accepted(&result);
    let (_, angle) = contact(&periodic, ids.tangency);
    assert!((angle - 3.0 * PI).abs() <= 5.0e-9);
    assert_tangent_geometry(&periodic, &result, ids, 2.0);
}

#[test]
fn temporary_center_drags_solve_radius_and_contacts_then_release_with_two_dof() {
    let (mut sketch, ids) = fixture(ArcCircleTangencySide::OutsideArc, false);
    assert_accepted(&solve(&mut sketch));

    for (distance, angle) in [
        (3.2, 0.2_f64),
        (3.0, 0.4),
        (2.8, 0.15),
        (3.3, -0.15),
        (3.7, -0.4),
        (4.1, -0.6),
    ] {
        let target = Point2::new(distance * angle.cos(), distance * angle.sin());
        let dragged = sketch
            .solve(
                SketchSolveRequest::default().with_drag(ids.circle_center, target),
                SolverConfig::default(),
            )
            .unwrap();
        assert!(
            dragged.accepted(),
            "drag distance={distance}, angle={angle}: rejection={:#?}, report={:#?}",
            dragged.rejection,
            dragged.unstable_core_report()
        );
        assert_accepted(&dragged);
        assert_eq!(dragged.unstable_core_report().local_degrees_of_freedom, 2);
        assert!((dragged.geometry.point(ids.circle_center).unwrap() - target).norm() <= 5.0e-9);
        assert!(
            (dragged.geometry.circle(ids.circle).unwrap().radius - (distance - 2.0)).abs()
                <= 5.0e-9
        );
        assert_tangent_geometry(&sketch, &dragged, ids, 2.0);

        let released = solve(&mut sketch);
        assert_accepted(&released);
        assert_eq!(released.unstable_core_report().local_degrees_of_freedom, 2);
        assert!((released.geometry.point(ids.circle_center).unwrap() - target).norm() <= 5.0e-9);
        assert!(
            (released.geometry.circle(ids.circle).unwrap().radius - (distance - 2.0)).abs()
                <= 5.0e-9
        );
    }
}

fn transform(point: Point2<f64>, scale: f64, angle: f64, offset: [f64; 2]) -> Point2<f64> {
    let (sine, cosine) = angle.sin_cos();
    Point2::new(
        scale * (cosine * point.x - sine * point.y) + offset[0],
        scale * (sine * point.x + cosine * point.y) + offset[1],
    )
}

#[test]
fn similarity_transforms_preserve_both_sides_and_direct_geometric_oracles() {
    for (scale, rotation, offset) in [
        (1.0e-6, -0.55, [3.0e-6, 7.0e-6]),
        (1.0, 0.65, [11.0, -4.0]),
        (1.0e6, 1.15, [-5.0e6, 2.0e6]),
    ] {
        for side in [
            ArcCircleTangencySide::OutsideArc,
            ArcCircleTangencySide::InsideArc,
        ] {
            let distance = if side == ArcCircleTangencySide::OutsideArc {
                3.0
            } else {
                1.0
            };
            let mut sketch = Sketch::new(scale).unwrap();
            let arc_center = sketch
                .add_point(transform(Point2::new(0.0, 0.0), scale, rotation, offset))
                .unwrap();
            let circle_center = sketch
                .add_point(transform(
                    Point2::new(distance, 0.0),
                    scale,
                    rotation,
                    offset,
                ))
                .unwrap();
            let circle = sketch.add_circle(circle_center, scale).unwrap();
            let arc = sketch
                .add_arc(
                    arc_center,
                    2.0 * scale,
                    rotation - 3.0 * PI / 4.0,
                    rotation + 3.0 * PI / 4.0,
                    ArcSweep::CounterClockwise,
                )
                .unwrap();
            sketch.add_fixed_point(arc_center).unwrap();
            sketch
                .add_arc_radius(arc, 2.0 * scale, DimensionMode::Driving)
                .unwrap();
            let tangency = sketch
                .add_circle_arc_tangency(
                    circle,
                    arc,
                    side,
                    0.5,
                    rotation
                        + if side == ArcCircleTangencySide::OutsideArc {
                            PI
                        } else {
                            0.0
                        },
                )
                .unwrap();
            let result = solve(&mut sketch);
            assert_accepted(&result);
            assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 2);
            let expected_center = transform(Point2::new(distance, 0.0), scale, rotation, offset);
            assert!(
                (result.geometry.point(circle_center).unwrap() - expected_center).norm() / scale
                    <= 3.0e-9
            );
            assert!(
                (result.geometry.circle(circle).unwrap().radius - scale).abs() / scale <= 3.0e-9
            );
            let ids = FixtureIds {
                arc_center,
                circle_center,
                circle,
                arc,
                tangency,
            };
            assert_tangent_geometry(&sketch, &result, ids, scale);
        }
    }
}

#[test]
fn analytic_jacobian_audit_source_and_variable_order_are_deterministic() {
    let (sketch, ids) = fixture(ArcCircleTangencySide::OutsideArc, true);
    let request = SketchSolveRequest::default().without_previous_state_preferences();
    let compiled = sketch.compile(request).unwrap();
    let repeated = sketch.compile(request).unwrap();
    assert_eq!(compiled.latent_variables(), repeated.latent_variables());
    assert_eq!(compiled.source_mappings(), repeated.source_mappings());

    let tangency_latents: Vec<_> = compiled
        .latent_variables()
        .iter()
        .filter(|mapping| mapping.constraint_id == ids.tangency)
        .copied()
        .collect();
    assert_eq!(
        tangency_latents
            .iter()
            .map(|mapping| mapping.role)
            .collect::<Vec<_>>(),
        vec![
            LatentVariableRole::CircleAngle,
            LatentVariableRole::ArcSpanParameter,
        ]
    );
    let source_mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(ids.tangency))
        .unwrap();
    let residual = compiled
        .problem()
        .residual(source_mapping.residual_ids[0])
        .unwrap();
    assert_eq!(
        residual.incident_variables(),
        &[
            compiled.variable_for_point(ids.circle_center).unwrap(),
            compiled.variable_for_circle_radius(ids.circle).unwrap(),
            compiled.variable_for_point(ids.arc_center).unwrap(),
            compiled.variable_for_arc_radius(ids.arc).unwrap(),
            tangency_latents[0].variable_id,
            tangency_latents[1].variable_id,
        ]
    );

    let rows: Vec<_> = compiled
        .problem()
        .audit_rows()
        .unwrap()
        .into_iter()
        .filter(|row| Some(row.source_id) == source_mapping.core_source_id)
        .collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().map(|row| row.scale).collect::<Vec<_>>(),
        vec![2.0, 2.0, 1.0]
    );
    assert!(rows[0].template.contains("circle(angle).x - arc(u).x"));
    assert!(rows[1].template.contains("circle(angle).y - arc(u).y"));
    assert!(rows[2].template.contains("unit_tangent"));
    assert!(rows.iter().all(|row| {
        row.scale.is_finite()
            && row.scale > 0.0
            && row
                .bindings
                .iter()
                .any(|binding| binding.name == "side" && binding.value == "OutsideArc")
            && row
                .bindings
                .iter()
                .any(|binding| binding.name == "warm-start circle angle")
            && row
                .bindings
                .iter()
                .any(|binding| binding.name == "warm-start arc span")
    }));

    let report = compiled.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        report.all_within(1.0e-6),
        "max FD error={:e}: {report:#?}",
        report.max_relative_error()
    );

    let mut clockwise = Sketch::new(2.0).unwrap();
    let arc_center = clockwise.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = clockwise.add_point(Point2::new(3.0, 0.0)).unwrap();
    let circle = clockwise.add_circle(circle_center, 0.8).unwrap();
    let arc = clockwise
        .add_arc(
            arc_center,
            2.0,
            3.0 * PI / 4.0,
            -3.0 * PI / 4.0,
            ArcSweep::Clockwise,
        )
        .unwrap();
    clockwise
        .add_circle_arc_tangency(
            circle,
            arc,
            ArcCircleTangencySide::OutsideArc,
            0.46,
            PI + 0.18,
        )
        .unwrap();
    let compiled = clockwise
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let report = compiled.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        report.all_within(1.0e-6),
        "clockwise max FD error={:e}: {report:#?}",
        report.max_relative_error()
    );
}

fn tiny_fixed_fixture(
    side: ArcCircleTangencySide,
    circle_radius: f64,
    arc_span_parameter: f64,
    circle_angle: f64,
) -> (Sketch, FixtureIds) {
    let arc_radius = 2.0e-12;
    let center_distance = match side {
        ArcCircleTangencySide::OutsideArc => 3.0e-12,
        ArcCircleTangencySide::InsideArc => 1.0e-12,
    };
    let mut sketch = Sketch::new(1.0).unwrap();
    let arc_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = sketch.add_point(Point2::new(center_distance, 0.0)).unwrap();
    let circle = sketch.add_circle(circle_center, circle_radius).unwrap();
    let arc = sketch
        .add_arc(
            arc_center,
            arc_radius,
            -FRAC_PI_2,
            FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(arc_center).unwrap();
    sketch.add_fixed_point(circle_center).unwrap();
    sketch
        .add_arc_radius(arc, arc_radius, DimensionMode::Driving)
        .unwrap();
    sketch
        .add_circle_radius(circle, circle_radius, DimensionMode::Driving)
        .unwrap();
    let tangency = sketch
        .add_circle_arc_tangency(circle, arc, side, arc_span_parameter, circle_angle)
        .unwrap();
    (
        sketch,
        FixtureIds {
            arc_center,
            circle_center,
            circle,
            arc,
            tangency,
        },
    )
}

fn mixed_scale_fixture(
    side: ArcCircleTangencySide,
    circle_radius: f64,
) -> (Sketch, FixtureIds, f64) {
    let arc_radius = 1.0_f64;
    let center_distance = match side {
        ArcCircleTangencySide::OutsideArc => 1.0 + 1.0e-15,
        ArcCircleTangencySide::InsideArc => 1.0 - 1.0e-15,
    };
    let derived_gap = (center_distance - arc_radius).abs();
    let mut sketch = Sketch::new(1.0).unwrap();
    let arc_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = sketch.add_point(Point2::new(center_distance, 0.0)).unwrap();
    let circle = sketch.add_circle(circle_center, circle_radius).unwrap();
    let arc = sketch
        .add_arc(
            arc_center,
            arc_radius,
            -FRAC_PI_2,
            FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(arc_center).unwrap();
    sketch.add_fixed_point(circle_center).unwrap();
    sketch
        .add_arc_radius(arc, arc_radius, DimensionMode::Driving)
        .unwrap();
    sketch
        .add_circle_radius(circle, circle_radius, DimensionMode::Driving)
        .unwrap();
    let circle_angle = if side == ArcCircleTangencySide::OutsideArc {
        PI
    } else {
        0.0
    };
    let tangency = sketch
        .add_circle_arc_tangency(circle, arc, side, 0.5, circle_angle)
        .unwrap();
    (
        sketch,
        FixtureIds {
            arc_center,
            circle_center,
            circle,
            arc,
            tangency,
        },
        derived_gap,
    )
}

#[test]
fn tiny_feature_radius_and_radial_root_mismatches_reject_independently_and_roll_back() {
    for side in [
        ArcCircleTangencySide::OutsideArc,
        ArcCircleTangencySide::InsideArc,
    ] {
        let exact_circle_angle = if side == ArcCircleTangencySide::OutsideArc {
            PI
        } else {
            0.0
        };
        let (mut wrong_radius, ids) = tiny_fixed_fixture(side, 0.2e-12, 0.5, exact_circle_angle);
        let retained_geometry = wrong_radius.geometry();
        let retained_contact = wrong_radius.contact_state(ids.tangency).unwrap();
        let retained_audit = wrong_radius
            .compile(SketchSolveRequest::default())
            .unwrap()
            .problem()
            .audit_snapshot()
            .unwrap();
        let result = solve(&mut wrong_radius);
        assert_eq!(
            result.unstable_core_report().termination,
            SolveTermination::Converged
        );
        assert!(result.unstable_core_report().hard_residuals_validated);
        assert!(result.unstable_core_report().hard_residual_max < TOLERANCE);
        assert!(result.acceptance_hard_residual_max.unwrap() < TOLERANCE);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::InvalidTangencyMode(ids.tangency))
        );
        assert_eq!(result.geometry, retained_geometry);
        assert_eq!(
            wrong_radius.contact_state(ids.tangency).unwrap(),
            retained_contact
        );
        assert_eq!(result.display_audit, retained_audit);
        assert_eq!(
            result.geometry.circle(ids.circle).unwrap().radius.to_bits(),
            0.2e-12_f64.to_bits()
        );

        let radial_angle = FRAC_PI_2 - 1.0e-4;
        let span = (radial_angle + FRAC_PI_2) / PI;
        let circle_angle = radial_angle
            + if side == ArcCircleTangencySide::OutsideArc {
                PI
            } else {
                0.0
            };
        let (mut wrong_root, ids) = tiny_fixed_fixture(side, 1.0e-12, span, circle_angle);
        let retained_geometry = wrong_root.geometry();
        let retained_contact = wrong_root.contact_state(ids.tangency).unwrap();
        let retained_audit = wrong_root
            .compile(SketchSolveRequest::default())
            .unwrap()
            .problem()
            .audit_snapshot()
            .unwrap();
        let result = solve(&mut wrong_root);
        assert_eq!(
            result.unstable_core_report().termination,
            SolveTermination::Converged
        );
        assert!(result.unstable_core_report().hard_residuals_validated);
        assert!(result.unstable_core_report().hard_residual_max < TOLERANCE);
        assert!(result.acceptance_hard_residual_max.unwrap() < TOLERANCE);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::CenterDirectionFlipped(ids.tangency))
        );
        assert_eq!(result.geometry, retained_geometry);
        assert_eq!(
            wrong_root.contact_state(ids.tangency).unwrap(),
            retained_contact
        );
        assert_eq!(result.display_audit, retained_audit);

        let (mut exact, ids) = tiny_fixed_fixture(side, 1.0e-12, 0.5, exact_circle_angle);
        let result = solve(&mut exact);
        assert_accepted(&result);
        assert_tangent_geometry(&exact, &result, ids, 2.0e-12);
    }
}

#[test]
fn mixed_scale_circle_gaps_reject_mismatch_and_unresolvable_exact_state() {
    for side in [
        ArcCircleTangencySide::OutsideArc,
        ArcCircleTangencySide::InsideArc,
    ] {
        let (mut wrong, ids, derived_gap) = mixed_scale_fixture(side, 2.0e-14);
        assert!(derived_gap > 0.0 && derived_gap < 2.0e-15);
        let retained_geometry = wrong.geometry();
        let retained_contact = wrong.contact_state(ids.tangency).unwrap();
        let retained_audit = wrong
            .compile(SketchSolveRequest::default())
            .unwrap()
            .problem()
            .audit_snapshot()
            .unwrap();
        let result = solve(&mut wrong);
        assert_eq!(
            result.unstable_core_report().termination,
            SolveTermination::Converged
        );
        assert!(result.unstable_core_report().hard_residuals_validated);
        assert!(result.unstable_core_report().hard_residual_max < TOLERANCE);
        assert!(result.acceptance_hard_residual_max.unwrap() < TOLERANCE);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::AmbiguousTangencyScale(ids.tangency))
        );
        assert_eq!(result.geometry, retained_geometry);
        assert_eq!(wrong.contact_state(ids.tangency).unwrap(), retained_contact);
        assert_eq!(result.display_audit, retained_audit);
        assert_eq!(
            result.geometry.circle(ids.circle).unwrap().radius.to_bits(),
            2.0e-14_f64.to_bits()
        );

        let (mut exact_but_unresolvable, ids, _derived_gap) =
            mixed_scale_fixture(side, derived_gap);
        let retained_geometry = exact_but_unresolvable.geometry();
        let retained_contact = exact_but_unresolvable.contact_state(ids.tangency).unwrap();
        let result = solve(&mut exact_but_unresolvable);
        assert_eq!(
            result.unstable_core_report().termination,
            SolveTermination::Converged
        );
        assert!(result.unstable_core_report().hard_residual_max < TOLERANCE);
        assert_eq!(
            result.rejection,
            Some(SolveRejection::AmbiguousTangencyScale(ids.tangency))
        );
        assert_eq!(result.geometry, retained_geometry);
        assert_eq!(
            exact_but_unresolvable.contact_state(ids.tangency).unwrap(),
            retained_contact
        );
    }
}

#[test]
fn span_escape_rolls_back_all_state_and_endpoint_roundoff_gets_full_reanalysis() {
    let mut escaped = Sketch::new(2.0).unwrap();
    let arc_center = escaped.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = escaped.add_point(Point2::new(-3.0, 0.0)).unwrap();
    let circle = escaped.add_circle(circle_center, 1.0).unwrap();
    let arc = escaped
        .add_arc(
            arc_center,
            2.0,
            -3.0 * PI / 4.0,
            3.0 * PI / 4.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    escaped.add_fixed_point(arc_center).unwrap();
    escaped.add_fixed_point(circle_center).unwrap();
    escaped
        .add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let tangency = escaped
        .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.9, 0.0)
        .unwrap();
    let retained_geometry = escaped.geometry();
    let retained_contact = escaped.contact_state(tangency).unwrap();
    let retained_audit = escaped
        .compile(SketchSolveRequest::default())
        .unwrap()
        .problem()
        .audit_snapshot()
        .unwrap();
    let rejected = solve(&mut escaped);
    assert_eq!(
        rejected.rejection,
        Some(SolveRejection::ContactParameterOutOfDomain(tangency))
    );
    assert_eq!(rejected.geometry, retained_geometry);
    assert_eq!(escaped.contact_state(tangency).unwrap(), retained_contact);
    assert_eq!(rejected.display_audit, retained_audit);
    assert_ne!(
        rejected.display_audit,
        rejected.unstable_core_report().audit
    );

    for scale in [1.0e-6, 1.0, 1.0e6] {
        let parameter_offset = 4.0 * f64::EPSILON;
        let endpoint = PI / 2.0;
        let target_angle = endpoint + PI * parameter_offset;
        let mut rounded = Sketch::new(scale).unwrap();
        let arc_center = rounded.add_point(Point2::new(0.0, 0.0)).unwrap();
        let circle_center = rounded
            .add_point(Point2::new(
                3.0 * scale * target_angle.cos(),
                3.0 * scale * target_angle.sin(),
            ))
            .unwrap();
        let circle = rounded.add_circle(circle_center, scale).unwrap();
        let arc = rounded
            .add_arc(
                arc_center,
                2.0 * scale,
                -PI / 2.0,
                endpoint,
                ArcSweep::CounterClockwise,
            )
            .unwrap();
        rounded.add_fixed_point(arc_center).unwrap();
        rounded.add_fixed_point(circle_center).unwrap();
        rounded
            .add_arc_radius(arc, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        let tangency = rounded
            .add_circle_arc_tangency(
                circle,
                arc,
                ArcCircleTangencySide::OutsideArc,
                0.9,
                target_angle + PI,
            )
            .unwrap();
        let config = SolverConfig {
            normalized_residual_tolerance: 64.0 * f64::EPSILON,
            normalized_step_tolerance: f64::EPSILON,
            ..SolverConfig::default()
        };
        let result = rounded
            .solve(SketchSolveRequest::default(), config)
            .unwrap();
        assert!(result.accepted(), "{:#?}", result.rejection);
        assert_eq!(contact(&rounded, tangency).0.to_bits(), 1.0_f64.to_bits());
        assert_eq!(result.display_audit, result.unstable_core_report().audit);
        assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 0);
        let mapping = result
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(tangency))
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
                .any(|binding| binding.name == "warm-start arc span" && binding.value == "1")
        }));

        let mut direct = rounded.clone();
        let direct_result = direct.solve(SketchSolveRequest::default(), config).unwrap();
        assert!(direct_result.accepted(), "{:#?}", direct_result.rejection);
        assert_eq!(
            result.unstable_core_report().rank,
            direct_result.unstable_core_report().rank
        );
        assert_eq!(
            result.unstable_core_report().local_degrees_of_freedom,
            direct_result
                .unstable_core_report()
                .local_degrees_of_freedom
        );
        assert_eq!(
            result.unstable_core_report().audit,
            direct_result.unstable_core_report().audit
        );
        assert_eq!(result.display_audit, direct_result.display_audit);
    }
}

fn broad_zero_to_three_halves_arc(sketch: &mut Sketch, center: PointId) -> ArcId {
    sketch
        .add_arc(center, 2.0, 0.0, 3.0 * PI / 2.0, ArcSweep::CounterClockwise)
        .unwrap()
}

#[test]
fn wrong_side_coincident_centers_zero_derived_radius_and_wrong_root_reject_typed() {
    let (mut wrong_side, ids) = fixture(ArcCircleTangencySide::OutsideArc, false);
    wrong_side
        .set_point_position(ids.circle_center, Point2::new(1.0, 0.0))
        .unwrap();
    wrong_side
        .set_contact_state(
            ids.tangency,
            ContactState::CircleArcTangency {
                arc_span_parameter: 0.5,
                circle_angle: 0.0,
            },
        )
        .unwrap();
    wrong_side.add_fixed_point(ids.circle_center).unwrap();
    let retained = wrong_side.geometry();
    let result = solve(&mut wrong_side);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::InvalidTangencyMode(ids.tangency))
    );
    assert_eq!(result.geometry, retained);

    let (mut coincident, ids) = fixture(ArcCircleTangencySide::OutsideArc, false);
    coincident
        .set_point_position(ids.circle_center, Point2::new(0.0, 0.0))
        .unwrap();
    coincident.set_circle_radius(ids.circle, 2.0).unwrap();
    coincident
        .set_contact_state(
            ids.tangency,
            ContactState::CircleArcTangency {
                arc_span_parameter: 0.5,
                circle_angle: 0.0,
            },
        )
        .unwrap();
    coincident.add_fixed_point(ids.circle_center).unwrap();
    let result = solve(&mut coincident);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::InvalidTangencyMode(ids.tangency))
    );

    let mut zero = Sketch::new(2.0).unwrap();
    let arc_center = zero.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = zero.add_point(Point2::new(3.0, 0.0)).unwrap();
    let circle = zero.add_circle(circle_center, 1.0).unwrap();
    let arc = broad_zero_to_three_halves_arc(&mut zero, arc_center);
    zero.add_fixed_point(arc_center).unwrap();
    zero.add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let tangency = zero
        .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.0, PI)
        .unwrap();
    zero.set_point_position(circle_center, Point2::new(2.0, 0.0))
        .unwrap();
    zero.set_circle_radius(circle, 4.0).unwrap();
    zero.set_contact_state(
        tangency,
        ContactState::CircleArcTangency {
            arc_span_parameter: 2.0 / 3.0,
            circle_angle: PI,
        },
    )
    .unwrap();
    zero.add_fixed_point(circle_center).unwrap();
    let result = solve(&mut zero);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::InvalidTangencyMode(tangency))
    );

    let mut wrong_root = Sketch::new(2.0).unwrap();
    let arc_center = wrong_root.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = wrong_root.add_point(Point2::new(3.0, 0.0)).unwrap();
    let circle = wrong_root.add_circle(circle_center, 5.0).unwrap();
    let arc = broad_zero_to_three_halves_arc(&mut wrong_root, arc_center);
    wrong_root.add_fixed_point(arc_center).unwrap();
    wrong_root.add_fixed_point(circle_center).unwrap();
    wrong_root
        .add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let tangency = wrong_root
        .add_circle_arc_tangency(
            circle,
            arc,
            ArcCircleTangencySide::OutsideArc,
            2.0 / 3.0,
            PI,
        )
        .unwrap();
    let result = solve(&mut wrong_root);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::InvalidTangencyMode(tangency))
    );
}

#[test]
fn constructor_contact_edits_stable_ids_and_removal_guards_are_transactional() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let arc_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = sketch.add_point(Point2::new(3.0, 0.0)).unwrap();
    let circle = sketch.add_circle(circle_center, 1.0).unwrap();
    let arc = sketch
        .add_arc(
            arc_center,
            2.0,
            -PI / 2.0,
            PI / 2.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    for bad_span in [f64::NAN, -0.1, 1.1] {
        assert!(matches!(
            sketch.add_circle_arc_tangency(
                circle,
                arc,
                ArcCircleTangencySide::OutsideArc,
                bad_span,
                PI,
            ),
            Err(SketchError::ParameterOutOfDomain { .. })
        ));
    }
    assert!(matches!(
        sketch.add_circle_arc_tangency(
            circle,
            arc,
            ArcCircleTangencySide::OutsideArc,
            0.5,
            f64::INFINITY,
        ),
        Err(SketchError::NonFiniteValue { .. })
    ));
    assert_eq!(
        sketch.add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::InsideArc, 0.5, 0.0,),
        Err(SketchError::ArcCircleTangencySideMismatch(
            ArcCircleTangencySide::InsideArc
        ))
    );

    let tangency = sketch
        .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.5, PI)
        .unwrap();
    assert_eq!(
        sketch.remove_circle(circle),
        Err(SketchError::CircleInUse(circle))
    );
    assert_eq!(sketch.remove_arc(arc), Err(SketchError::ArcInUse(arc)));
    let retained = sketch.contact_state(tangency).unwrap();
    assert!(matches!(
        sketch.set_contact_state(
            tangency,
            ContactState::CircleArcTangency {
                arc_span_parameter: 0.4,
                circle_angle: f64::NAN,
            },
        ),
        Err(SketchError::NonFiniteValue { .. })
    ));
    assert_eq!(sketch.contact_state(tangency).unwrap(), retained);
    sketch.remove_constraint(tangency).unwrap();
    sketch.remove_circle(circle).unwrap();
    sketch.remove_arc(arc).unwrap();
    sketch.remove_point(circle_center).unwrap();
    sketch.remove_point(arc_center).unwrap();

    let mut coincident = Sketch::new(1.0).unwrap();
    let first_center = coincident.add_point(Point2::new(0.0, 0.0)).unwrap();
    let same_position = coincident.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle = coincident.add_circle(first_center, 1.0).unwrap();
    let arc = coincident
        .add_arc(same_position, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    assert_eq!(
        coincident
            .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::InsideArc, 0.5, 0.0,),
        Err(SketchError::AmbiguousArcCircleTangencyCenters)
    );
    coincident
        .set_point_position(first_center, Point2::new(2.0, 0.0))
        .unwrap();
    assert_eq!(
        coincident
            .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.5, PI,),
        Err(SketchError::ZeroDerivedCircleRadius)
    );

    let mut stale = Sketch::new(1.0).unwrap();
    let first = stale.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = stale.add_point(Point2::new(3.0, 0.0)).unwrap();
    let stale_circle = stale.add_circle(second, 1.0).unwrap();
    stale.remove_circle(stale_circle).unwrap();
    let valid_arc = stale
        .add_arc(first, 2.0, -PI / 2.0, PI / 2.0, ArcSweep::CounterClockwise)
        .unwrap();
    assert!(matches!(
        stale.add_circle_arc_tangency(
            stale_circle,
            valid_arc,
            ArcCircleTangencySide::OutsideArc,
            0.5,
            PI,
        ),
        Err(SketchError::UnknownCircle(id)) if id == stale_circle
    ));
    let valid_circle = stale.add_circle(second, 1.0).unwrap();
    let stale_arc = stale
        .add_arc(first, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    stale.remove_arc(stale_arc).unwrap();
    assert!(matches!(
        stale.add_circle_arc_tangency(
            valid_circle,
            stale_arc,
            ArcCircleTangencySide::OutsideArc,
            0.5,
            PI,
        ),
        Err(SketchError::UnknownArc(id)) if id == stale_arc
    ));
}

#[test]
fn tiny_radius_tangent_row_is_dimensionless_and_zero_arc_derivative_is_invalid() {
    let scale = 1.0e-12;
    let angular_error = 1.0e-4;
    let mut short = Sketch::new(1.0).unwrap();
    let arc_center = short.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = short.add_point(Point2::new(3.0 * scale, 0.0)).unwrap();
    let circle = short.add_circle(circle_center, scale).unwrap();
    let arc = short
        .add_arc(
            arc_center,
            2.0 * scale,
            -PI / 2.0,
            PI / 2.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let tangency = short
        .add_circle_arc_tangency(
            circle,
            arc,
            ArcCircleTangencySide::OutsideArc,
            0.5,
            PI + angular_error,
        )
        .unwrap();
    let compiled = short
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(tangency))
        .unwrap();
    let source = compiled
        .problem()
        .audit_snapshot()
        .unwrap()
        .sources
        .into_iter()
        .find(|source| Some(source.source_id) == mapping.core_source_id)
        .unwrap();
    assert_eq!(source.rows[2].scale.to_bits(), 1.0_f64.to_bits());
    assert!(source.rows[2].normalized_residual.abs() > 0.9 * angular_error);

    let mut degenerate = Sketch::new(1.0).unwrap();
    let arc_center = degenerate.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = degenerate.add_point(Point2::new(1.0, 0.0)).unwrap();
    let circle = degenerate.add_circle(circle_center, 1.0).unwrap();
    let arc = degenerate
        .add_arc(
            arc_center,
            f64::MIN_POSITIVE,
            0.0,
            f64::MIN_POSITIVE,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    degenerate
        .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.5, PI)
        .unwrap();
    let result = degenerate
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(!result.accepted());
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Invalid
    );
    assert!(
        result
            .unstable_core_report()
            .audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .any(|row| {
                row.evaluation_status == AuditEvaluationStatus::Failed
                    && row.evaluation_error_category == Some(EvaluationErrorCategory::Degenerate)
            })
    );
    assert!(matches!(
        degenerate.set_arc_radius(arc, 0.0),
        Err(SketchError::InvalidRadius(0.0))
    ));
}
