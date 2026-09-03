// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic projection of an ordinary sketch document into managed V3 source.
//!
//! This is an equation-free authoring adapter. It serializes the document's
//! public semantic objects through the same named TypeScript builders used by
//! interactive authoring; it neither copies solver equations nor claims that a
//! source program existed before the projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use geosolve_sketch::{
    ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan,
    DesignPointId, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep,
    DocumentArcTangencySide, DocumentBSplineForm, DocumentCircleContainment,
    DocumentCircleTangencyMode, DocumentConstraint, DocumentConstraintDefinition,
    DocumentCoordinateAxis, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentCurveDirectionRelation, DocumentCurveNormalSide, DocumentDimension,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentDirectionSense, DocumentElementId,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentHyperbolaBranch,
    DocumentLineOffsetOrientation, DocumentLineSide, FeatureEndpoint, GeometryRole, ScalarUnit,
    SketchDocument, TangentOrientation,
};
use thiserror::Error;

use crate::MANAGED_SOURCE_LIMIT;

/// A native document cannot be represented truthfully by the standalone
/// managed-sketch language.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedSketchExportError {
    #[error("the source document is invalid: {0}")]
    InvalidDocument(String),
    #[error("the source document contains host-owned parameter or external-binding authority")]
    HostAuthority,
    #[error("document {kind} `{id}` is missing")]
    MissingObject { kind: &'static str, id: String },
    #[error("document {kind} `{id}` contains a non-finite value")]
    NonFinite { kind: &'static str, id: String },
    #[error("curve `{id}` uses spline topology that cannot be reproduced by the typed source API")]
    UnsupportedSplineTopology { id: CurveId },
    #[error("constraint `{id}` requires host-external authority")]
    HostConstraint { id: String },
    #[error(
        "dimension `{id}` uses Profile Offset authority, which is not a standalone document projection"
    )]
    ProfileOffsetDimension { id: String },
    #[error("the generated managed source is {actual} bytes; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
}

/// Projects one valid ordinary document into deterministic, readable, typed
/// managed V3 `sketch.ts` source.
///
/// Points are declared once and reused by every curve, so shared topology is
/// explicit rather than reconstructed from coincident coordinates. Contacts,
/// winding, neighborhoods, tangent orientation, curve roles, source order and
/// suppression are written explicitly. The returned text still requires the
/// ordinary pinned TypeScript compiler before it can become project authority.
///
/// # Errors
///
/// Returns a typed refusal for invalid/non-finite documents, host-external
/// authority, unsupported non-canonical spline knots, Profile Offset's
/// multi-object closure, or source exceeding the managed-source bound.
pub fn export_sketch_document_to_managed_source(
    document: &SketchDocument,
) -> Result<String, ManagedSketchExportError> {
    document
        .validate()
        .map_err(|error| ManagedSketchExportError::InvalidDocument(error.to_string()))?;
    if !document.parameters().is_empty()
        || !document.parameter_bindings().is_empty()
        || !document.parameter_outputs().is_empty()
        || !document.external_bindings().is_empty()
        || document.host_configuration_activation().is_some()
    {
        return Err(ManagedSketchExportError::HostAuthority);
    }

    let mut exporter = DocumentExporter::new(document);
    exporter.emit()?;
    if exporter.source.len() > MANAGED_SOURCE_LIMIT {
        return Err(ManagedSketchExportError::ResourceLimit {
            actual: exporter.source.len(),
            limit: MANAGED_SOURCE_LIMIT,
        });
    }
    Ok(exporter.source)
}

struct DocumentExporter<'a> {
    document: &'a SketchDocument,
    source: String,
    point_symbols: BTreeMap<DesignPointId, String>,
    curve_symbols: BTreeMap<CurveId, String>,
    constraint_symbols: BTreeMap<geosolve_sketch::DocumentConstraintId, String>,
    dimension_symbols: BTreeMap<geosolve_sketch::DocumentDimensionId, String>,
    inactive: BTreeSet<DocumentElementId>,
}

impl<'a> DocumentExporter<'a> {
    fn new(document: &'a SketchDocument) -> Self {
        let point_symbols = document
            .points()
            .iter()
            .enumerate()
            .map(|(index, point)| {
                (
                    point.id,
                    source_identifier("point", index + 1, &point.label),
                )
            })
            .collect();
        let curve_symbols = document
            .curves()
            .iter()
            .enumerate()
            .map(|(index, curve)| {
                (
                    curve.id,
                    source_identifier("curve", index + 1, &curve.label),
                )
            })
            .collect();
        let constraint_symbols = document
            .constraints()
            .iter()
            .enumerate()
            .map(|(index, constraint)| {
                (
                    constraint.id,
                    source_identifier("constraint", index + 1, &constraint.label),
                )
            })
            .collect();
        let dimension_symbols = document
            .dimensions()
            .iter()
            .enumerate()
            .map(|(index, dimension)| {
                (
                    dimension.id,
                    source_identifier("dimension", index + 1, &dimension.label),
                )
            })
            .collect();
        Self {
            document,
            source: String::new(),
            point_symbols,
            curve_symbols,
            constraint_symbols,
            dimension_symbols,
            inactive: document.user_inactive_elements().collect(),
        }
    }

    fn emit(&mut self) -> Result<(), ManagedSketchExportError> {
        self.source.push_str(
            "\"use geosolve sketch\";\nimport { sketch, mm, rad } from \"@geosolve/sketch-code\";\n\nexport default sketch(($) => {\n",
        );
        self.emit_points()?;
        self.emit_curves()?;
        self.emit_sources()?;
        self.emit_groups();
        self.source.push_str("  return {};\n});\n");
        Ok(())
    }

    fn emit_points(&mut self) -> Result<(), ManagedSketchExportError> {
        for point in self.document.points() {
            finite_pair(point.position, "point", point.id.to_string())?;
            let symbol = &self.point_symbols[&point.id];
            writeln!(
                self.source,
                "  const {symbol} = $.geometry.sketchPoint({symbol_json}, {{\n    point: {position},\n    label: {label},\n  }});",
                symbol_json = json_string(symbol),
                position = point_literal(point.position),
                label = json_string(&point.label),
            )
            .expect("writing managed source to a String cannot fail");
            if self.inactive.contains(&DocumentElementId::Point(point.id)) {
                writeln!(self.source, "  $.suppress({symbol});")
                    .expect("writing managed source to a String cannot fail");
            }
        }
        Ok(())
    }

    fn emit_curves(&mut self) -> Result<(), ManagedSketchExportError> {
        for curve in self.document.curves() {
            let symbol = self.curve_symbols[&curve.id].clone();
            let role = match self.document.geometry_role(curve.id).unwrap_or_default() {
                GeometryRole::Profile => "profile",
                GeometryRole::Construction => "construction",
            };
            let method_and_fields = self.curve_fields(curve.id, &curve.definition, role)?;
            writeln!(
                self.source,
                "  const {symbol} = $.geometry.{method}({symbol_json}, {{",
                method = method_and_fields.0,
                symbol_json = json_string(&symbol),
            )
            .expect("writing managed source to a String cannot fail");
            self.source.push_str(&method_and_fields.1);
            writeln!(self.source, "  }});")
                .expect("writing managed source to a String cannot fail");
            if self.inactive.contains(&DocumentElementId::Curve(curve.id)) {
                writeln!(self.source, "  $.suppress({symbol});")
                    .expect("writing managed source to a String cannot fail");
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn curve_fields(
        &self,
        id: CurveId,
        definition: &CurveDefinition,
        role: &str,
    ) -> Result<(&'static str, String), ManagedSketchExportError> {
        let label = &self
            .document
            .curve(id)
            .ok_or_else(|| missing("curve", &id))?
            .label;
        let mut fields = String::new();
        let method = match definition {
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            } => {
                finite_pair(*branch_direction, "curve", id.to_string())?;
                field(&mut fields, "start", &self.point_ref(*start)?);
                field(&mut fields, "end", &self.point_ref(*end)?);
                field(
                    &mut fields,
                    "branchDirection",
                    point_literal(*branch_direction),
                );
                presentation_fields(&mut fields, label, role);
                "segment"
            }
            CurveDefinition::Polyline {
                points,
                closed,
                branch_directions,
            } => {
                if branch_directions.iter().any(|value| !finite(*value)) {
                    return Err(non_finite("curve", &id));
                }
                fields.push_str("    vertices: [{\n");
                for (index, point) in points.iter().enumerate() {
                    if index > 0 {
                        fields.push_str("    }, {\n");
                    }
                    writeln!(
                        fields,
                        "      key: {},\n      position: {},",
                        json_string(&polyline_key(index)),
                        self.point_ref(*point)?,
                    )
                    .expect("writing managed source to a String cannot fail");
                }
                fields.push_str("    }],\n");
                field(&mut fields, "closed", bool_literal(*closed));
                presentation_fields(&mut fields, label, role);
                "polyline"
            }
            CurveDefinition::Circle { center, radius } => {
                field(&mut fields, "center", &self.point_ref(*center)?);
                field(&mut fields, "radius", &self.length_scalar(*radius)?);
                presentation_fields(&mut fields, label, role);
                "centerRadiusCircle"
            }
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            } => {
                self.require_scalar_unit(*radius, ScalarUnit::Length)?;
                self.require_scalar_unit(*start_angle, ScalarUnit::Angle)?;
                self.require_scalar_unit(*end_angle, ScalarUnit::Angle)?;
                field(&mut fields, "center", &self.point_ref(*center)?);
                field(
                    &mut fields,
                    "start",
                    point_literal(self.curve_endpoint(id, 0.0)?),
                );
                field(
                    &mut fields,
                    "end",
                    point_literal(self.curve_endpoint(id, 1.0)?),
                );
                field(&mut fields, "sweep", json_string(sweep_name(*sweep)));
                presentation_fields(&mut fields, label, role);
                "centerArc"
            }
            CurveDefinition::QuadraticBezier { controls } => {
                field(&mut fields, "start", &self.point_ref(controls[0])?);
                field(&mut fields, "control", &self.point_ref(controls[1])?);
                field(&mut fields, "end", &self.point_ref(controls[2])?);
                presentation_fields(&mut fields, label, role);
                "quadraticBezier"
            }
            CurveDefinition::CubicBezier { controls } => {
                field(&mut fields, "start", &self.point_ref(controls[0])?);
                field(&mut fields, "firstControl", &self.point_ref(controls[1])?);
                field(&mut fields, "secondControl", &self.point_ref(controls[2])?);
                field(&mut fields, "end", &self.point_ref(controls[3])?);
                presentation_fields(&mut fields, label, role);
                "cubicBezier"
            }
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            } => {
                field(&mut fields, "center", &self.point_ref(*center)?);
                field(
                    &mut fields,
                    "majorAxisPoint",
                    &self.point_ref(*major_axis_point)?,
                );
                field(
                    &mut fields,
                    "minorAxisPoint",
                    point_literal(self.minor_axis_point(
                        *center,
                        *major_axis_point,
                        *minor_axis_ratio,
                    )?),
                );
                presentation_fields(&mut fields, label, role);
                "centerAxesEllipse"
            }
            CurveDefinition::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            } => {
                self.require_scalar_unit(*start_angle, ScalarUnit::Angle)?;
                self.require_scalar_unit(*end_angle, ScalarUnit::Angle)?;
                field(&mut fields, "center", &self.point_ref(*center)?);
                field(
                    &mut fields,
                    "majorAxisPoint",
                    &self.point_ref(*major_axis_point)?,
                );
                field(
                    &mut fields,
                    "minorAxisPoint",
                    point_literal(self.minor_axis_point(
                        *center,
                        *major_axis_point,
                        *minor_axis_ratio,
                    )?),
                );
                field(
                    &mut fields,
                    "start",
                    point_literal(self.curve_endpoint(id, 0.0)?),
                );
                field(
                    &mut fields,
                    "end",
                    point_literal(self.curve_endpoint(id, 1.0)?),
                );
                field(&mut fields, "sweep", json_string(sweep_name(*sweep)));
                presentation_fields(&mut fields, label, role);
                "centerAxesEllipticalArc"
            }
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle,
                middle_weight,
                end,
            } => {
                finite_pair(*weighted_middle, "curve", id.to_string())?;
                field(&mut fields, "start", &self.point_ref(*start)?);
                field(&mut fields, "end", &self.point_ref(*end)?);
                field(
                    &mut fields,
                    "weightedMiddle",
                    point_literal(*weighted_middle),
                );
                field(
                    &mut fields,
                    "middleWeight",
                    &self.dimensionless_scalar(*middle_weight)?,
                );
                presentation_fields(&mut fields, label, role);
                "rationalQuadraticConic"
            }
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            } => {
                field(&mut fields, "vertex", &self.point_ref(*vertex)?);
                field(&mut fields, "focus", &self.point_ref(*focus)?);
                field(
                    &mut fields,
                    "trimStart",
                    &self.dimensionless_scalar(*trim_start)?,
                );
                field(
                    &mut fields,
                    "trimEnd",
                    &self.dimensionless_scalar(*trim_end)?,
                );
                presentation_fields(&mut fields, label, role);
                "parabola"
            }
            CurveDefinition::HyperbolaSegment {
                center,
                transverse_axis_point,
                semi_conjugate,
                branch,
                trim_start,
                trim_end,
            } => {
                field(&mut fields, "center", &self.point_ref(*center)?);
                field(
                    &mut fields,
                    "transverseAxisPoint",
                    &self.point_ref(*transverse_axis_point)?,
                );
                field(
                    &mut fields,
                    "semiConjugate",
                    &self.length_scalar(*semi_conjugate)?,
                );
                field(
                    &mut fields,
                    "trimStart",
                    &self.dimensionless_scalar(*trim_start)?,
                );
                field(
                    &mut fields,
                    "trimEnd",
                    &self.dimensionless_scalar(*trim_end)?,
                );
                field(
                    &mut fields,
                    "branch",
                    json_string(match branch {
                        DocumentHyperbolaBranch::Positive => "positive",
                        DocumentHyperbolaBranch::Negative => "negative",
                    }),
                );
                presentation_fields(&mut fields, label, role);
                "hyperbola"
            }
            CurveDefinition::BSpline {
                form,
                degree,
                controls,
                knots,
                span_ids,
                ..
            } => {
                Self::validate_spline_topology(
                    id,
                    *form,
                    *degree,
                    controls.len(),
                    knots,
                    span_ids,
                )?;
                self.spline_fields(&mut fields, controls, &vec![1.0; controls.len()], *degree)?;
                presentation_fields(&mut fields, label, role);
                nurbs_method(*form)
            }
            CurveDefinition::Nurbs {
                form,
                degree,
                controls,
                weights,
                gauge_weight,
                knots,
                span_ids,
                ..
            } => {
                Self::validate_spline_topology(
                    id,
                    *form,
                    *degree,
                    controls.len(),
                    knots,
                    span_ids,
                )?;
                let values = weights
                    .iter()
                    .map(|scalar| self.dimensionless_scalar_number(*scalar))
                    .collect::<Result<Vec<_>, _>>()?;
                let gauge = weights
                    .iter()
                    .position(|weight| weight == gauge_weight)
                    .ok_or_else(|| ManagedSketchExportError::MissingObject {
                        kind: "NURBS gauge weight",
                        id: gauge_weight.to_string(),
                    })?;
                self.spline_fields_with_gauge(&mut fields, controls, &values, *degree, gauge)?;
                presentation_fields(&mut fields, label, role);
                nurbs_method(*form)
            }
        };
        Ok((method, fields))
    }

    fn validate_spline_topology(
        id: CurveId,
        form: DocumentBSplineForm,
        degree: u32,
        controls: usize,
        knots: &[f64],
        span_ids: &[u32],
    ) -> Result<(), ManagedSketchExportError> {
        if knots.iter().any(|value| !value.is_finite()) {
            return Err(non_finite("curve", &id));
        }
        let degree = usize::try_from(degree)
            .map_err(|_| ManagedSketchExportError::UnsupportedSplineTopology { id })?;
        let expected_spans = match form {
            DocumentBSplineForm::Clamped => controls.checked_sub(degree),
            DocumentBSplineForm::Periodic => Some(controls),
        };
        if expected_spans != Some(span_ids.len()) {
            return Err(ManagedSketchExportError::UnsupportedSplineTopology { id });
        }
        // The named control-NURBS builders own canonical uniform knots. Knot
        // insertion is a separate operation and must not be silently erased.
        let expected_knots = canonical_knots(form, degree, controls)
            .ok_or(ManagedSketchExportError::UnsupportedSplineTopology { id })?;
        let same = knots.len() == expected_knots.len()
            && knots
                .iter()
                .zip(expected_knots)
                .all(|(actual, expected)| actual.to_bits() == expected.to_bits());
        if same {
            Ok(())
        } else {
            Err(ManagedSketchExportError::UnsupportedSplineTopology { id })
        }
    }

    fn spline_fields(
        &self,
        fields: &mut String,
        controls: &[DesignPointId],
        weights: &[f64],
        degree: u32,
    ) -> Result<(), ManagedSketchExportError> {
        self.spline_fields_with_gauge(fields, controls, weights, degree, 0)
    }

    fn spline_fields_with_gauge(
        &self,
        fields: &mut String,
        controls: &[DesignPointId],
        weights: &[f64],
        degree: u32,
        gauge: usize,
    ) -> Result<(), ManagedSketchExportError> {
        fields.push_str("    controls: [{\n");
        for (index, (point, weight)) in controls.iter().zip(weights).enumerate() {
            if !weight.is_finite() || *weight <= 0.0 {
                return Err(ManagedSketchExportError::NonFinite {
                    kind: "NURBS weight",
                    id: point.to_string(),
                });
            }
            if index > 0 {
                fields.push_str("    }, {\n");
            }
            writeln!(
                fields,
                "      key: {},\n      position: {},\n      weight: {},",
                json_string(&spline_key(index)),
                self.point_ref(*point)?,
                number(*weight),
            )
            .expect("writing managed source to a String cannot fail");
        }
        fields.push_str("    }],\n");
        field(fields, "degree", degree.to_string());
        field(fields, "gauge", json_string(&spline_key(gauge)));
        Ok(())
    }

    fn emit_sources(&mut self) -> Result<(), ManagedSketchExportError> {
        for source in self.document.sources() {
            match source.owner {
                geosolve_sketch::DocumentSourceOwner::Constraint(id) => {
                    let constraint = self.document.constraint(id).ok_or_else(|| {
                        ManagedSketchExportError::MissingObject {
                            kind: "constraint",
                            id: id.to_string(),
                        }
                    })?;
                    self.emit_constraint(constraint)?;
                }
                geosolve_sketch::DocumentSourceOwner::Dimension(id) => {
                    let dimension = self.document.dimension(id).ok_or_else(|| {
                        ManagedSketchExportError::MissingObject {
                            kind: "dimension",
                            id: id.to_string(),
                        }
                    })?;
                    self.emit_dimension(dimension)?;
                }
            }
        }
        Ok(())
    }

    fn emit_constraint(
        &mut self,
        constraint: &DocumentConstraint,
    ) -> Result<(), ManagedSketchExportError> {
        let symbol = self.constraint_symbols[&constraint.id].clone();
        let (method, mut fields) = self.constraint_fields(constraint)?;
        field(&mut fields, "label", json_string(&constraint.label));
        if constraint.suppressed {
            field(&mut fields, "suppressed", "true");
        }
        writeln!(
            self.source,
            "  const {symbol} = $.constraint.{method}({symbol_json}, {{",
            symbol_json = json_string(&symbol),
        )
        .expect("writing managed source to a String cannot fail");
        self.source.push_str(&fields);
        self.source.push_str("  });\n");
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn constraint_fields(
        &self,
        constraint: &DocumentConstraint,
    ) -> Result<(&'static str, String), ManagedSketchExportError> {
        use DocumentConstraintDefinition as C;
        let mut fields = String::new();
        let method = match &constraint.definition {
            C::FixedPoint { point, target } => {
                finite_pair(*target, "constraint", constraint.id.to_string())?;
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "target", point_literal(*target));
                "fixedPoint"
            }
            C::FixedCoordinate {
                point,
                axis,
                target,
            } => {
                finite_number(*target, "constraint", constraint.id.to_string())?;
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "axis", json_string(axis_name(*axis)));
                field(&mut fields, "target", unit("mm", *target));
                "fixedCoordinate"
            }
            C::CoincidentWithOrigin { point } => {
                field(&mut fields, "point", &self.point_ref(*point)?);
                "coincidentWithOrigin"
            }
            C::PointOnDatumAxis { point, axis } => {
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "axis", json_string(axis_name(*axis)));
                "pointOnDatumAxis"
            }
            C::Coincident { first, second } => {
                point_pair(self, &mut fields, *first, *second)?;
                "coincident"
            }
            C::ExternalPointCoincident { .. } | C::ExternalLineCollinear { .. } => {
                return Err(ManagedSketchExportError::HostConstraint {
                    id: constraint.id.to_string(),
                });
            }
            C::Horizontal { line } => {
                field(&mut fields, "span", &self.span_ref(*line)?);
                "horizontal"
            }
            C::Vertical { line } => {
                field(&mut fields, "span", &self.span_ref(*line)?);
                "vertical"
            }
            C::HorizontalPoints { first, second } => {
                point_pair(self, &mut fields, *first, *second)?;
                "horizontalPoints"
            }
            C::VerticalPoints { first, second } => {
                point_pair(self, &mut fields, *first, *second)?;
                "verticalPoints"
            }
            C::HorizontalPointToMidpoint { point, line } => {
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "line", &self.span_ref(*line)?);
                "horizontalPointToMidpoint"
            }
            C::VerticalPointToMidpoint { point, line } => {
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "line", &self.span_ref(*line)?);
                "verticalPointToMidpoint"
            }
            C::PointOnCurve { point, contact } => {
                let contact_value = self.contact(*contact)?;
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "curve", &self.span_ref(contact_value.curve)?);
                field(&mut fields, "contact", &self.contact_state(*contact, 6)?);
                "pointOnCurve"
            }
            C::Parallel { first, second } => {
                span_pair(self, &mut fields, *first, *second)?;
                "parallel"
            }
            C::Perpendicular { first, second } => {
                span_pair(self, &mut fields, *first, *second)?;
                "perpendicular"
            }
            C::CollinearWithDatumAxis { line, axis } => {
                field(&mut fields, "span", &self.span_ref(line.span)?);
                field(&mut fields, "axis", json_string(axis_name(*axis)));
                field(
                    &mut fields,
                    "direction",
                    json_string(direction_name(line.direction)),
                );
                "collinearWithDatumAxis"
            }
            C::Concentric { first, second } => {
                field(&mut fields, "first", &self.curve_ref(first.curve)?);
                field(&mut fields, "second", &self.curve_ref(second.curve)?);
                "concentric"
            }
            C::Collinear { first, second } => {
                field(&mut fields, "first", &self.span_ref(first.span)?);
                field(&mut fields, "second", &self.span_ref(second.span)?);
                field(
                    &mut fields,
                    "firstDirection",
                    json_string(direction_name(first.direction)),
                );
                field(
                    &mut fields,
                    "secondDirection",
                    json_string(direction_name(second.direction)),
                );
                "collinear"
            }
            C::EqualLength { first, second } => {
                span_pair(self, &mut fields, *first, *second)?;
                "equalLength"
            }
            C::EqualRadius { first, second } => {
                field(&mut fields, "first", &self.curve_ref(*first)?);
                field(&mut fields, "second", &self.curve_ref(*second)?);
                "equalRadius"
            }
            C::Midpoint { point, line } => {
                field(&mut fields, "point", &self.point_ref(*point)?);
                field(&mut fields, "line", &self.span_ref(*line)?);
                "midpoint"
            }
            C::SymmetricAboutLine {
                first,
                second,
                line,
            } => {
                point_pair(self, &mut fields, *first, *second)?;
                field(&mut fields, "axis", &self.span_ref(*line)?);
                "symmetricAboutLine"
            }
            C::SymmetricAboutDatumAxis {
                first,
                second,
                axis,
            } => {
                point_pair(self, &mut fields, *first, *second)?;
                field(&mut fields, "axis", json_string(axis_name(*axis)));
                "symmetricAboutDatumAxis"
            }
            C::LineCircleTangency {
                line_contact,
                circle_contact,
                side,
            } => {
                let first = self.contact(*line_contact)?;
                let second = self.contact(*circle_contact)?;
                field(&mut fields, "line", &self.span_ref(first.curve)?);
                field(&mut fields, "circle", &self.curve_ref(second.curve.curve)?);
                paired_contacts(self, &mut fields, *line_contact, *circle_contact)?;
                field(&mut fields, "side", json_string(side_name(*side)));
                "lineCircleTangency"
            }
            C::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            } => {
                finite_pair(*center_direction, "constraint", constraint.id.to_string())?;
                field(&mut fields, "first", &self.curve_ref(*first)?);
                field(&mut fields, "second", &self.curve_ref(*second)?);
                field(&mut fields, "mode", json_string(circle_mode(*mode)));
                field(
                    &mut fields,
                    "centerDirection",
                    point_literal(*center_direction),
                );
                "circleCircleTangency"
            }
            C::CircleArcTangency {
                circle_contact,
                arc_contact,
                side,
            } => {
                let first = self.contact(*circle_contact)?;
                let second = self.contact(*arc_contact)?;
                field(&mut fields, "circle", &self.curve_ref(first.curve.curve)?);
                field(&mut fields, "arc", &self.curve_ref(second.curve.curve)?);
                paired_contacts(self, &mut fields, *circle_contact, *arc_contact)?;
                field(
                    &mut fields,
                    "side",
                    json_string(match side {
                        DocumentArcTangencySide::OutsideArc => "outsideArc",
                        DocumentArcTangencySide::InsideArc => "insideArc",
                    }),
                );
                "circleArcTangency"
            }
            C::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => {
                let contact = self.contact(*curve_contact)?;
                field(&mut fields, "line", &self.span_ref(*line)?);
                field(&mut fields, "curve", &self.span_ref(contact.curve)?);
                field(
                    &mut fields,
                    "endpoint",
                    json_string(endpoint_name(*endpoint)),
                );
                field(
                    &mut fields,
                    "contact",
                    &self.contact_state(*curve_contact, 6)?,
                );
                "lineCurveTangency"
            }
            C::CurveCurveContact {
                first_contact,
                second_contact,
            } => {
                contact_span_pair(self, &mut fields, *first_contact, *second_contact)?;
                paired_contacts(self, &mut fields, *first_contact, *second_contact)?;
                "curveCurveContact"
            }
            C::CurveCurveTangency {
                first_contact,
                second_contact,
            } => {
                contact_span_pair(self, &mut fields, *first_contact, *second_contact)?;
                paired_contacts(self, &mut fields, *first_contact, *second_contact)?;
                "curveCurveTangency"
            }
            C::CurveDirection {
                line,
                curve_contact,
                relation,
            } => {
                let contact = self.contact(*curve_contact)?;
                field(&mut fields, "first", &self.span_ref(*line)?);
                field(&mut fields, "second", &self.span_ref(contact.curve)?);
                field(
                    &mut fields,
                    "contact",
                    &self.contact_state(*curve_contact, 6)?,
                );
                match relation {
                    DocumentCurveDirectionRelation::Tangent { orientation } => {
                        field(&mut fields, "relation", json_string("tangent"));
                        field(
                            &mut fields,
                            "orientation",
                            json_string(tangent_name(*orientation)),
                        );
                    }
                    DocumentCurveDirectionRelation::Normal { side } => {
                        field(&mut fields, "relation", json_string("normal"));
                        field(&mut fields, "side", json_string(normal_side_name(*side)));
                    }
                }
                "curveDirection"
            }
            C::EqualCurvature {
                first_contact,
                second_contact,
                relation,
            } => {
                contact_span_pair(self, &mut fields, *first_contact, *second_contact)?;
                paired_contacts(self, &mut fields, *first_contact, *second_contact)?;
                field(
                    &mut fields,
                    "relation",
                    json_string(match relation {
                        DocumentCurveCurvatureRelation::Signed => "signed",
                        DocumentCurveCurvatureRelation::MagnitudeSameSign => "magnitudeSameSign",
                        DocumentCurveCurvatureRelation::MagnitudeOppositeSign => {
                            "magnitudeOppositeSign"
                        }
                    }),
                );
                "equalCurvature"
            }
            C::EndpointContinuity {
                first_contact,
                second_contact,
                continuity,
            } => {
                contact_span_pair(self, &mut fields, *first_contact, *second_contact)?;
                paired_contacts(self, &mut fields, *first_contact, *second_contact)?;
                let (name, ratio) = match continuity {
                    DocumentCurveContinuity::G0 => ("g0", None),
                    DocumentCurveContinuity::G1 => ("g1", None),
                    DocumentCurveContinuity::G2 => ("g2", None),
                    DocumentCurveContinuity::ParametricC2 {
                        first_rate,
                        second_rate,
                    } => ("parametricC2", Some(first_rate / second_rate)),
                };
                field(&mut fields, "continuity", json_string(name));
                if let Some(ratio) = ratio {
                    finite_number(ratio, "constraint", constraint.id.to_string())?;
                    field(&mut fields, "parameterRatio", number(ratio));
                }
                "endpointContinuity"
            }
            C::LineLineFillet {
                arc,
                first_contact,
                first_side,
                second_contact,
                second_side,
                endpoint_order,
            } => {
                fillet_fields(
                    self,
                    &mut fields,
                    *arc,
                    *first_contact,
                    *first_side,
                    *second_contact,
                    *second_side,
                    *endpoint_order,
                )?;
                "lineLineFillet"
            }
            C::CurveCurveFillet {
                arc,
                first_contact,
                first_side,
                first_trim_endpoint,
                second_contact,
                second_side,
                second_trim_endpoint,
                endpoint_order,
            } => {
                fillet_fields(
                    self,
                    &mut fields,
                    *arc,
                    *first_contact,
                    *first_side,
                    *second_contact,
                    *second_side,
                    *endpoint_order,
                )?;
                field(
                    &mut fields,
                    "firstTrimEndpoint",
                    json_string(trim_endpoint_name(*first_trim_endpoint)),
                );
                field(
                    &mut fields,
                    "secondTrimEndpoint",
                    json_string(trim_endpoint_name(*second_trim_endpoint)),
                );
                "curveCurveFillet"
            }
        };
        Ok((method, fields))
    }

    fn emit_dimension(
        &mut self,
        dimension: &DocumentDimension,
    ) -> Result<(), ManagedSketchExportError> {
        let symbol = self.dimension_symbols[&dimension.id].clone();
        let (method, mut fields) = self.dimension_fields(dimension)?;
        field(&mut fields, "label", json_string(&dimension.label));
        field(
            &mut fields,
            "mode",
            json_string(match dimension.mode {
                DocumentDimensionMode::Driving => "driving",
                DocumentDimensionMode::Reference => "reference",
            }),
        );
        if dimension.suppressed {
            field(&mut fields, "suppressed", "true");
        }
        writeln!(
            self.source,
            "  const {symbol} = $.dimension.{method}({symbol_json}, {{",
            symbol_json = json_string(&symbol),
        )
        .expect("writing managed source to a String cannot fail");
        self.source.push_str(&fields);
        self.source.push_str("  });\n");
        Ok(())
    }

    fn dimension_fields(
        &self,
        dimension: &DocumentDimension,
    ) -> Result<(&'static str, String), ManagedSketchExportError> {
        use DocumentDimensionDefinition as D;
        let mut fields = String::new();
        let method = match &dimension.definition {
            D::PointDistance {
                first,
                second,
                target,
            } => {
                point_pair(self, &mut fields, *first, *second)?;
                field(&mut fields, "value", &self.length_scalar(*target)?);
                "pointDistance"
            }
            D::CurveLength { curve, target } => {
                field(&mut fields, "curve", &self.span_ref(*curve)?);
                field(&mut fields, "value", &self.length_scalar(*target)?);
                "curveLength"
            }
            D::Radius { curve, target } => {
                field(&mut fields, "curve", &self.curve_ref(*curve)?);
                field(&mut fields, "value", &self.length_scalar(*target)?);
                "radius"
            }
            D::Diameter { curve, target } => {
                field(&mut fields, "curve", &self.curve_ref(*curve)?);
                field(&mut fields, "value", &self.length_scalar(*target)?);
                "diameter"
            }
            D::OrientedAngle {
                first,
                second,
                target,
                orientation,
            } => {
                span_pair(self, &mut fields, *first, *second)?;
                field(&mut fields, "value", &self.angle_scalar(*target)?);
                field(
                    &mut fields,
                    "orientation",
                    json_string(match orientation {
                        DocumentAngleOrientation::CounterClockwise => "counterClockwise",
                        DocumentAngleOrientation::Clockwise => "clockwise",
                    }),
                );
                "orientedAngle"
            }
            D::SupportingLineOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => {
                offset_fields(
                    self,
                    &mut fields,
                    *source,
                    *target_segment,
                    *target,
                    *side,
                    *orientation,
                )?;
                "supportingLineOffset"
            }
            D::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            } => {
                offset_fields(
                    self,
                    &mut fields,
                    *source,
                    *target_segment,
                    *target,
                    *side,
                    *orientation,
                )?;
                "exactTranslatedSegmentOffset"
            }
            D::ProfileOffset { .. } => {
                return Err(ManagedSketchExportError::ProfileOffsetDimension {
                    id: dimension.id.to_string(),
                });
            }
        };
        Ok((method, fields))
    }

    fn emit_groups(&mut self) {
        let points = self.point_symbols.values().cloned().collect::<Vec<_>>();
        let curves = self.curve_symbols.values().cloned().collect::<Vec<_>>();
        let constraints = self
            .document
            .sources()
            .filter_map(|source| match source.owner {
                geosolve_sketch::DocumentSourceOwner::Constraint(id) => {
                    self.constraint_symbols.get(&id).cloned()
                }
                geosolve_sketch::DocumentSourceOwner::Dimension(_) => None,
            })
            .collect::<Vec<_>>();
        let dimensions = self
            .document
            .sources()
            .filter_map(|source| match source.owner {
                geosolve_sketch::DocumentSourceOwner::Dimension(id) => {
                    self.dimension_symbols.get(&id).cloned()
                }
                geosolve_sketch::DocumentSourceOwner::Constraint(_) => None,
            })
            .collect::<Vec<_>>();
        emit_group(&mut self.source, "Points", &points);
        emit_group(&mut self.source, "Geometry", &curves);
        emit_group(&mut self.source, "Constraints", &constraints);
        emit_group(&mut self.source, "Dimensions", &dimensions);
    }

    fn point_ref(&self, id: DesignPointId) -> Result<String, ManagedSketchExportError> {
        self.point_symbols
            .get(&id)
            .map(|symbol| format!("{symbol}.point"))
            .ok_or_else(|| missing("point", &id))
    }

    fn curve_ref(&self, id: CurveId) -> Result<String, ManagedSketchExportError> {
        self.curve_symbols
            .get(&id)
            .map(|symbol| format!("{symbol}.curve"))
            .ok_or_else(|| missing("curve", &id))
    }

    fn span_ref(&self, span: CurveSpan) -> Result<String, ManagedSketchExportError> {
        let symbol = self
            .curve_symbols
            .get(&span.curve)
            .ok_or_else(|| missing("curve", &span.curve))?;
        let curve = self
            .document
            .curve(span.curve)
            .ok_or_else(|| missing("curve", &span.curve))?;
        match &curve.definition {
            CurveDefinition::Polyline { points, closed, .. } => {
                let count = if *closed {
                    points.len()
                } else {
                    points.len().saturating_sub(1)
                };
                let index = usize::try_from(span.segment)
                    .ok()
                    .filter(|index| *index < count)
                    .ok_or_else(|| missing("polyline span", &span.segment))?;
                Ok(format!(
                    "{symbol}.segments.byKey[{}]",
                    json_string(&polyline_key(index))
                ))
            }
            CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
                let index = span_ids
                    .iter()
                    .position(|candidate| *candidate == span.segment)
                    .ok_or_else(|| missing("spline span", &span.segment))?;
                Ok(format!(
                    "{symbol}.spans.byKey[{}]",
                    json_string(&spline_key(index))
                ))
            }
            _ if span.segment == 0 => Ok(format!("{symbol}.span")),
            _ => Err(missing("curve span", &span.segment)),
        }
    }

    fn contact(
        &self,
        id: ContactId,
    ) -> Result<&geosolve_sketch::ContactSlot, ManagedSketchExportError> {
        self.document
            .contact(id)
            .ok_or_else(|| missing("contact", &id))
    }

    fn contact_state(
        &self,
        id: ContactId,
        nested_indent: usize,
    ) -> Result<String, ManagedSketchExportError> {
        let contact = self.contact(id)?;
        let parameter = self.dimensionless_scalar_number(contact.parameter)?;
        let mut value = String::from("{\n");
        nested_field(&mut value, nested_indent, "parameter", number(parameter));
        nested_field(
            &mut value,
            nested_indent,
            "winding",
            contact.winding.to_string(),
        );
        if matches!(contact.domain, ContactDomain::SupportingLine) {
            nested_field(
                &mut value,
                nested_indent,
                "support",
                json_string("supportingLine"),
            );
        }
        if let Some(range) = contact.admissible_range {
            nested_field(
                &mut value,
                nested_indent,
                "range",
                format!(
                    "{{\n{}lower: {},\n{}upper: {},\n{}}}",
                    " ".repeat(nested_indent + 2),
                    number(range.lower),
                    " ".repeat(nested_indent + 2),
                    number(range.upper),
                    " ".repeat(nested_indent),
                ),
            );
        }
        nested_field(
            &mut value,
            nested_indent,
            "neighborhood",
            neighborhood_literal(contact.neighborhood, nested_indent),
        );
        nested_field(
            &mut value,
            nested_indent,
            "orientation",
            json_string(contact.tangent_orientation.map_or("none", tangent_name)),
        );
        value.push_str(&" ".repeat(nested_indent.saturating_sub(2)));
        value.push('}');
        Ok(value)
    }

    fn length_scalar(&self, id: DesignScalarId) -> Result<String, ManagedSketchExportError> {
        self.require_scalar_unit(id, ScalarUnit::Length)
            .map(|value| unit("mm", value))
    }

    fn angle_scalar(&self, id: DesignScalarId) -> Result<String, ManagedSketchExportError> {
        self.require_scalar_unit(id, ScalarUnit::Angle)
            .map(|value| unit("rad", value))
    }

    fn dimensionless_scalar(&self, id: DesignScalarId) -> Result<String, ManagedSketchExportError> {
        self.dimensionless_scalar_number(id).map(number)
    }

    fn dimensionless_scalar_number(
        &self,
        id: DesignScalarId,
    ) -> Result<f64, ManagedSketchExportError> {
        let scalar = self
            .document
            .scalar(id)
            .ok_or_else(|| missing("scalar", &id))?;
        if !scalar.value.is_finite() {
            return Err(non_finite("scalar", &id));
        }
        if scalar.unit == ScalarUnit::Length {
            return Err(ManagedSketchExportError::InvalidDocument(format!(
                "scalar {id} is a length where a dimensionless value is required"
            )));
        }
        Ok(scalar.value)
    }

    fn require_scalar_unit(
        &self,
        id: DesignScalarId,
        unit: ScalarUnit,
    ) -> Result<f64, ManagedSketchExportError> {
        let scalar = self
            .document
            .scalar(id)
            .ok_or_else(|| missing("scalar", &id))?;
        if !scalar.value.is_finite() {
            return Err(non_finite("scalar", &id));
        }
        if scalar.unit != unit {
            return Err(ManagedSketchExportError::InvalidDocument(format!(
                "scalar {id} has unit {:?}, expected {unit:?}",
                scalar.unit
            )));
        }
        Ok(scalar.value)
    }

    fn curve_endpoint(
        &self,
        curve: CurveId,
        parameter: f64,
    ) -> Result<[f64; 2], ManagedSketchExportError> {
        let position = self
            .document
            .evaluate_curve_jet(CurveSpan::line(curve), parameter)
            .map_err(|error| ManagedSketchExportError::InvalidDocument(error.to_string()))?
            .position;
        let result = [position.x, position.y];
        finite_pair(result, "curve", curve.to_string())?;
        Ok(result)
    }

    fn minor_axis_point(
        &self,
        center: DesignPointId,
        major: DesignPointId,
        ratio: DesignScalarId,
    ) -> Result<[f64; 2], ManagedSketchExportError> {
        let center = self
            .document
            .point(center)
            .ok_or_else(|| missing("point", &center))?
            .position;
        let major = self
            .document
            .point(major)
            .ok_or_else(|| missing("point", &major))?
            .position;
        let ratio = self.dimensionless_scalar_number(ratio)?;
        let vector = [major[0] - center[0], major[1] - center[1]];
        let result = [center[0] - vector[1] * ratio, center[1] + vector[0] * ratio];
        finite_pair(result, "ellipse minor axis", "derived".into())?;
        Ok(result)
    }
}

fn point_pair(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    first: DesignPointId,
    second: DesignPointId,
) -> Result<(), ManagedSketchExportError> {
    field(fields, "first", &exporter.point_ref(first)?);
    field(fields, "second", &exporter.point_ref(second)?);
    Ok(())
}

fn span_pair(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    first: CurveSpan,
    second: CurveSpan,
) -> Result<(), ManagedSketchExportError> {
    field(fields, "first", &exporter.span_ref(first)?);
    field(fields, "second", &exporter.span_ref(second)?);
    Ok(())
}

fn contact_span_pair(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    first: ContactId,
    second: ContactId,
) -> Result<(), ManagedSketchExportError> {
    span_pair(
        exporter,
        fields,
        exporter.contact(first)?.curve,
        exporter.contact(second)?.curve,
    )
}

fn paired_contacts(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    first: ContactId,
    second: ContactId,
) -> Result<(), ManagedSketchExportError> {
    let value = format!(
        "{{\n      first: {},\n      second: {},\n    }}",
        exporter.contact_state(first, 8)?,
        exporter.contact_state(second, 8)?,
    );
    field(fields, "contacts", &value);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fillet_fields(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    arc: CurveId,
    first: ContactId,
    first_side: DocumentCurveNormalSide,
    second: ContactId,
    second_side: DocumentCurveNormalSide,
    endpoint_order: DocumentFilletEndpointOrder,
) -> Result<(), ManagedSketchExportError> {
    field(fields, "fillet", &exporter.curve_ref(arc)?);
    contact_span_pair(exporter, fields, first, second)?;
    paired_contacts(exporter, fields, first, second)?;
    field(
        fields,
        "firstSide",
        json_string(normal_side_name(first_side)),
    );
    field(
        fields,
        "secondSide",
        json_string(normal_side_name(second_side)),
    );
    field(
        fields,
        "endpointOrder",
        json_string(match endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => "firstThenSecond",
            DocumentFilletEndpointOrder::SecondThenFirst => "secondThenFirst",
        }),
    );
    Ok(())
}

fn offset_fields(
    exporter: &DocumentExporter<'_>,
    fields: &mut String,
    source: CurveSpan,
    target: CurveSpan,
    value: DesignScalarId,
    side: DocumentLineSide,
    orientation: DocumentLineOffsetOrientation,
) -> Result<(), ManagedSketchExportError> {
    span_pair(exporter, fields, source, target)?;
    field(fields, "value", &exporter.length_scalar(value)?);
    field(fields, "side", json_string(side_name(side)));
    field(
        fields,
        "orientation",
        json_string(match orientation {
            DocumentLineOffsetOrientation::Same => "same",
            DocumentLineOffsetOrientation::Reversed => "reversed",
        }),
    );
    Ok(())
}

fn presentation_fields(fields: &mut String, label: &str, role: &str) {
    field(fields, "label", json_string(label));
    field(fields, "role", json_string(role));
}

fn emit_group(source: &mut String, name: &str, symbols: &[String]) {
    if symbols.is_empty() {
        return;
    }
    writeln!(
        source,
        "  $.group({}, [{}]);",
        json_string(name),
        symbols.join(", ")
    )
    .expect("writing managed source to a String cannot fail");
}

fn field(fields: &mut String, name: &str, value: impl AsRef<str>) {
    writeln!(fields, "    {name}: {},", value.as_ref())
        .expect("writing managed source to a String cannot fail");
}

fn nested_field(fields: &mut String, indent: usize, name: &str, value: impl AsRef<str>) {
    writeln!(fields, "{}{name}: {},", " ".repeat(indent), value.as_ref())
        .expect("writing managed source to a String cannot fail");
}

fn source_identifier(prefix: &str, ordinal: usize, label: &str) -> String {
    let mut result = format!("{prefix}{ordinal}");
    let mut uppercase = true;
    for character in label.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase {
                result.extend(character.to_uppercase());
                uppercase = false;
            } else {
                result.extend(character.to_lowercase());
            }
        } else {
            uppercase = true;
        }
        if result.len() >= 96 {
            break;
        }
    }
    result
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string as JSON cannot fail")
}

fn number(value: f64) -> String {
    let encoded = serde_json::Number::from_f64(value)
        .expect("all source numbers are checked finite")
        .to_string();
    if let Some(integer) = encoded.strip_suffix(".0") {
        integer.to_owned()
    } else {
        encoded
    }
}

fn unit(name: &str, value: f64) -> String {
    format!("{name}({})", number(value))
}

fn point_literal(value: [f64; 2]) -> String {
    format!("[{}, {}]", number(value[0]), number(value[1]))
}

const fn bool_literal(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn finite(value: [f64; 2]) -> bool {
    value.into_iter().all(f64::is_finite)
}

fn finite_pair(
    value: [f64; 2],
    kind: &'static str,
    id: String,
) -> Result<(), ManagedSketchExportError> {
    if finite(value) {
        Ok(())
    } else {
        Err(ManagedSketchExportError::NonFinite { kind, id })
    }
}

fn finite_number(
    value: f64,
    kind: &'static str,
    id: String,
) -> Result<(), ManagedSketchExportError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ManagedSketchExportError::NonFinite { kind, id })
    }
}

fn missing(kind: &'static str, id: &impl ToString) -> ManagedSketchExportError {
    ManagedSketchExportError::MissingObject {
        kind,
        id: id.to_string(),
    }
}

fn non_finite(kind: &'static str, id: &impl ToString) -> ManagedSketchExportError {
    ManagedSketchExportError::NonFinite {
        kind,
        id: id.to_string(),
    }
}

const fn axis_name(axis: DocumentCoordinateAxis) -> &'static str {
    match axis {
        DocumentCoordinateAxis::X => "x",
        DocumentCoordinateAxis::Y => "y",
    }
}

const fn direction_name(direction: DocumentDirectionSense) -> &'static str {
    match direction {
        DocumentDirectionSense::Forward => "forward",
        DocumentDirectionSense::Reverse => "reverse",
    }
}

const fn side_name(side: DocumentLineSide) -> &'static str {
    match side {
        DocumentLineSide::Left => "left",
        DocumentLineSide::Right => "right",
    }
}

const fn normal_side_name(side: DocumentCurveNormalSide) -> &'static str {
    match side {
        DocumentCurveNormalSide::Left => "left",
        DocumentCurveNormalSide::Right => "right",
    }
}

const fn endpoint_name(endpoint: FeatureEndpoint) -> &'static str {
    match endpoint {
        FeatureEndpoint::Start => "start",
        FeatureEndpoint::End => "end",
    }
}

const fn trim_endpoint_name(endpoint: DocumentFilletTrimEndpoint) -> &'static str {
    match endpoint {
        DocumentFilletTrimEndpoint::Start => "start",
        DocumentFilletTrimEndpoint::End => "end",
    }
}

const fn sweep_name(sweep: DocumentArcSweep) -> &'static str {
    match sweep {
        DocumentArcSweep::CounterClockwise => "counterClockwise",
        DocumentArcSweep::Clockwise => "clockwise",
    }
}

const fn tangent_name(orientation: TangentOrientation) -> &'static str {
    match orientation {
        TangentOrientation::Aligned => "aligned",
        TangentOrientation::Opposed => "opposed",
    }
}

const fn circle_mode(mode: DocumentCircleTangencyMode) -> &'static str {
    match mode {
        DocumentCircleTangencyMode::External => "external",
        DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::FirstContainsSecond,
        } => "firstContainsSecond",
        DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::SecondContainsFirst,
        } => "secondContainsFirst",
    }
}

fn neighborhood_literal(neighborhood: ContactNeighborhood, indent: usize) -> String {
    let child = " ".repeat(indent + 2);
    let closing = " ".repeat(indent);
    match neighborhood {
        ContactNeighborhood::Interior => {
            format!("{{\n{child}kind: \"interior\",\n{closing}}}")
        }
        ContactNeighborhood::Start => {
            format!("{{\n{child}kind: \"start\",\n{closing}}}")
        }
        ContactNeighborhood::End => format!("{{\n{child}kind: \"end\",\n{closing}}}"),
        ContactNeighborhood::Local { lower, upper } => format!(
            "{{\n{child}kind: \"local\",\n{child}lower: {},\n{child}upper: {},\n{closing}}}",
            number(lower),
            number(upper)
        ),
    }
}

const fn nurbs_method(form: DocumentBSplineForm) -> &'static str {
    match form {
        DocumentBSplineForm::Clamped => "openControlNurbs",
        DocumentBSplineForm::Periodic => "periodicControlNurbs",
    }
}

fn polyline_key(index: usize) -> String {
    format!("vertex{}", index + 1)
}

fn spline_key(index: usize) -> String {
    format!("control{}", index + 1)
}

fn canonical_knots(form: DocumentBSplineForm, degree: usize, controls: usize) -> Option<Vec<f64>> {
    if degree == 0 || controls <= degree {
        return None;
    }
    match form {
        DocumentBSplineForm::Clamped => {
            let spans = controls - degree;
            let finite_spans = u32::try_from(spans).ok()?;
            let mut knots = vec![0.0; degree + 1];
            for index in 1..finite_spans {
                knots.push(f64::from(index));
            }
            knots.extend(std::iter::repeat_n(f64::from(finite_spans), degree + 1));
            Some(knots)
        }
        DocumentBSplineForm::Periodic => {
            let finite_controls = u32::try_from(controls).ok()?;
            Some((0..=finite_controls).map(f64::from).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        ContactAdmissibleRange, ContactAdmissibleRangeEdit, CurveDefinition, CurveSpan,
        DocumentConstraintDefinition, DocumentElementId, DocumentParameterKind, GeometryRole,
        SketchDocument,
    };

    use super::{ManagedSketchExportError, export_sketch_document_to_managed_source};

    #[test]
    fn projection_is_deterministic_and_keeps_shared_topology_contact_range_role_and_suppression() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document
            .add_point("shared start", [-2.0, 0.0])
            .expect("start");
        let end = document.add_point("line end", [2.0, 0.0]).expect("end");
        let probe = document.add_point("probe", [0.0, 0.0]).expect("probe");
        let line = document
            .add_curve_with_role(
                "construction support",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
                GeometryRole::Construction,
            )
            .expect("line");
        let contact = document
            .add_curve_contact_with_domain(
                "support contact",
                CurveSpan::line(line),
                geosolve_sketch::ContactDomain::SupportingLine,
                0.25,
                0,
                geosolve_sketch::ContactNeighborhood::Local {
                    lower: -1.0,
                    upper: 1.0,
                },
                None,
            )
            .expect("contact");
        let constraint = document
            .add_constraint(
                "probe on support",
                DocumentConstraintDefinition::PointOnCurve {
                    point: probe,
                    contact,
                },
            )
            .expect("constraint");
        document
            .set_contact_admissible_ranges(&[ContactAdmissibleRangeEdit {
                contact,
                range: Some(ContactAdmissibleRange {
                    lower: -0.5,
                    upper: 0.5,
                }),
            }])
            .expect("range");
        document
            .set_element_user_suppressed(DocumentElementId::Constraint(constraint), true)
            .expect("suppress constraint");

        let first = export_sketch_document_to_managed_source(&document).expect("first export");
        let second = export_sketch_document_to_managed_source(&document).expect("second export");

        assert_eq!(first, second);
        assert!(first.contains("start: point1SharedStart.point,"));
        assert!(first.contains("end: point2LineEnd.point,"));
        assert!(first.contains("role: \"construction\","));
        assert!(first.contains("point: point3Probe.point,"));
        assert!(first.contains("curve: curve1ConstructionSupport.span,"));
        assert!(first.contains("support: \"supportingLine\","));
        assert!(first.contains("lower: -0.5,\n        upper: 0.5,"));
        assert!(first.contains("suppressed: true,"));
        assert!(!first.contains("domain:"));
    }

    #[test]
    fn projection_fails_closed_for_host_owned_authority() {
        let mut document = SketchDocument::new(1.0).expect("document");
        document
            .add_parameter("host length", DocumentParameterKind::Length)
            .expect("parameter");

        assert_eq!(
            export_sketch_document_to_managed_source(&document),
            Err(ManagedSketchExportError::HostAuthority),
        );
    }
}
