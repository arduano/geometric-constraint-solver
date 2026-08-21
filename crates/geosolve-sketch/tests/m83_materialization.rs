// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::HardValidity;
use geosolve_sketch::{
    ContactDefinition, ContactDomain, ContactNeighborhood, ContactSlot, CurveDefinition, CurveId,
    CurveSpan, DesignCurve, DesignPoint, DesignPointId, DesignScalar, DocumentConstraint,
    DocumentConstraintDefinition, DocumentDimension, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentError, DocumentExternalBinding, DocumentId, DocumentParameter,
    DocumentParameterKind, ExternalFeatureKindV1, PersistentId, ScalarDomain, ScalarUnit,
    SketchDocument, SketchDocumentSession, SketchMaterializationBatch,
    SketchMaterializationReservationAllocator, SketchMaterializationReservationSet,
    SketchMaterializationSemanticCatalog, SolverConfig,
};

const DOCUMENT_ID: u128 = 0x1000;

#[derive(Clone, Copy)]
struct LineFixtureIds {
    start: DesignPointId,
    end: DesignPointId,
    contact_point: DesignPointId,
    length_target: geosolve_sketch::DesignScalarId,
    contact_parameter: geosolve_sketch::DesignScalarId,
    line: CurveId,
    contact: geosolve_sketch::ContactId,
    horizontal: geosolve_sketch::SketchMaterializationConstraintReservation,
    point_on_curve: geosolve_sketch::SketchMaterializationConstraintReservation,
    length: geosolve_sketch::SketchMaterializationDimensionReservation,
}

fn empty_document(id: u128) -> SketchDocument {
    SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(id))).expect("document")
}

fn reserve_line_fixture(
    document: &SketchDocument,
) -> (
    LineFixtureIds,
    geosolve_sketch::SketchMaterializationReservationSet,
) {
    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("allocator");
    let ids = LineFixtureIds {
        start: allocator.reserve_point().expect("start"),
        end: allocator.reserve_point().expect("end"),
        contact_point: allocator.reserve_point().expect("contact point"),
        length_target: allocator.reserve_scalar().expect("length target"),
        contact_parameter: allocator.reserve_scalar().expect("contact parameter"),
        line: allocator.reserve_curve().expect("line"),
        contact: allocator.reserve_contact().expect("contact"),
        horizontal: allocator.reserve_constraint().expect("horizontal"),
        point_on_curve: allocator.reserve_constraint().expect("point on curve"),
        length: allocator.reserve_dimension().expect("length"),
    };
    (ids, allocator.finish().expect("reservations"))
}

fn line_fixture_batch(
    ids: LineFixtureIds,
    reservations: geosolve_sketch::SketchMaterializationReservationSet,
) -> SketchMaterializationBatch {
    let mut batch = SketchMaterializationBatch::new(reservations);
    for point in [
        DesignPoint {
            id: ids.start,
            label: "start".into(),
            position: [0.0, 0.0],
        },
        DesignPoint {
            id: ids.end,
            label: "end".into(),
            position: [4.0, 0.0],
        },
        DesignPoint {
            id: ids.contact_point,
            label: "contact point".into(),
            position: [2.0, 0.0],
        },
    ] {
        batch.push_point(point);
    }
    batch.push_scalar(DesignScalar {
        id: ids.length_target,
        label: "length target".into(),
        value: 4.0,
        unit: ScalarUnit::Length,
        domain: ScalarDomain::Positive,
    });
    batch.push_scalar(DesignScalar {
        id: ids.contact_parameter,
        label: "contact parameter".into(),
        value: 0.5,
        unit: ScalarUnit::Parameter,
        domain: ScalarDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
    });
    batch.push_curve(DesignCurve {
        id: ids.line,
        label: "line".into(),
        definition: CurveDefinition::Line {
            start: ids.start,
            end: ids.end,
            branch_direction: [1.0, 0.0],
        },
    });
    batch.push_contact(ContactSlot {
        id: ids.contact,
        label: "line contact".into(),
        curve: CurveSpan::line(ids.line),
        parameter: ids.contact_parameter,
        domain: ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: None,
    });
    batch.push_constraint(DocumentConstraint {
        id: ids.horizontal.constraint,
        source_id: ids.horizontal.source,
        label: "horizontal".into(),
        suppressed: false,
        definition: DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(ids.line),
        },
    });
    batch.push_constraint(DocumentConstraint {
        id: ids.point_on_curve.constraint,
        source_id: ids.point_on_curve.source,
        label: "point on line".into(),
        suppressed: false,
        definition: DocumentConstraintDefinition::PointOnCurve {
            point: ids.contact_point,
            contact: ids.contact,
        },
    });
    batch.push_dimension(DocumentDimension {
        id: ids.length.dimension,
        source_id: ids.length.source,
        label: "line length".into(),
        mode: DocumentDimensionMode::Driving,
        suppressed: false,
        definition: DocumentDimensionDefinition::CurveLength {
            curve: CurveSpan::line(ids.line),
            target: ids.length_target,
        },
    });
    batch.push_source(ids.horizontal.source);
    batch.push_source(ids.point_on_curve.source);
    batch.push_source(ids.length.source);
    batch
}

fn legacy_line_fixture() -> SketchDocument {
    let mut document = empty_document(DOCUMENT_ID);
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [4.0, 0.0]).expect("end");
    let contact_point = document
        .add_point("contact point", [2.0, 0.0])
        .expect("contact point");
    let length_target = document
        .add_scalar(
            "length target",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("length target");
    let contact_parameter = document
        .add_scalar(
            "contact parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .expect("contact parameter");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let contact = document
        .add_contact(
            "line contact",
            ContactDefinition {
                curve: CurveSpan::line(line),
                parameter: contact_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .expect("contact");
    document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(line),
            },
        )
        .expect("horizontal");
    document
        .add_constraint(
            "point on line",
            DocumentConstraintDefinition::PointOnCurve {
                point: contact_point,
                contact,
            },
        )
        .expect("point on line");
    document
        .add_dimension(
            "line length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(line),
                target: length_target,
            },
            DocumentDimensionMode::Driving,
        )
        .expect("length");
    document
}

fn assert_rejected_unchanged(
    document: &mut SketchDocument,
    batch: &SketchMaterializationBatch,
) -> DocumentError {
    let before = document.clone();
    let error = document
        .apply_materialization_batch(batch)
        .expect_err("batch must reject");
    assert_eq!(*document, before, "rejection must be fully atomic");
    error
}

#[test]
fn exact_batch_cold_rebuild_matches_legacy_v4_bytes_and_independent_validation() {
    let mut first = empty_document(DOCUMENT_ID);
    let (ids, reservations) = reserve_line_fixture(&first);
    let expected_high_water = reservations.resulting_high_water().clone();
    let batch = line_fixture_batch(ids, reservations);
    first
        .apply_materialization_batch(&batch)
        .expect("first materialization");

    let mut cold = empty_document(DOCUMENT_ID);
    cold.apply_materialization_batch(&batch)
        .expect("cold materialization");
    assert_eq!(cold, first);
    assert_eq!(first.persistent_identity_high_water(), expected_high_water);

    let legacy = legacy_line_fixture();
    assert_eq!(first, legacy);
    assert_eq!(
        first.to_canonical_json().expect("materialized v4"),
        legacy.to_canonical_json().expect("legacy v4"),
        "the explicit seam must not alter frozen canonical-v4 bytes"
    );

    let solved = SketchDocumentSession::new(
        first,
        geosolve_sketch::DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("independently accepted session");
    let accepted = solved.accepted_result();
    assert_eq!(
        accepted.solve().unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(
        accepted
            .solve()
            .unstable_core_report()
            .hard_residuals_validated
    );
    assert!(accepted.solve().unstable_core_report().hard_residual_max <= 1.0e-9);
}

#[test]
fn full_object_kinds_include_parameters_and_external_bindings() {
    let mut document = empty_document(DOCUMENT_ID + 1);
    let (ids, base_reservations) = reserve_line_fixture(&document);
    let base = base_reservations.base_high_water().clone();
    let mut allocator = SketchMaterializationReservationAllocator::new(base).expect("allocator");

    // Recreate the same exact allocation prefix, then add the two remaining ID-bearing kinds.
    assert_eq!(allocator.reserve_point().expect("point"), ids.start);
    assert_eq!(allocator.reserve_point().expect("point"), ids.end);
    assert_eq!(allocator.reserve_point().expect("point"), ids.contact_point);
    assert_eq!(
        allocator.reserve_scalar().expect("scalar"),
        ids.length_target
    );
    assert_eq!(
        allocator.reserve_scalar().expect("scalar"),
        ids.contact_parameter
    );
    assert_eq!(allocator.reserve_curve().expect("curve"), ids.line);
    assert_eq!(allocator.reserve_contact().expect("contact"), ids.contact);
    let horizontal = allocator.reserve_constraint().expect("constraint");
    let point_on_curve = allocator.reserve_constraint().expect("constraint");
    let length = allocator.reserve_dimension().expect("dimension");
    assert_eq!(horizontal, ids.horizontal);
    assert_eq!(point_on_curve, ids.point_on_curve);
    assert_eq!(length, ids.length);
    let parameter = allocator.reserve_parameter().expect("parameter");
    let external = allocator
        .reserve_external_binding()
        .expect("external binding");
    let reservations = allocator.finish().expect("reservations");
    let mut batch = line_fixture_batch(ids, reservations);
    batch.push_parameter(DocumentParameter {
        id: parameter,
        label: "host length".into(),
        kind: DocumentParameterKind::Length,
    });
    batch.push_external_binding(DocumentExternalBinding {
        id: external,
        label: "external point".into(),
        expected_kind: ExternalFeatureKindV1::Point,
        expected_topology: None,
    });
    document
        .apply_materialization_batch(&batch)
        .expect("full object batch");
    assert_eq!(document.parameters()[0].id, parameter);
    assert_eq!(document.external_bindings()[0].id, external);
}

#[test]
#[allow(clippy::too_many_lines)]
fn wrong_kind_duplicate_dangling_and_source_order_reject_atomically() {
    let mut wrong_kind = empty_document(DOCUMENT_ID + 10);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(wrong_kind.persistent_identity_high_water())
            .expect("allocator");
    let scalar = allocator.reserve_scalar().expect("scalar");
    let mut batch = SketchMaterializationBatch::new(allocator.finish().expect("reservations"));
    batch.push_point(DesignPoint {
        id: DesignPointId(scalar.0),
        label: "wrong kind".into(),
        position: [0.0, 0.0],
    });
    assert!(matches!(
        assert_rejected_unchanged(&mut wrong_kind, &batch),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));

    let mut duplicate = empty_document(DOCUMENT_ID + 11);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(duplicate.persistent_identity_high_water())
            .expect("allocator");
    let point = allocator.reserve_point().expect("point");
    let mut batch = SketchMaterializationBatch::retaining_unused_reservations(
        allocator.finish().expect("reservations"),
    );
    for label in ["first", "duplicate"] {
        batch.push_point(DesignPoint {
            id: point,
            label: label.into(),
            position: [0.0, 0.0],
        });
    }
    assert!(matches!(
        assert_rejected_unchanged(&mut duplicate, &batch),
        DocumentError::DuplicateId(id) if id == point.0
    ));

    let mut dangling = empty_document(DOCUMENT_ID + 12);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(dangling.persistent_identity_high_water())
            .expect("allocator");
    let start = allocator.reserve_point().expect("start");
    let omitted_end = allocator.reserve_point().expect("end");
    let line = allocator.reserve_curve().expect("line");
    let mut batch = SketchMaterializationBatch::retaining_unused_reservations(
        allocator.finish().expect("reservations"),
    );
    batch.push_point(DesignPoint {
        id: start,
        label: "start".into(),
        position: [0.0, 0.0],
    });
    batch.push_curve(DesignCurve {
        id: line,
        label: "dangling".into(),
        definition: CurveDefinition::Line {
            start,
            end: omitted_end,
            branch_direction: [1.0, 0.0],
        },
    });
    assert!(matches!(
        assert_rejected_unchanged(&mut dangling, &batch),
        DocumentError::UnknownId { kind: "point", id } if id == omitted_end.0
    ));

    let mut unordered = empty_document(DOCUMENT_ID + 13);
    let (ids, reservations) = reserve_line_fixture(&unordered);
    let mut batch = line_fixture_batch(ids, reservations);
    let missing = batch.reservations().clone();
    batch = SketchMaterializationBatch::new(missing);
    batch.push_point(DesignPoint {
        id: ids.start,
        label: "start".into(),
        position: [0.0, 0.0],
    });
    batch.push_point(DesignPoint {
        id: ids.end,
        label: "end".into(),
        position: [4.0, 0.0],
    });
    batch.push_point(DesignPoint {
        id: ids.contact_point,
        label: "contact point".into(),
        position: [2.0, 0.0],
    });
    batch.push_scalar(DesignScalar {
        id: ids.length_target,
        label: "length".into(),
        value: 4.0,
        unit: ScalarUnit::Length,
        domain: ScalarDomain::Positive,
    });
    batch.push_scalar(DesignScalar {
        id: ids.contact_parameter,
        label: "parameter".into(),
        value: 0.5,
        unit: ScalarUnit::Parameter,
        domain: ScalarDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
    });
    batch.push_curve(DesignCurve {
        id: ids.line,
        label: "line".into(),
        definition: CurveDefinition::Line {
            start: ids.start,
            end: ids.end,
            branch_direction: [1.0, 0.0],
        },
    });
    batch.push_contact(ContactSlot {
        id: ids.contact,
        label: "contact".into(),
        curve: CurveSpan::line(ids.line),
        parameter: ids.contact_parameter,
        domain: ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: None,
    });
    batch.push_constraint(DocumentConstraint {
        id: ids.horizontal.constraint,
        source_id: ids.horizontal.source,
        label: "horizontal".into(),
        suppressed: false,
        definition: DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(ids.line),
        },
    });
    assert!(matches!(
        assert_rejected_unchanged(&mut unordered, &batch),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));
}

#[test]
fn namespace_stale_base_and_malformed_high_water_reject_atomically() {
    let mut target = empty_document(DOCUMENT_ID + 20);
    let foreign = empty_document(DOCUMENT_ID + 21);
    let allocator =
        SketchMaterializationReservationAllocator::new(foreign.persistent_identity_high_water())
            .expect("allocator");
    let batch = SketchMaterializationBatch::new(allocator.finish().expect("reservations"));
    assert!(matches!(
        assert_rejected_unchanged(&mut target, &batch),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));

    let mut stale = empty_document(DOCUMENT_ID + 22);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(stale.persistent_identity_high_water())
            .expect("allocator");
    allocator.reserve_point().expect("reserved point");
    let batch = SketchMaterializationBatch::new(allocator.finish().expect("reservations"));
    stale.add_point("intervening", [0.0, 0.0]).expect("point");
    assert!(matches!(
        assert_rejected_unchanged(&mut stale, &batch),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));

    let base_document = empty_document(DOCUMENT_ID + 23);
    let mut allocator = SketchMaterializationReservationAllocator::new(
        base_document.persistent_identity_high_water(),
    )
    .expect("allocator");
    allocator.reserve_point().expect("point");
    allocator.reserve_scalar().expect("scalar");
    let forward = allocator.finish().expect("forward");
    assert!(
        SketchMaterializationReservationSet::from_parts(
            forward.resulting_high_water().clone(),
            forward.base_high_water().clone(),
            Vec::new(),
        )
        .is_err()
    );
    let mut reordered = forward.reservations().to_vec();
    reordered.reverse();
    assert!(
        SketchMaterializationReservationSet::from_parts(
            forward.base_high_water().clone(),
            forward.resulting_high_water().clone(),
            reordered,
        )
        .is_err()
    );
}

#[test]
fn unused_reservations_retire_ids_and_are_never_reused() {
    let mut document = empty_document(DOCUMENT_ID + 30);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("allocator");
    let retired = allocator.reserve_point().expect("retired point");
    let reservations = allocator.finish().expect("reservations");
    let retained_high_water = reservations.resulting_high_water().clone();
    document
        .apply_materialization_batch(&SketchMaterializationBatch::retaining_unused_reservations(
            reservations,
        ))
        .expect("identity-only batch");
    assert_eq!(
        document.persistent_identity_high_water(),
        retained_high_water
    );
    assert!(document.point(retired).is_none());

    let later = document
        .add_point("later point", [1.0, 0.0])
        .expect("later point");
    assert_ne!(later, retired);
    assert!(later.0.as_u128() > retired.0.as_u128());
}

#[test]
#[allow(clippy::too_many_lines)]
fn spline_cursor_kind_presence_and_monotonicity_are_explicit() {
    let wrong_kind = empty_document(DOCUMENT_ID + 40);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(wrong_kind.persistent_identity_high_water())
            .expect("allocator");
    let point = allocator.reserve_point().expect("point");
    assert!(
        allocator
            .reserve_spline_span_cursor(CurveId(point.0), 1)
            .is_err()
    );

    let mut missing_cursor = empty_document(DOCUMENT_ID + 41);
    let mut allocator = SketchMaterializationReservationAllocator::new(
        missing_cursor.persistent_identity_high_water(),
    )
    .expect("allocator");
    let controls = [
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
    ];
    let curve = allocator.reserve_curve().expect("curve");
    let reservations = allocator.finish().expect("reservations");
    let mut batch = SketchMaterializationBatch::new(reservations);
    for (id, position) in controls
        .into_iter()
        .zip([[0.0, 0.0], [1.0, 1.0], [2.0, 1.0], [3.0, 0.0]])
    {
        batch.push_point(DesignPoint {
            id,
            label: "control".into(),
            position,
        });
    }
    batch.push_curve(DesignCurve {
        id: curve,
        label: "spline".into(),
        definition: CurveDefinition::BSpline {
            form: geosolve_sketch::DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.to_vec(),
            knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            span_ids: vec![3, 7],
            next_span_id: 10,
        },
    });
    assert!(matches!(
        assert_rejected_unchanged(&mut missing_cursor, &batch),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));

    let mut valid = empty_document(DOCUMENT_ID + 42);
    let mut allocator =
        SketchMaterializationReservationAllocator::new(valid.persistent_identity_high_water())
            .expect("allocator");
    let controls = [
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
        allocator.reserve_point().expect("control"),
    ];
    let curve = allocator.reserve_curve().expect("curve");
    allocator
        .reserve_spline_span_cursor(curve, 12)
        .expect("span cursor");
    let reservations = allocator.finish().expect("reservations");
    let expected = reservations.resulting_high_water().clone();
    let mut batch = SketchMaterializationBatch::new(reservations);
    for (id, position) in controls
        .into_iter()
        .zip([[0.0, 0.0], [1.0, 1.0], [2.0, 1.0], [3.0, 0.0]])
    {
        batch.push_point(DesignPoint {
            id,
            label: "control".into(),
            position,
        });
    }
    batch.push_curve(DesignCurve {
        id: curve,
        label: "spline".into(),
        definition: CurveDefinition::BSpline {
            form: geosolve_sketch::DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.to_vec(),
            knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            span_ids: vec![3, 7],
            next_span_id: 10,
        },
    });
    valid
        .apply_materialization_batch(&batch)
        .expect("spline materialization");
    assert_eq!(valid.persistent_identity_high_water(), expected);
    assert!(matches!(
        &valid.curve(curve).expect("curve").definition,
        CurveDefinition::BSpline {
            next_span_id: 12,
            ..
        }
    ));

    let mut allocator =
        SketchMaterializationReservationAllocator::new(valid.persistent_identity_high_water())
            .expect("allocator");
    assert!(allocator.reserve_spline_span_cursor(curve, 11).is_err());
}

#[test]
fn semantic_catalog_ownership_is_typed_and_atomic() {
    let mut document = empty_document(DOCUMENT_ID + 50);
    let base = document.persistent_identity_high_water();
    let mut predictor =
        SketchMaterializationReservationAllocator::new(base.clone()).expect("predictor allocator");
    predictor.reserve_point().expect("discarded prediction");
    let forward_catalog = predictor
        .reserve_semantic_catalog()
        .expect("predicted forward catalog");
    let mut forward =
        SketchMaterializationReservationAllocator::new(base).expect("forward allocator");
    forward
        .reserve_semantic_source(forward_catalog)
        .expect("forward semantic source");
    assert_eq!(
        forward
            .reserve_semantic_catalog()
            .expect("forward semantic catalog"),
        forward_catalog
    );
    assert!(forward.finish().is_err());

    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("allocator");
    let first_catalog = allocator.reserve_semantic_catalog().expect("first catalog");
    let second_catalog = allocator
        .reserve_semantic_catalog()
        .expect("second catalog");
    let source = allocator
        .reserve_semantic_source(first_catalog)
        .expect("semantic source");
    let reservations = allocator.finish().expect("reservations");

    let mut wrong = SketchMaterializationBatch::new(reservations.clone());
    wrong.push_semantic_catalog(SketchMaterializationSemanticCatalog {
        catalog: second_catalog,
        sources: vec![source],
    });
    assert!(matches!(
        assert_rejected_unchanged(&mut document, &wrong),
        DocumentError::InvalidField {
            field: "materialization batch",
            ..
        }
    ));

    let mut valid = SketchMaterializationBatch::new(reservations);
    valid.push_semantic_catalog(SketchMaterializationSemanticCatalog {
        catalog: first_catalog,
        sources: vec![source],
    });
    valid.push_semantic_catalog(SketchMaterializationSemanticCatalog {
        catalog: second_catalog,
        sources: Vec::new(),
    });
    document
        .apply_materialization_batch(&valid)
        .expect("semantic catalogs");
    assert!(document.element(first_catalog.0).is_none());
    assert!(document.source(source).is_none());

    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("allocator");
    let later = allocator
        .reserve_semantic_source(first_catalog)
        .expect("later source");
    let mut extension = SketchMaterializationBatch::new(allocator.finish().expect("reservations"));
    extension.push_semantic_catalog(SketchMaterializationSemanticCatalog {
        catalog: first_catalog,
        sources: vec![later],
    });
    document
        .apply_materialization_batch(&extension)
        .expect("catalog extension");
    assert!(document.element(later.0).is_none());
}
