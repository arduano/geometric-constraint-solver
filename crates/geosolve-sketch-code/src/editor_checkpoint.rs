// SPDX-License-Identifier: GPL-3.0-or-later
//! Native checkpoint codec used by all source-authoring hosts.
use geosolve_constraint_editor::ProjectionalEditorSession;

/// Complete native checkpoint admission, with its accepted editor retained for installation.
/// Private construction prevents hosts from marking opaque history as independently validated.
#[derive(Debug)]
pub struct ValidatedSourceHistory {
    history: crate::SketchCodeSession,
    accepted_editor: Box<ProjectionalEditorSession>,
}

impl ValidatedSourceHistory {
    /// Admits the existing source-history wire and every distinct native checkpoint once.
    ///
    /// # Errors
    /// Rejects malformed source/history authority, native geometry or checkpoint transport.
    pub fn decode(json: &str) -> Result<Self, String> {
        Self::validate(
            crate::SketchCodeSession::from_json(json).map_err(|error| error.to_string())?,
        )
    }

    /// Validates typed history whose opaque host checkpoints have no native proof yet.
    ///
    /// # Errors
    /// Rejects any invalid current, accepted, Undo or Redo native checkpoint.
    pub fn validate(history: crate::SketchCodeSession) -> Result<Self, String> {
        let current = history.pointer_frame_checkpoint();
        let mut seen = std::collections::BTreeSet::new();
        let mut accepted_editor = None;
        for snapshot in history.retained_snapshots() {
            for checkpoint in [
                &snapshot.editor_checkpoint,
                &snapshot.accepted_editor_checkpoint,
            ] {
                let encoded = checkpoint.as_str().ok_or_else(|| {
                    "code-project editor checkpoint is not encoded text".to_owned()
                })?;
                if seen.insert(encoded) {
                    let editor = restore_editor_checkpoint(checkpoint)?;
                    if checkpoint == current {
                        accepted_editor = Some(editor);
                    }
                }
            }
        }
        let accepted_editor = accepted_editor
            .ok_or_else(|| "source history has no accepted native checkpoint".to_owned())?;
        Ok(Self {
            history,
            accepted_editor,
        })
    }

    pub fn history(&self) -> &crate::SketchCodeSession {
        &self.history
    }

    /// Consumes the receipt; no mutable history reference can outlive its validation.
    pub fn into_parts(self) -> (crate::SketchCodeSession, Box<ProjectionalEditorSession>) {
        (self.history, self.accepted_editor)
    }
}

/// Encodes native authority as the delegated checkpoint stored in source history.
///
/// # Errors
/// Rejects unavailable native authority, invalid revisions or oversized serialization.
pub fn encode_editor_checkpoint(
    editor: &ProjectionalEditorSession,
) -> Result<serde_json::Value, String> {
    let (computed_evaluation_high_water, revisions) =
        geosolve_constraint_editor::workspace_persistence::WorkspaceSnapshot::projectional_authority_metadata(editor)?;
    geosolve_constraint_editor::workspace_persistence::WorkspaceSnapshot::encode_delegated_projectional_editor(
        editor,
        computed_evaluation_high_water,
        revisions,
    )
    .map(serde_json::Value::String)
}

/// Validates a source-history checkpoint through ordinary native cold restoration.
///
/// # Errors
/// Rejects malformed checkpoint bytes, changed ownership or failed independent geometry validation.
pub fn validate_editor_checkpoint(checkpoint: &serde_json::Value) -> Result<(), String> {
    restore_editor_checkpoint(checkpoint).map(|_| ())
}

/// Restores one delegated source-history editor with independently accepted geometry.
///
/// # Errors
/// Rejects non-text/corrupt checkpoints, unavailable accepted scenes or invalid residual/feature evidence.
pub fn restore_editor_checkpoint(
    checkpoint: &serde_json::Value,
) -> Result<Box<ProjectionalEditorSession>, String> {
    let encoded = checkpoint
        .as_str()
        .ok_or_else(|| "code-project editor checkpoint is not encoded text".to_owned())?;
    let snapshot =
        geosolve_constraint_editor::workspace_persistence::WorkspaceSnapshot::decode(encoded)?;
    snapshot.validate_delegated_intent_checkpoint()?;
    let editor = Box::new(
        geosolve_constraint_editor::workspace_persistence::projectional_editor_from_snapshot(
            &snapshot,
        )?,
    );
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code-project editor checkpoint has no accepted native scene".to_owned())?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err("code-project editor checkpoint failed independent validation".into());
    }
    Ok(editor)
}
