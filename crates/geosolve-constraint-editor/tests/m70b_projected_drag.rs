// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::RetainedEditorCoordinator;
use geosolve_sketch::{
    ContactDomain, ContactId, ContactNeighborhood, ContactStateEdit, CurveDefinition, CurveId,
    CurveSpan, DesignPointId, DesignScalarId, DocumentConstraintDefinition, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchLifecycleRevisionHighWater, SolverConfig,
};

const LOCAL_LOWER: f64 = 0.173_626_493_534_835_56;
const LOCAL_UPPER: f64 = 0.573_626_493_534_835_6;

#[derive(Clone, Copy)]
struct ReproductionIds {
    free_line_endpoint: DesignPointId,
    circle_line_endpoint: DesignPointId,
    ellipse_outer_point: DesignPointId,
    radius: DesignScalarId,
    circle_contact: ContactId,
    line_contact: ContactId,
}

fn add_line_and_circle(
    design: &mut SketchDocument,
) -> (
    DesignPointId,
    DesignPointId,
    CurveId,
    DesignScalarId,
    ContactId,
) {
    let free_line_endpoint = design
        .add_point(
            "free line endpoint",
            [-2.222_012_280_555_491, 1.255_576_547_099_815],
        )
        .expect("free line endpoint");
    let circle_line_endpoint = design
        .add_point(
            "circle-coincident line endpoint",
            [-0.905_264_262_845_745_5, 1.855_167_165_275_810_2],
        )
        .expect("circle-coincident line endpoint");
    let line = design
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: free_line_endpoint,
                end: circle_line_endpoint,
                branch_direction: [0.756_451_510_739_997, 0.654_049_777_845_063],
            },
        )
        .expect("line");

    let circle_center = design
        .add_point(
            "circle center",
            [1.575_394_950_926_363_4, 0.244_502_178_134_677_03],
        )
        .expect("circle center");
    let radius = design
        .add_scalar(
            "radius",
            2.957_686_906_296,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("radius");
    let circle = design
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius,
            },
        )
        .expect("circle");
    let circle_contact = design
        .add_curve_contact_with_domain(
            "circle contact",
            CurveSpan::line(circle),
            ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            },
            2.565_717_349_637_516_5,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("circle contact");
    design
        .add_constraint(
            "line endpoint on circle",
            DocumentConstraintDefinition::PointOnCurve {
                point: circle_line_endpoint,
                contact: circle_contact,
            },
        )
        .expect("line endpoint on circle");
    (
        free_line_endpoint,
        circle_line_endpoint,
        line,
        radius,
        circle_contact,
    )
}

fn add_ellipse_contact(design: &mut SketchDocument, line: CurveId) -> (DesignPointId, ContactId) {
    let ellipse_center = design
        .add_point(
            "ellipse center",
            [-5.008_345_142_497_244, 2.642_864_640_738_990_6],
        )
        .expect("ellipse center");
    let ellipse_outer_point = design
        .add_point(
            "ellipse outer point",
            [-2.591_636_770_463_29, 0.397_080_892_313_915_3],
        )
        .expect("ellipse outer point");
    let ellipse_ratio = design
        .add_scalar(
            "ellipse minor-axis ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .expect("ellipse ratio");
    design
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_outer_point,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .expect("ellipse");
    let line_contact = design
        .add_curve_contact_with_domain(
            "line contact",
            CurveSpan::line(line),
            ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            0.373_626_493_534_835_57,
            0,
            ContactNeighborhood::Local {
                lower: LOCAL_LOWER,
                upper: LOCAL_UPPER,
            },
            None,
        )
        .expect("line contact");
    design
        .add_constraint(
            "ellipse outer point on line",
            DocumentConstraintDefinition::PointOnCurve {
                point: ellipse_outer_point,
                contact: line_contact,
            },
        )
        .expect("ellipse outer point on line");
    (ellipse_outer_point, line_contact)
}

fn accepted_document(design: &SketchDocument, ids: ReproductionIds) -> SketchDocument {
    let mut accepted = design.clone();
    accepted
        .set_point_position(
            ids.circle_line_endpoint,
            [-1.345_820_720_856_005_3, 1.315_526_696_222_049_8],
        )
        .expect("accepted line endpoint");
    accepted
        .set_point_position(
            ids.ellipse_outer_point,
            [-1.760_392_654_269_638, 1.287_161_161_486_543_7],
        )
        .expect("accepted ellipse outer point");
    accepted
        .set_scalar_value(ids.radius, 3.111_365_376_085_641)
        .expect("accepted radius");
    accepted
        .set_contact_states(&[ContactStateEdit {
            contact: ids.circle_contact,
            value: 2.790_174_382_107_333_7,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            tangent_orientation: None,
        }])
        .expect("accepted circle contact");
    accepted
        .set_contact_states(&[ContactStateEdit {
            contact: ids.line_contact,
            value: 0.526_847_833_175_602_7,
            winding: 0,
            neighborhood: ContactNeighborhood::Local {
                lower: LOCAL_LOWER,
                upper: LOCAL_UPPER,
            },
            tangent_orientation: None,
        }])
        .expect("accepted line contact");
    accepted
}

fn reproduction_fixture() -> (RetainedEditorCoordinator, DesignPointId) {
    let mut design = SketchDocument::new(10.0).expect("document");
    let (free_line_endpoint, circle_line_endpoint, line, radius, circle_contact) =
        add_line_and_circle(&mut design);
    let (ellipse_outer_point, line_contact) = add_ellipse_contact(&mut design, line);
    let ids = ReproductionIds {
        free_line_endpoint,
        circle_line_endpoint,
        ellipse_outer_point,
        radius,
        circle_contact,
        line_contact,
    };
    let accepted = accepted_document(&design, ids);

    let session = RetainedSketchDocumentSession::restore_current_design_with_accepted(
        design,
        accepted,
        SketchLifecycleRevisionHighWater::from_raw(22, 22, Some(22)),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("payload-derived retained session");
    (
        RetainedEditorCoordinator::new(session).expect("coordinator"),
        ids.free_line_endpoint,
    )
}

fn assert_drag_sample(
    coordinator: &RetainedEditorCoordinator,
    point: DesignPointId,
    target: [f64; 2],
    index: usize,
) {
    let work = coordinator
        .projected_drag_work_evidence()
        .expect("drag work");
    assert_eq!(work.attempts, 1, "sample {index}: {work:#?}");
    assert_eq!(work.continued, index > 0, "sample {index}: {work:#?}");
    assert!(work.accepted, "sample {index}: {work:#?}");
    assert_eq!(work.rejection_stage, None, "sample {index}: {work:#?}");
    let preview = coordinator
        .solved_preview_session()
        .expect("accepted projected preview")
        .accepted_state()
        .expect("accepted preview state");
    let accepted_position = preview
        .document()
        .point(point)
        .expect("preview free endpoint")
        .position;
    assert!(
        (accepted_position[0] - target[0]).hypot(accepted_position[1] - target[1]) <= 1.0e-8,
        "sample {index}: requested {target:?}, accepted {accepted_position:?}"
    );
    let diagnostics = preview.diagnostics();
    let solve = diagnostics.solve.expect("preview solve diagnostics");
    assert_eq!(
        solve.hard_validity,
        geosolve_sketch::SketchHardValidity::Valid
    );
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );
    assert_eq!(
        diagnostics
            .rank
            .expect("preview rank diagnostics")
            .numerical_right_nullity,
        Some(10)
    );
    assert_eq!(
        diagnostics
            .mobility
            .expect("preview mobility diagnostics")
            .equality_degrees_of_freedom,
        Some(10)
    );
    let local_contact = preview
        .document()
        .contacts()
        .iter()
        .find(|contact| matches!(contact.neighborhood, ContactNeighborhood::Local { .. }))
        .expect("payload local line contact");
    let parameter = preview
        .document()
        .scalar(local_contact.parameter)
        .expect("line contact parameter")
        .value;
    assert_eq!(
        local_contact.neighborhood,
        ContactNeighborhood::Local {
            lower: LOCAL_LOWER,
            upper: LOCAL_UPPER,
        },
        "sample {index}: persisted branch metadata changed"
    );
    assert!(
        LOCAL_LOWER < parameter && parameter < LOCAL_UPPER,
        "sample {index}: accepted contact {parameter} must remain strictly inside ({LOCAL_LOWER}, {LOCAL_UPPER})"
    );
}

#[test]
fn payload_free_line_endpoint_follows_projected_drag() {
    let (mut coordinator, point) = reproduction_fixture();
    let initial = coordinator
        .session()
        .accepted_state()
        .expect("accepted payload");
    let initial_diagnostics = initial.diagnostics();
    let initial_rank = initial_diagnostics.rank.expect("payload rank diagnostics");
    let initial_mobility = initial_diagnostics
        .mobility
        .expect("payload mobility diagnostics");
    assert_eq!(
        initial_rank.numerical_right_nullity,
        Some(10),
        "the payload is highly mobile rather than singular or overconstrained"
    );
    assert_eq!(initial_mobility.equality_degrees_of_freedom, Some(10));
    assert_eq!(
        initial_mobility.bidirectional_bounded_degrees_of_freedom,
        Some(10)
    );
    let locality = coordinator
        .session()
        .drag_locality_plan(point)
        .expect("payload locality plan");
    assert_eq!(locality.passive_degrees_of_freedom(), 5);
    assert_eq!(locality.anchor_count(), 3);
    let start = initial
        .document()
        .point(point)
        .expect("free endpoint")
        .position;

    // Each of the first four motions reached the artificial local-contact edge
    // in the reported payload and was rejected as AmbiguousContactNeighborhood.
    // Continue through reversals as one gesture to protect the last-preview path too.
    for (index, target) in [
        [start[0] + 0.5, start[1]],
        [start[0] - 0.5, start[1]],
        [start[0], start[1] + 0.5],
        [start[0], start[1] - 0.5],
        [start[0] - 1.0, start[1] + 0.8],
        [start[0] + 1.0, start[1] - 0.8],
    ]
    .into_iter()
    .enumerate()
    {
        let _ = coordinator.resolve_projected_point_move(
            1,
            u64::try_from(index + 1).expect("request id"),
            point,
            target,
        );
        assert_drag_sample(&coordinator, point, target, index);
    }
}
