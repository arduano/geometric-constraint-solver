#![allow(clippy::too_many_lines)]

use geosolve_core::{HardValidity, SolveTermination, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DimensionKind, DimensionMode, DocumentAngleOrientation,
    DocumentBSplineForm, DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
    DocumentLineOffsetOrientation, DocumentLineSide, DocumentSolveRequest, LineOffsetOrientation,
    LineSide, ScalarDomain, ScalarUnit, Sketch, SketchDocument, SketchDocumentSession, SketchError,
    SketchSolveRequest, SolveRejection,
};

const TOLERANCE: f64 = 1.0e-9;

fn solve(sketch: &mut Sketch) -> geosolve_sketch::SketchSolveResult {
    sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap()
}

fn assert_accepted(result: &geosolve_sketch::SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(
        result.unstable_core_report().termination,
        SolveTermination::Converged
    );
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    assert!(result.unstable_core_report().hard_residuals_validated);
    assert!(result.unstable_core_report().hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
}

fn transformed(point: [f64; 2], scale: f64, angle: f64, offset: [f64; 2]) -> [f64; 2] {
    let (sine, cosine) = angle.sin_cos();
    [
        scale * (cosine * point[0] - sine * point[1]) + offset[0],
        scale * (sine * point[0] + cosine * point[1]) + offset[1],
    ]
}

fn line_pair(
    scale: f64,
    right_side: bool,
    reversed: bool,
) -> (
    Sketch,
    geosolve_sketch::SegmentId,
    geosolve_sketch::SegmentId,
) {
    let mut sketch = Sketch::new(scale).unwrap();
    let offset = if right_side {
        -2.0 * scale
    } else {
        2.0 * scale
    };
    let source_start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let source_end = sketch.add_point(Point2::new(4.0 * scale, 0.0)).unwrap();
    let first_target = if reversed { 4.0 * scale } else { 0.0 };
    let second_target = if reversed { 0.0 } else { 4.0 * scale };
    let target_start = sketch.add_point(Point2::new(first_target, offset)).unwrap();
    let target_end = sketch
        .add_point(Point2::new(second_target, offset))
        .unwrap();
    let source = sketch.add_segment(source_start, source_end).unwrap();
    let target = sketch.add_segment(target_start, target_end).unwrap();
    sketch.add_fixed_point(source_start).unwrap();
    sketch.add_fixed_point(source_end).unwrap();
    (sketch, source, target)
}

#[test]
fn offset_modes_have_analytic_jacobians_audits_and_truthful_dof_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for (side, right_side) in [(LineSide::Left, false), (LineSide::Right, true)] {
            for (orientation, reversed) in [
                (LineOffsetOrientation::Same, false),
                (LineOffsetOrientation::Reversed, true),
            ] {
                let (mut supporting, source, target_segment) =
                    line_pair(scale, right_side, reversed);
                let dimension = supporting
                    .add_supporting_line_offset(
                        source,
                        target_segment,
                        2.0 * scale,
                        side,
                        orientation,
                        DimensionMode::Driving,
                    )
                    .unwrap();
                assert!(matches!(
                    supporting.dimension(dimension).unwrap().kind(),
                    DimensionKind::SupportingLineOffset { source: actual_source, target_segment: actual_target, target, side: actual_side, orientation: actual_orientation }
                        if actual_source == source
                            && actual_target == target_segment
                            && target.to_bits() == (2.0 * scale).to_bits()
                            && actual_side == side
                            && actual_orientation == orientation
                ));
                let compiled = supporting.compile(SketchSolveRequest::default()).unwrap();
                let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
                assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
                let audit = compiled.problem().audit_rows().unwrap();
                let offset_audit = audit
                    .iter()
                    .filter(|row| row.template.contains("oriented_target"))
                    .collect::<Vec<_>>();
                assert_eq!(offset_audit.len(), 2);
                assert!(offset_audit.iter().all(|row| {
                    !row.template.is_empty()
                        && !row.bindings.is_empty()
                        && row.scale.is_finite()
                        && row.scale > 0.0
                }));
                let supporting_result = solve(&mut supporting);
                assert_accepted(&supporting_result);
                assert_eq!(supporting_result.unstable_core_report().rank, 2);
                assert_eq!(
                    supporting_result
                        .unstable_core_report()
                        .local_degrees_of_freedom,
                    2
                );

                let (mut exact, source, target_segment) = line_pair(scale, right_side, reversed);
                exact
                    .add_exact_translated_segment_offset(
                        source,
                        target_segment,
                        2.0 * scale,
                        side,
                        orientation,
                        DimensionMode::Driving,
                    )
                    .unwrap();
                let compiled = exact.compile(SketchSolveRequest::default()).unwrap();
                let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
                assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
                assert_eq!(
                    compiled
                        .problem()
                        .audit_rows()
                        .unwrap()
                        .iter()
                        .filter(|row| row.template.contains("oriented_target"))
                        .count(),
                    4
                );
                let exact_result = solve(&mut exact);
                assert_accepted(&exact_result);
                assert_eq!(exact_result.unstable_core_report().rank, 4);
                assert_eq!(
                    exact_result.unstable_core_report().local_degrees_of_freedom,
                    0
                );
            }
        }
    }
}

#[test]
fn offsets_and_mirrors_are_rotation_translation_and_scale_invariant() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let angle = 0.61;
        let translation = [7.0 * scale, -4.0 * scale];
        let source_positions = [
            transformed([0.0, 0.0], scale, angle, translation),
            transformed([4.0, 0.0], scale, angle, translation),
        ];
        let target_positions = [
            transformed([0.0, 2.0], scale, angle, translation),
            transformed([4.0, 2.0], scale, angle, translation),
        ];
        let mut sketch = Sketch::new(scale).unwrap();
        let source_points = source_positions.map(|position| {
            sketch
                .add_point(Point2::new(position[0], position[1]))
                .unwrap()
        });
        let target_points = target_positions.map(|position| {
            sketch
                .add_point(Point2::new(position[0], position[1]))
                .unwrap()
        });
        let source = sketch
            .add_segment(source_points[0], source_points[1])
            .unwrap();
        let target = sketch
            .add_segment(target_points[0], target_points[1])
            .unwrap();
        sketch.add_fixed_point(source_points[0]).unwrap();
        sketch.add_fixed_point(source_points[1]).unwrap();
        sketch
            .add_exact_translated_segment_offset(
                source,
                target,
                2.0 * scale,
                LineSide::Left,
                LineOffsetOrientation::Same,
                DimensionMode::Driving,
            )
            .unwrap();
        assert_accepted(&solve(&mut sketch));

        let mut document = SketchDocument::new(scale).unwrap();
        let axis_points = [
            transformed([-5.0, 0.0], scale, angle, translation),
            transformed([5.0, 0.0], scale, angle, translation),
        ]
        .map(|position| document.add_point("axis", position).unwrap());
        let axis = document
            .add_curve(
                "axis",
                CurveDefinition::Line {
                    start: axis_points[0],
                    end: axis_points[1],
                    branch_direction: [angle.cos(), angle.sin()],
                },
            )
            .unwrap();
        let source_positions = [
            transformed([0.0, 1.0], scale, angle, translation),
            transformed([3.0, 2.0], scale, angle, translation),
        ];
        let source_points =
            source_positions.map(|position| document.add_point("source", position).unwrap());
        let source_direction = [
            3.0 * angle.cos() - angle.sin(),
            3.0 * angle.sin() + angle.cos(),
        ];
        let source_length = source_direction[0].hypot(source_direction[1]);
        let source_curve = document
            .add_curve(
                "source",
                CurveDefinition::Line {
                    start: source_points[0],
                    end: source_points[1],
                    branch_direction: [
                        source_direction[0] / source_length,
                        source_direction[1] / source_length,
                    ],
                },
            )
            .unwrap();
        let mirror = document
            .add_mirrored_curve("mirror", source_curve, CurveSpan::line(axis))
            .unwrap();
        for (index, (_, mirrored)) in mirror.point_pairs.iter().enumerate() {
            let expected = transformed(
                if index == 0 { [0.0, -1.0] } else { [3.0, -2.0] },
                scale,
                angle,
                translation,
            );
            let actual = document.point(*mirrored).unwrap().position;
            assert!((actual[0] - expected[0]).abs() / scale <= 1.0e-9);
            assert!((actual[1] - expected[1]).abs() / scale <= 1.0e-9);
        }
        let session = SketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        assert!(session.runtime().accepted_result().accepted());
    }
}

#[test]
fn offset_validation_rejects_invalid_inputs_and_the_antiparallel_root() {
    let (mut sketch, source, target) = line_pair(1.0, false, false);
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            sketch.add_supporting_line_offset(
                source,
                target,
                invalid,
                LineSide::Left,
                LineOffsetOrientation::Same,
                DimensionMode::Driving,
            ),
            Err(SketchError::InvalidDimensionValue(value)) if value.to_bits() == invalid.to_bits()
        ));
    }
    assert!(matches!(
        sketch.add_exact_translated_segment_offset(
            source,
            source,
            2.0,
            LineSide::Left,
            LineOffsetOrientation::Same,
            DimensionMode::Driving,
        ),
        Err(SketchError::RepeatedEntity)
    ));

    let (mut antiparallel, source, target) = line_pair(1.0, false, true);
    let dimension = antiparallel
        .add_supporting_line_offset(
            source,
            target,
            2.0,
            LineSide::Left,
            LineOffsetOrientation::Same,
            DimensionMode::Driving,
        )
        .unwrap();
    let target_value = antiparallel.segment(target).unwrap();
    let (target_start, target_end) = (target_value.start(), target_value.end());
    antiparallel.add_fixed_point(target_start).unwrap();
    antiparallel.add_fixed_point(target_end).unwrap();
    let result = solve(&mut antiparallel);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::LineOffsetBranchFlipped(dimension))
    );
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Invalid
    );
}

#[test]
fn offset_reference_measurement_and_target_edit_preserve_identity() {
    let (mut sketch, source, target_segment) = line_pair(3.0, true, true);
    let dimension = sketch
        .add_exact_translated_segment_offset(
            source,
            target_segment,
            6.0,
            LineSide::Right,
            LineOffsetOrientation::Reversed,
            DimensionMode::Reference,
        )
        .unwrap();
    sketch.set_dimension_target(dimension, 9.0).unwrap();
    assert_eq!(
        sketch.dimension(dimension).unwrap().mode(),
        DimensionMode::Reference
    );
    let result = solve(&mut sketch);
    assert_accepted(&result);
    let reference = result
        .reference_values
        .iter()
        .find(|value| value.dimension_id == dimension)
        .unwrap();
    assert!((reference.value - 6.0).abs() <= f64::EPSILON);
    let mapping = result
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Dimension(dimension))
        .unwrap();
    assert!(mapping.core_source_id.is_none());
    assert!(mapping.residual_ids.is_empty());
}

fn document_line_pair() -> (SketchDocument, CurveSpan, CurveSpan) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let a = document.add_point("A", [0.0, 0.0]).unwrap();
    let b = document.add_point("B", [4.0, 0.0]).unwrap();
    let c = document.add_point("C", [0.0, 2.0]).unwrap();
    let d = document.add_point("D", [4.0, 2.0]).unwrap();
    let source = document
        .add_curve(
            "source",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let target = document
        .add_curve(
            "target",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    (
        document,
        CurveSpan {
            curve: source,
            segment: 0,
        },
        CurveSpan {
            curve: target,
            segment: 0,
        },
    )
}

#[test]
fn current_document_round_trips_offsets_and_v1_v2_migrate_their_frozen_schemas() {
    let (mut legacy, source, _) = document_line_pair();
    let legacy_target = legacy
        .add_scalar(
            "legacy length",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    legacy
        .add_dimension(
            "legacy dimension",
            DocumentDimensionDefinition::CurveLength {
                curve: source,
                target: legacy_target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let canonical_v4 = legacy.to_canonical_json().unwrap();
    assert!(canonical_v4.starts_with("{\"version\":4,"));
    let mut prior: serde_json::Value = serde_json::from_str(&canonical_v4).unwrap();
    prior.as_object_mut().unwrap().remove("trim_views");
    prior["version"] = 1.into();
    let legacy_v1 = serde_json::to_string(&prior).unwrap();
    let migrated = SketchDocument::from_json(&legacy_v1).unwrap();
    assert_eq!(migrated.version(), 4);
    assert_eq!(migrated.to_canonical_json().unwrap(), canonical_v4);
    prior["version"] = 2.into();
    let legacy_v2 = serde_json::to_string(&prior).unwrap();
    assert_eq!(
        SketchDocument::from_json(&legacy_v2)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical_v4
    );

    let (mut document, source, target_segment) = document_line_pair();
    let offset = document
        .add_scalar("offset", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    document
        .add_dimension(
            "supporting offset",
            DocumentDimensionDefinition::SupportingLineOffset {
                source,
                target_segment,
                target: offset,
                side: DocumentLineSide::Left,
                orientation: DocumentLineOffsetOrientation::Same,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let json = document.to_canonical_json().unwrap();
    let round_trip = SketchDocument::from_json(&json).unwrap();
    assert_eq!(round_trip.to_canonical_json().unwrap(), json);
    let lowered = round_trip.lower().unwrap();
    assert!(matches!(
        lowered.sketch().dimensions().next().unwrap().1.kind(),
        DimensionKind::SupportingLineOffset {
            side: LineSide::Left,
            orientation: LineOffsetOrientation::Same,
            ..
        }
    ));

    let mut offset_claiming_v1: serde_json::Value = serde_json::from_str(&json).unwrap();
    offset_claiming_v1["version"] = 1.into();
    offset_claiming_v1
        .as_object_mut()
        .unwrap()
        .remove("trim_views");
    assert!(
        SketchDocument::from_json(&serde_json::to_string(&offset_claiming_v1).unwrap()).is_err()
    );
}

fn add_mirror_axis(document: &mut SketchDocument) -> CurveSpan {
    let start = document.add_point("axis start", [-10.0, 0.0]).unwrap();
    let end = document.add_point("axis end", [10.0, 0.0]).unwrap();
    let curve = document
        .add_curve(
            "axis",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    CurveSpan::line(curve)
}

fn add_points(
    document: &mut SketchDocument,
    points: &[[f64; 2]],
) -> Vec<geosolve_sketch::DesignPointId> {
    points
        .iter()
        .enumerate()
        .map(|(index, position)| {
            document
                .add_point(format!("control {}", index + 1), *position)
                .unwrap()
        })
        .collect()
}

#[test]
fn point_defined_mirrors_expand_to_ordinary_geometry_constraints_and_reflected_branches() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let axis = add_mirror_axis(&mut document);
    let definitions = [
        {
            let points = add_points(&mut document, &[[0.0, 1.0], [2.0, 3.0]]);
            CurveDefinition::Line {
                start: points[0],
                end: points[1],
                branch_direction: [2.0_f64.sqrt() / 2.0; 2],
            }
        },
        {
            let points = add_points(&mut document, &[[0.0, 1.0], [1.0, 2.0], [3.0, 2.0]]);
            CurveDefinition::Polyline {
                points,
                closed: false,
                branch_directions: vec![[2.0_f64.sqrt() / 2.0; 2], [1.0, 0.0]],
            }
        },
        {
            let controls: [_; 3] = add_points(&mut document, &[[0.0, 1.0], [1.0, 3.0], [3.0, 2.0]])
                .try_into()
                .unwrap();
            CurveDefinition::QuadraticBezier { controls }
        },
        {
            let controls: [_; 4] = add_points(
                &mut document,
                &[[0.0, 1.0], [1.0, 3.0], [2.0, 3.0], [4.0, 1.0]],
            )
            .try_into()
            .unwrap();
            CurveDefinition::CubicBezier { controls }
        },
        {
            let controls = add_points(
                &mut document,
                &[[0.0, 1.0], [1.0, 3.0], [3.0, 3.0], [4.0, 1.0]],
            );
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                span_ids: vec![4, 9],
                next_span_id: 10,
            }
        },
    ];

    let mut mirrors = Vec::new();
    for (index, definition) in definitions.into_iter().enumerate() {
        let source = document
            .add_curve(format!("source {}", index + 1), definition)
            .unwrap();
        let mirrored = document
            .add_mirrored_curve(&format!("mirror {}", index + 1), source, axis)
            .unwrap();
        assert_eq!(mirrored.source_curve, source);
        assert_eq!(
            mirrored.point_pairs.len(),
            mirrored.symmetry_constraints.len()
        );
        for (source_point, mirrored_point) in &mirrored.point_pairs {
            let source_position = document.point(*source_point).unwrap().position;
            let mirrored_position = document.point(*mirrored_point).unwrap().position;
            assert!((mirrored_position[0] - source_position[0]).abs() <= f64::EPSILON);
            assert!((mirrored_position[1] + source_position[1]).abs() <= f64::EPSILON);
        }
        mirrors.push(mirrored);
    }

    let CurveDefinition::Line {
        branch_direction, ..
    } = document
        .curve(mirrors[0].mirrored_curve)
        .unwrap()
        .definition
    else {
        panic!("mirrored line expected");
    };
    assert!(branch_direction[0] > 0.0 && branch_direction[1] < 0.0);
    let CurveDefinition::Polyline {
        branch_directions, ..
    } = &document
        .curve(mirrors[1].mirrored_curve)
        .unwrap()
        .definition
    else {
        panic!("mirrored polyline expected");
    };
    assert!(branch_directions[0][0] > 0.0 && branch_directions[0][1] < 0.0);

    let canonical = document.to_canonical_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&canonical)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    let lowered = document.lower().unwrap();
    let jacobians = lowered
        .sketch()
        .compile(SketchSolveRequest::default())
        .unwrap()
        .problem()
        .check_jacobians(1.0e-6)
        .unwrap();
    assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
}

#[test]
fn mirror_construction_command_preserves_ids_through_undo_and_redo() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let axis = add_mirror_axis(&mut document);
    let controls = add_points(&mut document, &[[0.0, 1.0], [2.0, 2.0]]);
    let source = document
        .add_curve(
            "source",
            CurveDefinition::Line {
                start: controls[0],
                end: controls[1],
                branch_direction: [2.0_f64.sqrt() / 2.0; 2],
            },
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateMirroredCurve {
                label: "mirror".into(),
                source_curve: source,
                axis,
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedMirroredCurve(ref ids)) = outcome.effect else {
        panic!("mirrored curve effect expected");
    };
    assert!(outcome.accepted());
    assert!(session.document().curve(ids.mirrored_curve).is_some());
    let accepted_json = session.export_json().unwrap();

    session.undo(session.revision()).unwrap();
    assert!(session.document().curve(ids.mirrored_curve).is_none());
    assert!(
        ids.point_pairs
            .iter()
            .all(|(_, mirrored)| session.document().point(*mirrored).is_none())
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), accepted_json);

    let edited = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: controls[0],
                position: [1.0, 3.0],
            },
        ))
        .unwrap();
    assert!(edited.accepted());
    let mirrored = ids
        .point_pairs
        .iter()
        .find_map(|(source, mirrored)| (*source == controls[0]).then_some(*mirrored))
        .unwrap();
    let accepted_source = session.document().point(controls[0]).unwrap().position;
    let mirrored_position = session.document().point(mirrored).unwrap().position;
    let CurveDefinition::Line {
        start: axis_start,
        end: axis_end,
        ..
    } = session.document().curve(axis.curve).unwrap().definition
    else {
        panic!("mirror axis line expected");
    };
    let axis_start = session.document().point(axis_start).unwrap().position;
    let axis_end = session.document().point(axis_end).unwrap().position;
    let axis_delta = [axis_end[0] - axis_start[0], axis_end[1] - axis_start[1]];
    let axis_length = axis_delta[0].hypot(axis_delta[1]);
    let axis_unit = [axis_delta[0] / axis_length, axis_delta[1] / axis_length];
    let source_offset = [
        accepted_source[0] - axis_start[0],
        accepted_source[1] - axis_start[1],
    ];
    let projection = source_offset[0] * axis_unit[0] + source_offset[1] * axis_unit[1];
    let expected_mirror = [
        axis_start[0] + 2.0 * projection * axis_unit[0] - source_offset[0],
        axis_start[1] + 2.0 * projection * axis_unit[1] - source_offset[1],
    ];
    assert!((mirrored_position[0] - expected_mirror[0]).abs() <= TOLERANCE);
    assert!((mirrored_position[1] - expected_mirror[1]).abs() <= TOLERANCE);
}

#[test]
fn oriented_angle_command_crosses_the_branch_cut_under_transform_and_rolls_back_conflict() {
    let epsilon = 1.0_f64.to_radians();
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let rotation = 0.73;
        let origin = [3.0 * scale, -2.0 * scale];
        let first_angle = rotation + std::f64::consts::PI - epsilon;
        let second_angle = rotation - std::f64::consts::PI + epsilon;
        let first_end = [
            origin[0] + 4.0 * scale * first_angle.cos(),
            origin[1] + 4.0 * scale * first_angle.sin(),
        ];
        let second_end = [
            origin[0] + 3.0 * scale * second_angle.cos(),
            origin[1] + 3.0 * scale * second_angle.sin(),
        ];
        let mut document = SketchDocument::new(scale).unwrap();
        let shared = document.add_point("shared", origin).unwrap();
        let first_point = document.add_point("first end", first_end).unwrap();
        let second_point = document.add_point("second end", second_end).unwrap();
        let first = document
            .add_curve(
                "first",
                CurveDefinition::Line {
                    start: shared,
                    end: first_point,
                    branch_direction: [first_angle.cos(), first_angle.sin()],
                },
            )
            .unwrap();
        let second = document
            .add_curve(
                "second",
                CurveDefinition::Line {
                    start: shared,
                    end: second_point,
                    branch_direction: [second_angle.cos(), second_angle.sin()],
                },
            )
            .unwrap();
        for (index, (point, target)) in [
            (shared, origin),
            (first_point, first_end),
            (second_point, second_end),
        ]
        .into_iter()
        .enumerate()
        {
            document
                .add_constraint(
                    format!("fixed {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let target = document
            .add_scalar(
                "angle target",
                2.0 * epsilon,
                ScalarUnit::Angle,
                ScalarDomain::Positive,
            )
            .unwrap();
        document
            .add_dimension(
                "branch-cut angle",
                DocumentDimensionDefinition::OrientedAngle {
                    first: CurveSpan::line(first),
                    second: CurveSpan::line(second),
                    target,
                    orientation: DocumentAngleOrientation::CounterClockwise,
                },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
        let canonical = document.to_canonical_json().unwrap();
        assert_eq!(
            SketchDocument::from_json(&canonical)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            canonical
        );
        let mut session = SketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        assert!(session.runtime().accepted_result().accepted());

        let wound_target = std::f64::consts::TAU + 2.0 * epsilon;
        let wound = session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: target,
                    value: wound_target,
                },
            ))
            .unwrap();
        assert!(wound.accepted(), "scale={scale:e}");
        let wound_json = session.export_json().unwrap();
        session.undo(session.revision()).unwrap();
        session.redo(session.revision()).unwrap();
        assert_eq!(session.export_json().unwrap(), wound_json);

        let before_conflict = session.export_json().unwrap();
        let history_len = session.history_len();
        let conflict = session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: target,
                    value: std::f64::consts::FRAC_PI_2,
                },
            ))
            .unwrap();
        assert!(!conflict.accepted());
        assert_eq!(session.export_json().unwrap(), before_conflict);
        assert_eq!(session.history_len(), history_len);
    }
}

#[test]
fn mirrored_bspline_refinement_is_atomic_associative_and_undoable() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let axis = add_mirror_axis(&mut document);
    let controls = add_points(
        &mut document,
        &[[0.0, 1.0], [1.0, 3.0], [3.0, 3.0], [4.0, 1.0]],
    );
    let source = document
        .add_curve(
            "source spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                span_ids: vec![2, 7],
                next_span_id: 8,
            },
        )
        .unwrap();
    let mirror = document
        .add_mirrored_curve("spline mirror", source, axis)
        .unwrap();
    let mut incomplete = document.clone();
    let missing_source = incomplete
        .constraint(mirror.symmetry_constraints[0])
        .unwrap()
        .source_id;
    incomplete
        .set_source_suppressed(missing_source, true)
        .unwrap();
    let before_failed_refinement = incomplete.clone();
    assert!(
        incomplete
            .insert_mirrored_bspline_knot(
                "invalid paired refinement",
                source,
                mirror.mirrored_curve,
                axis,
                0.5,
            )
            .is_err()
    );
    assert_eq!(incomplete, before_failed_refinement);

    let original_json = document.to_canonical_json().unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertMirroredBSplineKnot {
                label: "paired refinement".into(),
                source_curve: source,
                mirrored_curve: mirror.mirrored_curve,
                axis,
                parameter: 0.5,
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::InsertedMirroredBSplineKnot(ref insertion)) = outcome.effect
    else {
        panic!("mirrored insertion effect expected");
    };
    assert!(outcome.accepted());
    let source_new = session
        .document()
        .point(insertion.source.new_control)
        .unwrap()
        .position;
    let mirrored_new = session
        .document()
        .point(insertion.mirrored.new_control)
        .unwrap()
        .position;
    assert!((source_new[0] - mirrored_new[0]).abs() <= TOLERANCE);
    assert!((source_new[1] + mirrored_new[1]).abs() <= TOLERANCE);
    let refined_json = session.export_json().unwrap();
    assert_ne!(refined_json, original_json);

    session.undo(session.revision()).unwrap();
    assert!(
        session
            .document()
            .point(insertion.source.new_control)
            .is_none()
    );
    assert!(
        session
            .document()
            .point(insertion.mirrored.new_control)
            .is_none()
    );
    assert!(
        session
            .document()
            .constraint(insertion.symmetry_constraint)
            .is_none()
    );
    let CurveDefinition::BSpline { controls, .. } =
        &session.document().curve(source).unwrap().definition
    else {
        panic!("source B-spline expected");
    };
    assert_eq!(controls.len(), 4);
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), refined_json);
}
