// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineSpanDirection,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintId, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode, DocumentEdit,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentLineOffsetOrientation,
    DocumentLineSide, PersistentId, SketchDocument, SketchDocumentSession, SketchSolveResult,
    VisualProfileAnalysis, VisualProfileCurveFamily, VisualProfileOptions, VisualProfileStatus,
    alpha_scenario,
};

const EXTREME_VALUES: [f64; 7] = [
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::MAX,
    -f64::MAX,
    f64::MIN_POSITIVE,
    f64::from_bits(1),
];

const ALL_PROFILE_FAMILIES: [VisualProfileCurveFamily; 15] = [
    VisualProfileCurveFamily::Line,
    VisualProfileCurveFamily::Polyline,
    VisualProfileCurveFamily::Circle,
    VisualProfileCurveFamily::CircularArc,
    VisualProfileCurveFamily::Ellipse,
    VisualProfileCurveFamily::EllipticalArc,
    VisualProfileCurveFamily::RationalQuadraticConic,
    VisualProfileCurveFamily::Parabola,
    VisualProfileCurveFamily::Hyperbola,
    VisualProfileCurveFamily::QuadraticBezier,
    VisualProfileCurveFamily::CubicBezier,
    VisualProfileCurveFamily::ClampedBSpline,
    VisualProfileCurveFamily::PeriodicBSpline,
    VisualProfileCurveFamily::ClampedNurbs,
    VisualProfileCurveFamily::PeriodicNurbs,
];

#[derive(Clone)]
struct SessionSnapshot {
    canonical: String,
    revision: u64,
    history_len: usize,
    history_cursor: usize,
    history_effects: Vec<DocumentCommandEffect>,
    can_undo: bool,
    can_redo: bool,
    accepted: SketchSolveResult,
}

fn scenario(kind: AlphaScenarioKind) -> (SketchDocumentSession, AlphaScenarioIds) {
    let fixture = alpha_scenario(kind, 1.0).unwrap();
    let session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    (session, fixture.ids)
}

fn prime_nonempty_history(session: &mut SketchDocumentSession) {
    let sentinel = session
        .transact(session.revision(), "M32 mutation history sentinel", |_| {
            Ok(())
        })
        .unwrap();
    assert!(sentinel.accepted());
    let undone = session.undo(session.revision()).unwrap();
    assert!(undone.accepted());
    assert_eq!(session.history_len(), 1);
    assert_eq!(session.history_cursor(), 0);
    assert!(!session.can_undo());
    assert!(session.can_redo());
}

fn snapshot(session: &SketchDocumentSession) -> SessionSnapshot {
    SessionSnapshot {
        canonical: session.export_json().unwrap(),
        revision: session.revision(),
        history_len: session.history_len(),
        history_cursor: session.history_cursor(),
        history_effects: (0..session.history_len())
            .map(|index| session.history_effect(index).unwrap().clone())
            .collect(),
        can_undo: session.can_undo(),
        can_redo: session.can_redo(),
        accepted: session.accepted_result().accepted_view().clone(),
    }
}

fn assert_finite_solve(result: &SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert!(result.unstable_core_report().hard_residual_max.is_finite());
    assert!(result.unstable_core_report().hard_residual_max <= 1.0e-9);
    let acceptance_max = result
        .acceptance_hard_residual_max
        .expect("accepted result has independent sketch validation");
    assert!(acceptance_max.is_finite());
    assert!(acceptance_max <= 1.0e-9);
    assert!(
        result
            .unstable_core_report()
            .singular_values
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(
        result
            .reference_values
            .iter()
            .all(|value| value.value.is_finite())
    );
    assert!(
        result
            .geometry
            .points
            .iter()
            .all(|point| { point.position.x.is_finite() && point.position.y.is_finite() })
    );
    assert!(result.geometry.circles.iter().all(|circle| {
        circle.center.x.is_finite()
            && circle.center.y.is_finite()
            && circle.radius.is_finite()
            && circle.radius > 0.0
    }));
    assert!(result.geometry.arcs.iter().all(|arc| {
        arc.center.x.is_finite()
            && arc.center.y.is_finite()
            && arc.radius.is_finite()
            && arc.radius > 0.0
            && arc.start_angle.is_finite()
            && arc.end_angle.is_finite()
            && arc.signed_sweep.is_finite()
    }));
    assert!(
        result
            .geometry
            .nurbs
            .iter()
            .flat_map(|nurbs| &nurbs.weights)
            .all(|weight| weight.is_finite() && *weight > 0.0)
    );
    assert!(result.display_audit.sources.iter().all(|source| {
        source.rows.iter().all(|row| {
            row.scale.is_finite()
                && row.scale > 0.0
                && row.raw_residual.is_finite()
                && row.normalized_residual.is_finite()
        })
    }));
}

fn assert_finite_document(document: &SketchDocument) {
    let canonical = document.to_canonical_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&canonical)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    assert!(
        document
            .points()
            .iter()
            .all(|point| { point.position[0].is_finite() && point.position[1].is_finite() })
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    for curve in document.curves() {
        for interval in document.visible_curve_intervals(curve.id).unwrap() {
            for parameter in [
                interval.start,
                interval.end,
                0.5 * (interval.start + interval.end),
            ] {
                if let Ok(jet) = document.evaluate_curve_jet(interval.support, parameter) {
                    assert!(jet.position.x.is_finite() && jet.position.y.is_finite());
                    for derivative in [
                        jet.first_derivative,
                        jet.second_derivative,
                        jet.third_derivative,
                    ] {
                        assert!(derivative.x.is_finite() && derivative.y.is_finite());
                    }
                }
            }
        }
    }
}

fn assert_accepted_session(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    assert_finite_solve(accepted.accepted_view());
    assert_finite_document(session.document());
}

fn assert_retained(session: &SketchDocumentSession, before: &SessionSnapshot, case: &str) {
    assert_eq!(session.export_json().unwrap(), before.canonical, "{case}");
    assert_eq!(session.revision(), before.revision, "{case}");
    assert_eq!(session.history_len(), before.history_len, "{case}");
    assert_eq!(session.history_cursor(), before.history_cursor, "{case}");
    assert_eq!(session.can_undo(), before.can_undo, "{case}");
    assert_eq!(session.can_redo(), before.can_redo, "{case}");
    let effects = (0..session.history_len())
        .map(|index| session.history_effect(index).unwrap().clone())
        .collect::<Vec<_>>();
    assert_eq!(effects, before.history_effects, "{case}");
    assert_eq!(
        session.accepted_result().accepted_view(),
        &before.accepted,
        "{case}"
    );
}

fn exercise_command(base: &SketchDocumentSession, case: &str, edit: DocumentEdit) {
    let mut session = base.clone();
    let before = snapshot(&session);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        session.apply(DocumentCommand::new(session.revision(), edit))
    }))
    .unwrap_or_else(|payload| panic!("{case} panicked: {payload:?}"));

    match outcome {
        Ok(outcome) if outcome.accepted() => {
            assert_finite_solve(outcome.result.solve());
            assert_finite_solve(outcome.result.accepted_view());
            assert_accepted_session(&session);
        }
        Ok(outcome) => {
            assert_eq!(outcome.revision, before.revision, "{case}");
            assert!(outcome.effect.is_none(), "{case}");
            assert_eq!(outcome.result.accepted_view(), &before.accepted, "{case}");
            assert_retained(&session, &before, case);
        }
        Err(_) => assert_retained(&session, &before, case),
    }
}

fn dimension_target(document: &SketchDocument, dimension: DocumentDimensionId) -> DesignScalarId {
    match document.dimension(dimension).unwrap().definition {
        DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. }
        | DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::ProfileOffset { target, .. } => target,
    }
}

fn unknown_curve() -> CurveId {
    CurveId(PersistentId::from_u128(u128::MAX - 1))
}

fn unknown_scalar() -> DesignScalarId {
    DesignScalarId(PersistentId::from_u128(u128::MAX - 2))
}

fn unknown_constraint() -> DocumentConstraintId {
    DocumentConstraintId(PersistentId::from_u128(u128::MAX - 3))
}

#[test]
fn malformed_and_extreme_m30_commands_are_panic_safe_and_transactional() {
    let (mut supporting, ids) = scenario(AlphaScenarioKind::SupportingOffset);
    prime_nonempty_history(&mut supporting);
    let AlphaScenarioIds::SupportingOffset(ids) = ids else {
        panic!("supporting offset IDs")
    };
    let target = dimension_target(supporting.document(), ids.dimension);
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &supporting,
            &format!("supporting offset point value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.target_points[1],
                position: [value, -value],
            },
        );
        exercise_command(
            &supporting,
            &format!("supporting offset target value {index}"),
            DocumentEdit::SetScalarValue {
                scalar: target,
                value,
            },
        );
    }
    exercise_command(
        &supporting,
        "supporting offset unknown source",
        DocumentEdit::CreateDimension {
            label: "mutated supporting offset".into(),
            definition: DocumentDimensionDefinition::SupportingLineOffset {
                source: CurveSpan::line(unknown_curve()),
                target_segment: CurveSpan::line(ids.target),
                target,
                side: DocumentLineSide::Left,
                orientation: DocumentLineOffsetOrientation::Same,
            },
            mode: DocumentDimensionMode::Driving,
        },
    );
    exercise_command(
        &supporting,
        "supporting offset invalid segment and scalar",
        DocumentEdit::CreateDimension {
            label: "mutated supporting offset".into(),
            definition: DocumentDimensionDefinition::SupportingLineOffset {
                source: CurveSpan {
                    curve: ids.source,
                    segment: u32::MAX,
                },
                target_segment: CurveSpan::line(ids.target),
                target: unknown_scalar(),
                side: DocumentLineSide::Right,
                orientation: DocumentLineOffsetOrientation::Reversed,
            },
            mode: DocumentDimensionMode::Driving,
        },
    );

    let (mut exact, ids) = scenario(AlphaScenarioKind::ExactTranslatedOffset);
    prime_nonempty_history(&mut exact);
    let AlphaScenarioIds::ExactTranslatedOffset(ids) = ids else {
        panic!("exact offset IDs")
    };
    let target = dimension_target(exact.document(), ids.dimension);
    exercise_command(
        &exact,
        "exact offset unknown target span",
        DocumentEdit::CreateDimension {
            label: "mutated exact offset".into(),
            definition: DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
                source: CurveSpan::line(ids.source),
                target_segment: CurveSpan {
                    curve: unknown_curve(),
                    segment: u32::MAX,
                },
                target,
                side: DocumentLineSide::Left,
                orientation: DocumentLineOffsetOrientation::Same,
            },
            mode: DocumentDimensionMode::Driving,
        },
    );
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &exact,
            &format!("exact offset source point value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.source_end,
                position: [value, value],
            },
        );
    }

    let (mut mirror, ids) = scenario(AlphaScenarioKind::EntityMirror);
    prime_nonempty_history(&mut mirror);
    let AlphaScenarioIds::EntityMirror(ids) = ids else {
        panic!("mirror IDs")
    };
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &mirror,
            &format!("mirror source point value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.source_end,
                position: [value, -value],
            },
        );
    }
    for (case, source_curve, axis) in [
        (
            "mirror unknown source",
            unknown_curve(),
            CurveSpan::line(ids.axis),
        ),
        (
            "mirror unknown axis",
            ids.mirror.source_curve,
            CurveSpan::line(unknown_curve()),
        ),
        (
            "mirror invalid axis segment",
            ids.mirror.source_curve,
            CurveSpan {
                curve: ids.axis,
                segment: u32::MAX,
            },
        ),
    ] {
        exercise_command(
            &mirror,
            case,
            DocumentEdit::CreateMirroredCurve {
                label: "mutated mirror".into(),
                source_curve,
                axis,
            },
        );
    }
    exercise_command(
        &mirror,
        "mirror empty label",
        DocumentEdit::CreateMirroredCurve {
            label: String::new(),
            source_curve: ids.mirror.source_curve,
            axis: CurveSpan::line(ids.axis),
        },
    );

    let (mut angle, ids) = scenario(AlphaScenarioKind::DirectedAngle);
    prime_nonempty_history(&mut angle);
    let AlphaScenarioIds::DirectedAngle(ids) = ids else {
        panic!("directed angle IDs")
    };
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &angle,
            &format!("directed angle target value {index}"),
            DocumentEdit::SetScalarValue {
                scalar: ids.target,
                value,
            },
        );
        exercise_command(
            &angle,
            &format!("directed angle point value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.moving_tip,
                position: [value, -value],
            },
        );
    }
    exercise_command(
        &angle,
        "directed angle wrong-kind dimension",
        DocumentEdit::SetOrientedAngleOrientation {
            dimension: DocumentDimensionId(ids.target.0),
            orientation: DocumentAngleOrientation::Clockwise,
        },
    );

    let (mut line_fillet, ids) = scenario(AlphaScenarioKind::M27ReferenceFillet);
    prime_nonempty_history(&mut line_fillet);
    let AlphaScenarioIds::M27ReferenceFillet(ids) = ids else {
        panic!("line fillet IDs")
    };
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &line_fillet,
            &format!("line fillet radius value {index}"),
            DocumentEdit::SetScalarValue {
                scalar: ids.fillet.radius_target,
                value,
            },
        );
        exercise_command(
            &line_fillet,
            &format!("line fillet center value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.fillet.center,
                position: [value, value],
            },
        );
    }
    exercise_command(
        &line_fillet,
        "line fillet unknown association",
        DocumentEdit::SetLineLineFilletBranch {
            constraint: unknown_constraint(),
            first_side: DocumentCurveNormalSide::Right,
            second_side: DocumentCurveNormalSide::Left,
            endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
            sweep: DocumentArcSweep::Clockwise,
        },
    );

    let (mut generic_fillet, ids) = scenario(AlphaScenarioKind::FilletLineBezier);
    prime_nonempty_history(&mut generic_fillet);
    let AlphaScenarioIds::FilletLineBezier(ids) = ids else {
        panic!("generic fillet IDs")
    };
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &generic_fillet,
            &format!("generic fillet radius value {index}"),
            DocumentEdit::SetScalarValue {
                scalar: ids.fillet.radius_target,
                value,
            },
        );
        exercise_command(
            &generic_fillet,
            &format!("generic fillet center value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.fillet.center,
                position: [value, -value],
            },
        );
    }
    exercise_command(
        &generic_fillet,
        "generic fillet unknown association",
        DocumentEdit::SetCurveCurveFilletBranch {
            constraint: unknown_constraint(),
            first_side: DocumentCurveNormalSide::Right,
            first_trim_endpoint: DocumentFilletTrimEndpoint::Start,
            second_side: DocumentCurveNormalSide::Left,
            second_trim_endpoint: DocumentFilletTrimEndpoint::End,
            endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
            sweep: DocumentArcSweep::Clockwise,
        },
    );

    let (mut quarter, ids) = scenario(AlphaScenarioKind::NurbsQuarterCircle);
    prime_nonempty_history(&mut quarter);
    let AlphaScenarioIds::NurbsQuarterCircle(ids) = ids else {
        panic!("quarter-circle NURBS IDs")
    };
    for (index, value) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &quarter,
            &format!("NURBS control value {index}"),
            DocumentEdit::SetPointPosition {
                point: ids.primary_control,
                position: [value, -value],
            },
        );
        exercise_command(
            &quarter,
            &format!("NURBS weight value {index}"),
            DocumentEdit::SetScalarValue {
                scalar: ids.weights[1],
                value,
            },
        );
    }
    exercise_command(
        &quarter,
        "NURBS unknown gauge weight",
        DocumentEdit::SetNurbsWeightGauge {
            curve: ids.curve,
            gauge_weight: unknown_scalar(),
        },
    );
    exercise_command(
        &quarter,
        "NURBS unknown gauge curve",
        DocumentEdit::SetNurbsWeightGauge {
            curve: unknown_curve(),
            gauge_weight: ids.weights[1],
        },
    );

    let (mut local, ids) = scenario(AlphaScenarioKind::NurbsLocalSupport);
    prime_nonempty_history(&mut local);
    let AlphaScenarioIds::NurbsLocalSupport(ids) = ids else {
        panic!("local-support NURBS IDs")
    };
    for (index, parameter) in EXTREME_VALUES.into_iter().enumerate() {
        exercise_command(
            &local,
            &format!("NURBS knot parameter {index}"),
            DocumentEdit::InsertNurbsKnot {
                curve: ids.curve,
                parameter,
            },
        );
    }
    exercise_command(
        &local,
        "NURBS knot unknown curve",
        DocumentEdit::InsertNurbsKnot {
            curve: unknown_curve(),
            parameter: 0.5,
        },
    );

    let (mut periodic, ids) = scenario(AlphaScenarioKind::NurbsPeriodic);
    prime_nonempty_history(&mut periodic);
    let AlphaScenarioIds::NurbsPeriodic(ids) = ids else {
        panic!("periodic NURBS IDs")
    };
    exercise_command(
        &periodic,
        "NURBS transition unknown contact",
        DocumentEdit::TransitionNurbsContact {
            contact: geosolve_sketch::ContactId(PersistentId::from_u128(u128::MAX - 4)),
            direction: DocumentBSplineSpanDirection::Next,
        },
    );
    exercise_command(
        &periodic,
        "NURBS transition wrong-kind contact",
        DocumentEdit::TransitionNurbsContact {
            contact: geosolve_sketch::ContactId(ids.weights[1].0),
            direction: DocumentBSplineSpanDirection::Previous,
        },
    );
}

fn assert_ordered_finite(enclosure: [f64; 2]) -> bool {
    enclosure[0].is_finite() && enclosure[1].is_finite() && enclosure[0] <= enclosure[1]
}

fn assert_profile_evidence(
    analysis: &VisualProfileAnalysis,
    options: VisualProfileOptions,
    expected_complete_faces: Option<usize>,
) {
    let counters = [
        (
            analysis.budgets.candidate_pairs,
            options.max_candidate_pairs,
        ),
        (
            analysis.budgets.intersection_subdivisions,
            options.max_intersection_subdivisions,
        ),
        (
            analysis.budgets.intersection_roots,
            options.max_intersection_roots,
        ),
        (analysis.budgets.fragments, options.max_fragments),
        (
            analysis.budgets.integration_subdivisions,
            options.max_integration_subdivisions,
        ),
        (
            analysis.budgets.containment_tests,
            options.max_containment_tests,
        ),
        (analysis.budgets.faces, options.max_faces),
    ];
    for (counter, limit) in counters {
        assert_eq!(counter.limit, limit, "{analysis:#?}");
        assert!(counter.consumed <= counter.limit, "{analysis:#?}");
    }
    assert_eq!(
        analysis.candidate_pairs,
        analysis.budgets.candidate_pairs.consumed
    );
    assert_eq!(analysis.fragment_count, analysis.budgets.fragments.consumed);
    assert!(analysis.intersections.iter().all(|intersection| {
        assert_ordered_finite(intersection.first_parameter_enclosure)
            && assert_ordered_finite(intersection.second_parameter_enclosure)
            && intersection
                .position_enclosure
                .into_iter()
                .flatten()
                .all(f64::is_finite)
            && intersection.position_enclosure[0][0] <= intersection.position_enclosure[1][0]
            && intersection.position_enclosure[0][1] <= intersection.position_enclosure[1][1]
    }));
    for face in &analysis.faces {
        assert!(face.visual_area.is_finite());
        assert!(face.area_uncertainty.is_finite());
        assert!(face.area_uncertainty >= 0.0);
        for contour in &face.contours {
            assert!(contour.signed_area.is_finite());
            assert!(contour.area_uncertainty.is_finite());
            assert!(contour.area_uncertainty >= 0.0);
            for edge in &contour.edges {
                assert!(edge.start.into_iter().all(f64::is_finite));
                assert!(edge.end.into_iter().all(f64::is_finite));
                assert!(edge.source_parameters.into_iter().all(f64::is_finite));
                assert!(
                    edge.source_parameter_enclosures
                        .into_iter()
                        .all(assert_ordered_finite)
                );
            }
        }
    }
    if analysis.status == VisualProfileStatus::Complete {
        assert!(analysis.issues.is_empty(), "{analysis:#?}");
        for family in ALL_PROFILE_FAMILIES {
            assert!(analysis.families.contains(&family), "{analysis:#?}");
        }
        if let Some(minimum) = expected_complete_faces {
            assert!(analysis.faces.len() >= minimum, "{analysis:#?}");
        }
    } else {
        assert!(!analysis.issues.is_empty(), "{analysis:#?}");
    }
}

fn analyze_under_guard(
    document: &SketchDocument,
    case: &str,
    options: VisualProfileOptions,
    expected_complete_faces: Option<usize>,
) -> VisualProfileAnalysis {
    let canonical = document.to_canonical_json().unwrap();
    let analysis = catch_unwind(AssertUnwindSafe(|| {
        document.analyze_visual_profiles(options)
    }))
    .unwrap_or_else(|payload| panic!("{case} panicked: {payload:?}"));
    assert_eq!(document.to_canonical_json().unwrap(), canonical, "{case}");
    assert_profile_evidence(&analysis, options, expected_complete_faces);
    analysis
}

fn family_mutation_targets(
    definition: &CurveDefinition,
) -> (DesignPointId, Option<DesignScalarId>) {
    match definition {
        CurveDefinition::Line { end, .. } => (*end, None),
        CurveDefinition::Polyline { points, .. } => (points[1], None),
        CurveDefinition::Circle { center, radius }
        | CurveDefinition::CircularArc { center, radius, .. } => (*center, Some(*radius)),
        CurveDefinition::QuadraticBezier { controls } => (controls[1], None),
        CurveDefinition::CubicBezier { controls } => (controls[1], None),
        CurveDefinition::Ellipse {
            major_axis_point,
            minor_axis_ratio,
            ..
        }
        | CurveDefinition::EllipticalArc {
            major_axis_point,
            minor_axis_ratio,
            ..
        } => (*major_axis_point, Some(*minor_axis_ratio)),
        CurveDefinition::RationalQuadraticConic {
            start,
            middle_weight,
            ..
        } => (*start, Some(*middle_weight)),
        CurveDefinition::ParabolaSegment {
            focus, trim_end, ..
        } => (*focus, Some(*trim_end)),
        CurveDefinition::HyperbolaSegment {
            transverse_axis_point,
            semi_conjugate,
            ..
        } => (*transverse_axis_point, Some(*semi_conjugate)),
        CurveDefinition::BSpline { controls, .. } => (controls[1], None),
        CurveDefinition::Nurbs {
            controls,
            weights,
            gauge_weight,
            ..
        } => (
            controls[1],
            weights
                .iter()
                .copied()
                .find(|weight| weight != gauge_weight),
        ),
    }
}

#[test]
fn all_m31_profile_families_and_options_are_panic_safe_and_bounded() {
    let fixture = alpha_scenario(AlphaScenarioKind::ProfileAllFamilies, 1.0).unwrap();
    let AlphaScenarioIds::ProfileAllFamilies(ids) = fixture.ids else {
        panic!("all-family profile IDs")
    };
    assert_eq!(ids.curves.len(), ALL_PROFILE_FAMILIES.len());

    let default = VisualProfileOptions::default();
    let baseline = analyze_under_guard(&fixture.document, "default options", default, Some(15));
    assert_eq!(
        baseline.status,
        VisualProfileStatus::Complete,
        "{baseline:#?}"
    );

    let zero = VisualProfileOptions {
        max_candidate_pairs: 0,
        max_intersection_subdivisions: 0,
        max_intersection_depth: 0,
        max_intersection_roots: 0,
        max_fragments: 0,
        max_integration_subdivisions: 0,
        max_containment_tests: 0,
        max_faces: 0,
    };
    let maximum = VisualProfileOptions {
        max_candidate_pairs: usize::MAX,
        max_intersection_subdivisions: usize::MAX,
        max_intersection_depth: usize::MAX,
        max_intersection_roots: usize::MAX,
        max_fragments: usize::MAX,
        max_integration_subdivisions: usize::MAX,
        max_containment_tests: usize::MAX,
        max_faces: usize::MAX,
    };
    let option_mutations = [
        ("all zero", zero),
        (
            "candidate pairs zero",
            VisualProfileOptions {
                max_candidate_pairs: 0,
                ..default
            },
        ),
        (
            "intersection subdivisions zero",
            VisualProfileOptions {
                max_intersection_subdivisions: 0,
                ..default
            },
        ),
        (
            "intersection depth zero",
            VisualProfileOptions {
                max_intersection_depth: 0,
                ..default
            },
        ),
        (
            "intersection roots zero",
            VisualProfileOptions {
                max_intersection_roots: 0,
                ..default
            },
        ),
        (
            "fragments zero",
            VisualProfileOptions {
                max_fragments: 0,
                ..default
            },
        ),
        (
            "integration subdivisions zero",
            VisualProfileOptions {
                max_integration_subdivisions: 0,
                ..default
            },
        ),
        (
            "containment tests zero",
            VisualProfileOptions {
                max_containment_tests: 0,
                ..default
            },
        ),
        (
            "faces zero",
            VisualProfileOptions {
                max_faces: 0,
                ..default
            },
        ),
        ("all usize max", maximum),
    ];
    for (case, options) in option_mutations {
        analyze_under_guard(&fixture.document, case, options, Some(15));
    }

    for ((family, curve), ordinal) in ALL_PROFILE_FAMILIES
        .into_iter()
        .zip(ids.curves)
        .zip(1_u32..)
    {
        let definition = &fixture.document.curve(curve).unwrap().definition;
        let (point, scalar) = family_mutation_targets(definition);
        let mut mutated = fixture.document.clone();
        let magnitude = 1.0e150;
        mutated
            .set_point_position(point, [magnitude * f64::from(ordinal), -magnitude])
            .unwrap();
        analyze_under_guard(
            &mutated,
            &format!("{family:?} extreme point"),
            default,
            None,
        );

        if let Some(scalar) = scalar {
            for value in [f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)] {
                let mut mutated = fixture.document.clone();
                if mutated.set_scalar_value(scalar, value).is_ok() {
                    analyze_under_guard(
                        &mutated,
                        &format!("{family:?} extreme scalar {value:e}"),
                        default,
                        None,
                    );
                    break;
                }
            }
        }
    }
}
