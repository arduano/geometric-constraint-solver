// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::IntentNativeBinding;
use geosolve_sketch::{
    DesignPointId, DocumentConstraintDefinition, DocumentId, GeometryRole, PersistentId,
};
use geosolve_sketch_code::{
    CodeProject, ExpandedSemanticTarget, KeyedReconcileState, ProjectKey,
    materialize_code_project_cold, parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{ConstraintKind, IntentNodeKind, IntentSessionId};

fn project(source: &str) -> CodeProject {
    CodeProject {
        project: ProjectKey("direct-coincident-role".into()),
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
#[allow(
    clippy::too_many_lines,
    reason = "one focused adapter regression keeps native role, suppression, and solved Coincident evidence together"
)]
fn direct_coincident_joins_distinct_points_and_line_roles_reach_native_geometry() {
    let project = project(
        r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const datum = $.geometry.line("datum", {
    start: [0, 0],
    end: [12, 0],
    role: "construction",
  });
  const channel = $.geometry.line("channel", {
    start: [12, 1],
    end: [12, 8],
    role: "profile",
  });
  const join = $.constraint.coincident("join", {
    first: datum.end,
    second: channel.start,
  });
  const suppressedJoin = $.constraint.coincident("suppressedJoin", {
    first: datum.start,
    second: channel.end,
    suppressed: true,
  });
  return $.outputs({ datumSpan: datum.span, datumEnd: datum.end, channelSpan: channel.span, channelStart: channel.start, join, suppressedJoin });
});
"#,
    );
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("direct owners"),
            &BTreeSet::new(),
        )
        .expect("direct reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x84_c01c_0001),
        DocumentId(PersistentId::from_u128(0x84_c01c_0001)),
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

    let accepted_state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted solved state for exact input");
    let document = accepted_state.document();
    assert_eq!(document.constraints().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::Coincident { .. }
    ));
    let datum_span = semantic_binding(&materialized, "datumSpan");
    let channel_span = semantic_binding(&materialized, "channelSpan");
    let IntentNativeBinding::CurveSpan(datum_span) = datum_span else {
        panic!("datum span must retain native span ownership")
    };
    let IntentNativeBinding::CurveSpan(channel_span) = channel_span else {
        panic!("channel span must retain native span ownership")
    };
    assert_eq!(
        document.geometry_role(datum_span.curve),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        document.geometry_role(channel_span.curve),
        Some(GeometryRole::Profile)
    );

    let datum_end = semantic_point(&materialized, "datumEnd");
    let channel_start = semantic_point(&materialized, "channelStart");
    assert_ne!(
        datum_end, channel_start,
        "Coincident preserves point identity"
    );
    let solved_document = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted state for current input")
        .document();
    let datum_position = solved_document
        .point(datum_end)
        .expect("datum endpoint")
        .position;
    let channel_position = solved_document
        .point(channel_start)
        .expect("channel start")
        .position;
    assert!(
        (datum_position[0] - channel_position[0]).hypot(datum_position[1] - channel_position[1])
            <= 1.0e-9
    );

    let graph_rows = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::Coincident,
            } => Some(node.suppressed),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(graph_rows.len(), 2);
    assert!(graph_rows.contains(&false));
    assert!(graph_rows.contains(&true));
}

fn semantic_binding(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    output: &str,
) -> IntentNativeBinding {
    let ExpandedSemanticTarget::Port { port } =
        &materialized.expansion.semantic_outputs[output].target
    else {
        panic!("`{output}` must be an Intent port")
    };
    let node = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&port.alias)
        .expect("semantic output owner");
    let reference = node
        .port_by_selector(port.selector)
        .expect("semantic output port")
        .as_ref(node.id);
    materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted materialization")
        .ownership
        .port(reference)
        .expect("native output ownership")
}

fn semantic_point(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    output: &str,
) -> DesignPointId {
    let IntentNativeBinding::Point(point) = semantic_binding(materialized, output) else {
        panic!("`{output}` must be a native point")
    };
    point
}
