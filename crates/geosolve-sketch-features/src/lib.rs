// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent computed-feature intent and exact-stamped generated sketch geometry.
//!
//! Computed features remain outside the constrained [`geosolve_sketch`] design graph.
//! This crate owns stable feature/corner intent, branch-explicit Fillet resolution,
//! atomic endpoint-claim composition and revision-local generated edges. It never
//! adds a solver equation or mutates accepted sketch geometry.

mod document;
mod evaluation;

#[cfg(test)]
mod tests;

pub use document::{
    COMPUTED_FEATURE_DOCUMENT_VERSION, ComputedFeature, ComputedFeatureAllocatorHighWater,
    ComputedFeatureCornerId, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentDigest, ComputedFeatureDocumentError, ComputedFeatureDocumentId,
    ComputedFeatureDocumentIdentity, ComputedFeatureId, ComputedFeatureLifecycleHighWater,
    ComputedFeatureRevision, ComputedFilletCorner, ComputedFilletParent, ComputedFilletSet,
    MAX_COMPUTED_FEATURE_CORNERS, MAX_COMPUTED_FEATURE_JSON_BYTES,
    MAX_COMPUTED_FEATURE_LABEL_BYTES, MAX_COMPUTED_FEATURES, NativeCurveSpanSource,
    NewComputedFilletCorner,
};
pub use evaluation::{
    ComputedCircularArc, ComputedClaimEndpoint, ComputedCornerRef, ComputedEdge,
    ComputedEdgeGeometry, ComputedEdgeId, ComputedEdgeProvenance, ComputedEvaluationAllocator,
    ComputedEvaluationAllocatorHighWater, ComputedEvaluationRevision,
    ComputedFeatureAuthoringError, ComputedFeatureAuthoringSnapshot, ComputedFeatureEvaluation,
    ComputedFeatureEvaluationError, ComputedFeatureEvaluationInput,
    ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureSnapshot,
    ComputedFeatureSnapshotError, ComputedFilletAuthoringOptions, ComputedFilletContact,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, ComputedSourceInterval,
    PreparedComputedFeatureEvaluation, ResolvedComputedFilletCorner,
};
