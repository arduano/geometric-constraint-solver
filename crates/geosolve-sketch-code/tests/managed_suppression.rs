// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::ComputedFeatureEvaluationState;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeExpansionError, CodeInteractionOverlay, CodeOwnerAddress, CodeProject, CodeProjectDemoId,
    CompiledManagedSource, EXECUTED_SKETCH_ARTIFACT_FORMAT, ExecutedConsumerTarget,
    ExecutedDeclarationResult, ExecutedGeneratedMember, ExecutedGroup, ExecutedResultLeaf,
    ExecutedSketchArtifact, ExecutedSuppression, ExecutedValueConsumer, FeatureKind,
    KeyedReconcileState, MANAGED_SKETCH_IR_FORMAT, ManagedControlAccess,
    ManagedControlConsumerTarget, ManagedControlReadOnlyReason, ManagedExpression, ManagedIrImport,
    ManagedObjectField, ManagedPathSegment, ManagedReference, ManagedSketchIr, ManagedSourceSite,
    ManagedSourceSiteKind, ManagedSourceSpan, ManagedStatement, ManagedValue, ProjectKey,
    SemanticOutputPath, SemanticSymbol, bundled_code_project_demos, expand_code_project,
    expand_code_project_with_overlay, managed_control_manifest, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId, intent_content_digest};
use serde::Serialize;

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

struct TestSites {
    source_digest: String,
    next: usize,
    values: Vec<ManagedSourceSite>,
}

impl TestSites {
    fn new(source: &str) -> Self {
        Self {
            source_digest: digest(source),
            next: 0,
            values: Vec::new(),
        }
    }

    fn add(&mut self, kind: ManagedSourceSiteKind) -> String {
        self.add_span(kind, ManagedSourceSpan { start: 0, end: 0 })
    }

    fn add_span(&mut self, kind: ManagedSourceSiteKind, span: ManagedSourceSpan) -> String {
        let id = digest(format!("m89-managed-suppression-test-site:{}", self.next));
        self.next += 1;
        self.values.push(ManagedSourceSite {
            id: id.clone(),
            kind,
            span,
            source_digest: self.source_digest.clone(),
        });
        id
    }
}

fn digest(value: impl AsRef<[u8]>) -> String {
    intent_content_digest(value.as_ref()).to_string()
}

fn number(sites: &mut TestSites, value: f64) -> ManagedExpression {
    ManagedExpression::Number {
        value,
        site: sites.add(ManagedSourceSiteKind::Value),
    }
}

fn point(sites: &mut TestSites, value: [f64; 2]) -> ManagedExpression {
    ManagedExpression::Array {
        values: vec![number(sites, value[0]), number(sites, value[1])],
        site: sites.add(ManagedSourceSiteKind::Value),
    }
}

fn line_arguments(sites: &mut TestSites, start: [f64; 2], end: [f64; 2]) -> ManagedExpression {
    ManagedExpression::Object {
        fields: vec![
            ManagedObjectField {
                name: "start".into(),
                value: point(sites, start),
                comments: Vec::new(),
            },
            ManagedObjectField {
                name: "end".into(),
                value: point(sites, end),
                comments: Vec::new(),
            },
        ],
        site: sites.add(ManagedSourceSiteKind::Value),
    }
}

fn object(
    sites: &mut TestSites,
    fields: impl IntoIterator<Item = (&'static str, ManagedExpression)>,
) -> ManagedExpression {
    ManagedExpression::Object {
        fields: fields
            .into_iter()
            .map(|(name, value)| ManagedObjectField {
                name: name.into(),
                value,
                comments: Vec::new(),
            })
            .collect(),
        site: sites.add(ManagedSourceSiteKind::Value),
    }
}

fn reference(
    sites: &mut TestSites,
    declaration: &str,
    path: impl IntoIterator<Item = &'static str>,
) -> ManagedExpression {
    ManagedExpression::Reference {
        declaration: declaration.into(),
        path: path
            .into_iter()
            .map(|value| ManagedPathSegment::Field(value.into()))
            .collect(),
        site: sites.add(ManagedSourceSiteKind::Value),
    }
}

fn synthetic_expression_origins(
    expression: &ManagedExpression,
    bindings: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Call { site, .. } => vec![site.clone()],
        ManagedExpression::Reference { declaration, .. } => {
            bindings.get(declaration).cloned().unwrap_or_default()
        }
        ManagedExpression::Array { values, .. } => values
            .iter()
            .flat_map(|value| synthetic_expression_origins(value, bindings))
            .collect(),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .flat_map(|field| synthetic_expression_origins(&field.value, bindings))
            .collect(),
    }
}

fn collect_synthetic_direct_consumers(
    expression: &ManagedExpression,
    target: &ExecutedConsumerTarget,
    property: &mut Vec<ManagedPathSegment>,
    bindings: &std::collections::BTreeMap<String, Vec<String>>,
    consumers: &mut Vec<ExecutedValueConsumer>,
) {
    let sites = match expression {
        ManagedExpression::Reference {
            declaration, site, ..
        } => bindings
            .get(declaration)
            .filter(|origins| !origins.is_empty())
            .cloned()
            .unwrap_or_else(|| vec![site.clone()]),
        ManagedExpression::Call { site, .. }
        | ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. } => vec![site.clone()],
        ManagedExpression::Array { values, .. } => {
            for (index, value) in values.iter().enumerate() {
                property.push(ManagedPathSegment::Index(index));
                collect_synthetic_direct_consumers(value, target, property, bindings, consumers);
                property.pop();
            }
            return;
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                property.push(ManagedPathSegment::Field(field.name.clone()));
                collect_synthetic_direct_consumers(
                    &field.value,
                    target,
                    property,
                    bindings,
                    consumers,
                );
                property.pop();
            }
            return;
        }
    };
    consumers.extend(sites.into_iter().map(|value_site| ExecutedValueConsumer {
        value_site,
        target: target.clone(),
        property: property.clone(),
    }));
}

fn synthetic_direct_consumers(statements: &[ManagedStatement]) -> Vec<ExecutedValueConsumer> {
    let mut bindings = std::collections::BTreeMap::new();
    let mut consumers = Vec::new();
    for statement in statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => {
                bindings.insert(
                    variable.clone(),
                    synthetic_expression_origins(value, &bindings),
                );
            }
            ManagedStatement::Declaration {
                symbol,
                builder_path,
                arguments,
                patch: None,
                ..
            } => collect_synthetic_direct_consumers(
                arguments,
                &ExecutedConsumerTarget::Declaration {
                    declaration: symbol.clone(),
                    family: builder_path.join("."),
                },
                &mut Vec::new(),
                &bindings,
                &mut consumers,
            ),
            ManagedStatement::Declaration { .. }
            | ManagedStatement::Group { .. }
            | ManagedStatement::Suppression { .. } => {}
        }
    }
    consumers
}

#[allow(
    clippy::too_many_arguments,
    reason = "the synthetic compiler envelope keeps each authenticated IR/artifact collection explicit at the test boundary"
)]
fn finish_compiled(
    source: String,
    mut sites: TestSites,
    imports: Vec<ManagedIrImport>,
    statements: Vec<ManagedStatement>,
    declarations: Vec<ExecutedDeclarationResult>,
    generated_members: Vec<ExecutedGeneratedMember>,
    suppressions: Vec<ExecutedSuppression>,
    value_consumers: Vec<ExecutedValueConsumer>,
) -> CompiledManagedSource {
    let mut complete_consumers = synthetic_direct_consumers(&statements);
    complete_consumers.extend(value_consumers);
    let output = object(&mut sites, []);
    let mut ir = ManagedSketchIr {
        format: MANAGED_SKETCH_IR_FORMAT.into(),
        imports,
        statements,
        output,
        source_sites: sites.values,
        source_digest: digest(&source),
        ir_digest: String::new(),
    };
    ir.ir_digest = digest(
        serde_json::to_string(&IrDigestEnvelope {
            format: &ir.format,
            imports: &ir.imports,
            statements: &ir.statements,
            output: &ir.output,
            source_sites: &ir.source_sites,
            source_digest: &ir.source_digest,
        })
        .unwrap(),
    );
    let mut artifact = ExecutedSketchArtifact {
        format: EXECUTED_SKETCH_ARTIFACT_FORMAT.into(),
        source_digest: ir.source_digest.clone(),
        ir_digest: ir.ir_digest.clone(),
        declarations,
        generated_members,
        groups: Vec::new(),
        suppressions,
        value_consumers: complete_consumers,
        output: ManagedValue::Object(BTreeMap::new()),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = digest(
        serde_json::to_string(&ArtifactDigestEnvelope {
            format: &artifact.format,
            source_digest: &artifact.source_digest,
            ir_digest: &artifact.ir_digest,
            declarations: &artifact.declarations,
            generated_members: &artifact.generated_members,
            groups: &artifact.groups,
            suppressions: &artifact.suppressions,
            value_consumers: &artifact.value_consumers,
            output: &artifact.output,
        })
        .unwrap(),
    );
    let compiled = CompiledManagedSource {
        input_source_digest: digest(&source),
        normalized_source: source,
        canonical_ir_json: serde_json::to_string(&ir).unwrap(),
        canonical_artifact_json: serde_json::to_string(&artifact).unwrap(),
        ir,
        artifact,
    };
    compiled.validate().unwrap();
    compiled
}

fn without_direct_value_consumers(mut compiled: CompiledManagedSource) -> CompiledManagedSource {
    compiled
        .artifact
        .value_consumers
        .retain(|consumer| !matches!(consumer.target, ExecutedConsumerTarget::Declaration { .. }));
    refresh_artifact_digest(&mut compiled);
    compiled
}

fn without_generated_value_consumers(mut compiled: CompiledManagedSource) -> CompiledManagedSource {
    compiled
        .artifact
        .value_consumers
        .retain(|consumer| !matches!(consumer.target, ExecutedConsumerTarget::Generated { .. }));
    refresh_artifact_digest(&mut compiled);
    compiled.validate().unwrap();
    compiled
}

fn refresh_artifact_digest(compiled: &mut CompiledManagedSource) {
    compiled.artifact.artifact_digest = digest(
        serde_json::to_string(&ArtifactDigestEnvelope {
            format: &compiled.artifact.format,
            source_digest: &compiled.artifact.source_digest,
            ir_digest: &compiled.artifact.ir_digest,
            declarations: &compiled.artifact.declarations,
            generated_members: &compiled.artifact.generated_members,
            groups: &compiled.artifact.groups,
            suppressions: &compiled.artifact.suppressions,
            value_consumers: &compiled.artifact.value_consumers,
            output: &compiled.artifact.output,
        })
        .unwrap(),
    );
    compiled.canonical_artifact_json = serde_json::to_string(&compiled.artifact).unwrap();
}

fn direct_lines_compiled(suppressed: bool) -> CompiledManagedSource {
    let suppression_source = if suppressed {
        "  $.suppress(hidden);\n"
    } else {
        ""
    };
    let source = format!(
        "\"use geosolve sketch\";\n\
         import {{ sketch }} from \"@geosolve/sketch-code\";\n\n\
         export default sketch(($) => {{\n\
           const kept = $.geometry.segment(\"kept\", {{ start: [0, 0], end: [20, 0] }});\n\
           const hidden = $.geometry.segment(\"hidden\", {{ start: [0, 10], end: [20, 10] }});\n\
         {suppression_source}}});\n"
    );
    let mut sites = TestSites::new(&source);
    let kept_site = sites.add(ManagedSourceSiteKind::Declaration);
    let hidden_site = sites.add(ManagedSourceSiteKind::Declaration);
    let mut statements = vec![
        ManagedStatement::Declaration {
            variable: "kept".into(),
            symbol: "kept".into(),
            builder_path: vec!["geometry".into(), "segment".into()],
            patch: None,
            arguments: line_arguments(&mut sites, [0.0, 0.0], [20.0, 0.0]),
            site: kept_site.clone(),
            comments: Vec::new(),
        },
        ManagedStatement::Declaration {
            variable: "hidden".into(),
            symbol: "hidden".into(),
            builder_path: vec!["geometry".into(), "segment".into()],
            patch: None,
            arguments: line_arguments(&mut sites, [0.0, 10.0], [20.0, 10.0]),
            site: hidden_site.clone(),
            comments: Vec::new(),
        },
    ];
    let suppression = suppressed.then(|| {
        let target = ManagedReference {
            declaration: "hidden".into(),
            path: Vec::new(),
            site: sites.add(ManagedSourceSiteKind::SuppressionReference),
        };
        let site = sites.add(ManagedSourceSiteKind::Suppression);
        statements.push(ManagedStatement::Suppression {
            target: target.clone(),
            site: site.clone(),
            comments: Vec::new(),
        });
        ExecutedSuppression { site, target }
    });
    let imports = vec![ManagedIrImport {
        module: "@geosolve/sketch-code".into(),
        bindings: vec!["sketch".into()],
    }];
    let segment_result = || {
        vec![
            ExecutedResultLeaf {
                kind: FeatureKind::Curve,
                path: vec![ManagedPathSegment::Field("curve".into())],
            },
            ExecutedResultLeaf {
                kind: FeatureKind::Point,
                path: vec![ManagedPathSegment::Field("end".into())],
            },
            ExecutedResultLeaf {
                kind: FeatureKind::CurveSpan,
                path: vec![ManagedPathSegment::Field("span".into())],
            },
            ExecutedResultLeaf {
                kind: FeatureKind::Point,
                path: vec![ManagedPathSegment::Field("start".into())],
            },
        ]
    };
    finish_compiled(
        source,
        sites,
        imports,
        statements,
        vec![
            ExecutedDeclarationResult {
                declaration: "kept".into(),
                family: "geometry.segment".into(),
                patch: None,
                site: kept_site,
                result: segment_result(),
            },
            ExecutedDeclarationResult {
                declaration: "hidden".into(),
                family: "geometry.segment".into(),
                patch: None,
                site: hidden_site,
                result: segment_result(),
            },
        ],
        Vec::new(),
        suppression.into_iter().collect(),
        Vec::new(),
    )
}

fn generated_fillets_compiled(suppressed: bool) -> CompiledManagedSource {
    let json = if suppressed {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-both-suppressed.json"
        ))
    } else {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-base.json"
        ))
    };
    CompiledManagedSource::from_json(json).expect("checked-in managed lifecycle fixture")
}

fn generated_fillets_project(suppressed: bool) -> CodeProject {
    let base = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::TypedPanel)
        .unwrap()
        .project();
    let project = CodeProject {
        project: ProjectKey("m89-managed-generated-suppression".into()),
        managed: generated_fillets_compiled(suppressed)
            .into_managed_document()
            .unwrap(),
        custom_files: base.custom_files,
        artifacts: base.artifacts,
        lock: base.lock,
    };
    project.validate().unwrap();
    project
}

#[allow(
    clippy::too_many_lines,
    reason = "the focused fixture keeps exact source sites, runtime results, and both fan-out edges adjacent"
)]
fn direct_shared_radius_compiled() -> CompiledManagedSource {
    let source = "\"use geosolve sketch\";\n\
                  import { sketch, mm } from \"@geosolve/sketch-code\";\n\n\
                  export default sketch(($) => {\n\
                    const sharedRadius = mm(4);\n\
                    const hole = $.geometry.centerRadiusCircle(\"hole\", { center: [0, 2], radius: sharedRadius });\n\
                    const radius = $.dimension.radius(\"radius\", { curve: hole.curve, value: sharedRadius, mode: \"driving\", suppressed: false });\n\
                  });\n"
        .to_owned();
    let mut sites = TestSites::new(&source);
    let binding_start = source.find("mm(4)").unwrap();
    let binding_site = sites.add_span(
        ManagedSourceSiteKind::Value,
        ManagedSourceSpan {
            start: binding_start,
            end: binding_start + "mm(4)".len(),
        },
    );
    let binding_value = ManagedExpression::Call {
        callee: "mm".into(),
        arguments: vec![number(&mut sites, 4.0)],
        site: binding_site.clone(),
    };
    let hole_site = sites.add(ManagedSourceSiteKind::Declaration);
    let center = point(&mut sites, [0.0, 2.0]);
    let hole_radius = reference(&mut sites, "sharedRadius", []);
    let hole_arguments = object(&mut sites, [("center", center), ("radius", hole_radius)]);
    let radius_site = sites.add(ManagedSourceSiteKind::Declaration);
    let curve = reference(&mut sites, "hole", ["curve"]);
    let value = reference(&mut sites, "sharedRadius", []);
    let mode = ManagedExpression::String {
        value: "driving".into(),
        site: sites.add(ManagedSourceSiteKind::Value),
    };
    let suppressed = ManagedExpression::Boolean {
        value: false,
        site: sites.add(ManagedSourceSiteKind::Value),
    };
    let radius_arguments = object(
        &mut sites,
        [
            ("curve", curve),
            ("value", value),
            ("mode", mode),
            ("suppressed", suppressed),
        ],
    );
    finish_compiled(
        source,
        sites,
        vec![ManagedIrImport {
            module: "@geosolve/sketch-code".into(),
            bindings: vec!["sketch".into(), "mm".into()],
        }],
        vec![
            ManagedStatement::Binding {
                variable: "sharedRadius".into(),
                value: binding_value,
                comments: Vec::new(),
            },
            ManagedStatement::Declaration {
                variable: "hole".into(),
                symbol: "hole".into(),
                builder_path: vec!["geometry".into(), "centerRadiusCircle".into()],
                patch: None,
                arguments: hole_arguments,
                site: hole_site.clone(),
                comments: Vec::new(),
            },
            ManagedStatement::Declaration {
                variable: "radius".into(),
                symbol: "radius".into(),
                builder_path: vec!["dimension".into(), "radius".into()],
                patch: None,
                arguments: radius_arguments,
                site: radius_site.clone(),
                comments: Vec::new(),
            },
        ],
        vec![
            ExecutedDeclarationResult {
                declaration: "hole".into(),
                family: "geometry.centerRadiusCircle".into(),
                patch: None,
                site: hole_site,
                result: vec![
                    ExecutedResultLeaf {
                        kind: FeatureKind::Point,
                        path: vec![ManagedPathSegment::Field("center".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::Curve,
                        path: vec![ManagedPathSegment::Field("curve".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::Scalar,
                        path: vec![ManagedPathSegment::Field("radius".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::CurveSpan,
                        path: vec![ManagedPathSegment::Field("span".into())],
                    },
                ],
            },
            ExecutedDeclarationResult {
                declaration: "radius".into(),
                family: "dimension.radius".into(),
                patch: None,
                site: radius_site,
                result: vec![
                    ExecutedResultLeaf {
                        kind: FeatureKind::Dimension,
                        path: vec![ManagedPathSegment::Field("dimension".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::Scalar,
                        path: vec![ManagedPathSegment::Field("value".into())],
                    },
                ],
            },
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn reconciled(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged()
}

#[test]
fn direct_source_suppression_reaches_cold_rust_materialization() {
    let compiled = direct_lines_compiled(true);
    let project = CodeProject::managed(
        ProjectKey("m89-managed-direct-suppression".into()),
        compiled,
    )
    .unwrap();
    let generated = reconciled(&project);
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_5101),
        DocumentId(PersistentId::from_u128(0x89_5101)),
        1.0,
    )
    .unwrap();

    let intent = materialized.editor.coordinator().intent();
    let declarations = &materialized.expansion.declaration_provenance;
    let hidden_nodes = intent
        .graph()
        .nodes()
        .values()
        .filter(|node| declarations.get(&node.symbol) == Some(&SemanticSymbol("hidden".into())))
        .collect::<Vec<_>>();
    assert!(!hidden_nodes.is_empty());
    assert!(hidden_nodes.iter().all(|node| node.suppressed));

    let kept_nodes = intent
        .graph()
        .nodes()
        .values()
        .filter(|node| declarations.get(&node.symbol) == Some(&SemanticSymbol("kept".into())))
        .collect::<Vec<_>>();
    assert!(!kept_nodes.is_empty());
    assert!(kept_nodes.iter().all(|node| !node.suppressed));

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert_eq!(accepted.validation.point_count, 2);
    assert_eq!(accepted.validation.curve_count, 1);
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );

    let restored_project = CodeProject::managed(
        ProjectKey("m89-managed-direct-suppression".into()),
        direct_lines_compiled(false),
    )
    .unwrap();
    let restored_generated = reconciled(&restored_project);
    assert_eq!(generated.active(), restored_generated.active());
    let restored = materialize_code_project_cold(
        &restored_project,
        &restored_generated,
        IntentSessionId::from_raw(0x89_5102),
        DocumentId(PersistentId::from_u128(0x89_5102)),
        1.0,
    )
    .unwrap();
    assert!(
        restored
            .editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .all(|node| !node.suppressed),
        "the suppression-free compiled candidate must restore the declaration",
    );
    assert_eq!(
        materialized.expansion.semantic_outputs, restored.expansion.semantic_outputs,
        "suppression must not stale or reroute the unrelated retained output",
    );
    let restored_accepted = restored
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert_eq!(restored_accepted.validation.point_count, 4);
    assert_eq!(restored_accepted.validation.curve_count, 2);
    assert!(restored_accepted.validation.hard_residuals_validated);
    assert!(restored_accepted.validation.all_active_features_current);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps source suppression, native inactivity, restore, and overlay rejection together"
)]
fn exact_generated_suppression_is_source_owned_and_restorable() {
    let project = generated_fillets_project(true);
    let generated = reconciled(&project);
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_5201),
        DocumentId(PersistentId::from_u128(0x89_5201)),
        1.0,
    )
    .unwrap();
    let lower = materialized
        .expansion
        .generated_children
        .iter()
        .find(|child| {
            matches!(
                &child.address.owner.address,
                CodeOwnerAddress::GeneratedMember { address }
                    if address.member_key == ["lowerLeft"]
            )
        })
        .unwrap();
    let upper = materialized
        .expansion
        .generated_children
        .iter()
        .find(|child| {
            matches!(
                &child.address.owner.address,
                CodeOwnerAddress::GeneratedMember { address }
                    if address.member_key == ["upperRight"]
            )
        })
        .unwrap();
    assert!(lower.suppressed);
    assert!(!upper.suppressed);

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
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let generated_address = |child: &geosolve_sketch_code::ExpandedGeneratedChild| {
        let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address else {
            panic!("generated child must retain one generated member address")
        };
        address.clone()
    };
    let lower_feature = materialized.host_outputs[&generated_address(lower)][0]
        .owner
        .feature;
    let upper_feature = materialized.host_outputs[&generated_address(upper)][0]
        .owner
        .feature;
    assert!(accepted.features.feature(lower_feature).unwrap().suppressed);
    assert!(!accepted.features.feature(upper_feature).unwrap().suppressed);
    assert_eq!(
        accepted
            .computed
            .feature_evaluations()
            .iter()
            .filter(|evaluation| matches!(
                &evaluation.state,
                ComputedFeatureEvaluationState::Suppressed
            ))
            .count(),
        1,
    );
    assert_eq!(
        accepted
            .computed
            .feature_evaluations()
            .iter()
            .filter(|evaluation| matches!(
                &evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            ))
            .count(),
        1,
    );

    let restored_project = generated_fillets_project(false);
    let required = required_generated_members(&project)
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let restored_required = required_generated_members(&restored_project)
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        required, restored_required,
        "source reorder and suppression must preserve the exact semantic member inventory",
    );
    let restored = materialize_code_project_cold(
        &restored_project,
        &generated,
        IntentSessionId::from_raw(0x89_5202),
        DocumentId(PersistentId::from_u128(0x89_5202)),
        1.0,
    )
    .unwrap();
    assert!(
        restored
            .expansion
            .generated_children
            .iter()
            .all(|child| !child.suppressed),
    );
    let restored_accepted = restored
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(restored_accepted.validation.hard_residuals_validated);
    assert!(restored_accepted.validation.all_active_features_current);
    assert!(
        restored_accepted
            .features
            .features()
            .iter()
            .all(|feature| !feature.suppressed),
        "the suppression-free compiled candidate must restore the exact generated member",
    );
    assert_eq!(
        materialized.expansion.semantic_outputs, restored.expansion.semantic_outputs,
        "the unrelated panel and upper-right output routes must remain current",
    );

    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x89_5203)).unwrap();
    let expanded = expand_code_project(&restored_project, &generated, intent.identity()).unwrap();
    let mut overlay = CodeInteractionOverlay::empty();
    overlay
        .set_generated_child_suppressed(expanded.generated_children[0].address.clone(), true)
        .unwrap();
    assert!(matches!(
        expand_code_project_with_overlay(
            &restored_project,
            &generated,
            &overlay,
            intent.identity(),
        ),
        Err(CodeExpansionError::Unsupported(message))
            if message.contains("explicit source authority")
    ));
}

fn manifest_control<'a>(
    manifest: &'a geosolve_sketch_code::ManagedControlManifest,
    declaration: &str,
    path: &SemanticOutputPath,
) -> &'a geosolve_sketch_code::ManagedControl {
    manifest
        .controls
        .iter()
        .find(|control| {
            control.source.declaration == SemanticSymbol(declaration.into())
                && control.source.path == *path
        })
        .unwrap_or_else(|| panic!("missing managed control for `{declaration}`"))
}

#[test]
fn direct_shared_radius_manifest_uses_exact_runtime_fan_out() {
    let compiled = direct_shared_radius_compiled();
    let project = CodeProject::managed(
        ProjectKey("m89-managed-direct-runtime-controls".into()),
        compiled,
    )
    .unwrap();
    let generated = reconciled(&project);
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x89_5301)).unwrap();
    let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();

    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let shared_path = SemanticOutputPath::default();
    let shared = manifest_control(&manifest, "sharedRadius", &shared_path);
    assert!(matches!(
        shared.access,
        ManagedControlAccess::Editable { .. }
    ));
    assert_eq!(shared.source.source_text, "mm(4)");

    let actual = shared
        .consumers
        .iter()
        .map(|consumer| match &consumer.target {
            ManagedControlConsumerTarget::Declaration {
                declaration,
                family,
            } => (
                declaration.0.clone(),
                family.clone(),
                consumer.property.clone(),
            ),
            ManagedControlConsumerTarget::Generated { .. } => {
                panic!("direct radius fan-out must not invent a generated consumer")
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            (
                "hole".into(),
                "geometry.centerRadiusCircle".into(),
                SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]),
            ),
            (
                "radius".into(),
                "dimension.radius".into(),
                SemanticOutputPath(vec![ManagedPathSegment::Field("value".into())]),
            ),
        ])
    );
}

#[test]
fn generated_shared_radius_manifest_authenticates_exact_two_fillet_consumers() {
    let compiled = generated_fillets_compiled(false);
    let base = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::TypedPanel)
        .unwrap()
        .project();
    let project = CodeProject {
        project: ProjectKey("m89-managed-generated-runtime-controls".into()),
        managed: compiled.into_managed_document().unwrap(),
        custom_files: base.custom_files,
        artifacts: base.artifacts,
        lock: base.lock,
    };
    project.validate().unwrap();
    let generated = reconciled(&project);
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x89_5302)).unwrap();
    let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();

    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let radius_path = SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]);
    let radius = manifest_control(&manifest, "cornerFillets", &radius_path);
    assert!(matches!(
        radius.access,
        ManagedControlAccess::Editable { .. }
    ));
    let actual = radius
        .consumers
        .iter()
        .map(|consumer| match &consumer.target {
            ManagedControlConsumerTarget::Generated {
                address,
                identity,
                artifact_digest,
                family,
            } => (
                address.clone(),
                *identity,
                artifact_digest.clone(),
                family.clone(),
                consumer.property.clone(),
            ),
            ManagedControlConsumerTarget::Declaration { .. } => {
                panic!("patch radius fan-out must contain only generated consumers")
            }
        })
        .collect::<BTreeSet<_>>();
    let expected = expansion
        .generated_provenance
        .iter()
        .filter(|(address, _)| {
            address.invocation == "cornerFillets" && address.template == ["fillet"]
        })
        .map(|(address, provenance)| {
            (
                address.clone(),
                provenance.identity,
                provenance.artifact_digest.clone().unwrap(),
                "computed.fillet".into(),
                SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 2);
}

#[test]
fn managed_schemas_never_invent_editability_without_an_executed_consumer_edge() {
    assert!(
        CodeProject::managed(
            ProjectKey("m89-managed-direct-no-runtime-controls".into()),
            without_direct_value_consumers(direct_shared_radius_compiled()),
        )
        .is_err(),
        "incomplete direct runtime provenance must reject at compiler-envelope admission"
    );

    let base = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::TypedPanel)
        .unwrap()
        .project();
    let patch = CodeProject {
        project: ProjectKey("m89-managed-generated-no-runtime-controls".into()),
        managed: without_generated_value_consumers(generated_fillets_compiled(false))
            .into_managed_document()
            .unwrap(),
        custom_files: base.custom_files,
        artifacts: base.artifacts,
        lock: base.lock,
    };
    patch.validate().unwrap();
    let generated = reconciled(&patch);
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x89_5304)).unwrap();
    let expansion = expand_code_project(&patch, &generated, intent.identity()).unwrap();
    let manifest = managed_control_manifest(&patch, &expansion).unwrap();
    let radius_path = SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]);
    let radius = manifest_control(&manifest, "cornerFillets", &radius_path);
    assert!(radius.consumers.is_empty());
    assert!(matches!(
        radius.access,
        ManagedControlAccess::ReadOnly {
            reason: ManagedControlReadOnlyReason::UnprovenTransform,
            ..
        }
    ));
}
