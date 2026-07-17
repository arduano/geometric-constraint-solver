use std::f64::consts::PI;

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssembly, SpatialAssemblyError, SpatialAssemblySession, SpatialAxisParity,
    SpatialBodyId, SpatialGaugePolicy, SpatialPatch, SpatialSourceKind,
    SpatialWorldActionCertification,
};

const RESIDUAL_TOLERANCE: f64 = 1.0e-9;
const JACOBIAN_TOLERANCE: f64 = 2.0e-6;

#[derive(Clone, Copy, Debug)]
enum Primitive {
    Ball,
    Fixed,
    Revolute(SpatialAxisParity),
}

impl Primitive {
    const fn expected_rank(self) -> usize {
        match self {
            Self::Ball => 3,
            Self::Fixed => 6,
            Self::Revolute(_) => 5,
        }
    }

    const fn expected_internal_mobility(self) -> usize {
        match self {
            Self::Ball => 3,
            Self::Fixed => 0,
            Self::Revolute(_) => 1,
        }
    }

    const fn expected_rows(self) -> usize {
        self.expected_rank()
    }
}

struct Fixture {
    assembly: SpatialAssembly,
    first_body: SpatialBodyId,
    second_body: SpatialBodyId,
}

#[test]
fn spatial_primitives_report_expected_floating_and_grounded_mobility() {
    for primitive in [
        Primitive::Ball,
        Primitive::Fixed,
        Primitive::Revolute(SpatialAxisParity::Aligned),
        Primitive::Revolute(SpatialAxisParity::Opposed),
    ] {
        let floating = fixture(primitive, 1.0, false, false, None);
        let floating = SpatialAssemblySession::new(floating.assembly, SolverConfig::default())
            .unwrap_or_else(|error| panic!("{primitive:?} floating: {error:#?}"));
        assert_accepted(&floating);
        assert_eq!(
            floating.accepted_result().core_report.rank,
            primitive.expected_rank()
        );
        assert_eq!(
            floating.accepted_result().core_report.right_nullity,
            6 + primitive.expected_internal_mobility()
        );
        assert_eq!(floating.gauge_report().gauge_dof, 6);
        assert_eq!(
            floating.gauge_report().internal_mobility,
            primitive.expected_internal_mobility()
        );
        assert_eq!(
            floating.gauge_report().components[0].world_action,
            SpatialWorldActionCertification::FloatingSe3
        );

        let grounded = fixture(primitive, 1.0, true, false, None);
        let grounded = SpatialAssemblySession::new(grounded.assembly, SolverConfig::default())
            .unwrap_or_else(|error| panic!("{primitive:?} grounded: {error:#?}"));
        assert_accepted(&grounded);
        assert_eq!(
            grounded.accepted_result().core_report.rank,
            primitive.expected_rank()
        );
        assert_eq!(
            grounded.accepted_result().core_report.right_nullity,
            primitive.expected_internal_mobility()
        );
        assert_eq!(grounded.gauge_report().gauge_dof, 0);
        assert_eq!(
            grounded.gauge_report().internal_mobility,
            primitive.expected_internal_mobility()
        );
        assert_eq!(
            grounded.gauge_report().components[0].world_action,
            SpatialWorldActionCertification::PhysicallyGrounded
        );
    }
}

#[test]
fn spatial_joint_jacobians_match_right_tangent_central_differences() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for primitive in [
            Primitive::Ball,
            Primitive::Fixed,
            Primitive::Revolute(SpatialAxisParity::Aligned),
            Primitive::Revolute(SpatialAxisParity::Opposed),
        ] {
            let fixture = fixture(primitive, scale, false, true, None);
            let compiled = fixture.assembly.compile().unwrap();
            let oracle = compiled.check_jacobians(2.0e-6).unwrap();
            assert_eq!(oracle.blocks.len(), 2, "{primitive:?}, scale={scale:e}");
            assert!(
                oracle.all_within(JACOBIAN_TOLERANCE),
                "{primitive:?}, scale={scale:e}: {oracle:#?}"
            );
            assert!(oracle.blocks.iter().all(|block| block.columns == 6));
        }
    }
}

#[test]
fn transformed_scaled_and_perturbed_spatial_fixtures_recover() {
    let transform = Pose3::exp([2.3, -1.7, 0.9, 0.4, -0.3, 0.2]).unwrap();
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let scaled_transform =
            Pose3::exp([2.3 * scale, -1.7 * scale, 0.9 * scale, 0.4, -0.3, 0.2]).unwrap();
        for primitive in [
            Primitive::Ball,
            Primitive::Fixed,
            Primitive::Revolute(SpatialAxisParity::Aligned),
            Primitive::Revolute(SpatialAxisParity::Opposed),
        ] {
            for common_left in [None, Some(scaled_transform), Some(transform)] {
                let fixture = fixture(primitive, scale, true, true, common_left);
                let session =
                    SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                        .unwrap_or_else(|error| {
                            panic!(
                                "{primitive:?}, scale={scale:e}, transform={}: {error:#?}",
                                common_left.is_some()
                            )
                        });
                assert_accepted(&session);
                assert_eq!(
                    session.accepted_result().core_report.rank,
                    primitive.expected_rank()
                );
                assert_eq!(
                    session.gauge_report().internal_mobility,
                    primitive.expected_internal_mobility()
                );
                assert!(
                    session
                        .accepted_result()
                        .geometry
                        .bodies
                        .iter()
                        .all(|body| body.pose.ambient().iter().all(|value| value.is_finite()))
                );
            }
        }
    }
}

#[test]
fn floating_gauges_are_six_dof_private_and_reference_independent() {
    let fixture = fixture(Primitive::Fixed, 1.0, false, true, None);
    let second_body = fixture.second_body;
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let automatic = session.accepted_result().clone();
    assert_eq!(session.gauge_report().gauge_dof, 6);
    assert_eq!(
        session.gauge_report().components[0]
            .numerical_reference
            .unwrap()
            .body,
        fixture.first_body
    );
    assert_private_gauges_absent(&session);

    session
        .set_gauge_policy(
            session.revision(),
            SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![second_body],
            },
        )
        .unwrap();
    assert_eq!(session.gauge_report().gauge_dof, 6);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_eq!(
        session.gauge_report().components[0]
            .numerical_reference
            .unwrap()
            .body,
        second_body
    );
    assert_geometry_close(
        &session.accepted_result().geometry,
        &automatic.geometry,
        1.0e-9,
    );
    assert_eq!(
        session.accepted_result().source_mappings,
        automatic.source_mappings
    );
    assert_private_gauges_absent(&session);

    let accepted = session.accepted_result().clone();
    let revision = session.revision();
    assert!(matches!(
        session.set_gauge_policy(
            revision,
            SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![fixture.first_body, second_body],
            },
        ),
        Err(SpatialAssemblyError::InvalidGaugePolicy(_))
    ));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &accepted);
}

#[test]
fn disconnected_spatial_components_receive_only_required_private_gauges() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let frame = identity_frame(Point3::origin());
    let mut floating_bodies = Vec::new();
    for pair in 0..2 {
        let first = assembly
            .add_body(format!("floating {pair} first"), Pose3::identity())
            .unwrap();
        let second = assembly
            .add_body(format!("floating {pair} second"), Pose3::identity())
            .unwrap();
        floating_bodies.extend([first, second]);
        let first_frame = assembly
            .add_frame_feature(format!("floating {pair} first frame"), first, frame)
            .unwrap();
        let second_frame = assembly
            .add_frame_feature(format!("floating {pair} second frame"), second, frame)
            .unwrap();
        assembly
            .add_fixed_frame(format!("floating {pair} fixed"), first_frame, second_frame)
            .unwrap();
    }
    let grounded = assembly
        .add_body("grounded isolated", Pose3::identity())
        .unwrap();
    assembly
        .add_physical_ground("isolated physical ground", grounded)
        .unwrap();

    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.gauge_report().components.len(), 3);
    assert_eq!(session.gauge_report().gauge_dof, 12);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_eq!(session.accepted_result().core_report.right_nullity, 12);
    assert_eq!(
        session
            .gauge_report()
            .components
            .iter()
            .filter(|component| component.numerical_reference.is_some())
            .count(),
        2
    );
    assert!(
        session
            .gauge_report()
            .components
            .iter()
            .find(|component| component.bodies == vec![grounded])
            .unwrap()
            .numerical_reference
            .is_none()
    );
    assert_private_gauges_absent(&session);
    assert_eq!(floating_bodies.len(), 4);
}

#[test]
fn spatial_audit_rows_are_complete_deterministic_and_physical() {
    for primitive in [
        Primitive::Ball,
        Primitive::Fixed,
        Primitive::Revolute(SpatialAxisParity::Aligned),
    ] {
        let fixture = fixture(primitive, 2.5, true, false, None);
        let session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let result = session.accepted_result();
        assert_eq!(result.source_mappings.len(), 2);
        assert_eq!(result.display_audit.sources.len(), 2);
        for mapping in &result.source_mappings {
            let source = result
                .display_audit
                .sources
                .iter()
                .find(|source| source.source_id == mapping.core_source_id)
                .unwrap();
            let expected_rows = if mapping.source_label.contains("ground") {
                6
            } else {
                primitive.expected_rows()
            };
            assert_eq!(source.rows.len(), expected_rows);
            assert!(source.rows.iter().all(|row| {
                row.evaluation_status == AuditEvaluationStatus::Evaluated
                    && row.raw_residual.is_finite()
                    && row.normalized_residual.is_finite()
                    && row.scale.is_finite()
                    && !row.template.is_empty()
                    && !row.bindings.is_empty()
            }));
            if mapping.source_label.contains("ground") {
                assert!(source.rows.iter().all(|row| row.annotations.eliminated));
                assert!(source.rows[..3].iter().all(|row| row.unit == "model-unit"));
                assert!(source.rows[3..].iter().all(|row| row.unit == "rad"));
            }
        }
        assert_private_gauges_absent(&session);
        let linearization = session
            .core_session()
            .accepted_hard_linearization()
            .unwrap();
        let active_rows = linearization
            .components()
            .iter()
            .flat_map(geosolve_core::AcceptedHardComponentLinearization::hard_rows)
            .collect::<Vec<_>>();
        assert!(active_rows.iter().all(|row| {
            result.source_mappings.iter().any(|mapping| {
                mapping.core_source_id == row.row.source_id
                    && mapping.residual_ids.contains(&row.row.residual_id)
            })
        }));
    }
}

#[test]
fn fixed_half_turn_and_wrong_revolute_parity_never_accept() {
    let mut fixed = SpatialAssembly::new(1.0).unwrap();
    let first = fixed.add_body("fixed first", Pose3::identity()).unwrap();
    let half_turn = Pose3::exp([0.0, 0.0, 0.0, PI, 0.0, 0.0]).unwrap();
    let second = fixed.add_body("fixed second", half_turn).unwrap();
    let frame = identity_frame(Point3::origin());
    let first_frame = fixed
        .add_frame_feature("first frame", first, frame)
        .unwrap();
    let second_frame = fixed
        .add_frame_feature("second frame", second, frame)
        .unwrap();
    fixed
        .add_fixed_frame("fixed false root", first_frame, second_frame)
        .unwrap();
    assert!(fixed.compile().unwrap().check_jacobians(2.0e-6).is_ok());
    assert!(matches!(
        SpatialAssemblySession::new(fixed, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("false half-turn")
    ));

    let opposed = fixture(
        Primitive::Revolute(SpatialAxisParity::Opposed),
        1.0,
        false,
        false,
        None,
    );
    let wrong = opposed.assembly;
    let source = wrong
        .sources()
        .iter()
        .find(|source| matches!(source.kind(), SpatialSourceKind::RevoluteJoint { .. }))
        .unwrap();
    let SpatialSourceKind::RevoluteJoint { first, second, .. } = source.kind() else {
        unreachable!()
    };
    let mut rebuilt = SpatialAssembly::new(1.0).unwrap();
    let first_body_pose = wrong.body(opposed.first_body).unwrap().pose_guess();
    let second_body_pose = wrong.body(opposed.second_body).unwrap().pose_guess();
    let first_body = rebuilt.add_body("first", first_body_pose).unwrap();
    let second_body = rebuilt.add_body("second", second_body_pose).unwrap();
    let first_frame = rebuilt
        .add_frame_feature(
            "first frame",
            first_body,
            wrong.frame_feature(first).unwrap().local_frame(),
        )
        .unwrap();
    let second_frame = rebuilt
        .add_frame_feature(
            "second frame",
            second_body,
            wrong.frame_feature(second).unwrap().local_frame(),
        )
        .unwrap();
    rebuilt
        .add_revolute_joint(
            "wrong aligned parity",
            first_frame,
            second_frame,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    assert!(matches!(
        SpatialAssemblySession::new(rebuilt, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("axis parity")
    ));
}

#[test]
fn invalid_spatial_features_and_overflowed_world_geometry_never_accept() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let body = assembly.add_body("body", Pose3::identity()).unwrap();
    assert!(matches!(
        assembly.add_point_feature("nan", body, Point3::new(f64::NAN, 0.0, 0.0)),
        Err(SpatialAssemblyError::InvalidField { .. })
    ));
    assert!(Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::x(), Vector3::z()).is_err());
    assert!(Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), -Vector3::z()).is_err());

    let mut overflow = SpatialAssembly::new(1.0).unwrap();
    let huge_pose = Pose3::try_new(Vector3::new(f64::MAX, 0.0, 0.0), [1.0, 0.0, 0.0, 0.0]).unwrap();
    let huge = overflow.add_body("huge", huge_pose).unwrap();
    overflow
        .add_point_feature("overflow point", huge, Point3::new(f64::MAX, 0.0, 0.0))
        .unwrap();
    assert!(SpatialAssemblySession::new(overflow, SolverConfig::default()).is_err());
}

#[test]
fn loose_solver_tolerance_cannot_weaken_spatial_acceptance() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first = assembly.add_body("first", Pose3::identity()).unwrap();
    let second = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_point = assembly
        .add_point_feature("first point", first, Point3::origin())
        .unwrap();
    let second_point = assembly
        .add_point_feature("second point", second, Point3::new(0.25, 0.0, 0.0))
        .unwrap();
    assembly.add_physical_ground("first ground", first).unwrap();
    assembly
        .add_physical_ground("second ground", second)
        .unwrap();
    assembly
        .add_ball_joint("impossible ball", first_point, second_point)
        .unwrap();
    let loose = SolverConfig {
        normalized_residual_tolerance: 1.0,
        ..SolverConfig::default()
    };
    assert!(matches!(
        SpatialAssemblySession::new(assembly, loose),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("exceeds")
    ));
}

#[test]
fn rejected_spatial_patch_retains_every_accepted_view_and_ground_target() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first = assembly.add_body("first", Pose3::identity()).unwrap();
    let second = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_point = assembly
        .add_point_feature("first point", first, Point3::origin())
        .unwrap();
    let second_point = assembly
        .add_point_feature("second point", second, Point3::origin())
        .unwrap();
    assembly.add_physical_ground("first ground", first).unwrap();
    assembly
        .add_physical_ground("second ground", second)
        .unwrap();
    assembly
        .add_ball_joint("locked ball", first_point, second_point)
        .unwrap();
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.gauge_report().components.len(), 1);
    assert_eq!(session.gauge_report().gauge_dof, 0);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_eq!(
        session.gauge_report().components[0]
            .core_component_indices
            .len(),
        3
    );
    let revision = session.revision();
    let accepted = session.accepted_result().clone();
    let gauge = session.gauge_report().clone();
    let report = session.core_session().report().clone();
    let linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();

    assert!(
        session
            .apply_patch(
                revision,
                SpatialPatch::PointLocal {
                    feature: second_point,
                    local_point: Point3::new(1.0, 0.0, 0.0),
                },
            )
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &accepted);
    assert_eq!(session.gauge_report(), &gauge);
    assert_eq!(session.core_session().report(), &report);
    assert_eq!(
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        linearization
    );

    assert!(matches!(
        session.apply_patch(
            revision + 1,
            SpatialPatch::BodyPoseGuess {
                body: second,
                pose: Pose3::identity(),
            },
        ),
        Err(SpatialAssemblyError::StaleRevision { .. })
    ));
    session
        .apply_patch(
            revision,
            SpatialPatch::BodyPoseGuess {
                body: second,
                pose: Pose3::exp([2.0, -1.0, 0.5, 0.3, -0.2, 0.1]).unwrap(),
            },
        )
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert_pose_close(
        session
            .accepted_result()
            .geometry
            .body_pose(second)
            .unwrap(),
        Pose3::identity(),
        1.0e-12,
    );
}

fn fixture(
    primitive: Primitive,
    scale: f64,
    grounded: bool,
    perturb_second: bool,
    common_left: Option<Pose3>,
) -> Fixture {
    let first_exact =
        Pose3::exp([0.7 * scale, -0.2 * scale, 0.4 * scale, 0.2, -0.15, 0.1]).unwrap();
    let second_exact = Pose3::exp([
        -0.35 * scale,
        0.55 * scale,
        -0.25 * scale,
        -0.18,
        0.22,
        -0.12,
    ])
    .unwrap();
    let desired =
        Pose3::exp([0.15 * scale, -0.45 * scale, 0.3 * scale, 0.27, -0.11, 0.19]).unwrap();
    let (first_exact, second_exact, desired) = if let Some(transform) = common_left {
        (
            transform.compose(&first_exact).unwrap(),
            transform.compose(&second_exact).unwrap(),
            transform.compose(&desired).unwrap(),
        )
    } else {
        (first_exact, second_exact, desired)
    };
    let second_guess = if perturb_second {
        second_exact
            .retract([0.08 * scale, -0.06 * scale, 0.04 * scale, 0.07, -0.05, 0.03])
            .unwrap()
    } else {
        second_exact
    };
    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let first_body = assembly.add_body("first body", first_exact).unwrap();
    let second_body = assembly.add_body("second body", second_guess).unwrap();
    if grounded {
        assembly
            .add_physical_ground("first body ground", first_body)
            .unwrap();
    }

    let desired_frame = frame_from_pose(desired);
    let first_local_frame = local_frame(first_exact, desired_frame);
    let second_world_frame = match primitive {
        Primitive::Revolute(SpatialAxisParity::Aligned) => {
            rotate_frame_about_z(desired_frame, 0.63)
        }
        Primitive::Revolute(SpatialAxisParity::Opposed) => opposed_frame(desired_frame),
        Primitive::Ball | Primitive::Fixed => desired_frame,
    };
    let second_local_frame = local_frame(second_exact, second_world_frame);
    let desired_point = desired_frame.origin();
    let first_local_point = first_exact
        .try_inverse_transform_point(desired_point)
        .unwrap();
    let second_local_point = second_exact
        .try_inverse_transform_point(desired_point)
        .unwrap();

    match primitive {
        Primitive::Ball => {
            let first_point = assembly
                .add_point_feature("first ball point", first_body, first_local_point)
                .unwrap();
            let second_point = assembly
                .add_point_feature("second ball point", second_body, second_local_point)
                .unwrap();
            assembly
                .add_ball_joint("ball joint", first_point, second_point)
                .unwrap();
        }
        Primitive::Fixed => {
            let first_frame = assembly
                .add_frame_feature("first fixed frame", first_body, first_local_frame)
                .unwrap();
            let second_frame = assembly
                .add_frame_feature("second fixed frame", second_body, second_local_frame)
                .unwrap();
            assembly
                .add_fixed_frame("fixed frame joint", first_frame, second_frame)
                .unwrap();
        }
        Primitive::Revolute(parity) => {
            let first_frame = assembly
                .add_frame_feature("first hinge frame", first_body, first_local_frame)
                .unwrap();
            let second_frame = assembly
                .add_frame_feature("second hinge frame", second_body, second_local_frame)
                .unwrap();
            assembly
                .add_revolute_joint("revolute joint", first_frame, second_frame, parity)
                .unwrap();
        }
    }
    Fixture {
        assembly,
        first_body,
        second_body,
    }
}

fn frame_from_pose(pose: Pose3) -> Frame3 {
    Frame3::try_new(
        Point3::from(pose.translation()),
        pose.try_transform_vector(Vector3::x()).unwrap(),
        pose.try_transform_vector(Vector3::y()).unwrap(),
        pose.try_transform_vector(Vector3::z()).unwrap(),
    )
    .unwrap()
}

fn identity_frame(origin: Point3<f64>) -> Frame3 {
    Frame3::try_new(origin, Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
}

fn local_frame(body: Pose3, world: Frame3) -> Frame3 {
    Frame3::try_new(
        body.try_inverse_transform_point(world.origin()).unwrap(),
        body.try_inverse_transform_vector(world.x_axis()).unwrap(),
        body.try_inverse_transform_vector(world.y_axis()).unwrap(),
        body.try_inverse_transform_vector(world.z_axis()).unwrap(),
    )
    .unwrap()
}

fn rotate_frame_about_z(frame: Frame3, angle: f64) -> Frame3 {
    let (sine, cosine) = angle.sin_cos();
    Frame3::try_new(
        frame.origin(),
        frame.x_axis() * cosine + frame.y_axis() * sine,
        -frame.x_axis() * sine + frame.y_axis() * cosine,
        frame.z_axis(),
    )
    .unwrap()
}

fn opposed_frame(frame: Frame3) -> Frame3 {
    Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        -frame.y_axis(),
        -frame.z_axis(),
    )
    .unwrap()
}

fn assert_accepted(session: &SpatialAssemblySession) {
    let result = session.accepted_result();
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.rank_is_valid);
    assert!(result.acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(
        result.geometry.points.iter().all(|point| point
            .world
            .coords
            .iter()
            .all(|value| value.is_finite()))
    );
    assert!(result.geometry.frames.iter().all(|frame| {
        frame
            .world
            .origin()
            .coords
            .iter()
            .all(|value| value.is_finite())
            && frame.world.x_axis().iter().all(|value| value.is_finite())
            && frame.world.y_axis().iter().all(|value| value.is_finite())
            && frame.world.z_axis().iter().all(|value| value.is_finite())
    }));
}

fn assert_private_gauges_absent(session: &SpatialAssemblySession) {
    let result = session.accepted_result();
    assert_eq!(
        result.source_mappings.len(),
        session.assembly().sources().len()
    );
    assert_eq!(
        result.display_audit.sources.len(),
        session.assembly().sources().len()
    );
    assert!(
        result
            .display_audit
            .sources
            .iter()
            .all(|source| !source.source_label.contains("numerical gauge"))
    );
    assert!(result.core_report.conflicting_sources.iter().all(|source| {
        result
            .source_mappings
            .iter()
            .any(|mapping| mapping.core_source_id == *source)
    }));
    assert!(result.core_report.redundant_rows.iter().all(|row| {
        result
            .source_mappings
            .iter()
            .any(|mapping| mapping.core_source_id == row.row.source_id)
    }));
}

fn assert_geometry_close(
    actual: &geosolve_linkage::SpatialGeometry,
    expected: &geosolve_linkage::SpatialGeometry,
    tolerance: f64,
) {
    assert_eq!(actual.bodies.len(), expected.bodies.len());
    for expected_body in &expected.bodies {
        let actual_pose = actual.body_pose(expected_body.body_id).unwrap();
        assert_pose_close(actual_pose, expected_body.pose, tolerance);
    }
}

fn assert_pose_close(actual: Pose3, expected: Pose3, tolerance: f64) {
    let difference = expected.local_difference(&actual).unwrap();
    let maximum = difference
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f64::max);
    assert!(
        maximum <= tolerance,
        "pose difference {difference:?} exceeds {tolerance:e}"
    );
}
