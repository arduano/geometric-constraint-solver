// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic read-only workbench projections over canonical design intent.
//!
//! These DTOs contain no geometry equations and never become materialization
//! authority. They expose the same stable declarations to Outline, Inspector,
//! structured-source, History, native hosts, and DOM-free RPC consumers.

use std::collections::BTreeMap;
use std::ops::Range;

use geosolve_sketch_intent::{
    CellId, IntentAttemptDisposition, IntentDefinitionFieldSchema, IntentFieldKey,
    IntentHistoryProjection, IntentKey, IntentLiteral, IntentNode, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortKind, IntentPortRef, IntentSession,
    IntentSessionIdentity, LeafRef, NodeId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One declaration row in the non-semantic Outline projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentOutlineDeclaration {
    pub node: NodeId,
    pub symbol: IntentKey,
    pub name: IntentKey,
    pub kind: IntentNodeKind,
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
        schema: IntentDefinitionFieldSchema,
        value: Option<IntentLiteral>,
    },
    Instance {
        leaf: LeafRef,
        port_kind: IntentPortKind,
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
    pub kind: IntentNodeKind,
    pub suppressed: bool,
    pub retained_failure: bool,
    pub inputs: Vec<(geosolve_sketch_intent::InputSlot, IntentPortRef)>,
    pub fields: Vec<IntentInspectorField>,
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
        let node = session.graph().node(node_id)?;
        let name = session.organization().node_names().get(&node_id)?.clone();
        let retained_failure = session.latest_attempt().is_some_and(|attempt| {
            attempt.disposition == IntentAttemptDisposition::RetainedFailed
                && attempt.failed_nodes.contains(&node_id)
        });
        let schema = node
            .kind
            .schema(u16::try_from(node.child_order.len()).expect("validated child count fits u16"));
        let mut fields = schema
            .fields
            .into_iter()
            .map(|field| IntentInspectorField::Definition {
                value: node.fields.get(&field.field).cloned(),
                schema: field,
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
                    port_kind: port.kind,
                    value: session.instance().values().get(&leaf).cloned(),
                });
            }
        }
        Some(IntentInspectorProjection {
            node: node_id,
            symbol: node.symbol.clone(),
            name,
            kind: node.kind.clone(),
            suppressed: node.suppressed,
            retained_failure,
            inputs: node
                .inputs
                .iter()
                .map(|(slot, source)| (*slot, *source))
                .collect(),
            fields,
        })
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
        kind: node.kind.clone(),
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
            "import { design } from \"@geosolve/intent\";\n\n",
            "export const sketch = design(({ cell, declare }) => {\n"
        )
        .to_owned(),
        tokens: Vec::new(),
    };
    let graph = session.graph();
    let organization = session.organization();
    for cell_id in organization.cell_order() {
        let cell = &organization.cells()[cell_id];
        writer.text.push_str("  cell(");
        writer.text.push_str(
            &serde_json::to_string(&cell.id.to_string())
                .expect("cell identity is infallibly serializable"),
        );
        writer.text.push_str(", ");
        writer.text.push_str(
            &serde_json::to_string(cell.name.as_str())
                .expect("cell name is infallibly serializable"),
        );
        writer.text.push_str(", () => {\n");
        for node_id in &cell.declarations {
            let node = &graph.nodes()[node_id];
            writer.text.push_str("    declare(");
            writer.text.push_str(
                &serde_json::to_string(&node.id.to_string())
                    .expect("node identity is infallibly serializable"),
            );
            writer.text.push_str(", ");
            let name = &organization.node_names()[node_id];
            let name_json =
                serde_json::to_string(name.as_str()).expect("node name is infallibly serializable");
            writer.token(
                &name_json,
                IntentSourceTokenTarget::NodeName { node: *node_id },
            );
            writer.text.push_str(", ");
            writer.text.push_str(
                &serde_json::to_string(&node.kind).expect("node kind is infallibly serializable"),
            );
            writer.text.push_str(", {\n      suppressed: ");
            writer.token(
                if node.suppressed { "true" } else { "false" },
                IntentSourceTokenTarget::Suppressed { node: *node_id },
            );
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
            writer.text.push_str("\n    });\n");
        }
        writer.text.push_str("  });\n");
    }
    writer.text.push_str("});\n");
    IntentStructuredSource {
        identity: session.identity(),
        text: writer.text,
        tokens: writer.tokens,
    }
}

fn write_literal_map<'a>(
    writer: &mut SourceWriter,
    label: &str,
    values: impl IntoIterator<Item = (String, &'a IntentLiteral, IntentSourceTokenTarget)>,
) {
    writer.text.push_str(",\n      ");
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
        writer.text.push_str("\n        ");
        writer
            .text
            .push_str(&serde_json::to_string(key).expect("literal key is infallibly serializable"));
        writer.text.push_str(": ");
        let literal =
            serde_json::to_string(value).expect("intent literal is infallibly serializable");
        writer.token(&literal, target.clone());
    }
    if !values.is_empty() {
        writer.text.push_str("\n      ");
    }
    writer.text.push('}');
}
