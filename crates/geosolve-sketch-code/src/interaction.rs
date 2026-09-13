// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact semantic point and peer-selection projections from accepted source ownership.
use crate::editor_terminal::expanded_port_point;
use crate::{CodePointEdit, ExpandedCodeProject, ExpandedWritablePoint, SemanticSymbol};
use geosolve_constraint_editor::{IntentNativeBinding, ProjectionalEditorSession};
use std::collections::{BTreeMap, BTreeSet};

/// Resolves an exact preferred owner, otherwise the unique non-reference producer.
/// # Errors
/// Rejects ambiguous producer or preferred-declaration lenses.
pub fn select_semantic_point_drag_lens(
    expansion: &ExpandedCodeProject,
    candidates: &[ExpandedWritablePoint],
    preferred_declaration: Option<&SemanticSymbol>,
) -> Result<Option<ExpandedWritablePoint>, String> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let preferred = preferred_declaration.map_or_else(Vec::new, |preferred_declaration| {
        candidates
            .iter()
            .filter(|candidate| {
                expansion.declaration_for_alias(&candidate.handle.alias)
                    == Some(preferred_declaration)
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    match preferred.as_slice() {
        [point] => Ok(Some(point.clone())),
        [] => {
            let producers = candidates
                .iter()
                .filter(|candidate| !candidate.source.is_reference())
                .cloned()
                .collect::<Vec<_>>();
            match producers.as_slice() {
                [point] => Ok(Some(point.clone())),
                [] if candidates.len() == 1 => Ok(Some(candidates[0].clone())),
                [] => {
                    Err("this shared code-owned point has no unique producer semantic lens".into())
                }
                _ => {
                    Err("this shared code-owned point has multiple producer semantic lenses".into())
                }
            }
        }
        _ => Err(
            "the selected declaration has multiple semantic point lenses at this shared point"
                .into(),
        ),
    }
}

/// Native points to exact writable semantic lenses. Ambiguous points are unavailable.
#[must_use]
pub fn point_targets(
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
) -> BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for lens in &expansion.writable_points {
        if let Some(point) = expanded_port_point(editor, &lens.handle) {
            groups.entry(point).or_default().push(lens.clone());
        }
    }
    let mut targets = BTreeMap::new();
    for (point, candidates) in groups {
        // Preserve the existing native producer/reference disambiguation.
        // Ambiguity is unavailable, never a nearest-position fallback.
        let Ok(Some(lens)) = select_semantic_point_drag_lens(expansion, &candidates, None) else {
            continue;
        };
        let target = match lens.edit {
            CodePointEdit::Point { address } => {
                serde_json::json!({"target":"point","address":address})
            }
            CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                corner,
                ..
            } => serde_json::json!({
                    "target":"rectangle_corner","lower_left":lower_left,"upper_right":upper_right,"corner":corner}),
        };
        targets.insert(point, target);
    }
    targets
}

/// Accepted source declarations to exact owned native bindings for peer highlights.
#[must_use]
pub fn presence_bindings(
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
) -> BTreeMap<String, Vec<IntentNativeBinding>> {
    let Some(bindings) = editor.presentation_bindings() else {
        return BTreeMap::new();
    };
    let mut by_declaration = BTreeMap::<String, BTreeSet<IntentNativeBinding>>::new();
    for (alias, owned) in bindings.nodes {
        if let Some(declaration) = expansion.declaration_for_alias(&alias) {
            by_declaration
                .entry(declaration.0.clone())
                .or_default()
                .extend(owned);
        }
    }
    by_declaration
        .into_iter()
        .map(|(symbol, owned)| (symbol, owned.into_iter().collect()))
        .collect()
}
