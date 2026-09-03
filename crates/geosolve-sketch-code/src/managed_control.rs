// SPDX-License-Identifier: GPL-3.0-or-later

//! Transient source-control provenance for managed code projects.
//!
//! Controls are derived from an already validated project and its exact
//! equation-free expansion. They are deliberately absent from artifact and
//! session wire formats: source remains durable authority, while this module
//! reconstructs reverse routes and generation evidence whenever required.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::{
    IntentFieldChoices, IntentLiteralSchema, IntentProjectionPath, IntentProjectionPathSegment,
    intent_content_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::template_argument_bindings_with_paths;
use crate::{
    AuthoringDeclaration, CodeAuthoringArgumentKind, CodeAuthoringCollectionMember,
    CodeAuthoringDeclarationDescriptor, CodeAuthoringDeclarationKind, CodeAuthoringDynamicChildren,
    CodeProject, CodeProjectError, CompiledManagedSource, ExecutedConsumerTarget,
    ExpandedCodeProject, FeatureKind, GeneratedMemberAddress, GeneratedMemberIdentity,
    ManagedExpression, ManagedOwnedSpanKind, ManagedPathSegment, ManagedSketchMutation,
    ManagedSpan, ManagedStatement, ManagedValue, ManagedValueMutation, PatchModuleArtifact,
    ProjectKey, SemanticOutputPath, SemanticSymbol, TemplateBinding, UnitLiteral,
    ValidatedPatchModuleArtifact, code_authoring_family, resolve_code_authoring_declaration,
};

/// Maximum transient source entries in one manifest. This independent bound
/// is enforced after the V3 compiler envelope has passed its own wire limits.
pub const MANAGED_CONTROL_LIMIT: usize = 8_192;

/// Independent bound on source-to-consumer fan-out.
pub const MANAGED_CONTROL_CONSUMER_LIMIT: usize = 65_536;

/// Stable semantic identity of one managed source value. It deliberately
/// excludes byte spans, source revisions, and generated allocations.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManagedControlId(pub String);

/// Numeric category admitted by a managed control.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedControlNumberKind {
    Real,
    Integer,
    Natural,
}

/// One finite numeric endpoint in an authoritative control schema.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlBound {
    pub value: f64,
    pub inclusive: bool,
}

/// Closed value schema enforced before an exact source rewrite is admitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedControlSchema {
    Number {
        number: ManagedControlNumberKind,
        minimum: Option<ManagedControlBound>,
        maximum: Option<ManagedControlBound>,
    },
    Unit {
        unit: String,
        number: ManagedControlNumberKind,
        minimum: Option<ManagedControlBound>,
        maximum: Option<ManagedControlBound>,
    },
    Boolean,
    Choice {
        choices: Vec<String>,
    },
    Text,
}

/// Exact parser-owned source location behind one manifest row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlSource {
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
    pub kind: ManagedOwnedSpanKind,
    pub span: ManagedSpan,
    pub source_text: String,
}

/// Navigation destination published for a source reference which is not
/// itself editable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlNavigation {
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
}

/// Why an authenticated managed value has no edit capability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedControlReadOnlyReason {
    Structure,
    Reference,
    StructuralIdentity,
    SolverInstance,
    Null,
    /// A semantic field which is not present has no parser-owned value span,
    /// so current manifests never synthesize a row carrying this reason.
    Absent,
    IncompatibleSchemas,
    UnprovenTransform,
}

/// Semantic property which consumes one source value.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedControlConsumerTarget {
    Declaration {
        declaration: SemanticSymbol,
        family: String,
    },
    Generated {
        address: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        artifact_digest: String,
        family: String,
    },
}

/// One complete source-to-semantic-property impact edge.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlConsumer {
    pub target: ManagedControlConsumerTarget,
    pub property: SemanticOutputPath,
}

/// Exact-CAS capability for one editable source leaf. `id` is stable while
/// the remaining fields authenticate the exact project/source/generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlToken {
    pub id: ManagedControlId,
    pub project: ProjectKey,
    pub project_digest: String,
    pub source_digest: String,
    pub declaration: SemanticSymbol,
    pub path: SemanticOutputPath,
    pub expected: ManagedValue,
    pub generation_digest: String,
    pub authentication: String,
}

/// Edit capability or a typed read-only classification for one source value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "access", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedControlAccess {
    Editable {
        token: ManagedControlToken,
    },
    ReadOnly {
        reason: ManagedControlReadOnlyReason,
        navigation: Option<ManagedControlNavigation>,
    },
}

/// One transient manifest row. Aggregate/reference rows remain present so a
/// caller never has to guess why a source value lacks an edit affordance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControl {
    pub id: ManagedControlId,
    pub source: ManagedControlSource,
    pub value: ManagedValue,
    pub schema: Option<ManagedControlSchema>,
    pub consumers: Vec<ManagedControlConsumer>,
    pub access: ManagedControlAccess,
}

impl ManagedControl {
    #[must_use]
    pub fn token(&self) -> Option<&ManagedControlToken> {
        match &self.access {
            ManagedControlAccess::Editable { token } => Some(token),
            ManagedControlAccess::ReadOnly { .. } => None,
        }
    }
}

/// Bounded, transient reverse-provenance view of one exact expansion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlManifest {
    pub project: ProjectKey,
    pub project_digest: String,
    pub source_digest: String,
    pub expansion_digest: String,
    pub controls: Vec<ManagedControl>,
}

impl ManagedControlManifest {
    #[must_use]
    pub fn control(&self, id: &ManagedControlId) -> Option<&ManagedControl> {
        self.controls
            .binary_search_by(|control| control.id.cmp(id))
            .ok()
            .map(|index| &self.controls[index])
    }

    pub fn editable(&self) -> impl Iterator<Item = &ManagedControl> {
        self.controls
            .iter()
            .filter(|control| control.token().is_some())
    }

    /// Finds every control which owns one exact generated semantic property.
    pub fn controls_for_generated<'a>(
        &'a self,
        address: &'a GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        property: &'a SemanticOutputPath,
    ) -> impl Iterator<Item = &'a ManagedControl> + 'a {
        self.controls.iter().filter(move |control| {
            control.consumers.iter().any(|consumer| {
                consumer.property == *property
                    && matches!(
                        &consumer.target,
                        ManagedControlConsumerTarget::Generated {
                            address: candidate,
                            identity: candidate_identity,
                            ..
                        } if candidate == address && *candidate_identity == identity
                    )
            })
        })
    }
}

/// One freshly derived manifest tied by Rust borrows to the exact project and
/// expansion that authenticated it. Callers can inspect and apply through the
/// same bounded snapshot without a second provenance derivation, while the
/// borrowed authorities cannot change underneath it.
#[derive(Debug)]
pub struct ManagedControlAuthority<'a> {
    _project: &'a CodeProject,
    _expansion: &'a ExpandedCodeProject,
    manifest: ManagedControlManifest,
}

impl ManagedControlAuthority<'_> {
    #[must_use]
    pub fn manifest(&self) -> &ManagedControlManifest {
        &self.manifest
    }

    /// Prepares the exact semantic value mutation authorized by this freshly
    /// derived manifest. Source printing and execution deliberately remain a
    /// browser or pinned-Deno responsibility; Rust validates the resulting
    /// compiler receipt through the ordinary prepared-mutation boundary.
    ///
    /// # Errors
    ///
    /// Returns a stale/foreign/read-only/typing/resource failure without
    /// changing the borrowed project or expansion.
    pub fn prepare_mutation(
        &self,
        batch: &ManagedControlEditBatch,
    ) -> Result<ManagedSketchMutation, ManagedControlError> {
        validate_control_batch_shape(batch)?;
        prepare_managed_control_mutation_against_manifest(&self.manifest, batch)
    }
}

/// Derives a borrow-scoped managed-control authority for one inspect/apply
/// operation.
///
/// # Errors
///
/// Returns the ordinary manifest project, provenance, encoding, or resource
/// failure before publishing an authority snapshot.
pub fn managed_control_authority<'a>(
    project: &'a CodeProject,
    expansion: &'a ExpandedCodeProject,
) -> Result<ManagedControlAuthority<'a>, ManagedControlError> {
    let manifest = managed_control_manifest(project, expansion)?;
    Ok(ManagedControlAuthority {
        _project: project,
        _expansion: expansion,
        manifest,
    })
}

/// One requested replacement authenticated by a manifest token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlEdit {
    pub token: ManagedControlToken,
    pub value: ManagedValue,
}

/// One atomic, unordered exact-CAS source transaction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedControlEditBatch {
    pub edits: Vec<ManagedControlEdit>,
}

impl ManagedControlEditBatch {
    #[must_use]
    pub fn new(edits: impl IntoIterator<Item = ManagedControlEdit>) -> Self {
        Self {
            edits: edits.into_iter().collect(),
        }
    }
}

/// Manifest derivation or exact-CAS batch failure.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ManagedControlError {
    #[error("managed control resource `{resource}` has {actual} entries; the limit is {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("managed control expansion is not authenticated for this project: {0}")]
    ForeignExpansion(String),
    #[error("managed control batch is empty")]
    EmptyBatch,
    #[error("managed control batch repeats `{0}`")]
    DuplicateControl(String),
    #[error("managed controls `{first}` and `{second}` own overlapping source spans")]
    OverlappingControls { first: String, second: String },
    #[error("managed control `{0}` is unavailable")]
    UnknownControl(String),
    #[error("managed control `{0}` is read-only")]
    ReadOnlyControl(String),
    #[error("managed control token for `{0}` is stale or foreign")]
    StaleToken(String),
    #[error("managed control replacement for `{control}` is invalid: {message}")]
    InvalidReplacement { control: String, message: String },
    #[error("managed control encoding failed: {0}")]
    Encoding(String),
    #[error(transparent)]
    Project(#[from] CodeProjectError),
}

type OwnedSpanIndex<'a> = BTreeMap<
    &'a SemanticSymbol,
    BTreeMap<&'a SemanticOutputPath, &'a crate::ManagedValueOwnedSpan>,
>;

fn owned_span_index(project: &CodeProject) -> Result<OwnedSpanIndex<'_>, ManagedControlError> {
    let mut index = OwnedSpanIndex::new();
    for owned in &project.managed.value_owned_spans {
        if owned.source_digest != project.managed.source_digest {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "source path `{}` has stale lexical ownership",
                path_text(&owned.declaration, &owned.path)
            )));
        }
        let replaced = index
            .entry(&owned.declaration)
            .or_default()
            .insert(&owned.path, owned);
        if replaced.is_some() {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "source path `{}` repeats lexical ownership",
                path_text(&owned.declaration, &owned.path)
            )));
        }
    }
    Ok(index)
}

#[derive(Clone, Debug)]
struct ManagedValueSiteOwner {
    declaration: SemanticSymbol,
    path: SemanticOutputPath,
    span: ManagedSpan,
}

fn managed_value_site_index(
    compiled: &CompiledManagedSource,
) -> Result<BTreeMap<String, ManagedValueSiteOwner>, ManagedControlError> {
    let spans = compiled
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
    let mut index = BTreeMap::new();
    for statement in &compiled.ir.statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => collect_value_sites(
                value,
                &SemanticSymbol(variable.clone()),
                &SemanticOutputPath::default(),
                &spans,
                &mut index,
            )?,
            ManagedStatement::Declaration {
                symbol, arguments, ..
            } => collect_value_sites(
                arguments,
                &SemanticSymbol(symbol.clone()),
                &SemanticOutputPath::default(),
                &spans,
                &mut index,
            )?,
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {}
        }
    }
    Ok(index)
}

fn collect_value_sites(
    expression: &ManagedExpression,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    spans: &BTreeMap<&str, ManagedSpan>,
    output: &mut BTreeMap<String, ManagedValueSiteOwner>,
) -> Result<(), ManagedControlError> {
    let site = managed_expression_site(expression);
    let span = spans.get(site).copied().ok_or_else(|| {
        ManagedControlError::ForeignExpansion(format!(
            "runtime value site `{site}` has no authenticated source span"
        ))
    })?;
    if output
        .insert(
            site.to_owned(),
            ManagedValueSiteOwner {
                declaration: declaration.clone(),
                path: path.clone(),
                span,
            },
        )
        .is_some()
    {
        return Err(ManagedControlError::ForeignExpansion(format!(
            "runtime value site `{site}` is reused by multiple expressions"
        )));
    }
    match expression {
        ManagedExpression::Array { values, .. } => {
            for (index, value) in values.iter().enumerate() {
                let mut child = path.clone();
                child.0.push(ManagedPathSegment::Index(index));
                collect_value_sites(value, declaration, &child, spans, output)?;
            }
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                let mut child = path.clone();
                child.0.push(ManagedPathSegment::Field(field.name.clone()));
                collect_value_sites(&field.value, declaration, &child, spans, output)?;
            }
        }
        // A unit call is one semantic managed value. Its argument sites remain
        // indexed for complete source-site uniqueness, but cannot authenticate
        // a control edge because they do not own a projected value span.
        ManagedExpression::Call { arguments, .. } => {
            for argument in arguments {
                collect_value_sites(argument, declaration, path, spans, output)?;
            }
        }
        ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. }
        | ManagedExpression::String { .. }
        | ManagedExpression::Reference { .. } => {}
    }
    Ok(())
}

fn managed_expression_site(expression: &ManagedExpression) -> &str {
    match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Array { site, .. }
        | ManagedExpression::Object { site, .. }
        | ManagedExpression::Reference { site, .. }
        | ManagedExpression::Call { site, .. } => site,
    }
}

/// Derives one transient control manifest from an exact project/expansion
/// pair. No artifact, managed source, session, or native authority changes.
///
/// # Errors
///
/// Returns a project, cross-provenance, encoding, or resource-bound failure.
#[allow(
    clippy::too_many_lines,
    reason = "one audit pass keeps source ownership, schemas, fan-out, and token evidence adjacent"
)]
pub fn managed_control_manifest(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<ManagedControlManifest, ManagedControlError> {
    project.validate()?;
    validate_expansion_cross_links(project, expansion)?;
    let project_digest = project_authority_digest(project)?;
    let compiled = project.managed.compiled.as_deref().ok_or_else(|| {
        ManagedControlError::ForeignExpansion(
            "managed V3 control authority requires its complete compiler envelope".into(),
        )
    })?;
    managed_control_manifest_compiled(project, expansion, &project_digest, compiled)
}
#[allow(
    clippy::too_many_lines,
    reason = "one authenticated manifest pass keeps source sites, runtime consumers, access policy, and ordering visibly joined"
)]
fn managed_control_manifest_compiled(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    project_digest: &str,
    compiled: &CompiledManagedSource,
) -> Result<ManagedControlManifest, ManagedControlError> {
    let owned_spans = owned_span_index(project)?;
    let value_sites = managed_value_site_index(compiled)?;
    let artifact_plans = artifact_plans(project)?;
    let mut artifact_plan_index = BTreeMap::new();
    for plan in &artifact_plans {
        if artifact_plan_index
            .insert(&plan.declaration.symbol, plan)
            .is_some()
        {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "invocation `{}` repeats one artifact plan",
                plan.declaration.symbol.0
            )));
        }
    }

    let mut consumer_map =
        BTreeMap::<(SemanticSymbol, SemanticOutputPath), BTreeSet<ManagedControlConsumer>>::new();
    let mut schema_map =
        BTreeMap::<(SemanticSymbol, SemanticOutputPath), Vec<ManagedControlSchema>>::new();
    let mut consumer_edges = 0_usize;
    for executed in &compiled.artifact.value_consumers {
        let owner = value_sites.get(&executed.value_site).ok_or_else(|| {
            ManagedControlError::ForeignExpansion(format!(
                "runtime consumer site `{}` has no exact IR value owner",
                executed.value_site
            ))
        })?;
        let lexical_owned = owned_spans
            .get(&owner.declaration)
            .and_then(|paths| paths.get(&owner.path))
            .copied()
            .ok_or_else(|| {
                ManagedControlError::ForeignExpansion(format!(
                    "runtime consumer site `{}` has no projected source owner",
                    executed.value_site
                ))
            })?;
        if lexical_owned.span != owner.span {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "runtime consumer site `{}` does not own its complete managed value span",
                executed.value_site
            )));
        }
        let source_value = managed_source_value(project, &owner.declaration, &owner.path)
            .ok_or_else(|| {
                ManagedControlError::ForeignExpansion(format!(
                    "runtime consumer site `{}` resolves to an absent managed value",
                    executed.value_site
                ))
            })?;
        let property = SemanticOutputPath(executed.property.clone());
        let (consumer, schema) = match &executed.target {
            ExecutedConsumerTarget::Declaration {
                declaration,
                family,
            } => {
                let symbol = SemanticSymbol(declaration.clone());
                let target = project
                    .managed
                    .program
                    .declarations
                    .iter()
                    .find(|candidate| candidate.symbol == symbol)
                    .ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime declaration consumer `{declaration}` has no managed owner"
                        ))
                    })?;
                if target.builder_path.join(".") != *family
                    || !expansion
                        .declaration_provenance
                        .values()
                        .any(|candidate| candidate == &symbol)
                {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime declaration consumer `{declaration}` is absent from exact expansion provenance"
                    )));
                }
                let target_value =
                    managed_value_at_path(&target.arguments, &property).ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime declaration consumer `{declaration}` has no property `{}`",
                            path_suffix_text(&property)
                        ))
                    })?;
                let (expected_declaration, expected_path, _) =
                    scalar_source_owner(project, &target.symbol, &property, target_value);
                if expected_declaration != owner.declaration || expected_path != owner.path {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime declaration consumer `{declaration}` does not match its exact IR value origin"
                    )));
                }
                (
                    ManagedControlConsumer {
                        target: ManagedControlConsumerTarget::Declaration {
                            declaration: symbol,
                            family: family.clone(),
                        },
                        property: property.clone(),
                    },
                    named_schema(target, &property, source_value)?,
                )
            }
            ExecutedConsumerTarget::Generated { address, family } => {
                let invocation = SemanticSymbol(address.invocation.clone());
                let plan = artifact_plan_index
                    .get(&invocation)
                    .copied()
                    .ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime generated consumer `{}` has no pinned patch owner",
                            address.invocation
                        ))
                    })?;
                let template = plan
                    .artifact
                    .artifact()
                    .templates
                    .iter()
                    .find(|template| template.path == address.template)
                    .ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime generated consumer `{}` has no exact template",
                            address.invocation
                        ))
                    })?;
                if template.declaration_family != *family {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime generated consumer `{}` has a foreign family",
                        address.invocation
                    )));
                }
                let binding = template_argument_bindings_with_paths(&template.arguments)
                    .into_iter()
                    .find(|(candidate, _)| candidate == &property)
                    .map(|(_, binding)| binding)
                    .ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime generated consumer `{}` names no template argument `{}`",
                            address.invocation,
                            property_path_text(&property),
                        ))
                    })?;
                let TemplateBinding::Input {
                    name,
                    path: _,
                    expected_kind,
                } = binding
                else {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime generated consumer `{}` is not backed by one source input",
                        address.invocation
                    )));
                };
                // The executed consumer site authenticates the complete
                // lexical invocation argument. `binding.path` traverses the
                // referenced runtime result, not nested source syntax.
                let source_path = invocation_input_path(name, &SemanticOutputPath::default());
                let invocation_value =
                    managed_value_at_path(&plan.declaration.arguments, &source_path).ok_or_else(
                        || {
                            ManagedControlError::ForeignExpansion(format!(
                                "runtime generated consumer `{}` lost its invocation input",
                                address.invocation
                            ))
                        },
                    )?;
                let (expected_declaration, expected_path, _) = scalar_source_owner(
                    project,
                    &plan.declaration.symbol,
                    &source_path,
                    invocation_value,
                );
                if expected_declaration != owner.declaration || expected_path != owner.path {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime generated consumer `{}` does not match its exact IR value origin",
                        address.invocation
                    )));
                }
                let generated_address = executed_generated_address(address);
                let provenance = expansion
                    .generated_provenance
                    .get(&generated_address)
                    .ok_or_else(|| {
                        ManagedControlError::ForeignExpansion(format!(
                            "runtime generated consumer `{}` is absent from exact expansion generation",
                            generated_address.display_path()
                        ))
                    })?;
                if provenance.declaration != invocation
                    || provenance.artifact_digest.as_deref() != Some(plan.digest.as_str())
                {
                    return Err(ManagedControlError::ForeignExpansion(format!(
                        "runtime generated consumer `{}` has stale generation provenance",
                        generated_address.display_path()
                    )));
                }
                (
                    ManagedControlConsumer {
                        target: ManagedControlConsumerTarget::Generated {
                            address: generated_address,
                            identity: provenance.identity,
                            artifact_digest: plan.digest.clone(),
                            family: family.clone(),
                        },
                        property: property.clone(),
                    },
                    schema_for_template_input(
                        family,
                        last_field(&property).unwrap_or("argument"),
                        *expected_kind,
                        source_value,
                    ),
                )
            }
        };
        let key = (owner.declaration.clone(), owner.path.clone());
        let inserted = consumer_map
            .entry(key.clone())
            .or_default()
            .insert(consumer);
        if inserted {
            admit_consumer_edge(&mut consumer_edges)?;
        }
        if let Some(schema) = schema {
            schema_map.entry(key).or_default().push(schema);
        }
    }

    let mut controls = Vec::with_capacity(project.managed.value_owned_spans.len());
    for binding in &project.managed.program.scalar_bindings {
        let empty_path = SemanticOutputPath::default();
        let owned = owned_spans
            .get(&binding.symbol)
            .and_then(|paths| paths.get(&empty_path))
            .copied()
            .ok_or_else(|| {
                ManagedControlError::ForeignExpansion(format!(
                    "lexical binding `{}` has no authenticated source value",
                    binding.symbol.0
                ))
            })?;
        let key = (binding.symbol.clone(), empty_path);
        let consumers = consumer_map
            .remove(&key)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let classification =
            classify_scalar_binding(&binding.value, &schema_map.remove(&key).unwrap_or_default());
        push_classified_control(
            &mut controls,
            project,
            project_digest,
            ClassifiedControl {
                declaration: binding.symbol.clone(),
                owned,
                value: &binding.value,
                consumers,
                classification,
            },
        )?;
    }
    for declaration in &project.managed.program.declarations {
        for owned in owned_spans
            .get(&declaration.symbol)
            .into_iter()
            .flat_map(|paths| paths.values().copied())
        {
            let value =
                managed_value_at_path(&declaration.arguments, &owned.path).ok_or_else(|| {
                    ManagedControlError::ForeignExpansion(format!(
                        "source path `{}` is absent",
                        path_text(&declaration.symbol, &owned.path)
                    ))
                })?;
            let key = (declaration.symbol.clone(), owned.path.clone());
            let consumers = consumer_map
                .remove(&key)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            let classification = classify_compiled_control(
                declaration,
                &owned.path,
                value,
                &schema_map.remove(&key).unwrap_or_default(),
                artifact_plan_index.get(&declaration.symbol).copied(),
            );
            push_classified_control(
                &mut controls,
                project,
                project_digest,
                ClassifiedControl {
                    declaration: declaration.symbol.clone(),
                    owned,
                    value,
                    consumers,
                    classification,
                },
            )?;
        }
    }
    if !consumer_map.is_empty() || !schema_map.is_empty() {
        return Err(ManagedControlError::ForeignExpansion(
            "runtime consumer provenance did not resolve to one projected control".into(),
        ));
    }
    controls.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ManagedControlManifest {
        project: project.project.clone(),
        project_digest: project_digest.to_owned(),
        source_digest: project.managed.source_digest.clone(),
        expansion_digest: expansion.digest.clone(),
        controls,
    })
}

fn managed_source_value<'a>(
    project: &'a CodeProject,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
) -> Option<&'a ManagedValue> {
    if path.0.is_empty()
        && let Some(binding) = project
            .managed
            .program
            .scalar_bindings
            .iter()
            .find(|binding| binding.symbol == *declaration)
    {
        return Some(&binding.value);
    }
    project
        .managed
        .program
        .declarations
        .iter()
        .find(|candidate| candidate.symbol == *declaration)
        .and_then(|candidate| managed_value_at_path(&candidate.arguments, path))
}

fn executed_generated_address(
    address: &crate::ExecutedGeneratedMemberAddress,
) -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(
        address.invocation.clone(),
        address.template.clone(),
        if address.member_key.is_empty() {
            vec!["self".to_owned()]
        } else {
            address.member_key.clone()
        },
        address.output.clone(),
    )
}

fn path_suffix_text(path: &SemanticOutputPath) -> String {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => format!("[{index}]"),
            ManagedPathSegment::Member { member } => format!("[\"{member}\"]"),
        })
        .collect::<Vec<_>>()
        .join(".")
}

struct ClassifiedControl<'a> {
    declaration: SemanticSymbol,
    owned: &'a crate::ManagedValueOwnedSpan,
    value: &'a ManagedValue,
    consumers: Vec<ManagedControlConsumer>,
    classification: ControlClassification,
}

fn push_classified_control(
    controls: &mut Vec<ManagedControl>,
    project: &CodeProject,
    project_digest: &str,
    input: ClassifiedControl<'_>,
) -> Result<(), ManagedControlError> {
    let ClassifiedControl {
        declaration,
        owned,
        value,
        mut consumers,
        classification,
    } = input;
    if controls.len() >= MANAGED_CONTROL_LIMIT {
        return Err(ManagedControlError::ResourceLimit {
            resource: "manifest controls",
            actual: controls.len().saturating_add(1),
            limit: MANAGED_CONTROL_LIMIT,
        });
    }
    consumers.sort();
    consumers.dedup();
    let id = managed_control_id(&project.project, &declaration, &owned.path)?;
    let source_text = project
        .managed
        .source
        .get(owned.span.start..owned.span.end)
        .ok_or_else(|| {
            ManagedControlError::ForeignExpansion(format!(
                "invalid source span for `{}`",
                path_text(&declaration, &owned.path)
            ))
        })?
        .to_owned();
    let source = ManagedControlSource {
        declaration: declaration.clone(),
        path: owned.path.clone(),
        kind: owned.kind,
        span: owned.span,
        source_text,
    };
    let (schema, access) = match classification {
        ControlClassification::Editable(schema) => {
            let generation_digest = digest_serializable(&consumers)?;
            let mut token = ManagedControlToken {
                id: id.clone(),
                project: project.project.clone(),
                project_digest: project_digest.to_owned(),
                source_digest: project.managed.source_digest.clone(),
                declaration,
                path: owned.path.clone(),
                expected: value.clone(),
                generation_digest,
                authentication: String::new(),
            };
            token.authentication = token_authentication(&token, &schema)?;
            (Some(schema), ManagedControlAccess::Editable { token })
        }
        ControlClassification::ReadOnly { reason, navigation } => {
            (None, ManagedControlAccess::ReadOnly { reason, navigation })
        }
    };
    controls.push(ManagedControl {
        id,
        source,
        value: value.clone(),
        schema,
        consumers,
        access,
    });
    Ok(())
}

/// Authenticates one exact-CAS control batch and projects it to the single
/// managed semantic mutation that a compiler host may execute.
///
/// This function never rewrites source. Callers pass the result to
/// [`crate::prepare_managed_mutation`], execute that prepared request in the
/// browser or pinned Deno host, and publish only after
/// [`crate::validate_prepared_managed_mutation`] accepts the receipt.
///
/// # Errors
///
/// Returns a stale/foreign/read-only/typing/resource failure without changing
/// `project` or `expansion`.
pub fn prepare_managed_control_mutation(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    batch: &ManagedControlEditBatch,
) -> Result<ManagedSketchMutation, ManagedControlError> {
    validate_control_batch_shape(batch)?;
    let manifest = managed_control_manifest(project, expansion)?;
    prepare_managed_control_mutation_against_manifest(&manifest, batch)
}

fn validate_control_batch_shape(
    batch: &ManagedControlEditBatch,
) -> Result<(), ManagedControlError> {
    if batch.edits.is_empty() {
        return Err(ManagedControlError::EmptyBatch);
    }
    if batch.edits.len() > MANAGED_CONTROL_LIMIT {
        return Err(ManagedControlError::ResourceLimit {
            resource: "control batch edits",
            actual: batch.edits.len(),
            limit: MANAGED_CONTROL_LIMIT,
        });
    }
    Ok(())
}

fn prepare_managed_control_mutation_against_manifest(
    manifest: &ManagedControlManifest,
    batch: &ManagedControlEditBatch,
) -> Result<ManagedSketchMutation, ManagedControlError> {
    let mut seen = BTreeSet::new();
    let mut spans = Vec::with_capacity(batch.edits.len());
    let mut values = Vec::with_capacity(batch.edits.len());
    for edit in &batch.edits {
        if !seen.insert(edit.token.id.clone()) {
            return Err(ManagedControlError::DuplicateControl(
                edit.token.id.0.clone(),
            ));
        }
        let Some(control) = manifest.control(&edit.token.id) else {
            return Err(ManagedControlError::UnknownControl(edit.token.id.0.clone()));
        };
        let Some(current_token) = control.token() else {
            return Err(ManagedControlError::ReadOnlyControl(
                edit.token.id.0.clone(),
            ));
        };
        let schema = control
            .schema
            .as_ref()
            .ok_or_else(|| ManagedControlError::ReadOnlyControl(edit.token.id.0.clone()))?;
        if token_authentication(&edit.token, schema)? != edit.token.authentication
            || !tokens_exactly_equal(current_token, &edit.token)
        {
            return Err(ManagedControlError::StaleToken(edit.token.id.0.clone()));
        }
        validate_replacement(schema, &edit.value).map_err(|message| {
            ManagedControlError::InvalidReplacement {
                control: edit.token.id.0.clone(),
                message,
            }
        })?;
        values.push(ManagedValueMutation {
            declaration: control.source.declaration.0.clone(),
            path: control.source.path.0.clone(),
            expected: control.value.clone(),
            value: edit.value.clone(),
        });
        spans.push((edit.token.id.0.as_str(), control.source.span));
    }
    validate_non_overlapping_control_spans(&mut spans)?;
    Ok(ManagedSketchMutation::SetValues { values })
}

#[derive(Clone)]
struct ArtifactPlan<'a> {
    declaration: &'a AuthoringDeclaration,
    digest: String,
    artifact: ValidatedPatchModuleArtifact,
}

fn artifact_plans(project: &CodeProject) -> Result<Vec<ArtifactPlan<'_>>, ManagedControlError> {
    let mut validated = Vec::with_capacity(project.artifacts.len());
    for (digest, value) in &project.artifacts {
        let artifact: PatchModuleArtifact = serde_json::from_value(value.clone())
            .map_err(|error| ManagedControlError::Encoding(error.to_string()))?;
        validated.push((digest.clone(), artifact.validate()?));
    }

    let mut artifacts_by_export = BTreeMap::<(&str, &str), Vec<usize>>::new();
    for (index, (_, artifact)) in validated.iter().enumerate() {
        artifacts_by_export
            .entry((
                artifact.artifact().export_name.as_str(),
                artifact.artifact().module_specifier.as_str(),
            ))
            .or_default()
            .push(index);
    }
    let mut artifacts_by_binding = BTreeMap::<&str, BTreeSet<usize>>::new();
    for import in &project.managed.imports {
        for binding in &import.bindings {
            if let Some(indices) =
                artifacts_by_export.get(&(binding.as_str(), import.module.as_str()))
            {
                artifacts_by_binding
                    .entry(binding.as_str())
                    .or_default()
                    .extend(indices.iter().copied());
            }
        }
    }

    let mut plans = Vec::new();
    for declaration in &project.managed.program.declarations {
        let Some(patch) = &declaration.patch else {
            continue;
        };
        let Some(indices) = artifacts_by_binding.get(patch.module_binding.as_str()) else {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "invocation `{}` does not resolve one pinned artifact",
                declaration.symbol.0
            )));
        };
        let mut indices = indices.iter();
        let Some(index) = indices.next().copied() else {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "invocation `{}` does not resolve one pinned artifact",
                declaration.symbol.0
            )));
        };
        if indices.next().is_some() {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "invocation `{}` does not resolve one pinned artifact",
                declaration.symbol.0
            )));
        }
        let (digest, artifact) = &validated[index];
        plans.push(ArtifactPlan {
            declaration,
            digest: digest.clone(),
            artifact: artifact.clone(),
        });
    }
    Ok(plans)
}

fn validate_expansion_cross_links(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<(), ManagedControlError> {
    if !valid_digest(&expansion.digest) {
        return Err(ManagedControlError::ForeignExpansion(
            "expansion digest is malformed".into(),
        ));
    }
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
        &expansion.operation_plans,
    ))
    .map_err(|error| ManagedControlError::Encoding(error.to_string()))?;
    if intent_content_digest(&bytes).to_string() != expansion.digest {
        return Err(ManagedControlError::ForeignExpansion(
            "expansion digest does not authenticate its contents".into(),
        ));
    }
    let declarations = project
        .managed
        .program
        .declarations
        .iter()
        .map(|declaration| &declaration.symbol)
        .collect::<BTreeSet<_>>();
    for (address, provenance) in &expansion.generated_provenance {
        if address.invocation != provenance.declaration.0
            || !declarations.contains(&provenance.declaration)
            || provenance
                .artifact_digest
                .as_ref()
                .is_some_and(|digest| !project.artifacts.contains_key(digest))
        {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "generated provenance `{}` is foreign",
                address.display_path()
            )));
        }
    }
    Ok(())
}

fn project_authority_digest(project: &CodeProject) -> Result<String, ManagedControlError> {
    digest_serializable(&(
        &project.project,
        &project.custom_files,
        &project.artifacts,
        &project.lock,
    ))
}

fn digest_serializable(value: &impl Serialize) -> Result<String, ManagedControlError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| ManagedControlError::Encoding(error.to_string()))?;
    Ok(intent_content_digest(&bytes).to_string())
}

fn managed_control_id(
    project: &ProjectKey,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
) -> Result<ManagedControlId, ManagedControlError> {
    digest_serializable(&(project, declaration, path)).map(ManagedControlId)
}

fn invocation_input_path(name: &str, suffix: &SemanticOutputPath) -> SemanticOutputPath {
    let mut path = vec![ManagedPathSegment::Field(name.to_owned())];
    path.extend(suffix.0.iter().cloned());
    SemanticOutputPath(path)
}

fn managed_value_at_path<'a>(
    root: &'a ManagedValue,
    path: &SemanticOutputPath,
) -> Option<&'a ManagedValue> {
    let mut value = root;
    for segment in &path.0 {
        value = match (segment, value) {
            (ManagedPathSegment::Field(field), ManagedValue::Object(fields)) => {
                fields.get(field)?
            }
            (ManagedPathSegment::Index(index), ManagedValue::Array(values)) => {
                values.get(*index)?
            }
            _ => return None,
        };
    }
    Some(value)
}

/// Follows exactly one parser-authenticated lexical scalar reference. The
/// managed grammar forbids scalar-binding chains and child paths, so there is
/// no expression evaluation, cycle traversal, or guessed inverse here.
fn scalar_source_owner<'a>(
    project: &'a CodeProject,
    invocation: &SemanticSymbol,
    invocation_path: &SemanticOutputPath,
    value: &'a ManagedValue,
) -> (SemanticSymbol, SemanticOutputPath, &'a ManagedValue) {
    let ManagedValue::Reference { declaration, path } = value else {
        return (invocation.clone(), invocation_path.clone(), value);
    };
    if !path.0.is_empty() {
        return (invocation.clone(), invocation_path.clone(), value);
    }
    let Some(binding) = project
        .managed
        .program
        .scalar_bindings
        .iter()
        .find(|candidate| candidate.symbol == *declaration)
    else {
        return (invocation.clone(), invocation_path.clone(), value);
    };
    (
        binding.symbol.clone(),
        SemanticOutputPath::default(),
        &binding.value,
    )
}

enum ControlClassification {
    Editable(ManagedControlSchema),
    ReadOnly {
        reason: ManagedControlReadOnlyReason,
        navigation: Option<ManagedControlNavigation>,
    },
}

/// Managed schemas may validate an executed consumer edge, but may not
/// create one. In particular, patch templates and the direct family catalog
/// cannot make an unobserved value editable merely because its type looks
/// compatible.
fn classify_compiled_control(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
    value: &ManagedValue,
    schemas: &[ManagedControlSchema],
    artifact: Option<&ArtifactPlan<'_>>,
) -> ControlClassification {
    match value {
        ManagedValue::Reference { declaration, path } => {
            return ControlClassification::ReadOnly {
                reason: ManagedControlReadOnlyReason::Reference,
                navigation: Some(ManagedControlNavigation {
                    declaration: declaration.clone(),
                    path: path.clone(),
                }),
            };
        }
        ManagedValue::Null => return read_only(ManagedControlReadOnlyReason::Null),
        ManagedValue::Array(_) | ManagedValue::Object(_) => {
            return read_only(ManagedControlReadOnlyReason::Structure);
        }
        ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_) => {}
    }
    if direct_solver_instance_path(declaration, path)
        || artifact.is_some_and(|plan| artifact_solver_instance_path(plan, path))
    {
        return read_only(ManagedControlReadOnlyReason::SolverInstance);
    }
    if direct_structural_identity_path(declaration, path) {
        return read_only(ManagedControlReadOnlyReason::StructuralIdentity);
    }
    let Some(first) = schemas.first().cloned() else {
        return read_only(ManagedControlReadOnlyReason::UnprovenTransform);
    };
    let mut merged = first;
    for schema in schemas.iter().skip(1) {
        let Some(next) = merge_schemas(&merged, schema) else {
            return read_only(ManagedControlReadOnlyReason::IncompatibleSchemas);
        };
        merged = next;
    }
    if validate_replacement(&merged, value).is_err() {
        return read_only(ManagedControlReadOnlyReason::IncompatibleSchemas);
    }
    ControlClassification::Editable(merged)
}

fn classify_scalar_binding(
    value: &ManagedValue,
    schemas: &[ManagedControlSchema],
) -> ControlClassification {
    if !matches!(value, ManagedValue::Number(_) | ManagedValue::Unit(_)) {
        return read_only(ManagedControlReadOnlyReason::UnprovenTransform);
    }
    let Some(first) = schemas.first().cloned() else {
        return read_only(ManagedControlReadOnlyReason::UnprovenTransform);
    };
    let mut merged = first;
    for schema in schemas.iter().skip(1) {
        let Some(next) = merge_schemas(&merged, schema) else {
            return read_only(ManagedControlReadOnlyReason::IncompatibleSchemas);
        };
        merged = next;
    }
    if validate_replacement(&merged, value).is_err() {
        return read_only(ManagedControlReadOnlyReason::IncompatibleSchemas);
    }
    ControlClassification::Editable(merged)
}

fn admit_consumer_edge(count: &mut usize) -> Result<(), ManagedControlError> {
    *count = count.saturating_add(1);
    if *count > MANAGED_CONTROL_CONSUMER_LIMIT {
        return Err(ManagedControlError::ResourceLimit {
            resource: "source-consumer edges",
            actual: *count,
            limit: MANAGED_CONTROL_CONSUMER_LIMIT,
        });
    }
    Ok(())
}

fn validate_non_overlapping_control_spans(
    spans: &mut [(&str, ManagedSpan)],
) -> Result<(), ManagedControlError> {
    spans.sort_unstable_by_key(|(_, span)| (span.start, span.end));
    for pair in spans.windows(2) {
        let [(first_id, first), (second_id, second)] = pair else {
            unreachable!("windows of two have exactly two entries");
        };
        if first.end > second.start {
            return Err(ManagedControlError::OverlappingControls {
                first: (*first_id).to_owned(),
                second: (*second_id).to_owned(),
            });
        }
    }
    Ok(())
}

fn read_only(reason: ManagedControlReadOnlyReason) -> ControlClassification {
    ControlClassification::ReadOnly {
        reason,
        navigation: None,
    }
}

fn named_declaration_descriptor(
    declaration: &AuthoringDeclaration,
) -> Result<Option<CodeAuthoringDeclarationDescriptor>, ManagedControlError> {
    if declaration.patch.is_some() {
        return Ok(None);
    }
    let [namespace, method] = declaration.builder_path.as_slice() else {
        return Ok(None);
    };
    let Some(family) = code_authoring_family(namespace, method) else {
        return Ok(None);
    };
    let dynamic_children = match family.dynamic_children {
        CodeAuthoringDynamicChildren::None => 0,
        CodeAuthoringDynamicChildren::PolylineVertices => {
            named_collection_len(&declaration.arguments, "vertices")?
        }
        CodeAuthoringDynamicChildren::SplineControls => {
            named_collection_len(&declaration.arguments, "controls")?
        }
        CodeAuthoringDynamicChildren::FilletCorners => {
            named_collection_len(&declaration.arguments, "corners")?
        }
        CodeAuthoringDynamicChildren::PatternInstances => {
            let Some(ManagedValue::Number(value)) =
                declaration_argument(&declaration.arguments, "instances")
            else {
                return Err(ManagedControlError::ForeignExpansion(format!(
                    "named declaration `{}` has no exact pattern instance count",
                    declaration.symbol.0
                )));
            };
            if !value.is_finite()
                || value.is_sign_negative()
                || value.fract() != 0.0
                || *value > f64::from(u16::MAX)
            {
                return Err(ManagedControlError::ForeignExpansion(format!(
                    "named declaration `{}` has an invalid pattern instance count",
                    declaration.symbol.0
                )));
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the finite integral value is bounded to u16 immediately above"
            )]
            {
                *value as u16
            }
        }
    };
    resolve_code_authoring_declaration(namespace, method, dynamic_children)
        .map(Some)
        .map_err(|error| ManagedControlError::ForeignExpansion(error.to_string()))
}

fn named_collection_len(arguments: &ManagedValue, name: &str) -> Result<u16, ManagedControlError> {
    let Some(ManagedValue::Array(values)) = declaration_argument(arguments, name) else {
        return Err(ManagedControlError::ForeignExpansion(format!(
            "named declaration argument `{name}` is not an array"
        )));
    };
    u16::try_from(values.len()).map_err(|_| ManagedControlError::ResourceLimit {
        resource: "named declaration dynamic children",
        actual: values.len(),
        limit: usize::from(u16::MAX),
    })
}

fn declaration_argument<'a>(arguments: &'a ManagedValue, name: &str) -> Option<&'a ManagedValue> {
    let ManagedValue::Object(fields) = arguments else {
        return None;
    };
    fields.get(name)
}

fn named_schema(
    declaration: &AuthoringDeclaration,
    property: &SemanticOutputPath,
    value: &ManagedValue,
) -> Result<Option<ManagedControlSchema>, ManagedControlError> {
    if path_is_field(property, "label") {
        return Ok(matches!(value, ManagedValue::String(_)).then_some(ManagedControlSchema::Text));
    }
    if path_ends_with_field(property, "suppressed") {
        return Ok(matches!(value, ManagedValue::Bool(_)).then_some(ManagedControlSchema::Boolean));
    }
    let Some(descriptor) = named_declaration_descriptor(declaration)? else {
        return Ok(None);
    };

    if descriptor.dynamic_children.kind == CodeAuthoringDynamicChildren::SplineControls {
        if spline_control_property(property, "weight") {
            return Ok(positive_scalar_schema(value));
        }
        if path_is_field(property, "gauge") {
            return named_spline_gauge_schema(declaration, value).map(Some);
        }
    }

    let definition = descriptor
        .fields
        .iter()
        .find(|field| named_source_projection_path(&field.path) == *property)
        .map(|field| {
            (
                field.schema.field.0.as_str(),
                field.schema.literal,
                &field.choices,
            )
        });
    let authored = descriptor
        .values
        .iter()
        .find(|field| named_source_projection_path(&field.path) == *property)
        .map(|field| ("", field.literal, &field.choices));
    let Some((field, literal, choices)) = definition.or(authored) else {
        return Ok(None);
    };
    if field.ends_with("_periodic_anchor") {
        return Ok(matches!(value, ManagedValue::String(_)).then(|| {
            ManagedControlSchema::Choice {
                choices: vec!["none".into(), "anchor".into()],
            }
        }));
    }
    if let Some(schema) = named_numeric_schema_override(&descriptor, property, literal, value) {
        return Ok(Some(schema));
    }
    Ok(schema_for_literal(literal, choices, value))
}

fn named_spline_gauge_schema(
    declaration: &AuthoringDeclaration,
    value: &ManagedValue,
) -> Result<ManagedControlSchema, ManagedControlError> {
    if !matches!(value, ManagedValue::String(_)) {
        return Err(ManagedControlError::ForeignExpansion(format!(
            "named NURBS declaration `{}` has a non-text gauge",
            declaration.symbol.0
        )));
    }
    let Some(ManagedValue::Array(controls)) =
        declaration_argument(&declaration.arguments, "controls")
    else {
        return Err(ManagedControlError::ForeignExpansion(format!(
            "named NURBS declaration `{}` has no control collection",
            declaration.symbol.0
        )));
    };
    let mut choices = Vec::with_capacity(controls.len());
    for control in controls {
        let ManagedValue::Object(fields) = control else {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "named NURBS declaration `{}` has a malformed control",
                declaration.symbol.0
            )));
        };
        let Some(ManagedValue::String(key)) = fields.get("key") else {
            return Err(ManagedControlError::ForeignExpansion(format!(
                "named NURBS declaration `{}` has a control without a text key",
                declaration.symbol.0
            )));
        };
        choices.push(key.clone());
    }
    Ok(ManagedControlSchema::Choice { choices })
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed named API's numeric-domain policy is intentionally reviewed in one exhaustive match"
)]
fn named_numeric_schema_override(
    descriptor: &CodeAuthoringDeclarationDescriptor,
    property: &SemanticOutputPath,
    literal: IntentLiteralSchema,
    value: &ManagedValue,
) -> Option<ManagedControlSchema> {
    use geosolve_sketch_intent::{
        ComputedFeatureKind, DimensionKind, GeometryRecipeKind, OperationKind,
    };

    let positive = match descriptor.declaration {
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Hyperbola)
            if path_is_field(property, "semiConjugate") =>
        {
            true
        }
        CodeAuthoringDeclarationKind::Dimension(
            DimensionKind::PointDistance
            | DimensionKind::CurveLength
            | DimensionKind::Radius
            | DimensionKind::Diameter
            | DimensionKind::OrientedAngle
            | DimensionKind::SupportingLineOffset
            | DimensionKind::ExactTranslatedSegmentOffset
            | DimensionKind::ProfileOffset,
        ) if path_is_field(property, "value") => true,
        CodeAuthoringDeclarationKind::Operation(OperationKind::Chamfer)
            if path_is_field(property, "firstDistance")
                || path_is_field(property, "secondDistance") =>
        {
            true
        }
        CodeAuthoringDeclarationKind::Operation(OperationKind::Rectangle)
            if path_is_field(property, "width") || path_is_field(property, "height") =>
        {
            true
        }
        CodeAuthoringDeclarationKind::Operation(OperationKind::ProfileOffset)
            if path_is_field(property, "distance") =>
        {
            true
        }
        CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::CenterRadiusCircle)
        | CodeAuthoringDeclarationKind::Operation(
            OperationKind::AssociativeFillet | OperationKind::RegularPolygon | OperationKind::Slot,
        )
        | CodeAuthoringDeclarationKind::ComputedFeature(ComputedFeatureKind::FilletSet)
            if path_is_field(property, "radius") =>
        {
            true
        }
        _ => false,
    };
    if positive {
        return positive_scalar_schema(value);
    }

    match descriptor.declaration {
        CodeAuthoringDeclarationKind::Geometry(
            GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs,
        ) if path_is_field(property, "degree") => scalar_schema(
            value,
            ManagedControlNumberKind::Natural,
            Some(ManagedControlBound {
                value: 1.0,
                inclusive: true,
            }),
            Some(ManagedControlBound {
                value: f64::from(descriptor.dynamic_children.count.saturating_sub(1)),
                inclusive: true,
            }),
        ),
        CodeAuthoringDeclarationKind::Operation(OperationKind::RegularPolygon)
            if path_is_field(property, "sides") =>
        {
            scalar_schema(
                value,
                ManagedControlNumberKind::Natural,
                Some(ManagedControlBound {
                    value: 3.0,
                    inclusive: true,
                }),
                Some(ManagedControlBound {
                    value: 256.0,
                    inclusive: true,
                }),
            )
        }
        CodeAuthoringDeclarationKind::Operation(OperationKind::LinearPattern)
            if path_is_field(property, "instances") =>
        {
            scalar_schema(
                value,
                ManagedControlNumberKind::Natural,
                Some(ManagedControlBound {
                    value: 2.0,
                    inclusive: true,
                }),
                Some(ManagedControlBound {
                    value: 256.0,
                    inclusive: true,
                }),
            )
        }
        _ if literal == IntentLiteralSchema::Natural => scalar_schema(
            value,
            ManagedControlNumberKind::Natural,
            Some(ManagedControlBound {
                value: 0.0,
                inclusive: true,
            }),
            None,
        ),
        _ => None,
    }
}

fn schema_for_literal(
    literal: IntentLiteralSchema,
    choices: &IntentFieldChoices,
    value: &ManagedValue,
) -> Option<ManagedControlSchema> {
    match literal {
        IntentLiteralSchema::Boolean => {
            matches!(value, ManagedValue::Bool(_)).then_some(ManagedControlSchema::Boolean)
        }
        IntentLiteralSchema::Integer => scalar_schema(
            value,
            ManagedControlNumberKind::Integer,
            Some(ManagedControlBound {
                value: f64::from(i32::MIN),
                inclusive: true,
            }),
            Some(ManagedControlBound {
                value: f64::from(i32::MAX),
                inclusive: true,
            }),
        ),
        IntentLiteralSchema::Natural => scalar_schema(
            value,
            ManagedControlNumberKind::Natural,
            Some(ManagedControlBound {
                value: 0.0,
                inclusive: true,
            }),
            None,
        ),
        IntentLiteralSchema::Text => {
            matches!(value, ManagedValue::String(_)).then_some(ManagedControlSchema::Text)
        }
        IntentLiteralSchema::Enum => {
            let IntentFieldChoices::Closed(choices) = choices else {
                return None;
            };
            matches!(value, ManagedValue::String(_)).then(|| ManagedControlSchema::Choice {
                choices: choices
                    .iter()
                    .map(|choice| source_enum_choice(choice.as_str()))
                    .collect(),
            })
        }
        IntentLiteralSchema::Point => None,
        IntentLiteralSchema::Quantity(_) => {
            scalar_schema(value, ManagedControlNumberKind::Real, None, None)
        }
    }
}

fn source_enum_choice(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut uppercase = false;
    for character in value.chars() {
        if character == '_' {
            uppercase = true;
        } else if uppercase {
            result.extend(character.to_uppercase());
            uppercase = false;
        } else {
            result.push(character);
        }
    }
    result
}

/// Translates the central semantic projection coordinate to the exact clean
/// named-object coordinate consumed by TypeScript. The mapping is structural
/// and injective; runtime values are never compared to guess an owner.
fn named_source_projection_path(path: &IntentProjectionPath) -> SemanticOutputPath {
    let source = path.segments();
    let mut output = Vec::with_capacity(source.len());
    let mut index = 0;
    while index < source.len() {
        match &source[index] {
            IntentProjectionPathSegment::Field(field) if index == 0 && field.as_str() == "name" => {
                output.push(ManagedPathSegment::Field("label".into()));
                index += 1;
            }
            IntentProjectionPathSegment::Field(field) if field.as_str() == "gaugeIndex" => {
                output.push(ManagedPathSegment::Field("gauge".into()));
                index += 1;
            }
            IntentProjectionPathSegment::Field(field)
                if field.as_str() == "contacts"
                    && matches!(
                        source.get(index + 1),
                        Some(IntentProjectionPathSegment::Index(0 | 1))
                    ) =>
            {
                output.push(ManagedPathSegment::Field("contacts".into()));
                let Some(IntentProjectionPathSegment::Index(side)) = source.get(index + 1) else {
                    unreachable!("guarded contact side")
                };
                output.push(ManagedPathSegment::Field(
                    if *side == 0 { "first" } else { "second" }.into(),
                ));
                index += 2;
            }
            IntentProjectionPathSegment::Field(field) if field.as_str() == "anchor" => {
                output.push(ManagedPathSegment::Field("periodicAnchor".into()));
                if matches!(
                    (source.get(index + 1), source.get(index + 2)),
                    (
                        Some(IntentProjectionPathSegment::Field(anchor)),
                        Some(IntentProjectionPathSegment::Field(enabled))
                    ) if anchor.as_str() == "anchor" && enabled.as_str() == "enabled"
                ) {
                    output.push(ManagedPathSegment::Field("kind".into()));
                    index += 3;
                } else if matches!(
                    source.get(index + 1),
                    Some(IntentProjectionPathSegment::Field(enabled)) if enabled.as_str() == "enabled"
                ) {
                    output.push(ManagedPathSegment::Field("kind".into()));
                    index += 2;
                } else {
                    index += 1;
                }
            }
            IntentProjectionPathSegment::Field(field) => {
                output.push(ManagedPathSegment::Field(field.as_str().to_owned()));
                index += 1;
            }
            IntentProjectionPathSegment::Index(value) => {
                output.push(ManagedPathSegment::Index(usize::from(*value)));
                index += 1;
            }
        }
    }
    SemanticOutputPath(output)
}

fn spline_control_property(path: &SemanticOutputPath, property: &str) -> bool {
    matches!(
        path.0.as_slice(),
        [
            ManagedPathSegment::Field(collection),
            ManagedPathSegment::Index(_),
            ManagedPathSegment::Field(field),
        ] if collection == "controls" && field == property
    )
}

fn collection_member_field(path: &SemanticOutputPath, expected: &str) -> bool {
    matches!(
        path.0.get(1..3),
        Some([
            ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. },
            ManagedPathSegment::Field(field),
        ]) if field == expected
    )
}

fn managed_projection_path(path: &IntentProjectionPath) -> SemanticOutputPath {
    SemanticOutputPath(
        path.segments()
            .iter()
            .map(|segment| match segment {
                IntentProjectionPathSegment::Field(field) => {
                    ManagedPathSegment::Field(field.as_str().to_owned())
                }
                IntentProjectionPathSegment::Index(index) => {
                    ManagedPathSegment::Index(usize::from(*index))
                }
            })
            .collect(),
    )
}

fn path_has_prefix(path: &SemanticOutputPath, prefix: &SemanticOutputPath) -> bool {
    path.0.starts_with(&prefix.0)
}

fn direct_solver_instance_path(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
) -> bool {
    let Ok(Some(descriptor)) = named_declaration_descriptor(declaration) else {
        return false;
    };
    for input in &descriptor.inputs {
        match &input.kind {
            CodeAuthoringArgumentKind::Point
                if path_starts_with_field(path, input.name.as_str()) =>
            {
                return true;
            }
            CodeAuthoringArgumentKind::Collection(
                CodeAuthoringCollectionMember::Point | CodeAuthoringCollectionMember::SplineControl,
            ) if path_starts_with_field(path, input.name.as_str())
                && collection_member_field(path, "position") =>
            {
                return true;
            }
            CodeAuthoringArgumentKind::Point
            | CodeAuthoringArgumentKind::Reference(_)
            | CodeAuthoringArgumentKind::ReferenceChoice(_)
            | CodeAuthoringArgumentKind::Collection(_) => {}
        }
    }
    descriptor.fields.iter().any(|field| {
        field.schema.literal == IntentLiteralSchema::Point
            && path_has_prefix(path, &managed_projection_path(&field.path))
    })
}

fn direct_structural_identity_path(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
) -> bool {
    let Ok(Some(descriptor)) = named_declaration_descriptor(declaration) else {
        return false;
    };
    path_ends_with_field(path, "key")
        && descriptor.inputs.iter().any(|input| {
            matches!(
                input.kind,
                CodeAuthoringArgumentKind::Collection(
                    CodeAuthoringCollectionMember::Point
                        | CodeAuthoringCollectionMember::SplineControl
                        | CodeAuthoringCollectionMember::FilletCorner
                        | CodeAuthoringCollectionMember::FilletParent
                )
            ) && path_starts_with_field(path, input.name.as_str())
        })
}

fn artifact_solver_instance_path(plan: &ArtifactPlan<'_>, path: &SemanticOutputPath) -> bool {
    let Some(ManagedPathSegment::Field(input)) = path.0.first() else {
        return false;
    };
    if plan.artifact.artifact().inputs.get(input) == Some(&FeatureKind::Point) {
        return true;
    }
    plan.artifact
        .artifact()
        .templates
        .iter()
        .flat_map(|template| template_argument_bindings_with_paths(&template.arguments))
        .any(|(_, binding)| match binding {
            TemplateBinding::CollectionMember {
                input: candidate,
                expected_kind: FeatureKind::Point,
                ..
            } => candidate == input && path.0.len() > 1,
            TemplateBinding::Input {
                name,
                expected_kind: FeatureKind::Point,
                ..
            } => name == input,
            _ => false,
        })
}

fn schema_for_template_input(
    family: &str,
    property: &str,
    expected_kind: FeatureKind,
    value: &ManagedValue,
) -> Option<ManagedControlSchema> {
    if expected_kind != FeatureKind::Scalar {
        return None;
    }
    if matches!(
        (family, property),
        ("computed.fillet" | "geometry.centerRadiusCircle", "radius")
            | ("dimension.radius", "value")
            | (
                "computed.roundedRectangleProfile",
                "width" | "height" | "cornerRadius"
            )
    ) {
        positive_scalar_schema(value)
    } else {
        scalar_schema(value, ManagedControlNumberKind::Real, None, None)
    }
}

fn positive_scalar_schema(value: &ManagedValue) -> Option<ManagedControlSchema> {
    scalar_schema(
        value,
        ManagedControlNumberKind::Real,
        Some(ManagedControlBound {
            value: 0.0,
            inclusive: false,
        }),
        None,
    )
}

fn scalar_schema(
    value: &ManagedValue,
    number: ManagedControlNumberKind,
    minimum: Option<ManagedControlBound>,
    maximum: Option<ManagedControlBound>,
) -> Option<ManagedControlSchema> {
    match value {
        ManagedValue::Number(value) if value.is_finite() => Some(ManagedControlSchema::Number {
            number,
            minimum,
            maximum,
        }),
        ManagedValue::Unit(UnitLiteral { unit, value }) if value.is_finite() => {
            Some(ManagedControlSchema::Unit {
                unit: unit.clone(),
                number,
                minimum,
                maximum,
            })
        }
        _ => None,
    }
}

fn merge_schemas(
    left: &ManagedControlSchema,
    right: &ManagedControlSchema,
) -> Option<ManagedControlSchema> {
    match (left, right) {
        (
            ManagedControlSchema::Number {
                number: left_number,
                minimum: left_minimum,
                maximum: left_maximum,
            },
            ManagedControlSchema::Number {
                number: right_number,
                minimum: right_minimum,
                maximum: right_maximum,
            },
        ) => numeric_schema(
            None,
            merge_number_kind(*left_number, *right_number),
            stricter_minimum(*left_minimum, *right_minimum),
            stricter_maximum(*left_maximum, *right_maximum),
        ),
        (
            ManagedControlSchema::Unit {
                unit: left_unit,
                number: left_number,
                minimum: left_minimum,
                maximum: left_maximum,
            },
            ManagedControlSchema::Unit {
                unit: right_unit,
                number: right_number,
                minimum: right_minimum,
                maximum: right_maximum,
            },
        ) if left_unit == right_unit => numeric_schema(
            Some(left_unit),
            merge_number_kind(*left_number, *right_number),
            stricter_minimum(*left_minimum, *right_minimum),
            stricter_maximum(*left_maximum, *right_maximum),
        ),
        (ManagedControlSchema::Boolean, ManagedControlSchema::Boolean) => {
            Some(ManagedControlSchema::Boolean)
        }
        (
            ManagedControlSchema::Choice {
                choices: left_choices,
            },
            ManagedControlSchema::Choice {
                choices: right_choices,
            },
        ) => {
            let choices = left_choices
                .iter()
                .filter(|choice| right_choices.contains(choice))
                .cloned()
                .collect::<Vec<_>>();
            (!choices.is_empty()).then_some(ManagedControlSchema::Choice { choices })
        }
        (ManagedControlSchema::Text, ManagedControlSchema::Text) => {
            Some(ManagedControlSchema::Text)
        }
        _ => None,
    }
}

fn numeric_schema(
    unit: Option<&String>,
    number: ManagedControlNumberKind,
    minimum: Option<ManagedControlBound>,
    maximum: Option<ManagedControlBound>,
) -> Option<ManagedControlSchema> {
    if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
        let ordering = minimum.value.total_cmp(&maximum.value);
        if ordering.is_gt() || (ordering.is_eq() && (!minimum.inclusive || !maximum.inclusive)) {
            return None;
        }
    }
    Some(match unit {
        Some(unit) => ManagedControlSchema::Unit {
            unit: unit.clone(),
            number,
            minimum,
            maximum,
        },
        None => ManagedControlSchema::Number {
            number,
            minimum,
            maximum,
        },
    })
}

fn merge_number_kind(
    left: ManagedControlNumberKind,
    right: ManagedControlNumberKind,
) -> ManagedControlNumberKind {
    left.max(right)
}

fn stricter_minimum(
    left: Option<ManagedControlBound>,
    right: Option<ManagedControlBound>,
) -> Option<ManagedControlBound> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) if left.value > right.value => Some(left),
        (Some(left), Some(right)) if right.value > left.value => Some(right),
        (Some(left), Some(right)) => Some(ManagedControlBound {
            value: left.value,
            inclusive: left.inclusive && right.inclusive,
        }),
    }
}

fn stricter_maximum(
    left: Option<ManagedControlBound>,
    right: Option<ManagedControlBound>,
) -> Option<ManagedControlBound> {
    match (left, right) {
        (None, value) | (value, None) => value,
        (Some(left), Some(right)) if left.value < right.value => Some(left),
        (Some(left), Some(right)) if right.value < left.value => Some(right),
        (Some(left), Some(right)) => Some(ManagedControlBound {
            value: left.value,
            inclusive: left.inclusive && right.inclusive,
        }),
    }
}

fn validate_replacement(schema: &ManagedControlSchema, value: &ManagedValue) -> Result<(), String> {
    match (schema, value) {
        (
            ManagedControlSchema::Number {
                number,
                minimum,
                maximum,
            },
            ManagedValue::Number(value),
        ) => validate_number(*value, *number, *minimum, *maximum),
        (
            ManagedControlSchema::Unit {
                unit,
                number,
                minimum,
                maximum,
            },
            ManagedValue::Unit(candidate),
        ) if candidate.unit == *unit => {
            validate_number(candidate.value, *number, *minimum, *maximum)
        }
        (ManagedControlSchema::Boolean, ManagedValue::Bool(_))
        | (ManagedControlSchema::Text, ManagedValue::String(_)) => Ok(()),
        (ManagedControlSchema::Choice { choices }, ManagedValue::String(value))
            if choices.contains(value) =>
        {
            Ok(())
        }
        (ManagedControlSchema::Unit { unit, .. }, ManagedValue::Unit(candidate)) => Err(format!(
            "unit `{}` does not match expected `{unit}`",
            candidate.unit
        )),
        (ManagedControlSchema::Choice { choices }, ManagedValue::String(value)) => {
            Err(format!("`{value}` is not one of {}", choices.join(", ")))
        }
        _ => Err("replacement kind does not match the authoritative schema".into()),
    }
}

fn validate_number(
    value: f64,
    number: ManagedControlNumberKind,
    minimum: Option<ManagedControlBound>,
    maximum: Option<ManagedControlBound>,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err("number must be finite".into());
    }
    if matches!(
        number,
        ManagedControlNumberKind::Integer | ManagedControlNumberKind::Natural
    ) && value.fract() != 0.0
    {
        return Err("number must be integral".into());
    }
    if number == ManagedControlNumberKind::Natural && (value < 0.0 || value.is_sign_negative()) {
        return Err("number must be natural".into());
    }
    if minimum.is_some_and(|bound| {
        let ordering = value.total_cmp(&bound.value);
        ordering.is_lt() || (!bound.inclusive && ordering.is_eq())
    }) {
        return Err("number is below its admitted minimum".into());
    }
    if maximum.is_some_and(|bound| {
        let ordering = value.total_cmp(&bound.value);
        ordering.is_gt() || (!bound.inclusive && ordering.is_eq())
    }) {
        return Err("number is above its admitted maximum".into());
    }
    Ok(())
}

fn token_authentication(
    token: &ManagedControlToken,
    schema: &ManagedControlSchema,
) -> Result<String, ManagedControlError> {
    let typed_value = managed_value_fingerprint(&token.expected);
    digest_serializable(&(
        &token.id,
        &token.project,
        &token.project_digest,
        &token.source_digest,
        &token.declaration,
        &token.path,
        typed_value,
        &token.generation_digest,
        schema,
    ))
}

fn tokens_exactly_equal(left: &ManagedControlToken, right: &ManagedControlToken) -> bool {
    left.id == right.id
        && left.project == right.project
        && left.project_digest == right.project_digest
        && left.source_digest == right.source_digest
        && left.declaration == right.declaration
        && left.path == right.path
        && managed_values_exactly_equal(&left.expected, &right.expected)
        && left.generation_digest == right.generation_digest
        && left.authentication == right.authentication
}

fn managed_values_exactly_equal(left: &ManagedValue, right: &ManagedValue) -> bool {
    managed_value_fingerprint(left) == managed_value_fingerprint(right)
}

fn managed_value_fingerprint(value: &ManagedValue) -> String {
    match value {
        ManagedValue::Null => "null".into(),
        ManagedValue::Bool(value) => format!("bool:{value}"),
        ManagedValue::Number(value) => format!("number:{:016x}", value.to_bits()),
        ManagedValue::String(value) => format!(
            "string:{}",
            serde_json::to_string(value).expect("Rust strings serialize")
        ),
        ManagedValue::Unit(value) => format!(
            "unit:{}:{}:{:016x}",
            value.unit.len(),
            value.unit,
            value.value.to_bits()
        ),
        ManagedValue::Array(values) => format!(
            "array:{}:[{}]",
            values.len(),
            values
                .iter()
                .map(managed_value_fingerprint)
                .collect::<Vec<_>>()
                .join("|")
        ),
        ManagedValue::Object(values) => format!(
            "object:{}:{{{}}}",
            values.len(),
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}={}",
                    key.len(),
                    key,
                    managed_value_fingerprint(value)
                ))
                .collect::<Vec<_>>()
                .join("|")
        ),
        ManagedValue::Reference { declaration, path } => format!(
            "reference:{}:{}:{}",
            declaration.0.len(),
            declaration.0,
            serde_json::to_string(path).expect("semantic paths serialize")
        ),
    }
}

fn path_is_field(path: &SemanticOutputPath, expected: &str) -> bool {
    matches!(path.0.as_slice(), [ManagedPathSegment::Field(field)] if field == expected)
}

fn path_starts_with_field(path: &SemanticOutputPath, expected: &str) -> bool {
    matches!(path.0.first(), Some(ManagedPathSegment::Field(field)) if field == expected)
}

fn path_ends_with_field(path: &SemanticOutputPath, expected: &str) -> bool {
    matches!(path.0.last(), Some(ManagedPathSegment::Field(field)) if field == expected)
}

fn last_field(path: &SemanticOutputPath) -> Option<&str> {
    match path.0.last()? {
        ManagedPathSegment::Field(field) => Some(field),
        ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. } => None,
    }
}

fn path_text(declaration: &SemanticSymbol, path: &SemanticOutputPath) -> String {
    let mut result = declaration.0.clone();
    for segment in &path.0 {
        match segment {
            ManagedPathSegment::Field(field) => {
                result.push('.');
                result.push_str(field);
            }
            ManagedPathSegment::Index(index) => {
                result.push('[');
                result.push_str(&index.to_string());
                result.push(']');
            }
            ManagedPathSegment::Member { member } => {
                result.push('{');
                result.push_str(member);
                result.push('}');
            }
        }
    }
    result
}

fn property_path_text(path: &SemanticOutputPath) -> String {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => index.to_string(),
            ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl From<crate::ArtifactValidationError> for ManagedControlError {
    fn from(value: crate::ArtifactValidationError) -> Self {
        Self::ForeignExpansion(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_schema_validation_covers_natural_and_text_without_inventing_an_owner() {
        let natural = ManagedControlSchema::Number {
            number: ManagedControlNumberKind::Natural,
            minimum: Some(ManagedControlBound {
                value: 0.0,
                inclusive: true,
            }),
            maximum: Some(ManagedControlBound {
                value: 10.0,
                inclusive: true,
            }),
        };
        assert_eq!(
            validate_replacement(&natural, &ManagedValue::Number(0.0)),
            Ok(())
        );
        for invalid in [-0.0, -1.0, 1.5, 11.0] {
            assert!(validate_replacement(&natural, &ManagedValue::Number(invalid)).is_err());
        }

        let text = ManagedControlSchema::Text;
        let value = ManagedValue::String("fixture text\nwith a quote: \"".into());
        assert_eq!(validate_replacement(&text, &value), Ok(()));
        assert_eq!(
            value,
            ManagedValue::String("fixture text\nwith a quote: \"".into())
        );
    }

    #[test]
    fn incompatible_schema_intersections_fail_closed() {
        let real = ManagedControlSchema::Number {
            number: ManagedControlNumberKind::Real,
            minimum: Some(ManagedControlBound {
                value: 2.0,
                inclusive: false,
            }),
            maximum: None,
        };
        let disjoint = ManagedControlSchema::Number {
            number: ManagedControlNumberKind::Real,
            minimum: None,
            maximum: Some(ManagedControlBound {
                value: 2.0,
                inclusive: true,
            }),
        };
        assert_eq!(merge_schemas(&real, &disjoint), None);
        assert_eq!(
            merge_schemas(
                &ManagedControlSchema::Choice {
                    choices: vec!["first".into()],
                },
                &ManagedControlSchema::Choice {
                    choices: vec!["second".into()],
                },
            ),
            None
        );
        assert_eq!(
            merge_schemas(
                &real,
                &ManagedControlSchema::Unit {
                    unit: "mm".into(),
                    number: ManagedControlNumberKind::Real,
                    minimum: None,
                    maximum: None,
                }
            ),
            None
        );

        assert!(matches!(
            classify_scalar_binding(&ManagedValue::Number(2.0), &[real, disjoint]),
            ControlClassification::ReadOnly {
                reason: ManagedControlReadOnlyReason::IncompatibleSchemas,
                navigation: None,
            }
        ));
    }

    #[test]
    fn managed_batch_span_guard_rejects_distinct_overlapping_controls() {
        let mut spans = [
            ("outer", ManagedSpan::new(10, 20)),
            ("inner", ManagedSpan::new(15, 18)),
        ];
        assert_eq!(
            validate_non_overlapping_control_spans(&mut spans),
            Err(ManagedControlError::OverlappingControls {
                first: "outer".into(),
                second: "inner".into(),
            })
        );
    }
}
