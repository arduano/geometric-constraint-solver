// SPDX-License-Identifier: GPL-3.0-or-later
//! Source properties and mutation descriptions, independent of host publication policy.
use crate::{
    CompiledManagedSource, ManagedAuthoredMetadata, ManagedControl, ManagedControlManifest,
    ManagedMetadataTarget, ManagedPresentation, ManagedSketchMutation, ManagedStatement,
    ManagedValue, SemanticSymbol,
};
use std::collections::BTreeSet;

/// Stable source target. Parameter IDs resolve through the accepted control manifest.
#[derive(Clone, Debug)]
pub enum ManagedPropertyTarget {
    Document,
    Declaration(String),
    Parameter(String),
}

/// Borrowed accepted source projections. Hosts validate their revision/gesture authority
/// before requesting a description; the ordinary source compiler validates publication.
#[derive(Debug)]
pub struct ManagedSourceProperties<'a> {
    compiled: &'a CompiledManagedSource,
    controls: &'a ManagedControlManifest,
}

impl<'a> ManagedSourceProperties<'a> {
    pub fn new(compiled: &'a CompiledManagedSource, controls: &'a ManagedControlManifest) -> Self {
        Self { compiled, controls }
    }

    pub fn is_dimension(&self, symbol: &SemanticSymbol) -> bool {
        self.compiled.ir.statements.iter().any(|statement| matches!(statement,
            ManagedStatement::Declaration { symbol: id, builder_path, .. }
                if id == &symbol.0 && builder_path.first().is_some_and(|namespace| namespace == "dimension")))
    }

    /// Describes one source property change without compiling or publishing it.
    ///
    /// # Errors
    /// Rejects absent declarations, unknown or unnamed parameters and incompatible values.
    pub fn metadata_mutation(
        &self,
        target: ManagedPropertyTarget,
        property: String,
        value: Option<ManagedValue>,
    ) -> Result<ManagedSketchMutation, String> {
        let target = match target {
            ManagedPropertyTarget::Document => ManagedMetadataTarget::Document,
            ManagedPropertyTarget::Declaration(id) => {
                if !self.compiled.ir.statements.iter().any(|statement| {
                    matches!(statement,
                    ManagedStatement::Declaration { symbol, .. } if symbol == &id)
                }) {
                    return Err("The source declaration is unavailable".into());
                }
                ManagedMetadataTarget::Declaration { declaration: id }
            }
            ManagedPropertyTarget::Parameter(id) => {
                let control = self
                    .controls
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
        if !matches!(
            value,
            None | Some(ManagedValue::Bool(_) | ManagedValue::String(_))
        ) {
            return Err("Source properties accept text, booleans, or reset".into());
        }
        Ok(ManagedSketchMutation::SetMetadata {
            target,
            property,
            value,
        })
    }

    /// Describes exact extraction using a collision-free name from the full source scope.
    ///
    /// # Errors
    /// Rejects unavailable, already-named, expression-owned or nonnumeric values.
    pub fn extract_parameter(
        &self,
        id: &str,
        mut presentation: ManagedPresentation,
    ) -> Result<ManagedSketchMutation, String> {
        let control = self
            .controls
            .controls
            .iter()
            .find(|control| control.id.0 == id)
            .ok_or("The source parameter is unavailable")?;
        if !parameter_can_extract(control) {
            return Err("This source value cannot be extracted into a named parameter".into());
        }
        let mut names = BTreeSet::new();
        for import in &self.compiled.ir.imports {
            names.extend(import.bindings.iter().map(String::as_str));
        }
        for statement in &self.compiled.ir.statements {
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
            self.compiled
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
        let symbol = existing_binding
            .or_else(|| {
                (1..=names.len() + 1)
                    .map(|index| format!("parameter{index}"))
                    .find(|name| !names.contains(name.as_str()))
            })
            .ok_or("No unused source parameter name is available")?;
        if presentation.label.is_none() {
            presentation.label = Some(managed_control_label(control));
        }
        Ok(ManagedSketchMutation::ExtractParameter {
            declaration: control.source.declaration.0.clone(),
            path: control.source.path.0.clone(),
            variable: symbol.clone(),
            symbol,
            presentation,
        })
    }
}

/// Whether an accepted scalar token can become a named source parameter.
pub fn parameter_can_extract(control: &ManagedControl) -> bool {
    !control.is_public_parameter
        && control.token().is_some()
        && matches!(
            control.value,
            ManagedValue::Number(_) | ManagedValue::Unit(_)
        )
}

/// User-authored label or a deterministic path through exact source ownership.
pub fn managed_control_label(control: &crate::ManagedControl) -> String {
    if let Some(label) = control
        .presentation
        .label
        .as_ref()
        .filter(|label| !label.is_empty())
    {
        return label.clone();
    }
    let path = control
        .source
        .path
        .0
        .iter()
        .map(|segment| match segment {
            crate::ManagedPathSegment::Field(field) => field.clone(),
            crate::ManagedPathSegment::Index(index) => format!("item {}", index + 1),
            crate::ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(" · ");
    if path.is_empty() {
        control.source.declaration.0.clone()
    } else {
        format!("{} · {path}", control.source.declaration.0)
    }
}

/// Display labels follow exact source order and compiled consumer ownership.
pub fn managed_parameter_consumer_labels(
    compiled: &CompiledManagedSource,
    metadata: &ManagedAuthoredMetadata,
    control: &ManagedControl,
) -> Vec<String> {
    let mut consumers = BTreeSet::new();
    for consumer in &control.consumers {
        match &consumer.target {
            crate::ManagedControlConsumerTarget::Declaration { declaration, .. } => {
                consumers.insert(declaration.clone());
            }
            crate::ManagedControlConsumerTarget::Generated { address, .. } => {
                consumers.insert(SemanticSymbol(address.invocation.clone()));
            }
        }
    }
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
