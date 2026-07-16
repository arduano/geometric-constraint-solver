use std::f64::consts::{FRAC_PI_2, PI};

use geosolve_core::{BoundStatus, HardValidity, OneSidedMobility, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ArcCircleTangencySide, ArcSweep, CircleContainment, CircleTangencyMode, ContactState,
    DimensionMode, LineParameterDomain, MIN_REPRESENTABLE_RADIUS, Sketch, SketchBound, SketchPatch,
    SketchSession, SketchSessionError, SketchSessionPatch, SketchSolveRequest, SketchSource,
    SolveRejection, tangent_circles, underconstrained_triangle,
};

#[derive(Clone, Debug, PartialEq)]
struct MappingSnapshot {
    layout: geosolve_core::PackedLayout,
    bounds: Vec<(geosolve_core::BoundId, String)>,
    sources: Vec<(
        geosolve_sketch::SketchSource,
        Option<geosolve_core::SourceConstraintId>,
        Vec<geosolve_core::ResidualId>,
    )>,
}

fn mappings(session: &SketchSession) -> MappingSnapshot {
    MappingSnapshot {
        layout: session
            .accepted_result()
            .core_report
            .accepted_state
            .layout()
            .clone(),
        bounds: session
            .accepted_result()
            .core_report
            .bounds
            .iter()
            .map(|bound| (bound.bound_id, bound.label.clone()))
            .collect(),
        sources: session
            .accepted_result()
            .source_mappings
            .iter()
            .map(|mapping| {
                (
                    mapping.source,
                    mapping.core_source_id,
                    mapping.residual_ids.clone(),
                )
            })
            .collect(),
    }
}

fn apply(session: &mut SketchSession, edit: SketchPatch) -> geosolve_sketch::SketchSolveResult {
    session
        .apply_patch(SketchSessionPatch::new(session.revision(), edit))
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn all_supported_nonstructural_patch_kinds_retain_compiled_mappings() {
    let (sketch, ids) = underconstrained_triangle().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let stable = mappings(&session);
    let compilations = session.topology_compilations();
    let point = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: ids.c,
            position: Point2::new(1.8, 2.2),
        },
    );
    assert!(point.accepted(), "{:#?}", point.rejection);
    assert_eq!(mappings(&session), stable);
    let dimension = apply(
        &mut session,
        SketchPatch::DimensionTarget {
            dimension: ids.distance_ac,
            target: 2.5,
        },
    );
    assert!(dimension.accepted(), "{:#?}", dimension.rejection);
    assert_eq!(mappings(&session), stable);
    assert_eq!(session.topology_compilations(), compilations);
    let dimension_mapping = session
        .source_mapping(SketchSource::Dimension(ids.distance_ac))
        .unwrap();
    let dimension_audit = session
        .audit_source(SketchSource::Dimension(ids.distance_ac))
        .unwrap();
    assert_eq!(dimension_mapping.source_label, dimension_audit.source_label);

    let (sketch, ids) = tangent_circles().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let stable = mappings(&session);
    let compilations = session.topology_compilations();
    let radius = apply(
        &mut session,
        SketchPatch::CircleRadius {
            circle: ids.circle_b,
            radius: 1.1,
        },
    );
    assert!(radius.accepted(), "{:#?}", radius.rejection);
    assert_eq!(mappings(&session), stable);
    let mode = apply(
        &mut session,
        SketchPatch::CircleTangencyMode {
            constraint: ids.tangency,
            mode: CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond,
            },
        },
    );
    assert!(mode.accepted(), "{:#?}", mode.rejection);
    assert_eq!(mappings(&session), stable);
    assert_eq!(session.topology_compilations(), compilations);
    let mode_mapping = session
        .source_mapping(SketchSource::Constraint(ids.tangency))
        .unwrap();
    let mode_audit = session
        .audit_source(SketchSource::Constraint(ids.tangency))
        .unwrap();
    assert_eq!(mode_mapping.source_label, mode_audit.source_label);

    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let point = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let arc = sketch
        .add_arc(center, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    sketch.add_fixed_point(center).unwrap();
    let contact = sketch.add_point_on_arc(point, arc, 0.0).unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let stable = mappings(&session);
    let compilations = session.topology_compilations();
    let arc_radius = apply(&mut session, SketchPatch::ArcRadius { arc, radius: 2.5 });
    assert!(arc_radius.accepted(), "{:#?}", arc_radius.rejection);
    assert_eq!(mappings(&session), stable);
    let contact_result = apply(
        &mut session,
        SketchPatch::ContactState {
            constraint: contact,
            state: ContactState::PointOnArc {
                span_parameter: 0.5,
            },
        },
    );
    assert!(contact_result.accepted(), "{contact_result:#?}");
    assert_eq!(mappings(&session), stable);
    assert_eq!(session.topology_compilations(), compilations);
}

#[test]
fn same_drag_point_updates_are_nonstructural_but_shape_changes_require_rebuild() {
    let (sketch, ids) = underconstrained_triangle().unwrap();
    let request = SketchSolveRequest::default().with_drag(ids.c, Point2::new(2.2, 2.0));
    let mut session = SketchSession::new(sketch, request, SolverConfig::default()).unwrap();
    let stable = mappings(&session);
    let result = apply(
        &mut session,
        SketchPatch::DragTarget {
            point: ids.c,
            target: Point2::new(0.0, 3.0),
        },
    );
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(mappings(&session), stable);
    assert_eq!(session.revision(), 1);

    let error = session
        .apply_patch(SketchSessionPatch::new(
            session.revision(),
            SketchPatch::DragTarget {
                point: ids.b,
                target: Point2::new(4.0, 0.0),
            },
        ))
        .unwrap_err();
    assert!(matches!(error, SketchSessionError::RebuildRequired));
    assert_eq!(session.revision(), 1);

    session
        .rebuild_request(session.revision(), SketchSolveRequest::default())
        .unwrap();
    assert_eq!(session.revision(), 2);
    assert!(session.request().drag.is_none());
}

#[test]
#[allow(clippy::too_many_lines)]
fn line_and_arc_endpoints_and_positive_radii_have_stable_active_bound_reports() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let p = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let center = sketch.add_point(Point2::new(5.0, 0.0)).unwrap();
    let q = sketch.add_point(Point2::new(7.0, 0.0)).unwrap();
    let line = sketch.add_segment(a, b).unwrap();
    let arc = sketch
        .add_arc(center, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    let circle = sketch.add_circle(center, f64::from_bits(1)).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch.add_fixed_point(b).unwrap();
    sketch.add_fixed_point(center).unwrap();
    let line_contact = sketch
        .add_point_on_line(p, line, LineParameterDomain::BoundedSegment, 0.0)
        .unwrap();
    let arc_contact = sketch.add_point_on_arc(q, arc, 0.0).unwrap();
    let session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let line_report = session
        .bound_report(SketchBound::Contact {
            constraint_id: line_contact,
            role: geosolve_sketch::LatentVariableRole::LineParameter,
        })
        .unwrap();
    assert_eq!(line_report.status, BoundStatus::ActiveLower);
    let line_mapping = session
        .accepted_result()
        .bound_mappings
        .iter()
        .find(|mapping| {
            mapping.bound
                == (SketchBound::Contact {
                    constraint_id: line_contact,
                    role: geosolve_sketch::LatentVariableRole::LineParameter,
                })
        })
        .unwrap();
    assert_eq!(line_mapping.bound_id, line_report.bound_id);
    let line_audit = session
        .audit_source(SketchSource::Constraint(line_contact))
        .unwrap();
    assert!(line_audit.rows.iter().any(|row| {
        row.active_bounds.iter().any(|bound| {
            bound.bound_id == line_report.bound_id && bound.status == BoundStatus::ActiveLower
        })
    }));
    assert!(line_audit.annotations.redundancy_diagnostics.is_some());
    let line_component = session
        .accepted_result()
        .core_report
        .component_solves
        .iter()
        .find(|component| component.active_bounds.contains(&line_report.bound_id))
        .unwrap();
    assert_eq!(
        line_component.bidirectional_degrees_of_freedom + 1,
        line_component.right_nullity
    );
    assert_eq!(line_component.one_sided_mobility, OneSidedMobility::Exists);

    let arc_result = session
        .bound_report(SketchBound::Contact {
            constraint_id: arc_contact,
            role: geosolve_sketch::LatentVariableRole::ArcSpanParameter,
        })
        .unwrap();
    assert_eq!(arc_result.status, BoundStatus::ActiveLower);
    let arc_component = session
        .accepted_result()
        .core_report
        .component_solves
        .iter()
        .find(|component| component.active_bounds.contains(&arc_result.bound_id))
        .unwrap();
    assert_eq!(
        arc_component.bidirectional_degrees_of_freedom + 1,
        arc_component.right_nullity
    );
    assert_eq!(arc_component.one_sided_mobility, OneSidedMobility::Exists);

    let radius_report = session
        .bound_report(SketchBound::CircleRadius(circle))
        .unwrap();
    assert_eq!(radius_report.lower, Some(MIN_REPRESENTABLE_RADIUS));
    assert_eq!(radius_report.status, BoundStatus::ActiveLower);
}

#[test]
fn genuine_selected_span_escape_rejects_and_retains_complete_accepted_revision() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let point = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let line = sketch.add_segment(a, b).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch.add_fixed_point(b).unwrap();
    let contact = sketch
        .add_point_on_line(point, line, LineParameterDomain::BoundedSegment, 0.5)
        .unwrap();
    let request = SketchSolveRequest::default().with_drag(point, Point2::new(1.0, 0.0));
    let mut session = SketchSession::new(sketch, request, SolverConfig::default()).unwrap();
    let retained_sketch = session.sketch().clone();
    let retained_result = session.accepted_result().clone();
    let retained_revision = session.revision();
    let retained_core_revisions = session.revisions();

    let rejected = apply(
        &mut session,
        SketchPatch::DragTarget {
            point,
            target: Point2::new(3.0, 0.0),
        },
    );
    assert_eq!(
        rejected.rejection,
        Some(SolveRejection::ContactParameterOutOfDomain(contact))
    );
    assert_eq!(session.sketch().geometry(), retained_sketch.geometry());
    assert_eq!(session.accepted_result(), &retained_result);
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.revisions(), retained_core_revisions);
    assert_eq!(rejected.display_audit, retained_result.display_audit);
}

#[test]
fn stale_invalid_compilation_and_failed_solve_patches_retain_revision_and_audit() {
    let (sketch, ids) = underconstrained_triangle().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = session.accepted_result().clone();
    let retained_revision = session.revision();
    let stale = session
        .apply_patch(SketchSessionPatch::new(
            retained_revision + 1,
            SketchPatch::PointPosition {
                point: ids.c,
                position: Point2::new(1.0, 1.0),
            },
        ))
        .unwrap_err();
    assert!(matches!(stale, SketchSessionError::StalePatch { .. }));

    let invalid = session
        .apply_patch(SketchSessionPatch::new(
            retained_revision,
            SketchPatch::DimensionTarget {
                dimension: ids.distance_ac,
                target: -1.0,
            },
        ))
        .unwrap_err();
    assert!(matches!(invalid, SketchSessionError::Sketch(_)));
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.accepted_result(), &retained);

    let compile_failure = session
        .apply_patch(SketchSessionPatch::new(
            retained_revision,
            SketchPatch::PointPosition {
                point: ids.b,
                position: Point2::new(0.0, 0.0),
            },
        ))
        .unwrap_err();
    assert!(matches!(compile_failure, SketchSessionError::Sketch(_)));
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.accepted_result(), &retained);

    let failed = apply(
        &mut session,
        SketchPatch::DimensionTarget {
            dimension: ids.length_ab,
            target: 100.0,
        },
    );
    assert!(!failed.accepted());
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.accepted_result(), &retained);
}

#[test]
fn disconnected_edits_reuse_other_components_and_periodic_audit_matches_commit() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(5.0, 0.0)).unwrap();
    sketch
        .add_fixed_coordinate(first, geosolve_sketch::CoordinateAxis::Y, 0.0)
        .unwrap();
    sketch
        .add_fixed_coordinate(second, geosolve_sketch::CoordinateAxis::Y, 0.0)
        .unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: first,
            position: Point2::new(1.0, 0.0),
        },
    );
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert!(
        result
            .core_report
            .component_solves
            .iter()
            .any(|component| component.reused)
    );
    assert!(
        result
            .core_report
            .component_solves
            .iter()
            .any(|component| !component.reused)
    );

    let mut periodic = Sketch::new(1.0).unwrap();
    let center = periodic.add_point(Point2::new(0.0, 0.0)).unwrap();
    let point = periodic.add_point(Point2::new(0.0, 1.0)).unwrap();
    let circle = periodic.add_circle(center, 1.0).unwrap();
    periodic.add_fixed_point(center).unwrap();
    periodic
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let contact = periodic
        .add_point_on_circle(point, circle, FRAC_PI_2)
        .unwrap();
    let mut session = SketchSession::new(
        periodic,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = apply(
        &mut session,
        SketchPatch::ContactState {
            constraint: contact,
            state: ContactState::PointOnCircle {
                angle: FRAC_PI_2 + 2.0 * PI,
            },
        },
    );
    assert!(result.accepted(), "{:#?}", result.rejection);
    let ContactState::PointOnCircle { angle } = session.sketch().contact_state(contact).unwrap()
    else {
        panic!("expected periodic state")
    };
    let mapping = result
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Constraint(contact))
        .unwrap();
    let audit = result
        .display_audit
        .sources
        .iter()
        .find(|source| Some(source.source_id) == mapping.core_source_id)
        .unwrap();
    assert!(audit.rows.iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "warm-start angle" && binding.value == angle.to_string())
    }));
    assert!(audit.rows.iter().all(|row| {
        row.incident_variables
            .iter()
            .any(|variable| variable.value == geosolve_core::VariableValue::Scalar(angle))
    }));
}

#[test]
fn circle_arc_branch_failure_is_transactional_through_session() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let arc_center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle_center = sketch.add_point(Point2::new(3.0, 0.0)).unwrap();
    let circle = sketch.add_circle(circle_center, 1.0).unwrap();
    let arc = sketch
        .add_arc(
            arc_center,
            2.0,
            -3.0 * PI / 4.0,
            3.0 * PI / 4.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    sketch.add_fixed_point(arc_center).unwrap();
    let tangency = sketch
        .add_circle_arc_tangency(circle, arc, ArcCircleTangencySide::OutsideArc, 0.5, PI)
        .unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = session.accepted_result().clone();
    let revision = session.revision();
    let failure = apply(
        &mut session,
        SketchPatch::ContactState {
            constraint: tangency,
            state: ContactState::CircleArcTangency {
                arc_span_parameter: 1.0,
                circle_angle: 0.0,
            },
        },
    );
    if !failure.accepted() {
        assert!(matches!(
            failure.rejection,
            Some(
                SolveRejection::InvalidTangencyMode(_)
                    | SolveRejection::CenterDirectionFlipped(_)
                    | SolveRejection::CoreTermination(_)
                    | SolveRejection::HardResidual { .. }
            )
        ));
        assert_eq!(session.revision(), revision);
        assert_eq!(session.accepted_result(), &retained);
    }
    assert_eq!(
        session.accepted_result().core_report.hard_validity,
        HardValidity::Valid
    );
}

#[test]
fn branch_and_secondary_failures_retain_the_accepted_session_revision() {
    let (sketch, ids) = tangent_circles().unwrap();
    let mut branch = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = branch.accepted_result().clone();
    let revision = branch.revision();
    let rejected = apply(
        &mut branch,
        SketchPatch::PointPosition {
            point: ids.center_b,
            position: Point2::new(-3.0, 0.0),
        },
    );
    assert_eq!(
        rejected.rejection,
        Some(SolveRejection::CenterDirectionFlipped(ids.tangency))
    );
    assert_eq!(branch.revision(), revision);
    assert_eq!(branch.accepted_result(), &retained);

    let invalid_mode = branch
        .apply_patch(SketchSessionPatch::new(
            branch.revision(),
            SketchPatch::CircleTangencyMode {
                constraint: ids.tangency,
                mode: CircleTangencyMode::Internal {
                    containment: CircleContainment::SecondContainsFirst,
                },
            },
        ))
        .unwrap_err();
    assert!(matches!(invalid_mode, SketchSessionError::Sketch(_)));
    assert_eq!(branch.revision(), revision);
    assert_eq!(branch.accepted_result(), &retained);

    let (mut sketch, ids) = underconstrained_triangle().unwrap();
    let initial = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(initial.accepted());
    let request =
        SketchSolveRequest::default().with_drag(ids.c, sketch.point(ids.c).unwrap().position());
    let mut secondary = SketchSession::new(
        sketch,
        request,
        SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        },
    )
    .unwrap();
    let retained = secondary.accepted_result().clone();
    let revision = secondary.revision();
    let rejected = apply(
        &mut secondary,
        SketchPatch::DragTarget {
            point: ids.c,
            target: Point2::new(0.0, 3.0),
        },
    );
    assert!(matches!(
        rejected.rejection,
        Some(SolveRejection::CoreTermination(_))
    ));
    assert_eq!(secondary.revision(), revision);
    assert_eq!(secondary.accepted_result(), &retained);
}

#[test]
fn accepted_contact_audit_refreshes_solver_updated_warm_start_payload() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let end = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let point = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let line = sketch.add_segment(start, end).unwrap();
    sketch.add_fixed_point(start).unwrap();
    sketch.add_fixed_point(end).unwrap();
    let contact = sketch
        .add_point_on_line(point, line, LineParameterDomain::BoundedSegment, 0.5)
        .unwrap();
    let request = SketchSolveRequest::default().with_drag(point, Point2::new(1.0, 0.0));
    let mut session = SketchSession::new(sketch, request, SolverConfig::default()).unwrap();
    let result = apply(
        &mut session,
        SketchPatch::DragTarget {
            point,
            target: Point2::new(1.5, 0.0),
        },
    );
    assert!(result.accepted(), "{result:#?}");
    let ContactState::PointOnLine { parameter } = session.sketch().contact_state(contact).unwrap()
    else {
        panic!("expected point-on-line contact")
    };
    assert!((parameter - 0.75).abs() <= 1.0e-9, "{parameter}");
    let audit = session
        .audit_source(SketchSource::Constraint(contact))
        .unwrap();
    assert!(audit.rows.iter().all(|row| {
        row.bindings.iter().any(|binding| {
            binding.name == "warm-start parameter" && binding.value == parameter.to_string()
        })
    }));
    assert!(audit.rows.iter().all(|row| {
        row.incident_variables
            .iter()
            .any(|variable| variable.value == geosolve_core::VariableValue::Scalar(parameter))
    }));
}

#[test]
fn reference_overflow_rejection_is_fully_atomic_and_not_evaluated() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(f64::MAX / 2.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(f64::MAX / 2.0, 0.0)).unwrap();
    sketch
        .add_point_distance(first, second, 1.0, DimensionMode::Reference)
        .unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained_sketch = session.sketch().clone();
    let retained_result = session.accepted_result().clone();
    let retained_revision = session.revision();
    let retained_revisions = session.revisions();
    let rejected = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: second,
            position: Point2::new(-f64::MAX / 2.0, 0.0),
        },
    );
    assert!(matches!(
        rejected.rejection,
        Some(SolveRejection::IndependentValidationFailed(_))
    ));
    assert_eq!(
        rejected.core_report.hard_validity,
        HardValidity::NotEvaluated
    );
    assert_eq!(session.sketch().geometry(), retained_sketch.geometry());
    assert_eq!(session.accepted_result(), &retained_result);
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.revisions(), retained_revisions);
}

#[test]
fn rebuild_revisions_are_monotonic_for_drag_start_point_change_and_release() {
    let (sketch, ids) = underconstrained_triangle().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let initial = session.revisions();
    session
        .rebuild_request(
            session.revision(),
            SketchSolveRequest::default().with_drag(ids.c, Point2::new(2.2, 2.0)),
        )
        .unwrap();
    let started = session.revisions();
    assert_eq!(started.topology, initial.topology + 1);
    assert_eq!(started.source, initial.source + 1);
    assert_eq!(started.state, initial.state + 1);

    session
        .rebuild_request(
            session.revision(),
            SketchSolveRequest::default().with_drag(ids.b, Point2::new(4.0, 0.0)),
        )
        .unwrap();
    let changed = session.revisions();
    assert_eq!(changed.topology, started.topology + 1);
    assert_eq!(changed.source, started.source + 1);
    assert_eq!(changed.state, started.state + 1);

    let before_failure = session.revisions();
    let failed = session.rebuild_request(
        session.revision(),
        SketchSolveRequest::default().with_drag(ids.b, Point2::new(f64::NAN, 0.0)),
    );
    assert!(failed.is_err());
    assert_eq!(session.revisions(), before_failure);

    session
        .rebuild_request(session.revision(), SketchSolveRequest::default())
        .unwrap();
    let released = session.revisions();
    assert_eq!(released.topology, changed.topology + 1);
    assert_eq!(released.source, changed.source + 1);
    assert_eq!(released.state, changed.state + 1);
}

#[test]
fn representable_radius_above_the_lower_bound_remains_interior() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle = sketch.add_circle(center, f64::from_bits(2)).unwrap();
    let session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let bound = session
        .bound_report(SketchBound::CircleRadius(circle))
        .unwrap();
    assert_eq!(bound.lower, Some(f64::from_bits(1)));
    assert_eq!(bound.value.to_bits(), 2);
    assert_eq!(bound.status, BoundStatus::Inactive);
}

#[test]
fn domain_rejection_precedes_secondary_compatibility_and_marks_attempt_invalid() {
    let (sketch, ids) = tangent_circles().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let rejected = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: ids.center_b,
            position: Point2::new(-3.0, 0.0),
        },
    );
    assert_eq!(
        rejected.rejection,
        Some(SolveRejection::CenterDirectionFlipped(ids.tangency))
    );
    assert_eq!(rejected.core_report.hard_validity, HardValidity::Invalid);
}

#[test]
fn reference_dimension_edits_advance_domain_source_revision_without_dirtying_geometry() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(3.0, 0.0)).unwrap();
    let reference = sketch
        .add_point_distance(first, second, 3.0, DimensionMode::Reference)
        .unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.revisions();
    let geometry = session.accepted_result().geometry.clone();
    let result = apply(
        &mut session,
        SketchPatch::DimensionTarget {
            dimension: reference,
            target: 7.0,
        },
    );
    assert!(result.accepted(), "{result:#?}");
    assert_eq!(result.geometry, geometry);
    assert_eq!(session.revisions().source, before.source + 1);
    assert_eq!(session.revisions().state, before.state + 1);
    assert!(
        result
            .core_report
            .component_solves
            .iter()
            .all(|item| item.reused)
    );
}

#[test]
fn sequential_edits_keep_previous_state_targets_equal_to_retained_evaluators() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let segment = sketch.add_segment(first, second).unwrap();
    let length = sketch
        .add_segment_length(segment, 2.0, DimensionMode::Driving)
        .unwrap();
    let mut sequential = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let moved = apply(
        &mut sequential,
        SketchPatch::PointPosition {
            point: second,
            position: Point2::new(4.0, 0.0),
        },
    );
    assert!(moved.accepted(), "{moved:#?}");

    let mut rebuilt_oracle = SketchSession::new(
        sequential.sketch().clone(),
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let sequential_result = apply(
        &mut sequential,
        SketchPatch::DimensionTarget {
            dimension: length,
            target: 4.0,
        },
    );
    let oracle_result = apply(
        &mut rebuilt_oracle,
        SketchPatch::DimensionTarget {
            dimension: length,
            target: 4.0,
        },
    );
    assert!(sequential_result.accepted(), "{sequential_result:#?}");
    assert!(oracle_result.accepted(), "{oracle_result:#?}");
    assert_eq!(sequential_result.geometry, oracle_result.geometry);
}

#[test]
fn implicit_preference_rebase_advances_source_revision_on_radius_edit() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
    let segment = sketch.add_segment(first, second).unwrap();
    sketch
        .add_segment_length(segment, 2.0, DimensionMode::Driving)
        .unwrap();
    let circle = sketch.add_circle(first, 1.0).unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let moved = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: second,
            position: Point2::new(4.0, 0.0),
        },
    );
    assert!(moved.accepted(), "{moved:#?}");
    let before = session.revisions();
    let radius = apply(
        &mut session,
        SketchPatch::CircleRadius {
            circle,
            radius: 2.0,
        },
    );
    assert!(radius.accepted(), "{radius:#?}");
    assert_eq!(session.revisions().source, before.source + 1);
    assert_eq!(session.revisions().state, before.state + 1);
}

#[test]
fn point_edit_without_preferences_changes_only_domain_state_revision() {
    let (sketch, ids) = underconstrained_triangle().unwrap();
    let mut session = SketchSession::new(
        sketch,
        SketchSolveRequest {
            previous_state_preferences: false,
            ..SketchSolveRequest::default()
        },
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.revisions();
    let result = apply(
        &mut session,
        SketchPatch::PointPosition {
            point: ids.c,
            position: Point2::new(1.8, 2.2),
        },
    );
    assert!(result.accepted(), "{result:#?}");
    assert_eq!(session.revisions().source, before.source);
    assert_eq!(session.revisions().state, before.state + 1);
}
