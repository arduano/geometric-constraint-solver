// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::{collections::BTreeSet, fmt::Write as _};

use geosolve_constraint_editor::{
    AdvancedConstructionKind, ConstructionPreview, ConstructionPreviewGeometry,
    DimensionTargetDisplayUnit, EditorProblemCategory, EditorProblemMetadata, EditorProblemScope,
    EditorProblemTarget, EditorScene, SceneAnnotationGeometry, SceneAnnotationKind,
    SceneConstraintGlyph, ScreenPoint, SelectionItem, Viewport, display_dimension_target,
};
#[cfg(test)]
use geosolve_sketch::DocumentConstraintDefinition;
use geosolve_sketch::{
    DesignScalarId, DocumentDimensionDefinition, DocumentDimensionMode, GeometryRole, ScalarUnit,
    SketchAcceptedDocumentState,
};

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
        let mut minimum = [f64::INFINITY; 2];
        let mut maximum = [f64::NEG_INFINITY; 2];
        let mut include = |point: [f64; 2]| {
            if point.into_iter().all(f64::is_finite) {
                for axis in 0..2 {
                    minimum[axis] = minimum[axis].min(point[axis]);
                    maximum[axis] = maximum[axis].max(point[axis]);
                }
            }
        };
        for point in &scene.points {
            include(point.model_position);
        }
        for curve in &scene.curves {
            for point in &curve.screen_polyline {
                include(scene.viewport.screen_to_model(*point));
            }
        }
        if !minimum.into_iter().all(f64::is_finite) || !maximum.into_iter().all(f64::is_finite) {
            return false;
        }
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
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
        None,
        construction_preview,
        problem,
        viewport,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn svg_markup_with_context(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    hovered: Option<SelectionItem>,
    construction_preview: Option<&ConstructionPreview>,
    problem: Option<&EditorProblemMetadata>,
    viewport: Viewport,
) -> String {
    let mut output = String::new();
    let mut problem_markers = String::new();
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
                    selection.contains(&annotation.item) || hovered == Some(annotation.item)
                })
                .flat_map(|annotation| annotation.operands.iter().copied())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
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
    output.push_str(
        "<defs><marker id=\"wb-dimension-arrow\" markerWidth=\"6\" markerHeight=\"6\" refX=\"3\" refY=\"3\" orient=\"auto-start-reverse\"><path d=\"M0 0L6 3L0 6Z\"/></marker></defs>",
    );
    let origin = viewport.model_to_screen([0.0, 0.0]);
    let _ = write!(
        output,
        "<g class=\"wb-grid\"><path d=\"M0 {:.3}H1000M{:.3} 0V700\"/></g><g class=\"wb-geometry\">",
        origin.y, origin.x,
    );
    if let Some(scene) = scene {
        for curve in &scene.curves {
            if curve.screen_polyline.len() < 2 {
                continue;
            }
            let path = polyline_path(&curve.screen_polyline);
            let selected = selection.contains(&SelectionItem::Curve(curve.span));
            let pending = pending.contains(&SelectionItem::Curve(curve.span));
            let target = EditorProblemTarget::Curve(curve.span.curve);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let role = accepted
                .and_then(|state| state.document().geometry_role(curve.span.curve))
                .unwrap_or(GeometryRole::Profile);
            let item = SelectionItem::Curve(curve.span);
            let _ = write!(
                output,
                "<path class=\"wb-curve{}{}{}{}{}\" d=\"{path}\" data-persistent-id=\"{}\" data-editor-item=\"curve\" data-editor-segment=\"{}\" data-role=\"{}\"/>",
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
                curve.span.curve,
                curve.span.segment,
                if role == GeometryRole::Construction {
                    "construction"
                } else {
                    "profile"
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
        output.push_str("</g><g class=\"wb-points\">");
        for point in &scene.points {
            let selected = selection.contains(&SelectionItem::Point(point.id));
            let pending = pending.contains(&SelectionItem::Point(point.id));
            let target = EditorProblemTarget::Point(point.id);
            let has_problem = problem.is_some_and(|problem| problem.targets.contains(&target));
            let _ = write!(
                output,
                "<circle class=\"wb-point{}{}{}{}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" data-persistent-id=\"{}\" data-editor-item=\"point\"/>",
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
            hovered,
            &problem_items,
            problem,
        );
    }
    output.push_str("</g>");
    if let Some(preview) = construction_preview {
        output.push_str(&construction_markup(preview, viewport));
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
    output.push_str("</g>");
    output
}

fn digest(bytes: [u8; 32]) -> String {
    let mut output = String::new();
    for byte in &bytes[..6] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[allow(clippy::too_many_arguments)]
fn render_annotations(
    output: &mut String,
    problem_markers: &mut String,
    resolved_targets: &mut BTreeSet<EditorProblemTarget>,
    scene: &EditorScene,
    accepted: &SketchAcceptedDocumentState,
    selection: &[SelectionItem],
    hovered: Option<SelectionItem>,
    problem_items: &[SelectionItem],
    problem: Option<&EditorProblemMetadata>,
) {
    let document = accepted.document();
    for annotation in &scene.annotations {
        if !annotation.is_visible(selection, hovered, problem_items) {
            continue;
        }
        let selected = selection.contains(&annotation.item);
        let is_hovered = hovered == Some(annotation.item);
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
            SelectionItem::Point(_) | SelectionItem::Curve(_) => continue,
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
        );
        output.push_str("</g>");

        if has_problem {
            let target = match annotation.item {
                SelectionItem::Constraint(id) => EditorProblemTarget::Constraint(id),
                SelectionItem::Dimension(id) => EditorProblemTarget::Dimension(id),
                SelectionItem::Point(_) | SelectionItem::Curve(_) => unreachable!(),
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
) {
    match geometry {
        SceneAnnotationGeometry::Glyph { markers } => {
            let SceneAnnotationKind::Constraint(glyph) = kind else {
                return;
            };
            for marker in markers {
                if let Some(origin) = marker.leader_from {
                    let _ = write!(
                        output,
                        "<path class=\"wb-annotation-leader\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                        origin.x, origin.y, marker.anchor.x, marker.anchor.y,
                    );
                }
                let _ = write!(
                    output,
                    "<g class=\"wb-constraint-symbol\" transform=\"translate({:.3} {:.3})\"><circle class=\"wb-annotation-hit\" r=\"12\"/>{}</g>",
                    marker.anchor.x,
                    marker.anchor.y,
                    constraint_symbol(glyph),
                );
            }
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
        SceneAnnotationKind::Constraint(glyph) => constraint_glyph_key(glyph),
        SceneAnnotationKind::PointDistance => "point-distance",
        SceneAnnotationKind::CurveLength => "segment-length",
        SceneAnnotationKind::Radius => "radius",
        SceneAnnotationKind::Diameter => "diameter",
        SceneAnnotationKind::OrientedAngle => "oriented-angle",
        SceneAnnotationKind::SupportingLineOffset => "supporting-line-offset",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => "translated-segment-offset",
    }
}

const fn constraint_glyph_key(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => "fixed",
        SceneConstraintGlyph::Coincident => "coincident",
        SceneConstraintGlyph::Horizontal => "horizontal",
        SceneConstraintGlyph::Vertical => "vertical",
        SceneConstraintGlyph::PointOnCurve => "point-on-curve",
        SceneConstraintGlyph::Parallel => "parallel",
        SceneConstraintGlyph::Perpendicular => "perpendicular",
        SceneConstraintGlyph::Collinear => "collinear",
        SceneConstraintGlyph::EqualLength => "equal-length",
        SceneConstraintGlyph::EqualRadius => "equal-radius",
        SceneConstraintGlyph::Midpoint => "midpoint",
        SceneConstraintGlyph::Symmetry => "symmetry",
        SceneConstraintGlyph::Contact => "generic-contact",
        SceneConstraintGlyph::Tangency => "tangency",
        SceneConstraintGlyph::Direction => "curve-direction",
        SceneConstraintGlyph::Normal => "normal",
        SceneConstraintGlyph::EqualCurvature => "equal-curvature",
        SceneConstraintGlyph::Continuity => "continuity",
        SceneConstraintGlyph::Fillet => "fillet",
    }
}

const fn constraint_symbol(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => "<path d=\"M-5 5V-3A5 5 0 0 1 5-3V5M-8 5H8\"/>",
        SceneConstraintGlyph::Coincident => "<circle cx=\"-3\" r=\"4\"/><circle cx=\"3\" r=\"4\"/>",
        SceneConstraintGlyph::Horizontal => "<path d=\"M-7 0H7M-5-3V3M5-3V3\"/>",
        SceneConstraintGlyph::Vertical => "<path d=\"M0-7V7M-3-5H3M-3 5H3\"/>",
        SceneConstraintGlyph::PointOnCurve => {
            "<path d=\"M-8 4Q0-6 8 4\"/><circle cy=\"-1\" r=\"2\"/>"
        }
        SceneConstraintGlyph::Parallel => "<path d=\"M-7 4L4-7M-3 7L8-4\"/>",
        SceneConstraintGlyph::Perpendicular => "<path d=\"M-7-6V6H7\"/>",
        SceneConstraintGlyph::Collinear => "<path d=\"M-8 4L8-4M-5 1L-2 4M2-4L5-1\"/>",
        SceneConstraintGlyph::EqualLength => "<path d=\"M-8-3H8M-8 3H8M-2-6V0M2 0V6\"/>",
        SceneConstraintGlyph::EqualRadius => {
            "<circle r=\"7\"/><path d=\"M0 0L6-4M-4-1H4M-4 3H4\"/>"
        }
        SceneConstraintGlyph::Midpoint => "<path d=\"M-8 5L0-6L8 5ZM-5 5H5\"/><circle r=\"1.5\"/>",
        SceneConstraintGlyph::Symmetry => "<path d=\"M0-8V8M-7-5L-3 0L-7 5M7-5L3 0L7 5\"/>",
        SceneConstraintGlyph::Contact => {
            "<path d=\"M-8 3Q-4-4 0 3Q4 10 8 3\"/><circle cy=\"3\" r=\"2\"/>"
        }
        SceneConstraintGlyph::Tangency => "<circle cy=\"-2\" r=\"5\"/><path d=\"M-8 3H8\"/>",
        SceneConstraintGlyph::Direction => "<path d=\"M-8 4H6M2 0L6 4L2 8\"/>",
        SceneConstraintGlyph::Normal => "<path d=\"M-7 5H5V-7M1-3H5V1\"/>",
        SceneConstraintGlyph::EqualCurvature => {
            "<path d=\"M-8 5Q-4-7 0 1Q4 9 8-3M-4-7H4M-4-3H4\"/>"
        }
        SceneConstraintGlyph::Continuity => "<path d=\"M-8 4Q-3-7 0 0Q3 7 8-4M-2-6L2-6\"/>",
        SceneConstraintGlyph::Fillet => "<path d=\"M-8 6H-3A9 9 0 0 1 6-3V-8\"/>",
    }
}

fn annotation_anchor(geometry: &SceneAnnotationGeometry) -> Option<ScreenPoint> {
    Some(match geometry {
        SceneAnnotationGeometry::Glyph { markers } => markers.first()?.anchor,
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
            "<circle r=\"10\"/><text class=\"wb-error-marker-icon\" x=\"0\" y=\"4\">!</text>",
            "<foreignObject class=\"wb-error-tooltip\" x=\"{}\" y=\"{}\" width=\"360\" height=\"72\">",
            "<div xmlns=\"http://www.w3.org/1999/xhtml\">{}</div></foreignObject></g>"
        ),
        if global { " global" } else { "" },
        anchor.x,
        anchor.y,
        message,
        target_key,
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
        ConstructionPreviewGeometry, EditorScene, RetainedEditorCoordinator, ScreenPoint,
        SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
        DocumentAngleOrientation, DocumentConstraintDefinition, DocumentDimensionDefinition,
        DocumentDimensionMode, DocumentEdit, DocumentParameterId, DocumentParameterKind,
        DocumentParameterTarget, DocumentSolveRequest, ParameterBatch, ParameterBatchEntry,
        ParameterValue, PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
        SketchDocument,
    };

    use super::{
        CanvasCamera, constraint_glyph, construction_geometry_markup, dimension_kind, svg_markup,
        viewport,
    };
    use crate::workbench::panels::{
        accepted_redundancy_markup, host_state_markup, lifecycle_presentation, problem_markup,
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
        let lifecycle = host_state_markup(coordinator.session());
        let attempt = coordinator.session().last_attempt().identity();
        assert!(lifecycle.contains(&format!(
            "data-design-revision=\"{}\"",
            coordinator.session().design_identity().revision().get()
        )));
        assert!(lifecycle.contains(&format!(
            "data-attempt-revision=\"{}\"",
            attempt.revision().get()
        )));
        assert!(lifecycle.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted_before.revision().get()
        )));
        assert!(lifecycle.contains("data-attempt-status=\"failed\""));
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
        let unavailable = accepted_redundancy_markup(None);
        assert!(unavailable.contains("unavailable"));
        assert!(!unavailable.contains("rank zero"));
        assert!(!unavailable.contains("No sources published"));

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
}
