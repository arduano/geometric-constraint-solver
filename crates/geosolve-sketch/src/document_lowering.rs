use std::collections::BTreeMap;

use geosolve_geometry::{DirectedParameterTrim, Point2, Vector2};

use crate::document::{
    ContactDomain, ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
    DocumentAngleOrientation, DocumentArcSweep, DocumentArcTangencySide, DocumentCircleContainment,
    DocumentCircleTangencyMode, DocumentConstraint, DocumentConstraintDefinition,
    DocumentConstraintId, DocumentCoordinateAxis, DocumentDimension, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentError, DocumentLineSide, DocumentSourceId,
    FeatureEndpoint, PersistentId, SketchDocument, TangentOrientation, document_arc_signed_sweep,
    document_hyperbola_branch,
};
use crate::{
    AngleOrientation, ArcCircleTangencySide, ArcId, ArcSweep, CenterDirectionBranch,
    CircleContainment, CircleId, CircleTangencyMode, ConicId, ConicKind, ContactState,
    CoordinateAxis, CurveContactNeighborhood, CurveTangentOrientation, DimensionMode,
    LineParameterDomain, LineSide, PointId, SegmentBranch, SegmentEndpoint, SegmentId, Sketch,
    SketchConstraintId, SketchCurve, SketchCurveContact, SketchDimensionId,
};

/// Runtime entities generated for one persistent curve.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeCurve {
    Line(SegmentId),
    Polyline(Vec<SegmentId>),
    Circle(CircleId),
    CircularArc(ArcId),
    QuadraticBezier(crate::BezierId),
    CubicBezier(crate::BezierId),
    Conic(ConicId),
}

/// Persistent point to ephemeral runtime identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointRuntimeMapping {
    pub persistent: DesignPointId,
    pub runtime: PointId,
}

/// Persistent curve to one or more ephemeral runtime entities.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveRuntimeMapping {
    pub persistent: CurveId,
    pub runtime: RuntimeCurve,
}

/// Ephemeral source selected by a persistent source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSource {
    Constraint(SketchConstraintId),
    Dimension(SketchDimensionId),
}

/// Persistent audit/source identity to optional runtime source.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentSourceRuntimeMapping {
    pub source_id: DocumentSourceId,
    pub label: String,
    pub runtime: Option<RuntimeSource>,
}

/// Semantic role of one persistent contact inside its runtime source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentContactRole {
    LineParameter,
    CircleAngle,
    ArcSpanParameter,
    BezierParameter,
    ConicParameter,
    FirstCurveParameter,
    SecondCurveParameter,
}

/// Persistent contact to runtime source/latent role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactRuntimeMapping {
    pub persistent: ContactId,
    pub constraint: SketchConstraintId,
    pub role: DocumentContactRole,
}

/// Complete deterministic identity remap for one lowering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentRuntimeMap {
    points: Vec<PointRuntimeMapping>,
    curves: Vec<CurveRuntimeMapping>,
    sources: Vec<DocumentSourceRuntimeMapping>,
    contacts: Vec<ContactRuntimeMapping>,
}

impl DocumentRuntimeMap {
    #[must_use]
    pub fn point_mappings(&self) -> &[PointRuntimeMapping] {
        &self.points
    }

    #[must_use]
    pub fn curve_mappings(&self) -> &[CurveRuntimeMapping] {
        &self.curves
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[DocumentSourceRuntimeMapping] {
        &self.sources
    }

    #[must_use]
    pub fn contact_mappings(&self) -> &[ContactRuntimeMapping] {
        &self.contacts
    }

    #[must_use]
    pub fn runtime_point(&self, id: DesignPointId) -> Option<PointId> {
        self.points
            .iter()
            .find_map(|mapping| (mapping.persistent == id).then_some(mapping.runtime))
    }

    #[must_use]
    pub fn runtime_curve(&self, id: CurveId) -> Option<&RuntimeCurve> {
        self.curves
            .iter()
            .find_map(|mapping| (mapping.persistent == id).then_some(&mapping.runtime))
    }

    #[must_use]
    pub fn runtime_source(&self, id: DocumentSourceId) -> Option<RuntimeSource> {
        self.sources
            .iter()
            .find_map(|mapping| (mapping.source_id == id).then_some(mapping.runtime))
            .flatten()
    }

    fn runtime_segment(&self, span: CurveSpan) -> Option<SegmentId> {
        match self.runtime_curve(span.curve)? {
            RuntimeCurve::Line(segment) => (span.segment == 0).then_some(*segment),
            RuntimeCurve::Polyline(segments) => segments.get(span.segment as usize).copied(),
            RuntimeCurve::Circle(_)
            | RuntimeCurve::CircularArc(_)
            | RuntimeCurve::QuadraticBezier(_)
            | RuntimeCurve::CubicBezier(_)
            | RuntimeCurve::Conic(_) => None,
        }
    }

    fn runtime_circle(&self, id: CurveId) -> Option<CircleId> {
        match self.runtime_curve(id)? {
            RuntimeCurve::Circle(circle) => Some(*circle),
            _ => None,
        }
    }

    fn runtime_arc(&self, id: CurveId) -> Option<ArcId> {
        match self.runtime_curve(id)? {
            RuntimeCurve::CircularArc(arc) => Some(*arc),
            _ => None,
        }
    }

    /// Returns the runtime conic generated for one persistent conic curve.
    #[must_use]
    pub fn runtime_conic(&self, id: CurveId) -> Option<ConicId> {
        match self.runtime_curve(id)? {
            RuntimeCurve::Conic(conic) => Some(*conic),
            _ => None,
        }
    }
}

/// Runtime sketch plus the deterministic remap that created it.
#[derive(Clone, Debug)]
pub struct LoweredDocument {
    sketch: Sketch,
    mappings: DocumentRuntimeMap,
}

impl LoweredDocument {
    #[must_use]
    pub const fn sketch(&self) -> &Sketch {
        &self.sketch
    }

    #[must_use]
    pub const fn mappings(&self) -> &DocumentRuntimeMap {
        &self.mappings
    }

    #[must_use]
    pub fn into_parts(self) -> (Sketch, DocumentRuntimeMap) {
        (self.sketch, self.mappings)
    }
}

impl SketchDocument {
    /// Deterministically lowers persistent semantic IDs to fresh runtime IDs.
    ///
    /// # Errors
    ///
    /// Returns a document-validation or guarded runtime-model error.
    pub fn lower(&self) -> Result<LoweredDocument, DocumentError> {
        self.validate()?;
        let mut sketch = Sketch::new(self.model_scale())?;
        let mut mappings = DocumentRuntimeMap::default();

        let mut points: Vec<_> = self.points().iter().collect();
        points.sort_by_key(|point| point.id);
        for point in points {
            let runtime = sketch.add_named_point(
                &point.label,
                Point2::new(point.position[0], point.position[1]),
            )?;
            mappings.points.push(PointRuntimeMapping {
                persistent: point.id,
                runtime,
            });
        }

        let mut curves: Vec<_> = self.curves().iter().collect();
        curves.sort_by_key(|curve| curve.id);
        for curve in curves {
            let runtime = lower_curve(self, &mut sketch, &mappings, curve)?;
            mappings.curves.push(CurveRuntimeMapping {
                persistent: curve.id,
                runtime,
            });
        }

        let constraints: BTreeMap<_, _> = self
            .constraints()
            .iter()
            .map(|constraint| (constraint.source_id, constraint))
            .collect();
        let dimensions: BTreeMap<_, _> = self
            .dimensions()
            .iter()
            .map(|dimension| (dimension.source_id, dimension))
            .collect();
        for source in self.source_order() {
            if let Some(constraint) = constraints.get(source) {
                let runtime = if constraint.suppressed {
                    None
                } else {
                    let (runtime, contacts) =
                        lower_constraint(self, &mut sketch, &mappings, constraint)?;
                    mappings.contacts.extend(contacts);
                    Some(RuntimeSource::Constraint(runtime))
                };
                mappings.sources.push(DocumentSourceRuntimeMapping {
                    source_id: *source,
                    label: constraint.label.clone(),
                    runtime,
                });
            } else if let Some(dimension) = dimensions.get(source) {
                let runtime = if dimension.suppressed {
                    None
                } else {
                    Some(RuntimeSource::Dimension(lower_dimension(
                        self,
                        &mut sketch,
                        &mappings,
                        dimension,
                    )?))
                };
                mappings.sources.push(DocumentSourceRuntimeMapping {
                    source_id: *source,
                    label: dimension.label.clone(),
                    runtime,
                });
            }
        }
        Ok(LoweredDocument { sketch, mappings })
    }

    /// Copies independently accepted runtime state back through persistent IDs.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn project_accepted_state(
        &mut self,
        sketch: &Sketch,
        mappings: &DocumentRuntimeMap,
    ) -> Result<(), DocumentError> {
        let mut candidate = self.clone();
        candidate.project_accepted_state_inner(sketch, mappings)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn project_accepted_state_inner(
        &mut self,
        sketch: &Sketch,
        mappings: &DocumentRuntimeMap,
    ) -> Result<(), DocumentError> {
        for mapping in &mappings.points {
            let position = sketch
                .point(mapping.runtime)
                .ok_or_else(|| unknown_runtime("runtime point", mapping.persistent.0))?
                .position();
            self.point_mut(mapping.persistent)
                .ok_or_else(|| unknown_runtime("point", mapping.persistent.0))?
                .position = [position.x, position.y];
        }
        for mapping in &mappings.curves {
            let (scalar, value) = match mapping.runtime {
                RuntimeCurve::Circle(circle) => {
                    let persistent = self
                        .curve(mapping.persistent)
                        .ok_or_else(|| unknown_runtime("curve", mapping.persistent.0))?;
                    let CurveDefinition::Circle { radius, .. } = persistent.definition else {
                        return invalid_runtime("curve mapping kind changed");
                    };
                    let value = sketch
                        .circle(circle)
                        .ok_or_else(|| unknown_runtime("runtime circle", mapping.persistent.0))?
                        .radius();
                    (radius, value)
                }
                RuntimeCurve::CircularArc(arc) => {
                    let persistent = self
                        .curve(mapping.persistent)
                        .ok_or_else(|| unknown_runtime("curve", mapping.persistent.0))?;
                    let CurveDefinition::CircularArc { radius, .. } = persistent.definition else {
                        return invalid_runtime("curve mapping kind changed");
                    };
                    let value = sketch
                        .arc(arc)
                        .ok_or_else(|| unknown_runtime("runtime arc", mapping.persistent.0))?
                        .radius();
                    (radius, value)
                }
                RuntimeCurve::Line(_)
                | RuntimeCurve::Polyline(_)
                | RuntimeCurve::QuadraticBezier(_)
                | RuntimeCurve::CubicBezier(_) => continue,
                RuntimeCurve::Conic(conic) => {
                    project_conic_state(self, sketch, mapping.persistent, conic)?;
                    continue;
                }
            };
            self.scalar_mut(scalar)
                .ok_or_else(|| unknown_runtime("scalar", scalar.0))?
                .value = value;
        }
        for mapping in &mappings.contacts {
            let state = sketch.contact_state(mapping.constraint)?;
            let value = match (mapping.role, state) {
                (DocumentContactRole::LineParameter, ContactState::PointOnLine { parameter }) => {
                    parameter
                }
                (
                    DocumentContactRole::LineParameter
                    | DocumentContactRole::CircleAngle
                    | DocumentContactRole::ArcSpanParameter
                    | DocumentContactRole::BezierParameter
                    | DocumentContactRole::ConicParameter,
                    ContactState::PointOnCurve { parameter }
                    | ContactState::LineCurveTangency { parameter },
                )
                | (
                    DocumentContactRole::BezierParameter,
                    ContactState::PointOnBezier { parameter }
                    | ContactState::LineBezierTangency { parameter },
                ) => parameter,
                (DocumentContactRole::CircleAngle, ContactState::PointOnCircle { angle }) => angle,
                (
                    DocumentContactRole::ArcSpanParameter,
                    ContactState::PointOnArc { span_parameter },
                ) => span_parameter,
                (
                    DocumentContactRole::LineParameter,
                    ContactState::LineCircleTangency { line_parameter, .. },
                ) => line_parameter,
                (
                    DocumentContactRole::CircleAngle,
                    ContactState::LineCircleTangency { circle_angle, .. }
                    | ContactState::CircleArcTangency { circle_angle, .. },
                ) => circle_angle,
                (
                    DocumentContactRole::ArcSpanParameter,
                    ContactState::CircleArcTangency {
                        arc_span_parameter, ..
                    },
                ) => arc_span_parameter,
                (
                    DocumentContactRole::FirstCurveParameter,
                    ContactState::CurveCurveContact {
                        first_parameter, ..
                    }
                    | ContactState::CurveCurveTangency {
                        first_parameter, ..
                    },
                ) => first_parameter,
                (
                    DocumentContactRole::SecondCurveParameter,
                    ContactState::CurveCurveContact {
                        second_parameter, ..
                    }
                    | ContactState::CurveCurveTangency {
                        second_parameter, ..
                    },
                ) => second_parameter,
                _ => return invalid_runtime("contact role does not match runtime source"),
            };
            set_contact_value(self, mapping.persistent, value)?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn lower_curve(
    document: &SketchDocument,
    sketch: &mut Sketch,
    mappings: &DocumentRuntimeMap,
    curve: &crate::document::DesignCurve,
) -> Result<RuntimeCurve, DocumentError> {
    let runtime = match &curve.definition {
        CurveDefinition::Line {
            start,
            end,
            branch_direction,
        } => {
            let span = CurveSpan::line(curve.id);
            let direction = if document.curve_branch_is_enforced(span) {
                *branch_direction
            } else {
                document.current_curve_span_direction(span)?
            };
            RuntimeCurve::Line(sketch.add_named_segment_with_branch(
                &curve.label,
                runtime_point(mappings, *start)?,
                runtime_point(mappings, *end)?,
                SegmentBranch::new(direction)?,
            )?)
        }
        CurveDefinition::Polyline {
            points,
            closed,
            branch_directions,
        } => {
            let count = points.len() - 1 + usize::from(*closed);
            let mut segments = Vec::with_capacity(count);
            for index in 0..count {
                let next = if index + 1 == points.len() {
                    0
                } else {
                    index + 1
                };
                let span = CurveSpan {
                    curve: curve.id,
                    segment: u32::try_from(index).map_err(|_| DocumentError::ResourceLimit {
                        resource: "polyline segment index",
                        actual: index,
                        limit: u32::MAX as usize,
                    })?,
                };
                let direction = if document.curve_branch_is_enforced(span) {
                    branch_directions[index]
                } else {
                    document.current_curve_span_direction(span)?
                };
                segments.push(sketch.add_named_segment_with_branch(
                    format!("{}.segment_{}", curve.label, index + 1),
                    runtime_point(mappings, points[index])?,
                    runtime_point(mappings, points[next])?,
                    SegmentBranch::new(direction)?,
                )?);
            }
            RuntimeCurve::Polyline(segments)
        }
        CurveDefinition::Circle { center, radius } => {
            RuntimeCurve::Circle(sketch.add_named_circle(
                &curve.label,
                runtime_point(mappings, *center)?,
                scalar_value(document, *radius)?,
            )?)
        }
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        } => RuntimeCurve::CircularArc(sketch.add_named_arc(
            &curve.label,
            runtime_point(mappings, *center)?,
            scalar_value(document, *radius)?,
            scalar_value(document, *start_angle)?,
            scalar_value(document, *end_angle)?,
            match sweep {
                DocumentArcSweep::CounterClockwise => ArcSweep::CounterClockwise,
                DocumentArcSweep::Clockwise => ArcSweep::Clockwise,
            },
        )?),
        CurveDefinition::QuadraticBezier {
            controls: [first, second, third],
        } => RuntimeCurve::QuadraticBezier(sketch.add_quadratic_bezier(
            &curve.label,
            [
                runtime_point(mappings, *first)?,
                runtime_point(mappings, *second)?,
                runtime_point(mappings, *third)?,
            ],
        )?),
        CurveDefinition::CubicBezier {
            controls: [first, second, third, fourth],
        } => RuntimeCurve::CubicBezier(sketch.add_cubic_bezier(
            &curve.label,
            [
                runtime_point(mappings, *first)?,
                runtime_point(mappings, *second)?,
                runtime_point(mappings, *third)?,
                runtime_point(mappings, *fourth)?,
            ],
        )?),
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => RuntimeCurve::Conic(sketch.add_named_ellipse(
            &curve.label,
            runtime_point(mappings, *center)?,
            runtime_point(mappings, *major_axis_point)?,
            scalar_value(document, *minor_axis_ratio)?,
        )?),
        CurveDefinition::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            end_angle,
            sweep,
        } => {
            let start = scalar_value(document, *start_angle)?;
            let end = scalar_value(document, *end_angle)?;
            RuntimeCurve::Conic(sketch.add_named_elliptical_arc(
                &curve.label,
                runtime_point(mappings, *center)?,
                runtime_point(mappings, *major_axis_point)?,
                scalar_value(document, *minor_axis_ratio)?,
                start,
                document_arc_signed_sweep(start, end, *sweep)?,
            )?)
        }
        CurveDefinition::RationalQuadraticConic {
            start,
            weighted_middle,
            middle_weight,
            end,
        } => RuntimeCurve::Conic(sketch.add_named_rational_quadratic(
            &curve.label,
            runtime_point(mappings, *start)?,
            Vector2::new(weighted_middle[0], weighted_middle[1]),
            scalar_value(document, *middle_weight)?,
            runtime_point(mappings, *end)?,
        )?),
        CurveDefinition::ParabolaSegment {
            vertex,
            focus,
            trim_start,
            trim_end,
        } => RuntimeCurve::Conic(sketch.add_named_parabola_segment(
            &curve.label,
            runtime_point(mappings, *vertex)?,
            runtime_point(mappings, *focus)?,
            directed_trim(document, curve.id, *trim_start, *trim_end)?,
        )?),
        CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim_start,
            trim_end,
        } => RuntimeCurve::Conic(sketch.add_named_hyperbola_segment(
            &curve.label,
            runtime_point(mappings, *center)?,
            runtime_point(mappings, *transverse_axis_point)?,
            scalar_value(document, *semi_conjugate)?,
            document_hyperbola_branch(*branch),
            directed_trim(document, curve.id, *trim_start, *trim_end)?,
        )?),
    };
    Ok(runtime)
}

fn project_conic_state(
    document: &mut SketchDocument,
    sketch: &Sketch,
    persistent: CurveId,
    runtime: ConicId,
) -> Result<(), DocumentError> {
    let definition = document
        .curve(persistent)
        .ok_or_else(|| unknown_runtime("curve", persistent.0))?
        .definition
        .clone();
    let kind = sketch
        .conic(runtime)
        .ok_or_else(|| unknown_runtime("runtime conic", persistent.0))?
        .kind();
    match (definition, kind) {
        (
            CurveDefinition::Ellipse {
                minor_axis_ratio, ..
            },
            ConicKind::Ellipse {
                minor_axis_ratio: value,
                ..
            },
        )
        | (
            CurveDefinition::EllipticalArc {
                minor_axis_ratio, ..
            },
            ConicKind::EllipticalArc {
                minor_axis_ratio: value,
                ..
            },
        ) => {
            document
                .scalar_mut(minor_axis_ratio)
                .ok_or_else(|| unknown_runtime("scalar", minor_axis_ratio.0))?
                .value = value;
        }
        (
            CurveDefinition::RationalQuadraticConic { middle_weight, .. },
            ConicKind::RationalQuadratic {
                weighted_middle,
                middle_weight: value,
                ..
            },
        ) => {
            document
                .scalar_mut(middle_weight)
                .ok_or_else(|| unknown_runtime("scalar", middle_weight.0))?
                .value = value;
            let CurveDefinition::RationalQuadraticConic {
                weighted_middle: persistent_middle,
                ..
            } = &mut document
                .curve_mut(persistent)
                .ok_or_else(|| unknown_runtime("curve", persistent.0))?
                .definition
            else {
                return invalid_runtime("curve mapping kind changed");
            };
            *persistent_middle = [weighted_middle.x, weighted_middle.y];
        }
        (
            CurveDefinition::HyperbolaSegment { semi_conjugate, .. },
            ConicKind::HyperbolaSegment {
                semi_conjugate: value,
                ..
            },
        ) => {
            document
                .scalar_mut(semi_conjugate)
                .ok_or_else(|| unknown_runtime("scalar", semi_conjugate.0))?
                .value = value;
        }
        (CurveDefinition::ParabolaSegment { .. }, ConicKind::ParabolaSegment { .. }) => {}
        _ => return invalid_runtime("curve mapping kind changed"),
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn lower_constraint(
    document: &SketchDocument,
    sketch: &mut Sketch,
    mappings: &DocumentRuntimeMap,
    constraint: &DocumentConstraint,
) -> Result<(SketchConstraintId, Vec<ContactRuntimeMapping>), DocumentError> {
    use DocumentConstraintDefinition as C;
    let mut contacts = Vec::new();
    let runtime =
        match &constraint.definition {
            C::FixedPoint { point, target } => sketch.add_fixed_point_at(
                runtime_point(mappings, *point)?,
                Point2::new(target[0], target[1]),
            )?,
            C::FixedCoordinate {
                point,
                axis,
                target,
            } => sketch.add_fixed_coordinate(
                runtime_point(mappings, *point)?,
                match axis {
                    DocumentCoordinateAxis::X => CoordinateAxis::X,
                    DocumentCoordinateAxis::Y => CoordinateAxis::Y,
                },
                *target,
            )?,
            C::Coincident { first, second } => sketch.add_coincident(
                runtime_point(mappings, *first)?,
                runtime_point(mappings, *second)?,
            )?,
            C::Horizontal { line } => sketch.add_horizontal(runtime_segment(mappings, *line)?)?,
            C::Vertical { line } => sketch.add_vertical(runtime_segment(mappings, *line)?)?,
            C::PointOnCurve { point, contact } => {
                let slot = document
                    .contact(*contact)
                    .ok_or_else(|| unknown_runtime("contact", contact.0))?;
                let runtime_contact = runtime_curve_contact(document, mappings, slot)?;
                let id =
                    sketch.add_point_on_curve(runtime_point(mappings, *point)?, runtime_contact)?;
                contacts.push(contact_mapping(*contact, id, contact_role(mappings, slot)?));
                id
            }
            C::Parallel { first, second } => sketch.add_parallel(
                runtime_segment(mappings, *first)?,
                runtime_segment(mappings, *second)?,
            )?,
            C::Perpendicular { first, second } => sketch.add_perpendicular(
                runtime_segment(mappings, *first)?,
                runtime_segment(mappings, *second)?,
            )?,
            C::EqualLength { first, second } => sketch.add_equal_segment_length(
                runtime_segment(mappings, *first)?,
                runtime_segment(mappings, *second)?,
            )?,
            C::EqualRadius { first, second } => sketch.add_equal_circle_radius(
                runtime_circle(mappings, *first)?,
                runtime_circle(mappings, *second)?,
            )?,
            C::Midpoint { point, line } => sketch.add_midpoint(
                runtime_point(mappings, *point)?,
                runtime_segment(mappings, *line)?,
            )?,
            C::SymmetricAboutLine {
                first,
                second,
                line,
            } => sketch.add_symmetric_about_line(
                runtime_point(mappings, *first)?,
                runtime_point(mappings, *second)?,
                runtime_segment(mappings, *line)?,
            )?,
            C::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            } => {
                let line = document
                    .contact(*line_contact)
                    .ok_or_else(|| unknown_runtime("contact", line_contact.0))?;
                let circle = document
                    .contact(*circle_contact)
                    .ok_or_else(|| unknown_runtime("contact", circle_contact.0))?;
                let id = sketch.add_line_circle_tangency(
                    runtime_segment(mappings, line.curve)?,
                    runtime_circle(mappings, circle.curve.curve)?,
                    line_domain(line.domain)?,
                    match side {
                        DocumentLineSide::Left => LineSide::Left,
                        DocumentLineSide::Right => LineSide::Right,
                    },
                    contact_value(document, line)?,
                    contact_value(document, circle)?,
                )?;
                contacts.push(contact_mapping(
                    *line_contact,
                    id,
                    DocumentContactRole::LineParameter,
                ));
                contacts.push(contact_mapping(
                    *circle_contact,
                    id,
                    DocumentContactRole::CircleAngle,
                ));
                id
            }
            C::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            } => sketch.add_circle_circle_tangency(
                runtime_circle(mappings, *first)?,
                runtime_circle(mappings, *second)?,
                circle_mode(*mode),
                CenterDirectionBranch::new(*center_direction)?,
            )?,
            C::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            } => {
                let circle = document
                    .contact(*circle_contact)
                    .ok_or_else(|| unknown_runtime("contact", circle_contact.0))?;
                let arc = document
                    .contact(*arc_contact)
                    .ok_or_else(|| unknown_runtime("contact", arc_contact.0))?;
                let id = sketch.add_circle_arc_tangency(
                    runtime_circle(mappings, circle.curve.curve)?,
                    runtime_arc(mappings, arc.curve.curve)?,
                    match side {
                        DocumentArcTangencySide::OutsideArc => ArcCircleTangencySide::OutsideArc,
                        DocumentArcTangencySide::InsideArc => ArcCircleTangencySide::InsideArc,
                    },
                    contact_value(document, arc)?,
                    contact_value(document, circle)?,
                )?;
                contacts.push(contact_mapping(
                    *circle_contact,
                    id,
                    DocumentContactRole::CircleAngle,
                ));
                contacts.push(contact_mapping(
                    *arc_contact,
                    id,
                    DocumentContactRole::ArcSpanParameter,
                ));
                id
            }
            C::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => {
                let contact = document
                    .contact(*curve_contact)
                    .ok_or_else(|| unknown_runtime("contact", curve_contact.0))?;
                let orientation = match contact.tangent_orientation.ok_or_else(|| {
                    DocumentError::InvalidField {
                        field: "contact.tangent_orientation",
                        message: "line-curve tangency requires orientation".into(),
                    }
                })? {
                    TangentOrientation::Aligned => CurveTangentOrientation::Aligned,
                    TangentOrientation::Opposed => CurveTangentOrientation::Opposed,
                };
                let id = sketch.add_line_curve_tangency(
                    runtime_segment(mappings, *line)?,
                    match endpoint {
                        FeatureEndpoint::Start => SegmentEndpoint::Start,
                        FeatureEndpoint::End => SegmentEndpoint::End,
                    },
                    runtime_curve_contact(document, mappings, contact)?,
                    orientation,
                )?;
                contacts.push(contact_mapping(
                    *curve_contact,
                    id,
                    contact_role(mappings, contact)?,
                ));
                id
            }
            C::CurveCurveContact {
                first_contact,
                second_contact,
            }
            | C::CurveCurveTangency {
                first_contact,
                second_contact,
            } => {
                let first = document
                    .contact(*first_contact)
                    .ok_or_else(|| unknown_runtime("contact", first_contact.0))?;
                let second = document
                    .contact(*second_contact)
                    .ok_or_else(|| unknown_runtime("contact", second_contact.0))?;
                let first_runtime = runtime_curve_contact(document, mappings, first)?;
                let second_runtime = runtime_curve_contact(document, mappings, second)?;
                let id = match &constraint.definition {
                    C::CurveCurveContact { .. } => {
                        sketch.add_curve_curve_contact(first_runtime, second_runtime)?
                    }
                    C::CurveCurveTangency { .. } => {
                        let orientation = first.tangent_orientation.ok_or_else(|| {
                            DocumentError::InvalidField {
                                field: "contact.tangent_orientation",
                                message: "curve tangency requires orientation".into(),
                            }
                        })?;
                        sketch.add_curve_curve_tangency(
                            first_runtime,
                            second_runtime,
                            match orientation {
                                TangentOrientation::Aligned => CurveTangentOrientation::Aligned,
                                TangentOrientation::Opposed => CurveTangentOrientation::Opposed,
                            },
                        )?
                    }
                    _ => unreachable!(),
                };
                contacts.push(contact_mapping(
                    *first_contact,
                    id,
                    DocumentContactRole::FirstCurveParameter,
                ));
                contacts.push(contact_mapping(
                    *second_contact,
                    id,
                    DocumentContactRole::SecondCurveParameter,
                ));
                id
            }
        };
    Ok((runtime, contacts))
}

fn lower_dimension(
    document: &SketchDocument,
    sketch: &mut Sketch,
    mappings: &DocumentRuntimeMap,
    dimension: &DocumentDimension,
) -> Result<SketchDimensionId, DocumentError> {
    use DocumentDimensionDefinition as D;
    let mode = match dimension.mode {
        DocumentDimensionMode::Driving => DimensionMode::Driving,
        DocumentDimensionMode::Reference => DimensionMode::Reference,
    };
    let runtime = match dimension.definition {
        D::PointDistance {
            first,
            second,
            target,
        } => sketch.add_point_distance(
            runtime_point(mappings, first)?,
            runtime_point(mappings, second)?,
            scalar_value(document, target)?,
            mode,
        )?,
        D::CurveLength { curve, target } => sketch.add_segment_length(
            runtime_segment(mappings, curve)?,
            scalar_value(document, target)?,
            mode,
        )?,
        D::Radius { curve, target } => match mappings
            .runtime_curve(curve)
            .ok_or_else(|| unknown_runtime("curve", curve.0))?
        {
            RuntimeCurve::Circle(circle) => {
                sketch.add_circle_radius(*circle, scalar_value(document, target)?, mode)?
            }
            RuntimeCurve::CircularArc(arc) => {
                sketch.add_arc_radius(*arc, scalar_value(document, target)?, mode)?
            }
            _ => return invalid_runtime("radius dimension requires a radial curve"),
        },
        D::Diameter { curve, target } => match mappings
            .runtime_curve(curve)
            .ok_or_else(|| unknown_runtime("curve", curve.0))?
        {
            RuntimeCurve::Circle(circle) => {
                sketch.add_circle_diameter(*circle, scalar_value(document, target)?, mode)?
            }
            RuntimeCurve::CircularArc(arc) => {
                sketch.add_arc_diameter(*arc, scalar_value(document, target)?, mode)?
            }
            _ => return invalid_runtime("diameter dimension requires a radial curve"),
        },
        D::OrientedAngle {
            first,
            second,
            target,
            orientation,
        } => sketch.add_oriented_angle(
            runtime_segment(mappings, first)?,
            runtime_segment(mappings, second)?,
            scalar_value(document, target)?,
            match orientation {
                DocumentAngleOrientation::CounterClockwise => AngleOrientation::CounterClockwise,
                DocumentAngleOrientation::Clockwise => AngleOrientation::Clockwise,
            },
            mode,
        )?,
    };
    Ok(runtime)
}

fn runtime_point(
    mappings: &DocumentRuntimeMap,
    id: DesignPointId,
) -> Result<PointId, DocumentError> {
    mappings
        .runtime_point(id)
        .ok_or_else(|| unknown_runtime("point", id.0))
}

fn runtime_segment(
    mappings: &DocumentRuntimeMap,
    span: CurveSpan,
) -> Result<SegmentId, DocumentError> {
    mappings
        .runtime_segment(span)
        .ok_or_else(|| unknown_runtime("curve span", span.curve.0))
}

fn runtime_circle(mappings: &DocumentRuntimeMap, id: CurveId) -> Result<CircleId, DocumentError> {
    mappings
        .runtime_circle(id)
        .ok_or_else(|| unknown_runtime("circle", id.0))
}

fn runtime_arc(mappings: &DocumentRuntimeMap, id: CurveId) -> Result<ArcId, DocumentError> {
    mappings
        .runtime_arc(id)
        .ok_or_else(|| unknown_runtime("arc", id.0))
}

fn scalar_value(document: &SketchDocument, id: DesignScalarId) -> Result<f64, DocumentError> {
    document
        .scalar(id)
        .map(|scalar| scalar.value)
        .ok_or_else(|| unknown_runtime("scalar", id.0))
}

fn directed_trim(
    document: &SketchDocument,
    curve: CurveId,
    start: DesignScalarId,
    end: DesignScalarId,
) -> Result<DirectedParameterTrim, DocumentError> {
    DirectedParameterTrim::try_new(scalar_value(document, start)?, scalar_value(document, end)?)
        .map_err(|source| DocumentError::ConicDefinition { curve, source })
}

fn contact_value(
    document: &SketchDocument,
    contact: &crate::document::ContactSlot,
) -> Result<f64, DocumentError> {
    let value = scalar_value(document, contact.parameter)?;
    match contact.domain {
        ContactDomain::Periodic { period } => Ok(value + f64::from(contact.winding) * period),
        ContactDomain::SupportingLine | ContactDomain::Bounded { .. } => Ok(value),
    }
}

fn runtime_curve_contact(
    document: &SketchDocument,
    mappings: &DocumentRuntimeMap,
    contact: &crate::document::ContactSlot,
) -> Result<SketchCurveContact, DocumentError> {
    Ok(SketchCurveContact {
        curve: match mappings
            .runtime_curve(contact.curve.curve)
            .ok_or_else(|| unknown_runtime("curve", contact.curve.curve.0))?
        {
            RuntimeCurve::Line(_) | RuntimeCurve::Polyline(_) => SketchCurve::Line {
                segment: runtime_segment(mappings, contact.curve)?,
                domain: line_domain(contact.domain)?,
            },
            RuntimeCurve::Circle(circle) => SketchCurve::Circle(*circle),
            RuntimeCurve::CircularArc(arc) => SketchCurve::Arc(*arc),
            RuntimeCurve::QuadraticBezier(bezier) | RuntimeCurve::CubicBezier(bezier) => {
                SketchCurve::Bezier(*bezier)
            }
            RuntimeCurve::Conic(conic) => SketchCurve::Conic(*conic),
        },
        parameter: contact_value(document, contact)?,
        neighborhood: match contact.neighborhood {
            crate::ContactNeighborhood::Interior => CurveContactNeighborhood::Interior,
            crate::ContactNeighborhood::Local { lower, upper } => {
                CurveContactNeighborhood::Local { lower, upper }
            }
            crate::ContactNeighborhood::Start => CurveContactNeighborhood::Start,
            crate::ContactNeighborhood::End => CurveContactNeighborhood::End,
        },
    })
}

fn contact_role(
    mappings: &DocumentRuntimeMap,
    contact: &crate::document::ContactSlot,
) -> Result<DocumentContactRole, DocumentError> {
    match mappings
        .runtime_curve(contact.curve.curve)
        .ok_or_else(|| unknown_runtime("curve", contact.curve.curve.0))?
    {
        RuntimeCurve::Line(_) | RuntimeCurve::Polyline(_) => Ok(DocumentContactRole::LineParameter),
        RuntimeCurve::Circle(_) => Ok(DocumentContactRole::CircleAngle),
        RuntimeCurve::CircularArc(_) => Ok(DocumentContactRole::ArcSpanParameter),
        RuntimeCurve::QuadraticBezier(_) | RuntimeCurve::CubicBezier(_) => {
            Ok(DocumentContactRole::BezierParameter)
        }
        RuntimeCurve::Conic(_) => Ok(DocumentContactRole::ConicParameter),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn set_contact_value(
    document: &mut SketchDocument,
    id: ContactId,
    value: f64,
) -> Result<(), DocumentError> {
    if !value.is_finite() {
        return invalid_runtime("accepted contact value is non-finite");
    }
    let contact = document
        .contact(id)
        .ok_or_else(|| unknown_runtime("contact", id.0))?
        .clone();
    let (principal, winding) = match contact.domain {
        ContactDomain::Periodic { period } => {
            let principal = value.rem_euclid(period);
            let winding = ((value - principal) / period).round();
            if winding < f64::from(i32::MIN) || winding > f64::from(i32::MAX) {
                return invalid_runtime("accepted contact winding exceeds i32 range");
            }
            (principal, winding as i32)
        }
        ContactDomain::SupportingLine | ContactDomain::Bounded { .. } => (value, 0),
    };
    document
        .scalar_mut(contact.parameter)
        .ok_or_else(|| unknown_runtime("scalar", contact.parameter.0))?
        .value = principal;
    document
        .contact_mut(id)
        .ok_or_else(|| unknown_runtime("contact", id.0))?
        .winding = winding;
    Ok(())
}

fn line_domain(domain: ContactDomain) -> Result<LineParameterDomain, DocumentError> {
    match domain {
        ContactDomain::SupportingLine => Ok(LineParameterDomain::SupportingLine),
        ContactDomain::Bounded { lower, upper }
            if lower.to_bits() == 0.0f64.to_bits() && upper.to_bits() == 1.0f64.to_bits() =>
        {
            Ok(LineParameterDomain::BoundedSegment)
        }
        _ => invalid_runtime("line contact domain must be supporting or bounded [0, 1]"),
    }
}

const fn circle_mode(mode: DocumentCircleTangencyMode) -> CircleTangencyMode {
    match mode {
        DocumentCircleTangencyMode::External => CircleTangencyMode::External,
        DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::FirstContainsSecond,
        } => CircleTangencyMode::Internal {
            containment: CircleContainment::FirstContainsSecond,
        },
        DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::SecondContainsFirst,
        } => CircleTangencyMode::Internal {
            containment: CircleContainment::SecondContainsFirst,
        },
    }
}

const fn contact_mapping(
    persistent: ContactId,
    constraint: SketchConstraintId,
    role: DocumentContactRole,
) -> ContactRuntimeMapping {
    ContactRuntimeMapping {
        persistent,
        constraint,
        role,
    }
}

fn unknown_runtime(kind: &'static str, id: PersistentId) -> DocumentError {
    DocumentError::UnknownId { kind, id }
}

fn invalid_runtime<T>(message: impl Into<String>) -> Result<T, DocumentError> {
    Err(DocumentError::InvalidField {
        field: "runtime mapping",
        message: message.into(),
    })
}

#[allow(dead_code)]
const fn persistent_constraint(id: DocumentConstraintId) -> PersistentId {
    id.0
}

#[allow(dead_code)]
const fn persistent_dimension(id: DocumentDimensionId) -> PersistentId {
    id.0
}
