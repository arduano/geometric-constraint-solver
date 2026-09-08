// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use crate::{SceneAnnotationGeometry, SceneAnnotationVisibility, ScreenPoint};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SolverConfig,
};

fn fixture(count: usize) -> (RetainedSketchDocumentSession, Vec<SelectionItem>) {
    let mut document = SketchDocument::new(100.0).expect("document");
    let mut curves = Vec::new();
    for index in 0..count {
        let y = f64::from(u32::try_from(index).expect("small fixture")) * 1.25;
        let first = document
            .add_point(format!("a{index}"), [-10.0, y])
            .expect("point");
        let second = document
            .add_point(format!("b{index}"), [10.0, y])
            .expect("point");
        let curve = document
            .add_curve(
                format!("line{index}"),
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        curves.push(SelectionItem::Curve(CurveSpan::line(curve)));
        let target = document
            .add_scalar(
                format!("length{index}"),
                20.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("scalar");
        document
            .add_dimension(
                format!("width{index}"),
                DocumentDimensionDefinition::PointDistance {
                    first,
                    second,
                    target,
                },
                DocumentDimensionMode::Driving,
            )
            .expect("dimension");
    }
    (
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session"),
        curves,
    )
}

fn scene(session: &RetainedSketchDocumentSession, zoom: f64) -> EditorScene {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted");
    let mut scene = EditorScene::from_accepted_for_design(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        session.design_document(),
        Viewport::new([1000.0, 700.0], [0.0, 4.0], zoom).expect("viewport"),
        0.5,
    )
    .expect("scene");
    assert!(scene.update_annotation_values(accepted));
    scene
}

fn anchor(annotation: &SceneAnnotation) -> ScreenPoint {
    match annotation.geometry {
        SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
        | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
        | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => label_anchor,
        SceneAnnotationGeometry::Label { anchor, .. } => anchor,
        SceneAnnotationGeometry::Glyph { .. } | SceneAnnotationGeometry::RightAngle { .. } => {
            panic!("dimension")
        }
    }
}

#[test]
fn m97_legacy_cold_rebuild_reproduces_dimension_jump_after_zoom() {
    let (session, _) = fixture(12);
    let mut retained = scene(&session, 18.0);
    let cold = scene(&session, 31.0);
    retained.reproject_viewport(cold.viewport).expect("zoom");
    let jumps: Vec<_> = retained
        .annotations
        .iter()
        .zip(&cold.annotations)
        .map(|(retained, cold)| anchor(retained).distance(anchor(cold)))
        .filter(|distance| *distance > 1.0)
        .collect();
    assert!(
        !jumps.is_empty(),
        "legacy cold layout must independently reproduce the reported reset"
    );
    eprintln!(
        "M97 legacy reproduction: {} moved labels, largest {} CSS px",
        jumps.len(),
        jumps.iter().copied().fold(0.0, f64::max)
    );
}

#[test]
fn m97_dimension_metadata_keeps_exact_values_and_angle_edit_quadrants() {
    let (session, _) = fixture(1);
    let mut scene = scene(&session, 18.0);
    let row = DimensionPresentationState::default()
        .apply(
            &mut scene,
            &AnnotationLayoutState::default(),
            &DimensionPresentationContext::default(),
        )
        .remove(0);
    let target = row.target_metadata(&scene).unwrap();
    scene
        .accepted_document
        .set_scalar_value(target.scalar, 12.0009)
        .unwrap();
    let exact = row.target_metadata(&scene).unwrap();
    assert_eq!(exact.value.to_bits(), 12.0009_f64.to_bits());
    assert_eq!(exact.display_value.to_bits(), 12.0009_f64.to_bits());
    assert_eq!(
        exact.storage_value_for_display(12.0).unwrap().to_bits(),
        12.0_f64.to_bits()
    );
    for (old, expected) in [
        (30.0_f64, 40.0_f64),
        (150.0, 140.0),
        (210.0, 220.0),
        (330.0, 320.0),
        (510.0, 500.0),
    ] {
        let metadata = crate::DimensionTargetMetadata {
            value: old.to_radians(),
            unit: ScalarUnit::Angle,
            display_value: 30.0,
            display_unit: crate::DimensionTargetDisplayUnit::AcuteDegrees,
            ..target
        };
        assert!(
            (metadata
                .storage_value_for_display(40.0)
                .unwrap()
                .to_degrees()
                - expected)
                .abs()
                < 1e-10
        );
        assert!(metadata.storage_value_for_display(91.0).is_err());
    }
}

#[test]
fn m97_focused_overview_hidden_picking_and_endpoint_selection() {
    let (session, curves) = fixture(12);
    let mut scene = scene(&session, 18.0);
    let accepted_before = scene.accepted_document.clone();
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let manual = AnnotationLayoutState::default();
    let rows = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    assert_eq!(rows.len(), 12);
    assert!(rows.iter().all(|row| !row.visible));
    for annotation in &scene.annotations {
        assert_eq!(annotation.visibility, SceneAnnotationVisibility::Hidden);
        assert!(!annotation.is_visible(
            &[annotation.item],
            Some(annotation.item),
            &[annotation.item]
        ));
        assert!(
            scene
                .annotation_hit_test(
                    anchor(annotation),
                    crate::PickTolerance::default(),
                    &[annotation.item],
                    Some(annotation.item),
                    &[annotation.item]
                )
                .is_none()
        );
    }
    let rows = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext {
            selection: curves,
            ..DimensionPresentationContext::default()
        },
    );
    assert_eq!(
        rows.iter().filter(|row| row.related).count(),
        12,
        "native curve endpoints relate point-distance dimensions"
    );
    assert!(rows.iter().filter(|row| row.visible).count() <= 6);
    assert!(rows.iter().any(|row| row.visible));
    assert_eq!(scene.accepted_document, accepted_before);
}

#[test]
fn m97_retained_slots_survive_zoom_then_cold_refresh_without_geometry_changes() {
    let (session, curves) = fixture(8);
    let mut scene = scene(&session, 18.0);
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let context = DimensionPresentationContext {
        selection: curves,
        ..DimensionPresentationContext::default()
    };
    let manual = AnnotationLayoutState::default();
    state.apply(&mut scene, &manual, &context);
    let next = Viewport::new([1000.0, 700.0], [1.0, 4.0], 31.0).expect("zoom");
    scene.reproject_viewport(next).expect("zoom");
    state.apply(&mut scene, &manual, &context);
    let retained: BTreeMap<_, _> = scene
        .annotations
        .iter()
        .map(|annotation| (annotation.item, anchor(annotation)))
        .collect();
    let accepted = session
        .accepted_state_for_current_input()
        .expect("accepted");
    let mut cold = EditorScene::from_accepted(
        accepted.identity().revision().get(),
        session.design_identity(),
        accepted.document(),
        next,
        0.5,
    )
    .expect("cold");
    assert!(cold.update_annotation_values(accepted));
    state.apply(&mut cold, &manual, &context);
    for annotation in &cold.annotations {
        if state
            .candidates
            .iter()
            .any(|key| key.item == annotation.item)
        {
            assert!(
                anchor(annotation).distance(retained[&annotation.item]) < 1e-8,
                "cold refresh must retain automatic placement"
            );
        }
    }
}

#[test]
fn m97_pin_focus_and_active_hidden_dimension_have_stable_exact_identities() {
    let (session, _) = fixture(8);
    let mut scene = scene(&session, 18.0);
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let manual = AnnotationLayoutState::default();
    let rows = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    state.pins = rows.iter().map(|row| row.key).collect();
    state.focus = Some(rows[7].key);
    let shown = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    assert_eq!(state.pins.len(), 4);
    assert!(shown[7].visible && shown[7].focused);
    assert_eq!(shown.iter().filter(|row| row.pinned).count(), 4);
    state.mode = DimensionDisplayMode::Hidden;
    let hidden = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    assert!(hidden.iter().all(|row| !row.visible));
    let active = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext {
            active: Some(rows[6].key.item),
            ..DimensionPresentationContext::default()
        },
    );
    assert_eq!(active.iter().filter(|row| row.visible).count(), 1);
    assert!(active[6].visible);
}

#[test]
fn m97_automatic_offsets_are_bounded_and_manual_placement_remains_separate() {
    let (session, curves) = fixture(12);
    let mut scene = scene(&session, 18.0);
    let raw = crate::annotations::build_annotations_unlaid(
        &scene.accepted_document,
        &scene.points,
        &scene.curves,
        scene.viewport,
    );
    let key = raw[0].layout_key(scene.accepted_document.id(), None);
    let manual = AnnotationLayoutState::from_entries([crate::AnnotationLayoutEntry {
        key,
        placement: crate::AnnotationPlacement::Linear {
            perpendicular_pixels: 200.0,
        },
    }]);
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext {
            selection: curves,
            ..DimensionPresentationContext::default()
        },
    );
    assert_eq!(manual.entries().len(), 1);
    for annotation in &scene.annotations {
        let base = raw
            .iter()
            .find(|base| base.item == annotation.item)
            .expect("raw");
        if annotation.item == key.item {
            assert!(anchor(annotation).distance(anchor(base)) > 96.0);
        } else {
            assert!(anchor(annotation).distance(anchor(base)) <= 96.0 + 1e-8);
        }
    }
    assert_eq!(
        scene.annotations[0].automatic_placement(None),
        manual.get(key)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one semantic fixture enumerates all eight accepted dimension families"
)]
fn m97_all_eight_dimension_families_and_reference_metadata_remain_available() {
    use geosolve_sketch::{
        DocumentAngleOrientation, DocumentDirectedProfileOffsetCurve,
        DocumentLineOffsetOrientation, DocumentLineSide, DocumentOffsetTraversal,
        DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetOperand,
        DocumentProfileOffsetTerminalPolicy,
    };
    let (session, curves) = fixture(2);
    let mut document = session.design_document().clone();
    let SelectionItem::Curve(first) = curves[0] else {
        unreachable!()
    };
    let SelectionItem::Curve(second) = curves[1] else {
        unreachable!()
    };
    let center = document
        .add_point("circle center", [25.0, 0.0])
        .expect("center");
    let radius = document
        .add_scalar(
            "circle radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("radius");
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .expect("circle");
    let length = document
        .add_scalar(
            "reference length",
            20.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("length");
    let angle = document
        .add_scalar(
            "reference angle",
            1.0,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .expect("angle");
    let offset = document
        .add_scalar("offset", 1.25, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("offset");
    let definitions = [
        DocumentDimensionDefinition::CurveLength {
            curve: first,
            target: length,
        },
        DocumentDimensionDefinition::Radius {
            curve: circle,
            target: radius,
        },
        DocumentDimensionDefinition::Diameter {
            curve: circle,
            target: length,
        },
        DocumentDimensionDefinition::OrientedAngle {
            first,
            second,
            target: angle,
            orientation: DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionDefinition::SupportingLineOffset {
            source: first,
            target_segment: second,
            target: offset,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
            source: first,
            target_segment: second,
            target: offset,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionDefinition::ProfileOffset {
            target: offset,
            operand: DocumentProfileOffsetOperand::OpenChain {
                side: DocumentLineSide::Left,
                chain: DocumentProfileOffsetChain {
                    edges: vec![DocumentProfileOffsetEdgePair {
                        source: DocumentDirectedProfileOffsetCurve {
                            curve: first,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                        target: DocumentDirectedProfileOffsetCurve {
                            curve: second,
                            traversal: DocumentOffsetTraversal::Forward,
                        },
                    }],
                    junctions: vec![],
                    start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                    end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                },
            },
        },
    ];
    for (index, mut definition) in definitions.into_iter().enumerate() {
        let target = match &mut definition {
            DocumentDimensionDefinition::PointDistance { target, .. }
            | DocumentDimensionDefinition::CurveLength { target, .. }
            | DocumentDimensionDefinition::Radius { target, .. }
            | DocumentDimensionDefinition::Diameter { target, .. }
            | DocumentDimensionDefinition::OrientedAngle { target, .. }
            | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
            | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. }
            | DocumentDimensionDefinition::ProfileOffset { target, .. } => target,
        };
        let scalar = document.scalar(*target).expect("template scalar").clone();
        *target = document
            .add_scalar(
                format!("target{index}"),
                scalar.value,
                scalar.unit,
                scalar.domain,
            )
            .expect("own scalar");
        let mode = if matches!(
            definition,
            DocumentDimensionDefinition::ProfileOffset { .. }
        ) {
            DocumentDimensionMode::Driving
        } else {
            DocumentDimensionMode::Reference
        };
        document
            .add_dimension(format!("reference{index}"), definition, mode)
            .expect("reference");
    }
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("all families session");
    let mut scene = scene(&session, 10.0);
    let mut state = DimensionPresentationState::default();
    let manual = AnnotationLayoutState::default();
    let rows = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    let kinds: BTreeSet<_> = rows.iter().map(|row| row.kind).collect();
    assert_eq!(kinds.len(), 8);
    assert_eq!(rows.len(), 9);
    assert!(rows.iter().filter(|row| row.reference).all(|row| {
        row.value_text
            .as_ref()
            .is_some_and(|text| text.starts_with('('))
    }));
    assert!(rows.iter().all(|row| row.visible));
    state.mode = DimensionDisplayMode::Hidden;
    let hidden = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext::default(),
    );
    assert_eq!(hidden.len(), rows.len());
    assert!(hidden.iter().all(|row| !row.visible));
}

#[test]
fn m97_navigation_freezes_membership_and_never_promotes_hidden_candidates() {
    let (session, curves) = fixture(12);
    let mut scene = scene(&session, 8.0);
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let manual = AnnotationLayoutState::default();
    let context = DimensionPresentationContext {
        selection: curves,
        ..DimensionPresentationContext::default()
    };
    let first = state.apply(&mut scene, &manual, &context);
    let initial_visible: BTreeSet<_> = first
        .iter()
        .filter(|row| row.visible)
        .map(|row| row.key)
        .collect();
    let candidate_keys = state.candidates.clone();
    assert!(
        candidate_keys.len() > initial_visible.len(),
        "crowded fixture must have a suppressed candidate"
    );
    scene
        .reproject_viewport(Viewport::new([1600.0, 1000.0], [0.0, 4.0], 50.0).expect("zoom"))
        .expect("navigation");
    let during = state.apply(
        &mut scene,
        &manual,
        &DimensionPresentationContext {
            navigation_active: true,
            ..context.clone()
        },
    );
    assert_eq!(state.candidates, candidate_keys);
    assert_eq!(
        during
            .iter()
            .filter(|row| row.visible)
            .map(|row| row.key)
            .collect::<BTreeSet<_>>(),
        initial_visible
    );
    let after = state.apply(&mut scene, &manual, &context);
    assert!(
        after
            .iter()
            .filter(|row| row.visible)
            .all(|row| initial_visible.contains(&row.key))
    );
    assert_eq!(state.candidates, candidate_keys);
}

#[test]
fn m97_hover_reveals_one_related_dimension_and_retains_label_transit() {
    let (session, curves) = fixture(3);
    let mut scene = scene(&session, 18.0);
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let rows = state.apply(
        &mut scene,
        &AnnotationLayoutState::default(),
        &DimensionPresentationContext {
            hovered: Some(curves[0]),
            ..DimensionPresentationContext::default()
        },
    );
    assert_eq!(rows.iter().filter(|row| row.visible).count(), 1);
    let annotation = scene
        .annotations
        .iter()
        .find(|annotation| annotation.visibility == SceneAnnotationVisibility::Always)
        .expect("preview");
    let origin = scene
        .curves
        .iter()
        .find(|curve| SelectionItem::Curve(curve.span) == curves[0])
        .expect("owner")
        .screen_polyline[0];
    assert!(scene.contextual_annotation_transit(
        anchor(annotation),
        crate::PickTolerance::default(),
        &[],
        curves[0],
        origin,
        &[]
    ));
    let navigation = DimensionPresentationContext {
        navigation_active: true,
        ..DimensionPresentationContext::default()
    };
    for _ in 0..2 {
        let rows = state.apply(&mut scene, &AnnotationLayoutState::default(), &navigation);
        assert!(
            rows.iter().all(|row| !row.visible),
            "pan must retire hover-only previews"
        );
    }
}

#[test]
fn m97_same_revision_geometry_preview_invalidates_only_affected_slots() {
    let (session, _) = fixture(2);
    let mut first = scene(&session, 18.0);
    let mut state = DimensionPresentationState::default();
    let context = DimensionPresentationContext::default();
    let manual = AnnotationLayoutState::default();
    state.apply(&mut first, &manual, &context);
    let initial = first.annotations.clone();
    let mut changed = first.accepted_document.clone();
    for operand in &initial[0].operands {
        if let SelectionItem::Point(id) = operand {
            let point = changed.point(*id).expect("point").position;
            changed
                .set_point_position(*id, [point[0] + 1.0, point[1]])
                .expect("move");
        }
    }
    let mut preview = EditorScene::from_accepted(
        first.accepted_revision,
        first.design_identity,
        &changed,
        first.viewport,
        0.5,
    )
    .expect("same revision accepted preview");
    state.apply(&mut preview, &manual, &context);
    assert!(anchor(&preview.annotations[0]).distance(anchor(&initial[0])) > 1.0);
    assert!(anchor(&preview.annotations[1]).distance(anchor(&initial[1])) < 1e-8);
    assert_eq!(preview.accepted_document, changed);
}

#[test]
fn m97_first_filtered_scene_restores_omitted_dimensions_without_a_design_edit() {
    let (session, curves) = fixture(2);
    let full = scene(&session, 18.0);
    let first = full.annotations[0].clone();
    let mut filtered = full.clone();
    filtered
        .hide_items(first.operands.iter().copied())
        .expect("hide first dimension's geometry");
    assert!(
        filtered
            .annotations
            .iter()
            .all(|annotation| annotation.item != first.item)
    );
    let accepted_before = filtered.accepted_document.clone();
    let manual = AnnotationLayoutState::default();
    let mut state = DimensionPresentationState {
        mode: DimensionDisplayMode::Focused,
        ..DimensionPresentationState::default()
    };
    let context = DimensionPresentationContext {
        selection: curves,
        ..DimensionPresentationContext::default()
    };
    let hidden = state.apply(&mut filtered, &manual, &context);
    assert_eq!(
        hidden.len(),
        2,
        "filtered geometry retains complete Inspector metadata"
    );
    let hidden_entry = hidden
        .iter()
        .find(|entry| entry.key.item == first.item)
        .expect("hidden entry");
    assert!(!hidden_entry.visible);
    state.focus = Some(hidden_entry.key);
    let mut restored = full;
    let visible = state.apply(&mut restored, &manual, &context);
    assert!(
        visible
            .iter()
            .any(|entry| entry.key.item == first.item && entry.visible),
        "restoring visibility must rebuild annotations missing from the first filtered scene"
    );
    let annotation = restored
        .annotations
        .iter()
        .find(|annotation| annotation.item == first.item)
        .expect("restored annotation");
    assert!(annotation.is_visible(&[], None, &[]));
    assert_eq!(
        restored
            .annotation_hit_test(
                anchor(annotation),
                crate::PickTolerance::default(),
                &[],
                None,
                &[]
            )
            .map(|hit| hit.item),
        Some(first.item)
    );
    assert_eq!(restored.accepted_document, accepted_before);
    assert_eq!(restored.accepted_revision, filtered.accepted_revision);
    assert_eq!(restored.design_identity, filtered.design_identity);
}
