// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{collections::BTreeSet, fmt::Write as _};

use geosolve_constraint_editor::{
    AdvancedConstructionKind, ComputedFeatureProblemMetadata, ComputedFilletContinuationLimitKind,
    ConstructionPreview, ConstructionPreviewGeometry, DimensionTargetDisplayUnit, DraftGuide,
    DraftGuideClassification, DraftGuideGeometry, DraftInferenceFamily, DraftInferenceRelation,
    DraftInferenceResolution, DraftInferenceStatus, EditorHoverState, EditorHoverTarget,
    EditorProblemCategory, EditorProblemMetadata, EditorProblemScope, EditorProblemTarget,
    EditorScene, GeometryInteractionPolicy, OffsetAuthoringChainPresentation,
    OffsetAuthoringChainTerminal, OffsetEndpointRole, OffsetTraversal, SceneAnnotationGeometry,
    SceneAnnotationKind, SceneConstraintGlyph, SceneCurveControl, SceneCurveControlGripGeometry,
    SceneCurveControlGuideKind, SceneCurveControlInteraction, SceneCurveOrigin, SceneDatum,
    SceneFilletAction, SceneFilletActionAvailability, SceneFilletActionId, SceneFilletActionTarget,
    SceneFilletCornerAffordances, ScreenPoint, SelectionItem, Viewport, display_dimension_target,
};
#[cfg(test)]
use geosolve_sketch::DocumentConstraintDefinition;
use geosolve_sketch::{
    DesignScalarId, DocumentArcSweep, DocumentCurveControlAvailability, DocumentCurveControlId,
    DocumentCurveControlKind, DocumentCurveControlWithholdingReason, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionMode, GeometryRole, ScalarUnit,
    SketchAcceptedDocumentState, SketchDatum,
};
use geosolve_sketch_features::NativeCurveSpanSource;

const SCREEN_SIZE: [f64; 2] = [1000.0, 700.0];
const DEFAULT_PIXELS_PER_MODEL_UNIT: f64 = 50.0;
const MIN_PIXELS_PER_MODEL_UNIT: f64 = 2.0;
const MAX_PIXELS_PER_MODEL_UNIT: f64 = 2_000.0;
const FIT_MARGIN_PIXELS: f64 = 64.0;
const GRID_TARGET_MAJOR_PIXELS: f64 = 96.0;

/// Transient, visual-only canvas presentation owned by the demo adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CanvasDisplayOptions {
    pub(crate) grid_visible: bool,
}

/// Transient Offset-specific canvas state that must not be flattened into ordinary selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OffsetCanvasPresentation {
    pub(crate) pending: Vec<SelectionItem>,
    pub(crate) unavailable: Vec<SelectionItem>,
    pub(crate) unavailable_message: Option<String>,
    pub(crate) chain: Option<OffsetAuthoringChainPresentation>,
}

impl Default for CanvasDisplayOptions {
    fn default() -> Self {
        Self { grid_visible: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AdaptiveGridSpec {
    model_major_step: f64,
    major_pixels: f64,
    minor_pixels: f64,
    screen_origin: ScreenPoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasCamera {
    pub(crate) model_center: [f64; 2],
    pub(crate) pixels_per_model_unit: f64,
}

impl Default for CanvasCamera {
    fn default() -> Self {
        Self {
            model_center: [0.0, 0.0],
            pixels_per_model_unit: DEFAULT_PIXELS_PER_MODEL_UNIT,
        }
    }
}

impl CanvasCamera {
    pub(crate) fn viewport(self) -> Viewport {
        Viewport::new(SCREEN_SIZE, self.model_center, self.pixels_per_model_unit)
            .expect("camera invariants keep the viewport valid")
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn center_origin(&mut self) -> bool {
        if self.model_center == [0.0, 0.0] {
            return false;
        }
        self.model_center = [0.0, 0.0];
        true
    }

    pub(crate) fn zoom_about(&mut self, anchor: ScreenPoint, factor: f64) -> bool {
        if !anchor.x.is_finite() || !anchor.y.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return false;
        }
        let before = self.viewport().screen_to_model(anchor);
        let next_scale = (self.pixels_per_model_unit * factor)
            .clamp(MIN_PIXELS_PER_MODEL_UNIT, MAX_PIXELS_PER_MODEL_UNIT);
        if (next_scale - self.pixels_per_model_unit).abs()
            <= f64::EPSILON * self.pixels_per_model_unit.max(1.0)
        {
            return false;
        }
        self.pixels_per_model_unit = next_scale;
        let after = self.viewport().screen_to_model(anchor);
        self.model_center[0] += before[0] - after[0];
        self.model_center[1] += before[1] - after[1];
        true
    }

    pub(crate) fn pan_from(
        &mut self,
        origin_center: [f64; 2],
        origin: ScreenPoint,
        current: ScreenPoint,
    ) -> bool {
        if !origin_center.into_iter().all(f64::is_finite)
            || !origin.x.is_finite()
            || !origin.y.is_finite()
            || !current.x.is_finite()
            || !current.y.is_finite()
        {
            return false;
        }
        self.model_center = [
            origin_center[0] - (current.x - origin.x) / self.pixels_per_model_unit,
            origin_center[1] + (current.y - origin.y) / self.pixels_per_model_unit,
        ];
        true
    }

    pub(crate) fn fit_scene(&mut self, scene: &EditorScene) -> bool {
        let Some((minimum, maximum)) = scene.model_bounds() else {
            return false;
        };
        let width = (maximum[0] - minimum[0]).max(1.0e-9);
        let height = (maximum[1] - minimum[1]).max(1.0e-9);
        let available = [
            SCREEN_SIZE[0] - 2.0 * FIT_MARGIN_PIXELS,
            SCREEN_SIZE[1] - 2.0 * FIT_MARGIN_PIXELS,
        ];
        self.model_center = [
            (minimum[0] + maximum[0]) * 0.5,
            (minimum[1] + maximum[1]) * 0.5,
        ];
        self.pixels_per_model_unit = (available[0] / width)
            .min(available[1] / height)
            .clamp(MIN_PIXELS_PER_MODEL_UNIT, MAX_PIXELS_PER_MODEL_UNIT);
        true
    }

    /// Fits finite native geometry, or returns an empty workplane to the canonical Origin view.
    pub(crate) fn fit_scene_or_reset(&mut self, scene: Option<&EditorScene>) -> bool {
        if scene.is_some_and(|scene| self.fit_scene(scene)) {
            return true;
        }
        self.reset();
        false
    }
}

#[cfg(test)]
pub(crate) fn viewport() -> Viewport {
    CanvasCamera::default().viewport()
}

#[allow(clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    selection: &[SelectionItem],
    construction_preview: Option<&ConstructionPreview>,
    problem: Option<&EditorProblemMetadata>,
    viewport: Viewport,
) -> String {
    svg_markup_with_pending(
        scene,
        accepted,
        selection,
        &[],
        construction_preview,
        problem,
        viewport,
    )
}

#[allow(clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup_with_pending(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    construction_preview: Option<&ConstructionPreview>,
    problem: Option<&EditorProblemMetadata>,
    viewport: Viewport,
) -> String {
    svg_markup_with_context(
        scene,
        accepted,
        selection,
        pending,
        EditorHoverState::default(),
        construction_preview,
        problem,
        viewport,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup_with_context(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    hover: EditorHoverState,
    construction_preview: Option<&ConstructionPreview>,
    problem: Option<&EditorProblemMetadata>,
    viewport: Viewport,
) -> String {
    svg_markup_with_computed_context(
        scene,
        accepted,
        &[],
        selection,
        pending,
        hover,
        construction_preview,
        problem,
        None,
        viewport,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup_with_computed_context(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    computed_problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    hover: EditorHoverState,
    construction_preview: Option<&ConstructionPreview>,
    problem: Option<&EditorProblemMetadata>,
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    viewport: Viewport,
) -> String {
    svg_markup_with_computed_context_and_action_stamp(
        scene,
        accepted,
        computed_problems,
        selection,
        pending,
        hover,
        construction_preview,
        None,
        problem,
        active_fillet_preview,
        None,
        GeometryInteractionPolicy::default(),
        viewport,
    )
}

/// Renders one exact scene while attaching an opaque adapter-owned stamp to
/// every actionable Fillet branch control.
///
/// The stamp is not feature semantics. The browser adapter retains its exact
/// [`geosolve_sketch_features::ComputedFeatureEvaluationInput`] and rejects a
/// DOM event unless both still match, so an old element cannot manufacture a
/// target for a newer scene from persistent owner/action IDs alone.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup_with_computed_context_and_action_stamp(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    computed_problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    hover: EditorHoverState,
    construction_preview: Option<&ConstructionPreview>,
    inference: Option<&DraftInferenceResolution>,
    problem: Option<&EditorProblemMetadata>,
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
    viewport: Viewport,
) -> String {
    svg_markup_with_computed_context_action_stamp_and_display(
        scene,
        accepted,
        computed_problems,
        selection,
        pending,
        hover,
        construction_preview,
        inference,
        problem,
        active_fillet_preview,
        fillet_action_stamp,
        geometry_policy,
        CanvasDisplayOptions::default(),
        viewport,
    )
}

/// Renders the exact accepted/intrinsic scene with transient visual-only display options.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(test)]
pub(crate) fn svg_markup_with_computed_context_action_stamp_and_display(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    computed_problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    hover: EditorHoverState,
    construction_preview: Option<&ConstructionPreview>,
    inference: Option<&DraftInferenceResolution>,
    problem: Option<&EditorProblemMetadata>,
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
    display: CanvasDisplayOptions,
    viewport: Viewport,
) -> String {
    svg_markup_with_computed_context_action_stamp_display_and_provisional(
        scene,
        accepted,
        computed_problems,
        selection,
        pending,
        &[],
        hover,
        construction_preview,
        inference,
        problem,
        active_fillet_preview,
        fillet_action_stamp,
        geometry_policy,
        display,
        None,
        viewport,
    )
}

/// Renders a candidate scene while keeping exact prepared-patch geometry visibly provisional and
/// outside every DOM interaction route.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn svg_markup_with_computed_context_action_stamp_display_and_provisional(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    computed_problems: &[ComputedFeatureProblemMetadata],
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    construction_preview: Option<&ConstructionPreview>,
    inference: Option<&DraftInferenceResolution>,
    problem: Option<&EditorProblemMetadata>,
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
    display: CanvasDisplayOptions,
    offset: Option<&OffsetCanvasPresentation>,
    viewport: Viewport,
) -> String {
    let mut output = String::new();
    let mut problem_markers = String::new();
    let mut computed_problem_markers = String::new();
    let mut resolved_targets = BTreeSet::new();
    let problem_items = problem
        .map(|problem| {
            problem
                .targets
                .iter()
                .filter_map(|target| problem_selection_item(*target, scene))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let related = scene.map_or_else(BTreeSet::new, |scene| {
        let mut related = scene
            .annotations
            .iter()
            .filter(|annotation| {
                selection.contains(&annotation.item)
                    || matches!(
                        hover.target,
                        Some(EditorHoverTarget::Annotation(occurrence))
                            if occurrence.item == annotation.item
                    )
            })
            .flat_map(|annotation| annotation.operands.iter().copied())
            .collect::<BTreeSet<_>>();
        for curve in &scene.computed_offset_curves {
            let owner = SelectionItem::Feature(curve.owner);
            if selection.contains(&owner) || geometry_is_hovered(hover, owner) {
                related.insert(SelectionItem::Curve(curve.source.span));
            }
        }
        related
    });
    let failed_feature_sources = failed_computed_sources(computed_problems);
    if let Some(accepted) = accepted {
        let identity = accepted.identity();
        let input = accepted.input();
        let _ = write!(
            output,
            "<g class=\"wb-accepted-scene\" data-scene-provenance=\"accepted\" data-accepted-document=\"{}\" data-accepted-revision=\"{}\" data-accepted-parameter-revision=\"{}\" data-accepted-parameter-digest=\"{}\" data-accepted-external-revision=\"{}\" data-accepted-external-digest=\"{}\" data-accepted-activation-revision=\"{}\" data-accepted-activation-digest=\"{}\">",
            identity.document(),
            identity.revision().get(),
            input.parameter_revision(),
            digest(input.parameter_digest().bytes()),
            input.external_snapshot_set_revision(),
            digest(input.external_snapshot_set_digest().bytes()),
            input.effective_activation_revision(),
            digest(input.activation_digest().bytes()),
        );
    } else {
        output.push_str("<g class=\"wb-accepted-scene\" data-scene-provenance=\"none\">");
    }
    output.push_str(concat!(
        "<defs><marker id=\"wb-fillet-direction-arrow\" markerWidth=\"6\" markerHeight=\"6\" ",
        "refX=\"5\" refY=\"3\" orient=\"auto\"><path fill=\"context-stroke\" ",
        "d=\"M0 0L6 3L0 6Z\"/></marker>",
        "<marker id=\"wb-offset-chain-arrow\" markerWidth=\"7\" markerHeight=\"7\" ",
        "refX=\"6\" refY=\"3.5\" orient=\"auto\"><path fill=\"context-stroke\" ",
        "d=\"M0 0L7 3.5L0 7Z\"/></marker></defs>"
    ));
    if display.grid_visible {
        render_adaptive_grid(&mut output, viewport);
    }
    if let Some(scene) = scene
        && geometry_policy.visibility.reference_geometry
    {
        render_datums(&mut output, scene, selection, hover, &related, viewport);
    }
    output.push_str("<g class=\"wb-geometry\">");
    if let Some(scene) = scene {
        for curve in scene
            .curves
            .iter()
            .filter(|curve| curve.is_visible(geometry_policy))
        {
            if curve.screen_polyline.len() < 2 {
                continue;
            }
            let path = polyline_path(&curve.screen_polyline);
            let item = SelectionItem::Curve(curve.span);
            let selected = selection.contains(&item);
            let pending = pending.contains(&item);
            let provisional = provisional.contains(&item);
            let offset_unavailable =
                offset.is_some_and(|offset| offset.unavailable.contains(&item));
            let target = EditorProblemTarget::Curve(curve.span.curve);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let role = curve.role;
            let interactive =
                curve.is_interactive(geometry_policy) && !provisional && !offset_unavailable;
            let hovered = geometry_is_hovered(hover, item);
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-curve{}{}{}{}{}{}{}{}{}\" d=\"{}\" ",
                    "data-persistent-id=\"{}\" {}",
                    "data-editor-segment=\"{}\" data-role=\"{}\" data-source-role=\"{}\" ",
                    "data-construction-origin=\"{}\" data-interactive=\"{}\" {}/>"
                ),
                if selected { " selected" } else { "" },
                if hovered { " geometry-hovered" } else { "" },
                if pending { " authoring-pending" } else { "" },
                if provisional {
                    " offset-provisional"
                } else {
                    ""
                },
                if offset_unavailable {
                    " offset-unavailable"
                } else {
                    ""
                },
                if related.contains(&item) {
                    " related"
                } else {
                    ""
                },
                if role == GeometryRole::Construction {
                    " construction"
                } else {
                    ""
                },
                if has_problem { " has-problem" } else { "" },
                if failed_feature_sources.contains(&NativeCurveSpanSource { span: curve.span }) {
                    " has-problem"
                } else {
                    ""
                },
                path,
                curve.span.curve,
                if interactive {
                    "data-editor-item=\"curve\" "
                } else {
                    ""
                },
                curve.span.segment,
                geometry_role_key(role),
                geometry_role_key(curve.source_role),
                scene_curve_origin_key(curve.origin, role),
                interactive,
                if offset_unavailable {
                    format!(
                        "role=\"img\" aria-disabled=\"true\" data-offset-availability=\"unavailable\" aria-label=\"{}\"",
                        escape(
                            offset
                                .and_then(|offset| offset.unavailable_message.as_deref())
                                .unwrap_or("Unavailable for Offset"),
                        ),
                    )
                } else {
                    String::new()
                },
            );
            if has_problem && resolved_targets.insert(target) {
                let anchor = curve.screen_polyline[curve.screen_polyline.len() / 2];
                problem_marker(
                    &mut problem_markers,
                    anchor,
                    Some(target),
                    problem.expect("targeted marker has problem metadata"),
                    false,
                );
            }
        }
        render_computed_geometry(
            &mut output,
            scene,
            selection,
            provisional,
            hover,
            active_fillet_preview,
            fillet_action_stamp,
            geometry_policy,
        );
        render_computed_problem_markers(
            &mut computed_problem_markers,
            scene,
            computed_problems,
            geometry_policy,
        );
        render_curve_control_guides(&mut output, scene, hover);
        output.push_str("</g><g class=\"wb-points\">");
        for point in scene
            .points
            .iter()
            .filter(|point| point.is_visible(geometry_policy))
        {
            let interactive = point.is_interactive(geometry_policy);
            let item = SelectionItem::Point(point.id);
            let selected = selection.contains(&item);
            let hovered = geometry_is_hovered(hover, item);
            let pending = pending.contains(&item);
            let provisional = provisional.contains(&item);
            let target = EditorProblemTarget::Point(point.id);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let interactive = interactive && !provisional;
            let _ = write!(
                output,
                concat!(
                    "<circle class=\"wb-point{}{}{}{}{}{}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" ",
                    "data-persistent-id=\"{}\" {}data-interactive=\"{}\"/>"
                ),
                if selected { " selected" } else { "" },
                if hovered { " geometry-hovered" } else { "" },
                if pending { " authoring-pending" } else { "" },
                if provisional {
                    " offset-provisional"
                } else {
                    ""
                },
                if related.contains(&item) {
                    " related"
                } else {
                    ""
                },
                if has_problem { " has-problem" } else { "" },
                point.screen_position.x,
                point.screen_position.y,
                point.id,
                if interactive {
                    "data-editor-item=\"point\" "
                } else {
                    ""
                },
                interactive,
            );
            if has_problem && resolved_targets.insert(target) {
                problem_marker(
                    &mut problem_markers,
                    point.screen_position,
                    Some(target),
                    problem.expect("targeted marker has problem metadata"),
                    false,
                );
            }
        }
    } else {
        output.push_str("</g><g class=\"wb-points\">");
    }
    output.push_str("</g>");
    output.push_str("<g class=\"wb-annotations\">");
    if let (Some(scene), Some(accepted)) = (scene, accepted) {
        render_annotations(
            &mut output,
            &mut problem_markers,
            &mut resolved_targets,
            scene,
            accepted,
            selection,
            pending,
            provisional,
            hover,
            &problem_items,
            problem,
        );
    }
    output.push_str("</g>");
    if let Some(scene) = scene {
        render_curve_controls(&mut output, scene, hover);
        if let Some(chain) = offset.and_then(|offset| offset.chain.as_ref()) {
            render_offset_chain_cues(&mut output, scene, chain, viewport);
        }
    }
    if let Some(inference) = inference {
        render_inference_guides(&mut output, inference, viewport);
    }
    if let Some(preview) = construction_preview {
        output.push_str(&construction_markup(preview, viewport));
    }
    if let Some(inference) = inference {
        render_inference_candidates(&mut output, inference);
    }
    if let Some(problem) = problem {
        if problem.scope == EditorProblemScope::Global || resolved_targets.is_empty() {
            problem_marker(
                &mut problem_markers,
                ScreenPoint { x: 970.0, y: 28.0 },
                None,
                problem,
                true,
            );
        }
        let _ = write!(
            output,
            "<g class=\"wb-error-overlay\" data-problem-attempt=\"{}\" data-problem-scope=\"{}\" data-problem-category=\"{}\">{problem_markers}</g>",
            problem.attempt.revision().get(),
            if problem.scope == EditorProblemScope::Global {
                "global"
            } else {
                "targeted"
            },
            problem_category_key(problem.category),
        );
    }
    if !computed_problem_markers.is_empty() {
        let _ = write!(
            output,
            "<g class=\"wb-error-overlay wb-computed-error-overlay\" data-computed-problems=\"{}\">{computed_problem_markers}</g>",
            computed_problems.len(),
        );
    }
    output.push_str("</g>");
    output
}

fn adaptive_grid_spec(viewport: Viewport) -> Option<AdaptiveGridSpec> {
    let raw_step = GRID_TARGET_MAJOR_PIXELS / viewport.pixels_per_model_unit;
    if !raw_step.is_finite() || raw_step <= 0.0 {
        return None;
    }
    let decade = 10.0_f64.powf(raw_step.log10().floor());
    let normalized = raw_step / decade;
    let multiplier = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    let model_major_step = multiplier * decade;
    let major_pixels = model_major_step * viewport.pixels_per_model_unit;
    let minor_pixels = major_pixels / 5.0;
    (model_major_step.is_finite()
        && model_major_step > 0.0
        && major_pixels.is_finite()
        && minor_pixels.is_finite()
        && minor_pixels > 0.0)
        .then(|| AdaptiveGridSpec {
            model_major_step,
            major_pixels,
            minor_pixels,
            screen_origin: viewport.model_to_screen([0.0, 0.0]),
        })
}

fn render_adaptive_grid(output: &mut String, viewport: Viewport) {
    let Some(spec) = adaptive_grid_spec(viewport) else {
        return;
    };
    let minor = grid_path(spec.screen_origin, spec.minor_pixels, viewport.screen_size);
    let major = grid_path(spec.screen_origin, spec.major_pixels, viewport.screen_size);
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-grid\" aria-hidden=\"true\" data-grid-kind=\"adaptive-1-2-5\" ",
            "data-grid-major-model=\"{:.12}\" data-grid-major-pixels=\"{:.3}\">",
            "<path class=\"wb-grid-minor\" d=\"{}\"/>",
            "<path class=\"wb-grid-major\" d=\"{}\"/></g>"
        ),
        spec.model_major_step, spec.major_pixels, minor, major,
    );
}

fn grid_path(origin: ScreenPoint, spacing: f64, screen_size: [f64; 2]) -> String {
    let mut path = String::new();
    let first_x = origin.x.rem_euclid(spacing);
    let first_y = origin.y.rem_euclid(spacing);
    let mut x = first_x;
    while x <= screen_size[0] {
        let _ = write!(path, "M{x:.3} 0V{:.3}", screen_size[1]);
        x += spacing;
    }
    let mut y = first_y;
    while y <= screen_size[1] {
        let _ = write!(path, "M0 {y:.3}H{:.3}", screen_size[0]);
        y += spacing;
    }
    path
}

fn render_datums(
    output: &mut String,
    scene: &EditorScene,
    selection: &[SelectionItem],
    hover: EditorHoverState,
    related: &BTreeSet<SelectionItem>,
    viewport: Viewport,
) {
    output.push_str("<g class=\"wb-reference-geometry\" data-reference-provenance=\"intrinsic\">");
    for datum in &scene.datums {
        let item = SelectionItem::Datum(datum.datum);
        let selected = selection.contains(&item);
        let hovered = geometry_is_hovered(hover, item);
        let related = related.contains(&item);
        let state_classes = format!(
            "{}{}{}",
            if selected { " selected" } else { "" },
            if hovered { " geometry-hovered" } else { "" },
            if related { " related" } else { "" },
        );
        match datum.datum {
            // The axis intersection already presents Origin visually. Keep the
            // headless datum for picking/authoring and its accessible tree row,
            // but do not paint a duplicate canvas marker or focus target.
            SketchDatum::Origin => {}
            SketchDatum::XAxis | SketchDatum::YAxis => {
                render_axis_datum(output, datum, &state_classes, viewport);
            }
        }
    }
    output.push_str("</g>");
}

fn geometry_is_hovered(hover: EditorHoverState, item: SelectionItem) -> bool {
    matches!(hover.target, Some(EditorHoverTarget::Geometry(target)) if target == item)
}

fn render_offset_chain_cues(
    output: &mut String,
    scene: &EditorScene,
    chain: &OffsetAuthoringChainPresentation,
    viewport: Viewport,
) {
    if chain.spans.is_empty() {
        return;
    }
    let _ = write!(
        output,
        "<g class=\"wb-offset-chain-cues\" role=\"img\" aria-label=\"Ordered Offset chain, {} edges, Start to End\" pointer-events=\"none\">",
        chain.spans.len(),
    );
    for (index, directed) in chain.spans.iter().enumerate() {
        let Some(curve) = scene
            .curves
            .iter()
            .find(|curve| curve.span == directed.span)
        else {
            continue;
        };
        let middle = curve.screen_polyline.len() / 2;
        if middle == 0 {
            continue;
        }
        let (local_start, local_end) = (
            curve.screen_polyline[middle - 1],
            curve.screen_polyline[middle],
        );
        let delta = [local_end.x - local_start.x, local_end.y - local_start.y];
        let length = delta[0].hypot(delta[1]);
        if !length.is_finite() || length <= f64::EPSILON {
            continue;
        }
        let center = ScreenPoint {
            x: (local_start.x + local_end.x) * 0.5,
            y: (local_start.y + local_end.y) * 0.5,
        };
        let half_length = 8.0;
        let direction = [delta[0] / length, delta[1] / length];
        let (mut start, mut end) = (
            ScreenPoint {
                x: center.x - half_length * direction[0],
                y: center.y - half_length * direction[1],
            },
            ScreenPoint {
                x: center.x + half_length * direction[0],
                y: center.y + half_length * direction[1],
            },
        );
        if directed.traversal == OffsetTraversal::Reverse {
            std::mem::swap(&mut start, &mut end);
        }
        let _ = write!(
            output,
            concat!(
                "<path class=\"wb-offset-chain-direction\" data-offset-chain-index=\"{}\" ",
                "data-offset-traversal=\"{}\" data-curve-id=\"{}\" data-editor-segment=\"{}\" ",
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\" marker-end=\"url(#wb-offset-chain-arrow)\"/>"
            ),
            index + 1,
            match directed.traversal {
                OffsetTraversal::Forward => "forward",
                OffsetTraversal::Reverse => "reverse",
            },
            directed.span.curve,
            directed.span.segment,
            start.x,
            start.y,
            end.x,
            end.y,
        );
    }
    render_offset_chain_terminal(output, chain.start, "start", "S", viewport);
    render_offset_chain_terminal(output, chain.end, "end", "E", viewport);
    output.push_str("</g>");
}

fn render_offset_chain_terminal(
    output: &mut String,
    terminal: OffsetAuthoringChainTerminal,
    kind: &'static str,
    label: &'static str,
    viewport: Viewport,
) {
    let position = viewport.model_to_screen(terminal.model_position);
    let native_endpoint = match terminal.endpoint.endpoint {
        OffsetEndpointRole::Start => "start",
        OffsetEndpointRole::End => "end",
    };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-offset-chain-terminal {}\" data-offset-terminal=\"{}\" ",
            "data-curve-id=\"{}\" data-editor-segment=\"{}\" data-native-endpoint=\"{}\" ",
            "transform=\"translate({:.3} {:.3})\"><title>{} terminal</title>",
            "<circle r=\"7\"/><text x=\"0\" y=\"0\">{}</text></g>"
        ),
        kind,
        kind,
        terminal.endpoint.span.curve,
        terminal.endpoint.span.segment,
        native_endpoint,
        position.x,
        position.y,
        if kind == "start" { "Start" } else { "End" },
        label,
    );
}

fn curve_control_is_hovered(hover: EditorHoverState, control: DocumentCurveControlId) -> bool {
    matches!(
        hover.target,
        Some(EditorHoverTarget::CurveControl { control: target, .. }) if target == control
    )
}

fn render_curve_control_guides(output: &mut String, scene: &EditorScene, hover: EditorHoverState) {
    if scene.curve_control_guides.is_empty() {
        return;
    }
    output.push_str(
        "<g class=\"wb-curve-control-guides\" aria-hidden=\"true\" pointer-events=\"none\">",
    );
    for guide in &scene.curve_control_guides {
        let hovered = guide
            .control
            .is_some_and(|control| curve_control_is_hovered(hover, control));
        let kind = curve_control_guide_key(guide.kind);
        let class = if guide.kind == SceneCurveControlGuideKind::SizeRail {
            "wb-curve-control-rail"
        } else {
            "wb-curve-control-guide"
        };
        let _ = write!(
            output,
            concat!(
                "<path class=\"{}{}\" data-control-guide=\"{}\" data-curve-id=\"{}\" ",
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>"
            ),
            class,
            if hovered { " hovered" } else { "" },
            kind,
            guide.owner,
            guide.screen_start.x,
            guide.screen_start.y,
            guide.screen_end.x,
            guide.screen_end.y,
        );
    }
    output.push_str("</g>");
}

fn render_curve_controls(output: &mut String, scene: &EditorScene, hover: EditorHoverState) {
    if scene.curve_controls.is_empty() {
        return;
    }
    output.push_str("<g class=\"wb-curve-control-cage\">");
    for control in &scene.curve_controls {
        // Stored design-point aliases keep the ordinary point presentation and
        // pointer owner. They remain in the headless catalog so guides can use
        // their exact anchors, but painting a second grip would falsely imply a
        // second selectable object over the same point.
        if !matches!(control.interaction, SceneCurveControlInteraction::Direct) {
            continue;
        }
        render_curve_control(output, control, hover);
    }
    output.push_str("</g>");
}

fn render_curve_control(output: &mut String, control: &SceneCurveControl, hover: EditorHoverState) {
    let hovered = curve_control_is_hovered(hover, control.id);
    let read_only = !control.is_editable();
    let role = curve_control_kind_key(control.id.kind);
    let label = match control.availability {
        DocumentCurveControlAvailability::Editable => control.accessible_name.clone(),
        DocumentCurveControlAvailability::ReadOnly(reason) => format!(
            "{} · read-only: {}",
            control.accessible_name,
            curve_control_read_only_reason(reason),
        ),
    };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-curve-control{}{}{}\" role=\"img\" aria-label=\"{}\" ",
            "aria-disabled=\"{}\" data-control-role=\"{}\" data-curve-id=\"{}\" ",
            "data-editor-segment=\"{}\"{} pointer-events=\"none\">"
        ),
        if hovered { " hovered" } else { "" },
        if read_only { " read-only" } else { "" },
        if control.offset_proxy.is_some() {
            " offset-proxy"
        } else {
            ""
        },
        escape(&label),
        read_only,
        role,
        control.id.curve,
        control.owner.segment,
        control.offset_proxy.map_or_else(String::new, |proxy| {
            format!(
                " data-offset-proxy=\"true\" data-feature-id=\"{}\"",
                proxy.feature
            )
        }),
    );
    let _ = write!(output, "<title>{}</title>", escape(&label));
    match control.grip {
        SceneCurveControlGripGeometry::Circle {
            center,
            radius_pixels,
        } => {
            let _ = write!(
                output,
                "<circle class=\"wb-curve-control-mark\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"/>",
                center.x, center.y, radius_pixels,
            );
        }
        SceneCurveControlGripGeometry::Square {
            center,
            half_extent_pixels,
        } => {
            let _ = write!(
                output,
                "<rect class=\"wb-curve-control-mark\" x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"/>",
                center.x - half_extent_pixels,
                center.y - half_extent_pixels,
                half_extent_pixels * 2.0,
                half_extent_pixels * 2.0,
            );
        }
        SceneCurveControlGripGeometry::Diamond {
            center,
            radius_pixels,
        } => {
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-curve-control-mark\" ",
                    "d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}Z\"/>"
                ),
                center.x,
                center.y - radius_pixels,
                center.x + radius_pixels,
                center.y,
                center.x,
                center.y + radius_pixels,
                center.x - radius_pixels,
                center.y,
            );
        }
    }
    if hovered {
        let _ = write!(
            output,
            "<text class=\"wb-curve-control-tooltip\" x=\"{:.3}\" y=\"{:.3}\" aria-hidden=\"true\">{}</text>",
            control.screen_position.x + 10.0,
            control.screen_position.y - 10.0,
            escape(&label),
        );
    }
    output.push_str("</g>");
}

const fn curve_control_guide_key(kind: SceneCurveControlGuideKind) -> &'static str {
    match kind {
        SceneCurveControlGuideKind::ControlPolygon => "control-polygon",
        SceneCurveControlGuideKind::PrincipalAxis => "principal-axis",
        SceneCurveControlGuideKind::FocusAxis => "focus-axis",
        SceneCurveControlGuideKind::RadiusSpoke => "radius-spoke",
        SceneCurveControlGuideKind::MinorAxisSpoke => "minor-axis-spoke",
        SceneCurveControlGuideKind::ConjugateAxisSpoke => "conjugate-axis-spoke",
        SceneCurveControlGuideKind::ProjectiveVector => "projective-vector",
        SceneCurveControlGuideKind::SizeRail => "size-rail",
    }
}

const fn curve_control_kind_key(kind: DocumentCurveControlKind) -> &'static str {
    match kind {
        DocumentCurveControlKind::Center => "center",
        DocumentCurveControlKind::StartPoint => "start-point",
        DocumentCurveControlKind::EndPoint => "end-point",
        DocumentCurveControlKind::ControlPoint { .. } => "control-point",
        DocumentCurveControlKind::Radius => "radius",
        DocumentCurveControlKind::TrimStart => "trim-start",
        DocumentCurveControlKind::TrimEnd => "trim-end",
        DocumentCurveControlKind::MajorAxisPoint => "major-axis-point",
        DocumentCurveControlKind::MinorAxis => "minor-axis",
        DocumentCurveControlKind::RationalMiddle => "rational-middle",
        DocumentCurveControlKind::Vertex => "vertex",
        DocumentCurveControlKind::Focus => "focus",
        DocumentCurveControlKind::TransverseAxisPoint => "transverse-axis-point",
        DocumentCurveControlKind::ConjugateAxis => "conjugate-axis",
        _ => "curve-control",
    }
}

const fn curve_control_read_only_reason(
    reason: DocumentCurveControlWithholdingReason,
) -> &'static str {
    match reason {
        DocumentCurveControlWithholdingReason::InactiveCurve => "curve is inactive",
        DocumentCurveControlWithholdingReason::AssociativeFilletOutput => {
            "the associative Fillet owns this output"
        }
        DocumentCurveControlWithholdingReason::HostParameterOwned => {
            "value is owned by a host parameter"
        }
        DocumentCurveControlWithholdingReason::GaugeOwned => "value is the active NURBS gauge",
        DocumentCurveControlWithholdingReason::DrivingDimensionOwned => {
            "an active driving radius or diameter dimension owns this size"
        }
        DocumentCurveControlWithholdingReason::EqualRadiusOwned => {
            "an active equal-radius relation owns this size"
        }
        _ => "the curve owner does not expose a direct edit",
    }
}

fn render_axis_datum(
    output: &mut String,
    datum: &SceneDatum,
    state_classes: &str,
    viewport: Viewport,
) {
    if !datum.is_visible_in_viewport(viewport) {
        return;
    }
    let is_x = datum.datum == SketchDatum::XAxis;
    let coordinate = if is_x {
        datum.screen_start.y
    } else {
        datum.screen_start.x
    };
    let (key, label, axis_class, label_x, label_y) = if is_x {
        (
            "x-axis",
            "X",
            "wb-datum-x-axis",
            viewport.screen_size[0] - 20.0,
            (coordinate - 8.0).max(14.0),
        )
    } else {
        (
            "y-axis",
            "Y",
            "wb-datum-y-axis",
            (coordinate + 9.0).min(viewport.screen_size[0] - 18.0),
            18.0,
        )
    };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-datum wb-datum-axis {}{}\" role=\"button\" tabindex=\"0\" ",
            "aria-label=\"{} axis · protected infinite intrinsic reference\" ",
            "data-editor-item=\"datum\" data-datum=\"{}\" data-protected=\"true\">",
            "<path class=\"wb-datum-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
            "<path class=\"wb-datum-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
            "<text class=\"wb-datum-label\" x=\"{:.3}\" y=\"{:.3}\">{}</text></g>"
        ),
        axis_class,
        state_classes,
        label,
        key,
        datum.screen_start.x,
        datum.screen_start.y,
        datum.screen_end.x,
        datum.screen_end.y,
        datum.screen_start.x,
        datum.screen_start.y,
        datum.screen_end.x,
        datum.screen_end.y,
        label_x,
        label_y,
        label,
    );
}

fn failed_computed_sources(
    problems: &[ComputedFeatureProblemMetadata],
) -> BTreeSet<NativeCurveSpanSource> {
    problems
        .iter()
        .filter(|problem| problem.scope == EditorProblemScope::Targeted)
        .flat_map(|problem| problem.sources.iter().copied())
        .collect()
}

fn render_computed_problem_markers(
    output: &mut String,
    scene: &EditorScene,
    problems: &[ComputedFeatureProblemMetadata],
    geometry_policy: GeometryInteractionPolicy,
) {
    for (index, problem) in problems.iter().enumerate() {
        let marker_row = u32::try_from(index).unwrap_or(u32::MAX);
        let source_anchor = (problem.scope == EditorProblemScope::Targeted)
            .then(|| {
                problem.sources.iter().find_map(|source| {
                    scene
                        .curves
                        .iter()
                        .filter(|curve| curve.is_visible(geometry_policy))
                        .find(|curve| curve.span == source.span)
                        .and_then(|curve| {
                            (!curve.screen_polyline.is_empty()).then(|| {
                                (
                                    curve.screen_polyline[curve.screen_polyline.len() / 2],
                                    *source,
                                )
                            })
                        })
                })
            })
            .flatten();
        let (anchor, source, global) = source_anchor.map_or_else(
            || {
                (
                    ScreenPoint {
                        x: 970.0,
                        y: 28.0 + 24.0 * f64::from(marker_row),
                    },
                    None,
                    true,
                )
            },
            |(anchor, source)| (anchor, Some(source), false),
        );
        computed_problem_marker(output, anchor, source, problem, global, index);
    }
}

fn computed_problem_marker(
    output: &mut String,
    anchor: ScreenPoint,
    source: Option<NativeCurveSpanSource>,
    problem: &ComputedFeatureProblemMetadata,
    global: bool,
    index: usize,
) {
    let message = escape(&problem.message);
    let feature = problem
        .feature
        .map_or_else(|| "global".to_owned(), |feature| feature.to_string());
    let source_key = source.map_or_else(
        || "global".to_owned(),
        |source| format!("{}:{}", source.span.curve, source.span.segment),
    );
    let tooltip_x = if global || anchor.x > 610.0 {
        -370.0
    } else {
        14.0
    };
    let tooltip_y = if anchor.y > 610.0 { -82.0 } else { 14.0 };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-error-marker computed{}\" transform=\"translate({:.3} {:.3})\" ",
            "tabindex=\"0\" role=\"img\" aria-label=\"{}\" ",
            "data-problem-marker=\"computed:{}:{}\" data-computed-problem=\"{}\" ",
            "data-feature-id=\"{}\" data-computed-source=\"{}\">",
            "<circle r=\"10\"/>{}",
            "<foreignObject class=\"wb-error-tooltip\" x=\"{}\" y=\"{}\" width=\"360\" height=\"72\">",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\">{}</div></foreignObject></g>"
        ),
        if global { " global" } else { "" },
        anchor.x,
        anchor.y,
        message,
        feature,
        source_key,
        index,
        feature,
        source_key,
        super::icons::PROBLEM_ICON,
        tooltip_x,
        tooltip_y,
        message,
    );
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one equation-free renderer keeps computed Fillet and Curve Offset scene authority explicit"
)]
fn render_computed_geometry(
    output: &mut String,
    scene: &EditorScene,
    selection: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
) {
    let evaluation = scene
        .computed_curves
        .first()
        .map_or(0, |curve| curve.edge.evaluation.raw());
    let evaluation = scene
        .computed_offset_curves
        .first()
        .map_or(evaluation, |curve| curve.edge.evaluation.raw());
    let _ = write!(
        output,
        "<g class=\"wb-computed-geometry\" data-computed-evaluation=\"{evaluation}\">"
    );
    let affected_owners = scene
        .fillet_affordances
        .iter()
        .filter(|affordances| fillet_owner_is_visible(affordances.owner, selection))
        .flat_map(|affordances| affordances.affected_owners.iter().copied())
        .collect::<BTreeSet<_>>();
    for curve in scene
        .computed_curves
        .iter()
        .filter(|curve| curve.is_visible(geometry_policy))
    {
        let item = SelectionItem::FeatureCorner(curve.owner);
        let selected = selection.contains(&item)
            || selection.contains(&SelectionItem::Feature(curve.owner.feature));
        let hovered = geometry_is_hovered(hover, item);
        let affected = affected_owners.contains(&curve.owner);
        let interactive = curve.is_interactive(geometry_policy);
        let path = polyline_path(&curve.screen_polyline);
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-computed-item{}{}{}{}\" {}",
                "data-feature-id=\"{}\" data-feature-corner-id=\"{}\" ",
                "data-computed-evaluation=\"{}\" data-computed-edge=\"{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\">",
                "<path class=\"wb-curve wb-computed-fillet{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\" d=\"{}\"/>"
            ),
            if selected { " selected" } else { "" },
            if hovered { " geometry-hovered" } else { "" },
            if affected {
                " shared-radius-affected"
            } else {
                ""
            },
            if interactive {
                ""
            } else {
                " interaction-disabled"
            },
            if interactive {
                "data-editor-item=\"feature-corner\" "
            } else {
                ""
            },
            curve.owner.feature,
            curve.owner.corner,
            curve.edge.evaluation.raw(),
            curve.edge.ordinal,
            geometry_role_key(curve.role),
            interactive,
            if curve.role == GeometryRole::Construction {
                " construction"
            } else {
                ""
            },
            geometry_role_key(curve.role),
            interactive,
            path,
        );
        if interactive {
            let _ = write!(output, "<path class=\"wb-computed-hit\" d=\"{path}\"/>");
        }
        output.push_str("</g>");
    }
    for curve in scene
        .computed_offset_curves
        .iter()
        .filter(|curve| curve.is_visible(geometry_policy))
    {
        let item = SelectionItem::Feature(curve.owner);
        let selected = selection.contains(&item);
        let provisional = provisional.contains(&item);
        let hovered = geometry_is_hovered(hover, item);
        let interactive = curve.is_interactive(geometry_policy) && !provisional;
        let path = polyline_path(&curve.screen_polyline);
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-computed-item wb-computed-offset-item{}{}{}{}\" {}",
                "data-feature-id=\"{}\" data-computed-evaluation=\"{}\" ",
                "data-computed-edge=\"{}\" data-computed-source=\"{}:{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\"{}>",
                "<path class=\"wb-curve wb-computed-offset{}{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\"{} d=\"{}\"/>"
            ),
            if selected { " selected" } else { "" },
            if hovered { " geometry-hovered" } else { "" },
            if provisional {
                " offset-provisional"
            } else {
                ""
            },
            if interactive {
                ""
            } else {
                " interaction-disabled"
            },
            if interactive {
                "data-editor-item=\"feature\" "
            } else {
                ""
            },
            curve.owner,
            curve.edge.evaluation.raw(),
            curve.edge.ordinal,
            curve.source.span.curve,
            curve.source.span.segment,
            geometry_role_key(curve.role),
            interactive,
            if provisional {
                " data-provisional=\"true\" role=\"img\" aria-label=\"Provisional computed Curve Offset edge\""
            } else {
                ""
            },
            if curve.role == GeometryRole::Construction {
                " construction"
            } else {
                ""
            },
            if provisional {
                " offset-provisional"
            } else {
                ""
            },
            geometry_role_key(curve.role),
            interactive,
            if provisional {
                " data-provisional=\"true\""
            } else {
                ""
            },
            path,
        );
        if interactive {
            let _ = write!(output, "<path class=\"wb-computed-hit\" d=\"{path}\"/>");
        }
        output.push_str("</g>");
    }
    render_fillet_affordances(
        output,
        scene,
        selection,
        active_fillet_preview,
        fillet_action_stamp,
        geometry_policy,
    );
    output.push_str("</g>");
}

pub(crate) fn fillet_action_key(action: SceneFilletActionId) -> String {
    match action {
        SceneFilletActionId::ReverseFirstRetainedDirection => "reverse-first".into(),
        SceneFilletActionId::ReverseSecondRetainedDirection => "reverse-second".into(),
        SceneFilletActionId::ComplementaryArc => "complementary-arc".into(),
        SceneFilletActionId::LocalAlternative { first, second } => format!(
            "local-alternative-{}-{}",
            normal_side_key(first),
            normal_side_key(second),
        ),
    }
}

pub(crate) fn fillet_action_from_key(key: &str) -> Option<SceneFilletActionId> {
    match key {
        "reverse-first" => Some(SceneFilletActionId::ReverseFirstRetainedDirection),
        "reverse-second" => Some(SceneFilletActionId::ReverseSecondRetainedDirection),
        "complementary-arc" => Some(SceneFilletActionId::ComplementaryArc),
        _ => {
            let sides = key.strip_prefix("local-alternative-")?;
            let (first, second) = sides.split_once('-')?;
            Some(SceneFilletActionId::LocalAlternative {
                first: normal_side_from_key(first)?,
                second: normal_side_from_key(second)?,
            })
        }
    }
}

const fn normal_side_key(side: DocumentCurveNormalSide) -> &'static str {
    match side {
        DocumentCurveNormalSide::Left => "left",
        DocumentCurveNormalSide::Right => "right",
    }
}

fn normal_side_from_key(key: &str) -> Option<DocumentCurveNormalSide> {
    match key {
        "left" => Some(DocumentCurveNormalSide::Left),
        "right" => Some(DocumentCurveNormalSide::Right),
        _ => None,
    }
}

fn render_fillet_affordances(
    output: &mut String,
    scene: &EditorScene,
    selection: &[SelectionItem],
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
) {
    output.push_str("<g class=\"wb-fillet-affordances\">");
    for affordances in &scene.fillet_affordances {
        if !fillet_owner_is_visible(affordances.owner, selection) {
            continue;
        }
        if !scene
            .computed_curves
            .iter()
            .find(|curve| curve.owner == affordances.owner)
            .is_some_and(|curve| curve.is_interactive(geometry_policy))
        {
            continue;
        }
        let owner = affordances.owner;
        let rail = affordances.radius_rail;
        // Branch actions paint below the direct radius affordance, so the
        // visible central grip remains the browser target where it truly covers
        // an action. Elsewhere, the headless action resolver verifies the
        // painted arrow before it can outrank an underlying arc or rail.
        for action in &affordances.actions {
            let target = scene.fillet_action_target(owner, action.id);
            render_fillet_canvas_action(
                output,
                affordances,
                action,
                target.as_ref() == active_fillet_preview,
                fillet_action_stamp,
            );
        }
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-fillet-radius-affordance\" data-editor-item=\"feature-corner\" ",
                "data-feature-id=\"{}\" data-feature-corner-id=\"{}\">",
                "<path class=\"wb-fillet-radius-rail\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                "<path class=\"wb-fillet-radius-spoke\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                "<circle class=\"wb-fillet-radius-grip\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"6\" ",
                "role=\"img\" aria-label=\"Drag shared Fillet radius\" ",
                "data-editor-item=\"feature-corner\" data-feature-id=\"{}\" ",
                "data-feature-corner-id=\"{}\"/></g>"
            ),
            owner.feature,
            owner.corner,
            rail.screen_rail_start.x,
            rail.screen_rail_start.y,
            rail.screen_rail_end.x,
            rail.screen_rail_end.y,
            rail.screen_center.x,
            rail.screen_center.y,
            rail.screen_grip.x,
            rail.screen_grip.y,
            rail.screen_grip.x,
            rail.screen_grip.y,
            owner.feature,
            owner.corner,
        );
    }
    output.push_str("</g>");
}

const fn geometry_role_key(role: GeometryRole) -> &'static str {
    match role {
        GeometryRole::Profile => "profile",
        GeometryRole::Construction => "construction",
    }
}

const fn scene_curve_origin_key(origin: SceneCurveOrigin, role: GeometryRole) -> &'static str {
    match origin {
        SceneCurveOrigin::FilletDiscarded { .. } => "implicit",
        SceneCurveOrigin::Native if matches!(role, GeometryRole::Construction) => "explicit",
        SceneCurveOrigin::Native => "profile",
    }
}

fn render_fillet_canvas_action(
    output: &mut String,
    affordances: &SceneFilletCornerAffordances,
    action: &SceneFilletAction,
    previewed: bool,
    fillet_action_stamp: Option<u64>,
) {
    let key = fillet_action_key(action.id);
    let label = escape(&action.label);
    let (availability, disabled, reason) = match &action.availability {
        SceneFilletActionAvailability::Applicable => ("applicable", false, String::new()),
        SceneFilletActionAvailability::Disabled { reason } => ("disabled", true, escape(reason)),
    };
    let anchor = fillet_action_anchor(affordances, action);
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-fillet-action{}{}\" tabindex=\"-1\" role=\"button\" ",
            "aria-label=\"{}\" aria-disabled=\"{}\" data-fillet-action=\"{}\" ",
            "data-fillet-action-input=\"canvas\" ",
            "data-fillet-action-availability=\"{}\" data-feature-id=\"{}\" ",
            "data-feature-corner-id=\"{}\"{}{}>"
        ),
        if disabled { " disabled" } else { "" },
        if previewed { " previewed" } else { "" },
        label,
        disabled,
        key,
        availability,
        action.owner.feature,
        action.owner.corner,
        fillet_action_stamp.map_or_else(String::new, |stamp| {
            format!(" data-fillet-action-stamp=\"{stamp}\"")
        }),
        if reason.is_empty() {
            String::new()
        } else {
            format!(" data-disabled-reason=\"{reason}\"")
        },
    );
    if previewed && let Some(geometry) = &action.dashed_alternative_arc {
        let _ = write!(
            output,
            "<path class=\"wb-fillet-alternative-ghost\" d=\"{}\"/>",
            polyline_path(&geometry.screen_polyline),
        );
    }
    if let Some(control) = action.control_geometry {
        let _ = write!(
            output,
            concat!(
                "<path class=\"wb-fillet-action-hit\" ",
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                "<path class=\"wb-fillet-retained-direction\" marker-end=\"url(#wb-fillet-direction-arrow)\" ",
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                "</g>"
            ),
            control.screen_start.x,
            control.screen_start.y,
            control.screen_end.x,
            control.screen_end.y,
            control.screen_start.x,
            control.screen_start.y,
            control.screen_end.x,
            control.screen_end.y,
        );
    } else {
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-fillet-action-control\" transform=\"translate({:.3} {:.3})\">",
                "<path class=\"wb-fillet-action-hit\" d=\"M-10 0H10\"/>",
                "{}</g></g>"
            ),
            anchor.x,
            anchor.y,
            fillet_action_symbol(action.id),
        );
    }
}

fn fillet_owner_is_visible(
    owner: geosolve_sketch_features::ComputedCornerRef,
    selection: &[SelectionItem],
) -> bool {
    selection.contains(&SelectionItem::FeatureCorner(owner))
        || selection.contains(&SelectionItem::Feature(owner.feature))
}

fn fillet_action_anchor(
    affordances: &SceneFilletCornerAffordances,
    action: &SceneFilletAction,
) -> ScreenPoint {
    if let Some(control) = action.control_geometry {
        return control.screen_end;
    }
    match action.id {
        SceneFilletActionId::ReverseFirstRetainedDirection
        | SceneFilletActionId::ReverseSecondRetainedDirection => {
            affordances.radius_rail.screen_grip
        }
        SceneFilletActionId::ComplementaryArc | SceneFilletActionId::LocalAlternative { .. } => {
            action
                .dashed_alternative_arc
                .as_ref()
                .and_then(|geometry| {
                    geometry
                        .screen_polyline
                        .get(geometry.screen_polyline.len() / 2)
                        .copied()
                })
                .unwrap_or(affordances.radius_rail.screen_grip)
        }
    }
}

fn fillet_action_symbol(action: SceneFilletActionId) -> &'static str {
    match action {
        SceneFilletActionId::ReverseFirstRetainedDirection
        | SceneFilletActionId::ReverseSecondRetainedDirection => {
            "<path d=\"M-4 0H4M-4 0l2-2M-4 0l2 2M4 0 2-2M4 0 2 2\"/>"
        }
        SceneFilletActionId::ComplementaryArc => "<path d=\"M-4 2A5 5 0 0 1 4-2M4-2V2M4-2H0\"/>",
        SceneFilletActionId::LocalAlternative { .. } => "<path d=\"M-4 3Q0-5 4 3M-3-2H3\"/>",
    }
}

pub(crate) fn fillet_action_panel_markup_with_stamp(
    scene: &EditorScene,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
) -> String {
    let mut output = fillet_continuation_status_markup(scene);
    for affordances in scene
        .fillet_affordances
        .iter()
        .filter(|affordances| {
            scene
                .computed_curves
                .iter()
                .find(|curve| curve.owner == affordances.owner)
                .is_some_and(|curve| curve.is_interactive(geometry_policy))
        })
        .filter(|affordances| !affordances.actions.is_empty())
    {
        let owner = affordances.owner;
        let _ = write!(
            output,
            concat!(
                "<div class=\"wb-fillet-action-group\" role=\"group\" ",
                "aria-label=\"Fillet corner {} actions\" data-feature-id=\"{}\" ",
                "data-feature-corner-id=\"{}\"><strong>Fillet corner {}</strong>"
            ),
            owner.corner, owner.feature, owner.corner, owner.corner,
        );
        for action in &affordances.actions {
            let key = fillet_action_key(action.id);
            let label = escape(&action.label);
            let (availability, disabled, described_by, reason_markup) = match &action.availability {
                SceneFilletActionAvailability::Applicable => {
                    ("applicable", String::new(), String::new(), String::new())
                }
                SceneFilletActionAvailability::Disabled { reason } => {
                    let reason_id = format!(
                        "wb-fillet-action-reason-{}-{}-{key}",
                        owner.feature, owner.corner,
                    );
                    (
                        "disabled",
                        " disabled aria-disabled=\"true\"".into(),
                        format!(" aria-describedby=\"{reason_id}\""),
                        format!(
                            "<small id=\"{reason_id}\" class=\"wb-fillet-action-reason\">Unavailable: {}</small>",
                            escape(reason),
                        ),
                    )
                }
            };
            let _ = write!(
                output,
                concat!(
                    "<button type=\"button\" data-fillet-action=\"{}\" ",
                    "data-fillet-action-input=\"accessible\" ",
                    "data-fillet-action-availability=\"{}\" data-feature-id=\"{}\" ",
                    "data-feature-corner-id=\"{}\"{}{}{}>{}</button>{}"
                ),
                key,
                availability,
                owner.feature,
                owner.corner,
                fillet_action_stamp.map_or_else(String::new, |stamp| {
                    format!(" data-fillet-action-stamp=\"{stamp}\"")
                }),
                disabled,
                described_by,
                label,
                reason_markup,
            );
        }
        output.push_str("</div>");
    }
    output
}

pub(crate) fn fillet_continuation_status_markup(scene: &EditorScene) -> String {
    let mut output = String::new();
    for status in &scene.computed_fillet_continuation_statuses {
        let (kind, label) = continuation_limit_presentation(status.limit.kind);
        let _ = write!(
            output,
            concat!(
                "<p class=\"wb-fillet-continuation-limit\" role=\"status\" ",
                "data-fillet-limit=\"{}\" data-feature-id=\"{}\" ",
                "data-feature-corner-id=\"{}\"><strong>{}:</strong> {}</p>"
            ),
            kind,
            status.owner.feature,
            status.owner.corner,
            label,
            escape(&status.limit.message),
        );
    }
    output
}

const fn continuation_limit_presentation(
    kind: ComputedFilletContinuationLimitKind,
) -> (&'static str, &'static str) {
    match kind {
        ComputedFilletContinuationLimitKind::BranchFold => ("branch-fold", "Branch fold"),
        ComputedFilletContinuationLimitKind::DomainBoundary => ("domain-boundary", "Parent limit"),
        ComputedFilletContinuationLimitKind::OffsetSingularity => {
            ("offset-singularity", "Offset singularity")
        }
        ComputedFilletContinuationLimitKind::LossOfRegularity => {
            ("loss-of-regularity", "Regularity limit")
        }
        ComputedFilletContinuationLimitKind::AmbiguousLocalRoot => {
            ("ambiguous-local-root", "Ambiguous local branch")
        }
        ComputedFilletContinuationLimitKind::WorkStopped => {
            ("work-stopped", "Continuation stopped")
        }
    }
}

fn digest(bytes: [u8; 32]) -> String {
    let mut output = String::new();
    for byte in &bytes[..6] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn render_annotations(
    output: &mut String,
    problem_markers: &mut String,
    resolved_targets: &mut BTreeSet<EditorProblemTarget>,
    scene: &EditorScene,
    accepted: &SketchAcceptedDocumentState,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    problem_items: &[SelectionItem],
    problem: Option<&EditorProblemMetadata>,
) {
    let visibility_context = hover
        .context_owner
        .or_else(|| hover.target.map(EditorHoverTarget::item));
    for annotation in &scene.annotations {
        if !(annotation.is_visible(selection, visibility_context, problem_items)
            || scene.show_all_constraint_annotations
                && matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
        {
            continue;
        }
        let selected = selection.contains(&annotation.item);
        let pending = pending.contains(&annotation.item);
        let provisional = provisional.contains(&annotation.item);
        let hovered_occurrence = match hover.target {
            Some(EditorHoverTarget::Annotation(occurrence))
                if occurrence.item == annotation.item =>
            {
                Some(occurrence)
            }
            Some(
                EditorHoverTarget::CurveControl { .. }
                | EditorHoverTarget::Geometry(_)
                | EditorHoverTarget::Annotation(_),
            )
            | None => None,
        };
        let is_hovered =
            hovered_occurrence.is_some_and(|occurrence| occurrence.marker_index.is_none());
        let has_problem = problem_items.contains(&annotation.item);
        let class = format!(
            "{}{}{}{}{}{}{}{}",
            if selected { " selected" } else { "" },
            if is_hovered { " hovered" } else { "" },
            if has_problem { " has-problem" } else { "" },
            if annotation.suppressed {
                " suppressed"
            } else {
                ""
            },
            if annotation.reference {
                " reference"
            } else {
                ""
            },
            if annotation.is_movable() {
                " movable"
            } else {
                ""
            },
            if pending { " authoring-pending" } else { "" },
            if provisional {
                " offset-provisional"
            } else {
                ""
            },
        );
        let (editor_kind, id, kind, _label, value, mode) = match annotation.item {
            SelectionItem::Constraint(id) => {
                let constraint = scene
                    .constraint_entries
                    .iter()
                    .find(|entry| entry.id == id && entry.source == annotation.source);
                let label = constraint.map_or_else(
                    || {
                        accepted
                            .document()
                            .constraint(id)
                            .filter(|constraint| constraint.source_id == annotation.source)
                            .map_or_else(
                                || "Accepted constraint".into(),
                                |constraint| constraint.label.clone(),
                            )
                    },
                    |constraint| constraint.label.clone(),
                );
                (
                    "constraint",
                    id.to_string(),
                    annotation_kind(annotation.kind),
                    label,
                    String::new(),
                    String::new(),
                )
            }
            SelectionItem::Dimension(id) => {
                let Some(dimension) = accepted.document().dimension(id) else {
                    continue;
                };
                let (value_attribute, displayed) = dimension_display(accepted, dimension);
                (
                    "dimension",
                    id.to_string(),
                    annotation_kind(annotation.kind),
                    format!("{} = {displayed}", dimension.label),
                    value_attribute,
                    match dimension.mode {
                        DocumentDimensionMode::Driving => "driving".into(),
                        DocumentDimensionMode::Reference => "reference".into(),
                    },
                )
            }
            SelectionItem::Point(_)
            | SelectionItem::Curve(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => continue,
        };
        let escaped_label = escape(&annotation.accessible_label);
        let visible_text = annotation.visible_text.as_deref().map(escape);
        let (identity, accessibility) = if provisional {
            (
                String::new(),
                "tabindex=\"-1\" role=\"img\" data-provisional=\"true\"".to_owned(),
            )
        } else {
            (
                format!("data-editor-item=\"{editor_kind}\" data-persistent-id=\"{id}\" "),
                "tabindex=\"0\" role=\"button\"".to_owned(),
            )
        };
        let _ = write!(
            output,
            "<g class=\"wb-annotation wb-{editor_kind}{class}\" aria-label=\"{escaped_label}\" {identity}data-{editor_kind}-kind=\"{kind}\"{}{} data-annotation-kind=\"{kind}\" {accessibility}>",
            if mode.is_empty() {
                String::new()
            } else {
                format!(" data-dimension-mode=\"{mode}\"")
            },
            if value.is_empty() {
                String::new()
            } else {
                format!(" data-dimension-value=\"{value}\"")
            },
        );
        let _ = write!(output, "<title>{escaped_label}</title>");
        annotation_geometry(
            output,
            annotation.kind,
            &annotation.geometry,
            visible_text.as_deref().unwrap_or(""),
            annotation.label_bounds,
            hovered_occurrence.and_then(|occurrence| occurrence.marker_index),
        );
        output.push_str("</g>");

        if has_problem {
            let target = match annotation.item {
                SelectionItem::Constraint(id) => EditorProblemTarget::Constraint(id),
                SelectionItem::Dimension(id) => EditorProblemTarget::Dimension(id),
                SelectionItem::Point(_)
                | SelectionItem::Curve(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => unreachable!(),
            };
            if resolved_targets.insert(target)
                && let Some(anchor) = annotation_anchor(&annotation.geometry)
            {
                problem_marker(
                    problem_markers,
                    anchor,
                    Some(target),
                    problem.expect("targeted marker has problem metadata"),
                    false,
                );
            }
        }
    }
}

fn dimension_display(
    accepted: &SketchAcceptedDocumentState,
    dimension: &geosolve_sketch::DocumentDimension,
) -> (String, String) {
    let document = accepted.document();
    let stored_value = match dimension.mode {
        DocumentDimensionMode::Driving => document
            .scalar(dimension_target(&dimension.definition))
            .map(|scalar| scalar.value),
        DocumentDimensionMode::Reference => accepted.reference_value(dimension.id),
    }
    .filter(|value| value.is_finite());
    let unit = if matches!(
        &dimension.definition,
        DocumentDimensionDefinition::OrientedAngle { .. }
    ) {
        ScalarUnit::Angle
    } else {
        ScalarUnit::Length
    };
    let display = stored_value.and_then(|value| display_dimension_target(value, unit));
    (
        display.map_or_else(String::new, |display| display.value.to_string()),
        display.map_or_else(
            || "unavailable".into(),
            |display| match display.unit {
                DimensionTargetDisplayUnit::ModelUnits => format!("{:.3}", display.value),
                DimensionTargetDisplayUnit::AcuteDegrees => format!("{:.3}°", display.value),
            },
        ),
    )
}

#[allow(clippy::too_many_lines)]
fn annotation_geometry(
    output: &mut String,
    kind: SceneAnnotationKind,
    geometry: &SceneAnnotationGeometry,
    visible_text: &str,
    label_bounds: Option<geosolve_constraint_editor::SceneAnnotationLabelBounds>,
    hovered_marker: Option<usize>,
) {
    let arrowheads = annotation_arrowheads(geometry);
    match geometry {
        SceneAnnotationGeometry::Glyph { markers } => {
            let SceneAnnotationKind::Constraint(glyph) = kind else {
                return;
            };
            for (index, marker) in markers.iter().enumerate() {
                if let Some(origin) = marker.leader_from {
                    let _ = write!(
                        output,
                        concat!(
                            "<path class=\"wb-annotation-leader\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                            "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>"
                        ),
                        origin.x,
                        origin.y,
                        marker.anchor.x,
                        marker.anchor.y,
                        origin.x,
                        origin.y,
                        marker.anchor.x,
                        marker.anchor.y,
                    );
                }
                let _ = write!(
                    output,
                    "<g class=\"wb-constraint-symbol{}\" transform=\"translate({:.3} {:.3}) rotate({:.3})\" data-annotation-marker=\"{index}\" data-marker-rotation-radians=\"{:.6}\"><circle class=\"wb-annotation-hit\" r=\"{:.3}\"/>{}</g>",
                    if hovered_marker == Some(index) {
                        " hovered"
                    } else {
                        ""
                    },
                    marker.anchor.x,
                    marker.anchor.y,
                    marker.rotation_radians.to_degrees(),
                    marker.rotation_radians,
                    marker.bounds().radius,
                    super::icons::constraint_icon_fragment(glyph),
                );
            }
        }
        SceneAnnotationGeometry::RightAngle {
            first_arm,
            corner,
            second_arm,
            ..
        } => {
            let _ = write!(
                output,
                concat!(
                    "<g class=\"wb-constraint-symbol\">",
                    "<path class=\"wb-right-angle\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "</g>"
                ),
                first_arm.x,
                first_arm.y,
                corner.x,
                corner.y,
                second_arm.x,
                second_arm.y,
                first_arm.x,
                first_arm.y,
                corner.x,
                corner.y,
                second_arm.x,
                second_arm.y,
            );
        }
        SceneAnnotationGeometry::LinearDimension {
            measured_first,
            measured_second,
            first,
            second,
            label_anchor,
        } => {
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-dimension-witness\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "M{:.3} {:.3}L{:.3} {:.3}M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "{}{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                measured_first.x,
                measured_first.y,
                first.x,
                first.y,
                measured_second.x,
                measured_second.y,
                second.x,
                second.y,
                first.x,
                first.y,
                second.x,
                second.y,
                measured_first.x,
                measured_first.y,
                first.x,
                first.y,
                measured_second.x,
                measured_second.y,
                second.x,
                second.y,
                first.x,
                first.y,
                second.x,
                second.y,
                label_hit_regions(*label_anchor, label_bounds),
                arrowheads,
                label_mask(label_bounds),
                label_anchor.x,
                label_anchor.y + 4.0,
                visible_text,
            );
        }
        SceneAnnotationGeometry::RadialDimension {
            center,
            edge,
            label_anchor,
            diameter,
            full_circle,
        } => {
            let measurement_start = if *diameter && *full_circle {
                ScreenPoint {
                    x: center.x.mul_add(2.0, -edge.x),
                    y: center.y.mul_add(2.0, -edge.y),
                }
            } else {
                *center
            };
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "L{:.3} {:.3}\"/>",
                    "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "L{:.3} {:.3}\"/>",
                    "{}{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                measurement_start.x,
                measurement_start.y,
                edge.x,
                edge.y,
                label_anchor.x,
                label_anchor.y,
                measurement_start.x,
                measurement_start.y,
                edge.x,
                edge.y,
                label_anchor.x,
                label_anchor.y,
                label_hit_regions(*label_anchor, label_bounds),
                arrowheads,
                label_mask(label_bounds),
                label_anchor.x,
                label_anchor.y + 4.0,
                visible_text,
            );
        }
        SceneAnnotationGeometry::AngularDimension {
            vertex,
            first_ray,
            second_ray,
            radius,
            clockwise,
            label_anchor,
        } => {
            let first_arc = ray_point(*vertex, *first_ray, *radius);
            let second_arc = ray_point(*vertex, *second_ray, *radius);
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-dimension-witness\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<path class=\"wb-angle-arc\" d=\"M{:.3} {:.3}A{:.3} {:.3} 0 0 {} {:.3} {:.3}\"/>",
                    "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "M{:.3} {:.3}L{:.3} {:.3}M{:.3} {:.3}",
                    "A{:.3} {:.3} 0 0 {} {:.3} {:.3}\"/>",
                    "{}{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                vertex.x,
                vertex.y,
                first_ray.x,
                first_ray.y,
                vertex.x,
                vertex.y,
                second_ray.x,
                second_ray.y,
                first_arc.x,
                first_arc.y,
                radius,
                radius,
                u8::from(*clockwise),
                second_arc.x,
                second_arc.y,
                vertex.x,
                vertex.y,
                first_ray.x,
                first_ray.y,
                vertex.x,
                vertex.y,
                second_ray.x,
                second_ray.y,
                first_arc.x,
                first_arc.y,
                radius,
                radius,
                u8::from(*clockwise),
                second_arc.x,
                second_arc.y,
                label_hit_regions(*label_anchor, label_bounds),
                arrowheads,
                label_mask(label_bounds),
                label_anchor.x,
                label_anchor.y + 4.0,
                visible_text,
            );
        }
        SceneAnnotationGeometry::Label {
            anchor,
            leader_from,
        } => {
            if let Some(origin) = leader_from {
                let _ = write!(
                    output,
                    concat!(
                        "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                        "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>"
                    ),
                    origin.x, origin.y, anchor.x, anchor.y, origin.x, origin.y, anchor.x, anchor.y,
                );
            }
            let _ = write!(
                output,
                "{}{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>",
                label_hit_regions(*anchor, label_bounds),
                arrowheads,
                label_mask(label_bounds),
                anchor.x,
                anchor.y + 4.0,
                visible_text,
            );
        }
    }
}

fn annotation_arrowheads(geometry: &SceneAnnotationGeometry) -> String {
    let mut output = String::new();
    for arrow in geometry.arrowheads() {
        let _ = write!(
            output,
            "<path class=\"wb-dimension-arrow\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}Z\"/>",
            arrow.tip.x,
            arrow.tip.y,
            arrow.base_first.x,
            arrow.base_first.y,
            arrow.base_second.x,
            arrow.base_second.y,
        );
    }
    output
}

fn label_mask(bounds: Option<geosolve_constraint_editor::SceneAnnotationLabelBounds>) -> String {
    bounds.map_or_else(String::new, |bounds| {
        format!(
            "<rect class=\"wb-dimension-label-mask\" x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"3\"/>",
            bounds.min.x,
            bounds.min.y,
            bounds.max.x - bounds.min.x,
            bounds.max.y - bounds.min.y,
        )
    })
}

fn label_hit_regions(
    anchor: ScreenPoint,
    bounds: Option<geosolve_constraint_editor::SceneAnnotationLabelBounds>,
) -> String {
    const PICK_TOLERANCE_PIXELS: f64 = 10.0;
    const MOVE_TOLERANCE_PIXELS: f64 = 2.0;
    bounds.map_or_else(
        || {
            format!(
                concat!(
                    "<circle class=\"wb-annotation-hit wb-annotation-label-hit\" ",
                    "cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"/>",
                    "<circle class=\"wb-annotation-hit wb-annotation-move-hit\" ",
                    "cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"/>"
                ),
                anchor.x,
                anchor.y,
                PICK_TOLERANCE_PIXELS,
                anchor.x,
                anchor.y,
                MOVE_TOLERANCE_PIXELS,
            )
        },
        |bounds| {
            let outer_min_x = bounds.min.x - PICK_TOLERANCE_PIXELS;
            let outer_min_y = bounds.min.y - PICK_TOLERANCE_PIXELS;
            let outer_width = bounds.max.x - bounds.min.x + 2.0 * PICK_TOLERANCE_PIXELS;
            let outer_height = bounds.max.y - bounds.min.y + 2.0 * PICK_TOLERANCE_PIXELS;
            let inner_min_x = bounds.min.x - MOVE_TOLERANCE_PIXELS;
            let inner_min_y = bounds.min.y - MOVE_TOLERANCE_PIXELS;
            let inner_width = bounds.max.x - bounds.min.x + 2.0 * MOVE_TOLERANCE_PIXELS;
            let inner_height = bounds.max.y - bounds.min.y + 2.0 * MOVE_TOLERANCE_PIXELS;
            format!(
                concat!(
                    "<rect class=\"wb-annotation-hit wb-annotation-label-hit\" ",
                    "x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"3\"/>",
                    "<rect class=\"wb-annotation-hit wb-annotation-move-hit\" ",
                    "x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"3\"/>"
                ),
                outer_min_x,
                outer_min_y,
                outer_width,
                outer_height,
                inner_min_x,
                inner_min_y,
                inner_width,
                inner_height,
            )
        },
    )
}

const fn annotation_kind(kind: SceneAnnotationKind) -> &'static str {
    match kind {
        SceneAnnotationKind::Constraint(glyph) => super::icons::constraint_icon_key(glyph),
        SceneAnnotationKind::PointDistance => "point-distance",
        SceneAnnotationKind::CurveLength => "segment-length",
        SceneAnnotationKind::Radius => "radius",
        SceneAnnotationKind::Diameter => "diameter",
        SceneAnnotationKind::OrientedAngle => "oriented-angle",
        SceneAnnotationKind::SupportingLineOffset => "supporting-line-offset",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => "translated-segment-offset",
        SceneAnnotationKind::ProfileOffset => "profile-offset",
    }
}

fn annotation_anchor(geometry: &SceneAnnotationGeometry) -> Option<ScreenPoint> {
    Some(match geometry {
        SceneAnnotationGeometry::Glyph { markers } => markers.first()?.anchor,
        SceneAnnotationGeometry::RightAngle { corner, .. } => *corner,
        SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
        | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
        | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => *label_anchor,
        SceneAnnotationGeometry::Label { anchor, .. } => *anchor,
    })
}

fn ray_point(vertex: ScreenPoint, ray: ScreenPoint, radius: f64) -> ScreenPoint {
    let delta = [ray.x - vertex.x, ray.y - vertex.y];
    let length = delta[0].hypot(delta[1]);
    if length <= f64::EPSILON {
        return vertex;
    }
    ScreenPoint {
        x: vertex.x + delta[0] * radius / length,
        y: vertex.y + delta[1] * radius / length,
    }
}

pub(crate) fn problem_selection_item(
    target: EditorProblemTarget,
    scene: Option<&EditorScene>,
) -> Option<SelectionItem> {
    Some(match target {
        EditorProblemTarget::Point(id) => SelectionItem::Point(id),
        EditorProblemTarget::Curve(id) => SelectionItem::Curve(
            scene?
                .curves
                .iter()
                .find(|curve| curve.span.curve == id)?
                .span,
        ),
        EditorProblemTarget::Constraint(id) => SelectionItem::Constraint(id),
        EditorProblemTarget::Dimension(id) => SelectionItem::Dimension(id),
    })
}

#[cfg(test)]
const fn constraint_glyph(
    definition: &DocumentConstraintDefinition,
) -> (&'static str, &'static str) {
    match definition {
        DocumentConstraintDefinition::FixedPoint { .. }
        | DocumentConstraintDefinition::FixedCoordinate { .. } => ("fixed", "Fix"),
        DocumentConstraintDefinition::Coincident { .. }
        | DocumentConstraintDefinition::CoincidentWithOrigin { .. }
        | DocumentConstraintDefinition::ExternalPointCoincident { .. } => ("coincident", "Coin"),
        DocumentConstraintDefinition::Horizontal { .. }
        | DocumentConstraintDefinition::HorizontalPoints { .. }
        | DocumentConstraintDefinition::HorizontalPointToMidpoint { .. } => ("horizontal", "H"),
        DocumentConstraintDefinition::Vertical { .. }
        | DocumentConstraintDefinition::VerticalPoints { .. }
        | DocumentConstraintDefinition::VerticalPointToMidpoint { .. } => ("vertical", "V"),
        DocumentConstraintDefinition::PointOnCurve { .. }
        | DocumentConstraintDefinition::PointOnDatumAxis { .. } => ("point-on-curve", "On"),
        DocumentConstraintDefinition::Parallel { .. } => ("parallel", "∥"),
        DocumentConstraintDefinition::Perpendicular { .. } => ("perpendicular", "⊥"),
        DocumentConstraintDefinition::ExternalLineCollinear { .. }
        | DocumentConstraintDefinition::Collinear { .. }
        | DocumentConstraintDefinition::CollinearWithDatumAxis { .. } => ("collinear", "Col"),
        DocumentConstraintDefinition::Concentric { .. } => ("concentric", "Con"),
        DocumentConstraintDefinition::EqualLength { .. } => ("equal-length", "L="),
        DocumentConstraintDefinition::EqualRadius { .. } => ("equal-radius", "R="),
        DocumentConstraintDefinition::Midpoint { .. } => ("midpoint", "Mid"),
        DocumentConstraintDefinition::SymmetricAboutLine { .. }
        | DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. } => ("symmetry", "Sym"),
        DocumentConstraintDefinition::CurveCurveContact { .. } => ("generic-contact", "Touch"),
        DocumentConstraintDefinition::CurveCurveTangency { .. } => ("generic-tangency", "Tan"),
        DocumentConstraintDefinition::LineCircleTangency { .. }
        | DocumentConstraintDefinition::CircleCircleTangency { .. }
        | DocumentConstraintDefinition::CircleArcTangency { .. }
        | DocumentConstraintDefinition::LineCurveTangency { .. } => ("tangency", "Tan"),
        DocumentConstraintDefinition::CurveDirection { .. } => ("curve-direction", "Dir"),
        DocumentConstraintDefinition::EqualCurvature { .. } => ("equal-curvature", "K="),
        DocumentConstraintDefinition::EndpointContinuity { .. } => ("continuity", "G"),
        DocumentConstraintDefinition::LineLineFillet { .. }
        | DocumentConstraintDefinition::CurveCurveFillet { .. } => ("fillet", "Fil"),
    }
}

#[cfg(test)]
const fn dimension_kind(definition: &DocumentDimensionDefinition) -> &'static str {
    match definition {
        DocumentDimensionDefinition::PointDistance { .. } => "point-distance",
        DocumentDimensionDefinition::CurveLength { .. } => "segment-length",
        DocumentDimensionDefinition::Radius { .. } => "radius",
        DocumentDimensionDefinition::Diameter { .. } => "diameter",
        DocumentDimensionDefinition::OrientedAngle { .. } => "oriented-angle",
        DocumentDimensionDefinition::SupportingLineOffset { .. } => "supporting-line-offset",
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset { .. } => {
            "translated-segment-offset"
        }
        DocumentDimensionDefinition::ProfileOffset { .. } => "profile-offset",
    }
}

fn problem_marker(
    output: &mut String,
    anchor: ScreenPoint,
    target: Option<EditorProblemTarget>,
    problem: &EditorProblemMetadata,
    global: bool,
) {
    let message = escape(&problem.message);
    let target_key = if global {
        "global".to_owned()
    } else {
        match target.expect("targeted error marker must have a persistent target") {
            EditorProblemTarget::Point(id) => format!("point:{id}"),
            EditorProblemTarget::Curve(id) => format!("curve:{id}"),
            EditorProblemTarget::Constraint(id) => format!("constraint:{id}"),
            EditorProblemTarget::Dimension(id) => format!("dimension:{id}"),
        }
    };
    let tooltip_x = if global || anchor.x > 610.0 {
        -370.0
    } else {
        14.0
    };
    let tooltip_y = if anchor.y > 610.0 { -82.0 } else { 14.0 };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-error-marker{}\" transform=\"translate({:.3} {:.3})\" ",
            "tabindex=\"0\" role=\"img\" aria-label=\"{}\" data-problem-marker=\"{}\">",
            "<circle r=\"10\"/>{}",
            "<foreignObject class=\"wb-error-tooltip\" x=\"{}\" y=\"{}\" width=\"360\" height=\"72\">",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\">{}</div></foreignObject></g>"
        ),
        if global { " global" } else { "" },
        anchor.x,
        anchor.y,
        message,
        target_key,
        super::icons::PROBLEM_ICON,
        tooltip_x,
        tooltip_y,
        message,
    );
}

const fn problem_category_key(category: EditorProblemCategory) -> &'static str {
    match category {
        EditorProblemCategory::Input => "input",
        EditorProblemCategory::Lowering => "lowering",
        EditorProblemCategory::Solver => "solver",
        EditorProblemCategory::Validation => "validation",
        EditorProblemCategory::Geometry => "geometry",
        EditorProblemCategory::Constraint => "constraint",
        EditorProblemCategory::Dimension => "dimension",
        EditorProblemCategory::Bound => "bound",
        EditorProblemCategory::Publication => "publication",
    }
}

fn dimension_target(definition: &DocumentDimensionDefinition) -> DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. }
        | DocumentDimensionDefinition::ProfileOffset { target, .. } => *target,
    }
}

fn polyline_path(points: &[ScreenPoint]) -> String {
    let mut path = String::new();
    for (index, point) in points.iter().enumerate() {
        let _ = write!(
            path,
            "{} {:.3} {:.3} ",
            if index == 0 { 'M' } else { 'L' },
            point.x,
            point.y,
        );
    }
    path
}

fn construction_markup(preview: &ConstructionPreview, viewport: Viewport) -> String {
    let mut output = String::from("<g class=\"wb-draft\">");
    match preview {
        ConstructionPreview::Complete { geometry, .. } => {
            construction_geometry_markup(&mut output, geometry, viewport);
        }
        ConstructionPreview::Anchor { position } => {
            marker(&mut output, viewport, *position, "wb-draft-center");
        }
        ConstructionPreview::ArcRadiusGuide { center, start } => {
            line(&mut output, viewport, &[*center, *start]);
            marker(&mut output, viewport, *center, "wb-draft-center");
            marker(&mut output, viewport, *start, "wb-draft-start");
        }
        ConstructionPreview::GuidePolyline { points, closed } => {
            let mut display_points = points.clone();
            if *closed && points.len() >= 3 {
                display_points.push(points[0]);
            }
            let display_points = display_points
                .into_iter()
                .map(|point| viewport.model_to_screen(point))
                .collect::<Vec<_>>();
            if display_points.len() >= 2 {
                let _ = write!(output, "<path d=\"{}\"/>", polyline_path(&display_points));
            }
            for &point in points {
                marker(&mut output, viewport, point, "wb-draft-point");
            }
        }
        ConstructionPreview::EllipticalArcSupport {
            center,
            major_axis_point,
            support_points,
            trim_start,
        } => elliptical_arc_support_markup(
            &mut output,
            viewport,
            *center,
            *major_axis_point,
            support_points,
            *trim_start,
        ),
        ConstructionPreview::ControlPolygon { kind, points } => {
            advanced_control_polygon(&mut output, viewport, *kind, points);
        }
    }
    output.push_str("</g>");
    output
}

fn render_inference_guides(
    output: &mut String,
    resolution: &DraftInferenceResolution,
    viewport: Viewport,
) {
    let _ = write!(
        output,
        "<g class=\"wb-inference-guides\" data-inference-status=\"{}\" pointer-events=\"none\">",
        inference_status_key(&resolution.status),
    );
    for guide in &resolution.guides {
        render_inference_guide(output, *guide, viewport);
    }
    output.push_str("</g>");
}

fn render_inference_guide(output: &mut String, guide: DraftGuide, viewport: Viewport) {
    let family = inference_family_key(guide.family);
    let label = inference_family_label(guide.family);
    let classification = match guide.classification {
        DraftGuideClassification::ConstraintBacked => "constraint-backed",
        DraftGuideClassification::TrackingOnly => "tracking-only",
    };
    let candidate = guide
        .id
        .candidate
        .map_or_else(|| "tracking".to_owned(), |id| id.get().to_string());
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-inference-guide {}\" data-inference-family=\"{}\" ",
            "data-inference-classification=\"{}\" data-inference-candidate=\"{}\" ",
            "data-inference-guide-ordinal=\"{}\" role=\"img\" aria-label=\"{}\"><title>{}</title>"
        ),
        classification,
        family,
        classification,
        candidate,
        guide.id.ordinal,
        escape(label),
        escape(label),
    );
    match guide.geometry {
        DraftGuideGeometry::Point { position } => {
            let point = viewport.model_to_screen(position);
            let _ = write!(
                output,
                "<circle class=\"wb-inference-guide-point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"7\"/>",
                point.x, point.y,
            );
        }
        DraftGuideGeometry::Segment { start, end } => {
            let start = viewport.model_to_screen(start);
            let end = viewport.model_to_screen(end);
            let _ = write!(
                output,
                "<path class=\"wb-inference-guide-segment\" d=\"M {:.3} {:.3} L {:.3} {:.3}\"/>",
                start.x, start.y, end.x, end.y,
            );
        }
    }
    output.push_str("</g>");
}

fn render_inference_candidates(output: &mut String, resolution: &DraftInferenceResolution) {
    let candidate_ids = match &resolution.status {
        DraftInferenceStatus::Resolved { candidate } => vec![*candidate],
        DraftInferenceStatus::Ambiguous { candidates } => candidates.clone(),
        DraftInferenceStatus::None
        | DraftInferenceStatus::Suppressed
        | DraftInferenceStatus::ResourceLimited
        | DraftInferenceStatus::StalePreferredCandidate { .. } => Vec::new(),
    };
    let ambiguous = matches!(&resolution.status, DraftInferenceStatus::Ambiguous { .. });
    if !candidate_ids.is_empty() {
        let _ = write!(
            output,
            "<g class=\"wb-inference-candidates{}\" data-inference-candidate-count=\"{}\" pointer-events=\"none\">",
            if ambiguous { " ambiguous" } else { "" },
            candidate_ids.len(),
        );
        let count = f64::from(u32::try_from(candidate_ids.len()).unwrap_or(u32::MAX));
        for (candidate_index, candidate_id) in candidate_ids.into_iter().enumerate() {
            let Some(candidate) = resolution
                .candidates
                .iter()
                .find(|candidate| candidate.id == candidate_id)
            else {
                continue;
            };
            let index = f64::from(u32::try_from(candidate_index).unwrap_or(u32::MAX));
            let candidate_offset = (index - (count - 1.0) * 0.5) * 24.0;
            for (relation_index, relation) in candidate.relations.iter().copied().enumerate() {
                let (key, label, glyph) = inference_relation_presentation(relation);
                let relation_offset =
                    f64::from(u32::try_from(relation_index).unwrap_or(u32::MAX)) * 22.0;
                let x = candidate.adjusted_screen_position.x + 16.0 + relation_offset;
                let y = candidate.adjusted_screen_position.y - 16.0 + candidate_offset;
                let _ = write!(
                    output,
                    concat!(
                        "<g class=\"wb-inference-glyph\" transform=\"translate({:.3} {:.3})\" ",
                        "data-inference-candidate=\"{}\" data-inference-relation=\"{}\" ",
                        "role=\"img\" aria-label=\"{}\"><title>{}</title>",
                        "<circle class=\"wb-inference-glyph-background\" r=\"10\"/>",
                        "<g class=\"wb-inference-glyph-symbol\">{}</g></g>"
                    ),
                    x,
                    y,
                    candidate.id.get(),
                    key,
                    escape(label),
                    escape(label),
                    super::icons::constraint_icon_fragment(glyph),
                );
            }
        }
        output.push_str("</g>");
    }
    if let Some((key, message)) = inference_status_warning(&resolution.status) {
        let message = escape(message);
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-inference-state\" data-inference-status=\"{}\" ",
                "transform=\"translate(18 18)\" role=\"status\" aria-label=\"{}\">",
                "<rect width=\"310\" height=\"30\" rx=\"5\"/><text x=\"12\" y=\"20\">{}</text></g>"
            ),
            key, message, message,
        );
    }
}

const fn inference_family_key(family: DraftInferenceFamily) -> &'static str {
    match family {
        DraftInferenceFamily::PointIdentity => "point-identity",
        DraftInferenceFamily::DatumOrigin => "datum-origin",
        DraftInferenceFamily::DatumAxis => "datum-axis",
        DraftInferenceFamily::PointOnCurve => "point-on-curve",
        DraftInferenceFamily::PointOnCreatedCurve => "point-on-created-curve",
        DraftInferenceFamily::Midpoint => "midpoint",
        DraftInferenceFamily::Horizontal => "horizontal",
        DraftInferenceFamily::Vertical => "vertical",
        DraftInferenceFamily::Parallel => "parallel",
        DraftInferenceFamily::Perpendicular => "perpendicular",
        DraftInferenceFamily::HorizontalPoints => "horizontal-points",
        DraftInferenceFamily::VerticalPoints => "vertical-points",
        DraftInferenceFamily::HorizontalPointToMidpoint => "horizontal-point-to-midpoint",
        DraftInferenceFamily::VerticalPointToMidpoint => "vertical-point-to-midpoint",
        DraftInferenceFamily::Concentric => "concentric",
        DraftInferenceFamily::Collinear => "collinear",
        DraftInferenceFamily::PointTracking => "point-tracking",
    }
}

const fn inference_family_label(family: DraftInferenceFamily) -> &'static str {
    match family {
        DraftInferenceFamily::PointIdentity => "Reuse existing point",
        DraftInferenceFamily::DatumOrigin => "Coincident with Origin",
        DraftInferenceFamily::DatumAxis => "Point on datum axis",
        DraftInferenceFamily::PointOnCurve => "Point on curve",
        DraftInferenceFamily::PointOnCreatedCurve => "Circle through point",
        DraftInferenceFamily::Midpoint => "Midpoint",
        DraftInferenceFamily::Horizontal => "Horizontal",
        DraftInferenceFamily::Vertical => "Vertical",
        DraftInferenceFamily::Parallel => "Parallel",
        DraftInferenceFamily::Perpendicular => "Perpendicular",
        DraftInferenceFamily::HorizontalPoints => "Horizontal points",
        DraftInferenceFamily::VerticalPoints => "Vertical points",
        DraftInferenceFamily::HorizontalPointToMidpoint => "Horizontal to midpoint",
        DraftInferenceFamily::VerticalPointToMidpoint => "Vertical to midpoint",
        DraftInferenceFamily::Concentric => "Concentric",
        DraftInferenceFamily::Collinear => "Collinear",
        DraftInferenceFamily::PointTracking => "Alignment tracking only",
    }
}

const fn inference_relation_presentation(
    relation: DraftInferenceRelation,
) -> (&'static str, &'static str, SceneConstraintGlyph) {
    match relation {
        DraftInferenceRelation::PointIdentity { .. } => (
            "point-identity",
            "Reuse existing point",
            SceneConstraintGlyph::Coincident,
        ),
        DraftInferenceRelation::CoincidentWithOrigin => (
            "coincident-with-origin",
            "Coincident with Origin",
            SceneConstraintGlyph::Coincident,
        ),
        DraftInferenceRelation::PointOnDatumAxis { axis } => match axis {
            geosolve_sketch::DocumentCoordinateAxis::X => (
                "point-on-x-axis",
                "Point on X axis",
                SceneConstraintGlyph::Horizontal,
            ),
            geosolve_sketch::DocumentCoordinateAxis::Y => (
                "point-on-y-axis",
                "Point on Y axis",
                SceneConstraintGlyph::Vertical,
            ),
        },
        DraftInferenceRelation::PointOnCurve { .. } => (
            "point-on-curve",
            "Point on curve",
            SceneConstraintGlyph::PointOnCurve,
        ),
        DraftInferenceRelation::PointOnCreatedCurve { .. } => (
            "point-on-created-curve",
            "Circle through point",
            SceneConstraintGlyph::PointOnCurve,
        ),
        DraftInferenceRelation::Midpoint { .. } => {
            ("midpoint", "Midpoint", SceneConstraintGlyph::Midpoint)
        }
        DraftInferenceRelation::Horizontal => {
            ("horizontal", "Horizontal", SceneConstraintGlyph::Horizontal)
        }
        DraftInferenceRelation::Vertical => {
            ("vertical", "Vertical", SceneConstraintGlyph::Vertical)
        }
        DraftInferenceRelation::Parallel { .. } => {
            ("parallel", "Parallel", SceneConstraintGlyph::Parallel)
        }
        DraftInferenceRelation::Perpendicular { .. } => (
            "perpendicular",
            "Perpendicular",
            SceneConstraintGlyph::Perpendicular,
        ),
        DraftInferenceRelation::HorizontalPoints { .. } => (
            "horizontal-points",
            "Horizontal points",
            SceneConstraintGlyph::Horizontal,
        ),
        DraftInferenceRelation::VerticalPoints { .. } => (
            "vertical-points",
            "Vertical points",
            SceneConstraintGlyph::Vertical,
        ),
        DraftInferenceRelation::HorizontalPointToMidpoint { .. } => (
            "horizontal-point-to-midpoint",
            "Horizontal to midpoint",
            SceneConstraintGlyph::Horizontal,
        ),
        DraftInferenceRelation::VerticalPointToMidpoint { .. } => (
            "vertical-point-to-midpoint",
            "Vertical to midpoint",
            SceneConstraintGlyph::Vertical,
        ),
        DraftInferenceRelation::Concentric { .. } => {
            ("concentric", "Concentric", SceneConstraintGlyph::Concentric)
        }
        DraftInferenceRelation::Collinear { .. } => {
            ("collinear", "Collinear", SceneConstraintGlyph::Collinear)
        }
    }
}

const fn inference_status_key(status: &DraftInferenceStatus) -> &'static str {
    match status {
        DraftInferenceStatus::None => "none",
        DraftInferenceStatus::Resolved { .. } => "resolved",
        DraftInferenceStatus::Ambiguous { .. } => "ambiguous",
        DraftInferenceStatus::Suppressed => "suppressed",
        DraftInferenceStatus::ResourceLimited => "resource-limited",
        DraftInferenceStatus::StalePreferredCandidate { .. } => "stale-preference",
    }
}

const fn inference_status_warning(
    status: &DraftInferenceStatus,
) -> Option<(&'static str, &'static str)> {
    match status {
        DraftInferenceStatus::Ambiguous { .. } => Some((
            "ambiguous",
            "Ambiguous auto-constraint — press Tab to cycle or move closer",
        )),
        DraftInferenceStatus::Suppressed => {
            Some(("suppressed", "Auto-constraints suppressed by Ctrl/Cmd"))
        }
        DraftInferenceStatus::ResourceLimited => Some((
            "resource-limited",
            "Auto-constraints unavailable: inference resource limit reached",
        )),
        DraftInferenceStatus::StalePreferredCandidate { .. } => Some((
            "stale-preference",
            "Auto-constraint choice expired — move to refresh",
        )),
        DraftInferenceStatus::None | DraftInferenceStatus::Resolved { .. } => None,
    }
}

fn construction_geometry_markup(
    output: &mut String,
    geometry: &ConstructionPreviewGeometry,
    viewport: Viewport,
) {
    match geometry {
        ConstructionPreviewGeometry::Point { position } => {
            marker(output, viewport, *position, "wb-draft-point");
        }
        ConstructionPreviewGeometry::Polyline { points } => {
            line(output, viewport, points);
        }
        ConstructionPreviewGeometry::Rectangle { first, second } => {
            let first = viewport.model_to_screen(*first);
            let second = viewport.model_to_screen(*second);
            let _ = write!(
                output,
                "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"/>",
                first.x.min(second.x),
                first.y.min(second.y),
                (second.x - first.x).abs(),
                (second.y - first.y).abs(),
            );
        }
        ConstructionPreviewGeometry::Circle { center, radius } => {
            let screen = viewport.model_to_screen(*center);
            let _ = write!(
                output,
                "<circle class=\"wb-draft-circle\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"/>",
                screen.x,
                screen.y,
                radius * viewport.pixels_per_model_unit,
            );
            marker(output, viewport, *center, "wb-draft-center");
        }
        ConstructionPreviewGeometry::CounterClockwiseArc {
            center,
            start,
            end,
            radius,
            large_arc,
            ..
        } => {
            circular_arc_markup(
                output,
                viewport,
                (*center, *start, *end),
                *radius,
                *large_arc,
                0,
            );
        }
        ConstructionPreviewGeometry::CircularArc {
            center,
            start,
            end,
            radius,
            large_arc,
            sweep,
            ..
        } => {
            let sweep_flag = match sweep {
                DocumentArcSweep::CounterClockwise => 0,
                DocumentArcSweep::Clockwise => 1,
            };
            circular_arc_markup(
                output,
                viewport,
                (*center, *start, *end),
                *radius,
                *large_arc,
                sweep_flag,
            );
        }
        ConstructionPreviewGeometry::AdvancedCurve {
            kind,
            control_points,
            curve_points,
        } => {
            if *kind == AdvancedConstructionKind::EllipticalArc {
                elliptical_arc_control_markup(output, viewport, control_points);
            } else {
                advanced_control_polygon(output, viewport, *kind, control_points);
            }
            let points = curve_points
                .iter()
                .copied()
                .map(|point| viewport.model_to_screen(point))
                .collect::<Vec<_>>();
            if points.len() >= 2 {
                let _ = write!(
                    output,
                    "<path class=\"wb-draft-advanced-curve\" data-draft-kind=\"{}\" d=\"{}\"/>",
                    advanced_kind_key(*kind),
                    polyline_path(&points),
                );
            }
        }
    }
}

fn circular_arc_markup(
    output: &mut String,
    viewport: Viewport,
    points: ([f64; 2], [f64; 2], [f64; 2]),
    radius: f64,
    large_arc: bool,
    sweep_flag: u8,
) {
    let (center, start, end) = points;
    let start_screen = viewport.model_to_screen(start);
    let end_screen = viewport.model_to_screen(end);
    let radius = radius * viewport.pixels_per_model_unit;
    let large = u8::from(large_arc);
    let _ = write!(
        output,
        "<path d=\"M {:.3} {:.3} A {radius:.3} {radius:.3} 0 {large} {sweep_flag} {:.3} {:.3}\"/>",
        start_screen.x, start_screen.y, end_screen.x, end_screen.y
    );
    marker(output, viewport, center, "wb-draft-center");
    marker(output, viewport, start, "wb-draft-start");
    marker(output, viewport, end, "wb-draft-end");
}

fn elliptical_arc_support_markup(
    output: &mut String,
    viewport: Viewport,
    center: [f64; 2],
    major_axis_point: [f64; 2],
    support_points: &[[f64; 2]],
    trim_start: Option<[f64; 2]>,
) {
    let center_screen = viewport.model_to_screen(center);
    let major_screen = viewport.model_to_screen(major_axis_point);
    let support = support_points
        .iter()
        .copied()
        .map(|point| viewport.model_to_screen(point))
        .collect::<Vec<_>>();
    if support.len() >= 2 {
        let _ = write!(
            output,
            "<path class=\"wb-draft-ellipse-support\" d=\"{}\"/>",
            polyline_path(&support),
        );
    }
    let _ = write!(
        output,
        "<path class=\"wb-draft-major-axis\" d=\"M {:.3} {:.3} L {:.3} {:.3}\"/>",
        center_screen.x, center_screen.y, major_screen.x, major_screen.y,
    );
    marker(output, viewport, center, "wb-draft-center");
    marker(
        output,
        viewport,
        major_axis_point,
        "wb-draft-major-axis-point",
    );
    if let Some(start) = trim_start {
        marker(output, viewport, start, "wb-draft-start");
    }
}

fn elliptical_arc_control_markup(output: &mut String, viewport: Viewport, points: &[[f64; 2]]) {
    let [center, major_axis_point, trim_start, trim_end] = points else {
        advanced_control_polygon(
            output,
            viewport,
            AdvancedConstructionKind::EllipticalArc,
            points,
        );
        return;
    };
    let center_screen = viewport.model_to_screen(*center);
    let major_screen = viewport.model_to_screen(*major_axis_point);
    let _ = write!(
        output,
        "<path class=\"wb-draft-major-axis\" d=\"M {:.3} {:.3} L {:.3} {:.3}\"/>",
        center_screen.x, center_screen.y, major_screen.x, major_screen.y,
    );
    marker(output, viewport, *center, "wb-draft-center");
    marker(
        output,
        viewport,
        *major_axis_point,
        "wb-draft-major-axis-point",
    );
    marker(output, viewport, *trim_start, "wb-draft-start");
    marker(output, viewport, *trim_end, "wb-draft-end");
}

fn advanced_control_polygon(
    output: &mut String,
    viewport: Viewport,
    kind: AdvancedConstructionKind,
    points: &[[f64; 2]],
) {
    let points = points
        .iter()
        .copied()
        .map(|point| viewport.model_to_screen(point))
        .collect::<Vec<_>>();
    if points.len() >= 2 {
        let _ = write!(
            output,
            "<path class=\"wb-draft-control-polygon\" data-draft-kind=\"{}\" d=\"{}\"/>",
            advanced_kind_key(kind),
            polyline_path(&points),
        );
    }
    for (index, point) in points.iter().enumerate() {
        let _ = write!(
            output,
            "<circle class=\"wb-draft-point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"4\" data-draft-control=\"{index}\"/>",
            point.x, point.y,
        );
    }
}

const fn advanced_kind_key(kind: AdvancedConstructionKind) -> &'static str {
    match kind {
        AdvancedConstructionKind::QuadraticBezier => "quadratic-bezier",
        AdvancedConstructionKind::CubicBezier => "cubic-bezier",
        AdvancedConstructionKind::Ellipse => "ellipse",
        AdvancedConstructionKind::EllipticalArc => "elliptical-arc",
        AdvancedConstructionKind::RationalQuadraticConic => "rational-conic",
        AdvancedConstructionKind::Parabola => "parabola",
        AdvancedConstructionKind::Hyperbola => "hyperbola",
        AdvancedConstructionKind::Nurbs => "nurbs",
    }
}

fn line(output: &mut String, viewport: Viewport, points: &[[f64; 2]]) {
    let points = points
        .iter()
        .copied()
        .map(|point| viewport.model_to_screen(point))
        .collect::<Vec<_>>();
    if points.len() >= 2 {
        let _ = write!(output, "<path d=\"{}\"/>", polyline_path(&points));
    }
    for point in points {
        let _ = write!(
            output,
            "<circle class=\"wb-draft-point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"4\"/>",
            point.x, point.y,
        );
    }
}

fn marker(output: &mut String, viewport: Viewport, point: [f64; 2], class: &str) {
    let point = viewport.model_to_screen(point);
    let _ = write!(
        output,
        "<circle class=\"wb-cursor-point {class}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"7\"/>",
        point.x, point.y,
    );
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

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
        SceneAnnotationOccurrence, SceneComputedOffsetCurve, ScenePointRoleIncidence, ScreenPoint,
        SelectionItem, Viewport,
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
        ComputedEdgeId, ComputedEvaluationRevision, ComputedFeatureCornerId, ComputedFeatureId,
        NativeCurveSpanSource,
    };

    use super::{
        CanvasCamera, CanvasDisplayOptions, OffsetCanvasPresentation, adaptive_grid_spec,
        annotation_geometry, constraint_glyph, construction_geometry_markup, construction_markup,
        dimension_kind, grid_path, render_curve_controls, svg_markup,
        svg_markup_with_computed_context,
        svg_markup_with_computed_context_action_stamp_and_display,
        svg_markup_with_computed_context_action_stamp_display_and_provisional,
        svg_markup_with_computed_context_and_action_stamp, svg_markup_with_context, viewport,
    };
    use crate::workbench::panels::{lifecycle_presentation, problem_markup};

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

    fn native_curve_element(markup: &str, span: CurveSpan) -> &str {
        let identity = format!("data-persistent-id=\"{}\"", span.curve);
        let identity_start = markup.find(&identity).expect("native curve identity");
        let start = markup[..identity_start]
            .rfind("<path class=\"wb-curve")
            .expect("native curve element start");
        let end = identity_start
            + markup[identity_start..]
                .find("/>")
                .expect("native curve element end")
            + 2;
        &markup[start..end]
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
            },
            viewport,
        );
        assert!(!markup.contains("data-datum=\"origin\""));
        assert!(!markup.contains("data-datum=\"x-axis\""));
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

        let origin_center = camera.model_center;
        assert!(camera.pan_from(
            origin_center,
            ScreenPoint { x: 100.0, y: 100.0 },
            ScreenPoint { x: 200.0, y: 150.0 },
        ));
        assert_point_close(
            camera.model_center,
            [
                origin_center[0] - 100.0 / camera.pixels_per_model_unit,
                origin_center[1] + 50.0 / camera.pixels_per_model_unit,
            ],
        );

        assert!(camera.center_origin());
        assert_point_close(camera.model_center, [0.0, 0.0]);
        assert!(!camera.center_origin());
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
            let path = grid_path(spec.screen_origin, spec.minor_pixels, viewport.screen_size);
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

        let mut read_only = middle.clone();
        read_only.availability = DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::HostParameterOwned,
        );
        scene.curve_controls = vec![read_only];
        let mut read_only_markup = String::new();
        render_curve_controls(&mut read_only_markup, &scene, EditorHoverState::default());
        assert!(read_only_markup.contains("class=\"wb-curve-control read-only\""));
        assert!(read_only_markup.contains("aria-disabled=\"true\""));
        assert!(read_only_markup.contains("read-only: value is owned by a host parameter"));

        let feature = ComputedFeatureId::from_raw(82);
        let mut offset_proxy = middle;
        offset_proxy.offset_proxy =
            Some(geosolve_constraint_editor::SceneCurveControlOffsetProxy {
                feature,
                source_model_offset: [-0.25, 0.1],
            });
        offset_proxy.accessible_name = format!("Offset proxy — {}", offset_proxy.accessible_name);
        scene.curve_controls = vec![offset_proxy];
        let mut proxy_markup = String::new();
        render_curve_controls(&mut proxy_markup, &scene, EditorHoverState::default());
        assert!(proxy_markup.contains("class=\"wb-curve-control offset-proxy\""));
        assert!(proxy_markup.contains("data-offset-proxy=\"true\""));
        assert!(proxy_markup.contains(&format!("data-feature-id=\"{feature}\"")));
        assert!(proxy_markup.contains("aria-label=\"Offset proxy — Middle control P1"));
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
        let mut camera = CanvasCamera {
            model_center: [15.0, -4.0],
            pixels_per_model_unit: 137.0,
        };
        assert!(!camera.fit_scene_or_reset(Some(&scene)));
        assert_eq!(camera, CanvasCamera::default());
        camera.model_center = [1.0, 2.0];
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
        let tree = crate::workbench::panels::tree_markup_with_pending(
            accepted.document(),
            &scene.constraint_entries,
            &selection,
            &[],
        );
        let point_identity = format!("data-persistent-id=\"{}\"", rectangle.points[0]);
        assert!(markup.contains("class=\"wb-point selected\""));
        assert!(markup.contains(&point_identity));
        assert!(tree.contains(&point_identity));
        assert!(tree.contains("aria-selected=\"true\""));
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
        reason = "one SVG contract binds multi-edge ownership, related native sources, preview interactivity and visibility scope"
    )]
    fn computed_curve_offset_edges_use_feature_selection_and_preview_stays_noninteractive() {
        let mut document = SketchDocument::new(8.0).expect("document");
        let start = document.add_point("start", [-2.0, 0.0]).expect("start");
        let end = document.add_point("end", [2.0, 0.0]).expect("end");
        let source = CurveSpan::line(
            document
                .add_curve(
                    "source",
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("source line"),
        );
        let second_start = document
            .add_point("second start", [-2.0, 3.0])
            .expect("second start");
        let second_end = document
            .add_point("second end", [2.0, 3.0])
            .expect("second end");
        let second_source = CurveSpan::line(
            document
                .add_curve(
                    "second source",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("second source line"),
        );
        let unrelated_start = document
            .add_point("unrelated start", [-2.0, -3.0])
            .expect("unrelated start");
        let unrelated_end = document
            .add_point("unrelated end", [2.0, -3.0])
            .expect("unrelated end");
        let unrelated_source = CurveSpan::line(
            document
                .add_curve(
                    "unrelated source",
                    CurveDefinition::Line {
                        start: unrelated_start,
                        end: unrelated_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("unrelated source line"),
        );
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session.accepted_state().expect("accepted source");
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
        let owner = ComputedFeatureId::from_raw(17);
        let unrelated_owner = ComputedFeatureId::from_raw(18);
        let item = SelectionItem::Feature(owner);
        scene.computed_offset_curves.extend([
            SceneComputedOffsetCurve {
                edge: ComputedEdgeId {
                    evaluation: ComputedEvaluationRevision::from_raw(23),
                    ordinal: 4,
                },
                owner,
                source: NativeCurveSpanSource { span: source },
                role: GeometryRole::Profile,
                screen_polyline: [[-2.0, 1.0], [0.0, 1.2], [2.0, 1.0]]
                    .map(|point| viewport.model_to_screen(point))
                    .to_vec(),
                screen_source_parameters: vec![0.0, 0.5, 1.0],
            },
            SceneComputedOffsetCurve {
                edge: ComputedEdgeId {
                    evaluation: ComputedEvaluationRevision::from_raw(23),
                    ordinal: 5,
                },
                owner,
                source: NativeCurveSpanSource { span: source },
                role: GeometryRole::Profile,
                screen_polyline: [[-2.0, 1.1], [2.0, 1.1]]
                    .map(|point| viewport.model_to_screen(point))
                    .to_vec(),
                screen_source_parameters: vec![0.0, 1.0],
            },
            SceneComputedOffsetCurve {
                edge: ComputedEdgeId {
                    evaluation: ComputedEvaluationRevision::from_raw(23),
                    ordinal: 6,
                },
                owner,
                source: NativeCurveSpanSource {
                    span: second_source,
                },
                role: GeometryRole::Profile,
                screen_polyline: [[-2.0, 4.0], [2.0, 4.0]]
                    .map(|point| viewport.model_to_screen(point))
                    .to_vec(),
                screen_source_parameters: vec![0.0, 1.0],
            },
            SceneComputedOffsetCurve {
                edge: ComputedEdgeId {
                    evaluation: ComputedEvaluationRevision::from_raw(23),
                    ordinal: 7,
                },
                owner: unrelated_owner,
                source: NativeCurveSpanSource {
                    span: unrelated_source,
                },
                role: GeometryRole::Profile,
                screen_polyline: [[-2.0, -2.0], [2.0, -2.0]]
                    .map(|point| viewport.model_to_screen(point))
                    .to_vec(),
                screen_source_parameters: vec![0.0, 1.0],
            },
        ]);
        assert!(
            scene
                .annotations
                .iter()
                .all(|annotation| annotation.item != item),
            "a generated Offset edge must not manufacture an ordinary sketch annotation owner",
        );

        let current = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[item],
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
            None,
            viewport,
        );
        let offset_item = |markup: &str| {
            let identity = format!("data-feature-id=\"{owner}\"");
            let identity_start = markup.find(&identity).expect("computed Offset owner");
            let start = markup[..identity_start]
                .rfind("<g class=\"wb-computed-item")
                .expect("computed Offset group");
            let end = identity_start
                + markup[identity_start..]
                    .find("</g>")
                    .expect("computed Offset group end")
                + "</g>".len();
            markup[start..end].to_owned()
        };
        let current_item = offset_item(&current);
        assert!(current_item.contains("wb-computed-offset-item selected"));
        assert!(!current_item.contains("geometry-hovered"));
        assert!(current_item.contains("data-editor-item=\"feature\""));
        assert!(current_item.contains("data-computed-source="));
        assert!(current_item.contains("class=\"wb-curve wb-computed-offset\""));
        assert!(current_item.contains("class=\"wb-computed-hit\""));
        assert!(!current_item.contains("data-feature-corner-id"));
        assert!(!current_item.contains("data-provisional"));
        assert!(native_curve_element(&current, source).contains(" related"));
        assert!(native_curve_element(&current, second_source).contains(" related"));
        assert!(!native_curve_element(&current, unrelated_source).contains(" related"));
        assert_eq!(
            current
                .matches(&format!("data-feature-id=\"{owner}\""))
                .count(),
            3,
            "every generated edge keeps the same stable multi-source feature owner",
        );

        let hover_only = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            &[],
            EditorHoverState {
                target: Some(EditorHoverTarget::Geometry(item)),
                context_owner: None,
            },
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
        let hover_item = offset_item(&hover_only);
        assert!(hover_item.contains("geometry-hovered"));
        assert!(!hover_item.contains(" selected"));
        assert!(native_curve_element(&hover_only, source).contains(" related"));
        assert!(native_curve_element(&hover_only, second_source).contains(" related"));
        assert!(!native_curve_element(&hover_only, unrelated_source).contains(" related"));

        let preview = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            &[item],
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
        let preview_item = offset_item(&preview);
        assert!(preview_item.contains("offset-provisional interaction-disabled"));
        assert!(preview_item.contains("data-provisional=\"true\""));
        assert!(preview_item.contains("role=\"img\""));
        assert!(!preview_item.contains("data-editor-item"));
        assert!(!preview_item.contains("wb-computed-hit"));

        scene.computed_offset_curves[0].role = GeometryRole::Construction;
        let scope_excluded = svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy {
                scope: geosolve_constraint_editor::GeometryPickScope::Profile,
                ..GeometryInteractionPolicy::default()
            },
            CanvasDisplayOptions::default(),
            None,
            viewport,
        );
        let scope_excluded_item = offset_item(&scope_excluded);
        assert!(scope_excluded_item.contains("wb-computed-offset construction"));
        assert!(scope_excluded_item.contains("interaction-disabled"));
        assert!(!scope_excluded_item.contains("data-editor-item"));
        assert!(!scope_excluded_item.contains("wb-computed-hit"));
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
        assert_eq!(
            lifecycle_presentation(coordinator.lifecycle().status),
            ("rejected-attempt", "Rejected attempt")
        );
        let problems = coordinator.problems();
        assert!(problems.failure.is_some() || problems.rejection.is_some());
        let problem = problem_markup("Rejected attempt: parameter value has the wrong kind");
        assert!(problem.contains("Rejected attempt"));

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
