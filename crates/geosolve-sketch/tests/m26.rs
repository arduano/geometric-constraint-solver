// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, SketchDocument,
    VisualProfileIssueKind, VisualProfileOptions, VisualProfileOrientation, VisualProfileStatus,
};

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let direction = [second[0] - first[0], second[1] - first[1]];
    let length = direction[0].hypot(direction[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [direction[0] / length, direction[1] / length],
            },
        )
        .unwrap()
}

fn add_polyline(
    document: &mut SketchDocument,
    label: &str,
    points: Vec<geosolve_sketch::DesignPointId>,
    closed: bool,
) -> geosolve_sketch::CurveId {
    let segment_count = points.len() - 1 + usize::from(closed);
    let branch_directions = (0..segment_count)
        .map(|index| {
            let start = document.point(points[index]).unwrap().position;
            let end = document
                .point(if index + 1 == points.len() {
                    points[0]
                } else {
                    points[index + 1]
                })
                .unwrap()
                .position;
            let direction = [end[0] - start[0], end[1] - start[1]];
            let length = direction[0].hypot(direction[1]);
            [direction[0] / length, direction[1] / length]
        })
        .collect();
    document
        .add_curve(
            label,
            CurveDefinition::Polyline {
                points,
                closed,
                branch_directions,
            },
        )
        .unwrap()
}

fn square(
    document: &mut SketchDocument,
    origin: [f64; 2],
    size: f64,
    label: &str,
) -> geosolve_sketch::CurveId {
    let points = [
        origin,
        [origin[0] + size, origin[1]],
        [origin[0] + size, origin[1] + size],
        [origin[0], origin[1] + size],
    ]
    .map(|position| document.add_point(label, position).unwrap());
    add_polyline(document, label, points.to_vec(), true)
}

fn transformed(point: [f64; 2], scale: f64, angle: f64, offset: [f64; 2]) -> [f64; 2] {
    let (sine, cosine) = angle.sin_cos();
    [
        scale * (cosine * point[0] - sine * point[1]) + offset[0],
        scale * (sine * point[0] + cosine * point[1]) + offset[1],
    ]
}

#[test]
fn exact_square_is_one_complete_counterclockwise_visual_face() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let curve = square(&mut document, [0.0, 0.0], 4.0, "square");
    let before = document.to_canonical_json().unwrap();
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.faces.len(), 1);
    assert_eq!(analysis.faces[0].contours.len(), 1);
    assert_eq!(
        analysis.faces[0].contours[0].orientation,
        VisualProfileOrientation::CounterClockwise
    );
    assert!((analysis.faces[0].visual_area - 16.0).abs() <= f64::EPSILON);
    assert_eq!(analysis.faces[0].contours[0].edges.len(), 4);
    assert!(
        analysis.faces[0].contours[0]
            .edges
            .iter()
            .all(|edge| edge.source_span.curve == curve)
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn open_chains_and_unwelded_coincident_coordinates_publish_no_face() {
    let mut open = SketchDocument::new(1.0).unwrap();
    let points = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]
        .map(|position| open.add_point("open", position).unwrap());
    add_polyline(&mut open, "open", points.to_vec(), false);
    let analysis = open.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.faces.is_empty());

    let mut unwelded = SketchDocument::new(1.0).unwrap();
    let coordinates = [
        ([0.0, 0.0], [1.0, 0.0]),
        ([1.0, 0.0], [1.0, 1.0]),
        ([1.0, 1.0], [0.0, 1.0]),
        ([0.0, 1.0], [0.0, 0.0]),
    ];
    for (index, (start, end)) in coordinates.into_iter().enumerate() {
        let start = unwelded.add_point("start", start).unwrap();
        let end = unwelded.add_point("end", end).unwrap();
        add_line(&mut unwelded, &format!("edge {index}"), start, end);
    }
    let analysis = unwelded.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.faces.is_empty());
}

#[test]
fn active_coincidence_constraints_weld_independent_line_endpoints() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let coordinates = [
        ([0.0, 0.0], [1.0, 0.0]),
        ([1.0, 0.0], [1.0, 1.0]),
        ([1.0, 1.0], [0.0, 1.0]),
        ([0.0, 1.0], [0.0, 0.0]),
    ];
    let mut endpoints = Vec::new();
    for (index, (start, end)) in coordinates.into_iter().enumerate() {
        let start = document.add_point("start", start).unwrap();
        let end = document.add_point("end", end).unwrap();
        add_line(&mut document, &format!("edge {index}"), start, end);
        endpoints.push((start, end));
    }
    for index in 0..4 {
        document
            .add_constraint(
                format!("corner {index}"),
                DocumentConstraintDefinition::Coincident {
                    first: endpoints[index].1,
                    second: endpoints[(index + 1) % 4].0,
                },
            )
            .unwrap();
    }
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert_eq!(analysis.faces.len(), 1);
    assert!((analysis.faces[0].visual_area - 1.0).abs() <= f64::EPSILON);

    document
        .set_point_position(endpoints[1].0, [1.25, 0.0])
        .unwrap();
    let inconsistent = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(inconsistent.status, VisualProfileStatus::Skipped);
    assert!(inconsistent.faces.is_empty());
    assert!(matches!(
        inconsistent.issues[0].kind,
        VisualProfileIssueKind::InconsistentCoincidence { first, second }
            if first == endpoints[0].1 && second == endpoints[1].0
    ));
}

#[test]
fn diagonal_crossing_t_junction_and_bow_tie_split_ephemerally() {
    let mut diagonal = SketchDocument::new(2.0).unwrap();
    let square_points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        .map(|position| diagonal.add_point("square", position).unwrap());
    add_polyline(&mut diagonal, "square", square_points.to_vec(), true);
    add_line(
        &mut diagonal,
        "diagonal",
        square_points[0],
        square_points[2],
    );
    let analysis = diagonal.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.faces.len(), 2, "{analysis:#?}");
    assert!(
        analysis
            .faces
            .iter()
            .all(|face| (face.visual_area - 2.0).abs() <= f64::EPSILON)
    );

    let mut tee = SketchDocument::new(2.0).unwrap();
    square(&mut tee, [0.0, 0.0], 2.0, "square");
    let bottom = tee.add_point("bottom tee", [1.0, 0.0]).unwrap();
    let top = tee.add_point("top tee", [1.0, 2.0]).unwrap();
    add_line(&mut tee, "splitter", bottom, top);
    let analysis = tee.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.faces.len(), 2);
    assert!(
        analysis
            .faces
            .iter()
            .all(|face| (face.visual_area - 2.0).abs() <= f64::EPSILON)
    );
    assert!(
        analysis
            .faces
            .iter()
            .flat_map(|face| &face.contours)
            .flat_map(|contour| &contour.edges)
            .any(|edge| {
                let parameters = edge.source_parameters.map(f64::to_bits);
                parameters == [0.0_f64.to_bits(), 0.5_f64.to_bits()]
                    || parameters == [0.5_f64.to_bits(), 1.0_f64.to_bits()]
            })
    );

    let mut bow_tie = SketchDocument::new(2.0).unwrap();
    let points = [[-1.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [1.0, -1.0]]
        .map(|position| bow_tie.add_point("bow tie", position).unwrap());
    add_polyline(&mut bow_tie, "bow tie", points.to_vec(), true);
    let analysis = bow_tie.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.faces.len(), 2);
    assert!(
        analysis
            .faces
            .iter()
            .all(|face| (face.visual_area - 1.0).abs() <= f64::EPSILON)
    );
}

#[test]
fn overlaps_skip_only_the_affected_component_and_budgets_are_typed() {
    let mut document = SketchDocument::new(2.0).unwrap();
    square(&mut document, [10.0, 0.0], 2.0, "clean square");
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [3.0, 0.0]).unwrap();
    let c = document.add_point("c", [1.0, 0.0]).unwrap();
    let d = document.add_point("d", [2.0, 0.0]).unwrap();
    let first = add_line(&mut document, "overlap first", a, b);
    let second = add_line(&mut document, "overlap second", c, d);
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Truncated);
    assert_eq!(analysis.faces.len(), 1);
    assert!(analysis.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::CollinearOverlap {
            first: CurveSpan { curve: first_curve, .. },
            second: CurveSpan { curve: second_curve, .. },
        } if first_curve == first && second_curve == second
    )));

    let skipped = document.analyze_visual_profiles(VisualProfileOptions {
        max_candidate_pairs: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(skipped.status, VisualProfileStatus::Skipped);
    assert!(skipped.faces.is_empty());
    assert!(matches!(
        skipped.issues[0].kind,
        VisualProfileIssueKind::CandidateBudgetExceeded { limit: 0, .. }
    ));
}

#[test]
fn nested_loops_publish_an_annulus_and_inner_face_without_overlap() {
    let mut document = SketchDocument::new(4.0).unwrap();
    square(&mut document, [0.0, 0.0], 4.0, "outer");
    square(&mut document, [1.0, 1.0], 2.0, "inner");
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert_eq!(analysis.faces.len(), 2);
    let mut areas = analysis
        .faces
        .iter()
        .map(|face| face.visual_area)
        .collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas, vec![4.0, 12.0]);
    let annulus = analysis
        .faces
        .iter()
        .find(|face| face.contours.len() == 2)
        .unwrap();
    assert_eq!(
        annulus.contours[1].orientation,
        VisualProfileOrientation::Clockwise
    );
    assert!((annulus.contours[1].signed_area + 4.0).abs() <= f64::EPSILON);

    let containment_limited = document.analyze_visual_profiles(VisualProfileOptions {
        max_containment_tests: 0,
        ..VisualProfileOptions::default()
    });
    assert_eq!(containment_limited.status, VisualProfileStatus::Skipped);
    assert!(containment_limited.faces.is_empty());
    assert!(matches!(
        containment_limited.issues[0].kind,
        VisualProfileIssueKind::ContainmentBudgetExceeded {
            required: 1,
            limit: 0
        }
    ));
}

#[test]
fn analysis_is_similarity_invariant_and_deterministic_after_json_round_trip() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).unwrap();
        let angle = 0.47;
        let offset = [7.0 * scale, -3.0 * scale];
        let points = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]
            .map(|point| transformed(point, scale, angle, offset))
            .map(|position| document.add_point("transformed square", position).unwrap());
        add_polyline(&mut document, "transformed square", points.to_vec(), true);
        add_line(&mut document, "transformed diagonal", points[0], points[2]);
        let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
        assert_eq!(
            analysis.status,
            VisualProfileStatus::Complete,
            "scale={scale:e}: {analysis:#?}"
        );
        assert_eq!(analysis.faces.len(), 2);
        let normalized_area = analysis
            .faces
            .iter()
            .map(|face| face.visual_area / (scale * scale))
            .sum::<f64>();
        assert!((normalized_area - 12.0).abs() <= 1.0e-8);

        let imported = SketchDocument::from_json(&document.to_canonical_json().unwrap()).unwrap();
        assert_eq!(
            imported.analyze_visual_profiles(VisualProfileOptions::default()),
            analysis
        );
    }
}

#[test]
fn certified_near_parallel_disjoint_components_and_limits_are_truthful() {
    let mut ambiguous = SketchDocument::new(1.0).unwrap();
    let a = ambiguous.add_point("a", [0.0, 0.0]).unwrap();
    let b = ambiguous.add_point("b", [1.0, 0.0]).unwrap();
    let c = ambiguous.add_point("c", [0.0, 1.0e-15]).unwrap();
    let d = ambiguous.add_point("d", [1.0, 2.0e-15]).unwrap();
    add_line(&mut ambiguous, "first", a, b);
    add_line(&mut ambiguous, "second", c, d);
    let analysis = ambiguous.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.faces.is_empty());
    assert!(analysis.issues.is_empty());

    let mut split = SketchDocument::new(2.0).unwrap();
    let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        .map(|position| split.add_point("square", position).unwrap());
    add_polyline(&mut split, "square", points.to_vec(), true);
    add_line(&mut split, "diagonal", points[0], points[2]);
    let fragment_limited = split.analyze_visual_profiles(VisualProfileOptions {
        max_fragments: 4,
        ..VisualProfileOptions::default()
    });
    assert_eq!(fragment_limited.status, VisualProfileStatus::Skipped);
    assert!(fragment_limited.faces.is_empty());
    assert!(matches!(
        fragment_limited.issues[0].kind,
        VisualProfileIssueKind::FragmentBudgetExceeded { limit: 4, .. }
    ));

    let face_limited = split.analyze_visual_profiles(VisualProfileOptions {
        max_faces: 1,
        ..VisualProfileOptions::default()
    });
    assert_eq!(face_limited.status, VisualProfileStatus::Truncated);
    assert_eq!(face_limited.faces.len(), 1);
    assert!(face_limited.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::FaceBudgetExceeded {
            required: 2,
            limit: 1
        }
    )));
}

#[test]
fn large_translation_and_bridge_connected_nesting_keep_truthful_faces() {
    let mut translated = SketchDocument::new(1.0).unwrap();
    let origin = [1.0e9, -1.0e9];
    let points = [
        origin,
        [origin[0] + 4.0, origin[1]],
        [origin[0] + 4.0, origin[1] + 3.0],
        [origin[0], origin[1] + 3.0],
    ]
    .map(|position| translated.add_point("translated", position).unwrap());
    add_polyline(&mut translated, "translated", points.to_vec(), true);
    add_line(&mut translated, "diagonal", points[0], points[2]);
    let analysis = translated.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.faces.len(), 2);
    assert!(
        (analysis
            .faces
            .iter()
            .map(|face| face.visual_area)
            .sum::<f64>()
            - 12.0)
            .abs()
            <= f64::EPSILON
    );

    let mut bridged = SketchDocument::new(4.0).unwrap();
    let outer = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]
        .map(|position| bridged.add_point("outer", position).unwrap());
    let inner = [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]]
        .map(|position| bridged.add_point("inner", position).unwrap());
    add_polyline(&mut bridged, "outer", outer.to_vec(), true);
    add_polyline(&mut bridged, "inner", inner.to_vec(), true);
    add_line(&mut bridged, "bridge", outer[0], inner[0]);
    let analysis = bridged.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.faces.len(), 2);
    let mut areas = analysis
        .faces
        .iter()
        .map(|face| face.visual_area)
        .collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas, vec![4.0, 12.0]);
}

#[test]
fn uncertainty_band_closed_walk_is_skipped_not_silently_complete() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let points = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0e-15], [0.0, 1.0e-15]]
        .map(|position| document.add_point("thin", position).unwrap());
    add_polyline(&mut document, "thin", points.to_vec(), true);
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Skipped);
    assert!(analysis.faces.is_empty());
    assert!(
        analysis
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::AreaUncertainty { .. }))
    );
}

#[test]
fn duplicate_t_split_is_charged_once_and_certified_divergence_is_not_overlap() {
    let mut duplicate = SketchDocument::new(2.0).unwrap();
    let left = duplicate.add_point("left", [0.0, 0.0]).unwrap();
    let junction = duplicate.add_point("junction", [1.0, 0.0]).unwrap();
    let right = duplicate.add_point("right", [2.0, 0.0]).unwrap();
    let up = duplicate.add_point("up", [1.0, 1.0]).unwrap();
    let down = duplicate.add_point("down", [1.0, -1.0]).unwrap();
    add_line(&mut duplicate, "horizontal", left, right);
    add_line(&mut duplicate, "up", junction, up);
    add_line(&mut duplicate, "down", junction, down);
    let analysis = duplicate.analyze_visual_profiles(VisualProfileOptions {
        max_fragments: 4,
        ..VisualProfileOptions::default()
    });
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert_eq!(analysis.fragment_count, 4);

    let mut divergence = SketchDocument::new(1.0).unwrap();
    let shared = divergence.add_point("shared", [0.0, 0.0]).unwrap();
    let straight = divergence.add_point("straight", [1.0, 0.0]).unwrap();
    let tilted = divergence.add_point("tilted", [1.0, 1.0e-15]).unwrap();
    add_line(&mut divergence, "straight", shared, straight);
    add_line(&mut divergence, "tilted", shared, tilted);
    let analysis = divergence.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert!(
        !analysis
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, VisualProfileIssueKind::CollinearOverlap { .. }))
    );
}
