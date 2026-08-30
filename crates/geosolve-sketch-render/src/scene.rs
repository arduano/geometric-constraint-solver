// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    collections::BTreeSet,
    error::Error,
    fmt::{self, Write as _},
};

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
use geosolve_sketch::DocumentConstraintDefinition;
use geosolve_sketch::{
    DesignScalarId, DocumentArcSweep, DocumentCurveControlAvailability, DocumentCurveControlId,
    DocumentCurveControlKind, DocumentCurveControlWithholdingReason, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionMode, GeometryRole, ScalarUnit,
    SketchAcceptedDocumentState, SketchDatum,
};
use geosolve_sketch_features::NativeCurveSpanSource;

pub const SCREEN_SIZE: [f64; 2] = [1000.0, 700.0];
pub const DEFAULT_PIXELS_PER_MODEL_UNIT: f64 = 50.0;
pub const MIN_PIXELS_PER_MODEL_UNIT: f64 = 2.0;
pub const MAX_PIXELS_PER_MODEL_UNIT: f64 = 2_000.0;
pub const FIT_MARGIN_PIXELS: f64 = 64.0;
pub const GRID_TARGET_MAJOR_PIXELS: f64 = 96.0;

/// Presentation-only affine mapping from one exact camera paint to a newer
/// desired camera. The accepted SVG scene is already expressed in screen
/// coordinates, so this transform can be applied to its retained root group
/// without rebuilding geometry, annotations, or computed features.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedCameraTransform {
    pub translate: [f64; 2],
    pub scale: f64,
}

impl RetainedCameraTransform {
    pub fn between(exact: CanvasCamera, desired: CanvasCamera) -> Option<Self> {
        let scale = desired.pixels_per_model_unit / exact.pixels_per_model_unit;
        let translate = [
            SCREEN_SIZE[0] * 0.5
                + (exact.model_center[0] - desired.model_center[0]) * desired.pixels_per_model_unit
                - scale * SCREEN_SIZE[0] * 0.5,
            SCREEN_SIZE[1] * 0.5
                + (desired.model_center[1] - exact.model_center[1]) * desired.pixels_per_model_unit
                - scale * SCREEN_SIZE[1] * 0.5,
        ];
        (scale.is_finite() && scale > 0.0 && translate.into_iter().all(f64::is_finite))
            .then_some(Self { translate, scale })
    }

    pub fn map_screen_point(self, point: ScreenPoint) -> Option<ScreenPoint> {
        let mapped = ScreenPoint {
            x: self.translate[0] + self.scale * point.x,
            y: self.translate[1] + self.scale * point.y,
        };
        (mapped.x.is_finite() && mapped.y.is_finite()).then_some(mapped)
    }
}

/// Transient, visual-only canvas presentation owned by the demo adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasDisplayOptions {
    pub grid_visible: bool,
    /// Keeps context-only annotations in the SVG as inert hidden nodes so a
    /// retained hover frame can reveal them without rebuilding the scene.
    pub retain_contextual_annotations: bool,
}

/// Transient Offset-specific canvas state that must not be flattened into ordinary selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OffsetCanvasPresentation {
    pub pending: Vec<SelectionItem>,
    pub unavailable: Vec<SelectionItem>,
    pub unavailable_message: Option<String>,
    pub chain: Option<OffsetAuthoringChainPresentation>,
}

impl Default for CanvasDisplayOptions {
    fn default() -> Self {
        Self {
            grid_visible: true,
            retain_contextual_annotations: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveGridSpec {
    pub model_major_step: f64,
    pub major_pixels: f64,
    pub minor_pixels: f64,
    pub screen_origin: ScreenPoint,
}

/// Exact lightweight grid paint for one desired camera. Camera RAFs may
/// update these two retained paths without serializing the accepted scene.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedGridPresentation {
    pub model_major_step: f64,
    pub major_pixels: f64,
    pub minor_path: String,
    pub major_path: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedDatumAxisPresentation {
    pub visible: bool,
    pub start: ScreenPoint,
    pub end: ScreenPoint,
    pub label: ScreenPoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedFixedSizeTransform {
    pub inverse_scale: f64,
}

impl RetainedFixedSizeTransform {
    pub fn for_camera(transform: RetainedCameraTransform) -> Option<Self> {
        let inverse_scale = transform.scale.recip();
        (inverse_scale.is_finite() && inverse_scale > 0.0).then_some(Self { inverse_scale })
    }

    pub fn map_about(self, anchor: ScreenPoint, point: ScreenPoint) -> ScreenPoint {
        ScreenPoint {
            x: anchor.x + (point.x - anchor.x) * self.inverse_scale,
            y: anchor.y + (point.y - anchor.y) * self.inverse_scale,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasCamera {
    model_center: [f64; 2],
    pixels_per_model_unit: f64,
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
    /// Constructs one finite camera inside the canonical scale interval.
    pub fn new(model_center: [f64; 2], pixels_per_model_unit: f64) -> Option<Self> {
        (model_center.into_iter().all(f64::is_finite)
            && pixels_per_model_unit.is_finite()
            && (MIN_PIXELS_PER_MODEL_UNIT..=MAX_PIXELS_PER_MODEL_UNIT)
                .contains(&pixels_per_model_unit))
        .then_some(Self {
            model_center,
            pixels_per_model_unit,
        })
    }

    pub const fn model_center(self) -> [f64; 2] {
        self.model_center
    }

    pub const fn pixels_per_model_unit(self) -> f64 {
        self.pixels_per_model_unit
    }

    /// Returns the validated viewport represented by this camera.
    ///
    /// # Panics
    ///
    /// Panics only if a private camera mutation violates the finite center or
    /// canonical scale invariants enforced by [`CanvasCamera::new`].
    pub fn viewport(self) -> Viewport {
        Viewport::new(SCREEN_SIZE, self.model_center, self.pixels_per_model_unit)
            .expect("CanvasCamera construction and mutation preserve viewport invariants")
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn center_origin(&mut self) -> bool {
        if self.model_center == [0.0, 0.0] {
            return false;
        }
        self.model_center = [0.0, 0.0];
        true
    }

    pub fn zoom_about(&mut self, anchor: ScreenPoint, factor: f64) -> bool {
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
        let mut candidate = *self;
        candidate.pixels_per_model_unit = next_scale;
        let after = candidate.viewport().screen_to_model(anchor);
        candidate.model_center = [
            candidate.model_center[0] + before[0] - after[0],
            candidate.model_center[1] + before[1] - after[1],
        ];
        if !candidate.model_center.into_iter().all(f64::is_finite) {
            return false;
        }
        *self = candidate;
        true
    }

    pub fn pan_from(
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
        let model_center = [
            origin_center[0] - (current.x - origin.x) / self.pixels_per_model_unit,
            origin_center[1] + (current.y - origin.y) / self.pixels_per_model_unit,
        ];
        if !model_center.into_iter().all(f64::is_finite) {
            return false;
        }
        self.model_center = model_center;
        true
    }

    pub fn fit_scene(&mut self, scene: &EditorScene) -> bool {
        self.fit_model_bounds(scene.model_bounds())
    }

    /// Fits explicit finite model bounds into the canonical canvas.
    ///
    /// Empty, reversed, non-finite, or uncontainable bounds are rejected
    /// without changing the camera. A successful fit guarantees that the
    /// complete bounds map finitely inside the canonical 64 px margin while
    /// honoring the finite 2–2000 px/model-unit scale interval.
    pub fn fit_model_bounds(&mut self, bounds: Option<([f64; 2], [f64; 2])>) -> bool {
        let Some((minimum, maximum)) = bounds else {
            return false;
        };
        if !minimum.into_iter().all(f64::is_finite)
            || !maximum.into_iter().all(f64::is_finite)
            || minimum
                .into_iter()
                .zip(maximum)
                .any(|(minimum, maximum)| minimum > maximum)
        {
            return false;
        }

        let spans = [maximum[0] - minimum[0], maximum[1] - minimum[1]];
        if !spans.into_iter().all(f64::is_finite) {
            return false;
        }
        let available = [
            SCREEN_SIZE[0] - 2.0 * FIT_MARGIN_PIXELS,
            SCREEN_SIZE[1] - 2.0 * FIT_MARGIN_PIXELS,
        ];
        let model_center = [minimum[0] + spans[0] * 0.5, minimum[1] + spans[1] * 0.5];
        let mut pixels_per_model_unit = MAX_PIXELS_PER_MODEL_UNIT;
        for (span, available) in spans.into_iter().zip(available) {
            if span > 0.0 {
                pixels_per_model_unit = pixels_per_model_unit.min(available / span);
            }
        }
        if !model_center.into_iter().all(f64::is_finite)
            || !pixels_per_model_unit.is_finite()
            || pixels_per_model_unit < MIN_PIXELS_PER_MODEL_UNIT
        {
            return false;
        }
        pixels_per_model_unit = pixels_per_model_unit.min(MAX_PIXELS_PER_MODEL_UNIT);

        let candidate = Self {
            model_center,
            pixels_per_model_unit,
        };
        if !bounds_fit_with_margin(candidate, minimum, maximum) {
            return false;
        }

        *self = candidate;
        true
    }

    /// Fits finite native geometry, or returns an empty workplane to the canonical Origin view.
    pub fn fit_scene_or_reset(&mut self, scene: Option<&EditorScene>) -> bool {
        if scene.is_some_and(|scene| self.fit_scene(scene)) {
            return true;
        }
        self.reset();
        false
    }
}

fn bounds_fit_with_margin(camera: CanvasCamera, minimum: [f64; 2], maximum: [f64; 2]) -> bool {
    const CONTAINMENT_EPSILON_PIXELS: f64 = 1.0e-7;

    let viewport = camera.viewport();
    [
        minimum,
        [minimum[0], maximum[1]],
        [maximum[0], minimum[1]],
        maximum,
    ]
    .into_iter()
    .map(|point| viewport.model_to_screen(point))
    .all(|point| {
        point.x.is_finite()
            && point.y.is_finite()
            && point.x >= FIT_MARGIN_PIXELS - CONTAINMENT_EPSILON_PIXELS
            && point.x <= SCREEN_SIZE[0] - FIT_MARGIN_PIXELS + CONTAINMENT_EPSILON_PIXELS
            && point.y >= FIT_MARGIN_PIXELS - CONTAINMENT_EPSILON_PIXELS
            && point.y <= SCREEN_SIZE[1] - FIT_MARGIN_PIXELS + CONTAINMENT_EPSILON_PIXELS
    })
}

pub fn viewport() -> Viewport {
    CanvasCamera::default().viewport()
}

/// Typed failure from strict fitted static-scene composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StaticSceneCompositionError {
    /// A non-empty scene cannot satisfy the finite camera and containment contract.
    UnfittableModelBounds,
    /// The authoritative scene could not be projected onto the fitted camera.
    SceneReprojection(String),
}

impl fmt::Display for StaticSceneCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnfittableModelBounds => formatter.write_str(
                "scene model bounds cannot fit the canonical canvas at 2–2000 px/model-unit with a 64 px margin",
            ),
            Self::SceneReprojection(message) => {
                write!(formatter, "scene reprojection onto the fitted camera failed: {message}")
            }
        }
    }
}

impl Error for StaticSceneCompositionError {}

/// Composes a target-neutral, interaction-free canvas scene.
///
/// This is the narrow entry point for native reports and screenshots. It
/// deliberately supplies no browser selection, hover, draft, inference,
/// provisional, problem, or action state and uses the canonical geometry and
/// display policies.
pub fn compose_static_scene_svg(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    camera: CanvasCamera,
) -> String {
    static_scene_markup(scene, accepted, camera.viewport())
}

/// Fits the authoritative scene, then composes its interaction-free canvas.
///
/// Empty scenes use the canonical Origin camera. Non-empty scenes fail closed
/// if their bounds cannot satisfy the finite containment contract or if exact
/// reprojection onto that camera fails. The returned camera makes the exact
/// fitted mapping inspectable by native callers.
///
/// # Errors
///
/// Returns [`StaticSceneCompositionError`] rather than publishing clipped,
/// reset-camera, or non-finite static output.
pub fn compose_fitted_static_scene_svg(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
) -> Result<(CanvasCamera, String), StaticSceneCompositionError> {
    let mut camera = CanvasCamera::default();
    if let Some(bounds) = scene.and_then(EditorScene::model_bounds)
        && !camera.fit_model_bounds(Some(bounds))
    {
        return Err(StaticSceneCompositionError::UnfittableModelBounds);
    }
    let projected = scene
        .map(|scene| {
            let mut projected = scene.clone();
            projected
                .reproject_viewport(camera.viewport())
                .map_err(|error| {
                    StaticSceneCompositionError::SceneReprojection(error.to_string())
                })?;
            Ok(projected)
        })
        .transpose()?;
    let markup = compose_static_scene_svg(projected.as_ref(), accepted, camera);
    Ok((camera, markup))
}

fn static_scene_markup(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    viewport: Viewport,
) -> String {
    let geometry_policy = GeometryInteractionPolicy::default();
    let mut output = String::from("<g class=\"wb-accepted-scene\">");
    render_static_adaptive_grid(&mut output, viewport);
    if let Some(scene) = scene
        && geometry_policy.visibility.reference_geometry
    {
        render_static_datums(&mut output, scene, viewport);
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
            let role_class = if curve.role == GeometryRole::Construction {
                " construction"
            } else {
                ""
            };
            let origin_class = if curve.origin.is_implicit_construction() {
                " implicit-construction"
            } else {
                ""
            };
            let _ = write!(
                output,
                "<path class=\"wb-curve{role_class}{origin_class}\" d=\"{}\"/>",
                polyline_path(&curve.screen_polyline),
            );
        }
        output.push_str("<g class=\"wb-computed-geometry\">");
        for curve in scene
            .computed_curves
            .iter()
            .filter(|curve| curve.is_visible(geometry_policy))
        {
            let role_class = if curve.role == GeometryRole::Construction {
                " construction"
            } else {
                ""
            };
            let _ = write!(
                output,
                "<path class=\"wb-curve wb-computed-fillet{role_class}\" d=\"{}\"/>",
                polyline_path(&curve.screen_polyline),
            );
        }
        output.push_str("</g>");
    }
    output.push_str("</g><g class=\"wb-points\">");
    if let Some(scene) = scene {
        for point in scene
            .points
            .iter()
            .filter(|point| point.is_visible(geometry_policy))
        {
            let _ = write!(
                output,
                "<circle class=\"wb-point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\"/>",
                point.screen_position.x, point.screen_position.y,
            );
        }
    }
    output.push_str("</g><g class=\"wb-annotations\">");
    if let (Some(scene), Some(accepted)) = (scene, accepted)
        && scene.annotations_visible
    {
        render_static_annotations(&mut output, scene, accepted);
    }
    output.push_str("</g></g>");
    output
}

#[allow(clippy::too_many_lines)]
pub fn svg_markup(
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
pub fn svg_markup_with_pending(
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
pub fn svg_markup_with_context(
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
pub fn svg_markup_with_computed_context(
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
pub fn svg_markup_with_computed_context_and_action_stamp(
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
pub fn svg_markup_with_computed_context_action_stamp_and_display(
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
pub fn svg_markup_with_computed_context_action_stamp_display_and_provisional(
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
        for (scene_curve_index, curve) in scene
            .curves
            .iter()
            .enumerate()
            .filter(|(_, curve)| curve.is_visible(geometry_policy))
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
                    "<path id=\"wb-scene-curve-{}\" class=\"wb-curve{}{}{}{}{}{}{}{}{}\" d=\"{}\" ",
                    "data-persistent-id=\"{}\" {}",
                    "data-editor-segment=\"{}\" data-role=\"{}\" data-source-role=\"{}\" ",
                    "data-construction-origin=\"{}\" data-interactive=\"{}\" {}/>"
                ),
                scene_curve_index,
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
            if let Some(problem) = problem
                && has_problem
                && resolved_targets.insert(target)
            {
                let anchor = curve.screen_polyline[curve.screen_polyline.len() / 2];
                problem_marker(&mut problem_markers, anchor, Some(target), problem, false);
            }
        }
        render_computed_geometry(
            &mut output,
            scene,
            selection,
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
        for (scene_point_index, point) in scene
            .points
            .iter()
            .enumerate()
            .filter(|(_, point)| point.is_visible(geometry_policy))
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
                    "<circle id=\"wb-scene-point-{}\" class=\"wb-point{}{}{}{}{}{}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" ",
                    "style=\"transform-origin:{:.3}px {:.3}px\" ",
                    "data-persistent-id=\"{}\" {}data-interactive=\"{}\"/>"
                ),
                scene_point_index,
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
            if let Some(problem) = problem
                && has_problem
                && resolved_targets.insert(target)
            {
                problem_marker(
                    &mut problem_markers,
                    point.screen_position,
                    Some(target),
                    problem,
                    false,
                );
            }
        }
    } else {
        output.push_str("</g><g class=\"wb-points\">");
    }
    output.push_str("</g>");
    output.push_str("<g class=\"wb-annotations\">");
    if let (Some(scene), Some(accepted)) = (scene, accepted)
        && scene.annotations_visible
    {
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
            display.retain_contextual_annotations,
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

pub fn adaptive_grid_spec(viewport: Viewport) -> Option<AdaptiveGridSpec> {
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
    let Some(grid) = retained_grid_presentation(viewport) else {
        return;
    };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-grid\" aria-hidden=\"true\" data-grid-kind=\"adaptive-1-2-5\" ",
            "data-grid-major-model=\"{:.12}\" data-grid-major-pixels=\"{:.3}\">",
            "<path class=\"wb-grid-minor\" d=\"{}\"/>",
            "<path class=\"wb-grid-major\" d=\"{}\"/></g>"
        ),
        grid.model_major_step, grid.major_pixels, grid.minor_path, grid.major_path,
    );
}

fn render_static_adaptive_grid(output: &mut String, viewport: Viewport) {
    let Some(grid) = retained_grid_presentation(viewport) else {
        return;
    };
    let _ = write!(
        output,
        concat!(
            "<g class=\"wb-grid\">",
            "<path class=\"wb-grid-minor\" d=\"{}\"/>",
            "<path class=\"wb-grid-major\" d=\"{}\"/></g>"
        ),
        grid.minor_path, grid.major_path,
    );
}

pub fn retained_grid_presentation(viewport: Viewport) -> Option<RetainedGridPresentation> {
    let spec = adaptive_grid_spec(viewport)?;
    Some(RetainedGridPresentation {
        model_major_step: spec.model_major_step,
        major_pixels: spec.major_pixels,
        minor_path: grid_path(spec.screen_origin, spec.minor_pixels, viewport.screen_size)?,
        major_path: grid_path(spec.screen_origin, spec.major_pixels, viewport.screen_size)?,
    })
}

pub fn retained_datum_axis_presentation(
    viewport: Viewport,
    datum: SketchDatum,
) -> Option<RetainedDatumAxisPresentation> {
    let origin = viewport.model_to_screen([0.0, 0.0]);
    let [width, height] = viewport.screen_size;
    match datum {
        SketchDatum::Origin => None,
        SketchDatum::XAxis => Some(RetainedDatumAxisPresentation {
            visible: (0.0..=height).contains(&origin.y),
            start: ScreenPoint {
                x: 0.0,
                y: origin.y,
            },
            end: ScreenPoint {
                x: width,
                y: origin.y,
            },
            label: ScreenPoint {
                x: width - 20.0,
                y: (origin.y - 8.0).max(14.0),
            },
        }),
        SketchDatum::YAxis => Some(RetainedDatumAxisPresentation {
            visible: (0.0..=width).contains(&origin.x),
            start: ScreenPoint {
                x: origin.x,
                y: height,
            },
            end: ScreenPoint {
                x: origin.x,
                y: 0.0,
            },
            label: ScreenPoint {
                x: (origin.x + 9.0).min(width - 18.0),
                y: 18.0,
            },
        }),
    }
}

fn grid_path(origin: ScreenPoint, spacing: f64, screen_size: [f64; 2]) -> Option<String> {
    const MAX_GRID_PATH_LINES: usize = 4_096;

    if !origin.x.is_finite()
        || !origin.y.is_finite()
        || !spacing.is_finite()
        || spacing <= 0.0
        || !screen_size
            .into_iter()
            .all(|extent| extent.is_finite() && extent >= 0.0)
    {
        return None;
    }
    let mut path = String::new();
    let mut line_count = 0_usize;
    let mut x = origin.x.rem_euclid(spacing);
    while x <= screen_size[0] {
        if line_count == MAX_GRID_PATH_LINES {
            return None;
        }
        let _ = write!(path, "M{x:.3} 0V{:.3}", screen_size[1]);
        line_count += 1;
        let next = x + spacing;
        if !next.is_finite() || next <= x {
            return None;
        }
        x = next;
    }
    let mut y = origin.y.rem_euclid(spacing);
    while y <= screen_size[1] {
        if line_count == MAX_GRID_PATH_LINES {
            return None;
        }
        let _ = write!(path, "M0 {y:.3}H{:.3}", screen_size[0]);
        line_count += 1;
        let next = y + spacing;
        if !next.is_finite() || next <= y {
            return None;
        }
        y = next;
    }
    Some(path)
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
    for (datum_index, datum) in scene.datums.iter().enumerate() {
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
                render_axis_datum(output, datum_index, datum, &state_classes, viewport);
            }
        }
    }
    output.push_str("</g>");
}

fn render_static_datums(output: &mut String, scene: &EditorScene, viewport: Viewport) {
    output.push_str("<g class=\"wb-reference-geometry\">");
    for datum in &scene.datums {
        let Some(presentation) = retained_datum_axis_presentation(viewport, datum.datum) else {
            continue;
        };
        if !presentation.visible {
            continue;
        }
        let (label, axis_class) = if datum.datum == SketchDatum::XAxis {
            ("X", "wb-datum-x-axis")
        } else {
            ("Y", "wb-datum-y-axis")
        };
        let _ = write!(
            output,
            concat!(
                "<g class=\"wb-datum wb-datum-axis {}\">",
                "<path class=\"wb-datum-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                "<text class=\"wb-datum-label\" x=\"{:.3}\" y=\"{:.3}\">{}</text></g>"
            ),
            axis_class,
            presentation.start.x,
            presentation.start.y,
            presentation.end.x,
            presentation.end.y,
            presentation.label.x,
            presentation.label.y,
            label,
        );
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
    for (guide_index, guide) in scene.curve_control_guides.iter().enumerate() {
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
                "<path id=\"wb-scene-control-guide-{}\" class=\"{}{}\" data-control-guide=\"{}\" data-curve-id=\"{}\" ",
                "d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>"
            ),
            guide_index,
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

pub fn render_curve_controls(output: &mut String, scene: &EditorScene, hover: EditorHoverState) {
    if scene.curve_controls.is_empty() {
        return;
    }
    output.push_str("<g class=\"wb-curve-control-cage\">");
    for (control_index, control) in scene.curve_controls.iter().enumerate() {
        // Stored design-point aliases keep the ordinary point presentation and
        // pointer owner. They remain in the headless catalog so guides can use
        // their exact anchors, but painting a second grip would falsely imply a
        // second selectable object over the same point.
        if !matches!(control.interaction, SceneCurveControlInteraction::Direct) {
            continue;
        }
        render_curve_control(output, control_index, control, hover);
    }
    output.push_str("</g>");
}

fn render_curve_control(
    output: &mut String,
    control_index: usize,
    control: &SceneCurveControl,
    hover: EditorHoverState,
) {
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
            "<g id=\"wb-scene-control-{}\" class=\"wb-curve-control{}{}\" role=\"img\" aria-label=\"{}\" ",
            "aria-disabled=\"{}\" data-control-role=\"{}\" data-curve-id=\"{}\" ",
            "data-editor-segment=\"{}\" pointer-events=\"none\">"
        ),
        control_index,
        if hovered { " hovered" } else { "" },
        if read_only { " read-only" } else { "" },
        escape(&label),
        read_only,
        role,
        control.id.curve,
        control.owner.segment,
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
    let _ = write!(
        output,
        "<text class=\"wb-curve-control-tooltip{}\" x=\"{:.3}\" y=\"{:.3}\" aria-hidden=\"true\">{}</text>",
        if hovered { "" } else { " context-hidden" },
        control.screen_position.x + 10.0,
        control.screen_position.y - 10.0,
        escape(&label),
    );
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

pub const fn curve_control_kind_key(kind: DocumentCurveControlKind) -> &'static str {
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
    datum_index: usize,
    datum: &SceneDatum,
    state_classes: &str,
    viewport: Viewport,
) {
    let is_x = datum.datum == SketchDatum::XAxis;
    let Some(presentation) = retained_datum_axis_presentation(viewport, datum.datum) else {
        return;
    };
    let (key, label, axis_class) = if is_x {
        ("x-axis", "X", "wb-datum-x-axis")
    } else {
        ("y-axis", "Y", "wb-datum-y-axis")
    };
    let _ = write!(
        output,
        concat!(
            "<g id=\"wb-scene-datum-{}\" class=\"wb-datum wb-datum-axis {}{}\" role=\"button\" tabindex=\"{}\" ",
            "aria-label=\"{} axis · protected infinite intrinsic reference\" ",
            "data-editor-item=\"datum\" data-datum=\"{}\" data-protected=\"true\"{}>",
            "<path class=\"wb-datum-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
            "<path class=\"wb-datum-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
            "<text class=\"wb-datum-label\" x=\"{:.3}\" y=\"{:.3}\">{}</text></g>"
        ),
        datum_index,
        axis_class,
        state_classes,
        if presentation.visible { "0" } else { "-1" },
        label,
        key,
        if presentation.visible {
            ""
        } else {
            " aria-hidden=\"true\" style=\"display:none\""
        },
        presentation.start.x,
        presentation.start.y,
        presentation.end.x,
        presentation.end.y,
        presentation.start.x,
        presentation.start.y,
        presentation.end.x,
        presentation.end.y,
        presentation.label.x,
        presentation.label.y,
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

fn render_computed_geometry(
    output: &mut String,
    scene: &EditorScene,
    selection: &[SelectionItem],
    hover: EditorHoverState,
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
    for (computed_curve_index, curve) in scene
        .computed_curves
        .iter()
        .enumerate()
        .filter(|(_, curve)| curve.is_visible(geometry_policy))
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
                "<g id=\"wb-scene-computed-curve-{}\" class=\"wb-computed-item{}{}{}{}\" {}",
                "data-feature-id=\"{}\" data-feature-corner-id=\"{}\" ",
                "data-computed-evaluation=\"{}\" data-computed-edge=\"{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\">",
                "<path class=\"wb-curve wb-computed-fillet{}\" data-role=\"{}\" ",
                "data-interactive=\"{}\" d=\"{}\"/>"
            ),
            computed_curve_index,
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

pub fn fillet_action_key(action: SceneFilletActionId) -> String {
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

pub fn fillet_action_from_key(key: &str) -> Option<SceneFilletActionId> {
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

pub fn fillet_action_panel_markup_with_stamp(
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

pub fn fillet_continuation_status_markup(scene: &EditorScene) -> String {
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
    retain_contextual: bool,
) {
    let visibility_context = hover
        .context_owner
        .or_else(|| hover.target.map(EditorHoverTarget::item));
    for (annotation_index, annotation) in scene.annotations.iter().enumerate() {
        let visible = annotation.is_visible(selection, visibility_context, problem_items)
            || scene.show_all_constraint_annotations
                && matches!(annotation.kind, SceneAnnotationKind::Constraint(_));
        if !visible && !retain_contextual {
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
            "{}{}{}{}{}{}{}{}{}",
            if selected { " selected" } else { "" },
            if is_hovered { " hovered" } else { "" },
            if visible { "" } else { " context-hidden" },
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
        } else if !visible {
            (
                format!("data-editor-item=\"{editor_kind}\" data-persistent-id=\"{id}\" "),
                "tabindex=\"-1\" role=\"button\" aria-hidden=\"true\"".to_owned(),
            )
        } else {
            (
                format!("data-editor-item=\"{editor_kind}\" data-persistent-id=\"{id}\" "),
                "tabindex=\"0\" role=\"button\"".to_owned(),
            )
        };
        let _ = write!(
            output,
            "<g class=\"wb-annotation wb-{editor_kind}{class}\" id=\"wb-scene-annotation-{annotation_index}\" aria-label=\"{escaped_label}\" {identity}data-{editor_kind}-kind=\"{kind}\"{}{} data-annotation-kind=\"{kind}\" {accessibility}>",
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

        if let Some(problem) = problem
            && has_problem
        {
            let target = match annotation.item {
                SelectionItem::Constraint(id) => EditorProblemTarget::Constraint(id),
                SelectionItem::Dimension(id) => EditorProblemTarget::Dimension(id),
                SelectionItem::Point(_)
                | SelectionItem::Curve(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => continue,
            };
            if resolved_targets.insert(target)
                && let Some(anchor) = annotation_anchor(&annotation.geometry)
            {
                problem_marker(problem_markers, anchor, Some(target), problem, false);
            }
        }
    }
}

fn render_static_annotations(
    output: &mut String,
    scene: &EditorScene,
    accepted: &SketchAcceptedDocumentState,
) {
    for annotation in &scene.annotations {
        let visible = annotation.is_visible(&[], None, &[])
            || scene.show_all_constraint_annotations
                && matches!(annotation.kind, SceneAnnotationKind::Constraint(_));
        if !visible {
            continue;
        }
        let (editor_kind, visible_text) = match annotation.item {
            SelectionItem::Constraint(_) => {
                ("constraint", annotation.visible_text.as_deref().map(escape))
            }
            SelectionItem::Dimension(id) => {
                let Some(_dimension) = accepted.document().dimension(id) else {
                    continue;
                };
                ("dimension", annotation.visible_text.as_deref().map(escape))
            }
            SelectionItem::Point(_)
            | SelectionItem::Curve(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => continue,
        };
        let mut classes = format!("wb-annotation wb-{editor_kind}");
        if annotation.suppressed {
            classes.push_str(" suppressed");
        }
        if annotation.reference {
            classes.push_str(" reference");
        }
        let _ = write!(output, "<g class=\"{classes}\">");
        static_annotation_geometry(
            output,
            annotation.kind,
            &annotation.geometry,
            visible_text.as_deref().unwrap_or(""),
            annotation.label_bounds,
        );
        output.push_str("</g>");
    }
}

#[allow(clippy::too_many_lines)]
fn static_annotation_geometry(
    output: &mut String,
    kind: SceneAnnotationKind,
    geometry: &SceneAnnotationGeometry,
    visible_text: &str,
    label_bounds: Option<geosolve_constraint_editor::SceneAnnotationLabelBounds>,
) {
    let arrowheads = annotation_arrowheads(geometry);
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
                    concat!(
                        "<g class=\"wb-constraint-symbol\" transform=\"translate({:.3} {:.3}) rotate({:.3})\">",
                        "<g class=\"wb-camera-fixed-size\" style=\"transform-origin:0px 0px\">{}</g></g>"
                    ),
                    marker.anchor.x,
                    marker.anchor.y,
                    marker.rotation_radians.to_degrees(),
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
                    "<g class=\"wb-constraint-symbol\"><g class=\"wb-camera-fixed-size\" ",
                    "style=\"transform-origin:{:.3}px {:.3}px\">",
                    "<path class=\"wb-right-angle\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "</g></g>"
                ),
                corner.x,
                corner.y,
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
                    "{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
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
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
                ),
                measurement_start.x,
                measurement_start.y,
                edge.x,
                edge.y,
                label_anchor.x,
                label_anchor.y,
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
                    "{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>"
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
                    "<path class=\"wb-dimension-line\" d=\"M{:.3} {:.3}L{:.3} {:.3}\"/>",
                    origin.x, origin.y, anchor.x, anchor.y,
                );
            }
            let _ = write!(
                output,
                "{}{}<text x=\"{:.3}\" y=\"{:.3}\">{}</text>",
                arrowheads,
                label_mask(label_bounds),
                anchor.x,
                anchor.y + 4.0,
                visible_text,
            );
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
pub fn annotation_geometry(
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
                    "<g class=\"wb-constraint-symbol{}\" transform=\"translate({:.3} {:.3}) rotate({:.3})\" data-annotation-marker=\"{index}\" data-marker-rotation-radians=\"{:.6}\"><g class=\"wb-camera-fixed-size\" style=\"transform-origin:0px 0px\"><circle class=\"wb-annotation-hit\" r=\"{:.3}\"/>{}</g></g>",
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
                    "<g class=\"wb-camera-fixed-size\" style=\"transform-origin:{:.3}px {:.3}px\">",
                    "<path class=\"wb-right-angle\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "<path class=\"wb-annotation-path-hit\" d=\"M{:.3} {:.3}L{:.3} {:.3}L{:.3} {:.3}\"/>",
                    "</g></g>"
                ),
                corner.x,
                corner.y,
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

pub fn problem_selection_item(
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

pub const fn constraint_glyph(
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

pub const fn dimension_kind(definition: &DocumentDimensionDefinition) -> &'static str {
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
        let Some(target) = target else {
            return;
        };
        match target {
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

pub fn construction_markup(preview: &ConstructionPreview, viewport: Viewport) -> String {
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

pub fn construction_geometry_markup(
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
    use super::*;
    use geosolve_sketch::{
        DocumentSolveRequest, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
    };

    #[test]
    fn camera_fit_rejects_empty_and_invalid_bounds_without_mutation() {
        let original = CanvasCamera::new([4.0, -3.0], 75.0).expect("valid camera");
        let mut camera = original;

        assert!(!camera.fit_model_bounds(None));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([2.0, 0.0], [1.0, 1.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([f64::NAN, 0.0], [1.0, 1.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([0.0, 0.0], [f64::INFINITY, 1.0]))));
        assert_eq!(camera, original);
    }

    #[test]
    fn camera_pan_rejects_finite_overflow_without_mutation() {
        let mut camera = CanvasCamera::default();
        let original = camera;

        assert!(!camera.pan_from(
            [f64::MAX, 0.0],
            ScreenPoint {
                x: -f64::MAX,
                y: 0.0,
            },
            ScreenPoint {
                x: f64::MAX,
                y: 0.0,
            },
        ));
        assert_eq!(camera, original);
    }

    #[test]
    fn grid_path_rejects_invalid_or_unbounded_work() {
        let origin = ScreenPoint { x: 0.0, y: 0.0 };
        assert!(grid_path(origin, -1.0, SCREEN_SIZE).is_none());
        assert!(grid_path(origin, 0.0, SCREEN_SIZE).is_none());
        assert!(grid_path(origin, f64::NAN, SCREEN_SIZE).is_none());
        assert!(grid_path(origin, 1.0, [f64::INFINITY, 700.0]).is_none());
        assert!(grid_path(origin, 1.0e-300, SCREEN_SIZE).is_none());
    }

    #[test]
    fn camera_fit_handles_points_degenerate_axes_and_off_origin_bounds() {
        let mut camera = CanvasCamera::default();

        assert!(camera.fit_model_bounds(Some(([12.0, -8.0], [12.0, -8.0]))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [12.0_f64.to_bits(), (-8.0_f64).to_bits()]
        );
        assert_eq!(
            camera.pixels_per_model_unit.to_bits(),
            MAX_PIXELS_PER_MODEL_UNIT.to_bits()
        );

        assert!(camera.fit_model_bounds(Some(([100.0, -40.0], [100.0 + 1.0e-12, -38.0],))));
        assert_eq!(camera.model_center[1].to_bits(), (-39.0_f64).to_bits());
        assert!((camera.pixels_per_model_unit - 286.0).abs() <= f64::EPSILON);

        assert!(camera.fit_model_bounds(Some(([100.0, -40.0], [104.0, -30.0]))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [102.0_f64.to_bits(), (-35.0_f64).to_bits()]
        );
        assert!((camera.pixels_per_model_unit - 57.2).abs() <= 1.0e-12);
    }

    #[test]
    fn camera_fit_rejects_uncontainable_and_overflowing_finite_bounds() {
        let mut camera = CanvasCamera::default();
        let original = camera;

        assert!(!camera.fit_model_bounds(Some(([-1_000.0, 0.0], [1_000.0, 0.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([-f64::MAX, -f64::MAX], [f64::MAX, f64::MAX],))));
        assert_eq!(camera, original);

        assert!(camera.fit_model_bounds(Some(([f64::MAX, f64::MAX], [f64::MAX, f64::MAX],))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [f64::MAX.to_bits(), f64::MAX.to_bits()]
        );
        assert_eq!(
            camera.pixels_per_model_unit.to_bits(),
            MAX_PIXELS_PER_MODEL_UNIT.to_bits()
        );
    }

    #[test]
    fn static_composer_uses_canonical_empty_camera_without_interaction_state() {
        let (camera, markup) =
            compose_fitted_static_scene_svg(None, None).expect("canonical empty scene");

        assert_eq!(camera, CanvasCamera::default());
        assert_eq!(markup, compose_static_scene_svg(None, None, camera));
        assert!(markup.contains("class=\"wb-grid\""));
        assert!(!markup.contains("data-"));
        assert!(!markup.contains(" selected"));
        assert!(!markup.contains(" geometry-hovered"));
        assert!(!markup.contains(" authoring-pending"));
    }

    #[test]
    fn populated_static_composition_contains_only_durable_paint() {
        let mut document = SketchDocument::new(8.0).expect("document");
        document
            .add_rectangle("static renderer", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            CanvasCamera::default().viewport(),
            0.25,
        )
        .expect("scene");
        scene.set_show_all_constraint_annotations(true);
        assert!(scene.update_annotation_values(accepted));

        let (camera, first) = compose_fitted_static_scene_svg(Some(&scene), Some(accepted))
            .expect("fitted static rectangle");
        let (_, second) = compose_fitted_static_scene_svg(Some(&scene), Some(accepted))
            .expect("repeat fitted static rectangle");
        assert_eq!(first, second);
        assert!(first.contains("class=\"wb-grid\""));
        assert!(first.contains("class=\"wb-reference-geometry\""));
        assert!(first.contains("class=\"wb-curve\""));
        assert!(first.contains("class=\"wb-point\""));
        assert!(first.contains("class=\"wb-annotations\""));
        assert!(first.contains("wb-dimension"));
        for forbidden in [
            "data-",
            "tabindex=",
            "role=\"button\"",
            "wb-datum-hit",
            "wb-computed-hit",
            "wb-annotation-hit",
            "wb-annotation-path-hit",
            "wb-fillet-affordance",
            "wb-fillet-action",
            "wb-curve-control",
            "wb-draft",
            "wb-inference",
            "wb-error-overlay",
        ] {
            assert!(
                !first.contains(forbidden),
                "leaked static token: {forbidden}"
            );
        }

        let (minimum, maximum) = scene.model_bounds().expect("rectangle bounds");
        assert!(bounds_fit_with_margin(camera, minimum, maximum));
    }

    #[test]
    fn fitted_static_composition_rejects_nonempty_uncontainable_scene() {
        let mut document = SketchDocument::new(8.0).expect("document");
        document
            .add_rectangle("unfit renderer", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let accepted = session.accepted_state().expect("accepted rectangle");
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            CanvasCamera::default().viewport(),
            0.25,
        )
        .expect("scene");
        scene.points[0].model_position = [-1_000.0, 0.0];
        scene.points[1].model_position = [1_000.0, 0.0];

        assert_eq!(
            compose_fitted_static_scene_svg(Some(&scene), Some(accepted)),
            Err(StaticSceneCompositionError::UnfittableModelBounds)
        );
    }
}
