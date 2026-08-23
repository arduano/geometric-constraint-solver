// SPDX-License-Identifier: GPL-3.0-or-later

//! Honest per-object normalization of already-materialized flat sketches.
//!
//! This adapter records only native objects which actually exist. It does not
//! infer rectangles, Fillets, offsets, or any other fictional authoring recipe
//! from a flat document.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ActivationDigest, ContactSlot, DesignCurve, DesignPoint, DesignScalar, DocumentConstraint,
    DocumentCurveTrimView, DocumentDimension, DocumentElementId, DocumentError,
    DocumentExternalBinding, DocumentParameter, DocumentParameterBinding, DocumentParameterOutput,
    DocumentParameterTarget, DocumentSourceId, DocumentSourceOwner, DocumentTrimBoundary,
    GeometryRole, GeometryRoleEdit, HostActivationOverride, HostConfigurationActivation,
    SketchDocument, SketchObjectBootstrap, SketchPersistentIdentityHighWater,
};
use geosolve_sketch_features::{
    ComputedFeature, ComputedFeatureAllocatorHighWater, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentId, ComputedFeatureLifecycleHighWater,
    ComputedFeatureObjectBootstrap, ComputedFeatureRevision,
};
use geosolve_sketch_intent::{
    BootstrapNativeKind, InputRole, InputSlot, IntentBootstrapObject, IntentEvaluation, IntentKey,
    IntentKeyError, IntentModelError, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPlanError, IntentPortRole, IntentPortSelector,
    IntentSession, IntentSessionError, IntentSessionId, MaterializationEvidence, PatchPortRef,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub const BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1: &str = "geosolve-bootstrap-document-header-v1";
pub const BOOTSTRAP_POINT_CODEC_V1: &str = "geosolve-bootstrap-point-v1";
pub const BOOTSTRAP_SCALAR_CODEC_V1: &str = "geosolve-bootstrap-scalar-v1";
pub const BOOTSTRAP_CURVE_CODEC_V1: &str = "geosolve-bootstrap-curve-v1";
pub const BOOTSTRAP_CONTACT_CODEC_V1: &str = "geosolve-bootstrap-contact-v1";
pub const BOOTSTRAP_CONSTRAINT_CODEC_V1: &str = "geosolve-bootstrap-constraint-v1";
pub const BOOTSTRAP_DIMENSION_CODEC_V1: &str = "geosolve-bootstrap-dimension-v1";
pub const BOOTSTRAP_PARAMETER_CODEC_V1: &str = "geosolve-bootstrap-parameter-v1";
pub const BOOTSTRAP_EXTERNAL_BINDING_CODEC_V1: &str = "geosolve-bootstrap-external-binding-v1";
pub const BOOTSTRAP_TRIM_VIEW_CODEC_V1: &str = "geosolve-bootstrap-trim-view-v1";
pub const BOOTSTRAP_GEOMETRY_ROLE_CODEC_V1: &str = "geosolve-bootstrap-geometry-role-v1";
pub const BOOTSTRAP_PARAMETER_BINDING_CODEC_V1: &str = "geosolve-bootstrap-parameter-binding-v1";
pub const BOOTSTRAP_PARAMETER_OUTPUT_CODEC_V1: &str = "geosolve-bootstrap-parameter-output-v1";
pub const BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1: &str = "geosolve-bootstrap-computed-feature-v1";
pub const BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1: &str = "geosolve-bootstrap-source-order-entry-v1";
pub const BOOTSTRAP_USER_INACTIVE_ENTRY_CODEC_V1: &str =
    "geosolve-bootstrap-user-inactive-entry-v1";
pub const BOOTSTRAP_HOST_ACTIVATION_HEADER_CODEC_V1: &str =
    "geosolve-bootstrap-host-activation-header-v1";
pub const BOOTSTRAP_HOST_ACTIVATION_OVERRIDE_CODEC_V1: &str =
    "geosolve-bootstrap-host-activation-override-v1";
pub const BOOTSTRAP_SEMANTIC_CATALOG_CODEC_V1: &str = "geosolve-bootstrap-semantic-catalog-v1";
pub const BOOTSTRAP_SEMANTIC_SOURCE_CODEC_V1: &str = "geosolve-bootstrap-semantic-source-v1";

const ROOT_ALIAS: &str = "legacy-document";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapDocumentHeaderV1 {
    document: geosolve_sketch::DocumentId,
    model_scale: f64,
    sketch_high_water: SketchPersistentIdentityHighWater,
    feature_document: ComputedFeatureDocumentId,
    feature_revision: ComputedFeatureRevision,
    feature_allocator: ComputedFeatureAllocatorHighWater,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapSourceOrderEntryV1 {
    index: u32,
    source: DocumentSourceId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapUserInactiveEntryV1 {
    element: DocumentElementId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapHostActivationHeaderV1 {
    revision: u64,
    digest: ActivationDigest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapHostActivationOverrideV1 {
    index: u32,
    value: HostActivationOverride,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapSemanticReservationV1 {
    source: DocumentSourceId,
    catalog: DocumentSourceId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapGeometryRoleV1 {
    curve: geosolve_sketch::CurveId,
    role: GeometryRole,
}

/// Exact decoded result of one per-object bootstrap graph.
#[derive(Clone, Debug)]
pub struct DecodedFlatIntentBootstrap {
    pub document: SketchDocument,
    pub features: ComputedFeatureDocument,
    pub feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
}

/// Typed normalization or exact-codec failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IntentBootstrapError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Feature(#[from] ComputedFeatureDocumentError),
    #[error(transparent)]
    IntentModel(#[from] IntentModelError),
    #[error(transparent)]
    IntentKey(#[from] IntentKeyError),
    #[error(transparent)]
    Plan(#[from] IntentPlanError),
    #[error(transparent)]
    Session(#[from] IntentSessionError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("computed-feature sidecar belongs to a different sketch document")]
    ForeignFeatureDocument,
    #[error("bootstrap input exceeds a stable index or patch resource limit")]
    ResourceLimit,
    #[error("bootstrap graph is missing or duplicates its exact document header")]
    InvalidDocumentHeader,
    #[error("bootstrap node kind and exact payload codec disagree")]
    CodecKindMismatch,
    #[error("bootstrap payload is malformed, unsupported, or non-canonical")]
    InvalidPayload,
    #[error("bootstrap graph contains a non-bootstrap declaration")]
    MixedDeclarationGraph,
    #[error("bootstrap graph contains duplicate side-table state")]
    DuplicateSideTableState,
    #[error("bootstrap dependency references an unknown native source")]
    UnknownDependency,
    #[error("bootstrap session differs from canonical per-object state")]
    NonCanonicalBootstrap,
}

/// Normalizes one strict flat sketch plus its exact computed-feature sidecar
/// into an accepted, history-free projectional session.
///
/// Every persistent object receives one typed `Bootstrap` declaration. Native
/// direct dependencies are retained as typed graph edges, every declaration
/// belongs to one document-header root, and side-table entries remain explicit
/// per-entry records. No higher-level recipe is inferred.
///
/// # Errors
///
/// Returns a typed document, feature, payload, graph, resource, planning, or
/// session error without publishing a partial intent session.
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive native-object normalization inventory is clearest in one pass"
)]
pub fn normalize_flat_sketch_intent(
    session_id: IntentSessionId,
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
) -> Result<IntentSession, IntentBootstrapError> {
    document.validate()?;
    validate_feature_bundle(document, features, feature_lifecycle_high_water)?;

    let header = BootstrapDocumentHeaderV1 {
        document: document.id(),
        model_scale: document.model_scale(),
        sketch_high_water: document.persistent_identity_high_water(),
        feature_document: features.id(),
        feature_revision: features.revision(),
        feature_allocator: features.allocator_high_water(),
        feature_lifecycle_high_water,
    };
    let mut operations = Vec::new();
    operations.push(create_operation(
        ROOT_ALIAS,
        BootstrapNativeKind::Document,
        BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1,
        &header,
        Vec::new(),
    )?);

    for point in document.points() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Point(point.id),
            BootstrapNativeKind::Point,
            BOOTSTRAP_POINT_CODEC_V1,
            point,
        )?);
    }
    for scalar in document.scalars() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Scalar(scalar.id),
            BootstrapNativeKind::Scalar,
            BOOTSTRAP_SCALAR_CODEC_V1,
            scalar,
        )?);
    }
    for curve in document.curves() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Curve(curve.id),
            BootstrapNativeKind::Curve,
            BOOTSTRAP_CURVE_CODEC_V1,
            curve,
        )?);
    }
    for contact in document.contacts() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Contact(contact.id),
            BootstrapNativeKind::Contact,
            BOOTSTRAP_CONTACT_CODEC_V1,
            contact,
        )?);
    }
    for constraint in document.constraints() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Constraint(constraint.id),
            BootstrapNativeKind::Constraint,
            BOOTSTRAP_CONSTRAINT_CODEC_V1,
            constraint,
        )?);
    }
    for dimension in document.dimensions() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Dimension(dimension.id),
            BootstrapNativeKind::Dimension,
            BOOTSTRAP_DIMENSION_CODEC_V1,
            dimension,
        )?);
    }
    for parameter in document.parameters() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::Parameter(parameter.id),
            BootstrapNativeKind::Parameter,
            BOOTSTRAP_PARAMETER_CODEC_V1,
            parameter,
        )?);
    }
    for binding in document.external_bindings() {
        operations.push(native_element_operation(
            document,
            DocumentElementId::ExternalBinding(binding.id),
            BootstrapNativeKind::ExternalBinding,
            BOOTSTRAP_EXTERNAL_BINDING_CODEC_V1,
            binding,
        )?);
    }

    for (index, view) in document.trim_views().iter().enumerate() {
        let index = stable_index(index)?;
        let mut dependencies = vec![DocumentElementId::Document(document.id())];
        dependencies.push(DocumentElementId::Curve(view.support.curve));
        trim_boundary_dependencies(view.start, &mut dependencies);
        trim_boundary_dependencies(view.end, &mut dependencies);
        operations.push(create_operation(
            &format!(
                "legacy-trim-{}-{:08x}-{:08x}",
                view.support.curve, view.support.segment, index
            ),
            BootstrapNativeKind::CurveTrimView,
            BOOTSTRAP_TRIM_VIEW_CODEC_V1,
            view,
            element_inputs(document, dependencies)?,
        )?);
    }
    for role in document.explicit_geometry_roles() {
        let payload = BootstrapGeometryRoleV1 {
            curve: role.curve,
            role: role.role,
        };
        operations.push(create_operation(
            &format!("legacy-role-{}", role.curve),
            BootstrapNativeKind::GeometryRole,
            BOOTSTRAP_GEOMETRY_ROLE_CODEC_V1,
            &payload,
            element_inputs(
                document,
                [
                    DocumentElementId::Document(document.id()),
                    DocumentElementId::Curve(role.curve),
                ],
            )?,
        )?);
    }
    for (index, binding) in document.parameter_bindings().iter().enumerate() {
        let mut dependencies = vec![
            DocumentElementId::Document(document.id()),
            DocumentElementId::Parameter(binding.parameter),
        ];
        dependencies.push(parameter_target_element(binding.target));
        operations.push(create_operation(
            &format!("legacy-parameter-binding-{:08x}", stable_index(index)?),
            BootstrapNativeKind::ParameterBinding,
            BOOTSTRAP_PARAMETER_BINDING_CODEC_V1,
            binding,
            element_inputs(document, dependencies)?,
        )?);
    }
    for output in document.parameter_outputs() {
        operations.push(create_operation(
            &format!(
                "legacy-parameter-output-{}-{}",
                output.parameter, output.dimension
            ),
            BootstrapNativeKind::ParameterOutput,
            BOOTSTRAP_PARAMETER_OUTPUT_CODEC_V1,
            output,
            element_inputs(
                document,
                [
                    DocumentElementId::Document(document.id()),
                    DocumentElementId::Parameter(output.parameter),
                    DocumentElementId::Dimension(output.dimension),
                ],
            )?,
        )?);
    }
    append_document_side_tables(document, &mut operations)?;
    append_semantic_reservations(document, &mut operations)?;
    append_computed_features(document, features, &mut operations)?;

    let materialization = document.to_draft_v5_json()?.into_bytes();
    let feature_artifact = features.to_json()?.into_bytes();
    let mut session = IntentSession::with_id(session_id)?;
    let evidence = MaterializationEvidence::new_host_artifacts(
        session.external_inputs().identity(),
        materialization,
        b"geosolve-bootstrap-per-object-v1".to_vec(),
        feature_artifact,
    )?;
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let plan = session.plan_patch(patch, |_| IntentEvaluation::Accepted { evidence })?;
    session.commit_initialization_plan(plan)?;
    Ok(session)
}

/// Strictly reconstructs the flat native sketch represented by one honest
/// per-object bootstrap graph.
///
/// Payload codecs are closed and versioned. Each payload must use canonical
/// JSON bytes, every side-table row must occur exactly once, and the complete
/// dependency graph must equal the graph that normalization would produce for
/// the reconstructed native state. This deliberately rejects mixed graphs and
/// never guesses a higher-level authoring recipe.
///
/// # Errors
///
/// Returns a typed codec, canonicality, graph, document, feature, resource, or
/// reconstruction error. The supplied session is never mutated.
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive strict-codec dispatch keeps the migration boundary auditable"
)]
pub fn decode_flat_intent_bootstrap(
    session: &IntentSession,
) -> Result<DecodedFlatIntentBootstrap, IntentBootstrapError> {
    let mut header = None;
    let mut points = Vec::<DesignPoint>::new();
    let mut scalars = Vec::<DesignScalar>::new();
    let mut curves = Vec::<DesignCurve>::new();
    let mut contacts = Vec::<ContactSlot>::new();
    let mut constraints = Vec::<DocumentConstraint>::new();
    let mut dimensions = Vec::<DocumentDimension>::new();
    let mut parameters = Vec::<DocumentParameter>::new();
    let mut external_bindings = Vec::<DocumentExternalBinding>::new();
    let mut trim_views = Vec::<DocumentCurveTrimView>::new();
    let mut parameter_bindings = Vec::<DocumentParameterBinding>::new();
    let mut parameter_outputs = Vec::<DocumentParameterOutput>::new();
    let mut computed_features = Vec::<ComputedFeature>::new();
    let mut source_order = BTreeMap::<u32, DocumentSourceId>::new();
    let mut user_inactive = BTreeSet::<DocumentElementId>::new();
    let mut geometry_roles = BTreeMap::<geosolve_sketch::CurveId, GeometryRole>::new();
    let mut activation_header = None;
    let mut activation_overrides = BTreeMap::<u32, HostActivationOverride>::new();
    let mut semantic_reservations = BTreeMap::<DocumentSourceId, DocumentSourceId>::new();

    for node in session.graph().nodes().values() {
        let IntentNodeKind::Bootstrap { object } = &node.kind else {
            return Err(IntentBootstrapError::MixedDeclarationGraph);
        };
        let codec = object.codec.as_str();
        let Some(expected_kind) = bootstrap_codec_kind(codec) else {
            return Err(IntentBootstrapError::InvalidPayload);
        };
        if object.kind != expected_kind {
            return Err(IntentBootstrapError::CodecKindMismatch);
        }

        match codec {
            BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1 => {
                let value = strict_payload::<BootstrapDocumentHeaderV1>(&object.payload)?;
                if header.replace(value).is_some() {
                    return Err(IntentBootstrapError::InvalidDocumentHeader);
                }
            }
            BOOTSTRAP_POINT_CODEC_V1 => {
                points.push(strict_payload::<DesignPoint>(&object.payload)?);
            }
            BOOTSTRAP_SCALAR_CODEC_V1 => {
                scalars.push(strict_payload::<DesignScalar>(&object.payload)?);
            }
            BOOTSTRAP_CURVE_CODEC_V1 => {
                curves.push(strict_payload::<DesignCurve>(&object.payload)?);
            }
            BOOTSTRAP_CONTACT_CODEC_V1 => {
                contacts.push(strict_payload::<ContactSlot>(&object.payload)?);
            }
            BOOTSTRAP_CONSTRAINT_CODEC_V1 => {
                constraints.push(strict_payload::<DocumentConstraint>(&object.payload)?);
            }
            BOOTSTRAP_DIMENSION_CODEC_V1 => {
                dimensions.push(strict_payload::<DocumentDimension>(&object.payload)?);
            }
            BOOTSTRAP_PARAMETER_CODEC_V1 => {
                parameters.push(strict_payload::<DocumentParameter>(&object.payload)?);
            }
            BOOTSTRAP_EXTERNAL_BINDING_CODEC_V1 => {
                external_bindings.push(strict_payload::<DocumentExternalBinding>(&object.payload)?);
            }
            BOOTSTRAP_TRIM_VIEW_CODEC_V1 => {
                trim_views.push(strict_payload::<DocumentCurveTrimView>(&object.payload)?);
            }
            BOOTSTRAP_GEOMETRY_ROLE_CODEC_V1 => {
                let value = strict_payload::<BootstrapGeometryRoleV1>(&object.payload)?;
                if geometry_roles.insert(value.curve, value.role).is_some() {
                    return Err(IntentBootstrapError::DuplicateSideTableState);
                }
            }
            BOOTSTRAP_PARAMETER_BINDING_CODEC_V1 => {
                parameter_bindings
                    .push(strict_payload::<DocumentParameterBinding>(&object.payload)?);
            }
            BOOTSTRAP_PARAMETER_OUTPUT_CODEC_V1 => {
                parameter_outputs.push(strict_payload::<DocumentParameterOutput>(&object.payload)?);
            }
            BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1 => {
                computed_features.push(strict_payload::<ComputedFeature>(&object.payload)?);
            }
            BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1 => {
                let value = strict_payload::<BootstrapSourceOrderEntryV1>(&object.payload)?;
                insert_side_table(&mut source_order, value.index, value.source)?;
            }
            BOOTSTRAP_USER_INACTIVE_ENTRY_CODEC_V1 => {
                let value = strict_payload::<BootstrapUserInactiveEntryV1>(&object.payload)?;
                if !user_inactive.insert(value.element) {
                    return Err(IntentBootstrapError::DuplicateSideTableState);
                }
            }
            BOOTSTRAP_HOST_ACTIVATION_HEADER_CODEC_V1 => {
                let value = strict_payload::<BootstrapHostActivationHeaderV1>(&object.payload)?;
                if activation_header.replace(value).is_some() {
                    return Err(IntentBootstrapError::DuplicateSideTableState);
                }
            }
            BOOTSTRAP_HOST_ACTIVATION_OVERRIDE_CODEC_V1 => {
                let value = strict_payload::<BootstrapHostActivationOverrideV1>(&object.payload)?;
                insert_side_table(&mut activation_overrides, value.index, value.value)?;
            }
            BOOTSTRAP_SEMANTIC_CATALOG_CODEC_V1 | BOOTSTRAP_SEMANTIC_SOURCE_CODEC_V1 => {
                let value = strict_payload::<BootstrapSemanticReservationV1>(&object.payload)?;
                let is_catalog = codec == BOOTSTRAP_SEMANTIC_CATALOG_CODEC_V1;
                if is_catalog != (value.source == value.catalog)
                    || semantic_reservations
                        .insert(value.source, value.catalog)
                        .is_some()
                {
                    return Err(IntentBootstrapError::DuplicateSideTableState);
                }
            }
            _ => return Err(IntentBootstrapError::InvalidPayload),
        }
    }

    let header = header.ok_or(IntentBootstrapError::InvalidDocumentHeader)?;
    let mut document = SketchObjectBootstrap::new(header.model_scale, header.sketch_high_water)?;
    for value in points {
        document.push_point(value);
    }
    for value in scalars {
        document.push_scalar(value);
    }
    for value in curves {
        document.push_curve(value);
    }
    for value in contacts {
        document.push_contact(value);
    }
    for value in trim_views {
        document.push_trim_view(value);
    }
    for value in constraints {
        document.push_constraint(value);
    }
    for value in dimensions {
        document.push_dimension(value);
    }
    for value in parameters {
        document.push_parameter(value);
    }
    for value in parameter_bindings {
        document.push_parameter_binding(value);
    }
    for value in parameter_outputs {
        document.push_parameter_output(value);
    }
    for value in external_bindings {
        document.push_external_binding(value);
    }
    for value in contiguous_side_table(source_order)? {
        document.push_source(value);
    }
    for (curve, role) in geometry_roles {
        document.push_geometry_role(GeometryRoleEdit::new(curve, role));
    }
    for value in user_inactive {
        document.push_user_inactive_element(value);
    }
    match (activation_header, activation_overrides.is_empty()) {
        (Some(header), _) => {
            document.set_host_activation(HostConfigurationActivation::from_digest(
                header.revision,
                header.digest,
                contiguous_side_table(activation_overrides)?,
            )?);
        }
        (None, true) => {}
        (None, false) => return Err(IntentBootstrapError::DuplicateSideTableState),
    }
    document.set_semantic_source_reservations(semantic_reservations);
    let document = document.finish()?;
    if document.id() != header.document {
        return Err(IntentBootstrapError::InvalidDocumentHeader);
    }

    let mut features = ComputedFeatureObjectBootstrap::new(
        header.document,
        header.feature_document,
        header.feature_revision,
        header.feature_allocator,
        header.feature_lifecycle_high_water,
    )?;
    for feature in computed_features {
        features.push_feature(feature);
    }
    let features = features.finish()?;
    validate_feature_bundle(&document, &features, header.feature_lifecycle_high_water)?;

    let canonical = normalize_flat_sketch_intent(
        session.id(),
        &document,
        &features,
        header.feature_lifecycle_high_water,
    )?;
    if canonical.graph() != session.graph()
        || canonical.instance() != session.instance()
        || canonical.reservations() != session.reservations()
        || canonical.external_inputs() != session.external_inputs()
        || canonical.latest_attempt() != session.latest_attempt()
        || canonical.accepted() != session.accepted()
    {
        return Err(IntentBootstrapError::NonCanonicalBootstrap);
    }

    Ok(DecodedFlatIntentBootstrap {
        document,
        features,
        feature_lifecycle_high_water: header.feature_lifecycle_high_water,
    })
}

fn bootstrap_codec_kind(codec: &str) -> Option<BootstrapNativeKind> {
    Some(match codec {
        BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1
        | BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1
        | BOOTSTRAP_USER_INACTIVE_ENTRY_CODEC_V1
        | BOOTSTRAP_HOST_ACTIVATION_HEADER_CODEC_V1
        | BOOTSTRAP_HOST_ACTIVATION_OVERRIDE_CODEC_V1 => BootstrapNativeKind::Document,
        BOOTSTRAP_POINT_CODEC_V1 => BootstrapNativeKind::Point,
        BOOTSTRAP_SCALAR_CODEC_V1 => BootstrapNativeKind::Scalar,
        BOOTSTRAP_CURVE_CODEC_V1 => BootstrapNativeKind::Curve,
        BOOTSTRAP_CONTACT_CODEC_V1 => BootstrapNativeKind::Contact,
        BOOTSTRAP_CONSTRAINT_CODEC_V1 => BootstrapNativeKind::Constraint,
        BOOTSTRAP_DIMENSION_CODEC_V1 => BootstrapNativeKind::Dimension,
        BOOTSTRAP_PARAMETER_CODEC_V1 => BootstrapNativeKind::Parameter,
        BOOTSTRAP_EXTERNAL_BINDING_CODEC_V1 => BootstrapNativeKind::ExternalBinding,
        BOOTSTRAP_TRIM_VIEW_CODEC_V1 => BootstrapNativeKind::CurveTrimView,
        BOOTSTRAP_GEOMETRY_ROLE_CODEC_V1 => BootstrapNativeKind::GeometryRole,
        BOOTSTRAP_PARAMETER_BINDING_CODEC_V1 => BootstrapNativeKind::ParameterBinding,
        BOOTSTRAP_PARAMETER_OUTPUT_CODEC_V1 => BootstrapNativeKind::ParameterOutput,
        BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1 => BootstrapNativeKind::ComputedFeature,
        BOOTSTRAP_SEMANTIC_CATALOG_CODEC_V1 => BootstrapNativeKind::SemanticCatalog,
        BOOTSTRAP_SEMANTIC_SOURCE_CODEC_V1 => BootstrapNativeKind::SemanticSource,
        _ => return None,
    })
}

fn insert_side_table<T>(
    table: &mut BTreeMap<u32, T>,
    index: u32,
    value: T,
) -> Result<(), IntentBootstrapError> {
    if table.insert(index, value).is_some() {
        return Err(IntentBootstrapError::DuplicateSideTableState);
    }
    Ok(())
}

fn contiguous_side_table<T>(table: BTreeMap<u32, T>) -> Result<Vec<T>, IntentBootstrapError> {
    table
        .into_iter()
        .enumerate()
        .map(|(expected, (actual, value))| {
            if stable_index(expected)? == actual {
                Ok(value)
            } else {
                Err(IntentBootstrapError::DuplicateSideTableState)
            }
        })
        .collect()
}

fn validate_feature_bundle(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    lifecycle: ComputedFeatureLifecycleHighWater,
) -> Result<(), IntentBootstrapError> {
    if features.sketch_document() != document.id() {
        return Err(IntentBootstrapError::ForeignFeatureDocument);
    }
    let mut bootstrap = ComputedFeatureObjectBootstrap::new(
        features.sketch_document(),
        features.id(),
        features.revision(),
        features.allocator_high_water(),
        lifecycle,
    )?;
    for feature in features.features() {
        bootstrap.push_feature(feature.clone());
    }
    if bootstrap.finish()? != *features {
        return Err(IntentBootstrapError::InvalidPayload);
    }
    Ok(())
}

fn native_element_operation<T: Serialize>(
    document: &SketchDocument,
    element: DocumentElementId,
    kind: BootstrapNativeKind,
    codec: &'static str,
    payload: &T,
) -> Result<IntentPatchOperation, IntentBootstrapError> {
    create_operation(
        &element_alias(document, element)?,
        kind,
        codec,
        payload,
        element_inputs(document, document.direct_dependencies(element))?,
    )
}

fn create_operation<T: Serialize>(
    alias: &str,
    kind: BootstrapNativeKind,
    codec: &'static str,
    payload: &T,
    dependencies: Vec<(InputRole, PatchPortRef)>,
) -> Result<IntentPatchOperation, IntentBootstrapError> {
    let alias = IntentKey::new(alias)?;
    let object =
        IntentBootstrapObject::new(kind, IntentKey::new(codec)?, canonical_payload(payload)?)?;
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Bootstrap { object }, alias.clone());
    let mut next_index = BTreeMap::<InputRole, u16>::new();
    for (role, source) in dependencies {
        let index = next_index.entry(role).or_default();
        draft = draft.with_input(InputSlot::new(role, *index), source);
        *index = index
            .checked_add(1)
            .ok_or(IntentBootstrapError::ResourceLimit)?;
    }
    Ok(IntentPatchOperation::CreateNode {
        alias,
        draft: Box::new(draft),
        cell: None,
    })
}

fn canonical_payload<T: Serialize>(value: &T) -> Result<Vec<u8>, IntentBootstrapError> {
    Ok(serde_json::to_vec(value)?)
}

fn strict_payload<T: DeserializeOwned + Serialize>(
    payload: &[u8],
) -> Result<T, IntentBootstrapError> {
    let value =
        serde_json::from_slice::<T>(payload).map_err(|_| IntentBootstrapError::InvalidPayload)?;
    if serde_json::to_vec(&value).map_err(|_| IntentBootstrapError::InvalidPayload)? != payload {
        return Err(IntentBootstrapError::InvalidPayload);
    }
    Ok(value)
}

fn stable_index(index: usize) -> Result<u32, IntentBootstrapError> {
    u32::try_from(index).map_err(|_| IntentBootstrapError::ResourceLimit)
}

fn element_inputs(
    document: &SketchDocument,
    dependencies: impl IntoIterator<Item = DocumentElementId>,
) -> Result<Vec<(InputRole, PatchPortRef)>, IntentBootstrapError> {
    dependencies
        .into_iter()
        .map(|element| element_input(document, element))
        .collect()
}

fn element_input(
    document: &SketchDocument,
    element: DocumentElementId,
) -> Result<(InputRole, PatchPortRef), IntentBootstrapError> {
    let (role, selector) = match element {
        DocumentElementId::Document(_) => (
            InputRole::Identity,
            IntentPortSelector::Node {
                role: IntentPortRole::Result,
                index: 0,
            },
        ),
        DocumentElementId::Point(_) => (
            InputRole::Point,
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
        ),
        DocumentElementId::Scalar(_) => (
            InputRole::Scalar,
            IntentPortSelector::Node {
                role: IntentPortRole::Target,
                index: 0,
            },
        ),
        DocumentElementId::Curve(_) => (
            InputRole::Curve,
            IntentPortSelector::Node {
                role: IntentPortRole::Curve,
                index: 0,
            },
        ),
        DocumentElementId::Contact(_) => (
            InputRole::Contact,
            IntentPortSelector::Node {
                role: IntentPortRole::Contact,
                index: 0,
            },
        ),
        DocumentElementId::Constraint(_) => (
            InputRole::Constraint,
            IntentPortSelector::Node {
                role: IntentPortRole::Constraint,
                index: 0,
            },
        ),
        DocumentElementId::Dimension(_) => (
            InputRole::Dimension,
            IntentPortSelector::Node {
                role: IntentPortRole::Dimension,
                index: 0,
            },
        ),
        DocumentElementId::Parameter(_) => (
            InputRole::Parameter,
            IntentPortSelector::Node {
                role: IntentPortRole::Parameter,
                index: 0,
            },
        ),
        DocumentElementId::ExternalBinding(_) => (
            InputRole::External,
            IntentPortSelector::Node {
                role: IntentPortRole::External,
                index: 0,
            },
        ),
        DocumentElementId::Source(_) => (
            InputRole::Source,
            IntentPortSelector::Node {
                role: IntentPortRole::Source,
                index: 0,
            },
        ),
        _ => return Err(IntentBootstrapError::UnknownDependency),
    };
    Ok((
        role,
        PatchPortRef::Alias {
            node: IntentKey::new(element_alias(document, element)?)?,
            selector,
        },
    ))
}

fn element_alias(
    document: &SketchDocument,
    element: DocumentElementId,
) -> Result<String, IntentBootstrapError> {
    Ok(match element {
        DocumentElementId::Document(id) if id == document.id() => ROOT_ALIAS.to_owned(),
        DocumentElementId::Point(id) => format!("legacy-point-{id}"),
        DocumentElementId::Scalar(id) => format!("legacy-scalar-{id}"),
        DocumentElementId::Curve(id) => format!("legacy-curve-{id}"),
        DocumentElementId::Contact(id) => format!("legacy-contact-{id}"),
        DocumentElementId::Constraint(id) => format!("legacy-constraint-{id}"),
        DocumentElementId::Dimension(id) => format!("legacy-dimension-{id}"),
        DocumentElementId::Parameter(id) => format!("legacy-parameter-{id}"),
        DocumentElementId::ExternalBinding(id) => format!("legacy-external-{id}"),
        DocumentElementId::Source(id) => {
            let source = document
                .source(id)
                .ok_or(IntentBootstrapError::UnknownDependency)?;
            match source.owner {
                DocumentSourceOwner::Constraint(owner) => format!("legacy-constraint-{owner}"),
                DocumentSourceOwner::Dimension(owner) => format!("legacy-dimension-{owner}"),
            }
        }
        _ => return Err(IntentBootstrapError::UnknownDependency),
    })
}

fn trim_boundary_dependencies(
    boundary: DocumentTrimBoundary,
    dependencies: &mut Vec<DocumentElementId>,
) {
    match boundary {
        DocumentTrimBoundary::Fixed(_) => {}
        DocumentTrimBoundary::FilletContact { owner, contact }
        | DocumentTrimBoundary::ConstraintContact { owner, contact } => {
            dependencies.push(DocumentElementId::Constraint(owner));
            dependencies.push(DocumentElementId::Contact(contact));
        }
    }
}

fn parameter_target_element(target: DocumentParameterTarget) -> DocumentElementId {
    match target {
        DocumentParameterTarget::DrivingDimension(dimension) => {
            DocumentElementId::Dimension(dimension)
        }
        DocumentParameterTarget::DimensionlessFixedScalar(property) => {
            DocumentElementId::Scalar(property.scalar)
        }
        DocumentParameterTarget::Activation(element) => element,
    }
}

fn append_document_side_tables(
    document: &SketchDocument,
    operations: &mut Vec<IntentPatchOperation>,
) -> Result<(), IntentBootstrapError> {
    for (index, source) in document.source_order().iter().copied().enumerate() {
        let index = stable_index(index)?;
        let payload = BootstrapSourceOrderEntryV1 { index, source };
        operations.push(create_operation(
            &format!("legacy-source-order-{index:08x}"),
            BootstrapNativeKind::Document,
            BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1,
            &payload,
            element_inputs(
                document,
                [
                    DocumentElementId::Document(document.id()),
                    DocumentElementId::Source(source),
                ],
            )?,
        )?);
    }
    for element in document.user_inactive_elements() {
        operations.push(create_operation(
            &format!(
                "legacy-user-inactive-{}-{}",
                element.kind(),
                element.persistent_id()
            ),
            BootstrapNativeKind::Document,
            BOOTSTRAP_USER_INACTIVE_ENTRY_CODEC_V1,
            &BootstrapUserInactiveEntryV1 { element },
            element_inputs(
                document,
                [DocumentElementId::Document(document.id()), element],
            )?,
        )?);
    }
    if let Some(activation) = document.host_configuration_activation() {
        let header = BootstrapHostActivationHeaderV1 {
            revision: activation.revision(),
            digest: activation.digest(),
        };
        operations.push(create_operation(
            "legacy-host-activation",
            BootstrapNativeKind::Document,
            BOOTSTRAP_HOST_ACTIVATION_HEADER_CODEC_V1,
            &header,
            element_inputs(document, [DocumentElementId::Document(document.id())])?,
        )?);
        for (index, value) in activation.overrides().iter().copied().enumerate() {
            let index = stable_index(index)?;
            let payload = BootstrapHostActivationOverrideV1 { index, value };
            operations.push(create_operation(
                &format!("legacy-host-activation-override-{index:08x}"),
                BootstrapNativeKind::Document,
                BOOTSTRAP_HOST_ACTIVATION_OVERRIDE_CODEC_V1,
                &payload,
                element_inputs(
                    document,
                    [DocumentElementId::Document(document.id()), value.element()],
                )?,
            )?);
        }
    }
    Ok(())
}

fn append_semantic_reservations(
    document: &SketchDocument,
    operations: &mut Vec<IntentPatchOperation>,
) -> Result<(), IntentBootstrapError> {
    for (source, catalog) in document.semantic_source_reservations() {
        let payload = BootstrapSemanticReservationV1 { source, catalog };
        let (kind, codec, alias, dependencies) = if source == catalog {
            (
                BootstrapNativeKind::SemanticCatalog,
                BOOTSTRAP_SEMANTIC_CATALOG_CODEC_V1,
                format!("legacy-semantic-catalog-{catalog}"),
                element_inputs(document, [DocumentElementId::Document(document.id())])?,
            )
        } else {
            (
                BootstrapNativeKind::SemanticSource,
                BOOTSTRAP_SEMANTIC_SOURCE_CODEC_V1,
                format!("legacy-semantic-source-{source}"),
                vec![
                    element_input(document, DocumentElementId::Document(document.id()))?,
                    (
                        InputRole::Catalog,
                        PatchPortRef::Alias {
                            node: IntentKey::new(format!("legacy-semantic-catalog-{catalog}"))?,
                            selector: IntentPortSelector::Node {
                                role: IntentPortRole::Catalog,
                                index: 0,
                            },
                        },
                    ),
                ],
            )
        };
        operations.push(create_operation(
            &alias,
            kind,
            codec,
            &payload,
            dependencies,
        )?);
    }
    Ok(())
}

fn append_computed_features(
    document: &SketchDocument,
    features: &ComputedFeatureDocument,
    operations: &mut Vec<IntentPatchOperation>,
) -> Result<(), IntentBootstrapError> {
    for feature in features.features() {
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
            &feature.definition;
        let mut dependencies = vec![DocumentElementId::Document(document.id())];
        for corner in &fillet.corners {
            dependencies.push(DocumentElementId::Curve(corner.first.source.span.curve));
            dependencies.push(DocumentElementId::Curve(corner.second.source.span.curve));
        }
        dependencies.sort_unstable();
        dependencies.dedup();
        operations.push(create_operation(
            &format!("legacy-computed-feature-{}", feature.id),
            BootstrapNativeKind::ComputedFeature,
            BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1,
            feature,
            element_inputs(document, dependencies)?,
        )?);
    }
    Ok(())
}
