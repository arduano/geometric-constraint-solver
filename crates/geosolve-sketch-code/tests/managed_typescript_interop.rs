// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{ComputedFeatureDefinition, IntentNativeBinding};
use geosolve_sketch::{DocumentConstraintDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeInteractionOverlay, CodeOwnerAddress, CodePointEdit, CodePointSeedSource, CodeProject,
    CompiledManagedSource, ExecutedConsumerTarget, ExecutedDeclarationResult,
    ExecutedGeneratedMember, ExecutedGroup, ExecutedSketchArtifact, ExecutedSuppression,
    ExecutedValueConsumer, FeatureKind, KeyedReconcileState, ManagedControl, ManagedControlAccess,
    ManagedControlNumberKind, ManagedControlReadOnlyReason, ManagedControlSchema,
    ManagedExpression, ManagedIrImport, ManagedPathSegment, ManagedSketchIr,
    ManagedSourceDeclarationClosure, ManagedSourceDeclarationClosureKind, ManagedSourceSite,
    ManagedStatement, ManagedValue, ProjectKey, SemanticOutputPath, SemanticSymbol, UnitLiteral,
    derive_managed_value_mutation, managed_control_manifest, managed_point_value_mutations,
    materialize_code_project_cold, materialize_code_project_cold_with_overlay,
    required_generated_members,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentNodeKind, IntentPortRole, IntentPortSelector, IntentSessionId,
    intent_content_digest,
};
use serde::Serialize;

const INPUT_SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.sketch.ts"
);
const TYPESCRIPT_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
);
const PROFILE_OFFSET_CLOSURE_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
);
const NAMED_POLYLINE_SOURCE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-polyline.sketch.ts");
const NAMED_POLYLINE_ENVELOPE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-polyline.json");
const NAMED_CURVE_TANGENCY_SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-curve-tangency.sketch.ts"
);
const NAMED_CURVE_TANGENCY_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-curve-tangency.json"
);
const NAMED_GEOMETRY_CONTROLS_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-geometry-controls.json"
);
const DIRECT_FILLET_SOURCE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-fillet.sketch.ts");
const DIRECT_FILLET_ENVELOPE: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-fillet.json");

#[derive(Serialize)]
struct IrDigestEnvelope<'a> {
    format: &'a str,
    imports: &'a [ManagedIrImport],
    statements: &'a [ManagedStatement],
    output: &'a ManagedExpression,
    source_sites: &'a [ManagedSourceSite],
    source_digest: &'a str,
}

#[derive(Serialize)]
struct ArtifactDigestEnvelope<'a> {
    format: &'a str,
    source_digest: &'a str,
    ir_digest: &'a str,
    declarations: &'a [ExecutedDeclarationResult],
    generated_members: &'a [ExecutedGeneratedMember],
    groups: &'a [ExecutedGroup],
    suppressions: &'a [ExecutedSuppression],
    value_consumers: &'a [ExecutedValueConsumer],
    output: &'a ManagedValue,
}

fn digest(value: impl AsRef<[u8]>) -> String {
    intent_content_digest(value.as_ref()).to_string()
}

fn refresh_artifact_authority(compiled: &mut CompiledManagedSource) {
    let envelope = ArtifactDigestEnvelope {
        format: &compiled.artifact.format,
        source_digest: &compiled.artifact.source_digest,
        ir_digest: &compiled.artifact.ir_digest,
        declarations: &compiled.artifact.declarations,
        generated_members: &compiled.artifact.generated_members,
        groups: &compiled.artifact.groups,
        suppressions: &compiled.artifact.suppressions,
        value_consumers: &compiled.artifact.value_consumers,
        output: &compiled.artifact.output,
    };
    compiled.artifact.artifact_digest =
        digest(serde_json::to_string(&envelope).expect("artifact digest envelope"));
    compiled.canonical_artifact_json =
        serde_json::to_string(&compiled.artifact).expect("canonical artifact");
}

#[test]
fn typescript_managed_envelope_is_exact_rust_canonical_json_and_digest_parity() {
    let compiled = CompiledManagedSource::from_json(TYPESCRIPT_ENVELOPE)
        .expect("the checked-in TypeScript compiler envelope must be admitted by Rust");
    compiled
        .validate_input_source(INPUT_SOURCE)
        .expect("inputSourceDigest must authenticate the exact pre-normalized TypeScript input");

    assert_eq!(compiled.input_source_digest, digest(INPUT_SOURCE));

    let normalized_source_digest = digest(&compiled.normalized_source);
    assert_ne!(
        compiled.input_source_digest, normalized_source_digest,
        "the non-canonical fixture must distinguish input and normalized source authority",
    );
    assert_eq!(compiled.ir.source_digest, normalized_source_digest);
    assert_eq!(compiled.artifact.source_digest, normalized_source_digest);
    assert!(
        compiled
            .ir
            .source_sites
            .iter()
            .all(|site| site.source_digest == normalized_source_digest),
        "every source-site digest must authenticate the normalized source",
    );

    let canonical_ir = serde_json::to_string::<ManagedSketchIr>(&compiled.ir).unwrap();
    assert_eq!(compiled.canonical_ir_json, canonical_ir);
    let ir_digest_envelope = IrDigestEnvelope {
        format: &compiled.ir.format,
        imports: &compiled.ir.imports,
        statements: &compiled.ir.statements,
        output: &compiled.ir.output,
        source_sites: &compiled.ir.source_sites,
        source_digest: &compiled.ir.source_digest,
    };
    assert_eq!(
        compiled.ir.ir_digest,
        digest(serde_json::to_string(&ir_digest_envelope).unwrap()),
    );

    let canonical_artifact =
        serde_json::to_string::<ExecutedSketchArtifact>(&compiled.artifact).unwrap();
    assert_eq!(compiled.canonical_artifact_json, canonical_artifact);
    assert_eq!(compiled.artifact.ir_digest, compiled.ir.ir_digest);
    let artifact_digest_envelope = ArtifactDigestEnvelope {
        format: &compiled.artifact.format,
        source_digest: &compiled.artifact.source_digest,
        ir_digest: &compiled.artifact.ir_digest,
        declarations: &compiled.artifact.declarations,
        generated_members: &compiled.artifact.generated_members,
        groups: &compiled.artifact.groups,
        suppressions: &compiled.artifact.suppressions,
        value_consumers: &compiled.artifact.value_consumers,
        output: &compiled.artifact.output,
    };
    assert_eq!(
        compiled.artifact.artifact_digest,
        digest(serde_json::to_string(&artifact_digest_envelope).unwrap()),
    );
}

#[test]
fn typescript_profile_offset_closure_reconstructs_from_authenticated_ir_after_cold_decode() {
    let compiled = CompiledManagedSource::from_json(PROFILE_OFFSET_CLOSURE_ENVELOPE)
        .expect("TypeScript Profile Offset compiler envelope");
    assert_eq!(
        compiled
            .source_declaration_closures()
            .expect("authenticated source closure projection"),
        [ManagedSourceDeclarationClosure {
            kind: ManagedSourceDeclarationClosureKind::ProfileOffset,
            root: SemanticSymbol("profileOffset12".into()),
            helpers: vec![SemanticSymbol("offsetChain11".into())],
        }],
    );
}

#[test]
fn named_polyline_typescript_envelope_cold_materializes_one_native_curve_and_axis_constraints() {
    let compiled = CompiledManagedSource::from_json(NAMED_POLYLINE_ENVELOPE)
        .expect("named Polyline TypeScript compiler envelope");
    compiled
        .validate_input_source(NAMED_POLYLINE_SOURCE)
        .expect("named source input digest");
    assert!(
        compiled
            .normalized_source
            .contains("$.geometry.polyline(\"geometry1\"")
    );
    assert!(!compiled.normalized_source.contains(".recipe("));
    assert!(compiled.normalized_source.lines().count() <= 30);
    assert!(compiled.normalized_source.len() <= 900);

    let project = CodeProject::managed(ProjectKey("named-polyline-interop".into()), compiled)
        .expect("named Polyline code project");
    let desired = required_generated_members(&project).expect("named member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("named reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_02),
        DocumentId(PersistentId::from_u128(0x89_c0_02)),
        1.0,
    )
    .expect("named Polyline cold materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted named Polyline authority");
    let document = accepted.session.design_document();
    assert_eq!(document.curves().len(), 1);
    assert_eq!(document.constraints().len(), 2);
    assert!(matches!(
        &document.curves()[0].definition,
        geosolve_sketch::CurveDefinition::Polyline {
            points,
            closed: false,
            ..
        } if points.len() == 3
    ));
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let graph = materialized.editor.coordinator().intent().graph();
    assert_eq!(
        graph
            .nodes()
            .values()
            .filter(|node| matches!(
                node.kind,
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Polyline
                }
            ))
            .count(),
        1
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one TypeScript interop owner keeps runtime provenance, native validation, and named constraint control routes together"
)]
fn named_curve_tangency_typescript_envelope_authenticates_and_cold_materializes() {
    let compiled = CompiledManagedSource::from_json(NAMED_CURVE_TANGENCY_ENVELOPE)
        .expect("named CurveCurveTangency TypeScript compiler envelope");
    compiled
        .validate_input_source(NAMED_CURVE_TANGENCY_SOURCE)
        .expect("named CurveCurveTangency input digest");
    assert!(
        compiled
            .normalized_source
            .contains("$.constraint.curveCurveTangency(\"tangent\"")
    );
    assert!(!compiled.normalized_source.contains(".recipe("));
    let declaration = compiled
        .artifact
        .declarations
        .iter()
        .find(|declaration| declaration.declaration == "tangent")
        .expect("executed tangent declaration");
    assert_eq!(declaration.family, "constraint.curveCurveTangency");
    assert_eq!(
        declaration
            .result
            .iter()
            .map(|result| (result.path.clone(), result.kind))
            .collect::<Vec<_>>(),
        vec![
            (
                vec![ManagedPathSegment::Field("constraint".into())],
                FeatureKind::Constraint,
            ),
            (
                vec![
                    ManagedPathSegment::Field("contacts".into()),
                    ManagedPathSegment::Field("first".into()),
                    ManagedPathSegment::Field("contact".into()),
                ],
                FeatureKind::Contact
            ),
            (
                vec![
                    ManagedPathSegment::Field("contacts".into()),
                    ManagedPathSegment::Field("first".into()),
                    ManagedPathSegment::Field("parameter".into())
                ],
                FeatureKind::Scalar
            ),
            (
                vec![
                    ManagedPathSegment::Field("contacts".into()),
                    ManagedPathSegment::Field("second".into()),
                    ManagedPathSegment::Field("contact".into()),
                ],
                FeatureKind::Contact
            ),
            (
                vec![
                    ManagedPathSegment::Field("contacts".into()),
                    ManagedPathSegment::Field("second".into()),
                    ManagedPathSegment::Field("parameter".into())
                ],
                FeatureKind::Scalar
            ),
        ],
        "the TypeScript runtime must retain the complete authenticated result catalog",
    );
    assert_eq!(
        compiled
            .artifact
            .value_consumers
            .iter()
            .filter(|consumer| {
                matches!(
                    &consumer.target,
                    ExecutedConsumerTarget::Declaration { declaration, .. }
                        if declaration == "tangent"
                ) && (consumer.property == [ManagedPathSegment::Field("first".into())]
                    || consumer.property == [ManagedPathSegment::Field("second".into())])
            })
            .count(),
        2,
        "both curve-span operands must retain lexical TypeScript provenance",
    );

    let project = CodeProject::managed(ProjectKey("named-curve-tangency-interop".into()), compiled)
        .expect("named CurveCurveTangency code project");
    let desired = required_generated_members(&project).expect("named member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("named reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_04),
        DocumentId(PersistentId::from_u128(0x89_c0_04)),
        1.0,
    )
    .expect("named CurveCurveTangency cold materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted named CurveCurveTangency authority");
    let document = accepted.session.design_document();
    assert_eq!(document.curves().len(), 2);
    assert_eq!(document.constraints().len(), 1);
    assert!(matches!(
        document.constraints()[0].definition,
        DocumentConstraintDefinition::CurveCurveTangency { .. }
    ));
    assert!(
        document
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let tangent_node = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find(|node| {
            matches!(
                node.kind,
                IntentNodeKind::Constraint {
                    constraint: geosolve_sketch_intent::ConstraintKind::CurveCurveTangency
                }
            )
        })
        .expect("authenticated CurveCurveTangency intent node");
    assert_eq!(tangent_node.inputs.len(), 2);
    assert_eq!(tangent_node.fields.len(), 12);
    assert!(
        tangent_node
            .fields
            .keys()
            .all(|field| !field.0.as_str().contains("domain")),
        "intrinsic curve topology must not be serialized as authored contact state",
    );

    let manifest = managed_control_manifest(&project, &materialized.expansion)
        .expect("named constraint managed controls");
    let neighborhood = named_control(
        &manifest,
        "tangent",
        &[
            ManagedPathSegment::Field("contacts".into()),
            ManagedPathSegment::Field("first".into()),
            ManagedPathSegment::Field("neighborhood".into()),
            ManagedPathSegment::Field("kind".into()),
        ],
    );
    assert_named_property(
        neighborhood,
        "tangent",
        "constraint.curveCurveTangency",
        "contacts/first/neighborhood/kind",
    );
    assert!(matches!(
        (&neighborhood.access, &neighborhood.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Choice { choices }),
        ) if choices == &["interior", "start", "end", "local"]
    ));
    let first_parameter = named_control(
        &manifest,
        "tangent",
        &[
            ManagedPathSegment::Field("contacts".into()),
            ManagedPathSegment::Field("first".into()),
            ManagedPathSegment::Field("parameter".into()),
        ],
    );
    assert_named_property(
        first_parameter,
        "tangent",
        "constraint.curveCurveTangency",
        "contacts/first/parameter",
    );
    assert!(matches!(
        (&first_parameter.access, &first_parameter.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Number {
                number: ManagedControlNumberKind::Real,
                ..
            }),
        )
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one interop regression keeps complete non-default Fillet branch and editable-radius evidence together"
)]
fn direct_fillet_typescript_execution_replays_branch_state_and_editable_radius() {
    let compiled = CompiledManagedSource::from_json(DIRECT_FILLET_ENVELOPE)
        .expect("direct Fillet TypeScript compiler envelope");
    compiled
        .validate_input_source(DIRECT_FILLET_SOURCE)
        .expect("direct Fillet input source digest");
    assert!(
        compiled
            .normalized_source
            .contains("$.computed.filletSet(\"round\"")
    );
    let declaration = compiled
        .artifact
        .declarations
        .iter()
        .find(|declaration| declaration.declaration == "round")
        .expect("executed direct Fillet declaration");
    assert_eq!(declaration.family, "computed.filletSet");
    assert!(compiled.artifact.value_consumers.iter().any(|consumer| {
        matches!(
            &consumer.target,
            ExecutedConsumerTarget::Declaration {
                declaration,
                family,
            } if declaration == "round" && family == "computed.filletSet"
        ) && consumer.property == [ManagedPathSegment::Field("radius".into())]
    }));

    let project =
        CodeProject::managed(ProjectKey("managed-direct-fillet-interop".into()), compiled)
            .expect("direct Fillet managed project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("direct Fillet generated inventory"),
            &BTreeSet::new(),
        )
        .expect("direct Fillet reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_f0_06),
        DocumentId(PersistentId::from_u128(0x89_f0_06)),
        1.0,
    )
    .expect("direct Fillet cold materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted direct Fillet authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let [feature] = accepted.features.features() else {
        panic!("direct Fillet fixture must own one computed feature")
    };
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    assert_eq!(fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(fillet.corners.len(), 2);

    let manifest = managed_control_manifest(&project, &materialized.expansion)
        .expect("direct Fillet managed controls");
    let radius = manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration == SemanticSymbol("round".into())
                && control.source.path
                    == SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())])
        })
        .expect("direct Fillet radius control");
    assert!(matches!(
        (&radius.access, &radius.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Unit {
                unit,
                number: ManagedControlNumberKind::Real,
                minimum: Some(minimum),
                maximum: None,
            })
        ) if unit == "mm" && minimum.value == 0.0 && !minimum.inclusive
    ));
    let compiled = project
        .managed
        .compiled
        .as_deref()
        .expect("direct Fillet compiler authority");
    let mutation = derive_managed_value_mutation(
        compiled,
        &radius.source.declaration,
        &radius.source.path,
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 1.25,
        }),
    )
    .expect("direct Fillet radius prepares one exact source mutation");
    assert_eq!(
        mutation.expected,
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 1.0,
        })
    );
    assert_eq!(
        mutation.value,
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 1.25,
        })
    );

    let anchor_kind = named_control(
        &manifest,
        "round",
        &[
            ManagedPathSegment::Field("corners".into()),
            ManagedPathSegment::Index(0),
            ManagedPathSegment::Field("parents".into()),
            ManagedPathSegment::Index(0),
            ManagedPathSegment::Field("periodicAnchor".into()),
            ManagedPathSegment::Field("kind".into()),
        ],
    );
    assert!(matches!(
        (&anchor_kind.access, &anchor_kind.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Choice { choices }),
        ) if choices == &["none", "anchor"]
    ));
    let endpoint_order = named_control(
        &manifest,
        "round",
        &[
            ManagedPathSegment::Field("corners".into()),
            ManagedPathSegment::Index(0),
            ManagedPathSegment::Field("endpointOrder".into()),
        ],
    );
    assert!(matches!(
        (&endpoint_order.access, &endpoint_order.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Choice { choices }),
        ) if choices == &["firstThenSecond", "secondThenFirst"]
    ));
}

fn named_control<'a>(
    manifest: &'a geosolve_sketch_code::ManagedControlManifest,
    declaration: &str,
    path: &[ManagedPathSegment],
) -> &'a ManagedControl {
    manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration == SemanticSymbol(declaration.into())
                && control.source.path.0 == path
        })
        .unwrap_or_else(|| panic!("missing named control `{declaration}` at {path:?}"))
}

fn assert_named_property(
    control: &ManagedControl,
    declaration: &str,
    family: &str,
    property: &str,
) {
    let expected = SemanticOutputPath(
        property
            .split('/')
            .map(|segment| {
                segment.parse::<usize>().map_or_else(
                    |_| ManagedPathSegment::Field(segment.into()),
                    ManagedPathSegment::Index,
                )
            })
            .collect(),
    );
    assert!(
        control.consumers.iter().any(|consumer| {
            matches!(
                &consumer.target,
                geosolve_sketch_code::ManagedControlConsumerTarget::Declaration {
                    declaration: candidate,
                    family: candidate_family,
                } if candidate == &SemanticSymbol(declaration.into())
                    && candidate_family == family
            ) && consumer.property == expected
        }),
        "named source {:?} did not route to semantic property {property}",
        control.source.path,
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one TypeScript-authored fixture keeps named scalar, choice, weight, field, point-drag, source mutation, and cold native authority together"
)]
fn named_geometry_controls_use_semantic_inspector_paths_and_exact_lexical_mutations() {
    let compiled = CompiledManagedSource::from_json(NAMED_GEOMETRY_CONTROLS_ENVELOPE)
        .expect("TypeScript-compiled named geometry controls fixture");
    let project = CodeProject::managed(ProjectKey("m90-named-geometry-controls".into()), compiled)
        .expect("named geometry controls project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("named generated inventory"),
            &BTreeSet::new(),
        )
        .expect("named reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_03),
        DocumentId(PersistentId::from_u128(0x89_c0_03)),
        1.0,
    )
    .expect("edited named geometry fixture cold materialization");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted named geometry authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );

    let manifest = managed_control_manifest(&project, &materialized.expansion)
        .expect("named geometry managed controls");
    let parameter_path = [
        ManagedPathSegment::Field("source".into()),
        ManagedPathSegment::Field("contact".into()),
        ManagedPathSegment::Field("parameter".into()),
    ];
    let weight_path = [
        ManagedPathSegment::Field("controls".into()),
        ManagedPathSegment::Index(1),
        ManagedPathSegment::Field("weight".into()),
    ];
    let degree_path = [ManagedPathSegment::Field("degree".into())];
    let gauge_path = [ManagedPathSegment::Field("gauge".into())];
    let point_x_path = [
        ManagedPathSegment::Field("firstControl".into()),
        ManagedPathSegment::Index(0),
    ];
    let parameter = named_control(&manifest, "tangent", &parameter_path);
    let weight = named_control(&manifest, "periodic", &weight_path);
    let degree = named_control(&manifest, "periodic", &degree_path);
    let gauge = named_control(&manifest, "periodic", &gauge_path);
    let point_x = named_control(&manifest, "cubic", &point_x_path);
    for (control, declaration, property) in [
        (parameter, "tangent", "source/contact/parameter"),
        (weight, "periodic", "controls/1/weight"),
        (degree, "periodic", "degree"),
        (gauge, "periodic", "gauge"),
        (point_x, "cubic", "firstControl/0"),
    ] {
        assert_named_property(
            control,
            declaration,
            if declaration == "periodic" {
                "geometry.periodicControlNurbs"
            } else if declaration == "cubic" {
                "geometry.cubicBezier"
            } else {
                "geometry.tangentArc"
            },
            property,
        );
    }
    for control in [parameter, weight] {
        assert!(matches!(
            (&control.access, &control.schema),
            (
                ManagedControlAccess::Editable { .. },
                Some(ManagedControlSchema::Number {
                    number: ManagedControlNumberKind::Real,
                    ..
                })
            )
        ));
    }
    assert!(matches!(
        (&degree.access, &degree.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Number {
                number: ManagedControlNumberKind::Natural,
                ..
            })
        )
    ));
    assert!(matches!(
        (&weight.access, &weight.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Number {
                number: ManagedControlNumberKind::Real,
                minimum: Some(minimum),
                maximum: None,
            }),
        ) if minimum.value == 0.0 && !minimum.inclusive
    ));
    assert!(manifest.controls.iter().all(|control| {
        control.source.path.0.iter().all(
            |segment| !matches!(segment, ManagedPathSegment::Field(field) if field == "domain"),
        )
    }));
    assert!(matches!(
        (&gauge.access, &gauge.schema),
        (
            ManagedControlAccess::Editable { .. },
            Some(ManagedControlSchema::Choice { choices }),
        ) if choices == &["c0", "c1", "c2", "c3"]
    ));
    assert!(matches!(
        point_x.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::SolverInstance,
            ..
        }
    ));

    let compiled = project
        .managed
        .compiled
        .as_deref()
        .expect("named compiler authority");
    for (control, replacement) in [
        (parameter, ManagedValue::Number(0.75)),
        (weight, ManagedValue::Number(2.0)),
        (gauge, ManagedValue::String("c2".into())),
    ] {
        let mutation = derive_managed_value_mutation(
            compiled,
            &control.source.declaration,
            &control.source.path,
            replacement.clone(),
        )
        .expect("named semantic control prepares one exact lexical mutation");
        assert_eq!(mutation.path, control.source.path.0);
        assert_eq!(mutation.expected, control.value);
        assert_eq!(mutation.value, replacement);
    }

    let cubic_writable = materialized
        .expansion
        .writable_points
        .iter()
        .find(|point| {
            matches!(
                &point.edit,
                CodePointEdit::Point { address }
                    if matches!(
                        &address.owner.address,
                        CodeOwnerAddress::DirectDeclaration { declaration }
                            if declaration == &SemanticSymbol("cubic".into())
                    ) && address.output.0 == [ManagedPathSegment::Field("firstControl".into())]
            )
        })
        .expect("cubic first control writable point");
    let mutations = managed_point_value_mutations(
        &project.project,
        project
            .managed
            .compiled
            .as_deref()
            .expect("named compiler authority"),
        &cubic_writable.edit.point_updates([3.0, 5.0]),
    )
    .expect("named Bezier point drag source mutations");
    assert_eq!(
        mutations
            .iter()
            .map(|mutation| (mutation.path.clone(), mutation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![(
            vec![ManagedPathSegment::Field("firstControl".into())],
            ManagedValue::Array(vec![ManagedValue::Number(2.0), ManagedValue::Number(4.0),]),
        )]
    );
}

#[test]
fn rust_rejects_digest_consistent_false_direct_declaration_result_shape() {
    let mut hostile = CompiledManagedSource::from_json(TYPESCRIPT_ENVELOPE)
        .expect("baseline TypeScript compiler envelope");
    let circle = hostile
        .artifact
        .declarations
        .iter_mut()
        .find(|declaration| declaration.declaration == "hole")
        .expect("direct circle declaration");
    circle.result[0].kind = FeatureKind::Constraint;
    refresh_artifact_authority(&mut hostile);

    let hostile_json = serde_json::to_string(&hostile).expect("hostile compiler envelope");
    let error = CompiledManagedSource::from_json(&hostile_json)
        .expect_err("Rust must own the exact result catalog for every direct declaration");
    assert!(
        error.to_string().contains("Rust-owned result descriptor"),
        "unexpected refusal: {error}",
    );
}

#[test]
fn rust_rejects_digest_consistent_incomplete_direct_value_provenance() {
    let mut hostile = CompiledManagedSource::from_json(TYPESCRIPT_ENVELOPE)
        .expect("baseline TypeScript compiler envelope");
    let removed = hostile
        .artifact
        .value_consumers
        .iter()
        .position(|consumer| {
            matches!(
                &consumer.target,
                geosolve_sketch_code::ExecutedConsumerTarget::Declaration {
                    declaration,
                    ..
                } if declaration == "hole"
            )
        })
        .expect("direct circle consumer");
    hostile.artifact.value_consumers.remove(removed);
    refresh_artifact_authority(&mut hostile);

    let hostile_json = serde_json::to_string(&hostile).expect("hostile compiler envelope");
    let error = CompiledManagedSource::from_json(&hostile_json)
        .expect_err("Rust must reconstruct the complete direct consumer graph from IR");
    assert!(
        error
            .to_string()
            .contains("complete direct value-consumer provenance"),
        "unexpected refusal: {error}",
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one cross-host fixture keeps named execution, cold materialization, validation, and exact native output ownership together"
)]
fn typescript_named_declarations_cold_materialize_finite_native_authority() {
    let compiled = CompiledManagedSource::from_json(TYPESCRIPT_ENVELOPE)
        .expect("TypeScript named-declaration fixture");
    let point_declaration = compiled
        .artifact
        .declarations
        .iter()
        .find(|declaration| declaration.declaration == "point")
        .expect("executed named point declaration");
    assert_eq!(point_declaration.family, "geometry.sketchPoint");
    assert_eq!(
        point_declaration.result,
        [geosolve_sketch_code::ExecutedResultLeaf {
            kind: FeatureKind::Point,
            path: vec![ManagedPathSegment::Field("point".into())],
        }]
    );

    let project = CodeProject::managed(ProjectKey("m90-named-declaration-cold".into()), compiled)
        .expect("Rust must admit the complete TypeScript authority");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("empty named-declaration reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_1ec1),
        DocumentId(PersistentId::from_u128(0x89_1ec1)),
        1.0,
    )
    .expect("named point plus dependent line must cold materialize");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("independently accepted native authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    assert!(
        accepted
            .session
            .design_document()
            .points()
            .iter()
            .all(|point| {
                point
                    .position
                    .iter()
                    .all(|coordinate| coordinate.is_finite())
            })
    );

    let intent = materialized.editor.coordinator().intent();
    let declaration_node = |symbol: &str| {
        let alias = materialized
            .expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| {
                (declaration == &SemanticSymbol(symbol.into())).then_some(alias)
            })
            .unwrap_or_else(|| panic!("{symbol} declaration provenance"));
        intent
            .graph()
            .node_by_symbol(alias)
            .unwrap_or_else(|| panic!("{symbol} Intent node"))
    };
    let point = declaration_node("point");
    assert!(matches!(
        point.kind,
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint
        }
    ));
    let primary = point
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        })
        .expect("named point output")
        .as_ref(point.id);
    let IntentNativeBinding::Point(native_point) = accepted
        .ownership
        .port(primary)
        .expect("generic output owns one native point")
    else {
        panic!("named point output has the wrong native kind")
    };
    assert_eq!(
        accepted
            .session
            .design_document()
            .point(native_point)
            .expect("named native point")
            .position
            .map(f64::to_bits),
        [1.25, -6.5].map(f64::to_bits)
    );

    let segment = declaration_node("segment");
    let start = segment
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Start,
            index: 0,
        })
        .expect("dependent segment start")
        .as_ref(segment.id);
    assert_eq!(
        accepted.ownership.port(start),
        Some(IntentNativeBinding::Point(native_point)),
        "the ordinary line must consume the exact named runtime output",
    );

    let writable = materialized
        .expansion
        .writable_points
        .iter()
        .filter(|writable| writable.handle.alias == point.symbol)
        .collect::<Vec<_>>();
    let [writable] = writable.as_slice() else {
        panic!("named source point must publish exactly one writable handle")
    };
    assert_eq!(writable.source, CodePointSeedSource::Literal);
    let CodePointEdit::Point { address } = &writable.edit else {
        panic!("named source point must use one Cartesian semantic address")
    };
    assert_eq!(
        address.output.0,
        [ManagedPathSegment::Field("point".into())]
    );
    let target = [3.75, -2.25];
    let staged = writable
        .stage_drag(&CodeInteractionOverlay::empty(), target)
        .expect("finite named point drag must stage atomically");
    assert_eq!(
        staged.draft(address).expect("staged point draft").value,
        geosolve_sketch_code::CodeDraftValue::Point(target)
    );
    assert!(
        writable
            .stage_drag(&CodeInteractionOverlay::empty(), [f64::NAN, 0.0])
            .is_err(),
        "a non-finite generic point drag must leave no staged draft"
    );
    let dragged = materialize_code_project_cold_with_overlay(
        &project,
        &generated,
        &staged,
        IntentSessionId::from_raw(0x89_1ec2),
        DocumentId(PersistentId::from_u128(0x89_1ec2)),
        1.0,
    )
    .expect("named point overlay must cold materialize");
    let dragged_alias = dragged
        .expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("point".into())).then_some(alias)
        })
        .expect("dragged point declaration provenance");
    let dragged_node = dragged
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(dragged_alias)
        .expect("dragged point node");
    let dragged_port = dragged_node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        })
        .expect("dragged point port")
        .as_ref(dragged_node.id);
    let IntentNativeBinding::Point(dragged_point) = dragged
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("dragged accepted authority")
        .ownership
        .port(dragged_port)
        .expect("dragged native point ownership")
    else {
        panic!("dragged named output has wrong native kind")
    };
    assert_eq!(
        dragged
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("dragged accepted authority")
            .session
            .design_document()
            .point(dragged_point)
            .expect("dragged native point")
            .position
            .map(f64::to_bits),
        target.map(f64::to_bits)
    );

    let source_mutations = managed_point_value_mutations(
        &project.project,
        project
            .managed
            .compiled
            .as_deref()
            .expect("managed compiler authority"),
        &writable.edit.point_updates(target),
    )
    .expect("named point drag must resolve to exact source leaves");
    assert_eq!(source_mutations.len(), 1);
    assert_eq!(
        source_mutations
            .iter()
            .map(|mutation| (
                mutation.path.clone(),
                mutation.expected.clone(),
                mutation.value.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![(
            vec![ManagedPathSegment::Field("point".into())],
            ManagedValue::Array(vec![ManagedValue::Number(1.25), ManagedValue::Number(-6.5),]),
            ManagedValue::Array(target.into_iter().map(ManagedValue::Number).collect()),
        ),]
    );
}
