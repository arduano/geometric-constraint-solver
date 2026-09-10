// SPDX-License-Identifier: GPL-3.0-or-later
//! Durable personal raw-source history reconstructed from authenticated native heads.
//! Stored events contain provenance, never caller-authored inverse tokens.
use super::{
    SharedTextDocument, SharedTextError, TextEdit, TextRevision, actor_id, check_bytes,
    file_layout, files_object, find_file, text_object,
};
use crate::protocol::OperationId;
use automerge::{CursorPosition, MoveCursor, ReadDoc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod horizon;
pub use horizon::TextHistoryHorizon;

const MAX_EVENTS: usize = 4096;
const MAX_CONTRIBUTIONS: usize = 512;
const MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_SPAN_SCALARS: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextHistoryEvent {
    operation: OperationId,
    actor: Vec<u8>,
    before: TextRevision,
    after: TextRevision,
    action: HistoryAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file_edits: Option<Vec<TextEdit>>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum HistoryAction {
    Text,
    Files,
    Undo { contribution: OperationId },
    Redo { contribution: OperationId },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTextHistory {
    pub undo_count: usize,
    pub redo_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_unavailable: Option<String>,
    pub redo_unavailable: Option<String>,
    pub horizon: TextHistoryHorizon,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Atom {
    id: Vec<u8>,
    scalar: char,
}
#[derive(Clone, Debug)]
struct Span {
    file: String,
    expected: Vec<Atom>,
    restore: Vec<Atom>,
    left: Option<Vec<u8>>,
    right: Option<Vec<u8>>,
}
#[derive(Clone, Debug)]
struct FileVersion {
    file: String,
    path: String,
    path_stamp: String,
    atoms: Vec<Atom>,
}
#[derive(Clone, Debug)]
enum FileInverse {
    Remove(FileVersion),
    Create {
        original: FileVersion,
        deleted_at: TextRevision,
    },
    Rename {
        file: String,
        expected_path: String,
        expected_stamp: String,
        restore_path: String,
        restore_stamp: String,
    },
}
#[derive(Clone, Debug, Default)]
struct Delta {
    spans: Vec<Span>,
    files: Vec<FileInverse>,
}
#[derive(Clone, Debug)]
struct Contribution {
    operation: OperationId,
    delta: Delta,
    active: bool,
}
#[derive(Clone, Debug, Default)]
struct Lineage {
    atoms: BTreeMap<Vec<u8>, Vec<u8>>,
    files: BTreeMap<String, String>,
    paths: BTreeMap<String, String>,
}
/// Server-owned personal history. Checkpoint events are independently rebuilt
/// against native historical text rather than deserialized as mutable Undo tokens.
#[derive(Clone, Debug, Default)]
pub struct TextContributionHistory {
    events: Vec<TextHistoryEvent>,
    contributions: Vec<Contribution>,
    redo: BTreeMap<String, Vec<OperationId>>,
    operations: BTreeSet<OperationId>,
    lineage: Lineage,
    horizon: TextHistoryHorizon,
}
impl TextContributionHistory {
    pub fn events(&self) -> &[TextHistoryEvent] {
        &self.events
    }
    /// Reconstruct every contribution/inverse from authenticated native histories.
    /// # Errors
    /// Rejects altered provenance, unavailable inverses, reordered/missing heads and limits.
    pub fn restore(
        events: Vec<TextHistoryEvent>,
        working: &SharedTextDocument,
    ) -> Result<Self, SharedTextError> {
        if events.len() > MAX_EVENTS {
            return Err(SharedTextError::ResourceLimit("text history events"));
        }
        let expected_events = events.clone();
        let mut history = Self::default();
        for event in events {
            let mut before = historical(working, &event.before, &event.actor)?;
            let after = historical(working, &event.after, &event.actor)?;
            if let Some(previous) = history.events.last() {
                before.validate_revision(&previous.after)?;
            }
            match &event.action {
                HistoryAction::Text => {
                    let actor = actor_id(&event.actor)?;
                    before.validate_new_actor(&after.document, &actor)?;
                    history.record_exact(
                        event.operation.clone(),
                        &before,
                        &after,
                        event.actor.clone(),
                        false,
                    )?;
                }
                HistoryAction::Files => {
                    let novel = before.document.get_changes_added(&after.document);
                    if novel
                        .iter()
                        .any(|change| change.actor_id().to_bytes() != event.actor)
                    {
                        return Err(SharedTextError::ActorMismatch);
                    }
                    if let Some(edits) = &event.file_edits {
                        history.record_working_edits_exact(
                            event.operation.clone(),
                            &before,
                            &after,
                            edits,
                        )?;
                    } else {
                        history.record_exact(
                            event.operation.clone(),
                            &before,
                            &after,
                            event.actor.clone(),
                            true,
                        )?;
                    }
                }
                HistoryAction::Undo { contribution } | HistoryAction::Redo { contribution } => {
                    let redo = matches!(event.action, HistoryAction::Redo { .. });
                    let selected = history.select(&event.operation.user_id, redo)?;
                    if &selected.operation != contribution {
                        return Err(SharedTextError::UndoUnavailable);
                    }
                    history.inverse_exact(event.operation.clone(), &mut before, redo, false)?;
                    if before.revision() != after.revision() {
                        return Err(SharedTextError::UndoUnavailable);
                    }
                }
            }
            if history.events.last() != Some(&event) {
                return Err(SharedTextError::UndoUnavailable);
            }
        }
        if history.events != expected_events {
            return Err(SharedTextError::UndoUnavailable);
        }
        Ok(history)
    }
    /// Record authenticated typing with a moving retained Undo horizon. History
    /// capacity may shorten Undo but cannot block otherwise valid raw typing.
    ///
    /// # Errors
    /// Rejects invalid provenance/operation IDs and native source invariants.
    pub fn record(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        actor: Vec<u8>,
        files: bool,
    ) -> Result<(), SharedTextError> {
        self.retain_new(operation, before, after, actor, files, None)
    }

    /// Records the server-observed effect of authenticated native changes.
    /// # Errors
    /// Rejects duplicate operations, invalid identities, ancestry and bounded history.
    fn record_exact(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        actor: Vec<u8>,
        files: bool,
    ) -> Result<(), SharedTextError> {
        self.check_operation(&operation)?;
        actor_id(&actor)?;
        after.validate_revision(&before.revision())?;
        if files {
            if before
                .document
                .get_changes_added(&after.document)
                .iter()
                .any(|change| change.actor_id().to_bytes() != actor)
            {
                return Err(SharedTextError::ActorMismatch);
            }
        } else {
            before.validate_new_actor(&after.document, &actor_id(&actor)?)?;
        }
        if self.events.len() >= MAX_EVENTS {
            return Err(SharedTextError::ResourceLimit("text history events"));
        }
        if self.contributions.len() >= MAX_CONTRIBUTIONS {
            return Err(SharedTextError::ResourceLimit("text contributions"));
        }
        let delta = derive_delta(before, after)?;
        let mut staged = self.clone();
        staged.redo.remove(&operation.user_id);
        staged.contributions.push(Contribution {
            operation: operation.clone(),
            delta,
            active: true,
        });
        staged.events.push(TextHistoryEvent {
            operation: operation.clone(),
            actor,
            before: before.revision(),
            after: after.revision(),
            action: if files {
                HistoryAction::Files
            } else {
                HistoryAction::Text
            },
            file_edits: None,
        });
        staged.operations.insert(operation);
        staged.check_size()?;
        *self = staged;
        Ok(())
    }
    /// Records ordered lifecycle with the same bounded personal history horizon.
    ///
    /// # Errors
    /// Rejects invalid/stale file commands and native provenance.
    pub fn record_file_edits(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        edits: &[TextEdit],
    ) -> Result<(), SharedTextError> {
        if edits
            .iter()
            .any(|edit| matches!(edit, TextEdit::Splice { .. }))
        {
            return Err(SharedTextError::FileLifecycleRequiresOrder);
        }
        self.record_working_edits(operation, before, after, edits)
    }

    /// Records one trusted ordered text and file transaction in personal history.
    ///
    /// # Errors
    /// Rejects invalid commands, altered native results and invalid provenance.
    pub fn record_working_edits(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        edits: &[TextEdit],
    ) -> Result<(), SharedTextError> {
        self.retain_new(
            operation,
            before,
            after,
            before.actor().to_vec(),
            true,
            Some(edits),
        )
    }

    /// Records the exact ordered working gateway command, including explicit
    /// same-path rename ownership when the native register needs no new value.
    ///
    /// # Errors
    /// Rejects empty commands, altered native results and history bounds.
    fn record_working_edits_exact(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        edits: &[TextEdit],
    ) -> Result<(), SharedTextError> {
        if edits.is_empty() {
            return Err(SharedTextError::FileLifecycleRequiresOrder);
        }
        let mut replay = before.clone();
        replay.edit(edits)?;
        if replay.revision() != after.revision() {
            return Err(SharedTextError::UndoUnavailable);
        }
        let mut staged = self.clone();
        staged.record_exact(operation, before, after, before.actor().to_vec(), true)?;
        staged
            .events
            .last_mut()
            .ok_or(SharedTextError::UndoUnavailable)?
            .file_edits = Some(edits.to_vec());
        let contribution = staged
            .contributions
            .last_mut()
            .ok_or(SharedTextError::UndoUnavailable)?;
        for (index, edit) in edits.iter().enumerate() {
            if let TextEdit::RenameFile { path, new_path } = edit
                && path == new_path
            {
                // Follow this observed file through the rest of the atomic
                // command. A later remove ends that identity even if a different
                // file is created under its old name.
                let mut final_path = Some(path.clone());
                for later in &edits[index + 1..] {
                    match later {
                        TextEdit::RenameFile { path, new_path }
                            if final_path.as_ref() == Some(path) =>
                        {
                            final_path = Some(new_path.clone());
                        }
                        TextEdit::RemoveFile { path } if final_path.as_ref() == Some(path) => {
                            final_path = None;
                        }
                        _ => {}
                    }
                }
                let Some(final_path) = final_path else {
                    continue;
                };
                let current = version(after, &final_path, false)?;
                let already_owned = contribution.delta.files.iter().any(|entry| match entry {
                    FileInverse::Rename { file, .. } => file == &current.file,
                    FileInverse::Remove(file) | FileInverse::Create { original: file, .. } => {
                        file.file == current.file
                    }
                });
                if !already_owned {
                    contribution.delta.files.push(FileInverse::Rename {
                        file: current.file,
                        expected_path: final_path.clone(),
                        expected_stamp: current.path_stamp.clone(),
                        restore_path: final_path,
                        restore_stamp: current.path_stamp,
                    });
                }
            }
        }
        staged.check_size()?;
        *self = staged;
        Ok(())
    }

    /// Checked personal inverse; capacity advances a durable horizon after the
    /// successful raw edit instead of preventing later Undo/Redo publication.
    ///
    /// # Errors
    /// Rejects lost ownership, invalid operations and native source limits.
    pub fn inverse(
        &mut self,
        operation: OperationId,
        working: &mut SharedTextDocument,
        redo: bool,
    ) -> Result<(), SharedTextError> {
        self.inverse_exact(operation, working, redo, true)
    }

    /// Mutates only raw source, never accepted geometry. The host persists both
    /// source and history as one staged text gateway result before its ACK.
    /// # Errors
    /// Rejects another user's active overlap, stale lifetimes and limits atomically.
    fn inverse_exact(
        &mut self,
        operation: OperationId,
        working: &mut SharedTextDocument,
        redo: bool,
        prune: bool,
    ) -> Result<(), SharedTextError> {
        self.check_operation(&operation)?;
        let contribution = self.select(&operation.user_id, redo)?.clone();
        self.check_path_ownership(&contribution)?;
        let before = working.clone();
        let mut staged = working.clone();
        let mut history = self.clone();
        apply_delta(&contribution.delta, &mut staged, &mut history.lineage)?;
        let inverse = derive_delta(&before, &staged)?;
        let selected = history
            .contributions
            .iter_mut()
            .find(|entry| entry.operation == contribution.operation)
            .ok_or(SharedTextError::UndoUnavailable)?;
        selected.delta = inverse;
        selected.active = redo;
        if redo {
            history
                .redo
                .get_mut(&operation.user_id)
                .ok_or(SharedTextError::UndoUnavailable)?
                .pop();
        } else {
            history
                .redo
                .entry(operation.user_id.clone())
                .or_default()
                .push(contribution.operation.clone());
        }
        history.events.push(TextHistoryEvent {
            operation: operation.clone(),
            actor: working.document.get_actor().to_bytes().to_vec(),
            before: before.revision(),
            after: staged.revision(),
            action: if redo {
                HistoryAction::Redo {
                    contribution: contribution.operation,
                }
            } else {
                HistoryAction::Undo {
                    contribution: contribution.operation,
                }
            },
            file_edits: None,
        });
        history.operations.insert(operation);
        if prune && history.needs_pruning() {
            history = history.prune_recent(&staged)?;
        }
        history.check_size()?;
        *self = history;
        *working = staged;
        Ok(())
    }
    pub fn user_history(&self, user: &str, working: &SharedTextDocument) -> UserTextHistory {
        let availability = |redo| {
            self.select(user, redo)
                .and_then(|entry| {
                    self.check_path_ownership(entry)?;
                    apply_delta(
                        &entry.delta,
                        &mut working.clone(),
                        &mut self.lineage.clone(),
                    )
                })
                .err()
                .map(|error| error.to_string())
        };
        let undo_unavailable = availability(false);
        let redo_unavailable = availability(true);
        UserTextHistory {
            undo_count: self
                .contributions
                .iter()
                .filter(|entry| entry.active && entry.operation.user_id == user)
                .count(),
            redo_count: self.redo.get(user).map_or(0, Vec::len),
            can_undo: undo_unavailable.is_none(),
            can_redo: redo_unavailable.is_none(),
            undo_unavailable,
            redo_unavailable,
            horizon: self.horizon(),
        }
    }
    fn select(&self, user: &str, redo: bool) -> Result<&Contribution, SharedTextError> {
        if redo {
            let operation = self
                .redo
                .get(user)
                .and_then(|entries| entries.last())
                .ok_or(SharedTextError::UndoUnavailable)?;
            self.contributions
                .iter()
                .find(|entry| &entry.operation == operation && !entry.active)
        } else {
            self.contributions
                .iter()
                .rev()
                .find(|entry| entry.active && entry.operation.user_id == user)
        }
        .ok_or(SharedTextError::UndoUnavailable)
    }
    fn check_path_ownership(&self, selected: &Contribution) -> Result<(), SharedTextError> {
        let index = self
            .contributions
            .iter()
            .position(|entry| entry.operation == selected.operation)
            .ok_or(SharedTextError::UndoUnavailable)?;
        for inverse in &selected.delta.files {
            if let FileInverse::Rename { file, .. }
            | FileInverse::Remove(FileVersion { file, .. }) = inverse
            {
                let file = resolve_alias(file, &self.lineage.files)?;
                for later in self.contributions[index + 1..]
                    .iter()
                    .filter(|entry| entry.active)
                {
                    for inverse in &later.delta.files {
                        if let FileInverse::Rename {
                            file: later_file, ..
                        } = inverse
                            && resolve_alias(later_file, &self.lineage.files)? == file
                        {
                            return Err(SharedTextError::RangeConflict);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn check_operation(&self, operation: &OperationId) -> Result<(), SharedTextError> {
        for identity in [
            &operation.user_id,
            &operation.client_id,
            &operation.request_id,
        ] {
            if identity.is_empty()
                || identity.len() > 128
                || !identity
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
            {
                return Err(SharedTextError::UndoUnavailable);
            }
        }
        if self.operations.contains(operation) {
            return Err(SharedTextError::UndoUnavailable);
        }
        Ok(())
    }
    fn check_size(&self) -> Result<(), SharedTextError> {
        let events = serde_json::to_vec(&self.events)
            .map_err(|_| SharedTextError::UndoUnavailable)?
            .len();
        let token_bytes = self
            .contributions
            .iter()
            .map(|entry| delta_bytes(&entry.delta))
            .sum::<usize>();
        let lineage_bytes = self
            .lineage
            .atoms
            .iter()
            .map(|(old, new)| old.len() + new.len() + 64)
            .sum::<usize>()
            + self
                .lineage
                .files
                .iter()
                .chain(self.lineage.paths.iter())
                .map(|(old, new)| old.len() + new.len() + 64)
                .sum::<usize>();
        check_bytes(
            [events, token_bytes, lineage_bytes],
            MAX_HISTORY_BYTES,
            "personal text history bytes",
        )
    }
}
fn historical(
    working: &SharedTextDocument,
    revision: &TextRevision,
    actor: &[u8],
) -> Result<SharedTextDocument, SharedTextError> {
    let mut document = working
        .document
        .fork_at(&working.validate_revision(revision)?)?;
    document.set_actor(actor_id(actor)?);
    SharedTextDocument::admit(document, working.limits)
}
fn atom_bytes(atoms: &[Atom]) -> usize {
    atoms.iter().map(|atom| atom.id.len() + 40).sum()
}
fn delta_bytes(delta: &Delta) -> usize {
    delta
        .spans
        .iter()
        .map(|span| span.file.len() + atom_bytes(&span.expected) + atom_bytes(&span.restore) + 256)
        .sum::<usize>()
        + delta
            .files
            .iter()
            .map(|entry| match entry {
                FileInverse::Create { original, .. } | FileInverse::Remove(original) => {
                    original.file.len() + original.path.len() + atom_bytes(&original.atoms) + 256
                }
                FileInverse::Rename { .. } => 1024,
            })
            .sum::<usize>()
}
fn atoms(document: &SharedTextDocument, path: &str) -> Result<Vec<Atom>, SharedTextError> {
    let object = text_object(&document.document, &files_object(&document.document)?, path)?;
    let text = document.document.text(&object)?;
    if text.chars().count() > document.limits.max_file_bytes {
        return Err(SharedTextError::ResourceLimit("text scalar count"));
    }
    let mut offset = 0;
    let mut atoms = Vec::new();
    for scalar in text.chars() {
        let id = document
            .document
            .get_cursor_moving(
                &object,
                CursorPosition::Index(offset),
                None,
                MoveCursor::After,
            )?
            .to_bytes();
        atoms.push(Atom { id, scalar });
        offset += scalar.len_utf16();
    }
    Ok(atoms)
}
fn version(
    document: &SharedTextDocument,
    path: &str,
    with_atoms: bool,
) -> Result<FileVersion, SharedTextError> {
    let (_, entry, object) =
        find_file(&document.document, &files_object(&document.document)?, path)?;
    let path_stamp = document
        .document
        .get(&entry, "path")?
        .ok_or(SharedTextError::InvalidTextTarget)?
        .1
        .to_string();
    Ok(FileVersion {
        file: object.to_string(),
        path: path.into(),
        path_stamp,
        atoms: if with_atoms {
            atoms(document, path)?
        } else {
            vec![]
        },
    })
}
fn derive_delta(
    before: &SharedTextDocument,
    after: &SharedTextDocument,
) -> Result<Delta, SharedTextError> {
    let old_layout = file_layout(&before.document)?;
    let new_layout = file_layout(&after.document)?;
    let mut delta = Delta::default();
    let touched = before
        .document
        .get_changes_added(&after.document)
        .iter()
        .flat_map(|change| {
            change
                .decode()
                .operations
                .into_iter()
                .map(|op| op.obj.to_string())
        })
        .collect::<BTreeSet<_>>();
    for (file, path) in &old_layout {
        if let Some(current) = new_layout.get(file) {
            let previous = version(before, path, false)?;
            let latest = version(after, current, false)?;
            if previous.path_stamp != latest.path_stamp {
                delta.files.push(FileInverse::Rename {
                    file: file.clone(),
                    expected_path: current.clone(),
                    expected_stamp: latest.path_stamp,
                    restore_path: path.clone(),
                    restore_stamp: previous.path_stamp,
                });
            }
            if touched.contains(file) {
                derive_spans(
                    file,
                    &atoms(before, path)?,
                    &atoms(after, current)?,
                    &mut delta.spans,
                )?;
            }
        } else {
            delta.files.push(FileInverse::Create {
                original: version(before, path, true)?,
                deleted_at: after.revision(),
            });
        }
    }
    for (file, path) in &new_layout {
        if !old_layout.contains_key(file) {
            delta
                .files
                .push(FileInverse::Remove(version(after, path, true)?));
        }
    }
    if delta_bytes(&delta) > MAX_HISTORY_BYTES {
        return Err(SharedTextError::ResourceLimit(
            "personal text contribution bytes",
        ));
    }
    Ok(delta)
}
fn derive_spans(
    file: &str,
    before: &[Atom],
    after: &[Atom],
    output: &mut Vec<Span>,
) -> Result<(), SharedTextError> {
    let indices = before
        .iter()
        .enumerate()
        .map(|(index, atom)| (&atom.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut old_start = 0;
    let mut new_start = 0;
    for (new_index, atom) in after.iter().enumerate().chain(std::iter::once((
        after.len(),
        &Atom {
            id: vec![],
            scalar: '\0',
        },
    ))) {
        let old_index = if new_index == after.len() {
            Some(before.len())
        } else {
            indices.get(&atom.id).copied()
        };
        let Some(old_index) = old_index else {
            continue;
        };
        if old_index < old_start {
            return Err(SharedTextError::RangeConflict);
        }
        if old_index > old_start || new_index > new_start {
            if old_index - old_start > MAX_SPAN_SCALARS || new_index - new_start > MAX_SPAN_SCALARS
            {
                return Err(SharedTextError::ResourceLimit("personal text span scalars"));
            }
            output.push(Span {
                file: file.into(),
                expected: after[new_start..new_index].to_vec(),
                restore: before[old_start..old_index].to_vec(),
                left: new_start
                    .checked_sub(1)
                    .map(|index| after[index].id.clone()),
                right: after.get(new_index).map(|atom| atom.id.clone()),
            });
        }
        old_start = old_index + 1;
        new_start = new_index + 1;
    }
    Ok(())
}
fn resolve_alias<T: Ord + Clone>(value: &T, map: &BTreeMap<T, T>) -> Result<T, SharedTextError> {
    let mut current = value.clone();
    for _ in 0..=map.len() {
        match map.get(&current) {
            Some(next) if next != &current => current = next.clone(),
            _ => return Ok(current),
        }
    }
    Err(SharedTextError::UndoUnavailable)
}
fn bind_alias<T: Ord + Clone>(
    original: T,
    restored: T,
    aliases: &mut BTreeMap<T, T>,
) -> Result<(), SharedTextError> {
    let previous = resolve_alias(&original, aliases)?;
    // Older contributions may reference either the original or a prior restored
    // identity. Both name the same effective contribution across repeated cycles.
    if previous != restored {
        aliases.insert(previous, restored.clone());
    }
    if original != restored {
        aliases.insert(original, restored);
    }
    Ok(())
}
fn equal_atoms(
    expected: &[Atom],
    actual: &[Atom],
    lineage: &Lineage,
) -> Result<bool, SharedTextError> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if expected.scalar != actual.scalar
            || resolve_alias(&expected.id, &lineage.atoms)? != actual.id
        {
            return Ok(false);
        }
    }
    Ok(true)
}
#[derive(Debug)]
struct ResolvedSpan {
    path: String,
    start: usize,
    end: usize,
    span: Span,
}
fn resolve_span(
    span: &Span,
    working: &SharedTextDocument,
    lineage: &Lineage,
) -> Result<ResolvedSpan, SharedTextError> {
    let file = resolve_alias(&span.file, &lineage.files)?;
    let layout = file_layout(&working.document)?;
    let path = layout
        .get(&file)
        .ok_or(SharedTextError::InvalidTextTarget)?;
    let actual = atoms(working, path)?;
    let locate = |id: &Vec<u8>| -> Result<usize, SharedTextError> {
        let id = resolve_alias(id, &lineage.atoms)?;
        actual
            .iter()
            .position(|atom| atom.id == id)
            .ok_or(SharedTextError::RangeConflict)
    };
    let start = if let Some(first) = span.expected.first() {
        locate(&first.id)?
    } else if let Some(right) = &span.right {
        locate(right)?
    } else {
        actual.len()
    };
    let end = start
        .checked_add(span.expected.len())
        .filter(|end| *end <= actual.len())
        .ok_or(SharedTextError::RangeConflict)?;
    if !equal_atoms(&span.expected, &actual[start..end], lineage)? {
        return Err(SharedTextError::RangeConflict);
    }
    // Empty spans retain both sides of their gap. Competing insertion there is a
    // real overlap, while insertions outside this gap remain independent.
    if span.expected.is_empty() {
        let left = span.left.as_ref().map(locate).transpose()?;
        if left.map_or(start != 0, |left| left + 1 != start) {
            return Err(SharedTextError::RangeConflict);
        }
    }
    Ok(ResolvedSpan {
        path: path.clone(),
        start,
        end,
        span: span.clone(),
    })
}
fn apply_delta(
    delta: &Delta,
    working: &mut SharedTextDocument,
    lineage: &mut Lineage,
) -> Result<(), SharedTextError> {
    let mut candidate = working.clone();
    let mut aliases = lineage.clone();
    let mut parked = Vec::new();
    // Preflight every path contribution before changing the private namespace.
    for inverse in &delta.files {
        if let FileInverse::Rename {
            file,
            expected_path,
            expected_stamp,
            restore_path,
            restore_stamp,
        } = inverse
        {
            let file = resolve_alias(file, &aliases.files)?;
            let layout = file_layout(&candidate.document)?;
            if layout.get(&file) != Some(expected_path) {
                return Err(SharedTextError::RangeConflict);
            }
            let current = version(&candidate, expected_path, false)?;
            if current.path_stamp != resolve_alias(expected_stamp, &aliases.paths)? {
                return Err(SharedTextError::RangeConflict);
            }
            parked.push((
                expected_path.clone(),
                restore_path.clone(),
                restore_stamp.clone(),
            ));
        }
    }
    for inverse in &delta.files {
        if let FileInverse::Remove(original) = inverse {
            remove_file(original, &mut candidate, &aliases)?;
        }
    }
    let mut final_names = Vec::new();
    for (index, (expected, restore, stamp)) in parked.into_iter().enumerate() {
        // A private temporary path permits simultaneous namespace swaps. It never
        // appears in a published source snapshot or replaces another file.
        let temporary = format!(
            "__geosolve_history_{}_{}.tmp",
            candidate.document.stats().num_ops,
            index
        );
        if candidate.capture().text(&temporary).is_some() {
            return Err(SharedTextError::FileExists(temporary));
        }
        candidate.rename_file(&expected, &temporary)?;
        final_names.push((temporary, restore, stamp));
    }
    for inverse in &delta.files {
        if let FileInverse::Create {
            original,
            deleted_at,
        } = inverse
        {
            restore_file(original, deleted_at, &mut candidate, &mut aliases)?;
        }
    }
    for (temporary, path, stamp) in final_names {
        candidate.rename_file(&temporary, &path)?;
        bind_alias(
            stamp,
            version(&candidate, &path, false)?.path_stamp,
            &mut aliases.paths,
        )?;
    }
    apply_spans(&delta.spans, &mut candidate, &mut aliases)?;
    *working = candidate;
    *lineage = aliases;
    Ok(())
}

fn restore_file(
    original: &FileVersion,
    deleted_at: &TextRevision,
    working: &mut SharedTextDocument,
    lineage: &mut Lineage,
) -> Result<(), SharedTextError> {
    let missing = working.capture().text(&original.path).is_none();
    if !missing {
        return Err(SharedTextError::FileExists(original.path.clone()));
    }
    // Any later assignment of this path invalidates the older tombstone, even
    // if that intervening file was subsequently removed or renamed away.
    let changes = working
        .document
        .get_changes(&working.validate_revision(deleted_at)?);
    if changes.iter().any(|change| {
        change.decode().operations.iter().any(|operation| {
            matches!(&operation.key,automerge::legacy::Key::Map(key) if key.as_str()=="path")
                && operation
                    .primitive_value()
                    .as_ref()
                    .and_then(automerge::ScalarValue::to_str)
                    == Some(original.path.as_str())
        })
    }) {
        return Err(SharedTextError::InvalidTextTarget);
    }
    let text = original
        .atoms
        .iter()
        .map(|atom| atom.scalar)
        .collect::<String>();
    working.create_file(&original.path, &text)?;
    let restored = version(working, &original.path, true)?;
    bind_alias(original.file.clone(), restored.file, &mut lineage.files)?;
    bind_alias(
        original.path_stamp.clone(),
        restored.path_stamp,
        &mut lineage.paths,
    )?;
    for (old, new) in original.atoms.iter().zip(restored.atoms) {
        bind_alias(old.id.clone(), new.id, &mut lineage.atoms)?;
    }
    Ok(())
}
fn remove_file(
    original: &FileVersion,
    working: &mut SharedTextDocument,
    lineage: &Lineage,
) -> Result<(), SharedTextError> {
    let file = resolve_alias(&original.file, &lineage.files)?;
    let layout = file_layout(&working.document)?;
    let path = layout
        .get(&file)
        .ok_or(SharedTextError::InvalidTextTarget)?;
    let current = version(working, path, true)?;
    if path != &original.path
        || current.path_stamp != resolve_alias(&original.path_stamp, &lineage.paths)?
        || !equal_atoms(&original.atoms, &current.atoms, lineage)?
    {
        return Err(SharedTextError::RangeConflict);
    }
    working.remove_file(path)?;
    Ok(())
}
fn apply_spans(
    spans: &[Span],
    working: &mut SharedTextDocument,
    lineage: &mut Lineage,
) -> Result<(), SharedTextError> {
    let mut resolved = spans
        .iter()
        .map(|span| resolve_span(span, working, lineage))
        .collect::<Result<Vec<_>, _>>()?;
    resolved.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
    if resolved
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path && pair[0].end >= pair[1].start)
    {
        return Err(SharedTextError::RangeConflict);
    }
    let mut edits = Vec::new();
    for range in resolved.iter().rev() {
        let original = atoms(working, &range.path)?;
        let start = original[..range.start]
            .iter()
            .map(|atom| atom.scalar.len_utf16())
            .sum();
        let delete = original[range.start..range.end]
            .iter()
            .map(|atom| atom.scalar.len_utf16())
            .sum();
        edits.push(TextEdit::Splice {
            path: range.path.clone(),
            start_utf16: start,
            delete_utf16: delete,
            insert: range.span.restore.iter().map(|atom| atom.scalar).collect(),
        });
    }
    working.edit(&edits)?;
    let mut shifts = BTreeMap::<String, isize>::new();
    for range in &resolved {
        let shift = shifts.entry(range.path.clone()).or_default();
        let start = range
            .start
            .checked_add_signed(*shift)
            .ok_or(SharedTextError::RangeConflict)?;
        let current = atoms(working, &range.path)?;
        let end = start + range.span.restore.len();
        let fresh = current
            .get(start..end)
            .ok_or(SharedTextError::RangeConflict)?;
        for (old, new) in range.span.restore.iter().zip(fresh) {
            if old.scalar != new.scalar {
                return Err(SharedTextError::RangeConflict);
            }
            bind_alias(old.id.clone(), new.id.clone(), &mut lineage.atoms)?;
        }
        *shift += isize::try_from(range.span.restore.len())
            .map_err(|_| SharedTextError::InvalidPosition)?
            - isize::try_from(range.end - range.start)
                .map_err(|_| SharedTextError::InvalidPosition)?;
    }
    Ok(())
}
