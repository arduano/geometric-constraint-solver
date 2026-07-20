// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use thiserror::Error;

use crate::{DocumentElementId, DocumentId, PersistentId, SketchDocument};

/// Validation failure for one application-owned sketch attribute sidecar.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum SketchAttributeError {
    #[error("attribute sidecar belongs to document {expected}, not {actual}")]
    ForeignDocument {
        expected: DocumentId,
        actual: DocumentId,
    },
    #[error("unknown {kind} element {id} in attribute document")]
    UnknownElement {
        kind: &'static str,
        id: PersistentId,
    },
    #[error("persistent element {id} is a {actual}, not a {requested}")]
    WrongElementKind {
        id: PersistentId,
        requested: &'static str,
        actual: &'static str,
    },
}

/// Generic application metadata keyed only by persistent sketch identities.
///
/// Attributes are intentionally absent from sketch JSON, runtime lowering,
/// equation audit and solver state. Embedders own any attribute codec, migration,
/// command history or combined workspace envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchAttributes<T> {
    document: DocumentId,
    values: BTreeMap<DocumentElementId, T>,
}

impl<T> SketchAttributes<T> {
    #[must_use]
    pub fn new(document: &SketchDocument) -> Self {
        Self {
            document: document.id(),
            values: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn document_id(&self) -> DocumentId {
        self.document
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Attaches one value after checking document identity and typed liveness.
    ///
    /// # Errors
    ///
    /// Rejects a foreign document, missing target, or an ID used with the wrong kind.
    pub fn insert(
        &mut self,
        document: &SketchDocument,
        target: impl Into<DocumentElementId>,
        value: T,
    ) -> Result<Option<T>, SketchAttributeError> {
        self.require_document(document)?;
        let target = target.into();
        if !document.contains_element(target) {
            if let Some(actual) = document.element(target.persistent_id()) {
                return Err(SketchAttributeError::WrongElementKind {
                    id: target.persistent_id(),
                    requested: target.kind(),
                    actual: actual.kind(),
                });
            }
            return Err(SketchAttributeError::UnknownElement {
                kind: target.kind(),
                id: target.persistent_id(),
            });
        }
        Ok(self.values.insert(target, value))
    }

    /// Returns an attribute whether its target is live or dormant.
    #[must_use]
    pub fn get(&self, target: impl Into<DocumentElementId>) -> Option<&T> {
        self.values.get(&target.into())
    }

    /// Returns an attribute only while its target exists in the supplied document.
    ///
    /// # Errors
    ///
    /// Rejects a document other than the one this sidecar was created for.
    pub fn get_live(
        &self,
        document: &SketchDocument,
        target: impl Into<DocumentElementId>,
    ) -> Result<Option<&T>, SketchAttributeError> {
        self.require_document(document)?;
        let target = target.into();
        Ok(document
            .contains_element(target)
            .then(|| self.values.get(&target))
            .flatten())
    }

    pub fn remove(&mut self, target: impl Into<DocumentElementId>) -> Option<T> {
        self.values.remove(&target.into())
    }

    pub fn iter(&self) -> impl Iterator<Item = (DocumentElementId, &T)> {
        self.values.iter().map(|(target, value)| (*target, value))
    }

    /// Lists values whose targets are currently absent without deleting them.
    ///
    /// # Errors
    ///
    /// Rejects a document other than the one this sidecar was created for.
    pub fn orphaned_targets(
        &self,
        document: &SketchDocument,
    ) -> Result<Vec<DocumentElementId>, SketchAttributeError> {
        self.require_document(document)?;
        Ok(self
            .values
            .keys()
            .copied()
            .filter(|target| !document.contains_element(*target))
            .collect())
    }

    /// Explicitly discards dormant values and returns the number removed.
    ///
    /// # Errors
    ///
    /// Rejects a document other than the one this sidecar was created for.
    pub fn retain_live(
        &mut self,
        document: &SketchDocument,
    ) -> Result<usize, SketchAttributeError> {
        self.require_document(document)?;
        let previous = self.values.len();
        self.values
            .retain(|target, _| document.contains_element(*target));
        Ok(previous - self.values.len())
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    fn require_document(&self, document: &SketchDocument) -> Result<(), SketchAttributeError> {
        if document.id() == self.document {
            Ok(())
        } else {
            Err(SketchAttributeError::ForeignDocument {
                expected: self.document,
                actual: document.id(),
            })
        }
    }
}
