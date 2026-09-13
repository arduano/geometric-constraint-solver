// SPDX-License-Identifier: GPL-3.0-or-later
//! Managed session history delegates to `SketchCodeSession`; private checkpoint keys
//! retain immutable accepted native authorities without serializing solved geometry.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeProject, CodeSessionIdentity, KeyedReconcileState,
    MaterializedCodeProject, ProjectKey, SketchCodeSession,
    materialize_code_project_cold_with_overlay, materialize_code_project_incremental_with_overlay,
    required_generated_members,
};
use serde::{Deserialize, Serialize};

use crate::{
    AcceptedEvaluation, EngineAuthoringMode, EngineError, SketchEngine, deterministic_ids,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditableDesign {
    pub format: String,
    pub project: ProjectKey,
    pub generated: KeyedReconcileState,
    pub overrides: CodeInteractionOverlay,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditableSessionState {
    pub token: CodeSessionIdentity,
    pub can_undo: bool,
    pub can_redo: bool,
    pub result: crate::EngineAcceptedResult,
}

#[derive(Debug)]
pub struct EditableSession {
    pub(super) history: SketchCodeSession,
    pub(super) authorities: BTreeMap<String, AcceptedEvaluation>,
    pub(super) accepted: AcceptedEvaluation,
    pub(super) persistable_history: bool,
}

/// Independently replayed, unpublished point edit bound to one exact live session.
/// Persist the candidate source/design in the host transaction before installing it.
#[derive(Debug)]
pub struct PreparedPointGestureCommit {
    expected: CodeSessionIdentity,
    history: SketchCodeSession,
    accepted: AcceptedEvaluation,
    design: EditableDesign,
    source_design_digest: String,
}

impl PreparedPointGestureCommit {
    pub fn result(&self) -> &crate::EngineAcceptedResult {
        self.accepted.result()
    }
    pub fn design(&self) -> &EditableDesign {
        &self.design
    }
    pub fn source_design_digest(&self) -> &str {
        &self.source_design_digest
    }
}

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

impl EditableSession {
    pub(super) fn fork_for_preparation(&self) -> Self {
        Self {
            history: self.history.clone(),
            authorities: self.authorities.clone(),
            accepted: self.accepted.clone(),
            persistable_history: self.persistable_history,
        }
    }

    pub(super) fn install_prepared_session(
        &mut self,
        expected: &CodeSessionIdentity,
        candidate: Self,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.authenticate(expected)?;
        *self = candidate;
        Ok(self.accepted.clone())
    }

    pub(super) fn code_snapshot(&self) -> &geosolve_sketch_code::CodeSessionSnapshot {
        self.history.snapshot()
    }

    /// Opens authentic source and optional semantic overrides, with no persisted native checkpoint.
    ///
    /// # Errors
    /// Rejects invalid source, foreign/stale semantic overrides or unaccepted geometry.
    pub fn open(project_json: &str, design_json: Option<&str>) -> Result<Self, EngineError> {
        Self::open_with_checkpoint_policy(project_json, design_json, false)
    }

    /// Opens a source-authoring session whose complete history can be saved and restored.
    /// Hosts retain their outer storage, draft and publication policies.
    ///
    /// # Errors
    /// Rejects invalid source/design or independently unaccepted native geometry.
    pub fn open_persistable(
        project_json: &str,
        design_json: Option<&str>,
    ) -> Result<Self, EngineError> {
        Self::open_with_checkpoint_policy(project_json, design_json, true)
    }

    fn open_with_checkpoint_policy(
        project_json: &str,
        design_json: Option<&str>,
        persistable_history: bool,
    ) -> Result<Self, EngineError> {
        let project = CodeProject::from_json(project_json).map_err(error)?;
        let design = match design_json {
            Some(json) => {
                if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
                    return Err(error("design byte limit exceeded"));
                }
                let design: EditableDesign = serde_json::from_str(json).map_err(error)?;
                if design.format != "geosolve-design-v1" || design.project != project.project {
                    return Err(error("design format or project identity mismatch"));
                }
                design.overrides.validate().map_err(error)?;
                design
            }
            None => EditableDesign {
                format: "geosolve-design-v1".into(),
                project: project.project.clone(),
                generated: KeyedReconcileState::empty(),
                overrides: CodeInteractionOverlay::empty(),
            },
        };
        let generated = design
            .generated
            .plan(
                required_generated_members(&project).map_err(error)?,
                &BTreeSet::new(),
            )
            .map_err(error)?
            .into_staged();
        let digest = input_digest(&project, &generated, &design.overrides, None)?;
        let (intent, document) = deterministic_ids(digest.as_bytes());
        let materialized = materialize_code_project_cold_with_overlay(
            &project,
            &generated,
            &design.overrides,
            intent,
            document,
            1.0,
        )
        .map_err(error)?;
        let expansion = materialized.expansion.clone();
        let accepted = publication(&project, materialized, digest)?;
        let checkpoint = editor_history_checkpoint(&accepted, persistable_history)?;
        let history = SketchCodeSession::new_project_with_overlay(
            project,
            generated,
            design.overrides,
            expansion,
            checkpoint,
        )
        .map_err(error)?;
        let key = history_authority_key(history.snapshot())?;
        Ok(Self {
            history,
            authorities: BTreeMap::from([(key, accepted.clone())]),
            accepted,
            persistable_history,
        })
    }

    /// Restores the unchanged source-session history format, validating every native checkpoint.
    /// Historical entries are rematerialized lazily when visited; none are discarded.
    ///
    /// # Errors
    /// Rejects corrupt current/accepted/Undo/Redo authority, native geometry or source ownership.
    pub fn restore_history(json: &str) -> Result<Self, EngineError> {
        Self::from_validated_source_history(
            geosolve_sketch_code::ValidatedSourceHistory::decode(json).map_err(error)?,
        )
    }

    /// Serializes every source-history entry using the existing delegated native checkpoint format.
    ///
    /// # Errors
    /// Rejects an ephemeral session or output beyond the source-session wire limit.
    pub fn export_history(&self) -> Result<String, EngineError> {
        if !self.persistable_history {
            return Err(error("session has no persistable native checkpoints"));
        }
        self.history.to_canonical_json().map_err(error)
    }

    pub fn accepted(&self) -> &AcceptedEvaluation {
        &self.accepted
    }
    pub fn token(&self) -> &CodeSessionIdentity {
        self.history.identity()
    }
    pub fn state(&self) -> EditableSessionState {
        EditableSessionState {
            token: self.token().clone(),
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            result: self.accepted.result().clone(),
        }
    }
    pub fn design(&self) -> EditableDesign {
        EditableDesign {
            format: "geosolve-design-v1".into(),
            project: self.history.snapshot().project.clone(),
            generated: self.history.snapshot().generated.clone(),
            overrides: self.history.snapshot().interaction_overlay.clone(),
        }
    }

    /// Exports the complete accepted source project, including compiler authority,
    /// dependency artifacts and source allocation high-water state.
    ///
    /// # Errors
    /// Returns a missing-project or serialization failure without changing the session.
    pub fn export_project_json(&self) -> Result<String, EngineError> {
        self.code_snapshot()
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?
            .to_canonical_json()
            .map_err(error)
    }

    /// Process-independent identity of the complete accepted source/design input.
    /// Unlike result/input IDs for individual evaluations, this survives cold reconstruction.
    ///
    /// # Errors
    /// Returns a serialization failure without changing the accepted session.
    pub fn source_design_digest(&self) -> Result<String, EngineError> {
        source_design_digest(self.history.snapshot())
    }

    /// Replays a semantic gesture on server-owned native authority, certifies complete
    /// terminal parity and stages one overlay/history contribution without publication.
    ///
    /// # Errors
    /// Stale source/design, failed native replay, incomplete computed features or terminal
    /// parity mismatches retain all accepted source, design, geometry and history.
    pub fn prepare_point_gesture_commit(
        &self,
        command: &crate::PointGestureCommand,
    ) -> Result<PreparedPointGestureCommit, EngineError> {
        let terminal = self.replay_point_gesture(command)?;
        self.prepare_point_gesture_terminal(&terminal)
    }

    /// Trusted historical replay followed by uniquely addressed latest-model replay.
    /// The host separately proves returned declaration lifetime continuity.
    ///
    /// # Errors
    /// Rejects unauthenticated original gestures, changed source codecs/branches and
    /// infeasible latest terminals without changing either accepted session.
    pub fn prepare_point_gesture_replay(
        &self,
        basis: &Self,
        command: &crate::PointGestureCommand,
    ) -> Result<(PreparedPointGestureCommit, crate::PointReplayWitness), EngineError> {
        let (terminal, witness) = self.replay_latest_point_gesture(basis, command)?;
        Ok((self.prepare_point_gesture_terminal(&terminal)?, witness))
    }

    fn prepare_point_gesture_terminal(
        &self,
        terminal: &crate::PointGestureTerminal,
    ) -> Result<PreparedPointGestureCommit, EngineError> {
        let expected = self.token().clone();
        let snapshot = self.history.snapshot();
        let project = snapshot
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        let (materialized, overlay) = geosolve_sketch_code::editor_terminal::materialize_terminal(
            &self.accepted.0.materialized,
            project,
            &snapshot.generated,
            &snapshot.interaction_overlay,
            &terminal.lens,
            &terminal.editor,
            terminal.proposal.terminal_session(),
        )
        .map_err(error)?;
        let digest = input_digest(project, &snapshot.generated, &overlay, Some(&expected))?;
        let expansion = materialized.expansion.clone();
        let accepted = publication(project, materialized, digest)?;
        let prepared = self
            .history
            .prepare_project_overlay(
                &expected,
                overlay.clone(),
                expansion,
                editor_history_checkpoint(&accepted, self.persistable_history)?,
                "Move semantic point",
            )
            .map_err(error)?;
        let mut history = self.history.clone();
        history.apply_prepared(prepared).map_err(error)?;
        let source_design_digest = source_design_digest(history.snapshot())?;
        let design = EditableDesign {
            format: "geosolve-design-v1".into(),
            project: snapshot.project.clone(),
            generated: snapshot.generated.clone(),
            overrides: overlay,
        };
        Ok(PreparedPointGestureCommit {
            expected,
            history,
            accepted,
            design,
            source_design_digest,
        })
    }

    /// Installs a fully validated server candidate after the host's durable transaction.
    ///
    /// # Errors
    /// Rejects stale/foreign candidates without replacing accepted state or history.
    pub fn apply_point_gesture_commit(
        &mut self,
        prepared: PreparedPointGestureCommit,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.authenticate(&prepared.expected)?;
        self.install(prepared.history, prepared.accepted)
    }

    /// Synchronous server-owned replay and commit. The persistent host should instead
    /// stage, durably record and then install through the two-step API above.
    ///
    /// # Errors
    /// Returns preparation/publication errors, retaining the complete accepted state.
    pub fn commit_point_gesture(
        &mut self,
        command: &crate::PointGestureCommand,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let prepared = self.prepare_point_gesture_commit(command)?;
        self.apply_point_gesture_commit(prepared)
    }

    /// Applies a complete authenticated source/dependency project in one history entry.
    ///
    /// # Errors
    /// Rejects stale/cross-session tokens, foreign projects or failed candidates, preserving history.
    pub fn apply_project(
        &mut self,
        expected: &CodeSessionIdentity,
        project_json: &str,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.authenticate(expected)?;
        let project = CodeProject::from_json(project_json).map_err(error)?;
        if project.project != self.history.snapshot().project {
            return Err(error(
                "project update belongs to a different editable session project",
            ));
        }
        self.apply_project_for_host(project, "Apply project", false)?;
        Ok(self.accepted.clone())
    }

    /// Applies typed authored point/suppression overrides through shared expansion and native validation.
    ///
    /// # Errors
    /// Rejects stale tokens, stale semantic addresses, invalid branches or unaccepted geometry.
    pub fn apply_overlay(
        &mut self,
        expected: &CodeSessionIdentity,
        overlay: CodeInteractionOverlay,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.authenticate(expected)?;
        overlay.validate().map_err(error)?;
        let snapshot = self.history.snapshot();
        let project = snapshot
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        let materialized = materialize_code_project_incremental_with_overlay(
            &self.accepted.0.materialized,
            project,
            &snapshot.generated,
            &overlay,
        )
        .map_err(error)?;
        let digest = input_digest(project, &snapshot.generated, &overlay, Some(expected))?;
        let expansion = materialized.expansion.clone();
        let accepted = publication(project, materialized, digest)?;
        let prepared = self
            .history
            .prepare_project_overlay(
                expected,
                overlay,
                expansion,
                editor_history_checkpoint(&accepted, self.persistable_history)?,
                "Apply semantic overrides",
            )
            .map_err(error)?;
        let mut history = self.history.clone();
        history.apply_prepared(prepared).map_err(error)?;
        self.install(history, accepted)
    }

    /// # Errors
    /// Rejects stale/cross-session tokens or inconsistent private history authority.
    pub fn undo(
        &mut self,
        expected: &CodeSessionIdentity,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.step_history(expected, true)
    }

    /// # Errors
    /// Rejects stale/cross-session tokens or inconsistent private history authority.
    pub fn redo(
        &mut self,
        expected: &CodeSessionIdentity,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.step_history(expected, false)
    }

    fn step_history(
        &mut self,
        expected: &CodeSessionIdentity,
        undo: bool,
    ) -> Result<AcceptedEvaluation, EngineError> {
        self.authenticate(expected)?;
        self.step_source_history(undo)?;
        Ok(self.accepted.clone())
    }

    pub(super) fn authenticate(&self, expected: &CodeSessionIdentity) -> Result<(), EngineError> {
        if expected != self.token() {
            return Err(error("stale or foreign editable session token"));
        }
        Ok(())
    }

    pub(super) fn install(
        &mut self,
        history: SketchCodeSession,
        accepted: AcceptedEvaluation,
    ) -> Result<AcceptedEvaluation, EngineError> {
        // The source owner has already authenticated history and its wire bound.
        // Retain native handles directly from its read-only inventory; no JSON
        // roundtrip or knowledge of the private durable entry schema is needed.
        let retained = history
            .retained_snapshots()
            .map(history_authority_key)
            .collect::<Result<BTreeSet<_>, _>>()?;
        let key = history_authority_key(history.snapshot())?;
        self.authorities.retain(|key, _| retained.contains(key));
        self.authorities.insert(key, accepted.clone());
        self.history = history;
        self.accepted = accepted.clone();
        Ok(accepted)
    }
}

pub(super) fn editor_history_checkpoint(
    accepted: &AcceptedEvaluation,
    persistable: bool,
) -> Result<serde_json::Value, EngineError> {
    if persistable {
        geosolve_sketch_code::encode_editor_checkpoint(&accepted.0.materialized.editor)
            .map_err(error)
    } else {
        Ok(serde_json::Value::String(
            accepted.result().result_id.clone(),
        ))
    }
}

pub(super) fn history_authority_key(
    snapshot: &geosolve_sketch_code::CodeSessionSnapshot,
) -> Result<String, EngineError> {
    let checkpoint = snapshot
        .accepted_editor_checkpoint
        .as_str()
        .ok_or_else(|| error("invalid native history checkpoint"))?;
    // Equal geometry can accompany different document/source presentation.
    // Allocator high-water retention during Undo does not change either basis.
    let encoded =
        serde_json::to_vec(&(checkpoint, &snapshot.accepted_source_digest)).map_err(error)?;
    Ok(geosolve_sketch_intent::intent_content_digest(&encoded).to_string())
}

pub(super) fn accepted_from_history(
    history: &SketchCodeSession,
) -> Result<AcceptedEvaluation, EngineError> {
    accepted_from_validated_history_editor(
        history,
        geosolve_sketch_code::restore_editor_checkpoint(history.pointer_frame_checkpoint())
            .map_err(error)?,
    )
}

pub(super) fn accepted_from_validated_history_editor(
    history: &SketchCodeSession,
    editor: Box<geosolve_constraint_editor::ProjectionalEditorSession>,
) -> Result<AcceptedEvaluation, EngineError> {
    let snapshot = history.snapshot();
    let project = snapshot
        .accepted_code_project
        .as_ref()
        .ok_or_else(|| error("history has no accepted source project"))?;
    let expansion = snapshot
        .accepted_expansion
        .clone()
        .ok_or_else(|| error("history has no accepted source expansion"))?;
    let generated = snapshot
        .accepted_generated
        .as_ref()
        .ok_or_else(|| error("history has no accepted generated identity"))?;
    let materialized = geosolve_sketch_code::rehydrate_materialized_code_project(editor, expansion)
        .map_err(error)?;
    let digest = input_digest(
        project,
        generated,
        &snapshot.accepted_interaction_overlay,
        Some(history.identity()),
    )?;
    publication(project, *materialized, digest)
}

fn source_design_digest(
    snapshot: &geosolve_sketch_code::CodeSessionSnapshot,
) -> Result<String, EngineError> {
    let bytes = serde_json::to_vec(&(
        &snapshot.code_project,
        &snapshot.generated,
        &snapshot.interaction_overlay,
    ))
    .map_err(error)?;
    Ok(geosolve_sketch_intent::intent_content_digest(&bytes).to_string())
}

pub(super) fn input_digest(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    prior: Option<&CodeSessionIdentity>,
) -> Result<String, EngineError> {
    let bytes = serde_json::to_vec(&(project, generated, overlay, prior)).map_err(error)?;
    Ok(geosolve_sketch_intent::intent_content_digest(&bytes).to_string())
}

pub(super) fn publication(
    project: &CodeProject,
    materialized: MaterializedCodeProject,
    digest: String,
) -> Result<AcceptedEvaluation, EngineError> {
    let document = project
        .managed
        .compiled
        .as_deref()
        .map(geosolve_sketch_code::CompiledManagedSource::document_presentation)
        .transpose()
        .map_err(error)?
        .unwrap_or_default();
    let mut accepted = SketchEngine::new().publish(
        materialized,
        digest,
        EngineAuthoringMode::Editable,
        document,
        None,
        None,
    )?;
    let authority = std::rc::Rc::get_mut(&mut accepted.0)
        .ok_or_else(|| error("candidate unexpectedly shared"))?;
    authority.report.capabilities.managed_source_edits = true;
    Ok(accepted)
}
