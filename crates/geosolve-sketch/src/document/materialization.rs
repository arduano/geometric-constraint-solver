// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    ContactId, ContactSlot, CurveDefinition, CurveId, DesignCurve, DesignPoint, DesignPointId,
    DesignScalar, DesignScalarId, DocumentConstraint, DocumentConstraintId, DocumentCurveTrimView,
    DocumentDimension, DocumentDimensionId, DocumentElementId, DocumentError,
    DocumentExternalBinding, DocumentExternalBindingId, DocumentId, DocumentParameter,
    DocumentParameterBinding, DocumentParameterId, DocumentParameterOutput, DocumentSourceId,
    GeometryRole, GeometryRoleEdit, HostConfigurationActivation, MAX_DOCUMENT_OBJECTS,
    PersistentId, SketchDocument, SketchPersistentIdentityHighWater,
};

/// Exact persistent identities reserved for one materialized constraint and its audit source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SketchMaterializationConstraintReservation {
    pub constraint: DocumentConstraintId,
    pub source: DocumentSourceId,
}

/// Exact persistent identities reserved for one materialized dimension and its audit source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SketchMaterializationDimensionReservation {
    pub dimension: DocumentDimensionId,
    pub source: DocumentSourceId,
}

/// One typed persistent-identity reservation consumed by deterministic materialization.
///
/// Constraint and dimension reservations include their separately ordered source identity.
/// Semantic catalog identities never become ordinary document sources.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum SketchMaterializationIdentityReservation {
    Point {
        id: DesignPointId,
    },
    Scalar {
        id: DesignScalarId,
    },
    Curve {
        id: CurveId,
    },
    Contact {
        id: ContactId,
    },
    Constraint {
        reservation: SketchMaterializationConstraintReservation,
    },
    Dimension {
        reservation: SketchMaterializationDimensionReservation,
    },
    Parameter {
        id: DocumentParameterId,
    },
    ExternalBinding {
        id: DocumentExternalBindingId,
    },
    SemanticCatalog {
        id: DocumentSourceId,
    },
    SemanticSource {
        id: DocumentSourceId,
        catalog: DocumentSourceId,
    },
}

impl SketchMaterializationIdentityReservation {
    fn append_roles(self, roles: &mut Vec<(PersistentId, ReservedIdentityRole)>) {
        match self {
            Self::Point { id } => roles.push((id.0, ReservedIdentityRole::Point)),
            Self::Scalar { id } => roles.push((id.0, ReservedIdentityRole::Scalar)),
            Self::Curve { id } => roles.push((id.0, ReservedIdentityRole::Curve)),
            Self::Contact { id } => roles.push((id.0, ReservedIdentityRole::Contact)),
            Self::Constraint { reservation } => {
                roles.push((
                    reservation.constraint.0,
                    ReservedIdentityRole::Constraint {
                        source: reservation.source,
                    },
                ));
                roles.push((
                    reservation.source.0,
                    ReservedIdentityRole::ConstraintSource {
                        owner: reservation.constraint,
                    },
                ));
            }
            Self::Dimension { reservation } => {
                roles.push((
                    reservation.dimension.0,
                    ReservedIdentityRole::Dimension {
                        source: reservation.source,
                    },
                ));
                roles.push((
                    reservation.source.0,
                    ReservedIdentityRole::DimensionSource {
                        owner: reservation.dimension,
                    },
                ));
            }
            Self::Parameter { id } => roles.push((id.0, ReservedIdentityRole::Parameter)),
            Self::ExternalBinding { id } => {
                roles.push((id.0, ReservedIdentityRole::ExternalBinding));
            }
            Self::SemanticCatalog { id } => {
                roles.push((id.0, ReservedIdentityRole::SemanticCatalog));
            }
            Self::SemanticSource { id, catalog } => {
                roles.push((id.0, ReservedIdentityRole::SemanticSource { catalog }));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReservedIdentityRole {
    Point,
    Scalar,
    Curve,
    Contact,
    Constraint { source: DocumentSourceId },
    ConstraintSource { owner: DocumentConstraintId },
    Dimension { source: DocumentSourceId },
    DimensionSource { owner: DocumentDimensionId },
    Parameter,
    ExternalBinding,
    SemanticCatalog,
    SemanticSource { catalog: DocumentSourceId },
}

/// A validated contiguous typed reservation interval for one atomic materialization batch.
///
/// The base and resulting high-water values remain field-opaque. Hosts persist those existing
/// DTOs and the typed reservation list, then restore this set with [`Self::from_parts`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchMaterializationReservationSet {
    base: SketchPersistentIdentityHighWater,
    resulting: SketchPersistentIdentityHighWater,
    reservations: Vec<SketchMaterializationIdentityReservation>,
    role_by_id: BTreeMap<PersistentId, ReservedIdentityRole>,
}

impl SketchMaterializationReservationSet {
    /// Restores one reservation set after validating namespace, monotonic high-water, typed order,
    /// duplicates, and curve-local spline cursors.
    ///
    /// # Errors
    ///
    /// Returns an error when the interval is malformed, non-contiguous, excessive, or regresses
    /// either global or curve-local allocator state.
    pub fn from_parts(
        base: SketchPersistentIdentityHighWater,
        resulting: SketchPersistentIdentityHighWater,
        reservations: Vec<SketchMaterializationIdentityReservation>,
    ) -> Result<Self, DocumentError> {
        let mut value = Self {
            base,
            resulting,
            reservations,
            role_by_id: BTreeMap::new(),
        };
        value.role_by_id = value.validate_structure()?;
        Ok(value)
    }

    /// Persistent sketch namespace owned by this reservation interval.
    #[must_use]
    pub const fn document(&self) -> DocumentId {
        self.base.document
    }

    /// Exact allocator state expected before this reservation interval.
    #[must_use]
    pub const fn base_high_water(&self) -> &SketchPersistentIdentityHighWater {
        &self.base
    }

    /// Allocator state retained after this reservation interval, including unused identities.
    #[must_use]
    pub const fn resulting_high_water(&self) -> &SketchPersistentIdentityHighWater {
        &self.resulting
    }

    /// Typed reservations in exact allocation order.
    #[must_use]
    pub fn reservations(&self) -> &[SketchMaterializationIdentityReservation] {
        &self.reservations
    }

    fn ordered_roles(&self) -> Vec<(PersistentId, ReservedIdentityRole)> {
        let mut roles = Vec::new();
        for reservation in &self.reservations {
            reservation.append_roles(&mut roles);
        }
        roles
    }

    fn role(&self, id: PersistentId) -> Option<ReservedIdentityRole> {
        self.role_by_id.get(&id).copied()
    }

    fn validate_structure(
        &self,
    ) -> Result<BTreeMap<PersistentId, ReservedIdentityRole>, DocumentError> {
        self.base.validate()?;
        self.resulting.validate()?;
        if self.base.document != self.resulting.document {
            return materialization_invalid(
                "reservation base and result belong to different sketch namespaces",
            );
        }
        if self.base.merged(&self.resulting)? != self.resulting {
            return materialization_invalid("resulting allocator high-water regresses its base");
        }

        let interval = self
            .resulting
            .next_id
            .as_u128()
            .checked_sub(self.base.next_id.as_u128())
            .ok_or_else(|| {
                materialization_invalid_error("resulting identity cursor precedes its base")
            })?;
        if interval > MAX_DOCUMENT_OBJECTS as u128 {
            return Err(DocumentError::ResourceLimit {
                resource: "materialization identity reservations",
                actual: usize::try_from(interval).unwrap_or(usize::MAX),
                limit: MAX_DOCUMENT_OBJECTS,
            });
        }

        let roles = self.ordered_roles();
        if roles.len() != usize::try_from(interval).expect("bounded reservation interval") {
            return materialization_invalid(
                "typed reservations must cover the complete allocator interval exactly once",
            );
        }
        let mut expected = self.base.next_id.as_u128();
        let mut role_by_id = BTreeMap::new();
        for (id, role) in roles {
            if id.as_u128() != expected {
                return materialization_invalid(
                    "typed reservations must be contiguous and retain allocation order",
                );
            }
            if role_by_id.insert(id, role).is_some() {
                return Err(DocumentError::DuplicateId(id));
            }
            expected = expected.checked_add(1).ok_or(DocumentError::IdExhausted)?;
        }
        if expected != self.resulting.next_id.as_u128() {
            return materialization_invalid(
                "typed reservations do not end at the resulting allocator cursor",
            );
        }

        for (id, role) in &role_by_id {
            if let ReservedIdentityRole::SemanticSource { catalog } = role
                && catalog.0 >= self.base.next_id
            {
                if !matches!(
                    role_by_id.get(&catalog.0),
                    Some(ReservedIdentityRole::SemanticCatalog)
                ) {
                    return materialization_invalid(
                        "same-batch semantic sources require a semantic-catalog reservation",
                    );
                }
                if catalog.0 >= *id {
                    return materialization_invalid(
                        "a same-batch semantic catalog must be reserved before its sources",
                    );
                }
            }
            if let Some(cursor) = self.resulting.spline_span_cursors.get(&CurveId(*id))
                && *cursor > 0
                && !matches!(role, ReservedIdentityRole::Curve)
            {
                return materialization_invalid(
                    "a spline cursor may advance only an identity reserved as a curve",
                );
            }
        }
        Ok(role_by_id)
    }
}

/// Pure staged allocator for a contiguous [`SketchMaterializationReservationSet`].
///
/// The allocator does not mutate a live [`SketchDocument`]. Its finished reservation set is
/// persisted by the caller and later consumed atomically by a materialization batch.
#[derive(Clone, Debug)]
pub struct SketchMaterializationReservationAllocator {
    base: SketchPersistentIdentityHighWater,
    resulting: SketchPersistentIdentityHighWater,
    reservations: Vec<SketchMaterializationIdentityReservation>,
    role_by_id: BTreeMap<PersistentId, ReservedIdentityRole>,
}

impl SketchMaterializationReservationAllocator {
    /// Begins staging reservations from one validated high-water context.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied allocator state is malformed.
    pub fn new(base: SketchPersistentIdentityHighWater) -> Result<Self, DocumentError> {
        base.validate()?;
        Ok(Self {
            resulting: base.clone(),
            base,
            reservations: Vec::new(),
            role_by_id: BTreeMap::new(),
        })
    }

    /// Reserves one point identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_point(&mut self) -> Result<DesignPointId, DocumentError> {
        let id = DesignPointId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::Point { id });
        Ok(id)
    }

    /// Reserves one scalar identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_scalar(&mut self) -> Result<DesignScalarId, DocumentError> {
        let id = DesignScalarId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::Scalar { id });
        Ok(id)
    }

    /// Reserves one curve identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_curve(&mut self) -> Result<CurveId, DocumentError> {
        let id = CurveId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::Curve { id });
        Ok(id)
    }

    /// Reserves one contact identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_contact(&mut self) -> Result<ContactId, DocumentError> {
        let id = ContactId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::Contact { id });
        Ok(id)
    }

    /// Reserves one constraint identity followed by its source identity atomically.
    ///
    /// # Errors
    ///
    /// Returns an error when both identities cannot be reserved within the bounded interval.
    pub fn reserve_constraint(
        &mut self,
    ) -> Result<SketchMaterializationConstraintReservation, DocumentError> {
        let ids = self.reserve_raw(2)?;
        let reservation = SketchMaterializationConstraintReservation {
            constraint: DocumentConstraintId(ids[0]),
            source: DocumentSourceId(ids[1]),
        };
        self.push_reservation(SketchMaterializationIdentityReservation::Constraint { reservation });
        Ok(reservation)
    }

    /// Reserves one dimension identity followed by its source identity atomically.
    ///
    /// # Errors
    ///
    /// Returns an error when both identities cannot be reserved within the bounded interval.
    pub fn reserve_dimension(
        &mut self,
    ) -> Result<SketchMaterializationDimensionReservation, DocumentError> {
        let ids = self.reserve_raw(2)?;
        let reservation = SketchMaterializationDimensionReservation {
            dimension: DocumentDimensionId(ids[0]),
            source: DocumentSourceId(ids[1]),
        };
        self.push_reservation(SketchMaterializationIdentityReservation::Dimension { reservation });
        Ok(reservation)
    }

    /// Reserves one host-parameter identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_parameter(&mut self) -> Result<DocumentParameterId, DocumentError> {
        let id = DocumentParameterId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::Parameter { id });
        Ok(id)
    }

    /// Reserves one external-binding identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_external_binding(&mut self) -> Result<DocumentExternalBindingId, DocumentError> {
        let id = DocumentExternalBindingId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::ExternalBinding { id });
        Ok(id)
    }

    /// Reserves one non-element semantic catalog identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_semantic_catalog(&mut self) -> Result<DocumentSourceId, DocumentError> {
        let id = DocumentSourceId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::SemanticCatalog { id });
        Ok(id)
    }

    /// Reserves one non-element semantic source owned by an existing or same-batch catalog.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent identity interval is exhausted or exceeds its bound.
    pub fn reserve_semantic_source(
        &mut self,
        catalog: DocumentSourceId,
    ) -> Result<DocumentSourceId, DocumentError> {
        let id = DocumentSourceId(self.reserve_raw(1)?[0]);
        self.push_reservation(SketchMaterializationIdentityReservation::SemanticSource {
            id,
            catalog,
        });
        Ok(id)
    }

    /// Advances the curve-local never-reuse cursor for an existing or newly reserved spline.
    ///
    /// # Errors
    ///
    /// Rejects zero/regressing cursors and identities that are not known as spline-capable curves
    /// in the base high-water or this reservation interval.
    pub fn reserve_spline_span_cursor(
        &mut self,
        curve: CurveId,
        next_span_id: u32,
    ) -> Result<(), DocumentError> {
        if next_span_id == 0 {
            return materialization_invalid("spline next-span cursor must be nonzero");
        }
        let existing = self.resulting.spline_span_cursors.get(&curve).copied();
        if existing.is_none() {
            if curve.0 < self.base.next_id {
                return materialization_invalid(
                    "pre-existing spline cursor must be present in the base high-water",
                );
            }
            if !matches!(
                self.reservation_role(curve.0),
                Some(ReservedIdentityRole::Curve)
            ) {
                return materialization_invalid(
                    "spline cursor identity is not reserved as a curve",
                );
            }
        }
        if existing.is_some_and(|cursor| next_span_id < cursor) {
            return materialization_invalid("spline next-span cursor must not regress");
        }
        self.resulting
            .spline_span_cursors
            .insert(curve, next_span_id);
        Ok(())
    }

    /// Finishes and independently revalidates the staged reservation set.
    ///
    /// # Errors
    ///
    /// Returns an error if the staged interval or high-water metadata is inconsistent.
    pub fn finish(self) -> Result<SketchMaterializationReservationSet, DocumentError> {
        SketchMaterializationReservationSet::from_parts(
            self.base,
            self.resulting,
            self.reservations,
        )
    }

    fn reservation_role(&self, id: PersistentId) -> Option<ReservedIdentityRole> {
        self.role_by_id.get(&id).copied()
    }

    fn push_reservation(&mut self, reservation: SketchMaterializationIdentityReservation) {
        let mut roles = Vec::new();
        reservation.append_roles(&mut roles);
        self.role_by_id.extend(roles);
        self.reservations.push(reservation);
    }

    fn reserve_raw(&mut self, count: u128) -> Result<Vec<PersistentId>, DocumentError> {
        let start = self.resulting.next_id.as_u128();
        let end = start.checked_add(count).ok_or(DocumentError::IdExhausted)?;
        let total = end
            .checked_sub(self.base.next_id.as_u128())
            .ok_or(DocumentError::IdExhausted)?;
        if total > MAX_DOCUMENT_OBJECTS as u128 {
            return Err(DocumentError::ResourceLimit {
                resource: "materialization identity reservations",
                actual: usize::try_from(total).unwrap_or(usize::MAX),
                limit: MAX_DOCUMENT_OBJECTS,
            });
        }
        self.resulting.next_id = PersistentId::from_u128(end);
        Ok((start..end).map(PersistentId::from_u128).collect())
    }
}

/// One semantic catalog and the exact semantic-source identities it owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchMaterializationSemanticCatalog {
    pub catalog: DocumentSourceId,
    pub sources: Vec<DocumentSourceId>,
}

/// Whether an applied batch must materialize every reserved persistent identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchMaterializationReservationConsumption {
    /// Every reservation must be consumed by an object or semantic-catalog record of the same kind.
    Exact,
    /// Unused reservations are deliberately retained above high-water and retired without reuse.
    RetainUnused,
}

/// Additive persistent objects and side tables consumed as one atomic materialization step.
///
/// Reserved identities may deliberately remain unused by this batch. Applying such a batch still
/// advances the document high-water, which retires identities owned by suppressed, deleted, or
/// failed retained owners without making their reserved identities available for reuse.
#[derive(Clone, Debug)]
pub struct SketchMaterializationBatch {
    reservations: SketchMaterializationReservationSet,
    reservation_consumption: SketchMaterializationReservationConsumption,
    points: Vec<DesignPoint>,
    scalars: Vec<DesignScalar>,
    curves: Vec<DesignCurve>,
    contacts: Vec<ContactSlot>,
    trim_views: Vec<DocumentCurveTrimView>,
    constraints: Vec<DocumentConstraint>,
    dimensions: Vec<DocumentDimension>,
    parameters: Vec<DocumentParameter>,
    parameter_bindings: Vec<DocumentParameterBinding>,
    parameter_outputs: Vec<DocumentParameterOutput>,
    external_bindings: Vec<DocumentExternalBinding>,
    source_order: Vec<DocumentSourceId>,
    geometry_roles: Vec<GeometryRoleEdit>,
    user_inactive_elements: Vec<DocumentElementId>,
    host_activation: Option<HostConfigurationActivation>,
    semantic_catalogs: Vec<SketchMaterializationSemanticCatalog>,
}

impl SketchMaterializationBatch {
    /// Creates an empty batch backed by one validated typed reservation interval.
    #[must_use]
    pub fn new(reservations: SketchMaterializationReservationSet) -> Self {
        Self {
            reservations,
            reservation_consumption: SketchMaterializationReservationConsumption::Exact,
            points: Vec::new(),
            scalars: Vec::new(),
            curves: Vec::new(),
            contacts: Vec::new(),
            trim_views: Vec::new(),
            constraints: Vec::new(),
            dimensions: Vec::new(),
            parameters: Vec::new(),
            parameter_bindings: Vec::new(),
            parameter_outputs: Vec::new(),
            external_bindings: Vec::new(),
            source_order: Vec::new(),
            geometry_roles: Vec::new(),
            user_inactive_elements: Vec::new(),
            host_activation: None,
            semantic_catalogs: Vec::new(),
        }
    }

    /// Creates an identity-retirement batch that may leave typed reservations unmaterialized.
    ///
    /// This explicit mode is intended for suppressed, deleted, or failed retained owners. The
    /// resulting high-water still advances, so later materialization cannot reuse those IDs.
    #[must_use]
    pub fn retaining_unused_reservations(
        reservations: SketchMaterializationReservationSet,
    ) -> Self {
        let mut batch = Self::new(reservations);
        batch.reservation_consumption = SketchMaterializationReservationConsumption::RetainUnused;
        batch
    }

    #[must_use]
    pub const fn reservations(&self) -> &SketchMaterializationReservationSet {
        &self.reservations
    }

    #[must_use]
    pub const fn reservation_consumption(&self) -> SketchMaterializationReservationConsumption {
        self.reservation_consumption
    }

    pub fn push_point(&mut self, point: DesignPoint) {
        self.points.push(point);
    }

    pub fn push_scalar(&mut self, scalar: DesignScalar) {
        self.scalars.push(scalar);
    }

    pub fn push_curve(&mut self, curve: DesignCurve) {
        self.curves.push(curve);
    }

    pub fn push_contact(&mut self, contact: ContactSlot) {
        self.contacts.push(contact);
    }

    pub fn push_trim_view(&mut self, view: DocumentCurveTrimView) {
        self.trim_views.push(view);
    }

    pub fn push_constraint(&mut self, constraint: DocumentConstraint) {
        self.constraints.push(constraint);
    }

    pub fn push_dimension(&mut self, dimension: DocumentDimension) {
        self.dimensions.push(dimension);
    }

    pub fn push_parameter(&mut self, parameter: DocumentParameter) {
        self.parameters.push(parameter);
    }

    pub fn push_parameter_binding(&mut self, binding: DocumentParameterBinding) {
        self.parameter_bindings.push(binding);
    }

    pub fn push_parameter_output(&mut self, output: DocumentParameterOutput) {
        self.parameter_outputs.push(output);
    }

    pub fn push_external_binding(&mut self, binding: DocumentExternalBinding) {
        self.external_bindings.push(binding);
    }

    /// Appends one newly materialized ordinary source in semantic execution/audit order.
    pub fn push_source(&mut self, source: DocumentSourceId) {
        self.source_order.push(source);
    }

    pub fn push_geometry_role(&mut self, edit: GeometryRoleEdit) {
        self.geometry_roles.push(edit);
    }

    pub fn push_user_inactive_element(&mut self, element: DocumentElementId) {
        self.user_inactive_elements.push(element);
    }

    /// Sets the one optional host-activation payload introduced by this batch.
    pub fn set_host_activation(&mut self, activation: HostConfigurationActivation) {
        self.host_activation = Some(activation);
    }

    pub fn push_semantic_catalog(&mut self, catalog: SketchMaterializationSemanticCatalog) {
        self.semantic_catalogs.push(catalog);
    }
}

impl SketchDocument {
    /// Atomically inserts a reserved materialization batch and advances all allocator high-water.
    ///
    /// The method validates the live base, namespace, reservation kinds, duplicates, source order,
    /// semantic ownership, references, domains, branches, and spline cursors on scratch state.
    /// The receiver is replaced only after ordinary complete [`SketchDocument::validate`]
    /// succeeds.
    ///
    /// # Errors
    ///
    /// Returns a typed document error without changing any receiver state when any structural,
    /// identity, reference, domain, or complete-document validation fails.
    pub fn apply_materialization_batch(
        &mut self,
        batch: &SketchMaterializationBatch,
    ) -> Result<(), DocumentError> {
        self.validate()?;
        let validated_roles = batch.reservations.validate_structure()?;
        if validated_roles != batch.reservations.role_by_id {
            return materialization_invalid("materialization reservation index is inconsistent");
        }
        validate_base_high_water(self, &batch.reservations.base)?;
        validate_batch_identity_consumption(self, batch)?;

        let mut candidate = self.clone();
        candidate.advance_spline_span_allocators(&batch.reservations.base.spline_span_cursors);
        candidate.next_id = batch.reservations.resulting.next_id;
        candidate.points.extend(batch.points.iter().cloned());
        candidate.scalars.extend(batch.scalars.iter().cloned());
        candidate.curves.extend(batch.curves.iter().cloned());
        candidate.contacts.extend(batch.contacts.iter().cloned());
        candidate
            .trim_views
            .extend(batch.trim_views.iter().copied());
        candidate
            .constraints
            .extend(batch.constraints.iter().cloned());
        candidate
            .dimensions
            .extend(batch.dimensions.iter().cloned());
        candidate
            .parameters
            .extend(batch.parameters.iter().cloned());
        candidate
            .parameter_bindings
            .extend(batch.parameter_bindings.iter().copied());
        candidate
            .parameter_outputs
            .extend(batch.parameter_outputs.iter().copied());
        candidate
            .external_bindings
            .extend(batch.external_bindings.iter().cloned());
        candidate
            .source_order
            .extend(batch.source_order.iter().copied());

        for edit in &batch.geometry_roles {
            match edit.role {
                GeometryRole::Profile => {
                    candidate.geometry_roles.remove(&edit.curve);
                }
                GeometryRole::Construction => {
                    candidate.geometry_roles.insert(edit.curve, edit.role);
                }
            }
        }
        candidate
            .user_inactive_elements
            .extend(batch.user_inactive_elements.iter().copied());
        if let Some(activation) = &batch.host_activation {
            candidate.host_activation = Some(activation.clone());
        }
        apply_semantic_catalogs(&mut candidate, &batch.semantic_catalogs);
        validate_resulting_spline_cursors(&candidate, &batch.reservations.resulting)?;
        candidate.advance_spline_span_allocators(&batch.reservations.resulting.spline_span_cursors);
        candidate.canonicalize();
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

fn validate_base_high_water(
    document: &SketchDocument,
    base: &SketchPersistentIdentityHighWater,
) -> Result<(), DocumentError> {
    if document.id != base.document {
        return materialization_invalid("materialization batch belongs to a foreign namespace");
    }
    if document.next_id != base.next_id {
        return materialization_invalid("materialization batch has a stale base allocator cursor");
    }
    for (curve, actual) in document.spline_span_allocator_cursors() {
        let Some(expected) = base.spline_span_cursors.get(&curve) else {
            return materialization_invalid("materialization base omits a live spline span cursor");
        };
        if *expected < actual {
            return materialization_invalid("materialization base regresses a live spline cursor");
        }
    }
    for curve in base.spline_span_cursors.keys() {
        if let Some(definition) = document.curve(*curve).map(|curve| &curve.definition)
            && !matches!(
                definition,
                CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. }
            )
        {
            return materialization_invalid(
                "materialization base assigns a spline cursor to a non-spline curve",
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_batch_identity_consumption(
    document: &SketchDocument,
    batch: &SketchMaterializationBatch,
) -> Result<(), DocumentError> {
    let reservations = &batch.reservations;
    let mut consumed = BTreeSet::new();
    let mut consume = |id: PersistentId, expected: ReservedIdentityRole| {
        if !consumed.insert(id) {
            return Err(DocumentError::DuplicateId(id));
        }
        let Some(actual) = reservations.role(id) else {
            return materialization_invalid("materialized object has no typed reservation");
        };
        if actual != expected {
            return materialization_invalid(
                "materialized object identity was reserved for a different kind or owner",
            );
        }
        if document.element(id).is_some()
            || document
                .semantic_source_reservations
                .contains_key(&DocumentSourceId(id))
        {
            return Err(DocumentError::DuplicateId(id));
        }
        Ok(())
    };

    for point in &batch.points {
        consume(point.id.0, ReservedIdentityRole::Point)?;
    }
    for scalar in &batch.scalars {
        consume(scalar.id.0, ReservedIdentityRole::Scalar)?;
    }
    for curve in &batch.curves {
        consume(curve.id.0, ReservedIdentityRole::Curve)?;
    }
    for contact in &batch.contacts {
        consume(contact.id.0, ReservedIdentityRole::Contact)?;
    }
    let mut new_sources = BTreeSet::new();
    for constraint in &batch.constraints {
        consume(
            constraint.id.0,
            ReservedIdentityRole::Constraint {
                source: constraint.source_id,
            },
        )?;
        consume(
            constraint.source_id.0,
            ReservedIdentityRole::ConstraintSource {
                owner: constraint.id,
            },
        )?;
        new_sources.insert(constraint.source_id);
    }
    for dimension in &batch.dimensions {
        consume(
            dimension.id.0,
            ReservedIdentityRole::Dimension {
                source: dimension.source_id,
            },
        )?;
        consume(
            dimension.source_id.0,
            ReservedIdentityRole::DimensionSource {
                owner: dimension.id,
            },
        )?;
        new_sources.insert(dimension.source_id);
    }
    for parameter in &batch.parameters {
        consume(parameter.id.0, ReservedIdentityRole::Parameter)?;
    }
    for binding in &batch.external_bindings {
        consume(binding.id.0, ReservedIdentityRole::ExternalBinding)?;
    }

    let ordered = batch.source_order.iter().copied().collect::<BTreeSet<_>>();
    if ordered.len() != batch.source_order.len() || ordered != new_sources {
        return materialization_invalid(
            "batch source order must contain every materialized source exactly once",
        );
    }

    let mut role_curves = BTreeSet::new();
    for edit in &batch.geometry_roles {
        if !role_curves.insert(edit.curve) {
            return Err(DocumentError::DuplicateId(edit.curve.0));
        }
    }
    let mut inactive = BTreeSet::new();
    for element in &batch.user_inactive_elements {
        if !inactive.insert(*element) {
            return Err(DocumentError::DuplicateId(element.persistent_id()));
        }
    }
    if document.host_activation.is_some() && batch.host_activation.is_some() {
        return materialization_invalid(
            "a materialization batch cannot replace an existing host activation payload",
        );
    }

    validate_semantic_catalogs(
        document,
        reservations,
        &batch.semantic_catalogs,
        &mut consumed,
    )?;
    if batch.reservation_consumption == SketchMaterializationReservationConsumption::Exact
        && consumed.len() != reservations.role_by_id.len()
    {
        return materialization_invalid(
            "exact materialization must consume every typed reservation once",
        );
    }
    Ok(())
}

fn validate_semantic_catalogs(
    document: &SketchDocument,
    reservations: &SketchMaterializationReservationSet,
    catalogs: &[SketchMaterializationSemanticCatalog],
    consumed: &mut BTreeSet<PersistentId>,
) -> Result<(), DocumentError> {
    let mut seen_catalogs = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    for entry in catalogs {
        if !seen_catalogs.insert(entry.catalog) {
            return Err(DocumentError::DuplicateId(entry.catalog.0));
        }
        match document.semantic_reservation_owner(entry.catalog) {
            Some(owner) if owner == entry.catalog => {}
            Some(_) => {
                return materialization_invalid(
                    "semantic catalog identity is owned as a semantic source",
                );
            }
            None => {
                if !consumed.insert(entry.catalog.0) {
                    return Err(DocumentError::DuplicateId(entry.catalog.0));
                }
                if reservations.role(entry.catalog.0) != Some(ReservedIdentityRole::SemanticCatalog)
                {
                    return materialization_invalid(
                        "new semantic catalog lacks a semantic-catalog reservation",
                    );
                }
            }
        }
        for source in &entry.sources {
            if !seen_sources.insert(*source) || !consumed.insert(source.0) {
                return Err(DocumentError::DuplicateId(source.0));
            }
            if document.element(source.0).is_some()
                || document.semantic_reservation_owner(*source).is_some()
            {
                return Err(DocumentError::DuplicateId(source.0));
            }
            if reservations.role(source.0)
                != Some(ReservedIdentityRole::SemanticSource {
                    catalog: entry.catalog,
                })
            {
                return materialization_invalid(
                    "semantic source reservation names the wrong catalog owner",
                );
            }
        }
    }
    Ok(())
}

fn apply_semantic_catalogs(
    document: &mut SketchDocument,
    catalogs: &[SketchMaterializationSemanticCatalog],
) {
    for entry in catalogs {
        document
            .semantic_source_reservations
            .entry(entry.catalog)
            .or_insert(entry.catalog);
        for source in &entry.sources {
            document
                .semantic_source_reservations
                .insert(*source, entry.catalog);
        }
    }
}

fn validate_resulting_spline_cursors(
    candidate: &SketchDocument,
    resulting: &SketchPersistentIdentityHighWater,
) -> Result<(), DocumentError> {
    for curve in &candidate.curves {
        match &curve.definition {
            CurveDefinition::BSpline { next_span_id, .. }
            | CurveDefinition::Nurbs { next_span_id, .. } => {
                let Some(retained) = resulting.spline_span_cursors.get(&curve.id) else {
                    return materialization_invalid(
                        "resulting high-water omits a materialized spline cursor",
                    );
                };
                if retained < next_span_id {
                    return materialization_invalid(
                        "resulting high-water regresses a materialized spline cursor",
                    );
                }
            }
            _ if resulting.spline_span_cursors.contains_key(&curve.id) => {
                return materialization_invalid(
                    "resulting high-water assigns a spline cursor to a non-spline curve",
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn materialization_invalid<T>(message: impl Into<String>) -> Result<T, DocumentError> {
    Err(materialization_invalid_error(message))
}

fn materialization_invalid_error(message: impl Into<String>) -> DocumentError {
    DocumentError::InvalidField {
        field: "materialization batch",
        message: message.into(),
    }
}
