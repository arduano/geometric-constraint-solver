use geosolve_core::ResidualCategory;
use geosolve_sketch::{
    CancellationToken, ContactNeighborhood, CurrentDocumentCommandKind, CurrentDocumentEffectKind,
    CurrentMeasurementKind, CurveDefinition, CurveSpan, DocumentAngleOrientation,
    DocumentCenterRef, DocumentConstraintDefinition, DocumentControlRef, DocumentCoordinateAxis,
    DocumentCurveSpanRef, DocumentDirectionRef, DocumentDirectionSense, DocumentEndpointRef,
    DocumentLineSupportRef, DocumentPointRef, DocumentScalarBranch, DocumentScalarPropertyRef,
    DocumentScalarRelation, DocumentScalarUnit, DocumentSemanticSourceCatalog,
    DocumentSignedLengthProvenance, FeatureEndpoint, FeatureRef, OperationControl, OperationLimits,
    OperationOutcome, ScalarDomain, ScalarUnit, SketchDocument, cancellation_pair,
};

#[test]
fn control_refs_survive_bspline_and_nurbs_refinement_without_retargeting() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = (0..4)
        .map(|index| {
            document
                .add_point(format!("p{index}"), [f64::from(index), 0.0])
                .unwrap()
        })
        .collect::<Vec<_>>();
    let spline = document
        .add_curve(
            "spline",
            CurveDefinition::BSpline {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.clone(),
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    let weights = controls
        .iter()
        .enumerate()
        .map(|(index, _)| {
            document
                .add_scalar(
                    format!("w{index}"),
                    1.0,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let nurbs = document
        .add_curve(
            "nurbs",
            CurveDefinition::Nurbs {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.clone(),
                weights: weights.clone(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();

    for curve in [spline, nurbs] {
        let reference = DocumentControlRef {
            curve,
            control: controls[2],
        };
        assert_eq!(
            document.resolve_control_ref(reference).unwrap(),
            controls[2]
        );
        if curve == spline {
            document.insert_bspline_knot(curve, 0.5).unwrap();
        } else {
            document.insert_nurbs_knot(curve, 0.5).unwrap();
        }
        assert_eq!(
            document.resolve_control_ref(reference).unwrap(),
            controls[2]
        );
        let foreign = document.add_point("foreign", [9.0, 9.0]).unwrap();
        assert!(
            document
                .validate_control_ref(DocumentControlRef {
                    curve,
                    control: foreign,
                })
                .is_err()
        );
    }
}

#[test]
fn semantic_source_catalog_reserves_document_ids_and_rejects_catalog_corruption() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let scalar = document
        .add_scalar("length", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let property = DocumentScalarPropertyRef {
        scalar,
        unit: DocumentScalarUnit::Length,
        domain: ScalarDomain::Positive,
        branch: DocumentScalarBranch::Unsigned,
    };
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let first = catalog
        .add_scalar_source(
            &mut document,
            "first",
            DocumentScalarRelation::Fixed {
                property,
                target: 1.0,
            },
        )
        .unwrap();
    let sibling = catalog
        .add_scalar_source(
            &mut document,
            "sibling",
            DocumentScalarRelation::Fixed {
                property,
                target: 1.0,
            },
        )
        .unwrap();
    let later_point = document.add_point("later", [0.0, 0.0]).unwrap();
    assert_ne!(first.0, sibling.0);
    assert_ne!(first.0, later_point.0);
    assert_ne!(sibling.0, later_point.0);

    let document_json = document.to_canonical_json().unwrap();
    let encoded = catalog.to_canonical_json().unwrap();
    assert!(DocumentSemanticSourceCatalog::from_json(&mut document, &encoded).is_err());
    let sibling_catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    assert_ne!(sibling_catalog.catalog_id(), catalog.catalog_id());
    let mut restored_document = SketchDocument::from_json(&document_json).unwrap();
    let restored =
        DocumentSemanticSourceCatalog::from_json(&mut restored_document, &encoded).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), encoded);

    let duplicate = encoded.replacen(
        &format!("\"source_id\":\"{}\"", sibling.0),
        &format!("\"source_id\":\"{}\"", first.0),
        1,
    );
    let mut duplicate_document = SketchDocument::from_json(&document_json).unwrap();
    assert!(DocumentSemanticSourceCatalog::from_json(&mut duplicate_document, &duplicate).is_err());
    DocumentSemanticSourceCatalog::from_json(&mut duplicate_document, &encoded).unwrap();
}

#[test]
fn spline_and_nurbs_controls_are_persistent_point_features() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [1.0, 1.0]).unwrap(),
        document.add_point("p2", [2.0, 1.0]).unwrap(),
        document.add_point("p3", [3.0, 0.0]).unwrap(),
    ];
    let spline = document
        .add_curve(
            "spline",
            CurveDefinition::BSpline {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    let weights = controls
        .iter()
        .enumerate()
        .map(|(index, _)| {
            document
                .add_scalar(
                    format!("w{index}"),
                    1.0,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let nurbs = document
        .add_curve(
            "nurbs",
            CurveDefinition::Nurbs {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                weights: weights.clone(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();

    for curve in [spline, nurbs] {
        for index in 0..controls.len() {
            document
                .validate_feature(FeatureRef::CurveControl {
                    curve,
                    index: u32::try_from(index).unwrap(),
                })
                .unwrap();
        }
        assert!(
            document
                .validate_feature(FeatureRef::CurveControl {
                    curve,
                    index: u32::try_from(controls.len()).unwrap(),
                })
                .is_err()
        );
    }
}

#[test]
fn current_command_effect_and_measurement_characterization_is_exhaustive_and_stable() {
    assert_eq!(CurrentDocumentCommandKind::ALL.len(), 39);
    assert_eq!(CurrentDocumentEffectKind::ALL.len(), 36);
    assert_eq!(CurrentMeasurementKind::ALL.len(), 16);

    let command_codes = CurrentDocumentCommandKind::ALL
        .iter()
        .map(|kind| kind.code())
        .collect::<std::collections::BTreeSet<_>>();
    let effect_codes = CurrentDocumentEffectKind::ALL
        .iter()
        .map(|kind| kind.code())
        .collect::<std::collections::BTreeSet<_>>();
    let measurement_codes = CurrentMeasurementKind::ALL
        .iter()
        .map(|kind| kind.code())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(command_codes.len(), CurrentDocumentCommandKind::ALL.len());
    assert_eq!(effect_codes.len(), CurrentDocumentEffectKind::ALL.len());
    assert_eq!(measurement_codes.len(), CurrentMeasurementKind::ALL.len());

    assert!(command_codes.contains("create_point"));
    assert!(command_codes.contains("set_curve_curve_fillet_branch"));
    let m41_command_codes = [
        "set_geometry_role",
        "set_element_user_suppressed",
        "set_host_configuration_activation",
    ];
    assert_eq!(m41_command_codes.len(), 3);
    assert!(
        m41_command_codes
            .iter()
            .all(|code| command_codes.contains(code))
    );
    assert!(command_codes.contains("delete"));
    assert!(effect_codes.contains("created_rectangle"));
    assert!(effect_codes.contains("transaction"));
    assert!(effect_codes.contains("redo"));
    let m41_effect_codes = [
        "updated_geometry_role",
        "updated_element_user_suppression",
        "updated_host_configuration_activation",
    ];
    assert_eq!(m41_effect_codes.len(), 3);
    assert!(
        m41_effect_codes
            .iter()
            .all(|code| effect_codes.contains(code))
    );
    assert!(measurement_codes.contains("dimension_curve_length"));
    assert!(measurement_codes.contains("curve_signed_curvature"));
    assert!(measurement_codes.contains("conic_conjugate_axis_length"));
}

#[test]
fn semantic_scalar_lowering_honors_cancellation_and_is_persistence_neutral() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let scalar = document
        .add_scalar("length", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_scalar_source(
            &mut document,
            "fixed length",
            DocumentScalarRelation::Fixed {
                property: DocumentScalarPropertyRef {
                    scalar,
                    unit: DocumentScalarUnit::Length,
                    domain: ScalarDomain::Positive,
                    branch: DocumentScalarBranch::Unsigned,
                },
                target: 2.0,
            },
        )
        .unwrap();
    let before = document.to_canonical_json().unwrap();

    let (handle, token) = cancellation_pair();
    handle.cancel();
    let cancelled = catalog
        .lower_controlled(
            &document,
            source,
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));

    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = 0;
    let exhausted = catalog
        .lower_controlled(
            &document,
            source,
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn fixed_and_equal_scalar_sources_are_typed_deterministic_and_differentiable() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first = document
        .add_scalar(
            "signed offset",
            -2.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let second = document
        .add_scalar(
            "other offset",
            -3.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .unwrap();
    let first_property = DocumentScalarPropertyRef {
        scalar: first,
        unit: DocumentScalarUnit::Length,
        domain: ScalarDomain::Finite,
        branch: DocumentScalarBranch::SignedLength {
            provenance: DocumentSignedLengthProvenance::OrderedOperands,
        },
    };
    let second_property = DocumentScalarPropertyRef {
        scalar: second,
        unit: DocumentScalarUnit::Length,
        domain: ScalarDomain::Finite,
        branch: DocumentScalarBranch::SignedLength {
            provenance: DocumentSignedLengthProvenance::OrderedOperands,
        },
    };
    document
        .validate_scalar_property_ref(first_property)
        .unwrap();
    document
        .validate_scalar_property_ref(second_property)
        .unwrap();

    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let fixed = catalog
        .add_scalar_source(
            &mut document,
            "fixed signed offset",
            DocumentScalarRelation::Fixed {
                property: first_property,
                target: -4.0,
            },
        )
        .unwrap();
    let equal = catalog
        .add_scalar_source(
            &mut document,
            "equal signed offsets",
            DocumentScalarRelation::Equal {
                first: first_property,
                second: second_property,
            },
        )
        .unwrap();
    let incompatible_second = DocumentScalarPropertyRef {
        branch: DocumentScalarBranch::SignedLength {
            provenance: DocumentSignedLengthProvenance::DatumAxis {
                axis: DocumentCoordinateAxis::X,
            },
        },
        ..second_property
    };
    assert!(
        catalog
            .add_scalar_source(
                &mut document,
                "incompatible signed meanings",
                DocumentScalarRelation::Equal {
                    first: first_property,
                    second: incompatible_second,
                },
            )
            .is_err()
    );

    let fixed_lowered = catalog.lower(&document, fixed).unwrap();
    let fixed_repeated = catalog.lower(&document, fixed).unwrap();
    assert_eq!(fixed_lowered, fixed_repeated);
    assert_eq!(fixed_lowered.source_id, fixed);
    assert_eq!(fixed_lowered.rows.len(), 1);
    assert_eq!(fixed_lowered.rows[0].category, ResidualCategory::Hard);
    assert!((fixed_lowered.rows[0].raw_value - 2.0).abs() <= f64::EPSILON);
    assert!((fixed_lowered.rows[0].normalized_value - 0.2).abs() <= f64::EPSILON);
    assert_eq!(fixed_lowered.rows[0].raw_jacobian, vec![1.0]);
    assert_eq!(fixed_lowered.rows[0].normalized_jacobian, vec![0.1]);
    assert_eq!(fixed_lowered.audit.bindings.len(), 1);

    let equal_lowered = catalog.lower(&document, equal).unwrap();
    assert_eq!(equal_lowered.source_id, equal);
    assert_eq!(equal_lowered.rows.len(), 1);
    assert!((equal_lowered.rows[0].raw_value - 1.0).abs() <= f64::EPSILON);
    assert_eq!(equal_lowered.rows[0].raw_jacobian, vec![1.0, -1.0]);
    assert_eq!(equal_lowered.rows[0].normalized_jacobian, vec![0.1, -0.1]);
    assert_eq!(equal_lowered.audit.bindings.len(), 2);

    let epsilon = 1.0e-6;
    for (row, values) in [
        (&fixed_lowered.rows[0], vec![-2.0]),
        (&equal_lowered.rows[0], vec![-2.0, -3.0]),
    ] {
        for column in 0..values.len() {
            let mut plus = values.clone();
            let mut minus = values.clone();
            plus[column] += epsilon;
            minus[column] -= epsilon;
            let finite_difference = (row.evaluate_normalized(&plus).unwrap()
                - row.evaluate_normalized(&minus).unwrap())
                / (2.0 * epsilon);
            assert!((finite_difference - row.normalized_jacobian[column]).abs() <= 1.0e-9);
        }
    }

    let document_json = document.to_canonical_json().unwrap();
    let encoded = catalog.to_canonical_json().unwrap();
    let mut decoded_document = SketchDocument::from_json(&document_json).unwrap();
    let decoded =
        DocumentSemanticSourceCatalog::from_json(&mut decoded_document, &encoded).unwrap();
    assert_eq!(decoded, catalog);
    assert!(encoded.contains("ordered_operands"));
    assert!(!encoded.contains("-2.0"));
    assert!(!encoded.contains("-3.0"));
}

#[test]
fn semantic_scalar_sources_reject_foreign_documents_without_mutation() {
    let mut owner = SketchDocument::new(1.0).unwrap();
    let scalar = owner
        .add_scalar("ratio", 0.5, ScalarUnit::Parameter, ScalarDomain::Finite)
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut owner).unwrap();
    let source = catalog
        .add_scalar_source(
            &mut owner,
            "fixed ratio",
            DocumentScalarRelation::Fixed {
                property: DocumentScalarPropertyRef {
                    scalar,
                    unit: DocumentScalarUnit::Dimensionless,
                    domain: ScalarDomain::Finite,
                    branch: DocumentScalarBranch::Dimensionless,
                },
                target: 0.5,
            },
        )
        .unwrap();
    let foreign = SketchDocument::new(1.0).unwrap();
    let before = foreign.to_canonical_json().unwrap();
    assert!(catalog.lower(&foreign, source).is_err());
    assert_eq!(foreign.to_canonical_json().unwrap(), before);
}

#[test]
fn semantic_scalar_source_identity_cannot_alias_an_existing_document_source() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("point", [0.0, 0.0]).unwrap();
    let constraint = document
        .add_constraint(
            "fixed point",
            DocumentConstraintDefinition::FixedPoint {
                point,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let existing_source = document.constraint(constraint).unwrap().source_id;
    let scalar = document
        .add_scalar("length", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_scalar_source(
            &mut document,
            "fixed length",
            DocumentScalarRelation::Fixed {
                property: DocumentScalarPropertyRef {
                    scalar,
                    unit: DocumentScalarUnit::Length,
                    domain: ScalarDomain::Positive,
                    branch: DocumentScalarBranch::Unsigned,
                },
                target: 1.0,
            },
        )
        .unwrap();
    assert_ne!(source, existing_source);
    assert!(catalog.validate(&document).is_ok());
}

#[test]
fn scalar_source_acceptance_requires_independent_normalized_row_validation() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let scalar = document
        .add_scalar("length", 6.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let property = DocumentScalarPropertyRef {
        scalar,
        unit: DocumentScalarUnit::Length,
        domain: ScalarDomain::Positive,
        branch: DocumentScalarBranch::Unsigned,
    };
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_scalar_source(
            &mut document,
            "fixed length",
            DocumentScalarRelation::Fixed {
                property,
                target: 6.0,
            },
        )
        .unwrap();
    let lowered = catalog.lower(&document, source).unwrap();
    assert_eq!(
        lowered.audit.relation,
        DocumentScalarRelation::Fixed {
            property,
            target: 6.0
        }
    );
    assert!(
        catalog
            .independently_validated(&document, source, &lowered, 1.0e-9)
            .unwrap()
    );
    document.set_scalar_value(scalar, 7.0).unwrap();
    assert!(
        catalog
            .independently_validated(&document, source, &lowered, 1.0e-9)
            .is_err()
    );
    document.set_scalar_value(scalar, 6.0).unwrap();
    let mismatched = catalog
        .add_scalar_source(
            &mut document,
            "wrong target",
            DocumentScalarRelation::Fixed {
                property: DocumentScalarPropertyRef {
                    scalar,
                    unit: DocumentScalarUnit::Length,
                    domain: ScalarDomain::Positive,
                    branch: DocumentScalarBranch::Unsigned,
                },
                target: 5.0,
            },
        )
        .unwrap();
    let mismatched_evidence = catalog.lower(&document, mismatched).unwrap();
    assert!(
        !catalog
            .independently_validated(&document, mismatched, &mismatched_evidence, 1.0e-9)
            .unwrap()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn parameter_properties_enforce_domain_topology_winding_and_neighborhood() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [1.0, 0.0]).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let bounded = document
        .add_scalar(
            "bounded",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let periodic = document
        .add_scalar(
            "periodic",
            0.25,
            ScalarUnit::Angle,
            ScalarDomain::Periodic {
                period: std::f64::consts::TAU,
            },
        )
        .unwrap();

    let bounded_property = |neighborhood, winding| DocumentScalarPropertyRef {
        scalar: bounded,
        unit: DocumentScalarUnit::Parameter,
        domain: ScalarDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        branch: DocumentScalarBranch::Parameter {
            support: DocumentCurveSpanRef {
                span: CurveSpan::line(line),
                winding,
            },
            neighborhood,
        },
    };
    document
        .validate_scalar_property_ref(bounded_property(ContactNeighborhood::Interior, 0))
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    assert!(
        catalog
            .add_scalar_source(
                &mut document,
                "endpoint target with interior branch",
                DocumentScalarRelation::Fixed {
                    property: bounded_property(ContactNeighborhood::Interior, 0),
                    target: 1.0,
                },
            )
            .is_err()
    );
    assert!(
        document
            .validate_scalar_property_ref(bounded_property(ContactNeighborhood::Start, 0))
            .is_err()
    );
    assert!(
        document
            .validate_scalar_property_ref(bounded_property(
                ContactNeighborhood::Local {
                    lower: 0.6,
                    upper: 0.4,
                },
                0,
            ))
            .is_err()
    );
    assert!(
        document
            .validate_scalar_property_ref(bounded_property(ContactNeighborhood::Interior, 1))
            .is_err()
    );

    let periodic_property = |neighborhood| DocumentScalarPropertyRef {
        scalar: periodic,
        unit: DocumentScalarUnit::Parameter,
        domain: ScalarDomain::Periodic {
            period: std::f64::consts::TAU,
        },
        branch: DocumentScalarBranch::Parameter {
            support: DocumentCurveSpanRef {
                span: CurveSpan::line(circle),
                winding: 1,
            },
            neighborhood,
        },
    };
    document
        .validate_scalar_property_ref(periodic_property(ContactNeighborhood::Local {
            lower: std::f64::consts::TAU,
            upper: std::f64::consts::TAU + 0.5,
        }))
        .unwrap();
    assert!(
        document
            .validate_scalar_property_ref(periodic_property(ContactNeighborhood::Local {
                lower: 0.0,
                upper: 0.5,
            }))
            .is_err()
    );

    let wrong_support = DocumentScalarPropertyRef {
        scalar: periodic,
        unit: DocumentScalarUnit::Parameter,
        domain: ScalarDomain::Periodic {
            period: std::f64::consts::TAU,
        },
        branch: DocumentScalarBranch::Parameter {
            support: DocumentCurveSpanRef {
                span: CurveSpan::line(line),
                winding: 0,
            },
            neighborhood: ContactNeighborhood::Interior,
        },
    };
    assert!(
        document
            .validate_scalar_property_ref(wrong_support)
            .is_err()
    );
}

#[test]
fn normalized_rows_scale_jacobians_and_reject_corrupt_public_evidence() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).unwrap();
        let scalar = document
            .add_scalar(
                "length",
                2.0 * scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let property = DocumentScalarPropertyRef {
            scalar,
            unit: DocumentScalarUnit::Length,
            domain: ScalarDomain::Positive,
            branch: DocumentScalarBranch::Unsigned,
        };
        let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
        let source = catalog
            .add_scalar_source(
                &mut document,
                "fixed length",
                DocumentScalarRelation::Fixed {
                    property,
                    target: scale,
                },
            )
            .unwrap();
        let lowered = catalog.lower(&document, source).unwrap();
        let row = &lowered.rows[0];
        assert_eq!(row.raw_jacobian, vec![1.0]);
        assert_eq!(row.normalized_jacobian, vec![1.0 / scale]);
        let step = 1.0e-6 * scale;
        let numeric = (row.evaluate_normalized(&[2.0 * scale + step]).unwrap()
            - row.evaluate_normalized(&[2.0 * scale - step]).unwrap())
            / (2.0 * step);
        assert!((numeric - row.normalized_jacobian[0]).abs() <= 1.0e-6 / scale);

        let mut corrupt_raw = lowered.clone();
        corrupt_raw.rows[0].raw_value += 1.0;
        assert!(
            catalog
                .independently_validated(&document, source, &corrupt_raw, 1.0e-9)
                .is_err()
        );
        let mut corrupt_normalized_value = lowered.clone();
        corrupt_normalized_value.rows[0].normalized_value += 1.0;
        assert!(
            catalog
                .independently_validated(&document, source, &corrupt_normalized_value, 1.0e-9,)
                .is_err()
        );
        let mut corrupt_normalized = lowered.clone();
        corrupt_normalized.rows[0].normalized_jacobian[0] *= 2.0;
        assert!(
            catalog
                .independently_validated(&document, source, &corrupt_normalized, 1.0e-9)
                .is_err()
        );
        let mut corrupt_audit = lowered.clone();
        corrupt_audit.audit.source_label.push_str(" tampered");
        assert!(
            catalog
                .independently_validated(&document, source, &corrupt_audit, 1.0e-9)
                .is_err()
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn scalar_units_domains_branches_and_scale_normalization_are_explicit() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).unwrap();
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [scale, 0.0]).unwrap();
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let span = DocumentCurveSpanRef {
            span: CurveSpan::line(line),
            winding: 0,
        };
        let cases = [
            (
                document
                    .add_scalar(
                        "signed length",
                        -2.0 * scale,
                        ScalarUnit::Length,
                        ScalarDomain::Finite,
                    )
                    .unwrap(),
                DocumentScalarUnit::Length,
                ScalarDomain::Finite,
                DocumentScalarBranch::SignedLength {
                    provenance: DocumentSignedLengthProvenance::OrderedOperands,
                },
            ),
            (
                document
                    .add_scalar("angle", 0.25, ScalarUnit::Angle, ScalarDomain::Finite)
                    .unwrap(),
                DocumentScalarUnit::Angle,
                ScalarDomain::Finite,
                DocumentScalarBranch::Angle {
                    orientation: DocumentAngleOrientation::CounterClockwise,
                    winding: -2,
                },
            ),
            (
                document
                    .add_scalar(
                        "ratio",
                        0.5,
                        ScalarUnit::Parameter,
                        ScalarDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                    )
                    .unwrap(),
                DocumentScalarUnit::Dimensionless,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                DocumentScalarBranch::Dimensionless,
            ),
            (
                document
                    .add_scalar(
                        "curvature",
                        2.0 / scale,
                        ScalarUnit::Parameter,
                        ScalarDomain::Finite,
                    )
                    .unwrap(),
                DocumentScalarUnit::Curvature,
                ScalarDomain::Finite,
                DocumentScalarBranch::Curvature {
                    signed: true,
                    normal_side: None,
                },
            ),
            (
                document
                    .add_scalar(
                        "parameter",
                        0.4,
                        ScalarUnit::Parameter,
                        ScalarDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                    )
                    .unwrap(),
                DocumentScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                DocumentScalarBranch::Parameter {
                    support: span,
                    neighborhood: ContactNeighborhood::Local {
                        lower: 0.2,
                        upper: 0.6,
                    },
                },
            ),
        ];

        for (scalar, unit, domain, branch) in cases {
            let property = DocumentScalarPropertyRef {
                scalar,
                unit,
                domain,
                branch,
            };
            document.validate_scalar_property_ref(property).unwrap();
            let encoded = serde_json::to_string(&property).unwrap();
            let decoded: DocumentScalarPropertyRef = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, property);
        }

        let length_property = DocumentScalarPropertyRef {
            scalar: cases[0].0,
            unit: DocumentScalarUnit::Length,
            domain: ScalarDomain::Finite,
            branch: cases[0].3,
        };
        let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
        let length_source = catalog
            .add_scalar_source(
                &mut document,
                "scaled length",
                DocumentScalarRelation::Fixed {
                    property: length_property,
                    target: -3.0 * scale,
                },
            )
            .unwrap();
        assert!(
            (catalog.lower(&document, length_source).unwrap().rows[0].normalized_value - 1.0).abs()
                <= 1.0e-12
        );

        let curvature_property = DocumentScalarPropertyRef {
            scalar: cases[3].0,
            unit: DocumentScalarUnit::Curvature,
            domain: ScalarDomain::Finite,
            branch: cases[3].3,
        };
        let curvature_source = catalog
            .add_scalar_source(
                &mut document,
                "scaled curvature",
                DocumentScalarRelation::Fixed {
                    property: curvature_property,
                    target: 1.0 / scale,
                },
            )
            .unwrap();
        assert!(
            (catalog.lower(&document, curvature_source).unwrap().rows[0].normalized_value - 1.0)
                .abs()
                <= 1.0e-12
        );

        let malformed = DocumentScalarPropertyRef {
            scalar: cases[1].0,
            unit: DocumentScalarUnit::Length,
            domain: ScalarDomain::Finite,
            branch: DocumentScalarBranch::Unsigned,
        };
        assert!(document.validate_scalar_property_ref(malformed).is_err());
        assert!(
            document
                .validate_curve_span_ref(DocumentCurveSpanRef {
                    span: CurveSpan::line(line),
                    winding: 1,
                })
                .is_err()
        );
    }
}

#[test]
fn capability_specific_operands_round_trip_and_validate_without_coordinates() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let center = document.add_point("center", [2.0, -3.0]).unwrap();
    let radius = document
        .add_scalar("radius", 4.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let start = document.add_point("start", [-1.0, 0.0]).unwrap();
    let end = document.add_point("end", [1.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();

    let point = DocumentPointRef::Point { point: center };
    let center_ref = DocumentCenterRef { curve: circle };
    let endpoint = DocumentEndpointRef {
        curve: line,
        endpoint: FeatureEndpoint::Start,
    };
    let control = DocumentControlRef {
        curve: line,
        control: end,
    };
    let support = DocumentLineSupportRef {
        span: CurveSpan::line(line),
        direction: DocumentDirectionSense::Forward,
    };
    let direction = DocumentDirectionRef::LineSupport(support);
    let span = DocumentCurveSpanRef {
        span: CurveSpan::line(circle),
        winding: 2,
    };

    document.validate_point_ref(point).unwrap();
    document.validate_center_ref(center_ref).unwrap();
    document.validate_endpoint_ref(endpoint).unwrap();
    document.validate_control_ref(control).unwrap();
    document.validate_line_support_ref(support).unwrap();
    document.validate_direction_ref(direction).unwrap();
    document.validate_curve_span_ref(span).unwrap();

    for operand in [
        serde_json::to_value(point).unwrap(),
        serde_json::to_value(center_ref).unwrap(),
        serde_json::to_value(endpoint).unwrap(),
        serde_json::to_value(control).unwrap(),
        serde_json::to_value(support).unwrap(),
        serde_json::to_value(direction).unwrap(),
        serde_json::to_value(span).unwrap(),
    ] {
        let encoded = serde_json::to_string(&operand).unwrap();
        assert!(!encoded.contains("2.0"));
        assert!(!encoded.contains("-3.0"));
    }

    assert!(
        document
            .validate_center_ref(DocumentCenterRef { curve: line })
            .is_err()
    );
    assert!(
        document
            .validate_endpoint_ref(DocumentEndpointRef {
                curve: circle,
                endpoint: FeatureEndpoint::Start,
            })
            .is_err()
    );
}
