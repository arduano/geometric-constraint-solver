// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic read-only workbench projections over canonical design intent.
//!
//! These DTOs contain no geometry equations and never become materialization
//! authority. They expose the same stable declarations to Outline, Inspector,
//! structured-source, History, native hosts, and DOM-free RPC consumers.

use std::collections::BTreeMap;
use std::ops::Range;

use geosolve_sketch_intent::{
    CellId, InputSlot, IntentAttemptDisposition, IntentDeclarationDescriptor,
    IntentDefinitionFieldDescriptor, IntentEditClassification, IntentFieldKey,
    IntentHistoryProjection, IntentKey, IntentLiteral, IntentNode, IntentOutputDescriptor,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRef, IntentSession,
    IntentSessionIdentity, LeafRef, NodeId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::IntentGraphNodeKind;

/// One declaration row in the non-semantic Outline projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOutlineDeclaration {
    pub node: NodeId,
    pub symbol: IntentKey,
    pub name: IntentKey,
    pub kind: IntentGraphNodeKind,
    pub suppressed: bool,
    pub retained_failure: bool,
    pub dependencies: Vec<NodeId>,
}

/// One organization cell and its presentation-ordered declarations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOutlineCell {
    pub cell: CellId,
    pub name: IntentKey,
    pub declarations: Vec<IntentOutlineDeclaration>,
}

/// One schema-derived Inspector definition value or writable instance leaf.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentInspectorField {
    Definition {
        definition: IntentFieldKey,
        value: Option<IntentLiteral>,
    },
    Instance {
        leaf: LeafRef,
        value: Option<IntentLiteral>,
    },
}

/// Complete typed Inspector projection for one stable declaration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentInspectorProjection {
    pub node: NodeId,
    pub symbol: IntentKey,
    pub name: IntentKey,
    pub kind: IntentGraphNodeKind,
    pub suppressed: bool,
    pub retained_failure: bool,
    pub inputs: Vec<(geosolve_sketch_intent::InputSlot, IntentPortRef)>,
    pub descriptor: IntentDeclarationDescriptor,
    pub fields: Vec<IntentInspectorField>,
}

/// Stable editable coordinate exposed by one schema-generated Inspector.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentInspectorEditTarget {
    Suppressed,
    Definition { field: IntentFieldKey },
    Instance { leaf: LeafRef },
}

/// Typed value submitted by an Inspector control.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentInspectorEditValue {
    Suppressed { suppressed: bool },
    Literal { literal: IntentLiteral },
}

impl IntentInspectorProjection {
    /// Builds one schema-generated Inspector directly from current intent.
    ///
    /// This declaration-local query avoids constructing Outline, Structured
    /// Source, or History when a host requests only Inspector data.
    ///
    /// # Panics
    ///
    /// Panics only if a session that already passed structural validation
    /// contains more children than the public bounded schema permits.
    #[must_use]
    pub fn from_session(session: &IntentSession, node_id: NodeId) -> Option<Self> {
        let node = session.graph().node(node_id)?;
        let name = session.organization().node_names().get(&node_id)?.clone();
        let retained_failure = session.latest_attempt().is_some_and(|attempt| {
            attempt.disposition == IntentAttemptDisposition::RetainedFailed
                && attempt.failed_nodes.contains(&node_id)
        });
        let descriptor = node.descriptor();
        let mut fields = descriptor
            .fields
            .iter()
            .map(|field| IntentInspectorField::Definition {
                value: node.fields.get(&field.schema.field).cloned(),
                definition: field.schema.field.clone(),
            })
            .collect::<Vec<_>>();
        for port in node.ports.values() {
            for field in &port.writable {
                let leaf = LeafRef {
                    node: node_id,
                    port: port.id,
                    field: *field,
                };
                fields.push(IntentInspectorField::Instance {
                    leaf,
                    value: session.instance().values().get(&leaf).cloned(),
                });
            }
        }
        Some(Self {
            node: node_id,
            symbol: node.symbol.clone(),
            name,
            kind: IntentGraphNodeKind::from_kind(&node.kind),
            suppressed: node.suppressed,
            retained_failure,
            inputs: node
                .inputs
                .iter()
                .map(|(slot, source)| (*slot, *source))
                .collect(),
            descriptor,
            fields,
        })
    }

    /// Returns the central schema/edit metadata for one projected definition field.
    #[must_use]
    pub fn definition_descriptor(
        &self,
        field: &IntentFieldKey,
    ) -> Option<&IntentDefinitionFieldDescriptor> {
        self.descriptor
            .fields
            .iter()
            .find(|candidate| &candidate.schema.field == field)
    }

    /// Returns the central output metadata which owns one projected writable leaf.
    #[must_use]
    pub fn output_descriptor(&self, leaf: LeafRef) -> Option<&IntentOutputDescriptor> {
        self.descriptor.outputs.iter().find(|output| {
            output.port.node == leaf.node
                && output.port.port == leaf.port
                && output.writable.contains(&leaf.field)
        })
    }

    /// Converts one current schema-generated Inspector coordinate into the
    /// ordinary unordered exact-CAS patch vocabulary.
    ///
    /// The caller supplies an already parsed typed literal. This method
    /// authenticates the complete Inspector projection and target against the
    /// current session before returning a patch; browser field names or stale
    /// markup therefore cannot address an arbitrary graph leaf.
    ///
    /// # Errors
    ///
    /// Returns a stale-projection, target-kind, unknown-field, or literal-kind
    /// error without mutating the session.
    pub fn patch_for_edit(
        &self,
        session: &IntentSession,
        target: &IntentInspectorEditTarget,
        value: IntentInspectorEditValue,
    ) -> Result<IntentPatch, IntentInspectorEditError> {
        let current = IntentWorkbenchProjection::from_session(session)
            .inspector(session, self.node)
            .ok_or(IntentInspectorEditError::StaleProjection)?;
        if current != *self {
            return Err(IntentInspectorEditError::StaleProjection);
        }
        let operation = match (target, value) {
            (
                IntentInspectorEditTarget::Suppressed,
                IntentInspectorEditValue::Suppressed { suppressed },
            ) if self.descriptor.suppression_edit == IntentEditClassification::Definition => {
                IntentPatchOperation::SetSuppressed {
                    node: self.node,
                    suppressed,
                }
            }
            (
                IntentInspectorEditTarget::Definition { field },
                IntentInspectorEditValue::Literal { literal },
            ) => {
                let descriptor = self
                    .definition_descriptor(field)
                    .filter(|candidate| {
                        candidate.edit == IntentEditClassification::Definition
                            && self.fields.iter().any(|projected| {
                                matches!(
                                    projected,
                                    IntentInspectorField::Definition {
                                        definition: projected,
                                        ..
                                    }
                                        if projected == field
                                )
                            })
                    })
                    .ok_or(IntentInspectorEditError::UnknownTarget)?;
                if !inspector_literal_matches(descriptor.schema.literal, &literal) {
                    return Err(IntentInspectorEditError::InvalidLiteral);
                }
                IntentPatchOperation::SetDefinitionField {
                    node: self.node,
                    field: field.clone(),
                    value: literal,
                }
            }
            (
                IntentInspectorEditTarget::Instance { leaf },
                IntentInspectorEditValue::Literal { literal },
            ) => {
                let current = self.fields.iter().find_map(|candidate| match candidate {
                    IntentInspectorField::Instance {
                        leaf: candidate,
                        value,
                    } if candidate == leaf => Some(value.as_ref()),
                    _ => None,
                });
                let current = current.ok_or(IntentInspectorEditError::UnknownTarget)?;
                let output = self
                    .output_descriptor(*leaf)
                    .filter(|output| {
                        output.edit == IntentEditClassification::Instance
                            && output.writable.contains(&leaf.field)
                    })
                    .ok_or(IntentInspectorEditError::UnknownTarget)?;
                debug_assert_eq!(output.port.node, self.node);
                if !inspector_leaf_literal_matches(leaf.field, current, &literal) {
                    return Err(IntentInspectorEditError::InvalidLiteral);
                }
                IntentPatchOperation::SetInstanceLeaf {
                    leaf: *leaf,
                    value: literal,
                }
            }
            _ => return Err(IntentInspectorEditError::TargetValueMismatch),
        };
        Ok(IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![operation],
        ))
    }
}

fn inspector_literal_matches(
    schema: geosolve_sketch_intent::IntentLiteralSchema,
    literal: &IntentLiteral,
) -> bool {
    use geosolve_sketch_intent::IntentLiteralSchema as S;
    matches!(
        (schema, literal),
        (S::Boolean, IntentLiteral::Boolean(_))
            | (S::Integer, IntentLiteral::Integer(_))
            | (S::Natural, IntentLiteral::Natural(_))
            | (S::Text, IntentLiteral::Text(_))
            | (S::Enum, IntentLiteral::Enum(_))
            | (S::Point, IntentLiteral::Point(_))
    ) || matches!(
        (schema, literal),
        (
            S::Quantity(expected),
            IntentLiteral::Quantity { unit: actual, .. }
        ) if expected == *actual
    )
}

fn inspector_leaf_literal_matches(
    field: geosolve_sketch_intent::LeafField,
    current: Option<&IntentLiteral>,
    literal: &IntentLiteral,
) -> bool {
    use geosolve_sketch_intent::{IntentUnit, LeafField};
    let IntentLiteral::Quantity { unit: actual, .. } = literal else {
        return false;
    };
    if let Some(IntentLiteral::Quantity { unit: expected, .. }) = current {
        return expected == actual;
    }
    matches!(
        (field, actual),
        (LeafField::X | LeafField::Y, IntentUnit::Length)
            | (LeafField::Angle, IntentUnit::Angle)
            | (
                LeafField::Weight | LeafField::Parameter,
                IntentUnit::Dimensionless
            )
            | (LeafField::Value, _)
    )
}

/// Rejected schema-generated Inspector mutation.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum IntentInspectorEditError {
    #[error("the Inspector projection is stale")]
    StaleProjection,
    #[error("the Inspector target is not present in the current schema")]
    UnknownTarget,
    #[error("the Inspector literal does not match the target schema")]
    InvalidLiteral,
    #[error("the Inspector target and submitted value kinds disagree")]
    TargetValueMismatch,
}

/// Stable index of one recognized editable token in a generated source view.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct IntentSourceTokenId(pub u32);

/// Typed mutation represented by one recognized source token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentSourceTokenTarget {
    NodeName { node: NodeId },
    Suppressed { node: NodeId },
    Definition { node: NodeId, field: IntentFieldKey },
    Instance { leaf: LeafRef },
}

/// Exact byte range and typed target of one recognized editable token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentSourceToken {
    pub id: IntentSourceTokenId,
    pub start: usize,
    pub end: usize,
    pub target: IntentSourceTokenTarget,
}

impl IntentSourceToken {
    #[must_use]
    pub const fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// Rust-generated TypeScript-shaped source plus its closed editable-token map.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentStructuredSource {
    pub identity: IntentSessionIdentity,
    pub text: String,
    pub tokens: Vec<IntentSourceToken>,
}

impl IntentStructuredSource {
    /// Converts one recognized replacement token into the ordinary exact-CAS
    /// patch vocabulary. Arbitrary source execution is intentionally absent.
    ///
    /// # Errors
    ///
    /// Returns a stale-source, unknown-token, malformed-literal, or invalid-key
    /// error without mutating intent.
    pub fn patch_for_edit(
        &self,
        session: &IntentSession,
        token: IntentSourceTokenId,
        replacement: &str,
    ) -> Result<IntentPatch, IntentSourceEditError> {
        if session.identity() != self.identity {
            return Err(IntentSourceEditError::StaleProjection);
        }
        let token = self
            .tokens
            .iter()
            .find(|candidate| candidate.id == token)
            .ok_or(IntentSourceEditError::UnknownToken)?;
        let operation = match &token.target {
            IntentSourceTokenTarget::NodeName { node } => {
                let value: String = serde_json::from_str(replacement)
                    .map_err(|_| IntentSourceEditError::InvalidReplacement)?;
                IntentPatchOperation::RenameNode {
                    node: *node,
                    name: IntentKey::new(value)
                        .map_err(|_| IntentSourceEditError::InvalidReplacement)?,
                }
            }
            IntentSourceTokenTarget::Suppressed { node } => {
                let suppressed: bool = serde_json::from_str(replacement)
                    .map_err(|_| IntentSourceEditError::InvalidReplacement)?;
                IntentPatchOperation::SetSuppressed {
                    node: *node,
                    suppressed,
                }
            }
            IntentSourceTokenTarget::Definition { node, field } => {
                let value = serde_json::from_str(replacement)
                    .map_err(|_| IntentSourceEditError::InvalidReplacement)?;
                IntentPatchOperation::SetDefinitionField {
                    node: *node,
                    field: field.clone(),
                    value,
                }
            }
            IntentSourceTokenTarget::Instance { leaf } => {
                let value = serde_json::from_str(replacement)
                    .map_err(|_| IntentSourceEditError::InvalidReplacement)?;
                IntentPatchOperation::SetInstanceLeaf { leaf: *leaf, value }
            }
        };
        Ok(IntentPatch::new(
            self.identity,
            IntentPatchPolicy::RetainFailedIntent,
            vec![operation],
        ))
    }
}

/// Recognized structured-source edit failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum IntentSourceEditError {
    #[error("the structured-source projection is stale")]
    StaleProjection,
    #[error("the structured-source token is unknown")]
    UnknownToken,
    #[error("the structured-source replacement is not a valid typed token")]
    InvalidReplacement,
}

/// Complete deterministic Design-panel projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentWorkbenchProjection {
    pub identity: IntentSessionIdentity,
    pub outline: Vec<IntentOutlineCell>,
    pub structured_source: IntentStructuredSource,
    pub history: IntentHistoryProjection,
    pub latest_disposition: Option<IntentAttemptDisposition>,
    pub latest_diagnostic: Option<IntentKey>,
}

impl IntentWorkbenchProjection {
    /// Builds all durable Design-panel projections from one session snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if a previously validated organization references a missing
    /// cell or declaration, or if closed Rust values stop serializing.
    #[must_use]
    pub fn from_session(session: &IntentSession) -> Self {
        let organization = session.organization();
        let graph = session.graph();
        let failed = session
            .latest_attempt()
            .filter(|attempt| attempt.disposition == IntentAttemptDisposition::RetainedFailed)
            .map_or_else(Default::default, |attempt| attempt.failed_nodes.clone());
        let outline = organization
            .cell_order()
            .iter()
            .map(|cell_id| {
                let cell = &organization.cells()[cell_id];
                IntentOutlineCell {
                    cell: *cell_id,
                    name: cell.name.clone(),
                    declarations: cell
                        .declarations
                        .iter()
                        .map(|node_id| {
                            let node = &graph.nodes()[node_id];
                            outline_declaration(
                                node,
                                organization.node_names()[node_id].clone(),
                                failed.contains(node_id),
                            )
                        })
                        .collect(),
                }
            })
            .collect();
        let structured_source = structured_source(session);
        Self {
            identity: session.identity(),
            outline,
            structured_source,
            history: session.history_projection(),
            latest_disposition: session.latest_attempt().map(|attempt| attempt.disposition),
            latest_diagnostic: session
                .latest_attempt()
                .and_then(|attempt| attempt.diagnostic.clone()),
        }
    }

    /// Returns one schema-generated Inspector projection.
    ///
    /// # Panics
    ///
    /// Panics only if a session that already passed structural validation
    /// contains more children than the public bounded schema permits.
    #[must_use]
    pub fn inspector(
        &self,
        session: &IntentSession,
        node_id: NodeId,
    ) -> Option<IntentInspectorProjection> {
        if session.identity() != self.identity {
            return None;
        }
        IntentInspectorProjection::from_session(session, node_id)
    }
}

fn outline_declaration(
    node: &IntentNode,
    name: IntentKey,
    retained_failure: bool,
) -> IntentOutlineDeclaration {
    IntentOutlineDeclaration {
        node: node.id,
        symbol: node.symbol.clone(),
        name,
        kind: IntentGraphNodeKind::from_kind(&node.kind),
        suppressed: node.suppressed,
        retained_failure,
        dependencies: node.dependencies().into_iter().collect(),
    }
}

struct SourceWriter {
    text: String,
    tokens: Vec<IntentSourceToken>,
}

impl SourceWriter {
    fn token(&mut self, value: &str, target: IntentSourceTokenTarget) {
        let start = self.text.len();
        self.text.push_str(value);
        let end = self.text.len();
        let id = IntentSourceTokenId(
            u32::try_from(self.tokens.len()).expect("bounded intent token count fits u32"),
        );
        self.tokens.push(IntentSourceToken {
            id,
            start,
            end,
            target,
        });
    }
}

fn structured_source(session: &IntentSession) -> IntentStructuredSource {
    let mut writer = SourceWriter {
        text: concat!(
            "import type { IntentSourceSnapshot } from \"@geosolve/intent\";\n\n",
            "export const sketch = {\n  cells: [\n"
        )
        .to_owned(),
        tokens: Vec::new(),
    };
    let graph = session.graph();
    let organization = session.organization();
    for cell_id in organization.cell_order() {
        let cell = &organization.cells()[cell_id];
        writer.text.push_str("    {\n      cell: ");
        writer.text.push_str(
            &serde_json::to_string(&cell.id.to_string())
                .expect("cell identity is infallibly serializable"),
        );
        writer.text.push_str(",\n      name: ");
        writer.text.push_str(
            &serde_json::to_string(cell.name.as_str())
                .expect("cell name is infallibly serializable"),
        );
        writer.text.push_str(",\n      declarations: [\n");
        for node_id in &cell.declarations {
            let node = &graph.nodes()[node_id];
            writer.text.push_str("        {\n          node: ");
            writer.text.push_str(
                &serde_json::to_string(&node.id.to_string())
                    .expect("node identity is infallibly serializable"),
            );
            writer.text.push_str(",\n          symbol: ");
            writer.text.push_str(
                &serde_json::to_string(node.symbol.as_str())
                    .expect("node symbol is infallibly serializable"),
            );
            writer.text.push_str(",\n          name: ");
            let name = &organization.node_names()[node_id];
            let name_json =
                serde_json::to_string(name.as_str()).expect("node name is infallibly serializable");
            writer.token(
                &name_json,
                IntentSourceTokenTarget::NodeName { node: *node_id },
            );
            writer.text.push_str(",\n          kind: ");
            writer.text.push_str(
                &serde_json::to_string(&IntentGraphNodeKind::from_kind(&node.kind))
                    .expect("compact node kind is infallibly serializable"),
            );
            writer.text.push_str(",\n          suppressed: ");
            writer.token(
                if node.suppressed { "true" } else { "false" },
                IntentSourceTokenTarget::Suppressed { node: *node_id },
            );
            write_input_map(&mut writer, &node.inputs);
            write_literal_map(
                &mut writer,
                "fields",
                node.fields.iter().map(|(field, value)| {
                    (
                        field.0.as_str().to_owned(),
                        value,
                        IntentSourceTokenTarget::Definition {
                            node: *node_id,
                            field: field.clone(),
                        },
                    )
                }),
            );
            let instance = node
                .ports
                .values()
                .flat_map(|port| {
                    port.writable.iter().map(|field| LeafRef {
                        node: *node_id,
                        port: port.id,
                        field: *field,
                    })
                })
                .filter_map(|leaf| {
                    session.instance().values().get(&leaf).map(|value| {
                        (
                            leaf.to_string(),
                            value,
                            IntentSourceTokenTarget::Instance { leaf },
                        )
                    })
                });
            write_literal_map(&mut writer, "instance", instance);
            writer.text.push_str("\n        },\n");
        }
        writer.text.push_str("      ],\n    },\n");
    }
    writer
        .text
        .push_str("  ],\n} satisfies IntentSourceSnapshot;\n");
    IntentStructuredSource {
        identity: session.identity(),
        text: writer.text,
        tokens: writer.tokens,
    }
}

fn write_input_map(writer: &mut SourceWriter, inputs: &BTreeMap<InputSlot, IntentPortRef>) {
    writer.text.push_str(",\n          inputs: {");
    for (index, (slot, source)) in inputs.iter().enumerate() {
        if index > 0 {
            writer.text.push(',');
        }
        writer.text.push_str("\n            ");
        writer.text.push_str(
            &serde_json::to_string(&slot.to_string())
                .expect("input slot is infallibly serializable"),
        );
        writer.text.push_str(": ");
        writer.text.push_str(
            &serde_json::to_string(source)
                .expect("stable typed port reference is infallibly serializable"),
        );
    }
    if !inputs.is_empty() {
        writer.text.push_str("\n          ");
    }
    writer.text.push('}');
}

fn write_literal_map<'a>(
    writer: &mut SourceWriter,
    label: &str,
    values: impl IntoIterator<Item = (String, &'a IntentLiteral, IntentSourceTokenTarget)>,
) {
    writer.text.push_str(",\n          ");
    writer.text.push_str(label);
    writer.text.push_str(": {");
    let mut values = values
        .into_iter()
        .map(|(key, value, target)| (key, (value, target)))
        .collect::<BTreeMap<_, _>>();
    for (index, (key, (value, target))) in values.iter_mut().enumerate() {
        if index > 0 {
            writer.text.push(',');
        }
        writer.text.push_str("\n            ");
        writer
            .text
            .push_str(&serde_json::to_string(key).expect("literal key is infallibly serializable"));
        writer.text.push_str(": ");
        let literal =
            serde_json::to_string(value).expect("intent literal is infallibly serializable");
        writer.token(&literal, target.clone());
    }
    if !values.is_empty() {
        writer.text.push_str("\n          ");
    }
    writer.text.push('}');
}
