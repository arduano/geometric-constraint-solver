use std::f64::consts::{FRAC_PI_2, PI, TAU};

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_geometry::{Point2, cubic_bezier_jet};
use geosolve_sketch::{
    ArcSweep, ContactDefinition, ContactDomain, ContactNeighborhood, ContactState, CoordinateAxis,
    CurveContactNeighborhood, CurveDefinition, CurveSpan, CurveTangentOrientation, DimensionMode,
    DocumentCommand, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentEdit, DocumentSolveRequest, FeatureEndpoint,
    LineParameterDomain, ScalarDomain, ScalarUnit, SegmentEndpoint, Sketch, SketchCurve,
    SketchCurveContact, SketchDocument, SketchDocumentSession, SketchPatch, SketchSession,
    SketchSessionPatch, SketchSolveRequest, SketchSource, TangentOrientation,
};

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let direction = [second[0] - first[0], second[1] - first[1]];
    let norm = direction[0].hypot(direction[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [direction[0] / norm, direction[1] / norm],
            },
        )
        .unwrap()
}

#[test]
#[allow(clippy::float_cmp, clippy::too_many_lines)]
fn a5_cubic_tangent_edit_solves_and_zero_speed_rolls_back() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(3.0 * scale).unwrap();
        let p0 = document.add_point("P0", [0.0, 0.0]).unwrap();
        let p1 = document.add_point("P1", [scale, 0.0]).unwrap();
        let p2 = document.add_point("P2", [2.0 * scale, scale]).unwrap();
        let p3 = document.add_point("P3", [3.0 * scale, scale]).unwrap();
        let a = document.add_point("A", [0.0, 0.0]).unwrap();
        let b = document.add_point("B", [2.0 * scale, 0.0]).unwrap();
        let line = add_line(&mut document, "AB", a, b);
        let bezier = document
            .add_curve(
                "Bezier",
                CurveDefinition::CubicBezier {
                    controls: [p0, p1, p2, p3],
                },
            )
            .unwrap();
        document
            .add_constraint(
                "A fixed",
                DocumentConstraintDefinition::FixedPoint {
                    point: a,
                    target: [0.0, 0.0],
                },
            )
            .unwrap();
        let length = document
            .add_scalar(
                "AB length",
                2.0 * scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        document
            .add_dimension(
                "AB length dimension",
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(line),
                    target: length,
                },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
        let parameter = document
            .add_scalar(
                "Bezier contact parameter",
                0.0,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )
            .unwrap();
        let contact = document
            .add_contact(
                "Bezier start contact",
                ContactDefinition {
                    curve: CurveSpan::line(bezier),
                    parameter,
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    winding: 0,
                    neighborhood: ContactNeighborhood::Start,
                    tangent_orientation: Some(TangentOrientation::Aligned),
                },
            )
            .unwrap();
        let tangency = document
            .add_constraint(
                "AB tangent at Bezier start",
                DocumentConstraintDefinition::LineCurveTangency {
                    line: CurveSpan::line(line),
                    endpoint: FeatureEndpoint::Start,
                    curve_contact: contact,
                },
            )
            .unwrap();
        let initial_json = document.to_canonical_json().unwrap();
        assert_eq!(
            SketchDocument::from_json(&initial_json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            initial_json
        );
        let mut session = SketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let edited = session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetPointPosition {
                    point: p1,
                    position: [scale, 0.5 * scale],
                },
            ))
            .unwrap();
        assert!(
            edited.accepted(),
            "{:#?}",
            edited.result.solve().core_report
        );
        let expected = [4.0 * scale / 5.0f64.sqrt(), 2.0 * scale / 5.0f64.sqrt()];
        let solved_b = session.document().point(b).unwrap().position;
        let tolerance = 1.0e-9 * scale.max(1.0);
        assert!(
            (solved_b[0] - expected[0]).abs() <= tolerance,
            "scale={scale:e}: {solved_b:?}"
        );
        assert!(
            (solved_b[1] - expected[1]).abs() <= tolerance,
            "scale={scale:e}: {solved_b:?}"
        );
        assert_eq!(session.document().point(a).unwrap().position, [0.0, 0.0]);
        let solved_p0 = session.document().point(p0).unwrap().position;
        assert!(solved_p0[0].abs() <= 1.0e-12, "{solved_p0:?}");
        assert!(solved_p0[1].abs() <= 1.0e-12, "{solved_p0:?}");
        let solved_p1 = session.document().point(p1).unwrap().position;
        assert!((solved_p1[0] - scale).abs() <= tolerance, "{solved_p1:?}");
        assert!(
            (solved_p1[1] - 0.5 * scale).abs() <= tolerance,
            "{solved_p1:?}"
        );
        assert!(edited.result.solve().core_report.hard_residual_max <= 1.0e-9);
        let report = &edited.result.solve().core_report;
        assert!(report.rank_is_valid);
        assert_eq!(
            (report.rank, report.left_nullity, report.right_nullity),
            (4, 0, 7)
        );
        assert_eq!(report.bidirectional_degrees_of_freedom, 6);
        assert_eq!(
            report.one_sided_mobility,
            geosolve_core::OneSidedMobility::Exists
        );
        assert_eq!(report.bounds.len(), 1);
        assert_eq!(report.bounds[0].status, geosolve_core::BoundStatus::Fixed);

        let source = session.document().constraint(tangency).unwrap().source_id;
        let runtime_source = session.mappings().runtime_source(source).unwrap();
        let geosolve_sketch::RuntimeSource::Constraint(runtime_constraint) = runtime_source else {
            panic!("constraint mapping expected");
        };
        let compiled = session
            .runtime()
            .sketch()
            .compile(SketchSolveRequest::default())
            .unwrap();
        let source_mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(runtime_constraint))
            .unwrap();
        let residual = compiled
            .problem()
            .residual(source_mapping.residual_ids[0])
            .unwrap();
        assert_eq!(residual.incident_variables().len(), 7);

        let accepted_json = session.export_json().unwrap();
        let accepted_revision = session.revision();
        let accepted_history = session.history_len();
        let coincident_control = session.document().point(p0).unwrap().position;
        let rejected = session.apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: p1,
                position: coincident_control,
            },
        ));
        assert!(rejected.is_err());
        assert_eq!(session.export_json().unwrap(), accepted_json);
        assert_eq!(session.revision(), accepted_revision);
        assert_eq!(session.history_len(), accepted_history);

        session.undo(session.revision()).unwrap();
        assert_eq!(session.document().point(p1).unwrap().position, [scale, 0.0]);
        session.redo(session.revision()).unwrap();
        assert_eq!(session.export_json().unwrap(), accepted_json);
    }
}

#[test]
fn bezier_local_ad_includes_all_controls_and_matches_interior_finite_differences() {
    let mut sketch = Sketch::new(3.0).unwrap();
    let controls = [
        sketch.add_point(Point2::new(0.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(1.0, 0.2)).unwrap(),
        sketch.add_point(Point2::new(2.0, 1.0)).unwrap(),
        sketch.add_point(Point2::new(3.0, 1.0)).unwrap(),
    ];
    let parameter = 0.37;
    let control_values = [
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.2),
        Point2::new(2.0, 1.0),
        Point2::new(3.0, 1.0),
    ];
    let jet = cubic_bezier_jet(control_values, parameter).unwrap();
    let point = sketch.add_point(jet.position).unwrap();
    let bezier = sketch.add_cubic_bezier("cubic", controls).unwrap();
    let point_constraint = sketch
        .add_point_on_bezier(point, bezier, parameter)
        .unwrap();

    let tangent = jet.first_derivative.normalize() * 2.0;
    let line_start = sketch.add_point(jet.position).unwrap();
    let line_end = sketch.add_point(jet.position + tangent).unwrap();
    let line = sketch.add_segment(line_start, line_end).unwrap();
    let tangent_constraint = sketch
        .add_line_bezier_tangency(
            line,
            SegmentEndpoint::Start,
            bezier,
            parameter,
            CurveTangentOrientation::Aligned,
        )
        .unwrap();
    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let check = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(check.all_within(1.0e-6), "{check:#?}");
    for constraint in [point_constraint, tangent_constraint] {
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(constraint))
            .unwrap();
        let residual = compiled
            .problem()
            .residual(mapping.residual_ids[0])
            .unwrap();
        let expected = if constraint == point_constraint { 6 } else { 7 };
        assert_eq!(residual.incident_variables().len(), expected);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_alpha_curve_family_recovers_through_common_contact_and_tangency_ad_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let origin = sketch.add_point(Point2::origin()).unwrap();

        let line_start = sketch.add_point(Point2::new(-scale, 0.0)).unwrap();
        let line_end = sketch.add_point(Point2::new(scale, 0.0)).unwrap();
        let line = sketch.add_segment(line_start, line_end).unwrap();
        let polyline_start = sketch.add_point(Point2::new(-2.0 * scale, 0.0)).unwrap();
        let polyline_end = sketch.add_point(Point2::new(2.0 * scale, 0.0)).unwrap();
        let polyline_segment = sketch
            .add_named_segment("polyline segment", polyline_start, polyline_end)
            .unwrap();

        let radial_center = sketch.add_point(Point2::new(0.0, scale)).unwrap();
        let circle = sketch.add_circle(radial_center, scale).unwrap();
        let arc = sketch
            .add_arc(radial_center, scale, -PI, 0.0, ArcSweep::CounterClockwise)
            .unwrap();

        let quadratic_controls = [
            sketch.add_point(Point2::new(-scale, 0.0)).unwrap(),
            sketch.add_point(Point2::origin()).unwrap(),
            sketch.add_point(Point2::new(scale, 0.0)).unwrap(),
        ];
        let quadratic = sketch
            .add_quadratic_bezier("quadratic", quadratic_controls)
            .unwrap();
        let cubic_controls = [
            sketch.add_point(Point2::new(-1.5 * scale, 0.0)).unwrap(),
            sketch.add_point(Point2::new(-0.5 * scale, 0.0)).unwrap(),
            sketch.add_point(Point2::new(0.5 * scale, 0.0)).unwrap(),
            sketch.add_point(Point2::new(1.5 * scale, 0.0)).unwrap(),
        ];
        let cubic = sketch.add_cubic_bezier("cubic", cubic_controls).unwrap();

        let contacts = [
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: line,
                    domain: LineParameterDomain::BoundedSegment,
                },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: polyline_segment,
                    domain: LineParameterDomain::BoundedSegment,
                },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
            SketchCurveContact {
                curve: SketchCurve::Circle(circle),
                parameter: -FRAC_PI_2,
                neighborhood: CurveContactNeighborhood::Interior,
            },
            SketchCurveContact {
                curve: SketchCurve::Arc(arc),
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
            SketchCurveContact {
                curve: SketchCurve::Bezier(quadratic),
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
            SketchCurveContact {
                curve: SketchCurve::Bezier(cubic),
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
        ];

        let perturbed_parameters = [0.49, 0.51, -FRAC_PI_2 + 0.01, 0.49, 0.49, 0.51];
        let mut point_constraints = Vec::new();
        for contact in contacts {
            point_constraints.push(sketch.add_point_on_curve(origin, contact).unwrap());
        }
        let mut pair_constraints = Vec::new();
        for first in 0..contacts.len() {
            for second in first + 1..contacts.len() {
                let contact = sketch
                    .add_curve_curve_contact(contacts[first], contacts[second])
                    .unwrap();
                let tangency = sketch
                    .add_curve_curve_tangency(
                        contacts[first],
                        contacts[second],
                        CurveTangentOrientation::Aligned,
                    )
                    .unwrap();
                pair_constraints.push((contact, tangency, first, second));
            }
        }

        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let check = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(check.all_within(1.0e-6), "scale={scale:e}: {check:#?}");
        for (constraint, parameter) in point_constraints.into_iter().zip(perturbed_parameters) {
            sketch
                .set_contact_state(constraint, ContactState::PointOnCurve { parameter })
                .unwrap();
        }
        for (contact, tangency, first, second) in pair_constraints {
            let state = ContactState::CurveCurveContact {
                first_parameter: perturbed_parameters[first],
                second_parameter: perturbed_parameters[second],
            };
            sketch.set_contact_state(contact, state).unwrap();
            sketch
                .set_contact_state(
                    tangency,
                    ContactState::CurveCurveTangency {
                        first_parameter: perturbed_parameters[first],
                        second_parameter: perturbed_parameters[second],
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
        assert!(solved.accepted(), "scale={scale:e}: {solved:#?}");
        assert_eq!(solved.core_report.hard_validity, HardValidity::Valid);
        assert!(solved.core_report.hard_residual_max <= 1.0e-9);
        assert!(solved.core_report.rank_is_valid);
        assert_eq!(
            (
                solved.core_report.rank,
                solved.core_report.left_nullity,
                solved.core_report.right_nullity,
            ),
            (70, 17, 24)
        );
        assert_eq!(solved.core_report.bidirectional_degrees_of_freedom, 24);
        assert_eq!(
            solved.core_report.one_sided_mobility,
            geosolve_core::OneSidedMobility::Exists
        );
        assert_eq!(solved.core_report.bounds.len(), 57);
        assert!(
            solved
                .core_report
                .bounds
                .iter()
                .all(|bound| bound.status == geosolve_core::BoundStatus::Inactive)
        );
        assert!(
            solved
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
#[allow(clippy::too_many_lines)]
fn generic_curve_pair_document_round_trips_ids_contacts_and_accepted_audit() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let center = document.add_point("circle center", [0.0, 1.0]).unwrap();
    let radius = document
        .add_scalar(
            "circle radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let controls = [
        document.add_point("B0", [-1.5, 0.0]).unwrap(),
        document.add_point("B1", [-0.5, 0.0]).unwrap(),
        document.add_point("B2", [0.5, 0.0]).unwrap(),
        document.add_point("B3", [1.5, 0.0]).unwrap(),
    ];
    let bezier = document
        .add_curve("cubic", CurveDefinition::CubicBezier { controls })
        .unwrap();
    let circle_parameter = document
        .add_scalar(
            "circle contact angle",
            3.0 * FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Periodic { period: TAU },
        )
        .unwrap();
    let circle_contact = document
        .add_contact(
            "circle contact",
            ContactDefinition {
                curve: CurveSpan::line(circle),
                parameter: circle_parameter,
                domain: ContactDomain::Periodic { period: TAU },
                winding: -1,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
        )
        .unwrap();
    let bezier_parameter = document
        .add_scalar(
            "Bezier contact parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let bezier_contact = document
        .add_contact(
            "Bezier contact",
            ContactDefinition {
                curve: CurveSpan::line(bezier),
                parameter: bezier_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
        )
        .unwrap();
    let tangency = document
        .add_constraint(
            "circle-cubic tangency",
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact: circle_contact,
                second_contact: bezier_contact,
            },
        )
        .unwrap();

    let circle_jet = document.evaluate_contact_jet(circle_contact).unwrap();
    let bezier_jet = document.evaluate_contact_jet(bezier_contact).unwrap();
    let queried_bezier = document
        .evaluate_curve_jet(CurveSpan::line(bezier), 0.5)
        .unwrap();
    assert!((circle_jet.position - bezier_jet.position).norm() <= 1.0e-12);
    assert!((queried_bezier.position - bezier_jet.position).norm() <= 1.0e-12);
    assert!(
        circle_jet
            .first_derivative
            .dot(&bezier_jet.first_derivative)
            > 0.0
    );

    let json = document.to_canonical_json().unwrap();
    let reloaded = SketchDocument::from_json(&json).unwrap();
    assert_eq!(reloaded.to_canonical_json().unwrap(), json);
    let session = SketchDocumentSession::new(
        reloaded,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = session.accepted_result();
    assert!(result.accepted());
    assert_eq!(
        result.solve().core_report.hard_validity,
        HardValidity::Valid
    );
    assert!(result.solve().core_report.hard_residual_max <= 1.0e-9);
    let source = session.document().constraint(tangency).unwrap().source_id;
    let runtime = session.mappings().runtime_source(source).unwrap();
    let geosolve_sketch::RuntimeSource::Constraint(runtime) = runtime else {
        panic!("constraint mapping expected");
    };
    assert_eq!(
        session
            .mappings()
            .contact_mappings()
            .iter()
            .filter(|mapping| mapping.constraint == runtime)
            .count(),
        2
    );
    let audit = session
        .runtime()
        .audit_source(SketchSource::Constraint(runtime))
        .unwrap();
    assert_eq!(audit.rows.len(), 3);
    assert!(audit.rows.iter().all(|row| {
        row.evaluation_status == AuditEvaluationStatus::Evaluated
            && row.raw_residual.is_finite()
            && row.normalized_residual.is_finite()
    }));
}

#[test]
fn explicit_local_neighborhood_retains_one_of_two_bezier_contact_roots() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let point = sketch.add_point(Point2::origin()).unwrap();
    sketch.add_fixed_point(point).unwrap();
    let controls = [
        sketch.add_point(Point2::new(0.1875, 0.0)).unwrap(),
        sketch.add_point(Point2::new(-0.3125, 0.0)).unwrap(),
        sketch.add_point(Point2::new(0.1875, 0.0)).unwrap(),
    ];
    for control in controls {
        sketch.add_fixed_point(control).unwrap();
    }
    let curve = sketch
        .add_quadratic_bezier("two-root quadratic", controls)
        .unwrap();
    let constraint = sketch
        .add_point_on_curve(
            point,
            SketchCurveContact {
                curve: SketchCurve::Bezier(curve),
                parameter: 0.35,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.1,
                    upper: 0.4,
                },
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
    let geosolve_sketch::ContactState::PointOnCurve { parameter } =
        sketch.contact_state(constraint).unwrap()
    else {
        panic!("generic point contact state expected");
    };
    assert!((parameter - 0.25).abs() <= 1.0e-9, "{parameter}");
    let bound = solved
        .bound_mappings
        .iter()
        .find(|mapping| {
            mapping.bound
                == geosolve_sketch::SketchBound::Contact {
                    constraint_id: constraint,
                    role: geosolve_sketch::LatentVariableRole::CurveParameter,
                }
        })
        .and_then(|mapping| {
            solved
                .core_report
                .bounds
                .iter()
                .find(|bound| bound.bound_id == mapping.bound_id)
        })
        .unwrap();
    assert_eq!(bound.lower, Some(0.1));
    assert_eq!(bound.upper, Some(0.4));
    assert!(
        sketch
            .set_contact_state(
                constraint,
                geosolve_sketch::ContactState::PointOnCurve { parameter: 0.75 },
            )
            .is_err()
    );
    assert_eq!(sketch.contact_state(constraint).unwrap(), {
        geosolve_sketch::ContactState::PointOnCurve { parameter }
    });
}

#[test]
#[allow(clippy::float_cmp)]
fn retained_legacy_line_bezier_parameter_edit_replaces_its_fixed_bound() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let a = sketch.add_point(Point2::origin()).unwrap();
    let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let line = sketch.add_segment(a, b).unwrap();
    let controls = [
        sketch.add_point(Point2::origin()).unwrap(),
        sketch.add_point(Point2::new(1.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(-1.0, 0.0)).unwrap(),
        sketch.add_point(Point2::origin()).unwrap(),
    ];
    let curve = sketch.add_cubic_bezier("loop", controls).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch
        .add_segment_length(line, 2.0, DimensionMode::Driving)
        .unwrap();
    let tangency = sketch
        .add_line_bezier_tangency(
            line,
            SegmentEndpoint::Start,
            curve,
            0.0,
            CurveTangentOrientation::Aligned,
        )
        .unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before_revisions = session.revisions();
    let result = session
        .apply_patch(SketchSessionPatch::new(
            session.revision(),
            SketchPatch::ContactState {
                constraint: tangency,
                state: ContactState::LineBezierTangency { parameter: 1.0 },
            },
        ))
        .unwrap();
    assert!(result.accepted(), "{result:#?}");
    let bound = session
        .bound_report(geosolve_sketch::SketchBound::Contact {
            constraint_id: tangency,
            role: geosolve_sketch::LatentVariableRole::BezierParameter,
        })
        .unwrap();
    assert_eq!(bound.lower, Some(1.0));
    assert_eq!(bound.upper, Some(1.0));
    assert_eq!(bound.value, 1.0);
    assert_eq!(session.revisions().bound, before_revisions.bound + 1);
    let retained_bound_revision = session.revisions().bound;
    let result = session
        .apply_patch(SketchSessionPatch::new(
            session.revision(),
            SketchPatch::ContactState {
                constraint: tangency,
                state: ContactState::LineBezierTangency { parameter: 1.0 },
            },
        ))
        .unwrap();
    assert!(result.accepted(), "{result:#?}");
    assert_eq!(session.revisions().bound, retained_bound_revision);
}

#[test]
fn document_drag_request_updates_and_releases_without_command_history() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let point = document.add_point("dragged point", [0.0, 0.0]).unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let dragged = session
        .rebuild_request(
            session.revision(),
            DocumentSolveRequest::default().with_drag(point, [2.0, 3.0]),
        )
        .unwrap();
    assert!(dragged.accepted());
    let position = session.document().point(point).unwrap().position;
    assert!((position[0] - 2.0).abs() <= 1.0e-12, "{position:?}");
    assert!((position[1] - 3.0).abs() <= 1.0e-12, "{position:?}");
    assert_eq!(session.history_len(), 0);

    let released = session
        .rebuild_request(session.revision(), DocumentSolveRequest::default())
        .unwrap();
    assert!(released.accepted());
    let released_position = session.document().point(point).unwrap().position;
    assert!((released_position[0] - 2.0).abs() <= 1.0e-12);
    assert!((released_position[1] - 3.0).abs() <= 1.0e-12);
    assert_eq!(session.history_len(), 0);
    assert_eq!(session.request(), DocumentSolveRequest::default());
}

#[test]
fn perturbed_generic_line_circle_tangency_recovers_with_scale_invariant_rank_and_branch() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let start = sketch.add_point(Point2::new(-2.0 * scale, 0.0)).unwrap();
        let end = sketch.add_point(Point2::new(2.0 * scale, 0.0)).unwrap();
        let center = sketch.add_point(Point2::new(0.0, 1.2 * scale)).unwrap();
        let line = sketch.add_segment(start, end).unwrap();
        let circle = sketch.add_circle(center, scale).unwrap();
        sketch.add_fixed_point(start).unwrap();
        sketch.add_fixed_point(end).unwrap();
        sketch
            .add_fixed_coordinate(center, CoordinateAxis::X, 0.0)
            .unwrap();
        sketch
            .add_circle_radius(circle, scale, DimensionMode::Driving)
            .unwrap();
        let tangency = sketch
            .add_curve_curve_tangency(
                SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment: line,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 0.5,
                    neighborhood: CurveContactNeighborhood::Local {
                        lower: 0.25,
                        upper: 0.75,
                    },
                },
                SketchCurveContact {
                    curve: SketchCurve::Circle(circle),
                    parameter: -FRAC_PI_2,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                CurveTangentOrientation::Aligned,
            )
            .unwrap();
        let solved = sketch
            .solve(
                SketchSolveRequest::default().without_previous_state_preferences(),
                SolverConfig::default(),
            )
            .unwrap();
        assert!(solved.accepted(), "scale={scale:e}: {solved:#?}");
        assert_eq!(solved.core_report.hard_validity, HardValidity::Valid);
        assert!(solved.core_report.hard_residual_max <= 1.0e-9);
        let solved_center = solved.geometry.point(center).unwrap();
        assert!(solved_center.y > 0.0, "scale={scale:e}: {solved_center:?}");
        assert!(
            (solved_center.y / scale - 1.0).abs() <= 1.0e-8,
            "scale={scale:e}: {solved_center:?}"
        );
        assert!(solved.core_report.rank_is_valid);
        assert_eq!(solved.core_report.rank, 5);
        assert_eq!(solved.core_report.left_nullity, 0);
        assert_eq!(solved.core_report.right_nullity, 0);
        assert_eq!(solved.core_report.local_degrees_of_freedom, 0);
        assert_eq!(solved.bound_mappings.len(), 2);
        assert_eq!(solved.core_report.bounds.len(), 2);
        let radius_mapping = solved
            .bound_mappings
            .iter()
            .find(|mapping| mapping.bound == geosolve_sketch::SketchBound::CircleRadius(circle))
            .unwrap();
        let radius_bound = solved
            .core_report
            .bounds
            .iter()
            .find(|bound| bound.bound_id == radius_mapping.bound_id)
            .unwrap();
        assert_eq!(radius_bound.status, geosolve_core::BoundStatus::Inactive);
        assert_eq!(
            radius_bound.lower,
            Some(geosolve_sketch::MIN_REPRESENTABLE_RADIUS)
        );
        assert_eq!(radius_bound.upper, None);
        assert!((radius_bound.value / scale - 1.0).abs() <= 1.0e-9);
        let contact_mapping = solved
            .bound_mappings
            .iter()
            .find(|mapping| {
                mapping.bound
                    == geosolve_sketch::SketchBound::Contact {
                        constraint_id: tangency,
                        role: geosolve_sketch::LatentVariableRole::FirstCurveParameter,
                    }
            })
            .unwrap();
        let contact_bound = solved
            .core_report
            .bounds
            .iter()
            .find(|bound| bound.bound_id == contact_mapping.bound_id)
            .unwrap();
        assert_eq!(contact_bound.status, geosolve_core::BoundStatus::Inactive);
        assert_eq!(contact_bound.lower, Some(0.25));
        assert_eq!(contact_bound.upper, Some(0.75));
        assert!((contact_bound.value - 0.5).abs() <= 1.0e-9);
    }
}

#[test]
fn rejected_document_attempt_retains_candidate_bound_mappings_separately() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let controls = [
        document.add_point("Q0", [-1.0, 0.0]).unwrap(),
        document.add_point("Q1", [0.0, 0.0]).unwrap(),
        document.add_point("Q2", [1.0, 0.0]).unwrap(),
    ];
    let curve = document
        .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    let point = document
        .add_point("fixed off-curve point", [0.0, 1.0])
        .unwrap();
    for (index, fixed) in controls.into_iter().chain([point]).enumerate() {
        let target = document.point(fixed).unwrap().position;
        document
            .add_constraint(
                format!("fixed {index}"),
                DocumentConstraintDefinition::FixedPoint {
                    point: fixed,
                    target,
                },
            )
            .unwrap();
    }
    let parameter = document
        .add_scalar(
            "contact parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let contact = document
        .add_contact(
            "local contact",
            ContactDefinition {
                curve: CurveSpan::line(curve),
                parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
                tangent_orientation: None,
            },
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let rejected = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateConstraint {
                label: "impossible point contact".into(),
                definition: DocumentConstraintDefinition::PointOnCurve { point, contact },
            },
        ))
        .unwrap();
    assert!(!rejected.accepted(), "{:#?}", rejected.result.solve());
    assert_eq!(rejected.result.solve().bound_mappings.len(), 1);
    assert!(rejected.result.accepted_view().bound_mappings.is_empty());
    assert_eq!(rejected.result.attempted_bound_mappings().len(), 1);
    assert_eq!(
        rejected.result.attempted_bound_mappings().len(),
        rejected.result.solve().core_report.bounds.len()
    );
}

#[test]
fn active_drag_is_released_across_history_and_import_transitions() {
    let mut session = SketchDocumentSession::new(
        SketchDocument::new(2.0).unwrap(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "temporary point".into(),
                position: [0.0, 0.0],
            },
        ))
        .unwrap();
    let Some(geosolve_sketch::DocumentCommandEffect::CreatedPoint(point)) = created.effect else {
        panic!("created point effect expected");
    };
    session
        .rebuild_request(
            session.revision(),
            DocumentSolveRequest::default().with_drag(point, [1.0, 1.0]),
        )
        .unwrap();
    session.undo(session.revision()).unwrap();
    assert_eq!(session.request().drag, None);
    assert!(session.document().point(point).is_none());

    session.redo(session.revision()).unwrap();
    assert_eq!(session.request().drag, None);
    session
        .rebuild_request(
            session.revision(),
            DocumentSolveRequest::default().with_drag(point, [1.0, 1.0]),
        )
        .unwrap();
    let imported = SketchDocument::new(2.0)
        .unwrap()
        .to_canonical_json()
        .unwrap();
    let outcome = session.import_json(session.revision(), &imported).unwrap();
    assert!(outcome.accepted());
    assert_eq!(session.request().drag, None);
    assert!(session.document().point(point).is_none());
}
