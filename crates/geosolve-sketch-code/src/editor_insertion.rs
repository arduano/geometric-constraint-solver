// SPDX-License-Identifier: GPL-3.0-or-later

//! Authenticated reverse projection of newly accepted editor declarations.
//!
//! This module deliberately stops before source mutation. It turns one exact
//! accepted editor-graph delta into bounded, typed declaration drafts which a
//! managed compiler host can insert into its IR. Source sites, byte spans
//! and artifact provenance are compiler outputs and are never fabricated by
//! Rust.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, IntentNativeBinding, NativeCurveSpanSource, NewComputedFilletCorner,
    ProjectionalEditorSession,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, DocumentArcSweep, DocumentCurveControlKind,
    DocumentCurveNormalSide, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    TangentOrientation,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, ConstraintKind, DimensionKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentLiteral, IntentNode, IntentNodeKind, IntentPortKind, IntentPortRef, IntentPortRole,
    IntentPortSelector, IntentProjectionPath, IntentProjectionPathSegment, IntentUnit, LeafField,
    LeafRef, NodeId, OperationKind, intent_content_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::declaration_catalog::CodeAuthoringInputBinding;
use crate::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringAvailability, CodeAuthoringDeclarationKind,
    CodeAuthoringDynamicChildren, CodeProject, CollectionRule, ExpandedCodeProject,
    ExpandedSemanticTarget, ManagedPathSegment, ManagedValue, PatchModuleArtifact,
    SemanticOutputPath, SemanticSymbol, UnitLiteral, resolve_code_authoring_declaration,
};

/// Presentation/source group which owns every declaration added from canvas
/// authoring in managed V3.
pub const CANVAS_ADDITIONS_GROUP: &str = "Canvas additions";

/// One accepted editor declaration selected for source projection.
///
/// The pair is intentionally independent of the retired whole-document
/// bootstrap path: canvas publication projects only the authenticated delta
/// into a Rust-prepared V3 compiler mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorBootstrapDeclaration {
    pub node: NodeId,
    pub symbol: SemanticSymbol,
}

impl EditorBootstrapDeclaration {
    #[must_use]
    pub const fn new(node: NodeId, symbol: SemanticSymbol) -> Self {
        Self { node, symbol }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingSourceOwner {
    declaration: SemanticSymbol,
    /// Smallest authenticated lexical result which reaches this node. Direct
    /// declarations use the empty path; patch-generated nodes use their
    /// compiler-recorded public member path.
    node_path: SemanticOutputPath,
    /// Exact public paths for ports exposed by a generated result. These let
    /// ergonomic builders reuse a point directly without guessing a child
    /// property on an opaque patch result.
    port_paths: BTreeMap<IntentPortSelector, SemanticOutputPath>,
}

impl ExistingSourceOwner {
    fn direct(declaration: SemanticSymbol) -> Self {
        Self {
            declaration,
            node_path: SemanticOutputPath::default(),
            port_paths: BTreeMap::new(),
        }
    }

    fn direct_node(declaration: SemanticSymbol, node: &IntentNode) -> Self {
        Self {
            declaration,
            node_path: SemanticOutputPath::default(),
            port_paths: node
                .descriptor()
                .outputs
                .into_iter()
                .map(|output| (output.selector, semantic_output_path(&output.path)))
                .collect(),
        }
    }

    fn generic_reference_path(&self, selector: IntentPortSelector) -> SemanticOutputPath {
        self.port_paths
            .get(&selector)
            .cloned()
            .unwrap_or_else(|| self.node_path.clone())
    }
}

/// One compiler-host input produced from an authenticated accepted graph
/// addition. It contains semantic data only: the compiler assigns source-site
/// IDs and spans after insertion and canonical printing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorSourceDeclarationDraft {
    pub node: NodeId,
    pub variable: String,
    pub symbol: SemanticSymbol,
    pub builder_path: Vec<String>,
    pub arguments: ManagedValue,
    pub group: String,
    pub suppressed: bool,
}

/// Stable declaration coordinate used by one source-authoring closure.
///
/// The node and managed symbol are both retained so presentation and lifecycle
/// consumers never infer closure ownership from display names or lexical
/// adjacency.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorSourceDeclarationRef {
    pub node: NodeId,
    pub symbol: SemanticSymbol,
}

impl EditorSourceDeclarationRef {
    fn new(declaration: &EditorBootstrapDeclaration) -> Self {
        Self {
            node: declaration.node,
            symbol: declaration.symbol.clone(),
        }
    }
}

/// Closed source-authoring families which own helper declarations.
///
/// This is deliberately semantic rather than presentation-derived. Additional
/// closure families must be admitted explicitly at this reverse-projection
/// boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorSourceDeclarationClosureKind {
    ProfileOffset,
}

/// One user-facing declaration root and its source-retained helper members.
///
/// Every member remains an ordinary entry in
/// [`EditorDeclarationInsertionPlan::declarations`] and therefore remains
/// explicit managed source/IR authority. This model only records that the
/// helpers participate in the same authored lifecycle as `root`; it never
/// erases, merges, or synthesizes a declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorSourceDeclarationClosure {
    pub kind: EditorSourceDeclarationClosureKind,
    pub root: EditorSourceDeclarationRef,
    pub helpers: Vec<EditorSourceDeclarationRef>,
}

/// Digest-bound reverse-projection result for one completed canvas gesture.
///
/// Applying this plan is not publication. A compiler host must insert the
/// drafts into the exact managed V3 IR, print and execute the candidate, and
/// return the complete source/IR/artifact tuple for Rust validation and cold
/// materialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorDeclarationInsertionPlan {
    pub project: crate::ProjectKey,
    pub source_digest: String,
    pub expansion_digest: String,
    pub declarations: Vec<EditorSourceDeclarationDraft>,
    /// Explicit authored lifecycle closures. Empty for ordinary independent
    /// declarations and for operations which consume reusable source-owned
    /// aggregates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declaration_closures: Vec<EditorSourceDeclarationClosure>,
}

impl EditorDeclarationInsertionPlan {
    /// Returns the declarations which should appear as user-facing roots while
    /// retaining every helper in [`Self::declarations`] for source mutation.
    #[must_use]
    pub fn user_facing_declaration_roots(&self) -> Vec<&EditorSourceDeclarationDraft> {
        let helpers = self
            .declaration_closures
            .iter()
            .flat_map(|closure| closure.helpers.iter().map(|helper| helper.node))
            .collect::<BTreeSet<_>>();
        self.declarations
            .iter()
            .filter(|declaration| !helpers.contains(&declaration.node))
            .collect()
    }
}

/// Why an accepted editor delta cannot become a managed V3 insertion ticket.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum EditorDeclarationInsertionError {
    #[error("the code project is invalid: {0}")]
    InvalidProject(String),
    #[error("the {which} editor has no independently accepted native authority")]
    MissingAcceptedAuthority { which: &'static str },
    #[error("the {which} editor failed independent native validation")]
    InvalidAcceptedAuthority { which: &'static str },
    #[error("the {which} accepted native authority is stale")]
    StaleAcceptedAuthority { which: &'static str },
    #[error("the accepted code expansion has unauthenticated declaration provenance: {0}")]
    InvalidExpansion(String),
    #[error("canvas insertion selection is empty")]
    EmptySelection,
    #[error("canvas insertion repeats declaration node {0}")]
    DuplicateNode(NodeId),
    #[error("canvas insertion repeats managed symbol `{0}`")]
    DuplicateSymbol(String),
    #[error("`{0}` is not a valid managed declaration identifier")]
    InvalidSymbol(String),
    #[error("managed symbol `{0}` already exists")]
    SymbolCollision(String),
    #[error("accepted code declaration node {0} disappeared from the candidate")]
    RemovedAcceptedNode(NodeId),
    #[error("accepted code/editor authority changed outside the canvas addition at node {0}")]
    ExistingNodeChanged(NodeId),
    #[error("candidate contains unselected added declaration node {0}")]
    UnselectedAddition(NodeId),
    #[error("selected canvas declaration node {0} was not added by this gesture")]
    SelectionIsNotAddition(NodeId),
    #[error("canvas declaration `{symbol}` depends on non-source-owned node {dependency}")]
    UnownedDependency { symbol: String, dependency: NodeId },
    #[error("canvas declarations contain a dependency cycle")]
    DependencyCycle,
    #[error("canvas declaration `{symbol}` uses unsupported recipe `{recipe}`")]
    UnsupportedRecipe { symbol: String, recipe: String },
    #[error("canvas declaration `{symbol}` has no accepted native `{output}` point")]
    MissingNativePoint {
        symbol: String,
        output: &'static str,
    },
    #[error("canvas declaration `{symbol}` contains non-finite accepted geometry")]
    NonFiniteGeometry { symbol: String },
    #[error("canvas declaration `{symbol}` has no exact accepted computed feature")]
    MissingComputedFeature { symbol: String },
    #[error("canvas declaration `{symbol}` has invalid accepted computed-feature state: {reason}")]
    InvalidComputedFeatureState { symbol: String, reason: String },
    #[error("canvas declaration `{symbol}` does not own its accepted Fillet span `{input}`")]
    FilletSpanOwnershipMismatch { symbol: String, input: String },
}

/// Authenticates one exact accepted graph addition and reverse-projects it to
/// compiler-host declaration drafts.
///
/// Existing graph nodes, instance leaves and organization names must remain
/// unchanged. Every new node must be named in `declarations`, and every input
/// dependency must resolve either to another named addition or to a lexical
/// declaration authenticated by `accepted_expansion`. The initial M89 slice
/// admits the ordinary Segment family; additional enabled recipes extend the
/// same closed conversion table rather than bypassing this gate.
///
/// # Errors
///
/// Returns a typed refusal before any source, artifact, scene or history
/// authority is changed.
pub fn prepare_editor_declaration_insertions(
    project: &CodeProject,
    accepted_expansion: &ExpandedCodeProject,
    accepted_editor: &ProjectionalEditorSession,
    candidate_editor: &ProjectionalEditorSession,
    declarations: &[EditorBootstrapDeclaration],
) -> Result<EditorDeclarationInsertionPlan, EditorDeclarationInsertionError> {
    project
        .validate()
        .map_err(|error| EditorDeclarationInsertionError::InvalidProject(error.to_string()))?;
    validate_editor_authority(accepted_editor, "accepted")?;
    validate_editor_authority(candidate_editor, "candidate")?;
    validate_expansion_authority(project, accepted_expansion, accepted_editor)?;
    validate_selection(project, declarations)?;

    let accepted_intent = accepted_editor.coordinator().intent();
    let candidate_intent = candidate_editor.coordinator().intent();
    let accepted_nodes = accepted_intent.graph().nodes();
    let candidate_nodes = candidate_intent.graph().nodes();

    for (node, accepted) in accepted_nodes {
        let candidate = candidate_nodes
            .get(node)
            .ok_or(EditorDeclarationInsertionError::RemovedAcceptedNode(*node))?;
        if accepted != candidate
            || accepted_intent.organization().node_names().get(node)
                != candidate_intent.organization().node_names().get(node)
        {
            return Err(EditorDeclarationInsertionError::ExistingNodeChanged(*node));
        }
    }
    for (leaf, value) in accepted_intent.instance().values() {
        if candidate_intent.instance().values().get(leaf) != Some(value) {
            return Err(EditorDeclarationInsertionError::ExistingNodeChanged(
                leaf.node,
            ));
        }
    }
    if let Some(leaf) = candidate_intent.instance().values().keys().find(|leaf| {
        accepted_nodes.contains_key(&leaf.node)
            && !accepted_intent.instance().values().contains_key(leaf)
    }) {
        return Err(EditorDeclarationInsertionError::ExistingNodeChanged(
            leaf.node,
        ));
    }
    validate_existing_organization(accepted_editor, candidate_editor, accepted_nodes.keys())?;

    let selected = declarations
        .iter()
        .map(|declaration| (declaration.node, declaration))
        .collect::<BTreeMap<_, _>>();
    for node in candidate_nodes.keys() {
        if !accepted_nodes.contains_key(node) && !selected.contains_key(node) {
            return Err(EditorDeclarationInsertionError::UnselectedAddition(*node));
        }
    }
    for declaration in declarations {
        if accepted_nodes.contains_key(&declaration.node)
            || !candidate_nodes.contains_key(&declaration.node)
        {
            return Err(EditorDeclarationInsertionError::SelectionIsNotAddition(
                declaration.node,
            ));
        }
    }

    let existing_owners = existing_node_owners(project, accepted_expansion, accepted_editor)?;
    let ordered = dependency_order(candidate_editor, declarations, &existing_owners)?;
    let selected_symbols = selected
        .iter()
        .map(|(node, declaration)| {
            let accepted = candidate_nodes.get(node).ok_or(
                EditorDeclarationInsertionError::SelectionIsNotAddition(*node),
            )?;
            let port_paths = accepted
                .descriptor()
                .outputs
                .into_iter()
                .map(|output| (output.selector, semantic_output_path(&output.path)))
                .collect();
            Ok((
                *node,
                ExistingSourceOwner {
                    declaration: declaration.symbol.clone(),
                    node_path: SemanticOutputPath::default(),
                    port_paths,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, EditorDeclarationInsertionError>>()?;
    let drafts = reverse_project_declarations(
        candidate_editor,
        &ordered,
        &selected_symbols,
        &existing_owners,
    )?;
    let declaration_closures = source_authoring_declaration_closures(candidate_editor, &ordered)?;

    Ok(EditorDeclarationInsertionPlan {
        project: project.project.clone(),
        source_digest: project.managed.source_digest.clone(),
        expansion_digest: accepted_expansion.digest.clone(),
        declarations: drafts,
        declaration_closures,
    })
}

fn reverse_project_declarations(
    candidate: &ProjectionalEditorSession,
    ordered: &[&EditorBootstrapDeclaration],
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
) -> Result<Vec<EditorSourceDeclarationDraft>, EditorDeclarationInsertionError> {
    let graph = candidate.coordinator().intent().graph();
    let mut keyed_geometry_declarations = BTreeSet::new();
    let mut drafts = Vec::with_capacity(ordered.len());
    for declaration in ordered {
        let node = graph.node(declaration.node).ok_or(
            EditorDeclarationInsertionError::SelectionIsNotAddition(declaration.node),
        )?;
        let draft = reverse_project_declaration(
            candidate,
            declaration,
            node,
            selected,
            existing,
            &keyed_geometry_declarations,
        )?;
        if matches!(
            draft.builder_path.as_slice(),
            [namespace, method]
                if namespace == "geometry"
                    && matches!(
                        method.as_str(),
                        "polyline" | "openControlNurbs" | "periodicControlNurbs"
                    )
        ) {
            keyed_geometry_declarations.insert(node.id);
        }
        drafts.push(draft);
    }
    Ok(drafts)
}

/// Finds exact private helper ownership within one accepted source insertion.
///
/// Projectional Profile Offset authoring creates one or more equation-free
/// Profile/OpenChain aggregates in the same patch as its operation. An added
/// aggregate is a helper only when it has exactly one consumer in the complete
/// candidate graph, that consumer is the selected Profile Offset root, and the
/// aggregate family matches the operation input role. Reusable or shared
/// aggregates remain independent user-facing declarations.
fn source_authoring_declaration_closures(
    candidate: &ProjectionalEditorSession,
    ordered: &[&EditorBootstrapDeclaration],
) -> Result<Vec<EditorSourceDeclarationClosure>, EditorDeclarationInsertionError> {
    let graph = candidate.coordinator().intent().graph();
    let selected = ordered
        .iter()
        .map(|declaration| (declaration.node, *declaration))
        .collect::<BTreeMap<_, _>>();
    let declaration_order = ordered
        .iter()
        .enumerate()
        .map(|(index, declaration)| (declaration.node, index))
        .collect::<BTreeMap<_, _>>();
    let mut consumers = BTreeMap::<NodeId, BTreeSet<NodeId>>::new();
    for consumer in graph.nodes().values() {
        for dependency in consumer.dependencies() {
            consumers.entry(dependency).or_default().insert(consumer.id);
        }
    }

    let mut closures = Vec::new();
    for root in ordered {
        let root_node = graph.node(root.node).ok_or(
            EditorDeclarationInsertionError::SelectionIsNotAddition(root.node),
        )?;
        if !matches!(
            root_node.kind,
            IntentNodeKind::Operation {
                operation: geosolve_sketch_intent::OperationKind::ProfileOffset
            }
        ) {
            continue;
        }
        let mut helper_nodes = root_node
            .inputs
            .iter()
            .filter_map(|(slot, source)| {
                let helper = graph.node(source.node)?;
                let matching_aggregate = matches!(
                    (slot.role, &helper.kind),
                    (
                        geosolve_sketch_intent::InputRole::Profile,
                        IntentNodeKind::Aggregate {
                            aggregate: geosolve_sketch_intent::AggregateKind::ClosedProfile
                        }
                    ) | (
                        geosolve_sketch_intent::InputRole::Chain,
                        IntentNodeKind::Aggregate {
                            aggregate: geosolve_sketch_intent::AggregateKind::OpenChain
                        }
                    )
                );
                (matching_aggregate
                    && selected.contains_key(&source.node)
                    && consumers
                        .get(&source.node)
                        .is_some_and(|owners| owners.len() == 1 && owners.contains(&root.node)))
                .then_some(source.node)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        helper_nodes.sort_by_key(|node| declaration_order.get(node).copied());
        if helper_nodes.is_empty() {
            continue;
        }
        let helpers = helper_nodes
            .into_iter()
            .map(|node| {
                selected
                    .get(&node)
                    .copied()
                    .map(EditorSourceDeclarationRef::new)
                    .ok_or(EditorDeclarationInsertionError::SelectionIsNotAddition(
                        node,
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        closures.push(EditorSourceDeclarationClosure {
            kind: EditorSourceDeclarationClosureKind::ProfileOffset,
            root: EditorSourceDeclarationRef::new(root),
            helpers,
        });
    }
    Ok(closures)
}

fn validate_editor_authority(
    editor: &ProjectionalEditorSession,
    which: &'static str,
) -> Result<(), EditorDeclarationInsertionError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorDeclarationInsertionError::MissingAcceptedAuthority { which })?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(EditorDeclarationInsertionError::InvalidAcceptedAuthority { which });
    }
    if accepted.validation.semantic != editor.coordinator().intent().semantic_identity() {
        return Err(EditorDeclarationInsertionError::StaleAcceptedAuthority { which });
    }
    Ok(())
}

fn validate_expansion_authority(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
) -> Result<(), EditorDeclarationInsertionError> {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let digest_bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
        &expansion.operation_plans,
    ))
    .map_err(|error| {
        EditorDeclarationInsertionError::InvalidExpansion(format!(
            "expansion digest input could not be encoded: {error}"
        ))
    })?;
    if expansion.digest != intent_content_digest(&digest_bytes).to_string() {
        return Err(EditorDeclarationInsertionError::InvalidExpansion(
            "expansion digest does not authenticate its semantic payload".into(),
        ));
    }
    let declarations = project
        .managed
        .program
        .declarations
        .iter()
        .map(|declaration| declaration.symbol.clone())
        .collect::<BTreeSet<_>>();
    for (alias, declaration) in &expansion.declaration_provenance {
        if !declarations.contains(declaration) {
            return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
                "alias `{alias}` names absent managed declaration `{}`",
                declaration.0,
            )));
        }
        if editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(alias)
            .is_none()
        {
            return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
                "alias `{alias}` has no accepted graph node"
            )));
        }
    }
    Ok(())
}

fn validate_existing_organization<'a>(
    accepted: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
    accepted_nodes: impl Iterator<Item = &'a NodeId>,
) -> Result<(), EditorDeclarationInsertionError> {
    let accepted_nodes = accepted_nodes.copied().collect::<BTreeSet<_>>();
    let signature = |editor: &ProjectionalEditorSession| {
        let organization = editor.coordinator().intent().organization();
        organization
            .cell_order()
            .iter()
            .filter_map(|cell_id| {
                let cell = organization.cells().get(cell_id)?;
                let declarations = cell
                    .declarations
                    .iter()
                    .filter(|node| accepted_nodes.contains(node))
                    .copied()
                    .collect::<Vec<_>>();
                (!declarations.is_empty()).then_some((cell.name.clone(), declarations))
            })
            .collect::<Vec<_>>()
    };
    if signature(accepted) == signature(candidate) {
        Ok(())
    } else {
        let node = accepted_nodes
            .iter()
            .next()
            .copied()
            .expect("an organization change requires an accepted node");
        Err(EditorDeclarationInsertionError::ExistingNodeChanged(node))
    }
}

fn validate_selection(
    project: &CodeProject,
    declarations: &[EditorBootstrapDeclaration],
) -> Result<(), EditorDeclarationInsertionError> {
    if declarations.is_empty() {
        return Err(EditorDeclarationInsertionError::EmptySelection);
    }
    let mut nodes = BTreeSet::new();
    let mut symbols = project
        .managed
        .program
        .declarations
        .iter()
        .map(|declaration| declaration.symbol.0.clone())
        .collect::<BTreeSet<_>>();
    for declaration in declarations {
        if !nodes.insert(declaration.node) {
            return Err(EditorDeclarationInsertionError::DuplicateNode(
                declaration.node,
            ));
        }
        if !valid_managed_identifier(&declaration.symbol.0) {
            return Err(EditorDeclarationInsertionError::InvalidSymbol(
                declaration.symbol.0.clone(),
            ));
        }
        if !symbols.insert(declaration.symbol.0.clone()) {
            if project
                .managed
                .program
                .declarations
                .iter()
                .any(|existing| existing.symbol == declaration.symbol)
            {
                return Err(EditorDeclarationInsertionError::SymbolCollision(
                    declaration.symbol.0.clone(),
                ));
            }
            return Err(EditorDeclarationInsertionError::DuplicateSymbol(
                declaration.symbol.0.clone(),
            ));
        }
    }
    Ok(())
}

fn existing_node_owners(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
) -> Result<BTreeMap<NodeId, ExistingSourceOwner>, EditorDeclarationInsertionError> {
    let mut owners = BTreeMap::<NodeId, ExistingSourceOwner>::new();
    let mut direct_generated_roots = BTreeMap::<SemanticSymbol, NodeId>::new();
    for (alias, declaration) in &expansion.declaration_provenance {
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(alias)
            .ok_or_else(|| {
                EditorDeclarationInsertionError::InvalidExpansion(format!(
                    "alias `{alias}` has no accepted graph node"
                ))
            })?;
        match owners.get(&node.id) {
            Some(existing) if &existing.declaration != declaration => {
                return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
                    "node {} has competing declaration owners `{}` and `{}`",
                    node.id, existing.declaration.0, declaration.0,
                )));
            }
            _ => {
                let owner = if let Some((keys, closed)) =
                    direct_single_curve_polyline_source(project, declaration)
                {
                    direct_generated_roots.insert(declaration.clone(), node.id);
                    direct_single_curve_polyline_owner(declaration.clone(), node, &keys, closed)?
                } else {
                    ExistingSourceOwner::direct_node(declaration.clone(), node)
                };
                owners.insert(node.id, owner);
            }
        }
    }

    refine_direct_semantic_output_owners(expansion, editor, &direct_generated_roots, &mut owners)?;

    let mut generated_nodes = BTreeSet::new();
    for provenance in expansion.generated_provenance.values() {
        let mut target_nodes = BTreeSet::new();
        collect_target_nodes(editor, &provenance.target, &mut target_nodes)?;
        for node in target_nodes {
            // Direct single-curve Polylines expose keyed vertex/span
            // identities on the declaration's one native root. Those rows
            // are generated-member identity, but the root remains a lexical
            // source owner for later canvas gestures.
            if provenance.artifact_digest.is_none()
                && direct_generated_roots.get(&provenance.declaration) == Some(&node)
            {
                continue;
            }
            generated_nodes.insert(node);
        }
    }
    for child in &expansion.generated_children {
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&child.alias)
            .ok_or_else(|| {
                EditorDeclarationInsertionError::InvalidExpansion(format!(
                    "generated child alias `{}` has no accepted graph node",
                    child.alias
                ))
            })?;
        generated_nodes.insert(node.id);
    }
    for node in generated_nodes {
        owners.remove(&node);
    }

    // Declaration provenance intentionally names every emitted alias, including
    // nodes nested inside a custom patch invocation. Refine those generated
    // aliases to the compiler-authenticated public result path. The artifact's
    // selected result output is the authority; internal template outputs are
    // not made source-addressable merely because they exist in the graph.
    for (address, provenance) in &expansion.generated_provenance {
        let Some(artifact_digest) = &provenance.artifact_digest else {
            continue;
        };
        let artifact = project_artifact(project, artifact_digest)?;
        let Some(path) = generated_public_result_path(&artifact, address)? else {
            continue;
        };
        record_target_owners(
            expansion,
            editor,
            &mut owners,
            &provenance.declaration,
            &path,
            &provenance.target,
        )?;
    }
    Ok(owners)
}

/// Refines direct owners through the exact public result paths authenticated
/// by the executed managed V3 artifact. Native Intent names and SDK-facing
/// result names may deliberately differ (for example `curve` versus
/// `.circle`), so later canvas gestures must use this public mapping.
fn refine_direct_semantic_output_owners(
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
    direct_generated_roots: &BTreeMap<SemanticSymbol, NodeId>,
    owners: &mut BTreeMap<NodeId, ExistingSourceOwner>,
) -> Result<(), EditorDeclarationInsertionError> {
    for output in expansion.semantic_outputs.values() {
        // The single-curve Polyline's keyed surface is more specific than its
        // native aggregate aliases and must retain `segments.byKey.vN`.
        if direct_generated_roots.contains_key(&output.reference.declaration) {
            continue;
        }
        // A patch may publish an equation-free logical root with no native
        // graph node. Its concrete outputs are refined through generated
        // provenance below, so the honest logical root is not stale.
        if matches!(
            &output.target,
            ExpandedSemanticTarget::Declaration { alias, .. }
                if editor.coordinator().intent().graph().node_by_symbol(alias).is_none()
        ) {
            continue;
        }
        record_target_owners(
            expansion,
            editor,
            owners,
            &output.reference.declaration,
            &output.reference.output,
            &output.target,
        )?;
    }
    Ok(())
}

fn direct_single_curve_polyline_source(
    project: &CodeProject,
    symbol: &SemanticSymbol,
) -> Option<(Vec<String>, bool)> {
    let declaration = project
        .managed
        .program
        .declarations
        .iter()
        .find(|declaration| &declaration.symbol == symbol)?;
    if declaration.patch.is_some() || declaration.builder_path != ["geometry", "polyline"] {
        return None;
    }
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        return None;
    };
    let Some(ManagedValue::Bool(closed)) = arguments.get("closed") else {
        return None;
    };
    let Some(ManagedValue::Array(vertices)) = arguments.get("vertices") else {
        return None;
    };
    let keys = vertices
        .iter()
        .map(|vertex| {
            let ManagedValue::Object(vertex) = vertex else {
                return None;
            };
            let Some(ManagedValue::String(key)) = vertex.get("key") else {
                return None;
            };
            Some(key.clone())
        })
        .collect::<Option<Vec<_>>>()?;
    Some((keys, *closed))
}

fn direct_single_curve_polyline_owner(
    declaration: SemanticSymbol,
    node: &IntentNode,
    keys: &[String],
    closed: bool,
) -> Result<ExistingSourceOwner, EditorDeclarationInsertionError> {
    if !matches!(
        node.kind,
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline
        }
    ) || node.child_order.len() != keys.len()
    {
        return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
            "single-curve Polyline `{}` does not own its exact native child set",
            declaration.0
        )));
    }
    let segment_count = if closed {
        keys.len()
    } else {
        keys.len().saturating_sub(1)
    };
    let mut port_paths = BTreeMap::new();
    for (ordinal, key) in keys.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).map_err(|_| {
            EditorDeclarationInsertionError::InvalidExpansion(format!(
                "single-curve Polyline `{}` exceeds the native child bound",
                declaration.0
            ))
        })?;
        port_paths.insert(
            IntentPortSelector::InitialChild {
                ordinal,
                role: IntentPortRole::Corner,
                index: 0,
            },
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("vertices".into()),
                ManagedPathSegment::Field("byKey".into()),
                ManagedPathSegment::Field(key.clone()),
            ]),
        );
        if usize::from(ordinal) < segment_count {
            port_paths.insert(
                IntentPortSelector::InitialChild {
                    ordinal,
                    role: IntentPortRole::Span,
                    index: 0,
                },
                SemanticOutputPath(vec![
                    ManagedPathSegment::Field("segments".into()),
                    ManagedPathSegment::Field("byKey".into()),
                    ManagedPathSegment::Field(key.clone()),
                ]),
            );
        }
    }
    Ok(ExistingSourceOwner {
        declaration,
        node_path: SemanticOutputPath::default(),
        port_paths,
    })
}

fn collect_target_nodes(
    editor: &ProjectionalEditorSession,
    target: &ExpandedSemanticTarget,
    nodes: &mut BTreeSet<NodeId>,
) -> Result<(), EditorDeclarationInsertionError> {
    let graph = editor.coordinator().intent().graph();
    match target {
        ExpandedSemanticTarget::Declaration { alias, .. }
        | ExpandedSemanticTarget::Port {
            port: crate::ExpandedPort { alias, .. },
        } => {
            let node = graph.node_by_symbol(alias).ok_or_else(|| {
                EditorDeclarationInsertionError::InvalidExpansion(format!(
                    "generated alias `{alias}` has no accepted graph node"
                ))
            })?;
            nodes.insert(node.id);
        }
        ExpandedSemanticTarget::FeatureCorner { corner } => {
            for alias in [
                &corner.point.alias,
                &corner.incoming.alias,
                &corner.outgoing.alias,
            ] {
                let node = graph.node_by_symbol(alias).ok_or_else(|| {
                    EditorDeclarationInsertionError::InvalidExpansion(format!(
                        "generated corner alias `{alias}` has no accepted graph node"
                    ))
                })?;
                nodes.insert(node.id);
            }
        }
        ExpandedSemanticTarget::Collection { members } => {
            for target in members.values() {
                collect_target_nodes(editor, target, nodes)?;
            }
        }
        ExpandedSemanticTarget::HostOutput { .. } => {}
    }
    Ok(())
}

fn project_artifact(
    project: &CodeProject,
    digest: &str,
) -> Result<PatchModuleArtifact, EditorDeclarationInsertionError> {
    let value = project.artifacts.get(digest).ok_or_else(|| {
        EditorDeclarationInsertionError::InvalidExpansion(format!(
            "generated provenance names absent artifact `{digest}`"
        ))
    })?;
    let artifact: PatchModuleArtifact = serde_json::from_value(value.clone()).map_err(|error| {
        EditorDeclarationInsertionError::InvalidExpansion(format!(
            "generated provenance artifact `{digest}` cannot be decoded: {error}"
        ))
    })?;
    let validated = artifact.clone().validate().map_err(|error| {
        EditorDeclarationInsertionError::InvalidExpansion(format!(
            "generated provenance artifact `{digest}` is invalid: {error}"
        ))
    })?;
    if validated.digest() != digest {
        return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
            "generated provenance artifact digest `{digest}` is stale"
        )));
    }
    Ok(artifact)
}

fn generated_public_result_path(
    artifact: &PatchModuleArtifact,
    address: &crate::GeneratedMemberAddress,
) -> Result<Option<SemanticOutputPath>, EditorDeclarationInsertionError> {
    let template = artifact
        .templates
        .iter()
        .find(|template| template.path == address.template)
        .ok_or_else(|| {
            EditorDeclarationInsertionError::InvalidExpansion(format!(
                "generated address `{}` names an absent artifact template",
                address.display_path()
            ))
        })?;
    let mut path = if address.member_key.as_slice() == ["self"] {
        let Some(result_path) = &template.result_path else {
            return Ok(None);
        };
        SemanticOutputPath(
            result_path
                .iter()
                .cloned()
                .map(ManagedPathSegment::Field)
                .collect(),
        )
    } else {
        let matching_collections = artifact
            .collections
            .iter()
            .filter_map(|rule| match rule {
                CollectionRule::Each {
                    path, templates, ..
                }
                | CollectionRule::MapRecord {
                    path, templates, ..
                } if templates.contains(&template.path) => Some(path),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [collection_path] = matching_collections.as_slice() else {
            return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
                "generated address `{}` has no unique public collection",
                address.display_path()
            )));
        };
        SemanticOutputPath(
            collection_path
                .iter()
                .cloned()
                .map(ManagedPathSegment::Field)
                .chain(
                    address
                        .member_key
                        .iter()
                        .cloned()
                        .map(|member| ManagedPathSegment::Member { member }),
                )
                .collect(),
        )
    };

    if let Some(selected) = &template.result_output {
        return Ok((address.output == generated_output_address(selected)).then_some(path));
    }
    let Some(output) = template
        .outputs
        .iter()
        .find(|output| address.output == generated_output_address(&output.path))
    else {
        return Ok(None);
    };
    path.0.extend(output.path.0.iter().cloned());
    Ok(Some(path))
}

fn generated_output_address(path: &SemanticOutputPath) -> Vec<String> {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => format!("field:{field}"),
            ManagedPathSegment::Index(index) => format!("index:{index}"),
            ManagedPathSegment::Member { member } => format!("member:{member}"),
        })
        .collect()
}

fn record_target_owners(
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
    owners: &mut BTreeMap<NodeId, ExistingSourceOwner>,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    target: &ExpandedSemanticTarget,
) -> Result<(), EditorDeclarationInsertionError> {
    match target {
        ExpandedSemanticTarget::Declaration { alias, .. } => {
            record_alias_owner(editor, owners, alias, declaration, path, None)
        }
        ExpandedSemanticTarget::Port { port } => record_alias_owner(
            editor,
            owners,
            &port.alias,
            declaration,
            path,
            Some(port.selector),
        ),
        ExpandedSemanticTarget::FeatureCorner { corner } => {
            for port in [&corner.point, &corner.incoming, &corner.outgoing] {
                record_alias_owner(
                    editor,
                    owners,
                    &port.alias,
                    declaration,
                    path,
                    Some(port.selector),
                )?;
            }
            Ok(())
        }
        ExpandedSemanticTarget::Collection { members } => {
            for (member, target) in members {
                let mut member_path = path.clone();
                member_path.0.push(ManagedPathSegment::Member {
                    member: member.clone(),
                });
                record_target_owners(expansion, editor, owners, declaration, &member_path, target)?;
            }
            Ok(())
        }
        ExpandedSemanticTarget::HostOutput {
            address, identity, ..
        } => {
            let mut matched = false;
            for child in expansion.generated_children.iter().filter(|child| {
                child.address.owner.generated_identity() == Some(*identity)
                    && matches!(
                        &child.address.owner.address,
                        crate::CodeOwnerAddress::GeneratedMember {
                            address: child_address
                        } if child_address == address
                    )
            }) {
                matched = true;
                let mut child_path = path.clone();
                child_path.0.extend(child.address.child.0.iter().cloned());
                record_alias_owner(editor, owners, &child.alias, declaration, &child_path, None)?;
            }
            let active_request = expansion.host_requests.iter().any(|request| match request {
                crate::CodeHostRequest::FilletAtCorner(request) => {
                    request.output == *address && request.identity == *identity
                }
                crate::CodeHostRequest::RoundedRectangleProfile {
                    output,
                    identity: request_identity,
                    ..
                } => output == address && *request_identity == *identity,
            });
            if matched || !active_request {
                Ok(())
            } else {
                Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
                    "generated host output `{}` has no accepted child alias",
                    address.display_path()
                )))
            }
        }
    }
}

fn record_alias_owner(
    editor: &ProjectionalEditorSession,
    owners: &mut BTreeMap<NodeId, ExistingSourceOwner>,
    alias: &geosolve_sketch_intent::IntentKey,
    declaration: &SemanticSymbol,
    path: &SemanticOutputPath,
    selector: Option<IntentPortSelector>,
) -> Result<(), EditorDeclarationInsertionError> {
    let node = editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(alias)
        .ok_or_else(|| {
            EditorDeclarationInsertionError::InvalidExpansion(format!(
                "generated alias `{alias}` has no accepted graph node"
            ))
        })?;
    let owner = owners
        .entry(node.id)
        .or_insert_with(|| ExistingSourceOwner::direct(declaration.clone()));
    if owner.declaration != *declaration {
        return Err(EditorDeclarationInsertionError::InvalidExpansion(format!(
            "node {} has competing declaration owners `{}` and `{}`",
            node.id, owner.declaration.0, declaration.0
        )));
    }
    owner.node_path = preferred_path(&owner.node_path, path);
    if let Some(selector) = selector {
        owner
            .port_paths
            .entry(selector)
            .and_modify(|existing| *existing = preferred_path(existing, path))
            .or_insert_with(|| path.clone());
    }
    Ok(())
}

fn preferred_path(
    current: &SemanticOutputPath,
    candidate: &SemanticOutputPath,
) -> SemanticOutputPath {
    if current.0.is_empty()
        || candidate.0.len() < current.0.len()
        || (candidate.0.len() == current.0.len() && candidate.0 < current.0)
    {
        candidate.clone()
    } else {
        current.clone()
    }
}

fn dependency_order<'a>(
    candidate: &ProjectionalEditorSession,
    declarations: &'a [EditorBootstrapDeclaration],
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
) -> Result<Vec<&'a EditorBootstrapDeclaration>, EditorDeclarationInsertionError> {
    let selected = declarations
        .iter()
        .map(|declaration| (declaration.node, declaration))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(declarations.len());
    while ordered.len() < declarations.len() {
        let mut progress = false;
        for declaration in declarations {
            if emitted.contains(&declaration.node) {
                continue;
            }
            let node = candidate
                .coordinator()
                .intent()
                .graph()
                .node(declaration.node)
                .ok_or(EditorDeclarationInsertionError::SelectionIsNotAddition(
                    declaration.node,
                ))?;
            for dependency in node.dependencies() {
                if !selected.contains_key(&dependency) && !existing.contains_key(&dependency) {
                    return Err(EditorDeclarationInsertionError::UnownedDependency {
                        symbol: declaration.symbol.0.clone(),
                        dependency,
                    });
                }
            }
            if node
                .dependencies()
                .iter()
                .all(|dependency| existing.contains_key(dependency) || emitted.contains(dependency))
            {
                emitted.insert(declaration.node);
                ordered.push(declaration);
                progress = true;
            }
        }
        if !progress {
            return Err(EditorDeclarationInsertionError::DependencyCycle);
        }
    }
    Ok(ordered)
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed family dispatch keeps every reverse-projected authoring surface explicit"
)]
fn reverse_project_declaration(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<EditorSourceDeclarationDraft, EditorDeclarationInsertionError> {
    let (builder_path, arguments) = match node.kind {
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        } => (
            vec!["computed".into(), "filletSet".into()],
            computed_fillet_set_arguments(
                editor,
                declaration,
                node,
                selected,
                existing,
                keyed_geometry_declarations,
            )?,
        ),
        _ => clean_named_declaration(
            editor,
            declaration,
            node,
            selected,
            existing,
            keyed_geometry_declarations,
        )?,
    };
    validate_clean_source_arguments(declaration, &arguments)?;
    Ok(EditorSourceDeclarationDraft {
        node: declaration.node,
        variable: declaration.symbol.0.clone(),
        symbol: declaration.symbol.clone(),
        builder_path,
        arguments,
        group: CANVAS_ADDITIONS_GROUP.into(),
        suppressed: node.suppressed,
    })
}

/// Keeps transport vocabulary out of the clean source surface even when a
/// future native descriptor adds a nested field with an accidentally
/// colliding name. Reverse projection must refuse such drift rather than
/// silently reintroducing generic recipe payloads.
fn validate_clean_source_arguments(
    declaration: &EditorBootstrapDeclaration,
    value: &ManagedValue,
) -> Result<(), EditorDeclarationInsertionError> {
    match value {
        ManagedValue::Array(values) => {
            for value in values {
                validate_clean_source_arguments(declaration, value)?;
            }
        }
        ManagedValue::Object(fields) => {
            for (name, value) in fields {
                if matches!(
                    name.as_str(),
                    "recipe"
                        | "inputs"
                        | "fields"
                        | "values"
                        | "results"
                        | "operationOutputs"
                        | "outputs"
                        | "editLens"
                ) {
                    return unsupported_clean_shape(
                        declaration,
                        &format!("transport property `{name}` reached named source projection"),
                    );
                }
                validate_clean_source_arguments(declaration, value)?;
            }
        }
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_)
        | ManagedValue::Reference { .. } => {}
    }
    Ok(())
}

/// Projects one native Intent declaration through the clean, named authoring
/// vocabulary. The central catalog owns the method identity and named input
/// bindings; concrete accepted state supplies only the authored values. No
/// transport schema, selector, result manifest, or generic escape hatch is
/// serialized into `sketch.ts`.
#[allow(
    clippy::too_many_arguments,
    reason = "one authenticated reverse-projection boundary needs both selected and accepted lexical owners"
)]
fn clean_named_declaration(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<(Vec<String>, ManagedValue), EditorDeclarationInsertionError> {
    let declaration_kind = authoring_declaration_kind(&node.kind).ok_or_else(|| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("{:?}", node.kind),
        }
    })?;
    let family = CODE_AUTHORING_FAMILIES
        .iter()
        .find(|family| family.declaration == declaration_kind)
        .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("{:?}", node.kind),
        })?;
    if family.availability != CodeAuthoringAvailability::Public {
        return Err(EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!(
                "{}.{} requires host snapshot authority",
                family.namespace, family.method
            ),
        });
    }
    let dynamic_children = u16::try_from(node.child_order.len()).map_err(|_| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "dynamic child count exceeds the clean authoring bound".into(),
        }
    })?;
    let descriptor =
        resolve_code_authoring_declaration(family.namespace, family.method, dynamic_children)
            .map_err(|error| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: error.to_string(),
            })?;

    let arguments = match node.kind {
        IntentNodeKind::Geometry { recipe } => clean_geometry_arguments(
            editor,
            declaration,
            node,
            recipe,
            &descriptor,
            selected,
            existing,
            keyed_geometry_declarations,
        )?,
        IntentNodeKind::Operation {
            operation: OperationKind::AssociativeFillet,
        } => clean_associative_fillet_arguments(
            editor,
            declaration,
            node,
            selected,
            existing,
            keyed_geometry_declarations,
        )?,
        _ => clean_static_arguments(
            editor,
            declaration,
            node,
            &descriptor,
            selected,
            existing,
            keyed_geometry_declarations,
        )?,
    };
    Ok((
        vec![family.namespace.into(), family.method.into()],
        ManagedValue::Object(arguments),
    ))
}

const fn authoring_declaration_kind(kind: &IntentNodeKind) -> Option<CodeAuthoringDeclarationKind> {
    match kind {
        IntentNodeKind::Geometry { recipe } => {
            Some(CodeAuthoringDeclarationKind::Geometry(*recipe))
        }
        IntentNodeKind::Constraint { constraint } => {
            Some(CodeAuthoringDeclarationKind::Constraint(*constraint))
        }
        IntentNodeKind::Dimension { dimension } => {
            Some(CodeAuthoringDeclarationKind::Dimension(*dimension))
        }
        IntentNodeKind::Operation { operation } => {
            Some(CodeAuthoringDeclarationKind::Operation(*operation))
        }
        IntentNodeKind::Aggregate { aggregate } => {
            Some(CodeAuthoringDeclarationKind::Aggregate(*aggregate))
        }
        IntentNodeKind::ComputedFeature { feature } => {
            Some(CodeAuthoringDeclarationKind::ComputedFeature(*feature))
        }
        IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the geometry projector authenticates typed point inputs and lexical owners together"
)]
fn clean_geometry_arguments(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    descriptor: &crate::CodeAuthoringDeclarationDescriptor,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<BTreeMap<String, ManagedValue>, EditorDeclarationInsertionError> {
    let mut arguments = BTreeMap::new();
    match descriptor.dynamic_children.kind {
        CodeAuthoringDynamicChildren::PolylineVertices => {
            arguments.insert(
                "vertices".into(),
                ManagedValue::Array(clean_polyline_vertices(
                    editor,
                    declaration,
                    node,
                    selected,
                    existing,
                    keyed_geometry_declarations,
                )?),
            );
        }
        CodeAuthoringDynamicChildren::SplineControls => {
            let (controls, gauge) = clean_nurbs_controls(
                editor,
                declaration,
                node,
                selected,
                existing,
                keyed_geometry_declarations,
            )?;
            arguments.insert("controls".into(), ManagedValue::Array(controls));
            arguments.insert("gauge".into(), ManagedValue::String(gauge));
        }
        CodeAuthoringDynamicChildren::None => {
            for input in &descriptor.inputs {
                let CodeAuthoringInputBinding::Slot { slot } = input.binding else {
                    return unsupported_clean_shape(
                        declaration,
                        "fixed-cardinality geometry input is not one named slot",
                    );
                };
                let value = if slot.role == InputRole::Point {
                    clean_geometry_point_argument(
                        editor,
                        declaration,
                        node,
                        recipe,
                        slot.index,
                        selected,
                        existing,
                        keyed_geometry_declarations,
                    )?
                } else {
                    lexical_input_reference(
                        editor,
                        declaration,
                        node,
                        slot,
                        selected,
                        existing,
                        keyed_geometry_declarations,
                    )?
                    .ok_or(
                        EditorDeclarationInsertionError::UnsupportedRecipe {
                            symbol: declaration.symbol.0.clone(),
                            recipe: format!("required input `{}` is absent", input.name),
                        },
                    )?
                };
                if recipe == GeometryRecipeKind::TangentArc && input.name == "source" {
                    let contact = accepted_contact_state(editor, declaration, node, 0)?;
                    arguments.insert(
                        input.name.clone(),
                        ManagedValue::Object(BTreeMap::from([
                            ("contact".into(), contact),
                            ("span".into(), value),
                        ])),
                    );
                } else {
                    arguments.insert(input.name.clone(), value);
                }
            }
        }
        CodeAuthoringDynamicChildren::FilletCorners
        | CodeAuthoringDynamicChildren::PatternInstances => {
            return unsupported_clean_shape(
                declaration,
                "geometry method has a non-geometry dynamic-child policy",
            );
        }
    }

    insert_clean_definition_fields(declaration, node, &mut arguments)?;
    if matches!(
        recipe,
        GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs
    ) {
        arguments.remove("gaugeIndex");
    }
    insert_geometry_independent_values(editor, declaration, node, recipe, &mut arguments)?;
    insert_display_label(editor, declaration, node, &mut arguments);
    Ok(arguments)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the common named projector authenticates each exact lexical dependency owner"
)]
fn clean_static_arguments(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    descriptor: &crate::CodeAuthoringDeclarationDescriptor,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<BTreeMap<String, ManagedValue>, EditorDeclarationInsertionError> {
    let mut arguments = BTreeMap::new();
    for input in &descriptor.inputs {
        let value = match &input.binding {
            CodeAuthoringInputBinding::Slot { slot } => lexical_input_reference(
                editor,
                declaration,
                node,
                *slot,
                selected,
                existing,
                keyed_geometry_declarations,
            )?,
            CodeAuthoringInputBinding::Role { role } => {
                Some(ManagedValue::Array(lexical_role_references(
                    editor,
                    declaration,
                    node,
                    &[*role],
                    selected,
                    existing,
                    keyed_geometry_declarations,
                )?))
            }
            CodeAuthoringInputBinding::Roles { roles } => {
                let values = lexical_role_references(
                    editor,
                    declaration,
                    node,
                    roles,
                    selected,
                    existing,
                    keyed_geometry_declarations,
                )?;
                if matches!(input.kind, crate::CodeAuthoringArgumentKind::Collection(_)) {
                    Some(ManagedValue::Array(values))
                } else {
                    match values.as_slice() {
                        [] => None,
                        [value] => Some(value.clone()),
                        _ => {
                            return unsupported_clean_shape(
                                declaration,
                                "one named reference-choice input resolved more than once",
                            );
                        }
                    }
                }
            }
            CodeAuthoringInputBinding::DynamicChildren { .. } => {
                return unsupported_clean_shape(
                    declaration,
                    "static authoring method unexpectedly owns dynamic children",
                );
            }
        };
        match value {
            Some(value) => {
                arguments.insert(input.name.clone(), value);
            }
            None if input.minimum == 0 => {}
            None => {
                return unsupported_clean_shape(
                    declaration,
                    &format!("required input `{}` is absent", input.name),
                );
            }
        }
    }
    insert_clean_definition_fields(declaration, node, &mut arguments)?;
    insert_dimension_value(editor, declaration, node, &mut arguments)?;
    insert_constraint_contacts(editor, declaration, node, &mut arguments)?;
    insert_display_label(editor, declaration, node, &mut arguments);
    Ok(arguments)
}

fn unsupported_clean_shape<T>(
    declaration: &EditorBootstrapDeclaration,
    reason: &str,
) -> Result<T, EditorDeclarationInsertionError> {
    Err(EditorDeclarationInsertionError::UnsupportedRecipe {
        symbol: declaration.symbol.0.clone(),
        recipe: reason.into(),
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "one helper authenticates the graph slot, source port, and exact lexical owner together"
)]
fn lexical_input_reference(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    slot: InputSlot,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<Option<ManagedValue>, EditorDeclarationInsertionError> {
    let Some(source) = node.inputs.get(&slot).copied() else {
        return Ok(None);
    };
    let owner = selected
        .get(&source.node)
        .or_else(|| existing.get(&source.node))
        .ok_or(EditorDeclarationInsertionError::UnownedDependency {
            symbol: declaration.symbol.0.clone(),
            dependency: source.node,
        })?;
    let source_node = editor
        .coordinator()
        .intent()
        .graph()
        .node(source.node)
        .ok_or(EditorDeclarationInsertionError::UnownedDependency {
            symbol: declaration.symbol.0.clone(),
            dependency: source.node,
        })?;
    let source_port = source_node.port(source.port).ok_or_else(|| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("dependency {} has no stable source port", source.node),
        }
    })?;
    if source_port.kind != source.kind {
        return unsupported_clean_shape(declaration, "dependency source-port kind is stale");
    }
    let path = if keyed_geometry_declarations.contains(&source.node) {
        clean_keyed_geometry_path(source_node, source_port.selector)
            .unwrap_or_else(|| owner.generic_reference_path(source_port.selector))
    } else {
        owner.generic_reference_path(source_port.selector)
    };
    Ok(Some(ManagedValue::Reference {
        declaration: owner.declaration.clone(),
        path,
    }))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one role collection retains exact graph order and lexical owner authentication"
)]
fn lexical_role_references(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    roles: &[InputRole],
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<Vec<ManagedValue>, EditorDeclarationInsertionError> {
    node.inputs
        .keys()
        .filter(|slot| roles.contains(&slot.role))
        .map(|slot| {
            lexical_input_reference(
                editor,
                declaration,
                node,
                *slot,
                selected,
                existing,
                keyed_geometry_declarations,
            )?
            .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: format!("input `{slot}` disappeared during projection"),
            })
        })
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    reason = "a point may be a lexical alias or one accepted recipe-owned geometric sample"
)]
fn clean_geometry_point_argument(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    input_index: u16,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let slot = InputSlot::new(InputRole::Point, input_index);
    if let Some(reference) = lexical_input_reference(
        editor,
        declaration,
        node,
        slot,
        selected,
        existing,
        keyed_geometry_declarations,
    )? {
        return Ok(reference);
    }
    let selector = geometry_point_input_selector(recipe, input_index).ok_or_else(|| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("geometry point input {input_index} has no semantic output"),
        }
    })?;
    let point = accepted_point_binding(editor, node, selector)
        .or_else(|| derived_geometry_input_point(editor, node, recipe, input_index))
        .ok_or_else(|| EditorDeclarationInsertionError::MissingNativePoint {
            symbol: declaration.symbol.0.clone(),
            output: "authored point",
        })?;
    finite_point_value(declaration, point)
}

const fn geometry_point_input_selector(
    recipe: GeometryRecipeKind,
    index: u16,
) -> Option<IntentPortSelector> {
    #![allow(
        clippy::match_same_arms,
        reason = "the exhaustive geometry recipe/input table is easier to audit by native recipe than by output role"
    )]
    use GeometryRecipeKind as G;
    use IntentPortRole as R;
    let (role, role_index) = match (recipe, index) {
        (G::SketchPoint, 0) => (R::Primary, 0),
        (G::Segment | G::TwoPointDiameterCircle | G::TangentArc, 0) => (R::Start, 0),
        (G::Segment | G::TwoPointDiameterCircle | G::TangentArc, 1) => (R::End, 0),
        (G::MidpointLine, 0) => (R::Midpoint, 0),
        (G::MidpointLine, 1) => (R::End, 0),
        (G::TwoPointAlignedRectangle, 0) => (R::Corner, 0),
        (G::TwoPointAlignedRectangle, 1) => (R::Corner, 2),
        (G::ThreePointCornerRectangle, 0..=2) => (R::Corner, index),
        (G::CenterRectangle | G::ThreePointCenterRectangle, 0) => (R::Center, 0),
        (G::CenterRectangle | G::ThreePointCenterRectangle, 1) => (R::Corner, 0),
        (G::CenterRadiusCircle, 0) => (R::Center, 0),
        (G::CenterRadiusCircle, 1) => (R::Control, 0),
        (G::ThreePointCircle | G::ThreePointArc, 0..=2) => (R::Control, index),
        (G::CenterArc, 0) => (R::Center, 0),
        (G::CenterArc, 1) => (R::Start, 0),
        (G::CenterArc, 2) => (R::End, 0),
        (G::CenterAxesEllipse | G::CenterAxesEllipticalArc, 0) => (R::Center, 0),
        (G::CenterAxesEllipse | G::CenterAxesEllipticalArc, 1) => (R::MajorAxisPoint, 0),
        (G::CenterAxesEllipse | G::CenterAxesEllipticalArc, 2) => (R::MinorAxisPoint, 0),
        (G::CenterAxesEllipticalArc, 3) => (R::Start, 0),
        (G::CenterAxesEllipticalArc, 4) => (R::End, 0),
        (G::AxisEndpointsEllipse | G::AxisEndpointsEllipticalArc, 0) => (R::MajorAxisPoint, 0),
        (G::AxisEndpointsEllipse, 1) => (R::End, 0),
        (G::AxisEndpointsEllipticalArc, 1) => (R::Control, 0),
        (G::AxisEndpointsEllipse | G::AxisEndpointsEllipticalArc, 2) => (R::MinorAxisPoint, 0),
        (G::AxisEndpointsEllipticalArc, 3) => (R::Start, 0),
        (G::AxisEndpointsEllipticalArc, 4) => (R::End, 0),
        (G::QuadraticBezier, 0) => (R::Start, 0),
        (G::QuadraticBezier, 1) => (R::Control, 0),
        (G::QuadraticBezier, 2) => (R::End, 0),
        (G::CubicBezier, 0) => (R::Start, 0),
        (G::CubicBezier, 1) => (R::Control, 0),
        (G::CubicBezier, 2) => (R::Control, 1),
        (G::CubicBezier, 3) => (R::End, 0),
        (G::RationalQuadraticConic, 0) => (R::Start, 0),
        (G::RationalQuadraticConic, 1) => (R::End, 0),
        (G::Parabola, 0) => (R::Center, 0),
        (G::Parabola, 1) => (R::Control, 0),
        (G::Hyperbola, 0) => (R::Center, 0),
        (G::Hyperbola, 1) => (R::Control, 0),
        _ => return None,
    };
    Some(IntentPortSelector::Node {
        role,
        index: role_index,
    })
}

fn accepted_point_binding(
    editor: &ProjectionalEditorSession,
    node: &IntentNode,
    selector: IntentPortSelector,
) -> Option<[f64; 2]> {
    let accepted = editor.coordinator().accepted_materialization()?;
    let port = node.port_by_selector(selector)?;
    let IntentNativeBinding::Point(point) = accepted.ownership.port(port.as_ref(node.id))? else {
        return None;
    };
    accepted
        .session
        .design_document()
        .point(point)
        .map(|point| point.position)
}

fn finite_point_value(
    declaration: &EditorBootstrapDeclaration,
    point: [f64; 2],
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    if point.iter().any(|coordinate| !coordinate.is_finite()) {
        return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    Ok(ManagedValue::Array(
        point.into_iter().map(ManagedValue::Number).collect(),
    ))
}

fn accepted_curve_for_node(
    editor: &ProjectionalEditorSession,
    node: &IntentNode,
) -> Option<geosolve_sketch::CurveId> {
    let port = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Curve,
        index: 0,
    })?;
    let IntentNativeBinding::Curve(curve) = editor
        .coordinator()
        .accepted_materialization()?
        .ownership
        .port(port.as_ref(node.id))?
    else {
        return None;
    };
    Some(curve)
}

fn accepted_curve_control_position(
    editor: &ProjectionalEditorSession,
    node: &IntentNode,
    kind: DocumentCurveControlKind,
) -> Option<[f64; 2]> {
    let accepted = editor.coordinator().accepted_materialization()?;
    let curve = accepted_curve_for_node(editor, node)?;
    accepted
        .session
        .design_document()
        .curve_controls(curve)
        .ok()?
        .into_iter()
        .find(|control| control.id.kind == kind)
        .map(|control| control.position)
}

fn derived_geometry_input_point(
    editor: &ProjectionalEditorSession,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    input_index: u16,
) -> Option<[f64; 2]> {
    use GeometryRecipeKind as G;
    if matches!(recipe, G::TwoPointDiameterCircle | G::ThreePointCircle) {
        let center =
            accepted_curve_control_position(editor, node, DocumentCurveControlKind::Center)?;
        let radius =
            accepted_curve_control_position(editor, node, DocumentCurveControlKind::Radius)?;
        let vector = [radius[0] - center[0], radius[1] - center[1]];
        return match (recipe, input_index) {
            (G::TwoPointDiameterCircle, 0) => Some([center[0] - vector[0], center[1] - vector[1]]),
            (G::TwoPointDiameterCircle, 1) | (G::ThreePointCircle, 0) => Some(radius),
            (G::ThreePointCircle, 1) => Some([
                center[0] - vector[0] * 0.5 - vector[1] * 3.0_f64.sqrt() * 0.5,
                center[1] - vector[1] * 0.5 + vector[0] * 3.0_f64.sqrt() * 0.5,
            ]),
            (G::ThreePointCircle, 2) => Some([
                center[0] - vector[0] * 0.5 + vector[1] * 3.0_f64.sqrt() * 0.5,
                center[1] - vector[1] * 0.5 - vector[0] * 3.0_f64.sqrt() * 0.5,
            ]),
            _ => None,
        };
    }
    if recipe == G::ThreePointArc {
        let curve = accepted_curve_for_node(editor, node)?;
        let parameter = match input_index {
            0 => 0.0,
            1 => 0.5,
            2 => 1.0,
            _ => return None,
        };
        let point = editor
            .coordinator()
            .accepted_materialization()?
            .session
            .design_document()
            .evaluate_curve_jet(geosolve_sketch::CurveSpan::line(curve), parameter)
            .ok()?
            .position;
        return Some([point.x, point.y]);
    }
    if matches!(
        recipe,
        G::AxisEndpointsEllipse | G::AxisEndpointsEllipticalArc
    ) && input_index == 1
    {
        let center =
            accepted_curve_control_position(editor, node, DocumentCurveControlKind::Center)?;
        let first = accepted_curve_control_position(
            editor,
            node,
            DocumentCurveControlKind::MajorAxisPoint,
        )?;
        return Some([2.0 * center[0] - first[0], 2.0 * center[1] - first[1]]);
    }
    let control = match (recipe, input_index) {
        (G::CenterRadiusCircle, 1) => DocumentCurveControlKind::Radius,
        (G::CenterArc, 1) | (G::TangentArc, 0) => DocumentCurveControlKind::TrimStart,
        (G::CenterArc, 2) | (G::TangentArc, 1) => DocumentCurveControlKind::TrimEnd,
        (
            G::CenterAxesEllipse
            | G::AxisEndpointsEllipse
            | G::CenterAxesEllipticalArc
            | G::AxisEndpointsEllipticalArc,
            2,
        ) => DocumentCurveControlKind::MinorAxis,
        (G::CenterAxesEllipticalArc | G::AxisEndpointsEllipticalArc, 3) => {
            DocumentCurveControlKind::TrimStart
        }
        (G::CenterAxesEllipticalArc | G::AxisEndpointsEllipticalArc, 4) => {
            DocumentCurveControlKind::TrimEnd
        }
        _ => return None,
    };
    accepted_curve_control_position(editor, node, control)
}

#[allow(
    clippy::too_many_arguments,
    reason = "each keyed child may be either one exact lexical point alias or one accepted owned point"
)]
fn clean_polyline_vertices(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<Vec<ManagedValue>, EditorDeclarationInsertionError> {
    let mut values = Vec::with_capacity(node.child_order.len());
    for ordinal in 0..node.child_order.len() {
        let ordinal = u16::try_from(ordinal).map_err(|_| {
            EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "Polyline child count exceeds source limits".into(),
            }
        })?;
        let position = if let Some(reference) = lexical_input_reference(
            editor,
            declaration,
            node,
            InputSlot::new(InputRole::Point, ordinal),
            selected,
            existing,
            keyed_geometry_declarations,
        )? {
            reference
        } else {
            let selector = IntentPortSelector::InitialChild {
                ordinal,
                role: IntentPortRole::Corner,
                index: 0,
            };
            finite_point_value(
                declaration,
                accepted_point_binding(editor, node, selector).ok_or_else(|| {
                    EditorDeclarationInsertionError::MissingNativePoint {
                        symbol: declaration.symbol.0.clone(),
                        output: "Polyline vertex",
                    }
                })?,
            )?
        };
        values.push(ManagedValue::Object(BTreeMap::from([
            (
                "key".into(),
                ManagedValue::String(polyline_vertex_key(ordinal)),
            ),
            ("position".into(), position),
        ])));
    }
    Ok(values)
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "each keyed NURBS control authenticates its point alias and accepted scalar together"
)]
fn clean_nurbs_controls(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<(Vec<ManagedValue>, String), EditorDeclarationInsertionError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorDeclarationInsertionError::MissingAcceptedAuthority { which: "candidate" })?;
    let mut controls = Vec::with_capacity(node.child_order.len());
    for ordinal in 0..node.child_order.len() {
        let ordinal = u16::try_from(ordinal).map_err(|_| {
            EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "NURBS control count exceeds source limits".into(),
            }
        })?;
        let position = if let Some(reference) = lexical_input_reference(
            editor,
            declaration,
            node,
            InputSlot::new(InputRole::Point, ordinal),
            selected,
            existing,
            keyed_geometry_declarations,
        )? {
            reference
        } else {
            finite_point_value(
                declaration,
                accepted_point_binding(
                    editor,
                    node,
                    IntentPortSelector::InitialChild {
                        ordinal,
                        role: IntentPortRole::Control,
                        index: 0,
                    },
                )
                .ok_or_else(|| {
                    EditorDeclarationInsertionError::MissingNativePoint {
                        symbol: declaration.symbol.0.clone(),
                        output: "NURBS control",
                    }
                })?,
            )?
        };
        let weight_port = node
            .port_by_selector(IntentPortSelector::InitialChild {
                ordinal,
                role: IntentPortRole::Target,
                index: 0,
            })
            .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "NURBS control has no weight output".into(),
            })?;
        let IntentNativeBinding::Scalar(weight) = accepted
            .ownership
            .port(weight_port.as_ref(node.id))
            .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "NURBS control weight has no accepted scalar".into(),
            })?
        else {
            return unsupported_clean_shape(declaration, "NURBS weight output is not scalar");
        };
        let weight = accepted
            .session
            .design_document()
            .scalar(weight)
            .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "NURBS accepted weight scalar disappeared".into(),
            })?
            .value;
        if !weight.is_finite() {
            return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
                symbol: declaration.symbol.0.clone(),
            });
        }
        controls.push(ManagedValue::Object(BTreeMap::from([
            (
                "key".into(),
                ManagedValue::String(polyline_vertex_key(ordinal)),
            ),
            ("position".into(), position),
            ("weight".into(), ManagedValue::Number(weight)),
        ])));
    }
    let gauge_index = match intent_field(node, "gauge_index") {
        Some(IntentLiteral::Natural(index)) => usize::try_from(*index).ok(),
        None => Some(0),
        Some(_) => None,
    }
    .filter(|index| *index < node.child_order.len())
    .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
        symbol: declaration.symbol.0.clone(),
        recipe: "NURBS gauge index does not name one control".into(),
    })?;
    let gauge_index = u16::try_from(gauge_index).map_err(|_| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "NURBS gauge index exceeds source limits".into(),
        }
    })?;
    Ok((controls, polyline_vertex_key(gauge_index)))
}

fn insert_clean_definition_fields(
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    arguments: &mut BTreeMap<String, ManagedValue>,
) -> Result<(), EditorDeclarationInsertionError> {
    let paths = node
        .descriptor()
        .fields
        .into_iter()
        .map(|field| (field.schema.field, field.path))
        .collect::<BTreeMap<_, _>>();
    for (field, literal) in &node.fields {
        let name = field.0.as_str();
        if field_is_projected_from_accepted_state(&node.kind, name) {
            continue;
        }
        let path =
            paths
                .get(field)
                .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                    symbol: declaration.symbol.0.clone(),
                    recipe: format!("definition field `{name}` has no named authoring path"),
                })?;
        insert_named_path(
            declaration,
            arguments,
            path,
            clean_intent_literal(declaration, literal)?,
        )?;
    }
    Ok(())
}

fn field_is_projected_from_accepted_state(kind: &IntentNodeKind, name: &str) -> bool {
    match kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TangentArc,
        } => name.starts_with("source_") || name == "orientation",
        IntentNodeKind::Constraint { .. } => {
            name.starts_with("contact_")
                || name.starts_with("first_contact_")
                || name.starts_with("second_contact_")
        }
        IntentNodeKind::Operation {
            operation: OperationKind::AssociativeFillet,
        } => name.starts_with("first_") || name.starts_with("second_"),
        IntentNodeKind::ComputedFeature {
            feature: ComputedFeatureKind::FilletSet,
        } => true,
        IntentNodeKind::Geometry { .. }
        | IntentNodeKind::Dimension { .. }
        | IntentNodeKind::Operation { .. }
        | IntentNodeKind::Aggregate { .. }
        | IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => false,
    }
}

fn insert_named_path(
    declaration: &EditorBootstrapDeclaration,
    arguments: &mut BTreeMap<String, ManagedValue>,
    path: &IntentProjectionPath,
    value: ManagedValue,
) -> Result<(), EditorDeclarationInsertionError> {
    let Some((IntentProjectionPathSegment::Field(first), tail)) = path.segments().split_first()
    else {
        return unsupported_clean_shape(
            declaration,
            "named authoring path must begin with a field",
        );
    };
    if tail.is_empty() {
        if arguments.insert(first.as_str().into(), value).is_some() {
            return unsupported_clean_shape(
                declaration,
                "two authored values share one named path",
            );
        }
        return Ok(());
    }
    let entry = arguments
        .entry(first.as_str().into())
        .or_insert(ManagedValue::Null);
    insert_named_path_tail(declaration, entry, tail, value)
}

fn insert_named_path_tail(
    declaration: &EditorBootstrapDeclaration,
    current: &mut ManagedValue,
    path: &[IntentProjectionPathSegment],
    value: ManagedValue,
) -> Result<(), EditorDeclarationInsertionError> {
    let Some((segment, tail)) = path.split_first() else {
        return unsupported_clean_shape(declaration, "named authoring path is empty");
    };
    match segment {
        IntentProjectionPathSegment::Field(field) => {
            if matches!(current, ManagedValue::Null) {
                *current = ManagedValue::Object(BTreeMap::new());
            }
            let ManagedValue::Object(fields) = current else {
                return unsupported_clean_shape(
                    declaration,
                    "named authoring object path collides with a scalar value",
                );
            };
            if tail.is_empty() {
                if fields.insert(field.as_str().into(), value).is_some() {
                    return unsupported_clean_shape(
                        declaration,
                        "two authored values share one named object path",
                    );
                }
                Ok(())
            } else {
                let entry = fields
                    .entry(field.as_str().into())
                    .or_insert(ManagedValue::Null);
                insert_named_path_tail(declaration, entry, tail, value)
            }
        }
        IntentProjectionPathSegment::Index(index) => {
            if matches!(current, ManagedValue::Null) {
                *current = ManagedValue::Array(Vec::new());
            }
            let ManagedValue::Array(values) = current else {
                return unsupported_clean_shape(
                    declaration,
                    "named authoring array path collides with an object or scalar value",
                );
            };
            let index = usize::from(*index);
            if values.len() <= index {
                values.resize(index + 1, ManagedValue::Null);
            }
            if tail.is_empty() {
                if !matches!(values[index], ManagedValue::Null) {
                    return unsupported_clean_shape(
                        declaration,
                        "two authored values share one named array path",
                    );
                }
                values[index] = value;
                Ok(())
            } else {
                insert_named_path_tail(declaration, &mut values[index], tail, value)
            }
        }
    }
}

fn clean_intent_literal(
    declaration: &EditorBootstrapDeclaration,
    literal: &IntentLiteral,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    match literal {
        IntentLiteral::Boolean(value) => Ok(ManagedValue::Bool(*value)),
        IntentLiteral::Integer(value) => Ok(ManagedValue::Number(exact_compact_i64(
            declaration,
            *value,
        )?)),
        IntentLiteral::Natural(value) => Ok(ManagedValue::Number(exact_compact_u64(
            declaration,
            *value,
        )?)),
        IntentLiteral::Text(value) => Ok(ManagedValue::String(value.as_str().into())),
        IntentLiteral::Enum(value) => Ok(ManagedValue::String(api_enum_value(value.as_str()))),
        IntentLiteral::Point(point) => finite_point_value(declaration, *point),
        IntentLiteral::Quantity { value, unit } => clean_quantity(declaration, *value, *unit),
    }
}

fn clean_quantity(
    declaration: &EditorBootstrapDeclaration,
    value: f64,
    unit: IntentUnit,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    if !value.is_finite() {
        return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    Ok(match unit {
        IntentUnit::Length => ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value,
        }),
        IntentUnit::Angle => ManagedValue::Unit(UnitLiteral {
            unit: "rad".into(),
            value,
        }),
        IntentUnit::Dimensionless => ManagedValue::Number(value),
    })
}

fn api_enum_value(value: &str) -> String {
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

fn insert_display_label(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    arguments: &mut BTreeMap<String, ManagedValue>,
) {
    let display = editor
        .coordinator()
        .intent()
        .organization()
        .node_names()
        .get(&node.id)
        .unwrap_or(&node.symbol)
        .as_str();
    if display != declaration.symbol.0 {
        arguments.insert("label".into(), ManagedValue::String(display.into()));
    }
}

fn insert_geometry_independent_values(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    arguments: &mut BTreeMap<String, ManagedValue>,
) -> Result<(), EditorDeclarationInsertionError> {
    use GeometryRecipeKind as G;
    let values: &[(&str, IntentPortRole, u16, LeafField, IntentUnit)] = match recipe {
        G::CenterRadiusCircle => &[(
            "radius",
            IntentPortRole::Target,
            0,
            LeafField::Value,
            IntentUnit::Length,
        )],
        G::RationalQuadraticConic => &[(
            "middleWeight",
            IntentPortRole::Target,
            0,
            LeafField::Weight,
            IntentUnit::Dimensionless,
        )],
        G::Parabola => &[
            (
                "trimStart",
                IntentPortRole::Target,
                0,
                LeafField::Parameter,
                IntentUnit::Dimensionless,
            ),
            (
                "trimEnd",
                IntentPortRole::Target,
                1,
                LeafField::Parameter,
                IntentUnit::Dimensionless,
            ),
        ],
        G::Hyperbola => &[
            (
                "semiConjugate",
                IntentPortRole::Target,
                0,
                LeafField::Value,
                IntentUnit::Length,
            ),
            (
                "trimStart",
                IntentPortRole::Target,
                1,
                LeafField::Parameter,
                IntentUnit::Dimensionless,
            ),
            (
                "trimEnd",
                IntentPortRole::Target,
                2,
                LeafField::Parameter,
                IntentUnit::Dimensionless,
            ),
        ],
        _ => &[],
    };
    for (name, role, index, leaf, unit) in values {
        let literal =
            accepted_node_output_literal(editor, node, *role, *index, *leaf).ok_or_else(|| {
                EditorDeclarationInsertionError::UnsupportedRecipe {
                    symbol: declaration.symbol.0.clone(),
                    recipe: format!("geometry authored value `{name}` has no accepted scalar"),
                }
            })?;
        let IntentLiteral::Quantity {
            value,
            unit: actual_unit,
        } = literal
        else {
            return unsupported_clean_shape(declaration, "geometry authored scalar is not numeric");
        };
        if actual_unit != *unit {
            return unsupported_clean_shape(declaration, "geometry authored scalar unit changed");
        }
        arguments.insert(name.to_string(), clean_quantity(declaration, value, *unit)?);
    }
    Ok(())
}

fn insert_dimension_value(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    arguments: &mut BTreeMap<String, ManagedValue>,
) -> Result<(), EditorDeclarationInsertionError> {
    let IntentNodeKind::Dimension { dimension } = node.kind else {
        return Ok(());
    };
    let unit = if dimension == DimensionKind::OrientedAngle {
        IntentUnit::Angle
    } else {
        IntentUnit::Length
    };
    let literal =
        accepted_node_output_literal(editor, node, IntentPortRole::Target, 0, LeafField::Value)
            .or_else(|| {
                accepted_node_output_literal(
                    editor,
                    node,
                    IntentPortRole::Target,
                    0,
                    LeafField::Angle,
                )
            })
            .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
                symbol: declaration.symbol.0.clone(),
                recipe: "dimension has no accepted target value".into(),
            })?;
    let IntentLiteral::Quantity {
        value,
        unit: actual_unit,
    } = literal
    else {
        return unsupported_clean_shape(declaration, "dimension target is not a quantity");
    };
    if actual_unit != unit {
        return unsupported_clean_shape(declaration, "dimension target unit changed");
    }
    arguments.insert("value".into(), clean_quantity(declaration, value, unit)?);
    Ok(())
}

fn accepted_node_output_literal(
    editor: &ProjectionalEditorSession,
    node: &IntentNode,
    role: IntentPortRole,
    index: u16,
    field: LeafField,
) -> Option<IntentLiteral> {
    let port = node.port_by_selector(IntentPortSelector::Node { role, index })?;
    editor
        .coordinator()
        .intent()
        .instance()
        .values()
        .get(&LeafRef {
            node: node.id,
            port: port.id,
            field,
        })
        .cloned()
        .or_else(|| accepted_output_literal(editor, port.as_ref(node.id), field))
}

fn insert_constraint_contacts(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    arguments: &mut BTreeMap<String, ManagedValue>,
) -> Result<(), EditorDeclarationInsertionError> {
    let IntentNodeKind::Constraint { constraint } = node.kind else {
        return Ok(());
    };
    let count = match constraint {
        ConstraintKind::PointOnCurve
        | ConstraintKind::LineCurveTangency
        | ConstraintKind::CurveDirection => 1,
        ConstraintKind::LineCircleTangency
        | ConstraintKind::CircleArcTangency
        | ConstraintKind::CurveCurveContact
        | ConstraintKind::CurveCurveTangency
        | ConstraintKind::EqualCurvature
        | ConstraintKind::EndpointContinuity
        | ConstraintKind::LineLineFillet
        | ConstraintKind::CurveCurveFillet => 2,
        _ => 0,
    };
    if count == 0 {
        return Ok(());
    }
    let contacts = (0..count)
        .map(|index| accepted_contact_state(editor, declaration, node, index))
        .collect::<Result<Vec<_>, _>>()?;
    if let [contact] = contacts.as_slice() {
        arguments.insert("contact".into(), contact.clone());
    } else if let [first, second] = contacts.as_slice() {
        arguments.insert(
            "contacts".into(),
            ManagedValue::Object(BTreeMap::from([
                ("first".into(), first.clone()),
                ("second".into(), second.clone()),
            ])),
        );
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one contact-state projection keeps the full explicit branch contract auditable together"
)]
fn accepted_contact_state(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    index: u16,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorDeclarationInsertionError::MissingAcceptedAuthority { which: "candidate" })?;
    let port = node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Contact,
            index,
        })
        .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("contact {index} has no semantic output"),
        })?;
    let Some(IntentNativeBinding::Contact(contact)) = accepted.ownership.port(port.as_ref(node.id))
    else {
        return unsupported_clean_shape(declaration, "contact output has no accepted native state");
    };
    let document = accepted.session.design_document();
    let contact = document.contact(contact).ok_or_else(|| {
        EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "accepted contact disappeared".into(),
        }
    })?;
    let parameter = document
        .scalar(contact.parameter)
        .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "accepted contact parameter disappeared".into(),
        })?
        .value;
    if !parameter.is_finite() {
        return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
            symbol: declaration.symbol.0.clone(),
        });
    }
    match contact.domain {
        ContactDomain::Bounded { lower, upper } if !lower.is_finite() || !upper.is_finite() => {
            return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
                symbol: declaration.symbol.0.clone(),
            });
        }
        ContactDomain::Periodic { period } if !period.is_finite() => {
            return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
                symbol: declaration.symbol.0.clone(),
            });
        }
        ContactDomain::SupportingLine
        | ContactDomain::Bounded { .. }
        | ContactDomain::Periodic { .. } => {}
    }
    let neighborhood = match contact.neighborhood {
        ContactNeighborhood::Interior => "interior",
        ContactNeighborhood::Start => "start",
        ContactNeighborhood::End => "end",
        ContactNeighborhood::Local { .. } => "local",
    };
    let mut neighborhood_fields =
        BTreeMap::from([("kind".into(), ManagedValue::String(neighborhood.into()))]);
    if let ContactNeighborhood::Local { lower, upper } = contact.neighborhood {
        if !lower.is_finite() || !upper.is_finite() {
            return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
                symbol: declaration.symbol.0.clone(),
            });
        }
        neighborhood_fields.insert("lower".into(), ManagedValue::Number(lower));
        neighborhood_fields.insert("upper".into(), ManagedValue::Number(upper));
    }
    let contact_prefix = if node
        .fields
        .keys()
        .any(|field| field.0.as_str().starts_with("first_contact_"))
    {
        if index == 0 {
            "first_contact"
        } else {
            "second_contact"
        }
    } else {
        "contact"
    };
    let retained_unoriented = matches!(
        intent_field(node, &format!("{contact_prefix}_orientation")),
        Some(IntentLiteral::Enum(value)) if value.as_str() == "unoriented"
    );
    let orientation = match contact.tangent_orientation {
        Some(TangentOrientation::Aligned) => "aligned",
        Some(TangentOrientation::Opposed) => "opposed",
        None if retained_unoriented => "unoriented",
        None => "none",
    };
    let mut state = BTreeMap::from([
        (
            "neighborhood".into(),
            ManagedValue::Object(neighborhood_fields),
        ),
        (
            "orientation".into(),
            ManagedValue::String(orientation.into()),
        ),
        ("parameter".into(), ManagedValue::Number(parameter)),
        (
            "winding".into(),
            ManagedValue::Number(f64::from(contact.winding)),
        ),
    ]);
    if matches!(contact.domain, ContactDomain::SupportingLine) {
        state.insert(
            "support".into(),
            ManagedValue::String("supportingLine".into()),
        );
    }
    if let Some(range) = contact.admissible_range {
        if !range.lower.is_finite() || !range.upper.is_finite() {
            return Err(EditorDeclarationInsertionError::NonFiniteGeometry {
                symbol: declaration.symbol.0.clone(),
            });
        }
        state.insert(
            "range".into(),
            ManagedValue::Object(BTreeMap::from([
                ("lower".into(), ManagedValue::Number(range.lower)),
                ("upper".into(), ManagedValue::Number(range.upper)),
            ])),
        );
    }
    Ok(ManagedValue::Object(state))
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the two explicit branch parents are authenticated independently at one operation boundary"
)]
fn clean_associative_fillet_arguments(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<BTreeMap<String, ManagedValue>, EditorDeclarationInsertionError> {
    let radius = required_quantity_field(declaration, node, "radius", IntentUnit::Length)?;
    let radius_mode = required_enum_field(declaration, node, "radius_mode")?;
    let endpoint_order = required_enum_field(declaration, node, "endpoint_order")?;
    let sweep = required_enum_field(declaration, node, "sweep")?;
    let mut parents = Vec::with_capacity(2);
    for (index, prefix) in [(0_u16, "first"), (1, "second")] {
        let span = lexical_input_reference(
            editor,
            declaration,
            node,
            InputSlot::new(InputRole::Span, index),
            selected,
            existing,
            keyed_geometry_declarations,
        )?
        .ok_or_else(|| EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: format!("Associative Fillet {prefix} parent is absent"),
        })?;
        let parameter = required_quantity_field(
            declaration,
            node,
            &format!("{prefix}_parameter"),
            IntentUnit::Dimensionless,
        )?;
        let winding = required_integer_field(declaration, node, &format!("{prefix}_winding"))?;
        let neighborhood_kind =
            required_enum_field(declaration, node, &format!("{prefix}_neighborhood"))?;
        let mut neighborhood =
            BTreeMap::from([("kind".into(), ManagedValue::String(neighborhood_kind))]);
        if matches!(neighborhood.get("kind"), Some(ManagedValue::String(kind)) if kind == "local") {
            neighborhood.insert(
                "lower".into(),
                required_quantity_field(
                    declaration,
                    node,
                    &format!("{prefix}_local_lower"),
                    IntentUnit::Dimensionless,
                )?,
            );
            neighborhood.insert(
                "upper".into(),
                required_quantity_field(
                    declaration,
                    node,
                    &format!("{prefix}_local_upper"),
                    IntentUnit::Dimensionless,
                )?,
            );
        }
        let anchored =
            required_boolean_field(declaration, node, &format!("{prefix}_periodic_anchor"))?;
        let periodic_anchor = if anchored {
            ManagedValue::Object(BTreeMap::from([
                ("kind".into(), ManagedValue::String("anchor".into())),
                (
                    "parameter".into(),
                    required_quantity_field(
                        declaration,
                        node,
                        &format!("{prefix}_anchor_parameter"),
                        IntentUnit::Dimensionless,
                    )?,
                ),
                (
                    "winding".into(),
                    required_integer_field(declaration, node, &format!("{prefix}_anchor_winding"))?,
                ),
            ]))
        } else {
            ManagedValue::Object(BTreeMap::from([(
                "kind".into(),
                ManagedValue::String("none".into()),
            )]))
        };
        parents.push(ManagedValue::Object(BTreeMap::from([
            ("neighborhood".into(), ManagedValue::Object(neighborhood)),
            (
                "normalSide".into(),
                ManagedValue::String(required_enum_field(
                    declaration,
                    node,
                    &format!("{prefix}_normal_side"),
                )?),
            ),
            ("parameter".into(), parameter),
            ("periodicAnchor".into(), periodic_anchor),
            ("span".into(), span),
            (
                "trimEndpoint".into(),
                ManagedValue::String(required_enum_field(
                    declaration,
                    node,
                    &format!("{prefix}_trim_endpoint"),
                )?),
            ),
            ("winding".into(), winding),
        ])));
    }
    let mut arguments = BTreeMap::from([
        ("endpointOrder".into(), ManagedValue::String(endpoint_order)),
        ("parents".into(), ManagedValue::Array(parents)),
        ("radius".into(), radius),
        ("radiusMode".into(), ManagedValue::String(radius_mode)),
        ("sweep".into(), ManagedValue::String(sweep)),
    ]);
    insert_display_label(editor, declaration, node, &mut arguments);
    Ok(arguments)
}

fn required_quantity_field(
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    name: &str,
    expected: IntentUnit,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let Some(IntentLiteral::Quantity { value, unit }) = intent_field(node, name) else {
        return unsupported_clean_shape(
            declaration,
            &format!("required quantity field `{name}` is absent"),
        );
    };
    if *unit != expected {
        return unsupported_clean_shape(
            declaration,
            &format!("quantity field `{name}` has the wrong unit"),
        );
    }
    clean_quantity(declaration, *value, expected)
}

fn required_enum_field(
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    name: &str,
) -> Result<String, EditorDeclarationInsertionError> {
    let Some(IntentLiteral::Enum(value)) = intent_field(node, name) else {
        return unsupported_clean_shape(
            declaration,
            &format!("required enum field `{name}` is absent"),
        );
    };
    Ok(api_enum_value(value.as_str()))
}

fn required_integer_field(
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    name: &str,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let Some(IntentLiteral::Integer(value)) = intent_field(node, name) else {
        return unsupported_clean_shape(
            declaration,
            &format!("required integer field `{name}` is absent"),
        );
    };
    Ok(ManagedValue::Number(exact_compact_i64(
        declaration,
        *value,
    )?))
}

fn required_boolean_field(
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    name: &str,
) -> Result<bool, EditorDeclarationInsertionError> {
    let Some(IntentLiteral::Boolean(value)) = intent_field(node, name) else {
        return unsupported_clean_shape(
            declaration,
            &format!("required boolean field `{name}` is absent"),
        );
    };
    Ok(*value)
}

/// Projects one accepted computed Fillet through its native feature authority
/// and exact lexical span owners.
///
/// The Intent node alone is insufficient here: its fields describe the
/// requested branch, while the accepted computed-feature sidecar proves the
/// branch which is current and owns every child corner. Conversely, native
/// feature state carries persistent spans rather than source coordinates. A
/// direct declaration is emitted only when both authorities agree exactly.
#[allow(
    clippy::too_many_lines,
    reason = "the complete accepted Fillet branch and lexical-owner authentication remains reviewable as one closed conversion"
)]
fn computed_fillet_set_arguments(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(EditorDeclarationInsertionError::MissingAcceptedAuthority { which: "candidate" })?;
    let feature_port = node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Feature,
            index: 0,
        })
        .ok_or_else(|| EditorDeclarationInsertionError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        })?;
    let Some(IntentNativeBinding::ComputedFeature(feature_id)) =
        accepted.ownership.port(feature_port.as_ref(node.id))
    else {
        return Err(EditorDeclarationInsertionError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        });
    };
    let feature = accepted.features.feature(feature_id).ok_or_else(|| {
        EditorDeclarationInsertionError::MissingComputedFeature {
            symbol: declaration.symbol.0.clone(),
        }
    })?;
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    let expected_name = match node.fields.get(&geosolve_sketch_intent::IntentFieldKey(
        geosolve_sketch_intent::IntentKey::new("name").expect("static Fillet name field is valid"),
    )) {
        Some(IntentLiteral::Text(value)) => value.as_str(),
        None => node.symbol.as_str(),
        Some(_) => {
            return invalid_computed_fillet(declaration, "Intent feature name is not text");
        }
    };
    if feature.label != expected_name {
        return invalid_computed_fillet(
            declaration,
            "accepted feature name disagrees with its Intent owner",
        );
    }
    if !fillet.radius.is_finite() || fillet.radius <= 0.0 {
        return invalid_computed_fillet(declaration, "radius must be finite and positive");
    }
    if feature.suppressed != node.suppressed {
        return invalid_computed_fillet(
            declaration,
            "accepted feature suppression disagrees with its Intent owner",
        );
    }
    if fillet.corners.is_empty()
        || fillet.corners.len() != node.child_order.len()
        || node.child_order.len() != node.children.len()
        || !node.operation_outputs.is_empty()
    {
        return invalid_computed_fillet(
            declaration,
            "accepted corners do not match the complete Intent child inventory",
        );
    }

    let mut corners = Vec::with_capacity(fillet.corners.len());
    for (ordinal, accepted_corner) in fillet.corners.iter().enumerate() {
        let ordinal_u16 = u16::try_from(ordinal).map_err(|_| {
            EditorDeclarationInsertionError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
                reason: "corner ordinal exceeds Intent limits".into(),
            }
        })?;
        let corner_port = node
            .port_by_selector(IntentPortSelector::InitialChild {
                ordinal: ordinal_u16,
                role: IntentPortRole::FeatureCorner,
                index: 0,
            })
            .ok_or_else(|| EditorDeclarationInsertionError::MissingComputedFeature {
                symbol: declaration.symbol.0.clone(),
            })?;
        if accepted.ownership.port(corner_port.as_ref(node.id))
            != Some(IntentNativeBinding::ComputedFeatureCorner(
                accepted_corner.id,
            ))
        {
            return invalid_computed_fillet(
                declaration,
                "accepted corner ownership disagrees with Intent child order",
            );
        }
        let corner = accepted_corner.without_id();
        validate_computed_fillet_corner(declaration, corner)?;
        let first_index = ordinal_u16.checked_mul(2).ok_or_else(|| {
            EditorDeclarationInsertionError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
                reason: "Fillet span input index exceeds Intent limits".into(),
            }
        })?;
        let second_index = first_index.checked_add(1).ok_or_else(|| {
            EditorDeclarationInsertionError::InvalidComputedFeatureState {
                symbol: declaration.symbol.0.clone(),
                reason: "Fillet span input index exceeds Intent limits".into(),
            }
        })?;
        corners.push(ManagedValue::Object(BTreeMap::from([
            (
                "endpointOrder".into(),
                ManagedValue::String(
                    match corner.endpoint_order {
                        DocumentFilletEndpointOrder::FirstThenSecond => "firstThenSecond",
                        DocumentFilletEndpointOrder::SecondThenFirst => "secondThenFirst",
                    }
                    .into(),
                ),
            ),
            (
                "key".into(),
                ManagedValue::String(fillet_corner_key(ordinal_u16)),
            ),
            (
                "parents".into(),
                ManagedValue::Array(vec![
                    computed_fillet_parent_value(
                        editor,
                        declaration,
                        node,
                        selected,
                        existing,
                        keyed_geometry_declarations,
                        first_index,
                        corner,
                    )?,
                    computed_fillet_parent_value(
                        editor,
                        declaration,
                        node,
                        selected,
                        existing,
                        keyed_geometry_declarations,
                        second_index,
                        corner,
                    )?,
                ]),
            ),
            (
                "sweep".into(),
                ManagedValue::String(
                    match corner.sweep {
                        DocumentArcSweep::CounterClockwise => "counterClockwise",
                        DocumentArcSweep::Clockwise => "clockwise",
                    }
                    .into(),
                ),
            ),
        ])));
    }

    Ok(ManagedValue::Object(BTreeMap::from([
        ("corners".into(), ManagedValue::Array(corners)),
        ("label".into(), ManagedValue::String(feature.label.clone())),
        (
            "radius".into(),
            ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: fillet.radius,
            }),
        ),
    ])))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one explicit Fillet parent coordinate binds accepted branch state to its exact lexical input"
)]
fn computed_fillet_parent_value(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
    input_index: u16,
    corner: NewComputedFilletCorner,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let parent = if input_index.is_multiple_of(2) {
        corner.first
    } else {
        corner.second
    };
    let input_name = format!(
        "corners[{}].parents[{}].span",
        input_index / 2,
        input_index % 2,
    );
    let span = computed_fillet_span_reference(
        editor,
        declaration,
        node,
        selected,
        existing,
        keyed_geometry_declarations,
        input_index,
        &input_name,
        parent.source,
    )?;
    let neighborhood = match parent.neighborhood {
        ContactNeighborhood::Interior => {
            BTreeMap::from([("kind".into(), ManagedValue::String("interior".into()))])
        }
        ContactNeighborhood::Start => {
            BTreeMap::from([("kind".into(), ManagedValue::String("start".into()))])
        }
        ContactNeighborhood::End => {
            BTreeMap::from([("kind".into(), ManagedValue::String("end".into()))])
        }
        ContactNeighborhood::Local { lower, upper } => BTreeMap::from([
            ("kind".into(), ManagedValue::String("local".into())),
            ("lower".into(), ManagedValue::Number(lower)),
            ("upper".into(), ManagedValue::Number(upper)),
        ]),
    };
    let periodic_anchor = parent.periodic_anchor.map_or_else(
        || {
            ManagedValue::Object(BTreeMap::from([(
                "kind".into(),
                ManagedValue::String("none".into()),
            )]))
        },
        |anchor| {
            ManagedValue::Object(BTreeMap::from([
                ("kind".into(), ManagedValue::String("anchor".into())),
                ("parameter".into(), ManagedValue::Number(anchor.parameter)),
                (
                    "winding".into(),
                    ManagedValue::Number(f64::from(anchor.winding)),
                ),
            ]))
        },
    );
    Ok(ManagedValue::Object(BTreeMap::from([
        ("neighborhood".into(), ManagedValue::Object(neighborhood)),
        (
            "normalSide".into(),
            ManagedValue::String(
                match parent.normal_side {
                    DocumentCurveNormalSide::Left => "left",
                    DocumentCurveNormalSide::Right => "right",
                }
                .into(),
            ),
        ),
        (
            "parameter".into(),
            ManagedValue::Number(parent.picked_parameter),
        ),
        ("periodicAnchor".into(), periodic_anchor),
        (
            "trimEndpoint".into(),
            ManagedValue::String(
                match parent.retained_endpoint {
                    DocumentFilletTrimEndpoint::Start => "start",
                    DocumentFilletTrimEndpoint::End => "end",
                }
                .into(),
            ),
        ),
        ("span".into(), span),
        (
            "winding".into(),
            ManagedValue::Number(f64::from(parent.winding)),
        ),
    ])))
}

#[allow(
    clippy::too_many_arguments,
    reason = "the accepted native span and lexical owner are authenticated at one boundary"
)]
fn computed_fillet_span_reference(
    editor: &ProjectionalEditorSession,
    declaration: &EditorBootstrapDeclaration,
    node: &IntentNode,
    selected: &BTreeMap<NodeId, ExistingSourceOwner>,
    existing: &BTreeMap<NodeId, ExistingSourceOwner>,
    keyed_geometry_declarations: &BTreeSet<NodeId>,
    input_index: u16,
    input_name: &str,
    expected: NativeCurveSpanSource,
) -> Result<ManagedValue, EditorDeclarationInsertionError> {
    let mismatch = || EditorDeclarationInsertionError::FilletSpanOwnershipMismatch {
        symbol: declaration.symbol.0.clone(),
        input: input_name.to_owned(),
    };
    let source = node
        .inputs
        .get(&InputSlot::new(InputRole::Span, input_index))
        .copied()
        .ok_or_else(mismatch)?;
    let source_node = editor
        .coordinator()
        .intent()
        .graph()
        .node(source.node)
        .ok_or_else(mismatch)?;
    let source_port = source_node.port(source.port).ok_or_else(mismatch)?;
    if source.kind != IntentPortKind::CurveSpan
        || source_port.kind != IntentPortKind::CurveSpan
        || editor
            .coordinator()
            .accepted_materialization()
            .and_then(|accepted| accepted.ownership.port(source))
            != Some(IntentNativeBinding::CurveSpan(expected.span))
    {
        return Err(mismatch());
    }
    let owner = selected
        .get(&source.node)
        .or_else(|| existing.get(&source.node))
        .ok_or(EditorDeclarationInsertionError::UnownedDependency {
            symbol: declaration.symbol.0.clone(),
            dependency: source.node,
        })?;
    let path = if keyed_geometry_declarations.contains(&source.node) {
        clean_keyed_geometry_path(source_node, source_port.selector)
            .unwrap_or_else(|| owner.generic_reference_path(source_port.selector))
    } else {
        owner.generic_reference_path(source_port.selector)
    };
    Ok(ManagedValue::Reference {
        declaration: owner.declaration.clone(),
        path,
    })
}

fn validate_computed_fillet_corner(
    declaration: &EditorBootstrapDeclaration,
    corner: NewComputedFilletCorner,
) -> Result<(), EditorDeclarationInsertionError> {
    for parent in [corner.first, corner.second] {
        let valid_neighborhood = match parent.neighborhood {
            ContactNeighborhood::Local { lower, upper } => {
                lower.is_finite() && upper.is_finite() && lower < upper
            }
            ContactNeighborhood::Interior
            | ContactNeighborhood::Start
            | ContactNeighborhood::End => true,
        };
        if !parent.picked_parameter.is_finite()
            || !valid_neighborhood
            || parent
                .periodic_anchor
                .is_some_and(|anchor| !anchor.parameter.is_finite())
        {
            return invalid_computed_fillet(
                declaration,
                "a parent contact contains invalid numeric branch state",
            );
        }
    }
    Ok(())
}

fn invalid_computed_fillet<T>(
    declaration: &EditorBootstrapDeclaration,
    reason: &str,
) -> Result<T, EditorDeclarationInsertionError> {
    Err(
        EditorDeclarationInsertionError::InvalidComputedFeatureState {
            symbol: declaration.symbol.0.clone(),
            reason: reason.into(),
        },
    )
}

fn fillet_corner_key(ordinal: u16) -> String {
    format!("corner{ordinal}")
}

/// Reads an accepted native writable value when the authored Intent draft
/// deliberately relied on the materializer's semantic default. Tangent Arc
/// contact parameters are the important example: they are real, editable
/// scalar outputs, but the construction draft may omit their initial leaves
/// because its explicit contact fields determine those defaults. Reverse
/// projection records the independently accepted native values so compact
/// source remains lossless and cold-replayable.
fn accepted_output_literal(
    editor: &ProjectionalEditorSession,
    port: IntentPortRef,
    field: LeafField,
) -> Option<IntentLiteral> {
    let accepted = editor.coordinator().accepted_materialization()?;
    let document = accepted.session.design_document();
    match (accepted.ownership.port(port)?, field) {
        (IntentNativeBinding::Point(point), LeafField::X) => Some(IntentLiteral::Quantity {
            value: document.point(point)?.position[0],
            unit: geosolve_sketch_intent::IntentUnit::Length,
        }),
        (IntentNativeBinding::Point(point), LeafField::Y) => Some(IntentLiteral::Quantity {
            value: document.point(point)?.position[1],
            unit: geosolve_sketch_intent::IntentUnit::Length,
        }),
        (IntentNativeBinding::Scalar(scalar), leaf) => {
            let scalar = document.scalar(scalar)?;
            let unit = match leaf {
                LeafField::Angle => geosolve_sketch_intent::IntentUnit::Angle,
                LeafField::Weight | LeafField::Parameter => {
                    geosolve_sketch_intent::IntentUnit::Dimensionless
                }
                LeafField::Value => match scalar.unit {
                    geosolve_sketch::ScalarUnit::Length => {
                        geosolve_sketch_intent::IntentUnit::Length
                    }
                    geosolve_sketch::ScalarUnit::Angle => geosolve_sketch_intent::IntentUnit::Angle,
                    geosolve_sketch::ScalarUnit::Parameter => {
                        geosolve_sketch_intent::IntentUnit::Dimensionless
                    }
                },
                LeafField::X | LeafField::Y => return None,
            };
            Some(IntentLiteral::Quantity {
                value: scalar.value,
                unit,
            })
        }
        _ => None,
    }
}

fn exact_compact_i64(
    declaration: &EditorBootstrapDeclaration,
    value: i64,
) -> Result<f64, EditorDeclarationInsertionError> {
    const MAX_EXACT_INTEGER: u64 = 1_u64 << 53;
    if value.unsigned_abs() > MAX_EXACT_INTEGER {
        return Err(EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "geometry integer exceeds the compact exact-number range".into(),
        });
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "the exact IEEE-754 integer range is checked immediately above"
    )]
    Ok(value as f64)
}

fn exact_compact_u64(
    declaration: &EditorBootstrapDeclaration,
    value: u64,
) -> Result<f64, EditorDeclarationInsertionError> {
    const MAX_EXACT_INTEGER: u64 = 1_u64 << 53;
    if value > MAX_EXACT_INTEGER {
        return Err(EditorDeclarationInsertionError::UnsupportedRecipe {
            symbol: declaration.symbol.0.clone(),
            recipe: "geometry natural exceeds the compact exact-number range".into(),
        });
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "the exact IEEE-754 integer range is checked immediately above"
    )]
    Ok(value as f64)
}

fn semantic_output_path(path: &geosolve_sketch_intent::IntentProjectionPath) -> SemanticOutputPath {
    SemanticOutputPath(
        path.segments()
            .iter()
            .map(|segment| match segment {
                geosolve_sketch_intent::IntentProjectionPathSegment::Field(field) => {
                    ManagedPathSegment::Field(field.as_str().to_owned())
                }
                geosolve_sketch_intent::IntentProjectionPathSegment::Index(index) => {
                    ManagedPathSegment::Index(usize::from(*index))
                }
            })
            .collect(),
    )
}

fn intent_field<'a>(node: &'a IntentNode, name: &str) -> Option<&'a IntentLiteral> {
    node.fields
        .iter()
        .find_map(|(field, value)| (field.0.as_str() == name).then_some(value))
}

fn polyline_vertex_key(ordinal: u16) -> String {
    format!("v{ordinal}")
}

fn direct_polyline_span_path(selector: IntentPortSelector) -> Option<SemanticOutputPath> {
    let IntentPortSelector::InitialChild {
        ordinal,
        role: IntentPortRole::Span,
        index: 0,
    } = selector
    else {
        return None;
    };
    Some(SemanticOutputPath(vec![
        ManagedPathSegment::Field("segments".into()),
        ManagedPathSegment::Field("byKey".into()),
        ManagedPathSegment::Field(polyline_vertex_key(ordinal)),
    ]))
}

/// Maps one just-emitted keyed geometry result through the public clean API.
///
/// Candidate graph descriptors necessarily use native ordinal paths, while
/// clean Polyline/NURBS source exposes stable authored keys. Existing source
/// declarations are refined through executed semantic outputs; declarations
/// created by this same gesture need this equivalent mapping before the
/// candidate source has been executed.
fn clean_keyed_geometry_path(
    node: &IntentNode,
    selector: IntentPortSelector,
) -> Option<SemanticOutputPath> {
    use GeometryRecipeKind as G;
    use IntentPortRole as R;

    let fields = |segments: &[&str]| {
        SemanticOutputPath(
            segments
                .iter()
                .map(|segment| ManagedPathSegment::Field((*segment).into()))
                .collect(),
        )
    };
    match (&node.kind, selector) {
        (
            IntentNodeKind::Geometry {
                recipe: G::Polyline,
            },
            IntentPortSelector::InitialChild {
                ordinal,
                role: R::Corner,
                index: 0,
            },
        ) => Some(fields(&[
            "vertices",
            "byKey",
            &polyline_vertex_key(ordinal),
        ])),
        (
            IntentNodeKind::Geometry {
                recipe: G::Polyline,
            },
            IntentPortSelector::InitialChild {
                ordinal,
                role: R::Span,
                index: 0,
            },
        ) => direct_polyline_span_path(IntentPortSelector::InitialChild {
            ordinal,
            role: R::Span,
            index: 0,
        }),
        (
            IntentNodeKind::Geometry {
                recipe: G::OpenControlNurbs | G::PeriodicControlNurbs,
            },
            IntentPortSelector::InitialChild {
                ordinal,
                role: R::Control,
                index: 0,
            },
        ) => Some(fields(&[
            "controls",
            "byKey",
            &polyline_vertex_key(ordinal),
            "position",
        ])),
        (
            IntentNodeKind::Geometry {
                recipe: G::OpenControlNurbs | G::PeriodicControlNurbs,
            },
            IntentPortSelector::InitialChild {
                ordinal,
                role: R::Target,
                index: 0,
            },
        ) => Some(fields(&[
            "controls",
            "byKey",
            &polyline_vertex_key(ordinal),
            "weight",
        ])),
        (
            IntentNodeKind::Geometry {
                recipe: G::OpenControlNurbs | G::PeriodicControlNurbs,
            },
            IntentPortSelector::Node {
                role: R::Span,
                index,
            },
        ) => Some(fields(&["spans", "byKey", &polyline_vertex_key(index)])),
        _ => None,
    }
}

fn valid_managed_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration() -> EditorBootstrapDeclaration {
        EditorBootstrapDeclaration::new(NodeId::from_raw(1), SemanticSymbol("geometry1".into()))
    }

    #[test]
    fn clean_argument_gate_rejects_transport_keys_at_any_depth() {
        for forbidden in [
            "recipe",
            "inputs",
            "fields",
            "values",
            "results",
            "operationOutputs",
            "outputs",
            "editLens",
        ] {
            let value = ManagedValue::Object(BTreeMap::from([(
                "nested".into(),
                ManagedValue::Array(vec![ManagedValue::Object(BTreeMap::from([(
                    forbidden.into(),
                    ManagedValue::Array(Vec::new()),
                )]))]),
            )]));
            assert!(matches!(
                validate_clean_source_arguments(&declaration(), &value),
                Err(EditorDeclarationInsertionError::UnsupportedRecipe { .. })
            ));
        }
    }

    #[test]
    fn clean_argument_gate_accepts_typed_named_objects_and_references() {
        let value = ManagedValue::Object(BTreeMap::from([
            (
                "start".into(),
                ManagedValue::Reference {
                    declaration: SemanticSymbol("point1".into()),
                    path: SemanticOutputPath(vec![ManagedPathSegment::Field("point".into())]),
                },
            ),
            (
                "contact".into(),
                ManagedValue::Object(BTreeMap::from([
                    ("orientation".into(), ManagedValue::String("none".into())),
                    ("parameter".into(), ManagedValue::Number(0.25)),
                ])),
            ),
        ]));
        validate_clean_source_arguments(&declaration(), &value).unwrap();
    }
}
