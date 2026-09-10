// SPDX-License-Identifier: GPL-3.0-or-later
//! Managed session history delegates to `SketchCodeSession`; private checkpoint keys
//! retain immutable accepted native authorities without serializing solved geometry.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeProject, CodeSessionIdentity, KeyedReconcileState,
    MaterializedCodeProject, ProjectKey, SketchCodeSession,
    materialize_code_project_cold_with_overlay,
    materialize_code_project_incremental_for_structural_edit,
    materialize_code_project_incremental_with_overlay, required_generated_members,
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
    history: SketchCodeSession,
    authorities: BTreeMap<String, AcceptedEvaluation>,
    accepted: AcceptedEvaluation,
}

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

impl EditableSession {
    pub(super) fn code_snapshot(&self) -> &geosolve_sketch_code::CodeSessionSnapshot {
        self.history.snapshot()
    }

    /// Opens authentic source and optional semantic overrides, with no persisted native checkpoint.
    ///
    /// # Errors
    /// Rejects invalid source, foreign/stale semantic overrides or unaccepted geometry.
    pub fn open(project_json: &str, design_json: Option<&str>) -> Result<Self, EngineError> {
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
        let key = accepted.result().result_id.clone();
        let history = SketchCodeSession::new_project_with_overlay(
            project,
            generated,
            design.overrides,
            expansion,
            serde_json::Value::String(key.clone()),
        )
        .map_err(error)?;
        Ok(Self {
            history,
            authorities: BTreeMap::from([(key, accepted.clone())]),
            accepted,
        })
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
        let plan = self
            .history
            .plan_structural_reconciliation(
                expected,
                required_generated_members(&project).map_err(error)?,
                &BTreeSet::new(),
            )
            .map_err(error)?;
        let (materialized, overlay) = materialize_code_project_incremental_for_structural_edit(
            &self.accepted.0.materialized,
            &project,
            plan.staged(),
            &self.history.snapshot().interaction_overlay,
        )
        .map_err(error)?;
        let digest = input_digest(&project, plan.staged(), &overlay, Some(expected))?;
        let expansion = materialized.expansion.clone();
        let accepted = publication(&project, materialized, digest)?;
        let prepared = self
            .history
            .prepare_project_edit_from_plan_with_overlay(
                expected,
                project,
                plan,
                overlay,
                expansion,
                serde_json::Value::String(accepted.result().result_id.clone()),
                "Apply project",
            )
            .map_err(error)?;
        let mut history = self.history.clone();
        history.apply_prepared(prepared).map_err(error)?;
        self.install(history, accepted)
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
                serde_json::Value::String(accepted.result().result_id.clone()),
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
        let mut history = self.history.clone();
        if undo { history.undo() } else { history.redo() }.map_err(error)?;
        let key = history
            .pointer_frame_checkpoint()
            .as_str()
            .ok_or_else(|| error("invalid private checkpoint key"))?;
        let accepted = self
            .authorities
            .get(key)
            .ok_or_else(|| error("missing private accepted history authority"))?
            .clone();
        self.history = history;
        self.accepted = accepted.clone();
        Ok(accepted)
    }

    fn authenticate(&self, expected: &CodeSessionIdentity) -> Result<(), EngineError> {
        if expected != self.token() {
            return Err(error("stale or foreign editable session token"));
        }
        Ok(())
    }

    fn install(
        &mut self,
        history: SketchCodeSession,
        accepted: AcceptedEvaluation,
    ) -> Result<AcceptedEvaluation, EngineError> {
        // Public owning history serialization includes every retained opaque checkpoint.
        // This private index is reconstructible only while this session owns the handles.
        let wire: serde_json::Value =
            serde_json::from_str(&history.to_canonical_json().map_err(error)?).map_err(error)?;
        let mut snapshots = vec![&wire["snapshot"]];
        for direction in ["undo", "redo"] {
            if let Some(entries) = wire[direction].as_array() {
                snapshots.extend(entries.iter().map(|entry| &entry["snapshot"]));
            }
        }
        let retained = snapshots
            .into_iter()
            .flat_map(|snapshot| {
                [
                    snapshot["editor_checkpoint"].as_str(),
                    snapshot["accepted_editor_checkpoint"].as_str(),
                ]
            })
            .flatten()
            .collect::<BTreeSet<_>>();
        self.authorities
            .retain(|key, _| retained.contains(key.as_str()));
        self.authorities
            .insert(accepted.result().result_id.clone(), accepted.clone());
        self.history = history;
        self.accepted = accepted.clone();
        Ok(accepted)
    }
}

fn input_digest(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    prior: Option<&CodeSessionIdentity>,
) -> Result<String, EngineError> {
    let bytes = serde_json::to_vec(&(project, generated, overlay, prior)).map_err(error)?;
    Ok(geosolve_sketch_intent::intent_content_digest(&bytes).to_string())
}

fn publication(
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
