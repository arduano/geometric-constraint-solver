// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin JSON/binary adapter over the shared Rust text owner. No JavaScript CRDT.

pub mod authority;
pub use authority::TrustedDocumentHost;
pub mod source;
pub use source::TrustedSourceHost;
pub mod semantic;
pub use semantic::TrustedSemanticHost;

use std::collections::VecDeque;

use geosolve_collaboration::{
    CursorDeletionBias, SharedTextDocument, SharedTextError, SharedTextLimits, TextCursor,
    TextEdit, TextRangeAnchor, TextRevision, TextUndoToken,
};
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_UNDO_ENTRIES: usize = 64;
const MAX_UNDO_BYTES: usize = 4 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

/// Explicitly owned local replica. Hosts bind actors to authenticated sessions.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Debug)]
pub struct SharedTextReplica {
    document: SharedTextDocument,
    undo: VecDeque<TextUndoToken>,
    redo: VecDeque<TextUndoToken>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl SharedTextReplica {
    /// Creates a genesis; joiners must load or fork that shared history.
    ///
    /// # Errors
    /// Rejects invalid actor bytes or bounded-limit JSON.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(actor: &[u8], limits_json: &str) -> Result<Self, String> {
        Ok(Self::from_document(
            SharedTextDocument::new(actor, limits(limits_json)?).map_err(error)?,
        ))
    }

    /// Loads a complete native checkpoint, retaining the same Rust validation.
    ///
    /// # Errors
    /// Rejects malformed, oversized or schema-invalid bytes.
    pub fn load(bytes: &[u8], actor: &[u8], limits_json: &str) -> Result<Self, String> {
        Ok(Self::from_document(
            SharedTextDocument::load(bytes, actor, limits(limits_json)?).map_err(error)?,
        ))
    }

    /// Forks another writer without copying a connection's disposable handshakes.
    ///
    /// # Errors
    /// Rejects malformed or identical actors.
    pub fn fork(&self, actor: &[u8]) -> Result<Self, String> {
        Ok(Self::from_document(
            self.document.fork(actor).map_err(error)?,
        ))
    }

    pub fn save(&self) -> Vec<u8> {
        self.document.save()
    }

    /// Captures exact raw source and causal heads, including syntax-invalid text.
    ///
    /// # Errors
    /// Reports serialization failure.
    pub fn capture(&self) -> Result<String, String> {
        json(&self.document.capture())
    }

    /// Applies revision-checked edits. Splice-only batches retain a bounded,
    /// contribution-local checked inverse; lifecycle actions use server ordering.
    ///
    /// # Errors
    /// Rejects stale or invalid edits transactionally.
    pub fn edit(&mut self, revision_json: &str, edits_json: &str) -> Result<String, String> {
        let revision: TextRevision = parse(revision_json)?;
        let edits: Vec<TextEdit> = parse(edits_json)?;
        if !edits.is_empty()
            && edits
                .iter()
                .all(|edit| matches!(edit, TextEdit::Splice { .. }))
        {
            match self.document.edit_undoable(&revision, &edits) {
                Ok((_, inverse)) => push_bounded(&mut self.undo, inverse),
                Err(SharedTextError::ResourceLimit("anchored range bytes" | "undo bytes")) => {
                    self.document
                        .apply_edits(&revision, &edits)
                        .map_err(error)?;
                    self.undo.clear();
                }
                Err(failure) => return Err(error(failure)),
            }
        } else {
            self.document
                .apply_edits(&revision, &edits)
                .map_err(error)?;
        }
        self.redo.clear();
        self.capture()
    }

    /// Generates one binary message. Reconnect/drop recovery resets both peers.
    ///
    /// # Errors
    /// Rejects invalid peer identifiers and sync resource bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = generateSyncMessage))]
    pub fn generate_sync_message(&mut self, peer: &str) -> Result<Option<Vec<u8>>, String> {
        self.document.generate_sync_message(peer).map_err(error)
    }

    /// Receives trusted server history. The server uses the actor-bound variant.
    ///
    /// # Errors
    /// Rejects malformed messages and invalid history atomically.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = receiveSyncMessage))]
    pub fn receive_sync_message(&mut self, peer: &str, bytes: &[u8]) -> Result<String, String> {
        self.document
            .receive_sync_message(peer, bytes)
            .map_err(error)?;
        self.capture()
    }

    /// Authenticates every novel contribution against the server-issued actor.
    ///
    /// # Errors
    /// Also rejects novel foreign-writer history.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = receiveSyncMessageFrom))]
    pub fn receive_sync_message_from(
        &mut self,
        peer: &str,
        bytes: &[u8],
        actor: &[u8],
    ) -> Result<String, String> {
        self.document
            .receive_sync_message_from(peer, bytes, actor)
            .map_err(error)?;
        self.capture()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = forgetPeer))]
    pub fn forget_peer(&mut self, peer: &str) {
        self.document.forget_peer(peer);
    }

    /// Exports native incremental history for independently durable text ACKs.
    ///
    /// # Errors
    /// Rejects unknown revision heads or exceeded bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = changesSince))]
    pub fn changes_since(&self, revision_json: &str) -> Result<String, String> {
        json(
            &self
                .document
                .changes_since(&parse(revision_json)?)
                .map_err(error)?,
        )
    }

    /// Imports session-owned native change bytes.
    ///
    /// # Errors
    /// Rejects unknown dependencies, invalid history or forged actors atomically.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = applyChangesFrom))]
    pub fn apply_changes_from(
        &mut self,
        changes_json: &str,
        actor: &[u8],
    ) -> Result<String, String> {
        self.document
            .apply_changes_from(&parse::<Vec<Vec<u8>>>(changes_json)?, actor)
            .map_err(error)?;
        self.capture()
    }

    /// Creates a stable cursor. Bias controls deletion fallback, not insertion affinity.
    ///
    /// # Errors
    /// Rejects invalid boundaries and files.
    pub fn cursor(&self, path: &str, utf16: usize, after: bool) -> Result<String, String> {
        let bias = if after {
            CursorDeletionBias::After
        } else {
            CursorDeletionBias::Before
        };
        json(&self.document.cursor(path, utf16, bias).map_err(error)?)
    }

    /// Resolves current path/position, including file renames.
    ///
    /// # Errors
    /// Rejects removed/recreated files or invalid anchors.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = resolveCursor))]
    pub fn resolve_cursor(&self, cursor_json: &str) -> Result<String, String> {
        json(
            &self
                .document
                .resolve_cursor_location(&parse::<TextCursor>(cursor_json)?)
                .map_err(error)?,
        )
    }

    /// Returns stable file identity, separate from the mutable path.
    ///
    /// # Errors
    /// Rejects absent/invalid paths.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = fileId))]
    pub fn file_id(&self, path: &str) -> Result<String, String> {
        self.document.file_id(path).map_err(error)
    }

    /// Captures exact ownership for a later localized source writeback.
    ///
    /// # Errors
    /// Rejects stale revisions, invalid ranges or exceeded bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = anchorRange))]
    pub fn anchor_range(
        &self,
        revision_json: &str,
        path: &str,
        start: usize,
        end: usize,
    ) -> Result<String, String> {
        json(
            &self
                .document
                .anchor_range(&parse(revision_json)?, path, start, end)
                .map_err(error)?,
        )
    }

    /// Applies a source writeback only while the original span still owns it.
    ///
    /// # Errors
    /// Rejects lost ownership without overwriting the current draft.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = replaceRange))]
    pub fn replace_range(&mut self, anchor_json: &str, insert: &str) -> Result<String, String> {
        self.document
            .replace_range(&parse::<TextRangeAnchor>(anchor_json)?, insert)
            .map_err(error)?;
        self.capture()
    }

    /// Undoes this writer's latest surviving contribution only.
    ///
    /// # Errors
    /// Reports absent history or concurrent overlap without mutation.
    pub fn undo(&mut self) -> Result<String, String> {
        let token = self
            .undo
            .back()
            .ok_or("no local text contribution to undo")?;
        let inverse = self.document.apply_inverse(token).map_err(error)?;
        self.undo.pop_back();
        push_bounded(&mut self.redo, inverse);
        self.capture()
    }

    /// Redoes through the same checked inverse ownership rules.
    ///
    /// # Errors
    /// Reports absent history or overlap without mutation.
    pub fn redo(&mut self) -> Result<String, String> {
        let token = self
            .redo
            .back()
            .ok_or("no local text contribution to redo")?;
        let inverse = self.document.apply_inverse(token).map_err(error)?;
        self.redo.pop_back();
        push_bounded(&mut self.undo, inverse);
        self.capture()
    }

    /// History availability is advisory; remote overlap can still refuse an inverse.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = undoCount))]
    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = redoCount))]
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }
}
impl SharedTextReplica {
    fn from_document(document: SharedTextDocument) -> Self {
        Self {
            document,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        }
    }
}
fn push_bounded(stack: &mut VecDeque<TextUndoToken>, token: TextUndoToken) {
    stack.push_back(token);
    while stack.len() > MAX_UNDO_ENTRIES
        || stack
            .iter()
            .map(TextUndoToken::retained_bytes)
            .sum::<usize>()
            > MAX_UNDO_BYTES
    {
        stack.pop_front();
    }
}
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn json(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(error)
}
fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    if text.len() > MAX_JSON_BYTES {
        return Err("JSON input exceeds collaboration adapter limit".into());
    }
    serde_json::from_str(text).map_err(error)
}
fn limits(text: &str) -> Result<SharedTextLimits, String> {
    if text.is_empty() {
        Ok(SharedTextLimits::default())
    } else {
        parse(text)
    }
}
