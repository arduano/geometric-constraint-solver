// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent, editable action lineage for flat `GeoSolve` sketches.
//!
//! This crate owns only declarative lineage intent, stable logical identity,
//! structural validation, exact-CAS mutation, and user-visible Undo/Redo. It
//! deliberately does not solve or materialize geometry. A materializer can
//! consume a validated [`LineageDocument`] and publish ordinary independently
//! validated sketch-domain results without making those results the lineage
//! source of truth.

mod document;
mod ids;
mod materialization_map;
mod model;
mod session;

pub use document::{
    LINEAGE_DOCUMENT_VERSION, LineageAllocatorHighWater, LineageDependencyPlanEntry,
    LineageDocument, LineageDocumentError, LineageEvaluationPolicy, LineageMutation, LineagePatch,
    LineagePatchOutcome, LineageStepEvaluationState, LineageStepRewrite, MAX_LINEAGE_ACTION_INPUTS,
    MAX_LINEAGE_ACTION_PAYLOAD_BYTES, MAX_LINEAGE_BASELINE_BYTES, MAX_LINEAGE_JSON_BYTES,
    MAX_LINEAGE_LABEL_BYTES, MAX_LINEAGE_OUTPUTS, MAX_LINEAGE_PAYLOAD_DEPTH,
    MAX_LINEAGE_PAYLOAD_NODES, MAX_LINEAGE_RESERVATIONS, MAX_LINEAGE_STEPS,
    MAX_LINEAGE_WRITABLE_LEAVES,
};
pub use ids::{
    LineageAuxiliaryHighWater, LineageDeveloperKey, LineageDigest, LineageDocumentId,
    LineageDocumentIdentity, LineageIdParseError, LineageKeyError, LineageOpaqueId,
    LineageOutputId, LineageReservationId, LineageRevision, LineageSemanticKey, LineageStepId,
};
pub use materialization_map::{
    LINEAGE_MATERIALIZATION_MAP_VERSION, LineageLogicalLeaf, LineageLogicalReverseBinding,
    LineageMaterializationBinding, LineageMaterializationMap, LineageMaterializationMapError,
    LineageMaterializationOwner, LineageMaterializationReverseBinding, LineageMaterializedIdentity,
    LineageMaterializedLeaf, MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES,
};
pub use model::{
    ImportedBaselineAction, ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind,
    LineageInputBinding, LineageOutput, LineageOutputIdentity, LineageOutputIdentityFlow,
    LineageOutputKind, LineageOutputRef, LineageReservation, LineageReservationKind, LineageStep,
    LineageStepState, LineageWritableLeaf, VersionedActionPayload,
};
pub use session::{
    LINEAGE_SESSION_VERSION, LineageAcceptedAuthority, LineageEvaluationAttempt,
    LineageEvaluationDisposition, LineageLifecycleHighWater, LineageSession,
    MAX_LINEAGE_HISTORY_ENTRIES, MAX_LINEAGE_SESSION_JSON_BYTES,
};

/// Computes the canonical 256-bit digest used for independently materialized
/// payloads and external-input stamps.
///
/// The bytes are hashed exactly as supplied; callers remain responsible for
/// using a deterministic, versioned encoding.
#[must_use]
pub fn lineage_content_digest(bytes: &[u8]) -> LineageDigest {
    document::digest_bytes(bytes)
}
