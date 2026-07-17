use std::collections::BTreeSet;

use geosolve_core::{HardValidity, SolverConfig, StructuralClassification};
use geosolve_geometry::{Point2, Pose2, Vector2};
use geosolve_linkage::{
    BodyId, Linkage, LinkageSource, PlanarBodyId, PlanarDocumentId, PlanarGaugePolicy,
    PlanarLinkageDocument, PlanarLinkageError, PlanarLinkageSession, PlanarSourceKind,
    PlanarWorldActionCertification, VelocityResult, xy_plane_frame,
};

const HARD_TOLERANCE: f64 = 1.0e-9;

#[test]
fn private_to_physical_handoff_and_live_gauge_rebuild_keep_all_views_coherent() {
    let scale = 3.0;
    let (linkage, [first, second, third]) = floating_weld_chain(scale);
    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a0_0000), &linkage)
            .unwrap();
    let persistent = [first, second, third].map(|body| captured.persistent_body(body).unwrap());
    let first_target = document
        .accepted_state()
        .body(persistent[0])
        .unwrap()
        .ambient_pose();
    let mut value: serde_json::Value = serde_json::from_str(&document.to_json().unwrap()).unwrap();
    replace_body_pose(&mut value, persistent[1], [2.4 * scale, -0.2 * scale, 0.55]);
    replace_body_pose(
        &mut value,
        persistent[2],
        [-0.3 * scale, 1.9 * scale, -0.75],
    );
    let perturbed =
        PlanarLinkageDocument::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
    let mut session = PlanarLinkageSession::new(perturbed, SolverConfig::default()).unwrap();

    assert_physical_acceptance(&session);
    assert_gauge_split(&session, 3, 3, 0);
    assert_eq!(session.accepted_result().core_report.rank, 6);
    assert_pose_bits_eq(
        session
            .document()
            .accepted_state()
            .body(persistent[0])
            .unwrap()
            .ambient_pose(),
        first_target,
    );
    assert_session_views_coherent(&session);
    assert_eq!(
        session.core_session().report().accepted_state,
        *session
            .core_session()
            .accepted_hard_linearization()
            .unwrap()
            .accepted_state()
    );
    assert_eq!(
        session.accepted_result().display_audit,
        session.accepted_result().core_report.audit
    );

    let accepted_bits = accepted_body_bits(&session);
    let rank_before = physical_report_signature(&session);
    let audit_before = session.accepted_result().display_audit.clone();
    let source_labels_before = source_labels(&session);
    session
        .set_gauge_policy(
            0,
            PlanarGaugePolicy::ExplicitReferences {
                bodies: vec![persistent[2]],
            },
        )
        .unwrap();

    assert_eq!(session.document().accepted_state().revision(), 1);
    assert_eq!(accepted_body_bits(&session), accepted_bits);
    assert_eq!(physical_report_signature(&session), rank_before);
    assert_eq!(session.accepted_result().display_audit, audit_before);
    assert_eq!(source_labels(&session), source_labels_before);
    assert_eq!(
        session.gauge_report().components[0]
            .numerical_reference
            .unwrap(),
        geosolve_linkage::PlanarGaugeReference {
            body: persistent[2],
            target_pose: session
                .document()
                .accepted_state()
                .body(persistent[2])
                .unwrap()
                .ambient_pose(),
        }
    );
    assert_session_views_coherent(&session);
}

#[test]
fn physical_ground_on_nonlowest_body_prevents_numerical_gauge() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let free_first = linkage
        .add_body(
            "free first",
            Pose2::try_new(Vector2::new(1.0, 2.0), 0.2).unwrap(),
            false,
        )
        .unwrap();
    let grounded_second = linkage
        .add_body(
            "grounded second",
            Pose2::try_new(Vector2::new(-2.0, 0.5), -0.4).unwrap(),
            true,
        )
        .unwrap();
    add_weld_at_world(
        &mut linkage,
        free_first,
        grounded_second,
        Point2::new(0.25, -0.75),
        "grounded weld",
    );
    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a1_0000), &linkage)
            .unwrap();
    let persistent_free = captured.persistent_body(free_first).unwrap();
    let persistent_ground = captured.persistent_body(grounded_second).unwrap();
    assert!(persistent_free < persistent_ground);
    let canonical = document.to_json().unwrap();
    let session = PlanarLinkageSession::new(document.clone(), SolverConfig::default()).unwrap();

    assert_physical_acceptance(&session);
    assert_gauge_split(&session, 0, 0, 0);
    assert_eq!(session.accepted_result().core_report.rank, 3);
    let component = &session.gauge_report().components[0];
    assert_eq!(
        component.world_action,
        PlanarWorldActionCertification::PhysicallyGrounded
    );
    assert_eq!(component.numerical_reference, None);
    assert_eq!(component.physical_ground_sources.len(), 1);
    let structural = &session.accepted_result().core_report.structural;
    assert_eq!(structural.fixed_eliminated_coordinates, 3);
    assert_eq!(structural.eliminated_rows, 3);
    assert_eq!(structural.scalar_rows, 6);
    assert_eq!(structural.active_hard_rows, 3);
    let runtime_ground = session
        .runtime_map()
        .runtime_body(persistent_ground)
        .unwrap();
    assert!(session.runtime().body(runtime_ground).unwrap().grounded());
    assert_eq!(
        session
            .accepted_result()
            .source_mappings
            .iter()
            .filter(|mapping| matches!(mapping.source, LinkageSource::Ground(_)))
            .count(),
        1
    );

    let mut invalid = document;
    assert!(matches!(
        invalid.set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![persistent_free],
        }),
        Err(PlanarLinkageError::InvalidGaugePolicy(_))
    ));
    assert_eq!(invalid.to_json().unwrap(), canonical);
}

#[test]
fn floating_persistent_gauges_are_common_left_se2_equivariant_at_all_scales() {
    for (scale_index, scale) in [1.0e-6, 1.0, 1.0e6].into_iter().enumerate() {
        let (base_linkage, base_bodies) = floating_weld_chain(scale);
        let left = Pose2::try_new(Vector2::new(2.75 * scale, -1.5 * scale), 0.83).unwrap();
        let mut transformed_linkage = base_linkage.clone();
        for body in base_bodies {
            let transformed = left
                .compose(&base_linkage.body(body).unwrap().pose())
                .unwrap();
            transformed_linkage
                .set_body_pose(body, transformed)
                .unwrap();
        }
        let (base_document, base_map) = PlanarLinkageDocument::from_linkage(
            PlanarDocumentId::from_u128(0x17a0_1000 + scale_index as u128 * 0x100),
            &base_linkage,
        )
        .unwrap();
        let (transformed_document, transformed_map) = PlanarLinkageDocument::from_linkage(
            PlanarDocumentId::from_u128(0x17a0_2000 + scale_index as u128 * 0x100),
            &transformed_linkage,
        )
        .unwrap();
        let base_ids = base_bodies.map(|body| base_map.persistent_body(body).unwrap());
        let transformed_ids =
            base_bodies.map(|body| transformed_map.persistent_body(body).unwrap());

        for reference_index in 0..base_ids.len() {
            let mut base_candidate = base_document.clone();
            let mut transformed_candidate = transformed_document.clone();
            if reference_index != 0 {
                base_candidate
                    .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
                        bodies: vec![base_ids[reference_index]],
                    })
                    .unwrap();
                transformed_candidate
                    .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
                        bodies: vec![transformed_ids[reference_index]],
                    })
                    .unwrap();
            }
            let base = PlanarLinkageSession::new(base_candidate, SolverConfig::default()).unwrap();
            let transformed =
                PlanarLinkageSession::new(transformed_candidate, SolverConfig::default()).unwrap();
            assert_physical_acceptance(&base);
            assert_physical_acceptance(&transformed);
            assert_gauge_split(&base, 3, 3, 0);
            assert_gauge_split(&transformed, 3, 3, 0);
            assert_eq!(
                physical_report_signature(&base),
                physical_report_signature(&transformed)
            );
            assert_eq!(
                base.accepted_result().core_report.structural,
                transformed.accepted_result().core_report.structural
            );
            assert_eq!(source_labels(&base), source_labels(&transformed));
            for body_index in 0..base_ids.len() {
                let base_pose = base
                    .document()
                    .accepted_state()
                    .body(base_ids[body_index])
                    .unwrap()
                    .pose()
                    .unwrap();
                let transformed_pose = transformed
                    .document()
                    .accepted_state()
                    .body(transformed_ids[body_index])
                    .unwrap()
                    .pose()
                    .unwrap();
                let expected = left.compose(&base_pose).unwrap();
                assert_pose_close_normalized(expected, transformed_pose, scale);
            }
            for body_index in 1..base_ids.len() {
                let base_relative =
                    persistent_relative_pose(&base, base_ids[0], base_ids[body_index]);
                let transformed_relative = persistent_relative_pose(
                    &transformed,
                    transformed_ids[0],
                    transformed_ids[body_index],
                );
                assert_pose_close_normalized(base_relative, transformed_relative, scale);
            }
            assert!(
                base.accepted_result()
                    .display_audit
                    .sources
                    .iter()
                    .chain(&transformed.accepted_result().display_audit.sources)
                    .all(|source| !source.source_label.contains("numerical gauge"))
            );
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn disconnected_persistent_velocity_is_component_local() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let driven_first = linkage
        .add_body("driven first", Pose2::identity(), false)
        .unwrap();
    let driven_second = linkage
        .add_body(
            "driven second",
            Pose2::try_new(Vector2::new(1.0, 0.0), 0.35).unwrap(),
            false,
        )
        .unwrap();
    let first_pin = linkage
        .add_point_feature("driven first pin", driven_first, Point2::new(1.0, 0.0))
        .unwrap();
    let second_pin = linkage
        .add_point_feature("driven second pin", driven_second, Point2::new(0.0, 0.0))
        .unwrap();
    linkage
        .add_revolute_joint("driven revolute", first_pin, second_pin)
        .unwrap();
    let driver = linkage
        .add_angular_driver(
            "driven relative angle",
            driven_first,
            driven_second,
            0.35,
            0.1,
        )
        .unwrap();

    let welded_first = linkage
        .add_body(
            "welded first",
            Pose2::try_new(Vector2::new(5.0, -1.0), -0.2).unwrap(),
            false,
        )
        .unwrap();
    let welded_second = linkage
        .add_body(
            "welded second",
            Pose2::try_new(Vector2::new(6.0, 0.5), 0.45).unwrap(),
            false,
        )
        .unwrap();
    add_weld_at_world(
        &mut linkage,
        welded_first,
        welded_second,
        Point2::new(5.5, 0.25),
        "unselected weld",
    );
    let isolated = linkage
        .add_body(
            "isolated floating",
            Pose2::try_new(Vector2::new(-4.0, 3.0), 0.7).unwrap(),
            false,
        )
        .unwrap();
    let ground = linkage
        .add_body(
            "isolated ground",
            Pose2::try_new(Vector2::new(9.0, -2.0), -0.5).unwrap(),
            true,
        )
        .unwrap();

    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a2_0000), &linkage)
            .unwrap();
    let persistent_driver = captured
        .persistent_source(LinkageSource::Driver(driver))
        .unwrap();
    let persistent_driven_first = captured.persistent_body(driven_first).unwrap();
    let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
    assert_physical_acceptance(&session);
    assert_gauge_split(&session, 9, 9, 0);
    assert_eq!(session.accepted_result().core_report.rank, 6);

    let velocity = session.velocity(persistent_driver, 1.25).unwrap();
    assert_velocity_report_matches_session(&session, &velocity);
    let driven_reference = session
        .runtime_map()
        .runtime_body(persistent_driven_first)
        .unwrap();
    assert_zero_velocity(&velocity, driven_reference);
    let driven_other = velocity
        .body(runtime_body(&session, &captured, driven_second))
        .unwrap();
    assert!((driven_other.angular - 1.25).abs() <= 1.0e-10);
    for body in [welded_first, welded_second, isolated, ground] {
        assert_zero_velocity(&velocity, runtime_body(&session, &captured, body));
    }
    assert!(velocity.differentiated_residual_max <= HARD_TOLERANCE);

    let core_indices = session
        .gauge_report()
        .components
        .iter()
        .flat_map(|component| component.core_component_indices.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(
        core_indices.iter().copied().collect::<BTreeSet<_>>().len(),
        core_indices.len()
    );
    assert_eq!(
        core_indices.len(),
        session.core_session().report().component_solves.len()
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn alternative_velocity_gauges_differ_by_one_common_world_twist_and_invalid_edit_is_atomic() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first_pose = Pose2::try_new(Vector2::new(2.0, -1.0), 0.3).unwrap();
    let second_pose = Pose2::try_new(Vector2::new(-1.0, 3.0), -0.4).unwrap();
    let first = linkage.add_body("first", first_pose, false).unwrap();
    let second = linkage.add_body("second", second_pose, false).unwrap();
    let joint = Point2::new(0.5, 1.2);
    let first_pin = linkage
        .add_point_feature(
            "first pin",
            first,
            first_pose.inverse_transform_point(joint),
        )
        .unwrap();
    let second_pin = linkage
        .add_point_feature(
            "second pin",
            second,
            second_pose.inverse_transform_point(joint),
        )
        .unwrap();
    linkage
        .add_revolute_joint("offset revolute", first_pin, second_pin)
        .unwrap();
    let driver = linkage
        .add_angular_driver(
            "offset relative angle",
            first,
            second,
            second_pose.angle - first_pose.angle,
            0.1,
        )
        .unwrap();
    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a3_0000), &linkage)
            .unwrap();
    let persistent_first = captured.persistent_body(first).unwrap();
    let persistent_second = captured.persistent_body(second).unwrap();
    let persistent_driver = captured
        .persistent_source(LinkageSource::Driver(driver))
        .unwrap();
    let config = SolverConfig {
        rank_relative_tolerance: 1.0e-8,
        ..SolverConfig::default()
    };
    let mut automatic = PlanarLinkageSession::new(document.clone(), config).unwrap();
    let mut explicit_document = document;
    explicit_document
        .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![persistent_second],
        })
        .unwrap();
    let explicit = PlanarLinkageSession::new(explicit_document, config).unwrap();
    let rate = 0.8;
    let automatic_velocity = automatic.velocity(persistent_driver, rate).unwrap();
    let explicit_velocity = explicit.velocity(persistent_driver, rate).unwrap();
    assert_velocity_report_matches_session(&automatic, &automatic_velocity);
    assert_velocity_report_matches_session(&explicit, &explicit_velocity);

    let automatic_first = automatic
        .runtime_map()
        .runtime_body(persistent_first)
        .unwrap();
    let automatic_second = automatic
        .runtime_map()
        .runtime_body(persistent_second)
        .unwrap();
    let explicit_first = explicit
        .runtime_map()
        .runtime_body(persistent_first)
        .unwrap();
    let explicit_second = explicit
        .runtime_map()
        .runtime_body(persistent_second)
        .unwrap();
    assert_zero_velocity(&automatic_velocity, automatic_first);
    assert_zero_velocity(&explicit_velocity, explicit_second);
    let first_delta = explicit_velocity.body(explicit_first).unwrap();
    let first_base = automatic_velocity.body(automatic_first).unwrap();
    let gauge_linear = first_delta.linear - first_base.linear;
    let gauge_angular = first_delta.angular - first_base.angular;
    let first_origin = automatic
        .runtime()
        .body(automatic_first)
        .unwrap()
        .pose()
        .translation;
    for (persistent, automatic_body, explicit_body) in [
        (persistent_first, automatic_first, explicit_first),
        (persistent_second, automatic_second, explicit_second),
    ] {
        let automatic_value = automatic_velocity.body(automatic_body).unwrap();
        let explicit_value = explicit_velocity.body(explicit_body).unwrap();
        let origin = automatic
            .runtime()
            .body(automatic.runtime_map().runtime_body(persistent).unwrap())
            .unwrap()
            .pose()
            .translation;
        let expected_linear = gauge_linear + perpendicular(origin - first_origin) * gauge_angular;
        assert!(
            (explicit_value.linear - automatic_value.linear - expected_linear).norm() <= 1.0e-10
        );
        assert!(
            (explicit_value.angular - automatic_value.angular - gauge_angular).abs() <= 1.0e-10
        );
    }

    let json_before = automatic.document().to_json().unwrap();
    let runtime_map_before = automatic.runtime_map().clone();
    let geometry_before = automatic.runtime().geometry().unwrap();
    let result_geometry_before = automatic.accepted_result().geometry.clone();
    let audit_before = automatic.accepted_result().display_audit.clone();
    let gauge_before = automatic.gauge_report().clone();
    let linearization_before = automatic
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    assert!(matches!(
        automatic.set_gauge_policy(
            0,
            PlanarGaugePolicy::ExplicitReferences {
                bodies: vec![PlanarBodyId::from_u128(u128::MAX - 1)],
            },
        ),
        Err(PlanarLinkageError::InvalidGaugePolicy(_))
    ));
    assert_eq!(automatic.document().to_json().unwrap(), json_before);
    assert_eq!(automatic.runtime_map(), &runtime_map_before);
    assert_eq!(automatic.runtime().geometry().unwrap(), geometry_before);
    assert_eq!(automatic.accepted_result().geometry, result_geometry_before);
    assert_eq!(automatic.accepted_result().display_audit, audit_before);
    assert_eq!(automatic.gauge_report(), &gauge_before);
    assert_eq!(
        automatic
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        linearization_before
    );
    assert_eq!(
        automatic.velocity(persistent_driver, rate).unwrap(),
        automatic_velocity
    );
    automatic
        .set_gauge_policy(
            0,
            PlanarGaugePolicy::ExplicitReferences {
                bodies: vec![persistent_second],
            },
        )
        .unwrap();
}

#[test]
fn floating_rank_boundary_uses_physical_component_threshold_before_gauge_split() {
    let config = SolverConfig {
        normalized_residual_tolerance: 1.0e-12,
        rank_relative_tolerance: 1.0e-6,
        ..SolverConfig::default()
    };
    for (scale_index, scale) in [1.0e-6, 1.0, 1.0e6].into_iter().enumerate() {
        for (
            case_index,
            (alpha, expected_rank, expected_left, expected_right, expected_internal),
        ) in [(4.0e-6, 3, 1, 3, 0), (1.0e-6, 2, 2, 4, 1)]
            .into_iter()
            .enumerate()
        {
            let linkage = rank_boundary_linkage(scale, alpha);
            let (document, _) = PlanarLinkageDocument::from_linkage(
                PlanarDocumentId::from_u128(
                    0x17a4_0000 + scale_index as u128 * 0x100 + case_index as u128,
                ),
                &linkage,
            )
            .unwrap();
            let session = PlanarLinkageSession::new(document, config).unwrap();
            assert_physical_acceptance(&session);
            let report = &session.accepted_result().core_report;
            assert_eq!(report.rank, expected_rank);
            assert_eq!(report.left_nullity, expected_left);
            assert_eq!(report.right_nullity, expected_right);
            assert_gauge_split(&session, expected_right, 3, expected_internal);
            assert_eq!(report.component_solves.len(), 1);
            let component = &report.component_solves[0];
            assert!(component.rank_machine_tolerance < component.rank_threshold);
            assert!((component.sigma_max - 2.0).abs() <= 1.0e-12);
            let mut spectrum = component.singular_values.clone();
            spectrum.sort_by(|first, second| second.total_cmp(first));
            assert_eq!(spectrum.len(), 4);
            if expected_rank == 3 {
                assert!(spectrum[2] > component.rank_threshold);
            } else {
                assert!(spectrum[2] < component.rank_threshold);
            }
            assert!(spectrum[3] <= component.rank_threshold);
            let structural = &report.structural;
            assert_eq!(structural.active_hard_rows, 4);
            assert_eq!(structural.active_tangent_dimensions, 6);
            assert_eq!(structural.structural_nnz, 24);
            assert_eq!(structural.structural_rank, 4);
            assert_eq!(structural.structural_left_nullity, 0);
            assert_eq!(structural.structural_right_nullity, 2);
            assert_eq!(
                structural.structural_classification,
                StructuralClassification::Under
            );
            let accepted = session
                .core_session()
                .accepted_hard_linearization()
                .unwrap();
            assert_eq!(accepted.components().len(), 1);
            assert_eq!(
                accepted.components()[0].normalized_jacobian().shape(),
                (4, 6)
            );
            assert_eq!(accepted.components()[0].rank(), expected_rank);
            assert_eq!(accepted.components()[0].right_nullity(), expected_right);
        }
    }
}

#[test]
fn malformed_multicomponent_gauge_json_is_rejected_semantically() {
    let (linkage, floating_pairs, ground) = two_floating_welds_and_ground();
    let (mut document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a5_0000), &linkage)
            .unwrap();
    let pairs = floating_pairs.map(|pair| pair.map(|body| captured.persistent_body(body).unwrap()));
    let persistent_ground = captured.persistent_body(ground).unwrap();
    document
        .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![pairs[0][0], pairs[1][0]],
        })
        .unwrap();
    let canonical = document.to_json().unwrap();
    PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();

    let mut mutations = Vec::new();
    let mut missing: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    missing["gauge_policy"]["bodies"] = serde_json::json!([pairs[0][0].to_string()]);
    mutations.push(missing);
    let mut duplicate_component: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    duplicate_component["gauge_policy"]["bodies"] = serde_json::json!([
        pairs[0][0].to_string(),
        pairs[0][1].to_string(),
        pairs[1][0].to_string()
    ]);
    mutations.push(duplicate_component);
    let mut grounded: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    grounded["gauge_policy"]["bodies"] = serde_json::json!([
        pairs[0][0].to_string(),
        pairs[1][0].to_string(),
        persistent_ground.to_string()
    ]);
    mutations.push(grounded);
    let mut unknown: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    unknown["gauge_policy"]["bodies"] = serde_json::json!([
        pairs[0][0].to_string(),
        pairs[1][0].to_string(),
        format!("{:032x}", u128::MAX - 2)
    ]);
    mutations.push(unknown);
    for mutation in mutations {
        assert!(matches!(
            PlanarLinkageDocument::from_json(&serde_json::to_string(&mutation).unwrap()),
            Err(PlanarLinkageError::InvalidGaugePolicy(_))
        ));
    }

    let mut injected: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    injected["gauge_policy"]["target_pose"] = serde_json::json!([0.0, 0.0, 0.0]);
    assert!(matches!(
        PlanarLinkageDocument::from_json(&serde_json::to_string(&injected).unwrap()),
        Err(PlanarLinkageError::Json(_))
    ));
    let mut private_kind: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    private_kind["gauge_policy"]["kind"] = serde_json::json!("private_fixed_pose");
    assert!(matches!(
        PlanarLinkageDocument::from_json(&serde_json::to_string(&private_kind).unwrap()),
        Err(PlanarLinkageError::Json(_))
    ));
    assert_eq!(
        PlanarLinkageDocument::from_json(&canonical)
            .unwrap()
            .to_json()
            .unwrap(),
        canonical
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn private_gauge_rows_never_leak_into_physical_reports_or_diagnostics() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), false).unwrap();
    let second = linkage
        .add_body(
            "second",
            Pose2::try_new(Vector2::new(2.0, 0.0), 0.4).unwrap(),
            false,
        )
        .unwrap();
    let first_anchor = linkage
        .add_point_feature("first anchor", first, Point2::new(2.0, 0.0))
        .unwrap();
    let second_anchor = linkage
        .add_point_feature("second anchor", second, Point2::new(0.0, 0.0))
        .unwrap();
    linkage
        .add_weld_joint_with_angle("primary weld", first_anchor, second_anchor, 0.4)
        .unwrap();
    linkage
        .add_weld_joint_with_angle("duplicate weld", first_anchor, second_anchor, 0.4)
        .unwrap();
    let (document, _) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x17a6_0000), &linkage)
            .unwrap();
    let json = document.to_json().unwrap();
    assert!(!json.contains("private numerical gauge"));
    assert!(
        document
            .topology()
            .sources()
            .iter()
            .all(|source| matches!(source.definition(), PlanarSourceKind::Weld { .. }))
    );
    let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
    assert_physical_acceptance(&session);
    assert_gauge_split(&session, 3, 3, 0);
    let report = &session.accepted_result().core_report;
    assert_eq!(report.rank, 3);
    assert_eq!(report.left_nullity, 3);
    assert_eq!(report.structural.scalar_rows, 6);
    assert_eq!(report.structural.fixed_eliminated_coordinates, 0);
    assert_eq!(report.structural.eliminated_rows, 0);
    assert_eq!(report.structural.active_hard_rows, 6);
    assert!(session.runtime().bodies().all(|(_, body)| !body.grounded()));
    assert_eq!(session.accepted_result().source_mappings.len(), 2);
    assert!(
        session
            .accepted_result()
            .source_mappings
            .iter()
            .all(|mapping| matches!(mapping.source, LinkageSource::Joint(_)))
    );
    assert_eq!(session.accepted_result().display_audit.sources.len(), 2);
    assert_eq!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .map(|source| source.rows.len())
            .sum::<usize>(),
        6
    );
    assert!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .all(|source| !source.source_label.contains("private numerical gauge"))
    );
    let accepted = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    assert_eq!(accepted.components().len(), 1);
    assert_eq!(accepted.components()[0].hard_rows().len(), 6);
    let physical_sources = session
        .accepted_result()
        .source_mappings
        .iter()
        .map(|mapping| mapping.core_source_id)
        .collect::<BTreeSet<_>>();
    assert!(
        accepted.components()[0]
            .hard_rows()
            .iter()
            .all(|row| physical_sources.contains(&row.row.source_id))
    );
    assert!(
        report
            .conflicting_sources
            .iter()
            .chain(&report.redundant_sources)
            .chain(&report.sources_containing_redundant_rows)
            .all(|source| physical_sources.contains(source))
    );
    assert!(
        report
            .singular_rows
            .iter()
            .all(|row| physical_sources.contains(&row.source_id))
    );
    assert!(!report.redundant_sources.is_empty());
}

fn floating_weld_chain(scale: f64) -> (Linkage, [BodyId; 3]) {
    let mut linkage = Linkage::new(scale, xy_plane_frame()).unwrap();
    let first = linkage
        .add_body(
            "first",
            Pose2::try_new(Vector2::new(-scale, 0.5 * scale), 0.2).unwrap(),
            false,
        )
        .unwrap();
    let second = linkage
        .add_body(
            "second",
            Pose2::try_new(Vector2::new(1.5 * scale, -0.4 * scale), -0.35).unwrap(),
            false,
        )
        .unwrap();
    let third = linkage
        .add_body(
            "third",
            Pose2::try_new(Vector2::new(0.25 * scale, 1.4 * scale), 0.65).unwrap(),
            false,
        )
        .unwrap();
    add_weld_at_world(
        &mut linkage,
        first,
        second,
        Point2::new(0.3 * scale, -0.1 * scale),
        "first-second weld",
    );
    add_weld_at_world(
        &mut linkage,
        second,
        third,
        Point2::new(0.8 * scale, 0.9 * scale),
        "second-third weld",
    );
    (linkage, [first, second, third])
}

fn rank_boundary_linkage(scale: f64, alpha: f64) -> Linkage {
    let mut linkage = Linkage::new(scale, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), false).unwrap();
    let second = linkage
        .add_body("second", Pose2::identity(), false)
        .unwrap();
    let half = 0.5 * alpha * scale;
    let first_minus = linkage
        .add_point_feature("first minus", first, Point2::new(-half, 0.0))
        .unwrap();
    let second_minus = linkage
        .add_point_feature("second minus", second, Point2::new(-half, 0.0))
        .unwrap();
    let first_plus = linkage
        .add_point_feature("first plus", first, Point2::new(half, 0.0))
        .unwrap();
    let second_plus = linkage
        .add_point_feature("second plus", second, Point2::new(half, 0.0))
        .unwrap();
    linkage
        .add_revolute_joint("minus closure", first_minus, second_minus)
        .unwrap();
    linkage
        .add_revolute_joint("plus closure", first_plus, second_plus)
        .unwrap();
    linkage
}

fn two_floating_welds_and_ground() -> (Linkage, [[BodyId; 2]; 2], BodyId) {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first_a = linkage
        .add_body("first a", Pose2::identity(), false)
        .unwrap();
    let first_b = linkage
        .add_body(
            "first b",
            Pose2::try_new(Vector2::new(1.0, 0.0), 0.2).unwrap(),
            false,
        )
        .unwrap();
    let second_a = linkage
        .add_body(
            "second a",
            Pose2::try_new(Vector2::new(5.0, 0.0), -0.1).unwrap(),
            false,
        )
        .unwrap();
    let second_b = linkage
        .add_body(
            "second b",
            Pose2::try_new(Vector2::new(6.0, 0.5), 0.3).unwrap(),
            false,
        )
        .unwrap();
    let ground = linkage
        .add_body(
            "ground",
            Pose2::try_new(Vector2::new(-4.0, -2.0), 0.7).unwrap(),
            true,
        )
        .unwrap();
    add_weld_at_world(
        &mut linkage,
        first_a,
        first_b,
        Point2::new(0.5, 0.25),
        "first weld",
    );
    add_weld_at_world(
        &mut linkage,
        second_a,
        second_b,
        Point2::new(5.5, -0.25),
        "second weld",
    );
    (linkage, [[first_a, first_b], [second_a, second_b]], ground)
}

fn add_weld_at_world(
    linkage: &mut Linkage,
    first: BodyId,
    second: BodyId,
    world_anchor: Point2<f64>,
    label: &str,
) {
    let first_pose = linkage.body(first).unwrap().pose();
    let second_pose = linkage.body(second).unwrap().pose();
    let first_anchor = linkage
        .add_point_feature(
            format!("{label} first"),
            first,
            first_pose.inverse_transform_point(world_anchor),
        )
        .unwrap();
    let second_anchor = linkage
        .add_point_feature(
            format!("{label} second"),
            second,
            second_pose.inverse_transform_point(world_anchor),
        )
        .unwrap();
    linkage
        .add_weld_joint_with_angle(
            label,
            first_anchor,
            second_anchor,
            second_pose.angle - first_pose.angle,
        )
        .unwrap();
}

fn replace_body_pose(value: &mut serde_json::Value, body: PlanarBodyId, pose: [f64; 3]) {
    let body = body.to_string();
    let state = value["accepted_state"]["bodies"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|state| state["body"].as_str() == Some(body.as_str()))
        .unwrap();
    state["pose"] = serde_json::json!(pose);
}

fn runtime_body(
    session: &PlanarLinkageSession,
    captured: &geosolve_linkage::PlanarLinkageRuntimeMap,
    original: BodyId,
) -> BodyId {
    let persistent = captured.persistent_body(original).unwrap();
    session.runtime_map().runtime_body(persistent).unwrap()
}

fn assert_physical_acceptance(session: &PlanarLinkageSession) {
    let result = session.accepted_result();
    assert!(result.accepted());
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= HARD_TOLERANCE);
    assert!(result.core_report.rank_is_valid);
    assert!(
        result
            .acceptance_hard_residual_max
            .is_some_and(|maximum| maximum <= HARD_TOLERANCE)
    );
}

fn assert_gauge_split(
    session: &PlanarLinkageSession,
    right_nullity: usize,
    gauge_dof: usize,
    internal_mobility: usize,
) {
    let report = session.gauge_report();
    assert_eq!(report.numerical_equality_right_nullity, right_nullity);
    assert_eq!(report.gauge_dof, gauge_dof);
    assert_eq!(report.internal_mobility, internal_mobility);
    assert_eq!(right_nullity, gauge_dof + internal_mobility);
    assert_eq!(right_nullity, session.core_session().report().right_nullity);
}

fn assert_session_views_coherent(session: &PlanarLinkageSession) {
    let runtime_geometry = session.runtime().geometry().unwrap();
    for state in session.document().accepted_state().bodies() {
        let runtime = session.runtime_map().runtime_body(state.body()).unwrap();
        let document_pose = state.ambient_pose();
        assert_pose_bits_eq(
            session.runtime().body(runtime).unwrap().pose().ambient(),
            document_pose,
        );
        assert_pose_bits_eq(
            session
                .accepted_result()
                .geometry
                .body_pose(runtime)
                .unwrap()
                .ambient(),
            document_pose,
        );
        assert_pose_bits_eq(
            runtime_geometry.body_pose(runtime).unwrap().ambient(),
            document_pose,
        );
    }
}

fn assert_velocity_report_matches_session(
    session: &PlanarLinkageSession,
    velocity: &VelocityResult,
) {
    let report = session.core_session().report();
    assert_eq!(velocity.rank_is_valid, report.rank_is_valid);
    assert_eq!(velocity.rank, report.rank);
    assert_eq!(velocity.local_degrees_of_freedom, report.right_nullity);
    assert_eq!(velocity.is_singular, report.is_singular);
    assert_eq!(
        velocity.rank_relative_tolerance.to_bits(),
        report.rank_relative_tolerance.to_bits()
    );
    assert_eq!(
        velocity.rank_threshold.to_bits(),
        report.rank_threshold.to_bits()
    );
    assert_eq!(velocity.singular_values, report.singular_values);
}

fn assert_zero_velocity(velocity: &VelocityResult, body: BodyId) {
    let value = velocity.body(body).unwrap();
    assert_eq!(value.linear, Vector2::zeros());
    assert_eq!(value.angular.to_bits(), 0.0_f64.to_bits());
}

fn accepted_body_bits(session: &PlanarLinkageSession) -> Vec<(PlanarBodyId, [u64; 3])> {
    session
        .document()
        .accepted_state()
        .bodies()
        .iter()
        .map(|state| (state.body(), state.ambient_pose().map(f64::to_bits)))
        .collect()
}

fn physical_report_signature(session: &PlanarLinkageSession) -> (usize, usize, usize, bool) {
    let report = &session.accepted_result().core_report;
    (
        report.rank,
        report.left_nullity,
        report.right_nullity,
        report.is_singular,
    )
}

fn source_labels(session: &PlanarLinkageSession) -> Vec<String> {
    session
        .accepted_result()
        .source_mappings
        .iter()
        .map(|mapping| mapping.source_label.clone())
        .collect()
}

fn assert_pose_bits_eq(first: [f64; 3], second: [f64; 3]) {
    assert_eq!(first.map(f64::to_bits), second.map(f64::to_bits));
}

fn persistent_relative_pose(
    session: &PlanarLinkageSession,
    first: PlanarBodyId,
    second: PlanarBodyId,
) -> Pose2 {
    let first = session
        .document()
        .accepted_state()
        .body(first)
        .unwrap()
        .pose()
        .unwrap();
    let second = session
        .document()
        .accepted_state()
        .body(second)
        .unwrap()
        .pose()
        .unwrap();
    first.inverse().unwrap().compose(&second).unwrap()
}

fn assert_pose_close_normalized(first: Pose2, second: Pose2, model_scale: f64) {
    let difference = first.local_difference(&second).unwrap();
    assert!(difference[0].abs() / model_scale <= HARD_TOLERANCE);
    assert!(difference[1].abs() / model_scale <= HARD_TOLERANCE);
    assert!(difference[2].abs() <= HARD_TOLERANCE);
}

fn perpendicular(vector: Vector2<f64>) -> Vector2<f64> {
    Vector2::new(-vector.y, vector.x)
}
