// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-owned presentation controls over the ordinary prepared mutation boundary.

use std::collections::BTreeMap;

use geosolve_sketch_code::{
    ManagedControl, ManagedPresentation, ManagedPropertyTarget, ManagedSourceProperties,
    ManagedStatement, SemanticSymbol, managed_parameter_consumer_labels, parameter_can_extract,
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

impl ChromeRead<'_> {
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
        let Some(compiled) = code.managed_compilation() else {
            return Vec::new();
        };
        let Ok(metadata) = code.authored_metadata_cached() else {
            return Vec::new();
        };
        managed_parameter_consumer_labels(compiled, &metadata, control)
    }

    pub(super) fn authoring_metadata_authority(&self) -> String {
        geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({
                "instance": self.dimension_instance(),
                "session": self.code_project.as_ref().map(CodeChrome::code_session_identity),
                "source": self.code_project.as_ref().and_then(CodeChrome::managed_compilation).map(|compiled| &compiled.ir.source_digest),
            }).to_string().as_bytes(),
        ).to_string()
    }

    fn metadata_blocked_reason(&self) -> Option<String> {
        if self.pending || self.interaction_blocked {
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
            .and_then(CodeChrome::managed_compilation)
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
        let can_extract = blocked.is_none() && parameter_can_extract(control);
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
        let target = match request.target {
            MetadataTarget::Document => ManagedPropertyTarget::Document,
            MetadataTarget::Declaration { id } | MetadataTarget::Dimension { id } => {
                ManagedPropertyTarget::Declaration(id)
            }
            MetadataTarget::Parameter { id } => ManagedPropertyTarget::Parameter(id),
        };
        let value = match value {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(value) => Some(ManagedValue::Bool(value)),
            serde_json::Value::String(value) => Some(ManagedValue::String(value)),
            _ => return Err("Source properties accept text, booleans, or reset".into()),
        };
        let code = self.code_project.as_ref().unwrap();
        ManagedSourceProperties::new(
            code.managed_compilation().unwrap(),
            code.managed_controls_cached()?.as_ref(),
        )
        .metadata_mutation(target, property, value)
    }

    pub(super) fn extraction_source_mutation(
        &self,
        payload: serde_json::Value,
    ) -> Result<ManagedSketchMutation, String> {
        let request: ExtractRequest = decode_payload(payload)?;
        self.validate_metadata_authority(&request.authority)?;
        let code = self.code_project.as_ref().unwrap();
        let controls = code.managed_controls_cached()?;
        let control = controls
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
        ManagedSourceProperties::new(code.managed_compilation().unwrap(), &controls)
            .extract_parameter(
                &request.id,
                ManagedPresentation {
                    label: request.label,
                    description: request.description,
                    is_key_parameter: request.is_key_parameter,
                    is_key_constraint: None,
                },
            )
    }
}

impl WorkbenchBridge {
    pub(super) fn selected_authoring_metadata(&self) -> Option<AuthoringMetadataSnapshot> {
        self.chrome_read().selected_authoring_metadata()
    }
    pub(super) fn parameter_consumer_labels(&self, control: &ManagedControl) -> Vec<String> {
        self.chrome_read().parameter_consumer_labels(control)
    }
    pub(super) fn authoring_metadata_authority(&self) -> String {
        self.chrome_read().authoring_metadata_authority()
    }
    pub(super) fn validate_metadata_authority(&self, authority: &str) -> Result<(), String> {
        self.chrome_read().validate_metadata_authority(authority)
    }
    pub(super) fn authoring_document_snapshot(&self) -> Option<AuthoringDocumentSnapshot> {
        self.chrome_read().authoring_document_snapshot()
    }
    pub(super) fn declaration_metadata_snapshot(
        &self,
        symbol: &SemanticSymbol,
        dimension: bool,
    ) -> Option<AuthoringMetadataSnapshot> {
        self.chrome_read()
            .declaration_metadata_snapshot(symbol, dimension)
    }
    pub(super) fn parameter_metadata_snapshot(
        &self,
        control: &ManagedControl,
    ) -> Option<AuthoringMetadataSnapshot> {
        self.chrome_read().parameter_metadata_snapshot(control)
    }
    pub(super) fn metadata_source_mutation(
        &self,
        payload: serde_json::Value,
    ) -> Result<ManagedSketchMutation, String> {
        self.chrome_read().metadata_source_mutation(payload)
    }
    pub(super) fn extraction_source_mutation(
        &self,
        payload: serde_json::Value,
    ) -> Result<ManagedSketchMutation, String> {
        self.chrome_read().extraction_source_mutation(payload)
    }
    pub(super) fn dispatch_authoring_metadata(
        &mut self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        if command == "authoring.parameter.extract" {
            let mutation = self.extraction_source_mutation(payload)?;
            return self
                .begin_selected_structured_managed_mutation("Make named parameter", mutation);
        }
        let mutation = self.metadata_source_mutation(payload)?;
        self.begin_selected_structured_managed_mutation("Edit source properties", mutation)
    }
}
