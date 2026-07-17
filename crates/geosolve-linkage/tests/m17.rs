use geosolve_core::SolverConfig;
use geosolve_geometry::{Point2, Pose2, Vector2};
use geosolve_linkage::{
    BranchSign, Linkage, LinkageSource, PlanarBodyId, PlanarDocumentId, PlanarGaugePolicy,
    PlanarLinkageDocument, PlanarLinkageError, PlanarLinkageSession, PlanarRuntimeSource,
    PlanarSourceKind, PlanarWorldActionCertification, four_bar_crossed, four_bar_open,
    slider_crank, xy_plane_frame,
};

const TOLERANCE: f64 = 1.0e-10;

fn assert_pose_close(first: [f64; 3], second: [f64; 3]) {
    for (first, second) in first.into_iter().zip(second) {
        assert!(
            (first - second).abs() <= TOLERANCE,
            "{first:?} != {second:?}"
        );
    }
}

#[test]
fn l1_l2_l3_persistent_sessions_preserve_geometry_rank_sources_and_local_features() {
    let fixtures: Vec<Linkage> = vec![
        four_bar_open().unwrap().0,
        four_bar_crossed().unwrap().0,
        slider_crank().unwrap().0,
    ];
    for (index, mut linkage) in fixtures.into_iter().enumerate() {
        let baseline = linkage.solve(SolverConfig::default()).unwrap();
        assert!(baseline.accepted(), "{:#?}", baseline.rejection);
        let baseline_rank = (
            baseline.core_report.rank,
            baseline.core_report.left_nullity,
            baseline.core_report.right_nullity,
            baseline.core_report.structural.clone(),
        );
        let baseline_labels = baseline
            .source_mappings
            .iter()
            .map(|mapping| mapping.source_label.clone())
            .collect::<Vec<_>>();
        let document_id = PlanarDocumentId::from_u128(0x1700_0000 + index as u128 * 0x1000);
        let (document, captured_map) =
            PlanarLinkageDocument::from_linkage(document_id, &linkage).unwrap();

        assert_eq!(document.version(), 1);
        assert_eq!(document.topology().bodies().len(), linkage.bodies().count());
        assert_eq!(
            document.accepted_state().bodies().len(),
            document.topology().bodies().len()
        );
        assert!(
            document
                .topology()
                .point_features()
                .iter()
                .all(|feature| feature.local_point().iter().all(|value| value.is_finite()))
        );
        assert!(
            document
                .topology()
                .axis_features()
                .iter()
                .all(|feature| feature.local_axis().iter().all(|value| value.is_finite()))
        );

        let json = document.to_json().unwrap();
        assert_eq!(
            PlanarLinkageDocument::from_json(&json)
                .unwrap()
                .to_json()
                .unwrap(),
            json
        );
        assert!(!json.contains("BodyId"));
        assert!(!json.contains("VariableId"));

        let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
        assert!(session.accepted_result().accepted());
        assert_eq!(
            (
                session.accepted_result().core_report.rank,
                session.accepted_result().core_report.left_nullity,
                session.accepted_result().core_report.right_nullity,
                session.accepted_result().core_report.structural.clone(),
            ),
            baseline_rank
        );
        assert_eq!(
            session
                .accepted_result()
                .source_mappings
                .iter()
                .map(|mapping| mapping.source_label.clone())
                .collect::<Vec<_>>(),
            baseline_labels
        );
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap();

        for body in document_body_ids(session.document()) {
            let original = captured_map.runtime_body(body).unwrap();
            let lowered = session.runtime_map().runtime_body(body).unwrap();
            assert_pose_close(
                linkage.body(original).unwrap().pose().ambient(),
                session.runtime().body(lowered).unwrap().pose().ambient(),
            );
        }
    }
}

#[test]
fn persistent_source_and_feature_maps_survive_deterministic_remapping() {
    let (linkage, ids) = slider_crank().unwrap();
    let (document, captured_map) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1710_0000), &linkage)
            .unwrap();
    let persistent_driver = captured_map
        .persistent_source(LinkageSource::Driver(ids.driver))
        .unwrap();
    let persistent_crank = captured_map.persistent_body(ids.crank).unwrap();
    let persistent_pin = captured_map
        .persistent_point_feature(ids.slider_pin)
        .unwrap();
    assert_eq!(persistent_driver.to_string().len(), 32);
    assert_eq!(persistent_crank.to_string().len(), 32);
    assert_eq!(persistent_pin.to_string().len(), 32);
    assert!(matches!(
        document
            .topology()
            .source(persistent_driver)
            .unwrap()
            .definition(),
        PlanarSourceKind::AngularDriver { .. }
    ));

    let (first, first_map) = document.lower().unwrap();
    let (second, second_map) = document.lower().unwrap();
    let first_driver = match first_map.runtime_source(persistent_driver).unwrap() {
        PlanarRuntimeSource::Driver(driver) => driver,
        source => panic!("driver mapping expected, got {source:?}"),
    };
    let second_driver = match second_map.runtime_source(persistent_driver).unwrap() {
        PlanarRuntimeSource::Driver(driver) => driver,
        source => panic!("driver mapping expected, got {source:?}"),
    };
    assert_eq!(
        first.driver(first_driver).unwrap().target().to_bits(),
        second.driver(second_driver).unwrap().target().to_bits()
    );
    assert_eq!(
        first
            .body(first_map.runtime_body(persistent_crank).unwrap())
            .unwrap()
            .pose()
            .ambient()
            .map(f64::to_bits),
        second
            .body(second_map.runtime_body(persistent_crank).unwrap())
            .unwrap()
            .pose()
            .ambient()
            .map(f64::to_bits)
    );
    assert!(matches!(
        first_map.runtime_feature(persistent_pin),
        Some(geosolve_linkage::PlanarRuntimeFeature::Point(_))
    ));
}

#[test]
fn canonical_json_reorders_records_and_rejects_duplicates_unknown_fields_and_versions() {
    let (linkage, _) = slider_crank().unwrap();
    let (document, _) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1720_0000), &linkage)
            .unwrap();
    let canonical = document.to_json().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    for pointer in [
        "/topology/bodies",
        "/topology/point_features",
        "/topology/axis_features",
        "/topology/sources",
        "/accepted_state/bodies",
        "/accepted_state/drivers",
    ] {
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .reverse();
    }
    let reordered = serde_json::to_string(&value).unwrap();
    assert_eq!(
        PlanarLinkageDocument::from_json(&reordered)
            .unwrap()
            .to_json()
            .unwrap(),
        canonical
    );

    let mut duplicate: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    let body_id = duplicate["topology"]["bodies"][0]["id"].clone();
    duplicate["topology"]["point_features"][0]["id"] = body_id;
    assert!(matches!(
        PlanarLinkageDocument::from_json(&serde_json::to_string(&duplicate).unwrap()),
        Err(PlanarLinkageError::DuplicateId(_))
    ));

    let mut unknown: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    unknown["unexpected"] = serde_json::json!(true);
    assert!(matches!(
        PlanarLinkageDocument::from_json(&serde_json::to_string(&unknown).unwrap()),
        Err(PlanarLinkageError::Json(_))
    ));

    let mut unsupported: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    unsupported["version"] = serde_json::json!(2);
    assert!(matches!(
        PlanarLinkageDocument::from_json(&serde_json::to_string(&unsupported).unwrap()),
        Err(PlanarLinkageError::UnsupportedVersion(2))
    ));

    let mut relabeled: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    let ground = relabeled["topology"]["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|source| source["definition"]["kind"] == "physical_ground")
        .unwrap();
    ground["label"] = serde_json::json!("persistent base fixture");
    let relabeled =
        PlanarLinkageDocument::from_json(&serde_json::to_string(&relabeled).unwrap()).unwrap();
    let session = PlanarLinkageSession::new(relabeled, SolverConfig::default()).unwrap();
    assert!(
        session
            .accepted_result()
            .source_mappings
            .iter()
            .any(|mapping| mapping.source_label == "persistent base fixture")
    );
    assert!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .any(|source| source.source_label == "persistent base fixture")
    );
}

#[test]
fn finite_but_incompatible_import_never_becomes_accepted_session_state() {
    let (linkage, ids) = four_bar_open().unwrap();
    let (document, mappings) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1730_0000), &linkage)
            .unwrap();
    let coupler = mappings.persistent_body(ids.coupler).unwrap().to_string();
    let mut value: serde_json::Value = serde_json::from_str(&document.to_json().unwrap()).unwrap();
    let states = value["accepted_state"]["bodies"].as_array_mut().unwrap();
    let state = states
        .iter_mut()
        .find(|state| state["body"].as_str() == Some(coupler.as_str()))
        .unwrap();
    state["pose"][0] = serde_json::json!(100.0);
    let candidate =
        PlanarLinkageDocument::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
    assert!(matches!(
        PlanarLinkageSession::new(candidate, SolverConfig::default()),
        Err(PlanarLinkageError::InitialRejected(_))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn floating_weld_reports_three_gauge_dof_and_alternative_references_preserve_relative_geometry() {
    for (index, scale) in [1.0e-6, 1.0, 1.0e6].into_iter().enumerate() {
        let (linkage, first_runtime, second_runtime) = floating_weld(scale);
        let (document, captured) = PlanarLinkageDocument::from_linkage(
            PlanarDocumentId::from_u128(0x1740_0000 + index as u128 * 0x1000),
            &linkage,
        )
        .unwrap();
        let first = captured.persistent_body(first_runtime).unwrap();
        let second = captured.persistent_body(second_runtime).unwrap();
        let perturbed = perturb_body_state(&document, second, [2.0 * scale, 0.35 * scale, 0.65]);

        let automatic =
            PlanarLinkageSession::new(perturbed.clone(), SolverConfig::default()).unwrap();
        let mut explicit_document = perturbed;
        explicit_document
            .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
                bodies: vec![second],
            })
            .unwrap();
        let explicit =
            PlanarLinkageSession::new(explicit_document, SolverConfig::default()).unwrap();

        assert_gauge_split(&automatic, 3, 3, 0);
        assert_gauge_split(&explicit, 3, 3, 0);
        assert_eq!(
            automatic.gauge_report().components[0].world_action,
            PlanarWorldActionCertification::FloatingSe2
        );
        assert_eq!(
            automatic.gauge_report().components[0]
                .numerical_reference
                .unwrap()
                .body,
            first
        );
        assert_eq!(
            explicit.gauge_report().components[0]
                .numerical_reference
                .unwrap()
                .body,
            second
        );
        assert_pose_close(
            automatic
                .document()
                .accepted_state()
                .body(first)
                .unwrap()
                .ambient_pose(),
            [0.0, 0.0, 0.0],
        );
        assert_pose_close(
            explicit
                .document()
                .accepted_state()
                .body(second)
                .unwrap()
                .ambient_pose(),
            [2.0 * scale, 0.35 * scale, 0.65],
        );
        assert_pose_close_with_tolerance(
            relative_pose(&automatic, first, second),
            relative_pose(&explicit, first, second),
            scale,
        );
        assert_eq!(
            automatic.accepted_result().core_report.structural,
            explicit.accepted_result().core_report.structural
        );
        assert_eq!(
            (
                automatic.accepted_result().core_report.rank,
                automatic.accepted_result().core_report.left_nullity,
                automatic.accepted_result().core_report.right_nullity,
                &automatic.accepted_result().core_report.conflicting_sources,
                &automatic.accepted_result().core_report.redundant_sources,
                &automatic
                    .accepted_result()
                    .core_report
                    .sources_containing_redundant_rows,
                automatic.accepted_result().core_report.conflict_diagnostics,
                automatic
                    .accepted_result()
                    .core_report
                    .redundancy_diagnostics,
            ),
            (
                explicit.accepted_result().core_report.rank,
                explicit.accepted_result().core_report.left_nullity,
                explicit.accepted_result().core_report.right_nullity,
                &explicit.accepted_result().core_report.conflicting_sources,
                &explicit.accepted_result().core_report.redundant_sources,
                &explicit
                    .accepted_result()
                    .core_report
                    .sources_containing_redundant_rows,
                explicit.accepted_result().core_report.conflict_diagnostics,
                explicit
                    .accepted_result()
                    .core_report
                    .redundancy_diagnostics,
            )
        );
        assert_eq!(
            automatic
                .accepted_result()
                .source_mappings
                .iter()
                .map(|mapping| mapping.source)
                .collect::<Vec<_>>(),
            explicit
                .accepted_result()
                .source_mappings
                .iter()
                .map(|mapping| mapping.source)
                .collect::<Vec<_>>()
        );
        assert_eq!(automatic.accepted_result().source_mappings.len(), 1);
        assert_eq!(
            automatic.accepted_result().display_audit.sources.len(),
            automatic.accepted_result().source_mappings.len()
        );
        assert!(
            automatic
                .accepted_result()
                .display_audit
                .sources
                .iter()
                .all(|source| !source.source_label.contains("numerical gauge"))
        );
        assert!(
            explicit
                .document()
                .to_json()
                .unwrap()
                .contains("explicit_references")
        );
    }
}

#[test]
fn floating_revolute_driver_velocity_uses_shared_rank_and_selected_gauge() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), false).unwrap();
    let second = linkage
        .add_body(
            "second",
            Pose2::try_new(Vector2::new(1.0, 0.0), 0.4).unwrap(),
            false,
        )
        .unwrap();
    let first_pin = linkage
        .add_point_feature("first pin", first, Point2::new(1.0, 0.0))
        .unwrap();
    let second_pin = linkage
        .add_point_feature("second pin", second, Point2::new(0.0, 0.0))
        .unwrap();
    linkage
        .add_revolute_joint("floating revolute", first_pin, second_pin)
        .unwrap();
    let (undriven_document, _) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x174f_0000), &linkage)
            .unwrap();
    let undriven = PlanarLinkageSession::new(undriven_document, SolverConfig::default()).unwrap();
    assert_gauge_split(&undriven, 4, 3, 1);
    let driver = linkage
        .add_angular_driver("relative angle", first, second, 0.4, 0.1)
        .unwrap();
    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1750_0000), &linkage)
            .unwrap();
    let persistent_first = captured.persistent_body(first).unwrap();
    let persistent_second = captured.persistent_body(second).unwrap();
    let persistent_driver = captured
        .persistent_source(LinkageSource::Driver(driver))
        .unwrap();

    let automatic = PlanarLinkageSession::new(document.clone(), SolverConfig::default()).unwrap();
    assert_gauge_split(&automatic, 3, 3, 0);
    let automatic_velocity = automatic.velocity(persistent_driver, 1.25).unwrap();
    assert_eq!(
        automatic_velocity.rank,
        automatic.core_session().report().rank
    );
    assert_eq!(
        automatic_velocity.local_degrees_of_freedom,
        automatic.core_session().report().right_nullity
    );
    assert_eq!(
        automatic_velocity.rank_threshold.to_bits(),
        automatic.core_session().report().rank_threshold.to_bits()
    );
    let first_velocity = automatic_velocity
        .body(
            automatic
                .runtime_map()
                .runtime_body(persistent_first)
                .unwrap(),
        )
        .unwrap();
    let second_velocity = automatic_velocity
        .body(
            automatic
                .runtime_map()
                .runtime_body(persistent_second)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(first_velocity.linear, Vector2::zeros());
    assert_eq!(first_velocity.angular.to_bits(), 0.0_f64.to_bits());
    assert!((second_velocity.angular - first_velocity.angular - 1.25).abs() <= 1.0e-10);
    assert!(automatic_velocity.differentiated_residual_max <= 1.0e-9);

    let mut explicit_document = document;
    explicit_document
        .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![persistent_second],
        })
        .unwrap();
    let explicit = PlanarLinkageSession::new(explicit_document, SolverConfig::default()).unwrap();
    let explicit_velocity = explicit.velocity(persistent_driver, 1.25).unwrap();
    let reference_velocity = explicit_velocity
        .body(
            explicit
                .runtime_map()
                .runtime_body(persistent_second)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(reference_velocity.linear, Vector2::zeros());
    assert_eq!(reference_velocity.angular.to_bits(), 0.0_f64.to_bits());
    assert!(explicit_velocity.differentiated_residual_max <= 1.0e-9);
}

#[test]
fn domain_component_certification_handles_disconnected_and_branch_only_relationships() {
    let mut disconnected = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = disconnected
        .add_body("ground", Pose2::identity(), true)
        .unwrap();
    let first_a = disconnected
        .add_body("first a", Pose2::identity(), false)
        .unwrap();
    let first_b = disconnected
        .add_body(
            "first b",
            Pose2::try_new(Vector2::new(1.0, 0.0), 0.2).unwrap(),
            false,
        )
        .unwrap();
    let second_a = disconnected
        .add_body(
            "second a",
            Pose2::try_new(Vector2::new(5.0, 0.0), -0.1).unwrap(),
            false,
        )
        .unwrap();
    let second_b = disconnected
        .add_body(
            "second b",
            Pose2::try_new(Vector2::new(6.0, 0.0), 0.3).unwrap(),
            false,
        )
        .unwrap();
    add_origin_weld(&mut disconnected, first_a, first_b, "first weld");
    add_origin_weld(&mut disconnected, second_a, second_b, "second weld");
    let (document, _) = PlanarLinkageDocument::from_linkage(
        PlanarDocumentId::from_u128(0x1760_0000),
        &disconnected,
    )
    .unwrap();
    let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
    assert_gauge_split(&session, 6, 6, 0);
    assert_eq!(session.gauge_report().components.len(), 3);
    assert_eq!(
        session
            .gauge_report()
            .components
            .iter()
            .filter(|component| component.numerical_reference.is_some())
            .count(),
        2
    );
    let persistent_ground = session.runtime_map().persistent_body(ground).unwrap();
    let ground_component = session
        .gauge_report()
        .components
        .iter()
        .find(|component| component.bodies.contains(&persistent_ground))
        .unwrap();
    assert_eq!(ground_component.gauge_dof, 0);
    assert_eq!(ground_component.physical_ground_sources.len(), 1);

    for grounded in [false, true] {
        let mut branch_only = Linkage::new(1.0, xy_plane_frame()).unwrap();
        let line_body = branch_only
            .add_body("line", Pose2::identity(), grounded)
            .unwrap();
        let observed_body = branch_only
            .add_body("observed", Pose2::identity(), false)
            .unwrap();
        let start = branch_only
            .add_point_feature("start", line_body, Point2::new(0.0, 0.0))
            .unwrap();
        let end = branch_only
            .add_point_feature("end", line_body, Point2::new(1.0, 0.0))
            .unwrap();
        let observed = branch_only
            .add_point_feature("observed", observed_body, Point2::new(0.0, 1.0))
            .unwrap();
        branch_only
            .add_orientation_branch_monitor(start, end, observed, BranchSign::Positive)
            .unwrap();
        let (document, _) = PlanarLinkageDocument::from_linkage(
            PlanarDocumentId::from_u128(0x1770_0000 + u128::from(grounded)),
            &branch_only,
        )
        .unwrap();
        let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
        assert_eq!(session.gauge_report().components.len(), 1);
        if grounded {
            assert_gauge_split(&session, 3, 0, 3);
        } else {
            assert_gauge_split(&session, 6, 3, 3);
        }
    }
}

#[test]
fn gauge_policy_validation_persistence_and_revision_change_are_transactional() {
    let (linkage, first_runtime, second_runtime) = floating_weld(1.0);
    let (mut document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1780_0000), &linkage)
            .unwrap();
    let first = captured.persistent_body(first_runtime).unwrap();
    let second = captured.persistent_body(second_runtime).unwrap();
    assert!(matches!(
        document.set_gauge_policy(PlanarGaugePolicy::ExplicitReferences { bodies: vec![] }),
        Err(PlanarLinkageError::InvalidGaugePolicy(_))
    ));
    assert!(matches!(
        document.set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![first, first],
        }),
        Err(PlanarLinkageError::InvalidGaugePolicy(_))
    ));
    document
        .set_gauge_policy(PlanarGaugePolicy::ExplicitReferences {
            bodies: vec![second],
        })
        .unwrap();
    let canonical = document.to_json().unwrap();
    assert_eq!(
        PlanarLinkageDocument::from_json(&canonical)
            .unwrap()
            .to_json()
            .unwrap(),
        canonical
    );

    let mut session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
    let before = session.document().to_json().unwrap();
    assert!(matches!(
        session.set_gauge_policy(1, PlanarGaugePolicy::LowestPersistentBody),
        Err(PlanarLinkageError::StaleRevision {
            expected: 1,
            actual: 0
        })
    ));
    assert_eq!(session.document().to_json().unwrap(), before);
    session
        .set_gauge_policy(0, PlanarGaugePolicy::LowestPersistentBody)
        .unwrap();
    assert_eq!(session.document().accepted_state().revision(), 1);
    assert_eq!(
        session.gauge_report().components[0]
            .numerical_reference
            .unwrap()
            .body,
        first
    );
}

#[test]
fn l3_persistent_velocity_matches_compatibility_facade_and_core_rank() {
    let (linkage, ids) = slider_crank().unwrap();
    let (document, captured) =
        PlanarLinkageDocument::from_linkage(PlanarDocumentId::from_u128(0x1790_0000), &linkage)
            .unwrap();
    let driver = captured
        .persistent_source(LinkageSource::Driver(ids.driver))
        .unwrap();
    let session = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
    let persistent = session.velocity(driver, 1.0).unwrap();
    let runtime_driver = match session.runtime_map().runtime_source(driver).unwrap() {
        PlanarRuntimeSource::Driver(driver) => driver,
        source => panic!("driver expected, got {source:?}"),
    };
    let compatibility = session.runtime().velocity(runtime_driver, 1.0).unwrap();
    assert_eq!(persistent.rank, session.core_session().report().rank);
    assert_eq!(
        persistent.local_degrees_of_freedom,
        session.core_session().report().right_nullity
    );
    assert_eq!(persistent.rank, compatibility.rank);
    assert_eq!(
        persistent.local_degrees_of_freedom,
        compatibility.local_degrees_of_freedom
    );
    for velocity in &persistent.body_velocities {
        let other = compatibility.body(velocity.body_id).unwrap();
        assert!((velocity.linear - other.linear).norm() <= TOLERANCE);
        assert!((velocity.angular - other.angular).abs() <= TOLERANCE);
    }
    assert!(persistent.differentiated_residual_max <= 1.0e-9);
}

fn floating_weld(scale: f64) -> (Linkage, geosolve_linkage::BodyId, geosolve_linkage::BodyId) {
    let mut linkage = Linkage::new(scale, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), false).unwrap();
    let second = linkage
        .add_body(
            "second",
            Pose2::try_new(Vector2::new(2.0 * scale, 0.0), 0.55).unwrap(),
            false,
        )
        .unwrap();
    let first_anchor = linkage
        .add_point_feature("first anchor", first, Point2::new(2.0 * scale, 0.0))
        .unwrap();
    let second_anchor = linkage
        .add_point_feature("second anchor", second, Point2::new(0.0, 0.0))
        .unwrap();
    linkage
        .add_weld_joint_with_angle("floating weld", first_anchor, second_anchor, 0.55)
        .unwrap();
    (linkage, first, second)
}

fn add_origin_weld(
    linkage: &mut Linkage,
    first: geosolve_linkage::BodyId,
    second: geosolve_linkage::BodyId,
    label: &str,
) {
    let first_pose = linkage.body(first).unwrap().pose();
    let second_pose = linkage.body(second).unwrap().pose();
    let first_anchor = linkage
        .add_point_feature(
            format!("{label} first"),
            first,
            first_pose.inverse_transform_point(Point2::from(second_pose.translation)),
        )
        .unwrap();
    let second_anchor = linkage
        .add_point_feature(format!("{label} second"), second, Point2::new(0.0, 0.0))
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

fn perturb_body_state(
    document: &PlanarLinkageDocument,
    body: PlanarBodyId,
    pose: [f64; 3],
) -> PlanarLinkageDocument {
    let mut value: serde_json::Value = serde_json::from_str(&document.to_json().unwrap()).unwrap();
    let body = body.to_string();
    let state = value["accepted_state"]["bodies"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|state| state["body"].as_str() == Some(body.as_str()))
        .unwrap();
    state["pose"] = serde_json::json!(pose);
    PlanarLinkageDocument::from_json(&serde_json::to_string(&value).unwrap()).unwrap()
}

fn relative_pose(
    session: &PlanarLinkageSession,
    first: PlanarBodyId,
    second: PlanarBodyId,
) -> [f64; 3] {
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
    first.inverse().unwrap().compose(&second).unwrap().ambient()
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
    assert_eq!(
        report.numerical_equality_right_nullity,
        report.gauge_dof + report.internal_mobility
    );
    assert_eq!(
        report.numerical_equality_right_nullity,
        session.core_session().report().right_nullity
    );
}

fn assert_pose_close_with_tolerance(first: [f64; 3], second: [f64; 3], model_scale: f64) {
    for (index, (first, second)) in first.into_iter().zip(second).enumerate() {
        let scale = if index < 2 { model_scale } else { 1.0 };
        assert!(
            (first - second).abs() <= 1.0e-9 * scale,
            "{first:?} != {second:?}"
        );
    }
}

fn document_body_ids(
    document: &PlanarLinkageDocument,
) -> impl Iterator<Item = geosolve_linkage::PlanarBodyId> + '_ {
    document
        .topology()
        .bodies()
        .iter()
        .map(geosolve_linkage::PlanarBody::id)
}
