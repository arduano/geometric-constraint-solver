// SPDX-License-Identifier: GPL-3.0-or-later
//! One source insertion path for native authoring in every host.
//! Source names, namespace ordering and authored dimension intent belong to the source owner.
use crate::{
    CodeProject, EditorBootstrapDeclaration, SemanticSymbol, direct_declaration_intent_symbol,
};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentNode, IntentNodeKind};
use std::collections::{BTreeMap, BTreeSet};

const fn managed_canvas_name_base(kind: &IntentNodeKind) -> &'static str {
    match kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        } => "segment",
        IntentNodeKind::Geometry { .. } => "geometry",
        IntentNodeKind::Constraint { .. } => "constraint",
        IntentNodeKind::Dimension { .. } => "dimension",
        IntentNodeKind::Operation { .. } => "operation",
        IntentNodeKind::ComputedFeature { .. } => "feature",
        IntentNodeKind::Aggregate { .. } => "aggregate",
        IntentNodeKind::Parameter { .. } => "parameter",
        IntentNodeKind::External { .. } => "external",
        IntentNodeKind::Bootstrap { .. } => "bootstrap",
        IntentNodeKind::Annotation => "annotation",
        IntentNodeKind::Identity { .. } => "identity",
    }
}

const fn managed_canvas_namespace(kind: &IntentNodeKind) -> Option<&'static str> {
    match kind {
        IntentNodeKind::Geometry { .. } => Some("geometry"),
        IntentNodeKind::Constraint { .. } => Some("constraint"),
        IntentNodeKind::Dimension { .. } => Some("dimension"),
        IntentNodeKind::Operation { .. } => Some("operation"),
        IntentNodeKind::ComputedFeature { .. } => Some("computed"),
        IntentNodeKind::Aggregate { .. } => Some("aggregate"),
        IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => None,
    }
}

fn allocate_canvas_declaration_names(
    project: &CodeProject,
    nodes: &[&IntentNode],
    current_high_water: u64,
) -> Result<(Vec<EditorBootstrapDeclaration>, u64), String> {
    let mut occupied = project
        .managed
        .program
        .declarations
        .iter()
        .flat_map(|declaration| [declaration.variable.clone(), declaration.symbol.0.clone()])
        .chain(
            project
                .managed
                .program
                .scalar_bindings
                .iter()
                .map(|binding| binding.variable.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut high_water = current_high_water;
    let mut declarations = Vec::with_capacity(nodes.len());
    for node in nodes {
        let symbol = loop {
            high_water = high_water
                .checked_add(1)
                .filter(|value| *value <= crate::MAX_CODE_SESSION_WIRE_INTEGER)
                .ok_or_else(|| "the managed declaration-name allocator is exhausted".to_owned())?;
            let candidate = format!("{}{high_water}", managed_canvas_name_base(&node.kind));
            if occupied.insert(candidate.clone()) {
                break candidate;
            }
        };
        declarations.push(EditorBootstrapDeclaration::new(
            node.id,
            SemanticSymbol(symbol),
        ));
    }

    // One unordered Intent patch allocates simultaneously ready declarations
    // by their durable native symbol, not by source-array order. GUI symbols
    // and managed declaration symbols intentionally use different spellings.
    // For declarations from the same authoring namespace/name family, assign
    // the already reserved monotonic names in their eventual native-symbol
    // order so a multi-node gesture retains the exact candidate identities.
    let mut families = BTreeMap::<(&str, &str), Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(namespace) = managed_canvas_namespace(&node.kind) else {
            continue;
        };
        families
            .entry((namespace, managed_canvas_name_base(&node.kind)))
            .or_default()
            .push(index);
    }
    for ((namespace, _), indices) in families {
        if indices.len() < 2 {
            continue;
        }
        let mut symbols = indices
            .iter()
            .map(|index| {
                let symbol = declarations[*index].symbol.clone();
                direct_declaration_intent_symbol(&project.project, namespace, &symbol)
                    .map(|intent| (intent, symbol))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        symbols.sort_by(|left, right| left.0.cmp(&right.0));
        for (index, (_, symbol)) in indices.into_iter().zip(symbols) {
            declarations[index].symbol = symbol;
        }
    }
    Ok((declarations, high_water))
}

/// Source declarations prepared from exactly the nodes added by one native transaction.
/// This is source projection data, not a compiler receipt or publication capability.
#[derive(Clone, Debug)]
pub struct PreparedEditorSourceInsertion {
    declarations: Vec<EditorBootstrapDeclaration>,
    high_water: u64,
    drafts: Vec<crate::ManagedDeclarationDraft>,
}

impl PreparedEditorSourceInsertion {
    /// Consumes the source projection into native/source correspondence, allocator and drafts.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<EditorBootstrapDeclaration>,
        u64,
        Vec<crate::ManagedDeclarationDraft>,
    ) {
        (self.declarations, self.high_water, self.drafts)
    }
}

/// Projects all and only the declarations added by an accepted native authoring transaction.
/// Names preserve native allocation order; authored dimensions retain their overview intent.
/// A host must still authenticate a compiler receipt and terminal parity before publication.
///
/// # Errors
/// Rejects empty/oversized additions, exhausted allocation, unsupported source projection,
/// or stale source/expansion identity without changing either editor or the source project.
pub fn prepare_editor_source_insertion(
    project: &CodeProject,
    expansion: &crate::ExpandedCodeProject,
    accepted: &geosolve_constraint_editor::ProjectionalEditorSession,
    candidate: &geosolve_constraint_editor::ProjectionalEditorSession,
) -> Result<PreparedEditorSourceInsertion, String> {
    if let Some(receipt) = candidate.completed_construction() {
        let intent = accepted.coordinator().intent();
        if receipt.origin() != intent.identity() {
            // Native preparation forks preserve the exact accepted graph/namespace but remove
            // nested history. Authenticate that exact delegated identity, not a partial digest.
            let delegated = intent
                .delegated_checkpoint()
                .map_err(|error| error.to_string())?;
            if receipt.origin() != delegated.identity() {
                return Err(
                    "completed construction belongs to another accepted source authority".into(),
                );
            }
        }
    }
    let accepted_nodes = accepted.coordinator().intent().graph().nodes();
    let added = candidate
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter(|node| !accepted_nodes.contains_key(&node.id))
        .collect::<Vec<_>>();
    if added.is_empty() || added.len() > crate::MANAGED_MUTATION_BATCH_LIMIT {
        return Err(
            "native authoring declaration count is outside the managed batch bounds".into(),
        );
    }
    let (declarations, high_water) = allocate_canvas_declaration_names(
        project,
        &added,
        project.managed.declaration_name_high_water,
    )?;
    let insertion = crate::prepare_editor_declaration_insertions(
        project,
        expansion,
        accepted,
        candidate,
        &declarations,
    )
    .map_err(|error| error.to_string())?;
    if insertion.project != project.project
        || insertion.source_digest != project.managed.source_digest
        || insertion.expansion_digest != expansion.digest
    {
        return Err(
            "native source projection belongs to stale source or expansion authority".into(),
        );
    }
    let drafts = insertion
        .declarations
        .into_iter()
        .map(|draft| {
            let mut draft = crate::ManagedDeclarationDraft::from(draft);
            if draft
                .builder_path
                .first()
                .is_some_and(|namespace| namespace == "dimension")
                && let crate::ManagedValue::Object(arguments) = &mut draft.arguments
            {
                arguments.insert("isKeyConstraint".into(), crate::ManagedValue::Bool(true));
            }
            draft
        })
        .collect();
    Ok(PreparedEditorSourceInsertion {
        declarations,
        high_water,
        drafts,
    })
}

/// Draft labels default to private native allocation symbols. Preserve the exact
/// original authenticated display label while allocating latest persistent names.
/// This is restricted to unrenamed newly created nodes; arbitrary metadata never
/// participates in alpha normalization.
pub fn retain_editor_default_labels(
    original_editor: &geosolve_constraint_editor::ProjectionalEditorSession,
    original_declarations: &[crate::EditorBootstrapDeclaration],
    original_drafts: &[crate::ManagedDeclarationDraft],
    latest_editor: &geosolve_constraint_editor::ProjectionalEditorSession,
    latest_declarations: &[crate::EditorBootstrapDeclaration],
    latest_drafts: &mut [crate::ManagedDeclarationDraft],
) {
    if original_drafts.len() != latest_drafts.len() {
        return;
    }
    let default_symbol = |editor: &geosolve_constraint_editor::ProjectionalEditorSession,
                          declarations: &[crate::EditorBootstrapDeclaration],
                          draft: &crate::ManagedDeclarationDraft|
     -> Option<String> {
        let declaration = declarations
            .iter()
            .find(|value| value.symbol.0 == draft.symbol)?;
        let intent = editor.coordinator().intent();
        let node = intent.graph().node(declaration.node)?;
        if intent
            .organization()
            .node_names()
            .get(&declaration.node)
            .is_some_and(|name| name != &node.symbol)
        {
            return None;
        }
        Some(
            intent
                .graph()
                .node(declaration.node)?
                .symbol
                .as_str()
                .to_owned(),
        )
    };
    let old_defaults = original_drafts
        .iter()
        .map(|draft| default_symbol(original_editor, original_declarations, draft))
        .collect::<Vec<_>>();
    let new_defaults = latest_drafts
        .iter()
        .map(|draft| default_symbol(latest_editor, latest_declarations, draft))
        .collect::<Vec<_>>();
    for (((old, new), old_default), new_default) in original_drafts
        .iter()
        .zip(latest_drafts)
        .zip(old_defaults)
        .zip(new_defaults)
    {
        let (Some(old_default), Some(new_default)) = (old_default, new_default) else {
            continue;
        };
        let (crate::ManagedValue::Object(old_fields), crate::ManagedValue::Object(new_fields)) =
            (&old.arguments, &mut new.arguments)
        else {
            continue;
        };
        if old_fields.get("label") == Some(&crate::ManagedValue::String(old_default.clone()))
            && new_fields.get("label") == Some(&crate::ManagedValue::String(new_default))
        {
            new_fields.insert("label".into(), crate::ManagedValue::String(old_default));
        }
    }
}
