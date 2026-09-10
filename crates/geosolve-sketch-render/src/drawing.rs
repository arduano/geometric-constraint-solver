// SPDX-License-Identifier: GPL-3.0-or-later
//! Numeric, presentation-only drawing commands. No SVG is parsed or rasterized.
//! The retained editor remains the sole authority for geometry, picking and edits.

use crate::{
    CanvasDisplayOptions, OffsetCanvasPresentation, adaptive_grid_spec,
    retained_datum_axis_presentation,
};
use geosolve_constraint_editor::{
    AdvancedConstructionKind, ComputedFeatureProblemMetadata, ConstructionPreview,
    ConstructionPreviewGeometry, DraftGuideClassification, DraftGuideGeometry,
    DraftInferenceResolution, DraftInferenceStatus, EditorHoverState, EditorHoverTarget,
    EditorProblemMetadata, EditorProblemScope, EditorProblemTarget, EditorScene,
    GeometryInteractionPolicy, OffsetAuthoringChainPresentation, OffsetTraversal,
    SceneAnnotationGeometry, SceneAnnotationKind, SceneConstraintGlyph,
    SceneCurveControlGripGeometry, SceneCurveControlGuideKind, SceneCurveControlInteraction,
    SceneFilletActionAvailability, SceneFilletActionId, SceneFilletActionTarget, ScreenPoint,
    SelectionItem, Viewport,
};
use geosolve_sketch::{
    DocumentArcSweep, DocumentCurveControlKind, GeometryRole, SketchAcceptedDocumentState,
    SketchDatum,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Ordered immutable drawing frame in the editor's logical screen coordinates.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawFrame {
    pub format: &'static str,
    pub view_box: [f64; 4],
    pub background: &'static str,
    pub provenance: BTreeMap<String, String>,
    pub items: Vec<DrawItem>,
}

/// One painted occurrence. Metadata describes it; it never authorizes an edit.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawItem {
    pub id: String,
    pub layer: &'static str,
    pub semantic_key: Option<String>,
    pub class_name: String,
    pub interactive: bool,
    pub accessible_label: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub style: DrawStyle,
    #[serde(flatten)]
    pub geometry: DrawGeometry,
}

/// All positions and lengths are finite logical screen values; rotation is radians.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DrawGeometry {
    Polyline {
        points: Vec<[f64; 2]>,
        closed: bool,
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Ellipse {
        center: [f64; 2],
        radii: [f64; 2],
        rotation: f64,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
    },
    Text {
        position: [f64; 2],
        text: String,
        rotation: f64,
    },
}

/// Resolved paint, independent of DOM selectors. Stroke widths are CSS pixels
/// when `nonScalingStroke` is true, matching the existing SVG paint contract.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: f64,
    pub dash: Vec<f64>,
    pub opacity: f64,
    pub line_cap: &'static str,
    pub line_join: &'static str,
    pub non_scaling_stroke: bool,
    pub font_family: &'static str,
    pub font_size: f64,
    pub font_weight: u16,
    pub text_anchor: &'static str,
    pub text_baseline: &'static str,
    pub letter_spacing: f64,
    pub shadow: Option<DrawShadow>,
}

/// Optional soft paint halo; never used for hit testing.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawShadow {
    pub color: String,
    pub blur: f64,
    pub offset: [f64; 2],
}

impl Default for DrawStyle {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: 0.0,
            dash: Vec::new(),
            opacity: 1.0,
            line_cap: "round",
            line_join: "round",
            non_scaling_stroke: true,
            font_family: "ui-monospace,monospace",
            font_size: 12.0,
            font_weight: 500,
            text_anchor: "start",
            text_baseline: "alphabetic",
            letter_spacing: 0.0,
            shadow: None,
        }
    }
}
impl DrawStyle {
    fn stroke(color: &str, width: f64) -> Self {
        Self {
            stroke: Some(color.into()),
            stroke_width: width,
            ..Self::default()
        }
    }
    fn fill(color: &str) -> Self {
        Self {
            fill: Some(color.into()),
            ..Self::default()
        }
    }
    fn dashed(mut self, dash: &[f64]) -> Self {
        self.dash = dash.to_vec();
        self
    }
    fn glow(mut self, color: &str, blur: f64) -> Self {
        self.shadow = Some(DrawShadow {
            color: color.into(),
            blur,
            offset: [0.0, 0.0],
        });
        self
    }
}

/// Invalid presentation is rejected as a whole, preserving a host's last complete frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawFrameError(pub String);
impl fmt::Display for DrawFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DrawFrameError {}

fn xy(p: ScreenPoint) -> [f64; 2] {
    [p.x, p.y]
}
fn sp(p: [f64; 2]) -> ScreenPoint {
    ScreenPoint { x: p[0], y: p[1] }
}
fn semantic(item: SelectionItem) -> String {
    match item {
        SelectionItem::Point(id) => format!("point:{id}"),
        SelectionItem::Curve(span) => format!("curve:{}:{}", span.curve, span.segment),
        SelectionItem::Constraint(id) => format!("constraint:{id}"),
        SelectionItem::Dimension(id) => format!("dimension:{id}"),
        SelectionItem::Datum(datum) => format!("datum:{datum:?}"),
        SelectionItem::Feature(id) => format!("feature:{id}"),
        SelectionItem::FeatureCorner(owner) => {
            format!("feature-corner:{}:{}", owner.feature, owner.corner)
        }
    }
}
fn hovered(hover: EditorHoverState, item: SelectionItem) -> bool {
    matches!(hover.target, Some(EditorHoverTarget::Geometry(target)) if target == item)
}
fn role_key(role: GeometryRole) -> &'static str {
    if role == GeometryRole::Profile {
        "profile"
    } else {
        "construction"
    }
}

struct Painter {
    frame: DrawFrame,
    occurrences: BTreeMap<String, usize>,
    invalid: Option<DrawFrameError>,
}
impl Painter {
    fn arc(
        &mut self,
        start: ScreenPoint,
        end: ScreenPoint,
        radius: f64,
        large: bool,
        clockwise: bool,
    ) -> Vec<ScreenPoint> {
        if !start.x.is_finite()
            || !start.y.is_finite()
            || !end.x.is_finite()
            || !end.y.is_finite()
            || !radius.is_finite()
            || radius < 0.0
        {
            self.invalid = Some(DrawFrameError("invalid circular drawing geometry".into()));
            return Vec::new();
        }
        endpoint_arc(start, end, radius, large, clockwise)
    }

    fn push(
        &mut self,
        base: &str,
        layer: &'static str,
        class: &str,
        key: Option<&str>,
        style: DrawStyle,
        geometry: DrawGeometry,
    ) -> &mut DrawItem {
        let occurrence = self.occurrences.entry(base.into()).or_default();
        let id = format!("{base}/{occurrence}");
        *occurrence += 1;
        self.frame.items.push(DrawItem {
            id,
            layer,
            semantic_key: key.map(str::to_owned),
            class_name: class.into(),
            interactive: false,
            accessible_label: None,
            metadata: BTreeMap::new(),
            style,
            geometry,
        });
        self.frame
            .items
            .last_mut()
            .expect("just appended draw item")
    }
    fn line(
        &mut self,
        base: &str,
        layer: &'static str,
        class: &str,
        key: Option<&str>,
        style: DrawStyle,
        points: &[ScreenPoint],
    ) -> &mut DrawItem {
        self.push(
            base,
            layer,
            class,
            key,
            style,
            DrawGeometry::Polyline {
                points: points.iter().copied().map(xy).collect(),
                closed: false,
            },
        )
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "numeric geometry and explicit paint identity remain separate"
    )]
    fn circle(
        &mut self,
        base: &str,
        layer: &'static str,
        class: &str,
        key: Option<&str>,
        style: DrawStyle,
        center: ScreenPoint,
        radius: f64,
    ) -> &mut DrawItem {
        self.push(
            base,
            layer,
            class,
            key,
            style,
            DrawGeometry::Circle {
                center: xy(center),
                radius,
            },
        )
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "numeric geometry and explicit paint identity remain separate"
    )]
    fn text(
        &mut self,
        base: &str,
        layer: &'static str,
        class: &str,
        key: Option<&str>,
        style: DrawStyle,
        position: ScreenPoint,
        text: &str,
    ) -> &mut DrawItem {
        self.push(
            base,
            layer,
            class,
            key,
            style,
            DrawGeometry::Text {
                position: xy(position),
                text: text.into(),
                rotation: 0.0,
            },
        )
    }
}

impl DrawFrame {
    /// Rejects non-finite or malformed paint without changing accepted geometry.
    ///
    /// # Errors
    /// Returns an error for invalid viewport, duplicate IDs or invalid numeric paint.
    pub fn validate(&self) -> Result<(), DrawFrameError> {
        if !self.view_box.iter().all(|x| x.is_finite())
            || self.view_box[2] <= 0.0
            || self.view_box[3] <= 0.0
        {
            return Err(DrawFrameError("invalid drawing viewport".into()));
        }
        let mut ids = BTreeSet::new();
        for item in &self.items {
            let finite = |p: &[f64]| p.iter().all(|x| x.is_finite());
            let valid = match &item.geometry {
                DrawGeometry::Polyline { points, .. } => points.iter().all(|p| finite(p)),
                DrawGeometry::Circle { center, radius } => {
                    finite(center) && radius.is_finite() && *radius >= 0.0
                }
                DrawGeometry::Ellipse {
                    center,
                    radii,
                    rotation,
                } => {
                    finite(center)
                        && finite(radii)
                        && radii.iter().all(|r| *r >= 0.0)
                        && rotation.is_finite()
                }
                DrawGeometry::Rect {
                    x,
                    y,
                    width,
                    height,
                    radius,
                } => {
                    finite(&[*x, *y, *width, *height, *radius])
                        && *width >= 0.0
                        && *height >= 0.0
                        && *radius >= 0.0
                }
                DrawGeometry::Text {
                    position, rotation, ..
                } => finite(position) && rotation.is_finite(),
            };
            let s = &item.style;
            if !valid
                || !ids.insert(&item.id)
                || !finite(&[s.stroke_width, s.opacity, s.font_size, s.letter_spacing])
                || s.stroke_width < 0.0
                || s.font_size <= 0.0
                || !(0.0..=1.0).contains(&s.opacity)
                || s.dash.iter().any(|x| !x.is_finite() || *x < 0.0)
                || s.shadow
                    .as_ref()
                    .is_some_and(|x| !x.blur.is_finite() || x.blur < 0.0 || !finite(&x.offset))
            {
                return Err(DrawFrameError(format!("invalid drawing item {}", item.id)));
            }
        }
        Ok(())
    }
}

/// Composes the same interactive scene inputs as the SVG presentation, using
/// only numeric primitives. It does not mutate or authenticate editor authority.
///
/// # Errors
/// Returns an error for an invalid viewport, excessive grid work, or non-finite paint.
/// The caller must retain its previous complete frame on failure.
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive presentation dispatch preserves one auditable paint order"
)]
pub fn compose_draw_frame(
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
) -> Result<DrawFrame, DrawFrameError> {
    Viewport::new(
        viewport.screen_size,
        viewport.model_center,
        viewport.pixels_per_model_unit,
    )
    .map_err(|error| DrawFrameError(error.to_string()))?;
    let mut provenance = BTreeMap::new();
    if let Some(accepted) = accepted {
        let identity = accepted.identity();
        let input = accepted.input();
        provenance.insert("scene".into(), "accepted".into());
        provenance.insert("document".into(), identity.document().to_string());
        provenance.insert("revision".into(), identity.revision().get().to_string());
        for (name, revision, bytes) in [
            (
                "parameter",
                input.parameter_revision(),
                input.parameter_digest().bytes(),
            ),
            (
                "external",
                input.external_snapshot_set_revision(),
                input.external_snapshot_set_digest().bytes(),
            ),
            (
                "activation",
                input.effective_activation_revision(),
                input.activation_digest().bytes(),
            ),
        ] {
            provenance.insert(format!("{name}Revision"), revision.to_string());
            provenance.insert(
                format!("{name}Digest"),
                bytes
                    .iter()
                    .flat_map(|byte| {
                        [
                            char::from(b"0123456789abcdef"[usize::from(byte >> 4)]),
                            char::from(b"0123456789abcdef"[usize::from(byte & 15)]),
                        ]
                    })
                    .collect(),
            );
        }
    } else if let Some(scene) = scene.filter(|scene| scene.is_detached_presentation()) {
        provenance.insert("scene".into(), "accepted-presentation".into());
        provenance.insert(
            "document".into(),
            scene.presentation_document().id().to_string(),
        );
        provenance.insert("revision".into(), scene.accepted_revision.to_string());
    } else {
        provenance.insert("scene".into(), "none".into());
    }
    let mut p = Painter {
        frame: DrawFrame {
            format: "geosolve-draw-frame-v1",
            view_box: [0.0, 0.0, viewport.screen_size[0], viewport.screen_size[1]],
            background: "#151619",
            provenance,
            items: Vec::new(),
        },
        occurrences: BTreeMap::new(),
        invalid: None,
    };
    p.frame.validate()?;
    let problem_items = problem
        .map(|x| {
            x.targets
                .iter()
                .filter_map(|x| crate::problem_selection_item(*x, scene))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let related = scene
        .map(|s| {
            s.annotations.iter().filter(|a| selection.contains(&a.item)
        || matches!(hover.target,Some(EditorHoverTarget::Annotation(o)) if o.item==a.item))
        .flat_map(|a| a.operands.iter().copied()).collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if display.grid_visible {
        draw_grid(&mut p, viewport)?;
    }
    if let Some(scene) = scene {
        if geometry_policy.visibility.reference_geometry {
            draw_datums(&mut p, scene, selection, hover, &related, viewport);
        }
        draw_curves(
            &mut p,
            scene,
            selection,
            pending,
            provisional,
            hover,
            &related,
            problem,
            computed_problems,
            geometry_policy,
            offset,
        );
        draw_computed(
            &mut p,
            scene,
            selection,
            hover,
            active_fillet_preview,
            fillet_action_stamp,
            geometry_policy,
        );
        draw_control_guides(&mut p, scene, hover);
        draw_points(
            &mut p,
            scene,
            selection,
            pending,
            provisional,
            hover,
            &related,
            problem,
            geometry_policy,
        );
        if let Some(document) = accepted
            .map(SketchAcceptedDocumentState::document)
            .or_else(|| {
                scene
                    .is_detached_presentation()
                    .then(|| scene.presentation_document())
            })
            && scene.annotations_visible
        {
            draw_annotations(
                &mut p,
                scene,
                document,
                selection,
                pending,
                provisional,
                hover,
                &problem_items,
            );
        }
        draw_controls(&mut p, scene, hover);
        if let Some(chain) = offset.and_then(|o| o.chain.as_ref()) {
            draw_offset(&mut p, scene, chain, viewport);
        }
    }
    if let Some(inference) = inference {
        draw_inference_guides(&mut p, inference, viewport);
    }
    if let Some(preview) = construction_preview {
        draw_preview(&mut p, preview, viewport);
    }
    if let Some(inference) = inference {
        draw_inference_candidates(&mut p, inference);
    }
    draw_problems(&mut p, scene, problem, computed_problems, geometry_policy);
    if let Some(error) = p.invalid {
        return Err(error);
    }
    p.frame.validate()?;
    Ok(p.frame)
}

fn draw_grid(p: &mut Painter, viewport: Viewport) -> Result<(), DrawFrameError> {
    let spec = adaptive_grid_spec(viewport)
        .ok_or_else(|| DrawFrameError("invalid adaptive grid".into()))?;
    for (name, spacing, color, width) in [
        ("minor", spec.minor_pixels, "#1b2223", 0.75),
        ("major", spec.major_pixels, "#263031", 1.0),
    ] {
        let mut count = 0;
        for axis in 0..2 {
            let origin = if axis == 0 {
                spec.screen_origin.x
            } else {
                spec.screen_origin.y
            };
            let mut pos = origin.rem_euclid(spacing);
            while pos <= viewport.screen_size[axis] {
                if count >= 4096 {
                    return Err(DrawFrameError("adaptive grid line limit".into()));
                }
                let points = if axis == 0 {
                    [sp([pos, 0.0]), sp([pos, viewport.screen_size[1]])]
                } else {
                    [sp([0.0, pos]), sp([viewport.screen_size[0], pos])]
                };
                p.line(
                    &format!("grid:{name}:{axis}:{count}"),
                    "grid",
                    &format!("wb-grid-{name}"),
                    None,
                    DrawStyle::stroke(color, width),
                    &points,
                );
                count += 1;
                let next = pos + spacing;
                if !next.is_finite() || next <= pos {
                    return Err(DrawFrameError("invalid grid increment".into()));
                }
                pos = next;
            }
        }
    }
    Ok(())
}
fn draw_datums(
    p: &mut Painter,
    scene: &EditorScene,
    selection: &[SelectionItem],
    hover: EditorHoverState,
    related: &BTreeSet<SelectionItem>,
    viewport: Viewport,
) {
    for datum in &scene.datums {
        let Some(axis) = retained_datum_axis_presentation(viewport, datum.datum) else {
            continue;
        };
        if !axis.visible {
            continue;
        }
        let item = SelectionItem::Datum(datum.datum);
        let key = semantic(item);
        let x = datum.datum == SketchDatum::XAxis;
        let mut style = DrawStyle::stroke(if x { "#8c5b55" } else { "#4f8273" }, 1.25);
        let mut text = DrawStyle::fill(if x { "#ba7770" } else { "#72ad9c" });
        if selected_or_hovered(selection, hover, item) {
            style = DrawStyle::stroke("#efb856", 2.2).glow("#efb85680", 3.0);
            text.fill = Some("#f2ca82".into());
        }
        if related.contains(&item) {
            style = DrawStyle::stroke("#79bfc4", 1.25).glow("#79bfc46b", 3.0);
        }
        let node = p.line(
            &key,
            "datums",
            "wb-datum-line",
            Some(&key),
            style,
            &[axis.start, axis.end],
        );
        node.interactive = true;
        node.accessible_label = Some(if x { "X axis" } else { "Y axis" }.into());
        text.font_size = 11.0;
        text.font_weight = 700;
        text.letter_spacing = 0.44;
        text.stroke = Some("#121617".into());
        text.stroke_width = 3.0;
        p.text(
            &format!("{key}:label"),
            "datums",
            "wb-datum-label",
            Some(&key),
            text,
            axis.label,
            if x { "X" } else { "Y" },
        );
    }
}
fn selected_or_hovered(
    selection: &[SelectionItem],
    hover: EditorHoverState,
    item: SelectionItem,
) -> bool {
    selection.contains(&item) || hovered(hover, item)
}
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "independent presentation states match existing style precedence"
)]
fn curve_style(
    role: GeometryRole,
    implicit: bool,
    selected: bool,
    hover: bool,
    related: bool,
    pending: bool,
    provisional: bool,
    unavailable: bool,
    problem: bool,
) -> DrawStyle {
    let mut s = DrawStyle::stroke("#e5e8df", 2.2);
    if selected || hover {
        s.stroke = Some("#efb856".into());
    }
    if selected {
        s = s.glow("#efb85673", 3.0);
    }
    if related {
        s.stroke = Some("#79bfc4".into());
        s = s.glow("#79bfc47a", 4.0);
    }
    if pending {
        s.stroke = Some("#79bfc4".into());
        s = s.dashed(&[4.0, 3.0]).glow("#79bfc48c", 4.0);
    }
    if provisional {
        s.stroke = Some("#63d6ce".into());
        s = s.dashed(&[7.0, 4.0]);
        s.opacity = 0.84;
    }
    if role == GeometryRole::Construction {
        s.stroke = Some(
            if related || pending {
                "#79bfc4"
            } else if selected || hover {
                "#efb856"
            } else {
                "#86a0a2"
            }
            .into(),
        );
        s.dash = vec![9.0, 3.0, 2.0, 3.0];
    }
    if implicit {
        s.stroke = Some(
            if selected || hover {
                "#efb856"
            } else {
                "#70888b"
            }
            .into(),
        );
        s.dash = vec![4.0, 5.0];
        s.opacity = if selected || hover { 1.0 } else { 0.72 };
    }
    if unavailable {
        s = DrawStyle::stroke("#c18778", 2.2)
            .dashed(&[3.0, 4.0])
            .glow("#c1877861", 3.0);
        s.opacity = 0.72;
    }
    if problem {
        s.stroke = Some("#ff7666".into());
        s = s.glow("#ff5245a6", 5.0);
    }
    s
}
#[allow(clippy::too_many_arguments)]
fn draw_curves(
    p: &mut Painter,
    scene: &EditorScene,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    related: &BTreeSet<SelectionItem>,
    problem: Option<&EditorProblemMetadata>,
    computed_problems: &[ComputedFeatureProblemMetadata],
    policy: GeometryInteractionPolicy,
    offset: Option<&OffsetCanvasPresentation>,
) {
    let failed = crate::scene::failed_computed_sources(computed_problems);
    for curve in scene
        .curves
        .iter()
        .filter(|c| c.is_visible(policy) && c.screen_polyline.len() >= 2)
    {
        let item = SelectionItem::Curve(curve.span);
        let key = semantic(item);
        let unavailable = offset.is_some_and(|o| o.unavailable.contains(&item));
        let provisional = provisional.contains(&item);
        let has_problem = problem.is_some_and(|p| {
            p.targets
                .contains(&EditorProblemTarget::Curve(curve.span.curve))
        }) || failed
            .contains(&geosolve_sketch_features::NativeCurveSpanSource { span: curve.span });
        let implicit = curve.origin.is_implicit_construction();
        let style = curve_style(
            curve.role,
            implicit,
            selection.contains(&item),
            hovered(hover, item),
            related.contains(&item),
            pending.contains(&item),
            provisional,
            unavailable,
            has_problem,
        );
        let origin = if implicit {
            "implicit"
        } else if curve.role == GeometryRole::Construction {
            "explicit"
        } else {
            "profile"
        };
        let occurrence = format!("{key}:{origin}");
        let node = p.line(
            &occurrence,
            "geometry",
            "wb-curve",
            Some(&key),
            style,
            &curve.screen_polyline,
        );
        node.interactive = curve.is_interactive(policy) && !provisional && !unavailable;
        node.metadata
            .insert("persistentId".into(), curve.span.curve.to_string());
        node.metadata
            .insert("segment".into(), curve.span.segment.to_string());
        node.metadata
            .insert("role".into(), role_key(curve.role).into());
        node.metadata
            .insert("sourceRole".into(), role_key(curve.source_role).into());
        node.metadata
            .insert("constructionOrigin".into(), origin.into());
        if unavailable {
            node.accessible_label = Some(
                offset
                    .and_then(|o| o.unavailable_message.clone())
                    .unwrap_or_else(|| "Unavailable for Offset".into()),
            );
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn draw_points(
    p: &mut Painter,
    scene: &EditorScene,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    related: &BTreeSet<SelectionItem>,
    problem: Option<&EditorProblemMetadata>,
    policy: GeometryInteractionPolicy,
) {
    for point in scene.points.iter().filter(|x| x.is_visible(policy)) {
        let item = SelectionItem::Point(point.id);
        let key = semantic(item);
        let mut s = DrawStyle::stroke("#8fd2ca", 2.0);
        s.fill = Some("#131718".into());
        s.non_scaling_stroke = false;
        if selected_or_hovered(selection, hover, item) {
            s.stroke = Some("#efb856".into());
        }
        if selection.contains(&item) {
            s = s.glow("#efb85673", 3.0);
        }
        if related.contains(&item) || pending.contains(&item) {
            s.stroke = Some("#79bfc4".into());
            s = s.glow("#79bfc47a", 4.0);
        }
        if pending.contains(&item) {
            s.dash = vec![4.0, 3.0];
        }
        if provisional.contains(&item) {
            s.fill = Some("#173d3b".into());
            s.stroke = Some("#7ae8df".into());
            s.opacity = 0.9;
        }
        if problem.is_some_and(|p| p.targets.contains(&EditorProblemTarget::Point(point.id))) {
            s.fill = Some("#4a1d1b".into());
            s.stroke = Some("#ff7666".into());
            s = s.glow("#ff5245a6", 5.0);
        }
        let node = p.circle(
            &key,
            "points",
            "wb-point",
            Some(&key),
            s,
            point.screen_position,
            5.0,
        );
        node.interactive = point.is_interactive(policy)
            && !provisional.contains(&item)
            && !pending.contains(&item);
        node.metadata
            .insert("persistentId".into(), point.id.to_string());
    }
}
fn draw_control_guides(p: &mut Painter, scene: &EditorScene, hover: EditorHoverState) {
    for guide in &scene.curve_control_guides {
        let rail = guide.kind == SceneCurveControlGuideKind::SizeRail;
        let s = if rail {
            DrawStyle::stroke("#b49b67", 1.25).dashed(&[2.5, 3.0])
        } else {
            DrawStyle::stroke("#728486", 1.15).dashed(&[4.0, 4.0])
        };
        let key = format!(
            "control-guide:{}:{:?}:{:?}",
            guide.owner, guide.kind, guide.control
        );
        let node = p.line(
            &key,
            "controlGuides",
            if rail {
                "wb-curve-control-rail"
            } else {
                "wb-curve-control-guide"
            },
            None,
            s,
            &[guide.screen_start, guide.screen_end],
        );
        node.metadata.insert(
            "hovered".into(),
            guide
                .control
                .is_some_and(|c| crate::scene::curve_control_is_hovered(hover, c))
                .to_string(),
        );
    }
}
fn draw_controls(p: &mut Painter, scene: &EditorScene, hover: EditorHoverState) {
    for c in &scene.curve_controls {
        if !matches!(c.interaction, SceneCurveControlInteraction::Direct) {
            continue;
        }
        let key = format!("control:{}:{:?}", c.id.curve, c.id.kind);
        let label = match c.availability {
            geosolve_sketch::DocumentCurveControlAvailability::Editable => {
                c.accessible_name.clone()
            }
            geosolve_sketch::DocumentCurveControlAvailability::ReadOnly(reason) => format!(
                "{} · read-only: {}",
                c.accessible_name,
                crate::scene::curve_control_read_only_reason(reason)
            ),
        };
        let is_hover = crate::scene::curve_control_is_hovered(hover, c.id);
        let color = match c.id.kind {
            DocumentCurveControlKind::TrimStart | DocumentCurveControlKind::TrimEnd => "#e6c785",
            DocumentCurveControlKind::RationalMiddle => "#b7a0df",
            _ => "#8fd2ca",
        };
        let mut s = DrawStyle::stroke(color, 1.8);
        s.fill = Some("#151a1b".into());
        if is_hover {
            s.stroke = Some("#ffd27d".into());
            s.stroke_width = 2.4;
            s = s.glow("#efb856a6", 4.0);
        }
        if !c.is_editable() {
            s.stroke = Some("#77817f".into());
            s.dash = vec![2.0, 2.0];
        }
        let geometry = match c.grip {
            SceneCurveControlGripGeometry::Circle {
                center,
                radius_pixels,
            } => DrawGeometry::Circle {
                center: xy(center),
                radius: radius_pixels,
            },
            SceneCurveControlGripGeometry::Square {
                center,
                half_extent_pixels: r,
            } => DrawGeometry::Rect {
                x: center.x - r,
                y: center.y - r,
                width: r * 2.0,
                height: r * 2.0,
                radius: 0.0,
            },
            SceneCurveControlGripGeometry::Diamond {
                center,
                radius_pixels: r,
            } => DrawGeometry::Polyline {
                points: vec![
                    [center.x, center.y - r],
                    [center.x + r, center.y],
                    [center.x, center.y + r],
                    [center.x - r, center.y],
                ],
                closed: true,
            },
        };
        let node = p.push(
            &key,
            "controls",
            "wb-curve-control-mark",
            Some(&key),
            s,
            geometry,
        );
        node.interactive = c.is_editable();
        node.accessible_label = Some(label.clone());
        node.metadata.insert(
            "controlRole".into(),
            crate::curve_control_kind_key(c.id.kind).into(),
        );
        node.metadata
            .insert("readOnly".into(), (!c.is_editable()).to_string());
        if is_hover {
            let mut text = DrawStyle::fill("#e6ece9");
            text.stroke = Some("#151a1b".into());
            text.stroke_width = 4.0;
            text.font_family = "ui-sans-serif,system-ui,sans-serif";
            text.font_size = 10.0;
            text.font_weight = 600;
            text.letter_spacing = 0.1;
            p.text(
                &format!("{key}:label"),
                "controls",
                "wb-curve-control-tooltip",
                Some(&key),
                text,
                sp([c.screen_position.x + 10.0, c.screen_position.y - 10.0]),
                &label,
            );
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "numeric geometry and explicit paint identity remain separate"
)]
fn arrow(
    p: &mut Painter,
    base: &str,
    layer: &'static str,
    key: Option<&str>,
    start: ScreenPoint,
    end: ScreenPoint,
    size: f64,
    color: &str,
    opacity: f64,
) {
    let d = [end.x - start.x, end.y - start.y];
    let len = d[0].hypot(d[1]);
    if len <= f64::EPSILON {
        return;
    }
    let u = [d[0] / len, d[1] / len];
    let unit = size / if layer == "offset" { 7.0 } else { 6.0 };
    let back = size - unit;
    let mut s = DrawStyle::fill(color);
    s.opacity = opacity;
    p.push(
        base,
        layer,
        "direction-arrow",
        key,
        s,
        DrawGeometry::Polyline {
            points: vec![
                [end.x + unit * u[0], end.y + unit * u[1]],
                [
                    end.x - back * u[0] + size * 0.5 * u[1],
                    end.y - back * u[1] - size * 0.5 * u[0],
                ],
                [
                    end.x - back * u[0] - size * 0.5 * u[1],
                    end.y - back * u[1] + size * 0.5 * u[0],
                ],
            ],
            closed: true,
        },
    );
}
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive presentation dispatch preserves one auditable paint order"
)]
fn draw_computed(
    p: &mut Painter,
    scene: &EditorScene,
    selection: &[SelectionItem],
    hover: EditorHoverState,
    active: Option<&SceneFilletActionTarget>,
    stamp: Option<u64>,
    policy: GeometryInteractionPolicy,
) {
    let affected = scene
        .fillet_affordances
        .iter()
        .filter(|a| crate::scene::fillet_owner_is_visible(a.owner, selection))
        .flat_map(|a| a.affected_owners.iter().copied())
        .collect::<BTreeSet<_>>();
    for curve in scene
        .computed_curves
        .iter()
        .filter(|c| c.is_visible(policy))
    {
        let item = SelectionItem::FeatureCorner(curve.owner);
        let key = semantic(item);
        let selected = selection.contains(&item)
            || selection.contains(&SelectionItem::Feature(curve.owner.feature));
        let mut s = DrawStyle::stroke("#8ed5ca", 2.6);
        if curve.role == GeometryRole::Construction {
            s.stroke = Some("#86a0a2".into());
            s.dash = vec![9.0, 3.0, 2.0, 3.0];
        }
        if selected || hovered(hover, item) {
            s.stroke = Some("#efb856".into());
            s = s.glow("#efb85673", 3.0);
        }
        if affected.contains(&curve.owner) {
            s.stroke = Some("#f0c66d".into());
            s = s.glow("#f0c66d7a", 3.0);
        }
        let node = p.line(
            &format!("{key}:edge:{}", curve.edge.ordinal),
            "computed",
            "wb-computed-fillet",
            Some(&key),
            s,
            &curve.screen_polyline,
        );
        node.interactive = curve.is_interactive(policy);
        node.metadata
            .insert("featureId".into(), curve.owner.feature.to_string());
        node.metadata
            .insert("cornerId".into(), curve.owner.corner.to_string());
        node.metadata
            .insert("role".into(), role_key(curve.role).into());
        node.metadata
            .insert("evaluation".into(), curve.edge.evaluation.raw().to_string());
    }
    for a in &scene.fillet_affordances {
        if !crate::scene::fillet_owner_is_visible(a.owner, selection)
            || !scene
                .computed_curves
                .iter()
                .any(|c| c.owner == a.owner && c.is_interactive(policy))
        {
            continue;
        }
        let key = semantic(SelectionItem::FeatureCorner(a.owner));
        for action in &a.actions {
            let action_key = crate::fillet_action_key(action.id);
            let base = format!("{key}:action:{action_key}");
            let previewed = active.is_some_and(|active| {
                scene.fillet_action_target(a.owner, action.id).as_ref() == Some(active)
            });
            let disabled = !matches!(
                action.availability,
                SceneFilletActionAvailability::Applicable
            );
            let opacity = if disabled { 0.35 } else { 1.0 };
            let first = p.frame.items.len();
            if previewed && let Some(ghost) = &action.dashed_alternative_arc {
                let mut s = DrawStyle::stroke("#d9b86e", 2.0).dashed(&[6.0, 4.0]);
                s.opacity = 0.92 * opacity;
                p.line(
                    &format!("{base}:ghost"),
                    "filletAffordances",
                    "wb-fillet-alternative-ghost",
                    Some(&key),
                    s,
                    &ghost.screen_polyline,
                );
            }
            let color = if previewed {
                "#fff0bd"
            } else if action.control_geometry.is_some() {
                "#e7ce8d"
            } else {
                "#f1d899"
            };
            if let Some(c) = action.control_geometry {
                let mut s = DrawStyle::stroke(color, 2.0);
                s.opacity = opacity;
                p.line(
                    &base,
                    "filletAffordances",
                    "wb-fillet-retained-direction",
                    Some(&key),
                    s,
                    &[c.screen_start, c.screen_end],
                );
                arrow(
                    p,
                    &base,
                    "filletAffordances",
                    Some(&key),
                    c.screen_start,
                    c.screen_end,
                    12.0,
                    color,
                    opacity,
                );
            } else {
                let anchor = crate::scene::fillet_action_anchor(a, action);
                let mut s = DrawStyle::stroke(color, 1.4);
                s.opacity = opacity;
                let shapes = match action.id {
                    SceneFilletActionId::ReverseFirstRetainedDirection
                    | SceneFilletActionId::ReverseSecondRetainedDirection => vec![
                        local_line(&[[-4.0, 0.0], [4.0, 0.0]]),
                        local_line(&[[-2.0, -2.0], [-4.0, 0.0], [-2.0, 2.0]]),
                        local_line(&[[2.0, -2.0], [4.0, 0.0], [2.0, 2.0]]),
                    ],
                    SceneFilletActionId::ComplementaryArc => vec![
                        DrawGeometry::Polyline {
                            points: endpoint_arc(
                                sp([-4.0, 2.0]),
                                sp([4.0, -2.0]),
                                5.0,
                                false,
                                true,
                            )
                            .into_iter()
                            .map(xy)
                            .collect(),
                            closed: false,
                        },
                        local_line(&[[4.0, 2.0], [4.0, -2.0], [0.0, -2.0]]),
                    ],
                    SceneFilletActionId::LocalAlternative { .. } => vec![
                        quadratic([-4.0, 3.0], [0.0, -5.0], [4.0, 3.0]),
                        local_line(&[[-3.0, -2.0], [3.0, -2.0]]),
                    ],
                };
                for shape in shapes {
                    p.push(
                        &base,
                        "filletAffordances",
                        "wb-fillet-action-control",
                        Some(&key),
                        s.clone(),
                        transform(shape, anchor, 0.0),
                    );
                }
            }
            for node in &mut p.frame.items[first..] {
                node.interactive = !disabled;
                node.accessible_label = Some(action.label.clone());
                node.metadata
                    .insert("filletAction".into(), action_key.clone());
                node.metadata
                    .insert("disabled".into(), disabled.to_string());
                if let Some(stamp) = stamp {
                    node.metadata
                        .insert("filletActionStamp".into(), stamp.to_string());
                }
                if let SceneFilletActionAvailability::Disabled { reason } = &action.availability {
                    node.metadata
                        .insert("disabledReason".into(), reason.clone());
                }
            }
        }
        let rail = a.radius_rail;
        p.line(
            &format!("{key}:rail"),
            "filletAffordances",
            "wb-fillet-radius-rail",
            Some(&key),
            DrawStyle::stroke("#e6c785", 1.4).dashed(&[3.0, 3.0]),
            &[rail.screen_rail_start, rail.screen_rail_end],
        );
        p.line(
            &format!("{key}:spoke"),
            "filletAffordances",
            "wb-fillet-radius-spoke",
            Some(&key),
            DrawStyle::stroke("#8ed5ca", 1.25),
            &[rail.screen_center, rail.screen_grip],
        );
        let mut s = DrawStyle::stroke("#f0c66d", 2.0);
        s.fill = Some("#171c1d".into());
        let node = p.circle(
            &format!("{key}:grip"),
            "filletAffordances",
            "wb-fillet-radius-grip",
            Some(&key),
            s,
            rail.screen_grip,
            6.0,
        );
        node.interactive = true;
        node.accessible_label = Some("Drag shared Fillet radius".into());
    }
}
fn local_line(points: &[[f64; 2]]) -> DrawGeometry {
    DrawGeometry::Polyline {
        points: points.to_vec(),
        closed: false,
    }
}
fn local_circle(x: f64, y: f64, r: f64) -> DrawGeometry {
    DrawGeometry::Circle {
        center: [x, y],
        radius: r,
    }
}
#[allow(
    clippy::many_single_char_names,
    reason = "conventional Bezier control and parameter notation"
)]
fn quadratic(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> DrawGeometry {
    DrawGeometry::Polyline {
        points: (0..=24)
            .map(|i| {
                let t = f64::from(i) / 24.0;
                let u = 1.0 - t;
                [
                    u * u * a[0] + 2.0 * u * t * b[0] + t * t * c[0],
                    u * u * a[1] + 2.0 * u * t * b[1] + t * t * c[1],
                ]
            })
            .collect(),
        closed: false,
    }
}
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "finite tessellation count is clamped to 1 through 16384 before conversion"
)]
fn arc_points(center: ScreenPoint, radius: f64, start: f64, sweep: f64) -> Vec<ScreenPoint> {
    // Presentation tessellation only: the caller supplies accepted/draft arc state.
    // Bound chord deviation to 0.05 logical pixels without changing semantic geometry.
    let step = if radius > 0.05 {
        2.0 * (1.0 - 0.05 / radius).clamp(-1.0, 1.0).acos()
    } else {
        std::f64::consts::FRAC_PI_4
    };
    let count = (sweep.abs() / step.max(0.00001)).ceil().clamp(1.0, 16384.0) as u32;
    (0..=count)
        .map(|i| {
            let a = start + sweep * f64::from(i) / f64::from(count);
            sp([center.x + radius * a.cos(), center.y + radius * a.sin()])
        })
        .collect()
}
fn transform(g: DrawGeometry, anchor: ScreenPoint, angle: f64) -> DrawGeometry {
    let (s, c) = angle.sin_cos();
    let map = |v: [f64; 2]| {
        [
            anchor.x + c * v[0] - s * v[1],
            anchor.y + s * v[0] + c * v[1],
        ]
    };
    match g {
        DrawGeometry::Polyline { points, closed } => DrawGeometry::Polyline {
            points: points.into_iter().map(map).collect(),
            closed,
        },
        DrawGeometry::Circle { center, radius } => DrawGeometry::Circle {
            center: map(center),
            radius,
        },
        DrawGeometry::Ellipse {
            center,
            radii,
            rotation,
        } => DrawGeometry::Ellipse {
            center: map(center),
            radii,
            rotation: rotation + angle,
        },
        DrawGeometry::Text {
            position,
            text,
            rotation,
        } => DrawGeometry::Text {
            position: map(position),
            text,
            rotation: rotation + angle,
        },
        DrawGeometry::Rect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            let r = radius.min(width * 0.5).min(height * 0.5);
            let mut points = Vec::new();
            for (center, begin) in [
                ([x + width - r, y + r], -std::f64::consts::FRAC_PI_2),
                ([x + width - r, y + height - r], 0.0),
                ([x + r, y + height - r], std::f64::consts::FRAC_PI_2),
                ([x + r, y + r], std::f64::consts::PI),
            ] {
                points.extend(
                    arc_points(sp(center), r, begin, std::f64::consts::FRAC_PI_2)
                        .into_iter()
                        .map(xy)
                        .map(map),
                );
            }
            DrawGeometry::Polyline {
                points,
                closed: true,
            }
        }
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive presentation dispatch preserves one auditable paint order"
)]
fn glyph_shapes(g: SceneConstraintGlyph) -> Vec<DrawGeometry> {
    use SceneConstraintGlyph as G;
    let l = local_line;
    let c = local_circle;
    let q = quadratic;
    match g {
        G::Fixed => vec![
            DrawGeometry::Rect {
                x: -5.5,
                y: -1.0,
                width: 11.0,
                height: 8.0,
                radius: 1.5,
            },
            l(&[[-3.5, -1.0], [-3.5, -4.0]]),
            DrawGeometry::Polyline {
                points: arc_points(
                    sp([0.0, -4.0]),
                    3.5,
                    std::f64::consts::PI,
                    std::f64::consts::PI,
                )
                .into_iter()
                .map(xy)
                .collect(),
                closed: false,
            },
            l(&[[3.5, -4.0], [3.5, -1.0]]),
            l(&[[0.0, 2.0], [0.0, 4.5]]),
        ],
        G::Coincident => vec![
            c(0.0, 0.0, 5.5),
            c(0.0, 0.0, 1.8),
            l(&[[-8.0, 0.0], [-6.0, 0.0]]),
            l(&[[6.0, 0.0], [8.0, 0.0]]),
            l(&[[0.0, -8.0], [0.0, -6.0]]),
            l(&[[0.0, 6.0], [0.0, 8.0]]),
        ],
        G::Horizontal => vec![
            l(&[[-8.0, 0.0], [8.0, 0.0]]),
            l(&[[-6.0, -3.0], [-6.0, 3.0]]),
            l(&[[6.0, -3.0], [6.0, 3.0]]),
        ],
        G::Vertical => vec![
            l(&[[0.0, -8.0], [0.0, 8.0]]),
            l(&[[-3.0, -6.0], [3.0, -6.0]]),
            l(&[[-3.0, 6.0], [3.0, 6.0]]),
        ],
        G::PointOnCurve => vec![q([-8.0, 4.0], [0.0, -6.0], [8.0, 4.0]), c(0.0, -1.0, 2.0)],
        G::Parallel => vec![
            l(&[[-8.0, 3.0], [2.0, -7.0]]),
            l(&[[-2.0, 7.0], [8.0, -3.0]]),
            l(&[[-5.0, -1.0], [-2.0, -1.0], [-2.0, -4.0]]),
            l(&[[1.0, 3.0], [4.0, 3.0], [4.0, 0.0]]),
        ],
        G::Perpendicular => vec![
            l(&[[-7.0, -7.0], [-7.0, 6.0], [7.0, 6.0]]),
            l(&[[-7.0, 2.0], [-3.0, 2.0], [-3.0, 6.0]]),
        ],
        G::Concentric => vec![c(0.0, 0.0, 7.0), c(0.0, 0.0, 3.5), c(0.0, 0.0, 1.0)],
        G::Collinear => vec![
            l(&[[-8.0, 5.0], [8.0, -5.0]]),
            c(-4.0, 2.5, 1.35),
            c(0.0, 0.0, 1.35),
            c(4.0, -2.5, 1.35),
        ],
        G::EqualLength => vec![
            l(&[[-8.0, -4.0], [8.0, -4.0]]),
            l(&[[-8.0, 4.0], [8.0, 4.0]]),
            l(&[[-1.0, -7.0], [1.0, -1.0]]),
            l(&[[-1.0, 1.0], [1.0, 7.0]]),
        ],
        G::EqualRadius => vec![
            c(-4.5, 0.0, 4.0),
            c(4.5, 0.0, 4.0),
            l(&[[-4.5, 0.0], [-1.7, -2.8]]),
            l(&[[4.5, 0.0], [7.3, -2.8]]),
        ],
        G::Midpoint => vec![
            l(&[[-8.0, 4.0], [8.0, 4.0]]),
            l(&[[-4.0, 1.0], [-4.0, 7.0]]),
            l(&[[4.0, 1.0], [4.0, 7.0]]),
            DrawGeometry::Polyline {
                points: vec![[0.0, -4.0], [4.0, 4.0], [-4.0, 4.0]],
                closed: true,
            },
        ],
        G::Symmetry => vec![
            l(&[[0.0, -9.0], [0.0, -5.0]]),
            l(&[[0.0, -2.0], [0.0, 2.0]]),
            l(&[[0.0, 5.0], [0.0, 9.0]]),
            l(&[[-3.0, -6.0], [-7.0, 0.0], [-3.0, 6.0]]),
            l(&[[3.0, -6.0], [7.0, 0.0], [3.0, 6.0]]),
        ],
        G::Contact => vec![
            q([-8.0, -5.0], [-2.0, -5.0], [0.0, 0.0]),
            q([0.0, 0.0], [2.0, 5.0], [8.0, 5.0]),
            c(0.0, 0.0, 1.6),
        ],
        G::Tangency => vec![c(0.0, -2.0, 5.0), l(&[[-8.0, 3.0], [8.0, 3.0]])],
        G::Direction => vec![
            q([-8.0, 5.0], [0.0, -1.0], [8.0, 5.0]),
            l(&[[-6.0, -4.0], [5.0, -4.0]]),
            l(&[[2.0, -7.0], [6.0, -4.0], [2.0, -1.0]]),
        ],
        G::Normal => vec![
            q([-8.0, 5.0], [0.0, -1.0], [8.0, 5.0]),
            l(&[[0.0, -1.0], [0.0, -8.0]]),
            l(&[[0.0, -5.0], [3.0, -5.0], [3.0, -2.0]]),
        ],
        G::EqualCurvature => vec![
            q([-9.0, 5.0], [-7.0, -5.0], [-1.0, -5.0]),
            q([1.0, 5.0], [3.0, -5.0], [9.0, -5.0]),
            l(&[[-2.0, -1.0], [2.0, -1.0]]),
            l(&[[-2.0, 2.0], [2.0, 2.0]]),
        ],
        G::Continuity => vec![
            q([-9.0, 5.0], [-4.0, 0.0], [0.0, 0.0]),
            q([0.0, 0.0], [4.0, 0.0], [9.0, -5.0]),
            l(&[[-5.0, 0.0], [5.0, 0.0]]),
            c(0.0, 0.0, 1.5),
        ],
        G::Fillet => vec![
            l(&[[-8.0, 7.0], [-4.0, 7.0]]),
            DrawGeometry::Polyline {
                points: arc_points(
                    sp([7.0, 7.0]),
                    11.0,
                    std::f64::consts::PI,
                    std::f64::consts::FRAC_PI_2,
                )
                .into_iter()
                .map(xy)
                .collect(),
                closed: false,
            },
            l(&[[7.0, -4.0], [7.0, -8.0]]),
        ],
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "numeric geometry and explicit paint identity remain separate"
)]
fn draw_glyph(
    p: &mut Painter,
    base: &str,
    layer: &'static str,
    key: Option<&str>,
    glyph: SceneConstraintGlyph,
    anchor: ScreenPoint,
    rotation: f64,
    style: &DrawStyle,
) {
    for g in glyph_shapes(glyph) {
        p.push(
            base,
            layer,
            "wb-constraint-symbol",
            key,
            style.clone(),
            transform(g, anchor, rotation),
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive presentation dispatch preserves one auditable paint order"
)]
fn draw_annotations(
    p: &mut Painter,
    scene: &EditorScene,
    document: &geosolve_sketch::SketchDocument,
    selection: &[SelectionItem],
    pending: &[SelectionItem],
    provisional: &[SelectionItem],
    hover: EditorHoverState,
    problem_items: &[SelectionItem],
) {
    let context = hover
        .context_owner
        .or_else(|| hover.target.map(EditorHoverTarget::item));
    for a in &scene.annotations {
        if !(a.is_visible(selection, context, problem_items)
            || scene.show_all_constraint_annotations
                && matches!(a.kind, SceneAnnotationKind::Constraint(_)))
        {
            continue;
        }
        if let SelectionItem::Dimension(id) = a.item
            && document.dimension(id).is_none()
        {
            continue;
        }
        let key = semantic(a.item);
        let base = format!("{key}:source:{}", a.source);
        let occurrence = match hover.target {
            Some(EditorHoverTarget::Annotation(o)) if o.item == a.item => Some(o),
            _ => None,
        };
        let selected =
            selection.contains(&a.item) || occurrence.is_some_and(|o| o.marker_index.is_none());
        let problem = problem_items.contains(&a.item);
        let dimension = !matches!(a.kind, SceneAnnotationKind::Constraint(_));
        let color = if problem {
            "#ff8b7d"
        } else if selected {
            "#fff2c8"
        } else if dimension {
            "#79bfc4"
        } else {
            "#d7a654"
        };
        let mut style = DrawStyle::stroke(color, if dimension { 1.25 } else { 1.8 });
        if selected {
            style = style.glow("#efb85680", 3.0);
        }
        if problem {
            style = style.glow("#ff524599", 4.0);
        }
        if a.reference {
            style.dash = vec![4.0, 3.0];
            style.opacity = 0.72;
        }
        if a.suppressed {
            style.opacity = 0.48;
        }
        if pending.contains(&a.item) {
            style.opacity *= 0.78;
        }
        if provisional.contains(&a.item) {
            style.opacity *= 0.82;
        }
        let first = p.frame.items.len();
        match &a.geometry {
            SceneAnnotationGeometry::Glyph { markers } => {
                if let SceneAnnotationKind::Constraint(glyph) = a.kind {
                    for (i, m) in markers.iter().enumerate() {
                        let marker_base = format!("{base}:marker:{i}");
                        if let Some(origin) = m.leader_from {
                            let mut s = DrawStyle::stroke("#8d774f", 1.0).dashed(&[2.0, 2.0]);
                            s.opacity = style.opacity;
                            p.line(
                                &format!("{marker_base}:leader"),
                                "annotations",
                                "wb-annotation-leader",
                                Some(&key),
                                s,
                                &[origin, m.anchor],
                            );
                        }
                        let mut s = style.clone();
                        if occurrence.is_some_and(|o| o.marker_index == Some(i)) {
                            s.stroke = Some("#fff2c8".into());
                            s = s.glow("#efb85680", 3.0);
                        }
                        let first_marker = p.frame.items.len();
                        draw_glyph(
                            p,
                            &marker_base,
                            "annotations",
                            Some(&key),
                            glyph,
                            m.anchor,
                            m.rotation_radians,
                            &s,
                        );
                        for node in &mut p.frame.items[first_marker..] {
                            node.metadata.insert("markerIndex".into(), i.to_string());
                        }
                    }
                }
            }
            SceneAnnotationGeometry::RightAngle {
                first_arm,
                corner,
                second_arm,
                ..
            } => {
                p.line(
                    &base,
                    "annotations",
                    "wb-right-angle",
                    Some(&key),
                    style.clone(),
                    &[*first_arm, *corner, *second_arm],
                );
            }
            SceneAnnotationGeometry::LinearDimension {
                measured_first,
                measured_second,
                first,
                second,
                ..
            } => {
                let mut witness = style.clone();
                witness.dash.clear();
                witness.opacity = if a.suppressed {
                    0.48
                } else if a.reference {
                    0.45
                } else {
                    0.62
                };
                p.line(
                    &format!("{base}:witness"),
                    "annotations",
                    "wb-dimension-witness",
                    Some(&key),
                    witness.clone(),
                    &[*measured_first, *first],
                );
                p.line(
                    &format!("{base}:witness"),
                    "annotations",
                    "wb-dimension-witness",
                    Some(&key),
                    witness,
                    &[*measured_second, *second],
                );
                p.line(
                    &base,
                    "annotations",
                    "wb-dimension-line",
                    Some(&key),
                    style.clone(),
                    &[*first, *second],
                );
            }
            SceneAnnotationGeometry::RadialDimension {
                center,
                edge,
                label_anchor,
                diameter,
                full_circle,
            } => {
                let start = if *diameter && *full_circle {
                    sp([
                        center.x.mul_add(2.0, -edge.x),
                        center.y.mul_add(2.0, -edge.y),
                    ])
                } else {
                    *center
                };
                p.line(
                    &base,
                    "annotations",
                    "wb-dimension-line",
                    Some(&key),
                    style.clone(),
                    &[start, *edge, *label_anchor],
                );
            }
            SceneAnnotationGeometry::AngularDimension {
                vertex,
                first_ray,
                second_ray,
                radius,
                clockwise,
                ..
            } => {
                let mut witness = style.clone();
                witness.dash.clear();
                witness.opacity = if a.suppressed {
                    0.48
                } else if a.reference {
                    0.45
                } else {
                    0.62
                };
                p.line(
                    &format!("{base}:witness"),
                    "annotations",
                    "wb-dimension-witness",
                    Some(&key),
                    witness.clone(),
                    &[*vertex, *first_ray],
                );
                p.line(
                    &format!("{base}:witness"),
                    "annotations",
                    "wb-dimension-witness",
                    Some(&key),
                    witness,
                    &[*vertex, *second_ray],
                );
                let first = crate::scene::ray_point(*vertex, *first_ray, *radius);
                let second = crate::scene::ray_point(*vertex, *second_ray, *radius);
                let points = p.arc(first, second, *radius, false, *clockwise);
                p.line(
                    &base,
                    "annotations",
                    "wb-angle-arc",
                    Some(&key),
                    style.clone(),
                    &points,
                );
            }
            SceneAnnotationGeometry::Label {
                anchor,
                leader_from,
            } => {
                if let Some(origin) = leader_from {
                    p.line(
                        &base,
                        "annotations",
                        "wb-dimension-line",
                        Some(&key),
                        style.clone(),
                        &[*origin, *anchor],
                    );
                }
            }
        }
        for head in a.geometry.arrowheads() {
            let mut s = DrawStyle::fill(color);
            s.opacity = style.opacity;
            p.push(
                &format!("{base}:arrow"),
                "annotations",
                "wb-dimension-arrow",
                Some(&key),
                s,
                DrawGeometry::Polyline {
                    points: vec![xy(head.tip), xy(head.base_first), xy(head.base_second)],
                    closed: true,
                },
            );
        }
        if dimension && let Some(anchor) = crate::scene::annotation_anchor(&a.geometry) {
            if let Some(bounds) = a.label_bounds {
                p.push(
                    &format!("{base}:mask"),
                    "annotations",
                    "wb-dimension-label-mask",
                    Some(&key),
                    DrawStyle::fill("#121617"),
                    DrawGeometry::Rect {
                        x: bounds.min.x,
                        y: bounds.min.y,
                        width: bounds.max.x - bounds.min.x,
                        height: bounds.max.y - bounds.min.y,
                        radius: 3.0,
                    },
                );
            }
            let mut s = DrawStyle::fill(color);
            s.opacity = style.opacity;
            s.shadow.clone_from(&style.shadow);
            s.text_anchor = "middle";
            p.text(
                &format!("{base}:text"),
                "annotations",
                "wb-dimension-text",
                Some(&key),
                s,
                sp([anchor.x, anchor.y + 4.0]),
                a.visible_text.as_deref().unwrap_or(""),
            );
        }
        for node in &mut p.frame.items[first..] {
            node.interactive = !pending.contains(&a.item) && !provisional.contains(&a.item);
            node.accessible_label = Some(a.accessible_label.clone());
            node.metadata.insert("source".into(), a.source.to_string());
            node.metadata
                .insert("reference".into(), a.reference.to_string());
            node.metadata
                .insert("suppressed".into(), a.suppressed.to_string());
        }
    }
}
// Numeric circular endpoint-arc presentation matches SVG's radius/large/sweep
// geometry; used only for dimension decoration and fixed glyph outlines.
#[allow(
    clippy::many_single_char_names,
    reason = "conventional circular endpoint construction notation"
)]
fn endpoint_arc(
    start: ScreenPoint,
    end: ScreenPoint,
    radius: f64,
    large: bool,
    clockwise: bool,
) -> Vec<ScreenPoint> {
    let dx = (start.x - end.x) * 0.5;
    let dy = (start.y - end.y) * 0.5;
    let d = dx.hypot(dy);
    if d <= f64::EPSILON || radius <= 0.0 {
        return vec![start, end];
    }
    let r = radius.max(d);
    let k =
        ((r * r - d * d).max(0.0) / (d * d)).sqrt() * if large == clockwise { -1.0 } else { 1.0 };
    let center = sp([
        (start.x + end.x) * 0.5 + k * dy,
        (start.y + end.y) * 0.5 - k * dx,
    ]);
    let a = (start.y - center.y).atan2(start.x - center.x);
    let b = (end.y - center.y).atan2(end.x - center.x);
    let tau = std::f64::consts::TAU;
    let sweep = if clockwise {
        (b - a).rem_euclid(tau)
    } else {
        -(a - b).rem_euclid(tau)
    };
    let mut points = arc_points(center, r, a, sweep);
    points[0] = start;
    if let Some(last) = points.last_mut() {
        *last = end;
    }
    points
}
fn draw_offset(
    p: &mut Painter,
    scene: &EditorScene,
    chain: &OffsetAuthoringChainPresentation,
    viewport: Viewport,
) {
    if chain.spans.is_empty() {
        return;
    }
    for (i, directed) in chain.spans.iter().enumerate() {
        let Some(c) = scene.curves.iter().find(|c| c.span == directed.span) else {
            continue;
        };
        let mid = c.screen_polyline.len() / 2;
        if mid == 0 {
            continue;
        }
        let a = c.screen_polyline[mid - 1];
        let b = c.screen_polyline[mid];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy);
        if len <= f64::EPSILON {
            continue;
        }
        let center = sp([(a.x + b.x) * 0.5, (a.y + b.y) * 0.5]);
        let mut start = sp([center.x - 8.0 * dx / len, center.y - 8.0 * dy / len]);
        let mut end = sp([center.x + 8.0 * dx / len, center.y + 8.0 * dy / len]);
        if directed.traversal == OffsetTraversal::Reverse {
            std::mem::swap(&mut start, &mut end);
        }
        let key = semantic(SelectionItem::Curve(c.span));
        let base = format!("offset-chain:{key}:{i}");
        p.line(
            &base,
            "offset",
            "wb-offset-chain-direction",
            Some(&key),
            DrawStyle::stroke("#f0c66d", 2.1).glow("#f0c66d7a", 2.0),
            &[start, end],
        );
        arrow(
            p,
            &base,
            "offset",
            Some(&key),
            start,
            end,
            14.7,
            "#f0c66d",
            1.0,
        );
    }
    for (kind, terminal, label, color, text_color) in [
        ("start", chain.start, "S", "#f0c66d", "#f4e3b4"),
        ("end", chain.end, "E", "#79bfc4", "#bde7e4"),
    ] {
        let point = viewport.model_to_screen(terminal.model_position);
        let key = semantic(SelectionItem::Curve(terminal.endpoint.span));
        let base = format!("offset-{kind}:{key}");
        let mut s = DrawStyle::stroke(color, 1.6);
        s.fill = Some("#162021".into());
        p.circle(
            &base,
            "offset",
            "wb-offset-chain-terminal",
            Some(&key),
            s,
            point,
            7.0,
        );
        let mut s = DrawStyle::fill(text_color);
        s.font_size = 8.0;
        s.font_weight = 700;
        s.font_family = "ui-sans-serif,system-ui,sans-serif";
        s.text_anchor = "middle";
        s.text_baseline = "central";
        p.text(
            &format!("{base}:text"),
            "offset",
            "wb-offset-chain-terminal-text",
            Some(&key),
            s,
            point,
            label,
        );
    }
}

fn draft_style() -> DrawStyle {
    DrawStyle::stroke("#efb856", 2.0).dashed(&[8.0, 5.0])
}
fn draft_mark(p: &mut Painter, viewport: Viewport, point: [f64; 2], name: &str) {
    let mut s = DrawStyle::stroke("#efb856", 2.0);
    s.opacity = 0.55;
    p.circle(
        name,
        "draft",
        name,
        None,
        s,
        viewport.model_to_screen(point),
        7.0,
    );
}
fn draft_line(p: &mut Painter, viewport: Viewport, points: &[[f64; 2]], name: &str, marks: bool) {
    let screen = points
        .iter()
        .map(|x| viewport.model_to_screen(*x))
        .collect::<Vec<_>>();
    if screen.len() >= 2 {
        p.line(name, "draft", name, None, draft_style(), &screen);
    }
    if marks {
        for x in screen {
            p.circle(
                &format!("{name}:point"),
                "draft",
                "wb-draft-point",
                None,
                DrawStyle::stroke("#efb856", 2.0),
                x,
                4.0,
            );
        }
    }
}
fn draft_controls(
    p: &mut Painter,
    viewport: Viewport,
    kind: AdvancedConstructionKind,
    points: &[[f64; 2]],
) {
    if kind == AdvancedConstructionKind::EllipticalArc && points.len() == 4 {
        draft_line(p, viewport, &points[..2], "wb-draft-major-axis", false);
        for (point, name) in points.iter().zip([
            "wb-draft-center",
            "wb-draft-major-axis-point",
            "wb-draft-start",
            "wb-draft-end",
        ]) {
            draft_mark(p, viewport, *point, name);
        }
    } else {
        draft_line(p, viewport, points, "wb-draft-control-polygon", true);
    }
}
fn draw_preview(p: &mut Painter, preview: &ConstructionPreview, viewport: Viewport) {
    match preview {
        ConstructionPreview::Complete { geometry, .. } => {
            draw_preview_geometry(p, geometry, viewport);
        }
        ConstructionPreview::Anchor { position } => {
            draft_mark(p, viewport, *position, "wb-draft-center");
        }
        ConstructionPreview::ArcRadiusGuide { center, start } => {
            draft_line(p, viewport, &[*center, *start], "wb-draft-radius", true);
            draft_mark(p, viewport, *center, "wb-draft-center");
            draft_mark(p, viewport, *start, "wb-draft-start");
        }
        ConstructionPreview::GuidePolyline { points, closed } => {
            let mut display = points.clone();
            if *closed && points.len() >= 3 {
                display.push(points[0]);
            }
            draft_line(p, viewport, &display, "wb-draft-guide", false);
            for point in points {
                draft_mark(p, viewport, *point, "wb-draft-point");
            }
        }
        ConstructionPreview::EllipticalArcSupport {
            center,
            major_axis_point,
            support_points,
            trim_start,
        } => {
            draft_line(
                p,
                viewport,
                support_points,
                "wb-draft-ellipse-support",
                false,
            );
            draft_line(
                p,
                viewport,
                &[*center, *major_axis_point],
                "wb-draft-major-axis",
                false,
            );
            draft_mark(p, viewport, *center, "wb-draft-center");
            draft_mark(p, viewport, *major_axis_point, "wb-draft-major-axis-point");
            if let Some(start) = trim_start {
                draft_mark(p, viewport, *start, "wb-draft-start");
            }
        }
        ConstructionPreview::ControlPolygon { kind, points } => {
            draft_controls(p, viewport, *kind, points);
        }
    }
}
fn draw_preview_geometry(
    p: &mut Painter,
    geometry: &ConstructionPreviewGeometry,
    viewport: Viewport,
) {
    match geometry {
        ConstructionPreviewGeometry::Point { position } => {
            draft_mark(p, viewport, *position, "wb-draft-point");
        }
        ConstructionPreviewGeometry::Polyline { points } => {
            draft_line(p, viewport, points, "wb-draft-polyline", true);
        }
        ConstructionPreviewGeometry::Rectangle { first, second } => {
            let a = viewport.model_to_screen(*first);
            let b = viewport.model_to_screen(*second);
            p.push(
                "wb-draft-rectangle",
                "draft",
                "wb-draft-rectangle",
                None,
                draft_style(),
                DrawGeometry::Rect {
                    x: a.x.min(b.x),
                    y: a.y.min(b.y),
                    width: (b.x - a.x).abs(),
                    height: (b.y - a.y).abs(),
                    radius: 0.0,
                },
            );
        }
        ConstructionPreviewGeometry::Circle { center, radius } => {
            p.circle(
                "wb-draft-circle",
                "draft",
                "wb-draft-circle",
                None,
                draft_style(),
                viewport.model_to_screen(*center),
                radius * viewport.pixels_per_model_unit,
            );
            draft_mark(p, viewport, *center, "wb-draft-center");
        }
        ConstructionPreviewGeometry::CounterClockwiseArc {
            center,
            start,
            end,
            radius,
            large_arc,
            ..
        } => draft_arc(
            p, viewport, *center, *start, *end, *radius, *large_arc, false,
        ),
        ConstructionPreviewGeometry::CircularArc {
            center,
            start,
            end,
            radius,
            large_arc,
            sweep,
            ..
        } => draft_arc(
            p,
            viewport,
            *center,
            *start,
            *end,
            *radius,
            *large_arc,
            *sweep == DocumentArcSweep::Clockwise,
        ),
        ConstructionPreviewGeometry::AdvancedCurve {
            kind,
            control_points,
            curve_points,
        } => {
            draft_controls(p, viewport, *kind, control_points);
            draft_line(p, viewport, curve_points, "wb-draft-advanced-curve", false);
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn draft_arc(
    p: &mut Painter,
    viewport: Viewport,
    center: [f64; 2],
    start: [f64; 2],
    end: [f64; 2],
    radius: f64,
    large: bool,
    clockwise: bool,
) {
    let points = p.arc(
        viewport.model_to_screen(start),
        viewport.model_to_screen(end),
        radius * viewport.pixels_per_model_unit,
        large,
        clockwise,
    );
    p.line(
        "wb-draft-arc",
        "draft",
        "wb-draft-arc",
        None,
        draft_style(),
        &points,
    );
    for (point, name) in [
        (center, "wb-draft-center"),
        (start, "wb-draft-start"),
        (end, "wb-draft-end"),
    ] {
        draft_mark(p, viewport, point, name);
    }
}
/// Paints resolved prediction geometry without a document or publication handle.
/// The caller must retain its accepted picking scene separately.
///
/// # Errors
/// Rejects invalid viewports and non-finite or invalid drawing primitives.
pub fn compose_prediction_guides(
    viewport: Viewport,
    preview: Option<&ConstructionPreviewGeometry>,
    guides: &[DraftGuideGeometry],
) -> Result<DrawFrame, DrawFrameError> {
    if let Some(ConstructionPreviewGeometry::Rectangle { first, second }) = preview
        && first
            .iter()
            .chain(second.iter())
            .any(|value| !value.is_finite())
    {
        return Err(DrawFrameError("invalid prediction rectangle".into()));
    }
    Viewport::new(
        viewport.screen_size,
        viewport.model_center,
        viewport.pixels_per_model_unit,
    )
    .map_err(|error| DrawFrameError(error.to_string()))?;
    let mut painter = Painter {
        frame: DrawFrame {
            format: "geosolve-draw-frame-v1",
            view_box: [0.0, 0.0, viewport.screen_size[0], viewport.screen_size[1]],
            background: "#151619",
            provenance: BTreeMap::from([("scene".into(), "provisional".into())]),
            items: Vec::new(),
        },
        occurrences: BTreeMap::new(),
        invalid: None,
    };
    if let Some(preview) = preview {
        draw_preview_geometry(&mut painter, preview, viewport);
    }
    for (index, guide) in guides.iter().enumerate() {
        let base = format!("prediction-guide:{index}");
        let style = DrawStyle::stroke("#79d6ca", 1.8).dashed(&[7.0, 4.0]);
        match guide {
            DraftGuideGeometry::Point { position } => {
                painter.circle(
                    &base,
                    "inferenceGuides",
                    "wb-inference-guide-point",
                    None,
                    style,
                    viewport.model_to_screen(*position),
                    7.0,
                );
            }
            DraftGuideGeometry::Segment { start, end } => {
                painter.line(
                    &base,
                    "inferenceGuides",
                    "wb-inference-guide-segment",
                    None,
                    style,
                    &[
                        viewport.model_to_screen(*start),
                        viewport.model_to_screen(*end),
                    ],
                );
            }
        }
    }
    if let Some(error) = painter.invalid {
        return Err(error);
    }
    for item in &mut painter.frame.items {
        item.id = format!("prediction:{}", item.id);
        item.interactive = false;
    }
    painter.frame.validate()?;
    Ok(painter.frame)
}

fn draw_inference_guides(
    p: &mut Painter,
    resolution: &DraftInferenceResolution,
    viewport: Viewport,
) {
    for guide in &resolution.guides {
        let backed = guide.classification == DraftGuideClassification::ConstraintBacked;
        let mut s = if backed {
            DrawStyle::stroke("#79d6ca", 1.8).glow("#79d6ca61", 3.0)
        } else {
            let mut s = DrawStyle::stroke("#8a9897", 1.25).dashed(&[2.0, 5.0]);
            s.opacity = 0.82;
            s
        };
        let base = format!(
            "inference-guide:{:?}:{}",
            guide.id.candidate, guide.id.ordinal
        );
        let item = match guide.geometry {
            DraftGuideGeometry::Point { position } => p.circle(
                &base,
                "inferenceGuides",
                "wb-inference-guide-point",
                None,
                s,
                viewport.model_to_screen(position),
                7.0,
            ),
            DraftGuideGeometry::Segment { start, end } => {
                if backed {
                    s.dash = vec![7.0, 4.0];
                }
                p.line(
                    &base,
                    "inferenceGuides",
                    "wb-inference-guide-segment",
                    None,
                    s,
                    &[
                        viewport.model_to_screen(start),
                        viewport.model_to_screen(end),
                    ],
                )
            }
        };
        item.accessible_label = Some(crate::scene::inference_family_label(guide.family).into());
        item.metadata.insert(
            "inferenceFamily".into(),
            crate::scene::inference_family_key(guide.family).into(),
        );
        item.metadata.insert(
            "classification".into(),
            if backed {
                "constraint-backed"
            } else {
                "tracking-only"
            }
            .into(),
        );
    }
}
fn draw_inference_candidates(p: &mut Painter, resolution: &DraftInferenceResolution) {
    let ids = match &resolution.status {
        DraftInferenceStatus::Resolved { candidate } => vec![*candidate],
        DraftInferenceStatus::Ambiguous { candidates } => candidates.clone(),
        _ => Vec::new(),
    };
    let ambiguous = matches!(resolution.status, DraftInferenceStatus::Ambiguous { .. });
    let count = f64::from(u32::try_from(ids.len()).unwrap_or(u32::MAX));
    for (index, id) in ids.iter().enumerate() {
        let Some(candidate) = resolution.candidates.iter().find(|c| c.id == *id) else {
            continue;
        };
        for (relation_index, relation) in candidate.relations.iter().copied().enumerate() {
            let (key, label, glyph) = crate::scene::inference_relation_presentation(relation);
            let x = candidate.adjusted_screen_position.x
                + 16.0
                + 22.0 * f64::from(u32::try_from(relation_index).unwrap_or(u32::MAX));
            let y = candidate.adjusted_screen_position.y - 16.0
                + (f64::from(u32::try_from(index).unwrap_or(u32::MAX)) - (count - 1.0) * 0.5)
                    * 24.0;
            let base = format!("inference-candidate:{}:{key}:{relation_index}", id.get());
            let mut s = DrawStyle::stroke(if ambiguous { "#e2ad5d" } else { "#79d6ca" }, 1.25);
            s.fill = Some("#171c1df0".into());
            p.circle(
                &format!("{base}:background"),
                "inferenceCandidates",
                "wb-inference-glyph-background",
                None,
                s,
                sp([x, y]),
                10.0,
            );
            let first = p.frame.items.len();
            draw_glyph(
                p,
                &base,
                "inferenceCandidates",
                None,
                glyph,
                sp([x, y]),
                0.0,
                &DrawStyle::stroke(if ambiguous { "#ffe0a1" } else { "#c8fff7" }, 1.65),
            );
            for item in &mut p.frame.items[first..] {
                item.accessible_label = Some(label.into());
                item.metadata
                    .insert("candidate".into(), id.get().to_string());
                item.metadata.insert("relation".into(), key.into());
            }
        }
    }
    if let Some((key, message)) = crate::scene::inference_status_warning(&resolution.status) {
        let suppressed = key == "suppressed";
        let mut s = DrawStyle::stroke(if suppressed { "#718080" } else { "#b88743" }, 1.0);
        s.fill = Some(if suppressed { "#1e2323f0" } else { "#292218f5" }.into());
        let node = p.push(
            "inference-state",
            "inferenceCandidates",
            "wb-inference-state",
            None,
            s,
            DrawGeometry::Rect {
                x: 18.0,
                y: 18.0,
                width: 310.0,
                height: 30.0,
                radius: 5.0,
            },
        );
        node.accessible_label = Some(message.into());
        let mut s = DrawStyle::fill(if suppressed { "#b9c5c4" } else { "#f2d59c" });
        s.font_family = "ui-sans-serif,system-ui,sans-serif";
        s.font_weight = 600;
        p.text(
            "inference-state:text",
            "inferenceCandidates",
            "wb-inference-state-text",
            None,
            s,
            sp([30.0, 38.0]),
            message,
        );
    }
}
fn error_marker(p: &mut Painter, key: &str, anchor: ScreenPoint, message: &str, global: bool) {
    let mut s = DrawStyle::stroke("#ffd7d1", 1.5);
    s.fill = Some(if global { "#d14b3f" } else { "#b83d32" }.into());
    let node = p.circle(
        key,
        "problems",
        "wb-error-marker",
        Some(key),
        s,
        anchor,
        10.0,
    );
    node.accessible_label = Some(message.into());
    node.metadata.insert("tooltip".into(), message.into());
    node.metadata.insert("global".into(), global.to_string());
    p.line(
        &format!("{key}:icon"),
        "problems",
        "wb-error-marker-icon",
        Some(key),
        DrawStyle::stroke("#ffffff", 2.0),
        &[
            sp([anchor.x, anchor.y - 6.0]),
            sp([anchor.x, anchor.y + 1.0]),
        ],
    );
    p.circle(
        &format!("{key}:dot"),
        "problems",
        "wb-error-marker-icon",
        Some(key),
        DrawStyle::fill("#ffffff"),
        sp([anchor.x, anchor.y + 5.1]),
        1.0,
    );
}
fn draw_problems(
    p: &mut Painter,
    scene: Option<&EditorScene>,
    problem: Option<&EditorProblemMetadata>,
    computed: &[ComputedFeatureProblemMetadata],
    policy: GeometryInteractionPolicy,
) {
    if let Some(problem) = problem {
        let mut resolved = BTreeSet::new();
        if let Some(scene) = scene {
            for target in &problem.targets {
                if !resolved.insert(*target) {
                    continue;
                }
                let anchor = match target {
                    EditorProblemTarget::Point(id) => scene
                        .points
                        .iter()
                        .find(|x| x.id == *id && x.is_visible(policy))
                        .map(|x| x.screen_position),
                    EditorProblemTarget::Curve(id) => scene
                        .curves
                        .iter()
                        .find(|x| {
                            x.span.curve == *id
                                && x.is_visible(policy)
                                && !x.screen_polyline.is_empty()
                        })
                        .map(|x| x.screen_polyline[x.screen_polyline.len() / 2]),
                    EditorProblemTarget::Constraint(id) => scene
                        .annotations
                        .iter()
                        .filter(|_| scene.annotations_visible)
                        .find(|x| x.item == SelectionItem::Constraint(*id))
                        .and_then(|x| crate::scene::annotation_anchor(&x.geometry)),
                    EditorProblemTarget::Dimension(id) => scene
                        .annotations
                        .iter()
                        .filter(|_| scene.annotations_visible)
                        .find(|x| x.item == SelectionItem::Dimension(*id))
                        .and_then(|x| crate::scene::annotation_anchor(&x.geometry)),
                };
                if let Some(anchor) = anchor {
                    error_marker(
                        p,
                        &format!("problem:{target:?}"),
                        anchor,
                        &problem.message,
                        false,
                    );
                } else {
                    resolved.remove(target);
                }
            }
        }
        if problem.scope == EditorProblemScope::Global || resolved.is_empty() {
            error_marker(
                p,
                "problem:global",
                sp([970.0, 28.0]),
                &problem.message,
                true,
            );
        }
    }
    if let Some(scene) = scene {
        for (index, problem) in computed.iter().enumerate() {
            let anchor = (problem.scope == EditorProblemScope::Targeted)
                .then(|| {
                    problem.sources.iter().find_map(|source| {
                        scene
                            .curves
                            .iter()
                            .find(|c| {
                                c.span == source.span
                                    && c.is_visible(policy)
                                    && !c.screen_polyline.is_empty()
                            })
                            .map(|c| c.screen_polyline[c.screen_polyline.len() / 2])
                    })
                })
                .flatten();
            let global = anchor.is_none();
            let position = anchor.unwrap_or(sp([
                970.0,
                28.0 + 24.0 * f64::from(u32::try_from(index).unwrap_or(u32::MAX)),
            ]));
            error_marker(
                p,
                &format!("computed-problem:{:?}:{index}", problem.feature),
                position,
                &problem.message,
                global,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geosolve_constraint_editor::{
        RetainedEditorCoordinator, SceneFilletAction, SceneFilletAlternativeGeometry,
    };
    use geosolve_sketch::{
        ContactNeighborhood, CurveDefinition, CurveSpan, DocumentCurveNormalSide,
        DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentSolveRequest,
        RetainedSketchDocumentSession, SketchDocument, SolverConfig,
    };
    use geosolve_sketch_features::{
        ComputedFeatureDocument, ComputedFilletParent, NativeCurveSpanSource,
        NewComputedFilletCorner,
    };

    fn session() -> RetainedSketchDocumentSession {
        let mut d = SketchDocument::new(8.0).unwrap();
        d.add_rectangle("draw contract", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        RetainedSketchDocumentSession::new(
            d,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap()
    }
    fn scene(session: &RetainedSketchDocumentSession, viewport: Viewport) -> EditorScene {
        let accepted = session.accepted_state().unwrap();
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.25,
        )
        .unwrap();
        scene.set_show_all_constraint_annotations(true);
        assert!(scene.update_annotation_values(accepted));
        scene
    }
    fn frame(
        scene: &EditorScene,
        accepted: &SketchAcceptedDocumentState,
        selection: &[SelectionItem],
    ) -> DrawFrame {
        compose_draw_frame(
            Some(scene),
            Some(accepted),
            &[],
            selection,
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
            scene.viewport,
        )
        .unwrap()
    }

    fn fillet_scene() -> (RetainedEditorCoordinator, EditorScene) {
        let mut document = SketchDocument::new(10.0).unwrap();
        let points = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]]
            .map(|position| document.add_point("corner point", position).unwrap());
        let curve = document
            .add_curve(
                "corner supports",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
                },
            )
            .unwrap();
        let parent = |segment, picked_parameter, retained_endpoint| ComputedFilletParent {
            source: NativeCurveSpanSource {
                span: CurveSpan { curve, segment },
            },
            picked_parameter,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            normal_side: DocumentCurveNormalSide::Left,
            retained_endpoint,
            periodic_anchor: None,
        };
        let mut features = ComputedFeatureDocument::new(document.id());
        features
            .create_fillet_set(
                "rounded corner",
                0.5,
                vec![NewComputedFilletCorner {
                    first: parent(0, 0.875, DocumentFilletTrimEndpoint::End),
                    second: parent(1, 0.125, DocumentFilletTrimEndpoint::Start),
                    endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                    sweep: DocumentArcSweep::CounterClockwise,
                }],
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let coordinator = RetainedEditorCoordinator::with_features(session, features).unwrap();
        let session = coordinator.session();
        let accepted = session.accepted_state_for_current_input().unwrap();
        let computed = coordinator.computed_snapshot().unwrap();
        let mut scene = EditorScene::from_accepted_with_computed(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            &session.accepted_prepared_input().unwrap(),
            &computed.input(),
            computed,
            crate::viewport(),
            0.25,
        )
        .unwrap();
        let owner = scene.computed_curves[0].owner;
        coordinator
            .populate_computed_fillet_affordances(
                &mut scene,
                &[SelectionItem::FeatureCorner(owner)],
                0.25,
            )
            .unwrap();
        (coordinator, scene)
    }

    #[test]
    fn detached_fillet_scene_does_not_preview_an_action_without_an_active_target() {
        let (coordinator, mut native) = fillet_scene();
        let owner = native.computed_curves[0].owner;
        let action_id = SceneFilletActionId::ComplementaryArc;
        let model_polyline = vec![[3.5, 0.0], [4.5, -0.5], [4.0, 0.5]];
        native
            .set_fillet_corner_actions(
                owner,
                vec![SceneFilletAction {
                    id: action_id,
                    owner,
                    label: "Preview complementary arc".into(),
                    availability: SceneFilletActionAvailability::Applicable,
                    control_geometry: None,
                    dashed_alternative_arc: Some(SceneFilletAlternativeGeometry {
                        screen_polyline: model_polyline
                            .iter()
                            .map(|point| native.viewport.model_to_screen(*point))
                            .collect(),
                        model_polyline,
                    }),
                }],
            )
            .unwrap();
        let detached =
            EditorScene::from_detached_json(&native.to_detached_json().unwrap()).unwrap();
        let target = native.fillet_action_target(owner, action_id).unwrap();
        assert_eq!(detached.fillet_action_target(owner, action_id), None);
        let selection = [SelectionItem::FeatureCorner(owner)];
        let render = |scene: &EditorScene, active| {
            compose_draw_frame(
                Some(scene),
                coordinator.session().accepted_state(),
                &[],
                &selection,
                &[],
                &[],
                EditorHoverState::default(),
                None,
                None,
                None,
                active,
                None,
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                scene.viewport,
            )
            .unwrap()
        };
        let native_frame = render(&native, None);
        let detached_frame = render(&detached, None);
        assert!(
            native_frame
                .items
                .iter()
                .any(|item| item.class_name == "wb-fillet-action-control")
        );
        for frame in [&native_frame, &detached_frame] {
            assert!(
                frame
                    .items
                    .iter()
                    .all(|item| item.class_name != "wb-fillet-alternative-ghost")
            );
        }
        assert_eq!(detached_frame.items, native_frame.items);
        assert_eq!(render(&detached, Some(&target)).items, native_frame.items);
        let preview = render(&native, Some(&target));
        assert_eq!(
            preview
                .items
                .iter()
                .filter(|item| item.class_name == "wb-fillet-alternative-ghost")
                .count(),
            1
        );
    }
    #[test]
    fn draw_frame_is_numeric_deterministic_and_preserves_accepted_provenance() {
        let owner = session();
        let scene = scene(&owner, crate::viewport());
        let accepted = owner.accepted_state().unwrap();
        let before = scene.clone();
        let first = frame(&scene, accepted, &[]);
        let second = frame(&scene, accepted, &[]);
        assert_eq!(first, second);
        assert_eq!(scene, before);
        first.validate().unwrap();
        assert_eq!(
            first.provenance["document"],
            accepted.identity().document().to_string()
        );
        assert_eq!(
            first.provenance["revision"],
            accepted.identity().revision().get().to_string()
        );
        let json = serde_json::to_value(&first).unwrap();
        assert_eq!(json["format"], "geosolve-draw-frame-v1");
        assert!(json.get("svg").is_none());
        assert_eq!(
            first
                .items
                .iter()
                .filter(|x| x.class_name == "wb-curve")
                .count(),
            scene.curves.len()
        );
        for curve in &scene.curves {
            let key = semantic(SelectionItem::Curve(curve.span));
            let item = first
                .items
                .iter()
                .find(|x| x.semantic_key.as_deref() == Some(&key) && x.class_name == "wb-curve")
                .unwrap();
            assert_eq!(
                item.geometry,
                DrawGeometry::Polyline {
                    points: curve.screen_polyline.iter().copied().map(xy).collect(),
                    closed: false
                }
            );
        }
    }
    #[test]
    fn drawing_layer_order_and_semantic_keys_survive_selection() {
        let owner = session();
        let scene = scene(&owner, crate::viewport());
        let accepted = owner.accepted_state().unwrap();
        let first = frame(&scene, accepted, &[]);
        let selected = SelectionItem::Point(scene.points[0].id);
        let second = frame(&scene, accepted, &[selected]);
        let key = semantic(selected);
        let a = first
            .items
            .iter()
            .find(|x| x.semantic_key.as_deref() == Some(&key))
            .unwrap();
        let b = second.items.iter().find(|x| x.id == a.id).unwrap();
        assert_eq!(a.geometry, b.geometry);
        assert_ne!(a.style.stroke, b.style.stroke);
        assert_eq!(b.style.stroke.as_deref(), Some("#efb856"));
        let last_grid = first.items.iter().rposition(|x| x.layer == "grid").unwrap();
        let first_curve = first
            .items
            .iter()
            .position(|x| x.layer == "geometry")
            .unwrap();
        let first_point = first
            .items
            .iter()
            .position(|x| x.layer == "points")
            .unwrap();
        let first_annotation = first
            .items
            .iter()
            .position(|x| x.layer == "annotations")
            .unwrap();
        assert!(
            last_grid < first_curve && first_curve < first_point && first_point < first_annotation
        );
        let ids = first.items.iter().map(|x| &x.id).collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), first.items.len());
    }
    #[test]
    fn drawing_reprojection_matches_cold_scene_without_mutating_the_owner() {
        let owner = session();
        let mut retained = scene(&owner, crate::viewport());
        let accepted = owner.accepted_state().unwrap();
        let viewport = Viewport::new([1000.0, 700.0], [1.5, -2.0], 83.0).unwrap();
        retained.reproject_viewport(viewport).unwrap();
        let cold = scene(&owner, viewport);
        let a = frame(&retained, accepted, &[]);
        let b = frame(&cold, accepted, &[]);
        let native = |f: DrawFrame| {
            f.items
                .into_iter()
                .filter(|x| matches!(x.layer, "geometry" | "points" | "datums"))
                .collect::<Vec<_>>()
        };
        assert_eq!(native(a), native(b));
    }
    #[test]
    fn drawing_rejects_nonfinite_geometry_and_bad_viewports() {
        let owner = session();
        let mut scene = scene(&owner, crate::viewport());
        scene.points[0].screen_position.x = f64::NAN;
        assert!(
            compose_draw_frame(
                Some(&scene),
                owner.accepted_state(),
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
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                scene.viewport
            )
            .is_err()
        );
        let invalid = Viewport {
            screen_size: [f64::INFINITY, 700.0],
            model_center: [0.0, 0.0],
            pixels_per_model_unit: 50.0,
        };
        assert!(
            compose_draw_frame(
                None,
                None,
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
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                invalid
            )
            .is_err()
        );
    }
    #[test]
    fn every_constraint_glyph_has_finite_numeric_paint() {
        for g in [
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
        ] {
            let mut p = Painter {
                frame: DrawFrame {
                    format: "geosolve-draw-frame-v1",
                    view_box: [0.0, 0.0, 100.0, 100.0],
                    background: "#151619",
                    provenance: BTreeMap::new(),
                    items: Vec::new(),
                },
                occurrences: BTreeMap::new(),
                invalid: None,
            };
            draw_glyph(
                &mut p,
                "glyph",
                "annotations",
                None,
                g,
                sp([50.0, 50.0]),
                0.73,
                &DrawStyle::stroke("#d7a654", 1.8),
            );
            assert!(!p.frame.items.is_empty());
            p.frame.validate().unwrap();
        }
    }
    #[test]
    fn overlay_numeric_drafts_do_not_change_accepted_frame_items() {
        let owner = session();
        let scene = scene(&owner, crate::viewport());
        let accepted = owner.accepted_state().unwrap();
        let initial = frame(&scene, accepted, &[]);
        let preview = ConstructionPreview::GuidePolyline {
            points: vec![[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]],
            closed: true,
        };
        let draft = compose_draw_frame(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            Some(&preview),
            None,
            None,
            None,
            None,
            GeometryInteractionPolicy::default(),
            CanvasDisplayOptions::default(),
            None,
            scene.viewport,
        )
        .unwrap();
        assert_eq!(&draft.items[..initial.items.len()], &initial.items);
        assert!(
            draft.items[initial.items.len()..]
                .iter()
                .all(|x| x.layer == "draft")
        );
    }
    #[test]
    fn m97_dimension_policy_agrees_in_numeric_svg_static_and_hidden_contextual_exports() {
        use geosolve_constraint_editor::{
            AnnotationLayoutState, DimensionDisplayMode, DimensionPresentationContext,
            DimensionPresentationState,
        };
        let owner = session();
        let mut scene = scene(&owner, crate::viewport());
        let accepted = owner.accepted_state().unwrap();
        let mut state = DimensionPresentationState::default();
        state.mode = DimensionDisplayMode::Focused;
        let rows = state.apply(
            &mut scene,
            &AnnotationLayoutState::default(),
            &DimensionPresentationContext::default(),
        );
        assert!(!rows.is_empty());
        let selection: Vec<_> = rows.iter().map(|row| row.key.item).collect();
        let drawing = frame(&scene, accepted, &selection);
        assert!(
            drawing
                .items
                .iter()
                .all(|item| !item.class_name.starts_with("wb-dimension"))
        );
        let svg = crate::svg_markup_with_computed_context_action_stamp_display_and_provisional(
            Some(&scene),
            Some(accepted),
            &[],
            &selection,
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
                retain_contextual_annotations: true,
                ..CanvasDisplayOptions::default()
            },
            None,
            scene.viewport,
        );
        assert!(!svg.contains("data-editor-kind=\"dimension\""));
        let exported = crate::compose_static_scene_svg(
            Some(&scene),
            Some(accepted),
            crate::CanvasCamera::default(),
        );
        assert!(!exported.contains("wb-annotation wb-dimension"));
        state.focus = Some(rows[0].key);
        let focused = state.apply(
            &mut scene,
            &AnnotationLayoutState::default(),
            &DimensionPresentationContext::default(),
        );
        assert!(focused[0].visible);
        assert!(
            frame(&scene, accepted, &[])
                .items
                .iter()
                .any(|item| item.class_name.starts_with("wb-dimension"))
        );
        assert!(
            crate::compose_static_scene_svg(
                Some(&scene),
                Some(accepted),
                crate::CanvasCamera::default()
            )
            .contains("wb-annotation wb-dimension")
        );
    }

    #[test]
    fn dimensions_paint_the_exact_headless_arrowheads_and_mask_bounds() {
        let owner = session();
        let mut scene = scene(&owner, crate::viewport());
        let mut annotation = scene
            .annotations
            .iter()
            .find(|a| matches!(a.item, SelectionItem::Dimension(_)))
            .unwrap()
            .clone();
        annotation.geometry = SceneAnnotationGeometry::LinearDimension {
            measured_first: sp([20.0, 30.0]),
            measured_second: sp([80.0, 30.0]),
            first: sp([20.0, 55.0]),
            second: sp([80.0, 55.0]),
            label_anchor: sp([50.0, 55.0]),
        };
        annotation.label_bounds = Some(geosolve_constraint_editor::SceneAnnotationLabelBounds {
            min: sp([40.0, 49.0]),
            max: sp([60.0, 62.0]),
        });
        annotation.visible_text = Some("40 mm".into());
        let arrows = annotation.geometry.arrowheads();
        scene.annotations = vec![annotation];
        let frame = frame(&scene, owner.accepted_state().unwrap(), &[]);
        let drawn = frame
            .items
            .iter()
            .filter(|x| x.class_name == "wb-dimension-arrow")
            .collect::<Vec<_>>();
        assert_eq!(drawn.len(), arrows.len());
        for (paint, arrow) in drawn.iter().zip(arrows) {
            assert_eq!(
                paint.geometry,
                DrawGeometry::Polyline {
                    points: vec![xy(arrow.tip), xy(arrow.base_first), xy(arrow.base_second)],
                    closed: true
                }
            );
        }
        let mask = frame
            .items
            .iter()
            .find(|x| x.class_name == "wb-dimension-label-mask")
            .unwrap();
        assert_eq!(
            mask.geometry,
            DrawGeometry::Rect {
                x: 40.0,
                y: 49.0,
                width: 20.0,
                height: 13.0,
                radius: 3.0
            }
        );
        assert!(
            frame
                .items
                .iter()
                .any(|x| matches!(&x.geometry,DrawGeometry::Text{text,..} if text=="40 mm"))
        );
    }
    #[test]
    fn nonfinite_angular_radius_cannot_be_normalized_into_valid_paint() {
        let owner = session();
        let mut scene = scene(&owner, crate::viewport());
        let mut a = scene
            .annotations
            .iter()
            .find(|a| matches!(a.item, SelectionItem::Dimension(_)))
            .unwrap()
            .clone();
        a.geometry = SceneAnnotationGeometry::AngularDimension {
            vertex: sp([50.0, 50.0]),
            first_ray: sp([100.0, 50.0]),
            second_ray: sp([50.0, 100.0]),
            radius: f64::NAN,
            clockwise: true,
            label_anchor: sp([80.0, 80.0]),
        };
        scene.annotations = vec![a];
        assert!(
            compose_draw_frame(
                Some(&scene),
                owner.accepted_state(),
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
                GeometryInteractionPolicy::default(),
                CanvasDisplayOptions::default(),
                None,
                scene.viewport
            )
            .is_err()
        );
    }
}
