// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{BoundStatus, HardValidity, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ContactState, CurveContactNeighborhood, LatentVariableRole, LineParameterDomain, Sketch,
    SketchBound, SketchConstraintKind, SketchCurve, SketchCurveContact, SketchSession,
    SketchSolveRequest,
};

fn assert_local_contact_bounds(
    neighborhood: CurveContactNeighborhood,
    semantic_bounds: (f64, f64),
    initial_parameter: f64,
    expected_status: BoundStatus,
) {
    let mut sketch = Sketch::new(1.0).expect("sketch");
    let start = sketch.add_point(Point2::new(0.0, 0.0)).expect("line start");
    let end = sketch.add_point(Point2::new(1.0, 0.0)).expect("line end");
    let contact_point = sketch
        .add_point(Point2::new(initial_parameter, 0.0))
        .expect("contact point");
    let line = sketch.add_segment(start, end).expect("line");
    sketch.add_fixed_point(start).expect("fixed line start");
    sketch.add_fixed_point(end).expect("fixed line end");
    let initial_contact = SketchCurveContact {
        curve: SketchCurve::Line {
            segment: line,
            domain: LineParameterDomain::BoundedSegment,
        },
        parameter: initial_parameter,
        neighborhood,
    };
    let constraint = sketch
        .add_point_on_curve(contact_point, initial_contact)
        .expect("point-on-line contact");

    let session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted session");
    let solve = session.accepted_result();
    assert!(solve.accepted(), "{solve:#?}");
    assert_eq!(
        solve.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(
        solve.unstable_core_report().hard_residual_max
            <= SolverConfig::default().normalized_residual_tolerance,
        "{solve:#?}"
    );

    let ContactState::PointOnCurve { parameter } = session
        .sketch()
        .contact_state(constraint)
        .expect("accepted contact state")
    else {
        panic!("generic contact state expected");
    };
    assert!(
        parameter > semantic_bounds.0 && parameter < semantic_bounds.1,
        "accepted parameter {parameter:?} must remain strictly inside {semantic_bounds:?}"
    );

    let SketchConstraintKind::PointOnCurve {
        contact: accepted_contact,
        ..
    } = session
        .sketch()
        .constraint(constraint)
        .expect("accepted constraint")
        .kind()
    else {
        panic!("generic point-on-curve constraint expected");
    };
    assert_eq!(accepted_contact.curve, initial_contact.curve);
    assert_eq!(
        accepted_contact.neighborhood, initial_contact.neighborhood,
        "solving must not rewrite explicit branch metadata"
    );
    assert_eq!(accepted_contact.parameter.to_bits(), parameter.to_bits());

    let bound = session
        .unstable_bound_report(SketchBound::Contact {
            constraint_id: constraint,
            role: LatentVariableRole::CurveParameter,
        })
        .expect("contact bound report");
    assert_eq!(bound.lower, Some(semantic_bounds.0.next_up()));
    assert_eq!(bound.upper, Some(semantic_bounds.1.next_down()));
    assert_eq!(bound.status, expected_status);
    assert!(
        bound.value >= bound.lower.expect("lower bound")
            && bound.value <= bound.upper.expect("upper bound")
    );
}

#[test]
fn local_contact_uses_closed_numeric_bounds_inside_its_open_branch() {
    let lower = 0.25_f64;
    let upper = 0.75_f64;
    assert_local_contact_bounds(
        CurveContactNeighborhood::Local { lower, upper },
        (lower, upper),
        lower.next_up(),
        BoundStatus::ActiveLower,
    );
    assert_local_contact_bounds(
        CurveContactNeighborhood::Local { lower, upper },
        (lower, upper),
        upper.next_down(),
        BoundStatus::ActiveUpper,
    );
}
