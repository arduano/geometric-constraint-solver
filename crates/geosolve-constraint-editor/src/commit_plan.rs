// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic construction-plus-inference document plans.
//!
//! Inference is resolved before persistent identities for a construction exist.  The
//! deterministic slots in this module let that prospective intent refer either to an
//! existing document object or to an object occurrence allocated by the construction.
//! Applying a plan resolves those slots only after the geometry has been allocated, then
//! adds every inferred contact and constraint to the same cloned document.

use geosolve_sketch::{
    ContactDomain, ContactId, ContactNeighborhood, CurveId, CurveSpan, DesignPointId,
    DocumentCenterRef, DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentDirectionSense, DocumentError, DocumentLineSupportRef, DocumentSourceId, GeometryRole,
    OperationCheckpoint, OperationController, OperationWorkCounter, SketchDocument,
    TangentOrientation,
};

use crate::{ConstructionProposal, ConstructionResult};

/// Maximum inferred relations admitted by one atomic construction plan.
///
/// The ordinary inference engine emits at most one positional and one
/// directional relation. The larger public envelope leaves room for custom
/// embedders while bounding repeated document/contact cloning before retained
/// validation and solving begin.
pub const MAX_CONSTRUCTION_PLAN_RELATIONS: usize = 32;

/// One point operand available to a prospective inferred relation.
///
/// `Created` indexes [`ConstructionResult::points`] in allocation order.  It is an
/// occurrence index, not a geometric-equality lookup: repeated coordinates remain distinct
/// points unless the proposal explicitly reuses an `Existing` identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftPointSlot {
    Existing(DesignPointId),
    Created { point_index: usize },
}

/// One curve span available to a prospective inferred relation.
///
/// `Created.curve_index` indexes [`ConstructionResult::curves`] in allocation order.  The
/// segment remains explicit so individual live polyline spans can receive inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftSpanSlot {
    Existing(CurveSpan),
    Created { curve_index: usize, segment: u32 },
}

/// One curve operand available to a relation in the same construction transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftCurveSlot {
    Existing(CurveId),
    Created { curve_index: usize },
}

/// One directed affine support available to a same-transaction relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftLineSupportSlot {
    pub span: DraftSpanSlot,
    pub direction: DocumentDirectionSense,
}

/// Exact contact state retained when point-on-curve inference is committed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftContactDescriptor {
    pub span: DraftSpanSlot,
    pub domain: ContactDomain,
    pub parameter: f64,
    pub winding: i32,
    pub neighborhood: ContactNeighborhood,
}

/// One solver-backed relation inferred for a construction.
///
/// Reusing an existing point identity is deliberately not represented here: identity reuse
/// is already encoded by `ConstructionPoint::Existing` and must not create a redundant
/// coincidence source. A standalone Point-tool confirmation of that existing identity is
/// therefore a history-neutral no-op and emits no construction plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InferredRelation {
    CoincidentWithOrigin {
        point: DraftPointSlot,
    },
    PointOnDatumAxis {
        point: DraftPointSlot,
        axis: DocumentCoordinateAxis,
    },
    PointOnCurve {
        point: DraftPointSlot,
        contact: DraftContactDescriptor,
    },
    Midpoint {
        point: DraftPointSlot,
        line: DraftSpanSlot,
    },
    Horizontal {
        line: DraftSpanSlot,
    },
    Vertical {
        line: DraftSpanSlot,
    },
    HorizontalPoints {
        first: DraftPointSlot,
        second: DraftPointSlot,
    },
    VerticalPoints {
        first: DraftPointSlot,
        second: DraftPointSlot,
    },
    HorizontalPointToMidpoint {
        point: DraftPointSlot,
        line: DraftSpanSlot,
    },
    VerticalPointToMidpoint {
        point: DraftPointSlot,
        line: DraftSpanSlot,
    },
    Concentric {
        first: DraftCurveSlot,
        second: DraftCurveSlot,
    },
    Collinear {
        first: DraftLineSupportSlot,
        second: DraftLineSupportSlot,
    },
    Parallel {
        first: DraftSpanSlot,
        second: DraftSpanSlot,
    },
    Perpendicular {
        first: DraftSpanSlot,
        second: DraftSpanSlot,
    },
    /// Recipe-owned equality between two created or existing affine spans.
    ///
    /// This is deliberately part of the same atomic construction plan as the
    /// geometry.  In particular, holding the regularization modifier while
    /// authoring a rectangle cannot publish a transient rectangle and add its
    /// square relation in a second history step.
    EqualLength {
        first: DraftSpanSlot,
        second: DraftSpanSlot,
    },
    /// Recipe-owned generic tangency between an accepted native contact and a
    /// curve allocated by the same construction transaction.
    CurveCurveTangency {
        first: DraftContactDescriptor,
        second: DraftContactDescriptor,
        orientation: TangentOrientation,
    },
}

/// One contact allocated for a named relation occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionContactResult {
    pub relation_index: usize,
    pub contact: ContactId,
}

/// One persistent constraint/source pair allocated for a relation occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionConstraintResult {
    pub relation_index: usize,
    pub constraint: DocumentConstraintId,
    pub source: DocumentSourceId,
}

/// Persistent identities allocated by one atomic construction-plus-inference plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstructionCommitResult {
    pub construction: ConstructionResult,
    pub contacts: Vec<ConstructionContactResult>,
    pub constraints: Vec<ConstructionConstraintResult>,
}

/// A complete prospective construction and its exact inferred relations.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructionCommitPlan {
    pub proposal: ConstructionProposal,
    pub role: GeometryRole,
    pub relations: Vec<InferredRelation>,
}

impl ConstructionCommitPlan {
    /// Applies geometry, contact metadata, and inferred sources as one document mutation.
    ///
    /// This is document-level atomicity only.  Retained-session callers must still solve and
    /// independently validate the resulting candidate before publishing it or entering it in
    /// history.
    ///
    /// # Errors
    ///
    /// Returns a public document error when construction, slot resolution, contact creation,
    /// or constraint validation fails.  `document` remains byte-for-byte unchanged on error.
    pub fn apply(
        &self,
        document: &mut SketchDocument,
    ) -> Result<ConstructionCommitResult, DocumentError> {
        let Some(result) = self.apply_inner(document, None, |_| {})? else {
            unreachable!("an uncontrolled construction plan cannot be stopped");
        };
        Ok(result)
    }

    /// Applies a bounded plan while sharing the retained transaction's
    /// cancellation and deterministic-work controller.
    ///
    /// A stopped operation returns `Ok(None)` without changing `document`.
    pub(crate) fn apply_in_controller(
        &self,
        document: &mut SketchDocument,
        controller: &mut OperationController,
    ) -> Result<Option<ConstructionCommitResult>, DocumentError> {
        self.apply_inner(document, Some(controller), |_| {})
    }

    fn apply_inner<F>(
        &self,
        document: &mut SketchDocument,
        mut controller: Option<&mut OperationController>,
        mut after_relation: F,
    ) -> Result<Option<ConstructionCommitResult>, DocumentError>
    where
        F: FnMut(usize),
    {
        self.validate_relation_count()?;
        if controller.as_deref_mut().is_some_and(|controller| {
            controller
                .checkpoint(OperationCheckpoint::DocumentValidation)
                .is_err()
        }) {
            return Ok(None);
        }
        let mut candidate = document.clone();
        let construction = self.proposal.apply_with_role(&mut candidate, self.role)?;
        let mut contacts = Vec::new();
        let mut constraints = Vec::with_capacity(self.relations.len());

        for (relation_index, relation) in self.relations.iter().copied().enumerate() {
            if controller.as_deref_mut().is_some_and(|controller| {
                controller
                    .charge(
                        OperationWorkCounter::DocumentValidationItems,
                        1,
                        OperationCheckpoint::DocumentValidation,
                    )
                    .is_err()
            }) {
                return Ok(None);
            }
            let (definition, relation_contacts) =
                relation.resolve(&mut candidate, &construction, relation_index)?;
            for contact in relation_contacts {
                contacts.push(ConstructionContactResult {
                    relation_index,
                    contact,
                });
            }
            let constraint = candidate.add_constraint(relation.label(), definition)?;
            let source = candidate
                .constraint(constraint)
                .ok_or_else(|| invalid_slot("inferred constraint", "allocated source is absent"))?
                .source_id;
            constraints.push(ConstructionConstraintResult {
                relation_index,
                constraint,
                source,
            });
            after_relation(relation_index);
        }
        if controller.is_some_and(|controller| {
            controller
                .checkpoint(OperationCheckpoint::DocumentValidation)
                .is_err()
        }) {
            return Ok(None);
        }

        *document = candidate;
        Ok(Some(ConstructionCommitResult {
            construction,
            contacts,
            constraints,
        }))
    }

    pub(crate) fn validate_relation_count(&self) -> Result<(), DocumentError> {
        if self.relations.len() > MAX_CONSTRUCTION_PLAN_RELATIONS {
            return Err(DocumentError::ResourceLimit {
                resource: "construction plan inferred relations",
                actual: self.relations.len(),
                limit: MAX_CONSTRUCTION_PLAN_RELATIONS,
            });
        }
        Ok(())
    }
}

impl InferredRelation {
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive relation-to-document lowering keeps slot resolution auditable"
    )]
    fn resolve(
        self,
        document: &mut SketchDocument,
        construction: &ConstructionResult,
        relation_index: usize,
    ) -> Result<(DocumentConstraintDefinition, Vec<ContactId>), DocumentError> {
        let label = || format!("auto point-on-curve contact {}", relation_index + 1);
        Ok(match self {
            Self::CoincidentWithOrigin { point } => (
                DocumentConstraintDefinition::CoincidentWithOrigin {
                    point: point.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::PointOnDatumAxis { point, axis } => (
                DocumentConstraintDefinition::PointOnDatumAxis {
                    point: point.resolve(document, construction)?,
                    axis,
                },
                Vec::new(),
            ),
            Self::PointOnCurve { point, contact } => {
                resolve_point_on_curve(document, construction, point, contact, label())?
            }
            Self::Midpoint { point, line } => (
                DocumentConstraintDefinition::Midpoint {
                    point: point.resolve(document, construction)?,
                    line: line.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::Horizontal { line } => (
                DocumentConstraintDefinition::Horizontal {
                    line: line.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::Vertical { line } => (
                DocumentConstraintDefinition::Vertical {
                    line: line.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::HorizontalPoints { first, second } => (
                DocumentConstraintDefinition::HorizontalPoints {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::VerticalPoints { first, second } => (
                DocumentConstraintDefinition::VerticalPoints {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::HorizontalPointToMidpoint { point, line } => (
                DocumentConstraintDefinition::HorizontalPointToMidpoint {
                    point: point.resolve(document, construction)?,
                    line: line.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::VerticalPointToMidpoint { point, line } => (
                DocumentConstraintDefinition::VerticalPointToMidpoint {
                    point: point.resolve(document, construction)?,
                    line: line.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::Concentric { first, second } => (
                DocumentConstraintDefinition::Concentric {
                    first: DocumentCenterRef {
                        curve: first.resolve(document, construction)?,
                    },
                    second: DocumentCenterRef {
                        curve: second.resolve(document, construction)?,
                    },
                },
                Vec::new(),
            ),
            Self::Collinear { first, second } => (
                DocumentConstraintDefinition::Collinear {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::Parallel { first, second } => (
                DocumentConstraintDefinition::Parallel {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::Perpendicular { first, second } => (
                DocumentConstraintDefinition::Perpendicular {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::EqualLength { first, second } => (
                DocumentConstraintDefinition::EqualLength {
                    first: first.resolve(document, construction)?,
                    second: second.resolve(document, construction)?,
                },
                Vec::new(),
            ),
            Self::CurveCurveTangency {
                first,
                second,
                orientation,
            } => resolve_curve_curve_tangency(
                document,
                construction,
                first,
                second,
                orientation,
                relation_index,
            )?,
        })
    }

    const fn label(self) -> &'static str {
        match self {
            Self::CoincidentWithOrigin { .. } => "auto coincident with origin",
            Self::PointOnDatumAxis { .. } => "auto point on datum axis",
            Self::PointOnCurve { .. } => "auto point on curve",
            Self::Midpoint { .. } => "auto midpoint",
            Self::Horizontal { .. } => "auto horizontal",
            Self::Vertical { .. } => "auto vertical",
            Self::HorizontalPoints { .. } => "auto horizontal points",
            Self::VerticalPoints { .. } => "auto vertical points",
            Self::HorizontalPointToMidpoint { .. } => "auto horizontal to midpoint",
            Self::VerticalPointToMidpoint { .. } => "auto vertical to midpoint",
            Self::Concentric { .. } => "auto concentric",
            Self::Collinear { .. } => "auto collinear",
            Self::Parallel { .. } => "auto parallel",
            Self::Perpendicular { .. } => "auto perpendicular",
            Self::EqualLength { .. } => "recipe equal length",
            Self::CurveCurveTangency { .. } => "recipe tangent arc",
        }
    }
}

fn resolve_point_on_curve(
    document: &mut SketchDocument,
    construction: &ConstructionResult,
    point: DraftPointSlot,
    contact: DraftContactDescriptor,
    label: String,
) -> Result<(DocumentConstraintDefinition, Vec<ContactId>), DocumentError> {
    let point = point.resolve(document, construction)?;
    let span = contact.span.resolve(document, construction)?;
    let contact = document.add_curve_contact_with_domain(
        label,
        span,
        contact.domain,
        contact.parameter,
        contact.winding,
        contact.neighborhood,
        None,
    )?;
    Ok((
        DocumentConstraintDefinition::PointOnCurve { point, contact },
        vec![contact],
    ))
}

fn resolve_curve_curve_tangency(
    document: &mut SketchDocument,
    construction: &ConstructionResult,
    first: DraftContactDescriptor,
    second: DraftContactDescriptor,
    orientation: TangentOrientation,
    relation_index: usize,
) -> Result<(DocumentConstraintDefinition, Vec<ContactId>), DocumentError> {
    let first_span = first.span.resolve(document, construction)?;
    let first_contact = document.add_curve_contact_with_domain(
        format!("recipe tangent arc contact {}.1", relation_index + 1),
        first_span,
        first.domain,
        first.parameter,
        first.winding,
        first.neighborhood,
        Some(orientation),
    )?;
    let second_span = second.span.resolve(document, construction)?;
    let second_contact = document.add_curve_contact_with_domain(
        format!("recipe tangent arc contact {}.2", relation_index + 1),
        second_span,
        second.domain,
        second.parameter,
        second.winding,
        second.neighborhood,
        Some(orientation),
    )?;
    Ok((
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        },
        vec![first_contact, second_contact],
    ))
}

impl DraftPointSlot {
    fn resolve(
        self,
        document: &SketchDocument,
        construction: &ConstructionResult,
    ) -> Result<DesignPointId, DocumentError> {
        let point = match self {
            Self::Existing(point) => point,
            Self::Created { point_index } => construction
                .points
                .get(point_index)
                .copied()
                .ok_or_else(|| {
                    invalid_slot(
                        "draft point slot",
                        "created-point occurrence is outside the construction result",
                    )
                })?,
        };
        document.point(point).map(|_| point).ok_or_else(|| {
            invalid_slot(
                "draft point slot",
                "resolved point is absent from the candidate document",
            )
        })
    }
}

impl DraftSpanSlot {
    fn resolve(
        self,
        document: &SketchDocument,
        construction: &ConstructionResult,
    ) -> Result<CurveSpan, DocumentError> {
        let span = match self {
            Self::Existing(span) => span,
            Self::Created {
                curve_index,
                segment,
            } => CurveSpan {
                curve: construction
                    .curves
                    .get(curve_index)
                    .copied()
                    .ok_or_else(|| {
                        invalid_slot(
                            "draft span slot",
                            "created-curve occurrence is outside the construction result",
                        )
                    })?,
                segment,
            },
        };
        document.curve(span.curve).map(|_| span).ok_or_else(|| {
            invalid_slot(
                "draft span slot",
                "resolved curve is absent from the candidate document",
            )
        })
    }
}

impl DraftCurveSlot {
    fn resolve(
        self,
        document: &SketchDocument,
        construction: &ConstructionResult,
    ) -> Result<CurveId, DocumentError> {
        let curve = match self {
            Self::Existing(curve) => curve,
            Self::Created { curve_index } => construction
                .curves
                .get(curve_index)
                .copied()
                .ok_or_else(|| {
                    invalid_slot(
                        "draft curve slot",
                        "created-curve occurrence is outside the construction result",
                    )
                })?,
        };
        document.curve(curve).map(|_| curve).ok_or_else(|| {
            invalid_slot(
                "draft curve slot",
                "resolved curve is absent from the candidate document",
            )
        })
    }
}

impl DraftLineSupportSlot {
    fn resolve(
        self,
        document: &SketchDocument,
        construction: &ConstructionResult,
    ) -> Result<DocumentLineSupportRef, DocumentError> {
        Ok(DocumentLineSupportRef {
            span: self.span.resolve(document, construction)?,
            direction: self.direction,
        })
    }
}

fn invalid_slot(field: &'static str, message: &'static str) -> DocumentError {
    DocumentError::InvalidField {
        field,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        CurveDefinition, OperationControl, OperationOutcome, OperationReport, OperationStopReason,
        ScalarDomain, ScalarUnit, SketchDocument, cancellation_pair,
    };

    use super::*;
    use crate::ConstructionPoint;

    fn add_line(document: &mut SketchDocument, start: [f64; 2], end: [f64; 2]) -> CurveSpan {
        let delta = [end[0] - start[0], end[1] - start[1]];
        let length = delta[0].hypot(delta[1]);
        let start = document.add_point("start", start).expect("start point");
        let end = document.add_point("end", end).expect("end point");
        let curve = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .expect("line");
        CurveSpan::line(curve)
    }

    fn add_circle(document: &mut SketchDocument, center: [f64; 2], radius: f64) -> CurveSpan {
        let center = document.add_point("center", center).expect("center");
        let radius = document
            .add_scalar("radius", radius, ScalarUnit::Length, ScalarDomain::Positive)
            .expect("radius");
        let curve = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle");
        CurveSpan::line(curve)
    }

    #[test]
    fn created_span_relation_and_role_commit_together() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 0.0]),
                end: ConstructionPoint::New([3.0, 0.0]),
            },
            role: GeometryRole::Construction,
            relations: vec![InferredRelation::Horizontal {
                line: DraftSpanSlot::Created {
                    curve_index: 0,
                    segment: 0,
                },
            }],
        };

        let result = plan.apply(&mut document).expect("atomic commit");
        assert_eq!(result.construction.points.len(), 2);
        assert_eq!(result.construction.curves.len(), 1);
        assert!(result.contacts.is_empty());
        assert_eq!(result.constraints.len(), 1);
        let curve = result.construction.curves[0];
        assert_eq!(
            document.geometry_role(curve),
            Some(GeometryRole::Construction)
        );
        assert!(matches!(
            &document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .definition,
            DocumentConstraintDefinition::Horizontal { line }
                if *line == CurveSpan::line(curve)
        ));
        assert_eq!(
            document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .source_id,
            result.constraints[0].source
        );
    }

    #[test]
    fn point_on_curve_retains_exact_contact_metadata() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let target = add_circle(&mut document, [0.0, 0.0], 4.0);
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Point {
                point: ConstructionPoint::New([1.0, 0.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![InferredRelation::PointOnCurve {
                point: DraftPointSlot::Created { point_index: 0 },
                contact: DraftContactDescriptor {
                    span: DraftSpanSlot::Existing(target),
                    domain: ContactDomain::Periodic {
                        period: std::f64::consts::TAU,
                    },
                    parameter: 0.25,
                    winding: -2,
                    neighborhood: ContactNeighborhood::Interior,
                },
            }],
        };

        let result = plan.apply(&mut document).expect("point on curve");
        assert_eq!(result.contacts.len(), 1);
        assert_eq!(result.constraints.len(), 1);
        let contact_id = result.contacts[0].contact;
        let contact = document.contact(contact_id).expect("contact");
        assert_eq!(contact.curve, target);
        assert_eq!(
            contact.domain,
            ContactDomain::Periodic {
                period: std::f64::consts::TAU
            }
        );
        assert_eq!(contact.winding, -2);
        assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
        assert_eq!(
            document
                .scalar(contact.parameter)
                .expect("parameter")
                .value
                .to_bits(),
            0.25f64.to_bits()
        );
        assert!(matches!(
            &document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .definition,
            DocumentConstraintDefinition::PointOnCurve { point, contact }
                if *point == result.construction.points[0] && *contact == contact_id
        ));
    }

    #[test]
    fn existing_point_on_created_circle_resolves_reverse_incidence_atomically() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let point = document
            .add_point("existing edge point", [2.0, 0.0])
            .expect("existing point");
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Circle {
                center: ConstructionPoint::New([0.0, 0.0]),
                radius: 2.0,
            },
            role: GeometryRole::Profile,
            relations: vec![InferredRelation::PointOnCurve {
                point: DraftPointSlot::Existing(point),
                contact: DraftContactDescriptor {
                    span: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    domain: ContactDomain::Periodic {
                        period: std::f64::consts::TAU,
                    },
                    parameter: 0.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                },
            }],
        };

        let result = plan.apply(&mut document).expect("reverse-incidence commit");
        assert_eq!(result.construction.points.len(), 1);
        assert_eq!(result.construction.curves.len(), 1);
        assert_eq!(result.contacts.len(), 1);
        assert_eq!(result.constraints.len(), 1);
        assert_eq!(result.contacts[0].relation_index, 0);
        assert_eq!(result.constraints[0].relation_index, 0);

        let circle = CurveSpan::line(result.construction.curves[0]);
        let contact = result.contacts[0].contact;
        let contact_state = document.contact(contact).expect("contact");
        assert_eq!(contact_state.curve, circle);
        assert_eq!(
            contact_state.domain,
            ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            }
        );
        assert_eq!(contact_state.winding, 0);
        assert_eq!(contact_state.neighborhood, ContactNeighborhood::Interior);
        assert_eq!(
            document
                .scalar(contact_state.parameter)
                .expect("contact parameter")
                .value
                .to_bits(),
            0.0f64.to_bits()
        );
        assert_eq!(
            document
                .point(point)
                .expect("existing point")
                .position
                .map(f64::to_bits),
            [2.0_f64.to_bits(), 0.0_f64.to_bits()]
        );
        assert!(matches!(
            &document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .definition,
            DocumentConstraintDefinition::PointOnCurve {
                point: resolved_point,
                contact: resolved_contact,
            } if *resolved_point == point && *resolved_contact == contact
        ));
    }

    #[test]
    fn created_curve_slot_supports_same_transaction_concentric_relation() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let existing = add_circle(&mut document, [0.0, 0.0], 4.0).curve;
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Circle {
                center: ConstructionPoint::New([0.3, -0.2]),
                radius: 2.0,
            },
            role: GeometryRole::Profile,
            relations: vec![InferredRelation::Concentric {
                first: DraftCurveSlot::Created { curve_index: 0 },
                second: DraftCurveSlot::Existing(existing),
            }],
        };

        let result = plan.apply(&mut document).expect("concentric commit");
        let created = result.construction.curves[0];
        assert!(matches!(
            document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .definition,
            DocumentConstraintDefinition::Concentric { first, second }
                if first.curve == created && second.curve == existing
        ));
    }

    #[test]
    fn created_span_slot_supports_same_transaction_collinear_relation() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let existing = add_line(&mut document, [-3.0, 0.0], [-1.0, 0.0]);
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([1.0, 0.0]),
                end: ConstructionPoint::New([3.0, 0.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![InferredRelation::Collinear {
                first: DraftLineSupportSlot {
                    span: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    direction: DocumentDirectionSense::Reverse,
                },
                second: DraftLineSupportSlot {
                    span: DraftSpanSlot::Existing(existing),
                    direction: DocumentDirectionSense::Forward,
                },
            }],
        };

        let result = plan.apply(&mut document).expect("collinear commit");
        let created = CurveSpan::line(result.construction.curves[0]);
        assert!(matches!(
            document
                .constraint(result.constraints[0].constraint)
                .expect("constraint")
                .definition,
            DocumentConstraintDefinition::Collinear { first, second }
                if first.span == created
                    && first.direction == DocumentDirectionSense::Reverse
                    && second.span == existing
                    && second.direction == DocumentDirectionSense::Forward
        ));
    }

    #[test]
    fn invalid_created_curve_slot_rolls_back_geometry_and_allocations() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let existing = add_circle(&mut document, [0.0, 0.0], 4.0).curve;
        let before = document.clone();
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Circle {
                center: ConstructionPoint::New([0.3, -0.2]),
                radius: 2.0,
            },
            role: GeometryRole::Profile,
            relations: vec![InferredRelation::Concentric {
                first: DraftCurveSlot::Created { curve_index: 9 },
                second: DraftCurveSlot::Existing(existing),
            }],
        };

        assert!(plan.apply(&mut document).is_err());
        assert_eq!(document, before);
    }

    #[test]
    fn a_late_invalid_slot_retains_the_complete_original_document() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let before = document.clone();
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 0.0]),
                end: ConstructionPoint::New([3.0, 1.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![
                InferredRelation::Horizontal {
                    line: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                },
                InferredRelation::Vertical {
                    line: DraftSpanSlot::Created {
                        curve_index: 9,
                        segment: 0,
                    },
                },
            ],
        };

        assert!(plan.apply(&mut document).is_err());
        assert_eq!(document, before);
    }

    #[test]
    fn reused_point_identity_allocates_no_redundant_relation() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let existing = document
            .add_point("existing", [0.0, 0.0])
            .expect("existing point");
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::Existing {
                    id: existing,
                    position: [0.0, 0.0],
                },
                end: ConstructionPoint::New([2.0, 0.0]),
            },
            role: GeometryRole::Profile,
            relations: Vec::new(),
        };

        let result = plan.apply(&mut document).expect("reused endpoint");
        assert_eq!(result.construction.points.len(), 1);
        assert!(result.constraints.is_empty());
        assert!(document.constraints().is_empty());
        let CurveDefinition::Line { start, .. } = &document
            .curve(result.construction.curves[0])
            .expect("line")
            .definition
        else {
            panic!("expected line");
        };
        assert_eq!(*start, existing);
    }

    #[test]
    fn representative_relation_bundle_preserves_input_order() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let reference = add_line(&mut document, [0.0, 0.0], [1.0, 0.0]);
        let midpoint = document
            .add_point("midpoint", [0.5, 0.0])
            .expect("midpoint point");
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.5, 0.0]),
                end: ConstructionPoint::New([0.5, 2.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![
                InferredRelation::Midpoint {
                    point: DraftPointSlot::Existing(midpoint),
                    line: DraftSpanSlot::Existing(reference),
                },
                InferredRelation::Perpendicular {
                    first: DraftSpanSlot::Created {
                        curve_index: 0,
                        segment: 0,
                    },
                    second: DraftSpanSlot::Existing(reference),
                },
            ],
        };

        let result = plan.apply(&mut document).expect("relation bundle");
        assert_eq!(
            result
                .constraints
                .iter()
                .map(|result| result.relation_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(matches!(
            &document
                .constraint(result.constraints[0].constraint)
                .expect("midpoint")
                .definition,
            DocumentConstraintDefinition::Midpoint { .. }
        ));
        assert!(matches!(
            &document
                .constraint(result.constraints[1].constraint)
                .expect("perpendicular")
                .definition,
            DocumentConstraintDefinition::Perpendicular { .. }
        ));
    }

    #[test]
    fn oversized_relation_plan_rejects_before_any_document_mutation() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let before = document.clone();
        let relation = InferredRelation::Horizontal {
            line: DraftSpanSlot::Created {
                curve_index: 0,
                segment: 0,
            },
        };
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 0.0]),
                end: ConstructionPoint::New([2.0, 0.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![relation; MAX_CONSTRUCTION_PLAN_RELATIONS + 1],
        };

        assert!(matches!(
            plan.apply(&mut document),
            Err(DocumentError::ResourceLimit {
                resource: "construction plan inferred relations",
                actual,
                limit: MAX_CONSTRUCTION_PLAN_RELATIONS,
            }) if actual == MAX_CONSTRUCTION_PLAN_RELATIONS + 1
        ));
        assert_eq!(document, before);
    }

    #[test]
    fn cancellation_between_relations_discards_the_complete_plan_candidate() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let before = document.clone();
        let created = DraftSpanSlot::Created {
            curve_index: 0,
            segment: 0,
        };
        let plan = ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: ConstructionPoint::New([0.0, 0.0]),
                end: ConstructionPoint::New([2.0, 1.0]),
            },
            role: GeometryRole::Profile,
            relations: vec![
                InferredRelation::Horizontal { line: created },
                InferredRelation::Vertical { line: created },
            ],
        };
        let (cancellation, token) = cancellation_pair();
        let mut control = OperationControl::unlimited();
        control.token = token;
        let mut controller = OperationController::new(control);

        let stopped = plan
            .apply_inner(&mut document, Some(&mut controller), |relation_index| {
                if relation_index == 0 {
                    cancellation.cancel();
                }
            })
            .expect("controlled plan");
        assert!(stopped.is_none());
        assert_eq!(document, before);
        assert!(matches!(
            controller.outcome_unchecked::<()>(),
            OperationOutcome::Cancelled {
                report: OperationReport {
                    stopping_reason: Some(OperationStopReason::Cancelled {
                        checkpoint: OperationCheckpoint::DocumentValidation,
                    }),
                    ..
                }
            }
        ));
        assert_eq!(controller.report().consumed.document_validation_items, 1);
    }
}
