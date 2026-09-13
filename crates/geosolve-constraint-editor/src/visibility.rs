// SPDX-License-Identifier: GPL-3.0-or-later
//! Personal visibility ownership shared by detached selection and rendering hosts.
use crate::{
    ConstraintEditor, EditorScene, GeometryVisibility, PresentationMapping, SelectionItem,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const MAX_VISIBILITY_ROWS: usize = 4_096;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplorerVisibilityPayload {
    id: String,
    visible: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionPayload {
    id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibilityState {
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
    /// Applies the exact native visibility ownership contract.
    /// # Errors
    /// Rejects stale rows, invalid ancestry or unavailable native ownership.
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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibilitySeed {
    rows: BTreeMap<String, VisibilityRow>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibilityRow {
    pub ancestors: Vec<String>,
    pub items: Vec<SelectionItem>,
    pub isolate: bool,
}
impl VisibilitySeed {
    /// Creates visibility ownership from already projected source/native rows.
    #[must_use]
    pub fn from_rows(rows: BTreeMap<String, VisibilityRow>) -> Self {
        Self { rows }
    }
    /// Applies the exact native visibility ownership contract.
    /// # Errors
    /// Rejects stale rows, invalid ancestry or unavailable native ownership.
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
    /// Applies the exact native visibility ownership contract.
    /// # Errors
    /// Rejects stale rows, invalid ancestry or unavailable native ownership.
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
    /// Applies the exact native visibility ownership contract.
    /// # Errors
    /// Rejects stale rows, invalid ancestry or unavailable native ownership.
    pub fn dispatch(
        &self,
        state: &mut VisibilityState,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        let mut next = state.clone();
        match command {
            "view.construction.toggle" => next.construction_visible = !next.construction_visible,
            "explorer.visibility.set" => {
                let payload: ExplorerVisibilityPayload = serde_json::from_value(payload)
                    .map_err(|error| format!("invalid command payload: {error}"))?;
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
                let payload: SelectionPayload = serde_json::from_value(payload)
                    .map_err(|error| format!("invalid command payload: {error}"))?;
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
