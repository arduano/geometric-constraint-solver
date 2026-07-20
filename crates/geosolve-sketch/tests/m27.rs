// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use geosolve_core::{HardValidity, SolveTermination, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ArcSweep, ContactDefinition, ContactDomain, ContactNeighborhood, CurveContactNeighborhood,
    CurveDefinition, CurveMeasurementKind, CurveNormalSide, CurveSpan, DimensionMode,
    DocumentArcSweep, DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentCurveNormalSide, DocumentDimensionMode, DocumentEdit, DocumentFilletEndpointOrder,
    DocumentObjectId, DocumentSolveRequest, FilletEndpointOrder, LineLineFilletIds,
    LineLineFilletRequest, LineParameterDomain, ScalarDomain, ScalarUnit, Sketch, SketchCurve,
    SketchCurveContact, SketchDocument, SketchDocumentSession, SketchSolveRequest,
};

fn transformed(point: [f64; 2], scale: f64, angle: f64, offset: [f64; 2]) -> [f64; 2] {
    let (sine, cosine) = angle.sin_cos();
    [
        scale * (cosine * point[0] - sine * point[1]) + offset[0],
        scale * (sine * point[0] + cosine * point[1]) + offset[1],
    ]
}

fn crossing_parents(
    scale: f64,
    angle: f64,
    offset: [f64; 2],
    fixed: bool,
) -> (
    SketchDocument,
    [geosolve_sketch::CurveId; 2],
    [geosolve_sketch::DesignPointId; 4],
) {
    let mut document = SketchDocument::new(scale).unwrap();
    let positions = [[-4.0, 0.0], [4.0, 0.0], [0.0, -4.0], [0.0, 4.0]]
        .map(|point| transformed(point, scale, angle, offset));
    let points = positions.map(|position| document.add_point("parent point", position).unwrap());
    let (sine, cosine) = angle.sin_cos();
    let curves = [
        document
            .add_curve(
                "first parent",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [cosine, sine],
                },
            )
            .unwrap(),
        document
            .add_curve(
                "second parent",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: [-sine, cosine],
                },
            )
            .unwrap(),
    ];
    if fixed {
        for (index, (point, target)) in points.into_iter().zip(positions).enumerate() {
            document
                .add_constraint(
                    format!("fixed parent {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
    }
    (document, curves, points)
}

fn request(
    curves: [geosolve_sketch::CurveId; 2],
    first_side: DocumentCurveNormalSide,
    second_side: DocumentCurveNormalSide,
    endpoint_order: DocumentFilletEndpointOrder,
    sweep: DocumentArcSweep,
    radius: f64,
    radius_mode: DocumentDimensionMode,
) -> LineLineFilletRequest {
    LineLineFilletRequest {
        first: CurveSpan::line(curves[0]),
        first_side,
        second: CurveSpan::line(curves[1]),
        second_side,
        endpoint_order,
        sweep,
        radius,
        radius_mode,
    }
}

#[test]
fn runtime_line_fillet_rows_derive_the_accepted_arc() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first_start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let corner = sketch.add_point(Point2::new(4.0, 0.0)).unwrap();
    let second_end = sketch.add_point(Point2::new(4.0, 4.0)).unwrap();
    let center = sketch.add_point(Point2::new(3.2, 0.8)).unwrap();
    let first = sketch.add_segment(first_start, corner).unwrap();
    let second = sketch.add_segment(corner, second_end).unwrap();
    for point in [first_start, corner, second_end] {
        sketch.add_fixed_point(point).unwrap();
    }
    let arc = sketch
        .add_arc(
            center,
            0.8,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let contact = |segment, parameter| SketchCurveContact {
        curve: SketchCurve::Line {
            segment,
            domain: LineParameterDomain::BoundedSegment,
        },
        parameter,
        neighborhood: CurveContactNeighborhood::Interior,
    };
    let mut preused = sketch.clone();
    preused.add_point_on_arc(first_start, arc, 0.5).unwrap();
    assert!(
        preused
            .add_line_line_fillet(
                arc,
                contact(first, 0.8),
                CurveNormalSide::Left,
                contact(second, 0.2),
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
    sketch
        .add_line_line_fillet(
            arc,
            contact(first, 0.8),
            CurveNormalSide::Left,
            contact(second, 0.2),
            CurveNormalSide::Left,
            FilletEndpointOrder::FirstThenSecond,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, 1.0, DimensionMode::Driving)
        .unwrap();

    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
    assert_eq!(
        compiled
            .problem()
            .audit_rows()
            .unwrap()
            .iter()
            .filter(|row| row.template.contains("left_normal"))
            .count(),
        4
    );

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    let solved = result.geometry.arc(arc).unwrap();
    assert!((solved.center.x - 3.0).abs() <= 1.0e-9);
    assert!((solved.center.y - 1.0).abs() <= 1.0e-9);
    assert!((solved.radius - 1.0).abs() <= 1.0e-9);
    let (start, end) = solved.endpoints().unwrap();
    assert!((start - Point2::new(3.0, 0.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(4.0, 1.0)).norm() <= 1.0e-9);
    assert_eq!(result.core_report.local_degrees_of_freedom, 0);
    assert!(
        (sketch
            .measure_curve(
                SketchCurveContact {
                    curve: SketchCurve::Arc(arc),
                    parameter: 0.5,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                CurveMeasurementKind::UnsignedCurvature,
            )
            .unwrap()
            - 1.0)
            .abs()
            <= 1.0e-9
    );
    assert!(sketch.add_point_on_arc(first_start, arc, 0.5).is_err());
    assert!(
        sketch
            .add_line_line_fillet(
                arc,
                contact(first, 0.75),
                CurveNormalSide::Left,
                contact(second, 0.25),
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
}

#[test]
fn persistent_line_fillet_round_trips_and_projects_derived_endpoints() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let corner = document.add_point("corner", [4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    for (label, point, target) in [
        ("fix first", first_start, [0.0, 0.0]),
        ("fix corner", corner, [4.0, 0.0]),
        ("fix second", second_end, [4.0, 4.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    let fillet = document
        .add_line_line_fillet(
            "fillet",
            LineLineFilletRequest {
                first: CurveSpan::line(first),
                first_side: DocumentCurveNormalSide::Left,
                second: CurveSpan::line(second),
                second_side: DocumentCurveNormalSide::Left,
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let canonical = document.to_canonical_json().unwrap();
    assert!(canonical.contains("\"version\":3"));
    assert!(canonical.contains("\"kind\":\"line_line_fillet\""));
    assert_eq!(
        SketchDocument::from_json(&canonical)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    assert!(
        SketchDocument::from_json(&canonical.replacen("\"version\":3", "\"version\":2", 1))
            .is_err()
    );
    assert!(
        SketchDocument::from_json(&canonical.replacen("\"version\":3", "\"version\":1", 1))
            .is_err()
    );

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.runtime().accepted_result().accepted());
    let accepted = session.document();
    let start = accepted
        .evaluate_curve_jet(CurveSpan::line(fillet.arc), 0.0)
        .unwrap()
        .position;
    let end = accepted
        .evaluate_curve_jet(CurveSpan::line(fillet.arc), 1.0)
        .unwrap()
        .position;
    assert!((start - Point2::new(3.0, 0.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(4.0, 1.0)).norm() <= 1.0e-9);
    assert!(
        accepted
            .project_curve_trim_endpoint(
                fillet.arc,
                geosolve_sketch::FeatureEndpoint::Start,
                [3.0, 0.0]
            )
            .is_err()
    );
    assert!(
        accepted
            .clone()
            .set_scalar_value(fillet.start_angle, 0.25)
            .is_err()
    );
    let mut consumer = accepted.clone();
    let consumer_point = consumer.add_point("consumer", [3.0, 0.0]).unwrap();
    let consumer_parameter = consumer
        .add_scalar(
            "consumer parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let consumer_contact = consumer
        .add_contact(
            "consumer contact",
            ContactDefinition {
                curve: CurveSpan::line(fillet.arc),
                parameter: consumer_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .unwrap();
    assert!(
        consumer
            .add_constraint(
                "unsupported fillet consumer",
                DocumentConstraintDefinition::PointOnCurve {
                    point: consumer_point,
                    contact: consumer_contact,
                }
            )
            .is_err()
    );
}

#[test]
fn every_side_order_and_sweep_is_similarity_covariant_at_all_scales() {
    let angle = 0.37;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let offset = [7.0 * scale, -3.0 * scale];
        for first_side in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            for second_side in [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ] {
                for endpoint_order in [
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentFilletEndpointOrder::SecondThenFirst,
                ] {
                    for sweep in [
                        DocumentArcSweep::CounterClockwise,
                        DocumentArcSweep::Clockwise,
                    ] {
                        let (mut document, curves, _) =
                            crossing_parents(scale, angle, offset, true);
                        let ids = document
                            .add_line_line_fillet(
                                "branch fillet",
                                request(
                                    curves,
                                    first_side,
                                    second_side,
                                    endpoint_order,
                                    sweep,
                                    scale,
                                    DocumentDimensionMode::Driving,
                                ),
                            )
                            .unwrap();
                        if first_side == DocumentCurveNormalSide::Left
                            && second_side == DocumentCurveNormalSide::Left
                            && endpoint_order == DocumentFilletEndpointOrder::FirstThenSecond
                            && sweep == DocumentArcSweep::CounterClockwise
                        {
                            let lowered = document.lower().unwrap();
                            let compiled = lowered
                                .sketch()
                                .compile(SketchSolveRequest::default())
                                .unwrap();
                            let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
                            assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
                        }
                        let session = SketchDocumentSession::new(
                            document,
                            DocumentSolveRequest::default(),
                            SolverConfig::default(),
                        )
                        .unwrap();
                        assert!(session.runtime().accepted_result().accepted());
                        assert_eq!(
                            session
                                .runtime()
                                .accepted_result()
                                .core_report
                                .local_degrees_of_freedom,
                            0
                        );
                        let first_sign = if first_side == DocumentCurveNormalSide::Left {
                            1.0
                        } else {
                            -1.0
                        };
                        let second_sign = if second_side == DocumentCurveNormalSide::Left {
                            1.0
                        } else {
                            -1.0
                        };
                        let first_contact = transformed([-second_sign, 0.0], scale, angle, offset);
                        let second_contact = transformed([0.0, first_sign], scale, angle, offset);
                        let (expected_start, expected_end) = match endpoint_order {
                            DocumentFilletEndpointOrder::FirstThenSecond => {
                                (first_contact, second_contact)
                            }
                            DocumentFilletEndpointOrder::SecondThenFirst => {
                                (second_contact, first_contact)
                            }
                        };
                        let accepted = session.document();
                        let start = accepted
                            .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
                            .unwrap()
                            .position;
                        let end = accepted
                            .evaluate_curve_jet(CurveSpan::line(ids.arc), 1.0)
                            .unwrap()
                            .position;
                        let tolerance = 2.0e-9 * scale;
                        assert!(
                            (start - Point2::new(expected_start[0], expected_start[1])).norm()
                                <= tolerance,
                            "scale={scale:e} {first_side:?} {second_side:?} {endpoint_order:?} {sweep:?}"
                        );
                        assert!(
                            (end - Point2::new(expected_end[0], expected_end[1])).norm()
                                <= tolerance
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn reference_radius_keeps_one_fillet_dof_and_reports_its_measurement() {
    let (mut document, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], true);
    let ids = document
        .add_line_line_fillet(
            "reference fillet",
            request(
                curves,
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Left,
                DocumentFilletEndpointOrder::FirstThenSecond,
                DocumentArcSweep::CounterClockwise,
                1.0,
                DocumentDimensionMode::Reference,
            ),
        )
        .unwrap();
    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(
        session
            .runtime()
            .accepted_result()
            .core_report
            .local_degrees_of_freedom,
        1
    );
    let result = session.accepted_result();
    assert!(
        (result
            .accepted_reference_value(session.document(), ids.radius_dimension)
            .unwrap()
            - 1.0)
            .abs()
            <= 1.0e-9
    );
}

#[test]
fn parent_edit_rederives_contacts_without_trimming_parent_definitions() {
    let (mut document, curves, points) = crossing_parents(1.0, 0.0, [0.0, 0.0], false);
    let ids = document
        .add_line_line_fillet(
            "editable fillet",
            request(
                curves,
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Left,
                DocumentFilletEndpointOrder::FirstThenSecond,
                DocumentArcSweep::CounterClockwise,
                1.0,
                DocumentDimensionMode::Driving,
            ),
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session
        .document()
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    let edit = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: points[1],
                position: [4.0, 1.0],
            },
        ))
        .unwrap();
    assert!(edit.accepted(), "{:#?}", edit.result.solve().rejection);
    let accepted = session.document();
    let after = accepted
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    assert!((after - before).norm() > 1.0e-3);
    let first_contact = accepted
        .evaluate_contact_jet(ids.contacts[0])
        .unwrap()
        .position;
    assert!((after - first_contact).norm() <= 1.0e-9);
    for curve in curves {
        assert!(matches!(
            accepted.curve(curve).unwrap().definition,
            CurveDefinition::Line { .. }
        ));
    }
}

#[test]
fn creation_branch_history_explode_and_failed_escape_are_atomic() {
    let (document, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], true);
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateLineLineFillet {
                label: "command fillet".into(),
                request: request(
                    curves,
                    DocumentCurveNormalSide::Left,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentArcSweep::CounterClockwise,
                    1.0,
                    DocumentDimensionMode::Driving,
                ),
            },
        ))
        .unwrap();
    assert!(created.accepted());
    let DocumentCommandEffect::CreatedLineLineFillet(ids) = created.effect.unwrap() else {
        panic!("unexpected creation effect")
    };
    let ids: LineLineFilletIds = *ids;
    let accepted = session.export_json().unwrap();
    session.undo(session.revision()).unwrap();
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), accepted);

    let before_escape = session.export_json().unwrap();
    let history = session.history_len();
    let escaped = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.radius_target,
                value: 10.0,
            },
        ))
        .unwrap();
    assert!(!escaped.accepted());
    assert_eq!(session.export_json().unwrap(), before_escape);
    assert_eq!(session.history_len(), history);

    let branch = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetLineLineFilletBranch {
                constraint: ids.constraint,
                first_side: DocumentCurveNormalSide::Left,
                second_side: DocumentCurveNormalSide::Left,
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: DocumentArcSweep::Clockwise,
            },
        ))
        .unwrap();
    assert!(branch.accepted());

    let source = session
        .document()
        .constraint(ids.constraint)
        .unwrap()
        .source_id;
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: true,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: ids.start_angle,
                    value: 0.2,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: false,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        (session
            .document()
            .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
            .unwrap()
            .position
            - Point2::new(0.0, 1.0))
        .norm()
            <= 1.0e-9
    );

    let mut selected_arc_delete = session.document().clone();
    assert!(matches!(
        selected_arc_delete.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
        Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
    ));
    for object in [
        DocumentObjectId::Point(ids.center),
        DocumentObjectId::Scalar(ids.radius),
        DocumentObjectId::Scalar(ids.start_angle),
    ] {
        let mut indirect = session.document().clone();
        assert!(matches!(
            indirect.remove_many_with_dependents(&[object]),
            Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
        ));
    }
    let mut suppressed_owner = session.document().clone();
    suppressed_owner
        .set_source_suppressed(source, true)
        .unwrap();
    assert!(suppressed_owner.line_line_fillet_for_arc(ids.arc).is_none());
    assert!(matches!(
        suppressed_owner.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
        Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
    ));

    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Constraint(ids.constraint),
            },
        ))
        .unwrap();
    assert!(deleted.accepted());
    assert!(session.document().curve(ids.arc).is_some());
    assert!(
        ids.contacts
            .iter()
            .all(|contact| session.document().contact(*contact).is_none())
    );
    assert!(
        session
            .document()
            .line_line_fillet_for_arc(ids.arc)
            .is_none()
    );
    session.undo(session.revision()).unwrap();
    assert!(
        session
            .document()
            .line_line_fillet_for_arc(ids.arc)
            .is_some()
    );
    session.redo(session.revision()).unwrap();
    assert!(session.document().curve(ids.arc).is_some());
}

#[test]
fn invalid_parallel_escaped_radius_and_nonfinite_inputs_allocate_nothing() {
    let (mut escaped, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], false);
    let before = escaped.to_canonical_json().unwrap();
    assert!(
        escaped
            .add_line_line_fillet(
                "escaped",
                request(
                    curves,
                    DocumentCurveNormalSide::Left,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentArcSweep::CounterClockwise,
                    5.0,
                    DocumentDimensionMode::Driving,
                )
            )
            .is_err()
    );
    assert_eq!(escaped.to_canonical_json().unwrap(), before);
    for radius in [0.0, f64::NAN, f64::INFINITY] {
        assert!(
            escaped
                .add_line_line_fillet(
                    "invalid radius",
                    request(
                        curves,
                        DocumentCurveNormalSide::Left,
                        DocumentCurveNormalSide::Left,
                        DocumentFilletEndpointOrder::FirstThenSecond,
                        DocumentArcSweep::CounterClockwise,
                        radius,
                        DocumentDimensionMode::Driving,
                    )
                )
                .is_err()
        );
    }

    for slope in [0.0, 1.0e-10] {
        let mut parallel = SketchDocument::new(1.0).unwrap();
        let points = [
            [-4.0, 0.0],
            [4.0, 0.0],
            [-4.0, 1.0],
            [4.0, 1.0 + 8.0 * slope],
        ]
        .map(|position| parallel.add_point("parallel", position).unwrap());
        let first = parallel
            .add_curve(
                "parallel first",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let direction = [
            1.0 / (1.0 + slope * slope).sqrt(),
            slope / (1.0 + slope * slope).sqrt(),
        ];
        let second = parallel
            .add_curve(
                "parallel second",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: direction,
                },
            )
            .unwrap();
        assert!(
            parallel
                .add_line_line_fillet(
                    "parallel fillet",
                    request(
                        [first, second],
                        DocumentCurveNormalSide::Left,
                        DocumentCurveNormalSide::Right,
                        DocumentFilletEndpointOrder::FirstThenSecond,
                        DocumentArcSweep::CounterClockwise,
                        0.5,
                        DocumentDimensionMode::Driving,
                    )
                )
                .is_err()
        );
    }
}
