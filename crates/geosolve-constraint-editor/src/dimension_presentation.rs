// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained dimension visibility and placement, independent of design history.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AnnotationLayoutKey, AnnotationLayoutState, EditorScene, SceneAnnotation, SceneAnnotationKind,
    SelectionItem, Viewport,
};

/// Canvas dimension density. Standalone scene construction retains legacy defaults.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DimensionDisplayMode {
    Focused,
    #[default]
    All,
    Hidden,
}

/// Exact accepted context supplied by a host. Timing and pointer gestures remain host-owned.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionPresentationContext {
    pub selection: Vec<SelectionItem>,
    pub hovered: Option<SelectionItem>,
    pub generated: BTreeSet<SelectionItem>,
    /// Design-intent measurements eligible for unselected Focused presentation.
    /// This affects annotation visibility only, never solver priority.
    pub default_priority: BTreeSet<SelectionItem>,
    pub navigation_active: bool,
    /// A dimension currently being authored or edited, including in Hidden mode.
    pub active: Option<SelectionItem>,
}

/// Complete accepted measurement metadata, including rows omitted from the canvas.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent accepted and presentation flags compose without exclusive states"
)]
pub struct SceneDimensionEntry {
    pub key: AnnotationLayoutKey,
    pub label: String,
    pub kind: SceneAnnotationKind,
    pub operands: Vec<SelectionItem>,
    pub value_text: Option<String>,
    pub reference: bool,
    pub suppressed: bool,
    pub related: bool,
    pub generated: bool,
    pub default_priority: bool,
    pub visible: bool,
    pub pinned: bool,
    pub focused: bool,
}

impl SceneDimensionEntry {
    /// Exact native target and display conversion for this document/source identity.
    /// Reference measurements use the accepted state's measured value instead.
    #[must_use]
    pub fn target_metadata(&self, scene: &EditorScene) -> Option<crate::DimensionTargetMetadata> {
        use geosolve_sketch::DocumentDimensionDefinition as Dimension;
        let document = &scene.accepted_document;
        let SelectionItem::Dimension(id) = self.key.item else {
            return None;
        };
        if document.id() != self.key.document {
            return None;
        }
        let dimension = document
            .dimension(id)
            .filter(|dimension| dimension.source_id == self.key.source)?;
        let target = match dimension.definition {
            Dimension::PointDistance { target, .. }
            | Dimension::CurveLength { target, .. }
            | Dimension::Radius { target, .. }
            | Dimension::Diameter { target, .. }
            | Dimension::OrientedAngle { target, .. }
            | Dimension::SupportingLineOffset { target, .. }
            | Dimension::ExactTranslatedSegmentOffset { target, .. }
            | Dimension::ProfileOffset { target, .. } => target,
        };
        let scalar = document.scalar(target)?;
        let display = crate::display_dimension_target(scalar.value, scalar.unit)?;
        Some(crate::DimensionTargetMetadata {
            dimension: id,
            scalar: target,
            value: scalar.value,
            unit: scalar.unit,
            display_value: display.value,
            display_unit: display.unit,
            mode: dimension.mode,
        })
    }
}

/// Host-retained presentation preferences plus automatic slots. Only mode and pins
/// should be persisted; the cache and inspection context are transient.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionPresentationState {
    pub mode: DimensionDisplayMode,
    pub focus: Option<AnnotationLayoutKey>,
    pub pins: Vec<AnnotationLayoutKey>,
    retained: BTreeMap<AnnotationLayoutKey, RetainedDimension>,
    candidates: Vec<AnnotationLayoutKey>,
    base: Option<DimensionPresentationBase>,
    interest: Option<DimensionPresentationInterest>,
    last_viewport: Option<Viewport>,
    navigation_seen: bool,
    reconsider_hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct DimensionPresentationInterest {
    mode: DimensionDisplayMode,
    focus: Option<AnnotationLayoutKey>,
    pins: Vec<AnnotationLayoutKey>,
    context: DimensionPresentationContext,
}

#[derive(Clone, Debug, PartialEq)]
struct DimensionPresentationBase {
    identity: (geosolve_sketch::DocumentId, u64),
    document: geosolve_sketch::SketchDocument,
    hidden_items: BTreeSet<SelectionItem>,
    viewport: Viewport,
    annotations: Vec<SceneAnnotation>,
    entries: Vec<SceneDimensionEntry>,
}

#[derive(Clone, Debug, PartialEq)]
struct RetainedDimension {
    annotation: SceneAnnotation,
    viewport: Viewport,
    model_anchors: Vec<[f64; 2]>,
    manual: Option<crate::AnnotationPlacement>,
    visible: bool,
}

impl DimensionPresentationState {
    /// Ordinary Focused candidate budget. Default priorities remain eligible
    /// beyond this budget; every candidate still needs a readable layout slot.
    pub const MAX_VISIBLE: usize = 6;
    pub const MAX_PINS: usize = 4;

    /// Requests one bounded placement search for hidden dimensions on the next
    /// idle application, for example after an explicit Fit action. Visible
    /// placements remain retained; ordinary camera navigation never requests this.
    pub fn reconsider_hidden_dimensions(&mut self) {
        self.reconsider_hidden = true;
    }

    /// Resolves the complete dimension policy into the shared scene used by paint
    /// and picking. Accepted geometry, source and design history are unchanged.
    pub fn apply(
        &mut self,
        scene: &mut EditorScene,
        manual_layout: &AnnotationLayoutState,
        context: &DimensionPresentationContext,
    ) -> Vec<SceneDimensionEntry> {
        self.resolve(scene, manual_layout, context)
    }
}

#[cfg(test)]
mod tests;

impl DimensionPresentationState {
    #[allow(
        clippy::too_many_lines,
        reason = "one pass resolves shared visibility and retained slots transactionally"
    )]
    fn resolve(
        &mut self,
        scene: &mut EditorScene,
        manual_layout: &AnnotationLayoutState,
        context: &DimensionPresentationContext,
    ) -> Vec<SceneDimensionEntry> {
        use crate::{SceneAnnotationVisibility, annotations};
        let sealed = scene.retained_reprojection_semantics_are_sealed();
        let document = &scene.accepted_document;
        let document_id = document.id();
        let previous: BTreeMap<_, _> = scene
            .annotations
            .iter()
            .map(|annotation| (annotation.layout_key(document_id, None), annotation))
            .collect();
        let identity = (document_id, scene.accepted_revision);
        let changed_geometry = self
            .base
            .as_ref()
            .is_none_or(|base| base.identity != identity || base.document != *document);
        let changed_visibility = self
            .base
            .as_ref()
            .is_none_or(|base| base.hidden_items != scene.hidden_presentation_items);
        if changed_geometry || changed_visibility {
            self.base = Some(DimensionPresentationBase {
                identity,
                document: document.clone(),
                hidden_items: scene.hidden_presentation_items.clone(),
                viewport: scene.viewport,
                annotations: annotations::build_dimension_annotations_unlaid(
                    document,
                    &scene.points,
                    &scene.curves,
                    scene.viewport,
                ),
                entries: dimension_entries(scene),
            });
        }
        let base = self.base.as_ref().expect("dimension base was established");
        let mut raw_annotations = base.annotations.clone();
        annotations::reproject_retained_layout(&mut raw_annotations, base.viewport, scene.viewport);
        let mut raw: BTreeMap<_, _> = raw_annotations
            .into_iter()
            .filter_map(|mut annotation| {
                let key = annotation.layout_key(document_id, None);
                let prior = previous.get(&key)?;
                annotation.visible_text.clone_from(&prior.visible_text);
                annotation
                    .accessible_label
                    .clone_from(&prior.accessible_label);
                annotation.refresh_label_bounds();
                Some((key, annotation))
            })
            .collect();
        let expanded_selection = expanded_context(document, &context.selection);
        let expanded_hover =
            expanded_context(document, &context.hovered.into_iter().collect::<Vec<_>>());
        let mut entries = base.entries.clone();
        let valid: BTreeSet<_> = entries.iter().map(|entry| entry.key).collect();
        let mut unique_pins = BTreeSet::new();
        self.pins
            .retain(|key| valid.contains(key) && unique_pins.insert(*key));
        self.pins.truncate(Self::MAX_PINS);
        self.focus = self.focus.filter(|key| valid.contains(key));
        self.retained.retain(|key, _| valid.contains(key));
        for entry in &mut entries {
            entry.value_text = previous
                .get(&entry.key)
                .and_then(|annotation| annotation.visible_text.clone());
            entry.related = related(entry, &expanded_selection);
            entry.generated = context.generated.contains(&entry.key.item);
            entry.default_priority = context.default_priority.contains(&entry.key.item);
            entry.pinned = self.pins.contains(&entry.key);
            entry.focused = self.focus == Some(entry.key);
        }
        let mut interest_context = context.clone();
        interest_context.navigation_active = false;
        let interest = DimensionPresentationInterest {
            mode: self.mode,
            focus: self.focus,
            pins: self.pins.clone(),
            context: interest_context,
        };
        let navigation_change = self.navigation_seen
            || self
                .last_viewport
                .is_some_and(|viewport| viewport != scene.viewport);
        let reconsider_hidden =
            !context.navigation_active && std::mem::take(&mut self.reconsider_hidden);
        if reconsider_hidden {
            self.retained.retain(|_, retained| retained.visible);
        }
        let may_restore = changed_geometry
            || changed_visibility
            || reconsider_hidden
            || (!navigation_change && self.interest.as_ref() != Some(&interest));
        self.navigation_seen = context.navigation_active;
        self.last_viewport = Some(scene.viewport);
        if !context.navigation_active {
            self.interest = Some(interest);
        }
        let hover_key = entries
            .iter()
            .filter(|entry| related(entry, &expanded_hover))
            .min_by_key(|entry| (entry.generated, entry.key.source))
            .map(|entry| entry.key);
        if !context.navigation_active {
            let mut ranked: Vec<_> = entries
                .iter()
                .filter(|entry| {
                    let active = context.active == Some(entry.key.item);
                    match self.mode {
                        DimensionDisplayMode::All => true,
                        DimensionDisplayMode::Hidden => active,
                        DimensionDisplayMode::Focused => {
                            active
                                || entry.focused
                                || entry.pinned
                                || entry.related
                                || entry.default_priority
                                || hover_key == Some(entry.key)
                        }
                    }
                })
                .collect();
            ranked.sort_by_key(|entry| {
                let rank = if context.active == Some(entry.key.item) || entry.focused {
                    0
                } else if entry.pinned {
                    1
                } else if hover_key == Some(entry.key) {
                    2
                } else if entry.related && !entry.generated {
                    3
                } else if entry.related {
                    4
                } else if entry.default_priority || !entry.generated {
                    5
                } else {
                    6
                };
                (rank, entry.key.source)
            });
            let mut ordinary = 0;
            self.candidates = ranked
                .into_iter()
                .filter(|entry| {
                    if self.mode == DimensionDisplayMode::All || entry.default_priority {
                        return true;
                    }
                    ordinary += 1;
                    ordinary <= Self::MAX_VISIBLE
                })
                .map(|entry| entry.key)
                .collect();
        }
        self.candidates.retain(|key| valid.contains(key));
        if context.navigation_active && self.mode == DimensionDisplayMode::Focused {
            // Navigation retains selected, pinned and default-priority callouts.
            // Idle hover previews end as soon as a camera gesture takes ownership.
            self.candidates.retain(|key| {
                entries.iter().any(|entry| {
                    entry.key == *key
                        && (entry.related
                            || entry.pinned
                            || entry.focused
                            || entry.default_priority
                            || context.active == Some(key.item))
                })
            });
        }
        let mut occupied: Vec<_> = scene
            .annotations
            .iter()
            .filter(|annotation| matches!(annotation.kind, SceneAnnotationKind::Constraint(_)))
            .filter(|annotation| {
                scene.show_all_constraint_annotations
                    || annotation.is_visible(&context.selection, context.hovered, &[])
            })
            .cloned()
            .collect();
        // Manual positions reserve space first, independent of source order.
        let mut candidates = self.candidates.clone();
        candidates.sort_by_key(|key| {
            (
                manual_layout.get(*key).is_none(),
                reconsider_hidden
                    && !self
                        .retained
                        .get(key)
                        .is_some_and(|retained| retained.visible),
            )
        });
        let mut visible = BTreeSet::new();
        for key in candidates {
            let Some(base) = raw.get(&key).cloned() else {
                continue;
            };
            let anchors = model_anchors(&base, scene.viewport, document);
            let manual = manual_layout.get(key);
            let retained = self.retained.get(&key).filter(|retained| {
                retained.manual == manual && same_anchors(&retained.model_anchors, &anchors)
            });
            let mut annotation = base.clone();
            let is_explicit = context.active == Some(key.item) || self.focus == Some(key);
            let clear = if let Some(retained) = retained {
                annotation
                    .geometry
                    .clone_from(&retained.annotation.geometry);
                annotations::reproject_retained_layout(
                    std::slice::from_mut(&mut annotation),
                    retained.viewport,
                    scene.viewport,
                );
                manual.is_some()
                    || is_explicit
                    || annotations::focused_dimension_is_clear(
                        &annotation,
                        &occupied,
                        &scene.points,
                        &scene.curves,
                        scene.viewport,
                        if retained.visible { 6.0 } else { 12.0 },
                    )
            } else if let Some(placement) = manual {
                annotation.apply_placement(placement);
                true
            } else {
                annotations::place_focused_dimension(
                    &mut annotation,
                    &occupied,
                    &scene.points,
                    &scene.curves,
                    scene.viewport,
                ) || is_explicit
            };
            // During a navigation gesture only reproject the existing surface.
            let show = if context.navigation_active {
                retained.is_some_and(|retained| retained.visible)
            } else {
                (clear || self.mode == DimensionDisplayMode::All)
                    && (may_restore || retained.is_none_or(|retained| retained.visible))
            };
            annotation.visibility = if show {
                SceneAnnotationVisibility::Always
            } else {
                SceneAnnotationVisibility::Hidden
            };
            self.retained.insert(
                key,
                RetainedDimension {
                    annotation: annotation.clone(),
                    viewport: scene.viewport,
                    model_anchors: anchors,
                    manual,
                    visible: show,
                },
            );
            if show {
                visible.insert(key);
                occupied.push(annotation.clone());
            }
            raw.insert(key, annotation);
        }
        for annotation in &mut scene.annotations {
            if !matches!(annotation.item, SelectionItem::Dimension(_)) {
                continue;
            }
            let key = annotation.layout_key(document_id, None);
            if let Some(resolved) = raw.remove(&key) {
                *annotation = resolved;
            }
            annotation.visibility = if visible.contains(&key) {
                SceneAnnotationVisibility::Always
            } else {
                SceneAnnotationVisibility::Hidden
            };
        }
        for entry in &mut entries {
            entry.visible = visible.contains(&entry.key);
        }
        if sealed {
            scene.refresh_retained_reprojection_seal();
        }
        entries
    }
}

fn related(entry: &SceneDimensionEntry, context: &BTreeSet<SelectionItem>) -> bool {
    context.contains(&entry.key.item) || entry.operands.iter().any(|item| context.contains(item))
}

pub(crate) fn expanded_context(
    document: &geosolve_sketch::SketchDocument,
    items: &[SelectionItem],
) -> BTreeSet<SelectionItem> {
    use geosolve_sketch::CurveDefinition;
    let mut expanded: BTreeSet<_> = items.iter().copied().collect();
    for item in items {
        let SelectionItem::Curve(span) = item else {
            continue;
        };
        let Some(curve) = document.curve(span.curve) else {
            continue;
        };
        let points = match &curve.definition {
            CurveDefinition::Polyline { points, closed, .. } => {
                let index = span.segment as usize;
                let next = index + 1;
                points
                    .get(index)
                    .copied()
                    .into_iter()
                    .chain(
                        points
                            .get(next)
                            .copied()
                            .or_else(|| (*closed && next == points.len()).then(|| points[0])),
                    )
                    .collect()
            }
            definition => crate::curve_definition_points(definition),
        };
        expanded.extend(points.into_iter().map(SelectionItem::Point));
    }
    expanded
}

fn dimension_entries(scene: &EditorScene) -> Vec<SceneDimensionEntry> {
    use geosolve_sketch::{DocumentDimensionDefinition as Dimension, DocumentDimensionMode};
    let document = &scene.accepted_document;
    let annotations: BTreeMap<_, _> = scene
        .annotations
        .iter()
        .map(|annotation| (annotation.item, annotation))
        .collect();
    document
        .dimensions()
        .iter()
        .map(|dimension| {
            let (kind, operands) = match &dimension.definition {
                Dimension::PointDistance { first, second, .. } => (
                    SceneAnnotationKind::PointDistance,
                    vec![SelectionItem::Point(*first), SelectionItem::Point(*second)],
                ),
                Dimension::CurveLength { curve, .. } => (
                    SceneAnnotationKind::CurveLength,
                    vec![SelectionItem::Curve(*curve)],
                ),
                Dimension::Radius { curve, .. } => (
                    SceneAnnotationKind::Radius,
                    vec![SelectionItem::Curve(geosolve_sketch::CurveSpan::line(
                        *curve,
                    ))],
                ),
                Dimension::Diameter { curve, .. } => (
                    SceneAnnotationKind::Diameter,
                    vec![SelectionItem::Curve(geosolve_sketch::CurveSpan::line(
                        *curve,
                    ))],
                ),
                Dimension::OrientedAngle { first, second, .. } => (
                    SceneAnnotationKind::OrientedAngle,
                    vec![SelectionItem::Curve(*first), SelectionItem::Curve(*second)],
                ),
                Dimension::SupportingLineOffset {
                    source,
                    target_segment,
                    ..
                } => (
                    SceneAnnotationKind::SupportingLineOffset,
                    vec![
                        SelectionItem::Curve(*source),
                        SelectionItem::Curve(*target_segment),
                    ],
                ),
                Dimension::ExactTranslatedSegmentOffset {
                    source,
                    target_segment,
                    ..
                } => (
                    SceneAnnotationKind::ExactTranslatedSegmentOffset,
                    vec![
                        SelectionItem::Curve(*source),
                        SelectionItem::Curve(*target_segment),
                    ],
                ),
                Dimension::ProfileOffset { operand, .. } => (
                    SceneAnnotationKind::ProfileOffset,
                    crate::annotations::profile_offset_edge_pairs(operand)
                        .iter()
                        .flat_map(|edge| {
                            [
                                SelectionItem::Curve(edge.source.curve),
                                SelectionItem::Curve(edge.target.curve),
                            ]
                        })
                        .collect(),
                ),
            };
            let item = SelectionItem::Dimension(dimension.id);
            SceneDimensionEntry {
                key: AnnotationLayoutKey {
                    document: document.id(),
                    source: dimension.source_id,
                    item,
                    kind,
                    marker_index: None,
                },
                label: dimension.label.clone(),
                kind,
                operands,
                value_text: annotations
                    .get(&item)
                    .and_then(|annotation| annotation.visible_text.clone()),
                reference: dimension.mode == DocumentDimensionMode::Reference,
                suppressed: dimension.suppressed,
                related: false,
                generated: false,
                default_priority: false,
                visible: false,
                pinned: false,
                focused: false,
            }
        })
        .collect()
}

fn model_anchors(
    annotation: &SceneAnnotation,
    viewport: Viewport,
    document: &geosolve_sketch::SketchDocument,
) -> Vec<[f64; 2]> {
    use crate::SceneAnnotationGeometry;
    let model = |point| viewport.screen_to_model(point);
    let mut anchors = match annotation.geometry {
        SceneAnnotationGeometry::LinearDimension {
            measured_first,
            measured_second,
            ..
        } => vec![model(measured_first), model(measured_second)],
        SceneAnnotationGeometry::RadialDimension { center, edge, .. } => {
            vec![model(center), model(edge)]
        }
        SceneAnnotationGeometry::AngularDimension {
            vertex,
            first_ray,
            second_ray,
            ..
        } => vec![
            model(vertex),
            [
                (first_ray.y - vertex.y).atan2(first_ray.x - vertex.x),
                (second_ray.y - vertex.y).atan2(second_ray.x - vertex.x),
            ],
        ],
        SceneAnnotationGeometry::Label {
            anchor,
            leader_from,
        } => vec![model(leader_from.unwrap_or(anchor))],
        SceneAnnotationGeometry::Glyph { .. } | SceneAnnotationGeometry::RightAngle { .. } => {
            Vec::new()
        }
    };
    for operand in &annotation.operands {
        let points = match operand {
            SelectionItem::Point(point) => vec![*point],
            SelectionItem::Curve(span) => {
                document.curve(span.curve).map_or_else(Vec::new, |curve| {
                    crate::curve_definition_points(&curve.definition)
                })
            }
            _ => Vec::new(),
        };
        anchors.extend(
            points
                .iter()
                .filter_map(|point| document.point(*point).map(|point| point.position)),
        );
    }
    anchors
}

fn same_anchors(first: &[[f64; 2]], second: &[[f64; 2]]) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(first, second)| {
            first.iter().zip(second).all(|(first, second)| {
                (first - second).abs() <= 1e-10 * first.abs().max(second.abs()).max(1.0)
            })
        })
}
