// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, FeatureAuthoringOptions, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentNativeBinding, SelectionItem,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CANVAS_ADDITIONS_GROUP, CodeProject, CodeProjectDemoId, CompiledManagedSource,
    EditorBootstrapDeclaration, EditorDeclarationInsertionError,
    EditorSourceDeclarationClosureKind, ExpandedSemanticTarget, GeneratedMemberAddress,
    KeyedReconcileState, ManagedPathSegment, ManagedValue, ProjectKey, SemanticOutputPath,
    SemanticSymbol, bundled_code_project_demos, materialize_code_project_cold,
    prepare_editor_declaration_insertions, required_generated_members,
};
use geosolve_sketch_intent::{
    AggregateKind, ComputedFeatureKind, ConstraintKind, DimensionKind, GeometryRecipeKind,
    InputRole, InputSlot, IntentFieldKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
    IntentOperationOutput, IntentOperationOutputKind, IntentPatch, IntentPatchOperation,
    IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit, LeafField,
    OperationKind, ParameterIntentKind, PatchPortRef,
};

const EMPTY_MANAGED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/geosolve-sketch-code/test/fixtures/managed-clean-empty.json"
));

const LINE_MANAGED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/geosolve-sketch-code/test/fixtures/managed-clean-segment.json"
));

const COMPACT_POLYLINE_ENVELOPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/geosolve-sketch-code/test/fixtures/managed-polyline.json"
));

fn length(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn dimensionless(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Dimensionless,
    }
}

fn enum_literal(value: &str) -> IntentLiteral {
    IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new(value).expect("fixture enum"))
}

fn managed_source_value(value: &ManagedValue) -> String {
    match value {
        ManagedValue::Null => "null".into(),
        ManagedValue::Bool(value) => value.to_string(),
        ManagedValue::Number(value) => serde_json::to_string(value).expect("finite managed number"),
        ManagedValue::String(value) => {
            serde_json::to_string(value).expect("managed string literal")
        }
        ManagedValue::Unit(unit) => format!(
            "{}({})",
            unit.unit,
            serde_json::to_string(&unit.value).expect("finite managed unit value"),
        ),
        ManagedValue::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(managed_source_value)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        ManagedValue::Object(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    serde_json::to_string(key).expect("managed object key"),
                    managed_source_value(value),
                ))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        ManagedValue::Reference { declaration, path } => {
            let mut source = declaration.0.clone();
            for segment in &path.0 {
                match segment {
                    ManagedPathSegment::Field(field) => {
                        source.push('.');
                        source.push_str(field);
                    }
                    ManagedPathSegment::Index(index) => {
                        write!(source, "[{index}]")
                            .expect("writing managed source to a String cannot fail");
                    }
                    ManagedPathSegment::Member { member } => {
                        source.push('[');
                        source.push_str(
                            &serde_json::to_string(member).expect("managed member literal"),
                        );
                        source.push(']');
                    }
                }
            }
            source
        }
    }
}

fn managed_source_declaration(
    declaration: &geosolve_sketch_code::EditorSourceDeclarationDraft,
) -> String {
    format!(
        "  const {} = $.{}({}, {});\n",
        declaration.variable,
        declaration.builder_path.join("."),
        serde_json::to_string(&declaration.symbol.0).expect("managed declaration symbol"),
        managed_source_value(&declaration.arguments),
    )
}

fn accepted_empty_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    geosolve_constraint_editor::ProjectionalEditorSession,
) {
    let compiled =
        CompiledManagedSource::from_json(EMPTY_MANAGED).expect("empty compiler envelope");
    let project = CodeProject::managed(ProjectKey("m89-editor-insertion".into()), compiled)
        .expect("empty managed project");
    let desired = required_generated_members(&project).expect("generated member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("empty reconcile plan")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_1a5e),
        DocumentId(PersistentId::from_u128(0x89_1a5e)),
        1.0,
    )
    .expect("accepted empty code project");
    (project, materialized.expansion, materialized.editor)
}

fn accepted_line_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    geosolve_constraint_editor::ProjectionalEditorSession,
) {
    let compiled = CompiledManagedSource::from_json(LINE_MANAGED).expect("line compiler envelope");
    let project = CodeProject::managed(ProjectKey("m89-editor-reference".into()), compiled)
        .expect("line managed project");
    let desired = required_generated_members(&project).expect("generated member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("line reconcile plan")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_1a6e),
        DocumentId(PersistentId::from_u128(0x89_1a6e)),
        1.0,
    )
    .expect("accepted line code project");
    (project, materialized.expansion, materialized.editor)
}

fn accepted_compact_polyline_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    geosolve_constraint_editor::ProjectionalEditorSession,
) {
    let compiled = CompiledManagedSource::from_json(COMPACT_POLYLINE_ENVELOPE)
        .expect("compact Polyline compiler envelope");
    let project =
        CodeProject::managed(ProjectKey("m89-compact-polyline-reopened".into()), compiled)
            .expect("compact Polyline managed project");
    let desired = required_generated_members(&project).expect("generated member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("compact Polyline reconcile plan")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_f002_0004),
        DocumentId(PersistentId::from_u128(0x89_f002_0004)),
        1.0,
    )
    .expect("accepted compact Polyline code project");
    (project, materialized.expansion, materialized.editor)
}

fn accepted_right_angle_polyline_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    geosolve_constraint_editor::ProjectionalEditorSession,
) {
    accepted_compact_polyline_code_project()
}

fn add_segment(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    ordinal: u64,
) -> geosolve_sketch_intent::NodeId {
    let selector = |role| IntentPortSelector::Node { role, index: 0 };
    let alias =
        geosolve_sketch_intent::IntentKey::new(format!("canvas.segment.gesture{ordinal}")).unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        geosolve_sketch_intent::IntentKey::new(format!("canvas.segment{ordinal}")).unwrap(),
    )
    .with_instance_leaf(selector(IntentPortRole::Start), LeafField::X, length(-2.5))
    .with_instance_leaf(selector(IntentPortRole::Start), LeafField::Y, length(3.25))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(7.75))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(-1.5));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("segment authoring patch");
    outcome.aliases.node(&alias).expect("new segment node")
}

fn add_segment_from_existing_end(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let selector = |role| IntentPortSelector::Node { role, index: 0 };
    let start = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(selector(IntentPortRole::End))
        .expect("base line end")
        .as_ref(base);
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.connected.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.connected").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: start },
    )
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(4.0))
    .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(3.0));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("connected segment authoring patch");
    outcome
        .aliases
        .node(&alias)
        .expect("new connected segment node")
}

fn add_computed_fillet_at_node_point(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    node: geosolve_sketch_intent::NodeId,
    selector: IntentPortSelector,
    radius: f64,
) -> geosolve_sketch_intent::NodeId {
    let point = editor
        .coordinator()
        .intent()
        .graph()
        .node(node)
        .expect("Fillet point owner")
        .port_by_selector(selector)
        .expect("Fillet point port")
        .as_ref(node);
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .expect("Fillet accepted parent authority");
    let IntentNativeBinding::Point(point) = accepted
        .ownership
        .port(point)
        .expect("Fillet point native owner")
    else {
        panic!("Fillet selection must resolve to one native point")
    };
    let before = editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut state = FeatureAuthoringState::default();
    let symbol = geosolve_sketch_intent::IntentKey::new("Canvas Fillet").unwrap();
    assert!(matches!(
        editor
            .activate_feature_authoring(
                &mut state,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(radius),
                    ..FeatureAuthoringOptions::default()
                },
                &[(SelectionItem::Point(point), None)],
                symbol.clone(),
            )
            .expect("Fillet preview activation"),
        FeatureAuthoringOutcome::PreviewRequested { .. }
    ));
    editor
        .apply_computed_fillet_preview(&mut state, symbol)
        .expect("computed Fillet publication");
    editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .find_map(|candidate| {
            (!before.contains(&candidate.id)
                && matches!(
                    candidate.kind,
                    IntentNodeKind::ComputedFeature {
                        feature: ComputedFeatureKind::FilletSet
                    }
                ))
            .then_some(candidate.id)
        })
        .expect("new computed Fillet declaration")
}

fn add_polyline(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    first_vertex_source: Option<geosolve_sketch_intent::NodeId>,
) -> geosolve_sketch_intent::NodeId {
    let vertex = |ordinal| IntentPortSelector::InitialChild {
        ordinal,
        role: IntentPortRole::Corner,
        index: 0,
    };
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.polyline.gesture").unwrap();
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.polyline").unwrap(),
    )
    .with_dynamic_children(3)
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("closed").unwrap()),
        IntentLiteral::Boolean(false),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("role").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("profile").unwrap()),
    )
    .with_instance_leaf(vertex(1), LeafField::X, length(6.0))
    .with_instance_leaf(vertex(1), LeafField::Y, length(3.0))
    .with_instance_leaf(vertex(2), LeafField::X, length(6.0))
    .with_instance_leaf(vertex(2), LeafField::Y, length(-4.0));
    if let Some(source) = first_vertex_source {
        let endpoint = editor
            .coordinator()
            .intent()
            .graph()
            .node(source)
            .expect("Polyline lexical source")
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            })
            .expect("Polyline lexical source endpoint")
            .as_ref(source);
        draft = draft.with_input(
            InputSlot::new(InputRole::Point, 0),
            PatchPortRef::Stable { port: endpoint },
        );
    } else {
        draft = draft
            .with_instance_leaf(vertex(0), LeafField::X, length(-3.0))
            .with_instance_leaf(vertex(0), LeafField::Y, length(3.0));
    }
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("Polyline authoring patch");
    outcome.aliases.node(&alias).expect("new Polyline node")
}

fn add_polyline_without_explicit_role(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
) -> geosolve_sketch_intent::NodeId {
    let vertex = |ordinal| IntentPortSelector::InitialChild {
        ordinal,
        role: IntentPortRole::Corner,
        index: 0,
    };
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.fallback.polyline").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.fallback.polyline.node").unwrap(),
    )
    .with_dynamic_children(2)
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("closed").unwrap()),
        IntentLiteral::Boolean(false),
    )
    .with_instance_leaf(vertex(0), LeafField::X, length(0.0))
    .with_instance_leaf(vertex(0), LeafField::Y, length(0.0))
    .with_instance_leaf(vertex(1), LeafField::X, length(2.0))
    .with_instance_leaf(vertex(1), LeafField::Y, length(1.0));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("fallback Polyline authoring patch");
    outcome
        .aliases
        .node(&alias)
        .expect("fallback Polyline node")
}

fn add_polyline_axis_constraint(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    polyline: geosolve_sketch_intent::NodeId,
    ordinal: u16,
    constraint: ConstraintKind,
) -> geosolve_sketch_intent::NodeId {
    assert!(matches!(
        constraint,
        ConstraintKind::Horizontal | ConstraintKind::Vertical
    ));
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(polyline)
        .expect("Polyline")
        .port_by_selector(IntentPortSelector::InitialChild {
            ordinal,
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("Polyline span")
        .as_ref(polyline);
    let family = if constraint == ConstraintKind::Horizontal {
        "horizontal"
    } else {
        "vertical"
    };
    let alias = geosolve_sketch_intent::IntentKey::new(format!("canvas.{family}.gesture")).unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint { constraint },
        geosolve_sketch_intent::IntentKey::new(format!("canvas.{family}")).unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    );
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("Polyline axis-constraint patch");
    outcome
        .aliases
        .node(&alias)
        .expect("new Polyline axis constraint")
}

fn add_sketch_point(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
) -> geosolve_sketch_intent::NodeId {
    let selector = IntentPortSelector::Node {
        role: IntentPortRole::Primary,
        index: 0,
    };
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.point.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.point").unwrap(),
    )
    .with_instance_leaf(selector, LeafField::X, length(1.25))
    .with_instance_leaf(selector, LeafField::Y, length(-6.5));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("point authoring patch");
    outcome.aliases.node(&alias).expect("new point node")
}

fn add_horizontal_constraint(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("base span")
        .as_ref(base);
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.horizontal.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::Horizontal,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.horizontal").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    );
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("horizontal authoring patch");
    outcome.aliases.node(&alias).expect("new horizontal node")
}

fn add_suppressed_horizontal_constraint(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("base span")
        .as_ref(base);
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.generated.horizontal").unwrap();
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::Horizontal,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.generated.horizontal.node").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    );
    draft.suppressed = true;
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("suppressed horizontal authoring patch");
    outcome
        .aliases
        .node(&alias)
        .expect("new suppressed horizontal node")
}

fn add_suppressed_parallel_constraint(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("base span")
        .as_ref(base);
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.parallel.gesture").unwrap();
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Constraint {
            constraint: ConstraintKind::Parallel,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.parallel").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        PatchPortRef::Stable { port: span },
    );
    draft.suppressed = true;
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("suppressed parallel authoring patch");
    outcome.aliases.node(&alias).expect("new parallel node")
}

fn add_parameter(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
) -> geosolve_sketch_intent::NodeId {
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.parameter.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Parameter {
            parameter: ParameterIntentKind::Parameter,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.parameter").unwrap(),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("kind").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("length").unwrap()),
    );
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("parameter authoring patch");
    outcome.aliases.node(&alias).expect("new parameter node")
}

fn add_segment_from_point(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    point: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let source = editor
        .coordinator()
        .intent()
        .graph()
        .node(point)
        .expect("point node")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        })
        .expect("point primary port")
        .as_ref(point);
    let end = IntentPortSelector::Node {
        role: IntentPortRole::End,
        index: 0,
    };
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.point.segment.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.point.segment").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable { port: source },
    )
    .with_instance_leaf(end, LeafField::X, length(9.0))
    .with_instance_leaf(end, LeafField::Y, length(2.0));
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("point-referenced segment authoring patch");
    outcome
        .aliases
        .node(&alias)
        .expect("new point-referenced segment node")
}

fn add_reference_length(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("base span")
        .as_ref(base);
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.length.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Dimension {
            dimension: DimensionKind::CurveLength,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.length").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("mode").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("reference").unwrap()),
    );
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("reference length authoring patch");
    outcome.aliases.node(&alias).expect("new dimension node")
}

fn add_open_chain_profile_offset(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    base: geosolve_sketch_intent::NodeId,
) -> (
    geosolve_sketch_intent::NodeId,
    geosolve_sketch_intent::NodeId,
) {
    let span = editor
        .coordinator()
        .intent()
        .graph()
        .node(base)
        .expect("base line")
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index: 0,
        })
        .expect("base span")
        .as_ref(base);
    let helper_alias = geosolve_sketch_intent::IntentKey::new("canvas.offset.open-chain").unwrap();
    let root_alias = geosolve_sketch_intent::IntentKey::new("canvas.offset.operation").unwrap();
    let helper = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::OpenChain,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.offset.helper").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span },
    );
    let root = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::ProfileOffset,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.offset").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Chain, 0),
        PatchPortRef::Alias {
            node: helper_alias.clone(),
            selector: IntentPortSelector::Node {
                role: IntentPortRole::Chain,
                index: 0,
            },
        },
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("distance").unwrap()),
        length(1.0),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("side").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("left").unwrap()),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("first_traversal").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("forward").unwrap()),
    )
    .with_operation_outputs(vec![
        IntentOperationOutput::native(IntentOperationOutputKind::Point),
        IntentOperationOutput::native(IntentOperationOutputKind::Point),
        IntentOperationOutput::curve(1),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Dimension),
    ]);
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: helper_alias.clone(),
                    draft: Box::new(helper),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: root_alias.clone(),
                    draft: Box::new(root),
                    cell: None,
                },
            ],
        ))
        .expect("open-chain Profile Offset authoring patch");
    (
        outcome
            .aliases
            .node(&helper_alias)
            .expect("new Profile Offset helper"),
        outcome
            .aliases
            .node(&root_alias)
            .expect("new Profile Offset root"),
    )
}

fn add_rectangle_operation(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
) -> geosolve_sketch_intent::NodeId {
    let mut outputs = Vec::new();
    outputs.extend((0..4).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Point)));
    outputs.extend((0..4).map(|_| IntentOperationOutput::curve(1)));
    outputs.extend(
        (0..5).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Constraint)),
    );
    outputs
        .extend((0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Scalar)));
    outputs.extend(
        (0..2).map(|_| IntentOperationOutput::native(IntentOperationOutputKind::Dimension)),
    );
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.rectangle.gesture").unwrap();
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::Rectangle,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.rectangle").unwrap(),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("origin").unwrap()),
        IntentLiteral::Point([10.0, 12.0]),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("width").unwrap()),
        length(8.0),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("height").unwrap()),
        length(5.0),
    )
    .with_field(
        IntentFieldKey(geosolve_sketch_intent::IntentKey::new("role").unwrap()),
        IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("profile").unwrap()),
    )
    .with_operation_outputs(outputs);
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("rectangle operation patch");
    outcome.aliases.node(&alias).expect("new operation node")
}

fn add_associative_fillet_operation(
    editor: &mut geosolve_constraint_editor::ProjectionalEditorSession,
    polyline: geosolve_sketch_intent::NodeId,
) -> geosolve_sketch_intent::NodeId {
    let span = |ordinal| {
        editor
            .coordinator()
            .intent()
            .graph()
            .node(polyline)
            .expect("Fillet Polyline")
            .port_by_selector(IntentPortSelector::InitialChild {
                ordinal,
                role: IntentPortRole::Span,
                index: 0,
            })
            .expect("Fillet Polyline span")
            .as_ref(polyline)
    };
    let alias = geosolve_sketch_intent::IntentKey::new("canvas.associative-fillet").unwrap();
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Operation {
            operation: OperationKind::AssociativeFillet,
        },
        geosolve_sketch_intent::IntentKey::new("canvas.associative-fillet.node").unwrap(),
    )
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Stable { port: span(0) },
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        PatchPortRef::Stable { port: span(1) },
    )
    .with_operation_outputs(vec![
        IntentOperationOutput::native(IntentOperationOutputKind::Point),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::curve(1),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Contact),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Contact),
        IntentOperationOutput::native(IntentOperationOutputKind::Constraint),
        IntentOperationOutput::native(IntentOperationOutputKind::Scalar),
        IntentOperationOutput::native(IntentOperationOutputKind::Dimension),
    ]);
    for (name, value) in [
        ("radius", length(1.0)),
        ("radius_mode", enum_literal("driving")),
        ("endpoint_order", enum_literal("first_then_second")),
        ("sweep", enum_literal("counter_clockwise")),
        ("first_parameter", dimensionless(0.75)),
        ("first_winding", IntentLiteral::Integer(0)),
        ("first_neighborhood", enum_literal("local")),
        ("first_local_lower", dimensionless(0.5)),
        ("first_local_upper", dimensionless(0.95)),
        ("first_normal_side", enum_literal("left")),
        ("first_trim_endpoint", enum_literal("end")),
        ("first_periodic_anchor", IntentLiteral::Boolean(false)),
        ("second_parameter", dimensionless(0.25)),
        ("second_winding", IntentLiteral::Integer(0)),
        ("second_neighborhood", enum_literal("local")),
        ("second_local_lower", dimensionless(0.05)),
        ("second_local_upper", dimensionless(0.5)),
        ("second_normal_side", enum_literal("left")),
        ("second_trim_endpoint", enum_literal("start")),
        ("second_periodic_anchor", IntentLiteral::Boolean(false)),
    ] {
        draft = draft.with_field(
            IntentFieldKey(geosolve_sketch_intent::IntentKey::new(name).unwrap()),
            value,
        );
    }
    let outcome = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .expect("Associative Fillet authoring patch");
    outcome
        .aliases
        .node(&alias)
        .expect("new Associative Fillet operation")
}

#[test]
fn accepted_canvas_segment_prepares_digest_bound_source_insertion_without_source_spans() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let segment = add_segment(&mut candidate, 1);

    let accepted_materialization = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted segment materialization");
    assert!(accepted_materialization.validation.hard_residuals_validated);
    assert!(
        accepted_materialization
            .validation
            .all_active_features_current
    );
    assert!(
        accepted_materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            segment,
            SemanticSymbol("segment1".into()),
        )],
    )
    .expect("source insertion plan");
    assert_eq!(plan.project, project.project);
    assert_eq!(plan.source_digest, project.managed.source_digest);
    assert_eq!(plan.expansion_digest, expansion.digest);
    let [declaration] = plan.declarations.as_slice() else {
        panic!("one segment declaration expected")
    };
    assert_eq!(declaration.node, segment);
    assert_eq!(declaration.variable, "segment1");
    assert_eq!(declaration.symbol, SemanticSymbol("segment1".into()));
    assert_eq!(declaration.builder_path, ["geometry", "segment"]);
    assert_eq!(declaration.group, CANVAS_ADDITIONS_GROUP);
    assert!(!declaration.suppressed);
    assert_eq!(
        declaration.arguments,
        ManagedValue::Object(std::collections::BTreeMap::from([
            (
                "end".into(),
                ManagedValue::Array(vec![ManagedValue::Number(7.75), ManagedValue::Number(-1.5)]),
            ),
            (
                "label".into(),
                ManagedValue::String("canvas.segment1".into()),
            ),
            (
                "start".into(),
                ManagedValue::Array(vec![ManagedValue::Number(-2.5), ManagedValue::Number(3.25)]),
            ),
        ])),
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exact compact-source regression keeps the Polyline vertices, both inferred axis references, size ceiling, and accepted native invariants together"
)]
fn canvas_polyline_and_inferred_axes_project_to_compact_semantic_declarations() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let polyline = add_polyline(&mut candidate, None);
    let horizontal =
        add_polyline_axis_constraint(&mut candidate, polyline, 0, ConstraintKind::Horizontal);
    let vertical =
        add_polyline_axis_constraint(&mut candidate, polyline, 1, ConstraintKind::Vertical);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(polyline, SemanticSymbol("geometry1".into())),
            EditorBootstrapDeclaration::new(horizontal, SemanticSymbol("constraint2".into())),
            EditorBootstrapDeclaration::new(vertical, SemanticSymbol("constraint3".into())),
        ],
    )
    .expect("compact Polyline source insertion plan");

    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| (
                declaration.symbol.0.as_str(),
                declaration.builder_path.join(".")
            ))
            .collect::<Vec<_>>(),
        [
            ("geometry1", "geometry.polyline".to_owned()),
            ("constraint2", "constraint.horizontal".to_owned()),
            ("constraint3", "constraint.vertical".to_owned()),
        ]
    );
    assert!(plan.declaration_closures.is_empty());
    let ManagedValue::Object(polyline_arguments) = &plan.declarations[0].arguments else {
        panic!("Polyline arguments must be an object")
    };
    assert_eq!(
        polyline_arguments,
        &BTreeMap::from([
            ("closed".into(), ManagedValue::Bool(false)),
            (
                "label".into(),
                ManagedValue::String("canvas.polyline".into()),
            ),
            ("role".into(), ManagedValue::String("profile".into())),
            (
                "vertices".into(),
                ManagedValue::Array(vec![
                    ManagedValue::Object(BTreeMap::from([
                        ("key".into(), ManagedValue::String("v0".into())),
                        (
                            "position".into(),
                            ManagedValue::Array(vec![
                                ManagedValue::Number(-3.0),
                                ManagedValue::Number(3.0),
                            ]),
                        ),
                    ])),
                    ManagedValue::Object(BTreeMap::from([
                        ("key".into(), ManagedValue::String("v1".into())),
                        (
                            "position".into(),
                            ManagedValue::Array(vec![
                                ManagedValue::Number(6.0),
                                ManagedValue::Number(3.0),
                            ]),
                        ),
                    ])),
                    ManagedValue::Object(BTreeMap::from([
                        ("key".into(), ManagedValue::String("v2".into())),
                        (
                            "position".into(),
                            ManagedValue::Array(vec![
                                ManagedValue::Number(6.0),
                                ManagedValue::Number(-4.0),
                            ]),
                        ),
                    ])),
                ]),
            ),
        ])
    );
    for (index, (owner, segment)) in [(1, ("constraint2", "v0")), (2, ("constraint3", "v1"))] {
        let ManagedValue::Object(arguments) = &plan.declarations[index].arguments else {
            panic!("axis-constraint arguments must be an object")
        };
        assert_eq!(
            arguments["span"],
            ManagedValue::Reference {
                declaration: SemanticSymbol("geometry1".into()),
                path: SemanticOutputPath(vec![
                    ManagedPathSegment::Field("segments".into()),
                    ManagedPathSegment::Field("byKey".into()),
                    ManagedPathSegment::Field(segment.into()),
                ]),
            },
            "{owner} must retain one concise keyed lexical curve reference",
        );
    }

    let compact = serde_json::to_string(&plan).expect("compact plan JSON");
    let projected_calls = plan
        .declarations
        .iter()
        .map(|declaration| format!("$.{}", declaration.builder_path.join(".")))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!projected_calls.contains("$.intent.recipe"));
    assert!(!compact.contains("geosolve-intent-recipe-v1"));
    assert!(!compact.contains("initialInstance"));
    assert!(
        compact.len() < 2_048,
        "three concise semantic declarations should stay below 2 KiB, got {} bytes",
        compact.len(),
    );
    let accepted_materialization = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted Polyline plus inferred axes");
    assert!(accepted_materialization.validation.hard_residuals_validated);
    assert!(
        accepted_materialization
            .validation
            .all_active_features_current
    );
    assert!(
        accepted_materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn compact_polyline_preserves_an_input_bound_vertex_as_a_lexical_point_reference() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base line")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let polyline = add_polyline(&mut candidate, Some(base));
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            polyline,
            SemanticSymbol("geometry1".into()),
        )],
    )
    .expect("input-bound compact Polyline insertion plan");
    assert_eq!(plan.declarations[0].builder_path, ["geometry", "polyline"]);
    let ManagedValue::Object(arguments) = &plan.declarations[0].arguments else {
        panic!("Polyline arguments must be an object")
    };
    let ManagedValue::Array(vertices) = &arguments["vertices"] else {
        panic!("Polyline vertices must be an array")
    };
    let ManagedValue::Object(first) = &vertices[0] else {
        panic!("Polyline vertex must be an object")
    };
    assert_eq!(
        first["position"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("base".into()),
            path: SemanticOutputPath(vec![ManagedPathSegment::Field("end".into())]),
        }
    );
}

#[test]
fn reopened_named_polyline_remains_a_source_owner_for_a_later_canvas_gesture() {
    let (project, expansion, accepted) = accepted_compact_polyline_code_project();
    let polyline_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("geometry1".into())).then_some(alias)
        })
        .expect("compact Polyline declaration provenance");
    let polyline = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(polyline_alias)
        .expect("reopened compact Polyline node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("follow-up candidate editor fork");
    let horizontal =
        add_polyline_axis_constraint(&mut candidate, polyline, 0, ConstraintKind::Horizontal);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            horizontal,
            SemanticSymbol("constraint4".into()),
        )],
    )
    .expect("the reopened direct Polyline must remain source-addressable");

    assert_eq!(plan.declarations.len(), 1);
    assert_eq!(
        plan.declarations[0].builder_path,
        ["constraint", "horizontal"]
    );
    let ManagedValue::Object(arguments) = &plan.declarations[0].arguments else {
        panic!("follow-up Horizontal arguments must be an object")
    };
    assert_eq!(
        arguments["span"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("geometry1".into()),
            path: SemanticOutputPath(vec![
                ManagedPathSegment::Field("segments".into()),
                ManagedPathSegment::Field("byKey".into()),
                ManagedPathSegment::Field("v0".into()),
            ]),
        },
        "the second gesture must reuse the published Polyline's authenticated lexical span key",
    );
    let accepted_materialization = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted follow-up Horizontal");
    assert!(accepted_materialization.validation.hard_residuals_validated);
    assert!(
        accepted_materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn polyline_without_an_explicit_role_still_uses_the_named_polyline_builder() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let polyline = add_polyline_without_explicit_role(&mut candidate);
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            polyline,
            SemanticSymbol("geometry1".into()),
        )],
    )
    .expect("named geometry insertion plan");
    assert_eq!(plan.declarations[0].builder_path, ["geometry", "polyline"]);
    let encoded = serde_json::to_string(&plan.declarations[0]).expect("named Polyline declaration");
    for forbidden in [
        "recipe", "fields", "values", "results", "outputs", "editLens",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}: {encoded}");
    }
}

#[test]
fn insertion_rejects_an_unselected_gui_only_addition_as_one_atomic_delta() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let first = add_segment(&mut candidate, 1);
    let second = add_segment(&mut candidate, 2);
    assert_ne!(first, second);

    let error = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            first,
            SemanticSymbol("segment1".into()),
        )],
    )
    .expect_err("the full accepted graph delta must be represented");
    assert_eq!(
        error,
        EditorDeclarationInsertionError::UnselectedAddition(second)
    );
    assert_eq!(
        project.managed.source,
        CompiledManagedSource::from_json(EMPTY_MANAGED)
            .expect("empty compiler envelope")
            .normalized_source,
    );
}

#[test]
fn canvas_segment_dependency_resolves_only_through_accepted_lexical_provenance() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let connected = add_segment_from_existing_end(&mut candidate, base);
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            connected,
            SemanticSymbol("segment2".into()),
        )],
    )
    .expect("connected source insertion plan");
    let ManagedValue::Object(arguments) = &plan.declarations[0].arguments else {
        panic!("line arguments must be an object")
    };
    assert_eq!(
        arguments["start"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("base".into()),
            path: geosolve_sketch_code::SemanticOutputPath(vec![
                geosolve_sketch_code::ManagedPathSegment::Field("end".into()),
            ]),
        }
    );
    assert_eq!(
        arguments["end"],
        ManagedValue::Array(vec![ManagedValue::Number(4.0), ManagedValue::Number(3.0)])
    );
}

#[test]
fn sketch_point_uses_the_named_geometry_builder() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let point = add_sketch_point(&mut candidate);
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            point,
            SemanticSymbol("point3".into()),
        )],
    )
    .expect("named point insertion plan");
    let declaration = &plan.declarations[0];
    assert_eq!(declaration.builder_path, ["geometry", "sketchPoint"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("named sketch-point arguments must be an object")
    };
    assert_eq!(
        arguments["point"],
        ManagedValue::Array(vec![ManagedValue::Number(1.25), ManagedValue::Number(-6.5)])
    );
    for forbidden in ["kind", "inputs", "fields", "values", "results", "outputs"] {
        assert!(!arguments.contains_key(forbidden));
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one inventory test compares representative constraint, dimension, and operation reverse projections"
)]
fn constraint_dimension_and_operation_use_named_typed_builders() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let horizontal = add_horizontal_constraint(&mut candidate, base);
    let dimension = add_reference_length(&mut candidate, base);
    let operation = add_rectangle_operation(&mut candidate);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(horizontal, SemanticSymbol("horizontal4".into())),
            EditorBootstrapDeclaration::new(dimension, SemanticSymbol("length5".into())),
            EditorBootstrapDeclaration::new(operation, SemanticSymbol("rectangle6".into())),
        ],
    )
    .expect("three-family insertion plan");
    assert_eq!(plan.declarations.len(), 3);
    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| declaration.builder_path.join("."))
            .collect::<Vec<_>>(),
        [
            "constraint.horizontal",
            "dimension.curveLength",
            "operation.rectangle"
        ]
    );
    let ManagedValue::Object(horizontal_arguments) = &plan.declarations[0].arguments else {
        panic!("horizontal arguments must be an object")
    };
    assert_eq!(
        horizontal_arguments["span"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("base".into()),
            path: SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
        }
    );
    let ManagedValue::Object(dimension_arguments) = &plan.declarations[1].arguments else {
        panic!("named dimension arguments must be an object")
    };
    assert_eq!(
        dimension_arguments["curve"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("base".into()),
            path: SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
        }
    );
    assert!(matches!(
        dimension_arguments["value"],
        ManagedValue::Unit(_)
    ));
    let ManagedValue::Object(operation_arguments) = &plan.declarations[2].arguments else {
        panic!("named operation arguments must be an object")
    };
    assert!(matches!(
        operation_arguments["origin"],
        ManagedValue::Array(_)
    ));
    assert!(matches!(
        operation_arguments["width"],
        ManagedValue::Unit(_)
    ));
    assert!(matches!(
        operation_arguments["height"],
        ManagedValue::Unit(_)
    ));
    let encoded = serde_json::to_string(&plan).expect("named terminal declarations");
    for forbidden in [
        "recipe",
        "operationOutputs",
        "\"inputs\"",
        "\"fields\"",
        "\"values\"",
        "\"results\"",
        "\"outputs\"",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}: {encoded}");
    }

    let accepted_materialization = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted multi-family materialization");
    assert!(accepted_materialization.validation.hard_residuals_validated);
    assert!(
        accepted_materialization
            .validation
            .all_active_features_current
    );
    assert!(
        accepted_materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owner regression keeps reverse projection and independently validated cold replay together"
)]
fn accepted_parallel_constraint_uses_named_typed_source() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let parallel = add_suppressed_parallel_constraint(&mut candidate, base);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            parallel,
            SemanticSymbol("parallel2".into()),
        )],
    )
    .expect("Parallel insertion plan");
    let [declaration] = plan.declarations.as_slice() else {
        panic!("one Parallel declaration expected")
    };
    assert_eq!(declaration.builder_path, ["constraint", "parallel"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("named constraint arguments must be an object")
    };
    assert_eq!(arguments.len(), 3);
    assert_eq!(
        arguments["label"],
        ManagedValue::String("canvas.parallel".into())
    );
    assert!(matches!(arguments["first"], ManagedValue::Reference { .. }));
    assert!(matches!(
        arguments["second"],
        ManagedValue::Reference { .. }
    ));
    assert!(declaration.suppressed);
    let encoded = serde_json::to_string(declaration).expect("named Parallel declaration");
    for forbidden in ["recipe", "inputs", "fields", "values", "results", "outputs"] {
        assert!(!encoded.contains(forbidden), "{forbidden}: {encoded}");
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one branch-explicit regression keeps the complete associative Fillet named source adjacent"
)]
fn associative_fillet_operation_reverse_projects_to_named_branch_state() {
    let (project, expansion, accepted) = accepted_right_angle_polyline_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("geometry1".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base Polyline intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let fillet = add_associative_fillet_operation(&mut candidate, base);

    let insertion = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            fillet,
            SemanticSymbol("legacyFillet1".into()),
        )],
    )
    .expect("Associative Fillet reverse projection");
    let [declaration] = insertion.declarations.as_slice() else {
        panic!("one Associative Fillet declaration expected")
    };
    assert_eq!(declaration.builder_path, ["operation", "associativeFillet"]);
    assert!(!declaration.suppressed);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("Associative Fillet arguments must be an object")
    };
    let ManagedValue::Array(parents) = &arguments["parents"] else {
        panic!("Associative Fillet parents must be an array")
    };
    assert_eq!(parents.len(), 2);
    let lexical_spans = parents
        .iter()
        .map(|parent| {
            let ManagedValue::Object(parent) = parent else {
                panic!("Associative Fillet parent must be an object")
            };
            let ManagedValue::Reference { declaration, path } = &parent["span"] else {
                panic!("Associative Fillet parent must retain one lexical span")
            };
            (declaration.clone(), path.clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lexical_spans,
        vec![
            (
                SemanticSymbol("geometry1".into()),
                SemanticOutputPath(vec![
                    ManagedPathSegment::Field("segments".into()),
                    ManagedPathSegment::Field("byKey".into()),
                    ManagedPathSegment::Field("v0".into()),
                ]),
            ),
            (
                SemanticSymbol("geometry1".into()),
                SemanticOutputPath(vec![
                    ManagedPathSegment::Field("segments".into()),
                    ManagedPathSegment::Field("byKey".into()),
                    ManagedPathSegment::Field("v1".into()),
                ]),
            ),
        ]
    );
    assert!(matches!(arguments["radius"], ManagedValue::Unit(_)));
    assert_eq!(
        arguments["radiusMode"],
        ManagedValue::String("driving".into())
    );
    assert_eq!(
        arguments["endpointOrder"],
        ManagedValue::String("firstThenSecond".into())
    );
    assert_eq!(
        arguments["sweep"],
        ManagedValue::String("counterClockwise".into())
    );

    let original_node = candidate
        .coordinator()
        .intent()
        .graph()
        .node(fillet)
        .expect("accepted Associative Fillet node");
    assert!(original_node.operation_outputs.len() >= 2);
    let accepted = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted Associative Fillet authority");
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
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        accepted
            .session
            .design_document()
            .constraints()
            .iter()
            .any(|constraint| matches!(
                constraint.definition,
                geosolve_sketch::DocumentConstraintDefinition::CurveCurveFillet { .. }
            ))
    );

    let source = managed_source_declaration(declaration);
    assert!(source.contains("$.operation.associativeFillet"));
    for forbidden in [
        ".recipe",
        "operationOutputs",
        "inputs",
        "fields",
        "values",
        "results",
        "outputs",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}: {source}");
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps Profile Offset root and helper closure assertions together"
)]
fn profile_offset_reverse_projection_retains_helpers_under_one_explicit_operation_closure() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let (helper, root) = add_open_chain_profile_offset(&mut candidate, base);

    // Supply root first to prove dependency ordering and closure ownership are
    // semantic rather than inferred from the caller's list adjacency.
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(root, SemanticSymbol("profileOffset12".into())),
            EditorBootstrapDeclaration::new(helper, SemanticSymbol("offsetChain11".into())),
        ],
    )
    .expect("Profile Offset source insertion plan");

    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| (declaration.node, declaration.symbol.0.as_str()))
            .collect::<Vec<_>>(),
        [(helper, "offsetChain11"), (root, "profileOffset12")]
    );
    let [closure] = plan.declaration_closures.as_slice() else {
        panic!("one Profile Offset declaration closure expected")
    };
    assert_eq!(
        closure.kind,
        EditorSourceDeclarationClosureKind::ProfileOffset
    );
    assert_eq!(closure.root.node, root);
    assert_eq!(
        closure.root.symbol,
        SemanticSymbol("profileOffset12".into())
    );
    let [closure_helper] = closure.helpers.as_slice() else {
        panic!("one private open-chain helper expected")
    };
    assert_eq!(closure_helper.node, helper);
    assert_eq!(
        closure_helper.symbol,
        SemanticSymbol("offsetChain11".into())
    );
    assert_eq!(
        plan.user_facing_declaration_roots()
            .into_iter()
            .map(|declaration| declaration.node)
            .collect::<Vec<_>>(),
        [root]
    );

    // Closure presentation must not collapse source authority: both the
    // equation-free helper and operation remain complete managed declarations.
    assert_eq!(plan.declarations.len(), 2);
    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| declaration.builder_path.join("."))
            .collect::<Vec<_>>(),
        ["aggregate.openChain", "operation.profileOffset"]
    );
    let ManagedValue::Object(helper_arguments) = &plan.declarations[0].arguments else {
        panic!("Open Chain arguments must be an object")
    };
    let ManagedValue::Array(spans) = &helper_arguments["spans"] else {
        panic!("Open Chain spans must be an array")
    };
    assert_eq!(spans.len(), 1);
    assert!(matches!(spans[0], ManagedValue::Reference { .. }));
    let ManagedValue::Object(root_arguments) = &plan.declarations[1].arguments else {
        panic!("Profile Offset arguments must be an object")
    };
    let ManagedValue::Array(sources) = &root_arguments["sources"] else {
        panic!("Profile Offset sources must be an array")
    };
    assert_eq!(
        sources[0],
        ManagedValue::Reference {
            declaration: SemanticSymbol("offsetChain11".into()),
            path: SemanticOutputPath(vec![ManagedPathSegment::Field("chain".into())]),
        }
    );
    assert!(matches!(root_arguments["distance"], ManagedValue::Unit(_)));
    let encoded = serde_json::to_string(&plan).expect("named Profile Offset closure");
    for forbidden in [
        ".recipe",
        "operationOutputs",
        "\"inputs\"",
        "\"fields\"",
        "\"values\"",
        "\"results\"",
        "\"outputs\"",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}: {encoded}");
    }

    let accepted_materialization = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted Profile Offset materialization");
    assert!(accepted_materialization.validation.hard_residuals_validated);
    assert!(
        accepted_materialization
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn accepted_non_authoring_node_is_rejected_without_source_mutation() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let parameter = add_parameter(&mut candidate);

    let error = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            parameter,
            SemanticSymbol("parameter7".into()),
        )],
    )
    .expect_err("accepted non-authoring authority cannot become a source recipe");
    assert_eq!(
        error,
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: "parameter7".into(),
            recipe: "Parameter { parameter: Parameter }".into(),
        }
    );
    assert_eq!(
        project.managed.source,
        CompiledManagedSource::from_json(EMPTY_MANAGED)
            .expect("empty compiler envelope")
            .normalized_source,
    );
}

#[test]
fn generated_patch_dependency_uses_its_authenticated_public_result_path() {
    let demo = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::BracedFrame)
        .expect("braced-frame fixture");
    let project = demo.project();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated reconcile plan")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_b12a),
        DocumentId(PersistentId::from_u128(0x89_b12a)),
        1.0,
    )
    .expect("accepted braced frame");
    let address = GeneratedMemberAddress::new("brace", ["rising"], ["self"], ["field:span"]);
    let ExpandedSemanticTarget::Port { port } =
        &materialized.expansion.generated_provenance[&address].target
    else {
        panic!("rising brace must expose its generated span")
    };
    let rising = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&port.alias)
        .expect("rising generated segment")
        .id;
    let mut candidate = materialized
        .editor
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let horizontal = add_suppressed_horizontal_constraint(&mut candidate, rising);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &materialized.expansion,
        &materialized.editor,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            horizontal,
            SemanticSymbol("horizontal8".into()),
        )],
    )
    .expect("generated-dependency source insertion plan");
    let [declaration] = plan.declarations.as_slice() else {
        panic!("one constraint declaration expected")
    };
    assert!(declaration.suppressed);
    assert_eq!(declaration.builder_path, ["constraint", "horizontal"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("horizontal arguments must be an object")
    };
    assert_eq!(
        arguments["span"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("brace".into()),
            path: SemanticOutputPath(vec![
                ManagedPathSegment::Field("diagonals".into()),
                ManagedPathSegment::Field("rising".into()),
                ManagedPathSegment::Field("span".into()),
            ]),
        }
    );
}

#[test]
fn generic_point_output_keeps_a_dependent_segment_on_the_ergonomic_line_builder() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let point = add_sketch_point(&mut candidate);
    let segment = add_segment_from_point(&mut candidate, point);

    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(segment, SemanticSymbol("segment10".into())),
            EditorBootstrapDeclaration::new(point, SemanticSymbol("point9".into())),
        ],
    )
    .expect("point plus segment source insertion plan");
    assert!(plan.declaration_closures.is_empty());
    assert_eq!(plan.user_facing_declaration_roots().len(), 2);
    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| declaration.symbol.0.as_str())
            .collect::<Vec<_>>(),
        ["point9", "segment10"]
    );
    let segment = &plan.declarations[1];
    assert_eq!(segment.builder_path, ["geometry", "segment"]);
    let ManagedValue::Object(arguments) = &segment.arguments else {
        panic!("line arguments must be an object")
    };
    assert_eq!(
        arguments["start"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("point9".into()),
            path: SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
        }
    );
    assert_eq!(
        arguments["end"],
        ManagedValue::Array(vec![ManagedValue::Number(9.0), ManagedValue::Number(2.0)])
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one Fillet owner regression keeps accepted feature authentication, compact source, lexical provenance, and cold replay contiguous"
)]
fn canvas_computed_fillet_projects_direct_semantic_source_and_replays_exact_branch_state() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base line Intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("Fillet candidate editor fork");
    let second = add_segment_from_existing_end(&mut candidate, base);
    let fillet = add_computed_fillet_at_node_point(
        &mut candidate,
        base,
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        1.0,
    );
    let original_node = candidate
        .coordinator()
        .intent()
        .graph()
        .node(fillet)
        .expect("authored Fillet node")
        .clone();

    // Root-first input proves that source order and lexical parent visibility
    // come from semantic dependencies rather than the caller's list order.
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(fillet, SemanticSymbol("fillet2".into())),
            EditorBootstrapDeclaration::new(second, SemanticSymbol("segment1".into())),
        ],
    )
    .expect("computed Fillet source insertion plan");
    assert_eq!(
        plan.declarations
            .iter()
            .map(|declaration| (
                declaration.symbol.0.as_str(),
                declaration.builder_path.join(".")
            ))
            .collect::<Vec<_>>(),
        [
            ("segment1", "geometry.segment".to_owned()),
            ("fillet2", "computed.filletSet".to_owned()),
        ]
    );
    let declaration = &plan.declarations[1];
    assert!(!declaration.suppressed);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("direct Fillet arguments must be an object")
    };
    assert_eq!(
        arguments.keys().map(String::as_str).collect::<Vec<_>>(),
        ["corners", "label", "radius"]
    );
    assert_eq!(
        arguments["label"],
        ManagedValue::String("Canvas Fillet".into())
    );
    assert_eq!(
        arguments["radius"],
        ManagedValue::Unit(geosolve_sketch_code::UnitLiteral {
            unit: "mm".into(),
            value: 1.0,
        })
    );
    let ManagedValue::Array(corners) = &arguments["corners"] else {
        panic!("direct Fillet corners must be an array")
    };
    let [ManagedValue::Object(corner)] = corners.as_slice() else {
        panic!("the right-angle authoring gesture must retain one Fillet corner")
    };
    assert_eq!(
        corner.keys().map(String::as_str).collect::<Vec<_>>(),
        ["endpointOrder", "key", "parents", "sweep"]
    );
    assert!(matches!(corner["key"], ManagedValue::String(_)));
    assert!(matches!(corner["endpointOrder"], ManagedValue::String(_)));
    assert!(matches!(corner["sweep"], ManagedValue::String(_)));
    let ManagedValue::Array(parents) = &corner["parents"] else {
        panic!("direct Fillet parents must be an array")
    };
    assert_eq!(parents.len(), 2);
    let mut lexical_parents = Vec::new();
    for parent in parents {
        let ManagedValue::Object(parent) = parent else {
            panic!("direct Fillet parent must be an object")
        };
        assert_eq!(
            parent.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "neighborhood",
                "normalSide",
                "parameter",
                "periodicAnchor",
                "span",
                "trimEndpoint",
                "winding",
            ]
        );
        assert!(matches!(parent["parameter"], ManagedValue::Number(_)));
        assert!(matches!(parent["winding"], ManagedValue::Number(_)));
        assert!(matches!(parent["neighborhood"], ManagedValue::Object(_)));
        assert!(matches!(parent["normalSide"], ManagedValue::String(_)));
        assert!(matches!(parent["trimEndpoint"], ManagedValue::String(_)));
        assert!(matches!(
            parent["periodicAnchor"],
            ManagedValue::Null | ManagedValue::Object(_)
        ));
        let ManagedValue::Reference { declaration, path } = &parent["span"] else {
            panic!("Fillet span must remain one typed lexical reference")
        };
        assert_eq!(
            path,
            &SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())])
        );
        lexical_parents.push((declaration.0.clone(), path.clone()));
    }
    assert_eq!(
        lexical_parents,
        vec![
            (
                "base".to_owned(),
                SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
            ),
            (
                "segment1".to_owned(),
                SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
            ),
        ],
        "Fillet parent order is branch state and must remain source-visible",
    );
    let encoded = serde_json::to_string(&plan).expect("compact Fillet plan");
    assert!(!arguments.contains_key("suppressed"));
    for forbidden in [
        ".recipe",
        "editLens",
        "operationOutputs",
        "\"inputs\"",
        "\"fields\"",
        "\"values\"",
        "\"results\"",
        "\"outputs\"",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}: {encoded}");
    }
    assert!(
        encoded.len() < 2_500,
        "one direct Fillet insertion should stay compact, got {} bytes",
        encoded.len()
    );

    let original_accepted = candidate
        .coordinator()
        .accepted_materialization()
        .expect("original accepted Fillet authority");
    let original_feature_port = original_node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Feature,
            index: 0,
        })
        .expect("original feature port")
        .as_ref(original_node.id);
    let Some(IntentNativeBinding::ComputedFeature(original_feature_id)) =
        original_accepted.ownership.port(original_feature_port)
    else {
        panic!("original Fillet must own one accepted feature")
    };
    let original_feature = original_accepted
        .features
        .feature(original_feature_id)
        .expect("original accepted Fillet");
    let ComputedFeatureDefinition::FilletSet(original_fillet) = &original_feature.definition;
    assert_eq!(original_feature.label.as_str(), "Canvas Fillet");
    assert_eq!(original_fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(original_fillet.corners.len(), 1);
    assert!(original_accepted.validation.hard_residuals_validated);
    assert!(original_accepted.validation.all_active_features_current);
    assert!(
        original_accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    assert!(
        original_accepted
            .session
            .design_document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused Fillet replay keeps every non-default periodic branch field auditable together"
)]
fn computed_fillet_reverse_source_preserves_nondefault_periodic_branch_state() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("periodic Fillet candidate fork");
    let key = |value| geosolve_sketch_intent::IntentKey::new(value).expect("fixture key");
    let field = |value| IntentFieldKey(key(value));
    let node_selector = |role| IntentPortSelector::Node { role, index: 0 };
    let circle_alias = key("canvas.periodic.circle.gesture");
    let line_alias = key("canvas.periodic.line.gesture");
    let fillet_alias = key("canvas.periodic.fillet.gesture");

    let circle = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        key("canvas.periodic.circle"),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Center),
        LeafField::X,
        length(0.0),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Center),
        LeafField::Y,
        length(0.0),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Target),
        LeafField::Value,
        length(2.0),
    );
    let line = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("canvas.periodic.line"),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Start),
        LeafField::X,
        length(0.0),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Start),
        LeafField::Y,
        length(1.0),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::End),
        LeafField::X,
        length(6.0),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::End),
        LeafField::Y,
        length(1.0),
    )
    .with_field(field("branch_direction"), IntentLiteral::Point([1.0, 0.0]));
    let mut fillet = IntentNodeDraft::new(
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        },
        key("canvas.periodic.fillet"),
    )
    .with_display_name(key("Periodic branch Fillet"))
    .with_dynamic_children(1)
    .with_input(
        InputSlot::new(InputRole::Span, 0),
        PatchPortRef::Alias {
            node: circle_alias.clone(),
            selector: node_selector(IntentPortRole::Span),
        },
    )
    .with_input(
        InputSlot::new(InputRole::Span, 1),
        PatchPortRef::Alias {
            node: line_alias.clone(),
            selector: node_selector(IntentPortRole::Span),
        },
    )
    .with_field(
        field("name"),
        IntentLiteral::Text(key("Periodic branch Fillet")),
    )
    .with_field(field("radius"), length(1.0))
    .with_field(field("corner_0000_first_parameter"), dimensionless(0.0))
    .with_field(
        field("corner_0000_first_winding"),
        IntentLiteral::Integer(0),
    )
    .with_field(
        field("corner_0000_first_neighborhood"),
        enum_literal("local"),
    )
    .with_field(field("corner_0000_first_local_lower"), dimensionless(-0.4))
    .with_field(field("corner_0000_first_local_upper"), dimensionless(0.4))
    .with_field(
        field("corner_0000_first_normal_side"),
        enum_literal("right"),
    )
    .with_field(
        field("corner_0000_first_trim_endpoint"),
        enum_literal("end"),
    )
    .with_field(
        field("corner_0000_first_periodic_anchor"),
        IntentLiteral::Boolean(true),
    )
    .with_field(
        field("corner_0000_first_anchor_parameter"),
        dimensionless(std::f64::consts::PI),
    )
    .with_field(
        field("corner_0000_first_anchor_winding"),
        IntentLiteral::Integer(-1),
    )
    .with_field(field("corner_0000_second_parameter"), dimensionless(0.5))
    .with_field(
        field("corner_0000_second_winding"),
        IntentLiteral::Integer(0),
    )
    .with_field(
        field("corner_0000_second_neighborhood"),
        enum_literal("interior"),
    )
    .with_field(
        field("corner_0000_second_normal_side"),
        enum_literal("right"),
    )
    .with_field(
        field("corner_0000_second_trim_endpoint"),
        enum_literal("start"),
    )
    .with_field(
        field("corner_0000_second_periodic_anchor"),
        IntentLiteral::Boolean(false),
    )
    .with_field(
        field("corner_0000_endpoint_order"),
        enum_literal("second_then_first"),
    )
    .with_field(
        field("corner_0000_sweep"),
        enum_literal("counter_clockwise"),
    );
    fillet.suppressed = false;

    let outcome = candidate
        .apply_patch(IntentPatch::new(
            candidate.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: circle_alias.clone(),
                    draft: Box::new(circle),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: line_alias.clone(),
                    draft: Box::new(line),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: fillet_alias.clone(),
                    draft: Box::new(fillet),
                    cell: None,
                },
            ],
        ))
        .expect("suppressed periodic Fillet candidate");
    let circle = outcome.aliases.node(&circle_alias).expect("circle node");
    let line = outcome.aliases.node(&line_alias).expect("line node");
    let fillet = outcome.aliases.node(&fillet_alias).expect("Fillet node");
    let insertion = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(circle, SemanticSymbol("periodicCircle".into())),
            EditorBootstrapDeclaration::new(line, SemanticSymbol("supportLine".into())),
            EditorBootstrapDeclaration::new(fillet, SemanticSymbol("branchFillet".into())),
        ],
    )
    .expect("periodic Fillet reverse projection");
    let declaration = insertion
        .declarations
        .iter()
        .find(|declaration| declaration.node == fillet)
        .expect("reverse-projected Fillet declaration");
    assert_eq!(declaration.builder_path, ["computed", "filletSet"]);
    assert!(!declaration.suppressed);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("Fillet arguments must be an object")
    };
    let ManagedValue::Array(corners) = &arguments["corners"] else {
        panic!("Fillet corners must be an array")
    };
    let [ManagedValue::Object(corner)] = corners.as_slice() else {
        panic!("one Fillet corner expected")
    };
    assert_eq!(
        corner["endpointOrder"],
        ManagedValue::String("secondThenFirst".into())
    );
    assert_eq!(
        corner["sweep"],
        ManagedValue::String("counterClockwise".into())
    );
    let ManagedValue::Array(parents) = &corner["parents"] else {
        panic!("Fillet parents must be an array")
    };
    let ManagedValue::Object(first) = &parents[0] else {
        panic!("first Fillet parent must be an object")
    };
    assert_eq!(first["winding"], ManagedValue::Number(0.0));
    assert_eq!(first["normalSide"], ManagedValue::String("right".into()));
    let ManagedValue::Object(neighborhood) = &first["neighborhood"] else {
        panic!("local neighborhood must be an object")
    };
    assert_eq!(neighborhood["kind"], ManagedValue::String("local".into()));
    assert_eq!(neighborhood["lower"], ManagedValue::Number(-0.4));
    assert_eq!(neighborhood["upper"], ManagedValue::Number(0.4));
    let ManagedValue::Object(anchor) = &first["periodicAnchor"] else {
        panic!("periodic parent must retain an anchor")
    };
    assert_eq!(
        anchor["parameter"],
        ManagedValue::Number(std::f64::consts::PI)
    );
    assert_eq!(anchor["winding"], ManagedValue::Number(-1.0));

    let source = insertion
        .declarations
        .iter()
        .map(managed_source_declaration)
        .collect::<String>();
    assert!(source.contains("$.computed.filletSet"));
    assert!(source.contains("periodicAnchor"));
    for forbidden in [
        ".recipe",
        "editLens",
        "operationOutputs",
        "inputs:",
        "fields:",
        "values:",
        "results:",
        "outputs:",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}: {source}");
    }
    let current = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted periodic Fillet authority");
    let [feature] = current.features.features() else {
        panic!("one accepted periodic Fillet expected")
    };
    assert_eq!(feature.label, "Periodic branch Fillet");
    assert!(!feature.suppressed);
    assert!(current.validation.hard_residuals_validated);
    assert!(current.validation.all_active_features_current);
    assert!(
        current
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn same_gesture_polyline_fillet_uses_keyed_segment_source_paths() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("Polyline Fillet candidate fork");
    let polyline = add_polyline(&mut candidate, None);
    let fillet = add_computed_fillet_at_node_point(
        &mut candidate,
        polyline,
        IntentPortSelector::InitialChild {
            ordinal: 1,
            role: IntentPortRole::Corner,
            index: 0,
        },
        1.0,
    );
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(fillet, SemanticSymbol("fillet2".into())),
            EditorBootstrapDeclaration::new(polyline, SemanticSymbol("geometry1".into())),
        ],
    )
    .expect("same-gesture Polyline Fillet source projection");
    assert_eq!(plan.declarations[0].builder_path, ["geometry", "polyline"]);
    assert_eq!(plan.declarations[1].builder_path, ["computed", "filletSet"]);
    let ManagedValue::Object(arguments) = &plan.declarations[1].arguments else {
        panic!("direct Fillet arguments must be an object")
    };
    let ManagedValue::Array(corners) = &arguments["corners"] else {
        panic!("direct Fillet corners must be an array")
    };
    let [ManagedValue::Object(corner)] = corners.as_slice() else {
        panic!("one Polyline Fillet corner expected")
    };
    let ManagedValue::Array(parents) = &corner["parents"] else {
        panic!("direct Fillet parents must be an array")
    };
    let paths = parents
        .iter()
        .map(|parent| {
            let ManagedValue::Object(parent) = parent else {
                panic!("direct Fillet parent must be an object")
            };
            let ManagedValue::Reference { declaration, path } = &parent["span"] else {
                panic!("Polyline Fillet span must be a lexical reference")
            };
            assert_eq!(declaration, &SemanticSymbol("geometry1".into()));
            path.clone()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        paths,
        BTreeSet::from([
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("segments".into()),
                ManagedPathSegment::Field("byKey".into()),
                ManagedPathSegment::Field("v0".into()),
            ]),
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("segments".into()),
                ManagedPathSegment::Field("byKey".into()),
                ManagedPathSegment::Field("v1".into()),
            ]),
        ]),
        "the direct Polyline surface must never regress to unstable ordinal span paths",
    );
    let accepted = candidate
        .coordinator()
        .accepted_materialization()
        .expect("accepted Polyline Fillet authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}

#[test]
fn suppressed_canvas_fillet_keeps_reversible_suppression_outside_its_base_declaration() {
    let (project, expansion, accepted) = accepted_line_code_project();
    let base_alias = expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| {
            (declaration == &SemanticSymbol("base".into())).then_some(alias)
        })
        .expect("base declaration provenance");
    let base = accepted
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(base_alias)
        .expect("base line Intent node")
        .id;
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("suppressed Fillet candidate fork");
    let second = add_segment_from_existing_end(&mut candidate, base);
    let fillet = add_computed_fillet_at_node_point(
        &mut candidate,
        base,
        IntentPortSelector::Node {
            role: IntentPortRole::End,
            index: 0,
        },
        1.0,
    );
    candidate
        .apply_patch(IntentPatch::new(
            candidate.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::SetSuppressed {
                node: fillet,
                suppressed: true,
            }],
        ))
        .expect("suppress accepted computed Fillet");
    let plan = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[
            EditorBootstrapDeclaration::new(second, SemanticSymbol("segment1".into())),
            EditorBootstrapDeclaration::new(fillet, SemanticSymbol("fillet2".into())),
        ],
    )
    .expect("suppressed Fillet source insertion plan");
    let declaration = &plan.declarations[1];
    assert!(declaration.suppressed);
    assert_eq!(declaration.builder_path, ["computed", "filletSet"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("direct Fillet arguments must be an object")
    };
    assert!(
        !arguments.contains_key("suppressed"),
        "managed must emit actual suppression as `$.suppress(...)` so later unsuppression remains reversible",
    );
    let accepted = candidate
        .coordinator()
        .accepted_materialization()
        .expect("suppressed Fillet accepted authority");
    let [feature] = accepted.features.features() else {
        panic!("one suppressed computed Fillet expected")
    };
    assert!(feature.suppressed);
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
}
