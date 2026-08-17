// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    CurveDefinition, CurveId, DocumentArcSweep, DocumentCommand, DocumentCommandEffect,
    DocumentCurveControlId, DocumentCurveControlKind, DocumentCurveControlProjection,
    DocumentCurveControlTarget, DocumentEdit, DocumentHyperbolaBranch,
    DocumentRationalConicControl, DocumentRationalConicControlMode, DocumentSolveRequest,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, OperationControl, OperationOutcome,
    PreparedSketchOperation, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
    SketchDocument, SketchDocumentSession,
};

#[derive(Clone, Copy)]
struct Gallery {
    circle: CurveId,
    circle_radius: geosolve_sketch::DesignScalarId,
    arc: CurveId,
    ellipse: CurveId,
    ellipse_ratio: geosolve_sketch::DesignScalarId,
    rational: CurveId,
    rational_weight: geosolve_sketch::DesignScalarId,
    parabola: CurveId,
    hyperbola: CurveId,
    hyperbola_conjugate: geosolve_sketch::DesignScalarId,
}

fn ratio_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    }
}

fn weight_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    }
}

#[allow(clippy::too_many_lines)]
fn gallery() -> (SketchDocument, Gallery) {
    let mut document = SketchDocument::new(10.0).unwrap();

    let circle_center = document.add_point("circle center", [1.0, 2.0]).unwrap();
    let circle_radius = document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();

    let arc_center = document.add_point("arc center", [10.0, 1.0]).unwrap();
    let arc_radius = document
        .add_scalar(
            "arc radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = document
        .add_scalar("arc start", 0.2, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = document
        .add_scalar("arc end", 1.4, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();

    let ellipse_center = document.add_point("ellipse center", [20.0, 0.0]).unwrap();
    let ellipse_axis = document.add_point("ellipse axis", [22.4, 1.8]).unwrap();
    let ellipse_ratio = document
        .add_scalar("ellipse ratio", 0.5, ScalarUnit::Parameter, ratio_domain())
        .unwrap();
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_axis,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .unwrap();

    let rational_start = document.add_point("rational start", [30.0, 0.0]).unwrap();
    let rational_end = document.add_point("rational end", [34.0, 0.0]).unwrap();
    let rational_weight = document
        .add_scalar(
            "rational weight",
            0.5,
            ScalarUnit::Parameter,
            weight_domain(),
        )
        .unwrap();
    let rational = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [16.0, 1.0],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .unwrap();

    let vertex = document.add_point("vertex", [40.0, 0.0]).unwrap();
    let focus = document.add_point("focus", [41.0, 0.0]).unwrap();
    let parabola_start = document
        .add_scalar(
            "parabola start",
            -0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola_end = document
        .add_scalar(
            "parabola end",
            1.2,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola = document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start: parabola_start,
                trim_end: parabola_end,
            },
        )
        .unwrap();

    let hyperbola_center = document
        .add_point("hyperbola center", [50.0, -1.0])
        .unwrap();
    let hyperbola_axis = document.add_point("hyperbola axis", [52.0, 0.5]).unwrap();
    let hyperbola_conjugate = document
        .add_scalar(
            "hyperbola conjugate",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let hyperbola_start = document
        .add_scalar(
            "hyperbola start",
            -0.7,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola_end = document
        .add_scalar(
            "hyperbola end",
            0.9,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola = document
        .add_curve(
            "hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate: hyperbola_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .unwrap();

    document.validate().unwrap();
    (
        document,
        Gallery {
            circle,
            circle_radius,
            arc,
            ellipse,
            ellipse_ratio,
            rational,
            rational_weight,
            parabola,
            hyperbola,
            hyperbola_conjugate,
        },
    )
}

fn control(
    document: &SketchDocument,
    curve: CurveId,
    kind: DocumentCurveControlKind,
) -> geosolve_sketch::DocumentCurveControl {
    document
        .curve_controls(curve)
        .unwrap()
        .into_iter()
        .find(|control| control.id.kind == kind)
        .unwrap()
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
}

#[test]
fn catalog_exposes_persistent_aliases_and_derived_controls_without_schema_state() {
    let (document, ids) = gallery();
    let before = document.to_canonical_json().unwrap();

    let circle = document.curve_controls(ids.circle).unwrap();
    assert_eq!(
        circle
            .iter()
            .map(|control| control.id.kind)
            .collect::<Vec<_>>(),
        vec![
            DocumentCurveControlKind::Center,
            DocumentCurveControlKind::Radius,
        ]
    );
    assert_eq!(pair_bits(circle[1].position), pair_bits([3.0, 2.0]));
    assert_eq!(
        circle[1].target,
        DocumentCurveControlTarget::Scalar(ids.circle_radius)
    );

    for (curve, required) in [
        (
            ids.arc,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::Radius,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
        (
            ids.ellipse,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::MajorAxisPoint,
                DocumentCurveControlKind::MinorAxis,
            ],
        ),
        (
            ids.parabola,
            vec![
                DocumentCurveControlKind::Vertex,
                DocumentCurveControlKind::Focus,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
        (
            ids.hyperbola,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::TransverseAxisPoint,
                DocumentCurveControlKind::ConjugateAxis,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
    ] {
        let actual = document
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .map(|control| control.id.kind)
            .collect::<Vec<_>>();
        assert_eq!(actual, required);
    }

    let rational = document.curve_controls(ids.rational).unwrap();
    assert_eq!(
        rational[1].id.kind,
        DocumentCurveControlKind::RationalMiddle
    );
    assert_eq!(pair_bits(rational[1].position), pair_bits([32.0, 2.0]));
    assert_eq!(
        rational[1].target,
        DocumentCurveControlTarget::RationalMiddle {
            weight: ids.rational_weight,
            mode: DocumentRationalConicControlMode::Euclidean,
        }
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn radius_minor_and_conjugate_grips_inverse_project_in_rotated_frames() {
    let (document, ids) = gallery();

    let radius = DocumentCurveControlId {
        curve: ids.circle,
        kind: DocumentCurveControlKind::Radius,
    };
    assert_eq!(
        document.project_curve_control(radius, [1.0, 6.0]).unwrap(),
        DocumentCurveControlProjection::Scalar {
            scalar: ids.circle_radius,
            value: 4.0,
        }
    );

    let minor = control(&document, ids.ellipse, DocumentCurveControlKind::MinorAxis);
    let center = control(&document, ids.ellipse, DocumentCurveControlKind::Center).position;
    let target = [
        center[0] + 1.5 * (minor.position[0] - center[0]),
        center[1] + 1.5 * (minor.position[1] - center[1]),
    ];
    let DocumentCurveControlProjection::Scalar { scalar, value } =
        document.project_curve_control(minor.id, target).unwrap()
    else {
        panic!("minor scalar projection expected")
    };
    assert_eq!(scalar, ids.ellipse_ratio);
    assert!((value - 0.75).abs() <= 1.0e-12, "ratio={value}");

    let conjugate = control(
        &document,
        ids.hyperbola,
        DocumentCurveControlKind::ConjugateAxis,
    );
    let center = control(&document, ids.hyperbola, DocumentCurveControlKind::Center).position;
    let target = [
        center[0] - 1.5 * (conjugate.position[0] - center[0]),
        center[1] - 1.5 * (conjugate.position[1] - center[1]),
    ];
    let DocumentCurveControlProjection::Scalar { scalar, value } = document
        .project_curve_control(conjugate.id, target)
        .unwrap()
    else {
        panic!("conjugate scalar projection expected")
    };
    assert_eq!(scalar, ids.hyperbola_conjugate);
    assert!((value - 3.0).abs() <= 1.0e-12, "semi-conjugate={value}");
}

#[test]
fn rational_control_modes_are_explicit_atomic_and_round_trip_existing_storage() {
    let (mut document, ids) = gallery();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [32.0, 2.0],
            weight: 0.5,
        }
    );

    document
        .set_rational_conic_control(
            ids.rational,
            DocumentRationalConicControl::Euclidean {
                middle: [31.5, 2.5],
                weight: -0.5,
            },
        )
        .unwrap();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [31.5, 2.5],
            weight: -0.5,
        }
    );
    let CurveDefinition::RationalQuadraticConic {
        weighted_middle, ..
    } = document.curve(ids.rational).unwrap().definition
    else {
        panic!("rational curve expected")
    };
    assert_eq!(pair_bits(weighted_middle), pair_bits([-15.75, -1.25]));

    document
        .set_rational_conic_control(
            ids.rational,
            DocumentRationalConicControl::Projective {
                weighted_middle,
                weight: 0.0,
            },
        )
        .unwrap();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Projective {
            weighted_middle,
            weight: 0.0,
        }
    );
    let projective = control(
        &document,
        ids.rational,
        DocumentCurveControlKind::RationalMiddle,
    );
    assert_eq!(
        projective.target,
        DocumentCurveControlTarget::RationalMiddle {
            weight: ids.rational_weight,
            mode: DocumentRationalConicControlMode::Projective,
        }
    );
    assert_eq!(pair_bits(projective.position), pair_bits([14.25, -1.25]));
    assert_eq!(
        document
            .project_curve_control(projective.id, [32.0, 3.0])
            .unwrap(),
        DocumentCurveControlProjection::RationalMiddle {
            curve: ids.rational,
            control: DocumentRationalConicControl::Projective {
                weighted_middle: [2.0, 3.0],
                weight: 0.0,
            },
        }
    );

    let before = document.to_canonical_json().unwrap();
    assert!(
        document
            .set_rational_conic_control(
                ids.rational,
                DocumentRationalConicControl::Euclidean {
                    middle: [31.0, 2.0],
                    weight: 0.0,
                },
            )
            .is_err()
    );
    assert!(
        document
            .set_rational_conic_control(
                ids.rational,
                DocumentRationalConicControl::Projective {
                    weighted_middle: [2.0, 3.0],
                    weight: 0.25,
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

fn completed_patch(
    outcome: OperationOutcome<geosolve_sketch::PreparedSketchPatch>,
) -> geosolve_sketch::PreparedSketchPatch {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } => panic!("prepared operation cancelled"),
        OperationOutcome::WorkExhausted { .. } => panic!("prepared operation exhausted"),
        _ => panic!("unknown prepared outcome"),
    }
}

#[test]
fn prepared_preview_is_read_only_exact_and_commits_the_visible_rational_candidate() {
    let (document, ids) = gallery();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before_input = session.prepared_input();
    let before_json = session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .to_canonical_json()
        .unwrap();
    let patch = completed_patch(
        session
            .prepared_snapshot()
            .prepare(PreparedSketchOperation::Apply(
                DocumentEdit::SetRationalConicControl {
                    curve: ids.rational,
                    control: DocumentRationalConicControl::Euclidean {
                        middle: [31.25, 2.75],
                        weight: 0.8,
                    },
                },
            ))
            .execute(OperationControl::unlimited())
            .unwrap(),
    );

    let preview = patch.preview();
    assert_eq!(preview.base_input(), before_input);
    assert_eq!(
        preview.candidate_input().design_identity(),
        preview.proposed_commit().design_identity()
    );
    assert_eq!(
        preview.candidate_input().latest_attempt_identity(),
        preview.proposed_commit().attempt_identity()
    );
    assert_eq!(
        preview.candidate_input().accepted_state_identity(),
        preview.proposed_commit().accepted_state_identity()
    );
    assert_eq!(
        preview
            .accepted_document()
            .unwrap()
            .rational_conic_control(ids.rational)
            .unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [31.25, 2.75],
            weight: 0.8,
        }
    );
    let preview_json = preview
        .accepted_document()
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        before_json
    );

    let proposed = patch.proposed_commit();
    assert_eq!(session.commit_prepared_patch(patch).unwrap(), proposed);
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        preview_json
    );
}

#[test]
fn accepted_session_rational_edit_is_one_undoable_transaction() {
    let (document, ids) = gallery();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.export_json().unwrap();
    let before_history = session.history_len();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetRationalConicControl {
                curve: ids.rational,
                control: DocumentRationalConicControl::Euclidean {
                    middle: [31.5, 2.25],
                    weight: 0.75,
                },
            },
        ))
        .unwrap();
    assert!(outcome.accepted());
    assert_eq!(
        outcome.effect,
        Some(DocumentCommandEffect::UpdatedRationalConicControl(
            ids.rational
        ))
    );
    assert_eq!(session.history_len(), before_history + 1);
    session.undo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), before);
}
