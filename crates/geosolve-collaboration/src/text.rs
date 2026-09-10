// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded raw source with explicit UTF-16 indexing. Text may be syntax-invalid.
//!
//! Edits and imports stage before admission. Limits bound input, visible text,
//! change count and retained uncompressed history. Hosts processing untrusted
//! binary data must additionally bound decoder CPU/memory externally. File
//! lifecycle edits require server ordering; existing-file text edits use CRDT
//! merging, while conflicting file objects are explicitly refused.

use std::collections::{BTreeMap, BTreeSet};

use automerge::sync::{Message, State, SyncDoc};
use automerge::{
    ActorId, Automerge, Change, ChangeHash, Cursor, CursorPosition, LoadOptions, MoveCursor, ObjId,
    ObjType, ROOT, ReadDoc, SaveOptions, TextEncoding, Value, transaction::Transactable,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod history;
pub use history::{TextContributionHistory, TextHistoryEvent, TextHistoryHorizon, UserTextHistory};

const FORMAT: &str = "geosolve-shared-text-v1";
const MAX_ACTOR_BYTES: usize = 64;
const MAX_PATH_BYTES: usize = 512;
const MAX_EDIT_BATCH: usize = 256;

/// Admission bounds for local edits, imports and checkpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SharedTextLimits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_history_bytes: usize,
    pub max_changes: usize,
    pub max_operations: u64,
    pub max_sync_bytes: usize,
    pub max_peers: usize,
}
impl Default for SharedTextLimits {
    fn default() -> Self {
        Self {
            max_files: 512,
            max_file_bytes: 4 * 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024,
            max_history_bytes: 64 * 1024 * 1024,
            max_changes: 100_000,
            max_operations: 2_000_000,
            max_sync_bytes: 8 * 1024 * 1024,
            max_peers: 64,
        }
    }
}
impl SharedTextLimits {
    fn validate(self) -> Result<(), SharedTextError> {
        if [
            self.max_files,
            self.max_file_bytes,
            self.max_total_bytes,
            self.max_history_bytes,
            self.max_changes,
            self.max_sync_bytes,
            self.max_peers,
        ]
        .contains(&0)
            || self.max_operations == 0
        {
            return Err(SharedTextError::InvalidLimits);
        }
        Ok(())
    }
}

/// Exact immutable causal frontier. Identifies text, never accepted geometry.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRevision {
    heads: Vec<String>,
}
impl TextRevision {
    pub fn heads(&self) -> &[String] {
        &self.heads
    }
}

/// Raw files and their causal frontier, independent of subsequent edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SharedTextSnapshot {
    revision: TextRevision,
    files: BTreeMap<String, String>,
}
impl SharedTextSnapshot {
    pub const fn revision(&self) -> &TextRevision {
        &self.revision
    }
    pub const fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }
    pub fn text(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }
}

/// Result of typing from a displayed frontier. The local branch is the text the
/// editor has actually displayed after its own edit, before unseen remote merges.
/// Queue the next local offsets against this revision until remote text is installed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTextEdit {
    pub snapshot: SharedTextSnapshot,
    pub local_revision: TextRevision,
}

/// A raw edit; splice coordinates count UTF-16 code units.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextEdit {
    CreateFile {
        path: String,
        text: String,
    },
    Splice {
        path: String,
        start_utf16: usize,
        delete_utf16: usize,
        insert: String,
    },
    RemoveFile {
        path: String,
    },
    RenameFile {
        path: String,
        new_path: String,
    },
}

/// Surviving neighbour used when an anchor's original character is deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorDeletionBias {
    Before,
    After,
}

/// Stable anchor scoped to its original file object, not only its path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCursor {
    path: String,
    object: String,
    encoded: Vec<u8>,
}

/// A stable cursor resolved in the current file namespace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextLocation {
    pub path: String,
    pub utf16: usize,
}
/// A captured scalar-aligned source span and its exact original bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRangeAnchor {
    start: TextCursor,
    end: TextCursor,
    expected: String,
    ownership: Vec<u8>,
}
/// Checked inverse of one local contribution, never a whole-document snapshot.
#[derive(Clone, Debug)]
pub struct TextUndoToken {
    actor: Vec<u8>,
    inverses: Vec<(TextRangeAnchor, String)>,
}
impl TextUndoToken {
    /// Conservative retained-payload accounting for a host's bounded Undo stack.
    pub fn retained_bytes(&self) -> usize {
        self.actor.len()
            + self
                .inverses
                .iter()
                .map(|(anchor, insert)| {
                    anchor.expected.len()
                        + insert.len()
                        + anchor.start.path.len()
                        + anchor.end.path.len()
                        + anchor.start.object.len()
                        + anchor.end.object.len()
                        + anchor.start.encoded.len()
                        + anchor.end.encoded.len()
                        + anchor.ownership.len()
                        + 256
                })
                .sum::<usize>()
    }
}
/// Current scalar-aligned positions of an authenticated source span.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRange {
    pub path: String,
    pub start_utf16: usize,
    pub end_utf16: usize,
}

/// Refusals never modify an existing document or its retained history.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SharedTextError {
    #[error("shared text limits must be positive")]
    InvalidLimits,
    #[error("actor must contain 1..=64 bytes; forks need a distinct actor")]
    InvalidActor,
    #[error("invalid local source path: {0}")]
    InvalidPath(String),
    #[error("source file already exists: {0}")]
    FileExists(String),
    #[error("source file is absent: {0}")]
    MissingFile(String),
    #[error("conflicting file objects require document-level resolution: {0}")]
    FileConflict(String),
    #[error("invalid shared text schema: {0}")]
    InvalidSchema(String),
    #[error("shared text resource limit: {0}")]
    ResourceLimit(&'static str),
    #[error("text position is outside the source or splits a Unicode scalar")]
    InvalidPosition,
    #[error("text revision contains unknown, duplicate or malformed heads")]
    InvalidRevision,
    #[error("text offsets were prepared against another revision")]
    StaleRevision,
    #[error("peer identifier must contain 1..=128 non-control bytes")]
    InvalidPeer,
    #[error("received changes contain an unauthenticated writer")]
    ActorMismatch,
    #[error("file lifecycle changes require server ordering")]
    FileLifecycleRequiresOrder,
    #[error("typing targets an absent/replaced file or non-text object")]
    InvalidTextTarget,
    #[error("source range lost its captured ownership")]
    RangeConflict,
    #[error("checked inverse unavailable for this actor or file lifecycle edit")]
    UndoUnavailable,
    #[error("text changes have missing causal dependencies")]
    MissingDependencies,
    #[error("cursor does not belong to the current source file")]
    InvalidCursor,
    #[error("invalid Automerge data: {0}")]
    InvalidData(String),
}
impl From<automerge::AutomergeError> for SharedTextError {
    fn from(error: automerge::AutomergeError) -> Self {
        Self::InvalidData(error.to_string())
    }
}

/// A bounded Automerge source replica. Clone retains its writer for staging;
/// use `fork` for another independently writable replica. Hosts own actor uniqueness.
#[derive(Clone, Debug)]
pub struct SharedTextDocument {
    document: Automerge,
    limits: SharedTextLimits,
    peers: BTreeMap<String, State>,
}
impl SharedTextDocument {
    /// Creates an empty source tree. Participants must fork or load this genesis.
    ///
    /// # Errors
    /// Rejects invalid actors, limits or insufficient history capacity.
    pub fn new(actor: &[u8], limits: SharedTextLimits) -> Result<Self, SharedTextError> {
        limits.validate()?;
        let mut document =
            Automerge::new_with_encoding(TextEncoding::Utf16CodeUnit).with_actor(actor_id(actor)?);
        let mut transaction = document.transaction();
        transaction.put(ROOT, "format", FORMAT)?;
        transaction.put_object(ROOT, "files", ObjType::Map)?;
        transaction.commit();
        Self::admit(document, limits)
    }
    pub const fn limits(&self) -> SharedTextLimits {
        self.limits
    }
    pub fn revision(&self) -> TextRevision {
        let mut heads = self
            .document
            .get_heads()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        heads.sort();
        TextRevision { heads }
    }
    /// Captures exact raw bytes and heads without executing source code.
    ///
    /// # Panics
    /// Only if this module's privately admitted schema invariant is broken.
    pub fn capture(&self) -> SharedTextSnapshot {
        SharedTextSnapshot {
            revision: self.revision(),
            files: read_files(&self.document, self.limits).expect("admitted shared text schema"),
        }
    }
    /// Reconstructs exact source at an authenticated historical causal frontier.
    /// No current typing or accepted-model state is substituted for those heads.
    ///
    /// # Errors
    /// Rejects unknown/invalid heads, invalid historical schema and resource bounds.
    pub fn snapshot_at(
        &self,
        revision: &TextRevision,
    ) -> Result<SharedTextSnapshot, SharedTextError> {
        let historical = self.document.fork_at(&self.validate_revision(revision)?)?;
        let mut heads = historical
            .get_heads()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        heads.sort();
        Ok(SharedTextSnapshot {
            revision: TextRevision { heads },
            files: read_files(&historical, self.limits)?,
        })
    }
    /// Reconstructs stable file identities at the same historical causal frontier.
    ///
    /// # Errors
    /// Rejects unknown heads, invalid historical schema and resource bounds.
    pub fn file_ids_at(
        &self,
        revision: &TextRevision,
    ) -> Result<BTreeMap<String, String>, SharedTextError> {
        let historical = self.document.fork_at(&self.validate_revision(revision)?)?;
        read_files(&historical, self.limits)?;
        Ok(file_layout(&historical)?
            .into_iter()
            .map(|(id, path)| (path, id))
            .collect())
    }
    /// Applies a batch in order as one change; only the complete result is installed.
    ///
    /// # Errors
    /// Rejects invalid paths, lifecycle targets, Unicode positions or limits.
    pub fn edit(&mut self, edits: &[TextEdit]) -> Result<TextRevision, SharedTextError> {
        if edits.len() > MAX_EDIT_BATCH {
            return Err(SharedTextError::ResourceLimit("edit batch"));
        }
        let mut document = self.document.clone();
        let files = files_object(&document)?;
        let actor = document.get_actor().to_hex_string();
        let operation_count = document.stats().num_ops;
        let mut transaction = document.transaction();
        for (index, edit) in edits.iter().enumerate() {
            let file_key = format!("{actor}:{operation_count}:{index}");
            apply_edit(&mut transaction, &files, edit, self.limits, &file_key)?;
        }
        transaction.commit();
        self.install(document)?;
        Ok(self.revision())
    }
    /// Applies offsets only to the immutable revision they were prepared against.
    ///
    /// # Errors
    /// Rejects stale revisions or any inadmissible edit without partial mutation.
    pub fn apply_edits(
        &mut self,
        expected: &TextRevision,
        edits: &[TextEdit],
    ) -> Result<SharedTextSnapshot, SharedTextError> {
        if &self.revision() != expected {
            return Err(SharedTextError::StaleRevision);
        }
        self.edit(edits)?;
        Ok(self.capture())
    }
    /// Applies existing-file typing to the actual displayed causal frontier and
    /// merges its native character operations into the latest remote state.
    /// Unseen changes by this same actor reject to prevent actor-sequence forks.
    ///
    /// # Errors
    /// Rejects unknown heads, unseen own edits, lifecycle edits, replaced files,
    /// invalid UTF-16 positions and normal admission bounds atomically.
    pub fn edit_from_revision(
        &mut self,
        expected: &TextRevision,
        edits: &[TextEdit],
    ) -> Result<HistoricalTextEdit, SharedTextError> {
        let mut historical = self.typing_basis(expected, edits)?;
        historical.apply_edits(expected, edits)?;
        let local_revision = historical.revision();
        let novel = historical.changes_since(expected)?;
        let actor = self.document.get_actor().to_bytes().to_vec();
        self.apply_changes_from(&novel, &actor)?;
        Ok(HistoricalTextEdit {
            snapshot: self.capture(),
            local_revision,
        })
    }

    fn typing_basis(
        &self,
        expected: &TextRevision,
        edits: &[TextEdit],
    ) -> Result<Self, SharedTextError> {
        if edits
            .iter()
            .any(|edit| !matches!(edit, TextEdit::Splice { .. }))
        {
            return Err(SharedTextError::FileLifecycleRequiresOrder);
        }
        let heads = self.validate_revision(expected)?;
        if self
            .document
            .get_changes(&heads)
            .iter()
            .any(|change| change.actor_id() == self.document.get_actor())
        {
            return Err(SharedTextError::StaleRevision);
        }
        let mut document = self.document.fork_at(&heads)?;
        document.set_actor(self.document.get_actor().clone());
        Self::admit(document, self.limits)
    }

    /// Updates a file through the smallest contiguous scalar-aligned splice.
    ///
    /// # Errors
    /// Rejects stale revisions, paths and resource bounds atomically.
    pub fn set_file(
        &mut self,
        expected: &TextRevision,
        path: &str,
        text: &str,
    ) -> Result<SharedTextSnapshot, SharedTextError> {
        if &self.revision() != expected {
            return Err(SharedTextError::StaleRevision);
        }
        let snapshot = self.capture();
        let edit = if let Some(old) = snapshot.text(path) {
            let prefix = old
                .chars()
                .zip(text.chars())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c.len_utf8())
                .sum::<usize>();
            let suffix = old[prefix..]
                .chars()
                .rev()
                .zip(text[prefix..].chars().rev())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c.len_utf8())
                .sum::<usize>();
            TextEdit::Splice {
                path: path.into(),
                start_utf16: utf8_to_utf16(old, prefix)?,
                delete_utf16: old[prefix..old.len() - suffix].encode_utf16().count(),
                insert: text[prefix..text.len() - suffix].into(),
            }
        } else {
            TextEdit::CreateFile {
                path: path.into(),
                text: text.into(),
            }
        };
        self.apply_edits(expected, &[edit])
    }
    /// Generates the pinned Automerge sync protocol for one connection.
    /// Reset both peers after reconnect or dropped generated messages.
    ///
    /// # Errors
    /// Rejects invalid peers and oversized output without advancing the handshake.
    pub fn generate_sync_message(
        &mut self,
        peer: &str,
    ) -> Result<Option<Vec<u8>>, SharedTextError> {
        let mut state = self.peer_state(peer)?;
        let bytes = self
            .document
            .generate_sync_message(&mut state)
            .map(Message::encode);
        if let Some(bytes) = &bytes {
            check_bytes([bytes.len()], self.limits.max_sync_bytes, "sync bytes")?;
        }
        self.peers.insert(peer.into(), state);
        Ok(bytes)
    }
    /// Admits a trusted binary sync message atomically. Server ingress should use
    /// `receive_sync_message_from` to bind novel contributions to the session actor.
    ///
    /// # Errors
    /// Rejects malformed, noncanonical, oversized or invalid-schema history.
    pub fn receive_sync_message(
        &mut self,
        peer: &str,
        bytes: &[u8],
    ) -> Result<TextRevision, SharedTextError> {
        self.receive_sync(peer, bytes, None)
    }
    /// Admits existing-file text changes attributed to the authenticated session actor.
    /// Existing changes may be relayed by any peer without acquiring authorship.
    ///
    /// # Errors
    /// Also rejects a novel contribution from any other writer or file lifecycle changes.
    pub fn receive_sync_message_from(
        &mut self,
        peer: &str,
        bytes: &[u8],
        actor: &[u8],
    ) -> Result<TextRevision, SharedTextError> {
        self.receive_sync(peer, bytes, Some(actor_id(actor)?))
    }
    /// Drops disposable connection state; source and history remain unchanged.
    pub fn forget_peer(&mut self, peer: &str) {
        self.peers.remove(peer);
    }
    fn receive_sync(
        &mut self,
        peer: &str,
        bytes: &[u8],
        actor: Option<ActorId>,
    ) -> Result<TextRevision, SharedTextError> {
        let mut state = self.peer_state(peer)?;
        check_bytes([bytes.len()], self.limits.max_sync_bytes, "sync bytes")?;
        let message = Message::decode(bytes)
            .map_err(|error| SharedTextError::InvalidData(error.to_string()))?;
        if message.clone().encode() != bytes {
            return Err(SharedTextError::InvalidData(
                "noncanonical or trailing sync bytes".into(),
            ));
        }
        let mut document = self.document.clone();
        document.receive_sync_message(&mut state, message)?;
        if let Some(actor) = actor {
            self.validate_new_actor(&document, &actor)?;
        }
        self.install(document)?;
        self.peers.insert(peer.into(), state);
        Ok(self.revision())
    }
    fn validate_new_actor(
        &self,
        document: &Automerge,
        actor: &ActorId,
    ) -> Result<(), SharedTextError> {
        if document.stats().num_ops > self.limits.max_operations {
            return Err(SharedTextError::ResourceLimit("operation count"));
        }
        let changes = self.document.get_changes_added(document);
        if changes.iter().any(|change| change.actor_id() != actor) {
            return Err(SharedTextError::ActorMismatch);
        }
        let layout = file_layout(&self.document)?;
        if layout != file_layout(document)? {
            return Err(SharedTextError::FileLifecycleRequiresOrder);
        }
        for change in changes {
            if change.decode().operations.iter().any(|operation| {
                !layout.contains_key(&operation.obj.to_string()) || operation.key.is_map_key()
            }) {
                return Err(SharedTextError::InvalidTextTarget);
            }
        }
        Ok(())
    }
    fn peer_state(&self, peer: &str) -> Result<State, SharedTextError> {
        if peer.is_empty() || peer.len() > 128 || peer.chars().any(char::is_control) {
            return Err(SharedTextError::InvalidPeer);
        }
        if let Some(state) = self.peers.get(peer) {
            return Ok(state.clone());
        }
        if self.peers.len() >= self.limits.max_peers {
            return Err(SharedTextError::ResourceLimit("peer count"));
        }
        Ok(State::new())
    }
    fn install(&mut self, document: Automerge) -> Result<(), SharedTextError> {
        let mut admitted = Self::admit(document, self.limits)?;
        admitted.peers = std::mem::take(&mut self.peers);
        *self = admitted;
        Ok(())
    }
    /// Creates raw text, including an empty or syntax-invalid file.
    ///
    /// # Errors
    /// Rejects an existing/invalid path or exceeded limits.
    pub fn create_file(&mut self, path: &str, text: &str) -> Result<TextRevision, SharedTextError> {
        self.edit(&[TextEdit::CreateFile {
            path: path.into(),
            text: text.into(),
        }])
    }
    /// Splices at exact UTF-16 boundaries; surrogate interiors are refused.
    ///
    /// # Errors
    /// Rejects missing files, invalid positions and exceeded limits.
    pub fn splice(
        &mut self,
        path: &str,
        start_utf16: usize,
        delete_utf16: usize,
        insert: &str,
    ) -> Result<TextRevision, SharedTextError> {
        self.edit(&[TextEdit::Splice {
            path: path.into(),
            start_utf16,
            delete_utf16,
            insert: insert.into(),
        }])
    }
    /// Removes a file. Hosts serialize file lifecycle intent separately.
    ///
    /// # Errors
    /// Rejects missing/invalid files or exceeded history capacity.
    pub fn remove_file(&mut self, path: &str) -> Result<TextRevision, SharedTextError> {
        self.edit(&[TextEdit::RemoveFile { path: path.into() }])
    }
    /// Renames a file while preserving its text object and stable cursors.
    /// Hosts serialize lifecycle changes against the current document.
    ///
    /// # Errors
    /// Rejects absent sources, occupied destinations or invalid paths.
    pub fn rename_file(
        &mut self,
        path: &str,
        new_path: &str,
    ) -> Result<TextRevision, SharedTextError> {
        self.edit(&[TextEdit::RenameFile {
            path: path.into(),
            new_path: new_path.into(),
        }])
    }
    pub(crate) fn actor(&self) -> &[u8] {
        self.document.get_actor().to_bytes()
    }

    /// Returns the persistent text object identity, independent of its path.
    ///
    /// # Errors
    /// Rejects absent or invalid paths.
    pub fn file_id(&self, path: &str) -> Result<String, SharedTextError> {
        Ok(text_object(&self.document, &files_object(&self.document)?, path)?.to_string())
    }
    /// Forks the same history with another explicit writer identity.
    ///
    /// # Errors
    /// Rejects malformed or identical actor IDs.
    pub fn fork(&self, actor: &[u8]) -> Result<Self, SharedTextError> {
        let actor = actor_id(actor)?;
        if &actor == self.document.get_actor() {
            return Err(SharedTextError::InvalidActor);
        }
        let mut replica = self.clone();
        replica.document.set_actor(actor);
        replica.peers.clear();
        Ok(replica)
    }
    /// Merges remote history through atomic change admission.
    ///
    /// # Errors
    /// Rejects invalid data, conflicting file objects or exceeded limits.
    pub fn merge(&mut self, other: &Self) -> Result<TextRevision, SharedTextError> {
        let changes = self.document.get_changes_added(&other.document);
        self.apply_changes(
            &changes
                .iter()
                .map(|change| change.raw_bytes().to_vec())
                .collect::<Vec<_>>(),
        )
    }
    /// Exports native changes missing from an authenticated causal frontier.
    ///
    /// # Errors
    /// Rejects unknown/malformed heads or an oversized export.
    pub fn changes_since(&self, revision: &TextRevision) -> Result<Vec<Vec<u8>>, SharedTextError> {
        let heads = self.validate_revision(revision)?;
        let changes = self
            .document
            .get_changes(&heads)
            .iter()
            .map(|change| change.raw_bytes().to_vec())
            .collect::<Vec<_>>();
        check_bytes(
            changes.iter().map(Vec::len),
            self.limits.max_history_bytes,
            "change bytes",
        )?;
        Ok(changes)
    }
    /// Applies native changes once; duplicate delivery is idempotent.
    ///
    /// # Errors
    /// Rejects missing dependencies, malformed/schema-invalid changes and limits.
    pub fn apply_changes(&mut self, changes: &[Vec<u8>]) -> Result<TextRevision, SharedTextError> {
        if changes.len() > self.limits.max_changes {
            return Err(SharedTextError::ResourceLimit("change count"));
        }
        check_bytes(
            changes.iter().map(Vec::len),
            self.limits.max_history_bytes,
            "change bytes",
        )?;
        let decoded = changes
            .iter()
            .map(|bytes| {
                Change::from_bytes(bytes.clone())
                    .map_err(|error| SharedTextError::InvalidData(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut document = self.document.clone();
        document.apply_changes(decoded)?;
        self.install(document)?;
        Ok(self.revision())
    }
    /// Saves native Automerge bytes without DEFLATE compression.
    pub fn save(&self) -> Vec<u8> {
        save_document(&self.document)
    }
    /// Restores a complete native checkpoint under host-owned limits.
    ///
    /// # Errors
    /// Rejects partial/corrupt data, invalid schema, actors or exceeded limits.
    pub fn load(
        bytes: &[u8],
        actor: &[u8],
        limits: SharedTextLimits,
    ) -> Result<Self, SharedTextError> {
        limits.validate()?;
        let actor = actor_id(actor)?;
        check_bytes([bytes.len()], limits.max_history_bytes, "checkpoint bytes")?;
        let mut document = Automerge::load_with_options(
            bytes,
            LoadOptions::new().text_encoding(TextEncoding::Utf16CodeUnit),
        )?;
        document.set_actor(actor);
        Self::admit(document, limits)
    }
    /// Creates a stable anchor at a scalar boundary. End follows future text.
    ///
    /// # Errors
    /// Rejects absent files and invalid UTF-16 positions.
    pub fn cursor(
        &self,
        path: &str,
        utf16: usize,
        bias: CursorDeletionBias,
    ) -> Result<TextCursor, SharedTextError> {
        let object = text_object(&self.document, &files_object(&self.document)?, path)?;
        let text = self.document.text(&object)?;
        utf16_to_utf8(&text, utf16)?;
        let position = if utf16 == text.encode_utf16().count() {
            CursorPosition::End
        } else {
            CursorPosition::Index(utf16)
        };
        let movement = match bias {
            CursorDeletionBias::Before => MoveCursor::Before,
            CursorDeletionBias::After => MoveCursor::After,
        };
        let cursor = self
            .document
            .get_cursor_moving(&object, position, None, movement)?;
        Ok(TextCursor {
            path: path.into(),
            object: object.to_string(),
            encoded: cursor.to_bytes(),
        })
    }
    /// Resolves an anchor in the latest version of its original file object.
    ///
    /// # Errors
    /// Rejects foreign/replaced files, malformed anchors and invalid boundaries.
    pub fn resolve_cursor(&self, cursor: &TextCursor) -> Result<usize, SharedTextError> {
        if cursor.encoded.len() > MAX_ACTOR_BYTES + 32
            || cursor.object.len() > MAX_ACTOR_BYTES * 2 + 32
        {
            return Err(SharedTextError::InvalidCursor);
        }
        let (_, object) = self.cursor_file(cursor)?;
        let decoded = Cursor::try_from(cursor.encoded.as_slice())
            .map_err(|_| SharedTextError::InvalidCursor)?;
        let position = self
            .document
            .get_cursor_position(&object, &decoded, None)
            .map_err(|_| SharedTextError::InvalidCursor)?;
        utf16_to_utf8(&self.document.text(&object)?, position)?;
        Ok(position)
    }
    /// Resolves the current path as well as the UTF-16 offset after renames.
    ///
    /// # Errors
    /// Rejects foreign, removed or replaced files and malformed cursors.
    pub fn resolve_cursor_location(
        &self,
        cursor: &TextCursor,
    ) -> Result<TextLocation, SharedTextError> {
        let (path, _) = self.cursor_file(cursor)?;
        Ok(TextLocation {
            path,
            utf16: self.resolve_cursor(cursor)?,
        })
    }
    fn cursor_file(&self, cursor: &TextCursor) -> Result<(String, ObjId), SharedTextError> {
        let files = files_object(&self.document)?;
        for key in self.document.keys(&files) {
            let (_, path, object) = file_entry(&self.document, &files, &key)?;
            if object.to_string() == cursor.object {
                return Ok((path, object));
            }
        }
        Err(SharedTextError::InvalidCursor)
    }
    /// Captures exact character ownership for a later localized source writeback.
    ///
    /// # Errors
    /// Rejects stale revisions, missing files and invalid boundaries.
    pub fn anchor_range(
        &self,
        expected: &TextRevision,
        path: &str,
        start_utf16: usize,
        end_utf16: usize,
    ) -> Result<TextRangeAnchor, SharedTextError> {
        if &self.revision() != expected {
            return Err(SharedTextError::StaleRevision);
        }
        if start_utf16 > end_utf16 {
            return Err(SharedTextError::InvalidPosition);
        }
        let object = text_object(&self.document, &files_object(&self.document)?, path)?;
        let text = self.document.text(object)?;
        let start = utf16_to_utf8(&text, start_utf16)?;
        let end = utf16_to_utf8(&text, end_utf16)?;
        Ok(TextRangeAnchor {
            start: self.cursor(path, start_utf16, CursorDeletionBias::After)?,
            end: self.cursor(path, end_utf16, CursorDeletionBias::After)?,
            expected: text[start..end].into(),
            ownership: self.range_ownership(path, start_utf16, end_utf16)?,
        })
    }
    /// Resolves a source range only while the captured characters still own it.
    /// Unrelated invalid syntax does not interfere; overlap refuses explicitly.
    ///
    /// # Errors
    /// Rejects deleted/replaced anchors or a changed captured span.
    pub fn resolve_range(&self, anchor: &TextRangeAnchor) -> Result<TextRange, SharedTextError> {
        let start = self.resolve_cursor_location(&anchor.start)?;
        let end = TextLocation {
            path: start.path.clone(),
            utf16: start
                .utf16
                .checked_add(anchor.expected.encode_utf16().count())
                .ok_or(SharedTextError::RangeConflict)?,
        };
        if anchor.expected.len() > 64 * 1024
            || anchor.ownership.len() != 32
            || anchor.start.object != anchor.end.object
        {
            return Err(SharedTextError::RangeConflict);
        }
        let snapshot = self.capture();
        let text = snapshot
            .text(&start.path)
            .ok_or(SharedTextError::InvalidCursor)?;
        let start_byte = utf16_to_utf8(text, start.utf16)?;
        let end_byte = utf16_to_utf8(text, end.utf16)?;
        if text[start_byte..end_byte] != anchor.expected
            || self.range_ownership(&start.path, start.utf16, end.utf16)? != anchor.ownership
            || self
                .cursor(&start.path, start.utf16, CursorDeletionBias::After)?
                .encoded
                != anchor.start.encoded
        {
            return Err(SharedTextError::RangeConflict);
        }
        Ok(TextRange {
            path: start.path,
            start_utf16: start.utf16,
            end_utf16: end.utf16,
        })
    }
    /// Applies one owned source writeback to the latest draft atomically.
    ///
    /// # Errors
    /// Refuses lost ownership and preserves the entire draft on failure.
    pub fn replace_range(
        &mut self,
        anchor: &TextRangeAnchor,
        insert: &str,
    ) -> Result<TextRevision, SharedTextError> {
        let range = self.resolve_range(anchor)?;
        self.splice(
            &range.path,
            range.start_utf16,
            range.end_utf16 - range.start_utf16,
            insert,
        )
    }
    fn range_ownership(
        &self,
        path: &str,
        start: usize,
        end: usize,
    ) -> Result<Vec<u8>, SharedTextError> {
        let object = text_object(&self.document, &files_object(&self.document)?, path)?;
        let text = self.document.text(&object)?;
        let span = &text[utf16_to_utf8(&text, start)?..utf16_to_utf8(&text, end)?];
        if span.len() > 64 * 1024 {
            return Err(SharedTextError::ResourceLimit("anchored range bytes"));
        }
        let mut hash = Sha256::new();
        let mut position = start;
        for scalar in span.chars() {
            let cursor = self.document.get_cursor_moving(
                &object,
                CursorPosition::Index(position),
                None,
                MoveCursor::After,
            )?;
            let bytes = cursor.to_bytes();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
            position += scalar.len_utf16();
        }
        Ok(hash.finalize().to_vec())
    }
    /// Applies one user's text contribution and captures its checked inverse.
    /// File lifecycle changes use server ordering and are excluded from draft Undo.
    ///
    /// # Errors
    /// Rejects stale input, non-splice edits, limits and invalid positions atomically.
    pub fn edit_undoable(
        &mut self,
        expected: &TextRevision,
        edits: &[TextEdit],
    ) -> Result<(SharedTextSnapshot, TextUndoToken), SharedTextError> {
        if &self.revision() != expected {
            return Err(SharedTextError::StaleRevision);
        }
        if edits.len() > MAX_EDIT_BATCH {
            return Err(SharedTextError::ResourceLimit("edit batch"));
        }
        let mut paths = BTreeSet::new();
        for edit in edits {
            let TextEdit::Splice { path, .. } = edit else {
                return Err(SharedTextError::UndoUnavailable);
            };
            paths.insert(path);
        }
        let before = self.capture();
        let mut staged = self.clone();
        let after = staged.apply_edits(expected, edits)?;
        let mut inverses = Vec::new();
        for path in paths {
            let old = before
                .text(path)
                .ok_or_else(|| SharedTextError::MissingFile(path.clone()))?;
            let current = after
                .text(path)
                .ok_or_else(|| SharedTextError::MissingFile(path.clone()))?;
            if old == current {
                continue;
            }
            let prefix = old
                .chars()
                .zip(current.chars())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c.len_utf8())
                .sum::<usize>();
            let suffix = old[prefix..]
                .chars()
                .rev()
                .zip(current[prefix..].chars().rev())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c.len_utf8())
                .sum::<usize>();
            let original = old[prefix..old.len() - suffix].to_owned();
            if original.len() > 64 * 1024 {
                return Err(SharedTextError::ResourceLimit("undo bytes"));
            }
            let anchor = staged.anchor_range(
                after.revision(),
                path,
                utf8_to_utf16(current, prefix)?,
                utf8_to_utf16(current, current.len() - suffix)?,
            )?;
            inverses.push((anchor, original));
        }
        let token = TextUndoToken {
            actor: self.document.get_actor().to_bytes().to_vec(),
            inverses,
        };
        *self = staged;
        Ok((self.capture(), token))
    }
    /// Undoes only surviving owned spans and returns the checked Redo inverse.
    /// Remote overlaps reject rather than overwrite another contribution.
    ///
    /// # Errors
    /// Rejects a foreign writer or lost range ownership, preserving all source.
    pub fn apply_inverse(
        &mut self,
        token: &TextUndoToken,
    ) -> Result<TextUndoToken, SharedTextError> {
        if self.document.get_actor().to_bytes() != token.actor {
            return Err(SharedTextError::UndoUnavailable);
        }
        let mut staged = self.clone();
        let mut inverse = Vec::new();
        for (anchor, insert) in &token.inverses {
            let range = staged.resolve_range(anchor)?;
            let old = anchor.expected.clone();
            staged.replace_range(anchor, insert)?;
            let next = staged.anchor_range(
                &staged.revision(),
                &range.path,
                range.start_utf16,
                range.start_utf16 + insert.encode_utf16().count(),
            )?;
            inverse.push((next, old));
        }
        inverse.reverse();
        *self = staged;
        Ok(TextUndoToken {
            actor: token.actor.clone(),
            inverses: inverse,
        })
    }
    /// Applies existing-file changes with session-authenticated writer ownership.
    ///
    /// # Errors
    /// Rejects novel foreign-actor contributions or inadmissible history atomically.
    pub fn apply_changes_from(
        &mut self,
        changes: &[Vec<u8>],
        actor: &[u8],
    ) -> Result<TextRevision, SharedTextError> {
        let actor = actor_id(actor)?;
        let mut staged = self.clone();
        staged.apply_changes(changes)?;
        self.validate_new_actor(&staged.document, &actor)?;
        *self = staged;
        Ok(self.revision())
    }
    fn validate_revision(
        &self,
        revision: &TextRevision,
    ) -> Result<Vec<ChangeHash>, SharedTextError> {
        if revision.heads.len() > self.limits.max_changes {
            return Err(SharedTextError::InvalidRevision);
        }
        let heads = revision
            .heads
            .iter()
            .map(|head| head.parse().map_err(|_| SharedTextError::InvalidRevision))
            .collect::<Result<Vec<ChangeHash>, _>>()?;
        if heads.iter().collect::<BTreeSet<_>>().len() != heads.len()
            || heads
                .iter()
                .any(|head| self.document.get_change_by_hash(head).is_none())
        {
            return Err(SharedTextError::InvalidRevision);
        }
        Ok(heads)
    }
    fn admit(document: Automerge, limits: SharedTextLimits) -> Result<Self, SharedTextError> {
        if !document.get_missing_deps(&[]).is_empty() {
            return Err(SharedTextError::MissingDependencies);
        }
        if document.stats().num_ops > limits.max_operations {
            return Err(SharedTextError::ResourceLimit("operation count"));
        }
        let changes = document.get_changes(&[]);
        if changes.len() > limits.max_changes {
            return Err(SharedTextError::ResourceLimit("change count"));
        }
        if changes
            .iter()
            .flat_map(Change::actors)
            .any(|actor| actor.to_bytes().len() > MAX_ACTOR_BYTES || actor.to_bytes().is_empty())
        {
            return Err(SharedTextError::InvalidActor);
        }
        check_bytes(
            changes.iter().map(|change| change.raw_bytes().len()),
            limits.max_history_bytes,
            "history bytes",
        )?;
        read_files(&document, limits)?;
        check_bytes(
            [save_document(&document).len()],
            limits.max_history_bytes,
            "checkpoint bytes",
        )?;
        Ok(Self {
            document,
            limits,
            peers: BTreeMap::new(),
        })
    }
}

fn actor_id(actor: &[u8]) -> Result<ActorId, SharedTextError> {
    if actor.is_empty() || actor.len() > MAX_ACTOR_BYTES {
        return Err(SharedTextError::InvalidActor);
    }
    Ok(ActorId::from(actor))
}
fn validate_path(path: &str) -> Result<(), SharedTextError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.contains(['\\', ':'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| matches!(part, "" | "." | ".." | ".geosolve"))
    {
        return Err(SharedTextError::InvalidPath(path.into()));
    }
    Ok(())
}
fn save_document(document: &Automerge) -> Vec<u8> {
    document.save_with_options(SaveOptions {
        deflate: false,
        retain_orphans: false,
    })
}
fn check_bytes(
    lengths: impl IntoIterator<Item = usize>,
    limit: usize,
    label: &'static str,
) -> Result<(), SharedTextError> {
    let mut total = 0_usize;
    for length in lengths {
        total = total
            .checked_add(length)
            .filter(|value| *value <= limit)
            .ok_or(SharedTextError::ResourceLimit(label))?;
    }
    Ok(())
}
fn files_object(document: &impl ReadDoc) -> Result<ObjId, SharedTextError> {
    let keys = document.keys(ROOT).collect::<BTreeSet<_>>();
    if keys != BTreeSet::from(["files".into(), "format".into()]) {
        return Err(SharedTextError::InvalidSchema("root fields".into()));
    }
    let formats = document.get_all(ROOT, "format")?;
    if formats.is_empty()
        || formats
            .iter()
            .any(|(value, _)| value.to_str() != Some(FORMAT))
    {
        return Err(SharedTextError::InvalidSchema("format".into()));
    }
    let values = document.get_all(ROOT, "files")?;
    match values.as_slice() {
        [(Value::Object(ObjType::Map), object)] => Ok(object.clone()),
        _ => Err(SharedTextError::InvalidSchema("files object".into())),
    }
}
fn file_entry(
    document: &impl ReadDoc,
    files: &ObjId,
    key: &str,
) -> Result<(ObjId, String, ObjId), SharedTextError> {
    if key.is_empty()
        || key.len() > 180
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b':')
    {
        return Err(SharedTextError::InvalidSchema("file identity".into()));
    }
    let values = document.get_all(files, key)?;
    let entry = match values.as_slice() {
        [(Value::Object(ObjType::Map), object)] => object.clone(),
        _ => return Err(SharedTextError::InvalidSchema("file record".into())),
    };
    if document.keys(&entry).collect::<BTreeSet<_>>()
        != BTreeSet::from(["path".into(), "text".into()])
    {
        return Err(SharedTextError::InvalidSchema("file fields".into()));
    }
    let paths = document.get_all(&entry, "path")?;
    let path = match paths.as_slice() {
        [(value, _)] => value
            .to_str()
            .ok_or_else(|| SharedTextError::InvalidSchema("file path".into()))?
            .to_owned(),
        _ => return Err(SharedTextError::FileConflict("concurrent rename".into())),
    };
    validate_path(&path)?;
    let values = document.get_all(&entry, "text")?;
    match values.as_slice() {
        [(Value::Object(ObjType::Text), object)] => Ok((entry, path, object.clone())),
        _ => Err(SharedTextError::InvalidSchema("file text".into())),
    }
}
fn find_file(
    document: &impl ReadDoc,
    files: &ObjId,
    path: &str,
) -> Result<(String, ObjId, ObjId), SharedTextError> {
    validate_path(path)?;
    let mut found = None;
    for key in document.keys(files) {
        let (entry, entry_path, text) = file_entry(document, files, &key)?;
        if entry_path == path {
            if found.is_some() {
                return Err(SharedTextError::FileConflict(path.into()));
            }
            found = Some((key, entry, text));
        }
    }
    found.ok_or_else(|| SharedTextError::MissingFile(path.into()))
}
fn file_layout(document: &impl ReadDoc) -> Result<BTreeMap<String, String>, SharedTextError> {
    let files = files_object(document)?;
    document
        .keys(&files)
        .map(|key| {
            let (_, path, text) = file_entry(document, &files, &key)?;
            Ok((text.to_string(), path))
        })
        .collect()
}
fn text_object(
    document: &impl ReadDoc,
    files: &ObjId,
    path: &str,
) -> Result<ObjId, SharedTextError> {
    Ok(find_file(document, files, path)?.2)
}
fn read_files(
    document: &impl ReadDoc,
    limits: SharedTextLimits,
) -> Result<BTreeMap<String, String>, SharedTextError> {
    let files = files_object(document)?;
    let paths = document
        .keys(&files)
        .take(limits.max_files.saturating_add(1))
        .collect::<Vec<_>>();
    if paths.len() > limits.max_files {
        return Err(SharedTextError::ResourceLimit("file count"));
    }
    let mut result = BTreeMap::new();
    let mut bytes = 0_usize;
    for key in paths {
        let (_, path, object) = file_entry(document, &files, &key)?;
        if result.contains_key(&path) {
            return Err(SharedTextError::FileConflict(path));
        }
        if document.length(&object) > limits.max_file_bytes {
            return Err(SharedTextError::ResourceLimit("file bytes"));
        }
        if document
            .spans(&object)?
            .any(|span| !matches!(span, automerge::iter::Span::Text { marks: None, .. }))
        {
            return Err(SharedTextError::InvalidSchema(
                "source contains rich text".into(),
            ));
        }
        let text = document.text(&object)?;
        check_bytes([text.len()], limits.max_file_bytes, "file bytes")?;
        bytes = bytes
            .checked_add(text.len())
            .filter(|value| *value <= limits.max_total_bytes)
            .ok_or(SharedTextError::ResourceLimit("total file bytes"))?;
        result.insert(path, text);
    }
    Ok(result)
}
fn apply_edit(
    document: &mut impl Transactable,
    files: &ObjId,
    edit: &TextEdit,
    limits: SharedTextLimits,
    new_file_key: &str,
) -> Result<(), SharedTextError> {
    match edit {
        TextEdit::CreateFile { path, text } => {
            validate_path(path)?;
            match find_file(document, files, path) {
                Ok(_) => return Err(SharedTextError::FileExists(path.clone())),
                Err(SharedTextError::MissingFile(_)) => (),
                Err(error) => return Err(error),
            }
            if document.length(files) >= limits.max_files {
                return Err(SharedTextError::ResourceLimit("file count"));
            }
            check_bytes([text.len()], limits.max_file_bytes, "file bytes")?;
            let entry = document.put_object(files, new_file_key, ObjType::Map)?;
            document.put(&entry, "path", path.as_str())?;
            let object = document.put_object(&entry, "text", ObjType::Text)?;
            document.splice_text(&object, 0, 0, text)?;
        }
        TextEdit::Splice {
            path,
            start_utf16,
            delete_utf16,
            insert,
        } => {
            let object = text_object(document, files, path)?;
            let text = document.text(&object)?;
            let end = start_utf16
                .checked_add(*delete_utf16)
                .ok_or(SharedTextError::InvalidPosition)?;
            let start_byte = utf16_to_utf8(&text, *start_utf16)?;
            let end_byte = utf16_to_utf8(&text, end)?;
            check_bytes(
                [text.len() - (end_byte - start_byte), insert.len()],
                limits.max_file_bytes,
                "file bytes",
            )?;
            let delete =
                isize::try_from(*delete_utf16).map_err(|_| SharedTextError::InvalidPosition)?;
            // A raw same-text replacement is still a new contribution with new
            // character identity. `set_file` computes an empty splice for no-op saves.
            document.splice_text(&object, *start_utf16, delete, insert)?;
        }
        TextEdit::RemoveFile { path } => {
            let (key, _, _) = find_file(document, files, path)?;
            document.delete(files, key)?;
        }
        TextEdit::RenameFile { path, new_path } => {
            validate_path(new_path)?;
            let (_, entry, _) = find_file(document, files, path)?;
            if path != new_path {
                match find_file(document, files, new_path) {
                    Ok(_) => return Err(SharedTextError::FileExists(new_path.clone())),
                    Err(SharedTextError::MissingFile(_)) => (),
                    Err(error) => return Err(error),
                }
                document.put(&entry, "path", new_path.as_str())?;
            }
        }
    }
    Ok(())
}

/// Converts browser UTF-16 offsets into compiler UTF-8 byte offsets.
///
/// # Errors
/// Rejects out-of-range positions and surrogate-pair interiors.
pub fn utf16_to_utf8(text: &str, position: usize) -> Result<usize, SharedTextError> {
    let mut offset = 0;
    for (byte, scalar) in text.char_indices() {
        if offset == position {
            return Ok(byte);
        }
        offset += scalar.len_utf16();
        if offset > position {
            return Err(SharedTextError::InvalidPosition);
        }
    }
    if offset == position {
        Ok(text.len())
    } else {
        Err(SharedTextError::InvalidPosition)
    }
}
/// Converts authenticated compiler byte offsets to browser UTF-16 offsets.
///
/// # Errors
/// Rejects out-of-range positions and non-scalar UTF-8 boundaries.
pub fn utf8_to_utf16(text: &str, position: usize) -> Result<usize, SharedTextError> {
    let prefix = text
        .get(..position)
        .ok_or(SharedTextError::InvalidPosition)?;
    Ok(prefix.encode_utf16().count())
}
/// Converts ordered compiler UTF-8 spans into exact UTF-16 endpoints.
///
/// # Errors
/// Rejects reversed, out-of-range or non-scalar spans.
pub fn utf8_span_to_utf16(
    text: &str,
    start: usize,
    end: usize,
) -> Result<(usize, usize), SharedTextError> {
    if start > end {
        return Err(SharedTextError::InvalidPosition);
    }
    Ok((utf8_to_utf16(text, start)?, utf8_to_utf16(text, end)?))
}
