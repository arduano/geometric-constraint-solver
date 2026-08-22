// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DesignScalarId, DocumentArcSweep,
    DocumentBSplineForm, DocumentConstraintId, DocumentCurveNormalSide, DocumentDimensionMode,
    DocumentFilletEndpointOrder, LineLineFilletRequest, MAX_DOCUMENT_OBJECTS, PersistentId,
    ScalarDomain, ScalarUnit, ScalarValueEdit, SketchDocument,
};

#[test]
fn scalar_batch_validates_the_complete_candidate_once() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let vertex = document.add_point("vertex", [0.0, 0.0]).expect("vertex");
    let focus = document.add_point("focus", [0.0, 1.0]).expect("focus");
    let trim_start = document
        .add_scalar(
            "trim start",
            -1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .expect("trim start");
    let trim_end = document
        .add_scalar("trim end", 1.0, ScalarUnit::Parameter, ScalarDomain::Finite)
        .expect("trim end");
    document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            },
        )
        .expect("parabola");

    let mut intermediate = document.clone();
    assert!(
        intermediate.set_scalar_value(trim_start, 1.0).is_err(),
        "the first half of the reversal collapses the directed trim"
    );
    assert_eq!(intermediate, document, "a rejected scalar edit is atomic");

    document
        .set_scalar_values(&[
            ScalarValueEdit::new(trim_start, 1.0),
            ScalarValueEdit::new(trim_end, -1.0),
        ])
        .expect("the complete reversed trim is valid");

    assert_eq!(
        document
            .scalar(trim_start)
            .expect("trim start")
            .value
            .to_bits(),
        1.0f64.to_bits()
    );
    assert_eq!(
        document.scalar(trim_end).expect("trim end").value.to_bits(),
        (-1.0f64).to_bits()
    );
    document.validate().expect("complete candidate validation");
}

#[test]
fn scalar_batch_preserves_every_dedicated_single_edit_guard_atomically() {
    let (
        mut document,
        ordinary,
        contact_parameter,
        fillet_angle,
        fillet_constraint,
        gauge_weight,
        free_weight,
    ) = protected_scalar_document();

    document
        .set_scalar_values(&[
            ScalarValueEdit::new(ordinary, 7.0),
            ScalarValueEdit::new(free_weight, 1.4),
        ])
        .expect("ordinary and non-gauge values");
    assert_eq!(
        document
            .scalar(ordinary)
            .expect("ordinary scalar")
            .value
            .to_bits(),
        7.0f64.to_bits()
    );
    assert_eq!(
        document
            .scalar(free_weight)
            .expect("free NURBS weight")
            .value
            .to_bits(),
        1.4f64.to_bits()
    );

    for (protected, value, expected_message) in [
        (
            contact_parameter,
            0.6,
            "contact-owned scalars require an atomic contact-state edit",
        ),
        (
            fillet_angle,
            -1.4,
            "active line-fillet endpoint angles are derived from parent contacts",
        ),
        (
            gauge_weight,
            1.1,
            "the selected NURBS gauge weight requires an explicit gauge transaction",
        ),
    ] {
        let before = document.clone();
        let error = document
            .set_scalar_values(&[
                ScalarValueEdit::new(ordinary, 8.0),
                ScalarValueEdit::new(protected, value),
            ])
            .expect_err("protected scalar must reject the complete batch");
        assert!(matches!(
            error,
            geosolve_sketch::DocumentError::InvalidField {
                field: "scalar edit",
                message,
            } if message == expected_message
        ));
        assert_eq!(document, before, "no earlier batch member may publish");
    }

    let fillet_source = document
        .constraint(fillet_constraint)
        .expect("Fillet constraint")
        .source_id;
    document
        .set_source_suppressed(fillet_source, true)
        .expect("suppress Fillet");
    document
        .set_scalar_values(&[
            ScalarValueEdit::new(ordinary, 8.0),
            ScalarValueEdit::new(fillet_angle, -1.4),
        ])
        .expect("inactive Fillet angles retain ordinary scalar edit semantics");
}

#[test]
fn scalar_batch_is_bounded_unique_and_domain_atomic() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let first = document
        .add_scalar("first", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("first");
    let second = document
        .add_scalar("second", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("second");
    let original = document.clone();

    assert!(document.set_scalar_values(&[]).is_err());
    assert_eq!(document, original);

    document
        .set_scalar_values(&[
            ScalarValueEdit::new(first, 1.0),
            ScalarValueEdit::new(second, 2.0),
        ])
        .expect("bit-identical batch is a no-op");
    assert_eq!(document, original);

    assert!(
        document
            .set_scalar_values(&[
                ScalarValueEdit::new(first, 3.0),
                ScalarValueEdit::new(first, 4.0),
            ])
            .is_err()
    );
    assert_eq!(document, original);

    assert!(
        document
            .set_scalar_values(&[
                ScalarValueEdit::new(first, 3.0),
                ScalarValueEdit::new(second, 0.0),
            ])
            .is_err()
    );
    assert_eq!(document, original);

    let unknown = DesignScalarId(PersistentId::from_u128(u128::MAX));
    assert!(
        document
            .set_scalar_values(&[
                ScalarValueEdit::new(first, 3.0),
                ScalarValueEdit::new(unknown, 4.0),
            ])
            .is_err()
    );
    assert_eq!(document, original);

    let oversized = vec![ScalarValueEdit::new(first, 3.0); MAX_DOCUMENT_OBJECTS + 1];
    let error = document
        .set_scalar_values(&oversized)
        .expect_err("oversized batch");
    assert!(matches!(
        error,
        geosolve_sketch::DocumentError::ResourceLimit {
            resource: "scalar edits",
            actual,
            limit: MAX_DOCUMENT_OBJECTS,
        } if actual == MAX_DOCUMENT_OBJECTS + 1
    ));
    assert_eq!(document, original);
}

#[allow(
    clippy::too_many_lines,
    reason = "one fixture assembles every independently guarded scalar-owner family for the atomic rejection matrix"
)]
fn protected_scalar_document() -> (
    SketchDocument,
    DesignScalarId,
    DesignScalarId,
    DesignScalarId,
    DocumentConstraintId,
    DesignScalarId,
    DesignScalarId,
) {
    let mut document = SketchDocument::new(1.0).expect("document");
    let ordinary = document
        .add_scalar("ordinary", 5.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("ordinary scalar");

    let contact_start = document
        .add_point("contact start", [-8.0, -6.0])
        .expect("contact start");
    let contact_end = document
        .add_point("contact end", [-4.0, -6.0])
        .expect("contact end");
    let contact_line = document
        .add_curve(
            "contact line",
            CurveDefinition::Line {
                start: contact_start,
                end: contact_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("contact line");
    let contact = document
        .add_curve_contact(
            "contact",
            CurveSpan::line(contact_line),
            0.5,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("contact");
    let contact_parameter = document.contact(contact).expect("contact slot").parameter;

    let first_start = document
        .add_point("first start", [0.0, 0.0])
        .expect("first start");
    let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
    let second_end = document
        .add_point("second end", [4.0, 4.0])
        .expect("second end");
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first");
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second");
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
        .expect("fillet");

    let controls = [[8.0, 0.0], [9.0, 1.0], [10.0, 0.0]].map(|position| {
        document
            .add_point("NURBS control", position)
            .expect("NURBS control")
    });
    let weights = [1.0, 0.8, 1.2].map(|value| {
        document
            .add_scalar(
                "NURBS weight",
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .expect("NURBS weight")
    });
    document
        .add_curve(
            "NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![1],
                next_span_id: 2,
            },
        )
        .expect("NURBS");

    document.validate().expect("protected scalar fixture");
    (
        document,
        ordinary,
        contact_parameter,
        fillet.start_angle,
        fillet.constraint,
        weights[0],
        weights[1],
    )
}
