// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-owned presentation controls over the ordinary prepared mutation boundary.

use std::collections::BTreeMap;

use geosolve_sketch_code::{
    ManagedControl, ManagedMetadataTarget, ManagedPresentation, ManagedStatement, SemanticSymbol,
};

use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(super) enum MetadataTarget {
    Document,
    Dimension { id: String },
    Parameter { id: String },
    Declaration { id: String },
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct AuthoringMetadataSnapshot {
    authority: String,
    target: MetadataTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_key_constraint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_key_parameter: Option<bool>,
    has_key_override: bool,
    editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    can_extract: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AuthoringDocumentSnapshot {
    authority: String,
    title: String,
    description: String,
    are_key_constraints_by_default: bool,
    editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataRequest {
    authority: String,
    target: MetadataTarget,
    changes: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtractRequest {
    authority: String,
    id: String,
    label: Option<String>,
    description: Option<String>,
    is_key_parameter: Option<bool>,
}

impl WorkbenchBridge {
    pub(super) fn selected_authoring_metadata(&self) -> Option<AuthoringMetadataSnapshot> {
        let code = self.code_project.as_ref()?;
        let symbol = code.selected_managed_declaration(self.editor()).ok()??;
        let compiled = code.managed_compilation()?;
        let dimension = compiled.ir.statements.iter().any(|statement| matches!(statement,
            ManagedStatement::Declaration { symbol: id, builder_path, .. }
                if id == &symbol.0 && builder_path.first().is_some_and(|namespace| namespace == "dimension")));
        self.declaration_metadata_snapshot(&symbol, dimension)
    }

    pub(super) fn parameter_consumer_labels(&self, control: &ManagedControl) -> Vec<String> {
        let Some(code) = self.code_project.as_ref() else {
            return Vec::new();
        };
        let Ok(metadata) = code.authored_metadata_cached() else {
            return Vec::new();
        };
        let mut consumers = std::collections::BTreeSet::new();
        for consumer in &control.consumers {
            match &consumer.target {
                geosolve_sketch_code::ManagedControlConsumerTarget::Declaration {
                    declaration,
                    ..
                } => {
                    consumers.insert(declaration.clone());
                }
                geosolve_sketch_code::ManagedControlConsumerTarget::Generated {
                    address, ..
                } => {
                    consumers.insert(SemanticSymbol(address.invocation.clone()));
                }
            }
        }
        let Some(compiled) = code.managed_compilation() else {
            return Vec::new();
        };
        compiled
            .ir
            .statements
            .iter()
            .filter_map(|statement| {
                let ManagedStatement::Declaration { symbol, .. } = statement else {
                    return None;
                };
                let symbol = SemanticSymbol(symbol.clone());
                consumers.contains(&symbol).then(|| {
                    metadata
                        .declarations
                        .get(&symbol)
                        .and_then(|presentation| presentation.label.as_ref())
                        .filter(|label| !label.is_empty())
                        .cloned()
                        .unwrap_or(symbol.0)
                })
            })
            .collect()
    }

    pub(super) fn authoring_metadata_authority(&self) -> String {
        geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({
                "instance": self.dimension_instance(),
                "session": self.code_project.as_ref().map(CodeProjectWorkbench::code_session_identity),
                "source": self.code_project.as_ref().and_then(CodeProjectWorkbench::managed_compilation).map(|compiled| &compiled.ir.source_digest),
            }).to_string().as_bytes(),
        ).to_string()
    }

    fn metadata_blocked_reason(&self) -> Option<String> {
        if self.pending_managed_mutation.is_some()
            || self.captured_pointer.is_some()
            || self.active_tool != "select"
            || self.editor().editor().active_pointer_gesture().is_some()
        {
            return Some(
                "Finish the current tool or gesture before editing source properties".into(),
            );
        }
        let code = self.code_project.as_ref()?;
        code.metadata_edit_blocked_reason()
    }

    pub(super) fn validate_metadata_authority(&self, authority: &str) -> Result<(), String> {
        if self
            .code_project
            .as_ref()
            .and_then(CodeProjectWorkbench::managed_compilation)
            .is_none()
        {
            return Err("This document has no managed source properties".into());
        }
        if authority != self.authoring_metadata_authority() {
            return Err("These properties belong to an older source revision".into());
        }
        if let Some(reason) = self.metadata_blocked_reason() {
            return Err(reason);
        }
        Ok(())
    }

    pub(super) fn authoring_document_snapshot(&self) -> Option<AuthoringDocumentSnapshot> {
        let document = self
            .code_project
            .as_ref()?
            .authored_metadata_cached()
            .ok()?
            .document
            .clone();
        let reason = self.metadata_blocked_reason();
        Some(AuthoringDocumentSnapshot {
            authority: self.authoring_metadata_authority(),
            title: document.title.unwrap_or_default(),
            description: document.description.unwrap_or_default(),
            are_key_constraints_by_default: document
                .dimensions
                .and_then(|defaults| defaults.are_key_constraints_by_default)
                .unwrap_or(false),
            editable: reason.is_none(),
            reason,
        })
    }

    pub(super) fn declaration_metadata_snapshot(
        &self,
        symbol: &SemanticSymbol,
        dimension: bool,
    ) -> Option<AuthoringMetadataSnapshot> {
        let metadata = self
            .code_project
            .as_ref()?
            .authored_metadata_cached()
            .ok()?;
        let presentation = metadata.declarations.get(symbol)?.clone();
        let default = metadata
            .document
            .dimensions
            .as_ref()
            .and_then(|defaults| defaults.are_key_constraints_by_default)
            .unwrap_or(false);
        let reason = self.metadata_blocked_reason();
        Some(AuthoringMetadataSnapshot {
            authority: self.authoring_metadata_authority(),
            target: if dimension {
                MetadataTarget::Dimension {
                    id: symbol.0.clone(),
                }
            } else {
                MetadataTarget::Declaration {
                    id: symbol.0.clone(),
                }
            },
            label: presentation.label,
            description: presentation.description,
            is_key_constraint: dimension
                .then_some(presentation.is_key_constraint.unwrap_or(default)),
            is_key_parameter: None,
            has_key_override: presentation.is_key_constraint.is_some(),
            editable: reason.is_none(),
            reason,
            can_extract: false,
        })
    }

    pub(super) fn parameter_metadata_snapshot(
        &self,
        control: &ManagedControl,
    ) -> Option<AuthoringMetadataSnapshot> {
        self.code_project.as_ref()?.managed_compilation()?;
        if !matches!(
            control.value,
            ManagedValue::Number(_) | ManagedValue::Unit(_)
        ) {
            return None;
        }
        let is_named = control.is_public_parameter;
        let presentation = control.presentation.clone();
        if !is_named && control.token().is_none() && presentation == ManagedPresentation::default()
        {
            return None;
        }
        let blocked = self.metadata_blocked_reason();
        let can_extract = blocked.is_none()
            && !is_named
            && control.token().is_some()
            && matches!(
                control.value,
                ManagedValue::Number(_) | ManagedValue::Unit(_)
            );
        let reason = blocked.or_else(|| {
            (!is_named).then(|| {
                if can_extract {
                    "Make this value a named parameter to edit its presentation".into()
                } else {
                    "This value is defined by its source expression".into()
                }
            })
        });
        Some(AuthoringMetadataSnapshot {
            authority: self.authoring_metadata_authority(),
            target: MetadataTarget::Parameter {
                id: control.id.0.clone(),
            },
            label: presentation.label,
            description: presentation.description,
            is_key_constraint: None,
            is_key_parameter: Some(presentation.is_key_parameter.unwrap_or(false)),
            has_key_override: presentation.is_key_parameter.is_some(),
            editable: is_named && reason.is_none(),
            reason,
            can_extract,
        })
    }

    pub(super) fn dispatch_authoring_metadata(
        &mut self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        if command == "authoring.parameter.extract" {
            return self.extract_authoring_parameter(decode_payload(payload)?);
        }
        let mutation = self.metadata_source_mutation(payload)?;
        self.begin_selected_structured_managed_mutation("Edit source properties", mutation)
    }

    pub(super) fn metadata_source_mutation(
        &self,
        payload: serde_json::Value,
    ) -> Result<ManagedSketchMutation, String> {
        let request: MetadataRequest = decode_payload(payload)?;
        self.validate_metadata_authority(&request.authority)?;
        if request.changes.len() != 1 {
            return Err("Edit one source property at a time".into());
        }
        let (property, value) = request.changes.into_iter().next().unwrap();
        let code = self.code_project.as_ref().unwrap();
        let compiled = code.managed_compilation().unwrap();
        let target = match request.target {
            MetadataTarget::Document => ManagedMetadataTarget::Document,
            MetadataTarget::Declaration { id } | MetadataTarget::Dimension { id } => {
                if !compiled.ir.statements.iter().any(|statement| matches!(statement, ManagedStatement::Declaration { symbol, .. } if symbol == &id)) {
                    return Err("The source declaration is unavailable".into());
                }
                ManagedMetadataTarget::Declaration { declaration: id }
            }
            MetadataTarget::Parameter { id } => {
                let manifest = code.managed_controls_cached()?;
                let control = manifest
                    .controls
                    .iter()
                    .find(|control| control.id.0 == id)
                    .ok_or("The source parameter is unavailable")?;
                if !control.is_public_parameter {
                    return Err(
                        "Make this value a named parameter before editing its presentation".into(),
                    );
                }
                ManagedMetadataTarget::Parameter {
                    declaration: control.source.declaration.0.clone(),
                }
            }
        };
        let value = match value {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(value) => Some(ManagedValue::Bool(value)),
            serde_json::Value::String(value) => Some(ManagedValue::String(value)),
            _ => return Err("Source properties accept text, booleans, or reset".into()),
        };
        Ok(ManagedSketchMutation::SetMetadata {
            target,
            property,
            value,
        })
    }

    fn extract_authoring_parameter(&mut self, request: ExtractRequest) -> Result<(), String> {
        self.validate_metadata_authority(&request.authority)?;
        let code = self.code_project.as_ref().unwrap();
        let manifest = code.managed_controls_cached()?;
        let control = manifest
            .controls
            .iter()
            .find(|control| control.id.0 == request.id)
            .ok_or("The source parameter is unavailable")?;
        if !self
            .parameter_metadata_snapshot(control)
            .is_some_and(|metadata| metadata.can_extract)
        {
            return Err("This source value cannot be extracted into a named parameter".into());
        }
        let compiled = code.managed_compilation().unwrap();
        let mut names = std::collections::BTreeSet::new();
        for import in &compiled.ir.imports {
            names.extend(import.bindings.iter().map(String::as_str));
        }
        for statement in &compiled.ir.statements {
            match statement {
                ManagedStatement::Declaration {
                    variable, symbol, ..
                } => {
                    names.insert(variable.as_str());
                    names.insert(symbol.as_str());
                }
                ManagedStatement::Binding {
                    variable,
                    parameter,
                    ..
                } => {
                    names.insert(variable.as_str());
                    if let Some(parameter) = parameter {
                        names.insert(parameter.symbol.as_str());
                    }
                }
                _ => {}
            }
        }
        let existing_binding =
            compiled
                .ir
                .statements
                .iter()
                .find_map(|statement| match statement {
                    ManagedStatement::Binding {
                        variable,
                        parameter: None,
                        ..
                    } if variable == &control.source.declaration.0
                        && control.source.path.0.is_empty() =>
                    {
                        Some(variable.clone())
                    }
                    _ => None,
                });
        let symbol = existing_binding.unwrap_or_else(|| {
            (1..=names.len() + 1)
                .map(|index| format!("parameter{index}"))
                .find(|name| !names.contains(name.as_str()))
                .expect("one more candidate than occupied source names")
        });
        let presentation = ManagedPresentation {
            label: request
                .label
                .or_else(|| Some(managed_control_label(control))),
            description: request.description,
            is_key_parameter: request.is_key_parameter,
            is_key_constraint: None,
        };
        let mutation = ManagedSketchMutation::ExtractParameter {
            declaration: control.source.declaration.0.clone(),
            path: control.source.path.0.clone(),
            variable: symbol.clone(),
            symbol,
            presentation,
        };
        self.begin_selected_structured_managed_mutation("Make named parameter", mutation)
    }
}
