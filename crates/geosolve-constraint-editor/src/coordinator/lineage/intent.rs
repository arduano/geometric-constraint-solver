// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed semantic authoring-intent compiler for retained lineage actions.
//!
//! This module deliberately serializes completed authoring requests, not
//! pointer events, solver attempts, revision stamps, or accepted flat-state
//! snapshots. The structural materialization recipe retained beside this
//! intent is derived evaluator data.

use geosolve_sketch::DocumentEdit;
use geosolve_sketch_lineage::LineageActionKind;
use serde_json::{Value, json};

use super::super::ReplayAction;
use crate::GeometryToolVariant;

const INTENT_VERSION: u32 = 1;

pub(super) struct SemanticReplayIntent {
    pub category: LineageActionKind,
    pub schema: String,
    pub value: Value,
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed dispatcher keeps every replay variant visibly mapped to its semantic schema"
)]
pub(super) fn compile_replay_intent(
    replay: &ReplayAction,
    variant: Option<GeometryToolVariant>,
) -> Result<SemanticReplayIntent, serde_json::Error> {
    let (category, schema, body) = match replay {
        ReplayAction::Construction { proposal, role, .. } => (
            LineageActionKind::GeometryRecipe,
            geometry_schema(variant),
            json!({ "proposal": proposal, "role": role }),
        ),
        ReplayAction::ConstructionPlan { plan, .. } => (
            LineageActionKind::GeometryRecipe,
            geometry_schema(variant),
            json!({ "commit_plan": plan }),
        ),
        ReplayAction::ConstraintAction {
            selection, request, ..
        } => (
            LineageActionKind::Constraint,
            format!("geosolve.constraint.v1.{}", request.intent.semantic_key()),
            json!({ "selection": selection, "request": request }),
        ),
        ReplayAction::DimensionAction {
            selection, request, ..
        } => (
            LineageActionKind::Dimension,
            format!("geosolve.dimension.v1.{}", request.kind.semantic_key()),
            json!({ "selection": selection, "request": request }),
        ),
        ReplayAction::PointDistance {
            points,
            mode,
            label,
            ..
        } => (
            LineageActionKind::Dimension,
            "geosolve.dimension.v1.point-distance".into(),
            json!({ "points": points, "mode": mode, "label": label }),
        ),
        ReplayAction::SegmentLength {
            curve, mode, label, ..
        } => (
            LineageActionKind::Dimension,
            "geosolve.dimension.v1.curve-length".into(),
            json!({ "curve": curve, "mode": mode, "label": label }),
        ),
        ReplayAction::CreateComputedFillet {
            label,
            radius,
            corners,
            ..
        } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.fillet-set".into(),
            json!({ "label": label, "radius": radius, "corners": corners }),
        ),
        ReplayAction::SetComputedFilletRadius {
            feature, radius, ..
        } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.fillet-set-radius".into(),
            json!({ "feature": feature, "radius": radius }),
        ),
        ReplayAction::SetComputedFilletConfiguration {
            feature,
            radius,
            corners,
            ..
        } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.fillet-set-configuration".into(),
            json!({ "feature": feature, "radius": radius, "corners": corners }),
        ),
        ReplayAction::RemoveComputedFeature { feature, .. } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.remove".into(),
            json!({ "feature": feature }),
        ),
        ReplayAction::RemoveComputedCorner { owner, .. } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.remove-fillet-corner".into(),
            json!({ "owner": owner }),
        ),
        ReplayAction::SetComputedFeatureSuppressed {
            feature,
            suppressed,
            ..
        } => (
            LineageActionKind::ComputedFeature,
            "geosolve.feature.v1.set-suppressed".into(),
            json!({ "feature": feature, "suppressed": suppressed }),
        ),
        ReplayAction::Edit { edit, .. } => (
            document_edit_category(edit),
            format!("geosolve.document-edit.v1.{}", edit.semantic_key()),
            serde_json::to_value(edit)?,
        ),
        ReplayAction::SetDimensionMode {
            dimension, mode, ..
        } => (
            LineageActionKind::Dimension,
            "geosolve.dimension.v1.set-mode".into(),
            json!({ "dimension": dimension, "mode": mode }),
        ),
        ReplayAction::SetContactBranches {
            selection, edits, ..
        } => (
            LineageActionKind::Constraint,
            "geosolve.constraint.v1.set-contact-branches".into(),
            json!({ "selection": selection, "edits": edits }),
        ),
        ReplayAction::SetAngleOrientation {
            dimension,
            orientation,
            ..
        } => (
            LineageActionKind::Dimension,
            "geosolve.dimension.v1.set-angle-orientation".into(),
            json!({ "dimension": dimension, "orientation": orientation }),
        ),
        ReplayAction::RebindExternalBinding {
            binding,
            expected_kind,
            expected_topology,
            ..
        } => (
            LineageActionKind::External,
            "geosolve.external.v1.rebind".into(),
            json!({
                "binding": binding,
                "expected_kind": expected_kind,
                "expected_topology": expected_topology,
            }),
        ),
        ReplayAction::Delete { selection, .. } => (
            LineageActionKind::Operation,
            "geosolve.operation.v1.delete".into(),
            json!({ "selection": selection }),
        ),
        ReplayAction::SetSuppressed {
            selection,
            suppressed,
            ..
        } => (
            LineageActionKind::Operation,
            "geosolve.operation.v1.set-suppressed".into(),
            json!({ "selection": selection, "suppressed": suppressed }),
        ),
        ReplayAction::Reattempt { .. } => (
            LineageActionKind::Operation,
            "geosolve.operation.v1.reattempt".into(),
            json!({}),
        ),
        ReplayAction::Undo => (
            LineageActionKind::Operation,
            "geosolve.operation.v1.undo".into(),
            json!({}),
        ),
        ReplayAction::Redo => (
            LineageActionKind::Operation,
            "geosolve.operation.v1.redo".into(),
            json!({}),
        ),
    };
    Ok(SemanticReplayIntent {
        category,
        schema,
        value: json!({
            "version": INTENT_VERSION,
            "body": body,
        }),
    })
}

fn geometry_schema(variant: Option<GeometryToolVariant>) -> String {
    format!(
        "geosolve.geometry.v1.{}",
        variant.map_or("legacy-construction", GeometryToolVariant::key)
    )
}

#[allow(
    clippy::match_same_arms,
    reason = "named operation variants document the closed catalog while the wildcard remains forward-compatible with a non-exhaustive domain enum"
)]
const fn document_edit_category(edit: &DocumentEdit) -> LineageActionKind {
    match edit {
        DocumentEdit::CreateConstraint { .. }
        | DocumentEdit::SetLineLineFilletBranch { .. }
        | DocumentEdit::SetCurveCurveFilletBranch { .. }
        | DocumentEdit::SetContactStates { .. }
        | DocumentEdit::SetContactBranches { .. }
        | DocumentEdit::SetCircleTangencyBranch { .. } => LineageActionKind::Constraint,
        DocumentEdit::CreateDimension { .. }
        | DocumentEdit::CreateProfileOffset { .. }
        | DocumentEdit::SetDimensionMode { .. }
        | DocumentEdit::SetProfileOffsetOperand { .. }
        | DocumentEdit::SetOrientedAngleOrientation { .. } => LineageActionKind::Dimension,
        DocumentEdit::CreateParameter { .. } => LineageActionKind::Parameter,
        DocumentEdit::AddParameterBinding { .. }
        | DocumentEdit::RemoveParameterBinding { .. }
        | DocumentEdit::AddParameterOutput { .. }
        | DocumentEdit::RemoveParameterOutput { .. }
        | DocumentEdit::SetGeometryRole { .. }
        | DocumentEdit::SetGeometryRoles { .. }
        | DocumentEdit::SetHostConfigurationActivation { .. } => LineageActionKind::Binding,
        DocumentEdit::CreatePoint { .. }
        | DocumentEdit::CreateScalar { .. }
        | DocumentEdit::CreateCurve { .. }
        | DocumentEdit::CreateContact { .. }
        | DocumentEdit::CreateProfileOffsetGeometry { .. }
        | DocumentEdit::CreatePreparedProfileOffsetGeometry { .. }
        | DocumentEdit::CreatePreparedNativeLineFilletGeometry { .. }
        | DocumentEdit::CreateRectangle { .. }
        | DocumentEdit::CreateMirroredCurve { .. }
        | DocumentEdit::CreateLineLineFillet { .. }
        | DocumentEdit::CreateCurveCurveFillet { .. }
        | DocumentEdit::SetPointPosition { .. }
        | DocumentEdit::SetScalarValue { .. }
        | DocumentEdit::SetCurveBranch { .. }
        | DocumentEdit::SetArcSweep { .. }
        | DocumentEdit::SetConicWeightedMiddle { .. }
        | DocumentEdit::SetRationalConicControl { .. }
        | DocumentEdit::SetHyperbolaBranch { .. }
        | DocumentEdit::InsertBSplineKnot { .. }
        | DocumentEdit::InsertMirroredBSplineKnot { .. }
        | DocumentEdit::TransitionBSplineContact { .. }
        | DocumentEdit::InsertNurbsKnot { .. }
        | DocumentEdit::TransitionNurbsContact { .. }
        | DocumentEdit::SetNurbsWeightGauge { .. }
        | DocumentEdit::SetSourceSuppressed { .. }
        | DocumentEdit::SetElementUserSuppressed { .. }
        | DocumentEdit::Delete { .. } => LineageActionKind::Operation,
        _ => LineageActionKind::Operation,
    }
}
