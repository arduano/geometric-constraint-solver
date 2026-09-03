// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded Rust authority for executed, reversible managed sketches.
//!
//! TypeScript owns closed-subset parsing, canonical printing and recorder
//! execution. This module admits only the resulting data-only wire after
//! independently authenticating its source, IR and artifact digests and all
//! cross-links. It deliberately does not execute JavaScript or solver
//! equations.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::intent_content_digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AuthoringDeclaration, AuthoringProgram, CodeResultKeySource, CodeResultShape, FeatureKind,
    GeneratedMemberAddress, ManagedDocument, ManagedImport, ManagedOrganization, ManagedOutput,
    ManagedOwnedSpan, ManagedOwnedSpanKind, ManagedPathSegment, ManagedScalarBinding, ManagedSpan,
    ManagedValue, ManagedValueOwnedSpan, PatchInvocation, SemanticOutputPath, SemanticSymbol,
    UnitLiteral, code_authoring_family, declaration_result_catalog,
};

pub const MANAGED_SKETCH_IR_FORMAT: &str = "geosolve-managed-sketch-ir-v3";
pub const EXECUTED_SKETCH_ARTIFACT_FORMAT: &str = "geosolve-executed-sketch-artifact-v3";
/// Maximum admitted byte length of one compiler-normalized managed sketch.
pub const MANAGED_SOURCE_LIMIT: usize = 4 * 1024 * 1024;
pub const MANAGED_WIRE_LIMIT: usize = 32 * 1024 * 1024;
const MAX_MANAGED_IMPORTS: usize = 4_096;
const MAX_MANAGED_STATEMENTS: usize = 65_536;
const MAX_MANAGED_SOURCE_SITES: usize = 262_144;
const MAX_MANAGED_RESULTS: usize = 262_144;
const MAX_MANAGED_VALUE_CONSUMERS: usize = 1_048_576;
const MAX_MANAGED_PATH_SEGMENTS: usize = 128;
const MAX_MANAGED_STRING_BYTES: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedSourceSiteKind {
    Declaration,
    Value,
    Group,
    GroupReference,
    Suppression,
    SuppressionReference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceSite {
    pub id: String,
    pub kind: ManagedSourceSiteKind,
    pub span: ManagedSourceSpan,
    pub source_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedExpression {
    Null {
        site: String,
    },
    Boolean {
        value: bool,
        site: String,
    },
    Number {
        value: f64,
        site: String,
    },
    String {
        value: String,
        site: String,
    },
    Array {
        values: Vec<Self>,
        site: String,
    },
    Object {
        fields: Vec<ManagedObjectField>,
        site: String,
    },
    Reference {
        declaration: String,
        path: Vec<ManagedPathSegment>,
        site: String,
    },
    Call {
        callee: String,
        #[serde(rename = "arguments")]
        arguments: Vec<Self>,
        site: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedObjectField {
    pub name: String,
    pub value: ManagedExpression,
    pub comments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedReference {
    pub declaration: String,
    pub path: Vec<ManagedPathSegment>,
    pub site: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "statement", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedStatement {
    Binding {
        variable: String,
        value: ManagedExpression,
        comments: Vec<String>,
    },
    Declaration {
        variable: String,
        symbol: String,
        builder_path: Vec<String>,
        patch: Option<String>,
        arguments: ManagedExpression,
        site: String,
        comments: Vec<String>,
    },
    Group {
        name: String,
        declarations: Vec<ManagedReference>,
        site: String,
        comments: Vec<String>,
    },
    Suppression {
        target: ManagedReference,
        site: String,
        comments: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedIrImport {
    pub module: String,
    pub bindings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSketchIr {
    pub format: String,
    pub imports: Vec<ManagedIrImport>,
    pub statements: Vec<ManagedStatement>,
    pub output: ManagedExpression,
    pub source_sites: Vec<ManagedSourceSite>,
    pub source_digest: String,
    pub ir_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedResultLeaf {
    pub kind: FeatureKind,
    pub path: Vec<ManagedPathSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedDeclarationResult {
    pub declaration: String,
    pub family: String,
    pub patch: Option<String>,
    pub site: String,
    pub result: Vec<ExecutedResultLeaf>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedGeneratedMemberAddress {
    pub invocation: String,
    pub template: Vec<String>,
    pub member_key: Vec<String>,
    pub output: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutedConsumerTarget {
    Declaration {
        declaration: String,
        family: String,
    },
    Generated {
        address: ExecutedGeneratedMemberAddress,
        family: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedValueConsumer {
    pub value_site: String,
    pub target: ExecutedConsumerTarget,
    pub property: Vec<ManagedPathSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedGeneratedMember {
    pub address: ExecutedGeneratedMemberAddress,
    pub family: String,
    pub kind: FeatureKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedGroup {
    pub name: String,
    pub site: String,
    pub declarations: Vec<ManagedReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedSuppression {
    pub site: String,
    pub target: ManagedReference,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedSketchArtifact {
    pub format: String,
    pub source_digest: String,
    pub ir_digest: String,
    pub declarations: Vec<ExecutedDeclarationResult>,
    pub generated_members: Vec<ExecutedGeneratedMember>,
    pub groups: Vec<ExecutedGroup>,
    pub suppressions: Vec<ExecutedSuppression>,
    pub value_consumers: Vec<ExecutedValueConsumer>,
    pub output: ManagedValue,
    pub artifact_digest: String,
}

/// Browser/Deno compiler handoff. Canonical strings are supplied separately
/// so Rust can prove exact cross-host number and object encoding parity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledManagedSource {
    pub input_source_digest: String,
    pub normalized_source: String,
    pub ir: ManagedSketchIr,
    pub artifact: ExecutedSketchArtifact,
    pub canonical_ir_json: String,
    pub canonical_artifact_json: String,
}

/// Authenticated source-level declaration families which own private helper
/// declarations as one lifecycle unit.
///
/// This projection is reconstructed from managed IR semantics on every use;
/// it is not a persisted editor overlay and never relies on declaration names,
/// display labels, or lexical adjacency.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedSourceDeclarationClosureKind {
    ProfileOffset,
}

/// One user-facing declaration root and the source-visible helper
/// declarations which share its lifecycle.
///
/// `root` and every entry in `helpers` are semantic declaration symbols. The
/// helpers remain ordinary managed declarations and therefore retain their
/// own source ranges, values, runtime outputs, and selection authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSourceDeclarationClosure {
    pub kind: ManagedSourceDeclarationClosureKind,
    pub root: SemanticSymbol,
    pub helpers: Vec<SemanticSymbol>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedValidationError {
    #[error("managed compiler handoff is {actual} bytes; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("managed compiler handoff JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("managed compiler handoff is invalid: {0}")]
    Invalid(String),
}

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

/// Source-owned managed suppression state lowered into the existing Rust
/// declaration and generated-member coordinates. It is reconstructed from the
/// authenticated IR/artifact pair rather than persisted as a GUI overlay.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ManagedSuppressionProjection {
    pub(crate) declarations: BTreeSet<SemanticSymbol>,
    pub(crate) generated_members: BTreeSet<GeneratedMemberAddress>,
}

impl CompiledManagedSource {
    /// Reconstructs authenticated declaration lifecycle closures from the
    /// complete validated managed IR.
    ///
    /// A generic open-chain or closed-profile aggregate is private to a
    /// Profile Offset only when exactly one semantic binding/declaration
    /// consumes it and that consumer is a Profile Offset owning the matching
    /// typed input with an exact result reference. Shared aggregates
    /// consequently remain independent source roots.
    ///
    /// # Errors
    ///
    /// Rejects an invalid compiler envelope before returning any lifecycle
    /// authority.
    pub fn source_declaration_closures(
        &self,
    ) -> Result<Vec<ManagedSourceDeclarationClosure>, ManagedValidationError> {
        self.validate()?;
        Ok(project_source_declaration_closures(self))
    }

    /// Reports whether one semantic declaration has explicit managed
    /// suppression authority.
    ///
    /// # Errors
    ///
    /// Returns the same typed validation error as suppression projection when
    /// the compiler envelope contains an invalid or ambiguous target.
    pub fn declaration_is_suppressed(
        &self,
        declaration: &SemanticSymbol,
    ) -> Result<bool, ManagedValidationError> {
        Ok(project_suppressions(self)?
            .declarations
            .contains(declaration))
    }

    /// Decodes and validates one bounded compiler result without executing it.
    ///
    /// # Errors
    ///
    /// Rejects malformed, non-canonical, stale, over-bound or cross-linked
    /// data before it can become project or native scene authority.
    pub fn from_json(json: &str) -> Result<Self, ManagedValidationError> {
        if json.len() > MANAGED_WIRE_LIMIT {
            return Err(ManagedValidationError::ResourceLimit {
                actual: json.len(),
                limit: MANAGED_WIRE_LIMIT,
            });
        }
        let compiled: Self = serde_json::from_str(json)
            .map_err(|error| ManagedValidationError::InvalidJson(error.to_string()))?;
        compiled.validate()?;
        Ok(compiled)
    }

    /// Validates exact source, canonical bytes, digests and semantic links.
    ///
    /// # Errors
    ///
    /// Returns a bounded diagnostic while leaving caller-owned authority
    /// untouched.
    pub fn validate(&self) -> Result<(), ManagedValidationError> {
        require_digest(&self.input_source_digest, "compiler input source")?;
        if self.normalized_source.len() > MANAGED_SOURCE_LIMIT {
            return invalid(format!(
                "normalized source is {} bytes; the limit is {MANAGED_SOURCE_LIMIT}",
                self.normalized_source.len()
            ));
        }
        if self.ir.format != MANAGED_SKETCH_IR_FORMAT {
            return invalid("unsupported managed sketch IR format");
        }
        if self.artifact.format != EXECUTED_SKETCH_ARTIFACT_FORMAT {
            return invalid("unsupported executed sketch artifact format");
        }
        let source_digest = intent_content_digest(self.normalized_source.as_bytes()).to_string();
        if self.ir.source_digest != source_digest || self.artifact.source_digest != source_digest {
            return invalid("source digest does not authenticate normalized source bytes");
        }
        require_digest(&self.ir.source_digest, "source")?;
        require_digest(&self.ir.ir_digest, "IR")?;
        require_digest(&self.artifact.artifact_digest, "artifact")?;

        let ir_envelope = IrDigestEnvelope {
            format: &self.ir.format,
            imports: &self.ir.imports,
            statements: &self.ir.statements,
            output: &self.ir.output,
            source_sites: &self.ir.source_sites,
            source_digest: &self.ir.source_digest,
        };
        let ir_bytes = serde_json::to_string(&ir_envelope)
            .map_err(|error| ManagedValidationError::Invalid(error.to_string()))?;
        if intent_content_digest(ir_bytes.as_bytes()).to_string() != self.ir.ir_digest {
            return invalid("IR digest mismatch");
        }
        let canonical_ir = serde_json::to_string(&self.ir)
            .map_err(|error| ManagedValidationError::Invalid(error.to_string()))?;
        if canonical_ir != self.canonical_ir_json {
            return invalid("TypeScript and Rust canonical IR bytes differ");
        }

        let artifact_envelope = ArtifactDigestEnvelope {
            format: &self.artifact.format,
            source_digest: &self.artifact.source_digest,
            ir_digest: &self.artifact.ir_digest,
            declarations: &self.artifact.declarations,
            generated_members: &self.artifact.generated_members,
            groups: &self.artifact.groups,
            suppressions: &self.artifact.suppressions,
            value_consumers: &self.artifact.value_consumers,
            output: &self.artifact.output,
        };
        let artifact_bytes = serde_json::to_string(&artifact_envelope)
            .map_err(|error| ManagedValidationError::Invalid(error.to_string()))?;
        if intent_content_digest(artifact_bytes.as_bytes()).to_string()
            != self.artifact.artifact_digest
        {
            return invalid("executed artifact digest mismatch");
        }
        let canonical_artifact = serde_json::to_string(&self.artifact)
            .map_err(|error| ManagedValidationError::Invalid(error.to_string()))?;
        if canonical_artifact != self.canonical_artifact_json {
            return invalid("TypeScript and Rust canonical artifact bytes differ");
        }
        if self.artifact.ir_digest != self.ir.ir_digest {
            return invalid("executed artifact belongs to a different managed IR");
        }
        self.validate_ir()?;
        self.validate_artifact()
    }

    /// Authenticates this compiler response against the exact Rust-retained
    /// source draft which was sent to the browser or Deno host.
    ///
    /// # Errors
    ///
    /// Returns a stale-input refusal without admitting the normalized result.
    pub fn validate_input_source(&self, source: &str) -> Result<(), ManagedValidationError> {
        if source.len() > MANAGED_SOURCE_LIMIT
            || intent_content_digest(source.as_bytes()).to_string() != self.input_source_digest
        {
            return invalid("managed compiler result belongs to different input source bytes");
        }
        self.validate()
    }

    /// Converts a validated compiler result into the existing equation-free
    /// authoring projection used by Intent expansion while retaining the
    /// complete managed IR/artifact as canonical authority.
    ///
    /// # Errors
    ///
    /// Rejects a recorder expression which cannot be represented by the
    /// current public authoring vocabulary. This never fabricates a GUI-only
    /// fallback.
    pub fn into_managed_document(self) -> Result<ManagedDocument, ManagedValidationError> {
        self.validate()?;
        let projection = project_managed_document(&self)?;
        Ok(ManagedDocument {
            compiled: Some(Box::new(self)),
            declaration_name_high_water: 0,
            ..projection
        })
    }

    pub(crate) fn projected_managed_document(
        &self,
    ) -> Result<ManagedDocument, ManagedValidationError> {
        self.validate()?;
        project_managed_document(self)
    }

    pub(crate) fn projected_suppressions(
        &self,
    ) -> Result<ManagedSuppressionProjection, ManagedValidationError> {
        project_suppressions(self)
    }

    pub(crate) fn projected_source_declaration_closures(
        &self,
    ) -> Vec<ManagedSourceDeclarationClosure> {
        project_source_declaration_closures(self)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one bounded pass keeps managed IR ordering and cross-site authentication adjacent"
    )]
    fn validate_ir(&self) -> Result<(), ManagedValidationError> {
        if self.ir.imports.len() > MAX_MANAGED_IMPORTS
            || self.ir.statements.len() > MAX_MANAGED_STATEMENTS
            || self.ir.source_sites.len() > MAX_MANAGED_SOURCE_SITES
        {
            return invalid("managed IR exceeds a collection bound");
        }
        let mut site_kinds = BTreeMap::new();
        for site in &self.ir.source_sites {
            require_text(&site.id, "source-site ID")?;
            require_digest(&site.id, "source-site ID")?;
            if site.source_digest != self.ir.source_digest {
                return invalid("source site belongs to a different source digest");
            }
            if site.span.start > site.span.end
                || site.span.end > self.normalized_source.len()
                || !self.normalized_source.is_char_boundary(site.span.start)
                || !self.normalized_source.is_char_boundary(site.span.end)
            {
                return invalid("source site is not a valid UTF-8 byte range");
            }
            if site_kinds.insert(site.id.as_str(), site.kind).is_some() {
                return invalid("duplicate managed source-site identity");
            }
        }
        let mut variables = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        let mut declarations = BTreeSet::new();
        let mut groups = BTreeSet::new();
        for import in &self.ir.imports {
            require_text(&import.module, "import module")?;
            if import.bindings.is_empty() {
                return invalid("managed import has no bindings");
            }
            let mut bindings = BTreeSet::new();
            for binding in &import.bindings {
                require_text(binding, "import binding")?;
                if !bindings.insert(binding) {
                    return invalid("managed import repeats a binding");
                }
            }
        }
        for statement in &self.ir.statements {
            match statement {
                ManagedStatement::Binding {
                    variable,
                    value,
                    comments,
                } => {
                    require_unique_name(variable, "binding", &mut variables)?;
                    validate_comments(comments)?;
                    validate_expression(value, &site_kinds, &variables, 0)?;
                    bindings.insert(variable.as_str(), value);
                }
                ManagedStatement::Declaration {
                    variable,
                    symbol,
                    builder_path,
                    patch,
                    arguments,
                    site,
                    comments,
                } => {
                    require_unique_name(variable, "binding", &mut variables)?;
                    require_unique_name(symbol, "declaration", &mut declarations)?;
                    if builder_path.is_empty() || builder_path.len() > MAX_MANAGED_PATH_SEGMENTS {
                        return invalid("managed declaration builder path is empty or too deep");
                    }
                    for segment in builder_path {
                        require_text(segment, "builder path segment")?;
                    }
                    if let Some(patch) = patch {
                        require_text(patch, "patch binding")?;
                    }
                    require_site(site, ManagedSourceSiteKind::Declaration, &site_kinds)?;
                    validate_comments(comments)?;
                    validate_expression(arguments, &site_kinds, &variables, 0)?;
                    if patch.is_none() {
                        validate_named_declaration_arguments(
                            arguments,
                            &bindings,
                            &mut BTreeSet::new(),
                        )?;
                    }
                }
                ManagedStatement::Group {
                    name,
                    declarations: references,
                    site,
                    comments,
                } => {
                    require_unique_name(name, "group", &mut groups)?;
                    require_site(site, ManagedSourceSiteKind::Group, &site_kinds)?;
                    validate_comments(comments)?;
                    for reference in references {
                        validate_reference(
                            reference,
                            ManagedSourceSiteKind::GroupReference,
                            &site_kinds,
                            &variables,
                        )?;
                    }
                }
                ManagedStatement::Suppression {
                    target,
                    site,
                    comments,
                } => {
                    require_site(site, ManagedSourceSiteKind::Suppression, &site_kinds)?;
                    validate_comments(comments)?;
                    validate_reference(
                        target,
                        ManagedSourceSiteKind::SuppressionReference,
                        &site_kinds,
                        &variables,
                    )?;
                }
            }
        }
        validate_expression(&self.ir.output, &site_kinds, &variables, 0)?;
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one bounded pass authenticates the complete executed artifact and its IR cross-links"
    )]
    fn validate_artifact(&self) -> Result<(), ManagedValidationError> {
        if self.artifact.declarations.len() > MAX_MANAGED_STATEMENTS
            || self.artifact.generated_members.len() > MAX_MANAGED_RESULTS
            || self.artifact.groups.len() > MAX_MANAGED_STATEMENTS
            || self.artifact.suppressions.len() > MAX_MANAGED_STATEMENTS
            || self.artifact.value_consumers.len() > MAX_MANAGED_VALUE_CONSUMERS
        {
            return invalid("executed managed artifact exceeds a collection bound");
        }
        let sites = self
            .ir
            .source_sites
            .iter()
            .map(|site| (site.id.as_str(), site.kind))
            .collect::<BTreeMap<_, _>>();
        let ir_declarations = self
            .ir
            .statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    builder_path,
                    patch,
                    site,
                    arguments,
                    ..
                } => Some((symbol, builder_path, patch, site, arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if ir_declarations.len() != self.artifact.declarations.len() {
            return invalid("executed declaration count differs from managed IR");
        }
        let result_catalog = declaration_result_catalog();
        let mut declaration_names = BTreeSet::new();
        for (executed, (symbol, builder_path, patch, site, arguments)) in
            self.artifact.declarations.iter().zip(ir_declarations)
        {
            let family = builder_path.join(".");
            if executed.declaration != *symbol
                || executed.family != family
                || executed.patch != *patch
                || executed.site != *site
            {
                return invalid("executed declaration does not match its lexical owner");
            }
            require_site(&executed.site, ManagedSourceSiteKind::Declaration, &sites)?;
            if !declaration_names.insert(executed.declaration.as_str()) {
                return invalid("executed artifact repeats a declaration");
            }
            let mut paths = BTreeSet::new();
            for result in &executed.result {
                validate_path(&result.path)?;
                if !paths.insert(&result.path) {
                    return invalid("executed declaration repeats a result path");
                }
            }
            if patch.is_none() {
                let [namespace, method] = builder_path.as_slice() else {
                    return invalid("managed declarations must use one named namespace and method");
                };
                let Some(authoring) = code_authoring_family(namespace, method) else {
                    return invalid(format!(
                        "unsupported named managed declaration family `{family}`"
                    ));
                };
                if authoring.availability != crate::CodeAuthoringAvailability::Public {
                    return invalid(format!(
                        "managed declaration family `{family}` requires host authority"
                    ));
                }
                let descriptor = result_catalog.get(&family).ok_or_else(|| {
                    ManagedValidationError::Invalid(format!(
                        "named declaration family `{family}` has no Rust result descriptor"
                    ))
                })?;
                let expected =
                    direct_declaration_result_leaves(&family, arguments, &descriptor.outputs);
                if executed.result != expected {
                    return invalid(
                        "direct declaration result differs from the Rust-owned result descriptor",
                    );
                }
            }
        }
        validate_direct_value_consumers(self)?;
        let mut generated = BTreeSet::new();
        for member in &self.artifact.generated_members {
            validate_generated_address(&member.address)?;
            let owner = self
                .artifact
                .declarations
                .iter()
                .find(|declaration| declaration.declaration == member.address.invocation);
            let Some(owner) = owner else {
                return invalid("generated member has no executed invocation owner");
            };
            if owner.patch.is_none() {
                return invalid("generated member owner is not a patch invocation");
            }
            if !generated.insert(&member.address) {
                return invalid("executed artifact repeats a generated-member address");
            }
            require_text(&member.family, "generated member family")?;
        }
        let ir_groups = self
            .ir
            .statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Group {
                    name,
                    declarations,
                    site,
                    ..
                } => Some((name, declarations, site)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if self.artifact.groups.len() != ir_groups.len()
            || self.artifact.groups.iter().zip(ir_groups).any(
                |(executed, (name, references, site))| {
                    executed.name != *name
                        || executed.declarations != *references
                        || executed.site != *site
                },
            )
        {
            return invalid("executed groups differ from managed IR");
        }
        let ir_suppressions = self
            .ir
            .statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Suppression { target, site, .. } => Some((target, site)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if self.artifact.suppressions.len() != ir_suppressions.len()
            || self.artifact.suppressions.iter().zip(ir_suppressions).any(
                |(executed, (target, site))| executed.target != *target || executed.site != *site,
            )
        {
            return invalid("executed suppressions differ from managed IR");
        }
        for consumer in &self.artifact.value_consumers {
            require_site(&consumer.value_site, ManagedSourceSiteKind::Value, &sites)?;
            validate_path(&consumer.property)?;
            match &consumer.target {
                ExecutedConsumerTarget::Declaration {
                    declaration,
                    family,
                } => {
                    if !self.artifact.declarations.iter().any(|candidate| {
                        candidate.declaration == *declaration && candidate.family == *family
                    }) {
                        return invalid("value consumer has no executed declaration target");
                    }
                }
                ExecutedConsumerTarget::Generated { address, family } => {
                    if !self.artifact.generated_members.iter().any(|candidate| {
                        candidate.address == *address && candidate.family == *family
                    }) {
                        return invalid("value consumer has no generated-member target");
                    }
                }
            }
        }
        let evaluated_output = evaluate_sketch_output(self)?;
        if !managed_callback_outputs_equivalent(&self.artifact.output, &evaluated_output) {
            return invalid("executed callback output differs from managed IR evaluation");
        }
        project_suppressions(self)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ProfileOffsetHelperKind {
    OpenChain,
    ClosedProfile,
}

impl ProfileOffsetHelperKind {
    const fn builder(self) -> &'static str {
        match self {
            Self::OpenChain => "openChain",
            Self::ClosedProfile => "closedProfile",
        }
    }

    const fn input_kind(self) -> &'static str {
        match self {
            Self::OpenChain => "chain",
            Self::ClosedProfile => "profile",
        }
    }

    fn result_path(self) -> Vec<ManagedPathSegment> {
        vec![ManagedPathSegment::Field(self.input_kind().to_owned())]
    }
}

struct LexicalDeclaration<'a> {
    variable: &'a str,
    symbol: &'a str,
    builder_path: &'a [String],
    patch: &'a Option<String>,
    arguments: &'a ManagedExpression,
}

#[derive(Clone, Copy)]
enum SourceSemanticConsumer<'a> {
    Binding,
    Declaration(&'a str),
    Multiple,
}

/// Derives lifecycle ownership from the validated IR only. Keep this pass
/// independent of source spelling and executed display metadata: callers may
/// use it after cold compile/reload and receive the same answer.
fn project_source_declaration_closures(
    compiled: &CompiledManagedSource,
) -> Vec<ManagedSourceDeclarationClosure> {
    let declarations = compiled
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                variable,
                symbol,
                builder_path,
                patch,
                arguments,
                ..
            } => Some(LexicalDeclaration {
                variable,
                symbol,
                builder_path,
                patch,
                arguments,
            }),
            ManagedStatement::Binding { .. }
            | ManagedStatement::Group { .. }
            | ManagedStatement::Suppression { .. } => None,
        })
        .collect::<Vec<_>>();

    let helper_variables = declarations
        .iter()
        .filter(|declaration| profile_offset_helper_kind(declaration).is_some())
        .map(|declaration| declaration.variable)
        .collect::<BTreeSet<_>>();
    let declarations_by_variable = declarations
        .iter()
        .map(|declaration| (declaration.variable, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut semantic_consumers = BTreeMap::<&str, SourceSemanticConsumer<'_>>::new();
    for statement in &compiled.ir.statements {
        let (consumer, expression) = match statement {
            ManagedStatement::Binding { value, .. } => (SourceSemanticConsumer::Binding, value),
            ManagedStatement::Declaration {
                variable,
                arguments,
                ..
            } => (SourceSemanticConsumer::Declaration(variable), arguments),
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {
                continue;
            }
        };
        let mut references = BTreeSet::new();
        collect_expression_reference_variables(expression, &mut references);
        for reference in references {
            if !helper_variables.contains(reference) {
                continue;
            }
            semantic_consumers
                .entry(reference)
                .and_modify(|current| *current = SourceSemanticConsumer::Multiple)
                .or_insert(consumer);
        }
    }

    let mut helpers_by_root = BTreeMap::<&str, Vec<SemanticSymbol>>::new();
    for helper in &declarations {
        let Some(helper_kind) = profile_offset_helper_kind(helper) else {
            continue;
        };
        let Some(SourceSemanticConsumer::Declaration(root_variable)) =
            semantic_consumers.get(helper.variable)
        else {
            // No consumer, a uniquely consuming lexical binding, or more than
            // one semantic consumer leaves the aggregate independent.
            continue;
        };
        let Some(root) = declarations_by_variable.get(root_variable) else {
            continue;
        };
        if !is_profile_offset_root(root)
            || !has_exact_profile_offset_input(root.arguments, helper, helper_kind)
        {
            continue;
        }
        helpers_by_root
            .entry(root.variable)
            .or_default()
            .push(SemanticSymbol(helper.symbol.to_owned()));
    }

    declarations
        .iter()
        .filter_map(|root| {
            helpers_by_root
                .remove(root.variable)
                .map(|helpers| ManagedSourceDeclarationClosure {
                    kind: ManagedSourceDeclarationClosureKind::ProfileOffset,
                    root: SemanticSymbol(root.symbol.to_owned()),
                    helpers,
                })
        })
        .collect()
}

fn profile_offset_helper_kind(
    declaration: &LexicalDeclaration<'_>,
) -> Option<ProfileOffsetHelperKind> {
    if declaration.patch.is_some() {
        return None;
    }
    [
        ProfileOffsetHelperKind::OpenChain,
        ProfileOffsetHelperKind::ClosedProfile,
    ]
    .into_iter()
    .find(|kind| declaration.builder_path == ["aggregate", kind.builder()])
}

fn is_profile_offset_root(declaration: &LexicalDeclaration<'_>) -> bool {
    declaration.patch.is_none() && declaration.builder_path == ["operation", "profileOffset"]
}

fn has_exact_profile_offset_input(
    arguments: &ManagedExpression,
    helper: &LexicalDeclaration<'_>,
    helper_kind: ProfileOffsetHelperKind,
) -> bool {
    let Some(ManagedExpression::Array { values, .. }) = object_field(arguments, "sources") else {
        return false;
    };
    let expected_path = helper_kind.result_path();
    values.iter().any(|input| {
        matches!(
            input,
            ManagedExpression::Reference { declaration, path, .. }
                if declaration == helper.variable && path == &expected_path
        )
    })
}

fn object_field<'a>(
    expression: &'a ManagedExpression,
    name: &str,
) -> Option<&'a ManagedExpression> {
    let ManagedExpression::Object { fields, .. } = expression else {
        return None;
    };
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| &field.value)
}

fn collect_expression_reference_variables<'a>(
    expression: &'a ManagedExpression,
    variables: &mut BTreeSet<&'a str>,
) {
    match expression {
        ManagedExpression::Reference { declaration, .. } => {
            variables.insert(declaration);
        }
        ManagedExpression::Array { values, .. } => {
            for value in values {
                collect_expression_reference_variables(value, variables);
            }
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                collect_expression_reference_variables(&field.value, variables);
            }
        }
        ManagedExpression::Call { arguments, .. } => {
            for argument in arguments {
                collect_expression_reference_variables(argument, variables);
            }
        }
        ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. }
        | ManagedExpression::String { .. } => {}
    }
}

fn project_suppressions(
    compiled: &CompiledManagedSource,
) -> Result<ManagedSuppressionProjection, ManagedValidationError> {
    let lexical_declarations = compiled
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                variable,
                symbol,
                patch,
                ..
            } => Some((
                variable.as_str(),
                (SemanticSymbol(symbol.clone()), patch.is_some()),
            )),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let executed_declarations = compiled
        .artifact
        .declarations
        .iter()
        .map(|declaration| (declaration.declaration.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let mut projection = ManagedSuppressionProjection::default();
    for suppression in &compiled.artifact.suppressions {
        let Some((owner, patch_owned)) =
            lexical_declarations.get(suppression.target.declaration.as_str())
        else {
            return invalid("managed suppression does not target a declaration");
        };
        if suppression.target.path.is_empty() {
            if !projection.declarations.insert(owner.clone()) {
                return invalid("managed repeats a declaration suppression");
            }
            continue;
        }
        if !patch_owned {
            return invalid("managed nested suppression does not target a generated patch member");
        }
        let executed = executed_declarations.get(owner.0.as_str()).ok_or_else(|| {
            ManagedValidationError::Invalid(
                "managed suppression lost its executed declaration owner".into(),
            )
        })?;
        let matches = compiled
            .artifact
            .generated_members
            .iter()
            .filter(|member| {
                member.address.invocation == owner.0
                    && generated_member_reference_matches(
                        executed,
                        &member.address,
                        &suppression.target.path,
                    )
            })
            .collect::<Vec<_>>();
        let [member] = matches.as_slice() else {
            return invalid(if matches.is_empty() {
                "managed nested suppression has no exact generated-member target"
            } else {
                "managed nested suppression is ambiguous across generated members"
            });
        };
        let member_key = if member.address.member_key.is_empty() {
            vec!["self".to_owned()]
        } else {
            member.address.member_key.clone()
        };
        let address = GeneratedMemberAddress::new(
            member.address.invocation.clone(),
            member.address.template.clone(),
            member_key,
            member.address.output.clone(),
        );
        if !projection.generated_members.insert(address) {
            return invalid("managed repeats a generated-member suppression");
        }
    }
    Ok(projection)
}

fn generated_member_reference_matches(
    declaration: &ExecutedDeclarationResult,
    address: &ExecutedGeneratedMemberAddress,
    target: &[ManagedPathSegment],
) -> bool {
    let member_suffix = address
        .member_key
        .iter()
        .cloned()
        .map(|member| ManagedPathSegment::Member { member })
        .collect::<Vec<_>>();
    if declaration.result.iter().any(|result| {
        let mut candidate = result.path.clone();
        candidate.extend(member_suffix.iter().cloned());
        candidate == target
    }) {
        return true;
    }
    address.member_key.is_empty()
        && address
            .template
            .iter()
            .cloned()
            .map(ManagedPathSegment::Field)
            .eq(target.iter().cloned())
}

#[allow(
    clippy::too_many_lines,
    reason = "one projection keeps every authenticated statement and source span in lexical order"
)]
fn project_managed_document(
    compiled: &CompiledManagedSource,
) -> Result<ManagedDocument, ManagedValidationError> {
    let site_spans = compiled
        .ir
        .source_sites
        .iter()
        .map(|site| {
            (
                site.id.as_str(),
                ManagedSpan::new(site.span.start, site.span.end),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let imports = compiled
        .ir
        .imports
        .iter()
        .map(|import| ManagedImport {
            module: import.module.clone(),
            bindings: import.bindings.clone(),
            span: ManagedSpan::new(0, 0),
        })
        .collect::<Vec<_>>();
    let mut variables = BTreeMap::<String, SemanticSymbol>::new();
    let mut scalar_bindings = Vec::new();
    let mut declarations = Vec::new();
    let mut value_owned_spans = Vec::new();
    let mut organizations = Vec::new();
    for statement in &compiled.ir.statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => {
                let symbol = SemanticSymbol(variable.clone());
                let converted = convert_scalar_binding(value, &variables)?;
                let span = expression_span(value, &site_spans)?;
                variables.insert(variable.clone(), symbol.clone());
                scalar_bindings.push(ManagedScalarBinding {
                    variable: variable.clone(),
                    symbol: symbol.clone(),
                    value: converted,
                    statement_span: span,
                    symbol_span: span,
                    value_span: span,
                });
                value_owned_spans.push(ManagedValueOwnedSpan {
                    declaration: symbol,
                    path: SemanticOutputPath::default(),
                    kind: expression_owned_kind(value),
                    span,
                    source_digest: compiled.ir.source_digest.clone(),
                });
            }
            ManagedStatement::Declaration {
                variable,
                symbol,
                builder_path,
                patch,
                arguments,
                site,
                ..
            } => {
                let semantic = SemanticSymbol(symbol.clone());
                let statement_span = site_span(site, &site_spans)?;
                let converted = convert_expression(arguments, &variables)?;
                collect_value_owned_spans(
                    arguments,
                    &semantic,
                    &SemanticOutputPath::default(),
                    &site_spans,
                    &compiled.ir.source_digest,
                    &mut value_owned_spans,
                )?;
                variables.insert(variable.clone(), semantic.clone());
                declarations.push(AuthoringDeclaration {
                    variable: variable.clone(),
                    symbol: semantic,
                    builder_path: builder_path.clone(),
                    arguments: converted.clone(),
                    patch: patch.as_ref().map(|module_binding| PatchInvocation {
                        module_binding: module_binding.clone(),
                        arguments: converted,
                    }),
                    statement_span,
                    symbol_span: statement_span,
                    arguments_span: expression_span(arguments, &site_spans)?,
                });
            }
            ManagedStatement::Group {
                name,
                declarations: references,
                site,
                ..
            } => {
                let mut grouped = Vec::new();
                for reference in references {
                    let declaration = variables.get(&reference.declaration).ok_or_else(|| {
                        ManagedValidationError::Invalid(format!(
                            "group `{name}` references absent variable `{}`",
                            reference.declaration
                        ))
                    })?;
                    if !grouped.contains(declaration) {
                        grouped.push(declaration.clone());
                    }
                }
                organizations.push(ManagedOrganization {
                    name: name.clone(),
                    declarations: grouped,
                    span: site_span(site, &site_spans)?,
                });
            }
            ManagedStatement::Suppression { .. } => {}
        }
    }
    let mut outputs = Vec::new();
    collect_callback_outputs(Some(compiled), &compiled.artifact.output, "", &mut outputs)?;
    let owned_spans = compiled
        .ir
        .source_sites
        .iter()
        .map(|site| ManagedOwnedSpan {
            kind: match site.kind {
                ManagedSourceSiteKind::Declaration => ManagedOwnedSpanKind::Declaration,
                ManagedSourceSiteKind::Value => ManagedOwnedSpanKind::Literal,
                ManagedSourceSiteKind::Group
                | ManagedSourceSiteKind::GroupReference
                | ManagedSourceSiteKind::Suppression
                | ManagedSourceSiteKind::SuppressionReference => ManagedOwnedSpanKind::Organization,
            },
            span: ManagedSpan::new(site.span.start, site.span.end),
            source_digest: compiled.ir.source_digest.clone(),
        })
        .collect();
    Ok(ManagedDocument {
        source: compiled.normalized_source.clone(),
        source_digest: compiled.ir.source_digest.clone(),
        imports,
        program: AuthoringProgram {
            scalar_bindings,
            declarations,
            organizations,
            outputs,
        },
        envelope_span: ManagedSpan::new(0, compiled.normalized_source.len()),
        owned_spans,
        value_owned_spans,
        compiled: None,
        declaration_name_high_water: 0,
    })
}

fn collect_callback_outputs(
    compiled: Option<&CompiledManagedSource>,
    value: &ManagedValue,
    name: &str,
    outputs: &mut Vec<ManagedOutput>,
) -> Result<(), ManagedValidationError> {
    match value {
        ManagedValue::Reference { declaration, path } => {
            if path.0.is_empty() {
                let compiled = compiled.ok_or_else(|| {
                    ManagedValidationError::Invalid(
                        "root callback output requires executed declaration results".into(),
                    )
                })?;
                let mut declarations = compiled
                    .artifact
                    .declarations
                    .iter()
                    .filter(|candidate| candidate.declaration == declaration.0);
                let declaration_result = declarations.next().ok_or_else(|| {
                    ManagedValidationError::Invalid(format!(
                        "managed callback output references absent declaration `{}`",
                        declaration.0
                    ))
                })?;
                if declarations.next().is_some() {
                    return invalid(format!(
                        "managed callback output references ambiguous declaration `{}`",
                        declaration.0
                    ));
                }
                for leaf in &declaration_result.result {
                    let suffix = semantic_path_text(&leaf.path);
                    let expanded_name = if name.is_empty() {
                        format!("{}.{}", declaration.0, suffix)
                    } else {
                        format!("{name}.{suffix}")
                    };
                    if outputs.iter().any(|output| output.name == expanded_name) {
                        return invalid(format!(
                            "managed callback output repeats semantic name `{expanded_name}`"
                        ));
                    }
                    outputs.push(ManagedOutput {
                        name: expanded_name,
                        declaration: declaration.clone(),
                        path: SemanticOutputPath(leaf.path.clone()),
                    });
                }
                return Ok(());
            }
            let name = if name.is_empty() {
                declaration.0.clone()
            } else {
                name.to_owned()
            };
            if outputs.iter().any(|output| output.name == name) {
                return invalid(format!(
                    "managed callback output repeats semantic name `{name}`"
                ));
            }
            outputs.push(ManagedOutput {
                name,
                declaration: declaration.clone(),
                path: path.clone(),
            });
        }
        ManagedValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let child = if name.is_empty() {
                    format!("[{index}]")
                } else {
                    format!("{name}[{index}]")
                };
                collect_callback_outputs(compiled, value, &child, outputs)?;
            }
        }
        ManagedValue::Object(fields) => {
            for (field, value) in fields {
                let child = if name.is_empty() {
                    field.clone()
                } else {
                    format!("{name}.{field}")
                };
                collect_callback_outputs(compiled, value, &child, outputs)?;
            }
        }
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_) => {}
    }
    Ok(())
}

fn semantic_path_text(path: &[ManagedPathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            ManagedPathSegment::Field(field) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(field);
            }
            ManagedPathSegment::Index(index) => {
                use std::fmt::Write as _;
                write!(text, "[{index}]").expect("writing a semantic path to String cannot fail");
            }
            ManagedPathSegment::Member { member } => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(member);
            }
        }
    }
    text
}

fn convert_scalar_binding(
    expression: &ManagedExpression,
    variables: &BTreeMap<String, SemanticSymbol>,
) -> Result<ManagedValue, ManagedValidationError> {
    let value = convert_expression(expression, variables)?;
    if !matches!(value, ManagedValue::Number(_) | ManagedValue::Unit(_)) {
        return invalid(
            "the current managed materializer admits only numeric or unit lexical bindings",
        );
    }
    Ok(value)
}

#[derive(Default)]
struct DirectDeclarationResultKeys {
    vertices: Vec<String>,
    segments: Vec<String>,
    corners: Vec<String>,
    controls: Vec<String>,
    spans: Vec<String>,
    fillets: Vec<String>,
}

impl DirectDeclarationResultKeys {
    fn for_path(&self, path: &[ManagedPathSegment]) -> &[String] {
        match path.last() {
            Some(ManagedPathSegment::Field(field)) if field == "segments" => &self.segments,
            Some(ManagedPathSegment::Field(field)) if field == "filletableCorners" => &self.corners,
            _ => &self.vertices,
        }
    }

    fn for_source(&self, source: CodeResultKeySource) -> &[String] {
        match source {
            CodeResultKeySource::PolylineVertices => &self.vertices,
            CodeResultKeySource::PolylineSegments => &self.segments,
            CodeResultKeySource::PolylineCorners => &self.corners,
            CodeResultKeySource::SplineControls => &self.controls,
            CodeResultKeySource::SplineSpans => &self.spans,
            CodeResultKeySource::FilletCorners => &self.fillets,
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed result-key dispatcher keeps every keyed direct-declaration output family auditable"
)]
fn direct_declaration_keys(
    family: &str,
    arguments: &ManagedExpression,
) -> DirectDeclarationResultKeys {
    let ManagedExpression::Object { fields, .. } = arguments else {
        return DirectDeclarationResultKeys::default();
    };
    let keyed_argument = |name: &str| -> Option<Vec<String>> {
        let ManagedExpression::Array { values, .. } = fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)?
        else {
            return None;
        };
        values
            .iter()
            .map(|value| {
                let ManagedExpression::Object { fields, .. } = value else {
                    return None;
                };
                let ManagedExpression::String { value, .. } = fields
                    .iter()
                    .find(|field| field.name == "key")
                    .map(|field| &field.value)?
                else {
                    return None;
                };
                Some(value.clone())
            })
            .collect()
    };
    match family {
        "geometry.polyline" => {
            let Some(keys) = keyed_argument("vertices") else {
                return DirectDeclarationResultKeys::default();
            };
            let closed = matches!(
                fields
                    .iter()
                    .find(|field| field.name == "closed")
                    .map(|field| &field.value),
                Some(ManagedExpression::Boolean { value: true, .. })
            );
            let segments = if closed {
                keys.clone()
            } else {
                keys[..keys.len().saturating_sub(1)].to_vec()
            };
            let corners = if closed {
                keys.clone()
            } else if keys.len() > 2 {
                keys[1..keys.len() - 1].to_vec()
            } else {
                Vec::new()
            };
            DirectDeclarationResultKeys {
                vertices: keys,
                segments,
                corners,
                ..DirectDeclarationResultKeys::default()
            }
        }
        "geometry.openControlBSpline"
        | "geometry.periodicControlBSpline"
        | "geometry.openControlNurbs"
        | "geometry.periodicControlNurbs" => {
            let Some(controls) = keyed_argument("controls") else {
                return DirectDeclarationResultKeys::default();
            };
            let degree = fields
                .iter()
                .find(|field| field.name == "degree")
                .and_then(|field| match field.value {
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "finite integral nonnegative source values are bounded before conversion"
                    )]
                    ManagedExpression::Number { value, .. }
                        if value.is_finite()
                            && (1.0..=9_007_199_254_740_991.0).contains(&value)
                            && value.fract() == 0.0 =>
                    {
                        Some(value as usize)
                    }
                    _ => None,
                })
                .unwrap_or(0);
            let span_count = if matches!(
                family,
                "geometry.periodicControlBSpline" | "geometry.periodicControlNurbs"
            ) {
                controls.len()
            } else {
                controls.len().saturating_sub(degree)
            };
            DirectDeclarationResultKeys {
                spans: controls[..span_count].to_vec(),
                controls,
                ..DirectDeclarationResultKeys::default()
            }
        }
        "computed.filletSet" => DirectDeclarationResultKeys {
            fillets: keyed_argument("corners").unwrap_or_default(),
            ..DirectDeclarationResultKeys::default()
        },
        _ => DirectDeclarationResultKeys::default(),
    }
}

/// Reconstructs the exact executed result leaves for one direct declaration.
///
/// Keep this shared by cold compiler-envelope validation and prepared mutation
/// validation. In particular, an open Polyline exposes every vertex, only its
/// real spans, and only its interior filletable corners; treating all keyed
/// collections as the vertex set would reject the compiler's truthful result
/// after a canvas insertion.
pub(crate) fn direct_declaration_result_leaves(
    family: &str,
    arguments: &ManagedExpression,
    shape: &CodeResultShape,
) -> Vec<ExecutedResultLeaf> {
    let keys = direct_declaration_keys(family, arguments);
    let mut leaves = Vec::new();
    flatten_direct_result_shape(shape, &mut Vec::new(), &keys, &mut leaves);
    leaves
}

fn flatten_direct_result_shape(
    shape: &CodeResultShape,
    path: &mut Vec<ManagedPathSegment>,
    keys: &DirectDeclarationResultKeys,
    leaves: &mut Vec<ExecutedResultLeaf>,
) {
    match shape {
        CodeResultShape::Leaf { kind } => leaves.push(ExecutedResultLeaf {
            kind: *kind,
            path: path.clone(),
        }),
        CodeResultShape::NativeSpan => leaves.push(ExecutedResultLeaf {
            kind: FeatureKind::CurveSpan,
            path: path.clone(),
        }),
        CodeResultShape::Object { fields } => {
            for (name, child) in fields {
                path.push(ManagedPathSegment::Field(name.clone()));
                flatten_direct_result_shape(child, path, keys, leaves);
                path.pop();
            }
        }
        CodeResultShape::Tuple { items } => {
            for (index, child) in items.iter().enumerate() {
                path.push(ManagedPathSegment::Index(index));
                flatten_direct_result_shape(child, path, keys, leaves);
                path.pop();
            }
        }
        CodeResultShape::Keyed { kind, .. } => {
            leaves.push(ExecutedResultLeaf {
                kind: FeatureKind::Collection,
                path: path.clone(),
            });
            for key in keys.for_path(path) {
                path.push(ManagedPathSegment::Member {
                    member: key.clone(),
                });
                leaves.push(ExecutedResultLeaf {
                    kind: *kind,
                    path: path.clone(),
                });
                path.pop();
            }
        }
        CodeResultShape::NativeSpanKeyed { .. } => {
            leaves.push(ExecutedResultLeaf {
                kind: FeatureKind::Collection,
                path: path.clone(),
            });
            for key in keys.for_path(path) {
                path.push(ManagedPathSegment::Member {
                    member: key.clone(),
                });
                leaves.push(ExecutedResultLeaf {
                    kind: FeatureKind::CurveSpan,
                    path: path.clone(),
                });
                path.pop();
            }
        }
        CodeResultShape::DynamicKeyed { source, member, .. } => {
            leaves.push(ExecutedResultLeaf {
                kind: FeatureKind::Collection,
                path: path.clone(),
            });
            for key in keys.for_source(*source) {
                path.push(ManagedPathSegment::Member {
                    member: key.clone(),
                });
                flatten_direct_result_shape(member, path, keys, leaves);
                path.pop();
            }
        }
    }
}

fn validate_direct_value_consumers(
    compiled: &CompiledManagedSource,
) -> Result<(), ManagedValidationError> {
    let mut binding_origins = BTreeMap::<String, Vec<String>>::new();
    let mut expected = BTreeMap::<String, usize>::new();
    for statement in &compiled.ir.statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => {
                binding_origins.insert(
                    variable.clone(),
                    direct_expression_origins(value, &binding_origins),
                );
            }
            ManagedStatement::Declaration {
                symbol,
                builder_path,
                arguments,
                patch: None,
                ..
            } => collect_expected_direct_consumers(
                arguments,
                &ExecutedConsumerTarget::Declaration {
                    declaration: symbol.clone(),
                    family: builder_path.join("."),
                },
                &mut Vec::new(),
                &binding_origins,
                &mut expected,
            )?,
            ManagedStatement::Declaration { .. }
            | ManagedStatement::Group { .. }
            | ManagedStatement::Suppression { .. } => {}
        }
    }

    let mut actual = BTreeMap::<String, usize>::new();
    for consumer in &compiled.artifact.value_consumers {
        if matches!(consumer.target, ExecutedConsumerTarget::Declaration { .. }) {
            let key =
                direct_consumer_key(&consumer.value_site, &consumer.target, &consumer.property)?;
            *actual.entry(key).or_insert(0) += 1;
        }
    }
    if actual != expected {
        return invalid(
            "executed artifact lacks complete direct value-consumer provenance from its IR",
        );
    }
    Ok(())
}

fn direct_expression_origins(
    expression: &ManagedExpression,
    bindings: &BTreeMap<String, Vec<String>>,
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
            .flat_map(|value| direct_expression_origins(value, bindings))
            .collect(),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .flat_map(|field| direct_expression_origins(&field.value, bindings))
            .collect(),
    }
}

fn validate_named_declaration_arguments<'a>(
    expression: &'a ManagedExpression,
    bindings: &BTreeMap<&'a str, &'a ManagedExpression>,
    visited_bindings: &mut BTreeSet<&'a str>,
) -> Result<(), ManagedValidationError> {
    match expression {
        ManagedExpression::Array { values, .. } => {
            for value in values {
                validate_named_declaration_arguments(value, bindings, visited_bindings)?;
            }
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                if matches!(
                    field.name.as_str(),
                    "recipe"
                        | "inputs"
                        | "fields"
                        | "values"
                        | "results"
                        | "operationOutputs"
                        | "outputs"
                        | "editLens"
                ) {
                    return invalid(format!(
                        "managed named declaration arguments cannot contain retired transport property `{}`",
                        field.name
                    ));
                }
                validate_named_declaration_arguments(&field.value, bindings, visited_bindings)?;
            }
        }
        ManagedExpression::Call { arguments, .. } => {
            for argument in arguments {
                validate_named_declaration_arguments(argument, bindings, visited_bindings)?;
            }
        }
        ManagedExpression::Reference { declaration, .. } => {
            if let Some(binding) = bindings.get(declaration.as_str())
                && visited_bindings.insert(declaration)
            {
                validate_named_declaration_arguments(binding, bindings, visited_bindings)?;
            }
        }
        ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. }
        | ManagedExpression::String { .. } => {}
    }
    Ok(())
}

fn collect_expected_direct_consumers(
    expression: &ManagedExpression,
    target: &ExecutedConsumerTarget,
    property: &mut Vec<ManagedPathSegment>,
    bindings: &BTreeMap<String, Vec<String>>,
    consumers: &mut BTreeMap<String, usize>,
) -> Result<(), ManagedValidationError> {
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
                collect_expected_direct_consumers(value, target, property, bindings, consumers)?;
                property.pop();
            }
            return Ok(());
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                property.push(ManagedPathSegment::Field(field.name.clone()));
                collect_expected_direct_consumers(
                    &field.value,
                    target,
                    property,
                    bindings,
                    consumers,
                )?;
                property.pop();
            }
            return Ok(());
        }
    };
    for site in sites {
        let key = direct_consumer_key(&site, target, property)?;
        *consumers.entry(key).or_insert(0) += 1;
    }
    Ok(())
}

fn direct_consumer_key(
    site: &str,
    target: &ExecutedConsumerTarget,
    property: &[ManagedPathSegment],
) -> Result<String, ManagedValidationError> {
    serde_json::to_string(&(site, target, property))
        .map_err(|error| ManagedValidationError::Invalid(error.to_string()))
}

fn convert_expression(
    expression: &ManagedExpression,
    variables: &BTreeMap<String, SemanticSymbol>,
) -> Result<ManagedValue, ManagedValidationError> {
    match expression {
        ManagedExpression::Null { .. } => Ok(ManagedValue::Null),
        ManagedExpression::Boolean { value, .. } => Ok(ManagedValue::Bool(*value)),
        ManagedExpression::Number { value, .. } => Ok(ManagedValue::Number(*value)),
        ManagedExpression::String { value, .. } => Ok(ManagedValue::String(value.clone())),
        ManagedExpression::Array { values, .. } => values
            .iter()
            .map(|value| convert_expression(value, variables))
            .collect::<Result<Vec<_>, _>>()
            .map(ManagedValue::Array),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .map(|field| {
                Ok((
                    field.name.clone(),
                    convert_expression(&field.value, variables)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ManagedValidationError>>()
            .map(ManagedValue::Object),
        ManagedExpression::Reference {
            declaration, path, ..
        } => Ok(ManagedValue::Reference {
            declaration: variables.get(declaration).cloned().ok_or_else(|| {
                ManagedValidationError::Invalid(format!(
                    "managed projection has an unknown reference `{declaration}`"
                ))
            })?,
            path: SemanticOutputPath(path.clone()),
        }),
        ManagedExpression::Call {
            callee, arguments, ..
        } => {
            if !matches!(callee.as_str(), "mm" | "cm" | "m" | "inch" | "deg" | "rad")
                || arguments.len() != 1
            {
                return invalid(format!(
                    "managed value call `{callee}` has no native authoring representation"
                ));
            }
            let ManagedExpression::Number { value, .. } = &arguments[0] else {
                return invalid(format!(
                    "managed unit call `{callee}` requires one numeric literal"
                ));
            };
            Ok(ManagedValue::Unit(UnitLiteral {
                unit: callee.clone(),
                value: *value,
            }))
        }
    }
}

fn evaluate_sketch_output(
    compiled: &CompiledManagedSource,
) -> Result<ManagedValue, ManagedValidationError> {
    let mut values = BTreeMap::<String, ManagedValue>::new();
    for statement in &compiled.ir.statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => {
                let evaluated = evaluate_runtime_expression(value, &values)?;
                values.insert(variable.clone(), evaluated);
            }
            ManagedStatement::Declaration {
                variable, symbol, ..
            } => {
                values.insert(
                    variable.clone(),
                    ManagedValue::Reference {
                        declaration: SemanticSymbol(symbol.clone()),
                        path: SemanticOutputPath::default(),
                    },
                );
            }
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {}
        }
    }
    evaluate_runtime_expression(&compiled.ir.output, &values)
}

/// Property access in lexical IR can only name the JavaScript key. The
/// executed result shape supplies whether that key is an ordinary field or a
/// keyed-collection member. Keep the independent callback-output check exact
/// for every value while comparing those two wire spellings by their shared
/// runtime property key.
#[allow(
    clippy::float_cmp,
    reason = "runtime callback parity compares canonical executed numbers exactly; this is not a numerical tolerance decision"
)]
fn managed_callback_outputs_equivalent(left: &ManagedValue, right: &ManagedValue) -> bool {
    match (left, right) {
        (ManagedValue::Null, ManagedValue::Null) => true,
        (ManagedValue::Bool(left), ManagedValue::Bool(right)) => left == right,
        (ManagedValue::Number(left), ManagedValue::Number(right)) => left == right,
        (ManagedValue::String(left), ManagedValue::String(right)) => left == right,
        (ManagedValue::Unit(left), ManagedValue::Unit(right)) => left == right,
        (ManagedValue::Array(left), ManagedValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| managed_callback_outputs_equivalent(left, right))
        }
        (ManagedValue::Object(left), ManagedValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| managed_callback_outputs_equivalent(left, right))
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
        ) => {
            left_declaration == right_declaration
                && left_path.0.len() == right_path.0.len()
                && left_path
                    .0
                    .iter()
                    .zip(&right_path.0)
                    .all(|(left, right)| managed_callback_path_segments_equivalent(left, right))
        }
        _ => false,
    }
}

fn managed_callback_path_segments_equivalent(
    left: &ManagedPathSegment,
    right: &ManagedPathSegment,
) -> bool {
    match (left, right) {
        (ManagedPathSegment::Index(left), ManagedPathSegment::Index(right)) => left == right,
        (
            ManagedPathSegment::Field(left) | ManagedPathSegment::Member { member: left },
            ManagedPathSegment::Field(right) | ManagedPathSegment::Member { member: right },
        ) => left == right,
        _ => false,
    }
}

fn evaluate_runtime_expression(
    expression: &ManagedExpression,
    variables: &BTreeMap<String, ManagedValue>,
) -> Result<ManagedValue, ManagedValidationError> {
    match expression {
        ManagedExpression::Null { .. } => Ok(ManagedValue::Null),
        ManagedExpression::Boolean { value, .. } => Ok(ManagedValue::Bool(*value)),
        ManagedExpression::Number { value, .. } => Ok(ManagedValue::Number(*value)),
        ManagedExpression::String { value, .. } => Ok(ManagedValue::String(value.clone())),
        ManagedExpression::Array { values, .. } => values
            .iter()
            .map(|value| evaluate_runtime_expression(value, variables))
            .collect::<Result<Vec<_>, _>>()
            .map(ManagedValue::Array),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .map(|field| {
                Ok((
                    field.name.clone(),
                    evaluate_runtime_expression(&field.value, variables)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ManagedValidationError>>()
            .map(ManagedValue::Object),
        ManagedExpression::Reference {
            declaration, path, ..
        } => {
            let value = variables.get(declaration).ok_or_else(|| {
                ManagedValidationError::Invalid(format!(
                    "managed callback output references unknown binding `{declaration}`"
                ))
            })?;
            evaluate_runtime_reference(value, path)
        }
        ManagedExpression::Call {
            callee, arguments, ..
        } => {
            if !matches!(callee.as_str(), "mm" | "cm" | "m" | "inch" | "deg" | "rad")
                || arguments.len() != 1
            {
                return invalid(format!(
                    "managed callback output call `{callee}` is unsupported"
                ));
            }
            let ManagedValue::Number(value) =
                evaluate_runtime_expression(&arguments[0], variables)?
            else {
                return invalid(format!(
                    "managed callback output unit `{callee}` requires one number"
                ));
            };
            Ok(ManagedValue::Unit(UnitLiteral {
                unit: callee.clone(),
                value,
            }))
        }
    }
}

fn evaluate_runtime_reference(
    root: &ManagedValue,
    path: &[ManagedPathSegment],
) -> Result<ManagedValue, ManagedValidationError> {
    if let ManagedValue::Reference {
        declaration,
        path: root_path,
    } = root
    {
        let mut combined = root_path.0.clone();
        combined.extend_from_slice(path);
        return Ok(ManagedValue::Reference {
            declaration: declaration.clone(),
            path: SemanticOutputPath(combined),
        });
    }
    let mut current = root;
    for segment in path {
        current = match (current, segment) {
            (ManagedValue::Object(fields), ManagedPathSegment::Field(field)) => {
                fields.get(field).ok_or_else(|| {
                    ManagedValidationError::Invalid(format!(
                        "managed callback output field `{field}` is unavailable"
                    ))
                })?
            }
            (ManagedValue::Object(fields), ManagedPathSegment::Member { member }) => {
                fields.get(member).ok_or_else(|| {
                    ManagedValidationError::Invalid(format!(
                        "managed callback output member `{member}` is unavailable"
                    ))
                })?
            }
            (ManagedValue::Array(values), ManagedPathSegment::Index(index)) => {
                values.get(*index).ok_or_else(|| {
                    ManagedValidationError::Invalid(
                        "managed callback output index is unavailable".into(),
                    )
                })?
            }
            _ => {
                return invalid(
                    "managed callback output reference traverses a non-container value",
                );
            }
        };
    }
    Ok(current.clone())
}

fn collect_value_owned_spans(
    expression: &ManagedExpression,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    sites: &BTreeMap<&str, ManagedSpan>,
    source_digest: &str,
    output: &mut Vec<ManagedValueOwnedSpan>,
) -> Result<(), ManagedValidationError> {
    output.push(ManagedValueOwnedSpan {
        declaration: declaration.clone(),
        path: path.clone(),
        kind: expression_owned_kind(expression),
        span: expression_span(expression, sites)?,
        source_digest: source_digest.to_owned(),
    });
    match expression {
        ManagedExpression::Array { values, .. } => {
            for (index, value) in values.iter().enumerate() {
                let mut child = path.clone();
                child.0.push(ManagedPathSegment::Index(index));
                collect_value_owned_spans(
                    value,
                    declaration,
                    &child,
                    sites,
                    source_digest,
                    output,
                )?;
            }
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                let mut child = path.clone();
                child.0.push(ManagedPathSegment::Field(field.name.clone()));
                collect_value_owned_spans(
                    &field.value,
                    declaration,
                    &child,
                    sites,
                    source_digest,
                    output,
                )?;
            }
        }
        // Unit calls own one complete source value; their internal number is
        // provenance, not a separately typed property.
        ManagedExpression::Call { .. }
        | ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. }
        | ManagedExpression::String { .. }
        | ManagedExpression::Reference { .. } => {}
    }
    Ok(())
}

fn expression_owned_kind(expression: &ManagedExpression) -> ManagedOwnedSpanKind {
    if matches!(expression, ManagedExpression::Reference { .. }) {
        ManagedOwnedSpanKind::Reference
    } else {
        ManagedOwnedSpanKind::Literal
    }
}

fn expression_span(
    expression: &ManagedExpression,
    sites: &BTreeMap<&str, ManagedSpan>,
) -> Result<ManagedSpan, ManagedValidationError> {
    let site = match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Array { site, .. }
        | ManagedExpression::Object { site, .. }
        | ManagedExpression::Reference { site, .. }
        | ManagedExpression::Call { site, .. } => site,
    };
    site_span(site, sites)
}

fn site_span(
    site: &str,
    sites: &BTreeMap<&str, ManagedSpan>,
) -> Result<ManagedSpan, ManagedValidationError> {
    sites.get(site).copied().ok_or_else(|| {
        ManagedValidationError::Invalid("managed projection lost a source site".into())
    })
}

fn validate_expression<'a>(
    expression: &'a ManagedExpression,
    sites: &BTreeMap<&'a str, ManagedSourceSiteKind>,
    variables: &BTreeSet<&'a str>,
    depth: usize,
) -> Result<(), ManagedValidationError> {
    if depth > MAX_MANAGED_PATH_SEGMENTS {
        return invalid("managed expression exceeds the depth bound");
    }
    let site = match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Array { site, .. }
        | ManagedExpression::Object { site, .. }
        | ManagedExpression::Reference { site, .. }
        | ManagedExpression::Call { site, .. } => site,
    };
    require_site(site, ManagedSourceSiteKind::Value, sites)?;
    match expression {
        ManagedExpression::Number { value, .. } if !value.is_finite() => {
            return invalid("managed numeric value is not finite");
        }
        ManagedExpression::String { value, .. } => require_text(value, "string value")?,
        ManagedExpression::Array { values, .. } => {
            for value in values {
                validate_expression(value, sites, variables, depth + 1)?;
            }
        }
        ManagedExpression::Object { fields, .. } => {
            let mut names = BTreeSet::new();
            for field in fields {
                require_text(&field.name, "object field")?;
                if !names.insert(field.name.as_str()) {
                    return invalid("managed object repeats a field");
                }
                validate_comments(&field.comments)?;
                validate_expression(&field.value, sites, variables, depth + 1)?;
            }
        }
        ManagedExpression::Reference {
            declaration, path, ..
        } => {
            if !variables.contains(declaration.as_str()) {
                return invalid("managed expression has an unknown or forward reference");
            }
            validate_path(path)?;
        }
        ManagedExpression::Call {
            callee, arguments, ..
        } => {
            require_text(callee, "value call")?;
            for argument in arguments {
                validate_expression(argument, sites, variables, depth + 1)?;
            }
        }
        ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. } => {}
    }
    Ok(())
}

fn validate_reference<'a>(
    reference: &ManagedReference,
    expected_kind: ManagedSourceSiteKind,
    sites: &BTreeMap<&'a str, ManagedSourceSiteKind>,
    variables: &BTreeSet<&'a str>,
) -> Result<(), ManagedValidationError> {
    if !variables.contains(reference.declaration.as_str()) {
        return invalid("managed organization references an unknown or forward binding");
    }
    require_site(&reference.site, expected_kind, sites)?;
    validate_path(&reference.path)
}

fn validate_generated_address(
    address: &ExecutedGeneratedMemberAddress,
) -> Result<(), ManagedValidationError> {
    require_text(&address.invocation, "generated invocation")?;
    for (label, path) in [
        ("generated template", &address.template),
        ("generated member key", &address.member_key),
        ("generated output", &address.output),
    ] {
        if path.len() > MAX_MANAGED_PATH_SEGMENTS {
            return invalid(format!("{label} path is too deep"));
        }
        for segment in path {
            require_text(segment, label)?;
        }
    }
    Ok(())
}

fn validate_path(path: &[ManagedPathSegment]) -> Result<(), ManagedValidationError> {
    if path.len() > MAX_MANAGED_PATH_SEGMENTS {
        return invalid("managed semantic path is too deep");
    }
    for segment in path {
        match segment {
            ManagedPathSegment::Field(value) | ManagedPathSegment::Member { member: value } => {
                require_text(value, "managed semantic path segment")?;
            }
            ManagedPathSegment::Index(_) => {}
        }
    }
    Ok(())
}

fn validate_comments(comments: &[String]) -> Result<(), ManagedValidationError> {
    if comments.len() > MAX_MANAGED_STATEMENTS {
        return invalid("managed comment count exceeds the bound");
    }
    for comment in comments {
        require_text(comment, "comment")?;
    }
    Ok(())
}

fn require_unique_name<'a>(
    value: &'a str,
    label: &str,
    names: &mut BTreeSet<&'a str>,
) -> Result<(), ManagedValidationError> {
    require_text(value, label)?;
    if !names.insert(value) {
        return invalid(format!("managed repeats a {label} name"));
    }
    Ok(())
}

fn require_site(
    site: &str,
    expected: ManagedSourceSiteKind,
    sites: &BTreeMap<&str, ManagedSourceSiteKind>,
) -> Result<(), ManagedValidationError> {
    if sites.get(site) != Some(&expected) {
        return invalid("managed source-site kind or identity mismatch");
    }
    Ok(())
}

fn require_digest(value: &str, label: &str) -> Result<(), ManagedValidationError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(format!(
            "{label} digest is not canonical lower-case SHA-256"
        ));
    }
    Ok(())
}

fn require_text(value: &str, label: &str) -> Result<(), ManagedValidationError> {
    if value.is_empty()
        || value.len() > MAX_MANAGED_STRING_BYTES
        || value.chars().any(|character| character == '\0')
    {
        return invalid(format!("{label} is empty, oversized, or contains NUL"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ManagedValidationError> {
    Err(ManagedValidationError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refresh_authority(compiled: &mut CompiledManagedSource) {
        let ir_envelope = IrDigestEnvelope {
            format: &compiled.ir.format,
            imports: &compiled.ir.imports,
            statements: &compiled.ir.statements,
            output: &compiled.ir.output,
            source_sites: &compiled.ir.source_sites,
            source_digest: &compiled.ir.source_digest,
        };
        compiled.ir.ir_digest = intent_content_digest(
            serde_json::to_string(&ir_envelope)
                .expect("IR digest envelope")
                .as_bytes(),
        )
        .to_string();
        compiled.canonical_ir_json =
            serde_json::to_string(&compiled.ir).expect("canonical managed IR");

        compiled
            .artifact
            .ir_digest
            .clone_from(&compiled.ir.ir_digest);
        let artifact_envelope = ArtifactDigestEnvelope {
            format: &compiled.artifact.format,
            source_digest: &compiled.artifact.source_digest,
            ir_digest: &compiled.artifact.ir_digest,
            declarations: &compiled.artifact.declarations,
            generated_members: &compiled.artifact.generated_members,
            groups: &compiled.artifact.groups,
            suppressions: &compiled.artifact.suppressions,
            value_consumers: &compiled.artifact.value_consumers,
            output: &compiled.artifact.output,
        };
        compiled.artifact.artifact_digest = intent_content_digest(
            serde_json::to_string(&artifact_envelope)
                .expect("artifact digest envelope")
                .as_bytes(),
        )
        .to_string();
        compiled.canonical_artifact_json =
            serde_json::to_string(&compiled.artifact).expect("canonical executed artifact");
    }

    fn minimal_compiled_output() -> CompiledManagedSource {
        let normalized_source = "output".to_owned();
        let source_digest = intent_content_digest(normalized_source.as_bytes()).to_string();
        let value_site = "0".repeat(64);
        let output = ManagedExpression::Object {
            fields: vec![ManagedObjectField {
                name: "answer".into(),
                value: ManagedExpression::Number {
                    value: 42.0,
                    site: value_site.clone(),
                },
                comments: Vec::new(),
            }],
            site: value_site.clone(),
        };
        let mut compiled = CompiledManagedSource {
            input_source_digest: source_digest.clone(),
            normalized_source,
            ir: ManagedSketchIr {
                format: MANAGED_SKETCH_IR_FORMAT.into(),
                imports: Vec::new(),
                statements: Vec::new(),
                output,
                source_sites: vec![ManagedSourceSite {
                    id: value_site,
                    kind: ManagedSourceSiteKind::Value,
                    span: ManagedSourceSpan { start: 0, end: 6 },
                    source_digest: source_digest.clone(),
                }],
                source_digest: source_digest.clone(),
                ir_digest: String::new(),
            },
            artifact: ExecutedSketchArtifact {
                format: EXECUTED_SKETCH_ARTIFACT_FORMAT.into(),
                source_digest,
                ir_digest: String::new(),
                declarations: Vec::new(),
                generated_members: Vec::new(),
                groups: Vec::new(),
                suppressions: Vec::new(),
                value_consumers: Vec::new(),
                output: ManagedValue::Object(BTreeMap::from([(
                    "answer".into(),
                    ManagedValue::Number(42.0),
                )])),
                artifact_digest: String::new(),
            },
            canonical_ir_json: String::new(),
            canonical_artifact_json: String::new(),
        };
        refresh_authority(&mut compiled);
        compiled
    }

    fn field(name: &str, value: ManagedExpression) -> ManagedObjectField {
        ManagedObjectField {
            name: name.into(),
            value,
            comments: Vec::new(),
        }
    }

    fn object(fields: impl IntoIterator<Item = ManagedObjectField>) -> ManagedExpression {
        ManagedExpression::Object {
            fields: fields.into_iter().collect(),
            site: String::new(),
        }
    }

    fn declaration(
        variable: &str,
        symbol: &str,
        builder_path: [&str; 2],
        arguments: ManagedExpression,
    ) -> ManagedStatement {
        ManagedStatement::Declaration {
            variable: variable.into(),
            symbol: symbol.into(),
            builder_path: builder_path.into_iter().map(str::to_owned).collect(),
            patch: None,
            arguments,
            site: String::new(),
            comments: Vec::new(),
        }
    }

    fn profile_offset_projection_fixture(
        aggregate: ProfileOffsetHelperKind,
    ) -> CompiledManagedSource {
        let helper_arguments = object([field(
            "spans",
            ManagedExpression::Array {
                values: Vec::new(),
                site: String::new(),
            },
        )]);
        let root_arguments = object([field(
            "sources",
            ManagedExpression::Array {
                values: vec![ManagedExpression::Reference {
                    declaration: "aggregate_variable".into(),
                    path: aggregate.result_path(),
                    site: String::new(),
                }],
                site: String::new(),
            },
        )]);
        CompiledManagedSource {
            input_source_digest: String::new(),
            normalized_source: String::new(),
            ir: ManagedSketchIr {
                format: MANAGED_SKETCH_IR_FORMAT.into(),
                imports: Vec::new(),
                statements: vec![
                    declaration(
                        "aggregate_variable",
                        "source-visible-helper",
                        ["aggregate", aggregate.builder()],
                        helper_arguments,
                    ),
                    declaration(
                        "operation_variable",
                        "source-visible-root",
                        ["operation", "profileOffset"],
                        root_arguments,
                    ),
                    ManagedStatement::Group {
                        name: "Canvas additions".into(),
                        declarations: vec![ManagedReference {
                            declaration: "aggregate_variable".into(),
                            path: Vec::new(),
                            site: String::new(),
                        }],
                        site: String::new(),
                        comments: Vec::new(),
                    },
                    ManagedStatement::Suppression {
                        target: ManagedReference {
                            declaration: "aggregate_variable".into(),
                            path: Vec::new(),
                            site: String::new(),
                        },
                        site: String::new(),
                        comments: Vec::new(),
                    },
                ],
                output: object([]),
                source_sites: Vec::new(),
                source_digest: String::new(),
                ir_digest: String::new(),
            },
            artifact: ExecutedSketchArtifact {
                format: EXECUTED_SKETCH_ARTIFACT_FORMAT.into(),
                source_digest: String::new(),
                ir_digest: String::new(),
                declarations: Vec::new(),
                generated_members: Vec::new(),
                groups: Vec::new(),
                suppressions: Vec::new(),
                value_consumers: Vec::new(),
                output: ManagedValue::Object(BTreeMap::new()),
                artifact_digest: String::new(),
            },
            canonical_ir_json: String::new(),
            canonical_artifact_json: String::new(),
        }
    }

    #[test]
    fn compiler_handoff_is_bounded_before_json_decode() {
        let oversized = " ".repeat(MANAGED_WIRE_LIMIT + 1);
        assert!(matches!(
            CompiledManagedSource::from_json(&oversized),
            Err(ManagedValidationError::ResourceLimit { .. })
        ));
    }

    #[test]
    fn prior_managed_wire_formats_are_rejected_exactly() {
        let compiled = |ir_format: &str, artifact_format: &str| CompiledManagedSource {
            input_source_digest: intent_content_digest(b"").to_string(),
            normalized_source: String::new(),
            ir: ManagedSketchIr {
                format: ir_format.into(),
                imports: Vec::new(),
                statements: Vec::new(),
                output: ManagedExpression::Null {
                    site: String::new(),
                },
                source_sites: Vec::new(),
                source_digest: "0".repeat(64),
                ir_digest: "0".repeat(64),
            },
            artifact: ExecutedSketchArtifact {
                format: artifact_format.into(),
                source_digest: "0".repeat(64),
                ir_digest: "0".repeat(64),
                declarations: Vec::new(),
                generated_members: Vec::new(),
                groups: Vec::new(),
                suppressions: Vec::new(),
                value_consumers: Vec::new(),
                output: ManagedValue::Null,
                artifact_digest: "0".repeat(64),
            },
            canonical_ir_json: String::new(),
            canonical_artifact_json: String::new(),
        };

        for prior in [
            "geosolve-managed-sketch-ir-v1",
            "geosolve-managed-sketch-ir-v2",
        ] {
            assert_eq!(
                compiled(prior, EXECUTED_SKETCH_ARTIFACT_FORMAT).validate(),
                Err(ManagedValidationError::Invalid(
                    "unsupported managed sketch IR format".into(),
                )),
            );
        }
        for prior in [
            "geosolve-executed-sketch-artifact-v1",
            "geosolve-executed-sketch-artifact-v2",
        ] {
            assert_eq!(
                compiled(MANAGED_SKETCH_IR_FORMAT, prior).validate(),
                Err(ManagedValidationError::Invalid(
                    "unsupported executed sketch artifact format".into(),
                )),
            );
        }

        let mut prior_shape = serde_json::to_value(compiled(
            MANAGED_SKETCH_IR_FORMAT,
            EXECUTED_SKETCH_ARTIFACT_FORMAT,
        ))
        .unwrap();
        prior_shape["artifact"]["edit_lenses"] = serde_json::json!([]);
        assert!(matches!(
            CompiledManagedSource::from_json(&serde_json::to_string(&prior_shape).unwrap()),
            Err(ManagedValidationError::InvalidJson(message))
                if message.contains("unknown field `edit_lenses`")
        ));
    }

    #[test]
    fn legacy_direct_family_alias_is_rejected_even_in_a_digest_consistent_envelope() {
        let mut compiled = CompiledManagedSource::from_json(include_str!(
            "../assets/demos/neon-manifold.compiled.json"
        ))
        .expect("checked-in V3 compiler envelope");
        let declaration = compiled
            .ir
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    builder_path,
                    patch: None,
                    ..
                } if builder_path == &["geometry", "segment"] => {
                    Some((symbol.clone(), builder_path))
                }
                _ => None,
            })
            .expect("line-fillet contains a named segment");
        declaration.1[1] = "line".into();
        compiled
            .artifact
            .declarations
            .iter_mut()
            .find(|candidate| candidate.declaration == declaration.0)
            .expect("executed segment declaration")
            .family = "geometry.line".into();
        refresh_authority(&mut compiled);

        assert_eq!(
            compiled.validate(),
            Err(ManagedValidationError::Invalid(
                "unsupported named managed declaration family `geometry.line`".into(),
            )),
        );
    }

    #[test]
    fn m90_f002_retired_transport_property_is_rejected_in_authenticated_envelope() {
        for retired in [
            "recipe",
            "inputs",
            "fields",
            "values",
            "results",
            "operationOutputs",
            "outputs",
            "editLens",
        ] {
            let mut compiled = CompiledManagedSource::from_json(include_str!(
                "../assets/demos/neon-manifold.compiled.json"
            ))
            .expect("checked-in V3 compiler envelope");
            let symbol = compiled
                .ir
                .statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ManagedStatement::Declaration {
                        symbol,
                        builder_path,
                        patch: None,
                        arguments: ManagedExpression::Object { fields, .. },
                        ..
                    } if builder_path == &["geometry", "segment"] => {
                        fields
                            .iter_mut()
                            .find(|field| field.name == "start")
                            .expect("segment start argument")
                            .name = retired.into();
                        Some(symbol.clone())
                    }
                    _ => None,
                })
                .expect("neon manifold contains a named segment");
            for consumer in &mut compiled.artifact.value_consumers {
                if matches!(
                    &consumer.target,
                    ExecutedConsumerTarget::Declaration { declaration, .. }
                        if declaration == &symbol
                ) && matches!(
                    consumer.property.first(),
                    Some(ManagedPathSegment::Field(field)) if field == "start"
                ) {
                    consumer.property[0] = ManagedPathSegment::Field(retired.into());
                }
            }
            refresh_authority(&mut compiled);

            assert_eq!(
                compiled.validate(),
                Err(ManagedValidationError::Invalid(format!(
                    "managed named declaration arguments cannot contain retired transport property `{retired}`"
                ))),
                "{retired}",
            );
        }
    }

    #[test]
    fn m90_f002_reachable_binding_cannot_hide_a_retired_transport_property() {
        let mut compiled = CompiledManagedSource::from_json(include_str!(
            "../assets/demos/neon-manifold.compiled.json"
        ))
        .expect("checked-in V3 compiler envelope");
        let first_declaration = compiled
            .ir
            .statements
            .iter()
            .position(|statement| matches!(statement, ManagedStatement::Declaration { .. }))
            .expect("neon manifold contains a named declaration");
        let hidden_site = compiled
            .ir
            .source_sites
            .iter()
            .find(|site| site.kind == ManagedSourceSiteKind::Value)
            .expect("neon manifold contains a value source site")
            .id
            .clone();
        compiled.ir.statements.insert(
            first_declaration,
            ManagedStatement::Binding {
                variable: "legacyPayload".into(),
                value: ManagedExpression::Object {
                    fields: vec![field(
                        "recipe",
                        ManagedExpression::Array {
                            values: Vec::new(),
                            site: hidden_site.clone(),
                        },
                    )],
                    site: hidden_site.clone(),
                },
                comments: Vec::new(),
            },
        );
        let declaration = compiled
            .ir
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    builder_path,
                    patch: None,
                    arguments: ManagedExpression::Object { fields, .. },
                    ..
                } if builder_path == &["geometry", "segment"] => Some(fields),
                _ => None,
            })
            .expect("neon manifold contains a named segment");
        declaration.push(field(
            "metadata",
            ManagedExpression::Reference {
                declaration: "legacyPayload".into(),
                path: Vec::new(),
                site: hidden_site,
            },
        ));
        refresh_authority(&mut compiled);

        assert_eq!(
            compiled.validate(),
            Err(ManagedValidationError::Invalid(
                "managed named declaration arguments cannot contain retired transport property `recipe`"
                    .into(),
            )),
        );
    }

    #[test]
    fn managed_callback_output_is_mandatory_digest_bound_and_independently_evaluated() {
        let compiled = minimal_compiled_output();
        compiled
            .validate()
            .expect("valid callback output authority");

        for owner in ["ir", "artifact"] {
            let mut missing = serde_json::to_value(&compiled).expect("compiler envelope JSON");
            missing[owner]
                .as_object_mut()
                .expect("wire owner object")
                .remove("output");
            assert!(matches!(
                CompiledManagedSource::from_json(
                    &serde_json::to_string(&missing).expect("missing-output envelope")
                ),
                Err(ManagedValidationError::InvalidJson(message))
                    if message.contains("missing field `output`")
            ));
        }

        let mut unauthenticated = compiled.clone();
        unauthenticated.artifact.output = ManagedValue::Bool(false);
        assert_eq!(
            unauthenticated.validate(),
            Err(ManagedValidationError::Invalid(
                "executed artifact digest mismatch".into(),
            )),
        );

        let mut false_execution = compiled;
        false_execution.artifact.output = ManagedValue::Bool(false);
        refresh_authority(&mut false_execution);
        assert_eq!(
            false_execution.validate(),
            Err(ManagedValidationError::Invalid(
                "executed callback output differs from managed IR evaluation".into(),
            )),
        );
    }

    #[test]
    fn managed_callback_output_matches_runtime_member_classification_by_property_key() {
        let lexical = ManagedValue::Reference {
            declaration: SemanticSymbol("corners".into()),
            path: SemanticOutputPath(vec![
                ManagedPathSegment::Field("fillets".into()),
                ManagedPathSegment::Field("lowerLeft".into()),
            ]),
        };
        let executed = ManagedValue::Reference {
            declaration: SemanticSymbol("corners".into()),
            path: SemanticOutputPath(vec![
                ManagedPathSegment::Field("fillets".into()),
                ManagedPathSegment::Member {
                    member: "lowerLeft".into(),
                },
            ]),
        };

        assert!(managed_callback_outputs_equivalent(&lexical, &executed));
        assert!(!managed_callback_outputs_equivalent(
            &lexical,
            &ManagedValue::Reference {
                declaration: SemanticSymbol("corners".into()),
                path: SemanticOutputPath(vec![
                    ManagedPathSegment::Field("fillets".into()),
                    ManagedPathSegment::Member {
                        member: "upperRight".into(),
                    },
                ]),
            },
        ));
    }

    #[test]
    fn managed_program_outputs_follow_callback_references_not_declaration_leaves() {
        let value = ManagedValue::Object(BTreeMap::from([
            (
                "whole".into(),
                ManagedValue::Reference {
                    declaration: SemanticSymbol("patch".into()),
                    path: SemanticOutputPath(vec![ManagedPathSegment::Field("segment".into())]),
                },
            ),
            (
                "nested".into(),
                ManagedValue::Object(BTreeMap::from([(
                    "span".into(),
                    ManagedValue::Reference {
                        declaration: SemanticSymbol("line".into()),
                        path: SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
                    },
                )])),
            ),
        ]));
        let mut outputs = Vec::new();

        collect_callback_outputs(None, &value, "", &mut outputs).unwrap();

        assert_eq!(
            outputs,
            vec![
                ManagedOutput {
                    name: "nested.span".into(),
                    declaration: SemanticSymbol("line".into()),
                    path: SemanticOutputPath(vec![ManagedPathSegment::Field("span".into())]),
                },
                ManagedOutput {
                    name: "whole".into(),
                    declaration: SemanticSymbol("patch".into()),
                    path: SemanticOutputPath(vec![ManagedPathSegment::Field("segment".into())]),
                },
            ],
        );
    }

    #[test]
    fn canonical_number_encoding_preserves_rust_f64_shape() {
        let expression = ManagedExpression::Number {
            value: -0.0,
            site: "0".repeat(64),
        };
        assert_eq!(
            serde_json::to_string(&expression).unwrap(),
            format!(
                r#"{{"kind":"number","value":-0.0,"site":"{}"}}"#,
                "0".repeat(64)
            )
        );
    }

    #[test]
    fn profile_offset_closure_projection_uses_exact_ir_semantics_for_both_aggregate_kinds() {
        for aggregate in [
            ProfileOffsetHelperKind::OpenChain,
            ProfileOffsetHelperKind::ClosedProfile,
        ] {
            let compiled = profile_offset_projection_fixture(aggregate);
            assert_eq!(
                project_source_declaration_closures(&compiled),
                [ManagedSourceDeclarationClosure {
                    kind: ManagedSourceDeclarationClosureKind::ProfileOffset,
                    root: SemanticSymbol("source-visible-root".into()),
                    helpers: vec![SemanticSymbol("source-visible-helper".into())],
                }],
                "groups and suppressions are lifecycle metadata, not semantic consumers",
            );
        }
    }

    #[test]
    fn profile_offset_closure_projection_keeps_shared_or_indirect_aggregates_independent() {
        let mut binding_consumer =
            profile_offset_projection_fixture(ProfileOffsetHelperKind::OpenChain);
        binding_consumer.ir.statements.insert(
            1,
            ManagedStatement::Binding {
                variable: "shared_alias".into(),
                value: ManagedExpression::Reference {
                    declaration: "aggregate_variable".into(),
                    path: vec![ManagedPathSegment::Field("chain".into())],
                    site: String::new(),
                },
                comments: Vec::new(),
            },
        );
        assert!(project_source_declaration_closures(&binding_consumer).is_empty());

        let mut second_declaration =
            profile_offset_projection_fixture(ProfileOffsetHelperKind::OpenChain);
        second_declaration.ir.statements.insert(
            2,
            declaration(
                "second_consumer",
                "second-consumer",
                ["operation", "profileOffset"],
                object([field(
                    "source",
                    ManagedExpression::Reference {
                        declaration: "aggregate_variable".into(),
                        path: vec![ManagedPathSegment::Field("chain".into())],
                        site: String::new(),
                    },
                )]),
            ),
        );
        assert!(project_source_declaration_closures(&second_declaration).is_empty());
    }

    #[test]
    fn profile_offset_closure_projection_requires_exact_named_input_and_result_reference() {
        let mut wrong_input = profile_offset_projection_fixture(ProfileOffsetHelperKind::OpenChain);
        let ManagedStatement::Declaration { arguments, .. } = &mut wrong_input.ir.statements[1]
        else {
            panic!("Profile Offset root declaration")
        };
        let ManagedExpression::Array { values, .. } =
            object_field(arguments, "sources").expect("Profile Offset sources")
        else {
            panic!("Profile Offset source array")
        };
        let source = values[0].clone();
        let ManagedExpression::Object {
            fields: argument_fields,
            ..
        } = arguments
        else {
            panic!("Profile Offset arguments")
        };
        argument_fields
            .iter_mut()
            .find(|field| field.name == "sources")
            .expect("sources field")
            .name = "source".into();
        argument_fields
            .iter_mut()
            .find(|field| field.name == "source")
            .expect("source field")
            .value = ManagedExpression::Array {
            values: vec![source],
            site: String::new(),
        };
        assert!(project_source_declaration_closures(&wrong_input).is_empty());

        let mut wrong_path = profile_offset_projection_fixture(ProfileOffsetHelperKind::OpenChain);
        let ManagedStatement::Declaration { arguments, .. } = &mut wrong_path.ir.statements[1]
        else {
            panic!("Profile Offset root declaration")
        };
        let ManagedExpression::Object { fields, .. } = arguments else {
            panic!("Profile Offset arguments")
        };
        let ManagedExpression::Array { values, .. } = &mut fields
            .iter_mut()
            .find(|field| field.name == "sources")
            .expect("sources field")
            .value
        else {
            panic!("Profile Offset sources")
        };
        let ManagedExpression::Reference { path, .. } = &mut values[0] else {
            panic!("source reference")
        };
        path.clear();
        assert!(project_source_declaration_closures(&wrong_path).is_empty());
    }
}
