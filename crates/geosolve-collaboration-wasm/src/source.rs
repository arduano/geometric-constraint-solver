// SPDX-License-Identifier: GPL-3.0-or-later
//! Trusted dual-source composition with asynchronous durable staging. Working
//! text remains independent of long-lived model preparation and Apply captures.

use crate::{error, json, parse};
use geosolve_collaboration::{
    SharedTextLimits, TextEdit, TextRevision,
    document::{
        AcceptedFilePatch, ApplyCapture, PreparedSourceUpdate, SourceDocument, WorkingSourceChange,
    },
    protocol::MAX_REVISION,
    source_patch::{SourcePatch, source_digest},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU32, Ordering},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_ENVELOPE_BYTES: usize = 384 * 1024 * 1024;
const MAX_HANDLES: usize = 32;
const MAX_HANDLE_BYTES: usize = 64 * 1024 * 1024;
static NEXT_SOURCE_HOST: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    document_epoch: String,
    server_epoch: String,
    initial_input: String,
    files: BTreeMap<String, String>,
    #[serde(default)]
    limits: SharedTextLimits,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    document_epoch: String,
    sequence: u64,
    source_json: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePatch {
    path: String,
    patch: SourcePatch,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Reconciliation {
    Captured { path: String },
    Reconciled { path: String, patch: SourcePatch },
    Pending { path: String, reason: String },
}
impl From<Reconciliation> for WorkingSourceChange {
    fn from(value: Reconciliation) -> Self {
        match value {
            Reconciliation::Captured { path } => Self::Captured { path },
            Reconciliation::Reconciled { path, patch } => Self::Reconciled { path, patch },
            Reconciliation::Pending { path, reason } => Self::Pending { path, reason },
        }
    }
}
#[derive(Debug)]
struct CaptureEntry {
    capture: ApplyCapture,
    bytes: usize,
}
#[derive(Debug)]
struct PreparedEntry {
    prepared: PreparedSourceUpdate,
    bytes: usize,
}
#[derive(Debug)]
struct PendingSource {
    id: String,
    basis_sequence: u64,
    candidate: SourceDocument,
    sequence: u64,
    terminal_ticket: Option<String>,
}

/// Trusted source host. Caller supplies authenticated text actors and validated
/// model outcomes; no browser packet acquires accepted-source publication authority.
#[derive(Debug)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct TrustedSourceHost {
    document: SourceDocument,
    document_epoch: String,
    server_epoch: String,
    instance: u32,
    sequence: u64,
    next_handle: u64,
    captures: BTreeMap<String, CaptureEntry>,
    prepared: BTreeMap<String, PreparedEntry>,
    pending: Option<PendingSource>,
    poisoned: bool,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl TrustedSourceHost {
    /// Creates accepted initial source only after the host independently validates it.
    ///
    /// # Errors
    /// Rejects invalid config, actors, source trees and bounded resources.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(config_json: &str, actor: &[u8]) -> Result<Self, String> {
        let config: Configuration = parse(config_json)?;
        let document = SourceDocument::new(
            config.document_epoch.clone(),
            config.initial_input.clone(),
            config.files.clone(),
            actor,
            config.limits,
        )
        .map_err(error)?;
        Self::from_document(document, config, 0)
    }
    /// Restores an exact durable source envelope; accepted geometry is rebuilt separately.
    /// Runtime captures/tickets are deliberately not restored from client descriptors.
    ///
    /// # Errors
    /// Rejects malformed/foreign/oversized envelopes and core snapshot inconsistencies.
    pub fn restore(config_json: &str, actor: &[u8], checkpoint_json: &str) -> Result<Self, String> {
        let config: Configuration = parse(config_json)?;
        if checkpoint_json.len() > MAX_ENVELOPE_BYTES {
            return Err("source envelope byte limit".into());
        }
        let envelope: Envelope = serde_json::from_str(checkpoint_json).map_err(error)?;
        if envelope.format != "geosolve-source-host-v1"
            || envelope.document_epoch != config.document_epoch
            || envelope.sequence > MAX_REVISION
        {
            return Err("invalid source envelope identity or sequence".into());
        }
        let document = SourceDocument::from_json(&envelope.source_json, actor, config.limits)
            .map_err(error)?;
        if document.document_epoch() != config.document_epoch
            || document.accepted().model_revision > envelope.sequence
        {
            return Err("invalid source document epoch or sequence".into());
        }
        Self::from_document(document, config, envelope.sequence)
    }
    /// Committed accepted/working files, causal heads, stable file IDs and notices.
    ///
    /// # Errors
    /// Reports source identity or JSON encoding failure.
    pub fn snapshot(&self) -> Result<String, String> {
        Self::snapshot_of(
            &self.document,
            self.sequence,
            self.pending.is_some(),
            self.poisoned,
        )
    }
    /// Exact committed envelope for durable restore; no pending candidate is included.
    ///
    /// # Errors
    /// Rejects oversized checkpoint serialization.
    pub fn checkpoint(&self) -> Result<String, String> {
        self.envelope(&self.document, self.sequence)
    }
    /// Native raw text checkpoint for joining client replicas, never accepted geometry.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = textCheckpoint))]
    pub fn text_checkpoint(&self) -> Vec<u8> {
        self.document.working().save()
    }
    /// Disposable handshake generation uses committed text only.
    ///
    /// # Errors
    /// Rejects pending source persistence, recovery and text peer/message limits.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = generateSyncMessage))]
    pub fn generate_sync_message(&mut self, peer: &str) -> Result<Option<Vec<u8>>, String> {
        self.mutable()?;
        self.document
            .working_mut()
            .generate_sync_message(peer)
            .map_err(error)
    }
    /// Resets peer handshake after transport reconnect or dropped generated messages.
    ///
    /// # Errors
    /// Rejects pending persistence and recovery.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = forgetPeer))]
    pub fn forget_peer(&mut self, peer: &str) -> Result<(), String> {
        self.mutable()?;
        self.document.working_mut().forget_peer(peer);
        Ok(())
    }
    /// Stages session-bound existing-file typing. Host authenticates role and actor.
    /// The text ACK becomes visible only after its own durable source envelope sync.
    ///
    /// # Errors
    /// Rejects forged writers, stale file targets, malformed history and limits.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageTextSync))]
    pub fn stage_text_sync(
        &mut self,
        peer: &str,
        message: &[u8],
        actor: &[u8],
    ) -> Result<String, String> {
        self.mutable()?;
        let mut candidate = self.document.clone();
        candidate
            .working_mut()
            .receive_sync_message_from(peer, message, actor)
            .map_err(error)?;
        self.stage(candidate, None)
    }
    /// Stages session-bound native incremental changes for a host text gateway.
    ///
    /// # Errors
    /// Rejects unknown causal dependencies, foreign writers and non-text targets.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageTextChanges))]
    pub fn stage_text_changes(
        &mut self,
        changes_json: &str,
        actor: &[u8],
    ) -> Result<String, String> {
        self.mutable()?;
        let changes: Vec<Vec<u8>> = parse(changes_json)?;
        let mut candidate = self.document.clone();
        candidate
            .working_mut()
            .apply_changes_from(&changes, actor)
            .map_err(error)?;
        self.stage(candidate, None)
    }
    /// Host-owned external mirror/reconciliation edits, checked against exact text heads.
    /// File lifecycle must already have server admission order; this does not assign it.
    ///
    /// # Errors
    /// Rejects stale offsets, malformed edits and core resource limits.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageHostEdits))]
    pub fn stage_host_edits(
        &mut self,
        expected_json: &str,
        edits_json: &str,
    ) -> Result<String, String> {
        self.mutable()?;
        let expected: TextRevision = parse(expected_json)?;
        let edits: Vec<TextEdit> = parse(edits_json)?;
        let mut candidate = self.document.clone();
        candidate
            .working_mut()
            .apply_edits(&expected, &edits)
            .map_err(error)?;
        self.stage(candidate, None)
    }
    /// Retains an immutable Apply capture without blocking subsequent text stages.
    ///
    /// # Errors
    /// Rejects invalid provenance and exhausted bounded capture storage.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = captureApply))]
    pub fn capture_apply(&mut self) -> Result<String, String> {
        self.mutable()?;
        let capture = self.document.capture_apply().map_err(error)?;
        self.retain_capture(capture)
    }
    /// Restores captured old source only against a trusted journal-authenticated basis.
    ///
    /// # Errors
    /// Rejects forged source/heads/file IDs/accepted basis and handle bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = restoreApplyCapture))]
    pub fn restore_apply_capture(
        &mut self,
        capture_json: &str,
        basis_json: &str,
    ) -> Result<String, String> {
        self.mutable()?;
        let basis = parse(basis_json)?;
        let capture = self
            .document
            .restore_apply_capture(capture_json, &basis)
            .map_err(error)?;
        self.retain_capture(capture)
    }
    /// Returns whether captured source needs compiler-owned three-way rebase.
    ///
    /// # Errors
    /// Rejects unknown or foreign capture handles.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = applyNeedsRebase))]
    pub fn apply_needs_rebase(&self, capture_id: &str) -> Result<bool, String> {
        let capture = self
            .captures
            .get(capture_id)
            .ok_or("unknown source capture")?;
        self.document
            .apply_needs_rebase(&capture.capture)
            .map_err(error)
    }
    /// Prepares exact compiler-generated canvas source; model solving occurs outside
    /// this handle, and this retained ticket does not block typing while it runs.
    ///
    /// # Errors
    /// Rejects forged patches, unknown files and bounded ticket capacity.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = prepareCanvasUpdate))]
    pub fn prepare_canvas_update(&mut self, patches_json: &str) -> Result<String, String> {
        self.mutable()?;
        let patches: Vec<FilePatch> = parse(patches_json)?;
        let entries = patches
            .into_iter()
            .map(|item| AcceptedFilePatch {
                path: item.path,
                patch: item.patch,
            })
            .collect::<Vec<_>>();
        let prepared = self
            .document
            .prepare_canvas_update(&entries)
            .map_err(error)?;
        self.retain_prepared(prepared, 0)
    }
    /// Prepares immutable captured Apply after host compiler rebase against accepted source.
    ///
    /// # Errors
    /// Rejects foreign captures, altered uncrossed captures and candidate bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = prepareApplyUpdate))]
    pub fn prepare_apply_update(
        &mut self,
        capture_id: &str,
        files_json: &str,
    ) -> Result<String, String> {
        self.mutable()?;
        let capture = self
            .captures
            .get(capture_id)
            .ok_or("unknown source capture")?;
        let extra = capture.bytes;
        let files = parse(files_json)?;
        let prepared = self
            .document
            .prepare_apply_update(&capture.capture, files)
            .map_err(error)?;
        self.retain_prepared(prepared, extra)
    }
    /// Releases disposable bounded capture/preparation storage; committed source is unchanged.
    ///
    /// # Errors
    /// Rejects release during a pending persistence transaction or recovery.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = releaseHandle))]
    pub fn release_handle(&mut self, handle: &str) -> Result<bool, String> {
        self.mutable()?;
        Ok(self.captures.remove(handle).is_some() || self.prepared.remove(handle).is_some())
    }
    /// Stages a trusted independently validated model candidate against LATEST working
    /// heads. Host persists this envelope/model with its authority terminal event.
    ///
    /// # Errors
    /// Rejects stale model/text bases, unknown tickets and incomplete reconciliation.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageValidatedPublication))]
    pub fn stage_validated_publication(
        &mut self,
        ticket: &str,
        accepted_input: &str,
        working_json: &str,
        reconciliations_json: &str,
    ) -> Result<String, String> {
        self.mutable()?;
        let entry = self
            .prepared
            .get(ticket)
            .ok_or("unknown source preparation ticket")?;
        let expected: TextRevision = parse(working_json)?;
        let updates: Vec<Reconciliation> = parse(reconciliations_json)?;
        let mut candidate = self.document.clone();
        candidate
            .publish_validated(
                &entry.prepared,
                accepted_input.into(),
                &expected,
                &updates
                    .into_iter()
                    .map(WorkingSourceChange::from)
                    .collect::<Vec<_>>(),
            )
            .map_err(error)?;
        self.stage(candidate, Some(ticket.into()))
    }
    /// Installs a staged source only after host durable append+sync completes.
    ///
    /// # Errors
    /// Rejects wrong/reused stage handles, changed basis and recovery.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = commitStage))]
    pub fn commit_stage(&mut self, stage_id: &str) -> Result<String, String> {
        self.healthy()?;
        let pending = self
            .pending
            .as_ref()
            .filter(|item| item.id == stage_id)
            .ok_or("unknown source stage")?;
        if pending.basis_sequence != self.sequence {
            return Err("source stage basis changed".into());
        }
        let response = Self::snapshot_of(&pending.candidate, pending.sequence, false, false)?;
        let pending = self.pending.take().ok_or("source stage absent")?;
        self.document = pending.candidate;
        self.sequence = pending.sequence;
        if let Some(ticket) = pending.terminal_ticket {
            self.prepared.remove(&ticket);
        }
        Ok(response)
    }
    /// An uncertain write poisons the entire source handle until durable reconstruction.
    ///
    /// # Errors
    /// Rejects wrong/reused stage IDs without dropping the actual pending stage.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = failStage))]
    pub fn fail_stage(&mut self, stage_id: &str) -> Result<(), String> {
        if self.pending.as_ref().is_none_or(|item| item.id != stage_id) {
            return Err("unknown source stage".into());
        }
        self.poisoned = true;
        self.pending = None;
        self.captures.clear();
        self.prepared.clear();
        Ok(())
    }
}
impl TrustedSourceHost {
    fn from_document(
        document: SourceDocument,
        config: Configuration,
        sequence: u64,
    ) -> Result<Self, String> {
        if config.server_epoch.is_empty()
            || config.server_epoch.len() > 128
            || !config
                .server_epoch
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
        {
            return Err("invalid source process epoch".into());
        }
        let instance = NEXT_SOURCE_HOST
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "source host instance limit")?;
        Ok(Self {
            document,
            document_epoch: config.document_epoch,
            server_epoch: config.server_epoch,
            instance,
            sequence,
            next_handle: 0,
            captures: BTreeMap::new(),
            prepared: BTreeMap::new(),
            pending: None,
            poisoned: false,
        })
    }
    fn healthy(&self) -> Result<(), String> {
        if self.poisoned {
            Err("source requires durable recovery".into())
        } else {
            Ok(())
        }
    }
    fn mutable(&self) -> Result<(), String> {
        self.healthy()?;
        if self.pending.is_some() {
            Err("source persistence is pending".into())
        } else {
            Ok(())
        }
    }
    fn handle(&mut self, kind: &str) -> Result<String, String> {
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .filter(|n| *n <= MAX_REVISION)
            .ok_or("source handle counter exhausted")?;
        Ok(format!(
            "{}:{}:{}:{}:{}",
            self.document_epoch, self.server_epoch, self.instance, kind, self.next_handle
        ))
    }
    fn capacity(&self, bytes: usize) -> Result<(), String> {
        let retained = self
            .captures
            .values()
            .map(|item| item.bytes)
            .chain(self.prepared.values().map(|item| item.bytes))
            .sum::<usize>();
        if self.captures.len() + self.prepared.len() >= MAX_HANDLES
            || retained.saturating_add(bytes) > MAX_HANDLE_BYTES
        {
            return Err(
                "source capture/preparation capacity exhausted; release old handles".into(),
            );
        }
        Ok(())
    }
    fn retain_capture(&mut self, capture: ApplyCapture) -> Result<String, String> {
        let encoded = self
            .document
            .encode_apply_capture(&capture)
            .map_err(error)?;
        let bytes = encoded.len();
        self.capacity(bytes)?;
        let handle = self.handle("capture")?;
        let response = json(
            &serde_json::json!({"handle":handle,"captureJson":encoded,"acceptedBasis":capture.accepted_basis(),"working":capture.working(),"fileIds":capture.file_ids()}),
        )?;
        self.captures
            .insert(handle, CaptureEntry { capture, bytes });
        Ok(response)
    }
    fn retain_prepared(
        &mut self,
        prepared: PreparedSourceUpdate,
        extra: usize,
    ) -> Result<String, String> {
        let payload = serde_json::json!({"acceptedBasis":prepared.accepted_basis(),"candidateFiles":prepared.candidate_files(),"changedPaths":prepared.changed_paths()});
        let bytes = json(&payload)?.len().saturating_add(extra);
        self.capacity(bytes)?;
        let ticket = self.handle("prepared")?;
        let response = json(
            &serde_json::json!({"ticket":ticket,"acceptedBasis":prepared.accepted_basis(),"candidateFiles":prepared.candidate_files(),"changedPaths":prepared.changed_paths()}),
        )?;
        self.prepared
            .insert(ticket, PreparedEntry { prepared, bytes });
        Ok(response)
    }
    fn envelope(&self, document: &SourceDocument, sequence: u64) -> Result<String, String> {
        let encoded = json(&Envelope {
            format: "geosolve-source-host-v1".into(),
            document_epoch: self.document_epoch.clone(),
            sequence,
            source_json: document.to_json().map_err(error)?,
        })?;
        if encoded.len() > MAX_ENVELOPE_BYTES {
            return Err("source envelope byte limit".into());
        }
        Ok(encoded)
    }
    fn stage(
        &mut self,
        candidate: SourceDocument,
        terminal_ticket: Option<String>,
    ) -> Result<String, String> {
        let sequence = self
            .sequence
            .checked_add(1)
            .filter(|n| *n <= MAX_REVISION)
            .ok_or("source sequence exhausted")?;
        let checkpoint = self.envelope(&candidate, sequence)?;
        let id = format!(
            "{}:{}:{}",
            self.server_epoch,
            self.instance,
            source_digest(&checkpoint)
        );
        let response = json(
            &serde_json::json!({"status":"staged","stageId":id,"sequence":sequence,"checkpointJson":checkpoint,"accepted":candidate.accepted(),"workingRevision":candidate.working().revision()}),
        )?;
        self.pending = Some(PendingSource {
            id,
            basis_sequence: self.sequence,
            candidate,
            sequence,
            terminal_ticket,
        });
        Ok(response)
    }
    fn snapshot_of(
        document: &SourceDocument,
        sequence: u64,
        has_pending: bool,
        poisoned: bool,
    ) -> Result<String, String> {
        let capture = document.capture_apply().map_err(error)?;
        json(
            &serde_json::json!({"sequence":sequence,"accepted":document.accepted(),"working":capture.working(),"fileIds":capture.file_ids(),"pendingNotices":document.reconciliation_pending(),"hasPendingStage":has_pending,"needsRecovery":poisoned}),
        )
    }
}
