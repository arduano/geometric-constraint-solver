use geosolve_core::SolverConfig;
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssembly, SpatialAssemblyDocument, SpatialAssemblyDocumentSession,
    SpatialAssemblySession, SpatialAxisParity, SpatialBoundaryHysteresisState, SpatialDocumentId,
    SpatialExampleIds, SpatialExampleKind, SpatialGaugePolicy, SpatialModeSign,
    SpatialRuntimeFeature, spatial_example,
};
use serde_json::{Value, json};

const DOCUMENT_ID: SpatialDocumentId =
    SpatialDocumentId::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);

#[test]
#[allow(clippy::too_many_lines)]
fn spatial_document_round_trip_is_canonical_and_remaps_fresh_runtime_ids() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, scale).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!()
        };
        let accepted =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let persistent =
            SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
        let json = persistent.to_json().unwrap();
        let parsed = SpatialAssemblyDocument::from_json(&json).unwrap();
        assert_eq!(parsed.to_json().unwrap(), json);

        let restored =
            SpatialAssemblyDocumentSession::new(parsed, SolverConfig::default()).unwrap();
        assert_eq!(restored.document().id(), DOCUMENT_ID);
        assert_eq!(restored.document().revision(), accepted.revision());
        assert_eq!(
            restored.accepted_result().core_report.rank,
            accepted.accepted_result().core_report.rank,
        );
        assert_eq!(restored.runtime_map().document_id(), DOCUMENT_ID);

        for body in ids.bodies {
            let persistent_id = persistent.runtime_map().persistent_body(body).unwrap();
            let remapped = restored.runtime_map().runtime_body(persistent_id).unwrap();
            assert_ne!(
                remapped, body,
                "fresh lowering must use a fresh runtime namespace"
            );
            let before = accepted
                .accepted_result()
                .geometry
                .body_pose(body)
                .unwrap()
                .ambient();
            let after = restored
                .accepted_result()
                .geometry
                .body_pose(remapped)
                .unwrap()
                .ambient();
            assert!(
                before
                    .into_iter()
                    .zip(after)
                    .all(|(a, b)| (a - b).abs() <= 2.0e-10)
            );
        }
        for frame in ids.frames {
            let persistent_id = persistent
                .runtime_map()
                .persistent_feature(SpatialRuntimeFeature::Frame(frame))
                .unwrap();
            assert!(matches!(
                restored.runtime_map().runtime_feature(persistent_id),
                Some(SpatialRuntimeFeature::Frame(_))
            ));
        }
        for axis in ids.axes {
            let persistent_id = persistent
                .runtime_map()
                .persistent_feature(SpatialRuntimeFeature::Axis(axis))
                .unwrap();
            assert!(matches!(
                restored.runtime_map().runtime_feature(persistent_id),
                Some(SpatialRuntimeFeature::Axis(_))
            ));
        }
        for coordinate in ids.coordinates {
            let persistent_id = persistent
                .runtime_map()
                .persistent_coordinate(coordinate)
                .unwrap();
            assert!(
                restored
                    .runtime_map()
                    .runtime_coordinate(persistent_id)
                    .is_some()
            );
        }
        for driver in ids.drivers {
            let persistent_id = persistent.runtime_map().persistent_source(driver).unwrap();
            assert!(
                restored
                    .runtime_map()
                    .runtime_source(persistent_id)
                    .is_some()
            );
        }
        for monitor in ids.monitors {
            let persistent_id = persistent
                .runtime_map()
                .persistent_monitor(monitor)
                .unwrap();
            assert!(
                restored
                    .runtime_map()
                    .runtime_monitor(persistent_id)
                    .is_some()
            );
        }
    }
}

#[test]
fn spatial_document_restores_serialized_boundary_hysteresis() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let accepted = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let persistent =
        SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
    let mut value: Value = serde_json::from_str(&persistent.to_json().unwrap()).unwrap();
    let boundaries = value["accepted_state"]["boundaries"]
        .as_array_mut()
        .unwrap();
    assert!(!boundaries.is_empty());
    boundaries[0]["state"] = json!("near");

    let restored = SpatialAssemblyDocumentSession::from_json(
        &serde_json::to_string(&value).unwrap(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(
        restored.accepted_result().branch_boundary_evaluations[0].hysteresis_state,
        SpatialBoundaryHysteresisState::Near,
    );
}

#[test]
fn malformed_spatial_documents_are_rejected() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let accepted = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let persistent =
        SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
    let original: Value = serde_json::from_str(&persistent.to_json().unwrap()).unwrap();

    let mut unsupported = original.clone();
    unsupported["version"] = json!(99);
    assert!(SpatialAssemblyDocument::from_json(&unsupported.to_string()).is_err());

    let mut unknown_field = original.clone();
    unknown_field["unexpected"] = json!(true);
    assert!(SpatialAssemblyDocument::from_json(&unknown_field.to_string()).is_err());

    let mut unknown_reference = original.clone();
    unknown_reference["topology"]["point_features"][0]["body"] =
        json!("ffffffffffffffffffffffffffffffff");
    assert!(SpatialAssemblyDocument::from_json(&unknown_reference.to_string()).is_err());

    let mut duplicate = original.clone();
    duplicate["topology"]["bodies"][1]["id"] = duplicate["topology"]["bodies"][0]["id"].clone();
    assert!(SpatialAssemblyDocument::from_json(&duplicate.to_string()).is_err());

    let mut wrong_driver_kind = original.clone();
    let drivers = wrong_driver_kind["accepted_state"]["drivers"]
        .as_array_mut()
        .unwrap();
    let hinge = drivers
        .iter_mut()
        .find(|driver| driver["target"]["kind"] == "hinge")
        .unwrap();
    hinge["target"] = json!({ "kind": "translation", "target": 0.48 });
    assert!(SpatialAssemblyDocument::from_json(&wrong_driver_kind.to_string()).is_err());

    let mut missing_boundary = original;
    missing_boundary["accepted_state"]["boundaries"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        SpatialAssemblyDocumentSession::from_json(
            &missing_boundary.to_string(),
            SolverConfig::default(),
        )
        .is_err()
    );
}

#[test]
fn failed_spatial_json_replacement_retains_every_accepted_view() {
    let fixture = spatial_example(SpatialExampleKind::BlockBase, 1.0).unwrap();
    let accepted = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let mut persistent =
        SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
    let before_json = persistent.to_json().unwrap();
    let before_revision = persistent.session().revision();
    let before_geometry = persistent.accepted_result().geometry.clone();

    let mut malformed: Value = serde_json::from_str(&before_json).unwrap();
    malformed["topology"]["model_scale"] = json!(0.0);
    assert!(persistent.replace_json(&malformed.to_string()).is_err());

    assert_eq!(persistent.to_json().unwrap(), before_json);
    assert_eq!(persistent.session().revision(), before_revision);
    assert_eq!(persistent.accepted_result().geometry, before_geometry);
}

#[test]
fn complete_spatial_source_and_monitor_catalog_round_trips() {
    let assembly = complete_catalog_assembly();
    let accepted = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let persistent =
        SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
    let json = persistent.to_json().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let kinds = value["topology"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| source["definition"]["kind"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from([
            "physical_ground",
            "ball_joint",
            "fixed_frame",
            "revolute_joint",
            "prismatic_joint",
            "cylindrical_joint",
            "planar_joint",
            "universal_joint",
            "point_distance_mate",
            "axis_angle_mate",
            "axis_alignment_mate",
            "frame_offset_mate",
        ])
    );
    assert!(
        value["topology"]["monitors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|monitor| monitor["definition"]["kind"] == "signed_volume")
    );

    let restored =
        SpatialAssemblyDocumentSession::from_json(&json, SolverConfig::default()).unwrap();
    assert_eq!(
        restored.session().assembly().sources().len(),
        accepted.assembly().sources().len()
    );
    assert_eq!(restored.session().assembly().mode_monitors().len(), 1);
    assert_eq!(
        restored.accepted_result().core_report.rank,
        accepted.accepted_result().core_report.rank
    );
}

#[test]
fn explicit_spatial_gauge_reference_survives_persistent_remapping() {
    let identity =
        Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap();
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first = assembly.add_body("first", Pose3::identity()).unwrap();
    let second = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_frame = assembly
        .add_frame_feature("first frame", first, identity)
        .unwrap();
    let second_frame = assembly
        .add_frame_feature("second frame", second, identity)
        .unwrap();
    assembly
        .add_fixed_frame("floating fixed pair", first_frame, second_frame)
        .unwrap();
    let mut accepted = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    accepted
        .set_gauge_policy(
            0,
            SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![second],
            },
        )
        .unwrap();

    let persistent =
        SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
    let second_persistent = persistent.runtime_map().persistent_body(second).unwrap();
    let restored = SpatialAssemblyDocumentSession::from_json(
        &persistent.to_json().unwrap(),
        SolverConfig::default(),
    )
    .unwrap();
    let remapped_second = restored
        .runtime_map()
        .runtime_body(second_persistent)
        .unwrap();
    assert_eq!(restored.document().revision(), 1);
    assert_eq!(
        restored.session().gauge_report().components[0]
            .numerical_reference
            .unwrap()
            .body,
        remapped_second,
    );
}

#[allow(clippy::too_many_lines)]
fn complete_catalog_assembly() -> SpatialAssembly {
    let identity =
        Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap();
    let x_axis =
        Frame3::try_new(Point3::origin(), Vector3::y(), Vector3::z(), Vector3::x()).unwrap();
    let angle = 0.7_f64;
    let angle_axis = Frame3::try_new(
        Point3::origin(),
        Vector3::x(),
        Vector3::new(0.0, angle.cos(), angle.sin()),
        Vector3::new(0.0, -angle.sin(), angle.cos()),
    )
    .unwrap();
    let offset = Frame3::try_new(
        Point3::new(0.2, -0.1, 0.3),
        Vector3::x(),
        Vector3::y(),
        Vector3::z(),
    )
    .unwrap();
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let anchor = assembly.add_body("anchor", Pose3::identity()).unwrap();
    assembly
        .add_physical_ground("anchor ground", anchor)
        .unwrap();

    let mut moving = || assembly.add_body("moving body", Pose3::identity()).unwrap();
    let ball_body = moving();
    let fixed_body = moving();
    let revolute_body = moving();
    let prismatic_body = moving();
    let cylindrical_body = moving();
    let planar_body = moving();
    let universal_body = moving();
    let distance_body = moving();
    let angle_body = moving();
    let alignment_body = moving();
    let offset_body = moving();

    let anchor_ball = assembly
        .add_point_feature("anchor ball", anchor, Point3::origin())
        .unwrap();
    let moving_ball = assembly
        .add_point_feature("moving ball", ball_body, Point3::origin())
        .unwrap();
    assembly
        .add_ball_joint("ball", anchor_ball, moving_ball)
        .unwrap();

    let anchor_fixed = assembly
        .add_frame_feature("anchor fixed", anchor, identity)
        .unwrap();
    let moving_fixed = assembly
        .add_frame_feature("moving fixed", fixed_body, identity)
        .unwrap();
    assembly
        .add_fixed_frame("fixed", anchor_fixed, moving_fixed)
        .unwrap();

    let anchor_revolute = assembly
        .add_frame_feature("anchor revolute", anchor, identity)
        .unwrap();
    let moving_revolute = assembly
        .add_frame_feature("moving revolute", revolute_body, identity)
        .unwrap();
    assembly
        .add_revolute_joint(
            "revolute",
            anchor_revolute,
            moving_revolute,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let anchor_prismatic = assembly
        .add_axis_feature("anchor prismatic", anchor, identity)
        .unwrap();
    let moving_prismatic = assembly
        .add_axis_feature("moving prismatic", prismatic_body, identity)
        .unwrap();
    assembly
        .add_prismatic_joint(
            "prismatic",
            anchor_prismatic,
            moving_prismatic,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let anchor_cylindrical = assembly
        .add_axis_feature("anchor cylindrical", anchor, identity)
        .unwrap();
    let moving_cylindrical = assembly
        .add_axis_feature("moving cylindrical", cylindrical_body, identity)
        .unwrap();
    assembly
        .add_cylindrical_joint(
            "cylindrical",
            anchor_cylindrical,
            moving_cylindrical,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let anchor_planar = assembly
        .add_plane_feature("anchor planar", anchor, identity)
        .unwrap();
    let moving_planar = assembly
        .add_plane_feature("moving planar", planar_body, identity)
        .unwrap();
    assembly
        .add_planar_joint(
            "planar",
            anchor_planar,
            moving_planar,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let anchor_universal = assembly
        .add_axis_feature("anchor universal", anchor, identity)
        .unwrap();
    let moving_universal = assembly
        .add_axis_feature("moving universal", universal_body, x_axis)
        .unwrap();
    assembly
        .add_universal_joint("universal", anchor_universal, moving_universal)
        .unwrap();

    let anchor_distance = assembly
        .add_point_feature("anchor distance", anchor, Point3::origin())
        .unwrap();
    let moving_distance = assembly
        .add_point_feature("moving distance", distance_body, Point3::new(1.0, 0.0, 0.0))
        .unwrap();
    assembly
        .add_point_distance_mate("distance", anchor_distance, moving_distance, 1.0)
        .unwrap();

    let anchor_angle = assembly
        .add_axis_feature("anchor angle", anchor, identity)
        .unwrap();
    let moving_angle = assembly
        .add_axis_feature("moving angle", angle_body, angle_axis)
        .unwrap();
    assembly
        .add_axis_angle_mate("angle", anchor_angle, moving_angle, angle)
        .unwrap();

    let anchor_alignment = assembly
        .add_axis_feature("anchor alignment", anchor, identity)
        .unwrap();
    let moving_alignment = assembly
        .add_axis_feature("moving alignment", alignment_body, identity)
        .unwrap();
    assembly
        .add_axis_alignment_mate(
            "alignment",
            anchor_alignment,
            moving_alignment,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let anchor_offset = assembly
        .add_frame_feature("anchor offset", anchor, identity)
        .unwrap();
    let moving_offset = assembly
        .add_frame_feature("moving offset", offset_body, offset)
        .unwrap();
    assembly
        .add_frame_offset_mate("offset", anchor_offset, moving_offset, offset)
        .unwrap();

    let volume_points = [
        assembly
            .add_point_feature("volume a", anchor, Point3::origin())
            .unwrap(),
        assembly
            .add_point_feature("volume b", anchor, Point3::new(1.0, 0.0, 0.0))
            .unwrap(),
        assembly
            .add_point_feature("volume c", anchor, Point3::new(0.0, 1.0, 0.0))
            .unwrap(),
        assembly
            .add_point_feature("volume d", anchor, Point3::new(0.0, 0.0, 1.0))
            .unwrap(),
    ];
    assembly
        .add_signed_volume_monitor("positive volume", volume_points, SpatialModeSign::Positive)
        .unwrap();
    assembly
}
