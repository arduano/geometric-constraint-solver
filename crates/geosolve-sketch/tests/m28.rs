// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

mod m28_support;

use geosolve_core::SolverConfig;
use geosolve_geometry::Point2;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ArcAngleRole, ArcSweep, ContactNeighborhood, ContactState,
    ContactStateEdit, CurveContactNeighborhood, CurveCurvatureRelation, CurveCurveFilletRequest,
    CurveDefinition, CurveFilletParentRequest, CurveNormalSide, CurveSpan, CurveTangentOrientation,
    DimensionMode, DocumentArcSweep, DocumentCommand, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentEdit, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter,
    FilletEndpointOrder, LineParameterDomain, ScalarDomain, ScalarUnit, Sketch, SketchCurve,
    SketchCurveContact, SketchDocument, SketchDocumentSession, SketchPatch, SketchSession,
    SketchSessionPatch, SketchSolveRequest, SketchSource, VisualProfileOptions,
    VisualProfileStatus, alpha_scenario,
};

struct LineCircleFixture {
    document: SketchDocument,
    circle: geosolve_sketch::CurveId,
    line: geosolve_sketch::CurveId,
    line_end: geosolve_sketch::DesignPointId,
}

fn line_circle_fixture(fix_line_end: bool) -> LineCircleFixture {
    let mut document = SketchDocument::new(1.0).unwrap();
    let circle_center = document.add_point("circle center", [0.0, 0.0]).unwrap();
    let line_start = document.add_point("line start", [0.0, 1.0]).unwrap();
    let line_end = document.add_point("line end", [6.0, 1.0]).unwrap();
    let circle_radius = document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: line_start,
                end: line_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    for (label, point, target) in [
        ("fix circle center", circle_center, [0.0, 0.0]),
        ("fix line start", line_start, [0.0, 1.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    if fix_line_end {
        document
            .add_constraint(
                "fix line end",
                DocumentConstraintDefinition::FixedPoint {
                    point: line_end,
                    target: [6.0, 1.0],
                },
            )
            .unwrap();
    }
    let radius_target = document
        .add_scalar(
            "circle radius target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            "fix circle radius",
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target: radius_target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    LineCircleFixture {
        document,
        circle,
        line,
        line_end,
    }
}

fn line_circle_request(
    fixture: &LineCircleFixture,
    circle_winding: i32,
) -> CurveCurveFilletRequest {
    let total = f64::from(circle_winding) * std::f64::consts::TAU;
    CurveCurveFilletRequest {
        first: CurveFilletParentRequest {
            curve: CurveSpan::line(fixture.circle),
            parameter: 0.0,
            winding: circle_winding,
            neighborhood: ContactNeighborhood::Local {
                lower: total - 0.4,
                upper: total + 0.4,
            },
            side: DocumentCurveNormalSide::Right,
            trim_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: Some(DocumentTrimParameter {
                parameter: std::f64::consts::PI,
                winding: circle_winding - 1,
            }),
        },
        second: CurveFilletParentRequest {
            curve: CurveSpan::line(fixture.line),
            parameter: 0.5,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            side: DocumentCurveNormalSide::Right,
            trim_endpoint: DocumentFilletTrimEndpoint::Start,
            periodic_anchor: None,
        },
        endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
        sweep: DocumentArcSweep::CounterClockwise,
        radius: 1.0,
        radius_mode: DocumentDimensionMode::Driving,
    }
}

fn adjacent_polyline_fillet(
    radius_mode: DocumentDimensionMode,
) -> (
    SketchDocument,
    [CurveSpan; 2],
    geosolve_sketch::CurveCurveFilletIds,
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let points = [
        document.add_point("start", [0.0, 0.0]).unwrap(),
        document.add_point("corner", [4.0, 0.0]).unwrap(),
        document.add_point("end", [4.0, 4.0]).unwrap(),
    ];
    for (index, (point, target)) in points
        .into_iter()
        .zip([[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]])
        .enumerate()
    {
        document
            .add_constraint(
                format!("fix polyline point {index}"),
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    let polyline = document
        .add_curve(
            "right angle polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    let spans = [
        CurveSpan {
            curve: polyline,
            segment: 0,
        },
        CurveSpan {
            curve: polyline,
            segment: 1,
        },
    ];
    let ids = document
        .add_curve_curve_fillet(
            "polyline corner fillet",
            CurveCurveFilletRequest {
                first: CurveFilletParentRequest {
                    curve: spans[0],
                    parameter: 0.75,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::End,
                    periodic_anchor: None,
                },
                second: CurveFilletParentRequest {
                    curve: spans[1],
                    parameter: 0.25,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    side: DocumentCurveNormalSide::Left,
                    trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    periodic_anchor: None,
                },
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode,
            },
        )
        .unwrap();
    (document, spans, ids)
}

fn assert_independently_valid(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    let report = accepted.accepted_view().unstable_core_report();
    assert_eq!(report.hard_validity, geosolve_core::HardValidity::Valid);
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
}

#[test]
fn reusable_trimmed_fillet_alpha_scenario_is_accepted_and_scale_invariant() {
    let mut identity = None;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = alpha_scenario(AlphaScenarioKind::M28TrimmedFillet, scale).unwrap();
        let AlphaScenarioIds::M28TrimmedFillet(ids) = fixture.ids else {
            panic!("wrong M28 scenario IDs")
        };
        let session =
            SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
                .unwrap();
        let accepted = session.accepted_result();
        let report = &accepted.accepted_view().unstable_core_report();
        assert_eq!(report.hard_validity, geosolve_core::HardValidity::Valid);
        assert!(report.hard_residual_max <= 1.0e-9);
        let circle = session
            .document()
            .visible_interval(CurveSpan::line(ids.circle))
            .unwrap();
        let line = session
            .document()
            .visible_interval(CurveSpan::line(ids.line))
            .unwrap();
        assert!((circle.start + std::f64::consts::PI).abs() <= 1.0e-12);
        assert!(circle.end.abs() <= 1.0e-12);
        assert!((line.start - 0.5).abs() <= 1.0e-9);
        assert_eq!(line.end.to_bits(), 1.0_f64.to_bits());
        let current = (
            ids.circle,
            ids.line,
            ids.fillet.constraint,
            ids.fillet.arc,
            ids.fillet.contacts,
        );
        if let Some(expected) = identity {
            assert_eq!(current, expected);
        } else {
            identity = Some(current);
        }
    }
}

#[test]
fn adjacent_open_polyline_spans_accept_associative_end_start_fillet() {
    let (document, spans, ids) = adjacent_polyline_fillet(DocumentDimensionMode::Driving);
    let constraint = document.constraint(ids.constraint).unwrap();
    assert!(matches!(
        constraint.definition,
        DocumentConstraintDefinition::CurveCurveFillet {
            first_contact,
            first_trim_endpoint: DocumentFilletTrimEndpoint::End,
            second_contact,
            second_trim_endpoint: DocumentFilletTrimEndpoint::Start,
            ..
        } if first_contact == ids.contacts[0] && second_contact == ids.contacts[1]
    ));
    let first_view = document.trim_view(spans[0]).unwrap();
    assert!(matches!(
        first_view.end,
        DocumentTrimBoundary::FilletContact { owner, contact }
            if owner == ids.constraint && contact == ids.contacts[0]
    ));
    let second_view = document.trim_view(spans[1]).unwrap();
    assert!(matches!(
        second_view.start,
        DocumentTrimBoundary::FilletContact { owner, contact }
            if owner == ids.constraint && contact == ids.contacts[1]
    ));

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_independently_valid(&session);
    let intervals = spans.map(|span| session.document().visible_interval(span).unwrap());
    assert_eq!(intervals[0].start.to_bits(), 0.0f64.to_bits());
    assert!((intervals[0].end - 0.75).abs() <= 1.0e-9);
    assert!((intervals[1].start - 0.25).abs() <= 1.0e-9);
    assert_eq!(intervals[1].end.to_bits(), 1.0f64.to_bits());
}

#[test]
fn adjacent_polyline_reference_fillet_moves_center_and_radius_on_its_one_dof() {
    let (document, spans, ids) = adjacent_polyline_fillet(DocumentDimensionMode::Reference);
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_independently_valid(&session);
    assert_eq!(
        session
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .local_degrees_of_freedom,
        1
    );
    let center_before = session.document().point(ids.center).unwrap().position;
    let radius_before = session.document().scalar(ids.radius).unwrap().value;

    let moved = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.center,
                position: [2.0, 2.0],
            },
        ))
        .unwrap();
    assert!(moved.accepted(), "{moved:#?}");
    assert_independently_valid(&session);
    assert_eq!(
        session
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .local_degrees_of_freedom,
        1
    );
    let center_after = session.document().point(ids.center).unwrap().position;
    let radius_after = session.document().scalar(ids.radius).unwrap().value;
    assert!((center_after[0] - 2.0).abs() <= 1.0e-9);
    assert!((center_after[1] - 2.0).abs() <= 1.0e-9);
    assert!((center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1]) > 0.5);
    assert!((radius_after - radius_before).abs() > 0.5);
    assert!((radius_after - 2.0).abs() <= 1.0e-9);
    assert!(
        (session
            .accepted_result()
            .accepted_reference_value(session.document(), ids.radius_dimension)
            .unwrap()
            - 2.0)
            .abs()
            <= 1.0e-9
    );
    let contacts = ids.contacts.map(|contact| {
        session
            .document()
            .evaluate_contact_jet(contact)
            .unwrap()
            .position
    });
    assert!((contacts[0] - Point2::new(2.0, 0.0)).norm() <= 1.0e-9);
    assert!((contacts[1] - Point2::new(4.0, 2.0)).norm() <= 1.0e-9);
    let intervals = spans.map(|span| session.document().visible_interval(span).unwrap());
    assert!((intervals[0].end - 0.5).abs() <= 1.0e-9);
    assert!((intervals[1].start - 0.5).abs() <= 1.0e-9);
}

#[test]
fn deleting_a_driving_fillet_radius_releases_the_same_center_and_radius_motion() {
    let (document, spans, ids) = adjacent_polyline_fillet(DocumentDimensionMode::Driving);
    let association = document.constraint(ids.constraint).unwrap().clone();
    let contacts = ids
        .contacts
        .map(|contact| document.contact(contact).unwrap().clone());
    let trim_views = spans.map(|span| *document.trim_view(span).unwrap());
    let output_definition = document.curve(ids.arc).unwrap().definition.clone();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_independently_valid(&session);
    assert_eq!(
        session
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .local_degrees_of_freedom,
        0
    );

    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Dimension(ids.radius_dimension),
            },
        ))
        .unwrap();
    assert!(deleted.accepted(), "{deleted:#?}");
    assert!(session.document().dimension(ids.radius_dimension).is_none());
    assert_eq!(
        session.document().constraint(ids.constraint),
        Some(&association)
    );
    assert_eq!(
        ids.contacts
            .map(|contact| session.document().contact(contact).unwrap().clone()),
        contacts
    );
    assert_eq!(
        spans.map(|span| *session.document().trim_view(span).unwrap()),
        trim_views
    );
    assert_eq!(
        session.document().curve(ids.arc).unwrap().definition,
        output_definition
    );
    assert_independently_valid(&session);
    assert_eq!(
        session
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .local_degrees_of_freedom,
        1
    );

    let moved = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.center,
                position: [2.0, 2.0],
            },
        ))
        .unwrap();
    assert!(moved.accepted(), "{moved:#?}");
    assert_independently_valid(&session);
    let center = session.document().point(ids.center).unwrap().position;
    let radius = session.document().scalar(ids.radius).unwrap().value;
    assert!((center[0] - 2.0).abs() <= 1.0e-9);
    assert!((center[1] - 2.0).abs() <= 1.0e-9);
    assert!((radius - 2.0).abs() <= 1.0e-9);
    assert_eq!(
        session.document().constraint(ids.constraint),
        Some(&association)
    );
    assert_eq!(
        ids.contacts
            .map(|contact| session.document().contact(contact).unwrap().clone()),
        contacts
    );
    assert_eq!(
        spans.map(|span| *session.document().trim_view(span).unwrap()),
        trim_views
    );
    assert_eq!(
        session.document().curve(ids.arc).unwrap().definition,
        output_definition
    );
    let contact_positions = ids.contacts.map(|contact| {
        session
            .document()
            .evaluate_contact_jet(contact)
            .unwrap()
            .position
    });
    assert!((contact_positions[0] - Point2::new(2.0, 0.0)).norm() <= 1.0e-9);
    assert!((contact_positions[1] - Point2::new(4.0, 2.0)).norm() <= 1.0e-9);
    let intervals = spans.map(|span| session.document().visible_interval(span).unwrap());
    assert!((intervals[0].end - 0.5).abs() <= 1.0e-9);
    assert!((intervals[1].start - 0.5).abs() <= 1.0e-9);
}

#[test]
fn trimmed_square_side_cannot_publish_the_old_full_support_face() {
    let mut fixture = line_circle_fixture(true);
    let CurveDefinition::Line {
        start: bottom_left,
        end: bottom_right,
        ..
    } = fixture.document.curve(fixture.line).unwrap().definition
    else {
        unreachable!()
    };
    let top_right = fixture.document.add_point("top right", [6.0, 5.0]).unwrap();
    let top_left = fixture.document.add_point("top left", [0.0, 5.0]).unwrap();
    let mut add_line = |label, start, end| {
        let first = fixture.document.point(start).unwrap().position;
        let second = fixture.document.point(end).unwrap().position;
        let delta = [second[0] - first[0], second[1] - first[1]];
        let length = delta[0].hypot(delta[1]);
        fixture
            .document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .unwrap();
    };
    add_line("right", bottom_right, top_right);
    add_line("top", top_right, top_left);
    add_line("left", top_left, bottom_left);
    let before = fixture
        .document
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(before.status, VisualProfileStatus::Complete);
    assert_eq!(before.intersections.len(), 2, "{before:#?}");
    assert_eq!(before.faces.len(), 3, "{before:#?}");
    assert!(before.faces.iter().all(|face| face.visual_area > 0.0));
    assert!(
        before
            .faces
            .iter()
            .all(|face| (face.visual_area - 24.0).abs() > f64::EPSILON)
    );

    fixture
        .document
        .add_curve_curve_fillet("generic", line_circle_request(&fixture, 0))
        .unwrap();
    let after = fixture
        .document
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(after.status, VisualProfileStatus::Complete, "{after:#?}");
    assert!(after.issues.is_empty(), "{after:#?}");
    assert!(after.faces.is_empty(), "{after:#?}");
}

#[test]
fn v4_round_trip_and_frozen_v1_v3_languages_are_strict() {
    let baseline = line_circle_fixture(true).document;
    let mut baseline_wire: serde_json::Value =
        serde_json::from_str(&baseline.to_canonical_json().unwrap()).unwrap();
    baseline_wire.as_object_mut().unwrap().remove("trim_views");
    for old_version in [1, 2, 3] {
        baseline_wire["version"] = old_version.into();
        let migrated =
            SketchDocument::from_json(&serde_json::to_string(&baseline_wire).unwrap()).unwrap();
        assert_eq!(migrated.version(), 4);
        assert!(migrated.trim_views().is_empty());
    }

    let mut periodic_local = line_circle_fixture(true);
    let point = periodic_local
        .document
        .add_point("periodic local point", [2.0, 0.0])
        .unwrap();
    let contact = periodic_local
        .document
        .add_curve_contact(
            "periodic local contact",
            CurveSpan::line(periodic_local.circle),
            0.0,
            0,
            ContactNeighborhood::Local {
                lower: -0.25,
                upper: 0.25,
            },
            None,
        )
        .unwrap();
    periodic_local
        .document
        .add_constraint(
            "periodic local point on curve",
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
    let mut periodic_local_wire: serde_json::Value =
        serde_json::from_str(&periodic_local.document.to_canonical_json().unwrap()).unwrap();
    periodic_local_wire
        .as_object_mut()
        .unwrap()
        .remove("trim_views");
    for old_version in [1, 2, 3] {
        periodic_local_wire["version"] = old_version.into();
        assert!(
            SketchDocument::from_json(&serde_json::to_string(&periodic_local_wire).unwrap())
                .is_err()
        );
    }

    let mut fixture = line_circle_fixture(true);
    fixture
        .document
        .add_curve_curve_fillet("generic", line_circle_request(&fixture, 0))
        .unwrap();
    let canonical = fixture.document.to_canonical_json().unwrap();
    assert!(canonical.contains("\"version\":4"));
    assert!(canonical.contains("\"trim_views\""));
    assert!(canonical.contains("\"kind\":\"curve_curve_fillet\""));
    assert_eq!(
        SketchDocument::from_json(&canonical)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    for old_version in [1, 2, 3] {
        let relabeled =
            canonical.replacen("\"version\":4", &format!("\"version\":{old_version}"), 1);
        assert!(SketchDocument::from_json(&relabeled).is_err());

        let mut generic_only: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        generic_only["version"] = old_version.into();
        generic_only.as_object_mut().unwrap().remove("trim_views");
        assert!(SketchDocument::from_json(&serde_json::to_string(&generic_only).unwrap()).is_err());
    }

    let mut legacy = SketchDocument::new(1.0).unwrap();
    let points = [
        legacy.add_point("a", [-2.0, 0.0]).unwrap(),
        legacy.add_point("b", [2.0, 0.0]).unwrap(),
        legacy.add_point("c", [0.0, -2.0]).unwrap(),
        legacy.add_point("d", [0.0, 2.0]).unwrap(),
    ];
    let first = legacy
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: points[0],
                end: points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = legacy
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: points[2],
                end: points[3],
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    legacy
        .add_line_line_fillet(
            "legacy",
            geosolve_sketch::LineLineFilletRequest {
                first: CurveSpan::line(first),
                first_side: DocumentCurveNormalSide::Left,
                second: CurveSpan::line(second),
                second_side: DocumentCurveNormalSide::Left,
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 0.5,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let mut legacy_value: serde_json::Value =
        serde_json::from_str(&legacy.to_canonical_json().unwrap()).unwrap();
    legacy_value["version"] = 3.into();
    legacy_value.as_object_mut().unwrap().remove("trim_views");
    let migrated =
        SketchDocument::from_json(&serde_json::to_string(&legacy_value).unwrap()).unwrap();
    assert_eq!(migrated.version(), 4);
    assert!(migrated.trim_views().is_empty());
    assert!(migrated.constraints().iter().any(|constraint| matches!(
        constraint.definition,
        DocumentConstraintDefinition::LineLineFillet { .. }
    )));
}

#[test]
fn generic_command_creates_two_visible_trims_and_projects_parent_edits() {
    let fixture = line_circle_fixture(false);
    let request = line_circle_request(&fixture, 0);
    let mut session = SketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateCurveCurveFillet {
                label: "generic".into(),
                request,
            },
        ))
        .unwrap();
    assert!(
        created.accepted(),
        "{:#?}",
        created.result.solve().rejection
    );
    let DocumentCommandEffect::CreatedCurveCurveFillet(ids) = created.effect.unwrap() else {
        panic!("unexpected command effect")
    };
    assert_eq!(session.document().trim_views().len(), 2);
    let circle_interval = session
        .document()
        .visible_interval(CurveSpan::line(fixture.circle))
        .unwrap();
    assert!((circle_interval.start + std::f64::consts::PI).abs() <= 1.0e-12);
    assert!(circle_interval.end.abs() <= 1.0e-12);
    let before = session
        .document()
        .visible_interval(CurveSpan::line(fixture.line))
        .unwrap();
    assert!((before.start - 0.5).abs() <= 1.0e-9);
    assert_eq!(before.end.to_bits(), 1.0f64.to_bits());
    assert!(
        session
            .document()
            .is_parameter_visible(CurveSpan::line(fixture.line), 0.75)
            .unwrap()
    );
    assert!(
        !session
            .document()
            .is_parameter_visible(CurveSpan::line(fixture.line), 0.25)
            .unwrap()
    );
    let start = session
        .document()
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    let end = session
        .document()
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 1.0)
        .unwrap()
        .position;
    assert!((start - Point2::new(3.0, 1.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(2.0, 0.0)).norm() <= 1.0e-9);

    let edited = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: fixture.line_end,
                position: [6.0, 2.0],
            },
        ))
        .unwrap();
    assert!(edited.accepted(), "{:#?}", edited.result.solve().rejection);
    let after = session
        .document()
        .visible_interval(CurveSpan::line(fixture.line))
        .unwrap();
    assert!((after.start - before.start).abs() > 1.0e-4);
    assert_eq!(after.start_boundary, before.start_boundary);

    let branch = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetCurveCurveFilletBranch {
                constraint: ids.constraint,
                first_side: DocumentCurveNormalSide::Right,
                first_trim_endpoint: DocumentFilletTrimEndpoint::End,
                second_side: DocumentCurveNormalSide::Right,
                second_trim_endpoint: DocumentFilletTrimEndpoint::End,
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        ))
        .unwrap();
    assert!(branch.accepted(), "{:#?}", branch.result.solve().rejection);
    let changed = session
        .document()
        .trim_view(CurveSpan::line(fixture.line))
        .unwrap();
    assert!(matches!(
        changed.end,
        DocumentTrimBoundary::FilletContact {
            owner,
            contact
        } if owner == ids.constraint && contact == ids.contacts[1]
    ));
}

#[test]
fn suppression_explode_fixed_views_and_output_ownership_are_preserved() {
    let mut fixture = line_circle_fixture(true);
    let ids = fixture
        .document
        .add_curve_curve_fillet("generic", line_circle_request(&fixture, 0))
        .unwrap();
    let mut output_delete = fixture.document.clone();
    assert!(matches!(
        output_delete.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
        Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
    ));
    let mut parent_delete = fixture.document.clone();
    parent_delete
        .remove_many_with_dependents(&[DocumentObjectId::Curve(fixture.line)])
        .unwrap();
    assert!(parent_delete.curve(fixture.line).is_none());
    assert!(
        parent_delete
            .trim_view(CurveSpan::line(fixture.line))
            .is_none()
    );
    assert!(matches!(
        parent_delete
            .trim_view(CurveSpan::line(fixture.circle))
            .unwrap()
            .end,
        DocumentTrimBoundary::Fixed(_)
    ));
    assert!(parent_delete.curve(ids.arc).is_some());
    let mut session = SketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let source = session
        .document()
        .constraint(ids.constraint)
        .unwrap()
        .source_id;
    let before = session
        .document()
        .visible_curve_intervals(fixture.line)
        .unwrap();
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
    assert_eq!(
        session
            .document()
            .visible_curve_intervals(fixture.line)
            .unwrap(),
        before
    );
    let mut suppressed_contact_edit = session.document().clone();
    let edits = ids.contacts.map(|contact| {
        let slot = session.document().contact(contact).unwrap();
        ContactStateEdit {
            contact,
            value: session.document().scalar(slot.parameter).unwrap().value + 0.01,
            winding: slot.winding,
            neighborhood: slot.neighborhood,
            tangent_orientation: slot.tangent_orientation,
        }
    });
    assert!(suppressed_contact_edit.set_contact_states(&edits).is_err());
    assert_eq!(
        suppressed_contact_edit
            .visible_curve_intervals(fixture.line)
            .unwrap(),
        before
    );
    let mut owned_view = session.document().clone();
    assert!(
        owned_view
            .clear_fixed_trim_view(CurveSpan::line(fixture.line))
            .is_err()
    );
    let mut suppressed_delete = session.document().clone();
    assert!(matches!(
        suppressed_delete.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
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
    for support in [
        CurveSpan::line(fixture.circle),
        CurveSpan::line(fixture.line),
    ] {
        let view = *session.document().trim_view(support).unwrap();
        assert!(matches!(view.start, DocumentTrimBoundary::Fixed(_)));
        assert!(matches!(view.end, DocumentTrimBoundary::Fixed(_)));
    }
    session
        .transact(session.revision(), "clear fixed trim", |document| {
            document.clear_fixed_trim_view(CurveSpan::line(fixture.line))?;
            Ok(())
        })
        .unwrap();
    assert!(
        session
            .document()
            .trim_view(CurveSpan::line(fixture.line))
            .is_none()
    );
}

#[test]
fn malformed_ownership_conflicts_and_branch_edits_roll_back() {
    let mut fixture = line_circle_fixture(true);
    let request = line_circle_request(&fixture, 0);
    let ids = fixture
        .document
        .add_curve_curve_fillet("generic", request)
        .unwrap();
    let accepted = fixture.document.to_canonical_json().unwrap();
    assert!(
        fixture
            .document
            .add_curve_curve_fillet("conflict", request)
            .is_err()
    );
    assert_eq!(fixture.document.to_canonical_json().unwrap(), accepted);

    let mut malformed: serde_json::Value = serde_json::from_str(&accepted).unwrap();
    let bad_owner = malformed["id"].clone();
    let views = malformed["trim_views"].as_array_mut().unwrap();
    let mut changed = false;
    'views: for view in views {
        for key in ["start", "end"] {
            let boundary = view.get_mut(key).unwrap();
            if boundary["kind"] == "fillet_contact" {
                boundary["owner"] = bad_owner.clone();
                changed = true;
                break 'views;
            }
        }
    }
    assert!(changed);
    assert!(SketchDocument::from_json(&serde_json::to_string(&malformed).unwrap()).is_err());

    let mut session = SketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.export_json().unwrap();
    let revision = session.revision();
    let history = session.history_len();
    assert!(
        session
            .apply(DocumentCommand::new(
                revision,
                DocumentEdit::SetCurveCurveFilletBranch {
                    constraint: ids.constraint,
                    first_side: DocumentCurveNormalSide::Right,
                    first_trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    second_side: DocumentCurveNormalSide::Right,
                    second_trim_endpoint: DocumentFilletTrimEndpoint::Start,
                    endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            ))
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.history_len(), history);
    assert_eq!(session.export_json().unwrap(), before);
}

#[test]
fn periodic_anchor_uses_unwrapped_local_root_and_singular_offsets_reject() {
    let fixture = line_circle_fixture(true);
    let mut missing_anchor = line_circle_request(&fixture, 0);
    missing_anchor.first.periodic_anchor = None;
    let before = fixture.document.to_canonical_json().unwrap();
    let mut rejected = fixture.document.clone();
    assert!(
        rejected
            .add_curve_curve_fillet("missing anchor", missing_anchor)
            .is_err()
    );
    assert_eq!(rejected.to_canonical_json().unwrap(), before);

    let mut singular = line_circle_request(&fixture, 0);
    singular.first.side = DocumentCurveNormalSide::Left;
    singular.radius = 2.0;
    let mut rejected = fixture.document.clone();
    assert!(
        rejected
            .add_curve_curve_fillet("singular offset", singular)
            .is_err()
    );
    assert_eq!(rejected.to_canonical_json().unwrap(), before);

    let wound_request = line_circle_request(&fixture, 1);
    let mut periodic = fixture.document;
    let ids = periodic
        .add_curve_curve_fillet("wound generic", wound_request)
        .unwrap();
    let session = SketchDocumentSession::new(
        periodic,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let contact = session.document().contact(ids.contacts[0]).unwrap();
    assert_eq!(contact.winding, 1);
    assert!(matches!(
        contact.neighborhood,
        ContactNeighborhood::Local { lower, upper }
            if lower < std::f64::consts::TAU && std::f64::consts::TAU < upper
    ));
    let interval = session
        .document()
        .visible_interval(CurveSpan::line(fixture.circle))
        .unwrap();
    assert!((interval.start - std::f64::consts::PI).abs() <= 1.0e-12);
    assert!((interval.end - std::f64::consts::TAU).abs() <= 1.0e-12);
}

#[test]
fn associated_arc_consumers_differentiate_both_angles_after_parent_edit() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let circle_center = sketch.add_point(Point2::origin()).unwrap();
    let line_start = sketch.add_point(Point2::new(0.0, 1.0)).unwrap();
    let line_end = sketch.add_point(Point2::new(6.0, 1.0)).unwrap();
    let fillet_center = sketch.add_point(Point2::new(3.0, 0.0)).unwrap();
    let generic_angle = 5.0 * std::f64::consts::PI / 8.0;
    let generic_point = sketch
        .add_point(Point2::new(3.0 + generic_angle.cos(), generic_angle.sin()))
        .unwrap();
    let midpoint = sketch
        .add_point(Point2::new(
            2.0_f64.sqrt().mul_add(-0.5, 3.0),
            0.5_f64.sqrt(),
        ))
        .unwrap();
    let circle = sketch.add_circle(circle_center, 2.0).unwrap();
    let line = sketch.add_segment(line_start, line_end).unwrap();
    sketch.add_fixed_point(circle_center).unwrap();
    sketch.add_fixed_point(line_start).unwrap();
    sketch
        .add_circle_radius(circle, 2.0, DimensionMode::Driving)
        .unwrap();
    let arc = sketch
        .add_arc(
            fillet_center,
            1.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let fillet = sketch
        .add_curve_curve_fillet(
            arc,
            SketchCurveContact {
                curve: SketchCurve::Circle(circle),
                parameter: 0.0,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: -0.4,
                    upper: 0.4,
                },
            },
            CurveNormalSide::Right,
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: line,
                    domain: LineParameterDomain::SupportingLine,
                },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.2,
                    upper: 0.8,
                },
            },
            CurveNormalSide::Right,
            FilletEndpointOrder::SecondThenFirst,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, 1.0, DimensionMode::Driving)
        .unwrap();
    let comparison_circle = sketch.add_circle(fillet_center, 1.0).unwrap();
    let arc_contact = SketchCurveContact {
        curve: SketchCurve::Arc(arc),
        parameter: 0.25,
        neighborhood: CurveContactNeighborhood::Interior,
    };
    let comparison_contact = SketchCurveContact {
        curve: SketchCurve::Circle(comparison_circle),
        parameter: generic_angle,
        neighborhood: CurveContactNeighborhood::Local {
            lower: generic_angle - 0.2,
            upper: generic_angle + 0.2,
        },
    };
    let generic_consumer = sketch
        .add_point_on_curve(generic_point, arc_contact)
        .unwrap();
    let native_consumer = sketch.add_point_on_arc(midpoint, arc, 0.5).unwrap();
    let tangent_consumer = sketch
        .add_curve_curve_tangency(
            arc_contact,
            comparison_contact,
            CurveTangentOrientation::Aligned,
        )
        .unwrap();
    let curvature_consumer = sketch
        .add_equal_curvature(
            arc_contact,
            comparison_contact,
            CurveCurvatureRelation::Signed,
        )
        .unwrap();

    let request = SketchSolveRequest::default();
    let compiled = sketch.compile(request).unwrap();
    assert_eq!(
        compiled
            .arc_angle_variables()
            .iter()
            .map(|mapping| mapping.role)
            .collect::<Vec<_>>(),
        vec![ArcAngleRole::Start, ArcAngleRole::End]
    );
    let start_angle = compiled
        .variable_for_arc_angle(arc, ArcAngleRole::Start)
        .unwrap();
    let end_angle = compiled
        .variable_for_arc_angle(arc, ArcAngleRole::End)
        .unwrap();
    let fillet_source = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(fillet))
        .unwrap();
    let fillet_rows = compiled
        .problem()
        .audit_rows()
        .unwrap()
        .into_iter()
        .filter(|row| Some(row.source_id) == fillet_source.core_source_id)
        .collect::<Vec<_>>();
    assert_eq!(fillet_rows.len(), 6);
    assert_eq!(fillet_rows[4].scale.to_bits(), 1.0f64.to_bits());
    assert_eq!(fillet_rows[5].scale.to_bits(), 1.0f64.to_bits());
    assert!(
        fillet_rows[4]
            .template
            .contains("output_radial(start_angle)")
    );
    assert!(fillet_rows[5].template.contains("output_radial(end_angle)"));
    for constraint in [
        fillet,
        generic_consumer,
        native_consumer,
        tangent_consumer,
        curvature_consumer,
    ] {
        let source = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == SketchSource::Constraint(constraint))
            .unwrap();
        let residual = compiled.problem().residual(source.residual_ids[0]).unwrap();
        assert!(residual.incident_variables().contains(&start_angle));
        assert!(residual.incident_variables().contains(&end_angle));
    }
    let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(
        jacobians.blocks.iter().all(|block| {
            block.max_relative_error <= 2.0e-6 || block.max_absolute_error <= 1.0e-8
        }),
        "initial max FD error={:e}: {jacobians:#?}",
        jacobians.max_relative_error()
    );

    sketch.remove_constraint(tangent_consumer).unwrap();
    sketch.remove_constraint(curvature_consumer).unwrap();
    let mut session = SketchSession::new(sketch, request, SolverConfig::default()).unwrap();
    let before = session.accepted_result().geometry.arc(arc).unwrap();
    let before_start = before.endpoints().unwrap().0;
    let edited = session
        .apply_patch(SketchSessionPatch::new(
            session.revision(),
            SketchPatch::PointPosition {
                point: line_end,
                position: Point2::new(6.0, 2.0),
            },
        ))
        .unwrap();
    assert!(edited.accepted(), "{:#?}", edited.rejection);
    assert_eq!(session.topology_compilations(), 1);
    let solved_arc = edited.geometry.arc(arc).unwrap();
    let after_start = solved_arc.endpoints().unwrap().0;
    assert!((after_start - before_start).norm() > 1.0e-4);
    let ContactState::PointOnCurve {
        parameter: generic_parameter,
    } = session.sketch().contact_state(generic_consumer).unwrap()
    else {
        panic!("generic consumer lost its contact state")
    };
    assert!(
        (edited.geometry.point(generic_point).unwrap()
            - solved_arc.evaluate(generic_parameter).unwrap())
        .norm()
            <= 1.0e-9
    );
    let ContactState::PointOnArc { span_parameter } =
        session.sketch().contact_state(native_consumer).unwrap()
    else {
        panic!("native consumer lost its contact state")
    };
    assert!(
        (edited.geometry.point(midpoint).unwrap() - solved_arc.evaluate(span_parameter).unwrap())
            .norm()
            <= 1.0e-9
    );

    let recompiled = session.sketch().compile(request).unwrap();
    assert_eq!(
        recompiled.arc_angle_variables(),
        compiled.arc_angle_variables()
    );
    let jacobians = recompiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(
        jacobians.blocks.iter().all(|block| {
            block.max_relative_error <= 2.0e-6 || block.max_absolute_error <= 1.0e-8
        }),
        "edited max FD error={:e}: {jacobians:#?}",
        jacobians.max_relative_error()
    );
}
