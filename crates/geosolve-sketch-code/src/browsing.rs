// SPDX-License-Identifier: GPL-3.0-or-later
//! Accepted source, generated ownership and declaration actions for host browsing.
use crate::{
    CodeOwnerAddress, ExpandedCodeProject, ExpandedSemanticTarget, GeneratedMemberAddress,
    ManagedControl, ManagedControlAccess, ManagedControlEdit, ManagedControlEditBatch,
    ManagedControlId, ManagedControlManifest, ManagedControlSchema, ManagedPathSegment,
    ManagedValue, SemanticSymbol, UnitLiteral, generated_panel_row_id, managed_control_authority,
    managed_panel_row_id,
};
use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_intent::NodeId;
use std::collections::{BTreeMap, BTreeSet};

/// Read-only source/expansion projection consumed by the DOM-free bridge.
/// Stable managed symbols and generated addresses are presentation identity;
/// opaque Intent aliases remain action tokens owned by this adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedDeclarationPanelProjection {
    pub source_digest: String,
    pub dirty: bool,
    pub blocked_reason: Option<String>,
    pub declarations: Vec<ManagedDeclarationPanelRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedDeclarationPanelRow {
    pub id: String,
    pub symbol: SemanticSymbol,
    pub label: String,
    pub kind: String,
    pub group: Option<String>,
    pub source_start: usize,
    pub source_end: usize,
    pub selected: bool,
    pub selection_node: Option<NodeId>,
    pub suppressed: Option<bool>,
    pub suppression_control_id: Option<String>,
    pub closure_role: ManagedDeclarationClosureRole,
    pub closure_helpers: Vec<Self>,
    pub generated: Vec<ManagedGeneratedPanelRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedDeclarationClosureRole {
    Independent,
    Root,
    Helper { root: SemanticSymbol },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedGeneratedPanelRow {
    pub id: String,
    pub address: GeneratedMemberAddress,
    pub label: String,
    pub kind: String,
    pub source_start: usize,
    pub source_end: usize,
    pub selected: bool,
    pub selection_node: Option<NodeId>,
    pub suppressed: bool,
    pub suppression_token: Option<String>,
}

/// Projects exact accepted declaration ownership and source actions without materialization.
/// Draft and retained-failure states disable structured actions while preserving accepted rows.
///
/// # Panics
/// Panics if the caller supplies an accepted compilation with invalid suppression metadata.
#[allow(
    clippy::too_many_lines,
    reason = "one accepted projection preserves lexical order, generated ownership and closure lifecycle"
)]
pub fn managed_declaration_panel_projection(
    snapshot: &crate::CodeSessionSnapshot,
    editor: &ProjectionalEditorSession,
    dirty: bool,
    controls: Option<&ManagedControlManifest>,
    metadata: Option<&crate::ManagedAuthoredMetadata>,
) -> ManagedDeclarationPanelProjection {
    let managed = snapshot
        .accepted_code_project
        .as_ref()
        .map_or(&snapshot.managed, |project| &project.managed);
    let expansion = snapshot.accepted_expansion.as_ref();
    let generated = if snapshot.failure.is_some() {
        snapshot
            .accepted_generated
            .as_ref()
            .unwrap_or(&snapshot.generated)
    } else {
        &snapshot.generated
    };
    let blocked_reason = if dirty {
        Some(
            "Apply or Revert the managed-source draft before structured declaration actions".into(),
        )
    } else if snapshot.failure.is_some() {
        Some(
            "Resolve or Undo the retained code failure before structured declaration actions"
                .into(),
        )
    } else {
        None
    };
    let manifest = blocked_reason.is_none().then_some(controls).flatten();
    let selected = editor.selected_declaration();
    let graph = editor.coordinator().intent().graph();
    let mut groups = BTreeMap::<SemanticSymbol, String>::new();
    for organization in &managed.program.organizations {
        for declaration in &organization.declarations {
            groups
                .entry(declaration.clone())
                .or_insert_with(|| organization.name.clone());
        }
    }

    let declaration_symbols = managed
        .program
        .declarations
        .iter()
        .map(|declaration| declaration.symbol.clone())
        .collect::<BTreeSet<_>>();
    let mut generated_by_declaration = BTreeMap::<SemanticSymbol, Vec<_>>::new();
    if let Some(expansion) = expansion {
        for member in generated.ordered_members() {
            let Some(provenance) = expansion.generated_provenance.get(&member.address) else {
                continue;
            };
            let invocation = SemanticSymbol(member.address.invocation.clone());
            let source_owner = if declaration_symbols.contains(&invocation) {
                invocation
            } else {
                provenance.declaration.clone()
            };
            generated_by_declaration
                .entry(source_owner)
                .or_default()
                .push((member, provenance));
        }
    }

    let mut declarations = managed
        .program
        .declarations
        .iter()
        .map(|declaration| {
            let representative = expansion.and_then(|expansion| {
                representative_declaration_node(expansion, graph, &declaration.symbol)
            });
            let explicit_suppressed = managed.compiled.as_deref().map_or_else(
                || match &declaration.arguments {
                    ManagedValue::Object(arguments) => {
                        arguments.get("suppressed").and_then(|value| match value {
                            ManagedValue::Bool(value) => Some(*value),
                            _ => None,
                        })
                    }
                    _ => None,
                },
                |compiled| {
                    Some(
                        compiled
                            .declaration_is_suppressed(&declaration.symbol)
                            .expect("accepted managed suppression projection is valid"),
                    )
                },
            );
            let suppression_control_id = manifest.as_ref().and_then(|manifest| {
                manifest.controls.iter().find_map(|control| {
                    let exact_field = matches!(
                        control.source.path.0.as_slice(),
                        [ManagedPathSegment::Field(field)] if field == "suppressed"
                    );
                    (control.source.declaration == declaration.symbol
                        && exact_field
                        && matches!(control.value, ManagedValue::Bool(_))
                        && matches!(control.access, ManagedControlAccess::Editable { .. }))
                    .then(|| control.id.0.clone())
                })
            });
            let mut generated = generated_by_declaration
                .remove(&declaration.symbol)
                .unwrap_or_default()
                .into_iter()
                .map(|(member, _)| {
                    let generated_node = expansion.and_then(|expansion| {
                        generated_member_node(expansion, graph, &member.address)
                    });
                    let host_children = expansion.map_or_else(Vec::new, |expansion| {
                        expansion
                            .generated_children
                            .iter()
                            .filter(|child| {
                                matches!(
                                    &child.address.owner.address,
                                    CodeOwnerAddress::GeneratedMember { address }
                                        if address == &member.address
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    let suppressed = !host_children.is_empty()
                        && host_children.iter().all(|child| child.suppressed);
                    let suppression_token = match host_children.as_slice() {
                        [child] => serde_json::to_string(&child.address).ok(),
                        _ => None,
                    };
                    ManagedGeneratedPanelRow {
                        id: generated_panel_row_id(&member.address),
                        address: member.address.clone(),
                        label: generated_member_label(&member.address),
                        kind: "Generated output".into(),
                        source_start: declaration.statement_span.start,
                        source_end: declaration.statement_span.end,
                        selected: generated_node == selected,
                        selection_node: generated_node,
                        suppressed,
                        suppression_token,
                    }
                })
                .collect::<Vec<_>>();
            distinguish_generated_output_labels(&mut generated);
            ManagedDeclarationPanelRow {
                id: managed_panel_row_id(&declaration.symbol),
                symbol: declaration.symbol.clone(),
                label: metadata
                    .and_then(|metadata| {
                        metadata
                            .declarations
                            .get(&declaration.symbol)
                            .and_then(|presentation| presentation.label.clone())
                    })
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| declaration.symbol.0.clone()),
                kind: declaration.patch.as_ref().map_or_else(
                    || declaration.builder_path.join("."),
                    |_| "Patch invocation".into(),
                ),
                group: groups.get(&declaration.symbol).cloned(),
                source_start: declaration.statement_span.start,
                source_end: declaration.statement_span.end,
                selected: representative == selected,
                selection_node: representative,
                suppressed: explicit_suppressed,
                suppression_control_id,
                closure_role: ManagedDeclarationClosureRole::Independent,
                closure_helpers: Vec::new(),
                generated,
            }
        })
        .collect::<Vec<_>>();
    if let Some(compiled) = managed.compiled.as_deref() {
        let closures = compiled
            .source_declaration_closures()
            .expect("accepted managed declaration closure projection is valid");
        let mut rows = declarations
            .into_iter()
            .map(|row| (row.symbol.clone(), row))
            .collect::<BTreeMap<_, _>>();
        let helper_symbols = closures
            .iter()
            .flat_map(|closure| closure.helpers.iter().cloned())
            .collect::<BTreeSet<_>>();
        for closure in closures {
            let helpers = closure
                .helpers
                .into_iter()
                .filter_map(|helper| {
                    rows.get_mut(&helper).map(|row| {
                        row.closure_role = ManagedDeclarationClosureRole::Helper {
                            root: closure.root.clone(),
                        };
                        row.clone()
                    })
                })
                .collect::<Vec<_>>();
            if let Some(root) = rows.get_mut(&closure.root) {
                root.closure_role = ManagedDeclarationClosureRole::Root;
                root.closure_helpers = helpers;
            }
        }
        declarations = managed
            .program
            .declarations
            .iter()
            .filter(|declaration| !helper_symbols.contains(&declaration.symbol))
            .filter_map(|declaration| rows.remove(&declaration.symbol))
            .collect();
    }
    ManagedDeclarationPanelProjection {
        source_digest: managed.source_digest.clone(),
        dirty,
        blocked_reason,
        declarations,
    }
}
// A generated member can expose separate curve/centre/radius ports. Keep
// familiar short labels when unique and name the output only for collisions.
fn distinguish_generated_output_labels(rows: &mut [ManagedGeneratedPanelRow]) {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows.iter() {
        *counts.entry(row.label.clone()).or_default() += 1;
    }
    for row in rows {
        if counts[&row.label] > 1 {
            let output = row
                .address
                .output
                .iter()
                .map(|part| part.strip_prefix("field:").unwrap_or(part))
                .collect::<Vec<_>>()
                .join(" / ");
            if !output.is_empty() {
                row.label = format!("{} / {output}", row.label);
            }
        }
    }
}

fn generated_member_label(address: &GeneratedMemberAddress) -> String {
    for path in [&address.member_key, &address.output, &address.template] {
        if !path.is_empty() {
            return path.join(" / ");
        }
    }
    address.invocation.clone()
}

fn generated_target_alias(
    target: &ExpandedSemanticTarget,
) -> Option<&geosolve_sketch_intent::IntentKey> {
    match target {
        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias),
        ExpandedSemanticTarget::Port { port } => Some(&port.alias),
        ExpandedSemanticTarget::FeatureCorner { corner } => Some(&corner.point.alias),
        ExpandedSemanticTarget::Collection { members } => members
            .values()
            .find_map(|member| generated_target_alias(member)),
        ExpandedSemanticTarget::HostOutput { .. } => None,
    }
}

fn generated_member_node(
    expansion: &ExpandedCodeProject,
    graph: &geosolve_sketch_intent::IntentGraph,
    address: &GeneratedMemberAddress,
) -> Option<NodeId> {
    // A host-generated semantic child (notably a computed Fillet) is the
    // selectable output owned by this row. Its generated-provenance target
    // may instead be the parent corner operand, so prefer the exact child
    // address before considering ordinary generated declaration aliases.
    expansion
        .generated_children
        .iter()
        .find_map(|child| {
            let matches = matches!(
                &child.address.owner.address,
                CodeOwnerAddress::GeneratedMember { address: candidate }
                    if candidate == address
            );
            matches
                .then(|| graph.node_by_symbol(&child.alias).map(|node| node.id))
                .flatten()
        })
        .or_else(|| {
            expansion
                .generated_provenance
                .get(address)
                .and_then(|provenance| generated_target_alias(&provenance.target))
                .and_then(|alias| graph.node_by_symbol(alias))
                .map(|node| node.id)
        })
}

fn representative_declaration_node(
    expansion: &ExpandedCodeProject,
    graph: &geosolve_sketch_intent::IntentGraph,
    declaration: &SemanticSymbol,
) -> Option<NodeId> {
    let generated_aliases = expansion
        .generated_provenance
        .values()
        .filter_map(|provenance| generated_target_alias(&provenance.target))
        .chain(
            expansion
                .generated_children
                .iter()
                .map(|child| &child.alias),
        )
        .collect::<BTreeSet<_>>();
    let candidates = expansion
        .declaration_provenance
        .iter()
        .filter(|(_, owner)| *owner == declaration)
        .filter_map(|(alias, _)| graph.node_by_symbol(alias).map(|node| (alias, node.id)))
        .collect::<Vec<_>>();
    candidates
        .iter()
        .find(|(alias, _)| !generated_aliases.contains(alias))
        .or_else(|| candidates.first())
        .map(|(_, node)| *node)
}

/// Browser-decoded value for one Code-panel managed control. The stable
/// control ID is the only source coordinate carried by DOM; this value is
/// reinterpreted against a freshly derived manifest before mutation.
#[derive(Debug)]
pub enum ManagedControlSubmission {
    Number(f64),
    Boolean(bool),
    String(String),
}

/// Describes one exact source control change using freshly authenticated control authority.
/// No compiler job, geometry edit or history publication occurs here.
///
/// # Errors
/// Rejects stale/read-only controls or values incompatible with the accepted schema.
pub fn managed_control_source_mutation(
    project: &crate::CodeProject,
    expansion: &ExpandedCodeProject,
    id: &str,
    submission: ManagedControlSubmission,
) -> Result<Option<crate::ManagedSketchMutation>, String> {
    let authority =
        managed_control_authority(project, expansion).map_err(|error| error.to_string())?;
    let control = authority
        .manifest()
        .control(&ManagedControlId(id.to_owned()))
        .filter(|control| control.token().is_some())
        .ok_or_else(|| "managed control is unavailable or read-only".to_owned())?;
    let replacement = managed_value_from_submission(control, submission)?;
    if managed_values_exactly_equal(&control.value, &replacement) {
        return Ok(None);
    }
    let batch = ManagedControlEditBatch::new([ManagedControlEdit {
        token: control
            .token()
            .ok_or("editable managed control has no exact token")?
            .clone(),
        value: replacement,
    }]);
    authority
        .prepare_mutation(&batch)
        .map(Some)
        .map_err(|error| error.to_string())
}
fn managed_value_from_submission(
    control: &ManagedControl,
    submission: ManagedControlSubmission,
) -> Result<ManagedValue, String> {
    let schema = control
        .schema
        .as_ref()
        .ok_or_else(|| "the selected managed control is read-only".to_owned())?;
    match (schema, &control.value, submission) {
        (
            ManagedControlSchema::Number { .. },
            ManagedValue::Number(_),
            ManagedControlSubmission::Number(value),
        ) if value.is_finite() => Ok(ManagedValue::Number(value)),
        (
            ManagedControlSchema::Unit { unit, .. },
            ManagedValue::Unit(current),
            ManagedControlSubmission::Number(value),
        ) if current.unit == *unit && value.is_finite() => Ok(ManagedValue::Unit(UnitLiteral {
            unit: current.unit.clone(),
            value,
        })),
        (
            ManagedControlSchema::Boolean,
            ManagedValue::Bool(_),
            ManagedControlSubmission::Boolean(value),
        ) => Ok(ManagedValue::Bool(value)),
        (
            ManagedControlSchema::Choice { .. } | ManagedControlSchema::Text,
            ManagedValue::String(_),
            ManagedControlSubmission::String(value),
        ) => Ok(ManagedValue::String(value)),
        (
            ManagedControlSchema::Number { .. } | ManagedControlSchema::Unit { .. },
            _,
            ManagedControlSubmission::Number(value),
        ) if !value.is_finite() => Err("managed-control number must be finite".into()),
        _ => Err(
            "managed-control submission does not match its fresh source representation and schema"
                .into(),
        ),
    }
}

fn managed_values_exactly_equal(left: &ManagedValue, right: &ManagedValue) -> bool {
    match (left, right) {
        (ManagedValue::Null, ManagedValue::Null) => true,
        (ManagedValue::Bool(left), ManagedValue::Bool(right)) => left == right,
        (ManagedValue::Number(left), ManagedValue::Number(right)) => {
            left.to_bits() == right.to_bits()
        }
        (ManagedValue::String(left), ManagedValue::String(right)) => left == right,
        (ManagedValue::Unit(left), ManagedValue::Unit(right)) => {
            left.unit == right.unit && left.value.to_bits() == right.value.to_bits()
        }
        (ManagedValue::Array(left), ManagedValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| managed_values_exactly_equal(left, right))
        }
        (ManagedValue::Object(left), ManagedValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| managed_values_exactly_equal(left, right))
                })
        }
        (
            ManagedValue::Reference {
                declaration: left_declaration,
                path: left_path,
            },
            ManagedValue::Reference {
                declaration: right_declaration,
                path: right_path,
            },
        ) => left_declaration == right_declaration && left_path == right_path,
        _ => false,
    }
}

/// Projects source navigation against exact retained native identity, preserving draft failures.
pub fn managed_browsing_navigation_index(
    snapshot: &crate::CodeSessionSnapshot,
    editor: &ProjectionalEditorSession,
    materialized_identity: Option<geosolve_sketch_intent::IntentSessionIdentity>,
    dirty: bool,
    draft_disagrees: bool,
) -> crate::ManagedNavigationIndex {
    let managed = snapshot
        .accepted_code_project
        .as_ref()
        .map_or(&snapshot.managed, |project| &project.managed);
    let blocked_reason = if dirty {
        Some("Apply or Revert the managed-source draft before source navigation".into())
    } else if snapshot.failure.is_some() || draft_disagrees {
        Some("Resolve or Undo the retained code failure before source navigation".into())
    } else {
        None
    };
    let mut index = crate::ManagedNavigationIndex {
        source: managed.source.clone(),
        source_digest: managed.source_digest.clone(),
        blocked_reason,
        entries: Vec::new(),
    };
    let Some(expansion) = snapshot.accepted_expansion.as_ref() else {
        index.blocked_reason = Some("No accepted managed source is available".into());
        return index;
    };
    if materialized_identity
        .is_some_and(|identity| identity != editor.coordinator().intent().identity())
    {
        index.blocked_reason = Some("The canvas does not match accepted code authority".into());
        return index;
    }
    let mut projected = crate::managed_navigation_index(
        managed,
        expansion,
        snapshot
            .accepted_generated
            .as_ref()
            .unwrap_or(&snapshot.generated),
        editor,
    );
    projected.blocked_reason = index.blocked_reason;
    projected
}
