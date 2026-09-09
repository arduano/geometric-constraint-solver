// SPDX-License-Identifier: GPL-3.0-or-later

//! Admission of evaluated generator programs. These records grant no lexical
//! source authority and never construct a `ManagedDocument` or compiler receipt.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::DocumentId;
use geosolve_sketch_intent::{IntentSessionId, intent_content_digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AuthoringDeclaration, AuthoringProgram, CodeCompositionError, FeatureKind,
    ManagedDocumentPresentation, ManagedOrganization, ManagedOutput, ManagedPathSegment,
    ManagedPresentation, ManagedSpan, ManagedValue, MaterializedCodeProject, SKETCH_CODE_SDK_ABI,
    SemanticOutputPath, SemanticSymbol, UnitLiteral,
};

pub const GENERATED_SKETCH_ARTIFACT_FORMAT: &str = "geosolve-generated-sketch-v1";
pub const GENERATED_SKETCH_ARTIFACT_LIMIT: usize = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 65_536;
const MAX_VALUE_DEPTH: usize = 64;
const MAX_VALUES: usize = 262_144;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GeneratedValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Unit(UnitLiteral),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
    Reference(GeneratedReference),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedReference {
    pub identity: Vec<String>,
    pub path: Vec<ManagedPathSegment>,
    pub kind: FeatureKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedDeclaration {
    pub identity: Vec<String>,
    pub family: String,
    pub arguments: GeneratedValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedApplication {
    pub identity: Vec<String>,
    pub declarations: Vec<Vec<String>>,
    pub output: GeneratedValue,
    pub presentation: ManagedPresentation,
    pub input_presentation: BTreeMap<String, ManagedPresentation>,
    pub inputs: GeneratedValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedParameter {
    pub id: String,
    pub value: GeneratedValue,
    pub presentation: ManagedPresentation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedGroup {
    pub name: String,
    pub declarations: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSketchArtifact {
    pub format: String,
    pub sdk_abi: String,
    pub declarations: Vec<GeneratedDeclaration>,
    pub applications: Vec<GeneratedApplication>,
    pub parameters: Vec<GeneratedParameter>,
    pub groups: Vec<GeneratedGroup>,
    pub suppressions: Vec<Vec<String>>,
    pub document: ManagedDocumentPresentation,
    pub output: GeneratedValue,
}

/// Opaque admitted evaluated data. No managed-source edit token can be derived
/// from this type. Consumers may inspect the original identity vectors.
#[derive(Clone, Debug)]
pub struct ValidatedGeneratedSketch {
    artifact: GeneratedSketchArtifact,
    digest: String,
    pub(crate) program: AuthoringProgram,
    pub(crate) references: Vec<(SemanticSymbol, SemanticOutputPath, FeatureKind)>,
    pub(crate) suppressions: BTreeSet<SemanticSymbol>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GeneratedValidationError {
    #[error("generated sketch admission: {0}")]
    Invalid(String),
    #[error(transparent)]
    Composition(#[from] CodeCompositionError),
}

fn invalid(message: impl Into<String>) -> GeneratedValidationError {
    GeneratedValidationError::Invalid(message.into())
}

impl GeneratedSketchArtifact {
    /// Decodes bounded generator data; admission does not execute JavaScript.
    ///
    /// # Errors
    /// Rejects malformed, oversized, unsupported or inconsistent evaluated data.
    pub fn from_json(json: &str) -> Result<ValidatedGeneratedSketch, GeneratedValidationError> {
        if json.len() > GENERATED_SKETCH_ARTIFACT_LIMIT {
            return Err(invalid("artifact byte limit exceeded"));
        }
        let artifact: Self =
            serde_json::from_str(json).map_err(|error| invalid(error.to_string()))?;
        artifact.validate()
    }

    /// Independently validates structural data and references. Native family,
    /// path/kind and geometry validation occurs during materialization.
    ///
    /// # Errors
    /// Rejects duplicate identities, invalid values, forward/dangling references,
    /// inconsistent ownership, or exceeded resource limits.
    #[allow(
        clippy::too_many_lines,
        reason = "one admission pass owns complete artifact cross-links"
    )]
    pub fn validate(self) -> Result<ValidatedGeneratedSketch, GeneratedValidationError> {
        if self.format != GENERATED_SKETCH_ARTIFACT_FORMAT || self.sdk_abi != SKETCH_CODE_SDK_ABI {
            return Err(invalid("unsupported format or SDK ABI"));
        }
        if [
            self.declarations.len(),
            self.applications.len(),
            self.parameters.len(),
            self.groups.len(),
            self.suppressions.len(),
        ]
        .into_iter()
        .any(|count| count > MAX_RECORDS)
        {
            return Err(invalid("record limit exceeded"));
        }
        let mut validator = ValueValidator::default();
        let mut declarations = Vec::new();
        for declaration in &self.declarations {
            validate_identity(&declaration.identity)?;
            if validator.identities.contains_key(&declaration.identity) {
                return Err(invalid(format!(
                    "duplicate declaration {:?}",
                    declaration.identity
                )));
            }
            let (namespace, method) = declaration
                .family
                .split_once('.')
                .filter(|(_, method)| !method.contains('.'))
                .ok_or_else(|| invalid("family must be namespace.method"))?;
            if crate::code_authoring_family(namespace, method).is_none()
                && !matches!(
                    declaration.family.as_str(),
                    "computed.polylineChannel"
                        | "computed.fillet"
                        | "computed.roundedRectangleProfile"
                )
            {
                return Err(invalid(format!("unknown family {}", declaration.family)));
            }
            validate_declaration_presentation(&declaration.arguments, namespace)?;
            let symbol = symbol(&declaration.identity)?;
            let arguments = validator.value(&declaration.arguments, 0)?;
            if !matches!(arguments, ManagedValue::Object(_)) {
                return Err(invalid("declaration arguments must be an object"));
            }
            declarations.push(AuthoringDeclaration {
                variable: symbol.0.clone(),
                symbol: symbol.clone(),
                builder_path: vec![namespace.into(), method.into()],
                arguments,
                patch: None,
                statement_span: ManagedSpan::new(0, 0),
                symbol_span: ManagedSpan::new(0, 0),
                arguments_span: ManagedSpan::new(0, 0),
            });
            validator
                .identities
                .insert(declaration.identity.clone(), symbol);
        }
        let mut applications = BTreeMap::new();
        for application in &self.applications {
            validate_identity(&application.identity)?;
            if validator.identities.contains_key(&application.identity)
                || applications.contains_key(&application.identity)
            {
                return Err(invalid("duplicate application identity"));
            }
            let mut seen = BTreeSet::new();
            for identity in &application.declarations {
                if !identity.starts_with(&application.identity)
                    || identity == &application.identity
                    || !seen.insert(identity)
                    || !validator.identities.contains_key(identity)
                {
                    return Err(invalid("application has invalid declaration ownership"));
                }
            }
            validate_presentation(&application.presentation, None)?;
            for presentation in application.input_presentation.values() {
                validate_presentation(presentation, Some(false))?;
            }
            validator.value(&application.inputs, 0)?;
            validator.value(&application.output, 0)?;
            applications.insert(
                application.identity.clone(),
                application.declarations.clone(),
            );
        }
        let mut parameters = BTreeSet::new();
        for parameter in &self.parameters {
            validate_identity(std::slice::from_ref(&parameter.id))?;
            if validator
                .identities
                .contains_key(&vec![parameter.id.clone()])
                || applications.contains_key(&vec![parameter.id.clone()])
                || !parameters.insert(&parameter.id)
            {
                return Err(invalid("duplicate parameter ID"));
            }
            validate_presentation(&parameter.presentation, Some(false))?;
            let value = validator.value(&parameter.value, 0)?;
            if !matches!(value, ManagedValue::Number(_) | ManagedValue::Unit(_)) {
                return Err(invalid("named parameter must be a finite number or unit"));
            }
        }
        let mut organizations = Vec::new();
        let mut group_names = BTreeSet::new();
        for group in &self.groups {
            validate_identity(std::slice::from_ref(&group.name))?;
            if !group_names.insert(&group.name) {
                return Err(invalid("duplicate group name"));
            }
            let members =
                expand_members(&group.declarations, &validator.identities, &applications)?;
            organizations.push(ManagedOrganization {
                name: group.name.clone(),
                declarations: members,
                span: ManagedSpan::new(0, 0),
            });
        }
        let suppressions =
            expand_members(&self.suppressions, &validator.identities, &applications)?
                .into_iter()
                .collect();
        validate_document(&self.document)?;
        let output = validator.value(&self.output, 0)?;
        let mut outputs = Vec::new();
        flatten_outputs(&output, String::new(), &mut outputs);
        let bytes = serde_json::to_vec(&self).map_err(|error| invalid(error.to_string()))?;
        if bytes.len() > GENERATED_SKETCH_ARTIFACT_LIMIT {
            return Err(invalid("artifact byte limit exceeded"));
        }
        Ok(ValidatedGeneratedSketch {
            artifact: self,
            digest: intent_content_digest(&bytes).to_string(),
            program: AuthoringProgram {
                scalar_bindings: Vec::new(),
                declarations,
                organizations,
                outputs,
            },
            references: validator.references,
            suppressions,
        })
    }
}

impl ValidatedGeneratedSketch {
    pub fn artifact(&self) -> &GeneratedSketchArtifact {
        &self.artifact
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Uses the same ordinary intent lowering, native operations, explicit computed
/// feature authoring and independent validation as managed source.
///
/// # Errors
/// Returns admission, reference, native solve, or computed-feature failures;
/// no caller-owned previously accepted result is changed.
pub fn materialize_generated_sketch_cold(
    sketch: &ValidatedGeneratedSketch,
    intent_session: IntentSessionId,
    document: DocumentId,
    model_scale: f64,
) -> Result<MaterializedCodeProject, GeneratedValidationError> {
    crate::composition::materialize_generated_cold(sketch, intent_session, document, model_scale)
        .map_err(Into::into)
}

fn symbol(identity: &[String]) -> Result<SemanticSymbol, GeneratedValidationError> {
    let bytes = serde_json::to_vec(identity).map_err(|error| invalid(error.to_string()))?;
    Ok(SemanticSymbol(format!(
        "generated.{}",
        intent_content_digest(&bytes)
    )))
}

fn validate_identity(identity: &[String]) -> Result<(), GeneratedValidationError> {
    if identity.is_empty()
        || identity.len() > MAX_VALUE_DEPTH
        || identity.iter().any(|part| {
            part.is_empty()
                || part.len() > 256
                || part.trim() != part
                || part.chars().any(char::is_control)
        })
    {
        return Err(invalid(
            "identity must contain bounded nonempty plain-text segments",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct ValueValidator {
    identities: BTreeMap<Vec<String>, SemanticSymbol>,
    references: Vec<(SemanticSymbol, SemanticOutputPath, FeatureKind)>,
    values: usize,
}

impl ValueValidator {
    fn value(
        &mut self,
        value: &GeneratedValue,
        depth: usize,
    ) -> Result<ManagedValue, GeneratedValidationError> {
        self.values += 1;
        if depth > MAX_VALUE_DEPTH || self.values > MAX_VALUES {
            return Err(invalid("value complexity limit exceeded"));
        }
        Ok(match value {
            GeneratedValue::Null => ManagedValue::Null,
            GeneratedValue::Bool(value) => ManagedValue::Bool(*value),
            GeneratedValue::String(value) => {
                if value.len() > 65_536 {
                    return Err(invalid("string limit exceeded"));
                }
                ManagedValue::String(value.clone())
            }
            GeneratedValue::Number(value) => {
                if !value.is_finite() {
                    return Err(invalid("nonfinite number"));
                }
                ManagedValue::Number(*value)
            }
            GeneratedValue::Unit(value) => {
                if !value.value.is_finite()
                    || !matches!(
                        value.unit.as_str(),
                        "mm" | "cm" | "m" | "inch" | "deg" | "rad"
                    )
                {
                    return Err(invalid("nonfinite or unknown unit"));
                }
                let angle = matches!(value.unit.as_str(), "deg" | "rad");
                let expected = if angle {
                    geosolve_sketch_intent::IntentUnit::Angle
                } else {
                    geosolve_sketch_intent::IntentUnit::Length
                };
                let converted =
                    crate::expansion::convert_named_unit(value, expected, "generated unit")
                        .map_err(|error| invalid(error.to_string()))?;
                ManagedValue::Unit(UnitLiteral {
                    unit: if angle { "rad" } else { "mm" }.into(),
                    value: converted,
                })
            }
            GeneratedValue::Array(values) => ManagedValue::Array(
                values
                    .iter()
                    .map(|value| self.value(value, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            GeneratedValue::Object(fields) => ManagedValue::Object(
                fields
                    .iter()
                    .map(|(key, value)| {
                        if key.len() > 256 || key.contains('\0') {
                            return Err(invalid("invalid object field"));
                        }
                        Ok((key.clone(), self.value(value, depth + 1)?))
                    })
                    .collect::<Result<_, _>>()?,
            ),
            GeneratedValue::Reference(reference) => {
                let declaration = self
                    .identities
                    .get(&reference.identity)
                    .ok_or_else(|| {
                        invalid(format!(
                            "unknown/forward reference {:?}",
                            reference.identity
                        ))
                    })?
                    .clone();
                if reference.path.len() > MAX_VALUE_DEPTH {
                    return Err(invalid("reference path limit exceeded"));
                }
                for part in &reference.path {
                    match part {
                        ManagedPathSegment::Field(value)
                        | ManagedPathSegment::Member { member: value }
                            if value.is_empty() || value.len() > 256 =>
                        {
                            return Err(invalid("invalid reference path"));
                        }
                        ManagedPathSegment::Index(index) if *index > MAX_RECORDS => {
                            return Err(invalid("reference index limit exceeded"));
                        }
                        _ => {}
                    }
                }
                let path = SemanticOutputPath(reference.path.clone());
                self.references
                    .push((declaration.clone(), path.clone(), reference.kind));
                ManagedValue::Reference { declaration, path }
            }
        })
    }
}

fn expand_members(
    members: &[Vec<String>],
    declarations: &BTreeMap<Vec<String>, SemanticSymbol>,
    applications: &BTreeMap<Vec<String>, Vec<Vec<String>>>,
) -> Result<Vec<SemanticSymbol>, GeneratedValidationError> {
    let mut result = BTreeSet::new();
    for identity in members {
        if let Some(symbol) = declarations.get(identity) {
            result.insert(symbol.clone());
        } else if let Some(members) = applications.get(identity) {
            result.extend(
                members
                    .iter()
                    .map(|identity| declarations[identity].clone()),
            );
        } else {
            return Err(invalid("unknown organization/suppression member"));
        }
    }
    Ok(result.into_iter().collect())
}

fn flatten_outputs(value: &ManagedValue, name: String, outputs: &mut Vec<ManagedOutput>) {
    match value {
        ManagedValue::Reference { declaration, path } => outputs.push(ManagedOutput {
            name: if name.is_empty() { "/".into() } else { name },
            declaration: declaration.clone(),
            path: path.clone(),
        }),
        ManagedValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                flatten_outputs(value, format!("{name}/{index}"), outputs);
            }
        }
        ManagedValue::Object(values) => {
            for (key, value) in values {
                let key = key.replace('~', "~0").replace('/', "~1");
                flatten_outputs(value, format!("{name}/{key}"), outputs);
            }
        }
        _ => {}
    }
}

fn validate_presentation(
    value: &ManagedPresentation,
    flag: Option<bool>,
) -> Result<(), GeneratedValidationError> {
    if value.is_key_constraint.is_some() && flag != Some(true)
        || value.is_key_parameter.is_some() && flag != Some(false)
    {
        return Err(invalid("presentation flag does not belong to this owner"));
    }
    if value.label.as_ref().is_some_and(|label| {
        label.len() > 256 || label.trim() != label || label.chars().any(char::is_control)
    }) {
        return Err(invalid("invalid presentation label"));
    }
    validate_description(value.description.as_deref())
}

fn validate_description(description: Option<&str>) -> Result<(), GeneratedValidationError> {
    if description.is_some_and(|text| text.chars().count() > 2048 || text.contains('\0')) {
        return Err(invalid("invalid description"));
    }
    Ok(())
}

fn validate_document(
    document: &ManagedDocumentPresentation,
) -> Result<(), GeneratedValidationError> {
    if document
        .title
        .as_ref()
        .is_some_and(|title| title.chars().count() > 128 || title.chars().any(char::is_control))
    {
        return Err(invalid("invalid document title"));
    }
    validate_description(document.description.as_deref())
}

fn validate_declaration_presentation(
    arguments: &GeneratedValue,
    namespace: &str,
) -> Result<(), GeneratedValidationError> {
    let GeneratedValue::Object(fields) = arguments else {
        return Err(invalid("declaration arguments must be an object"));
    };
    let mut presentation = ManagedPresentation::default();
    for (name, value) in fields {
        match (name.as_str(), value) {
            ("label", GeneratedValue::String(value)) => presentation.label = Some(value.clone()),
            ("description", GeneratedValue::String(value)) => {
                presentation.description = Some(value.clone());
            }
            ("isKeyConstraint", GeneratedValue::Bool(value)) if namespace == "dimension" => {
                presentation.is_key_constraint = Some(*value);
            }
            ("label" | "description" | "isKeyConstraint" | "isKeyParameter", _) => {
                return Err(invalid("invalid declaration presentation"));
            }
            _ => {}
        }
    }
    validate_presentation(&presentation, (namespace == "dimension").then_some(true))
}
