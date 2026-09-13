// SPDX-License-Identifier: GPL-3.0-or-later
//! Personal browsing over one retained, independently accepted source/native result.

use crate::{AcceptedEvaluation, EditableSession, EngineError};
use geosolve_constraint_editor::{
    EditorScene, GeometryInteractionPolicy, ProjectionalEditorSession, SelectionPresentationState,
    Viewport,
};
use geosolve_sketch_code::{
    CodeSessionIdentity, CodeSessionSnapshot, ManagedAuthoredMetadata, ManagedControlManifest,
    ManagedDeclarationPanelProjection, ManagedNavigationIndex,
};

/// An isolated personal presentation. The native result and source authority are
/// retained; there is no compiler, solve, publication, or durable history method.
#[derive(Debug)]
pub struct AcceptedBrowsingSession {
    accepted: AcceptedEvaluation,
    presentation: Box<geosolve_sketch_code::MaterializedCodeProject>,
    source: Option<AcceptedBrowsingSource>,
}

/// Managed source authority paired with one exact native browsing presentation.
#[derive(Debug)]
pub struct AcceptedBrowsingSource {
    token: CodeSessionIdentity,
    snapshot: CodeSessionSnapshot,
    navigation: ManagedNavigationIndex,
    controls: ManagedControlManifest,
    metadata: ManagedAuthoredMetadata,
}

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

impl EditableSession {
    /// Retains this exact accepted source/native basis for independent tab browsing.
    ///
    /// # Errors
    /// Rejects unavailable accepted authority or invalid source descriptors.
    pub fn browsing(&self) -> Result<AcceptedBrowsingSession, EngineError> {
        let source = self.code_snapshot();
        let project = source
            .accepted_code_project
            .as_ref()
            .ok_or_else(|| error("missing accepted browsing project"))?;
        let expansion = source
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| error("missing accepted browsing expansion"))?;
        let generated = source
            .accepted_generated
            .as_ref()
            .unwrap_or(&source.generated);
        let editor = self
            .accepted()
            .0
            .materialized
            .editor
            .fork_accepted_authority()
            .map_err(error)?;
        let controls =
            geosolve_sketch_code::managed_control_manifest(project, expansion).map_err(error)?;
        let metadata = project
            .managed
            .compiled
            .as_ref()
            .ok_or_else(|| error("missing accepted browsing compiler receipt"))?
            .authored_metadata()
            .map_err(error)?;
        let navigation = geosolve_sketch_code::managed_navigation_index(
            &project.managed,
            expansion,
            generated,
            &editor,
        );
        Ok(AcceptedBrowsingSession {
            accepted: self.accepted().clone(),
            presentation: geosolve_sketch_code::rehydrate_materialized_code_project(
                Box::new(editor),
                expansion.clone(),
            )
            .map_err(error)?,
            source: Some(AcceptedBrowsingSource {
                token: self.token().clone(),
                snapshot: source.clone(),
                navigation,
                controls,
                metadata,
            }),
        })
    }
}

impl AcceptedEvaluation {
    /// Creates personal native browsing without granting source authoring authority.
    ///
    /// # Errors
    /// Rejects unavailable accepted native geometry or expansion correspondence.
    pub fn browsing(&self) -> Result<AcceptedBrowsingSession, EngineError> {
        let editor = self
            .0
            .materialized
            .editor
            .fork_accepted_authority()
            .map_err(error)?;
        Ok(AcceptedBrowsingSession {
            accepted: self.clone(),
            source: None,
            presentation: geosolve_sketch_code::rehydrate_materialized_code_project(
                Box::new(editor),
                self.0.materialized.expansion.clone(),
            )
            .map_err(error)?,
        })
    }
}
impl AcceptedBrowsingSource {
    pub fn token(&self) -> &CodeSessionIdentity {
        &self.token
    }
    pub fn snapshot(&self) -> &CodeSessionSnapshot {
        &self.snapshot
    }
    pub fn navigation(&self) -> &ManagedNavigationIndex {
        &self.navigation
    }
    pub fn controls(&self) -> &ManagedControlManifest {
        &self.controls
    }
    pub fn metadata(&self) -> &ManagedAuthoredMetadata {
        &self.metadata
    }
}

impl AcceptedBrowsingSession {
    pub fn accepted(&self) -> &AcceptedEvaluation {
        &self.accepted
    }
    pub fn editor(&self) -> &ProjectionalEditorSession {
        &self.presentation.editor
    }
    pub fn source(&self) -> Option<&AcceptedBrowsingSource> {
        self.source.as_ref()
    }
    /// Exact source/Inspector resolution over this retained accepted materialization.
    pub fn inspector(&self) -> Option<geosolve_sketch_code::ManagedSourceInspector<'_>> {
        self.source.as_ref().map(|source| {
            geosolve_sketch_code::ManagedSourceInspector::new(
                &source.snapshot,
                Some(&self.presentation),
            )
        })
    }
    /// Source order, closure membership and exact generated row ownership.
    pub fn declarations(&self) -> Option<ManagedDeclarationPanelProjection> {
        self.source.as_ref().map(|source| {
            geosolve_sketch_code::managed_declaration_panel_projection(
                &source.snapshot,
                self.editor(),
                false,
                Some(&source.controls),
                Some(&source.metadata),
            )
        })
    }
    /// Describes a source-control edit without preparing or applying a transaction.
    ///
    /// # Errors
    /// Rejects generator mode, unknown controls, read-only lenses and incompatible values.
    pub fn describe_control(
        &self,
        id: &str,
        value: geosolve_sketch_code::ManagedControlSubmission,
    ) -> Result<Option<geosolve_sketch_code::ManagedSketchMutation>, EngineError> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| error("generator output has no managed source controls"))?;
        geosolve_sketch_code::managed_control_source_mutation(
            source
                .snapshot
                .accepted_code_project
                .as_ref()
                .ok_or_else(|| error("accepted browsing project unavailable"))?,
            source
                .snapshot
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| error("accepted browsing expansion unavailable"))?,
            id,
            value,
        )
        .map_err(error)
    }

    /// Native scene projection of the retained authority, with no materialization.
    ///
    /// # Errors
    /// Rejects invalid viewport or native scene projection.
    pub fn scene(&self, viewport: Viewport) -> Result<EditorScene, EngineError> {
        self.presentation
            .editor
            .scene(viewport, 0.25)
            .map_err(error)
    }

    /// Installs a previously authenticated exact native selection and personal policy.
    /// The scene and occurrences are validated before any presentation is changed.
    ///
    /// # Errors
    /// Rejects stale native authority, foreign selections, or invalid occurrences.
    pub fn restore_presentation(
        &mut self,
        scene: &EditorScene,
        selection: SelectionPresentationState,
        policy: GeometryInteractionPolicy,
    ) -> Result<(), EngineError> {
        // A camera-only refresh must retain an explicitly selected declaration,
        // including a source row that currently has no visible native outputs.
        if self
            .presentation
            .editor
            .editor()
            .selection_presentation_state()
            == selection
        {
            selection.validate(scene).map_err(error)?;
            if !self.presentation.editor.scene_is_current(scene) {
                return Err(error(
                    "browsing presentation belongs to a stale native scene",
                ));
            }
        } else {
            self.presentation
                .editor
                .restore_selection_presentation(scene, selection)
                .map_err(error)?;
        }
        self.presentation
            .editor
            .editor_mut()
            .set_geometry_interaction_policy(policy);
        Ok(())
    }
}

use geosolve_constraint_editor::detached_interaction::{
    DetachedInteractionSeed, DetachedInteractionState, camera, mode,
};
use geosolve_constraint_editor::{
    DetachedDimensionPresentation, DimensionPresentationState, PresentationMapping,
    SceneDimensionEntry, VisibilitySeed,
};

/// Exact namespace mapping and personal view of an accepted browsing session.
/// Chrome adapters consume its projections without constructing an editing host.
#[derive(Debug)]
pub struct AcceptedBrowsingView {
    session: AcceptedBrowsingSession,
    source_key: String,
    source_scene: EditorScene,
    native_full_scene: EditorScene,
    scene: EditorScene,
    mapping: PresentationMapping,
    reverse: PresentationMapping,
    visibility: VisibilitySeed,
    dimensions: DetachedDimensionPresentation,
    dimension_entries: Vec<SceneDimensionEntry>,
    policy: GeometryInteractionPolicy,
    state: Option<DetachedInteractionState>,
}

impl AcceptedBrowsingView {
    /// Binds a detached scene to exact accepted source/native ownership once.
    ///
    /// # Errors
    /// Rejects invalid seeds, foreign bindings or ambiguous native correspondence.
    pub fn new(
        session: AcceptedBrowsingSession,
        seed: DetachedInteractionSeed,
    ) -> Result<Self, EngineError> {
        if seed.format != "geosolve-local-interaction-v1" {
            return Err(error("unsupported browsing scene format"));
        }
        let source_scene = EditorScene::from_detached_json(&seed.scene).map_err(error)?;
        camera(source_scene.viewport).map_err(error)?;
        seed.visibility_seed
            .validate(&seed.visibility)
            .map_err(error)?;
        let native_full_scene = session.scene(source_scene.viewport)?;
        let mapping = PresentationMapping::new(
            seed.bindings.as_ref(),
            session.editor().presentation_bindings().as_ref(),
            source_scene.presentation_document().id(),
            native_full_scene.presentation_document().id(),
        )
        .map_err(error)?;
        let reverse = mapping.reverse().map_err(error)?;
        let mut dimensions = seed.dimensions;
        mapping.dimensions(&mut dimensions).map_err(error)?;
        Ok(Self {
            session,
            source_key: seed.scene_key,
            source_scene,
            scene: native_full_scene.clone(),
            native_full_scene,
            mapping,
            reverse,
            visibility: seed.visibility_seed,
            dimensions,
            dimension_entries: Vec::new(),
            policy: seed.policy,
            state: None,
        })
    }

    pub fn session(&self) -> &AcceptedBrowsingSession {
        &self.session
    }
    pub fn scene(&self) -> &EditorScene {
        &self.scene
    }
    pub fn state(&self) -> Option<&DetachedInteractionState> {
        self.state.as_ref()
    }
    pub fn dimension_entries(&self) -> &[SceneDimensionEntry] {
        &self.dimension_entries
    }
    pub fn dimensions(&self) -> &DetachedDimensionPresentation {
        &self.dimensions
    }

    /// Exact mapping from the retained native presentation back to the host seed.
    pub fn source_mapping(&self) -> &PresentationMapping {
        &self.reverse
    }

    /// Installs host-computed read-only dimension and visibility projection, and
    /// returns dimension metadata expressed in the original seed namespace.
    ///
    /// # Errors
    /// Rejects foreign dimension identities or invalid source-row visibility.
    pub fn initialize_presentation(
        &mut self,
        dimensions: DetachedDimensionPresentation,
        visibility: VisibilitySeed,
    ) -> Result<DetachedDimensionPresentation, EngineError> {
        if self.state.is_some() {
            return Err(error("browsing presentation has already initialized"));
        }
        let mut source = dimensions.clone();
        self.reverse.dimensions(&mut source).map_err(error)?;
        source.context.selection = dimensions
            .context
            .selection
            .iter()
            .map(|item| self.reverse.selection(*item))
            .collect::<Result<_, _>>()
            .map_err(error)?;
        source.state.mode = dimensions.state.mode;
        source.state.pins = dimensions
            .state
            .pins
            .iter()
            .map(|key| self.reverse.layout_key(*key))
            .collect::<Result<_, _>>()
            .map_err(error)?;
        source.state.focus = dimensions
            .state
            .focus
            .map(|key| self.reverse.layout_key(key))
            .transpose()
            .map_err(error)?;
        visibility
            .validate(&geosolve_constraint_editor::VisibilityState::default())
            .map_err(error)?;
        self.dimensions = dimensions;
        self.visibility = visibility;
        Ok(source)
    }

    /// Installs a personal view after complete source and native selection validation.
    /// Geometry is only reprojected and masked; accepted authority is never solved.
    ///
    /// # Errors
    /// Rejects stale views, foreign occurrences, dimensions, rows or invalid cameras atomically.
    pub fn update(&mut self, state: DetachedInteractionState) -> Result<(), EngineError> {
        if state.format != "geosolve-local-interaction-v1" || state.scene_key != self.source_key {
            return Err(error("browsing view belongs to an obsolete accepted model"));
        }
        camera(state.viewport).map_err(error)?;
        let display_mode = mode(&state.dimension_mode).map_err(error)?;
        if state.selection.len() > 4096
            || state.dimension_pins.len() > DimensionPresentationState::MAX_PINS
        {
            return Err(error("browsing selection or pins exceed their bound"));
        }
        self.visibility.validate(&state.visibility).map_err(error)?;
        let source_selection = SelectionPresentationState {
            items: state.selection.clone(),
            curve_picks: state.curve_picks.clone(),
        };
        source_selection
            .validate(&self.source_scene)
            .map_err(error)?;
        let selection = SelectionPresentationState {
            items: source_selection
                .items
                .into_iter()
                .map(|item| self.mapping.selection(item))
                .collect::<Result<_, _>>()
                .map_err(error)?,
            curve_picks: source_selection
                .curve_picks
                .into_iter()
                .map(|pick| self.mapping.curve_pick(pick, &self.native_full_scene))
                .collect::<Result<_, _>>()
                .map_err(error)?,
        };
        selection.validate(&self.native_full_scene).map_err(error)?;
        let mut dimensions = self.dimensions.clone();
        let dimension = |id: &str| {
            dimensions
                .ids
                .get(id)
                .copied()
                .ok_or_else(|| error("browsing dimension belongs to another scene"))
        };
        let pins = state
            .dimension_pins
            .iter()
            .map(|id| dimension(id))
            .collect::<Result<_, _>>()?;
        let focus = state
            .dimension_focus
            .as_deref()
            .map(dimension)
            .transpose()?;
        dimensions.state.mode = display_mode;
        dimensions.state.pins = pins;
        dimensions.state.focus = focus;
        let mut scene = self.native_full_scene.clone();
        scene.reproject_viewport(state.viewport).map_err(error)?;
        self.visibility
            .mask_prediction(&mut scene, &state.visibility, &self.mapping)
            .map_err(error)?;
        let mut policy = self.policy;
        policy.visibility.explicit_construction = state.visibility.construction_visible;
        policy.visibility.implicit_construction = state.visibility.construction_visible;
        self.session
            .restore_presentation(&self.native_full_scene, selection, policy)?;
        dimensions.context.selection = self.session.editor().editor().selection().to_vec();
        if let Some(owner) = self.session.editor().selected_declaration() {
            dimensions
                .context
                .selection
                .extend(self.session.editor().navigation_selection_items([owner]));
        }
        let entries = dimensions
            .state
            .apply(&mut scene, &dimensions.layout, &dimensions.context);
        self.scene = scene;
        self.dimensions = dimensions;
        self.dimension_entries = entries;
        self.state = Some(state);
        Ok(())
    }

    /// Installs the exact personal result of source/row navigation. Authored state
    /// remains fixed; a logical owner is meaningful even for an empty declaration.
    ///
    /// # Errors
    /// Rejects foreign owners, unavailable native selections and stale occurrences.
    pub fn restore_navigation(
        &mut self,
        selection: SelectionPresentationState,
        logical_owner: Option<geosolve_sketch_intent::NodeId>,
    ) -> Result<(), EngineError> {
        if self.state.is_none() {
            return Err(error("accepted browsing state unavailable"));
        }
        selection.validate(&self.native_full_scene).map_err(error)?;
        let source_selection = SelectionPresentationState {
            items: selection
                .items
                .iter()
                .map(|item| self.reverse.selection(*item))
                .collect::<Result<_, _>>()
                .map_err(error)?,
            curve_picks: selection
                .curve_picks
                .iter()
                .map(|pick| self.reverse.curve_pick(*pick, &self.source_scene))
                .collect::<Result<_, _>>()
                .map_err(error)?,
        };
        source_selection
            .validate(&self.source_scene)
            .map_err(error)?;
        self.session
            .presentation
            .editor
            .restore_navigation_selection(&self.native_full_scene, selection, logical_owner)
            .map_err(error)?;
        let state = self
            .state
            .as_mut()
            .ok_or_else(|| error("accepted browsing state unavailable"))?;
        state.selection = source_selection.items;
        state.curve_picks = source_selection.curve_picks;
        self.dimensions.context.selection = self.session.editor().editor().selection().to_vec();
        if let Some(owner) = self.session.editor().selected_declaration() {
            self.dimensions
                .context
                .selection
                .extend(self.session.editor().navigation_selection_items([owner]));
        }
        self.dimension_entries = self.dimensions.state.apply(
            &mut self.scene,
            &self.dimensions.layout,
            &self.dimensions.context,
        );
        Ok(())
    }

    /// Maps native selection back into the caller's exact accepted scene.
    ///
    /// # Errors
    /// Rejects invalid or unavailable source correspondence.
    pub fn source_selection(&self) -> Result<SelectionPresentationState, EngineError> {
        let native = self
            .session
            .editor()
            .editor()
            .selection_presentation_state();
        let selection = SelectionPresentationState {
            items: native
                .items
                .into_iter()
                .map(|item| self.reverse.selection(item))
                .collect::<Result<_, _>>()
                .map_err(error)?,
            curve_picks: native
                .curve_picks
                .into_iter()
                .map(|pick| self.reverse.curve_pick(pick, &self.source_scene))
                .collect::<Result<_, _>>()
                .map_err(error)?,
        };
        selection.validate(&self.source_scene).map_err(error)?;
        Ok(selection)
    }
}
