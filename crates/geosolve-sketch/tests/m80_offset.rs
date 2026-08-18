// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{HardValidity, OperationControl, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ArcAngleEndpoint, ArcSweep, ContactDomain, ContactNeighborhood, CurveContactNeighborhood,
    CurveContinuity, CurveDefinition, CurveSpan, DimensionKind, DocumentCommand,
    DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveContinuity,
    DocumentDirectedProfileOffsetCurve, DocumentEdit, DocumentFaceOffsetDirection,
    DocumentObjectId, DocumentOffsetTraversal, DocumentProfileOffsetChain,
    DocumentProfileOffsetCreationJunction, DocumentProfileOffsetCreationOperand,
    DocumentProfileOffsetCreationPath, DocumentProfileOffsetCreationRequest,
    DocumentProfileOffsetEdgePair, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetOperand,
    DocumentProfileOffsetTerminalPolicy, DocumentProfileOffsetTurn, FaceOffsetDirection,
    GeometryRole, LineParameterDomain, LineSide, OffsetTraversal, PreparedSketchOperation,
    ProfileOffsetAssociation, ProfileOffsetChain, ProfileOffsetCurve, ProfileOffsetEdgePair,
    ProfileOffsetLoop, ProfileOffsetOperand, ProfileOffsetTerminalPolicy,
    RetainedSketchDocumentSession, Sketch, SketchCurve, SketchCurveContact, SketchDocument,
    SketchDocumentSession, SketchError, SketchSolveRequest,
};

const TOLERANCE: f64 = 1.0e-8;

fn directed(curve: ProfileOffsetCurve) -> geosolve_sketch::DirectedProfileOffsetCurve {
    directed_with(curve, OffsetTraversal::Forward)
}

fn directed_with(
    curve: ProfileOffsetCurve,
    traversal: OffsetTraversal,
) -> geosolve_sketch::DirectedProfileOffsetCurve {
    geosolve_sketch::DirectedProfileOffsetCurve { curve, traversal }
}

fn pair(source: ProfileOffsetCurve, target: ProfileOffsetCurve) -> ProfileOffsetEdgePair {
    ProfileOffsetEdgePair {
        source: directed(source),
        target: directed(target),
    }
}

fn assert_accepted(result: &geosolve_sketch::SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert!(result.unstable_core_report().hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
    assert!(
        result
            .geometry
            .points
            .iter()
            .all(|point| point.position.x.is_finite() && point.position.y.is_finite())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scale-loop fixture keeps exact open-chain rows, path audits, rank, and geometry directly comparable"
)]
fn exact_open_line_offset_has_grouped_audit_fd_jacobian_and_scale_invariance() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let source_points = [
            sketch.add_point(Point2::new(0.0, 0.0)).unwrap(),
            sketch.add_point(Point2::new(4.0 * scale, 0.0)).unwrap(),
        ];
        let target_points = [
            sketch
                .add_point(Point2::new(0.2 * scale, 1.7 * scale))
                .unwrap(),
            sketch
                .add_point(Point2::new(4.2 * scale, 1.7 * scale))
                .unwrap(),
        ];
        let source = sketch
            .add_segment(source_points[0], source_points[1])
            .unwrap();
        let target = sketch
            .add_segment(target_points[0], target_points[1])
            .unwrap();
        for point in source_points {
            sketch.add_fixed_point(point).unwrap();
        }
        let (_, dimension) = sketch
            .add_profile_offset(
                ProfileOffsetAssociation {
                    operand: ProfileOffsetOperand::OpenChain {
                        side: LineSide::Left,
                        chain: ProfileOffsetChain {
                            edges: vec![pair(
                                ProfileOffsetCurve::Line(source),
                                ProfileOffsetCurve::Line(target),
                            )],
                            junctions: vec![],
                            start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                            end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        },
                    },
                },
                2.0 * scale,
            )
            .unwrap();
        assert!(matches!(
            sketch.dimension(dimension).unwrap().kind(),
            DimensionKind::ProfileOffset { target, .. }
                if (target - 2.0 * scale).abs() <= f64::EPSILON * scale.max(1.0)
        ));
        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(jacobians.all_within(3.0e-6), "{jacobians:#?}");
        let rows = compiled
            .problem()
            .audit_rows()
            .unwrap()
            .into_iter()
            .filter(|row| {
                row.template.contains("directed_source") || row.template.contains("source_join")
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| {
            !row.template.is_empty()
                && !row.bindings.is_empty()
                && row
                    .bindings
                    .iter()
                    .any(|binding| binding.name == "selected path" && binding.value == "open chain")
                && row.scale.is_finite()
                && row.scale > 0.0
        }));
        let result = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&result);
        assert_eq!(result.unstable_core_report().rank, 4);
        assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 0);
        let start = result.geometry.point(target_points[0]).unwrap();
        let end = result.geometry.point(target_points[1]).unwrap();
        assert!((start.x / scale).abs() <= TOLERANCE);
        assert!((start.y / scale - 2.0).abs() <= TOLERANCE);
        assert!((end.x / scale - 4.0).abs() <= TOLERANCE);
        assert!((end.y / scale - 2.0).abs() <= TOLERANCE);
        let mapping = result
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Dimension(dimension))
            .unwrap();
        assert_eq!(mapping.residual_ids.len(), 3);

        sketch
            .set_dimension_target(dimension, 1.25 * scale)
            .unwrap();
        let edited = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&edited);
        assert!(
            (edited.geometry.point(target_points[0]).unwrap().y / scale - 1.25).abs() <= TOLERANCE
        );
        assert!(
            (edited.geometry.point(target_points[1]).unwrap().y / scale - 1.25).abs() <= TOLERANCE
        );
    }
}

#[test]
fn exact_open_line_offset_moves_the_source_when_the_target_side_is_driven() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let source_points = [
        sketch.add_point(Point2::new(0.9, 0.9)).unwrap(),
        sketch.add_point(Point2::new(4.9, 0.9)).unwrap(),
    ];
    let target_points = [
        sketch.add_point(Point2::new(1.0, 3.0)).unwrap(),
        sketch.add_point(Point2::new(5.0, 3.0)).unwrap(),
    ];
    let source = sketch
        .add_segment(source_points[0], source_points[1])
        .unwrap();
    let target = sketch
        .add_segment(target_points[0], target_points[1])
        .unwrap();
    for point in target_points {
        sketch.add_fixed_point(point).unwrap();
    }
    sketch
        .add_profile_offset(
            ProfileOffsetAssociation {
                operand: ProfileOffsetOperand::OpenChain {
                    side: LineSide::Left,
                    chain: ProfileOffsetChain {
                        edges: vec![pair(
                            ProfileOffsetCurve::Line(source),
                            ProfileOffsetCurve::Line(target),
                        )],
                        junctions: vec![],
                        start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                    },
                },
            },
            2.0,
        )
        .unwrap();

    let solved = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&solved);
    for (point, expected) in source_points.into_iter().zip([[1.0, 1.0], [5.0, 1.0]]) {
        let actual = solved.geometry.point(point).unwrap();
        assert!((actual.x - expected[0]).abs() <= TOLERANCE);
        assert!((actual.y - expected[1]).abs() <= TOLERANCE);
    }
}

#[test]
fn radial_offsets_solve_arc_endpoints_and_full_circle_deterministically() {
    let mut circle_sketch = Sketch::new(4.0).unwrap();
    let source_center = circle_sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let target_center = circle_sketch.add_point(Point2::new(0.1, -0.1)).unwrap();
    let source = circle_sketch.add_circle(source_center, 4.0).unwrap();
    let target = circle_sketch.add_circle(target_center, 4.8).unwrap();
    circle_sketch.add_fixed_point(source_center).unwrap();
    circle_sketch
        .add_circle_radius(source, 4.0, geosolve_sketch::DimensionMode::Driving)
        .unwrap();
    circle_sketch
        .add_profile_offset(
            ProfileOffsetAssociation {
                operand: ProfileOffsetOperand::Face {
                    direction: FaceOffsetDirection::Outward,
                    outer: ProfileOffsetLoop {
                        edges: vec![pair(
                            ProfileOffsetCurve::Circle(source),
                            ProfileOffsetCurve::Circle(target),
                        )],
                        junctions: vec![],
                    },
                    holes: vec![],
                },
            },
            1.0,
        )
        .unwrap();
    let result = circle_sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&result);
    let target_circle = result.geometry.circle(target).unwrap();
    assert!(target_circle.center.coords.norm() <= TOLERANCE);
    assert!((target_circle.radius - 5.0).abs() <= TOLERANCE);

    let mut arc_sketch = Sketch::new(4.0).unwrap();
    let source_center = arc_sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let target_center = arc_sketch.add_point(Point2::new(0.2, 0.1)).unwrap();
    let source = arc_sketch
        .add_arc(
            source_center,
            4.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let target = arc_sketch
        .add_arc(
            target_center,
            3.2,
            0.05,
            std::f64::consts::FRAC_PI_2 - 0.04,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    arc_sketch.add_fixed_point(source_center).unwrap();
    arc_sketch
        .add_arc_radius(source, 4.0, geosolve_sketch::DimensionMode::Driving)
        .unwrap();
    arc_sketch
        .add_profile_offset(
            ProfileOffsetAssociation {
                operand: ProfileOffsetOperand::OpenChain {
                    side: LineSide::Left,
                    chain: ProfileOffsetChain {
                        edges: vec![pair(
                            ProfileOffsetCurve::CircularArc(source),
                            ProfileOffsetCurve::CircularArc(target),
                        )],
                        junctions: vec![],
                        start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                    },
                },
            },
            1.0,
        )
        .unwrap();
    let compiled = arc_sketch.compile(SketchSolveRequest::default()).unwrap();
    assert!(
        compiled
            .problem()
            .check_jacobians(1.0e-6)
            .unwrap()
            .all_within(3.0e-6)
    );
    let result = arc_sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&result);
    let target_arc = result.geometry.arc(target).unwrap();
    assert!((target_arc.radius - 3.0).abs() <= TOLERANCE);
    assert!(target_arc.center.coords.norm() <= TOLERANCE);
    assert!(target_arc.start_angle.abs() <= TOLERANCE);
    assert!((target_arc.end_angle - std::f64::consts::FRAC_PI_2).abs() <= TOLERANCE);
}

#[test]
fn target_arc_endpoint_constraints_drive_the_free_source_arc_through_profile_offset() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let source_center = sketch.add_point(Point2::new(0.2, -0.1)).unwrap();
    let target_center = sketch.add_point(Point2::origin()).unwrap();
    let source = sketch
        .add_arc(
            source_center,
            4.2,
            0.0,
            std::f64::consts::FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let target = sketch
        .add_arc(
            target_center,
            3.0,
            std::f64::consts::FRAC_PI_4,
            3.0 * std::f64::consts::FRAC_PI_4,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(target_center).unwrap();
    sketch
        .add_arc_radius(target, 3.0, geosolve_sketch::DimensionMode::Driving)
        .unwrap();
    sketch
        .add_profile_offset(
            ProfileOffsetAssociation {
                operand: ProfileOffsetOperand::OpenChain {
                    side: LineSide::Left,
                    chain: ProfileOffsetChain {
                        edges: vec![pair(
                            ProfileOffsetCurve::CircularArc(source),
                            ProfileOffsetCurve::CircularArc(target),
                        )],
                        junctions: vec![],
                        start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                    },
                },
            },
            1.0,
        )
        .unwrap();
    // Add the downstream drivers after the Profile Offset to prove activation
    // is semantic rather than an artifact of persistent-source insertion order.
    sketch
        .add_fixed_arc_angle(target, ArcAngleEndpoint::Start, std::f64::consts::FRAC_PI_4)
        .unwrap();
    sketch
        .add_fixed_arc_angle(
            target,
            ArcAngleEndpoint::End,
            3.0 * std::f64::consts::FRAC_PI_4,
        )
        .unwrap();

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&result);
    let source_arc = result.geometry.arc(source).unwrap();
    assert!(source_arc.center.coords.norm() <= TOLERANCE);
    assert!((source_arc.radius - 4.0).abs() <= TOLERANCE);
    assert!((source_arc.start_angle - std::f64::consts::FRAC_PI_4).abs() <= TOLERANCE);
    assert!((source_arc.end_angle - 3.0 * std::f64::consts::FRAC_PI_4).abs() <= TOLERANCE);
}

#[test]
fn one_target_arc_endpoint_driver_propagates_without_a_shared_angle_gauge() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let source_center = sketch.add_point(Point2::new(0.2, -0.1)).unwrap();
    let target_center = sketch.add_point(Point2::origin()).unwrap();
    let source = sketch
        .add_arc(
            source_center,
            4.2,
            0.0,
            std::f64::consts::FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let target = sketch
        .add_arc(
            target_center,
            3.0,
            0.1,
            std::f64::consts::FRAC_PI_2 - 0.1,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(target_center).unwrap();
    sketch
        .add_arc_radius(target, 3.0, geosolve_sketch::DimensionMode::Driving)
        .unwrap();
    sketch
        .add_profile_offset(
            ProfileOffsetAssociation {
                operand: ProfileOffsetOperand::OpenChain {
                    side: LineSide::Left,
                    chain: ProfileOffsetChain {
                        edges: vec![pair(
                            ProfileOffsetCurve::CircularArc(source),
                            ProfileOffsetCurve::CircularArc(target),
                        )],
                        junctions: vec![],
                        start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                    },
                },
            },
            1.0,
        )
        .unwrap();
    sketch
        .add_fixed_arc_angle(target, ArcAngleEndpoint::Start, std::f64::consts::FRAC_PI_4)
        .unwrap();

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert_accepted(&result);
    let source_arc = result.geometry.arc(source).unwrap();
    let target_arc = result.geometry.arc(target).unwrap();
    assert!((source_arc.start_angle - std::f64::consts::FRAC_PI_4).abs() <= TOLERANCE);
    assert!((target_arc.start_angle - std::f64::consts::FRAC_PI_4).abs() <= TOLERANCE);
    assert!((source_arc.end_angle - std::f64::consts::FRAC_PI_2).abs() <= TOLERANCE);
    assert!((target_arc.end_angle - std::f64::consts::FRAC_PI_2).abs() <= TOLERANCE);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scale-loop fixture keeps every mixed tangent row, audit, and independent solve invariant directly comparable"
)]
fn mixed_tangent_offset_rows_have_fd_and_structured_audit_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let source_line_points = [
            sketch.add_point(Point2::new(-2.0 * scale, 0.0)).unwrap(),
            sketch.add_point(Point2::new(0.0, 0.0)).unwrap(),
        ];
        let target_line_points = [
            sketch
                .add_point(Point2::new(-1.9 * scale, 0.6 * scale))
                .unwrap(),
            sketch
                .add_point(Point2::new(0.1 * scale, 0.6 * scale))
                .unwrap(),
        ];
        let source_line = sketch
            .add_segment(source_line_points[0], source_line_points[1])
            .unwrap();
        let target_line = sketch
            .add_segment(target_line_points[0], target_line_points[1])
            .unwrap();
        let source_center = sketch.add_point(Point2::new(0.0, 1.0 * scale)).unwrap();
        let target_center = sketch
            .add_point(Point2::new(0.1 * scale, 1.1 * scale))
            .unwrap();
        let source_arc = sketch
            .add_arc(
                source_center,
                scale,
                -std::f64::consts::FRAC_PI_2,
                0.0,
                ArcSweep::CounterClockwise,
            )
            .unwrap();
        let target_arc = sketch
            .add_arc(
                target_center,
                0.6 * scale,
                -std::f64::consts::FRAC_PI_2 + 0.05,
                -0.04,
                ArcSweep::CounterClockwise,
            )
            .unwrap();
        sketch
            .add_endpoint_continuity(
                SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment: target_line,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 1.0,
                    neighborhood: CurveContactNeighborhood::End,
                },
                SketchCurveContact {
                    curve: SketchCurve::Arc(target_arc),
                    parameter: 0.0,
                    neighborhood: CurveContactNeighborhood::Start,
                },
                CurveContinuity::G1,
            )
            .unwrap();
        for point in source_line_points {
            sketch.add_fixed_point(point).unwrap();
        }
        sketch.add_fixed_point(source_center).unwrap();
        sketch
            .add_arc_radius(source_arc, scale, geosolve_sketch::DimensionMode::Driving)
            .unwrap();
        sketch
            .add_fixed_arc_angle(
                source_arc,
                ArcAngleEndpoint::Start,
                -std::f64::consts::FRAC_PI_2,
            )
            .unwrap();
        sketch
            .add_fixed_arc_angle(source_arc, ArcAngleEndpoint::End, 0.0)
            .unwrap();
        let (_, dimension) = sketch
            .add_profile_offset(
                ProfileOffsetAssociation {
                    operand: ProfileOffsetOperand::OpenChain {
                        side: LineSide::Left,
                        chain: ProfileOffsetChain {
                            edges: vec![
                                pair(
                                    ProfileOffsetCurve::Line(source_line),
                                    ProfileOffsetCurve::Line(target_line),
                                ),
                                pair(
                                    ProfileOffsetCurve::CircularArc(source_arc),
                                    ProfileOffsetCurve::CircularArc(target_arc),
                                ),
                            ],
                            junctions: vec![geosolve_sketch::ProfileOffsetJunctionBranch::Tangent],
                            start_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                            end_terminal: ProfileOffsetTerminalPolicy::NormalTranslation,
                        },
                    },
                },
                0.5 * scale,
            )
            .unwrap();

        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(
            jacobians.blocks.iter().all(|block| {
                block.max_relative_error <= 4.0e-6 || block.max_absolute_error <= 1.0e-8
            }),
            "{jacobians:#?}"
        );
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Dimension(dimension))
            .unwrap();
        assert_eq!(mapping.residual_ids.len(), 7);
        let audit_rows = compiled.problem().audit_rows().unwrap();
        let ordinary_g1_rows = audit_rows
            .iter()
            .filter(|row| {
                row.bindings
                    .iter()
                    .any(|binding| binding.name == "continuity" && binding.value == "G1")
            })
            .collect::<Vec<_>>();
        assert_eq!(ordinary_g1_rows.len(), 3);
        assert!(
            ordinary_g1_rows
                .iter()
                .any(|row| row.template.contains("incoming_endpoint.x"))
        );
        assert!(
            ordinary_g1_rows
                .iter()
                .any(|row| row.template.contains("incoming_endpoint.y"))
        );
        let rows = audit_rows
            .into_iter()
            .filter(|row| mapping.residual_ids.contains(&row.residual_id))
            .collect::<Vec<_>>();
        assert!(
            rows.iter()
                .all(|row| !row.template.contains("target_in.end"))
        );
        assert!(rows.iter().any(|row| row.template.contains("target_join")));
        let tangent_anchor = rows
            .iter()
            .find(|row| row.template.contains("target_join"))
            .unwrap();
        assert!(
            tangent_anchor
                .bindings
                .iter()
                .any(|binding| binding.name == "junction" && binding.value == "0")
        );
        assert!(rows.iter().all(|row| {
            !row.template.is_empty()
                && row.scale.is_finite()
                && row.scale > 0.0
                && row
                    .bindings
                    .iter()
                    .any(|binding| binding.name == "selected path" && binding.value == "open chain")
        }));

        let solved = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&solved);
        assert_eq!(solved.unstable_core_report().rank, 12);
        assert_eq!(solved.unstable_core_report().local_degrees_of_freedom, 0);
        assert!(solved.unstable_core_report().conflicting_sources.is_empty());
        assert!(solved.unstable_core_report().redundant_sources.is_empty());
        let target = solved.geometry.arc(target_arc).unwrap();
        assert!((target.radius / scale - 0.5).abs() <= TOLERANCE);
        assert!((target.start_angle + std::f64::consts::FRAC_PI_2).abs() <= TOLERANCE);
        assert!(target.end_angle.abs() <= TOLERANCE);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one annular fixture compares material direction, outer/hole audit identity, and atomic collapse at both branches"
)]
fn annular_material_direction_and_radial_collapse_are_explicit_and_transactional() {
    for (direction, outer_target_radius, hole_target_radius) in [
        (FaceOffsetDirection::Outward, 7.0, 1.0),
        (FaceOffsetDirection::Inward, 5.0, 3.0),
    ] {
        let mut sketch = Sketch::new(4.0).unwrap();
        let center = sketch.add_point(Point2::origin()).unwrap();
        let outer_source = sketch.add_circle(center, 6.0).unwrap();
        let outer_target = sketch
            .add_circle(center, outer_target_radius + 0.2)
            .unwrap();
        let hole_source = sketch.add_circle(center, 2.0).unwrap();
        let hole_target = sketch.add_circle(center, hole_target_radius + 0.2).unwrap();
        sketch.add_fixed_point(center).unwrap();
        sketch
            .add_circle_radius(outer_source, 6.0, geosolve_sketch::DimensionMode::Driving)
            .unwrap();
        sketch
            .add_circle_radius(hole_source, 2.0, geosolve_sketch::DimensionMode::Driving)
            .unwrap();
        let (_, offset) = sketch
            .add_profile_offset(
                ProfileOffsetAssociation {
                    operand: ProfileOffsetOperand::Face {
                        direction,
                        outer: ProfileOffsetLoop {
                            edges: vec![ProfileOffsetEdgePair {
                                source: directed_with(
                                    ProfileOffsetCurve::Circle(outer_source),
                                    OffsetTraversal::Forward,
                                ),
                                target: directed_with(
                                    ProfileOffsetCurve::Circle(outer_target),
                                    OffsetTraversal::Forward,
                                ),
                            }],
                            junctions: vec![],
                        },
                        holes: vec![ProfileOffsetLoop {
                            edges: vec![ProfileOffsetEdgePair {
                                source: directed_with(
                                    ProfileOffsetCurve::Circle(hole_source),
                                    OffsetTraversal::Reverse,
                                ),
                                target: directed_with(
                                    ProfileOffsetCurve::Circle(hole_target),
                                    OffsetTraversal::Reverse,
                                ),
                            }],
                            junctions: vec![],
                        }],
                    },
                },
                1.0,
            )
            .unwrap();
        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Dimension(offset))
            .unwrap();
        let selected_paths_and_edges = compiled
            .problem()
            .audit_rows()
            .unwrap()
            .into_iter()
            .filter(|row| mapping.residual_ids.contains(&row.residual_id))
            .map(|row| {
                let selected_path = row
                    .bindings
                    .iter()
                    .find(|binding| binding.name == "selected path")
                    .expect("every grouped Profile Offset row must identify its selected path")
                    .value
                    .clone();
                let edge = row
                    .bindings
                    .into_iter()
                    .find(|binding| binding.name == "edge")
                    .expect("every radial Profile Offset row must identify its edge")
                    .value;
                (selected_path, edge)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected_paths_and_edges,
            [
                ("outer".into(), "0".into()),
                ("outer".into(), "0".into()),
                ("outer".into(), "0".into()),
                ("hole 0".into(), "0".into()),
                ("hole 0".into(), "0".into()),
                ("hole 0".into(), "0".into()),
            ]
        );
        let accepted = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert_accepted(&accepted);
        assert!(
            (accepted.geometry.circle(outer_target).unwrap().radius - outer_target_radius).abs()
                <= TOLERANCE
        );
        assert!(
            (accepted.geometry.circle(hole_target).unwrap().radius - hole_target_radius).abs()
                <= TOLERANCE
        );

        if direction == FaceOffsetDirection::Inward {
            let retained = accepted.geometry.clone();
            sketch.set_dimension_target(offset, 5.0).unwrap();
            let rejected = sketch
                .solve(SketchSolveRequest::default(), SolverConfig::default())
                .unwrap();
            assert!(!rejected.accepted(), "{rejected:#?}");
            assert_eq!(rejected.geometry, retained);
            assert_eq!(sketch.geometry(), retained);
        }
    }
}

fn document_line_operand(
    document: &mut SketchDocument,
) -> (DocumentProfileOffsetOperand, CurveSpan, CurveSpan) {
    let source_start = document.add_point("source start", [0.0, 0.0]).unwrap();
    let source_end = document.add_point("source end", [4.0, 0.0]).unwrap();
    let target_start = document.add_point("target start", [0.0, 2.0]).unwrap();
    let target_end = document.add_point("target end", [4.0, 2.0]).unwrap();
    let source = document
        .add_curve(
            "source",
            CurveDefinition::Line {
                start: source_start,
                end: source_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let target = document
        .add_curve(
            "target",
            CurveDefinition::Line {
                start: target_start,
                end: target_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let source = CurveSpan::line(source);
    let target = CurveSpan::line(target);
    (
        DocumentProfileOffsetOperand::OpenChain {
            side: geosolve_sketch::DocumentLineSide::Left,
            chain: DocumentProfileOffsetChain {
                edges: vec![DocumentProfileOffsetEdgePair {
                    source: DocumentDirectedProfileOffsetCurve {
                        curve: source,
                        traversal: DocumentOffsetTraversal::Forward,
                    },
                    target: DocumentDirectedProfileOffsetCurve {
                        curve: target,
                        traversal: DocumentOffsetTraversal::Forward,
                    },
                }],
                junctions: vec![],
                start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            },
        },
        source,
        target,
    )
}

fn assert_m80_f005_construction_operand_rejected(construction_is_source: bool) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let (operand, source, target) = document_line_operand(&mut document);
    let curve = if construction_is_source {
        source.curve
    } else {
        target.curve
    };
    document
        .set_geometry_role(curve, GeometryRole::Construction)
        .unwrap();
    let before = document.clone();
    let before_bytes = document.to_draft_v5_json().unwrap();

    let result = document.add_profile_offset("invalid construction operand", 2.0, operand);

    assert!(
        matches!(
            &result,
            Err(geosolve_sketch::DocumentError::InvalidField { field, .. })
                if *field == "profile offset geometry role"
        ),
        "unexpected result: {result:?}"
    );
    assert_eq!(document, before);
    assert_eq!(document.to_draft_v5_json().unwrap(), before_bytes);
}

#[test]
fn m80_f005_construction_source_is_rejected_without_mutation() {
    assert_m80_f005_construction_operand_rejected(true);
}

#[test]
fn m80_f005_construction_target_is_rejected_without_mutation() {
    assert_m80_f005_construction_operand_rejected(false);
}

fn assert_m80_f005_associated_role_change_rejected(change_source: bool) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let (operand, source, target) = document_line_operand(&mut document);
    document.add_profile_offset("offset", 2.0, operand).unwrap();
    let before = document.clone();
    let before_bytes = document.to_draft_v5_json().unwrap();
    let curve = if change_source {
        source.curve
    } else {
        target.curve
    };

    let result = document.set_geometry_role(curve, GeometryRole::Construction);

    assert!(
        matches!(
            &result,
            Err(geosolve_sketch::DocumentError::InvalidField { field, .. })
                if *field == "profile offset geometry role"
        ),
        "unexpected result: {result:?}"
    );
    assert_eq!(document, before);
    assert_eq!(document.to_draft_v5_json().unwrap(), before_bytes);
}

#[test]
fn m80_f005_associated_source_role_change_is_atomically_rejected() {
    assert_m80_f005_associated_role_change_rejected(true);
}

#[test]
fn m80_f005_associated_target_role_change_is_atomically_rejected() {
    assert_m80_f005_associated_role_change_rejected(false);
}

#[test]
fn m80_f005_draft_v5_rejects_construction_profile_offset_operands() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let (operand, source, target) = document_line_operand(&mut document);
    document.add_profile_offset("offset", 2.0, operand).unwrap();
    let draft = document.to_draft_v5_json().unwrap();
    for curve in [source.curve, target.curve] {
        let mut invalid: serde_json::Value = serde_json::from_str(&draft).unwrap();
        invalid["geometry_roles"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "curve": curve,
                "role": "construction"
            }));
        let invalid = serde_json::to_string(&invalid).unwrap();

        let result = SketchDocument::from_draft_v5_json(&invalid);

        assert!(
            matches!(
                &result,
                Err(geosolve_sketch::DocumentError::InvalidField { field, .. })
                    if *field == "profile offset geometry role"
            ),
            "unexpected result: {result:?}"
        );
    }
}

#[test]
fn draft_v5_session_and_prepared_lifecycle_preserve_atomic_profile_offset_identity() {
    assert_profile_offset_draft_v5_round_trip();
    assert_profile_offset_prepared_publication();
    assert_profile_offset_history_round_trip();
}

fn assert_profile_offset_draft_v5_round_trip() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let (operand, _, _) = document_line_operand(&mut document);
    let before = document.clone();
    assert!(matches!(
        document.add_profile_offset("bad", 0.0, operand.clone()),
        Err(geosolve_sketch::DocumentError::InvalidField { .. })
    ));
    assert_eq!(document, before);
    let ids = document
        .add_profile_offset("offset", 2.0, operand.clone())
        .unwrap();
    assert!(matches!(
        document.to_canonical_json(),
        Err(geosolve_sketch::DocumentError::UnsupportedM80State)
    ));
    let draft = document.to_draft_v5_json().unwrap();
    assert!(draft.contains("profile_offset_dimensions"));
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);
    assert_eq!(
        restored.dimension(ids.dimension).unwrap().source_id,
        document.dimension(ids.dimension).unwrap().source_id
    );
    let lowered = restored.lower().unwrap();
    assert!(matches!(
        lowered
            .sketch()
            .dimension(
                lowered
                    .mappings()
                    .runtime_source(document.dimension(ids.dimension).unwrap().source_id)
                    .and_then(|source| match source {
                        geosolve_sketch::RuntimeSource::Dimension(id) => Some(id),
                        geosolve_sketch::RuntimeSource::Constraint(_) => None,
                    })
                    .unwrap()
            )
            .unwrap()
            .kind(),
        DimensionKind::ProfileOffset { .. }
    ));
}

#[test]
fn draft_v5_round_trip_preserves_complete_mixed_offset_operand_byte_stably() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let (source, source_owners) = add_line_arc_endpoint_owners(&mut document, "source", 0.0);
    let reverse =
        |directed: DocumentDirectedProfileOffsetCurve| DocumentDirectedProfileOffsetCurve {
            curve: directed.curve,
            traversal: DocumentOffsetTraversal::Reverse,
        };
    let ordered_source = [reverse(source[1]), reverse(source[0])];
    let source_owner = source_owners[1];
    let ids = document
        .create_profile_offset_geometry(DocumentProfileOffsetCreationRequest {
            label: "reverse mixed tangent offset".into(),
            distance: 0.25,
            operand: DocumentProfileOffsetCreationOperand::OpenChain {
                side: geosolve_sketch::DocumentLineSide::Right,
                chain: DocumentProfileOffsetCreationPath {
                    edges: ordered_source.to_vec(),
                    junctions: vec![DocumentProfileOffsetCreationJunction {
                        source_owner: DocumentProfileOffsetJunctionOwner::Constraint(source_owner),
                        branch: DocumentProfileOffsetJunctionBranch::Tangent,
                    }],
                },
            },
        })
        .unwrap();

    let definition = &document.dimension(ids.dimension).unwrap().definition;
    let geosolve_sketch::DocumentDimensionDefinition::ProfileOffset {
        operand: operand @ DocumentProfileOffsetOperand::OpenChain { side, chain },
        ..
    } = definition
    else {
        panic!("mixed construction must retain one open-chain Profile Offset");
    };
    assert_eq!(*side, geosolve_sketch::DocumentLineSide::Right);
    assert_eq!(chain.edges.len(), 2);
    assert_eq!(chain.edges[0].source, ordered_source[0]);
    assert_eq!(chain.edges[1].source, ordered_source[1]);
    assert!(chain.edges.iter().all(|edge| {
        edge.source.traversal == DocumentOffsetTraversal::Reverse
            && edge.target.traversal == DocumentOffsetTraversal::Reverse
    }));
    assert_eq!(
        chain.start_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    assert_eq!(
        chain.end_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    let [junction] = chain.junctions.as_slice() else {
        panic!("mixed chain must retain one junction");
    };
    assert_eq!(
        junction.source_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(source_owner)
    );
    assert!(matches!(
        junction.target_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(_)
    ));
    assert_ne!(junction.target_owner, junction.source_owner);
    assert_eq!(
        junction.branch,
        DocumentProfileOffsetJunctionBranch::Tangent
    );
    let expected_operand = operand.clone();

    let draft = document.to_draft_v5_json().unwrap();
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);
    let restored_definition = &restored.dimension(ids.dimension).unwrap().definition;
    let geosolve_sketch::DocumentDimensionDefinition::ProfileOffset {
        operand: restored_operand,
        ..
    } = restored_definition
    else {
        panic!("restored dimension must remain a Profile Offset");
    };
    assert_eq!(restored_operand, &expected_operand);
}

fn assert_profile_offset_prepared_publication() {
    let mut base_with_geometry = SketchDocument::new(4.0).unwrap();
    let (operand, _, _) = document_line_operand(&mut base_with_geometry);
    let mut retained = RetainedSketchDocumentSession::new(
        base_with_geometry,
        geosolve_sketch::DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let prepared = retained
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateProfileOffset {
                label: "offset".into(),
                distance: 2.0,
                operand,
            },
        ));
    let patch = match prepared.execute(OperationControl::unlimited()).unwrap() {
        geosolve_core::OperationOutcome::Completed { value, .. } => value,
        other => panic!("prepared profile offset did not complete: {other:?}"),
    };
    assert!(patch.preview().accepted_state().is_some());
    retained.commit_prepared_patch(patch).unwrap();
    assert_eq!(
        retained
            .design_document()
            .dimensions()
            .iter()
            .filter(|dimension| matches!(
                dimension.definition,
                geosolve_sketch::DocumentDimensionDefinition::ProfileOffset { .. }
            ))
            .count(),
        1
    );
}

fn assert_profile_offset_history_round_trip() {
    let mut history_document = SketchDocument::new(4.0).unwrap();
    let (history_operand, _, _) = document_line_operand(&mut history_document);
    let mut history = SketchDocumentSession::new(
        history_document,
        geosolve_sketch::DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = history
        .apply(DocumentCommand::new(
            history.revision(),
            DocumentEdit::CreateProfileOffset {
                label: "offset".into(),
                distance: 2.0,
                operand: history_operand,
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedProfileOffset(created)) = outcome.effect else {
        panic!("profile offset effect expected");
    };
    let created = *created;
    assert!(history.document().dimension(created.dimension).is_some());
    history.undo(history.revision()).unwrap();
    assert!(history.document().dimension(created.dimension).is_none());
    history.redo(history.revision()).unwrap();
    assert!(history.document().dimension(created.dimension).is_some());
}

#[test]
fn invalid_profile_associations_reject_before_mutation() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let points = [
        sketch.add_point(Point2::new(0.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(1.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(0.0, 1.0)).unwrap(),
        sketch.add_point(Point2::new(1.0, 1.0)).unwrap(),
    ];
    let source = sketch.add_segment(points[0], points[1]).unwrap();
    let target = sketch.add_segment(points[2], points[3]).unwrap();
    let before = sketch.clone();
    let invalid = ProfileOffsetAssociation {
        operand: ProfileOffsetOperand::Face {
            direction: FaceOffsetDirection::Outward,
            outer: ProfileOffsetLoop {
                edges: vec![pair(
                    ProfileOffsetCurve::Line(source),
                    ProfileOffsetCurve::Line(target),
                )],
                junctions: vec![],
            },
            holes: vec![],
        },
    };
    assert!(matches!(
        sketch.add_profile_offset(invalid, 1.0),
        Err(SketchError::InvalidProfileOffset(_))
    ));
    assert_eq!(sketch.dimensions().count(), before.dimensions().count());
    assert_eq!(
        sketch.profile_offsets().count(),
        before.profile_offsets().count()
    );
}

fn rectangle_offset_creation_request(
    document: &mut SketchDocument,
) -> DocumentProfileOffsetCreationRequest {
    let points = [
        document
            .add_point("source bottom left", [-2.0, -1.0])
            .unwrap(),
        document
            .add_point("source bottom right", [2.0, -1.0])
            .unwrap(),
        document.add_point("source top right", [2.0, 1.0]).unwrap(),
        document.add_point("source top left", [-2.0, 1.0]).unwrap(),
    ];
    let mut edges = Vec::new();
    for index in 0..4 {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let start_position = document.point(start).unwrap().position;
        let end_position = document.point(end).unwrap().position;
        let delta = [
            end_position[0] - start_position[0],
            end_position[1] - start_position[1],
        ];
        let length = delta[0].hypot(delta[1]);
        let curve = document
            .add_curve(
                format!("source edge {}", index + 1),
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .unwrap();
        edges.push(DocumentDirectedProfileOffsetCurve {
            curve: CurveSpan::line(curve),
            traversal: DocumentOffsetTraversal::Forward,
        });
    }
    DocumentProfileOffsetCreationRequest {
        label: "rectangle profile offset".into(),
        distance: 0.5,
        operand: DocumentProfileOffsetCreationOperand::Face {
            direction: DocumentFaceOffsetDirection::Outward,
            outer: DocumentProfileOffsetCreationPath {
                edges,
                junctions: points
                    .into_iter()
                    .cycle()
                    .skip(1)
                    .take(4)
                    .map(|point| DocumentProfileOffsetCreationJunction {
                        source_owner: DocumentProfileOffsetJunctionOwner::SharedPoint(point),
                        branch: DocumentProfileOffsetJunctionBranch::Miter {
                            turn: DocumentProfileOffsetTurn::Left,
                        },
                    })
                    .collect(),
            },
            holes: Vec::new(),
        },
    }
}

#[test]
fn atomic_rectangle_construction_persists_both_junction_owners_and_detaches_cleanly() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let request = rectangle_offset_creation_request(&mut document);
    let before = document.clone();
    let ids = document
        .create_profile_offset_geometry(request.clone())
        .unwrap();
    assert_eq!(document.curves().len(), before.curves().len() + 4);
    let dimension = document.dimension(ids.dimension).unwrap();
    let geosolve_sketch::DocumentDimensionDefinition::ProfileOffset { operand, .. } =
        &dimension.definition
    else {
        panic!("created dimension must be a profile offset");
    };
    let DocumentProfileOffsetOperand::Face { outer, holes, .. } = operand else {
        panic!("rectangle must remain a face operand");
    };
    assert!(holes.is_empty());
    assert_eq!(outer.edges.len(), 4);
    assert_eq!(outer.junctions.len(), 4);
    assert!(outer.junctions.iter().all(|junction| {
        matches!(
            junction.source_owner,
            DocumentProfileOffsetJunctionOwner::SharedPoint(_)
        ) && matches!(
            junction.target_owner,
            DocumentProfileOffsetJunctionOwner::SharedPoint(_)
        )
    }));
    let target_curves = outer
        .edges
        .iter()
        .map(|edge| edge.target.curve.curve)
        .collect::<Vec<_>>();
    let target_points = outer
        .junctions
        .iter()
        .map(|junction| match junction.target_owner {
            DocumentProfileOffsetJunctionOwner::SharedPoint(point) => point,
            DocumentProfileOffsetJunctionOwner::Constraint(_) => panic!("line miter owner"),
        })
        .collect::<Vec<_>>();

    let mut session = RetainedSketchDocumentSession::new(
        before,
        geosolve_sketch::DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let prepared = session
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreateProfileOffsetGeometry { request },
        ))
        .execute(OperationControl::unlimited())
        .unwrap();
    let geosolve_core::OperationOutcome::Completed { value: patch, .. } = prepared else {
        panic!("profile-offset construction unexpectedly stopped");
    };
    assert!(patch.preview().accepted_state().is_some());
    let proposed = patch.proposed_commit();
    assert_eq!(session.commit_prepared_patch(patch).unwrap(), proposed);

    document
        .remove_with_owned_state(DocumentObjectId::Dimension(ids.dimension))
        .unwrap();
    assert!(document.dimension(ids.dimension).is_none());
    assert!(document.scalar(ids.target).is_none());
    assert!(
        target_curves
            .iter()
            .all(|curve| document.curve(*curve).is_some())
    );
    assert!(
        target_points
            .iter()
            .all(|point| document.point(*point).is_some())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end fixture keeps mixed target construction and provenance assertions together"
)]
fn mixed_tangent_chain_construction_owns_both_joins_and_explicit_terminals() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let line_start = document.add_point("line start", [-2.0, 0.0]).unwrap();
    let tangent_point = document.add_point("tangent point", [0.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "source line",
            CurveDefinition::Line {
                start: line_start,
                end: tangent_point,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let arc_center = document.add_point("arc center", [0.0, 1.0]).unwrap();
    let arc_radius = document
        .add_scalar(
            "arc radius",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = document
        .add_scalar(
            "arc start",
            -std::f64::consts::FRAC_PI_2,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let arc_end = document
        .add_scalar(
            "arc end",
            0.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "source arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let line_span = CurveSpan::line(line);
    let arc_span = CurveSpan::line(arc);
    let line_contact = document
        .add_curve_contact(
            "source line end",
            line_span,
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let arc_contact = document
        .add_curve_contact(
            "source arc start",
            arc_span,
            0.0,
            0,
            ContactNeighborhood::Start,
            Some(geosolve_sketch::TangentOrientation::Aligned),
        )
        .unwrap();
    let source_owner = document
        .add_constraint(
            "source tangent continuity",
            DocumentConstraintDefinition::EndpointContinuity {
                first_contact: line_contact,
                second_contact: arc_contact,
                continuity: DocumentCurveContinuity::G1,
            },
        )
        .unwrap();
    let ids = document
        .create_profile_offset_geometry(DocumentProfileOffsetCreationRequest {
            label: "mixed tangent offset".into(),
            distance: 0.25,
            operand: DocumentProfileOffsetCreationOperand::OpenChain {
                side: geosolve_sketch::DocumentLineSide::Left,
                chain: DocumentProfileOffsetCreationPath {
                    edges: vec![
                        DocumentDirectedProfileOffsetCurve {
                            curve: line_span,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                        DocumentDirectedProfileOffsetCurve {
                            curve: arc_span,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                    ],
                    junctions: vec![DocumentProfileOffsetCreationJunction {
                        source_owner: DocumentProfileOffsetJunctionOwner::Constraint(source_owner),
                        branch: DocumentProfileOffsetJunctionBranch::Tangent,
                    }],
                },
            },
        })
        .unwrap();
    let definition = &document.dimension(ids.dimension).unwrap().definition;
    let geosolve_sketch::DocumentDimensionDefinition::ProfileOffset {
        operand: DocumentProfileOffsetOperand::OpenChain { chain, .. },
        ..
    } = definition
    else {
        panic!("mixed construction must create an open profile offset");
    };
    assert_eq!(
        chain.start_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    assert_eq!(
        chain.end_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    assert_eq!(
        chain.junctions[0].source_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(source_owner)
    );
    let DocumentProfileOffsetJunctionOwner::Constraint(target_owner) =
        chain.junctions[0].target_owner
    else {
        panic!("mixed target join must retain ordinary continuity ownership");
    };
    let target_curves = chain
        .edges
        .iter()
        .map(|edge| edge.target.curve.curve)
        .collect::<Vec<_>>();
    let before = document.clone();
    assert!(
        document
            .remove_with_owned_state(DocumentObjectId::Constraint(target_owner))
            .is_err()
    );
    assert_eq!(document, before);

    document
        .remove_with_owned_state(DocumentObjectId::Dimension(ids.dimension))
        .expect("deleting only the association");
    assert!(document.constraint(target_owner).is_some());
    assert!(
        target_curves
            .iter()
            .all(|curve| document.curve(*curve).is_some()),
        "ordinary target curves and their connectivity must outlive the association"
    );
    document
        .remove_with_owned_state(DocumentObjectId::Constraint(target_owner))
        .expect("detached ordinary target connectivity becomes independently editable");
}

#[test]
fn construction_rejects_stale_provenance_and_radius_collapse_without_allocating() {
    let mut rectangle = SketchDocument::new(1.0).unwrap();
    let mut request = rectangle_offset_creation_request(&mut rectangle);
    let unrelated = rectangle.add_point("unrelated", [9.0, 9.0]).unwrap();
    let DocumentProfileOffsetCreationOperand::Face { outer, .. } = &mut request.operand else {
        unreachable!();
    };
    outer.junctions[0].source_owner = DocumentProfileOffsetJunctionOwner::SharedPoint(unrelated);
    let before = rectangle.clone();
    assert!(rectangle.create_profile_offset_geometry(request).is_err());
    assert_eq!(rectangle, before);

    let mut circle = SketchDocument::new(1.0).unwrap();
    let center = circle.add_point("center", [0.0, 0.0]).unwrap();
    let radius = circle
        .add_scalar(
            "radius",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let source = circle
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let before = circle.clone();
    let result = circle.create_profile_offset_geometry(DocumentProfileOffsetCreationRequest {
        label: "collapsed circle offset".into(),
        distance: 1.0,
        operand: DocumentProfileOffsetCreationOperand::Face {
            direction: DocumentFaceOffsetDirection::Inward,
            outer: DocumentProfileOffsetCreationPath {
                edges: vec![DocumentDirectedProfileOffsetCurve {
                    curve: CurveSpan::line(source),
                    traversal: DocumentOffsetTraversal::Forward,
                }],
                junctions: Vec::new(),
            },
            holes: Vec::new(),
        },
    });
    assert!(result.is_err());
    assert_eq!(circle, before);
}

#[test]
fn supporting_line_contact_cannot_authenticate_a_profile_offset_endpoint_junction() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let source_start = document.add_point("source start", [0.0, 0.0]).unwrap();
    let source_join_in = document.add_point("source join in", [2.0, 0.0]).unwrap();
    let source_join_out = document.add_point("source join out", [2.0, 0.0]).unwrap();
    let source_end = document.add_point("source end", [2.0, 2.0]).unwrap();
    let target_start = document.add_point("target start", [0.0, 1.0]).unwrap();
    let target_join = document.add_point("target join", [1.0, 1.0]).unwrap();
    let target_end = document.add_point("target end", [1.0, 2.0]).unwrap();
    let source = [
        add_directed_document_line(
            &mut document,
            "source incoming",
            source_start,
            source_join_in,
            [1.0, 0.0],
        ),
        add_directed_document_line(
            &mut document,
            "source outgoing",
            source_join_out,
            source_end,
            [0.0, 1.0],
        ),
    ];
    let target = [
        add_directed_document_line(
            &mut document,
            "target incoming",
            target_start,
            target_join,
            [1.0, 0.0],
        ),
        add_directed_document_line(
            &mut document,
            "target outgoing",
            target_join,
            target_end,
            [0.0, 1.0],
        ),
    ];
    let incoming_contact = document
        .add_curve_contact_with_domain(
            "source incoming support sample",
            source[0].curve,
            ContactDomain::SupportingLine,
            1.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let outgoing_contact = document
        .add_curve_contact_with_domain(
            "source outgoing support sample",
            source[1].curve,
            ContactDomain::SupportingLine,
            0.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let false_owner = document
        .add_constraint(
            "support-only source contact",
            DocumentConstraintDefinition::CurveCurveContact {
                first_contact: incoming_contact,
                second_contact: outgoing_contact,
            },
        )
        .unwrap();
    let operand = DocumentProfileOffsetOperand::OpenChain {
        side: geosolve_sketch::DocumentLineSide::Left,
        chain: DocumentProfileOffsetChain {
            edges: source
                .into_iter()
                .zip(target)
                .map(|(source, target)| DocumentProfileOffsetEdgePair { source, target })
                .collect(),
            junctions: vec![geosolve_sketch::DocumentProfileOffsetJunction {
                source_owner: DocumentProfileOffsetJunctionOwner::Constraint(false_owner),
                target_owner: DocumentProfileOffsetJunctionOwner::SharedPoint(target_join),
                branch: DocumentProfileOffsetJunctionBranch::Miter {
                    turn: DocumentProfileOffsetTurn::Left,
                },
            }],
            start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
        },
    };
    let before = document.clone();

    assert!(
        document
            .add_profile_offset("forged endpoint owner", 1.0, operand)
            .is_err()
    );
    assert_eq!(document, before);
}

fn add_line_arc_endpoint_owners(
    document: &mut SketchDocument,
    label: &str,
    y: f64,
) -> (
    [DocumentDirectedProfileOffsetCurve; 2],
    [geosolve_sketch::DocumentConstraintId; 2],
) {
    let (line, arc, join) = add_line_arc_curves(document, label, y);
    let point_contact = document
        .add_curve_contact(
            format!("{label} point arc endpoint"),
            arc,
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    let tangent_contact = document
        .add_curve_contact(
            format!("{label} tangent arc endpoint"),
            arc,
            0.0,
            0,
            ContactNeighborhood::Start,
            Some(geosolve_sketch::TangentOrientation::Aligned),
        )
        .unwrap();
    let point_owner = document
        .add_constraint(
            format!("{label} point owner"),
            DocumentConstraintDefinition::PointOnCurve {
                point: join,
                contact: point_contact,
            },
        )
        .unwrap();
    let tangent_owner = document
        .add_constraint(
            format!("{label} tangent owner"),
            DocumentConstraintDefinition::LineCurveTangency {
                line,
                endpoint: geosolve_sketch::FeatureEndpoint::End,
                curve_contact: tangent_contact,
            },
        )
        .unwrap();
    (
        [
            DocumentDirectedProfileOffsetCurve {
                curve: line,
                traversal: DocumentOffsetTraversal::Forward,
            },
            DocumentDirectedProfileOffsetCurve {
                curve: arc,
                traversal: DocumentOffsetTraversal::Forward,
            },
        ],
        [point_owner, tangent_owner],
    )
}

fn add_line_arc_curves(
    document: &mut SketchDocument,
    label: &str,
    y: f64,
) -> (CurveSpan, CurveSpan, geosolve_sketch::DesignPointId) {
    let line_start = document
        .add_point(format!("{label} line start"), [-2.0, y])
        .unwrap();
    let join = document
        .add_point(format!("{label} join"), [0.0, y])
        .unwrap();
    let line = document
        .add_curve(
            format!("{label} line"),
            CurveDefinition::Line {
                start: line_start,
                end: join,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let center = document
        .add_point(format!("{label} center"), [0.0, y + 1.0])
        .unwrap();
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let start_angle = document
        .add_scalar(
            format!("{label} start"),
            -std::f64::consts::FRAC_PI_2,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let end_angle = document
        .add_scalar(
            format!("{label} end"),
            0.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            format!("{label} arc"),
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    (CurveSpan::line(line), CurveSpan::line(arc), join)
}

#[test]
fn point_on_curve_and_line_curve_tangency_own_exact_mixed_endpoints() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let (source, source_owners) = add_line_arc_endpoint_owners(&mut document, "source", 0.0);
    let (target, target_owners) = add_line_arc_endpoint_owners(&mut document, "target", 0.5);
    let operand_with = |source_owner, target_owner| DocumentProfileOffsetOperand::OpenChain {
        side: geosolve_sketch::DocumentLineSide::Left,
        chain: DocumentProfileOffsetChain {
            edges: source
                .into_iter()
                .zip(target)
                .map(|(source, target)| DocumentProfileOffsetEdgePair { source, target })
                .collect(),
            junctions: vec![geosolve_sketch::DocumentProfileOffsetJunction {
                source_owner: DocumentProfileOffsetJunctionOwner::Constraint(source_owner),
                target_owner: DocumentProfileOffsetJunctionOwner::Constraint(target_owner),
                branch: DocumentProfileOffsetJunctionBranch::Tangent,
            }],
            start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
        },
    };
    let ids = document
        .add_profile_offset(
            "endpoint owner offset",
            0.5,
            operand_with(source_owners[0], target_owners[0]),
        )
        .unwrap();
    document
        .set_profile_offset_operand(
            ids.dimension,
            operand_with(source_owners[1], target_owners[1]),
        )
        .unwrap();
}

fn add_directed_document_line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
    branch_direction: [f64; 2],
) -> DocumentDirectedProfileOffsetCurve {
    let curve = document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            },
        )
        .unwrap();
    DocumentDirectedProfileOffsetCurve {
        curve: CurveSpan::line(curve),
        traversal: DocumentOffsetTraversal::Forward,
    }
}

fn add_equal_curvature_endpoint_owner(
    document: &mut SketchDocument,
    incoming: DocumentDirectedProfileOffsetCurve,
    outgoing: DocumentDirectedProfileOffsetCurve,
) -> geosolve_sketch::DocumentConstraintId {
    let incoming_contact = document
        .add_curve_contact(
            "source incoming endpoint curvature",
            incoming.curve,
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let outgoing_contact = document
        .add_curve_contact(
            "source outgoing endpoint curvature",
            outgoing.curve,
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "equal curvature is not connectivity",
            DocumentConstraintDefinition::EqualCurvature {
                first_contact: incoming_contact,
                second_contact: outgoing_contact,
                relation: geosolve_sketch::DocumentCurveCurvatureRelation::Signed,
            },
        )
        .unwrap()
}

#[test]
fn equal_curvature_contacts_do_not_own_profile_offset_junctions() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let source_points = [
        document.add_point("source start", [-2.0, 0.0]).unwrap(),
        document
            .add_point("source incoming end", [0.0, 0.0])
            .unwrap(),
        document
            .add_point("disconnected source outgoing start", [1.0, 0.0])
            .unwrap(),
        document.add_point("source end", [1.0, 2.0]).unwrap(),
    ];
    let target_points = [
        document.add_point("target start", [-2.0, 0.5]).unwrap(),
        document.add_point("target junction", [0.0, 0.5]).unwrap(),
        document.add_point("target end", [0.0, 2.5]).unwrap(),
    ];
    let source = [
        add_directed_document_line(
            &mut document,
            "source incoming",
            source_points[0],
            source_points[1],
            [1.0, 0.0],
        ),
        add_directed_document_line(
            &mut document,
            "source outgoing",
            source_points[2],
            source_points[3],
            [0.0, 1.0],
        ),
    ];
    let target = [
        add_directed_document_line(
            &mut document,
            "target incoming",
            target_points[0],
            target_points[1],
            [1.0, 0.0],
        ),
        add_directed_document_line(
            &mut document,
            "target outgoing",
            target_points[1],
            target_points[2],
            [0.0, 1.0],
        ),
    ];
    let arbitrary_two_contact_owner =
        add_equal_curvature_endpoint_owner(&mut document, source[0], source[1]);
    let before = document.clone();

    let result = document.add_profile_offset(
        "invalid junction owner",
        0.5,
        DocumentProfileOffsetOperand::OpenChain {
            side: geosolve_sketch::DocumentLineSide::Left,
            chain: DocumentProfileOffsetChain {
                edges: source
                    .into_iter()
                    .zip(target)
                    .map(|(source, target)| DocumentProfileOffsetEdgePair { source, target })
                    .collect(),
                junctions: vec![geosolve_sketch::DocumentProfileOffsetJunction {
                    source_owner: DocumentProfileOffsetJunctionOwner::Constraint(
                        arbitrary_two_contact_owner,
                    ),
                    target_owner: DocumentProfileOffsetJunctionOwner::SharedPoint(target_points[1]),
                    branch: DocumentProfileOffsetJunctionBranch::Miter {
                        turn: DocumentProfileOffsetTurn::Left,
                    },
                }],
                start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            },
        },
    );

    assert!(
        matches!(
            &result,
            Err(geosolve_sketch::DocumentError::InvalidField { field, .. })
                if *field == "profile offset junction owner"
        ),
        "unexpected result: {result:?}"
    );
    assert_eq!(document, before);
}
