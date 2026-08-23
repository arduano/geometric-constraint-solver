// SPDX-License-Identifier: GPL-3.0-or-later

//! Browser-ready typed patches for computed Fillet intent.

use geosolve_sketch::{
    ContactNeighborhood, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, PreparedSketchInput,
    SketchAcceptedStateIdentity,
};
use geosolve_sketch_features::{
    ComputedFeatureId, ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, InputRole, InputSlot, IntentFieldKey, IntentKey, IntentKeyError,
    IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortKind, IntentSession, IntentSessionIdentity, IntentUnit,
    PatchPortRef,
};
use thiserror::Error;

use crate::{FeatureAuthoringCandidate, IntentMaterializationMap, IntentNativeBinding};

/// One complete computed-Fillet declaration patch.
#[derive(Clone, Debug)]
pub struct ProjectionalFilletPatch {
    pub patch: IntentPatch,
    pub declaration_alias: IntentKey,
}

/// Fail-closed translation error for projectional computed-Fillet authoring.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ProjectionalFilletAuthoringError {
    #[error(transparent)]
    Key(#[from] IntentKeyError),
    #[error("the computed-Fillet patch expected identity is stale")]
    StaleIntentIdentity,
    #[error("the computed-Fillet ownership map is stale")]
    StaleOwnership,
    #[error("the computed-Fillet preview belongs to a different accepted sketch input")]
    StaleCandidate,
    #[error("the computed-Fillet candidate has no corners")]
    EmptyCandidate,
    #[error("the computed-Fillet candidate exceeds intent-v1 cardinality")]
    CardinalityExceeded,
    #[error("a computed-Fillet native span has no unique logical span owner")]
    AmbiguousSpanOwnership,
    #[error("computed feature {0:?} has no unique logical declaration owner")]
    AmbiguousFeatureOwnership(ComputedFeatureId),
    #[error("computed feature {0:?} is not a FilletSet declaration")]
    WrongFeatureKind(ComputedFeatureId),
    #[error("the computed-Fillet radius must be finite and positive")]
    InvalidRadius,
}

/// Translates one authenticated grouped Fillet preview into a single typed
/// declaration. Every branch field is copied; no branch or contact default is
/// inferred by this adapter.
pub fn projectional_fillet_patch(
    expected: IntentSessionIdentity,
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    accepted_input: PreparedSketchInput,
    accepted_state: SketchAcceptedStateIdentity,
    symbol: IntentKey,
    candidate: &FeatureAuthoringCandidate,
) -> Result<ProjectionalFilletPatch, ProjectionalFilletAuthoringError> {
    if expected != intent.identity() {
        return Err(ProjectionalFilletAuthoringError::StaleIntentIdentity);
    }
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalFilletAuthoringError::StaleOwnership);
    }
    if candidate.sketch_input() != accepted_input
        || candidate.accepted_state_identity() != accepted_state
    {
        return Err(ProjectionalFilletAuthoringError::StaleCandidate);
    }
    if !candidate.radius().is_finite() || candidate.radius() <= 0.0 {
        return Err(ProjectionalFilletAuthoringError::InvalidRadius);
    }
    let corners = candidate.persistent_corners();
    if corners.is_empty() {
        return Err(ProjectionalFilletAuthoringError::EmptyCandidate);
    }
    let child_count = u16::try_from(corners.len())
        .map_err(|_| ProjectionalFilletAuthoringError::CardinalityExceeded)?;
    let alias = IntentKey::new(format!(
        "computed-fillet-{:016x}",
        expected.revision.raw().saturating_add(1)
    ))?;
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        },
        symbol,
    )
    .with_dynamic_children(child_count)
    .with_field(
        key("radius")?,
        quantity(candidate.radius(), IntentUnit::Length),
    );
    for (ordinal, corner) in corners.iter().copied().enumerate() {
        let first_index = u16::try_from(ordinal.saturating_mul(2))
            .map_err(|_| ProjectionalFilletAuthoringError::CardinalityExceeded)?;
        let second_index = first_index
            .checked_add(1)
            .ok_or(ProjectionalFilletAuthoringError::CardinalityExceeded)?;
        draft = draft
            .with_input(
                InputSlot::new(InputRole::Span, first_index),
                exact_span_owner(ownership, corner.first.source)?,
            )
            .with_input(
                InputSlot::new(InputRole::Span, second_index),
                exact_span_owner(ownership, corner.second.source)?,
            );
        draft = with_corner_fields(draft, ordinal, corner)?;
    }
    Ok(ProjectionalFilletPatch {
        patch: IntentPatch::new(
            expected,
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ),
        declaration_alias: alias,
    })
}

/// Builds a retained-invalid-capable radius edit for one stably owned
/// projectional Fillet declaration.
pub fn projectional_fillet_radius_patch(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    feature: ComputedFeatureId,
    radius: f64,
) -> Result<IntentPatch, ProjectionalFilletAuthoringError> {
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalFilletAuthoringError::StaleOwnership);
    }
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ProjectionalFilletAuthoringError::InvalidRadius);
    }
    let owners = ownership
        .nodes
        .iter()
        .filter(|node| {
            node.owned
                .contains(&IntentNativeBinding::ComputedFeature(feature))
        })
        .map(|node| node.node)
        .collect::<Vec<_>>();
    let [node] = owners.as_slice() else {
        return Err(ProjectionalFilletAuthoringError::AmbiguousFeatureOwnership(
            feature,
        ));
    };
    if !matches!(
        intent.graph().node(*node).map(|node| &node.kind),
        Some(IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet
        })
    ) {
        return Err(ProjectionalFilletAuthoringError::WrongFeatureKind(feature));
    }
    Ok(IntentPatch::new(
        intent.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::SetDefinitionField {
            node: *node,
            field: key("radius")?,
            value: quantity(radius, IntentUnit::Length),
        }],
    ))
}

fn exact_span_owner(
    ownership: &IntentMaterializationMap,
    source: NativeCurveSpanSource,
) -> Result<PatchPortRef, ProjectionalFilletAuthoringError> {
    let owners = ownership
        .ports
        .iter()
        .filter(|(port, binding)| {
            port.kind == IntentPortKind::CurveSpan
                && **binding == IntentNativeBinding::CurveSpan(source.span)
        })
        .map(|(port, _)| *port)
        .collect::<Vec<_>>();
    let [port] = owners.as_slice() else {
        return Err(ProjectionalFilletAuthoringError::AmbiguousSpanOwnership);
    };
    Ok(PatchPortRef::Stable { port: *port })
}

fn with_corner_fields(
    mut draft: IntentNodeDraft,
    ordinal: usize,
    corner: NewComputedFilletCorner,
) -> Result<IntentNodeDraft, IntentKeyError> {
    draft = with_parent_fields(draft, ordinal, "first", corner.first)?;
    draft = with_parent_fields(draft, ordinal, "second", corner.second)?;
    draft = draft
        .with_field(
            key(&format!("corner_{ordinal:04}_endpoint_order"))?,
            enum_literal(match corner.endpoint_order {
                DocumentFilletEndpointOrder::FirstThenSecond => "first_then_second",
                DocumentFilletEndpointOrder::SecondThenFirst => "second_then_first",
            })?,
        )
        .with_field(
            key(&format!("corner_{ordinal:04}_sweep"))?,
            enum_literal(match corner.sweep {
                DocumentArcSweep::CounterClockwise => "counter_clockwise",
                DocumentArcSweep::Clockwise => "clockwise",
            })?,
        );
    Ok(draft)
}

fn with_parent_fields(
    mut draft: IntentNodeDraft,
    ordinal: usize,
    parent_name: &str,
    parent: ComputedFilletParent,
) -> Result<IntentNodeDraft, IntentKeyError> {
    let prefix = format!("corner_{ordinal:04}_{parent_name}");
    draft = draft
        .with_field(
            key(&format!("{prefix}_parameter"))?,
            quantity(parent.picked_parameter, IntentUnit::Dimensionless),
        )
        .with_field(
            key(&format!("{prefix}_winding"))?,
            IntentLiteral::Integer(i64::from(parent.winding)),
        )
        .with_field(
            key(&format!("{prefix}_normal_side"))?,
            enum_literal(match parent.normal_side {
                DocumentCurveNormalSide::Left => "left",
                DocumentCurveNormalSide::Right => "right",
            })?,
        )
        .with_field(
            key(&format!("{prefix}_trim_endpoint"))?,
            enum_literal(match parent.retained_endpoint {
                DocumentFilletTrimEndpoint::Start => "start",
                DocumentFilletTrimEndpoint::End => "end",
            })?,
        );
    draft = match parent.neighborhood {
        ContactNeighborhood::Interior => draft.with_field(
            key(&format!("{prefix}_neighborhood"))?,
            enum_literal("interior")?,
        ),
        ContactNeighborhood::Local { lower, upper } => draft
            .with_field(
                key(&format!("{prefix}_neighborhood"))?,
                enum_literal("local")?,
            )
            .with_field(
                key(&format!("{prefix}_local_lower"))?,
                quantity(lower, IntentUnit::Dimensionless),
            )
            .with_field(
                key(&format!("{prefix}_local_upper"))?,
                quantity(upper, IntentUnit::Dimensionless),
            ),
        ContactNeighborhood::Start => draft.with_field(
            key(&format!("{prefix}_neighborhood"))?,
            enum_literal("start")?,
        ),
        ContactNeighborhood::End => draft.with_field(
            key(&format!("{prefix}_neighborhood"))?,
            enum_literal("end")?,
        ),
    };
    draft = if let Some(anchor) = parent.periodic_anchor {
        draft
            .with_field(
                key(&format!("{prefix}_periodic_anchor"))?,
                IntentLiteral::Boolean(true),
            )
            .with_field(
                key(&format!("{prefix}_anchor_parameter"))?,
                quantity(anchor.parameter, IntentUnit::Dimensionless),
            )
            .with_field(
                key(&format!("{prefix}_anchor_winding"))?,
                IntentLiteral::Integer(i64::from(anchor.winding)),
            )
    } else {
        draft.with_field(
            key(&format!("{prefix}_periodic_anchor"))?,
            IntentLiteral::Boolean(false),
        )
    };
    Ok(draft)
}

fn key(value: &str) -> Result<IntentFieldKey, IntentKeyError> {
    Ok(IntentFieldKey(IntentKey::new(value)?))
}

fn enum_literal(value: &str) -> Result<IntentLiteral, IntentKeyError> {
    Ok(IntentLiteral::Enum(IntentKey::new(value)?))
}

const fn quantity(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
}

