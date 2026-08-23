// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed canvas-authoring patches for the projectional intent session.
//!
//! This module translates the editor's already-resolved, equation-free M78
//! construction recipes into the closed intent vocabulary.  It does not apply
//! geometry directly and does not reproduce solver equations.

use std::collections::BTreeMap;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId,
    DocumentAngleOrientation, DocumentArcSweep, DocumentBSplineForm, DocumentCenterRef,
    DocumentCoordinateAxis, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentDimensionMode, DocumentDirectionSense, SketchDatum, SketchDocument, TangentOrientation,
};
use geosolve_sketch_intent::{
    ConstraintKind, DimensionKind as IntentDimensionKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentFieldKey, IntentIdentityFlow, IntentKey, IntentKeyError, IntentLiteral, IntentNodeDraft,
    IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionIdentity, IntentUnit, LeafField, PatchPortRef,
};
use thiserror::Error;

use crate::{
    AuthoringApplication, AuthoringTool, ConstraintRelationChoice, ConstructionCommitPlan,
    ConstructionPoint, ConstructionRelationDefinition, ConstructionRelationProvenance,
    DimensionKind as AuthoringDimensionKind, DraftContactDescriptor, DraftCurveSlot,
    DraftPointSlot, DraftSpanSlot, GeometryToolVariant, InferredRelation, IntentMaterializationMap,
    IntentNativeBinding, MAX_CONSTRUCTION_PLAN_RELATIONS, ResolvedConstraintKind, SelectionItem,
};

use crate::coordinator::{
    dimension_target, resolve_constraint, resolved_authoring_constraint_request,
};

/// A complete typed transaction derived from one authenticated M78 construction plan.
///
/// The geometry declaration is accompanied by ambient inferred relation
/// declarations. Recipe-intrinsic relations stay inside the recipe node and
/// are checked before conversion, so they are never duplicated as peer nodes.
#[derive(Clone, Debug)]
pub struct ProjectionalConstructionPatch {
    pub patch: IntentPatch,
    pub geometry_alias: IntentKey,
}

/// One complete typed relation or dimension transaction produced from the
/// ordinary headless authoring state.
#[derive(Clone, Debug)]
pub struct ProjectionalApplicationPatch {
    pub patch: IntentPatch,
    pub declaration_alias: IntentKey,
}

/// Fail-closed construction-to-intent translation error.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ProjectionalAuthoringError {
    #[error(transparent)]
    Key(#[from] IntentKeyError),
    #[error("the construction patch expected identity is stale for this intent session")]
    StaleIntentIdentity,
    #[error("the construction ownership map is stale or belongs to another intent state")]
    StaleOwnership,
    #[error("the construction plan exceeds the bounded native relation inventory")]
    RelationLimitExceeded,
    #[error("the construction proposal does not match its selected geometry recipe")]
    RecipeProposalMismatch,
    #[error("an existing native construction operand has no stable logical owner")]
    UnownedNativeOperand,
    #[error("a created construction slot is outside the recipe's typed output inventory")]
    InvalidCreatedSlot,
    #[error("the construction plan contains an unsupported inferred relation")]
    UnsupportedRelation,
    #[error("the contextual relation or dimension resolution no longer matches its operands")]
    StaleAuthoringResolution,
    #[error("the native contextual authoring request could not be represented: {0}")]
    InvalidAuthoringRequest(String),
    #[error("the construction plan's intrinsic relation inventory does not match the recipe")]
    IntrinsicRelationMismatch,
    #[error("the construction plan's curve-role inventory cannot be represented by the recipe")]
    CurveRoleMismatch,
    #[error("the construction proposal contains invalid finite geometry")]
    InvalidGeometry,
}

#[derive(Clone, Debug, Default)]
struct CreatedOutputs {
    points: Vec<PatchPortRef>,
    curves: Vec<PatchPortRef>,
    spans: BTreeMap<(usize, u32), PatchPortRef>,
}

impl CreatedOutputs {
    fn point(&self, index: usize) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        self.points
            .get(index)
            .cloned()
            .ok_or(ProjectionalAuthoringError::InvalidCreatedSlot)
    }

    fn curve(&self, index: usize) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        self.curves
            .get(index)
            .cloned()
            .ok_or(ProjectionalAuthoringError::InvalidCreatedSlot)
    }

    fn span(
        &self,
        curve_index: usize,
        segment: u32,
    ) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        self.spans
            .get(&(curve_index, segment))
            .cloned()
            .ok_or(ProjectionalAuthoringError::InvalidCreatedSlot)
    }
}

/// Converts one exact canvas construction into the canonical intent patch vocabulary.
///
/// Existing native operands are resolved through the accepted logical/native
/// ownership map. New outputs are referenced through transaction-local aliases,
/// so no caller predicts persistent IDs. All operands, intrinsic relations and
/// curve roles are authenticated before a patch is returned.
///
/// # Errors
///
/// Returns a typed mismatch, identity, topology, branch or key error. No intent
/// or native state is changed by this function.
pub fn projectional_construction_patch(
    expected: IntentSessionIdentity,
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    variant: GeometryToolVariant,
    plan: &ConstructionCommitPlan,
) -> Result<ProjectionalConstructionPatch, ProjectionalAuthoringError> {
    if expected != intent.identity() {
        return Err(ProjectionalAuthoringError::StaleIntentIdentity);
    }
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalAuthoringError::StaleOwnership);
    }
    if plan.relations.len() > MAX_CONSTRUCTION_PLAN_RELATIONS {
        return Err(ProjectionalAuthoringError::RelationLimitExceeded);
    }
    let geometry_alias = IntentKey::new(format!(
        "{}-{:016x}",
        variant.key(),
        expected.revision.raw().saturating_add(1)
    ))?;
    let mut context = DraftContext {
        intent,
        ownership,
        geometry_alias: geometry_alias.clone(),
        created: CreatedOutputs::default(),
    };
    let mut geometry = geometry_draft(variant, plan, &mut context)?;
    authenticate_curve_roles(variant, plan, &mut geometry)?;
    authenticate_recipe_relations(variant, plan, &context)?;

    let mut operations = vec![IntentPatchOperation::CreateNode {
        alias: geometry_alias.clone(),
        draft: Box::new(geometry),
        cell: None,
    }];
    let mut ambient_index = 0_u16;
    for (_, relation) in plan.ordered_effective_relations() {
        if relation.provenance != ConstructionRelationProvenance::AutoInference {
            continue;
        }
        let alias = IntentKey::new(format!(
            "{}-relation-{:04x}",
            geometry_alias.as_str(),
            ambient_index
        ))?;
        let draft = relation_draft(&context, &relation, alias.clone())?;
        operations.push(IntentPatchOperation::CreateNode {
            alias,
            draft: Box::new(draft),
            cell: None,
        });
        ambient_index = ambient_index
            .checked_add(1)
            .ok_or(ProjectionalAuthoringError::InvalidCreatedSlot)?;
    }

    Ok(ProjectionalConstructionPatch {
        patch: IntentPatch::new(expected, IntentPatchPolicy::RequireAccepted, operations),
        geometry_alias,
    })
}

/// Converts one complete contextual relation or dimension application into
/// the canonical intent patch vocabulary.
///
/// The existing authoring resolver remains the sole owner of applicability,
/// contact-domain defaults and explicit branch metadata. Native operands are
/// resolved only through the accepted logical/native ownership map, and the
/// returned declaration uses retain-failure policy just like an ordinary
/// explicit relation or dimension edit.
///
/// # Errors
///
/// Returns a typed stale identity/ownership, contextual-resolution, native-
/// request or operand-ownership error without changing intent or native state.
pub fn projectional_application_patch(
    expected: IntentSessionIdentity,
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    document: &SketchDocument,
    accepted_document: &SketchDocument,
    application: &AuthoringApplication,
) -> Result<ProjectionalApplicationPatch, ProjectionalAuthoringError> {
    if expected != intent.identity() {
        return Err(ProjectionalAuthoringError::StaleIntentIdentity);
    }
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalAuthoringError::StaleOwnership);
    }
    let selection = application
        .operands
        .iter()
        .map(|operand| operand.item)
        .collect::<Vec<_>>();
    let next_revision = expected.revision.raw().saturating_add(1);
    let (prefix, mut draft, display_name) = match application.tool {
        AuthoringTool::Constraint(authoring_intent) => {
            let resolved = resolve_constraint(document, &selection, authoring_intent)
                .map_err(|_| ProjectionalAuthoringError::StaleAuthoringResolution)?;
            if application.resolved_constraint != Some(resolved) {
                return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
            }
            let request = resolved_authoring_constraint_request(
                document,
                accepted_document,
                authoring_intent,
                resolved,
                &selection,
                &application.operands,
                application.options,
            )
            .map_err(|error| {
                ProjectionalAuthoringError::InvalidAuthoringRequest(error.to_string())
            })?;
            let draft = application_relation_draft(
                intent,
                ownership,
                document,
                &selection,
                resolved,
                &request,
                IntentKey::new(format!("relation-{next_revision:016x}"))?,
            )?;
            ("relation", draft, request.label)
        }
        AuthoringTool::Dimension(kind) => {
            if application.resolved_constraint.is_some() {
                return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
            }
            let target = dimension_target(
                accepted_document,
                &selection,
                kind,
                application.options.angle_orientation,
            )
            .map_err(|_| ProjectionalAuthoringError::StaleAuthoringResolution)?;
            let label = authoring_dimension_label(kind).to_owned();
            let draft = application_dimension_draft(
                intent,
                ownership,
                &selection,
                kind,
                application.options.dimension_mode,
                application.options.angle_orientation,
                target,
                IntentKey::new(format!("dimension-{next_revision:016x}"))?,
            )?;
            ("dimension", draft, label)
        }
    };
    let declaration_alias = IntentKey::new(format!("{prefix}-{next_revision:016x}"))?;
    draft = draft.with_display_name(IntentKey::new(display_name)?);
    Ok(ProjectionalApplicationPatch {
        patch: IntentPatch::new(
            expected,
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: declaration_alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ),
        declaration_alias,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive native contextual relation table keeps every branch and operand auditable"
)]
fn application_relation_draft(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    document: &SketchDocument,
    selection: &[SelectionItem],
    resolved: ResolvedConstraintKind,
    request: &crate::ConstraintActionRequest,
    symbol: IntentKey,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    use ConstraintKind as C;
    use ResolvedConstraintKind as R;

    let context = AcceptedOperandContext { intent, ownership };
    let points = selection
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Point(point) => Some(*point),
            _ => None,
        })
        .collect::<Vec<_>>();
    let spans = selection
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Curve(span) => Some(*span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let datum = selection.iter().find_map(|item| match item {
        SelectionItem::Datum(datum) => Some(*datum),
        _ => None,
    });
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: match resolved {
                R::FixedPoint => C::FixedPoint,
                R::CoincidentWithOrigin => C::CoincidentWithOrigin,
                R::PointOnDatumAxis => C::PointOnDatumAxis,
                R::CoincidentPoints => C::Coincident,
                R::PointOnCurve | R::RadialLine => C::PointOnCurve,
                R::CurveContact => C::CurveCurveContact,
                R::HorizontalLine => C::Horizontal,
                R::VerticalLine => C::Vertical,
                R::HorizontalPoints => C::HorizontalPoints,
                R::VerticalPoints => C::VerticalPoints,
                R::ConcentricCurves => C::Concentric,
                R::CollinearSupports => C::Collinear,
                R::CollinearWithDatumAxis => C::CollinearWithDatumAxis,
                R::ParallelLines => C::Parallel,
                R::PerpendicularLines => C::Perpendicular,
                R::EqualLength => C::EqualLength,
                R::EqualRadius => C::EqualRadius,
                R::EqualCurvature => C::EqualCurvature,
                R::Midpoint => C::Midpoint,
                R::SymmetricAboutLine => C::SymmetricAboutLine,
                R::SymmetricAboutDatumAxis => C::SymmetricAboutDatumAxis,
                R::CurveTangency => C::CurveCurveTangency,
                R::EndpointContinuity => C::EndpointContinuity,
            },
        },
        symbol,
    );

    match resolved {
        R::FixedPoint => {
            let [point] = points.as_slice() else {
                return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
            };
            draft = draft
                .with_input(InputSlot::new(InputRole::Point, 0), context.point(*point)?)
                .with_field(
                    IntentFieldKey(IntentKey::new("target")?),
                    IntentLiteral::Point(
                        document
                            .point(*point)
                            .ok_or(ProjectionalAuthoringError::StaleAuthoringResolution)?
                            .position,
                    ),
                );
        }
        R::CoincidentWithOrigin => {
            draft = with_points(draft, &context, &points, 1)?;
        }
        R::PointOnDatumAxis => {
            draft = with_points(draft, &context, &points, 1)?;
            draft = enum_field(draft, "axis", datum_axis_key(datum)?)?;
        }
        R::CoincidentPoints | R::HorizontalPoints | R::VerticalPoints => {
            draft = with_points(draft, &context, &points, 2)?;
        }
        R::HorizontalLine | R::VerticalLine => {
            draft = with_spans(draft, &context, &spans, 1)?;
        }
        R::ConcentricCurves | R::EqualRadius => {
            draft = with_curves(draft, &context, &spans, 2)?;
        }
        R::CollinearSupports => {
            draft = with_spans(draft, &context, &spans, 2)?;
            draft = enum_field(draft, "first_direction", "forward")?;
            draft = enum_field(draft, "second_direction", "forward")?;
        }
        R::CollinearWithDatumAxis => {
            draft = with_spans(draft, &context, &spans, 1)?;
            draft = enum_field(draft, "direction", "forward")?;
            draft = enum_field(draft, "axis", datum_axis_key(datum)?)?;
        }
        R::ParallelLines | R::PerpendicularLines | R::EqualLength => {
            draft = with_spans(draft, &context, &spans, 2)?;
        }
        R::Midpoint => {
            draft = with_points(draft, &context, &points, 1)?;
            draft = with_spans(draft, &context, &spans, 1)?;
        }
        R::SymmetricAboutLine => {
            draft = with_points(draft, &context, &points, 2)?;
            draft = with_spans(draft, &context, &spans, 1)?;
        }
        R::SymmetricAboutDatumAxis => {
            draft = with_points(draft, &context, &points, 2)?;
            draft = enum_field(draft, "axis", datum_axis_key(datum)?)?;
        }
        R::PointOnCurve => {
            draft = with_points(draft, &context, &points, 1)?;
            draft = with_spans(draft, &context, &spans, 1)?;
            draft = with_action_contact(draft, "contact", request.contacts.as_slice(), 0)?;
        }
        R::RadialLine => {
            let [contact] = request.contacts.as_slice() else {
                return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
            };
            let center_curve = spans
                .iter()
                .find(|span| **span != contact.support.span)
                .ok_or(ProjectionalAuthoringError::StaleAuthoringResolution)?
                .curve;
            let center = document
                .resolve_center_ref(DocumentCenterRef {
                    curve: center_curve,
                })
                .map_err(|_| ProjectionalAuthoringError::StaleAuthoringResolution)?;
            draft = draft.with_input(InputSlot::new(InputRole::Point, 0), context.point(center)?);
            draft = draft.with_input(
                InputSlot::new(InputRole::Span, 0),
                context.span(contact.support.span)?,
            );
            draft = with_action_contact(draft, "contact", request.contacts.as_slice(), 0)?;
        }
        R::CurveContact | R::CurveTangency | R::EqualCurvature | R::EndpointContinuity => {
            draft = with_spans(draft, &context, &spans, 2)?;
            draft = with_action_contact(draft, "first_contact", &request.contacts, 0)?;
            draft = with_action_contact(draft, "second_contact", &request.contacts, 1)?;
            match request.relation {
                Some(ConstraintRelationChoice::EqualCurvature(relation)) => {
                    draft = enum_field(draft, "relation", curvature_relation_key(relation))?;
                }
                Some(ConstraintRelationChoice::Continuity(continuity)) => {
                    draft = enum_field(draft, "continuity", continuity_key(continuity))?;
                    if let DocumentCurveContinuity::ParametricC2 {
                        first_rate,
                        second_rate,
                    } = continuity
                    {
                        let ratio = first_rate / second_rate;
                        if !ratio.is_finite() || ratio <= 0.0 {
                            return Err(ProjectionalAuthoringError::InvalidGeometry);
                        }
                        draft = draft.with_field(
                            IntentFieldKey(IntentKey::new("parameter_ratio")?),
                            dimensionless(ratio),
                        );
                    }
                }
                None if matches!(resolved, R::CurveContact | R::CurveTangency) => {}
                _ => return Err(ProjectionalAuthoringError::StaleAuthoringResolution),
            }
        }
    }
    Ok(draft)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the complete native dimension application is one atomic typed declaration"
)]
fn application_dimension_draft(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    selection: &[SelectionItem],
    kind: AuthoringDimensionKind,
    mode: DocumentDimensionMode,
    orientation: DocumentAngleOrientation,
    target: f64,
    symbol: IntentKey,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    let context = AcceptedOperandContext { intent, ownership };
    let points = selection
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Point(point) => Some(*point),
            _ => None,
        })
        .collect::<Vec<_>>();
    let spans = selection
        .iter()
        .filter_map(|item| match item {
            SelectionItem::Curve(span) => Some(*span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let intent_kind = match kind {
        AuthoringDimensionKind::PointDistance => IntentDimensionKind::PointDistance,
        AuthoringDimensionKind::SegmentLength => IntentDimensionKind::CurveLength,
        AuthoringDimensionKind::Radius => IntentDimensionKind::Radius,
        AuthoringDimensionKind::Diameter => IntentDimensionKind::Diameter,
        AuthoringDimensionKind::OrientedAngle => IntentDimensionKind::OrientedAngle,
    };
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: intent_kind,
        },
        symbol,
    );
    draft = enum_field(
        draft,
        "mode",
        match mode {
            DocumentDimensionMode::Driving => "driving",
            DocumentDimensionMode::Reference => "reference",
        },
    )?;
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        target,
        if kind == AuthoringDimensionKind::OrientedAngle {
            IntentUnit::Angle
        } else {
            IntentUnit::Length
        },
    );
    match kind {
        AuthoringDimensionKind::PointDistance => {
            draft = with_points(draft, &context, &points, 2)?;
        }
        AuthoringDimensionKind::SegmentLength => {
            draft = with_spans(draft, &context, &spans, 1)?;
        }
        AuthoringDimensionKind::Radius | AuthoringDimensionKind::Diameter => {
            draft = with_curves(draft, &context, &spans, 1)?;
        }
        AuthoringDimensionKind::OrientedAngle => {
            draft = with_spans(draft, &context, &spans, 2)?;
            draft = enum_field(
                draft,
                "orientation",
                match orientation {
                    DocumentAngleOrientation::CounterClockwise => "counter_clockwise",
                    DocumentAngleOrientation::Clockwise => "clockwise",
                },
            )?;
        }
    }
    Ok(draft)
}

struct AcceptedOperandContext<'a> {
    intent: &'a IntentSession,
    ownership: &'a IntentMaterializationMap,
}

impl AcceptedOperandContext<'_> {
    fn point(&self, point: DesignPointId) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        accepted_native_port(
            self.intent,
            self.ownership,
            IntentNativeBinding::Point(point),
        )
    }

    fn curve(&self, curve: CurveId) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        accepted_native_port(
            self.intent,
            self.ownership,
            IntentNativeBinding::Curve(curve),
        )
    }

    fn span(&self, span: CurveSpan) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        accepted_native_port(
            self.intent,
            self.ownership,
            IntentNativeBinding::CurveSpan(span),
        )
    }
}

fn with_points(
    mut draft: IntentNodeDraft,
    context: &AcceptedOperandContext<'_>,
    points: &[DesignPointId],
    expected: usize,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    if points.len() != expected {
        return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
    }
    for (index, point) in points.iter().enumerate() {
        draft = draft.with_input(
            InputSlot::new(InputRole::Point, u16_index(index)?),
            context.point(*point)?,
        );
    }
    Ok(draft)
}

fn with_spans(
    mut draft: IntentNodeDraft,
    context: &AcceptedOperandContext<'_>,
    spans: &[CurveSpan],
    expected: usize,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    if spans.len() != expected {
        return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
    }
    for (index, span) in spans.iter().enumerate() {
        draft = draft.with_input(
            InputSlot::new(InputRole::Span, u16_index(index)?),
            context.span(*span)?,
        );
    }
    Ok(draft)
}

fn with_curves(
    mut draft: IntentNodeDraft,
    context: &AcceptedOperandContext<'_>,
    spans: &[CurveSpan],
    expected: usize,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    if spans.len() != expected {
        return Err(ProjectionalAuthoringError::StaleAuthoringResolution);
    }
    for (index, span) in spans.iter().enumerate() {
        draft = draft.with_input(
            InputSlot::new(InputRole::Curve, u16_index(index)?),
            context.curve(span.curve)?,
        );
    }
    Ok(draft)
}

fn with_action_contact(
    mut draft: IntentNodeDraft,
    prefix: &str,
    contacts: &[crate::ContactActionChoice],
    index: usize,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    let contact = contacts
        .get(index)
        .ok_or(ProjectionalAuthoringError::StaleAuthoringResolution)?;
    let descriptor = DraftContactDescriptor {
        span: DraftSpanSlot::Existing(contact.support.span),
        domain: contact.domain,
        parameter: contact.parameter,
        winding: contact.support.winding,
        neighborhood: contact.neighborhood,
    };
    for (name, value) in contact_field_values(prefix, descriptor, contact.tangent_orientation)? {
        draft = draft.with_field(IntentFieldKey(IntentKey::new(name)?), value);
    }
    Ok(draft)
}

fn datum_axis_key(datum: Option<SketchDatum>) -> Result<&'static str, ProjectionalAuthoringError> {
    match datum {
        Some(SketchDatum::XAxis) => Ok("x"),
        Some(SketchDatum::YAxis) => Ok("y"),
        Some(SketchDatum::Origin) | None => {
            Err(ProjectionalAuthoringError::StaleAuthoringResolution)
        }
    }
}

const fn curvature_relation_key(value: DocumentCurveCurvatureRelation) -> &'static str {
    match value {
        DocumentCurveCurvatureRelation::Signed => "signed",
        DocumentCurveCurvatureRelation::MagnitudeSameSign => "magnitude_same_sign",
        DocumentCurveCurvatureRelation::MagnitudeOppositeSign => "magnitude_opposite_sign",
    }
}

const fn continuity_key(value: DocumentCurveContinuity) -> &'static str {
    match value {
        DocumentCurveContinuity::G0 => "g0",
        DocumentCurveContinuity::G1 => "g1",
        DocumentCurveContinuity::G2 => "g2",
        DocumentCurveContinuity::ParametricC2 { .. } => "parametric_c2",
    }
}

const fn authoring_dimension_label(kind: AuthoringDimensionKind) -> &'static str {
    match kind {
        AuthoringDimensionKind::PointDistance => "Point distance",
        AuthoringDimensionKind::SegmentLength => "Segment length",
        AuthoringDimensionKind::Radius => "Radius",
        AuthoringDimensionKind::Diameter => "Diameter",
        AuthoringDimensionKind::OrientedAngle => "Oriented angle",
    }
}

struct DraftContext<'a> {
    intent: &'a IntentSession,
    ownership: &'a IntentMaterializationMap,
    geometry_alias: IntentKey,
    created: CreatedOutputs,
}

impl DraftContext<'_> {
    fn alias(&self, selector: IntentPortSelector) -> PatchPortRef {
        PatchPortRef::Alias {
            node: self.geometry_alias.clone(),
            selector,
        }
    }

    fn native_port(
        &self,
        binding: IntentNativeBinding,
    ) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        accepted_native_port(self.intent, self.ownership, binding)
    }

    fn point_slot(&self, slot: DraftPointSlot) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        match slot {
            DraftPointSlot::Existing(point) => self.native_port(IntentNativeBinding::Point(point)),
            DraftPointSlot::Created { point_index } => self.created.point(point_index),
        }
    }

    fn span_slot(&self, slot: DraftSpanSlot) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        match slot {
            DraftSpanSlot::Existing(span) => self.native_port(IntentNativeBinding::CurveSpan(span)),
            DraftSpanSlot::Created {
                curve_index,
                segment,
            } => self.created.span(curve_index, segment),
        }
    }

    fn curve_slot(&self, slot: DraftCurveSlot) -> Result<PatchPortRef, ProjectionalAuthoringError> {
        match slot {
            DraftCurveSlot::Existing(curve) => self.native_port(IntentNativeBinding::Curve(curve)),
            DraftCurveSlot::Created { curve_index } => self.created.curve(curve_index),
        }
    }
}

fn accepted_native_port(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    binding: IntentNativeBinding,
) -> Result<PatchPortRef, ProjectionalAuthoringError> {
    let mut candidates = ownership
        .ports
        .iter()
        .filter_map(|(port, candidate)| (*candidate == binding).then_some(*port))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|port| {
        let flow_rank = intent
            .graph()
            .node(port.node)
            .and_then(|node| node.port(port.port))
            .map_or(4, |port| match port.flow {
                IntentIdentityFlow::Created { .. } => 0,
                IntentIdentityFlow::Continued { .. } => 1,
                IntentIdentityFlow::Aliased { .. } => 2,
                IntentIdentityFlow::OwnedLogical => 3,
                IntentIdentityFlow::Retired { .. } => 4,
            });
        (flow_rank, *port)
    });
    candidates
        .into_iter()
        .next()
        .map(|port| PatchPortRef::Stable { port })
        .ok_or(ProjectionalAuthoringError::UnownedNativeOperand)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive match keeps all 25 authenticated recipe translations visibly closed"
)]
fn geometry_draft(
    variant: GeometryToolVariant,
    plan: &ConstructionCommitPlan,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    use crate::ConstructionProposal as P;
    use GeometryToolVariant as V;

    let recipe = recipe_kind(variant);
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry { recipe },
        context.geometry_alias.clone(),
    );
    match (variant, &plan.proposal) {
        (V::SketchPoint, P::Point { point }) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Primary, 0),
                Some(0),
                *point,
                context,
            )?;
        }
        (V::Segment, P::Line { start, end }) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Start, 0),
                Some(0),
                *start,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::End, 0),
                Some(1),
                *end,
                context,
            )?;
            draft = point_field(draft, "branch_direction", unit_direction(*start, *end)?)?;
            register_single_curve(context, 1)?;
        }
        (V::Polyline, P::Polyline { points }) => {
            draft = polyline_draft(draft, points, false, context)?;
        }
        (V::Polyline, P::PolylinePath { points, closed }) => {
            draft = polyline_draft(draft, points, *closed, context)?;
        }
        (
            V::MidpointLine,
            P::MidpointLine {
                center,
                endpoint,
                opposite,
            },
        ) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Midpoint, 0),
                Some(0),
                *center,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::End, 0),
                Some(1),
                *endpoint,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Start, 0),
                None,
                *opposite,
                context,
            )?;
            draft = point_field(
                draft,
                "branch_direction",
                unit_direction(*opposite, *endpoint)?,
            )?;
            register_single_curve(context, 1)?;
        }
        (
            V::TwoPointAlignedRectangle
            | V::ThreePointCornerRectangle
            | V::CenterRectangle
            | V::ThreePointCenterRectangle,
            P::RectangleLoop {
                points,
                corners,
                center,
            },
        ) => {
            draft = rectangle_draft(variant, draft, points, *corners, *center, context)?;
            let regularized = plan.relations.iter().any(|relation| {
                relation.provenance == ConstructionRelationProvenance::RecipeRegularization
            });
            draft = bool_field(draft, "regularized", regularized)?;
        }
        (
            V::CenterRadiusCircle | V::TwoPointDiameterCircle | V::ThreePointCircle,
            P::Circle { center, radius },
        ) => {
            let input = (variant == V::CenterRadiusCircle).then_some(0);
            draft = bind_point(
                draft,
                selector(IntentPortRole::Center, 0),
                input,
                *center,
                context,
            )?;
            draft = quantity_leaf(
                draft,
                selector(IntentPortRole::Target, 0),
                LeafField::Value,
                *radius,
                IntentUnit::Length,
            );
            register_single_curve(context, 1)?;
        }
        (
            V::CenterArc | V::ThreePointArc | V::TangentArc,
            P::CircularArc {
                center,
                start,
                end,
                sweep,
            },
        ) => {
            draft = circular_arc_draft(variant, draft, *center, *start, *end, *sweep, context)?;
            if variant == V::TangentArc {
                draft = tangent_arc_fields(draft, plan, context)?;
            }
        }
        (V::CenterArc, P::CounterClockwiseArc { center, start, end }) => {
            draft = circular_arc_draft(
                variant,
                draft,
                *center,
                *start,
                *end,
                DocumentArcSweep::CounterClockwise,
                context,
            )?;
        }
        (
            V::CenterAxesEllipse,
            P::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            },
        ) => {
            draft = ellipse_draft(
                draft,
                *center,
                *major_axis_point,
                *minor_axis_ratio,
                true,
                context,
            )?;
        }
        (
            V::AxisEndpointsEllipse,
            P::AxisEndpointEllipse {
                major_axis_point,
                center,
                minor_axis_ratio,
            },
        ) => {
            draft = ellipse_draft(
                draft,
                *center,
                *major_axis_point,
                *minor_axis_ratio,
                false,
                context,
            )?;
        }
        (
            V::CenterAxesEllipticalArc,
            P::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            },
        ) => {
            draft = elliptical_arc_draft(
                draft,
                *center,
                *major_axis_point,
                *minor_axis_ratio,
                *start_angle,
                *end_angle,
                *sweep,
                true,
                context,
            )?;
        }
        (
            V::AxisEndpointsEllipticalArc,
            P::AxisEndpointEllipticalArc {
                major_axis_point,
                center,
                minor_axis_ratio,
                start_angle,
                end_angle,
                sweep,
            },
        ) => {
            draft = elliptical_arc_draft(
                draft,
                *center,
                *major_axis_point,
                *minor_axis_ratio,
                *start_angle,
                *end_angle,
                *sweep,
                false,
                context,
            )?;
        }
        (V::QuadraticBezier, P::QuadraticBezier { controls }) => {
            for (index, (role, point)) in [
                (IntentPortRole::Start, controls[0]),
                (IntentPortRole::Control, controls[1]),
                (IntentPortRole::End, controls[2]),
            ]
            .into_iter()
            .enumerate()
            {
                draft = bind_point(
                    draft,
                    selector(role, 0),
                    Some(u16_index(index)?),
                    point,
                    context,
                )?;
            }
            register_single_curve(context, 1)?;
        }
        (V::CubicBezier, P::CubicBezier { controls }) => {
            for (index, (role, role_index, point)) in [
                (IntentPortRole::Start, 0, controls[0]),
                (IntentPortRole::Control, 0, controls[1]),
                (IntentPortRole::Control, 1, controls[2]),
                (IntentPortRole::End, 0, controls[3]),
            ]
            .into_iter()
            .enumerate()
            {
                draft = bind_point(
                    draft,
                    selector(role, role_index),
                    Some(u16_index(index)?),
                    point,
                    context,
                )?;
            }
            register_single_curve(context, 1)?;
        }
        (
            V::RationalQuadraticConic,
            P::RationalQuadraticConic {
                start,
                weighted_middle,
                middle_weight,
                end,
            },
        ) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Start, 0),
                Some(0),
                *start,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::End, 0),
                Some(1),
                *end,
                context,
            )?;
            draft = point_field(draft, "weighted_middle", *weighted_middle)?;
            draft = quantity_leaf(
                draft,
                selector(IntentPortRole::Target, 0),
                LeafField::Weight,
                *middle_weight,
                IntentUnit::Dimensionless,
            );
            register_single_curve(context, 1)?;
        }
        (
            V::Parabola,
            P::Parabola {
                vertex,
                focus,
                trim_start,
                trim_end,
            },
        ) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Center, 0),
                Some(0),
                *vertex,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Control, 0),
                Some(1),
                *focus,
                context,
            )?;
            draft = parameter_leaf(draft, 0, *trim_start);
            draft = parameter_leaf(draft, 1, *trim_end);
            register_single_curve(context, 1)?;
        }
        (
            V::Hyperbola,
            P::Hyperbola {
                center,
                transverse_axis_point,
                semi_conjugate,
                branch,
                trim_start,
                trim_end,
            },
        ) => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Center, 0),
                Some(0),
                *center,
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Control, 0),
                Some(1),
                *transverse_axis_point,
                context,
            )?;
            draft = quantity_leaf(
                draft,
                selector(IntentPortRole::Target, 0),
                LeafField::Value,
                *semi_conjugate,
                IntentUnit::Length,
            );
            draft = parameter_leaf(draft, 1, *trim_start);
            draft = parameter_leaf(draft, 2, *trim_end);
            draft = enum_field(
                draft,
                "branch",
                match branch {
                    geosolve_sketch::DocumentHyperbolaBranch::Positive => "positive",
                    geosolve_sketch::DocumentHyperbolaBranch::Negative => "negative",
                },
            )?;
            register_single_curve(context, 1)?;
        }
        (V::OpenControlNurbs | V::PeriodicControlNurbs, P::Nurbs { controls, options }) => {
            let expected_form = if variant == V::OpenControlNurbs {
                DocumentBSplineForm::Clamped
            } else {
                DocumentBSplineForm::Periodic
            };
            if options.form != expected_form {
                return Err(ProjectionalAuthoringError::RecipeProposalMismatch);
            }
            draft = draft.with_dynamic_children(u16_index(controls.len())?);
            draft = natural_field(draft, "degree", u64::from(options.degree))?;
            draft = natural_field(
                draft,
                "gauge_index",
                u64::try_from(options.gauge_index)
                    .map_err(|_| ProjectionalAuthoringError::InvalidGeometry)?,
            )?;
            let weights = if options.weights.is_empty() {
                vec![1.0; controls.len()]
            } else {
                options.weights.clone()
            };
            if weights.len() != controls.len() {
                return Err(ProjectionalAuthoringError::InvalidGeometry);
            }
            for (index, (point, weight)) in controls.iter().zip(weights).enumerate() {
                let ordinal = u16_index(index)?;
                let point_selector = child_selector(ordinal, IntentPortRole::Control);
                draft = bind_point(draft, point_selector, Some(ordinal), *point, context)?;
                draft = quantity_leaf(
                    draft,
                    child_selector(ordinal, IntentPortRole::Target),
                    LeafField::Weight,
                    weight,
                    IntentUnit::Dimensionless,
                );
            }
            register_nurbs_curve(
                context,
                nurbs_span_count(options.form, options.degree, controls.len())?,
            )?;
        }
        _ => return Err(ProjectionalAuthoringError::RecipeProposalMismatch),
    }
    Ok(draft)
}

fn recipe_kind(variant: GeometryToolVariant) -> GeometryRecipeKind {
    use GeometryRecipeKind as R;
    use GeometryToolVariant as V;
    match variant {
        V::SketchPoint => R::SketchPoint,
        V::Segment => R::Segment,
        V::Polyline => R::Polyline,
        V::MidpointLine => R::MidpointLine,
        V::TwoPointAlignedRectangle => R::TwoPointAlignedRectangle,
        V::ThreePointCornerRectangle => R::ThreePointCornerRectangle,
        V::CenterRectangle => R::CenterRectangle,
        V::ThreePointCenterRectangle => R::ThreePointCenterRectangle,
        V::CenterRadiusCircle => R::CenterRadiusCircle,
        V::TwoPointDiameterCircle => R::TwoPointDiameterCircle,
        V::ThreePointCircle => R::ThreePointCircle,
        V::CenterArc => R::CenterArc,
        V::ThreePointArc => R::ThreePointArc,
        V::TangentArc => R::TangentArc,
        V::CenterAxesEllipse => R::CenterAxesEllipse,
        V::AxisEndpointsEllipse => R::AxisEndpointsEllipse,
        V::CenterAxesEllipticalArc => R::CenterAxesEllipticalArc,
        V::AxisEndpointsEllipticalArc => R::AxisEndpointsEllipticalArc,
        V::QuadraticBezier => R::QuadraticBezier,
        V::CubicBezier => R::CubicBezier,
        V::RationalQuadraticConic => R::RationalQuadraticConic,
        V::Parabola => R::Parabola,
        V::Hyperbola => R::Hyperbola,
        V::OpenControlNurbs => R::OpenControlNurbs,
        V::PeriodicControlNurbs => R::PeriodicControlNurbs,
    }
}

fn bind_point(
    mut draft: IntentNodeDraft,
    port: IntentPortSelector,
    input_index: Option<u16>,
    point: ConstructionPoint,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    match point {
        ConstructionPoint::Existing { id, .. } => {
            let index = input_index.ok_or(ProjectionalAuthoringError::RecipeProposalMismatch)?;
            draft = draft.with_input(
                InputSlot::new(InputRole::Point, index),
                context.native_port(IntentNativeBinding::Point(id))?,
            );
        }
        ConstructionPoint::New(position) => {
            if !position.into_iter().all(f64::is_finite) {
                return Err(ProjectionalAuthoringError::InvalidGeometry);
            }
            draft = point_leaves(draft, port, position);
            context.created.points.push(context.alias(port));
        }
    }
    Ok(draft)
}

fn polyline_draft(
    mut draft: IntentNodeDraft,
    points: &[ConstructionPoint],
    closed: bool,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    draft = draft.with_dynamic_children(u16_index(points.len())?);
    draft = bool_field(draft, "closed", closed)?;
    for (index, point) in points.iter().copied().enumerate() {
        let ordinal = u16_index(index)?;
        draft = bind_point(
            draft,
            child_selector(ordinal, IntentPortRole::Corner),
            Some(ordinal),
            point,
            context,
        )?;
    }
    let span_count = points
        .len()
        .checked_sub(1)
        .and_then(|count| count.checked_add(usize::from(closed)))
        .ok_or(ProjectionalAuthoringError::InvalidGeometry)?;
    register_polyline_curve(context, span_count)?;
    Ok(draft)
}

#[allow(
    clippy::too_many_lines,
    reason = "the four rectangle recipes share one exact authored-corner allocation audit"
)]
fn rectangle_draft(
    variant: GeometryToolVariant,
    mut draft: IntentNodeDraft,
    points: &[ConstructionPoint],
    corners: [usize; 4],
    center: Option<usize>,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    use GeometryToolVariant as V;

    if corners.iter().any(|index| *index >= points.len())
        || center.is_some_and(|index| index >= points.len())
    {
        return Err(ProjectionalAuthoringError::InvalidGeometry);
    }
    match variant {
        V::TwoPointAlignedRectangle => {
            draft = bind_point(
                draft,
                selector(IntentPortRole::Corner, 0),
                Some(0),
                points[corners[0]],
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Corner, 2),
                Some(1),
                points[corners[2]],
                context,
            )?;
            for index in [1_usize, 3] {
                draft = bind_point(
                    draft,
                    selector(IntentPortRole::Corner, u16_index(index)?),
                    None,
                    points[corners[index]],
                    context,
                )?;
            }
        }
        V::ThreePointCornerRectangle => {
            for index in 0..3 {
                draft = bind_point(
                    draft,
                    selector(IntentPortRole::Corner, u16_index(index)?),
                    Some(u16_index(index)?),
                    points[corners[index]],
                    context,
                )?;
            }
            draft = bind_point(
                draft,
                selector(IntentPortRole::Corner, 3),
                None,
                points[corners[3]],
                context,
            )?;
        }
        V::CenterRectangle => {
            let center = center.ok_or(ProjectionalAuthoringError::RecipeProposalMismatch)?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Center, 0),
                Some(0),
                points[center],
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Corner, 0),
                Some(1),
                points[corners[0]],
                context,
            )?;
            for index in 1..4 {
                draft = bind_point(
                    draft,
                    selector(IntentPortRole::Corner, u16_index(index)?),
                    None,
                    points[corners[index]],
                    context,
                )?;
            }
        }
        V::ThreePointCenterRectangle => {
            let center = center.ok_or(ProjectionalAuthoringError::RecipeProposalMismatch)?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Center, 0),
                Some(0),
                points[center],
                context,
            )?;
            draft = bind_point(
                draft,
                selector(IntentPortRole::Corner, 0),
                Some(1),
                points[corners[0]],
                context,
            )?;
            for index in 1..4 {
                draft = bind_point(
                    draft,
                    selector(IntentPortRole::Corner, u16_index(index)?),
                    None,
                    points[corners[index]],
                    context,
                )?;
            }
            let first = point_position(points[corners[0]]);
            let second = point_position(points[corners[1]]);
            draft = point_field(draft, "side_midpoint", finite_midpoint(first, second)?)?;
        }
        _ => return Err(ProjectionalAuthoringError::RecipeProposalMismatch),
    }
    for curve_index in 0..4 {
        register_curve(context, curve_index, 1)?;
    }
    if center.is_some() {
        register_curve(context, 4, 1)?;
    }
    Ok(draft)
}

fn circular_arc_draft(
    variant: GeometryToolVariant,
    mut draft: IntentNodeDraft,
    center: ConstructionPoint,
    start: [f64; 2],
    end: [f64; 2],
    sweep: DocumentArcSweep,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    draft = bind_point(
        draft,
        selector(IntentPortRole::Center, 0),
        (variant == GeometryToolVariant::CenterArc).then_some(0),
        center,
        context,
    )?;
    let center_position = point_position(center);
    let radius = distance(center_position, start)?;
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 0),
        LeafField::Value,
        radius,
        IntentUnit::Length,
    );
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 1),
        LeafField::Angle,
        angle_from(center_position, start)?,
        IntentUnit::Angle,
    );
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 2),
        LeafField::Angle,
        angle_from(center_position, end)?,
        IntentUnit::Angle,
    );
    draft = enum_field(draft, "sweep", sweep_key(sweep))?;
    register_single_curve(context, 1)?;
    Ok(draft)
}

fn tangent_arc_fields(
    mut draft: IntentNodeDraft,
    plan: &ConstructionCommitPlan,
    context: &DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    let relation = plan
        .relations
        .iter()
        .find(|relation| relation.provenance == ConstructionRelationProvenance::RecipeIntrinsic)
        .ok_or(ProjectionalAuthoringError::IntrinsicRelationMismatch)?;
    let InferredRelation::CurveCurveTangency {
        first,
        second,
        orientation,
    } = relation.relation
    else {
        return Err(ProjectionalAuthoringError::IntrinsicRelationMismatch);
    };
    let DraftSpanSlot::Existing(source) = first.span else {
        return Err(ProjectionalAuthoringError::IntrinsicRelationMismatch);
    };
    if !matches!(
        second.span,
        DraftSpanSlot::Created {
            curve_index: 0,
            segment: 0
        }
    ) {
        return Err(ProjectionalAuthoringError::IntrinsicRelationMismatch);
    }
    draft = draft.with_input(
        InputSlot::new(InputRole::Span, 0),
        context.native_port(IntentNativeBinding::CurveSpan(source))?,
    );
    draft = contact_fields_without_orientation(draft, "source", first)?;
    enum_field(draft, "orientation", orientation_key(orientation))
}

fn ellipse_draft(
    mut draft: IntentNodeDraft,
    center: ConstructionPoint,
    major: ConstructionPoint,
    ratio: f64,
    center_authored: bool,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    if center_authored {
        draft = bind_point(
            draft,
            selector(IntentPortRole::Center, 0),
            Some(0),
            center,
            context,
        )?;
        draft = bind_point(
            draft,
            selector(IntentPortRole::MajorAxisPoint, 0),
            Some(1),
            major,
            context,
        )?;
    } else {
        // Axis-endpoint recipes allocate the stored major pole before their
        // derived centre. Preserve that native point order so construction
        // relation slots and cold materialization agree exactly.
        draft = bind_point(
            draft,
            selector(IntentPortRole::MajorAxisPoint, 0),
            Some(0),
            major,
            context,
        )?;
        draft = bind_point(
            draft,
            selector(IntentPortRole::Center, 0),
            None,
            center,
            context,
        )?;
    }
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 0),
        LeafField::Parameter,
        ratio,
        IntentUnit::Dimensionless,
    );
    register_single_curve(context, 1)?;
    Ok(draft)
}

#[allow(clippy::too_many_arguments)]
fn elliptical_arc_draft(
    mut draft: IntentNodeDraft,
    center: ConstructionPoint,
    major: ConstructionPoint,
    ratio: f64,
    start: f64,
    end: f64,
    sweep: DocumentArcSweep,
    center_authored: bool,
    context: &mut DraftContext<'_>,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    draft = ellipse_draft(draft, center, major, ratio, center_authored, context)?;
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 1),
        LeafField::Angle,
        start,
        IntentUnit::Angle,
    );
    draft = quantity_leaf(
        draft,
        selector(IntentPortRole::Target, 2),
        LeafField::Angle,
        end,
        IntentUnit::Angle,
    );
    enum_field(draft, "sweep", sweep_key(sweep))
}

fn relation_draft(
    context: &DraftContext<'_>,
    relation: &ConstructionRelationDefinition,
    symbol: IntentKey,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    let (kind, inputs, fields) = relation_parts(context, relation.relation)?;
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Constraint { constraint: kind }, symbol);
    for (slot, source) in inputs {
        draft = draft.with_input(slot, source);
    }
    for (name, value) in fields {
        draft = draft.with_field(IntentFieldKey(IntentKey::new(name)?), value);
    }
    Ok(draft)
}

type RelationParts = (
    ConstraintKind,
    Vec<(InputSlot, PatchPortRef)>,
    Vec<(String, IntentLiteral)>,
);

#[allow(clippy::too_many_lines)]
fn relation_parts(
    context: &DraftContext<'_>,
    relation: InferredRelation,
) -> Result<RelationParts, ProjectionalAuthoringError> {
    use ConstraintKind as C;
    use InferredRelation as R;
    let point = |slot| context.point_slot(slot);
    let span = |slot| context.span_slot(slot);
    let curve = |slot| context.curve_slot(slot);
    let one = |role, source| vec![(InputSlot::new(role, 0), source)];
    let pair = |role, first, second| {
        vec![
            (InputSlot::new(role, 0), first),
            (InputSlot::new(role, 1), second),
        ]
    };
    Ok(match relation {
        R::CoincidentWithOrigin { point: value } => (
            C::CoincidentWithOrigin,
            one(InputRole::Point, point(value)?),
            Vec::new(),
        ),
        R::PointOnDatumAxis { point: value, axis } => (
            C::PointOnDatumAxis,
            one(InputRole::Point, point(value)?),
            vec![("axis".into(), enum_literal(axis_key(axis))?)],
        ),
        R::PointOnCurve {
            point: value,
            contact,
        } => {
            let mut inputs = one(InputRole::Point, point(value)?);
            inputs.push((InputSlot::new(InputRole::Span, 0), span(contact.span)?));
            (
                C::PointOnCurve,
                inputs,
                contact_field_values("contact", contact, None)?,
            )
        }
        R::Midpoint { point: value, line } => {
            let mut inputs = one(InputRole::Point, point(value)?);
            inputs.push((InputSlot::new(InputRole::Span, 0), span(line)?));
            (C::Midpoint, inputs, Vec::new())
        }
        R::Horizontal { line } => (C::Horizontal, one(InputRole::Span, span(line)?), Vec::new()),
        R::Vertical { line } => (C::Vertical, one(InputRole::Span, span(line)?), Vec::new()),
        R::HorizontalPoints { first, second } => (
            C::HorizontalPoints,
            pair(InputRole::Point, point(first)?, point(second)?),
            Vec::new(),
        ),
        R::VerticalPoints { first, second } => (
            C::VerticalPoints,
            pair(InputRole::Point, point(first)?, point(second)?),
            Vec::new(),
        ),
        R::HorizontalPointToMidpoint { point: value, line } => {
            let mut inputs = one(InputRole::Point, point(value)?);
            inputs.push((InputSlot::new(InputRole::Span, 0), span(line)?));
            (C::HorizontalPointToMidpoint, inputs, Vec::new())
        }
        R::VerticalPointToMidpoint { point: value, line } => {
            let mut inputs = one(InputRole::Point, point(value)?);
            inputs.push((InputSlot::new(InputRole::Span, 0), span(line)?));
            (C::VerticalPointToMidpoint, inputs, Vec::new())
        }
        R::Concentric { first, second } => (
            C::Concentric,
            pair(InputRole::Curve, curve(first)?, curve(second)?),
            Vec::new(),
        ),
        R::Collinear { first, second } => (
            C::Collinear,
            pair(InputRole::Span, span(first.span)?, span(second.span)?),
            vec![
                (
                    "first_direction".into(),
                    enum_literal(direction_key(first.direction))?,
                ),
                (
                    "second_direction".into(),
                    enum_literal(direction_key(second.direction))?,
                ),
            ],
        ),
        R::Parallel { first, second } => (
            C::Parallel,
            pair(InputRole::Span, span(first)?, span(second)?),
            Vec::new(),
        ),
        R::Perpendicular { first, second } => (
            C::Perpendicular,
            pair(InputRole::Span, span(first)?, span(second)?),
            Vec::new(),
        ),
        R::EqualLength { first, second } => (
            C::EqualLength,
            pair(InputRole::Span, span(first)?, span(second)?),
            Vec::new(),
        ),
        R::CurveCurveTangency {
            first,
            second,
            orientation,
        } => {
            let mut fields = contact_field_values("first_contact", first, Some(orientation))?;
            fields.extend(contact_field_values(
                "second_contact",
                second,
                Some(orientation),
            )?);
            (
                C::CurveCurveTangency,
                pair(InputRole::Span, span(first.span)?, span(second.span)?),
                fields,
            )
        }
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed recipe table makes intrinsic-relation precedence auditable"
)]
fn authenticate_recipe_relations(
    variant: GeometryToolVariant,
    plan: &ConstructionCommitPlan,
    _context: &DraftContext<'_>,
) -> Result<(), ProjectionalAuthoringError> {
    use crate::ConstructionProposal as P;
    use GeometryToolVariant as V;
    use InferredRelation as R;

    let mut expected = match (variant, &plan.proposal) {
        (V::MidpointLine, P::MidpointLine { center, .. }) => vec![R::Midpoint {
            point: proposal_point_slot(std::slice::from_ref(center), 0)?,
            line: created_span_slot(0),
        }],
        (V::TangentArc, P::CircularArc { .. }) => {
            let intrinsic = plan
                .relations
                .iter()
                .filter(|relation| {
                    relation.provenance == ConstructionRelationProvenance::RecipeIntrinsic
                })
                .collect::<Vec<_>>();
            if intrinsic.len() != 1
                || !matches!(intrinsic[0].relation, R::CurveCurveTangency { .. })
            {
                return Err(ProjectionalAuthoringError::IntrinsicRelationMismatch);
            }
            Vec::new()
        }
        (V::TwoPointAlignedRectangle | V::CenterRectangle, P::RectangleLoop { .. }) => vec![
            R::Horizontal {
                line: created_span_slot(0),
            },
            R::Vertical {
                line: created_span_slot(1),
            },
            R::Horizontal {
                line: created_span_slot(2),
            },
            R::Vertical {
                line: created_span_slot(3),
            },
        ],
        (V::ThreePointCornerRectangle | V::ThreePointCenterRectangle, P::RectangleLoop { .. }) => {
            vec![
                R::Perpendicular {
                    first: created_span_slot(0),
                    second: created_span_slot(1),
                },
                R::Parallel {
                    first: created_span_slot(0),
                    second: created_span_slot(2),
                },
                R::Parallel {
                    first: created_span_slot(1),
                    second: created_span_slot(3),
                },
            ]
        }
        _ => Vec::new(),
    };
    if let (
        V::CenterRectangle | V::ThreePointCenterRectangle,
        P::RectangleLoop {
            points,
            center: Some(center),
            ..
        },
    ) = (variant, &plan.proposal)
    {
        expected.push(R::Midpoint {
            point: proposal_point_slot(points, *center)?,
            line: created_span_slot(4),
        });
    }
    let intrinsic = plan
        .relations
        .iter()
        .filter(|relation| relation.provenance == ConstructionRelationProvenance::RecipeIntrinsic)
        .map(|relation| relation.relation)
        .collect::<Vec<_>>();
    if variant != V::TangentArc && intrinsic != expected {
        return Err(ProjectionalAuthoringError::IntrinsicRelationMismatch);
    }
    let regularization = plan
        .relations
        .iter()
        .filter(|relation| {
            relation.provenance == ConstructionRelationProvenance::RecipeRegularization
        })
        .map(|relation| relation.relation)
        .collect::<Vec<_>>();
    let is_rectangle = matches!(
        variant,
        V::TwoPointAlignedRectangle
            | V::ThreePointCornerRectangle
            | V::CenterRectangle
            | V::ThreePointCenterRectangle
    );
    if regularization.is_empty()
        || is_rectangle
            && regularization.as_slice()
                == [R::EqualLength {
                    first: created_span_slot(0),
                    second: created_span_slot(1),
                }]
    {
        Ok(())
    } else {
        Err(ProjectionalAuthoringError::IntrinsicRelationMismatch)
    }
}

fn proposal_point_slot(
    points: &[ConstructionPoint],
    index: usize,
) -> Result<DraftPointSlot, ProjectionalAuthoringError> {
    let point = points
        .get(index)
        .ok_or(ProjectionalAuthoringError::RecipeProposalMismatch)?;
    Ok(match point {
        ConstructionPoint::Existing { id, .. } => DraftPointSlot::Existing(*id),
        ConstructionPoint::New(_) => DraftPointSlot::Created {
            point_index: points[..index]
                .iter()
                .filter(|point| matches!(point, ConstructionPoint::New(_)))
                .count(),
        },
    })
}

const fn created_span_slot(curve_index: usize) -> DraftSpanSlot {
    DraftSpanSlot::Created {
        curve_index,
        segment: 0,
    }
}

fn authenticate_curve_roles(
    variant: GeometryToolVariant,
    plan: &ConstructionCommitPlan,
    draft: &mut IntentNodeDraft,
) -> Result<(), ProjectionalAuthoringError> {
    let expected_count = match variant {
        GeometryToolVariant::SketchPoint => 0,
        GeometryToolVariant::TwoPointAlignedRectangle
        | GeometryToolVariant::ThreePointCornerRectangle => 4,
        GeometryToolVariant::CenterRectangle | GeometryToolVariant::ThreePointCenterRectangle => 5,
        _ => 1,
    };
    if plan.curve_roles.len() != expected_count {
        return Err(ProjectionalAuthoringError::CurveRoleMismatch);
    }
    let helper = matches!(
        variant,
        GeometryToolVariant::CenterRectangle | GeometryToolVariant::ThreePointCenterRectangle
    );
    let primary_roles = if helper {
        &plan.curve_roles[..plan.curve_roles.len().saturating_sub(1)]
    } else {
        plan.curve_roles.as_slice()
    };
    let role = primary_roles.first().copied();
    if primary_roles
        .iter()
        .copied()
        .any(|candidate| Some(candidate) != role)
        || helper
            && plan.curve_roles.last().copied() != Some(geosolve_sketch::GeometryRole::Construction)
    {
        return Err(ProjectionalAuthoringError::CurveRoleMismatch);
    }
    if let Some(role) = role {
        *draft = enum_field(
            draft.clone(),
            "role",
            match role {
                geosolve_sketch::GeometryRole::Profile => "profile",
                geosolve_sketch::GeometryRole::Construction => "construction",
            },
        )?;
    }
    Ok(())
}

fn register_single_curve(
    context: &mut DraftContext<'_>,
    span_count: usize,
) -> Result<(), ProjectionalAuthoringError> {
    register_curve(context, 0, span_count)
}

fn register_curve(
    context: &mut DraftContext<'_>,
    curve_index: usize,
    span_count: usize,
) -> Result<(), ProjectionalAuthoringError> {
    let curve_selector = selector(IntentPortRole::Curve, u16_index(curve_index)?);
    context.created.curves.push(context.alias(curve_selector));
    for segment in 0..span_count {
        let span_selector = selector(
            IntentPortRole::Span,
            u16_index(curve_index.saturating_add(segment))?,
        );
        context.created.spans.insert(
            (
                curve_index,
                u32::try_from(segment)
                    .map_err(|_| ProjectionalAuthoringError::InvalidCreatedSlot)?,
            ),
            context.alias(span_selector),
        );
    }
    Ok(())
}

fn register_polyline_curve(
    context: &mut DraftContext<'_>,
    span_count: usize,
) -> Result<(), ProjectionalAuthoringError> {
    context
        .created
        .curves
        .push(context.alias(selector(IntentPortRole::Curve, 0)));
    for segment in 0..span_count {
        context.created.spans.insert(
            (
                0,
                u32::try_from(segment)
                    .map_err(|_| ProjectionalAuthoringError::InvalidCreatedSlot)?,
            ),
            context.alias(child_selector(u16_index(segment)?, IntentPortRole::Span)),
        );
    }
    Ok(())
}

fn register_nurbs_curve(
    context: &mut DraftContext<'_>,
    span_count: usize,
) -> Result<(), ProjectionalAuthoringError> {
    context
        .created
        .curves
        .push(context.alias(selector(IntentPortRole::Curve, 0)));
    for index in 0..span_count {
        context.created.spans.insert(
            (
                0,
                u32::try_from(index).map_err(|_| ProjectionalAuthoringError::InvalidCreatedSlot)?,
            ),
            context.alias(selector(IntentPortRole::Span, u16_index(index)?)),
        );
    }
    Ok(())
}

fn point_leaves(
    draft: IntentNodeDraft,
    selector: IntentPortSelector,
    position: [f64; 2],
) -> IntentNodeDraft {
    quantity_leaf(
        quantity_leaf(
            draft,
            selector,
            LeafField::X,
            position[0],
            IntentUnit::Length,
        ),
        selector,
        LeafField::Y,
        position[1],
        IntentUnit::Length,
    )
}

fn quantity_leaf(
    draft: IntentNodeDraft,
    selector: IntentPortSelector,
    field: LeafField,
    value: f64,
    unit: IntentUnit,
) -> IntentNodeDraft {
    draft.with_instance_leaf(selector, field, IntentLiteral::Quantity { value, unit })
}

fn parameter_leaf(draft: IntentNodeDraft, index: u16, value: f64) -> IntentNodeDraft {
    quantity_leaf(
        draft,
        selector(IntentPortRole::Target, index),
        LeafField::Parameter,
        value,
        IntentUnit::Dimensionless,
    )
}

fn point_field(
    draft: IntentNodeDraft,
    name: &str,
    value: [f64; 2],
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    Ok(draft.with_field(
        IntentFieldKey(IntentKey::new(name)?),
        IntentLiteral::Point(value),
    ))
}

fn enum_field(
    draft: IntentNodeDraft,
    name: &str,
    value: &str,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    Ok(draft.with_field(IntentFieldKey(IntentKey::new(name)?), enum_literal(value)?))
}

fn bool_field(
    draft: IntentNodeDraft,
    name: &str,
    value: bool,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    Ok(draft.with_field(
        IntentFieldKey(IntentKey::new(name)?),
        IntentLiteral::Boolean(value),
    ))
}

fn natural_field(
    draft: IntentNodeDraft,
    name: &str,
    value: u64,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    Ok(draft.with_field(
        IntentFieldKey(IntentKey::new(name)?),
        IntentLiteral::Natural(value),
    ))
}

fn enum_literal(value: &str) -> Result<IntentLiteral, ProjectionalAuthoringError> {
    Ok(IntentLiteral::Enum(IntentKey::new(value)?))
}

fn contact_fields_without_orientation(
    mut draft: IntentNodeDraft,
    prefix: &str,
    contact: DraftContactDescriptor,
) -> Result<IntentNodeDraft, ProjectionalAuthoringError> {
    let orientation_field = format!("{prefix}_orientation");
    for (name, value) in contact_field_values(prefix, contact, None)? {
        if name == orientation_field {
            continue;
        }
        draft = draft.with_field(IntentFieldKey(IntentKey::new(name)?), value);
    }
    Ok(draft)
}

fn contact_field_values(
    prefix: &str,
    contact: DraftContactDescriptor,
    orientation: Option<TangentOrientation>,
) -> Result<Vec<(String, IntentLiteral)>, ProjectionalAuthoringError> {
    let mut values = vec![
        (
            format!("{prefix}_parameter"),
            IntentLiteral::Quantity {
                value: contact.parameter,
                unit: IntentUnit::Dimensionless,
            },
        ),
        (
            format!("{prefix}_winding"),
            IntentLiteral::Integer(i64::from(contact.winding)),
        ),
    ];
    match contact.domain {
        ContactDomain::SupportingLine => {
            values.push((format!("{prefix}_domain"), enum_literal("supporting_line")?));
        }
        ContactDomain::Bounded { lower, upper } => {
            values.push((format!("{prefix}_domain"), enum_literal("bounded")?));
            values.push((format!("{prefix}_domain_lower"), dimensionless(lower)));
            values.push((format!("{prefix}_domain_upper"), dimensionless(upper)));
        }
        ContactDomain::Periodic { period } => {
            values.push((format!("{prefix}_domain"), enum_literal("periodic")?));
            values.push((format!("{prefix}_domain_period"), dimensionless(period)));
        }
    }
    match contact.neighborhood {
        ContactNeighborhood::Interior => {
            values.push((format!("{prefix}_neighborhood"), enum_literal("interior")?));
        }
        ContactNeighborhood::Start => {
            values.push((format!("{prefix}_neighborhood"), enum_literal("start")?));
        }
        ContactNeighborhood::End => {
            values.push((format!("{prefix}_neighborhood"), enum_literal("end")?));
        }
        ContactNeighborhood::Local { lower, upper } => {
            values.push((format!("{prefix}_neighborhood"), enum_literal("local")?));
            values.push((format!("{prefix}_neighborhood_lower"), dimensionless(lower)));
            values.push((format!("{prefix}_neighborhood_upper"), dimensionless(upper)));
        }
    }
    values.push((
        format!("{prefix}_orientation"),
        enum_literal(orientation.map_or("none", orientation_key))?,
    ));
    Ok(values)
}

fn dimensionless(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Dimensionless,
    }
}

fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

fn child_selector(ordinal: u16, role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::InitialChild {
        ordinal,
        role,
        index: 0,
    }
}

fn point_position(point: ConstructionPoint) -> [f64; 2] {
    match point {
        ConstructionPoint::Existing { position, .. } | ConstructionPoint::New(position) => position,
    }
}

fn unit_direction(
    start: ConstructionPoint,
    end: ConstructionPoint,
) -> Result<[f64; 2], ProjectionalAuthoringError> {
    let start = point_position(start);
    let end = point_position(end);
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    if !(length.is_finite() && length > 0.0) {
        return Err(ProjectionalAuthoringError::InvalidGeometry);
    }
    Ok([delta[0] / length, delta[1] / length])
}

fn finite_midpoint(
    first: [f64; 2],
    second: [f64; 2],
) -> Result<[f64; 2], ProjectionalAuthoringError> {
    let midpoint = [
        first[0] + 0.5 * (second[0] - first[0]),
        first[1] + 0.5 * (second[1] - first[1]),
    ];
    midpoint
        .into_iter()
        .all(f64::is_finite)
        .then_some(midpoint)
        .ok_or(ProjectionalAuthoringError::InvalidGeometry)
}

fn distance(first: [f64; 2], second: [f64; 2]) -> Result<f64, ProjectionalAuthoringError> {
    let value = (second[0] - first[0]).hypot(second[1] - first[1]);
    (value.is_finite() && value > 0.0)
        .then_some(value)
        .ok_or(ProjectionalAuthoringError::InvalidGeometry)
}

fn angle_from(first: [f64; 2], second: [f64; 2]) -> Result<f64, ProjectionalAuthoringError> {
    let value = (second[1] - first[1]).atan2(second[0] - first[0]);
    value
        .is_finite()
        .then_some(value)
        .ok_or(ProjectionalAuthoringError::InvalidGeometry)
}

fn nurbs_span_count(
    form: DocumentBSplineForm,
    degree: u32,
    controls: usize,
) -> Result<usize, ProjectionalAuthoringError> {
    match form {
        DocumentBSplineForm::Clamped => controls
            .checked_sub(
                usize::try_from(degree).map_err(|_| ProjectionalAuthoringError::InvalidGeometry)?,
            )
            .filter(|count| *count > 0),
        DocumentBSplineForm::Periodic => (controls > 0).then_some(controls),
    }
    .ok_or(ProjectionalAuthoringError::InvalidGeometry)
}

fn u16_index(value: usize) -> Result<u16, ProjectionalAuthoringError> {
    u16::try_from(value).map_err(|_| ProjectionalAuthoringError::InvalidCreatedSlot)
}

const fn sweep_key(value: DocumentArcSweep) -> &'static str {
    match value {
        DocumentArcSweep::CounterClockwise => "counter_clockwise",
        DocumentArcSweep::Clockwise => "clockwise",
    }
}

const fn orientation_key(value: TangentOrientation) -> &'static str {
    match value {
        TangentOrientation::Aligned => "aligned",
        TangentOrientation::Opposed => "opposed",
    }
}

const fn axis_key(value: DocumentCoordinateAxis) -> &'static str {
    match value {
        DocumentCoordinateAxis::X => "x",
        DocumentCoordinateAxis::Y => "y",
    }
}

const fn direction_key(value: DocumentDirectionSense) -> &'static str {
    match value {
        DocumentDirectionSense::Forward => "forward",
        DocumentDirectionSense::Reverse => "reverse",
    }
}

// Keep native operand kinds explicit in this module's public contract.
const _: fn(CurveId) -> IntentNativeBinding = IntentNativeBinding::Curve;
const _: fn(CurveSpan) -> IntentNativeBinding = IntentNativeBinding::CurveSpan;
const _: fn(DesignPointId) -> IntentNativeBinding = IntentNativeBinding::Point;
