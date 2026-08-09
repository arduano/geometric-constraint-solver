// SPDX-License-Identifier: GPL-3.0-or-later

//! Geometry-derived, presentation-neutral constraint annotations.

use geosolve_sketch::{
    ContactId, CurveDefinition, CurveId, CurveSpan, DocumentConstraintDefinition as Constraint,
    DocumentDimensionDefinition as Dimension, DocumentDimensionMode, SketchDocument,
};

use crate::{SceneCurve, ScenePoint, ScreenPoint, SelectionItem, Viewport};

/// Semantic symbol requested for one constraint annotation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneConstraintGlyph {
    Fixed,
    Coincident,
    Horizontal,
    Vertical,
    PointOnCurve,
    Parallel,
    Perpendicular,
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
        first: ScreenPoint,
        second: ScreenPoint,
        label_anchor: ScreenPoint,
    },
    RadialDimension {
        center: ScreenPoint,
        edge: ScreenPoint,
        label_anchor: ScreenPoint,
        diameter: bool,
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
    },
}

/// Typed semantic category for one accepted annotation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// One persistent constraint or dimension projected into an accepted editor scene.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneAnnotation {
    pub item: SelectionItem,
    pub kind: SceneAnnotationKind,
    pub operands: Vec<SelectionItem>,
    pub geometry: SceneAnnotationGeometry,
    pub visibility: SceneAnnotationVisibility,
    /// Whether the persistent source is currently suppressed.
    pub suppressed: bool,
}

impl SceneAnnotation {
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
    /// Glyph leaders are deliberately excluded: they provide contextual navigation,
    /// but hovering a leader is not the same as hovering its icon.
    pub(crate) fn proximity_hit(
        &self,
        position: ScreenPoint,
        tolerance_pixels: f64,
    ) -> Option<(Option<usize>, f64)> {
        if !position.is_finite() || !tolerance_pixels.is_finite() || tolerance_pixels < 0.0 {
            return None;
        }
        let (occurrence, distance) =
            match &self.geometry {
                SceneAnnotationGeometry::Glyph { markers } => markers
                    .iter()
                    .enumerate()
                    .map(|(index, marker)| (Some(index), position.distance(marker.anchor)))
                    .min_by(|first, second| first.1.total_cmp(&second.1))?,
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
                    label_anchor,
                    first,
                    second,
                } => (
                    None,
                    position
                        .distance(*label_anchor)
                        .min(point_segment_distance(position, *first, *second)),
                ),
                SceneAnnotationGeometry::RadialDimension {
                    center,
                    edge,
                    label_anchor,
                    ..
                } => (
                    None,
                    position
                        .distance(*label_anchor)
                        .min(point_segment_distance(position, *center, *edge)),
                ),
                SceneAnnotationGeometry::AngularDimension {
                    vertex,
                    first_ray,
                    second_ray,
                    radius,
                    label_anchor,
                    ..
                } => {
                    let radial_distance = position.distance(*vertex);
                    (
                        None,
                        position
                            .distance(*label_anchor)
                            .min((radial_distance - radius).abs())
                            .min(point_segment_distance(position, *vertex, *first_ray))
                            .min(point_segment_distance(position, *vertex, *second_ray)),
                    )
                }
                SceneAnnotationGeometry::Label { anchor } => (None, position.distance(*anchor)),
            };
        (distance <= tolerance_pixels).then_some((occurrence, distance))
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
            SceneAnnotationGeometry::Label { anchor } => {
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
            SceneAnnotationGeometry::Label { anchor } => vec![*anchor],
        }
    }
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
        let geometry = match &constraint.definition {
            Constraint::Perpendicular { first, second } => {
                right_angle_geometry(curves, viewport, *first, *second)
                    .unwrap_or_else(|| glyph_geometry(anchors))
            }
            _ => glyph_geometry(anchors),
        };
        annotations.push(SceneAnnotation {
            item: SelectionItem::Constraint(constraint.id),
            kind: SceneAnnotationKind::Constraint(glyph),
            operands,
            geometry,
            visibility: SceneAnnotationVisibility::Contextual,
            suppressed: constraint.suppressed,
        });
    }
    for dimension in document.dimensions() {
        if let Some((kind, operands, geometry)) =
            dimension_presentation(document, points, curves, viewport, &dimension.definition)
        {
            annotations.push(SceneAnnotation {
                item: SelectionItem::Dimension(dimension.id),
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
            });
        }
    }
    fan_out_glyphs(&mut annotations);
    annotations
}

fn glyph_geometry(anchors: Vec<ScreenPoint>) -> SceneAnnotationGeometry {
    SceneAnnotationGeometry::Glyph {
        markers: anchors
            .into_iter()
            .map(|anchor| SceneGlyphMarker {
                anchor,
                leader_from: None,
            })
            .collect(),
    }
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
                SceneAnnotationGeometry::LinearDimension {
                    first: first_anchor,
                    second: second_anchor,
                    label_anchor: offset_midpoint(first_anchor, second_anchor, 16.0),
                },
            ))
        }
        Dimension::CurveLength { curve, .. } => Some((
            SceneAnnotationKind::CurveLength,
            vec![SelectionItem::Curve(*curve)],
            SceneAnnotationGeometry::Label {
                anchor: offset(curve_anchor(curves, *curve)?, 0.0, -14.0),
            },
        )),
        Dimension::Radius { curve, .. } | Dimension::Diameter { curve, .. } => {
            let (center, edge) = radial_geometry(document, points, curves, viewport, *curve)?;
            let diameter = matches!(definition, Dimension::Diameter { .. });
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
                    label_anchor: offset_midpoint(center, edge, -14.0),
                    diameter,
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
        }
        | Dimension::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            ..
        } => {
            let first = curve_anchor(curves, *source)?;
            let second = curve_anchor(curves, *target_segment)?;
            Some((
                if matches!(definition, Dimension::SupportingLineOffset { .. }) {
                    SceneAnnotationKind::SupportingLineOffset
                } else {
                    SceneAnnotationKind::ExactTranslatedSegmentOffset
                },
                vec![
                    SelectionItem::Curve(*source),
                    SelectionItem::Curve(*target_segment),
                ],
                SceneAnnotationGeometry::LinearDimension {
                    first,
                    second,
                    label_anchor: offset_midpoint(first, second, 16.0),
                },
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
    let radius = 34.0;
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
    let vertex = line_intersection(first_start, first_line, second_start, second_line)?;
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
) -> Option<(ScreenPoint, ScreenPoint)> {
    let definition = &document.curve(id)?.definition;
    let (center_id, parameter) = match definition {
        // A full circle has no distinguished presentation point. Parameter zero is
        // the canonical positive-X branch and therefore stays stable across solves.
        CurveDefinition::Circle { center, .. } => (*center, 0.0),
        // Bounded circular arcs use [0, 1], so their semantic midpoint is stable
        // even when adaptive tessellation changes.
        CurveDefinition::CircularArc { center, .. } => (*center, 0.5),
        _ => return None,
    };
    let center = point_anchor(points, center_id)?;
    let span = first_curve_span(curves, id)?;
    let jet = document.evaluate_curve_jet(span, parameter).ok()?;
    let edge = viewport.model_to_screen([jet.position.x, jet.position.y]);
    if !center.is_finite() || !edge.is_finite() {
        return None;
    }
    Some((center, edge))
}

const GLYPH_MIN_SEPARATION_PIXELS: f64 = 22.0;
const GLYPH_RING_STEP_PIXELS: f64 = 24.0;
const GLYPH_MAX_SEARCH_RINGS: u32 = 64;

fn fan_out_glyphs(annotations: &mut [SceneAnnotation]) {
    // Geometry-anchored right-angle squares do not move. Reserve their
    // selectable corners before placing ordinary glyphs around dense junctions.
    let mut occupied = annotations
        .iter()
        .filter_map(|annotation| match &annotation.geometry {
            SceneAnnotationGeometry::RightAngle { corner, .. } => Some(*corner),
            _ => None,
        })
        .collect::<Vec<_>>();
    for annotation in annotations {
        let SceneAnnotationGeometry::Glyph { markers } = &mut annotation.geometry else {
            continue;
        };
        for marker in markers {
            let original = marker.anchor;
            if !glyph_position_is_clear(original, &occupied) {
                marker.anchor = glyph_fan_out_position(original, &occupied);
                marker.leader_from = Some(original);
            }
            occupied.push(marker.anchor);
        }
    }
}

fn glyph_position_is_clear(candidate: ScreenPoint, occupied: &[ScreenPoint]) -> bool {
    occupied
        .iter()
        .all(|anchor| anchor.distance(candidate) >= GLYPH_MIN_SEPARATION_PIXELS)
}

fn glyph_fan_out_position(original: ScreenPoint, occupied: &[ScreenPoint]) -> ScreenPoint {
    for ring_index in 1..=GLYPH_MAX_SEARCH_RINGS {
        let radius = GLYPH_RING_STEP_PIXELS * f64::from(ring_index);
        // Six slots per 24 px of radius keep neighboring candidates about
        // 25 px apart without a lossy float-to-integer conversion.
        let slots = 6 * ring_index;
        for phase_index in 0..slots {
            let phase = std::f64::consts::TAU * f64::from(phase_index) / f64::from(slots);
            let candidate = offset(original, radius * phase.cos(), radius * phase.sin());
            if glyph_position_is_clear(candidate, occupied) {
                return candidate;
            }
        }
    }

    // The document resource limits keep ordinary sketches far below this path.
    // Still fail visibly and deterministically to the right instead of overlapping.
    let rightmost = occupied
        .iter()
        .map(|anchor| anchor.x)
        .max_by(f64::total_cmp)
        .unwrap_or(original.x);
    ScreenPoint {
        x: rightmost + GLYPH_MIN_SEPARATION_PIXELS,
        y: original.y,
    }
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

fn offset(point: ScreenPoint, x: f64, y: f64) -> ScreenPoint {
    ScreenPoint {
        x: point.x + x,
        y: point.y + y,
    }
}

fn offset_midpoint(first: ScreenPoint, second: ScreenPoint, distance: f64) -> ScreenPoint {
    let middle = midpoint(first, second);
    let direction = unit(second.x - first.x, second.y - first.y).unwrap_or([1.0, 0.0]);
    offset(middle, -direction[1] * distance, direction[0] * distance)
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

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        ContactNeighborhood, CurveDefinition, CurveSpan, DocumentFilletTrimEndpoint, GeometryRole,
        SketchDocument,
    };

    use super::{SceneCurve, ScreenPoint, contact_operand_anchor, curve_parameter_anchor};
    use crate::{
        ComputedConstructionFragmentId, ComputedConstructionFragmentProvenance, ComputedCornerRef,
        ComputedEvaluationRevision, ComputedFeatureCornerId, ComputedFeatureId,
        ComputedSourceInterval, NativeCurveSpanSource, SceneCurveOrigin, SelectionItem,
    };

    fn point(x: f64) -> ScreenPoint {
        ScreenPoint { x, y: 40.0 }
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
}
