// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact per-feature reconstruction used by projectional persistence adapters.

use geosolve_sketch::DocumentId;

use super::{
    ComputedFeature, ComputedFeatureAllocatorHighWater, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentId, ComputedFeatureLifecycleHighWater,
    ComputedFeatureRevision, invalid_field,
};

/// Validated builder for reconstructing a computed-feature sidecar from one
/// independently decoded feature object at a time.
///
/// This adapter deliberately allocates nothing and infers no feature recipe.
/// The caller supplies the exact current allocator state separately from the
/// lifecycle high-water retained across Undo/Redo.
#[derive(Clone, Debug)]
pub struct ComputedFeatureObjectBootstrap {
    document: ComputedFeatureDocument,
    lifecycle_high_water: ComputedFeatureLifecycleHighWater,
}

impl ComputedFeatureObjectBootstrap {
    /// Starts one exact per-feature reconstruction.
    ///
    /// # Errors
    ///
    /// Rejects zero allocator cursors or a lifecycle high-water that trails
    /// the reconstructed document revision or allocator state.
    pub fn new(
        sketch_document: DocumentId,
        document_id: ComputedFeatureDocumentId,
        revision: ComputedFeatureRevision,
        allocator: ComputedFeatureAllocatorHighWater,
        lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    ) -> Result<Self, ComputedFeatureDocumentError> {
        if document_id.raw() == 0 {
            return Err(invalid_field("document_id", "must be nonzero"));
        }
        if allocator.next_feature_id.raw() == 0 || allocator.next_corner_id.raw() == 0 {
            return Err(invalid_field(
                "allocator",
                "allocator cursors must be nonzero",
            ));
        }
        if lifecycle_high_water.revision < revision
            || lifecycle_high_water.allocator.next_feature_id < allocator.next_feature_id
            || lifecycle_high_water.allocator.next_corner_id < allocator.next_corner_id
        {
            return Err(invalid_field(
                "lifecycle high-water",
                "must not trail the reconstructed document",
            ));
        }
        Ok(Self {
            document: ComputedFeatureDocument {
                id: document_id,
                sketch_document,
                revision,
                next_feature_id: allocator.next_feature_id,
                next_corner_id: allocator.next_corner_id,
                features: Vec::new(),
            },
            lifecycle_high_water,
        })
    }

    /// Adds one exact persisted feature object. Duplicate and malformed
    /// identities are rejected by [`Self::finish`], keeping object ingestion
    /// order non-semantic.
    pub fn push_feature(&mut self, feature: ComputedFeature) {
        self.document.features.push(feature);
    }

    /// Returns the separately retained lifecycle high-water authenticated by
    /// this builder.
    #[must_use]
    pub const fn lifecycle_high_water(&self) -> ComputedFeatureLifecycleHighWater {
        self.lifecycle_high_water
    }

    /// Canonicalizes and validates the complete exact sidecar.
    ///
    /// # Errors
    ///
    /// Rejects duplicate, malformed, non-finite, invalid-branch, resource, or
    /// allocator state.
    pub fn finish(mut self) -> Result<ComputedFeatureDocument, ComputedFeatureDocumentError> {
        self.document.normalize();
        self.document.validate()?;
        if self.document.lifecycle_high_water().revision > self.lifecycle_high_water.revision
            || self.document.allocator_high_water().next_feature_id
                > self.lifecycle_high_water.allocator.next_feature_id
            || self.document.allocator_high_water().next_corner_id
                > self.lifecycle_high_water.allocator.next_corner_id
        {
            return Err(invalid_field(
                "lifecycle high-water",
                "must not trail the reconstructed document",
            ));
        }
        Ok(self.document)
    }
}
