// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only accepted source ownership, shared by editor hosts and engine inspection.
use crate::{
    CodeOwnerAddress, ExpandedCodeProject, ExpandedPort, ExpandedSemanticTarget,
    GeneratedMemberAddress, KeyedReconcileState, ManagedDocument, SemanticSymbol,
};
use geosolve_constraint_editor::{IntentNativeBinding, ProjectionalEditorSession};
use geosolve_sketch_intent::{IntentKey, NodeId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ManagedNavigationIndex {
    pub source: String,
    pub source_digest: String,
    pub blocked_reason: Option<String>,
    pub entries: Vec<ManagedNavigationEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ManagedNavigationEntry {
    pub id: String,
    pub nodes: Vec<NodeId>,
    /// Generated members may name one point/span of a larger declaration.
    /// `None` selects complete declaration ownership; `Some` selects only
    /// these exact accepted outputs, including an honestly empty member.
    pub exact_bindings: Option<Vec<IntentNativeBinding>>,
    pub source_start: usize,
    pub source_end: usize,
}

/// Stable presentation row for one managed declaration.
#[must_use]
pub fn managed_panel_row_id(symbol: &SemanticSymbol) -> String {
    format!("managed:{}", symbol.0)
}

/// Stable presentation row for one validated generated member.
/// # Panics
/// Only if serializing a validated generated-member address fails.
#[must_use]
pub fn generated_panel_row_id(address: &GeneratedMemberAddress) -> String {
    format!(
        "generated:{}",
        serde_json::to_string(address)
            .expect("validated generated-member addresses serialize infallibly")
    )
}

/// Projects source spans and exact native ownership from an accepted expansion.
/// Host draft/failure policy is recorded separately in `blocked_reason`.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "accepted source and exact generated ownership form one coherent read-only index"
)]
pub fn managed_navigation_index(
    managed: &ManagedDocument,
    expansion: &ExpandedCodeProject,
    generated: &KeyedReconcileState,
    editor: &ProjectionalEditorSession,
) -> ManagedNavigationIndex {
    let mut index = ManagedNavigationIndex {
        source: managed.source.clone(),
        source_digest: managed.source_digest.clone(),
        blocked_reason: None,
        entries: Vec::new(),
    };
    let nodes_by_alias = editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .map(|node| (&node.symbol, node.id))
        .collect::<BTreeMap<_, _>>();
    let mut declarations = managed
        .program
        .declarations
        .iter()
        .map(|declaration| (declaration.symbol.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (alias, owner) in &expansion.declaration_provenance {
        if let Some(nodes) = declarations.get_mut(owner)
            && let Some(node) = nodes_by_alias.get(alias)
        {
            nodes.insert(*node);
        }
    }

    let mut host_nodes = BTreeMap::<GeneratedMemberAddress, BTreeSet<NodeId>>::new();
    for child in &expansion.generated_children {
        let node = nodes_by_alias.get(&child.alias);
        let owner = match &child.address.owner.address {
            CodeOwnerAddress::DirectDeclaration { declaration } => declaration.clone(),
            CodeOwnerAddress::GeneratedMember { address } => {
                let nodes = host_nodes.entry(address.clone()).or_default();
                nodes.extend(node.copied());
                source_owner(address, &child.declaration, &declarations)
            }
        };
        if let Some(nodes) = declarations.get_mut(&owner) {
            nodes.extend(node.copied());
        }
    }

    let mut generated_entries = BTreeMap::<SemanticSymbol, Vec<_>>::new();
    for member in generated.ordered_members() {
        let Some(provenance) = expansion.generated_provenance.get(&member.address) else {
            continue;
        };
        let owner = source_owner(&member.address, &provenance.declaration, &declarations);
        let Some(declaration_nodes) = declarations.get_mut(&owner) else {
            continue;
        };
        let resolver = NavigationBindingResolver {
            editor,
            expansion,
            nodes_by_alias: &nodes_by_alias,
            host_nodes: &host_nodes,
        };
        let nodes = generated_member_nodes(&member.address, &resolver, &mut BTreeSet::new());
        let exact_bindings = resolver.member_bindings(&member.address, &mut BTreeSet::new());
        declaration_nodes.extend(&nodes);
        generated_entries.entry(owner).or_default().push((
            generated_panel_row_id(&member.address),
            nodes.into_iter().collect::<Vec<_>>(),
            exact_bindings.into_iter().collect::<Vec<_>>(),
        ));
    }

    for declaration in &managed.program.declarations {
        let span = declaration.statement_span;
        index.entries.push(ManagedNavigationEntry {
            id: managed_panel_row_id(&declaration.symbol),
            nodes: declarations
                .remove(&declaration.symbol)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            exact_bindings: None,
            source_start: span.start,
            source_end: span.end,
        });
        for (id, nodes, exact_bindings) in generated_entries
            .remove(&declaration.symbol)
            .unwrap_or_default()
        {
            index.entries.push(ManagedNavigationEntry {
                id,
                nodes,
                exact_bindings: Some(exact_bindings),
                source_start: span.start,
                source_end: span.end,
            });
        }
    }
    index
}

struct NavigationBindingResolver<'a> {
    editor: &'a ProjectionalEditorSession,
    expansion: &'a ExpandedCodeProject,
    nodes_by_alias: &'a BTreeMap<&'a IntentKey, NodeId>,
    host_nodes: &'a BTreeMap<GeneratedMemberAddress, BTreeSet<NodeId>>,
}

impl NavigationBindingResolver<'_> {
    fn member_bindings(
        &self,
        address: &GeneratedMemberAddress,
        visiting: &mut BTreeSet<GeneratedMemberAddress>,
    ) -> BTreeSet<IntentNativeBinding> {
        if !visiting.insert(address.clone()) {
            return BTreeSet::new();
        }
        let mut bindings = BTreeSet::new();
        // Ordinary Fillets own their host results, never their borrowed input
        // corners. Composite outputs may additionally own native aggregate spans.
        if let Some(nodes) = self.host_nodes.get(address) {
            bindings.extend(
                nodes
                    .iter()
                    .flat_map(|node| self.owned_bindings(*node).iter().copied()),
            );
        } else if let Some(provenance) = self.expansion.generated_provenance.get(address) {
            self.collect_target_bindings(&provenance.target, visiting, &mut bindings);
        }
        bindings.extend(self.member_aggregate_bindings(address));
        visiting.remove(address);
        bindings
    }

    fn member_aggregate_bindings(
        &self,
        address: &GeneratedMemberAddress,
    ) -> BTreeSet<IntentNativeBinding> {
        let mut bindings = BTreeSet::new();
        if let Some(provenance) = self.expansion.generated_provenance.get(address) {
            self.collect_aggregate_bindings(&provenance.target, &address.invocation, &mut bindings);
        }
        bindings
    }

    fn collect_aggregate_bindings(
        &self,
        target: &ExpandedSemanticTarget,
        invocation: &str,
        bindings: &mut BTreeSet<IntentNativeBinding>,
    ) {
        let (alias, selector) = match target {
            ExpandedSemanticTarget::Declaration { alias, .. } => (alias, None),
            ExpandedSemanticTarget::Port { port } => {
                (&port.alias, Some((port.selector, port.kind)))
            }
            ExpandedSemanticTarget::Collection { members } => {
                for member in members.values() {
                    self.collect_aggregate_bindings(member, invocation, bindings);
                }
                return;
            }
            ExpandedSemanticTarget::FeatureCorner { .. }
            | ExpandedSemanticTarget::HostOutput { .. } => return,
        };
        if self
            .expansion
            .declaration_provenance
            .get(alias)
            .is_none_or(|owner| owner.0 != invocation)
        {
            return;
        }
        let Some(accepted) = self.editor.coordinator().accepted_materialization() else {
            return;
        };
        let graph = self.editor.coordinator().intent().graph();
        let Some(node) = graph.node_by_symbol(alias) else {
            return;
        };
        for port in node
            .ports
            .values()
            .filter(|port| selector.is_none_or(|selector| selector == (port.selector, port.kind)))
        {
            let Some(aggregate) = accepted.ownership.aggregate(port.as_ref(node.id)) else {
                continue;
            };
            for span in &aggregate.spans {
                let Some(owner) = accepted
                    .ownership
                    .exact_owner(IntentNativeBinding::Curve(span.curve))
                    .and_then(|owner| graph.node(owner))
                else {
                    continue;
                };
                // An aggregate can contain operands from other declarations. Its
                // output only acquires spans generated by this same invocation.
                if self
                    .expansion
                    .declaration_provenance
                    .get(&owner.symbol)
                    .is_some_and(|owner| owner.0 == invocation)
                {
                    bindings.insert(IntentNativeBinding::CurveSpan(*span));
                }
            }
        }
    }

    fn collect_target_bindings(
        &self,
        target: &ExpandedSemanticTarget,
        visiting: &mut BTreeSet<GeneratedMemberAddress>,
        bindings: &mut BTreeSet<IntentNativeBinding>,
    ) {
        match target {
            ExpandedSemanticTarget::Declaration { alias, .. } => {
                if let Some(node) = self.nodes_by_alias.get(alias) {
                    bindings.extend(self.owned_bindings(*node));
                }
            }
            ExpandedSemanticTarget::Port { port } => bindings.extend(self.port_binding(port)),
            ExpandedSemanticTarget::FeatureCorner { corner } => {
                bindings.extend(self.port_binding(&corner.point));
            }
            ExpandedSemanticTarget::Collection { members } => {
                for member in members.values() {
                    self.collect_target_bindings(member, visiting, bindings);
                }
            }
            ExpandedSemanticTarget::HostOutput { address, .. } => {
                bindings.extend(self.member_bindings(address, visiting));
            }
        }
    }

    fn owned_bindings(&self, node: NodeId) -> &[IntentNativeBinding] {
        self.editor
            .coordinator()
            .accepted_materialization()
            .and_then(|accepted| accepted.ownership.node(node))
            .map_or(&[], |owner| owner.owned.as_slice())
    }

    fn port_binding(&self, handle: &ExpandedPort) -> Option<IntentNativeBinding> {
        let node = self
            .editor
            .coordinator()
            .intent()
            .graph()
            .node(*self.nodes_by_alias.get(&handle.alias)?)?;
        let port = node.port_by_selector(handle.selector)?;
        if port.kind != handle.kind {
            return None;
        }
        let binding = self
            .editor
            .coordinator()
            .accepted_materialization()?
            .ownership
            .port(port.as_ref(node.id))?;
        // Public ports include aliased operands. Browsing a generated output
        // must not confer selection or mutation authority over those inputs.
        let owned = self.owned_bindings(node.id);
        (owned.binary_search(&binding).is_ok() || matches!(binding,
            IntentNativeBinding::CurveSpan(span) if owned.binary_search(&IntentNativeBinding::Curve(span.curve)).is_ok()
        )).then_some(binding)
    }
}

fn source_owner(
    address: &GeneratedMemberAddress,
    fallback: &SemanticSymbol,
    declarations: &BTreeMap<SemanticSymbol, BTreeSet<NodeId>>,
) -> SemanticSymbol {
    let invocation = SemanticSymbol(address.invocation.clone());
    if declarations.contains_key(&invocation) {
        invocation
    } else {
        fallback.clone()
    }
}

fn generated_member_nodes(
    address: &GeneratedMemberAddress,
    resolver: &NavigationBindingResolver<'_>,
    visiting: &mut BTreeSet<GeneratedMemberAddress>,
) -> BTreeSet<NodeId> {
    if !visiting.insert(address.clone()) {
        return BTreeSet::new();
    }
    let mut nodes = BTreeSet::new();
    // Host-only Fillets must not acquire the input corner in their provenance.
    if let Some(hosts) = resolver.host_nodes.get(address) {
        nodes.extend(hosts);
    } else if let Some(provenance) = resolver.expansion.generated_provenance.get(address) {
        collect_target_nodes(&provenance.target, resolver, visiting, &mut nodes);
    }
    if let Some(accepted) = resolver.editor.coordinator().accepted_materialization() {
        for binding in resolver.member_aggregate_bindings(address) {
            if let IntentNativeBinding::CurveSpan(span) = binding {
                nodes.extend(
                    accepted
                        .ownership
                        .exact_owner(IntentNativeBinding::Curve(span.curve)),
                );
            }
        }
    }
    visiting.remove(address);
    nodes
}

fn collect_target_nodes(
    target: &ExpandedSemanticTarget,
    resolver: &NavigationBindingResolver<'_>,
    visiting: &mut BTreeSet<GeneratedMemberAddress>,
    nodes: &mut BTreeSet<NodeId>,
) {
    let alias = match target {
        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias),
        ExpandedSemanticTarget::Port { port } => Some(&port.alias),
        ExpandedSemanticTarget::FeatureCorner { corner } => Some(&corner.point.alias),
        ExpandedSemanticTarget::Collection { members } => {
            for member in members.values() {
                collect_target_nodes(member, resolver, visiting, nodes);
            }
            None
        }
        ExpandedSemanticTarget::HostOutput { address, .. } => {
            nodes.extend(generated_member_nodes(address, resolver, visiting));
            None
        }
    };
    if let Some(alias) = alias
        && let Some(node) = resolver.nodes_by_alias.get(alias)
    {
        nodes.insert(*node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CodeProject, CompiledManagedSource, MaterializedCodeProject, ProjectKey};
    fn compiled_fixture(bytes: &str) -> CompiledManagedSource {
        CompiledManagedSource::from_json(bytes).expect("checked compiler fixture")
    }
    fn open_project(project: &CodeProject) -> MaterializedCodeProject {
        let generated = KeyedReconcileState::empty()
            .plan(
                crate::required_generated_members(project).unwrap(),
                &BTreeSet::new(),
            )
            .unwrap()
            .into_staged();
        crate::materialize_code_project_cold(
            project,
            &generated,
            geosolve_sketch_intent::IntentSessionId::from_raw(0x9900_0101),
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x9900_0101)),
            1.0,
        )
        .expect("independently accepted exact navigation fixture")
    }
    fn open_compiled(key: &str, compiled: CompiledManagedSource) -> MaterializedCodeProject {
        open_project(&CodeProject::managed(ProjectKey(key.into()), compiled).unwrap())
    }
    #[test]
    fn navigation_generated_aggregate_excludes_spans_borrowed_from_another_declaration() {
        let compiled = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
        )));
        let materialized = open_compiled("m96-borrowed-aggregate-navigation", compiled);
        let editor = &materialized.editor;
        let expansion = &materialized.expansion;
        let graph = editor.coordinator().intent().graph();
        let nodes_by_alias = graph
            .nodes()
            .values()
            .map(|node| (&node.symbol, node.id))
            .collect();
        let resolver = NavigationBindingResolver {
            editor,
            expansion,
            nodes_by_alias: &nodes_by_alias,
            host_nodes: &BTreeMap::new(),
        };
        let mut aggregates = 0;
        for aggregate in &editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .aggregates
        {
            let node = graph.node(aggregate.port.node).unwrap();
            let owner = &expansion.declaration_provenance[&node.symbol];
            let target = ExpandedSemanticTarget::Declaration {
                alias: node.symbol.clone(),
                kind: geosolve_sketch_code::FeatureKind::Feature,
            };
            let mut bindings = BTreeSet::new();
            resolver.collect_aggregate_bindings(&target, &owner.0, &mut bindings);
            assert!(
                bindings.is_empty(),
                "aggregate membership does not confer ownership of its input curves"
            );
            aggregates += 1;
        }
        assert!(aggregates > 0);
    }

    #[test]
    fn navigation_target_collection_recurses_all_members_and_deduplicates_aliases() {
        let materialized = open_project(
            &crate::managed_regression_projects::managed_regression_project("typed-panel").unwrap(),
        );
        let editor = &materialized.editor;
        let expansion = &materialized.expansion;
        let nodes_by_alias = editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .map(|node| (&node.symbol, node.id))
            .collect::<BTreeMap<_, _>>();
        let targets = nodes_by_alias
            .keys()
            .take(2)
            .map(|alias| ExpandedSemanticTarget::Declaration {
                alias: (**alias).clone(),
                kind: geosolve_sketch_code::FeatureKind::Feature,
            })
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 2);
        let collection = ExpandedSemanticTarget::Collection {
            members: BTreeMap::from([
                ("first".into(), Box::new(targets[0].clone())),
                (
                    "nested".into(),
                    Box::new(ExpandedSemanticTarget::Collection {
                        members: BTreeMap::from([
                            ("second".into(), Box::new(targets[1].clone())),
                            ("duplicate".into(), Box::new(targets[0].clone())),
                        ]),
                    }),
                ),
            ]),
        };
        let mut actual = BTreeSet::new();
        collect_target_nodes(
            &collection,
            &NavigationBindingResolver {
                editor,
                expansion,
                nodes_by_alias: &nodes_by_alias,
                host_nodes: &BTreeMap::new(),
            },
            &mut BTreeSet::new(),
            &mut actual,
        );
        let expected = targets
            .iter()
            .filter_map(|target| match target {
                ExpandedSemanticTarget::Declaration { alias, .. } => {
                    nodes_by_alias.get(alias).copied()
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(expected.len(), 2);
        assert_eq!(actual, expected);
    }

    #[test]
    fn navigation_binding_resolver_excludes_borrowed_native_input_ports() {
        let compiled = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
        )));
        let materialized = open_compiled("m95-borrowed-navigation", compiled);
        let editor = &materialized.editor;
        let expansion = &materialized.expansion;
        let graph = editor.coordinator().intent().graph();
        let nodes_by_alias = graph
            .nodes()
            .values()
            .map(|node| (&node.symbol, node.id))
            .collect();
        let resolver = NavigationBindingResolver {
            editor,
            expansion,
            nodes_by_alias: &nodes_by_alias,
            host_nodes: &BTreeMap::new(),
        };
        let ownership = &editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership;
        let mut borrowed = 0;
        for node in graph.nodes().values() {
            let owned = ownership
                .node(node.id)
                .map_or(&[][..], |owner| owner.owned.as_slice());
            for port in node.ports.values() {
                let Some(binding) = ownership.port(port.as_ref(node.id)) else {
                    continue;
                };
                if owned.contains(&binding)
                    || matches!(binding,
                        IntentNativeBinding::CurveSpan(span) if owned.contains(&IntentNativeBinding::Curve(span.curve))
                    )
                {
                    continue;
                }
                let handle = ExpandedPort {
                    alias: node.symbol.clone(),
                    selector: port.selector,
                    kind: port.kind,
                };
                assert!(resolver.port_binding(&handle).is_none());
                borrowed += 1;
            }
        }
        assert!(
            borrowed > 0,
            "the segment start exposes a borrowed producer point"
        );
    }
}
