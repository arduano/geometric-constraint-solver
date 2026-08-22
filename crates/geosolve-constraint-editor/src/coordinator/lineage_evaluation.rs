// SPDX-License-Identifier: GPL-3.0-or-later

//! DOM-free cold evaluation of one authoritative lineage session.
//!
//! This module is deliberately a thin composition seam. Lineage owns ordering
//! and materialization, while the sketch and computed-feature domains retain
//! their ordinary solve and independent-validation authority.

use geosolve_sketch::{
    DocumentSolveRequest, ExternalSnapshotSet, ParameterBatch, RetainedSketchDocumentSession,
    SketchDocument, SketchLifecycleRevisionHighWater, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureDocument, ComputedFeatureEvaluationState,
};
use geosolve_sketch_lineage::{
    LineageDigest, LineageDocumentIdentity, LineageEvaluationPolicy, LineageMaterializationMap,
    LineageOpaqueId, LineageSession, LineageStepId, lineage_content_digest,
};
use serde::Serialize;

use super::lineage::{CoordinatorLineage, external_input_stamp};
use super::{bounded_geometry_control, evaluate_computed_features};

/// Stable evaluator identity included in every accepted materialization digest.
pub const LINEAGE_DOMAIN_EVALUATOR: &str = "geosolve.constraint-editor.lineage-cold.v1";

/// Engine-owned evidence for one complete cold, independently validated
/// workbench materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageDomainEvaluationEvidence {
    lineage: LineageDocumentIdentity,
    external_inputs: LineageOpaqueId,
    materialization_digest: LineageDigest,
    accepted_encoding: &'static str,
    accepted_sketch_json: String,
    sketch_digest: LineageDigest,
    feature_digest: LineageDigest,
    computed_feature_count: usize,
    validated_prefix_count: usize,
    strict_prefix_digest: LineageDigest,
    ownership_digest: LineageDigest,
    work: LineageEvaluationWorkEvidence,
}

/// Exact structural and owning-domain work performed by one policy-qualified
/// evaluation. The mandatory differential oracle is charged separately from
/// dependency-local work so reuse is measurable without concealing its gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageEvaluationWorkEvidence {
    policy: LineageEvaluationPolicy,
    ready_prefix_count: usize,
    reusable_prefix_count: usize,
    reconstructed_prefix_count: usize,
    replayed_suffix_count: usize,
    policy_prefix_evaluation_count: usize,
    oracle_prefix_evaluation_count: usize,
    dirty_steps: Vec<LineageStepId>,
}

impl LineageEvaluationWorkEvidence {
    /// Policy selected by the retained lineage document.
    #[must_use]
    pub const fn policy(&self) -> LineageEvaluationPolicy {
        self.policy
    }

    /// Complete number of ready chronological prefixes in the target program.
    #[must_use]
    pub const fn ready_prefix_count(&self) -> usize {
        self.ready_prefix_count
    }

    /// Ready steps in the exact unchanged accepted prefix.
    #[must_use]
    pub const fn reusable_prefix_count(&self) -> usize {
        self.reusable_prefix_count
    }

    /// Reusable-prefix steps structurally reconstructed from retained lineage.
    /// This is reported because the current cold API has no cross-call cache.
    #[must_use]
    pub const fn reconstructed_prefix_count(&self) -> usize {
        self.reconstructed_prefix_count
    }

    /// Ready suffix steps structurally replayed after the reusable prefix.
    #[must_use]
    pub const fn replayed_suffix_count(&self) -> usize {
        self.replayed_suffix_count
    }

    /// Prefix checkpoints independently evaluated by the selected policy.
    #[must_use]
    pub const fn policy_prefix_evaluation_count(&self) -> usize {
        self.policy_prefix_evaluation_count
    }

    /// Prefix checkpoints independently evaluated by the strict differential
    /// oracle. In strict mode this is the same pass, not additional work.
    #[must_use]
    pub const fn oracle_prefix_evaluation_count(&self) -> usize {
        self.oracle_prefix_evaluation_count
    }

    /// Exact typed forward dependency closure selected for local evaluation.
    #[must_use]
    pub fn dirty_steps(&self) -> &[LineageStepId] {
        &self.dirty_steps
    }

    /// Total owning-domain prefix evaluations actually performed. Local mode
    /// includes both its optimized pass and the mandatory strict recheck.
    #[must_use]
    pub const fn total_prefix_evaluation_count(&self) -> usize {
        if matches!(self.policy, LineageEvaluationPolicy::StrictChronological) {
            self.oracle_prefix_evaluation_count
        } else {
            self.policy_prefix_evaluation_count
                .saturating_add(self.oracle_prefix_evaluation_count)
        }
    }
}

impl LineageDomainEvaluationEvidence {
    /// Exact evaluated lineage revision and digest.
    #[must_use]
    pub const fn lineage(&self) -> LineageDocumentIdentity {
        self.lineage
    }

    /// Canonical engine-derived identity of the exact immutable host inputs
    /// used by this owning-domain attempt.
    #[must_use]
    pub fn external_inputs(&self) -> &LineageOpaqueId {
        &self.external_inputs
    }

    /// Digest of the complete engine-owned workbench materialization evidence.
    #[must_use]
    pub const fn materialization_digest(&self) -> LineageDigest {
        self.materialization_digest
    }

    /// Exact encoding selected by the owning sketch domain for the accepted
    /// cold materialization. This remains crate-private because callers should
    /// normally consume the authenticated digest rather than flat cache bytes.
    pub(super) fn accepted_uses_draft_v5(&self) -> bool {
        matches!(self.accepted_encoding, "draft_v5")
    }

    /// Canonical accepted sketch bytes produced by the genuine cold lineage
    /// evaluation. Projected owner reconciliation uses these bytes to prove
    /// that its staged flat projection is exactly reproducible.
    pub(super) fn accepted_sketch_json(&self) -> &str {
        &self.accepted_sketch_json
    }

    /// Digest of the independently accepted sketch payload.
    #[must_use]
    pub const fn sketch_digest(&self) -> LineageDigest {
        self.sketch_digest
    }

    /// Digest of the canonical computed-feature intent payload.
    #[must_use]
    pub const fn feature_digest(&self) -> LineageDigest {
        self.feature_digest
    }

    /// Number of persistent computed features evaluated in the cold pass.
    #[must_use]
    pub const fn computed_feature_count(&self) -> usize {
        self.computed_feature_count
    }

    /// Number of ready chronological step prefixes independently accepted by
    /// the owning sketch and computed-feature domains.
    #[must_use]
    pub const fn validated_prefix_count(&self) -> usize {
        self.validated_prefix_count
    }

    /// Canonical digest of the ordered independently accepted strict-prefix
    /// evidence. Dependency-local evaluation must reproduce this oracle.
    #[must_use]
    pub const fn strict_prefix_digest(&self) -> LineageDigest {
        self.strict_prefix_digest
    }

    /// Digest of the exact revision-stamped logical/native ownership map.
    #[must_use]
    pub const fn ownership_digest(&self) -> LineageDigest {
        self.ownership_digest
    }

    /// Exact policy-local, structural-replay, and strict-oracle work evidence.
    #[must_use]
    pub const fn work(&self) -> &LineageEvaluationWorkEvidence {
        &self.work
    }
}

/// Structured fail-closed evidence from cold workbench evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageDomainEvaluationFailure {
    code: &'static str,
    diagnostic: &'static str,
    message: String,
    failed_features: Vec<String>,
    failed_step: Option<LineageStepId>,
}

impl LineageDomainEvaluationFailure {
    fn new(code: &'static str, diagnostic: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            diagnostic,
            message: message.into(),
            failed_features: Vec::new(),
            failed_step: None,
        }
    }

    fn with_failed_step(mut self, failed_step: LineageStepId) -> Self {
        self.failed_step = Some(failed_step);
        self
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Stable semantic diagnostic suitable for retained lineage evidence.
    #[must_use]
    pub const fn diagnostic(&self) -> &'static str {
        self.diagnostic
    }

    /// Human-readable owning-domain evidence.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Persistent computed-feature IDs that failed, when feature evaluation
    /// reached a complete per-feature result.
    #[must_use]
    pub fn failed_features(&self) -> &[String] {
        &self.failed_features
    }

    /// Exact chronological owner step whose prefix first failed independent
    /// domain evaluation, when prefix construction reached that step.
    #[must_use]
    pub const fn failed_step(&self) -> Option<LineageStepId> {
        self.failed_step
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptedMaterializationEvidence<'a> {
    evaluator: &'static str,
    version: u32,
    lineage: LineageDocumentIdentity,
    external_inputs: &'a LineageOpaqueId,
    retained_encoding: &'static str,
    retained_sketch_json: &'a str,
    accepted_encoding: &'static str,
    accepted_sketch_json: &'a str,
    computed_feature_json: &'a str,
    computed_features: &'a [AcceptedComputedFeatureEvidence],
    strict_prefixes: &'a [AcceptedPrefixEvidence],
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptedPrefixEvidence {
    step: String,
    external_inputs: String,
    sketch_digest: LineageDigest,
    feature_digest: LineageDigest,
    computed_feature_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AcceptedComputedFeatureEvidence {
    feature: String,
    state: &'static str,
    corner_edges: Vec<[String; 2]>,
    diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceptedCheckpointEvidence {
    external_inputs: LineageOpaqueId,
    retained_encoding: &'static str,
    retained_sketch_json: String,
    accepted_encoding: &'static str,
    accepted_sketch_json: String,
    computed_feature_json: String,
    computed_features: Vec<AcceptedComputedFeatureEvidence>,
    sketch_digest: LineageDigest,
    feature_digest: LineageDigest,
}

struct StrictEvaluationRun {
    final_step: LineageStepId,
    final_checkpoint: super::lineage::LineageCheckpoint,
    accepted: AcceptedCheckpointEvidence,
    prefixes: Vec<AcceptedPrefixEvidence>,
}

struct DependencyLocalEvaluationRun {
    final_checkpoint: super::lineage::LineageCheckpoint,
    accepted: AcceptedCheckpointEvidence,
    work: LineageEvaluationWorkEvidence,
}

/// Strictly rematerializes one lineage session, then delegates sketch solving,
/// hard-residual validation, and computed-feature evaluation to their owning
/// domains. A document that is merely dependency-valid but lacks the
/// workbench's imported-baseline/compiled-materialization representation fails
/// closed.
///
/// # Errors
///
/// Returns structured fail-closed evidence when lineage cannot be strictly
/// rematerialized, either domain rejects or stops, or canonical acceptance
/// evidence cannot be produced.
pub fn evaluate_lineage_session_cold(
    session: &LineageSession,
) -> Result<LineageDomainEvaluationEvidence, LineageDomainEvaluationFailure> {
    evaluate_lineage_session_cold_with_inputs(
        session,
        &ParameterBatch::default(),
        &ExternalSnapshotSet::default(),
    )
}

/// Strictly rematerializes and evaluates one lineage session using the exact
/// immutable host parameter and external-snapshot payloads supplied by the
/// embedding host. The payloads are independently validated by their owning
/// sketch domain and their canonical engine-derived identity is returned in
/// the evidence.
///
/// # Errors
///
/// Returns structured fail-closed evidence when the host payloads cannot be
/// canonically identified, lineage cannot be rematerialized, or either owning
/// domain rejects or stops.
#[allow(
    clippy::too_many_lines,
    reason = "one linear authority pipeline keeps strict rebuild, exact host inputs, independent sketch acceptance, computed-feature acceptance, and evidence hashing visibly ordered"
)]
pub fn evaluate_lineage_session_cold_with_inputs(
    session: &LineageSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<LineageDomainEvaluationEvidence, LineageDomainEvaluationFailure> {
    evaluate_lineage_session_cold_with_inputs_and_branch(
        session,
        parameters,
        snapshots,
        EvaluationBranch::Retained,
    )
}

/// Cold-authenticates the older accepted sketch embedded in one honest legacy
/// imported baseline. This is used only while constructing or validating the
/// retained-invalid/older-accepted authority split; ordinary current
/// evaluation always uses [`evaluate_lineage_session_cold_with_inputs`].
pub(super) fn evaluate_lineage_embedded_accepted_baseline_cold_with_inputs(
    session: &LineageSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<LineageDomainEvaluationEvidence, LineageDomainEvaluationFailure> {
    evaluate_lineage_session_cold_with_inputs_and_branch(
        session,
        parameters,
        snapshots,
        EvaluationBranch::EmbeddedAcceptedBaseline,
    )
}

/// Reproduces the exact accepted authority, preferring the ordinary retained
/// branch and using the embedded legacy branch only when that is the digest
/// the session actually published.
pub(super) fn evaluate_lineage_accepted_authority_cold_with_inputs(
    session: &LineageSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<LineageDomainEvaluationEvidence, LineageDomainEvaluationFailure> {
    let authority = session
        .last_accepted()
        .ok_or_else(no_accepted_prefix_failure)?;
    let document = session
        .last_accepted_document()
        .ok_or_else(no_accepted_prefix_failure)?;
    let expected = authority.materialization_digest;
    let accepted_session = LineageSession::new(document.clone()).map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "lineage_session_invalid",
            "lineage-evaluation-session-invalid",
            error.to_string(),
        )
    })?;
    let ordinary =
        evaluate_lineage_session_cold_with_inputs(&accepted_session, parameters, snapshots);
    if let Ok(evaluation) = &ordinary
        && evaluation.materialization_digest() == expected
    {
        return ordinary;
    }
    let embedded = evaluate_lineage_embedded_accepted_baseline_cold_with_inputs(
        &accepted_session,
        parameters,
        snapshots,
    );
    if let Ok(evaluation) = &embedded
        && evaluation.materialization_digest() == expected
    {
        return embedded;
    }
    ordinary.or(embedded)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvaluationBranch {
    Retained,
    EmbeddedAcceptedBaseline,
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear authority pipeline keeps strict rebuild, exact host inputs, independent sketch acceptance, computed-feature acceptance, and evidence hashing visibly ordered"
)]
fn evaluate_lineage_session_cold_with_inputs_and_branch(
    session: &LineageSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    branch: EvaluationBranch,
) -> Result<LineageDomainEvaluationEvidence, LineageDomainEvaluationFailure> {
    let external_inputs = lineage_external_input_stamp(parameters, snapshots)?;
    let session_json = session.to_canonical_session_json().map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "lineage_session_invalid",
            "lineage-evaluation-session-invalid",
            error.to_string(),
        )
    })?;
    let lineage = CoordinatorLineage::from_session_json(&session_json).map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "lineage_session_invalid",
            "lineage-evaluation-session-invalid",
            error.to_string(),
        )
    })?;
    let accepted_inputs_match = session.last_accepted().is_some_and(|authority| {
        authority.external_inputs.as_ref() == Some(&external_inputs)
            && session.last_accepted_document().is_some()
    });
    let local_result = (branch == EvaluationBranch::Retained
        && session.document().evaluation_policy() == LineageEvaluationPolicy::DependencyLocal)
        .then(|| {
            evaluate_dependency_local_run(&lineage, parameters, snapshots, accepted_inputs_match)
        });
    let strict_result = match branch {
        EvaluationBranch::Retained => evaluate_strict_run(&lineage, parameters, snapshots),
        EvaluationBranch::EmbeddedAcceptedBaseline => lineage
            .embedded_accepted_baseline_prefixes()
            .map_err(|error| materialization_failure(&error))?
            .ok_or_else(no_accepted_prefix_failure)
            .and_then(|prefixes| {
                evaluate_strict_run_from_prefixes(&lineage, prefixes, parameters, snapshots)
            }),
    };
    let (strict, work) = match (local_result, strict_result) {
        (None, Ok(strict)) => {
            let prefix_count = strict.prefixes.len();
            (
                strict,
                LineageEvaluationWorkEvidence {
                    policy: LineageEvaluationPolicy::StrictChronological,
                    ready_prefix_count: prefix_count,
                    reusable_prefix_count: 0,
                    reconstructed_prefix_count: 0,
                    replayed_suffix_count: prefix_count,
                    policy_prefix_evaluation_count: prefix_count,
                    oracle_prefix_evaluation_count: prefix_count,
                    dirty_steps: Vec::new(),
                },
            )
        }
        (None, Err(failure)) => return Err(failure),
        (Some(Ok(local)), Ok(strict)) => {
            if local.final_checkpoint != strict.final_checkpoint
                || local.accepted != strict.accepted
            {
                return Err(dependency_local_mismatch(
                    "dependency-local accepted checkpoint differs from the strict chronological oracle",
                    Some(strict.final_step),
                ));
            }
            let mut work = local.work;
            work.oracle_prefix_evaluation_count = strict.prefixes.len();
            (strict, work)
        }
        (Some(Err(local)), Err(strict)) if local == strict => return Err(strict),
        (Some(Err(local)), Err(strict)) => {
            return Err(dependency_local_mismatch(
                format!(
                    "dependency-local failure authority differs from the strict chronological oracle: local={}/{}; strict={}/{}",
                    local.code(),
                    local.message(),
                    strict.code(),
                    strict.message()
                ),
                strict.failed_step().or(local.failed_step()),
            ));
        }
        (Some(Ok(_)), Err(strict)) => {
            return Err(dependency_local_mismatch(
                "dependency-local evaluation accepted a strict chronological rejection",
                strict.failed_step(),
            ));
        }
        (Some(Err(local)), Ok(strict)) => {
            return Err(dependency_local_mismatch(
                format!(
                    "dependency-local evaluation rejected a strict chronological acceptance: local={}/{}",
                    local.code(),
                    local.message()
                ),
                local.failed_step().or(Some(strict.final_step)),
            ));
        }
    };
    let StrictEvaluationRun {
        accepted,
        prefixes: strict_prefixes,
        ..
    } = strict;
    if accepted.external_inputs != external_inputs {
        return Err(LineageDomainEvaluationFailure::new(
            "host_input_identity_mismatch",
            "lineage-evaluation-host-input-mismatch",
            "the final strict prefix did not consume the complete exact host-input payload",
        ));
    }
    let ownership_json = LineageMaterializationMap::derive(session.document())
        .and_then(|map| map.to_canonical_json())
        .map_err(|error| {
            LineageDomainEvaluationFailure::new(
                "materialization_map_error",
                "lineage-evaluation-materialization-map-error",
                error.to_string(),
            )
        })?;
    let ownership_digest = lineage_content_digest(ownership_json.as_bytes());
    let strict_prefix_digest =
        lineage_content_digest(&serde_json::to_vec(&strict_prefixes).map_err(|error| {
            LineageDomainEvaluationFailure::new(
                "materialization_evidence_error",
                "lineage-evaluation-evidence-error",
                error.to_string(),
            )
        })?);
    let evidence = AcceptedMaterializationEvidence {
        evaluator: LINEAGE_DOMAIN_EVALUATOR,
        version: 1,
        lineage: session.identity(),
        external_inputs: &external_inputs,
        retained_encoding: accepted.retained_encoding,
        retained_sketch_json: &accepted.retained_sketch_json,
        accepted_encoding: accepted.accepted_encoding,
        accepted_sketch_json: &accepted.accepted_sketch_json,
        computed_feature_json: &accepted.computed_feature_json,
        computed_features: &accepted.computed_features,
        strict_prefixes: &strict_prefixes,
    };
    let materialization_digest =
        lineage_content_digest(&serde_json::to_vec(&evidence).map_err(|error| {
            LineageDomainEvaluationFailure::new(
                "materialization_evidence_error",
                "lineage-evaluation-evidence-error",
                error.to_string(),
            )
        })?);
    Ok(LineageDomainEvaluationEvidence {
        lineage: session.identity(),
        external_inputs,
        materialization_digest,
        accepted_encoding: accepted.accepted_encoding,
        accepted_sketch_json: accepted.accepted_sketch_json,
        sketch_digest: accepted.sketch_digest,
        feature_digest: accepted.feature_digest,
        computed_feature_count: accepted.computed_features.len(),
        validated_prefix_count: strict_prefixes.len(),
        strict_prefix_digest,
        ownership_digest,
        work,
    })
}

fn evaluate_strict_run(
    lineage: &CoordinatorLineage,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<StrictEvaluationRun, LineageDomainEvaluationFailure> {
    let prefixes = lineage
        .strict_ready_prefixes()
        .map_err(|error| materialization_failure(&error))?;
    evaluate_strict_run_from_prefixes(lineage, prefixes, parameters, snapshots)
}

fn evaluate_strict_run_from_prefixes(
    lineage: &CoordinatorLineage,
    prefixes: Vec<super::lineage::LineageMaterializedPrefix>,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<StrictEvaluationRun, LineageDomainEvaluationFailure> {
    let mut accepted_prefixes = Vec::with_capacity(prefixes.len());
    let mut accepted_final = None;
    let mut previous_accepted = None;
    let mut final_checkpoint = None;
    let mut final_step = None;
    let prefix_count = prefixes.len();
    for (index, prefix) in prefixes.into_iter().enumerate() {
        let step = prefix.step();
        let is_final = index + 1 == prefix_count;
        let (prefix_parameters, prefix_snapshots) = if is_final {
            (parameters, snapshots)
        } else {
            (
                prefix.host_inputs().parameters(),
                prefix.host_inputs().snapshots(),
            )
        };
        let native_fillet = lineage
            .native_fillet_continuation_plan(step)
            .map_err(|error| materialization_failure(&error))?;
        let accepted = evaluate_checkpoint_cold(
            &prefix.checkpoint().clone().into_restore_checkpoint(),
            prefix_parameters,
            prefix_snapshots,
            prefix.host_inputs().parameters(),
            prefix.host_inputs().snapshots(),
            previous_accepted.as_ref(),
            native_fillet.as_ref(),
        )
        .map_err(|failure| failure.with_failed_step(step))?;
        accepted_prefixes.push(AcceptedPrefixEvidence {
            step: step.to_string(),
            external_inputs: accepted.external_inputs.as_str().to_owned(),
            sketch_digest: accepted.sketch_digest,
            feature_digest: accepted.feature_digest,
            computed_feature_count: accepted.computed_features.len(),
        });
        final_checkpoint = Some(prefix.checkpoint().clone());
        final_step = Some(step);
        previous_accepted = Some(PreviousAcceptedCheckpoint {
            retained_encoding: accepted.retained_encoding,
            retained_sketch_json: accepted.retained_sketch_json.clone(),
            encoding: accepted.accepted_encoding,
            sketch_json: accepted.accepted_sketch_json.clone(),
            parameters: prefix_parameters.clone(),
            snapshots: prefix_snapshots.clone(),
        });
        accepted_final = Some(accepted);
    }
    Ok(StrictEvaluationRun {
        final_step: final_step.ok_or_else(no_ready_prefix_failure)?,
        final_checkpoint: final_checkpoint.ok_or_else(no_ready_prefix_failure)?,
        accepted: accepted_final.ok_or_else(no_accepted_prefix_failure)?,
        prefixes: accepted_prefixes,
    })
}

fn evaluate_dependency_local_run(
    lineage: &CoordinatorLineage,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    accepted_inputs_match: bool,
) -> Result<DependencyLocalEvaluationRun, LineageDomainEvaluationFailure> {
    let materialized = lineage
        .dependency_local_materialization(accepted_inputs_match)
        .map_err(|error| materialization_failure(&error))?;
    let checkpoint_count = materialized.evaluation_checkpoints().len();
    let mut accepted_final = None;
    let mut previous_accepted = None;
    for (index, prefix) in materialized.evaluation_checkpoints().iter().enumerate() {
        let is_final = index + 1 == checkpoint_count;
        let (prefix_parameters, prefix_snapshots) = if is_final {
            (parameters, snapshots)
        } else {
            (
                prefix.host_inputs().parameters(),
                prefix.host_inputs().snapshots(),
            )
        };
        let native_fillet = lineage
            .native_fillet_continuation_plan(prefix.step())
            .map_err(|error| materialization_failure(&error))?;
        let accepted = evaluate_checkpoint_cold(
            &prefix.checkpoint().clone().into_restore_checkpoint(),
            prefix_parameters,
            prefix_snapshots,
            prefix.host_inputs().parameters(),
            prefix.host_inputs().snapshots(),
            previous_accepted.as_ref(),
            native_fillet.as_ref(),
        )
        .map_err(|failure| failure.with_failed_step(prefix.step()))?;
        previous_accepted = Some(PreviousAcceptedCheckpoint {
            retained_encoding: accepted.retained_encoding,
            retained_sketch_json: accepted.retained_sketch_json.clone(),
            encoding: accepted.accepted_encoding,
            sketch_json: accepted.accepted_sketch_json.clone(),
            parameters: prefix_parameters.clone(),
            snapshots: prefix_snapshots.clone(),
        });
        accepted_final = Some(accepted);
    }
    Ok(DependencyLocalEvaluationRun {
        final_checkpoint: materialized.final_checkpoint().clone(),
        accepted: accepted_final.ok_or_else(no_accepted_prefix_failure)?,
        work: LineageEvaluationWorkEvidence {
            policy: LineageEvaluationPolicy::DependencyLocal,
            ready_prefix_count: materialized.ready_prefix_count(),
            reusable_prefix_count: materialized.reusable_prefix_count(),
            reconstructed_prefix_count: materialized.reconstructed_prefix_count(),
            replayed_suffix_count: materialized.replayed_suffix_count(),
            policy_prefix_evaluation_count: checkpoint_count,
            oracle_prefix_evaluation_count: 0,
            dirty_steps: materialized.dirty_steps().to_vec(),
        },
    })
}

fn materialization_failure(
    error: &super::lineage::LineageBridgeError,
) -> LineageDomainEvaluationFailure {
    let failed_step = error.materialization_step();
    let failure = LineageDomainEvaluationFailure::new(
        "workbench_materialization_unsupported",
        "lineage-evaluation-materialization-unsupported",
        error.to_string(),
    );
    failed_step.map_or(failure.clone(), |step| failure.with_failed_step(step))
}

fn no_ready_prefix_failure() -> LineageDomainEvaluationFailure {
    LineageDomainEvaluationFailure::new(
        "workbench_materialization_unsupported",
        "lineage-evaluation-materialization-unsupported",
        "strict chronological evaluation produced no ready workbench prefix",
    )
}

fn no_accepted_prefix_failure() -> LineageDomainEvaluationFailure {
    LineageDomainEvaluationFailure::new(
        "workbench_materialization_unsupported",
        "lineage-evaluation-materialization-unsupported",
        "lineage evaluation produced no accepted workbench prefix",
    )
}

fn dependency_local_mismatch(
    message: impl Into<String>,
    failed_step: Option<LineageStepId>,
) -> LineageDomainEvaluationFailure {
    let failure = LineageDomainEvaluationFailure::new(
        "dependency_local_mismatch",
        "lineage-evaluation-dependency-local-mismatch",
        message,
    );
    failed_step.map_or(failure.clone(), |step| failure.with_failed_step(step))
}

#[allow(
    clippy::too_many_lines,
    reason = "one owning-domain prefix evaluation keeps strict document decoding, independent sketch acceptance, computed-feature acceptance, and canonical evidence extraction visibly ordered"
)]
fn evaluate_checkpoint_cold(
    checkpoint: &super::RestoreCheckpoint,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    authored_parameters: &ParameterBatch,
    authored_snapshots: &ExternalSnapshotSet,
    previous_accepted: Option<&PreviousAcceptedCheckpoint>,
    native_fillet: Option<&geosolve_sketch::DocumentPreparedNativeLineFilletGeometry>,
) -> Result<AcceptedCheckpointEvidence, LineageDomainEvaluationFailure> {
    let design = if checkpoint.design_uses_draft_v5() {
        SketchDocument::from_draft_v5_json(checkpoint.design_json())
    } else {
        SketchDocument::from_json(checkpoint.design_json())
    }
    .map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "sketch_document_invalid",
            "lineage-evaluation-sketch-invalid",
            error.to_string(),
        )
    })?;
    let features =
        ComputedFeatureDocument::from_json(checkpoint.feature_json()).map_err(|error| {
            LineageDomainEvaluationFailure::new(
                "feature_document_invalid",
                "lineage-evaluation-feature-invalid",
                error.to_string(),
            )
        })?;
    if features.sketch_document() != design.id() {
        return Err(LineageDomainEvaluationFailure::new(
            "feature_namespace_mismatch",
            "lineage-evaluation-feature-namespace-mismatch",
            "computed-feature intent belongs to a different sketch document",
        ));
    }

    let retained_encoding = if checkpoint.design_uses_draft_v5() {
        "draft_v5"
    } else {
        "canonical_v4"
    };
    let retained_sketch_json = checkpoint.design_json().to_owned();
    let sketch_session = evaluate_sketch_prefix_with_exact_inputs(
        &design,
        parameters,
        snapshots,
        authored_parameters,
        authored_snapshots,
        previous_accepted,
        native_fillet,
    )?;
    let accepted = sketch_session
        .accepted_state_for_current_input()
        .ok_or_else(|| {
            let message = sketch_session.last_attempt().failure().map_or_else(
                || "owning sketch domain rejected the cold materialization".to_owned(),
                |failure| failure.message().to_owned(),
            );
            LineageDomainEvaluationFailure::new(
                "sketch_evaluation_rejected",
                "lineage-evaluation-sketch-rejected",
                message,
            )
        })?;
    let (accepted_sketch_json, accepted_is_draft_v5) =
        super::history::checkpoint_document_to_json(accepted.document()).map_err(|error| {
            LineageDomainEvaluationFailure::new(
                "sketch_evidence_error",
                "lineage-evaluation-sketch-evidence-error",
                error.to_string(),
            )
        })?;

    // Computed edge identities are revision-local derived evidence, not
    // persistent lineage identity.  A restored host allocator may legitimately
    // be exhausted while the same feature intent remains independently
    // reproducible (the live coordinator then withholds computed presentation
    // and reports that allocator problem).  Cold lineage validation therefore
    // evaluates every prefix in its own deterministic scratch identity space;
    // the exact host high-water is retained separately as session auxiliary
    // lifecycle state and is reapplied when the checkpoint is restored.
    let mut evaluation_allocator = ComputedEvaluationAllocator::default();
    let evaluated = evaluate_computed_features(
        &sketch_session,
        &features,
        &mut evaluation_allocator,
        bounded_geometry_control(),
    )
    .map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "feature_evaluation_error",
            "lineage-evaluation-feature-error",
            error.to_string(),
        )
    })?;
    // Computed output is derived presentation authority, not a prerequisite
    // for accepting independently solved native sketch state and persistent
    // feature intent.  A persistent feature may truthfully be Failed, and a
    // bounded evaluation may truthfully withhold every generated fragment.
    // Authenticate those dispositions in the cold digest instead of
    // reclassifying the owning sketch materialization as rejected.  Projected
    // direct manipulation keeps its stricter all-Current publication gate at
    // the retained coordinator boundary.
    let computed_features = match evaluated {
        geosolve_sketch::OperationOutcome::Completed {
            value: snapshot, ..
        } => snapshot
            .feature_evaluations()
            .iter()
            .map(|evaluation| match &evaluation.state {
                ComputedFeatureEvaluationState::Current { corner_edges } => {
                    AcceptedComputedFeatureEvidence {
                        feature: evaluation.feature.to_string(),
                        state: "current",
                        corner_edges: corner_edges
                            .iter()
                            .map(|(corner, edge)| {
                                [
                                    corner.to_string(),
                                    format!("{:016x}:{:08x}", edge.evaluation.raw(), edge.ordinal),
                                ]
                            })
                            .collect(),
                        diagnostic: None,
                    }
                }
                ComputedFeatureEvaluationState::Suppressed => AcceptedComputedFeatureEvidence {
                    feature: evaluation.feature.to_string(),
                    state: "suppressed",
                    corner_edges: Vec::new(),
                    diagnostic: None,
                },
                ComputedFeatureEvaluationState::Failed { failure } => {
                    AcceptedComputedFeatureEvidence {
                        feature: evaluation.feature.to_string(),
                        state: "failed",
                        corner_edges: Vec::new(),
                        diagnostic: Some(failure.to_string()),
                    }
                }
            })
            .collect(),
        stopped => {
            let diagnostic = format!(
                "computed-feature evaluation stopped: {:?}",
                stopped.report().stopping_reason
            );
            features
                .features()
                .iter()
                .map(|feature| AcceptedComputedFeatureEvidence {
                    feature: feature.id.to_string(),
                    state: "withheld",
                    corner_edges: Vec::new(),
                    diagnostic: Some(diagnostic.clone()),
                })
                .collect()
        }
    };

    let sketch_digest = lineage_content_digest(accepted_sketch_json.as_bytes());
    let computed_feature_json = features.to_json().map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "feature_evidence_error",
            "lineage-evaluation-feature-evidence-error",
            error.to_string(),
        )
    })?;
    let feature_digest = lineage_content_digest(computed_feature_json.as_bytes());
    let external_inputs = lineage_external_input_stamp(
        sketch_session.parameter_batch(),
        sketch_session.latest_attempt_external_snapshot_set(),
    )?;
    Ok(AcceptedCheckpointEvidence {
        external_inputs,
        retained_encoding,
        retained_sketch_json,
        accepted_encoding: if accepted_is_draft_v5 {
            "draft_v5"
        } else {
            "canonical_v4"
        },
        accepted_sketch_json,
        computed_feature_json,
        computed_features,
        sketch_digest,
        feature_digest,
    })
}

fn evaluate_sketch_prefix_with_exact_inputs(
    design: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    authored_parameters: &ParameterBatch,
    authored_snapshots: &ExternalSnapshotSet,
    previous_accepted: Option<&PreviousAcceptedCheckpoint>,
    native_fillet: Option<&geosolve_sketch::DocumentPreparedNativeLineFilletGeometry>,
) -> Result<RetainedSketchDocumentSession, LineageDomainEvaluationFailure> {
    let revisions = SketchLifecycleRevisionHighWater::from_raw(0, 0, None);
    let request = DocumentSolveRequest::default();

    if let (Some(previous), Some(request)) = (previous_accepted, native_fillet) {
        return evaluate_native_fillet_continuation(
            design,
            parameters,
            snapshots,
            authored_parameters,
            authored_snapshots,
            previous,
            request,
        );
    }

    // A projected owner rewrite deliberately reifies one independently accepted
    // solution into retained intent. Certify that exact solution before asking
    // the nonlinear solver to rediscover it: a fresh solve may return an equally
    // valid underconstrained solution with different floating-point bits. Native
    // Fillet transitions take the authenticated continuation path above first;
    // their materialized graph must never bypass reauthentication merely because
    // it is independently valid in isolation.
    if let Ok(exact) =
        RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
            design.clone(),
            design.clone(),
            revisions,
            parameters.clone(),
            snapshots.clone(),
            request,
            SolverConfig::default(),
        )
    {
        return Ok(exact);
    }

    if let Some(previous) = previous_accepted {
        let previous_design =
            decode_prefix_sketch(previous.retained_encoding, &previous.retained_sketch_json)?;
        let previous_accepted = decode_prefix_sketch(previous.encoding, &previous.sketch_json)?;
        let continuation = design
            .prepare_continuation_seed(&previous_design, &previous_accepted)
            .map_err(|error| sketch_evaluation_failure(error.to_string()))?;

        // The continuation seed is derived only from independently accepted
        // upstream evidence: unchanged retained leaves receive their exact
        // accepted values, while every newly authored or rewritten leaf keeps
        // the downstream action value. Certify it without optimization first.
        // This preserves an already-satisfied underconstrained prefix exactly
        // and cannot admit an arbitrary caller-supplied witness.
        if let Ok(exact) =
            RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
                design.clone(),
                continuation.clone(),
                revisions,
                parameters.clone(),
                snapshots.clone(),
                request,
                SolverConfig::default(),
            )
        {
            return Ok(exact);
        }

        // When the authored action genuinely requires movement, solve from the
        // authenticated continuation seed and then bind that independently
        // accepted result back to the retained downstream design.
        let continued = RetainedSketchDocumentSession::new_with_inputs(
            continuation,
            parameters.clone(),
            snapshots.clone(),
            request,
            SolverConfig::default(),
        )
        .map_err(|error| sketch_evaluation_failure(error.to_string()))?;
        let accepted = continued
            .accepted_state_for_current_input()
            .ok_or_else(|| {
                sketch_evaluation_failure(
                    "the authenticated ordinary continuation seed was not accepted",
                )
            })?
            .document()
            .clone();
        return RetainedSketchDocumentSession::restore_current_design_with_accepted_and_inputs(
            design.clone(),
            accepted,
            revisions,
            parameters.clone(),
            snapshots.clone(),
            request,
            SolverConfig::default(),
        )
        .map_err(|error| sketch_evaluation_failure(error.to_string()));
    }

    let session = RetainedSketchDocumentSession::new_with_inputs(
        design.clone(),
        parameters.clone(),
        snapshots.clone(),
        request,
        SolverConfig::default(),
    );
    session.map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "sketch_evaluation_error",
            "lineage-evaluation-sketch-error",
            error.to_string(),
        )
    })
}

fn evaluate_native_fillet_continuation(
    design: &SketchDocument,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    authored_parameters: &ParameterBatch,
    authored_snapshots: &ExternalSnapshotSet,
    previous: &PreviousAcceptedCheckpoint,
    prepared: &geosolve_sketch::DocumentPreparedNativeLineFilletGeometry,
) -> Result<RetainedSketchDocumentSession, LineageDomainEvaluationFailure> {
    let previous_design =
        decode_prefix_sketch(previous.retained_encoding, &previous.retained_sketch_json)?;
    let previous_accepted = decode_prefix_sketch(previous.encoding, &previous.sketch_json)?;
    // Host inputs can change without appending a lineage action. Reconstruct
    // the complete upstream prefix under the Fillet action's authenticated
    // authoring-time inputs before reauthenticating its frozen prepared plan;
    // the current host payload is a later reattempt, not permission to
    // reinterpret history against changed geometry.
    let authored_upstream =
        RetainedSketchDocumentSession::restore_current_design_with_accepted_and_distinct_inputs(
            previous_design.clone(),
            previous_accepted,
            SketchLifecycleRevisionHighWater::from_raw(0, 0, None),
            authored_parameters.clone(),
            authored_snapshots.clone(),
            previous.parameters.clone(),
            previous.snapshots.clone(),
            DocumentSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .map_err(|error| sketch_evaluation_failure(error.to_string()))?;
    let authored_upstream_accepted = authored_upstream
        .accepted_state_for_current_input()
        .ok_or_else(|| {
            sketch_evaluation_failure(
                "the native Fillet upstream prefix is not accepted under its authoring inputs",
            )
        })?;
    let accepted_seed = authored_upstream_accepted
        .document()
        .prepare_materialized_native_line_fillet_seed(&previous_design, design, prepared)
        .map_err(|error| sketch_evaluation_failure(error.to_string()))?;
    RetainedSketchDocumentSession::restore_current_design_with_accepted_and_distinct_inputs(
        design.clone(),
        accepted_seed,
        SketchLifecycleRevisionHighWater::from_raw(0, 0, None),
        parameters.clone(),
        snapshots.clone(),
        authored_parameters.clone(),
        authored_snapshots.clone(),
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .map_err(|error| sketch_evaluation_failure(error.to_string()))
}

fn decode_prefix_sketch(
    encoding: &str,
    json: &str,
) -> Result<SketchDocument, LineageDomainEvaluationFailure> {
    match encoding {
        "draft_v5" => SketchDocument::from_draft_v5_json(json),
        "canonical_v4" => SketchDocument::from_json(json),
        _ => {
            return Err(sketch_evaluation_failure(
                "the lineage prefix uses an unknown sketch encoding",
            ));
        }
    }
    .map_err(|error| sketch_evaluation_failure(error.to_string()))
}

fn sketch_evaluation_failure(message: impl Into<String>) -> LineageDomainEvaluationFailure {
    LineageDomainEvaluationFailure::new(
        "sketch_evaluation_error",
        "lineage-evaluation-sketch-error",
        message,
    )
}

struct PreviousAcceptedCheckpoint {
    retained_encoding: &'static str,
    retained_sketch_json: String,
    encoding: &'static str,
    sketch_json: String,
    parameters: ParameterBatch,
    snapshots: ExternalSnapshotSet,
}

/// Computes the canonical engine-owned identity for an exact pair of immutable
/// host-input payloads. Callers cannot supply or certify this stamp directly.
///
/// # Errors
///
/// Returns structured evidence if either supposedly validated host payload
/// cannot be re-encoded canonically.
pub fn lineage_external_input_stamp(
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<LineageOpaqueId, LineageDomainEvaluationFailure> {
    external_input_stamp(parameters, snapshots).map_err(|error| {
        LineageDomainEvaluationFailure::new(
            "host_input_invalid",
            "lineage-evaluation-host-input-invalid",
            error.to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
        DocumentEdit, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
        DocumentParameterKind, DocumentParameterTarget, DocumentSolveRequest, ExternalSnapshotSet,
        ParameterBatch, ParameterBatchEntry, ParameterValue, RetainedSketchDocumentSession,
        ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    };
    use geosolve_sketch_features::{
        ComputedEvaluationAllocatorHighWater, ComputedEvaluationRevision, ComputedFeatureDocument,
        ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
    };
    use geosolve_sketch_lineage::{
        LineageActionDefinition, LineageDocument, LineageEvaluationPolicy,
        LineageMaterializationMap, LineageMutation, LineagePatch, LineageSession,
        LineageStepRewrite,
    };

    use super::super::lineage::CoordinatorLineage;
    use super::{
        LINEAGE_DOMAIN_EVALUATOR, evaluate_lineage_session_cold,
        evaluate_lineage_session_cold_with_inputs, evaluate_strict_run,
        lineage_external_input_stamp,
    };
    use crate::RetainedEditorCoordinator;

    fn lineage_with_parallel_fillet_features(count: usize) -> LineageSession {
        let mut document = SketchDocument::new(10.0).expect("sketch");
        let first_start = document
            .add_point("first start", [0.0, 0.0])
            .expect("point");
        let first_end = document.add_point("first end", [4.0, 0.0]).expect("point");
        let second_start = document
            .add_point("second start", [0.0, 2.0])
            .expect("point");
        let second_end = document.add_point("second end", [4.0, 2.0]).expect("point");
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first line",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second line",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .expect("curve"),
        );
        let parent = |span, retained_endpoint| ComputedFilletParent {
            source: NativeCurveSpanSource { span },
            picked_parameter: 0.5,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            normal_side: DocumentCurveNormalSide::Left,
            retained_endpoint,
            periodic_anchor: None,
        };
        let corner = NewComputedFilletCorner {
            first: parent(first, DocumentFilletTrimEndpoint::End),
            second: parent(second, DocumentFilletTrimEndpoint::Start),
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
        };
        let mut features = ComputedFeatureDocument::new(document.id());
        for index in 0..count {
            features
                .create_fillet_set(format!("parallel feature {index}"), 0.5, vec![corner])
                .expect("persistent feature intent");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted native sketch");
        let coordinator = RetainedEditorCoordinator::with_features(session, features)
            .expect("computed disposition must not reject native lineage authority");
        LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session")
    }

    fn policy_pair(
        base: &LineageSession,
        mutations: Vec<LineageMutation>,
    ) -> (
        LineageSession,
        super::LineageDomainEvaluationEvidence,
        LineageSession,
        super::LineageDomainEvaluationEvidence,
    ) {
        let mut strict = base.clone();
        strict
            .apply_patch(LineagePatch::new(strict.identity(), mutations.clone()))
            .expect("strict edit class");
        let strict_evidence =
            evaluate_lineage_session_cold(&strict).expect("strict chronological oracle");

        let mut local = base.clone();
        let mut local_mutations = mutations;
        local_mutations.push(LineageMutation::SetEvaluationPolicy {
            policy: LineageEvaluationPolicy::DependencyLocal,
        });
        local
            .apply_patch(LineagePatch::new(local.identity(), local_mutations))
            .expect("dependency-local edit class");
        let local_evidence =
            evaluate_lineage_session_cold(&local).expect("dependency-local differential gate");

        assert_eq!(
            local_evidence.sketch_digest(),
            strict_evidence.sketch_digest()
        );
        assert_eq!(
            local_evidence.feature_digest(),
            strict_evidence.feature_digest()
        );
        assert_eq!(
            local_evidence.strict_prefix_digest(),
            strict_evidence.strict_prefix_digest()
        );
        assert_eq!(
            local_evidence.validated_prefix_count(),
            strict_evidence.validated_prefix_count()
        );
        assert_eq!(
            local_evidence.work().oracle_prefix_evaluation_count(),
            strict_evidence.validated_prefix_count()
        );
        assert_eq!(
            local_evidence.work().ready_prefix_count(),
            strict_evidence.validated_prefix_count()
        );
        assert_eq!(
            local_evidence.work().reconstructed_prefix_count()
                + local_evidence.work().replayed_suffix_count(),
            local_evidence.work().ready_prefix_count()
        );
        assert!(!local_evidence.work().dirty_steps().is_empty());

        let strict_map = LineageMaterializationMap::derive(strict.document()).expect("strict map");
        let local_map = LineageMaterializationMap::derive(local.document()).expect("local map");
        assert_eq!(strict_map.bindings(), local_map.bindings());
        assert_eq!(strict_map.reverse_bindings(), local_map.reverse_bindings());
        for step in strict.document().steps() {
            assert_eq!(
                strict_map.owned_outputs(step.id),
                local_map.owned_outputs(step.id)
            );
            assert_eq!(
                strict_map.live_owned_outputs(step.id),
                local_map.live_owned_outputs(step.id)
            );
        }

        (strict, strict_evidence, local, local_evidence)
    }

    fn action_rewrites(before: &LineageDocument, after: &LineageDocument) -> Vec<LineageMutation> {
        after
            .steps()
            .iter()
            .filter_map(|step| {
                let previous = before.step(step.id)?;
                (previous.label != step.label || previous.action != step.action).then(|| {
                    LineageMutation::Rewrite {
                        step: step.id,
                        replacement: Box::new(LineageStepRewrite {
                            label: step.label.clone(),
                            action: step.action.clone(),
                        }),
                    }
                })
            })
            .collect()
    }

    #[test]
    fn imported_workbench_lineage_is_cold_solved_and_independently_accepted() {
        let retained = RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("empty sketch"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        let lineage = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");

        let first = evaluate_lineage_session_cold(&lineage).expect("accepted cold evaluation");
        let second = evaluate_lineage_session_cold(&lineage).expect("repeated cold evaluation");
        assert_eq!(first, second);
        assert_eq!(first.lineage(), lineage.identity());
        assert_eq!(first.computed_feature_count(), 0);
        assert_eq!(first.validated_prefix_count(), 1);
        assert_ne!(first.materialization_digest().bytes(), [0; 32]);
        assert_ne!(first.strict_prefix_digest().bytes(), [0; 32]);
        assert_eq!(
            LINEAGE_DOMAIN_EVALUATOR,
            "geosolve.constraint-editor.lineage-cold.v1"
        );
    }

    #[test]
    fn cold_evidence_authenticates_failed_feature_without_rejecting_native_authority() {
        let lineage = lineage_with_parallel_fillet_features(1);
        let evidence = evaluate_lineage_session_cold(&lineage)
            .expect("a truthful Failed feature remains accepted lineage intent");
        assert_eq!(evidence.computed_feature_count(), 1);
        let strict = evaluate_strict_run(
            &CoordinatorLineage::from_session_json(
                &lineage
                    .to_canonical_session_json()
                    .expect("canonical session"),
            )
            .expect("coordinator lineage"),
            &ParameterBatch::default(),
            &ExternalSnapshotSet::default(),
        )
        .expect("strict evidence");
        assert_eq!(strict.accepted.computed_features[0].state, "failed");
        assert!(strict.accepted.computed_features[0].diagnostic.is_some());
    }

    #[test]
    fn revision_local_computed_allocator_exhaustion_does_not_poison_lineage_authority() {
        let document = SketchDocument::new(10.0).expect("empty sketch");
        let features = ComputedFeatureDocument::new(document.id());
        let feature_lifecycle = features.lifecycle_high_water();
        let retained = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let exhausted = ComputedEvaluationAllocatorHighWater {
            next_revision: ComputedEvaluationRevision::from_raw(u64::MAX),
        };
        let coordinator = RetainedEditorCoordinator::with_features_and_high_water(
            retained,
            features,
            feature_lifecycle,
            exhausted,
        )
        .expect("revision-local exhaustion must not reject lineage construction");
        assert_eq!(
            coordinator
                .persistence_checkpoint()
                .expect("coordinator checkpoint")
                .computed_evaluation_high_water(),
            exhausted,
            "the host allocator remains exact lifecycle state even though cold validation uses scratch IDs",
        );

        let lineage = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");
        let evidence = evaluate_lineage_session_cold(&lineage)
            .expect("scratch computed identities reproduce accepted lineage");
        assert_eq!(evidence.computed_feature_count(), 0);
        assert_eq!(evidence.validated_prefix_count(), 1);
    }

    #[test]
    fn exact_nondefault_host_inputs_drive_cold_evaluation_and_evidence_identity() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let batch = |revision, width| {
            ParameterBatch::new(
                revision,
                vec![ParameterBatchEntry {
                    parameter,
                    value: ParameterValue::Length(width),
                }],
            )
            .expect("parameter batch")
        };
        let first_batch = batch(1, 4.0);
        let retained = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            first_batch.clone(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        let lineage = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");
        let snapshots = ExternalSnapshotSet::default();

        let first = evaluate_lineage_session_cold_with_inputs(&lineage, &first_batch, &snapshots)
            .expect("first exact-input evaluation");
        assert_eq!(
            first.external_inputs(),
            &lineage_external_input_stamp(&first_batch, &snapshots)
                .expect("first host-input identity")
        );

        let second_batch = batch(2, 6.0);
        let second = evaluate_lineage_session_cold_with_inputs(&lineage, &second_batch, &snapshots)
            .expect("second exact-input evaluation");
        assert_eq!(
            second.external_inputs(),
            &lineage_external_input_stamp(&second_batch, &snapshots)
                .expect("second host-input identity")
        );
        assert_ne!(first.external_inputs(), second.external_inputs());
        assert_ne!(
            first.materialization_digest(),
            second.materialization_digest()
        );
        assert_ne!(first.sketch_digest(), second.sketch_digest());
        assert_eq!(first.lineage(), second.lineage());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the rejection regression keeps the exact prefix, owner attribution, and retained authority evidence in one scenario"
    )]
    fn strict_prefix_oracle_attributes_early_domain_rejection_to_its_owner_step() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let parameter = document
            .add_parameter("width input", DocumentParameterKind::Length)
            .expect("parameter");
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .expect("parameter binding");
        let retained = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            ParameterBatch::new(
                1,
                vec![ParameterBatchEntry {
                    parameter,
                    value: ParameterValue::Length(4.0),
                }],
            )
            .expect("initial parameter batch"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let mut coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "later point".into(),
                    position: [8.0, 5.0],
                },
            )
            .expect("later authored step");
        let mut lineage = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");
        assert!(lineage.document().steps().len() >= 2);
        let baseline = lineage.document().steps()[0].id;
        let later = lineage.document().steps().last().expect("later step").id;

        // Preserve the attribution regression with a structurally authentic
        // but domain-insufficient exact historical input on the baseline. The
        // ordinary imported lineage now correctly retains the original bound
        // parameter, so evaluating with an empty *final* payload alone is no
        // longer an early-prefix failure.
        let baseline_step = lineage.document().steps()[0].clone();
        let mut replacement_action = baseline_step.action.clone();
        let LineageActionDefinition::ImportedBaseline {
            baseline: baseline_action,
        } = &mut replacement_action
        else {
            panic!("first workbench step must be its imported baseline");
        };
        let empty_parameters = ParameterBatch::default();
        let empty_snapshots = ExternalSnapshotSet::default();
        let mut payload = serde_json::from_str::<serde_json::Value>(&baseline_action.payload)
            .expect("baseline bridge payload");
        let host_inputs = payload
            .get_mut("host_inputs")
            .and_then(serde_json::Value::as_object_mut)
            .expect("baseline exact host-input provenance");
        host_inputs.insert(
            "parameter_batch_json".into(),
            serde_json::Value::String(
                empty_parameters
                    .to_canonical_json()
                    .expect("canonical empty parameter batch"),
            ),
        );
        host_inputs.insert(
            "external_snapshot_set_json".into(),
            serde_json::Value::String(
                empty_snapshots
                    .to_canonical_json()
                    .expect("canonical empty external snapshots"),
            ),
        );
        host_inputs.insert(
            "external_inputs".into(),
            serde_json::to_value(
                lineage_external_input_stamp(&empty_parameters, &empty_snapshots)
                    .expect("empty exact-input stamp"),
            )
            .expect("host-input stamp value"),
        );
        baseline_action.payload =
            serde_json::to_string(&payload).expect("rewritten baseline payload");
        lineage
            .apply_patch(LineagePatch::new(
                lineage.identity(),
                vec![LineageMutation::Rewrite {
                    step: baseline,
                    replacement: Box::new(LineageStepRewrite {
                        label: baseline_step.label,
                        action: replacement_action,
                    }),
                }],
            ))
            .expect("authenticated early-prefix input rewrite");

        let failure = evaluate_lineage_session_cold(&lineage)
            .expect_err("the missing required parameter must reject the first prefix");
        assert_eq!(failure.code(), "sketch_evaluation_rejected");
        assert_eq!(failure.failed_step(), Some(baseline));
        assert_ne!(failure.failed_step(), Some(later));

        let mut local = lineage.clone();
        local
            .apply_patch(LineagePatch::new(
                local.identity(),
                vec![LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::DependencyLocal,
                }],
            ))
            .expect("dependency-local failure policy");
        let local_failure = evaluate_lineage_session_cold(&local)
            .expect_err("dependency-local evaluation must preserve strict failure authority");
        assert_eq!(local_failure, failure);
    }

    #[test]
    fn dependency_local_policy_matches_the_strict_prefix_oracle() {
        let retained = RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("empty sketch"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let mut coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "authored point".into(),
                    position: [3.0, 4.0],
                },
            )
            .expect("later authored step");
        let strict = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");
        let strict_evidence =
            evaluate_lineage_session_cold(&strict).expect("strict prefix evaluation");
        assert_eq!(strict_evidence.validated_prefix_count(), 2);

        let mut local = strict.clone();
        local
            .apply_patch(LineagePatch::new(
                local.identity(),
                vec![LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::DependencyLocal,
                }],
            ))
            .expect("dependency-local policy");
        let local_evidence =
            evaluate_lineage_session_cold(&local).expect("dependency-local evaluation");
        assert_eq!(
            local_evidence.validated_prefix_count(),
            strict_evidence.validated_prefix_count()
        );
        assert_eq!(
            local_evidence.strict_prefix_digest(),
            strict_evidence.strict_prefix_digest()
        );
        assert_eq!(
            local_evidence.sketch_digest(),
            strict_evidence.sketch_digest()
        );
        assert_eq!(
            local_evidence.feature_digest(),
            strict_evidence.feature_digest()
        );
        assert_eq!(
            local_evidence.work().policy(),
            LineageEvaluationPolicy::DependencyLocal
        );
        assert_eq!(local_evidence.work().ready_prefix_count(), 2);
        assert_eq!(local_evidence.work().reusable_prefix_count(), 2);
        assert_eq!(local_evidence.work().reconstructed_prefix_count(), 2);
        assert_eq!(local_evidence.work().replayed_suffix_count(), 0);
        assert_eq!(local_evidence.work().policy_prefix_evaluation_count(), 1);
        assert_eq!(local_evidence.work().oracle_prefix_evaluation_count(), 2);
        assert_eq!(local_evidence.work().total_prefix_evaluation_count(), 3);
        assert!(local_evidence.work().dirty_steps().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one differential matrix keeps every W7 edit/history class against the same accepted lineage and immutable host inputs"
    )]
    fn dependency_local_dirty_closure_matches_strict_for_every_w7_edit_class() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document
            .add_point("arc center", [0.0, 0.0])
            .expect("center");
        let radius = document
            .add_scalar(
                "arc radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("radius");
        let start = document
            .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
            .expect("start");
        let end = document
            .add_scalar(
                "arc end",
                std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .expect("end");
        let arc = document
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle: start,
                    end_angle: end,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .expect("arc");
        let retained = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let mut coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        for (label, position) in [
            ("first independent point", [4.0, 1.0]),
            ("second independent point", [5.0, 2.0]),
            ("third independent point", [6.0, 3.0]),
        ] {
            coordinator
                .apply_edit(
                    coordinator.session().design_identity(),
                    DocumentEdit::CreatePoint {
                        label: label.into(),
                        position,
                    },
                )
                .expect("independent point action");
        }
        let base = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("accepted base lineage"),
        )
        .expect("base session");
        assert_eq!(base.document().steps().len(), 4);
        let first = base.document().steps()[1].id;
        let second = base.document().steps()[2].id;
        let third = base.document().steps()[3].id;

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetArcSweep {
                    curve: arc,
                    sweep: DocumentArcSweep::Clockwise,
                },
            )
            .expect("explicit branch/property edit");
        let branch_after = LineageSession::from_session_json(
            &coordinator.lineage_session_json().expect("branch lineage"),
        )
        .expect("branch session");
        let branch_mutations = action_rewrites(base.document(), branch_after.document());
        assert!(!branch_mutations.is_empty());
        coordinator
            .undo()
            .expect("restore base before insert fixture");

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreatePoint {
                    label: "inserted point".into(),
                    position: [8.0, 4.0],
                },
            )
            .expect("insert fixture");
        let insert_after = LineageSession::from_session_json(
            &coordinator.lineage_session_json().expect("insert lineage"),
        )
        .expect("insert session");
        let mut insert_mutations = action_rewrites(base.document(), insert_after.document());
        let inserted = insert_after
            .document()
            .steps()
            .iter()
            .find(|step| base.document().step(step.id).is_none())
            .expect("one inserted action")
            .clone();
        insert_mutations.push(LineageMutation::Insert {
            before: None,
            step: Box::new(inserted),
        });

        let rewrite = vec![LineageMutation::Rewrite {
            step: second,
            replacement: Box::new(LineageStepRewrite {
                label: "rewritten independent point".into(),
                action: base
                    .document()
                    .step(second)
                    .expect("rewrite owner")
                    .action
                    .clone(),
            }),
        }];
        let cases = [
            ("insert", insert_mutations),
            ("rewrite", rewrite.clone()),
            (
                "suppress",
                vec![LineageMutation::SetSuppressed {
                    step: second,
                    suppressed: true,
                }],
            ),
            (
                "delete",
                vec![LineageMutation::DeleteSubtree { root: second }],
            ),
            (
                "reorder",
                vec![LineageMutation::Reorder {
                    step: third,
                    before: Some(first),
                }],
            ),
            ("branch/property", branch_mutations),
        ];
        for (case, mutations) in cases {
            let (_, strict, _, local) = policy_pair(&base, mutations);
            assert!(
                local.work().policy_prefix_evaluation_count()
                    <= strict.work().policy_prefix_evaluation_count(),
                "{case} must not perform more policy-local prefix solves than strict"
            );
        }

        let mut history = base.clone();
        history
            .apply_patch(LineagePatch::new(
                history.identity(),
                vec![LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::DependencyLocal,
                }],
            ))
            .expect("local history base");
        let base_evidence =
            evaluate_lineage_session_cold(&history).expect("accept local history base");
        history
            .accept_current(
                history.identity(),
                Some(base_evidence.external_inputs().clone()),
                base_evidence.materialization_digest(),
            )
            .expect("publish local history base");
        history
            .apply_patch(LineagePatch::new(history.identity(), rewrite))
            .expect("local history rewrite");
        let edited = evaluate_lineage_session_cold(&history).expect("local history edit");
        history
            .accept_current(
                history.identity(),
                Some(edited.external_inputs().clone()),
                edited.materialization_digest(),
            )
            .expect("publish local history edit");

        history.undo().expect("undo").expect("undo position");
        let undone = evaluate_lineage_session_cold(&history).expect("local Undo evaluation");
        assert_eq!(undone.work().reusable_prefix_count(), 4);
        assert_eq!(undone.work().policy_prefix_evaluation_count(), 1);
        history
            .accept_current(
                history.identity(),
                Some(undone.external_inputs().clone()),
                undone.materialization_digest(),
            )
            .expect("publish Undo");

        history.redo().expect("redo").expect("redo position");
        let redone = evaluate_lineage_session_cold(&history).expect("local Redo evaluation");
        assert_eq!(redone.sketch_digest(), edited.sketch_digest());
        assert_eq!(redone.feature_digest(), edited.feature_digest());
        assert_eq!(redone.work().reusable_prefix_count(), 4);
        assert_eq!(redone.work().policy_prefix_evaluation_count(), 1);
    }

    #[test]
    fn strict_prefixes_accept_host_parameters_introduced_by_later_steps() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let rectangle = document
            .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
            .expect("rectangle");
        let retained = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session");
        let mut coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained editor coordinator");
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateParameter {
                    label: "later width".into(),
                    kind: DocumentParameterKind::Length,
                },
            )
            .expect("parameter declaration");
        let parameter = coordinator
            .session()
            .design_document()
            .parameters()
            .iter()
            .find(|parameter| parameter.label == "later width")
            .expect("parameter identity")
            .id;
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::AddParameterBinding {
                    parameter,
                    target: DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
                },
            )
            .expect("parameter binding");
        let batch = ParameterBatch::new(
            1,
            vec![ParameterBatchEntry {
                parameter,
                value: ParameterValue::Length(5.0),
            }],
        )
        .expect("host parameter batch");
        coordinator
            .replace_parameter_batch(
                coordinator.session().design_identity(),
                batch.clone(),
                DocumentSolveRequest::default(),
            )
            .expect("host input publication");
        let lineage = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical lineage session"),
        )
        .expect("strict lineage session");

        let evidence = evaluate_lineage_session_cold_with_inputs(
            &lineage,
            &batch,
            &ExternalSnapshotSet::default(),
        )
        .expect("later parameter must not invalidate an earlier prefix");
        assert_eq!(
            evidence.validated_prefix_count(),
            lineage.document().steps().len()
        );
    }

    #[test]
    fn dependency_valid_but_nonmaterializable_lineage_fails_closed() {
        let lineage = LineageSession::new(LineageDocument::new()).expect("empty lineage session");
        let failure = evaluate_lineage_session_cold(&lineage)
            .expect_err("a lineage without an imported workbench baseline cannot be accepted");
        assert_eq!(failure.code(), "workbench_materialization_unsupported");
        assert_eq!(
            failure.diagnostic(),
            "lineage-evaluation-materialization-unsupported"
        );
    }
}
