// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use geosolve_constraint_editor::ComputedFeatureEvaluationState;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeHostRequest, CodeProject, CodeProjectDemoId, ExpandedCodeProject, KeyedReconcileState,
    MANAGED_CONTROL_CONSUMER_LIMIT, MANAGED_CONTROL_LIMIT, ManagedControl, ManagedControlAccess,
    ManagedControlConsumerTarget, ManagedControlEdit, ManagedControlEditBatch, ManagedControlError,
    ManagedControlManifest, ManagedControlNumberKind, ManagedControlReadOnlyReason,
    ManagedControlSchema, ManagedPathSegment, ManagedValue, MaterializedCodeProject, ProjectKey,
    SemanticOutputPath, UnitLiteral, apply_managed_control_batch,
    apply_managed_control_manifest_batch, bundled_code_project_demos, expand_code_project,
    managed_control_manifest, materialize_code_project_cold, parse_managed_source,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId, intent_content_digest};

fn demo_project(id: CodeProjectDemoId) -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == id)
        .expect("bundled demo")
        .project()
}

fn expand(
    project: &CodeProject,
    session: u128,
) -> (KeyedReconcileState, IntentSession, ExpandedCodeProject) {
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(session)).unwrap();
    let expansion = expand_code_project(project, &generated, intent.identity()).unwrap();
    (generated, intent, expansion)
}

fn materialize(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    seed: u128,
) -> MaterializedCodeProject {
    materialize_code_project_cold(
        project,
        generated,
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .unwrap_or_else(|error| panic!("cold materialization failed: {error}"))
}

fn assert_independently_valid(materialized: &MaterializedCodeProject) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold materialization owns accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    let state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("success-like materialization has exact accepted input");
    assert!(
        state
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        state
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    assert!(
        accepted
            .computed
            .feature_evaluations()
            .iter()
            .all(|evaluation| {
                matches!(
                    evaluation.state,
                    ComputedFeatureEvaluationState::Current { .. }
                )
            })
    );
}

fn control_at<'a>(
    manifest: &'a ManagedControlManifest,
    declaration: &str,
    path: &[&str],
) -> &'a ManagedControl {
    let path = SemanticOutputPath(
        path.iter()
            .map(|field| ManagedPathSegment::Field((*field).into()))
            .collect(),
    );
    manifest
        .controls
        .iter()
        .find(|control| control.source.declaration.0 == declaration && control.source.path == path)
        .unwrap_or_else(|| panic!("missing control {declaration}.{}", path_text(&path.0)))
}

fn assert_exact_generated_consumers(
    control: &ManagedControl,
    expansion: &ExpandedCodeProject,
    expected: impl Fn(&geosolve_sketch_code::GeneratedMemberAddress) -> bool,
) {
    let actual = control
        .consumers
        .iter()
        .map(|consumer| match &consumer.target {
            ManagedControlConsumerTarget::Generated {
                address, identity, ..
            } => (address.clone(), *identity),
            ManagedControlConsumerTarget::Declaration { .. } => {
                panic!("transitive artifact control must not invent a direct consumer")
            }
        })
        .collect::<BTreeSet<_>>();
    let expected = expansion
        .generated_provenance
        .iter()
        .filter(|(address, _)| expected(address))
        .map(|(address, provenance)| (address.clone(), provenance.identity))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
}

fn edited_project(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    control: &ManagedControl,
    value: f64,
) -> CodeProject {
    apply_managed_control_batch(
        project,
        expansion,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: control.token().expect("editable control").clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value,
            }),
        }]),
    )
    .unwrap()
}

fn project_with_source(project: &CodeProject, source: &str) -> CodeProject {
    let mut changed = project.clone();
    changed.managed = parse_managed_source(source).unwrap();
    changed.validate().unwrap();
    changed
}

fn host_fillet_radii(expansion: &ExpandedCodeProject) -> BTreeMap<String, Vec<u64>> {
    let mut values = BTreeMap::<String, Vec<u64>>::new();
    for request in &expansion.host_requests {
        let CodeHostRequest::FilletAtCorner(request) = request else {
            continue;
        };
        values
            .entry(request.invocation.0.clone())
            .or_default()
            .push(request.radius.value.to_bits());
    }
    for radii in values.values_mut() {
        radii.sort_unstable();
    }
    values
}

fn path_text(path: &[ManagedPathSegment]) -> String {
    path.iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => index.to_string(),
            ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn m87_f001_typed_panel_shared_radius_has_one_source_and_complete_fillet_fan_out() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (generated, intent, expanded) = expand(&project, 0x87_0001);

    let radii = expanded
        .host_requests
        .iter()
        .filter_map(|request| match request {
            CodeHostRequest::FilletAtCorner(request) => Some(request.radius.value.to_bits()),
            CodeHostRequest::RoundedRectangleProfile { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(radii, vec![4.0_f64.to_bits(), 4.0_f64.to_bits()]);

    let artifact_lenses = project
        .artifacts
        .values()
        .next()
        .and_then(|value| value.get("edit_lenses"))
        .and_then(serde_json::Value::as_array)
        .expect("Typed Panel artifact edit lenses");
    assert!(artifact_lenses.is_empty(), "EditLens is not edit authority");

    let manifest = managed_control_manifest(&project, &expanded).unwrap();
    let radius = manifest
        .editable()
        .find(|control| {
            matches!(
                &control.value,
                ManagedValue::Unit(UnitLiteral { unit, value })
                    if unit == "mm" && value.to_bits() == 4.0_f64.to_bits()
            )
        })
        .expect("shared radius control");
    assert_eq!(radius.source.source_text, "mm(4)");
    assert_eq!(radius.consumers.len(), 2);
    assert!(radius.consumers.iter().all(|consumer| {
        consumer.property == SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())])
            && matches!(
                &consumer.target,
                ManagedControlConsumerTarget::Generated { family, .. }
                    if family == "computed.fillet"
            )
    }));

    let radius_property = SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]);
    for request in &expanded.host_requests {
        let CodeHostRequest::FilletAtCorner(request) = request else {
            continue;
        };
        let routed = manifest
            .controls_for_generated(&request.output, request.identity, &radius_property)
            .collect::<Vec<_>>();
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].id, radius.id);
    }

    let edited = apply_managed_control_batch(
        &project,
        &expanded,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: radius.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 2.5,
            }),
        }]),
    )
    .unwrap();
    assert_eq!(edited.managed.source.matches("radius: mm(2.5)").count(), 1);
    assert_eq!(edited.artifacts, project.artifacts);
    assert_eq!(edited.custom_files, project.custom_files);

    let reexpanded = expand_code_project(&edited, &generated, intent.identity()).unwrap();
    let edited_radii = reexpanded
        .host_requests
        .iter()
        .filter_map(|request| match request {
            CodeHostRequest::FilletAtCorner(request) => Some(request.radius.value.to_bits()),
            CodeHostRequest::RoundedRectangleProfile { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(edited_radii, vec![2.5_f64.to_bits(), 2.5_f64.to_bits()]);
}

#[test]
fn tokens_authenticate_source_project_expected_value_and_generated_generation() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (generated, intent, expansion) = expand(&project, 0x87_0002);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let radius = control_at(&manifest, "cornerFillets", &["radius"]);
    let token = radius.token().unwrap().clone();

    assert_eq!(
        managed_control_manifest(&project, &expansion).unwrap(),
        manifest
    );

    let replacement = ManagedValue::Unit(UnitLiteral {
        unit: "mm".into(),
        value: 3.0,
    });
    let edit = |token| {
        ManagedControlEditBatch::new([ManagedControlEdit {
            token,
            value: replacement.clone(),
        }])
    };

    let mut wrong_project = token.clone();
    wrong_project.project.0.push_str("-foreign");
    assert!(matches!(
        apply_managed_control_batch(&project, &expansion, &edit(wrong_project)),
        Err(ManagedControlError::StaleToken(_))
    ));

    let mut wrong_expected = token.clone();
    wrong_expected.expected = ManagedValue::Unit(UnitLiteral {
        unit: "mm".into(),
        value: 5.0,
    });
    assert!(matches!(
        apply_managed_control_batch(&project, &expansion, &edit(wrong_expected)),
        Err(ManagedControlError::StaleToken(_))
    ));

    let mut unknown = token.clone();
    unknown.id.0 = "0".repeat(64);
    assert!(matches!(
        apply_managed_control_batch(&project, &expansion, &edit(unknown)),
        Err(ManagedControlError::UnknownControl(_))
    ));

    let edited = apply_managed_control_batch(&project, &expansion, &edit(token.clone())).unwrap();
    let edited_expansion = expand_code_project(&edited, &generated, intent.identity()).unwrap();
    let edited_manifest = managed_control_manifest(&edited, &edited_expansion).unwrap();
    let edited_radius = control_at(&edited_manifest, "cornerFillets", &["radius"]);
    assert_eq!(
        edited_radius.id, radius.id,
        "semantic ID must survive source edits"
    );
    assert_ne!(
        edited_radius.token().unwrap().authentication,
        token.authentication,
        "source CAS evidence must advance"
    );
    assert!(matches!(
        apply_managed_control_batch(&edited, &edited_expansion, &edit(token.clone())),
        Err(ManagedControlError::StaleToken(_))
    ));
    assert!(matches!(
        apply_managed_control_manifest_batch(&project, &expansion, &edited_manifest, &edit(token)),
        Err(ManagedControlError::StaleToken(_))
    ));

    let removed = generated
        .plan(Vec::new(), &BTreeSet::new())
        .unwrap()
        .into_staged();
    let regenerated = removed
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let regenerated_expansion =
        expand_code_project(&project, &regenerated, intent.identity()).unwrap();
    let regenerated_manifest = managed_control_manifest(&project, &regenerated_expansion).unwrap();
    let regenerated_radius = control_at(&regenerated_manifest, "cornerFillets", &["radius"]);
    assert_eq!(regenerated_radius.id, radius.id);
    assert_ne!(
        regenerated_radius.token().unwrap().generation_digest,
        radius.token().unwrap().generation_digest
    );
    assert!(matches!(
        apply_managed_control_batch(
            &project,
            &regenerated_expansion,
            &edit(radius.token().unwrap().clone())
        ),
        Err(ManagedControlError::StaleToken(_))
    ));
}

#[test]
fn exact_unit_schema_and_positive_domain_reject_before_rewrite() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, expansion) = expand(&project, 0x87_0003);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let radius = control_at(&manifest, "cornerFillets", &["radius"]);
    let source = project.managed.source.clone();

    for value in [
        ManagedValue::Number(2.0),
        ManagedValue::Unit(UnitLiteral {
            unit: "cm".into(),
            value: 2.0,
        }),
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 0.0,
        }),
    ] {
        let result = apply_managed_control_batch(
            &project,
            &expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: radius.token().unwrap().clone(),
                value,
            }]),
        );
        assert!(matches!(
            result,
            Err(ManagedControlError::InvalidReplacement { .. })
        ));
        assert_eq!(project.managed.source, source);
    }
}

#[test]
fn direct_boolean_choice_and_signed_zero_schemas_rewrite_exact_typed_values() {
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const axisLine = $.geometry.line("axisLine", {
    start: [0, 0],
    end: [10, 0],
    role: "construction",
  });
  const guard = $.constraint.fixedCoordinate("guard", {
    point: axisLine.start,
    axis: "x",
    target: mm(-0),
    suppressed: false,
  });
  return $.outputs({ axisLine, guard });
});
"#;
    let project =
        CodeProject::managed_only(ProjectKey("m87-direct-schema".into()), SOURCE).unwrap();
    let (_, _, expansion) = expand(&project, 0x87_0008);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let role = control_at(&manifest, "axisLine", &["role"]);
    let axis = control_at(&manifest, "guard", &["axis"]);
    let target = control_at(&manifest, "guard", &["target"]);
    let suppressed = control_at(&manifest, "guard", &["suppressed"]);

    assert!(matches!(
        role.schema,
        Some(ManagedControlSchema::Choice { .. })
    ));
    assert!(matches!(
        axis.schema,
        Some(ManagedControlSchema::Choice { .. })
    ));
    assert!(matches!(
        suppressed.schema,
        Some(ManagedControlSchema::Boolean)
    ));
    assert!(matches!(
        &target.value,
        ManagedValue::Unit(UnitLiteral { unit, value })
            if unit == "mm" && value.to_bits() == (-0.0_f64).to_bits()
    ));

    let invalid_axis = apply_managed_control_batch(
        &project,
        &expansion,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: axis.token().unwrap().clone(),
            value: ManagedValue::String("z".into()),
        }]),
    );
    assert!(matches!(
        invalid_axis,
        Err(ManagedControlError::InvalidReplacement { .. })
    ));

    let edited = apply_managed_control_batch(
        &project,
        &expansion,
        &ManagedControlEditBatch::new([
            ManagedControlEdit {
                token: suppressed.token().unwrap().clone(),
                value: ManagedValue::Bool(true),
            },
            ManagedControlEdit {
                token: target.token().unwrap().clone(),
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 0.0,
                }),
            },
            ManagedControlEdit {
                token: axis.token().unwrap().clone(),
                value: ManagedValue::String("y".into()),
            },
            ManagedControlEdit {
                token: role.token().unwrap().clone(),
                value: ManagedValue::String("profile".into()),
            },
        ]),
    )
    .unwrap();
    assert!(edited.managed.source.contains("role: \"profile\""));
    assert!(edited.managed.source.contains("axis: \"y\""));
    assert!(edited.managed.source.contains("target: mm(0)"));
    assert!(edited.managed.source.contains("suppressed: true"));
}

#[test]
fn unordered_multi_edit_batch_is_atomic_and_reparses_only_the_complete_candidate() {
    let project = demo_project(CodeProjectDemoId::MountingPlate);
    let (_, _, expansion) = expand(&project, 0x87_0004);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let width = control_at(&manifest, "plate", &["width"]);
    let height = control_at(&manifest, "plate", &["height"]);
    let before = project.to_canonical_json().unwrap();

    assert_eq!(
        apply_managed_control_batch(&project, &expansion, &ManagedControlEditBatch::default()),
        Err(ManagedControlError::EmptyBatch)
    );

    let invalid = ManagedControlEditBatch::new([
        ManagedControlEdit {
            token: height.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "cm".into(),
                value: 60.0,
            }),
        },
        ManagedControlEdit {
            token: width.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 100.0,
            }),
        },
    ]);
    assert!(matches!(
        apply_managed_control_batch(&project, &expansion, &invalid),
        Err(ManagedControlError::InvalidReplacement { .. })
    ));
    assert_eq!(project.to_canonical_json().unwrap(), before);

    let duplicate = ManagedControlEditBatch::new([
        ManagedControlEdit {
            token: width.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 100.0,
            }),
        },
        ManagedControlEdit {
            token: width.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 101.0,
            }),
        },
    ]);
    assert!(matches!(
        apply_managed_control_batch(&project, &expansion, &duplicate),
        Err(ManagedControlError::DuplicateControl(_))
    ));

    let edited = apply_managed_control_batch(
        &project,
        &expansion,
        &ManagedControlEditBatch::new([
            ManagedControlEdit {
                token: height.token().unwrap().clone(),
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 60.0,
                }),
            },
            ManagedControlEdit {
                token: width.token().unwrap().clone(),
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 100.0,
                }),
            },
        ]),
    )
    .unwrap();
    assert!(edited.managed.source.contains("width: mm(100)"));
    assert!(edited.managed.source.contains("height: mm(60)"));
    assert_eq!(edited.managed.source.matches("width: mm(100)").count(), 1);
    assert_eq!(edited.managed.source.matches("height: mm(60)").count(), 1);
}

#[test]
fn references_structure_identities_and_solver_instance_values_are_explicitly_read_only() {
    let typed = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, typed_expansion) = expand(&typed, 0x87_0005);
    let typed_manifest = managed_control_manifest(&typed, &typed_expansion).unwrap();

    let corners = control_at(&typed_manifest, "cornerFillets", &["corners"]);
    assert!(matches!(
        corners.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::Structure,
            ..
        }
    ));
    let lower_left = control_at(&typed_manifest, "cornerFillets", &["corners", "lowerLeft"]);
    assert!(matches!(
        &lower_left.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::Reference,
            navigation: Some(navigation),
        } if navigation.declaration.0 == "panel"
            && navigation.path
                == SemanticOutputPath(vec![
                    ManagedPathSegment::Field("corners".into()),
                    ManagedPathSegment::Field("lowerLeft".into()),
                ])
    ));
    let panel_x = typed_manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration.0 == "panel"
                && control.source.path.0
                    == [
                        ManagedPathSegment::Field("lowerLeft".into()),
                        ManagedPathSegment::Index(0),
                    ]
        })
        .unwrap();
    assert!(matches!(
        panel_x.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::SolverInstance,
            ..
        }
    ));

    let rounded = demo_project(CodeProjectDemoId::RoundedPolyline);
    let (_, _, rounded_expansion) = expand(&rounded, 0x87_0006);
    let rounded_manifest = managed_control_manifest(&rounded, &rounded_expansion).unwrap();
    let vertex_key = rounded_manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration.0 == "path"
                && control.source.path.0.last().is_some_and(
                    |segment| matches!(segment, ManagedPathSegment::Field(field) if field == "key"),
                )
        })
        .unwrap();
    assert!(matches!(
        vertex_key.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::StructuralIdentity,
            ..
        }
    ));

    let fabricated = ManagedControlEditBatch::new([ManagedControlEdit {
        token: {
            let mut token = control_at(&typed_manifest, "cornerFillets", &["radius"])
                .token()
                .unwrap()
                .clone();
            token.id = corners.id.clone();
            token
        },
        value: ManagedValue::Number(1.0),
    }]);
    assert!(matches!(
        apply_managed_control_batch(&typed, &typed_expansion, &fabricated),
        Err(ManagedControlError::ReadOnlyControl(_))
    ));
}

#[test]
fn explicit_null_is_not_misclassified_as_an_absent_source_member() {
    let project = demo_project(CodeProjectDemoId::NeonManifold);
    let (_, _, expansion) = expand(&project, 0x87_0009);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let explicit_null = manifest
        .controls
        .iter()
        .find(|control| matches!(control.value, ManagedValue::Null))
        .expect("explicit periodicAnchor null");
    assert!(matches!(
        explicit_null.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::Null,
            navigation: None,
        }
    ));
    assert!(
        manifest.controls.iter().all(|control| !matches!(
            control.access,
            ManagedControlAccess::ReadOnly {
                reason: ManagedControlReadOnlyReason::Absent,
                ..
            }
        )),
        "an absent member has no parser-owned span and must not acquire a fabricated row"
    );
}

#[test]
fn manifest_derivation_is_transient_and_does_not_change_project_or_expansion_wire_bytes() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, expansion) = expand(&project, 0x87_0007);
    let project_before = project.to_canonical_json().unwrap();
    let expansion_before = serde_json::to_vec(&expansion).unwrap();

    let first = managed_control_manifest(&project, &expansion).unwrap();
    let second = managed_control_manifest(&project, &expansion).unwrap();
    assert_eq!(first, second);
    assert_eq!(project.to_canonical_json().unwrap(), project_before);
    assert_eq!(serde_json::to_vec(&expansion).unwrap(), expansion_before);
    let restored_project = CodeProject::from_json(&project_before).unwrap();
    let restored_expansion: ExpandedCodeProject =
        serde_json::from_slice(&expansion_before).unwrap();
    assert_eq!(
        managed_control_manifest(&restored_project, &restored_expansion).unwrap(),
        first,
        "transient controls and tokens must reconstruct after reload"
    );
    assert!(!project_before.contains("ManagedControl"));
    assert!(
        !String::from_utf8(expansion_before)
            .unwrap()
            .contains("managed_control")
    );
}

#[test]
fn all_bundled_definition_leaves_are_typed_or_deliberately_read_only() {
    for (index, demo) in bundled_code_project_demos().into_iter().enumerate() {
        let project = demo.project();
        let (_, _, expansion) = expand(&project, 0x87_1000 + index as u128);
        let manifest = managed_control_manifest(&project, &expansion).unwrap();
        assert!(!manifest.controls.is_empty(), "{} manifest", demo.id.key());

        let edits = manifest
            .controls
            .iter()
            .filter_map(|control| match (&control.value, &control.access) {
                (
                    ManagedValue::Bool(_)
                    | ManagedValue::Number(_)
                    | ManagedValue::String(_)
                    | ManagedValue::Unit(_),
                    ManagedControlAccess::Editable { token },
                ) => {
                    assert!(
                        control.schema.is_some(),
                        "{} {} schema",
                        demo.id.key(),
                        control.id.0
                    );
                    Some(ManagedControlEdit {
                        token: token.clone(),
                        value: control.value.clone(),
                    })
                }
                (
                    ManagedValue::Bool(_)
                    | ManagedValue::Number(_)
                    | ManagedValue::String(_)
                    | ManagedValue::Unit(_),
                    ManagedControlAccess::ReadOnly { reason, .. },
                ) => {
                    assert!(
                        matches!(
                            reason,
                            ManagedControlReadOnlyReason::StructuralIdentity
                                | ManagedControlReadOnlyReason::SolverInstance
                        ),
                        "{} scalar leaf {} was unexpectedly {reason:?}",
                        demo.id.key(),
                        control.id.0
                    );
                    None
                }
                (ManagedValue::Array(_) | ManagedValue::Object(_), access) => {
                    assert!(matches!(
                        access,
                        ManagedControlAccess::ReadOnly {
                            reason: ManagedControlReadOnlyReason::Structure,
                            ..
                        }
                    ));
                    None
                }
                (ManagedValue::Reference { .. }, access) => {
                    assert!(matches!(
                        access,
                        ManagedControlAccess::ReadOnly {
                            reason: ManagedControlReadOnlyReason::Reference,
                            navigation: Some(_),
                        }
                    ));
                    None
                }
                (ManagedValue::Null, access) => {
                    assert!(matches!(
                        access,
                        ManagedControlAccess::ReadOnly {
                            reason: ManagedControlReadOnlyReason::Null,
                            ..
                        }
                    ));
                    None
                }
            })
            .collect::<Vec<_>>();
        if edits.is_empty() {
            continue;
        }
        let reparsed =
            apply_managed_control_batch(&project, &expansion, &ManagedControlEditBatch::new(edits))
                .unwrap();
        assert_eq!(reparsed.managed.program, project.managed.program);
    }
}

#[test]
fn direct_fillet_winding_is_a_bounded_signed_integer_control() {
    let project = demo_project(CodeProjectDemoId::NeonManifold);
    let (_, _, expansion) = expand(&project, 0x87_5000);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let winding = manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration.0 == "bends"
                && matches!(
                    control.source.path.0.last(),
                    Some(ManagedPathSegment::Field(field)) if field == "winding"
                )
        })
        .expect("FilletSet winding control");
    assert!(matches!(
        winding.schema,
        Some(ManagedControlSchema::Number {
            number: ManagedControlNumberKind::Integer,
            minimum: Some(minimum),
            maximum: Some(maximum),
        }) if minimum.inclusive
            && minimum.value.to_bits() == f64::from(i32::MIN).to_bits()
            && maximum.inclusive
            && maximum.value.to_bits() == f64::from(i32::MAX).to_bits()
    ));

    for value in [
        ManagedValue::Number(0.5),
        ManagedValue::Number(f64::from(i32::MAX) + 1.0),
    ] {
        assert!(matches!(
            apply_managed_control_batch(
                &project,
                &expansion,
                &ManagedControlEditBatch::new([ManagedControlEdit {
                    token: winding.token().unwrap().clone(),
                    value,
                }])
            ),
            Err(ManagedControlError::InvalidReplacement { .. })
        ));
    }

    let edited = apply_managed_control_batch(
        &project,
        &expansion,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: winding.token().unwrap().clone(),
            value: ManagedValue::Number(-2.0),
        }]),
    )
    .unwrap();
    assert!(edited.managed.source.contains("winding: -2"));
}

#[test]
fn manifest_control_limit_is_independent_of_the_managed_parser_node_limit() {
    let declaration_count = 1_200;
    let mut source = String::from(
        "\"use geosolve managed-v1\";\nimport { sketch } from \"@geosolve/sketch-code\";\n\nexport default sketch(($) => {\n",
    );
    for index in 0..declaration_count {
        writeln!(
            source,
            "  const line{index} = $.geometry.line(\"line{index}\", {{ start: [0, {index}], end: [1, {index}] }});"
        )
        .unwrap();
    }
    source.push_str("  return $.outputs({");
    for index in 0..declaration_count {
        write!(source, "line{index},").unwrap();
    }
    source.push_str("});\n});\n");

    let project = CodeProject::managed_only(ProjectKey("m87-manifest-limit".into()), &source)
        .expect("source remains below the independently larger parser limits");
    assert!(project.managed.value_owned_spans.len() > MANAGED_CONTROL_LIMIT);
    let (_, _, expansion) = expand(&project, 0x87_6000);
    assert_eq!(
        managed_control_manifest(&project, &expansion),
        Err(ManagedControlError::ResourceLimit {
            resource: "manifest controls",
            actual: MANAGED_CONTROL_LIMIT + 1,
            limit: MANAGED_CONTROL_LIMIT,
        })
    );
}

#[test]
fn consumer_edge_limit_is_independent_of_manifest_control_count() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, mut expansion) = expand(&project, 0x87_7000);
    let (prototype_address, prototype_provenance) = expansion
        .generated_provenance
        .iter()
        .next()
        .map(|(address, provenance)| (address.clone(), provenance.clone()))
        .expect("Typed Panel generated provenance");
    expansion.generated_provenance.clear();
    for index in 0..=MANAGED_CONTROL_CONSUMER_LIMIT {
        let mut address = prototype_address.clone();
        address.member_key = vec![format!("fanout-{index:05}")];
        expansion
            .generated_provenance
            .insert(address, prototype_provenance.clone());
    }
    refresh_expansion_digest(&mut expansion);

    assert_eq!(
        managed_control_manifest(&project, &expansion),
        Err(ManagedControlError::ResourceLimit {
            resource: "source-consumer edges",
            actual: MANAGED_CONTROL_CONSUMER_LIMIT + 1,
            limit: MANAGED_CONTROL_CONSUMER_LIMIT,
        })
    );
}

fn refresh_expansion_digest(expansion: &mut ExpandedCodeProject) {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
    ))
    .unwrap();
    expansion.digest = intent_content_digest(&bytes).to_string();
}

#[test]
fn bundled_patch_fan_out_is_complete_and_invocation_local() {
    let cases = [
        (CodeProjectDemoId::TypedPanel, "cornerFillets", "radius", 2),
        (CodeProjectDemoId::MountingPlate, "plate", "width", 5),
        (CodeProjectDemoId::MountingPlate, "plate", "height", 5),
        (CodeProjectDemoId::MountingPlate, "plate", "cornerRadius", 5),
        (CodeProjectDemoId::MountingPlate, "plate", "holeRadius", 4),
        (
            CodeProjectDemoId::AdaptiveLanterns,
            "decorations",
            "bulbRadius",
            7,
        ),
        (
            CodeProjectDemoId::AdaptiveLanterns,
            "decorations",
            "bendRadius",
            5,
        ),
    ];
    for (index, (demo, declaration, field, expected_consumers)) in cases.into_iter().enumerate() {
        let project = demo_project(demo);
        let (_, _, expansion) = expand(&project, 0x87_2000 + index as u128);
        let manifest = managed_control_manifest(&project, &expansion).unwrap();
        let control = control_at(&manifest, declaration, &[field]);
        assert!(control.token().is_some(), "{declaration}.{field}");
        assert_eq!(
            control.consumers.len(),
            expected_consumers,
            "{declaration}.{field}"
        );
        assert!(control.consumers.iter().all(|consumer| matches!(
            &consumer.target,
            ManagedControlConsumerTarget::Generated { address, .. }
                if address.invocation == declaration
        )));
    }
}

#[test]
fn mounting_plate_and_adaptive_lantern_controls_apply_real_validated_edits() {
    let cases = [
        (
            CodeProjectDemoId::MountingPlate,
            "plate",
            vec![
                ("width", 92.0, vec!["profile"]),
                ("height", 57.0, vec!["profile"]),
                ("cornerRadius", 6.0, vec!["profile"]),
                ("holeRadius", 2.25, vec!["holes"]),
            ],
        ),
        (
            CodeProjectDemoId::AdaptiveLanterns,
            "decorations",
            vec![
                ("bulbRadius", 2.4, vec!["circle"]),
                ("bendRadius", 2.8, vec!["fillet"]),
            ],
        ),
    ];

    for (ordinal, (demo, invocation, edits)) in cases.into_iter().enumerate() {
        let project = demo_project(demo);
        let custom_before = project.custom_files.clone();
        let artifacts_before = project.artifacts.clone();
        let (generated, _, expansion) = expand(&project, 0x87_c500 + ordinal as u128);
        let manifest = managed_control_manifest(&project, &expansion).unwrap();
        let batch = edits
            .iter()
            .map(|(field, value, templates)| {
                let control = control_at(&manifest, invocation, &[*field]);
                assert_exact_generated_consumers(control, &expansion, |address| {
                    address.invocation == invocation
                        && address
                            .template
                            .first()
                            .is_some_and(|template| templates.contains(&template.as_str()))
                });
                ManagedControlEdit {
                    token: control.token().expect("artifact input is editable").clone(),
                    value: ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: *value,
                    }),
                }
            })
            .collect::<Vec<_>>();
        let edited =
            apply_managed_control_batch(&project, &expansion, &ManagedControlEditBatch::new(batch))
                .unwrap();
        assert_eq!(edited.custom_files, custom_before);
        assert_eq!(edited.artifacts, artifacts_before);
        for (field, value, _) in &edits {
            assert!(
                edited
                    .managed
                    .source
                    .contains(&format!("{field}: mm({value})"))
            );
        }

        let reloaded = CodeProject::from_json(&edited.to_canonical_json().unwrap()).unwrap();
        assert_eq!(reloaded, edited);
        assert_eq!(reloaded.custom_files, custom_before);
        assert_eq!(reloaded.artifacts, artifacts_before);
        let materialized = materialize(&reloaded, &generated, 0x87_c510 + ordinal as u128);
        assert_independently_valid(&materialized);
        let after_manifest = managed_control_manifest(&reloaded, &materialized.expansion).unwrap();
        for (field, _, templates) in edits {
            let control = control_at(&after_manifest, invocation, &[field]);
            assert_exact_generated_consumers(control, &materialized.expansion, |address| {
                address.invocation == invocation
                    && address
                        .template
                        .first()
                        .is_some_and(|template| templates.contains(&template.as_str()))
            });
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one crossover regression keeps the shared and three invocation-local routes adjacent"
)]
fn water_manifold_shared_binding_and_each_invocation_route_are_exact() {
    let project = demo_project(CodeProjectDemoId::PcWaterManifold);
    let custom_before = project.custom_files.clone();
    let artifacts_before = project.artifacts.clone();
    let (generated, intent, expansion) = expand(&project, 0x87_c520);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let shared = control_at(&manifest, "channelBendRadius", &[]);
    assert_eq!(shared.source.source_text, "mm(5)");
    assert_eq!(project.managed.source.matches("mm(5)").count(), 9);
    assert_eq!(
        project
            .managed
            .source
            .matches("const channelBendRadius = mm(5);")
            .count(),
        1
    );
    assert_exact_generated_consumers(shared, &expansion, |address| {
        matches!(
            address.invocation.as_str(),
            "upperChannelBends" | "middleChannelBends" | "lowerChannelBends"
        ) && address.template == ["fillet"]
    });
    assert_eq!(shared.consumers.len(), 6);
    for invocation in [
        "upperChannelBends",
        "middleChannelBends",
        "lowerChannelBends",
    ] {
        let reference = control_at(&manifest, invocation, &["bendRadius"]);
        assert!(reference.consumers.is_empty());
        assert!(matches!(
            &reference.access,
            ManagedControlAccess::ReadOnly {
                reason: ManagedControlReadOnlyReason::Reference,
                navigation: Some(navigation),
            } if navigation.declaration.0 == "channelBendRadius"
                && navigation.path.0.is_empty()
        ));
    }

    let shared_token = shared.token().unwrap().clone();
    let edited = edited_project(&project, &expansion, shared, 4.0);
    assert_eq!(edited.custom_files, custom_before);
    assert_eq!(edited.artifacts, artifacts_before);
    assert_eq!(
        edited
            .managed
            .source
            .matches("const channelBendRadius = mm(4);")
            .count(),
        1
    );
    let reloaded = CodeProject::from_json(&edited.to_canonical_json().unwrap()).unwrap();
    let shared_materialized = materialize(&reloaded, &generated, 0x87_c521);
    assert_independently_valid(&shared_materialized);
    let radii = host_fillet_radii(&shared_materialized.expansion);
    for invocation in [
        "upperChannelBends",
        "middleChannelBends",
        "lowerChannelBends",
    ] {
        assert_eq!(radii[invocation], vec![4.0_f64.to_bits(); 2]);
    }
    assert!(matches!(
        apply_managed_control_batch(
            &reloaded,
            &shared_materialized.expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: shared_token,
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 3.5,
                }),
            }]),
        ),
        Err(ManagedControlError::StaleToken(_))
    ));

    let needle = "bendRadius: channelBendRadius";
    for (ordinal, invocation) in [
        "upperChannelBends",
        "middleChannelBends",
        "lowerChannelBends",
    ]
    .into_iter()
    .enumerate()
    {
        let mut matches = project
            .managed
            .source
            .match_indices(needle)
            .map(|(start, _)| start)
            .collect::<Vec<_>>();
        let start = matches.remove(ordinal);
        let mut source = project.managed.source.clone();
        source.replace_range(start..start + needle.len(), "bendRadius: mm(4.5)");
        let local = project_with_source(&project, &source);
        assert_eq!(local.custom_files, custom_before);
        assert_eq!(local.artifacts, artifacts_before);
        let (local_generated, _, local_expansion) = expand(&local, 0x87_c530 + ordinal as u128);
        let local_manifest = managed_control_manifest(&local, &local_expansion).unwrap();
        let local_control = control_at(&local_manifest, invocation, &["bendRadius"]);
        assert_exact_generated_consumers(local_control, &local_expansion, |address| {
            address.invocation == invocation && address.template == ["fillet"]
        });
        assert_eq!(local_control.consumers.len(), 2);
        let remaining_shared = control_at(&local_manifest, "channelBendRadius", &[]);
        assert_eq!(remaining_shared.consumers.len(), 4);
        assert!(remaining_shared.consumers.iter().all(|consumer| matches!(
            &consumer.target,
            ManagedControlConsumerTarget::Generated { address, .. }
                if address.invocation != invocation
        )));

        let local_edited = edited_project(&local, &local_expansion, local_control, 4.0);
        let local_materialized =
            materialize(&local_edited, &local_generated, 0x87_c540 + ordinal as u128);
        assert_independently_valid(&local_materialized);
        let radii = host_fillet_radii(&local_materialized.expansion);
        for candidate in [
            "upperChannelBends",
            "middleChannelBends",
            "lowerChannelBends",
        ] {
            let expected: f64 = if candidate == invocation { 4.0 } else { 5.0 };
            assert_eq!(radii[candidate], vec![expected.to_bits(); 2]);
        }
        assert_eq!(local_edited.custom_files, custom_before);
        assert_eq!(local_edited.artifacts, artifacts_before);
    }

    let reexpanded = expand_code_project(&project, &generated, intent.identity()).unwrap();
    assert_eq!(reexpanded, expansion);
}

#[test]
fn lexical_scalar_binding_rejects_wrong_kind_custom_patch_input() {
    let project = demo_project(CodeProjectDemoId::PcWaterManifold);
    let original = concat!(
        "corners: upperCenterline.filletableCorners, ",
        "bendRadius: channelBendRadius"
    );
    assert_eq!(project.managed.source.matches(original).count(), 1);
    let wrong_source = project.managed.source.replacen(
        original,
        "corners: channelBendRadius, bendRadius: channelBendRadius",
        1,
    );
    let wrong_kind = project_with_source(&project, &wrong_source);

    assert!(matches!(
        required_generated_members(&wrong_kind),
        Err(geosolve_sketch_code::CodeExpansionError::KindMismatch {
            reference,
            expected: geosolve_sketch_code::FeatureKind::Collection,
            actual: geosolve_sketch_code::FeatureKind::Scalar,
        }) if reference == "upperChannelBends.corners"
    ));
}

#[test]
fn adaptive_key_removal_reorder_and_reinsertion_advance_only_retired_generations() {
    let project = demo_project(CodeProjectDemoId::AdaptiveLanterns);
    let custom_before = project.custom_files.clone();
    let artifacts_before = project.artifacts.clone();
    let initial_members = required_generated_members(&project).unwrap();
    let initial = KeyedReconcileState::empty()
        .plan(initial_members.clone(), &BTreeSet::new())
        .unwrap()
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x87_c550)).unwrap();
    let expansion = expand_code_project(&project, &initial, intent.identity()).unwrap();
    let token = control_at(
        &managed_control_manifest(&project, &expansion).unwrap(),
        "decorations",
        &["bendRadius"],
    )
    .token()
    .unwrap()
    .clone();

    let gold_line = "      { key: \"gold\", position: [0, 18] },\n";
    assert_eq!(project.managed.source.matches(gold_line).count(), 1);
    let removed_source = project.managed.source.replace(gold_line, "");
    let removed_project = project_with_source(&project, &removed_source);
    let removed_members = required_generated_members(&removed_project).unwrap();
    let removal = initial.plan(removed_members, &BTreeSet::new()).unwrap();
    assert!(!removal.removed.is_empty());
    let retired = removal
        .removed
        .iter()
        .map(|member| (member.address.clone(), member.identity))
        .collect::<BTreeMap<_, _>>();
    let retained_before = removal
        .retained
        .iter()
        .map(|member| (member.address.clone(), member.identity))
        .collect::<BTreeMap<_, _>>();
    let removed = removal.into_staged();
    let removed_materialized = materialize(&removed_project, &removed, 0x87_c551);
    assert_independently_valid(&removed_materialized);
    assert_eq!(removed_project.custom_files, custom_before);
    assert_eq!(removed_project.artifacts, artifacts_before);

    let mut reinserted_members = initial_members;
    reinserted_members.rotate_left(7);
    let reinsertion = removed.plan(reinserted_members, &BTreeSet::new()).unwrap();
    assert!(reinsertion.reordered);
    let reinserted = reinsertion.staged().clone();
    for (address, identity) in &retained_before {
        assert_eq!(reinserted.active().get(address), Some(identity));
    }
    for (address, old_identity) in &retired {
        assert_eq!(reinserted.tombstones().get(address), Some(old_identity));
        let new_identity = reinserted.active()[address];
        assert!(new_identity.allocation > old_identity.allocation);
        assert_eq!(new_identity.generation, old_identity.generation + 1);
    }
    let reinserted_materialized = materialize(&project, &reinserted, 0x87_c552);
    assert_independently_valid(&reinserted_materialized);
    assert_eq!(project.custom_files, custom_before);
    assert_eq!(project.artifacts, artifacts_before);
    let reinserted_manifest =
        managed_control_manifest(&project, &reinserted_materialized.expansion).unwrap();
    let new_control = control_at(&reinserted_manifest, "decorations", &["bendRadius"]);
    assert_ne!(
        new_control.token().unwrap().generation_digest,
        token.generation_digest
    );
    assert!(matches!(
        apply_managed_control_batch(
            &project,
            &reinserted_materialized.expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token,
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 2.8,
                }),
            }]),
        ),
        Err(ManagedControlError::StaleToken(_))
    ));
}

#[test]
fn expansion_digest_is_validated_before_manifest_authority_is_issued() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, expansion) = expand(&project, 0x87_3000);

    let mut forged_contents = expansion.clone();
    forged_contents.host_requests.clear();
    assert!(matches!(
        managed_control_manifest(&project, &forged_contents),
        Err(ManagedControlError::ForeignExpansion(_))
    ));

    let mut malformed_digest = expansion;
    malformed_digest.digest = "not-a-digest".into();
    assert!(matches!(
        managed_control_manifest(&project, &malformed_digest),
        Err(ManagedControlError::ForeignExpansion(_))
    ));
}

#[test]
fn oversized_batch_rejects_at_its_independent_bound_before_source_rewrite() {
    let project = demo_project(CodeProjectDemoId::TypedPanel);
    let (_, _, expansion) = expand(&project, 0x87_4000);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let radius = control_at(&manifest, "cornerFillets", &["radius"]);
    let edit = ManagedControlEdit {
        token: radius.token().unwrap().clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 3.0,
        }),
    };
    let batch = ManagedControlEditBatch {
        edits: vec![edit; MANAGED_CONTROL_LIMIT + 1],
    };
    let source_before = project.managed.source.clone();

    assert_eq!(
        apply_managed_control_batch(&project, &expansion, &batch),
        Err(ManagedControlError::ResourceLimit {
            resource: "control batch edits",
            actual: MANAGED_CONTROL_LIMIT + 1,
            limit: MANAGED_CONTROL_LIMIT,
        })
    );
    assert_eq!(project.managed.source, source_before);
}
