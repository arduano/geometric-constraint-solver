use std::collections::BTreeMap;

use geosolve_core::{OperationCheckpoint, OperationController, OperationWorkCounter};
use geosolve_geometry::{BSplineSpanIndex, DirectedParameterTrim, Point2, Vector2};

use crate::document::{
    ContactDomain, ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
    DocumentAngleOrientation, DocumentArcSweep, DocumentArcTangencySide, DocumentBSplineForm,
    DocumentCircleContainment, DocumentCircleTangencyMode, DocumentConstraint,
    DocumentConstraintDefinition, DocumentCoordinateAxis, DocumentCurveContinuity,
    DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation, DocumentCurveNormalSide,
    DocumentDimension, DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentError, DocumentFilletEndpointOrder, DocumentLineOffsetOrientation, DocumentLineSide,
    DocumentParameterId, DocumentParameterTarget, DocumentSourceId, EffectiveActivity,
    FeatureEndpoint, PersistentId, SketchDocument, TangentOrientation,
    canonical_parameter_target_key, document_arc_signed_sweep, document_hyperbola_branch,
};
use crate::document_session::{ExternalSnapshotEntry, ExternalSnapshotSetDigest, ParameterDigest};

/// One resolved fixed coefficient and its immutable host-input provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentParameterRuntimeBinding {
    pub parameter: DocumentParameterId,
    pub target: DocumentParameterTarget,
    pub runtime: RuntimeSource,
    pub value: f64,
    pub parameter_revision: u64,
    pub parameter_digest: ParameterDigest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedParameterBinding {
    pub parameter: DocumentParameterId,
    pub target: DocumentParameterTarget,
    pub value: f64,
    pub parameter_revision: u64,
    pub parameter_digest: ParameterDigest,
}

/// Complete immutable M42 input view captured before lowering.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedDocumentParameters {
    pub activity: EffectiveActivity,
    pub dimensions: BTreeMap<DocumentDimensionId, ResolvedParameterBinding>,
    pub dimensionless: BTreeMap<DesignScalarId, ResolvedParameterBinding>,
    pub external_revision: u64,
    pub external_digest: ExternalSnapshotSetDigest,
    pub external: BTreeMap<crate::DocumentExternalBindingId, ExternalSnapshotEntry>,
}
use crate::{
    AngleOrientation, ArcCircleTangencySide, ArcId, ArcSweep, BSplineId, CenterDirectionBranch,
    CircleContainment, CircleId, CircleTangencyMode, ConicId, ConicKind, ContactState,
    CoordinateAxis, CurveContactNeighborhood, CurveContinuity, CurveCurvatureRelation,
    CurveDirectionRelation, CurveNormalSide, CurveTangentOrientation, DimensionMode,
    FilletEndpointOrder, LineOffsetOrientation, LineParameterDomain, LineSide, NurbsId, PointId,
    SegmentBranch, SegmentEndpoint, SegmentId, Sketch, SketchConstraintId, SketchConstraintKind,
    SketchCurve, SketchCurveContact, SketchDimensionId,
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
    BSpline {
        spline: BSplineId,
        spans: Vec<(u32, BSplineSpanIndex)>,
    },
    Nurbs {
        nurbs: NurbsId,
        spans: Vec<(u32, BSplineSpanIndex)>,
    },
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
    BSplineParameter,
    NurbsParameter,
    CurveParameter,
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
    parameter_bindings: Vec<DocumentParameterRuntimeBinding>,
    point_index: BTreeMap<DesignPointId, usize>,
    persistent_point_index: BTreeMap<PointId, DesignPointId>,
    curve_index: BTreeMap<CurveId, usize>,
    source_index: BTreeMap<DocumentSourceId, usize>,
    contact_index: BTreeMap<ContactId, usize>,
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

    /// Returns every active parameter-supplied numeric target with exact runtime/input provenance.
    #[must_use]
    pub fn parameter_bindings(&self) -> &[DocumentParameterRuntimeBinding] {
        &self.parameter_bindings
    }

    pub(crate) fn has_compatible_runtime_topology(&self, candidate: &Self) -> bool {
        self.points == candidate.points
            && self.curves == candidate.curves
            && self.sources == candidate.sources
            && self.contacts == candidate.contacts
            && self.parameter_bindings.len() == candidate.parameter_bindings.len()
            && self
                .parameter_bindings
                .iter()
                .zip(&candidate.parameter_bindings)
                .all(|(retained, candidate)| {
                    retained.parameter == candidate.parameter
                        && retained.target == candidate.target
                        && retained.runtime == candidate.runtime
                })
    }

    #[must_use]
    pub fn runtime_point(&self, id: DesignPointId) -> Option<PointId> {
        if let Some(index) = self.point_index.get(&id) {
            return self.points.get(*index).map(|mapping| mapping.runtime);
        }
        self.points
            .iter()
            .find_map(|mapping| (mapping.persistent == id).then_some(mapping.runtime))
    }

    pub(crate) fn persistent_point(&self, id: PointId) -> Option<DesignPointId> {
        self.persistent_point_index.get(&id).copied()
    }

    #[must_use]
    pub fn runtime_curve(&self, id: CurveId) -> Option<&RuntimeCurve> {
        if let Some(index) = self.curve_index.get(&id) {
            return self.curves.get(*index).map(|mapping| &mapping.runtime);
        }
        self.curves
            .iter()
            .find_map(|mapping| (mapping.persistent == id).then_some(&mapping.runtime))
    }

    #[must_use]
    pub fn runtime_source(&self, id: DocumentSourceId) -> Option<RuntimeSource> {
        if let Some(index) = self.source_index.get(&id) {
            return self.sources.get(*index).and_then(|mapping| mapping.runtime);
        }
        self.sources
            .iter()
            .find_map(|mapping| (mapping.source_id == id).then_some(mapping.runtime))
            .flatten()
    }

    /// Returns the retained runtime source/latent role for one persistent contact.
    #[must_use]
    pub fn runtime_contact(&self, id: ContactId) -> Option<ContactRuntimeMapping> {
        if let Some(index) = self.contact_index.get(&id) {
            return self.contacts.get(*index).copied();
        }
        self.contacts
            .iter()
            .find(|mapping| mapping.persistent == id)
            .copied()
    }

    fn rebuild_indices(&mut self) {
        self.point_index = self
            .points
            .iter()
            .enumerate()
            .map(|(index, mapping)| (mapping.persistent, index))
            .collect();
        self.persistent_point_index = self
            .points
            .iter()
            .map(|mapping| (mapping.runtime, mapping.persistent))
            .collect();
        self.curve_index = self
            .curves
            .iter()
            .enumerate()
            .map(|(index, mapping)| (mapping.persistent, index))
            .collect();
        self.source_index = self
            .sources
            .iter()
            .enumerate()
            .map(|(index, mapping)| (mapping.source_id, index))
            .collect();
        self.contact_index = self
            .contacts
            .iter()
            .enumerate()
            .map(|(index, mapping)| (mapping.persistent, index))
            .collect();
    }

    pub(crate) fn runtime_segment(&self, span: CurveSpan) -> Option<SegmentId> {
        match self.runtime_curve(span.curve)? {
            RuntimeCurve::Line(segment) => (span.segment == 0).then_some(*segment),
            RuntimeCurve::Polyline(segments) => segments.get(span.segment as usize).copied(),
            RuntimeCurve::Circle(_)
            | RuntimeCurve::CircularArc(_)
            | RuntimeCurve::QuadraticBezier(_)
            | RuntimeCurve::CubicBezier(_)
            | RuntimeCurve::Conic(_)
            | RuntimeCurve::BSpline { .. }
            | RuntimeCurve::Nurbs { .. } => None,
        }
    }

    pub(crate) fn runtime_circle(&self, id: CurveId) -> Option<CircleId> {
        match self.runtime_curve(id)? {
            RuntimeCurve::Circle(circle) => Some(*circle),
            _ => None,
        }
    }

    pub(crate) fn runtime_arc(&self, id: CurveId) -> Option<ArcId> {
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

    /// Returns the runtime NURBS generated for one persistent NURBS curve.
    #[must_use]
    pub fn runtime_nurbs(&self, id: CurveId) -> Option<NurbsId> {
        match self.runtime_curve(id)? {
            RuntimeCurve::Nurbs { nurbs, .. } => Some(*nurbs),
            _ => None,
        }
    }

    fn runtime_bspline_span(&self, span: CurveSpan) -> Option<(BSplineId, BSplineSpanIndex)> {
        let RuntimeCurve::BSpline { spline, spans } = self.runtime_curve(span.curve)? else {
            return None;
        };
        spans.iter().find_map(|(semantic, runtime)| {
            (*semantic == span.segment).then_some((*spline, *runtime))
        })
    }

    fn runtime_nurbs_span(&self, span: CurveSpan) -> Option<(NurbsId, BSplineSpanIndex)> {
        let RuntimeCurve::Nurbs { nurbs, spans } = self.runtime_curve(span.curve)? else {
            return None;
        };
        spans.iter().find_map(|(semantic, runtime)| {
            (*semantic == span.segment).then_some((*nurbs, *runtime))
        })
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

fn lowering_item(
    controller: &mut Option<&mut OperationController>,
) -> Result<(), geosolve_core::OperationStopReason> {
    let Some(controller) = controller.as_deref_mut() else {
        return Ok(());
    };
    controller.charge(
        OperationWorkCounter::DocumentLoweringItems,
        1,
        OperationCheckpoint::DocumentLowering,
    )
}

impl SketchDocument {
    /// Deterministically lowers persistent semantic IDs to fresh runtime IDs.
    ///
    /// # Errors
    ///
    /// Returns a document-validation or guarded runtime-model error.
    ///
    /// # Panics
    ///
    /// Panics only if the internal unlimited path reports an interruption
    /// without an operation controller.
    pub fn lower(&self) -> Result<LoweredDocument, DocumentError> {
        self.lower_inner(None)
            .map(|lowered| lowered.expect("uncontrolled lowering cannot be interrupted"))
    }

    /// Deterministically lowers this document under cooperative operation control.
    ///
    /// Lowering uses scratch runtime state and never modifies this document.
    ///
    /// # Errors
    ///
    /// Returns a document-validation or guarded runtime-model error.
    pub fn lower_controlled(
        &self,
        control: geosolve_core::OperationControl,
    ) -> Result<geosolve_core::OperationOutcome<LoweredDocument>, DocumentError> {
        let mut controller = OperationController::new(control);
        let Some(lowered) = self.lower_inner(Some(&mut controller))? else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(lowered))
    }

    pub(crate) fn lower_with_controller(
        &self,
        controller: &mut OperationController,
    ) -> Result<Option<LoweredDocument>, DocumentError> {
        self.lower_inner(Some(controller))
    }

    pub(crate) fn lower_with_resolved_parameters(
        &self,
        resolved: &ResolvedDocumentParameters,
    ) -> Result<LoweredDocument, DocumentError> {
        self.lower_inner_with_parameters(None, Some(resolved))
            .map(|lowered| lowered.expect("uncontrolled lowering cannot be interrupted"))
    }

    pub(crate) fn lower_with_resolved_parameters_with_controller(
        &self,
        resolved: &ResolvedDocumentParameters,
        controller: &mut OperationController,
    ) -> Result<Option<LoweredDocument>, DocumentError> {
        self.lower_inner_with_parameters(Some(controller), Some(resolved))
    }

    fn lower_inner(
        &self,
        controller: Option<&mut OperationController>,
    ) -> Result<Option<LoweredDocument>, DocumentError> {
        self.lower_inner_with_parameters(controller, None)
    }

    #[allow(clippy::too_many_lines)]
    fn lower_inner_with_parameters(
        &self,
        mut controller: Option<&mut OperationController>,
        resolved: Option<&ResolvedDocumentParameters>,
    ) -> Result<Option<LoweredDocument>, DocumentError> {
        if !self.validate_with_controller(controller.as_deref_mut())? {
            return Ok(None);
        }
        let default_activity;
        let activity = if let Some(resolved) = resolved {
            &resolved.activity
        } else {
            default_activity = self.effective_activity();
            &default_activity
        };
        let mut sketch = Sketch::new(self.model_scale())?;
        let mut mappings = DocumentRuntimeMap::default();

        let mut points: Vec<_> = self.points().iter().collect();
        points.sort_by_key(|point| point.id);
        for point in points {
            if !activity.is_active(point.id) {
                continue;
            }
            if lowering_item(&mut controller).is_err() {
                return Ok(None);
            }
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
            if !activity.is_active(curve.id) {
                continue;
            }
            if lowering_item(&mut controller).is_err() {
                return Ok(None);
            }
            let runtime = lower_curve(self, &mut sketch, &mappings, curve, activity)?;
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
            if lowering_item(&mut controller).is_err() {
                return Ok(None);
            }
            if let Some(constraint) = constraints.get(source) {
                let runtime = if activity.is_active(constraint.id) {
                    let (runtime, contacts) =
                        lower_constraint(self, &mut sketch, &mappings, constraint, resolved)?;
                    mappings.contacts.extend(contacts);
                    Some(RuntimeSource::Constraint(runtime))
                } else {
                    None
                };
                mappings.sources.push(DocumentSourceRuntimeMapping {
                    source_id: *source,
                    label: constraint.label.clone(),
                    runtime,
                });
            } else if let Some(dimension) = dimensions.get(source) {
                let runtime = if activity.is_active(dimension.id) {
                    let runtime = RuntimeSource::Dimension(lower_dimension(
                        self,
                        &mut sketch,
                        &mappings,
                        dimension,
                        resolved.and_then(|values| values.dimensions.get(&dimension.id)),
                    )?);
                    if let Some(binding) =
                        resolved.and_then(|values| values.dimensions.get(&dimension.id))
                    {
                        mappings
                            .parameter_bindings
                            .push(DocumentParameterRuntimeBinding {
                                parameter: binding.parameter,
                                target: binding.target,
                                runtime,
                                value: binding.value,
                                parameter_revision: binding.parameter_revision,
                                parameter_digest: binding.parameter_digest,
                            });
                    }
                    Some(runtime)
                } else {
                    None
                };
                mappings.sources.push(DocumentSourceRuntimeMapping {
                    source_id: *source,
                    label: dimension.label.clone(),
                    runtime,
                });
            }
        }
        if let Some(resolved) = resolved {
            for binding in resolved.dimensionless.values() {
                if lowering_item(&mut controller).is_err() {
                    return Ok(None);
                }
                let DocumentParameterTarget::DimensionlessFixedScalar(property) = binding.target
                else {
                    return invalid_runtime("resolved dimensionless target kind changed");
                };
                let runtime =
                    RuntimeSource::Constraint(crate::semantic::add_parameter_fixed_scalar(
                        self,
                        &mappings,
                        &mut sketch,
                        property,
                        binding.value,
                    )?);
                mappings
                    .parameter_bindings
                    .push(DocumentParameterRuntimeBinding {
                        parameter: binding.parameter,
                        target: binding.target,
                        runtime,
                        value: binding.value,
                        parameter_revision: binding.parameter_revision,
                        parameter_digest: binding.parameter_digest,
                    });
            }
            mappings.parameter_bindings.sort_by_key(|binding| {
                (
                    binding.parameter,
                    canonical_parameter_target_key(binding.target),
                )
            });
        }
        mappings.rebuild_indices();
        Ok(Some(LoweredDocument { sketch, mappings }))
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

    pub(crate) fn project_accepted_state_with_controller(
        &mut self,
        sketch: &Sketch,
        mappings: &DocumentRuntimeMap,
        controller: &mut OperationController,
    ) -> Result<bool, DocumentError> {
        let mut candidate = self.clone();
        candidate.project_accepted_state_inner(sketch, mappings)?;
        if !candidate.validate_with_controller(Some(controller))? {
            return Ok(false);
        }
        *self = candidate;
        Ok(true)
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
                    let CurveDefinition::CircularArc {
                        radius,
                        start_angle,
                        end_angle,
                        ..
                    } = persistent.definition
                    else {
                        return invalid_runtime("curve mapping kind changed");
                    };
                    let value = sketch
                        .arc(arc)
                        .ok_or_else(|| unknown_runtime("runtime arc", mapping.persistent.0))?
                        .clone();
                    for (scalar, accepted) in [
                        (radius, value.radius()),
                        (start_angle, value.start_angle()),
                        (end_angle, value.end_angle()),
                    ] {
                        self.scalar_mut(scalar)
                            .ok_or_else(|| unknown_runtime("scalar", scalar.0))?
                            .value = accepted;
                    }
                    continue;
                }
                RuntimeCurve::Line(_)
                | RuntimeCurve::Polyline(_)
                | RuntimeCurve::QuadraticBezier(_)
                | RuntimeCurve::CubicBezier(_)
                | RuntimeCurve::BSpline { .. } => continue,
                RuntimeCurve::Nurbs { nurbs, .. } => {
                    project_nurbs_state(self, sketch, mapping.persistent, nurbs)?;
                    continue;
                }
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
            let constraint = sketch
                .constraint(mapping.constraint)
                .ok_or_else(|| unknown_runtime("runtime constraint", mapping.persistent.0))?;
            let advanced_value = match (mapping.role, constraint.kind()) {
                (
                    DocumentContactRole::CurveParameter,
                    SketchConstraintKind::CurveDirection { contact, .. },
                ) => Some(contact.parameter),
                (
                    DocumentContactRole::FirstCurveParameter,
                    SketchConstraintKind::EqualCurvature { first, .. }
                    | SketchConstraintKind::EndpointContinuity { first, .. }
                    | SketchConstraintKind::CurveCurveFillet { first, .. },
                ) => Some(first.parameter),
                (
                    DocumentContactRole::SecondCurveParameter,
                    SketchConstraintKind::EqualCurvature { second, .. }
                    | SketchConstraintKind::EndpointContinuity { second, .. }
                    | SketchConstraintKind::CurveCurveFillet { second, .. },
                ) => Some(second.parameter),
                _ => None,
            };
            let value = if let Some(value) = advanced_value {
                value
            } else {
                let state = sketch.contact_state(mapping.constraint)?;
                match (mapping.role, state) {
                    (
                        DocumentContactRole::LineParameter,
                        ContactState::PointOnLine { parameter },
                    )
                    | (
                        DocumentContactRole::LineParameter
                        | DocumentContactRole::CircleAngle
                        | DocumentContactRole::ArcSpanParameter
                        | DocumentContactRole::BezierParameter
                        | DocumentContactRole::ConicParameter
                        | DocumentContactRole::BSplineParameter
                        | DocumentContactRole::NurbsParameter,
                        ContactState::PointOnCurve { parameter }
                        | ContactState::LineCurveTangency { parameter },
                    )
                    | (
                        DocumentContactRole::BezierParameter,
                        ContactState::PointOnBezier { parameter }
                        | ContactState::LineBezierTangency { parameter },
                    ) => parameter,
                    (DocumentContactRole::CircleAngle, ContactState::PointOnCircle { angle }) => {
                        angle
                    }
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
                        }
                        | ContactState::CurveCurveFillet {
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
                        }
                        | ContactState::CurveCurveFillet {
                            second_parameter, ..
                        },
                    ) => second_parameter,
                    _ => return invalid_runtime("contact role does not match runtime source"),
                }
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
    activity: &EffectiveActivity,
) -> Result<RuntimeCurve, DocumentError> {
    let runtime = match &curve.definition {
        CurveDefinition::Line {
            start,
            end,
            branch_direction,
        } => {
            let span = CurveSpan::line(curve.id);
            let direction = if document.curve_branch_is_enforced_with_activity(span, activity) {
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
                let direction = if document.curve_branch_is_enforced_with_activity(span, activity) {
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
        CurveDefinition::BSpline {
            form,
            degree,
            controls,
            knots,
            span_ids,
            ..
        } => {
            let spline = sketch.add_named_bspline(
                &curve.label,
                match form {
                    DocumentBSplineForm::Clamped => geosolve_geometry::BSplineForm::Clamped,
                    DocumentBSplineForm::Periodic => geosolve_geometry::BSplineForm::Periodic,
                },
                *degree,
                controls
                    .iter()
                    .map(|control| runtime_point(mappings, *control))
                    .collect::<Result<Vec<_>, _>>()?,
                knots.clone(),
            )?;
            let runtime = sketch
                .bspline(spline)
                .ok_or_else(|| unknown_runtime("runtime B-spline", curve.id.0))?;
            let spans = span_ids
                .iter()
                .copied()
                .zip(
                    runtime
                        .basis()
                        .spans()
                        .iter()
                        .map(geosolve_geometry::BSplineSpan::index),
                )
                .collect();
            RuntimeCurve::BSpline { spline, spans }
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
            let gauge_index = weights
                .iter()
                .position(|weight| weight == gauge_weight)
                .ok_or_else(|| unknown_runtime("NURBS gauge weight", gauge_weight.0))?;
            let nurbs = sketch.add_named_nurbs(
                &curve.label,
                match form {
                    DocumentBSplineForm::Clamped => geosolve_geometry::BSplineForm::Clamped,
                    DocumentBSplineForm::Periodic => geosolve_geometry::BSplineForm::Periodic,
                },
                *degree,
                controls
                    .iter()
                    .map(|control| runtime_point(mappings, *control))
                    .collect::<Result<Vec<_>, _>>()?,
                weights
                    .iter()
                    .map(|weight| scalar_value(document, *weight))
                    .collect::<Result<Vec<_>, _>>()?,
                gauge_index,
                knots.clone(),
            )?;
            let runtime = sketch
                .nurbs(nurbs)
                .ok_or_else(|| unknown_runtime("runtime NURBS", curve.id.0))?;
            let spans = span_ids
                .iter()
                .copied()
                .zip(
                    runtime
                        .basis()
                        .spans()
                        .iter()
                        .map(geosolve_geometry::BSplineSpan::index),
                )
                .collect();
            RuntimeCurve::Nurbs { nurbs, spans }
        }
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

fn project_nurbs_state(
    document: &mut SketchDocument,
    sketch: &Sketch,
    persistent: CurveId,
    runtime: NurbsId,
) -> Result<(), DocumentError> {
    let (weight_ids, gauge_weight) = match &document
        .curve(persistent)
        .ok_or_else(|| unknown_runtime("curve", persistent.0))?
        .definition
    {
        CurveDefinition::Nurbs {
            weights,
            gauge_weight,
            ..
        } => (weights.clone(), *gauge_weight),
        _ => return invalid_runtime("curve mapping kind changed"),
    };
    let runtime = sketch
        .nurbs(runtime)
        .ok_or_else(|| unknown_runtime("runtime NURBS", persistent.0))?;
    if runtime.weights().len() != weight_ids.len()
        || weight_ids.get(runtime.gauge_index()) != Some(&gauge_weight)
    {
        return invalid_runtime("NURBS weight mapping changed");
    }
    for (index, (weight, value)) in weight_ids.iter().zip(runtime.weights()).enumerate() {
        document
            .scalar_mut(*weight)
            .ok_or_else(|| unknown_runtime("scalar", weight.0))?
            .value = if index == runtime.gauge_index() {
            1.0
        } else {
            *value
        };
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn lower_constraint(
    document: &SketchDocument,
    sketch: &mut Sketch,
    mappings: &DocumentRuntimeMap,
    constraint: &DocumentConstraint,
    resolved: Option<&ResolvedDocumentParameters>,
) -> Result<(SketchConstraintId, Vec<ContactRuntimeMapping>), DocumentError> {
    use DocumentConstraintDefinition as C;
    let mut contacts = Vec::new();
    let runtime = match &constraint.definition {
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
        C::ExternalPointCoincident { point, external } => {
            let inputs = resolved.ok_or_else(missing_external_input)?;
            let entry = resolved_external(inputs, external.binding)?;
            let crate::ExternalSnapshotFeatureV1::Point { position, .. } = &entry.feature else {
                return invalid_runtime("resolved external point kind changed");
            };
            sketch.add_external_point(
                runtime_point(mappings, *point)?,
                Point2::new(position[0], position[1]),
                external_provenance(document, entry, inputs),
            )?
        }
        C::ExternalLineCollinear { line, external } => {
            let inputs = resolved.ok_or_else(missing_external_input)?;
            let entry = resolved_external(inputs, external.binding)?;
            let crate::ExternalSnapshotFeatureV1::LineSegment { start, end, .. } = &entry.feature
            else {
                return invalid_runtime("resolved external line kind changed");
            };
            let (start, end) =
                if matches!(external.direction, crate::DocumentDirectionSense::Forward) {
                    (start, end)
                } else {
                    (end, start)
                };
            sketch.add_external_line_collinear(
                runtime_segment(mappings, line.span)?,
                Point2::new(start[0], start[1]),
                Point2::new(end[0], end[1]),
                external_provenance(document, entry, inputs),
            )?
        }
        C::Concentric { first, second } => sketch.add_coincident(
            runtime_point(mappings, document.resolve_center_ref(*first)?)?,
            runtime_point(mappings, document.resolve_center_ref(*second)?)?,
        )?,
        C::Collinear { first, second } => {
            let first = directed_runtime_segment(mappings, sketch, *first)?;
            let second = directed_runtime_segment(mappings, sketch, *second)?;
            sketch.add_collinear(first, second)?
        }
        C::Horizontal { line } => sketch.add_horizontal(runtime_segment(mappings, *line)?)?,
        C::Vertical { line } => sketch.add_vertical(runtime_segment(mappings, *line)?)?,
        C::HorizontalPoints { first, second } => sketch.add_horizontal_points(
            runtime_point(mappings, *first)?,
            runtime_point(mappings, *second)?,
        )?,
        C::VerticalPoints { first, second } => sketch.add_vertical_points(
            runtime_point(mappings, *first)?,
            runtime_point(mappings, *second)?,
        )?,
        C::HorizontalPointToMidpoint { point, line } => sketch.add_horizontal_point_to_midpoint(
            runtime_point(mappings, *point)?,
            runtime_segment(mappings, *line)?,
        )?,
        C::VerticalPointToMidpoint { point, line } => sketch.add_vertical_point_to_midpoint(
            runtime_point(mappings, *point)?,
            runtime_segment(mappings, *line)?,
        )?,
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
            let orientation =
                match contact
                    .tangent_orientation
                    .ok_or_else(|| DocumentError::InvalidField {
                        field: "contact.tangent_orientation",
                        message: "line-curve tangency requires orientation".into(),
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
                    let orientation =
                        first
                            .tangent_orientation
                            .ok_or_else(|| DocumentError::InvalidField {
                                field: "contact.tangent_orientation",
                                message: "curve tangency requires orientation".into(),
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
        C::CurveDirection {
            line,
            curve_contact,
            relation,
        } => {
            let contact = document
                .contact(*curve_contact)
                .ok_or_else(|| unknown_runtime("contact", curve_contact.0))?;
            let id = sketch.add_curve_direction(
                runtime_segment(mappings, *line)?,
                runtime_curve_contact(document, mappings, contact)?,
                runtime_curve_direction(*relation),
            )?;
            contacts.push(contact_mapping(
                *curve_contact,
                id,
                DocumentContactRole::CurveParameter,
            ));
            id
        }
        C::EqualCurvature {
            first_contact,
            second_contact,
            relation,
        } => {
            let first = document
                .contact(*first_contact)
                .ok_or_else(|| unknown_runtime("contact", first_contact.0))?;
            let second = document
                .contact(*second_contact)
                .ok_or_else(|| unknown_runtime("contact", second_contact.0))?;
            let id = sketch.add_equal_curvature(
                runtime_curve_contact(document, mappings, first)?,
                runtime_curve_contact(document, mappings, second)?,
                runtime_curvature_relation(*relation),
            )?;
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
        C::EndpointContinuity {
            first_contact,
            second_contact,
            continuity,
        } => {
            let first = document
                .contact(*first_contact)
                .ok_or_else(|| unknown_runtime("contact", first_contact.0))?;
            let second = document
                .contact(*second_contact)
                .ok_or_else(|| unknown_runtime("contact", second_contact.0))?;
            let id = sketch.add_endpoint_continuity(
                runtime_curve_contact(document, mappings, first)?,
                runtime_curve_contact(document, mappings, second)?,
                runtime_curve_continuity(*continuity),
            )?;
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
        C::LineLineFillet {
            arc,
            first_contact,
            first_side,
            second_contact,
            second_side,
            endpoint_order,
        } => {
            let first = document
                .contact(*first_contact)
                .ok_or_else(|| unknown_runtime("contact", first_contact.0))?;
            let second = document
                .contact(*second_contact)
                .ok_or_else(|| unknown_runtime("contact", second_contact.0))?;
            let id = sketch.add_line_line_fillet(
                runtime_arc(mappings, *arc)?,
                runtime_curve_contact(document, mappings, first)?,
                runtime_curve_normal_side(*first_side),
                runtime_curve_contact(document, mappings, second)?,
                runtime_curve_normal_side(*second_side),
                match endpoint_order {
                    DocumentFilletEndpointOrder::FirstThenSecond => {
                        FilletEndpointOrder::FirstThenSecond
                    }
                    DocumentFilletEndpointOrder::SecondThenFirst => {
                        FilletEndpointOrder::SecondThenFirst
                    }
                },
            )?;
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
        C::CurveCurveFillet {
            arc,
            first_contact,
            first_side,
            second_contact,
            second_side,
            endpoint_order,
            ..
        } => {
            let first = document
                .contact(*first_contact)
                .ok_or_else(|| unknown_runtime("contact", first_contact.0))?;
            let second = document
                .contact(*second_contact)
                .ok_or_else(|| unknown_runtime("contact", second_contact.0))?;
            let id = sketch.add_curve_curve_fillet(
                runtime_arc(mappings, *arc)?,
                runtime_curve_contact(document, mappings, first)?,
                runtime_curve_normal_side(*first_side),
                runtime_curve_contact(document, mappings, second)?,
                runtime_curve_normal_side(*second_side),
                runtime_fillet_endpoint_order(*endpoint_order),
            )?;
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
    parameter: Option<&ResolvedParameterBinding>,
) -> Result<SketchDimensionId, DocumentError> {
    use DocumentDimensionDefinition as D;
    let mode = match dimension.mode {
        DocumentDimensionMode::Driving => DimensionMode::Driving,
        DocumentDimensionMode::Reference => DimensionMode::Reference,
    };
    let target_value = |target| {
        parameter.map_or_else(
            || scalar_value(document, target),
            |binding| Ok(binding.value),
        )
    };
    let runtime = match dimension.definition {
        D::PointDistance {
            first,
            second,
            target,
        } => sketch.add_point_distance(
            runtime_point(mappings, first)?,
            runtime_point(mappings, second)?,
            target_value(target)?,
            mode,
        )?,
        D::CurveLength { curve, target } => sketch.add_segment_length(
            runtime_segment(mappings, curve)?,
            target_value(target)?,
            mode,
        )?,
        D::Radius { curve, target } => match mappings
            .runtime_curve(curve)
            .ok_or_else(|| unknown_runtime("curve", curve.0))?
        {
            RuntimeCurve::Circle(circle) => {
                sketch.add_circle_radius(*circle, target_value(target)?, mode)?
            }
            RuntimeCurve::CircularArc(arc) => {
                sketch.add_arc_radius(*arc, target_value(target)?, mode)?
            }
            _ => return invalid_runtime("radius dimension requires a radial curve"),
        },
        D::Diameter { curve, target } => match mappings
            .runtime_curve(curve)
            .ok_or_else(|| unknown_runtime("curve", curve.0))?
        {
            RuntimeCurve::Circle(circle) => {
                sketch.add_circle_diameter(*circle, target_value(target)?, mode)?
            }
            RuntimeCurve::CircularArc(arc) => {
                sketch.add_arc_diameter(*arc, target_value(target)?, mode)?
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
            target_value(target)?,
            match orientation {
                DocumentAngleOrientation::CounterClockwise => AngleOrientation::CounterClockwise,
                DocumentAngleOrientation::Clockwise => AngleOrientation::Clockwise,
            },
            mode,
        )?,
        D::SupportingLineOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => sketch.add_supporting_line_offset(
            runtime_segment(mappings, source)?,
            runtime_segment(mappings, target_segment)?,
            target_value(target)?,
            document_line_side(side),
            document_line_offset_orientation(orientation),
            mode,
        )?,
        D::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => sketch.add_exact_translated_segment_offset(
            runtime_segment(mappings, source)?,
            runtime_segment(mappings, target_segment)?,
            target_value(target)?,
            document_line_side(side),
            document_line_offset_orientation(orientation),
            mode,
        )?,
    };
    Ok(runtime)
}

const fn document_line_offset_orientation(
    orientation: DocumentLineOffsetOrientation,
) -> LineOffsetOrientation {
    match orientation {
        DocumentLineOffsetOrientation::Same => LineOffsetOrientation::Same,
        DocumentLineOffsetOrientation::Reversed => LineOffsetOrientation::Reversed,
    }
}

const fn document_line_side(side: DocumentLineSide) -> LineSide {
    match side {
        DocumentLineSide::Left => LineSide::Left,
        DocumentLineSide::Right => LineSide::Right,
    }
}

fn runtime_point(
    mappings: &DocumentRuntimeMap,
    id: DesignPointId,
) -> Result<PointId, DocumentError> {
    mappings
        .runtime_point(id)
        .ok_or_else(|| unknown_runtime("point", id.0))
}

fn missing_external_input() -> DocumentError {
    DocumentError::InvalidField {
        field: "external snapshot",
        message: "external constraints require resolved immutable snapshot input".into(),
    }
}

fn resolved_external(
    resolved: &ResolvedDocumentParameters,
    binding: crate::DocumentExternalBindingId,
) -> Result<&ExternalSnapshotEntry, DocumentError> {
    resolved
        .external
        .get(&binding)
        .ok_or_else(missing_external_input)
}

fn external_provenance(
    document: &SketchDocument,
    entry: &ExternalSnapshotEntry,
    resolved: &ResolvedDocumentParameters,
) -> crate::ExternalConstraintProvenance {
    let declaration = document
        .external_binding(entry.binding)
        .expect("validated external binding exists");
    let (feature_scale, line_domain, line_orientation, line_topology_digest) = match &entry.feature
    {
        crate::ExternalSnapshotFeatureV1::Point { scale, .. } => (*scale, None, None, None),
        crate::ExternalSnapshotFeatureV1::LineSegment {
            domain,
            orientation,
            scale,
            topology_digest,
            ..
        } => (
            *scale,
            Some(*domain),
            Some(*orientation),
            Some(*topology_digest),
        ),
    };
    crate::ExternalConstraintProvenance {
        binding: entry.binding,
        expected_kind: declaration.expected_kind,
        actual_kind: entry.feature.kind(),
        feature_scale,
        line_domain,
        line_orientation,
        line_topology_digest,
        set_revision: resolved.external_revision,
        set_digest: resolved.external_digest,
        source_revision: entry.source_revision,
        source_digest: entry.source_digest,
    }
}

fn runtime_segment(
    mappings: &DocumentRuntimeMap,
    span: CurveSpan,
) -> Result<SegmentId, DocumentError> {
    mappings
        .runtime_segment(span)
        .ok_or_else(|| unknown_runtime("curve span", span.curve.0))
}

fn directed_runtime_segment(
    mappings: &DocumentRuntimeMap,
    sketch: &mut Sketch,
    support: crate::DocumentLineSupportRef,
) -> Result<SegmentId, DocumentError> {
    let segment = runtime_segment(mappings, support.span)?;
    if support.direction == crate::DocumentDirectionSense::Forward {
        return Ok(segment);
    }
    let (start, end) = sketch.segment_endpoints(segment)?;
    let label = sketch
        .segment(segment)
        .ok_or_else(|| unknown_runtime("segment", support.span.curve.0))?
        .label()
        .to_owned();
    Ok(sketch.add_named_segment(label, end, start)?)
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

pub(crate) fn runtime_curve_contact(
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
            RuntimeCurve::BSpline { .. } => {
                let (spline, span) = mappings
                    .runtime_bspline_span(contact.curve)
                    .ok_or_else(|| unknown_runtime("B-spline span", contact.curve.curve.0))?;
                SketchCurve::BSpline { spline, span }
            }
            RuntimeCurve::Nurbs { .. } => {
                let (nurbs, span) = mappings
                    .runtime_nurbs_span(contact.curve)
                    .ok_or_else(|| unknown_runtime("NURBS span", contact.curve.curve.0))?;
                SketchCurve::Nurbs { nurbs, span }
            }
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

pub(crate) fn runtime_bounded_curve(
    mappings: &DocumentRuntimeMap,
    span: CurveSpan,
) -> Result<SketchCurve, DocumentError> {
    match mappings
        .runtime_curve(span.curve)
        .ok_or_else(|| unknown_runtime("curve", span.curve.0))?
    {
        RuntimeCurve::Line(_) | RuntimeCurve::Polyline(_) => Ok(SketchCurve::Line {
            segment: runtime_segment(mappings, span)?,
            domain: LineParameterDomain::BoundedSegment,
        }),
        RuntimeCurve::Circle(circle) => Ok(SketchCurve::Circle(*circle)),
        RuntimeCurve::CircularArc(arc) => Ok(SketchCurve::Arc(*arc)),
        RuntimeCurve::QuadraticBezier(bezier) | RuntimeCurve::CubicBezier(bezier) => {
            Ok(SketchCurve::Bezier(*bezier))
        }
        RuntimeCurve::Conic(conic) => Ok(SketchCurve::Conic(*conic)),
        RuntimeCurve::BSpline { .. } => {
            let (spline, span) = mappings
                .runtime_bspline_span(span)
                .ok_or_else(|| unknown_runtime("B-spline span", span.curve.0))?;
            Ok(SketchCurve::BSpline { spline, span })
        }
        RuntimeCurve::Nurbs { .. } => {
            let (nurbs, span) = mappings
                .runtime_nurbs_span(span)
                .ok_or_else(|| unknown_runtime("NURBS span", span.curve.0))?;
            Ok(SketchCurve::Nurbs { nurbs, span })
        }
    }
}

pub(crate) fn runtime_endpoint_contact(
    mappings: &DocumentRuntimeMap,
    span: CurveSpan,
    endpoint: FeatureEndpoint,
) -> Result<SketchCurveContact, DocumentError> {
    let curve = match mappings
        .runtime_curve(span.curve)
        .ok_or_else(|| unknown_runtime("curve", span.curve.0))?
    {
        RuntimeCurve::Line(_) | RuntimeCurve::Polyline(_) => SketchCurve::Line {
            segment: runtime_segment(mappings, span)?,
            domain: LineParameterDomain::BoundedSegment,
        },
        RuntimeCurve::CircularArc(arc) => SketchCurve::Arc(*arc),
        RuntimeCurve::QuadraticBezier(bezier) | RuntimeCurve::CubicBezier(bezier) => {
            SketchCurve::Bezier(*bezier)
        }
        RuntimeCurve::Conic(conic) => SketchCurve::Conic(*conic),
        RuntimeCurve::BSpline { .. } => {
            let (spline, span) = mappings
                .runtime_bspline_span(span)
                .ok_or_else(|| unknown_runtime("B-spline span", span.curve.0))?;
            SketchCurve::BSpline { spline, span }
        }
        RuntimeCurve::Nurbs { .. } => {
            let (nurbs, span) = mappings
                .runtime_nurbs_span(span)
                .ok_or_else(|| unknown_runtime("NURBS span", span.curve.0))?;
            SketchCurve::Nurbs { nurbs, span }
        }
        RuntimeCurve::Circle(_) => {
            return invalid_runtime("periodic curve has no bounded endpoint");
        }
    };
    Ok(SketchCurveContact {
        curve,
        parameter: match endpoint {
            FeatureEndpoint::Start => 0.0,
            FeatureEndpoint::End => 1.0,
        },
        neighborhood: match endpoint {
            FeatureEndpoint::Start => CurveContactNeighborhood::Start,
            FeatureEndpoint::End => CurveContactNeighborhood::End,
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
        RuntimeCurve::BSpline { .. } => Ok(DocumentContactRole::BSplineParameter),
        RuntimeCurve::Nurbs { .. } => Ok(DocumentContactRole::NurbsParameter),
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
        ContactDomain::SupportingLine => (value, 0),
        ContactDomain::Bounded { .. } => {
            let winding = if matches!(
                document
                    .curve(contact.curve.curve)
                    .ok_or_else(|| unknown_runtime("curve", contact.curve.curve.0))?
                    .definition,
                CurveDefinition::BSpline {
                    form: DocumentBSplineForm::Periodic,
                    ..
                } | CurveDefinition::Nurbs {
                    form: DocumentBSplineForm::Periodic,
                    ..
                }
            ) {
                contact.winding
            } else {
                0
            };
            (value, winding)
        }
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

const fn runtime_curve_direction(
    relation: DocumentCurveDirectionRelation,
) -> CurveDirectionRelation {
    match relation {
        DocumentCurveDirectionRelation::Tangent { orientation } => {
            CurveDirectionRelation::Tangent(match orientation {
                TangentOrientation::Aligned => CurveTangentOrientation::Aligned,
                TangentOrientation::Opposed => CurveTangentOrientation::Opposed,
            })
        }
        DocumentCurveDirectionRelation::Normal { side } => {
            CurveDirectionRelation::Normal(match side {
                DocumentCurveNormalSide::Left => CurveNormalSide::Left,
                DocumentCurveNormalSide::Right => CurveNormalSide::Right,
            })
        }
    }
}

const fn runtime_curve_normal_side(side: DocumentCurveNormalSide) -> CurveNormalSide {
    match side {
        DocumentCurveNormalSide::Left => CurveNormalSide::Left,
        DocumentCurveNormalSide::Right => CurveNormalSide::Right,
    }
}

const fn runtime_fillet_endpoint_order(order: DocumentFilletEndpointOrder) -> FilletEndpointOrder {
    match order {
        DocumentFilletEndpointOrder::FirstThenSecond => FilletEndpointOrder::FirstThenSecond,
        DocumentFilletEndpointOrder::SecondThenFirst => FilletEndpointOrder::SecondThenFirst,
    }
}

const fn runtime_curvature_relation(
    relation: DocumentCurveCurvatureRelation,
) -> CurveCurvatureRelation {
    match relation {
        DocumentCurveCurvatureRelation::Signed => CurveCurvatureRelation::Signed,
        DocumentCurveCurvatureRelation::MagnitudeSameSign => {
            CurveCurvatureRelation::MagnitudeSameSign
        }
        DocumentCurveCurvatureRelation::MagnitudeOppositeSign => {
            CurveCurvatureRelation::MagnitudeOppositeSign
        }
    }
}

const fn runtime_curve_continuity(kind: DocumentCurveContinuity) -> CurveContinuity {
    match kind {
        DocumentCurveContinuity::G0 => CurveContinuity::G0,
        DocumentCurveContinuity::G1 => CurveContinuity::G1,
        DocumentCurveContinuity::G2 => CurveContinuity::G2,
        DocumentCurveContinuity::ParametricC2 {
            first_rate,
            second_rate,
        } => CurveContinuity::ParametricC2 {
            first_rate,
            second_rate,
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
