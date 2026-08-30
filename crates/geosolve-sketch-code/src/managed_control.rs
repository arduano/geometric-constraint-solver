// SPDX-License-Identifier: GPL-3.0-or-later

//! Transient source-control provenance for managed code projects.
//!
//! Controls are derived from an already validated project and its exact
//! equation-free expansion. They are deliberately absent from artifact and
//! session wire formats: source remains durable authority, while this module
//! reconstructs reverse routes and generation evidence whenever required.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::intent_content_digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::parser::rewrite_managed_source_batch;
use crate::{
    AuthoringDeclaration, CodeProject, CodeProjectError, ExpandedCodeProject, FeatureKind,
    GeneratedMemberAddress, GeneratedMemberIdentity, ManagedOwnedSpanKind, ManagedPathSegment,
    ManagedRewrite, ManagedSpan, ManagedValue, PatchModuleArtifact, ProjectKey, SemanticOutputPath,
    SemanticSymbol, TemplateBinding, UnitLiteral, ValidatedPatchModuleArtifact,
};

/// Maximum transient source entries in one manifest. This is deliberately
/// lower than the parser value-node bound so manifest construction has an
/// independently exercisable resource ceiling.
pub const MANAGED_CONTROL_LIMIT: usize = crate::MANAGED_VALUE_NODE_LIMIT / 2;

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
    project: &'a CodeProject,
    _expansion: &'a ExpandedCodeProject,
    manifest: ManagedControlManifest,
}

impl ManagedControlAuthority<'_> {
    #[must_use]
    pub fn manifest(&self) -> &ManagedControlManifest {
        &self.manifest
    }

    /// Applies one exact-CAS batch against this freshly authenticated
    /// manifest without re-deriving its complete fan-out.
    ///
    /// # Errors
    ///
    /// Returns the ordinary batch shape, token, replacement, rewrite, parse,
    /// or project-validation failure without changing the borrowed project.
    pub fn apply_batch(
        &self,
        batch: &ManagedControlEditBatch,
    ) -> Result<CodeProject, ManagedControlError> {
        validate_control_batch_shape(batch)?;
        apply_managed_control_batch_against_manifest(self.project, &self.manifest, batch)
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
        project,
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
    Managed(#[from] crate::ManagedParseError),
    #[error(transparent)]
    Project(#[from] CodeProjectError),
}

type OwnedSpanIndex<'a> = BTreeMap<
    &'a SemanticSymbol,
    BTreeMap<&'a SemanticOutputPath, &'a crate::ManagedValueOwnedSpan>,
>;
type GeneratedProvenanceIndex<'a> = BTreeMap<
    &'a SemanticSymbol,
    BTreeMap<
        &'a str,
        BTreeMap<
            &'a [String],
            Vec<(
                &'a GeneratedMemberAddress,
                &'a crate::GeneratedIntentProvenance,
            )>,
        >,
    >,
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

fn generated_provenance_index(expansion: &ExpandedCodeProject) -> GeneratedProvenanceIndex<'_> {
    let mut index = GeneratedProvenanceIndex::new();
    for (address, provenance) in &expansion.generated_provenance {
        let Some(digest) = provenance.artifact_digest.as_deref() else {
            continue;
        };
        index
            .entry(&provenance.declaration)
            .or_default()
            .entry(digest)
            .or_default()
            .entry(address.template.as_slice())
            .or_default()
            .push((address, provenance));
    }
    index
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
    let provenance_index = generated_provenance_index(expansion);
    let owned_spans = owned_span_index(project)?;
    let mut consumer_map =
        BTreeMap::<(SemanticSymbol, SemanticOutputPath), BTreeSet<ManagedControlConsumer>>::new();
    let mut schema_map =
        BTreeMap::<(SemanticSymbol, SemanticOutputPath), Vec<ManagedControlSchema>>::new();
    let mut consumer_edges = 0_usize;

    for plan in &artifact_plans {
        for template in &plan.artifact.artifact().templates {
            for (property, binding) in &template.inputs {
                let TemplateBinding::Input {
                    name,
                    path,
                    expected_kind,
                } = binding
                else {
                    continue;
                };
                let source_path = invocation_input_path(name, path);
                let Some(value) = managed_value_at_path(&plan.declaration.arguments, &source_path)
                else {
                    continue;
                };
                let (source_declaration, source_path, source_value) =
                    scalar_source_owner(project, &plan.declaration.symbol, &source_path, value);
                if let Some(schema) = schema_for_template_input(
                    &template.declaration_family,
                    property,
                    *expected_kind,
                    source_value,
                ) {
                    schema_map
                        .entry((source_declaration.clone(), source_path.clone()))
                        .or_default()
                        .push(schema);
                }
                let provenance_rows = provenance_index
                    .get(&plan.declaration.symbol)
                    .and_then(|digests| digests.get(plan.digest.as_str()))
                    .and_then(|templates| templates.get(template.path.as_slice()));
                for (address, provenance) in provenance_rows
                    .into_iter()
                    .flat_map(|rows| rows.iter().copied())
                {
                    let inserted = consumer_map
                        .entry((source_declaration.clone(), source_path.clone()))
                        .or_default()
                        .insert(ManagedControlConsumer {
                            target: ManagedControlConsumerTarget::Generated {
                                address: address.clone(),
                                identity: provenance.identity,
                                artifact_digest: plan.digest.clone(),
                                family: template.declaration_family.clone(),
                            },
                            property: fields_path(&[property]),
                        });
                    if inserted {
                        admit_consumer_edge(&mut consumer_edges)?;
                    }
                }
            }
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
        let key = (binding.symbol.clone(), SemanticOutputPath::default());
        let consumers = consumer_map
            .remove(&key)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let schemas = schema_map.remove(&key).unwrap_or_default();
        let classification = classify_scalar_binding(&binding.value, &schemas);
        push_classified_control(
            &mut controls,
            project,
            &project_digest,
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
            let Some(value) = managed_value_at_path(&declaration.arguments, &owned.path) else {
                return Err(ManagedControlError::ForeignExpansion(format!(
                    "source path `{}` is absent",
                    path_text(&declaration.symbol, &owned.path)
                )));
            };
            let key = (declaration.symbol.clone(), owned.path.clone());
            let mut consumers = consumer_map
                .remove(&key)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            let classification = classify_control(
                declaration,
                &owned.path,
                value,
                schema_map.remove(&key).unwrap_or_default(),
                artifact_plan_index.get(&declaration.symbol).copied(),
            );
            if let ControlClassification::Editable(_) = &classification
                && declaration.patch.is_none()
            {
                let consumer = ManagedControlConsumer {
                    target: ManagedControlConsumerTarget::Declaration {
                        declaration: declaration.symbol.clone(),
                        family: declaration.builder_path.join("."),
                    },
                    property: owned.path.clone(),
                };
                if !consumers.contains(&consumer) {
                    admit_consumer_edge(&mut consumer_edges)?;
                    consumers.push(consumer);
                }
            }
            push_classified_control(
                &mut controls,
                project,
                &project_digest,
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
    controls.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(ManagedControlManifest {
        project: project.project.clone(),
        project_digest,
        source_digest: project.managed.source_digest.clone(),
        expansion_digest: expansion.digest.clone(),
        controls,
    })
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

/// Applies an unordered batch against the exact current manifest, validates
/// every token and typed replacement before rewriting, then reparses once.
/// The input project is never mutated and no partial candidate is returned.
///
/// # Errors
///
/// Returns a stale/foreign/read-only/typing/resource/parse/project failure
/// without changing `project`.
pub fn apply_managed_control_batch(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    batch: &ManagedControlEditBatch,
) -> Result<CodeProject, ManagedControlError> {
    validate_control_batch_shape(batch)?;
    let manifest = managed_control_manifest(project, expansion)?;
    apply_managed_control_batch_against_manifest(project, &manifest, batch)
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

fn apply_managed_control_batch_against_manifest(
    project: &CodeProject,
    manifest: &ManagedControlManifest,
    batch: &ManagedControlEditBatch,
) -> Result<CodeProject, ManagedControlError> {
    let mut seen = BTreeSet::new();
    let mut rewrites = Vec::with_capacity(batch.edits.len());
    let mut spans = Vec::with_capacity(batch.edits.len());
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
        let replacement = format_control_value(&edit.value).ok_or_else(|| {
            ManagedControlError::InvalidReplacement {
                control: edit.token.id.0.clone(),
                message: "replacement is not one editable scalar leaf".into(),
            }
        })?;
        rewrites.push(ManagedRewrite::new(
            &project.managed,
            control.source.span,
            replacement,
        ));
        spans.push((edit.token.id.0.as_str(), control.source.span));
    }
    validate_non_overlapping_control_spans(&mut spans)?;
    let managed = rewrite_managed_source_batch(&project.managed, &rewrites)?;
    let mut candidate = project.clone();
    candidate.managed = managed;
    candidate.validate()?;
    Ok(candidate)
}

/// Applies an exact batch against a previously inspected manifest. The
/// manifest is accepted only when it is byte-for-byte the current transient
/// derivation, so callers can bind an inspect response to a later edit request
/// without weakening source, project, or generation authentication.
///
/// # Errors
///
/// Returns [`ManagedControlError::StaleToken`] when any manifest authority has
/// changed, otherwise the ordinary batch validation failures.
pub fn apply_managed_control_manifest_batch(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    manifest: &ManagedControlManifest,
    batch: &ManagedControlEditBatch,
) -> Result<CodeProject, ManagedControlError> {
    validate_control_batch_shape(batch)?;
    let current = managed_control_manifest(project, expansion)?;
    if &current != manifest {
        let id = batch
            .edits
            .first()
            .map_or_else(|| "manifest".into(), |edit| edit.token.id.0.clone());
        return Err(ManagedControlError::StaleToken(id));
    }
    apply_managed_control_batch_against_manifest(project, &current, batch)
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

fn fields_path(fields: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        fields
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).to_owned()))
            .collect(),
    )
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

fn classify_control(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
    value: &ManagedValue,
    artifact_schemas: Vec<ManagedControlSchema>,
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
        ManagedValue::Null => {
            return read_only(ManagedControlReadOnlyReason::Null);
        }
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

    let direct = (declaration.patch.is_none())
        .then(|| direct_schema(declaration, path, value))
        .flatten();
    let mut schemas = artifact_schemas;
    if let Some(direct) = direct {
        schemas.push(direct);
    }
    if schemas.is_empty()
        && let Some(plan) = artifact
        && artifact_scalar_input_path(plan, path)
        && let Some(schema) = scalar_schema(value, ManagedControlNumberKind::Real, None, None)
    {
        schemas.push(schema);
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

fn direct_solver_instance_path(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
) -> bool {
    let family = declaration.builder_path.join(".");
    match family.as_str() {
        "geometry.line" => {
            path_starts_with_field(path, "start") || path_starts_with_field(path, "end")
        }
        "geometry.circle" => path_starts_with_field(path, "center"),
        "geometry.rectangle" => {
            path_starts_with_field(path, "lowerLeft") || path_starts_with_field(path, "upperRight")
        }
        "geometry.polyline" => path_contains_field(path, "position"),
        _ => false,
    }
}

fn direct_structural_identity_path(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
) -> bool {
    declaration.builder_path.join(".") == "geometry.polyline" && path_ends_with_field(path, "key")
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
        .flat_map(|template| template.inputs.values())
        .any(|binding| match binding {
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

fn artifact_scalar_input_path(plan: &ArtifactPlan<'_>, path: &SemanticOutputPath) -> bool {
    let [ManagedPathSegment::Field(input)] = path.0.as_slice() else {
        return false;
    };
    plan.artifact.artifact().inputs.get(input) == Some(&FeatureKind::Scalar)
}

fn direct_schema(
    declaration: &AuthoringDeclaration,
    path: &SemanticOutputPath,
    value: &ManagedValue,
) -> Option<ManagedControlSchema> {
    let family = declaration.builder_path.join(".");
    if path_ends_with_field(path, "suppressed") {
        return matches!(value, ManagedValue::Bool(_)).then_some(ManagedControlSchema::Boolean);
    }
    match family.as_str() {
        "geometry.line" if path_is_field(path, "role") => {
            choice_schema(value, &["profile", "construction"])
        }
        "geometry.circle" if path_is_field(path, "radius") => positive_scalar_schema(value),
        "geometry.polyline" if path_is_field(path, "closed") => {
            matches!(value, ManagedValue::Bool(_)).then_some(ManagedControlSchema::Boolean)
        }
        "constraint.fixedPoint" if path_starts_with_field(path, "target") && path.0.len() == 2 => {
            scalar_schema(value, ManagedControlNumberKind::Real, None, None)
        }
        "constraint.fixedCoordinate" | "constraint.symmetricAboutDatumAxis"
            if path_is_field(path, "axis") =>
        {
            choice_schema(value, &["x", "y"])
        }
        "constraint.fixedCoordinate" if path_is_field(path, "target") => {
            scalar_schema(value, ManagedControlNumberKind::Real, None, None)
        }
        "dimension.curveLength" | "dimension.radius" | "dimension.diameter"
            if path_is_field(path, "target") =>
        {
            positive_scalar_schema(value)
        }
        "dimension.curveLength" | "dimension.radius" | "dimension.diameter"
            if path_is_field(path, "mode") =>
        {
            choice_schema(value, &["driving", "reference"])
        }
        "computed.filletSet" => direct_fillet_schema(path, value),
        _ => None,
    }
}

fn direct_fillet_schema(
    path: &SemanticOutputPath,
    value: &ManagedValue,
) -> Option<ManagedControlSchema> {
    if path_is_field(path, "radius") {
        return positive_scalar_schema(value);
    }
    let field = last_field(path)?;
    match field {
        "endpointOrder" => choice_schema(value, &["firstThenSecond", "secondThenFirst"]),
        "sweep" => choice_schema(value, &["counterClockwise", "clockwise"]),
        "normalSide" => choice_schema(value, &["left", "right"]),
        "retainedEndpoint" => choice_schema(value, &["start", "end"]),
        "kind" if path_contains_field(path, "neighborhood") => {
            choice_schema(value, &["interior", "start", "end", "local"])
        }
        "winding" => scalar_schema(
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
        "parameter" | "lower" | "upper" => {
            scalar_schema(value, ManagedControlNumberKind::Real, None, None)
        }
        _ => None,
    }
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
        ("computed.fillet" | "geometry.circle", "radius")
            | ("dimension.radius", "target")
            | ("geometry.rectangle", "width" | "height" | "cornerRadius")
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

fn choice_schema(value: &ManagedValue, choices: &[&str]) -> Option<ManagedControlSchema> {
    matches!(value, ManagedValue::String(_)).then(|| ManagedControlSchema::Choice {
        choices: choices.iter().map(|choice| (*choice).to_owned()).collect(),
    })
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

fn format_control_value(value: &ManagedValue) -> Option<String> {
    match value {
        ManagedValue::Bool(value) => Some(value.to_string()),
        ManagedValue::Number(value) if value.is_finite() => Some(format_number(*value)),
        ManagedValue::String(value) => serde_json::to_string(value).ok(),
        ManagedValue::Unit(value) if value.value.is_finite() && valid_identifier(&value.unit) => {
            Some(format!("{}({})", value.unit, format_number(value.value)))
        }
        ManagedValue::Null
        | ManagedValue::Number(_)
        | ManagedValue::Array(_)
        | ManagedValue::Object(_)
        | ManagedValue::Reference { .. }
        | ManagedValue::Unit(_) => None,
    }
}

fn format_number(value: f64) -> String {
    if value == 0.0 && value.is_sign_negative() {
        "-0".into()
    } else {
        value.to_string()
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
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

fn path_contains_field(path: &SemanticOutputPath, expected: &str) -> bool {
    path.0
        .iter()
        .any(|segment| matches!(segment, ManagedPathSegment::Field(field) if field == expected))
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
            format_control_value(&value).as_deref(),
            Some("\"fixture text\\nwith a quote: \\\"\"")
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

        let declaration = AuthoringDeclaration {
            variable: "fixture".into(),
            symbol: SemanticSymbol("fixture".into()),
            builder_path: vec!["fixture".into()],
            arguments: ManagedValue::Number(2.0),
            patch: None,
            statement_span: ManagedSpan::new(0, 0),
            symbol_span: ManagedSpan::new(0, 0),
            arguments_span: ManagedSpan::new(0, 0),
        };
        assert!(matches!(
            classify_control(
                &declaration,
                &SemanticOutputPath::default(),
                &declaration.arguments,
                vec![real, disjoint],
                None,
            ),
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
