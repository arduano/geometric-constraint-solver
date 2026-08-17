// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationWorkCounter, ResidualCategory, SolverConfig,
};
use geosolve_geometry::Point2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{
    AngleOrientation, ContactNeighborhood, CurveDefinition, CurveId, DesignPointId, DesignScalarId,
    DocumentAngleOrientation, DocumentBSplineForm, DocumentCoordinateAxis, DocumentCurveNormalSide,
    DocumentCurveSpanRef, DocumentError, DocumentId, DocumentLineSide, DocumentLineSupportRef,
    DocumentSourceId, EffectiveActivity, ScalarDomain, ScalarUnit, SketchDocument,
    SketchSolveRequest, SketchSolveResult,
};

/// One explicitly oriented and unwrapped angle between directed line supports.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentAngleOperand {
    pub first: DocumentLineSupportRef,
    pub second: DocumentLineSupportRef,
    pub orientation: DocumentAngleOrientation,
    pub winding: i32,
}

/// Closed M37 standard planar relation vocabulary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentPlanarRelation {
    Concentric {
        first: crate::DocumentCenterRef,
        second: crate::DocumentCenterRef,
    },
    Collinear {
        first: DocumentLineSupportRef,
        second: DocumentLineSupportRef,
    },
    HorizontalPoints {
        first: crate::DocumentPointRef,
        second: crate::DocumentPointRef,
    },
    VerticalPoints {
        first: crate::DocumentPointRef,
        second: crate::DocumentPointRef,
    },
    PointSymmetry {
        first: crate::DocumentPointRef,
        second: crate::DocumentPointRef,
        center: crate::DocumentPointRef,
    },
    EntitySymmetry {
        first_entity: CurveId,
        second_entity: CurveId,
        point_pairs: Vec<[crate::DocumentPointRef; 2]>,
        scalar_pairs: Vec<[DesignScalarId; 2]>,
        axis: DocumentLineSupportRef,
    },
    EqualCircularRadius {
        first: CurveId,
        second: CurveId,
    },
    EqualDistance {
        first: [crate::DocumentPointRef; 2],
        second: [crate::DocumentPointRef; 2],
    },
    EqualAngle {
        first: DocumentAngleOperand,
        second: DocumentAngleOperand,
    },
    BlockEntity {
        curve: CurveId,
        captured_definition: CurveDefinition,
        captured_points: Vec<(DesignPointId, [f64; 2])>,
        captured_scalars: Vec<(DesignScalarId, f64)>,
    },
}

/// One separately persisted M37 relation source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentPlanarSource {
    document_id: DocumentId,
    source_id: DocumentSourceId,
    label: String,
    relation: DocumentPlanarRelation,
}

impl DocumentPlanarSource {
    #[must_use]
    pub const fn source_id(&self) -> DocumentSourceId {
        self.source_id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn relation(&self) -> &DocumentPlanarRelation {
        &self.relation
    }
}

/// One grouped source-level audit descriptor for executable M37 rows.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPlanarAudit {
    pub source_id: DocumentSourceId,
    pub source_label: String,
    pub relation: DocumentPlanarRelation,
    pub equation_templates: Vec<&'static str>,
    pub rows: Vec<geosolve_core::AuditRowSnapshot>,
}

/// Independently accepted document and solver evidence for one catalog solve.
#[derive(Clone, Debug)]
pub struct DocumentSemanticSolveResult {
    pub document: SketchDocument,
    pub solve_result: SketchSolveResult,
    pub audit: Vec<DocumentPlanarAudit>,
    pub scalar_audit: Vec<LoweredDocumentScalarSource>,
}

/// Retained accepted lifecycle for one document plus its separate M36/M37 catalog.
#[derive(Clone, Debug)]
pub struct DocumentSemanticCatalogSession {
    catalog: DocumentSemanticSourceCatalog,
    accepted: DocumentSemanticSolveResult,
    request: SketchSolveRequest,
    config: SolverConfig,
    revision: u64,
}

impl DocumentSemanticCatalogSession {
    /// Builds an independently accepted catalog-backed session.
    ///
    /// # Errors
    /// Returns a document, lowering, solve, or independent-validation error.
    pub fn new(
        document: &SketchDocument,
        catalog: DocumentSemanticSourceCatalog,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, DocumentError> {
        let accepted = catalog.solve_document(document, request, config)?;
        Ok(Self {
            catalog,
            accepted,
            request,
            config,
            revision: 0,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn accepted(&self) -> &DocumentSemanticSolveResult {
        &self.accepted
    }

    #[must_use]
    pub const fn catalog(&self) -> &DocumentSemanticSourceCatalog {
        &self.catalog
    }

    /// Solves a candidate on scratch state and swaps only a freshly accepted result.
    ///
    /// # Errors
    /// Returns a stale revision, document, solve, or independent-validation error.
    pub fn replace_document(
        &mut self,
        expected_revision: u64,
        document: &SketchDocument,
    ) -> Result<&DocumentSemanticSolveResult, DocumentError> {
        if expected_revision != self.revision {
            return invalid("semantic session revision", "candidate revision is stale");
        }
        let accepted = self
            .catalog
            .solve_document(document, self.request, self.config)?;
        self.accepted = accepted;
        self.revision = self.revision.saturating_add(1);
        Ok(&self.accepted)
    }
}

/// Complete explicit seed for one constructor-owned latent curve contact.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentContactSeed {
    pub support: DocumentCurveSpanRef,
    pub parameter: f64,
    pub neighborhood: ContactNeighborhood,
}

/// Persistent identities atomically allocated by a high-level relation constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentConstructedRelation {
    pub constraint: crate::DocumentConstraintId,
    pub contacts: Vec<crate::ContactId>,
}

/// Explicit branch-complete seed for specialized line/circle tangency.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentLineCircleTangentRequest {
    pub line: DocumentContactSeed,
    pub circle: DocumentContactSeed,
    pub side: crate::DocumentLineSide,
    pub orientation: crate::TangentOrientation,
}

/// Explicit branch-complete seed for specialized circle/arc tangency.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCircleArcTangentRequest {
    pub circle: DocumentContactSeed,
    pub arc: DocumentContactSeed,
    pub side: crate::DocumentArcTangencySide,
    pub orientation: crate::TangentOrientation,
}

impl SketchDocument {
    /// Resolves one whole-curve semantic endpoint to its exact executable contact seed.
    ///
    /// Start selects the first semantic span at parameter `0`; End selects the last span at
    /// parameter `1`. Closed and periodic topology has no endpoint and is rejected through the
    /// same capability validation as [`SketchDocument::validate_endpoint_ref`].
    ///
    /// # Errors
    /// Returns an error for a missing curve, closed or periodic topology, or a bounded curve with
    /// no executable span.
    pub fn curve_endpoint_contact_seed(
        &self,
        endpoint: crate::DocumentEndpointRef,
    ) -> Result<DocumentContactSeed, DocumentError> {
        self.validate_endpoint_ref(endpoint)?;
        let spans = self.curve_spans(endpoint.curve)?;
        let (span, parameter, neighborhood) = match endpoint.endpoint {
            crate::FeatureEndpoint::Start => {
                (spans.first().copied(), 0.0, ContactNeighborhood::Start)
            }
            crate::FeatureEndpoint::End => (spans.last().copied(), 1.0, ContactNeighborhood::End),
        };
        let span = span.ok_or_else(|| DocumentError::InvalidField {
            field: "feature endpoint",
            message: "bounded endpoint curve has no executable span".into(),
        })?;
        Ok(DocumentContactSeed {
            support: DocumentCurveSpanRef { span, winding: 0 },
            parameter,
            neighborhood,
        })
    }

    /// Validates the deliberately narrow dimensionless host-parameter target set.
    pub(crate) fn validate_dimensionless_parameter_property(
        &self,
        property: DocumentScalarPropertyRef,
    ) -> Result<(), DocumentError> {
        self.validate_scalar_property_ref(property)?;
        if property.unit != DocumentScalarUnit::Dimensionless
            || property.branch != DocumentScalarBranch::Dimensionless
        {
            return invalid(
                "parameter binding",
                "dimensionless targets cannot reinterpret unit or branch semantics",
            );
        }
        self.validate_parameter_scalar_value(
            property.scalar,
            scalar_value(self, property.scalar)?,
        )?;
        if !is_dimensionless_runtime_curve_scalar(self, property.scalar) {
            return invalid(
                "parameter binding",
                "scalar is not an executable dimensionless curve property",
            );
        }
        Ok(())
    }

    pub(crate) fn dimensionless_parameter_target_is_active(
        &self,
        property: DocumentScalarPropertyRef,
        activity: &EffectiveActivity,
    ) -> bool {
        self.curves().iter().any(|curve| {
            activity.is_active(curve.id)
                && curve_has_dimensionless_runtime_scalar(&curve.definition, property.scalar)
        })
    }

    /// Atomically allocates two explicit latent contacts and their common-jet contact source.
    ///
    /// # Errors
    /// Returns an error for malformed seeds, domains, neighborhoods, or references.
    pub fn add_curve_curve_contact_relation(
        &mut self,
        label: &str,
        first: DocumentContactSeed,
        second: DocumentContactSeed,
    ) -> Result<DocumentConstructedRelation, DocumentError> {
        self.add_curve_pair_relation(label, first, second, None)
    }

    /// Atomically allocates two explicit latent contacts and an oriented common-jet tangent source.
    ///
    /// # Errors
    /// Returns an error for malformed seeds, domains, neighborhoods, or references.
    pub fn add_curve_curve_tangent_relation(
        &mut self,
        label: &str,
        first: DocumentContactSeed,
        second: DocumentContactSeed,
        orientation: crate::TangentOrientation,
    ) -> Result<DocumentConstructedRelation, DocumentError> {
        self.add_curve_pair_relation(label, first, second, Some(orientation))
    }

    /// Atomically allocates explicit line/circle contacts and their selected side branch.
    ///
    /// # Errors
    /// Returns an error for wrong curve families or malformed latent branch state.
    pub fn add_line_circle_tangent_relation(
        &mut self,
        label: &str,
        request: DocumentLineCircleTangentRequest,
    ) -> Result<DocumentConstructedRelation, DocumentError> {
        self.validate_line_support_ref(DocumentLineSupportRef {
            span: request.line.support.span,
            direction: crate::DocumentDirectionSense::Forward,
        })?;
        if !matches!(
            self.curve(request.circle.support.span.curve)
                .map(|curve| &curve.definition),
            Some(CurveDefinition::Circle { .. })
        ) {
            return invalid(
                "line-circle tangent",
                "second operand must be a full circle",
            );
        }
        let mut candidate = self.clone();
        let line_contact = candidate.add_curve_contact(
            format!("{label} line contact"),
            request.line.support.span,
            request.line.parameter,
            request.line.support.winding,
            request.line.neighborhood,
            Some(request.orientation),
        )?;
        let circle_contact = candidate.add_curve_contact(
            format!("{label} circle contact"),
            request.circle.support.span,
            request.circle.parameter,
            request.circle.support.winding,
            request.circle.neighborhood,
            Some(request.orientation),
        )?;
        let constraint = candidate.add_constraint(
            label,
            crate::DocumentConstraintDefinition::LineCircleTangency {
                line_contact,
                circle_contact,
                side: request.side,
            },
        )?;
        *self = candidate;
        Ok(DocumentConstructedRelation {
            constraint,
            contacts: vec![line_contact, circle_contact],
        })
    }

    /// Atomically allocates explicit circle/arc contacts and their radial-side branch.
    ///
    /// # Errors
    /// Returns an error for wrong curve families or malformed latent branch state.
    pub fn add_circle_arc_tangent_relation(
        &mut self,
        label: &str,
        request: DocumentCircleArcTangentRequest,
    ) -> Result<DocumentConstructedRelation, DocumentError> {
        if !matches!(
            self.curve(request.circle.support.span.curve)
                .map(|curve| &curve.definition),
            Some(CurveDefinition::Circle { .. })
        ) || !matches!(
            self.curve(request.arc.support.span.curve)
                .map(|curve| &curve.definition),
            Some(CurveDefinition::CircularArc { .. })
        ) {
            return invalid(
                "circle-arc tangent",
                "operands must be one full circle and one circular arc",
            );
        }
        let mut candidate = self.clone();
        let circle_contact = candidate.add_curve_contact(
            format!("{label} circle contact"),
            request.circle.support.span,
            request.circle.parameter,
            request.circle.support.winding,
            request.circle.neighborhood,
            Some(request.orientation),
        )?;
        let arc_contact = candidate.add_curve_contact(
            format!("{label} arc contact"),
            request.arc.support.span,
            request.arc.parameter,
            request.arc.support.winding,
            request.arc.neighborhood,
            Some(request.orientation),
        )?;
        let constraint = candidate.add_constraint(
            label,
            crate::DocumentConstraintDefinition::CircleArcTangency {
                circle_contact,
                arc_contact,
                side: request.side,
            },
        )?;
        *self = candidate;
        Ok(DocumentConstructedRelation {
            constraint,
            contacts: vec![circle_contact, arc_contact],
        })
    }

    /// Adds circle/circle tangency with explicit containment and center-direction state.
    ///
    /// # Errors
    /// Returns an error for wrong families, invalid direction, or repeated operands.
    pub fn add_circle_circle_tangent_relation(
        &mut self,
        label: &str,
        first: CurveId,
        second: CurveId,
        mode: crate::DocumentCircleTangencyMode,
        center_direction: [f64; 2],
    ) -> Result<crate::DocumentConstraintId, DocumentError> {
        self.add_constraint(
            label,
            crate::DocumentConstraintDefinition::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            },
        )
    }

    fn add_curve_pair_relation(
        &mut self,
        label: &str,
        first: DocumentContactSeed,
        second: DocumentContactSeed,
        orientation: Option<crate::TangentOrientation>,
    ) -> Result<DocumentConstructedRelation, DocumentError> {
        validate_label(label)?;
        self.validate_curve_span_ref(first.support)?;
        self.validate_curve_span_ref(second.support)?;
        let first_parameter_bits = first.parameter.to_bits();
        let second_parameter_bits = second.parameter.to_bits();
        let same_parameter = first_parameter_bits == second_parameter_bits
            || (first_parameter_bits << 1 == 0 && second_parameter_bits << 1 == 0);
        if first.support == second.support && same_parameter {
            return invalid(
                "curve relation contacts",
                "two-host contact or tangency requires distinct explicit contacts",
            );
        }
        let mut candidate = self.clone();
        let first_contact = candidate.add_curve_contact(
            format!("{label} first contact"),
            first.support.span,
            first.parameter,
            first.support.winding,
            first.neighborhood,
            orientation,
        )?;
        let second_contact = candidate.add_curve_contact(
            format!("{label} second contact"),
            second.support.span,
            second.parameter,
            second.support.winding,
            second.neighborhood,
            orientation,
        )?;
        let definition = if orientation.is_some() {
            crate::DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            }
        } else {
            crate::DocumentConstraintDefinition::CurveCurveContact {
                first_contact,
                second_contact,
            }
        };
        let constraint = candidate.add_constraint(label, definition)?;
        *self = candidate;
        Ok(DocumentConstructedRelation {
            constraint,
            contacts: vec![first_contact, second_contact],
        })
    }
}

/// Canonical quantity carried by one M36 scalar operand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentScalarUnit {
    Length,
    Angle,
    Dimensionless,
    Curvature,
    Parameter,
}

/// Closed semantic provenance for a signed length.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentSignedLengthProvenance {
    OrderedOperands,
    Axis {
        axis: DocumentCoordinateAxis,
    },
    LineSide {
        support: DocumentLineSupportRef,
        side: DocumentLineSide,
    },
    DatumAxis {
        axis: DocumentCoordinateAxis,
    },
}

/// Explicit unit-specific branch state for one scalar property.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentScalarBranch {
    Unsigned,
    SignedLength {
        provenance: DocumentSignedLengthProvenance,
    },
    Angle {
        orientation: DocumentAngleOrientation,
        winding: i32,
    },
    Dimensionless,
    Curvature {
        signed: bool,
        normal_side: Option<DocumentCurveNormalSide>,
    },
    Parameter {
        support: DocumentCurveSpanRef,
        neighborhood: ContactNeighborhood,
    },
}

/// Persistent typed scalar-property operand.
///
/// The explicit M36 unit is intentionally separate from frozen sketch-v1-to-v4
/// [`ScalarUnit`]. Existing dimensionless curve-shape scalars use the legacy
/// `parameter` storage tag but cannot be consumed here without declaring their
/// dimensionless meaning.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentScalarPropertyRef {
    pub scalar: DesignScalarId,
    pub unit: DocumentScalarUnit,
    pub domain: ScalarDomain,
    pub branch: DocumentScalarBranch,
}

/// Closed fixed/equal scalar relation foundation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentScalarRelation {
    Fixed {
        property: DocumentScalarPropertyRef,
        target: f64,
    },
    Equal {
        first: DocumentScalarPropertyRef,
        second: DocumentScalarPropertyRef,
    },
}

const SEMANTIC_SOURCE_CATALOG_VERSION: u32 = 1;

/// One persistent scalar semantic source owned by an M36 catalog.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentScalarSource {
    document_id: DocumentId,
    source_id: DocumentSourceId,
    label: String,
    relation: DocumentScalarRelation,
}

impl DocumentScalarSource {
    #[must_use]
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    #[must_use]
    pub const fn source_id(&self) -> DocumentSourceId {
        self.source_id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn relation(&self) -> DocumentScalarRelation {
        self.relation
    }
}

/// Separately serialized M36 semantic-source envelope bound to one sketch document.
///
/// Source allocation consumes the owning document's persistent allocator, so later
/// document objects and sibling semantic sources cannot reuse an identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSemanticSourceCatalog {
    version: u32,
    document_id: DocumentId,
    catalog_id: DocumentSourceId,
    sources: Vec<DocumentScalarSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    planar_sources: Vec<DocumentPlanarSource>,
}

/// One named operand in structured scalar audit evidence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentScalarAuditBinding {
    pub role: &'static str,
    pub property: DocumentScalarPropertyRef,
}

/// Complete human-readable descriptor for one semantic scalar source.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentScalarAudit {
    pub document_id: DocumentId,
    pub source_id: DocumentSourceId,
    pub source_label: String,
    pub relation: DocumentScalarRelation,
    pub equation_template: &'static str,
    pub unit: DocumentScalarUnit,
    pub characteristic_scale: f64,
    pub bindings: Vec<DocumentScalarAuditBinding>,
}

/// One deterministic ordinary scalar row emitted by a semantic source.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentScalarRow {
    pub category: ResidualCategory,
    pub raw_value: f64,
    pub normalized_value: f64,
    pub raw_jacobian: Vec<f64>,
    pub normalized_jacobian: Vec<f64>,
    pub characteristic_scale: f64,
    target: f64,
}

impl DocumentScalarRow {
    /// Evaluates the raw row for values in audit-binding order.
    ///
    /// # Errors
    ///
    /// Returns an error for wrong arity, non-finite input, or non-finite output.
    pub fn evaluate_raw(&self, values: &[f64]) -> Result<f64, DocumentError> {
        if values.len() != self.raw_jacobian.len() {
            return invalid(
                "scalar row values",
                "value count must match scalar-row incidence",
            );
        }
        if !values.iter().all(|value| value.is_finite()) {
            return invalid("scalar row values", "all values must be finite");
        }
        let value = self.raw_jacobian.iter().zip(values).try_fold(
            -self.target,
            |sum, (coefficient, value)| {
                let next = sum + coefficient * value;
                next.is_finite()
                    .then_some(next)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "scalar row value",
                        message: "evaluation produced a non-finite value".into(),
                    })
            },
        )?;
        Ok(value)
    }

    /// Evaluates the scale-normalized row for raw scalar values.
    ///
    /// # Errors
    ///
    /// Returns an error for wrong arity, non-finite input, or non-finite output.
    pub fn evaluate_normalized(&self, values: &[f64]) -> Result<f64, DocumentError> {
        let raw = self.evaluate_raw(values)?;
        let normalized = raw / self.characteristic_scale;
        if normalized.is_finite() {
            Ok(normalized)
        } else {
            invalid(
                "normalized scalar row value",
                "evaluation produced a non-finite value",
            )
        }
    }
}

/// Deterministic row and audit lowering for one persistent semantic source.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredDocumentScalarSource {
    pub source_id: DocumentSourceId,
    pub rows: Vec<DocumentScalarRow>,
    pub audit: DocumentScalarAudit,
}

struct ScalarRowDefinition {
    unit: DocumentScalarUnit,
    template: &'static str,
    bindings: Vec<DocumentScalarAuditBinding>,
    values: Vec<f64>,
    raw_jacobian: Vec<f64>,
    target: f64,
}

impl DocumentSemanticSourceCatalog {
    /// Creates and reserves one empty semantic-source catalog for a document.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent document allocator is exhausted.
    pub fn new(document: &mut SketchDocument) -> Result<Self, DocumentError> {
        let catalog_id = document.allocate_semantic_catalog_id()?;
        Ok(Self {
            version: SEMANTIC_SOURCE_CATALOG_VERSION,
            document_id: document.id(),
            catalog_id,
            sources: Vec::new(),
            planar_sources: Vec::new(),
        })
    }

    #[must_use]
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    #[must_use]
    pub const fn catalog_id(&self) -> DocumentSourceId {
        self.catalog_id
    }

    #[must_use]
    pub fn sources(&self) -> &[DocumentScalarSource] {
        &self.sources
    }

    #[must_use]
    pub fn source(&self, id: DocumentSourceId) -> Option<&DocumentScalarSource> {
        self.sources.iter().find(|source| source.source_id == id)
    }

    #[must_use]
    pub fn planar_sources(&self) -> &[DocumentPlanarSource] {
        &self.planar_sources
    }

    #[must_use]
    pub fn planar_source(&self, id: DocumentSourceId) -> Option<&DocumentPlanarSource> {
        self.planar_sources
            .iter()
            .find(|source| source.source_id == id)
    }

    /// Allocates one validated standard planar relation source.
    ///
    /// # Errors
    /// Returns an error for malformed operands, identity exhaustion, or a foreign document.
    pub fn add_planar_source(
        &mut self,
        document: &mut SketchDocument,
        label: impl Into<String>,
        relation: DocumentPlanarRelation,
    ) -> Result<DocumentSourceId, DocumentError> {
        self.validate_header(document)?;
        if self.sources.len().saturating_add(self.planar_sources.len())
            >= crate::MAX_DOCUMENT_OBJECTS
        {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self
                    .sources
                    .len()
                    .saturating_add(self.planar_sources.len())
                    .saturating_add(1),
                limit: crate::MAX_DOCUMENT_OBJECTS,
            });
        }
        let label = label.into();
        validate_label(&label)?;
        validate_planar_relation(document, &relation)?;
        let source_id = document.allocate_semantic_source_id(self.catalog_id)?;
        self.planar_sources.push(DocumentPlanarSource {
            document_id: self.document_id,
            source_id,
            label,
            relation,
        });
        Ok(source_id)
    }

    /// Captures and groups every independent stored point/scalar degree of one entity.
    ///
    /// # Errors
    /// Returns an error for a missing entity, malformed state, or identity exhaustion.
    pub fn add_block_entity_source(
        &mut self,
        document: &mut SketchDocument,
        label: impl Into<String>,
        curve: CurveId,
    ) -> Result<DocumentSourceId, DocumentError> {
        let (captured_points, captured_scalars) = capture_entity(document, curve)?;
        let captured_definition = document
            .curve(curve)
            .ok_or(DocumentError::UnknownId {
                kind: "curve",
                id: curve.0,
            })?
            .definition
            .clone();
        self.add_planar_source(
            document,
            label,
            DocumentPlanarRelation::BlockEntity {
                curve,
                captured_definition,
                captured_points,
                captured_scalars,
            },
        )
    }

    /// Allocates and validates one source through the owning document allocator.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign document, exhausted identity/resource limit,
    /// malformed label, or invalid relation.
    pub fn add_scalar_source(
        &mut self,
        document: &mut SketchDocument,
        label: impl Into<String>,
        relation: DocumentScalarRelation,
    ) -> Result<DocumentSourceId, DocumentError> {
        self.validate_header(document)?;
        if self.sources.len() >= crate::MAX_DOCUMENT_OBJECTS {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self.sources.len() + 1,
                limit: crate::MAX_DOCUMENT_OBJECTS,
            });
        }
        let label = label.into();
        validate_label(&label)?;
        lower_relation(document, relation)?;
        let source_id = document.allocate_semantic_source_id(self.catalog_id)?;
        let source = DocumentScalarSource {
            document_id: self.document_id,
            source_id,
            label,
            relation,
        };
        self.sources.push(source);
        debug_assert!(self.validate(document).is_ok());
        Ok(source_id)
    }

    /// Strictly validates document binding, reservation, identity order, and sources.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed catalog structure, identity reservation,
    /// ordering, document binding, or scalar relation semantics.
    pub fn validate(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        self.validate_header(document)?;
        if self.sources.len().saturating_add(self.planar_sources.len())
            > crate::MAX_DOCUMENT_OBJECTS
        {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self.sources.len().saturating_add(self.planar_sources.len()),
                limit: crate::MAX_DOCUMENT_OBJECTS,
            });
        }
        if document.semantic_reservation_owner(self.catalog_id) != Some(self.catalog_id) {
            return invalid(
                "semantic catalog identity",
                "catalog is not reserved by this document",
            );
        }
        let mut identities = BTreeSet::new();
        let mut previous = None;
        for source in &self.sources {
            if source.document_id != self.document_id {
                return invalid(
                    "semantic source document",
                    "every source must belong to the catalog document",
                );
            }
            if document.element(source.source_id.0).is_some() {
                return invalid(
                    "semantic source identity",
                    "source identity aliases an existing document element",
                );
            }
            if source.source_id.0 >= document.allocator_cursor() {
                return invalid(
                    "semantic source identity",
                    "source identity is not reserved below the document allocator cursor",
                );
            }
            if document.semantic_reservation_owner(source.source_id) != Some(self.catalog_id) {
                return invalid(
                    "semantic source identity",
                    "source is not reserved by its semantic catalog",
                );
            }
            if !identities.insert(source.source_id) {
                return Err(DocumentError::DuplicateId(source.source_id.0));
            }
            if previous.is_some_and(|id| id >= source.source_id) {
                return invalid(
                    "semantic source order",
                    "source identities must be unique and strictly increasing",
                );
            }
            previous = Some(source.source_id);
            validate_label(&source.label)?;
            lower_relation(document, source.relation)?;
        }
        let mut previous_planar = None;
        for source in &self.planar_sources {
            if source.document_id != self.document_id
                || document.element(source.source_id.0).is_some()
                || source.source_id.0 >= document.allocator_cursor()
                || document.semantic_reservation_owner(source.source_id) != Some(self.catalog_id)
            {
                return invalid(
                    "semantic planar source identity",
                    "source must be reserved by this catalog and document",
                );
            }
            if !identities.insert(source.source_id) {
                return Err(DocumentError::DuplicateId(source.source_id.0));
            }
            if previous_planar.is_some_and(|id| id >= source.source_id) {
                return invalid(
                    "semantic planar source order",
                    "source identities must be strictly increasing",
                );
            }
            previous_planar = Some(source.source_id);
            validate_label(&source.label)?;
            validate_planar_relation(document, &source.relation)?;
        }
        Ok(())
    }

    fn validate_header(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        if self.version != SEMANTIC_SOURCE_CATALOG_VERSION {
            return invalid(
                "semantic source catalog version",
                "unsupported semantic source catalog version",
            );
        }
        if self.document_id != document.id() {
            return invalid(
                "semantic source catalog document",
                "catalog belongs to a different sketch document",
            );
        }
        Ok(())
    }

    /// Serializes this separate M36 envelope in deterministic source order.
    ///
    /// # Errors
    ///
    /// Returns an error if the validated in-memory envelope cannot be serialized.
    pub fn to_canonical_json(&self) -> Result<String, DocumentError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Parses and strictly validates a separate M36 envelope against its document.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, foreign, colliding, unordered, or
    /// semantically invalid input.
    pub fn from_json(document: &mut SketchDocument, input: &str) -> Result<Self, DocumentError> {
        if input.len() > crate::MAX_DOCUMENT_JSON_BYTES {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic source JSON bytes",
                actual: input.len(),
                limit: crate::MAX_DOCUMENT_JSON_BYTES,
            });
        }
        let catalog: Self = serde_json::from_str(input)?;
        catalog.validate_unregistered(document)?;
        let source_ids = catalog
            .sources
            .iter()
            .map(|source| source.source_id)
            .chain(catalog.planar_sources.iter().map(|source| source.source_id))
            .collect::<Vec<_>>();
        document.register_semantic_catalog(catalog.catalog_id, &source_ids)?;
        catalog.validate(document)?;
        Ok(catalog)
    }

    fn validate_unregistered(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        self.validate_header(document)?;
        if self.sources.len().saturating_add(self.planar_sources.len())
            > crate::MAX_DOCUMENT_OBJECTS
        {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self.sources.len().saturating_add(self.planar_sources.len()),
                limit: crate::MAX_DOCUMENT_OBJECTS,
            });
        }
        if self.catalog_id.0 >= document.allocator_cursor()
            || document.element(self.catalog_id.0).is_some()
        {
            return invalid(
                "semantic catalog identity",
                "catalog identity is not reserved below the document allocator cursor",
            );
        }
        let mut identities = BTreeSet::new();
        let mut previous = None;
        for source in &self.sources {
            if source.document_id != self.document_id
                || source.source_id.0 >= document.allocator_cursor()
                || document.element(source.source_id.0).is_some()
                || !identities.insert(source.source_id)
                || previous.is_some_and(|id| id >= source.source_id)
            {
                return invalid(
                    "semantic source identity",
                    "sources must be reserved, unique and strictly increasing",
                );
            }
            previous = Some(source.source_id);
            validate_label(&source.label)?;
            lower_relation(document, source.relation)?;
        }
        let mut previous_planar = None;
        for source in &self.planar_sources {
            if source.document_id != self.document_id
                || source.source_id.0 >= document.allocator_cursor()
                || document.element(source.source_id.0).is_some()
                || !identities.insert(source.source_id)
                || previous_planar.is_some_and(|id| id >= source.source_id)
            {
                return invalid(
                    "semantic planar source identity",
                    "sources must be reserved, unique and strictly increasing",
                );
            }
            previous_planar = Some(source.source_id);
            validate_label(&source.label)?;
            validate_planar_relation(document, &source.relation)?;
        }
        Ok(())
    }

    /// Lowers the complete catalog into executable sketch rows, solves, independently
    /// validates, and publishes a projected document only after accepted success.
    ///
    /// # Errors
    /// Returns a validation, lowering, solve, or independent-publication error.
    pub fn solve_document(
        &self,
        document: &SketchDocument,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<DocumentSemanticSolveResult, DocumentError> {
        self.validate(document)?;
        let lowered = document.lower()?;
        let (mut sketch, mappings) = lowered.into_parts();
        let mut pending_audit = Vec::with_capacity(self.planar_sources.len());
        for source in &self.planar_sources {
            let before = runtime_sketch_sources(&sketch).collect::<Vec<_>>();
            let templates = lower_planar_source(document, &mappings, &mut sketch, source)?;
            let sources = runtime_sketch_sources(&sketch)
                .filter(|source| !before.contains(source))
                .collect::<Vec<_>>();
            pending_audit.push((source, templates, sources));
        }
        lower_executable_scalar_sources(document, &mappings, &mut sketch, &self.sources)?;
        let solve_result = sketch.solve(request, config)?;
        if !solve_result.accepted()
            || solve_result
                .acceptance_hard_residual_max
                .is_none_or(|value| {
                    !value.is_finite() || value > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
                })
        {
            return invalid(
                "semantic catalog solve",
                "catalog rows did not produce an independently accepted state",
            );
        }
        let mut accepted = document.clone();
        accepted.project_accepted_state(&sketch, &mappings)?;
        independently_validate_planar_catalog(self, &accepted, request)?;
        let audit = grouped_planar_audit(&solve_result, pending_audit)?;
        let scalar_audit = independently_validated_scalar_audit(self, &accepted)?;
        Ok(DocumentSemanticSolveResult {
            document: accepted,
            solve_result,
            audit,
            scalar_audit,
        })
    }

    /// Controlled counterpart to [`Self::solve_document`]. Interrupted work never
    /// projects or publishes a partially solved document.
    ///
    /// # Errors
    /// Returns a validation, lowering, solve, or independent-publication error.
    pub fn solve_document_controlled(
        &self,
        document: &SketchDocument,
        request: SketchSolveRequest,
        config: SolverConfig,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentSemanticSolveResult>, DocumentError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        self.validate(document)?;
        let Some(lowered) = document.lower_with_controller(&mut controller)? else {
            return Ok(controller.outcome_unchecked());
        };
        let (mut sketch, mappings) = lowered.into_parts();
        let mut pending_audit = Vec::with_capacity(self.planar_sources.len());
        for source in &self.planar_sources {
            if controller
                .charge(
                    OperationWorkCounter::DocumentLoweringItems,
                    1,
                    OperationCheckpoint::DocumentLowering,
                )
                .is_err()
            {
                return Ok(controller.outcome_unchecked());
            }
            let before = runtime_sketch_sources(&sketch).collect::<Vec<_>>();
            let templates = lower_planar_source(document, &mappings, &mut sketch, source)?;
            let sources = runtime_sketch_sources(&sketch)
                .filter(|source| !before.contains(source))
                .collect::<Vec<_>>();
            pending_audit.push((source, templates, sources));
        }
        lower_executable_scalar_sources(document, &mappings, &mut sketch, &self.sources)?;
        let Some(solve_result) = sketch.solve_with_controller(request, config, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        if !solve_result.accepted()
            || solve_result
                .acceptance_hard_residual_max
                .is_none_or(|value| {
                    !value.is_finite() || value > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
                })
        {
            return invalid(
                "semantic catalog solve",
                "catalog rows did not produce an independently accepted state",
            );
        }
        let mut accepted = document.clone();
        if !accepted.project_accepted_state_with_controller(&sketch, &mappings, &mut controller)? {
            return Ok(controller.outcome_unchecked());
        }
        if !independently_validate_planar_catalog_controlled(
            self,
            &accepted,
            request,
            &mut controller,
        )? {
            return Ok(controller.outcome_unchecked());
        }
        let audit = grouped_planar_audit(&solve_result, pending_audit)?;
        let scalar_audit = independently_validated_scalar_audit(self, &accepted)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeCommit)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(DocumentSemanticSolveResult {
            document: accepted,
            solve_result,
            audit,
            scalar_audit,
        }))
    }

    /// Lowers one catalog-owned source after complete catalog validation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid catalog, unknown source, or invalid relation.
    pub fn lower(
        &self,
        document: &SketchDocument,
        source: DocumentSourceId,
    ) -> Result<LoweredDocumentScalarSource, DocumentError> {
        self.validate(document)?;
        self.source(source)
            .ok_or(DocumentError::UnknownId {
                kind: "semantic source",
                id: source.0,
            })?
            .lower(document)
    }

    /// Lowers one source with cooperative cancellation and deterministic work limits.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed semantic input; interruption is returned as an
    /// [`OperationOutcome`].
    pub fn lower_controlled(
        &self,
        document: &SketchDocument,
        source: DocumentSourceId,
        control: OperationControl,
    ) -> Result<OperationOutcome<LoweredDocumentScalarSource>, DocumentError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        for _ in &self.sources {
            if controller
                .charge(
                    OperationWorkCounter::DocumentValidationItems,
                    1,
                    OperationCheckpoint::DocumentValidation,
                )
                .is_err()
            {
                return Ok(controller.outcome_unchecked());
            }
        }
        self.validate(document)?;
        if controller
            .charge(
                OperationWorkCounter::DocumentLoweringItems,
                1,
                OperationCheckpoint::DocumentLowering,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let lowered = self
            .source(source)
            .ok_or(DocumentError::UnknownId {
                kind: "semantic source",
                id: source.0,
            })?
            .lower(document)?;
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
            || controller
                .checkpoint(OperationCheckpoint::AfterFinalValidation)
                .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(lowered))
    }

    /// Recomputes and compares all public row/audit evidence for one source.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed/stale/tampered evidence, invalid tolerance,
    /// catalog corruption, or an unknown source.
    pub fn independently_validated(
        &self,
        document: &SketchDocument,
        source: DocumentSourceId,
        evidence: &LoweredDocumentScalarSource,
        tolerance: f64,
    ) -> Result<bool, DocumentError> {
        self.validate(document)?;
        evidence.independently_validated(
            document,
            self.source(source).ok_or(DocumentError::UnknownId {
                kind: "semantic source",
                id: source.0,
            })?,
            tolerance,
        )
    }
}

impl LoweredDocumentScalarSource {
    /// Independently validates every normalized row before a caller treats it as satisfied.
    ///
    /// The effective tolerance is capped at the sketch acceptance threshold `1e-9`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid tolerance or incomplete/non-finite evidence.
    fn independently_validated(
        &self,
        document: &SketchDocument,
        source: &DocumentScalarSource,
        tolerance: f64,
    ) -> Result<bool, DocumentError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return invalid(
                "scalar validation tolerance",
                "tolerance must be finite and nonnegative",
            );
        }
        if document.id() != source.document_id
            || self.audit.document_id != source.document_id
            || self.rows.len() != 1
            || self.source_id != source.source_id
            || self.audit.source_id != source.source_id
            || self.audit.source_label != source.label
            || self.audit.relation != source.relation
            || self.rows.iter().any(|row| {
                row.category != ResidualCategory::Hard
                    || !row.raw_value.is_finite()
                    || !row.normalized_value.is_finite()
                    || !row.characteristic_scale.is_finite()
            })
        {
            return invalid(
                "scalar validation evidence",
                "row and audit evidence must be complete, matching, and finite",
            );
        }
        let definition = lower_relation(document, source.relation)?;
        let characteristic_scale = characteristic_scale(document.model_scale(), definition.unit)?;
        let independent_raw = evaluate_raw(
            &definition.raw_jacobian,
            &definition.values,
            definition.target,
        )?;
        let independent_normalized = independent_raw / characteristic_scale;
        let normalized_jacobian = definition
            .raw_jacobian
            .iter()
            .map(|coefficient| coefficient / characteristic_scale)
            .collect::<Vec<_>>();
        if self.audit.equation_template != definition.template
            || self.audit.unit != definition.unit
            || self.audit.characteristic_scale.to_bits() != characteristic_scale.to_bits()
            || self.audit.bindings != definition.bindings
            || self.rows[0].characteristic_scale.to_bits() != characteristic_scale.to_bits()
            || self.rows[0].raw_jacobian != definition.raw_jacobian
            || self.rows[0].normalized_jacobian != normalized_jacobian
            || self.rows[0].raw_value.to_bits() != independent_raw.to_bits()
            || self.rows[0].normalized_value.to_bits() != independent_normalized.to_bits()
        {
            return invalid(
                "scalar validation evidence",
                "row and audit structure does not match the persistent relation",
            );
        }
        if !independent_normalized.is_finite() {
            return invalid(
                "scalar validation evidence",
                "independent evaluation produced a non-finite value",
            );
        }
        let effective = tolerance.min(crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE);
        Ok(independent_normalized.abs() <= effective)
    }
}

impl DocumentScalarSource {
    /// Validates and deterministically lowers this source against one document.
    ///
    /// This produces the M36 closed row IR; it does not add a sketch-v1-to-v4
    /// constraint variant or mutate an accepted document.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed labels, operands, units, domains, branches,
    /// targets, or non-finite evaluated rows.
    fn lower(
        &self,
        document: &SketchDocument,
    ) -> Result<LoweredDocumentScalarSource, DocumentError> {
        if self.document_id != document.id() {
            return invalid(
                "scalar source document",
                "source belongs to a different sketch document",
            );
        }
        validate_label(&self.label)?;
        let definition = lower_relation(document, self.relation)?;
        let characteristic_scale = characteristic_scale(document.model_scale(), definition.unit)?;
        let normalized_jacobian = definition
            .raw_jacobian
            .iter()
            .map(|coefficient| coefficient / characteristic_scale)
            .collect();
        let mut row = DocumentScalarRow {
            category: ResidualCategory::Hard,
            raw_value: 0.0,
            normalized_value: 0.0,
            raw_jacobian: definition.raw_jacobian,
            normalized_jacobian,
            characteristic_scale,
            target: definition.target,
        };
        row.raw_value = row.evaluate_raw(&definition.values)?;
        row.normalized_value = row.evaluate_normalized(&definition.values)?;
        if !row.normalized_value.is_finite() {
            return invalid(
                "scalar row value",
                "normalization produced a non-finite value",
            );
        }
        Ok(LoweredDocumentScalarSource {
            source_id: self.source_id,
            rows: vec![row],
            audit: DocumentScalarAudit {
                document_id: self.document_id,
                source_id: self.source_id,
                source_label: self.label.clone(),
                relation: self.relation,
                equation_template: definition.template,
                unit: definition.unit,
                characteristic_scale,
                bindings: definition.bindings,
            },
        })
    }
}

fn lower_relation(
    document: &SketchDocument,
    relation: DocumentScalarRelation,
) -> Result<ScalarRowDefinition, DocumentError> {
    match relation {
        DocumentScalarRelation::Fixed { property, target } => {
            document.validate_scalar_property_ref(property)?;
            validate_domain_value(target, property.domain)?;
            if let DocumentScalarBranch::Parameter {
                support,
                neighborhood,
            } = property.branch
            {
                document.validate_parameter_property(
                    support,
                    target,
                    property.domain,
                    neighborhood,
                )?;
            }
            Ok(ScalarRowDefinition {
                unit: property.unit,
                template: "property - target",
                bindings: vec![DocumentScalarAuditBinding {
                    role: "property",
                    property,
                }],
                values: vec![scalar_value(document, property.scalar)?],
                raw_jacobian: vec![1.0],
                target,
            })
        }
        DocumentScalarRelation::Equal { first, second } => {
            document.validate_scalar_property_ref(first)?;
            document.validate_scalar_property_ref(second)?;
            if first.unit != second.unit
                || first.domain != second.domain
                || first.branch != second.branch
            {
                return invalid(
                    "equal scalar operands",
                    "units, domains and branch semantics must match exactly",
                );
            }
            Ok(ScalarRowDefinition {
                unit: first.unit,
                template: "first - second",
                bindings: vec![
                    DocumentScalarAuditBinding {
                        role: "first",
                        property: first,
                    },
                    DocumentScalarAuditBinding {
                        role: "second",
                        property: second,
                    },
                ],
                values: vec![
                    scalar_value(document, first.scalar)?,
                    scalar_value(document, second.scalar)?,
                ],
                raw_jacobian: vec![1.0, -1.0],
                target: 0.0,
            })
        }
    }
}

fn scalar_value(document: &SketchDocument, scalar: DesignScalarId) -> Result<f64, DocumentError> {
    document
        .scalar(scalar)
        .map(|value| value.value)
        .ok_or(DocumentError::UnknownId {
            kind: "scalar",
            id: scalar.0,
        })
}

impl SketchDocument {
    /// Validates one typed scalar property through persistent identity.
    ///
    /// # Errors
    ///
    /// Returns an error for missing scalars or mismatched unit/domain/branch state.
    pub fn validate_scalar_property_ref(
        &self,
        property: DocumentScalarPropertyRef,
    ) -> Result<(), DocumentError> {
        let scalar = self
            .scalar(property.scalar)
            .ok_or(DocumentError::UnknownId {
                kind: "scalar",
                id: property.scalar.0,
            })?;
        if scalar.domain != property.domain {
            return invalid(
                "scalar property domain",
                "operand domain must match the persistent scalar",
            );
        }
        let storage_unit_matches = match property.unit {
            DocumentScalarUnit::Length => scalar.unit == ScalarUnit::Length,
            DocumentScalarUnit::Angle => scalar.unit == ScalarUnit::Angle,
            DocumentScalarUnit::Parameter => match property.domain {
                ScalarDomain::Periodic { .. } => scalar.unit == ScalarUnit::Angle,
                _ => scalar.unit == ScalarUnit::Parameter,
            },
            DocumentScalarUnit::Dimensionless | DocumentScalarUnit::Curvature => {
                scalar.unit == ScalarUnit::Parameter
            }
        };
        if !storage_unit_matches {
            return invalid(
                "scalar property unit",
                "operand unit is incompatible with the persistent scalar",
            );
        }
        let branch_matches = matches!(
            (property.unit, property.branch),
            (
                DocumentScalarUnit::Length,
                DocumentScalarBranch::Unsigned | DocumentScalarBranch::SignedLength { .. }
            ) | (
                DocumentScalarUnit::Angle,
                DocumentScalarBranch::Angle { .. }
            ) | (
                DocumentScalarUnit::Dimensionless,
                DocumentScalarBranch::Dimensionless
            ) | (
                DocumentScalarUnit::Curvature,
                DocumentScalarBranch::Curvature { .. }
            ) | (
                DocumentScalarUnit::Parameter,
                DocumentScalarBranch::Parameter { .. }
            )
        );
        if !branch_matches {
            return invalid(
                "scalar property branch",
                "branch state is incompatible with the scalar unit",
            );
        }
        match property.branch {
            DocumentScalarBranch::Unsigned
                if !matches!(
                    property.domain,
                    ScalarDomain::Positive | ScalarDomain::Bounded { lower: 0.0, .. }
                ) =>
            {
                return invalid(
                    "scalar property domain",
                    "unsigned length requires an explicit nonnegative domain",
                );
            }
            DocumentScalarBranch::SignedLength { provenance } => {
                if matches!(property.domain, ScalarDomain::Positive) {
                    return invalid(
                        "scalar property domain",
                        "signed length cannot use a positive-only domain",
                    );
                }
                if let DocumentSignedLengthProvenance::LineSide { support, .. } = provenance {
                    self.validate_line_support_ref(support)?;
                }
            }
            DocumentScalarBranch::Parameter {
                support,
                neighborhood,
            } => {
                self.validate_parameter_property(
                    support,
                    scalar.value,
                    property.domain,
                    neighborhood,
                )?;
            }
            _ => {}
        }
        validate_domain_value(scalar.value, property.domain)
    }
}

impl SketchDocument {
    #[allow(clippy::too_many_lines)]
    fn validate_parameter_property(
        &self,
        support: DocumentCurveSpanRef,
        value: f64,
        domain: ScalarDomain,
        neighborhood: ContactNeighborhood,
    ) -> Result<(), DocumentError> {
        self.validate_curve_span_ref(support)?;
        let curve = self
            .curve(support.span.curve)
            .ok_or(DocumentError::UnknownId {
                kind: "curve",
                id: support.span.curve.0,
            })?;
        let bounded_winding = matches!(
            curve.definition,
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                ..
            } | CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Periodic,
                ..
            }
        );
        let total = match domain {
            ScalarDomain::Finite => {
                if !matches!(
                    curve.definition,
                    CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
                ) || support.winding != 0
                {
                    return invalid(
                        "parameter support",
                        "finite supporting-line parameters require non-periodic line topology",
                    );
                }
                value
            }
            ScalarDomain::Bounded { lower, upper } => {
                if lower.to_bits() != 0.0f64.to_bits()
                    || upper.to_bits() != 1.0f64.to_bits()
                    || matches!(
                        curve.definition,
                        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
                    )
                    || (support.winding != 0 && !bounded_winding)
                {
                    return invalid(
                        "parameter support",
                        "bounded curve parameters require the matching unit span and winding topology",
                    );
                }
                value
            }
            ScalarDomain::Periodic { period } => {
                if period.to_bits() != std::f64::consts::TAU.to_bits()
                    || !matches!(
                        curve.definition,
                        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
                    )
                {
                    return invalid(
                        "parameter support",
                        "periodic parameters require a full circle or ellipse with TAU period",
                    );
                }
                period.mul_add(f64::from(support.winding), value)
            }
            ScalarDomain::Positive => {
                return invalid(
                    "parameter domain",
                    "curve parameters cannot use a positive-only scalar domain",
                );
            }
        };
        let valid = match (domain, neighborhood) {
            (
                ScalarDomain::Finite | ScalarDomain::Periodic { .. },
                ContactNeighborhood::Interior,
            ) => true,
            (
                ScalarDomain::Finite | ScalarDomain::Periodic { .. },
                ContactNeighborhood::Local { lower, upper },
            ) => lower.is_finite() && upper.is_finite() && lower < total && total < upper,
            (ScalarDomain::Bounded { lower, .. }, ContactNeighborhood::Start) => {
                value.to_bits() == lower.to_bits()
            }
            (ScalarDomain::Bounded { upper, .. }, ContactNeighborhood::End) => {
                value.to_bits() == upper.to_bits()
            }
            (ScalarDomain::Bounded { lower, upper }, ContactNeighborhood::Interior) => {
                lower < value && value < upper
            }
            (
                ScalarDomain::Bounded {
                    lower: domain_lower,
                    upper: domain_upper,
                },
                ContactNeighborhood::Local { lower, upper },
            ) => {
                lower.is_finite()
                    && upper.is_finite()
                    && domain_lower <= lower
                    && lower < value
                    && value < upper
                    && upper <= domain_upper
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            invalid(
                "parameter neighborhood",
                "selection does not match the parameter value and domain",
            )
        }
    }
}

fn characteristic_scale(model_scale: f64, unit: DocumentScalarUnit) -> Result<f64, DocumentError> {
    let scale = match unit {
        DocumentScalarUnit::Length => model_scale,
        DocumentScalarUnit::Curvature => 1.0 / model_scale,
        DocumentScalarUnit::Angle
        | DocumentScalarUnit::Dimensionless
        | DocumentScalarUnit::Parameter => 1.0,
    };
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        invalid(
            "scalar characteristic scale",
            "unit scaling must be positive and finite",
        )
    }
}

fn evaluate_raw(jacobian: &[f64], values: &[f64], target: f64) -> Result<f64, DocumentError> {
    if jacobian.len() != values.len() {
        return invalid(
            "scalar row values",
            "value count must match scalar-row incidence",
        );
    }
    jacobian
        .iter()
        .zip(values)
        .try_fold(-target, |sum, (coefficient, value)| {
            let next = sum + coefficient * value;
            next.is_finite()
                .then_some(next)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "scalar row value",
                    message: "evaluation produced a non-finite value".into(),
                })
        })
}

fn validate_label(label: &str) -> Result<(), DocumentError> {
    if label.is_empty() || label.len() > crate::MAX_LABEL_BYTES {
        invalid(
            "scalar source label",
            "label must be nonempty and within the document byte limit",
        )
    } else {
        Ok(())
    }
}

fn validate_domain_value(value: f64, domain: ScalarDomain) -> Result<(), DocumentError> {
    if !value.is_finite() {
        return invalid("scalar value", "value must be finite");
    }
    match domain {
        ScalarDomain::Finite => Ok(()),
        ScalarDomain::Positive if value > 0.0 => Ok(()),
        ScalarDomain::Bounded { lower, upper }
            if lower.is_finite()
                && upper.is_finite()
                && lower < upper
                && (lower..=upper).contains(&value) =>
        {
            Ok(())
        }
        ScalarDomain::Periodic { period }
            if period.is_finite() && period > 0.0 && (0.0..period).contains(&value) =>
        {
            Ok(())
        }
        _ => invalid("scalar domain", "value is outside its finite domain"),
    }
}

#[allow(clippy::too_many_lines)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PointOperandIdentity {
    Stored(DesignPointId),
    BoundedEndpoint(CurveId, crate::FeatureEndpoint),
    FixedCurveLocation(crate::ContactId),
}

fn point_operand_identity(
    document: &SketchDocument,
    point: crate::DocumentPointRef,
) -> Result<PointOperandIdentity, DocumentError> {
    use crate::DocumentPointRef as P;

    document.validate_point_ref(point)?;
    match point {
        P::Point { point } => Ok(PointOperandIdentity::Stored(point)),
        P::Center(center) => {
            resolve_point_ref(document, P::Center(center)).map(PointOperandIdentity::Stored)
        }
        P::Control(control) => document
            .resolve_control_ref(control)
            .map(PointOperandIdentity::Stored),
        P::Endpoint(endpoint) => match resolve_point_ref(document, P::Endpoint(endpoint)) {
            Ok(point) => Ok(PointOperandIdentity::Stored(point)),
            Err(_) => Ok(PointOperandIdentity::BoundedEndpoint(
                endpoint.curve,
                endpoint.endpoint,
            )),
        },
        P::Focus { curve, index: 0 } => resolve_point_ref(document, point)
            .map(PointOperandIdentity::Stored)
            .map_err(|_| DocumentError::InvalidField {
                field: "point feature",
                message: format!(
                    "derived focus on curve {} is outside the executable M37 point matrix",
                    curve.0
                ),
            }),
        P::Focus { .. } => invalid(
            "point feature",
            "derived focus is outside the executable M37 point matrix",
        ),
        P::FixedCurveLocation { contact } => Ok(PointOperandIdentity::FixedCurveLocation(contact)),
    }
}

fn same_unordered_point_pair(
    first: [PointOperandIdentity; 2],
    second: [PointOperandIdentity; 2],
) -> bool {
    (first[0] == second[0] && first[1] == second[1])
        || (first[0] == second[1] && first[1] == second[0])
}

fn validate_entity_symmetry_families(
    document: &SketchDocument,
    first: CurveId,
    second: CurveId,
) -> Result<(), DocumentError> {
    let first = &document
        .curve(first)
        .ok_or(DocumentError::UnknownId {
            kind: "curve",
            id: first.0,
        })?
        .definition;
    let second = &document
        .curve(second)
        .ok_or(DocumentError::UnknownId {
            kind: "curve",
            id: second.0,
        })?
        .definition;
    let compatible = match (first, second) {
        (CurveDefinition::Line { .. }, CurveDefinition::Line { .. })
        | (CurveDefinition::Circle { .. }, CurveDefinition::Circle { .. })
        | (CurveDefinition::QuadraticBezier { .. }, CurveDefinition::QuadraticBezier { .. })
        | (CurveDefinition::CubicBezier { .. }, CurveDefinition::CubicBezier { .. })
        | (CurveDefinition::ParabolaSegment { .. }, CurveDefinition::ParabolaSegment { .. }) => {
            true
        }
        (
            CurveDefinition::Polyline {
                closed: first_closed,
                ..
            },
            CurveDefinition::Polyline {
                closed: second_closed,
                ..
            },
        ) => first_closed == second_closed,
        (
            CurveDefinition::BSpline {
                degree: first_degree,
                knots: first_knots,
                form: first_form,
                ..
            },
            CurveDefinition::BSpline {
                degree: second_degree,
                knots: second_knots,
                form: second_form,
                ..
            },
        ) => {
            first_degree == second_degree
                && first_knots == second_knots
                && first_form == second_form
        }
        _ => false,
    };
    if compatible {
        Ok(())
    } else {
        invalid(
            "entity symmetry families",
            "entities must have compatible fully corresponded point-defined semantics",
        )
    }
}

#[allow(clippy::too_many_lines)]
fn validate_planar_relation(
    document: &SketchDocument,
    relation: &DocumentPlanarRelation,
) -> Result<(), DocumentError> {
    use DocumentPlanarRelation as R;
    match relation {
        R::Concentric { first, second } => {
            document.validate_center_ref(*first)?;
            document.validate_center_ref(*second)?;
            if point_operand_identity(document, crate::DocumentPointRef::Center(*first))?
                == point_operand_identity(document, crate::DocumentPointRef::Center(*second))?
            {
                return invalid("concentric operands", "centers must be distinct features");
            }
        }
        R::Collinear { first, second } => {
            document.validate_line_support_ref(*first)?;
            document.validate_line_support_ref(*second)?;
            if first.span == second.span {
                return invalid("collinear operands", "line supports must be distinct");
            }
        }
        R::HorizontalPoints { first, second } | R::VerticalPoints { first, second } => {
            if point_operand_identity(document, *first)?
                == point_operand_identity(document, *second)?
            {
                return invalid("point-pair operands", "points must be distinct features");
            }
        }
        R::PointSymmetry {
            first,
            second,
            center,
        } => {
            let first = point_operand_identity(document, *first)?;
            let second = point_operand_identity(document, *second)?;
            let center = point_operand_identity(document, *center)?;
            if first == second && second == center {
                return invalid(
                    "point symmetry",
                    "all three point operands resolve identically",
                );
            }
        }
        R::EntitySymmetry {
            first_entity,
            second_entity,
            point_pairs,
            scalar_pairs,
            axis,
        } => {
            document.validate_line_support_ref(*axis)?;
            if first_entity == second_entity {
                return invalid(
                    "entity symmetry correspondence",
                    "entity operands must be distinct",
                );
            }
            validate_entity_symmetry_families(document, *first_entity, *second_entity)?;
            let (first_points, first_scalars) = capture_entity(document, *first_entity)?;
            let (second_points, second_scalars) = capture_entity(document, *second_entity)?;
            if point_pairs.len() != first_points.len()
                || point_pairs.len() != second_points.len()
                || scalar_pairs.len() != first_scalars.len()
                || scalar_pairs.len() != second_scalars.len()
            {
                return invalid(
                    "entity symmetry correspondence",
                    "correspondence must cover every independent entity degree",
                );
            }
            for (index, [first, second]) in point_pairs.iter().enumerate() {
                if point_operand_identity(document, *first)?
                    != PointOperandIdentity::Stored(first_points[index].0)
                    || point_operand_identity(document, *second)?
                        != PointOperandIdentity::Stored(second_points[index].0)
                {
                    return invalid(
                        "entity symmetry correspondence",
                        "point correspondence does not match complete entity definition order",
                    );
                }
            }
            for (index, [first, second]) in scalar_pairs.iter().enumerate() {
                if *first != first_scalars[index].0
                    || *second != second_scalars[index].0
                    || document.scalar(*first).map(|value| value.unit)
                        != document.scalar(*second).map(|value| value.unit)
                {
                    return invalid(
                        "entity symmetry correspondence",
                        "scalar correspondence does not match complete compatible entity state",
                    );
                }
            }
        }
        R::EqualCircularRadius { first, second } => {
            let first = circular_radius_scalar(document, *first)?;
            let second = circular_radius_scalar(document, *second)?;
            if first == second {
                return invalid("equal circular radius", "curves must be distinct");
            }
        }
        R::EqualDistance { first, second } => {
            let first = [
                point_operand_identity(document, first[0])?,
                point_operand_identity(document, first[1])?,
            ];
            let second = [
                point_operand_identity(document, second[0])?,
                point_operand_identity(document, second[1])?,
            ];
            if first[0] == first[1] || second[0] == second[1] {
                return invalid("equal distance", "each point pair must be distinct");
            }
            if same_unordered_point_pair(first, second) {
                return invalid("equal distance", "point pairs resolve to the same distance");
            }
        }
        R::EqualAngle { first, second } => {
            validate_angle_operand(document, *first)?;
            validate_angle_operand(document, *second)?;
            if equivalent_angle_operands(*first, *second) {
                return invalid("equal angle", "angle operands must be distinct");
            }
        }
        R::BlockEntity {
            curve,
            captured_definition,
            captured_points,
            captured_scalars,
        } => {
            if document
                .curve(*curve)
                .is_none_or(|value| value.definition != *captured_definition)
            {
                return invalid(
                    "block entity definition",
                    "entity shape or explicit branch state differs from its capture",
                );
            }
            let (expected_points, expected_scalars) = capture_entity(document, *curve)?;
            if !captured_points
                .iter()
                .map(|(id, _)| id)
                .eq(expected_points.iter().map(|(id, _)| id))
                || !captured_scalars
                    .iter()
                    .map(|(id, _)| id)
                    .eq(expected_scalars.iter().map(|(id, _)| id))
                || captured_points
                    .iter()
                    .any(|(_, target)| !target.iter().all(|value| value.is_finite()))
                || captured_scalars
                    .iter()
                    .any(|(_, target)| !target.is_finite())
            {
                return invalid(
                    "block entity capture",
                    "captured semantic state is incomplete",
                );
            }
        }
    }
    Ok(())
}

fn validate_angle_operand(
    document: &SketchDocument,
    operand: DocumentAngleOperand,
) -> Result<(), DocumentError> {
    document.validate_line_support_ref(operand.first)?;
    document.validate_line_support_ref(operand.second)?;
    if operand.first.span == operand.second.span {
        return invalid(
            "angle operand",
            "an angle requires two distinct underlying supports",
        );
    }
    Ok(())
}

fn equivalent_angle_operands(first: DocumentAngleOperand, second: DocumentAngleOperand) -> bool {
    if first.winding != second.winding {
        return false;
    }
    let relative_reversal =
        |operand: DocumentAngleOperand| operand.first.direction != operand.second.direction;
    let same_order = first.orientation == second.orientation
        && first.first.span == second.first.span
        && first.second.span == second.second.span;
    let opposite_orientation = matches!(
        (first.orientation, second.orientation),
        (
            DocumentAngleOrientation::CounterClockwise,
            DocumentAngleOrientation::Clockwise
        ) | (
            DocumentAngleOrientation::Clockwise,
            DocumentAngleOrientation::CounterClockwise
        )
    );
    let reversed_order = opposite_orientation
        && first.first.span == second.second.span
        && first.second.span == second.first.span;
    (same_order || reversed_order) && relative_reversal(first) == relative_reversal(second)
}

fn resolve_point_ref(
    document: &SketchDocument,
    point: crate::DocumentPointRef,
) -> Result<DesignPointId, DocumentError> {
    use crate::DocumentPointRef as P;

    document.validate_point_ref(point)?;
    match point {
        P::Point { point } => Ok(point),
        P::Center(center) => match &document
            .curve(center.curve)
            .ok_or(DocumentError::UnknownId {
                kind: "curve",
                id: center.curve.0,
            })?
            .definition
        {
            CurveDefinition::Circle { center, .. }
            | CurveDefinition::CircularArc { center, .. }
            | CurveDefinition::Ellipse { center, .. }
            | CurveDefinition::EllipticalArc { center, .. }
            | CurveDefinition::HyperbolaSegment { center, .. } => Ok(*center),
            _ => invalid("point feature", "curve has no stored center"),
        },
        P::Endpoint(endpoint) => {
            let curve = &document
                .curve(endpoint.curve)
                .ok_or(DocumentError::UnknownId {
                    kind: "curve",
                    id: endpoint.curve.0,
                })?
                .definition;
            let pair = match curve {
                CurveDefinition::Line { start, end, .. }
                | CurveDefinition::RationalQuadraticConic { start, end, .. } => [*start, *end],
                CurveDefinition::Polyline { points, .. }
                | CurveDefinition::BSpline {
                    controls: points, ..
                }
                | CurveDefinition::Nurbs {
                    controls: points, ..
                } => [points[0], points[points.len() - 1]],
                CurveDefinition::QuadraticBezier { controls } => [controls[0], controls[2]],
                CurveDefinition::CubicBezier { controls } => [controls[0], controls[3]],
                _ => {
                    return invalid(
                        "point feature",
                        "derived curve endpoint requires common-jet incidence",
                    );
                }
            };
            Ok(match endpoint.endpoint {
                crate::FeatureEndpoint::Start => pair[0],
                crate::FeatureEndpoint::End => pair[1],
            })
        }
        P::Control(control) => document.resolve_control_ref(control),
        P::Focus { curve, index: 0 } => {
            match document.curve(curve).map(|value| &value.definition) {
                Some(CurveDefinition::ParabolaSegment { focus, .. }) => Ok(*focus),
                _ => invalid(
                    "point feature",
                    "derived conic focus requires common-jet incidence",
                ),
            }
        }
        P::Focus { .. } | P::FixedCurveLocation { .. } => invalid(
            "point feature",
            "derived point feature requires common-jet incidence",
        ),
    }
}

pub(crate) fn runtime_point_ref(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    point: crate::DocumentPointRef,
) -> Result<crate::PointId, DocumentError> {
    let persistent = resolve_point_ref(document, point)?;
    mappings
        .runtime_point(persistent)
        .ok_or(DocumentError::UnknownId {
            kind: "runtime point",
            id: persistent.0,
        })
}

fn runtime_planar_point_ref(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    cache: &mut Vec<(PointOperandIdentity, crate::PointId)>,
    templates: &mut Vec<&'static str>,
    point: crate::DocumentPointRef,
) -> Result<crate::PointId, DocumentError> {
    if let Ok(runtime) = runtime_point_ref(document, mappings, point) {
        return Ok(runtime);
    }
    let identity = point_operand_identity(document, point)?;
    if let Some((_, runtime)) = cache.iter().find(|(candidate, _)| *candidate == identity) {
        return Ok(*runtime);
    }
    let (position, contact) = match point {
        crate::DocumentPointRef::Endpoint(endpoint) => {
            let spans = document.curve_spans(endpoint.curve)?;
            let span = match endpoint.endpoint {
                crate::FeatureEndpoint::Start => spans.first(),
                crate::FeatureEndpoint::End => spans.last(),
            }
            .copied()
            .ok_or_else(|| DocumentError::InvalidField {
                field: "point feature",
                message: "bounded endpoint curve has no executable span".into(),
            })?;
            let parameter = match endpoint.endpoint {
                crate::FeatureEndpoint::Start => 0.0,
                crate::FeatureEndpoint::End => 1.0,
            };
            let jet = document
                .evaluate_curve_jet(span, parameter)
                .map_err(|error| DocumentError::InvalidField {
                    field: "point feature",
                    message: error.to_string(),
                })?;
            (
                jet.position,
                crate::document_lowering::runtime_endpoint_contact(
                    mappings,
                    span,
                    endpoint.endpoint,
                )?,
            )
        }
        crate::DocumentPointRef::FixedCurveLocation { contact } => {
            let slot = document.contact(contact).ok_or(DocumentError::UnknownId {
                kind: "contact",
                id: contact.0,
            })?;
            let jet = document.evaluate_contact_jet(contact).map_err(|error| {
                DocumentError::InvalidField {
                    field: "point feature",
                    message: error.to_string(),
                }
            })?;
            (
                jet.position,
                crate::document_lowering::runtime_curve_contact(document, mappings, slot)?,
            )
        }
        _ => return runtime_point_ref(document, mappings, point),
    };
    let runtime = sketch.add_point(Point2::new(position.x, position.y))?;
    sketch.add_point_on_curve(runtime, contact)?;
    templates.extend([
        "derived point.x - curve position.x",
        "derived point.y - curve position.y",
    ]);
    cache.push((identity, runtime));
    Ok(runtime)
}

fn directed_runtime_segment(
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    support: DocumentLineSupportRef,
) -> Result<crate::SegmentId, DocumentError> {
    let segment = mappings
        .runtime_segment(support.span)
        .ok_or(DocumentError::UnknownId {
            kind: "runtime line support",
            id: support.span.curve.0,
        })?;
    if support.direction == crate::DocumentDirectionSense::Forward {
        return Ok(segment);
    }
    let (start, end) = sketch.segment_endpoints(segment)?;
    Ok(sketch.add_segment(end, start)?)
}

#[allow(clippy::too_many_lines)]
fn lower_planar_source(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    source: &DocumentPlanarSource,
) -> Result<Vec<&'static str>, DocumentError> {
    use DocumentPlanarRelation as R;
    let mut point_cache = Vec::new();
    let mut incidence_templates = Vec::new();
    macro_rules! runtime_planar {
        ($point:expr) => {
            runtime_planar_point_ref(
                document,
                mappings,
                sketch,
                &mut point_cache,
                &mut incidence_templates,
                $point,
            )?
        };
    }
    let templates = match &source.relation {
        R::Concentric { first, second } => {
            sketch.add_coincident(
                runtime_point_ref(document, mappings, crate::DocumentPointRef::Center(*first))?,
                runtime_point_ref(document, mappings, crate::DocumentPointRef::Center(*second))?,
            )?;
            vec![
                "center(first).x - center(second).x",
                "center(first).y - center(second).y",
            ]
        }
        R::Collinear { first, second } => {
            let first = directed_runtime_segment(mappings, sketch, *first)?;
            let second = directed_runtime_segment(mappings, sketch, *second)?;
            sketch.add_collinear(first, second)?;
            vec![
                "cross(unit(first), unit(second))",
                "cross(unit(first), second.start - first.start)",
            ]
        }
        R::HorizontalPoints { first, second } => {
            let first = runtime_planar!(*first);
            let second = runtime_planar!(*second);
            sketch.add_horizontal_points(first, second)?;
            vec!["second.y - first.y"]
        }
        R::VerticalPoints { first, second } => {
            let first = runtime_planar!(*first);
            let second = runtime_planar!(*second);
            sketch.add_vertical_points(first, second)?;
            vec!["second.x - first.x"]
        }
        R::PointSymmetry {
            first,
            second,
            center,
        } => {
            let first = runtime_planar!(*first);
            let second = runtime_planar!(*second);
            let center = runtime_planar!(*center);
            sketch.add_point_symmetry(first, second, center)?;
            vec![
                "center.x - (first.x + second.x)/2",
                "center.y - (first.y + second.y)/2",
            ]
        }
        R::EntitySymmetry {
            point_pairs,
            scalar_pairs,
            axis,
            ..
        } => {
            let axis = directed_runtime_segment(mappings, sketch, *axis)?;
            for [first, second] in point_pairs {
                let first = runtime_planar!(*first);
                let second = runtime_planar!(*second);
                sketch.add_symmetric_about_line(first, second, axis)?;
            }
            for [first, second] in scalar_pairs {
                let first_runtime = runtime_scalar_ref(document, mappings, *first)?;
                let second_runtime = runtime_scalar_ref(document, mappings, *second)?;
                let scale = match document
                    .scalar(*first)
                    .ok_or(DocumentError::UnknownId {
                        kind: "scalar",
                        id: first.0,
                    })?
                    .unit
                {
                    ScalarUnit::Length => document.model_scale(),
                    ScalarUnit::Angle | ScalarUnit::Parameter => 1.0,
                };
                sketch.add_equal_scalar(first_runtime, second_runtime, scale)?;
            }
            let mut templates = point_pairs
                .iter()
                .flat_map(|_| {
                    [
                        "pair midpoint lies on symmetry axis",
                        "pair displacement is normal to symmetry axis",
                    ]
                })
                .collect::<Vec<_>>();
            templates.extend(
                scalar_pairs
                    .iter()
                    .map(|_| "corresponding entity scalar(first) - scalar(second)"),
            );
            templates
        }
        R::EqualCircularRadius { first, second } => {
            match (
                mappings.runtime_circle(*first),
                mappings.runtime_arc(*first),
                mappings.runtime_circle(*second),
                mappings.runtime_arc(*second),
            ) {
                (Some(first), _, Some(second), _) => {
                    sketch.add_equal_circle_radius(first, second)?;
                }
                (Some(circle), _, _, Some(arc)) | (_, Some(arc), Some(circle), _) => {
                    sketch.add_equal_circle_arc_radius(circle, arc)?;
                }
                (_, Some(first), _, Some(second)) => {
                    sketch.add_equal_arc_radius(first, second)?;
                }
                _ => return invalid("equal circular radius", "operands are not circular curves"),
            }
            vec!["radius(first) - radius(second)"]
        }
        R::EqualDistance { first, second } => {
            let first_start = runtime_planar!(first[0]);
            let first_end = runtime_planar!(first[1]);
            let second_start = runtime_planar!(second[0]);
            let second_end = runtime_planar!(second[1]);
            sketch.add_equal_distance(first_start, first_end, second_start, second_end)?;
            vec!["distance(first pair) - distance(second pair)"]
        }
        R::EqualAngle { first, second } => {
            let first_from = directed_runtime_segment(mappings, sketch, first.first)?;
            let first_to = directed_runtime_segment(mappings, sketch, first.second)?;
            let second_from = directed_runtime_segment(mappings, sketch, second.first)?;
            let second_to = directed_runtime_segment(mappings, sketch, second.second)?;
            sketch.add_equal_angle(
                first_from,
                first_to,
                second_from,
                second_to,
                runtime_angle_orientation(first.orientation),
                first.winding,
                runtime_angle_orientation(second.orientation),
                second.winding,
            )?;
            vec!["directed_angle(first) - directed_angle(second)"]
        }
        R::BlockEntity {
            captured_points,
            captured_scalars,
            ..
        } => {
            let mut templates = Vec::new();
            for (point, target) in captured_points {
                let runtime = mappings
                    .runtime_point(*point)
                    .ok_or(DocumentError::UnknownId {
                        kind: "runtime point",
                        id: point.0,
                    })?;
                sketch.add_fixed_point_at(runtime, Point2::new(target[0], target[1]))?;
                templates.extend([
                    "entity point.x - captured point.x",
                    "entity point.y - captured point.y",
                ]);
            }
            for (scalar, target) in captured_scalars {
                if runtime_scalar_ref(document, mappings, *scalar).is_ok() {
                    add_fixed_document_scalar(document, mappings, sketch, *scalar, *target)?;
                    templates.push("entity scalar - captured scalar");
                } else if document
                    .scalar(*scalar)
                    .is_none_or(|value| value.value.to_bits() != target.to_bits())
                {
                    return invalid(
                        "block entity scalar",
                        "equation-free entity topology scalar differs from its capture",
                    );
                }
            }
            templates
        }
    };
    incidence_templates.extend(templates);
    Ok(incidence_templates)
}

fn runtime_angle_orientation(value: DocumentAngleOrientation) -> AngleOrientation {
    match value {
        DocumentAngleOrientation::CounterClockwise => AngleOrientation::CounterClockwise,
        DocumentAngleOrientation::Clockwise => AngleOrientation::Clockwise,
    }
}

fn circular_radius_scalar(
    document: &SketchDocument,
    curve: CurveId,
) -> Result<DesignScalarId, DocumentError> {
    match document.curve(curve).map(|value| &value.definition) {
        Some(
            CurveDefinition::Circle { radius, .. } | CurveDefinition::CircularArc { radius, .. },
        ) => Ok(*radius),
        Some(_) => invalid(
            "circular radius operand",
            "curve is not a circle or circular arc",
        ),
        None => Err(DocumentError::UnknownId {
            kind: "curve",
            id: curve.0,
        }),
    }
}

type CapturedEntityState = (Vec<(DesignPointId, [f64; 2])>, Vec<(DesignScalarId, f64)>);

fn capture_entity(
    document: &SketchDocument,
    curve: CurveId,
) -> Result<CapturedEntityState, DocumentError> {
    let definition = &document
        .curve(curve)
        .ok_or(DocumentError::UnknownId {
            kind: "curve",
            id: curve.0,
        })?
        .definition;
    let (points, scalars): (Vec<_>, Vec<_>) = match definition {
        CurveDefinition::Line { start, end, .. } => (vec![*start, *end], vec![]),
        CurveDefinition::Polyline { points, .. }
        | CurveDefinition::BSpline {
            controls: points, ..
        } => (points.clone(), vec![]),
        CurveDefinition::Circle { center, radius } => (vec![*center], vec![*radius]),
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => (vec![*center], vec![*radius, *start_angle, *end_angle]),
        CurveDefinition::QuadraticBezier { controls } => (controls.to_vec(), vec![]),
        CurveDefinition::CubicBezier { controls } => (controls.to_vec(), vec![]),
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => (vec![*center, *major_axis_point], vec![*minor_axis_ratio]),
        CurveDefinition::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            end_angle,
            ..
        } => (
            vec![*center, *major_axis_point],
            vec![*minor_axis_ratio, *start_angle, *end_angle],
        ),
        CurveDefinition::RationalQuadraticConic {
            start,
            middle_weight,
            end,
            ..
        } => (vec![*start, *end], vec![*middle_weight]),
        CurveDefinition::ParabolaSegment {
            vertex,
            focus,
            trim_start,
            trim_end,
        } => (vec![*vertex, *focus], vec![*trim_start, *trim_end]),
        CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            trim_start,
            trim_end,
            ..
        } => (
            vec![*center, *transverse_axis_point],
            vec![*semi_conjugate, *trim_start, *trim_end],
        ),
        CurveDefinition::Nurbs {
            controls, weights, ..
        } => (controls.clone(), weights.clone()),
    };
    let captured_points = points
        .into_iter()
        .map(|id| {
            document
                .point(id)
                .map(|point| (id, point.position))
                .ok_or(DocumentError::UnknownId {
                    kind: "point",
                    id: id.0,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let captured_scalars = scalars
        .into_iter()
        .map(|id| {
            document
                .scalar(id)
                .map(|scalar| (id, scalar.value))
                .ok_or(DocumentError::UnknownId {
                    kind: "scalar",
                    id: id.0,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((captured_points, captured_scalars))
}

fn add_fixed_document_scalar(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    scalar: DesignScalarId,
    target: f64,
) -> Result<(), DocumentError> {
    let property = runtime_scalar_ref(document, mappings, scalar)?;
    let unit = document
        .scalar(scalar)
        .ok_or(DocumentError::UnknownId {
            kind: "scalar",
            id: scalar.0,
        })?
        .unit;
    let scale = match unit {
        ScalarUnit::Length => document.model_scale(),
        ScalarUnit::Angle | ScalarUnit::Parameter => 1.0,
    };
    sketch.add_fixed_scalar(property, target, scale)?;
    Ok(())
}

pub(crate) fn add_parameter_fixed_scalar(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    property: DocumentScalarPropertyRef,
    target: f64,
) -> Result<crate::SketchConstraintId, DocumentError> {
    document.validate_dimensionless_parameter_property(property)?;
    document.validate_parameter_scalar_value(property.scalar, target)?;
    let runtime = runtime_scalar_ref(document, mappings, property.scalar)?;
    Ok(sketch.add_fixed_scalar(runtime, target, 1.0)?)
}

fn is_dimensionless_runtime_curve_scalar(
    document: &SketchDocument,
    scalar: DesignScalarId,
) -> bool {
    document
        .curves()
        .iter()
        .any(|curve| curve_has_dimensionless_runtime_scalar(&curve.definition, scalar))
}

fn curve_has_dimensionless_runtime_scalar(
    definition: &CurveDefinition,
    scalar: DesignScalarId,
) -> bool {
    match definition {
        CurveDefinition::Ellipse {
            minor_axis_ratio, ..
        }
        | CurveDefinition::EllipticalArc {
            minor_axis_ratio, ..
        } => *minor_axis_ratio == scalar,
        CurveDefinition::RationalQuadraticConic { middle_weight, .. } => *middle_weight == scalar,
        CurveDefinition::Nurbs {
            weights,
            gauge_weight,
            ..
        } => weights.contains(&scalar) && *gauge_weight != scalar,
        _ => false,
    }
}

fn lower_executable_scalar_sources(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    sketch: &mut crate::Sketch,
    sources: &[DocumentScalarSource],
) -> Result<(), DocumentError> {
    for source in sources {
        match source.relation {
            DocumentScalarRelation::Fixed { property, target } => {
                add_fixed_document_scalar(document, mappings, sketch, property.scalar, target)?;
            }
            DocumentScalarRelation::Equal { first, second } => {
                let residual_scale = characteristic_scale(document.model_scale(), first.unit)?;
                sketch.add_equal_scalar(
                    runtime_scalar_ref(document, mappings, first.scalar)?,
                    runtime_scalar_ref(document, mappings, second.scalar)?,
                    residual_scale,
                )?;
            }
        }
    }
    Ok(())
}

fn runtime_scalar_ref(
    document: &SketchDocument,
    mappings: &crate::DocumentRuntimeMap,
    scalar: DesignScalarId,
) -> Result<crate::SketchScalarRef, DocumentError> {
    for curve in document.curves() {
        let mapped =
            match &curve.definition {
                CurveDefinition::Circle { radius, .. } if *radius == scalar => mappings
                    .runtime_circle(curve.id)
                    .map(crate::SketchScalarRef::CircleRadius),
                CurveDefinition::CircularArc { radius, .. } if *radius == scalar => mappings
                    .runtime_arc(curve.id)
                    .map(crate::SketchScalarRef::ArcRadius),
                CurveDefinition::CircularArc {
                    start_angle,
                    end_angle,
                    ..
                } if *start_angle == scalar || *end_angle == scalar => mappings
                    .runtime_arc(curve.id)
                    .map(|arc| crate::SketchScalarRef::ArcAngle {
                        arc,
                        endpoint: if *start_angle == scalar {
                            crate::ArcAngleEndpoint::Start
                        } else {
                            crate::ArcAngleEndpoint::End
                        },
                    }),
                CurveDefinition::Ellipse {
                    minor_axis_ratio, ..
                }
                | CurveDefinition::EllipticalArc {
                    minor_axis_ratio, ..
                } if *minor_axis_ratio == scalar => mappings.runtime_conic(curve.id).map(|conic| {
                    crate::SketchScalarRef::ConicScalar {
                        conic,
                        role: crate::ConicScalarRole::MinorAxisRatio,
                    }
                }),
                CurveDefinition::RationalQuadraticConic { middle_weight, .. }
                    if *middle_weight == scalar =>
                {
                    mappings.runtime_conic(curve.id).map(|conic| {
                        crate::SketchScalarRef::ConicScalar {
                            conic,
                            role: crate::ConicScalarRole::MiddleWeight,
                        }
                    })
                }
                CurveDefinition::HyperbolaSegment { semi_conjugate, .. }
                    if *semi_conjugate == scalar =>
                {
                    mappings.runtime_conic(curve.id).map(|conic| {
                        crate::SketchScalarRef::ConicScalar {
                            conic,
                            role: crate::ConicScalarRole::SemiConjugate,
                        }
                    })
                }
                CurveDefinition::Nurbs {
                    weights,
                    gauge_weight,
                    ..
                } if weights.contains(&scalar) && *gauge_weight != scalar => mappings
                    .runtime_nurbs(curve.id)
                    .map(|nurbs| crate::SketchScalarRef::NurbsWeight {
                        nurbs,
                        control_index: weights
                            .iter()
                            .position(|candidate| *candidate == scalar)
                            .expect("matched NURBS weight"),
                    }),
                _ => None,
            };
        if let Some(mapped) = mapped {
            return Ok(mapped);
        }
    }
    invalid(
        "executable scalar property",
        "scalar is not an active mapped curve property",
    )
}

fn independently_validate_planar_catalog(
    catalog: &DocumentSemanticSourceCatalog,
    document: &SketchDocument,
    request: SketchSolveRequest,
) -> Result<(), DocumentError> {
    let lowered = document.lower()?;
    let (mut sketch, mappings) = lowered.into_parts();
    let mut pending = Vec::with_capacity(catalog.planar_sources.len());
    for source in &catalog.planar_sources {
        let before = runtime_sketch_sources(&sketch).collect::<Vec<_>>();
        let templates = lower_planar_source(document, &mappings, &mut sketch, source)?;
        let sources = runtime_sketch_sources(&sketch)
            .filter(|runtime_source| !before.contains(runtime_source))
            .collect::<Vec<_>>();
        pending.push((source, templates, sources));
    }
    lower_executable_scalar_sources(document, &mappings, &mut sketch, &catalog.sources)?;
    let compiled = sketch.compile(request)?;
    let snapshot =
        compiled
            .problem()
            .audit_snapshot()
            .map_err(|error| DocumentError::InvalidField {
                field: "semantic planar acceptance",
                message: error.to_string(),
            })?;
    for (_, templates, runtime_sources) in pending {
        let mut row_count = 0usize;
        for runtime_source in runtime_sources {
            let mapping = compiled
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == runtime_source)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "semantic planar acceptance",
                    message: "freshly compiled source has no mapping".into(),
                })?;
            let core_source =
                mapping
                    .core_source_id
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "semantic planar acceptance",
                        message: "freshly compiled source has no core source".into(),
                    })?;
            let source_audit = snapshot
                .sources
                .iter()
                .find(|audit| audit.source_id == core_source)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "semantic planar acceptance",
                    message: "freshly compiled source has no audit rows".into(),
                })?;
            row_count = row_count.saturating_add(source_audit.rows.len());
            if source_audit.rows.iter().any(|row| {
                row.evaluation_status != geosolve_core::AuditEvaluationStatus::Evaluated
                    || !row.raw_residual.is_finite()
                    || !row.normalized_residual.is_finite()
                    || row.normalized_residual.abs() > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            }) {
                return invalid(
                    "semantic planar acceptance",
                    "projected document fails fresh independent row evaluation",
                );
            }
        }
        if row_count != templates.len() {
            return invalid(
                "semantic planar acceptance",
                "fresh row ownership does not match equation templates",
            );
        }
    }
    Ok(())
}

fn independently_validate_planar_catalog_controlled(
    catalog: &DocumentSemanticSourceCatalog,
    document: &SketchDocument,
    request: SketchSolveRequest,
    controller: &mut OperationController,
) -> Result<bool, DocumentError> {
    let Some(lowered) = document.lower_with_controller(controller)? else {
        return Ok(false);
    };
    let (mut sketch, mappings) = lowered.into_parts();
    let mut pending = Vec::with_capacity(catalog.planar_sources.len());
    for source in &catalog.planar_sources {
        if controller
            .charge(
                OperationWorkCounter::DocumentLoweringItems,
                1,
                OperationCheckpoint::DocumentLowering,
            )
            .is_err()
        {
            return Ok(false);
        }
        let before = runtime_sketch_sources(&sketch).collect::<Vec<_>>();
        let templates = lower_planar_source(document, &mappings, &mut sketch, source)?;
        let sources = runtime_sketch_sources(&sketch)
            .filter(|runtime_source| !before.contains(runtime_source))
            .collect::<Vec<_>>();
        pending.push((templates, sources));
    }
    if controller
        .charge(
            OperationWorkCounter::DocumentLoweringItems,
            catalog.sources.len(),
            OperationCheckpoint::DocumentLowering,
        )
        .is_err()
    {
        return Ok(false);
    }
    lower_executable_scalar_sources(document, &mappings, &mut sketch, &catalog.sources)?;
    let Some(compiled) = sketch.compile_with_controller(request, controller)? else {
        return Ok(false);
    };
    let Some(snapshot) = compiled
        .problem()
        .audit_snapshot_with_controller(controller)
        .map_err(|error| DocumentError::InvalidField {
            field: "semantic planar acceptance",
            message: error.to_string(),
        })?
    else {
        return Ok(false);
    };
    for (templates, runtime_sources) in pending {
        let mut row_count = 0usize;
        for runtime_source in runtime_sources {
            let mapping = compiled
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == runtime_source)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "semantic planar acceptance",
                    message: "freshly compiled source has no mapping".into(),
                })?;
            let core_source =
                mapping
                    .core_source_id
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "semantic planar acceptance",
                        message: "freshly compiled source has no core source".into(),
                    })?;
            let source_audit = snapshot
                .sources
                .iter()
                .find(|audit| audit.source_id == core_source)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "semantic planar acceptance",
                    message: "freshly compiled source has no audit rows".into(),
                })?;
            row_count = row_count.saturating_add(source_audit.rows.len());
            if source_audit.rows.iter().any(|row| {
                row.evaluation_status != geosolve_core::AuditEvaluationStatus::Evaluated
                    || !row.raw_residual.is_finite()
                    || !row.normalized_residual.is_finite()
                    || row.normalized_residual.abs() > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            }) {
                return invalid(
                    "semantic planar acceptance",
                    "projected document fails fresh independent row evaluation",
                );
            }
        }
        if row_count != templates.len() {
            return invalid(
                "semantic planar acceptance",
                "fresh row ownership does not match equation templates",
            );
        }
    }
    Ok(controller
        .checkpoint(OperationCheckpoint::AfterFinalValidation)
        .is_ok())
}

fn grouped_planar_audit(
    solve: &SketchSolveResult,
    pending: Vec<(
        &DocumentPlanarSource,
        Vec<&'static str>,
        Vec<crate::SketchSource>,
    )>,
) -> Result<Vec<DocumentPlanarAudit>, DocumentError> {
    pending
        .into_iter()
        .map(|(source, equation_templates, runtime_sources)| {
            let mut rows = Vec::new();
            for runtime_source in runtime_sources {
                let mapping = solve
                    .source_mappings
                    .iter()
                    .find(|mapping| mapping.source == runtime_source)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "semantic planar audit",
                        message: "runtime constraint has no accepted source mapping".into(),
                    })?;
                let core_source =
                    mapping
                        .core_source_id
                        .ok_or_else(|| DocumentError::InvalidField {
                            field: "semantic planar audit",
                            message: "executable runtime constraint has no core source".into(),
                        })?;
                let core_audit = solve
                    .display_audit
                    .sources
                    .iter()
                    .find(|audit| audit.source_id == core_source)
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "semantic planar audit",
                        message: "accepted core source has no audit rows".into(),
                    })?;
                if core_audit.rows.iter().any(|row| {
                    row.evaluation_status != geosolve_core::AuditEvaluationStatus::Evaluated
                        || !row.raw_residual.is_finite()
                        || !row.normalized_residual.is_finite()
                }) {
                    return invalid(
                        "semantic planar audit",
                        "accepted source audit must be complete and finite",
                    );
                }
                rows.extend(core_audit.rows.clone());
            }
            if equation_templates.len() != rows.len() {
                return invalid(
                    "semantic planar audit",
                    "every emitted row must have one equation template",
                );
            }
            Ok(DocumentPlanarAudit {
                source_id: source.source_id,
                source_label: source.label.clone(),
                relation: source.relation.clone(),
                equation_templates,
                rows,
            })
        })
        .collect()
}

fn runtime_sketch_sources(
    sketch: &crate::Sketch,
) -> impl Iterator<Item = crate::SketchSource> + '_ {
    sketch
        .constraints()
        .map(|(id, _)| crate::SketchSource::Constraint(id))
        .chain(
            sketch
                .dimensions()
                .map(|(id, _)| crate::SketchSource::Dimension(id)),
        )
}

fn independently_validated_scalar_audit(
    catalog: &DocumentSemanticSourceCatalog,
    document: &SketchDocument,
) -> Result<Vec<LoweredDocumentScalarSource>, DocumentError> {
    catalog
        .sources
        .iter()
        .map(|source| {
            let evidence = source.lower(document)?;
            if !evidence.independently_validated(
                document,
                source,
                crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE,
            )? {
                return invalid(
                    "semantic scalar acceptance",
                    "accepted geometry does not satisfy the scalar source",
                );
            }
            Ok(evidence)
        })
        .collect()
}

fn invalid<T>(field: &'static str, message: &str) -> Result<T, DocumentError> {
    Err(DocumentError::InvalidField {
        field,
        message: message.into(),
    })
}
