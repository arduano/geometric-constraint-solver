// SPDX-License-Identifier: GPL-3.0-or-later

//! Geometry-derived, presentation-neutral constraint annotations.

use std::collections::BTreeMap;

use geosolve_sketch::{
    ContactId, CurveDefinition, CurveId, CurveSpan, DocumentCenterRef,
    DocumentConstraintDefinition as Constraint, DocumentConstraintId,
    DocumentDimensionDefinition as Dimension, DocumentDimensionMode, DocumentId, DocumentSourceId,
    ScalarUnit, SketchAcceptedDocumentState, SketchDatum, SketchDocument,
};

use crate::coordinator::display_dimension_target;
use crate::{SceneCurve, ScenePoint, ScreenPoint, SelectionItem, Viewport};

/// Semantic symbol requested for one constraint annotation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SceneConstraintGlyph {
    Fixed,
    Coincident,
    Horizontal,
    Vertical,
    PointOnCurve,
    Parallel,
    Perpendicular,
    Concentric,
    Collinear,
    EqualLength,
    EqualRadius,
    Midpoint,
    Symmetry,
    Contact,
    Tangency,
    Direction,
    Normal,
    EqualCurvature,
    Continuity,
    Fillet,
}

/// One persistent constraint entry published by the headless scene owner.
///
/// Entries exist independently of drawable annotation geometry, so a host can
/// render a complete constraint tree without re-reading document definitions or
/// reconstructing operands and presentation families. Their order is the
/// document's ordinary persistent constraint order.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneConstraintEntry {
    pub id: DocumentConstraintId,
    pub source: DocumentSourceId,
    pub label: String,
    pub glyph: SceneConstraintGlyph,
    pub operands: Vec<SelectionItem>,
    pub suppressed: bool,
}

/// Publishes the complete constraint-entry surface for any validated retained
/// document, including a current design that has no accepted geometry.
///
/// Unlike canvas annotations, entries require no solved positions. This keeps
/// rejected design intent visible without letting rejected coordinates become
/// presentation authority.
#[must_use]
pub fn constraint_entries(document: &SketchDocument) -> Vec<SceneConstraintEntry> {
    document
        .constraints()
        .iter()
        .map(|constraint| {
            let (glyph, operands) = constraint_entry_presentation(document, &constraint.definition);
            SceneConstraintEntry {
                id: constraint.id,
                source: constraint.source_id,
                label: constraint.label.clone(),
                glyph,
                operands,
                suppressed: constraint.suppressed,
            }
        })
        .collect()
}

/// Default presentation density for one accepted annotation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneAnnotationVisibility {
    /// Visible without a related hover or selection.
    Always,
    /// Visible only through direct context, selection, or diagnostics.
    Contextual,
}

/// One glyph location. A displaced marker retains its semantic leader origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneGlyphMarker {
    pub anchor: ScreenPoint,
    pub leader_from: Option<ScreenPoint>,
    /// Clockwise screen-space rotation of the compact mark around `anchor`.
    ///
    /// The value follows the SVG/canvas convention because screen Y increases
    /// downward. It is geometry-derived and never presentation adapter state.
    pub rotation_radians: f64,
}

impl SceneGlyphMarker {
    /// Exact circular pointer bound rendered around every compact glyph.
    pub const BOUND_RADIUS_PIXELS: f64 = 10.0;

    #[must_use]
    pub const fn bounds(self) -> SceneAnnotationGlyphBounds {
        SceneAnnotationGlyphBounds {
            center: self.anchor,
            radius: Self::BOUND_RADIUS_PIXELS,
        }
    }
}

/// Exact circular bound shared by compact-glyph painting and pointer ownership.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneAnnotationGlyphBounds {
    pub center: ScreenPoint,
    pub radius: f64,
}

impl SceneAnnotationGlyphBounds {
    #[must_use]
    pub fn contains(self, point: ScreenPoint) -> bool {
        point.is_finite() && self.center.distance(point) <= self.radius
    }
}

/// Exact triangular arrowhead published by the headless annotation scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneAnnotationArrowhead {
    pub tip: ScreenPoint,
    pub base_first: ScreenPoint,
    pub base_second: ScreenPoint,
}

impl SceneAnnotationArrowhead {
    #[must_use]
    pub fn proximity(self, point: ScreenPoint) -> f64 {
        point_triangle_distance(point, self.tip, self.base_first, self.base_second)
    }
}

/// Exact screen-space label rectangle shared by painting and picking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneAnnotationLabelBounds {
    pub min: ScreenPoint,
    pub max: ScreenPoint,
}

impl SceneAnnotationLabelBounds {
    #[must_use]
    pub fn contains(self, point: ScreenPoint, tolerance: f64) -> bool {
        point.is_finite()
            && tolerance.is_finite()
            && tolerance >= 0.0
            && point.x >= self.min.x - tolerance
            && point.x <= self.max.x + tolerance
            && point.y >= self.min.y - tolerance
            && point.y <= self.max.y + tolerance
    }

    #[must_use]
    pub fn distance(self, point: ScreenPoint) -> f64 {
        let x = point.x.clamp(self.min.x, self.max.x);
        let y = point.y.clamp(self.min.y, self.max.y);
        point.distance(ScreenPoint { x, y })
    }
}

/// Screen-space geometry needed to render and hit-test an annotation.
#[derive(Clone, Debug, PartialEq)]
pub enum SceneAnnotationGeometry {
    Glyph {
        markers: Vec<SceneGlyphMarker>,
    },
    /// A line-line perpendicular relation drawn as the two free sides of a
    /// square corner. The accepted lines themselves provide the other sides.
    RightAngle {
        vertex: ScreenPoint,
        first_arm: ScreenPoint,
        corner: ScreenPoint,
        second_arm: ScreenPoint,
    },
    LinearDimension {
        /// Accepted measurement attachments.
        measured_first: ScreenPoint,
        measured_second: ScreenPoint,
        /// Offset dimension baseline endpoints.
        first: ScreenPoint,
        second: ScreenPoint,
        label_anchor: ScreenPoint,
    },
    RadialDimension {
        center: ScreenPoint,
        edge: ScreenPoint,
        label_anchor: ScreenPoint,
        diameter: bool,
        /// Whether a diameter may truthfully cross the opposite side.
        full_circle: bool,
    },
    AngularDimension {
        vertex: ScreenPoint,
        first_ray: ScreenPoint,
        second_ray: ScreenPoint,
        radius: f64,
        clockwise: bool,
        label_anchor: ScreenPoint,
    },
    Label {
        anchor: ScreenPoint,
        leader_from: Option<ScreenPoint>,
    },
}

impl SceneAnnotationGeometry {
    /// Publishes every arrowhead exactly once for shared rendering and picking.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive geometry dispatch keeps paired-arrow fallback and paint/pick identity together"
    )]
    pub fn arrowheads(&self) -> Vec<SceneAnnotationArrowhead> {
        match self {
            Self::LinearDimension { first, second, .. } => {
                let Some(direction) = unit(second.x - first.x, second.y - first.y) else {
                    return Vec::new();
                };
                let outside = first.distance(*second) < ANNOTATION_MIN_INWARD_ARROW_SPAN_PIXELS;
                let first_base_direction = if outside {
                    [-direction[0], -direction[1]]
                } else {
                    direction
                };
                let second_base_direction = if outside {
                    direction
                } else {
                    [-direction[0], -direction[1]]
                };
                [
                    annotation_arrowhead(*first, first_base_direction),
                    annotation_arrowhead(*second, second_base_direction),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Self::RadialDimension {
                center,
                edge,
                diameter,
                full_circle,
                ..
            } => {
                let Some(direction) = unit(edge.x - center.x, edge.y - center.y) else {
                    return Vec::new();
                };
                if *diameter && *full_circle {
                    let opposite = ScreenPoint {
                        x: center.x.mul_add(2.0, -edge.x),
                        y: center.y.mul_add(2.0, -edge.y),
                    };
                    let outside =
                        opposite.distance(*edge) < ANNOTATION_MIN_INWARD_ARROW_SPAN_PIXELS;
                    let opposite_base_direction = if outside {
                        [-direction[0], -direction[1]]
                    } else {
                        direction
                    };
                    let edge_base_direction = if outside {
                        direction
                    } else {
                        [-direction[0], -direction[1]]
                    };
                    [
                        annotation_arrowhead(opposite, opposite_base_direction),
                        annotation_arrowhead(*edge, edge_base_direction),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                } else {
                    annotation_arrowhead(*edge, [-direction[0], -direction[1]])
                        .into_iter()
                        .collect()
                }
            }
            Self::AngularDimension {
                vertex,
                first_ray,
                second_ray,
                radius,
                clockwise,
                ..
            } => {
                let Some(first_direction) = unit(first_ray.x - vertex.x, first_ray.y - vertex.y)
                else {
                    return Vec::new();
                };
                let Some(second_direction) = unit(second_ray.x - vertex.x, second_ray.y - vertex.y)
                else {
                    return Vec::new();
                };
                let first_tip = offset(
                    *vertex,
                    first_direction[0] * *radius,
                    first_direction[1] * *radius,
                );
                let second_tip = offset(
                    *vertex,
                    second_direction[0] * *radius,
                    second_direction[1] * *radius,
                );
                let first_interior = if *clockwise {
                    [-first_direction[1], first_direction[0]]
                } else {
                    [first_direction[1], -first_direction[0]]
                };
                let second_interior = if *clockwise {
                    [second_direction[1], -second_direction[0]]
                } else {
                    [-second_direction[1], second_direction[0]]
                };
                let sweep = cross(first_direction, second_direction)
                    .abs()
                    .atan2(dot(first_direction, second_direction).clamp(-1.0, 1.0));
                let outside = radius * sweep < ANNOTATION_MIN_INWARD_ARROW_SPAN_PIXELS;
                let first_base_direction = if outside {
                    [-first_interior[0], -first_interior[1]]
                } else {
                    first_interior
                };
                let second_base_direction = if outside {
                    [-second_interior[0], -second_interior[1]]
                } else {
                    second_interior
                };
                [
                    annotation_arrowhead(first_tip, first_base_direction),
                    annotation_arrowhead(second_tip, second_base_direction),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Self::Label {
                anchor,
                leader_from: Some(origin),
            } => annotation_arrowhead(*origin, [anchor.x - origin.x, anchor.y - origin.y])
                .into_iter()
                .collect(),
            Self::Glyph { .. }
            | Self::RightAngle { .. }
            | Self::Label {
                leader_from: None, ..
            } => Vec::new(),
        }
    }
}

/// Typed semantic category for one accepted annotation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SceneAnnotationKind {
    Constraint(SceneConstraintGlyph),
    PointDistance,
    CurveLength,
    Radius,
    Diameter,
    OrientedAngle,
    SupportingLineOffset,
    ExactTranslatedSegmentOffset,
}

/// Stable identity of one movable annotation occurrence within a document.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AnnotationLayoutKey {
    pub document: DocumentId,
    pub source: DocumentSourceId,
    pub item: SelectionItem,
    pub kind: SceneAnnotationKind,
    pub marker_index: Option<usize>,
}

/// Semantic manual placement retained independently from sketch history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnnotationPlacement {
    Linear {
        perpendicular_pixels: f64,
    },
    Radial {
        direction_radians: f64,
        clearance_pixels: f64,
    },
    Angular {
        radius_pixels: f64,
    },
    Free {
        offset_pixels: [f64; 2],
    },
}

impl AnnotationPlacement {
    const MAX_ABS_PIXELS: f64 = 10_000.0;

    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Linear {
                perpendicular_pixels,
            } => {
                perpendicular_pixels.is_finite()
                    && perpendicular_pixels.abs() <= Self::MAX_ABS_PIXELS
            }
            Self::Radial {
                direction_radians,
                clearance_pixels,
            } => {
                direction_radians.is_finite()
                    && clearance_pixels.is_finite()
                    && clearance_pixels.abs() <= Self::MAX_ABS_PIXELS
            }
            Self::Angular { radius_pixels } => {
                radius_pixels.is_finite() && (12.0..=Self::MAX_ABS_PIXELS).contains(&radius_pixels)
            }
            Self::Free { offset_pixels } => offset_pixels
                .iter()
                .all(|value| value.is_finite() && value.abs() <= Self::MAX_ABS_PIXELS),
        }
    }
}

/// One exported cache row. Hosts persist these rows outside canonical sketch data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationLayoutEntry {
    pub key: AnnotationLayoutKey,
    pub placement: AnnotationPlacement,
}

impl AnnotationLayoutEntry {
    fn is_valid(self) -> bool {
        if !self.placement.is_valid() {
            return false;
        }
        match (self.key.item, self.key.kind, self.placement) {
            (
                SelectionItem::Constraint(_),
                SceneAnnotationKind::Constraint(_),
                AnnotationPlacement::Free { .. },
            ) => self.key.marker_index.is_some(),
            (
                SelectionItem::Dimension(_),
                SceneAnnotationKind::PointDistance
                | SceneAnnotationKind::SupportingLineOffset
                | SceneAnnotationKind::ExactTranslatedSegmentOffset,
                AnnotationPlacement::Linear { .. },
            )
            | (
                SelectionItem::Dimension(_),
                SceneAnnotationKind::Radius | SceneAnnotationKind::Diameter,
                AnnotationPlacement::Radial { .. },
            )
            | (
                SelectionItem::Dimension(_),
                SceneAnnotationKind::OrientedAngle,
                AnnotationPlacement::Angular { .. },
            )
            | (
                SelectionItem::Dimension(_),
                SceneAnnotationKind::CurveLength,
                AnnotationPlacement::Linear { .. } | AnnotationPlacement::Free { .. },
            ) => self.key.marker_index.is_none(),
            (
                SelectionItem::Point(_)
                | SelectionItem::Curve(_)
                | SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_),
                _,
                _,
            ) => false,
        }
    }
}

/// Presentation-only manual placement retained by [`crate::ConstraintEditor`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotationLayoutState {
    entries: BTreeMap<AnnotationLayoutKey, AnnotationPlacement>,
}

impl AnnotationLayoutState {
    pub const VERSION: u32 = 1;
    pub const MAX_ENTRIES: usize = 100_000;

    #[must_use]
    pub fn entries(&self) -> Vec<AnnotationLayoutEntry> {
        self.entries
            .iter()
            .map(|(key, placement)| AnnotationLayoutEntry {
                key: *key,
                placement: *placement,
            })
            .collect()
    }

    /// Reconstructs a bounded cache, dropping malformed rows independently.
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = AnnotationLayoutEntry>) -> Self {
        let mut state = Self::default();
        for entry in entries.into_iter().take(Self::MAX_ENTRIES) {
            if entry.is_valid() {
                state.entries.insert(entry.key, entry.placement);
            }
        }
        state
    }

    pub(crate) fn get(&self, key: AnnotationLayoutKey) -> Option<AnnotationPlacement> {
        self.entries.get(&key).copied()
    }

    pub(crate) fn insert(&mut self, key: AnnotationLayoutKey, placement: AnnotationPlacement) {
        if placement.is_valid() {
            self.entries.insert(key, placement);
        }
    }

    pub(crate) fn remove_item(&mut self, item: SelectionItem) -> bool {
        let before = self.entries.len();
        self.entries.retain(|key, _| key.item != item);
        self.entries.len() != before
    }

    pub(crate) fn clear(&mut self) -> bool {
        let changed = !self.entries.is_empty();
        self.entries.clear();
        changed
    }
}

/// One persistent constraint or dimension projected into an accepted editor scene.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneAnnotation {
    pub item: SelectionItem,
    /// Exact persistent source that owns this accepted annotation.
    ///
    /// Presentation adapters use it to keep accepted annotation metadata tied
    /// to the same retained source when current design entries diverge or are
    /// absent. Constraint IDs are never reused; this remains a defensive
    /// provenance boundary, while dimensions resolve through the accepted
    /// document directly.
    pub source: DocumentSourceId,
    pub kind: SceneAnnotationKind,
    pub operands: Vec<SelectionItem>,
    pub geometry: SceneAnnotationGeometry,
    pub visibility: SceneAnnotationVisibility,
    /// Whether the persistent source is currently suppressed.
    pub suppressed: bool,
    /// Compact CAD value painted on the canvas. Constraint glyphs carry none.
    pub visible_text: Option<String>,
    /// Full human-readable source/value used by title and accessibility surfaces.
    pub accessible_label: String,
    /// Exact label rectangle used by both rendering and pointer ownership.
    pub label_bounds: Option<SceneAnnotationLabelBounds>,
    /// Reference dimensions use parenthesized text and non-colour styling.
    pub reference: bool,
}

impl SceneAnnotation {
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive semantic placement dispatch keeps every annotation geometry form explicit"
    )]
    fn apply_placement(&mut self, placement: AnnotationPlacement) {
        match (&mut self.geometry, placement) {
            (
                SceneAnnotationGeometry::LinearDimension {
                    measured_first,
                    measured_second,
                    first,
                    second,
                    label_anchor,
                },
                AnnotationPlacement::Linear {
                    perpendicular_pixels,
                },
            ) => {
                if let Some(direction) = unit(
                    measured_second.x - measured_first.x,
                    measured_second.y - measured_first.y,
                ) {
                    let normal = [-direction[1], direction[0]];
                    *first = offset(
                        *measured_first,
                        normal[0] * perpendicular_pixels,
                        normal[1] * perpendicular_pixels,
                    );
                    *second = offset(
                        *measured_second,
                        normal[0] * perpendicular_pixels,
                        normal[1] * perpendicular_pixels,
                    );
                    *label_anchor = midpoint(*first, *second);
                }
            }
            (
                SceneAnnotationGeometry::RadialDimension {
                    center,
                    edge,
                    label_anchor,
                    full_circle,
                    ..
                },
                AnnotationPlacement::Radial {
                    direction_radians,
                    clearance_pixels,
                },
            ) => {
                let length = center.distance(*edge);
                let direction = if *full_circle {
                    [direction_radians.cos(), direction_radians.sin()]
                } else {
                    // A bounded arc has no truthful boundary point at an arbitrary
                    // polar direction. Keep its canonical on-arc attachment and
                    // move the label only along that radius.
                    unit(edge.x - center.x, edge.y - center.y).unwrap_or([1.0, 0.0])
                };
                if *full_circle {
                    *edge = offset(*center, direction[0] * length, direction[1] * length);
                }
                *label_anchor = offset(
                    *center,
                    direction[0] * (length + clearance_pixels),
                    direction[1] * (length + clearance_pixels),
                );
            }
            (
                SceneAnnotationGeometry::AngularDimension {
                    vertex,
                    first_ray,
                    second_ray,
                    radius,
                    label_anchor,
                    ..
                },
                AnnotationPlacement::Angular {
                    radius_pixels: next_radius,
                },
            ) => {
                let direction = unit(label_anchor.x - vertex.x, label_anchor.y - vertex.y)
                    .unwrap_or([1.0, 0.0]);
                let first_direction =
                    unit(first_ray.x - vertex.x, first_ray.y - vertex.y).unwrap_or([1.0, 0.0]);
                let second_direction =
                    unit(second_ray.x - vertex.x, second_ray.y - vertex.y).unwrap_or([0.0, 1.0]);
                *radius = next_radius;
                *first_ray = offset(
                    *vertex,
                    first_direction[0] * (next_radius + 12.0),
                    first_direction[1] * (next_radius + 12.0),
                );
                *second_ray = offset(
                    *vertex,
                    second_direction[0] * (next_radius + 12.0),
                    second_direction[1] * (next_radius + 12.0),
                );
                *label_anchor = offset(
                    *vertex,
                    direction[0] * (next_radius + 18.0),
                    direction[1] * (next_radius + 18.0),
                );
            }
            (
                SceneAnnotationGeometry::Glyph { markers },
                AnnotationPlacement::Free { offset_pixels },
            ) => {
                for marker in markers {
                    marker.anchor = offset(marker.anchor, offset_pixels[0], offset_pixels[1]);
                }
            }
            (
                SceneAnnotationGeometry::Label { anchor, .. },
                AnnotationPlacement::Free { offset_pixels },
            ) => {
                *anchor = offset(*anchor, offset_pixels[0], offset_pixels[1]);
            }
            (
                SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
                | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
                | SceneAnnotationGeometry::AngularDimension { label_anchor, .. },
                AnnotationPlacement::Free { offset_pixels },
            ) => {
                *label_anchor = offset(*label_anchor, offset_pixels[0], offset_pixels[1]);
            }
            (
                SceneAnnotationGeometry::RightAngle { .. }
                | SceneAnnotationGeometry::Glyph { .. }
                | SceneAnnotationGeometry::Label { .. }
                | SceneAnnotationGeometry::LinearDimension { .. }
                | SceneAnnotationGeometry::RadialDimension { .. }
                | SceneAnnotationGeometry::AngularDimension { .. },
                _,
            ) => {}
        }
        self.refresh_label_bounds();
    }

    #[must_use]
    pub const fn is_movable(&self) -> bool {
        !matches!(self.geometry, SceneAnnotationGeometry::RightAngle { .. })
    }

    pub(crate) fn movable_handle_hit(
        &self,
        position: ScreenPoint,
        marker_index: Option<usize>,
    ) -> bool {
        if !self.is_movable() || !position.is_finite() {
            return false;
        }
        match &self.geometry {
            SceneAnnotationGeometry::Glyph { markers } => marker_index
                .and_then(|index| markers.get(index))
                .is_some_and(|marker| marker.bounds().contains(position)),
            SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
            | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
            | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => {
                self.label_hit(position) || label_anchor.distance(position) <= 2.0
            }
            SceneAnnotationGeometry::Label { anchor, .. } => {
                self.label_bounds
                    .is_some_and(|bounds| bounds.contains(position, 2.0))
                    || anchor.distance(position) <= 2.0
            }
            SceneAnnotationGeometry::RightAngle { .. } => false,
        }
    }

    pub(crate) fn automatic_placement(
        &self,
        marker_index: Option<usize>,
    ) -> Option<AnnotationPlacement> {
        match &self.geometry {
            SceneAnnotationGeometry::Glyph { markers } if marker_index? < markers.len() => {
                Some(AnnotationPlacement::Free {
                    offset_pixels: [0.0, 0.0],
                })
            }
            SceneAnnotationGeometry::Label { .. } => Some(AnnotationPlacement::Free {
                offset_pixels: [0.0, 0.0],
            }),
            SceneAnnotationGeometry::LinearDimension {
                measured_first,
                measured_second,
                first,
                ..
            } => {
                let direction = unit(
                    measured_second.x - measured_first.x,
                    measured_second.y - measured_first.y,
                )?;
                Some(AnnotationPlacement::Linear {
                    perpendicular_pixels: (first.x - measured_first.x)
                        .mul_add(-direction[1], (first.y - measured_first.y) * direction[0]),
                })
            }
            SceneAnnotationGeometry::RadialDimension {
                center,
                edge,
                label_anchor,
                ..
            } => {
                let delta = [label_anchor.x - center.x, label_anchor.y - center.y];
                let distance = delta[0].hypot(delta[1]);
                let radius = center.distance(*edge);
                Some(AnnotationPlacement::Radial {
                    direction_radians: delta[1].atan2(delta[0]),
                    clearance_pixels: distance - radius,
                })
            }
            SceneAnnotationGeometry::AngularDimension { radius, .. } => {
                Some(AnnotationPlacement::Angular {
                    radius_pixels: *radius,
                })
            }
            SceneAnnotationGeometry::Glyph { .. } | SceneAnnotationGeometry::RightAngle { .. } => {
                None
            }
        }
    }

    pub(crate) const fn layout_key(
        &self,
        document: DocumentId,
        marker_index: Option<usize>,
    ) -> AnnotationLayoutKey {
        AnnotationLayoutKey {
            document,
            source: self.source,
            item: self.item,
            kind: self.kind,
            marker_index,
        }
    }

    /// Reports whether direct interaction context makes this annotation visible.
    #[must_use]
    pub fn is_visible(
        &self,
        selection: &[SelectionItem],
        hovered: Option<SelectionItem>,
        problem_items: &[SelectionItem],
    ) -> bool {
        self.visibility == SceneAnnotationVisibility::Always
            || selection.contains(&self.item)
            || problem_items.contains(&self.item)
            || hovered == Some(self.item)
            || hovered.is_some_and(|item| self.operands.contains(&item))
            || selection
                .iter()
                .any(|selected| self.operands.contains(selected))
    }

    /// Reports whether the screen position hits this annotation.
    #[must_use]
    pub fn hit_test(&self, position: ScreenPoint, tolerance_pixels: f64) -> bool {
        self.proximity_hit(position, tolerance_pixels).is_some()
    }

    /// Returns the exact presentation occurrence under the pointer and its distance.
    ///
    /// Painted glyph leaders select the owning occurrence, while only the exact
    /// glyph bound may begin movement.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive paint-primitive dispatch keeps shared picking geometry auditable"
    )]
    pub(crate) fn proximity_hit(
        &self,
        position: ScreenPoint,
        tolerance_pixels: f64,
    ) -> Option<(Option<usize>, f64)> {
        if !position.is_finite() || !tolerance_pixels.is_finite() || tolerance_pixels < 0.0 {
            return None;
        }
        let label_distance = self
            .label_bounds
            .map_or(f64::INFINITY, |bounds| bounds.distance(position));
        let (occurrence, mut distance) =
            match &self.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers
                    .iter()
                    .enumerate()
                    .map(|(index, marker)| {
                        let distance = marker.leader_from.map_or_else(
                            || position.distance(marker.anchor),
                            |origin| {
                                position.distance(marker.anchor).min(point_segment_distance(
                                    position,
                                    origin,
                                    marker.anchor,
                                ))
                            },
                        );
                        (Some(index), distance)
                    })
                    .min_by(|first, second| {
                        first
                            .1
                            .total_cmp(&second.1)
                            .then_with(|| first.0.cmp(&second.0))
                    })?,
                SceneAnnotationGeometry::RightAngle {
                    first_arm,
                    corner,
                    second_arm,
                    ..
                } => (
                    None,
                    point_segment_distance(position, *first_arm, *corner)
                        .min(point_segment_distance(position, *corner, *second_arm)),
                ),
                SceneAnnotationGeometry::LinearDimension {
                    measured_first,
                    measured_second,
                    label_anchor,
                    first,
                    second,
                } => (
                    None,
                    label_distance
                        .min(position.distance(*label_anchor))
                        .min(point_segment_distance(position, *first, *second))
                        .min(point_segment_distance(position, *measured_first, *first))
                        .min(point_segment_distance(position, *measured_second, *second)),
                ),
                SceneAnnotationGeometry::RadialDimension {
                    center,
                    edge,
                    label_anchor,
                    diameter,
                    full_circle,
                } => (
                    None,
                    label_distance
                        .min(position.distance(*label_anchor))
                        .min(point_segment_distance(position, *edge, *label_anchor))
                        .min(if *diameter && *full_circle {
                            let opposite = ScreenPoint {
                                x: center.x.mul_add(2.0, -edge.x),
                                y: center.y.mul_add(2.0, -edge.y),
                            };
                            point_segment_distance(position, opposite, *edge)
                        } else {
                            point_segment_distance(position, *center, *edge)
                        }),
                ),
                SceneAnnotationGeometry::AngularDimension {
                    vertex,
                    first_ray,
                    second_ray,
                    radius,
                    clockwise,
                    label_anchor,
                } => (
                    None,
                    label_distance
                        .min(position.distance(*label_anchor))
                        .min(point_arc_distance(
                            position,
                            *vertex,
                            *first_ray,
                            *second_ray,
                            *radius,
                            *clockwise,
                        ))
                        .min(point_segment_distance(position, *vertex, *first_ray))
                        .min(point_segment_distance(position, *vertex, *second_ray)),
                ),
                SceneAnnotationGeometry::Label {
                    anchor,
                    leader_from,
                } => (
                    None,
                    label_distance.min(position.distance(*anchor)).min(
                        leader_from.map_or(f64::INFINITY, |origin| {
                            point_segment_distance(position, origin, *anchor)
                        }),
                    ),
                ),
            };
        for arrowhead in self.geometry.arrowheads() {
            distance = distance.min(arrowhead.proximity(position));
        }
        (distance <= tolerance_pixels).then_some((occurrence, distance))
    }

    /// Reports whether a press owns the movable label rather than another painted part.
    #[must_use]
    pub(crate) fn label_hit(&self, position: ScreenPoint) -> bool {
        self.label_bounds
            .is_some_and(|bounds| bounds.contains(position, 2.0))
    }

    /// Reports whether the pointer remains inside a bounded corridor from
    /// previously hovered related geometry to this annotation.
    #[must_use]
    pub fn context_hit_test(
        &self,
        position: ScreenPoint,
        context_origin: ScreenPoint,
        tolerance_pixels: f64,
    ) -> bool {
        tolerance_pixels.is_finite()
            && tolerance_pixels >= 0.0
            && self
                .context_distance(position, context_origin)
                .is_some_and(|distance| distance <= tolerance_pixels)
    }

    /// Returns the nearest distance to a contextual corridor from related geometry.
    #[must_use]
    pub fn context_distance(
        &self,
        position: ScreenPoint,
        context_origin: ScreenPoint,
    ) -> Option<f64> {
        if !position.is_finite() || !context_origin.is_finite() {
            return None;
        }
        Some(match &self.geometry {
            SceneAnnotationGeometry::Glyph { markers } => markers
                .iter()
                .map(|marker| point_segment_distance(position, context_origin, marker.anchor))
                .min_by(f64::total_cmp)?,
            SceneAnnotationGeometry::RightAngle { corner, .. } => {
                point_segment_distance(position, context_origin, *corner)
            }
            SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
            | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
            | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => {
                point_segment_distance(position, context_origin, *label_anchor)
            }
            SceneAnnotationGeometry::Label { anchor, .. } => {
                point_segment_distance(position, context_origin, *anchor)
            }
        })
    }

    pub(crate) fn context_anchors(&self) -> Vec<ScreenPoint> {
        match &self.geometry {
            SceneAnnotationGeometry::Glyph { markers } => {
                markers.iter().map(|marker| marker.anchor).collect()
            }
            SceneAnnotationGeometry::RightAngle { corner, .. } => vec![*corner],
            SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
            | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
            | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => {
                vec![*label_anchor]
            }
            SceneAnnotationGeometry::Label { anchor, .. } => vec![*anchor],
        }
    }

    pub(crate) fn label_anchor(&self) -> Option<ScreenPoint> {
        match &self.geometry {
            SceneAnnotationGeometry::LinearDimension { label_anchor, .. }
            | SceneAnnotationGeometry::RadialDimension { label_anchor, .. }
            | SceneAnnotationGeometry::AngularDimension { label_anchor, .. } => Some(*label_anchor),
            SceneAnnotationGeometry::Label { anchor, .. } => Some(*anchor),
            SceneAnnotationGeometry::Glyph { .. } | SceneAnnotationGeometry::RightAngle { .. } => {
                None
            }
        }
    }

    pub(crate) fn refresh_label_bounds(&mut self) {
        self.label_bounds = self
            .visible_text
            .as_deref()
            .zip(self.label_anchor())
            .map(|(text, anchor)| annotation_label_bounds(anchor, text));
    }

    fn update_dimension_value(&mut self, source_label: &str, value: Option<f64>) {
        self.visible_text =
            value.and_then(|value| compact_dimension_text(value, self.kind, self.reference));
        self.accessible_label =
            accessible_dimension_label(source_label, self.kind, self.reference, value);
        self.refresh_label_bounds();
    }
}

fn accessible_dimension_label(
    source_label: &str,
    kind: SceneAnnotationKind,
    reference: bool,
    value: Option<f64>,
) -> String {
    let family = match kind {
        SceneAnnotationKind::PointDistance => "point-distance dimension",
        SceneAnnotationKind::CurveLength => "curve-length dimension",
        SceneAnnotationKind::Radius => "radius dimension",
        SceneAnnotationKind::Diameter => "diameter dimension",
        SceneAnnotationKind::OrientedAngle => "oriented-angle dimension",
        SceneAnnotationKind::SupportingLineOffset => "supporting-line offset dimension",
        SceneAnnotationKind::ExactTranslatedSegmentOffset => {
            "exact translated-segment offset dimension"
        }
        SceneAnnotationKind::Constraint(_) => "constraint",
    };
    let mode = if reference { "Reference" } else { "Driving" };
    let value = value
        .filter(|value| value.is_finite())
        .and_then(|value| {
            if kind == SceneAnnotationKind::OrientedAngle {
                display_dimension_target(value, ScalarUnit::Angle)
                    .map(|display| format!("{} degrees", compact_number(display.value)))
            } else {
                Some(format!("{} model units", compact_number(value)))
            }
        })
        .unwrap_or_else(|| "value unavailable".into());
    format!("{source_label}; {mode} {family}; {value}")
}

fn annotation_label_bounds(anchor: ScreenPoint, text: &str) -> SceneAnnotationLabelBounds {
    let character_count = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    let width = (f64::from(character_count) * 7.4 + 12.0).max(24.0);
    let half_width = width * 0.5;
    SceneAnnotationLabelBounds {
        min: ScreenPoint {
            x: anchor.x - half_width,
            y: anchor.y - 10.0,
        },
        max: ScreenPoint {
            x: anchor.x + half_width,
            y: anchor.y + 10.0,
        },
    }
}

fn dimension_default_text(
    document: &SketchDocument,
    dimension: &geosolve_sketch::DocumentDimension,
) -> Option<String> {
    let value = dimension_stored_value(document, dimension)?;
    compact_dimension_text(
        value,
        match dimension.definition {
            Dimension::OrientedAngle { .. } => SceneAnnotationKind::OrientedAngle,
            Dimension::Radius { .. } => SceneAnnotationKind::Radius,
            Dimension::Diameter { .. } => SceneAnnotationKind::Diameter,
            Dimension::PointDistance { .. } => SceneAnnotationKind::PointDistance,
            Dimension::CurveLength { .. } => SceneAnnotationKind::CurveLength,
            Dimension::SupportingLineOffset { .. } => SceneAnnotationKind::SupportingLineOffset,
            Dimension::ExactTranslatedSegmentOffset { .. } => {
                SceneAnnotationKind::ExactTranslatedSegmentOffset
            }
        },
        dimension.mode == DocumentDimensionMode::Reference,
    )
}

fn dimension_stored_value(
    document: &SketchDocument,
    dimension: &geosolve_sketch::DocumentDimension,
) -> Option<f64> {
    let target = match &dimension.definition {
        Dimension::PointDistance { target, .. }
        | Dimension::CurveLength { target, .. }
        | Dimension::Radius { target, .. }
        | Dimension::Diameter { target, .. }
        | Dimension::OrientedAngle { target, .. }
        | Dimension::SupportingLineOffset { target, .. }
        | Dimension::ExactTranslatedSegmentOffset { target, .. } => *target,
    };
    document
        .scalar(target)
        .map(|scalar| scalar.value)
        .filter(|value| value.is_finite())
}

pub(crate) fn update_dimension_values(
    annotations: &mut [SceneAnnotation],
    accepted: &SketchAcceptedDocumentState,
) {
    for annotation in annotations.iter_mut() {
        let SelectionItem::Dimension(id) = annotation.item else {
            continue;
        };
        let Some(dimension) = accepted
            .document()
            .dimension(id)
            .filter(|dimension| dimension.source_id == annotation.source)
        else {
            annotation.update_dimension_value("Accepted dimension", None);
            continue;
        };
        let value = match dimension.mode {
            DocumentDimensionMode::Driving => {
                let target = match &dimension.definition {
                    Dimension::PointDistance { target, .. }
                    | Dimension::CurveLength { target, .. }
                    | Dimension::Radius { target, .. }
                    | Dimension::Diameter { target, .. }
                    | Dimension::OrientedAngle { target, .. }
                    | Dimension::SupportingLineOffset { target, .. }
                    | Dimension::ExactTranslatedSegmentOffset { target, .. } => *target,
                };
                accepted
                    .document()
                    .scalar(target)
                    .map(|scalar| scalar.value)
            }
            DocumentDimensionMode::Reference => accepted.reference_value(id),
        }
        .filter(|value| value.is_finite());
        annotation.update_dimension_value(&dimension.label, value);
    }
}

/// Formats one finite dimension as compact conventional CAD notation.
#[must_use]
pub fn compact_dimension_text(
    value: f64,
    kind: SceneAnnotationKind,
    reference: bool,
) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let converted = if kind == SceneAnnotationKind::OrientedAngle {
        display_dimension_target(value, ScalarUnit::Angle)?.value
    } else {
        value
    };
    let number = compact_number(converted);
    let value = match kind {
        SceneAnnotationKind::Radius => format!("R{number}"),
        SceneAnnotationKind::Diameter => format!("⌀{number}"),
        SceneAnnotationKind::OrientedAngle => format!("{number}°"),
        SceneAnnotationKind::Constraint(_)
        | SceneAnnotationKind::PointDistance
        | SceneAnnotationKind::CurveLength
        | SceneAnnotationKind::SupportingLineOffset
        | SceneAnnotationKind::ExactTranslatedSegmentOffset => number,
    };
    Some(if reference {
        format!("({value})")
    } else {
        value
    })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "finite IEEE-754 base-10 exponents are bounded to approximately plus or minus 308"
)]
fn compact_number(value: f64) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return "0".into();
    }
    let mut exponent = magnitude.log10().floor() as i32;
    if !(1.0e-3..1.0e5).contains(&magnitude) {
        let mut scaled = value / 10.0_f64.powi(exponent);
        scaled = (scaled * 1_000.0).round() / 1_000.0;
        if scaled.abs() >= 10.0 {
            scaled /= 10.0;
            exponent += 1;
        }
        return format!("{}e{exponent}", trim_decimal(format!("{scaled:.3}")));
    }
    if exponent > 3 {
        let step = 10.0_f64.powi(exponent - 3);
        let rounded = (value / step).round() * step;
        if rounded.abs() >= 1.0e5 {
            return format!("{}e5", trim_decimal(format!("{:.3}", rounded / 1.0e5)));
        }
        return format!("{rounded:.0}");
    }
    let decimals = usize::try_from((3 - exponent).clamp(0, 12)).unwrap_or(0);
    trim_decimal(format!("{value:.decimals$}"))
}

fn trim_decimal(mut value: String) -> String {
    if value.contains('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
    }
    if value == "-0" {
        value = "0".into();
    }
    value
}

pub(crate) fn build_annotations(
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
) -> Vec<SceneAnnotation> {
    let mut annotations = Vec::new();
    for constraint in document.constraints() {
        let (glyph, operands, anchors) =
            constraint_presentation(document, points, curves, &constraint.definition);
        if anchors.is_empty() {
            continue;
        }
        let rotations = constraint_marker_rotations(
            document,
            curves,
            &constraint.definition,
            glyph,
            &operands,
            &anchors,
        );
        let geometry = match &constraint.definition {
            Constraint::Perpendicular { first, second } if !constraint.suppressed => {
                right_angle_geometry(curves, viewport, *first, *second)
                    .unwrap_or_else(|| glyph_geometry(anchors, rotations))
            }
            _ => glyph_geometry(anchors, rotations),
        };
        annotations.push(SceneAnnotation {
            item: SelectionItem::Constraint(constraint.id),
            source: constraint.source_id,
            kind: SceneAnnotationKind::Constraint(glyph),
            operands,
            geometry,
            visibility: SceneAnnotationVisibility::Contextual,
            suppressed: constraint.suppressed,
            visible_text: None,
            accessible_label: accessible_constraint_label(&constraint.label, glyph),
            label_bounds: None,
            reference: false,
        });
    }
    for dimension in document.dimensions() {
        if let Some((kind, operands, geometry)) =
            dimension_presentation(document, points, curves, viewport, &dimension.definition)
        {
            let visible_text = dimension_default_text(document, dimension);
            let reference = dimension.mode == DocumentDimensionMode::Reference;
            let mut annotation = SceneAnnotation {
                item: SelectionItem::Dimension(dimension.id),
                source: dimension.source_id,
                kind,
                operands,
                geometry,
                visibility: if !dimension.suppressed
                    && (dimension.mode == DocumentDimensionMode::Driving
                        || kind == SceneAnnotationKind::OrientedAngle)
                {
                    SceneAnnotationVisibility::Always
                } else {
                    SceneAnnotationVisibility::Contextual
                },
                suppressed: dimension.suppressed,
                accessible_label: accessible_dimension_label(
                    &dimension.label,
                    kind,
                    reference,
                    dimension_stored_value(document, dimension),
                ),
                visible_text,
                label_bounds: None,
                reference,
            };
            annotation.refresh_label_bounds();
            annotations.push(annotation);
        }
    }
    resolve_automatic_layout(
        document.id(),
        &mut annotations,
        points,
        curves,
        viewport,
        None,
    );
    annotations
}

fn accessible_constraint_label(source_label: &str, glyph: SceneConstraintGlyph) -> String {
    let family = match glyph {
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
    };
    format!("{source_label}; {family} constraint")
}

pub(crate) fn apply_layout(
    document: DocumentId,
    annotations: &mut [SceneAnnotation],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
    layout: &AnnotationLayoutState,
) {
    for annotation in &mut *annotations {
        if let SceneAnnotationGeometry::Glyph { markers } = &mut annotation.geometry {
            for (index, marker) in markers.iter_mut().enumerate() {
                let key = AnnotationLayoutKey {
                    document,
                    source: annotation.source,
                    item: annotation.item,
                    kind: annotation.kind,
                    marker_index: Some(index),
                };
                if let Some(AnnotationPlacement::Free { offset_pixels }) = layout.get(key) {
                    marker.anchor = offset(marker.anchor, offset_pixels[0], offset_pixels[1]);
                }
            }
        } else if let Some(placement) = layout.get(annotation.layout_key(document, None)) {
            annotation.apply_placement(placement);
        }
    }
    resolve_automatic_layout(
        document,
        annotations,
        points,
        curves,
        viewport,
        Some(layout),
    );
}

pub(crate) fn build_constraint_entries(document: &SketchDocument) -> Vec<SceneConstraintEntry> {
    constraint_entries(document)
}

#[allow(clippy::too_many_lines)]
fn constraint_entry_presentation(
    document: &SketchDocument,
    definition: &Constraint,
) -> (SceneConstraintGlyph, Vec<SelectionItem>) {
    let contact_span = |contact| document.contact(contact).map(|slot| slot.curve);
    let curve_span = |curve| {
        document
            .curve_spans(curve)
            .ok()
            .and_then(|spans| spans.into_iter().next())
    };
    let contact_operands = |contacts: [ContactId; 2]| {
        unique_items(
            contacts
                .into_iter()
                .filter_map(contact_span)
                .map(SelectionItem::Curve)
                .collect(),
        )
    };
    let curve_operands = |curves: [CurveId; 2]| {
        curves
            .into_iter()
            .filter_map(curve_span)
            .map(SelectionItem::Curve)
            .collect()
    };
    match definition {
        Constraint::FixedPoint { point, .. } | Constraint::FixedCoordinate { point, .. } => (
            SceneConstraintGlyph::Fixed,
            vec![SelectionItem::Point(*point)],
        ),
        Constraint::CoincidentWithOrigin { point } => (
            SceneConstraintGlyph::Coincident,
            vec![
                SelectionItem::Point(*point),
                SelectionItem::Datum(SketchDatum::Origin),
            ],
        ),
        Constraint::PointOnDatumAxis { point, axis } => (
            SceneConstraintGlyph::Coincident,
            vec![
                SelectionItem::Point(*point),
                SelectionItem::Datum(match axis {
                    geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                    geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
                }),
            ],
        ),
        Constraint::Coincident { first, second } => (
            SceneConstraintGlyph::Coincident,
            vec![SelectionItem::Point(*first), SelectionItem::Point(*second)],
        ),
        Constraint::ExternalPointCoincident { point, .. } => (
            SceneConstraintGlyph::Coincident,
            vec![SelectionItem::Point(*point)],
        ),
        Constraint::Horizontal { line } => (
            SceneConstraintGlyph::Horizontal,
            vec![SelectionItem::Curve(*line)],
        ),
        Constraint::Vertical { line } => (
            SceneConstraintGlyph::Vertical,
            vec![SelectionItem::Curve(*line)],
        ),
        Constraint::HorizontalPoints { first, second } => (
            SceneConstraintGlyph::Horizontal,
            vec![SelectionItem::Point(*first), SelectionItem::Point(*second)],
        ),
        Constraint::VerticalPoints { first, second } => (
            SceneConstraintGlyph::Vertical,
            vec![SelectionItem::Point(*first), SelectionItem::Point(*second)],
        ),
        Constraint::HorizontalPointToMidpoint { point, line } => (
            SceneConstraintGlyph::Horizontal,
            vec![SelectionItem::Point(*point), SelectionItem::Curve(*line)],
        ),
        Constraint::VerticalPointToMidpoint { point, line } => (
            SceneConstraintGlyph::Vertical,
            vec![SelectionItem::Point(*point), SelectionItem::Curve(*line)],
        ),
        Constraint::PointOnCurve { point, contact } => (
            SceneConstraintGlyph::PointOnCurve,
            std::iter::once(SelectionItem::Point(*point))
                .chain(contact_span(*contact).map(SelectionItem::Curve))
                .collect(),
        ),
        Constraint::Parallel { first, second } => (
            SceneConstraintGlyph::Parallel,
            vec![SelectionItem::Curve(*first), SelectionItem::Curve(*second)],
        ),
        Constraint::Perpendicular { first, second } => (
            SceneConstraintGlyph::Perpendicular,
            vec![SelectionItem::Curve(*first), SelectionItem::Curve(*second)],
        ),
        Constraint::ExternalLineCollinear { line, .. } => (
            SceneConstraintGlyph::Collinear,
            vec![SelectionItem::Curve(line.span)],
        ),
        Constraint::CollinearWithDatumAxis { line, axis } => (
            SceneConstraintGlyph::Collinear,
            vec![
                SelectionItem::Curve(line.span),
                SelectionItem::Datum(match axis {
                    geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                    geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
                }),
            ],
        ),
        Constraint::Concentric { first, second } => (
            SceneConstraintGlyph::Concentric,
            curve_operands([first.curve, second.curve]),
        ),
        Constraint::Collinear { first, second } => (
            SceneConstraintGlyph::Collinear,
            vec![
                SelectionItem::Curve(first.span),
                SelectionItem::Curve(second.span),
            ],
        ),
        Constraint::EqualLength { first, second } => (
            SceneConstraintGlyph::EqualLength,
            vec![SelectionItem::Curve(*first), SelectionItem::Curve(*second)],
        ),
        Constraint::EqualRadius { first, second } => (
            SceneConstraintGlyph::EqualRadius,
            curve_operands([*first, *second]),
        ),
        Constraint::Midpoint { point, line } => (
            SceneConstraintGlyph::Midpoint,
            vec![SelectionItem::Point(*point), SelectionItem::Curve(*line)],
        ),
        Constraint::SymmetricAboutLine {
            first,
            second,
            line,
        } => (
            SceneConstraintGlyph::Symmetry,
            vec![
                SelectionItem::Point(*first),
                SelectionItem::Point(*second),
                SelectionItem::Curve(*line),
            ],
        ),
        Constraint::SymmetricAboutDatumAxis {
            first,
            second,
            axis,
        } => (
            SceneConstraintGlyph::Symmetry,
            vec![
                SelectionItem::Point(*first),
                SelectionItem::Point(*second),
                SelectionItem::Datum(match axis {
                    geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                    geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
                }),
            ],
        ),
        Constraint::LineCircleTangency {
            line_contact,
            circle_contact,
            ..
        }
        | Constraint::CircleArcTangency {
            circle_contact: line_contact,
            arc_contact: circle_contact,
            ..
        }
        | Constraint::CurveCurveTangency {
            first_contact: line_contact,
            second_contact: circle_contact,
        } => (
            SceneConstraintGlyph::Tangency,
            contact_operands([*line_contact, *circle_contact]),
        ),
        Constraint::CircleCircleTangency { first, second, .. } => (
            SceneConstraintGlyph::Tangency,
            curve_operands([*first, *second]),
        ),
        Constraint::LineCurveTangency {
            line,
            curve_contact,
            ..
        } => (
            SceneConstraintGlyph::Tangency,
            std::iter::once(SelectionItem::Curve(*line))
                .chain(contact_span(*curve_contact).map(SelectionItem::Curve))
                .collect(),
        ),
        Constraint::CurveCurveContact {
            first_contact,
            second_contact,
        } => (
            SceneConstraintGlyph::Contact,
            contact_operands([*first_contact, *second_contact]),
        ),
        Constraint::CurveDirection {
            line,
            curve_contact,
            relation,
        } => (
            match relation {
                geosolve_sketch::DocumentCurveDirectionRelation::Tangent { .. } => {
                    SceneConstraintGlyph::Direction
                }
                geosolve_sketch::DocumentCurveDirectionRelation::Normal { .. } => {
                    SceneConstraintGlyph::Normal
                }
            },
            std::iter::once(SelectionItem::Curve(*line))
                .chain(contact_span(*curve_contact).map(SelectionItem::Curve))
                .collect(),
        ),
        Constraint::EqualCurvature {
            first_contact,
            second_contact,
            ..
        } => (
            SceneConstraintGlyph::EqualCurvature,
            contact_operands([*first_contact, *second_contact]),
        ),
        Constraint::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        } => (
            SceneConstraintGlyph::Continuity,
            contact_operands([*first_contact, *second_contact]),
        ),
        Constraint::LineLineFillet {
            arc,
            first_contact,
            second_contact,
            ..
        }
        | Constraint::CurveCurveFillet {
            arc,
            first_contact,
            second_contact,
            ..
        } => {
            let mut operands = contact_operands([*first_contact, *second_contact]);
            operands.extend(curve_span(*arc).map(SelectionItem::Curve));
            (SceneConstraintGlyph::Fillet, unique_items(operands))
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the one-shot paired marker vectors are consumed together by scene composition"
)]
fn glyph_geometry(anchors: Vec<ScreenPoint>, rotations: Vec<f64>) -> SceneAnnotationGeometry {
    SceneAnnotationGeometry::Glyph {
        markers: anchors
            .into_iter()
            .enumerate()
            .map(|(index, origin)| SceneGlyphMarker {
                anchor: offset(origin, 24.0, -24.0),
                leader_from: Some(origin),
                rotation_radians: rotations.get(index).copied().unwrap_or(0.0),
            })
            .collect(),
    }
}

fn constraint_marker_rotations(
    document: &SketchDocument,
    curves: &[SceneCurve],
    definition: &Constraint,
    glyph: SceneConstraintGlyph,
    operands: &[SelectionItem],
    anchors: &[ScreenPoint],
) -> Vec<f64> {
    let fixed_rotation = match definition {
        Constraint::SymmetricAboutLine { line, .. } => {
            curve_direction_at(curves, *line, anchors[0])
                .map(|direction| direction[1].atan2(direction[0]) - std::f64::consts::FRAC_PI_2)
        }
        Constraint::SymmetricAboutDatumAxis { axis, .. } => Some(match axis {
            geosolve_sketch::DocumentCoordinateAxis::X => -std::f64::consts::FRAC_PI_2,
            geosolve_sketch::DocumentCoordinateAxis::Y => 0.0,
        }),
        Constraint::CurveDirection { curve_contact, .. } => document
            .contact(*curve_contact)
            .and_then(|contact| curve_direction_at(curves, contact.curve, anchors[0]))
            .map(|direction| direction[1].atan2(direction[0])),
        _ => None,
    };
    if let Some(rotation) = fixed_rotation.filter(|rotation| rotation.is_finite()) {
        return vec![rotation; anchors.len()];
    }

    let rotates_with_curve = matches!(
        glyph,
        SceneConstraintGlyph::PointOnCurve
            | SceneConstraintGlyph::Parallel
            | SceneConstraintGlyph::Perpendicular
            | SceneConstraintGlyph::Collinear
            | SceneConstraintGlyph::EqualLength
            | SceneConstraintGlyph::Contact
            | SceneConstraintGlyph::Tangency
            | SceneConstraintGlyph::Direction
            | SceneConstraintGlyph::Normal
            | SceneConstraintGlyph::EqualCurvature
            | SceneConstraintGlyph::Continuity
            | SceneConstraintGlyph::Fillet
    );
    if !rotates_with_curve {
        return vec![0.0; anchors.len()];
    }

    let spans = operands
        .iter()
        .filter_map(|operand| match operand {
            SelectionItem::Curve(span) => Some(*span),
            SelectionItem::Point(_)
            | SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        })
        .collect::<Vec<_>>();
    anchors
        .iter()
        .map(|anchor| {
            spans
                .iter()
                .filter_map(|span| {
                    curve_direction_at(curves, *span, *anchor).map(|direction| {
                        let distance = curves
                            .iter()
                            .filter(|curve| curve.span == *span)
                            .flat_map(|curve| curve.screen_polyline.windows(2))
                            .map(|segment| point_segment_distance(*anchor, segment[0], segment[1]))
                            .min_by(f64::total_cmp)
                            .unwrap_or(f64::INFINITY);
                        (distance, *span, direction)
                    })
                })
                .min_by(|first, second| {
                    first
                        .0
                        .total_cmp(&second.0)
                        .then_with(|| first.1.cmp(&second.1))
                })
                .map_or(0.0, |(_, _, direction)| direction[1].atan2(direction[0]))
        })
        .collect()
}

fn curve_direction_at(
    curves: &[SceneCurve],
    span: CurveSpan,
    anchor: ScreenPoint,
) -> Option<[f64; 2]> {
    curves
        .iter()
        .enumerate()
        .filter(|(_, curve)| curve.span == span)
        .flat_map(|(curve_index, curve)| {
            curve.screen_polyline.windows(2).enumerate().filter_map(
                move |(segment_index, segment)| {
                    let direction = unit(segment[1].x - segment[0].x, segment[1].y - segment[0].y)?;
                    Some((
                        point_segment_distance(anchor, segment[0], segment[1]),
                        curve_index,
                        segment_index,
                        direction,
                    ))
                },
            )
        })
        .min_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.cmp(&second.1))
                .then_with(|| first.2.cmp(&second.2))
        })
        .map(|(_, _, _, direction)| direction)
}

#[allow(clippy::too_many_lines)]
fn constraint_presentation(
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    definition: &Constraint,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    match definition {
        Constraint::FixedPoint { point, .. } | Constraint::FixedCoordinate { point, .. } => {
            point_relation(SceneConstraintGlyph::Fixed, points, *point)
        }
        Constraint::CoincidentWithOrigin { point } => {
            let (glyph, mut operands, anchors) =
                point_relation(SceneConstraintGlyph::Coincident, points, *point);
            operands.push(SelectionItem::Datum(SketchDatum::Origin));
            (glyph, operands, anchors)
        }
        Constraint::PointOnDatumAxis { point, axis } => {
            let (glyph, mut operands, anchors) =
                point_relation(SceneConstraintGlyph::Coincident, points, *point);
            operands.push(SelectionItem::Datum(match axis {
                geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
            }));
            (glyph, operands, anchors)
        }
        Constraint::Coincident { first, second } => {
            let operands = vec![SelectionItem::Point(*first), SelectionItem::Point(*second)];
            let anchors = paired_point_anchor(points, *first, *second)
                .into_iter()
                .collect();
            (SceneConstraintGlyph::Coincident, operands, anchors)
        }
        Constraint::ExternalPointCoincident { point, .. } => {
            point_relation(SceneConstraintGlyph::Coincident, points, *point)
        }
        Constraint::Horizontal { line } => {
            curve_relation(SceneConstraintGlyph::Horizontal, curves, [*line])
        }
        Constraint::Vertical { line } => {
            curve_relation(SceneConstraintGlyph::Vertical, curves, [*line])
        }
        Constraint::HorizontalPoints { first, second } => {
            point_pair_relation(SceneConstraintGlyph::Horizontal, points, *first, *second)
        }
        Constraint::VerticalPoints { first, second } => {
            point_pair_relation(SceneConstraintGlyph::Vertical, points, *first, *second)
        }
        Constraint::HorizontalPointToMidpoint { point, line } => point_span_relation(
            SceneConstraintGlyph::Horizontal,
            points,
            curves,
            *point,
            *line,
        ),
        Constraint::VerticalPointToMidpoint { point, line } => point_span_relation(
            SceneConstraintGlyph::Vertical,
            points,
            curves,
            *point,
            *line,
        ),
        Constraint::PointOnCurve { point, contact } => point_contact_relation(
            SceneConstraintGlyph::PointOnCurve,
            document,
            points,
            curves,
            *point,
            *contact,
        ),
        Constraint::Parallel { first, second } => {
            curve_relation(SceneConstraintGlyph::Parallel, curves, [*first, *second])
        }
        Constraint::Perpendicular { first, second } => curve_relation(
            SceneConstraintGlyph::Perpendicular,
            curves,
            [*first, *second],
        ),
        Constraint::ExternalLineCollinear { line, .. } => {
            curve_relation(SceneConstraintGlyph::Collinear, curves, [line.span])
        }
        Constraint::CollinearWithDatumAxis { line, axis } => {
            let (glyph, mut operands, anchors) =
                curve_relation(SceneConstraintGlyph::Collinear, curves, [line.span]);
            operands.push(SelectionItem::Datum(match axis {
                geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
            }));
            (glyph, operands, anchors)
        }
        Constraint::Concentric { first, second } => {
            concentric_relation(document, points, curves, *first, *second)
        }
        Constraint::Collinear { first, second } => curve_relation(
            SceneConstraintGlyph::Collinear,
            curves,
            [first.span, second.span],
        ),
        Constraint::EqualLength { first, second } => {
            curve_relation(SceneConstraintGlyph::EqualLength, curves, [*first, *second])
        }
        Constraint::EqualRadius { first, second } => {
            curve_ids_relation(SceneConstraintGlyph::EqualRadius, curves, [*first, *second])
        }
        Constraint::Midpoint { point, line } => {
            let mut operands = vec![SelectionItem::Point(*point), SelectionItem::Curve(*line)];
            operands.sort();
            let anchors = point_anchor(points, *point).into_iter().collect();
            (SceneConstraintGlyph::Midpoint, operands, anchors)
        }
        Constraint::SymmetricAboutLine {
            first,
            second,
            line,
        } => {
            let operands = vec![
                SelectionItem::Point(*first),
                SelectionItem::Point(*second),
                SelectionItem::Curve(*line),
            ];
            let anchors = paired_point_anchor(points, *first, *second)
                .into_iter()
                .collect();
            (SceneConstraintGlyph::Symmetry, operands, anchors)
        }
        Constraint::SymmetricAboutDatumAxis {
            first,
            second,
            axis,
        } => {
            let operands = vec![
                SelectionItem::Point(*first),
                SelectionItem::Point(*second),
                SelectionItem::Datum(match axis {
                    geosolve_sketch::DocumentCoordinateAxis::X => SketchDatum::XAxis,
                    geosolve_sketch::DocumentCoordinateAxis::Y => SketchDatum::YAxis,
                }),
            ];
            let anchors = paired_point_anchor(points, *first, *second)
                .into_iter()
                .collect();
            (SceneConstraintGlyph::Symmetry, operands, anchors)
        }
        Constraint::LineCircleTangency {
            line_contact,
            circle_contact,
            ..
        }
        | Constraint::CircleArcTangency {
            circle_contact: line_contact,
            arc_contact: circle_contact,
            ..
        }
        | Constraint::CurveCurveTangency {
            first_contact: line_contact,
            second_contact: circle_contact,
        } => contact_pair_relation(
            SceneConstraintGlyph::Tangency,
            document,
            curves,
            *line_contact,
            *circle_contact,
        ),
        Constraint::CircleCircleTangency { first, second, .. } => {
            curve_ids_relation(SceneConstraintGlyph::Tangency, curves, [*first, *second])
        }
        Constraint::LineCurveTangency {
            line,
            curve_contact,
            ..
        } => {
            let Some((curve_operand, contact_anchor)) =
                contact_operand_anchor(document, curves, *curve_contact)
            else {
                return (
                    SceneConstraintGlyph::Tangency,
                    vec![SelectionItem::Curve(*line)],
                    curve_anchor(curves, *line).into_iter().collect(),
                );
            };
            (
                SceneConstraintGlyph::Tangency,
                vec![SelectionItem::Curve(*line), curve_operand],
                vec![contact_anchor],
            )
        }
        Constraint::CurveCurveContact {
            first_contact,
            second_contact,
        } => contact_pair_relation(
            SceneConstraintGlyph::Contact,
            document,
            curves,
            *first_contact,
            *second_contact,
        ),
        Constraint::CurveDirection {
            line,
            curve_contact,
            relation,
        } => {
            let glyph = match relation {
                geosolve_sketch::DocumentCurveDirectionRelation::Tangent { .. } => {
                    SceneConstraintGlyph::Direction
                }
                geosolve_sketch::DocumentCurveDirectionRelation::Normal { .. } => {
                    SceneConstraintGlyph::Normal
                }
            };
            let Some((curve_operand, anchor)) =
                contact_operand_anchor(document, curves, *curve_contact)
            else {
                return curve_relation(glyph, curves, [*line]);
            };
            (
                glyph,
                vec![SelectionItem::Curve(*line), curve_operand],
                vec![anchor],
            )
        }
        Constraint::EqualCurvature {
            first_contact,
            second_contact,
            ..
        } => contact_pair_relation(
            SceneConstraintGlyph::EqualCurvature,
            document,
            curves,
            *first_contact,
            *second_contact,
        ),
        Constraint::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        } => contact_pair_relation(
            SceneConstraintGlyph::Continuity,
            document,
            curves,
            *first_contact,
            *second_contact,
        ),
        Constraint::LineLineFillet {
            arc,
            first_contact,
            second_contact,
            ..
        }
        | Constraint::CurveCurveFillet {
            arc,
            first_contact,
            second_contact,
            ..
        } => {
            let (mut operands, mut anchors) =
                contact_pair_operands(document, curves, *first_contact, *second_contact);
            if let Some(span) = first_curve_span(curves, *arc) {
                operands.push(SelectionItem::Curve(span));
                if let Some(anchor) = curve_anchor(curves, span) {
                    anchors.push(anchor);
                }
            }
            (
                SceneConstraintGlyph::Fillet,
                unique_items(operands),
                anchors,
            )
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive dimension-family dispatch keeps all seven public geometry contracts together"
)]
fn dimension_presentation(
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
    definition: &Dimension,
) -> Option<(
    SceneAnnotationKind,
    Vec<SelectionItem>,
    SceneAnnotationGeometry,
)> {
    match definition {
        Dimension::PointDistance { first, second, .. } => {
            let first_anchor = point_anchor(points, *first)?;
            let second_anchor = point_anchor(points, *second)?;
            Some((
                SceneAnnotationKind::PointDistance,
                vec![SelectionItem::Point(*first), SelectionItem::Point(*second)],
                linear_dimension(first_anchor, second_anchor, 28.0)?,
            ))
        }
        Dimension::CurveLength { curve, .. } => {
            let span = *curve;
            let definition = &document.curve(span.curve)?.definition;
            let geometry = if matches!(
                definition,
                CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
            ) {
                let [first, second] = curve_endpoints(curves, span)?;
                linear_dimension(first, second, 28.0)?
            } else {
                let anchor = curve_anchor(curves, span)?;
                let curve = curves.iter().find(|candidate| candidate.span == span)?;
                let middle = curve.screen_polyline.len().saturating_sub(1) / 2;
                let before = curve
                    .screen_polyline
                    .get(middle.saturating_sub(1))
                    .copied()
                    .unwrap_or(anchor);
                let after = curve
                    .screen_polyline
                    .get(middle.saturating_add(1))
                    .copied()
                    .unwrap_or(anchor);
                let tangent = unit(after.x - before.x, after.y - before.y).unwrap_or([1.0, 0.0]);
                SceneAnnotationGeometry::Label {
                    anchor: offset(anchor, -tangent[1] * 30.0, tangent[0] * 30.0),
                    leader_from: Some(anchor),
                }
            };
            Some((
                SceneAnnotationKind::CurveLength,
                vec![SelectionItem::Curve(span)],
                geometry,
            ))
        }
        Dimension::Radius { curve, .. } | Dimension::Diameter { curve, .. } => {
            let (center, edge, full_circle) =
                radial_geometry(document, points, curves, viewport, *curve)?;
            let diameter = matches!(definition, Dimension::Diameter { .. });
            let direction = unit(edge.x - center.x, edge.y - center.y)?;
            let radius = center.distance(edge);
            Some((
                if diameter {
                    SceneAnnotationKind::Diameter
                } else {
                    SceneAnnotationKind::Radius
                },
                curve_operands(curves, *curve),
                SceneAnnotationGeometry::RadialDimension {
                    center,
                    edge,
                    label_anchor: offset(
                        center,
                        direction[0] * (radius + 34.0),
                        direction[1] * (radius + 34.0),
                    ),
                    diameter,
                    full_circle,
                },
            ))
        }
        Dimension::OrientedAngle { first, second, .. } => {
            let geometry = angle_geometry(curves, viewport, *first, *second)?;
            Some((
                SceneAnnotationKind::OrientedAngle,
                vec![SelectionItem::Curve(*first), SelectionItem::Curve(*second)],
                geometry,
            ))
        }
        Dimension::SupportingLineOffset {
            source,
            target_segment,
            ..
        } => {
            let [source_start, source_end] = curve_endpoints(curves, *source)?;
            let [target_start, target_end] = curve_endpoints(curves, *target_segment)?;
            let direction = unit(source_end.x - source_start.x, source_end.y - source_start.y)?;
            let target = midpoint(target_start, target_end);
            let axial = dot(
                [target.x - source_start.x, target.y - source_start.y],
                direction,
            );
            let foot = offset(source_start, direction[0] * axial, direction[1] * axial);
            Some((
                SceneAnnotationKind::SupportingLineOffset,
                vec![
                    SelectionItem::Curve(*source),
                    SelectionItem::Curve(*target_segment),
                ],
                linear_dimension(foot, target, 24.0)?,
            ))
        }
        Dimension::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            ..
        } => {
            let [source_start, source_end] = curve_endpoints(curves, *source)?;
            let [target_start, target_end] = curve_endpoints(curves, *target_segment)?;
            let direct = source_start.distance(target_start) + source_end.distance(target_end);
            let reversed = source_start.distance(target_end) + source_end.distance(target_start);
            let (target_first, target_second) = if direct <= reversed {
                (target_start, target_end)
            } else {
                (target_end, target_start)
            };
            let first = midpoint(source_start, source_end);
            let second = midpoint(target_first, target_second);
            Some((
                SceneAnnotationKind::ExactTranslatedSegmentOffset,
                vec![
                    SelectionItem::Curve(*source),
                    SelectionItem::Curve(*target_segment),
                ],
                linear_dimension(first, second, 24.0)?,
            ))
        }
    }
}

fn angle_geometry(
    curves: &[SceneCurve],
    viewport: Viewport,
    first: CurveSpan,
    second: CurveSpan,
) -> Option<SceneAnnotationGeometry> {
    let [first_start, first_end] = curve_endpoints(curves, first)?;
    let [second_start, second_end] = curve_endpoints(curves, second)?;
    let first_direction = unit(first_end.x - first_start.x, first_end.y - first_start.y)?;
    let mut second_direction = unit(second_end.x - second_start.x, second_end.y - second_start.y)?;
    if dot(first_direction, second_direction) < 0.0 {
        second_direction = [-second_direction[0], -second_direction[1]];
    }
    let vertex = line_intersection(first_start, first_direction, second_start, second_direction)
        .filter(|point| {
            point.is_finite()
                && point.x >= -250.0
                && point.y >= -250.0
                && point.x <= viewport.screen_size[0] + 250.0
                && point.y <= viewport.screen_size[1] + 250.0
        })
        .unwrap_or_else(|| {
            midpoint(
                curve_anchor(curves, first).unwrap(),
                curve_anchor(curves, second).unwrap(),
            )
        });
    let angle = cross(first_direction, second_direction)
        .abs()
        .atan2(dot(first_direction, second_direction).clamp(-1.0, 1.0));
    let radius = (18.0 / (angle * 0.5).sin().abs().max(0.25)).clamp(34.0, 72.0);
    let first_ray = offset(
        vertex,
        first_direction[0] * (radius + 12.0),
        first_direction[1] * (radius + 12.0),
    );
    let second_ray = offset(
        vertex,
        second_direction[0] * (radius + 12.0),
        second_direction[1] * (radius + 12.0),
    );
    let bisector = unit(
        first_direction[0] + second_direction[0],
        first_direction[1] + second_direction[1],
    )
    .unwrap_or([-first_direction[1], first_direction[0]]);
    Some(SceneAnnotationGeometry::AngularDimension {
        vertex,
        first_ray,
        second_ray,
        radius,
        clockwise: cross(first_direction, second_direction) > 0.0,
        label_anchor: offset(
            vertex,
            bisector[0] * (radius + 18.0),
            bisector[1] * (radius + 18.0),
        ),
    })
}

fn point_relation(
    glyph: SceneConstraintGlyph,
    points: &[ScenePoint],
    point: geosolve_sketch::DesignPointId,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    (
        glyph,
        vec![SelectionItem::Point(point)],
        point_anchor(points, point).into_iter().collect(),
    )
}

fn point_pair_relation(
    glyph: SceneConstraintGlyph,
    points: &[ScenePoint],
    first: geosolve_sketch::DesignPointId,
    second: geosolve_sketch::DesignPointId,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    (
        glyph,
        vec![SelectionItem::Point(first), SelectionItem::Point(second)],
        paired_point_anchor(points, first, second)
            .into_iter()
            .collect(),
    )
}

fn point_span_relation(
    glyph: SceneConstraintGlyph,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    point: geosolve_sketch::DesignPointId,
    span: CurveSpan,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    let mut anchors = point_anchor(points, point).into_iter().collect::<Vec<_>>();
    anchors.extend(line_relation_anchor(curves, span));
    (
        glyph,
        vec![SelectionItem::Point(point), SelectionItem::Curve(span)],
        anchors,
    )
}

fn curve_relation<const N: usize>(
    glyph: SceneConstraintGlyph,
    curves: &[SceneCurve],
    spans: [CurveSpan; N],
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    (
        glyph,
        spans.iter().copied().map(SelectionItem::Curve).collect(),
        spans
            .iter()
            .filter_map(|span| line_relation_anchor(curves, *span))
            .collect(),
    )
}

fn curve_ids_relation<const N: usize>(
    glyph: SceneConstraintGlyph,
    curves: &[SceneCurve],
    ids: [CurveId; N],
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    let spans: Vec<_> = ids
        .iter()
        .filter_map(|id| first_curve_span(curves, *id))
        .collect();
    (
        glyph,
        spans.iter().copied().map(SelectionItem::Curve).collect(),
        spans
            .iter()
            .filter_map(|span| curve_anchor(curves, *span))
            .collect(),
    )
}

fn concentric_relation(
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    first: DocumentCenterRef,
    second: DocumentCenterRef,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    let spans = [first.curve, second.curve]
        .into_iter()
        .filter_map(|id| first_curve_span(curves, id))
        .collect::<Vec<_>>();
    let anchors = document
        .resolve_center_ref(first)
        .and_then(|first| {
            document
                .resolve_center_ref(second)
                .map(|second| (first, second))
        })
        .ok()
        .and_then(|(first, second)| paired_point_anchor(points, first, second))
        .into_iter()
        .collect();
    (
        SceneConstraintGlyph::Concentric,
        spans.into_iter().map(SelectionItem::Curve).collect(),
        anchors,
    )
}

fn point_contact_relation(
    glyph: SceneConstraintGlyph,
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    point: geosolve_sketch::DesignPointId,
    contact: ContactId,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    let mut operands = vec![SelectionItem::Point(point)];
    let mut anchors = point_anchor(points, point).into_iter().collect::<Vec<_>>();
    if let Some((operand, anchor)) = contact_operand_anchor(document, curves, contact) {
        operands.push(operand);
        anchors = vec![anchor];
    }
    (glyph, operands, anchors)
}

fn contact_pair_relation(
    glyph: SceneConstraintGlyph,
    document: &SketchDocument,
    curves: &[SceneCurve],
    first: ContactId,
    second: ContactId,
) -> (SceneConstraintGlyph, Vec<SelectionItem>, Vec<ScreenPoint>) {
    let (operands, anchors) = contact_pair_operands(document, curves, first, second);
    (glyph, operands, anchors)
}

fn contact_pair_operands(
    document: &SketchDocument,
    curves: &[SceneCurve],
    first: ContactId,
    second: ContactId,
) -> (Vec<SelectionItem>, Vec<ScreenPoint>) {
    let resolved: Vec<_> = [first, second]
        .into_iter()
        .filter_map(|contact| contact_operand_anchor(document, curves, contact))
        .collect();
    let operands = unique_items(resolved.iter().map(|(item, _)| *item).collect());
    let anchors = if let [(.., first), (.., second)] = resolved.as_slice() {
        vec![midpoint(*first, *second)]
    } else {
        resolved.iter().map(|(_, anchor)| *anchor).collect()
    };
    (operands, anchors)
}

fn contact_operand_anchor(
    document: &SketchDocument,
    curves: &[SceneCurve],
    contact: ContactId,
) -> Option<(SelectionItem, ScreenPoint)> {
    let slot = document.contact(contact)?;
    let parameter = document.scalar(slot.parameter)?.value;
    let anchor = curve_parameter_anchor(curves, slot.curve, parameter)
        .or_else(|| curve_anchor(curves, slot.curve))?;
    Some((SelectionItem::Curve(slot.curve), anchor))
}

fn curve_parameter_anchor(
    curves: &[SceneCurve],
    span: CurveSpan,
    parameter: f64,
) -> Option<ScreenPoint> {
    parameter.is_finite().then_some(())?;
    curves
        .iter()
        .filter(|curve| curve.span == span)
        .flat_map(|curve| {
            let origin = match curve.origin {
                crate::SceneCurveOrigin::Native => None,
                crate::SceneCurveOrigin::FilletDiscarded { fragment, .. } => Some(fragment),
            };
            curve
                .screen_parameters
                .iter()
                .copied()
                .zip(curve.screen_polyline.iter().copied())
                .enumerate()
                .filter(|(_, (sample, position))| sample.is_finite() && position.is_finite())
                .map(move |(sample_index, (sample, position))| {
                    ((sample - parameter).abs(), origin, sample_index, position)
                })
        })
        .min_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.cmp(&second.1))
                .then_with(|| first.2.cmp(&second.2))
                .then_with(|| first.3.x.total_cmp(&second.3.x))
                .then_with(|| first.3.y.total_cmp(&second.3.y))
        })
        .map(|(_, _, _, position)| position)
}

fn point_anchor(points: &[ScenePoint], id: geosolve_sketch::DesignPointId) -> Option<ScreenPoint> {
    points
        .iter()
        .find(|point| point.id == id)
        .map(|point| point.screen_position)
}

fn paired_point_anchor(
    points: &[ScenePoint],
    first: geosolve_sketch::DesignPointId,
    second: geosolve_sketch::DesignPointId,
) -> Option<ScreenPoint> {
    Some(midpoint(
        point_anchor(points, first)?,
        point_anchor(points, second)?,
    ))
}

fn curve_anchor(curves: &[SceneCurve], span: CurveSpan) -> Option<ScreenPoint> {
    let curve = curves.iter().find(|curve| curve.span == span)?;
    curve
        .screen_polyline
        .get(curve.screen_polyline.len().saturating_sub(1) / 2)
        .copied()
}

fn line_relation_anchor(curves: &[SceneCurve], span: CurveSpan) -> Option<ScreenPoint> {
    let [start, end] = curve_endpoints(curves, span)?;
    Some(midpoint(start, end))
}

const RIGHT_ANGLE_SIZE_PIXELS: f64 = 12.0;
const RIGHT_ANGLE_VIEW_MARGIN_PIXELS: f64 = 24.0;

fn right_angle_geometry(
    curves: &[SceneCurve],
    viewport: Viewport,
    first: CurveSpan,
    second: CurveSpan,
) -> Option<SceneAnnotationGeometry> {
    let [first_start, first_end] = curve_endpoints(curves, first)?;
    let [second_start, second_end] = curve_endpoints(curves, second)?;
    let first_line = unit(first_end.x - first_start.x, first_end.y - first_start.y)?;
    let second_line = unit(second_end.x - second_start.x, second_end.y - second_start.y)?;
    if dot(first_line, second_line).abs() > 1.0e-6 {
        return None;
    }
    let vertex = line_intersection(first_start, first_line, second_start, second_line)?;
    if !point_lies_on_segment(vertex, first_start, first_end)
        || !point_lies_on_segment(vertex, second_start, second_end)
    {
        return None;
    }
    if !point_within_view_margin(vertex, viewport, RIGHT_ANGLE_VIEW_MARGIN_PIXELS) {
        return None;
    }

    let first_ray = span_ray_toward_interior(vertex, first_start, first_end)?;
    let second_ray = span_ray_toward_interior(vertex, second_start, second_end)?;
    let left_normal = [-first_ray[1], first_ray[0]];
    let square_normal = if dot(left_normal, second_ray) >= 0.0 {
        left_normal
    } else {
        [-left_normal[0], -left_normal[1]]
    };
    let first_arm = offset(
        vertex,
        first_ray[0] * RIGHT_ANGLE_SIZE_PIXELS,
        first_ray[1] * RIGHT_ANGLE_SIZE_PIXELS,
    );
    let second_arm = offset(
        vertex,
        square_normal[0] * RIGHT_ANGLE_SIZE_PIXELS,
        square_normal[1] * RIGHT_ANGLE_SIZE_PIXELS,
    );
    let corner = offset(
        first_arm,
        square_normal[0] * RIGHT_ANGLE_SIZE_PIXELS,
        square_normal[1] * RIGHT_ANGLE_SIZE_PIXELS,
    );
    Some(SceneAnnotationGeometry::RightAngle {
        vertex,
        first_arm,
        corner,
        second_arm,
    })
}

fn point_lies_on_segment(point: ScreenPoint, start: ScreenPoint, end: ScreenPoint) -> bool {
    const EPSILON_PIXELS: f64 = 1.0e-6;
    let length = start.distance(end);
    length.is_finite()
        && length > EPSILON_PIXELS
        && point_segment_distance(point, start, end) <= EPSILON_PIXELS
        && point.distance(start) <= length + EPSILON_PIXELS
        && point.distance(end) <= length + EPSILON_PIXELS
}

fn span_ray_toward_interior(
    vertex: ScreenPoint,
    start: ScreenPoint,
    end: ScreenPoint,
) -> Option<[f64; 2]> {
    let direction = unit(end.x - start.x, end.y - start.y)?;
    let length = start.distance(end);
    let vertex_parameter = dot([vertex.x - start.x, vertex.y - start.y], direction);
    if vertex_parameter >= length - 1.0e-6 {
        Some([-direction[0], -direction[1]])
    } else {
        Some(direction)
    }
}

fn point_within_view_margin(point: ScreenPoint, viewport: Viewport, margin: f64) -> bool {
    point.is_finite()
        && point.x >= -margin
        && point.y >= -margin
        && point.x <= viewport.screen_size[0] + margin
        && point.y <= viewport.screen_size[1] + margin
}

fn curve_endpoints(curves: &[SceneCurve], span: CurveSpan) -> Option<[ScreenPoint; 2]> {
    let curve = curves.iter().find(|curve| curve.span == span)?;
    Some([
        *curve.screen_polyline.first()?,
        *curve.screen_polyline.last()?,
    ])
}

fn first_curve_span(curves: &[SceneCurve], id: CurveId) -> Option<CurveSpan> {
    curves
        .iter()
        .find(|curve| curve.span.curve == id)
        .map(|curve| curve.span)
}

fn curve_operands(curves: &[SceneCurve], id: CurveId) -> Vec<SelectionItem> {
    curves
        .iter()
        .filter(|curve| curve.span.curve == id)
        .map(|curve| SelectionItem::Curve(curve.span))
        .collect()
}

fn radial_geometry(
    document: &SketchDocument,
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
    id: CurveId,
) -> Option<(ScreenPoint, ScreenPoint, bool)> {
    let definition = &document.curve(id)?.definition;
    let (center_id, parameter, full_circle) = match definition {
        // A full circle has no distinguished presentation point. Parameter zero is
        // the canonical positive-X branch and therefore stays stable across solves.
        CurveDefinition::Circle { center, .. } => (*center, 0.0, true),
        // Bounded circular arcs use [0, 1], so their semantic midpoint is stable
        // even when adaptive tessellation changes.
        CurveDefinition::CircularArc { center, .. } => (*center, 0.5, false),
        _ => return None,
    };
    let center = point_anchor(points, center_id)?;
    let span = first_curve_span(curves, id)?;
    let jet = document.evaluate_curve_jet(span, parameter).ok()?;
    let edge = viewport.model_to_screen([jet.position.x, jet.position.y]);
    if !center.is_finite() || !edge.is_finite() {
        return None;
    }
    Some((center, edge, full_circle))
}

const GLYPH_MIN_SEPARATION_PIXELS: f64 = 22.0;
const GLYPH_RING_STEP_PIXELS: f64 = 24.0;
const GLYPH_MAX_SEARCH_RINGS: u32 = 64;
const GLYPH_VIEWPORT_MARGIN_PIXELS: f64 = SceneGlyphMarker::BOUND_RADIUS_PIXELS + 4.0;
const GLYPH_LABEL_CLEARANCE_PIXELS: f64 = SceneGlyphMarker::BOUND_RADIUS_PIXELS + 4.0;
const AUTO_LAYOUT_CLEARANCE_PIXELS: f64 = 5.0;
const AUTO_POINT_RADIUS_PIXELS: f64 = 7.0;
const AUTO_DIMENSION_STEP_PIXELS: f64 = 20.0;
const AUTO_DIMENSION_SEARCH_RINGS: u32 = 32;

#[derive(Clone, Copy)]
enum AnnotationLayoutObstacle {
    Rectangle(SceneAnnotationLabelBounds),
    Circle {
        center: ScreenPoint,
        radius: f64,
    },
    Segment {
        start: ScreenPoint,
        end: ScreenPoint,
    },
    Arc {
        center: ScreenPoint,
        first_ray: ScreenPoint,
        second_ray: ScreenPoint,
        radius: f64,
        clockwise: bool,
    },
}

fn resolve_automatic_layout(
    document: DocumentId,
    annotations: &mut [SceneAnnotation],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
    manual_layout: Option<&AnnotationLayoutState>,
) {
    let is_manual = |annotation: &SceneAnnotation, marker_index| {
        manual_layout.is_some_and(|layout| {
            layout
                .get(annotation.layout_key(document, marker_index))
                .is_some()
        })
    };
    let mut occupied = Vec::new();

    // Fixed marks and explicit user placements own their locations. Automatic
    // candidates are resolved around them rather than silently moving them.
    for annotation in annotations.iter() {
        match &annotation.geometry {
            SceneAnnotationGeometry::RightAngle {
                first_arm,
                corner,
                second_arm,
                ..
            } => {
                occupied.push(AnnotationLayoutObstacle::Rectangle(bounds_for_points(&[
                    *first_arm,
                    *corner,
                    *second_arm,
                ])));
                occupied.extend([*first_arm, *corner, *second_arm].map(|center| {
                    AnnotationLayoutObstacle::Circle {
                        center,
                        radius: 0.0,
                    }
                }));
            }
            SceneAnnotationGeometry::Glyph { markers } => {
                for (index, marker) in markers.iter().enumerate() {
                    if is_manual(annotation, Some(index)) {
                        push_marker_obstacles(marker, &mut occupied);
                    }
                }
            }
            SceneAnnotationGeometry::LinearDimension { .. }
            | SceneAnnotationGeometry::RadialDimension { .. }
            | SceneAnnotationGeometry::AngularDimension { .. }
            | SceneAnnotationGeometry::Label { .. } => {
                if is_manual(annotation, None) {
                    push_dimension_obstacles(annotation, &mut occupied);
                }
            }
        }
    }

    // Resolve dimension text before glyphs. Dimensions retain their semantic
    // baseline/radius forms; only the typed automatic placement changes.
    for annotation in annotations.iter_mut() {
        if matches!(annotation.geometry, SceneAnnotationGeometry::Glyph { .. })
            || is_manual(annotation, None)
        {
            continue;
        }
        place_dimension_automatically(annotation, &occupied, points, curves, viewport);
        push_dimension_obstacles(annotation, &mut occupied);
    }

    // Each repeated relation marker has its own stable occurrence key, so a
    // manually placed paired mark can reserve space while its sibling remains
    // automatically recomputable.
    for annotation in annotations.iter_mut() {
        let item = annotation.item;
        let source = annotation.source;
        let kind = annotation.kind;
        let SceneAnnotationGeometry::Glyph { markers } = &mut annotation.geometry else {
            continue;
        };
        for (index, marker) in markers.iter_mut().enumerate() {
            let key = AnnotationLayoutKey {
                document,
                source,
                item,
                kind,
                marker_index: Some(index),
            };
            if manual_layout.is_none_or(|layout| layout.get(key).is_none()) {
                marker.anchor =
                    glyph_fan_out_position(marker.anchor, &occupied, points, curves, viewport);
            }
            push_marker_obstacles(marker, &mut occupied);
        }
    }
}

fn push_marker_obstacles(marker: &SceneGlyphMarker, occupied: &mut Vec<AnnotationLayoutObstacle>) {
    if let Some(origin) = marker.leader_from {
        occupied.push(AnnotationLayoutObstacle::Segment {
            start: origin,
            end: marker.anchor,
        });
    }
    occupied.push(AnnotationLayoutObstacle::Circle {
        center: marker.anchor,
        radius: marker.bounds().radius,
    });
}

fn push_dimension_obstacles(
    annotation: &SceneAnnotation,
    occupied: &mut Vec<AnnotationLayoutObstacle>,
) {
    if let Some(bounds) = annotation.label_bounds {
        occupied.push(AnnotationLayoutObstacle::Rectangle(bounds));
    }
    let segment = |occupied: &mut Vec<_>, start, end| {
        occupied.push(AnnotationLayoutObstacle::Segment { start, end });
    };
    match &annotation.geometry {
        SceneAnnotationGeometry::LinearDimension {
            measured_first,
            measured_second,
            first,
            second,
            ..
        } => {
            segment(occupied, *measured_first, *first);
            segment(occupied, *measured_second, *second);
            segment(occupied, *first, *second);
        }
        SceneAnnotationGeometry::RadialDimension {
            center,
            edge,
            label_anchor,
            diameter,
            full_circle,
        } => {
            let start = if *diameter && *full_circle {
                ScreenPoint {
                    x: center.x.mul_add(2.0, -edge.x),
                    y: center.y.mul_add(2.0, -edge.y),
                }
            } else {
                *center
            };
            segment(occupied, start, *edge);
            segment(occupied, *edge, *label_anchor);
        }
        SceneAnnotationGeometry::AngularDimension {
            vertex,
            first_ray,
            second_ray,
            radius,
            clockwise,
            ..
        } => {
            segment(occupied, *vertex, *first_ray);
            segment(occupied, *vertex, *second_ray);
            occupied.push(AnnotationLayoutObstacle::Arc {
                center: *vertex,
                first_ray: *first_ray,
                second_ray: *second_ray,
                radius: *radius,
                clockwise: *clockwise,
            });
        }
        SceneAnnotationGeometry::Label {
            anchor,
            leader_from: Some(origin),
        } => segment(occupied, *origin, *anchor),
        SceneAnnotationGeometry::Glyph { .. }
        | SceneAnnotationGeometry::RightAngle { .. }
        | SceneAnnotationGeometry::Label {
            leader_from: None, ..
        } => {}
    }
}

fn place_dimension_automatically(
    annotation: &mut SceneAnnotation,
    occupied: &[AnnotationLayoutObstacle],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
) {
    if annotation.label_bounds.is_none()
        || annotation_label_is_clear(annotation, occupied, points, curves, viewport)
    {
        return;
    }
    let Some(default) = annotation.automatic_placement(None) else {
        return;
    };
    let base = annotation.clone();
    let mut candidates = Vec::new();
    match default {
        AnnotationPlacement::Linear {
            perpendicular_pixels,
        } => {
            let side = if perpendicular_pixels < 0.0 {
                -1.0
            } else {
                1.0
            };
            for ring in 1..=AUTO_DIMENSION_SEARCH_RINGS {
                let distance = AUTO_DIMENSION_STEP_PIXELS * f64::from(ring);
                candidates.push(AnnotationPlacement::Linear {
                    perpendicular_pixels: perpendicular_pixels + side * distance,
                });
                candidates.push(AnnotationPlacement::Linear {
                    perpendicular_pixels: -perpendicular_pixels - side * distance,
                });
            }
        }
        AnnotationPlacement::Radial {
            direction_radians,
            clearance_pixels,
        } => {
            let full_circle = matches!(
                annotation.geometry,
                SceneAnnotationGeometry::RadialDimension {
                    full_circle: true,
                    ..
                }
            );
            for ring in 0..=AUTO_DIMENSION_SEARCH_RINGS {
                let clearance = clearance_pixels + AUTO_DIMENSION_STEP_PIXELS * f64::from(ring);
                let phases = if full_circle { 16 } else { 1 };
                for phase_index in 0..phases {
                    let alternating = if phase_index == 0 {
                        0.0
                    } else {
                        let step = f64::from((phase_index + 1) / 2);
                        if phase_index % 2 == 1 { step } else { -step }
                    };
                    candidates.push(AnnotationPlacement::Radial {
                        direction_radians: direction_radians
                            + alternating * std::f64::consts::TAU / 16.0,
                        clearance_pixels: clearance,
                    });
                }
            }
        }
        AnnotationPlacement::Angular { radius_pixels } => {
            for ring in 1..=AUTO_DIMENSION_SEARCH_RINGS {
                candidates.push(AnnotationPlacement::Angular {
                    radius_pixels: radius_pixels + AUTO_DIMENSION_STEP_PIXELS * f64::from(ring),
                });
            }
        }
        AnnotationPlacement::Free { .. } => {
            for ring in 1..=AUTO_DIMENSION_SEARCH_RINGS {
                let radius = AUTO_DIMENSION_STEP_PIXELS * f64::from(ring);
                let slots = 8 * ring;
                for phase_index in 0..slots {
                    let phase = std::f64::consts::TAU * f64::from(phase_index) / f64::from(slots);
                    candidates.push(AnnotationPlacement::Free {
                        offset_pixels: [radius * phase.cos(), radius * phase.sin()],
                    });
                }
            }
        }
    }
    for placement in candidates {
        let mut candidate = base.clone();
        candidate.apply_placement(placement);
        if annotation_label_is_clear(&candidate, occupied, points, curves, viewport) {
            *annotation = candidate;
            return;
        }
    }
}

fn annotation_label_is_clear(
    annotation: &SceneAnnotation,
    occupied: &[AnnotationLayoutObstacle],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
) -> bool {
    annotation.label_bounds.is_none_or(|bounds| {
        rectangle_within_viewport(bounds, viewport, AUTO_LAYOUT_CLEARANCE_PIXELS)
            && points.iter().all(|point| {
                bounds.distance(point.screen_position)
                    >= AUTO_POINT_RADIUS_PIXELS + AUTO_LAYOUT_CLEARANCE_PIXELS
            })
            && curves.iter().all(|curve| {
                curve.screen_polyline.windows(2).all(|segment| {
                    segment_rectangle_distance(segment[0], segment[1], bounds)
                        >= AUTO_LAYOUT_CLEARANCE_PIXELS
                })
            })
            && occupied.iter().all(|obstacle| {
                rectangle_obstacle_distance(bounds, *obstacle) >= AUTO_LAYOUT_CLEARANCE_PIXELS
            })
    })
}

fn glyph_position_is_clear(
    candidate: ScreenPoint,
    occupied: &[AnnotationLayoutObstacle],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
) -> bool {
    candidate.is_finite()
        && candidate.x >= GLYPH_VIEWPORT_MARGIN_PIXELS
        && candidate.y >= GLYPH_VIEWPORT_MARGIN_PIXELS
        && candidate.x <= viewport.screen_size[0] - GLYPH_VIEWPORT_MARGIN_PIXELS
        && candidate.y <= viewport.screen_size[1] - GLYPH_VIEWPORT_MARGIN_PIXELS
        && points.iter().all(|point| {
            point.screen_position.distance(candidate)
                >= SceneGlyphMarker::BOUND_RADIUS_PIXELS
                    + AUTO_POINT_RADIUS_PIXELS
                    + AUTO_LAYOUT_CLEARANCE_PIXELS
        })
        && curves.iter().all(|curve| {
            curve.screen_polyline.windows(2).all(|segment| {
                point_segment_distance(candidate, segment[0], segment[1])
                    >= SceneGlyphMarker::BOUND_RADIUS_PIXELS + AUTO_LAYOUT_CLEARANCE_PIXELS
            })
        })
        && occupied.iter().all(|obstacle| match obstacle {
            AnnotationLayoutObstacle::Rectangle(bounds) => {
                bounds.distance(candidate) >= GLYPH_LABEL_CLEARANCE_PIXELS
            }
            AnnotationLayoutObstacle::Circle { center, radius } => {
                center.distance(candidate)
                    >= GLYPH_MIN_SEPARATION_PIXELS
                        + (radius - SceneGlyphMarker::BOUND_RADIUS_PIXELS).max(0.0)
            }
            AnnotationLayoutObstacle::Segment { start, end } => {
                point_segment_distance(candidate, *start, *end)
                    >= SceneGlyphMarker::BOUND_RADIUS_PIXELS + AUTO_LAYOUT_CLEARANCE_PIXELS
            }
            AnnotationLayoutObstacle::Arc {
                center,
                first_ray,
                second_ray,
                radius,
                clockwise,
            } => {
                point_arc_distance(
                    candidate,
                    *center,
                    *first_ray,
                    *second_ray,
                    *radius,
                    *clockwise,
                ) >= SceneGlyphMarker::BOUND_RADIUS_PIXELS + AUTO_LAYOUT_CLEARANCE_PIXELS
            }
        })
}

fn glyph_fan_out_position(
    original: ScreenPoint,
    occupied: &[AnnotationLayoutObstacle],
    points: &[ScenePoint],
    curves: &[SceneCurve],
    viewport: Viewport,
) -> ScreenPoint {
    if glyph_position_is_clear(original, occupied, points, curves, viewport) {
        return original;
    }
    for ring_index in 1..=GLYPH_MAX_SEARCH_RINGS {
        let radius = GLYPH_RING_STEP_PIXELS * f64::from(ring_index);
        // Six slots per 24 px of radius keep neighboring candidates about
        // 25 px apart without a lossy float-to-integer conversion.
        let slots = 6 * ring_index;
        for phase_index in 0..slots {
            let phase = std::f64::consts::TAU * f64::from(phase_index) / f64::from(slots);
            let candidate = offset(original, radius * phase.cos(), radius * phase.sin());
            if glyph_position_is_clear(candidate, occupied, points, curves, viewport) {
                return candidate;
            }
        }
    }
    original
}

fn bounds_for_points(points: &[ScreenPoint]) -> SceneAnnotationLabelBounds {
    SceneAnnotationLabelBounds {
        min: ScreenPoint {
            x: points
                .iter()
                .map(|point| point.x)
                .min_by(f64::total_cmp)
                .unwrap_or(0.0),
            y: points
                .iter()
                .map(|point| point.y)
                .min_by(f64::total_cmp)
                .unwrap_or(0.0),
        },
        max: ScreenPoint {
            x: points
                .iter()
                .map(|point| point.x)
                .max_by(f64::total_cmp)
                .unwrap_or(0.0),
            y: points
                .iter()
                .map(|point| point.y)
                .max_by(f64::total_cmp)
                .unwrap_or(0.0),
        },
    }
}

fn rectangle_within_viewport(
    bounds: SceneAnnotationLabelBounds,
    viewport: Viewport,
    margin: f64,
) -> bool {
    bounds.min.x >= margin
        && bounds.min.y >= margin
        && bounds.max.x <= viewport.screen_size[0] - margin
        && bounds.max.y <= viewport.screen_size[1] - margin
}

fn rectangle_obstacle_distance(
    bounds: SceneAnnotationLabelBounds,
    obstacle: AnnotationLayoutObstacle,
) -> f64 {
    match obstacle {
        AnnotationLayoutObstacle::Rectangle(other) => rectangle_distance(bounds, other),
        AnnotationLayoutObstacle::Circle { center, radius } => {
            (bounds.distance(center) - radius).max(0.0)
        }
        AnnotationLayoutObstacle::Segment { start, end } => {
            segment_rectangle_distance(start, end, bounds)
        }
        AnnotationLayoutObstacle::Arc {
            center,
            first_ray,
            second_ray,
            radius,
            clockwise,
        } => {
            let center_of_bounds = ScreenPoint {
                x: (bounds.min.x + bounds.max.x) * 0.5,
                y: (bounds.min.y + bounds.max.y) * 0.5,
            };
            let half_diagonal = center_of_bounds.distance(bounds.max);
            (point_arc_distance(
                center_of_bounds,
                center,
                first_ray,
                second_ray,
                radius,
                clockwise,
            ) - half_diagonal)
                .max(0.0)
        }
    }
}

fn rectangle_distance(
    first: SceneAnnotationLabelBounds,
    second: SceneAnnotationLabelBounds,
) -> f64 {
    let dx = if first.max.x < second.min.x {
        second.min.x - first.max.x
    } else if second.max.x < first.min.x {
        first.min.x - second.max.x
    } else {
        0.0
    };
    let dy = if first.max.y < second.min.y {
        second.min.y - first.max.y
    } else if second.max.y < first.min.y {
        first.min.y - second.max.y
    } else {
        0.0
    };
    dx.hypot(dy)
}

fn segment_rectangle_distance(
    start: ScreenPoint,
    end: ScreenPoint,
    bounds: SceneAnnotationLabelBounds,
) -> f64 {
    if bounds.contains(start, 0.0)
        || bounds.contains(end, 0.0)
        || segment_intersects_rectangle(start, end, bounds)
    {
        return 0.0;
    }
    let corners = [
        bounds.min,
        ScreenPoint {
            x: bounds.max.x,
            y: bounds.min.y,
        },
        bounds.max,
        ScreenPoint {
            x: bounds.min.x,
            y: bounds.max.y,
        },
    ];
    bounds.distance(start).min(bounds.distance(end)).min(
        corners
            .into_iter()
            .map(|corner| point_segment_distance(corner, start, end))
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY),
    )
}

fn segment_intersects_rectangle(
    start: ScreenPoint,
    end: ScreenPoint,
    bounds: SceneAnnotationLabelBounds,
) -> bool {
    let top_right = ScreenPoint {
        x: bounds.max.x,
        y: bounds.min.y,
    };
    let bottom_left = ScreenPoint {
        x: bounds.min.x,
        y: bounds.max.y,
    };
    [
        (bounds.min, top_right),
        (top_right, bounds.max),
        (bounds.max, bottom_left),
        (bottom_left, bounds.min),
    ]
    .into_iter()
    .any(|(first, second)| segments_intersect(start, end, first, second))
}

fn segments_intersect(
    first_start: ScreenPoint,
    first_end: ScreenPoint,
    second_start: ScreenPoint,
    second_end: ScreenPoint,
) -> bool {
    const EPSILON: f64 = 1.0e-9;
    let orientation = |a: ScreenPoint, b: ScreenPoint, c: ScreenPoint| {
        cross([b.x - a.x, b.y - a.y], [c.x - a.x, c.y - a.y])
    };
    let first_side = orientation(first_start, first_end, second_start);
    let second_side = orientation(first_start, first_end, second_end);
    let third_side = orientation(second_start, second_end, first_start);
    let fourth_side = orientation(second_start, second_end, first_end);
    (first_side * second_side < -EPSILON && third_side * fourth_side < -EPSILON)
        || (first_side.abs() <= EPSILON
            && point_lies_on_segment(second_start, first_start, first_end))
        || (second_side.abs() <= EPSILON
            && point_lies_on_segment(second_end, first_start, first_end))
        || (third_side.abs() <= EPSILON
            && point_lies_on_segment(first_start, second_start, second_end))
        || (fourth_side.abs() <= EPSILON
            && point_lies_on_segment(first_end, second_start, second_end))
}

fn unique_items(mut items: Vec<SelectionItem>) -> Vec<SelectionItem> {
    items.sort();
    items.dedup();
    items
}

fn midpoint(first: ScreenPoint, second: ScreenPoint) -> ScreenPoint {
    ScreenPoint {
        x: (first.x + second.x) * 0.5,
        y: (first.y + second.y) * 0.5,
    }
}

fn linear_dimension(
    measured_first: ScreenPoint,
    measured_second: ScreenPoint,
    perpendicular_pixels: f64,
) -> Option<SceneAnnotationGeometry> {
    let direction = unit(
        measured_second.x - measured_first.x,
        measured_second.y - measured_first.y,
    )?;
    let normal = [-direction[1], direction[0]];
    let first = offset(
        measured_first,
        normal[0] * perpendicular_pixels,
        normal[1] * perpendicular_pixels,
    );
    let second = offset(
        measured_second,
        normal[0] * perpendicular_pixels,
        normal[1] * perpendicular_pixels,
    );
    Some(SceneAnnotationGeometry::LinearDimension {
        measured_first,
        measured_second,
        first,
        second,
        label_anchor: midpoint(first, second),
    })
}

fn offset(point: ScreenPoint, x: f64, y: f64) -> ScreenPoint {
    ScreenPoint {
        x: point.x + x,
        y: point.y + y,
    }
}

const ANNOTATION_ARROW_LENGTH_PIXELS: f64 = 7.0;
const ANNOTATION_MIN_INWARD_ARROW_SPAN_PIXELS: f64 = 2.0 * ANNOTATION_ARROW_LENGTH_PIXELS + 4.0;

fn annotation_arrowhead(
    tip: ScreenPoint,
    toward_interior: [f64; 2],
) -> Option<SceneAnnotationArrowhead> {
    const HALF_WIDTH_PIXELS: f64 = 3.5;
    let direction = unit(toward_interior[0], toward_interior[1])?;
    let base = offset(
        tip,
        direction[0] * ANNOTATION_ARROW_LENGTH_PIXELS,
        direction[1] * ANNOTATION_ARROW_LENGTH_PIXELS,
    );
    let normal = [-direction[1], direction[0]];
    Some(SceneAnnotationArrowhead {
        tip,
        base_first: offset(
            base,
            normal[0] * HALF_WIDTH_PIXELS,
            normal[1] * HALF_WIDTH_PIXELS,
        ),
        base_second: offset(
            base,
            -normal[0] * HALF_WIDTH_PIXELS,
            -normal[1] * HALF_WIDTH_PIXELS,
        ),
    })
}

fn unit(x: f64, y: f64) -> Option<[f64; 2]> {
    let length = x.hypot(y);
    (length.is_finite() && length > 1.0e-9).then_some([x / length, y / length])
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[0], first[1] * second[1])
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[1], -first[1] * second[0])
}

fn line_intersection(
    first: ScreenPoint,
    first_direction: [f64; 2],
    second: ScreenPoint,
    second_direction: [f64; 2],
) -> Option<ScreenPoint> {
    let denominator = cross(first_direction, second_direction);
    if denominator.abs() <= 1.0e-6 {
        return None;
    }
    let delta = [second.x - first.x, second.y - first.y];
    let parameter = cross(delta, second_direction) / denominator;
    Some(offset(
        first,
        first_direction[0] * parameter,
        first_direction[1] * parameter,
    ))
}

fn point_segment_distance(point: ScreenPoint, start: ScreenPoint, end: ScreenPoint) -> f64 {
    let delta = [end.x - start.x, end.y - start.y];
    let length_squared = dot(delta, delta);
    if length_squared <= f64::EPSILON {
        return point.distance(start);
    }
    let projection =
        ((point.x - start.x) * delta[0] + (point.y - start.y) * delta[1]) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    point.distance(ScreenPoint {
        x: delta[0].mul_add(projection, start.x),
        y: delta[1].mul_add(projection, start.y),
    })
}

fn point_triangle_distance(
    point: ScreenPoint,
    first: ScreenPoint,
    second: ScreenPoint,
    third: ScreenPoint,
) -> f64 {
    let side = |start: ScreenPoint, end: ScreenPoint| {
        (end.x - start.x).mul_add(point.y - start.y, -(end.y - start.y) * (point.x - start.x))
    };
    let signs = [side(first, second), side(second, third), side(third, first)];
    let inside =
        signs.iter().all(|value| *value >= -1.0e-9) || signs.iter().all(|value| *value <= 1.0e-9);
    if inside {
        0.0
    } else {
        point_segment_distance(point, first, second)
            .min(point_segment_distance(point, second, third))
            .min(point_segment_distance(point, third, first))
    }
}

fn point_arc_distance(
    point: ScreenPoint,
    center: ScreenPoint,
    first_ray: ScreenPoint,
    second_ray: ScreenPoint,
    radius: f64,
    clockwise: bool,
) -> f64 {
    if !radius.is_finite() || radius <= 0.0 {
        return f64::INFINITY;
    }
    let start = (first_ray.y - center.y).atan2(first_ray.x - center.x);
    let end = (second_ray.y - center.y).atan2(second_ray.x - center.x);
    let candidate = (point.y - center.y).atan2(point.x - center.x);
    let sweep = if clockwise {
        (end - start).rem_euclid(std::f64::consts::TAU)
    } else {
        (start - end).rem_euclid(std::f64::consts::TAU)
    };
    let progress = if clockwise {
        (candidate - start).rem_euclid(std::f64::consts::TAU)
    } else {
        (start - candidate).rem_euclid(std::f64::consts::TAU)
    };
    if progress <= sweep + 1.0e-9 {
        (point.distance(center) - radius).abs()
    } else {
        let first = offset(center, radius * start.cos(), radius * start.sin());
        let second = offset(center, radius * end.cos(), radius * end.sin());
        point.distance(first).min(point.distance(second))
    }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        ContactDomain, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DocumentCenterRef,
        DocumentCommandEffect, DocumentConstraintDefinition, DocumentConstraintId,
        DocumentDimensionId, DocumentDirectionSense, DocumentEdit, DocumentFilletTrimEndpoint,
        DocumentLineSupportRef, DocumentSolveRequest, DocumentSourceId, GeometryRole, PersistentId,
        RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    };

    use super::{
        AnnotationLayoutEntry, AnnotationLayoutKey, AnnotationLayoutState, AnnotationPlacement,
        SceneAnnotation, SceneAnnotationGeometry, SceneAnnotationKind, SceneAnnotationVisibility,
        SceneConstraintGlyph, SceneCurve, SceneGlyphMarker, ScreenPoint,
        accessible_constraint_label, compact_dimension_text, contact_operand_anchor,
        curve_parameter_anchor,
    };
    use crate::{
        ComputedConstructionFragmentId, ComputedConstructionFragmentProvenance, ComputedCornerRef,
        ComputedEvaluationRevision, ComputedFeatureCornerId, ComputedFeatureId,
        ComputedSourceInterval, EditorError, EditorScene, NativeCurveSpanSource, SceneCurveOrigin,
        SelectionItem, Viewport,
    };

    fn point(x: f64) -> ScreenPoint {
        ScreenPoint { x, y: 40.0 }
    }

    #[test]
    fn m76_compact_cad_values_cover_prefix_suffix_reference_and_thresholds() {
        assert_eq!(
            compact_dimension_text(25.0, SceneAnnotationKind::PointDistance, false).as_deref(),
            Some("25")
        );
        assert_eq!(
            compact_dimension_text(12.0, SceneAnnotationKind::Radius, false).as_deref(),
            Some("R12")
        );
        assert_eq!(
            compact_dimension_text(24.0, SceneAnnotationKind::Diameter, true).as_deref(),
            Some("(⌀24)")
        );
        assert_eq!(
            compact_dimension_text(
                std::f64::consts::FRAC_PI_4,
                SceneAnnotationKind::OrientedAngle,
                false,
            )
            .as_deref(),
            Some("45°")
        );
        assert_eq!(
            compact_dimension_text(-0.0, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("0")
        );
        assert_eq!(
            compact_dimension_text(0.000_5, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("5e-4")
        );
        assert_eq!(
            compact_dimension_text(1.0e-6, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("1e-6")
        );
        assert_eq!(
            compact_dimension_text(1.0, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("1")
        );
        assert_eq!(
            compact_dimension_text(100_000.0, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("1e5")
        );
        assert_eq!(
            compact_dimension_text(1.0e6, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("1e6")
        );
        assert_eq!(
            compact_dimension_text(12_345.0, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("12350")
        );
        assert_eq!(
            compact_dimension_text(999_950.0, SceneAnnotationKind::CurveLength, false).as_deref(),
            Some("1e6")
        );
        assert_eq!(
            compact_dimension_text(
                5.0 * std::f64::consts::FRAC_PI_4,
                SceneAnnotationKind::OrientedAngle,
                false,
            )
            .as_deref(),
            Some("45°"),
            "compact display must preserve the shared acute supporting-line convention",
        );
    }

    #[test]
    fn m76_constraint_accessibility_always_names_the_semantic_family() {
        let families = [
            (SceneConstraintGlyph::Fixed, "fixed"),
            (SceneConstraintGlyph::Coincident, "coincident"),
            (SceneConstraintGlyph::Horizontal, "horizontal"),
            (SceneConstraintGlyph::Vertical, "vertical"),
            (SceneConstraintGlyph::PointOnCurve, "point-on-curve"),
            (SceneConstraintGlyph::Parallel, "parallel"),
            (SceneConstraintGlyph::Perpendicular, "perpendicular"),
            (SceneConstraintGlyph::Concentric, "concentric"),
            (SceneConstraintGlyph::Collinear, "collinear"),
            (SceneConstraintGlyph::EqualLength, "equal-length"),
            (SceneConstraintGlyph::EqualRadius, "equal-radius"),
            (SceneConstraintGlyph::Midpoint, "midpoint"),
            (SceneConstraintGlyph::Symmetry, "symmetry"),
            (SceneConstraintGlyph::Contact, "contact"),
            (SceneConstraintGlyph::Tangency, "tangency"),
            (SceneConstraintGlyph::Direction, "direction"),
            (SceneConstraintGlyph::Normal, "normal"),
            (SceneConstraintGlyph::EqualCurvature, "equal-curvature"),
            (SceneConstraintGlyph::Continuity, "continuity"),
            (SceneConstraintGlyph::Fillet, "fillet"),
        ];
        for (glyph, family) in families {
            assert_eq!(
                accessible_constraint_label("alignment 2", glyph),
                format!("alignment 2; {family} constraint"),
            );
        }
    }

    #[test]
    fn m76_painted_glyph_leader_selects_without_becoming_a_move_handle() {
        let marker = SceneGlyphMarker {
            anchor: ScreenPoint { x: 60.0, y: 40.0 },
            leader_from: Some(ScreenPoint { x: 20.0, y: 40.0 }),
            rotation_radians: 0.0,
        };
        let annotation = SceneAnnotation {
            item: SelectionItem::Constraint(DocumentConstraintId(PersistentId::from_u128(1))),
            source: DocumentSourceId(PersistentId::from_u128(2)),
            kind: SceneAnnotationKind::Constraint(SceneConstraintGlyph::Horizontal),
            operands: Vec::new(),
            geometry: SceneAnnotationGeometry::Glyph {
                markers: vec![marker],
            },
            visibility: SceneAnnotationVisibility::Always,
            suppressed: false,
            visible_text: None,
            accessible_label: "horizontal constraint".into(),
            label_bounds: None,
            reference: false,
        };
        let leader_probe = ScreenPoint { x: 40.0, y: 40.0 };
        assert_eq!(
            annotation.proximity_hit(leader_probe, 0.0),
            Some((Some(0), 0.0))
        );
        assert!(!annotation.movable_handle_hit(leader_probe, Some(0)));
        assert!(annotation.movable_handle_hit(marker.anchor, Some(0)));
    }

    #[test]
    fn m76_right_angle_square_requires_accepted_perpendicular_visible_spans() {
        let span = |id| CurveSpan::line(CurveId(PersistentId::from_u128(id)));
        let scene_curve = |span, start, end| SceneCurve {
            span,
            authoring_eligible: true,
            affine: true,
            contact_domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: SceneCurveOrigin::Native,
            screen_polyline: vec![start, end],
            screen_parameters: vec![0.0, 1.0],
            drag_handle_point: None,
        };
        let first = span(1);
        let second = span(2);
        let viewport = Viewport::new([200.0, 120.0], [0.0, 0.0], 1.0).expect("viewport");
        let horizontal = scene_curve(
            first,
            ScreenPoint { x: 10.0, y: 50.0 },
            ScreenPoint { x: 90.0, y: 50.0 },
        );
        let skew = scene_curve(
            second,
            ScreenPoint { x: 50.0, y: 10.0 },
            ScreenPoint { x: 70.0, y: 90.0 },
        );
        assert!(
            super::right_angle_geometry(&[horizontal.clone(), skew], viewport, first, second)
                .is_none()
        );

        let outside = scene_curve(
            second,
            ScreenPoint { x: 110.0, y: 20.0 },
            ScreenPoint { x: 110.0, y: 80.0 },
        );
        assert!(
            super::right_angle_geometry(&[horizontal.clone(), outside], viewport, first, second)
                .is_none()
        );

        let genuine = scene_curve(
            second,
            ScreenPoint { x: 90.0, y: 20.0 },
            ScreenPoint { x: 90.0, y: 80.0 },
        );
        assert!(
            super::right_angle_geometry(&[horizontal, genuine], viewport, first, second).is_some()
        );
    }

    #[test]
    fn m76_automatic_dimension_layout_reserves_scene_geometry_and_annotations() {
        let mut annotation = SceneAnnotation {
            item: SelectionItem::Dimension(DocumentDimensionId(PersistentId::from_u128(1))),
            source: DocumentSourceId(PersistentId::from_u128(2)),
            kind: SceneAnnotationKind::PointDistance,
            operands: Vec::new(),
            geometry: SceneAnnotationGeometry::LinearDimension {
                measured_first: ScreenPoint { x: 20.0, y: 40.0 },
                measured_second: ScreenPoint { x: 120.0, y: 40.0 },
                first: ScreenPoint { x: 20.0, y: 68.0 },
                second: ScreenPoint { x: 120.0, y: 68.0 },
                label_anchor: ScreenPoint { x: 70.0, y: 68.0 },
            },
            visibility: SceneAnnotationVisibility::Always,
            suppressed: false,
            visible_text: Some("100".into()),
            accessible_label: "point-distance dimension; 100 model units".into(),
            label_bounds: None,
            reference: false,
        };
        annotation.refresh_label_bounds();
        let blocker = SceneCurve {
            span: CurveSpan::line(CurveId(PersistentId::from_u128(3))),
            authoring_eligible: true,
            affine: true,
            contact_domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: SceneCurveOrigin::Native,
            screen_polyline: vec![
                ScreenPoint { x: 10.0, y: 68.0 },
                ScreenPoint { x: 130.0, y: 68.0 },
            ],
            screen_parameters: vec![0.0, 1.0],
            drag_handle_point: None,
        };
        let occupied = [super::AnnotationLayoutObstacle::Rectangle(
            super::SceneAnnotationLabelBounds {
                min: ScreenPoint { x: 55.0, y: 80.0 },
                max: ScreenPoint { x: 85.0, y: 100.0 },
            },
        )];
        super::place_dimension_automatically(
            &mut annotation,
            &occupied,
            &[],
            &[blocker],
            Viewport::new([160.0, 220.0], [0.0, 0.0], 1.0).expect("viewport"),
        );
        let SceneAnnotationGeometry::LinearDimension { label_anchor, .. } = annotation.geometry
        else {
            unreachable!()
        };
        assert!((label_anchor.y - 68.0).abs() > 1.0e-9);
        assert!(super::annotation_label_is_clear(
            &annotation,
            &occupied,
            &[],
            &[],
            Viewport::new([160.0, 220.0], [0.0, 0.0], 1.0).expect("viewport"),
        ));
    }

    #[test]
    fn m76_short_paired_arrows_move_outside_the_measured_span() {
        let linear = SceneAnnotationGeometry::LinearDimension {
            measured_first: ScreenPoint { x: 0.0, y: 0.0 },
            measured_second: ScreenPoint { x: 10.0, y: 0.0 },
            first: ScreenPoint { x: 0.0, y: 20.0 },
            second: ScreenPoint { x: 10.0, y: 20.0 },
            label_anchor: ScreenPoint { x: 5.0, y: 20.0 },
        };
        let arrows = linear.arrowheads();
        assert_eq!(arrows.len(), 2);
        assert!(arrows[0].base_first.x < arrows[0].tip.x);
        assert!(arrows[1].base_first.x > arrows[1].tip.x);

        let short_angle = SceneAnnotationGeometry::AngularDimension {
            vertex: ScreenPoint { x: 0.0, y: 0.0 },
            first_ray: ScreenPoint { x: 40.0, y: 0.0 },
            second_ray: ScreenPoint {
                x: 40.0 * 0.1_f64.cos(),
                y: -40.0 * 0.1_f64.sin(),
            },
            radius: 12.0,
            clockwise: false,
            label_anchor: ScreenPoint { x: 30.0, y: -1.5 },
        };
        let arrows = short_angle.arrowheads();
        assert_eq!(arrows.len(), 2);
        let first_base_y = (arrows[0].base_first.y + arrows[0].base_second.y) * 0.5;
        assert!(
            first_base_y > arrows[0].tip.y,
            "a cramped angular arrow must put its base outside the sector",
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one placement matrix keeps every M76 semantic cache form visibly comparable"
    )]
    fn m76_every_semantic_placement_form_moves_only_its_presentation_geometry() {
        let annotation = |kind, geometry| {
            let mut annotation = super::SceneAnnotation {
                item: SelectionItem::Dimension(DocumentDimensionId(PersistentId::from_u128(1))),
                source: DocumentSourceId(PersistentId::from_u128(2)),
                kind,
                operands: Vec::new(),
                geometry,
                visibility: SceneAnnotationVisibility::Always,
                suppressed: false,
                visible_text: Some("25".into()),
                accessible_label: "dimension = 25".into(),
                label_bounds: None,
                reference: false,
            };
            annotation.refresh_label_bounds();
            annotation
        };

        let mut linear = annotation(
            SceneAnnotationKind::PointDistance,
            SceneAnnotationGeometry::LinearDimension {
                measured_first: ScreenPoint { x: 0.0, y: 0.0 },
                measured_second: ScreenPoint { x: 100.0, y: 0.0 },
                first: ScreenPoint { x: 0.0, y: 28.0 },
                second: ScreenPoint { x: 100.0, y: 28.0 },
                label_anchor: ScreenPoint { x: 50.0, y: 28.0 },
            },
        );
        linear.apply_placement(AnnotationPlacement::Linear {
            perpendicular_pixels: 40.0,
        });
        assert!(matches!(
            linear.geometry,
            SceneAnnotationGeometry::LinearDimension {
                first: ScreenPoint { x: 0.0, y: 40.0 },
                second: ScreenPoint { x: 100.0, y: 40.0 },
                label_anchor: ScreenPoint { x: 50.0, y: 40.0 },
                ..
            }
        ));

        let mut radial = annotation(
            SceneAnnotationKind::Radius,
            SceneAnnotationGeometry::RadialDimension {
                center: ScreenPoint { x: 0.0, y: 0.0 },
                edge: ScreenPoint { x: 10.0, y: 0.0 },
                label_anchor: ScreenPoint { x: 30.0, y: 0.0 },
                diameter: false,
                full_circle: true,
            },
        );
        radial.apply_placement(AnnotationPlacement::Radial {
            direction_radians: std::f64::consts::FRAC_PI_2,
            clearance_pixels: 20.0,
        });
        let SceneAnnotationGeometry::RadialDimension {
            edge, label_anchor, ..
        } = radial.geometry
        else {
            unreachable!()
        };
        assert!(edge.x.abs() <= 1.0e-12 && (edge.y - 10.0).abs() <= 1.0e-12);
        assert!(label_anchor.x.abs() <= 1.0e-12 && (label_anchor.y - 30.0).abs() <= 1.0e-12);

        let mut bounded_radial = annotation(
            SceneAnnotationKind::Radius,
            SceneAnnotationGeometry::RadialDimension {
                center: ScreenPoint { x: 0.0, y: 0.0 },
                edge: ScreenPoint { x: 10.0, y: 0.0 },
                label_anchor: ScreenPoint { x: 30.0, y: 0.0 },
                diameter: false,
                full_circle: false,
            },
        );
        bounded_radial.apply_placement(AnnotationPlacement::Radial {
            direction_radians: std::f64::consts::FRAC_PI_2,
            clearance_pixels: 20.0,
        });
        assert!(matches!(
            bounded_radial.geometry,
            SceneAnnotationGeometry::RadialDimension {
                edge: ScreenPoint { x: 10.0, y: 0.0 },
                label_anchor: ScreenPoint { x: 30.0, y: 0.0 },
                ..
            }
        ));

        let mut angular = annotation(
            SceneAnnotationKind::OrientedAngle,
            SceneAnnotationGeometry::AngularDimension {
                vertex: ScreenPoint { x: 0.0, y: 0.0 },
                first_ray: ScreenPoint { x: 46.0, y: 0.0 },
                second_ray: ScreenPoint { x: 0.0, y: 46.0 },
                radius: 34.0,
                clockwise: false,
                label_anchor: ScreenPoint { x: 36.77, y: 36.77 },
            },
        );
        angular.apply_placement(AnnotationPlacement::Angular {
            radius_pixels: 50.0,
        });
        assert!(matches!(
            angular.geometry,
            SceneAnnotationGeometry::AngularDimension {
                first_ray: ScreenPoint { x: 62.0, y: 0.0 },
                second_ray: ScreenPoint { x: 0.0, y: 62.0 },
                radius: 50.0,
                ..
            }
        ));

        let mut free = annotation(
            SceneAnnotationKind::CurveLength,
            SceneAnnotationGeometry::Label {
                anchor: ScreenPoint { x: 10.0, y: 20.0 },
                leader_from: Some(ScreenPoint { x: 5.0, y: 5.0 }),
            },
        );
        free.apply_placement(AnnotationPlacement::Free {
            offset_pixels: [7.0, -3.0],
        });
        assert!(matches!(
            free.geometry,
            SceneAnnotationGeometry::Label {
                anchor: ScreenPoint { x: 17.0, y: 17.0 },
                leader_from: Some(ScreenPoint { x: 5.0, y: 5.0 }),
            }
        ));
        assert!(linear.label_bounds.is_some());
        assert!(radial.label_bounds.is_some());
        assert!(bounded_radial.label_bounds.is_some());
        assert!(angular.label_bounds.is_some());
        assert!(free.label_bounds.is_some());
    }

    #[test]
    fn m76_layout_cache_is_bounded_deterministic_and_drops_invalid_rows() {
        let document = geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(1));
        let source = geosolve_sketch::DocumentSourceId(geosolve_sketch::PersistentId::from_u128(2));
        let item = SelectionItem::Dimension(geosolve_sketch::DocumentDimensionId(
            geosolve_sketch::PersistentId::from_u128(3),
        ));
        let key = AnnotationLayoutKey {
            document,
            source,
            item,
            kind: SceneAnnotationKind::PointDistance,
            marker_index: None,
        };
        let state = AnnotationLayoutState::from_entries([
            AnnotationLayoutEntry {
                key,
                placement: AnnotationPlacement::Linear {
                    perpendicular_pixels: 32.0,
                },
            },
            AnnotationLayoutEntry {
                key,
                placement: AnnotationPlacement::Free {
                    offset_pixels: [f64::NAN, 0.0],
                },
            },
        ]);
        assert_eq!(state.entries().len(), 1);
        assert_eq!(
            state.entries()[0].placement,
            AnnotationPlacement::Linear {
                perpendicular_pixels: 32.0
            }
        );
    }

    #[test]
    fn contact_parameter_anchor_searches_retained_and_fillet_discarded_occurrences() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("point");
        let end = document.add_point("end", [1.0, 0.0]).expect("point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("curve");
        let span = CurveSpan::line(curve);
        let contact = document
            .add_curve_contact(
                "contact",
                span,
                0.75,
                0,
                ContactNeighborhood::Local {
                    lower: 0.5,
                    upper: 1.0,
                },
                None,
            )
            .expect("contact");
        let retained = SceneCurve {
            span,
            authoring_eligible: true,
            affine: true,
            contact_domain: ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            role: GeometryRole::Profile,
            source_role: GeometryRole::Profile,
            origin: SceneCurveOrigin::Native,
            screen_polyline: vec![point(0.0), point(25.0), point(50.0)],
            screen_parameters: vec![0.0, 0.25, 0.5],
            drag_handle_point: None,
        };
        let discarded = SceneCurve {
            role: GeometryRole::Construction,
            origin: SceneCurveOrigin::FilletDiscarded {
                fragment: ComputedConstructionFragmentId {
                    evaluation: ComputedEvaluationRevision::from_raw(1),
                    ordinal: 0,
                },
                source: NativeCurveSpanSource { span },
                interval: ComputedSourceInterval {
                    start: 0.5,
                    end: 1.0,
                },
                provenance: ComputedConstructionFragmentProvenance {
                    owner: ComputedCornerRef {
                        feature: ComputedFeatureId::from_raw(1),
                        corner: ComputedFeatureCornerId::from_raw(1),
                    },
                    endpoint: DocumentFilletTrimEndpoint::End,
                    base_interval: ComputedSourceInterval {
                        start: 0.0,
                        end: 1.0,
                    },
                },
            },
            screen_polyline: vec![point(50.0), point(75.0), point(100.0)],
            screen_parameters: vec![0.5, 0.75, 1.0],
            ..retained.clone()
        };

        let expected = Some((SelectionItem::Curve(span), point(75.0)));
        assert_eq!(
            contact_operand_anchor(&document, &[retained.clone(), discarded.clone()], contact),
            expected,
            "a contact on a discarded interval must not snap to the retained trim endpoint"
        );
        assert_eq!(
            contact_operand_anchor(&document, &[discarded.clone(), retained.clone()], contact),
            expected,
            "presentation occurrence order must not change the contact annotation"
        );

        let mut displaced_discarded = discarded;
        displaced_discarded.screen_polyline[0] = point(55.0);
        for occurrences in [
            vec![retained.clone(), displaced_discarded.clone()],
            vec![displaced_discarded, retained],
        ] {
            assert_eq!(
                curve_parameter_anchor(&occurrences, span, 0.5),
                Some(point(50.0)),
                "Native wins an exact retained/discarded boundary tie before fragment identity"
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact owner matrix freezes all six M71 scene relations and interaction states"
    )]
    fn retained_drafting_relations_publish_exact_headless_annotations() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let horizontal_points = [
            document
                .add_point("horizontal first", [-8.0, 5.0])
                .expect("point"),
            document
                .add_point("horizontal second", [-4.0, 5.0])
                .expect("point"),
        ];
        let horizontal = document
            .add_constraint(
                "horizontal points",
                DocumentConstraintDefinition::HorizontalPoints {
                    first: horizontal_points[0],
                    second: horizontal_points[1],
                },
            )
            .expect("horizontal");

        let vertical_points = [
            document
                .add_point("vertical first", [-8.0, 1.0])
                .expect("point"),
            document
                .add_point("vertical second", [-8.0, -3.0])
                .expect("point"),
        ];
        let vertical = document
            .add_constraint(
                "vertical points",
                DocumentConstraintDefinition::VerticalPoints {
                    first: vertical_points[0],
                    second: vertical_points[1],
                },
            )
            .expect("vertical");
        document
            .set_element_user_suppressed(
                geosolve_sketch::DocumentElementId::Constraint(vertical),
                true,
            )
            .expect("suppress vertical");

        let centers = [
            document
                .add_point("outer center", [0.0, 4.0])
                .expect("point"),
            document
                .add_point("inner center", [0.0, 4.0])
                .expect("point"),
        ];
        let radii = [2.0, 1.0].map(|value| {
            document
                .add_scalar("radius", value, ScalarUnit::Length, ScalarDomain::Positive)
                .expect("radius")
        });
        let circles = [0, 1].map(|index| {
            document
                .add_curve(
                    "circle",
                    CurveDefinition::Circle {
                        center: centers[index],
                        radius: radii[index],
                    },
                )
                .expect("circle")
        });
        let concentric = document
            .add_constraint(
                "concentric",
                DocumentConstraintDefinition::Concentric {
                    first: DocumentCenterRef { curve: circles[0] },
                    second: DocumentCenterRef { curve: circles[1] },
                },
            )
            .expect("concentric");

        let first_line_points = [
            document
                .add_point("first line start", [3.0, -2.0])
                .expect("point"),
            document
                .add_point("first line end", [6.0, -2.0])
                .expect("point"),
        ];
        let first_line = document
            .add_curve(
                "first line",
                CurveDefinition::Line {
                    start: first_line_points[0],
                    end: first_line_points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        let polyline_points = [
            document
                .add_point("polyline first", [7.0, -2.0])
                .expect("point"),
            document
                .add_point("polyline second", [9.0, -2.0])
                .expect("point"),
            document
                .add_point("polyline third", [11.0, -2.0])
                .expect("point"),
        ];
        let polyline = document
            .add_curve(
                "polyline",
                CurveDefinition::Polyline {
                    points: polyline_points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let first_span = CurveSpan::line(first_line);
        let second_span = CurveSpan {
            curve: polyline,
            segment: 1,
        };
        let collinear = document
            .add_constraint(
                "collinear",
                DocumentConstraintDefinition::Collinear {
                    first: DocumentLineSupportRef {
                        span: first_span,
                        direction: DocumentDirectionSense::Forward,
                    },
                    second: DocumentLineSupportRef {
                        span: second_span,
                        direction: DocumentDirectionSense::Reverse,
                    },
                },
            )
            .expect("collinear");

        let midpoint_point = document
            .add_point("midpoint-axis point", [4.5, -2.0])
            .expect("point");
        let horizontal_to_midpoint = document
            .add_constraint(
                "horizontal to midpoint",
                DocumentConstraintDefinition::HorizontalPointToMidpoint {
                    point: midpoint_point,
                    line: first_span,
                },
            )
            .expect("horizontal to midpoint");
        let vertical_to_midpoint = document
            .add_constraint(
                "vertical to midpoint",
                DocumentConstraintDefinition::VerticalPointToMidpoint {
                    point: midpoint_point,
                    line: first_span,
                },
            )
            .expect("vertical to midpoint");

        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let viewport = Viewport::new([1200.0, 800.0], [0.0, 0.0], 40.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene");
        assert_eq!(scene.annotations.len(), 6);
        assert_eq!(scene.constraint_entries.len(), 6);

        let entry = |id| {
            scene
                .constraint_entries
                .iter()
                .find(|entry| entry.id == id)
                .expect("constraint entry")
        };
        let expected_entries = [
            (
                horizontal,
                "horizontal points",
                SceneConstraintGlyph::Horizontal,
                horizontal_points.map(SelectionItem::Point).to_vec(),
                false,
            ),
            (
                vertical,
                "vertical points",
                SceneConstraintGlyph::Vertical,
                vertical_points.map(SelectionItem::Point).to_vec(),
                true,
            ),
            (
                concentric,
                "concentric",
                SceneConstraintGlyph::Concentric,
                circles
                    .map(|curve| SelectionItem::Curve(CurveSpan::line(curve)))
                    .to_vec(),
                false,
            ),
            (
                collinear,
                "collinear",
                SceneConstraintGlyph::Collinear,
                vec![
                    SelectionItem::Curve(first_span),
                    SelectionItem::Curve(second_span),
                ],
                false,
            ),
            (
                horizontal_to_midpoint,
                "horizontal to midpoint",
                SceneConstraintGlyph::Horizontal,
                vec![
                    SelectionItem::Point(midpoint_point),
                    SelectionItem::Curve(first_span),
                ],
                false,
            ),
            (
                vertical_to_midpoint,
                "vertical to midpoint",
                SceneConstraintGlyph::Vertical,
                vec![
                    SelectionItem::Point(midpoint_point),
                    SelectionItem::Curve(first_span),
                ],
                false,
            ),
        ];
        assert_eq!(
            scene
                .constraint_entries
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            expected_entries
                .iter()
                .map(|expected| expected.0)
                .collect::<Vec<_>>()
        );
        for (id, label, glyph, operands, suppressed) in expected_entries {
            let entry = entry(id);
            assert_eq!(entry.label, label);
            assert_eq!(entry.glyph, glyph);
            assert_eq!(entry.operands, operands);
            assert_eq!(entry.suppressed, suppressed);
            assert_eq!(
                entry.source,
                session
                    .design_document()
                    .constraint(id)
                    .expect("document constraint")
                    .source_id
            );
        }

        let annotation = |id| {
            scene
                .annotations
                .iter()
                .find(|annotation| annotation.item == SelectionItem::Constraint(id))
                .expect("annotation")
        };
        for annotation in &scene.annotations {
            if let SelectionItem::Constraint(id) = annotation.item {
                let entry = entry(id);
                assert_eq!(
                    annotation.kind,
                    SceneAnnotationKind::Constraint(entry.glyph)
                );
                assert_eq!(annotation.operands, entry.operands);
                assert_eq!(annotation.suppressed, entry.suppressed);
            }
        }
        let horizontal_annotation = annotation(horizontal);
        assert_eq!(
            horizontal_annotation.kind,
            SceneAnnotationKind::Constraint(SceneConstraintGlyph::Horizontal)
        );
        assert_eq!(
            horizontal_annotation.operands,
            horizontal_points.map(SelectionItem::Point)
        );
        assert_eq!(
            horizontal_annotation.visibility,
            SceneAnnotationVisibility::Contextual
        );
        assert!(!horizontal_annotation.suppressed);
        assert!(!horizontal_annotation.is_visible(&[], None, &[]));
        assert!(horizontal_annotation.is_visible(
            &[],
            Some(SelectionItem::Point(horizontal_points[0])),
            &[]
        ));
        assert!(horizontal_annotation.is_visible(
            &[SelectionItem::Constraint(horizontal)],
            None,
            &[]
        ));
        assert!(horizontal_annotation.is_visible(
            &[],
            None,
            &[SelectionItem::Constraint(horizontal)]
        ));

        let vertical_annotation = annotation(vertical);
        assert_eq!(
            vertical_annotation.kind,
            SceneAnnotationKind::Constraint(SceneConstraintGlyph::Vertical)
        );
        assert_eq!(
            vertical_annotation.operands,
            vertical_points.map(SelectionItem::Point)
        );
        assert!(vertical_annotation.suppressed);

        let concentric_annotation = annotation(concentric);
        assert_eq!(
            concentric_annotation.kind,
            SceneAnnotationKind::Constraint(SceneConstraintGlyph::Concentric)
        );
        assert_eq!(
            concentric_annotation.operands,
            circles.map(|curve| SelectionItem::Curve(CurveSpan::line(curve)))
        );
        let expected_center = viewport.model_to_screen(
            accepted
                .document()
                .point(centers[0])
                .expect("accepted center")
                .position,
        );
        let SceneAnnotationGeometry::Glyph { markers } = &concentric_annotation.geometry else {
            panic!("concentric must use one glyph")
        };
        assert_eq!(markers.len(), 1);
        assert!(
            markers[0].anchor.distance(expected_center) >= super::GLYPH_MIN_SEPARATION_PIXELS,
            "automatic placement must keep the center mark clear of its accepted geometry",
        );
        assert!(markers[0].anchor.is_finite());
        assert_eq!(markers[0].leader_from, Some(expected_center));
        assert!(concentric_annotation.hit_test(markers[0].anchor, 0.0));
        assert!(concentric_annotation.is_visible(
            &[SelectionItem::Curve(CurveSpan::line(circles[1]))],
            None,
            &[]
        ));

        let collinear_annotation = annotation(collinear);
        assert_eq!(
            collinear_annotation.kind,
            SceneAnnotationKind::Constraint(SceneConstraintGlyph::Collinear)
        );
        assert_eq!(
            collinear_annotation.operands,
            vec![
                SelectionItem::Curve(first_span),
                SelectionItem::Curve(second_span)
            ]
        );
        assert!(matches!(
            &collinear_annotation.geometry,
            SceneAnnotationGeometry::Glyph { markers }
                if markers.len() == 2 && markers.iter().all(|marker| marker.anchor.is_finite())
        ));

        for (id, glyph) in [
            (horizontal_to_midpoint, SceneConstraintGlyph::Horizontal),
            (vertical_to_midpoint, SceneConstraintGlyph::Vertical),
        ] {
            let midpoint_annotation = annotation(id);
            assert_eq!(
                midpoint_annotation.kind,
                SceneAnnotationKind::Constraint(glyph)
            );
            assert_eq!(
                midpoint_annotation.operands,
                vec![
                    SelectionItem::Point(midpoint_point),
                    SelectionItem::Curve(first_span),
                ]
            );
            assert!(matches!(
                &midpoint_annotation.geometry,
                SceneAnnotationGeometry::Glyph { markers }
                    if markers.len() == 2
                        && markers.iter().all(|marker| marker.anchor.is_finite())
            ));
        }
    }

    #[test]
    fn m71_f001_rejected_design_entry_is_published_without_unaccepted_annotation_geometry() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("point");
        let line_end = document.add_point("line end", [2.0, 0.0]).expect("point");
        let constrained = document
            .add_point("constrained", [1.0, 1.0])
            .expect("point");
        let line = document
            .add_curve(
                "reference line",
                CurveDefinition::Line {
                    start: first,
                    end: line_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        for (label, point) in [
            ("fix first", first),
            ("fix line end", line_end),
            ("fix constrained", constrained),
        ] {
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
        let mut session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        let accepted_before = session.accepted_state().expect("accepted state");
        let accepted_identity = accepted_before.identity();
        let accepted_document = accepted_before.document().clone();
        let outcome = session
            .apply(
                session.design_identity(),
                DocumentEdit::CreateConstraint {
                    label: "rejected horizontal to midpoint".into(),
                    definition: DocumentConstraintDefinition::HorizontalPointToMidpoint {
                        point: constrained,
                        line: CurveSpan::line(line),
                    },
                },
            )
            .expect("structurally valid rejected relation");
        let DocumentCommandEffect::CreatedConstraint(rejected) = outcome.value() else {
            panic!("created relation effect expected")
        };
        assert!(outcome.published_accepted_identity().is_none());
        assert!(session.accepted_state_for_current_input().is_none());
        let retained_accepted = session.accepted_state().expect("retained accepted state");
        assert_eq!(retained_accepted.identity(), accepted_identity);
        assert_eq!(retained_accepted.document(), &accepted_document);

        let scene = EditorScene::from_accepted_for_design(
            accepted_identity.revision().get(),
            session.design_identity(),
            &accepted_document,
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("detached retained-accepted scene");
        assert!(scene.constraint_entries.iter().any(|entry| {
            entry.id == *rejected
                && entry.source
                    == session
                        .design_document()
                        .constraint(*rejected)
                        .expect("retained rejected constraint")
                        .source_id
                && entry.label == "rejected horizontal to midpoint"
                && entry.glyph == SceneConstraintGlyph::Horizontal
                && entry.operands
                    == [
                        SelectionItem::Point(constrained),
                        SelectionItem::Curve(CurveSpan::line(line)),
                    ]
        }));
        assert!(
            scene
                .annotations
                .iter()
                .all(|annotation| annotation.item != SelectionItem::Constraint(*rejected)),
            "rejected coordinates must not acquire accepted annotation geometry"
        );
        assert_eq!(scene.accepted_document, accepted_document);
        assert!(
            matches!(
                scene.with_retained_session(&session),
                Err(EditorError::StalePreparedSketchInput)
            ),
            "a historical accepted scene beneath rejected design must remain detached"
        );
    }
}
