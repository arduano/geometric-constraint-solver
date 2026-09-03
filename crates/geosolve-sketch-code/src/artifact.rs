// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::intent_content_digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    FeatureKind, ManagedPathSegment, ManagedValue, PATCH_ARTIFACT_LIMIT, SKETCH_CODE_SDK_ABI,
    SemanticOutputPath, code_authoring_family,
};

pub const PATCH_ARTIFACT_FORMAT: &str = "geosolve-patch-artifact-v2";
const MAX_ARTIFACT_ITEMS: usize = 65_536;
const MAX_ARTIFACT_DEPTH: usize = 64;
const MAX_ARTIFACT_KEY_BYTES: usize = 256;

/// One equation-free declaration template emitted by the caller-owned build.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchTemplateNode {
    /// Mandatory patch-local identity, independent of where an ordinary
    /// callback return exposes one selected result.
    pub path: Vec<String>,
    /// Exact ordinary callback return path. With `result_output`, this exposes
    /// one selected semantic result; without it, this exposes the complete
    /// authenticated result tree. `None` keeps the declaration private or
    /// collection-owned.
    pub result_path: Option<Vec<String>>,
    /// Exact semantic output exposed at `result_path`. `None` means the whole
    /// authenticated result tree when `result_path` is present.
    pub result_output: Option<SemanticOutputPath>,
    pub declaration_family: String,
    /// Complete named declaration arguments. Bindings may occur at any leaf;
    /// keeping them in the data tree preserves nested typed API shapes such as
    /// tangent sources, contact state, Fillet parents and aggregate arrays.
    pub arguments: TemplateArgument,
    /// Rust-catalog-derived typed result inventory. Paths are recursive
    /// semantic coordinates, never flattened transport keys or caller
    /// manifests. Dynamic keyed collection nodes and their concrete typed
    /// members are both first-class results.
    pub outputs: Vec<PatchTemplateOutput>,
}

/// One exact typed node in a patch declaration result tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchTemplateOutput {
    pub path: SemanticOutputPath,
    pub kind: FeatureKind,
}

/// One recursively typed custom-patch declaration argument.
///
/// Literal values and dependency bindings share one lossless tree. This is a
/// transport contract only: Rust still authenticates the declaration family
/// and lowers the reconstructed named arguments through the ordinary clean
/// authoring catalog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "argument",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TemplateArgument {
    Literal(ManagedValue),
    Binding(TemplateBinding),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

/// Data-only source for a template input. There is no raw intent identity or
/// executable callback variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum TemplateBinding {
    Input {
        name: String,
        path: SemanticOutputPath,
        expected_kind: FeatureKind,
    },
    TemplateOutput {
        template: Vec<String>,
        path: SemanticOutputPath,
        expected_kind: FeatureKind,
    },
    CollectionMember {
        input: String,
        path: SemanticOutputPath,
        expected_kind: FeatureKind,
    },
}

/// Structural collection expansion recorded by `p.each` or `p.mapRecord`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectionRule {
    Each {
        path: Vec<String>,
        input: String,
        member_key_field: String,
        templates: Vec<Vec<String>>,
    },
    MapRecord {
        path: Vec<String>,
        input: String,
        templates: Vec<Vec<String>>,
    },
}

/// Canonical, data-only product of the explicit caller-owned Node build step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchModuleArtifact {
    pub format: String,
    pub sdk_abi: String,
    pub module_specifier: String,
    pub export_name: String,
    pub source_digest: String,
    pub interface_digest: String,
    pub inputs: BTreeMap<String, FeatureKind>,
    pub outputs: BTreeMap<String, FeatureKind>,
    pub templates: Vec<PatchTemplateNode>,
    pub collections: Vec<CollectionRule>,
}

/// Fully bounded and structurally authenticated artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedPatchModuleArtifact {
    artifact: PatchModuleArtifact,
    digest: String,
    canonical_json: String,
}

impl ValidatedPatchModuleArtifact {
    #[must_use]
    pub const fn artifact(&self) -> &PatchModuleArtifact {
        &self.artifact
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    #[must_use]
    pub fn into_artifact(self) -> PatchModuleArtifact {
        self.artifact
    }
}

/// Rejected untrusted patch artifact.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ArtifactValidationError {
    #[error("patch artifact is {actual} bytes; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("patch artifact contains {actual} structural items; the limit is {limit}")]
    StructuralItemLimit { actual: usize, limit: usize },
    #[error("patch artifact JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("patch artifact JSON is not in canonical byte form")]
    NonCanonicalJson,
    #[error("unsupported patch artifact format `{0}`")]
    UnsupportedFormat(String),
    #[error("patch artifact SDK ABI `{0}` does not match this build")]
    UnsupportedSdkAbi(String),
    #[error("invalid {field} digest `{value}`")]
    InvalidDigest { field: &'static str, value: String },
    #[error(
        "patch artifact interface digest is {actual}, but the canonical interface is {expected}"
    )]
    InterfaceDigestMismatch { expected: String, actual: String },
    #[error("invalid custom patch module specifier `{0}`")]
    InvalidModuleSpecifier(String),
    #[error("invalid artifact key `{0}`")]
    InvalidKey(String),
    #[error("duplicate artifact path `{0}`")]
    DuplicatePath(String),
    #[error("unsupported declaration family `{0}`")]
    UnsupportedDeclarationFamily(String),
    #[error("artifact declaration family `{family}` has an invalid schema: {message}")]
    InvalidFamilySchema { family: String, message: String },
    #[error("artifact reference is invalid: {0}")]
    InvalidReference(String),
    #[error("artifact kind mismatch: {0}")]
    KindMismatch(String),
    #[error("artifact collection rule is invalid: {0}")]
    InvalidCollection(String),
    #[error("artifact literal exceeds the maximum nesting depth")]
    LiteralDepth,
}

impl PatchModuleArtifact {
    /// Validates a Rust value and derives its canonical bytes and content
    /// digest. Custom source is never executed by this operation.
    ///
    /// # Errors
    ///
    /// Returns a typed schema, reference, ABI, digest or resource error when
    /// the untrusted data-only artifact is not admissible.
    pub fn validate(self) -> Result<ValidatedPatchModuleArtifact, ArtifactValidationError> {
        validate_artifact(&self)?;
        let canonical_json = serde_json::to_string(&self)
            .map_err(|error| ArtifactValidationError::InvalidJson(error.to_string()))?;
        if canonical_json.len() > PATCH_ARTIFACT_LIMIT {
            return Err(ArtifactValidationError::ResourceLimit {
                actual: canonical_json.len(),
                limit: PATCH_ARTIFACT_LIMIT,
            });
        }
        let digest = intent_content_digest(canonical_json.as_bytes()).to_string();
        Ok(ValidatedPatchModuleArtifact {
            artifact: self,
            digest,
            canonical_json,
        })
    }

    /// Admits only exact canonical JSON bytes. Whitespace, duplicate/unknown
    /// fields and alternate encodings do not silently acquire authority.
    ///
    /// # Errors
    ///
    /// Returns a typed decoding, canonicality, validation or resource error.
    pub fn from_canonical_json(
        json: &str,
    ) -> Result<ValidatedPatchModuleArtifact, ArtifactValidationError> {
        if json.len() > PATCH_ARTIFACT_LIMIT {
            return Err(ArtifactValidationError::ResourceLimit {
                actual: json.len(),
                limit: PATCH_ARTIFACT_LIMIT,
            });
        }
        let artifact: Self = serde_json::from_str(json)
            .map_err(|error| ArtifactValidationError::InvalidJson(error.to_string()))?;
        let validated = artifact.validate()?;
        if validated.canonical_json != json {
            return Err(ArtifactValidationError::NonCanonicalJson);
        }
        Ok(validated)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one audit pass keeps cross-artifact schema invariants together"
)]
fn validate_artifact(artifact: &PatchModuleArtifact) -> Result<(), ArtifactValidationError> {
    validate_structural_item_bound(artifact)?;
    if artifact.format != PATCH_ARTIFACT_FORMAT {
        return Err(ArtifactValidationError::UnsupportedFormat(
            artifact.format.clone(),
        ));
    }
    if artifact.sdk_abi != SKETCH_CODE_SDK_ABI {
        return Err(ArtifactValidationError::UnsupportedSdkAbi(
            artifact.sdk_abi.clone(),
        ));
    }
    validate_digest("source", &artifact.source_digest)?;
    validate_digest("interface", &artifact.interface_digest)?;
    if !artifact.module_specifier.starts_with("./patches/")
        || !artifact.module_specifier.ends_with(".patch.ts")
        || artifact.module_specifier.contains("..")
        || artifact.module_specifier.contains('\\')
    {
        return Err(ArtifactValidationError::InvalidModuleSpecifier(
            artifact.module_specifier.clone(),
        ));
    }
    validate_key(&artifact.export_name)?;
    for key in artifact.inputs.keys().chain(artifact.outputs.keys()) {
        validate_key(key)?;
    }
    let expected_interface_digest = canonical_interface_digest(artifact)?;
    if artifact.interface_digest != expected_interface_digest {
        return Err(ArtifactValidationError::InterfaceDigestMismatch {
            expected: expected_interface_digest,
            actual: artifact.interface_digest.clone(),
        });
    }

    let mut template_paths = BTreeMap::<Vec<String>, &PatchTemplateNode>::new();
    for template in &artifact.templates {
        validate_path(&template.path)?;
        if let Some(result_path) = &template.result_path {
            validate_path(result_path)?;
        }
        if template_paths
            .insert(template.path.clone(), template)
            .is_some()
        {
            return Err(ArtifactValidationError::DuplicatePath(path_text(
                &template.path,
            )));
        }
        if !supported_declaration_family(&template.declaration_family) {
            return Err(ArtifactValidationError::UnsupportedDeclarationFamily(
                template.declaration_family.clone(),
            ));
        }
        let mut output_paths = BTreeSet::new();
        for output in &template.outputs {
            validate_semantic_path(&output.path, false)?;
            if !output_paths.insert(output.path.clone()) {
                return Err(ArtifactValidationError::DuplicatePath(format!(
                    "{}/{}",
                    path_text(&template.path),
                    semantic_path_text(&output.path)
                )));
            }
        }
        if let Some(output) = &template.result_output {
            validate_semantic_path(output, false)?;
            if !output_paths.contains(output) {
                return Err(ArtifactValidationError::InvalidReference(format!(
                    "template `{}` exposes unknown result output `{}`",
                    path_text(&template.path),
                    semantic_path_text(output)
                )));
            }
        }
        validate_template_argument(&template.arguments, 0)?;
        validate_family_schema(template)?;
    }

    let available = template_paths
        .iter()
        .map(|(path, template)| {
            (
                path.clone(),
                template
                    .outputs
                    .iter()
                    .map(|output| (output.path.clone(), output.kind))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for template in &artifact.templates {
        for binding in template_bindings(&template.arguments) {
            validate_binding(binding, artifact, &available)?;
        }
    }
    validate_template_dag(&artifact.templates)?;

    let mut collection_paths = BTreeSet::new();
    for collection in &artifact.collections {
        let (path, input, templates) = match collection {
            CollectionRule::Each {
                path,
                input,
                member_key_field,
                templates,
            } => {
                validate_key(member_key_field)?;
                (path, input, templates)
            }
            CollectionRule::MapRecord {
                path,
                input,
                templates,
            } => (path, input, templates),
        };
        validate_path(path)?;
        if !collection_paths.insert(path.clone()) {
            return Err(ArtifactValidationError::DuplicatePath(path_text(path)));
        }
        if artifact.inputs.get(input) != Some(&FeatureKind::Collection) {
            return Err(ArtifactValidationError::InvalidCollection(format!(
                "`{input}` is not a declared collection input"
            )));
        }
        if templates.is_empty() {
            return Err(ArtifactValidationError::InvalidCollection(format!(
                "collection `{}` has no template",
                path_text(path)
            )));
        }
        for template in templates {
            if !template_paths.contains_key(template) {
                return Err(ArtifactValidationError::InvalidCollection(format!(
                    "collection `{}` references unknown template `{}`",
                    path_text(path),
                    path_text(template)
                )));
            }
        }
    }

    Ok(())
}

fn validate_structural_item_bound(
    artifact: &PatchModuleArtifact,
) -> Result<(), ArtifactValidationError> {
    let mut count = 0;
    add_structural_items(&mut count, artifact.inputs.len())?;
    add_structural_items(&mut count, artifact.outputs.len())?;
    add_structural_items(&mut count, artifact.templates.len())?;
    add_structural_items(&mut count, artifact.collections.len())?;

    for template in &artifact.templates {
        add_structural_items(&mut count, template.path.len())?;
        add_structural_items(
            &mut count,
            template.result_path.as_ref().map_or(0, Vec::len),
        )?;
        add_structural_items(
            &mut count,
            template
                .result_output
                .as_ref()
                .map_or(0, |path| path.0.len()),
        )?;
        add_structural_items(&mut count, template.outputs.len())?;
        for output in &template.outputs {
            add_structural_items(&mut count, output.path.0.len())?;
        }
        count_template_argument_items(&template.arguments, 0, &mut count)?;
    }
    for collection in &artifact.collections {
        let (path, templates) = match collection {
            CollectionRule::Each {
                path, templates, ..
            }
            | CollectionRule::MapRecord {
                path, templates, ..
            } => (path, templates),
        };
        add_structural_items(&mut count, path.len())?;
        add_structural_items(&mut count, templates.len())?;
        for template in templates {
            add_structural_items(&mut count, template.len())?;
        }
    }
    Ok(())
}

fn count_template_argument_items(
    argument: &TemplateArgument,
    depth: usize,
    count: &mut usize,
) -> Result<(), ArtifactValidationError> {
    if depth > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::LiteralDepth);
    }
    add_structural_items(count, 1)?;
    match argument {
        TemplateArgument::Literal(value) => count_managed_value_items(value, depth + 1, count),
        TemplateArgument::Binding(binding) => {
            let path_len = match binding {
                TemplateBinding::Input { path, .. }
                | TemplateBinding::CollectionMember { path, .. } => path.0.len(),
                TemplateBinding::TemplateOutput { template, path, .. } => {
                    template.len().saturating_add(path.0.len())
                }
            };
            add_structural_items(count, path_len)
        }
        TemplateArgument::Array(values) => {
            add_structural_items(count, values.len())?;
            for value in values {
                count_template_argument_items(value, depth + 1, count)?;
            }
            Ok(())
        }
        TemplateArgument::Object(values) => {
            add_structural_items(count, values.len())?;
            for value in values.values() {
                count_template_argument_items(value, depth + 1, count)?;
            }
            Ok(())
        }
    }
}

fn count_managed_value_items(
    value: &ManagedValue,
    depth: usize,
    count: &mut usize,
) -> Result<(), ArtifactValidationError> {
    if depth > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::LiteralDepth);
    }
    add_structural_items(count, 1)?;
    match value {
        ManagedValue::Array(values) => {
            add_structural_items(count, values.len())?;
            for value in values {
                count_managed_value_items(value, depth + 1, count)?;
            }
        }
        ManagedValue::Object(values) => {
            add_structural_items(count, values.len())?;
            for value in values.values() {
                count_managed_value_items(value, depth + 1, count)?;
            }
        }
        ManagedValue::Reference { path, .. } => {
            add_structural_items(count, path.0.len())?;
        }
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_) => {}
    }
    Ok(())
}

fn add_structural_items(
    count: &mut usize,
    additional: usize,
) -> Result<(), ArtifactValidationError> {
    *count = count.saturating_add(additional);
    if *count > MAX_ARTIFACT_ITEMS {
        return Err(ArtifactValidationError::StructuralItemLimit {
            actual: *count,
            limit: MAX_ARTIFACT_ITEMS,
        });
    }
    Ok(())
}

fn validate_binding(
    binding: &TemplateBinding,
    artifact: &PatchModuleArtifact,
    available: &BTreeMap<Vec<String>, BTreeMap<SemanticOutputPath, FeatureKind>>,
) -> Result<(), ArtifactValidationError> {
    match binding {
        TemplateBinding::Input {
            name,
            path,
            expected_kind,
        }
        | TemplateBinding::CollectionMember {
            input: name,
            path,
            expected_kind,
        } => {
            validate_semantic_path(path, true)?;
            let Some(actual) = artifact.inputs.get(name) else {
                return Err(ArtifactValidationError::InvalidReference(format!(
                    "unknown artifact input `{name}`"
                )));
            };
            if matches!(binding, TemplateBinding::Input { .. })
                && path.0.is_empty()
                && actual != expected_kind
            {
                return Err(ArtifactValidationError::KindMismatch(format!(
                    "input `{name}` is {actual:?}, not {expected_kind:?}"
                )));
            }
            if matches!(binding, TemplateBinding::CollectionMember { .. })
                && actual != &FeatureKind::Collection
            {
                return Err(ArtifactValidationError::KindMismatch(format!(
                    "collection member source `{name}` is not a collection"
                )));
            }
        }
        TemplateBinding::TemplateOutput {
            template,
            path,
            expected_kind,
        } => {
            let Some(outputs) = available.get(template) else {
                return Err(ArtifactValidationError::InvalidReference(format!(
                    "unknown template `{}`",
                    path_text(template)
                )));
            };
            if path.0.is_empty() {
                if *expected_kind != FeatureKind::Feature {
                    return Err(ArtifactValidationError::KindMismatch(format!(
                        "whole template `{}` is a feature, not {expected_kind:?}",
                        path_text(template)
                    )));
                }
                return Ok(());
            }
            validate_semantic_path(path, false)?;
            let Some(actual) = outputs.get(path) else {
                return Err(ArtifactValidationError::InvalidReference(format!(
                    "template `{}` has no output `{}`",
                    path_text(template),
                    semantic_path_text(path)
                )));
            };
            if actual != expected_kind {
                return Err(ArtifactValidationError::KindMismatch(format!(
                    "template output `{}`.`{}` is {actual:?}, not {expected_kind:?}",
                    path_text(template),
                    semantic_path_text(path)
                )));
            }
        }
    }
    Ok(())
}

fn validate_template_dag(templates: &[PatchTemplateNode]) -> Result<(), ArtifactValidationError> {
    let dependencies = templates
        .iter()
        .map(|template| {
            let dependencies = template_bindings(&template.arguments)
                .into_iter()
                .filter_map(|binding| match binding {
                    TemplateBinding::TemplateOutput { template, .. } => Some(template.clone()),
                    TemplateBinding::Input { .. } | TemplateBinding::CollectionMember { .. } => {
                        None
                    }
                })
                .collect::<BTreeSet<_>>();
            (template.path.clone(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();
    let mut remaining = dependencies.clone();
    let mut admitted = BTreeSet::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, required)| required.iter().all(|path| admitted.contains(path)))
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            let paths = remaining
                .keys()
                .map(|path| path_text(path))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(ArtifactValidationError::InvalidReference(format!(
                "template dependency cycle among {paths}"
            )));
        }
        for path in ready {
            remaining.remove(&path);
            admitted.insert(path);
        }
    }
    Ok(())
}

fn template_bindings(argument: &TemplateArgument) -> Vec<&TemplateBinding> {
    let mut bindings = Vec::new();
    collect_template_bindings(argument, &mut bindings);
    bindings
}

pub(crate) fn template_argument_bindings_with_paths(
    argument: &TemplateArgument,
) -> Vec<(SemanticOutputPath, &TemplateBinding)> {
    let mut bindings = Vec::new();
    collect_template_argument_bindings_with_paths(argument, &mut Vec::new(), &mut bindings);
    bindings
}

fn collect_template_argument_bindings_with_paths<'a>(
    argument: &'a TemplateArgument,
    path: &mut Vec<ManagedPathSegment>,
    bindings: &mut Vec<(SemanticOutputPath, &'a TemplateBinding)>,
) {
    match argument {
        TemplateArgument::Binding(binding) => {
            bindings.push((SemanticOutputPath(path.clone()), binding));
        }
        TemplateArgument::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                path.push(ManagedPathSegment::Index(index));
                collect_template_argument_bindings_with_paths(value, path, bindings);
                path.pop();
            }
        }
        TemplateArgument::Object(values) => {
            for (name, value) in values {
                path.push(ManagedPathSegment::Field(name.clone()));
                collect_template_argument_bindings_with_paths(value, path, bindings);
                path.pop();
            }
        }
        TemplateArgument::Literal(_) => {}
    }
}

fn collect_template_bindings<'a>(
    argument: &'a TemplateArgument,
    bindings: &mut Vec<&'a TemplateBinding>,
) {
    match argument {
        TemplateArgument::Binding(binding) => bindings.push(binding),
        TemplateArgument::Array(values) => {
            for value in values {
                collect_template_bindings(value, bindings);
            }
        }
        TemplateArgument::Object(values) => {
            for value in values.values() {
                collect_template_bindings(value, bindings);
            }
        }
        TemplateArgument::Literal(_) => {}
    }
}

fn validate_template_argument(
    argument: &TemplateArgument,
    depth: usize,
) -> Result<(), ArtifactValidationError> {
    if depth > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::LiteralDepth);
    }
    match argument {
        TemplateArgument::Literal(value) => validate_managed_value(value, depth + 1),
        TemplateArgument::Binding(_) => Ok(()),
        TemplateArgument::Array(values) => {
            for value in values {
                validate_template_argument(value, depth + 1)?;
            }
            Ok(())
        }
        TemplateArgument::Object(values) => {
            for (key, value) in values {
                validate_key(key)?;
                validate_template_argument(value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn validate_managed_value(
    value: &ManagedValue,
    depth: usize,
) -> Result<(), ArtifactValidationError> {
    if depth > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::LiteralDepth);
    }
    match value {
        ManagedValue::Number(value) if !value.is_finite() => Err(
            ArtifactValidationError::InvalidReference("non-finite template literal".into()),
        ),
        ManagedValue::Unit(value) if !value.value.is_finite() => Err(
            ArtifactValidationError::InvalidReference("non-finite unit literal".into()),
        ),
        ManagedValue::Array(values) => {
            for value in values {
                validate_managed_value(value, depth + 1)?;
            }
            Ok(())
        }
        ManagedValue::Object(values) => {
            for (key, value) in values {
                validate_key(key)?;
                validate_managed_value(value, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), ArtifactValidationError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ArtifactValidationError::InvalidDigest {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_path(path: &[String]) -> Result<(), ArtifactValidationError> {
    if path.is_empty() || path.len() > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::InvalidKey(path_text(path)));
    }
    for key in path {
        validate_key(key)?;
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), ArtifactValidationError> {
    if key.is_empty()
        || key.len() > MAX_ARTIFACT_KEY_BYTES
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ArtifactValidationError::InvalidKey(key.to_owned()));
    }
    Ok(())
}

fn path_text(path: &[String]) -> String {
    path.join("/")
}

fn semantic_path_text(path: &SemanticOutputPath) -> String {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => format!("[{index}]"),
            ManagedPathSegment::Member { member } => format!("[{member}]"),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn supported_declaration_family(value: &str) -> bool {
    // These two families are deliberately patch-private host-authored
    // composites. Unlike ordinary clean declarations, neither pretends to be
    // one standalone Intent node. Expansion authenticates them through the
    // existing native Fillet/rounded-profile authoring routes.
    if matches!(
        value,
        "computed.fillet" | "computed.roundedRectangleProfile"
    ) {
        return true;
    }
    let Some((namespace, method)) = value.split_once('.') else {
        return false;
    };
    code_authoring_family(namespace, method).is_some()
}

#[derive(Serialize)]
struct CanonicalArtifactInterface<'a> {
    // Field order is the UTF-8 lexical order used by TypeScript's
    // `canonicalStringify` helper. BTreeMap applies the same order to schemas.
    export_name: &'a str,
    inputs: &'a BTreeMap<String, FeatureKind>,
    module_specifier: &'a str,
    outputs: &'a BTreeMap<String, FeatureKind>,
    sdk_abi: &'a str,
}

fn canonical_interface_digest(
    artifact: &PatchModuleArtifact,
) -> Result<String, ArtifactValidationError> {
    let canonical = serde_json::to_vec(&CanonicalArtifactInterface {
        export_name: &artifact.export_name,
        inputs: &artifact.inputs,
        module_specifier: &artifact.module_specifier,
        outputs: &artifact.outputs,
        sdk_abi: &artifact.sdk_abi,
    })
    .map_err(|error| ArtifactValidationError::InvalidJson(error.to_string()))?;
    Ok(intent_content_digest(&canonical).to_string())
}

fn validate_family_schema(template: &PatchTemplateNode) -> Result<(), ArtifactValidationError> {
    if !matches!(template.arguments, TemplateArgument::Object(_)) {
        return family_schema_error(template, "named declaration arguments must be an object");
    }
    if template.outputs.is_empty() {
        return family_schema_error(template, "declaration result shape cannot be empty");
    }
    Ok(())
}

fn family_schema_error<T>(
    template: &PatchTemplateNode,
    message: impl Into<String>,
) -> Result<T, ArtifactValidationError> {
    Err(ArtifactValidationError::InvalidFamilySchema {
        family: template.declaration_family.clone(),
        message: message.into(),
    })
}

fn validate_semantic_path(
    path: &SemanticOutputPath,
    allow_empty: bool,
) -> Result<(), ArtifactValidationError> {
    if (!allow_empty && path.0.is_empty()) || path.0.len() > MAX_ARTIFACT_DEPTH {
        return Err(ArtifactValidationError::InvalidReference(
            "semantic path has invalid depth".into(),
        ));
    }
    for segment in &path.0 {
        match segment {
            crate::ManagedPathSegment::Field(key)
            | crate::ManagedPathSegment::Member { member: key } => validate_key(key)?,
            crate::ManagedPathSegment::Index(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str, kind: FeatureKind) -> PatchTemplateOutput {
        PatchTemplateOutput {
            path: SemanticOutputPath(vec![ManagedPathSegment::Field(name.into())]),
            kind,
        }
    }

    fn output_path(name: &str) -> SemanticOutputPath {
        SemanticOutputPath(vec![ManagedPathSegment::Field(name.into())])
    }

    fn artifact() -> PatchModuleArtifact {
        let mut artifact = PatchModuleArtifact {
            format: PATCH_ARTIFACT_FORMAT.into(),
            sdk_abi: SKETCH_CODE_SDK_ABI.into(),
            module_specifier: "./patches/round.patch.ts".into(),
            export_name: "roundEveryCorner".into(),
            source_digest: "1".repeat(64),
            interface_digest: String::new(),
            inputs: BTreeMap::from([
                ("corners".into(), FeatureKind::Collection),
                ("radius".into(), FeatureKind::Scalar),
            ]),
            outputs: BTreeMap::from([("fillets".into(), FeatureKind::Collection)]),
            templates: vec![PatchTemplateNode {
                path: vec!["fillet".into()],
                result_path: None,
                result_output: Some(output_path("arc")),
                declaration_family: "computed.fillet".into(),
                arguments: TemplateArgument::Object(BTreeMap::from([
                    (
                        "corner".into(),
                        TemplateArgument::Binding(TemplateBinding::CollectionMember {
                            input: "corners".into(),
                            path: SemanticOutputPath::default(),
                            expected_kind: FeatureKind::FeatureCorner,
                        }),
                    ),
                    (
                        "radius".into(),
                        TemplateArgument::Binding(TemplateBinding::Input {
                            name: "radius".into(),
                            path: SemanticOutputPath::default(),
                            expected_kind: FeatureKind::Scalar,
                        }),
                    ),
                ])),
                outputs: vec![output("arc", FeatureKind::CurveSpan)],
            }],
            collections: vec![CollectionRule::Each {
                path: vec!["fillets".into()],
                input: "corners".into(),
                member_key_field: "key".into(),
                templates: vec![vec!["fillet".into()]],
            }],
        };
        artifact.interface_digest = canonical_interface_digest(&artifact).unwrap();
        artifact
    }

    #[test]
    fn validates_canonical_data_only_artifact_and_digest() {
        let validated = artifact().validate().unwrap();
        assert_eq!(validated.digest().len(), 64);
        assert_eq!(
            PatchModuleArtifact::from_canonical_json(validated.canonical_json())
                .unwrap()
                .digest(),
            validated.digest()
        );
        assert_eq!(
            PatchModuleArtifact::from_canonical_json(&format!(" {}", validated.canonical_json()))
                .unwrap_err(),
            ArtifactValidationError::NonCanonicalJson
        );
    }

    #[test]
    fn rejects_tampering_unknown_families_and_raw_id_fields() {
        let mut wrong = artifact();
        wrong.source_digest = "XYZ".into();
        assert!(matches!(
            wrong.validate(),
            Err(ArtifactValidationError::InvalidDigest {
                field: "source",
                ..
            })
        ));
        let mut unknown = artifact();
        unknown.templates[0].declaration_family = "solver.custom_residual".into();
        assert!(matches!(
            unknown.validate(),
            Err(ArtifactValidationError::UnsupportedDeclarationFamily(_))
        ));

        let mut forged_interface = artifact();
        forged_interface.interface_digest = "2".repeat(64);
        assert!(matches!(
            forged_interface.validate(),
            Err(ArtifactValidationError::InterfaceDigestMismatch { .. })
        ));

        let mut wrong_schema = artifact();
        wrong_schema.templates[0].arguments = TemplateArgument::Array(Vec::new());
        assert!(matches!(
            wrong_schema.validate(),
            Err(ArtifactValidationError::InvalidFamilySchema { .. })
        ));

        let mut unknown_result = artifact();
        unknown_result.templates[0].result_output = Some(output_path("alphabeticAccident"));
        assert!(matches!(
            unknown_result.validate(),
            Err(ArtifactValidationError::InvalidReference(_))
        ));

        let canonical = artifact().validate().unwrap().canonical_json().to_owned();
        let with_raw_id = canonical.replacen(
            "\"export_name\":\"roundEveryCorner\"",
            "\"export_name\":\"roundEveryCorner\",\"node_id\":\"0001\"",
            1,
        );
        assert!(matches!(
            PatchModuleArtifact::from_canonical_json(&with_raw_id),
            Err(ArtifactValidationError::InvalidJson(_))
        ));
    }

    #[test]
    fn radius_dimension_template_requires_exact_curve_target_and_dimension_schema() {
        let mut template = PatchTemplateNode {
            path: vec!["radius".into()],
            result_path: None,
            result_output: None,
            declaration_family: "dimension.radius".into(),
            arguments: TemplateArgument::Object(BTreeMap::from([
                (
                    "curve".into(),
                    TemplateArgument::Binding(TemplateBinding::TemplateOutput {
                        template: vec!["circle".into()],
                        path: output_path("circle"),
                        expected_kind: FeatureKind::Curve,
                    }),
                ),
                (
                    "target".into(),
                    TemplateArgument::Binding(TemplateBinding::Input {
                        name: "radius".into(),
                        path: SemanticOutputPath::default(),
                        expected_kind: FeatureKind::Scalar,
                    }),
                ),
            ])),
            outputs: vec![output("dimension", FeatureKind::Dimension)],
        };

        assert!(supported_declaration_family("dimension.radius"));
        validate_family_schema(&template).unwrap();

        let TemplateArgument::Object(arguments) = &mut template.arguments else {
            unreachable!("fixture arguments are an object")
        };
        arguments.insert(
            "target".into(),
            TemplateArgument::Binding(TemplateBinding::Input {
                name: "radius".into(),
                path: SemanticOutputPath::default(),
                expected_kind: FeatureKind::Point,
            }),
        );
        // Artifact admission authenticates structure and family identity.
        // Exact argument names and kinds are checked once references have
        // been resolved by catalog-driven lowering.
        validate_family_schema(&template).unwrap();
    }

    #[test]
    fn rejects_oversized_input_before_json_decode() {
        let oversized = "x".repeat(PATCH_ARTIFACT_LIMIT + 1);
        assert_eq!(
            PatchModuleArtifact::from_canonical_json(&oversized).unwrap_err(),
            ArtifactValidationError::ResourceLimit {
                actual: PATCH_ARTIFACT_LIMIT + 1,
                limit: PATCH_ARTIFACT_LIMIT,
            }
        );
    }

    #[test]
    fn rejects_excessive_nested_artifact_items_before_expansion() {
        let mut excessive = artifact();
        let CollectionRule::Each { templates, .. } = &mut excessive.collections[0] else {
            panic!("fixture collection is an each rule")
        };
        *templates = vec![Vec::new(); MAX_ARTIFACT_ITEMS];
        assert!(matches!(
            excessive.validate(),
            Err(ArtifactValidationError::StructuralItemLimit {
                actual,
                limit: MAX_ARTIFACT_ITEMS,
            }) if actual > MAX_ARTIFACT_ITEMS
        ));
    }

    #[test]
    fn template_dag_is_order_independent_but_rejects_cycles() {
        let mut valid = artifact().templates;
        let TemplateArgument::Object(arguments) = &mut valid[0].arguments else {
            unreachable!("fixture arguments are an object")
        };
        arguments.insert(
            "future".into(),
            TemplateArgument::Binding(TemplateBinding::TemplateOutput {
                template: vec!["later".into()],
                path: output_path("point"),
                expected_kind: FeatureKind::Point,
            }),
        );
        valid.push(PatchTemplateNode {
            path: vec!["later".into()],
            result_path: Some(vec!["point".into()]),
            result_output: Some(output_path("point")),
            declaration_family: "geometry.sketchPoint".into(),
            arguments: TemplateArgument::Object(BTreeMap::new()),
            outputs: vec![output("point", FeatureKind::Point)],
        });
        validate_template_dag(&valid).unwrap();

        let mut invalid = valid;
        let TemplateArgument::Object(arguments) = &mut invalid[1].arguments else {
            unreachable!("fixture arguments are an object")
        };
        arguments.insert(
            "back".into(),
            TemplateArgument::Binding(TemplateBinding::TemplateOutput {
                template: vec!["fillet".into()],
                path: output_path("arc"),
                expected_kind: FeatureKind::CurveSpan,
            }),
        );
        assert!(matches!(
            validate_template_dag(&invalid),
            Err(ArtifactValidationError::InvalidReference(_))
        ));
    }

    #[test]
    fn canonical_interface_digest_authenticates_export_and_top_level_schemas() {
        let original = artifact();
        let digest = original.interface_digest.clone();
        assert_eq!(digest, canonical_interface_digest(&original).unwrap());

        let mut renamed = original.clone();
        renamed.export_name = "roundDifferentCorners".into();
        assert_ne!(canonical_interface_digest(&renamed).unwrap(), digest);
        assert!(matches!(
            renamed.validate(),
            Err(ArtifactValidationError::InterfaceDigestMismatch { .. })
        ));

        let mut widened = original;
        widened
            .outputs
            .insert("unsafeExtra".into(), FeatureKind::Curve);
        assert_ne!(canonical_interface_digest(&widened).unwrap(), digest);
        assert!(matches!(
            widened.validate(),
            Err(ArtifactValidationError::InterfaceDigestMismatch { .. })
        ));
    }

    #[test]
    fn admits_only_the_two_explicit_patch_private_composites() {
        assert!(supported_declaration_family("computed.fillet"));
        assert!(supported_declaration_family(
            "computed.roundedRectangleProfile"
        ));
        assert!(!supported_declaration_family("computed.unknownComposite"));
    }
}
