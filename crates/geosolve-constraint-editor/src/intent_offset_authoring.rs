// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed projectional patches for native topology-preserving Profile Offset.
//!
//! This adapter consumes the existing topology-authenticated Offset collector
//! and the exact native operation output plan. It owns no offset equations,
//! curve construction, or topology policy.

use geosolve_sketch::{
    DocumentDimensionId, DocumentFaceOffsetDirection, DocumentLineSide, OperationControl,
    OperationOutcome, RetainedSketchDocumentSession, SketchMaterializationIdentityKind,
};
use geosolve_sketch_intent::{
    AggregateKind, DeletePolicy, InputRole, InputSlot, IntentFieldKey, IntentIdentityFlow,
    IntentKey, IntentKeyError, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentOperationOutput, IntentOperationOutputKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSession, IntentSessionIdentity,
    IntentUnit, NodeId, OperationKind, PatchPortRef,
};
use geosolve_sketch_ops::{
    SketchOperationKind, SketchOperationOutputPlan, SketchOperationRequest, SketchOperationResult,
    SketchOperationSnapshot, SketchProfileOffsetOperand,
};
use geosolve_sketch_topology::{OffsetDirectedSpan, OffsetTraversal};
use thiserror::Error;

use crate::{
    IntentMaterializationMap, IntentNativeBinding, OffsetAuthoringCandidate,
    OffsetAuthoringOperand, OffsetAuthoringState, ProfileOffsetDirectionState,
};

/// One complete native Profile Offset intent transaction.
#[derive(Clone, Debug)]
pub struct ProjectionalProfileOffsetPatch {
    /// One exact-CAS patch containing every operand aggregate and the operation.
    pub patch: IntentPatch,
    /// Alias of the user-facing Profile Offset operation declaration.
    pub declaration_alias: IntentKey,
    /// Aliases of the equation-free closed-profile or open-chain operands.
    pub aggregate_aliases: Vec<IntentKey>,
}

/// Fail-closed translation/property error for projectional Profile Offset.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ProjectionalProfileOffsetError {
    #[error(transparent)]
    Key(#[from] IntentKeyError),
    #[error("the Profile Offset patch expected identity is stale")]
    StaleIntentIdentity,
    #[error("the Profile Offset ownership map is stale")]
    StaleOwnership,
    #[error("the Profile Offset candidate or topology index is stale")]
    StaleCandidate,
    #[error("the Profile Offset collector has no complete finite candidate")]
    IncompleteCandidate,
    #[error("native Profile Offset preparation stopped before producing an output plan")]
    PreparationStopped,
    #[error("native Profile Offset preparation rejected: {0}")]
    PreparationRejected(String),
    #[error("the authenticated native output plan is not a Profile Offset plan")]
    WrongOutputPlan,
    #[error("the authenticated native output plan contains an unsupported output kind")]
    UnsupportedOutputKind,
    #[error("the Profile Offset operand exceeds the bounded intent cardinality")]
    CardinalityExceeded,
    #[error("a native Profile Offset source span has no stable logical owner")]
    UnownedSourceSpan,
    #[error("native Profile Offset dimension {0:?} has no unique operation owner")]
    AmbiguousDimensionOwnership(DocumentDimensionId),
    #[error("the resolved declaration is not a native Profile Offset operation")]
    WrongDeclarationKind,
    #[error("the Profile Offset distance must be finite and positive")]
    InvalidDistance,
    #[error("the requested Profile Offset direction belongs to the other operand family")]
    DirectionFamilyMismatch,
}

/// Prepares the existing native operation against the exact accepted input and
/// translates its authenticated output plan into one unordered intent patch.
///
/// The returned patch creates equation-free aggregate declarations for the
/// selected face loops or ordered open chain, then one Profile Offset operation
/// which owns every generated native identity. The patch is one history action.
///
/// # Errors
///
/// Rejects stale intent/native input, incomplete collection, topology failure,
/// missing logical source ownership, or a non-Profile-Offset output plan.
pub fn projectional_profile_offset_patch(
    expected: IntentSessionIdentity,
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    accepted: &RetainedSketchDocumentSession,
    state: &OffsetAuthoringState,
    symbol: IntentKey,
) -> Result<ProjectionalProfileOffsetPatch, ProjectionalProfileOffsetError> {
    if expected != intent.identity() {
        return Err(ProjectionalProfileOffsetError::StaleIntentIdentity);
    }
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalProfileOffsetError::StaleOwnership);
    }
    let candidate = state
        .candidate()
        .ok_or(ProjectionalProfileOffsetError::IncompleteCandidate)?;
    let index = state
        .index()
        .cloned()
        .ok_or(ProjectionalProfileOffsetError::IncompleteCandidate)?;
    if candidate.input != accepted.prepared_input() || index.validate_current(accepted).is_err() {
        return Err(ProjectionalProfileOffsetError::StaleCandidate);
    }
    let request = SketchOperationRequest::ProfileOffset {
        label: symbol.as_str().to_owned(),
        distance: candidate.distance,
        operand: native_operand(&candidate.operand),
        operand_index: index,
    };
    let outcome = SketchOperationSnapshot::capture(accepted)
        .prepare(request)
        .execute(OperationControl::unlimited())
        .map_err(|error| ProjectionalProfileOffsetError::PreparationRejected(error.to_string()))?;
    let OperationOutcome::Completed { value, .. } = outcome else {
        return Err(ProjectionalProfileOffsetError::PreparationStopped);
    };
    let proposal = match value {
        SketchOperationResult::Proposed(proposal) => proposal,
        SketchOperationResult::Unsupported(reason) => {
            return Err(ProjectionalProfileOffsetError::PreparationRejected(
                format!("{:?}", reason.reason),
            ));
        }
        SketchOperationResult::Incomplete(reason) => {
            return Err(ProjectionalProfileOffsetError::PreparationRejected(
                format!("{:?}", reason.reason),
            ));
        }
        _ => {
            return Err(ProjectionalProfileOffsetError::PreparationRejected(
                "native operation returned an unknown outcome".into(),
            ));
        }
    };
    if proposal.input() != candidate.input {
        return Err(ProjectionalProfileOffsetError::StaleCandidate);
    }
    patch_from_output_plan(
        expected,
        intent,
        ownership,
        symbol,
        &candidate,
        proposal.output_plan(),
    )
}

/// Builds a retained-invalid-capable distance property edit for one accepted
/// native Profile Offset dimension.
///
/// # Errors
///
/// Rejects a non-finite/non-positive distance, stale ownership, ambiguous
/// dimension ownership, or a dimension not owned by Profile Offset.
pub fn projectional_profile_offset_distance_patch(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    dimension: DocumentDimensionId,
    distance: f64,
) -> Result<IntentPatch, ProjectionalProfileOffsetError> {
    if !distance.is_finite() || distance <= 0.0 {
        return Err(ProjectionalProfileOffsetError::InvalidDistance);
    }
    let node = profile_offset_owner(intent, ownership, dimension)?;
    Ok(IntentPatch::new(
        intent.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::SetDefinitionField {
            node,
            field: field_key("distance")?,
            value: quantity(distance, IntentUnit::Length),
        }],
    ))
}

/// Builds a retained-invalid-capable explicit direction property edit for one
/// accepted native Profile Offset dimension.
///
/// # Errors
///
/// Rejects stale or ambiguous ownership, a dimension not owned by Profile
/// Offset, or a face/open-chain direction-family mismatch.
pub fn projectional_profile_offset_direction_patch(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    dimension: DocumentDimensionId,
    direction: ProfileOffsetDirectionState,
) -> Result<IntentPatch, ProjectionalProfileOffsetError> {
    let node = profile_offset_owner(intent, ownership, dimension)?;
    let declaration = intent
        .graph()
        .node(node)
        .ok_or(ProjectionalProfileOffsetError::WrongDeclarationKind)?;
    let direction_key = field_key("direction")?;
    let side_key = field_key("side")?;
    let (field, value) = match (
        declaration.fields.get(&direction_key),
        declaration.fields.get(&side_key),
        direction,
    ) {
        (Some(IntentLiteral::Enum(_)), None, ProfileOffsetDirectionState::Outward) => {
            (direction_key, "outward")
        }
        (Some(IntentLiteral::Enum(_)), None, ProfileOffsetDirectionState::Inward) => {
            (direction_key, "inward")
        }
        (None, Some(IntentLiteral::Enum(_)), ProfileOffsetDirectionState::Left) => {
            (side_key, "left")
        }
        (None, Some(IntentLiteral::Enum(_)), ProfileOffsetDirectionState::Right) => {
            (side_key, "right")
        }
        _ => return Err(ProjectionalProfileOffsetError::DirectionFamilyMismatch),
    };
    Ok(IntentPatch::new(
        intent.identity(),
        IntentPatchPolicy::RetainFailedIntent,
        vec![IntentPatchOperation::SetDefinitionField {
            node,
            field,
            value: IntentLiteral::Enum(IntentKey::new(value)?),
        }],
    ))
}

/// Builds one exact dependent-closure deletion for the operation which owns an
/// accepted native Profile Offset dimension.
///
/// # Errors
///
/// Rejects stale or ambiguous ownership, a dimension not owned by Profile
/// Offset, or failure to compute the current exact dependent closure.
pub fn projectional_profile_offset_delete_patch(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    dimension: DocumentDimensionId,
) -> Result<IntentPatch, ProjectionalProfileOffsetError> {
    let node = profile_offset_owner(intent, ownership, dimension)?;
    projectional_profile_offset_delete_node_patch(intent, node)
}

/// Builds one exact closure deletion from the stable Profile Offset
/// declaration itself.
///
/// Unlike [`projectional_profile_offset_delete_patch`], this route does not
/// require the operation to occur in the last accepted native ownership map.
/// That distinction lets retained-invalid explicit intent remove its private
/// Profile/OpenChain helper in the same transaction instead of falling back to
/// a generic downstream-only declaration delete.
///
/// # Errors
///
/// Rejects a missing or non-Offset declaration and any changed/invalid
/// dependency closure before returning a patch.
pub fn projectional_profile_offset_delete_node_patch(
    intent: &IntentSession,
    node: NodeId,
) -> Result<IntentPatch, ProjectionalProfileOffsetError> {
    let declaration = intent
        .graph()
        .node(node)
        .ok_or(ProjectionalProfileOffsetError::WrongDeclarationKind)?;
    if !matches!(
        declaration.kind,
        IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset
        }
    ) {
        return Err(ProjectionalProfileOffsetError::WrongDeclarationKind);
    }
    let operation_closure = intent
        .graph()
        .dependent_closure([node])
        .map_err(|error| ProjectionalProfileOffsetError::PreparationRejected(error.to_string()))?;
    let mut roots = std::collections::BTreeSet::from([node]);
    for (slot, source) in &declaration.inputs {
        let owned_kind = match slot.role {
            InputRole::Profile => Some(AggregateKind::ClosedProfile),
            InputRole::Chain => Some(AggregateKind::OpenChain),
            _ => None,
        };
        let Some(owned_kind) = owned_kind else {
            continue;
        };
        if !matches!(
            intent.graph().node(source.node).map(|source| &source.kind),
            Some(IntentNodeKind::Aggregate { aggregate }) if *aggregate == owned_kind
        ) {
            continue;
        }
        let aggregate_closure =
            intent
                .graph()
                .dependent_closure([source.node])
                .map_err(|error| {
                    ProjectionalProfileOffsetError::PreparationRejected(error.to_string())
                })?;
        let mut exclusively_owned = operation_closure.clone();
        exclusively_owned.insert(source.node);
        if aggregate_closure == exclusively_owned {
            // Authoring-created Profile/Chain helpers have no consumer outside
            // this operation's exact downstream closure. A shared aggregate is
            // a reusable semantic declaration and must survive deleting only
            // this Offset. Names and source order never participate.
            roots.insert(source.node);
        }
    }
    let closure = intent
        .graph()
        .dependent_closure(roots.clone())
        .map_err(|error| ProjectionalProfileOffsetError::PreparationRejected(error.to_string()))?;
    let policy = if roots.len() == 1 && closure.len() == 1 {
        DeletePolicy::RejectDependents
    } else if roots.len() == 1 {
        DeletePolicy::Cascade {
            exact_nodes: closure,
        }
    } else {
        DeletePolicy::CascadeRoots {
            exact_roots: roots,
            exact_nodes: closure,
        }
    };
    Ok(IntentPatch::new(
        intent.identity(),
        IntentPatchPolicy::RequireAccepted,
        vec![IntentPatchOperation::DeleteNode { node, policy }],
    ))
}

fn patch_from_output_plan(
    expected: IntentSessionIdentity,
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    symbol: IntentKey,
    candidate: &OffsetAuthoringCandidate,
    output_plan: &SketchOperationOutputPlan,
) -> Result<ProjectionalProfileOffsetPatch, ProjectionalProfileOffsetError> {
    if output_plan.kind != SketchOperationKind::ProfileOffset {
        return Err(ProjectionalProfileOffsetError::WrongOutputPlan);
    }
    let revision = expected.revision.raw().saturating_add(1);
    let declaration_alias = IntentKey::new(format!("profile-offset-{revision:016x}"))?;
    let mut aggregate_aliases = Vec::new();
    let mut operations = Vec::new();
    let mut operation = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset,
        },
        symbol,
    )
    .with_field(
        field_key("distance")?,
        quantity(candidate.distance, IntentUnit::Length),
    )
    .with_operation_outputs(intent_outputs(output_plan)?);

    append_operand(
        intent,
        ownership,
        revision,
        candidate,
        &mut operation,
        &mut operations,
        &mut aggregate_aliases,
    )?;
    operations.push(IntentPatchOperation::CreateNode {
        alias: declaration_alias.clone(),
        draft: Box::new(operation),
        cell: None,
    });
    Ok(ProjectionalProfileOffsetPatch {
        patch: IntentPatch::new(expected, IntentPatchPolicy::RequireAccepted, operations),
        declaration_alias,
        aggregate_aliases,
    })
}

fn append_operand(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    revision: u64,
    candidate: &OffsetAuthoringCandidate,
    operation: &mut IntentNodeDraft,
    operations: &mut Vec<IntentPatchOperation>,
    aggregate_aliases: &mut Vec<IntentKey>,
) -> Result<(), ProjectionalProfileOffsetError> {
    match &candidate.operand {
        OffsetAuthoringOperand::Face { key, direction } => {
            let loops = std::iter::once(&key.outer.spans)
                .chain(key.holes.iter().map(|hole| &hole.spans))
                .collect::<Vec<_>>();
            for (ordinal, spans) in loops.into_iter().enumerate() {
                let index = u16::try_from(ordinal)
                    .map_err(|_| ProjectionalProfileOffsetError::CardinalityExceeded)?;
                let alias = IntentKey::new(format!(
                    "profile-offset-{revision:016x}-profile-{index:04x}"
                ))?;
                let aggregate = aggregate_draft(
                    intent,
                    ownership,
                    AggregateKind::ClosedProfile,
                    alias.clone(),
                    spans,
                )?;
                operations.push(IntentPatchOperation::CreateNode {
                    alias: alias.clone(),
                    draft: Box::new(aggregate),
                    cell: None,
                });
                operation.inputs.insert(
                    InputSlot::new(InputRole::Profile, index),
                    alias_port(&alias, IntentPortRole::Profile),
                );
                aggregate_aliases.push(alias);
            }
            operation.fields.insert(
                field_key("direction")?,
                enum_literal(match direction {
                    DocumentFaceOffsetDirection::Outward => "outward",
                    DocumentFaceOffsetDirection::Inward => "inward",
                })?,
            );
        }
        OffsetAuthoringOperand::OpenChain { spans, side } => {
            let alias = IntentKey::new(format!("profile-offset-{revision:016x}-open-chain"))?;
            let aggregate = aggregate_draft(
                intent,
                ownership,
                AggregateKind::OpenChain,
                alias.clone(),
                spans,
            )?;
            operations.push(IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(aggregate),
                cell: None,
            });
            operation.inputs.insert(
                InputSlot::new(InputRole::Chain, 0),
                alias_port(&alias, IntentPortRole::Chain),
            );
            operation.fields.insert(
                field_key("side")?,
                enum_literal(match side {
                    DocumentLineSide::Left => "left",
                    DocumentLineSide::Right => "right",
                })?,
            );
            operation.fields.insert(
                field_key("first_traversal")?,
                enum_literal(match spans.first().map(|span| span.traversal) {
                    Some(OffsetTraversal::Forward) => "forward",
                    Some(OffsetTraversal::Reverse) => "reverse",
                    None => return Err(ProjectionalProfileOffsetError::IncompleteCandidate),
                })?,
            );
            aggregate_aliases.push(alias);
        }
    }
    Ok(())
}

fn aggregate_draft(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    kind: AggregateKind,
    symbol: IntentKey,
    spans: &[OffsetDirectedSpan],
) -> Result<IntentNodeDraft, ProjectionalProfileOffsetError> {
    if spans.is_empty() {
        return Err(ProjectionalProfileOffsetError::IncompleteCandidate);
    }
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Aggregate { aggregate: kind }, symbol);
    for (ordinal, directed) in spans.iter().copied().enumerate() {
        let ordinal = u16::try_from(ordinal)
            .map_err(|_| ProjectionalProfileOffsetError::CardinalityExceeded)?;
        draft = draft.with_input(
            InputSlot::new(InputRole::Span, ordinal),
            exact_span_owner(intent, ownership, directed.span)?,
        );
    }
    Ok(draft)
}

fn exact_span_owner(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    span: geosolve_sketch::CurveSpan,
) -> Result<PatchPortRef, ProjectionalProfileOffsetError> {
    let mut candidates = ownership
        .ports
        .iter()
        .filter_map(|(port, binding)| {
            (*binding == IntentNativeBinding::CurveSpan(span)).then_some(*port)
        })
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
        .ok_or(ProjectionalProfileOffsetError::UnownedSourceSpan)
}

fn intent_outputs(
    plan: &SketchOperationOutputPlan,
) -> Result<Vec<IntentOperationOutput>, ProjectionalProfileOffsetError> {
    plan.slots
        .iter()
        .enumerate()
        .map(|(ordinal, slot)| {
            if slot.ordinal != ordinal {
                return Err(ProjectionalProfileOffsetError::WrongOutputPlan);
            }
            Ok(match slot.kind {
                SketchMaterializationIdentityKind::Point => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Point)
                }
                SketchMaterializationIdentityKind::Scalar => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Scalar)
                }
                SketchMaterializationIdentityKind::Curve => IntentOperationOutput::curve(1),
                SketchMaterializationIdentityKind::Contact => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Contact)
                }
                SketchMaterializationIdentityKind::Constraint => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Constraint)
                }
                SketchMaterializationIdentityKind::Dimension => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Dimension)
                }
                SketchMaterializationIdentityKind::Parameter => {
                    IntentOperationOutput::native(IntentOperationOutputKind::Parameter)
                }
                SketchMaterializationIdentityKind::ExternalBinding => {
                    IntentOperationOutput::native(IntentOperationOutputKind::ExternalBinding)
                }
                _ => return Err(ProjectionalProfileOffsetError::UnsupportedOutputKind),
            })
        })
        .collect()
}

fn profile_offset_owner(
    intent: &IntentSession,
    ownership: &IntentMaterializationMap,
    dimension: DocumentDimensionId,
) -> Result<NodeId, ProjectionalProfileOffsetError> {
    if ownership.semantic != intent.semantic_identity() {
        return Err(ProjectionalProfileOffsetError::StaleOwnership);
    }
    let owners = ownership
        .nodes
        .iter()
        .filter(|node| {
            node.owned
                .contains(&IntentNativeBinding::Dimension(dimension))
        })
        .map(|node| node.node)
        .collect::<Vec<_>>();
    let [node] = owners.as_slice() else {
        return Err(ProjectionalProfileOffsetError::AmbiguousDimensionOwnership(
            dimension,
        ));
    };
    if !matches!(
        intent.graph().node(*node).map(|node| &node.kind),
        Some(IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset
        })
    ) {
        return Err(ProjectionalProfileOffsetError::WrongDeclarationKind);
    }
    Ok(*node)
}

fn native_operand(operand: &OffsetAuthoringOperand) -> SketchProfileOffsetOperand {
    match operand {
        OffsetAuthoringOperand::Face { key, direction } => SketchProfileOffsetOperand::Face {
            key: key.clone(),
            direction: *direction,
        },
        OffsetAuthoringOperand::OpenChain { spans, side } => {
            SketchProfileOffsetOperand::OpenChain {
                spans: spans.clone(),
                side: *side,
            }
        }
    }
}

fn alias_port(alias: &IntentKey, role: IntentPortRole) -> PatchPortRef {
    PatchPortRef::Alias {
        node: alias.clone(),
        selector: IntentPortSelector::Node { role, index: 0 },
    }
}

fn field_key(value: &str) -> Result<IntentFieldKey, IntentKeyError> {
    Ok(IntentFieldKey(IntentKey::new(value)?))
}

fn enum_literal(value: &str) -> Result<IntentLiteral, IntentKeyError> {
    Ok(IntentLiteral::Enum(IntentKey::new(value)?))
}

const fn quantity(value: f64, unit: IntentUnit) -> IntentLiteral {
    IntentLiteral::Quantity { value, unit }
}
