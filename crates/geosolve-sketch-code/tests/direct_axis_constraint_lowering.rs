// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    CurveDefinition, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentId, PersistentId,
};
use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, ProjectKey, materialize_code_project_cold,
    parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;
use geosolve_sketch_intent::{ConstraintKind, IntentNodeKind};

fn project(source: &str) -> CodeProject {
    CodeProject {
        project: ProjectKey("direct-axis-constraints".into()),
        managed: parse_managed_source(source).expect("managed source"),
        custom_files: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        lock: serde_json::json!({
            "format": "geosolve-lock-v1",
            "modules": {},
        }),
    }
}

#[test]
fn direct_axis_constraints_lower_line_and_span_references_with_exact_suppression() {
    let project = project(
        r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const first = $.geometry.line("first", { start: [0, 0], end: [4, 0] });
  const horizontal = $.constraint.horizontal("horizontal", {
    curve: first,
    suppressed: false,
  });
  const second = $.geometry.line("second", { start: first.end, end: [4, 4] });
  const vertical = $.constraint.vertical("vertical", {
    curve: second.span,
    suppressed: true,
  });
  return $.outputs({ first, horizontal, second, vertical });
});
"#,
    );
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_f004_c001),
        DocumentId(PersistentId::from_u128(0x84_f004_c001)),
        1.0,
    )
    .unwrap();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );

    let constraints = accepted.session.design_document().constraints();
    assert_eq!(constraints.len(), 1);
    assert!(!constraints[0].suppressed);
    assert!(matches!(
        constraints[0].definition,
        DocumentConstraintDefinition::Horizontal { .. }
    ));

    let graph_constraints = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Constraint { constraint } => Some((constraint, node.suppressed)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(graph_constraints.len(), 2);
    assert!(graph_constraints.contains(&(ConstraintKind::Horizontal, false)));
    assert!(graph_constraints.contains(&(ConstraintKind::Vertical, true)));
}

#[test]
fn direct_datum_symmetry_and_radius_dimension_lower_to_existing_native_relations() {
    let project = project(
        r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const pair = $.geometry.line("pair", { start: [-2, 1], end: [2, 1] });
  const mirror = $.constraint.symmetricAboutDatumAxis("mirror", {
    first: pair.start,
    second: pair.end,
    axis: "y",
  });
  const hole = $.geometry.circle("hole", { center: pair.start, radius: mm(1) });
  const holeRadius = $.dimension.radius("holeRadius", {
    curve: hole.circle,
    target: mm(3),
    mode: "driving",
  });
  return $.outputs({ pair, mirror, hole, holeRadius });
});
"#,
    );
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x87_f003_c001),
        DocumentId(PersistentId::from_u128(0x87_f003_c001)),
        1.0,
    )
    .unwrap();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );

    let document = accepted
        .session
        .accepted_state_for_current_input()
        .expect("direct managed relations own accepted state")
        .document();
    assert_eq!(document.constraints().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. }
    ));
    assert_eq!(document.dimensions().len(), 1);
    let dimension = &document.dimensions()[0];
    assert_eq!(dimension.mode, DocumentDimensionMode::Driving);
    let DocumentDimensionDefinition::Radius { curve, target } = &dimension.definition else {
        panic!("managed radius must lower to one native radius dimension")
    };
    assert!((document.scalar(*target).unwrap().value - 3.0).abs() <= f64::EPSILON);
    let CurveDefinition::Circle { radius, .. } = &document.curve(*curve).unwrap().definition else {
        panic!("radius dimension must target the managed circle")
    };
    assert!((document.scalar(*radius).unwrap().value - 3.0).abs() <= 1.0e-9);
}
