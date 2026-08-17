// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    CoordinatorError, CurveNumericPropertyKind, CurvePropertyFamily, RetainedEditorCoordinator,
    SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentConstraintDefinition,
    DocumentCurveControlAvailability, DocumentCurveControlWithholdingReason,
    DocumentCurveNormalSide, DocumentDimensionDefinition, DocumentDimensionMode, DocumentElementId,
    DocumentFilletEndpointOrder, DocumentHyperbolaBranch, DocumentParameterKind,
    DocumentParameterTarget, DocumentRationalConicControl, DocumentScalarBranch,
    DocumentScalarPropertyRef, DocumentScalarUnit, DocumentSolveRequest, LineLineFilletRequest,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};

fn assert_curve_property_rejection_is_atomic<T>(
    coordinator: &mut RetainedEditorCoordinator,
    action: impl FnOnce(&mut RetainedEditorCoordinator) -> Result<T, CoordinatorError>,
) -> CoordinatorError {
    let lifecycle = coordinator.lifecycle();
    let design_json = coordinator
        .session()
        .design_document()
        .to_draft_v5_json()
        .unwrap();
    let accepted = coordinator.session().accepted_state().map(|state| {
        (
            state.identity(),
            state.input(),
            state.document().to_draft_v5_json().unwrap(),
        )
    });
    let history = (coordinator.history_len(), coordinator.history_cursor());
    let checkpoint = coordinator.checkpoint().clone();

    let Err(error) = action(coordinator) else {
        panic!("withheld curve property action unexpectedly mutated the document");
    };

    assert_eq!(coordinator.lifecycle(), lifecycle);
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .unwrap(),
        design_json
    );
    assert_eq!(
        coordinator.session().accepted_state().map(|state| (
            state.identity(),
            state.input(),
            state.document().to_draft_v5_json().unwrap(),
        )),
        accepted
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history
    );
    let retained = coordinator.checkpoint();
    assert_eq!(retained.design_json(), checkpoint.design_json());
    assert_eq!(
        retained.design_uses_draft_v5(),
        checkpoint.design_uses_draft_v5()
    );
    assert_eq!(retained.accepted_json(), checkpoint.accepted_json());
    assert_eq!(
        retained.accepted_uses_draft_v5(),
        checkpoint.accepted_uses_draft_v5()
    );
    assert_eq!(
        retained.accepted_belongs_to_current_design(),
        checkpoint.accepted_belongs_to_current_design()
    );
    assert_eq!(retained.revisions(), checkpoint.revisions());
    assert_eq!(
        retained.sketch_identity_high_water(),
        checkpoint.sketch_identity_high_water()
    );
    assert_eq!(retained.feature_json(), checkpoint.feature_json());
    assert_eq!(
        retained.feature_lifecycle_high_water(),
        checkpoint.feature_lifecycle_high_water()
    );
    assert_eq!(
        retained.computed_evaluation_high_water(),
        checkpoint.computed_evaluation_high_water()
    );
    error
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one rational-control history scenario covers nonzero and projective inspector transitions"
)]
fn selected_curve_properties_edit_rational_weight_without_moving_p1() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let weight = document
        .add_scalar(
            "weight",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .unwrap();
    let curve = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [1.0, 1.5],
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);

    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(metadata.family, CurvePropertyFamily::RationalQuadraticConic);
    assert_eq!(metadata.numeric.len(), 1);
    assert_eq!(
        metadata.numeric[0].kind,
        CurveNumericPropertyKind::RationalWeight
    );
    assert_eq!(
        metadata.rational_control,
        Some(DocumentRationalConicControl::Euclidean {
            middle: [2.0, 3.0],
            weight: 0.5,
        })
    );

    let before_history = coordinator.history_len();
    coordinator
        .set_curve_numeric_property(
            coordinator.session().design_identity(),
            curve,
            CurveNumericPropertyKind::RationalWeight,
            0.8,
        )
        .unwrap();
    assert_eq!(coordinator.history_len(), before_history + 1);
    let DocumentRationalConicControl::Euclidean { middle, weight } = coordinator
        .session()
        .design_document()
        .rational_conic_control(curve)
        .unwrap()
    else {
        panic!("expected Euclidean rational control");
    };
    assert!((middle[0] - 2.0).abs() <= 1.0e-14);
    assert!((middle[1] - 3.0).abs() <= 1.0e-14);
    assert_eq!(weight.to_bits(), 0.8_f64.to_bits());
    coordinator.undo().unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [2.0, 3.0],
            weight: 0.5,
        }
    );

    coordinator
        .set_curve_rational_middle(coordinator.session().design_identity(), curve, [3.0, 4.0])
        .unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [3.0, 4.0],
            weight: 0.5,
        }
    );
    coordinator.undo().unwrap();

    coordinator
        .set_curve_numeric_property(
            coordinator.session().design_identity(),
            curve,
            CurveNumericPropertyKind::RationalWeight,
            0.0,
        )
        .unwrap();
    coordinator
        .set_curve_rational_middle(coordinator.session().design_identity(), curve, [5.0, 6.0])
        .unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .unwrap(),
        DocumentRationalConicControl::Projective {
            weighted_middle: [5.0, 6.0],
            weight: 0.0,
        }
    );
    coordinator.undo().unwrap();
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .unwrap(),
        DocumentRationalConicControl::Projective {
            weighted_middle: [1.0, 1.5],
            weight: 0.0,
        }
    );
}

#[test]
fn host_bound_numeric_property_is_read_only_and_rejects_without_state_change() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let domain = ScalarDomain::Bounded {
        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    };
    let weight = document
        .add_scalar("weight", 0.5, ScalarUnit::Parameter, domain)
        .unwrap();
    let curve = document
        .add_curve(
            "host-owned rational conic",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [1.0, 1.5],
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    let parameter = document
        .add_parameter("weight", DocumentParameterKind::Dimensionless)
        .unwrap();
    document
        .add_parameter_binding(
            parameter,
            DocumentParameterTarget::DimensionlessFixedScalar(DocumentScalarPropertyRef {
                scalar: weight,
                unit: DocumentScalarUnit::Dimensionless,
                domain,
                branch: DocumentScalarBranch::Dimensionless,
            }),
        )
        .unwrap();
    let batch = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Dimensionless(0.5),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);

    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(metadata.numeric.len(), 1);
    assert_eq!(
        metadata.direct_edit_availability,
        DocumentCurveControlAvailability::Editable,
        "host ownership of weight must not withhold the independent Qh/P1 coordinate edit",
    );
    assert_eq!(
        metadata.numeric[0].availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::HostParameterOwned,
        )
    );
    let expected = coordinator.session().design_identity();
    let error = assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
        coordinator.set_curve_numeric_property(
            expected,
            curve,
            CurveNumericPropertyKind::RationalWeight,
            0.75,
        )
    });
    assert!(matches!(
        error,
        CoordinatorError::CurvePropertyUnavailable(
            DocumentCurveControlWithholdingReason::HostParameterOwned
        )
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ownership matrix keeps canvas and inspector radius availability aligned"
)]
fn radius_property_respects_driving_dimension_and_equal_radius_ownership() {
    let mut dimensioned = SketchDocument::new(4.0).unwrap();
    let center = dimensioned.add_point("center", [0.0, 0.0]).unwrap();
    let radius = dimensioned
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = dimensioned
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let target = dimensioned
        .add_scalar(
            "driving target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    dimensioned
        .add_dimension(
            "driving radius",
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        dimensioned,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(
        metadata.numeric[0].availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned,
        )
    );
    let expected = coordinator.session().design_identity();
    let error = assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
        coordinator.set_curve_numeric_property(
            expected,
            circle,
            CurveNumericPropertyKind::Radius,
            3.0,
        )
    });
    assert!(matches!(
        error,
        CoordinatorError::CurvePropertyUnavailable(
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned
        )
    ));

    let mut diameter_dimensioned = SketchDocument::new(4.0).unwrap();
    let center = diameter_dimensioned
        .add_point("diameter center", [0.0, 0.0])
        .unwrap();
    let radius = diameter_dimensioned
        .add_scalar(
            "diameter-owned radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = diameter_dimensioned
        .add_curve(
            "diameter circle",
            CurveDefinition::Circle { center, radius },
        )
        .unwrap();
    let target = diameter_dimensioned
        .add_scalar(
            "driving diameter target",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    diameter_dimensioned
        .add_dimension(
            "driving diameter",
            DocumentDimensionDefinition::Diameter {
                curve: circle,
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        diameter_dimensioned,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(circle))]);
    assert_eq!(
        coordinator
            .selected_curve_property_metadata()
            .unwrap()
            .numeric[0]
            .availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned,
        )
    );
    let expected = coordinator.session().design_identity();
    let error = assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
        coordinator.set_curve_numeric_property(
            expected,
            circle,
            CurveNumericPropertyKind::Radius,
            3.0,
        )
    });
    assert!(matches!(
        error,
        CoordinatorError::CurvePropertyUnavailable(
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned
        )
    ));

    let mut related = SketchDocument::new(4.0).unwrap();
    let first_center = related.add_point("first center", [0.0, 0.0]).unwrap();
    let first_radius = related
        .add_scalar(
            "first radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let first = related
        .add_curve(
            "first circle",
            CurveDefinition::Circle {
                center: first_center,
                radius: first_radius,
            },
        )
        .unwrap();
    let second_center = related.add_point("second center", [5.0, 0.0]).unwrap();
    let second_radius = related
        .add_scalar(
            "second radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let second = related
        .add_curve(
            "second circle",
            CurveDefinition::Circle {
                center: second_center,
                radius: second_radius,
            },
        )
        .unwrap();
    related
        .add_constraint(
            "equal radii",
            DocumentConstraintDefinition::EqualRadius { first, second },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        related,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(second))]);
    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(
        metadata.numeric[0].availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::EqualRadiusOwned,
        )
    );
    let expected = coordinator.session().design_identity();
    let error = assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
        coordinator.set_curve_numeric_property(
            expected,
            second,
            CurveNumericPropertyKind::Radius,
            3.0,
        )
    });
    assert!(matches!(
        error,
        CoordinatorError::CurvePropertyUnavailable(
            DocumentCurveControlWithholdingReason::EqualRadiusOwned
        )
    ));
}

#[test]
fn inactive_curve_numeric_and_branch_actions_are_truthfully_withheld() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let transverse = document.add_point("transverse", [2.0, 0.0]).unwrap();
    let semi_conjugate = document
        .add_scalar("conjugate", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let trim_start = document
        .add_scalar("start", -0.5, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let trim_end = document
        .add_scalar("end", 0.5, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let curve = document
        .add_curve(
            "inactive hyperbola",
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point: transverse,
                semi_conjugate,
                branch: DocumentHyperbolaBranch::Positive,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    let activation = document
        .add_parameter("hyperbola active", DocumentParameterKind::Activation)
        .unwrap();
    document
        .add_parameter_binding(
            activation,
            DocumentParameterTarget::Activation(DocumentElementId::Curve(curve)),
        )
        .unwrap();
    let batch = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter: activation,
            value: ParameterValue::Activation(false),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);

    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(
        metadata.direct_edit_availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::InactiveCurve,
        )
    );
    assert!(metadata.numeric.iter().all(|property| {
        property.availability
            == DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::InactiveCurve,
            )
    }));

    let expected = coordinator.session().design_identity();
    for error in [
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_numeric_property(
                expected,
                curve,
                CurveNumericPropertyKind::SemiConjugate,
                2.0,
            )
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_hyperbola_branch(
                expected,
                curve,
                DocumentHyperbolaBranch::Negative,
            )
        }),
    ] {
        assert!(matches!(
            error,
            CoordinatorError::CurvePropertyUnavailable(
                DocumentCurveControlWithholdingReason::InactiveCurve
            )
        ));
    }
}

#[test]
fn active_fillet_arc_numeric_and_sweep_actions_are_truthfully_withheld() {
    let mut document = SketchDocument::new(4.0).unwrap();
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
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(fillet.arc))]);

    let metadata = coordinator.selected_curve_property_metadata().unwrap();
    assert_eq!(
        metadata.direct_edit_availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::AssociativeFilletOutput,
        )
    );
    assert!(metadata.numeric.iter().all(|property| {
        property.availability
            == DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::AssociativeFilletOutput,
            )
    }));

    let expected = coordinator.session().design_identity();
    for error in [
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_numeric_property(
                expected,
                fillet.arc,
                CurveNumericPropertyKind::Radius,
                2.0,
            )
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_sweep(expected, fillet.arc, DocumentArcSweep::Clockwise)
        }),
    ] {
        assert!(matches!(
            error,
            CoordinatorError::CurvePropertyUnavailable(
                DocumentCurveControlWithholdingReason::AssociativeFilletOutput
            )
        ));
    }
}

#[test]
fn curve_property_actions_require_the_exact_current_curve_selection() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let center = document.add_point("circle center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let other_center = document.add_point("other center", [3.0, 0.0]).unwrap();
    let other_radius = document
        .add_scalar(
            "other radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let other = document
        .add_curve(
            "other circle",
            CurveDefinition::Circle {
                center: other_center,
                radius: other_radius,
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan {
            curve: circle,
            segment: 1,
        })]);
    assert!(coordinator.selected_curve_property_metadata().is_none());
    let stale_span_error =
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_numeric_property(
                coordinator.session().design_identity(),
                circle,
                CurveNumericPropertyKind::Radius,
                2.0,
            )
        });
    assert!(matches!(
        stale_span_error,
        CoordinatorError::CurvePropertySelectionMismatch
    ));
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(CurveSpan::line(other))]);

    let expected = coordinator.session().design_identity();
    for error in [
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_numeric_property(
                expected,
                circle,
                CurveNumericPropertyKind::Radius,
                2.0,
            )
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_rational_middle(expected, circle, [1.0, 1.0])
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_sweep(expected, circle, DocumentArcSweep::Clockwise)
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_hyperbola_branch(
                expected,
                circle,
                DocumentHyperbolaBranch::Negative,
            )
        }),
        assert_curve_property_rejection_is_atomic(&mut coordinator, |coordinator| {
            coordinator.set_curve_nurbs_gauge(expected, circle, radius)
        }),
    ] {
        assert!(matches!(
            error,
            CoordinatorError::CurvePropertySelectionMismatch
        ));
    }
}
