// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentDimensionMode, DocumentId,
    PersistentId,
};
use geosolve_sketch_code::{
    CodeExpansionError, CodeProject, ExpandedSemanticTarget, FeatureKind, KeyedReconcileState,
    ProjectKey, expand_code_project, materialize_code_project_cold, parse_managed_source,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentPortKind, IntentSession, IntentSessionId};

const MANIFOLD_VOCABULARY_SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const route = $.geometry.polyline("route", {
    vertices: [
      { key: "inlet", position: [0, 0] },
      { key: "elbow", position: [20, 0] },
      { key: "outlet", position: [20, 10] },
    ],
    closed: false,
  });
  const screw = $.geometry.circle("screw", {
    center: [30, 10],
    radius: mm(2.5),
  });
  const anchor = $.constraint.fixedPoint("anchor", {
    point: route.vertices.byKey.inlet,
    target: [0, 0],
  });
  const firstAxis = $.constraint.horizontal("firstAxis", {
    curve: route.segments.byKey.inlet,
  });
  const secondAxis = $.constraint.vertical("secondAxis", {
    curve: route.segments.byKey.elbow,
  });
  const firstLength = $.dimension.curveLength("firstLength", {
    curve: route.segments.byKey.inlet,
    target: mm(20),
    mode: "driving",
  });
  const secondLength = $.dimension.curveLength("secondLength", {
    curve: route.segments.byKey.elbow,
    target: mm(10),
  });
  const screwX = $.constraint.fixedCoordinate("screwX", {
    point: screw.center,
    axis: "x",
    target: mm(30),
  });
  const screwY = $.constraint.fixedCoordinate("screwY", {
    point: screw.center,
    axis: "y",
    target: mm(10),
  });
  const screwDiameter = $.dimension.diameter("screwDiameter", {
    curve: screw.circle,
    target: mm(5),
    mode: "driving",
  });
  return $.outputs({
    routeStart: route.vertices.byKey.inlet,
    firstSpan: route.segments.byKey.inlet,
    screw,
    screwCenter: screw.center,
    screwCurve: screw.circle,
    anchor,
    firstLength,
    screwDiameter,
  });
});
"#;

fn project(key: &str, source: &str) -> CodeProject {
    CodeProject {
        project: ProjectKey(key.into()),
        managed: parse_managed_source(source).expect("managed source"),
        custom_files: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        lock: serde_json::json!({
            "format": "geosolve-lock-v1",
            "modules": {},
        }),
    }
}

fn reconciled(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("direct generated members"),
            &BTreeSet::new(),
        )
        .expect("direct reconciliation")
        .into_staged()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one zero-DOF fixture audits native equations, semantic types, and accepted diagnostics together"
)]
fn direct_manifold_vocabulary_materializes_one_absolute_fix_and_zero_dof() {
    let project = project("direct-manifold-vocabulary", MANIFOLD_VOCABULARY_SOURCE);
    let generated = reconciled(&project);
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_0d0f_0001),
        DocumentId(PersistentId::from_u128(0x84_0d0f_0001)),
        1.0,
    )
    .expect("cold native materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted native authority");

    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let document = accepted.session.design_document();
    assert!(document.points().iter().all(|point| {
        point
            .position
            .iter()
            .all(|coordinate| coordinate.is_finite())
    }));
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );

    let fixed_points = document
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedPoint { .. }
            )
        })
        .count();
    let fixed_coordinates = document
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedCoordinate { .. }
            )
        })
        .count();
    let horizontal = document
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::Horizontal { .. }
            )
        })
        .count();
    let vertical = document
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::Vertical { .. }
            )
        })
        .count();
    assert_eq!(
        fixed_points, 1,
        "one absolute point fix anchors the fixture"
    );
    assert_eq!(fixed_coordinates, 2);
    assert_eq!(horizontal, 1);
    assert_eq!(vertical, 1);

    let mut curve_lengths = 0;
    let mut diameter = None;
    for dimension in document.dimensions() {
        assert_eq!(dimension.mode, DocumentDimensionMode::Driving);
        match dimension.definition {
            DocumentDimensionDefinition::CurveLength { .. } => curve_lengths += 1,
            DocumentDimensionDefinition::Diameter { target, .. } => {
                diameter = Some(document.scalar(target).expect("diameter target").value);
            }
            _ => panic!("unexpected direct dimension: {:?}", dimension.definition),
        }
    }
    assert_eq!(curve_lengths, 2);
    assert_eq!(diameter, Some(5.0));

    let diagnostics = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted state for exact input")
        .diagnostics();
    assert_eq!(
        diagnostics
            .rank
            .expect("rank diagnostics")
            .numerical_right_nullity,
        Some(0)
    );
    let mobility = diagnostics.mobility.expect("mobility diagnostics");
    assert_eq!(mobility.equality_degrees_of_freedom, Some(0));
    assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(0));

    assert_eq!(
        materialized.expansion.semantic_outputs["routeStart"]
            .reference
            .expected_kind,
        FeatureKind::Point
    );
    assert_eq!(
        materialized.expansion.semantic_outputs["firstSpan"]
            .reference
            .expected_kind,
        FeatureKind::CurveSpan
    );
    assert_eq!(
        materialized.expansion.semantic_outputs["screwCenter"]
            .reference
            .expected_kind,
        FeatureKind::Point
    );
    assert_eq!(
        materialized.expansion.semantic_outputs["screwCurve"]
            .reference
            .expected_kind,
        FeatureKind::Curve
    );
    assert!(matches!(
        materialized.expansion.semantic_outputs["screwCurve"].target,
        ExpandedSemanticTarget::Port {
            port: ref curve
        } if curve.kind == IntentPortKind::Curve
    ));
    assert_eq!(
        materialized.expansion.semantic_outputs["anchor"]
            .reference
            .expected_kind,
        FeatureKind::Constraint
    );
    assert_eq!(
        materialized.expansion.semantic_outputs["screwDiameter"]
            .reference
            .expected_kind,
        FeatureKind::Dimension
    );
}

#[test]
fn missing_typed_by_key_member_fails_closed_as_an_unresolved_reference() {
    let source = MANIFOLD_VOCABULARY_SOURCE.replace(
        "route.segments.byKey.inlet,\n    target: mm(20)",
        "route.segments.byKey.missing,\n    target: mm(20)",
    );
    let project = project("direct-manifold-missing-member", &source);
    let generated = reconciled(&project);
    let intent =
        IntentSession::with_id(IntentSessionId::from_raw(0x84_0d0f_0002)).expect("intent session");
    let error = expand_code_project(&project, &generated, intent.identity())
        .expect_err("unknown typed member must fail closed");
    assert!(matches!(
        error,
        CodeExpansionError::UnresolvedReference { .. }
    ));
    assert!(error.to_string().contains("route.segments.byKey.missing"));
}
