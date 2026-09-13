// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-tree visibility ownership and host installation.
use super::*;
use geosolve_constraint_editor::VisibilityRow;
pub(super) use geosolve_constraint_editor::{VisibilitySeed, VisibilityState};
use std::collections::BTreeSet;

pub(super) fn visibility_seed(bridge: &WorkbenchBridge, scene: &EditorScene) -> VisibilitySeed {
    VisibilitySeed::from_rows(visibility_rows(&bridge.chrome_read(), scene))
}

pub(in crate::workbench::bridge) fn browsing_visibility_seed(
    read: &ChromeRead<'_>,
    scene: &EditorScene,
    mapping: &PresentationMapping,
) -> Result<VisibilitySeed, String> {
    let mut rows = visibility_rows(read, scene);
    for row in rows.values_mut() {
        row.items = row
            .items
            .iter()
            .map(|item| mapping.selection(*item))
            .collect::<Result<_, _>>()?;
    }
    Ok(VisibilitySeed::from_rows(rows))
}

fn visibility_rows(read: &ChromeRead<'_>, scene: &EditorScene) -> BTreeMap<String, VisibilityRow> {
    fn collect(
        rows: &[ExplorerSnapshot],
        ancestors: &[String],
        map: &mut BTreeMap<String, VisibilityRow>,
    ) {
        for row in rows {
            map.insert(
                row.id.clone(),
                VisibilityRow {
                    ancestors: ancestors.to_vec(),
                    items: Vec::new(),
                    isolate: ancestors.is_empty() && row.row_kind == ExplorerRowKind::Group,
                },
            );
            let mut next = ancestors.to_vec();
            next.push(row.id.clone());
            collect(&row.children, &next, map);
        }
    }
    let mut rows = BTreeMap::new();
    collect(&read.base_explorer_snapshot(), &[], &mut rows);
    for (id, items) in read.explorer_scene_items(scene, rows.keys().cloned()) {
        if let Some(row) = rows.get_mut(&id) {
            row.items = items;
        }
    }
    rows
}
impl WorkbenchBridge {
    pub(super) fn local_visibility_state(&self) -> VisibilityState {
        let policy = self.editor().editor().geometry_interaction_policy();
        VisibilityState {
            hidden_rows: self.explorer_visibility.hidden_rows.clone(),
            isolate_restore: self.explorer_visibility.isolate_restore.clone(),
            construction_visible: policy.visibility.explicit_construction
                && policy.visibility.implicit_construction,
        }
    }
    pub(super) fn validate_local_visibility(&self, state: &VisibilityState) -> Result<(), String> {
        let mut rows = BTreeSet::new();
        collect_explorer_row_ids(&self.base_explorer_snapshot(), &mut rows);
        state.validate(&rows)
    }
    pub(super) fn apply_local_visibility(&mut self, state: &VisibilityState) {
        if *state == self.local_visibility_state() {
            return;
        }
        self.explorer_visibility = ExplorerVisibilityState {
            hidden_rows: state.hidden_rows.clone(),
            isolate_restore: state.isolate_restore.clone(),
        };
        state.apply_policy(self.editor_mut().editor_mut());
        self.invalidate_visibility_presentation();
    }
}
