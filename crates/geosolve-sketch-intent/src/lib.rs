// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

//! Order-independent, projectional design intent for `GeoSolve` sketches.
//!
//! This crate owns stable logical identity, typed declarative structure,
//! presentation-only organization, writable instance state, exact-CAS patch
//! planning, and bounded user Undo/Redo. It deliberately contains no solver
//! equations. A host materializer evaluates a structurally validated
//! [`IntentCandidate`] through the existing sketch solver and returns bounded,
//! content-authenticated [`MaterializationEvidence`]. The host, not this
//! equation-free crate, remains responsible for native residual and geometry
//! validation before it returns an accepted evaluation.

mod graph;
mod ids;
mod model;
mod patch;
mod session;

pub use graph::{
    IntentAllocatorHighWater, IntentChild, IntentGraph, IntentGraphError, IntentNode, IntentPort,
    IntentReservation, IntentReservationLedger, IntentReservationRecord, IntentReservationState,
    MAX_INTENT_RESERVATION_LEDGER_ENTRIES,
};
pub use ids::{
    CellId, ChildId, ComponentIdentity, ContentDigest, ExternalInputRevision, IntentIdParseError,
    IntentKey, IntentKeyError, IntentSessionId, NodeId, PlanToken, PortId, ReservationId, Revision,
};
pub use model::{
    Axis, BootstrapNativeKind, ComputedFeatureKind, ConstraintKind, DimensionKind,
    ExternalIntentKind, GeometryRecipeKind, IdentityTransitionKind, InputRole, InputSlot,
    IntentBootstrapObject, IntentChildSchema, IntentExternalInputs, IntentExternalInputsIdentity,
    IntentFieldKey, IntentGraphIdentity, IntentIdentityFlow, IntentInstanceIdentity,
    IntentInstanceState, IntentLiteral, IntentModelError, IntentNativeReservationKind,
    IntentNodeDraft, IntentNodeKind, IntentOrganization, IntentOrganizationIdentity,
    IntentPortKind, IntentPortRef, IntentPortRole, IntentPortSelector,
    IntentReservationLedgerIdentity, IntentSemanticIdentity, IntentUnit, LeafField, LeafRef,
    MaterializationEvidence, OperationKind, OrganizationCell, ParameterIntentKind, PatchPortRef,
};
pub use patch::{
    CellTarget, DeletePolicy, IntentAliasMap, IntentPatch, IntentPatchOperation,
    IntentPatchOperationKind, IntentPatchPolicy, IntentSemanticDiff,
};
pub use session::{
    INTENT_SESSION_VERSION, IntentAcceptedAuthority, IntentAttemptDisposition, IntentCandidate,
    IntentEvaluation, IntentEvaluationFailure, IntentEvaluationFailureKind,
    IntentHistoryProjection, IntentLatestAttempt, IntentPatchPlan, IntentPlanDisposition,
    IntentPlanError, IntentSession, IntentSessionError, IntentSessionIdentity,
    IntentTransactionDescriptor, MAX_INTENT_HISTORY_ENTRIES, MAX_INTENT_SESSION_JSON_BYTES,
};

/// Computes the canonical deterministic 256-bit content digest used by intent
/// identities and opaque materialization evidence.
#[must_use]
pub fn intent_content_digest(bytes: &[u8]) -> ContentDigest {
    ids::digest_bytes(bytes)
}
