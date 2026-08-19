// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveNormalSide,
    DocumentCurveTrimView, DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
    DocumentError, DocumentFilletEndpointOrder, DocumentNativeLineFilletCreationRequest,
    DocumentNativeLineFilletIds, DocumentNativeLineFilletParent, DocumentObjectId,
    DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter, FeatureEndpoint,
    GeometryRole, OperationControl, OperationLimits, OperationOutcome, PreparedSketchOperation,
    RetainedSketchDocumentSession, SketchDocument, SketchDocumentSession,
    SketchPersistentIdentityHighWater, TangentOrientation,
};

struct NativeFilletFixture {
    document: SketchDocument,
    request: DocumentNativeLineFilletCreationRequest,
    first_line: geosolve_sketch::CurveId,
    second_line: geosolve_sketch::CurveId,
    first_outer: geosolve_sketch::DesignPointId,
    corner: geosolve_sketch::DesignPointId,
    second_outer: geosolve_sketch::DesignPointId,
}

fn native_fillet_fixture() -> NativeFilletFixture {
    let mut document = SketchDocument::new(4.0).unwrap();
    let first_outer = document.add_point("first outer", [-3.0, 0.0]).unwrap();
    let corner = document.add_point("sharp corner", [0.0, 0.0]).unwrap();
    let second_outer = document.add_point("second outer", [0.0, 3.0]).unwrap();
    let first_line = document
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start: first_outer,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second_line = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: corner,
                end: second_outer,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let request = DocumentNativeLineFilletCreationRequest {
        label: "native corner".into(),
        first: DocumentNativeLineFilletParent {
            curve: CurveSpan::line(first_line),
            endpoint: FeatureEndpoint::End,
            normal_side: DocumentCurveNormalSide::Left,
            tangent_orientation: TangentOrientation::Aligned,
            contact_position: [-1.0, 0.0],
        },
        second: DocumentNativeLineFilletParent {
            curve: CurveSpan::line(second_line),
            endpoint: FeatureEndpoint::Start,
            normal_side: DocumentCurveNormalSide::Left,
            tangent_orientation: TangentOrientation::Aligned,
            contact_position: [0.0, 1.0],
        },
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        center: [-1.0, 1.0],
        radius: 1.0,
        start_angle: -std::f64::consts::FRAC_PI_2,
        end_angle: 0.0,
        sweep: DocumentArcSweep::CounterClockwise,
    };
    NativeFilletFixture {
        document,
        request,
        first_line,
        second_line,
        first_outer,
        corner,
        second_outer,
    }
}

fn assert_f64_bits(actual: f64, expected: f64) {
    assert_eq!(actual.to_bits(), expected.to_bits());
}

fn assert_pair_bits(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

fn assert_native_objects_absent(document: &SketchDocument, ids: &DocumentNativeLineFilletIds) {
    assert!(document.point(ids.removed_corner).is_some());
    assert!(
        ids.contact_points
            .iter()
            .all(|point| document.point(*point).is_none())
    );
    assert!(document.point(ids.center).is_none());
    assert!(document.curve(ids.arc).is_none());
    assert!(document.scalar(ids.radius).is_none());
    assert!(document.scalar(ids.start_angle).is_none());
    assert!(document.scalar(ids.end_angle).is_none());
    assert!(
        ids.contacts
            .iter()
            .all(|contact| document.contact(*contact).is_none())
    );
    assert!(
        ids.tangencies
            .iter()
            .all(|constraint| document.constraint(*constraint).is_none())
    );
    assert!(document.dimension(ids.radius_dimension).is_none());
    assert!(document.scalar(ids.radius_target).is_none());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owner regression keeps native identities, topology, branches, and independent acceptance together"
)]
fn native_line_fillet_materializes_exact_ordinary_topology_and_solves() {
    let fixture = native_fillet_fixture();
    let mut document = fixture.document;
    let prepared = document
        .prepare_native_line_fillet_geometry(fixture.request.clone())
        .unwrap();
    let expected = prepared.expected_ids().clone();
    let ids = document
        .create_prepared_native_line_fillet_geometry(prepared)
        .unwrap();

    assert_eq!(ids, expected);
    assert_eq!(ids.source_lines, [fixture.first_line, fixture.second_line]);
    assert!(document.point(fixture.corner).is_none());
    assert_pair_bits(
        document.point(fixture.first_outer).unwrap().position,
        [-3.0, 0.0],
    );
    assert_pair_bits(
        document.point(fixture.second_outer).unwrap().position,
        [0.0, 3.0],
    );
    let CurveDefinition::Line {
        start,
        end,
        branch_direction,
    } = document
        .curve(fixture.first_line)
        .unwrap()
        .definition
        .clone()
    else {
        panic!("first native Fillet parent must remain a Line");
    };
    assert_eq!((start, end), (fixture.first_outer, ids.contact_points[0]));
    assert_pair_bits(branch_direction, [1.0, 0.0]);
    let CurveDefinition::Line {
        start,
        end,
        branch_direction,
    } = document
        .curve(fixture.second_line)
        .unwrap()
        .definition
        .clone()
    else {
        panic!("second native Fillet parent must remain a Line");
    };
    assert_eq!((start, end), (ids.contact_points[1], fixture.second_outer));
    assert_pair_bits(branch_direction, [0.0, 1.0]);
    assert_pair_bits(
        document.point(ids.contact_points[0]).unwrap().position,
        [-1.0, 0.0],
    );
    assert_pair_bits(
        document.point(ids.contact_points[1]).unwrap().position,
        [0.0, 1.0],
    );
    assert_pair_bits(document.point(ids.center).unwrap().position, [-1.0, 1.0]);
    assert_eq!(document.geometry_role(ids.arc), Some(GeometryRole::Profile));
    let CurveDefinition::CircularArc {
        center,
        radius,
        start_angle,
        end_angle,
        sweep,
    } = document.curve(ids.arc).unwrap().definition.clone()
    else {
        panic!("native Fillet must publish one ordinary CircularArc");
    };
    assert_eq!(
        (center, radius, start_angle, end_angle, sweep),
        (
            ids.center,
            ids.radius,
            ids.start_angle,
            ids.end_angle,
            DocumentArcSweep::CounterClockwise,
        )
    );
    assert_f64_bits(document.scalar(ids.radius).unwrap().value, 1.0);
    assert_f64_bits(
        document.scalar(ids.start_angle).unwrap().value,
        -std::f64::consts::FRAC_PI_2,
    );
    assert_f64_bits(document.scalar(ids.end_angle).unwrap().value, 0.0);

    for (index, (parameter, neighborhood)) in [
        (0.0, ContactNeighborhood::Start),
        (1.0, ContactNeighborhood::End),
    ]
    .into_iter()
    .enumerate()
    {
        let contact = document.contact(ids.contacts[index]).unwrap();
        assert_eq!(contact.curve, CurveSpan::line(ids.arc));
        assert_eq!(contact.parameter, ids.contact_parameters[index]);
        let ContactDomain::Bounded { lower, upper } = contact.domain else {
            panic!("native Fillet arc contact must be exactly bounded");
        };
        assert_f64_bits(lower, 0.0);
        assert_f64_bits(upper, 1.0);
        assert_eq!(contact.winding, 0);
        assert!(matches!(
            (contact.neighborhood, neighborhood),
            (ContactNeighborhood::Start, ContactNeighborhood::Start)
                | (ContactNeighborhood::End, ContactNeighborhood::End)
        ));
        assert_eq!(
            contact.tangent_orientation,
            Some(TangentOrientation::Aligned)
        );
        assert_f64_bits(
            document
                .scalar(ids.contact_parameters[index])
                .unwrap()
                .value,
            parameter,
        );
    }
    assert_eq!(
        document.constraint(ids.tangencies[0]).unwrap().definition,
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(fixture.first_line),
            endpoint: FeatureEndpoint::End,
            curve_contact: ids.contacts[0],
        }
    );
    assert_eq!(
        document.constraint(ids.tangencies[1]).unwrap().definition,
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(fixture.second_line),
            endpoint: FeatureEndpoint::Start,
            curve_contact: ids.contacts[1],
        }
    );
    let dimension = document.dimension(ids.radius_dimension).unwrap();
    assert_eq!(dimension.mode, DocumentDimensionMode::Driving);
    let DocumentDimensionDefinition::Radius { curve, target } = dimension.definition.clone() else {
        panic!("native Fillet radius owner must be an ordinary Radius dimension");
    };
    assert_eq!((curve, target), (ids.arc, ids.radius_target));
    assert_f64_bits(document.scalar(ids.radius_target).unwrap().value, 1.0);

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = session.accepted_result();
    let report = result.accepted_view().unstable_core_report();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    assert!(
        result
            .accepted_view()
            .acceptance_hard_residual_max
            .is_some_and(|maximum| maximum <= 1.0e-9)
    );
    assert!(
        session
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        session
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
}

#[test]
fn native_line_fillet_preserves_clockwise_second_first_opposed_gauge() {
    let fixture = native_fillet_fixture();
    let mut document = fixture.document;
    let mut request = fixture.request;
    request.endpoint_order = DocumentFilletEndpointOrder::SecondThenFirst;
    request.start_angle = 0.0;
    request.end_angle = -std::f64::consts::FRAC_PI_2;
    request.sweep = DocumentArcSweep::Clockwise;
    request.first.tangent_orientation = TangentOrientation::Opposed;
    request.second.tangent_orientation = TangentOrientation::Opposed;

    let ids = document
        .create_native_line_fillet_geometry(request)
        .expect("reversed native Fillet gauge");
    for (index, (parameter, neighborhood)) in [
        (1.0, ContactNeighborhood::End),
        (0.0, ContactNeighborhood::Start),
    ]
    .into_iter()
    .enumerate()
    {
        assert_f64_bits(
            document
                .scalar(ids.contact_parameters[index])
                .unwrap()
                .value,
            parameter,
        );
        let contact = document.contact(ids.contacts[index]).unwrap();
        assert_eq!(contact.neighborhood, neighborhood);
        assert_eq!(
            contact.tangent_orientation,
            Some(TangentOrientation::Opposed)
        );
    }
    let CurveDefinition::CircularArc {
        start_angle,
        end_angle,
        sweep,
        ..
    } = document.curve(ids.arc).unwrap().definition
    else {
        panic!("native Fillet must remain an ordinary CircularArc");
    };
    assert_f64_bits(document.scalar(start_angle).unwrap().value, 0.0);
    assert_f64_bits(
        document.scalar(end_angle).unwrap().value,
        -std::f64::consts::FRAC_PI_2,
    );
    assert_eq!(sweep, DocumentArcSweep::Clockwise);

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .expect("accepted reversed-gauge native Fillet");
    let result = session.accepted_result();
    let report = result.accepted_view().unstable_core_report();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the rejection matrix keeps all malformed and excluded native Fillet requests under one exact state-neutrality contract"
)]
fn invalid_or_ineligible_native_fillet_requests_reject_without_mutation() {
    let fixture = native_fillet_fixture();

    let mut dependent = fixture.document.clone();
    dependent
        .add_constraint(
            "corner dependency",
            DocumentConstraintDefinition::FixedPoint {
                point: fixture.corner,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let before = dependent.clone();
    let error = dependent
        .create_native_line_fillet_geometry(fixture.request.clone())
        .expect_err("a point-based dependent must reject native publication");
    assert!(matches!(
        error,
        DocumentError::InvalidField {
            field: "native fillet corner",
            ref message,
        } if message == "shared corner must be owned only by the two selected source lines"
    ));
    assert_eq!(dependent, before);

    let mut high_valence = fixture.document.clone();
    let branch_end = high_valence.add_point("branch end", [2.0, 2.0]).unwrap();
    high_valence
        .add_curve(
            "third corner owner",
            CurveDefinition::Line {
                start: fixture.corner,
                end: branch_end,
                branch_direction: [std::f64::consts::FRAC_1_SQRT_2; 2],
            },
        )
        .unwrap();
    let before = high_valence.clone();
    let error = high_valence
        .create_native_line_fillet_geometry(fixture.request.clone())
        .expect_err("a third incident line must reject native publication");
    assert!(matches!(
        error,
        DocumentError::InvalidField {
            field: "native fillet corner",
            ref message,
        } if message == "shared corner must be owned only by the two selected source lines"
    ));
    assert_eq!(high_valence, before);

    let mut construction = fixture.document.clone();
    construction
        .set_geometry_role(fixture.first_line, GeometryRole::Construction)
        .unwrap();
    let before = construction.clone();
    assert!(
        construction
            .create_native_line_fillet_geometry(fixture.request.clone())
            .is_err()
    );
    assert_eq!(construction, before);

    let mut trimmed = fixture.document.clone();
    trimmed
        .replace_trim_views(
            CurveSpan::line(fixture.first_line),
            vec![DocumentCurveTrimView {
                support: CurveSpan::line(fixture.first_line),
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.1,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.9,
                    winding: 0,
                }),
            }],
        )
        .unwrap();
    let before = trimmed.clone();
    assert!(
        trimmed
            .create_native_line_fillet_geometry(fixture.request.clone())
            .is_err()
    );
    assert_eq!(trimmed, before);

    let mut invalid_request = fixture.request.clone();
    invalid_request.radius = f64::NAN;
    let mut invalid = fixture.document.clone();
    let before = invalid.clone();
    assert!(
        invalid
            .create_native_line_fillet_geometry(invalid_request)
            .is_err()
    );
    assert_eq!(invalid, before);

    let mut wrong_branch_request = fixture.request.clone();
    wrong_branch_request.first.tangent_orientation = TangentOrientation::Opposed;
    let mut wrong_branch = fixture.document.clone();
    let before = wrong_branch.clone();
    assert!(
        wrong_branch
            .create_native_line_fillet_geometry(wrong_branch_request)
            .is_err()
    );
    assert_eq!(wrong_branch, before);

    let mut exhausted = fixture.document;
    let mut high_water = serde_json::to_value(exhausted.persistent_identity_high_water()).unwrap();
    high_water["next_id"] = serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
    let high_water: SketchPersistentIdentityHighWater = serde_json::from_value(high_water).unwrap();
    exhausted
        .retain_persistent_identity_high_water(&high_water)
        .unwrap();
    let before = exhausted.clone();
    assert!(matches!(
        exhausted.create_native_line_fillet_geometry(fixture.request),
        Err(geosolve_sketch::DocumentError::IdExhausted)
    ));
    assert_eq!(exhausted, before);
}

#[test]
fn controlled_native_fillet_preparation_exhaustion_is_state_neutral() {
    let fixture = native_fillet_fixture();
    let document_before = fixture.document.clone();
    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 0;
    let preparation = fixture
        .document
        .prepare_native_line_fillet_geometry_controlled(
            fixture.request.clone(),
            OperationControl::new(geosolve_core::CancellationToken::default(), limits),
        )
        .unwrap();
    assert!(matches!(
        preparation,
        OperationOutcome::WorkExhausted { .. }
    ));
    assert_eq!(fixture.document, document_before);

    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let prepared = session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .prepare_native_line_fillet_geometry(fixture.request)
        .unwrap();
    let input = session.prepared_input();
    let design = session.export_design_json().unwrap();
    let accepted = session.export_accepted_json().unwrap();
    let high_water = session.persistent_identity_high_water().clone();
    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 0;

    let outcome = session
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::Apply(
            DocumentEdit::CreatePreparedNativeLineFilletGeometry {
                prepared: Box::new(prepared),
            },
        ))
        .execute(OperationControl::new(
            geosolve_core::CancellationToken::default(),
            limits,
        ))
        .unwrap();
    assert!(matches!(outcome, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), input);
    assert_eq!(session.export_design_json().unwrap(), design);
    assert_eq!(session.export_accepted_json().unwrap(), accepted);
    assert_eq!(session.persistent_identity_high_water(), &high_water);
}

#[test]
fn stale_prepared_source_rejects_atomically() {
    let fixture = native_fillet_fixture();
    let mut document = fixture.document;
    let prepared = document
        .prepare_native_line_fillet_geometry(fixture.request)
        .unwrap();
    document
        .set_curve_branch(CurveSpan::line(fixture.first_line), [1.0, 0.125])
        .unwrap();
    let before = document.clone();
    assert!(
        document
            .create_prepared_native_line_fillet_geometry(prepared)
            .is_err()
    );
    assert_eq!(document, before);
}

#[test]
fn native_fillet_is_one_history_step_and_undo_redo_preserve_identities() {
    let fixture = native_fillet_fixture();
    let mut session = SketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let original_first = session
        .document()
        .curve(fixture.first_line)
        .unwrap()
        .clone();
    let original_second = session
        .document()
        .curve(fixture.second_line)
        .unwrap()
        .clone();
    let prepared = session
        .document()
        .prepare_native_line_fillet_geometry(fixture.request)
        .unwrap();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePreparedNativeLineFilletGeometry {
                prepared: Box::new(prepared),
            },
        ))
        .unwrap();
    assert!(
        outcome.accepted(),
        "{:#?}",
        outcome.result.solve().rejection
    );
    let Some(DocumentCommandEffect::CreatedNativeLineFillet(ids)) = outcome.effect else {
        panic!("native Fillet creation effect expected");
    };
    let ids = *ids;
    assert_eq!(session.history_len(), 1);
    assert_eq!(session.history_cursor(), 1);
    assert!(session.document().curve(ids.arc).is_some());

    assert!(session.undo(session.revision()).unwrap().accepted());
    assert_eq!(session.history_len(), 1);
    assert_eq!(session.history_cursor(), 0);
    assert_eq!(
        session.document().curve(fixture.first_line).unwrap(),
        &original_first
    );
    assert_eq!(
        session.document().curve(fixture.second_line).unwrap(),
        &original_second
    );
    assert_native_objects_absent(session.document(), &ids);

    assert!(session.redo(session.revision()).unwrap().accepted());
    assert_eq!(session.history_len(), 1);
    assert_eq!(session.history_cursor(), 1);
    assert!(session.document().curve(ids.arc).is_some());
    assert!(session.document().point(ids.removed_corner).is_none());
    assert_eq!(
        session
            .document()
            .constraint(ids.tangencies[0])
            .unwrap()
            .definition,
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(fixture.first_line),
            endpoint: FeatureEndpoint::End,
            curve_contact: ids.contacts[0],
        }
    );

    session.undo(session.revision()).unwrap();
    let replacement = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "post-undo replacement".into(),
                position: [4.0, 4.0],
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedPoint(replacement)) = replacement.effect else {
        panic!("replacement point effect expected");
    };
    let highest_native_point = ids
        .contact_points
        .into_iter()
        .chain([ids.center])
        .map(|point| point.0.as_u128())
        .max()
        .unwrap();
    assert!(replacement.0.as_u128() > highest_native_point);
    assert!(!session.can_redo());

    // The sharp corner is ordinary again and may be removed only after its two restored lines.
    assert!(matches!(
        session
            .document()
            .clone()
            .remove(DocumentObjectId::Point(fixture.corner)),
        Err(geosolve_sketch::DocumentError::ObjectInUse(_))
    ));
}

#[test]
fn native_fillet_radius_and_remote_endpoint_remain_ordinary_editable_state() {
    let fixture = native_fillet_fixture();
    let mut session = SketchDocumentSession::new(
        fixture.document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let prepared = session
        .document()
        .prepare_native_line_fillet_geometry(fixture.request)
        .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePreparedNativeLineFilletGeometry {
                prepared: Box::new(prepared),
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedNativeLineFillet(ids)) = created.effect else {
        panic!("native Fillet creation effect expected");
    };
    let ids = *ids;

    let radius_edit = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.radius_target,
                value: 0.75,
            },
        ))
        .unwrap();
    assert!(
        radius_edit.accepted(),
        "{:#?}",
        radius_edit.result.solve().rejection
    );
    assert!((session.document().scalar(ids.radius).unwrap().value - 0.75).abs() <= 1.0e-9);

    let endpoint_edit = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: fixture.first_outer,
                position: [-4.0, 0.0],
            },
        ))
        .unwrap();
    assert!(
        endpoint_edit.accepted(),
        "{:#?}",
        endpoint_edit.result.solve().rejection
    );
    let endpoint = session
        .document()
        .point(fixture.first_outer)
        .unwrap()
        .position;
    assert!((endpoint[0] + 4.0).hypot(endpoint[1]) <= 1.0e-9);
    assert!(session.document().curve(ids.arc).is_some());
    assert!(session.document().dimension(ids.radius_dimension).is_some());
    let result = session.accepted_result();
    let report = result.accepted_view().unstable_core_report();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
}
