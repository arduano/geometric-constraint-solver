use std::f64::consts::{PI, TAU};

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_geometry::{DirectedParameterTrim, HyperbolaBranch, Point2, SMatrix, Vector2};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ConicId, ConicKind, ConicScalarRole, ConicVectorRole,
    ContactNeighborhood, ContactState, CurveContactNeighborhood, CurveDefinition, CurveId,
    CurveSpan, CurveTangentOrientation, DesignScalarId, DimensionMode, DocumentArcSweep,
    DocumentCommand, DocumentCommandEffect, DocumentConicFeature, DocumentConicMeasurement,
    DocumentConicQueryError, DocumentConstraintDefinition, DocumentEdit, DocumentHyperbolaBranch,
    DocumentObjectId, DocumentSolveRequest, DocumentTrimProjectionError, EllipseAxisObservability,
    FeatureEndpoint, FeatureRef, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, RuntimeCurve,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, ScalarDomain, ScalarUnit, Sketch, SketchBound,
    SketchCurve, SketchCurveContact, SketchDocument, SketchDocumentSession, SketchPatch,
    SketchSession, SketchSessionError, SketchSessionPatch, SketchSolveRequest, SketchSource,
    SolvedConicKind, TangentOrientation, alpha_scenario,
};

#[derive(Clone, Copy)]
struct FamilySet {
    ellipse: ConicId,
    arc: ConicId,
    rational: ConicId,
    parabola: ConicId,
    hyperbola: ConicId,
}

fn add_family_set(sketch: &mut Sketch, scale: f64) -> FamilySet {
    let ellipse_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let ellipse_axis = sketch.add_point(Point2::new(2.0 * scale, 0.0)).unwrap();
    let ellipse = sketch
        .add_named_ellipse("ellipse", ellipse_center, ellipse_axis, 0.5)
        .unwrap();

    let arc_center = sketch.add_point(Point2::new(10.0 * scale, 0.0)).unwrap();
    let arc_axis = sketch.add_point(Point2::new(12.0 * scale, 0.0)).unwrap();
    let arc = sketch
        .add_named_elliptical_arc("elliptical arc", arc_center, arc_axis, 0.6, -0.7, 1.8)
        .unwrap();

    let rational_start = sketch.add_point(Point2::new(20.0 * scale, 0.0)).unwrap();
    let rational_end = sketch
        .add_point(Point2::new(22.0 * scale, 0.2 * scale))
        .unwrap();
    let middle_weight = 0.7;
    let weighted_middle = Vector2::new(21.0 * scale, 2.0 * scale) * middle_weight;
    let rational = sketch
        .add_named_rational_quadratic(
            "rational quadratic",
            rational_start,
            weighted_middle,
            middle_weight,
            rational_end,
        )
        .unwrap();

    let vertex = sketch.add_point(Point2::new(30.0 * scale, 0.0)).unwrap();
    let focus = sketch
        .add_point(Point2::new(31.0 * scale, 0.4 * scale))
        .unwrap();
    let parabola = sketch
        .add_named_parabola_segment(
            "parabola",
            vertex,
            focus,
            DirectedParameterTrim::try_new(-1.0, 1.5).unwrap(),
        )
        .unwrap();

    let hyperbola_center = sketch.add_point(Point2::new(40.0 * scale, 0.0)).unwrap();
    let hyperbola_axis = sketch
        .add_point(Point2::new(42.0 * scale, 0.8 * scale))
        .unwrap();
    let hyperbola = sketch
        .add_named_hyperbola_segment(
            "hyperbola",
            hyperbola_center,
            hyperbola_axis,
            1.2 * scale,
            HyperbolaBranch::Positive,
            DirectedParameterTrim::try_new(-0.6, 0.8).unwrap(),
        )
        .unwrap();

    FamilySet {
        ellipse,
        arc,
        rational,
        parabola,
        hyperbola,
    }
}

fn contact(conic: ConicId, parameter: f64, periodic: bool) -> SketchCurveContact {
    SketchCurveContact {
        curve: SketchCurve::Conic(conic),
        parameter,
        neighborhood: if periodic {
            CurveContactNeighborhood::Interior
        } else if parameter == 0.0 {
            CurveContactNeighborhood::Start
        } else if parameter.to_bits() == 1.0f64.to_bits() {
            CurveContactNeighborhood::End
        } else {
            CurveContactNeighborhood::Local {
                lower: 0.05,
                upper: 0.95,
            }
        },
    }
}

#[test]
fn all_runtime_families_construct_evaluate_and_publish_features() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let ids = add_family_set(&mut sketch, 1.0);
    assert_eq!(sketch.conics().count(), 5);

    for (id, parameter) in [
        (ids.ellipse, 0.37),
        (ids.arc, 0.37),
        (ids.rational, 0.37),
        (ids.parabola, 0.37),
        (ids.hyperbola, 0.37),
    ] {
        let jet = sketch.evaluate_conic(id, parameter).unwrap();
        assert!(jet.position.coords.iter().all(|value| value.is_finite()));
        assert!(jet.first_derivative.iter().all(|value| value.is_finite()));
    }

    assert_eq!(sketch.conic_endpoints(ids.ellipse).unwrap(), None);
    for id in [ids.arc, ids.rational, ids.parabola, ids.hyperbola] {
        let endpoints = sketch.conic_endpoints(id).unwrap().unwrap();
        assert!(
            endpoints
                .iter()
                .all(|point| point.coords.iter().all(|value| value.is_finite()))
        );
    }
    assert_eq!(
        sketch.conic_major_axis_length(ids.ellipse).unwrap(),
        Some(4.0)
    );
    assert_eq!(
        sketch.conic_minor_axis_length(ids.ellipse).unwrap(),
        Some(2.0)
    );
    assert!(
        sketch
            .conic_linear_eccentricity(ids.ellipse)
            .unwrap()
            .unwrap()
            > 0.0
    );
    assert!(sketch.conic_foci(ids.ellipse).unwrap().is_some());
    assert!(sketch.conic_focus(ids.parabola).unwrap().is_some());
    assert!(
        sketch
            .conic_selected_branch_focus(ids.hyperbola)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        sketch.conic_proper_kind(ids.rational).unwrap(),
        Some(geosolve_sketch::ProperConicKind::Ellipse)
    );

    let geometry = sketch.geometry();
    assert_eq!(geometry.conics.len(), 5);
    for solved in &geometry.conics {
        assert!(solved.evaluate(0.37).is_ok());
    }
    let runtime = sketch.conic(ids.rational).unwrap().kind();
    let solved = geometry.conic(ids.rational).unwrap().kind();
    assert!(matches!(
        (runtime, solved),
        (
            ConicKind::RationalQuadratic {
                start,
                weighted_middle,
                middle_weight,
                end,
            },
            SolvedConicKind::RationalQuadratic {
                start: solved_start,
                weighted_middle: solved_middle,
                middle_weight: solved_weight,
                end: solved_end,
            },
        ) if sketch.point(start).unwrap().position() == solved_start
            && weighted_middle == solved_middle
            && middle_weight.to_bits() == solved_weight.to_bits()
            && sketch.point(end).unwrap().position() == solved_end
    ));
}

#[test]
fn runtime_rational_quadratics_preserve_homogeneous_state_and_affine_covariance() {
    let matrix = SMatrix::<f64, 2, 2>::new(2.0, 0.5, -0.3, 1.4);
    let translation = Vector2::new(-4.0, 7.0);
    let transform_point = |point: Point2<f64>| Point2::from(matrix * point.coords + translation);
    let transform_vector = |vector: Vector2<f64>| matrix * vector;
    let start_position = Point2::new(-1.0, 0.25);
    let end_position = Point2::new(2.0, -0.5);
    let weighted_middle = Vector2::new(0.4, 1.7);
    let middle_weight = 0.6;

    let mut sketch = Sketch::new(1.0).unwrap();
    let start = sketch.add_point(start_position).unwrap();
    let end = sketch.add_point(end_position).unwrap();
    let conic = sketch
        .add_rational_quadratic(start, weighted_middle, middle_weight, end)
        .unwrap();
    assert_eq!(sketch.conic(conic).unwrap().points(), vec![start, end]);

    let mapped_start = sketch.add_point(transform_point(start_position)).unwrap();
    let mapped_end = sketch.add_point(transform_point(end_position)).unwrap();
    let mapped_middle = transform_vector(weighted_middle) + translation * middle_weight;
    let mapped = sketch
        .add_rational_quadratic(mapped_start, mapped_middle, middle_weight, mapped_end)
        .unwrap();
    for parameter in [0.0, 0.23, 0.5, 0.91, 1.0] {
        let base = sketch.evaluate_conic(conic, parameter).unwrap();
        let transformed = sketch.evaluate_conic(mapped, parameter).unwrap();
        assert!((transformed.position - transform_point(base.position)).norm() <= 2.0e-12);
        assert!(
            (transformed.first_derivative - transform_vector(base.first_derivative)).norm()
                <= 2.0e-12
        );
    }

    let solved = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(solved.accepted(), "{solved:#?}");
    assert!(matches!(
        solved.geometry.conic(conic).unwrap().kind(),
        SolvedConicKind::RationalQuadratic {
            weighted_middle: solved_middle,
            middle_weight: solved_weight,
            ..
        } if solved_middle == weighted_middle
            && solved_weight.to_bits() == middle_weight.to_bits()
    ));
}

#[test]
fn runtime_rational_domain_allows_zero_and_negative_weights_and_setters_roll_back() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let start = sketch.add_point(Point2::new(-1.0, 0.0)).unwrap();
    let end = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let zero = sketch
        .add_rational_quadratic(start, Vector2::new(0.0, 1.0), 0.0, end)
        .unwrap();
    let negative = sketch
        .add_rational_quadratic(start, Vector2::new(0.0, -0.5), -0.5, end)
        .unwrap();
    for conic in [zero, negative] {
        assert!(sketch.evaluate_conic(conic, 0.5).is_ok());
        assert_eq!(
            sketch.conic_proper_kind(conic).unwrap(),
            Some(geosolve_sketch::ProperConicKind::Ellipse)
        );
    }
    for weight in [-1.0, -1.5] {
        assert!(
            sketch
                .add_rational_quadratic(start, Vector2::new(0.0, 1.0), weight, end)
                .is_err()
        );
    }

    let retained = sketch.conic(zero).unwrap().kind();
    assert!(
        sketch
            .set_conic_weighted_middle(zero, Vector2::zeros())
            .is_err()
    );
    assert_eq!(sketch.conic(zero).unwrap().kind(), retained);
    assert!(
        sketch
            .set_conic_weighted_middle(zero, Vector2::new(f64::NAN, 1.0))
            .is_err()
    );
    assert_eq!(sketch.conic(zero).unwrap().kind(), retained);

    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_middle = Vector2::new(0.0, 1.2);
    let accepted = session
        .apply_patch(SketchSessionPatch::new(
            session.revision(),
            SketchPatch::ConicWeightedMiddle {
                conic: zero,
                weighted_middle: accepted_middle,
            },
        ))
        .unwrap();
    assert!(accepted.accepted(), "{accepted:#?}");
    let retained = session.sketch().conic(zero).unwrap().kind();
    assert!(matches!(
        retained,
        ConicKind::RationalQuadratic {
            weighted_middle,
            ..
        } if weighted_middle == accepted_middle
    ));
    let revision = session.revision();
    assert!(
        session
            .apply_patch(SketchSessionPatch::new(
                revision,
                SketchPatch::ConicWeightedMiddle {
                    conic: zero,
                    weighted_middle: Vector2::zeros(),
                },
            ))
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.sketch().conic(zero).unwrap().kind(), retained);
}

#[test]
#[allow(clippy::too_many_lines)]
fn generic_incidence_jacobians_audit_and_mappings_cover_all_families_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let ids = add_family_set(&mut sketch, scale);
        let definitions = [
            (ids.ellipse, 0.37, true, 5),
            (ids.arc, 0.37, false, 5),
            (ids.rational, 0.37, false, 6),
            (ids.parabola, 0.37, false, 4),
            (ids.hyperbola, 0.37, false, 5),
        ];
        let mut point_sources = Vec::new();
        for (conic, parameter, periodic, expected_incidence) in definitions {
            let point = sketch
                .add_point(sketch.evaluate_conic(conic, parameter).unwrap().position)
                .unwrap();
            let source = sketch
                .add_point_on_curve(point, contact(conic, parameter, periodic))
                .unwrap();
            point_sources.push((conic, source, expected_incidence));
        }

        let center = sketch.add_point(Point2::new(50.0 * scale, 0.0)).unwrap();
        let axis = sketch
            .add_point(Point2::new(52.0 * scale, 1.0 * scale))
            .unwrap();
        let tangent_ellipse = sketch.add_ellipse(center, axis, 0.65).unwrap();
        let arc_center = sketch.add_point(Point2::new(50.0 * scale, 0.0)).unwrap();
        let arc_axis = sketch
            .add_point(Point2::new(52.0 * scale, 1.0 * scale))
            .unwrap();
        let tangent_arc = sketch
            .add_elliptical_arc(arc_center, arc_axis, 0.65, -0.3, 1.2)
            .unwrap();
        let ellipse_arc_tangency = sketch
            .add_curve_curve_tangency(
                contact(tangent_ellipse, 0.3, true),
                contact(tangent_arc, 0.5, false),
                CurveTangentOrientation::Aligned,
            )
            .unwrap();

        let rational_copy = match sketch.conic(ids.rational).unwrap().kind() {
            ConicKind::RationalQuadratic {
                start,
                weighted_middle,
                middle_weight,
                end,
            } => sketch
                .add_rational_quadratic(start, weighted_middle, middle_weight, end)
                .unwrap(),
            _ => unreachable!(),
        };
        let rational_tangency = sketch
            .add_curve_curve_tangency(
                contact(ids.rational, 0.41, false),
                contact(rational_copy, 0.41, false),
                CurveTangentOrientation::Aligned,
            )
            .unwrap();

        let ConicKind::ParabolaSegment {
            vertex,
            focus,
            trim: parabola_trim,
        } = sketch.conic(ids.parabola).unwrap().kind()
        else {
            unreachable!()
        };
        let parabola_vertex = sketch
            .add_point(sketch.point(vertex).unwrap().position())
            .unwrap();
        let parabola_focus = sketch
            .add_point(sketch.point(focus).unwrap().position())
            .unwrap();
        let parabola_copy = sketch
            .add_parabola_segment(parabola_vertex, parabola_focus, parabola_trim)
            .unwrap();
        let parabola_tangency = sketch
            .add_curve_curve_tangency(
                contact(ids.parabola, 0.43, false),
                contact(parabola_copy, 0.43, false),
                CurveTangentOrientation::Aligned,
            )
            .unwrap();

        let ConicKind::HyperbolaSegment {
            center: hyperbola_center,
            transverse_axis_point,
            semi_conjugate,
            trim: hyperbola_trim,
            ..
        } = sketch.conic(ids.hyperbola).unwrap().kind()
        else {
            unreachable!()
        };
        let add_hyperbola_copy = |sketch: &mut Sketch, branch| {
            let center = sketch
                .add_point(sketch.point(hyperbola_center).unwrap().position())
                .unwrap();
            let axis = sketch
                .add_point(sketch.point(transverse_axis_point).unwrap().position())
                .unwrap();
            sketch
                .add_hyperbola_segment(center, axis, semi_conjugate, branch, hyperbola_trim)
                .unwrap()
        };
        let positive_copy = add_hyperbola_copy(&mut sketch, HyperbolaBranch::Positive);
        let positive_hyperbola_tangency = sketch
            .add_curve_curve_tangency(
                contact(ids.hyperbola, 0.39, false),
                contact(positive_copy, 0.39, false),
                CurveTangentOrientation::Aligned,
            )
            .unwrap();
        let negative_first = add_hyperbola_copy(&mut sketch, HyperbolaBranch::Negative);
        let negative_second = add_hyperbola_copy(&mut sketch, HyperbolaBranch::Negative);
        let negative_hyperbola_tangency = sketch
            .add_curve_curve_tangency(
                contact(negative_first, 0.39, false),
                contact(negative_second, 0.39, false),
                CurveTangentOrientation::Aligned,
            )
            .unwrap();

        let request = SketchSolveRequest::default().without_previous_state_preferences();
        let compiled = sketch.compile(request).unwrap();
        let repeated = sketch.compile(request).unwrap();
        assert_eq!(compiled.source_mappings(), repeated.source_mappings());
        assert_eq!(compiled.bound_mappings(), repeated.bound_mappings());
        assert_eq!(
            compiled.conic_vector_variables(),
            repeated.conic_vector_variables()
        );
        assert_eq!(compiled.conic_vector_variables().len(), 2);
        assert_eq!(compiled.conic_scalar_variables().len(), 10);
        let check = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(
            check.all_within(1.0e-6),
            "scale={scale:e}, error={:e}: {check:#?}",
            check.max_relative_error()
        );

        for (conic, source, expected_incidence) in point_sources {
            let mapping = compiled
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == SketchSource::Constraint(source))
                .unwrap();
            let residual = compiled
                .problem()
                .residual(mapping.residual_ids[0])
                .unwrap();
            assert_eq!(residual.incident_variables().len(), expected_incidence);
            if conic == ids.rational {
                let weighted_middle = compiled
                    .variable_for_conic_vector(conic, ConicVectorRole::WeightedMiddle)
                    .unwrap();
                let middle_weight = compiled
                    .variable_for_conic_scalar(conic, ConicScalarRole::MiddleWeight)
                    .unwrap();
                assert!(residual.incident_variables().contains(&weighted_middle));
                assert!(residual.incident_variables().contains(&middle_weight));
            }
            assert_eq!(
                residual.audit_rows()[0].template,
                "(P.x - curve(t).x) / model_scale"
            );
            assert_eq!(
                residual.audit_rows()[1].template,
                "(P.y - curve(t).y) / model_scale"
            );
        }
        for source in [
            ellipse_arc_tangency,
            rational_tangency,
            parabola_tangency,
            positive_hyperbola_tangency,
            negative_hyperbola_tangency,
        ] {
            let mapping = compiled
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == SketchSource::Constraint(source))
                .unwrap();
            let rows = &compiled
                .problem()
                .residual(mapping.residual_ids[0])
                .unwrap()
                .audit_rows();
            assert_eq!(rows.len(), 3);
            assert_eq!(
                rows[2].template,
                "cross(unit(first_curve'(t1)), unit(second_curve'(t2)))"
            );
        }

        let scalar_roles = compiled
            .conic_scalar_variables()
            .iter()
            .map(|mapping| (mapping.conic_id, mapping.role))
            .collect::<Vec<_>>();
        assert!(scalar_roles.contains(&(ids.ellipse, ConicScalarRole::MinorAxisRatio)));
        assert!(scalar_roles.contains(&(ids.arc, ConicScalarRole::MinorAxisRatio)));
        assert!(scalar_roles.contains(&(ids.rational, ConicScalarRole::MiddleWeight)));
        assert!(scalar_roles.contains(&(ids.hyperbola, ConicScalarRole::SemiConjugate)));
        let weighted_middle = compiled
            .variable_for_conic_vector(ids.rational, ConicVectorRole::WeightedMiddle)
            .unwrap();
        assert_eq!(
            compiled
                .problem()
                .variable(weighted_middle)
                .unwrap()
                .step_scales(),
            [scale, scale]
        );
        assert!(compiled.bound_mappings().iter().all(|mapping| {
            compiled
                .problem()
                .bound(mapping.bound_id)
                .is_some_and(|bound| {
                    bound.lower().is_none_or(f64::is_finite)
                        && bound.upper().is_none_or(f64::is_finite)
                })
        }));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn perturbed_shape_scalars_and_contacts_recover_for_every_family() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let ids = add_family_set(&mut sketch, scale);
        let definitions = [
            (ids.ellipse, 0.37, true),
            (ids.arc, 0.37, false),
            (ids.rational, 0.37, false),
            (ids.parabola, 0.37, false),
            (ids.hyperbola, 0.37, false),
        ];
        let mut sources = Vec::new();
        for (conic, parameter, periodic) in definitions {
            for point in sketch.conic(conic).unwrap().kind().points() {
                if !sketch.constraints().any(|(_, constraint)| {
                    matches!(constraint.kind(), geosolve_sketch::SketchConstraintKind::FixedPoint { point: id, .. } if id == point)
                }) {
                    sketch.add_fixed_point(point).unwrap();
                }
            }
            let point = sketch
                .add_point(sketch.evaluate_conic(conic, parameter).unwrap().position)
                .unwrap();
            sketch.add_fixed_point(point).unwrap();
            sources.push(
                sketch
                    .add_point_on_curve(point, contact(conic, parameter, periodic))
                    .unwrap(),
            );
        }
        for parameter in [0.2, 0.7] {
            let point = sketch
                .add_point(
                    sketch
                        .evaluate_conic(ids.rational, parameter)
                        .unwrap()
                        .position,
                )
                .unwrap();
            sketch.add_fixed_point(point).unwrap();
            sources.push(
                sketch
                    .add_point_on_curve(point, contact(ids.rational, parameter, false))
                    .unwrap(),
            );
        }

        sketch
            .set_conic_minor_axis_ratio(ids.ellipse, 0.58)
            .unwrap();
        sketch.set_conic_minor_axis_ratio(ids.arc, 0.68).unwrap();
        sketch.set_conic_middle_weight(ids.rational, 0.82).unwrap();
        sketch
            .set_conic_semi_conjugate(ids.hyperbola, 1.35 * scale)
            .unwrap();
        for source in sources {
            let ContactState::PointOnCurve { parameter } = sketch.contact_state(source).unwrap()
            else {
                unreachable!()
            };
            sketch
                .set_contact_state(
                    source,
                    ContactState::PointOnCurve {
                        parameter: parameter + 0.025,
                    },
                )
                .unwrap();
        }

        let result = sketch
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap();
        assert!(result.accepted(), "scale={scale:e}: {result:#?}");
        assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
        assert!(result.core_report.hard_residual_max <= 1.0e-9);
        let scalar = |id| sketch.conic(id).unwrap().kind();
        assert!(
            matches!(scalar(ids.ellipse), ConicKind::Ellipse { minor_axis_ratio, .. } if (minor_axis_ratio - 0.5).abs() <= 1.0e-8)
        );
        assert!(
            matches!(scalar(ids.arc), ConicKind::EllipticalArc { minor_axis_ratio, .. } if (minor_axis_ratio - 0.6).abs() <= 1.0e-8)
        );
        assert!(
            matches!(scalar(ids.rational), ConicKind::RationalQuadratic { middle_weight, .. } if (middle_weight - 0.7).abs() <= 1.0e-8)
        );
        assert!(
            matches!(scalar(ids.hyperbola), ConicKind::HyperbolaSegment { semi_conjugate, .. } if (semi_conjugate / scale - 1.2).abs() <= 1.0e-8)
        );
        assert!(
            result
                .core_report
                .audit
                .sources
                .iter()
                .all(|source| source.rows.iter().all(|row| row.evaluation_status
                    == AuditEvaluationStatus::Evaluated
                    && row.raw_residual.is_finite()
                    && row.normalized_residual.is_finite()))
        );
    }
}

#[test]
fn full_ellipse_is_periodic_and_circle_limit_has_one_orientation_gauge() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::origin()).unwrap();
    let axis = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let ellipse = sketch.add_ellipse(center, axis, 1.0).unwrap();
    sketch.add_fixed_point(center).unwrap();
    sketch
        .add_point_distance(center, axis, 2.0, DimensionMode::Driving)
        .unwrap();

    let mut contacts = Vec::new();
    for parameter in [0.4, 1.6] {
        let point = sketch
            .add_point(sketch.evaluate_conic(ellipse, parameter).unwrap().position)
            .unwrap();
        sketch.add_fixed_point(point).unwrap();
        contacts.push(
            sketch
                .add_point_on_curve(point, contact(ellipse, parameter, true))
                .unwrap(),
        );
    }
    sketch
        .set_contact_state(
            contacts[0],
            ContactState::PointOnCurve {
                parameter: 0.4 + TAU,
            },
        )
        .unwrap();
    let solved = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.accepted(), "{solved:#?}");
    assert_eq!(solved.core_report.rank, 4);
    assert_eq!(solved.core_report.right_nullity, 1);
    assert_eq!(
        sketch.conic_axis_observability(ellipse).unwrap(),
        Some(EllipseAxisObservability::UnobservableCircleLimit)
    );
    let ContactState::PointOnCurve { parameter } = sketch.contact_state(contacts[0]).unwrap()
    else {
        unreachable!()
    };
    assert!((parameter - (0.4 + TAU)).abs() <= 1.0e-9);
    let base = sketch.evaluate_conic(ellipse, -0.3).unwrap().position;
    let wrapped = sketch
        .evaluate_conic(ellipse, -0.3 + 3.0 * TAU)
        .unwrap()
        .position;
    assert!((base - wrapped).norm() <= 1.0e-12);
}

#[test]
fn circle_limit_arc_trim_makes_directed_orientation_observable() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::origin()).unwrap();
    let axis = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let arc = sketch
        .add_elliptical_arc(center, axis, 1.0, 0.0, PI / 2.0)
        .unwrap();
    sketch.add_fixed_point(center).unwrap();
    sketch
        .add_point_distance(center, axis, 2.0, DimensionMode::Driving)
        .unwrap();

    let endpoints = sketch.conic_endpoints(arc).unwrap().unwrap();
    for (parameter, neighborhood, endpoint) in [
        (0.0, CurveContactNeighborhood::Start, endpoints[0]),
        (1.0, CurveContactNeighborhood::End, endpoints[1]),
    ] {
        let point = sketch.add_point(endpoint).unwrap();
        sketch.add_fixed_point(point).unwrap();
        sketch
            .add_point_on_curve(
                point,
                SketchCurveContact {
                    curve: SketchCurve::Conic(arc),
                    parameter,
                    neighborhood,
                },
            )
            .unwrap();
    }

    let solved = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.accepted(), "{solved:#?}");
    assert_eq!(solved.core_report.right_nullity, 1);
    assert_eq!(solved.core_report.bidirectional_degrees_of_freedom, 0);
    assert_eq!(sketch.conic_endpoints(arc).unwrap().unwrap(), endpoints);
    assert_eq!(
        sketch.conic_axis_observability(arc).unwrap(),
        Some(EllipseAxisObservability::ObservableByDirectedTrim)
    );
}

#[test]
fn direct_and_session_conic_acceptance_clamp_loose_solver_tolerances() {
    let loose = SolverConfig {
        normalized_residual_tolerance: 1.0e-2,
        ..SolverConfig::default()
    };

    let (mut inconsistent, _) = rational_endpoint_fixture(5.0e-8);
    let rejected = inconsistent
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            loose,
        )
        .unwrap();
    assert!(!rejected.accepted(), "{rejected:#?}");
    assert_eq!(rejected.core_report.hard_validity, HardValidity::Invalid);
    assert!(rejected.core_report.hard_residual_max > SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE);
    assert!(rejected.core_report.hard_residual_max < loose.normalized_residual_tolerance);

    let (inconsistent, _) = rational_endpoint_fixture(5.0e-8);
    let session_rejection = SketchSession::new(
        inconsistent,
        SketchSolveRequest::default().without_previous_state_preferences(),
        loose,
    );
    assert!(
        matches!(
            session_rejection,
            Err(SketchSessionError::CoreSession(
                geosolve_core::SessionError::InitialRejected(_)
            ))
        ),
        "{session_rejection:#?}"
    );

    let (mut exact, conic) = rational_endpoint_fixture(0.0);
    let direct = exact
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            loose,
        )
        .unwrap();
    assert!(direct.accepted(), "{direct:#?}");
    assert!(direct.geometry.conic(conic).is_some());
    assert!(direct.acceptance_hard_residual_max.unwrap() <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE);

    let (exact, conic) = rational_endpoint_fixture(0.0);
    let session = SketchSession::new(
        exact,
        SketchSolveRequest::default().without_previous_state_preferences(),
        loose,
    )
    .unwrap();
    assert!(session.accepted_result().geometry.conic(conic).is_some());
    assert!(
        session
            .accepted_result()
            .acceptance_hard_residual_max
            .unwrap()
            <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
    );

    let strict = SolverConfig {
        normalized_residual_tolerance: 1.0e-12,
        ..SolverConfig::default()
    };
    let (mut stricter_fixture, _) = rational_endpoint_fixture(5.0e-10);
    let stricter_rejection = stricter_fixture
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            strict,
        )
        .unwrap();
    assert!(!stricter_rejection.accepted(), "{stricter_rejection:#?}");
    assert_eq!(
        stricter_rejection.core_report.hard_validity,
        HardValidity::Invalid
    );
    assert!(
        stricter_rejection.core_report.hard_residual_max > strict.normalized_residual_tolerance
    );
}

#[test]
fn hyperbola_branches_and_reversed_trims_are_explicit_and_retained() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::new(1.0, -2.0)).unwrap();
    let axis = sketch.add_point(Point2::new(3.0, -2.0)).unwrap();
    let trim = DirectedParameterTrim::try_new(2.0, -1.0).unwrap();
    let hyperbola = sketch
        .add_hyperbola_segment(center, axis, 1.25, HyperbolaBranch::Negative, trim)
        .unwrap();
    let negative = sketch.conic_geometry(hyperbola).unwrap();
    let negative_point = negative.evaluate(0.5).unwrap().position;
    assert!(negative_point.x < sketch.point(center).unwrap().position().x);
    assert_eq!(
        sketch.conic_endpoints(hyperbola).unwrap().unwrap()[0],
        negative.evaluate(0.0).unwrap().position
    );

    sketch
        .set_hyperbola_branch(hyperbola, HyperbolaBranch::Positive)
        .unwrap();
    let positive = sketch.conic_geometry(hyperbola).unwrap();
    let positive_point = positive.evaluate(0.5).unwrap().position;
    assert!(positive_point.x > sketch.point(center).unwrap().position().x);
    assert!(matches!(
        sketch.conic(hyperbola).unwrap().kind(),
        ConicKind::HyperbolaSegment {
            branch: HyperbolaBranch::Positive,
            trim: retained,
            ..
        } if retained == trim && retained.signed_rate() < 0.0
    ));
}

#[test]
fn invalid_scalars_axes_foci_poles_and_overflow_never_construct_or_commit() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let center = sketch.add_point(Point2::origin()).unwrap();
    let axis = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    assert!(sketch.add_ellipse(center, axis, 0.0).is_err());
    assert!(sketch.add_ellipse(center, center, 0.5).is_err());
    assert!(
        sketch
            .add_parabola_segment(
                center,
                center,
                DirectedParameterTrim::try_new(-1.0, 1.0).unwrap()
            )
            .is_err()
    );

    let start = sketch.add_point(Point2::new(-1.0, 0.0)).unwrap();
    let end = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let weighted_middle = Vector2::new(0.0, 1.0);
    assert!(
        sketch
            .add_rational_quadratic(start, weighted_middle, -1.0, end)
            .is_err()
    );
    assert!(
        sketch
            .add_rational_quadratic(start, weighted_middle, -2.0, end)
            .is_err()
    );
    let rational = sketch
        .add_rational_quadratic(start, weighted_middle, 0.7, end)
        .unwrap();
    assert!(sketch.set_conic_middle_weight(rational, -1.0).is_err());
    assert!(matches!(
        sketch.conic(rational).unwrap().kind(),
        ConicKind::RationalQuadratic {
            middle_weight: 0.7,
            ..
        }
    ));

    let ellipse = sketch.add_ellipse(center, axis, 0.5).unwrap();
    assert!(sketch.set_conic_minor_axis_ratio(ellipse, 1.1).is_err());
    assert!(matches!(
        sketch.conic(ellipse).unwrap().kind(),
        ConicKind::Ellipse {
            minor_axis_ratio: 0.5,
            ..
        }
    ));
    assert!(
        sketch
            .add_hyperbola_segment(
                center,
                axis,
                1.0,
                HyperbolaBranch::Positive,
                DirectedParameterTrim::try_new(0.0, 1_000.0).unwrap()
            )
            .is_err()
    );
    assert!(
        sketch
            .add_elliptical_arc(center, axis, 0.5, 0.0, f64::MAX)
            .is_err()
    );
}

#[test]
fn failed_session_axis_edit_retains_accepted_conic_geometry_and_scalar() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::origin()).unwrap();
    let axis = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let ellipse = sketch.add_ellipse(center, axis, 0.5).unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.accepted_result().geometry.clone();
    let before_revision = session.revision();
    let failed = session.apply_patch(SketchSessionPatch::new(
        before_revision,
        SketchPatch::PointPosition {
            point: axis,
            position: Point2::origin(),
        },
    ));
    let failed = failed.unwrap();
    assert!(!failed.accepted(), "{failed:#?}");
    assert_eq!(session.revision(), before_revision);
    assert_eq!(session.accepted_result().geometry, before);
    assert!(matches!(
        session.sketch().conic(ellipse).unwrap().kind(),
        ConicKind::Ellipse {
            minor_axis_ratio: 0.5,
            ..
        }
    ));
}

#[test]
fn conic_scalar_bounds_are_typed_deterministic_and_finite() {
    let mut sketch = Sketch::new(3.0).unwrap();
    let ids = add_family_set(&mut sketch, 1.0);
    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let expected = [
        (ids.ellipse, ConicScalarRole::MinorAxisRatio),
        (ids.arc, ConicScalarRole::MinorAxisRatio),
        (ids.rational, ConicScalarRole::MiddleWeight),
        (ids.hyperbola, ConicScalarRole::SemiConjugate),
    ];
    for (conic_id, role) in expected {
        let variable = compiled.variable_for_conic_scalar(conic_id, role).unwrap();
        let mapping = compiled
            .bound_mappings()
            .iter()
            .find(|mapping| mapping.bound == SketchBound::ConicScalar { conic_id, role })
            .unwrap();
        let bound = compiled.problem().bound(mapping.bound_id).unwrap();
        assert_eq!(bound.variable_id(), variable);
        assert_eq!(
            bound.lower(),
            Some(if role == ConicScalarRole::MiddleWeight {
                MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT
            } else {
                f64::from_bits(1)
            })
        );
        assert_eq!(
            bound.upper(),
            (role == ConicScalarRole::MinorAxisRatio).then_some(1.0)
        );
    }
    assert!(
        compiled
            .variable_for_conic_scalar(ids.parabola, ConicScalarRole::SemiConjugate)
            .is_none()
    );
    let solved = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(solved.accepted(), "{solved:#?}");
    assert!(solved.geometry.conics.iter().all(|conic| {
        matches!(
            conic.kind,
            SolvedConicKind::Ellipse { .. }
                | SolvedConicKind::EllipticalArc { .. }
                | SolvedConicKind::RationalQuadratic { .. }
                | SolvedConicKind::ParabolaSegment { .. }
                | SolvedConicKind::HyperbolaSegment { .. }
        ) && conic.evaluate(0.37).is_ok()
    }));
}

#[test]
fn bounded_conic_contacts_reject_escape_while_full_ellipse_remains_unwrapped() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let ids = add_family_set(&mut sketch, 1.0);
    for id in [ids.arc, ids.rational, ids.parabola, ids.hyperbola] {
        let point = sketch
            .add_point(sketch.evaluate_conic(id, 0.5).unwrap().position)
            .unwrap();
        assert!(
            sketch
                .add_point_on_curve(
                    point,
                    SketchCurveContact {
                        curve: SketchCurve::Conic(id),
                        parameter: 1.01,
                        neighborhood: CurveContactNeighborhood::Interior,
                    }
                )
                .is_err()
        );
    }
    let point = sketch
        .add_point(
            sketch
                .evaluate_conic(ids.ellipse, 10.0 * PI)
                .unwrap()
                .position,
        )
        .unwrap();
    assert!(
        sketch
            .add_point_on_curve(point, contact(ids.ellipse, 10.0 * PI, true))
            .is_ok()
    );
}

fn rational_endpoint_fixture(offset: f64) -> (Sketch, ConicId) {
    let mut sketch = Sketch::new(1.0).unwrap();
    let start = sketch.add_point(Point2::origin()).unwrap();
    let end = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let conic = sketch
        .add_rational_quadratic(start, Vector2::new(0.5, 1.0), 0.5, end)
        .unwrap();
    let point = sketch.add_point(Point2::new(offset, 0.0)).unwrap();
    sketch.add_fixed_point(start).unwrap();
    sketch.add_fixed_point(point).unwrap();
    sketch
        .add_point_on_curve(
            point,
            SketchCurveContact {
                curve: SketchCurve::Conic(conic),
                parameter: 0.0,
                neighborhood: CurveContactNeighborhood::Start,
            },
        )
        .unwrap();
    (sketch, conic)
}

#[derive(Clone, Copy)]
struct PersistentFamilySet {
    ellipse: geosolve_sketch::CurveId,
    arc: geosolve_sketch::CurveId,
    rational: geosolve_sketch::CurveId,
    parabola: geosolve_sketch::CurveId,
    hyperbola: geosolve_sketch::CurveId,
    ellipse_ratio: geosolve_sketch::DesignScalarId,
    rational_weight: geosolve_sketch::DesignScalarId,
    hyperbola_semi_conjugate: geosolve_sketch::DesignScalarId,
}

fn bounded_parameter(lower: f64, upper: f64) -> ScalarDomain {
    ScalarDomain::Bounded { lower, upper }
}

#[allow(clippy::too_many_lines)]
fn add_persistent_family_set(document: &mut SketchDocument) -> PersistentFamilySet {
    let ratio_domain = bounded_parameter(f64::from_bits(1), 1.0);
    let weight_domain = bounded_parameter(MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, f64::MAX);

    let ellipse_center = document.add_point("ellipse center", [0.0, 0.0]).unwrap();
    let ellipse_axis = document.add_point("ellipse axis", [2.0, 0.0]).unwrap();
    let ellipse_ratio = document
        .add_scalar("ellipse ratio", 0.5, ScalarUnit::Parameter, ratio_domain)
        .unwrap();
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_axis,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .unwrap();

    let arc_center = document.add_point("arc center", [10.0, 0.0]).unwrap();
    let arc_axis = document.add_point("arc axis", [12.0, 0.0]).unwrap();
    let arc_ratio = document
        .add_scalar("arc ratio", 0.6, ScalarUnit::Parameter, ratio_domain)
        .unwrap();
    let arc_start = document
        .add_scalar("arc start", 1.2, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = document
        .add_scalar("arc end", -0.4, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = document
        .add_curve(
            "elliptical arc",
            CurveDefinition::EllipticalArc {
                center: arc_center,
                major_axis_point: arc_axis,
                minor_axis_ratio: arc_ratio,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::Clockwise,
            },
        )
        .unwrap();

    let rational_start = document.add_point("rational start", [20.0, 0.0]).unwrap();
    let rational_end = document.add_point("rational end", [22.0, 0.2]).unwrap();
    let rational_weight = document
        .add_scalar("rational weight", 0.7, ScalarUnit::Parameter, weight_domain)
        .unwrap();
    let rational = document
        .add_curve(
            "rational quadratic",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [14.7, 1.4],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .unwrap();

    let vertex = document.add_point("parabola vertex", [30.0, 0.0]).unwrap();
    let focus = document.add_point("parabola focus", [31.0, 0.0]).unwrap();
    let parabola_start = document
        .add_scalar(
            "parabola trim start",
            1.5,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola_end = document
        .add_scalar(
            "parabola trim end",
            -1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola = document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start: parabola_start,
                trim_end: parabola_end,
            },
        )
        .unwrap();

    let hyperbola_center = document.add_point("hyperbola center", [40.0, 0.0]).unwrap();
    let hyperbola_axis = document.add_point("hyperbola axis", [42.0, 0.8]).unwrap();
    let hyperbola_semi_conjugate = document
        .add_scalar(
            "hyperbola semi-conjugate",
            1.2,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let hyperbola_start = document
        .add_scalar(
            "hyperbola trim start",
            0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola_end = document
        .add_scalar(
            "hyperbola trim end",
            -0.6,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola = document
        .add_curve(
            "hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate: hyperbola_semi_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .unwrap();

    PersistentFamilySet {
        ellipse,
        arc,
        rational,
        parabola,
        hyperbola,
        ellipse_ratio,
        rational_weight,
        hyperbola_semi_conjugate,
    }
}

#[test]
#[allow(clippy::float_cmp, clippy::too_many_lines)]
fn persistent_families_validate_query_round_trip_lower_and_solve() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let ids = add_persistent_family_set(&mut document);
    let ellipse_contact = document
        .add_curve_contact(
            "wound ellipse contact",
            CurveSpan::line(ids.ellipse),
            0.4,
            2,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let ellipse_point = document
        .add_point(
            "ellipse contact point",
            document
                .evaluate_contact_jet(ellipse_contact)
                .unwrap()
                .position
                .coords
                .into(),
        )
        .unwrap();
    document
        .add_constraint(
            "point on wound ellipse",
            DocumentConstraintDefinition::PointOnCurve {
                point: ellipse_point,
                contact: ellipse_contact,
            },
        )
        .unwrap();
    document.validate().unwrap();
    assert_eq!(
        document.scalar(ids.ellipse_ratio).unwrap().domain,
        bounded_parameter(f64::from_bits(1), 1.0)
    );
    assert_eq!(
        document
            .scalar(ids.hyperbola_semi_conjugate)
            .unwrap()
            .domain,
        ScalarDomain::Positive
    );

    for feature in [
        FeatureRef::CurveCenter { curve: ids.ellipse },
        FeatureRef::CurveAxis { curve: ids.arc },
        FeatureRef::CurveAxis {
            curve: ids.parabola,
        },
        FeatureRef::CurveFocus {
            curve: ids.hyperbola,
            index: 1,
        },
        FeatureRef::CurveEndpoint {
            curve: ids.rational,
            endpoint: FeatureEndpoint::End,
        },
    ] {
        document.validate_feature(feature).unwrap();
    }
    assert!(
        document
            .validate_feature(FeatureRef::CurveEndpoint {
                curve: ids.ellipse,
                endpoint: FeatureEndpoint::Start,
            })
            .is_err()
    );
    assert!(
        document
            .validate_feature(FeatureRef::CurveControl {
                curve: ids.rational,
                index: 1,
            })
            .is_err()
    );

    assert_eq!(
        document
            .evaluate_conic_feature(ids.ellipse, DocumentConicFeature::Center)
            .unwrap(),
        [0.0, 0.0]
    );
    assert_eq!(
        document
            .measure_conic(ids.ellipse, DocumentConicMeasurement::MajorAxisLength)
            .unwrap(),
        4.0
    );
    assert_eq!(
        document
            .measure_conic(ids.ellipse, DocumentConicMeasurement::MinorAxisLength)
            .unwrap(),
        2.0
    );
    assert!(
        document
            .evaluate_conic_feature(ids.parabola, DocumentConicFeature::Focus { index: 0 })
            .unwrap()
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(
        document
            .evaluate_conic_feature(ids.hyperbola, DocumentConicFeature::SelectedBranchVertex,)
            .unwrap()[0]
            < 40.0
    );
    assert!(matches!(
        document.measure_conic(ids.rational, DocumentConicMeasurement::MajorAxisLength),
        Err(DocumentConicQueryError::UnsupportedMeasurement { .. })
    ));

    let json = document.to_canonical_json().unwrap();
    let imported = SketchDocument::from_json(&json).unwrap();
    assert_eq!(imported.to_canonical_json().unwrap(), json);
    assert_eq!(
        imported.contact(ellipse_contact).unwrap().winding,
        2,
        "full-ellipse winding is persistent state"
    );
    let CurveDefinition::RationalQuadraticConic {
        weighted_middle, ..
    } = imported.curve(ids.rational).unwrap().definition
    else {
        panic!("rational conic expected")
    };
    assert_eq!(
        weighted_middle.map(f64::to_bits),
        [14.7, 1.4].map(f64::to_bits)
    );

    let first = imported.lower().unwrap();
    let second = imported.lower().unwrap();
    for curve in [
        ids.ellipse,
        ids.arc,
        ids.rational,
        ids.parabola,
        ids.hyperbola,
    ] {
        assert!(matches!(
            first.mappings().runtime_curve(curve),
            Some(RuntimeCurve::Conic(_))
        ));
        assert_eq!(
            first.mappings().runtime_conic(curve),
            second.mappings().runtime_conic(curve)
        );
    }
    let session = SketchDocumentSession::new(
        imported,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.accepted_result().accepted());
    assert!(
        session
            .accepted_result()
            .solve()
            .acceptance_hard_residual_max
            .unwrap()
            <= 1.0e-9
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn persistent_generic_conic_sources_use_common_audit_templates() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let ids = add_persistent_family_set(&mut document);
    let parameter = 0.37;
    let point = document
        .add_point(
            "point on ellipse",
            document
                .evaluate_curve_jet(CurveSpan::line(ids.ellipse), parameter)
                .unwrap()
                .position
                .coords
                .into(),
        )
        .unwrap();
    let point_contact = document
        .add_curve_contact(
            "ellipse point contact",
            CurveSpan::line(ids.ellipse),
            parameter,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let point_source = document
        .add_constraint(
            "generic point conic",
            DocumentConstraintDefinition::PointOnCurve {
                point,
                contact: point_contact,
            },
        )
        .unwrap();

    let center = document.add_point("copy center", [0.0, 0.0]).unwrap();
    let axis = document.add_point("copy axis", [2.0, 0.0]).unwrap();
    let ratio = document
        .add_scalar(
            "copy ratio",
            0.5,
            ScalarUnit::Parameter,
            bounded_parameter(f64::from_bits(1), 1.0),
        )
        .unwrap();
    let copy = document
        .add_curve(
            "ellipse copy",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let first_contact = document
        .add_curve_contact(
            "first tangent contact",
            CurveSpan::line(ids.ellipse),
            0.61,
            0,
            ContactNeighborhood::Interior,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let second_contact = document
        .add_curve_contact(
            "second tangent contact",
            CurveSpan::line(copy),
            0.61,
            0,
            ContactNeighborhood::Interior,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let tangent_source = document
        .add_constraint(
            "generic conic conic tangency",
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            },
        )
        .unwrap();

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_result();
    assert!(accepted.accepted(), "{:#?}", accepted.solve().core_report);
    assert!(accepted.solve().core_report.hard_residual_max <= 1.0e-9);
    let compiled = session
        .runtime()
        .sketch()
        .compile(SketchSolveRequest::default())
        .unwrap();
    for (persistent, templates) in [
        (
            point_source,
            vec![
                "(P.x - curve(t).x) / model_scale",
                "(P.y - curve(t).y) / model_scale",
            ],
        ),
        (
            tangent_source,
            vec![
                "(first_curve(t1).x - second_curve(t2).x) / model_scale",
                "(first_curve(t1).y - second_curve(t2).y) / model_scale",
                "cross(unit(first_curve'(t1)), unit(second_curve'(t2)))",
            ],
        ),
    ] {
        let source = session.document().constraint(persistent).unwrap().source_id;
        let geosolve_sketch::RuntimeSource::Constraint(runtime) =
            session.mappings().runtime_source(source).unwrap()
        else {
            panic!("runtime constraint expected")
        };
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
            .unwrap();
        let rows = compiled
            .problem()
            .residual(mapping.residual_ids[0])
            .unwrap()
            .audit_rows();
        assert_eq!(
            rows.iter()
                .map(|row| row.template.as_str())
                .collect::<Vec<_>>(),
            templates
        );
    }
    let roles = session
        .mappings()
        .contact_mappings()
        .iter()
        .map(|mapping| (mapping.persistent, mapping.role))
        .collect::<Vec<_>>();
    assert!(roles.contains(&(
        point_contact,
        geosolve_sketch::DocumentContactRole::ConicParameter
    )));
    assert!(roles.contains(&(
        first_contact,
        geosolve_sketch::DocumentContactRole::FirstCurveParameter
    )));
    assert!(roles.contains(&(
        second_contact,
        geosolve_sketch::DocumentContactRole::SecondCurveParameter
    )));
}

#[test]
#[allow(clippy::too_many_lines)]
fn persistent_conic_commands_history_and_failed_edits_are_atomic() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let ids = add_persistent_family_set(&mut document);
    let created_center = document.add_point("created center", [60.0, 0.0]).unwrap();
    let created_axis = document.add_point("created axis", [62.0, 0.0]).unwrap();
    let created_ratio = document
        .add_scalar(
            "created ratio",
            0.8,
            ScalarUnit::Parameter,
            bounded_parameter(f64::from_bits(1), 1.0),
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateCurve {
                label: "command ellipse".into(),
                definition: CurveDefinition::Ellipse {
                    center: created_center,
                    major_axis_point: created_axis,
                    minor_axis_ratio: created_ratio,
                },
            },
        ))
        .unwrap();
    assert!(created.accepted());
    let Some(DocumentCommandEffect::CreatedCurve(created_curve)) = created.effect else {
        panic!("created curve effect expected")
    };
    assert!(matches!(
        session.mappings().runtime_curve(created_curve),
        Some(RuntimeCurve::Conic(_))
    ));

    let weighted_middle = [14.8, 1.5];
    let weighted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetConicWeightedMiddle {
                curve: ids.rational,
                weighted_middle,
            },
        ))
        .unwrap();
    assert!(weighted.accepted());
    assert_eq!(
        weighted.effect,
        Some(DocumentCommandEffect::UpdatedConicWeightedMiddle(
            ids.rational
        ))
    );
    let branched = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetHyperbolaBranch {
                curve: ids.hyperbola,
                branch: DocumentHyperbolaBranch::Positive,
            },
        ))
        .unwrap();
    assert!(branched.accepted());
    assert_eq!(
        branched.effect,
        Some(DocumentCommandEffect::UpdatedHyperbolaBranch(ids.hyperbola))
    );
    let accepted_json = session.export_json().unwrap();
    let accepted_revision = session.revision();
    let accepted_history = session.history_len();

    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetConicWeightedMiddle {
                    curve: ids.rational,
                    weighted_middle: [f64::NAN, 0.0],
                },
            ))
            .is_err()
    );
    assert_eq!(session.export_json().unwrap(), accepted_json);
    assert_eq!(session.revision(), accepted_revision);
    assert_eq!(session.history_len(), accepted_history);
    assert!(
        session
            .apply(DocumentCommand::new(
                accepted_revision - 1,
                DocumentEdit::SetHyperbolaBranch {
                    curve: ids.hyperbola,
                    branch: DocumentHyperbolaBranch::Negative,
                },
            ))
            .is_err()
    );
    assert_eq!(session.export_json().unwrap(), accepted_json);

    session.undo(session.revision()).unwrap();
    assert!(matches!(
        session.document().curve(ids.hyperbola).unwrap().definition,
        CurveDefinition::HyperbolaSegment {
            branch: DocumentHyperbolaBranch::Negative,
            ..
        }
    ));
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), accepted_json);

    let weight = ids.rational_weight;
    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Curve(ids.rational),
            },
        ))
        .unwrap();
    assert!(deleted.accepted());
    assert!(session.document().curve(ids.rational).is_none());
    assert!(session.document().scalar(weight).is_none());
    session.undo(session.revision()).unwrap();
    assert!(session.document().curve(ids.rational).is_some());
    assert!(session.document().scalar(weight).is_some());
}

#[test]
#[allow(clippy::too_many_lines)]
fn accepted_projection_updates_persistent_conic_shape_and_contact_state() {
    fn fix(document: &mut SketchDocument, point: geosolve_sketch::DesignPointId) {
        let target = document.point(point).unwrap().position;
        document
            .add_constraint(
                format!("fix {point}"),
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }

    let ratio_domain = bounded_parameter(f64::from_bits(1), 1.0);
    let weight_domain = bounded_parameter(MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, f64::MAX);
    let mut document = SketchDocument::new(5.0).unwrap();

    let ellipse_center = document
        .add_point("projection ellipse center", [0.0, 0.0])
        .unwrap();
    let ellipse_axis = document
        .add_point("projection ellipse axis", [2.0, 0.0])
        .unwrap();
    let ellipse_ratio = document
        .add_scalar(
            "projection ellipse ratio",
            0.65,
            ScalarUnit::Parameter,
            ratio_domain,
        )
        .unwrap();
    let ellipse = document
        .add_curve(
            "projection ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_axis,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .unwrap();
    let ellipse_target = document.add_point("ellipse target", [0.0, 1.0]).unwrap();
    for point in [ellipse_center, ellipse_axis, ellipse_target] {
        fix(&mut document, point);
    }
    let ellipse_contact = document
        .add_curve_contact(
            "projection ellipse contact",
            CurveSpan::line(ellipse),
            1.4,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "project ellipse ratio",
            DocumentConstraintDefinition::PointOnCurve {
                point: ellipse_target,
                contact: ellipse_contact,
            },
        )
        .unwrap();

    let desired_start = Point2::new(10.0, 0.0);
    let desired_end = Point2::new(12.0, 0.2);
    let desired_middle = Vector2::new(7.7, 1.4);
    let desired_weight = 0.7;
    let mut oracle = Sketch::new(5.0).unwrap();
    let oracle_start = oracle.add_point(desired_start).unwrap();
    let oracle_end = oracle.add_point(desired_end).unwrap();
    let oracle_rational = oracle
        .add_rational_quadratic(oracle_start, desired_middle, desired_weight, oracle_end)
        .unwrap();
    let rational_start = document
        .add_point("projection rational start", desired_start.coords.into())
        .unwrap();
    let rational_end = document
        .add_point("projection rational end", desired_end.coords.into())
        .unwrap();
    let rational_weight = document
        .add_scalar(
            "projection rational weight",
            0.75,
            ScalarUnit::Parameter,
            weight_domain,
        )
        .unwrap();
    let rational = document
        .add_curve(
            "projection rational",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [7.75, 1.35],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .unwrap();
    for point in [rational_start, rational_end] {
        fix(&mut document, point);
    }
    let mut rational_contacts = Vec::new();
    for (index, (target_parameter, initial_parameter)) in [(0.2, 0.19), (0.5, 0.48), (0.8, 0.81)]
        .into_iter()
        .enumerate()
    {
        let target = oracle
            .evaluate_conic(oracle_rational, target_parameter)
            .unwrap()
            .position;
        let point = document
            .add_point(format!("rational target {index}"), target.coords.into())
            .unwrap();
        fix(&mut document, point);
        let contact = document
            .add_curve_contact(
                format!("rational contact {index}"),
                CurveSpan::line(rational),
                initial_parameter,
                0,
                ContactNeighborhood::Local {
                    lower: target_parameter - 0.15,
                    upper: target_parameter + 0.15,
                },
                None,
            )
            .unwrap();
        document
            .add_constraint(
                format!("project rational target {index}"),
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
            .unwrap();
        rational_contacts.push(contact);
    }

    let desired_hyperbola_center = Point2::new(20.0, 0.0);
    let desired_hyperbola_axis = Point2::new(22.0, 0.8);
    let mut hyperbola_oracle = Sketch::new(5.0).unwrap();
    let oracle_center = hyperbola_oracle
        .add_point(desired_hyperbola_center)
        .unwrap();
    let oracle_axis = hyperbola_oracle.add_point(desired_hyperbola_axis).unwrap();
    let oracle_hyperbola = hyperbola_oracle
        .add_hyperbola_segment(
            oracle_center,
            oracle_axis,
            1.2,
            HyperbolaBranch::Negative,
            DirectedParameterTrim::try_new(0.8, -0.6).unwrap(),
        )
        .unwrap();
    let hyperbola_center = document
        .add_point(
            "projection hyperbola center",
            desired_hyperbola_center.coords.into(),
        )
        .unwrap();
    let hyperbola_axis = document
        .add_point(
            "projection hyperbola axis",
            desired_hyperbola_axis.coords.into(),
        )
        .unwrap();
    let semi_conjugate = document
        .add_scalar(
            "projection semi-conjugate",
            1.3,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let trim_start = document
        .add_scalar(
            "projection hyperbola trim start",
            0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let trim_end = document
        .add_scalar(
            "projection hyperbola trim end",
            -0.6,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola = document
        .add_curve(
            "projection hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    for point in [hyperbola_center, hyperbola_axis] {
        fix(&mut document, point);
    }
    for (index, (target_parameter, initial_parameter)) in
        [(0.3, 0.28), (0.7, 0.72)].into_iter().enumerate()
    {
        let target = hyperbola_oracle
            .evaluate_conic(oracle_hyperbola, target_parameter)
            .unwrap()
            .position;
        let point = document
            .add_point(format!("hyperbola target {index}"), target.coords.into())
            .unwrap();
        fix(&mut document, point);
        let contact = document
            .add_curve_contact(
                format!("hyperbola contact {index}"),
                CurveSpan::line(hyperbola),
                initial_parameter,
                0,
                ContactNeighborhood::Local {
                    lower: target_parameter - 0.15,
                    upper: target_parameter + 0.15,
                },
                None,
            )
            .unwrap();
        document
            .add_constraint(
                format!("project hyperbola target {index}"),
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
            .unwrap();
    }

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.accepted_result().accepted());
    assert!((session.document().scalar(ellipse_ratio).unwrap().value - 0.5).abs() <= 1.0e-8);
    assert!(
        (session.document().scalar(rational_weight).unwrap().value - desired_weight).abs()
            <= 1.0e-7
    );
    let CurveDefinition::RationalQuadraticConic {
        weighted_middle, ..
    } = session.document().curve(rational).unwrap().definition
    else {
        panic!("rational conic expected")
    };
    assert!((weighted_middle[0] - desired_middle.x).abs() <= 1.0e-7);
    assert!((weighted_middle[1] - desired_middle.y).abs() <= 1.0e-7);
    assert!((session.document().scalar(semi_conjugate).unwrap().value - 1.2).abs() <= 1.0e-8);
    assert!(
        (session
            .document()
            .scalar(
                session
                    .document()
                    .contact(ellipse_contact)
                    .unwrap()
                    .parameter
            )
            .unwrap()
            .value
            - PI / 2.0)
            .abs()
            <= 1.0e-8
    );
    assert!(rational_contacts.iter().any(|contact| {
        let slot = session.document().contact(*contact).unwrap();
        let value = session.document().scalar(slot.parameter).unwrap().value;
        ![0.19, 0.48, 0.81]
            .iter()
            .any(|initial| value.to_bits() == f64::to_bits(*initial))
    }));
}

#[test]
fn persistent_invalid_conic_schema_and_geometry_reject_before_lowering() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let axis = document.add_point("axis", [2.0, 0.0]).unwrap();
    let wrong_ratio = document
        .add_scalar(
            "wrong ratio",
            0.5,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    assert!(
        document
            .add_curve(
                "invalid ellipse",
                CurveDefinition::Ellipse {
                    center,
                    major_axis_point: axis,
                    minor_axis_ratio: wrong_ratio,
                },
            )
            .is_err()
    );
    let ratio = document
        .add_scalar(
            "ratio",
            0.5,
            ScalarUnit::Parameter,
            bounded_parameter(f64::from_bits(1), 1.0),
        )
        .unwrap();
    assert!(
        document
            .add_curve(
                "collapsed ellipse",
                CurveDefinition::Ellipse {
                    center,
                    major_axis_point: center,
                    minor_axis_ratio: ratio,
                },
            )
            .is_err()
    );
    let trim = document
        .add_scalar("trim", 1.0, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    assert!(
        document
            .add_curve(
                "equal trim parabola",
                CurveDefinition::ParabolaSegment {
                    vertex: center,
                    focus: axis,
                    trim_start: trim,
                    trim_end: trim,
                },
            )
            .is_err()
    );

    let mut valid = SketchDocument::new(3.0).unwrap();
    let ids = add_persistent_family_set(&mut valid);
    let bytes = valid.to_canonical_json().unwrap();
    let before = valid.clone();
    let CurveDefinition::Ellipse {
        major_axis_point, ..
    } = valid.curve(ids.ellipse).unwrap().definition
    else {
        panic!("ellipse expected")
    };
    assert!(
        valid
            .set_point_position(major_axis_point, [f64::MAX, f64::MAX])
            .is_err()
    );
    assert_eq!(valid, before);
    assert!(valid.set_scalar_value(ids.rational_weight, -1.0).is_err());
    assert_eq!(valid.to_canonical_json().unwrap(), bytes);
    assert!(
        valid
            .set_conic_weighted_middle(ids.rational, [f64::MAX, f64::MAX])
            .is_err()
    );
    assert_eq!(valid.to_canonical_json().unwrap(), bytes);
    assert!(
        SketchDocument::from_json(&bytes.replace("\"unit\":\"parameter\"", "\"unit\":\"length\""))
            .is_err()
    );
}

#[test]
fn public_conic_examples_validate_lower_round_trip_solve_and_render_at_all_scales() {
    for (kind, key, expected_curves) in [
        (AlphaScenarioKind::ConicGallery, "conic-gallery", 5),
        (AlphaScenarioKind::ConicTangency, "conic-tangency", 2),
        (AlphaScenarioKind::ConicCircleLimit, "conic-circle-limit", 2),
    ] {
        assert_eq!(kind.key(), key);
        let mut reference_ids = None;
        let mut reference_document_id = None;
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let fixture = alpha_scenario(kind, scale).unwrap();
            fixture.document.validate().unwrap();
            assert_eq!(fixture.document.curves().len(), expected_curves);
            assert!(fixture.document.curves().iter().all(|curve| matches!(
                curve.definition,
                CurveDefinition::Ellipse { .. }
                    | CurveDefinition::EllipticalArc { .. }
                    | CurveDefinition::RationalQuadraticConic { .. }
                    | CurveDefinition::ParabolaSegment { .. }
                    | CurveDefinition::HyperbolaSegment { .. }
            )));
            if let Some(ids) = &reference_ids {
                assert_eq!(&fixture.ids, ids);
            } else {
                reference_ids = Some(fixture.ids.clone());
            }
            if let Some(id) = reference_document_id {
                assert_eq!(fixture.document.id(), id);
            } else {
                reference_document_id = Some(fixture.document.id());
            }

            let json = fixture.document.to_canonical_json().unwrap();
            let imported = SketchDocument::from_json(&json).unwrap();
            assert_eq!(imported.to_canonical_json().unwrap(), json);
            let first = imported.lower().unwrap();
            let second = imported.lower().unwrap();
            assert_eq!(first.mappings(), second.mappings());
            assert!(imported.curves().iter().all(|curve| matches!(
                first.mappings().runtime_curve(curve.id),
                Some(RuntimeCurve::Conic(_))
            )));

            for curve in imported.curves() {
                let parameters: &[f64] =
                    if matches!(curve.definition, CurveDefinition::Ellipse { .. }) {
                        &[-0.7, 0.4, TAU + 0.9]
                    } else {
                        &[0.0, 0.5, 1.0]
                    };
                for parameter in parameters {
                    let jet = imported
                        .evaluate_curve_jet(CurveSpan::line(curve.id), *parameter)
                        .unwrap();
                    assert!(jet.position.coords.iter().all(|value| value.is_finite()));
                    assert!(
                        jet.first_derivative
                            .iter()
                            .chain(jet.second_derivative.iter())
                            .chain(jet.third_derivative.iter())
                            .all(|value| value.is_finite())
                    );
                }
            }

            let session =
                SketchDocumentSession::new(imported, fixture.request, SolverConfig::default())
                    .unwrap_or_else(|error| panic!("{key}, scale={scale:e}: {error:#?}"));
            let accepted_result = session.accepted_result();
            let accepted = accepted_result.solve();
            assert!(accepted.accepted(), "{key}, scale={scale:e}: {accepted:#?}");
            assert_eq!(accepted.core_report.hard_validity, HardValidity::Valid);
            assert!(accepted.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
            assert!(accepted.core_report.audit.sources.iter().all(|source| {
                source.rows.iter().all(|row| {
                    row.evaluation_status == AuditEvaluationStatus::Evaluated
                        && row.raw_residual.is_finite()
                        && row.normalized_residual.is_finite()
                })
            }));
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn conic_gallery_and_tangency_examples_retain_scalar_and_contact_semantics() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let gallery = alpha_scenario(AlphaScenarioKind::ConicGallery, scale).unwrap();
        let AlphaScenarioIds::ConicGallery(ids) = gallery.ids else {
            panic!("conic gallery IDs expected");
        };
        let [ellipse, arc, rational, parabola, hyperbola] = ids.curves;
        assert!(matches!(
            gallery.document.curve(ellipse).unwrap().definition,
            CurveDefinition::Ellipse { minor_axis_ratio, .. }
                if gallery.document.scalar(minor_axis_ratio).unwrap().unit == ScalarUnit::Parameter
                    && gallery.document.scalar(minor_axis_ratio).unwrap().domain
                        == bounded_parameter(f64::from_bits(1), 1.0)
        ));
        assert!(matches!(
            gallery.document.curve(arc).unwrap().definition,
            CurveDefinition::EllipticalArc {
                start_angle,
                end_angle,
                sweep: DocumentArcSweep::Clockwise,
                ..
            } if gallery.document.scalar(start_angle).unwrap().value
                > gallery.document.scalar(end_angle).unwrap().value
        ));
        assert!(matches!(
            gallery.document.curve(rational).unwrap().definition,
            CurveDefinition::RationalQuadraticConic {
                weighted_middle,
                middle_weight,
                ..
            } if (weighted_middle[0] / scale - 5.2).abs() <= 1.0e-12
                && (weighted_middle[1] / scale - 3.25).abs() <= 1.0e-12
                && gallery.document.scalar(middle_weight).unwrap().unit == ScalarUnit::Parameter
                && gallery.document.scalar(middle_weight).unwrap().domain
                    == bounded_parameter(MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, f64::MAX)
        ));
        assert!(matches!(
            gallery.document.curve(parabola).unwrap().definition,
            CurveDefinition::ParabolaSegment {
                trim_start,
                trim_end,
                ..
            } if gallery.document.scalar(trim_start).unwrap().value
                > gallery.document.scalar(trim_end).unwrap().value
        ));
        assert!(matches!(
            gallery.document.curve(hyperbola).unwrap().definition,
            CurveDefinition::HyperbolaSegment {
                semi_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start,
                trim_end,
                ..
            } if (gallery.document.scalar(semi_conjugate).unwrap().value / scale - 1.4).abs()
                    <= 1.0e-12
                && gallery.document.scalar(semi_conjugate).unwrap().unit == ScalarUnit::Length
                && gallery.document.scalar(semi_conjugate).unwrap().domain
                    == ScalarDomain::Positive
                && gallery.document.scalar(trim_start).unwrap().value
                    > gallery.document.scalar(trim_end).unwrap().value
        ));

        let tangency = alpha_scenario(AlphaScenarioKind::ConicTangency, scale).unwrap();
        let AlphaScenarioIds::ConicTangency(ids) = tangency.ids else {
            panic!("conic tangency IDs expected");
        };
        assert!(ids.tangency_contacts.iter().all(|contact| {
            let contact = tangency.document.contact(*contact).unwrap();
            contact.neighborhood == ContactNeighborhood::Interior
                && contact.tangent_orientation == Some(TangentOrientation::Opposed)
        }));
        assert!(ids.point_contacts.iter().all(|contact| {
            let contact = tangency.document.contact(*contact).unwrap();
            contact.neighborhood == ContactNeighborhood::Interior
                && contact.tangent_orientation.is_none()
        }));
        assert!(matches!(
            tangency.document.constraint(ids.tangency).unwrap().definition,
            DocumentConstraintDefinition::CurveCurveTangency { first_contact, second_contact }
                if [first_contact, second_contact] == ids.tangency_contacts
        ));
        let first = tangency
            .document
            .evaluate_contact_jet(ids.tangency_contacts[0])
            .unwrap();
        let second = tangency
            .document
            .evaluate_contact_jet(ids.tangency_contacts[1])
            .unwrap();
        assert!((first.position - second.position).norm() / scale <= 1.0e-12);
        assert!(first.first_derivative.dot(&second.first_derivative) < 0.0);

        let session = SketchDocumentSession::new(
            tangency.document,
            tangency.request,
            SolverConfig::default(),
        )
        .unwrap();
        let accepted_result = session.accepted_result();
        let audit = accepted_result
            .solve()
            .core_report
            .audit
            .sources
            .iter()
            .find(|source| {
                source.rows.iter().any(|row| {
                    row.template == "cross(unit(first_curve'(t1)), unit(second_curve'(t2)))"
                })
            })
            .unwrap();
        assert_eq!(audit.rows.len(), 3);
        assert_eq!(
            audit.rows[2].template,
            "cross(unit(first_curve'(t1)), unit(second_curve'(t2)))"
        );
    }
}

#[test]
fn conic_circle_limit_example_exposes_unobservable_full_axis_and_directed_arc_rank() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = alpha_scenario(AlphaScenarioKind::ConicCircleLimit, scale).unwrap();
        let AlphaScenarioIds::ConicCircleLimit(ids) = fixture.ids else {
            panic!("circle-limit IDs expected");
        };
        let [ellipse, arc] = ids.curves;
        assert_eq!(
            fixture.document.conic_axis_observability(ellipse).unwrap(),
            EllipseAxisObservability::UnobservableCircleLimit
        );
        assert_eq!(
            fixture.document.conic_axis_observability(arc).unwrap(),
            EllipseAxisObservability::ObservableByDirectedTrim
        );
        assert_eq!(
            fixture
                .document
                .contact(ids.full_ellipse_contacts[0])
                .unwrap()
                .winding,
            1
        );
        assert_eq!(
            ids.arc_endpoint_contacts.map(|contact| fixture
                .document
                .contact(contact)
                .unwrap()
                .neighborhood),
            [ContactNeighborhood::Start, ContactNeighborhood::End]
        );
        assert!(matches!(
            fixture.document.curve(arc).unwrap().definition,
            CurveDefinition::EllipticalArc {
                sweep: DocumentArcSweep::CounterClockwise,
                ..
            }
        ));
        let session =
            SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
                .unwrap();
        let accepted_result = session.accepted_result();
        let report = &accepted_result.solve().core_report;
        assert_eq!(report.rank, 8, "scale={scale:e}: {report:#?}");
        assert_eq!(report.right_nullity, 2, "scale={scale:e}: {report:#?}");
        assert_eq!(
            report.bidirectional_degrees_of_freedom, 1,
            "scale={scale:e}: {report:#?}"
        );
    }
}

fn trim_endpoint_position(
    document: &SketchDocument,
    curve: CurveId,
    endpoint: FeatureEndpoint,
) -> [f64; 2] {
    let parameter = match endpoint {
        FeatureEndpoint::Start => 0.0,
        FeatureEndpoint::End => 1.0,
    };
    let point = document
        .evaluate_curve_jet(CurveSpan::line(curve), parameter)
        .unwrap()
        .position;
    [point.x, point.y]
}

fn assert_trim_projection_applies(
    document: &SketchDocument,
    curve: CurveId,
    endpoint: FeatureEndpoint,
    scalar: DesignScalarId,
    desired: f64,
    scale: f64,
) {
    let mut oracle = document.clone();
    oracle.set_scalar_value(scalar, desired).unwrap();
    let target = trim_endpoint_position(&oracle, curve, endpoint);
    let before = document.to_canonical_json().unwrap();

    let projection = document
        .project_curve_trim_endpoint(curve, endpoint, target)
        .unwrap();
    assert_eq!(projection.scalar, scalar);
    assert!(
        (projection.value - desired).abs() <= 2.0e-11 * desired.abs().max(1.0),
        "desired={desired}, projected={}",
        projection.value
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);

    let mut applied = document.clone();
    applied
        .set_scalar_value(projection.scalar, projection.value)
        .unwrap();
    let applied_target = trim_endpoint_position(&applied, curve, endpoint);
    let error = (applied_target[0] - target[0]).hypot(applied_target[1] - target[1]);
    assert!(error <= 2.0e-10 * scale, "scale={scale:e}, error={error:e}");
}

fn arc_trim_projection_fixture(
    scale: f64,
    elliptical: bool,
    sweep: DocumentArcSweep,
) -> (SketchDocument, CurveId, [DesignScalarId; 2]) {
    let mut document = SketchDocument::new(scale).unwrap();
    let center_position = [1.3 * scale, -0.7 * scale];
    let center = document.add_point("arc center", center_position).unwrap();
    let start = document
        .add_scalar(
            "arc start",
            2.0 * TAU + 0.35,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end = document
        .add_scalar(
            "arc end",
            -2.0 * TAU + 1.45,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let definition = if elliptical {
        let axis_angle: f64 = 0.63;
        let axis = document
            .add_point(
                "ellipse major axis",
                [
                    center_position[0] + 3.2 * scale * axis_angle.cos(),
                    center_position[1] + 3.2 * scale * axis_angle.sin(),
                ],
            )
            .unwrap();
        let ratio = document
            .add_scalar(
                "ellipse ratio",
                0.37,
                ScalarUnit::Parameter,
                bounded_parameter(f64::from_bits(1), 1.0),
            )
            .unwrap();
        CurveDefinition::EllipticalArc {
            center,
            major_axis_point: axis,
            minor_axis_ratio: ratio,
            start_angle: start,
            end_angle: end,
            sweep,
        }
    } else {
        let radius = document
            .add_scalar(
                "arc radius",
                2.3 * scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle: start,
            end_angle: end,
            sweep,
        }
    };
    let curve = document.add_curve("trim arc", definition).unwrap();
    (document, curve, [start, end])
}

#[test]
fn arc_trim_projection_unwraps_both_endpoints_for_both_sweeps_and_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for elliptical in [false, true] {
            for sweep in [
                DocumentArcSweep::CounterClockwise,
                DocumentArcSweep::Clockwise,
            ] {
                for (endpoint, index, desired) in [
                    (FeatureEndpoint::Start, 0, 2.0 * TAU - 0.25),
                    (FeatureEndpoint::End, 1, -2.0 * TAU + 1.85),
                ] {
                    let (document, curve, scalars) =
                        arc_trim_projection_fixture(scale, elliptical, sweep);
                    assert_trim_projection_applies(
                        &document,
                        curve,
                        endpoint,
                        scalars[index],
                        desired,
                        scale,
                    );
                }

                let (mut document, curve, _) =
                    arc_trim_projection_fixture(scale, elliptical, sweep);
                let replacement = match sweep {
                    DocumentArcSweep::CounterClockwise => DocumentArcSweep::Clockwise,
                    DocumentArcSweep::Clockwise => DocumentArcSweep::CounterClockwise,
                };
                document.set_arc_sweep(curve, replacement).unwrap();
                assert!(matches!(
                    document.curve(curve).unwrap().definition,
                    CurveDefinition::CircularArc { sweep, .. }
                        | CurveDefinition::EllipticalArc { sweep, .. }
                        if sweep == replacement
                ));
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_conic_trim_projection_preserves_reversed_trims_and_hyperbola_branches() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for (endpoint, index, desired) in [
            (FeatureEndpoint::Start, 0, 0.65),
            (FeatureEndpoint::End, 1, -1.2),
        ] {
            let mut document = SketchDocument::new(scale).unwrap();
            let vertex_position = [-2.0 * scale, 1.5 * scale];
            let vertex = document
                .add_point("parabola vertex", vertex_position)
                .unwrap();
            let axis_angle: f64 = -0.42;
            let focus = document
                .add_point(
                    "parabola focus",
                    [
                        vertex_position[0] + 1.7 * scale * axis_angle.cos(),
                        vertex_position[1] + 1.7 * scale * axis_angle.sin(),
                    ],
                )
                .unwrap();
            let start = document
                .add_scalar(
                    "parabola start",
                    1.3,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )
                .unwrap();
            let end = document
                .add_scalar(
                    "parabola end",
                    -0.9,
                    ScalarUnit::Parameter,
                    ScalarDomain::Finite,
                )
                .unwrap();
            let curve = document
                .add_curve(
                    "reversed parabola",
                    CurveDefinition::ParabolaSegment {
                        vertex,
                        focus,
                        trim_start: start,
                        trim_end: end,
                    },
                )
                .unwrap();
            assert_trim_projection_applies(
                &document,
                curve,
                endpoint,
                [start, end][index],
                desired,
                scale,
            );
        }

        for branch in [
            DocumentHyperbolaBranch::Positive,
            DocumentHyperbolaBranch::Negative,
        ] {
            for (endpoint, index, desired) in [
                (FeatureEndpoint::Start, 0, 0.45),
                (FeatureEndpoint::End, 1, -1.15),
            ] {
                let mut document = SketchDocument::new(scale).unwrap();
                let center_position = [3.0 * scale, -4.0 * scale];
                let center = document
                    .add_point("hyperbola center", center_position)
                    .unwrap();
                let axis_angle: f64 = 0.78;
                let axis = document
                    .add_point(
                        "hyperbola axis",
                        [
                            center_position[0] + 2.4 * scale * axis_angle.cos(),
                            center_position[1] + 2.4 * scale * axis_angle.sin(),
                        ],
                    )
                    .unwrap();
                let semi_conjugate = document
                    .add_scalar(
                        "hyperbola semi-conjugate",
                        1.1 * scale,
                        ScalarUnit::Length,
                        ScalarDomain::Positive,
                    )
                    .unwrap();
                let start = document
                    .add_scalar(
                        "hyperbola start",
                        1.0,
                        ScalarUnit::Parameter,
                        ScalarDomain::Finite,
                    )
                    .unwrap();
                let end = document
                    .add_scalar(
                        "hyperbola end",
                        -0.7,
                        ScalarUnit::Parameter,
                        ScalarDomain::Finite,
                    )
                    .unwrap();
                let curve = document
                    .add_curve(
                        "reversed hyperbola",
                        CurveDefinition::HyperbolaSegment {
                            center,
                            transverse_axis_point: axis,
                            semi_conjugate,
                            branch,
                            trim_start: start,
                            trim_end: end,
                        },
                    )
                    .unwrap();
                assert_trim_projection_applies(
                    &document,
                    curve,
                    endpoint,
                    [start, end][index],
                    desired,
                    scale,
                );
                assert!(matches!(
                    document.curve(curve).unwrap().definition,
                    CurveDefinition::HyperbolaSegment { branch: retained, .. }
                        if retained == branch
                ));
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn trim_projection_reports_typed_failures_and_leaves_degenerate_edits_transactional() {
    let (circular, circular_curve, _) =
        arc_trim_projection_fixture(1.0, false, DocumentArcSweep::CounterClockwise);
    assert!(matches!(
        circular.project_curve_trim_endpoint(
            circular_curve,
            FeatureEndpoint::Start,
            [f64::NAN, 0.0]
        ),
        Err(DocumentTrimProjectionError::NonFiniteTarget { curve })
            if curve == circular_curve
    ));
    let CurveDefinition::CircularArc { center, .. } =
        circular.curve(circular_curve).unwrap().definition
    else {
        panic!("circular arc expected")
    };
    assert!(matches!(
        circular.project_curve_trim_endpoint(
            circular_curve,
            FeatureEndpoint::End,
            circular.point(center).unwrap().position,
        ),
        Err(DocumentTrimProjectionError::AmbiguousCenterTarget { curve })
            if curve == circular_curve
    ));

    let (elliptical, elliptical_curve, _) =
        arc_trim_projection_fixture(1.0, true, DocumentArcSweep::Clockwise);
    let CurveDefinition::EllipticalArc { center, .. } =
        elliptical.curve(elliptical_curve).unwrap().definition
    else {
        panic!("elliptical arc expected")
    };
    assert!(matches!(
        elliptical.project_curve_trim_endpoint(
            elliptical_curve,
            FeatureEndpoint::Start,
            elliptical.point(center).unwrap().position,
        ),
        Err(DocumentTrimProjectionError::AmbiguousCenterTarget { curve })
            if curve == elliptical_curve
    ));
    assert!(matches!(
        elliptical.project_curve_trim_endpoint(
            elliptical_curve,
            FeatureEndpoint::End,
            [0.0, f64::INFINITY],
        ),
        Err(DocumentTrimProjectionError::NonFiniteTarget { curve })
            if curve == elliptical_curve
    ));

    let mut unsupported = SketchDocument::new(1.0).unwrap();
    let first = unsupported.add_point("line start", [0.0, 0.0]).unwrap();
    let second = unsupported.add_point("line end", [1.0, 0.0]).unwrap();
    let line = unsupported
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let weight = unsupported
        .add_scalar(
            "rational weight",
            0.5,
            ScalarUnit::Parameter,
            bounded_parameter(MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, f64::MAX),
        )
        .unwrap();
    let rational = unsupported
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start: first,
                weighted_middle: [0.25, 0.5],
                middle_weight: weight,
                end: second,
            },
        )
        .unwrap();
    for curve in [line, rational] {
        assert!(matches!(
            unsupported.project_curve_trim_endpoint(
                curve,
                FeatureEndpoint::Start,
                [0.5, 0.5],
            ),
            Err(DocumentTrimProjectionError::UnsupportedCurve { curve: rejected })
                if rejected == curve
        ));
    }
    assert!(
        unsupported
            .set_arc_sweep(line, DocumentArcSweep::Clockwise)
            .is_err()
    );

    let mut overflow = SketchDocument::new(1.0).unwrap();
    let center = overflow
        .add_point("overflow center", [-f64::MAX, 0.0])
        .unwrap();
    let radius = overflow
        .add_scalar(
            "overflow radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let start = overflow
        .add_scalar(
            "overflow start",
            0.0,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end = overflow
        .add_scalar("overflow end", 1.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let overflow_curve = overflow
        .add_curve(
            "overflow arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    assert!(matches!(
        overflow.project_curve_trim_endpoint(
            overflow_curve,
            FeatureEndpoint::Start,
            [f64::MAX, 0.0],
        ),
        Err(DocumentTrimProjectionError::NonFiniteResult { curve })
            if curve == overflow_curve
    ));

    let mut reversed = SketchDocument::new(1.0).unwrap();
    let vertex = reversed.add_point("vertex", [0.0, 0.0]).unwrap();
    let focus = reversed.add_point("focus", [1.0, 0.0]).unwrap();
    let start = reversed
        .add_scalar(
            "trim start",
            1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end = reversed
        .add_scalar(
            "trim end",
            -1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola = reversed
        .add_curve(
            "reversed parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start: start,
                trim_end: end,
            },
        )
        .unwrap();
    let target = trim_endpoint_position(&reversed, parabola, FeatureEndpoint::End);
    let projection = reversed
        .project_curve_trim_endpoint(parabola, FeatureEndpoint::Start, target)
        .unwrap();
    assert_eq!(projection.scalar, start);
    assert!((projection.value - reversed.scalar(end).unwrap().value).abs() <= 1.0e-12);
    let before = reversed.to_canonical_json().unwrap();
    assert!(
        reversed
            .set_scalar_value(projection.scalar, projection.value)
            .is_err()
    );
    assert_eq!(reversed.to_canonical_json().unwrap(), before);
}
