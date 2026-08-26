// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, ComputedFeatureEvaluationState, IntentNativeBinding,
};
use geosolve_sketch::{DocumentConstraintDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeExpansionError, CodeProject, CodeProjectDemoId, FeatureKind, KeyedReconcileState,
    ManagedValue, ProjectKey, SemanticOutputPath, bundled_code_project_demos, expand_code_project,
    materialize_code_project_cold, parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, InputRole, InputSlot, IntentLiteral, IntentNodeKind, IntentPatchOperation,
    IntentSession, IntentSessionId, IntentUnit,
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

fn valid_project() -> CodeProject {
    project(
        "direct-fillet-set",
        r#"  const first = $.geometry.line("first", {
    start: [0, 0],
    end: [4, 0],
  });
  const second = $.geometry.line("second", {
    start: first.end,
    end: [4, 4],
  });
  const round = $.computed.filletSet("round", {
    radius: 1,
    corners: [{
      parents: [{
        span: first.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: second.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: false,
  });
  return $.outputs({ first, second, round });
"#,
    )
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

#[test]
fn parser_valid_managed_fillet_fixture_is_the_typescript_compile_target() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("code crate lives under the workspace crates directory");
    let path = workspace.join("packages/geosolve-sketch-code/test/managed/line-fillet.managed.ts");
    let source = fs::read_to_string(path).expect("checked-in managed Fillet TypeScript fixture");
    let mut candidate = project(
        "managed-ts-fillet",
        source
            .split_once("export default sketch(($) => {\n")
            .expect("managed fixture wrapper")
            .1
            .strip_suffix("});\n")
            .expect("managed fixture suffix"),
    );
    candidate.managed = parse_managed_source(&source).expect("managed fixture must parse in Rust");
    let materialized = materialize_code_project_cold(
        &candidate,
        &reconciled(&candidate),
        IntentSessionId::from_raw(0x84f0_0409),
        DocumentId(PersistentId::from_u128(0x84f0_0409)),
        1.0,
    )
    .expect("the same managed source compiled by TypeScript must materialize");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("managed fixture accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    let constraints = accepted.session.design_document().constraints();
    assert_eq!(constraints.len(), 2);
    assert!(constraints.iter().any(|constraint| matches!(
        constraint.definition,
        DocumentConstraintDefinition::Horizontal { .. }
    )));
    assert!(constraints.iter().any(|constraint| matches!(
        constraint.definition,
        DocumentConstraintDefinition::Vertical { .. }
    )));
    assert!(constraints.iter().all(|constraint| !constraint.suppressed));
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused owner proves exact Fillet fields and independently accepted computed geometry"
)]
fn direct_fillet_set_lowers_exact_explicit_state_and_materializes_current_geometry() {
    let project = valid_project();
    let declaration = &project.managed.program.declarations[2];
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("FilletSet arguments must remain a managed object")
    };
    let ManagedValue::Array(corners) = &arguments["corners"] else {
        panic!("FilletSet corners must remain an array")
    };
    let [ManagedValue::Object(corner)] = corners.as_slice() else {
        panic!("fixture must contain one corner")
    };
    let ManagedValue::Array(parents) = &corner["parents"] else {
        panic!("FilletSet parents must remain a fixed array")
    };
    assert_eq!(parents.len(), 2);
    for parent in parents {
        let ManagedValue::Object(parent) = parent else {
            panic!("FilletSet parent must remain an object")
        };
        assert!(matches!(parent["span"], ManagedValue::Reference { .. }));
    }

    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0401)).unwrap();
    let expansion =
        expand_code_project(&project, &reconciled(&project), intent.identity()).unwrap();
    let draft = expansion
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. }
                if matches!(
                    draft.kind,
                    IntentNodeKind::ComputedFeature {
                        feature: ComputedFeatureKind::FilletSet
                    }
                ) =>
            {
                Some(draft.as_ref())
            }
            _ => None,
        })
        .expect("one direct FilletSet draft");
    assert_eq!(draft.dynamic_children, 1);
    assert!(!draft.suppressed);
    assert_eq!(draft.inputs.len(), 2);
    assert!(
        draft
            .inputs
            .contains_key(&InputSlot::new(InputRole::Span, 0))
    );
    assert!(
        draft
            .inputs
            .contains_key(&InputSlot::new(InputRole::Span, 1))
    );
    assert_eq!(
        draft.fields[&geosolve_sketch_intent::IntentFieldKey(
            geosolve_sketch_intent::IntentKey::new("radius").unwrap()
        )],
        IntentLiteral::Quantity {
            value: 1.0,
            unit: IntentUnit::Length,
        }
    );
    for field in [
        "corner_0000_first_parameter",
        "corner_0000_first_winding",
        "corner_0000_first_neighborhood",
        "corner_0000_first_normal_side",
        "corner_0000_first_trim_endpoint",
        "corner_0000_first_periodic_anchor",
        "corner_0000_second_parameter",
        "corner_0000_second_winding",
        "corner_0000_second_neighborhood",
        "corner_0000_second_normal_side",
        "corner_0000_second_trim_endpoint",
        "corner_0000_second_periodic_anchor",
        "corner_0000_endpoint_order",
        "corner_0000_sweep",
    ] {
        assert!(
            draft.fields.keys().any(|key| key.0.as_str() == field),
            "missing exact field `{field}`"
        );
    }

    let output = &expansion.semantic_outputs["round"];
    assert_eq!(output.reference.expected_kind, FeatureKind::Feature);
    assert_eq!(output.reference.output, SemanticOutputPath::default());

    let materialized = materialize_code_project_cold(
        &project,
        &reconciled(&project),
        IntentSessionId::from_raw(0x84f0_0402),
        DocumentId(PersistentId::from_u128(0x84f0_0402)),
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
    assert!(
        accepted
            .session
            .design_document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    let [feature] = accepted.features.features() else {
        panic!("one accepted computed feature expected")
    };
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    assert_eq!(fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(fillet.corners.len(), 1);
    assert_eq!(accepted.computed.edges().len(), 3);
    assert!(matches!(
        accepted.computed.feature_evaluations()[0].state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    let fillet_node = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| matches!(node.kind, IntentNodeKind::ComputedFeature { .. }))
        .unwrap();
    assert!(
        accepted
            .ownership
            .node(fillet_node.id)
            .unwrap()
            .owned
            .contains(&IntentNativeBinding::ComputedFeature(feature.id))
    );
}

#[test]
fn direct_fillet_set_radius_and_suppression_remain_authoritative() {
    for (key, source, expected_radius, expected_suppressed) in [
        (
            "edited-radius",
            valid_project()
                .managed
                .source
                .replacen("radius: 1,", "radius: 0.5,", 1),
            0.5_f64,
            false,
        ),
        (
            "suppressed",
            valid_project()
                .managed
                .source
                .replacen("suppressed: false", "suppressed: true", 1),
            1.0_f64,
            true,
        ),
    ] {
        let mut candidate = valid_project();
        candidate.project = ProjectKey(key.into());
        candidate.managed = parse_managed_source(&source).unwrap();
        let materialized = materialize_code_project_cold(
            &candidate,
            &reconciled(&candidate),
            IntentSessionId::from_raw(if key == "edited-radius" {
                0x84f0_0405
            } else {
                0x84f0_0406
            }),
            DocumentId(PersistentId::from_u128(if key == "edited-radius" {
                0x84f0_0405
            } else {
                0x84f0_0406
            })),
            1.0,
        )
        .unwrap();
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let [feature] = accepted.features.features() else {
            panic!("one direct FilletSet feature expected")
        };
        let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
        assert_eq!(fillet.radius.to_bits(), expected_radius.to_bits());
        assert_eq!(feature.suppressed, expected_suppressed);
        let state = &accepted.computed.feature_evaluations()[0].state;
        assert!(if expected_suppressed {
            matches!(state, ComputedFeatureEvaluationState::Suppressed)
        } else {
            matches!(state, ComputedFeatureEvaluationState::Current { .. })
        });
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
    }
}

#[test]
fn direct_fillet_set_resolves_a_typed_span_from_an_earlier_custom_patch() {
    let mut candidate = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::BracedFrame)
        .expect("braced-frame demo")
        .project();
    let insertion = r#"  const round = $.computed.filletSet("round", {
    radius: 1,
    corners: [{
      parents: [{
        span: frame.edges.bottom,
        parameter: 0.1,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }, {
        span: brace.diagonals.rising.span,
        parameter: 0.1,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "right",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: true,
  });
"#;
    let source = candidate.managed.source.replacen(
        "  const datum =",
        &format!("{insertion}  const datum ="),
        1,
    );
    assert_ne!(source, candidate.managed.source);
    candidate.managed = parse_managed_source(&source).expect("managed custom-span FilletSet");

    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0407)).unwrap();
    let expansion = expand_code_project(&candidate, &reconciled(&candidate), intent.identity())
        .expect("custom patch outputs must be available before direct computed lowering");
    let fillet = expansion
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. }
                if matches!(
                    draft.kind,
                    IntentNodeKind::ComputedFeature {
                        feature: ComputedFeatureKind::FilletSet
                    }
                ) =>
            {
                Some(draft.as_ref())
            }
            _ => None,
        })
        .expect("one direct FilletSet");
    assert_eq!(fillet.inputs.len(), 2);
    assert!(fillet.suppressed);
}

#[test]
fn direct_fillet_set_rejects_a_computed_host_arc_as_a_native_parent() {
    let mut candidate = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::TypedPanel)
        .expect("typed-panel demo")
        .project();
    let insertion = r#"  const nested = $.computed.filletSet("nested", {
    radius: 1,
    corners: [{
      parents: [{
        span: panel.edges.bottom,
        parameter: 0.1,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }, {
        span: corners.fillets.lowerLeft,
        parameter: 0.1,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "right",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: true,
  });
"#;
    let source = candidate.managed.source.replacen(
        "  return $.outputs",
        &format!("{insertion}  return $.outputs"),
        1,
    );
    assert_ne!(source, candidate.managed.source);
    candidate.managed = parse_managed_source(&source).expect("lexical computed-arc reference");

    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0408)).unwrap();
    let error = expand_code_project(&candidate, &reconciled(&candidate), intent.identity())
        .expect_err("a computed host arc cannot become a native Fillet parent");
    assert!(
        matches!(error, CodeExpansionError::Unsupported(_)),
        "unexpected rejection: {error:?}"
    );
    assert!(error.to_string().contains("Intent-backed native span"));
}

#[test]
fn direct_fillet_set_rejects_nonlexical_wrong_kind_and_malformed_state() {
    let base = valid_project().managed.source;
    let cases = [
        ("raw span", "span: first.span", "span: \"first.span\""),
        (
            "point instead of span",
            "span: first.span",
            "span: first.start",
        ),
        (
            "extra parent field",
            "span: first.span,",
            "span: first.span, extra: 1,",
        ),
        ("fractional winding", "winding: 0,", "winding: 0.5,"),
        ("bad enum", "normalSide: \"left\"", "normalSide: \"inside\""),
        (
            "missing branch field",
            "        normalSide: \"left\",\n",
            "",
        ),
        (
            "reversed local bounds",
            "neighborhood: { kind: \"interior\" }",
            "neighborhood: { kind: \"local\", lower: 1, upper: 0.5 }",
        ),
        (
            "partial anchor",
            "periodicAnchor: null",
            "periodicAnchor: { parameter: 0.5 }",
        ),
        ("angular radius", "radius: 1", "radius: deg(1)"),
        ("unsupported length unit", "radius: 1", "radius: cm(1)"),
        ("nonpositive radius", "radius: 1,", "radius: 0,"),
    ];
    for (name, from, to) in cases {
        let candidate = CodeProject {
            project: ProjectKey(name.into()),
            managed: parse_managed_source(&base.replacen(from, to, 1)).unwrap(),
            custom_files: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            lock: serde_json::json!({ "format": "geosolve-lock-v1", "modules": {} }),
        };
        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0403)).unwrap();
        assert!(
            matches!(
                expand_code_project(&candidate, &KeyedReconcileState::empty(), intent.identity()),
                Err(CodeExpansionError::Unsupported(_)
                    | CodeExpansionError::InvalidDeclaration { .. }
                    | CodeExpansionError::KindMismatch { .. }
                    | CodeExpansionError::UnresolvedReference { .. })
            ),
            "invalid direct FilletSet case `{name}` must fail closed"
        );
    }

    let one_parent = base.replacen(
        concat!(
            "      }, {\n",
            "        span: second.span,\n",
            "        parameter: 0.25,\n",
            "        winding: 0,\n",
            "        neighborhood: { kind: \"interior\" },\n",
            "        normalSide: \"left\",\n",
            "        retainedEndpoint: \"start\",\n",
            "        periodicAnchor: null,\n",
            "      }],",
        ),
        "      }],",
        1,
    );
    assert_ne!(
        one_parent, base,
        "the malformed-parent fixture must be applied"
    );
    let one_parent = project(
        "one-parent",
        one_parent
            .split_once("export default sketch(($) => {\n")
            .unwrap()
            .1
            .strip_suffix("});\n")
            .unwrap(),
    );
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84f0_0404)).unwrap();
    assert!(matches!(
        expand_code_project(
            &one_parent,
            &KeyedReconcileState::empty(),
            intent.identity()
        ),
        Err(CodeExpansionError::Unsupported(_) | CodeExpansionError::InvalidDeclaration { .. })
    ));

    let missing = base.replacen("span: first.span", "span: missing.span", 1);
    assert!(
        parse_managed_source(&missing).is_err(),
        "a missing declaration must fail during managed-source authentication"
    );
}
