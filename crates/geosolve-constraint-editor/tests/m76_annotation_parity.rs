// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
    ConstraintEditor, EditorScene, Modifiers, PointerInput, SceneAnnotationGeometry,
    SceneAnnotationKind, SceneConstraintGlyph, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    AlphaScenarioKind, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentAngleOrientation,
    DocumentCenterRef, DocumentConstraintDefinition, DocumentCurveCurvatureRelation,
    DocumentCurveDirectionRelation, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentDirectionSense, DocumentLineSupportRef, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    TangentOrientation, alpha_scenario,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

fn pointer(pointer_id: u64, position: ScreenPoint) -> PointerInput {
    PointerInput {
        pointer_id,
        position,
        modifiers: Modifiers::default(),
    }
}

fn distance(first: ScreenPoint, second: ScreenPoint) -> f64 {
    (first.x - second.x).hypot(first.y - second.y)
}

fn finite(point: ScreenPoint) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn accepted_scene(document: SketchDocument, request: DocumentSolveRequest) -> EditorScene {
    let session = RetainedSketchDocumentSession::new(document, request, SolverConfig::default())
        .expect("accepted annotation session");
    let accepted = session.accepted_state().expect("accepted annotation state");
    let mut scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([1000.0, 700.0], [100.0, 0.0], 3.0).expect("annotation viewport"),
        0.25,
    )
    .expect("accepted annotation scene");
    assert!(scene.update_annotation_values(accepted));
    scene
}

fn alpha_scene(kind: AlphaScenarioKind) -> EditorScene {
    let fixture = alpha_scenario(kind, 1.0).expect("alpha annotation fixture");
    accepted_scene(fixture.document, fixture.request)
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
) -> geosolve_sketch::CurveId {
    let start_id = document
        .add_point(format!("{label} start"), start)
        .expect("line start");
    let end_id = document
        .add_point(format!("{label} end"), end)
        .expect("line end");
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start: start_id,
                end: end_id,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .expect("line")
}

fn add_circle(
    document: &mut SketchDocument,
    label: &str,
    center: [f64; 2],
    radius: f64,
) -> geosolve_sketch::CurveId {
    let center = document
        .add_point(format!("{label} center"), center)
        .expect("circle center");
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("circle radius");
    document
        .add_curve(label, CurveDefinition::Circle { center, radius })
        .expect("circle")
}

#[allow(
    clippy::too_many_lines,
    reason = "five disconnected public constraint definitions complete the alpha sampler's missing glyph families"
)]
fn supplemental_constraint_scene() -> EditorScene {
    let mut document = SketchDocument::new(8.0).expect("supplemental document");

    let concentric = [
        add_circle(&mut document, "concentric outer", [0.0, 0.0], 2.0),
        add_circle(&mut document, "concentric inner", [0.0, 0.0], 1.0),
    ];
    document
        .add_constraint(
            "supplemental concentric",
            DocumentConstraintDefinition::Concentric {
                first: DocumentCenterRef {
                    curve: concentric[0],
                },
                second: DocumentCenterRef {
                    curve: concentric[1],
                },
            },
        )
        .expect("concentric constraint");

    let collinear = [
        add_line(&mut document, "collinear first", [6.0, 0.0], [9.0, 0.0]),
        add_line(&mut document, "collinear second", [10.0, 0.0], [13.0, 0.0]),
    ];
    document
        .add_constraint(
            "supplemental collinear",
            DocumentConstraintDefinition::Collinear {
                first: DocumentLineSupportRef {
                    span: CurveSpan::line(collinear[0]),
                    direction: DocumentDirectionSense::Forward,
                },
                second: DocumentLineSupportRef {
                    span: CurveSpan::line(collinear[1]),
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .expect("collinear constraint");

    let direction_circle = add_circle(&mut document, "direction circle", [18.0, -1.5], 1.5);
    let direction_line = add_line(&mut document, "direction line", [15.0, 0.0], [21.0, 0.0]);
    let direction_contact = document
        .add_curve_contact(
            "direction contact",
            CurveSpan::line(direction_circle),
            std::f64::consts::FRAC_PI_2,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("direction contact");
    document
        .add_constraint(
            "supplemental direction",
            DocumentConstraintDefinition::CurveDirection {
                line: CurveSpan::line(direction_line),
                curve_contact: direction_contact,
                relation: DocumentCurveDirectionRelation::Tangent {
                    orientation: TangentOrientation::Opposed,
                },
            },
        )
        .expect("direction constraint");

    let normal_circle = add_circle(&mut document, "normal circle", [27.0, 0.0], 1.5);
    let normal_line = add_line(&mut document, "normal line", [24.0, 0.0], [30.0, 0.0]);
    let normal_contact = document
        .add_curve_contact(
            "normal contact",
            CurveSpan::line(normal_circle),
            0.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("normal contact");
    document
        .add_constraint(
            "supplemental normal",
            DocumentConstraintDefinition::CurveDirection {
                line: CurveSpan::line(normal_line),
                curve_contact: normal_contact,
                relation: DocumentCurveDirectionRelation::Normal {
                    side: DocumentCurveNormalSide::Right,
                },
            },
        )
        .expect("normal constraint");

    let curvature = [
        add_circle(&mut document, "curvature first", [36.0, 0.0], 1.5),
        add_circle(&mut document, "curvature second", [40.0, 0.0], 1.5),
    ];
    let curvature_contacts = curvature.map(|curve| {
        document
            .add_curve_contact(
                "curvature contact",
                CurveSpan::line(curve),
                0.0,
                0,
                ContactNeighborhood::Interior,
                None,
            )
            .expect("curvature contact")
    });
    document
        .add_constraint(
            "supplemental equal curvature",
            DocumentConstraintDefinition::EqualCurvature {
                first_contact: curvature_contacts[0],
                second_contact: curvature_contacts[1],
                relation: DocumentCurveCurvatureRelation::Signed,
            },
        )
        .expect("equal-curvature constraint");

    let symmetry_axis = add_line(
        &mut document,
        "oblique symmetry axis",
        [46.0, -2.0],
        [50.0, 2.0],
    );
    let symmetric_first = document
        .add_point("oblique symmetric first", [50.0, 0.0])
        .expect("symmetric first");
    let symmetric_second = document
        .add_point("oblique symmetric second", [48.0, 2.0])
        .expect("symmetric second");
    document
        .add_constraint(
            "supplemental oblique symmetry",
            DocumentConstraintDefinition::SymmetricAboutLine {
                first: symmetric_first,
                second: symmetric_second,
                line: CurveSpan::line(symmetry_axis),
            },
        )
        .expect("oblique symmetry constraint");

    accepted_scene(document, DocumentSolveRequest::default())
}

fn annotation_fixture() -> (
    EditorScene,
    geosolve_sketch::DocumentId,
    CurveSpan,
    SelectionItem,
    ScreenPoint,
) {
    let mut document = SketchDocument::new(8.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [4.0, 0.0]).expect("end");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    let span = CurveSpan::line(line);
    let constraint = document
        .add_constraint(
            "custom alignment",
            DocumentConstraintDefinition::Horizontal { line: span },
        )
        .expect("horizontal constraint");
    let target = document
        .add_scalar(
            "distance target",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("target");
    document
        .add_dimension(
            "endpoint distance",
            DocumentDimensionDefinition::PointDistance {
                first: start,
                second: end,
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .expect("point-distance dimension");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("session");
    let accepted = session.accepted_state().expect("accepted");
    let document_id = accepted.document().id();
    let mut scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([800.0, 600.0], [2.0, 0.0], 50.0).expect("viewport"),
        0.5,
    )
    .expect("scene");
    assert!(scene.update_annotation_values(accepted));
    let item = SelectionItem::Constraint(constraint);
    let marker = scene
        .annotations
        .iter()
        .find(|annotation| annotation.item == item)
        .and_then(|annotation| match &annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => {
                markers.first().map(|marker| marker.anchor)
            }
            SceneAnnotationGeometry::RightAngle { .. }
            | SceneAnnotationGeometry::LinearDimension { .. }
            | SceneAnnotationGeometry::RadialDimension { .. }
            | SceneAnnotationGeometry::AngularDimension { .. }
            | SceneAnnotationGeometry::Label { .. } => None,
        })
        .expect("constraint marker");
    (scene, document_id, span, item, marker)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn annotation_geometry_and_layout_are_native_wasm_identical() {
    let (scene, document_id, _, _, _) = annotation_fixture();
    let dimension = scene
        .annotations
        .iter()
        .find(|annotation| annotation.kind == SceneAnnotationKind::PointDistance)
        .expect("point-distance annotation");
    assert_eq!(dimension.visible_text.as_deref(), Some("(4)"));
    assert!(
        dimension
            .accessible_label
            .contains("point-distance dimension")
    );
    assert_eq!(dimension.geometry.arrowheads().len(), 2);
    let bounds = dimension.label_bounds.expect("label bounds");
    let SceneAnnotationGeometry::LinearDimension {
        measured_first,
        measured_second,
        label_anchor,
        ..
    } = dimension.geometry
    else {
        panic!("point distance must use linear dimension geometry");
    };
    assert!(bounds.contains(label_anchor, 0.0));

    let layout = AnnotationLayoutState::from_entries([
        AnnotationLayoutEntry {
            key: AnnotationLayoutKey {
                document: document_id,
                source: dimension.source,
                item: dimension.item,
                kind: dimension.kind,
                marker_index: None,
            },
            placement: AnnotationPlacement::Linear {
                perpendicular_pixels: 44.0,
            },
        },
        AnnotationLayoutEntry {
            key: AnnotationLayoutKey {
                document: document_id,
                source: dimension.source,
                item: dimension.item,
                kind: dimension.kind,
                marker_index: None,
            },
            placement: AnnotationPlacement::Free {
                offset_pixels: [f64::NAN, 0.0],
            },
        },
    ]);
    assert_eq!(layout.entries().len(), 1);
    let mut moved = scene.clone();
    moved.apply_annotation_layout(&layout);
    let moved = moved
        .annotations
        .iter()
        .find(|annotation| annotation.item == dimension.item)
        .expect("moved dimension");
    let SceneAnnotationGeometry::LinearDimension {
        first,
        second,
        label_anchor,
        ..
    } = moved.geometry
    else {
        panic!("moved point distance must stay linear");
    };
    assert!((distance(first, measured_first) - 44.0).abs() <= 1.0e-12);
    assert!((distance(second, measured_second) - 44.0).abs() <= 1.0e-12);
    assert!((label_anchor.y - first.y).abs() <= 1.0e-12);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn annotation_drag_preview_cancel_and_commit_are_native_wasm_identical() {
    let (scene, _, span, item, origin) = annotation_fixture();
    let mut editor = ConstraintEditor::default();
    editor.set_selection([SelectionItem::Curve(span)]);
    editor.pointer_move(&scene, pointer(76, origin));
    editor.pointer_down(&scene, pointer(76, origin));
    let preview_position = ScreenPoint {
        x: origin.x + 12.0,
        y: origin.y - 7.0,
    };
    editor.pointer_move(&scene, pointer(76, preview_position));
    assert!(editor.annotation_layout().entries().is_empty());
    assert_eq!(editor.annotation_layout_for_scene().entries().len(), 1);
    editor.cancel();
    assert!(editor.annotation_layout_for_scene().entries().is_empty());

    editor.set_selection([SelectionItem::Curve(span)]);
    editor.pointer_move(&scene, pointer(77, origin));
    editor.pointer_down(&scene, pointer(77, origin));
    editor.pointer_move(&scene, pointer(77, preview_position));
    editor.pointer_up(&scene, scene.design_identity, pointer(77, preview_position));
    assert_eq!(editor.annotation_layout().entries().len(), 1);

    let mut moved = scene.clone();
    moved.apply_annotation_layout(editor.annotation_layout());
    let moved_marker = moved
        .annotations
        .iter()
        .find(|annotation| annotation.item == item)
        .and_then(|annotation| match &annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => {
                markers.first().map(|marker| marker.anchor)
            }
            SceneAnnotationGeometry::RightAngle { .. }
            | SceneAnnotationGeometry::LinearDimension { .. }
            | SceneAnnotationGeometry::RadialDimension { .. }
            | SceneAnnotationGeometry::AngularDimension { .. }
            | SceneAnnotationGeometry::Label { .. } => None,
        })
        .expect("moved marker");
    assert_eq!(moved_marker, preview_position);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn all_seven_dimension_families_publish_native_wasm_geometry() {
    use std::collections::BTreeSet;

    let scenes = [
        AlphaScenarioKind::Corpus,
        AlphaScenarioKind::A2,
        AlphaScenarioKind::A3,
        AlphaScenarioKind::SupportingOffset,
        AlphaScenarioKind::ExactTranslatedOffset,
    ]
    .map(alpha_scene);
    let annotations = scenes
        .iter()
        .flat_map(|scene| &scene.annotations)
        .filter(|annotation| !matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
        .collect::<Vec<_>>();
    let actual = annotations
        .iter()
        .map(|annotation| annotation.kind)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        SceneAnnotationKind::PointDistance,
        SceneAnnotationKind::CurveLength,
        SceneAnnotationKind::Radius,
        SceneAnnotationKind::Diameter,
        SceneAnnotationKind::OrientedAngle,
        SceneAnnotationKind::SupportingLineOffset,
        SceneAnnotationKind::ExactTranslatedSegmentOffset,
    ]);
    assert_eq!(actual, expected);

    for kind in expected {
        let annotation = annotations
            .iter()
            .copied()
            .find(|annotation| annotation.kind == kind)
            .expect("dimension family annotation");
        let geometry_matches = match kind {
            SceneAnnotationKind::PointDistance
            | SceneAnnotationKind::CurveLength
            | SceneAnnotationKind::SupportingLineOffset
            | SceneAnnotationKind::ExactTranslatedSegmentOffset => {
                matches!(
                    annotation.geometry,
                    SceneAnnotationGeometry::LinearDimension { .. }
                )
            }
            SceneAnnotationKind::Radius => matches!(
                annotation.geometry,
                SceneAnnotationGeometry::RadialDimension {
                    diameter: false,
                    ..
                }
            ),
            SceneAnnotationKind::Diameter => matches!(
                annotation.geometry,
                SceneAnnotationGeometry::RadialDimension { diameter: true, .. }
            ),
            SceneAnnotationKind::OrientedAngle => matches!(
                annotation.geometry,
                SceneAnnotationGeometry::AngularDimension { .. }
            ),
            SceneAnnotationKind::Constraint(_) => false,
        };
        assert!(geometry_matches, "wrong geometry for {kind:?}");
        assert!(annotation.visible_text.is_some());
        assert!(annotation.accessible_label.contains(" dimension;"));
        let bounds = annotation.label_bounds.expect("dimension label bounds");
        let center = ScreenPoint {
            x: (bounds.min.x + bounds.max.x) * 0.5,
            y: (bounds.min.y + bounds.max.y) * 0.5,
        };
        assert!(annotation.hit_test(center, 0.0));
        assert!(annotation.is_movable());
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "the native/WASM regression constructs and independently checks the complete shared-wedge paint/pick contract"
)]
fn shared_endpoint_angle_uses_the_finite_interior_wedge() {
    let mut document = SketchDocument::new(8.0).expect("angle document");
    let first_outer = document
        .add_point("first outer", [-4.0, 0.0])
        .expect("first outer point");
    let second_outer = document
        .add_point("second outer", [-3.0, 4.0])
        .expect("second outer point");
    let shared = document
        .add_point("shared vertex", [0.0, 0.0])
        .expect("shared vertex");
    let first = document
        .add_curve(
            "first ray",
            CurveDefinition::Line {
                start: first_outer,
                end: shared,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = document
        .add_curve(
            "second ray",
            CurveDefinition::Line {
                start: second_outer,
                end: shared,
                branch_direction: [0.6, -0.8],
            },
        )
        .expect("second line");
    let target = document
        .add_scalar(
            "clockwise acute angle",
            (4.0_f64).atan2(3.0),
            ScalarUnit::Angle,
            ScalarDomain::Positive,
        )
        .expect("angle target");
    let dimension = document
        .add_dimension(
            "shared endpoint angle",
            DocumentDimensionDefinition::OrientedAngle {
                first: CurveSpan::line(first),
                second: CurveSpan::line(second),
                target,
                orientation: DocumentAngleOrientation::Clockwise,
            },
            DocumentDimensionMode::Driving,
        )
        .expect("angle dimension");

    let scene = accepted_scene(document, DocumentSolveRequest::default());
    let annotation = scene
        .annotations
        .iter()
        .find(|annotation| annotation.item == SelectionItem::Dimension(dimension))
        .expect("shared endpoint annotation");
    let SceneAnnotationGeometry::AngularDimension {
        vertex,
        first_ray,
        second_ray,
        radius,
        label_anchor,
        ..
    } = annotation.geometry
    else {
        panic!("shared endpoint angle must publish angular geometry");
    };
    let shared_screen = scene
        .points
        .iter()
        .find(|point| point.id == shared)
        .expect("shared scene point")
        .screen_position;
    let first_outer_screen = scene
        .points
        .iter()
        .find(|point| point.id == first_outer)
        .expect("first outer scene point")
        .screen_position;
    let second_outer_screen = scene
        .points
        .iter()
        .find(|point| point.id == second_outer)
        .expect("second outer scene point")
        .screen_position;
    assert!(distance(vertex, shared_screen) <= 1.0e-9);

    let normalized = |from: ScreenPoint, to: ScreenPoint| {
        let delta = [to.x - from.x, to.y - from.y];
        let length = delta[0].hypot(delta[1]);
        [delta[0] / length, delta[1] / length]
    };
    let dot = |first: [f64; 2], second: [f64; 2]| first[0].mul_add(second[0], first[1] * second[1]);
    let first_finite_ray = normalized(vertex, first_outer_screen);
    let second_finite_ray = normalized(vertex, second_outer_screen);
    assert!(dot(normalized(vertex, first_ray), first_finite_ray) >= 1.0 - 1.0e-12);
    assert!(dot(normalized(vertex, second_ray), second_finite_ray) >= 1.0 - 1.0e-12);

    let interior_bisector = normalized(
        vertex,
        ScreenPoint {
            x: vertex.x + first_finite_ray[0] + second_finite_ray[0],
            y: vertex.y + first_finite_ray[1] + second_finite_ray[1],
        },
    );
    let label_direction = normalized(vertex, label_anchor);
    assert!(dot(label_direction, interior_bisector) >= 1.0 - 1.0e-12);
    assert!(dot(label_direction, first_finite_ray) > 0.0);
    assert!(dot(label_direction, second_finite_ray) > 0.0);

    let interior_arc = ScreenPoint {
        x: vertex.x + interior_bisector[0] * radius,
        y: vertex.y + interior_bisector[1] * radius,
    };
    let opposite_arc = ScreenPoint {
        x: vertex.x - interior_bisector[0] * radius,
        y: vertex.y - interior_bisector[1] * radius,
    };
    assert!(annotation.hit_test(interior_arc, 1.0e-9));
    assert!(!annotation.hit_test(opposite_arc, 1.0e-9));
    assert_eq!(annotation.visible_text.as_deref(), Some("53.13°"));
    assert_eq!(annotation.geometry.arrowheads().len(), 2);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive native/WASM loop validates the closed twenty-glyph annotation contract"
)]
fn all_twenty_constraint_glyph_families_publish_native_wasm_geometry() {
    use std::collections::BTreeSet;

    let scenes = [
        alpha_scene(AlphaScenarioKind::Corpus),
        alpha_scene(AlphaScenarioKind::NurbsDifferential),
        alpha_scene(AlphaScenarioKind::M28TrimmedFillet),
        supplemental_constraint_scene(),
    ];
    let annotations = scenes
        .iter()
        .flat_map(|scene| &scene.annotations)
        .filter(|annotation| matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
        .collect::<Vec<_>>();
    let actual = annotations
        .iter()
        .filter_map(|annotation| match annotation.kind {
            SceneAnnotationKind::Constraint(glyph) => Some(glyph),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        SceneConstraintGlyph::Fixed,
        SceneConstraintGlyph::Coincident,
        SceneConstraintGlyph::Horizontal,
        SceneConstraintGlyph::Vertical,
        SceneConstraintGlyph::PointOnCurve,
        SceneConstraintGlyph::Parallel,
        SceneConstraintGlyph::Perpendicular,
        SceneConstraintGlyph::Concentric,
        SceneConstraintGlyph::Collinear,
        SceneConstraintGlyph::EqualLength,
        SceneConstraintGlyph::EqualRadius,
        SceneConstraintGlyph::Midpoint,
        SceneConstraintGlyph::Symmetry,
        SceneConstraintGlyph::Contact,
        SceneConstraintGlyph::Tangency,
        SceneConstraintGlyph::Direction,
        SceneConstraintGlyph::Normal,
        SceneConstraintGlyph::EqualCurvature,
        SceneConstraintGlyph::Continuity,
        SceneConstraintGlyph::Fillet,
    ]);
    assert_eq!(actual, expected);

    for glyph in expected {
        let annotation = annotations
            .iter()
            .copied()
            .filter(|annotation| {
                annotation.kind == SceneAnnotationKind::Constraint(glyph) && !annotation.suppressed
            })
            .find(|annotation| {
                !matches!(
                    glyph,
                    SceneConstraintGlyph::Symmetry
                        | SceneConstraintGlyph::Direction
                        | SceneConstraintGlyph::Normal
                ) || matches!(
                    &annotation.geometry,
                    SceneAnnotationGeometry::Glyph { markers }
                        if markers.iter().any(|marker| marker.rotation_radians.abs() > 1.0e-6)
                )
            })
            .expect("unsuppressed constraint glyph annotation");
        assert!(
            annotation
                .accessible_label
                .ends_with(&format!("{} constraint", constraint_family_name(glyph))),
            "{}",
            annotation.accessible_label,
        );
        match &annotation.geometry {
            SceneAnnotationGeometry::Glyph { markers } => {
                assert!(!markers.is_empty(), "{glyph:?} has no marker");
                assert!(markers.iter().all(|marker| {
                    finite(marker.anchor)
                        && marker.leader_from.is_none_or(finite)
                        && marker.rotation_radians.is_finite()
                        && annotation.hit_test(marker.anchor, 0.0)
                }));
                if matches!(
                    glyph,
                    SceneConstraintGlyph::Symmetry
                        | SceneConstraintGlyph::Direction
                        | SceneConstraintGlyph::Normal
                ) {
                    assert!(
                        markers
                            .iter()
                            .any(|marker| marker.rotation_radians.abs() > 1.0e-6),
                        "{glyph:?} must expose a non-trivial local-frame rotation",
                    );
                }
                assert!(annotation.is_movable());
            }
            SceneAnnotationGeometry::RightAngle { corner, .. } => {
                assert_eq!(glyph, SceneConstraintGlyph::Perpendicular);
                assert!(finite(*corner));
                assert!(annotation.hit_test(*corner, 0.0));
                assert!(!annotation.is_movable());
            }
            SceneAnnotationGeometry::LinearDimension { .. }
            | SceneAnnotationGeometry::RadialDimension { .. }
            | SceneAnnotationGeometry::AngularDimension { .. }
            | SceneAnnotationGeometry::Label { .. } => {
                panic!("constraint {glyph:?} published dimension geometry")
            }
        }
    }
}

const fn constraint_family_name(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => "fixed",
        SceneConstraintGlyph::Coincident => "coincident",
        SceneConstraintGlyph::Horizontal => "horizontal",
        SceneConstraintGlyph::Vertical => "vertical",
        SceneConstraintGlyph::PointOnCurve => "point-on-curve",
        SceneConstraintGlyph::Parallel => "parallel",
        SceneConstraintGlyph::Perpendicular => "perpendicular",
        SceneConstraintGlyph::Concentric => "concentric",
        SceneConstraintGlyph::Collinear => "collinear",
        SceneConstraintGlyph::EqualLength => "equal-length",
        SceneConstraintGlyph::EqualRadius => "equal-radius",
        SceneConstraintGlyph::Midpoint => "midpoint",
        SceneConstraintGlyph::Symmetry => "symmetry",
        SceneConstraintGlyph::Contact => "contact",
        SceneConstraintGlyph::Tangency => "tangency",
        SceneConstraintGlyph::Direction => "direction",
        SceneConstraintGlyph::Normal => "normal",
        SceneConstraintGlyph::EqualCurvature => "equal-curvature",
        SceneConstraintGlyph::Continuity => "continuity",
        SceneConstraintGlyph::Fillet => "fillet",
    }
}
