// SPDX-License-Identifier: GPL-3.0-or-later
//! Cross-view browsing over accepted native ownership and managed provenance.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_NAVIGATION_INSTANCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct NavigationState {
    instance: u64,
    cache: Option<NavigationIndex>,
    explicit_rows: BTreeSet<String>,
    remembered_items: Vec<SelectionItem>,
    remembered_owner: Option<NodeId>,
    notice: Option<String>,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            instance: NEXT_NAVIGATION_INSTANCE.fetch_add(1, Ordering::Relaxed),
            cache: None,
            explicit_rows: BTreeSet::new(),
            remembered_items: Vec::new(),
            remembered_owner: None,
            notice: None,
        }
    }
}

struct NavigationIndex {
    key: String,
    authority: String,
    source: Option<String>,
    unavailable_reason: Option<String>,
    entries: BTreeMap<String, NavigationEntry>,
    explorer: Vec<ExplorerSnapshot>,
}

#[derive(Clone, Default)]
struct NavigationEntry {
    label: String,
    nodes: BTreeSet<NodeId>,
    items: BTreeSet<SelectionItem>,
    sources: BTreeSet<SelectionSourceSnapshot>,
    children: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NavigationSnapshot {
    authority: String,
    selection_key: String,
    rows: Vec<NavigationRowSnapshot>,
    sources: Vec<SelectionSourceSnapshot>,
    item_count: usize,
    can_navigate_source: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notice: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct NavigationRowSnapshot {
    id: String,
    state: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RowRequest {
    authority: String,
    ids: Vec<String>,
    mode: NavigationMode,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum NavigationMode {
    Replace,
    Toggle,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRequest {
    authority: String,
    path: String,
    from: usize,
    to: usize,
}

impl WorkbenchBridge {
    fn navigation_cache_key(&self) -> String {
        // All values are small authority tokens or bounded presentation state.
        // Camera and selection are deliberately absent from the index lifetime.
        serde_json::json!({
            "instance": self.navigation.instance,
            "pending": self.pending_managed_mutation.is_some(),
            "intent": self.editor().coordinator().intent().identity(),
            "code": self.code_project.as_ref().map(CodeProjectWorkbench::code_session_identity),
            "dirty": self.code_project.as_ref().is_some_and(CodeProjectWorkbench::is_dirty),
            "hidden": self.explorer_visibility.hidden_rows,
            "visibility": format!("{:?}", self.editor().editor().geometry_interaction_policy().visibility),
        }).to_string()
    }

    pub(super) fn ensure_navigation_index(&mut self) {
        let key = self.navigation_cache_key();
        if self
            .navigation
            .cache
            .as_ref()
            .is_some_and(|cache| cache.key == key)
        {
            return;
        }
        self.refresh_current_scene();
        let mut entries = BTreeMap::new();
        let (source, source_digest, unavailable_reason) = if let Some(code) = &self.code_project {
            let index = code.navigation_index(self.editor());
            for entry in index.entries {
                let items = entry.exact_bindings.map_or_else(
                    || {
                        self.editor()
                            .navigation_selection_items(entry.nodes.iter().copied())
                    },
                    |bindings| {
                        self.editor()
                            .navigation_selection_items_for_bindings(bindings)
                    },
                );
                entries.insert(
                    entry.id,
                    NavigationEntry {
                        label: String::new(),
                        items: items.into_iter().collect(),
                        nodes: entry.nodes.into_iter().collect(),
                        sources: BTreeSet::from([SelectionSourceSnapshot {
                            path: "sketch.ts".into(),
                            from: entry.source_start,
                            to: entry.source_end,
                        }]),
                        children: Vec::new(),
                    },
                );
            }
            (
                Some(index.source),
                Some(index.source_digest),
                index.blocked_reason,
            )
        } else {
            for cell in self.editor().workbench_projection().outline {
                for declaration in cell.declarations {
                    entries.insert(
                        intent_panel_row_id(&declaration.symbol),
                        NavigationEntry {
                            nodes: BTreeSet::from([declaration.node]),
                            items: self
                                .editor()
                                .navigation_selection_items([declaration.node])
                                .into_iter()
                                .collect(),
                            ..NavigationEntry::default()
                        },
                    );
                }
            }
            (
                None,
                None,
                Some("This project has no managed source statements".into()),
            )
        };
        let explorer = self.explorer_snapshot();
        complete_hierarchy(&explorer, &mut entries);
        let policy = self.editor().editor().geometry_interaction_policy();
        for entry in entries.values_mut() {
            entry.items =
                canonical_selection(entry.items.iter().copied(), self.retained_scene.as_ref());
            entry.items.retain(|item| {
                self.retained_scene
                    .as_ref()
                    .is_some_and(|scene| item_is_visible(scene, *item, policy))
            });
        }
        let authority = geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({"key":key, "source":source_digest})
                .to_string()
                .as_bytes(),
        )
        .to_string();
        self.navigation.cache = Some(NavigationIndex {
            key,
            authority,
            source,
            unavailable_reason,
            entries,
            explorer,
        });
        self.navigation.explicit_rows.clear();
        self.navigation.notice = None;
        self.remember_navigation_selection();
    }

    fn remember_navigation_selection(&mut self) {
        self.navigation.remembered_items = self.editor().editor().selection().to_vec();
        self.navigation.remembered_owner = self.editor().selected_declaration();
    }

    pub(super) fn navigation_snapshot(&mut self) -> NavigationSnapshot {
        self.ensure_navigation_index();
        if self.navigation.remembered_items != self.editor().editor().selection()
            || self.navigation.remembered_owner != self.editor().selected_declaration()
        {
            self.navigation.explicit_rows.clear();
            self.navigation.notice = None;
            self.remember_navigation_selection();
        }
        let cache = self
            .navigation
            .cache
            .as_ref()
            .expect("navigation index installed");
        let selected = canonical_selection(
            self.editor().editor().selection().iter().copied(),
            self.retained_scene.as_ref(),
        );
        let owners = if self.navigation.explicit_rows.is_empty() {
            self.editor()
                .selected_declaration()
                .into_iter()
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let mut rows = Vec::new();
        let mut sources = BTreeSet::new();
        collect_navigation_rows(
            &cache.explorer,
            &cache.entries,
            &selected,
            &owners,
            &self.navigation.explicit_rows,
            &mut rows,
            &mut sources,
        );
        let can_navigate_source = cache.source.is_some() && cache.unavailable_reason.is_none();
        if !can_navigate_source {
            sources.clear();
        }
        let sources = sources.into_iter().collect::<Vec<_>>();
        // Durable point movement changes authority, but not this selection key.
        let selection_key = geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({"instance":self.navigation.instance,"rows":rows,"items":format!("{selected:?}")})
                .to_string()
                .as_bytes(),
        )
        .to_string();
        NavigationSnapshot {
            authority: cache.authority.clone(),
            selection_key,
            rows,
            sources,
            item_count: selected.len(),
            can_navigate_source,
            unavailable_reason: cache.unavailable_reason.clone(),
            notice: self.navigation.notice.clone(),
        }
    }

    pub(super) fn navigation_explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        fn mark(rows: &mut [ExplorerSnapshot], target: Option<&str>) {
            for row in rows {
                row.selected =
                    row.row_kind != ExplorerRowKind::Group && Some(row.id.as_str()) == target;
                mark(&mut row.children, target);
            }
        }

        let mut rows = self
            .navigation
            .cache
            .as_ref()
            .expect("navigation index installed")
            .explorer
            .clone();
        let target = self.navigation_inspector_row_id();
        mark(&mut rows, target);
        rows
    }

    fn navigation_inspector_row_id(&self) -> Option<&str> {
        let selected = self.editor().selected_declaration();
        let cache = self.navigation.cache.as_ref().unwrap();
        let items = canonical_selection(
            self.editor().editor().selection().iter().copied(),
            self.retained_scene.as_ref(),
        );
        // Only one row exposes mutation controls. Exact siblings may share a
        // declaration identity, but they must never all become Inspector targets.
        let empty_generated_count = self
            .navigation
            .explicit_rows
            .iter()
            .filter(|id| {
                id.starts_with("generated:")
                    && cache
                        .entries
                        .get(*id)
                        .is_some_and(|entry| entry.items.is_empty())
            })
            .count();
        selected.and_then(|node| {
            cache
                .entries
                .iter()
                .filter(|(_, entry)| entry.nodes.len() == 1 && entry.nodes.contains(&node))
                .filter(|(id, entry)| {
                    (id.starts_with("managed:")
                        || id.starts_with("generated:")
                        || id.starts_with("intent:"))
                        && items.is_subset(&entry.items)
                        && !(empty_generated_count > 1 && id.starts_with("generated:"))
                })
                .min_by_key(|(id, entry)| {
                    (
                        entry.items.len(),
                        !self.navigation.explicit_rows.contains(*id),
                        !id.starts_with("managed:"),
                        *id,
                    )
                })
                .map(|(id, _)| id.as_str())
        })
    }

    pub(super) fn navigation_selection_snapshot(
        &self,
        navigation: &NavigationSnapshot,
    ) -> Option<SelectionSnapshot> {
        if self.editor().selected_declaration().is_none()
            && (navigation.item_count > 1 || navigation.rows.len() > 1)
        {
            return None;
        }
        let mut selection = self.selection_snapshot()?;
        if let Some(id) = self.navigation_inspector_row_id() {
            selection
                .label
                .clone_from(&self.navigation.cache.as_ref()?.entries.get(id)?.label);
        }
        if let Some(source) = navigation.sources.first() {
            selection.source = Some(source.clone());
        }
        Some(selection)
    }

    fn validate_navigation_authority(&mut self, authority: &str) -> Result<(), String> {
        if self.captured_pointer.is_some()
            || self.pending_managed_mutation.is_some()
            || self.editor().editor().active_pointer_gesture().is_some()
            || self.active_tool != "select"
            || self.authoring.active_tool().is_some()
            || self.feature_authoring.active_tool().is_some()
            || self.offset_authoring.is_active()
            || self.editor().editor().geometry_draft_status().is_some()
        {
            return Err(
                "Finish the current tool or gesture before navigating between views".into(),
            );
        }
        self.ensure_navigation_index();
        if self.navigation.cache.as_ref().unwrap().authority != authority {
            return Err("Navigation belongs to stale source or scene authority".into());
        }
        Ok(())
    }

    pub(super) fn select_navigation_rows_json(
        &mut self,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let request: RowRequest = decode_payload(value)?;
        self.validate_navigation_authority(&request.authority)?;
        self.select_navigation_rows(&request.ids, request.mode)
    }

    pub(super) fn select_navigation_rows_legacy(&mut self, id: &str) -> Result<(), String> {
        self.ensure_navigation_index();
        let authority = self.navigation.cache.as_ref().unwrap().authority.clone();
        self.validate_navigation_authority(&authority)?;
        self.select_navigation_rows(&[id.to_owned()], NavigationMode::Replace)
    }

    fn select_navigation_rows(
        &mut self,
        ids: &[String],
        mode: NavigationMode,
    ) -> Result<(), String> {
        if ids.is_empty() || ids.len() > MAX_VISIBILITY_ROWS {
            return Err("Navigation requires a bounded nonempty row selection".into());
        }
        let cache = self
            .navigation
            .cache
            .as_ref()
            .expect("navigation index installed");
        let mut nodes = BTreeSet::new();
        let mut requested_items = BTreeSet::new();
        for id in ids {
            let entry = cache
                .entries
                .get(id)
                .ok_or("The navigation row is unavailable or stale")?;
            nodes.extend(&entry.nodes);
            requested_items.extend(&entry.items);
        }
        let current = self.navigation_snapshot();
        let remove = matches!(mode, NavigationMode::Toggle)
            && ids.iter().all(|id| {
                current
                    .rows
                    .iter()
                    .any(|row| &row.id == id && row.state == "selected")
            });
        let previous = canonical_selection(
            self.editor().editor().selection().iter().copied(),
            self.retained_scene.as_ref(),
        )
        .into_iter()
        .collect::<Vec<_>>();
        if !self
            .editor_mut()
            .select_navigation_declarations(nodes, Modifiers::default())
        {
            return Err("The navigation output no longer matches accepted authority".into());
        }
        let logical = self.editor().selected_declaration();
        let target = requested_items;
        let mut kept = if matches!(mode, NavigationMode::Toggle) {
            previous
        } else {
            Vec::new()
        };
        if remove {
            kept.retain(|item| !target.contains(item));
        } else {
            for item in target {
                if !kept.contains(&item) {
                    kept.push(item);
                }
            }
        }
        self.editor_mut().set_selection(kept);
        if logical.is_none() && matches!(mode, NavigationMode::Replace) {
            self.editor_mut().set_selected_declaration(None);
        } else if !remove && self.editor().editor().selection().is_empty() {
            let _ = self.editor_mut().set_selected_declaration(logical);
        }
        if matches!(mode, NavigationMode::Replace) {
            self.navigation.explicit_rows.clear();
        }
        let cache = self.navigation.cache.as_ref().unwrap();
        let targets = ids
            .iter()
            .flat_map(|id| std::iter::once(id.clone()).chain(cache.entries[id].children.clone()))
            .collect::<BTreeSet<_>>();
        if remove {
            self.navigation.explicit_rows.retain(|id| {
                !targets.contains(id)
                    && !cache.entries[id]
                        .children
                        .iter()
                        .any(|child| targets.contains(child))
            });
        } else {
            self.navigation.explicit_rows.extend(targets);
        }
        self.reconcile_navigation_logical_owner();
        self.navigation.notice = None;
        self.remember_navigation_selection();
        Ok(())
    }

    fn reconcile_navigation_logical_owner(&mut self) {
        let cache = self
            .navigation
            .cache
            .as_ref()
            .expect("navigation index installed");
        let logical_owners = self
            .navigation
            .explicit_rows
            .iter()
            .filter_map(|id| cache.entries.get(id))
            .filter(|entry| entry.items.is_empty())
            .flat_map(|entry| entry.nodes.iter().copied())
            .collect::<BTreeSet<_>>();
        if !logical_owners.is_empty() {
            let owner = self.editor().selected_declaration();
            if logical_owners.len() > 1
                || owner.is_some_and(|owner| !logical_owners.contains(&owner))
            {
                self.editor_mut().set_selected_declaration(None);
            } else if self.editor().editor().selection().is_empty() {
                self.editor_mut()
                    .set_selected_declaration(logical_owners.iter().next().copied());
            }
        }
    }

    pub(super) fn select_navigation_source_json(
        &mut self,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let request: SourceRequest = decode_payload(value)?;
        self.validate_navigation_authority(&request.authority)?;
        let cache = self.navigation.cache.as_ref().unwrap();
        if let Some(reason) = &cache.unavailable_reason {
            return Err(reason.clone());
        }
        if request.path != "sketch.ts" {
            return Err("This file has no accepted sketch statements".into());
        }
        let source = cache
            .source
            .as_deref()
            .ok_or("No accepted source is available")?;
        if request.from > request.to
            || request.to > source.len()
            || !source.is_char_boundary(request.from)
            || !source.is_char_boundary(request.to)
        {
            return Err("Source navigation requires exact UTF-8 character boundaries".into());
        }
        let ids = cache
            .entries
            .iter()
            .filter(|(id, entry)| {
                id.starts_with("managed:")
                    && entry.sources.iter().any(|span| {
                        if request.from == request.to {
                            span.from <= request.from && request.from < span.to
                        } else {
                            request.from < span.to && span.from < request.to
                        }
                    })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            self.navigation.notice = Some("No sketch object here".into());
            return Ok(());
        }
        self.select_navigation_rows(&ids, NavigationMode::Replace)
    }

    pub(super) fn navigation_update_json(&mut self) -> Result<String, String> {
        fn selected_rows(rows: &[ExplorerSnapshot], selected: &mut Vec<String>) {
            for row in rows {
                if row.selected {
                    selected.push(row.id.clone());
                }
                selected_rows(&row.children, selected);
            }
        }

        let frame = self.frame_snapshot();
        if self.last_error.is_some() {
            return self.snapshot_json();
        }
        let navigation = self.navigation_snapshot();
        let selection = self.navigation_selection_snapshot(&navigation);
        let selected_geometry_role = self
            .editor()
            .selected_geometry_role_state()
            .ok()
            .flatten()
            .map(|state| match state {
                GeometryRoleSelectionState::Profile => "profile",
                GeometryRoleSelectionState::Construction => "construction",
                GeometryRoleSelectionState::Mixed => "mixed",
            });
        let explorer = self.navigation_explorer_snapshot();
        let mut selected_declarations = Vec::new();
        selected_rows(&explorer, &mut selected_declarations);
        serde_json::to_string(&serde_json::json!({
            "version":PROTOCOL_VERSION, "kind":"selection", "revision":self.revision,
            "frame":frame, "navigation":navigation, "selectedDeclarations":selected_declarations,
            "selection":selection, "selectedGeometryRole":selected_geometry_role,
        }))
        .map_err(|error| error.to_string())
    }
}

fn complete_hierarchy(rows: &[ExplorerSnapshot], entries: &mut BTreeMap<String, NavigationEntry>) {
    for row in rows {
        complete_hierarchy(&row.children, entries);
        let children = row
            .children
            .iter()
            .filter_map(|child| entries.get(&child.id).cloned())
            .collect::<Vec<_>>();
        let entry = entries.entry(row.id.clone()).or_default();
        entry.label.clone_from(&row.label);
        entry.children = row.children.iter().map(|child| child.id.clone()).collect();
        for child in children {
            entry.children.extend(child.children);
            entry.nodes.extend(child.nodes);
            entry.items.extend(child.items);
            // Source spans describe direct statement ownership. Descendant
            // statements are collected only when those descendants are selected.
        }
    }
}

fn collect_navigation_rows(
    rows: &[ExplorerSnapshot],
    entries: &BTreeMap<String, NavigationEntry>,
    selected: &BTreeSet<SelectionItem>,
    owners: &BTreeSet<NodeId>,
    explicit: &BTreeSet<String>,
    result: &mut Vec<NavigationRowSnapshot>,
    sources: &mut BTreeSet<SelectionSourceSnapshot>,
) {
    for row in rows {
        let Some(entry) = entries.get(&row.id) else {
            continue;
        };
        let exact = explicit.contains(&row.id);
        let owned = entry.items.intersection(selected).count();
        let logical =
            selected.is_empty() && explicit.is_empty() && !entry.nodes.is_disjoint(owners);
        let missing_logical_child = entry.children.iter().any(|id| {
            entries.get(id).is_some_and(|child| {
                child.items.is_empty()
                    && !explicit.contains(id)
                    && !(selected.is_empty()
                        && explicit.is_empty()
                        && !child.nodes.is_disjoint(owners))
            })
        });
        let state = if ((exact && entry.items.is_empty())
            || (owned > 0 && owned == entry.items.len())
            || logical)
            && !missing_logical_child
        {
            Some("selected")
        } else if owned > 0 || logical || entry.children.iter().any(|id| explicit.contains(id)) {
            Some("partial")
        } else {
            None
        };
        if let Some(state) = state {
            result.push(NavigationRowSnapshot {
                id: row.id.clone(),
                state,
            });
            if exact || row.row_kind != ExplorerRowKind::Group {
                sources.extend(entry.sources.iter().cloned());
            }
        }
        collect_navigation_rows(
            &row.children,
            entries,
            selected,
            owners,
            explicit,
            result,
            sources,
        );
    }
}

fn item_is_visible(
    scene: &EditorScene,
    item: SelectionItem,
    policy: geosolve_constraint_editor::GeometryInteractionPolicy,
) -> bool {
    match item {
        SelectionItem::Point(id) => scene
            .points
            .iter()
            .any(|point| point.id == id && point.is_visible(policy)),
        SelectionItem::Curve(span) => scene
            .curves
            .iter()
            .any(|curve| curve.span == span && curve.is_visible(policy)),
        SelectionItem::Constraint(_) | SelectionItem::Dimension(_) => {
            scene.annotations_visible
                && scene
                    .annotations
                    .iter()
                    .any(|annotation| annotation.item == item)
        }
        SelectionItem::Feature(id) => scene
            .computed_curves
            .iter()
            .any(|curve| curve.owner.feature == id && curve.is_visible(policy)),
        SelectionItem::FeatureCorner(owner) => scene
            .computed_curves
            .iter()
            .any(|curve| curve.owner == owner && curve.is_visible(policy)),
        SelectionItem::Datum(_) => true,
    }
}

/// `Feature` and `FeatureCorner` are alternate native selection representations.
/// Coverage and toggles use the exact visible corner set, so a picked Fillet
/// cannot appear partially selected merely because an aggregate alias exists.
fn canonical_selection(
    items: impl IntoIterator<Item = SelectionItem>,
    scene: Option<&EditorScene>,
) -> BTreeSet<SelectionItem> {
    let mut canonical = BTreeSet::new();
    for item in items {
        if let SelectionItem::Feature(feature) = item {
            let corners = scene
                .into_iter()
                .flat_map(|scene| &scene.computed_curves)
                .filter(|curve| curve.owner.feature == feature)
                .map(|curve| SelectionItem::FeatureCorner(curve.owner))
                .collect::<BTreeSet<_>>();
            if !corners.is_empty() {
                canonical.extend(corners);
                continue;
            }
        }
        canonical.insert(item);
    }
    canonical
}
