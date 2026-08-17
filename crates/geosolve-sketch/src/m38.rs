// SPDX-License-Identifier: GPL-3.0-or-later

//! Draft-v5 dimension and persistent-measurement catalog.
//!
//! This envelope is deliberately separate from frozen sketch JSON v1-v4.  It owns
//! persistent identities through the document allocator while leaving the frozen
//! dimension language byte compatible.

#![allow(
    clippy::many_single_char_names,
    clippy::missing_errors_doc,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::collections::HashSet;

use geosolve_core::{
    AuditRowSnapshot, OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationWorkCounter, SolverConfig,
};
use serde::{Deserialize, Serialize};

use crate::{
    ContactId, CurveDefinition, CurveId, DimensionMode, DocumentAngleOrientation,
    DocumentConicFeature, DocumentConicMeasurement, DocumentCoordinateAxis,
    DocumentCurveMeasurementKind, DocumentCurveSpanRef, DocumentDimensionMode,
    DocumentDirectionSense, DocumentError, DocumentId, DocumentLineSide, DocumentLineSupportRef,
    DocumentPointRef, DocumentScalarUnit, DocumentSourceId, RetainedSketchDocumentSession,
    SketchDocument, SketchSolveRequest, SketchSolveResult,
};

const VERSION: u32 = 1;
const ABSOLUTE_LENGTH_TOLERANCE_FACTOR: f64 = 1.0e-11;
const MAX_INTEGRATION_DEPTH: u32 = 24;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentDatumAxis {
    pub origin: DocumentPointRef,
    pub axis: DocumentCoordinateAxis,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentBoundedCurveInterval {
    pub support: DocumentCurveSpanRef,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentM38DimensionDefinition {
    RelativeHorizontal {
        first: DocumentPointRef,
        second: DocumentPointRef,
    },
    RelativeVertical {
        first: DocumentPointRef,
        second: DocumentPointRef,
    },
    DatumCoordinate {
        point: DocumentPointRef,
        datum: DocumentDatumAxis,
    },
    PointLineDistance {
        point: DocumentPointRef,
        line: DocumentLineSupportRef,
        side: DocumentLineSide,
    },
    ParallelLineSeparation {
        first: DocumentLineSupportRef,
        second: DocumentLineSupportRef,
        side: DocumentLineSide,
    },
    TwoLineAngle {
        first: DocumentLineSupportRef,
        second: DocumentLineSupportRef,
        orientation: DocumentAngleOrientation,
        winding: i32,
    },
    ThreePointAngle {
        first: DocumentPointRef,
        vertex: DocumentPointRef,
        second: DocumentPointRef,
        orientation: DocumentAngleOrientation,
        winding: i32,
    },
    CircularSweep {
        arc: CurveId,
    },
    CircularArcLength {
        arc: CurveId,
    },
    EllipseMajorAxis {
        curve: CurveId,
    },
    EllipseMinorAxis {
        curve: CurveId,
    },
    ConicLinearEccentricity {
        curve: CurveId,
    },
    ConicFocalDistance {
        curve: CurveId,
    },
    ConicTransverseAxisLength {
        curve: CurveId,
    },
    ConicConjugateAxisLength {
        curve: CurveId,
    },
    PathLength {
        interval: DocumentBoundedCurveInterval,
    },
    EqualPathLength {
        first: DocumentBoundedCurveInterval,
        second: DocumentBoundedCurveInterval,
    },
    SegmentLength {
        line: DocumentLineSupportRef,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentMeasurementProvenance {
    AcceptedDocument { revision: u64 },
    RetainedDesign { revision: u64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentMeasurementDefinition {
    SignedCurvature {
        contact: ContactId,
    },
    UnsignedCurvature {
        contact: ContactId,
    },
    OsculatingRadius {
        contact: ContactId,
    },
    ConicProperty {
        curve: CurveId,
        property: DocumentConicMeasurement,
    },
    BoundedCurveLength {
        interval: DocumentBoundedCurveInterval,
    },
    DimensionValue {
        definition: DocumentM38DimensionDefinition,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DimensionSource {
    document_id: DocumentId,
    source_id: DocumentSourceId,
    label: String,
    definition: DocumentM38DimensionDefinition,
    mode: DocumentDimensionMode,
    target: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementSource {
    document_id: DocumentId,
    source_id: DocumentSourceId,
    label: String,
    definition: DocumentMeasurementDefinition,
    provenance: DocumentMeasurementProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentMeasurementCatalog {
    version: u32,
    document_id: DocumentId,
    catalog_id: DocumentSourceId,
    dimensions: Vec<DimensionSource>,
    measurements: Vec<MeasurementSource>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DocumentMeasurementWork {
    pub integrations: usize,
    pub derivative_evaluations: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentMeasurementAudit {
    pub source_id: DocumentSourceId,
    pub source_label: String,
    pub equation_template: &'static str,
    pub unit: DocumentScalarUnit,
    pub provenance: Option<DocumentMeasurementProvenance>,
    pub independently_evaluated: bool,
    pub rows: Vec<AuditRowSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentMeasurementValue {
    pub source_id: DocumentSourceId,
    pub value: f64,
    pub unit: DocumentScalarUnit,
    pub residual: Option<f64>,
    pub work: DocumentMeasurementWork,
    pub audit: DocumentMeasurementAudit,
}

#[derive(Clone, Debug)]
pub struct DocumentM38SolveResult {
    pub document: SketchDocument,
    pub solve_result: SketchSolveResult,
    pub audit: Vec<DocumentMeasurementValue>,
}

impl DocumentMeasurementCatalog {
    pub fn new(document: &mut SketchDocument) -> Result<Self, DocumentError> {
        let catalog_id = document.allocate_semantic_catalog_id()?;
        Ok(Self {
            version: VERSION,
            document_id: document.id(),
            catalog_id,
            dimensions: Vec::new(),
            measurements: Vec::new(),
        })
    }

    pub fn solve_document(
        &self,
        document: &SketchDocument,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<DocumentM38SolveResult, DocumentError> {
        self.validate(document)?;
        let lowered = document.lower()?;
        let (mut sketch, mappings) = lowered.into_parts();
        let mut runtime_dimensions = Vec::new();
        for source in &self.dimensions {
            if source.mode != DocumentDimensionMode::Driving {
                continue;
            }
            let runtime_dimension = match source.definition {
                DocumentM38DimensionDefinition::RelativeHorizontal { first, second } => {
                    let first = crate::semantic::runtime_point_ref(document, &mappings, first)?;
                    let second = crate::semantic::runtime_point_ref(document, &mappings, second)?;
                    sketch.add_coordinate_difference_dimension(
                        first,
                        second,
                        crate::CoordinateAxis::X,
                        source.target,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::RelativeVertical { first, second } => {
                    let first = crate::semantic::runtime_point_ref(document, &mappings, first)?;
                    let second = crate::semantic::runtime_point_ref(document, &mappings, second)?;
                    sketch.add_coordinate_difference_dimension(
                        first,
                        second,
                        crate::CoordinateAxis::Y,
                        source.target,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::DatumCoordinate { point, datum } => {
                    let axis = match datum.axis {
                        DocumentCoordinateAxis::X => crate::CoordinateAxis::X,
                        DocumentCoordinateAxis::Y => crate::CoordinateAxis::Y,
                    };
                    let first =
                        crate::semantic::runtime_point_ref(document, &mappings, datum.origin)?;
                    let second = crate::semantic::runtime_point_ref(document, &mappings, point)?;
                    sketch.add_coordinate_difference_dimension(
                        first,
                        second,
                        axis,
                        source.target,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::PointLineDistance { point, line, side } => {
                    let point = crate::semantic::runtime_point_ref(document, &mappings, point)?;
                    let source_segment = runtime_line_support(&mappings, line)?;
                    let (source_start, source_end) = sketch.segment_endpoints(source_segment)?;
                    let direction =
                        sketch.point_position(source_end)? - sketch.point_position(source_start)?;
                    let auxiliary = sketch.add_point(sketch.point_position(point)? + direction)?;
                    let target_segment = sketch.add_segment(point, auxiliary)?;
                    let mut side = effective_line_side(line.direction, side);
                    let target = if source.target < 0.0 {
                        side = opposite_line_side(side);
                        -source.target
                    } else {
                        source.target
                    };
                    sketch.add_supporting_line_offset(
                        source_segment,
                        target_segment,
                        target,
                        side,
                        crate::LineOffsetOrientation::Same,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::ParallelLineSeparation {
                    first,
                    second,
                    side,
                } => {
                    let first_segment = runtime_line_support(&mappings, first)?;
                    let second_segment = runtime_line_support(&mappings, second)?;
                    let mut side = effective_line_side(first.direction, side);
                    let target = if source.target < 0.0 {
                        side = opposite_line_side(side);
                        -source.target
                    } else {
                        source.target
                    };
                    let orientation = if first.direction == second.direction {
                        crate::LineOffsetOrientation::Same
                    } else {
                        crate::LineOffsetOrientation::Reversed
                    };
                    sketch.add_supporting_line_offset(
                        first_segment,
                        second_segment,
                        target,
                        side,
                        orientation,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::TwoLineAngle {
                    first,
                    second,
                    orientation,
                    winding,
                } => {
                    let first_segment = runtime_line_support(&mappings, first)?;
                    let second_segment = runtime_line_support(&mappings, second)?;
                    let orientation = runtime_angle_orientation(orientation);
                    let mut target = unwrapped_angle_target(source.target, winding)?;
                    if first.direction != second.direction {
                        target += match orientation {
                            crate::AngleOrientation::CounterClockwise => std::f64::consts::PI,
                            crate::AngleOrientation::Clockwise => -std::f64::consts::PI,
                        };
                    }
                    while target <= 0.0 {
                        target += std::f64::consts::TAU;
                    }
                    sketch.add_oriented_angle(
                        first_segment,
                        second_segment,
                        target,
                        orientation,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::ThreePointAngle {
                    first,
                    vertex,
                    second,
                    orientation,
                    winding,
                } => {
                    let first = crate::semantic::runtime_point_ref(document, &mappings, first)?;
                    let vertex = crate::semantic::runtime_point_ref(document, &mappings, vertex)?;
                    let second = crate::semantic::runtime_point_ref(document, &mappings, second)?;
                    let first_segment = sketch.add_segment(vertex, first)?;
                    let second_segment = sketch.add_segment(vertex, second)?;
                    let orientation = runtime_angle_orientation(orientation);
                    let mut target = unwrapped_angle_target(source.target, winding)?;
                    while target <= 0.0 {
                        target += std::f64::consts::TAU;
                    }
                    sketch.add_oriented_angle(
                        first_segment,
                        second_segment,
                        target,
                        orientation,
                        DimensionMode::Driving,
                    )?
                }
                DocumentM38DimensionDefinition::CircularSweep { arc } => sketch
                    .add_circular_sweep_dimension(
                        mappings.runtime_arc(arc).ok_or(DocumentError::UnknownId {
                            kind: "circular arc",
                            id: arc.0,
                        })?,
                        source.target,
                    )?,
                DocumentM38DimensionDefinition::CircularArcLength { arc } => sketch
                    .add_circular_arc_length_dimension(
                        mappings.runtime_arc(arc).ok_or(DocumentError::UnknownId {
                            kind: "circular arc",
                            id: arc.0,
                        })?,
                        source.target,
                    )?,
                DocumentM38DimensionDefinition::EllipseMajorAxis { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::MajorAxisLength,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::EllipseMinorAxis { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::MinorAxisLength,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::ConicLinearEccentricity { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::LinearEccentricity,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::ConicFocalDistance { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::FocalDistance,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::ConicTransverseAxisLength { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::TransverseAxisLength,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::ConicConjugateAxisLength { curve } => {
                    add_runtime_conic_dimension(
                        &mut sketch,
                        &mappings,
                        curve,
                        crate::model::M38ConicProperty::ConjugateAxisLength,
                        source.target,
                    )?
                }
                DocumentM38DimensionDefinition::PathLength { interval } => sketch
                    .add_path_length_dimension(
                        crate::document_lowering::runtime_bounded_curve(
                            &mappings,
                            interval.support.span,
                        )?,
                        interval.start,
                        interval.end,
                        source.target,
                    )?,
                DocumentM38DimensionDefinition::EqualPathLength { first, second } => sketch
                    .add_equal_path_length_dimension(
                        crate::document_lowering::runtime_bounded_curve(
                            &mappings,
                            first.support.span,
                        )?,
                        first.start,
                        first.end,
                        crate::document_lowering::runtime_bounded_curve(
                            &mappings,
                            second.support.span,
                        )?,
                        second.start,
                        second.end,
                        source.target,
                    )?,
                DocumentM38DimensionDefinition::SegmentLength { line } => sketch
                    .add_segment_length(
                        runtime_line_support(&mappings, line)?,
                        source.target.abs(),
                        DimensionMode::Driving,
                    )?,
            };
            runtime_dimensions.push((source.source_id, runtime_dimension));
        }
        let solve_result = sketch.solve(request, config)?;
        if !solve_result.accepted()
            || solve_result
                .acceptance_hard_residual_max
                .is_none_or(|value| {
                    !value.is_finite() || value > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
                })
        {
            return invalid(
                "M38 catalog solve",
                "driving rows did not produce an independently accepted state",
            );
        }
        let mut accepted = document.clone();
        accepted.project_accepted_state(&sketch, &mappings)?;
        let mut audit = self
            .dimensions
            .iter()
            .map(|source| self.evaluate_dimension(&accepted, source.source_id))
            .collect::<Result<Vec<_>, _>>()?;
        for value in &mut audit {
            let Some((_, dimension)) = runtime_dimensions
                .iter()
                .find(|(source, _)| *source == value.source_id)
            else {
                continue;
            };
            let mapping = solve_result
                .source_mappings
                .iter()
                .find(|mapping| mapping.source == crate::SketchSource::Dimension(*dimension))
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "M38 dimension audit",
                    message: "runtime dimension has no accepted source mapping".into(),
                })?;
            let core_source =
                mapping
                    .core_source_id
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "M38 dimension audit",
                        message: "driving dimension has no executable core source".into(),
                    })?;
            let source_audit = solve_result
                .display_audit
                .sources
                .iter()
                .find(|audit| audit.source_id == core_source)
                .ok_or_else(|| DocumentError::InvalidField {
                    field: "M38 dimension audit",
                    message: "accepted driving dimension has no core audit".into(),
                })?;
            if source_audit.rows.iter().any(|row| {
                row.evaluation_status != geosolve_core::AuditEvaluationStatus::Evaluated
                    || !row.raw_residual.is_finite()
                    || !row.normalized_residual.is_finite()
            }) {
                return invalid(
                    "M38 dimension audit",
                    "accepted driving rows must be completely and finitely evaluated",
                );
            }
            value.audit.rows.clone_from(&source_audit.rows);
        }
        if audit.iter().any(|value| {
            value.residual.is_some_and(|residual| {
                !residual.is_finite()
                    || residual.abs() / accepted.model_scale()
                        > crate::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            })
        }) {
            return invalid(
                "M38 catalog solve",
                "independent dimension validation rejected the candidate",
            );
        }
        Ok(DocumentM38SolveResult {
            document: accepted,
            solve_result,
            audit,
        })
    }

    pub fn add_dimension(
        &mut self,
        document: &mut SketchDocument,
        label: impl Into<String>,
        definition: DocumentM38DimensionDefinition,
        mode: DocumentDimensionMode,
        target: f64,
    ) -> Result<DocumentSourceId, DocumentError> {
        self.validate_header(document)?;
        let label = checked_label(label.into())?;
        if !target.is_finite() {
            return invalid("M38 dimension target", "target must be finite");
        }
        validate_dimension(document, &definition)?;
        let source_id = document.allocate_semantic_source_id(self.catalog_id)?;
        self.dimensions.push(DimensionSource {
            document_id: self.document_id,
            source_id,
            label,
            definition,
            mode,
            target,
        });
        Ok(source_id)
    }

    pub fn add_measurement(
        &mut self,
        document: &mut SketchDocument,
        label: impl Into<String>,
        definition: DocumentMeasurementDefinition,
        provenance: DocumentMeasurementProvenance,
    ) -> Result<DocumentSourceId, DocumentError> {
        self.validate_header(document)?;
        let label = checked_label(label.into())?;
        validate_measurement(document, &definition)?;
        let source_id = document.allocate_semantic_source_id(self.catalog_id)?;
        self.measurements.push(MeasurementSource {
            document_id: self.document_id,
            source_id,
            label,
            definition,
            provenance,
        });
        Ok(source_id)
    }

    pub fn evaluate_dimension(
        &self,
        document: &SketchDocument,
        source: DocumentSourceId,
    ) -> Result<DocumentMeasurementValue, DocumentError> {
        let mut controller = OperationController::new(OperationControl::default());
        self.evaluate_dimension_inner(document, source, &mut controller)?
            .ok_or_else(|| DocumentError::InvalidField {
                field: "M38 dimension work",
                message: "unlimited evaluation was interrupted".into(),
            })
    }

    pub fn evaluate_measurement(
        &self,
        session: &RetainedSketchDocumentSession,
        source: DocumentSourceId,
    ) -> Result<DocumentMeasurementValue, DocumentError> {
        let document = self.measurement_document(session, source)?;
        let mut controller = OperationController::new(OperationControl::default());
        self.evaluate_measurement_inner(document, source, &mut controller)?
            .ok_or_else(|| DocumentError::InvalidField {
                field: "M38 measurement work",
                message: "unlimited evaluation was interrupted".into(),
            })
    }

    pub fn evaluate_measurement_controlled(
        &self,
        session: &RetainedSketchDocumentSession,
        source: DocumentSourceId,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentMeasurementValue>, DocumentError> {
        let document = self.measurement_document(session, source)?;
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let Some(value) = self.evaluate_measurement_inner(document, source, &mut controller)?
        else {
            return Ok(controller.outcome_unchecked());
        };
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
            || controller
                .checkpoint(OperationCheckpoint::AfterFinalValidation)
                .is_err()
            || controller
                .checkpoint(OperationCheckpoint::BeforeCommit)
                .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(value))
    }

    fn measurement_document<'a>(
        &self,
        session: &'a RetainedSketchDocumentSession,
        id: DocumentSourceId,
    ) -> Result<&'a SketchDocument, DocumentError> {
        let source = self.measurements.iter().find(|s| s.source_id == id).ok_or(
            DocumentError::UnknownId {
                kind: "persistent measurement",
                id: id.0,
            },
        )?;
        let (document, actual_revision) = match source.provenance {
            DocumentMeasurementProvenance::RetainedDesign { revision: _ } => (
                session.design_document(),
                session.design_identity().revision().get(),
            ),
            DocumentMeasurementProvenance::AcceptedDocument { revision: _ } => {
                let accepted =
                    session
                        .accepted_state()
                        .ok_or_else(|| DocumentError::InvalidField {
                            field: "measurement provenance",
                            message: "the requested accepted document is unavailable".into(),
                        })?;
                (accepted.document(), accepted.identity().revision().get())
            }
        };
        let expected_revision = match source.provenance {
            DocumentMeasurementProvenance::AcceptedDocument { revision }
            | DocumentMeasurementProvenance::RetainedDesign { revision } => revision,
        };
        if document.id() != self.document_id || actual_revision != expected_revision {
            return invalid(
                "measurement provenance",
                "persisted provenance does not match the current lifecycle view",
            );
        }
        Ok(document)
    }

    pub fn to_canonical_json(&self) -> Result<String, DocumentError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(document: &mut SketchDocument, input: &str) -> Result<Self, DocumentError> {
        if input.len() > crate::MAX_DOCUMENT_JSON_BYTES {
            return Err(DocumentError::ResourceLimit {
                resource: "measurement catalog JSON bytes",
                actual: input.len(),
                limit: crate::MAX_DOCUMENT_JSON_BYTES,
            });
        }
        let value: Self = serde_json::from_str(input)?;
        value.validate_unregistered(document)?;
        let ids = value
            .dimensions
            .iter()
            .map(|v| v.source_id)
            .chain(value.measurements.iter().map(|v| v.source_id))
            .collect::<Vec<_>>();
        let mut candidate = document.clone();
        candidate.register_semantic_catalog(value.catalog_id, &ids)?;
        value.validate(&candidate)?;
        *document = candidate;
        Ok(value)
    }

    pub fn validate(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        self.validate_header(document)?;
        if document.semantic_reservation_owner(self.catalog_id) != Some(self.catalog_id) {
            return invalid("measurement catalog", "catalog reservation is missing");
        }
        let mut seen = HashSet::new();
        let mut previous_dimension = None;
        let mut previous_measurement = None;
        for source in self
            .dimensions
            .iter()
            .map(|s| (s.document_id, s.source_id, &s.label, true))
            .chain(
                self.measurements
                    .iter()
                    .map(|s| (s.document_id, s.source_id, &s.label, false)),
            )
        {
            let previous = if source.3 {
                &mut previous_dimension
            } else {
                &mut previous_measurement
            };
            if source.0 != self.document_id
                || document.semantic_reservation_owner(source.1) != Some(self.catalog_id)
                || previous.is_some_and(|p| p >= source.1)
                || !seen.insert(source.1)
            {
                return invalid(
                    "measurement source identity",
                    "sources must be document-bound, reserved and ordered",
                );
            }
            checked_label(source.2.clone())?;
            *previous = Some(source.1);
        }
        for source in &self.dimensions {
            validate_dimension(document, &source.definition)?;
        }
        for source in &self.measurements {
            validate_measurement(document, &source.definition)?;
        }
        Ok(())
    }

    fn validate_unregistered(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        self.validate_header(document)?;
        if self.catalog_id.0 >= document.allocator_cursor()
            || document.element(self.catalog_id.0).is_some()
        {
            return invalid(
                "measurement catalog identity",
                "catalog identity was not reserved by the persisted document",
            );
        }
        let mut seen = HashSet::new();
        for ids in [
            self.dimensions
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>(),
            self.measurements
                .iter()
                .map(|s| s.source_id)
                .collect::<Vec<_>>(),
        ] {
            let mut previous = None;
            for id in ids {
                if id.0 >= document.allocator_cursor()
                    || document.element(id.0).is_some()
                    || previous.is_some_and(|p| p >= id)
                    || !seen.insert(id)
                {
                    return invalid(
                        "measurement source identity",
                        "source identities must be unique reserved and ordered",
                    );
                }
                previous = Some(id);
            }
        }
        for source in &self.dimensions {
            validate_dimension(document, &source.definition)?;
        }
        for source in &self.measurements {
            validate_measurement(document, &source.definition)?;
        }
        Ok(())
    }

    fn validate_header(&self, document: &SketchDocument) -> Result<(), DocumentError> {
        if self.version != VERSION {
            return invalid("measurement catalog version", "unsupported version");
        }
        if self.document_id != document.id() {
            return invalid(
                "measurement catalog document",
                "catalog belongs to another document",
            );
        }
        Ok(())
    }

    fn evaluate_dimension_inner(
        &self,
        document: &SketchDocument,
        id: DocumentSourceId,
        controller: &mut OperationController,
    ) -> Result<Option<DocumentMeasurementValue>, DocumentError> {
        self.validate(document)?;
        let source =
            self.dimensions
                .iter()
                .find(|s| s.source_id == id)
                .ok_or(DocumentError::UnknownId {
                    kind: "M38 dimension",
                    id: id.0,
                })?;
        let Some((value, unit, work, template)) =
            evaluate_dimension_value(document, &source.definition, controller)?
        else {
            return Ok(None);
        };
        let target = effective_dimension_target(&source.definition, source.target)?;
        let residual = (source.mode == DocumentDimensionMode::Driving).then_some(value - target);
        Ok(Some(result(
            source.source_id,
            &source.label,
            value,
            unit,
            residual,
            work,
            template,
            None,
        )))
    }

    fn evaluate_measurement_inner(
        &self,
        document: &SketchDocument,
        id: DocumentSourceId,
        controller: &mut OperationController,
    ) -> Result<Option<DocumentMeasurementValue>, DocumentError> {
        self.validate(document)?;
        let source = self.measurements.iter().find(|s| s.source_id == id).ok_or(
            DocumentError::UnknownId {
                kind: "persistent measurement",
                id: id.0,
            },
        )?;
        let (value, unit, work, template) = match &source.definition {
            DocumentMeasurementDefinition::SignedCurvature { contact } => (
                document
                    .measure_curve_contact(*contact, DocumentCurveMeasurementKind::SignedCurvature)
                    .map_err(field_error("curve measurement"))?,
                DocumentScalarUnit::Curvature,
                DocumentMeasurementWork::default(),
                "signed_curvature(curve_contact)",
            ),
            DocumentMeasurementDefinition::UnsignedCurvature { contact } => (
                document
                    .measure_curve_contact(
                        *contact,
                        DocumentCurveMeasurementKind::UnsignedCurvature,
                    )
                    .map_err(field_error("curve measurement"))?,
                DocumentScalarUnit::Curvature,
                DocumentMeasurementWork::default(),
                "unsigned_curvature(curve_contact)",
            ),
            DocumentMeasurementDefinition::OsculatingRadius { contact } => (
                document
                    .measure_curve_contact(*contact, DocumentCurveMeasurementKind::OsculatingRadius)
                    .map_err(field_error("curve measurement"))?,
                DocumentScalarUnit::Length,
                DocumentMeasurementWork::default(),
                "osculating_radius(curve_contact)",
            ),
            DocumentMeasurementDefinition::ConicProperty { curve, property } => (
                document
                    .measure_conic(*curve, *property)
                    .map_err(field_error("conic measurement"))?,
                DocumentScalarUnit::Length,
                DocumentMeasurementWork::default(),
                "conic_property(curve)",
            ),
            DocumentMeasurementDefinition::BoundedCurveLength { interval } => {
                let Some((v, w)) = bounded_length(document, *interval, controller)? else {
                    return Ok(None);
                };
                (
                    v,
                    DocumentScalarUnit::Length,
                    w,
                    "integral(start,end, norm(curve'(t)), dt)",
                )
            }
            DocumentMeasurementDefinition::DimensionValue { definition } => {
                let Some((v, u, w, t)) =
                    evaluate_dimension_value(document, definition, controller)?
                else {
                    return Ok(None);
                };
                (v, u, w, t)
            }
        };
        Ok(Some(result(
            source.source_id,
            &source.label,
            value,
            unit,
            None,
            work,
            template,
            Some(source.provenance),
        )))
    }
}

fn result(
    id: DocumentSourceId,
    label: &str,
    value: f64,
    unit: DocumentScalarUnit,
    residual: Option<f64>,
    work: DocumentMeasurementWork,
    template: &'static str,
    provenance: Option<DocumentMeasurementProvenance>,
) -> DocumentMeasurementValue {
    DocumentMeasurementValue {
        source_id: id,
        value,
        unit,
        residual,
        work,
        audit: DocumentMeasurementAudit {
            source_id: id,
            source_label: label.into(),
            equation_template: template,
            unit,
            provenance,
            independently_evaluated: true,
            rows: Vec::new(),
        },
    }
}

fn effective_dimension_target(
    definition: &DocumentM38DimensionDefinition,
    target: f64,
) -> Result<f64, DocumentError> {
    match definition {
        DocumentM38DimensionDefinition::TwoLineAngle { winding, .. }
        | DocumentM38DimensionDefinition::ThreePointAngle { winding, .. } => {
            unwrapped_angle_target(target, *winding)
        }
        _ => Ok(target),
    }
}

fn unwrapped_angle_target(target: f64, winding: i32) -> Result<f64, DocumentError> {
    let value = target + f64::from(winding) * std::f64::consts::TAU;
    if value.is_finite() {
        Ok(value)
    } else {
        invalid("M38 angle target", "unwrapped target must be finite")
    }
}

fn add_runtime_conic_dimension(
    sketch: &mut crate::Sketch,
    mappings: &crate::DocumentRuntimeMap,
    curve: CurveId,
    property: crate::model::M38ConicProperty,
    target: f64,
) -> Result<crate::SketchDimensionId, DocumentError> {
    sketch
        .add_conic_property_dimension(
            mappings
                .runtime_conic(curve)
                .ok_or(DocumentError::UnknownId {
                    kind: "conic",
                    id: curve.0,
                })?,
            property,
            target,
        )
        .map_err(DocumentError::from)
}

fn evaluate_dimension_value(
    document: &SketchDocument,
    definition: &DocumentM38DimensionDefinition,
    controller: &mut OperationController,
) -> Result<
    Option<(
        f64,
        DocumentScalarUnit,
        DocumentMeasurementWork,
        &'static str,
    )>,
    DocumentError,
> {
    use DocumentM38DimensionDefinition as D;
    let zero = DocumentMeasurementWork::default();
    let value = match definition {
        D::RelativeHorizontal { first, second } => (
            point_value(document, *second)?[0] - point_value(document, *first)?[0],
            DocumentScalarUnit::Length,
            zero,
            "second.x - first.x",
        ),
        D::RelativeVertical { first, second } => (
            point_value(document, *second)?[1] - point_value(document, *first)?[1],
            DocumentScalarUnit::Length,
            zero,
            "second.y - first.y",
        ),
        D::DatumCoordinate { point, datum } => {
            let axis = axis_index(datum.axis);
            (
                point_value(document, *point)?[axis] - point_value(document, datum.origin)?[axis],
                DocumentScalarUnit::Length,
                zero,
                "point.axis - datum.origin.axis",
            )
        }
        D::PointLineDistance { point, line, side } => {
            let (a, u) = line_value(document, *line)?;
            (
                side_sign(*side) * cross(u, sub(point_value(document, *point)?, a)),
                DocumentScalarUnit::Length,
                zero,
                "side * cross(unit(line), point-line.start)",
            )
        }
        D::ParallelLineSeparation {
            first,
            second,
            side,
        } => {
            let (a, u) = line_value(document, *first)?;
            let (b, v) = line_value(document, *second)?;
            if cross(u, v).abs() > 1e-9 {
                return invalid("parallel line separation", "supports are not parallel");
            }
            (
                side_sign(*side) * cross(u, sub(b, a)),
                DocumentScalarUnit::Length,
                zero,
                "parallel(first,second); side * cross(unit(first), second.start-first.start)",
            )
        }
        D::TwoLineAngle {
            first,
            second,
            orientation,
            winding,
        } => {
            let (_, a) = line_value(document, *first)?;
            let (_, b) = line_value(document, *second)?;
            (
                angle(a, b, *orientation, *winding),
                DocumentScalarUnit::Angle,
                zero,
                "directed_angle(first, second, orientation, winding)",
            )
        }
        D::ThreePointAngle {
            first,
            vertex,
            second,
            orientation,
            winding,
        } => {
            let v = point_value(document, *vertex)?;
            let a = unit(sub(point_value(document, *first)?, v))?;
            let b = unit(sub(point_value(document, *second)?, v))?;
            (
                angle(a, b, *orientation, *winding),
                DocumentScalarUnit::Angle,
                zero,
                "directed_angle(first-vertex, second-vertex, orientation, winding)",
            )
        }
        D::CircularSweep { arc } => (
            arc_data(document, *arc)?.1,
            DocumentScalarUnit::Angle,
            zero,
            "explicit_signed_arc_sweep",
        ),
        D::CircularArcLength { arc } => {
            let (r, sweep) = arc_data(document, *arc)?;
            (
                r * sweep.abs(),
                DocumentScalarUnit::Length,
                zero,
                "radius * abs(explicit_signed_arc_sweep)",
            )
        }
        D::EllipseMajorAxis { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::MajorAxisLength)
                .map_err(field_error("ellipse axis"))?,
            DocumentScalarUnit::Length,
            zero,
            "ellipse_major_axis_length",
        ),
        D::EllipseMinorAxis { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::MinorAxisLength)
                .map_err(field_error("ellipse axis"))?,
            DocumentScalarUnit::Length,
            zero,
            "ellipse_minor_axis_length",
        ),
        D::ConicLinearEccentricity { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::LinearEccentricity)
                .map_err(field_error("conic property"))?,
            DocumentScalarUnit::Length,
            zero,
            "conic_linear_eccentricity",
        ),
        D::ConicFocalDistance { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::FocalDistance)
                .map_err(field_error("conic property"))?,
            DocumentScalarUnit::Length,
            zero,
            "conic_focal_distance",
        ),
        D::ConicTransverseAxisLength { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::TransverseAxisLength)
                .map_err(field_error("conic property"))?,
            DocumentScalarUnit::Length,
            zero,
            "conic_transverse_axis_length",
        ),
        D::ConicConjugateAxisLength { curve } => (
            document
                .measure_conic(*curve, DocumentConicMeasurement::ConjugateAxisLength)
                .map_err(field_error("conic property"))?,
            DocumentScalarUnit::Length,
            zero,
            "conic_conjugate_axis_length",
        ),
        D::PathLength { interval } => {
            let Some((v, w)) = bounded_length(document, *interval, controller)? else {
                return Ok(None);
            };
            (
                v,
                DocumentScalarUnit::Length,
                w,
                "integral(start,end,norm(curve'(t)),dt)",
            )
        }
        D::EqualPathLength { first, second } => {
            let Some((a, mut w)) = bounded_length(document, *first, controller)? else {
                return Ok(None);
            };
            let Some((b, other)) = bounded_length(document, *second, controller)? else {
                return Ok(None);
            };
            w.integrations = w.integrations.saturating_add(other.integrations);
            w.derivative_evaluations = w
                .derivative_evaluations
                .saturating_add(other.derivative_evaluations);
            (
                a - b,
                DocumentScalarUnit::Length,
                w,
                "path_length(first)-path_length(second)",
            )
        }
        D::SegmentLength { line } => {
            let span = line.support_span();
            let a = document
                .evaluate_curve_jet(span, 0.0)
                .map_err(field_error("segment length"))?
                .position;
            let b = document
                .evaluate_curve_jet(span, 1.0)
                .map_err(field_error("segment length"))?
                .position;
            (
                (b - a).norm(),
                DocumentScalarUnit::Length,
                zero,
                "norm(segment.end-segment.start)",
            )
        }
    };
    if !value.0.is_finite() {
        return invalid("M38 dimension value", "evaluation was non-finite");
    }
    Ok(Some(value))
}

trait LineSpan {
    fn support_span(self) -> crate::CurveSpan;
}
impl LineSpan for DocumentLineSupportRef {
    fn support_span(self) -> crate::CurveSpan {
        self.span
    }
}

fn validate_dimension(
    document: &SketchDocument,
    value: &DocumentM38DimensionDefinition,
) -> Result<(), DocumentError> {
    use DocumentM38DimensionDefinition as D;
    match value {
        D::RelativeHorizontal { first, second } | D::RelativeVertical { first, second } => {
            distinct_points(document, *first, *second)?;
        }
        D::DatumCoordinate { point, datum } => {
            distinct_points(document, *point, datum.origin)?;
        }
        D::PointLineDistance { point, line, .. } => {
            document.validate_point_ref(*point)?;
            document.validate_line_support_ref(*line)?;
        }
        D::ParallelLineSeparation { first, second, .. } | D::TwoLineAngle { first, second, .. } => {
            document.validate_line_support_ref(*first)?;
            document.validate_line_support_ref(*second)?;
            if first == second {
                return invalid("dimension supports", "operands must be distinct");
            }
        }
        D::ThreePointAngle {
            first,
            vertex,
            second,
            ..
        } => {
            distinct_points(document, *first, *vertex)?;
            distinct_points(document, *vertex, *second)?;
            distinct_points(document, *first, *second)?;
        }
        D::CircularSweep { arc } | D::CircularArcLength { arc } => {
            arc_data(document, *arc)?;
        }
        D::EllipseMajorAxis { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::MajorAxisLength)
                .map_err(field_error("ellipse axis"))?;
        }
        D::EllipseMinorAxis { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::MinorAxisLength)
                .map_err(field_error("ellipse axis"))?;
        }
        D::ConicLinearEccentricity { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::LinearEccentricity)
                .map_err(field_error("conic property"))?;
        }
        D::ConicFocalDistance { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::FocalDistance)
                .map_err(field_error("conic property"))?;
        }
        D::ConicTransverseAxisLength { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::TransverseAxisLength)
                .map_err(field_error("conic property"))?;
        }
        D::ConicConjugateAxisLength { curve } => {
            document
                .measure_conic(*curve, DocumentConicMeasurement::ConjugateAxisLength)
                .map_err(field_error("conic property"))?;
        }
        D::PathLength { interval } => validate_interval(document, *interval)?,
        D::EqualPathLength { first, second } => {
            validate_interval(document, *first)?;
            validate_interval(document, *second)?;
            if first == second {
                return invalid("equal path length", "intervals must be distinct");
            }
        }
        D::SegmentLength { line } => document.validate_line_support_ref(*line)?,
    }
    Ok(())
}

fn validate_measurement(
    document: &SketchDocument,
    value: &DocumentMeasurementDefinition,
) -> Result<(), DocumentError> {
    match value {
        DocumentMeasurementDefinition::SignedCurvature { contact } => {
            document
                .measure_curve_contact(*contact, DocumentCurveMeasurementKind::SignedCurvature)
                .map_err(field_error("curve measurement"))?;
        }
        DocumentMeasurementDefinition::UnsignedCurvature { contact } => {
            document
                .measure_curve_contact(*contact, DocumentCurveMeasurementKind::UnsignedCurvature)
                .map_err(field_error("curve measurement"))?;
        }
        DocumentMeasurementDefinition::OsculatingRadius { contact } => {
            document
                .measure_curve_contact(*contact, DocumentCurveMeasurementKind::OsculatingRadius)
                .map_err(field_error("curve measurement"))?;
        }
        DocumentMeasurementDefinition::ConicProperty { curve, property } => {
            document
                .measure_conic(*curve, *property)
                .map_err(field_error("conic measurement"))?;
        }
        DocumentMeasurementDefinition::BoundedCurveLength { interval } => {
            validate_interval(document, *interval)?;
        }
        DocumentMeasurementDefinition::DimensionValue { definition } => {
            validate_dimension(document, definition)?;
        }
    }
    Ok(())
}

fn validate_interval(
    document: &SketchDocument,
    value: DocumentBoundedCurveInterval,
) -> Result<(), DocumentError> {
    document.validate_curve_span_ref(value.support)?;
    if value.support.winding != 0 {
        return invalid(
            "bounded curve interval",
            "bounded spans require zero winding",
        );
    }
    if !value.start.is_finite()
        || !value.end.is_finite()
        || value.start < 0.0
        || value.end > 1.0
        || value.start >= value.end
    {
        return invalid(
            "bounded curve interval",
            "endpoints must satisfy 0 <= start < end <= 1",
        );
    }
    for t in [value.start, (value.start + value.end) * 0.5, value.end] {
        let jet = document
            .evaluate_curve_jet(value.support.span, t)
            .map_err(field_error("bounded curve interval"))?;
        if !jet.first_derivative.norm().is_finite() || jet.first_derivative.norm() == 0.0 {
            return invalid("bounded curve interval", "curve must be regular");
        }
    }
    Ok(())
}

fn bounded_length(
    document: &SketchDocument,
    interval: DocumentBoundedCurveInterval,
    controller: &mut OperationController,
) -> Result<Option<(f64, DocumentMeasurementWork)>, DocumentError> {
    validate_interval(document, interval)?;
    let mut work = DocumentMeasurementWork::default();
    let speed = |t: f64,
                 controller: &mut OperationController,
                 work: &mut DocumentMeasurementWork|
     -> Result<Option<f64>, DocumentError> {
        if controller
            .charge(
                OperationWorkCounter::MeasurementIntegrations,
                1,
                OperationCheckpoint::MeasurementIntegration,
            )
            .is_err()
            || controller
                .charge(
                    OperationWorkCounter::MeasurementDerivativeEvaluations,
                    1,
                    OperationCheckpoint::MeasurementDerivative,
                )
                .is_err()
        {
            return Ok(None);
        }
        work.integrations = work.integrations.saturating_add(1);
        work.derivative_evaluations = work.derivative_evaluations.saturating_add(1);
        let speed = document
            .evaluate_curve_jet(interval.support.span, t)
            .map_err(field_error("path length derivative"))?
            .first_derivative
            .norm();
        if !speed.is_finite() || speed == 0.0 {
            return invalid(
                "path length derivative",
                "curve speed must be finite and nonzero",
            );
        }
        Ok(Some(speed))
    };
    let a = interval.start;
    let b = interval.end;
    let m = (a + b) * 0.5;
    let Some(fa) = speed(a, controller, &mut work)? else {
        return Ok(None);
    };
    let Some(fm) = speed(m, controller, &mut work)? else {
        return Ok(None);
    };
    let Some(fb) = speed(b, controller, &mut work)? else {
        return Ok(None);
    };
    let whole = (b - a) * (fa + 4.0 * fm + fb) / 6.0;
    let tolerance = document.model_scale() * ABSOLUTE_LENGTH_TOLERANCE_FACTOR;
    let Some(value) = adaptive_simpson(
        &speed, a, b, fa, fm, fb, whole, tolerance, 0, controller, &mut work,
    )?
    else {
        return Ok(None);
    };
    if !value.is_finite() || value < 0.0 {
        return invalid(
            "bounded curve length",
            "integral must be finite and nonnegative",
        );
    }
    Ok(Some((value, work)))
}

#[allow(clippy::too_many_arguments)]
fn adaptive_simpson<F>(
    speed: &F,
    a: f64,
    b: f64,
    fa: f64,
    fm: f64,
    fb: f64,
    whole: f64,
    tol: f64,
    depth: u32,
    controller: &mut OperationController,
    work: &mut DocumentMeasurementWork,
) -> Result<Option<f64>, DocumentError>
where
    F: Fn(
        f64,
        &mut OperationController,
        &mut DocumentMeasurementWork,
    ) -> Result<Option<f64>, DocumentError>,
{
    if depth >= MAX_INTEGRATION_DEPTH {
        return invalid(
            "bounded curve length",
            "certified integration depth exhausted",
        );
    }
    if controller
        .checkpoint(OperationCheckpoint::ProfileSubdivision)
        .is_err()
    {
        return Ok(None);
    }
    let m = (a + b) * 0.5;
    let lm = (a + m) * 0.5;
    let rm = (m + b) * 0.5;
    let Some(fl) = speed(lm, controller, work)? else {
        return Ok(None);
    };
    let Some(fr) = speed(rm, controller, work)? else {
        return Ok(None);
    };
    let left = (m - a) * (fa + 4.0 * fl + fm) / 6.0;
    let right = (b - m) * (fm + 4.0 * fr + fb) / 6.0;
    let refined = left + right;
    let error = (refined - whole).abs() / 15.0;
    if error <= tol {
        return Ok(Some(refined + (refined - whole) / 15.0));
    }
    let Some(l) = adaptive_simpson(
        speed,
        a,
        m,
        fa,
        fl,
        fm,
        left,
        tol * 0.5,
        depth + 1,
        controller,
        work,
    )?
    else {
        return Ok(None);
    };
    let Some(r) = adaptive_simpson(
        speed,
        m,
        b,
        fm,
        fr,
        fb,
        right,
        tol * 0.5,
        depth + 1,
        controller,
        work,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(l + r))
}

fn point_value(
    document: &SketchDocument,
    value: DocumentPointRef,
) -> Result<[f64; 2], DocumentError> {
    let p = match value {
        DocumentPointRef::Point { point }
        | DocumentPointRef::Control(crate::DocumentControlRef { control: point, .. }) => {
            document
                .point(point)
                .ok_or(DocumentError::UnknownId {
                    kind: "point",
                    id: point.0,
                })?
                .position
        }
        DocumentPointRef::Center(center) => {
            match document.curve(center.curve).map(|c| &c.definition) {
                Some(
                    CurveDefinition::Circle { center, .. }
                    | CurveDefinition::CircularArc { center, .. },
                ) => {
                    document
                        .point(*center)
                        .ok_or(DocumentError::UnknownId {
                            kind: "center point",
                            id: center.0,
                        })?
                        .position
                }
                _ => document
                    .evaluate_conic_feature(center.curve, DocumentConicFeature::Center)
                    .map_err(field_error("point feature"))?,
            }
        }
        DocumentPointRef::Endpoint(endpoint) => {
            let seed = document.curve_endpoint_contact_seed(endpoint)?;
            let p = document
                .evaluate_curve_jet(seed.support.span, seed.parameter)
                .map_err(field_error("endpoint feature"))?
                .position;
            [p.x, p.y]
        }
        DocumentPointRef::Focus { curve, index } => document
            .evaluate_conic_feature(curve, DocumentConicFeature::Focus { index })
            .map_err(field_error("focus feature"))?,
        DocumentPointRef::FixedCurveLocation { contact } => {
            let p = document
                .evaluate_contact_jet(contact)
                .map_err(field_error("contact point feature"))?
                .position;
            [p.x, p.y]
        }
    };
    if p.iter().all(|x| x.is_finite()) {
        Ok(p)
    } else {
        invalid("point feature", "coordinates must be finite")
    }
}

fn line_value(
    document: &SketchDocument,
    line: DocumentLineSupportRef,
) -> Result<([f64; 2], [f64; 2]), DocumentError> {
    document.validate_line_support_ref(line)?;
    let mut a = document
        .evaluate_curve_jet(line.span, 0.0)
        .map_err(field_error("line support"))?
        .position;
    let mut b = document
        .evaluate_curve_jet(line.span, 1.0)
        .map_err(field_error("line support"))?
        .position;
    if line.direction == DocumentDirectionSense::Reverse {
        std::mem::swap(&mut a, &mut b);
    }
    Ok(([a.x, a.y], unit([b.x - a.x, b.y - a.y])?))
}
fn arc_data(document: &SketchDocument, arc: CurveId) -> Result<(f64, f64), DocumentError> {
    match document.curve(arc).map(|c| &c.definition) {
        Some(CurveDefinition::CircularArc {
            radius,
            start_angle,
            end_angle,
            sweep,
            ..
        }) => {
            let r = document
                .scalar(*radius)
                .ok_or(DocumentError::UnknownId {
                    kind: "arc radius",
                    id: radius.0,
                })?
                .value;
            let a = document
                .scalar(*start_angle)
                .ok_or(DocumentError::UnknownId {
                    kind: "arc angle",
                    id: start_angle.0,
                })?
                .value;
            let b = document
                .scalar(*end_angle)
                .ok_or(DocumentError::UnknownId {
                    kind: "arc angle",
                    id: end_angle.0,
                })?
                .value;
            let raw = match sweep {
                crate::DocumentArcSweep::CounterClockwise => {
                    (b - a).rem_euclid(std::f64::consts::TAU)
                }
                crate::DocumentArcSweep::Clockwise => -(a - b).rem_euclid(std::f64::consts::TAU),
            };
            if r.is_finite() && r > 0.0 && raw.is_finite() && raw != 0.0 {
                Ok((r, raw))
            } else {
                invalid(
                    "circular arc",
                    "radius and explicit sweep must be finite and nonzero",
                )
            }
        }
        Some(_) => invalid("circular arc", "curve is not a circular arc"),
        None => Err(DocumentError::UnknownId {
            kind: "curve",
            id: arc.0,
        }),
    }
}
fn angle(a: [f64; 2], b: [f64; 2], orientation: DocumentAngleOrientation, winding: i32) -> f64 {
    let sign = if orientation == DocumentAngleOrientation::CounterClockwise {
        1.0
    } else {
        -1.0
    };
    sign * cross(a, b).atan2(dot(a, b)) + f64::from(winding) * std::f64::consts::TAU
}
fn distinct_points(
    document: &SketchDocument,
    a: DocumentPointRef,
    b: DocumentPointRef,
) -> Result<(), DocumentError> {
    document.validate_point_ref(a)?;
    document.validate_point_ref(b)?;
    if a == b {
        invalid("point operands", "points must be distinct")
    } else {
        Ok(())
    }
}
fn unit(v: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    let n = v[0].hypot(v[1]);
    if n.is_finite() && n > 0.0 {
        Ok([v[0] / n, v[1] / n])
    } else {
        invalid("direction", "direction must be finite and nonzero")
    }
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn side_sign(side: DocumentLineSide) -> f64 {
    match side {
        DocumentLineSide::Left => 1.0,
        DocumentLineSide::Right => -1.0,
    }
}
fn axis_index(axis: DocumentCoordinateAxis) -> usize {
    match axis {
        DocumentCoordinateAxis::X => 0,
        DocumentCoordinateAxis::Y => 1,
    }
}

fn runtime_line_support(
    mappings: &crate::DocumentRuntimeMap,
    support: DocumentLineSupportRef,
) -> Result<crate::SegmentId, DocumentError> {
    mappings
        .runtime_segment(support.span)
        .ok_or(DocumentError::UnknownId {
            kind: "line support",
            id: support.span.curve.0,
        })
}

const fn runtime_angle_orientation(
    orientation: DocumentAngleOrientation,
) -> crate::AngleOrientation {
    match orientation {
        DocumentAngleOrientation::CounterClockwise => crate::AngleOrientation::CounterClockwise,
        DocumentAngleOrientation::Clockwise => crate::AngleOrientation::Clockwise,
    }
}

const fn effective_line_side(
    direction: DocumentDirectionSense,
    side: DocumentLineSide,
) -> crate::LineSide {
    match (direction, side) {
        (DocumentDirectionSense::Forward, DocumentLineSide::Left)
        | (DocumentDirectionSense::Reverse, DocumentLineSide::Right) => crate::LineSide::Left,
        (DocumentDirectionSense::Forward, DocumentLineSide::Right)
        | (DocumentDirectionSense::Reverse, DocumentLineSide::Left) => crate::LineSide::Right,
    }
}

const fn opposite_line_side(side: crate::LineSide) -> crate::LineSide {
    match side {
        crate::LineSide::Left => crate::LineSide::Right,
        crate::LineSide::Right => crate::LineSide::Left,
    }
}

fn checked_label(label: String) -> Result<String, DocumentError> {
    if label.is_empty() || label.len() > crate::MAX_LABEL_BYTES {
        invalid(
            "measurement label",
            "label must be nonempty and within the byte limit",
        )
    } else {
        Ok(label)
    }
}
fn invalid<T>(field: &'static str, message: &str) -> Result<T, DocumentError> {
    Err(DocumentError::InvalidField {
        field,
        message: message.into(),
    })
}
fn field_error<E: std::fmt::Display>(field: &'static str) -> impl FnOnce(E) -> DocumentError {
    move |error| DocumentError::InvalidField {
        field,
        message: error.to_string(),
    }
}
