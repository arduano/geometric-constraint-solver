// SPDX-License-Identifier: GPL-3.0-or-later

//! Authoritative lineage/materialization bridge for the retained editor.
//!
//! Lineage actions keep semantic parameters and stable logical identity. The
//! bridge also retains an equation-free structural delta for each accepted
//! action. Cold evaluation applies those deltas to an honest imported root,
//! then hands the resulting ordinary documents back to their owning domain
//! validators and solvers.

mod host_inputs;
mod intent;

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use geosolve_sketch_lineage::{
    ImportedBaselineAction, ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind,
    LineageAuxiliaryHighWater, LineageDeveloperKey, LineageDigest, LineageDocument,
    LineageDocumentId, LineageDocumentIdentity, LineageEvaluationDisposition, LineageInputBinding,
    LineageMaterializationMap, LineageMaterializationMapError, LineageMaterializedIdentity,
    LineageMaterializedLeaf, LineageMutation, LineageOpaqueId, LineageOutput, LineageOutputId,
    LineageOutputIdentity, LineageOutputIdentityFlow, LineageOutputKind, LineageOutputRef,
    LineagePatch, LineageReservation, LineageReservationId, LineageReservationKind,
    LineageSemanticKey, LineageSession, LineageStep, LineageStepId, LineageStepRewrite,
    LineageWritableLeaf, VersionedActionPayload, lineage_content_digest,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use super::history::checkpoint;
use super::lineage_evaluation::LineageDomainEvaluationEvidence;
use super::{
    CoordinatorError, ExternalSnapshotSet, MutationOutcome, OperationOutcome, ParameterBatch,
    ReplayAction, RestoreCheckpoint, RetainedEditorCoordinator, bounded_geometry_control,
    evaluate_computed_features,
};
use crate::{GeometryToolVariant, SelectionItem};
use host_inputs::{DecodedLineageHostInputs, LineageHostInputLedger, LineageHostInputProvenance};
use intent::compile_replay_intent;

use geosolve_sketch_ops::{
    LineEndpoint, SketchOperationApplication, SketchOperationProposal, SketchOperationRequest,
    SketchProfileOffsetOperand, SplitRetainedPiece, TrimRetainedSide,
};
use geosolve_sketch_topology::{OffsetDirectedSpan, OffsetFaceKey, OffsetTraversal};

const BASELINE_MEDIA_TYPE: &str = "application/vnd.geosolve.workbench-imported-baseline+json";
const AUTHORED_INTENT_PARAMETER: &str = "authored_intent";
const AUTHORED_OWNER_FIELDS_PARAMETER: &str = "authored_owner_fields";
const AUTHORED_OUTPUT_FIELD_MANIFEST: &str = "owned_output_field_manifest";
const HOST_INPUT_PROVENANCE_PARAMETER: &str = "host_input_provenance";
const MATERIALIZATION_PARAMETER: &str = "compiled_materialization";
const MAX_MATERIALIZATION_PATCH_OPERATIONS: usize = 262_144;
const MAX_MATERIALIZATION_PATH_DEPTH: usize = 64;
const COMPUTED_EVALUATION_HIGH_WATER_KEY: &str = "computed-evaluation-next-revision";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CompiledMaterializationRecipe {
    version: u32,
    intent_digest: LineageDigest,
    operations: Vec<StructuralDeltaOperation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthoredOutputFieldValue {
    output: LineageOutputId,
    kind: LineageOutputKind,
    field: LineageSemanticKey,
    value: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthoredOutputFieldKey {
    output: LineageOutputId,
    kind: LineageOutputKind,
    field: LineageSemanticKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MaterializedEntity {
    persistent_id: String,
    key: String,
    output_kind: LineageOutputKind,
    reservation_kind: Option<LineageReservationKind>,
}

#[derive(Clone, Debug, Default)]
struct WritableLeafCatalog {
    materialized: BTreeMap<(LineageOutputKind, String), BTreeSet<LineageSemanticKey>>,
    logical: BTreeMap<(LineageOutputKind, String), BTreeSet<LineageSemanticKey>>,
}

impl WritableLeafCatalog {
    fn keys_for(&self, entity: &MaterializedEntity) -> impl Iterator<Item = &LineageSemanticKey> {
        let key = (entity.output_kind, entity.persistent_id.clone());
        entity.reservation_kind.map_or_else(
            || {
                self.logical
                    .get(&key)
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .into_iter()
            },
            |_| {
                self.materialized
                    .get(&key)
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .into_iter()
            },
        )
    }
}

#[derive(Clone, Debug)]
struct StepManifest {
    inputs: Vec<LineageInputBinding>,
    outputs: Vec<LineageOutput>,
    identities: Vec<LineageOutputIdentity>,
    writable_leaves: Vec<LineageWritableLeaf>,
    reservations: Vec<LineageReservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImmutableActionBinding {
    kind: LineageActionKind,
    schema: LineageSemanticKey,
    version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImmutableStepBinding {
    key: LineageDeveloperKey,
    action: ImmutableActionBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImmutableOutputBinding {
    step: LineageStepId,
    key: LineageSemanticKey,
    kind: LineageOutputKind,
    reservation: Option<LineageReservationId>,
    flow: LineageOutputIdentityFlow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImmutableReservationBinding {
    step: LineageStepId,
    key: LineageSemanticKey,
    kind: LineageReservationKind,
    persistent_id: LineageOpaqueId,
}

#[derive(Default)]
struct SessionIdentityLedger {
    steps: BTreeMap<(LineageDocumentId, LineageStepId), ImmutableStepBinding>,
    outputs: BTreeMap<(LineageDocumentId, LineageOutputId), ImmutableOutputBinding>,
    reservations: BTreeMap<(LineageDocumentId, LineageReservationId), ImmutableReservationBinding>,
    persistent_ids: BTreeMap<
        (LineageDocumentId, LineageReservationKind, LineageOpaqueId),
        LineageReservationId,
    >,
}

struct PendingDirectStepRewrite {
    replacement: LineageStepRewrite,
    owner_changes: Vec<(StructuralTargetKey, StructuralDeltaOperation)>,
    owner_fields: BTreeMap<
        (LineageOutputId, LineageOutputKind, LineageSemanticKey),
        AuthoredOutputFieldValue,
    >,
    cache_changes: Vec<(StructuralTargetKey, StructuralDeltaOperation)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportedCoordinatorBaseline {
    version: u32,
    checkpoint: LineageCheckpoint,
    host_inputs: LineageHostInputProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_host_inputs: Option<LineageHostInputProvenance>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageSketchRevisionHighWater {
    design: u64,
    attempt: u64,
    accepted: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LineageCheckpoint {
    design_json: String,
    design_is_draft_v5: bool,
    accepted_json: Option<String>,
    accepted_is_draft_v5: bool,
    accepted_belongs_to_current_design: bool,
    feature_json: String,
    revisions: LineageSketchRevisionHighWater,
    sketch_identity_high_water: super::SketchPersistentIdentityHighWater,
    feature_lifecycle: geosolve_sketch_features::ComputedFeatureLifecycleHighWater,
    evaluation_allocator: geosolve_sketch_features::ComputedEvaluationAllocatorHighWater,
}

impl LineageCheckpoint {
    fn capture(checkpoint: &RestoreCheckpoint) -> Self {
        Self {
            design_json: checkpoint.design_json().to_owned(),
            design_is_draft_v5: checkpoint.design_uses_draft_v5(),
            accepted_json: checkpoint.accepted_json().map(str::to_owned),
            accepted_is_draft_v5: checkpoint.accepted_uses_draft_v5(),
            accepted_belongs_to_current_design: checkpoint.accepted_belongs_to_current_design(),
            feature_json: checkpoint.feature_json().to_owned(),
            revisions: LineageSketchRevisionHighWater {
                design: checkpoint.revisions().design().get(),
                attempt: checkpoint.revisions().attempt().get(),
                accepted: checkpoint
                    .revisions()
                    .accepted()
                    .map(geosolve_sketch::SketchAcceptedRevision::get),
            },
            sketch_identity_high_water: checkpoint.sketch_identity_high_water().clone(),
            feature_lifecycle: checkpoint.feature_lifecycle_high_water(),
            evaluation_allocator: checkpoint.computed_evaluation_high_water(),
        }
    }

    fn structural_value(&self) -> Result<Value, LineageBridgeError> {
        let document =
            normalized_sketch_document_value(&self.design_json, self.design_is_draft_v5)?;
        Ok(serde_json::json!({
            "design": {
                "document": document,
            },
            "features": serde_json::from_str::<Value>(&self.feature_json)?,
            "lifecycle": {
                "sketch_revisions": {
                    "design": self.revisions.design,
                    "attempt": self.revisions.attempt,
                    "accepted": self.revisions.accepted,
                },
                "sketch_identity_high_water": self.sketch_identity_high_water,
                "feature": self.feature_lifecycle,
                "computed_evaluation": self.evaluation_allocator,
            },
        }))
    }

    fn replace_structural_value(&mut self, value: &Value) -> Result<(), LineageBridgeError> {
        let object = value
            .as_object()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization root must be an object",
            ))?;
        let design = object.get("design").and_then(Value::as_object).ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization root is missing design state",
            ),
        )?;
        let normalized_design =
            design
                .get("document")
                .ok_or(LineageBridgeError::InvalidMaterialization(
                    "materialization design document is missing",
                ))?;
        let normalized_design_json = serde_json::to_string(normalized_design)?;
        let design_document =
            geosolve_sketch::SketchDocument::from_draft_v5_json(&normalized_design_json)?;
        if let Ok(json) = design_document.to_canonical_json() {
            self.design_is_draft_v5 = false;
            self.design_json = json;
        } else {
            self.design_is_draft_v5 = true;
            self.design_json = design_document.to_draft_v5_json()?;
        }
        self.feature_json = serde_json::to_string(object.get("features").ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization feature document is missing",
            ),
        )?)?;
        let lifecycle = object.get("lifecycle").and_then(Value::as_object).ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization lifecycle metadata is missing",
            ),
        )?;
        let revisions = lifecycle
            .get("sketch_revisions")
            .and_then(Value::as_object)
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization sketch revisions are missing",
            ))?;
        let design_revision = revisions.get("design").and_then(Value::as_u64).ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization design revision is invalid",
            ),
        )?;
        let attempt_revision = revisions.get("attempt").and_then(Value::as_u64).ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization attempt revision is invalid",
            ),
        )?;
        let accepted_revision = revisions.get("accepted").map_or(Ok(None), |value| {
            if value.is_null() {
                Ok(None)
            } else {
                value
                    .as_u64()
                    .map(Some)
                    .ok_or(LineageBridgeError::InvalidMaterialization(
                        "materialization accepted revision is invalid",
                    ))
            }
        })?;
        self.revisions = LineageSketchRevisionHighWater {
            design: design_revision,
            attempt: attempt_revision,
            accepted: accepted_revision,
        };
        self.sketch_identity_high_water =
            serde_json::from_value(lifecycle.get("sketch_identity_high_water").cloned().ok_or(
                LineageBridgeError::InvalidMaterialization(
                    "materialization sketch identity high-water is missing",
                ),
            )?)?;
        self.feature_lifecycle = serde_json::from_value(lifecycle.get("feature").cloned().ok_or(
            LineageBridgeError::InvalidMaterialization(
                "materialization feature lifecycle is missing",
            ),
        )?)?;
        self.evaluation_allocator =
            serde_json::from_value(lifecycle.get("computed_evaluation").cloned().ok_or(
                LineageBridgeError::InvalidMaterialization(
                    "materialization computed-evaluation allocator is missing",
                ),
            )?)?;
        self.apply_feature_lifecycle()?;
        self.canonicalize_persistent_documents()?;
        // Accepted data is evaluator-owned evidence for the complete lineage
        // revision and is deliberately absent from the structural delta. The
        // imported root is refreshed after every recorded evaluation, so
        // applying semantic operations must carry that exact cache evidence
        // through to the final materialization rather than discarding it.
        Ok(())
    }

    fn apply_feature_lifecycle(&mut self) -> Result<(), LineageBridgeError> {
        let mut feature_root = serde_json::json!({
            "features": serde_json::from_str::<Value>(&self.feature_json)?,
        });
        let features = feature_root
            .get_mut("features")
            .and_then(Value::as_object_mut)
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization feature document is invalid",
            ))?;
        features.insert(
            "revision".into(),
            serde_json::to_value(self.feature_lifecycle.revision)?,
        );
        features.insert(
            "next_feature_id".into(),
            serde_json::to_value(self.feature_lifecycle.allocator.next_feature_id)?,
        );
        features.insert(
            "next_corner_id".into(),
            serde_json::to_value(self.feature_lifecycle.allocator.next_corner_id)?,
        );
        refresh_computed_feature_digest(&mut feature_root)?;
        self.feature_json = serde_json::to_string(
            feature_root
                .get("features")
                .expect("feature root was constructed above"),
        )?;
        Ok(())
    }

    fn canonicalize_persistent_documents(&mut self) -> Result<(), LineageBridgeError> {
        let design = if self.design_is_draft_v5 {
            geosolve_sketch::SketchDocument::from_draft_v5_json(&self.design_json)?
        } else {
            geosolve_sketch::SketchDocument::from_json(&self.design_json)?
        };
        self.design_json = if self.design_is_draft_v5 {
            design.to_draft_v5_json()?
        } else {
            design.to_canonical_json()?
        };
        let features =
            geosolve_sketch_features::ComputedFeatureDocument::from_json(&self.feature_json)?;
        self.feature_json = features.to_json()?;
        Ok(())
    }

    fn retain_lifecycle_high_water(&mut self, retained: &Self) -> Result<(), LineageBridgeError> {
        self.revisions.design = self.revisions.design.max(retained.revisions.design);
        self.revisions.attempt = self.revisions.attempt.max(retained.revisions.attempt);
        self.revisions.accepted = match (self.revisions.accepted, retained.revisions.accepted) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };

        self.sketch_identity_high_water = self
            .sketch_identity_high_water
            .merged(&retained.sketch_identity_high_water)?;
        let mut design = if self.design_is_draft_v5 {
            geosolve_sketch::SketchDocument::from_draft_v5_json(&self.design_json)?
        } else {
            geosolve_sketch::SketchDocument::from_json(&self.design_json)?
        };
        design.retain_persistent_identity_high_water(&self.sketch_identity_high_water)?;
        self.design_json = if self.design_is_draft_v5 {
            design.to_draft_v5_json()?
        } else {
            design.to_canonical_json()?
        };

        // Feature revision and native feature allocators are exact program
        // state at this lineage history position. Only computed-evaluation IDs
        // are session-global auxiliary high-water. The baseline carries this
        // exact lifecycle beside semantic feature deltas; cold materialization
        // reapplies it after those deltas instead of inventing a restore bump.
        self.feature_lifecycle = retained.feature_lifecycle;
        self.apply_feature_lifecycle()?;
        self.canonicalize_persistent_documents()?;

        self.evaluation_allocator.next_revision = self
            .evaluation_allocator
            .next_revision
            .max(retained.evaluation_allocator.next_revision);
        Ok(())
    }

    fn retain_every_lifecycle_cursor(&mut self, retained: &Self) -> Result<(), LineageBridgeError> {
        self.revisions.design = self.revisions.design.max(retained.revisions.design);
        self.revisions.attempt = self.revisions.attempt.max(retained.revisions.attempt);
        self.revisions.accepted = match (self.revisions.accepted, retained.revisions.accepted) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
        self.sketch_identity_high_water = self
            .sketch_identity_high_water
            .merged(&retained.sketch_identity_high_water)?;
        self.feature_lifecycle = super::merge_feature_lifecycle_high_water(
            self.feature_lifecycle,
            retained.feature_lifecycle,
        );
        self.evaluation_allocator.next_revision = self
            .evaluation_allocator
            .next_revision
            .max(retained.evaluation_allocator.next_revision);
        Ok(())
    }

    fn retain_computed_evaluation_high_water(&mut self, retained: LineageAuxiliaryHighWater) {
        self.evaluation_allocator.next_revision = self
            .evaluation_allocator
            .next_revision
            .max(geosolve_sketch_features::ComputedEvaluationRevision::from_raw(retained.raw()));
    }

    fn replace_materialization_evidence(&mut self, evaluated: &Self) {
        // Accepted output is evaluator-owned cache evidence, not authored
        // action state. Keeping the exact bytes beside each retained lineage
        // history position lets Undo/Redo authenticate and reuse an exact
        // accepted scene (or retain an older accepted scene beside a rejected
        // design) without placing solver output in semantic action payloads.
        self.accepted_json.clone_from(&evaluated.accepted_json);
        self.accepted_is_draft_v5 = evaluated.accepted_is_draft_v5;
        self.accepted_belongs_to_current_design = evaluated.accepted_belongs_to_current_design;
    }

    fn embedded_accepted_branch(&self) -> Result<Option<Self>, LineageBridgeError> {
        let Some(accepted_json) = self.accepted_json.as_ref() else {
            return Ok(None);
        };
        let mut accepted = self.clone();
        accepted.design_json.clone_from(accepted_json);
        accepted.design_is_draft_v5 = self.accepted_is_draft_v5;
        accepted.accepted_json = Some(accepted_json.clone());
        accepted.accepted_is_draft_v5 = self.accepted_is_draft_v5;
        accepted.accepted_belongs_to_current_design = true;
        accepted.canonicalize_persistent_documents()?;
        Ok(Some(accepted))
    }

    pub(super) fn into_restore_checkpoint(self) -> RestoreCheckpoint {
        RestoreCheckpoint {
            design_json: self.design_json,
            design_is_draft_v5: self.design_is_draft_v5,
            accepted_json: self.accepted_json,
            accepted_is_draft_v5: self.accepted_is_draft_v5,
            accepted_belongs_to_current_design: self.accepted_belongs_to_current_design,
            revisions: super::SketchLifecycleRevisionHighWater::from_raw(
                self.revisions.design,
                self.revisions.attempt,
                self.revisions.accepted,
            ),
            sketch_identity_high_water: self.sketch_identity_high_water,
            feature_json: self.feature_json,
            feature_lifecycle: self.feature_lifecycle,
            evaluation_allocator: self.evaluation_allocator,
        }
    }

    pub(super) fn semantically_matches(
        &self,
        checkpoint: &RestoreCheckpoint,
    ) -> Result<bool, LineageBridgeError> {
        Ok(semantic_structural_value(self)?
            == semantic_structural_value(&Self::capture(checkpoint))?)
    }
}

fn normalized_sketch_document_value(
    json: &str,
    is_draft_v5: bool,
) -> Result<Value, LineageBridgeError> {
    let document = serde_json::from_str::<Value>(json)?;
    if is_draft_v5 {
        return Ok(document);
    }
    // The persistent sketch codec deliberately chooses canonical-v4 whenever
    // the current document fits that closed language. Lineage comparisons need
    // one semantic shape across a later transition into the extended draft-v5
    // side tables, otherwise one role or host-binding edit looks like removal
    // and reinsertion of the complete sketch.
    Ok(serde_json::json!({
        "version": 5,
        "document": document,
        "geometry_roles": [],
        "user_inactive_elements": [],
        "host_activation": null,
        "parameters": [],
        "parameter_bindings": [],
        "parameter_outputs": [],
        "external_bindings": [],
    }))
}

fn semantic_accepted_sketch_value(
    json: &str,
    is_draft_v5: bool,
) -> Result<Value, LineageBridgeError> {
    let mut value = normalized_sketch_document_value(json, is_draft_v5)?;
    let document = value
        .as_object_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "accepted sketch materialization must be an object",
        ))?;
    strip_design_allocator_fields(document)?;
    Ok(value)
}

fn accepted_materialization_semantically_matches(
    expected_json: &str,
    expected_is_draft_v5: bool,
    evaluated_json: &str,
    evaluated_is_draft_v5: bool,
) -> Result<bool, LineageBridgeError> {
    Ok(
        semantic_accepted_sketch_value(expected_json, expected_is_draft_v5)?
            == semantic_accepted_sketch_value(evaluated_json, evaluated_is_draft_v5)?,
    )
}

fn semantic_structural_value(checkpoint: &LineageCheckpoint) -> Result<Value, LineageBridgeError> {
    let mut value = checkpoint.structural_value()?;
    let root = value
        .as_object_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root must be an object",
        ))?;
    root.remove("lifecycle");
    let design = root
        .get_mut("design")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root is missing design state",
        ))?;
    let document = design
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization design document is invalid",
        ))?;
    strip_design_allocator_fields(document)?;
    let features = root
        .get_mut("features")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization feature document is invalid",
        ))?;
    for field in ["revision", "next_feature_id", "next_corner_id", "digest"] {
        features.remove(field);
    }
    Ok(value)
}

/// Canonical authored materialization excludes runtime lifecycle state and
/// derived authentication fields. The imported baseline owns allocator
/// high-waters; authored steps only describe how semantic entities change.
fn authored_structural_value(checkpoint: &LineageCheckpoint) -> Result<Value, LineageBridgeError> {
    let mut value = checkpoint.structural_value()?;
    let root = value
        .as_object_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root must be an object",
        ))?;
    root.remove("lifecycle");
    let design = root
        .get_mut("design")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root is missing design state",
        ))?;
    let document = design
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization design document is invalid",
        ))?;
    strip_design_allocator_fields(document)?;
    let features = root
        .get_mut("features")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization feature document is invalid",
        ))?;
    for field in ["revision", "next_feature_id", "next_corner_id", "digest"] {
        features.remove(field);
    }
    Ok(value)
}

fn strip_design_allocator_fields(
    encoded_document: &mut Map<String, Value>,
) -> Result<(), LineageBridgeError> {
    // `LineageCheckpoint::structural_value` always exposes the normalized
    // draft-v5-shaped semantic document, even when persistence later lowers it
    // back to canonical-v4.
    encoded_document.remove("next_id");
    let document = encoded_document
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "normalized materialization document is invalid",
        ))?;
    document.remove("next_id");
    if let Some(curves) = document.get_mut("curves").and_then(Value::as_array_mut) {
        for curve in curves {
            if let Some(definition) = curve
                .as_object_mut()
                .and_then(|curve| curve.get_mut("definition"))
                .and_then(Value::as_object_mut)
            {
                // B-spline/NURBS span IDs are persistent topology identity;
                // only the next-span cursor is monotonic allocator state.
                definition.remove("next_span_id");
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StructuralDeltaOperation {
    SetField {
        path: Vec<String>,
        value: Value,
    },
    RemoveField {
        path: Vec<String>,
    },
    UpsertEntity {
        path: Vec<String>,
        id: String,
        before: Option<String>,
        value: Value,
    },
    RemoveEntity {
        path: Vec<String>,
        id: String,
    },
}

impl StructuralDeltaOperation {
    fn target_key(&self) -> StructuralTargetKey {
        match self {
            Self::SetField { path, .. } | Self::RemoveField { path } => {
                StructuralTargetKey::Field(path.clone())
            }
            Self::UpsertEntity { path, id, .. } | Self::RemoveEntity { path, id } => {
                StructuralTargetKey::Entity {
                    path: path.clone(),
                    id: id.clone(),
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StructuralTargetKey {
    Field(Vec<String>),
    Entity { path: Vec<String>, id: String },
}

#[derive(Clone, Debug)]
pub(super) struct CoordinatorLineage {
    session: LineageSession,
    host_input_ledger: LineageHostInputLedger,
    #[cfg(test)]
    reject_next_record: Arc<AtomicBool>,
    #[cfg(test)]
    reject_next_evaluation_publication: Arc<AtomicBool>,
    #[cfg(test)]
    reject_next_direct_reproduction: Arc<AtomicBool>,
    #[cfg(test)]
    mismatch_next_direct_reproduction: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerRewriteMode {
    Ordinary,
    DirectManipulation,
}

/// One dependency-local structural replay beside the exact work it performed.
///
/// The accepted prefix is authenticated by exact step equality, immutable
/// host-input identity, and an independent owning-domain evaluation in the
/// caller. Reconstructing that prefix from retained lineage remains explicit
/// work; it is never reported as a free cache hit.
#[derive(Clone, Debug)]
pub(super) struct DependencyLocalMaterialization {
    final_checkpoint: LineageCheckpoint,
    evaluation_checkpoints: Vec<LineageMaterializedPrefix>,
    dirty_steps: Vec<LineageStepId>,
    ready_prefix_count: usize,
    reusable_prefix_count: usize,
    reconstructed_prefix_count: usize,
    replayed_suffix_count: usize,
}

#[derive(Clone, Debug)]
pub(super) struct LineageMaterializedPrefix {
    step: LineageStepId,
    checkpoint: LineageCheckpoint,
    host_inputs: DecodedLineageHostInputs,
}

impl LineageMaterializedPrefix {
    pub(super) const fn step(&self) -> LineageStepId {
        self.step
    }

    pub(super) fn checkpoint(&self) -> &LineageCheckpoint {
        &self.checkpoint
    }

    pub(super) fn host_inputs(&self) -> &DecodedLineageHostInputs {
        &self.host_inputs
    }
}

impl DependencyLocalMaterialization {
    pub(super) fn final_checkpoint(&self) -> &LineageCheckpoint {
        &self.final_checkpoint
    }

    pub(super) fn evaluation_checkpoints(&self) -> &[LineageMaterializedPrefix] {
        &self.evaluation_checkpoints
    }

    pub(super) fn dirty_steps(&self) -> &[LineageStepId] {
        &self.dirty_steps
    }

    pub(super) const fn ready_prefix_count(&self) -> usize {
        self.ready_prefix_count
    }

    pub(super) const fn reusable_prefix_count(&self) -> usize {
        self.reusable_prefix_count
    }

    pub(super) const fn reconstructed_prefix_count(&self) -> usize {
        self.reconstructed_prefix_count
    }

    pub(super) const fn replayed_suffix_count(&self) -> usize {
        self.replayed_suffix_count
    }
}

#[derive(Debug, Error)]
pub(super) enum LineageBridgeError {
    #[error(transparent)]
    Lineage(#[from] geosolve_sketch_lineage::LineageDocumentError),
    #[error(transparent)]
    MaterializationMap(#[from] LineageMaterializationMapError),
    #[error(transparent)]
    Key(#[from] geosolve_sketch_lineage::LineageKeyError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Document(#[from] geosolve_sketch::DocumentError),
    #[error(transparent)]
    FeatureDocument(#[from] geosolve_sketch_features::ComputedFeatureDocumentError),
    #[error("invalid external snapshot input: {0}")]
    InvalidExternalSnapshot(String),
    #[error("invalid lineage materialization: {0}")]
    InvalidMaterialization(&'static str),
    #[error("lineage materialization has no imported baseline")]
    MissingBaseline,
    #[error("lineage action has no writable owner for a changed field")]
    MissingOwner,
    #[error("lineage step {step:?} writable-leaf manifest does not match its authenticated action")]
    WritableManifestMismatch { step: LineageStepId },
    #[error(
        "lineage step {step:?} complete input/output/identity/reservation manifest does not match its authenticated action"
    )]
    ActionManifestMismatch { step: LineageStepId },
    #[error("lineage materialization patch exceeds its bounded operation count")]
    PatchResourceLimit,
    #[error("lineage history has no {0} entry")]
    MissingHistory(&'static str),
    #[error("lineage session history rebinds an immutable {0}")]
    HistoryIdentityRebinding(&'static str),
    #[error("cold lineage materialization does not reproduce the staged workbench state")]
    ReproductionMismatch,
    #[error("lineage step {step:?} cannot be structurally materialized: {source}")]
    StepMaterialization {
        step: LineageStepId,
        #[source]
        source: Box<LineageBridgeError>,
    },
    #[error("accepted lineage external-input provenance does not match its exact payloads")]
    AcceptedExternalInputMismatch,
    #[error("accepted lineage materialization digest does not match a cold lineage rebuild")]
    AcceptedMaterializationMismatch,
    #[error("accepted lineage could not be independently rebuilt: {0}")]
    AcceptedEvaluationRejected(String),
    #[cfg(test)]
    #[error("injected lineage record failure")]
    InjectedRecordFailure,
    #[cfg(test)]
    #[error("injected lineage evaluation-publication failure")]
    InjectedEvaluationPublicationFailure,
}

impl LineageBridgeError {
    pub(super) const fn materialization_step(&self) -> Option<LineageStepId> {
        match self {
            Self::StepMaterialization { step, .. } => Some(*step),
            Self::WritableManifestMismatch { step } | Self::ActionManifestMismatch { step } => {
                Some(*step)
            }
            _ => None,
        }
    }
}

impl CoordinatorLineage {
    #[allow(
        clippy::too_many_lines,
        reason = "one import boundary keeps baseline identity, retained/accepted host provenance, owning-domain authentication, and initial authority publication adjacent"
    )]
    pub(super) fn import(
        document: LineageDocumentId,
        checkpoint: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        accepted_parameters: Option<&ParameterBatch>,
        accepted_snapshots: Option<&ExternalSnapshotSet>,
        accepted_current: bool,
    ) -> Result<Self, LineageBridgeError> {
        let accepted_host_inputs = match (
            checkpoint.accepted_json(),
            accepted_parameters,
            accepted_snapshots,
            accepted_current,
        ) {
            (Some(_), Some(_), Some(_), true) | (None, None, None, false) => None,
            (Some(_), Some(parameters), Some(snapshots), false) => {
                Some(LineageHostInputProvenance::capture(parameters, snapshots)?)
            }
            _ => return Err(LineageBridgeError::AcceptedExternalInputMismatch),
        };
        let mut lineage = LineageDocument::with_id(document);
        let allocators = lineage.allocator_high_water();
        let baseline = ImportedCoordinatorBaseline {
            version: 1,
            checkpoint: LineageCheckpoint::capture(checkpoint),
            host_inputs: LineageHostInputProvenance::capture(parameters, snapshots)?,
            accepted_host_inputs,
        };
        let baseline_value = authored_structural_value(&baseline.checkpoint)?;
        let entities = collect_materialized_entities(&baseline_value);
        let manifest = created_manifest(
            document,
            allocators.next_step_id,
            allocators.next_output_id,
            allocators.next_reservation_id,
            &entities,
            &baseline_value,
        )?;
        let step = LineageStep::new(
            allocators.next_step_id,
            LineageDeveloperKey::new("imported-baseline")?,
            "Imported baseline",
            LineageActionDefinition::ImportedBaseline {
                baseline: ImportedBaselineAction {
                    encoding: ImportedBaselineEncoding::Opaque {
                        media_type: LineageSemanticKey::new(BASELINE_MEDIA_TYPE)?,
                        version: 1,
                    },
                    payload: serde_json::to_string(&baseline)?,
                },
            },
            manifest.outputs,
            manifest.reservations,
        )
        .with_output_identities(manifest.identities)
        .with_writable_leaves(manifest.writable_leaves);
        lineage.apply_patch(LineagePatch::new(
            lineage.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(step),
            }],
        ))?;
        let mut value = Self {
            session: LineageSession::new(lineage)?,
            host_input_ledger: LineageHostInputLedger::default(),
            #[cfg(test)]
            reject_next_record: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            reject_next_evaluation_publication: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            reject_next_direct_reproduction: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mismatch_next_direct_reproduction: Arc::new(AtomicBool::new(false)),
        };
        value.retain_computed_evaluation_high_water(checkpoint.computed_evaluation_high_water())?;
        if accepted_current {
            let expected_accepted = checkpoint
                .accepted_json()
                .map(|json| (json, checkpoint.accepted_uses_draft_v5()))
                .ok_or(LineageBridgeError::AcceptedMaterializationMismatch)?;
            match value.record_evaluation(
                parameters,
                snapshots,
                true,
                allocators.next_step_id,
                Some(expected_accepted),
            ) {
                Ok(_) => {}
                Err(
                    LineageBridgeError::AcceptedMaterializationMismatch
                    | LineageBridgeError::AcceptedEvaluationRejected(_),
                ) => value.record_embedded_imported_accepted_evaluation(
                    parameters,
                    snapshots,
                    expected_accepted,
                )?,
                Err(error) => return Err(error),
            }
        } else if checkpoint.accepted_json().is_none() {
            value.record_evaluation(parameters, snapshots, false, allocators.next_step_id, None)?;
        } else {
            let accepted_parameters =
                accepted_parameters.ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
            let accepted_snapshots =
                accepted_snapshots.ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
            let accepted = super::lineage_evaluation::
                evaluate_lineage_embedded_accepted_baseline_cold_with_inputs(
                    &value.session,
                    accepted_parameters,
                    accepted_snapshots,
                )
                .map_err(|failure| {
                    LineageBridgeError::AcceptedEvaluationRejected(format!(
                        "{}: {}",
                        failure.code(),
                        failure.message()
                    ))
                })?;
            let accepted_inputs = external_input_stamp(accepted_parameters, accepted_snapshots)?;
            if accepted.external_inputs() != &accepted_inputs {
                return Err(LineageBridgeError::AcceptedExternalInputMismatch);
            }
            if !accepted_materialization_semantically_matches(
                checkpoint
                    .accepted_json()
                    .ok_or(LineageBridgeError::AcceptedMaterializationMismatch)?,
                checkpoint.accepted_uses_draft_v5(),
                accepted.accepted_sketch_json(),
                accepted.accepted_uses_draft_v5(),
            )? {
                return Err(LineageBridgeError::AcceptedMaterializationMismatch);
            }
            value.session.accept_current(
                value.session.identity(),
                Some(accepted_inputs),
                accepted.materialization_digest(),
            )?;
            let required = value.accepted_external_input_stamps()?;
            value.host_input_ledger.retain_accepted_pair(
                accepted_parameters,
                accepted_snapshots,
                &required,
            )?;
            value.record_evaluation(parameters, snapshots, false, allocators.next_step_id, None)?;
        }
        Ok(value)
    }

    pub(super) const fn session(&self) -> &LineageSession {
        &self.session
    }

    pub(super) fn identity(&self) -> LineageDocumentIdentity {
        self.session.identity()
    }

    pub(super) fn to_canonical_json(&self) -> Result<String, LineageBridgeError> {
        Ok(self.session.to_canonical_json()?)
    }

    pub(super) fn to_canonical_session_json(&self) -> Result<String, LineageBridgeError> {
        Ok(self.session.to_canonical_session_json()?)
    }

    pub(super) fn to_canonical_host_input_ledger_json(&self) -> Result<String, LineageBridgeError> {
        let mut ledger = self.host_input_ledger.clone();
        ledger.retain_required(&self.accepted_external_input_stamps()?)?;
        ledger.to_canonical_json()
    }

    pub(super) fn from_session_json(json: &str) -> Result<Self, LineageBridgeError> {
        let session = LineageSession::from_session_json(json)?;
        validate_lineage_session_semantics(&session)?;
        Ok(Self {
            session,
            host_input_ledger: LineageHostInputLedger::default(),
            #[cfg(test)]
            reject_next_record: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            reject_next_evaluation_publication: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            reject_next_direct_reproduction: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            mismatch_next_direct_reproduction: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(super) fn from_session_and_host_input_ledger_json(
        session_json: &str,
        host_input_ledger_json: &str,
    ) -> Result<Self, LineageBridgeError> {
        let mut lineage = Self::from_session_json(session_json)?;
        lineage.host_input_ledger = LineageHostInputLedger::from_json(host_input_ledger_json)?;
        lineage
            .host_input_ledger
            .retain_required(&lineage.accepted_external_input_stamps()?)?;
        Ok(lineage)
    }

    #[cfg(test)]
    pub(super) fn reject_next_record_for_test(&self) {
        self.reject_next_record.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn reject_next_evaluation_publication_for_test(&self) {
        self.reject_next_evaluation_publication
            .store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn reject_next_direct_reproduction_for_test(&self) {
        self.reject_next_direct_reproduction
            .store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn mismatch_next_direct_reproduction_for_test(&self) {
        self.mismatch_next_direct_reproduction
            .store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(super) fn forge_historical_accepted_cache_for_test(
        &mut self,
        accepted_json: String,
        accepted_is_draft_v5: bool,
    ) -> Result<(), LineageBridgeError> {
        let baseline = self
            .session
            .document()
            .steps()
            .iter()
            .find(|step| {
                matches!(
                    step.action,
                    LineageActionDefinition::ImportedBaseline { .. }
                )
            })
            .ok_or(LineageBridgeError::MissingBaseline)?
            .clone();
        let mut forged_action = baseline.action.clone();
        let LineageActionDefinition::ImportedBaseline { baseline: action } = &mut forged_action
        else {
            return Err(LineageBridgeError::MissingBaseline);
        };
        let mut payload = serde_json::from_str::<ImportedCoordinatorBaseline>(&action.payload)?;
        payload.checkpoint.accepted_json = Some(accepted_json);
        payload.checkpoint.accepted_is_draft_v5 = accepted_is_draft_v5;
        payload.checkpoint.accepted_belongs_to_current_design = true;
        action.payload = serde_json::to_string(&payload)?;
        self.session.apply_patch(LineagePatch::new(
            self.session.identity(),
            vec![LineageMutation::Rewrite {
                step: baseline.id,
                replacement: Box::new(LineageStepRewrite {
                    label: baseline.label.clone(),
                    action: forged_action,
                }),
            }],
        ))?;

        // Push the forged program into Undo while keeping its authored and
        // materialized geometry unchanged. The next Undo therefore exercises
        // a hostile serialized historical cache, not the current flat scene.
        let current = self
            .session
            .document()
            .step(baseline.id)
            .ok_or(LineageBridgeError::MissingBaseline)?
            .clone();
        self.session.apply_patch(LineagePatch::new(
            self.session.identity(),
            vec![LineageMutation::Rewrite {
                step: current.id,
                replacement: Box::new(LineageStepRewrite {
                    label: format!("{} (history witness)", current.label),
                    action: current.action,
                }),
            }],
        ))?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn take_direct_reproduction_rejection_for_test(&self) -> bool {
        self.reject_next_direct_reproduction
            .swap(false, Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(super) fn take_direct_reproduction_mismatch_for_test(&self) -> bool {
        self.mismatch_next_direct_reproduction
            .swap(false, Ordering::SeqCst)
    }

    pub(super) fn retain_computed_evaluation_high_water(
        &mut self,
        high_water: geosolve_sketch_features::ComputedEvaluationAllocatorHighWater,
    ) -> Result<(), LineageBridgeError> {
        self.session.retain_auxiliary_high_water(
            LineageSemanticKey::new(COMPUTED_EVALUATION_HIGH_WATER_KEY)?,
            LineageAuxiliaryHighWater::from_raw(high_water.next_revision.raw()),
        )?;
        Ok(())
    }

    pub(super) const fn undo_len(&self) -> usize {
        self.session.undo_len()
    }

    pub(super) const fn redo_len(&self) -> usize {
        self.session.redo_len()
    }

    pub(super) const fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub(super) const fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    pub(super) fn record(
        &mut self,
        replay: &ReplayAction,
        variant: Option<GeometryToolVariant>,
        before: &RestoreCheckpoint,
        after: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        self.record_with_owner_mode(
            replay,
            variant,
            before,
            after,
            parameters,
            snapshots,
            OwnerRewriteMode::Ordinary,
        )
    }

    pub(super) fn direct_manipulation_has_authored_change(
        &self,
        before: &RestoreCheckpoint,
        after: &RestoreCheckpoint,
    ) -> Result<bool, LineageBridgeError> {
        if !self.materialize()?.semantically_matches(before)? {
            return Err(LineageBridgeError::ReproductionMismatch);
        }
        let before = authored_structural_value(&LineageCheckpoint::capture(before))?;
        let after = authored_structural_value(&LineageCheckpoint::capture(after))?;
        Ok(!structural_delta(&before, &after)?.is_empty())
    }

    pub(super) fn record_direct_manipulation(
        &mut self,
        replay: &ReplayAction,
        variant: Option<GeometryToolVariant>,
        before: &RestoreCheckpoint,
        after: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<LineageDomainEvaluationEvidence, LineageBridgeError> {
        self.record_with_owner_mode(
            replay,
            variant,
            before,
            after,
            parameters,
            snapshots,
            OwnerRewriteMode::DirectManipulation,
        )?
        .ok_or(LineageBridgeError::ReproductionMismatch)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::too_many_arguments,
        reason = "one authority seam keeps exact replay, materialization, host inputs, owner routing, and rollback adjacent"
    )]
    fn record_with_owner_mode(
        &mut self,
        replay: &ReplayAction,
        variant: Option<GeometryToolVariant>,
        before: &RestoreCheckpoint,
        after: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        owner_mode: OwnerRewriteMode,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        #[cfg(test)]
        if self.reject_next_record.swap(false, Ordering::SeqCst) {
            return Err(LineageBridgeError::InjectedRecordFailure);
        }
        // The flat coordinator state is a transaction-local materialization,
        // never an independent source of truth. Authenticate its exact
        // semantic starting point against a fresh lineage rebuild before
        // deriving an action delta from the owning domains.
        if !self.materialize()?.semantically_matches(before)? {
            return Err(LineageBridgeError::ReproductionMismatch);
        }
        let before_checkpoint = LineageCheckpoint::capture(before);
        let before_value = authored_structural_value(&before_checkpoint)?;
        let after_checkpoint = LineageCheckpoint::capture(after);
        let after_value = authored_structural_value(&after_checkpoint)?;
        let delta = structural_delta(&before_value, &after_value)?;
        let previous = self.session.clone();
        let previous_ledger = self.host_input_ledger.clone();
        let deleted_owners = self.complete_deleted_owner_steps(replay, &delta, &after_value);
        let failed_step = if owner_mode == OwnerRewriteMode::DirectManipulation {
            if !deleted_owners.is_empty() {
                return Err(LineageBridgeError::MissingOwner);
            }
            let owners =
                self.rewrite_direct_owners(&before_value, &after_value, &delta, &after_checkpoint)?;
            owners.last().copied()
        } else if !deleted_owners.is_empty() {
            let mut mutations = deleted_owners
                .iter()
                .copied()
                .map(|step| LineageMutation::Tombstone { step })
                .collect::<Vec<_>>();
            if let Some(rewrite) = self.baseline_high_water_rewrite(&after_checkpoint)? {
                mutations.push(rewrite);
            }
            self.session
                .apply_patch(LineagePatch::new(self.session.identity(), mutations))?;
            deleted_owners.last().copied()
        } else if self.delta_has_complete_owner_map(&before_value, &delta) {
            let owners =
                self.rewrite_owners(&before_value, &after_value, &delta, &after_checkpoint)?;
            owners.last().copied()
        } else {
            let allocators = self.session.document().allocator_high_water();
            let mut manifest = manifest_for_delta(
                self.session.document(),
                allocators.next_step_id,
                allocators.next_output_id,
                allocators.next_reservation_id,
                &before_value,
                &after_value,
                &delta,
            )?;
            enrich_replay_manifest(self.session.document(), replay, &mut manifest)?;
            let action = action_for_replay(
                replay,
                variant,
                &delta,
                manifest.inputs.clone(),
                ReplayActionContext {
                    outputs: &manifest.outputs,
                    writable_leaves: &manifest.writable_leaves,
                    output_fields: &authored_output_field_values_for_manifest(
                        self.session.document(),
                        &manifest,
                        &after_value,
                    )?,
                    host_parameters: parameters,
                    host_snapshots: snapshots,
                },
            )?;
            let sequence = allocators.next_step_id.raw();
            let step = LineageStep::new(
                allocators.next_step_id,
                LineageDeveloperKey::new(format!("action-{sequence:016x}"))?,
                replay_label(replay),
                action,
                manifest.outputs,
                manifest.reservations,
            )
            .with_output_identities(manifest.identities)
            .with_writable_leaves(manifest.writable_leaves);
            let step_id = step.id;
            let mut mutations = vec![LineageMutation::Insert {
                before: None,
                step: Box::new(step),
            }];
            if let Some(rewrite) = self.baseline_high_water_rewrite(&after_checkpoint)? {
                mutations.push(rewrite);
            }
            self.session
                .apply_patch(LineagePatch::new(self.session.identity(), mutations))?;
            Some(step_id)
        };

        let rebuilt = self.materialize_with_failed_steps(&[])?;
        if semantic_structural_value(&rebuilt)? != semantic_structural_value(&after_checkpoint)? {
            self.session = previous;
            self.host_input_ledger = previous_ledger;
            return Err(LineageBridgeError::ReproductionMismatch);
        }
        let accepted_current =
            after.accepted_belongs_to_current_design() && after.accepted_json().is_some();
        let evaluation = match self.record_evaluation(
            parameters,
            snapshots,
            accepted_current,
            failed_step.unwrap_or_else(|| self.session.document().steps()[0].id),
            None,
        ) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                self.session = previous;
                self.host_input_ledger = previous_ledger;
                return Err(error);
            }
        };
        if let Err(error) =
            self.retain_computed_evaluation_high_water(after.computed_evaluation_high_water())
        {
            self.session = previous;
            self.host_input_ledger = previous_ledger;
            return Err(error);
        }
        Ok(evaluation)
    }

    pub(super) fn record_operation(
        &mut self,
        proposal: &SketchOperationProposal,
        before: &RestoreCheckpoint,
        after: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        if !self.materialize()?.semantically_matches(before)? {
            return Err(LineageBridgeError::ReproductionMismatch);
        }
        let before_checkpoint = LineageCheckpoint::capture(before);
        let before_value = authored_structural_value(&before_checkpoint)?;
        let after_checkpoint = LineageCheckpoint::capture(after);
        let after_value = authored_structural_value(&after_checkpoint)?;
        let delta = structural_delta(&before_value, &after_value)?;
        if delta.is_empty() {
            return Err(LineageBridgeError::InvalidMaterialization(
                "accepted sketch operation produced no authored change",
            ));
        }

        let previous = self.session.clone();
        let previous_ledger = self.host_input_ledger.clone();
        let allocators = self.session.document().allocator_high_water();
        let mut manifest = manifest_for_delta(
            self.session.document(),
            allocators.next_step_id,
            allocators.next_output_id,
            allocators.next_reservation_id,
            &before_value,
            &after_value,
            &delta,
        )?;
        enrich_operation_manifest(self.session.document(), proposal, &mut manifest)?;
        let action = action_for_operation(
            proposal,
            &delta,
            manifest.inputs.clone(),
            ReplayActionContext {
                outputs: &manifest.outputs,
                writable_leaves: &manifest.writable_leaves,
                output_fields: &authored_output_field_values_for_manifest(
                    self.session.document(),
                    &manifest,
                    &after_value,
                )?,
                host_parameters: parameters,
                host_snapshots: snapshots,
            },
        )?;
        let sequence = allocators.next_step_id.raw();
        let step = LineageStep::new(
            allocators.next_step_id,
            LineageDeveloperKey::new(format!("operation-{sequence:016x}"))?,
            format!("{} operation", proposal.request().kind().semantic_key()),
            action,
            manifest.outputs,
            manifest.reservations,
        )
        .with_output_identities(manifest.identities)
        .with_writable_leaves(manifest.writable_leaves);
        let step_id = step.id;
        let mut mutations = vec![LineageMutation::Insert {
            before: None,
            step: Box::new(step),
        }];
        if let Some(rewrite) = self.baseline_high_water_rewrite(&after_checkpoint)? {
            mutations.push(rewrite);
        }
        self.session
            .apply_patch(LineagePatch::new(self.session.identity(), mutations))?;

        let rebuilt = match self.materialize_with_failed_steps(&[]) {
            Ok(rebuilt) => rebuilt,
            Err(error) => {
                self.session = previous;
                self.host_input_ledger = previous_ledger;
                return Err(error);
            }
        };
        if semantic_structural_value(&rebuilt)? != semantic_structural_value(&after_checkpoint)? {
            self.session = previous;
            self.host_input_ledger = previous_ledger;
            return Err(LineageBridgeError::ReproductionMismatch);
        }
        let accepted_current =
            after.accepted_belongs_to_current_design() && after.accepted_json().is_some();
        let evaluation =
            match self.record_evaluation(parameters, snapshots, accepted_current, step_id, None) {
                Ok(evaluation) => evaluation,
                Err(error) => {
                    self.session = previous;
                    self.host_input_ledger = previous_ledger;
                    return Err(error);
                }
            };
        if let Err(error) =
            self.retain_computed_evaluation_high_water(after.computed_evaluation_high_water())
        {
            self.session = previous;
            self.host_input_ledger = previous_ledger;
            return Err(error);
        }
        Ok(evaluation)
    }

    fn complete_deleted_owner_steps(
        &self,
        replay: &ReplayAction,
        delta: &[StructuralDeltaOperation],
        after: &Value,
    ) -> BTreeSet<LineageStepId> {
        let selected = deletion_targets(replay, delta);
        if selected.is_empty() {
            return BTreeSet::new();
        }
        let remaining = collect_materialized_entities(after)
            .into_iter()
            .map(|entity| (entity.output_kind, entity.persistent_id))
            .collect::<BTreeSet<_>>();
        let mut created_owners = BTreeMap::<(LineageOutputKind, String), LineageStepId>::new();
        let mut owned_by_step =
            BTreeMap::<LineageStepId, BTreeSet<(LineageOutputKind, String)>>::new();
        for step in self.session.document().steps() {
            if matches!(
                step.action,
                LineageActionDefinition::ImportedBaseline { .. }
            ) {
                continue;
            }
            let reservations = step
                .reservations
                .iter()
                .map(|reservation| (reservation.id, reservation))
                .collect::<BTreeMap<_, _>>();
            for output in &step.outputs {
                if !matches!(
                    step.output_identity(output.id)
                        .map(|identity| &identity.flow),
                    Some(LineageOutputIdentityFlow::Created { .. })
                ) {
                    continue;
                }
                let Some(raw) = output
                    .reservation
                    .and_then(|reservation| reservations.get(&reservation))
                    .and_then(|reservation| reservation.persistent_id.as_str().split_once(':'))
                    .map(|(_, raw)| raw.to_owned())
                else {
                    continue;
                };
                let key = (output.kind, raw);
                created_owners.insert(key.clone(), step.id);
                owned_by_step.entry(step.id).or_default().insert(key);
            }
        }

        let candidates = selected
            .iter()
            .filter_map(|target| created_owners.get(target).copied())
            .collect::<BTreeSet<_>>();
        candidates
            .into_iter()
            .filter(|owner| {
                owned_by_step
                    .get(owner)
                    .is_some_and(|owned| owned.iter().all(|identity| !remaining.contains(identity)))
            })
            .collect()
    }

    fn baseline_high_water_rewrite(
        &self,
        after: &LineageCheckpoint,
    ) -> Result<Option<LineageMutation>, LineageBridgeError> {
        let Some(step) = self.session.document().steps().iter().find(|step| {
            matches!(
                step.action,
                LineageActionDefinition::ImportedBaseline { .. }
            )
        }) else {
            return Err(LineageBridgeError::MissingBaseline);
        };
        let mut action = step.action.clone();
        if !retain_baseline_materialization_evidence(&mut action, after)? {
            return Ok(None);
        }
        Ok(Some(LineageMutation::Rewrite {
            step: step.id,
            replacement: Box::new(LineageStepRewrite {
                label: step.label.clone(),
                action,
            }),
        }))
    }

    pub(super) fn undo(&mut self) -> Result<LineageCheckpoint, LineageBridgeError> {
        self.session
            .undo()?
            .ok_or(LineageBridgeError::MissingHistory("Undo"))?;
        self.materialize_with_failed_steps(&[])
    }

    pub(super) fn redo(&mut self) -> Result<LineageCheckpoint, LineageBridgeError> {
        self.session
            .redo()?
            .ok_or(LineageBridgeError::MissingHistory("Redo"))?;
        self.materialize_with_failed_steps(&[])
    }

    pub(super) fn materialize(&self) -> Result<LineageCheckpoint, LineageBridgeError> {
        // A failed owning-domain solve still has a complete retained program.
        // Failure evidence controls accepted publication, not whether the
        // authored step exists in cold retained materialization.
        self.materialize_with_failed_steps(&[])
    }

    /// Returns every strictly ready authored prefix beside the exact step that
    /// completed it. Owning-domain evaluation uses these prefixes to validate
    /// topology-sensitive input against accepted upstream state and to
    /// attribute the first rejected prefix to its semantic lineage step.
    pub(super) fn strict_ready_prefixes(
        &self,
    ) -> Result<Vec<LineageMaterializedPrefix>, LineageBridgeError> {
        let mut prefixes = Self::materialize_document_prefixes_for_policy(
            self.session.document(),
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            &[],
        )?;
        if let Some(high_water) = self.session.auxiliary_high_water(&LineageSemanticKey::new(
            COMPUTED_EVALUATION_HIGH_WATER_KEY,
        )?) {
            for prefix in &mut prefixes {
                prefix
                    .checkpoint
                    .retain_computed_evaluation_high_water(high_water);
            }
        }
        Ok(prefixes)
    }

    /// Returns the one honest legacy accepted branch embedded in an imported
    /// baseline whose retained design was already invalid when migration
    /// began. The branch changes no lineage action, identity, or history: it
    /// only selects the exact older accepted sketch bytes and host inputs for
    /// independent owning-domain authentication.
    pub(super) fn embedded_accepted_baseline_prefixes(
        &self,
    ) -> Result<Option<Vec<LineageMaterializedPrefix>>, LineageBridgeError> {
        Self::embedded_accepted_baseline_prefixes_for_document(self.session.document())
    }

    fn embedded_accepted_baseline_prefixes_for_document(
        document: &LineageDocument,
    ) -> Result<Option<Vec<LineageMaterializedPrefix>>, LineageBridgeError> {
        let [step] = document.steps() else {
            return Ok(None);
        };
        let LineageActionDefinition::ImportedBaseline { baseline } = &step.action else {
            return Ok(None);
        };
        let imported: ImportedCoordinatorBaseline = serde_json::from_str(&baseline.payload)?;
        if imported.version != 1 {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported imported-baseline bridge version",
            ));
        }
        let provenance = if let Some(provenance) = imported.accepted_host_inputs {
            provenance
        } else if imported.checkpoint.accepted_belongs_to_current_design
            && imported.checkpoint.accepted_json.is_some()
        {
            // An honest current-design flat-session import may carry an exact
            // independently certified accepted graph whose coordinates differ
            // from its retained seeds. Its current host provenance is already
            // the baseline's authored input provenance; no duplicate sidecar
            // payload is needed merely to authenticate that accepted branch.
            imported.host_inputs
        } else {
            return Ok(None);
        };
        let checkpoint = imported
            .checkpoint
            .embedded_accepted_branch()?
            .ok_or(LineageBridgeError::AcceptedMaterializationMismatch)?;
        let plan = document.dependency_plan_for(
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            [],
        )?;
        if !matches!(
            plan.as_slice(),
            [entry]
                if entry.step == step.id
                    && matches!(
                        entry.state,
                        geosolve_sketch_lineage::LineageStepEvaluationState::Ready
                    )
        ) {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        }
        Ok(Some(vec![LineageMaterializedPrefix {
            step: step.id,
            checkpoint,
            host_inputs: provenance.decode()?,
        }]))
    }

    /// Decodes the semantic prepared plan retained by one native Fillet action.
    /// Cold evaluation never accepts it by inspection: the sketch domain
    /// reauthenticates its geometry against the independently accepted upstream
    /// prefix and its reserved identities against the materialized output graph.
    pub(super) fn native_fillet_continuation_plan(
        &self,
        step: LineageStepId,
    ) -> Result<Option<geosolve_sketch::DocumentPreparedNativeLineFilletGeometry>, LineageBridgeError>
    {
        let step = self.session.document().step(step).ok_or(
            LineageBridgeError::InvalidMaterialization(
                "lineage evaluation references a missing step",
            ),
        )?;
        let LineageActionDefinition::Operation { action } = &step.action else {
            return Ok(None);
        };
        if action.schema.as_str()
            != "geosolve.document-edit.v1.create-prepared-native-line-fillet-geometry"
        {
            return Ok(None);
        }
        let intent = persisted_intent_object(action)?;
        validate_persisted_intent_keys(
            intent,
            &["version", "body", AUTHORED_OUTPUT_FIELD_MANIFEST],
        )?;
        let body = persisted_object_field(intent, "body")?;
        validate_exact_object_keys(body, &["CreatePreparedNativeLineFilletGeometry"])?;
        let edit = persisted_object_field(body, "CreatePreparedNativeLineFilletGeometry")?;
        validate_exact_object_keys(edit, &["prepared"])?;
        Ok(Some(persisted_field::<
            geosolve_sketch::DocumentPreparedNativeLineFilletGeometry,
        >(edit, "prepared")?))
    }

    /// Reconstructs the largest unchanged accepted prefix, structurally
    /// replays the current suffix, and returns only the checkpoints that the
    /// dependency-local owning-domain evaluator must inspect: the reusable
    /// anchor, every ready dirty step, and the complete final state.
    ///
    /// `accepted_inputs_match` must be derived from the exact engine-owned
    /// host-input stamp. A changed or unavailable stamp deliberately disables
    /// all reuse and degenerates to complete chronological evaluation.
    #[allow(
        clippy::too_many_lines,
        reason = "one ordered authority pipeline keeps accepted-prefix authentication, typed dirty closure, allocator-safe reconstruction, suffix replay, and exact work charging adjacent"
    )]
    pub(super) fn dependency_local_materialization(
        &self,
        accepted_inputs_match: bool,
    ) -> Result<DependencyLocalMaterialization, LineageBridgeError> {
        let current = self.session.document();
        let current_plan = current.dependency_plan_for(
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            [],
        )?;
        let ready_prefix_count = current_plan
            .iter()
            .filter(|entry| {
                matches!(
                    entry.state,
                    geosolve_sketch_lineage::LineageStepEvaluationState::Ready
                )
            })
            .count();
        let accepted = accepted_inputs_match
            .then(|| self.session.last_accepted_document())
            .flatten()
            .filter(|document| document.id() == current.id());

        let (common_prefix_len, dirty_steps) = if let Some(accepted) = accepted {
            let common_prefix_len = common_materialization_prefix_len(accepted, current)?;
            let changed = changed_current_steps(accepted, current)?;
            let dirty = if changed.is_empty() {
                Vec::new()
            } else {
                current.dirty_dependency_closure(changed)?
            };
            (common_prefix_len, dirty)
        } else {
            (
                0,
                current
                    .steps()
                    .iter()
                    .map(|step| step.id)
                    .collect::<Vec<_>>(),
            )
        };
        let dirty = dirty_steps.iter().copied().collect::<BTreeSet<_>>();

        let anchor = accepted
            .filter(|_| common_prefix_len > 0)
            .map(|_| {
                // The accepted document authenticates semantic equivalence,
                // while the current equivalent prefix carries monotonic
                // allocator/lifecycle high-waters needed by newly appended
                // reservations. Reconstruct from current bytes so reuse can
                // never regress an allocator through a cache seam.
                Self::materialize_document_prefix_for_policy(
                    current,
                    geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
                    common_prefix_len,
                    &[],
                )
            })
            .transpose()?
            .flatten();
        let mut checkpoint = anchor.as_ref().map(|prefix| prefix.checkpoint.clone());
        let mut final_host_inputs = anchor.as_ref().map(|prefix| prefix.host_inputs.clone());
        let reusable_prefix_count = current_plan
            .iter()
            .take(common_prefix_len)
            .filter(|entry| {
                matches!(
                    entry.state,
                    geosolve_sketch_lineage::LineageStepEvaluationState::Ready
                )
            })
            .count();
        let reconstructed_prefix_count = reusable_prefix_count;
        let mut evaluation_checkpoints = Vec::new();

        if let Some(anchor) = anchor.as_ref() {
            // A reusable structural anchor can contain a native topology
            // transition whose retained numerical seeds deliberately differ
            // from its accepted preview. Preserve an independently accepted
            // barrier immediately before and after every such transition;
            // otherwise evaluating only the terminal anchor would skip the
            // semantic reauthentication needed to reconstruct its exact
            // accepted continuation. Ordinary anchors retain the one-checkpoint
            // fast path.
            let prefix_step_ids = current
                .steps()
                .iter()
                .take(common_prefix_len)
                .map(|step| step.id)
                .collect::<BTreeSet<_>>();
            let ready_prefixes = Self::materialize_document_prefixes_for_policy(
                current,
                geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
                &[],
            )?;
            let reusable_prefixes = ready_prefixes
                .iter()
                .take_while(|prefix| prefix_step_ids.contains(&prefix.step))
                .collect::<Vec<_>>();
            for (index, prefix) in reusable_prefixes.iter().enumerate() {
                let step =
                    current
                        .step(prefix.step)
                        .ok_or(LineageBridgeError::InvalidMaterialization(
                            "dependency-local anchor references a missing step",
                        ))?;
                if !is_native_line_fillet_materialization_step(step) {
                    continue;
                }
                if let Some(upstream) = index
                    .checked_sub(1)
                    .and_then(|upstream| reusable_prefixes.get(upstream))
                {
                    push_distinct_evaluation_checkpoint(
                        &mut evaluation_checkpoints,
                        (*upstream).clone(),
                    );
                }
                push_distinct_evaluation_checkpoint(&mut evaluation_checkpoints, (*prefix).clone());
            }
            // Re-authenticate the reconstructed reusable anchor. This is one
            // real owning-domain evaluation and is charged by policy telemetry.
            push_distinct_evaluation_checkpoint(&mut evaluation_checkpoints, anchor.clone());
        }

        let mut replayed_suffix_count = 0_usize;
        let mut last_ready_step = anchor.as_ref().map(|prefix| prefix.step);
        for (step, plan) in current
            .steps()
            .iter()
            .zip(current_plan.iter())
            .skip(common_prefix_len)
        {
            if !matches!(
                plan.state,
                geosolve_sketch_lineage::LineageStepEvaluationState::Ready
            ) {
                continue;
            }
            if is_native_line_fillet_materialization_step(step)
                && let (Some(upstream_step), Some(upstream_checkpoint), Some(upstream_inputs)) = (
                    last_ready_step,
                    checkpoint.as_ref(),
                    final_host_inputs.as_ref(),
                )
            {
                push_distinct_evaluation_checkpoint(
                    &mut evaluation_checkpoints,
                    LineageMaterializedPrefix {
                        step: upstream_step,
                        checkpoint: upstream_checkpoint.clone(),
                        host_inputs: upstream_inputs.clone(),
                    },
                );
            }
            let host_inputs = apply_materialization_step(current, &mut checkpoint, step)?;
            final_host_inputs = Some(host_inputs.clone());
            replayed_suffix_count = replayed_suffix_count.saturating_add(1);
            if dirty.contains(&step.id) || is_native_line_fillet_materialization_step(step) {
                push_distinct_evaluation_checkpoint(
                    &mut evaluation_checkpoints,
                    LineageMaterializedPrefix {
                        step: step.id,
                        checkpoint: checkpoint
                            .as_ref()
                            .ok_or(LineageBridgeError::MissingBaseline)?
                            .clone(),
                        host_inputs,
                    },
                );
            }
            last_ready_step = Some(step.id);
        }

        let final_checkpoint = checkpoint.ok_or(LineageBridgeError::MissingBaseline)?;
        let final_host_inputs = final_host_inputs.ok_or(LineageBridgeError::MissingBaseline)?;
        let final_step = current_plan
            .iter()
            .rev()
            .find(|entry| {
                matches!(
                    entry.state,
                    geosolve_sketch_lineage::LineageStepEvaluationState::Ready
                )
            })
            .map(|entry| entry.step)
            .ok_or(LineageBridgeError::MissingBaseline)?;
        if evaluation_checkpoints
            .last()
            .is_none_or(|prefix| prefix.step != final_step)
        {
            push_distinct_evaluation_checkpoint(
                &mut evaluation_checkpoints,
                LineageMaterializedPrefix {
                    step: final_step,
                    checkpoint: final_checkpoint.clone(),
                    host_inputs: final_host_inputs,
                },
            );
        }

        if let Some(high_water) = self.session.auxiliary_high_water(&LineageSemanticKey::new(
            COMPUTED_EVALUATION_HIGH_WATER_KEY,
        )?) {
            let mut final_checkpoint = final_checkpoint;
            final_checkpoint.retain_computed_evaluation_high_water(high_water);
            for prefix in &mut evaluation_checkpoints {
                prefix
                    .checkpoint
                    .retain_computed_evaluation_high_water(high_water);
            }
            return Ok(DependencyLocalMaterialization {
                final_checkpoint,
                evaluation_checkpoints,
                dirty_steps,
                ready_prefix_count,
                reusable_prefix_count,
                reconstructed_prefix_count,
                replayed_suffix_count,
            });
        }

        Ok(DependencyLocalMaterialization {
            final_checkpoint,
            evaluation_checkpoints,
            dirty_steps,
            ready_prefix_count,
            reusable_prefix_count,
            reconstructed_prefix_count,
            replayed_suffix_count,
        })
    }

    pub(super) fn materialize_last_accepted(
        &self,
    ) -> Result<Option<LineageCheckpoint>, LineageBridgeError> {
        let Some(document) = self.session.last_accepted_document() else {
            return Ok(None);
        };
        let authority = self
            .session
            .last_accepted()
            .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
        let distinct_embedded_baseline = match document.steps() {
            [step] => match &step.action {
                LineageActionDefinition::ImportedBaseline { baseline } => {
                    let imported: ImportedCoordinatorBaseline =
                        serde_json::from_str(&baseline.payload)?;
                    imported.accepted_host_inputs.is_some()
                        && !imported.checkpoint.accepted_belongs_to_current_design
                }
                _ => false,
            },
            _ => false,
        };
        if distinct_embedded_baseline
            && let Some(mut embedded) =
                Self::embedded_accepted_baseline_prefixes_for_document(document)?
        {
            let prefix = embedded
                .pop()
                .ok_or(LineageBridgeError::AcceptedMaterializationMismatch)?;
            let embedded_inputs = external_input_stamp(
                prefix.host_inputs.parameters(),
                prefix.host_inputs.snapshots(),
            )?;
            if authority.external_inputs.as_ref() == Some(&embedded_inputs) {
                return Ok(Some(prefix.checkpoint));
            }
        }
        Self::materialize_document_for_policy(
            document,
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            &[],
        )
        .map(Some)
    }

    pub(super) fn materialize_last_accepted_with_inputs(
        &self,
    ) -> Result<
        Option<(
            LineageCheckpoint,
            DecodedLineageHostInputs,
            bool,
            LineageDomainEvaluationEvidence,
        )>,
        LineageBridgeError,
    > {
        let Some(document) = self.session.last_accepted_document() else {
            return Ok(None);
        };
        let authority = self
            .session
            .last_accepted()
            .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
        let stamp = authority
            .external_inputs
            .as_ref()
            .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
        let prefix = Self::materialize_document_prefixes_for_policy(
            document,
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            &[],
        )?
        .pop()
        .ok_or(LineageBridgeError::MissingBaseline)?;
        let inputs = if let Some(inputs) = self.host_input_ledger.decoded_entry(stamp)? {
            inputs
        } else if let Some(inputs) = declared_host_inputs(document)?.into_iter().find(|inputs| {
            external_input_stamp(inputs.parameters(), inputs.snapshots())
                .is_ok_and(|candidate| &candidate == stamp)
        }) {
            inputs
        } else {
            let prefix_stamp = external_input_stamp(
                prefix.host_inputs.parameters(),
                prefix.host_inputs.snapshots(),
            )?;
            if &prefix_stamp != stamp {
                return Err(LineageBridgeError::AcceptedExternalInputMismatch);
            }
            prefix.host_inputs
        };
        let evaluation_session = LineageSession::new(document.clone())?;
        if let Ok(evaluation) = super::lineage_evaluation::evaluate_lineage_session_cold_with_inputs(
            &evaluation_session,
            inputs.parameters(),
            inputs.snapshots(),
        ) && evaluation.materialization_digest() == authority.materialization_digest
        {
            return Ok(Some((prefix.checkpoint, inputs, false, evaluation)));
        }
        let embedded = super::lineage_evaluation::
            evaluate_lineage_embedded_accepted_baseline_cold_with_inputs(
                &evaluation_session,
                inputs.parameters(),
                inputs.snapshots(),
            )
            .map_err(|failure| {
                LineageBridgeError::AcceptedEvaluationRejected(format!(
                    "{}: {}",
                    failure.code(),
                    failure.message()
                ))
            })?;
        if embedded.materialization_digest() != authority.materialization_digest {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        }
        let embedded_lineage =
            Self::from_session_json(&evaluation_session.to_canonical_session_json()?)?;
        let embedded_prefix = embedded_lineage
            .embedded_accepted_baseline_prefixes()?
            .and_then(|mut prefixes| prefixes.pop())
            .ok_or(LineageBridgeError::AcceptedMaterializationMismatch)?;
        let embedded_stamp = external_input_stamp(
            embedded_prefix.host_inputs.parameters(),
            embedded_prefix.host_inputs.snapshots(),
        )?;
        if &embedded_stamp != stamp || embedded_prefix.host_inputs != inputs {
            return Err(LineageBridgeError::AcceptedExternalInputMismatch);
        }
        let [baseline] = document.steps() else {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        };
        let LineageActionDefinition::ImportedBaseline { baseline } = &baseline.action else {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        };
        let imported: ImportedCoordinatorBaseline = serde_json::from_str(&baseline.payload)?;
        let embedded_is_distinct_historical =
            !imported.checkpoint.accepted_belongs_to_current_design;
        Ok(Some((
            embedded_prefix.checkpoint,
            inputs,
            embedded_is_distinct_historical,
            embedded,
        )))
    }

    pub(super) fn cold_historical_accepted_evidence_checkpoint(
        &self,
    ) -> Result<Option<LineageCheckpoint>, LineageBridgeError> {
        let Some((mut checkpoint, _inputs, _embedded_accepted_baseline, evidence)) =
            self.materialize_last_accepted_with_inputs()?
        else {
            return Ok(None);
        };
        checkpoint.accepted_json = Some(evidence.accepted_sketch_json().to_owned());
        checkpoint.accepted_is_draft_v5 = evidence.accepted_uses_draft_v5();
        checkpoint.accepted_belongs_to_current_design = false;
        Ok(Some(checkpoint))
    }

    pub(super) fn cold_current_accepted_evidence_checkpoint(
        &self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<Option<LineageCheckpoint>, LineageBridgeError> {
        let Some(evidence) = self.reproduce_current_accepted_evaluation(parameters, snapshots)?
        else {
            return Ok(None);
        };
        if evidence.lineage() != self.session.identity() {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        }
        let mut checkpoint = self.materialize()?;
        checkpoint.accepted_json = Some(evidence.accepted_sketch_json().to_owned());
        checkpoint.accepted_is_draft_v5 = evidence.accepted_uses_draft_v5();
        checkpoint.accepted_belongs_to_current_design = true;
        Ok(Some(checkpoint))
    }

    fn retained_sessions(&self) -> Result<Vec<LineageSession>, LineageBridgeError> {
        let mut positions = vec![self.session.clone()];
        let mut undo = self.session.clone();
        for _ in 0..self.undo_len() {
            undo.undo()?
                .ok_or(LineageBridgeError::MissingHistory("Undo"))?;
            positions.push(undo.clone());
        }
        let mut redo = self.session.clone();
        for _ in 0..self.redo_len() {
            redo.redo()?
                .ok_or(LineageBridgeError::MissingHistory("Redo"))?;
            positions.push(redo.clone());
        }
        Ok(positions)
    }

    fn accepted_external_input_stamps(
        &self,
    ) -> Result<BTreeSet<LineageOpaqueId>, LineageBridgeError> {
        Ok(self
            .retained_sessions()?
            .into_iter()
            .filter_map(|position| position.last_accepted()?.external_inputs.clone())
            .collect())
    }

    /// Cold-authenticates every distinct accepted authority reachable in the
    /// persisted current, Undo, and Redo positions. Session structure alone
    /// cannot certify an owning-domain materialization digest, and deferring
    /// this check until traversal would let a workspace claim historical
    /// authority that it cannot actually reproduce.
    pub(super) fn validate_complete_accepted_authority(
        &mut self,
        accepted_parameters: Option<&ParameterBatch>,
        accepted_snapshots: Option<&ExternalSnapshotSet>,
    ) -> Result<(), LineageBridgeError> {
        let positions = self.retained_sessions()?;

        // Host-only evaluations deliberately do not create lineage steps or
        // user-visible history. The current workspace therefore supplies its
        // exact accepted input pair separately. Historical pairs are resolved
        // from every retained action-time provenance payload, including a
        // later action that follows a host-only acceptance of an older
        // program. Indexing by the authenticated pair stamp keeps restoration
        // independent of flat accepted geometry and of history traversal
        // order.
        let mut host_inputs = BTreeMap::<LineageOpaqueId, DecodedLineageHostInputs>::new();
        match (accepted_parameters, accepted_snapshots) {
            (Some(parameters), Some(snapshots)) => retain_host_input_candidate(
                &mut host_inputs,
                DecodedLineageHostInputs::new(parameters.clone(), snapshots.clone()),
            )?,
            (None, None) => {}
            _ => return Err(LineageBridgeError::AcceptedExternalInputMismatch),
        }
        for inputs in self.host_input_ledger.decoded_entries()? {
            retain_host_input_candidate(&mut host_inputs, inputs)?;
        }
        for position in &positions {
            for document in
                std::iter::once(position.document()).chain(position.last_accepted_document())
            {
                for inputs in declared_host_inputs(document)? {
                    retain_host_input_candidate(&mut host_inputs, inputs)?;
                }
            }
        }

        let mut validated = BTreeSet::new();
        let mut validate_visible = |session: &LineageSession| -> Result<(), LineageBridgeError> {
            let Some(authority) = session.last_accepted() else {
                return Self::validate_session_accepted_external_inputs(session, None, None);
            };
            let key = (
                authority.lineage.document,
                authority.lineage.revision,
                authority.lineage.digest,
                authority.external_inputs.clone(),
                authority.materialization_digest,
            );
            if !validated.insert(key) {
                return Ok(());
            }
            let stamp = authority
                .external_inputs
                .as_ref()
                .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
            let inputs = host_inputs
                .get(stamp)
                .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
            Self::validate_session_accepted_external_inputs(
                session,
                Some(inputs.parameters()),
                Some(inputs.snapshots()),
            )
        };

        for position in &positions {
            validate_visible(position)?;
        }
        let required = self.accepted_external_input_stamps()?;
        let mut ledger = self.host_input_ledger.clone();
        for stamp in &required {
            let inputs = host_inputs
                .get(stamp)
                .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
            ledger.retain_accepted_pair(inputs.parameters(), inputs.snapshots(), &required)?;
        }
        ledger.retain_required(&required)?;
        self.host_input_ledger = ledger;
        Ok(())
    }

    /// Replaces caller-supplied latest-attempt metadata with evidence from a
    /// fresh owning-domain execution over the exact retained host inputs.
    /// A failed reconstruction preserves the separately authenticated last
    /// accepted program while recording only the engine-observed failed step
    /// and diagnostic.
    pub(super) fn reconstruct_current_evaluation_authority(
        &mut self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<(), LineageBridgeError> {
        let identity = self.session.identity();
        let external_inputs = external_input_stamp(parameters, snapshots)?;
        match super::lineage_evaluation::evaluate_lineage_session_cold_with_inputs(
            &self.session,
            parameters,
            snapshots,
        ) {
            Ok(evaluation) => {
                if evaluation.external_inputs() != &external_inputs {
                    return Err(LineageBridgeError::AcceptedExternalInputMismatch);
                }
                self.session.accept_current(
                    identity,
                    Some(external_inputs),
                    evaluation.materialization_digest(),
                )?;
                let required = self.accepted_external_input_stamps()?;
                self.host_input_ledger
                    .retain_accepted_pair(parameters, snapshots, &required)?;
            }
            Err(failure) => {
                let failed_step = failure.failed_step().ok_or_else(|| {
                    LineageBridgeError::AcceptedEvaluationRejected(format!(
                        "{}: {}",
                        failure.code(),
                        failure.message()
                    ))
                })?;
                self.session.reject_current(
                    identity,
                    Some(external_inputs),
                    LineageSemanticKey::new(failure.diagnostic())?,
                    vec![failed_step],
                )?;
                let required = self.accepted_external_input_stamps()?;
                self.host_input_ledger.retain_required(&required)?;
            }
        }
        Ok(())
    }

    /// Merges every owning-domain never-reuse cursor found in current,
    /// accepted, Undo, and Redo lineage programs. The returned checkpoint's
    /// authored current intent is unchanged; only its lifecycle cursors are
    /// advanced. This is the authority needed before a restored Redo branch
    /// can be abandoned by a new edit.
    pub(super) fn retained_history_lifecycle_checkpoint(
        &self,
    ) -> Result<LineageCheckpoint, LineageBridgeError> {
        let mut aggregate = self.materialize()?;
        let wire = serde_json::from_str::<Value>(&self.session.to_canonical_session_json()?)?;
        let root = wire
            .as_object()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "lineage session wire must be an object",
            ))?;
        let mut document_json = Vec::new();
        for field in ["document", "last_accepted_document"] {
            if let Some(json) = root.get(field).and_then(Value::as_str) {
                document_json.push(json);
            }
        }
        for direction in ["undo", "redo"] {
            let checkpoints = root.get(direction).and_then(Value::as_array).ok_or(
                LineageBridgeError::InvalidMaterialization(
                    "lineage session history must be an array",
                ),
            )?;
            for checkpoint in checkpoints {
                let checkpoint =
                    checkpoint
                        .as_object()
                        .ok_or(LineageBridgeError::InvalidMaterialization(
                            "lineage session history checkpoint must be an object",
                        ))?;
                for field in ["document", "last_accepted_document"] {
                    if let Some(json) = checkpoint.get(field).and_then(Value::as_str) {
                        document_json.push(json);
                    }
                }
            }
        }
        for json in document_json {
            let document = LineageDocument::from_json(json)?;
            let checkpoint = Self::materialize_document_for_policy(
                &document,
                geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
                &[],
            )?;
            aggregate.retain_every_lifecycle_cursor(&checkpoint)?;
        }
        if let Some(high_water) = self.session.auxiliary_high_water(&LineageSemanticKey::new(
            COMPUTED_EVALUATION_HIGH_WATER_KEY,
        )?) {
            aggregate.retain_computed_evaluation_high_water(high_water);
        }
        Ok(aggregate)
    }

    pub(super) fn validate_accepted_external_inputs(
        &self,
        parameters: Option<&ParameterBatch>,
        snapshots: Option<&ExternalSnapshotSet>,
    ) -> Result<(), LineageBridgeError> {
        Self::validate_session_accepted_external_inputs(&self.session, parameters, snapshots)
    }

    pub(super) fn current_evaluation_is_accepted(&self) -> bool {
        self.session.latest_attempt().is_some_and(|attempt| {
            attempt.target == self.session.identity()
                && matches!(attempt.disposition, LineageEvaluationDisposition::Accepted)
        })
    }

    pub(super) fn retain_validated_visible_accepted_external_inputs(
        &mut self,
        parameters: Option<&ParameterBatch>,
        snapshots: Option<&ExternalSnapshotSet>,
    ) -> Result<(), LineageBridgeError> {
        self.validate_accepted_external_inputs(parameters, snapshots)?;
        match (parameters, snapshots) {
            (Some(parameters), Some(snapshots)) => {
                let required = self.accepted_external_input_stamps()?;
                self.host_input_ledger
                    .retain_accepted_pair(parameters, snapshots, &required)
            }
            (None, None) => Ok(()),
            _ => Err(LineageBridgeError::AcceptedExternalInputMismatch),
        }
    }

    pub(super) fn reproduce_current_accepted_evaluation(
        &self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        let (Some(document), Some(authority)) = (
            self.session.last_accepted_document(),
            self.session.last_accepted(),
        ) else {
            return Ok(None);
        };
        if authority.lineage != document.identity() {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        }
        let current_inputs = external_input_stamp(parameters, snapshots)?;
        let authority_inputs = authority
            .external_inputs
            .as_ref()
            .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
        let (_, _historical_inputs, embedded_accepted_baseline, evaluation) = self
            .materialize_last_accepted_with_inputs()?
            .ok_or(LineageBridgeError::AcceptedExternalInputMismatch)?;
        if evaluation.external_inputs() != authority_inputs {
            return Err(LineageBridgeError::AcceptedExternalInputMismatch);
        }
        if evaluation.materialization_digest() != authority.materialization_digest {
            return Err(LineageBridgeError::AcceptedMaterializationMismatch);
        }
        if embedded_accepted_baseline {
            // This authority belongs to the older accepted branch embedded in
            // the imported root, never to the retained invalid branch even
            // though both intentionally share one lineage action identity.
            return Ok(None);
        }
        if authority_inputs != &current_inputs {
            // The historical authority has been authenticated under its own
            // exact payload, but it is not an accepted witness for the host's
            // current inputs. Continue through the ordinary current-program
            // solve and retain this authority only as the distinct fallback.
            return Ok(None);
        }
        let current = self.session.document();
        let same_program = current.id() == document.id()
            && current.evaluation_policy() == document.evaluation_policy()
            && current.steps().len() == document.steps().len()
            && document.steps().iter().zip(current.steps()).try_fold(
                true,
                |equivalent, (accepted, retained)| {
                    Ok::<_, LineageBridgeError>(
                        equivalent && steps_materialization_equivalent(accepted, retained)?,
                    )
                },
            )?;
        if !same_program {
            // Undo/Redo rebases the selected retained program to a fresh
            // revision while preserving older accepted authority. The exact
            // historical evaluation above must still be authenticated, but it
            // is current-design authority only when the declarative programs
            // are materialization-equivalent; otherwise the caller must solve
            // the selected current program and retain the older acceptance as
            // a distinct fallback.
            return Ok(None);
        }
        Ok(Some(evaluation))
    }

    fn validate_session_accepted_external_inputs(
        session: &LineageSession,
        parameters: Option<&ParameterBatch>,
        snapshots: Option<&ExternalSnapshotSet>,
    ) -> Result<(), LineageBridgeError> {
        match (
            session.last_accepted_document(),
            session.last_accepted(),
            parameters,
            snapshots,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(_document), Some(authority), Some(parameters), Some(snapshots)) => {
                let expected = external_input_stamp(parameters, snapshots)?;
                if authority.external_inputs.as_ref() != Some(&expected) {
                    return Err(LineageBridgeError::AcceptedExternalInputMismatch);
                }
                // A lineage checkpoint is only equation-free retained intent;
                // its embedded accepted sketch bytes are a disposable legacy
                // cache and may legitimately lag a host-input-only solve. Rebuild
                // the exact accepted program through the ordinary sketch and
                // computed-feature owners before trusting its authority digest.
                let evaluation = super::lineage_evaluation::
                    evaluate_lineage_accepted_authority_cold_with_inputs(
                        session,
                        parameters,
                        snapshots,
                    )
                    .map_err(|failure| {
                        LineageBridgeError::AcceptedEvaluationRejected(format!(
                            "{}: {}",
                            failure.code(),
                            failure.message()
                        ))
                    })?;
                if authority.materialization_digest != evaluation.materialization_digest() {
                    return Err(LineageBridgeError::AcceptedMaterializationMismatch);
                }
                Ok(())
            }
            _ => Err(LineageBridgeError::AcceptedExternalInputMismatch),
        }
    }

    fn materialize_with_failed_steps(
        &self,
        failed_steps: &[LineageStepId],
    ) -> Result<LineageCheckpoint, LineageBridgeError> {
        let mut strict = self.materialize_for_policy(
            geosolve_sketch_lineage::LineageEvaluationPolicy::StrictChronological,
            failed_steps,
        )?;
        if self.session.document().evaluation_policy()
            == geosolve_sketch_lineage::LineageEvaluationPolicy::DependencyLocal
        {
            let local = self.materialize_for_policy(
                geosolve_sketch_lineage::LineageEvaluationPolicy::DependencyLocal,
                failed_steps,
            )?;
            if semantic_structural_value(&strict)? != semantic_structural_value(&local)? {
                return Err(LineageBridgeError::ReproductionMismatch);
            }
        }
        if let Some(high_water) = self.session.auxiliary_high_water(&LineageSemanticKey::new(
            COMPUTED_EVALUATION_HIGH_WATER_KEY,
        )?) {
            strict.retain_computed_evaluation_high_water(high_water);
        }
        Ok(strict)
    }

    fn materialize_for_policy(
        &self,
        policy: geosolve_sketch_lineage::LineageEvaluationPolicy,
        failed_steps: &[LineageStepId],
    ) -> Result<LineageCheckpoint, LineageBridgeError> {
        Self::materialize_document_for_policy(self.session.document(), policy, failed_steps)
    }

    fn materialize_document_for_policy(
        document: &LineageDocument,
        policy: geosolve_sketch_lineage::LineageEvaluationPolicy,
        failed_steps: &[LineageStepId],
    ) -> Result<LineageCheckpoint, LineageBridgeError> {
        Self::materialize_document_prefixes_for_policy(document, policy, failed_steps)?
            .pop()
            .map(|prefix| prefix.checkpoint)
            .ok_or(LineageBridgeError::MissingBaseline)
    }

    fn materialize_document_prefixes_for_policy(
        document: &LineageDocument,
        policy: geosolve_sketch_lineage::LineageEvaluationPolicy,
        failed_steps: &[LineageStepId],
    ) -> Result<Vec<LineageMaterializedPrefix>, LineageBridgeError> {
        let mut checkpoint = None;
        let mut prefixes = Vec::new();
        let plan = document.dependency_plan_for(policy, failed_steps.iter().copied())?;
        for (step, plan) in document.steps().iter().zip(plan) {
            if !matches!(
                plan.state,
                geosolve_sketch_lineage::LineageStepEvaluationState::Ready
            ) {
                continue;
            }
            let host_inputs = apply_materialization_step(document, &mut checkpoint, step)?;
            prefixes.push(LineageMaterializedPrefix {
                step: step.id,
                checkpoint: checkpoint
                    .as_ref()
                    .ok_or(LineageBridgeError::MissingBaseline)?
                    .clone(),
                host_inputs,
            });
        }
        if checkpoint.is_none() {
            return Err(LineageBridgeError::MissingBaseline);
        }
        Ok(prefixes)
    }

    fn materialize_document_prefix_for_policy(
        document: &LineageDocument,
        policy: geosolve_sketch_lineage::LineageEvaluationPolicy,
        step_count: usize,
        failed_steps: &[LineageStepId],
    ) -> Result<Option<LineageMaterializedPrefix>, LineageBridgeError> {
        let mut checkpoint = None;
        let mut prefix = None;
        let plan = document.dependency_plan_for(policy, failed_steps.iter().copied())?;
        for (step, plan) in document.steps().iter().zip(plan).take(step_count) {
            if matches!(
                plan.state,
                geosolve_sketch_lineage::LineageStepEvaluationState::Ready
            ) {
                let host_inputs = apply_materialization_step(document, &mut checkpoint, step)?;
                prefix = Some(LineageMaterializedPrefix {
                    step: step.id,
                    checkpoint: checkpoint
                        .as_ref()
                        .ok_or(LineageBridgeError::MissingBaseline)?
                        .clone(),
                    host_inputs,
                });
            }
        }
        Ok(prefix)
    }

    fn rewrite_owners(
        &mut self,
        before: &Value,
        after_value: &Value,
        changed: &[StructuralDeltaOperation],
        after: &LineageCheckpoint,
    ) -> Result<Vec<LineageStepId>, LineageBridgeError> {
        let mut replacements = BTreeMap::<
            LineageStepId,
            (
                LineageStepRewrite,
                Vec<(StructuralTargetKey, StructuralDeltaOperation)>,
                BTreeMap<
                    (LineageOutputId, LineageOutputKind, LineageSemanticKey),
                    AuthoredOutputFieldValue,
                >,
            ),
        >::new();
        let ownership = LineageMaterializationMap::derive(self.session.document())?;
        for change in changed {
            let target = change.target_key();
            let owner = self
                .owner_for_change(change)
                .ok_or(LineageBridgeError::MissingOwner)?;
            let (_, owner_changes, owner_fields) =
                replacements.entry(owner.id).or_insert_with(|| {
                    (
                        LineageStepRewrite {
                            label: owner.label.clone(),
                            action: owner.action.clone(),
                        },
                        Vec::new(),
                        BTreeMap::new(),
                    )
                });
            owner_changes.push((target, change.clone()));
            for leaf in direct_materialized_leaves_for_change(before, after_value, change)? {
                let Some(field_owner) = ownership.owner_for_leaf(&leaf) else {
                    continue;
                };
                if field_owner.step != owner.id
                    || field_owner.output.document != self.session.document().id()
                    || field_owner.output.step != owner.id
                    || field_owner.field != leaf.key
                {
                    continue;
                }
                let field = AuthoredOutputFieldValue {
                    output: field_owner.output.output,
                    kind: field_owner.output.kind,
                    field: field_owner.field.clone(),
                    value: materialized_leaf_value(after_value, &leaf)?,
                };
                owner_fields.insert((field.output, field.kind, field.field.clone()), field);
            }
            if let Some(field) = logical_output_field_for_change(owner, after_value, change)? {
                owner_fields.insert((field.output, field.kind, field.field.clone()), field);
            }
        }
        for (owner, (replacement, owner_changes, owner_fields)) in &mut replacements {
            let output_fields = owner_fields.values().cloned().collect::<Vec<_>>();
            let output_raw = step_output_raw_map(self.session.document(), *owner)?;
            replace_action_delta_targets(
                &mut replacement.action,
                owner_changes,
                &output_fields,
                &output_raw,
            )?;
        }
        // An imported entity is legitimately owned by the baseline step. In
        // that case the semantic owner rewrite and the baseline lifecycle /
        // materialization-evidence refresh must be folded into the same step
        // replacement. Emitting two Rewrite mutations for that step would let
        // the later high-water rewrite overwrite the semantic edit.
        let baseline_evidence_merged = replacements
            .values_mut()
            .find(|(replacement, _, _)| {
                matches!(
                    replacement.action,
                    LineageActionDefinition::ImportedBaseline { .. }
                )
            })
            .map(|(replacement, _, _)| {
                retain_baseline_materialization_evidence(&mut replacement.action, after)
            })
            .transpose()?
            .is_some();
        let owners = replacements.keys().copied().collect::<Vec<_>>();
        let mut mutations = replacements
            .into_iter()
            .map(|(step, (replacement, _, _))| LineageMutation::Rewrite {
                step,
                replacement: Box::new(replacement),
            })
            .collect::<Vec<_>>();
        if !baseline_evidence_merged
            && let Some(rewrite) = self.baseline_high_water_rewrite(after)?
        {
            mutations.push(rewrite);
        }
        self.session
            .apply_patch(LineagePatch::new(self.session.identity(), mutations))?;
        Ok(owners)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exact-CAS authority pass keeps leaf ownership, downstream cache refresh, baseline evidence, and atomic mutation composition adjacent"
    )]
    fn rewrite_direct_owners(
        &mut self,
        before: &Value,
        after: &Value,
        changed: &[StructuralDeltaOperation],
        after_checkpoint: &LineageCheckpoint,
    ) -> Result<Vec<LineageStepId>, LineageBridgeError> {
        if changed.is_empty() {
            return Err(LineageBridgeError::MissingOwner);
        }
        let ownership = LineageMaterializationMap::derive(self.session.document())?;
        if ownership.lineage() != self.session.identity() {
            return Err(LineageBridgeError::MissingOwner);
        }

        let step_indices = self
            .session
            .document()
            .steps()
            .iter()
            .enumerate()
            .map(|(index, step)| (step.id, index))
            .collect::<BTreeMap<_, _>>();
        let mut replacements = BTreeMap::<LineageStepId, PendingDirectStepRewrite>::new();
        let mut changed_by_target =
            BTreeMap::<StructuralTargetKey, StructuralDeltaOperation>::new();
        for change in changed {
            let target = change.target_key();
            if changed_by_target
                .insert(target.clone(), change.clone())
                .is_some()
            {
                return Err(LineageBridgeError::InvalidMaterialization(
                    "direct manipulation produced duplicate structural targets",
                ));
            }
            let leaves = direct_materialized_leaves_for_change(before, after, change)?;
            if leaves.is_empty() {
                return Err(LineageBridgeError::MissingOwner);
            }
            let mut change_owners = BTreeMap::<
                LineageStepId,
                BTreeMap<
                    (LineageOutputId, LineageOutputKind, LineageSemanticKey),
                    AuthoredOutputFieldValue,
                >,
            >::new();
            for leaf in leaves {
                let owner = ownership
                    .owner_for_leaf(&leaf)
                    .ok_or(LineageBridgeError::MissingOwner)?;
                if owner.field != leaf.key
                    || owner.output.document != self.session.document().id()
                    || owner.output.step != owner.step
                {
                    return Err(LineageBridgeError::MissingOwner);
                }
                let field = AuthoredOutputFieldValue {
                    output: owner.output.output,
                    kind: owner.output.kind,
                    field: owner.field.clone(),
                    value: materialized_leaf_value(after, &leaf)?,
                };
                change_owners
                    .entry(owner.step)
                    .or_default()
                    .insert((field.output, field.kind, field.field.clone()), field);
            }
            if change_owners.is_empty() {
                return Err(LineageBridgeError::MissingOwner);
            }
            for (owner, fields) in change_owners {
                let step = self
                    .session
                    .document()
                    .step(owner)
                    .filter(|step| step.state == geosolve_sketch_lineage::LineageStepState::Live)
                    .ok_or(LineageBridgeError::MissingOwner)?;
                if !action_owns_structural_target(&step.action, change)? {
                    return Err(LineageBridgeError::MissingOwner);
                }
                let pending =
                    replacements
                        .entry(owner)
                        .or_insert_with(|| PendingDirectStepRewrite {
                            replacement: LineageStepRewrite {
                                label: step.label.clone(),
                                action: step.action.clone(),
                            },
                            owner_changes: Vec::new(),
                            owner_fields: BTreeMap::new(),
                            cache_changes: Vec::new(),
                        });
                // Structural entity operations carry a complete validated
                // entity snapshot, while writable authority remains leaf
                // precise. When one accepted projection changes leaves owned
                // by several chronological snapshots, rewrite that same
                // structural target in every affected owner inside the one
                // CAS patch. This keeps their replay composition coherent;
                // unrelated structural targets and action intent bytes remain
                // untouched, and cold reproduction still gates publication.
                pending.owner_changes.push((target.clone(), change.clone()));
                pending.owner_fields.extend(fields);
            }
        }

        let owners = replacements.keys().copied().collect::<Vec<_>>();
        let mut earliest_owner = BTreeMap::<StructuralTargetKey, usize>::new();
        for (owner, pending) in &replacements {
            let index = *step_indices
                .get(owner)
                .ok_or(LineageBridgeError::MissingOwner)?;
            for (target, _) in &pending.owner_changes {
                earliest_owner
                    .entry(target.clone())
                    .and_modify(|earliest| *earliest = (*earliest).min(index))
                    .or_insert(index);
            }
        }

        // Every complete entity snapshot replayed after the earliest changed
        // leaf owner is a derived cache dependency of that owner. Refresh it
        // in the same exact-CAS patch unless that step already owns a changed
        // leaf on the target. Its semantic intent, owner fields, outputs, and
        // writable declarations remain byte-for-byte unchanged.
        for (index, step) in self.session.document().steps().iter().enumerate() {
            if step.state != geosolve_sketch_lineage::LineageStepState::Live
                || matches!(
                    step.action,
                    LineageActionDefinition::ImportedBaseline { .. }
                )
            {
                continue;
            }
            let compiled_targets = action_delta(&step.action)?
                .iter()
                .map(StructuralDeltaOperation::target_key)
                .collect::<BTreeSet<_>>();
            let cache_changes = changed_by_target
                .iter()
                .filter(|(target, _)| {
                    earliest_owner
                        .get(*target)
                        .is_some_and(|earliest| index > *earliest)
                        && compiled_targets.contains(*target)
                        && replacements.get(&step.id).is_none_or(|pending| {
                            !pending
                                .owner_changes
                                .iter()
                                .any(|(owned, _)| owned == *target)
                        })
                })
                .map(|(target, operation)| (target.clone(), operation.clone()))
                .collect::<Vec<_>>();
            if cache_changes.is_empty() {
                continue;
            }
            replacements
                .entry(step.id)
                .or_insert_with(|| PendingDirectStepRewrite {
                    replacement: LineageStepRewrite {
                        label: step.label.clone(),
                        action: step.action.clone(),
                    },
                    owner_changes: Vec::new(),
                    owner_fields: BTreeMap::new(),
                    cache_changes: Vec::new(),
                })
                .cache_changes
                .extend(cache_changes);
        }

        for (owner, pending) in &mut replacements {
            if !pending.owner_changes.is_empty() {
                let output_field_values =
                    pending.owner_fields.values().cloned().collect::<Vec<_>>();
                let output_raw = step_output_raw_map(self.session.document(), *owner)?;
                replace_action_delta_targets(
                    &mut pending.replacement.action,
                    &pending.owner_changes,
                    &output_field_values,
                    &output_raw,
                )?;
            }
            refresh_compiled_delta_targets(
                &mut pending.replacement.action,
                &pending.cache_changes,
            )?;
        }

        let baseline_evidence_merged = replacements
            .values_mut()
            .find(|pending| {
                matches!(
                    pending.replacement.action,
                    LineageActionDefinition::ImportedBaseline { .. }
                )
            })
            .map(|pending| {
                retain_baseline_materialization_evidence(
                    &mut pending.replacement.action,
                    after_checkpoint,
                )
            })
            .transpose()?
            .is_some();
        let mut mutations = replacements
            .into_iter()
            .map(|(step, pending)| LineageMutation::Rewrite {
                step,
                replacement: Box::new(pending.replacement),
            })
            .collect::<Vec<_>>();
        if !baseline_evidence_merged
            && let Some(rewrite) = self.baseline_high_water_rewrite(after_checkpoint)?
        {
            mutations.push(rewrite);
        }
        // This one exact-CAS patch is the coordinator's `RewriteSteps`
        // transaction. Every owner replacement is validated together and the
        // lineage session creates exactly one history position.
        self.session
            .apply_patch(LineagePatch::new(self.session.identity(), mutations))?;
        Ok(owners)
    }

    fn delta_has_complete_owner_map(
        &self,
        before: &Value,
        delta: &[StructuralDeltaOperation],
    ) -> bool {
        !delta.is_empty()
            // Removing one persistent identity is a new lifecycle action, not
            // a field rewrite of the step that originally created/imported
            // it.  Keeping the predecessor action intact preserves its
            // authenticated output manifest; `manifest_for_delta` then emits
            // the exact Retired flow and Undo removes that later action.
            && !delta
                .iter()
                .any(|change| structural_change_transitions_identity(before, change))
            && delta
                .iter()
                .all(|change| self.owner_for_change(change).is_some())
    }

    fn owner_for_change(&self, change: &StructuralDeltaOperation) -> Option<&LineageStep> {
        let target = change.target_key();
        if let Some(owner) = self.session.document().steps().iter().rev().find(|step| {
            authored_action_delta(&step.action).is_ok_and(|delta| {
                delta
                    .iter()
                    .any(|operation| operation.target_key() == target)
            })
        }) {
            return Some(owner);
        }

        // Imported-baseline actions deliberately have no compiled action
        // delta: their opaque payload is the honest owner of every imported
        // persistent entity. Resolve an edit of an existing entity through
        // its exact typed materialized port. A newly created entity has no
        // such port and therefore still becomes a new semantic action.
        if let StructuralDeltaOperation::UpsertEntity { path, id, .. }
        | StructuralDeltaOperation::RemoveEntity { path, id } = change
            && let Some((kind, _)) = entity_kind_for_collection(path.last().map(String::as_str))
        {
            let index = materialized_output_index(self.session.document());
            if let Some(output) = index.by_kind_and_raw.get(&(kind, id.clone())) {
                return self.session.document().step(output.step);
            }
        }

        // A curve's Profile/Construction association is a writable semantic
        // leaf of the recipe that owns that curve, even though draft-v5 stores
        // non-default roles in a side table. Resolve it through the exact curve
        // port; never through coordinates, table order, or a generic Edit step.
        let curve = geometry_role_curve(change)?;
        let index = materialized_output_index(self.session.document());
        let output = index
            .by_kind_and_raw
            .get(&(LineageOutputKind::Curve, curve.to_owned()))?;
        self.session.document().step(output.step)
    }

    fn record_evaluation(
        &mut self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        accepted_current: bool,
        failed_step: LineageStepId,
        expected_accepted: Option<(&str, bool)>,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        let previous_session = self.session.clone();
        let previous_ledger = self.host_input_ledger.clone();
        let result = (|| {
            let identity = self.session.identity();
            let external_inputs = external_input_stamp(parameters, snapshots)?;
            // Accepted lineage authority is always the result of a fresh
            // owning-domain rebuild, even when the transaction-local live
            // solve reported rejection. The live result remains useful as a
            // staged hint, but it cannot overrule strict chronological
            // materialization in either direction.
            //
            // Evaluate the retained program itself, without session-local
            // auxiliary allocator high-waters. Those cursors protect future
            // revision-local generated identities but are deliberately not
            // part of reproducible accepted lineage authority.
            let evaluation_session = LineageSession::new(self.session.document().clone())?;
            let already_accepted_same_authority =
                self.session.last_accepted().is_some_and(|authority| {
                    authority.lineage == identity
                        && authority.external_inputs.as_ref() == Some(&external_inputs)
                });
            match super::lineage_evaluation::evaluate_lineage_session_cold_with_inputs(
                &evaluation_session,
                parameters,
                snapshots,
            ) {
                Ok(evaluation) if accepted_current || !already_accepted_same_authority => {
                    if evaluation.external_inputs() != &external_inputs {
                        return Err(LineageBridgeError::AcceptedExternalInputMismatch);
                    }
                    if let Some((expected_json, expected_is_draft_v5)) = expected_accepted
                        && !accepted_materialization_semantically_matches(
                            expected_json,
                            expected_is_draft_v5,
                            evaluation.accepted_sketch_json(),
                            evaluation.accepted_uses_draft_v5(),
                        )?
                    {
                        return Err(LineageBridgeError::AcceptedMaterializationMismatch);
                    }
                    self.session.accept_current(
                        identity,
                        Some(external_inputs),
                        evaluation.materialization_digest(),
                    )?;
                    let required = self.accepted_external_input_stamps()?;
                    self.host_input_ledger
                        .retain_accepted_pair(parameters, snapshots, &required)?;
                    Ok(Some(evaluation))
                }
                // A failed no-history solve request (for example a temporary
                // drag target) is not represented by the lineage program or
                // immutable host-input stamp. Cold acceptance of the already
                // accepted same pair therefore cannot authenticate that newer
                // request. Retain the live rejection after still consulting
                // the strict oracle.
                Ok(_) => {
                    self.session.reject_current(
                        identity,
                        Some(external_inputs),
                        LineageSemanticKey::new("owning-domain-rejected")?,
                        vec![failed_step],
                    )?;
                    let required = self.accepted_external_input_stamps()?;
                    self.host_input_ledger.retain_required(&required)?;
                    Ok(None)
                }
                Err(failure) if accepted_current => {
                    Err(LineageBridgeError::AcceptedEvaluationRejected(format!(
                        "{}: {}",
                        failure.code(),
                        failure.message()
                    )))
                }
                Err(failure) => {
                    let cold_failed_step = failure.failed_step().unwrap_or(failed_step);
                    self.session.reject_current(
                        identity,
                        Some(external_inputs),
                        LineageSemanticKey::new(failure.diagnostic())?,
                        vec![cold_failed_step],
                    )?;
                    let required = self.accepted_external_input_stamps()?;
                    self.host_input_ledger.retain_required(&required)?;
                    Ok(None)
                }
            }
        })();
        if result.is_err() {
            self.session = previous_session;
            self.host_input_ledger = previous_ledger;
        }
        result
    }

    fn record_embedded_imported_accepted_evaluation(
        &mut self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        expected_accepted: (&str, bool),
    ) -> Result<(), LineageBridgeError> {
        let previous_session = self.session.clone();
        let previous_ledger = self.host_input_ledger.clone();
        let result = (|| {
            let identity = self.session.identity();
            let external_inputs = external_input_stamp(parameters, snapshots)?;
            let evaluation_session = LineageSession::new(self.session.document().clone())?;
            let evaluation = super::lineage_evaluation::
                evaluate_lineage_embedded_accepted_baseline_cold_with_inputs(
                    &evaluation_session,
                    parameters,
                    snapshots,
                )
                .map_err(|failure| {
                    LineageBridgeError::AcceptedEvaluationRejected(format!(
                        "{}: {}",
                        failure.code(),
                        failure.message()
                    ))
                })?;
            if evaluation.external_inputs() != &external_inputs {
                return Err(LineageBridgeError::AcceptedExternalInputMismatch);
            }
            if !accepted_materialization_semantically_matches(
                expected_accepted.0,
                expected_accepted.1,
                evaluation.accepted_sketch_json(),
                evaluation.accepted_uses_draft_v5(),
            )? {
                return Err(LineageBridgeError::AcceptedMaterializationMismatch);
            }
            self.session.accept_current(
                identity,
                Some(external_inputs),
                evaluation.materialization_digest(),
            )?;
            let required = self.accepted_external_input_stamps()?;
            self.host_input_ledger
                .retain_accepted_pair(parameters, snapshots, &required)
        })();
        if result.is_err() {
            self.session = previous_session;
            self.host_input_ledger = previous_ledger;
        }
        result
    }

    pub(super) fn publish_current_evaluation(
        &mut self,
        checkpoint: &RestoreCheckpoint,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        accepted_current: bool,
    ) -> Result<Option<LineageDomainEvaluationEvidence>, LineageBridgeError> {
        #[cfg(test)]
        if self
            .reject_next_evaluation_publication
            .swap(false, Ordering::SeqCst)
        {
            return Err(LineageBridgeError::InjectedEvaluationPublicationFailure);
        }
        self.retain_computed_evaluation_high_water(checkpoint.computed_evaluation_high_water())?;
        let failed_step = self
            .session
            .document()
            .steps()
            .iter()
            .rev()
            .find(|step| matches!(step.state, geosolve_sketch_lineage::LineageStepState::Live))
            .map_or_else(|| self.session.document().steps()[0].id, |step| step.id);
        self.record_evaluation(parameters, snapshots, accepted_current, failed_step, None)
    }
}

/// Authenticates every editor-generated semantic authority declaration in one
/// lineage document without materializing or mutating it. Generic low-level
/// actions remain a valid RPC language when they carry none of the private
/// workbench materialization parameters.
pub(super) fn validate_lineage_document_semantics(
    document: &LineageDocument,
) -> Result<(), LineageBridgeError> {
    document.validate()?;
    let mut checkpoint = None;
    for (position, step) in document.steps().iter().enumerate() {
        let result = (|| match &step.action {
            LineageActionDefinition::ImportedBaseline { baseline } => {
                validate_imported_baseline_manifest(document, step, baseline)?;
                let imported: ImportedCoordinatorBaseline =
                    serde_json::from_str(&baseline.payload)?;
                checkpoint = Some(imported.checkpoint);
                Ok(())
            }
            action if has_workbench_materialization_parameters(action) => {
                validate_editor_generated_action_manifest(document, position, step, &mut checkpoint)
            }
            _ => Ok(()),
        })();
        result.map_err(|source| LineageBridgeError::StepMaterialization {
            step: step.id,
            source: Box::new(source),
        })?;
    }
    Ok(())
}

/// Recompiles the complete action-local port manifest from the authenticated
/// predecessor materialization and the action's structural recipe. The
/// retained port declarations are outputs of this compiler, never inputs to
/// it. Applying every generated recipe here regardless of current suppression
/// or tombstone state preserves the chronological authoring context needed to
/// authenticate later declarations without publishing any geometry.
fn validate_editor_generated_action_manifest(
    document: &LineageDocument,
    position: usize,
    step: &LineageStep,
    checkpoint: &mut Option<LineageCheckpoint>,
) -> Result<(), LineageBridgeError> {
    let current = checkpoint
        .as_mut()
        .ok_or(LineageBridgeError::MissingBaseline)?;
    action_host_input_provenance(&step.action)?.decode()?;

    let before = current.structural_value()?;
    let compiled_operations = action_delta(&step.action)?;
    let mut after = before.clone();
    apply_authored_operations(&mut after, &compiled_operations)?;

    let index =
        materialized_output_index_for_steps(document.id(), document.steps()[..position].iter());
    let next_output = step.outputs.iter().map(|output| output.id).min().ok_or(
        LineageBridgeError::InvalidMaterialization("editor-generated action has no output"),
    )?;
    let next_reservation = step
        .reservations
        .iter()
        .map(|reservation| reservation.id)
        .min()
        .unwrap_or_else(|| LineageReservationId::from_raw(1));
    let mut expected = manifest_for_delta_with_index(
        document.id(),
        &index,
        step.id,
        next_output,
        next_reservation,
        &before,
        &after,
        &compiled_operations,
    )?;
    enrich_persisted_action_manifest(&index, &step.action, &mut expected)?;
    expected
        .inputs
        .sort_by(|left, right| left.key.cmp(&right.key));
    validate_exact_step_manifest(step, &expected)?;

    // Direct owner rewrites retain stable ports while their action-local
    // output fields override disposable complete-entity snapshots. Only use
    // those fields after the complete identity manifest above is trusted.
    let effective_operations = action_delta_for_step(document, step)?;
    let owned = declared_materialized_writable_leaves(document, step)?;
    validate_compiled_operation_values(
        &before,
        &authored_action_delta(&step.action)?,
        &effective_operations,
        &owned,
    )?;
    let mut effective_after = before;
    apply_authored_operations(&mut effective_after, &effective_operations)?;
    current.replace_structural_value(&effective_after)
}

/// Authenticates disposable complete-entity snapshots against the durable
/// action-local semantic values and the exact chronological predecessor.
/// Existing-entity snapshots may refresh predecessor-owned leaves, but they
/// cannot smuggle another value or structural change. Created entities and
/// non-entity operations remain exact. A compiled semantic no-op is safe
/// because it cannot change the predecessor even if a later override made the
/// original cached value stale.
fn validate_compiled_operation_values(
    before: &Value,
    authored: &[StructuralDeltaOperation],
    effective: &[StructuralDeltaOperation],
    owned: &BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    if authored.len() != effective.len() {
        return Err(LineageBridgeError::InvalidMaterialization(
            "compiled materialization operation count differs from authored intent",
        ));
    }
    let mut predecessor = before.clone();
    for (authored, effective) in authored.iter().zip(effective) {
        if !operation_shapes_match(authored, effective) {
            return Err(LineageBridgeError::InvalidMaterialization(
                "compiled materialization operation shape differs from authored intent",
            ));
        }
        let mut next = predecessor.clone();
        apply_authored_operations(&mut next, std::slice::from_ref(effective))?;
        if next != predecessor {
            match (authored, effective) {
                (
                    StructuralDeltaOperation::UpsertEntity {
                        path,
                        id,
                        value: authored_value,
                        ..
                    },
                    StructuralDeltaOperation::UpsertEntity {
                        value: effective_value,
                        ..
                    },
                ) if entity_kind_for_collection(path.last().map(String::as_str)).is_some()
                    && entity_value_at_path(&predecessor, path, id).is_some() =>
                {
                    validate_existing_entity_rebase(
                        &predecessor,
                        path,
                        id,
                        authored_value,
                        effective_value,
                        owned,
                    )?;
                }
                _ if authored == effective => {}
                _ => {
                    return Err(LineageBridgeError::InvalidMaterialization(
                        "compiled materialization value differs from authored intent",
                    ));
                }
            }
        }
        predecessor = next;
    }
    Ok(())
}

fn operation_shapes_match(
    authored: &StructuralDeltaOperation,
    compiled: &StructuralDeltaOperation,
) -> bool {
    match (authored, compiled) {
        (
            StructuralDeltaOperation::SetField { path: left, .. },
            StructuralDeltaOperation::SetField { path: right, .. },
        )
        | (
            StructuralDeltaOperation::RemoveField { path: left },
            StructuralDeltaOperation::RemoveField { path: right },
        ) => left == right,
        (
            StructuralDeltaOperation::UpsertEntity {
                path: left_path,
                id: left_id,
                before: left_before,
                ..
            },
            StructuralDeltaOperation::UpsertEntity {
                path: right_path,
                id: right_id,
                before: right_before,
                ..
            },
        ) => left_path == right_path && left_id == right_id && left_before == right_before,
        (
            StructuralDeltaOperation::RemoveEntity {
                path: left_path,
                id: left_id,
            },
            StructuralDeltaOperation::RemoveEntity {
                path: right_path,
                id: right_id,
            },
        ) => left_path == right_path && left_id == right_id,
        _ => false,
    }
}

fn declared_materialized_writable_leaves(
    document: &LineageDocument,
    step: &LineageStep,
) -> Result<BTreeSet<LineageMaterializedLeaf>, LineageBridgeError> {
    let index = materialized_output_index(document);
    step.writable_leaves
        .iter()
        .filter_map(|writable| {
            let output = step
                .outputs
                .iter()
                .find(|output| output.id == writable.output)?;
            reservation_kind_for_output(output.kind)?;
            Some((output, writable))
        })
        .map(|(output, writable)| {
            let reference = LineageOutputRef {
                document: document.id(),
                step: step.id,
                output: output.id,
                kind: output.kind,
            };
            let raw = index
                .raw_by_ref
                .get(&reference)
                .ok_or(LineageBridgeError::MissingOwner)?;
            materialized_leaf(output.kind, raw, writable.key.as_str())
        })
        .collect()
}

fn validate_existing_entity_rebase(
    predecessor: &Value,
    path: &[String],
    id: &str,
    authored: &Value,
    effective: &Value,
    owned: &BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    let (kind, _) = entity_kind_for_collection(path.last().map(String::as_str))
        .ok_or(LineageBridgeError::MissingOwner)?;
    let before =
        entity_value_at_path(predecessor, path, id).ok_or(LineageBridgeError::MissingOwner)?;
    let authored_shape = normalized_rebased_entity_shape(predecessor, authored, kind, id)?;
    let effective_shape = normalized_rebased_entity_shape(predecessor, effective, kind, id)?;
    if authored_shape != effective_shape {
        return Err(LineageBridgeError::InvalidMaterialization(
            "compiled existing-entity structure differs from authored intent",
        ));
    }
    validate_existing_entity_leaf_values(predecessor, before, authored, effective, kind, id, owned)
}

fn validate_existing_entity_leaf_values(
    predecessor_root: &Value,
    before: &Value,
    authored: &Value,
    effective: &Value,
    kind: LineageOutputKind,
    id: &str,
    owned: &BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    let mut before_leaves = BTreeMap::new();
    collect_entity_leaf_values(&mut Vec::new(), before, &mut before_leaves)?;
    let mut authored_leaves = BTreeMap::new();
    collect_entity_leaf_values(&mut Vec::new(), authored, &mut authored_leaves)?;
    let mut effective_leaves = BTreeMap::new();
    collect_entity_leaf_values(&mut Vec::new(), effective, &mut effective_leaves)?;
    for key in before_leaves
        .keys()
        .chain(authored_leaves.keys())
        .chain(effective_leaves.keys())
        .collect::<BTreeSet<_>>()
    {
        let leaf = materialized_leaf(kind, id, key.as_str())?;
        let expected = if owned.contains(&leaf) {
            authored_leaves.get(key)
        } else {
            before_leaves.get(key)
        };
        if effective_leaves.get(key) != expected {
            return Err(LineageBridgeError::InvalidMaterialization(
                "compiled existing-entity leaf is neither owned intent nor predecessor cache",
            ));
        }
    }
    validate_nested_existing_entity_leaf_values(predecessor_root, authored, effective, owned)
}

fn validate_nested_existing_entity_leaf_values(
    predecessor_root: &Value,
    authored: &Value,
    effective: &Value,
    owned: &BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    match (authored, effective) {
        (Value::Object(authored), Value::Object(effective)) => {
            for (field, authored_value) in authored {
                let effective_value = effective
                    .get(field)
                    .ok_or(LineageBridgeError::MissingOwner)?;
                if let Some((kind, reservation_kind)) = entity_kind_for_collection(Some(field)) {
                    let authored_values = authored_value
                        .as_array()
                        .ok_or(LineageBridgeError::MissingOwner)?;
                    let effective_values = effective_value
                        .as_array()
                        .ok_or(LineageBridgeError::MissingOwner)?;
                    for (authored_child, effective_child) in
                        authored_values.iter().zip(effective_values)
                    {
                        let Some(id) = entity_id(authored_child) else {
                            continue;
                        };
                        let Some(before_child) = find_materialized_entity(
                            &mut Vec::new(),
                            predecessor_root,
                            reservation_kind,
                            id,
                        ) else {
                            continue;
                        };
                        validate_existing_entity_leaf_values(
                            predecessor_root,
                            before_child,
                            authored_child,
                            effective_child,
                            kind,
                            id,
                            owned,
                        )?;
                    }
                } else {
                    validate_nested_existing_entity_leaf_values(
                        predecessor_root,
                        authored_value,
                        effective_value,
                        owned,
                    )?;
                }
            }
            Ok(())
        }
        (Value::Array(authored), Value::Array(effective)) => {
            for (authored, effective) in authored.iter().zip(effective) {
                validate_nested_existing_entity_leaf_values(
                    predecessor_root,
                    authored,
                    effective,
                    owned,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn normalized_rebased_entity_shape(
    predecessor_root: &Value,
    value: &Value,
    kind: LineageOutputKind,
    id: &str,
) -> Result<Value, LineageBridgeError> {
    let mut normalized = value.clone();
    redact_direct_entity_leaf_values(&mut Vec::new(), &mut normalized);
    normalize_nested_existing_entity_shapes(predecessor_root, &mut normalized)?;
    let _ = (kind, id);
    Ok(normalized)
}

fn redact_direct_entity_leaf_values(path: &mut Vec<String>, value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (field, value) in object {
                if matches!(
                    field.as_str(),
                    "id" | "source_id" | "next_span_id" | "span_ids"
                ) {
                    continue;
                }
                if matches!(field.as_str(), "domain" | "neighborhood") {
                    // The writable-leaf compiler treats tagged branch values
                    // atomically. Redact the same atomic boundary here so a
                    // legitimate variant rewrite is not mistaken for an
                    // unauthorized entity-shape change.
                    *value = Value::Null;
                    continue;
                }
                if value.as_array().is_some_and(|values| {
                    entity_kind_for_collection(Some(field)).is_some()
                        && values.iter().all(|value| entity_id(value).is_some())
                }) {
                    continue;
                }
                path.push(field.clone());
                redact_direct_entity_leaf_values(path, value);
                path.pop();
            }
        }
        Value::Array(values)
            if values.len() == 2 && values.iter().all(Value::is_number) && !path.is_empty() =>
        {
            values.fill(Value::Null);
        }
        Value::Array(_) | Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
            if !path.is_empty() =>
        {
            *value = Value::Null;
        }
        Value::Array(_) | Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn normalize_nested_existing_entity_shapes(
    predecessor_root: &Value,
    value: &mut Value,
) -> Result<(), LineageBridgeError> {
    match value {
        Value::Object(object) => {
            for (field, value) in object {
                if let Some((_, reservation_kind)) = entity_kind_for_collection(Some(field)) {
                    let values = value
                        .as_array_mut()
                        .ok_or(LineageBridgeError::MissingOwner)?;
                    for entity in values {
                        let Some(id) = entity_id(entity) else {
                            continue;
                        };
                        if find_materialized_entity(
                            &mut Vec::new(),
                            predecessor_root,
                            reservation_kind,
                            id,
                        )
                        .is_some()
                        {
                            redact_direct_entity_leaf_values(&mut Vec::new(), entity);
                            normalize_nested_existing_entity_shapes(predecessor_root, entity)?;
                        }
                    }
                } else {
                    normalize_nested_existing_entity_shapes(predecessor_root, value)?;
                }
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                normalize_nested_existing_entity_shapes(predecessor_root, value)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
    }
}

fn validate_exact_step_manifest(
    step: &LineageStep,
    expected: &StepManifest,
) -> Result<(), LineageBridgeError> {
    if step.writable_leaves != expected.writable_leaves {
        return Err(LineageBridgeError::WritableManifestMismatch { step: step.id });
    }
    if step.action.inputs() != expected.inputs.as_slice()
        || step.outputs != expected.outputs
        || step.output_identities != expected.identities
        || step.reservations != expected.reservations
    {
        return Err(LineageBridgeError::ActionManifestMismatch { step: step.id });
    }
    Ok(())
}

/// Authenticates current, accepted, Undo, and Redo programs retained by one
/// complete session. History is traversed only on clones, so validation is
/// read-only and cannot rebase caller authority.
pub(super) fn validate_lineage_session_semantics(
    session: &LineageSession,
) -> Result<(), LineageBridgeError> {
    let mut identities = SessionIdentityLedger::default();
    validate_visible_session_documents(session, &mut identities)?;

    let mut undo = session.clone();
    for _ in 0..session.undo_len() {
        undo.undo()?
            .ok_or(LineageBridgeError::MissingHistory("Undo"))?;
        validate_visible_session_documents(&undo, &mut identities)?;
    }

    let mut redo = session.clone();
    for _ in 0..session.redo_len() {
        redo.redo()?
            .ok_or(LineageBridgeError::MissingHistory("Redo"))?;
        validate_visible_session_documents(&redo, &mut identities)?;
    }
    Ok(())
}

fn validate_visible_session_documents(
    session: &LineageSession,
    identities: &mut SessionIdentityLedger,
) -> Result<(), LineageBridgeError> {
    validate_lineage_document_semantics(session.document())?;
    identities.observe(session.document())?;
    if let Some(accepted) = session.last_accepted_document() {
        validate_lineage_document_semantics(accepted)?;
        identities.observe(accepted)?;
    }
    Ok(())
}

impl SessionIdentityLedger {
    fn observe(&mut self, document: &LineageDocument) -> Result<(), LineageBridgeError> {
        for step in document.steps() {
            insert_immutable_binding(
                &mut self.steps,
                (document.id(), step.id),
                ImmutableStepBinding {
                    key: step.key.clone(),
                    action: immutable_action_binding(&step.action)?,
                },
                "step identity",
            )?;
            for output in &step.outputs {
                let flow = step
                    .output_identity(output.id)
                    .ok_or(LineageBridgeError::InvalidMaterialization(
                        "lineage output has no identity flow",
                    ))?
                    .flow;
                insert_immutable_binding(
                    &mut self.outputs,
                    (document.id(), output.id),
                    ImmutableOutputBinding {
                        step: step.id,
                        key: output.key.clone(),
                        kind: output.kind,
                        reservation: output.reservation,
                        flow,
                    },
                    "output identity",
                )?;
            }
            for reservation in &step.reservations {
                insert_immutable_binding(
                    &mut self.reservations,
                    (document.id(), reservation.id),
                    ImmutableReservationBinding {
                        step: step.id,
                        key: reservation.key.clone(),
                        kind: reservation.kind,
                        persistent_id: reservation.persistent_id.clone(),
                    },
                    "reservation identity",
                )?;
                insert_immutable_binding(
                    &mut self.persistent_ids,
                    (
                        document.id(),
                        reservation.kind,
                        reservation.persistent_id.clone(),
                    ),
                    reservation.id,
                    "persistent identity reservation",
                )?;
            }
        }
        Ok(())
    }
}

fn immutable_action_binding(
    action: &LineageActionDefinition,
) -> Result<ImmutableActionBinding, LineageBridgeError> {
    let (schema, version) = match action {
        LineageActionDefinition::ImportedBaseline { baseline } => match &baseline.encoding {
            ImportedBaselineEncoding::Canonical { schema, version } => (schema.clone(), *version),
            ImportedBaselineEncoding::Opaque {
                media_type,
                version,
            } => (media_type.clone(), *version),
        },
        action => {
            let payload = ordinary_action_payload(action)?;
            (payload.schema.clone(), payload.version)
        }
    };
    Ok(ImmutableActionBinding {
        kind: action.kind(),
        schema,
        version,
    })
}

fn insert_immutable_binding<K: Ord, V: Eq>(
    bindings: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    identity: &'static str,
) -> Result<(), LineageBridgeError> {
    if let Some(retained) = bindings.get(&key) {
        if retained != &value {
            return Err(LineageBridgeError::HistoryIdentityRebinding(identity));
        }
    } else {
        bindings.insert(key, value);
    }
    Ok(())
}

fn validate_imported_baseline_manifest(
    document: &LineageDocument,
    step: &LineageStep,
    baseline: &ImportedBaselineAction,
) -> Result<(), LineageBridgeError> {
    match &baseline.encoding {
        ImportedBaselineEncoding::Opaque {
            media_type,
            version,
        } if media_type.as_str() == BASELINE_MEDIA_TYPE && *version == 1 => {}
        _ => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported imported-baseline bridge encoding",
            ));
        }
    }

    let imported: ImportedCoordinatorBaseline = serde_json::from_str(&baseline.payload)?;
    if imported.version != 1 {
        return Err(LineageBridgeError::InvalidMaterialization(
            "unsupported imported-baseline bridge version",
        ));
    }
    imported.host_inputs.decode()?;
    if let Some(inputs) = &imported.accepted_host_inputs {
        if imported.checkpoint.accepted_json.is_none() {
            return Err(LineageBridgeError::InvalidMaterialization(
                "imported accepted host inputs require an embedded accepted sketch",
            ));
        }
        inputs.decode()?;
    }
    let value = authored_structural_value(&imported.checkpoint)?;
    let entities = collect_materialized_entities(&value);
    let first_output = step.outputs.first().map(|output| output.id).ok_or(
        LineageBridgeError::InvalidMaterialization("imported baseline has no result output"),
    )?;
    let first_reservation = step
        .reservations
        .first()
        .map_or(LineageReservationId::from_raw(1), |reservation| {
            reservation.id
        });
    let expected = created_manifest(
        document.id(),
        step.id,
        first_output,
        first_reservation,
        &entities,
        &value,
    )?;

    if step.writable_leaves != expected.writable_leaves {
        return Err(LineageBridgeError::WritableManifestMismatch { step: step.id });
    }
    if step.outputs != expected.outputs
        || step.output_identities != expected.identities
        || step.reservations != expected.reservations
    {
        return Err(LineageBridgeError::InvalidMaterialization(
            "imported baseline identity manifest does not match its exact payload",
        ));
    }
    Ok(())
}

fn has_workbench_materialization_parameters(action: &LineageActionDefinition) -> bool {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => return false,
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    [
        AUTHORED_INTENT_PARAMETER,
        AUTHORED_OWNER_FIELDS_PARAMETER,
        HOST_INPUT_PROVENANCE_PARAMETER,
        MATERIALIZATION_PARAMETER,
    ]
    .into_iter()
    .any(|parameter| payload.parameters.contains_key(parameter))
}

fn is_native_line_fillet_materialization_step(step: &LineageStep) -> bool {
    matches!(
        &step.action,
        LineageActionDefinition::Operation { action }
            if action.schema.as_str()
                == "geosolve.document-edit.v1.create-prepared-native-line-fillet-geometry"
    )
}

fn push_distinct_evaluation_checkpoint(
    checkpoints: &mut Vec<LineageMaterializedPrefix>,
    checkpoint: LineageMaterializedPrefix,
) {
    if checkpoints
        .last()
        .is_none_or(|existing| existing.step != checkpoint.step)
    {
        checkpoints.push(checkpoint);
    }
}

fn apply_materialization_step(
    document: &LineageDocument,
    checkpoint: &mut Option<LineageCheckpoint>,
    step: &LineageStep,
) -> Result<DecodedLineageHostInputs, LineageBridgeError> {
    let application = (|| -> Result<DecodedLineageHostInputs, LineageBridgeError> {
        match &step.action {
            LineageActionDefinition::ImportedBaseline { baseline } => {
                let imported: ImportedCoordinatorBaseline =
                    serde_json::from_str(&baseline.payload)?;
                if imported.version != 1 {
                    return Err(LineageBridgeError::InvalidMaterialization(
                        "unsupported imported-baseline bridge version",
                    ));
                }
                let host_inputs = imported.host_inputs.decode()?;
                *checkpoint = Some(imported.checkpoint);
                Ok(host_inputs)
            }
            action => {
                let host_inputs = action_host_input_provenance(action)?.decode()?;
                let current = checkpoint
                    .as_mut()
                    .ok_or(LineageBridgeError::MissingBaseline)?;
                let mut value = current.structural_value()?;
                apply_authored_operations(&mut value, &action_delta_for_step(document, step)?)?;
                current.replace_structural_value(&value)?;
                Ok(host_inputs)
            }
        }
    })();
    application.map_err(|source| LineageBridgeError::StepMaterialization {
        step: step.id,
        source: Box::new(source),
    })
}

fn action_host_input_provenance(
    action: &LineageActionDefinition,
) -> Result<LineageHostInputProvenance, LineageBridgeError> {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "imported baseline host inputs belong in its bridge payload",
            ));
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    serde_json::from_value(
        payload
            .parameters
            .get(HOST_INPUT_PROVENANCE_PARAMETER)
            .cloned()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "action has no exact historical host-input provenance",
            ))?,
    )
    .map_err(LineageBridgeError::from)
}

fn declared_host_inputs(
    document: &LineageDocument,
) -> Result<Vec<DecodedLineageHostInputs>, LineageBridgeError> {
    let mut declared = Vec::new();
    for step in document.steps() {
        match &step.action {
            LineageActionDefinition::ImportedBaseline { baseline } => {
                let imported: ImportedCoordinatorBaseline =
                    serde_json::from_str(&baseline.payload)?;
                if imported.version != 1 {
                    return Err(LineageBridgeError::InvalidMaterialization(
                        "unsupported imported-baseline bridge version",
                    ));
                }
                declared.push(imported.host_inputs.decode()?);
                if let Some(inputs) = imported.accepted_host_inputs {
                    declared.push(inputs.decode()?);
                }
            }
            action => declared.push(action_host_input_provenance(action)?.decode()?),
        }
    }
    Ok(declared)
}

fn retain_host_input_candidate(
    candidates: &mut BTreeMap<LineageOpaqueId, DecodedLineageHostInputs>,
    inputs: DecodedLineageHostInputs,
) -> Result<(), LineageBridgeError> {
    let stamp = external_input_stamp(inputs.parameters(), inputs.snapshots())?;
    if let Some(existing) = candidates.get(&stamp)
        && existing != &inputs
    {
        return Err(LineageBridgeError::AcceptedExternalInputMismatch);
    }
    candidates.insert(stamp, inputs);
    Ok(())
}

fn common_materialization_prefix_len(
    accepted: &LineageDocument,
    current: &LineageDocument,
) -> Result<usize, LineageBridgeError> {
    let mut common = 0_usize;
    for (accepted_step, current_step) in accepted.steps().iter().zip(current.steps()) {
        if !steps_materialization_equivalent(accepted_step, current_step)? {
            break;
        }
        common = common.saturating_add(1);
    }
    Ok(common)
}

fn changed_current_steps(
    accepted: &LineageDocument,
    current: &LineageDocument,
) -> Result<Vec<LineageStepId>, LineageBridgeError> {
    let accepted_positions = accepted
        .steps()
        .iter()
        .enumerate()
        .map(|(index, step)| (step.id, (index, step)))
        .collect::<BTreeMap<_, _>>();
    if accepted
        .steps()
        .iter()
        .any(|step| current.step(step.id).is_none())
    {
        // Reconciliation is allowed to retain an allocator reservation while
        // omitting an old tombstone. With no current owner node from which to
        // compute a forward closure, conservatively dirty the complete program.
        return Ok(current.steps().iter().map(|step| step.id).collect());
    }
    let mut changed = Vec::new();
    for (index, step) in current.steps().iter().enumerate() {
        let differs = match accepted_positions.get(&step.id) {
            Some((accepted_index, accepted_step)) => {
                *accepted_index != index || !steps_materialization_equivalent(accepted_step, step)?
            }
            None => true,
        };
        if differs {
            changed.push(step.id);
        }
    }
    Ok(changed)
}

fn steps_materialization_equivalent(
    accepted: &LineageStep,
    current: &LineageStep,
) -> Result<bool, LineageBridgeError> {
    if accepted.id != current.id
        || accepted.key != current.key
        || accepted.label != current.label
        || accepted.state != current.state
        || accepted.outputs != current.outputs
        || accepted.output_identities != current.output_identities
        || accepted.writable_leaves != current.writable_leaves
        || accepted.reservations != current.reservations
    {
        return Ok(false);
    }
    match (&accepted.action, &current.action) {
        (
            LineageActionDefinition::ImportedBaseline {
                baseline: accepted_baseline,
            },
            LineageActionDefinition::ImportedBaseline {
                baseline: current_baseline,
            },
        ) => {
            if accepted_baseline.encoding != current_baseline.encoding {
                return Ok(false);
            }
            let accepted: ImportedCoordinatorBaseline =
                serde_json::from_str(&accepted_baseline.payload)?;
            let current: ImportedCoordinatorBaseline =
                serde_json::from_str(&current_baseline.payload)?;
            Ok(accepted.version == current.version
                && accepted.host_inputs == current.host_inputs
                && accepted.accepted_host_inputs == current.accepted_host_inputs
                && semantic_structural_value(&accepted.checkpoint)?
                    == semantic_structural_value(&current.checkpoint)?)
        }
        _ => Ok(accepted.action == current.action),
    }
}

impl RetainedEditorCoordinator {
    /// Publishes one exact prepared equation-free sketch operation through the
    /// authoritative lineage boundary.
    ///
    /// The operation proposal still owns exact-input validation and ordinary
    /// sketch-domain application. This wrapper stages its complete accepted
    /// materialization, records one semantic operation step with typed identity
    /// flow, and only then swaps the live coordinator state.
    ///
    /// # Errors
    ///
    /// Returns a stale-input, operation-application, materialization, computed-
    /// feature, or lineage validation error without partially publishing state.
    pub fn apply_sketch_operation(
        &mut self,
        proposal: &SketchOperationProposal,
    ) -> Result<MutationOutcome<SketchOperationApplication>, CoordinatorError> {
        if proposal.input() != self.session.prepared_input() {
            return Err(CoordinatorError::Lineage(
                "stale sketch-operation proposal input".into(),
            ));
        }
        let mut trial = self.session.clone();
        let outcome = proposal
            .apply(&mut trial)
            .map_err(|error| CoordinatorError::Lineage(error.to_string()))?;
        let mut result = super::mutation_from(&outcome);
        let mut staged = self.stage_construction_publication(trial)?;
        let previous = self.persistence_checkpoint()?;
        let mut lineage = self.lineage.clone();
        let evaluation = lineage.record_operation(
            proposal,
            &previous,
            &staged.checkpoint,
            staged.session.parameter_batch(),
            staged.session.external_snapshot_set(),
        )?;
        if evaluation.is_some() {
            super::replace_session_accepted_from_lineage_evidence(
                &mut staged.session,
                evaluation.as_ref(),
            )?;
            let mut allocator = self.computed_evaluation_allocator.clone();
            let (computed_input, computed_snapshot, computed_evaluation_problem) =
                match evaluate_computed_features(
                    &staged.session,
                    &self.features,
                    &mut allocator,
                    bounded_geometry_control(),
                ) {
                    Ok(OperationOutcome::Completed { value, .. }) => {
                        (Some(value.input()), Some(value), None)
                    }
                    Ok(stopped) => (
                        None,
                        None,
                        Some(format!(
                            "computed-feature evaluation stopped: {:?}",
                            stopped.report().stopping_reason
                        )),
                    ),
                    Err(error) => (None, None, Some(error.to_string())),
                };
            staged.computed_evaluation_allocator = allocator;
            staged.computed_input = computed_input;
            staged.computed_snapshot = computed_snapshot;
            staged.computed_evaluation_problem = computed_evaluation_problem;
            staged.checkpoint = checkpoint(
                &staged.session,
                &self.features,
                &staged.computed_evaluation_allocator,
            )?;
            lineage.retain_computed_evaluation_high_water(
                staged.checkpoint.computed_evaluation_high_water(),
            )?;
        }

        self.lineage = lineage;
        self.session = staged.session;
        self.computed_evaluation_allocator = staged.computed_evaluation_allocator;
        self.computed_input = staged.computed_input;
        self.computed_snapshot = staged.computed_snapshot;
        self.computed_evaluation_problem = staged.computed_evaluation_problem;
        self.history.truncate(self.history_cursor + 1);
        self.history.push(staged.checkpoint);
        self.history_cursor += 1;
        self.editor.invalidate_for_retained_state_change(false);
        self.clear_transient();
        self.reconcile_selection();
        result.published_accepted = self.session.last_attempt().accepted_state_identity();
        Ok(result)
    }
}

fn logical_output_field_for_change(
    owner: &LineageStep,
    after: &Value,
    change: &StructuralDeltaOperation,
) -> Result<Option<AuthoredOutputFieldValue>, LineageBridgeError> {
    let StructuralDeltaOperation::SetField { path, .. } = change else {
        return Ok(None);
    };
    if path.last().is_none_or(|field| field != "host_activation") {
        return Ok(None);
    }
    let output = owner
        .outputs
        .iter()
        .find(|output| output.kind == LineageOutputKind::Activation)
        .ok_or(LineageBridgeError::MissingOwner)?;
    let field = LineageSemanticKey::new("configuration")?;
    if !owner
        .writable_leaves
        .iter()
        .any(|writable| writable.output == output.id && writable.key == field)
    {
        return Err(LineageBridgeError::MissingOwner);
    }
    Ok(Some(AuthoredOutputFieldValue {
        output: output.id,
        kind: output.kind,
        field,
        value: value_at_path(after, path)
            .cloned()
            .ok_or(LineageBridgeError::MissingOwner)?,
    }))
}

fn retain_baseline_materialization_evidence(
    action: &mut LineageActionDefinition,
    after: &LineageCheckpoint,
) -> Result<bool, LineageBridgeError> {
    let LineageActionDefinition::ImportedBaseline { baseline } = action else {
        return Err(LineageBridgeError::InvalidMaterialization(
            "baseline evidence can only be retained by an imported baseline",
        ));
    };
    let previous = baseline.clone();
    let mut imported: ImportedCoordinatorBaseline = serde_json::from_str(&baseline.payload)?;
    imported.checkpoint.retain_lifecycle_high_water(after)?;
    imported.checkpoint.replace_materialization_evidence(after);
    baseline.payload = serde_json::to_string(&imported)?;
    Ok(*baseline != previous)
}

pub(super) fn external_input_stamp(
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<LineageOpaqueId, LineageBridgeError> {
    let value = serde_json::json!({
        "version": 1,
        "parameter_batch": serde_json::from_str::<Value>(&parameters.to_canonical_json()?)?,
        "external_snapshot_set": serde_json::from_str::<Value>(&snapshots.to_canonical_json().map_err(|error| {
            LineageBridgeError::InvalidExternalSnapshot(error.to_string())
        })?)?,
    });
    let digest = lineage_content_digest(&serde_json::to_vec(&value)?);
    Ok(LineageOpaqueId::new(format!("external-inputs:{digest}"))?)
}

fn created_manifest(
    document: LineageDocumentId,
    step: LineageStepId,
    next_output: LineageOutputId,
    next_reservation: LineageReservationId,
    entities: &[MaterializedEntity],
    materialized: &Value,
) -> Result<StepManifest, LineageBridgeError> {
    let writable = writable_leaf_catalog(materialized)?;
    build_manifest(
        document,
        step,
        next_output,
        next_reservation,
        Vec::new(),
        entities.to_vec(),
        Vec::new(),
        Vec::new(),
        &writable,
        &BTreeMap::new(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the manifest builder keeps created, continued, retired, aliased, and exact writable-leaf identity flow in one closed audit pass"
)]
fn manifest_for_delta(
    document: &LineageDocument,
    step: LineageStepId,
    next_output: LineageOutputId,
    next_reservation: LineageReservationId,
    before: &Value,
    after: &Value,
    delta: &[StructuralDeltaOperation],
) -> Result<StepManifest, LineageBridgeError> {
    let index = materialized_output_index(document);
    manifest_for_delta_with_index(
        document.id(),
        &index,
        step,
        next_output,
        next_reservation,
        before,
        after,
        delta,
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the manifest compiler receives an authenticated predecessor index beside the exact structural delta"
)]
fn manifest_for_delta_with_index(
    document: LineageDocumentId,
    index: &MaterializedOutputIndex,
    step: LineageStepId,
    next_output: LineageOutputId,
    next_reservation: LineageReservationId,
    before: &Value,
    after: &Value,
    delta: &[StructuralDeltaOperation],
) -> Result<StepManifest, LineageBridgeError> {
    let writable = writable_leaf_catalog(after)?;
    let mut created = collect_created_entities(before, delta);
    created.retain(|entity| {
        !index
            .by_kind_and_raw
            .contains_key(&(entity.output_kind, entity.persistent_id.clone()))
    });
    let created_keys = created
        .iter()
        .map(|entity| (entity.output_kind, entity.persistent_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut referenced = BTreeSet::<LineageOutputRef>::new();
    let mut continued = BTreeSet::<LineageOutputRef>::new();
    let mut continued_writable = BTreeMap::<LineageOutputRef, BTreeSet<LineageSemanticKey>>::new();
    let mut retired = BTreeSet::<LineageOutputRef>::new();

    for operation in delta {
        let mut strings = Vec::new();
        collect_operation_strings(operation, &mut strings);
        for value in strings {
            if let Some(values) = index.by_raw.get(value) {
                referenced.extend(values.iter().copied());
            }
        }
        let transfers_writable_leaves = match operation {
            StructuralDeltaOperation::UpsertEntity { path, id, .. } => {
                entity_exists(before, path, id)?
            }
            StructuralDeltaOperation::RemoveEntity { path, .. } => {
                path.last().is_some_and(|field| {
                    matches!(field.as_str(), "geometry_roles" | "user_inactive_elements")
                })
            }
            StructuralDeltaOperation::SetField { .. }
            | StructuralDeltaOperation::RemoveField { .. } => false,
        };
        if transfers_writable_leaves {
            for leaf in direct_materialized_leaves_for_change(before, after, operation)? {
                let source = index
                    .by_materialized
                    .get(&leaf.materialized)
                    .copied()
                    .ok_or(LineageBridgeError::MissingOwner)?;
                continued.insert(source);
                continued_writable
                    .entry(source)
                    .or_default()
                    .insert(leaf.key);
            }
        }
        match operation {
            StructuralDeltaOperation::RemoveEntity { path, id } => {
                if let Some((output_kind, _)) =
                    entity_kind_for_collection(path.last().map(String::as_str))
                    && let Some(source) = index.by_kind_and_raw.get(&(output_kind, id.clone()))
                {
                    retired.insert(*source);
                }
            }
            StructuralDeltaOperation::SetField { path, value }
                if entity_kind_for_collection(path.last().map(String::as_str)).is_some() =>
            {
                let after_entities = collect_materialized_entities_at(path, value);
                for entity in collect_materialized_entities_at(
                    path,
                    value_at_path(before, path).unwrap_or(&Value::Null),
                ) {
                    if !after_entities.iter().any(|after| {
                        after.output_kind == entity.output_kind
                            && after.persistent_id == entity.persistent_id
                    }) && let Some(source) = index
                        .by_kind_and_raw
                        .get(&(entity.output_kind, entity.persistent_id.clone()))
                    {
                        retired.insert(*source);
                    }
                }
            }
            StructuralDeltaOperation::UpsertEntity {
                path, id, value, ..
            } => {
                let after_entities = collect_materialized_entities_at(path, value)
                    .into_iter()
                    .map(|entity| (entity.output_kind, entity.persistent_id))
                    .collect::<BTreeSet<_>>();
                if let Some(previous) = entity_value_at_path(before, path, id) {
                    for entity in collect_materialized_entities_at(path, previous) {
                        if !after_entities
                            .contains(&(entity.output_kind, entity.persistent_id.clone()))
                            && let Some(source) = index
                                .by_kind_and_raw
                                .get(&(entity.output_kind, entity.persistent_id))
                        {
                            retired.insert(*source);
                        }
                    }
                }
            }
            StructuralDeltaOperation::SetField { .. }
            | StructuralDeltaOperation::RemoveField { .. } => {}
        }
    }

    // Removing one entity can also remove a side-table property owned by the
    // same identity. Retirement wins over that property-level continuation:
    // one action must never consume the same identity generation twice.
    continued.retain(|source| !retired.contains(source));
    continued_writable.retain(|source, _| !retired.contains(source));
    referenced.retain(|source| {
        index
            .raw_by_ref
            .get(source)
            .is_none_or(|raw| !created_keys.contains(&(source.kind, raw.clone())))
    });
    let inputs = referenced
        .iter()
        .enumerate()
        .map(|(ordinal, source)| {
            Ok(LineageInputBinding {
                key: LineageSemanticKey::new(format!("operand-{ordinal:04}"))?,
                kind: source.kind,
                source: *source,
            })
        })
        .collect::<Result<Vec<_>, LineageBridgeError>>()?;
    build_manifest(
        document,
        step,
        next_output,
        next_reservation,
        inputs,
        created,
        continued.into_iter().collect(),
        retired.into_iter().collect(),
        &writable,
        &continued_writable,
    )
}

fn enrich_replay_manifest(
    document: &LineageDocument,
    replay: &ReplayAction,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    let ReplayAction::CreateComputedFillet { corners, .. } = replay else {
        return Ok(());
    };
    let index = materialized_output_index(document);
    for (corner_ordinal, corner) in corners.iter().enumerate() {
        append_span_input(
            &index,
            manifest,
            format!("corner-{corner_ordinal:04}-first"),
            corner.first.source.span,
        )?;
        append_span_input(
            &index,
            manifest,
            format!("corner-{corner_ordinal:04}-second"),
            corner.second.source.span,
        )?;
    }
    append_owned_logical_output(manifest, "radius", LineageOutputKind::Parameter)?;
    Ok(())
}

/// Restores the schema-specific portion of a generated manifest from durable
/// semantic intent. Generic structural inputs/identity flow are compiled by
/// `manifest_for_delta_with_index`; only roles that are intentionally richer
/// than the flat delta live here.
fn enrich_persisted_action_manifest(
    index: &MaterializedOutputIndex,
    action: &LineageActionDefinition,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    let payload = ordinary_action_payload(action)?;
    let schema = payload.schema.as_str();
    if schema == "geosolve.feature.v1.fillet-set" {
        return enrich_persisted_computed_fillet_manifest(index, action, payload, manifest);
    }

    let Some(operation_kind) = schema.strip_prefix("geosolve.operation.v1.") else {
        return Ok(());
    };
    if !matches!(
        operation_kind,
        "split"
            | "break"
            | "trim"
            | "extend"
            | "mirror"
            | "chamfer"
            | "associative-fillet"
            | "rectangle"
            | "regular-polygon"
            | "slot"
            | "linear-pattern"
            | "profile-offset"
    ) {
        // Other Operation-category replay schemas (Delete, suppression,
        // reattempt, and so on) have no enrichment beyond their delta.
        return Ok(());
    }
    enrich_persisted_sketch_operation_manifest(index, action, payload, operation_kind, manifest)
}

fn enrich_persisted_computed_fillet_manifest(
    index: &MaterializedOutputIndex,
    action: &LineageActionDefinition,
    payload: &VersionedActionPayload,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    if !matches!(action, LineageActionDefinition::ComputedFeature { .. }) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "computed Fillet manifest has the wrong action category",
        ));
    }
    let intent = persisted_intent_object(payload)?;
    validate_persisted_intent_keys(intent, &["version", "body", AUTHORED_OUTPUT_FIELD_MANIFEST])?;
    let body = persisted_object_field(intent, "body")?;
    validate_exact_object_keys(body, &["label", "radius", "corners"])?;
    let corners =
        persisted_field::<Vec<geosolve_sketch_features::NewComputedFilletCorner>>(body, "corners")?;
    for (corner_ordinal, corner) in corners.iter().enumerate() {
        append_span_input(
            index,
            manifest,
            format!("corner-{corner_ordinal:04}-first"),
            corner.first.source.span,
        )?;
        append_span_input(
            index,
            manifest,
            format!("corner-{corner_ordinal:04}-second"),
            corner.second.source.span,
        )?;
    }
    append_owned_logical_output(manifest, "radius", LineageOutputKind::Parameter)
}

fn enrich_persisted_sketch_operation_manifest(
    index: &MaterializedOutputIndex,
    action: &LineageActionDefinition,
    payload: &VersionedActionPayload,
    operation_kind: &str,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    if !matches!(action, LineageActionDefinition::Operation { .. }) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "sketch-operation manifest has the wrong action category",
        ));
    }
    let intent = persisted_intent_object(payload)?;
    validate_persisted_intent_keys(
        intent,
        &[
            "version",
            "kind",
            "source_free_geometry_role",
            "body",
            AUTHORED_OUTPUT_FIELD_MANIFEST,
        ],
    )?;
    if intent.get("kind").and_then(Value::as_str) != Some(operation_kind) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "sketch-operation schema and intent kind differ",
        ));
    }
    let body = persisted_object_field(intent, "body")?;
    enrich_persisted_sketch_operation_body(index, operation_kind, body, manifest)?;
    append_owned_logical_output(manifest, "operation", LineageOutputKind::Operation)
}

fn enrich_persisted_sketch_operation_body(
    index: &MaterializedOutputIndex,
    operation_kind: &str,
    body: &Map<String, Value>,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    match operation_kind {
        "split" | "trim" => {
            validate_exact_object_keys(body, &["support", "parameter", "retained"])?;
            let support = persisted_field::<geosolve_sketch::CurveSpan>(body, "support")?;
            append_span_input(index, manifest, "support", support)?;
            ensure_continued_input(manifest, span_output_ref(index, support)?)?;
        }
        "break" => {
            validate_exact_object_keys(body, &["support", "start", "end", "retained"])?;
            let support = persisted_field::<geosolve_sketch::CurveSpan>(body, "support")?;
            append_span_input(index, manifest, "support", support)?;
            ensure_continued_input(manifest, span_output_ref(index, support)?)?;
        }
        "extend" => {
            validate_exact_object_keys(body, &["line", "endpoint", "target"])?;
            append_span_input(index, manifest, "line", persisted_field(body, "line")?)?;
            append_span_input(index, manifest, "target", persisted_field(body, "target")?)?;
        }
        "mirror" => {
            validate_exact_object_keys(body, &["label", "source", "axis"])?;
            append_curve_input(index, manifest, "source", persisted_field(body, "source")?)?;
            append_span_input(index, manifest, "axis", persisted_field(body, "axis")?)?;
        }
        "chamfer" => {
            validate_exact_object_keys(
                body,
                &[
                    "label",
                    "first",
                    "second",
                    "first_distance",
                    "second_distance",
                ],
            )?;
            let first = persisted_field(body, "first")?;
            let second = persisted_field(body, "second")?;
            append_span_input(index, manifest, "first", first)?;
            append_span_input(index, manifest, "second", second)?;
            ensure_continued_input(manifest, span_output_ref(index, first)?)?;
            ensure_continued_input(manifest, span_output_ref(index, second)?)?;
        }
        "associative-fillet" => {
            validate_exact_object_keys(body, &["label", "request"])?;
            let request = persisted_object_field(body, "request")?;
            let first = persisted_object_field(request, "first")?;
            let second = persisted_object_field(request, "second")?;
            let first = persisted_field(first, "curve")?;
            let second = persisted_field(second, "curve")?;
            append_span_input(index, manifest, "first", first)?;
            append_span_input(index, manifest, "second", second)?;
            append_owned_logical_output(manifest, "radius", LineageOutputKind::Parameter)?;
        }
        "rectangle" => {
            validate_exact_object_keys(body, &["label", "origin", "width", "height"])?;
        }
        "regular-polygon" => {
            validate_exact_object_keys(body, &["label", "center", "radius", "sides", "rotation"])?;
        }
        "slot" => {
            validate_exact_object_keys(
                body,
                &["label", "first_center", "second_center", "radius"],
            )?;
        }
        "linear-pattern" => {
            validate_exact_object_keys(body, &["label", "sources", "instances", "step"])?;
            for (ordinal, source) in
                persisted_field::<Vec<geosolve_sketch::CurveId>>(body, "sources")?
                    .into_iter()
                    .enumerate()
            {
                append_curve_input(index, manifest, format!("source-{ordinal:04}"), source)?;
            }
        }
        "profile-offset" => {
            validate_exact_object_keys(body, &["label", "distance", "operand"])?;
            enrich_persisted_profile_offset_operand(
                index,
                manifest,
                persisted_object_field(body, "operand")?,
            )?;
            append_owned_logical_output(manifest, "distance", LineageOutputKind::Parameter)?;
        }
        _ => unreachable!("closed operation schema match above"),
    }
    Ok(())
}

fn ordinary_action_payload(
    action: &LineageActionDefinition,
) -> Result<&VersionedActionPayload, LineageBridgeError> {
    Ok(match action {
        LineageActionDefinition::ImportedBaseline { .. } => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "imported baseline has no ordinary action payload",
            ));
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    })
}

fn persisted_intent_object(
    payload: &VersionedActionPayload,
) -> Result<&Map<String, Value>, LineageBridgeError> {
    if payload.version != 1 {
        return Err(LineageBridgeError::InvalidMaterialization(
            "unsupported editor-generated action version",
        ));
    }
    payload
        .parameters
        .get(AUTHORED_INTENT_PARAMETER)
        .and_then(Value::as_object)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent is not an object",
        ))
}

fn validate_persisted_intent_keys(
    object: &Map<String, Value>,
    required: &[&str],
) -> Result<(), LineageBridgeError> {
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "unsupported authored semantic intent version",
        ));
    }
    let mut allowed = required.iter().copied().collect::<BTreeSet<_>>();
    allowed.insert("owned_output_fields");
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if !required.iter().all(|key| actual.contains(key)) || !actual.is_subset(&allowed) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent has an invalid schema-specific shape",
        ));
    }
    Ok(())
}

fn validate_exact_object_keys(
    object: &Map<String, Value>,
    expected: &[&str],
) -> Result<(), LineageBridgeError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent body has an invalid schema-specific shape",
        ));
    }
    Ok(())
}

fn persisted_object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, LineageBridgeError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent object field is missing or invalid",
        ))
}

fn persisted_field<T: serde::de::DeserializeOwned>(
    object: &Map<String, Value>,
    field: &str,
) -> Result<T, LineageBridgeError> {
    Ok(serde_json::from_value(object.get(field).cloned().ok_or(
        LineageBridgeError::InvalidMaterialization("authored semantic intent field is missing"),
    )?)?)
}

fn enrich_persisted_profile_offset_operand(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    operand: &Map<String, Value>,
) -> Result<(), LineageBridgeError> {
    match operand.get("kind").and_then(Value::as_str) {
        Some("face") => {
            validate_exact_object_keys(operand, &["kind", "direction", "outer", "holes"])?;
            append_persisted_offset_spans(index, manifest, "outer", operand, "outer")?;
            let holes = operand.get("holes").and_then(Value::as_array).ok_or(
                LineageBridgeError::InvalidMaterialization("profile-offset holes are invalid"),
            )?;
            for (hole_ordinal, hole) in holes.iter().enumerate() {
                let spans = hole
                    .as_array()
                    .ok_or(LineageBridgeError::InvalidMaterialization(
                        "profile-offset hole is invalid",
                    ))?;
                append_persisted_offset_span_values(
                    index,
                    manifest,
                    &format!("hole-{hole_ordinal:04}"),
                    spans,
                )?;
            }
            append_owned_logical_output(manifest, "profile", LineageOutputKind::Profile)?;
        }
        Some("open_chain") => {
            validate_exact_object_keys(operand, &["kind", "side", "spans"])?;
            append_persisted_offset_spans(index, manifest, "chain", operand, "spans")?;
            append_owned_logical_output(manifest, "chain", LineageOutputKind::Chain)?;
        }
        _ => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "profile-offset operand kind is invalid",
            ));
        }
    }
    Ok(())
}

fn append_persisted_offset_spans(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    role: &str,
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), LineageBridgeError> {
    let spans = object.get(field).and_then(Value::as_array).ok_or(
        LineageBridgeError::InvalidMaterialization("profile-offset span list is invalid"),
    )?;
    append_persisted_offset_span_values(index, manifest, role, spans)
}

fn append_persisted_offset_span_values(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    role: &str,
    spans: &[Value],
) -> Result<(), LineageBridgeError> {
    for (ordinal, span) in spans.iter().enumerate() {
        let span = span
            .as_object()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "profile-offset directed span is invalid",
            ))?;
        validate_exact_object_keys(span, &["span", "traversal"])?;
        append_span_input(
            index,
            manifest,
            format!("{role}-{ordinal:04}"),
            persisted_field(span, "span")?,
        )?;
    }
    Ok(())
}

fn enrich_operation_manifest(
    document: &LineageDocument,
    proposal: &SketchOperationProposal,
    manifest: &mut StepManifest,
) -> Result<(), LineageBridgeError> {
    let index = materialized_output_index(document);
    match proposal.request() {
        SketchOperationRequest::Split { support, .. }
        | SketchOperationRequest::Break { support, .. }
        | SketchOperationRequest::Trim { support, .. } => {
            append_span_input(&index, manifest, "support", *support)?;
            ensure_continued_input(manifest, span_output_ref(&index, *support)?)?;
        }
        SketchOperationRequest::ExtendLineToLine { line, target, .. } => {
            append_span_input(&index, manifest, "line", *line)?;
            append_span_input(&index, manifest, "target", *target)?;
        }
        SketchOperationRequest::Mirror { source, axis, .. } => {
            append_curve_input(&index, manifest, "source", *source)?;
            append_span_input(&index, manifest, "axis", *axis)?;
        }
        SketchOperationRequest::Chamfer { first, second, .. } => {
            append_span_input(&index, manifest, "first", *first)?;
            append_span_input(&index, manifest, "second", *second)?;
        }
        SketchOperationRequest::AssociativeFillet { request, .. } => {
            append_span_input(&index, manifest, "first", request.first.curve)?;
            append_span_input(&index, manifest, "second", request.second.curve)?;
            append_owned_logical_output(manifest, "radius", LineageOutputKind::Parameter)?;
        }
        SketchOperationRequest::Rectangle { .. }
        | SketchOperationRequest::RegularPolygon { .. }
        | SketchOperationRequest::Slot { .. } => {}
        SketchOperationRequest::LinearPattern { sources, .. } => {
            for (ordinal, source) in sources.iter().copied().enumerate() {
                append_curve_input(&index, manifest, format!("source-{ordinal:04}"), source)?;
            }
        }
        SketchOperationRequest::ProfileOffset { operand, .. } => {
            match operand {
                SketchProfileOffsetOperand::Face { key, .. } => {
                    append_offset_face_inputs(&index, manifest, key)?;
                    append_owned_logical_output(manifest, "profile", LineageOutputKind::Profile)?;
                }
                SketchProfileOffsetOperand::OpenChain { spans, .. } => {
                    for (ordinal, span) in spans.iter().enumerate() {
                        append_span_input(
                            &index,
                            manifest,
                            format!("chain-{ordinal:04}"),
                            span.span,
                        )?;
                    }
                    append_owned_logical_output(manifest, "chain", LineageOutputKind::Chain)?;
                }
            }
            append_owned_logical_output(manifest, "distance", LineageOutputKind::Parameter)?;
        }
        _ => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported sketch operation request",
            ));
        }
    }
    for change in &proposal.expected_application().identity_changes {
        match change {
            geosolve_sketch_ops::SketchOperationIdentityChange::Replaced(element) => {
                ensure_continued_input(manifest, element_output_ref(&index, *element)?)?;
            }
            geosolve_sketch_ops::SketchOperationIdentityChange::Split { source, .. } => {
                ensure_continued_input(manifest, curve_output_ref(&index, *source)?)?;
            }
            geosolve_sketch_ops::SketchOperationIdentityChange::Retained(_)
            | geosolve_sketch_ops::SketchOperationIdentityChange::Proposed(_) => {}
            _ => {
                return Err(LineageBridgeError::InvalidMaterialization(
                    "unsupported sketch operation identity change",
                ));
            }
        }
    }
    append_owned_logical_output(manifest, "operation", LineageOutputKind::Operation)?;
    Ok(())
}

fn append_offset_face_inputs(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    face: &OffsetFaceKey,
) -> Result<(), LineageBridgeError> {
    for (ordinal, span) in face.outer.spans.iter().enumerate() {
        append_span_input(index, manifest, format!("outer-{ordinal:04}"), span.span)?;
    }
    for (hole_ordinal, hole) in face.holes.iter().enumerate() {
        for (span_ordinal, span) in hole.spans.iter().enumerate() {
            append_span_input(
                index,
                manifest,
                format!("hole-{hole_ordinal:04}-{span_ordinal:04}"),
                span.span,
            )?;
        }
    }
    Ok(())
}

fn append_curve_input(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    key: impl Into<String>,
    curve: geosolve_sketch::CurveId,
) -> Result<(), LineageBridgeError> {
    append_exact_input(manifest, key, curve_output_ref(index, curve)?)
}

fn curve_output_ref(
    index: &MaterializedOutputIndex,
    curve: geosolve_sketch::CurveId,
) -> Result<LineageOutputRef, LineageBridgeError> {
    index
        .by_kind_and_raw
        .get(&(LineageOutputKind::Curve, curve.to_string()))
        .copied()
        .ok_or(LineageBridgeError::MissingOwner)
}

fn element_output_ref(
    index: &MaterializedOutputIndex,
    element: geosolve_sketch::DocumentElementId,
) -> Result<LineageOutputRef, LineageBridgeError> {
    let (kind, raw) = match element {
        geosolve_sketch::DocumentElementId::Point(id) => (LineageOutputKind::Point, id.to_string()),
        geosolve_sketch::DocumentElementId::Scalar(id) => {
            (LineageOutputKind::Scalar, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Curve(id) => (LineageOutputKind::Curve, id.to_string()),
        geosolve_sketch::DocumentElementId::Contact(id) => {
            (LineageOutputKind::Contact, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Constraint(id) => {
            (LineageOutputKind::Constraint, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Dimension(id) => {
            (LineageOutputKind::Dimension, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Parameter(id) => {
            (LineageOutputKind::Parameter, id.to_string())
        }
        geosolve_sketch::DocumentElementId::ExternalBinding(id) => {
            (LineageOutputKind::ExternalBinding, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Source(id) => {
            (LineageOutputKind::Source, id.to_string())
        }
        geosolve_sketch::DocumentElementId::Document(_) => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "document identity cannot be continued by a sketch operation",
            ));
        }
        _ => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported sketch operation element identity",
            ));
        }
    };
    index
        .by_kind_and_raw
        .get(&(kind, raw))
        .copied()
        .ok_or(LineageBridgeError::MissingOwner)
}

fn append_span_input(
    index: &MaterializedOutputIndex,
    manifest: &mut StepManifest,
    key: impl Into<String>,
    span: geosolve_sketch::CurveSpan,
) -> Result<(), LineageBridgeError> {
    append_exact_input(manifest, key, span_output_ref(index, span)?)
}

fn span_output_ref(
    index: &MaterializedOutputIndex,
    span: geosolve_sketch::CurveSpan,
) -> Result<LineageOutputRef, LineageBridgeError> {
    index
        .by_kind_and_raw
        .get(&(
            LineageOutputKind::CurveSpan,
            format!("{}/{}", span.curve, span.segment),
        ))
        .or_else(|| {
            index
                .by_kind_and_raw
                .get(&(LineageOutputKind::Curve, span.curve.to_string()))
        })
        .copied()
        .ok_or(LineageBridgeError::MissingOwner)
}

fn append_exact_input(
    manifest: &mut StepManifest,
    key: impl Into<String>,
    source: LineageOutputRef,
) -> Result<(), LineageBridgeError> {
    let key = LineageSemanticKey::new(key.into())?;
    if manifest.inputs.iter().any(|input| input.key == key) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "operation input role is duplicated",
        ));
    }
    manifest.inputs.push(LineageInputBinding {
        key,
        kind: source.kind,
        source,
    });
    Ok(())
}

fn ensure_continued_input(
    manifest: &mut StepManifest,
    source: LineageOutputRef,
) -> Result<(), LineageBridgeError> {
    if manifest.identities.iter().any(|identity| {
        matches!(
            identity.flow,
            LineageOutputIdentityFlow::Continued {
                source: candidate
            } if candidate == source
        )
    }) {
        return Ok(());
    }
    let output = next_manifest_output_id(manifest)?;
    manifest.outputs.push(LineageOutput {
        id: output,
        key: LineageSemanticKey::new(format!(
            "continued-{}-{:016x}",
            output_kind_key(source.kind),
            output.raw()
        ))?,
        kind: source.kind,
        reservation: None,
    });
    manifest.identities.push(LineageOutputIdentity {
        output,
        flow: LineageOutputIdentityFlow::Continued { source },
    });
    Ok(())
}

fn append_owned_logical_output(
    manifest: &mut StepManifest,
    key: &str,
    kind: LineageOutputKind,
) -> Result<(), LineageBridgeError> {
    if manifest
        .outputs
        .iter()
        .any(|output| output.kind == kind && output.key.as_str() == key)
    {
        return Ok(());
    }
    let output = next_manifest_output_id(manifest)?;
    manifest.outputs.push(LineageOutput {
        id: output,
        key: LineageSemanticKey::new(key)?,
        kind,
        reservation: None,
    });
    manifest.identities.push(LineageOutputIdentity {
        output,
        flow: LineageOutputIdentityFlow::OwnedLogical,
    });
    Ok(())
}

fn next_manifest_output_id(manifest: &StepManifest) -> Result<LineageOutputId, LineageBridgeError> {
    let next = manifest
        .outputs
        .iter()
        .map(|output| output.id.raw())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(geosolve_sketch_lineage::LineageDocumentError::IdExhausted)?;
    Ok(LineageOutputId::from_raw(next))
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one manifest allocator receives the exact typed identity partitions"
)]
fn build_manifest(
    document: LineageDocumentId,
    step: LineageStepId,
    next_output: LineageOutputId,
    next_reservation: LineageReservationId,
    inputs: Vec<LineageInputBinding>,
    mut created: Vec<MaterializedEntity>,
    mut continued: Vec<LineageOutputRef>,
    mut retired: Vec<LineageOutputRef>,
    writable: &WritableLeafCatalog,
    continued_writable: &BTreeMap<LineageOutputRef, BTreeSet<LineageSemanticKey>>,
) -> Result<StepManifest, LineageBridgeError> {
    created.sort_by(|left, right| {
        (left.output_kind, &left.persistent_id).cmp(&(right.output_kind, &right.persistent_id))
    });
    created.dedup_by(|left, right| {
        left.output_kind == right.output_kind && left.persistent_id == right.persistent_id
    });
    continued.sort_unstable();
    continued.dedup();
    retired.sort_unstable();
    retired.dedup();
    let transitioned_sources = continued
        .iter()
        .chain(&retired)
        .copied()
        .collect::<BTreeSet<_>>();

    let mut output_cursor = next_output.raw();
    let mut reservation_cursor = next_reservation.raw();
    let mut outputs = Vec::new();
    let mut identities = Vec::new();
    let mut writable_leaves = Vec::new();
    let mut reservations = Vec::new();

    for (created_ordinal, entity) in created.into_iter().enumerate() {
        let output = LineageOutputId::from_raw(output_cursor);
        output_cursor = output_cursor
            .checked_add(1)
            .ok_or(geosolve_sketch_lineage::LineageDocumentError::IdExhausted)?;
        // Port keys are compact semantic roles; exact native/logical identity
        // remains in the reservation or the owning action payload.  In
        // particular, parameter-binding identity is a canonical structured
        // value and can legitimately exceed the semantic-key byte bound.
        let key = format!("{}-{created_ordinal:04}", entity.key);
        writable_leaves.extend(writable.keys_for(&entity).map(|key| LineageWritableLeaf {
            output,
            key: key.clone(),
        }));
        if let Some(reservation_kind) = entity.reservation_kind {
            let reservation = LineageReservationId::from_raw(reservation_cursor);
            reservation_cursor = reservation_cursor
                .checked_add(1)
                .ok_or(geosolve_sketch_lineage::LineageDocumentError::IdExhausted)?;
            outputs.push(LineageOutput {
                id: output,
                key: LineageSemanticKey::new(key.clone())?,
                kind: entity.output_kind,
                reservation: Some(reservation),
            });
            identities.push(LineageOutputIdentity {
                output,
                flow: LineageOutputIdentityFlow::Created { reservation },
            });
            reservations.push(LineageReservation {
                id: reservation,
                key: LineageSemanticKey::new(key)?,
                kind: reservation_kind,
                persistent_id: LineageOpaqueId::new(format!(
                    "{}:{}",
                    output_kind_key(entity.output_kind),
                    entity.persistent_id
                ))?,
            });
        } else {
            outputs.push(LineageOutput {
                id: output,
                key: LineageSemanticKey::new(key)?,
                kind: entity.output_kind,
                reservation: None,
            });
            identities.push(LineageOutputIdentity {
                output,
                flow: LineageOutputIdentityFlow::OwnedLogical,
            });
        }
    }

    for (flow_key, sources, retired_flow) in
        [("continued", continued, false), ("retired", retired, true)]
    {
        for (ordinal, source) in sources.into_iter().enumerate() {
            let output = LineageOutputId::from_raw(output_cursor);
            output_cursor = output_cursor
                .checked_add(1)
                .ok_or(geosolve_sketch_lineage::LineageDocumentError::IdExhausted)?;
            outputs.push(LineageOutput {
                id: output,
                key: LineageSemanticKey::new(format!(
                    "{flow_key}-{ordinal:04}-{}",
                    output_kind_key(source.kind)
                ))?,
                kind: source.kind,
                reservation: None,
            });
            identities.push(LineageOutputIdentity {
                output,
                flow: if retired_flow {
                    LineageOutputIdentityFlow::Retired { source }
                } else {
                    LineageOutputIdentityFlow::Continued { source }
                },
            });
            if !retired_flow && let Some(keys) = continued_writable.get(&source) {
                writable_leaves.extend(keys.iter().map(|key| LineageWritableLeaf {
                    output,
                    key: key.clone(),
                }));
            }
        }
    }

    for (ordinal, input) in inputs
        .iter()
        .filter(|input| !transitioned_sources.contains(&input.source))
        .enumerate()
    {
        let output = LineageOutputId::from_raw(output_cursor);
        output_cursor = output_cursor
            .checked_add(1)
            .ok_or(geosolve_sketch_lineage::LineageDocumentError::IdExhausted)?;
        outputs.push(LineageOutput {
            id: output,
            key: LineageSemanticKey::new(format!(
                "alias-{ordinal:04}-{}",
                output_kind_key(input.kind)
            ))?,
            kind: input.kind,
            reservation: None,
        });
        identities.push(LineageOutputIdentity {
            output,
            flow: LineageOutputIdentityFlow::Aliased {
                source: input.source,
            },
        });
    }

    let output = LineageOutputId::from_raw(output_cursor);
    outputs.push(LineageOutput {
        id: output,
        key: LineageSemanticKey::new("result")?,
        kind: LineageOutputKind::Collection,
        reservation: None,
    });
    identities.push(LineageOutputIdentity {
        output,
        flow: LineageOutputIdentityFlow::OwnedLogical,
    });

    let _ = (document, step);
    Ok(StepManifest {
        inputs,
        outputs,
        identities,
        writable_leaves,
        reservations,
    })
}

#[derive(Default)]
struct MaterializedOutputIndex {
    by_raw: BTreeMap<String, Vec<LineageOutputRef>>,
    by_kind_and_raw: BTreeMap<(LineageOutputKind, String), LineageOutputRef>,
    by_materialized: BTreeMap<LineageMaterializedIdentity, LineageOutputRef>,
    raw_by_ref: BTreeMap<LineageOutputRef, String>,
}

fn materialized_output_index(document: &LineageDocument) -> MaterializedOutputIndex {
    materialized_output_index_for_steps(document.id(), document.steps().iter())
}

fn materialized_output_index_for_steps<'a>(
    document: LineageDocumentId,
    steps: impl IntoIterator<Item = &'a LineageStep>,
) -> MaterializedOutputIndex {
    let mut index = MaterializedOutputIndex::default();
    for step in steps {
        if !matches!(
            step.action,
            LineageActionDefinition::ImportedBaseline { .. }
        ) && !has_workbench_materialization_parameters(&step.action)
        {
            // Registered caller-authored actions are valid structural lineage,
            // but they are deliberately not executable workbench intent. Do
            // not let their caller-declared reservations participate in the
            // editor's native identity or reverse-owner lookup.
            continue;
        }
        let reservations = step
            .reservations
            .iter()
            .map(|reservation| (reservation.id, reservation))
            .collect::<BTreeMap<_, _>>();
        for output in &step.outputs {
            let reference = LineageOutputRef {
                document,
                step: step.id,
                output: output.id,
                kind: output.kind,
            };
            let raw = output
                .reservation
                .and_then(|reservation| reservations.get(&reservation))
                .and_then(|reservation| {
                    reservation
                        .persistent_id
                        .as_str()
                        .split_once(':')
                        .map(|(_, raw)| raw.to_owned())
                })
                .or_else(|| {
                    step.output_identity(output.id)
                        .and_then(LineageOutputIdentity::source)
                        .and_then(|source| index.raw_by_ref.get(&source).cloned())
                });
            let Some(raw) = raw else {
                continue;
            };
            index.raw_by_ref.insert(reference, raw.clone());
            match step
                .output_identity(output.id)
                .map(|identity| identity.flow)
            {
                Some(LineageOutputIdentityFlow::Aliased { .. }) => {}
                Some(
                    LineageOutputIdentityFlow::OwnedLogical
                    | LineageOutputIdentityFlow::Created { .. }
                    | LineageOutputIdentityFlow::Continued { .. }
                    | LineageOutputIdentityFlow::Retired { .. },
                )
                | None => {
                    index
                        .by_kind_and_raw
                        .insert((output.kind, raw.clone()), reference);
                    if let Some(kind) = reservation_kind_for_output(output.kind)
                        && let Ok(persistent_id) =
                            LineageOpaqueId::new(format!("{}:{raw}", output_kind_key(output.kind)))
                    {
                        index.by_materialized.insert(
                            LineageMaterializedIdentity {
                                kind,
                                persistent_id,
                            },
                            reference,
                        );
                    }
                }
            }
        }
    }
    for ((_, raw), reference) in &index.by_kind_and_raw {
        index
            .by_raw
            .entry(raw.clone())
            .or_default()
            .push(*reference);
    }
    for values in index.by_raw.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    index
}

fn collect_created_entities(
    before: &Value,
    delta: &[StructuralDeltaOperation],
) -> Vec<MaterializedEntity> {
    let mut created = Vec::new();
    for operation in delta {
        match operation {
            StructuralDeltaOperation::UpsertEntity {
                path, id, value, ..
            } => {
                let previous = entity_value_at_path(before, path, id);
                if previous.is_none()
                    && let Some((output_kind, reservation_kind)) =
                        entity_kind_for_collection(path.last().map(String::as_str))
                {
                    created.push(MaterializedEntity {
                        persistent_id: id.clone(),
                        key: output_kind_key(output_kind).into(),
                        output_kind,
                        reservation_kind: Some(reservation_kind),
                    });
                }
                let previous = previous
                    .map(|value| collect_materialized_entities_at(path, value))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|entity| (entity.output_kind, entity.persistent_id))
                    .collect::<BTreeSet<_>>();
                created.extend(
                    collect_materialized_entities_at(path, value)
                        .into_iter()
                        .filter(|entity| {
                            !previous.contains(&(entity.output_kind, entity.persistent_id.clone()))
                        }),
                );
            }
            StructuralDeltaOperation::SetField { path, value } => {
                let previous = value_at_path(before, path);
                for entity in collect_materialized_entities_at(path, value) {
                    let existed = previous.is_some_and(|previous| {
                        collect_materialized_entities_at(path, previous)
                            .iter()
                            .any(|candidate| {
                                candidate.output_kind == entity.output_kind
                                    && candidate.persistent_id == entity.persistent_id
                            })
                    });
                    if !existed {
                        created.push(entity);
                    }
                }
            }
            StructuralDeltaOperation::RemoveField { .. }
            | StructuralDeltaOperation::RemoveEntity { .. } => {}
        }
    }
    created.sort_by(|left, right| {
        (left.output_kind, &left.persistent_id).cmp(&(right.output_kind, &right.persistent_id))
    });
    created.dedup_by(|left, right| {
        left.output_kind == right.output_kind && left.persistent_id == right.persistent_id
    });
    created
}

fn structural_change_transitions_identity(
    before: &Value,
    operation: &StructuralDeltaOperation,
) -> bool {
    let entity_keys = |path: &[String], value: &Value| {
        collect_materialized_entities_at(path, value)
            .into_iter()
            // Only reservation-backed native identities require a later
            // lifecycle action when they appear or disappear inside an
            // existing structural parent. Logical side-table projections
            // such as a curve's default/Profile role remain writable leaves
            // of their already-authenticated owner step.
            .filter(|entity| entity.reservation_kind.is_some())
            .map(|entity| (entity.output_kind, entity.persistent_id))
            .collect::<BTreeSet<_>>()
    };
    match operation {
        StructuralDeltaOperation::RemoveEntity { path, .. } => {
            entity_kind_for_collection(path.last().map(String::as_str)).is_some()
        }
        StructuralDeltaOperation::UpsertEntity {
            path, id, value, ..
        } => entity_value_at_path(before, path, id).map_or_else(
            || {
                // A direct entry in a recognized persistent collection owns
                // a new native identity. Other ID-indexed side tables may
                // represent logical writable leaves (for example a curve's
                // non-default geometry role); only nested reservation-backed
                // children make those additions lifecycle changes.
                entity_kind_for_collection(path.last().map(String::as_str)).is_some()
                    || !entity_keys(path, value).is_empty()
            },
            |previous| entity_keys(path, previous) != entity_keys(path, value),
        ),
        StructuralDeltaOperation::SetField { path, value } => value_at_path(before, path)
            .is_some_and(|previous| entity_keys(path, previous) != entity_keys(path, value)),
        StructuralDeltaOperation::RemoveField { path } => value_at_path(before, path)
            .is_some_and(|previous| !entity_keys(path, previous).is_empty()),
    }
}

fn collect_materialized_entities(root: &Value) -> Vec<MaterializedEntity> {
    collect_materialized_entities_at(&[], root)
}

fn writable_leaf_catalog(root: &Value) -> Result<WritableLeafCatalog, LineageBridgeError> {
    let mut catalog = WritableLeafCatalog::default();
    collect_writable_leaf_catalog_inner(&[], root, &mut catalog)?;
    Ok(catalog)
}

fn collect_writable_leaf_catalog_inner(
    path: &[String],
    value: &Value,
    catalog: &mut WritableLeafCatalog,
) -> Result<(), LineageBridgeError> {
    match value {
        Value::Array(values) => {
            if let Some((kind, _)) = entity_kind_for_collection(path.last().map(String::as_str)) {
                for value in values {
                    let Some(id) = entity_id(value) else {
                        continue;
                    };
                    let mut leaves = BTreeMap::new();
                    collect_entity_leaf_values(&mut Vec::new(), value, &mut leaves)?;
                    let entry = catalog
                        .materialized
                        .entry((kind, id.to_owned()))
                        .or_default();
                    entry.extend(leaves.into_keys());
                    append_default_entity_leaves(entry, kind)?;

                    if matches!(
                        kind,
                        LineageOutputKind::Constraint | LineageOutputKind::Dimension
                    ) && let Some(source) = value
                        .as_object()
                        .and_then(|object| object.get("source_id"))
                        .and_then(Value::as_str)
                    {
                        let source_entry = catalog
                            .materialized
                            .entry((LineageOutputKind::Source, source.to_owned()))
                            .or_default();
                        append_default_entity_leaves(source_entry, LineageOutputKind::Source)?;
                    }
                }
            }
            for value in values {
                collect_writable_leaf_catalog_inner(path, value, catalog)?;
            }
        }
        Value::Object(object) => {
            if path.last().is_some_and(|field| field == "host_activation") {
                catalog
                    .logical
                    .entry((LineageOutputKind::Activation, "host-configuration".into()))
                    .or_default()
                    .insert(LineageSemanticKey::new("configuration")?);
            }
            if matches!(
                path.last().map(String::as_str),
                Some(
                    "constraints"
                        | "retained_planar_constraints"
                        | "dimensions"
                        | "profile_offset_dimensions"
                )
            ) && let Some(source) = object.get("source_id").and_then(Value::as_str)
            {
                let source_entry = catalog
                    .materialized
                    .entry((LineageOutputKind::Source, source.to_owned()))
                    .or_default();
                append_default_entity_leaves(source_entry, LineageOutputKind::Source)?;
            }
            for (key, value) in object {
                let mut nested = path.to_vec();
                nested.push(key.clone());
                collect_writable_leaf_catalog_inner(&nested, value, catalog)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn append_default_entity_leaves(
    leaves: &mut BTreeSet<LineageSemanticKey>,
    kind: LineageOutputKind,
) -> Result<(), LineageBridgeError> {
    if supports_document_activation(kind) {
        leaves.insert(LineageSemanticKey::new("activation")?);
    }
    if kind == LineageOutputKind::Curve {
        leaves.insert(LineageSemanticKey::new("role")?);
    }
    Ok(())
}

const fn supports_document_activation(kind: LineageOutputKind) -> bool {
    matches!(
        kind,
        LineageOutputKind::Point
            | LineageOutputKind::Scalar
            | LineageOutputKind::Curve
            | LineageOutputKind::Contact
            | LineageOutputKind::Constraint
            | LineageOutputKind::Dimension
            | LineageOutputKind::Source
            | LineageOutputKind::Parameter
            | LineageOutputKind::ExternalBinding
    )
}

fn collect_entity_leaf_values(
    path: &mut Vec<String>,
    value: &Value,
    leaves: &mut BTreeMap<LineageSemanticKey, Value>,
) -> Result<(), LineageBridgeError> {
    match value {
        Value::Object(object) => {
            if object.is_empty() && !path.is_empty() {
                leaves.insert(semantic_leaf_key(path)?, value.clone());
                return Ok(());
            }
            for (field, value) in object {
                if matches!(
                    field.as_str(),
                    "id" | "source_id" | "next_span_id" | "span_ids"
                ) {
                    continue;
                }
                if matches!(field.as_str(), "domain" | "neighborhood") {
                    // Tagged branch values change JSON shape as their explicit
                    // variant changes (for example bounded `{ lower, upper }`
                    // to the unit `supporting_line` variant). They remain one
                    // semantic property of one persistent output, so retain
                    // the complete branch value under one stable owner leaf.
                    path.push(field.clone());
                    leaves.insert(semantic_leaf_key(path)?, value.clone());
                    path.pop();
                    continue;
                }
                if value.as_array().is_some_and(|values| {
                    entity_kind_for_collection(Some(field)).is_some()
                        && values.iter().all(|value| entity_id(value).is_some())
                }) {
                    // Nested persistent children have their own materialized
                    // outputs and writable manifests. The parent must not
                    // duplicate their ownership under an array-valued leaf.
                    continue;
                }
                path.push(field.clone());
                collect_entity_leaf_values(path, value, leaves)?;
                path.pop();
            }
        }
        Value::Array(values)
            if values.len() == 2 && values.iter().all(Value::is_number) && !path.is_empty() =>
        {
            for (axis, value) in ["x", "y"].into_iter().zip(values) {
                path.push(axis.into());
                leaves.insert(semantic_leaf_key(path)?, value.clone());
                path.pop();
            }
        }
        Value::Array(_) | Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            if !path.is_empty() {
                leaves.insert(semantic_leaf_key(path)?, value.clone());
            }
        }
    }
    Ok(())
}

fn semantic_leaf_key(path: &[String]) -> Result<LineageSemanticKey, LineageBridgeError> {
    if path.is_empty() {
        return Err(LineageBridgeError::InvalidMaterialization(
            "writable leaf path must not be empty",
        ));
    }
    Ok(LineageSemanticKey::new(path.join("."))?)
}

fn deletion_targets(
    replay: &ReplayAction,
    delta: &[StructuralDeltaOperation],
) -> BTreeSet<(LineageOutputKind, String)> {
    let mut targets = BTreeSet::new();
    let deletion = matches!(
        replay,
        ReplayAction::Delete { .. }
            | ReplayAction::RemoveComputedFeature { .. }
            | ReplayAction::RemoveComputedCorner { .. }
            | ReplayAction::Edit {
                edit: geosolve_sketch::DocumentEdit::Delete { .. },
                ..
            }
    );
    match replay {
        ReplayAction::Delete { selection, .. } => {
            for item in selection {
                let target = match item {
                    SelectionItem::Point(id) => (LineageOutputKind::Point, id.to_string()),
                    SelectionItem::Curve(span) => {
                        (LineageOutputKind::Curve, span.curve.to_string())
                    }
                    SelectionItem::Constraint(id) => {
                        (LineageOutputKind::Constraint, id.to_string())
                    }
                    SelectionItem::Dimension(id) => (LineageOutputKind::Dimension, id.to_string()),
                    SelectionItem::Feature(id) => (LineageOutputKind::Feature, id.to_string()),
                    SelectionItem::FeatureCorner(owner) => {
                        (LineageOutputKind::FeatureCorner, owner.corner.to_string())
                    }
                    SelectionItem::Datum(_) => continue,
                };
                targets.insert(target);
            }
        }
        ReplayAction::RemoveComputedFeature { feature, .. } => {
            targets.insert((LineageOutputKind::Feature, feature.to_string()));
        }
        ReplayAction::RemoveComputedCorner { owner, .. } => {
            targets.insert((LineageOutputKind::FeatureCorner, owner.corner.to_string()));
        }
        ReplayAction::Edit {
            edit: geosolve_sketch::DocumentEdit::Delete { object },
            ..
        } => insert_document_object_target(&mut targets, *object),
        _ => {}
    }
    if deletion {
        for operation in delta {
            if let StructuralDeltaOperation::RemoveEntity { path, id } = operation
                && let Some((kind, _)) = entity_kind_for_collection(path.last().map(String::as_str))
            {
                targets.insert((kind, id.clone()));
            }
        }
    }
    targets
}

fn insert_document_object_target(
    targets: &mut BTreeSet<(LineageOutputKind, String)>,
    object: geosolve_sketch::DocumentObjectId,
) {
    let target = match object {
        geosolve_sketch::DocumentObjectId::Point(id) => (LineageOutputKind::Point, id.to_string()),
        geosolve_sketch::DocumentObjectId::Scalar(id) => {
            (LineageOutputKind::Scalar, id.to_string())
        }
        geosolve_sketch::DocumentObjectId::Curve(id) => (LineageOutputKind::Curve, id.to_string()),
        geosolve_sketch::DocumentObjectId::Contact(id) => {
            (LineageOutputKind::Contact, id.to_string())
        }
        geosolve_sketch::DocumentObjectId::Constraint(id) => {
            (LineageOutputKind::Constraint, id.to_string())
        }
        geosolve_sketch::DocumentObjectId::Dimension(id) => {
            (LineageOutputKind::Dimension, id.to_string())
        }
        geosolve_sketch::DocumentObjectId::Parameter(id) => {
            (LineageOutputKind::Parameter, id.to_string())
        }
        geosolve_sketch::DocumentObjectId::ExternalBinding(id) => {
            (LineageOutputKind::ExternalBinding, id.to_string())
        }
    };
    targets.insert(target);
}

fn collect_materialized_entities_at(path: &[String], value: &Value) -> Vec<MaterializedEntity> {
    let mut entities = BTreeMap::<(LineageOutputKind, String), MaterializedEntity>::new();
    collect_entities_inner(path, value, &mut entities);
    entities.into_values().collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "one recursive collector keeps every typed native and logical materialization family exhaustive"
)]
fn collect_entities_inner(
    path: &[String],
    value: &Value,
    entities: &mut BTreeMap<(LineageOutputKind, String), MaterializedEntity>,
) {
    match value {
        Value::Array(values) => {
            if let Some((output_kind, reservation_kind)) =
                entity_kind_for_collection(path.last().map(String::as_str))
            {
                for value in values {
                    if let Some(id) = entity_id(value) {
                        insert_materialized_entity(entities, id, output_kind, reservation_kind);
                        if matches!(
                            output_kind,
                            LineageOutputKind::Constraint | LineageOutputKind::Dimension
                        ) && let Some(source) = value
                            .as_object()
                            .and_then(|object| object.get("source_id"))
                            .and_then(Value::as_str)
                        {
                            insert_materialized_entity(
                                entities,
                                source,
                                LineageOutputKind::Source,
                                LineageReservationKind::Source,
                            );
                        }
                    }
                }
            }
            match path.last().map(String::as_str) {
                Some("parameter_bindings") => {
                    for value in values {
                        if let Some(id) = entity_id_at_path(path, value) {
                            insert_logical_entity(
                                entities,
                                &id,
                                LineageOutputKind::ParameterBinding,
                            );
                        }
                    }
                }
                Some("parameter_outputs") => {
                    for value in values {
                        if let Some(id) = entity_id_at_path(path, value) {
                            insert_logical_entity(
                                entities,
                                &id,
                                LineageOutputKind::ParameterOutput,
                            );
                        }
                    }
                }
                Some("geometry_roles") => {
                    for value in values {
                        if let Some(curve) = value
                            .as_object()
                            .and_then(|object| object.get("curve"))
                            .and_then(Value::as_str)
                        {
                            insert_logical_entity(entities, curve, LineageOutputKind::GeometryRole);
                        }
                    }
                }
                _ => {}
            }
            for value in values {
                collect_entities_inner(path, value, entities);
            }
        }
        Value::Object(object) => {
            match path.last().map(String::as_str) {
                Some("parameter_bindings") => {
                    if let Some(id) = entity_id_at_path(path, value) {
                        insert_logical_entity(entities, &id, LineageOutputKind::ParameterBinding);
                    }
                }
                Some("parameter_outputs") => {
                    if let Some(id) = entity_id_at_path(path, value) {
                        insert_logical_entity(entities, &id, LineageOutputKind::ParameterOutput);
                    }
                }
                Some("geometry_roles") => {
                    if let Some(curve) = object.get("curve").and_then(Value::as_str) {
                        insert_logical_entity(entities, curve, LineageOutputKind::GeometryRole);
                    }
                }
                _ => {}
            }
            if path.last().is_some_and(|field| field == "host_activation") {
                insert_logical_entity(
                    entities,
                    "host-configuration",
                    LineageOutputKind::Activation,
                );
            }
            if matches!(
                path.last().map(String::as_str),
                Some(
                    "constraints"
                        | "retained_planar_constraints"
                        | "dimensions"
                        | "profile_offset_dimensions"
                )
            ) && let Some(source) = object.get("source_id").and_then(Value::as_str)
            {
                insert_materialized_entity(
                    entities,
                    source,
                    LineageOutputKind::Source,
                    LineageReservationKind::Source,
                );
            }
            if path.last().is_some_and(|field| field == "curves")
                && let Some(curve) = object.get("id").and_then(Value::as_str)
            {
                // Every native curve owns one stable logical role port even
                // when Profile is represented by absence from the draft-v5
                // non-default side table.
                insert_logical_entity(entities, curve, LineageOutputKind::GeometryRole);
                if let Some(spans) = object
                    .get("definition")
                    .and_then(Value::as_object)
                    .and_then(|definition| definition.get("span_ids"))
                    .and_then(Value::as_array)
                {
                    for span in spans {
                        if let Some(span) = span.as_u64() {
                            insert_materialized_entity(
                                entities,
                                &format!("{curve}/{span}"),
                                LineageOutputKind::CurveSpan,
                                LineageReservationKind::CurveSpan,
                            );
                        }
                    }
                }
            }
            for (key, value) in object {
                let mut nested = path.to_vec();
                nested.push(key.clone());
                collect_entities_inner(&nested, value, entities);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn insert_materialized_entity(
    entities: &mut BTreeMap<(LineageOutputKind, String), MaterializedEntity>,
    id: &str,
    output_kind: LineageOutputKind,
    reservation_kind: LineageReservationKind,
) {
    entities.insert(
        (output_kind, id.to_owned()),
        MaterializedEntity {
            persistent_id: id.to_owned(),
            key: output_kind_key(output_kind).into(),
            output_kind,
            reservation_kind: Some(reservation_kind),
        },
    );
}

fn insert_logical_entity(
    entities: &mut BTreeMap<(LineageOutputKind, String), MaterializedEntity>,
    id: &str,
    output_kind: LineageOutputKind,
) {
    entities.insert(
        (output_kind, id.to_owned()),
        MaterializedEntity {
            persistent_id: id.to_owned(),
            key: output_kind_key(output_kind).into(),
            output_kind,
            reservation_kind: None,
        },
    );
}

fn entity_kind_for_collection(
    collection: Option<&str>,
) -> Option<(LineageOutputKind, LineageReservationKind)> {
    Some(match collection? {
        "points" => (LineageOutputKind::Point, LineageReservationKind::Point),
        "scalars" => (LineageOutputKind::Scalar, LineageReservationKind::Scalar),
        "curves" => (LineageOutputKind::Curve, LineageReservationKind::Curve),
        "contacts" => (LineageOutputKind::Contact, LineageReservationKind::Contact),
        "trim_views" => (
            LineageOutputKind::TrimView,
            LineageReservationKind::TrimView,
        ),
        "constraints" | "retained_planar_constraints" => (
            LineageOutputKind::Constraint,
            LineageReservationKind::Constraint,
        ),
        "dimensions" | "profile_offset_dimensions" => (
            LineageOutputKind::Dimension,
            LineageReservationKind::Dimension,
        ),
        "parameters" => (
            LineageOutputKind::Parameter,
            LineageReservationKind::Parameter,
        ),
        "external_bindings" => (
            LineageOutputKind::ExternalBinding,
            LineageReservationKind::ExternalBinding,
        ),
        "features" => (LineageOutputKind::Feature, LineageReservationKind::Feature),
        "corners" => (
            LineageOutputKind::FeatureCorner,
            LineageReservationKind::FeatureCorner,
        ),
        "annotations" => (
            LineageOutputKind::Annotation,
            LineageReservationKind::Annotation,
        ),
        _ => return None,
    })
}

const fn output_kind_key(kind: LineageOutputKind) -> &'static str {
    match kind {
        LineageOutputKind::Point => "point",
        LineageOutputKind::Scalar => "scalar",
        LineageOutputKind::Curve => "curve",
        LineageOutputKind::CurveSpan => "curve-span",
        LineageOutputKind::TrimView => "trim-view",
        LineageOutputKind::Contact => "contact",
        LineageOutputKind::Constraint => "constraint",
        LineageOutputKind::Dimension => "dimension",
        LineageOutputKind::Source => "source",
        LineageOutputKind::Parameter => "parameter",
        LineageOutputKind::ParameterBinding => "parameter-binding",
        LineageOutputKind::ParameterOutput => "parameter-output",
        LineageOutputKind::ExternalBinding => "external-binding",
        LineageOutputKind::GeometryRole => "geometry-role",
        LineageOutputKind::Activation => "activation",
        LineageOutputKind::Profile => "profile",
        LineageOutputKind::Chain => "chain",
        LineageOutputKind::Operation => "operation",
        LineageOutputKind::Feature => "feature",
        LineageOutputKind::FeatureCorner => "feature-corner",
        LineageOutputKind::Annotation => "annotation",
        LineageOutputKind::Collection => "collection",
    }
}

const fn reservation_kind_for_output(kind: LineageOutputKind) -> Option<LineageReservationKind> {
    Some(match kind {
        LineageOutputKind::Point => LineageReservationKind::Point,
        LineageOutputKind::Scalar => LineageReservationKind::Scalar,
        LineageOutputKind::Curve => LineageReservationKind::Curve,
        LineageOutputKind::CurveSpan => LineageReservationKind::CurveSpan,
        LineageOutputKind::TrimView => LineageReservationKind::TrimView,
        LineageOutputKind::Contact => LineageReservationKind::Contact,
        LineageOutputKind::Constraint => LineageReservationKind::Constraint,
        LineageOutputKind::Dimension => LineageReservationKind::Dimension,
        LineageOutputKind::Source => LineageReservationKind::Source,
        LineageOutputKind::Parameter => LineageReservationKind::Parameter,
        LineageOutputKind::ParameterBinding => LineageReservationKind::ParameterBinding,
        LineageOutputKind::ParameterOutput => LineageReservationKind::ParameterOutput,
        LineageOutputKind::ExternalBinding => LineageReservationKind::ExternalBinding,
        LineageOutputKind::Profile => LineageReservationKind::Profile,
        LineageOutputKind::Chain => LineageReservationKind::Chain,
        LineageOutputKind::Operation => LineageReservationKind::Operation,
        LineageOutputKind::Feature => LineageReservationKind::Feature,
        LineageOutputKind::FeatureCorner => LineageReservationKind::FeatureCorner,
        LineageOutputKind::Annotation => LineageReservationKind::Annotation,
        LineageOutputKind::GeometryRole
        | LineageOutputKind::Activation
        | LineageOutputKind::Collection => return None,
    })
}

fn value_at_path<'a>(root: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut value = root;
    for segment in path {
        value = value.as_object()?.get(segment)?;
    }
    Some(value)
}

fn entity_exists(root: &Value, path: &[String], id: &str) -> Result<bool, LineageBridgeError> {
    let Some(value) = value_at_path(root, path) else {
        return Ok(false);
    };
    let values = value
        .as_array()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization entity path is not an array",
        ))?;
    Ok(values
        .iter()
        .any(|candidate| entity_id_at_path(path, candidate).as_deref() == Some(id)))
}

fn collect_operation_strings<'a>(
    operation: &'a StructuralDeltaOperation,
    strings: &mut Vec<&'a str>,
) {
    match operation {
        StructuralDeltaOperation::SetField { value, .. }
        | StructuralDeltaOperation::UpsertEntity { value, .. } => {
            collect_value_reference_strings(value, None, strings);
        }
        StructuralDeltaOperation::RemoveField { .. } => {}
        StructuralDeltaOperation::RemoveEntity { id, .. } => strings.push(id),
    }
}

fn collect_value_reference_strings<'a>(
    value: &'a Value,
    field: Option<&str>,
    strings: &mut Vec<&'a str>,
) {
    match value {
        Value::String(value) if field.is_none_or(is_reference_field) => strings.push(value),
        Value::Array(values) => {
            for value in values {
                collect_value_reference_strings(value, field, strings);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                collect_value_reference_strings(value, Some(key), strings);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_reference_field(field: &str) -> bool {
    !matches!(
        field,
        "id" | "label"
            | "kind"
            | "version"
            | "digest"
            | "encoding"
            | "media_type"
            | "unit"
            | "domain"
            | "mode"
            | "role"
            | "state"
            | "sweep"
            | "document_id"
            | "sketch_document"
            | "next_id"
            | "next_feature_id"
            | "next_corner_id"
    )
}

fn authored_output_field_manifest(
    outputs: &[LineageOutput],
    writable_leaves: &[LineageWritableLeaf],
) -> Result<Vec<AuthoredOutputFieldKey>, LineageBridgeError> {
    let output_kinds = outputs
        .iter()
        .map(|output| (output.id, output.kind))
        .collect::<BTreeMap<_, _>>();
    let mut manifest = writable_leaves
        .iter()
        .map(|leaf| {
            Ok(AuthoredOutputFieldKey {
                output: leaf.output,
                kind: *output_kinds
                    .get(&leaf.output)
                    .ok_or(LineageBridgeError::MissingOwner)?,
                field: leaf.key.clone(),
            })
        })
        .collect::<Result<Vec<_>, LineageBridgeError>>()?;
    manifest.sort_unstable();
    if manifest.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored output field manifest contains a duplicate field",
        ));
    }
    Ok(manifest)
}

/// Captures the complete current semantic value of every writable output
/// leaf when a step is first authored. These values live in the durable
/// intent and are subsequently rewritten by direct manipulation; complete
/// entity snapshots remain disposable materialization cache bytes.
fn authored_output_field_values_for_manifest(
    document: &LineageDocument,
    manifest: &StepManifest,
    materialized: &Value,
) -> Result<Vec<AuthoredOutputFieldValue>, LineageBridgeError> {
    let predecessor = materialized_output_index(document);
    let mut fields = manifest
        .writable_leaves
        .iter()
        .map(|writable| {
            let output = manifest
                .outputs
                .iter()
                .find(|output| output.id == writable.output)
                .ok_or(LineageBridgeError::MissingOwner)?;
            let value = match manifest_output_raw(&predecessor, manifest, output)? {
                Some(raw) => materialized_leaf_value(
                    materialized,
                    &materialized_leaf(output.kind, &raw, writable.key.as_str())?,
                )?,
                None => logical_output_field_value(materialized, output.kind, &writable.key)?,
            };
            Ok(AuthoredOutputFieldValue {
                output: output.id,
                kind: output.kind,
                field: writable.key.clone(),
                value,
            })
        })
        .collect::<Result<Vec<_>, LineageBridgeError>>()?;
    fields.sort_by(|left, right| {
        (&left.output, &left.kind, &left.field).cmp(&(&right.output, &right.kind, &right.field))
    });
    Ok(fields)
}

fn manifest_output_raw(
    predecessor: &MaterializedOutputIndex,
    manifest: &StepManifest,
    output: &LineageOutput,
) -> Result<Option<String>, LineageBridgeError> {
    let identity = manifest
        .identities
        .iter()
        .find(|identity| identity.output == output.id)
        .ok_or(LineageBridgeError::MissingOwner)?;
    match identity.flow {
        LineageOutputIdentityFlow::Created { reservation } => {
            let reservation = manifest
                .reservations
                .iter()
                .find(|candidate| candidate.id == reservation)
                .ok_or(LineageBridgeError::MissingOwner)?;
            let (prefix, raw) = reservation
                .persistent_id
                .as_str()
                .split_once(':')
                .ok_or(LineageBridgeError::MissingOwner)?;
            if materialized_identity_prefix(reservation.kind) != prefix {
                return Err(LineageBridgeError::MissingOwner);
            }
            Ok(Some(raw.to_owned()))
        }
        LineageOutputIdentityFlow::Continued { source }
        | LineageOutputIdentityFlow::Retired { source }
        | LineageOutputIdentityFlow::Aliased { source } => predecessor
            .raw_by_ref
            .get(&source)
            .cloned()
            .map(Some)
            .ok_or(LineageBridgeError::MissingOwner),
        LineageOutputIdentityFlow::OwnedLogical => Ok(None),
    }
}

fn logical_output_field_value(
    materialized: &Value,
    kind: LineageOutputKind,
    field: &LineageSemanticKey,
) -> Result<Value, LineageBridgeError> {
    if kind == LineageOutputKind::Activation && field.as_str() == "configuration" {
        return value_at_path(
            materialized,
            &["design".into(), "document".into(), "host_activation".into()],
        )
        .cloned()
        .ok_or(LineageBridgeError::MissingOwner);
    }
    Err(LineageBridgeError::InvalidMaterialization(
        "logical writable output has no semantic value compiler",
    ))
}

fn attach_owned_output_field_manifest(
    intent: &mut Value,
    outputs: &[LineageOutput],
    writable_leaves: &[LineageWritableLeaf],
    output_fields: &[AuthoredOutputFieldValue],
) -> Result<(), LineageBridgeError> {
    let object = intent
        .as_object_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent is not an object",
        ))?;
    let manifest = authored_output_field_manifest(outputs, writable_leaves)?;
    let field_manifest = output_fields
        .iter()
        .map(|field| AuthoredOutputFieldKey {
            output: field.output,
            kind: field.kind,
            field: field.field.clone(),
        })
        .collect::<Vec<_>>();
    if field_manifest != manifest {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored output field values do not exactly cover the writable manifest",
        ));
    }
    if object
        .insert(
            AUTHORED_OUTPUT_FIELD_MANIFEST.into(),
            serde_json::to_value(manifest)?,
        )
        .is_some()
    {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent already contains an output field manifest",
        ));
    }
    if object
        .insert(
            "owned_output_fields".into(),
            serde_json::to_value(output_fields)?,
        )
        .is_some()
    {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent already contains output field values",
        ));
    }
    Ok(())
}

fn action_for_replay(
    replay: &ReplayAction,
    variant: Option<GeometryToolVariant>,
    delta: &[StructuralDeltaOperation],
    inputs: Vec<LineageInputBinding>,
    context: ReplayActionContext<'_>,
) -> Result<LineageActionDefinition, LineageBridgeError> {
    validate_authored_operations(delta)?;
    let mut intent = compile_replay_intent(replay, variant)?;
    attach_owned_output_field_manifest(
        &mut intent.value,
        context.outputs,
        context.writable_leaves,
        context.output_fields,
    )?;
    let owner_fields = serde_json::to_value(delta)?;
    let intent_digest = authored_intent_digest(&intent.value, &owner_fields)?;
    let mut parameters = BTreeMap::new();
    parameters.insert(AUTHORED_INTENT_PARAMETER.into(), intent.value);
    parameters.insert(AUTHORED_OWNER_FIELDS_PARAMETER.into(), owner_fields);
    parameters.insert(
        HOST_INPUT_PROVENANCE_PARAMETER.into(),
        serde_json::to_value(LineageHostInputProvenance::capture(
            context.host_parameters,
            context.host_snapshots,
        )?)?,
    );
    parameters.insert(
        MATERIALIZATION_PARAMETER.into(),
        serde_json::to_value(CompiledMaterializationRecipe {
            version: 1,
            intent_digest,
            operations: delta.to_vec(),
        })?,
    );
    let payload = VersionedActionPayload {
        schema: LineageSemanticKey::new(intent.schema)?,
        version: 1,
        inputs,
        parameters,
    };
    Ok(match intent.category {
        geosolve_sketch_lineage::LineageActionKind::GeometryRecipe => {
            LineageActionDefinition::GeometryRecipe { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Constraint => {
            LineageActionDefinition::Constraint { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Dimension => {
            LineageActionDefinition::Dimension { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Trim => {
            LineageActionDefinition::Trim { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Parameter => {
            LineageActionDefinition::Parameter { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Binding => {
            LineageActionDefinition::Binding { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::External => {
            LineageActionDefinition::External { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Operation => {
            LineageActionDefinition::Operation { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::ComputedFeature => {
            LineageActionDefinition::ComputedFeature { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::Annotation => {
            LineageActionDefinition::Annotation { action: payload }
        }
        geosolve_sketch_lineage::LineageActionKind::ImportedBaseline => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "ordinary replay cannot compile an imported baseline action",
            ));
        }
    })
}

#[derive(Clone, Copy)]
struct ReplayActionContext<'a> {
    outputs: &'a [LineageOutput],
    writable_leaves: &'a [LineageWritableLeaf],
    output_fields: &'a [AuthoredOutputFieldValue],
    host_parameters: &'a ParameterBatch,
    host_snapshots: &'a ExternalSnapshotSet,
}

fn action_for_operation(
    proposal: &SketchOperationProposal,
    delta: &[StructuralDeltaOperation],
    inputs: Vec<LineageInputBinding>,
    context: ReplayActionContext<'_>,
) -> Result<LineageActionDefinition, LineageBridgeError> {
    validate_authored_operations(delta)?;
    let mut intent = operation_intent(proposal)?;
    attach_owned_output_field_manifest(
        &mut intent,
        context.outputs,
        context.writable_leaves,
        context.output_fields,
    )?;
    let owner_fields = serde_json::to_value(delta)?;
    let intent_digest = authored_intent_digest(&intent, &owner_fields)?;
    let mut parameters = BTreeMap::new();
    parameters.insert(AUTHORED_INTENT_PARAMETER.into(), intent);
    parameters.insert(AUTHORED_OWNER_FIELDS_PARAMETER.into(), owner_fields);
    parameters.insert(
        HOST_INPUT_PROVENANCE_PARAMETER.into(),
        serde_json::to_value(LineageHostInputProvenance::capture(
            context.host_parameters,
            context.host_snapshots,
        )?)?,
    );
    parameters.insert(
        MATERIALIZATION_PARAMETER.into(),
        serde_json::to_value(CompiledMaterializationRecipe {
            version: 1,
            intent_digest,
            operations: delta.to_vec(),
        })?,
    );
    Ok(LineageActionDefinition::Operation {
        action: VersionedActionPayload {
            schema: LineageSemanticKey::new(format!(
                "geosolve.operation.v1.{}",
                proposal.request().kind().semantic_key()
            ))?,
            version: 1,
            inputs,
            parameters,
        },
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed serializer keeps all twelve operation authoring schemas exhaustive and data-only"
)]
fn operation_intent(proposal: &SketchOperationProposal) -> Result<Value, LineageBridgeError> {
    let body = match proposal.request() {
        SketchOperationRequest::Split {
            support,
            parameter,
            retained,
        } => serde_json::json!({
            "support": support,
            "parameter": parameter,
            "retained": split_retained_key(*retained),
        }),
        SketchOperationRequest::Break {
            support,
            start,
            end,
            retained,
        } => serde_json::json!({
            "support": support,
            "start": start,
            "end": end,
            "retained": split_retained_key(*retained),
        }),
        SketchOperationRequest::Trim {
            support,
            parameter,
            retained,
        } => serde_json::json!({
            "support": support,
            "parameter": parameter,
            "retained": trim_retained_key(*retained),
        }),
        SketchOperationRequest::ExtendLineToLine {
            line,
            endpoint,
            target,
        } => serde_json::json!({
            "line": line,
            "endpoint": line_endpoint_key(*endpoint),
            "target": target,
        }),
        SketchOperationRequest::Mirror {
            label,
            source,
            axis,
        } => serde_json::json!({
            "label": label,
            "source": source,
            "axis": axis,
        }),
        SketchOperationRequest::Chamfer {
            label,
            first,
            second,
            first_distance,
            second_distance,
        } => serde_json::json!({
            "label": label,
            "first": first,
            "second": second,
            "first_distance": first_distance,
            "second_distance": second_distance,
        }),
        SketchOperationRequest::AssociativeFillet { label, request } => serde_json::json!({
            "label": label,
            "request": request,
        }),
        SketchOperationRequest::Rectangle {
            label,
            origin,
            width,
            height,
        } => serde_json::json!({
            "label": label,
            "origin": origin,
            "width": width,
            "height": height,
        }),
        SketchOperationRequest::RegularPolygon {
            label,
            center,
            radius,
            sides,
            rotation,
        } => serde_json::json!({
            "label": label,
            "center": center,
            "radius": radius,
            "sides": sides,
            "rotation": rotation,
        }),
        SketchOperationRequest::Slot {
            label,
            first_center,
            second_center,
            radius,
        } => serde_json::json!({
            "label": label,
            "first_center": first_center,
            "second_center": second_center,
            "radius": radius,
        }),
        SketchOperationRequest::LinearPattern {
            label,
            sources,
            instances,
            step,
        } => serde_json::json!({
            "label": label,
            "sources": sources,
            "instances": instances,
            "step": step,
        }),
        SketchOperationRequest::ProfileOffset {
            label,
            distance,
            operand,
            ..
        } => serde_json::json!({
            "label": label,
            "distance": distance,
            "operand": profile_offset_operand_intent(operand),
        }),
        _ => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported sketch operation request",
            ));
        }
    };
    Ok(serde_json::json!({
        "version": 1,
        "kind": proposal.request().kind().semantic_key(),
        "source_free_geometry_role": proposal.source_free_geometry_role(),
        "body": body,
    }))
}

fn profile_offset_operand_intent(operand: &SketchProfileOffsetOperand) -> Value {
    match operand {
        SketchProfileOffsetOperand::Face { key, direction } => serde_json::json!({
            "kind": "face",
            "direction": direction,
            "outer": offset_spans_intent(&key.outer.spans),
            "holes": key
                .holes
                .iter()
                .map(|hole| offset_spans_intent(&hole.spans))
                .collect::<Vec<_>>(),
        }),
        SketchProfileOffsetOperand::OpenChain { spans, side } => serde_json::json!({
            "kind": "open_chain",
            "side": side,
            "spans": offset_spans_intent(spans),
        }),
    }
}

fn offset_spans_intent(spans: &[OffsetDirectedSpan]) -> Vec<Value> {
    spans
        .iter()
        .map(|span| {
            serde_json::json!({
                "span": span.span,
                "traversal": offset_traversal_key(span.traversal),
            })
        })
        .collect()
}

const fn split_retained_key(retained: SplitRetainedPiece) -> &'static str {
    match retained {
        SplitRetainedPiece::Before => "before",
        SplitRetainedPiece::After => "after",
    }
}

const fn trim_retained_key(retained: TrimRetainedSide) -> &'static str {
    match retained {
        TrimRetainedSide::Before => "before",
        TrimRetainedSide::After => "after",
    }
}

const fn line_endpoint_key(endpoint: LineEndpoint) -> &'static str {
    match endpoint {
        LineEndpoint::Start => "start",
        LineEndpoint::End => "end",
    }
}

const fn offset_traversal_key(traversal: OffsetTraversal) -> &'static str {
    match traversal {
        OffsetTraversal::Forward => "forward",
        OffsetTraversal::Reverse => "reverse",
    }
}

fn authored_intent_digest(
    intent: &Value,
    owner_fields: &Value,
) -> Result<LineageDigest, LineageBridgeError> {
    Ok(lineage_content_digest(&serde_json::to_vec(
        &serde_json::json!({
            "authored_intent": intent,
            "authored_owner_fields": owner_fields,
        }),
    )?))
}

fn replay_label(replay: &ReplayAction) -> String {
    match replay {
        ReplayAction::Construction { .. } | ReplayAction::ConstructionPlan { .. } => {
            "Geometry recipe"
        }
        ReplayAction::ConstraintAction { .. } => "Constraint",
        ReplayAction::DimensionAction { .. }
        | ReplayAction::PointDistance { .. }
        | ReplayAction::SegmentLength { .. } => "Dimension",
        ReplayAction::CreateComputedFillet { .. } => "Computed Fillet",
        ReplayAction::Delete { .. }
        | ReplayAction::RemoveComputedFeature { .. }
        | ReplayAction::RemoveComputedCorner { .. } => "Delete",
        _ => "Edit",
    }
    .into()
}

fn action_delta(
    action: &LineageActionDefinition,
) -> Result<Vec<StructuralDeltaOperation>, LineageBridgeError> {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => return Ok(Vec::new()),
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let intent = payload.parameters.get(AUTHORED_INTENT_PARAMETER).ok_or(
        LineageBridgeError::InvalidMaterialization("action has no authored semantic intent"),
    )?;
    let owner_fields = payload
        .parameters
        .get(AUTHORED_OWNER_FIELDS_PARAMETER)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "action has no authored owner fields",
        ))?;
    let compiled = payload.parameters.get(MATERIALIZATION_PARAMETER).ok_or(
        LineageBridgeError::InvalidMaterialization("action has no compiled materialization"),
    )?;
    let compiled = serde_json::from_value::<CompiledMaterializationRecipe>(compiled.clone())?;
    if compiled.version != 1 {
        return Err(LineageBridgeError::InvalidMaterialization(
            "unsupported compiled materialization version",
        ));
    }
    if compiled.intent_digest != authored_intent_digest(intent, owner_fields)? {
        return Err(LineageBridgeError::InvalidMaterialization(
            "compiled materialization does not match authored intent",
        ));
    }
    let authored = serde_json::from_value::<Vec<StructuralDeltaOperation>>(owner_fields.clone())?;
    let mut authored_targets = authored
        .iter()
        .map(StructuralDeltaOperation::target_key)
        .collect::<Vec<_>>();
    authored_targets.sort_unstable();
    let mut compiled_targets = compiled
        .operations
        .iter()
        .map(StructuralDeltaOperation::target_key)
        .collect::<Vec<_>>();
    compiled_targets.sort_unstable();
    if compiled_targets != authored_targets {
        return Err(LineageBridgeError::InvalidMaterialization(
            "compiled materialization targets differ from authored owner fields",
        ));
    }
    if compiled.operations.len() > MAX_MATERIALIZATION_PATCH_OPERATIONS {
        return Err(LineageBridgeError::PatchResourceLimit);
    }
    validate_authored_operations(&compiled.operations)?;
    Ok(compiled.operations)
}

fn action_delta_for_step(
    document: &LineageDocument,
    step: &LineageStep,
) -> Result<Vec<StructuralDeltaOperation>, LineageBridgeError> {
    let mut operations = action_delta(&step.action)?;
    let authored_operations = authored_action_delta(&step.action)?;
    let declared_manifest = action_output_field_manifest(&step.action)?;
    let retained_manifest = authored_output_field_manifest(&step.outputs, &step.writable_leaves)?;
    if declared_manifest != retained_manifest {
        return Err(LineageBridgeError::WritableManifestMismatch { step: step.id });
    }
    let fields = authored_output_fields(&step.action)?;
    let field_manifest = fields
        .iter()
        .map(|field| AuthoredOutputFieldKey {
            output: field.output,
            kind: field.kind,
            field: field.field.clone(),
        })
        .collect::<Vec<_>>();
    if field_manifest != declared_manifest {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored output field values do not exactly cover the writable manifest",
        ));
    }
    let index = materialized_output_index(document);
    let mut previous = None;
    let mut authenticated_authored = authored_operations.clone();
    for field in &fields {
        let key = (field.output, field.kind, field.field.clone());
        if previous.as_ref().is_some_and(|previous| previous >= &key) {
            return Err(LineageBridgeError::InvalidMaterialization(
                "authored output fields are not uniquely canonical",
            ));
        }
        previous = Some(key);
        let output = step
            .outputs
            .iter()
            .find(|output| output.id == field.output && output.kind == field.kind)
            .ok_or(LineageBridgeError::MissingOwner)?;
        if !step
            .writable_leaves
            .iter()
            .any(|writable| writable.output == output.id && writable.key == field.field)
        {
            return Err(LineageBridgeError::MissingOwner);
        }
        let reference = LineageOutputRef {
            document: document.id(),
            step: step.id,
            output: output.id,
            kind: output.kind,
        };
        let persistent_id = if field.kind == LineageOutputKind::Activation
            && field.field.as_str() == "configuration"
        {
            "host-configuration"
        } else {
            index
                .raw_by_ref
                .get(&reference)
                .map(String::as_str)
                .ok_or(LineageBridgeError::MissingOwner)?
        };
        apply_authored_output_field(&mut authenticated_authored, field, persistent_id)?;
        apply_authored_output_field(&mut operations, field, persistent_id)?;
    }
    if authenticated_authored != authored_operations {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored owner fields contradict the semantic output field values",
        ));
    }
    validate_authored_operations(&operations)?;
    Ok(operations)
}

fn action_output_field_manifest(
    action: &LineageActionDefinition,
) -> Result<Vec<AuthoredOutputFieldKey>, LineageBridgeError> {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "imported baseline output fields are derived from its exact payload",
            ));
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let intent = payload.parameters.get(AUTHORED_INTENT_PARAMETER).ok_or(
        LineageBridgeError::InvalidMaterialization("action has no authored semantic intent"),
    )?;
    let manifest = intent
        .as_object()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent is not an object",
        ))?
        .get(AUTHORED_OUTPUT_FIELD_MANIFEST)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent has no output field manifest",
        ))?;
    let manifest = serde_json::from_value::<Vec<AuthoredOutputFieldKey>>(manifest.clone())?;
    if manifest.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored output field manifest is not uniquely canonical",
        ));
    }
    Ok(manifest)
}

fn authored_output_fields(
    action: &LineageActionDefinition,
) -> Result<Vec<AuthoredOutputFieldValue>, LineageBridgeError> {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => return Ok(Vec::new()),
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let intent = payload.parameters.get(AUTHORED_INTENT_PARAMETER).ok_or(
        LineageBridgeError::InvalidMaterialization("action has no authored semantic intent"),
    )?;
    intent
        .as_object()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent is not an object",
        ))?
        .get("owned_output_fields")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(Into::into)
        .map(Option::unwrap_or_default)
}

fn apply_authored_output_field(
    operations: &mut [StructuralDeltaOperation],
    field: &AuthoredOutputFieldValue,
    persistent_id: &str,
) -> Result<(), LineageBridgeError> {
    if field.kind == LineageOutputKind::Activation
        && field.field.as_str() == "configuration"
        && persistent_id == "host-configuration"
    {
        let operation = operations
            .iter_mut()
            .rev()
            .find(|operation| {
                matches!(
                    operation,
                    StructuralDeltaOperation::SetField { path, .. }
                        if path.last().is_some_and(|name| name == "host_activation")
                )
            })
            .ok_or(LineageBridgeError::MissingOwner)?;
        let StructuralDeltaOperation::SetField { value, .. } = operation else {
            unreachable!("the guarded operation match is exhaustive");
        };
        *value = field.value.clone();
        return Ok(());
    }
    if field.field.as_str() == "role" && field.kind == LineageOutputKind::Curve {
        let operation = operations.iter_mut().rev().find(|operation| {
            matches!(
                operation,
                StructuralDeltaOperation::UpsertEntity { path, id, .. }
                    | StructuralDeltaOperation::RemoveEntity { path, id }
                    if path.last().is_some_and(|name| name == "geometry_roles")
                        && id == persistent_id
            )
        });
        if operation.is_none() && field.value.as_str() == Some("profile") {
            return Ok(());
        }
        let operation = operation.ok_or(LineageBridgeError::MissingOwner)?;
        return match (field.value.as_str(), operation) {
            (Some("construction"), StructuralDeltaOperation::UpsertEntity { value, .. }) => {
                value
                    .as_object_mut()
                    .ok_or(LineageBridgeError::MissingOwner)?
                    .insert("role".into(), field.value.clone());
                Ok(())
            }
            (Some("profile"), StructuralDeltaOperation::RemoveEntity { .. }) => Ok(()),
            _ => Err(LineageBridgeError::MissingOwner),
        };
    }
    if field.field.as_str() == "activation" {
        let operation = operations.iter().rev().find(|operation| {
            matches!(
                operation,
                StructuralDeltaOperation::UpsertEntity { path, value, .. }
                    if path.last().is_some_and(|name| name == "user_inactive_elements")
                        && document_element_materialization(value).is_some_and(|(kind, id)| {
                            id == persistent_id && kind == field.kind
                        })
            ) || matches!(
                operation,
                StructuralDeltaOperation::RemoveEntity { path, id }
                    if path.last().is_some_and(|name| name == "user_inactive_elements")
                        && serde_json::from_str::<Value>(id).ok().as_ref().is_some_and(|value| {
                            document_element_materialization(value).is_some_and(|(kind, id)| {
                                id == persistent_id && kind == field.kind
                            })
                        })
            )
        });
        if operation.is_none() && field.value.as_bool() == Some(true) {
            return Ok(());
        }
        let operation = operation.ok_or(LineageBridgeError::MissingOwner)?;
        return match (field.value.as_bool(), operation) {
            (Some(false), StructuralDeltaOperation::UpsertEntity { .. })
            | (Some(true), StructuralDeltaOperation::RemoveEntity { .. }) => Ok(()),
            _ => Err(LineageBridgeError::MissingOwner),
        };
    }

    let entity = operations
        .iter_mut()
        .rev()
        .find_map(|operation| {
            operation_materialized_entity_mut(operation, field.kind, persistent_id)
        })
        .ok_or(LineageBridgeError::MissingOwner)?;
    set_semantic_leaf_value(entity, field.field.as_str(), field.value.clone())
}

fn operation_materialized_entity_mut<'a>(
    operation: &'a mut StructuralDeltaOperation,
    kind: LineageOutputKind,
    persistent_id: &str,
) -> Option<&'a mut Value> {
    match operation {
        StructuralDeltaOperation::UpsertEntity {
            path, id, value, ..
        } => {
            if entity_kind_for_collection(path.last().map(String::as_str))
                .is_some_and(|(candidate, _)| candidate == kind)
                && id == persistent_id
            {
                return Some(value);
            }
            find_materialized_entity_mut(&mut path.clone(), value, kind, persistent_id)
        }
        StructuralDeltaOperation::SetField { path, value } => {
            find_materialized_entity_mut(&mut path.clone(), value, kind, persistent_id)
        }
        StructuralDeltaOperation::RemoveField { .. }
        | StructuralDeltaOperation::RemoveEntity { .. } => None,
    }
}

fn set_semantic_leaf_value(
    entity: &mut Value,
    field: &str,
    replacement: Value,
) -> Result<(), LineageBridgeError> {
    let mut current = entity;
    let mut segments = field.split('.').peekable();
    while let Some(segment) = segments.next() {
        let last = segments.peek().is_none();
        match current {
            Value::Object(object) => {
                let value = object
                    .get_mut(segment)
                    .ok_or(LineageBridgeError::MissingOwner)?;
                if last {
                    *value = replacement;
                    return Ok(());
                }
                current = value;
            }
            Value::Array(values) if last && matches!(segment, "x" | "y") => {
                let index = usize::from(segment == "y");
                let value = values
                    .get_mut(index)
                    .ok_or(LineageBridgeError::MissingOwner)?;
                *value = replacement;
                return Ok(());
            }
            Value::Array(_)
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => return Err(LineageBridgeError::MissingOwner),
        }
    }
    Err(LineageBridgeError::MissingOwner)
}

fn authored_action_delta(
    action: &LineageActionDefinition,
) -> Result<Vec<StructuralDeltaOperation>, LineageBridgeError> {
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => return Ok(Vec::new()),
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let owner_fields = payload
        .parameters
        .get(AUTHORED_OWNER_FIELDS_PARAMETER)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "action has no authored owner fields",
        ))?;
    let operations = serde_json::from_value::<Vec<StructuralDeltaOperation>>(owner_fields.clone())?;
    validate_authored_operations(&operations)?;
    Ok(operations)
}

fn replace_action_delta_targets(
    action: &mut LineageActionDefinition,
    replacements: &[(StructuralTargetKey, StructuralDeltaOperation)],
    output_field_values: &[AuthoredOutputFieldValue],
    output_raw: &BTreeMap<(LineageOutputId, LineageOutputKind), String>,
) -> Result<(), LineageBridgeError> {
    let mut compiled_operations = action_delta(action)?;
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { baseline } => {
            if output_field_values.is_empty() {
                return Err(LineageBridgeError::InvalidMaterialization(
                    "imported baseline owner rewrite has no authenticated output fields",
                ));
            }
            let mut imported: ImportedCoordinatorBaseline =
                serde_json::from_str(&baseline.payload)?;
            let mut value = imported.checkpoint.structural_value()?;
            apply_imported_output_fields(
                &mut value,
                replacements,
                output_field_values,
                output_raw,
            )?;
            imported.checkpoint.replace_structural_value(&value)?;
            baseline.payload = serde_json::to_string(&imported)?;
            return Ok(());
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let mut intent = payload
        .parameters
        .get(AUTHORED_INTENT_PARAMETER)
        .cloned()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "action has no authored semantic intent",
        ))?;
    let owner_fields = payload
        .parameters
        .get(AUTHORED_OWNER_FIELDS_PARAMETER)
        .cloned()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "action has no authored owner fields",
        ))?;
    let mut owner_fields = serde_json::from_value::<Vec<StructuralDeltaOperation>>(owner_fields)?;
    replace_structural_targets(&mut owner_fields, replacements, true)?;
    replace_structural_targets(&mut compiled_operations, replacements, true)?;
    rewrite_authored_output_fields(&mut intent, output_field_values)?;
    validate_authored_operations(&owner_fields)?;
    validate_authored_operations(&compiled_operations)?;
    let owner_fields_value = serde_json::to_value(&owner_fields)?;
    let intent_digest = authored_intent_digest(&intent, &owner_fields_value)?;
    payload
        .parameters
        .insert(AUTHORED_INTENT_PARAMETER.into(), intent);
    payload
        .parameters
        .insert(AUTHORED_OWNER_FIELDS_PARAMETER.into(), owner_fields_value);
    payload.parameters.insert(
        MATERIALIZATION_PARAMETER.into(),
        serde_json::to_value(CompiledMaterializationRecipe {
            version: 1,
            intent_digest,
            operations: compiled_operations,
        })?,
    );
    Ok(())
}

fn step_output_raw_map(
    document: &LineageDocument,
    owner: LineageStepId,
) -> Result<BTreeMap<(LineageOutputId, LineageOutputKind), String>, LineageBridgeError> {
    let index = materialized_output_index(document);
    let step = document
        .step(owner)
        .ok_or(LineageBridgeError::MissingOwner)?;
    let output_raw = step
        .outputs
        .iter()
        .filter_map(|output| {
            let reference = LineageOutputRef {
                document: document.id(),
                step: owner,
                output: output.id,
                kind: output.kind,
            };
            index
                .raw_by_ref
                .get(&reference)
                .cloned()
                .map(|raw| ((output.id, output.kind), raw))
        })
        .collect::<BTreeMap<_, _>>();
    Ok(output_raw)
}

fn apply_imported_output_fields(
    root: &mut Value,
    replacements: &[(StructuralTargetKey, StructuralDeltaOperation)],
    fields: &[AuthoredOutputFieldValue],
    output_raw: &BTreeMap<(LineageOutputId, LineageOutputKind), String>,
) -> Result<(), LineageBridgeError> {
    let mut applied_special_targets = BTreeSet::new();
    for field in fields {
        let persistent_id = output_raw
            .get(&(field.output, field.kind))
            .ok_or(LineageBridgeError::MissingOwner)?;
        if matches!(field.field.as_str(), "role" | "activation") {
            let (target, operation) = replacements
                .iter()
                .find(|(_, operation)| {
                    imported_special_operation_matches(
                        operation,
                        field.kind,
                        persistent_id,
                        field.field.as_str(),
                    )
                })
                .ok_or(LineageBridgeError::MissingOwner)?;
            if applied_special_targets.insert(target.clone()) {
                apply_operation(root, operation)?;
            }
            continue;
        }
        let entity = find_materialized_entity_mut(&mut Vec::new(), root, field.kind, persistent_id)
            .ok_or(LineageBridgeError::MissingOwner)?;
        set_semantic_leaf_value(entity, field.field.as_str(), field.value.clone())?;
    }
    Ok(())
}

fn imported_special_operation_matches(
    operation: &StructuralDeltaOperation,
    kind: LineageOutputKind,
    persistent_id: &str,
    field: &str,
) -> bool {
    match field {
        "role" if kind == LineageOutputKind::Curve => {
            geometry_role_curve(operation) == Some(persistent_id)
        }
        "activation" => match operation {
            StructuralDeltaOperation::UpsertEntity { path, value, .. }
                if path
                    .last()
                    .is_some_and(|name| name == "user_inactive_elements") =>
            {
                document_element_materialization(value).is_some_and(|(candidate_kind, id)| {
                    candidate_kind == kind && id == persistent_id
                })
            }
            StructuralDeltaOperation::RemoveEntity { path, id }
                if path
                    .last()
                    .is_some_and(|name| name == "user_inactive_elements") =>
            {
                serde_json::from_str::<Value>(id).ok().is_some_and(|value| {
                    document_element_materialization(&value).is_some_and(|(candidate_kind, id)| {
                        candidate_kind == kind && id == persistent_id
                    })
                })
            }
            StructuralDeltaOperation::SetField { .. }
            | StructuralDeltaOperation::RemoveField { .. }
            | StructuralDeltaOperation::UpsertEntity { .. }
            | StructuralDeltaOperation::RemoveEntity { .. } => false,
        },
        _ => false,
    }
}

fn refresh_compiled_delta_targets(
    action: &mut LineageActionDefinition,
    replacements: &[(StructuralTargetKey, StructuralDeltaOperation)],
) -> Result<(), LineageBridgeError> {
    if replacements.is_empty() {
        return Ok(());
    }
    let mut compiled_operations = action_delta(action)?;
    replace_structural_targets(&mut compiled_operations, replacements, false)?;
    validate_authored_operations(&compiled_operations)?;
    let payload = match action {
        LineageActionDefinition::ImportedBaseline { .. } => {
            return Err(LineageBridgeError::InvalidMaterialization(
                "imported baseline has no disposable compiled snapshot",
            ));
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    };
    let intent = payload.parameters.get(AUTHORED_INTENT_PARAMETER).ok_or(
        LineageBridgeError::InvalidMaterialization("action has no authored semantic intent"),
    )?;
    let owner_fields = payload
        .parameters
        .get(AUTHORED_OWNER_FIELDS_PARAMETER)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "action has no authored owner fields",
        ))?;
    let intent_digest = authored_intent_digest(intent, owner_fields)?;
    payload.parameters.insert(
        MATERIALIZATION_PARAMETER.into(),
        serde_json::to_value(CompiledMaterializationRecipe {
            version: 1,
            intent_digest,
            operations: compiled_operations,
        })?,
    );
    Ok(())
}

fn replace_structural_targets(
    operations: &mut Vec<StructuralDeltaOperation>,
    replacements: &[(StructuralTargetKey, StructuralDeltaOperation)],
    allow_missing: bool,
) -> Result<(), LineageBridgeError> {
    for (target, replacement) in replacements {
        if let Some(operation) = operations
            .iter_mut()
            .rev()
            .find(|operation| operation.target_key() == *target)
        {
            *operation = preserve_upsert_anchor(operation, replacement.clone());
        } else if allow_missing {
            // Side-table associations such as a curve's non-default geometry
            // role are writable leaves of an already owning recipe. Their
            // first non-default value has no earlier structural operation.
            operations.push(replacement.clone());
        } else {
            return Err(LineageBridgeError::MissingOwner);
        }
    }
    Ok(())
}

fn rewrite_authored_output_fields(
    intent: &mut Value,
    replacements: &[AuthoredOutputFieldValue],
) -> Result<(), LineageBridgeError> {
    if replacements.is_empty() {
        return Ok(());
    }
    let object = intent
        .as_object_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "authored semantic intent is not an object",
        ))?;
    let existing = object
        .remove("owned_output_fields")
        .map(serde_json::from_value::<Vec<AuthoredOutputFieldValue>>)
        .transpose()?
        .unwrap_or_default();
    let mut rewritten = BTreeMap::<
        (LineageOutputId, LineageOutputKind, LineageSemanticKey),
        AuthoredOutputFieldValue,
    >::new();
    for rewrite in existing.iter().chain(replacements) {
        rewritten.insert(
            (rewrite.output, rewrite.kind, rewrite.field.clone()),
            rewrite.clone(),
        );
    }
    object.insert(
        "owned_output_fields".into(),
        serde_json::to_value(rewritten.into_values().collect::<Vec<_>>())?,
    );
    Ok(())
}

fn preserve_upsert_anchor(
    operation: &StructuralDeltaOperation,
    replacement: StructuralDeltaOperation,
) -> StructuralDeltaOperation {
    match (operation, replacement) {
        (
            StructuralDeltaOperation::UpsertEntity { before, .. },
            StructuralDeltaOperation::UpsertEntity {
                path, id, value, ..
            },
        ) => StructuralDeltaOperation::UpsertEntity {
            path,
            id,
            before: before.clone(),
            value,
        },
        (_, replacement) => replacement,
    }
}

fn validate_authored_operations(
    operations: &[StructuralDeltaOperation],
) -> Result<(), LineageBridgeError> {
    if operations.len() > MAX_MATERIALIZATION_PATCH_OPERATIONS {
        return Err(LineageBridgeError::PatchResourceLimit);
    }
    for operation in operations {
        match operation {
            StructuralDeltaOperation::SetField { path, value }
            | StructuralDeltaOperation::UpsertEntity { path, value, .. } => {
                validate_authored_value(path, value)?;
            }
            StructuralDeltaOperation::RemoveField { path }
            | StructuralDeltaOperation::RemoveEntity { path, .. } => {
                reject_disposable_authored_path(path)?;
            }
        }
    }
    Ok(())
}

fn validate_authored_value(path: &[String], value: &Value) -> Result<(), LineageBridgeError> {
    reject_disposable_authored_path(path)?;
    match value {
        Value::Array(values) => {
            for value in values {
                validate_authored_value(path, value)?;
            }
        }
        Value::Object(object) => {
            for (field, value) in object {
                let mut nested = path.to_vec();
                nested.push(field.clone());
                validate_authored_value(&nested, value)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn reject_disposable_authored_path(path: &[String]) -> Result<(), LineageBridgeError> {
    let lifecycle = path.first().is_some_and(|field| field == "lifecycle");
    let sketch_allocator = matches!(
        path,
        [design, document, next]
            if design == "design" && document == "document" && next == "next_id"
    ) || matches!(
        path,
        [design, document, nested, next]
            if design == "design"
                && document == "document"
                && nested == "document"
                && next == "next_id"
    ) || (path.first().is_some_and(|field| field == "design")
        && path.last().is_some_and(|field| field == "next_span_id"));
    let feature_lifecycle = matches!(
        path,
        [features, field]
            if features == "features"
                && matches!(
                    field.as_str(),
                    "revision" | "next_feature_id" | "next_corner_id" | "digest"
                )
    );
    if lifecycle || sketch_allocator || feature_lifecycle {
        return Err(LineageBridgeError::InvalidMaterialization(
            "authored action contains disposable lifecycle metadata",
        ));
    }
    Ok(())
}

fn structural_delta(
    before: &Value,
    after: &Value,
) -> Result<Vec<StructuralDeltaOperation>, LineageBridgeError> {
    let mut operations = Vec::new();
    diff_value(&mut Vec::new(), before, after, &mut operations)?;
    if operations.len() > MAX_MATERIALIZATION_PATCH_OPERATIONS {
        return Err(LineageBridgeError::PatchResourceLimit);
    }
    Ok(operations)
}

fn diff_value(
    path: &mut Vec<String>,
    before: &Value,
    after: &Value,
    operations: &mut Vec<StructuralDeltaOperation>,
) -> Result<(), LineageBridgeError> {
    if before == after {
        return Ok(());
    }
    if path.len() > MAX_MATERIALIZATION_PATH_DEPTH {
        return Err(LineageBridgeError::InvalidMaterialization(
            "materialization path is too deep",
        ));
    }
    if path.last().is_some_and(|field| field == "host_activation") {
        operations.push(StructuralDeltaOperation::SetField {
            path: path.clone(),
            value: after.clone(),
        });
        return Ok(());
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            let keys = before
                .keys()
                .chain(after.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            for key in keys {
                path.push(key.clone());
                match (before.get(&key), after.get(&key)) {
                    (Some(before), Some(after)) => {
                        diff_value(path, before, after, operations)?;
                    }
                    (None, Some(value)) => operations.push(StructuralDeltaOperation::SetField {
                        path: path.clone(),
                        value: value.clone(),
                    }),
                    (Some(_), None) => {
                        operations
                            .push(StructuralDeltaOperation::RemoveField { path: path.clone() });
                    }
                    (None, None) => unreachable!("key came from one object"),
                }
                path.pop();
            }
        }
        (Value::Array(before), Value::Array(after))
            if id_index(path, before).is_some() && id_index(path, after).is_some() =>
        {
            diff_entity_array(path, before, after, operations)?;
        }
        _ => operations.push(StructuralDeltaOperation::SetField {
            path: path.clone(),
            value: after.clone(),
        }),
    }
    Ok(())
}

fn diff_entity_array(
    path: &[String],
    before: &[Value],
    after: &[Value],
    operations: &mut Vec<StructuralDeltaOperation>,
) -> Result<(), LineageBridgeError> {
    let before_by_id = id_index(path, before).ok_or(LineageBridgeError::InvalidMaterialization(
        "entity array has missing or duplicate IDs",
    ))?;
    let after_by_id = id_index(path, after).ok_or(LineageBridgeError::InvalidMaterialization(
        "entity array has missing or duplicate IDs",
    ))?;
    for id in before_by_id.keys() {
        if !after_by_id.contains_key(id) {
            operations.push(StructuralDeltaOperation::RemoveEntity {
                path: path.to_vec(),
                id: id.clone(),
            });
        }
    }
    for (index, value) in after.iter().enumerate().rev() {
        let id = entity_id_at_path(path, value).expect("validated entity array");
        if before_by_id.get(&id).is_some_and(|before| *before == value) {
            continue;
        }
        let before_id = after.get(index + 1).and_then(entity_id).map(str::to_owned);
        operations.push(StructuralDeltaOperation::UpsertEntity {
            path: path.to_vec(),
            id,
            before: before_id,
            value: value.clone(),
        });
    }
    Ok(())
}

fn id_index<'a>(path: &[String], values: &'a [Value]) -> Option<BTreeMap<String, &'a Value>> {
    if values.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut indexed = BTreeMap::new();
    for value in values {
        let id = entity_id_at_path(path, value)?;
        if indexed.insert(id, value).is_some() {
            return None;
        }
    }
    Some(indexed)
}

fn entity_id(value: &Value) -> Option<&str> {
    value.as_object()?.get("id")?.as_str()
}

fn entity_id_at_path(path: &[String], value: &Value) -> Option<String> {
    if let Some(id) = entity_id(value) {
        return Some(id.to_owned());
    }
    match path.last().map(String::as_str)? {
        "geometry_roles" => value.as_object()?.get("curve")?.as_str().map(str::to_owned),
        "parameter_bindings" | "parameter_outputs" | "user_inactive_elements" => {
            serde_json::to_string(value).ok()
        }
        _ => None,
    }
}

fn geometry_role_curve(operation: &StructuralDeltaOperation) -> Option<&str> {
    match operation {
        StructuralDeltaOperation::UpsertEntity { path, value, .. }
            if path.last().is_some_and(|field| field == "geometry_roles") =>
        {
            value.as_object()?.get("curve")?.as_str()
        }
        StructuralDeltaOperation::RemoveEntity { path, id }
            if path.last().is_some_and(|field| field == "geometry_roles") =>
        {
            Some(id)
        }
        StructuralDeltaOperation::SetField { .. }
        | StructuralDeltaOperation::RemoveField { .. }
        | StructuralDeltaOperation::UpsertEntity { .. }
        | StructuralDeltaOperation::RemoveEntity { .. } => None,
    }
}

fn action_owns_structural_target(
    action: &LineageActionDefinition,
    change: &StructuralDeltaOperation,
) -> Result<bool, LineageBridgeError> {
    if matches!(action, LineageActionDefinition::ImportedBaseline { .. }) {
        return Ok(true);
    }
    let target = change.target_key();
    Ok(authored_action_delta(action)?
        .iter()
        .any(|operation| operation.target_key() == target)
        || geometry_role_curve(change).is_some())
}

fn direct_materialized_leaves_for_change(
    before: &Value,
    after: &Value,
    change: &StructuralDeltaOperation,
) -> Result<Vec<LineageMaterializedLeaf>, LineageBridgeError> {
    let mut leaves = BTreeSet::new();
    match change {
        StructuralDeltaOperation::UpsertEntity {
            path, id, value, ..
        } if path.last().is_some_and(|field| field == "geometry_roles") => {
            let curve = value
                .as_object()
                .and_then(|object| object.get("curve"))
                .and_then(Value::as_str)
                .ok_or(LineageBridgeError::MissingOwner)?;
            leaves.insert(materialized_leaf(LineageOutputKind::Curve, curve, "role")?);
            if id != curve {
                return Err(LineageBridgeError::MissingOwner);
            }
        }
        StructuralDeltaOperation::RemoveEntity { path, id }
            if path.last().is_some_and(|field| field == "geometry_roles") =>
        {
            leaves.insert(materialized_leaf(LineageOutputKind::Curve, id, "role")?);
        }
        StructuralDeltaOperation::UpsertEntity {
            path, id, value, ..
        } if path
            .last()
            .is_some_and(|field| field == "user_inactive_elements") =>
        {
            let (kind, persistent_id) =
                document_element_materialization(value).ok_or(LineageBridgeError::MissingOwner)?;
            if entity_id_at_path(path, value).as_deref() != Some(id) {
                return Err(LineageBridgeError::MissingOwner);
            }
            leaves.insert(materialized_leaf(kind, persistent_id, "activation")?);
        }
        StructuralDeltaOperation::RemoveEntity { path, id }
            if path
                .last()
                .is_some_and(|field| field == "user_inactive_elements") =>
        {
            let value = serde_json::from_str::<Value>(id)?;
            let (kind, persistent_id) =
                document_element_materialization(&value).ok_or(LineageBridgeError::MissingOwner)?;
            leaves.insert(materialized_leaf(kind, persistent_id, "activation")?);
        }
        StructuralDeltaOperation::UpsertEntity { path, id, .. }
            if entity_kind_for_collection(path.last().map(String::as_str)).is_some() =>
        {
            let (kind, _) = entity_kind_for_collection(path.last().map(String::as_str))
                .ok_or(LineageBridgeError::MissingOwner)?;
            let before_entity =
                entity_value_at_path(before, path, id).ok_or(LineageBridgeError::MissingOwner)?;
            let after_entity =
                entity_value_at_path(after, path, id).ok_or(LineageBridgeError::MissingOwner)?;
            collect_changed_entity_leaves(kind, id, before_entity, after_entity, &mut leaves)?;
        }
        StructuralDeltaOperation::SetField { .. }
        | StructuralDeltaOperation::RemoveField { .. }
        | StructuralDeltaOperation::UpsertEntity { .. }
        | StructuralDeltaOperation::RemoveEntity { .. } => {}
    }
    Ok(leaves.into_iter().collect())
}

fn collect_changed_entity_leaves(
    kind: LineageOutputKind,
    id: &str,
    before: &Value,
    after: &Value,
    changed: &mut BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    let mut before_leaves = BTreeMap::new();
    collect_entity_leaf_values(&mut Vec::new(), before, &mut before_leaves)?;
    let mut after_leaves = BTreeMap::new();
    collect_entity_leaf_values(&mut Vec::new(), after, &mut after_leaves)?;
    for key in before_leaves
        .keys()
        .chain(after_leaves.keys())
        .collect::<BTreeSet<_>>()
    {
        if before_leaves.get(key) != after_leaves.get(key) {
            changed.insert(materialized_leaf(kind, id, key.as_str())?);
        }
    }

    collect_changed_nested_entity_leaves(before, after, changed)
}

/// Finds persistent child collections at any depth inside one parent entity.
/// Computed-feature corners, for example, live below `definition.corners`
/// rather than directly on the feature object. Their writable fields remain
/// independently owned and must participate in the same atomic owner rewrite.
fn collect_changed_nested_entity_leaves(
    before: &Value,
    after: &Value,
    changed: &mut BTreeSet<LineageMaterializedLeaf>,
) -> Result<(), LineageBridgeError> {
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            for field in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
                let Some(before_value) = before.get(field) else {
                    continue;
                };
                let Some(after_value) = after.get(field) else {
                    continue;
                };
                if let Some((child_kind, _)) = entity_kind_for_collection(Some(field)) {
                    let before_values = before_value
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let after_values = after_value
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let path = [field.clone()];
                    let before_index =
                        id_index(&path, before_values).ok_or(LineageBridgeError::MissingOwner)?;
                    let after_index =
                        id_index(&path, after_values).ok_or(LineageBridgeError::MissingOwner)?;
                    for child in before_index
                        .keys()
                        .filter(|child| after_index.contains_key(*child))
                    {
                        collect_changed_entity_leaves(
                            child_kind,
                            child,
                            before_index
                                .get(child)
                                .ok_or(LineageBridgeError::MissingOwner)?,
                            after_index
                                .get(child)
                                .ok_or(LineageBridgeError::MissingOwner)?,
                            changed,
                        )?;
                    }
                } else {
                    collect_changed_nested_entity_leaves(before_value, after_value, changed)?;
                }
            }
            Ok(())
        }
        (Value::Array(before), Value::Array(after)) => {
            for (before, after) in before.iter().zip(after) {
                collect_changed_nested_entity_leaves(before, after, changed)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn entity_value_at_path<'a>(root: &'a Value, path: &[String], id: &str) -> Option<&'a Value> {
    value_at_path(root, path)?
        .as_array()?
        .iter()
        .find(|value| entity_id_at_path(path, value).as_deref() == Some(id))
}

fn materialized_leaf(
    kind: LineageOutputKind,
    persistent_id: &str,
    key: &str,
) -> Result<LineageMaterializedLeaf, LineageBridgeError> {
    let reservation_kind =
        reservation_kind_for_output(kind).ok_or(LineageBridgeError::InvalidMaterialization(
            "logical-only output cannot own a materialized writable leaf",
        ))?;
    Ok(LineageMaterializedLeaf {
        materialized: LineageMaterializedIdentity {
            kind: reservation_kind,
            persistent_id: LineageOpaqueId::new(format!(
                "{}:{persistent_id}",
                output_kind_key(kind)
            ))?,
        },
        key: LineageSemanticKey::new(key)?,
    })
}

fn materialized_leaf_value(
    root: &Value,
    leaf: &LineageMaterializedLeaf,
) -> Result<Value, LineageBridgeError> {
    let (prefix, persistent_id) = leaf
        .materialized
        .persistent_id
        .as_str()
        .split_once(':')
        .ok_or(LineageBridgeError::MissingOwner)?;
    if materialized_identity_prefix(leaf.materialized.kind) != prefix {
        return Err(LineageBridgeError::MissingOwner);
    }
    match leaf.key.as_str() {
        "role" if leaf.materialized.kind == LineageReservationKind::Curve => {
            let role = find_collection_entry(root, "geometry_roles", |value| {
                value
                    .as_object()
                    .and_then(|object| object.get("curve"))
                    .and_then(Value::as_str)
                    == Some(persistent_id)
            })
            .and_then(Value::as_object)
            .and_then(|object| object.get("role"))
            .cloned()
            .unwrap_or_else(|| Value::String("profile".into()));
            Ok(role)
        }
        "activation" => {
            let inactive = find_collection_entry(root, "user_inactive_elements", |value| {
                document_element_materialization(value).is_some_and(|(kind, id)| {
                    id == persistent_id
                        && reservation_kind_for_output(kind) == Some(leaf.materialized.kind)
                })
            })
            .is_some();
            Ok(Value::Bool(!inactive))
        }
        key => {
            let entity = find_materialized_entity(
                &mut Vec::new(),
                root,
                leaf.materialized.kind,
                persistent_id,
            )
            .ok_or(LineageBridgeError::MissingOwner)?;
            let mut values = BTreeMap::new();
            collect_entity_leaf_values(&mut Vec::new(), entity, &mut values)?;
            values
                .get(&LineageSemanticKey::new(key)?)
                .cloned()
                .ok_or(LineageBridgeError::MissingOwner)
        }
    }
}

fn find_materialized_entity<'a>(
    path: &mut Vec<String>,
    value: &'a Value,
    kind: LineageReservationKind,
    persistent_id: &str,
) -> Option<&'a Value> {
    match value {
        Value::Array(values) => {
            if entity_kind_for_collection(path.last().map(String::as_str))
                .is_some_and(|(_, candidate_kind)| candidate_kind == kind)
                && let Some(entity) = values.iter().find(|entity| {
                    entity_id_at_path(path, entity).as_deref() == Some(persistent_id)
                })
            {
                return Some(entity);
            }
            values
                .iter()
                .find_map(|value| find_materialized_entity(path, value, kind, persistent_id))
        }
        Value::Object(object) => object.iter().find_map(|(field, value)| {
            path.push(field.clone());
            let found = find_materialized_entity(path, value, kind, persistent_id);
            path.pop();
            found
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn find_materialized_entity_mut<'a>(
    path: &mut Vec<String>,
    value: &'a mut Value,
    kind: LineageOutputKind,
    persistent_id: &str,
) -> Option<&'a mut Value> {
    match value {
        Value::Array(values) => {
            if entity_kind_for_collection(path.last().map(String::as_str))
                .is_some_and(|(candidate_kind, _)| candidate_kind == kind)
                && let Some(index) = values.iter().position(|entity| {
                    entity_id_at_path(path, entity).as_deref() == Some(persistent_id)
                })
            {
                return values.get_mut(index);
            }
            for value in values {
                if let Some(found) = find_materialized_entity_mut(path, value, kind, persistent_id)
                {
                    return Some(found);
                }
            }
            None
        }
        Value::Object(object) => {
            for (field, value) in object {
                path.push(field.clone());
                let found = find_materialized_entity_mut(path, value, kind, persistent_id);
                path.pop();
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn find_collection_entry<'a>(
    value: &'a Value,
    collection: &str,
    predicate: impl Copy + Fn(&Value) -> bool,
) -> Option<&'a Value> {
    match value {
        Value::Array(values) => values
            .iter()
            .find_map(|value| find_collection_entry(value, collection, predicate)),
        Value::Object(object) => {
            if let Some(found) = object
                .get(collection)
                .and_then(Value::as_array)
                .and_then(|values| values.iter().find(|value| predicate(value)))
            {
                return Some(found);
            }
            object
                .values()
                .find_map(|value| find_collection_entry(value, collection, predicate))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

const fn materialized_identity_prefix(kind: LineageReservationKind) -> &'static str {
    match kind {
        LineageReservationKind::Point => "point",
        LineageReservationKind::Scalar => "scalar",
        LineageReservationKind::Curve => "curve",
        LineageReservationKind::CurveSpan => "curve-span",
        LineageReservationKind::TrimView => "trim-view",
        LineageReservationKind::Contact => "contact",
        LineageReservationKind::Constraint => "constraint",
        LineageReservationKind::Dimension => "dimension",
        LineageReservationKind::Source => "source",
        LineageReservationKind::Parameter => "parameter",
        LineageReservationKind::ParameterBinding => "parameter-binding",
        LineageReservationKind::ParameterOutput => "parameter-output",
        LineageReservationKind::ExternalBinding => "external-binding",
        LineageReservationKind::Profile => "profile",
        LineageReservationKind::Chain => "chain",
        LineageReservationKind::Operation => "operation",
        LineageReservationKind::Feature => "feature",
        LineageReservationKind::FeatureCorner => "feature-corner",
        LineageReservationKind::Annotation => "annotation",
        LineageReservationKind::HiddenSupport => "hidden-support",
    }
}

fn document_element_materialization(value: &Value) -> Option<(LineageOutputKind, &str)> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?;
    let kind = match object.get("kind")?.as_str()? {
        "point" => LineageOutputKind::Point,
        "scalar" => LineageOutputKind::Scalar,
        "curve" => LineageOutputKind::Curve,
        "contact" => LineageOutputKind::Contact,
        "constraint" => LineageOutputKind::Constraint,
        "dimension" => LineageOutputKind::Dimension,
        "parameter" => LineageOutputKind::Parameter,
        "external_binding" => LineageOutputKind::ExternalBinding,
        "source" => LineageOutputKind::Source,
        _ => return None,
    };
    Some((kind, id))
}

fn apply_authored_operations(
    root: &mut Value,
    operations: &[StructuralDeltaOperation],
) -> Result<(), LineageBridgeError> {
    validate_authored_operations(operations)?;
    let next_id = sketch_next_id(root)?.clone();
    let sketch_identity_high_water = root
        .get("lifecycle")
        .and_then(Value::as_object)
        .and_then(|lifecycle| lifecycle.get("sketch_identity_high_water"))
        .cloned()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization sketch identity high-water is missing",
        ))?;
    for operation in operations {
        apply_operation(root, operation)?;
    }
    restore_sketch_next_id(root, next_id)?;
    restore_spline_next_span_ids(root, &sketch_identity_high_water)?;
    refresh_computed_feature_digest(root)
}

fn restore_spline_next_span_ids(
    root: &mut Value,
    sketch_identity_high_water: &Value,
) -> Result<(), LineageBridgeError> {
    let cursors = sketch_identity_high_water
        .as_object()
        .and_then(|high_water| high_water.get("spline_span_cursors"))
        .and_then(Value::as_object)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization spline span high-water is invalid",
        ))?;
    let design = root
        .get_mut("design")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root is missing design state",
        ))?;
    let document = design
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization design document is invalid",
        ))?
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "normalized materialization document is invalid",
        ))?;
    let curves = document
        .get_mut("curves")
        .and_then(Value::as_array_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization sketch curves are invalid",
        ))?;
    for curve in curves {
        let Some(curve) = curve.as_object_mut() else {
            return Err(LineageBridgeError::InvalidMaterialization(
                "materialization sketch curve is invalid",
            ));
        };
        let Some(id) = curve.get("id").and_then(Value::as_str) else {
            return Err(LineageBridgeError::InvalidMaterialization(
                "materialization sketch curve identity is invalid",
            ));
        };
        let Some(cursor) = cursors.get(id) else {
            continue;
        };
        let definition = curve
            .get_mut("definition")
            .and_then(Value::as_object_mut)
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization sketch curve definition is invalid",
            ))?;
        if definition.contains_key("span_ids") {
            definition.insert("next_span_id".into(), cursor.clone());
        }
    }
    Ok(())
}

/// Re-authenticates the computed-feature sidecar after semantic authored
/// operations have been applied over baseline-owned lifecycle state.
///
/// Computed-feature V1 deliberately includes its revision and allocator
/// cursors in the authenticated payload. Those values are retained only by the
/// imported baseline, so a replayed semantic feature delta must derive a fresh
/// digest rather than retaining the forward checkpoint's stale digest.
fn refresh_computed_feature_digest(root: &mut Value) -> Result<(), LineageBridgeError> {
    #[derive(Serialize)]
    #[serde(deny_unknown_fields)]
    struct ComputedFeaturePayload<'a> {
        document_id: geosolve_sketch_features::ComputedFeatureDocumentId,
        sketch_document: geosolve_sketch::DocumentId,
        revision: geosolve_sketch_features::ComputedFeatureRevision,
        next_feature_id: geosolve_sketch_features::ComputedFeatureId,
        next_corner_id: geosolve_sketch_features::ComputedFeatureCornerId,
        features: &'a [geosolve_sketch_features::ComputedFeature],
    }

    let features = root
        .get_mut("features")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization feature document is invalid",
        ))?;
    let document_id = serde_json::from_value(features.get("document_id").cloned().ok_or(
        LineageBridgeError::InvalidMaterialization(
            "materialization feature document identity is missing",
        ),
    )?)?;
    let sketch_document = serde_json::from_value(features.get("sketch_document").cloned().ok_or(
        LineageBridgeError::InvalidMaterialization(
            "materialization feature sketch identity is missing",
        ),
    )?)?;
    let revision = serde_json::from_value(features.get("revision").cloned().ok_or(
        LineageBridgeError::InvalidMaterialization("materialization feature revision is missing"),
    )?)?;
    let next_feature_id = serde_json::from_value(features.get("next_feature_id").cloned().ok_or(
        LineageBridgeError::InvalidMaterialization("materialization feature allocator is missing"),
    )?)?;
    let next_corner_id = serde_json::from_value(features.get("next_corner_id").cloned().ok_or(
        LineageBridgeError::InvalidMaterialization("materialization feature allocator is missing"),
    )?)?;
    let feature_values = serde_json::from_value::<Vec<geosolve_sketch_features::ComputedFeature>>(
        features
            .get("features")
            .cloned()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization computed features are missing",
            ))?,
    )?;
    let payload = ComputedFeaturePayload {
        document_id,
        sketch_document,
        revision,
        next_feature_id,
        next_corner_id,
        features: &feature_values,
    };
    let digest = geosolve_sketch_features::ComputedFeatureDocumentDigest::from_bytes(
        lineage_content_digest(&serde_json::to_vec(&payload)?).bytes(),
    );
    features.insert("digest".into(), serde_json::to_value(digest)?);
    Ok(())
}

fn sketch_next_id(root: &Value) -> Result<&Value, LineageBridgeError> {
    let design = root.get("design").and_then(Value::as_object).ok_or(
        LineageBridgeError::InvalidMaterialization("materialization root is missing design state"),
    )?;
    let document = design.get("document").and_then(Value::as_object).ok_or(
        LineageBridgeError::InvalidMaterialization("materialization design document is invalid"),
    )?;
    document
        .get("document")
        .and_then(Value::as_object)
        .and_then(|document| document.get("next_id"))
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization sketch allocator is missing",
        ))
}

fn restore_sketch_next_id(root: &mut Value, next_id: Value) -> Result<(), LineageBridgeError> {
    let design = root
        .get_mut("design")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization root is missing design state",
        ))?;
    let document = design
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization design document is invalid",
        ))?;
    document.remove("next_id");
    document
        .get_mut("document")
        .and_then(Value::as_object_mut)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "normalized materialization document is invalid",
        ))?
        .insert("next_id".into(), next_id);
    Ok(())
}

fn apply_operation(
    root: &mut Value,
    operation: &StructuralDeltaOperation,
) -> Result<(), LineageBridgeError> {
    match operation {
        StructuralDeltaOperation::SetField { path, value } => set_field(root, path, value.clone()),
        StructuralDeltaOperation::RemoveField { path } => remove_field(root, path),
        StructuralDeltaOperation::UpsertEntity {
            path,
            id,
            before,
            value,
        } => upsert_entity(root, path, id, before.as_deref(), value.clone()),
        StructuralDeltaOperation::RemoveEntity { path, id } => remove_entity(root, path, id),
    }
}

fn parent_object_mut<'a>(
    root: &'a mut Value,
    path: &[String],
) -> Result<(&'a mut Map<String, Value>, String), LineageBridgeError> {
    let (field, parents) = path
        .split_last()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization cannot replace its root",
        ))?;
    let mut value = root;
    for segment in parents {
        value = value
            .as_object_mut()
            .and_then(|object| object.get_mut(segment))
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization path does not exist",
            ))?;
    }
    Ok((
        value
            .as_object_mut()
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization path parent is not an object",
            ))?,
        field.clone(),
    ))
}

fn set_field(root: &mut Value, path: &[String], value: Value) -> Result<(), LineageBridgeError> {
    let (object, field) = parent_object_mut(root, path)?;
    object.insert(field, value);
    Ok(())
}

fn remove_field(root: &mut Value, path: &[String]) -> Result<(), LineageBridgeError> {
    let (object, field) = parent_object_mut(root, path)?;
    object
        .remove(&field)
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization removed an absent field",
        ))?;
    Ok(())
}

fn array_mut<'a>(
    root: &'a mut Value,
    path: &[String],
) -> Result<&'a mut Vec<Value>, LineageBridgeError> {
    let mut value = root;
    for segment in path {
        value = value
            .as_object_mut()
            .and_then(|object| object.get_mut(segment))
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "materialization entity-array path does not exist",
            ))?;
    }
    value
        .as_array_mut()
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization entity-array target is not an array",
        ))
}

fn upsert_entity(
    root: &mut Value,
    path: &[String],
    id: &str,
    before: Option<&str>,
    value: Value,
) -> Result<(), LineageBridgeError> {
    if entity_id_at_path(path, &value).as_deref() != Some(id) {
        return Err(LineageBridgeError::InvalidMaterialization(
            "upsert entity identity does not match its value",
        ));
    }
    let array = array_mut(root, path)?;
    if let Some(index) = array
        .iter()
        .position(|candidate| entity_id_at_path(path, candidate).as_deref() == Some(id))
    {
        array[index] = value;
        return Ok(());
    }
    let index = before.map_or(Ok(array.len()), |before| {
        array
            .iter()
            .position(|candidate| entity_id_at_path(path, candidate).as_deref() == Some(before))
            .ok_or(LineageBridgeError::InvalidMaterialization(
                "upsert successor entity does not exist",
            ))
    })?;
    array.insert(index, value);
    Ok(())
}

fn remove_entity(root: &mut Value, path: &[String], id: &str) -> Result<(), LineageBridgeError> {
    let array = array_mut(root, path)?;
    let index = array
        .iter()
        .position(|candidate| entity_id_at_path(path, candidate).as_deref() == Some(id))
        .ok_or(LineageBridgeError::InvalidMaterialization(
            "materialization removed an absent entity",
        ))?;
    array.remove(index);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentEdit,
        DocumentSolveRequest, ExternalSnapshotSet, GeometryRole, OperationControl,
        OperationOutcome, ParameterBatch, RetainedSketchDocumentSession, SketchDocument,
        SolverConfig,
    };
    use geosolve_sketch_features::{
        ComputedFeatureCornerId, ComputedFeatureId, ComputedFeatureRevision,
    };
    use geosolve_sketch_lineage::{
        LineageActionDefinition, LineageDeveloperKey, LineageDocument, LineageDocumentId,
        LineageEvaluationDisposition, LineageMaterializationMap, LineageMutation, LineageOpaqueId,
        LineageOutput, LineageOutputId, LineageOutputIdentity, LineageOutputIdentityFlow,
        LineageOutputKind, LineageOutputRef, LineagePatch, LineageReservation,
        LineageReservationId, LineageReservationKind, LineageSemanticKey, LineageSession,
        LineageStep, LineageStepId, LineageStepRewrite, LineageWritableLeaf,
        VersionedActionPayload,
    };
    use geosolve_sketch_ops::{
        LineEndpoint, SketchOperationRequest, SketchOperationResult, SketchOperationSnapshot,
    };
    use serde_json::{Value, json};

    use super::{
        AUTHORED_INTENT_PARAMETER, AUTHORED_OWNER_FIELDS_PARAMETER, AuthoredOutputFieldValue,
        CompiledMaterializationRecipe, CoordinatorLineage, LineageBridgeError, LineageCheckpoint,
        MATERIALIZATION_PARAMETER, StructuralDeltaOperation, action_delta_for_step,
        apply_operation, authored_intent_digest, authored_structural_value,
        direct_materialized_leaves_for_change, materialized_leaf, materialized_output_index,
        structural_change_transitions_identity, structural_delta,
    };
    use crate::{
        ConstructionPoint, ConstructionProposal, EditorEffect, RetainedEditorCoordinator,
        SelectionItem,
    };

    fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted retained session");
        RetainedEditorCoordinator::new(session).expect("coordinator")
    }

    #[test]
    fn maximum_bounded_generic_persistent_identity_cannot_panic_output_indexing() {
        let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x83_018));
        let step = LineageStep::new(
            LineageStepId::from_raw(1),
            LineageDeveloperKey::new("bounded-generic-output").expect("developer key"),
            "bounded generic output",
            LineageActionDefinition::GeometryRecipe {
                action: VersionedActionPayload::empty(
                    LineageSemanticKey::new("geosolve.geometry.v1.sketch-point").expect("schema"),
                    1,
                ),
            },
            vec![LineageOutput {
                id: LineageOutputId::from_raw(1),
                key: LineageSemanticKey::new("point").expect("output key"),
                kind: LineageOutputKind::Point,
                reservation: Some(LineageReservationId::from_raw(1)),
            }],
            vec![LineageReservation {
                id: LineageReservationId::from_raw(1),
                key: LineageSemanticKey::new("point").expect("reservation key"),
                kind: LineageReservationKind::Point,
                persistent_id: LineageOpaqueId::new(format!("x:{}", "a".repeat(510)))
                    .expect("maximum bounded persistent identity"),
            }],
        );
        document
            .apply_patch(LineagePatch::new(
                document.identity(),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(step),
                }],
            ))
            .expect("structurally valid generic action");

        let index = materialized_output_index(&document);
        assert!(index.by_materialized.is_empty());
        assert!(index.by_raw.is_empty());
    }

    fn coordinator_with_extended_line() -> (
        RetainedEditorCoordinator,
        geosolve_sketch::DesignPointId,
        geosolve_sketch_lineage::LineageStepId,
        geosolve_sketch_lineage::LineageStepId,
    ) {
        let mut document = SketchDocument::new(1.0).expect("document");
        let source_start = document
            .add_point("source start", [0.0, 0.0])
            .expect("source start");
        let source_end = document
            .add_point("source end", [1.0, 0.0])
            .expect("source end");
        let source = document
            .add_curve(
                "source",
                CurveDefinition::Line {
                    start: source_start,
                    end: source_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("source line");
        let target_start = document
            .add_point("target start", [2.0, -1.0])
            .expect("target start");
        let target_end = document
            .add_point("target end", [2.0, 1.0])
            .expect("target end");
        let target = document
            .add_curve(
                "target",
                CurveDefinition::Line {
                    start: target_start,
                    end: target_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .expect("target line");
        let mut coordinator = coordinator(document);
        let prepared = SketchOperationSnapshot::capture(coordinator.session())
            .prepare(SketchOperationRequest::ExtendLineToLine {
                line: CurveSpan::line(source),
                endpoint: LineEndpoint::End,
                target: CurveSpan::line(target),
            })
            .execute(OperationControl::unlimited())
            .expect("extend preparation");
        let OperationOutcome::Completed { value, .. } = prepared else {
            panic!("unbounded Extend preparation must complete");
        };
        let SketchOperationResult::Proposed(proposal) = value else {
            panic!("Extend proposal expected");
        };
        coordinator
            .apply_sketch_operation(&proposal)
            .expect("accepted Extend lineage action");
        let baseline_owner = coordinator.lineage_document().steps()[0].id;
        let operation_owner = coordinator.lineage_document().steps()[1].id;
        (coordinator, source_end, baseline_owner, operation_owner)
    }

    fn without_writable_leaf(
        lineage: &CoordinatorLineage,
        owner: geosolve_sketch_lineage::LineageStepId,
        key: &str,
    ) -> CoordinatorLineage {
        let source = lineage.session.document();
        let mut document = geosolve_sketch_lineage::LineageDocument::with_id(source.id());
        for source_step in source.steps() {
            let mut step = source_step.clone();
            if step.id == owner {
                step.writable_leaves.retain(|leaf| leaf.key.as_str() != key);
            }
            document
                .apply_patch(LineagePatch::new(
                    document.identity(),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(step),
                    }],
                ))
                .expect("rebuild valid test lineage");
        }
        let mut altered = lineage.clone();
        altered.session = LineageSession::new(document).expect("lineage without one writable leaf");
        altered
    }

    fn session_with_forged_step(
        lineage: &CoordinatorLineage,
        owner: LineageStepId,
        mut forge: impl FnMut(&mut LineageStep),
    ) -> String {
        let source = lineage.session.document();
        let mut document = geosolve_sketch_lineage::LineageDocument::with_id(source.id());
        for source_step in source.steps() {
            let mut step = source_step.clone();
            if step.id == owner {
                forge(&mut step);
            }
            document
                .apply_patch(LineagePatch::new(
                    document.identity(),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(step),
                    }],
                ))
                .expect("forged step remains structurally valid");
        }
        LineageSession::new(document)
            .expect("digest-valid hostile session")
            .to_canonical_session_json()
            .expect("canonical hostile session")
    }

    fn ordinary_payload_mut(step: &mut LineageStep) -> &mut VersionedActionPayload {
        match &mut step.action {
            LineageActionDefinition::GeometryRecipe { action }
            | LineageActionDefinition::Constraint { action }
            | LineageActionDefinition::Dimension { action }
            | LineageActionDefinition::Trim { action }
            | LineageActionDefinition::Parameter { action }
            | LineageActionDefinition::Binding { action }
            | LineageActionDefinition::External { action }
            | LineageActionDefinition::Operation { action }
            | LineageActionDefinition::ComputedFeature { action }
            | LineageActionDefinition::Annotation { action } => action,
            LineageActionDefinition::ImportedBaseline { .. } => {
                panic!("test forgery requires an ordinary action")
            }
        }
    }

    fn refresh_compiled_intent_digest(step: &mut LineageStep) {
        let payload = ordinary_payload_mut(step);
        let intent = payload
            .parameters
            .get(AUTHORED_INTENT_PARAMETER)
            .expect("authored intent")
            .clone();
        let owner_fields = payload
            .parameters
            .get(AUTHORED_OWNER_FIELDS_PARAMETER)
            .expect("authored owner fields")
            .clone();
        let mut compiled = serde_json::from_value::<CompiledMaterializationRecipe>(
            payload
                .parameters
                .get(MATERIALIZATION_PARAMETER)
                .expect("compiled materialization")
                .clone(),
        )
        .expect("typed compiled materialization");
        compiled.intent_digest =
            authored_intent_digest(&intent, &owner_fields).expect("recomputed hostile digest");
        payload.parameters.insert(
            MATERIALIZATION_PARAMETER.into(),
            serde_json::to_value(compiled).expect("hostile compiled materialization"),
        );
    }

    fn set_point_position(operations: &mut [StructuralDeltaOperation], position: [f64; 2]) {
        let point = operations
            .iter_mut()
            .find_map(|operation| match operation {
                StructuralDeltaOperation::UpsertEntity { path, value, .. }
                    if path.last().is_some_and(|field| field == "points") =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("point upsert");
        point
            .as_object_mut()
            .expect("point object")
            .insert("position".into(), json!(position));
    }

    fn with_bumped_label(
        mut document: geosolve_sketch_lineage::LineageDocument,
        suffix: &str,
    ) -> geosolve_sketch_lineage::LineageDocument {
        let step = document.steps()[0].clone();
        document
            .apply_patch(LineagePatch::new(
                document.identity(),
                vec![LineageMutation::Rewrite {
                    step: step.id,
                    replacement: Box::new(LineageStepRewrite {
                        label: format!("{} {suffix}", step.label),
                        action: step.action,
                    }),
                }],
            ))
            .expect("bump document revision without changing manifest semantics");
        document
    }

    #[test]
    fn live_rejected_valid_checkpoint_publishes_strict_cold_accepted_authority() {
        let coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        let mut lineage = coordinator.lineage.clone();
        let baseline = lineage.session.document().steps()[0].id;
        let replacement = with_bumped_label(
            lineage.session.document().clone(),
            "live-rejected-cold-accepted",
        );
        lineage
            .session
            .reconcile(lineage.session.identity(), replacement)
            .expect("one valid pending lineage checkpoint");
        let target = lineage.session.identity();
        let history = (lineage.session.undo_len(), lineage.session.redo_len());
        assert_eq!(
            lineage
                .session
                .latest_attempt()
                .expect("pending live checkpoint")
                .disposition,
            LineageEvaluationDisposition::Pending
        );

        let evidence = lineage
            .record_evaluation(
                &ParameterBatch::default(),
                &ExternalSnapshotSet::default(),
                false,
                baseline,
                None,
            )
            .expect("strict cold acceptance must overrule the live rejection report")
            .expect("cold accepted evidence");

        let attempt = lineage
            .session
            .latest_attempt()
            .expect("accepted cold attempt");
        assert_eq!(attempt.target, target);
        assert_eq!(attempt.disposition, LineageEvaluationDisposition::Accepted);
        assert_eq!(
            attempt.materialization_digest,
            Some(evidence.materialization_digest())
        );
        assert!(attempt.failed_steps.is_empty());
        let authority = lineage
            .session
            .last_accepted()
            .expect("current accepted lineage authority");
        assert_eq!(authority.lineage, target);
        assert_eq!(
            authority.materialization_digest,
            evidence.materialization_digest()
        );
        assert_eq!(
            (lineage.session.undo_len(), lineage.session.redo_len()),
            history,
            "evaluation publication cannot create another history position"
        );
    }

    #[test]
    fn live_and_strict_cold_rejection_retain_failed_lineage_authority() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let point = document.add_point("fixed", [0.0, 0.0]).expect("point");
        document
            .add_constraint(
                "original fixed point",
                DocumentConstraintDefinition::FixedPoint {
                    point,
                    target: [0.0, 0.0],
                },
            )
            .expect("original fixed point");
        let mut coordinator = coordinator(document);
        let accepted_before = coordinator
            .lineage
            .session
            .last_accepted()
            .cloned()
            .expect("initial accepted authority");

        let rejected = coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::CreateConstraint {
                    label: "conflicting fixed point".into(),
                    definition: DocumentConstraintDefinition::FixedPoint {
                        point,
                        target: [1.0, 0.0],
                    },
                },
            )
            .expect("genuine owning-domain rejection remains retained");

        assert!(rejected.published_accepted.is_none());
        let attempt = coordinator
            .lineage
            .session
            .latest_attempt()
            .expect("retained failed attempt");
        let cold_failure =
            crate::coordinator::lineage_evaluation::evaluate_lineage_session_cold_with_inputs(
                &LineageSession::new(coordinator.lineage.session.document().clone())
                    .expect("strict evaluation session"),
                &ParameterBatch::default(),
                &ExternalSnapshotSet::default(),
            )
            .expect_err("the conflicting fixed point fails strict cold evaluation");
        assert_eq!(attempt.disposition, LineageEvaluationDisposition::Failed);
        assert_eq!(
            attempt.diagnostic.as_ref().map(LineageSemanticKey::as_str),
            Some(cold_failure.diagnostic())
        );
        assert_eq!(
            attempt.failed_steps,
            vec![
                cold_failure
                    .failed_step()
                    .expect("strict prefix failure attribution")
            ]
        );
        assert_eq!(
            coordinator.lineage.session.last_accepted(),
            Some(&accepted_before),
            "a genuine rejection must preserve the complete previous accepted authority"
        );

        let mut falsely_accepted = coordinator.lineage.clone();
        let failed_step = attempt.failed_steps[0];
        let retained_session = falsely_accepted
            .to_canonical_session_json()
            .expect("retained session before false acceptance");
        let retained_ledger = falsely_accepted
            .to_canonical_host_input_ledger_json()
            .expect("retained ledger before false acceptance");
        let error = falsely_accepted
            .record_evaluation(
                &ParameterBatch::default(),
                &ExternalSnapshotSet::default(),
                true,
                failed_step,
                None,
            )
            .expect_err("a live acceptance report cannot overrule strict cold rejection");
        assert!(matches!(
            error,
            LineageBridgeError::AcceptedEvaluationRejected(_)
        ));
        assert_eq!(
            falsely_accepted
                .to_canonical_session_json()
                .expect("session after false acceptance"),
            retained_session
        );
        assert_eq!(
            falsely_accepted
                .to_canonical_host_input_ledger_json()
                .expect("ledger after false acceptance"),
            retained_ledger,
            "cold rejection must roll back lineage session and host-input ledger together"
        );
    }

    #[test]
    fn accepted_live_report_rolls_back_when_cold_bytes_mismatch_staged_acceptance() {
        let mut document = SketchDocument::new(1.0).expect("document");
        document.add_point("point", [1.0, 2.0]).expect("point");
        let coordinator = coordinator(document);
        let mut lineage = coordinator.lineage.clone();
        let failed_step = lineage.session.document().steps()[0].id;
        let mut mismatched = serde_json::from_str::<Value>(
            coordinator
                .checkpoint()
                .accepted_json()
                .expect("accepted sketch bytes"),
        )
        .expect("accepted sketch value");
        mismatched["points"][0]["position"] = json!([9.0, 10.0]);
        let mismatched = serde_json::to_string(&mismatched).expect("mismatched accepted bytes");
        let retained_session = lineage
            .to_canonical_session_json()
            .expect("retained session before mismatch");
        let retained_ledger = lineage
            .to_canonical_host_input_ledger_json()
            .expect("retained input ledger before mismatch");

        let error = lineage
            .record_evaluation(
                &ParameterBatch::default(),
                &ExternalSnapshotSet::default(),
                true,
                failed_step,
                Some((&mismatched, false)),
            )
            .expect_err("mismatched staged/cold accepted bytes must reject");

        assert!(matches!(
            error,
            LineageBridgeError::AcceptedMaterializationMismatch
        ));
        assert_eq!(
            lineage
                .to_canonical_session_json()
                .expect("session after mismatch"),
            retained_session
        );
        assert_eq!(
            lineage
                .to_canonical_host_input_ledger_json()
                .expect("input ledger after mismatch"),
            retained_ledger,
            "failed accepted publication must roll back session and ledger together"
        );
    }

    #[test]
    fn structural_delta_round_trips_entity_add_update_delete_and_plain_arrays() {
        let before = json!({
            "points": [
                { "id": "p1", "position": [0.0, 0.0] },
                { "id": "p2", "position": [1.0, 0.0] }
            ],
            "order": ["p1", "p2"],
            "metadata": { "label": "before", "obsolete": true }
        });
        let after = json!({
            "points": [
                { "id": "p1", "position": [2.0, 3.0] },
                { "id": "p3", "position": [4.0, 5.0] }
            ],
            "order": ["p1", "p3"],
            "metadata": { "label": "after" }
        });
        let delta = structural_delta(&before, &after).expect("delta");
        let mut rebuilt = before.clone();
        for operation in &delta {
            apply_operation(&mut rebuilt, operation).expect("apply delta");
        }
        assert_eq!(rebuilt, after);
    }

    #[test]
    fn branch_variant_shape_changes_keep_stable_atomic_owner_leaves() {
        let before = json!({
            "contacts": [{
                "id": "contact-1",
                "domain": { "kind": "bounded", "lower": 0.0, "upper": 1.0 },
                "neighborhood": "interior"
            }],
            "scalars": [{
                "id": "scalar-1",
                "domain": { "kind": "bounded", "lower": 0.0, "upper": 1.0 }
            }]
        });
        let after = json!({
            "contacts": [{
                "id": "contact-1",
                "domain": { "kind": "supporting_line" },
                "neighborhood": { "local": { "lower": -0.5, "upper": 0.5 } }
            }],
            "scalars": [{
                "id": "scalar-1",
                "domain": { "kind": "finite" }
            }]
        });
        let delta = structural_delta(&before, &after).expect("branch delta");
        let leaves = delta
            .iter()
            .flat_map(|change| {
                direct_materialized_leaves_for_change(&before, &after, change)
                    .expect("stable branch leaves")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            leaves,
            BTreeSet::from([
                materialized_leaf(LineageOutputKind::Contact, "contact-1", "domain")
                    .expect("contact domain leaf"),
                materialized_leaf(LineageOutputKind::Contact, "contact-1", "neighborhood")
                    .expect("contact neighborhood leaf"),
                materialized_leaf(LineageOutputKind::Scalar, "scalar-1", "domain")
                    .expect("scalar domain leaf"),
            ]),
            "explicit branch variants retain one stable owner per semantic property"
        );
    }

    #[test]
    fn nested_feature_corner_changes_keep_exact_child_owner_leaves() {
        let before = json!({
            "features": [{
                "id": "feature-1",
                "label": "Fillet",
                "suppressed": false,
                "definition": {
                    "kind": "fillet_set",
                    "radius": 0.8,
                    "corners": [{
                        "id": "corner-1",
                        "first": { "picked_parameter": 0.8 },
                        "second": { "picked_parameter": 0.16 }
                    }]
                }
            }]
        });
        let after = json!({
            "features": [{
                "id": "feature-1",
                "label": "Fillet",
                "suppressed": false,
                "definition": {
                    "kind": "fillet_set",
                    "radius": 0.96,
                    "corners": [{
                        "id": "corner-1",
                        "first": { "picked_parameter": 0.76 },
                        "second": { "picked_parameter": 0.192 }
                    }]
                }
            }]
        });
        let delta = structural_delta(&before, &after).expect("feature delta");
        let [change] = delta.as_slice() else {
            panic!("one feature entity replacement expected");
        };
        assert!(
            !structural_change_transitions_identity(&before, change),
            "editing stable feature/corner values must not invent identity lifecycle"
        );
        let leaves = direct_materialized_leaves_for_change(&before, &after, change)
            .expect("nested writable leaves");
        for expected in [
            materialized_leaf(LineageOutputKind::Feature, "feature-1", "definition.radius")
                .expect("feature radius leaf"),
            materialized_leaf(
                LineageOutputKind::FeatureCorner,
                "corner-1",
                "first.picked_parameter",
            )
            .expect("first corner parameter leaf"),
            materialized_leaf(
                LineageOutputKind::FeatureCorner,
                "corner-1",
                "second.picked_parameter",
            )
            .expect("second corner parameter leaf"),
        ] {
            assert!(
                leaves.contains(&expected),
                "missing nested owner {expected:?}"
            );
        }
    }

    #[test]
    fn imported_curve_deletion_retires_once_and_undo_restores_the_reserved_identity() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [1.0, 0.0]).expect("end");
        let curve = document
            .add_curve(
                "construction line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        document
            .set_geometry_role(curve, GeometryRole::Construction)
            .expect("construction role");
        let mut coordinator = coordinator(document);
        let baseline = coordinator.lineage_document().steps()[0].clone();
        let baseline_curve = baseline
            .outputs
            .iter()
            .find(|output| output.kind == LineageOutputKind::Curve)
            .expect("baseline curve output");
        let source = LineageOutputRef {
            document: coordinator.lineage_document().id(),
            step: baseline.id,
            output: baseline_curve.id,
            kind: baseline_curve.kind,
        };

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(curve))]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("delete imported construction curve");
        let retirement = &coordinator.lineage_document().steps()[1];
        assert!(retirement.output_identities.iter().any(|identity| {
            matches!(identity.flow, LineageOutputIdentityFlow::Retired { source: retired } if retired == source)
        }));
        assert!(!retirement.output_identities.iter().any(|identity| {
            matches!(identity.flow, LineageOutputIdentityFlow::Continued { source: continued } if continued == source)
        }));
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(curve)
                .is_none()
        );

        coordinator.undo().expect("Undo retirement action");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(curve)
                .is_some()
        );
        coordinator.redo().expect("Redo retirement action");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(curve)
                .is_none()
        );
        assert!(
            baseline.reservations.iter().any(|reservation| {
                reservation.persistent_id.as_str() == format!("curve:{curve}")
            })
        );
    }

    #[test]
    fn exact_feature_lifecycle_is_restored_without_an_invented_revision_bump() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(8.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let mut baseline = LineageCheckpoint::capture(coordinator.checkpoint());
        let mut current = baseline.clone();
        current.feature_lifecycle.revision = ComputedFeatureRevision::from_raw(7);
        current.feature_lifecycle.allocator.next_feature_id = ComputedFeatureId::from_raw(3);
        current.feature_lifecycle.allocator.next_corner_id = ComputedFeatureCornerId::from_raw(5);

        baseline
            .retain_lifecycle_high_water(&current)
            .expect("retain exact lifecycle");
        let value = baseline.structural_value().expect("structural value");
        let mut rebuilt = baseline;
        rebuilt
            .replace_structural_value(&value)
            .expect("replace structural value");
        let features =
            geosolve_sketch_features::ComputedFeatureDocument::from_json(&rebuilt.feature_json)
                .expect("strict feature document");
        assert_eq!(features.revision(), ComputedFeatureRevision::from_raw(7));
        assert_eq!(features.lifecycle_high_water(), current.feature_lifecycle);
    }

    #[test]
    fn imported_and_authored_points_publish_distinct_exact_xy_owners() {
        let mut imported_document = SketchDocument::new(1.0).expect("document");
        let imported = imported_document
            .add_point("imported", [1.0, 2.0])
            .expect("imported point");
        let imported_coordinator = coordinator(imported_document);
        let imported_owner = imported_coordinator.lineage_document().steps()[0].id;
        let imported_map =
            LineageMaterializationMap::derive(imported_coordinator.lineage_document())
                .expect("imported ownership map");
        let imported_x = materialized_leaf(
            LineageOutputKind::Point,
            &imported.to_string(),
            "position.x",
        )
        .expect("imported X leaf");
        let imported_y = materialized_leaf(
            LineageOutputKind::Point,
            &imported.to_string(),
            "position.y",
        )
        .expect("imported Y leaf");
        let imported_owners = [
            imported_map
                .owner_for_leaf(&imported_x)
                .expect("imported X owner"),
            imported_map
                .owner_for_leaf(&imported_y)
                .expect("imported Y owner"),
        ];
        assert_eq!(imported_owners[0].step, imported_owner);
        assert_eq!(imported_owners[1].step, imported_owner);
        assert_eq!(imported_owners[0].field.as_str(), "position.x");
        assert_eq!(imported_owners[1].field.as_str(), "position.y");
        assert_ne!(imported_owners[0], imported_owners[1]);

        let mut authored_coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        let authored = authored_coordinator
            .apply_construction(
                authored_coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([3.0, 4.0]),
                },
            )
            .expect("authored point")
            .value
            .points[0];
        let authored_owner = authored_coordinator.lineage_document().steps()[1].id;
        let authored_map =
            LineageMaterializationMap::derive(authored_coordinator.lineage_document())
                .expect("authored ownership map");
        for (key, expected_field) in [("position.x", "position.x"), ("position.y", "position.y")] {
            let leaf = materialized_leaf(LineageOutputKind::Point, &authored.to_string(), key)
                .expect("authored point leaf");
            let owner = authored_map
                .owner_for_leaf(&leaf)
                .expect("authored point owner");
            assert_eq!(owner.step, authored_owner);
            assert_eq!(owner.field.as_str(), expected_field);
        }
    }

    #[test]
    fn partial_direct_writable_declaration_rejects_before_any_lineage_mutation() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        let point = coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point")
            .value
            .points[0];
        let owner = coordinator.lineage_document().steps()[1].id;
        let before_checkpoint = LineageCheckpoint::capture(coordinator.checkpoint());
        let before = authored_structural_value(&before_checkpoint).expect("before value");
        let original_lineage = coordinator.lineage.clone();

        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                DocumentEdit::SetPointPosition {
                    point,
                    position: [3.0, 4.0],
                },
            )
            .expect("reference accepted projection");
        let after_checkpoint = LineageCheckpoint::capture(coordinator.checkpoint());
        let after = authored_structural_value(&after_checkpoint).expect("after value");
        let delta = structural_delta(&before, &after).expect("point delta");
        assert!(!delta.is_empty());

        let mut partial = without_writable_leaf(&original_lineage, owner, "position.x");
        let retained = partial
            .to_canonical_session_json()
            .expect("retained partial lineage");
        let retained_identity = partial.identity();
        let retained_history = (partial.undo_len(), partial.redo_len());
        let error = partial
            .rewrite_direct_owners(&before, &after, &delta, &after_checkpoint)
            .expect_err("one missing changed leaf must reject the complete rewrite");
        assert!(matches!(error, LineageBridgeError::MissingOwner));
        assert_eq!(partial.identity(), retained_identity);
        assert_eq!((partial.undo_len(), partial.redo_len()), retained_history);
        assert_eq!(
            partial
                .to_canonical_session_json()
                .expect("unchanged partial lineage"),
            retained,
            "rejection must preserve lineage, history, and allocator authority"
        );
    }

    #[test]
    fn loaded_baseline_manifest_cannot_omit_an_authored_leaf() {
        let mut document = SketchDocument::new(1.0).expect("document");
        document
            .add_point("imported", [1.0, 2.0])
            .expect("imported point");
        let coordinator = coordinator(document);
        let source = coordinator.lineage_document();
        let baseline_owner = source.steps()[0].id;
        let mut hostile_document = geosolve_sketch_lineage::LineageDocument::with_id(source.id());
        let mut baseline = source.steps()[0].clone();
        let removed = baseline
            .writable_leaves
            .iter()
            .position(|leaf| leaf.key.as_str() == "position.y")
            .map(|index| baseline.writable_leaves.remove(index))
            .expect("imported Y declaration");
        assert_eq!(removed.key.as_str(), "position.y");
        hostile_document
            .apply_patch(LineagePatch::new(
                hostile_document.identity(),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(baseline),
                }],
            ))
            .expect("omitted baseline leaf remains structurally valid");
        let hostile = LineageSession::new(hostile_document).expect("digest-valid hostile session");
        let error = CoordinatorLineage::from_session_json(
            &hostile
                .to_canonical_session_json()
                .expect("hostile session JSON"),
        )
        .expect_err("baseline writable ownership must derive from its exact payload");
        assert_eq!(error.materialization_step(), Some(baseline_owner));
        assert!(error.to_string().contains("writable-leaf manifest"));
    }

    #[test]
    fn stale_or_tampered_disposable_map_rejects_without_touching_live_authority() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("first point");
        let stale_map = coordinator
            .lineage_materialization_map_json()
            .expect("first ownership map");
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([3.0, 4.0]),
                },
            )
            .expect("second point");
        let session = coordinator
            .lineage_session_json()
            .expect("current lineage authority");
        let retained = (
            coordinator.lineage_identity(),
            coordinator.history_len(),
            coordinator.history_cursor(),
            coordinator
                .session()
                .persistent_identity_high_water()
                .clone(),
        );
        assert!(
            RetainedEditorCoordinator::validate_lineage_materialization_map_json(
                &session, &stale_map,
            )
            .is_err(),
            "a map from an earlier lineage revision must be rejected"
        );

        let current_map = coordinator
            .lineage_materialization_map_json()
            .expect("current ownership map");
        let mut tampered =
            serde_json::from_str::<serde_json::Value>(&current_map).expect("map JSON");
        tampered["reverse"] = json!([]);
        assert!(
            RetainedEditorCoordinator::validate_lineage_materialization_map_json(
                &session,
                &serde_json::to_string(&tampered).expect("tampered map JSON"),
            )
            .is_err(),
            "a caller-edited reverse map must be rejected"
        );
        assert_eq!(
            (
                coordinator.lineage_identity(),
                coordinator.history_len(),
                coordinator.history_cursor(),
                coordinator
                    .session()
                    .persistent_identity_high_water()
                    .clone(),
            ),
            retained,
            "cache validation cannot mutate lineage, history, or allocators"
        );
    }

    #[test]
    fn loaded_continuation_manifest_cannot_claim_unchanged_leaf() {
        let (coordinator, source_end, baseline_owner, operation_owner) =
            coordinator_with_extended_line();
        let before_session = coordinator
            .lineage_session_json()
            .expect("original lineage session");
        let original_map = LineageMaterializationMap::derive(coordinator.lineage_document())
            .expect("original ownership map");
        let y = materialized_leaf(
            LineageOutputKind::Point,
            &source_end.to_string(),
            "position.y",
        )
        .expect("extended endpoint Y");
        assert_eq!(
            original_map
                .owner_for_leaf(&y)
                .expect("original Y owner")
                .step,
            baseline_owner,
            "Extend did not change Y and must not own it"
        );

        let source = coordinator.lineage_document();
        let mut forged_document = geosolve_sketch_lineage::LineageDocument::with_id(source.id());
        for source_step in source.steps() {
            let mut step = source_step.clone();
            if step.id == operation_owner {
                let x = step
                    .writable_leaves
                    .iter()
                    .find(|leaf| leaf.key.as_str() == "position.x")
                    .expect("Extend X declaration")
                    .clone();
                step.writable_leaves.push(LineageWritableLeaf {
                    output: x.output,
                    key: geosolve_sketch_lineage::LineageSemanticKey::new("position.y")
                        .expect("Y field"),
                });
            }
            forged_document
                .apply_patch(LineagePatch::new(
                    forged_document.identity(),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(step),
                    }],
                ))
                .expect("forged manifest remains structurally valid");
        }
        let forged_map = LineageMaterializationMap::derive(&forged_document)
            .expect("core map demonstrates the hostile declaration");
        assert_eq!(
            forged_map.owner_for_leaf(&y).expect("forged Y owner").step,
            operation_owner,
            "the low-level declaration alone would steal reverse Y authority"
        );

        let forged_session = LineageSession::new(forged_document)
            .expect("structurally valid forged session")
            .to_canonical_session_json()
            .expect("canonical forged session");
        let error = CoordinatorLineage::from_session_json(&forged_session)
            .expect_err("editor semantic loading must reject the forged owner");
        assert_eq!(error.materialization_step(), Some(operation_owner));
        assert!(
            error.to_string().contains("writable-leaf manifest"),
            "typed manifest rejection: {error}"
        );
        assert!(
            RetainedEditorCoordinator::lineage_materialization_map_json_for_session(
                &forged_session,
            )
            .is_err(),
            "a forged session cannot publish a disposable reverse map"
        );
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged original session"),
            before_session,
            "read-only hostile validation cannot mutate live authority"
        );
    }

    #[test]
    fn loaded_action_reservations_cannot_swap_persistent_id_authority() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Line {
                    start: ConstructionPoint::New([1.0, 2.0]),
                    end: ConstructionPoint::New([3.0, 4.0]),
                },
            )
            .expect("authored line");
        let owner = coordinator.lineage_document().steps()[1].id;
        let retained = coordinator
            .lineage_session_json()
            .expect("original lineage authority");
        let hostile = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let points = step
                .reservations
                .iter()
                .enumerate()
                .filter(|reservation| {
                    reservation.1.kind == geosolve_sketch_lineage::LineageReservationKind::Point
                })
                .map(|(index, _)| index)
                .take(2)
                .collect::<Vec<_>>();
            let [first, second] = points.as_slice() else {
                panic!("line recipe must reserve two points");
            };
            let first_id = step.reservations[*first].persistent_id.clone();
            let second_id = step.reservations[*second].persistent_id.clone();
            step.reservations[*first].persistent_id = second_id;
            step.reservations[*second].persistent_id = first_id;
        });

        let error = CoordinatorLineage::from_session_json(&hostile)
            .expect_err("action reservations must derive from the structural recipe");
        assert_eq!(error.materialization_step(), Some(owner));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged original lineage"),
            retained,
            "hostile validation must preserve the live coordinator byte-for-byte"
        );

        let reservation_ids = coordinator.lineage_document().steps()[1]
            .reservations
            .iter()
            .filter(|reservation| reservation.kind == LineageReservationKind::Point)
            .map(|reservation| reservation.id)
            .take(2)
            .collect::<Vec<_>>();
        let [first, second] = reservation_ids.as_slice() else {
            panic!("line recipe must reserve two points");
        };
        let hostile = session_with_forged_step(&coordinator.lineage, owner, |step| {
            for reservation in &mut step.reservations {
                reservation.id = if reservation.id == *first {
                    *second
                } else if reservation.id == *second {
                    *first
                } else {
                    reservation.id
                };
            }
            for output in &mut step.outputs {
                output.reservation = output.reservation.map(|reservation| {
                    if reservation == *first {
                        *second
                    } else if reservation == *second {
                        *first
                    } else {
                        reservation
                    }
                });
            }
            for identity in &mut step.output_identities {
                if let LineageOutputIdentityFlow::Created { reservation } = &mut identity.flow {
                    *reservation = if *reservation == *first {
                        *second
                    } else if *reservation == *second {
                        *first
                    } else {
                        *reservation
                    };
                }
            }
        });
        let error = CoordinatorLineage::from_session_json(&hostile)
            .expect_err("action reservation IDs must preserve compiler allocation order");
        assert_eq!(error.materialization_step(), Some(owner));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged original lineage"),
            retained
        );
    }

    #[test]
    fn loaded_created_output_cannot_forge_continued_or_owned_logical_flow() {
        let mut imported = SketchDocument::new(1.0).expect("document");
        imported
            .add_point("imported", [0.0, 0.0])
            .expect("imported point");
        let mut coordinator = coordinator(imported);
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point");
        let source = coordinator.lineage_document();
        let owner = source.steps()[1].id;
        let baseline_output = source.steps()[0]
            .outputs
            .iter()
            .find(|output| output.kind == LineageOutputKind::Point)
            .expect("baseline point output");
        let baseline_ref = LineageOutputRef {
            document: source.id(),
            step: source.steps()[0].id,
            output: baseline_output.id,
            kind: baseline_output.kind,
        };
        let retained = coordinator
            .lineage_session_json()
            .expect("original lineage authority");

        let continued = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let output = step
                .outputs
                .iter_mut()
                .find(|output| output.kind == LineageOutputKind::Point)
                .expect("authored point output");
            let reservation = output.reservation.take().expect("point reservation");
            step.reservations
                .retain(|candidate| candidate.id != reservation);
            step.output_identities
                .iter_mut()
                .find(|identity| identity.output == output.id)
                .expect("point identity")
                .flow = LineageOutputIdentityFlow::Continued {
                source: baseline_ref,
            };
        });
        let error = CoordinatorLineage::from_session_json(&continued)
            .expect_err("Created-to-Continued identity forgery must reject");
        assert_eq!(error.materialization_step(), Some(owner));

        let logical = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let output = step
                .outputs
                .iter_mut()
                .find(|output| output.kind == LineageOutputKind::Point)
                .expect("authored point output");
            let reservation = output.reservation.take().expect("point reservation");
            step.reservations
                .retain(|candidate| candidate.id != reservation);
            step.output_identities
                .iter_mut()
                .find(|identity| identity.output == output.id)
                .expect("point identity")
                .flow = LineageOutputIdentityFlow::OwnedLogical;
        });
        let error = CoordinatorLineage::from_session_json(&logical)
            .expect_err("Created-to-OwnedLogical identity forgery must reject");
        assert_eq!(error.materialization_step(), Some(owner));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged original lineage"),
            retained,
            "both hostile loads must leave live authority unchanged"
        );
    }

    #[test]
    fn loaded_action_cannot_publish_ghost_output_or_reservation() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point");
        let source = coordinator.lineage_document();
        let owner = source.steps()[1].id;
        let allocators = source.allocator_high_water();
        let hostile = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let output = allocators.next_output_id;
            let reservation = allocators.next_reservation_id;
            step.outputs.push(LineageOutput {
                id: output,
                key: LineageSemanticKey::new("ghost-point").expect("ghost output key"),
                kind: LineageOutputKind::Point,
                reservation: Some(reservation),
            });
            step.output_identities.push(LineageOutputIdentity {
                output,
                flow: LineageOutputIdentityFlow::Created { reservation },
            });
            step.reservations.push(LineageReservation {
                id: reservation,
                key: LineageSemanticKey::new("ghost-point").expect("ghost reservation key"),
                kind: LineageReservationKind::Point,
                persistent_id: LineageOpaqueId::new("point:m83-ghost")
                    .expect("ghost persistent ID"),
            });
        });

        let error = CoordinatorLineage::from_session_json(&hostile)
            .expect_err("geometry-free ghost identity authority must reject");
        assert_eq!(error.materialization_step(), Some(owner));
    }

    #[test]
    fn loaded_action_cannot_forge_owner_and_compiled_values_behind_unchanged_intent() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point");
        let owner = coordinator.lineage_document().steps()[1].id;
        let retained = coordinator
            .lineage_session_json()
            .expect("original lineage authority");
        let hostile = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let payload = ordinary_payload_mut(step);
            let mut owner_fields = serde_json::from_value::<Vec<StructuralDeltaOperation>>(
                payload
                    .parameters
                    .get(AUTHORED_OWNER_FIELDS_PARAMETER)
                    .expect("owner fields")
                    .clone(),
            )
            .expect("typed owner fields");
            set_point_position(&mut owner_fields, [9.0, 9.0]);
            payload.parameters.insert(
                AUTHORED_OWNER_FIELDS_PARAMETER.into(),
                serde_json::to_value(owner_fields).expect("hostile owner fields"),
            );

            let mut compiled = serde_json::from_value::<CompiledMaterializationRecipe>(
                payload
                    .parameters
                    .get(MATERIALIZATION_PARAMETER)
                    .expect("compiled materialization")
                    .clone(),
            )
            .expect("typed compiled materialization");
            set_point_position(&mut compiled.operations, [9.0, 9.0]);
            payload.parameters.insert(
                MATERIALIZATION_PARAMETER.into(),
                serde_json::to_value(compiled).expect("hostile compiled materialization"),
            );
            refresh_compiled_intent_digest(step);
        });

        let error = CoordinatorLineage::from_session_json(&hostile)
            .expect_err("duplicate cache bytes cannot contradict semantic output values");
        assert_eq!(error.materialization_step(), Some(owner));
        assert!(error.to_string().contains("semantic output field values"));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged live authority"),
            retained
        );
    }

    #[test]
    fn loaded_action_requires_exact_complete_semantic_output_values() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point");
        let owner = coordinator.lineage_document().steps()[1].id;

        let missing = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let payload = ordinary_payload_mut(step);
            let fields = payload
                .parameters
                .get_mut(AUTHORED_INTENT_PARAMETER)
                .and_then(Value::as_object_mut)
                .and_then(|intent| intent.get_mut("owned_output_fields"))
                .and_then(Value::as_array_mut)
                .expect("semantic output fields");
            fields.pop().expect("at least one semantic output field");
            refresh_compiled_intent_digest(step);
        });
        let error = CoordinatorLineage::from_session_json(&missing)
            .expect_err("a missing semantic output field must reject");
        assert_eq!(error.materialization_step(), Some(owner));
        assert!(error.to_string().contains("exactly cover"));

        let duplicate = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let payload = ordinary_payload_mut(step);
            let fields = payload
                .parameters
                .get_mut(AUTHORED_INTENT_PARAMETER)
                .and_then(Value::as_object_mut)
                .and_then(|intent| intent.get_mut("owned_output_fields"))
                .and_then(Value::as_array_mut)
                .expect("semantic output fields");
            fields.push(fields[0].clone());
            refresh_compiled_intent_digest(step);
        });
        let error = CoordinatorLineage::from_session_json(&duplicate)
            .expect_err("a duplicate semantic output field must reject");
        assert_eq!(error.materialization_step(), Some(owner));
        assert!(error.to_string().contains("exactly cover"));
    }

    #[test]
    fn loaded_action_cannot_retarget_same_kind_semantic_input() {
        let (coordinator, _, _, owner) = coordinator_with_extended_line();
        let retained = coordinator
            .lineage_session_json()
            .expect("original lineage authority");
        let hostile = session_with_forged_step(&coordinator.lineage, owner, |step| {
            let LineageActionDefinition::Operation { action } = &mut step.action else {
                panic!("Extend must be an Operation action");
            };
            let target = action
                .inputs
                .iter()
                .find(|input| input.key.as_str() == "target")
                .expect("target input")
                .source;
            let line = action
                .inputs
                .iter_mut()
                .find(|input| input.key.as_str() == "line")
                .expect("line input");
            assert_eq!(line.kind, target.kind);
            assert_ne!(line.source, target);
            line.source = target;
        });

        let error = CoordinatorLineage::from_session_json(&hostile)
            .expect_err("same-kind input retargeting must reject");
        assert_eq!(error.materialization_step(), Some(owner));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged original lineage"),
            retained
        );
    }

    #[test]
    fn forged_manifest_cannot_enter_accepted_undo_or_redo_authority() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point");
        let good = coordinator.lineage_document().clone();
        let owner = good.steps()[1].id;
        let hostile_json = session_with_forged_step(&coordinator.lineage, owner, |step| {
            step.reservations
                .iter_mut()
                .find(|reservation| reservation.kind == LineageReservationKind::Point)
                .expect("point reservation")
                .persistent_id =
                LineageOpaqueId::new("point:m83-history-forgery").expect("hostile ID");
        });
        let forged = LineageSession::from_session_json(&hostile_json)
            .expect("structurally valid forged session")
            .document()
            .clone();
        let retained = coordinator
            .lineage_session_json()
            .expect("original live authority");
        let current_error = CoordinatorLineage::from_session_json(&hostile_json)
            .expect_err("the forged current manifest must fail semantic authentication");
        assert_eq!(current_error.materialization_step(), Some(owner));

        let mut accepted = LineageSession::new(forged.clone()).expect("forged accepted session");
        accepted
            .accept_current(
                accepted.identity(),
                None,
                geosolve_sketch_lineage::lineage_content_digest(b"forged-accepted"),
            )
            .expect("self-consistent untrusted accepted authority");
        let accepted_before = accepted
            .to_canonical_session_json()
            .expect("forged accepted authority before rejection");
        let error = accepted
            .reconcile(
                accepted.identity(),
                with_bumped_label(good.clone(), "accepted"),
            )
            .expect_err("cross-history ledger must reject a forged accepted checkpoint");
        assert!(matches!(
            error,
            geosolve_sketch_lineage::LineageDocumentError::CrossHistoryIdentityRebinding { .. }
        ));
        assert_eq!(
            accepted
                .to_canonical_session_json()
                .expect("unchanged forged accepted authority"),
            accepted_before
        );

        let mut undo = LineageSession::new(forged.clone()).expect("forged Undo session");
        let error = undo
            .reconcile(undo.identity(), with_bumped_label(good.clone(), "undo"))
            .expect_err("cross-history ledger must reject a forged Undo predecessor");
        assert!(matches!(
            error,
            geosolve_sketch_lineage::LineageDocumentError::CrossHistoryIdentityRebinding { .. }
        ));
        assert_eq!((undo.undo_len(), undo.redo_len()), (0, 0));

        let mut redo = LineageSession::new(good.clone()).expect("good Redo session");
        let error = redo
            .reconcile(redo.identity(), with_bumped_label(forged, "redo"))
            .expect_err("cross-history ledger must reject a forged Redo successor");
        assert!(matches!(
            error,
            geosolve_sketch_lineage::LineageDocumentError::CrossHistoryIdentityRebinding { .. }
        ));
        assert_eq!((redo.undo_len(), redo.redo_len()), (0, 0));
        assert_eq!(
            coordinator
                .lineage_session_json()
                .expect("unchanged live authority"),
            retained,
            "history validation is read-only and atomic"
        );
    }

    #[test]
    fn history_cannot_rebind_abandoned_step_output_or_reservation_ids() {
        let source = SketchDocument::new(1.0).expect("document");
        let mut point_branch = coordinator(source.clone());
        point_branch
            .apply_construction(
                point_branch.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("point branch");
        let mut line_branch = coordinator(source);
        line_branch
            .apply_construction(
                line_branch.session().design_identity(),
                &ConstructionProposal::Line {
                    start: ConstructionPoint::New([1.0, 2.0]),
                    end: ConstructionPoint::New([3.0, 4.0]),
                },
            )
            .expect("line branch");

        let point_document = point_branch.lineage_document().clone();
        let line_document = line_branch.lineage_document().clone();
        assert_eq!(point_document.id(), line_document.id());
        assert_eq!(point_document.steps()[1].id, line_document.steps()[1].id);
        assert_ne!(
            point_document.steps()[1].outputs,
            line_document.steps()[1].outputs,
            "the hostile branches must reuse an output ID for different port meaning"
        );

        let mut hostile = LineageSession::new(point_document).expect("point lineage session");
        let retained = hostile
            .to_canonical_session_json()
            .expect("authority before hostile replacement");
        let error = hostile
            .reconcile(
                hostile.identity(),
                with_bumped_label(line_document, "history-rebind"),
            )
            .expect_err("cross-history identity reuse must reject before entering history");
        assert!(matches!(
            error,
            geosolve_sketch_lineage::LineageDocumentError::CrossHistoryIdentityRebinding { .. }
        ));
        assert_eq!((hostile.undo_len(), hostile.redo_len()), (0, 0));
        assert_eq!(
            hostile
                .to_canonical_session_json()
                .expect("unchanged authority after hostile replacement"),
            retained
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one predecessor-only W6 regression keeps leaf ownership, cache-only bytes, atomic history, and cold reproduction evidence together"
    )]
    fn predecessor_only_leaf_rewrite_refreshes_later_compiled_snapshot_atomically() {
        let (mut coordinator, source_end, baseline_owner, operation_owner) =
            coordinator_with_extended_line();
        let ownership = LineageMaterializationMap::derive(coordinator.lineage_document())
            .expect("post-Extend ownership map");
        let x = materialized_leaf(
            LineageOutputKind::Point,
            &source_end.to_string(),
            "position.x",
        )
        .expect("extended endpoint X");
        let y = materialized_leaf(
            LineageOutputKind::Point,
            &source_end.to_string(),
            "position.y",
        )
        .expect("extended endpoint Y");
        assert_eq!(
            ownership.owner_for_leaf(&x).expect("X owner").step,
            operation_owner
        );
        assert_eq!(
            ownership.owner_for_leaf(&y).expect("Y owner").step,
            baseline_owner
        );

        let initial_operation = coordinator
            .lineage_document()
            .step(operation_owner)
            .expect("operation owner")
            .clone();
        let LineageActionDefinition::Operation {
            action: initial_payload,
        } = &initial_operation.action
        else {
            panic!("Extend must retain a typed operation action");
        };
        let retained_intent = initial_payload
            .parameters
            .get(AUTHORED_INTENT_PARAMETER)
            .expect("operation intent")
            .clone();
        let retained_owner_fields = initial_payload
            .parameters
            .get(AUTHORED_OWNER_FIELDS_PARAMETER)
            .expect("operation owner fields")
            .clone();
        let retained_compiled = initial_payload
            .parameters
            .get(MATERIALIZATION_PARAMETER)
            .expect("operation compiled snapshot")
            .clone();
        let retained_history = (coordinator.history_len(), coordinator.history_cursor());
        let current = coordinator
            .session()
            .design_document()
            .point(source_end)
            .expect("extended endpoint")
            .position;

        let _ = coordinator.resolve_projected_point_move(
            0x83_06_03,
            1,
            source_end,
            [current[0], current[1] + 1.0],
        );
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted),
            "Y-only predecessor projection must be accepted"
        );
        let release_position = coordinator
            .solved_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
            .and_then(|accepted| accepted.document().point(source_end))
            .map(|point| point.position)
            .expect("accepted Y-only projection");
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: source_end,
                model_position: release_position,
            })
            .expect("atomic predecessor-owner publication")
            .expect("one committed point move");

        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            (retained_history.0 + 1, retained_history.1 + 1),
            "owner rewrite and downstream cache refresh must share one history position"
        );
        let refreshed_operation = coordinator
            .lineage_document()
            .step(operation_owner)
            .expect("refreshed operation owner");
        let LineageActionDefinition::Operation {
            action: refreshed_payload,
        } = &refreshed_operation.action
        else {
            panic!("Extend must remain a typed operation action");
        };
        assert_eq!(
            refreshed_payload.parameters.get(AUTHORED_INTENT_PARAMETER),
            Some(&retained_intent),
            "a cache-only refresh cannot rewrite operation intent"
        );
        assert_eq!(
            refreshed_payload
                .parameters
                .get(AUTHORED_OWNER_FIELDS_PARAMETER),
            Some(&retained_owner_fields),
            "a cache-only refresh cannot transfer semantic ownership"
        );
        assert_ne!(
            refreshed_payload.parameters.get(MATERIALIZATION_PARAMETER),
            Some(&retained_compiled),
            "the later complete snapshot must be refreshed to the accepted Y value"
        );
        let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(
            &coordinator
                .lineage_session_json()
                .expect("predecessor-only lineage session"),
        )
        .expect("cold predecessor-only materialization");
        assert_eq!(cold.design_json(), coordinator.checkpoint().design_json());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one semantic write-back regression keeps logical-port identity, exact values, stale-cache compilation, and cold reproduction evidence together"
    )]
    fn direct_point_rewrite_updates_exact_logical_output_fields_consumed_by_replay() {
        let mut coordinator = coordinator(SketchDocument::new(1.0).expect("document"));
        let point = coordinator
            .apply_construction(
                coordinator.session().design_identity(),
                &ConstructionProposal::Point {
                    point: ConstructionPoint::New([1.0, 2.0]),
                },
            )
            .expect("authored point")
            .value
            .points[0];
        let owner = coordinator.lineage_document().steps()[1].id;
        let initial = coordinator
            .lineage_document()
            .step(owner)
            .expect("geometry owner")
            .clone();
        let LineageActionDefinition::GeometryRecipe {
            action: initial_payload,
        } = &initial.action
        else {
            panic!("point construction must retain a geometry recipe");
        };
        let initial_intent = initial_payload
            .parameters
            .get(AUTHORED_INTENT_PARAMETER)
            .expect("initial point intent")
            .clone();
        let initial_output_fields = serde_json::from_value::<Vec<AuthoredOutputFieldValue>>(
            initial_intent
                .get("owned_output_fields")
                .expect("complete initial output fields")
                .clone(),
        )
        .expect("typed initial output fields");
        let initial_values = initial_output_fields
            .iter()
            .map(|field| (field.field.as_str(), field.value.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(initial_values.get("position.x"), Some(&json!(1.0)));
        assert_eq!(initial_values.get("position.y"), Some(&json!(2.0)));

        let _ = coordinator.resolve_projected_point_move(0x83_06_04, 1, point, [3.0, 4.0]);
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted),
            "point projection must be accepted"
        );
        let release_position = coordinator
            .solved_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
            .and_then(|accepted| accepted.document().point(point))
            .map(|point| point.position)
            .expect("accepted point projection");
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point,
                model_position: release_position,
            })
            .expect("semantic point owner publication")
            .expect("one committed point move");

        let rewritten = coordinator
            .lineage_document()
            .step(owner)
            .expect("rewritten geometry owner");
        let LineageActionDefinition::GeometryRecipe {
            action: rewritten_payload,
        } = &rewritten.action
        else {
            panic!("point construction must remain a geometry recipe");
        };
        let rewritten_intent = rewritten_payload
            .parameters
            .get(AUTHORED_INTENT_PARAMETER)
            .expect("rewritten point intent");
        assert_ne!(rewritten_intent, &initial_intent);
        assert_eq!(
            rewritten_intent.get("body"),
            initial_intent.get("body"),
            "direct manipulation rewrites the semantic output override, not the genesis request"
        );
        let output_fields = serde_json::from_value::<Vec<AuthoredOutputFieldValue>>(
            rewritten_intent
                .get("owned_output_fields")
                .expect("exact action-local output fields")
                .clone(),
        )
        .expect("typed action-local output fields");
        assert_eq!(output_fields.len(), initial_output_fields.len());
        let values = output_fields
            .iter()
            .map(|field| (field.field.as_str(), field.value.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(values.get("position.x"), Some(&json!(release_position[0])));
        assert_eq!(values.get("position.y"), Some(&json!(release_position[1])));
        let point_output = rewritten
            .outputs
            .iter()
            .find(|output| output.kind == LineageOutputKind::Point)
            .expect("stable point output");
        assert!(output_fields.iter().all(|field| {
            field.output == point_output.id && field.kind == LineageOutputKind::Point
        }));

        // The logical output fields are evaluator input, not decorative audit
        // metadata. Even a stale full-snapshot cache value is overwritten by
        // the stable action-local X field before structural replay.
        let mut stale_cache_step = rewritten.clone();
        let LineageActionDefinition::GeometryRecipe {
            action: stale_payload,
        } = &mut stale_cache_step.action
        else {
            panic!("point construction must remain a geometry recipe");
        };
        let mut stale_recipe = serde_json::from_value::<CompiledMaterializationRecipe>(
            stale_payload
                .parameters
                .get(MATERIALIZATION_PARAMETER)
                .expect("compiled point recipe")
                .clone(),
        )
        .expect("typed compiled point recipe");
        let stale_point = stale_recipe
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                StructuralDeltaOperation::UpsertEntity { id, value, .. }
                    if id == &point.to_string() =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("compiled point snapshot");
        stale_point["position"][0] = json!(-83.0);
        stale_payload.parameters.insert(
            MATERIALIZATION_PARAMETER.into(),
            serde_json::to_value(stale_recipe).expect("stale compiled cache"),
        );
        let intent_compiled =
            action_delta_for_step(coordinator.lineage_document(), &stale_cache_step)
                .expect("action-local fields compile over stale cache");
        let replayed_point = intent_compiled
            .iter()
            .find_map(|operation| match operation {
                StructuralDeltaOperation::UpsertEntity { id, value, .. }
                    if id == &point.to_string() =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("intent-compiled point snapshot");
        assert_eq!(replayed_point["position"][0], json!(release_position[0]));
        let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(
            &coordinator
                .lineage_session_json()
                .expect("rewritten point lineage session"),
        )
        .expect("cold rewritten point materialization");
        assert_eq!(cold.design_json(), coordinator.checkpoint().design_json());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact W6 regression keeps selective continuation ownership beside the resulting mixed-owner projected rewrite"
    )]
    fn changed_leaf_continuation_supports_one_atomic_mixed_owner_entity_rewrite() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let source_start = document
            .add_point("source start", [0.0, 0.0])
            .expect("source start");
        let source_end = document
            .add_point("source end", [1.0, 0.0])
            .expect("source end");
        let source = document
            .add_curve(
                "source",
                CurveDefinition::Line {
                    start: source_start,
                    end: source_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("source line");
        let target_start = document
            .add_point("target start", [2.0, -1.0])
            .expect("target start");
        let target_end = document
            .add_point("target end", [2.0, 1.0])
            .expect("target end");
        let target = document
            .add_curve(
                "target",
                CurveDefinition::Line {
                    start: target_start,
                    end: target_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .expect("target line");
        let mut coordinator = coordinator(document);
        let prepared = SketchOperationSnapshot::capture(coordinator.session())
            .prepare(SketchOperationRequest::ExtendLineToLine {
                line: CurveSpan::line(source),
                endpoint: LineEndpoint::End,
                target: CurveSpan::line(target),
            })
            .execute(OperationControl::unlimited())
            .expect("extend preparation");
        let OperationOutcome::Completed { value, .. } = prepared else {
            panic!("unbounded Extend preparation must complete");
        };
        let SketchOperationResult::Proposed(proposal) = value else {
            panic!("Extend proposal expected");
        };
        coordinator
            .apply_sketch_operation(&proposal)
            .expect("accepted Extend lineage action");

        let baseline_owner = coordinator.lineage_document().steps()[0].id;
        let operation_owner = coordinator.lineage_document().steps()[1].id;
        let ownership = LineageMaterializationMap::derive(coordinator.lineage_document())
            .expect("post-Extend ownership map");
        let x = materialized_leaf(
            LineageOutputKind::Point,
            &source_end.to_string(),
            "position.x",
        )
        .expect("extended endpoint X");
        let y = materialized_leaf(
            LineageOutputKind::Point,
            &source_end.to_string(),
            "position.y",
        )
        .expect("extended endpoint Y");
        assert_eq!(
            ownership.owner_for_leaf(&x).expect("X owner").step,
            operation_owner,
            "Extend changed X and therefore owns only that leaf"
        );
        assert_eq!(
            ownership.owner_for_leaf(&y).expect("Y owner").step,
            baseline_owner,
            "the numerically unchanged Y leaf must remain with its prior owner"
        );
        let before_actions = coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| serde_json::to_value(&step.action).expect("owner action"))
            .collect::<Vec<_>>();
        let retained_step_count = before_actions.len();
        let retained_history = (coordinator.history_len(), coordinator.history_cursor());

        let _ = coordinator.resolve_projected_point_move(0x83_06_02, 1, source_end, [3.0, 1.0]);
        assert!(
            coordinator
                .projected_drag_work_evidence()
                .is_some_and(|work| work.accepted),
            "mixed-owner endpoint projection must be accepted"
        );
        let release_position = coordinator
            .solved_preview_session()
            .and_then(RetainedSketchDocumentSession::accepted_state_for_current_input)
            .and_then(|accepted| accepted.document().point(source_end))
            .map(|point| point.position)
            .expect("accepted mixed-owner projection");
        coordinator
            .apply_editor_effect(&EditorEffect::CommitPointMove {
                expected: coordinator.session().design_identity(),
                point: source_end,
                model_position: release_position,
            })
            .expect("atomic mixed-owner publication")
            .expect("one committed point move");

        assert_eq!(
            coordinator.lineage_document().steps().len(),
            retained_step_count
        );
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            (retained_history.0 + 1, retained_history.1 + 1),
            "both owner rewrites must occupy one history position"
        );
        let after_actions = coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| serde_json::to_value(&step.action).expect("rewritten owner action"))
            .collect::<Vec<_>>();
        assert_ne!(after_actions[0], before_actions[0]);
        assert_ne!(after_actions[1], before_actions[1]);
        let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(
            &coordinator
                .lineage_session_json()
                .expect("mixed-owner lineage session"),
        )
        .expect("cold mixed-owner materialization");
        assert_eq!(cold.design_json(), coordinator.checkpoint().design_json());
    }
}
