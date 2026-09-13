// SPDX-License-Identifier: GPL-3.0-or-later
//! Borrowed accepted projections shared by editing hosts and read-only browsing.
use super::*;
use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_code::{
    CodeSessionIdentity, CodeSessionSnapshot, CompiledManagedSource, InspectorDescriptorIndex,
    InspectorParameterPresentation, ManagedAuthoredMetadata, ManagedControlManifest,
    ManagedSourceInspector, ManagedSpan, MaterializedCodeProject,
};
use geosolve_sketch_code::{ManagedDeclarationPanelProjection, SemanticSymbol};
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone)]
pub(super) enum ChromeProjection<'a, T> {
    Borrowed(&'a T),
    Shared(Rc<T>),
}
impl<T> AsRef<T> for ChromeProjection<'_, T> {
    fn as_ref(&self) -> &T {
        match self {
            Self::Borrowed(value) => value,
            Self::Shared(value) => value,
        }
    }
}
impl<T> std::ops::Deref for ChromeProjection<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

/// Source projections only; no source working buffer, compiler, publication or history owner.
pub(crate) struct CodeChrome<'a> {
    snapshot: &'a CodeSessionSnapshot,
    token: &'a CodeSessionIdentity,
    inspector: ManagedSourceInspector<'a>,
    dirty: bool,
    controls: Result<ChromeProjection<'a, ManagedControlManifest>, String>,
    metadata: Result<ChromeProjection<'a, ManagedAuthoredMetadata>, String>,
    navigation: Option<&'a geosolve_sketch_code::ManagedNavigationIndex>,
    materialized_identity: Option<geosolve_sketch_intent::IntentSessionIdentity>,
}
impl<'a> CodeChrome<'a> {
    pub(crate) fn new(
        snapshot: &'a CodeSessionSnapshot,
        token: &'a CodeSessionIdentity,
        materialized: Option<&'a MaterializedCodeProject>,
        dirty: bool,
        controls: Result<Rc<ManagedControlManifest>, String>,
        metadata: Result<Rc<ManagedAuthoredMetadata>, String>,
    ) -> Self {
        Self {
            snapshot,
            token,
            inspector: ManagedSourceInspector::new(snapshot, materialized),
            dirty,
            controls: controls.map(ChromeProjection::Shared),
            metadata: metadata.map(ChromeProjection::Shared),
            navigation: None,
            materialized_identity: materialized
                .map(|value| value.editor.coordinator().intent().identity()),
        }
    }
    pub(super) fn accepted(
        session: &'a geosolve_sketch_engine::AcceptedBrowsingSession,
    ) -> Option<Self> {
        let source = session.source()?;
        Some(Self {
            snapshot: source.snapshot(),
            token: source.token(),
            inspector: session.inspector()?,
            dirty: false,
            controls: Ok(ChromeProjection::Borrowed(source.controls())),
            metadata: Ok(ChromeProjection::Borrowed(source.metadata())),
            navigation: Some(source.navigation()),
            materialized_identity: Some(session.editor().coordinator().intent().identity()),
        })
    }
    pub(super) fn code_session_identity(&self) -> &CodeSessionIdentity {
        self.token
    }
    pub(super) fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub(super) fn managed_controls_cached(
        &self,
    ) -> Result<ChromeProjection<'a, ManagedControlManifest>, String> {
        self.controls.clone()
    }
    pub(super) fn authored_metadata_cached(
        &self,
    ) -> Result<ChromeProjection<'a, ManagedAuthoredMetadata>, String> {
        self.metadata.clone()
    }
    pub(super) fn managed_compilation(&self) -> Option<&CompiledManagedSource> {
        self.snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&self.snapshot.managed, |project| &project.managed)
            .compiled
            .as_deref()
    }
    pub(super) fn metadata_edit_blocked_reason(&self) -> Option<String> {
        if self.dirty {
            Some("Apply or Revert the source draft before editing properties".into())
        } else if self.snapshot.failure.is_some() {
            Some("Resolve or Undo the retained failure before editing properties".into())
        } else {
            None
        }
    }
    pub(super) fn selected_managed_declaration(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<SemanticSymbol>, String> {
        self.inspector.selected_managed_declaration(editor)
    }
    pub(super) fn managed_declaration_source_span(
        &self,
        symbol: &SemanticSymbol,
    ) -> Result<ManagedSpan, String> {
        self.inspector.managed_declaration_source_span(symbol)
    }
    pub(super) fn declaration_panel_projection(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> ManagedDeclarationPanelProjection {
        geosolve_sketch_code::managed_declaration_panel_projection(
            self.snapshot,
            editor,
            self.dirty,
            self.controls.as_ref().ok().map(AsRef::as_ref),
            self.metadata.as_ref().ok().map(AsRef::as_ref),
        )
    }
    pub(super) fn navigation_index(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> geosolve_sketch_code::ManagedNavigationIndex {
        self.navigation.cloned().unwrap_or_else(|| {
            geosolve_sketch_code::managed_browsing_navigation_index(
                self.snapshot,
                editor,
                self.materialized_identity,
                self.dirty,
                false,
            )
        })
    }
    pub(super) fn inspector_parameter_presentations_with_manifest(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &geosolve_constraint_editor::IntentWorkbenchProjection,
        inspector: &geosolve_constraint_editor::IntentInspectorProjection,
        descriptors: &InspectorDescriptorIndex<'_>,
        manifest: Result<&ManagedControlManifest, &str>,
    ) -> Result<Vec<InspectorParameterPresentation>, String> {
        self.inspector.inspector_parameter_presentations(
            editor,
            projection,
            inspector,
            descriptors,
            manifest,
        )
    }
    pub(super) fn inspector_parameter_presentations(
        &self,
        editor: &ProjectionalEditorSession,
        inspector: &geosolve_constraint_editor::IntentInspectorProjection,
    ) -> Result<Vec<InspectorParameterPresentation>, String> {
        self.inspector_parameter_presentations_with_manifest(
            editor,
            &editor.workbench_projection(),
            inspector,
            &InspectorDescriptorIndex::new(inspector),
            self.controls
                .as_ref()
                .map(AsRef::as_ref)
                .map_err(String::as_str),
        )
    }
    pub(super) fn managed_control_source_mutation(
        &self,
        id: &str,
        value: ManagedControlSubmission,
    ) -> Result<Option<ManagedSketchMutation>, String> {
        if self.dirty {
            return Err(
                "Apply or Revert the managed-source draft before editing managed controls".into(),
            );
        }
        geosolve_sketch_code::managed_control_source_mutation(
            self.snapshot
                .accepted_code_project
                .as_ref()
                .ok_or("accepted source unavailable")?,
            self.snapshot
                .accepted_expansion
                .as_ref()
                .ok_or("accepted source expansion unavailable")?,
            id,
            value,
        )
    }
}

/// One immutable host observation. Hosts supply only their own transient blocking policy.
pub(super) struct ChromeRead<'a> {
    pub(super) editor: &'a ProjectionalEditorSession,
    pub(super) code_project: Option<CodeChrome<'a>>,
    pub(super) dimension_instance: u64,
    pub(super) pending: bool,
    pub(super) interaction_blocked: bool,
    pub(super) hidden_rows: &'a BTreeSet<String>,
}
impl ChromeRead<'_> {
    pub(super) fn editor(&self) -> &ProjectionalEditorSession {
        self.editor
    }
    pub(super) fn dimension_instance(&self) -> u64 {
        self.dimension_instance
    }
}
impl WorkbenchBridge {
    pub(super) fn chrome_read(&self) -> ChromeRead<'_> {
        ChromeRead {
            editor: self.editor(),
            code_project: self.code_project.as_ref().map(|code| code.chrome_source()),
            dimension_instance: self.dimension_instance(),
            pending: self.pending_managed_mutation.is_some(),
            interaction_blocked: self.captured_pointer.is_some()
                || self.active_tool != "select"
                || self.editor().editor().active_pointer_gesture().is_some(),
            hidden_rows: &self.explorer_visibility.hidden_rows,
        }
    }
}

impl ChromeRead<'_> {
    pub(super) fn declaration_row_target(&self, id: &str) -> Option<DeclarationRowTarget> {
        if let Some(code) = &self.code_project {
            let projection = code.declaration_panel_projection(self.editor());
            return managed_declaration_row_target(&projection.declarations, id);
        }
        self.editor()
            .workbench_projection()
            .outline
            .into_iter()
            .flat_map(|cell| cell.declarations)
            .find(|declaration| intent_panel_row_id(&declaration.symbol) == id)
            .map(|declaration| DeclarationRowTarget::Intent {
                node: declaration.node,
            })
    }

    pub(super) fn declaration_move_mutation(
        &self,
        payload: &DeclarationMovePayload,
    ) -> Result<Option<ManagedSketchMutation>, String> {
        let direction_route = payload.direction.is_some()
            && payload.target_id.is_none()
            && payload.position.is_none();
        let drop_route = payload.direction.is_none()
            && payload.target_id.is_some()
            && payload.position.is_some();
        if !direction_route && !drop_route {
            return Err(
                "declaration move requires either one direction or one target/position pair".into(),
            );
        }
        let source = match self.declaration_row_target(&payload.id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => {
                return Err(format!(
                    "Profile Offset helper declarations cannot move independently of `{}`",
                    root.0
                ));
            }
            Some(DeclarationRowTarget::Managed { symbol, .. }) => symbol,
            Some(DeclarationRowTarget::Generated { .. }) => {
                return Err("generated outputs cannot move independently of their owner".into());
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                return Err("ordinary design declaration order is read-only here".into());
            }
            None => return Err("the declaration move target is unavailable or stale".into()),
        };
        let code = self
            .code_project
            .as_ref()
            .ok_or_else(|| "the current project has no managed declaration order".to_owned())?;
        let order = code
            .declaration_panel_projection(self.editor())
            .declarations
            .into_iter()
            .map(|row| row.symbol.0)
            .collect::<Vec<_>>();
        let source_index = order
            .iter()
            .position(|symbol| symbol == &source)
            .ok_or_else(|| "the declaration move source is stale".to_owned())?;
        let before = if let Some(direction) = &payload.direction {
            match direction {
                DeclarationMoveDirection::Up => {
                    let previous = source_index
                        .checked_sub(1)
                        .ok_or_else(|| "the declaration is already first".to_owned())?;
                    Some(order[previous].clone())
                }
                DeclarationMoveDirection::Down => {
                    if source_index + 1 >= order.len() {
                        return Err("the declaration is already last".into());
                    }
                    order.get(source_index + 2).cloned()
                }
            }
        } else {
            let target_id = payload
                .target_id
                .as_deref()
                .expect("validated drop route has a target");
            let target = self.declaration_drop_target(target_id)?;
            if target == source {
                return Ok(None);
            }
            let mut remaining = order
                .into_iter()
                .filter(|symbol| symbol != &source)
                .collect::<Vec<_>>();
            let target_index = remaining
                .iter()
                .position(|symbol| symbol == &target)
                .ok_or_else(|| "the declaration drop target is stale".to_owned())?;
            let insertion = match payload.position {
                Some(DeclarationMovePosition::Before) => target_index,
                Some(DeclarationMovePosition::After) => target_index + 1,
                None => unreachable!("validated drop route has a position"),
            };
            remaining.insert(insertion, source.clone());
            remaining.get(insertion + 1).cloned()
        };
        Ok(Some(ManagedSketchMutation::ReorderDeclaration {
            declaration: source,
            before,
        }))
    }

    pub(super) fn declaration_drop_target(&self, id: &str) -> Result<String, String> {
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => Err(format!(
                "Profile Offset helper declarations cannot be independent move destinations; target `{}` instead",
                root.0
            )),
            Some(DeclarationRowTarget::Managed { symbol, .. }) => Ok(symbol),
            _ => Err("the declaration drop target is unavailable or stale".into()),
        }
    }

    pub(super) fn suppression_source_mutation(
        &self,
        id: &str,
        suppressed: bool,
    ) -> Result<ManagedSketchMutation, String> {
        if let Some(DeclarationRowTarget::Managed {
            closure_role: ManagedDeclarationClosureRole::Helper { root },
            ..
        }) = self.declaration_row_target(id)
        {
            return Err(format!(
                "Profile Offset helper declarations cannot be suppressed independently of `{}`",
                root.0
            ));
        }
        Ok(ManagedSketchMutation::SetSuppressed {
            target: self.managed_row_target(id)?,
            suppressed,
        })
    }

    pub(super) fn parameter_source_mutation(
        &self,
        id: &str,
        value: serde_json::Value,
    ) -> Result<Option<ManagedSketchMutation>, String> {
        let submission = {
            let code = self
                .code_project
                .as_ref()
                .ok_or_else(|| "the current project has no managed parameters".to_owned())?;
            let manifest = code.managed_controls_cached()?;
            let control = manifest
                .controls
                .iter()
                .find(|control| control.id.0 == id)
                .ok_or_else(|| "managed parameter is unavailable".to_owned())?;
            match (&control.value, value) {
                (
                    ManagedValue::Number(_) | ManagedValue::Unit(_),
                    serde_json::Value::Number(value),
                ) => ManagedControlSubmission::Number(
                    value
                        .as_f64()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| "managed parameter number must be finite".to_owned())?,
                ),
                (
                    ManagedValue::Number(_) | ManagedValue::Unit(_),
                    serde_json::Value::String(value),
                ) => ManagedControlSubmission::Number(
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| "managed parameter number must be finite".to_owned())?,
                ),
                (ManagedValue::Bool(_), serde_json::Value::Bool(value)) => {
                    ManagedControlSubmission::Boolean(value)
                }
                (ManagedValue::Bool(_), serde_json::Value::String(value)) => {
                    ManagedControlSubmission::Boolean(match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err("managed Boolean parameter must be true or false".into()),
                    })
                }
                (ManagedValue::String(_), serde_json::Value::String(value)) => {
                    ManagedControlSubmission::String(value)
                }
                _ => return Err("managed parameter value has the wrong type".into()),
            }
        };
        self.code_project
            .as_ref()
            .expect("managed parameter authority was checked")
            .managed_control_source_mutation(id, submission)
    }

    pub(super) fn managed_row_target(&self, id: &str) -> Result<ManagedMutationTarget, String> {
        match self.declaration_row_target(id) {
            Some(DeclarationRowTarget::Managed {
                closure_role: ManagedDeclarationClosureRole::Helper { root },
                ..
            }) => Err(format!(
                "Profile Offset helper declaration lifecycle is owned by `{}`",
                root.0
            )),
            Some(DeclarationRowTarget::Managed { symbol, .. }) => {
                Ok(ManagedMutationTarget::Declaration {
                    declaration: symbol,
                })
            }
            Some(DeclarationRowTarget::Generated { address, .. }) => {
                Ok(ManagedMutationTarget::Generated {
                    address: ExecutedGeneratedMemberAddress {
                        invocation: address.invocation,
                        template: address.template,
                        member_key: address.member_key,
                        output: address.output,
                    },
                })
            }
            Some(DeclarationRowTarget::Intent { .. }) => {
                Err("ordinary design declarations are not managed mutation targets".into())
            }
            None => Err("the managed declaration target is unavailable or stale".into()),
        }
    }
    pub(super) fn base_explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        let selected = self.editor().selected_declaration();
        let Some(code) = &self.code_project else {
            return self
                .editor()
                .workbench_projection()
                .outline
                .into_iter()
                .map(|cell| ExplorerSnapshot {
                    id: format!("intent-group:{}", cell.cell),
                    label: cell.name.to_string(),
                    kind: "Group".into(),
                    row_kind: ExplorerRowKind::Group,
                    selected: false,
                    suppressed: None,
                    source: None,
                    visible: true,
                    effective_visible: true,
                    visibility_state: ExplorerVisibilitySnapshot::Visible,
                    children: cell
                        .declarations
                        .into_iter()
                        .map(|declaration| ExplorerSnapshot {
                            id: intent_panel_row_id(&declaration.symbol),
                            label: declaration.name.to_string(),
                            kind: node_family_label(&declaration.kind).into(),
                            row_kind: ExplorerRowKind::Declaration,
                            selected: selected == Some(declaration.node),
                            suppressed: Some(declaration.suppressed),
                            source: None,
                            visible: true,
                            effective_visible: true,
                            visibility_state: ExplorerVisibilitySnapshot::Visible,
                            children: Vec::new(),
                            capabilities: read_only_intent_capabilities(),
                        })
                        .collect(),
                    capabilities: group_capabilities(),
                })
                .collect();
        };

        let projection = code.declaration_panel_projection(self.editor());
        let blocked = projection.blocked_reason.as_deref();
        let declaration_count = projection.declarations.len();
        let mut groups = Vec::<ExplorerSnapshot>::new();
        let mut group_indices = std::collections::BTreeMap::<Option<String>, usize>::new();
        for (index, declaration) in projection.declarations.into_iter().enumerate() {
            let group = declaration.group.clone();
            let group_index = if let Some(group_index) = group_indices.get(&group) {
                *group_index
            } else {
                let group_index = groups.len();
                groups.push(ExplorerSnapshot {
                    id: managed_group_row_id(group.as_deref()),
                    label: group.clone().unwrap_or_else(|| "Declarations".into()),
                    kind: "Group".into(),
                    row_kind: ExplorerRowKind::Group,
                    selected: false,
                    suppressed: None,
                    source: None,
                    visible: true,
                    effective_visible: true,
                    visibility_state: ExplorerVisibilitySnapshot::Visible,
                    children: Vec::new(),
                    capabilities: group_capabilities(),
                });
                group_indices.insert(group, group_index);
                group_index
            };
            groups[group_index]
                .children
                .push(managed_declaration_explorer_snapshot(
                    declaration,
                    blocked,
                    index > 0,
                    index + 1 < declaration_count,
                ));
        }
        groups
    }

    pub(super) fn explorer_snapshot(&self) -> Vec<ExplorerSnapshot> {
        let mut rows = self.base_explorer_snapshot();
        apply_explorer_visibility(&mut rows, self.hidden_rows, true);
        rows
    }

    pub(super) fn selection_snapshot(&self) -> Option<SelectionSnapshot> {
        let editor = self.editor();
        if let Some(inspector) = editor.selected_declaration().and_then(|node| {
            geosolve_constraint_editor::IntentInspectorProjection::from_session(
                editor.coordinator().intent(),
                node,
            )
        }) {
            let (ownership, source) = self.code_project.as_ref().map_or_else(
                || ("Modifiable instance".into(), None),
                |code| managed_selection_source(code, editor, &inspector),
            );
            return Some(SelectionSnapshot {
                id: inspector.node.to_string(),
                label: inspector.name.to_string(),
                kind: node_family_label(&inspector.kind).into(),
                ownership: Some(ownership),
                source,
                metadata: self.selected_authoring_metadata(),
            });
        }
        editor
            .editor()
            .selection()
            .first()
            .map(|item| SelectionSnapshot {
                id: format!("{item:?}"),
                label: selection_label(*item),
                kind: selection_kind(*item).into(),
                ownership: Some("Modifiable instance".into()),
                source: None,
                metadata: self.selected_authoring_metadata(),
            })
    }

    pub(super) fn parameter_snapshot(&self) -> Vec<ParameterSnapshot> {
        let Some(code) = &self.code_project else {
            return Vec::new();
        };
        let Ok(manifest) = code.managed_controls_cached() else {
            return Vec::new();
        };
        let mut controls: Vec<_> = manifest.controls.iter().collect();
        controls.sort_by_key(|control| control.source.span.start);
        controls
            .into_iter()
            .filter_map(|control| {
                let editable = matches!(control.access, ManagedControlAccess::Editable { .. });
                let (value, unit) = managed_value_text(&control.value)?;
                Some(ParameterSnapshot {
                    id: control.id.0.clone(),
                    label: managed_control_label(control),
                    value,
                    unit,
                    editable,
                    row_key: format!("{}:{}", self.dimension_instance(), control.id.0),
                    metadata: self.parameter_metadata_snapshot(control),
                    consumers: self.parameter_consumer_labels(control),
                })
            })
            .collect()
    }
}

impl ChromeRead<'_> {
    pub(super) fn append_native_problems(&self, problems: &mut Vec<ProblemSnapshot>) {
        let projection = self.editor().workbench_projection();
        if let Some(diagnostic) = projection.latest_diagnostic
            && !problems
                .iter()
                .any(|problem| problem.id == "retained-code-failure")
        {
            problems.push(ProblemSnapshot {
                id: "retained-intent-failure".into(),
                severity: "error",
                title: "Design intent retained a failure".into(),
                detail: diagnostic.to_string(),
                file: None,
                line: None,
                column: None,
            });
        }
    }
    pub(super) fn accepted_problems(&self) -> Vec<ProblemSnapshot> {
        let mut problems = Vec::new();
        self.append_native_problems(&mut problems);
        problems
    }
}

impl ChromeRead<'_> {
    pub(super) fn explorer_scene_items(
        &self,
        scene: &EditorScene,
        rows: impl IntoIterator<Item = String>,
    ) -> std::collections::BTreeMap<String, Vec<SelectionItem>> {
        let Some(materialization) = self.editor().coordinator().accepted_materialization() else {
            return std::collections::BTreeMap::new();
        };
        // Resolve one accepted declaration projection for the entire visibility
        // batch. Rebuilding the panel for every hidden generated row turns dense
        // group isolation into repeated whole-project work.
        let managed = self
            .code_project
            .as_ref()
            .map(|code| code.declaration_panel_projection(self.editor()));
        let ordinary = managed.is_none().then(|| {
            self.editor()
                .workbench_projection()
                .outline
                .into_iter()
                .flat_map(|cell| cell.declarations)
                .map(|declaration| (intent_panel_row_id(&declaration.symbol), declaration.node))
                .collect::<std::collections::BTreeMap<_, _>>()
        });
        let mut nodes = std::collections::BTreeMap::new();
        for id in rows {
            let target = managed.as_ref().map_or_else(
                || {
                    ordinary
                        .as_ref()?
                        .get(&id)
                        .copied()
                        .map(|node| DeclarationRowTarget::Intent { node })
                },
                |projection| managed_declaration_row_target(&projection.declarations, &id),
            );
            let node = match target {
                Some(DeclarationRowTarget::Intent { node }) => Some(node),
                Some(
                    DeclarationRowTarget::Managed { node, .. }
                    | DeclarationRowTarget::Generated { node, .. },
                ) => node,
                None => None,
            };
            if let Some(node) = node {
                nodes.insert(id, node);
            }
        }

        let mut result = std::collections::BTreeMap::new();
        for (id, node) in nodes {
            let mut hidden = std::collections::BTreeSet::new();
            let Some(owner) = materialization.ownership.node(node) else {
                continue;
            };
            for binding in &owner.owned {
                match *binding {
                    geosolve_constraint_editor::IntentNativeBinding::Point(point) => {
                        hidden.insert(SelectionItem::Point(point));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Curve(curve) => {
                        hidden.extend(
                            scene
                                .curves
                                .iter()
                                .filter(|candidate| candidate.span.curve == curve)
                                .map(|candidate| SelectionItem::Curve(candidate.span)),
                        );
                    }
                    geosolve_constraint_editor::IntentNativeBinding::CurveSpan(span) => {
                        hidden.insert(SelectionItem::Curve(span));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Constraint(constraint) => {
                        hidden.insert(SelectionItem::Constraint(constraint));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Dimension(dimension) => {
                        hidden.insert(SelectionItem::Dimension(dimension));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::ComputedFeature(feature) => {
                        hidden.insert(SelectionItem::Feature(feature));
                    }
                    geosolve_constraint_editor::IntentNativeBinding::ComputedFeatureCorner(
                        corner,
                    ) => {
                        hidden.extend(
                            scene
                                .computed_curves
                                .iter()
                                .filter(|curve| curve.owner.corner == corner)
                                .map(|curve| SelectionItem::FeatureCorner(curve.owner)),
                        );
                    }
                    geosolve_constraint_editor::IntentNativeBinding::Scalar(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Contact(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Source(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Parameter(_)
                    | geosolve_constraint_editor::IntentNativeBinding::ExternalBinding(_)
                    | geosolve_constraint_editor::IntentNativeBinding::Logical(_) => {}
                }
            }
            result.insert(id, hidden.into_iter().collect());
        }
        result
    }
}
