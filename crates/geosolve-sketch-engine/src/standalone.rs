// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent native host composition over the same engine/source transaction owners.
//! View state and unfinished text belong to the host; native source/history authority does not.

use crate::authoring_commit::PreparedNativeAuthoring;
use crate::session::{
    accepted_from_history, editor_history_checkpoint, history_authority_key, input_digest,
    publication,
};
use crate::{EditableSession, EngineError, PreparedAuthoringMutation};
use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeProject, CodeSessionReceipt, ExpandedCodeProject,
    ExpandedWritablePoint, ManagedSketchMutation, MaterializedCodeProject,
    PreparedManagedMutationReceipt, PreparedManagedMutationRequest, SketchCodeSession,
    encode_editor_checkpoint, expand_code_project_for_structural_edit,
    materialize_code_project_incremental_for_structural_edit, rehydrate_materialized_code_project,
    required_generated_members, restore_editor_checkpoint,
};
use std::collections::{BTreeMap, BTreeSet};

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

/// Explicit source-host policy: failed authored source can occupy an Undo entry while the
/// independently accepted scene stays retained. Server atomic edits keep their rejection policy.
#[derive(Debug)]
pub enum PersistentSourceApply {
    Accepted {
        receipt: CodeSessionReceipt,
    },
    RetainedFailure {
        receipt: CodeSessionReceipt,
        diagnostic: String,
    },
}

/// Engine-created preparation for a native terminal or structured source mutation.
#[derive(Debug)]
pub struct PreparedCanvasSourceMutation {
    mutation: PreparedAuthoringMutation,
    native: Option<PreparedNativeAuthoring>,
}
impl PreparedCanvasSourceMutation {
    pub fn request(&self) -> &PreparedManagedMutationRequest {
        self.mutation.request()
    }
}

impl EditableSession {
    /// Read-only source/history authority for native host projections and exact persisted formats.
    pub fn source_session(&self) -> &SketchCodeSession {
        &self.history
    }

    /// Immutable accepted geometry for native presentation and disposable gesture setup.
    pub fn accepted_materialization(&self) -> &MaterializedCodeProject {
        &self.accepted.0.materialized
    }

    /// Restores already decoded source history through the same independent native admission.
    ///
    /// # Errors
    /// Rejects unavailable or invalid accepted source/native authority.
    pub fn from_source_session(history: SketchCodeSession) -> Result<Self, EngineError> {
        Self::from_validated_source_history(
            geosolve_sketch_code::ValidatedSourceHistory::validate(history).map_err(error)?,
        )
    }

    /// Consumes complete source/native history admission without repeating checkpoint validation.
    ///
    /// # Errors
    /// Rejects mismatch between accepted native geometry and authenticated source expansion.
    pub fn from_validated_source_history(
        validated: geosolve_sketch_code::ValidatedSourceHistory,
    ) -> Result<Self, EngineError> {
        let (history, editor) = validated.into_parts();
        let accepted = crate::session::accepted_from_validated_history_editor(&history, editor)?;
        let key = history_authority_key(history.snapshot())?;
        Ok(Self {
            history,
            authorities: BTreeMap::from([(key, accepted.clone())]),
            accepted,
            persistable_history: true,
        })
    }

    /// Isolates preparation without serializing or dropping any Undo/Redo entries.
    #[must_use]
    pub fn fork_authoring(&self) -> Self {
        self.fork_for_preparation()
    }

    /// Prepares authored native additions from their original completed-authoring receipt.
    ///
    /// # Errors
    /// Rejects unavailable source, invalid native additions or absent provenance.
    pub fn prepare_canvas_source_mutation(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<PreparedCanvasSourceMutation, EngineError> {
        let materialized = self.accepted_materialization();
        let project = self
            .history
            .snapshot()
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        let (declarations, high_water, drafts) =
            geosolve_sketch_code::prepare_editor_source_insertion(
                project,
                &materialized.expansion,
                &materialized.editor,
                editor,
            )
            .map_err(error)?
            .into_parts();
        let native = self.prepare_native_authoring(
            Box::new(editor.fork_accepted_authority().map_err(error)?),
            &declarations,
            ManagedSketchMutation::InsertDeclarations {
                declarations: drafts,
            },
            high_water,
        )?;
        Ok(PreparedCanvasSourceMutation {
            mutation: native.mutation.clone(),
            native: Some(native),
        })
    }

    /// Prepares a managed panel edit through the same genuine compiler owner as engine tools.
    ///
    /// # Errors
    /// Rejects invalid source mutations or source/name authority.
    pub fn prepare_structured_source_mutation(
        &self,
        mutation: ManagedSketchMutation,
    ) -> Result<PreparedCanvasSourceMutation, EngineError> {
        let high_water = self.history.snapshot().managed.declaration_name_high_water;
        Ok(PreparedCanvasSourceMutation {
            mutation: self.prepare_managed_mutation(mutation, high_water)?,
            native: None,
        })
    }

    /// Applies genuine compiler evidence and native terminal parity as one isolated source transaction.
    ///
    /// # Errors
    /// Rejects stale receipts, invalid geometry or terminal mismatch without changing live authority.
    pub fn apply_canvas_source_mutation(
        &mut self,
        prepared: &PreparedCanvasSourceMutation,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<CodeSessionReceipt, EngineError> {
        let before = self.token().clone();
        if let Some(native) = &prepared.native {
            let candidate =
                self.resolve_native_authoring(native, receipt, "Canvas", "Apply managed source")?;
            self.install_native_authoring(candidate)?;
        } else {
            self.apply_managed_mutation_for_host(
                &prepared.mutation,
                receipt,
                "Apply managed source",
            )?;
        }
        Ok(CodeSessionReceipt {
            before,
            after: self.token().clone(),
            label: "Apply managed source".into(),
            retained_failure: false,
        })
    }

    /// Applies source with the standalone host's explicit retained-failure history policy.
    ///
    /// # Errors
    /// Rejects cross-project input, corrupt history or invalid checkpoint publication.
    pub fn apply_project_retaining_failure(
        &mut self,
        project: CodeProject,
    ) -> Result<PersistentSourceApply, EngineError> {
        self.apply_project_for_host(project, "Apply managed source", true)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "source planning, materialization and explicit failure policy form one atomic publication"
    )]
    pub(super) fn apply_project_for_host(
        &mut self,
        project: CodeProject,
        label: &str,
        retain_failure: bool,
    ) -> Result<PersistentSourceApply, EngineError> {
        project.validate().map_err(error)?;
        if project.project != self.history.snapshot().project {
            return Err(error(
                "project update belongs to a different editable session project",
            ));
        }
        let desired = match required_generated_members(&project) {
            Ok(value) => value,
            Err(cause) if retain_failure => {
                return self.retain_source_failure(
                    project,
                    None,
                    None,
                    self.history.snapshot().interaction_overlay.clone(),
                    "structural expansion",
                    cause.to_string(),
                    label,
                );
            }
            Err(cause) => return Err(error(cause)),
        };
        let expected = self.token().clone();
        let plan =
            match self
                .history
                .plan_structural_reconciliation(&expected, desired, &BTreeSet::new())
            {
                Ok(value) => value,
                Err(cause) if retain_failure => {
                    return self.retain_source_failure(
                        project,
                        None,
                        None,
                        self.history.snapshot().interaction_overlay.clone(),
                        "keyed reconciliation",
                        cause.to_string(),
                        label,
                    );
                }
                Err(cause) => return Err(error(cause)),
            };
        match materialize_code_project_incremental_for_structural_edit(
            self.accepted_materialization(),
            &project,
            plan.staged(),
            &self.history.snapshot().interaction_overlay,
        ) {
            Ok((materialized, overlay)) => {
                let expansion = materialized.expansion.clone();
                // Persistent source hosts historically restored a delegated checkpoint before
                // publication. Preserve its exact input/identity normalization in this owner.
                let materialized = if self.persistable_history {
                    let checkpoint =
                        encode_editor_checkpoint(&materialized.editor).map_err(error)?;
                    *rehydrate_materialized_code_project(
                        restore_editor_checkpoint(&checkpoint).map_err(error)?,
                        expansion.clone(),
                    )
                    .map_err(error)?
                } else {
                    materialized
                };
                let digest = input_digest(&project, plan.staged(), &overlay, Some(&expected))?;
                let accepted = publication(&project, materialized, digest)?;
                let prepared = self
                    .history
                    .prepare_project_edit_from_plan_with_overlay(
                        &expected,
                        project,
                        plan,
                        overlay,
                        expansion,
                        editor_history_checkpoint(&accepted, self.persistable_history)?,
                        label,
                    )
                    .map_err(error)?;
                let mut history = self.history.clone();
                let receipt = history.apply_prepared(prepared).map_err(error)?;
                self.install(history, accepted)?;
                Ok(PersistentSourceApply::Accepted { receipt })
            }
            Err(cause) if retain_failure => {
                let overlay = &self.history.snapshot().interaction_overlay;
                let (expansion, retained) = expand_code_project_for_structural_edit(
                    &project,
                    plan.staged(),
                    overlay,
                    self.accepted_materialization()
                        .editor
                        .coordinator()
                        .intent()
                        .identity(),
                )
                .map_or_else(
                    |_| (None, overlay.clone()),
                    |(expanded, retained)| (Some(expanded), retained),
                );
                self.retain_source_failure(
                    project,
                    Some(plan),
                    expansion,
                    retained,
                    "native materialization",
                    cause.to_string(),
                    label,
                )
            }
            Err(cause) => Err(error(cause)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn retain_source_failure(
        &mut self,
        project: CodeProject,
        plan: Option<geosolve_sketch_code::KeyedReconcilePlan>,
        expansion: Option<ExpandedCodeProject>,
        overlay: CodeInteractionOverlay,
        stage: &str,
        diagnostic: String,
        label: &str,
    ) -> Result<PersistentSourceApply, EngineError> {
        let prepared = self
            .history
            .prepare_project_retained_failure(
                self.token(),
                project,
                plan,
                overlay,
                expansion,
                self.history.snapshot().accepted_editor_checkpoint.clone(),
                stage,
                diagnostic.clone(),
                format!("{label} (retained failure)"),
            )
            .map_err(error)?;
        let mut history = self.history.clone();
        let receipt = history.apply_prepared(prepared).map_err(error)?;
        self.install(history, self.accepted.clone())?;
        Ok(PersistentSourceApply::RetainedFailure {
            receipt,
            diagnostic,
        })
    }

    /// Publishes an independently accepted delegated point terminal through the shared source codec.
    ///
    /// # Errors
    /// Rejects stale writable lenses, explicit branches or incomplete terminal parity.
    pub fn commit_delegated_point_terminal(
        &mut self,
        authenticated: &ExpandedWritablePoint,
        editor: &ProjectionalEditorSession,
        terminal: &geosolve_sketch::RetainedSketchDocumentSession,
        label: &str,
    ) -> Result<CodeSessionReceipt, EngineError> {
        let snapshot = self.history.snapshot();
        if snapshot.failure.is_some() {
            return Err(error(
                "resolve or Undo retained source failure before point publication",
            ));
        }
        let project = snapshot
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        // The source session adds exact writable-owner authentication before the shared terminal codec.
        let position =
            geosolve_sketch_code::editor_terminal::TerminalPointPreview::new(editor, terminal)
                .map_err(error)?
                .position(&authenticated.handle)
                .ok_or_else(|| error("terminal point lens disappeared"))?;
        self.history
            .stage_point_drag(authenticated, position)
            .map_err(error)?;
        let (materialized, overlay) = geosolve_sketch_code::editor_terminal::materialize_terminal(
            self.accepted_materialization(),
            project,
            &snapshot.generated,
            &snapshot.interaction_overlay,
            authenticated,
            editor,
            terminal,
        )
        .map_err(error)?;
        let expansion = materialized.expansion.clone();
        let digest = input_digest(project, &snapshot.generated, &overlay, Some(self.token()))?;
        let accepted = publication(project, materialized, digest)?;
        let prepared = self
            .history
            .prepare_project_overlay(
                self.token(),
                overlay,
                expansion,
                editor_history_checkpoint(&accepted, self.persistable_history)?,
                label,
            )
            .map_err(error)?;
        let mut history = self.history.clone();
        let receipt = history.apply_prepared(prepared).map_err(error)?;
        self.install(history, accepted)?;
        Ok(receipt)
    }

    /// Steps existing persisted source history while retaining its authentic receipt/label.
    ///
    /// # Errors
    /// Rejects corrupt native history without publishing a partial step.
    pub fn step_source_history(
        &mut self,
        undo: bool,
    ) -> Result<Option<CodeSessionReceipt>, EngineError> {
        let mut history = self.history.clone();
        let receipt = if undo { history.undo() } else { history.redo() }.map_err(error)?;
        let Some(receipt) = receipt else {
            return Ok(None);
        };
        let key = history_authority_key(history.snapshot())?;
        let accepted = match self.authorities.get(&key) {
            Some(value) => value.clone(),
            None if self.persistable_history => accepted_from_history(&history)?,
            None => return Err(error("missing private accepted history authority")),
        };
        self.install(history, accepted)?;
        Ok(Some(receipt))
    }
}
