// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed canvas-authoring patches for the projectional intent session.
//!
//! This module translates the editor's already-resolved, equation-free M78
//! construction recipes into the closed intent vocabulary.  It does not apply
//! geometry directly and does not reproduce solver equations.

use std::collections::BTreeMap;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveId, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentBSplineForm, DocumentCoordinateAxis, DocumentDirectionSense, TangentOrientation,
};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentIdentityFlow,
    IntentKey, IntentKeyError, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSession,
    IntentSessionIdentity, IntentUnit, LeafField, PatchPortRef,
};
use thiserror::Error;

use crate::{
    ConstructionCommitPlan, ConstructionPoint, ConstructionRelationDefinition,
    ConstructionRelationProvenance, DraftContactDescriptor, DraftCurveSlot, DraftPointSlot,
    DraftSpanSlot, GeometryToolVariant, InferredRelation, IntentMaterializationMap,
    IntentNativeBinding, MAX_CONSTRUCTION_PLAN_RELATIONS,
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
        let mut candidates = self
            .ownership
            .ports
            .iter()
            .filter_map(|(port, candidate)| (*candidate == binding).then_some(*port))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|port| {
            let flow_rank = self
                .intent
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
