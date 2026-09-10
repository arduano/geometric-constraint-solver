// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-name allocation extracted from the existing workbench canvas insertion path.
use geosolve_sketch_code::{
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

pub(super) fn allocate_canvas_declaration_names(
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
                .filter(|value| *value <= geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER)
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
