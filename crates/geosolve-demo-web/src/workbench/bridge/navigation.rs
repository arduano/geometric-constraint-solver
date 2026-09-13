// SPDX-License-Identifier: GPL-3.0-or-later
//! Cross-view browsing over accepted native ownership and managed provenance.

use super::*;
use geosolve_constraint_editor::ProjectionalEditorSession;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_NAVIGATION_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(super) struct NavigationState {
    instance: u64,
    cache: Option<Rc<NavigationIndex>>,
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
    geometry_key: String,
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

/// Personal result of one authenticated navigation command; contains no editing authority.
pub(super) struct PreparedNavigation {
    pub(super) selection: geosolve_constraint_editor::SelectionPresentationState,
    pub(super) logical_owner: Option<NodeId>,
    pub(super) navigation: NavigationState,
}

struct NavigationMutation<'a> {
    editor: &'a mut ProjectionalEditorSession,
    navigation: &'a mut NavigationState,
    scene: Option<&'a EditorScene>,
}

impl NavigationState {
    pub(super) fn geometry_key(&self, read: &ChromeRead<'_>) -> String {
        // All values are small authority tokens or bounded presentation state.
        // Camera and selection are deliberately absent from the index lifetime.
        serde_json::json!({
            "instance": self.instance,
            "intent": read.editor().coordinator().intent().identity(),
            "code": read.code_project.as_ref().map(CodeChrome::code_session_identity),
            "hidden": read.hidden_rows,
            "visibility": format!("{:?}", read.editor().editor().geometry_interaction_policy().visibility),
        }).to_string()
    }

    pub(super) fn needs_scene_refresh(&self, read: &ChromeRead<'_>) -> bool {
        let geometry_key = self.geometry_key(read);
        self.cache
            .as_ref()
            .is_none_or(|cache| cache.geometry_key != geometry_key)
    }

    pub(super) fn snapshot(
        &mut self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
    ) -> NavigationSnapshot {
        self.ensure_index(read, scene);
        self.current_snapshot(read.editor(), scene)
    }

    fn validate_authority(&self, authority: &str) -> Result<(), String> {
        if self
            .cache
            .as_ref()
            .expect("navigation index installed")
            .authority
            != authority
        {
            return Err("Navigation belongs to stale source or scene authority".into());
        }
        Ok(())
    }

    /// Prepares only personal selection and remembered navigation against one accepted fork.
    pub(super) fn prepare_rows_json(
        &self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
        action_blocked: bool,
        value: serde_json::Value,
    ) -> Result<PreparedNavigation, String> {
        let request: RowRequest = decode_payload(value)?;
        self.prepare(
            read,
            scene,
            action_blocked,
            Some(&request.authority),
            |view| view.select_navigation_rows(&request.ids, request.mode),
        )
    }

    pub(super) fn prepare_source_json(
        &self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
        action_blocked: bool,
        value: serde_json::Value,
    ) -> Result<PreparedNavigation, String> {
        let request: SourceRequest = decode_payload(value)?;
        let authority = request.authority.clone();
        self.prepare(read, scene, action_blocked, Some(&authority), |view| {
            view.select_source_request(&request)
        })
    }

    pub(super) fn prepare_rows_legacy(
        &self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
        action_blocked: bool,
        id: &str,
    ) -> Result<PreparedNavigation, String> {
        self.prepare(read, scene, action_blocked, None, |view| {
            view.select_navigation_rows(&[id.to_owned()], NavigationMode::Replace)
        })
    }

    fn prepare(
        &self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
        action_blocked: bool,
        authority: Option<&str>,
        apply: impl FnOnce(&mut NavigationMutation<'_>) -> Result<(), String>,
    ) -> Result<PreparedNavigation, String> {
        if action_blocked {
            return Err(
                "Finish the current tool or gesture before navigating between views".into(),
            );
        }
        let mut navigation = self.clone();
        navigation.ensure_index(read, scene);
        if let Some(authority) = authority {
            navigation.validate_authority(authority)?;
        }
        let mut editor = read
            .editor()
            .fork_accepted_authority()
            .map_err(|error| error.to_string())?;
        // A fork retains accepted geometry but has its own retained-session seal.
        // Map exact occurrences against that presentation before installing them.
        let fork_scene = scene
            .map(|scene| editor.scene(scene.viewport, 0.25))
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(scene) = scene {
            let selection = geosolve_constraint_editor::map_presentation_selection(
                scene,
                read.editor().presentation_bindings().as_ref(),
                fork_scene.as_ref().expect("fork scene supplied"),
                editor.presentation_bindings().as_ref(),
                read.editor().editor().selection_presentation_state(),
            )
            .map_err(|error| format!("navigation fork selection: {error}"))?;
            editor
                .restore_navigation_selection(
                    fork_scene.as_ref().expect("fork scene supplied"),
                    selection,
                    read.editor().selected_declaration(),
                )
                .map_err(|error| format!("navigation fork restore: {error}"))?;
        }
        if scene.is_none() {
            editor.set_selection(read.editor().editor().selection().to_vec());
            if editor.selected_declaration() != read.editor().selected_declaration() {
                editor.set_selected_declaration(read.editor().selected_declaration());
            }
        }
        apply(&mut NavigationMutation {
            editor: &mut editor,
            navigation: &mut navigation,
            scene,
        })?;
        let selection = editor.editor().selection_presentation_state();
        let selection = if let Some(scene) = scene {
            geosolve_constraint_editor::map_presentation_selection(
                fork_scene.as_ref().expect("fork scene supplied"),
                editor.presentation_bindings().as_ref(),
                scene,
                read.editor().presentation_bindings().as_ref(),
                selection,
            )
            .map_err(|error| format!("navigation return selection: {error}"))?
        } else {
            selection
        };
        Ok(PreparedNavigation {
            selection,
            logical_owner: editor.selected_declaration(),
            navigation,
        })
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one accepted ownership/cache publication, with a scene-free draft-only refresh"
    )]
    pub(super) fn ensure_index(&mut self, read: &ChromeRead<'_>, scene: Option<&EditorScene>) {
        let geometry_key = self.geometry_key(read);
        let key = serde_json::json!({"geometry":geometry_key,
            "pending":read.pending,
            "dirty":read.code_project.as_ref().is_some_and(CodeChrome::is_dirty),
        })
        .to_string();
        if self.cache.as_ref().is_some_and(|cache| cache.key == key) {
            return;
        }
        let reuse_items = self
            .cache
            .as_ref()
            .is_some_and(|cache| cache.geometry_key == geometry_key);
        let mut entries = BTreeMap::new();
        let (source, source_digest, unavailable_reason) = if let Some(code) = &read.code_project {
            let index = code.navigation_index(read.editor());
            for entry in index.entries {
                let items = entry.exact_bindings.map_or_else(
                    || {
                        read.editor()
                            .navigation_selection_items(entry.nodes.iter().copied())
                    },
                    |bindings| {
                        read.editor()
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
            for cell in read.editor().workbench_projection().outline {
                for declaration in cell.declarations {
                    entries.insert(
                        intent_panel_row_id(&declaration.symbol),
                        NavigationEntry {
                            nodes: BTreeSet::from([declaration.node]),
                            items: read
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
        let explorer = read.explorer_snapshot();
        complete_hierarchy(&explorer, &mut entries);
        let policy = read.editor().editor().geometry_interaction_policy();
        for (id, entry) in &mut entries {
            if reuse_items
                && let Some(previous) = self.cache.as_ref().and_then(|cache| cache.entries.get(id))
            {
                entry.items.clone_from(&previous.items);
                continue;
            }
            entry.items = canonical_selection(entry.items.iter().copied(), scene);
            entry
                .items
                .retain(|item| scene.is_some_and(|scene| item_is_visible(scene, *item, policy)));
        }
        let authority = geosolve_sketch_intent::intent_content_digest(
            serde_json::json!({"key":key, "source":source_digest})
                .to_string()
                .as_bytes(),
        )
        .to_string();
        self.cache = Some(Rc::new(NavigationIndex {
            key,
            geometry_key,
            authority,
            source,
            unavailable_reason,
            entries,
            explorer,
        }));
        self.explicit_rows.clear();
        self.notice = None;
        self.remember_selection(read.editor());
    }

    fn remember_selection(&mut self, editor: &ProjectionalEditorSession) {
        self.remembered_items = editor.editor().selection().to_vec();
        self.remembered_owner = editor.selected_declaration();
    }

    fn current_snapshot(
        &mut self,
        editor: &ProjectionalEditorSession,
        scene: Option<&EditorScene>,
    ) -> NavigationSnapshot {
        if self.remembered_items != editor.editor().selection()
            || self.remembered_owner != editor.selected_declaration()
        {
            self.explicit_rows.clear();
            self.notice = None;
            self.remember_selection(editor);
        }
        let cache = self.cache.as_ref().expect("navigation index installed");
        let selected = canonical_selection(editor.editor().selection().iter().copied(), scene);
        let owners = if self.explicit_rows.is_empty() {
            editor
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
            &self.explicit_rows,
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
            serde_json::json!({"instance":self.instance,"rows":rows,"items":format!("{selected:?}")})
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
            notice: self.notice.clone(),
        }
    }

    pub(super) fn explorer_snapshot(
        &self,
        editor: &ProjectionalEditorSession,
        scene: Option<&EditorScene>,
    ) -> Vec<ExplorerSnapshot> {
        fn mark(rows: &mut [ExplorerSnapshot], target: Option<&str>) {
            for row in rows {
                row.selected =
                    row.row_kind != ExplorerRowKind::Group && Some(row.id.as_str()) == target;
                mark(&mut row.children, target);
            }
        }

        let mut rows = self
            .cache
            .as_ref()
            .expect("navigation index installed")
            .explorer
            .clone();
        let target = self.inspector_row_id(editor, scene);
        mark(&mut rows, target);
        rows
    }

    fn inspector_row_id(
        &self,
        editor: &ProjectionalEditorSession,
        scene: Option<&EditorScene>,
    ) -> Option<&str> {
        let selected = editor.selected_declaration();
        let cache = self.cache.as_ref().unwrap();
        let items = canonical_selection(editor.editor().selection().iter().copied(), scene);
        // Only one row exposes mutation controls. Exact siblings may share a
        // declaration identity, but they must never all become Inspector targets.
        let empty_generated_count = self
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
                        !self.explicit_rows.contains(*id),
                        !id.starts_with("managed:"),
                        *id,
                    )
                })
                .map(|(id, _)| id.as_str())
        })
    }

    pub(super) fn selection_snapshot(
        &self,
        read: &ChromeRead<'_>,
        scene: Option<&EditorScene>,
        navigation: &NavigationSnapshot,
    ) -> Option<SelectionSnapshot> {
        let editor = read.editor();
        if editor.selected_declaration().is_none()
            && (navigation.item_count > 1 || navigation.rows.len() > 1)
        {
            return None;
        }
        let mut selection = read.selection_snapshot()?;
        if let Some(id) = self.inspector_row_id(editor, scene) {
            selection
                .label
                .clone_from(&self.cache.as_ref()?.entries.get(id)?.label);
        }
        if let Some(source) = navigation.sources.first() {
            selection.source = Some(source.clone());
        }
        Some(selection)
    }
}

impl NavigationMutation<'_> {
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
        let current = self.navigation.current_snapshot(self.editor, self.scene);
        let remove = matches!(mode, NavigationMode::Toggle)
            && ids.iter().all(|id| {
                current
                    .rows
                    .iter()
                    .any(|row| &row.id == id && row.state == "selected")
            });
        let previous =
            canonical_selection(self.editor.editor().selection().iter().copied(), self.scene)
                .into_iter()
                .collect::<Vec<_>>();
        if !self
            .editor
            .select_navigation_declarations(nodes, Modifiers::default())
        {
            return Err("The navigation output no longer matches accepted authority".into());
        }
        let logical = self.editor.selected_declaration();
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
        self.editor.set_selection(kept);
        if logical.is_none() && matches!(mode, NavigationMode::Replace) {
            self.editor.set_selected_declaration(None);
        } else if !remove && self.editor.editor().selection().is_empty() {
            let _ = self.editor.set_selected_declaration(logical);
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
        self.navigation.remember_selection(self.editor);
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
            let owner = self.editor.selected_declaration();
            if logical_owners.len() > 1
                || owner.is_some_and(|owner| !logical_owners.contains(&owner))
            {
                self.editor.set_selected_declaration(None);
            } else if self.editor.editor().selection().is_empty() {
                self.editor
                    .set_selected_declaration(logical_owners.iter().next().copied());
            }
        }
    }

    fn select_source_request(&mut self, request: &SourceRequest) -> Result<(), String> {
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
}

impl WorkbenchBridge {
    pub(super) fn navigation_geometry_key(&self) -> String {
        self.navigation.geometry_key(&self.chrome_read())
    }

    fn navigation_context(
        &mut self,
    ) -> (&mut NavigationState, ChromeRead<'_>, Option<&EditorScene>) {
        let dimension_instance = self.dimension_instance();
        let editor = self
            .authority
            .projectional_ref()
            .expect("bridge owns projectional authority");
        let read = ChromeRead {
            editor,
            code_project: self
                .code_project
                .as_ref()
                .map(CodeProjectWorkbench::chrome_source),
            dimension_instance,
            pending: self.pending_managed_mutation.is_some(),
            interaction_blocked: self.captured_pointer.is_some()
                || self.active_tool != "select"
                || editor.editor().active_pointer_gesture().is_some(),
            hidden_rows: &self.explorer_visibility.hidden_rows,
        };
        (&mut self.navigation, read, self.retained_scene.as_ref())
    }

    fn refresh_navigation_scene(&mut self) {
        if self.navigation.needs_scene_refresh(&self.chrome_read()) {
            self.refresh_current_scene();
        }
    }

    pub(super) fn ensure_navigation_index(&mut self) {
        self.refresh_navigation_scene();
        let (navigation, read, scene) = self.navigation_context();
        navigation.ensure_index(&read, scene);
    }

    pub(super) fn navigation_snapshot(&mut self) -> NavigationSnapshot {
        self.refresh_navigation_scene();
        let (navigation, read, scene) = self.navigation_context();
        navigation.snapshot(&read, scene)
    }

    pub(super) fn navigation_explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        self.navigation
            .explorer_snapshot(self.editor(), self.retained_scene.as_ref())
    }

    pub(super) fn navigation_selection_snapshot(
        &self,
        navigation: &NavigationSnapshot,
    ) -> Option<SelectionSnapshot> {
        self.navigation.selection_snapshot(
            &self.chrome_read(),
            self.retained_scene.as_ref(),
            navigation,
        )
    }

    fn navigation_action_blocked(&self) -> bool {
        self.captured_pointer.is_some()
            || self.pending_managed_mutation.is_some()
            || self.editor().editor().active_pointer_gesture().is_some()
            || self.active_tool != "select"
            || self.authoring.active_tool().is_some()
            || self.feature_authoring.active_tool().is_some()
            || self.offset_authoring.is_active()
            || self.editor().editor().geometry_draft_status().is_some()
    }

    fn install_navigation(&mut self, prepared: PreparedNavigation) -> Result<(), String> {
        let editor = self
            .authority
            .projectional_mut()
            .expect("bridge owns projectional authority");
        if let Some(scene) = self.retained_scene.as_ref() {
            // The renderer may retain a filtered presentation. Install against
            // a fresh scene sealed by the current editor, translating its exact
            // picked occurrences before any personal state changes.
            let native_scene = editor
                .scene(scene.viewport, 0.25)
                .map_err(|error| error.to_string())?;
            let selection = geosolve_constraint_editor::map_presentation_selection(
                scene,
                editor.presentation_bindings().as_ref(),
                &native_scene,
                editor.presentation_bindings().as_ref(),
                prepared.selection,
            )?;
            editor
                .restore_navigation_selection(&native_scene, selection, prepared.logical_owner)
                .map_err(|error| format!("navigation install: {error}"))?;
        } else {
            editor.set_selection(prepared.selection.items);
        }
        if editor.selected_declaration() != prepared.logical_owner {
            editor.set_selected_declaration(prepared.logical_owner);
        }
        self.navigation = prepared.navigation;
        Ok(())
    }

    pub(super) fn select_navigation_rows_json(
        &mut self,
        value: serde_json::Value,
    ) -> Result<(), String> {
        self.refresh_navigation_scene();
        let prepared = self.navigation.prepare_rows_json(
            &self.chrome_read(),
            self.retained_scene.as_ref(),
            self.navigation_action_blocked(),
            value,
        )?;
        self.install_navigation(prepared)
    }

    pub(super) fn select_navigation_rows_legacy(&mut self, id: &str) -> Result<(), String> {
        self.refresh_navigation_scene();
        let prepared = self.navigation.prepare_rows_legacy(
            &self.chrome_read(),
            self.retained_scene.as_ref(),
            self.navigation_action_blocked(),
            id,
        )?;
        self.install_navigation(prepared)
    }

    pub(super) fn select_navigation_source_json(
        &mut self,
        value: serde_json::Value,
    ) -> Result<(), String> {
        self.refresh_navigation_scene();
        let prepared = self.navigation.prepare_source_json(
            &self.chrome_read(),
            self.retained_scene.as_ref(),
            self.navigation_action_blocked(),
            value,
        )?;
        self.install_navigation(prepared)
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
        let dimensions = self.dimensions_snapshot();
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
            "dimensions":dimensions,
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
