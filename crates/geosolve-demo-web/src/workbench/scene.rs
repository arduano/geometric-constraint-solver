// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub(crate) use geosolve_sketch_render::*;

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        AdvancedConstructionKind, ComputedFeatureProblemMetadata, ConstraintEditor,
        ConstructionPreview, ConstructionPreviewGeometry, DraftInferenceBehavior,
        DraftInferenceEngine, DraftInferenceFrame, DraftInferenceInput, DraftInferencePolicy,
        DraftInferenceResolution, DraftInferenceSample, DraftInferenceSubject,
        DraftReferenceAnchor, DraftReferenceOrigin, EditorHoverState, EditorHoverTarget,
        EditorProblemScope, EditorScene, GeometryInteractionPolicy,
        OffsetAuthoringChainPresentation, OffsetAuthoringChainTerminal, OffsetDirectedSpan,
        OffsetEndpointRef, OffsetEndpointRole, OffsetTraversal, RetainedEditorCoordinator,
        SceneAnnotationGeometry, SceneAnnotationKind, SceneAnnotationLabelBounds,
        SceneAnnotationOccurrence, ScenePointRoleIncidence, ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
        DocumentAngleOrientation, DocumentArcSweep, DocumentCenterRef,
        DocumentConstraintDefinition, DocumentCoordinateAxis, DocumentCurveControlAvailability,
        DocumentCurveControlKind, DocumentCurveControlWithholdingReason,
        DocumentDimensionDefinition, DocumentDimensionMode, DocumentDirectionSense, DocumentEdit,
        DocumentLineSupportRef, DocumentObjectId, DocumentParameterId, DocumentParameterKind,
        DocumentParameterTarget, DocumentSolveRequest, GeometryRole,
        MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, ParameterBatch, ParameterBatchEntry, ParameterValue,
        PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
        SketchDesignIdentity, SketchDocument,
    };
    use geosolve_sketch_features::{
        ComputedFeatureCornerId, ComputedFeatureId, NativeCurveSpanSource,
    };

    use super::{
        CanvasCamera, CanvasDisplayOptions, OffsetCanvasPresentation, RetainedCameraTransform,
        RetainedFixedSizeTransform, adaptive_grid_spec, annotation_geometry, constraint_glyph,
        construction_geometry_markup, construction_markup, dimension_kind, render_curve_controls,
        retained_grid_presentation, svg_markup, svg_markup_with_computed_context,
        svg_markup_with_computed_context_action_stamp_and_display,
        svg_markup_with_computed_context_action_stamp_display_and_provisional,
        svg_markup_with_computed_context_and_action_stamp, svg_markup_with_context, viewport,
    };

    fn arc_geometry(large_arc: bool, sweep_radians: f64) -> ConstructionPreviewGeometry {
        ConstructionPreviewGeometry::CounterClockwiseArc {
            center: [0.0, 0.0],
            start: [1.0, 0.0],
            end: [0.0, 1.0],
            radius: 1.0,
            sweep_radians,
            large_arc,
        }
    }

    fn parameter_batch(
        parameter: DocumentParameterId,
        revision: u64,
        value: ParameterValue,
    ) -> ParameterBatch {
        ParameterBatch::new(revision, vec![ParameterBatchEntry { parameter, value }]).unwrap()
    }

    fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
        for axis in 0..2 {
            assert!(
                (actual[axis] - expected[axis]).abs() <= 1.0e-12,
                "axis {axis}: actual={} expected={}",
                actual[axis],
                expected[axis]
            );
        }
    }

    fn assert_offscreen_datum_clipping(session: &RetainedSketchDocumentSession) {
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted empty scene");
        let viewport =
            Viewport::new([1000.0, 700.0], [0.0, -7.02], 50.0).expect("offscreen viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("offscreen scene");
        let markup = svg_markup_with_computed_context_action_stamp_and_display(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions {
                grid_visible: false,
                ..CanvasDisplayOptions::default()
            },
            viewport,
        );
        assert!(!markup.contains("data-datum=\"origin\""));
        assert!(markup.contains("data-datum=\"x-axis\""));
        assert!(markup.contains("data-datum=\"x-axis\" data-protected=\"true\" aria-hidden=\"true\" style=\"display:none\""));
        assert!(markup.contains("data-datum=\"y-axis\""));
    }

    fn inference_fixture() -> (SketchDesignIdentity, u64, Viewport, [DesignPointId; 2]) {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("first");
        let second = document.add_point("second", [0.0, 0.0]).expect("second");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted inference fixture");
        (
            session.design_identity(),
            accepted.identity().revision().get(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 100.0).expect("viewport"),
            [first, second],
        )
    }

    fn point_anchor(point: DesignPointId) -> DraftReferenceAnchor {
        DraftReferenceAnchor::PersistentPoint {
            point,
            model_position: [0.0, 0.0],
            role_incidence: ScenePointRoleIncidence {
                profile: true,
                construction: false,
            },
        }
    }

    fn inference_frame(
        design_identity: SketchDesignIdentity,
        accepted_revision: u64,
        viewport: Viewport,
        raw_model_position: [f64; 2],
        anchors: Vec<DraftReferenceAnchor>,
    ) -> DraftInferenceFrame {
        DraftInferenceFrame {
            design_identity,
            accepted_revision,
            prepared_input: None,
            viewport,
            geometry_policy: GeometryInteractionPolicy::default(),
            sample: DraftInferenceSample {
                raw_screen_position: viewport.model_to_screen(raw_model_position),
                subject: DraftInferenceSubject::PointOperand,
                span_start: None,
            },
            anchors,
            semantic_centers: Vec::new(),
        }
    }

    fn inference_markup(resolution: &DraftInferenceResolution, viewport: Viewport) -> String {
        svg_markup_with_computed_context_and_action_stamp(
            None,
            None,
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            Some(resolution),
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            viewport,
        )
    }

    #[test]
    fn m76_dimension_markup_exposes_the_same_path_and_label_hit_envelopes() {
        let geometry = SceneAnnotationGeometry::LinearDimension {
            measured_first: ScreenPoint { x: 10.0, y: 20.0 },
            measured_second: ScreenPoint { x: 90.0, y: 20.0 },
            first: ScreenPoint { x: 10.0, y: 40.0 },
            second: ScreenPoint { x: 90.0, y: 40.0 },
            label_anchor: ScreenPoint { x: 50.0, y: 40.0 },
        };
        let mut markup = String::new();
        annotation_geometry(
            &mut markup,
            SceneAnnotationKind::PointDistance,
            &geometry,
            "80",
            Some(SceneAnnotationLabelBounds {
                min: ScreenPoint { x: 35.0, y: 30.0 },
                max: ScreenPoint { x: 65.0, y: 50.0 },
            }),
            None,
        );
        assert!(markup.contains("class=\"wb-annotation-path-hit\""));
        assert!(markup.contains(
            "class=\"wb-annotation-hit wb-annotation-label-hit\" x=\"25.000\" y=\"20.000\" width=\"50.000\" height=\"40.000\""
        ));
        assert!(markup.contains(
            "class=\"wb-annotation-hit wb-annotation-move-hit\" x=\"33.000\" y=\"28.000\" width=\"34.000\" height=\"24.000\""
        ));
        assert!(markup.contains("class=\"wb-dimension-label-mask\""));
    }

    #[test]
    fn camera_zoom_preserves_anchor_and_pan_uses_screen_space_direction() {
        let mut camera = CanvasCamera::default();
        let anchor = ScreenPoint { x: 750.0, y: 175.0 };
        let before = camera.viewport().screen_to_model(anchor);
        assert!(camera.zoom_about(anchor, 2.0));
        assert_point_close(camera.viewport().screen_to_model(anchor), before);

        let origin_center = camera.model_center();
        assert!(camera.pan_from(
            origin_center,
            ScreenPoint { x: 100.0, y: 100.0 },
            ScreenPoint { x: 200.0, y: 150.0 },
        ));
        assert_point_close(
            camera.model_center(),
            [
                origin_center[0] - 100.0 / camera.pixels_per_model_unit(),
                origin_center[1] + 50.0 / camera.pixels_per_model_unit(),
            ],
        );

        assert!(camera.center_origin());
        assert_point_close(camera.model_center(), [0.0, 0.0]);
        assert!(!camera.center_origin());
    }

    #[test]
    fn retained_camera_transform_matches_exact_viewport_mapping() {
        let exact = CanvasCamera::new([-4.0, 6.0], 37.5).expect("valid exact camera");
        let desired = CanvasCamera::new([3.25, -2.5], 92.0).expect("valid desired camera");
        let transform = RetainedCameraTransform::between(exact, desired)
            .expect("finite cameras have a retained affine mapping");
        for model in [[0.0, 0.0], [-12.5, 3.0], [24.0, -18.75]] {
            let retained = transform
                .map_screen_point(exact.viewport().model_to_screen(model))
                .expect("finite transformed screen point");
            let exact_screen = desired.viewport().model_to_screen(model);
            assert!((retained.x - exact_screen.x).abs() <= 1.0e-10);
            assert!((retained.y - exact_screen.y).abs() <= 1.0e-10);
        }

        assert!(CanvasCamera::new([f64::NAN, 0.0], 50.0).is_none());
        assert!(CanvasCamera::new([0.0, f64::INFINITY], 50.0).is_none());
        assert!(CanvasCamera::new([0.0, 0.0], f64::NAN).is_none());
        assert!(CanvasCamera::new([0.0, 0.0], 1.0).is_none());
        assert!(CanvasCamera::new([0.0, 0.0], 2_001.0).is_none());
    }

    #[test]
    fn retained_fixed_size_transform_cancels_parent_zoom_about_its_anchor() {
        let camera = RetainedCameraTransform {
            translate: [120.0, -40.0],
            scale: 2.5,
        };
        let fixed = RetainedFixedSizeTransform::for_camera(camera).expect("finite scale");
        let anchor = ScreenPoint { x: 200.0, y: 90.0 };
        let point = ScreenPoint { x: 205.0, y: 90.0 };
        let local = fixed.map_about(anchor, point);
        let mapped_anchor = camera.map_screen_point(anchor).expect("mapped anchor");
        let mapped_point = camera.map_screen_point(local).expect("mapped point");
        assert!((mapped_point.x - mapped_anchor.x - 5.0).abs() <= 1.0e-12);
        assert!((mapped_point.y - mapped_anchor.y).abs() <= 1.0e-12);
    }

    #[test]
    fn adaptive_grid_uses_origin_aligned_one_two_five_steps_and_is_visual_only() {
        for scale in [2.0, 7.5, 50.0, 175.0, 2_000.0] {
            let viewport =
                Viewport::new([1000.0, 700.0], [0.37, -1.25], scale).expect("grid viewport");
            let spec = adaptive_grid_spec(viewport).expect("adaptive grid");
            let decade = 10.0_f64.powf(spec.model_major_step.log10().floor());
            let mantissa = spec.model_major_step / decade;
            assert!(
                [1.0, 2.0, 5.0]
                    .into_iter()
                    .any(|expected| (mantissa - expected).abs() <= 1.0e-12),
                "{spec:?} must use a 1-2-5 model step"
            );
            assert!(spec.major_pixels >= 96.0 - 1.0e-9);
            assert!(spec.major_pixels <= 240.0 + 1.0e-9);
            let path = retained_grid_presentation(viewport)
                .expect("bounded retained grid")
                .minor_path;
            assert!(path.starts_with(&format!(
                "M{:.3} 0V700.000",
                spec.screen_origin.x.rem_euclid(spec.minor_pixels)
            )));
        }

        let viewport = viewport();
        let markup = svg_markup_with_computed_context_action_stamp_and_display(
            None,
            None,
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions::default(),
            viewport,
        );
        assert!(markup.contains("data-grid-kind=\"adaptive-1-2-5\""));
        assert!(markup.contains("class=\"wb-grid-minor\""));
        let grid = markup
            .split_once("<g class=\"wb-grid\"")
            .and_then(|(_, rest)| rest.split_once("</g>"))
            .map(|(grid, _)| grid)
            .expect("grid group");
        assert!(grid.contains("aria-hidden=\"true\""));
        assert!(!grid.contains("data-editor-item"));

        let hidden = svg_markup_with_computed_context_action_stamp_and_display(
            None,
            None,
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions {
                grid_visible: false,
                ..CanvasDisplayOptions::default()
            },
            viewport,
        );
        assert!(!hidden.contains("data-grid-kind"));
    }

    #[test]
    fn intrinsic_origin_stays_headless_while_only_axes_render_behind_native_geometry() {
        let document = SketchDocument::new(10.0).expect("document");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted empty scene");
        let viewport = viewport();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("scene");
        let markup = svg_markup_with_computed_context_action_stamp_and_display(
            Some(&scene),
            Some(accepted),
            &[],
            &[SelectionItem::Datum(geosolve_sketch::SketchDatum::Origin)],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions::default(),
            viewport,
        );
        assert_eq!(scene.datums.len(), 3);
        assert!(
            scene
                .datums
                .iter()
                .any(|datum| datum.datum == geosolve_sketch::SketchDatum::Origin)
        );
        for (key, label) in [("x-axis", "X"), ("y-axis", "Y")] {
            assert!(markup.contains(&format!("data-datum=\"{key}\"")));
            assert!(markup.contains(label));
        }
        assert!(!markup.contains("data-datum=\"origin\""));
        assert!(!markup.contains("wb-datum-origin"));
        assert!(!markup.contains("wb-datum-origin-ring"));
        assert!(!markup.contains("wb-datum-origin-cross"));
        assert!(!markup.contains(">Origin<"));
        assert_eq!(markup.matches("data-protected=\"true\"").count(), 2);
        assert!(markup.contains("M0.000 350.000L1000.000 350.000"));
        assert!(markup.contains("M500.000 700.000L500.000 0.000"));
        let references = markup.find("class=\"wb-reference-geometry\"").unwrap();
        let native = markup.find("class=\"wb-geometry\"").unwrap();
        assert!(
            references < native,
            "native geometry must paint over datums"
        );

        let mut hidden_policy = GeometryInteractionPolicy::default();
        hidden_policy.visibility.reference_geometry = false;
        let hidden = svg_markup_with_computed_context_action_stamp_and_display(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            hidden_policy,
            CanvasDisplayOptions::default(),
            viewport,
        );
        assert!(!hidden.contains("data-datum="));
        assert!(hidden.contains("data-grid-kind=\"adaptive-1-2-5\""));

        assert_offscreen_datum_clipping(&session);
    }

    #[test]
    fn headless_hover_target_marks_exactly_one_native_or_datum_geometry_item() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document.add_point("start", [-2.0, 1.0]).expect("point");
        let end = document.add_point("end", [2.0, 1.0]).expect("point");
        let curve = CurveSpan::line(
            document
                .add_curve(
                    "line",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("line"),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted scene");
        let viewport = viewport();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("scene");
        let render = |item| {
            svg_markup_with_computed_context_action_stamp_and_display(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                EditorHoverState {
                    target: Some(EditorHoverTarget::Geometry(item)),
                    context_owner: Some(item),
                },
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions {
                    grid_visible: false,
                    ..CanvasDisplayOptions::default()
                },
                viewport,
            )
        };

        let point_markup = render(SelectionItem::Point(start));
        assert_eq!(point_markup.matches(" geometry-hovered").count(), 1);
        assert!(point_markup.contains("class=\"wb-point geometry-hovered\" cx="));

        let curve_markup = render(SelectionItem::Curve(curve));
        assert_eq!(curve_markup.matches(" geometry-hovered").count(), 1);
        assert!(curve_markup.contains("class=\"wb-curve geometry-hovered\" d="));

        let datum_markup = render(SelectionItem::Datum(geosolve_sketch::SketchDatum::YAxis));
        assert_eq!(datum_markup.matches(" geometry-hovered").count(), 1);
        assert!(datum_markup.contains("wb-datum-y-axis geometry-hovered"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one renderer regression compares exact guide, grip, hover, paint-order and accessibility output"
    )]
    fn m77_curve_control_markup_uses_published_geometry_hover_and_accessibility() {
        let mut document = SketchDocument::new(4.0).unwrap();
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [4.0, 0.0]).unwrap();
        let weight = document
            .add_scalar(
                "weight",
                0.5,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
            )
            .unwrap();
        let curve = document
            .add_curve(
                "rational demo",
                CurveDefinition::RationalQuadraticConic {
                    start,
                    weighted_middle: [1.0, 1.5],
                    middle_weight: weight,
                    end,
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state_for_current_input().unwrap();
        let viewport = viewport();
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .unwrap();
        let mut editor = ConstraintEditor::default();
        editor.set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
        editor.populate_curve_controls(&mut scene).unwrap();
        let middle = scene
            .curve_controls
            .iter()
            .find(|control| control.id.kind == DocumentCurveControlKind::RationalMiddle)
            .cloned()
            .expect("middle control");
        let hover = EditorHoverState {
            target: Some(EditorHoverTarget::CurveControl {
                control: middle.id,
                owner: middle.owner,
            }),
            context_owner: Some(SelectionItem::Curve(middle.owner)),
        };
        let markup = svg_markup_with_computed_context_action_stamp_and_display(
            Some(&scene),
            Some(accepted),
            &[],
            &[SelectionItem::Curve(CurveSpan::line(curve))],
            &[],
            hover,
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions {
                grid_visible: false,
                ..CanvasDisplayOptions::default()
            },
            viewport,
        );
        assert_eq!(markup.matches("wb-curve-control hovered").count(), 1);
        assert!(markup.contains("data-control-role=\"rational-middle\""));
        assert!(markup.contains("aria-label=\"Middle control P1 — rational demo\""));
        assert!(markup.contains("<title>Middle control P1 — rational demo</title>"));
        assert!(markup.contains("class=\"wb-curve-control-tooltip\""));
        assert!(markup.contains("pointer-events=\"none\""));
        assert!(
            markup.find("class=\"wb-annotations\"").unwrap()
                < markup.find("class=\"wb-curve-control-cage\"").unwrap(),
            "direct handles must paint above curve annotations while guides remain below points",
        );
        assert!(!markup.contains("data-control-role=\"start-point\""));
        assert!(!markup.contains("data-control-role=\"end-point\""));
        for guide in &scene.curve_control_guides {
            assert!(markup.contains(&format!(
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\"",
                guide.screen_start.x, guide.screen_start.y, guide.screen_end.x, guide.screen_end.y,
            )));
        }
        let geosolve_constraint_editor::SceneCurveControlGripGeometry::Square {
            center,
            half_extent_pixels,
        } = middle.grip
        else {
            panic!("rational P1 must use the published square grip");
        };
        assert!(markup.contains(&format!(
            "x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"",
            center.x - half_extent_pixels,
            center.y - half_extent_pixels,
            half_extent_pixels * 2.0,
            half_extent_pixels * 2.0,
        )));

        let mut read_only = middle;
        read_only.availability = DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::HostParameterOwned,
        );
        scene.curve_controls = vec![read_only];
        let mut read_only_markup = String::new();
        render_curve_controls(&mut read_only_markup, &scene, EditorHoverState::default());
        assert!(read_only_markup.contains("class=\"wb-curve-control read-only\""));
        assert!(read_only_markup.contains("aria-disabled=\"true\""));
        assert!(read_only_markup.contains("read-only: value is owned by a host parameter"));
    }

    #[test]
    fn inference_markup_distinguishes_constraint_backed_and_tracking_only_guides() {
        let (design, accepted_revision, viewport, [point, _]) = inference_fixture();
        let anchor = point_anchor(point);
        let frame = inference_frame(
            design,
            accepted_revision,
            viewport,
            [0.0, 0.0],
            vec![anchor],
        );
        let resolved = DraftInferenceEngine::default()
            .resolve(&frame, DraftInferenceInput::default())
            .expect("resolved point inference");
        let resolved_markup = inference_markup(&resolved, viewport);
        assert!(resolved_markup.contains("data-inference-status=\"resolved\""));
        assert!(resolved_markup.contains("data-inference-classification=\"constraint-backed\""));
        assert!(resolved_markup.contains("data-inference-relation=\"point-identity\""));
        assert!(resolved_markup.contains("aria-label=\"Reuse existing point\""));

        let tracking_policy = DraftInferencePolicy {
            point_tracking: DraftInferenceBehavior::tracking_only(),
            ..DraftInferencePolicy::default()
        };
        let mut tracking_engine =
            DraftInferenceEngine::new(tracking_policy).expect("tracking-only display policy");
        tracking_engine
            .remember_reference(anchor)
            .expect("remember point reference");
        let mut tracking_frame =
            inference_frame(design, accepted_revision, viewport, [2.0, 0.0], Vec::new());
        tracking_frame.geometry_policy.visibility.reference_geometry = false;
        let tracking = tracking_engine
            .resolve(&tracking_frame, DraftInferenceInput::default())
            .expect("tracking-only inference");
        let tracking_markup = inference_markup(&tracking, viewport);
        assert!(tracking_markup.contains("data-inference-family=\"point-tracking\""));
        assert!(tracking_markup.contains("data-inference-classification=\"tracking-only\""));
        assert!(!tracking_markup.contains("data-inference-relation="));
    }

    #[test]
    fn inference_markup_presents_native_midpoint_axes_as_constraint_backed() {
        let (design, accepted_revision, viewport, _) = inference_fixture();
        let span = CurveSpan::line(CurveId(PersistentId::from_u128(71)));
        let midpoint = DraftReferenceAnchor::Midpoint {
            span,
            model_position: [0.0, 1.0],
            affine_direction: [1.0, 0.0],
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: DraftReferenceOrigin::Native,
        };
        let mut engine = DraftInferenceEngine::default();
        engine
            .remember_reference(midpoint)
            .expect("remember midpoint");
        let frame = inference_frame(design, accepted_revision, viewport, [3.0, 1.05], Vec::new());
        let resolved = engine
            .resolve(&frame, DraftInferenceInput::default())
            .expect("midpoint-axis inference");

        let markup = inference_markup(&resolved, viewport);
        assert!(markup.contains("data-inference-status=\"resolved\""));
        assert!(markup.contains("data-inference-classification=\"constraint-backed\""));
        assert!(markup.contains("data-inference-family=\"horizontal-point-to-midpoint\""));
        assert!(markup.contains("data-inference-relation=\"horizontal-point-to-midpoint\""));
        assert!(markup.contains("aria-label=\"Horizontal to midpoint\""));
    }

    #[test]
    fn circumference_inference_is_presented_as_circle_through_point() {
        let (design, accepted_revision, viewport, [point, _]) = inference_fixture();
        let mut frame = inference_frame(
            design,
            accepted_revision,
            viewport,
            [0.0, 0.0],
            vec![point_anchor(point)],
        );
        frame.sample.subject = DraftInferenceSubject::CircleCircumference;
        let resolved = DraftInferenceEngine::default()
            .resolve(&frame, DraftInferenceInput::default())
            .expect("resolved circle-through-point inference");

        let markup = inference_markup(&resolved, viewport);
        assert!(markup.contains("data-inference-status=\"resolved\""));
        assert!(markup.contains("data-inference-family=\"point-on-created-curve\""));
        assert!(markup.contains("data-inference-relation=\"point-on-created-curve\""));
        assert!(markup.contains("aria-label=\"Circle through point\""));
        assert!(!markup.contains("aria-label=\"Reuse existing point\""));
    }

    #[test]
    fn inference_markup_exposes_ambiguity_and_suppression_accessibly() {
        let (design, accepted_revision, viewport, points) = inference_fixture();
        let frame = inference_frame(
            design,
            accepted_revision,
            viewport,
            [0.0, 0.0],
            points.into_iter().map(point_anchor).collect(),
        );
        let ambiguous = DraftInferenceEngine::default()
            .resolve(&frame, DraftInferenceInput::default())
            .expect("ambiguous point inference");
        let ambiguous_markup = inference_markup(&ambiguous, viewport);
        assert!(ambiguous_markup.contains("data-inference-status=\"ambiguous\""));
        assert!(ambiguous_markup.contains("role=\"status\""));
        assert!(ambiguous_markup.contains("aria-label=\"Ambiguous auto-constraint"));

        let suppressed = DraftInferenceEngine::default()
            .resolve(
                &frame,
                DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
            )
            .expect("suppressed inference");
        let suppressed_markup = inference_markup(&suppressed, viewport);
        assert!(suppressed_markup.contains("data-inference-status=\"suppressed\""));
        assert!(
            suppressed_markup.contains("aria-label=\"Auto-constraints suppressed by Ctrl/Cmd\"")
        );
        assert!(!suppressed_markup.contains("wb-inference-glyph"));

        let mut resource_policy = DraftInferencePolicy::default();
        resource_policy.limits.max_candidates = 1;
        let resource_limited = DraftInferenceEngine::new(resource_policy)
            .expect("bounded engine")
            .resolve(&frame, DraftInferenceInput::default())
            .expect("resource-limited inference");
        let resource_markup = inference_markup(&resource_limited, viewport);
        assert!(resource_markup.contains("data-inference-status=\"resource-limited\""));
        assert!(resource_markup.contains(
            "aria-label=\"Auto-constraints unavailable: inference resource limit reached\""
        ));
    }

    #[test]
    fn camera_fit_contains_scene_geometry_with_margin() {
        let fixture = geosolve_sketch::alpha_scenario(
            geosolve_sketch::AlphaScenarioKind::MotionScissorTower,
            1.0,
        )
        .unwrap();
        let session = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let initial = viewport();
        let scene = EditorScene::from_accepted(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            initial,
            0.8,
        )
        .unwrap();
        let mut camera = CanvasCamera::default();
        assert!(camera.fit_scene(&scene));
        let fitted = camera.viewport();
        for point in scene.points {
            let point = fitted.model_to_screen(point.model_position);
            assert!((63.0..=937.0).contains(&point.x));
            assert!((63.0..=637.0).contains(&point.y));
        }
    }

    #[test]
    fn empty_fit_resets_to_the_canonical_origin_camera() {
        let document = SketchDocument::new(10.0).expect("document");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted empty document");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .expect("empty scene");
        let mut camera = CanvasCamera::new([15.0, -4.0], 137.0).expect("valid camera");
        assert!(!camera.fit_scene_or_reset(Some(&scene)));
        assert_eq!(camera, CanvasCamera::default());
        camera = CanvasCamera::new([1.0, 2.0], 137.0).expect("valid camera");
        assert!(!camera.fit_scene_or_reset(None));
        assert_eq!(camera, CanvasCamera::default());
    }

    #[test]
    fn serializes_explicit_minor_and_major_counterclockwise_arc_flags() {
        let mut minor = String::new();
        construction_geometry_markup(
            &mut minor,
            &arc_geometry(false, std::f64::consts::FRAC_PI_2),
            viewport(),
        );
        let mut major = String::new();
        construction_geometry_markup(
            &mut major,
            &arc_geometry(true, 3.0 * std::f64::consts::FRAC_PI_2),
            viewport(),
        );
        assert!(minor.contains("A 50.000 50.000 0 0 0"));
        assert!(major.contains("A 50.000 50.000 0 1 0"));
    }

    #[test]
    fn m78_preview_renders_explicit_clockwise_sweep_and_multistage_guides() {
        let mut clockwise = String::new();
        construction_geometry_markup(
            &mut clockwise,
            &ConstructionPreviewGeometry::CircularArc {
                center: [0.0, 0.0],
                start: [1.0, 0.0],
                end: [0.0, 1.0],
                radius: 1.0,
                sweep_radians: 3.0 * std::f64::consts::FRAC_PI_2,
                large_arc: true,
                sweep: DocumentArcSweep::Clockwise,
            },
            viewport(),
        );
        assert!(clockwise.contains("A 50.000 50.000 0 1 1"));
        assert!(clockwise.contains("wb-draft-start"));
        assert!(clockwise.contains("wb-draft-end"));

        let guide = construction_markup(
            &ConstructionPreview::GuidePolyline {
                points: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 1.0]],
                closed: true,
            },
            viewport(),
        );
        assert_eq!(guide.matches("wb-draft-point").count(), 3);
        assert!(
            guide.contains(
                "M 500.000 350.000 L 600.000 350.000 L 600.000 300.000 L 500.000 350.000"
            )
        );
    }

    #[test]
    fn elliptical_arc_support_preview_renders_projection_without_a_fake_control_polygon() {
        let markup = construction_markup(
            &ConstructionPreview::EllipticalArcSupport {
                center: [0.0, 0.0],
                major_axis_point: [4.0, 0.0],
                support_points: vec![[4.0, 0.0], [0.0, 2.0], [-4.0, 0.0], [0.0, -2.0], [4.0, 0.0]],
                trim_start: Some([0.0, 2.0]),
            },
            viewport(),
        );
        assert!(markup.contains("class=\"wb-draft-ellipse-support\""));
        assert!(markup.contains("class=\"wb-draft-major-axis\""));
        assert!(markup.contains("wb-draft-center"));
        assert!(markup.contains("wb-draft-major-axis-point"));
        assert!(markup.contains("wb-draft-start"));
        assert!(!markup.contains("wb-draft-control-polygon"));
    }

    #[test]
    fn completed_elliptical_arc_preview_renders_spatial_roles_without_a_fake_control_polygon() {
        let mut markup = String::new();
        construction_geometry_markup(
            &mut markup,
            &ConstructionPreviewGeometry::AdvancedCurve {
                kind: AdvancedConstructionKind::EllipticalArc,
                control_points: vec![[0.0, 0.0], [4.0, 0.0], [0.0, 2.0], [-4.0, 0.0]],
                curve_points: vec![[0.0, 2.0], [-2.8, 1.4], [-4.0, 0.0]],
            },
            viewport(),
        );
        assert!(markup.contains("class=\"wb-draft-major-axis\""));
        assert!(markup.contains("wb-draft-center"));
        assert!(markup.contains("wb-draft-major-axis-point"));
        assert!(markup.contains("wb-draft-start"));
        assert!(markup.contains("wb-draft-end"));
        assert!(markup.contains("data-draft-kind=\"elliptical-arc\""));
        assert!(!markup.contains("wb-draft-control-polygon"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the complete relation and dimension presentation matrix is clearer in one test"
    )]
    fn m55_required_glyph_and_dimension_labels_cover_the_complete_action_surface() {
        let point = |value| DesignPointId(PersistentId::from_u128(value));
        let curve = |value| CurveId(PersistentId::from_u128(value));
        let contact = |value| ContactId(PersistentId::from_u128(value));
        let line = |value| CurveSpan::line(curve(value));
        let definitions = [
            DocumentConstraintDefinition::FixedPoint {
                point: point(1),
                target: [0.0, 0.0],
            },
            DocumentConstraintDefinition::Coincident {
                first: point(1),
                second: point(2),
            },
            DocumentConstraintDefinition::Horizontal { line: line(3) },
            DocumentConstraintDefinition::Vertical { line: line(3) },
            DocumentConstraintDefinition::HorizontalPoints {
                first: point(1),
                second: point(2),
            },
            DocumentConstraintDefinition::VerticalPoints {
                first: point(1),
                second: point(2),
            },
            DocumentConstraintDefinition::PointOnCurve {
                point: point(1),
                contact: contact(4),
            },
            DocumentConstraintDefinition::Parallel {
                first: line(3),
                second: line(5),
            },
            DocumentConstraintDefinition::Perpendicular {
                first: line(3),
                second: line(5),
            },
            DocumentConstraintDefinition::Concentric {
                first: DocumentCenterRef { curve: curve(6) },
                second: DocumentCenterRef { curve: curve(7) },
            },
            DocumentConstraintDefinition::Collinear {
                first: DocumentLineSupportRef {
                    span: line(3),
                    direction: DocumentDirectionSense::Forward,
                },
                second: DocumentLineSupportRef {
                    span: line(5),
                    direction: DocumentDirectionSense::Reverse,
                },
            },
            DocumentConstraintDefinition::EqualLength {
                first: line(3),
                second: line(5),
            },
            DocumentConstraintDefinition::EqualRadius {
                first: curve(6),
                second: curve(7),
            },
            DocumentConstraintDefinition::Midpoint {
                point: point(1),
                line: line(3),
            },
            DocumentConstraintDefinition::SymmetricAboutLine {
                first: point(1),
                second: point(2),
                line: line(3),
            },
            DocumentConstraintDefinition::SymmetricAboutDatumAxis {
                first: point(1),
                second: point(2),
                axis: DocumentCoordinateAxis::X,
            },
            DocumentConstraintDefinition::CurveCurveContact {
                first_contact: contact(4),
                second_contact: contact(8),
            },
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact: contact(4),
                second_contact: contact(8),
            },
        ];
        let expected = [
            "fixed",
            "coincident",
            "horizontal",
            "vertical",
            "horizontal",
            "vertical",
            "point-on-curve",
            "parallel",
            "perpendicular",
            "concentric",
            "collinear",
            "equal-length",
            "equal-radius",
            "midpoint",
            "symmetry",
            "symmetry",
            "generic-contact",
            "generic-tangency",
        ];
        assert_eq!(
            definitions
                .iter()
                .map(|definition| {
                    let (kind, glyph) = constraint_glyph(definition);
                    assert!(!glyph.is_empty());
                    kind
                })
                .collect::<Vec<_>>(),
            expected
        );

        let target = DesignScalarId(PersistentId::from_u128(9));
        let dimensions = [
            DocumentDimensionDefinition::PointDistance {
                first: point(1),
                second: point(2),
                target,
            },
            DocumentDimensionDefinition::CurveLength {
                curve: line(3),
                target,
            },
            DocumentDimensionDefinition::Radius {
                curve: curve(6),
                target,
            },
            DocumentDimensionDefinition::Diameter {
                curve: curve(6),
                target,
            },
            DocumentDimensionDefinition::OrientedAngle {
                first: line(3),
                second: line(5),
                target,
                orientation: geosolve_sketch::DocumentAngleOrientation::CounterClockwise,
            },
        ];
        assert_eq!(
            dimensions.iter().map(dimension_kind).collect::<Vec<_>>(),
            [
                "point-distance",
                "segment-length",
                "radius",
                "diameter",
                "oriented-angle",
            ]
        );
    }

    #[test]
    fn accepted_scene_glyphs_and_dimensions_keep_persistent_identity_and_domain_values() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("qualified", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        document
            .set_dimension_mode(rectangle.dimensions[1], DocumentDimensionMode::Reference)
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        assert!(scene.update_annotation_values(accepted));
        let selection = [SelectionItem::Point(rectangle.points[0])];
        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &selection,
            None,
            None,
            viewport(),
        );
        let point_identity = format!("data-persistent-id=\"{}\"", rectangle.points[0]);
        assert!(markup.contains("class=\"wb-point selected\""));
        assert!(markup.contains(&point_identity));
        for constraint in accepted.document().constraints() {
            let contextual = svg_markup(
                Some(&scene),
                Some(accepted),
                &[SelectionItem::Constraint(constraint.id)],
                None,
                None,
                viewport(),
            );
            assert_eq!(
                contextual
                    .matches(&format!("data-persistent-id=\"{}\"", constraint.id))
                    .count(),
                1,
                "selected contextual constraint identity must be unique"
            );
        }
        assert!(markup.contains(&format!(
            "data-persistent-id=\"{}\" data-dimension-kind=\"segment-length\" data-dimension-mode=\"driving\" data-dimension-value=\"4\"",
            rectangle.dimensions[0]
        )));
        assert!(!markup.contains(&format!(
            "data-persistent-id=\"{}\" data-dimension-kind=\"segment-length\" data-dimension-mode=\"reference\" data-dimension-value=\"3\"",
            rectangle.dimensions[1]
        )));
        let reference_markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[SelectionItem::Dimension(rectangle.dimensions[1])],
            None,
            None,
            viewport(),
        );
        assert!(reference_markup.contains(&format!(
            "data-persistent-id=\"{}\" data-dimension-kind=\"segment-length\" data-dimension-mode=\"reference\" data-dimension-value=\"3\"",
            rectangle.dimensions[1]
        )));
        let reference_label = &accepted
            .document()
            .dimension(rectangle.dimensions[1])
            .expect("reference dimension")
            .label;
        assert!(reference_markup.contains("wb-dimension selected reference"));
        assert!(reference_markup.contains(">(3)</text>"));
        assert!(reference_markup.contains(&format!(
            "<title>{reference_label}; Reference curve-length dimension; 3 model units</title>"
        )));
        assert!(markup.contains("class=\"wb-dimension-arrow\""));
    }

    #[test]
    fn m87_pre_extraction_renderer_bytes_are_frozen() {
        let document =
            SketchDocument::from_json(include_str!("../../tests/fixtures/m87_renderer_scene.json"))
                .expect("frozen renderer fixture");
        let selected_point = document.points()[0].id;
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted fixture");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .expect("scene");
        assert!(scene.update_annotation_values(accepted));
        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[SelectionItem::Point(selected_point)],
            None,
            None,
            viewport(),
        );
        let digest = geosolve_sketch_intent::intent_content_digest(markup.as_bytes());
        assert_eq!(markup.len(), 8_309);
        assert_eq!(
            digest.to_string(),
            "bd364332a1c4c1024aa17434ec862d5e4968124a2e8cb09829b04c677b898605"
        );
    }

    #[test]
    fn hidden_annotations_remove_constraint_and_dimension_paint_and_dom_hit_targets() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("annotation visibility", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let viewport = viewport();
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("scene");
        scene.set_show_all_constraint_annotations(true);
        let selection = [
            SelectionItem::Constraint(rectangle.constraints[0]),
            SelectionItem::Dimension(rectangle.dimensions[0]),
        ];
        let render = |annotations_visible| {
            let mut scene = scene.clone();
            scene.set_annotations_visible(annotations_visible);
            svg_markup_with_computed_context_action_stamp_and_display(
                Some(&scene),
                Some(accepted),
                &[],
                &selection,
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                viewport,
            )
        };

        let visible = render(true);
        assert!(visible.contains("data-editor-item=\"constraint\""));
        assert!(visible.contains("data-editor-item=\"dimension\""));
        let hidden = render(false);
        assert!(hidden.contains("class=\"wb-curve"));
        assert!(hidden.contains("<g class=\"wb-annotations\"></g>"));
        assert!(!hidden.contains("data-editor-item=\"constraint\""));
        assert!(!hidden.contains("data-editor-item=\"dimension\""));
        assert!(!hidden.contains("class=\"wb-annotation wb-"));
    }

    #[test]
    fn retained_contextual_annotations_are_hidden_inert_nodes_until_context_reveals_them() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("retained contextual annotation", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let viewport = viewport();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("scene");
        let item = SelectionItem::Constraint(rectangle.constraints[0]);
        let annotation_index = scene
            .annotations
            .iter()
            .position(|annotation| annotation.item == item)
            .expect("contextual constraint annotation");
        let annotation_id = format!("id=\"wb-scene-annotation-{annotation_index}\"");
        let render = |display| {
            svg_markup_with_computed_context_action_stamp_and_display(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                display,
                viewport,
            )
        };

        let ordinary = render(CanvasDisplayOptions::default());
        assert!(
            !ordinary.contains(&annotation_id),
            "the ordinary cold renderer must continue omitting context-only annotations"
        );

        let retained = render(CanvasDisplayOptions {
            retain_contextual_annotations: true,
            ..CanvasDisplayOptions::default()
        });
        let start = retained
            .find(&annotation_id)
            .expect("retained annotation id");
        let element_start = retained[..start].rfind('<').expect("annotation start");
        let element_end = start + retained[start..].find('>').expect("annotation end") + 1;
        let element = &retained[element_start..element_end];
        assert!(element.contains("class=\"wb-annotation wb-constraint context-hidden"));
        assert!(element.contains("tabindex=\"-1\""));
        assert!(element.contains("aria-hidden=\"true\""));
        assert!(element.contains("data-editor-item=\"constraint\""));

        let revealed = svg_markup_with_computed_context_action_stamp_and_display(
            Some(&scene),
            Some(accepted),
            &[],
            &[item],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions {
                retain_contextual_annotations: true,
                ..CanvasDisplayOptions::default()
            },
            viewport,
        );
        let start = revealed
            .find(&annotation_id)
            .expect("revealed annotation id");
        let element_start = revealed[..start].rfind('<').expect("annotation start");
        let element_end = start + revealed[start..].find('>').expect("annotation end") + 1;
        let element = &revealed[element_start..element_end];
        assert!(!element.contains("context-hidden"));
        assert!(element.contains("tabindex=\"0\""));
        assert!(!element.contains("aria-hidden"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one SVG contract binds provisional identity, hover and accessibility surfaces"
    )]
    fn provisional_offset_geometry_and_annotation_have_no_selectable_dom_identity() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let rectangle = document
            .add_rectangle("provisional offset", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let viewport = viewport();
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .expect("scene");
        scene.set_show_all_constraint_annotations(true);
        let point = rectangle.points[0];
        let curve = CurveSpan::line(rectangle.curves[0]);
        let constraint = rectangle.constraints[0];
        let dimension = rectangle.dimensions[0];
        let provisional = [
            SelectionItem::Point(point),
            SelectionItem::Curve(curve),
            SelectionItem::Constraint(constraint),
            SelectionItem::Dimension(dimension),
        ];
        let markup = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            &provisional,
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions::default(),
            None,
            viewport,
        );

        let element = |id: &str| {
            let identity = format!("data-persistent-id=\"{id}\"");
            let identity_start = markup.find(&identity).expect("persistent identity");
            let start = markup[..identity_start].rfind('<').expect("element start");
            let end = identity_start + markup[identity_start..].find('>').expect("element end") + 1;
            &markup[start..end]
        };
        let point_element = element(&point.to_string());
        let curve_element = element(&curve.curve.to_string());
        let annotation_start = markup
            .match_indices("<g class=\"wb-annotation")
            .filter_map(|(start, _)| {
                let end = start + markup[start..].find('>')? + 1;
                markup[start..end]
                    .contains("offset-provisional")
                    .then_some((start, end))
            })
            .collect::<Vec<_>>();
        assert_eq!(annotation_start.len(), 2);
        let annotation_elements = annotation_start
            .into_iter()
            .map(|(start, end)| &markup[start..end])
            .collect::<Vec<_>>();
        let constraint_element = annotation_elements
            .iter()
            .copied()
            .find(|element| element.contains("wb-constraint"))
            .expect("provisional constraint glyph");
        let dimension_element = annotation_elements
            .iter()
            .copied()
            .find(|element| element.contains("wb-dimension"))
            .expect("provisional dimension annotation");

        for geometry in [point_element, curve_element] {
            assert!(geometry.contains("offset-provisional"));
            assert!(geometry.contains("data-interactive=\"false\""));
            assert!(!geometry.contains("data-editor-item"));
        }
        for annotation in [constraint_element, dimension_element] {
            assert!(annotation.contains("offset-provisional"));
            assert!(annotation.contains("tabindex=\"-1\" role=\"img\""));
            assert!(annotation.contains("data-provisional=\"true\""));
            assert!(!annotation.contains("data-editor-item"));
            assert!(!annotation.contains("data-persistent-id"));
        }
        assert!(!constraint_element.contains(&constraint.to_string()));
        assert!(!dimension_element.contains(&dimension.to_string()));

        let curve_hover = EditorHoverState {
            target: Some(EditorHoverTarget::Geometry(SelectionItem::Curve(curve))),
            context_owner: Some(SelectionItem::Curve(curve)),
        };
        let curve_hover_markup =
            svg_markup_with_computed_context_action_stamp_display_and_provisional(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                &provisional,
                curve_hover,
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                viewport,
            );
        let curve_identity = format!("data-persistent-id=\"{}\"", curve.curve);
        let curve_identity_start = curve_hover_markup
            .find(&curve_identity)
            .expect("hovered provisional curve identity");
        let curve_start = curve_hover_markup[..curve_identity_start]
            .rfind('<')
            .expect("hovered provisional curve start");
        let curve_end = curve_identity_start
            + curve_hover_markup[curve_identity_start..]
                .find('>')
                .expect("hovered provisional curve end")
            + 1;
        let hovered_curve = &curve_hover_markup[curve_start..curve_end];
        assert!(hovered_curve.contains("geometry-hovered"));
        assert!(hovered_curve.contains("offset-provisional"));
        assert!(!hovered_curve.contains("data-editor-item"));

        let annotation_hover = EditorHoverState {
            target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                item: SelectionItem::Dimension(dimension),
                marker_index: None,
            })),
            context_owner: None,
        };
        let annotation_hover_markup =
            svg_markup_with_computed_context_action_stamp_display_and_provisional(
                Some(&scene),
                Some(accepted),
                &[],
                &[],
                &[],
                &provisional,
                annotation_hover,
                None,
                None,
                None,
                None,
                None,
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                viewport,
            );
        let hovered_annotation = annotation_hover_markup
            .match_indices("<g class=\"wb-annotation")
            .find_map(|(start, _)| {
                let end = start + annotation_hover_markup[start..].find('>')? + 1;
                let element = &annotation_hover_markup[start..end];
                (element.contains("wb-dimension") && element.contains("offset-provisional"))
                    .then_some(element)
            })
            .expect("hovered provisional dimension annotation");
        assert!(hovered_annotation.contains(" hovered"));
        assert!(!hovered_annotation.contains("data-editor-item"));
        assert!(!hovered_annotation.contains("data-persistent-id"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one SVG regression freezes ordered cues, terminals, and disabled-hover semantics"
    )]
    fn offset_chain_markup_preserves_order_reversal_terminals_and_disabled_hover() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let a = document.add_point("a", [-3.0, 0.0]).unwrap();
        let b = document.add_point("b", [-1.0, 0.0]).unwrap();
        let c = document.add_point("c", [1.0, 0.0]).unwrap();
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: b,
                        end: a,
                        branch_direction: [-1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: b,
                        end: c,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().expect("accepted chain");
        let viewport = viewport();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.8,
        )
        .unwrap();
        let presentation = OffsetCanvasPresentation {
            pending: vec![SelectionItem::Curve(first), SelectionItem::Curve(second)],
            unavailable: vec![SelectionItem::Curve(first)],
            unavailable_message: Some("This curve is unavailable for Offset".into()),
            chain: Some(OffsetAuthoringChainPresentation {
                spans: vec![
                    OffsetDirectedSpan {
                        span: first,
                        traversal: OffsetTraversal::Reverse,
                    },
                    OffsetDirectedSpan {
                        span: second,
                        traversal: OffsetTraversal::Forward,
                    },
                ],
                start: OffsetAuthoringChainTerminal {
                    endpoint: OffsetEndpointRef {
                        span: first,
                        endpoint: OffsetEndpointRole::End,
                    },
                    model_position: [-3.0, 0.0],
                },
                end: OffsetAuthoringChainTerminal {
                    endpoint: OffsetEndpointRef {
                        span: second,
                        endpoint: OffsetEndpointRole::End,
                    },
                    model_position: [1.0, 0.0],
                },
            }),
        };
        let markup = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &presentation.pending,
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions::default(),
            Some(&presentation),
            viewport,
        );

        let first_order = markup
            .find("data-offset-chain-index=\"1\"")
            .expect("first ordered cue");
        let second_order = markup
            .find("data-offset-chain-index=\"2\"")
            .expect("second ordered cue");
        assert!(first_order < second_order);
        assert!(
            markup[first_order..]
                .starts_with("data-offset-chain-index=\"1\" data-offset-traversal=\"reverse\"")
        );
        assert!(
            markup[second_order..]
                .starts_with("data-offset-chain-index=\"2\" data-offset-traversal=\"forward\"")
        );
        assert!(markup.contains("data-offset-terminal=\"start\""));
        assert!(markup.contains("data-offset-terminal=\"end\""));
        let cues = markup
            .split_once("<g class=\"wb-offset-chain-cues\"")
            .and_then(|(_, rest)| rest.split_once("</g></g>"))
            .map(|(cues, _)| cues)
            .expect("complete Offset cue group");
        assert!(!cues.contains("data-editor-item"));
        assert!(!cues.contains("data-persistent-id"));

        let first_identity = format!("data-persistent-id=\"{}\"", first.curve);
        let identity = markup.find(&first_identity).expect("first curve identity");
        let element_start = markup[..identity].rfind('<').unwrap();
        let element_end = identity + markup[identity..].find('>').unwrap();
        let first_element = &markup[element_start..=element_end];
        assert!(first_element.contains("offset-unavailable"));
        assert!(first_element.contains("data-interactive=\"false\""));
        assert!(!first_element.contains("data-editor-item"));
        assert!(first_element.contains("aria-disabled=\"true\""));
        assert!(first_element.contains("data-offset-availability=\"unavailable\""));
    }

    #[test]
    fn historical_constraint_annotation_uses_accepted_label_after_design_deletion() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let second = document.add_point("second", [2.0, 0.0]).expect("point");
        for (label, point) in [("fix first", first), ("fix second", second)] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint {
                        point,
                        target: document.point(point).expect("fixed point").position,
                    },
                )
                .expect("fixed constraint");
        }
        let historical = document
            .add_constraint(
                "accepted horizontal",
                DocumentConstraintDefinition::HorizontalPoints { first, second },
            )
            .expect("historical constraint");
        let mut session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let accepted_before = session.accepted_state().expect("accepted state");
        let accepted_identity = accepted_before.identity();
        let historical_source = accepted_before
            .document()
            .constraint(historical)
            .expect("accepted constraint")
            .source_id;

        let outcome = session
            .transact(session.design_identity(), |document| {
                document.remove_with_owned_state(DocumentObjectId::Constraint(historical))?;
                document.set_point_position(first, [40.0, 40.0])?;
                document.add_constraint(
                    "newer rejected fixed point",
                    DocumentConstraintDefinition::FixedPoint {
                        point: first,
                        target: [40.0, 40.0],
                    },
                )
            })
            .expect("retained rejected design");
        assert!(outcome.published_accepted_identity().is_none());
        assert!(session.accepted_state_for_current_input().is_none());
        assert!(session.design_document().constraint(historical).is_none());

        let accepted = session.accepted_state().expect("historical accepted state");
        assert_eq!(accepted.identity(), accepted_identity);
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .expect("detached historical scene");
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(historical))
            .expect("historical annotation");
        assert_eq!(annotation.source, historical_source);
        assert!(
            scene
                .constraint_entries
                .iter()
                .all(|entry| entry.id != historical)
        );

        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[SelectionItem::Constraint(historical)],
            None,
            None,
            viewport(),
        );
        let identity = format!("data-persistent-id=\"{historical}\"");
        assert_eq!(markup.matches(&identity).count(), 1);
        assert!(markup.contains(&format!(
            "aria-label=\"accepted horizontal; horizontal constraint\" data-editor-item=\"constraint\" {identity}"
        )));
        assert!(!markup.contains("aria-label=\"Accepted constraint\""));
        assert!(!markup.contains("aria-label=\"newer rejected fixed point\""));
    }

    #[test]
    fn multi_marker_constraint_hover_marks_only_the_proximate_symbol() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let a = document.add_point("a", [-3.0, 1.0]).unwrap();
        let b = document.add_point("b", [3.0, 1.0]).unwrap();
        let c = document.add_point("c", [-3.0, -1.0]).unwrap();
        let d = document.add_point("d", [3.0, -1.0]).unwrap();
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: a,
                        end: b,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: c,
                        end: d,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let parallel = document
            .add_constraint(
                "parallel pair",
                DocumentConstraintDefinition::Parallel { first, second },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(parallel))
            .unwrap();
        assert!(matches!(
            &annotation.geometry,
            SceneAnnotationGeometry::Glyph { markers } if markers.len() == 2
        ));
        let markup = svg_markup_with_context(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            EditorHoverState {
                target: Some(EditorHoverTarget::Annotation(SceneAnnotationOccurrence {
                    item: annotation.item,
                    marker_index: Some(1),
                })),
                context_owner: Some(SelectionItem::Curve(first)),
            },
            None,
            None,
            viewport(),
        );
        assert_eq!(
            markup
                .matches("class=\"wb-constraint-symbol hovered\"")
                .count(),
            1
        );
        assert!(markup.contains("class=\"wb-constraint-symbol hovered\" transform=\"translate("));
        assert!(markup.contains("data-annotation-marker=\"1\""));
        assert!(!markup.contains("class=\"wb-annotation wb-constraint hovered\""));
    }

    #[test]
    fn perpendicular_constraint_renders_selectable_right_angle_square() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let vertex = document.add_point("vertex", [0.0, 0.0]).unwrap();
        let right = document.add_point("right", [4.0, 0.0]).unwrap();
        let up = document.add_point("up", [0.0, 3.0]).unwrap();
        let horizontal = CurveSpan::line(
            document
                .add_curve(
                    "horizontal",
                    CurveDefinition::Line {
                        start: right,
                        end: vertex,
                        branch_direction: [-1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let vertical = CurveSpan::line(
            document
                .add_curve(
                    "vertical",
                    CurveDefinition::Line {
                        start: up,
                        end: vertex,
                        branch_direction: [0.0, -1.0],
                    },
                )
                .unwrap(),
        );
        let perpendicular = document
            .add_constraint(
                "right angle",
                DocumentConstraintDefinition::Perpendicular {
                    first: horizontal,
                    second: vertical,
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let annotation = scene
            .annotations
            .iter()
            .find(|annotation| annotation.item == SelectionItem::Constraint(perpendicular))
            .unwrap();
        assert!(matches!(
            &annotation.geometry,
            SceneAnnotationGeometry::RightAngle { .. }
        ));

        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[annotation.item],
            None,
            None,
            viewport(),
        );
        assert!(markup.contains("class=\"wb-right-angle\""));
        assert!(markup.contains("class=\"wb-constraint-symbol\""));
        assert!(!markup.contains("data-annotation-marker="));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the reversed-line fixture verifies accepted acute-angle semantics and complete SVG metadata together"
    )]
    fn oriented_angle_annotation_uses_acute_degrees_for_reversed_line_direction() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let intersection = document.add_point("intersection", [0.0, 0.0]).unwrap();
        let x = document.add_point("x", [2.0, 0.0]).unwrap();
        let tip = document
            .add_point(
                "tip",
                [
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                ],
            )
            .unwrap();
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: intersection,
                        end: x,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: tip,
                        end: intersection,
                        branch_direction: [
                            -std::f64::consts::FRAC_1_SQRT_2,
                            -std::f64::consts::FRAC_1_SQRT_2,
                        ],
                    },
                )
                .unwrap(),
        );
        for (label, point, target) in [
            ("fix intersection", intersection, [0.0, 0.0]),
            ("fix x", x, [2.0, 0.0]),
            (
                "fix tip",
                tip,
                [
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                    2.0 * std::f64::consts::FRAC_1_SQRT_2,
                ],
            ),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let target = document
            .add_scalar(
                "angle",
                5.0 * std::f64::consts::FRAC_PI_4,
                ScalarUnit::Angle,
                ScalarDomain::Positive,
            )
            .unwrap();
        let dimension = document
            .add_dimension(
                "angle",
                DocumentDimensionDefinition::OrientedAngle {
                    first,
                    second,
                    target,
                    orientation: DocumentAngleOrientation::CounterClockwise,
                },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let scene = EditorScene::from_accepted(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let markup = svg_markup(Some(&scene), Some(accepted), &[], None, None, viewport());
        assert!(markup.contains(&format!(
            "data-persistent-id=\"{dimension}\" data-dimension-kind=\"oriented-angle\" data-dimension-mode=\"driving\" data-dimension-value=\"45\""
        )));
        assert!(
            markup.contains("aria-label=\"angle; Driving oriented-angle dimension; 45 degrees\"")
        );
        assert!(markup.contains(">45°</text>"));
    }

    #[test]
    fn rejected_line_dimension_highlights_resolved_accepted_operands_and_exposes_tooltips() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let first = document.add_point("first", [0.0, 0.0]).unwrap();
        let second = document.add_point("second", [2.0, 0.0]).unwrap();
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        for (label, point, target) in [
            ("fix first", first, [0.0, 0.0]),
            ("fix second", second, [2.0, 0.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let target = document
            .add_scalar(
                "incompatible target",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateDimension {
                    label: "conflicting length".into(),
                    definition: DocumentDimensionDefinition::CurveLength {
                        curve: CurveSpan::line(line),
                        target,
                    },
                    mode: DocumentDimensionMode::Driving,
                },
            )
            .unwrap();
        let accepted = coordinator.session().accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let problem = coordinator.current_problem_metadata().unwrap();
        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[],
            None,
            Some(&problem),
            viewport(),
        );

        assert!(markup.contains("data-problem-scope=\"targeted\""));
        assert!(markup.contains(&format!(
            "class=\"wb-curve has-problem\" d=\"M 500.000 350.000 L 600.000 350.000 \" data-persistent-id=\"{line}\""
        )));
        assert!(markup.contains(&format!("data-problem-marker=\"curve:{line}\"")));
        assert!(markup.contains(&format!("data-problem-marker=\"point:{first}\"")));
        assert!(markup.contains(&format!("data-problem-marker=\"point:{second}\"")));
        assert!(markup.contains("tabindex=\"0\" role=\"img\" aria-label="));
        assert!(markup.contains("<path class=\"wb-error-marker-icon\""));
        assert!(!markup.contains("<text class=\"wb-error-marker-icon\""));
        assert!(markup.contains("class=\"wb-error-tooltip\""));
        assert!(!markup.contains("data-problem-marker=\"global\""));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn m47_lifecycle_attempt_and_accepted_identity_never_leak_attempt_into_scene() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("lifecycle", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        let parameter = document
            .add_parameter("width", DocumentParameterKind::Length)
            .unwrap();
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            parameter_batch(parameter, 7, ParameterValue::Length(4.0)),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let accepted_before = coordinator.session().accepted_state().unwrap().identity();
        let accepted_geometry = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .solve_result()
            .geometry
            .clone();

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                parameter_batch(parameter, 8, ParameterValue::Angle(4.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let attempt = coordinator.session().last_attempt().identity();
        assert_ne!(attempt.revision().get(), accepted_before.revision().get());

        let accepted = coordinator.session().accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let problem = coordinator.current_problem_metadata().unwrap();
        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &[],
            None,
            Some(&problem),
            viewport(),
        );
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted_before.revision().get()
        )));
        assert!(markup.contains("data-accepted-parameter-revision=\"7\""));
        assert!(!markup.contains("data-attempt-revision="));
        assert!(!markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            attempt.revision().get()
        )));
        assert!(markup.contains("data-problem-scope=\"global\""));
        assert!(markup.contains("data-problem-marker=\"global\""));
        assert!(!markup.contains("has-problem"));
        assert_eq!(accepted.solve_result().geometry, accepted_geometry);
        let problems = coordinator.problems();
        assert!(problems.failure.is_some() || problems.rejection.is_some());

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                parameter_batch(parameter, 9, ParameterValue::Length(5.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let recovered = coordinator.session().accepted_state().unwrap();
        assert_ne!(recovered.identity(), accepted_before);
        assert_eq!(recovered.input().parameter_revision(), 9);
    }

    #[test]
    fn computed_feature_problems_highlight_exact_sources_and_have_global_fallback() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let points = [[-3.0, 0.0], [-1.0, 0.0], [1.0, 0.0], [3.0, 0.0]]
            .map(|position| document.add_point("source point", position).unwrap());
        let first = document
            .add_curve(
                "affected source",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let second = document
            .add_curve(
                "unaffected source",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let accepted = coordinator.session().accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let feature = ComputedFeatureId::from_raw(7);
        let targeted = ComputedFeatureProblemMetadata {
            feature: Some(feature),
            corners: vec![ComputedFeatureCornerId::from_raw(9)],
            sources: vec![NativeCurveSpanSource {
                span: CurveSpan::line(first),
            }],
            scope: EditorProblemScope::Targeted,
            message: "Fillet <root> is unavailable".into(),
        };
        let markup = svg_markup_with_computed_context(
            Some(&scene),
            Some(accepted),
            &[targeted],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            viewport(),
        );
        let curve_tag = |curve| {
            let key = format!("data-persistent-id=\"{curve}\"");
            let end = markup.find(&key).expect("native source path");
            let start = markup[..end].rfind("<path").expect("path boundary");
            &markup[start..end]
        };
        assert!(curve_tag(first).contains("has-problem"));
        assert!(!curve_tag(second).contains("has-problem"));
        assert!(markup.contains(&format!("data-feature-id=\"{feature}\"")));
        assert!(markup.contains(&format!("data-computed-source=\"{first}:0\"")));
        assert!(markup.contains("tabindex=\"0\" role=\"img\""));
        assert!(markup.contains("aria-label=\"Fillet &lt;root&gt; is unavailable\""));

        let global = ComputedFeatureProblemMetadata {
            feature: None,
            corners: Vec::new(),
            sources: Vec::new(),
            scope: EditorProblemScope::Global,
            message: "Computed evaluation unavailable".into(),
        };
        let global_markup = svg_markup_with_computed_context(
            Some(&scene),
            Some(accepted),
            &[global],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            viewport(),
        );
        assert!(global_markup.contains("class=\"wb-error-marker computed global\""));
        assert!(global_markup.contains("data-feature-id=\"global\""));
        assert!(global_markup.contains("data-computed-source=\"global\""));
        assert!(!global_markup.contains("wb-curve has-problem"));
    }
}
