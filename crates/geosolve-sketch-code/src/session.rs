// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::missing_errors_doc,
    reason = "the closed CodeSessionError enum documents the shared transactional failures"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_sketch_intent::intent_content_digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::expansion::expand_code_project_with_overlay_and_retained_operation_plans;
use crate::{
    AuditedCodeWork, CodeGeneratedChildAddress, CodeInteractionOverlay, CodeOverlayError,
    CodeProject, CodeWorkReceipt, ExpandedCodeProject, ExpandedWritablePoint,
    GeneratedMemberAddress, GeneratedMemberIdentity, KeyedReconcileError, KeyedReconcilePlan,
    KeyedReconcileState, ManagedDocument, ManagedValue, ProjectKey, stage_point_drags,
};

const MAX_HISTORY: usize = 256;
/// Largest session or revision admitted by the Rust/WASM/TypeScript wire.
///
/// JavaScript represents these fields as numbers, so accepting a larger
/// integer would silently destroy exact-CAS identity at the browser boundary.
pub const MAX_CODE_SESSION_WIRE_INTEGER: u64 = 9_007_199_254_740_991;
const SESSION_WIRE_VERSION: &str = "geosolve-sketch-code-session-v2";
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSessionIdentity {
    pub session: u64,
    pub revision: u64,
    pub digest: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use geosolve_sketch_intent::{IntentSession, IntentSessionId};

    use super::*;

    fn compiled_fixture(name: &str) -> crate::CompiledManagedSource {
        let json = match name {
            "base" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
            )),
            "restored" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope-restored.json"
            )),
            _ => panic!("unknown compiled session fixture"),
        };
        crate::CompiledManagedSource::from_json(json).expect("compiled session fixture")
    }

    fn project(key: &ProjectKey, fixture: &str) -> CodeProject {
        CodeProject::managed(key.clone(), compiled_fixture(fixture)).expect("valid V3 project")
    }

    fn generated(project: &CodeProject) -> KeyedReconcileState {
        KeyedReconcileState::empty()
            .plan(
                crate::required_generated_members(project).expect("member inventory"),
                &BTreeSet::new(),
            )
            .expect("member reconciliation")
            .into_staged()
    }

    fn expanded(
        project: &CodeProject,
        generated: &KeyedReconcileState,
        seed: u128,
    ) -> ExpandedCodeProject {
        let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).expect("intent");
        crate::expand_code_project(project, generated, intent.identity()).expect("V3 expansion")
    }

    fn session(key: &str) -> SketchCodeSession {
        let key = ProjectKey(key.into());
        let project = project(&key, "base");
        let generated = generated(&project);
        let expansion = expanded(&project, &generated, 0x90_5001);
        SketchCodeSession::new_project(
            project,
            generated,
            expansion,
            serde_json::json!({"accepted": "base"}),
        )
        .expect("V3 session")
    }

    fn prepare_fixture_edit(
        session: &SketchCodeSession,
        fixture: &str,
        seed: u128,
        checkpoint: serde_json::Value,
    ) -> PreparedCodeEdit {
        let candidate = project(&session.snapshot().project, fixture);
        let plan = session
            .plan_structural_reconciliation(
                session.identity(),
                crate::required_generated_members(&candidate).expect("candidate members"),
                &BTreeSet::new(),
            )
            .expect("candidate reconciliation");
        let expansion = expanded(&candidate, plan.staged(), seed);
        session
            .prepare_project_edit_from_plan(
                session.identity(),
                candidate,
                plan,
                expansion,
                checkpoint,
                "Compile V3 source",
            )
            .expect("prepared V3 edit")
    }

    #[test]
    fn complete_v3_edit_is_exact_cas_and_one_history_entry() {
        let mut session = session("exact-cas");
        let prepared = prepare_fixture_edit(
            &session,
            "restored",
            0x90_5002,
            serde_json::json!({"accepted": "restored"}),
        );
        let published = session.apply_prepared_audited(prepared.clone());
        published.outcome.expect("publication");
        assert_eq!(published.work.accepted_publications(), 1);
        assert!(session.can_undo());
        assert!(matches!(
            session.apply_prepared(prepared),
            Err(CodeSessionError::StaleSession { .. })
        ));
    }

    #[test]
    fn pointer_frames_are_compiler_and_expansion_free() {
        let session = session("pointer-frame");
        for _ in 0..1_000 {
            assert_eq!(
                session.pointer_frame_checkpoint(),
                &serde_json::json!({"accepted": "base"})
            );
        }
        assert_eq!(session.structural_expansions(), 0);
    }

    #[test]
    fn managed_v3_instance_overlay_is_exact_cas_history_and_persistence_authority() {
        let mut session = session("instance-overlay");
        let source_authority = session.snapshot().managed.clone();
        let project_authority = session.snapshot().code_project.clone();
        let generated = session.snapshot().generated.clone();
        let accepted = session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .expect("accepted V3 expansion")
            .clone();
        let point = accepted
            .writable_points
            .first()
            .expect("managed fixture exposes a writable instance point")
            .clone();

        assert!(session.stage_point_drag(&point, [f64::NAN, 2.0]).is_err());
        let mut foreign = point.clone();
        match &mut foreign.edit {
            crate::CodePointEdit::Point { address } => {
                address.project = ProjectKey("foreign-project".into());
            }
            crate::CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                ..
            } => {
                lower_left.project = ProjectKey("foreign-project".into());
                upper_right.project = ProjectKey("foreign-project".into());
            }
        }
        assert!(matches!(
            session.stage_point_drag(&foreign, [3.0, 2.0]),
            Err(CodeSessionError::UnknownSemanticOwner(_))
        ));

        let overlay = session
            .stage_point_drag(&point, [3.0, 2.0])
            .expect("finite authenticated point overlay");
        assert!(!overlay.drafts().is_empty());
        let expansion = crate::expand_code_project_with_overlay(
            session
                .snapshot()
                .code_project
                .as_ref()
                .expect("complete V3 project"),
            &generated,
            &overlay,
            accepted.patch.expected,
        )
        .expect("instance-overlay expansion");
        let prepared = session
            .prepare_project_overlay(
                session.identity(),
                overlay.clone(),
                expansion,
                serde_json::json!({"accepted": "instance-overlay"}),
                "Move managed instance point",
            )
            .expect("prepared overlay publication");
        let stale = prepared.clone();
        let published = session.apply_prepared_audited(prepared);
        published.outcome.expect("overlay publication");
        assert_eq!(published.work.accepted_publications(), 1);
        assert_eq!(session.snapshot().managed, source_authority);
        assert_eq!(session.snapshot().code_project, project_authority);
        assert_eq!(session.snapshot().interaction_overlay, overlay);
        assert_eq!(
            session.snapshot().interaction_overlay,
            session.snapshot().accepted_interaction_overlay
        );
        assert!(matches!(
            session.apply_prepared(stale),
            Err(CodeSessionError::StaleSession { .. })
        ));

        let published_json = session.to_canonical_json().expect("published session wire");
        let restored =
            SketchCodeSession::from_json(&published_json).expect("overlay session replay");
        assert_eq!(
            restored.to_canonical_json().expect("replayed session wire"),
            published_json
        );

        session.undo().expect("overlay Undo").expect("Undo receipt");
        assert_eq!(
            session.snapshot().interaction_overlay,
            CodeInteractionOverlay::empty()
        );
        assert_eq!(session.snapshot().managed, source_authority);
        session.redo().expect("overlay Redo").expect("Redo receipt");
        assert_eq!(session.snapshot().interaction_overlay, overlay);
        assert_eq!(session.snapshot().managed, source_authority);
    }

    #[test]
    fn complete_v3_history_round_trips_and_authenticates_host_checkpoints() {
        let mut session = session("persistence");
        let prepared = prepare_fixture_edit(
            &session,
            "restored",
            0x90_5003,
            serde_json::json!({"accepted": "restored"}),
        );
        session.apply_prepared(prepared).expect("publication");
        let canonical = session.to_canonical_json().expect("canonical session");
        let restored =
            SketchCodeSession::from_json_validating_checkpoints(&canonical, |checkpoint| {
                checkpoint
                    .get("accepted")
                    .and_then(serde_json::Value::as_str)
                    .map(|_| ())
                    .ok_or("missing accepted marker")
            })
            .expect("authenticated restore");
        assert_eq!(
            restored.to_canonical_json().expect("canonical restore"),
            canonical
        );

        let mut hostile: serde_json::Value = serde_json::from_str(&canonical).expect("wire");
        hostile["snapshot"]["accepted_editor_checkpoint"] = serde_json::json!({"hostile": true});
        assert!(matches!(
            SketchCodeSession::from_json_validating_checkpoints(
                &serde_json::to_string(&hostile).expect("hostile wire"),
                |checkpoint| checkpoint
                    .get("accepted")
                    .and_then(serde_json::Value::as_str)
                    .map(|_| ())
                    .ok_or("missing accepted marker")
            ),
            Err(CodeSessionError::InvalidPersistence(_))
        ));
    }

    #[test]
    fn retained_v3_failure_preserves_complete_previous_authority() {
        let mut session = session("retained-failure");
        let candidate = project(&session.snapshot().project, "restored");
        let plan = session
            .plan_structural_reconciliation(
                session.identity(),
                crate::required_generated_members(&candidate).expect("candidate members"),
                &BTreeSet::new(),
            )
            .expect("candidate reconciliation");
        let expansion = expanded(&candidate, plan.staged(), 0x90_5004);
        let accepted_before = session.snapshot().accepted_code_project.clone();
        let prepared = session
            .prepare_project_retained_failure(
                session.identity(),
                candidate,
                Some(plan),
                CodeInteractionOverlay::empty(),
                Some(expansion),
                serde_json::json!({"attempted": "restored"}),
                "native validation",
                "candidate rejected",
                "Retain rejected V3 candidate",
            )
            .expect("retained failure");
        session
            .apply_prepared(prepared)
            .expect("failure publication");
        assert_eq!(session.snapshot().accepted_code_project, accepted_before);
        assert_eq!(
            session.pointer_frame_checkpoint(),
            &serde_json::json!({"accepted": "base"})
        );
        assert!(session.snapshot().failure.is_some());
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSessionFailure {
    pub stage: String,
    pub diagnostic: String,
    pub attempted_source_digest: String,
}

/// One authoritative code checkpoint plus the one nested headless-editor
/// checkpoint. The editor's own Undo stack is not duplicated here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSessionSnapshot {
    pub project: ProjectKey,
    pub managed: ManagedDocument,
    /// Complete current offline code authority. Clean-break sessions always
    /// carry this field; the optional wire shape lets validation reject
    /// incomplete or hostile persisted transactions explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_project: Option<CodeProject>,
    /// Last code project whose nested editor checkpoint independently
    /// materialized and validated. This may differ from `code_project` while
    /// a valid-but-failed code attempt is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_code_project: Option<CodeProject>,
    /// Current equation-free expansion/provenance. A parseable project may
    /// retain `None` when artifact validation or expansion itself failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion: Option<ExpandedCodeProject>,
    /// Expansion corresponding exactly to the last accepted nested editor
    /// checkpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_expansion: Option<ExpandedCodeProject>,
    /// Keyed ledger which owns `accepted_expansion`. It is separate from the
    /// current ledger while a structural candidate is retained as failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_generated: Option<KeyedReconcileState>,
    /// Current semantic GUI placement layered over code-authored seeds.
    #[serde(default)]
    pub interaction_overlay: CodeInteractionOverlay,
    /// Overlay corresponding exactly to the accepted expansion/editor pair.
    #[serde(default)]
    pub accepted_interaction_overlay: CodeInteractionOverlay,
    pub accepted_source_digest: String,
    pub artifact_digests: BTreeMap<String, String>,
    pub generated: KeyedReconcileState,
    pub editor_checkpoint: serde_json::Value,
    pub accepted_editor_checkpoint: serde_json::Value,
    pub failure: Option<CodeSessionFailure>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCodeEdit {
    expected: CodeSessionIdentity,
    next: CodeSessionSnapshot,
    plan: Option<KeyedReconcilePlan>,
    label: String,
}

impl PreparedCodeEdit {
    pub fn expected(&self) -> &CodeSessionIdentity {
        &self.expected
    }

    pub fn next(&self) -> &CodeSessionSnapshot {
        &self.next
    }

    pub fn reconcile_plan(&self) -> Option<&KeyedReconcilePlan> {
        self.plan.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSessionReceipt {
    pub before: CodeSessionIdentity,
    pub after: CodeSessionIdentity,
    pub label: String,
    pub retained_failure: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryEntry {
    snapshot: CodeSessionSnapshot,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionWire {
    version: String,
    identity: CodeSessionIdentity,
    snapshot: CodeSessionSnapshot,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    structural_expansions: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SketchCodeSession {
    identity: CodeSessionIdentity,
    snapshot: CodeSessionSnapshot,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    structural_expansions: u64,
}

impl SketchCodeSession {
    /// Initializes one complete, already cold-materialized code project
    /// without manufacturing a user-visible history entry.
    ///
    /// The supplied expansion must be the exact deterministic expansion of
    /// `project` under `generated`, and the opaque editor checkpoint must
    /// contain the independently accepted native authority produced from it.
    pub fn new_project(
        project: CodeProject,
        generated: KeyedReconcileState,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
    ) -> Result<Self, CodeSessionError> {
        project.validate().map_err(|error| {
            CodeSessionError::InvalidPersistence(format!("invalid code project: {error}"))
        })?;
        validate_expansion(
            &project,
            &generated,
            &CodeInteractionOverlay::empty(),
            &expansion,
        )?;
        let artifact_digests = artifact_digests(&project)?;
        let accepted_source_digest = project.managed.source_digest.clone();
        let snapshot = CodeSessionSnapshot {
            project: project.project.clone(),
            managed: project.managed.clone(),
            code_project: Some(project.clone()),
            accepted_code_project: Some(project),
            expansion: Some(expansion.clone()),
            accepted_expansion: Some(expansion),
            accepted_generated: Some(generated.clone()),
            interaction_overlay: CodeInteractionOverlay::empty(),
            accepted_interaction_overlay: CodeInteractionOverlay::empty(),
            accepted_source_digest,
            artifact_digests,
            generated,
            editor_checkpoint: editor_checkpoint.clone(),
            accepted_editor_checkpoint: editor_checkpoint,
            failure: None,
        };
        let session = allocate_session()?;
        let identity = identity(session, 0, &snapshot, &[], &[], 0)?;
        let result = Self {
            identity,
            snapshot,
            undo: Vec::new(),
            redo: Vec::new(),
            structural_expansions: 0,
        };
        validate_snapshot(&result.snapshot)?;
        validate_wire_size(
            &result.identity,
            &result.snapshot,
            &result.undo,
            &result.redo,
            result.structural_expansions,
        )?;
        Ok(result)
    }

    pub fn identity(&self) -> &CodeSessionIdentity {
        &self.identity
    }

    pub fn snapshot(&self) -> &CodeSessionSnapshot {
        &self.snapshot
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub const fn structural_expansions(&self) -> u64 {
        self.structural_expansions
    }

    /// Pointer frames consume only the already accepted nested checkpoint.
    /// No compiler execution or custom-patch expansion is reachable here.
    pub fn pointer_frame_checkpoint(&self) -> &serde_json::Value {
        &self.snapshot.accepted_editor_checkpoint
    }

    /// Stages keyed membership first so callers can expand and cold-validate
    /// against the exact candidate identities before preparing one composite
    /// code/editor transaction.
    pub fn plan_structural_reconciliation(
        &self,
        expected: &CodeSessionIdentity,
        desired_members: Vec<GeneratedMemberAddress>,
        outside_dependents: &BTreeSet<GeneratedMemberIdentity>,
    ) -> Result<KeyedReconcilePlan, CodeSessionError> {
        self.authenticate(expected)?;
        Ok(self
            .snapshot
            .generated
            .plan(desired_members, outside_dependents)?)
    }

    /// Stages one complete code-project/native-editor publication.
    ///
    /// Unlike the lower-level reconciliation helper, this owns every custom
    /// file, artifact byte, lock pin, parsed program, expansion/provenance row,
    /// override and the complete delegated editor checkpoint in the same
    /// transaction and therefore in the same Undo entry.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_edit_from_plan(
        &self,
        expected: &CodeSessionIdentity,
        project: CodeProject,
        plan: KeyedReconcilePlan,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        let overlay_free = expand_code_project_with_overlay_and_retained_operation_plans(
            &project,
            plan.staged(),
            &CodeInteractionOverlay::empty(),
            expansion.patch.expected,
            &expansion.operation_plans,
        )
        .map_err(|error| {
            CodeSessionError::InvalidPersistence(format!(
                "structural overlay preflight cannot be reconstructed: {error}"
            ))
        })?;
        let retained = overlay_free.retained_overlay(&self.snapshot.accepted_interaction_overlay);
        self.prepare_project_edit_from_plan_with_overlay(
            expected,
            project,
            plan,
            retained,
            expansion,
            editor_checkpoint,
            label,
        )
    }

    /// Stages one structural project edit with its exact retained/pruned
    /// semantic overlay. Removing a structural owner can therefore prune its
    /// drafts in the same history entry, while persisted stale payloads remain
    /// strict under ordinary snapshot validation.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_edit_from_plan_with_overlay(
        &self,
        expected: &CodeSessionIdentity,
        project: CodeProject,
        plan: KeyedReconcilePlan,
        overlay: CodeInteractionOverlay,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        self.authenticate(expected)?;
        if plan.expected_identity() != self.snapshot.generated.identity() {
            return Err(KeyedReconcileError::StalePlan.into());
        }
        project.validate().map_err(|error| {
            CodeSessionError::InvalidPersistence(format!("invalid code project: {error}"))
        })?;
        overlay.validate()?;
        let overlay_free = expand_code_project_with_overlay_and_retained_operation_plans(
            &project,
            plan.staged(),
            &CodeInteractionOverlay::empty(),
            expansion.patch.expected,
            &expansion.operation_plans,
        )
        .map_err(|error| {
            CodeSessionError::InvalidPersistence(format!(
                "structural overlay preflight cannot be reconstructed: {error}"
            ))
        })?;
        let expected_overlay =
            overlay_free.retained_overlay(&self.snapshot.accepted_interaction_overlay);
        if overlay != expected_overlay {
            return Err(CodeSessionError::InvalidPersistence(
                "structural publication overlay is not the deterministic retained projection"
                    .into(),
            ));
        }
        validate_expansion(&project, plan.staged(), &overlay, &expansion)?;
        let artifact_digests = artifact_digests(&project)?;
        let mut next = self.snapshot.clone();
        next.project = project.project.clone();
        next.managed = project.managed.clone();
        next.code_project = Some(project.clone());
        next.accepted_code_project = Some(project);
        next.expansion = Some(expansion.clone());
        next.accepted_expansion = Some(expansion);
        next.accepted_generated = Some(plan.staged().clone());
        next.interaction_overlay = overlay.clone();
        next.accepted_interaction_overlay = overlay;
        next.accepted_source_digest
            .clone_from(&next.managed.source_digest);
        next.artifact_digests = artifact_digests;
        next.generated = plan.staged().clone();
        next.editor_checkpoint = editor_checkpoint.clone();
        next.accepted_editor_checkpoint = editor_checkpoint;
        next.failure = None;
        validate_snapshot(&next)?;
        Ok(PreparedCodeEdit {
            expected: expected.clone(),
            next,
            plan: Some(plan),
            label: label.into(),
        })
    }

    /// Retains a fully parsed code project and optional expansion over the
    /// previous independently accepted project/editor scene.
    ///
    /// `editor_checkpoint` may itself contain newer retained-invalid intent;
    /// the nested projectional editor remains responsible for presenting its
    /// authenticated previous accepted scene. The accepted project and
    /// expansion in this wrapper never advance on this path.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_retained_failure(
        &self,
        expected: &CodeSessionIdentity,
        project: CodeProject,
        plan: Option<KeyedReconcilePlan>,
        interaction_overlay: CodeInteractionOverlay,
        expansion: Option<ExpandedCodeProject>,
        editor_checkpoint: serde_json::Value,
        stage: impl Into<String>,
        diagnostic: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        self.authenticate(expected)?;
        project.validate().map_err(|error| {
            CodeSessionError::InvalidPersistence(format!("invalid code project: {error}"))
        })?;
        let generated = if let Some(plan) = &plan {
            if plan.expected_identity() != self.snapshot.generated.identity() {
                return Err(KeyedReconcileError::StalePlan.into());
            }
            plan.staged().clone()
        } else {
            self.snapshot.generated.clone()
        };
        interaction_overlay.validate()?;
        if let Some(expansion) = &expansion {
            let overlay_free = expand_code_project_with_overlay_and_retained_operation_plans(
                &project,
                &generated,
                &CodeInteractionOverlay::empty(),
                expansion.patch.expected,
                &expansion.operation_plans,
            )
            .map_err(|error| {
                CodeSessionError::InvalidPersistence(format!(
                    "retained-failure overlay preflight cannot be reconstructed: {error}"
                ))
            })?;
            let expected_overlay =
                overlay_free.retained_overlay(&self.snapshot.accepted_interaction_overlay);
            if interaction_overlay != expected_overlay {
                return Err(CodeSessionError::InvalidPersistence(
                    "retained-failure overlay is not the deterministic owner-pruned projection"
                        .into(),
                ));
            }
            validate_expansion(&project, &generated, &interaction_overlay, expansion)?;
        }
        let artifact_digests = artifact_digests(&project)?;
        let mut next = self.snapshot.clone();
        next.project = project.project.clone();
        next.managed = project.managed.clone();
        next.code_project = Some(project);
        next.expansion = expansion;
        next.generated = generated;
        next.interaction_overlay = interaction_overlay;
        next.artifact_digests = artifact_digests;
        next.editor_checkpoint = editor_checkpoint;
        next.failure = Some(CodeSessionFailure {
            stage: stage.into(),
            diagnostic: diagnostic.into(),
            attempted_source_digest: next.managed.source_digest.clone(),
        });
        validate_snapshot(&next)?;
        Ok(PreparedCodeEdit {
            expected: expected.clone(),
            next,
            plan,
            label: label.into(),
        })
    }

    /// Publishes one already accepted action from the delegated headless
    /// editor without manufacturing or mirroring an inner Undo entry.
    ///
    /// A retained code failure, when present, remains current. Only the
    /// accepted nested checkpoint advances, so pointer frames immediately use
    /// the newly accepted native authority while the failed code attempt and
    /// its diagnostic remain available for repair or Undo.
    pub fn prepare_delegated_editor_publication(
        &self,
        expected: &CodeSessionIdentity,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        self.authenticate(expected)?;
        let mut next = self.snapshot.clone();
        if next.failure.is_none() {
            next.editor_checkpoint = editor_checkpoint.clone();
        }
        next.accepted_editor_checkpoint = editor_checkpoint;
        validate_snapshot(&next)?;
        Ok(PreparedCodeEdit {
            expected: expected.clone(),
            next,
            plan: None,
            label: label.into(),
        })
    }

    /// Stages an override together with its exact replacement expansion and
    /// independently accepted delegated editor checkpoint.
    ///
    /// This is the complete-project path for a geometry-effective terminal
    /// placement. Mutating only the override ledger is deliberately rejected
    /// for complete projects because that could expose stale expansion/editor
    /// authority between frames.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_override(
        &self,
        expected: &CodeSessionIdentity,
        address: &GeneratedMemberAddress,
        value: ManagedValue,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        self.authenticate(expected)?;
        let project = self.clean_current_project()?;
        let mut generated = self.snapshot.generated.clone();
        generated.set_override(address, value)?;
        self.prepare_generated_publication(
            expected,
            project,
            generated,
            expansion,
            editor_checkpoint,
            label,
        )
    }

    /// Computes the bounded semantic overlay produced by one visible point
    /// drag without parsing, expanding or touching accepted authority.
    pub fn stage_point_drag(
        &self,
        point: &ExpandedWritablePoint,
        target: [f64; 2],
    ) -> Result<CodeInteractionOverlay, CodeSessionError> {
        let point = self.authenticate_writable_point(point)?;
        Ok(point.stage_drag(&self.snapshot.interaction_overlay, target)?)
    }

    /// Computes one atomic terminal bundle over several semantic point edit
    /// lenses. Equal duplicate seeds collapse and contradictory same-tier
    /// seeds reject without changing session authority.
    pub fn stage_point_drags<'a>(
        &self,
        drags: impl IntoIterator<Item = (&'a ExpandedWritablePoint, [f64; 2])>,
    ) -> Result<CodeInteractionOverlay, CodeSessionError> {
        let drags = drags
            .into_iter()
            .map(|(point, target)| Ok((self.authenticate_writable_point(point)?, target)))
            .collect::<Result<Vec<_>, CodeSessionError>>()?;
        Ok(stage_point_drags(
            &self.snapshot.interaction_overlay,
            drags,
        )?)
    }

    /// Stages reversible suppression for one exact generated host child.
    pub fn stage_generated_child_suppression(
        &self,
        address: &CodeGeneratedChildAddress,
        suppressed: bool,
    ) -> Result<CodeInteractionOverlay, CodeSessionError> {
        if self.snapshot.failure.is_some() {
            return Err(CodeSessionError::RetainedFailureActive);
        }
        let known = self
            .snapshot
            .accepted_expansion
            .as_ref()
            .and_then(|expansion| {
                expansion
                    .generated_children
                    .iter()
                    .find(|child| child.address == *address)
            })
            .ok_or_else(|| CodeSessionError::UnknownSemanticOwner(address.display_path()))?;
        let mut overlay = self.snapshot.interaction_overlay.clone();
        overlay.set_generated_child_suppressed(known.address.clone(), suppressed)?;
        Ok(overlay)
    }

    fn authenticate_writable_point<'a>(
        &'a self,
        candidate: &ExpandedWritablePoint,
    ) -> Result<&'a ExpandedWritablePoint, CodeSessionError> {
        if self.snapshot.failure.is_some() {
            return Err(CodeSessionError::RetainedFailureActive);
        }
        self.snapshot
            .accepted_expansion
            .as_ref()
            .and_then(|expansion| {
                expansion
                    .writable_points
                    .iter()
                    .find(|known| *known == candidate)
            })
            .ok_or_else(|| {
                CodeSessionError::UnknownSemanticOwner(format!(
                    "{}:{:?}",
                    candidate.handle.alias, candidate.handle.selector
                ))
            })
    }

    /// Stages one complete semantic overlay/native-editor publication.
    /// Expansion and the delegated checkpoint must already describe this
    /// exact overlay; publication enters the outer history once.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_overlay(
        &self,
        expected: &CodeSessionIdentity,
        overlay: CodeInteractionOverlay,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        self.authenticate(expected)?;
        let project = self.clean_current_project()?;
        overlay.validate()?;
        validate_expansion(project, &self.snapshot.generated, &overlay, &expansion)?;
        let mut next = self.snapshot.clone();
        next.interaction_overlay = overlay.clone();
        next.accepted_interaction_overlay = overlay;
        next.expansion = Some(expansion.clone());
        next.accepted_expansion = Some(expansion);
        next.accepted_generated = Some(next.generated.clone());
        next.accepted_code_project = Some(project.clone());
        next.editor_checkpoint = editor_checkpoint.clone();
        next.accepted_editor_checkpoint = editor_checkpoint;
        next.failure = None;
        validate_snapshot(&next)?;
        Ok(PreparedCodeEdit {
            expected: expected.clone(),
            next,
            plan: None,
            label: label.into(),
        })
    }

    /// Stages reset-to-code for one semantic draft. `None` means no draft was
    /// active and therefore no history entry is due.
    pub fn prepare_project_reset_draft(
        &self,
        expected: &CodeSessionIdentity,
        address: &crate::CodeWritableAddress,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<Option<PreparedCodeEdit>, CodeSessionError> {
        self.authenticate(expected)?;
        let edit = self
            .snapshot
            .accepted_expansion
            .as_ref()
            .and_then(|expansion| {
                expansion.writable_points.iter().find_map(|point| {
                    point
                        .edit
                        .writable_addresses()
                        .contains(address)
                        .then(|| point.edit.clone())
                })
            })
            .ok_or_else(|| CodeSessionError::UnknownSemanticOwner(address.display_path()))?;
        let mut overlay = self.snapshot.interaction_overlay.clone();
        if overlay.reset_many(edit.writable_addresses()) == 0 {
            return Ok(None);
        }
        self.prepare_project_overlay(expected, overlay, expansion, editor_checkpoint, label)
            .map(Some)
    }

    /// Removes one generated-child suppression override. `None` means the
    /// child already follows its generated definition.
    pub fn prepare_project_reset_generated_child_suppression(
        &self,
        expected: &CodeSessionIdentity,
        address: &CodeGeneratedChildAddress,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<Option<PreparedCodeEdit>, CodeSessionError> {
        self.authenticate(expected)?;
        let mut overlay = self.snapshot.interaction_overlay.clone();
        if !overlay.reset_generated_child_suppression(address) {
            return Ok(None);
        }
        self.prepare_project_overlay(expected, overlay, expansion, editor_checkpoint, label)
            .map(Some)
    }

    /// Stages Reset-to-code together with its exact replacement expansion and
    /// independently accepted delegated editor checkpoint. `None` means the
    /// requested address was already code-owned and no history entry is due.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_project_reset_to_code(
        &self,
        expected: &CodeSessionIdentity,
        address: &GeneratedMemberAddress,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<Option<PreparedCodeEdit>, CodeSessionError> {
        self.authenticate(expected)?;
        let project = self.clean_current_project()?;
        let mut generated = self.snapshot.generated.clone();
        if !generated.reset_to_code(address)? {
            return Ok(None);
        }
        self.prepare_generated_publication(
            expected,
            project,
            generated,
            expansion,
            editor_checkpoint,
            label,
        )
        .map(Some)
    }

    pub fn apply_prepared(
        &mut self,
        prepared: PreparedCodeEdit,
    ) -> Result<CodeSessionReceipt, CodeSessionError> {
        self.apply_prepared_audited(prepared).into_outcome()
    }

    pub fn apply_prepared_audited(
        &mut self,
        prepared: PreparedCodeEdit,
    ) -> AuditedCodeWork<Result<CodeSessionReceipt, CodeSessionError>> {
        let mut work = CodeWorkReceipt::default();
        let outcome = self.apply_prepared_with_work(prepared, &mut work);
        AuditedCodeWork::new(outcome, work)
    }

    fn apply_prepared_with_work(
        &mut self,
        prepared: PreparedCodeEdit,
        work: &mut CodeWorkReceipt,
    ) -> Result<CodeSessionReceipt, CodeSessionError> {
        self.authenticate(&prepared.expected)?;
        validate_snapshot(&prepared.next)?;
        if prepared.next.managed.declaration_name_high_water
            < self.snapshot.managed.declaration_name_high_water
        {
            return Err(CodeSessionError::InvalidPersistence(
                "prepared edit forgets declaration-name allocator authority".into(),
            ));
        }
        if !prepared
            .next
            .generated
            .retains_allocator_authority_from(&self.snapshot.generated)
        {
            return Err(CodeSessionError::InvalidPersistence(
                "prepared edit forgets generated allocator authority".into(),
            ));
        }
        let revision = next_revision(self.identity.revision)?;
        let before = self.identity.clone();
        let mut undo = self.undo.clone();
        push_bounded(
            &mut undo,
            HistoryEntry {
                snapshot: self.snapshot.clone(),
                label: prepared.label.clone(),
            },
        );
        let redo = Vec::new();
        let structural_expansions = if prepared.plan.is_some() {
            self.structural_expansions.saturating_add(1)
        } else {
            self.structural_expansions
        };
        let next_identity = identity(
            self.identity.session,
            revision,
            &prepared.next,
            &undo,
            &redo,
            structural_expansions,
        )?;
        validate_wire_size(
            &next_identity,
            &prepared.next,
            &undo,
            &redo,
            structural_expansions,
        )?;
        self.undo = undo;
        self.redo = redo;
        self.snapshot = prepared.next;
        self.identity = next_identity;
        self.structural_expansions = structural_expansions;
        work.record_accepted_publication();
        Ok(CodeSessionReceipt {
            before,
            after: self.identity.clone(),
            label: prepared.label,
            retained_failure: self.snapshot.failure.is_some(),
        })
    }

    pub fn undo(&mut self) -> Result<Option<CodeSessionReceipt>, CodeSessionError> {
        let Some(entry) = self.undo.last().cloned() else {
            return Ok(None);
        };
        let revision = next_revision(self.identity.revision)?;
        let before = self.identity.clone();
        let current = self.snapshot.clone();
        let restored = restore_snapshot(&current, entry.snapshot)?;
        let mut undo = self.undo.clone();
        undo.pop();
        let mut redo = self.redo.clone();
        push_bounded(
            &mut redo,
            HistoryEntry {
                snapshot: current,
                label: entry.label.clone(),
            },
        );
        let next_identity = identity(
            self.identity.session,
            revision,
            &restored,
            &undo,
            &redo,
            self.structural_expansions,
        )?;
        validate_wire_size(
            &next_identity,
            &restored,
            &undo,
            &redo,
            self.structural_expansions,
        )?;
        self.undo = undo;
        self.redo = redo;
        self.snapshot = restored;
        self.identity = next_identity;
        Ok(Some(CodeSessionReceipt {
            before,
            after: self.identity.clone(),
            label: format!("Undo {}", entry.label),
            retained_failure: self.snapshot.failure.is_some(),
        }))
    }

    pub fn redo(&mut self) -> Result<Option<CodeSessionReceipt>, CodeSessionError> {
        let Some(entry) = self.redo.last().cloned() else {
            return Ok(None);
        };
        let revision = next_revision(self.identity.revision)?;
        let before = self.identity.clone();
        let current = self.snapshot.clone();
        let restored = restore_snapshot(&current, entry.snapshot)?;
        let mut redo = self.redo.clone();
        redo.pop();
        let mut undo = self.undo.clone();
        push_bounded(
            &mut undo,
            HistoryEntry {
                snapshot: current,
                label: entry.label.clone(),
            },
        );
        let next_identity = identity(
            self.identity.session,
            revision,
            &restored,
            &undo,
            &redo,
            self.structural_expansions,
        )?;
        validate_wire_size(
            &next_identity,
            &restored,
            &undo,
            &redo,
            self.structural_expansions,
        )?;
        self.undo = undo;
        self.redo = redo;
        self.snapshot = restored;
        self.identity = next_identity;
        Ok(Some(CodeSessionReceipt {
            before,
            after: self.identity.clone(),
            label: format!("Redo {}", entry.label),
            retained_failure: self.snapshot.failure.is_some(),
        }))
    }

    /// Serializes the complete composite history and opaque editor
    /// checkpoints. No editor-owned history is mirrored or replayed.
    pub fn to_canonical_json(&self) -> Result<String, CodeSessionError> {
        let wire = SessionWire {
            version: SESSION_WIRE_VERSION.into(),
            identity: self.identity.clone(),
            snapshot: self.snapshot.clone(),
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            structural_expansions: self.structural_expansions,
        };
        let json = serde_json::to_string(&wire)
            .map_err(|error| CodeSessionError::Serialization(error.to_string()))?;
        if json.len() > crate::CODE_PROJECT_LIMIT {
            return Err(CodeSessionError::ResourceLimit {
                actual: json.len(),
                limit: crate::CODE_PROJECT_LIMIT,
            });
        }
        Ok(json)
    }

    /// Validates a complete candidate before atomically constructing a
    /// session. Failed loads cannot partially replace an existing session.
    pub fn from_json(json: &str) -> Result<Self, CodeSessionError> {
        Self::from_json_validating_checkpoints(json, |_| Ok::<(), std::convert::Infallible>(()))
    }

    /// Validates a complete candidate plus every opaque delegated editor
    /// checkpoint before atomically constructing a session.
    ///
    /// The adjacent code crate cannot interpret a host editor's persistence
    /// schema. Workbench/RPC hosts therefore supply one validator which is
    /// applied to current, accepted, Undo and Redo checkpoints. No partially
    /// restored session is returned if any historical nested authority is
    /// corrupt.
    pub fn from_json_validating_checkpoints<E>(
        json: &str,
        mut validate_checkpoint: impl FnMut(&serde_json::Value) -> Result<(), E>,
    ) -> Result<Self, CodeSessionError>
    where
        E: std::fmt::Display,
    {
        if json.len() > crate::CODE_PROJECT_LIMIT {
            return Err(CodeSessionError::ResourceLimit {
                actual: json.len(),
                limit: crate::CODE_PROJECT_LIMIT,
            });
        }
        let wire: SessionWire = serde_json::from_str(json)
            .map_err(|error| CodeSessionError::Serialization(error.to_string()))?;
        if wire.version != SESSION_WIRE_VERSION {
            return Err(CodeSessionError::InvalidPersistence(
                "unsupported code-session version".into(),
            ));
        }
        validate_imported_identity(&wire.identity)?;
        if wire.structural_expansions > wire.identity.revision {
            return Err(CodeSessionError::InvalidPersistence(
                "structural expansion count exceeds session revision".into(),
            ));
        }
        if wire.undo.len() > MAX_HISTORY || wire.redo.len() > MAX_HISTORY {
            return Err(CodeSessionError::InvalidPersistence(
                "code-session history exceeds its bound".into(),
            ));
        }
        validate_snapshot(&wire.snapshot)?;
        validate_snapshot_editor_checkpoints(&wire.snapshot, &mut validate_checkpoint)?;
        for entry in wire.undo.iter().chain(&wire.redo) {
            validate_snapshot(&entry.snapshot)?;
            validate_snapshot_editor_checkpoints(&entry.snapshot, &mut validate_checkpoint)?;
            if wire.snapshot.managed.declaration_name_high_water
                < entry.snapshot.managed.declaration_name_high_water
            {
                return Err(CodeSessionError::InvalidPersistence(
                    "live checkpoint forgets declaration-name allocator history".into(),
                ));
            }
            if !wire
                .snapshot
                .generated
                .retains_allocator_authority_from(&entry.snapshot.generated)
            {
                return Err(CodeSessionError::InvalidPersistence(
                    "live checkpoint forgets generated allocator history".into(),
                ));
            }
        }
        validate_history_identity_consistency(&wire.snapshot, &wire.undo, &wire.redo)?;
        let expected = identity(
            wire.identity.session,
            wire.identity.revision,
            &wire.snapshot,
            &wire.undo,
            &wire.redo,
            wire.structural_expansions,
        )?;
        if expected != wire.identity {
            return Err(CodeSessionError::InvalidPersistence(
                "code-session identity does not authenticate its checkpoints".into(),
            ));
        }
        NEXT_SESSION.fetch_max(wire.identity.session.saturating_add(1), Ordering::Relaxed);
        Ok(Self {
            identity: wire.identity,
            snapshot: wire.snapshot,
            undo: wire.undo,
            redo: wire.redo,
            structural_expansions: wire.structural_expansions,
        })
    }

    fn authenticate(&self, expected: &CodeSessionIdentity) -> Result<(), CodeSessionError> {
        if expected == &self.identity {
            Ok(())
        } else {
            Err(CodeSessionError::StaleSession {
                expected: expected.clone(),
                actual: self.identity.clone(),
            })
        }
    }

    fn clean_current_project(&self) -> Result<&CodeProject, CodeSessionError> {
        if self.snapshot.failure.is_some() {
            return Err(CodeSessionError::RetainedFailureActive);
        }
        self.snapshot
            .code_project
            .as_ref()
            .ok_or(CodeSessionError::CompleteProjectRequired)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_generated_publication(
        &self,
        expected: &CodeSessionIdentity,
        project: &CodeProject,
        generated: KeyedReconcileState,
        expansion: ExpandedCodeProject,
        editor_checkpoint: serde_json::Value,
        label: impl Into<String>,
    ) -> Result<PreparedCodeEdit, CodeSessionError> {
        validate_expansion(
            project,
            &generated,
            &self.snapshot.interaction_overlay,
            &expansion,
        )?;
        let mut next = self.snapshot.clone();
        next.generated = generated.clone();
        next.accepted_generated = Some(generated);
        next.accepted_interaction_overlay = next.interaction_overlay.clone();
        next.expansion = Some(expansion.clone());
        next.accepted_expansion = Some(expansion);
        next.accepted_code_project = Some(project.clone());
        next.accepted_source_digest
            .clone_from(&project.managed.source_digest);
        next.editor_checkpoint = editor_checkpoint.clone();
        next.accepted_editor_checkpoint = editor_checkpoint;
        next.failure = None;
        validate_snapshot(&next)?;
        Ok(PreparedCodeEdit {
            expected: expected.clone(),
            next,
            plan: None,
            label: label.into(),
        })
    }
}

fn validate_snapshot_editor_checkpoints<E>(
    snapshot: &CodeSessionSnapshot,
    validate_checkpoint: &mut impl FnMut(&serde_json::Value) -> Result<(), E>,
) -> Result<(), CodeSessionError>
where
    E: std::fmt::Display,
{
    validate_checkpoint(&snapshot.editor_checkpoint).map_err(|error| {
        CodeSessionError::InvalidPersistence(format!(
            "invalid delegated editor checkpoint: {error}"
        ))
    })?;
    if snapshot.accepted_editor_checkpoint != snapshot.editor_checkpoint {
        validate_checkpoint(&snapshot.accepted_editor_checkpoint).map_err(|error| {
            CodeSessionError::InvalidPersistence(format!(
                "invalid accepted delegated editor checkpoint: {error}"
            ))
        })?;
    }
    Ok(())
}

fn push_bounded(history: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    if history.len() == MAX_HISTORY {
        history.remove(0);
    }
    history.push(entry);
}

fn restore_snapshot(
    current: &CodeSessionSnapshot,
    mut checkpoint: CodeSessionSnapshot,
) -> Result<CodeSessionSnapshot, CodeSessionError> {
    let declaration_name_high_water = current
        .managed
        .declaration_name_high_water
        .max(checkpoint.managed.declaration_name_high_water);
    checkpoint.managed.declaration_name_high_water = declaration_name_high_water;
    if let Some(project) = checkpoint.code_project.as_mut() {
        project.managed.declaration_name_high_water = declaration_name_high_water;
    }
    if let Some(project) = checkpoint.accepted_code_project.as_mut() {
        project.managed.declaration_name_high_water = declaration_name_high_water;
    }
    checkpoint.generated = current
        .generated
        .restored_checkpoint(&checkpoint.generated)?;
    if let Some(accepted) = checkpoint.accepted_generated.take() {
        checkpoint.accepted_generated = Some(checkpoint.generated.restored_checkpoint(&accepted)?);
    }
    validate_snapshot(&checkpoint)?;
    Ok(checkpoint)
}

fn validate_history_identity_consistency(
    snapshot: &CodeSessionSnapshot,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
) -> Result<(), CodeSessionError> {
    let mut by_allocation = BTreeMap::<u64, (GeneratedMemberAddress, u32)>::new();
    let mut by_generation = BTreeMap::<(GeneratedMemberAddress, u32), u64>::new();
    for snapshot in std::iter::once(snapshot)
        .chain(undo.iter().map(|entry| &entry.snapshot))
        .chain(redo.iter().map(|entry| &entry.snapshot))
    {
        record_generated_identities(&snapshot.generated, &mut by_allocation, &mut by_generation)?;
        if let Some(accepted) = &snapshot.accepted_generated {
            record_generated_identities(accepted, &mut by_allocation, &mut by_generation)?;
        }
    }
    Ok(())
}

fn record_generated_identities(
    generated: &KeyedReconcileState,
    by_allocation: &mut BTreeMap<u64, (GeneratedMemberAddress, u32)>,
    by_generation: &mut BTreeMap<(GeneratedMemberAddress, u32), u64>,
) -> Result<(), CodeSessionError> {
    for (address, identity) in generated.active().iter().chain(generated.tombstones()) {
        let semantic = (address.clone(), identity.generation);
        if by_allocation
            .insert(identity.allocation, semantic.clone())
            .is_some_and(|known| known != semantic)
            || by_generation
                .insert(semantic, identity.allocation)
                .is_some_and(|known| known != identity.allocation)
        {
            return Err(CodeSessionError::InvalidPersistence(
                "generated identity is reused inconsistently across history".into(),
            ));
        }
    }
    Ok(())
}

fn identity(
    session: u64,
    revision: u64,
    snapshot: &CodeSessionSnapshot,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
    structural_expansions: u64,
) -> Result<CodeSessionIdentity, CodeSessionError> {
    let canonical = serde_json::to_vec(&(snapshot, undo, redo, structural_expansions))
        .map_err(|error| CodeSessionError::Serialization(error.to_string()))?;
    Ok(CodeSessionIdentity {
        session,
        revision,
        digest: intent_content_digest(&canonical).to_string(),
    })
}

fn allocate_session() -> Result<u64, CodeSessionError> {
    NEXT_SESSION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            (next <= MAX_CODE_SESSION_WIRE_INTEGER)
                .then(|| next.checked_add(1))
                .flatten()
        })
        .map_err(|_| CodeSessionError::SessionIdentityExhausted)
}

fn next_revision(revision: u64) -> Result<u64, CodeSessionError> {
    revision
        .checked_add(1)
        .filter(|next| *next <= MAX_CODE_SESSION_WIRE_INTEGER)
        .ok_or(CodeSessionError::RevisionExhausted)
}

fn validate_imported_identity(identity: &CodeSessionIdentity) -> Result<(), CodeSessionError> {
    if identity.session == 0 || identity.session > MAX_CODE_SESSION_WIRE_INTEGER {
        Err(CodeSessionError::InvalidPersistence(
            "invalid code-session allocation".into(),
        ))
    } else if identity.revision > MAX_CODE_SESSION_WIRE_INTEGER {
        Err(CodeSessionError::InvalidPersistence(
            "invalid code-session revision".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_wire_size(
    identity: &CodeSessionIdentity,
    snapshot: &CodeSessionSnapshot,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
    structural_expansions: u64,
) -> Result<(), CodeSessionError> {
    validate_history_identity_consistency(snapshot, undo, redo)?;
    let wire = SessionWire {
        version: SESSION_WIRE_VERSION.into(),
        identity: identity.clone(),
        snapshot: snapshot.clone(),
        undo: undo.to_vec(),
        redo: redo.to_vec(),
        structural_expansions,
    };
    let actual = serde_json::to_vec(&wire)
        .map_err(|error| CodeSessionError::Serialization(error.to_string()))?
        .len();
    if actual > crate::CODE_PROJECT_LIMIT {
        Err(CodeSessionError::ResourceLimit {
            actual,
            limit: crate::CODE_PROJECT_LIMIT,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum CodeSessionError {
    #[error(transparent)]
    Reconcile(#[from] KeyedReconcileError),
    #[error(transparent)]
    Overlay(#[from] CodeOverlayError),
    #[error("stale code-session identity: expected {expected:?}, actual {actual:?}")]
    StaleSession {
        expected: CodeSessionIdentity,
        actual: CodeSessionIdentity,
    },
    #[error("code-session state cannot be encoded canonically: {0}")]
    Serialization(String),
    #[error("invalid persisted code session: {0}")]
    InvalidPersistence(String),
    #[error("this operation requires complete code-project authority")]
    CompleteProjectRequired,
    #[error("a retained code failure must be resolved or undone before this publication")]
    RetainedFailureActive,
    #[error("semantic interaction owner `{0}` is stale, unknown or not accepted")]
    UnknownSemanticOwner(String),
    #[error("code session is {actual} bytes; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("code-session identity allocator is exhausted")]
    SessionIdentityExhausted,
    #[error("code-session revision is exhausted")]
    RevisionExhausted,
}

#[allow(
    clippy::single_match_else,
    clippy::too_many_lines,
    reason = "one audit pass validates all current and accepted snapshot cross-links"
)]
fn validate_snapshot(snapshot: &CodeSessionSnapshot) -> Result<(), CodeSessionError> {
    if snapshot.managed.declaration_name_high_water > MAX_CODE_SESSION_WIRE_INTEGER {
        return Err(CodeSessionError::InvalidPersistence(
            "declaration-name high-water is outside the exact JavaScript integer range".into(),
        ));
    }
    if let Some(compiled) = snapshot.managed.compiled.as_deref() {
        compiled.validate().map_err(|error| {
            CodeSessionError::InvalidPersistence(format!(
                "invalid managed compiler authority: {error}"
            ))
        })?;
        let mut projected = compiled.projected_managed_document().map_err(|error| {
            CodeSessionError::InvalidPersistence(format!(
                "invalid managed equation-free projection: {error}"
            ))
        })?;
        projected.declaration_name_high_water = snapshot.managed.declaration_name_high_water;
        let mut managed = snapshot.managed.clone();
        managed.compiled = None;
        if projected != managed {
            return Err(CodeSessionError::InvalidPersistence(
                "managed document differs from its executed compiler authority".into(),
            ));
        }
    } else {
        return Err(CodeSessionError::InvalidPersistence(
            "code session lacks executed managed compiler authority".into(),
        ));
    }
    snapshot.generated.validate()?;
    snapshot.interaction_overlay.validate()?;
    snapshot.accepted_interaction_overlay.validate()?;
    if let Some(accepted) = &snapshot.accepted_generated {
        accepted.validate()?;
        if !snapshot
            .generated
            .retains_allocator_authority_from(accepted)
        {
            return Err(CodeSessionError::InvalidPersistence(
                "current generated ledger forgets accepted allocator authority".into(),
            ));
        }
    }
    if snapshot.project.0.is_empty()
        || snapshot.project.0.len() > 256
        || snapshot.project.0.chars().any(char::is_control)
    {
        return Err(CodeSessionError::InvalidPersistence(
            "invalid project key".into(),
        ));
    }
    for digest in snapshot.artifact_digests.values() {
        validate_digest(digest)?;
    }
    match &snapshot.code_project {
        Some(project) => {
            project.validate().map_err(|error| {
                CodeSessionError::InvalidPersistence(format!(
                    "invalid current code project: {error}"
                ))
            })?;
            if project.project != snapshot.project || project.managed != snapshot.managed {
                return Err(CodeSessionError::InvalidPersistence(
                    "current code project disagrees with its projected key/source".into(),
                ));
            }
            if artifact_digests(project)? != snapshot.artifact_digests {
                return Err(CodeSessionError::InvalidPersistence(
                    "current code project disagrees with artifact pins".into(),
                ));
            }
            if let Some(expansion) = &snapshot.expansion {
                validate_expansion(
                    project,
                    &snapshot.generated,
                    &snapshot.interaction_overlay,
                    expansion,
                )?;
            }
        }
        None => {
            return Err(CodeSessionError::InvalidPersistence(
                "code session lacks complete code-project authority".into(),
            ));
        }
    }
    match (
        &snapshot.accepted_code_project,
        &snapshot.accepted_expansion,
        &snapshot.accepted_generated,
    ) {
        (Some(project), Some(expansion), Some(generated)) => {
            project.validate().map_err(|error| {
                CodeSessionError::InvalidPersistence(format!(
                    "invalid accepted code project: {error}"
                ))
            })?;
            if project.project != snapshot.project {
                return Err(CodeSessionError::InvalidPersistence(
                    "accepted code project belongs to another project".into(),
                ));
            }
            if project.managed.compiled.is_none() || snapshot.managed.compiled.is_none() {
                return Err(CodeSessionError::InvalidPersistence(
                    "accepted and current code projects require compiled managed authority".into(),
                ));
            }
            validate_expansion(
                project,
                generated,
                &snapshot.accepted_interaction_overlay,
                expansion,
            )?;
            if snapshot.accepted_source_digest != project.managed.source_digest {
                return Err(CodeSessionError::InvalidPersistence(
                    "accepted source digest disagrees with accepted code project".into(),
                ));
            }
        }
        _ => {
            return Err(CodeSessionError::InvalidPersistence(
                "accepted project, expansion and generated ledger are incomplete".into(),
            ));
        }
    }
    if snapshot.failure.is_none()
        && snapshot.code_project.is_some()
        && (snapshot.expansion.is_none()
            || snapshot.code_project != snapshot.accepted_code_project
            || snapshot.expansion != snapshot.accepted_expansion
            || &snapshot.generated
                != snapshot
                    .accepted_generated
                    .as_ref()
                    .expect("complete authority was checked above")
            || snapshot.interaction_overlay != snapshot.accepted_interaction_overlay
            || snapshot.editor_checkpoint != snapshot.accepted_editor_checkpoint)
    {
        return Err(CodeSessionError::InvalidPersistence(
            "successful current and accepted project bundles disagree".into(),
        ));
    }
    if let Some(failure) = &snapshot.failure {
        if failure.attempted_source_digest != snapshot.managed.source_digest {
            return Err(CodeSessionError::InvalidPersistence(
                "retained failure does not authenticate attempted source".into(),
            ));
        }
    } else if snapshot.accepted_source_digest != snapshot.managed.source_digest {
        return Err(CodeSessionError::InvalidPersistence(
            "accepted source digest does not authenticate managed source".into(),
        ));
    }
    validate_digest(&snapshot.accepted_source_digest)?;
    Ok(())
}

fn artifact_digests(project: &CodeProject) -> Result<BTreeMap<String, String>, CodeSessionError> {
    let modules = project
        .lock
        .get("modules")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            CodeSessionError::InvalidPersistence("code project lock has no module pins".into())
        })?;
    modules
        .iter()
        .map(|(module, pin)| {
            let digest = pin
                .get("artifact")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CodeSessionError::InvalidPersistence(format!(
                        "code project module `{module}` has no artifact digest"
                    ))
                })?;
            validate_digest(digest)?;
            Ok((module.clone(), digest.to_owned()))
        })
        .collect()
}

fn validate_expansion(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    expansion: &ExpandedCodeProject,
) -> Result<(), CodeSessionError> {
    let recomputed = expand_code_project_with_overlay_and_retained_operation_plans(
        project,
        generated,
        overlay,
        expansion.patch.expected,
        &expansion.operation_plans,
    )
    .map_err(|error| {
        CodeSessionError::InvalidPersistence(format!(
            "code-project expansion cannot be reconstructed: {error}"
        ))
    })?;
    if &recomputed != expansion {
        return Err(CodeSessionError::InvalidPersistence(
            "code-project expansion/provenance is not canonical".into(),
        ));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), CodeSessionError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CodeSessionError::InvalidPersistence(format!(
            "invalid content digest `{value}`"
        )))
    }
}
