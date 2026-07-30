use std::collections::BTreeSet;

use geosolve_core::{
    BoundStatus, DiagnosticStatus, HardValidity, OneSidedMobility, SolverConfig,
    StructuralClassification,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, ContactStateEdit, CurveDefinition,
    DocumentCommand, DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
    DocumentError, DocumentSessionError, DocumentSolveRequest, MAX_LABEL_BYTES, ScalarDomain,
    ScalarUnit, SketchDocument, SketchDocumentSession, SketchSolveRequest, TangentOrientation,
    alpha_scenario,
};

const SCALES: [f64; 3] = [1.0e-6, 1.0, 1.0e6];

fn session(kind: AlphaScenarioKind, scale: f64) -> (SketchDocumentSession, AlphaScenarioIds) {
    let fixture = alpha_scenario(kind, scale).unwrap();
    let session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    assert_valid(&session.accepted_result());
    (session, fixture.ids)
}

fn assert_valid(result: &geosolve_sketch::DocumentSolveResult) {
    let report = &result.accepted_view().unstable_core_report();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    assert!(report.rank_is_valid);
}

fn assert_point(actual: [f64; 2], expected: [f64; 2], scale: f64) {
    let tolerance = 2.0e-9 * scale.max(1.0);
    assert!((actual[0] - expected[0]).abs() <= tolerance, "{actual:?}");
    assert!((actual[1] - expected[1]).abs() <= tolerance, "{actual:?}");
}

#[test]
#[allow(clippy::too_many_lines)]
fn alpha_fixtures_solve_with_scale_invariant_ids_and_explicit_branches() {
    for kind in [
        AlphaScenarioKind::A1,
        AlphaScenarioKind::A2,
        AlphaScenarioKind::A3,
        AlphaScenarioKind::A4,
        AlphaScenarioKind::A5,
        AlphaScenarioKind::A8,
        AlphaScenarioKind::Corpus,
        AlphaScenarioKind::StressCompass,
        AlphaScenarioKind::StressBridge,
        AlphaScenarioKind::MotionCam,
        AlphaScenarioKind::MotionOrbit,
        AlphaScenarioKind::MotionTrammel,
        AlphaScenarioKind::MotionScotchYoke,
        AlphaScenarioKind::MotionRotatingSquare,
        AlphaScenarioKind::MotionScissor,
        AlphaScenarioKind::MotionScissorTower,
        AlphaScenarioKind::MotionPeaucellier,
        AlphaScenarioKind::MotionFourBarCoupler,
        AlphaScenarioKind::MotionPantograph,
        AlphaScenarioKind::MotionDrawingArm,
        AlphaScenarioKind::DiagnosticRankDrop,
        AlphaScenarioKind::DiagnosticEndpointBound,
        AlphaScenarioKind::DiagnosticRedundancy,
    ] {
        let mut baseline_json = None;
        for scale in SCALES {
            let (session, _) = session(kind, scale);
            let value: serde_json::Value =
                serde_json::from_str(&session.export_json().unwrap()).unwrap();
            let accepted_result = session.accepted_result();
            let report = &accepted_result.accepted_view().unstable_core_report();
            let conflicting_sources = report
                .conflicting_sources
                .iter()
                .filter_map(|source| accepted_result.persistent_core_source(*source))
                .collect::<Vec<_>>();
            let redundant_sources = report
                .sources_containing_redundant_rows
                .iter()
                .filter_map(|source| accepted_result.persistent_core_source(*source))
                .collect::<Vec<_>>();
            let identity = serde_json::json!({
                "id": value["id"],
                "next_id": value["next_id"],
                "point_ids": value["points"].as_array().unwrap().iter().map(|item| item["id"].clone()).collect::<Vec<_>>(),
                "scalar_ids": value["scalars"].as_array().unwrap().iter().map(|item| item["id"].clone()).collect::<Vec<_>>(),
                "curve_ids": value["curves"].as_array().unwrap().iter().map(|item| item["id"].clone()).collect::<Vec<_>>(),
                "contact_ids": value["contacts"].as_array().unwrap().iter().map(|item| item["id"].clone()).collect::<Vec<_>>(),
                "constraint_ids": value["constraints"].as_array().unwrap().iter().map(|item| [item["id"].clone(), item["source_id"].clone()]).collect::<Vec<_>>(),
                "dimension_ids": value["dimensions"].as_array().unwrap().iter().map(|item| [item["id"].clone(), item["source_id"].clone()]).collect::<Vec<_>>(),
                "source_order": value["source_order"],
                "curve_branches": value["curves"].as_array().unwrap().iter().map(|item| serde_json::json!({
                    "id": item["id"],
                    "branch_direction": item["definition"]["branch_direction"],
                    "branch_directions": item["definition"]["branch_directions"],
                    "sweep": item["definition"]["sweep"],
                })).collect::<Vec<_>>(),
                "contact_state": value["contacts"],
                "constraint_branches": value["constraints"].as_array().unwrap().iter().map(|item| serde_json::json!({
                    "id": item["id"],
                    "kind": item["definition"]["kind"],
                    "side": item["definition"]["side"],
                    "mode": item["definition"]["mode"],
                    "endpoint": item["definition"]["endpoint"],
                })).collect::<Vec<_>>(),
                "dimension_modes": value["dimensions"].as_array().unwrap().iter().map(|item| [item["id"].clone(), item["mode"].clone(), item["suppressed"].clone()]).collect::<Vec<_>>(),
                "rank": [report.rank, report.left_nullity, report.right_nullity, report.bidirectional_degrees_of_freedom],
                "structural": [format!("{:?}", report.structural.structural_classification), report.structural.structural_rank.to_string(), report.structural.structural_left_nullity.to_string(), report.structural.structural_right_nullity.to_string()],
                "one_sided_mobility": format!("{:?}", report.one_sided_mobility),
                "bound_statuses": report.bounds.iter().map(|bound| format!("{:?}", bound.status)).collect::<Vec<_>>(),
                "diagnostics": [format!("{:?}", report.conflict_diagnostics.status), format!("{:?}", report.redundancy_diagnostics.status)],
                "conflicting_sources": conflicting_sources,
                "redundant_sources": redundant_sources,
            });
            if let Some(baseline) = &baseline_json {
                assert_eq!(&identity, baseline, "kind={kind:?}, scale={scale:e}");
            } else {
                baseline_json = Some(identity);
            }
            if kind == AlphaScenarioKind::Corpus {
                let curve_kinds = value["curves"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["definition"]["kind"].as_str().unwrap())
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    curve_kinds,
                    BTreeSet::from([
                        "line",
                        "polyline",
                        "circle",
                        "circular_arc",
                        "quadratic_bezier",
                        "cubic_bezier",
                    ])
                );
                let constraint_kinds = value["constraints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["definition"]["kind"].as_str().unwrap())
                    .collect::<BTreeSet<_>>();
                for required in [
                    "fixed_point",
                    "coincident",
                    "horizontal",
                    "vertical",
                    "point_on_curve",
                    "parallel",
                    "perpendicular",
                    "equal_length",
                    "equal_radius",
                    "midpoint",
                    "symmetric_about_line",
                    "line_circle_tangency",
                    "circle_arc_tangency",
                    "line_curve_tangency",
                    "curve_curve_contact",
                    "curve_curve_tangency",
                ] {
                    assert!(constraint_kinds.contains(required), "missing {required}");
                }
                let dimension_kinds = value["dimensions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| item["definition"]["kind"].as_str().unwrap())
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    dimension_kinds,
                    BTreeSet::from([
                        "point_distance",
                        "curve_length",
                        "radius",
                        "diameter",
                        "oriented_angle",
                    ])
                );
                let compiled = session
                    .runtime()
                    .sketch()
                    .compile(SketchSolveRequest::default())
                    .unwrap();
                let check = compiled.problem().check_jacobians(1.0e-6).unwrap();
                assert!(check.all_within(1.0e-6), "scale={scale:e}: {check:#?}");
            }
        }
    }
}

#[test]
fn m64_editable_mechanisms_expose_one_two_and_three_freedoms_at_all_scales() {
    for (kind, expected_dof) in [
        (AlphaScenarioKind::MotionFourBarCoupler, 1),
        (AlphaScenarioKind::MotionPantograph, 2),
        (AlphaScenarioKind::MotionDrawingArm, 3),
    ] {
        for scale in SCALES {
            let (session, ids) = session(kind, scale);
            let result = session.accepted_result();
            let report = result.accepted_view().unstable_core_report();
            assert_eq!(
                report.right_nullity, expected_dof,
                "kind={kind:?}, scale={scale:e}: {report:#?}"
            );
            assert_eq!(
                report.bidirectional_degrees_of_freedom, expected_dof,
                "kind={kind:?}, scale={scale:e}: {report:#?}"
            );
            match (kind, ids) {
                (
                    AlphaScenarioKind::MotionFourBarCoupler,
                    AlphaScenarioIds::MotionFourBarCoupler(ids),
                ) => {
                    assert!(session.document().point(ids.tracer).is_some());
                    assert!(
                        ids.bars
                            .iter()
                            .all(|curve| session.document().curve(*curve).is_some())
                    );
                }
                (AlphaScenarioKind::MotionPantograph, AlphaScenarioIds::MotionPantograph(ids)) => {
                    assert!(session.document().point(ids.center).is_some());
                    assert!(
                        ids.bars
                            .iter()
                            .all(|curve| session.document().curve(*curve).is_some())
                    );
                }
                (AlphaScenarioKind::MotionDrawingArm, AlphaScenarioIds::MotionDrawingArm(ids)) => {
                    assert!(session.document().point(ids.anchor).is_some());
                    assert!(
                        ids.links
                            .iter()
                            .all(|curve| session.document().curve(*curve).is_some())
                    );
                }
                _ => panic!("fixture returned incompatible persistent roles"),
            }
        }
    }
}

#[test]
fn advanced_diagnostic_examples_expose_rank_bounds_and_redundancy() {
    let (rank_drop, rank_ids) = session(AlphaScenarioKind::DiagnosticRankDrop, 1.0);
    let AlphaScenarioIds::DiagnosticRankDrop(rank_ids) = rank_ids else {
        panic!("rank-drop IDs expected");
    };
    let rank_result = rank_drop.accepted_result();
    let rank_report = &rank_result.accepted_view().unstable_core_report();
    assert_eq!(
        (rank_report.left_nullity, rank_report.right_nullity),
        (1, 1)
    );
    assert_eq!(
        rank_report.structural.structural_classification,
        StructuralClassification::Well
    );
    assert_eq!(
        (
            rank_report.structural.structural_left_nullity,
            rank_report.structural.structural_right_nullity,
        ),
        (0, 0)
    );
    assert!(rank_report.is_singular);
    assert!(rank_drop.document().point(rank_ids.point).is_some());

    let (endpoint, endpoint_ids) = session(AlphaScenarioKind::DiagnosticEndpointBound, 1.0);
    let AlphaScenarioIds::DiagnosticEndpointBound(endpoint_ids) = endpoint_ids else {
        panic!("endpoint-bound IDs expected");
    };
    let endpoint_result = endpoint.accepted_result();
    let endpoint_report = &endpoint_result.accepted_view().unstable_core_report();
    assert_eq!(endpoint_report.right_nullity, 2);
    assert_eq!(endpoint_report.bidirectional_degrees_of_freedom, 0);
    assert_eq!(endpoint_report.one_sided_mobility, OneSidedMobility::Exists);
    assert!(
        endpoint_report
            .bounds
            .iter()
            .any(|bound| bound.status == BoundStatus::ActiveLower)
    );
    assert!(endpoint.document().contact(endpoint_ids.contact).is_some());
    assert!(endpoint.document().curve(endpoint_ids.circle).is_some());
    assert!(endpoint.document().scalar(endpoint_ids.radius).is_some());

    let (redundancy, redundancy_ids) = session(AlphaScenarioKind::DiagnosticRedundancy, 1.0);
    let AlphaScenarioIds::DiagnosticRedundancy(redundancy_ids) = redundancy_ids else {
        panic!("redundancy IDs expected");
    };
    let redundancy_result = redundancy.accepted_result();
    let redundancy_report = &redundancy_result.accepted_view().unstable_core_report();
    assert_eq!(
        (
            redundancy_report.left_nullity,
            redundancy_report.right_nullity
        ),
        (1, 0)
    );
    assert_eq!(
        redundancy_report.structural.structural_classification,
        StructuralClassification::Over
    );
    assert_eq!(
        redundancy_report.redundancy_diagnostics.status,
        DiagnosticStatus::Complete
    );
    let duplicate_source = redundancy
        .document()
        .dimension(redundancy_ids.duplicate_length)
        .unwrap()
        .source_id;
    assert!(
        redundancy_report
            .sources_containing_redundant_rows
            .iter()
            .any(|source| redundancy_result.persistent_core_source(*source)
                == Some(duplicate_source))
    );
}

#[test]
fn compass_stress_example_exposes_and_locks_rotational_mobility() {
    let (mut compass, compass_ids) = session(AlphaScenarioKind::StressCompass, 1.0);
    let AlphaScenarioIds::StressCompass(compass_ids) = compass_ids else {
        panic!("compass IDs expected");
    };
    let compass_result = compass.accepted_result();
    let compass_report = &compass_result.accepted_view().unstable_core_report();
    assert_eq!(compass_report.right_nullity, 1);
    assert_eq!(compass_report.left_nullity, 1);
    let outcome = compass
        .apply(DocumentCommand::new(
            compass.revision(),
            DocumentEdit::SetDimensionMode {
                dimension: compass_ids.angle,
                mode: DocumentDimensionMode::Driving,
            },
        ))
        .unwrap();
    assert!(outcome.accepted());
    assert_eq!(
        compass
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .right_nullity,
        0
    );
}

#[test]
fn bridge_stress_example_exposes_mobility_and_rejects_degeneracy() {
    let (mut bridge, bridge_ids) = session(AlphaScenarioKind::StressBridge, 1.0);
    let AlphaScenarioIds::StressBridge(bridge_ids) = bridge_ids else {
        panic!("bridge IDs expected");
    };
    let bridge_result = bridge.accepted_result();
    let bridge_report = &bridge_result.accepted_view().unstable_core_report();
    assert_eq!(bridge_report.right_nullity, 3);
    assert_eq!(bridge_report.bidirectional_degrees_of_freedom, 1);
    let equal_handles_source = bridge
        .document()
        .constraint(bridge_ids.equal_handles)
        .unwrap()
        .source_id;
    assert!(
        bridge
            .apply(DocumentCommand::new(
                bridge.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source: equal_handles_source,
                    suppressed: false,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert_eq!(
        bridge
            .accepted_result()
            .accepted_view()
            .unstable_core_report()
            .bidirectional_degrees_of_freedom,
        0
    );
    assert!(
        bridge
            .apply(DocumentCommand::new(
                bridge.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source: equal_handles_source,
                    suppressed: true,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        bridge
            .apply(DocumentCommand::new(
                bridge.revision(),
                DocumentEdit::SetPointPosition {
                    point: bridge_ids.left_seam,
                    position: [0.25, -0.5],
                },
            ))
            .unwrap()
            .accepted()
    );
    assert_point(
        bridge
            .document()
            .point(bridge_ids.left_seam)
            .unwrap()
            .position,
        [0.25, -0.5],
        1.0,
    );
    assert_point(
        bridge
            .document()
            .point(bridge_ids.right_seam)
            .unwrap()
            .position,
        [0.25, -0.5],
        1.0,
    );
    let retained = bridge.export_json().unwrap();
    let rejected = bridge.apply(DocumentCommand::new(
        bridge.revision(),
        DocumentEdit::SetPointPosition {
            point: bridge_ids.left_seam,
            position: [-1.0, 2.0],
        },
    ));
    assert!(rejected.is_err());
    assert_eq!(bridge.export_json().unwrap(), retained);
}

#[test]
fn cam_motion_projects_one_roller_while_stabilizing_the_other() {
    let (mut cam, cam_ids) = session(AlphaScenarioKind::MotionCam, 1.0);
    let AlphaScenarioIds::MotionCam(cam_ids) = cam_ids else {
        panic!("cam IDs expected");
    };
    let cam_result = cam.accepted_result();
    let cam_report = &cam_result.accepted_view().unstable_core_report();
    assert_eq!(cam_report.right_nullity, 2);
    assert_eq!(cam_report.bidirectional_degrees_of_freedom, 2);
    let right_before = cam.document().point(cam_ids.right_center).unwrap().position;
    let mut left_target = [0.0, 0.0];
    for step in 1..=5 {
        let parameter = 0.25 + 0.01 * f64::from(step);
        let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
        let tangent_norm = tangent[0].hypot(tangent[1]);
        left_target = [
            -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
            8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
        ];
        let request = cam
            .request()
            .without_previous_state_preferences()
            .with_drag(cam_ids.left_center, left_target)
            .with_stability_target(cam_ids.right_center, right_before);
        let moved = cam.rebuild_request(cam.revision(), request).unwrap();
        assert!(moved.accepted(), "{:#?}", moved.solve().rejection);
    }
    assert_point(
        cam.document().point(cam_ids.left_center).unwrap().position,
        left_target,
        1.0,
    );
    assert_point(
        cam.document().point(cam_ids.right_center).unwrap().position,
        right_before,
        1.0,
    );
}

#[test]
fn tangent_orbit_projected_drag_traverses_all_quadrants() {
    for scale in SCALES {
        let (mut orbit, orbit_ids) = session(AlphaScenarioKind::MotionOrbit, scale);
        let AlphaScenarioIds::MotionOrbit(orbit_ids) = orbit_ids else {
            panic!("orbit IDs expected");
        };
        let orbit_result = orbit.accepted_result();
        let orbit_report = &orbit_result.accepted_view().unstable_core_report();
        assert_eq!(orbit_report.right_nullity, 1);
        assert_eq!(orbit_report.bidirectional_degrees_of_freedom, 1);

        for unscaled in [
            [2.0, 12.0_f64.sqrt()],
            [0.0, 4.0],
            [-2.0, 12.0_f64.sqrt()],
            [-4.0, 0.0],
            [-2.0, -12.0_f64.sqrt()],
            [0.0, -4.0],
            [2.0, -12.0_f64.sqrt()],
            [4.0, 0.0],
        ] {
            let target = [unscaled[0] * scale, unscaled[1] * scale];
            let request = orbit
                .request()
                .without_previous_state_preferences()
                .with_drag(orbit_ids.moving_center, target);
            let moved = orbit.rebuild_request(orbit.revision(), request).unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
            assert_valid(&moved);
            assert_point(
                orbit
                    .document()
                    .point(orbit_ids.moving_center)
                    .unwrap()
                    .position,
                target,
                scale,
            );
        }

        let released = orbit
            .rebuild_request(
                orbit.revision(),
                DocumentSolveRequest::default().without_previous_state_preferences(),
            )
            .unwrap();
        assert!(
            released.accepted(),
            "scale={scale:e}: {:#?}",
            released.solve()
        );
        assert_eq!(
            released
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn compound_constraint_mechanisms_follow_their_emergent_motion() {
    for scale in SCALES {
        let (mut trammel, trammel_ids) = session(AlphaScenarioKind::MotionTrammel, scale);
        let AlphaScenarioIds::MotionTrammel(trammel_ids) = trammel_ids else {
            panic!("trammel IDs expected");
        };
        assert_eq!(
            trammel
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        let initial_angle = 0.8_f64.acos();
        for step in 1..=4 {
            let angle = initial_angle
                + (std::f64::consts::FRAC_PI_2 - initial_angle) * f64::from(step) / 4.0;
            let target = [3.75 * angle.cos() * scale, 1.25 * angle.sin() * scale];
            let request = trammel
                .request()
                .without_previous_state_preferences()
                .with_drag(trammel_ids.tracer, target);
            let moved = trammel
                .rebuild_request(trammel.revision(), request)
                .unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        }
        assert_point(
            trammel
                .document()
                .point(trammel_ids.horizontal_slider)
                .unwrap()
                .position,
            [0.0, 0.0],
            scale,
        );
        assert_point(
            trammel
                .document()
                .point(trammel_ids.vertical_slider)
                .unwrap()
                .position,
            [0.0, 5.0 * scale],
            scale,
        );

        let (mut yoke, yoke_ids) = session(AlphaScenarioKind::MotionScotchYoke, scale);
        let AlphaScenarioIds::MotionScotchYoke(yoke_ids) = yoke_ids else {
            panic!("Scotch-yoke IDs expected");
        };
        assert_eq!(
            yoke.accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        let initial_angle = (4.0_f64).atan2(3.0);
        for step in 1..=4 {
            let angle = initial_angle
                + (std::f64::consts::FRAC_PI_2 - initial_angle) * f64::from(step) / 4.0;
            let target = [5.0 * angle.cos() * scale, 5.0 * angle.sin() * scale];
            let request = yoke
                .request()
                .without_previous_state_preferences()
                .with_drag(yoke_ids.crank_pin, target);
            let moved = yoke.rebuild_request(yoke.revision(), request).unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        }
        assert_point(
            yoke.document().point(yoke_ids.slider).unwrap().position,
            [0.0, -6.0 * scale],
            scale,
        );

        let (mut square, square_ids) = session(AlphaScenarioKind::MotionRotatingSquare, scale);
        let AlphaScenarioIds::MotionRotatingSquare(square_ids) = square_ids else {
            panic!("rotating-square IDs expected");
        };
        assert_eq!(
            square
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        for step in 1..=4 {
            let angle = std::f64::consts::FRAC_PI_4 * f64::from(step) / 4.0;
            let target = [3.0 * angle.cos() * scale, 3.0 * angle.sin() * scale];
            let request = square
                .request()
                .without_previous_state_preferences()
                .with_drag(square_ids.corners[1], target);
            let moved = square.rebuild_request(square.revision(), request).unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        }
        let side = 3.0 * std::f64::consts::FRAC_1_SQRT_2 * scale;
        assert_point(
            square
                .document()
                .point(square_ids.corners[2])
                .unwrap()
                .position,
            [0.0, 2.0 * side],
            scale,
        );

        let (mut scissor, scissor_ids) = session(AlphaScenarioKind::MotionScissor, scale);
        let AlphaScenarioIds::MotionScissor(scissor_ids) = scissor_ids else {
            panic!("scissor IDs expected");
        };
        assert_eq!(
            scissor
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        for step in 1..=4 {
            let x = (4.0 - 0.5 * f64::from(step)) * scale;
            let request = scissor
                .request()
                .without_previous_state_preferences()
                .with_drag(scissor_ids.slider, [x, 0.0]);
            let moved = scissor
                .rebuild_request(scissor.revision(), request)
                .unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        }
        assert_point(
            scissor
                .document()
                .point(scissor_ids.upper_joint)
                .unwrap()
                .position,
            [-scale, 4.0 * scale],
            scale,
        );
        assert_point(
            scissor
                .document()
                .point(scissor_ids.lower_joint)
                .unwrap()
                .position,
            [-scale, -4.0 * scale],
            scale,
        );
    }
}

#[test]
fn advanced_linkage_examples_propagate_one_driver_through_every_bar() {
    for scale in SCALES {
        let (mut tower, tower_ids) = session(AlphaScenarioKind::MotionScissorTower, scale);
        let AlphaScenarioIds::MotionScissorTower(tower_ids) = tower_ids else {
            panic!("scissor-tower IDs expected");
        };
        assert_eq!(
            tower
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        for step in 1..=4 {
            let x = (4.0 - 0.5 * f64::from(step)) * scale;
            let request = tower
                .request()
                .without_previous_state_preferences()
                .with_drag(tower_ids.right_levels[0], [x, 0.0]);
            let moved = tower.rebuild_request(tower.revision(), request).unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        }
        assert_point(
            tower
                .document()
                .point(tower_ids.left_levels[5])
                .unwrap()
                .position,
            [-4.0 * scale, 40.0 * scale],
            scale,
        );
        assert_point(
            tower
                .document()
                .point(tower_ids.right_levels[5])
                .unwrap()
                .position,
            [2.0 * scale, 40.0 * scale],
            scale,
        );

        let (mut cell, cell_ids) = session(AlphaScenarioKind::MotionPeaucellier, scale);
        let AlphaScenarioIds::MotionPeaucellier(cell_ids) = cell_ids else {
            panic!("Peaucellier IDs expected");
        };
        assert_eq!(
            cell.accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        for step in 1..=6 {
            let angle = std::f64::consts::FRAC_PI_2
                - (std::f64::consts::FRAC_PI_2 - std::f64::consts::FRAC_PI_3) * f64::from(step)
                    / 6.0;
            let target = [(4.0 + 4.0 * angle.cos()) * scale, 4.0 * angle.sin() * scale];
            let request = cell
                .request()
                .without_previous_state_preferences()
                .with_drag(cell_ids.input, target);
            let moved = cell.rebuild_request(cell.revision(), request).unwrap();
            assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
            let output = cell.document().point(cell_ids.output).unwrap().position;
            assert!((output[0] - 2.0 * scale).abs() <= 2.0e-9 * scale.max(1.0));
        }
        assert_point(
            cell.document().point(cell_ids.output).unwrap().position,
            [2.0 * scale, 2.0 * scale / 3.0_f64.sqrt()],
            scale,
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn a1_dimension_edits_and_a2_projected_drag_match_the_canonical_workflows() {
    for scale in SCALES {
        let (mut rectangle, ids) = session(AlphaScenarioKind::A1, scale);
        let AlphaScenarioIds::A1(ids) = ids else {
            panic!("A1 IDs expected");
        };
        let persistent_points = ids.rectangle.points;
        let persistent_sources = rectangle.document().source_order().to_vec();
        for (target, value) in [
            (ids.rectangle.targets[0], 6.0 * scale),
            (ids.rectangle.targets[1], 2.5 * scale),
        ] {
            let outcome = rectangle
                .apply(DocumentCommand::new(
                    rectangle.revision(),
                    DocumentEdit::SetScalarValue {
                        scalar: target,
                        value,
                    },
                ))
                .unwrap();
            assert!(outcome.accepted(), "{:#?}", outcome.result.solve());
            assert_valid(&outcome.result);
        }
        for (point, expected) in persistent_points.into_iter().zip([
            [0.0, 0.0],
            [6.0 * scale, 0.0],
            [6.0 * scale, 2.5 * scale],
            [0.0, 2.5 * scale],
        ]) {
            assert_point(
                rectangle.document().point(point).unwrap().position,
                expected,
                scale,
            );
        }
        assert_eq!(rectangle.document().source_order(), persistent_sources);
        let accepted = rectangle.accepted_result();
        let diagonal = accepted
            .accepted_reference_value(rectangle.document(), ids.diagonal)
            .unwrap();
        assert!((diagonal - 6.5 * scale).abs() <= 2.0e-9 * scale.max(1.0));
        let diagonal_source = rectangle
            .document()
            .dimension(ids.diagonal)
            .unwrap()
            .source_id;
        assert!(
            rectangle
                .mappings()
                .runtime_source(diagonal_source)
                .is_some()
        );
        let runtime_dimension = match rectangle
            .mappings()
            .runtime_source(diagonal_source)
            .unwrap()
        {
            geosolve_sketch::RuntimeSource::Dimension(id) => id,
            geosolve_sketch::RuntimeSource::Constraint(_) => panic!("dimension mapping expected"),
        };
        let mapping = rectangle
            .runtime()
            .accepted_result()
            .source_mappings
            .iter()
            .find(|mapping| {
                mapping.source == geosolve_sketch::SketchSource::Dimension(runtime_dimension)
            })
            .unwrap();
        assert!(mapping.core_source_id.is_none());
        assert!(mapping.residual_ids.is_empty());

        let (mut triangle, ids) = session(AlphaScenarioKind::A2, scale);
        let AlphaScenarioIds::A2(ids) = ids else {
            panic!("A2 IDs expected");
        };
        assert_eq!(
            triangle
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        let history = triangle.history_len();
        let dragged = triangle
            .rebuild_request(
                triangle.revision(),
                DocumentSolveRequest::default()
                    .without_previous_state_preferences()
                    .with_drag(ids.c, [0.0, 3.0 * scale]),
            )
            .unwrap();
        assert!(dragged.accepted(), "{:#?}", dragged.solve());
        assert_point(
            triangle.document().point(ids.b).unwrap().position,
            [4.0 * scale, 0.0],
            scale,
        );
        assert_point(
            triangle.document().point(ids.c).unwrap().position,
            [0.0, 3.0 * scale],
            scale,
        );
        assert_eq!(triangle.history_len(), history);
        let released = triangle
            .rebuild_request(
                triangle.revision(),
                DocumentSolveRequest::default().without_previous_state_preferences(),
            )
            .unwrap();
        assert!(released.accepted());
        assert_eq!(
            released
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            1
        );
        assert_point(
            triangle.document().point(ids.c).unwrap().position,
            [0.0, 3.0 * scale],
            scale,
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn a3_a4_contacts_retain_explicit_state_and_reject_branch_escape() {
    for scale in SCALES {
        let (mut line_circle, ids) = session(AlphaScenarioKind::A3, scale);
        let AlphaScenarioIds::A3(ids) = ids else {
            panic!("A3 IDs expected");
        };
        assert_point(
            line_circle.document().point(ids.center).unwrap().position,
            [scale, 2.0 * scale],
            scale,
        );
        for contact in [ids.line_contact, ids.circle_contact] {
            let slot = line_circle.document().contact(contact).unwrap();
            assert_eq!(slot.winding, 0);
            assert_eq!(slot.neighborhood, ContactNeighborhood::Interior);
            assert_eq!(slot.tangent_orientation, Some(TangentOrientation::Aligned));
            let jet = line_circle
                .document()
                .evaluate_contact_jet(contact)
                .unwrap();
            assert_point([jet.position.x, jet.position.y], [scale, 0.0], scale);
        }
        assert!(
            (line_circle
                .document()
                .scalar(
                    line_circle
                        .document()
                        .contact(ids.line_contact)
                        .unwrap()
                        .parameter
                )
                .unwrap()
                .value
                - 0.6)
                .abs()
                <= 1.0e-10
        );
        let before = line_circle.export_json().unwrap();
        let line_slot = line_circle.document().contact(ids.line_contact).unwrap();
        let circle_slot = line_circle.document().contact(ids.circle_contact).unwrap();
        let escaped = line_circle
            .apply(DocumentCommand::new(
                line_circle.revision(),
                DocumentEdit::SetContactStates {
                    edits: vec![
                        ContactStateEdit {
                            contact: ids.line_contact,
                            value: 1.1,
                            winding: line_slot.winding,
                            neighborhood: line_slot.neighborhood,
                            tangent_orientation: line_slot.tangent_orientation,
                        },
                        ContactStateEdit {
                            contact: ids.circle_contact,
                            value: line_circle
                                .document()
                                .scalar(circle_slot.parameter)
                                .unwrap()
                                .value,
                            winding: circle_slot.winding,
                            neighborhood: circle_slot.neighborhood,
                            tangent_orientation: circle_slot.tangent_orientation,
                        },
                    ],
                },
            ))
            .unwrap_err();
        assert!(matches!(
            escaped,
            DocumentSessionError::Document(DocumentError::ContactParameterOutOfDomain {
                contact,
                ..
            }) if contact == ids.line_contact
        ));
        assert_eq!(line_circle.export_json().unwrap(), before);

        let (mut circle_arc, ids) = session(AlphaScenarioKind::A4, scale);
        let AlphaScenarioIds::A4(ids) = ids else {
            panic!("A4 IDs expected");
        };
        assert_eq!(
            circle_arc
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            2
        );
        assert!(
            (circle_arc
                .document()
                .scalar(ids.circle_radius)
                .unwrap()
                .value
                - 3.0 * scale)
                .abs()
                <= 2.0e-9 * scale.max(1.0)
        );
        for contact in [ids.circle_contact, ids.arc_contact] {
            let slot = circle_arc.document().contact(contact).unwrap();
            assert_eq!(slot.winding, 0);
            assert_eq!(slot.neighborhood, ContactNeighborhood::Interior);
            assert_eq!(slot.tangent_orientation, Some(TangentOrientation::Opposed));
            let jet = circle_arc.document().evaluate_contact_jet(contact).unwrap();
            assert_point([jet.position.x, jet.position.y], [5.0 * scale, 0.0], scale);
        }
        let moved = circle_arc
            .rebuild_request(
                circle_arc.revision(),
                DocumentSolveRequest::default()
                    .with_drag(ids.circle_center, [8.0 * scale, 1.0 * scale]),
            )
            .unwrap();
        assert!(moved.accepted(), "scale={scale:e}: {:#?}", moved.solve());
        let radial_distance = 65.0_f64.sqrt();
        let expected_contact = [
            40.0 * scale / radial_distance,
            5.0 * scale / radial_distance,
        ];
        let expected_radius = (radial_distance - 5.0) * scale;
        assert!(
            (circle_arc
                .document()
                .scalar(ids.circle_radius)
                .unwrap()
                .value
                - expected_radius)
                .abs()
                <= 2.0e-9 * scale.max(1.0)
        );
        for contact in [ids.circle_contact, ids.arc_contact] {
            let jet = circle_arc.document().evaluate_contact_jet(contact).unwrap();
            assert_point([jet.position.x, jet.position.y], expected_contact, scale);
        }
        let released = circle_arc
            .rebuild_request(circle_arc.revision(), DocumentSolveRequest::default())
            .unwrap();
        assert!(released.accepted());
        assert_eq!(
            released
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            2
        );
        let before = circle_arc.export_json().unwrap();
        let rejected = circle_arc
            .rebuild_request(
                circle_arc.revision(),
                DocumentSolveRequest::default().with_drag(ids.circle_center, [-8.0 * scale, 0.0]),
            )
            .unwrap();
        assert!(
            !rejected.accepted(),
            "scale={scale:e}: {:#?}",
            rejected.solve()
        );
        assert_eq!(circle_arc.export_json().unwrap(), before);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn a5_and_a8_round_trip_preserve_geometry_ids_and_branches() {
    for scale in SCALES {
        let target = [2.0f64.sqrt() * scale, 2.0f64.sqrt() * scale];
        let (mut preview, preview_ids) = session(AlphaScenarioKind::A5, scale);
        let AlphaScenarioIds::A5(preview_ids) = preview_ids else {
            panic!("A5 IDs expected");
        };
        let history = preview.history_len();
        for step in 1..=8 {
            let fraction = f64::from(step) / 8.0;
            let intermediate = [
                2.0 * scale + (target[0] - 2.0 * scale) * fraction,
                target[1] * fraction,
            ];
            let preview_result = preview
                .rebuild_request(
                    preview.revision(),
                    DocumentSolveRequest::default()
                        .without_previous_state_preferences()
                        .with_drag(preview_ids.b, intermediate),
                )
                .unwrap();
            assert!(
                preview_result.accepted(),
                "scale={scale:e}, step={step}: {:#?}",
                preview_result.solve()
            );
        }
        assert_point(
            preview.document().point(preview_ids.b).unwrap().position,
            target,
            scale,
        );
        assert_eq!(preview.history_len(), history);

        let (mut committed, _) = session(AlphaScenarioKind::A5, scale);
        let accepted_preview = preview.document().clone();
        let dragged = committed
            .transact(
                committed.revision(),
                "projected point drag",
                move |document| {
                    *document = accepted_preview;
                    Ok(())
                },
            )
            .unwrap();
        assert!(
            dragged.accepted(),
            "scale={scale:e}: {:#?}",
            dragged.outcome.result.solve()
        );
        assert_point(
            committed.document().point(preview_ids.b).unwrap().position,
            target,
            scale,
        );
        assert_eq!(committed.history_len(), 1);

        let (mut bezier, ids) = session(AlphaScenarioKind::A5, scale);
        let AlphaScenarioIds::A5(ids) = ids else {
            panic!("A5 IDs expected");
        };
        let edited = bezier
            .apply(DocumentCommand::new(
                bezier.revision(),
                DocumentEdit::SetPointPosition {
                    point: ids.controls[1],
                    position: [scale, 0.5 * scale],
                },
            ))
            .unwrap();
        assert!(edited.accepted());
        assert_point(
            bezier.document().point(ids.b).unwrap().position,
            [4.0 * scale / 5.0f64.sqrt(), 2.0 * scale / 5.0f64.sqrt()],
            scale,
        );
        let before = bezier.export_json().unwrap();
        let zero_speed = bezier
            .apply(DocumentCommand::new(
                bezier.revision(),
                DocumentEdit::SetPointPosition {
                    point: ids.controls[1],
                    position: bezier.document().point(ids.controls[0]).unwrap().position,
                },
            ))
            .unwrap_err();
        assert!(
            matches!(
                &zero_speed,
                DocumentSessionError::Document(DocumentError::ContactRegularity {
                    source: geosolve_geometry::CurveRegularityError::ZeroSpeed,
                    ..
                })
            ),
            "{zero_speed}"
        );
        assert_eq!(bezier.export_json().unwrap(), before);

        let (combined, _) = session(AlphaScenarioKind::A8, scale);
        let json = combined.export_json().unwrap();
        let imported_document = SketchDocument::from_json(&json).unwrap();
        assert_eq!(imported_document.to_canonical_json().unwrap(), json);
        let imported = SketchDocumentSession::new(
            imported_document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        assert_valid(&imported.accepted_result());
        assert_eq!(imported.export_json().unwrap(), json);
        assert_eq!(
            imported
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity,
            combined
                .accepted_result()
                .accepted_view()
                .unstable_core_report()
                .right_nullity
        );
        assert!(
            imported
                .document()
                .contacts()
                .iter()
                .all(|contact| { contact.winding == 0 && contact.tangent_orientation.is_some() })
        );
        assert!(
            imported
                .document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::CircularArc { .. }))
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn a6_a7_a9_conflict_history_and_import_failures_are_atomic() {
    for scale in SCALES {
        let (mut conflict, ids) = session(AlphaScenarioKind::A1, scale);
        let AlphaScenarioIds::A1(ids) = ids else {
            panic!("A1 IDs expected");
        };
        let accepted = conflict.export_json().unwrap();
        let width_four = conflict
            .document()
            .dimension(ids.rectangle.dimensions[0])
            .unwrap()
            .source_id;
        let attempted = conflict
            .transact(conflict.revision(), "A6 width-5", |document| {
                let target = document.add_scalar(
                    "A6 width-5 target",
                    5.0 * scale,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                document.add_dimension(
                    "width-5",
                    DocumentDimensionDefinition::CurveLength {
                        curve: geosolve_sketch::CurveSpan::line(ids.rectangle.curves[0]),
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )
            })
            .unwrap();
        assert!(!attempted.accepted());
        assert_eq!(
            attempted
                .outcome
                .result
                .solve()
                .unstable_core_report()
                .hard_validity,
            HardValidity::Invalid
        );
        assert_eq!(
            attempted
                .outcome
                .result
                .solve()
                .unstable_core_report()
                .conflict_diagnostics
                .status,
            DiagnosticStatus::Complete
        );
        let conflicts: BTreeSet<_> = attempted
            .outcome
            .result
            .solve()
            .unstable_core_report()
            .conflicting_sources
            .iter()
            .filter_map(|source| attempted.outcome.result.persistent_core_source(*source))
            .collect();
        let width_five = attempted
            .outcome
            .result
            .attempted_mappings()
            .source_mappings()
            .iter()
            .find(|mapping| mapping.label == "width-5")
            .unwrap()
            .source_id;
        assert_eq!(conflicts, BTreeSet::from([width_four, width_five]));
        assert_eq!(conflict.export_json().unwrap(), accepted);
        assert_eq!(conflict.history_len(), 0);

        let fixture = alpha_scenario(AlphaScenarioKind::A1, scale).unwrap();
        let AlphaScenarioIds::A1(single_ids) = fixture.ids else {
            panic!("A1 IDs expected");
        };
        let mut conflicting_document = fixture.document;
        let width_five_target = conflicting_document
            .add_scalar(
                "A6 width-5 target",
                5.0 * scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let width_five_dimension = conflicting_document
            .add_dimension(
                "width-5",
                DocumentDimensionDefinition::CurveLength {
                    curve: geosolve_sketch::CurveSpan::line(single_ids.rectangle.curves[0]),
                    target: width_five_target,
                },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
        for (removed, expected_width) in [
            (width_five_dimension, 4.0 * scale),
            (single_ids.rectangle.dimensions[0], 5.0 * scale),
        ] {
            let mut remaining = conflicting_document.clone();
            remaining
                .remove_with_owned_state(geosolve_sketch::DocumentObjectId::Dimension(removed))
                .unwrap();
            let solved = SketchDocumentSession::new(
                remaining,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap();
            assert_valid(&solved.accepted_result());
            assert_point(
                solved
                    .document()
                    .point(single_ids.rectangle.points[1])
                    .unwrap()
                    .position,
                [expected_width, 0.0],
                scale,
            );
        }

        let rectangle = conflict
            .apply(DocumentCommand::new(
                conflict.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: ids.rectangle.targets[0],
                    value: 6.0 * scale,
                },
            ))
            .unwrap();
        assert!(rectangle.accepted());
        let height_source = conflict
            .document()
            .dimension(ids.rectangle.dimensions[1])
            .unwrap()
            .source_id;
        assert!(
            conflict
                .apply(DocumentCommand::new(
                    conflict.revision(),
                    DocumentEdit::SetSourceSuppressed {
                        source: height_source,
                        suppressed: true,
                    },
                ))
                .unwrap()
                .accepted()
        );
        let created = conflict
            .apply(DocumentCommand::new(
                conflict.revision(),
                DocumentEdit::CreatePoint {
                    label: "E".into(),
                    position: [9.0 * scale, 9.0 * scale],
                },
            ))
            .unwrap();
        let Some(geosolve_sketch::DocumentCommandEffect::CreatedPoint(e)) = created.effect else {
            panic!("point E expected");
        };
        assert!(
            conflict
                .apply(DocumentCommand::new(
                    conflict.revision(),
                    DocumentEdit::Delete {
                        object: geosolve_sketch::DocumentObjectId::Point(e),
                    },
                ))
                .unwrap()
                .accepted()
        );
        let final_json = conflict.export_json().unwrap();
        for _ in 0..4 {
            conflict.undo(conflict.revision()).unwrap();
        }
        for _ in 0..4 {
            conflict.redo(conflict.revision()).unwrap();
        }
        assert_eq!(conflict.export_json().unwrap(), final_json);

        conflict.undo(conflict.revision()).unwrap();
        let baseline = (
            conflict.export_json().unwrap(),
            conflict.revision(),
            conflict.history_len(),
            conflict.history_cursor(),
            conflict.can_undo(),
            conflict.can_redo(),
            conflict.accepted_result().accepted_view().clone(),
        );
        let malformed_payload = "{not JSON".to_owned();
        let version_payload = baseline.0.replacen("\"version\":4", "\"version\":5", 1);
        let value: serde_json::Value = serde_json::from_str(&baseline.0).unwrap();
        let nonfinite_payload = baseline.0.replacen(
            &format!("\"model_scale\":{}", value["model_scale"]),
            "\"model_scale\":1e999",
            1,
        );
        let mut duplicate: serde_json::Value = serde_json::from_str(&baseline.0).unwrap();
        duplicate["points"][1]["id"] = duplicate["points"][0]["id"].clone();
        let duplicate_payload = serde_json::to_string(&duplicate).unwrap();
        let mut dangling: serde_json::Value = serde_json::from_str(&baseline.0).unwrap();
        dangling["curves"][0]["definition"]["start"] =
            serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
        let dangling_payload = serde_json::to_string(&dangling).unwrap();
        let mut oversized: serde_json::Value = serde_json::from_str(&baseline.0).unwrap();
        oversized["points"][0]["label"] =
            serde_json::Value::String("x".repeat(MAX_LABEL_BYTES + 1));
        let oversized_payload = serde_json::to_string(&oversized).unwrap();

        for payload in [
            &malformed_payload,
            &version_payload,
            &nonfinite_payload,
            &duplicate_payload,
            &dangling_payload,
            &oversized_payload,
        ] {
            assert!(conflict.import_json(conflict.revision(), payload).is_err());
            assert_eq!(conflict.export_json().unwrap(), baseline.0);
            assert_eq!(conflict.revision(), baseline.1);
            assert_eq!(conflict.history_len(), baseline.2);
            assert_eq!(conflict.history_cursor(), baseline.3);
            assert_eq!(conflict.can_undo(), baseline.4);
            assert_eq!(conflict.can_redo(), baseline.5);
            assert_eq!(conflict.accepted_result().accepted_view(), &baseline.6);
        }

        let version = conflict
            .import_json(conflict.revision(), &version_payload)
            .unwrap_err();
        assert!(matches!(
            version,
            DocumentSessionError::Document(DocumentError::UnsupportedVersion {
                actual: 5,
                expected: 4
            })
        ));
        assert!(matches!(
            conflict
                .import_json(conflict.revision(), &duplicate_payload)
                .unwrap_err(),
            DocumentSessionError::Document(DocumentError::DuplicateId(_))
        ));
        assert!(matches!(
            conflict
                .import_json(conflict.revision(), &dangling_payload)
                .unwrap_err(),
            DocumentSessionError::Document(DocumentError::UnknownId { .. })
        ));
        assert!(matches!(
            conflict
                .import_json(conflict.revision(), &nonfinite_payload)
                .unwrap_err(),
            DocumentSessionError::Document(DocumentError::Json(_))
        ));
        assert!(matches!(
            conflict
                .import_json(conflict.revision(), &oversized_payload)
                .unwrap_err(),
            DocumentSessionError::Document(DocumentError::ResourceLimit { .. })
        ));
    }
}
