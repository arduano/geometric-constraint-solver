// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed semantic surface evaluated by the DOM-free RPC adapter.
//!
//! `geosolve-sketch-lineage` deliberately permits versioned host schemas. The
//! public RPC is narrower: it admits only the M83 schemas that this adapter can
//! evaluate deterministically. Adding a schema therefore requires an explicit
//! registry change and native/WASM transcript evidence.

use geosolve_sketch_lineage::{
    ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind, LineageDocument,
};

const SUPPORTED_ACTION_VERSION: u32 = 1;
const WORKBENCH_BASELINE_MEDIA_TYPE: &str =
    "application/vnd.geosolve.workbench-imported-baseline+json";

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SemanticSchemaError {
    pub code: &'static str,
    pub message: String,
}

impl SemanticSchemaError {
    fn schema(schema: &str) -> Self {
        Self {
            code: "unsupported_action_schema",
            message: format!("unsupported lineage action schema `{schema}`"),
        }
    }

    fn version(schema: &str, version: u32) -> Self {
        Self {
            code: "unsupported_action_version",
            message: format!(
                "unsupported lineage action version {version} for `{schema}`; expected {SUPPORTED_ACTION_VERSION}"
            ),
        }
    }

    fn kind(schema: &str, expected: LineageActionKind, actual: LineageActionKind) -> Self {
        Self {
            code: "action_kind_mismatch",
            message: format!(
                "lineage action schema `{schema}` belongs to {expected:?}, not {actual:?}"
            ),
        }
    }
}

/// Rejects every schema/version pair not owned by the current RPC evaluator.
pub(super) fn validate_document(document: &LineageDocument) -> Result<(), SemanticSchemaError> {
    for step in document.steps() {
        match &step.action {
            LineageActionDefinition::ImportedBaseline { baseline } => {
                let (schema, version, supported) = match &baseline.encoding {
                    ImportedBaselineEncoding::Opaque {
                        media_type,
                        version,
                    } => (
                        media_type.as_str(),
                        *version,
                        media_type.as_str() == WORKBENCH_BASELINE_MEDIA_TYPE,
                    ),
                    ImportedBaselineEncoding::Canonical { schema, version } => {
                        (schema.as_str(), *version, false)
                    }
                };
                if version != SUPPORTED_ACTION_VERSION {
                    return Err(SemanticSchemaError::version(schema, version));
                }
                if !supported {
                    return Err(SemanticSchemaError::schema(schema));
                }
            }
            action => {
                let payload = action_payload(action);
                if payload.version != SUPPORTED_ACTION_VERSION {
                    return Err(SemanticSchemaError::version(
                        payload.schema.as_str(),
                        payload.version,
                    ));
                }
                let expected = schema_kind(payload.schema.as_str())
                    .ok_or_else(|| SemanticSchemaError::schema(payload.schema.as_str()))?;
                let actual = action.kind();
                if expected != actual {
                    return Err(SemanticSchemaError::kind(
                        payload.schema.as_str(),
                        expected,
                        actual,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn action_payload(
    action: &LineageActionDefinition,
) -> &geosolve_sketch_lineage::VersionedActionPayload {
    match action {
        LineageActionDefinition::ImportedBaseline { .. } => {
            unreachable!("imported baselines are handled before versioned action payloads")
        }
        LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::Trim { action }
        | LineageActionDefinition::Parameter { action }
        | LineageActionDefinition::Binding { action }
        | LineageActionDefinition::External { action }
        | LineageActionDefinition::Operation { action }
        | LineageActionDefinition::ComputedFeature { action }
        | LineageActionDefinition::Annotation { action } => action,
    }
}

fn schema_kind(schema: &str) -> Option<LineageActionKind> {
    if is_geometry_schema(schema) {
        Some(LineageActionKind::GeometryRecipe)
    } else if is_constraint_schema(schema) {
        Some(LineageActionKind::Constraint)
    } else if is_dimension_schema(schema) {
        Some(LineageActionKind::Dimension)
    } else if schema == "geosolve.trim.v1.curve-trim-view" {
        Some(LineageActionKind::Trim)
    } else if schema == "geosolve.parameter.v1.host-parameter" {
        Some(LineageActionKind::Parameter)
    } else if is_binding_schema(schema) {
        Some(LineageActionKind::Binding)
    } else if is_external_schema(schema) {
        Some(LineageActionKind::External)
    } else if is_operation_schema(schema) {
        Some(LineageActionKind::Operation)
    } else if is_feature_schema(schema) {
        Some(LineageActionKind::ComputedFeature)
    } else if schema == "geosolve.annotation.v1.annotation-layout" {
        Some(LineageActionKind::Annotation)
    } else {
        document_edit_kind(schema)
    }
}

fn is_geometry_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.geometry.v1.sketch-point"
            | "geosolve.geometry.v1.segment"
            | "geosolve.geometry.v1.polyline"
            | "geosolve.geometry.v1.midpoint-line"
            | "geosolve.geometry.v1.two-point-aligned-rectangle"
            | "geosolve.geometry.v1.three-point-corner-rectangle"
            | "geosolve.geometry.v1.center-rectangle"
            | "geosolve.geometry.v1.three-point-center-rectangle"
            | "geosolve.geometry.v1.center-radius-circle"
            | "geosolve.geometry.v1.two-point-diameter-circle"
            | "geosolve.geometry.v1.three-point-circle"
            | "geosolve.geometry.v1.center-arc"
            | "geosolve.geometry.v1.three-point-arc"
            | "geosolve.geometry.v1.tangent-arc"
            | "geosolve.geometry.v1.center-axes-ellipse"
            | "geosolve.geometry.v1.axis-endpoints-ellipse"
            | "geosolve.geometry.v1.center-axes-elliptical-arc"
            | "geosolve.geometry.v1.axis-endpoints-elliptical-arc"
            | "geosolve.geometry.v1.quadratic-bezier"
            | "geosolve.geometry.v1.cubic-bezier"
            | "geosolve.geometry.v1.rational-quadratic-conic"
            | "geosolve.geometry.v1.parabola"
            | "geosolve.geometry.v1.hyperbola"
            | "geosolve.geometry.v1.open-control-nurbs"
            | "geosolve.geometry.v1.periodic-control-nurbs"
            | "geosolve.geometry.v1.legacy-construction"
    )
}

fn is_constraint_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.constraint.v1.fixed-point"
            | "geosolve.constraint.v1.fixed-coordinate"
            | "geosolve.constraint.v1.coincident-with-origin"
            | "geosolve.constraint.v1.point-on-datum-axis"
            | "geosolve.constraint.v1.coincident"
            | "geosolve.constraint.v1.external-point-coincident"
            | "geosolve.constraint.v1.horizontal"
            | "geosolve.constraint.v1.vertical"
            | "geosolve.constraint.v1.horizontal-points"
            | "geosolve.constraint.v1.vertical-points"
            | "geosolve.constraint.v1.horizontal-point-to-midpoint"
            | "geosolve.constraint.v1.vertical-point-to-midpoint"
            | "geosolve.constraint.v1.point-on-curve"
            | "geosolve.constraint.v1.parallel"
            | "geosolve.constraint.v1.perpendicular"
            | "geosolve.constraint.v1.external-line-collinear"
            | "geosolve.constraint.v1.collinear-with-datum-axis"
            | "geosolve.constraint.v1.concentric"
            | "geosolve.constraint.v1.collinear"
            | "geosolve.constraint.v1.equal-length"
            | "geosolve.constraint.v1.equal-radius"
            | "geosolve.constraint.v1.midpoint"
            | "geosolve.constraint.v1.symmetric-about-line"
            | "geosolve.constraint.v1.symmetric-about-datum-axis"
            | "geosolve.constraint.v1.line-circle-tangency"
            | "geosolve.constraint.v1.circle-circle-tangency"
            | "geosolve.constraint.v1.circle-arc-tangency"
            | "geosolve.constraint.v1.line-curve-tangency"
            | "geosolve.constraint.v1.curve-curve-contact"
            | "geosolve.constraint.v1.curve-curve-tangency"
            | "geosolve.constraint.v1.curve-direction"
            | "geosolve.constraint.v1.equal-curvature"
            | "geosolve.constraint.v1.endpoint-continuity"
            | "geosolve.constraint.v1.line-line-fillet"
            | "geosolve.constraint.v1.curve-curve-fillet"
            | "geosolve.constraint.v1.lock"
            | "geosolve.constraint.v1.equal"
            | "geosolve.constraint.v1.symmetric"
            | "geosolve.constraint.v1.tangent"
            | "geosolve.constraint.v1.continuity"
            | "geosolve.constraint.v1.set-contact-branches"
    )
}

fn is_dimension_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.dimension.v1.point-distance"
            | "geosolve.dimension.v1.curve-length"
            | "geosolve.dimension.v1.radius"
            | "geosolve.dimension.v1.diameter"
            | "geosolve.dimension.v1.oriented-angle"
            | "geosolve.dimension.v1.supporting-line-offset"
            | "geosolve.dimension.v1.exact-translated-segment-offset"
            | "geosolve.dimension.v1.profile-offset"
            | "geosolve.dimension.v1.set-mode"
            | "geosolve.dimension.v1.set-angle-orientation"
    )
}

fn is_binding_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.binding.v1.parameter-binding"
            | "geosolve.binding.v1.parameter-output"
            | "geosolve.binding.v1.host-activation"
            | "geosolve.binding.v1.geometry-role"
    )
}

fn is_external_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.external.v1.external-binding"
            | "geosolve.external.v1.external-snapshot"
            | "geosolve.external.v1.rebind"
    )
}

fn is_operation_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.operation.v1.split"
            | "geosolve.operation.v1.break"
            | "geosolve.operation.v1.trim"
            | "geosolve.operation.v1.extend"
            | "geosolve.operation.v1.mirror"
            | "geosolve.operation.v1.chamfer"
            | "geosolve.operation.v1.associative-fillet"
            | "geosolve.operation.v1.rectangle"
            | "geosolve.operation.v1.regular-polygon"
            | "geosolve.operation.v1.slot"
            | "geosolve.operation.v1.linear-pattern"
            | "geosolve.operation.v1.profile-offset"
            | "geosolve.operation.v1.native-line-fillet"
            | "geosolve.operation.v1.delete"
            | "geosolve.operation.v1.set-suppressed"
            | "geosolve.operation.v1.reattempt"
            | "geosolve.operation.v1.undo"
            | "geosolve.operation.v1.redo"
    )
}

fn is_feature_schema(schema: &str) -> bool {
    matches!(
        schema,
        "geosolve.feature.v1.fillet-set"
            | "geosolve.feature.v1.fillet-set-radius"
            | "geosolve.feature.v1.fillet-set-configuration"
            | "geosolve.feature.v1.remove"
            | "geosolve.feature.v1.remove-fillet-corner"
            | "geosolve.feature.v1.set-suppressed"
    )
}

fn document_edit_kind(schema: &str) -> Option<LineageActionKind> {
    let key = schema.strip_prefix("geosolve.document-edit.v1.")?;
    if matches!(
        key,
        "create-constraint"
            | "set-line-line-fillet-branch"
            | "set-curve-curve-fillet-branch"
            | "set-contact-states"
            | "set-contact-branches"
            | "set-circle-tangency-branch"
    ) {
        Some(LineageActionKind::Constraint)
    } else if matches!(
        key,
        "create-dimension"
            | "create-profile-offset"
            | "set-dimension-mode"
            | "set-profile-offset-operand"
            | "set-oriented-angle-orientation"
    ) {
        Some(LineageActionKind::Dimension)
    } else if key == "create-parameter" {
        Some(LineageActionKind::Parameter)
    } else if matches!(
        key,
        "add-parameter-binding"
            | "remove-parameter-binding"
            | "add-parameter-output"
            | "remove-parameter-output"
            | "set-geometry-role"
            | "set-geometry-roles"
            | "set-host-configuration-activation"
    ) {
        Some(LineageActionKind::Binding)
    } else if is_operation_document_edit(key) {
        Some(LineageActionKind::Operation)
    } else {
        None
    }
}

fn is_operation_document_edit(key: &str) -> bool {
    matches!(
        key,
        "create-point"
            | "create-scalar"
            | "create-curve"
            | "create-contact"
            | "create-profile-offset-geometry"
            | "create-prepared-profile-offset-geometry"
            | "create-prepared-native-line-fillet-geometry"
            | "create-rectangle"
            | "create-mirrored-curve"
            | "create-line-line-fillet"
            | "create-curve-curve-fillet"
            | "set-point-position"
            | "set-scalar-value"
            | "set-curve-branch"
            | "set-arc-sweep"
            | "set-conic-weighted-middle"
            | "set-rational-conic-control"
            | "set-hyperbola-branch"
            | "insert-b-spline-knot"
            | "insert-mirrored-b-spline-knot"
            | "transition-b-spline-contact"
            | "insert-nurbs-knot"
            | "transition-nurbs-contact"
            | "set-nurbs-weight-gauge"
            | "set-source-suppressed"
            | "set-element-user-suppressed"
            | "delete"
    )
}

#[cfg(test)]
mod tests {
    use geosolve_sketch_lineage::{
        LineageActionDefinition, LineageSemanticKey, VersionedActionPayload,
    };

    use super::{SemanticSchemaError, schema_kind};

    #[test]
    fn schema_registry_is_closed_and_kind_specific() {
        assert!(schema_kind("geosolve.geometry.v1.sketch-point").is_some());
        assert!(schema_kind("geosolve.geometry.v1.future-shape").is_none());
        assert!(schema_kind("geosolve.geometry.v2.sketch-point").is_none());

        let error = SemanticSchemaError::schema("future");
        assert_eq!(error.code, "unsupported_action_schema");
        let action = LineageActionDefinition::GeometryRecipe {
            action: VersionedActionPayload::empty(
                LineageSemanticKey::new("geosolve.geometry.v1.sketch-point").expect("schema"),
                1,
            ),
        };
        assert_eq!(
            action.kind(),
            schema_kind("geosolve.geometry.v1.sketch-point").unwrap()
        );
    }
}
