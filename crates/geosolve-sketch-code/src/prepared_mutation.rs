// SPDX-License-Identifier: GPL-3.0-or-later

//! Digest-bound two-phase authority for managed source transactions.
//!
//! Rust prepares either an exact structured mutation or an exact raw-source
//! replacement against one accepted code session. A browser or pinned native
//! compiler may compile that request, but its response becomes publishable
//! only after this module authenticates the ticket, the live accepted
//! authority, the complete managed compiler envelope and, for structured
//! mutations, the exact site-independent semantic delta. All validation is
//! side-effect free; callers receive an owned candidate only after every gate
//! succeeds.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use geosolve_sketch_intent::{
    IntentPortKind, IntentProjectionPath, IntentProjectionPathSegment, intent_content_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::declaration_catalog::CodeAuthoringInputBinding;
use crate::managed::direct_declaration_result_leaves;
use crate::{
    CANVAS_ADDITIONS_GROUP, CodeAuthoringArgumentKind, CodeAuthoringDeclarationKind,
    CodeAuthoringDynamicChildren, CodeOwnerAddress, CodeSessionIdentity, CodeWritableAddress,
    CodeWritableField, CompiledManagedSource, EditorSourceDeclarationDraft, ExecutedConsumerTarget,
    ExecutedGeneratedMemberAddress, MANAGED_SOURCE_LIMIT, MAX_CODE_SESSION_WIRE_INTEGER,
    ManagedExpression, ManagedIrImport, ManagedObjectField, ManagedPathSegment, ManagedReference,
    ManagedStatement, ManagedValidationError, ManagedValue, ProjectKey, SemanticOutputPath,
    SemanticSymbol, code_authoring_family, declaration_result_catalog,
    resolve_code_authoring_declaration,
};

/// Wire format for one Rust-prepared managed mutation ticket.
pub const PREPARED_MANAGED_MUTATION_FORMAT: &str = "geosolve-prepared-managed-mutation-v1";

/// Wire format for one Rust-prepared complete source replacement ticket.
pub const PREPARED_MANAGED_SOURCE_FORMAT: &str = "geosolve-prepared-managed-source-v1";

/// Largest declaration batch accepted by the TypeScript mutation host.
pub const MANAGED_MUTATION_BATCH_LIMIT: usize = 1_024;

/// Largest complete prepared request or response admitted at the Rust/host
/// boundary. It accommodates one maximum compiler envelope plus its bounded
/// exact mutation without inheriting an unbounded generic JSON payload.
pub const PREPARED_MANAGED_MUTATION_WIRE_LIMIT: usize = 64 * 1024 * 1024;

const MUTATION_PATH_LIMIT: usize = 128;
const MUTATION_VALUE_DEPTH_LIMIT: usize = 64;
const MUTATION_VALUE_NODE_LIMIT: usize = 16_384;
const MUTATION_COMMENT_LIMIT: usize = 1_024;
const MUTATION_STRING_LIMIT: usize = 16_384;
const MUTATION_STATEMENT_LIMIT: usize = 65_536;

/// Site-free semantic declaration accepted by the managed TypeScript
/// mutation API.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedDeclarationDraft {
    pub variable: String,
    pub symbol: String,
    pub builder_path: Vec<String>,
    pub arguments: ManagedValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppressed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<String>>,
}

impl From<EditorSourceDeclarationDraft> for ManagedDeclarationDraft {
    fn from(draft: EditorSourceDeclarationDraft) -> Self {
        Self {
            variable: draft.variable,
            symbol: draft.symbol.0,
            builder_path: draft.builder_path,
            arguments: draft.arguments,
            patch: None,
            group: Some(draft.group),
            suppressed: Some(draft.suppressed),
            comments: None,
        }
    }
}

/// Exact declaration or generated-member target understood by the
/// managed TypeScript mutation API.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedMutationTarget {
    Declaration {
        declaration: String,
    },
    Generated {
        address: ExecutedGeneratedMemberAddress,
    },
}

/// One exact owner/path compare-and-swap inside an atomic value batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedValueMutation {
    pub declaration: String,
    pub path: Vec<ManagedPathSegment>,
    pub expected: ManagedValue,
    pub value: ManagedValue,
}

/// Derives one exact lexical compare-and-swap mutation from a semantic source
/// coordinate.
///
/// Ordinary object fields and array indexes retain their spelling. A keyed
/// semantic member is reversible only when the expression at that step is a
/// well-formed keyed array: every item must be an object with exactly one
/// string `key`, all keys must be unique, and exactly one item must own the
/// requested key. The returned mutation records that item's current lexical
/// index and captures the selected expression as its CAS expectation.
///
/// # Errors
///
/// Rejects invalid compiler authority, absent or ambiguous declaration
/// symbols, malformed or unrepresentable semantic paths, unsupported current
/// expressions, and invalid replacement values.
pub fn derive_managed_value_mutation(
    current: &CompiledManagedSource,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    replacement: ManagedValue,
) -> Result<ManagedValueMutation, PreparedManagedMutationError> {
    current.validate().map_err(|error| {
        PreparedManagedMutationError::InvalidCurrent(format!(
            "managed compiler authority is invalid: {error}"
        ))
    })?;
    require_text(&declaration.0, "value owner declaration").map_err(semantic_error)?;
    validate_path(&path.0)?;

    let matches = current
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                symbol, arguments, ..
            } if symbol == &declaration.0 => Some(arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [arguments] = matches.as_slice() else {
        return semantic_refusal("managed value owner is absent or ambiguous");
    };
    let (lexical_path, selected) = resolve_semantic_source_path(arguments, &path.0)?;
    let expected = expression_to_managed_value(selected)?;

    let variables = current
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Binding { variable, .. }
            | ManagedStatement::Declaration { variable, .. } => Some(variable.clone()),
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let mut nodes = 0;
    validate_managed_value(&expected, &variables, 0, &mut nodes)?;
    let mut nodes = 0;
    validate_managed_value(&replacement, &variables, 0, &mut nodes)?;

    Ok(ManagedValueMutation {
        declaration: declaration.0.clone(),
        path: lexical_path,
        expected,
        value: replacement,
    })
}

fn resolve_semantic_source_path<'a>(
    mut expression: &'a ManagedExpression,
    path: &[ManagedPathSegment],
) -> Result<(Vec<ManagedPathSegment>, &'a ManagedExpression), PreparedManagedMutationError> {
    let mut lexical_path = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            ManagedPathSegment::Field(name) => {
                let ManagedExpression::Object { fields, .. } = expression else {
                    return semantic_refusal(
                        "managed semantic field does not address a lexical object",
                    );
                };
                let mut matches = fields.iter().filter(|field| field.name == *name);
                let Some(field) = matches.next() else {
                    return semantic_refusal("managed semantic field is absent");
                };
                if matches.next().is_some() {
                    return semantic_refusal("managed semantic field is ambiguous");
                }
                expression = &field.value;
                lexical_path.push(ManagedPathSegment::Field(name.clone()));
            }
            ManagedPathSegment::Index(index) => {
                let ManagedExpression::Array { values, .. } = expression else {
                    return semantic_refusal(
                        "managed semantic index does not address a lexical array",
                    );
                };
                let Some(value) = values.get(*index) else {
                    return semantic_refusal("managed semantic index is absent");
                };
                expression = value;
                lexical_path.push(ManagedPathSegment::Index(*index));
            }
            ManagedPathSegment::Member { member } => {
                let ManagedExpression::Array { values, .. } = expression else {
                    return semantic_refusal(
                        "managed keyed member does not address a lexical array",
                    );
                };
                let index = unique_keyed_member_index(values, member)?;
                expression = &values[index];
                lexical_path.push(ManagedPathSegment::Index(index));
            }
        }
    }
    Ok((lexical_path, expression))
}

fn unique_keyed_member_index(
    values: &[ManagedExpression],
    requested: &str,
) -> Result<usize, PreparedManagedMutationError> {
    let mut keys = BTreeSet::new();
    let mut selected = None;
    for (index, value) in values.iter().enumerate() {
        let ManagedExpression::Object { fields, .. } = value else {
            return semantic_refusal("managed keyed array contains a non-object item");
        };
        let mut key_fields = fields.iter().filter(|field| field.name == "key");
        let Some(key_field) = key_fields.next() else {
            return semantic_refusal("managed keyed array item has no key field");
        };
        if key_fields.next().is_some() {
            return semantic_refusal("managed keyed array item repeats its key field");
        }
        let ManagedExpression::String { value: key, .. } = &key_field.value else {
            return semantic_refusal("managed keyed array item has a non-string key");
        };
        if !keys.insert(key.as_str()) {
            return semantic_refusal("managed keyed array repeats a member key");
        }
        if key == requested {
            selected = Some(index);
        }
    }
    selected.ok_or_else(|| {
        PreparedManagedMutationError::SemanticDelta("managed keyed member is absent".into())
    })
}

/// Converts an authenticated direct writable-point bundle into exact
/// source-owned compare-and-swap mutations.
///
/// The writable addresses come from the accepted Rust expansion, while the
/// executed artifact must independently prove that each lexical point value
/// reached the same declaration/property at runtime. Descriptor-owned named
/// arguments bridge native result paths such as `controls[0]` back to source
/// names such as `firstControl`; the one keyed Polyline member shape is
/// resolved through its authenticated key rather than retained as an overlay.
///
/// # Errors
///
/// Rejects foreign projects, generated/runtime-only owners, ambiguous paths,
/// missing runtime provenance, non-point values and non-finite targets.
#[allow(
    clippy::too_many_lines,
    reason = "one bounded pass keeps point-owner, execution-provenance, and atomic source-coordinate authentication adjacent"
)]
pub fn managed_point_value_mutations(
    project: &ProjectKey,
    compiled: &CompiledManagedSource,
    placements: &[(CodeWritableAddress, [f64; 2])],
) -> Result<Vec<ManagedValueMutation>, PreparedManagedMutationError> {
    compiled.validate()?;
    if placements.is_empty() {
        return semantic_refusal("managed point mutation bundle is empty");
    }
    if placements.len() > MANAGED_MUTATION_BATCH_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "point mutation bundle has {} edits; the limit is {MANAGED_MUTATION_BATCH_LIMIT}",
            placements.len()
        )));
    }
    let mut seen = BTreeSet::new();
    let mut mutations = Vec::with_capacity(placements.len());
    let mut mutation_coordinates = BTreeSet::new();
    for (address, target) in placements {
        if &address.project != project {
            return semantic_refusal("managed point address belongs to another project");
        }
        if address.field != CodeWritableField::Point {
            return semantic_refusal("managed writable address is not a Cartesian point");
        }
        if target.iter().any(|value| !value.is_finite()) {
            return semantic_refusal("managed point target is not finite");
        }
        let declaration = point_source_declaration(&address.owner.address, &address.output)?;
        if !seen.insert((declaration.clone(), address.output.clone())) {
            return semantic_refusal("managed point bundle repeats one source coordinate");
        }
        validate_path(&address.output.0)?;
        let matches = compiled
            .ir
            .statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    arguments,
                    builder_path,
                    ..
                } if symbol == &declaration.0 => Some((arguments, builder_path.join("."))),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(arguments, family)] = matches.as_slice() else {
            return semantic_refusal("managed point source owner is absent or ambiguous");
        };
        let source_path = named_point_source_path(family, arguments, &address.output)?;
        let (lexical_path, current) = resolve_semantic_source_path(arguments, &source_path.0)?;
        let expected = expression_to_managed_value(current)?;
        let reversible_point = match &expected {
            ManagedValue::Reference { .. } => true,
            ManagedValue::Array(values) => matches!(
                values.as_slice(),
                [ManagedValue::Number(x), ManagedValue::Number(y)]
                    if x.is_finite() && y.is_finite()
            ),
            _ => false,
        };
        if !reversible_point {
            return semantic_refusal(
                "managed writable point is not a reversible reference or finite pair",
            );
        }
        authenticate_point_consumers(compiled, &declaration.0, family, &lexical_path, current)?;
        let mutation = ManagedValueMutation {
            declaration: declaration.0.clone(),
            path: lexical_path,
            expected,
            value: ManagedValue::Array(target.iter().copied().map(ManagedValue::Number).collect()),
        };
        if !mutation_coordinates.insert((mutation.declaration.clone(), mutation.path.clone())) {
            return semantic_refusal("managed point bundle repeats one lexical source coordinate");
        }
        mutations.push(mutation);
    }
    Ok(mutations)
}

fn point_source_declaration(
    owner: &CodeOwnerAddress,
    output: &SemanticOutputPath,
) -> Result<SemanticSymbol, PreparedManagedMutationError> {
    match owner {
        CodeOwnerAddress::DirectDeclaration { declaration } => Ok(declaration.clone()),
        CodeOwnerAddress::GeneratedMember { address }
            if address.template == ["polyline", "vertex"]
                && address.output == ["point"]
                && address.member_key.len() == 1
                && output.0
                    == [
                        ManagedPathSegment::Field("vertices".into()),
                        ManagedPathSegment::Member {
                            member: address.member_key[0].clone(),
                        },
                        ManagedPathSegment::Field("position".into()),
                    ] =>
        {
            Ok(SemanticSymbol(address.invocation.clone()))
        }
        CodeOwnerAddress::GeneratedMember { .. } => semantic_refusal(
            "generated managed point has no catalog-authenticated reversible source coordinate",
        ),
    }
}

fn named_point_source_path(
    family: &str,
    arguments: &ManagedExpression,
    output: &SemanticOutputPath,
) -> Result<SemanticOutputPath, PreparedManagedMutationError> {
    let (namespace, method) = family.split_once('.').ok_or_else(|| {
        PreparedManagedMutationError::SemanticDelta(
            "managed point owner is not one named declaration family".into(),
        )
    })?;
    let family_descriptor = code_authoring_family(namespace, method).ok_or_else(|| {
        PreparedManagedMutationError::SemanticDelta(format!(
            "managed point owner `{family}` is not in the named authoring catalog"
        ))
    })?;
    let dynamic_children = named_dynamic_children(arguments, family_descriptor.dynamic_children)?;
    let descriptor = resolve_code_authoring_declaration(namespace, method, dynamic_children)
        .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
    let CodeAuthoringDeclarationKind::Geometry(_) = descriptor.declaration else {
        return semantic_refusal("managed writable point owner is not named geometry");
    };

    let mut candidates = BTreeSet::new();
    for input in &descriptor.inputs {
        let (CodeAuthoringInputBinding::Slot { slot }, CodeAuthoringArgumentKind::Point) =
            (&input.binding, &input.kind)
        else {
            continue;
        };
        let source = SemanticOutputPath(vec![ManagedPathSegment::Field(input.name.clone())]);
        let input_projection = descriptor
            .declaration
            .intent_kind()
            .draft_input_projection_path(*slot, dynamic_children)
            .map(|path| managed_projection_path(&path));
        let native_output = descriptor.outputs.iter().any(|candidate| {
            candidate.alias_input == Some(*slot)
                && matches!(
                    candidate.kind,
                    IntentPortKind::Point | IntentPortKind::HandlePoint
                )
                && managed_projection_path(&candidate.path) == *output
        });
        if input_projection.as_ref() == Some(output) || native_output {
            candidates.insert(source);
        }
    }

    for field in &descriptor.fields {
        if field.schema.literal == geosolve_sketch_intent::IntentLiteralSchema::Point
            && managed_projection_path(&field.path) == *output
        {
            candidates.insert(output.clone());
        }
    }

    // Tangent Arc owns an explicit center seed even though that seed is not a
    // native point input: native authoring derives its center output from the
    // complete circular-arc instance. The clean named API nevertheless owns
    // the exact `center` source pair, so it remains one reversible value.
    if family == "geometry.tangentArc" && output.0 == [ManagedPathSegment::Field("center".into())] {
        candidates.insert(output.clone());
    }

    match descriptor.dynamic_children.kind {
        CodeAuthoringDynamicChildren::PolylineVertices
            if keyed_position_path(output, "vertices").is_some() =>
        {
            candidates.insert(output.clone());
        }
        CodeAuthoringDynamicChildren::SplineControls
            if keyed_position_path(output, "controls").is_some() =>
        {
            candidates.insert(output.clone());
        }
        CodeAuthoringDynamicChildren::None
        | CodeAuthoringDynamicChildren::FilletCorners
        | CodeAuthoringDynamicChildren::PatternInstances
        | CodeAuthoringDynamicChildren::PolylineVertices
        | CodeAuthoringDynamicChildren::SplineControls => {}
    }

    let mut candidates = candidates.into_iter();
    let Some(candidate) = candidates.next() else {
        return semantic_refusal(
            "managed writable point has no catalog-authenticated named source argument",
        );
    };
    if candidates.next().is_some() {
        return semantic_refusal("managed writable point has ambiguous named source arguments");
    }
    // Resolve here as a structural check. The caller repeats the lookup to
    // retain the selected expression for its exact CAS and provenance gate.
    resolve_semantic_source_path(arguments, &candidate.0)?;
    Ok(candidate)
}

fn keyed_position_path(path: &SemanticOutputPath, collection: &str) -> Option<()> {
    match path.0.as_slice() {
        [
            ManagedPathSegment::Field(actual),
            ManagedPathSegment::Member { member },
            ManagedPathSegment::Field(position),
        ] if actual == collection && !member.is_empty() && position == "position" => Some(()),
        _ => None,
    }
}

fn named_dynamic_children(
    arguments: &ManagedExpression,
    policy: CodeAuthoringDynamicChildren,
) -> Result<u16, PreparedManagedMutationError> {
    let field = |name: &str| exact_expression_field(arguments, name);
    let count = match policy {
        CodeAuthoringDynamicChildren::None => return Ok(0),
        CodeAuthoringDynamicChildren::PolylineVertices => match field("vertices")? {
            ManagedExpression::Array { values, .. } => values.len(),
            _ => return semantic_refusal("named Polyline vertices are not an array"),
        },
        CodeAuthoringDynamicChildren::SplineControls => match field("controls")? {
            ManagedExpression::Array { values, .. } => values.len(),
            _ => return semantic_refusal("named spline controls are not an array"),
        },
        CodeAuthoringDynamicChildren::FilletCorners => match field("corners")? {
            ManagedExpression::Array { values, .. } => values.len(),
            _ => return semantic_refusal("named Fillet corners are not an array"),
        },
        CodeAuthoringDynamicChildren::PatternInstances => match field("instances")? {
            ManagedExpression::Number { value, .. }
                if value.is_finite()
                    && !value.is_sign_negative()
                    && value.fract() == 0.0
                    && *value <= f64::from(u16::MAX) =>
            {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the finite integral value is bounded to u16 immediately above"
                )]
                return Ok(*value as u16);
            }
            _ => return semantic_refusal("named pattern instance count is not an exact u16"),
        },
    };
    u16::try_from(count).map_err(|_| {
        PreparedManagedMutationError::SemanticDelta(
            "named declaration dynamic child count exceeds u16".into(),
        )
    })
}

fn exact_expression_field<'a>(
    expression: &'a ManagedExpression,
    name: &str,
) -> Result<&'a ManagedExpression, PreparedManagedMutationError> {
    let ManagedExpression::Object { fields, .. } = expression else {
        return semantic_refusal("named declaration arguments are not an object");
    };
    let mut matching = fields.iter().filter(|field| field.name == name);
    let Some(field) = matching.next() else {
        return semantic_refusal(format!("named declaration is missing `{name}`"));
    };
    if matching.next().is_some() {
        return semantic_refusal(format!("named declaration repeats `{name}`"));
    }
    Ok(&field.value)
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

fn authenticate_point_consumers(
    compiled: &CompiledManagedSource,
    declaration: &str,
    family: &str,
    path: &[ManagedPathSegment],
    expression: &ManagedExpression,
) -> Result<(), PreparedManagedMutationError> {
    let relevant = compiled
        .artifact
        .value_consumers
        .iter()
        .filter(|consumer| {
            matches!(
                &consumer.target,
                ExecutedConsumerTarget::Declaration {
                    declaration: candidate,
                    family: candidate_family,
                } if candidate == declaration && candidate_family == family
            ) && (consumer.property == path
                || consumer
                    .property
                    .strip_prefix(path)
                    .is_some_and(|suffix| matches!(suffix, [ManagedPathSegment::Index(0 | 1)])))
        })
        .collect::<Vec<_>>();
    let source_sites = compiled
        .ir
        .source_sites
        .iter()
        .map(|site| site.id.as_str())
        .collect::<BTreeSet<_>>();
    if relevant.is_empty()
        || relevant
            .iter()
            .any(|consumer| !source_sites.contains(consumer.value_site.as_str()))
    {
        return semantic_refusal(
            "managed point source lacks authenticated runtime consumer provenance",
        );
    }
    match expression {
        ManagedExpression::Array { values, .. } => {
            let [
                ManagedExpression::Number { site: x_site, .. },
                ManagedExpression::Number { site: y_site, .. },
            ] = values.as_slice()
            else {
                return semantic_refusal("managed point source is not a finite numeric pair");
            };
            for (index, site) in [(0, x_site), (1, y_site)] {
                let mut property = path.to_vec();
                property.push(ManagedPathSegment::Index(index));
                let exact = relevant
                    .iter()
                    .filter(|consumer| {
                        consumer.property == property && consumer.value_site == *site
                    })
                    .count();
                if exact != 1 {
                    return semantic_refusal(
                        "managed point component has ambiguous runtime provenance",
                    );
                }
            }
        }
        ManagedExpression::Reference { site, .. } => {
            let exact = relevant
                .iter()
                .filter(|consumer| consumer.property == path && consumer.value_site == *site)
                .count();
            if exact != 1 || relevant.len() != 1 {
                return semantic_refusal(
                    "managed point reference lacks one exact lexical consumer provenance edge",
                );
            }
        }
        _ => {
            return semantic_refusal(
                "managed point source is not a reversible reference or finite pair",
            );
        }
    }
    Ok(())
}

/// Closed mutation vocabulary shared byte-for-byte with TypeScript.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mutation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedSketchMutation {
    InsertDeclarations {
        declarations: Vec<ManagedDeclarationDraft>,
    },
    ReorderDeclaration {
        declaration: String,
        before: Option<String>,
    },
    SetSuppressed {
        target: ManagedMutationTarget,
        suppressed: bool,
    },
    SetValue {
        declaration: String,
        path: Vec<ManagedPathSegment>,
        expected: ManagedValue,
        value: ManagedValue,
    },
    SetValues {
        values: Vec<ManagedValueMutation>,
    },
    Delete {
        target: ManagedMutationTarget,
    },
}

/// Independently requested lifecycle action refused for a source-owned
/// declaration helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedSourceDeclarationHelperMutation {
    Move,
    ReorderDestination,
    Suppress,
    Delete,
}

impl std::fmt::Display for ManagedSourceDeclarationHelperMutation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Move => "moved",
            Self::ReorderDestination => "used as a reorder destination",
            Self::Suppress => "suppressed",
            Self::Delete => "deleted",
        })
    }
}

/// Complete accepted identity against which a mutation may be prepared.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedMutationAuthority {
    pub project: ProjectKey,
    pub session: CodeSessionIdentity,
    pub accepted_source_digest: String,
    pub accepted_ir_digest: String,
    pub accepted_artifact_digest: String,
    pub accepted_expansion_digest: String,
    pub declaration_name_high_water: u64,
}

impl ManagedMutationAuthority {
    /// Constructs one accepted authority directly from a validated compiled
    /// managed owner.
    ///
    /// # Errors
    ///
    /// Rejects malformed identities, unsafe JavaScript integers, and invalid
    /// compiler envelopes before a ticket can be issued.
    pub fn new(
        project: ProjectKey,
        session: CodeSessionIdentity,
        accepted_expansion_digest: String,
        declaration_name_high_water: u64,
        compiled: &CompiledManagedSource,
    ) -> Result<Self, PreparedManagedMutationError> {
        compiled.validate().map_err(|error| {
            PreparedManagedMutationError::InvalidAuthority(format!(
                "accepted managed compiler authority is invalid: {error}"
            ))
        })?;
        let authority = Self {
            project,
            session,
            accepted_source_digest: compiled.ir.source_digest.clone(),
            accepted_ir_digest: compiled.ir.ir_digest.clone(),
            accepted_artifact_digest: compiled.artifact.artifact_digest.clone(),
            accepted_expansion_digest,
            declaration_name_high_water,
        };
        authority.validate()?;
        Ok(authority)
    }

    fn validate(&self) -> Result<(), PreparedManagedMutationError> {
        require_text(&self.project.0, "project key").map_err(invalid_authority)?;
        if self.session.session == 0
            || self.session.session > MAX_CODE_SESSION_WIRE_INTEGER
            || self.session.revision > MAX_CODE_SESSION_WIRE_INTEGER
        {
            return Err(PreparedManagedMutationError::InvalidAuthority(
                "code-session identity is outside the exact JavaScript integer range".into(),
            ));
        }
        for (label, digest) in [
            ("session", self.session.digest.as_str()),
            ("accepted source", self.accepted_source_digest.as_str()),
            ("accepted IR", self.accepted_ir_digest.as_str()),
            ("accepted artifact", self.accepted_artifact_digest.as_str()),
            (
                "accepted expansion",
                self.accepted_expansion_digest.as_str(),
            ),
        ] {
            require_digest(digest, label).map_err(invalid_authority)?;
        }
        if self.declaration_name_high_water > MAX_CODE_SESSION_WIRE_INTEGER {
            return Err(PreparedManagedMutationError::InvalidAuthority(
                "declaration-name high-water is outside the exact JavaScript integer range".into(),
            ));
        }
        Ok(())
    }
}

/// Digest-authenticated permission for one exact semantic mutation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedManagedMutationTicket {
    pub format: String,
    pub ticket_digest: String,
    pub project: ProjectKey,
    pub session: CodeSessionIdentity,
    pub accepted_source_digest: String,
    pub accepted_ir_digest: String,
    pub accepted_artifact_digest: String,
    pub accepted_expansion_digest: String,
    pub declaration_name_high_water: u64,
    pub candidate_declaration_name_high_water: u64,
    pub base_semantics_digest: String,
    pub candidate_semantics_digest: String,
    pub mutation: ManagedSketchMutation,
}

/// Browser/Deno request containing the exact accepted compiler envelope and
/// the only mutation it is permitted to apply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedManagedMutationRequest {
    pub ticket: PreparedManagedMutationTicket,
    pub current: CompiledManagedSource,
}

/// Digest-authenticated permission to compile one exact raw managed-source
/// candidate against one accepted project identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedManagedSourceTicket {
    pub format: String,
    pub ticket_digest: String,
    pub project: ProjectKey,
    pub session: CodeSessionIdentity,
    pub accepted_source_digest: String,
    pub accepted_ir_digest: String,
    pub accepted_artifact_digest: String,
    pub accepted_expansion_digest: String,
    pub declaration_name_high_water: u64,
    pub candidate_input_source_digest: String,
}

/// Complete request sent to a browser or pinned Deno compiler for an exact
/// raw-source replacement. The accepted compiler envelope is retained so
/// Rust can authenticate compare-and-swap identity at resolution time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedManagedSourceRequest {
    pub ticket: PreparedManagedSourceTicket,
    pub current: CompiledManagedSource,
    pub candidate_source: String,
}

/// Exact receipt returned by `applyManagedSketchMutation` in TypeScript.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedMutationReceipt {
    pub base_source_digest: String,
    pub candidate_source_digest: String,
    pub compiled: CompiledManagedSource,
}

/// Asynchronous host response bound back to the prepared ticket which
/// authorized it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedManagedMutationReceipt {
    pub ticket_digest: String,
    pub base_source_digest: String,
    pub candidate_source_digest: String,
    pub compiled: CompiledManagedSource,
}

impl PreparedManagedMutationReceipt {
    /// Attaches a TypeScript receipt to the digest of the request which
    /// initiated that compiler invocation.
    #[must_use]
    pub fn new(ticket_digest: String, receipt: ManagedMutationReceipt) -> Self {
        Self {
            ticket_digest,
            base_source_digest: receipt.base_source_digest,
            candidate_source_digest: receipt.candidate_source_digest,
            compiled: receipt.compiled,
        }
    }
}

/// Candidate authority returned only after the complete prepared transaction
/// has been authenticated. Constructing this type performs no publication.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedManagedMutation {
    ticket_digest: String,
    project: ProjectKey,
    session: CodeSessionIdentity,
    mutation: ManagedSketchMutation,
    declaration_name_high_water: u64,
    compiled: CompiledManagedSource,
}

/// Candidate compiler authority returned only after an exact raw-source
/// request and receipt have both been authenticated. Constructing this type
/// performs no project, native-scene or history publication.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedManagedSource {
    ticket_digest: String,
    project: ProjectKey,
    session: CodeSessionIdentity,
    declaration_name_high_water: u64,
    compiled: CompiledManagedSource,
}

impl ValidatedManagedSource {
    #[must_use]
    pub fn ticket_digest(&self) -> &str {
        &self.ticket_digest
    }

    #[must_use]
    pub const fn project(&self) -> &ProjectKey {
        &self.project
    }

    #[must_use]
    pub const fn session(&self) -> &CodeSessionIdentity {
        &self.session
    }

    #[must_use]
    pub const fn declaration_name_high_water(&self) -> u64 {
        self.declaration_name_high_water
    }

    #[must_use]
    pub const fn compiled(&self) -> &CompiledManagedSource {
        &self.compiled
    }

    #[must_use]
    pub fn into_compiled(self) -> CompiledManagedSource {
        self.compiled
    }
}

impl ValidatedManagedMutation {
    #[must_use]
    pub fn ticket_digest(&self) -> &str {
        &self.ticket_digest
    }

    #[must_use]
    pub const fn project(&self) -> &ProjectKey {
        &self.project
    }

    #[must_use]
    pub const fn session(&self) -> &CodeSessionIdentity {
        &self.session
    }

    #[must_use]
    pub const fn mutation(&self) -> &ManagedSketchMutation {
        &self.mutation
    }

    #[must_use]
    pub const fn declaration_name_high_water(&self) -> u64 {
        self.declaration_name_high_water
    }

    #[must_use]
    pub const fn compiled(&self) -> &CompiledManagedSource {
        &self.compiled
    }

    #[must_use]
    pub fn into_compiled(self) -> CompiledManagedSource {
        self.compiled
    }
}

/// Transactional refusal from the prepared managed boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum PreparedManagedMutationError {
    #[error("invalid accepted managed authority: {0}")]
    InvalidAuthority(String),
    #[error("invalid prepared managed ticket: {0}")]
    InvalidTicket(String),
    #[error("prepared managed ticket is stale: {0}")]
    StaleAuthority(String),
    #[error("invalid accepted managed compiler envelope: {0}")]
    InvalidCurrent(String),
    #[error("invalid managed mutation receipt: {0}")]
    InvalidReceipt(String),
    #[error("managed mutation receipt exceeds its exact semantic delta: {0}")]
    SemanticDelta(String),
    #[error(
        "Profile Offset helper {helper} cannot be {mutation} independently; mutate its Profile Offset root {root}"
    )]
    OwnedHelperMutation {
        helper: String,
        root: String,
        mutation: ManagedSourceDeclarationHelperMutation,
    },
    #[error("managed mutation exceeds a resource bound: {0}")]
    ResourceLimit(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TicketDigestEnvelope<'a> {
    format: &'a str,
    project: &'a ProjectKey,
    session: &'a CodeSessionIdentity,
    accepted_source_digest: &'a str,
    accepted_ir_digest: &'a str,
    accepted_artifact_digest: &'a str,
    accepted_expansion_digest: &'a str,
    declaration_name_high_water: u64,
    candidate_declaration_name_high_water: u64,
    base_semantics_digest: &'a str,
    candidate_semantics_digest: &'a str,
    mutation: &'a ManagedSketchMutation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceTicketDigestEnvelope<'a> {
    format: &'a str,
    project: &'a ProjectKey,
    session: &'a CodeSessionIdentity,
    accepted_source_digest: &'a str,
    accepted_ir_digest: &'a str,
    accepted_artifact_digest: &'a str,
    accepted_expansion_digest: &'a str,
    declaration_name_high_water: u64,
    candidate_input_source_digest: &'a str,
}

/// Prepares one exact managed compiler-host request without changing any
/// caller-owned authority.
///
/// # Errors
///
/// Rejects stale compiler identities, malformed or impossible operations,
/// and non-monotonic declaration-name allocation before a host is invoked.
pub fn prepare_managed_mutation(
    authority: &ManagedMutationAuthority,
    current: &CompiledManagedSource,
    mutation: ManagedSketchMutation,
    candidate_declaration_name_high_water: u64,
) -> Result<PreparedManagedMutationRequest, PreparedManagedMutationError> {
    authority.validate()?;
    validate_current(authority, current)?;
    validate_high_water(
        authority.declaration_name_high_water,
        candidate_declaration_name_high_water,
        &mutation,
    )?;
    let expected = apply_expected_mutation(current, &mutation)?;
    let base_semantics_digest = semantic_ir_digest(&current.ir.imports, &current.ir.statements)?;
    let candidate_semantics_digest = semantic_ir_digest(&expected.imports, &expected.statements)?;
    let mut ticket = PreparedManagedMutationTicket {
        format: PREPARED_MANAGED_MUTATION_FORMAT.into(),
        ticket_digest: String::new(),
        project: authority.project.clone(),
        session: authority.session.clone(),
        accepted_source_digest: authority.accepted_source_digest.clone(),
        accepted_ir_digest: authority.accepted_ir_digest.clone(),
        accepted_artifact_digest: authority.accepted_artifact_digest.clone(),
        accepted_expansion_digest: authority.accepted_expansion_digest.clone(),
        declaration_name_high_water: authority.declaration_name_high_water,
        candidate_declaration_name_high_water,
        base_semantics_digest,
        candidate_semantics_digest,
        mutation,
    };
    ticket.ticket_digest = ticket_digest(&ticket)?;
    let request = PreparedManagedMutationRequest {
        ticket,
        current: current.clone(),
    };
    validate_wire_size(&request, "prepared mutation request")?;
    Ok(request)
}

/// Prepares one exact raw-source compiler request without changing any
/// caller-owned authority.
///
/// Unlike a structured mutation, a deliberate whole-source edit has no
/// precompiled semantic delta for Rust to compare. Its ticket instead binds
/// the exact candidate input bytes to the complete accepted source, IR,
/// artifact, expansion, session and allocator identity. Resolution still
/// validates the complete V3 compiler envelope before any caller may attempt
/// native materialization or publication.
///
/// # Errors
///
/// Rejects stale accepted authority, malformed compiler authority, an
/// unchanged source candidate, and source/request resource-limit violations.
pub fn prepare_managed_source(
    authority: &ManagedMutationAuthority,
    current: &CompiledManagedSource,
    candidate_source: String,
) -> Result<PreparedManagedSourceRequest, PreparedManagedMutationError> {
    authority.validate()?;
    validate_current(authority, current)?;
    if candidate_source.len() > MANAGED_SOURCE_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "candidate managed source is {} bytes; the limit is {MANAGED_SOURCE_LIMIT}",
            candidate_source.len()
        )));
    }
    let candidate_input_source_digest =
        intent_content_digest(candidate_source.as_bytes()).to_string();
    if candidate_input_source_digest == current.input_source_digest
        && candidate_source == current.normalized_source
    {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "raw-source replacement is unchanged".into(),
        ));
    }
    let mut ticket = PreparedManagedSourceTicket {
        format: PREPARED_MANAGED_SOURCE_FORMAT.into(),
        ticket_digest: String::new(),
        project: authority.project.clone(),
        session: authority.session.clone(),
        accepted_source_digest: authority.accepted_source_digest.clone(),
        accepted_ir_digest: authority.accepted_ir_digest.clone(),
        accepted_artifact_digest: authority.accepted_artifact_digest.clone(),
        accepted_expansion_digest: authority.accepted_expansion_digest.clone(),
        declaration_name_high_water: authority.declaration_name_high_water,
        candidate_input_source_digest,
    };
    ticket.ticket_digest = source_ticket_digest(&ticket)?;
    let request = PreparedManagedSourceRequest {
        ticket,
        current: current.clone(),
        candidate_source,
    };
    validate_wire_size(&request, "prepared source request")?;
    Ok(request)
}

/// Authenticates one compiler-host receipt for an exact Rust-prepared raw
/// source candidate against the current live accepted authority.
///
/// # Errors
///
/// Stale sessions, replayed or tampered tickets, wrong source bytes/digests,
/// malformed V3 compiler envelopes and oversized requests or receipts reject
/// without returning a publishable candidate.
pub fn validate_prepared_managed_source(
    live: &ManagedMutationAuthority,
    request: &PreparedManagedSourceRequest,
    receipt: PreparedManagedMutationReceipt,
) -> Result<ValidatedManagedSource, PreparedManagedMutationError> {
    validate_source_request(live, request)?;
    if receipt.ticket_digest != request.ticket.ticket_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "response belongs to a different prepared source ticket".into(),
        ));
    }
    validate_wire_size(&receipt, "prepared source receipt")?;
    require_digest(&receipt.base_source_digest, "receipt base source").map_err(invalid_receipt)?;
    require_digest(&receipt.candidate_source_digest, "receipt candidate source")
        .map_err(invalid_receipt)?;
    if receipt.base_source_digest != request.ticket.accepted_source_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "response names a different accepted source".into(),
        ));
    }
    if request.ticket.candidate_input_source_digest != receipt.compiled.input_source_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "response belongs to different raw source bytes".into(),
        ));
    }
    if receipt.candidate_source_digest != receipt.compiled.ir.source_digest
        || receipt.candidate_source_digest != receipt.compiled.artifact.source_digest
    {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "candidate digest does not authenticate normalized source bytes".into(),
        ));
    }
    receipt
        .compiled
        .validate_input_source(&request.candidate_source)
        .map_err(|error| PreparedManagedMutationError::InvalidReceipt(error.to_string()))?;
    Ok(ValidatedManagedSource {
        ticket_digest: request.ticket.ticket_digest.clone(),
        project: request.ticket.project.clone(),
        session: request.ticket.session.clone(),
        declaration_name_high_water: request.ticket.declaration_name_high_water,
        compiled: receipt.compiled,
    })
}

/// Authenticates one compiler-host receipt against both the originally
/// prepared request and the current live accepted authority.
///
/// # Errors
///
/// Stale sessions, replayed or tampered tickets, wrong base/candidate
/// digests, invalid compiler envelopes, and any unrelated semantic change are
/// rejected. The function is side-effect free on every path.
pub fn validate_prepared_managed_mutation(
    live: &ManagedMutationAuthority,
    request: &PreparedManagedMutationRequest,
    receipt: PreparedManagedMutationReceipt,
) -> Result<ValidatedManagedMutation, PreparedManagedMutationError> {
    validate_request(live, request)?;
    if receipt.ticket_digest != request.ticket.ticket_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "response belongs to a different prepared ticket".into(),
        ));
    }
    validate_wire_size(&receipt, "prepared mutation receipt")?;
    require_digest(&receipt.base_source_digest, "receipt base source").map_err(invalid_receipt)?;
    require_digest(&receipt.candidate_source_digest, "receipt candidate source")
        .map_err(invalid_receipt)?;
    if receipt.base_source_digest != request.ticket.accepted_source_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "response names a different accepted source".into(),
        ));
    }
    receipt.compiled.validate().map_err(|error| {
        PreparedManagedMutationError::InvalidReceipt(format!(
            "candidate compiler envelope is invalid: {error}"
        ))
    })?;
    if receipt.candidate_source_digest != receipt.compiled.ir.source_digest
        || receipt.candidate_source_digest != receipt.compiled.artifact.source_digest
        || receipt.candidate_source_digest != receipt.compiled.input_source_digest
    {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "candidate digest does not authenticate the canonically recompiled source".into(),
        ));
    }

    let expected = apply_expected_mutation(&request.current, &request.ticket.mutation)?;
    let actual_semantics = semantic_ir_digest(
        &receipt.compiled.ir.imports,
        &receipt.compiled.ir.statements,
    )?;
    if actual_semantics != request.ticket.candidate_semantics_digest
        || expected.imports != receipt.compiled.ir.imports
        || !same_semantic_statements(&expected.statements, &receipt.compiled.ir.statements)?
    {
        return Err(PreparedManagedMutationError::SemanticDelta(
            "candidate IR differs from the one operation authorized by Rust".into(),
        ));
    }
    let base_semantics =
        semantic_ir_digest(&request.current.ir.imports, &request.current.ir.statements)?;
    if actual_semantics == base_semantics {
        if !same_compiled_authority(&request.current, &receipt.compiled) {
            return Err(PreparedManagedMutationError::SemanticDelta(
                "a semantic no-op changed normalized source or executed authority".into(),
            ));
        }
    } else if receipt.candidate_source_digest == receipt.base_source_digest {
        return Err(PreparedManagedMutationError::InvalidReceipt(
            "a semantic mutation reused the accepted source digest".into(),
        ));
    }
    validate_artifact_delta(
        &request.current,
        &receipt.compiled,
        &request.ticket.mutation,
    )?;

    Ok(ValidatedManagedMutation {
        ticket_digest: request.ticket.ticket_digest.clone(),
        project: request.ticket.project.clone(),
        session: request.ticket.session.clone(),
        mutation: request.ticket.mutation.clone(),
        declaration_name_high_water: request.ticket.candidate_declaration_name_high_water,
        compiled: receipt.compiled,
    })
}

fn validate_request(
    live: &ManagedMutationAuthority,
    request: &PreparedManagedMutationRequest,
) -> Result<(), PreparedManagedMutationError> {
    validate_wire_size(request, "prepared mutation request")?;
    live.validate()?;
    validate_ticket(&request.ticket)?;
    let ticket_authority = ManagedMutationAuthority {
        project: request.ticket.project.clone(),
        session: request.ticket.session.clone(),
        accepted_source_digest: request.ticket.accepted_source_digest.clone(),
        accepted_ir_digest: request.ticket.accepted_ir_digest.clone(),
        accepted_artifact_digest: request.ticket.accepted_artifact_digest.clone(),
        accepted_expansion_digest: request.ticket.accepted_expansion_digest.clone(),
        declaration_name_high_water: request.ticket.declaration_name_high_water,
    };
    if &ticket_authority != live {
        return Err(PreparedManagedMutationError::StaleAuthority(
            "project, code session, source, IR, artifact, expansion, or name allocator changed"
                .into(),
        ));
    }
    validate_current(live, &request.current)?;
    validate_high_water(
        request.ticket.declaration_name_high_water,
        request.ticket.candidate_declaration_name_high_water,
        &request.ticket.mutation,
    )?;
    let expected = apply_expected_mutation(&request.current, &request.ticket.mutation)?;
    let base_semantics =
        semantic_ir_digest(&request.current.ir.imports, &request.current.ir.statements)?;
    let candidate_semantics = semantic_ir_digest(&expected.imports, &expected.statements)?;
    if base_semantics != request.ticket.base_semantics_digest
        || candidate_semantics != request.ticket.candidate_semantics_digest
    {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "ticket semantic fingerprints do not match its accepted owner and operation".into(),
        ));
    }
    Ok(())
}

fn validate_source_request(
    live: &ManagedMutationAuthority,
    request: &PreparedManagedSourceRequest,
) -> Result<(), PreparedManagedMutationError> {
    validate_wire_size(request, "prepared source request")?;
    validate_source_ticket(&request.ticket)?;
    let ticket_authority = ManagedMutationAuthority {
        project: request.ticket.project.clone(),
        session: request.ticket.session.clone(),
        accepted_source_digest: request.ticket.accepted_source_digest.clone(),
        accepted_ir_digest: request.ticket.accepted_ir_digest.clone(),
        accepted_artifact_digest: request.ticket.accepted_artifact_digest.clone(),
        accepted_expansion_digest: request.ticket.accepted_expansion_digest.clone(),
        declaration_name_high_water: request.ticket.declaration_name_high_water,
    };
    if &ticket_authority != live {
        return Err(PreparedManagedMutationError::StaleAuthority(
            "project, code session, source, IR, artifact, expansion, or name allocator changed"
                .into(),
        ));
    }
    validate_current(live, &request.current)?;
    if request.candidate_source.len() > MANAGED_SOURCE_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "candidate managed source is {} bytes; the limit is {MANAGED_SOURCE_LIMIT}",
            request.candidate_source.len()
        )));
    }
    if intent_content_digest(request.candidate_source.as_bytes()).to_string()
        != request.ticket.candidate_input_source_digest
    {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "candidate source bytes do not match the prepared digest".into(),
        ));
    }
    Ok(())
}

fn validate_ticket(
    ticket: &PreparedManagedMutationTicket,
) -> Result<(), PreparedManagedMutationError> {
    if ticket.format != PREPARED_MANAGED_MUTATION_FORMAT {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "unsupported prepared-mutation format".into(),
        ));
    }
    require_digest(&ticket.ticket_digest, "ticket").map_err(invalid_ticket)?;
    require_digest(&ticket.base_semantics_digest, "base semantics").map_err(invalid_ticket)?;
    require_digest(&ticket.candidate_semantics_digest, "candidate semantics")
        .map_err(invalid_ticket)?;
    if ticket_digest(ticket)? != ticket.ticket_digest {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "ticket digest mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_source_ticket(
    ticket: &PreparedManagedSourceTicket,
) -> Result<(), PreparedManagedMutationError> {
    if ticket.format != PREPARED_MANAGED_SOURCE_FORMAT {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "unsupported prepared-source format".into(),
        ));
    }
    require_digest(&ticket.ticket_digest, "source ticket").map_err(invalid_ticket)?;
    require_digest(
        &ticket.candidate_input_source_digest,
        "candidate input source",
    )
    .map_err(invalid_ticket)?;
    if ticket.declaration_name_high_water > MAX_CODE_SESSION_WIRE_INTEGER {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "source ticket declaration-name high-water is not wire-safe".into(),
        ));
    }
    if source_ticket_digest(ticket)? != ticket.ticket_digest {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "source ticket digest mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_current(
    authority: &ManagedMutationAuthority,
    current: &CompiledManagedSource,
) -> Result<(), PreparedManagedMutationError> {
    current
        .validate()
        .map_err(|error| PreparedManagedMutationError::InvalidCurrent(error.to_string()))?;
    if current.ir.source_digest != authority.accepted_source_digest
        || current.ir.ir_digest != authority.accepted_ir_digest
        || current.artifact.artifact_digest != authority.accepted_artifact_digest
    {
        return Err(PreparedManagedMutationError::StaleAuthority(
            "compiled source, IR, or artifact does not match accepted authority".into(),
        ));
    }
    Ok(())
}

fn validate_high_water(
    current: u64,
    candidate: u64,
    mutation: &ManagedSketchMutation,
) -> Result<(), PreparedManagedMutationError> {
    if candidate > MAX_CODE_SESSION_WIRE_INTEGER || candidate < current {
        return Err(PreparedManagedMutationError::InvalidTicket(
            "candidate declaration-name high-water is non-monotonic or not wire-safe".into(),
        ));
    }
    match mutation {
        ManagedSketchMutation::InsertDeclarations { declarations } => {
            let minimum = current
                .checked_add(declarations.len() as u64)
                .ok_or_else(|| {
                    PreparedManagedMutationError::ResourceLimit(
                        "declaration-name high-water overflow".into(),
                    )
                })?;
            if candidate < minimum {
                return Err(PreparedManagedMutationError::InvalidTicket(
                    "insertion did not advance the declaration-name allocator once per declaration"
                        .into(),
                ));
            }
        }
        ManagedSketchMutation::ReorderDeclaration { .. }
        | ManagedSketchMutation::SetSuppressed { .. }
        | ManagedSketchMutation::SetValue { .. }
        | ManagedSketchMutation::SetValues { .. }
        | ManagedSketchMutation::Delete { .. } => {
            if candidate != current {
                return Err(PreparedManagedMutationError::InvalidTicket(
                    "non-insertion mutation changed the declaration-name high-water".into(),
                ));
            }
        }
    }
    Ok(())
}

fn ticket_digest(
    ticket: &PreparedManagedMutationTicket,
) -> Result<String, PreparedManagedMutationError> {
    let envelope = TicketDigestEnvelope {
        format: &ticket.format,
        project: &ticket.project,
        session: &ticket.session,
        accepted_source_digest: &ticket.accepted_source_digest,
        accepted_ir_digest: &ticket.accepted_ir_digest,
        accepted_artifact_digest: &ticket.accepted_artifact_digest,
        accepted_expansion_digest: &ticket.accepted_expansion_digest,
        declaration_name_high_water: ticket.declaration_name_high_water,
        candidate_declaration_name_high_water: ticket.candidate_declaration_name_high_water,
        base_semantics_digest: &ticket.base_semantics_digest,
        candidate_semantics_digest: &ticket.candidate_semantics_digest,
        mutation: &ticket.mutation,
    };
    serde_json::to_vec(&envelope)
        .map(|bytes| intent_content_digest(&bytes).to_string())
        .map_err(|error| {
            PreparedManagedMutationError::InvalidTicket(format!(
                "ticket cannot be encoded canonically: {error}"
            ))
        })
}

fn source_ticket_digest(
    ticket: &PreparedManagedSourceTicket,
) -> Result<String, PreparedManagedMutationError> {
    let envelope = SourceTicketDigestEnvelope {
        format: &ticket.format,
        project: &ticket.project,
        session: &ticket.session,
        accepted_source_digest: &ticket.accepted_source_digest,
        accepted_ir_digest: &ticket.accepted_ir_digest,
        accepted_artifact_digest: &ticket.accepted_artifact_digest,
        accepted_expansion_digest: &ticket.accepted_expansion_digest,
        declaration_name_high_water: ticket.declaration_name_high_water,
        candidate_input_source_digest: &ticket.candidate_input_source_digest,
    };
    serde_json::to_vec(&envelope)
        .map(|bytes| intent_content_digest(&bytes).to_string())
        .map_err(|error| {
            PreparedManagedMutationError::InvalidTicket(format!(
                "source ticket cannot be encoded canonically: {error}"
            ))
        })
}

struct ExpectedManagedSemantics {
    imports: Vec<ManagedIrImport>,
    statements: Vec<ManagedStatement>,
}

fn apply_expected_mutation(
    current: &CompiledManagedSource,
    mutation: &ManagedSketchMutation,
) -> Result<ExpectedManagedSemantics, PreparedManagedMutationError> {
    let mut imports = current.ir.imports.clone();
    let mut statements = current.ir.statements.clone();
    match mutation {
        ManagedSketchMutation::InsertDeclarations { declarations } => {
            insert_declarations(current, &mut statements, declarations)?;
            add_required_generated_unit_helpers(&mut imports, declarations)?;
        }
        ManagedSketchMutation::ReorderDeclaration {
            declaration,
            before,
        } => reorder_declaration(current, &mut statements, declaration, before.as_deref())?,
        ManagedSketchMutation::SetSuppressed { target, suppressed } => {
            refuse_helper_suppression(current, target)?;
            set_suppressed(current, &mut statements, target, *suppressed)?;
        }
        ManagedSketchMutation::SetValue {
            declaration,
            path,
            expected,
            value,
        } => set_value(&mut statements, declaration, path, expected, value)?,
        ManagedSketchMutation::SetValues { values } => {
            set_values(&mut statements, values)?;
        }
        ManagedSketchMutation::Delete { target } => match target {
            ManagedMutationTarget::Generated { .. } => {
                set_suppressed(current, &mut statements, target, true)?;
            }
            ManagedMutationTarget::Declaration { declaration } => {
                delete_declaration_closure(current, &mut statements, declaration)?;
            }
        },
    }
    if statements.len() > MUTATION_STATEMENT_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "candidate has {} statements; the limit is {MUTATION_STATEMENT_LIMIT}",
            statements.len()
        )));
    }
    Ok(ExpectedManagedSemantics {
        imports,
        statements,
    })
}

fn add_required_generated_unit_helpers(
    imports: &mut [ManagedIrImport],
    drafts: &[ManagedDeclarationDraft],
) -> Result<(), PreparedManagedMutationError> {
    let mut required = BTreeSet::new();
    for draft in drafts {
        collect_generated_unit_helpers(&draft.arguments, &mut required);
    }
    let imported = imports
        .iter()
        .flat_map(|import| import.bindings.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let missing = ["mm", "rad"]
        .into_iter()
        .filter(|helper| required.contains(helper) && !imported.contains(helper))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    let owner = imports
        .iter_mut()
        .find(|import| {
            import.module == "@geosolve/sketch-code"
                && import.bindings.iter().any(|binding| binding == "sketch")
        })
        .ok_or_else(|| {
            PreparedManagedMutationError::SemanticDelta(
                "managed insertion cannot add generated unit helpers without the owning sketch import"
                    .into(),
            )
        })?;
    owner
        .bindings
        .extend(missing.into_iter().map(str::to_owned));
    Ok(())
}

fn collect_generated_unit_helpers<'a>(value: &'a ManagedValue, required: &mut BTreeSet<&'a str>) {
    match value {
        ManagedValue::Unit(value) if matches!(value.unit.as_str(), "mm" | "rad") => {
            required.insert(value.unit.as_str());
        }
        ManagedValue::Array(values) => {
            for value in values {
                collect_generated_unit_helpers(value, required);
            }
        }
        ManagedValue::Object(fields) => {
            for value in fields.values() {
                collect_generated_unit_helpers(value, required);
            }
        }
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_)
        | ManagedValue::Reference { .. } => {}
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "this deliberately mirrors the closed TypeScript insertion transaction in one audit block"
)]
fn insert_declarations(
    current: &CompiledManagedSource,
    statements: &mut Vec<ManagedStatement>,
    drafts: &[ManagedDeclarationDraft],
) -> Result<(), PreparedManagedMutationError> {
    if drafts.is_empty() {
        return semantic_refusal("managed declaration insertion is empty");
    }
    if drafts.len() > MANAGED_MUTATION_BATCH_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "insertion has {} declarations; the limit is {MANAGED_MUTATION_BATCH_LIMIT}",
            drafts.len()
        )));
    }
    let mut occupied_variables = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for statement in statements.iter() {
        match statement {
            ManagedStatement::Binding { variable, .. } => {
                occupied_variables.insert(variable.clone());
            }
            ManagedStatement::Declaration {
                variable, symbol, ..
            } => {
                occupied_variables.insert(variable.clone());
                symbols.insert(symbol.clone());
            }
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {}
        }
    }
    let existing_group_index = statements.iter().position(|statement| {
        matches!(statement, ManagedStatement::Group { name, .. } if name == CANVAS_ADDITIONS_GROUP)
    });
    let insertion_index = existing_group_index.unwrap_or_else(|| {
        statements
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                matches!(
                    statement,
                    ManagedStatement::Binding { .. } | ManagedStatement::Declaration { .. }
                )
                .then_some(index + 1)
            })
            .next_back()
            .unwrap_or(0)
    });
    let mut available_variables = statements[..insertion_index]
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Binding { variable, .. }
            | ManagedStatement::Declaration { variable, .. } => Some(variable.clone()),
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let imported = current
        .ir
        .imports
        .iter()
        .flat_map(|import| import.bindings.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut inserted = Vec::with_capacity(drafts.len());
    let mut suppressed = Vec::new();
    for draft in drafts {
        validate_draft(
            draft,
            &occupied_variables,
            &available_variables,
            &symbols,
            &imported,
        )?;
        occupied_variables.insert(draft.variable.clone());
        available_variables.insert(draft.variable.clone());
        symbols.insert(draft.symbol.clone());
        if draft.suppressed.unwrap_or(false) {
            suppressed.push(draft.variable.clone());
        }
        inserted.push(ManagedStatement::Declaration {
            variable: draft.variable.clone(),
            symbol: draft.symbol.clone(),
            builder_path: draft.builder_path.clone(),
            patch: draft.patch.clone(),
            arguments: managed_value_expression(&draft.arguments)?,
            site: placeholder_site("declaration"),
            comments: draft.comments.clone().unwrap_or_default(),
        });
    }
    let inserted_len = inserted.len();
    statements.splice(insertion_index..insertion_index, inserted);
    let grouped = drafts
        .iter()
        .map(|draft| draft.variable.clone())
        .collect::<Vec<_>>();
    if let Some(index) = existing_group_index {
        let group_index = index + inserted_len;
        let ManagedStatement::Group { declarations, .. } = &mut statements[group_index] else {
            return semantic_refusal("Canvas additions group moved during insertion");
        };
        declarations.extend(grouped.into_iter().map(|declaration| ManagedReference {
            declaration,
            path: Vec::new(),
            site: placeholder_site("group_reference"),
        }));
    } else {
        statements.push(ManagedStatement::Group {
            name: CANVAS_ADDITIONS_GROUP.into(),
            declarations: grouped
                .into_iter()
                .map(|declaration| ManagedReference {
                    declaration,
                    path: Vec::new(),
                    site: placeholder_site("group_reference"),
                })
                .collect(),
            site: placeholder_site("group"),
            comments: Vec::new(),
        });
    }
    for declaration in suppressed {
        statements.push(ManagedStatement::Suppression {
            target: ManagedReference {
                declaration,
                path: Vec::new(),
                site: placeholder_site("suppression_reference"),
            },
            site: placeholder_site("suppression"),
            comments: Vec::new(),
        });
    }
    Ok(())
}

fn validate_draft(
    draft: &ManagedDeclarationDraft,
    occupied_variables: &BTreeSet<String>,
    available_variables: &BTreeSet<String>,
    symbols: &BTreeSet<String>,
    _imported: &BTreeSet<String>,
) -> Result<(), PreparedManagedMutationError> {
    require_text(&draft.variable, "declaration variable").map_err(semantic_error)?;
    require_text(&draft.symbol, "declaration symbol").map_err(semantic_error)?;
    if occupied_variables.contains(&draft.variable) || symbols.contains(&draft.symbol) {
        return semantic_refusal("inserted declaration repeats a variable or semantic symbol");
    }
    if draft.builder_path.is_empty() || draft.builder_path.len() > MUTATION_PATH_LIMIT {
        return semantic_refusal("inserted declaration builder path is empty or too deep");
    }
    for segment in &draft.builder_path {
        require_text(segment, "builder path segment").map_err(semantic_error)?;
    }
    if draft.patch.is_some() {
        return semantic_refusal(
            "prepared insertion cannot add a patch invocation without pinning its exact artifact in the ticket",
        );
    }
    if draft
        .group
        .as_deref()
        .is_some_and(|group| group != CANVAS_ADDITIONS_GROUP)
    {
        return semantic_refusal("insertions must belong to the Canvas additions group");
    }
    let comments = draft.comments.as_deref().unwrap_or_default();
    if comments.len() > MUTATION_COMMENT_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(
            "inserted declaration has too many comments".into(),
        ));
    }
    for comment in comments {
        require_text(comment, "comment").map_err(semantic_error)?;
        if comment.contains(['\n', '\r']) {
            return semantic_refusal("managed comments must occupy one line");
        }
    }
    let mut nodes = 0;
    validate_managed_value(&draft.arguments, available_variables, 0, &mut nodes)?;
    if !matches!(draft.arguments, ManagedValue::Object(_)) {
        return semantic_refusal("inserted declaration arguments must be an object");
    }
    Ok(())
}

fn validate_managed_value(
    value: &ManagedValue,
    variables: &BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), PreparedManagedMutationError> {
    *nodes += 1;
    if depth > MUTATION_VALUE_DEPTH_LIMIT || *nodes > MUTATION_VALUE_NODE_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(
            "managed declaration value exceeds its depth or node bound".into(),
        ));
    }
    match value {
        ManagedValue::Null | ManagedValue::Bool(_) => {}
        ManagedValue::Number(value) => {
            if !value.is_finite() {
                return semantic_refusal("managed declaration contains a non-finite number");
            }
        }
        ManagedValue::String(value) => {
            require_text(value, "managed string").map_err(semantic_error)?;
        }
        ManagedValue::Unit(value) => {
            require_text(&value.unit, "managed unit").map_err(semantic_error)?;
            if !matches!(value.unit.as_str(), "mm" | "rad") || !value.value.is_finite() {
                return semantic_refusal(
                    "managed insertion supports only finite millimetres or radians",
                );
            }
        }
        ManagedValue::Array(values) => {
            for value in values {
                validate_managed_value(value, variables, depth + 1, nodes)?;
            }
        }
        ManagedValue::Object(fields) => {
            for (name, value) in fields {
                require_text(name, "managed object field").map_err(semantic_error)?;
                validate_managed_value(value, variables, depth + 1, nodes)?;
            }
        }
        ManagedValue::Reference { declaration, path } => {
            require_text(&declaration.0, "managed reference").map_err(semantic_error)?;
            if !variables.contains(&declaration.0) {
                return semantic_refusal(
                    "managed declaration references an unknown or forward binding",
                );
            }
            validate_path(&path.0)?;
        }
    }
    Ok(())
}

fn managed_value_expression(
    value: &ManagedValue,
) -> Result<ManagedExpression, PreparedManagedMutationError> {
    let site = placeholder_site("value");
    match value {
        ManagedValue::Null => Ok(ManagedExpression::Null { site }),
        ManagedValue::Bool(value) => Ok(ManagedExpression::Boolean {
            value: *value,
            site,
        }),
        ManagedValue::Number(value) => Ok(ManagedExpression::Number {
            value: *value,
            site,
        }),
        ManagedValue::String(value) => Ok(ManagedExpression::String {
            value: value.clone(),
            site,
        }),
        ManagedValue::Unit(value) => {
            if !matches!(value.unit.as_str(), "mm" | "rad") {
                return semantic_refusal("managed insertion supports only millimetres or radians");
            }
            Ok(ManagedExpression::Call {
                callee: value.unit.clone(),
                arguments: vec![ManagedExpression::Number {
                    value: value.value,
                    site: site.clone(),
                }],
                site,
            })
        }
        ManagedValue::Array(values) => Ok(ManagedExpression::Array {
            values: values
                .iter()
                .map(managed_value_expression)
                .collect::<Result<_, _>>()?,
            site,
        }),
        ManagedValue::Object(fields) => Ok(ManagedExpression::Object {
            fields: fields
                .iter()
                .map(|(name, value)| {
                    Ok(ManagedObjectField {
                        name: name.clone(),
                        value: managed_value_expression(value)?,
                        comments: Vec::new(),
                    })
                })
                .collect::<Result<_, PreparedManagedMutationError>>()?,
            site,
        }),
        ManagedValue::Reference { declaration, path } => Ok(ManagedExpression::Reference {
            declaration: declaration.0.clone(),
            path: path.0.clone(),
            site,
        }),
    }
}

fn reorder_declaration(
    current: &CompiledManagedSource,
    statements: &mut [ManagedStatement],
    declaration: &str,
    before: Option<&str>,
) -> Result<(), PreparedManagedMutationError> {
    require_text(declaration, "declaration symbol").map_err(semantic_error)?;
    if let Some(before) = before {
        require_text(before, "destination declaration symbol").map_err(semantic_error)?;
    }
    let closures = current.projected_source_declaration_closures();
    if let Some(error) = owned_helper_mutation_error(
        &closures,
        declaration,
        ManagedSourceDeclarationHelperMutation::Move,
    ) {
        return Err(error);
    }
    if let Some(before) = before
        && let Some(error) = owned_helper_mutation_error(
            &closures,
            before,
            ManagedSourceDeclarationHelperMutation::ReorderDestination,
        )
    {
        return Err(error);
    }

    let indexes = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            ManagedStatement::Declaration { symbol, .. } => Some((index, symbol.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !indexes.iter().any(|(_, symbol)| *symbol == declaration) {
        return semantic_refusal("reorder target declaration does not exist");
    }
    if before == Some(declaration) {
        return Ok(());
    }
    let slots = indexes.iter().map(|(index, _)| *index).collect::<Vec<_>>();
    let declarations = slots
        .iter()
        .zip(indexes.iter())
        .map(|(index, (_, symbol))| ((*symbol).to_owned(), statements[*index].clone()))
        .collect::<Vec<_>>();
    let source_symbols = closure_for_root(&closures, declaration)
        .map_or_else(|| BTreeSet::from([declaration.to_owned()]), closure_symbols);
    let moved = declarations
        .iter()
        .filter(|(symbol, _)| source_symbols.contains(symbol))
        .cloned()
        .collect::<Vec<_>>();
    let mut reordered = declarations
        .into_iter()
        .filter(|(symbol, _)| !source_symbols.contains(symbol))
        .collect::<Vec<_>>();
    let destination_symbols = before
        .and_then(|symbol| closure_for_root(&closures, symbol))
        .map(closure_symbols);
    let destination = match before {
        Some(before) => reordered
            .iter()
            .position(|(symbol, _)| {
                destination_symbols
                    .as_ref()
                    .map_or(symbol == before, |closure| closure.contains(symbol))
            })
            .ok_or_else(|| {
                PreparedManagedMutationError::SemanticDelta(
                    "reorder destination declaration does not exist".into(),
                )
            })?,
        None => reordered.len(),
    };
    reordered.splice(destination..destination, moved);
    for (slot, (_, declaration)) in slots.into_iter().zip(reordered) {
        statements[slot] = declaration;
    }
    validate_lexical_order(statements)
}

fn closure_for_root<'a>(
    closures: &'a [crate::ManagedSourceDeclarationClosure],
    root: &str,
) -> Option<&'a crate::ManagedSourceDeclarationClosure> {
    closures.iter().find(|closure| closure.root.0 == root)
}

fn closure_root_for_helper<'a>(
    closures: &'a [crate::ManagedSourceDeclarationClosure],
    helper: &str,
) -> Option<&'a SemanticSymbol> {
    closures.iter().find_map(|closure| {
        closure
            .helpers
            .iter()
            .any(|candidate| candidate.0 == helper)
            .then_some(&closure.root)
    })
}

fn closure_symbols(closure: &crate::ManagedSourceDeclarationClosure) -> BTreeSet<String> {
    closure
        .helpers
        .iter()
        .chain(std::iter::once(&closure.root))
        .map(|symbol| symbol.0.clone())
        .collect()
}

fn owned_helper_mutation_error(
    closures: &[crate::ManagedSourceDeclarationClosure],
    helper: &str,
    mutation: ManagedSourceDeclarationHelperMutation,
) -> Option<PreparedManagedMutationError> {
    closure_root_for_helper(closures, helper).map(|root| {
        PreparedManagedMutationError::OwnedHelperMutation {
            helper: helper.to_owned(),
            root: root.0.clone(),
            mutation,
        }
    })
}

fn refuse_helper_suppression(
    current: &CompiledManagedSource,
    target: &ManagedMutationTarget,
) -> Result<(), PreparedManagedMutationError> {
    let ManagedMutationTarget::Declaration { declaration } = target else {
        return Ok(());
    };
    let closures = current.projected_source_declaration_closures();
    if let Some(error) = owned_helper_mutation_error(
        &closures,
        declaration,
        ManagedSourceDeclarationHelperMutation::Suppress,
    ) {
        return Err(error);
    }
    Ok(())
}

fn validate_lexical_order(
    statements: &[ManagedStatement],
) -> Result<(), PreparedManagedMutationError> {
    let mut seen = BTreeSet::new();
    for statement in statements {
        let references = match statement {
            ManagedStatement::Binding { value, .. } => expression_references(value),
            ManagedStatement::Declaration { arguments, .. } => expression_references(arguments),
            ManagedStatement::Group { declarations, .. } => declarations
                .iter()
                .map(|reference| reference.declaration.clone())
                .collect(),
            ManagedStatement::Suppression { target, .. } => {
                vec![target.declaration.clone()]
            }
        };
        if references.iter().any(|reference| !seen.contains(reference)) {
            return semantic_refusal("reorder would place a consumer before its dependency");
        }
        match statement {
            ManagedStatement::Binding { variable, .. }
            | ManagedStatement::Declaration { variable, .. } => {
                seen.insert(variable.clone());
            }
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {}
        }
    }
    Ok(())
}

fn set_suppressed(
    current: &CompiledManagedSource,
    statements: &mut Vec<ManagedStatement>,
    target: &ManagedMutationTarget,
    suppressed: bool,
) -> Result<(), PreparedManagedMutationError> {
    let reference = mutation_target_reference(current, target)?;
    let indexes = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            ManagedStatement::Suppression { target, .. }
                if target.declaration == reference.declaration && target.path == reference.path =>
            {
                Some(index)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if indexes.len() > 1 {
        return semantic_refusal("accepted source repeats an exact suppression target");
    }
    if suppressed && indexes.is_empty() {
        statements.push(ManagedStatement::Suppression {
            target: ManagedReference {
                declaration: reference.declaration,
                path: reference.path,
                site: placeholder_site("suppression_reference"),
            },
            site: placeholder_site("suppression"),
            comments: Vec::new(),
        });
    } else if !suppressed && let Some(index) = indexes.first() {
        statements.remove(*index);
    }
    Ok(())
}

fn set_value(
    statements: &mut [ManagedStatement],
    declaration: &str,
    path: &[ManagedPathSegment],
    expected: &ManagedValue,
    replacement: &ManagedValue,
) -> Result<(), PreparedManagedMutationError> {
    require_text(declaration, "value owner declaration").map_err(semantic_error)?;
    validate_path(path)?;
    let variables = statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Binding { variable, .. }
            | ManagedStatement::Declaration { variable, .. } => Some(variable.clone()),
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let mut nodes = 0;
    validate_managed_value(expected, &variables, 0, &mut nodes)?;
    let mut nodes = 0;
    validate_managed_value(replacement, &variables, 0, &mut nodes)?;
    let indexes = statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            ManagedStatement::Binding { variable, .. } if variable == declaration => Some(index),
            ManagedStatement::Declaration { symbol, .. } if symbol == declaration => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [index] = indexes.as_slice() else {
        return semantic_refusal("managed value owner is absent or ambiguous");
    };
    let statement = statements[*index].clone();
    let root = match &statement {
        ManagedStatement::Binding { value, .. } => value,
        ManagedStatement::Declaration { arguments, .. } => arguments,
        ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {
            return semantic_refusal("managed value owner disappeared");
        }
    };
    let current = expression_at_path(root, path)?;
    if expression_to_managed_value(current)? != *expected {
        return Err(PreparedManagedMutationError::StaleAuthority(
            "managed value mutation expected value changed".into(),
        ));
    }
    let replacement = managed_value_expression(replacement)?;
    let next = replace_expression_at_path(root, path, &replacement)?;
    statements[*index] = match statement {
        ManagedStatement::Binding {
            variable, comments, ..
        } => ManagedStatement::Binding {
            variable,
            value: next,
            comments,
        },
        ManagedStatement::Declaration {
            variable,
            symbol,
            builder_path,
            patch,
            site,
            comments,
            ..
        } => ManagedStatement::Declaration {
            variable,
            symbol,
            builder_path,
            patch,
            arguments: next,
            site,
            comments,
        },
        ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {
            unreachable!("value owner was checked")
        }
    };
    Ok(())
}

fn set_values(
    statements: &mut [ManagedStatement],
    values: &[ManagedValueMutation],
) -> Result<(), PreparedManagedMutationError> {
    if values.is_empty() {
        return semantic_refusal("managed value batch is empty");
    }
    if values.len() > MANAGED_MUTATION_BATCH_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(format!(
            "value batch has {} edits; the limit is {MANAGED_MUTATION_BATCH_LIMIT}",
            values.len()
        )));
    }
    let mut coordinates = BTreeSet::<(&str, &[ManagedPathSegment])>::new();
    for value in values {
        if !coordinates.insert((&value.declaration, &value.path)) {
            return semantic_refusal("managed value batch repeats an owner/path coordinate");
        }
    }
    for left in values {
        for right in values {
            if std::ptr::eq(left, right) || left.declaration != right.declaration {
                continue;
            }
            let common = left.path.len().min(right.path.len());
            if left.path[..common] == right.path[..common] {
                return semantic_refusal("managed value batch contains overlapping paths");
            }
        }
    }
    for value in values {
        set_value(
            statements,
            &value.declaration,
            &value.path,
            &value.expected,
            &value.value,
        )?;
    }
    Ok(())
}

fn expression_at_path<'a>(
    mut expression: &'a ManagedExpression,
    path: &[ManagedPathSegment],
) -> Result<&'a ManagedExpression, PreparedManagedMutationError> {
    for segment in path {
        expression = match (segment, expression) {
            (ManagedPathSegment::Field(name), ManagedExpression::Object { fields, .. }) => {
                let mut matches = fields.iter().filter(|field| field.name == *name);
                let value = matches.next().ok_or_else(|| {
                    PreparedManagedMutationError::SemanticDelta(
                        "managed value field is unavailable".into(),
                    )
                })?;
                if matches.next().is_some() {
                    return semantic_refusal("managed value object repeats a field");
                }
                &value.value
            }
            (ManagedPathSegment::Index(index), ManagedExpression::Array { values, .. }) => {
                values.get(*index).ok_or_else(|| {
                    PreparedManagedMutationError::SemanticDelta(
                        "managed value array index is unavailable".into(),
                    )
                })?
            }
            (ManagedPathSegment::Member { .. }, _) => {
                return semantic_refusal(
                    "managed source values cannot be addressed through generated member paths",
                );
            }
            _ => return semantic_refusal("managed value path has the wrong lexical shape"),
        };
    }
    Ok(expression)
}

fn replace_expression_at_path(
    expression: &ManagedExpression,
    path: &[ManagedPathSegment],
    replacement: &ManagedExpression,
) -> Result<ManagedExpression, PreparedManagedMutationError> {
    let Some((segment, rest)) = path.split_first() else {
        return Ok(replacement.clone());
    };
    match (segment, expression) {
        (ManagedPathSegment::Field(name), ManagedExpression::Object { fields, site }) => {
            let matches = fields.iter().filter(|field| field.name == *name).count();
            if matches != 1 {
                return semantic_refusal("managed value field is unavailable or ambiguous");
            }
            let fields = fields
                .iter()
                .map(|field| {
                    if field.name == *name {
                        Ok(ManagedObjectField {
                            name: field.name.clone(),
                            value: replace_expression_at_path(&field.value, rest, replacement)?,
                            comments: field.comments.clone(),
                        })
                    } else {
                        Ok(field.clone())
                    }
                })
                .collect::<Result<_, PreparedManagedMutationError>>()?;
            Ok(ManagedExpression::Object {
                fields,
                site: site.clone(),
            })
        }
        (ManagedPathSegment::Index(index), ManagedExpression::Array { values, site }) => {
            if *index >= values.len() {
                return semantic_refusal("managed value array index is unavailable");
            }
            let mut values = values.clone();
            values[*index] = replace_expression_at_path(&values[*index], rest, replacement)?;
            Ok(ManagedExpression::Array {
                values,
                site: site.clone(),
            })
        }
        (ManagedPathSegment::Member { .. }, _) => semantic_refusal(
            "managed source values cannot be addressed through generated member paths",
        ),
        _ => semantic_refusal("managed value path has the wrong lexical shape"),
    }
}

fn expression_to_managed_value(
    expression: &ManagedExpression,
) -> Result<ManagedValue, PreparedManagedMutationError> {
    match expression {
        ManagedExpression::Null { .. } => Ok(ManagedValue::Null),
        ManagedExpression::Boolean { value, .. } => Ok(ManagedValue::Bool(*value)),
        ManagedExpression::Number { value, .. } => Ok(ManagedValue::Number(*value)),
        ManagedExpression::String { value, .. } => Ok(ManagedValue::String(value.clone())),
        ManagedExpression::Array { values, .. } => values
            .iter()
            .map(expression_to_managed_value)
            .collect::<Result<Vec<_>, _>>()
            .map(ManagedValue::Array),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .map(|field| {
                Ok((
                    field.name.clone(),
                    expression_to_managed_value(&field.value)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(ManagedValue::Object),
        ManagedExpression::Reference {
            declaration, path, ..
        } => Ok(ManagedValue::Reference {
            declaration: crate::SemanticSymbol(declaration.clone()),
            path: crate::SemanticOutputPath(path.clone()),
        }),
        ManagedExpression::Call {
            callee, arguments, ..
        } => match (callee.as_str(), arguments.as_slice()) {
            (unit @ ("mm" | "rad"), [ManagedExpression::Number { value, .. }]) => {
                Ok(ManagedValue::Unit(crate::UnitLiteral {
                    unit: unit.into(),
                    value: *value,
                }))
            }
            _ => semantic_refusal("managed value mutation encountered an unsupported lexical call"),
        },
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "direct and generated target authentication stay adjacent to make exact wire routing auditable"
)]
fn mutation_target_reference(
    current: &CompiledManagedSource,
    target: &ManagedMutationTarget,
) -> Result<ManagedReference, PreparedManagedMutationError> {
    match target {
        ManagedMutationTarget::Declaration { declaration } => {
            require_text(declaration, "declaration target").map_err(semantic_error)?;
            let variable = current
                .ir
                .statements
                .iter()
                .find_map(|statement| match statement {
                    ManagedStatement::Declaration {
                        variable, symbol, ..
                    } if symbol == declaration => Some(variable.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    PreparedManagedMutationError::SemanticDelta(
                        "declaration mutation target does not exist".into(),
                    )
                })?;
            Ok(ManagedReference {
                declaration: variable,
                path: Vec::new(),
                site: String::new(),
            })
        }
        ManagedMutationTarget::Generated { address } => {
            validate_generated_address(address)?;
            let matches = current
                .artifact
                .generated_members
                .iter()
                .filter(|member| member.address == *address)
                .count();
            if matches != 1 {
                return semantic_refusal(
                    "generated-member target is absent or ambiguous in accepted authority",
                );
            }
            let owner = current
                .ir
                .statements
                .iter()
                .find_map(|statement| match statement {
                    ManagedStatement::Declaration {
                        variable,
                        symbol,
                        patch: Some(_),
                        ..
                    } if symbol == &address.invocation => Some(variable.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    PreparedManagedMutationError::SemanticDelta(
                        "generated member has no lexical patch owner".into(),
                    )
                })?;
            let result_paths = current
                .artifact
                .declarations
                .iter()
                .find(|declaration| declaration.declaration == address.invocation)
                .map(|declaration| {
                    declaration
                        .result
                        .iter()
                        .map(|result| result.path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut candidates = Vec::new();
            for root in &result_paths {
                let mut path = root.clone();
                if !address.member_key.is_empty() {
                    path.extend(address.member_key.iter().map(|member| {
                        ManagedPathSegment::Member {
                            member: member.clone(),
                        }
                    }));
                }
                candidates.push(path);
            }
            if address.member_key.is_empty() {
                candidates.push(
                    address
                        .template
                        .iter()
                        .cloned()
                        .map(ManagedPathSegment::Field)
                        .collect(),
                );
            }
            let exact = candidates.iter().find(|path| {
                current.artifact.suppressions.iter().any(|suppression| {
                    suppression.target.declaration == owner && suppression.target.path == **path
                })
            });
            let path = if let Some(path) = exact {
                (*path).clone()
            } else if result_paths.len() == 1 {
                let mut path = result_paths[0].clone();
                if !address.member_key.is_empty() {
                    path.extend(address.member_key.iter().map(|member| {
                        ManagedPathSegment::Member {
                            member: member.clone(),
                        }
                    }));
                }
                path
            } else if let Some(path) = result_paths.iter().find(|path| {
                path.iter()
                    .filter_map(|segment| match segment {
                        ManagedPathSegment::Field(field) => Some(field.as_str()),
                        ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. } => None,
                    })
                    .eq(address.template.iter().map(String::as_str))
            }) {
                path.clone()
            } else {
                return semantic_refusal(
                    "generated member cannot be projected to one lexical result path",
                );
            };
            Ok(ManagedReference {
                declaration: owner,
                path,
                site: String::new(),
            })
        }
    }
}

fn validate_generated_address(
    address: &ExecutedGeneratedMemberAddress,
) -> Result<(), PreparedManagedMutationError> {
    require_text(&address.invocation, "generated invocation").map_err(semantic_error)?;
    for (label, path) in [
        ("generated template", &address.template),
        ("generated member key", &address.member_key),
        ("generated output", &address.output),
    ] {
        if path.len() > MUTATION_PATH_LIMIT {
            return Err(PreparedManagedMutationError::ResourceLimit(format!(
                "{label} path is too deep"
            )));
        }
        for value in path {
            require_text(value, label).map_err(semantic_error)?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "dependency closure and upstream binding pruning mirror one indivisible TypeScript operation"
)]
fn delete_declaration_closure(
    current: &CompiledManagedSource,
    statements: &mut Vec<ManagedStatement>,
    declaration: &str,
) -> Result<(), PreparedManagedMutationError> {
    let closures = current.projected_source_declaration_closures();
    if let Some(error) = owned_helper_mutation_error(
        &closures,
        declaration,
        ManagedSourceDeclarationHelperMutation::Delete,
    ) {
        return Err(error);
    }
    let owner = statements.iter().find_map(|statement| match statement {
        ManagedStatement::Declaration {
            variable, symbol, ..
        } if symbol == declaration => Some(variable.clone()),
        _ => None,
    });
    let Some(owner) = owner else {
        return semantic_refusal("delete target declaration does not exist");
    };
    let mut removed = BTreeSet::from([owner]);
    if let Some(closure) = closure_for_root(&closures, declaration) {
        let variables_by_symbol = statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Declaration {
                    variable, symbol, ..
                } => Some((symbol.as_str(), variable.clone())),
                ManagedStatement::Binding { .. }
                | ManagedStatement::Group { .. }
                | ManagedStatement::Suppression { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        for helper in &closure.helpers {
            let variable = variables_by_symbol.get(helper.0.as_str()).ok_or_else(|| {
                PreparedManagedMutationError::SemanticDelta(
                    "authenticated Profile Offset closure lost a helper declaration".into(),
                )
            })?;
            removed.insert(variable.clone());
        }
    }
    loop {
        let mut changed = false;
        for statement in statements.iter() {
            let (variable, expression) = match statement {
                ManagedStatement::Binding {
                    variable, value, ..
                } => (variable, value),
                ManagedStatement::Declaration {
                    variable,
                    arguments,
                    ..
                } => (variable, arguments),
                ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {
                    continue;
                }
            };
            if !removed.contains(variable)
                && expression_references(expression)
                    .iter()
                    .any(|reference| removed.contains(reference))
            {
                removed.insert(variable.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let bindings = statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => Some((variable.clone(), value.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let by_variable = statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Binding {
                variable, value, ..
            }
            | ManagedStatement::Declaration {
                variable,
                arguments: value,
                ..
            } => Some((variable.clone(), value.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut prunable_bindings = BTreeSet::new();
    let mut pending = removed
        .iter()
        .filter_map(|variable| by_variable.get(variable))
        .flat_map(expression_references)
        .collect::<Vec<_>>();
    while let Some(variable) = pending.pop() {
        if !prunable_bindings.insert(variable.clone()) {
            continue;
        }
        if let Some(value) = bindings.get(&variable) {
            pending.extend(expression_references(value));
        }
    }
    let mut retained = Vec::with_capacity(statements.len());
    for statement in statements.drain(..) {
        match statement {
            ManagedStatement::Binding { ref variable, .. }
            | ManagedStatement::Declaration { ref variable, .. }
                if removed.contains(variable) => {}
            ManagedStatement::Suppression { ref target, .. }
                if removed.contains(&target.declaration) => {}
            ManagedStatement::Group {
                name,
                declarations,
                site,
                comments,
            } => {
                let declarations = declarations
                    .into_iter()
                    .filter(|reference| !removed.contains(&reference.declaration))
                    .collect::<Vec<_>>();
                if !declarations.is_empty() {
                    retained.push(ManagedStatement::Group {
                        name,
                        declarations,
                        site,
                        comments,
                    });
                }
            }
            statement => retained.push(statement),
        }
    }
    *statements = retained;
    prune_unused_bindings(statements, &prunable_bindings);
    Ok(())
}

fn prune_unused_bindings(statements: &mut Vec<ManagedStatement>, candidates: &BTreeSet<String>) {
    loop {
        let used = statements
            .iter()
            .flat_map(|statement| match statement {
                ManagedStatement::Binding { value, .. } => expression_references(value),
                ManagedStatement::Declaration { arguments, .. } => expression_references(arguments),
                ManagedStatement::Group { declarations, .. } => declarations
                    .iter()
                    .map(|reference| reference.declaration.clone())
                    .collect(),
                ManagedStatement::Suppression { target, .. } => {
                    vec![target.declaration.clone()]
                }
            })
            .collect::<BTreeSet<_>>();
        let removable = statements.iter().position(|statement| {
            matches!(statement, ManagedStatement::Binding { variable, .. }
                if candidates.contains(variable) && !used.contains(variable))
        });
        if let Some(index) = removable {
            statements.remove(index);
        } else {
            break;
        }
    }
}

fn expression_references(expression: &ManagedExpression) -> Vec<String> {
    match expression {
        ManagedExpression::Reference { declaration, .. } => {
            vec![declaration.clone()]
        }
        ManagedExpression::Array { values, .. } => {
            values.iter().flat_map(expression_references).collect()
        }
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .flat_map(|field| expression_references(&field.value))
            .collect(),
        ManagedExpression::Call { arguments, .. } => {
            arguments.iter().flat_map(expression_references).collect()
        }
        ManagedExpression::Null { .. }
        | ManagedExpression::Boolean { .. }
        | ManagedExpression::Number { .. }
        | ManagedExpression::String { .. } => Vec::new(),
    }
}

#[derive(Serialize)]
struct SemanticIrEnvelope<'a> {
    imports: &'a [crate::ManagedIrImport],
    statements: &'a [ManagedStatement],
}

fn semantic_ir_digest(
    imports: &[ManagedIrImport],
    statements: &[ManagedStatement],
) -> Result<String, PreparedManagedMutationError> {
    let erased = statements
        .iter()
        .cloned()
        .map(erase_statement_sites)
        .collect::<Vec<_>>();
    let envelope = SemanticIrEnvelope {
        imports,
        statements: &erased,
    };
    serde_json::to_vec(&envelope)
        .map(|bytes| intent_content_digest(&bytes).to_string())
        .map_err(|error| {
            PreparedManagedMutationError::InvalidTicket(format!(
                "managed semantic fingerprint cannot be encoded: {error}"
            ))
        })
}

fn same_semantic_statements(
    expected: &[ManagedStatement],
    candidate: &[ManagedStatement],
) -> Result<bool, PreparedManagedMutationError> {
    let expected = expected
        .iter()
        .cloned()
        .map(erase_statement_sites)
        .collect::<Vec<_>>();
    let candidate = candidate
        .iter()
        .cloned()
        .map(erase_statement_sites)
        .collect::<Vec<_>>();
    let expected = serde_json::to_vec(&expected)
        .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
    let candidate = serde_json::to_vec(&candidate)
        .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
    Ok(expected == candidate)
}

fn erase_statement_sites(mut statement: ManagedStatement) -> ManagedStatement {
    match &mut statement {
        ManagedStatement::Binding { value, .. } => erase_expression_sites(value),
        ManagedStatement::Declaration {
            arguments, site, ..
        } => {
            site.clear();
            erase_expression_sites(arguments);
        }
        ManagedStatement::Group {
            declarations, site, ..
        } => {
            site.clear();
            for reference in declarations {
                reference.site.clear();
            }
        }
        ManagedStatement::Suppression { target, site, .. } => {
            site.clear();
            target.site.clear();
        }
    }
    statement
}

fn erase_expression_sites(expression: &mut ManagedExpression) {
    match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Reference { site, .. } => site.clear(),
        ManagedExpression::Array { values, site } => {
            site.clear();
            for value in values {
                erase_expression_sites(value);
            }
        }
        ManagedExpression::Object { fields, site } => {
            site.clear();
            for field in fields {
                erase_expression_sites(&mut field.value);
            }
        }
        ManagedExpression::Call {
            arguments, site, ..
        } => {
            site.clear();
            for argument in arguments {
                erase_expression_sites(argument);
            }
        }
    }
}

fn same_compiled_authority(
    current: &CompiledManagedSource,
    candidate: &CompiledManagedSource,
) -> bool {
    current.normalized_source == candidate.normalized_source
        && current.ir == candidate.ir
        && current.artifact == candidate.artifact
        && current.canonical_ir_json == candidate.canonical_ir_json
        && current.canonical_artifact_json == candidate.canonical_artifact_json
}

fn validate_artifact_delta(
    current: &CompiledManagedSource,
    candidate: &CompiledManagedSource,
    mutation: &ManagedSketchMutation,
) -> Result<(), PreparedManagedMutationError> {
    let base_symbols = declaration_symbols(&current.ir.statements);
    let candidate_symbols = declaration_symbols(&candidate.ir.statements);
    let inserted_symbols = candidate_symbols
        .difference(&base_symbols)
        .cloned()
        .collect::<BTreeSet<_>>();
    let retained_symbols = base_symbols
        .intersection(&candidate_symbols)
        .cloned()
        .collect::<BTreeSet<_>>();

    let base_declarations = current
        .artifact
        .declarations
        .iter()
        .map(|declaration| (declaration.declaration.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let candidate_declarations = candidate
        .artifact
        .declarations
        .iter()
        .map(|declaration| (declaration.declaration.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    for symbol in &retained_symbols {
        let Some(base) = base_declarations.get(symbol.as_str()) else {
            return semantic_refusal("accepted artifact lost a retained declaration owner");
        };
        let Some(next) = candidate_declarations.get(symbol.as_str()) else {
            return semantic_refusal("candidate artifact lost a retained declaration owner");
        };
        if base.family != next.family || base.patch != next.patch || base.result != next.result {
            return semantic_refusal("candidate changed an existing declaration's executed result");
        }
    }
    validate_inserted_results(candidate, mutation, &inserted_symbols)?;

    let base_generated = current
        .artifact
        .generated_members
        .iter()
        .map(|member| (&member.address, (&member.family, member.kind)))
        .collect::<BTreeMap<_, _>>();
    let candidate_generated = candidate
        .artifact
        .generated_members
        .iter()
        .map(|member| (&member.address, (&member.family, member.kind)))
        .collect::<BTreeMap<_, _>>();
    for (address, value) in &base_generated {
        if retained_symbols.contains(&address.invocation)
            && candidate_generated.get(address) != Some(value)
        {
            return semantic_refusal(
                "candidate changed a generated member owned by a retained declaration",
            );
        }
    }
    for address in candidate_generated.keys() {
        if !base_generated.contains_key(address) {
            return semantic_refusal(
                "candidate added a generated member without pinned patch-insertion authority",
            );
        }
    }

    let mutable_consumer_owners = directly_mutated_declaration_owners(candidate, mutation);
    let stable_consumer_owners = retained_symbols
        .difference(&mutable_consumer_owners)
        .cloned()
        .collect::<BTreeSet<_>>();
    let base_consumers = semantic_consumers(current, &stable_consumer_owners)?;
    let candidate_consumers = semantic_consumers(candidate, &stable_consumer_owners)?;
    if base_consumers != candidate_consumers {
        return semantic_refusal(
            "candidate changed value-consumer provenance for a retained declaration",
        );
    }
    validate_direct_consumers(
        candidate,
        &mutable_consumer_owners,
        "mutated declaration value-consumer provenance differs from its exact IR",
    )?;
    validate_inserted_consumers(candidate, &inserted_symbols)?;
    for consumer in &candidate.artifact.value_consumers {
        let owner = consumer_target_owner(&consumer.target);
        if !retained_symbols.contains(owner) && !inserted_symbols.contains(owner) {
            return semantic_refusal("candidate consumer has no authorized declaration owner");
        }
    }

    if !matches!(mutation, ManagedSketchMutation::InsertDeclarations { .. })
        && !inserted_symbols.is_empty()
    {
        return semantic_refusal("non-insertion mutation added executed declarations");
    }
    Ok(())
}

fn directly_mutated_declaration_owners(
    candidate: &CompiledManagedSource,
    mutation: &ManagedSketchMutation,
) -> BTreeSet<String> {
    let requested = match mutation {
        ManagedSketchMutation::SetValue { declaration, .. } => {
            BTreeSet::from([declaration.clone()])
        }
        ManagedSketchMutation::SetValues { values } => values
            .iter()
            .map(|value| value.declaration.clone())
            .collect(),
        ManagedSketchMutation::InsertDeclarations { .. }
        | ManagedSketchMutation::ReorderDeclaration { .. }
        | ManagedSketchMutation::SetSuppressed { .. }
        | ManagedSketchMutation::Delete { .. } => BTreeSet::new(),
    };
    candidate
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                symbol,
                patch: None,
                ..
            } if requested.contains(symbol) => Some(symbol.clone()),
            _ => None,
        })
        .collect()
}

fn validate_inserted_results(
    candidate: &CompiledManagedSource,
    mutation: &ManagedSketchMutation,
    inserted: &BTreeSet<String>,
) -> Result<(), PreparedManagedMutationError> {
    if inserted.is_empty() {
        return Ok(());
    }
    let ManagedSketchMutation::InsertDeclarations { declarations } = mutation else {
        return semantic_refusal("non-insertion mutation produced inserted declarations");
    };
    let drafts = declarations
        .iter()
        .map(|draft| (draft.symbol.as_str(), draft))
        .collect::<BTreeMap<_, _>>();
    let statements = candidate
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration {
                symbol,
                builder_path,
                arguments,
                patch,
                ..
            } if inserted.contains(symbol) => Some((
                symbol.as_str(),
                (builder_path.as_slice(), arguments, patch.as_ref()),
            )),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let executed = candidate
        .artifact
        .declarations
        .iter()
        .filter(|declaration| inserted.contains(&declaration.declaration))
        .map(|declaration| (declaration.declaration.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    if drafts.len() != inserted.len()
        || statements.len() != inserted.len()
        || executed.len() != inserted.len()
    {
        return semantic_refusal(
            "inserted declaration ownership is incomplete across ticket, IR, and artifact",
        );
    }
    let catalog = declaration_result_catalog();
    for symbol in inserted {
        let draft = drafts.get(symbol.as_str()).ok_or_else(|| {
            PreparedManagedMutationError::SemanticDelta(
                "inserted artifact declaration has no ticket draft".into(),
            )
        })?;
        let (builder_path, arguments, patch) =
            statements.get(symbol.as_str()).ok_or_else(|| {
                PreparedManagedMutationError::SemanticDelta(
                    "inserted declaration has no candidate IR statement".into(),
                )
            })?;
        if draft.patch.is_some() || patch.is_some() {
            return semantic_refusal("inserted patch declaration has no exact result authority");
        }
        let family = builder_path.join(".");
        let descriptor = catalog.get(&family).ok_or_else(|| {
            PreparedManagedMutationError::SemanticDelta(format!(
                "inserted declaration family `{family}` has no Rust result descriptor"
            ))
        })?;
        let expected = direct_declaration_result_leaves(&family, arguments, &descriptor.outputs);
        let actual = &executed
            .get(symbol.as_str())
            .ok_or_else(|| {
                PreparedManagedMutationError::SemanticDelta(
                    "inserted declaration has no executed artifact result".into(),
                )
            })?
            .result;
        if *actual != expected {
            return semantic_refusal(
                "inserted declaration result differs from the Rust-owned result descriptor",
            );
        }
    }
    Ok(())
}

fn validate_inserted_consumers(
    candidate: &CompiledManagedSource,
    inserted: &BTreeSet<String>,
) -> Result<(), PreparedManagedMutationError> {
    if inserted.is_empty() {
        return Ok(());
    }
    validate_direct_consumers(
        candidate,
        inserted,
        "inserted declaration value-consumer provenance differs from its exact IR",
    )
}

fn validate_direct_consumers(
    candidate: &CompiledManagedSource,
    declarations: &BTreeSet<String>,
    mismatch: &str,
) -> Result<(), PreparedManagedMutationError> {
    if declarations.is_empty() {
        return Ok(());
    }
    let mut binding_origins = BTreeMap::<String, Vec<String>>::new();
    let mut expected = BTreeMap::<String, usize>::new();
    for statement in &candidate.ir.statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => {
                binding_origins.insert(
                    variable.clone(),
                    expression_origins(value, &binding_origins),
                );
            }
            ManagedStatement::Declaration {
                symbol,
                builder_path,
                arguments,
                patch,
                ..
            } if declarations.contains(symbol) => {
                if patch.is_some() {
                    return semantic_refusal("inserted patch consumer provenance is unauthorized");
                }
                collect_expected_consumers(
                    arguments,
                    &ExecutedConsumerTarget::Declaration {
                        declaration: symbol.clone(),
                        family: builder_path.join("."),
                    },
                    &mut Vec::new(),
                    &binding_origins,
                    &mut expected,
                )?;
            }
            ManagedStatement::Declaration { .. }
            | ManagedStatement::Group { .. }
            | ManagedStatement::Suppression { .. } => {}
        }
    }
    let mut actual = BTreeMap::<String, usize>::new();
    for consumer in &candidate.artifact.value_consumers {
        if declarations.contains(consumer_target_owner(&consumer.target)) {
            let key = serde_json::to_string(&(
                &consumer.value_site,
                &consumer.target,
                &consumer.property,
            ))
            .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
            *actual.entry(key).or_insert(0) += 1;
        }
    }
    if actual != expected {
        return semantic_refusal(mismatch);
    }
    Ok(())
}

fn expression_origins(
    expression: &ManagedExpression,
    bindings: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    match expression {
        ManagedExpression::Null { site }
        | ManagedExpression::Boolean { site, .. }
        | ManagedExpression::Number { site, .. }
        | ManagedExpression::String { site, .. }
        | ManagedExpression::Call { site, .. } => vec![site.clone()],
        ManagedExpression::Reference {
            declaration, site, ..
        } => bindings
            .get(declaration)
            .cloned()
            .unwrap_or_else(|| vec![site.clone()]),
        ManagedExpression::Array { values, .. } => values
            .iter()
            .flat_map(|value| expression_origins(value, bindings))
            .collect(),
        ManagedExpression::Object { fields, .. } => fields
            .iter()
            .flat_map(|field| expression_origins(&field.value, bindings))
            .collect(),
    }
}

fn collect_expected_consumers(
    expression: &ManagedExpression,
    target: &ExecutedConsumerTarget,
    property: &mut Vec<ManagedPathSegment>,
    bindings: &BTreeMap<String, Vec<String>>,
    consumers: &mut BTreeMap<String, usize>,
) -> Result<(), PreparedManagedMutationError> {
    let sites = match expression {
        ManagedExpression::Reference {
            declaration, site, ..
        } => bindings
            .get(declaration)
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
                collect_expected_consumers(value, target, property, bindings, consumers)?;
                property.pop();
            }
            return Ok(());
        }
        ManagedExpression::Object { fields, .. } => {
            for field in fields {
                property.push(ManagedPathSegment::Field(field.name.clone()));
                collect_expected_consumers(&field.value, target, property, bindings, consumers)?;
                property.pop();
            }
            return Ok(());
        }
    };
    for site in sites {
        let key = serde_json::to_string(&(&site, target, property.as_slice()))
            .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
        *consumers.entry(key).or_insert(0) += 1;
    }
    Ok(())
}

fn declaration_symbols(statements: &[ManagedStatement]) -> BTreeSet<String> {
    statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration { symbol, .. } => Some(symbol.clone()),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "owner", content = "name", rename_all = "snake_case")]
enum ValueOwner {
    Binding(String),
    Declaration(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "step", content = "value", rename_all = "snake_case")]
enum ValueStep {
    Array(usize),
    Object { index: usize, name: String },
    Call(usize),
}

#[derive(Serialize)]
struct ValueCoordinate<'a> {
    owner: &'a ValueOwner,
    path: &'a [ValueStep],
}

fn semantic_consumers(
    compiled: &CompiledManagedSource,
    owners: &BTreeSet<String>,
) -> Result<BTreeMap<String, usize>, PreparedManagedMutationError> {
    let sites = value_site_coordinates(&compiled.ir.statements)?;
    let mut consumers = BTreeMap::new();
    for consumer in &compiled.artifact.value_consumers {
        if !owners.contains(consumer_target_owner(&consumer.target)) {
            continue;
        }
        let coordinate = sites.get(&consumer.value_site).ok_or_else(|| {
            PreparedManagedMutationError::SemanticDelta(
                "value consumer has no stable lexical coordinate".into(),
            )
        })?;
        let key = serde_json::to_string(&(coordinate, &consumer.target, &consumer.property))
            .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
        *consumers.entry(key).or_insert(0) += 1;
    }
    Ok(consumers)
}

fn consumer_target_owner(target: &ExecutedConsumerTarget) -> &str {
    match target {
        ExecutedConsumerTarget::Declaration { declaration, .. } => declaration,
        ExecutedConsumerTarget::Generated { address, .. } => &address.invocation,
    }
}

fn value_site_coordinates(
    statements: &[ManagedStatement],
) -> Result<BTreeMap<String, String>, PreparedManagedMutationError> {
    let mut sites = BTreeMap::new();
    for statement in statements {
        match statement {
            ManagedStatement::Binding {
                variable, value, ..
            } => collect_value_coordinates(
                value,
                &ValueOwner::Binding(variable.clone()),
                &mut Vec::new(),
                &mut sites,
            )?,
            ManagedStatement::Declaration {
                symbol, arguments, ..
            } => collect_value_coordinates(
                arguments,
                &ValueOwner::Declaration(symbol.clone()),
                &mut Vec::new(),
                &mut sites,
            )?,
            ManagedStatement::Group { .. } | ManagedStatement::Suppression { .. } => {}
        }
    }
    Ok(sites)
}

fn collect_value_coordinates(
    expression: &ManagedExpression,
    owner: &ValueOwner,
    path: &mut Vec<ValueStep>,
    sites: &mut BTreeMap<String, String>,
) -> Result<(), PreparedManagedMutationError> {
    let site = expression_site(expression);
    let coordinate = serde_json::to_string(&ValueCoordinate { owner, path })
        .map_err(|error| PreparedManagedMutationError::SemanticDelta(error.to_string()))?;
    if sites.insert(site.to_owned(), coordinate).is_some() {
        return semantic_refusal("one value source-site is reused by multiple expressions");
    }
    match expression {
        ManagedExpression::Array { values, .. } => {
            for (index, value) in values.iter().enumerate() {
                path.push(ValueStep::Array(index));
                collect_value_coordinates(value, owner, path, sites)?;
                path.pop();
            }
        }
        ManagedExpression::Object { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                path.push(ValueStep::Object {
                    index,
                    name: field.name.clone(),
                });
                collect_value_coordinates(&field.value, owner, path, sites)?;
                path.pop();
            }
        }
        ManagedExpression::Call { arguments, .. } => {
            for (index, argument) in arguments.iter().enumerate() {
                path.push(ValueStep::Call(index));
                collect_value_coordinates(argument, owner, path, sites)?;
                path.pop();
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

fn expression_site(expression: &ManagedExpression) -> &str {
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

fn validate_path(path: &[ManagedPathSegment]) -> Result<(), PreparedManagedMutationError> {
    if path.len() > MUTATION_PATH_LIMIT {
        return Err(PreparedManagedMutationError::ResourceLimit(
            "managed semantic path is too deep".into(),
        ));
    }
    for segment in path {
        match segment {
            ManagedPathSegment::Field(value) | ManagedPathSegment::Member { member: value } => {
                require_text(value, "managed path segment").map_err(semantic_error)?;
            }
            ManagedPathSegment::Index(_) => {}
        }
    }
    Ok(())
}

fn placeholder_site(kind: &str) -> String {
    format!("mutation:{kind}")
}

struct BoundedJsonWriter {
    written: usize,
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::FileTooLarge, "JSON size overflow"))?;
        if next > PREPARED_MANAGED_MUTATION_WIRE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "prepared managed JSON exceeds its wire bound",
            ));
        }
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_wire_size<T: Serialize>(
    value: &T,
    label: &str,
) -> Result<(), PreparedManagedMutationError> {
    let mut writer = BoundedJsonWriter { written: 0 };
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        PreparedManagedMutationError::ResourceLimit(format!(
            "{label} is not bounded canonical JSON: {error}"
        ))
    })
}

fn require_text(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MUTATION_STRING_LIMIT || value.contains('\0') {
        Err(format!("{label} is empty, oversized, or contains NUL"))
    } else {
        Ok(())
    }
}

fn require_digest(value: &str, label: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!(
            "{label} digest is not canonical lower-case SHA-256"
        ))
    }
}

fn invalid_authority(message: String) -> PreparedManagedMutationError {
    PreparedManagedMutationError::InvalidAuthority(message)
}

fn invalid_ticket(message: String) -> PreparedManagedMutationError {
    PreparedManagedMutationError::InvalidTicket(message)
}

fn invalid_receipt(message: String) -> PreparedManagedMutationError {
    PreparedManagedMutationError::InvalidReceipt(message)
}

fn semantic_error(message: String) -> PreparedManagedMutationError {
    PreparedManagedMutationError::SemanticDelta(message)
}

fn semantic_refusal<T>(message: impl Into<String>) -> Result<T, PreparedManagedMutationError> {
    Err(PreparedManagedMutationError::SemanticDelta(message.into()))
}

impl From<ManagedValidationError> for PreparedManagedMutationError {
    fn from(error: ManagedValidationError) -> Self {
        Self::InvalidReceipt(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecutedResultLeaf, FeatureKind};

    #[derive(Serialize)]
    struct IrDigestEnvelope<'a> {
        format: &'a str,
        imports: &'a [crate::ManagedIrImport],
        statements: &'a [ManagedStatement],
        output: &'a ManagedExpression,
        source_sites: &'a [crate::ManagedSourceSite],
        source_digest: &'a str,
    }

    #[derive(Serialize)]
    struct ArtifactDigestEnvelope<'a> {
        format: &'a str,
        source_digest: &'a str,
        ir_digest: &'a str,
        declarations: &'a [crate::ExecutedDeclarationResult],
        generated_members: &'a [crate::ExecutedGeneratedMember],
        groups: &'a [crate::ExecutedGroup],
        suppressions: &'a [crate::ExecutedSuppression],
        value_consumers: &'a [crate::ExecutedValueConsumer],
        output: &'a ManagedValue,
    }

    fn compiled_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
        )))
        .expect("managed interop fixture")
    }

    fn empty_compiled_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-clean-empty.json"
        )))
        .expect("managed empty fixture")
    }

    fn empty_circle_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json"
        )))
        .expect("managed empty-circle fixture")
    }

    fn named_polyline_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-polyline.json"
        )))
        .expect("managed named Polyline fixture")
    }

    fn named_constraint_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-curve-tangency.json"
        )))
        .expect("managed named constraint fixture")
    }

    fn profile_offset_fixture(name: &str) -> CompiledManagedSource {
        let json = match name {
            "base" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
            )),
            "reordered" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-reordered.json"
            )),
            "deleted" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-deleted.json"
            )),
            _ => panic!("unknown Profile Offset fixture"),
        };
        CompiledManagedSource::from_json(json).expect("Profile Offset TypeScript fixture")
    }

    fn closure_field(name: &str, value: ManagedExpression) -> ManagedObjectField {
        ManagedObjectField {
            name: name.into(),
            value,
            comments: Vec::new(),
        }
    }

    fn closure_object(fields: impl IntoIterator<Item = ManagedObjectField>) -> ManagedExpression {
        ManagedExpression::Object {
            fields: fields.into_iter().collect(),
            site: String::new(),
        }
    }

    fn closure_declaration(
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

    fn independent_declaration(variable: &str, symbol: &str) -> ManagedStatement {
        ManagedStatement::Declaration {
            variable: variable.into(),
            symbol: symbol.into(),
            builder_path: vec!["geometry".into(), "centerRadiusCircle".into()],
            patch: None,
            arguments: closure_object([]),
            site: String::new(),
            comments: Vec::new(),
        }
    }

    fn profile_offset_compiled_for_mutation() -> CompiledManagedSource {
        let mut compiled = compiled_fixture();
        let helper = closure_declaration(
            "helper_variable",
            "offsetChain11",
            ["aggregate", "openChain"],
            closure_object([]),
        );
        let root = closure_declaration(
            "root_variable",
            "profileOffset12",
            ["operation", "profileOffset"],
            closure_object([closure_field(
                "sources",
                ManagedExpression::Array {
                    values: vec![ManagedExpression::Reference {
                        declaration: "helper_variable".into(),
                        path: vec![ManagedPathSegment::Field("chain".into())],
                        site: String::new(),
                    }],
                    site: String::new(),
                },
            )]),
        );
        compiled.ir.statements = vec![
            independent_declaration("base_variable", "base"),
            helper,
            root,
            independent_declaration("marker_variable", "marker"),
            ManagedStatement::Group {
                name: "Canvas additions".into(),
                declarations: [
                    "base_variable",
                    "helper_variable",
                    "root_variable",
                    "marker_variable",
                ]
                .into_iter()
                .map(|declaration| ManagedReference {
                    declaration: declaration.into(),
                    path: Vec::new(),
                    site: String::new(),
                })
                .collect(),
                site: String::new(),
                comments: Vec::new(),
            },
        ];
        compiled
    }

    fn ordered_declaration_symbols(statements: &[ManagedStatement]) -> Vec<&str> {
        statements
            .iter()
            .filter_map(|statement| match statement {
                ManagedStatement::Declaration { symbol, .. } => Some(symbol.as_str()),
                ManagedStatement::Binding { .. }
                | ManagedStatement::Group { .. }
                | ManagedStatement::Suppression { .. } => None,
            })
            .collect()
    }

    fn detached_reference_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-reference-detached.json"
        )))
        .expect("managed detached-reference fixture")
    }

    fn authority(compiled: &CompiledManagedSource) -> ManagedMutationAuthority {
        ManagedMutationAuthority::new(
            ProjectKey("prepared-test".into()),
            CodeSessionIdentity {
                session: 7,
                revision: 11,
                digest: "1".repeat(64),
            },
            "2".repeat(64),
            41,
            compiled,
        )
        .expect("accepted authority")
    }

    fn canonical_host_compiled(mut compiled: CompiledManagedSource) -> CompiledManagedSource {
        compiled.input_source_digest = compiled.ir.source_digest.clone();
        compiled
    }

    fn remove_radius_suppression(mut compiled: CompiledManagedSource) -> CompiledManagedSource {
        compiled
            .ir
            .statements
            .retain(|statement| !matches!(statement, ManagedStatement::Suppression { .. }));
        compiled.artifact.suppressions.clear();
        compiled.normalized_source.push(' ');
        refresh_compiled_digests(&mut compiled);
        compiled
    }

    fn replace_shared_radius(
        mut compiled: CompiledManagedSource,
        value: f64,
    ) -> CompiledManagedSource {
        let binding = compiled
            .ir
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                ManagedStatement::Binding {
                    variable, value, ..
                } if variable == "sharedRadius" => Some(value),
                _ => None,
            })
            .expect("shared radius binding");
        let ManagedExpression::Call { arguments, .. } = binding else {
            panic!("shared radius must remain a unit call")
        };
        let [ManagedExpression::Number { value: radius, .. }] = arguments.as_mut_slice() else {
            panic!("shared radius unit call must contain one number")
        };
        *radius = value;
        compiled.normalized_source.push(' ');
        refresh_compiled_digests(&mut compiled);
        compiled
    }

    fn refresh_compiled_digests(compiled: &mut CompiledManagedSource) {
        let source = intent_content_digest(compiled.normalized_source.as_bytes()).to_string();
        compiled.input_source_digest.clone_from(&source);
        compiled.ir.source_digest.clone_from(&source);
        for site in &mut compiled.ir.source_sites {
            site.source_digest.clone_from(&source);
        }
        let ir = IrDigestEnvelope {
            format: &compiled.ir.format,
            imports: &compiled.ir.imports,
            statements: &compiled.ir.statements,
            output: &compiled.ir.output,
            source_sites: &compiled.ir.source_sites,
            source_digest: &compiled.ir.source_digest,
        };
        compiled.ir.ir_digest =
            intent_content_digest(&serde_json::to_vec(&ir).expect("IR digest envelope"))
                .to_string();
        compiled.canonical_ir_json = serde_json::to_string(&compiled.ir).expect("canonical IR");
        compiled.artifact.source_digest.clone_from(&source);
        compiled
            .artifact
            .ir_digest
            .clone_from(&compiled.ir.ir_digest);
        let artifact = ArtifactDigestEnvelope {
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
            &serde_json::to_vec(&artifact).expect("artifact digest envelope"),
        )
        .to_string();
        compiled.canonical_artifact_json =
            serde_json::to_string(&compiled.artifact).expect("canonical artifact");
    }

    fn compiled_with_keyed_values() -> CompiledManagedSource {
        let mut compiled = compiled_fixture();
        let (family, arguments) = compiled
            .ir
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    builder_path,
                    arguments,
                    ..
                } if symbol == "hole" => Some((builder_path.join("."), arguments)),
                _ => None,
            })
            .expect("hole declaration");
        let site = expression_site(arguments).to_owned();
        let keyed_item = |key: &str, value: f64| ManagedExpression::Object {
            fields: vec![
                ManagedObjectField {
                    name: "key".into(),
                    value: ManagedExpression::String {
                        value: key.into(),
                        site: site.clone(),
                    },
                    comments: Vec::new(),
                },
                ManagedObjectField {
                    name: "value".into(),
                    value: ManagedExpression::Number {
                        value,
                        site: site.clone(),
                    },
                    comments: Vec::new(),
                },
            ],
            site: site.clone(),
        };
        let ManagedExpression::Object { fields, .. } = arguments else {
            panic!("hole arguments must remain an object")
        };
        fields.push(ManagedObjectField {
            name: "keyedValues".into(),
            value: ManagedExpression::Array {
                values: vec![keyed_item("alpha", 3.0), keyed_item("beta", 9.0)],
                site: site.clone(),
            },
            comments: Vec::new(),
        });
        let target = ExecutedConsumerTarget::Declaration {
            declaration: "hole".into(),
            family,
        };
        for index in 0..2 {
            for field in ["key", "value"] {
                compiled
                    .artifact
                    .value_consumers
                    .push(crate::ExecutedValueConsumer {
                        value_site: site.clone(),
                        target: target.clone(),
                        property: vec![
                            ManagedPathSegment::Field("keyedValues".into()),
                            ManagedPathSegment::Index(index),
                            ManagedPathSegment::Field(field.into()),
                        ],
                    });
            }
        }
        compiled.normalized_source.push(' ');
        refresh_compiled_digests(&mut compiled);
        compiled.validate().expect("keyed compiler authority");
        compiled
    }

    fn direct_point_address() -> CodeWritableAddress {
        CodeWritableAddress::direct_point(
            ProjectKey("prepared-test".into()),
            SemanticSymbol("point".into()),
            crate::GeneratedMemberIdentity {
                allocation: 17,
                generation: 0,
            },
            SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
        )
    }

    fn direct_point_values_mut(
        compiled: &mut CompiledManagedSource,
    ) -> &mut Vec<ManagedExpression> {
        let arguments = compiled
            .ir
            .statements
            .iter_mut()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol, arguments, ..
                } if symbol == "point" => Some(arguments),
                _ => None,
            })
            .expect("generic point declaration");
        let ManagedExpression::Object { fields, .. } = arguments else {
            panic!("direct point arguments")
        };
        let ManagedExpression::Array { values, .. } = &mut fields
            .iter_mut()
            .find(|field| field.name == "point")
            .expect("point field")
            .value
        else {
            panic!("point array")
        };
        values
    }

    fn named_cubic_control_address() -> CodeWritableAddress {
        CodeWritableAddress::direct_point(
            ProjectKey("prepared-test".into()),
            SemanticSymbol("cubic".into()),
            crate::GeneratedMemberIdentity {
                allocation: 17,
                generation: 0,
            },
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("controls".into()),
                ManagedPathSegment::Index(0),
            ]),
        )
    }

    fn named_geometry_controls_fixture() -> CompiledManagedSource {
        CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-geometry-controls.json"
        )))
        .expect("managed named geometry controls fixture")
    }

    fn prepared_restore(
        compiled: &CompiledManagedSource,
    ) -> (ManagedMutationAuthority, PreparedManagedMutationRequest) {
        let authority = authority(compiled);
        let request = prepare_managed_mutation(
            &authority,
            compiled,
            ManagedSketchMutation::SetSuppressed {
                target: ManagedMutationTarget::Declaration {
                    declaration: "radius".into(),
                },
                suppressed: false,
            },
            41,
        )
        .expect("prepared suppression restore");
        (authority, request)
    }

    fn line_draft() -> ManagedDeclarationDraft {
        let point =
            |x, y| ManagedValue::Array(vec![ManagedValue::Number(x), ManagedValue::Number(y)]);
        ManagedDeclarationDraft {
            variable: "segment42".into(),
            symbol: "segment42".into(),
            builder_path: vec!["geometry".into(), "segment".into()],
            arguments: ManagedValue::Object(BTreeMap::from([
                ("start".into(), point(0.0, 0.0)),
                ("end".into(), point(8.0, 3.0)),
            ])),
            patch: None,
            group: Some(CANVAS_ADDITIONS_GROUP.into()),
            suppressed: Some(false),
            comments: None,
        }
    }

    fn unit_draft(
        variable: &str,
        units: impl IntoIterator<Item = (&'static str, f64)>,
    ) -> ManagedDeclarationDraft {
        ManagedDeclarationDraft {
            variable: variable.into(),
            symbol: variable.into(),
            builder_path: vec!["geometry".into(), "segment".into()],
            arguments: ManagedValue::Object(BTreeMap::from([(
                "nested".into(),
                ManagedValue::Array(
                    units
                        .into_iter()
                        .map(|(unit, value)| {
                            ManagedValue::Object(BTreeMap::from([(
                                "value".into(),
                                ManagedValue::Unit(crate::UnitLiteral {
                                    unit: unit.into(),
                                    value,
                                }),
                            )]))
                        })
                        .collect(),
                ),
            )])),
            patch: None,
            group: Some(CANVAS_ADDITIONS_GROUP.into()),
            suppressed: Some(false),
            comments: None,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the candidate fixture keeps its mutually authenticated IR, result catalog, consumers, and group in one audit block"
    )]
    fn inserted_line_candidate(mut compiled: CompiledManagedSource) -> CompiledManagedSource {
        let digest = compiled.ir.source_digest.clone();
        let site_id = |ordinal: u64| format!("{ordinal:064x}");
        let source_site = |ordinal, kind| crate::ManagedSourceSite {
            id: site_id(ordinal),
            kind,
            span: crate::ManagedSourceSpan { start: 0, end: 0 },
            source_digest: digest.clone(),
        };
        compiled.ir.source_sites.extend([
            source_site(100, crate::ManagedSourceSiteKind::Declaration),
            source_site(101, crate::ManagedSourceSiteKind::Value),
            source_site(102, crate::ManagedSourceSiteKind::Value),
            source_site(103, crate::ManagedSourceSiteKind::Value),
            source_site(104, crate::ManagedSourceSiteKind::Value),
            source_site(105, crate::ManagedSourceSiteKind::Value),
            source_site(106, crate::ManagedSourceSiteKind::Value),
            source_site(107, crate::ManagedSourceSiteKind::Value),
            source_site(108, crate::ManagedSourceSiteKind::GroupReference),
        ]);
        let arguments = ManagedExpression::Object {
            fields: vec![
                ManagedObjectField {
                    name: "end".into(),
                    value: ManagedExpression::Array {
                        values: vec![
                            ManagedExpression::Number {
                                value: 8.0,
                                site: site_id(103),
                            },
                            ManagedExpression::Number {
                                value: 3.0,
                                site: site_id(104),
                            },
                        ],
                        site: site_id(102),
                    },
                    comments: Vec::new(),
                },
                ManagedObjectField {
                    name: "start".into(),
                    value: ManagedExpression::Array {
                        values: vec![
                            ManagedExpression::Number {
                                value: 0.0,
                                site: site_id(106),
                            },
                            ManagedExpression::Number {
                                value: 0.0,
                                site: site_id(107),
                            },
                        ],
                        site: site_id(105),
                    },
                    comments: Vec::new(),
                },
            ],
            site: site_id(101),
        };
        let group_index = compiled
            .ir
            .statements
            .iter()
            .position(|statement| {
                matches!(statement, ManagedStatement::Group { name, .. } if name == CANVAS_ADDITIONS_GROUP)
            })
            .expect("fixture Canvas additions group");
        compiled.ir.statements.insert(
            group_index,
            ManagedStatement::Declaration {
                variable: "segment42".into(),
                symbol: "segment42".into(),
                builder_path: vec!["geometry".into(), "segment".into()],
                patch: None,
                arguments,
                site: site_id(100),
                comments: Vec::new(),
            },
        );
        let ManagedStatement::Group { declarations, .. } =
            &mut compiled.ir.statements[group_index + 1]
        else {
            panic!("fixture group moved")
        };
        declarations.push(ManagedReference {
            declaration: "segment42".into(),
            path: Vec::new(),
            site: site_id(108),
        });
        compiled
            .artifact
            .declarations
            .push(crate::ExecutedDeclarationResult {
                declaration: "segment42".into(),
                family: "geometry.segment".into(),
                patch: None,
                site: site_id(100),
                result: vec![
                    ExecutedResultLeaf {
                        kind: FeatureKind::Curve,
                        path: vec![ManagedPathSegment::Field("curve".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::Point,
                        path: vec![ManagedPathSegment::Field("end".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::CurveSpan,
                        path: vec![ManagedPathSegment::Field("span".into())],
                    },
                    ExecutedResultLeaf {
                        kind: FeatureKind::Point,
                        path: vec![ManagedPathSegment::Field("start".into())],
                    },
                ],
            });
        let target = ExecutedConsumerTarget::Declaration {
            declaration: "segment42".into(),
            family: "geometry.segment".into(),
        };
        compiled.artifact.value_consumers.extend([
            crate::ExecutedValueConsumer {
                value_site: site_id(103),
                target: target.clone(),
                property: vec![
                    ManagedPathSegment::Field("end".into()),
                    ManagedPathSegment::Index(0),
                ],
            },
            crate::ExecutedValueConsumer {
                value_site: site_id(104),
                target: target.clone(),
                property: vec![
                    ManagedPathSegment::Field("end".into()),
                    ManagedPathSegment::Index(1),
                ],
            },
            crate::ExecutedValueConsumer {
                value_site: site_id(106),
                target: target.clone(),
                property: vec![
                    ManagedPathSegment::Field("start".into()),
                    ManagedPathSegment::Index(0),
                ],
            },
            crate::ExecutedValueConsumer {
                value_site: site_id(107),
                target,
                property: vec![
                    ManagedPathSegment::Field("start".into()),
                    ManagedPathSegment::Index(1),
                ],
            },
        ]);
        let group = compiled
            .ir
            .statements
            .iter()
            .find_map(|statement| match statement {
                ManagedStatement::Group {
                    name,
                    declarations,
                    site,
                    ..
                } if name == CANVAS_ADDITIONS_GROUP => Some(crate::ExecutedGroup {
                    name: name.clone(),
                    site: site.clone(),
                    declarations: declarations.clone(),
                }),
                _ => None,
            })
            .expect("candidate group");
        let artifact_group = compiled
            .artifact
            .groups
            .iter_mut()
            .find(|candidate| candidate.name == CANVAS_ADDITIONS_GROUP)
            .expect("fixture artifact group");
        *artifact_group = group;
        compiled.normalized_source.push(' ');
        refresh_compiled_digests(&mut compiled);
        compiled
    }

    #[test]
    fn wire_shapes_match_the_typescript_mutation_api() {
        let current = compiled_fixture();
        let (_, request) = prepared_restore(&current);
        let request_json = serde_json::to_value(&request).expect("request JSON");
        assert_eq!(
            request_json["ticket"]["mutation"]["mutation"],
            "set_suppressed"
        );
        assert!(request_json["ticket"].get("ticketDigest").is_some());
        assert!(request_json["ticket"].get("acceptedSourceDigest").is_some());
        assert!(request_json["current"].get("normalizedSource").is_some());

        let host = ManagedMutationReceipt {
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: current.ir.source_digest.clone(),
            compiled: canonical_host_compiled(current),
        };
        let host_json = serde_json::to_value(host).expect("host receipt JSON");
        assert!(host_json.get("baseSourceDigest").is_some());
        assert!(host_json.get("candidateSourceDigest").is_some());
        assert!(host_json.get("compiled").is_some());
    }

    #[test]
    fn profile_offset_root_reorder_moves_its_authenticated_helper_block() {
        let current = profile_offset_compiled_for_mutation();
        let mut statements = current.ir.statements.clone();
        reorder_declaration(&current, &mut statements, "profileOffset12", None)
            .expect("Profile Offset closure can move as one block");
        assert_eq!(
            ordered_declaration_symbols(&statements),
            ["base", "marker", "offsetChain11", "profileOffset12"],
        );

        let mut statements = current.ir.statements.clone();
        reorder_declaration(&current, &mut statements, "marker", Some("profileOffset12"))
            .expect("destination root resolves to its earliest closure member");
        assert_eq!(
            ordered_declaration_symbols(&statements),
            ["base", "marker", "offsetChain11", "profileOffset12"],
        );
    }

    #[test]
    fn profile_offset_helper_lifecycle_mutations_are_precisely_refused() {
        let current = profile_offset_compiled_for_mutation();
        let mut statements = current.ir.statements.clone();
        let moved = reorder_declaration(&current, &mut statements, "offsetChain11", None)
            .expect_err("helper cannot move independently");
        assert_eq!(
            moved,
            PreparedManagedMutationError::OwnedHelperMutation {
                helper: "offsetChain11".into(),
                root: "profileOffset12".into(),
                mutation: ManagedSourceDeclarationHelperMutation::Move,
            }
        );

        let mut statements = current.ir.statements.clone();
        let destination =
            reorder_declaration(&current, &mut statements, "marker", Some("offsetChain11"))
                .expect_err("helper cannot be a destination");
        assert_eq!(
            destination,
            PreparedManagedMutationError::OwnedHelperMutation {
                helper: "offsetChain11".into(),
                root: "profileOffset12".into(),
                mutation: ManagedSourceDeclarationHelperMutation::ReorderDestination,
            }
        );

        let target = ManagedMutationTarget::Declaration {
            declaration: "offsetChain11".into(),
        };
        let suppressed = refuse_helper_suppression(&current, &target)
            .expect_err("helper cannot be suppressed independently");
        assert_eq!(
            suppressed,
            PreparedManagedMutationError::OwnedHelperMutation {
                helper: "offsetChain11".into(),
                root: "profileOffset12".into(),
                mutation: ManagedSourceDeclarationHelperMutation::Suppress,
            }
        );

        let mut statements = current.ir.statements.clone();
        let deleted = delete_declaration_closure(&current, &mut statements, "offsetChain11")
            .expect_err("helper cannot be deleted independently");
        assert_eq!(
            deleted,
            PreparedManagedMutationError::OwnedHelperMutation {
                helper: "offsetChain11".into(),
                root: "profileOffset12".into(),
                mutation: ManagedSourceDeclarationHelperMutation::Delete,
            }
        );
    }

    #[test]
    fn profile_offset_root_delete_removes_helper_and_preserves_unrelated_roots() {
        let current = profile_offset_compiled_for_mutation();
        let mut statements = current.ir.statements.clone();
        delete_declaration_closure(&current, &mut statements, "profileOffset12")
            .expect("root deletion owns its authenticated helper");
        assert_eq!(ordered_declaration_symbols(&statements), ["base", "marker"]);
        let [ManagedStatement::Group { declarations, .. }] = statements
            .iter()
            .filter(|statement| matches!(statement, ManagedStatement::Group { .. }))
            .collect::<Vec<_>>()
            .as_slice()
        else {
            panic!("retained non-empty group")
        };
        assert_eq!(
            declarations
                .iter()
                .map(|reference| reference.declaration.as_str())
                .collect::<Vec<_>>(),
            ["base_variable", "marker_variable"],
        );
    }

    #[test]
    fn profile_offset_typescript_receipt_matches_dependency_safe_direct_reorder() {
        let current = profile_offset_fixture("base");
        let accepted = authority(&current);

        let reorder = prepare_managed_mutation(
            &accepted,
            &current,
            ManagedSketchMutation::ReorderDeclaration {
                declaration: "guide".into(),
                before: None,
            },
            accepted.declaration_name_high_water,
        )
        .expect("Rust-prepared dependency-safe direct reorder");
        let reordered = profile_offset_fixture("reordered");
        validate_prepared_managed_mutation(
            &accepted,
            &reorder,
            PreparedManagedMutationReceipt {
                ticket_digest: reorder.ticket.ticket_digest.clone(),
                base_source_digest: current.ir.source_digest.clone(),
                candidate_source_digest: reordered.ir.source_digest.clone(),
                compiled: reordered,
            },
        )
        .expect("TypeScript receipt exactly matches the direct declaration reorder");
    }

    #[test]
    fn profile_offset_dependency_violation_is_refused_before_a_typescript_host_request() {
        let current = profile_offset_fixture("base");
        let accepted = authority(&current);
        let error = prepare_managed_mutation(
            &accepted,
            &current,
            ManagedSketchMutation::ReorderDeclaration {
                declaration: "profileOffset12".into(),
                before: Some("base".into()),
            },
            accepted.declaration_name_high_water,
        )
        .expect_err("operation closure cannot move before its explicit geometry dependency");
        assert_eq!(
            error,
            PreparedManagedMutationError::SemanticDelta(
                "reorder would place a consumer before its dependency".into()
            )
        );
    }

    #[test]
    fn shared_profile_aggregate_remains_an_independent_deletion_root() {
        let mut current = profile_offset_compiled_for_mutation();
        current.ir.statements.insert(
            3,
            closure_declaration(
                "other_root_variable",
                "profileOffset13",
                ["operation", "profileOffset"],
                closure_object([closure_field(
                    "sources",
                    ManagedExpression::Array {
                        values: vec![ManagedExpression::Reference {
                            declaration: "helper_variable".into(),
                            path: vec![ManagedPathSegment::Field("chain".into())],
                            site: String::new(),
                        }],
                        site: String::new(),
                    },
                )]),
            ),
        );
        assert!(current.projected_source_declaration_closures().is_empty());

        let mut statements = current.ir.statements.clone();
        delete_declaration_closure(&current, &mut statements, "profileOffset12")
            .expect("shared aggregate is not owned by one Profile Offset");
        assert_eq!(
            ordered_declaration_symbols(&statements),
            ["base", "offsetChain11", "profileOffset13", "marker"],
        );
    }

    #[test]
    fn exact_suppression_delta_validates_without_mutating_accepted_authority() {
        let current = compiled_fixture();
        let retained = current.clone();
        let (accepted_authority, request) = prepared_restore(&current);
        let candidate = remove_radius_suppression(current.clone());
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        let validated = validate_prepared_managed_mutation(&accepted_authority, &request, receipt)
            .expect("exact suppression restore");
        assert_eq!(validated.compiled(), &candidate);
        assert_eq!(validated.declaration_name_high_water(), 41);
        assert_eq!(current, retained);
        assert_eq!(accepted_authority, authority(&current));
    }

    #[test]
    fn exact_raw_source_ticket_authenticates_candidate_bytes_and_compiler_receipt() {
        let current = compiled_fixture();
        let accepted = authority(&current);
        let candidate = replace_shared_radius(current.clone(), 7.0);
        let request =
            prepare_managed_source(&accepted, &current, candidate.normalized_source.clone())
                .expect("exact raw-source request");
        assert_eq!(request.current, current);
        assert_eq!(request.candidate_source, candidate.normalized_source);
        let validated = validate_prepared_managed_source(
            &accepted,
            &request,
            PreparedManagedMutationReceipt {
                ticket_digest: request.ticket.ticket_digest.clone(),
                base_source_digest: current.ir.source_digest.clone(),
                candidate_source_digest: candidate.ir.source_digest.clone(),
                compiled: candidate.clone(),
            },
        )
        .expect("exact raw-source receipt");
        assert_eq!(validated.compiled(), &candidate);
        assert_eq!(validated.declaration_name_high_water(), 41);
        assert_eq!(validated.project(), &accepted.project);
        assert_eq!(validated.session(), &accepted.session);
    }

    #[test]
    fn raw_source_ticket_rejects_tampered_bytes_stale_authority_and_foreign_receipt() {
        let current = compiled_fixture();
        let accepted = authority(&current);
        let candidate = replace_shared_radius(current.clone(), 7.0);
        let mut request =
            prepare_managed_source(&accepted, &current, candidate.normalized_source.clone())
                .expect("exact raw-source request");
        request.candidate_source.push(' ');
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        assert!(matches!(
            validate_prepared_managed_source(&accepted, &request, receipt.clone()),
            Err(PreparedManagedMutationError::InvalidTicket(_))
        ));

        let request =
            prepare_managed_source(&accepted, &current, candidate.normalized_source.clone())
                .expect("exact raw-source request");
        let mut stale = accepted.clone();
        stale.session.revision += 1;
        assert!(matches!(
            validate_prepared_managed_source(&stale, &request, receipt.clone()),
            Err(PreparedManagedMutationError::StaleAuthority(_))
        ));

        let mut foreign = receipt;
        foreign.ticket_digest = "f".repeat(64);
        assert!(matches!(
            validate_prepared_managed_source(&accepted, &request, foreign),
            Err(PreparedManagedMutationError::InvalidReceipt(_))
        ));
    }

    #[test]
    fn stale_session_and_replayed_ticket_are_rejected_transactionally() {
        let current = compiled_fixture();
        let retained = current.clone();
        let (authority, request) = prepared_restore(&current);
        let candidate = remove_radius_suppression(current.clone());
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        let mut stale = authority.clone();
        stale.session.revision += 1;
        assert!(matches!(
            validate_prepared_managed_mutation(&stale, &request, receipt),
            Err(PreparedManagedMutationError::StaleAuthority(_))
        ));
        assert_eq!(current, retained);
    }

    #[test]
    fn valid_wrong_operation_candidate_is_rejected_by_exact_delta() {
        let current = compiled_fixture();
        let authority = authority(&current);
        let request = prepare_managed_mutation(
            &authority,
            &current,
            ManagedSketchMutation::SetSuppressed {
                target: ManagedMutationTarget::Declaration {
                    declaration: "radius".into(),
                },
                suppressed: true,
            },
            41,
        )
        .expect("prepared no-op suppression");
        let candidate = remove_radius_suppression(current.clone());
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        assert!(matches!(
            validate_prepared_managed_mutation(&authority, &request, receipt),
            Err(PreparedManagedMutationError::InvalidReceipt(_)
                | PreparedManagedMutationError::SemanticDelta(_))
        ));
    }

    #[test]
    fn tampered_ticket_and_compiler_receipt_are_rejected() {
        let current = compiled_fixture();
        let (authority, mut request) = prepared_restore(&current);
        request.ticket.accepted_expansion_digest = "3".repeat(64);
        let candidate = remove_radius_suppression(current.clone());
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        assert!(matches!(
            validate_prepared_managed_mutation(&authority, &request, receipt),
            Err(PreparedManagedMutationError::InvalidTicket(_))
        ));

        let (_, request) = prepared_restore(&current);
        let mut candidate = remove_radius_suppression(current.clone());
        candidate.artifact.artifact_digest = "4".repeat(64);
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        assert!(matches!(
            validate_prepared_managed_mutation(&authority, &request, receipt),
            Err(PreparedManagedMutationError::InvalidReceipt(_))
        ));
    }

    #[test]
    fn declaration_allocator_is_monotonic_and_operation_bound() {
        let current = compiled_fixture();
        let authority = authority(&current);
        let draft = ManagedDeclarationDraft {
            variable: "segment42".into(),
            symbol: "segment42".into(),
            builder_path: vec!["geometry".into(), "segment".into()],
            arguments: ManagedValue::Object(BTreeMap::new()),
            patch: None,
            group: Some(CANVAS_ADDITIONS_GROUP.into()),
            suppressed: Some(false),
            comments: None,
        };
        assert!(matches!(
            prepare_managed_mutation(
                &authority,
                &current,
                ManagedSketchMutation::InsertDeclarations {
                    declarations: vec![draft]
                },
                41,
            ),
            Err(PreparedManagedMutationError::InvalidTicket(_))
        ));
        assert!(matches!(
            prepare_managed_mutation(
                &authority,
                &current,
                ManagedSketchMutation::Delete {
                    target: ManagedMutationTarget::Declaration {
                        declaration: "radius".into()
                    }
                },
                42,
            ),
            Err(PreparedManagedMutationError::InvalidTicket(_))
        ));
    }

    #[test]
    fn insertion_result_and_runtime_consumer_delta_are_rust_authenticated() {
        let current = compiled_fixture();
        let authority = authority(&current);
        let request = prepare_managed_mutation(
            &authority,
            &current,
            ManagedSketchMutation::InsertDeclarations {
                declarations: vec![line_draft()],
            },
            42,
        )
        .expect("prepared line insertion");
        let candidate = inserted_line_candidate(current.clone());
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        validate_prepared_managed_mutation(&authority, &request, receipt)
            .expect("exact inserted result and consumers");

        let mut forged = candidate;
        forged
            .artifact
            .declarations
            .last_mut()
            .expect("inserted declaration")
            .result[0]
            .kind = FeatureKind::Feature;
        refresh_compiled_digests(&mut forged);
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: forged.ir.source_digest.clone(),
            compiled: forged,
        };
        assert!(matches!(
            validate_prepared_managed_mutation(&authority, &request, receipt),
            Err(PreparedManagedMutationError::InvalidReceipt(_)
                | PreparedManagedMutationError::SemanticDelta(_))
        ));
    }

    #[test]
    fn canvas_circle_insertion_authorizes_only_its_generated_mm_import() {
        let current = empty_compiled_fixture();
        let candidate = empty_circle_fixture();
        let (builder_path, arguments) = candidate
            .ir
            .statements
            .iter()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    builder_path,
                    arguments,
                    ..
                } if symbol == "geometry1" => Some((builder_path.clone(), arguments)),
                _ => None,
            })
            .expect("empty-circle declaration");
        let mutation = ManagedSketchMutation::InsertDeclarations {
            declarations: vec![ManagedDeclarationDraft {
                variable: "geometry1".into(),
                symbol: "geometry1".into(),
                builder_path,
                arguments: expression_to_managed_value(arguments)
                    .expect("empty-circle managed arguments"),
                patch: None,
                group: Some(CANVAS_ADDITIONS_GROUP.into()),
                suppressed: Some(false),
                comments: None,
            }],
        };
        let expected =
            apply_expected_mutation(&current, &mutation).expect("expected circle insertion");
        assert_eq!(
            expected.imports,
            vec![ManagedIrImport {
                module: "@geosolve/sketch-code".into(),
                bindings: vec!["sketch".into(), "mm".into()],
            }]
        );

        let accepted_authority = authority(&current);
        let request = prepare_managed_mutation(
            &accepted_authority,
            &current,
            mutation,
            accepted_authority.declaration_name_high_water + 1,
        )
        .expect("prepared empty-circle insertion");
        assert_eq!(
            request.ticket.candidate_semantics_digest,
            semantic_ir_digest(&expected.imports, &expected.statements)
                .expect("expected candidate semantic digest")
        );
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        validate_prepared_managed_mutation(&accepted_authority, &request, receipt)
            .expect("exact empty-circle compiler receipt");

        for mut forged in [
            {
                let mut forged = candidate.clone();
                forged.ir.imports[0].bindings.push("rad".into());
                forged
            },
            {
                let mut forged = candidate.clone();
                forged.ir.imports[0].bindings.swap(0, 1);
                forged
            },
            {
                let mut forged = candidate.clone();
                forged.ir.imports[0]
                    .bindings
                    .retain(|binding| binding != "mm");
                forged
            },
            {
                let mut forged = candidate.clone();
                forged.ir.imports.push(ManagedIrImport {
                    module: "./unrelated.patch.ts".into(),
                    bindings: vec!["unrelated".into()],
                });
                forged
            },
        ] {
            forged.normalized_source.push(' ');
            refresh_compiled_digests(&mut forged);
            let receipt = PreparedManagedMutationReceipt {
                ticket_digest: request.ticket.ticket_digest.clone(),
                base_source_digest: current.ir.source_digest.clone(),
                candidate_source_digest: forged.ir.source_digest.clone(),
                compiled: forged,
            };
            assert!(matches!(
                validate_prepared_managed_mutation(&accepted_authority, &request, receipt),
                Err(PreparedManagedMutationError::SemanticDelta(_))
            ));
        }
    }

    #[test]
    fn insertion_unit_helper_closure_is_minimal_recursive_and_canonical() {
        let current = empty_compiled_fixture();
        let expected = apply_expected_mutation(
            &current,
            &ManagedSketchMutation::InsertDeclarations {
                declarations: vec![line_draft()],
            },
        )
        .expect("unit-free insertion");
        assert_eq!(expected.imports, current.ir.imports);

        let mut existing_order = current.clone();
        existing_order.ir.imports[0].bindings = vec!["cm".into(), "sketch".into()];
        let expected = apply_expected_mutation(
            &existing_order,
            &ManagedSketchMutation::InsertDeclarations {
                declarations: vec![
                    unit_draft("angle42", [("rad", 0.5), ("rad", 1.0)]),
                    unit_draft("length43", [("mm", 2.0)]),
                ],
            },
        )
        .expect("mixed nested unit insertion");
        assert_eq!(expected.imports[0].bindings, ["cm", "sketch", "mm", "rad"]);

        let rad_only = apply_expected_mutation(
            &current,
            &ManagedSketchMutation::InsertDeclarations {
                declarations: vec![unit_draft("angle42", [("rad", 0.5)])],
            },
        )
        .expect("angle-only insertion");
        assert_eq!(rad_only.imports[0].bindings, ["sketch", "rad"]);
        let rad_arguments = rad_only
            .statements
            .iter()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol, arguments, ..
                } if symbol == "angle42" => Some(arguments),
                _ => None,
            })
            .expect("inserted angle arguments");
        assert_eq!(
            expression_to_managed_value(rad_arguments).expect("inserted angle value"),
            unit_draft("angle42", [("rad", 0.5)]).arguments
        );

        let mut split = current.clone();
        split.ir.imports.extend([
            ManagedIrImport {
                module: "@geosolve/sketch-code".into(),
                bindings: vec!["rad".into()],
            },
            ManagedIrImport {
                module: "@geosolve/sketch-code".into(),
                bindings: vec!["mm".into()],
            },
        ]);
        let expected = apply_expected_mutation(
            &split,
            &ManagedSketchMutation::InsertDeclarations {
                declarations: vec![unit_draft("unit42", [("mm", 2.0), ("rad", 0.5)])],
            },
        )
        .expect("already imported helpers");
        assert_eq!(expected.imports, split.ir.imports);

        let mut split_mm = current.clone();
        split_mm.ir.imports.push(ManagedIrImport {
            module: "@geosolve/sketch-code".into(),
            bindings: vec!["mm".into()],
        });
        let expected = apply_expected_mutation(
            &split_mm,
            &ManagedSketchMutation::InsertDeclarations {
                declarations: vec![unit_draft("unit42", [("mm", 2.0), ("rad", 0.5)])],
            },
        )
        .expect("partially imported helpers");
        assert_eq!(expected.imports[0].bindings, ["sketch", "rad"]);
        assert_eq!(expected.imports[1].bindings, ["mm"]);
    }

    #[test]
    fn insertion_units_and_import_owner_fail_closed() {
        let current = empty_compiled_fixture();
        assert!(matches!(
            apply_expected_mutation(
                &current,
                &ManagedSketchMutation::InsertDeclarations {
                    declarations: vec![unit_draft("unit42", [("cm", 2.0)])],
                },
            ),
            Err(PreparedManagedMutationError::SemanticDelta(_))
        ));

        let mut no_owner = current;
        no_owner.ir.imports[0].bindings = vec!["mm".into()];
        assert!(matches!(
            apply_expected_mutation(
                &no_owner,
                &ManagedSketchMutation::InsertDeclarations {
                    declarations: vec![unit_draft("unit42", [("mm", 2.0), ("rad", 0.5)])],
                },
            ),
            Err(PreparedManagedMutationError::SemanticDelta(_))
        ));
    }

    #[test]
    fn non_insertion_mutations_preserve_imports_exactly() {
        let current = compiled_fixture();
        let expected = apply_expected_mutation(
            &current,
            &ManagedSketchMutation::SetSuppressed {
                target: ManagedMutationTarget::Declaration {
                    declaration: "radius".into(),
                },
                suppressed: false,
            },
        )
        .expect("suppression mutation");
        assert_eq!(expected.imports, current.ir.imports);
    }

    #[test]
    fn open_polyline_insertion_result_uses_exact_keyed_subsets() {
        let compiled = named_polyline_fixture();
        let arguments = compiled
            .ir
            .statements
            .iter()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol, arguments, ..
                } if symbol == "geometry1" => Some(arguments),
                _ => None,
            })
            .expect("named Polyline IR declaration");
        let actual = &compiled
            .artifact
            .declarations
            .iter()
            .find(|declaration| declaration.declaration == "geometry1")
            .expect("named Polyline artifact declaration")
            .result;
        let descriptor = declaration_result_catalog()
            .remove("geometry.polyline")
            .expect("Polyline result descriptor");
        let expected =
            direct_declaration_result_leaves("geometry.polyline", arguments, &descriptor.outputs);

        assert_eq!(
            &expected, actual,
            "prepared insertion validation must use every vertex, n-1 open spans, and only interior open corners",
        );
    }

    #[test]
    fn named_constraint_insertion_uses_rust_result_authority() {
        let candidate = named_constraint_fixture();
        let (builder_path, arguments) = candidate
            .ir
            .statements
            .iter()
            .find_map(|statement| match statement {
                ManagedStatement::Declaration {
                    symbol,
                    builder_path,
                    arguments,
                    ..
                } if symbol == "tangent" => Some((builder_path.clone(), arguments)),
                _ => None,
            })
            .expect("named constraint declaration");
        let mutation = ManagedSketchMutation::InsertDeclarations {
            declarations: vec![ManagedDeclarationDraft {
                variable: "tangent".into(),
                symbol: "tangent".into(),
                builder_path,
                arguments: expression_to_managed_value(arguments).expect("managed arguments"),
                patch: None,
                group: None,
                suppressed: None,
                comments: None,
            }],
        };
        let inserted = BTreeSet::from(["tangent".into()]);
        validate_inserted_results(&candidate, &mutation, &inserted)
            .expect("named constraint result authority");

        let mut forged = candidate;
        forged
            .artifact
            .declarations
            .iter_mut()
            .find(|declaration| declaration.declaration == "tangent")
            .expect("executed named constraint")
            .result[0]
            .kind = FeatureKind::Curve;
        assert!(validate_inserted_results(&forged, &mutation, &inserted).is_err());
    }

    #[test]
    fn runtime_derived_control_value_mutation_is_exact_and_stale_safe() {
        let current = compiled_fixture();
        let accepted_authority = authority(&current);
        let mutation = ManagedSketchMutation::SetValue {
            declaration: "sharedRadius".into(),
            path: Vec::new(),
            expected: ManagedValue::Unit(crate::UnitLiteral {
                unit: "mm".into(),
                value: 4.0,
            }),
            value: ManagedValue::Unit(crate::UnitLiteral {
                unit: "mm".into(),
                value: 6.0,
            }),
        };
        let request = prepare_managed_mutation(
            &accepted_authority,
            &current,
            mutation,
            accepted_authority.declaration_name_high_water,
        )
        .expect("prepared runtime-derived control mutation");
        let candidate = replace_shared_radius(current.clone(), 6.0);
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        let validated = validate_prepared_managed_mutation(&accepted_authority, &request, receipt)
            .expect("exact source-value mutation");
        assert_eq!(validated.compiled(), &candidate);

        let stale = prepare_managed_mutation(
            &accepted_authority,
            &current,
            ManagedSketchMutation::SetValue {
                declaration: "sharedRadius".into(),
                path: Vec::new(),
                expected: ManagedValue::Unit(crate::UnitLiteral {
                    unit: "mm".into(),
                    value: 5.0,
                }),
                value: ManagedValue::Unit(crate::UnitLiteral {
                    unit: "mm".into(),
                    value: 6.0,
                }),
            },
            accepted_authority.declaration_name_high_water,
        );
        assert!(matches!(
            stale,
            Err(PreparedManagedMutationError::StaleAuthority(_))
        ));
    }

    #[test]
    fn direct_reference_detachment_authenticates_its_new_runtime_consumer_shape() {
        let current = compiled_fixture();
        let accepted_authority = authority(&current);
        let mutation = ManagedSketchMutation::SetValue {
            declaration: "segment".into(),
            path: vec![ManagedPathSegment::Field("start".into())],
            expected: ManagedValue::Reference {
                declaration: SemanticSymbol("point".into()),
                path: SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
            },
            value: ManagedValue::Array(vec![
                ManagedValue::Number(-1.999_999_999_999_999_3),
                ManagedValue::Number(4.0),
            ]),
        };
        let request = prepare_managed_mutation(
            &accepted_authority,
            &current,
            mutation,
            accepted_authority.declaration_name_high_water,
        )
        .expect("prepared reference detachment");
        let candidate = detached_reference_fixture();
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate.clone(),
        };
        let validated = validate_prepared_managed_mutation(&accepted_authority, &request, receipt)
            .expect("exact reference detachment");
        assert_eq!(validated.compiled(), &candidate);

        let mut forged = candidate;
        let consumer = forged
            .artifact
            .value_consumers
            .iter_mut()
            .find(|consumer| {
                consumer_target_owner(&consumer.target) == "segment"
                    && consumer.property
                        == [
                            ManagedPathSegment::Field("start".into()),
                            ManagedPathSegment::Index(0),
                        ]
            })
            .expect("detached X consumer");
        consumer.property[1] = ManagedPathSegment::Index(1);
        refresh_compiled_digests(&mut forged);
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: current.ir.source_digest.clone(),
            candidate_source_digest: forged.ir.source_digest.clone(),
            compiled: forged,
        };
        assert!(matches!(
            validate_prepared_managed_mutation(&accepted_authority, &request, receipt),
            Err(PreparedManagedMutationError::InvalidReceipt(_)
                | PreparedManagedMutationError::SemanticDelta(_))
        ));
    }

    #[test]
    fn derives_direct_field_and_index_value_mutation_with_exact_cas() {
        let current = compiled_fixture();
        let path = SemanticOutputPath(vec![
            ManagedPathSegment::Field("center".into()),
            ManagedPathSegment::Index(1),
        ]);
        let mutation = derive_managed_value_mutation(
            &current,
            &SemanticSymbol("hole".into()),
            &path,
            ManagedValue::Number(7.5),
        )
        .expect("direct numeric mutation");

        assert_eq!(mutation.declaration, "hole");
        assert_eq!(mutation.path, path.0);
        assert_eq!(mutation.expected, ManagedValue::Number(2.0));
        assert_eq!(mutation.value, ManagedValue::Number(7.5));
    }

    #[test]
    fn derives_reference_value_mutation_without_dereferencing_source() {
        let current = compiled_fixture();
        let path = SemanticOutputPath(vec![ManagedPathSegment::Field("start".into())]);
        let mutation = derive_managed_value_mutation(
            &current,
            &SemanticSymbol("segment".into()),
            &path,
            ManagedValue::Array(vec![ManagedValue::Number(-2.0), ManagedValue::Number(4.0)]),
        )
        .expect("reference replacement mutation");

        assert_eq!(mutation.path, path.0);
        assert_eq!(
            mutation.expected,
            ManagedValue::Reference {
                declaration: SemanticSymbol("point".into()),
                path: SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
            }
        );
    }

    #[test]
    fn direct_point_mutation_requires_exact_paired_runtime_provenance() {
        let current = compiled_fixture();
        let mutations = managed_point_value_mutations(
            &ProjectKey("prepared-test".into()),
            &current,
            &[(direct_point_address(), [3.0, 4.0])],
        )
        .expect("direct source point mutation");
        assert_eq!(mutations.len(), 1);
        assert_eq!(
            mutations[0].path,
            [ManagedPathSegment::Field("point".into())]
        );
        assert_eq!(
            mutations[0].expected,
            ManagedValue::Array(vec![ManagedValue::Number(1.25), ManagedValue::Number(-6.5),])
        );
        assert_eq!(
            mutations[0].value,
            ManagedValue::Array(vec![ManagedValue::Number(3.0), ManagedValue::Number(4.0)])
        );

        let mut missing_provenance = current;
        let removed = missing_provenance
            .artifact
            .value_consumers
            .iter()
            .position(|consumer| {
                matches!(
                    &consumer.target,
                    ExecutedConsumerTarget::Declaration { declaration, family }
                        if declaration == "point" && family == "geometry.sketchPoint"
                ) && consumer.property
                    == [
                        ManagedPathSegment::Field("point".into()),
                        ManagedPathSegment::Index(0),
                    ]
            })
            .expect("direct X consumer");
        missing_provenance.artifact.value_consumers.remove(removed);
        missing_provenance.normalized_source.push(' ');
        refresh_compiled_digests(&mut missing_provenance);
        assert!(
            managed_point_value_mutations(
                &ProjectKey("prepared-test".into()),
                &missing_provenance,
                &[(direct_point_address(), [3.0, 4.0])],
            )
            .is_err()
        );
    }

    #[test]
    fn named_cubic_native_control_maps_back_to_first_control_argument() {
        let current = named_geometry_controls_fixture();
        let mutations = managed_point_value_mutations(
            &ProjectKey("prepared-test".into()),
            &current,
            &[(named_cubic_control_address(), [3.0, 5.0])],
        )
        .expect("native controls[0] maps to the named firstControl source argument");

        assert_eq!(mutations.len(), 1);
        assert_eq!(
            mutations[0].path,
            [ManagedPathSegment::Field("firstControl".into())]
        );
        assert_eq!(
            mutations[0].expected,
            ManagedValue::Array(vec![ManagedValue::Number(2.0), ManagedValue::Number(4.0)])
        );
        assert_eq!(
            mutations[0].value,
            ManagedValue::Array(vec![ManagedValue::Number(3.0), ManagedValue::Number(5.0)])
        );
    }

    #[test]
    fn keyed_polyline_vertex_maps_through_its_key_to_the_lexical_array_member() {
        let current = named_polyline_fixture();
        let project = ProjectKey("prepared-test".into());
        let address = CodeWritableAddress::generated_point(
            project.clone(),
            crate::GeneratedMemberAddress::new(
                "geometry1",
                ["polyline", "vertex"],
                ["v1"],
                ["point"],
            ),
            crate::GeneratedMemberIdentity {
                allocation: 17,
                generation: 0,
            },
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("vertices".into()),
                ManagedPathSegment::Member {
                    member: "v1".into(),
                },
                ManagedPathSegment::Field("position".into()),
            ]),
        );
        let mutations =
            managed_point_value_mutations(&project, &current, &[(address, [21.0, 2.0])])
                .expect("keyed vertex maps to the exact lexical array entry");

        assert_eq!(mutations.len(), 1);
        assert_eq!(
            mutations[0].path,
            [
                ManagedPathSegment::Field("vertices".into()),
                ManagedPathSegment::Index(1),
                ManagedPathSegment::Field("position".into()),
            ]
        );
        assert_eq!(
            mutations[0].expected,
            ManagedValue::Array(vec![ManagedValue::Number(20.0), ManagedValue::Number(0.0)])
        );
    }

    #[test]
    fn direct_point_mutation_rejects_missing_duplicate_and_malformed_components() {
        let assert_refused = |mut compiled: CompiledManagedSource| {
            compiled.normalized_source.push(' ');
            refresh_compiled_digests(&mut compiled);
            assert!(
                managed_point_value_mutations(
                    &ProjectKey("prepared-test".into()),
                    &compiled,
                    &[(direct_point_address(), [3.0, 4.0])],
                )
                .is_err()
            );
        };

        let mut missing = compiled_fixture();
        direct_point_values_mut(&mut missing).pop();
        assert_refused(missing);

        let mut duplicate = compiled_fixture();
        let duplicate_y = direct_point_values_mut(&mut duplicate)[1].clone();
        direct_point_values_mut(&mut duplicate).push(duplicate_y);
        assert_refused(duplicate);

        let mut malformed = compiled_fixture();
        let y = &mut direct_point_values_mut(&mut malformed)[1];
        let site = expression_site(y).to_owned();
        *y = ManagedExpression::String {
            value: "not-a-coordinate".into(),
            site,
        };
        assert_refused(malformed);
    }

    #[test]
    fn generated_and_runtime_only_points_have_no_source_mutation_coordinate() {
        let current = compiled_fixture();
        let project = ProjectKey("prepared-test".into());
        let generated_address =
            crate::GeneratedMemberAddress::new("point", ["template"], ["member"], ["point"]);
        let generated = CodeWritableAddress::generated_point(
            project.clone(),
            generated_address,
            crate::GeneratedMemberIdentity {
                allocation: 19,
                generation: 0,
            },
            SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
        );
        assert!(
            managed_point_value_mutations(&project, &current, &[(generated, [3.0, 4.0])],).is_err()
        );

        let runtime_only = CodeWritableAddress::direct_point(
            project.clone(),
            SemanticSymbol("point".into()),
            crate::GeneratedMemberIdentity {
                allocation: 20,
                generation: 0,
            },
            SemanticOutputPath(vec![ManagedPathSegment::Field("runtimeOnly".into())]),
        );
        assert!(
            managed_point_value_mutations(&project, &current, &[(runtime_only, [3.0, 4.0])],)
                .is_err()
        );
    }

    #[test]
    fn point_reference_detachment_requires_one_exact_executed_consumer_site() {
        let current = compiled_fixture();
        let project = ProjectKey("prepared-test".into());
        let start = SemanticOutputPath(vec![ManagedPathSegment::Field("start".into())]);
        let address = CodeWritableAddress::direct_point(
            project.clone(),
            SemanticSymbol("segment".into()),
            crate::GeneratedMemberIdentity {
                allocation: 17,
                generation: 0,
            },
            start.clone(),
        );
        let target = [-2.0, 4.0];
        let values =
            managed_point_value_mutations(&project, &current, &[(address.clone(), target)])
                .expect("one lexical result reference can detach to a local point pair");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].declaration, "segment");
        assert_eq!(values[0].path, start.0);
        assert_eq!(
            values[0].expected,
            ManagedValue::Reference {
                declaration: SemanticSymbol("point".into()),
                path: SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
            }
        );
        assert_eq!(
            values[0].value,
            ManagedValue::Array(vec![
                ManagedValue::Number(target[0]),
                ManagedValue::Number(target[1]),
            ])
        );

        let mut missing = current.clone();
        missing.artifact.value_consumers.retain(|consumer| {
            !(matches!(
                &consumer.target,
                ExecutedConsumerTarget::Declaration { declaration, .. }
                    if declaration == "segment"
            ) && consumer.property == start.0)
        });
        refresh_compiled_digests(&mut missing);
        assert!(matches!(
            managed_point_value_mutations(&project, &missing, &[(address.clone(), target)]),
            Err(PreparedManagedMutationError::InvalidReceipt(_)
                | PreparedManagedMutationError::SemanticDelta(_))
        ));

        let mut ambiguous = current;
        let exact = ambiguous
            .artifact
            .value_consumers
            .iter()
            .find(|consumer| {
                matches!(
                    &consumer.target,
                    ExecutedConsumerTarget::Declaration { declaration, .. }
                        if declaration == "segment"
                ) && consumer.property == start.0
            })
            .expect("fixture reference consumer")
            .clone();
        ambiguous.artifact.value_consumers.push(exact);
        refresh_compiled_digests(&mut ambiguous);
        assert!(matches!(
            managed_point_value_mutations(&project, &ambiguous, &[(address, target)]),
            Err(PreparedManagedMutationError::InvalidReceipt(_)
                | PreparedManagedMutationError::SemanticDelta(_))
        ));
    }

    #[test]
    fn derives_keyed_member_as_its_exact_lexical_array_index() {
        let current = compiled_with_keyed_values();
        let mutation = derive_managed_value_mutation(
            &current,
            &SemanticSymbol("hole".into()),
            &SemanticOutputPath(vec![
                ManagedPathSegment::Field("keyedValues".into()),
                ManagedPathSegment::Member {
                    member: "beta".into(),
                },
                ManagedPathSegment::Field("value".into()),
            ]),
            ManagedValue::Number(11.0),
        )
        .expect("keyed member mutation");

        assert_eq!(
            mutation.path,
            vec![
                ManagedPathSegment::Field("keyedValues".into()),
                ManagedPathSegment::Index(1),
                ManagedPathSegment::Field("value".into()),
            ]
        );
        assert_eq!(mutation.expected, ManagedValue::Number(9.0));
    }

    #[test]
    fn keyed_member_resolution_rejects_absent_duplicate_and_malformed_keys() {
        let string = |value: &str| ManagedExpression::String {
            value: value.into(),
            site: "site".into(),
        };
        let item = |key: ManagedExpression| ManagedExpression::Object {
            fields: vec![ManagedObjectField {
                name: "key".into(),
                value: key,
                comments: Vec::new(),
            }],
            site: "site".into(),
        };

        assert!(unique_keyed_member_index(&[item(string("other"))], "wanted").is_err());
        assert!(
            unique_keyed_member_index(&[item(string("wanted")), item(string("wanted"))], "wanted")
                .is_err()
        );
        assert!(
            unique_keyed_member_index(
                &[item(ManagedExpression::Number {
                    value: 1.0,
                    site: "site".into(),
                })],
                "wanted"
            )
            .is_err()
        );
    }

    #[test]
    fn prepared_wire_writer_refuses_before_crossing_its_exact_bound() {
        let mut writer = BoundedJsonWriter {
            written: PREPARED_MANAGED_MUTATION_WIRE_LIMIT,
        };
        assert_eq!(
            writer.write(&[0]).expect_err("one byte over bound").kind(),
            io::ErrorKind::FileTooLarge,
        );
        assert_eq!(writer.written, PREPARED_MANAGED_MUTATION_WIRE_LIMIT);
    }
}
