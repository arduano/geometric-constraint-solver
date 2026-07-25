// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationWorkCounter, ResidualCategory,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::{
    ContactNeighborhood, CurveDefinition, DesignScalarId, DocumentAngleOrientation,
    DocumentBSplineForm, DocumentCoordinateAxis, DocumentCurveNormalSide, DocumentCurveSpanRef,
    DocumentError, DocumentId, DocumentLineSide, DocumentLineSupportRef, DocumentSourceId,
    ScalarDomain, ScalarUnit, SketchDocument,
};

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
        if self.sources.len() > crate::MAX_DOCUMENT_OBJECTS {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self.sources.len(),
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
            .collect::<Vec<_>>();
        document.register_semantic_catalog(catalog.catalog_id, &source_ids)?;
        catalog.validate(document)?;
        Ok(catalog)
    }

    fn validate_unregistered(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        self.validate_header(document)?;
        if self.sources.len() > crate::MAX_DOCUMENT_OBJECTS {
            return Err(DocumentError::ResourceLimit {
                resource: "semantic sources",
                actual: self.sources.len(),
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
        Ok(())
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

fn invalid<T>(field: &'static str, message: &str) -> Result<T, DocumentError> {
    Err(DocumentError::InvalidField {
        field,
        message: message.into(),
    })
}
