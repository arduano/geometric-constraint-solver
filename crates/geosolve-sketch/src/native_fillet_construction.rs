// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic materialization of one branch-explicit line-line Fillet as ordinary native topology.

use std::collections::BTreeSet;

use crate::{
    ContactDefinition, ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveId,
    CurveSpan, DesignPointId, DesignScalarId, DocumentArcSweep, DocumentConstraintDefinition,
    DocumentConstraintId, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentError, DocumentFilletEndpointOrder,
    DocumentObjectId, FeatureEndpoint, GeometryRole, ScalarDomain, ScalarUnit, SketchDocument,
    TangentOrientation,
};
use geosolve_core::{OperationCheckpoint, OperationControl, OperationController, OperationOutcome};

const NATIVE_FILLET_GEOMETRY_EPSILON: f64 = 1.0e-9;

/// One retained line and the exact endpoint replaced by a native Fillet contact.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNativeLineFilletParent {
    pub curve: CurveSpan,
    pub endpoint: FeatureEndpoint,
    pub normal_side: DocumentCurveNormalSide,
    pub tangent_orientation: TangentOrientation,
    pub contact_position: [f64; 2],
}

/// Complete accepted-geometry request for one ordinary native line-line Fillet.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNativeLineFilletCreationRequest {
    pub label: String,
    pub first: DocumentNativeLineFilletParent,
    pub second: DocumentNativeLineFilletParent,
    pub endpoint_order: DocumentFilletEndpointOrder,
    pub center: [f64; 2],
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub sweep: DocumentArcSweep,
}

/// Opaque accepted-geometry-derived plan for one native line-line Fillet.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentPreparedNativeLineFilletGeometry {
    request: DocumentNativeLineFilletCreationRequest,
    source_definitions: [CurveDefinition; 2],
    accepted_line_tangents: [[f64; 2]; 2],
    corner: DesignPointId,
    tangent_orientations: [TangentOrientation; 2],
    expected_ids: Option<DocumentNativeLineFilletIds>,
}

impl DocumentPreparedNativeLineFilletGeometry {
    /// Returns the deterministic persistent identities proved by accepted-document preparation.
    ///
    /// # Panics
    ///
    /// This cannot panic for a value returned by
    /// [`SketchDocument::prepare_native_line_fillet_geometry`]. The optional internal slot exists
    /// only while that method trials an unpublished clone.
    #[must_use]
    pub fn expected_ids(&self) -> &DocumentNativeLineFilletIds {
        self.expected_ids
            .as_ref()
            .expect("a public prepared native Fillet always carries its trial identities")
    }
}

/// Persistent identities created or replaced by one native line-line Fillet transaction.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNativeLineFilletIds {
    pub source_lines: [CurveId; 2],
    pub removed_corner: DesignPointId,
    pub contact_points: [DesignPointId; 2],
    pub arc: CurveId,
    pub center: DesignPointId,
    pub radius: DesignScalarId,
    pub start_angle: DesignScalarId,
    pub end_angle: DesignScalarId,
    pub contacts: [ContactId; 2],
    pub contact_parameters: [DesignScalarId; 2],
    pub tangencies: [DocumentConstraintId; 2],
    pub radius_dimension: DocumentDimensionId,
    pub radius_target: DesignScalarId,
}

impl SketchDocument {
    /// Authenticates one accepted line-line Fillet preview without mutating the document.
    ///
    /// The returned plan retains the exact source definitions and finite branch geometry. A
    /// retained design may consume it only while those source definitions and roles still match.
    ///
    /// # Errors
    ///
    /// Rejects non-line, non-Profile, trimmed, disconnected, dependent, non-interior, stale, or
    /// geometrically inconsistent requests.
    pub fn prepare_native_line_fillet_geometry(
        &self,
        request: DocumentNativeLineFilletCreationRequest,
    ) -> Result<DocumentPreparedNativeLineFilletGeometry, DocumentError> {
        let outcome = self.prepare_native_line_fillet_geometry_controlled(
            request,
            OperationControl::unlimited(),
        )?;
        let OperationOutcome::Completed { value, .. } = outcome else {
            unreachable!("unlimited native Fillet preparation cannot be interrupted");
        };
        Ok(value)
    }

    /// Authenticates and trials one accepted line-line Fillet preview under deterministic work
    /// and cancellation control without mutating the document.
    ///
    /// # Errors
    ///
    /// Returns the same semantic validation errors as
    /// [`Self::prepare_native_line_fillet_geometry`]. Interrupted work returns a typed incomplete
    /// outcome and retains the exact source document and allocator state.
    pub fn prepare_native_line_fillet_geometry_controlled(
        &self,
        request: DocumentNativeLineFilletCreationRequest,
        control: OperationControl,
    ) -> Result<OperationOutcome<DocumentPreparedNativeLineFilletGeometry>, DocumentError> {
        let mut controller = OperationController::new(control);
        if controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let mut prepared = self.prepare_native_line_fillet_geometry_untrialed(request)?;
        let corner = prepared.corner;
        let mut trial = self.clone();
        trial.defer_mutation_validation();
        let expected = match trial.create_prepared_native_line_fillet_geometry(prepared.clone()) {
            Err(DocumentError::ObjectInUse(id)) if id == corner.0 => {
                return native_fillet_error(
                    "native fillet corner",
                    "shared corner must be owned only by the two selected source lines",
                );
            }
            result => result?,
        };
        trial.resume_mutation_validation();
        if !trial.validate_with_controller(Some(&mut controller))? {
            return Ok(controller.outcome_unchecked());
        }
        if controller
            .checkpoint(OperationCheckpoint::AfterFinalValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        prepared.expected_ids = Some(expected);
        Ok(controller.outcome(prepared))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "source authentication and the complete explicit Fillet branch are intentionally checked at one preparation boundary"
    )]
    fn prepare_native_line_fillet_geometry_untrialed(
        &self,
        request: DocumentNativeLineFilletCreationRequest,
    ) -> Result<DocumentPreparedNativeLineFilletGeometry, DocumentError> {
        validate_finite_pair(request.center, "native fillet center")?;
        validate_finite_pair(
            request.first.contact_position,
            "native fillet first contact",
        )?;
        validate_finite_pair(
            request.second.contact_position,
            "native fillet second contact",
        )?;
        validate_finite_positive(request.radius, "native fillet radius")?;
        validate_finite(request.start_angle, "native fillet start angle")?;
        validate_finite(request.end_angle, "native fillet end angle")?;
        if request.first.curve == request.second.curve {
            return native_fillet_error("native fillet parents", "line spans must be distinct");
        }

        let parents = [request.first, request.second];
        let mut definitions = Vec::with_capacity(2);
        let mut line_tangents = Vec::with_capacity(2);
        let mut corner = None;
        let mut orientations = Vec::with_capacity(2);
        for (index, parent) in parents.into_iter().enumerate() {
            if parent.curve.segment != 0 {
                return native_fillet_error(
                    "native fillet parent",
                    "only complete standalone line spans are supported",
                );
            }
            let curve = self.curve(parent.curve.curve).ok_or_else(|| {
                native_fillet_error_value("native fillet parent", "source curve is missing")
            })?;
            let CurveDefinition::Line { start, end, .. } = &curve.definition else {
                return native_fillet_error(
                    "native fillet parent",
                    "only standalone line curves are supported",
                );
            };
            if self.geometry_role(parent.curve.curve) != Some(GeometryRole::Profile) {
                return native_fillet_error(
                    "native fillet parent",
                    "both source lines must be Profile geometry",
                );
            }
            if self.trim_views_for_span(parent.curve).next().is_some() {
                return native_fillet_error(
                    "native fillet parent",
                    "trimmed or partial source lines are not supported",
                );
            }
            let endpoint = match parent.endpoint {
                FeatureEndpoint::Start => *start,
                FeatureEndpoint::End => *end,
            };
            if let Some(expected) = corner {
                if expected != endpoint {
                    return native_fillet_error(
                        "native fillet corner",
                        "selected line endpoints do not share one persistent point",
                    );
                }
            } else {
                corner = Some(endpoint);
            }
            let start_position = self
                .point(*start)
                .ok_or_else(|| {
                    native_fillet_error_value("native fillet parent", "line start is missing")
                })?
                .position;
            let end_position = self
                .point(*end)
                .ok_or_else(|| {
                    native_fillet_error_value("native fillet parent", "line end is missing")
                })?
                .position;
            let tangent = normalized(sub(end_position, start_position), "native fillet line")?;
            validate_strict_line_contact(
                start_position,
                end_position,
                parent.contact_position,
                self.model_scale(),
            )?;
            validate_sided_center(
                parent.contact_position,
                tangent,
                parent.normal_side,
                request.center,
                request.radius,
                self.model_scale(),
            )?;
            let arc_parameter = match (request.endpoint_order, index) {
                (DocumentFilletEndpointOrder::FirstThenSecond, 0)
                | (DocumentFilletEndpointOrder::SecondThenFirst, 1) => 0.0,
                (DocumentFilletEndpointOrder::FirstThenSecond, 1)
                | (DocumentFilletEndpointOrder::SecondThenFirst, 0) => 1.0,
                _ => unreachable!("two native Fillet parents"),
            };
            let arc_tangent = arc_endpoint_tangent(
                request.start_angle,
                request.end_angle,
                request.sweep,
                arc_parameter,
            )?;
            let actual_orientation = if dot(tangent, arc_tangent) > 0.0 {
                TangentOrientation::Aligned
            } else {
                TangentOrientation::Opposed
            };
            if actual_orientation != parent.tangent_orientation {
                return native_fillet_error(
                    "native fillet tangent orientation",
                    "explicit tangent orientation disagrees with the accepted preview geometry",
                );
            }
            orientations.push(parent.tangent_orientation);
            line_tangents.push(tangent);
            definitions.push(curve.definition.clone());
        }

        validate_arc_geometry(&request, self.model_scale())?;
        let Some(corner) = corner else {
            return native_fillet_error("native fillet corner", "two parents are required");
        };
        let source_definitions: [CurveDefinition; 2] = definitions.try_into().map_err(|_| {
            native_fillet_error_value("native fillet parents", "exactly two parents are required")
        })?;
        let accepted_line_tangents: [[f64; 2]; 2] = line_tangents.try_into().map_err(|_| {
            native_fillet_error_value(
                "native fillet line tangents",
                "exactly two line tangents are required",
            )
        })?;
        let tangent_orientations: [TangentOrientation; 2] =
            orientations.try_into().map_err(|_| {
                native_fillet_error_value(
                    "native fillet tangent orientations",
                    "exactly two tangent branches are required",
                )
            })?;

        Ok(DocumentPreparedNativeLineFilletGeometry {
            request,
            source_definitions,
            accepted_line_tangents,
            corner,
            tangent_orientations,
            expected_ids: None,
        })
    }

    /// Creates one ordinary shortened-line/arc/shortened-line Fillet atomically.
    ///
    /// # Errors
    ///
    /// Returns the same typed validation failures as preparation and leaves this document exact on
    /// any allocation, dependency, or document-validation failure.
    pub fn create_native_line_fillet_geometry(
        &mut self,
        request: DocumentNativeLineFilletCreationRequest,
    ) -> Result<DocumentNativeLineFilletIds, DocumentError> {
        let prepared = self.prepare_native_line_fillet_geometry(request)?;
        self.create_prepared_native_line_fillet_geometry(prepared)
    }

    /// Applies an accepted-geometry-derived native Fillet plan to a compatible retained design.
    ///
    /// # Errors
    ///
    /// Rejects changed source topology/roles, a dependent sharp point, allocation failure, or an
    /// invalid resulting document. No prefix is published.
    #[allow(clippy::too_many_lines)]
    pub fn create_prepared_native_line_fillet_geometry(
        &mut self,
        prepared: DocumentPreparedNativeLineFilletGeometry,
    ) -> Result<DocumentNativeLineFilletIds, DocumentError> {
        let mut candidate = self.clone();
        let expected_ids = prepared.expected_ids.clone();
        let contact_positions = prepared_contact_seed_positions(&candidate, &prepared)?;
        let request = prepared.request;
        let parents = [request.first, request.second];
        for (parent, definition) in parents.iter().zip(&prepared.source_definitions) {
            let retained = candidate.curve(parent.curve.curve).ok_or_else(|| {
                native_fillet_error_value("native fillet prepared source", "source is missing")
            })?;
            if retained.definition != *definition
                || candidate.geometry_role(parent.curve.curve) != Some(GeometryRole::Profile)
                || candidate.trim_views_for_span(parent.curve).next().is_some()
            {
                return native_fillet_error(
                    "native fillet prepared source",
                    "source definition, role, or complete-span state changed after preparation",
                );
            }
        }

        let contact_points = [
            candidate.add_point(
                format!("{}.first_contact_point", request.label),
                contact_positions[0],
            )?,
            candidate.add_point(
                format!("{}.second_contact_point", request.label),
                contact_positions[1],
            )?,
        ];
        for (parent, contact) in parents.iter().zip(contact_points) {
            let curve = candidate.curve_mut(parent.curve.curve).ok_or_else(|| {
                native_fillet_error_value("native fillet prepared source", "source is missing")
            })?;
            let CurveDefinition::Line { start, end, .. } = &mut curve.definition else {
                return native_fillet_error(
                    "native fillet prepared source",
                    "source family changed after preparation",
                );
            };
            let replaced = match parent.endpoint {
                FeatureEndpoint::Start => std::mem::replace(start, contact),
                FeatureEndpoint::End => std::mem::replace(end, contact),
            };
            if replaced != prepared.corner {
                return native_fillet_error(
                    "native fillet prepared source",
                    "selected corner endpoint changed after preparation",
                );
            }
        }
        candidate.remove(DocumentObjectId::Point(prepared.corner))?;

        let center = candidate.add_point(format!("{}.center", request.label), request.center)?;
        let radius = candidate.add_scalar(
            format!("{}.radius", request.label),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let start_angle = candidate.add_scalar(
            format!("{}.start_angle", request.label),
            request.start_angle,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let end_angle = candidate.add_scalar(
            format!("{}.end_angle", request.label),
            request.end_angle,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )?;
        let arc = candidate.add_curve_with_role(
            format!("{}.arc", request.label),
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep: request.sweep,
            },
            GeometryRole::Profile,
        )?;

        let arc_parameters: [f64; 2] = match request.endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => [0.0, 1.0],
            DocumentFilletEndpointOrder::SecondThenFirst => [1.0, 0.0],
        };
        let mut contact_parameters = Vec::with_capacity(2);
        let mut contacts = Vec::with_capacity(2);
        let mut tangencies = Vec::with_capacity(2);
        for (index, parent) in parents.iter().enumerate() {
            let parameter = arc_parameters[index];
            let parameter_id = candidate.add_scalar(
                format!("{}.arc_contact_{}_parameter", request.label, index + 1),
                parameter,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )?;
            let contact = candidate.add_contact(
                format!("{}.arc_contact_{}", request.label, index + 1),
                ContactDefinition {
                    curve: CurveSpan {
                        curve: arc,
                        segment: 0,
                    },
                    parameter: parameter_id,
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    winding: 0,
                    neighborhood: if parameter == 0.0 {
                        ContactNeighborhood::Start
                    } else {
                        ContactNeighborhood::End
                    },
                    tangent_orientation: Some(prepared.tangent_orientations[index]),
                },
            )?;
            let tangency = candidate.add_constraint(
                format!("{}.tangency_{}", request.label, index + 1),
                DocumentConstraintDefinition::LineCurveTangency {
                    line: parent.curve,
                    endpoint: parent.endpoint,
                    curve_contact: contact,
                },
            )?;
            contact_parameters.push(parameter_id);
            contacts.push(contact);
            tangencies.push(tangency);
        }
        let radius_target = candidate.add_scalar(
            format!("{}.radius_target", request.label),
            request.radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )?;
        let radius_dimension = candidate.add_dimension(
            format!("{}.radius_dimension", request.label),
            DocumentDimensionDefinition::Radius {
                curve: arc,
                target: radius_target,
            },
            DocumentDimensionMode::Driving,
        )?;
        candidate.validate_after_mutation()?;
        let contacts: [ContactId; 2] = contacts.try_into().map_err(|_| {
            native_fillet_error_value(
                "native fillet contacts",
                "exactly two contacts are required",
            )
        })?;
        let contact_parameters: [DesignScalarId; 2] =
            contact_parameters.try_into().map_err(|_| {
                native_fillet_error_value(
                    "native fillet contact parameters",
                    "exactly two contact parameters are required",
                )
            })?;
        let tangencies: [DocumentConstraintId; 2] = tangencies.try_into().map_err(|_| {
            native_fillet_error_value(
                "native fillet tangencies",
                "exactly two tangencies are required",
            )
        })?;
        let ids = DocumentNativeLineFilletIds {
            source_lines: [request.first.curve.curve, request.second.curve.curve],
            removed_corner: prepared.corner,
            contact_points,
            arc,
            center,
            radius,
            start_angle,
            end_angle,
            contacts,
            contact_parameters,
            tangencies,
            radius_dimension,
            radius_target,
        };
        if expected_ids
            .as_ref()
            .is_some_and(|expected| *expected != ids)
        {
            return native_fillet_error(
                "native fillet prepared identities",
                "retained design allocation no longer matches the accepted prepared plan",
            );
        }
        *self = candidate;
        Ok(ids)
    }

    /// Reconstructs the exact accepted numerical seed for an already materialized
    /// native Fillet action.
    ///
    /// `self` is the independently accepted upstream prefix and
    /// `upstream_design` is its retained counterpart. The opaque prepared action
    /// is reauthenticated against that upstream geometry while its exact reserved
    /// output identities are checked against `materialized`. Only continuation
    /// values are copied; no topology or identity is created or removed.
    ///
    /// The returned graph is a numerical seed, not accepted authority. Callers
    /// must still certify it through the ordinary retained-session publication
    /// boundary.
    ///
    /// # Errors
    ///
    /// Rejects stale/forged prepared geometry, a foreign or differently
    /// materialized output graph, missing identities, or invalid resulting state.
    #[allow(
        clippy::too_many_lines,
        reason = "one domain boundary authenticates every native Fillet identity before changing any numerical seed"
    )]
    pub fn prepare_materialized_native_line_fillet_seed(
        &self,
        upstream_design: &Self,
        materialized: &Self,
        prepared: &DocumentPreparedNativeLineFilletGeometry,
    ) -> Result<Self, DocumentError> {
        if self.id() != upstream_design.id() || self.id() != materialized.id() {
            return native_fillet_error(
                "native fillet continuation",
                "upstream and materialized graphs belong to different documents",
            );
        }
        let authenticated = self.prepare_native_line_fillet_geometry(prepared.request.clone())?;
        if authenticated.request != prepared.request
            || authenticated.source_definitions != prepared.source_definitions
            || authenticated.accepted_line_tangents != prepared.accepted_line_tangents
            || authenticated.corner != prepared.corner
            || authenticated.tangent_orientations != prepared.tangent_orientations
        {
            return native_fillet_error(
                "native fillet continuation",
                "prepared action does not match the accepted upstream prefix",
            );
        }
        let ids = prepared.expected_ids.as_ref().ok_or_else(|| {
            native_fillet_error_value(
                "native fillet continuation",
                "prepared action has no reserved output identities",
            )
        })?;
        // Session-global never-reuse cursors may be ahead of this historical
        // action after Undo or divergence. Authenticate its prepared IDs
        // against the owning intent reservations and complete materialized identity
        // delta below, never against a fresh allocation from this prefix.
        let request = &prepared.request;
        if ids.source_lines != [request.first.curve.curve, request.second.curve.curve]
            || materialized.point(ids.removed_corner).is_some()
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized source or retired-corner identity differs from the prepared action",
            );
        }
        validate_materialized_native_line_fillet_metadata(
            upstream_design,
            materialized,
            prepared,
            ids,
        )?;

        let parents = [request.first, request.second];
        for (index, (parent, source_definition)) in
            parents.iter().zip(&prepared.source_definitions).enumerate()
        {
            let mut expected = source_definition.clone();
            let CurveDefinition::Line { start, end, .. } = &mut expected else {
                return native_fillet_error(
                    "native fillet continuation",
                    "prepared source is no longer a line",
                );
            };
            let replaced = match parent.endpoint {
                FeatureEndpoint::Start => std::mem::replace(start, ids.contact_points[index]),
                FeatureEndpoint::End => std::mem::replace(end, ids.contact_points[index]),
            };
            let upstream_source =
                upstream_design
                    .curve(ids.source_lines[index])
                    .ok_or_else(|| {
                        native_fillet_error_value(
                            "native fillet continuation",
                            "retained upstream source is missing",
                        )
                    })?;
            if replaced != ids.removed_corner
                || materialized
                    .curve(ids.source_lines[index])
                    .is_none_or(|curve| {
                        curve.label != upstream_source.label || curve.definition != expected
                    })
                || materialized.geometry_role(ids.source_lines[index])
                    != Some(GeometryRole::Profile)
            {
                return native_fillet_error(
                    "native fillet continuation",
                    "materialized shortened source differs from the prepared action",
                );
            }
        }

        let expected_arc = CurveDefinition::CircularArc {
            center: ids.center,
            radius: ids.radius,
            start_angle: ids.start_angle,
            end_angle: ids.end_angle,
            sweep: request.sweep,
        };
        if materialized
            .curve(ids.arc)
            .is_none_or(|curve| curve.definition != expected_arc)
            || materialized.geometry_role(ids.arc) != Some(GeometryRole::Profile)
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized arc differs from the prepared action",
            );
        }

        let arc_parameters: [f64; 2] = match request.endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => [0.0, 1.0],
            DocumentFilletEndpointOrder::SecondThenFirst => [1.0, 0.0],
        };
        for (index, parent) in parents.iter().enumerate() {
            let parameter = materialized
                .scalar(ids.contact_parameters[index])
                .ok_or_else(|| {
                    native_fillet_error_value(
                        "native fillet continuation",
                        "materialized contact parameter is missing",
                    )
                })?;
            let contact = materialized
                .contacts()
                .iter()
                .find(|contact| contact.id == ids.contacts[index]);
            let tangency = materialized.constraint(ids.tangencies[index]);
            if parameter.value.to_bits() != arc_parameters[index].to_bits()
                || contact.is_none_or(|contact| {
                    contact.curve.curve != ids.arc
                        || contact.parameter != ids.contact_parameters[index]
                        || contact.tangent_orientation != Some(prepared.tangent_orientations[index])
                })
                || tangency.is_none_or(|constraint| {
                    constraint.definition
                        != DocumentConstraintDefinition::LineCurveTangency {
                            line: parent.curve,
                            endpoint: parent.endpoint,
                            curve_contact: ids.contacts[index],
                        }
                })
            {
                return native_fillet_error(
                    "native fillet continuation",
                    "materialized contact or tangency differs from the prepared action",
                );
            }
        }
        if materialized
            .dimension(ids.radius_dimension)
            .is_none_or(|dimension| {
                dimension.mode != DocumentDimensionMode::Driving
                    || dimension.definition
                        != DocumentDimensionDefinition::Radius {
                            curve: ids.arc,
                            target: ids.radius_target,
                        }
            })
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized radius dimension differs from the prepared action",
            );
        }

        let mut seed = materialized.prepare_continuation_seed(upstream_design, self)?;
        for (point, position) in [
            (ids.contact_points[0], request.first.contact_position),
            (ids.contact_points[1], request.second.contact_position),
            (ids.center, request.center),
        ] {
            seed.point_mut(point)
                .ok_or_else(|| {
                    native_fillet_error_value(
                        "native fillet continuation",
                        "materialized Fillet point is missing",
                    )
                })?
                .position = position;
        }
        for (scalar, value) in [
            (ids.radius, request.radius),
            (ids.start_angle, request.start_angle),
            (ids.end_angle, request.end_angle),
            (ids.radius_target, request.radius),
        ] {
            seed.scalar_mut(scalar)
                .ok_or_else(|| {
                    native_fillet_error_value(
                        "native fillet continuation",
                        "materialized Fillet scalar is missing",
                    )
                })?
                .value = value;
        }
        seed.validate()?;
        Ok(seed)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "native Fillet replay authenticates its complete persistent topology and audit metadata at one domain boundary"
)]
fn validate_materialized_native_line_fillet_metadata(
    upstream: &SketchDocument,
    materialized: &SketchDocument,
    prepared: &DocumentPreparedNativeLineFilletGeometry,
    ids: &DocumentNativeLineFilletIds,
) -> Result<(), DocumentError> {
    let request = &prepared.request;
    let expected_point_ids = upstream
        .points()
        .iter()
        .map(|point| point.id)
        .filter(|point| *point != ids.removed_corner)
        .chain(ids.contact_points)
        .chain([ids.center])
        .collect::<BTreeSet<_>>();
    let actual_point_ids = materialized
        .points()
        .iter()
        .map(|point| point.id)
        .collect::<BTreeSet<_>>();
    let expected_scalar_ids = upstream
        .scalars()
        .iter()
        .map(|scalar| scalar.id)
        .chain([
            ids.radius,
            ids.start_angle,
            ids.end_angle,
            ids.contact_parameters[0],
            ids.contact_parameters[1],
            ids.radius_target,
        ])
        .collect::<BTreeSet<_>>();
    let actual_scalar_ids = materialized
        .scalars()
        .iter()
        .map(|scalar| scalar.id)
        .collect::<BTreeSet<_>>();
    let expected_curve_ids = upstream
        .curves()
        .iter()
        .map(|curve| curve.id)
        .chain([ids.arc])
        .collect::<BTreeSet<_>>();
    let actual_curve_ids = materialized
        .curves()
        .iter()
        .map(|curve| curve.id)
        .collect::<BTreeSet<_>>();
    let expected_contact_ids = upstream
        .contacts()
        .iter()
        .map(|contact| contact.id)
        .chain(ids.contacts)
        .collect::<BTreeSet<_>>();
    let actual_contact_ids = materialized
        .contacts()
        .iter()
        .map(|contact| contact.id)
        .collect::<BTreeSet<_>>();
    let expected_constraint_ids = upstream
        .constraints()
        .iter()
        .map(|constraint| constraint.id)
        .chain(ids.tangencies)
        .collect::<BTreeSet<_>>();
    let actual_constraint_ids = materialized
        .constraints()
        .iter()
        .map(|constraint| constraint.id)
        .collect::<BTreeSet<_>>();
    let expected_dimension_ids = upstream
        .dimensions()
        .iter()
        .map(|dimension| dimension.id)
        .chain([ids.radius_dimension])
        .collect::<BTreeSet<_>>();
    let actual_dimension_ids = materialized
        .dimensions()
        .iter()
        .map(|dimension| dimension.id)
        .collect::<BTreeSet<_>>();
    if actual_point_ids != expected_point_ids
        || actual_scalar_ids != expected_scalar_ids
        || actual_curve_ids != expected_curve_ids
        || actual_contact_ids != expected_contact_ids
        || actual_constraint_ids != expected_constraint_ids
        || actual_dimension_ids != expected_dimension_ids
    {
        return native_fillet_error(
            "native fillet continuation",
            "materialized persistent identity delta differs from the prepared action",
        );
    }

    for point in upstream
        .points()
        .iter()
        .filter(|point| point.id != ids.removed_corner)
    {
        if materialized.point(point.id) != Some(point) {
            return native_fillet_error(
                "native fillet continuation",
                "materialized action changed an unrelated upstream point",
            );
        }
    }
    for scalar in upstream.scalars() {
        if materialized.scalar(scalar.id) != Some(scalar) {
            return native_fillet_error(
                "native fillet continuation",
                "materialized action changed an unrelated upstream scalar",
            );
        }
    }
    for curve in upstream
        .curves()
        .iter()
        .filter(|curve| !ids.source_lines.contains(&curve.id))
    {
        if materialized.curve(curve.id) != Some(curve)
            || materialized.geometry_role(curve.id) != upstream.geometry_role(curve.id)
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized action changed an unrelated upstream curve",
            );
        }
    }
    if upstream.contacts() != &materialized.contacts()[..upstream.contacts().len()]
        || upstream.constraints() != &materialized.constraints()[..upstream.constraints().len()]
        || upstream.dimensions() != &materialized.dimensions()[..upstream.dimensions().len()]
        || upstream.trim_views() != materialized.trim_views()
        || upstream.parameters() != materialized.parameters()
        || upstream.parameter_bindings() != materialized.parameter_bindings()
        || upstream.parameter_outputs() != materialized.parameter_outputs()
        || upstream.external_bindings() != materialized.external_bindings()
    {
        return native_fillet_error(
            "native fillet continuation",
            "materialized action changed unrelated persistent metadata",
        );
    }

    let point_labels = [
        format!("{}.first_contact_point", request.label),
        format!("{}.second_contact_point", request.label),
        format!("{}.center", request.label),
    ];
    for (point, label) in ids
        .contact_points
        .into_iter()
        .chain([ids.center])
        .zip(point_labels)
    {
        if materialized
            .point(point)
            .is_none_or(|point| point.label != label)
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized Fillet point metadata differs from the prepared action",
            );
        }
    }
    let scalar_metadata = [
        (
            ids.radius,
            format!("{}.radius", request.label),
            ScalarUnit::Length,
            ScalarDomain::Positive,
        ),
        (
            ids.start_angle,
            format!("{}.start_angle", request.label),
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        ),
        (
            ids.end_angle,
            format!("{}.end_angle", request.label),
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        ),
        (
            ids.contact_parameters[0],
            format!("{}.arc_contact_1_parameter", request.label),
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        ),
        (
            ids.contact_parameters[1],
            format!("{}.arc_contact_2_parameter", request.label),
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        ),
        (
            ids.radius_target,
            format!("{}.radius_target", request.label),
            ScalarUnit::Length,
            ScalarDomain::Positive,
        ),
    ];
    for (id, label, unit, domain) in scalar_metadata {
        if materialized.scalar(id).is_none_or(|scalar| {
            scalar.label != label || scalar.unit != unit || scalar.domain != domain
        }) {
            return native_fillet_error(
                "native fillet continuation",
                "materialized Fillet scalar metadata differs from the prepared action",
            );
        }
    }
    if materialized
        .curve(ids.arc)
        .is_none_or(|curve| curve.label != format!("{}.arc", request.label))
    {
        return native_fillet_error(
            "native fillet continuation",
            "materialized Fillet arc metadata differs from the prepared action",
        );
    }

    let parents = [request.first, request.second];
    let arc_parameters: [f64; 2] = match request.endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => [0.0, 1.0],
        DocumentFilletEndpointOrder::SecondThenFirst => [1.0, 0.0],
    };
    for (index, parent) in parents.iter().enumerate() {
        let parameter = arc_parameters[index];
        let expected_neighborhood = if parameter == 0.0 {
            ContactNeighborhood::Start
        } else {
            ContactNeighborhood::End
        };
        if materialized
            .contact(ids.contacts[index])
            .is_none_or(|contact| {
                contact.label != format!("{}.arc_contact_{}", request.label, index + 1)
                    || contact.curve != CurveSpan::line(ids.arc)
                    || contact.parameter != ids.contact_parameters[index]
                    || contact.domain
                        != ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        }
                    || contact.winding != 0
                    || contact.neighborhood != expected_neighborhood
                    || contact.tangent_orientation != Some(prepared.tangent_orientations[index])
            })
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized Fillet contact metadata differs from the prepared action",
            );
        }
        if materialized
            .constraint(ids.tangencies[index])
            .is_none_or(|constraint| {
                constraint.label != format!("{}.tangency_{}", request.label, index + 1)
                    || constraint.suppressed
                    || constraint.definition
                        != DocumentConstraintDefinition::LineCurveTangency {
                            line: parent.curve,
                            endpoint: parent.endpoint,
                            curve_contact: ids.contacts[index],
                        }
            })
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized Fillet tangency metadata differs from the prepared action",
            );
        }
    }
    if materialized
        .dimension(ids.radius_dimension)
        .is_none_or(|dimension| {
            dimension.label != format!("{}.radius_dimension", request.label)
                || dimension.suppressed
                || dimension.mode != DocumentDimensionMode::Driving
                || dimension.definition
                    != DocumentDimensionDefinition::Radius {
                        curve: ids.arc,
                        target: ids.radius_target,
                    }
        })
    {
        return native_fillet_error(
            "native fillet continuation",
            "materialized Fillet dimension metadata differs from the prepared action",
        );
    }

    let upstream_source_count = upstream.source_order().len();
    if materialized.source_order().len() != upstream_source_count.saturating_add(3)
        || materialized.source_order()[..upstream_source_count] != *upstream.source_order()
    {
        return native_fillet_error(
            "native fillet continuation",
            "materialized Fillet source order differs from the prepared action",
        );
    }
    let source_owners = [
        crate::DocumentSourceOwner::Constraint(ids.tangencies[0]),
        crate::DocumentSourceOwner::Constraint(ids.tangencies[1]),
        crate::DocumentSourceOwner::Dimension(ids.radius_dimension),
    ];
    for (source, owner) in materialized.source_order()[upstream_source_count..]
        .iter()
        .copied()
        .zip(source_owners)
    {
        if materialized
            .source(source)
            .is_none_or(|source| source.owner != owner || source.suppressed)
        {
            return native_fillet_error(
                "native fillet continuation",
                "materialized Fillet source ownership differs from the prepared action",
            );
        }
    }
    Ok(())
}

/// A prepared native edit can be replayed against retained design coordinates that differ from
/// the accepted scene used to build it. Preserve every old retained point bit and adjust only the
/// newly allocated contact seed when the accepted contact would put a shortened line on the wrong
/// explicit tangent/line branch. The exact accepted document keeps the preview contact unchanged;
/// the retained-session publication path solves from that separately edited accepted seed.
#[allow(
    clippy::too_many_lines,
    reason = "the two-parent loop keeps accepted contact reuse and branch-valid retained contact fallback under one exact prepared-plan check"
)]
fn prepared_contact_seed_positions(
    document: &SketchDocument,
    prepared: &DocumentPreparedNativeLineFilletGeometry,
) -> Result<[[f64; 2]; 2], DocumentError> {
    let parents = [prepared.request.first, prepared.request.second];
    let requested = [
        prepared.request.first.contact_position,
        prepared.request.second.contact_position,
    ];
    let arc_parameters = match prepared.request.endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => [0.0, 1.0],
        DocumentFilletEndpointOrder::SecondThenFirst => [1.0, 0.0],
    };
    let mut result = requested;
    for index in 0..2 {
        let parent = parents[index];
        let curve = document.curve(parent.curve.curve).ok_or_else(|| {
            native_fillet_error_value("native fillet prepared source", "source is missing")
        })?;
        let CurveDefinition::Line {
            start,
            end,
            branch_direction,
        } = &curve.definition
        else {
            return native_fillet_error(
                "native fillet prepared source",
                "source family changed after preparation",
            );
        };
        let start_position = document
            .point(*start)
            .ok_or_else(|| {
                native_fillet_error_value("native fillet prepared source", "line start is missing")
            })?
            .position;
        let end_position = document
            .point(*end)
            .ok_or_else(|| {
                native_fillet_error_value("native fillet prepared source", "line end is missing")
            })?
            .position;
        let arc_tangent = arc_endpoint_tangent(
            prepared.request.start_angle,
            prepared.request.end_angle,
            prepared.request.sweep,
            arc_parameters[index],
        )?;
        let requested_endpoints = replaced_line_endpoints(
            start_position,
            end_position,
            parent.endpoint,
            requested[index],
        );
        if line_seed_matches_branches(
            requested_endpoints,
            *branch_direction,
            arc_tangent,
            prepared.tangent_orientations[index],
        ) {
            continue;
        }

        let reference = match parent.endpoint {
            FeatureEndpoint::Start => end_position,
            FeatureEndpoint::End => start_position,
        };
        let coordinate_scale = reference[0]
            .abs()
            .max(reference[1].abs())
            .max(document.model_scale().abs())
            .max(prepared.request.radius)
            .max(1.0);
        let current_length = norm(sub(end_position, start_position));
        let mut length = if current_length.is_finite() && current_length > 0.0 {
            current_length.max(coordinate_scale * 1.0e-12)
        } else {
            coordinate_scale * 1.0e-6
        };
        let accepted_tangent = prepared.accepted_line_tangents[index];
        let mut replacement = None;
        for _ in 0..128 {
            let displacement = scale(accepted_tangent, length);
            let contact = match parent.endpoint {
                FeatureEndpoint::Start => sub(reference, displacement),
                FeatureEndpoint::End => add(reference, displacement),
            };
            let endpoints =
                replaced_line_endpoints(start_position, end_position, parent.endpoint, contact);
            if contact.into_iter().all(f64::is_finite)
                && contact.map(f64::to_bits) != reference.map(f64::to_bits)
                && line_seed_matches_branches(
                    endpoints,
                    *branch_direction,
                    arc_tangent,
                    prepared.tangent_orientations[index],
                )
            {
                replacement = Some(contact);
                break;
            }
            length *= 0.5;
        }
        result[index] = replacement.ok_or_else(|| {
            native_fillet_error_value(
                "native fillet retained seed",
                "cannot represent a branch-compatible new contact without changing source points",
            )
        })?;
    }
    Ok(result)
}

const fn replaced_line_endpoints(
    start: [f64; 2],
    end: [f64; 2],
    endpoint: FeatureEndpoint,
    replacement: [f64; 2],
) -> [[f64; 2]; 2] {
    match endpoint {
        FeatureEndpoint::Start => [replacement, end],
        FeatureEndpoint::End => [start, replacement],
    }
}

fn line_seed_matches_branches(
    endpoints: [[f64; 2]; 2],
    branch_direction: [f64; 2],
    arc_tangent: [f64; 2],
    orientation: TangentOrientation,
) -> bool {
    let Ok(line_tangent) = normalized(sub(endpoints[1], endpoints[0]), "native fillet line") else {
        return false;
    };
    if dot(line_tangent, branch_direction) <= 0.0 {
        return false;
    }
    let product = dot(line_tangent, arc_tangent);
    match orientation {
        TangentOrientation::Aligned => product > 0.0,
        TangentOrientation::Opposed => product < 0.0,
    }
}

fn validate_arc_geometry(
    request: &DocumentNativeLineFilletCreationRequest,
    model_scale: f64,
) -> Result<(), DocumentError> {
    let expected = match request.endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => [
            request.first.contact_position,
            request.second.contact_position,
        ],
        DocumentFilletEndpointOrder::SecondThenFirst => [
            request.second.contact_position,
            request.first.contact_position,
        ],
    };
    let actual = [
        point_on_circle(request.center, request.radius, request.start_angle),
        point_on_circle(request.center, request.radius, request.end_angle),
    ];
    let tolerance = geometry_tolerance(model_scale, request.radius);
    if actual
        .iter()
        .zip(expected)
        .any(|(actual, expected)| norm(sub(*actual, expected)) > tolerance)
    {
        return native_fillet_error(
            "native fillet arc",
            "arc endpoints do not match the selected line contacts",
        );
    }
    let sweep = match request.sweep {
        DocumentArcSweep::CounterClockwise => {
            (request.end_angle - request.start_angle).rem_euclid(std::f64::consts::TAU)
        }
        DocumentArcSweep::Clockwise => {
            (request.start_angle - request.end_angle).rem_euclid(std::f64::consts::TAU)
        }
    };
    if !sweep.is_finite() || sweep <= 1.0e-10 || sweep >= std::f64::consts::TAU - 1.0e-10 {
        return native_fillet_error("native fillet arc", "arc sweep is degenerate or full-turn");
    }
    Ok(())
}

fn validate_strict_line_contact(
    start: [f64; 2],
    end: [f64; 2],
    contact: [f64; 2],
    model_scale: f64,
) -> Result<(), DocumentError> {
    let delta = sub(end, start);
    let length_squared = dot(delta, delta);
    if !length_squared.is_finite() || length_squared <= 0.0 {
        return native_fillet_error("native fillet parent", "source line is degenerate");
    }
    let parameter = dot(sub(contact, start), delta) / length_squared;
    let projected = add(start, scale(delta, parameter));
    let tolerance = geometry_tolerance(model_scale, length_squared.sqrt());
    if !parameter.is_finite()
        || parameter <= 0.0
        || parameter >= 1.0
        || norm(sub(projected, contact)) > tolerance
    {
        return native_fillet_error(
            "native fillet contact",
            "contact must lie strictly inside its complete source line",
        );
    }
    Ok(())
}

fn validate_sided_center(
    contact: [f64; 2],
    tangent: [f64; 2],
    side: DocumentCurveNormalSide,
    center: [f64; 2],
    radius: f64,
    model_scale: f64,
) -> Result<(), DocumentError> {
    let sign = match side {
        DocumentCurveNormalSide::Left => 1.0,
        DocumentCurveNormalSide::Right => -1.0,
    };
    let expected = add(contact, scale([-tangent[1], tangent[0]], sign * radius));
    if norm(sub(expected, center)) > geometry_tolerance(model_scale, radius) {
        return native_fillet_error(
            "native fillet branch",
            "center disagrees with the explicit parent normal side",
        );
    }
    Ok(())
}

fn arc_endpoint_tangent(
    start_angle: f64,
    end_angle: f64,
    sweep: DocumentArcSweep,
    parameter: f64,
) -> Result<[f64; 2], DocumentError> {
    let angle = if parameter.to_bits() == 0.0_f64.to_bits() {
        start_angle
    } else if parameter.to_bits() == 1.0_f64.to_bits() {
        end_angle
    } else {
        return native_fillet_error(
            "native fillet arc",
            "endpoint parameter must be zero or one",
        );
    };
    let ccw = [-angle.sin(), angle.cos()];
    Ok(match sweep {
        DocumentArcSweep::CounterClockwise => ccw,
        DocumentArcSweep::Clockwise => [-ccw[0], -ccw[1]],
    })
}

fn point_on_circle(center: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

fn normalized(vector: [f64; 2], field: &'static str) -> Result<[f64; 2], DocumentError> {
    let length = norm(vector);
    if !length.is_finite() || length <= 0.0 {
        return native_fillet_error(field, "direction must be finite and nonzero");
    }
    Ok([vector[0] / length, vector[1] / length])
}

fn geometry_tolerance(model_scale: f64, magnitude: f64) -> f64 {
    (model_scale.abs().max(magnitude.abs()) * NATIVE_FILLET_GEOMETRY_EPSILON).max(1.0e-10)
}

const fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

const fn sub(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

const fn scale(vector: [f64; 2], factor: f64) -> [f64; 2] {
    [vector[0] * factor, vector[1] * factor]
}

const fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

fn norm(vector: [f64; 2]) -> f64 {
    vector[0].hypot(vector[1])
}

fn validate_finite(value: f64, field: &'static str) -> Result<(), DocumentError> {
    if value.is_finite() {
        Ok(())
    } else {
        native_fillet_error(field, "must be finite")
    }
}

fn validate_finite_pair(value: [f64; 2], field: &'static str) -> Result<(), DocumentError> {
    if value.into_iter().all(f64::is_finite) {
        Ok(())
    } else {
        native_fillet_error(field, "both coordinates must be finite")
    }
}

fn validate_finite_positive(value: f64, field: &'static str) -> Result<(), DocumentError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        native_fillet_error(field, "must be finite and positive")
    }
}

fn native_fillet_error<T>(field: &'static str, message: &'static str) -> Result<T, DocumentError> {
    Err(native_fillet_error_value(field, message))
}

fn native_fillet_error_value(field: &'static str, message: &'static str) -> DocumentError {
    DocumentError::InvalidField {
        field,
        message: message.into(),
    }
}
