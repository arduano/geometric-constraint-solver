// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    ContactAdmissibleRange, ContactAdmissibleRangeEdit, ContactNeighborhood, CurveDefinition,
    CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentElementId, DocumentSolveRequest, RetainedSketchDocumentSession,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, ScalarDomain, ScalarUnit, SketchBoundStatus,
    SketchDocument,
};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owning-layer regression keeps the structural edit, numerical continuation, independent solve validation, locality, and bound diagnostics together"
)]
fn range_only_contact_edit_continues_to_inclusive_upper_bound() {
    let mut design = SketchDocument::new(10.0).expect("document");
    let start = design.add_point("start", [0.0, 0.0]).expect("start");
    let end = design.add_point("end", [10.0, 0.0]).expect("end");
    let contact_point = design
        .add_point("contact point", [8.0, 0.0])
        .expect("contact point");
    let unrelated = design
        .add_point("unrelated", [17.0, -9.0])
        .expect("unrelated point");
    let line = design
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    design
        .add_constraint(
            "fixed start",
            DocumentConstraintDefinition::FixedPoint {
                point: start,
                target: [0.0, 0.0],
            },
        )
        .expect("fixed start");
    design
        .add_constraint(
            "fixed end",
            DocumentConstraintDefinition::FixedPoint {
                point: end,
                target: [10.0, 0.0],
            },
        )
        .expect("fixed end");
    let contact = design
        .add_curve_contact(
            "contact",
            geosolve_sketch::CurveSpan::line(line),
            0.8,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("contact");
    design
        .add_constraint(
            "point on curve",
            DocumentConstraintDefinition::PointOnCurve {
                point: contact_point,
                contact,
            },
        )
        .expect("point-on-curve");

    let upstream = RetainedSketchDocumentSession::new(
        design,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("upstream session");
    let upstream_design = upstream.design_document().clone();
    let upstream_accepted = upstream
        .accepted_state_for_current_input()
        .expect("upstream accepted")
        .document()
        .clone();
    let upstream_contact = upstream_accepted
        .contact(contact)
        .expect("accepted contact");
    let upstream_parameter = upstream_accepted
        .scalar(upstream_contact.parameter)
        .expect("accepted parameter")
        .value;
    assert!(upstream_parameter > 0.5);

    let mut changed_design = upstream_design.clone();
    changed_design
        .set_contact_admissible_ranges(&[ContactAdmissibleRangeEdit {
            contact,
            range: Some(ContactAdmissibleRange {
                lower: 0.0,
                upper: 0.5,
            }),
        }])
        .expect("range-only edit");
    let changed = RetainedSketchDocumentSession::new_from_numerical_continuation(
        changed_design,
        &upstream_design,
        &upstream_accepted,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("continued session");
    let accepted = changed
        .accepted_state_for_current_input()
        .expect("range edit accepted");
    let accepted_contact = accepted
        .document()
        .contact(contact)
        .expect("contact retained");
    let accepted_parameter = accepted
        .document()
        .scalar(accepted_contact.parameter)
        .expect("accepted parameter")
        .value;

    assert_eq!(accepted_parameter.to_bits(), 0.5_f64.to_bits());
    assert_eq!(accepted_contact.curve, upstream_contact.curve);
    assert_eq!(accepted_contact.winding, upstream_contact.winding);
    assert_eq!(accepted_contact.neighborhood, upstream_contact.neighborhood);
    assert_eq!(
        accepted_contact.tangent_orientation,
        upstream_contact.tangent_orientation
    );
    let unrelated_after = accepted
        .document()
        .point(unrelated)
        .expect("unrelated point")
        .position;
    let unrelated_before = upstream_accepted
        .point(unrelated)
        .expect("upstream unrelated point")
        .position;
    assert_eq!(
        unrelated_after.map(f64::to_bits),
        unrelated_before.map(f64::to_bits)
    );

    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert!(solve.accepted);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_none_or(|value| value <= SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
    );
    let bound = accepted
        .diagnostics()
        .bounds
        .into_iter()
        .find(|bound| bound.target == DocumentElementId::Contact(contact))
        .expect("contact bound");
    assert_eq!(bound.upper, Some(0.5));
    assert_eq!(bound.status, SketchBoundStatus::ActiveUpper);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one regression keeps the coordinated accepted points, explicit line branch, range edit, and independently validated solve together"
)]
fn range_only_contact_edit_copies_coordinated_accepted_points_atomically() {
    let mut design = SketchDocument::new(3.0).expect("document");
    let start = design.add_point("start", [0.0, 0.0]).expect("start");
    let end = design.add_point("end", [1.0, 0.0]).expect("end");
    let contact_point = design
        .add_point("contact point", [2.8, 0.0])
        .expect("contact point");
    let line = design
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    design
        .add_constraint(
            "fixed start",
            DocumentConstraintDefinition::FixedPoint {
                point: start,
                target: [2.0, 0.0],
            },
        )
        .expect("fixed start");
    design
        .add_constraint(
            "fixed end",
            DocumentConstraintDefinition::FixedPoint {
                point: end,
                target: [3.0, 0.0],
            },
        )
        .expect("fixed end");
    design
        .add_constraint(
            "horizontal line",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(line),
            },
        )
        .expect("horizontal line");
    let length = design
        .add_scalar(
            "line length",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("line length");
    design
        .add_dimension(
            "line length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(line),
                target: length,
            },
            DocumentDimensionMode::Driving,
        )
        .expect("line length dimension");
    let contact = design
        .add_curve_contact(
            "contact",
            CurveSpan::line(line),
            0.8,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("contact");
    design
        .add_constraint(
            "point on curve",
            DocumentConstraintDefinition::PointOnCurve {
                point: contact_point,
                contact,
            },
        )
        .expect("point-on-curve");

    let upstream = RetainedSketchDocumentSession::new(
        design,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("upstream session");
    let upstream_design = upstream.design_document().clone();
    let upstream_accepted = upstream
        .accepted_state_for_current_input()
        .expect("upstream accepted")
        .document()
        .clone();
    assert_eq!(
        upstream_accepted
            .point(start)
            .expect("accepted start")
            .position
            .map(f64::to_bits),
        [2.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        upstream_accepted
            .point(end)
            .expect("accepted end")
            .position
            .map(f64::to_bits),
        [3.0, 0.0].map(f64::to_bits)
    );

    let mut changed_design = upstream_design.clone();
    changed_design
        .set_contact_admissible_ranges(&[ContactAdmissibleRangeEdit {
            contact,
            range: Some(ContactAdmissibleRange {
                lower: 0.0,
                upper: 0.5,
            }),
        }])
        .expect("range-only edit");
    let changed = RetainedSketchDocumentSession::new_from_numerical_continuation(
        changed_design,
        &upstream_design,
        &upstream_accepted,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("coordinated continuation seed");
    let accepted = changed
        .accepted_state_for_current_input()
        .expect("range edit accepted");
    let accepted_contact = accepted
        .document()
        .contact(contact)
        .expect("contact retained");
    assert_eq!(
        accepted
            .document()
            .scalar(accepted_contact.parameter)
            .expect("accepted parameter")
            .value
            .to_bits(),
        0.5_f64.to_bits()
    );
    assert_eq!(
        accepted
            .document()
            .point(start)
            .expect("continued start")
            .position
            .map(f64::to_bits),
        [2.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        accepted
            .document()
            .point(end)
            .expect("continued end")
            .position
            .map(f64::to_bits),
        [3.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        accepted
            .document()
            .curve_branch_direction(CurveSpan::line(line)),
        Some([1.0, 0.0])
    );
    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert!(solve.accepted && solve.hard_residuals_validated);
}
