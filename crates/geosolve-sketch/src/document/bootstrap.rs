// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact per-object reconstruction used by projectional persistence adapters.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ContactSlot, DesignCurve, DesignPoint, DesignScalar, DocumentCurveTrimView, DocumentDimension,
    DocumentElementId, DocumentError, DocumentExternalBinding, DocumentParameter,
    DocumentParameterBinding, DocumentParameterOutput, DocumentSourceId, GeometryRoleEdit,
    HostConfigurationActivation, SketchDocument, SketchPersistentIdentityHighWater,
};

/// Validated builder for reconstructing one sketch from independently decoded
/// persistent objects rather than an opaque aggregate scene payload.
///
/// This intentionally does not allocate identities or infer authoring recipes.
/// Persistence adapters supply every object with its exact historical ID, then
/// [`Self::finish`] canonicalizes and validates the complete ordinary document.
#[derive(Clone, Debug)]
pub struct SketchObjectBootstrap {
    document: SketchDocument,
    high_water: SketchPersistentIdentityHighWater,
}

impl SketchObjectBootstrap {
    /// Starts an exact object bootstrap for one native namespace.
    ///
    /// # Errors
    ///
    /// Rejects an invalid model scale, namespace, or foreign/malformed
    /// persistent-identity high-water value.
    pub fn new(
        model_scale: f64,
        high_water: SketchPersistentIdentityHighWater,
    ) -> Result<Self, DocumentError> {
        high_water.validate()?;
        let document = SketchDocument::with_id(model_scale, high_water.document)?;
        Ok(Self {
            document,
            high_water,
        })
    }

    pub fn push_point(&mut self, value: DesignPoint) {
        self.document.points.push(value);
    }

    pub fn push_scalar(&mut self, value: DesignScalar) {
        self.document.scalars.push(value);
    }

    pub fn push_curve(&mut self, value: DesignCurve) {
        self.document.curves.push(value);
    }

    pub fn push_contact(&mut self, value: ContactSlot) {
        self.document.contacts.push(value);
    }

    pub fn push_trim_view(&mut self, value: DocumentCurveTrimView) {
        self.document.trim_views.push(value);
    }

    pub fn push_constraint(&mut self, value: super::DocumentConstraint) {
        self.document.constraints.push(value);
    }

    pub fn push_dimension(&mut self, value: DocumentDimension) {
        self.document.dimensions.push(value);
    }

    pub fn push_parameter(&mut self, value: DocumentParameter) {
        self.document.parameters.push(value);
    }

    pub fn push_parameter_binding(&mut self, value: DocumentParameterBinding) {
        self.document.parameter_bindings.push(value);
    }

    pub fn push_parameter_output(&mut self, value: DocumentParameterOutput) {
        self.document.parameter_outputs.push(value);
    }

    pub fn push_external_binding(&mut self, value: DocumentExternalBinding) {
        self.document.external_bindings.push(value);
    }

    pub fn push_source(&mut self, value: DocumentSourceId) {
        self.document.source_order.push(value);
    }

    pub fn push_geometry_role(&mut self, value: GeometryRoleEdit) {
        self.document.geometry_roles.insert(value.curve, value.role);
    }

    pub fn push_user_inactive_element(&mut self, value: DocumentElementId) {
        self.document.user_inactive_elements.insert(value);
    }

    pub fn set_host_activation(&mut self, value: HostConfigurationActivation) {
        self.document.host_activation = Some(value);
    }

    /// Replaces the exact non-element semantic reservation side table.
    /// Catalog entries own themselves; all other sources name one present
    /// catalog. Complete validation is deferred to [`Self::finish`].
    pub fn set_semantic_source_reservations(
        &mut self,
        values: impl IntoIterator<Item = (DocumentSourceId, DocumentSourceId)>,
    ) {
        self.document.semantic_source_reservations = values.into_iter().collect::<BTreeMap<_, _>>();
    }

    /// Canonicalizes and independently validates the reconstructed document.
    ///
    /// # Errors
    ///
    /// Rejects duplicate, foreign, missing, non-finite, invalid-domain,
    /// invalid-branch, dependency, source-order, side-table, or allocator state.
    pub fn finish(mut self) -> Result<SketchDocument, DocumentError> {
        self.document
            .retain_persistent_identity_high_water(&self.high_water)?;
        self.document.canonicalize();
        self.document.validate()?;
        let actual = self.document.persistent_identity_high_water();
        if actual != self.high_water {
            return super::invalid(
                "object bootstrap high-water",
                "reconstructed document does not retain the exact supplied allocator state",
            );
        }
        Ok(self.document)
    }
}

// Keep imports exhaustive when side-table types evolve; duplicate rejection is
// intentionally delegated to complete document validation.
const _: Option<BTreeSet<DocumentElementId>> = None;
