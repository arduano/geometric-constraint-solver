// SPDX-License-Identifier: GPL-3.0-or-later
//! Personal visibility uses native declaration ownership for both paint and picking.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VisibilityState {
    pub hidden_rows: BTreeSet<String>,
    pub isolate_restore: Option<BTreeSet<String>>,
    pub construction_visible: bool,
}
impl Default for VisibilityState {
    fn default() -> Self {
        Self {
            hidden_rows: BTreeSet::new(),
            isolate_restore: None,
            construction_visible: true,
        }
    }
}
impl VisibilityState {
    pub fn validate(&self, rows: &BTreeSet<String>) -> Result<(), String> {
        for ids in std::iter::once(&self.hidden_rows).chain(self.isolate_restore.iter()) {
            if ids.len() > MAX_VISIBILITY_ROWS || !ids.is_subset(rows) {
                return Err("Local visibility contains stale or excessive Explorer rows".into());
            }
        }
        Ok(())
    }
    pub fn reconcile(&mut self, seed: &VisibilitySeed) {
        self.hidden_rows.retain(|id| seed.rows.contains_key(id));
        if let Some(restore) = &mut self.isolate_restore {
            restore.retain(|id| seed.rows.contains_key(id));
        }
    }
    pub fn apply_policy(&self, editor: &mut ConstraintEditor) {
        let current = editor.geometry_interaction_policy().visibility;
        let _ = editor.set_geometry_visibility(GeometryVisibility {
            explicit_construction: self.construction_visible,
            implicit_construction: self.construction_visible,
            ..current
        });
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VisibilitySeed {
    rows: BTreeMap<String, VisibilityRow>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VisibilityRow {
    ancestors: Vec<String>,
    items: Vec<SelectionItem>,
    isolate: bool,
}
impl VisibilitySeed {
    pub fn new(bridge: &WorkbenchBridge, scene: &EditorScene) -> Self {
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
        let mut seed = Self::default();
        collect(&bridge.base_explorer_snapshot(), &[], &mut seed.rows);
        for (id, items) in bridge.explorer_scene_items(scene, seed.rows.keys().cloned()) {
            if let Some(row) = seed.rows.get_mut(&id) {
                row.items = items;
            }
        }
        seed
    }
    pub fn validate(&self, state: &VisibilityState) -> Result<(), String> {
        let ids = self.rows.keys().cloned().collect();
        state.validate(&ids)?;
        if self.rows.values().any(|row| {
            row.ancestors.len() > MAX_VISIBILITY_ROWS
                || row.ancestors.iter().any(|id| !self.rows.contains_key(id))
        }) {
            return Err("Local visibility ownership contains an invalid ancestor".into());
        }
        Ok(())
    }
    pub fn hidden_items(&self, state: &VisibilityState) -> BTreeSet<SelectionItem> {
        self.rows
            .iter()
            .filter(|(id, row)| {
                state.hidden_rows.contains(*id)
                    || row
                        .ancestors
                        .iter()
                        .any(|id| state.hidden_rows.contains(id))
            })
            .flat_map(|(_, row)| row.items.iter().copied())
            .collect()
    }
    pub fn mask_prediction(
        &self,
        scene: &mut EditorScene,
        state: &VisibilityState,
        mapping: &PresentationMapping,
    ) -> Result<(), String> {
        self.validate(state)?;
        let hidden = self
            .hidden_items(state)
            .into_iter()
            .map(|item| mapping.selection(item))
            .collect::<Result<Vec<_>, _>>()?;
        scene.hide_items(hidden).map_err(|error| error.to_string())
    }
    pub fn dispatch(
        &self,
        state: &mut VisibilityState,
        request: &CommandRequest,
    ) -> Result<(), String> {
        let mut next = state.clone();
        match request.command.as_str() {
            "view.construction.toggle" => next.construction_visible = !next.construction_visible,
            "explorer.visibility.set" => {
                let payload: ExplorerVisibilityPayload = decode_payload(request.payload.clone())?;
                if !self.rows.contains_key(&payload.id) {
                    return Err("Explorer visibility target is unavailable or stale".into());
                }
                let changed = if payload.visible {
                    next.hidden_rows.remove(&payload.id)
                } else {
                    next.hidden_rows.insert(payload.id)
                };
                if changed {
                    next.isolate_restore = None;
                }
            }
            "explorer.visibility.isolate" => {
                let payload: SelectionPayload = decode_payload(request.payload.clone())?;
                if !self.rows.get(&payload.id).is_some_and(|row| row.isolate) {
                    return Err("Only a current top-level Explorer group can be isolated".into());
                }
                let baseline = next
                    .isolate_restore
                    .clone()
                    .unwrap_or_else(|| next.hidden_rows.clone());
                next.hidden_rows.clone_from(&baseline);
                for (id, row) in &self.rows {
                    if !row.ancestors.is_empty() {
                        continue;
                    }
                    if *id == payload.id {
                        next.hidden_rows.remove(id);
                    } else {
                        next.hidden_rows.insert(id.clone());
                    }
                }
                next.isolate_restore = Some(baseline);
            }
            "explorer.visibility.restore" => {
                next.hidden_rows = next
                    .isolate_restore
                    .take()
                    .ok_or("There is no isolated visibility state to restore")?;
            }
            _ => return Err("Unsupported local visibility command".into()),
        }
        self.validate(&next)?;
        *state = next;
        Ok(())
    }
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
