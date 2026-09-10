// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable exact source patches and conservative CRDT-anchor reconciliation.
//!
//! Digests authenticate bytes, never geometry or semantic target lifetime. The
//! compiler host supplies complete lexical ownership regions. If those regions
//! change, a semantic reconciler must prove ownership afresh before overwriting.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::text::{
    SharedTextDocument, SharedTextError, TextEdit, TextRangeAnchor, TextRevision, utf16_to_utf8,
};

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_EDITS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceEdit {
    pub start: usize,
    pub end: usize,
    pub expected: String,
    pub replacement: String,
}

/// Same shape and SHA-256 convention as the managed TypeScript source patch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourcePatch {
    pub base_source_digest: String,
    pub candidate_source_digest: String,
    pub edits: Vec<SourceEdit>,
}

/// A complete lexical owner (for example an authored declaration statement),
/// supplied by the trusted compiler host, in the exact raw source's UTF-16 units.
/// A client must not choose smaller regions to bypass changed ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceRegion {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SourcePatchError {
    #[error("source patch does not authenticate the exact source bytes")]
    Digest,
    #[error("source patch has invalid, overlapping or unauthenticated spans")]
    Span,
    #[error("source patch resource limit")]
    Limit,
    #[error("source patch lacks complete lexical ownership")]
    Ownership,
    #[error(transparent)]
    Text(#[from] SharedTextError),
}

#[derive(Clone, Debug)]
struct AnchoredEdit {
    range: TextRangeAnchor,
    expected: String,
    replacement: String,
}

/// Captured only from exact accepted text. Stable cursors alone never establish
/// that a target property still belongs to the same declaration after typing.
#[derive(Clone, Debug)]
pub struct AnchoredSourcePatch {
    path: String,
    owners: Vec<TextRangeAnchor>,
    edits: Vec<AnchoredEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceReconciliation {
    Applied {
        revision: TextRevision,
        patch: SourcePatch,
    },
    /// Keep working text verbatim and retain the separately accepted canvas edit.
    Pending { reason: String },
}

impl SourcePatch {
    /// Applies an exact byte-authenticated patch without parsing source.
    ///
    /// # Errors
    /// Rejects digest/slice mismatch, overlaps, invalid Unicode boundaries and limits.
    pub fn apply(&self, source: &str) -> Result<String, SourcePatchError> {
        if source.len() > MAX_SOURCE_BYTES || self.edits.len() > MAX_EDITS {
            return Err(SourcePatchError::Limit);
        }
        if source_digest(source) != self.base_source_digest {
            return Err(SourcePatchError::Digest);
        }
        let mut candidate = String::new();
        let mut previous = 0;
        let mut prior_span = None;
        for edit in &self.edits {
            if edit.replacement.len() > MAX_SOURCE_BYTES
                || edit.expected.len() > MAX_SOURCE_BYTES
                || edit.end < edit.start
            {
                return Err(SourcePatchError::Span);
            }
            let start = utf16_to_utf8(source, edit.start)?;
            let end = utf16_to_utf8(source, edit.end)?;
            if start < previous
                || prior_span == Some((start, end))
                || source.get(start..end) != Some(edit.expected.as_str())
            {
                return Err(SourcePatchError::Span);
            }
            candidate.push_str(&source[previous..start]);
            candidate.push_str(&edit.replacement);
            if candidate.len() > MAX_SOURCE_BYTES {
                return Err(SourcePatchError::Limit);
            }
            previous = end;
            prior_span = Some((start, end));
        }
        candidate.push_str(&source[previous..]);
        if candidate.len() > MAX_SOURCE_BYTES {
            return Err(SourcePatchError::Limit);
        }
        if source_digest(&candidate) != self.candidate_source_digest {
            return Err(SourcePatchError::Digest);
        }
        Ok(candidate)
    }
}

impl AnchoredSourcePatch {
    /// Capture on the replica whose file contains the exact accepted source. Each
    /// edit must be enclosed by at least one trusted complete lexical owner.
    ///
    /// # Errors
    /// Rejects invalid exact patches, absent files or missing ownership regions.
    pub fn capture(
        document: &SharedTextDocument,
        file_path: &str,
        patch: &SourcePatch,
        owners: &[SourceRegion],
    ) -> Result<Self, SourcePatchError> {
        let snapshot = document.capture();
        let source = snapshot
            .text(file_path)
            .ok_or_else(|| SharedTextError::MissingFile(file_path.into()))?;
        patch.apply(source)?;
        if owners.len() > MAX_EDITS
            || patch.edits.iter().any(|edit| {
                !owners
                    .iter()
                    .any(|owner| owner.start <= edit.start && owner.end >= edit.end)
            })
        {
            return Err(SourcePatchError::Ownership);
        }
        let anchor = |start, end| -> Result<TextRangeAnchor, SourcePatchError> {
            Ok(document.anchor_range(snapshot.revision(), file_path, start, end)?)
        };
        // Coalesce duplicate/overlapping owners so hostile repeated regions cannot
        // amplify one bounded source file into unbounded retained copies.
        let mut regions = owners.to_vec();
        regions.sort_by_key(|region| (region.start, region.end));
        let mut merged: Vec<SourceRegion> = Vec::new();
        for region in regions {
            if region.end <= region.start {
                return Err(SourcePatchError::Ownership);
            }
            if let Some(previous) = merged
                .last_mut()
                .filter(|previous| previous.end >= region.start)
            {
                previous.end = previous.end.max(region.end);
            } else {
                merged.push(region);
            }
        }
        let owners = merged
            .iter()
            .map(|owner| anchor(owner.start, owner.end))
            .collect::<Result<_, _>>()?;
        let edits = patch
            .edits
            .iter()
            .map(|edit| {
                Ok(AnchoredEdit {
                    range: anchor(edit.start, edit.end)?,
                    expected: edit.expected.clone(),
                    replacement: edit.replacement.clone(),
                })
            })
            .collect::<Result<_, SourcePatchError>>()?;
        Ok(Self {
            path: file_path.into(),
            owners,
            edits,
        })
    }

    /// Integrates only if every original owner and exact edited slice survive.
    /// Changed owners return Pending atomically; the host can then use semantic
    /// recovery-AST reconciliation for identifiable competing property writes.
    ///
    /// # Errors
    /// Rejects invalid text edits/resource limits without changing the working draft.
    pub fn reconcile(
        &self,
        document: &mut SharedTextDocument,
    ) -> Result<SourceReconciliation, SourcePatchError> {
        let snapshot = document.capture();
        let mut file_path = self.path.clone();
        for (index, owner) in self.owners.iter().enumerate() {
            let Ok(range) = document.resolve_range(owner) else {
                return Ok(pending(
                    "lexical ownership changed; semantic reconciliation is required",
                ));
            };
            if index == 0 {
                file_path = range.path;
            } else if file_path != range.path {
                return Ok(pending("source owners moved to different files"));
            }
        }
        let Some(source) = snapshot.text(&file_path) else {
            return Ok(pending("source file no longer exists"));
        };
        let mut edits = Vec::new();
        for edit in &self.edits {
            let Ok(range) = document.resolve_range(&edit.range) else {
                return Ok(pending("edited source range was replaced"));
            };
            if range.path != file_path {
                return Ok(pending("edited source moved to a different file"));
            }
            edits.push(SourceEdit {
                start: range.start_utf16,
                end: range.end_utf16,
                expected: edit.expected.clone(),
                replacement: edit.replacement.clone(),
            });
        }
        edits.sort_by_key(|edit| (edit.start, edit.end));
        if edits.windows(2).any(|pair| {
            pair[0].end > pair[1].start
                || (pair[0].start, pair[0].end) == (pair[1].start, pair[1].end)
        }) {
            return Ok(pending("anchored edits became ambiguous"));
        }
        let mut candidate = source.to_string();
        for edit in edits.iter().rev() {
            candidate.replace_range(
                utf16_to_utf8(source, edit.start)?..utf16_to_utf8(source, edit.end)?,
                &edit.replacement,
            );
        }
        let patch = SourcePatch {
            base_source_digest: source_digest(source),
            candidate_source_digest: source_digest(&candidate),
            edits,
        };
        patch.apply(source)?;
        let operations = patch
            .edits
            .iter()
            .rev()
            .map(|edit| TextEdit::Splice {
                path: file_path.clone(),
                start_utf16: edit.start,
                delete_utf16: edit.end - edit.start,
                insert: edit.replacement.clone(),
            })
            .collect::<Vec<_>>();
        let revision = document.edit(&operations)?;
        Ok(SourceReconciliation::Applied { revision, patch })
    }
}

pub fn source_digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
fn pending(reason: &str) -> SourceReconciliation {
    SourceReconciliation::Pending {
        reason: reason.into(),
    }
}
