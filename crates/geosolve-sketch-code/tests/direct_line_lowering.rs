// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    reason = "authored literal endpoint coordinates are required to round-trip bit-exactly"
)]

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::IntentNativeBinding;
use geosolve_sketch::{DesignPointId, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeExpansionError, CodeInteractionOverlay, CodePointEdit, CodeProject, CodeProjectDemoId,
    ExpandedCodeProject, ExpandedSemanticTarget, KeyedReconcileState, ManagedDiagnosticCode,
    ProjectKey, SemanticSymbol, bundled_code_project_demos, expand_code_project,
    materialize_code_project_cold, materialize_code_project_cold_with_overlay,
    parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeKind,
    IntentPatchOperation, IntentSession, IntentSessionId,
};

fn project(key: &str, body: &str) -> CodeProject {
    let source = format!(
        concat!(
            "// SPDX-License-Identifier: GPL-3.0-or-later\n\n",
            "\"use geosolve managed-v1\";\n",
            "import {{ sketch }} from \"@geosolve/sketch-code\";\n\n",
            "export default sketch(($) => {{\n",
            "{body}",
            "}});\n",
        ),
        body = body,
    );
    CodeProject {
        project: ProjectKey(key.into()),
        managed: parse_managed_source(&source).expect("managed source"),
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
            required_generated_members(project).expect("direct generated-member plan"),
            &BTreeSet::new(),
        )
        .expect("direct reconciliation")
        .into_staged()
}

fn materialize(project: &CodeProject, seed: u128) -> geosolve_sketch_code::MaterializedCodeProject {
    materialize_code_project_cold(
        project,
        &reconciled(project),
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .expect("cold materialization")
}

fn output_point(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    output: &str,
) -> DesignPointId {
    let target = &materialized.expansion.semantic_outputs[output].target;
    let port = match target {
        ExpandedSemanticTarget::Port { port } => port,
        ExpandedSemanticTarget::FeatureCorner { corner } => &corner.point,
        other => panic!("`{output}` is not a point output: {other:?}"),
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
        .expect("semantic point port")
        .as_ref(node.id);
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted materialization");
    let IntentNativeBinding::Point(point) = accepted
        .ownership
        .port(reference)
        .expect("native point ownership")
    else {
        panic!("semantic point port did not retain point ownership")
    };
    point
}

fn assert_current_and_hard_valid(materialized: &geosolve_sketch_code::MaterializedCodeProject) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted materialization");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

fn segment_direction_for_declaration(
    expansion: &ExpandedCodeProject,
    declaration: &str,
) -> [f64; 2] {
    let field = IntentFieldKey(IntentKey::new("branch_direction").unwrap());
    expansion
        .patch
        .operations()
        .iter()
        .find_map(|operation| {
            let IntentPatchOperation::CreateNode { alias, draft, .. } = operation else {
                return None;
            };
            if expansion.declaration_for_alias(alias) != Some(&SemanticSymbol(declaration.into()))
                || !matches!(
                    draft.kind,
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment
                    }
                )
            {
                return None;
            }
            let Some(IntentLiteral::Point(direction)) = draft.fields.get(&field) else {
                panic!("segment `{declaration}` has no explicit branch direction")
            };
            Some(*direction)
        })
        .unwrap_or_else(|| panic!("segment declaration `{declaration}` was not expanded"))
}

fn assert_segment_branch_contract(expansion: &ExpandedCodeProject, expected_count: usize) {
    let field = IntentFieldKey(IntentKey::new("branch_direction").unwrap());
    let mut count = 0;
    for operation in expansion.patch.operations() {
        let IntentPatchOperation::CreateNode { draft, .. } = operation else {
            continue;
        };
        if !matches!(
            draft.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment
            }
        ) {
            continue;
        }
        count += 1;
        let Some(IntentLiteral::Point(direction)) = draft.fields.get(&field) else {
            panic!("every lowered segment must retain explicit branch direction")
        };
        assert!(direction.iter().all(|component| component.is_finite()));
        assert!((direction[0].hypot(direction[1]) - 1.0).abs() <= 1.0e-12);
    }
    assert_eq!(count, expected_count);
}

#[test]
fn direct_line_reuses_two_lexically_referenced_rectangle_corners() {
    let project = project(
        "direct-line-references",
        r#"  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  return $.outputs({
    frameLowerLeft: frame.corners.lowerLeft,
    frameUpperRight: frame.corners.upperRight,
    lineStart: diagonal.start,
    lineEnd: diagonal.end,
    diagonal,
  });
"#,
    );

    let line = &project.managed.program.declarations[1];
    let geosolve_sketch_code::ManagedValue::Object(arguments) = &line.arguments else {
        panic!("line arguments must remain a managed object")
    };
    assert!(matches!(
        arguments["start"],
        geosolve_sketch_code::ManagedValue::Reference { .. }
    ));
    assert!(matches!(
        arguments["end"],
        geosolve_sketch_code::ManagedValue::Reference { .. }
    ));

    let materialized = materialize(&project, 0x84f0_0301);
    assert_current_and_hard_valid(&materialized);
    assert_segment_branch_contract(&materialized.expansion, 1);
    assert_eq!(
        output_point(&materialized, "lineStart"),
        output_point(&materialized, "frameLowerLeft")
    );
    assert_eq!(
        output_point(&materialized, "lineEnd"),
        output_point(&materialized, "frameUpperRight")
    );
    assert_eq!(
        materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len(),
        4,
        "referenced endpoints must not allocate duplicate native points"
    );
}

#[test]
fn direct_line_supports_one_literal_and_one_lexical_point_reference() {
    let project = project(
        "direct-line-mixed",
        r#"  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const leader = $.geometry.line("leader", {
    start: [-12, 7],
    end: frame.corners.lowerRight,
  });
  return $.outputs({
    frameEnd: frame.corners.lowerRight,
    lineStart: leader.start,
    lineEnd: leader.end,
    leader,
  });
"#,
    );

    let materialized = materialize(&project, 0x84f0_0302);
    assert_current_and_hard_valid(&materialized);
    assert_segment_branch_contract(&materialized.expansion, 1);
    assert_eq!(
        output_point(&materialized, "lineEnd"),
        output_point(&materialized, "frameEnd")
    );
    let start = output_point(&materialized, "lineStart");
    assert_eq!(
        materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .point(start)
            .unwrap()
            .position,
        [-12.0, 7.0]
    );
    assert_eq!(
        materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len(),
        5,
        "only the literal endpoint may allocate a new native point"
    );
}

#[test]
fn direct_line_accepts_an_ordinary_point_output_reference() {
    let project = project(
        "direct-line-point-output",
        r#"  const base = $.geometry.line("base", {
    start: [1, 2],
    end: [8, 5],
  });
  const continuation = $.geometry.line("continuation", {
    start: base.end,
    end: [13, 11],
  });
  return $.outputs({
    baseEnd: base.end,
    continuationStart: continuation.start,
    continuationEnd: continuation.end,
    continuation,
  });
"#,
    );

    let materialized = materialize(&project, 0x84f0_0304);
    assert_current_and_hard_valid(&materialized);
    assert_segment_branch_contract(&materialized.expansion, 2);
    assert_eq!(
        output_point(&materialized, "baseEnd"),
        output_point(&materialized, "continuationStart")
    );
    assert_eq!(
        materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len(),
        3,
        "the second declaration must reuse the first declaration's point output"
    );
}

#[test]
fn detached_reference_and_generated_lines_recompute_explicit_branch_direction() {
    let project = project(
        "direct-line-detached-reference",
        r#"  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  return $.outputs({ frame, diagonal });
"#,
    );
    let generated = reconciled(&project);
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0306)).unwrap();
    let expanded = expand_code_project(&project, &generated, intent.identity()).unwrap();
    let detached = expanded
        .writable_points
        .iter()
        .find(|point| {
            expanded.declaration_for_alias(&point.handle.alias)
                == Some(&SemanticSymbol("diagonal".into()))
                && matches!(&point.edit, CodePointEdit::Point { address }
                    if address.output.0.last().is_some_and(|segment| {
                        matches!(segment, geosolve_sketch_code::ManagedPathSegment::Field(field) if field == "start")
                    }))
        })
        .unwrap();
    let overlay = detached
        .stage_drag(&CodeInteractionOverlay::empty(), [10.0, -5.0])
        .unwrap();
    let materialized = materialize_code_project_cold_with_overlay(
        &project,
        &generated,
        &overlay,
        IntentSessionId::from_raw(0x84f0_0307),
        DocumentId(PersistentId::from_u128(0x84f0_0307)),
        1.0,
    )
    .unwrap();
    assert_current_and_hard_valid(&materialized);
    assert_segment_branch_contract(&materialized.expansion, 1);
    let direction = segment_direction_for_declaration(&materialized.expansion, "diagonal");
    let expected_norm = 50.0_f64.hypot(40.0);
    assert!((direction[0] - 50.0 / expected_norm).abs() <= 1.0e-12);
    assert!((direction[1] - 40.0 / expected_norm).abs() <= 1.0e-12);

    let generated_project = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::BracedFrame)
        .unwrap()
        .project();
    let generated_materialized = materialize(&generated_project, 0x84f0_0308);
    assert_current_and_hard_valid(&generated_materialized);
    assert_segment_branch_contract(&generated_materialized.expansion, 2);
}

#[test]
fn direct_line_rejects_a_curve_reference_as_a_point_endpoint() {
    let project = project(
        "direct-line-wrong-kind",
        r#"  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const invalid = $.geometry.line("invalid", {
    start: frame.edges.bottom,
    end: frame.corners.upperRight,
  });
  return $.outputs({ invalid });
"#,
    );
    let generated = KeyedReconcileState::empty();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0303)).unwrap();

    assert!(matches!(
        expand_code_project(&project, &generated, intent.identity()),
        Err(CodeExpansionError::KindMismatch {
            expected: geosolve_sketch_code::FeatureKind::Point,
            actual: geosolve_sketch_code::FeatureKind::CurveSpan,
            ..
        })
    ));
}

#[test]
fn direct_line_rejects_a_foreign_lexical_reference_before_expansion() {
    let source = concat!(
        "// SPDX-License-Identifier: GPL-3.0-or-later\n\n",
        "\"use geosolve managed-v1\";\n",
        "import { sketch } from \"@geosolve/sketch-code\";\n\n",
        "export default sketch(($) => {\n",
        "  const local = $.geometry.line(\"local\", {\n",
        "    start: foreignProject.point,\n",
        "    end: [1, 2],\n",
        "  });\n",
        "  return $.outputs({ local });\n",
        "});\n",
    );

    let error = parse_managed_source(source).unwrap_err();
    assert_eq!(
        error.diagnostic.code,
        ManagedDiagnosticCode::ForwardReference
    );
    assert!(
        error
            .diagnostic
            .message
            .contains("does not name an earlier declaration")
    );
}

#[test]
fn direct_line_rejects_raw_ids_transport_dtos_and_misspelled_members() {
    let cases = [
        (
            "raw-string-id",
            r#""frame.corners.lowerLeft""#,
            "invalid.start",
        ),
        (
            "transport-dto",
            r#"{ declaration: "frame", output: ["corners", "lowerLeft"], kind: "point" }"#,
            "invalid.start",
        ),
        (
            "misspelled-member",
            "frame.corners.lowerleft",
            "frame.corners.lowerleft",
        ),
    ];

    for (key, endpoint, expected_reference) in cases {
        let project = project(
            key,
            &format!(
                r#"  const frame = $.geometry.rectangle("frame", {{
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  }});
  const invalid = $.geometry.line("invalid", {{
    start: {endpoint},
    end: frame.corners.upperRight,
  }});
  return $.outputs({{ invalid }});
"#,
            ),
        );
        let generated = KeyedReconcileState::empty();
        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0305)).unwrap();

        assert_eq!(
            expand_code_project(&project, &generated, intent.identity()),
            Err(CodeExpansionError::UnresolvedReference {
                reference: expected_reference.into(),
            }),
            "managed source case `{key}` must fail closed before materialization",
        );
    }
}
