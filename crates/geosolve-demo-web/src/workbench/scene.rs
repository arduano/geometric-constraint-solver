// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{collections::BTreeSet, fmt::Write as _};

use geosolve_constraint_editor::{
    AdvancedConstructionKind, ComputedFeatureProblemMetadata, ComputedFilletContinuationLimitKind,
    ConstructionPreview, ConstructionPreviewGeometry, DimensionTargetDisplayUnit, DraftGuide,
    DraftGuideClassification, DraftGuideGeometry, DraftInferenceFamily, DraftInferenceRelation,
    DraftInferenceResolution, DraftInferenceStatus, EditorHoverState, EditorHoverTarget,
    EditorProblemCategory, EditorProblemMetadata, EditorProblemScope, EditorProblemTarget,
    EditorScene, GeometryInteractionPolicy, SceneAnnotationGeometry, SceneAnnotationKind,
    SceneConstraintGlyph, SceneCurveOrigin, SceneFilletAction, SceneFilletActionAvailability,
    SceneFilletActionId, SceneFilletActionTarget, SceneFilletCornerAffordances, ScreenPoint,
    SelectionItem, Viewport, display_dimension_target,
};
#[cfg(test)]
use geosolve_sketch::DocumentConstraintDefinition;
use geosolve_sketch::{
    DesignScalarId, DocumentCurveNormalSide, DocumentDimensionDefinition, DocumentDimensionMode,
    GeometryRole, ScalarUnit, SketchAcceptedDocumentState,
};
use geosolve_sketch_features::NativeCurveSpanSource;

const SCREEN_SIZE: [f64; 2] = [1000.0, 700.0];
const DEFAULT_PIXELS_PER_MODEL_UNIT: f64 = 50.0;
const MIN_PIXELS_PER_MODEL_UNIT: f64 = 2.0;
const MAX_PIXELS_PER_MODEL_UNIT: f64 = 2_000.0;
const FIT_MARGIN_PIXELS: f64 = 64.0;

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
    let related = scene
        .map(|scene| {
            scene
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
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
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
        "<defs><marker id=\"wb-dimension-arrow\" markerWidth=\"6\" markerHeight=\"6\" ",
        "refX=\"3\" refY=\"3\" orient=\"auto-start-reverse\"><path d=\"M0 0L6 3L0 6Z\"/>",
        "</marker><marker id=\"wb-fillet-direction-arrow\" markerWidth=\"6\" markerHeight=\"6\" ",
        "refX=\"5\" refY=\"3\" orient=\"auto\"><path fill=\"context-stroke\" ",
        "d=\"M0 0L6 3L0 6Z\"/></marker></defs>"
    ));
    let origin = viewport.model_to_screen([0.0, 0.0]);
    let _ = write!(
        output,
        "<g class=\"wb-grid\"><path d=\"M0 {:.3}H1000M{:.3} 0V700\"/></g><g class=\"wb-geometry\">",
        origin.y, origin.x,
    );
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
            let selected = selection.contains(&SelectionItem::Curve(curve.span));
            let pending = pending.contains(&SelectionItem::Curve(curve.span));
            let target = EditorProblemTarget::Curve(curve.span.curve);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let role = curve.role;
            let item = SelectionItem::Curve(curve.span);
            let interactive = curve.is_interactive(geometry_policy);
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-curve{}{}{}{}{}{}\" d=\"{}\" ",
                    "data-persistent-id=\"{}\" {}",
                    "data-editor-segment=\"{}\" data-role=\"{}\" data-source-role=\"{}\" ",
                    "data-construction-origin=\"{}\" data-interactive=\"{}\"/>"
                ),
                if selected { " selected" } else { "" },
                if pending { " authoring-pending" } else { "" },
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
        output.push_str("</g><g class=\"wb-points\">");
        for point in scene
            .points
            .iter()
            .filter(|point| point.is_visible(geometry_policy))
        {
            let interactive = point.is_interactive(geometry_policy);
            let selected = selection.contains(&SelectionItem::Point(point.id));
            let pending = pending.contains(&SelectionItem::Point(point.id));
            let target = EditorProblemTarget::Point(point.id);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let _ = write!(
                output,
                concat!(
                    "<circle class=\"wb-point{}{}{}{}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" ",
                    "data-persistent-id=\"{}\" {}data-interactive=\"{}\"/>"
                ),
                if selected { " selected" } else { "" },
                if pending { " authoring-pending" } else { "" },
                if related.contains(&SelectionItem::Point(point.id)) {
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
    output.push_str("</g><g class=\"wb-annotations\">");
    if let (Some(scene), Some(accepted)) = (scene, accepted) {
        render_annotations(
            &mut output,
            &mut problem_markers,
            &mut resolved_targets,
            scene,
            accepted,
            selection,
            hover,
            &problem_items,
            problem,
        );
    }
    output.push_str("</g>");
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

fn render_computed_geometry(
    output: &mut String,
    scene: &EditorScene,
    selection: &[SelectionItem],
    active_fillet_preview: Option<&SceneFilletActionTarget>,
    fillet_action_stamp: Option<u64>,
    geometry_policy: GeometryInteractionPolicy,
) {
    let evaluation = scene
        .computed_curves
        .first()
        .map_or(0, |curve| curve.edge.evaluation.raw());
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
        let affected = affected_owners.contains(&curve.owner);
        let interactive = curve.is_interactive(geometry_policy);
        let path = polyline_path(&curve.screen_polyline);
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-computed-item{}{}{}\" {}",
                "data-feature-id=\"{}\" data-feature-corner-id=\"{}\" ",
                "data-computed-evaluation=\"{}\" data-computed-edge=\"{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\">",
                "<path class=\"wb-curve wb-computed-fillet{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\" d=\"{}\"/>"
            ),
            if selected { " selected" } else { "" },
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
                "<g class=\"wb-fillet-radius-affordance\" data-feature-id=\"{}\" ",
                "data-feature-corner-id=\"{}\">",
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
    hover: EditorHoverState,
    problem_items: &[SelectionItem],
    problem: Option<&EditorProblemMetadata>,
) {
    let document = accepted.document();
    let visibility_context = hover
        .context_owner
        .or_else(|| hover.target.map(EditorHoverTarget::item));
    for annotation in &scene.annotations {
        if !annotation.is_visible(selection, visibility_context, problem_items) {
            continue;
        }
        let selected = selection.contains(&annotation.item);
        let hovered_occurrence = match hover.target {
            Some(EditorHoverTarget::Annotation(occurrence))
                if occurrence.item == annotation.item =>
            {
                Some(occurrence)
            }
            Some(EditorHoverTarget::Geometry(_) | EditorHoverTarget::Annotation(_)) | None => None,
        };
        let is_hovered =
            hovered_occurrence.is_some_and(|occurrence| occurrence.marker_index.is_none());
        let has_problem = problem_items.contains(&annotation.item);
        let class = format!(
            "{}{}{}{}",
            if selected { " selected" } else { "" },
            if is_hovered { " hovered" } else { "" },
            if has_problem { " has-problem" } else { "" },
            if annotation.suppressed {
                " suppressed"
            } else {
                ""
            },
        );
        let (editor_kind, id, kind, label, value, mode) = match annotation.item {
            SelectionItem::Constraint(id) => {
                let Some(constraint) = document.constraint(id) else {
                    continue;
                };
                (
                    "constraint",
                    id.to_string(),
                    annotation_kind(annotation.kind),
                    constraint.label.clone(),
                    String::new(),
                    String::new(),
                )
            }
            SelectionItem::Dimension(id) => {
                let Some(dimension) = document.dimension(id) else {
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
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => continue,
        };
        let escaped_label = escape(&label);
        let _ = write!(
            output,
            "<g class=\"wb-annotation wb-{editor_kind}{class}\" tabindex=\"0\" role=\"button\" aria-label=\"{escaped_label}\" data-editor-item=\"{editor_kind}\" data-persistent-id=\"{id}\" data-{editor_kind}-kind=\"{kind}\"{}{} data-annotation-kind=\"{kind}\">",
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
        annotation_geometry(
            output,
            annotation.kind,
            &annotation.geometry,
            &escaped_label,
            hovered_occurrence.and_then(|occurrence| occurrence.marker_index),
        );
        output.push_str("</g>");

        if has_problem {
            let target = match annotation.item {
                SelectionItem::Constraint(id) => EditorProblemTarget::Constraint(id),
                SelectionItem::Dimension(id) => EditorProblemTarget::Dimension(id),
                SelectionItem::Point(_)
                | SelectionItem::Curve(_)
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
    label: &str,
    hovered_marker: Option<usize>,
) {
    match geometry {
        SceneAnnotationGeometry::Glyph { markers } => {
            let SceneAnnotationKind::Constraint(glyph) = kind else {
                return;
            };
            for (index, marker) in markers.iter().enumerate() {
                if let Some(origin) = marker.leader_from {
                    let _ = write!(
                        output,
                        "<path class=\"wb-annotation-leader\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                        origin.x, origin.y, marker.anchor.x, marker.anchor.y,
                    );
                }
                let _ = write!(
                    output,
                    "<g class=\"wb-constraint-symbol{}\" transform=\"translate({:.3} {:.3})\" data-annotation-marker=\"{index}\"><circle class=\"wb-annotation-hit\" r=\"12\"/>{}</g>",
                    if hovered_marker == Some(index) {
                        " hovered"
                    } else {
                        ""
                    },
                    marker.anchor.x,
                    marker.anchor.y,
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
                    "<circle class=\"wb-annotation-hit\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"12\"/>",
                    "</g>"
                ),
                first_arm.x,
                first_arm.y,
                corner.x,
                corner.y,
                second_arm.x,
                second_arm.y,
                corner.x,
                corner.y,
            );
        }
        SceneAnnotationGeometry::LinearDimension {
            first,
            second,
            label_anchor,
        } => {
            let _ = write!(
                output,
                concat!(
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}",
                    "M{:.3} {:.3}L{:.3} {:.3}M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<circle class=\"wb-annotation-hit\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"12\"/>",
                    "<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                first.x,
                first.y,
                second.x,
                second.y,
                first.x,
                first.y,
                label_anchor.x,
                label_anchor.y,
                second.x,
                second.y,
                label_anchor.x,
                label_anchor.y,
                label_anchor.x,
                label_anchor.y,
                label_anchor.x,
                label_anchor.y - 5.0,
                label,
            );
        }
        SceneAnnotationGeometry::RadialDimension {
            center,
            edge,
            label_anchor,
            diameter,
        } => {
            let opposite = if *diameter {
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
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<circle class=\"wb-annotation-hit\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"12\"/>",
                    "<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                opposite.x,
                opposite.y,
                edge.x,
                edge.y,
                label_anchor.x,
                label_anchor.y,
                label_anchor.x,
                label_anchor.y - 5.0,
                label,
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
                    "<path class=\"wb-angle-arc\" marker-start=\"url(#wb-dimension-arrow)\" ",
                    "marker-end=\"url(#wb-dimension-arrow)\" d=\"M{:.3} {:.3}A{:.3} {:.3} 0 0 {} {:.3} {:.3}\"/>",
                    "<circle class=\"wb-annotation-hit\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"12\"/>",
                    "<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
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
                label_anchor.x,
                label_anchor.y,
                label_anchor.x,
                label_anchor.y - 5.0,
                label,
            );
        }
        SceneAnnotationGeometry::Label { anchor } => {
            let _ = write!(
                output,
                "<circle class=\"wb-annotation-hit\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"12\"/><text x=\"{:.3}\" y=\"{:.3}\">{}</text>",
                anchor.x,
                anchor.y,
                anchor.x,
                anchor.y - 5.0,
                label,
            );
        }
    }
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
    }
}

fn annotation_anchor(geometry: &SceneAnnotationGeometry) -> Option<ScreenPoint> {
    Some(match geometry {
        SceneAnnotationGeometry::Glyph { markers } => markers.first()?.anchor,
        SceneAnnotationGeometry::RightAngle { corner, .. } => *corner,
        SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
        | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
        | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => *label_anchor,
        SceneAnnotationGeometry::Label { anchor } => *anchor,
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
        | DocumentConstraintDefinition::ExternalPointCoincident { .. } => ("coincident", "Coin"),
        DocumentConstraintDefinition::Horizontal { .. } => ("horizontal", "H"),
        DocumentConstraintDefinition::Vertical { .. } => ("vertical", "V"),
        DocumentConstraintDefinition::PointOnCurve { .. } => ("point-on-curve", "On"),
        DocumentConstraintDefinition::Parallel { .. } => ("parallel", "∥"),
        DocumentConstraintDefinition::Perpendicular { .. } => ("perpendicular", "⊥"),
        DocumentConstraintDefinition::ExternalLineCollinear { .. } => ("collinear", "Col"),
        DocumentConstraintDefinition::EqualLength { .. } => ("equal-length", "L="),
        DocumentConstraintDefinition::EqualRadius { .. } => ("equal-radius", "R="),
        DocumentConstraintDefinition::Midpoint { .. } => ("midpoint", "Mid"),
        DocumentConstraintDefinition::SymmetricAboutLine { .. } => ("symmetry", "Sym"),
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
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. } => *target,
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
        DraftInferenceFamily::PointOnCurve => "point-on-curve",
        DraftInferenceFamily::PointOnCreatedCurve => "point-on-created-curve",
        DraftInferenceFamily::Midpoint => "midpoint",
        DraftInferenceFamily::Horizontal => "horizontal",
        DraftInferenceFamily::Vertical => "vertical",
        DraftInferenceFamily::Parallel => "parallel",
        DraftInferenceFamily::Perpendicular => "perpendicular",
        DraftInferenceFamily::PointTracking => "point-tracking",
    }
}

const fn inference_family_label(family: DraftInferenceFamily) -> &'static str {
    match family {
        DraftInferenceFamily::PointIdentity => "Reuse existing point",
        DraftInferenceFamily::PointOnCurve => "Point on curve",
        DraftInferenceFamily::PointOnCreatedCurve => "Circle through point",
        DraftInferenceFamily::Midpoint => "Midpoint",
        DraftInferenceFamily::Horizontal => "Horizontal",
        DraftInferenceFamily::Vertical => "Vertical",
        DraftInferenceFamily::Parallel => "Parallel",
        DraftInferenceFamily::Perpendicular => "Perpendicular",
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
            "Ambiguous auto-constraint — move closer to one target",
        )),
        DraftInferenceStatus::Suppressed => Some(("suppressed", "Auto-constraints suppressed")),
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
            let start_screen = viewport.model_to_screen(*start);
            let end_screen = viewport.model_to_screen(*end);
            let radius = radius * viewport.pixels_per_model_unit;
            let large = u8::from(*large_arc);
            let _ = write!(
                output,
                "<path d=\"M {:.3} {:.3} A {radius:.3} {radius:.3} 0 {large} 0 {:.3} {:.3}\"/>",
                start_screen.x, start_screen.y, end_screen.x, end_screen.y
            );
            marker(output, viewport, *center, "wb-draft-center");
            marker(output, viewport, *start, "wb-draft-start");
            marker(output, viewport, *end, "wb-draft-end");
        }
        ConstructionPreviewGeometry::AdvancedCurve {
            kind,
            control_points,
            curve_points,
        } => {
            advanced_control_polygon(output, viewport, *kind, control_points);
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
        ComputedFeatureProblemMetadata, ConstructionPreviewGeometry, DraftInferenceEngine,
        DraftInferenceFrame, DraftInferenceInput, DraftInferencePolicy, DraftInferenceResolution,
        DraftInferenceSample, DraftInferenceSubject, DraftReferenceAnchor, EditorHoverState,
        EditorHoverTarget, EditorProblemScope, EditorScene, GeometryInteractionPolicy,
        RetainedEditorCoordinator, SceneAnnotationGeometry, SceneAnnotationOccurrence,
        ScenePointRoleIncidence, ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
        DocumentAngleOrientation, DocumentConstraintDefinition, DocumentDimensionDefinition,
        DocumentDimensionMode, DocumentEdit, DocumentParameterId, DocumentParameterKind,
        DocumentParameterTarget, DocumentSolveRequest, ParameterBatch, ParameterBatchEntry,
        ParameterValue, PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
        SketchDesignIdentity, SketchDocument,
    };
    use geosolve_sketch_features::{
        ComputedFeatureCornerId, ComputedFeatureId, NativeCurveSpanSource,
    };

    use super::{
        CanvasCamera, constraint_glyph, construction_geometry_markup, dimension_kind, svg_markup,
        svg_markup_with_computed_context, svg_markup_with_computed_context_and_action_stamp,
        svg_markup_with_context, viewport,
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

        let mut tracking_engine = DraftInferenceEngine::default();
        tracking_engine
            .remember_reference(anchor)
            .expect("remember point reference");
        let tracking_frame =
            inference_frame(design, accepted_revision, viewport, [2.0, 0.0], Vec::new());
        let tracking = tracking_engine
            .resolve(&tracking_frame, DraftInferenceInput::default())
            .expect("tracking-only inference");
        let tracking_markup = inference_markup(&tracking, viewport);
        assert!(tracking_markup.contains("data-inference-family=\"point-tracking\""));
        assert!(tracking_markup.contains("data-inference-classification=\"tracking-only\""));
        assert!(!tracking_markup.contains("data-inference-relation="));
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
        assert!(suppressed_markup.contains("aria-label=\"Auto-constraints suppressed\""));
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
            "point-on-curve",
            "parallel",
            "perpendicular",
            "equal-length",
            "equal-radius",
            "midpoint",
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
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let selection = [SelectionItem::Point(rectangle.points[0])];
        let markup = svg_markup(
            Some(&scene),
            Some(accepted),
            &selection,
            None,
            None,
            viewport(),
        );
        let tree = crate::workbench::panels::tree_markup(accepted.document(), &selection);
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
        assert!(markup.contains("angle = 45.000°"));
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
