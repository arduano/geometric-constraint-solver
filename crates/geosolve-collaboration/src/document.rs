// SPDX-License-Identifier: GPL-3.0-or-later
//! Distinct shared working source and independently accepted model source.
//!
//! This is trusted host composition state. Source text and client predictions
//! never construct an accepted model. The host stages a clone, independently
//! validates the engine candidate, then journals source/model/history together
//! before installing the staged document and acknowledging publication.

use crate::{
    protocol::MAX_REVISION,
    source_patch::{SourcePatch, source_digest},
    text::{SharedTextDocument, SharedTextLimits, SharedTextSnapshot, TextEdit, TextRevision},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const MAX_FILES: usize = 512;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptedSource {
    pub model_revision: u64,
    pub accepted_input: String,
    pub files: BTreeMap<String, String>,
}

/// Capture does not claim its source parses, evaluates or was ever accepted.
/// It retains the exact accepted basis needed for the host's three-way rebase.
#[derive(Clone, Debug)]
pub struct ApplyCapture {
    document_epoch: String,
    accepted_basis: AcceptedSource,
    working: SharedTextSnapshot,
    file_ids: BTreeMap<String, String>,
}
impl ApplyCapture {
    pub fn accepted_basis(&self) -> &AcceptedSource {
        &self.accepted_basis
    }
    pub fn working(&self) -> &SharedTextSnapshot {
        &self.working
    }
    pub fn file_ids(&self) -> &BTreeMap<String, String> {
        &self.file_ids
    }
}

/// Compiler-owned localized candidate, distinct from an invalid shared draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedFilePatch {
    pub path: String,
    pub patch: SourcePatch,
}

/// A host's recovery-AST result against exact current working bytes. It does not
/// establish geometry acceptance. Failure keeps the full file verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkingSourceChange {
    /// The exact Apply capture already contains this accepted source contribution.
    Captured {
        path: String,
    },
    Reconciled {
        path: String,
        patch: SourcePatch,
    },
    Pending {
        path: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceReconciliationNotice {
    pub path: String,
    pub accepted_revision: u64,
    pub accepted_source_digest: String,
    pub reason: String,
}

/// Captures only accepted source, not shared text. Typing while the server solves
/// does not invalidate this ticket; another model publication does.
#[derive(Clone, Debug)]
pub struct PreparedSourceUpdate {
    document_epoch: String,
    accepted_basis: AcceptedSource,
    candidate_files: BTreeMap<String, String>,
    changed_paths: BTreeSet<String>,
    apply_capture: Option<SharedTextSnapshot>,
}
impl PreparedSourceUpdate {
    pub fn accepted_basis(&self) -> &AcceptedSource {
        &self.accepted_basis
    }
    pub fn candidate_files(&self) -> &BTreeMap<String, String> {
        &self.candidate_files
    }
    pub fn changed_paths(&self) -> &BTreeSet<String> {
        &self.changed_paths
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SourceDocumentError {
    #[error("invalid source document identity or source tree")]
    Invalid,
    #[error("source document resource limit")]
    Limit,
    #[error("accepted source changed while this model candidate was being prepared")]
    StaleAccepted,
    #[error("shared text changed; reconcile this accepted source update again")]
    StaleWorking,
    #[error("source patch does not authenticate its exact source")]
    Patch,
    #[error("source reconciliation must cover each changed file exactly once")]
    Reconciliation,
    #[error("shared text rejected the proposed update: {0}")]
    Text(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceDocumentSnapshot {
    format: String,
    document_epoch: String,
    accepted: AcceptedSource,
    working_checkpoint: Vec<u8>,
    pending: Vec<SourceReconciliationNotice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    text_history: Vec<crate::TextHistoryEvent>,
    #[serde(default, skip_serializing_if = "crate::TextHistoryHorizon::is_empty")]
    text_history_horizon: crate::TextHistoryHorizon,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DurableApplyCapture {
    format: String,
    document_epoch: String,
    accepted_basis: AcceptedSource,
    working_revision: TextRevision,
    files: BTreeMap<String, String>,
    file_ids: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct SourceDocument {
    document_epoch: String,
    accepted: AcceptedSource,
    working: SharedTextDocument,
    pending: BTreeMap<String, SourceReconciliationNotice>,
    text_history: crate::TextContributionHistory,
}
impl SourceDocument {
    pub fn document_epoch(&self) -> &str {
        &self.document_epoch
    }

    /// Create only after independently validating the initial complete source.
    ///
    /// # Errors
    /// Rejects invalid identities/source and text bounds.
    pub fn new(
        document_epoch: String,
        accepted_input: String,
        files: BTreeMap<String, String>,
        actor: &[u8],
        limits: SharedTextLimits,
    ) -> Result<Self, SourceDocumentError> {
        validate_id(&document_epoch)?;
        validate_id(&accepted_input)?;
        validate_files(&files)?;
        let mut working = SharedTextDocument::new(actor, limits).map_err(text_error)?;
        let edits = files
            .iter()
            .map(|(path, text)| TextEdit::CreateFile {
                path: path.clone(),
                text: text.clone(),
            })
            .collect::<Vec<_>>();
        // Text's per-batch limit remains enforced; initialization is unpublished.
        for batch in edits.chunks(256) {
            working.edit(batch).map_err(text_error)?;
        }
        Ok(Self {
            document_epoch,
            accepted: AcceptedSource {
                model_revision: 0,
                accepted_input,
                files,
            },
            working,
            pending: BTreeMap::new(),
            text_history: crate::TextContributionHistory::default(),
        })
    }
    pub fn accepted(&self) -> &AcceptedSource {
        &self.accepted
    }
    pub fn working(&self) -> &SharedTextDocument {
        &self.working
    }
    /// The caller stages/persists text ACKs independently of the model worker.
    pub fn working_mut(&mut self) -> &mut SharedTextDocument {
        &mut self.working
    }
    /// Applies authenticated raw typing and records its native character effect
    /// under the trusted server principal/operation. Stage before durable ACK.
    ///
    /// # Errors
    /// Rejects forged writers, overlap/lifetimes, invalid history and resource bounds.
    pub fn apply_user_text_changes(
        &mut self,
        changes: &[Vec<u8>],
        actor: &[u8],
        operation: crate::protocol::OperationId,
    ) -> Result<(), SourceDocumentError> {
        let mut staged = self.clone();
        staged
            .working
            .apply_changes_from(changes, actor)
            .map_err(text_error)?;
        staged
            .text_history
            .record(
                operation,
                &self.working,
                &staged.working,
                actor.to_vec(),
                false,
            )
            .map_err(text_error)?;
        *self = staged;
        Ok(())
    }

    /// Server-ordered file lifecycle shares the personal text history timeline.
    ///
    /// # Errors
    /// Rejects stale namespace basis, non-lifecycle edits and native limits.
    pub fn apply_user_file_edits(
        &mut self,
        expected: &TextRevision,
        edits: &[TextEdit],
        operation: crate::protocol::OperationId,
    ) -> Result<(), SourceDocumentError> {
        if edits.is_empty()
            || edits
                .iter()
                .any(|edit| matches!(edit, TextEdit::Splice { .. }))
        {
            return Err(SourceDocumentError::Invalid);
        }
        let mut staged = self.clone();
        staged
            .working
            .apply_edits(expected, edits)
            .map_err(text_error)?;
        staged
            .text_history
            .record_file_edits(operation, &self.working, &staged.working, edits)
            .map_err(text_error)?;
        *self = staged;
        Ok(())
    }

    /// Personal Undo/Redo only changes working source. Explicit Apply is required
    /// to change the accepted model; no accepted source or solver state is reused.
    ///
    /// # Errors
    /// Rejects unavailable/overwritten contributions and bounded history.
    pub fn apply_user_text_inverse(
        &mut self,
        operation: crate::protocol::OperationId,
        redo: bool,
    ) -> Result<(), SourceDocumentError> {
        let mut staged = self.clone();
        staged
            .text_history
            .inverse(operation, &mut staged.working, redo)
            .map_err(text_error)?;
        *self = staged;
        Ok(())
    }
    pub fn user_text_history(&self, user: &str) -> crate::UserTextHistory {
        self.text_history.user_history(user, &self.working)
    }

    pub fn reconciliation_pending(&self) -> Vec<SourceReconciliationNotice> {
        self.pending.values().cloned().collect()
    }

    /// Captures immutable working files and their accepted basis for explicit Apply.
    ///
    /// # Errors
    /// Rejects a malformed working file identity.
    pub fn capture_apply(&self) -> Result<ApplyCapture, SourceDocumentError> {
        let working = self.working.capture();
        let file_ids = working
            .files()
            .keys()
            .map(|path| {
                Ok((
                    path.clone(),
                    self.working.file_id(path).map_err(text_error)?,
                ))
            })
            .collect::<Result<_, SourceDocumentError>>()?;
        Ok(ApplyCapture {
            document_epoch: self.document_epoch.clone(),
            accepted_basis: self.accepted.clone(),
            working,
            file_ids,
        })
    }

    /// Encode an immutable capture for the host's durable admission record.
    /// This does not mark its source accepted or give clients a restoration route.
    ///
    /// # Errors
    /// Rejects foreign/unknown captures and oversized durable admission data.
    pub fn encode_apply_capture(
        &self,
        capture: &ApplyCapture,
    ) -> Result<String, SourceDocumentError> {
        self.apply_needs_rebase(capture)?;
        let wire = DurableApplyCapture {
            format: "geosolve-apply-capture-v1".into(),
            document_epoch: capture.document_epoch.clone(),
            accepted_basis: capture.accepted_basis.clone(),
            working_revision: capture.working.revision().clone(),
            files: capture.working.files().clone(),
            file_ids: capture.file_ids.clone(),
        };
        let json = serde_json::to_string(&wire).map_err(|_| SourceDocumentError::Invalid)?;
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(SourceDocumentError::Limit);
        }
        Ok(json)
    }

    /// Restore a queued Apply after journal replay. The caller supplies the exact
    /// accepted basis authenticated at that admission, not a client-supplied basis.
    /// Historical CRDT heads independently reconstruct both text and file identity.
    /// Later typing and later model publications cannot replace the capture.
    ///
    /// # Errors
    /// Rejects mismatched journal basis, foreign epoch, unknown heads, altered
    /// captured bytes/file lifetimes and resource exhaustion.
    pub fn restore_apply_capture(
        &self,
        json: &str,
        authenticated_basis: &AcceptedSource,
    ) -> Result<ApplyCapture, SourceDocumentError> {
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(SourceDocumentError::Limit);
        }
        let wire: DurableApplyCapture =
            serde_json::from_str(json).map_err(|_| SourceDocumentError::Invalid)?;
        if wire.format != "geosolve-apply-capture-v1"
            || wire.document_epoch != self.document_epoch
            || &wire.accepted_basis != authenticated_basis
            || wire.accepted_basis.model_revision > self.accepted.model_revision
            || (wire.accepted_basis.model_revision == self.accepted.model_revision
                && wire.accepted_basis != self.accepted)
        {
            return Err(SourceDocumentError::Invalid);
        }
        validate_id(&wire.accepted_basis.accepted_input)?;
        validate_files(&wire.accepted_basis.files)?;
        let working = self
            .working
            .snapshot_at(&wire.working_revision)
            .map_err(text_error)?;
        let file_ids = self
            .working
            .file_ids_at(&wire.working_revision)
            .map_err(text_error)?;
        if working.files() != &wire.files || file_ids != wire.file_ids {
            return Err(SourceDocumentError::Invalid);
        }
        Ok(ApplyCapture {
            document_epoch: wire.document_epoch,
            accepted_basis: wire.accepted_basis,
            working,
            file_ids,
        })
    }

    /// Validates capture provenance and exposes whether a three-way source rebase
    /// is needed. A stale capture is not silently replaced by current typing.
    ///
    /// # Errors
    /// Rejects a capture from another document epoch or unknown causal history.
    pub fn apply_needs_rebase(&self, capture: &ApplyCapture) -> Result<bool, SourceDocumentError> {
        if capture.document_epoch != self.document_epoch {
            return Err(SourceDocumentError::Invalid);
        }
        self.working
            .changes_since(capture.working.revision())
            .map_err(text_error)?;
        Ok(capture.accepted_basis != self.accepted)
    }

    /// Prepare exact compiler-produced source changes for a semantic canvas edit.
    /// This never reads, parses or replaces an invalid working draft.
    ///
    /// # Errors
    /// Rejects forged/duplicate file patches and bounded invalid source trees.
    pub fn prepare_canvas_update(
        &self,
        patches: &[AcceptedFilePatch],
    ) -> Result<PreparedSourceUpdate, SourceDocumentError> {
        if patches.is_empty() || patches.len() > MAX_FILES {
            return Err(SourceDocumentError::Limit);
        }
        let mut candidate_files = self.accepted.files.clone();
        let mut changed_paths = BTreeSet::new();
        for entry in patches {
            if !changed_paths.insert(entry.path.clone()) {
                return Err(SourceDocumentError::Patch);
            }
            let source = candidate_files
                .get(&entry.path)
                .ok_or(SourceDocumentError::Patch)?;
            let candidate = entry
                .patch
                .apply(source)
                .map_err(|_| SourceDocumentError::Patch)?;
            candidate_files.insert(entry.path.clone(), candidate);
        }
        validate_files(&candidate_files)?;
        Ok(PreparedSourceUpdate {
            document_epoch: self.document_epoch.clone(),
            accepted_basis: self.accepted.clone(),
            candidate_files,
            changed_paths,
            apply_capture: None,
        })
    }

    /// Prepare an Apply candidate after the compiler host reconciles the captured
    /// working changes with accepted changes since its basis. Unchanged basis must
    /// preserve the exact captured files; rebased semantics are compiler-owned.
    ///
    /// # Errors
    /// Rejects foreign captures, altered uncrossed captures and invalid source trees.
    pub fn prepare_apply_update(
        &self,
        capture: &ApplyCapture,
        rebased_files: BTreeMap<String, String>,
    ) -> Result<PreparedSourceUpdate, SourceDocumentError> {
        let needs_rebase = self.apply_needs_rebase(capture)?;
        if !needs_rebase && &rebased_files != capture.working.files() {
            return Err(SourceDocumentError::Invalid);
        }
        validate_files(&rebased_files)?;
        let changed_paths = self
            .accepted
            .files
            .keys()
            .chain(rebased_files.keys())
            .filter(|path| self.accepted.files.get(*path) != rebased_files.get(*path))
            .cloned()
            .collect();
        Ok(PreparedSourceUpdate {
            document_epoch: self.document_epoch.clone(),
            accepted_basis: self.accepted.clone(),
            candidate_files: rebased_files,
            changed_paths,
            apply_capture: Some(capture.working.clone()),
        })
    }

    /// Publish only after independent engine validation of this exact candidate.
    /// Working reconciliations are prepared against current causal heads after
    /// solving. Missing lexical ownership retains draft bytes with a notice.
    /// The host durably installs a staged clone, not this mutation alone.
    ///
    /// # Errors
    /// Rejects stale accepted/working bases, incomplete reconciliation and invalid
    /// patches atomically, preserving accepted files, text heads and notices.
    pub fn publish_validated(
        &mut self,
        prepared: &PreparedSourceUpdate,
        accepted_input: String,
        expected_working: &TextRevision,
        reconciliations: &[WorkingSourceChange],
    ) -> Result<(), SourceDocumentError> {
        validate_id(&accepted_input)?;
        if prepared.document_epoch != self.document_epoch
            || prepared.accepted_basis != self.accepted
        {
            return Err(SourceDocumentError::StaleAccepted);
        }
        if self.working.revision() != *expected_working {
            return Err(SourceDocumentError::StaleWorking);
        }
        if reconciliations.len() != prepared.changed_paths.len() {
            return Err(SourceDocumentError::Reconciliation);
        }
        let revision = self
            .accepted
            .model_revision
            .checked_add(1)
            .filter(|value| *value <= MAX_REVISION)
            .ok_or(SourceDocumentError::Limit)?;
        let mut staged = self.clone();
        let mut seen = BTreeSet::new();
        for reconciliation in reconciliations {
            let path = match reconciliation {
                WorkingSourceChange::Captured { path }
                | WorkingSourceChange::Reconciled { path, .. }
                | WorkingSourceChange::Pending { path, .. } => path,
            };
            if !prepared.changed_paths.contains(path) || !seen.insert(path.clone()) {
                return Err(SourceDocumentError::Reconciliation);
            }
            match reconciliation {
                WorkingSourceChange::Captured { path } => {
                    let capture = prepared
                        .apply_capture
                        .as_ref()
                        .ok_or(SourceDocumentError::Reconciliation)?;
                    self.working
                        .changes_since(capture.revision())
                        .map_err(text_error)?;
                    if capture.files().get(path) != prepared.candidate_files.get(path) {
                        return Err(SourceDocumentError::Reconciliation);
                    }
                    staged.pending.remove(path);
                }
                WorkingSourceChange::Reconciled { path, patch } => {
                    let snapshot = staged.working.capture();
                    let source = snapshot.text(path).ok_or(SourceDocumentError::Patch)?;
                    patch
                        .apply(source)
                        .map_err(|_| SourceDocumentError::Patch)?;
                    let edits = patch
                        .edits
                        .iter()
                        .rev()
                        .map(|edit| TextEdit::Splice {
                            path: path.clone(),
                            start_utf16: edit.start,
                            delete_utf16: edit.end - edit.start,
                            insert: edit.replacement.clone(),
                        })
                        .collect::<Vec<_>>();
                    staged.working.edit(&edits).map_err(text_error)?;
                    // An earlier unresolved contribution must stay visible until
                    // the whole file matches its accepted counterpart or Apply
                    // explicitly reconciles its captured semantic difference.
                    if staged.working.capture().text(path)
                        == prepared.candidate_files.get(path).map(String::as_str)
                    {
                        staged.pending.remove(path);
                    }
                }
                WorkingSourceChange::Pending { path, reason } => {
                    if reason.is_empty() || reason.len() > 1024 {
                        return Err(SourceDocumentError::Invalid);
                    }
                    staged.pending.insert(
                        path.clone(),
                        SourceReconciliationNotice {
                            path: path.clone(),
                            accepted_revision: revision,
                            accepted_source_digest: prepared
                                .candidate_files
                                .get(path)
                                .map_or_else(String::new, |source| source_digest(source)),
                            reason: reason.clone(),
                        },
                    );
                }
            }
        }
        staged.accepted = AcceptedSource {
            model_revision: revision,
            accepted_input,
            files: prepared.candidate_files.clone(),
        };
        *self = staged;
        Ok(())
    }

    /// # Errors
    /// Rejects snapshots exceeding the bounded transport or encoding failure.
    pub fn to_json(&self) -> Result<String, SourceDocumentError> {
        let snapshot = SourceDocumentSnapshot {
            format: "geosolve-source-document-v1".into(),
            document_epoch: self.document_epoch.clone(),
            accepted: self.accepted.clone(),
            working_checkpoint: self.working.save(),
            pending: self.pending.values().cloned().collect(),
            text_history: self.text_history.events().to_vec(),
            text_history_horizon: self.text_history.checkpoint_horizon().clone(),
        };
        let json = serde_json::to_string(&snapshot).map_err(|_| SourceDocumentError::Invalid)?;
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(SourceDocumentError::Limit);
        }
        Ok(json)
    }
    /// Restore raw and accepted sources separately. The host must independently
    /// reconstruct accepted geometry, even when the shared draft is invalid.
    ///
    /// # Errors
    /// Rejects corrupt/oversized text, invalid identity and inconsistent notices.
    pub fn from_json(
        json: &str,
        actor: &[u8],
        limits: SharedTextLimits,
    ) -> Result<Self, SourceDocumentError> {
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(SourceDocumentError::Limit);
        }
        let snapshot: SourceDocumentSnapshot =
            serde_json::from_str(json).map_err(|_| SourceDocumentError::Invalid)?;
        if snapshot.format != "geosolve-source-document-v1"
            || snapshot.accepted.model_revision > MAX_REVISION
            || snapshot.pending.len() > MAX_FILES
        {
            return Err(SourceDocumentError::Invalid);
        }
        validate_id(&snapshot.document_epoch)?;
        validate_id(&snapshot.accepted.accepted_input)?;
        validate_files(&snapshot.accepted.files)?;
        let working = SharedTextDocument::load(&snapshot.working_checkpoint, actor, limits)
            .map_err(text_error)?;
        let text_history = crate::TextContributionHistory::restore_with_horizon(
            snapshot.text_history,
            snapshot.text_history_horizon,
            &working,
        )
        .map_err(text_error)?;
        let mut pending = BTreeMap::new();
        for notice in snapshot.pending {
            if notice.accepted_revision > snapshot.accepted.model_revision
                || notice.accepted_revision == 0
                || notice.reason.is_empty()
                || notice.reason.len() > 1024
                || (!notice.accepted_source_digest.is_empty()
                    && (notice.accepted_source_digest.len() != 64
                        || !notice
                            .accepted_source_digest
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit())))
                || pending.insert(notice.path.clone(), notice).is_some()
            {
                return Err(SourceDocumentError::Invalid);
            }
        }
        Ok(Self {
            document_epoch: snapshot.document_epoch,
            accepted: snapshot.accepted,
            working,
            pending,
            text_history,
        })
    }
}
fn validate_id(id: &str) -> Result<(), SourceDocumentError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
    {
        return Err(SourceDocumentError::Invalid);
    }
    Ok(())
}
fn validate_files(files: &BTreeMap<String, String>) -> Result<(), SourceDocumentError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(SourceDocumentError::Limit);
    }
    let mut total = 0_usize;
    for (path, source) in files {
        if path.is_empty()
            || path.len() > 512
            || path.starts_with('/')
            || path.contains('\\')
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".." || part == ".geosolve")
        {
            return Err(SourceDocumentError::Invalid);
        }
        total = total.saturating_add(source.len());
        if source.len() > MAX_FILE_BYTES || total > MAX_SOURCE_BYTES {
            return Err(SourceDocumentError::Limit);
        }
    }
    Ok(())
}
fn text_error(error: impl std::fmt::Display) -> SourceDocumentError {
    SourceDocumentError::Text(error.to_string())
}
