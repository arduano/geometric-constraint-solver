// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional M84 code-project composition for the demonstration workbench.
//!
//! This module deliberately owns presentation and caller-facing project
//! composition only. Managed parsing, artifact validation, semantic generated
//! identity, overrides and unified code history remain public
//! `geosolve-sketch-code` responsibilities. No source is evaluated in the
//! browser and this adapter contains no solver equations.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureEvaluationState,
    ComputedFeatureSnapshot, IntentNativeBinding, ProjectionalEditorSession, SelectionItem,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CodeProjectDemo, CodeProjectDemoId, CodeSessionReceipt,
    EditorBootstrapDeclaration, ExpandedCodeProject, ExpandedSemanticTarget, FeatureKind,
    GeneratedMemberAddress, KeyedReconcileState, ManagedDiagnostic, ManagedEdit, ManagedValue,
    MaterializedCodeProject, PatchModuleArtifact, ProjectKey, SemanticSymbol, SketchCodeSession,
    UnitLiteral, apply_managed_edit, bundled_code_project_demos, expand_code_project,
    initialize_code_project_from_editor, materialize_code_project_cold,
    materialize_code_project_incremental, parse_managed_source, plan_managed_edit,
    rehydrate_materialized_code_project, required_generated_members,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentLiteral, IntentNodeKind, IntentPortRole, IntentPortSelector,
    IntentSession, IntentSessionId, IntentUnit, LeafField, LeafRef, NodeId,
};
use serde::{Deserialize, Serialize};

const MANAGED_FILE: &str = "sketch.ts";
const CODE_WORKBENCH_WIRE_VERSION: &str = "geosolve-code-workbench-v1";
const CODE_PROJECT_MODEL_SCALE: f64 = 1.0;
static NEXT_CODE_MATERIALIZATION: AtomicU64 = AtomicU64::new(1);

/// One independently accepted replacement for the live projectional canvas.
/// The caller installs `editor` only after the code-session transaction has
/// published the matching opaque checkpoint.
pub(crate) struct AcceptedCodePublication {
    pub(crate) editor: Box<ProjectionalEditorSession>,
    pub(crate) receipt: CodeSessionReceipt,
}

/// A syntactically valid Apply either replaces native authority atomically or
/// retains its code diagnostic over the previous accepted editor checkpoint.
pub(crate) enum CodeApplyOutcome {
    Accepted(AcceptedCodePublication),
    RetainedFailure {
        receipt: CodeSessionReceipt,
        diagnostic: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum CodeOwnedEditorChange {
    GeneratedPoint {
        address: GeneratedMemberAddress,
        position: [f64; 2],
    },
    ManagedRectangle {
        declaration: SemanticSymbol,
        alias: geosolve_sketch_intent::IntentKey,
        changed_arguments: BTreeSet<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WritableCodeLeaf {
    GeneratedPoint(GeneratedMemberAddress),
    ManagedRectangle {
        declaration: SemanticSymbol,
        alias: geosolve_sketch_intent::IntentKey,
        argument: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelectedCodeFile {
    Managed,
    Custom(String),
}

impl SelectedCodeFile {
    fn path(&self) -> &str {
        match self {
            Self::Managed => MANAGED_FILE,
            Self::Custom(path) => path,
        }
    }
}

/// One genuine optional code-project session composed beside the ordinary
/// projectional editor. The project and code session are authoritative; the
/// selected file and invalid text draft are presentation state only.
pub(crate) struct CodeProjectWorkbench {
    origin: CodeProjectOrigin,
    project: CodeProject,
    session: SketchCodeSession,
    selected_file: SelectedCodeFile,
    managed_draft: String,
    draft_diagnostic: Option<ManagedDiagnostic>,
    last_receipt: Option<CodeSessionReceipt>,
    // Reconstructible warm authority. Persistence stores the delegated editor
    // checkpoint plus authenticated expansion, never this runtime cache.
    materialized: Option<Box<MaterializedCodeProject>>,
}

/// Presentation provenance for one genuine code project. Bundled projects
/// retain their curated sample identity; GUI-promoted projects deliberately
/// have no fake sample key.
#[derive(Clone, Debug)]
enum CodeProjectOrigin {
    Bundled(CodeProjectDemo),
    Promoted,
}

impl CodeProjectOrigin {
    fn title(&self) -> &'static str {
        match self {
            Self::Bundled(demo) => demo.title,
            Self::Promoted => "Promoted sketch",
        }
    }

    fn demo_key(&self) -> Option<&'static str> {
        match self {
            Self::Bundled(demo) => Some(demo.id.key()),
            Self::Promoted => None,
        }
    }

    fn to_wire(&self) -> CodeProjectOriginWire {
        match self {
            Self::Bundled(demo) => CodeProjectOriginWire::Bundled {
                demo: demo.id.key().into(),
            },
            Self::Promoted => CodeProjectOriginWire::Promoted,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CodeProjectOriginWire {
    Bundled { demo: String },
    Promoted,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectWorkbenchWire {
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<CodeProjectOriginWire>,
    /// Historical nominated-M84 wire accepted only on input. New persistence
    /// always writes the explicit bounded `origin` variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    demo: Option<String>,
    project: String,
    session: String,
    selected_file: String,
    managed_draft: String,
}

fn restore_code_project_origin(
    origin: Option<CodeProjectOriginWire>,
    legacy_demo: Option<&str>,
) -> Result<CodeProjectOrigin, String> {
    let bundled = |key: &str| {
        bundled_code_project_demos()
            .into_iter()
            .find(|demo| demo.id.key() == key)
            .map(CodeProjectOrigin::Bundled)
            .ok_or_else(|| format!("unknown code project `{key}`"))
    };
    match (origin, legacy_demo) {
        (Some(CodeProjectOriginWire::Bundled { demo }), None) => bundled(&demo),
        (Some(CodeProjectOriginWire::Promoted), None) => Ok(CodeProjectOrigin::Promoted),
        (None, Some(demo)) => bundled(demo),
        (None, None) => Err("code-project persistence has no origin".into()),
        (Some(CodeProjectOriginWire::Bundled { .. }), Some(_)) => {
            Err("bundled code-project origin is duplicated".into())
        }
        (Some(CodeProjectOriginWire::Promoted), Some(_)) => {
            Err("promoted code-project origin cannot name a bundled demo".into())
        }
    }
}

impl CodeProjectWorkbench {
    pub(crate) fn open_key(key: &str) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let demo = bundled_code_project_demos()
            .into_iter()
            .find(|demo| demo.id.key() == key)
            .ok_or_else(|| format!("unknown code project `{key}`"))?;
        let project = demo.project();
        Self::open_project(CodeProjectOrigin::Bundled(demo), project)
    }

    /// Builds a genuine optional code project from every declaration in one
    /// accepted ordinary workspace. This is intentionally all-or-nothing: a
    /// successful promotion cannot silently discard unsupported ordinary
    /// declarations or retain a second hidden GUI authority.
    pub(crate) fn promote_from_editor(
        editor: &ProjectionalEditorSession,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let project = projected_code_project(editor)?;
        Self::open_project(CodeProjectOrigin::Promoted, project)
    }

    fn open_project(
        origin: CodeProjectOrigin,
        project: CodeProject,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        project.validate().map_err(|error| error.to_string())?;
        let desired = required_generated_members(&project).map_err(|error| error.to_string())?;
        let plan = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .map_err(|error| error.to_string())?;
        let materialized = materialize_candidate(&project, plan.staged())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let materialized = rehydrate_materialized_code_project(
            restore_editor_checkpoint(&checkpoint)?,
            expansion.clone(),
        )
        .map_err(|error| error.to_string())?;
        let session = SketchCodeSession::new_project(
            project.clone(),
            plan.into_staged(),
            expansion,
            checkpoint,
        )
        .map_err(|error| error.to_string())?;
        let managed_draft = project.managed.source.clone();
        Ok((
            Self {
                origin,
                project,
                session,
                selected_file: SelectedCodeFile::Managed,
                managed_draft,
                draft_diagnostic: None,
                last_receipt: None,
                materialized: Some(materialized),
            },
            delegated_editor,
        ))
    }

    pub(crate) fn to_persistence_json(&self) -> Result<String, String> {
        let wire = CodeProjectWorkbenchWire {
            version: CODE_WORKBENCH_WIRE_VERSION.into(),
            origin: Some(self.origin.to_wire()),
            demo: None,
            project: self
                .project
                .to_canonical_json()
                .map_err(|error| error.to_string())?,
            session: self
                .session
                .to_canonical_json()
                .map_err(|error| error.to_string())?,
            selected_file: self.selected_file.path().into(),
            managed_draft: self.managed_draft.clone(),
        };
        let json = serde_json::to_string(&wire).map_err(|error| error.to_string())?;
        if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
            return Err(format!(
                "code workbench is {} bytes; the limit is {}",
                json.len(),
                geosolve_sketch_code::CODE_PROJECT_LIMIT,
            ));
        }
        Ok(json)
    }

    pub(crate) fn from_persistence_json(json: &str) -> Result<Self, String> {
        if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
            return Err(format!(
                "code workbench is {} bytes; the limit is {}",
                json.len(),
                geosolve_sketch_code::CODE_PROJECT_LIMIT,
            ));
        }
        let wire: CodeProjectWorkbenchWire =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if wire.version != CODE_WORKBENCH_WIRE_VERSION {
            return Err("unsupported code-workbench version".into());
        }
        let origin = restore_code_project_origin(wire.origin, wire.demo.as_deref())?;
        let project = CodeProject::from_json(&wire.project).map_err(|error| error.to_string())?;
        let session = SketchCodeSession::from_json_validating_checkpoints(
            &wire.session,
            validate_editor_checkpoint,
        )
        .map_err(|error| error.to_string())?;
        if session.snapshot().project != project.project
            || session.snapshot().managed != project.managed
            || session.snapshot().code_project.as_ref() != Some(&project)
            || session.snapshot().artifact_digests != artifact_digests(&project)?
        {
            return Err("code project and unified session checkpoints disagree".into());
        }
        let snapshot = session.snapshot();
        let accepted_project = snapshot
            .accepted_code_project
            .as_ref()
            .ok_or_else(|| "code session has no accepted project authority".to_owned())?;
        let accepted_generated = snapshot
            .accepted_generated
            .as_ref()
            .ok_or_else(|| "code session has no accepted generated authority".to_owned())?;
        validate_generated_members(accepted_project, accepted_generated, "accepted")?;
        match required_generated_members(&project) {
            Ok(desired) => {
                let actual = snapshot
                    .generated
                    .ordered_members()
                    .into_iter()
                    .map(|member| member.address)
                    .collect::<Vec<_>>();
                if actual != desired {
                    return Err(
                        "current code-session generated provenance does not match managed source"
                            .into(),
                    );
                }
            }
            Err(error) if snapshot.failure.is_some() && snapshot.expansion.is_none() => {
                if snapshot
                    .failure
                    .as_ref()
                    .is_none_or(|failure| failure.diagnostic != error.to_string())
                {
                    return Err(
                        "retained structural diagnostic does not authenticate managed source"
                            .into(),
                    );
                }
            }
            Err(error) => return Err(error.to_string()),
        }
        validate_editor_checkpoint(session.pointer_frame_checkpoint())?;
        let draft_diagnostic = if wire.managed_draft == project.managed.source {
            None
        } else {
            parse_managed_source(&wire.managed_draft)
                .err()
                .map(|error| error.diagnostic)
        };
        let accepted_expansion = snapshot
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code session has no accepted expansion authority".to_owned())?;
        let materialized = rehydrate_materialized_code_project(
            restore_editor_checkpoint(session.pointer_frame_checkpoint())?,
            accepted_expansion,
        )
        .map_err(|error| error.to_string())?;
        let mut value = Self {
            origin,
            project,
            session,
            selected_file: SelectedCodeFile::Managed,
            managed_draft: wire.managed_draft,
            draft_diagnostic,
            last_receipt: None,
            materialized: Some(materialized),
        };
        value.select_file(&wire.selected_file)?;
        Ok(value)
    }

    pub(crate) fn accepted_editor_checkpoint(&self) -> &serde_json::Value {
        &self.session.snapshot().accepted_editor_checkpoint
    }

    fn ensure_materialized_cache(&mut self) -> Result<(), String> {
        if self.materialized.is_some() {
            return Ok(());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        self.materialized = Some(rehydrate_editor_checkpoint(
            self.session.pointer_frame_checkpoint(),
            expansion,
        )?);
        Ok(())
    }

    /// Captures one already accepted direct GUI mutation in the outer code
    /// history. Identical checkpoints are presentation-only and create no
    /// duplicate Undo entry.
    pub(crate) fn publish_delegated_editor_checkpoint(
        &mut self,
        editor_checkpoint: serde_json::Value,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if &editor_checkpoint == self.session.pointer_frame_checkpoint() {
            return Ok(None);
        }
        let candidate_editor = restore_editor_checkpoint(&editor_checkpoint)?;
        let accepted_editor = restore_editor_checkpoint(self.session.pointer_frame_checkpoint())?;
        let code_change = classify_code_owned_editor_change(
            &accepted_editor,
            &candidate_editor,
            &self.project,
            self.session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?,
        )?;
        if let Some(change) = code_change {
            if self.session.snapshot().failure.is_some() {
                return Err(
                    "resolve or Undo the retained code failure before editing code-owned geometry"
                        .into(),
                );
            }
            return match change {
                CodeOwnedEditorChange::GeneratedPoint { address, position } => self
                    .publish_generated_point_override(&address, position, &candidate_editor, label),
                CodeOwnedEditorChange::ManagedRectangle {
                    declaration,
                    alias,
                    changed_arguments,
                } => self.publish_managed_rectangle_placement(
                    &declaration,
                    &alias,
                    &changed_arguments,
                    &candidate_editor,
                    label,
                ),
            };
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let candidate_cache = rehydrate_editor_checkpoint(&editor_checkpoint, expansion)?;
        let prepared = self
            .session
            .prepare_delegated_editor_publication(self.session.identity(), editor_checkpoint, label)
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: candidate_editor,
            receipt,
        }))
    }

    /// Code-owned points without an explicit generated-placement lens stay
    /// read-only in the demo. Ordinary GUI-authored points remain editable.
    pub(crate) fn point_drag_permission(
        &self,
        editor: &ProjectionalEditorSession,
        point: geosolve_sketch::DesignPointId,
    ) -> Result<(), String> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
        let Some(owner) = accepted
            .ownership
            .exact_owner(IntentNativeBinding::Point(point))
        else {
            return Ok(());
        };
        let Some(node) = editor.coordinator().intent().graph().node(owner) else {
            return Err("point owner is absent from accepted intent".into());
        };
        if !node.symbol.as_str().starts_with("code.") {
            return Ok(());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let supported = expansion
            .generated_provenance
            .iter()
            .any(|(address, provenance)| {
                supported_point_override_address(address)
                    && expanded_target_point(editor, &provenance.target) == Some(point)
            })
            || managed_rectangle_points(editor, &self.project, expansion)?.contains_key(&point);
        if supported && self.session.snapshot().failure.is_none() {
            Ok(())
        } else if supported {
            Err(
                "resolve or Undo the retained code failure before dragging code-owned geometry"
                    .into(),
            )
        } else {
            Err(
                "this code-owned point is read-only on canvas; edit its managed source or an exposed lens"
                    .into(),
            )
        }
    }

    /// Returns a truthful read-only diagnostic for unsupported direct
    /// manipulation of code-owned curves/features. Annotation placement and
    /// ordinary GUI-owned geometry are intentionally outside this guard.
    pub(crate) fn selected_code_geometry_mutation_permission(
        editor: &ProjectionalEditorSession,
    ) -> Result<(), String> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
        for item in editor.editor().selection() {
            let bindings = match item {
                SelectionItem::Point(point) => vec![IntentNativeBinding::Point(*point)],
                SelectionItem::Curve(span) => vec![
                    IntentNativeBinding::CurveSpan(*span),
                    IntentNativeBinding::Curve(span.curve),
                ],
                SelectionItem::Constraint(constraint) => {
                    vec![IntentNativeBinding::Constraint(*constraint)]
                }
                SelectionItem::Dimension(dimension) => {
                    vec![IntentNativeBinding::Dimension(*dimension)]
                }
                SelectionItem::Feature(feature) => {
                    vec![IntentNativeBinding::ComputedFeature(*feature)]
                }
                SelectionItem::FeatureCorner(corner) => vec![
                    IntentNativeBinding::ComputedFeatureCorner(corner.corner),
                    IntentNativeBinding::ComputedFeature(corner.feature),
                ],
                SelectionItem::Datum(_) => Vec::new(),
            };
            if bindings.into_iter().any(|binding| {
                accepted
                    .ownership
                    .exact_owner(binding)
                    .and_then(|node| editor.coordinator().intent().graph().node(node))
                    .is_some_and(|node| node.symbol.as_str().starts_with("code."))
            }) {
                return Err(
                    "this code-owned property is read-only on canvas; edit managed source or an exposed lens"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn restore_accepted_editor(&self) -> Result<Box<ProjectionalEditorSession>, String> {
        restore_editor_checkpoint(self.session.pointer_frame_checkpoint())
    }

    fn publish_generated_point_override(
        &mut self,
        address: &GeneratedMemberAddress,
        position: [f64; 2],
        candidate_editor: &ProjectionalEditorSession,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let value = ManagedValue::Array(vec![
            ManagedValue::Number(position[0]),
            ManagedValue::Number(position[1]),
        ]);
        let mut generated = self.session.snapshot().generated.clone();
        generated
            .set_override(address, value.clone())
            .map_err(|error| error.to_string())?;

        // The live projectional editor has already reconstructed its accepted
        // terminal pointer sample. Independently replay the override over the
        // warm accepted composition so ordinary GUI dependents participate in
        // the same native validation without being reallocated or erased.
        self.ensure_materialized_cache()?;
        let parity = materialize_code_project_incremental(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &generated,
        )
        .map_err(|error| error.to_string())?;
        let parity_point = generated_point_position(&parity.editor, &parity.expansion, address)
            .ok_or_else(|| {
                "generated point override has no cold native parity target".to_owned()
            })?;
        let candidate_point = generated_point_position(
            candidate_editor,
            self.session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?,
            address,
        )
        .ok_or_else(|| "generated point override has no live native target".to_owned())?;
        if pair_bits(parity_point) != pair_bits(candidate_point)
            || pair_bits(candidate_point) != pair_bits(position)
        {
            return Err("generated point override failed exact cold/native parity".into());
        }
        validate_terminal_native_parity(candidate_editor, &parity.editor, &parity.expansion)?;
        let expansion = parity.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&parity.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_override(
                self.session.identity(),
                address,
                value,
                expansion,
                checkpoint,
                label,
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one atomic reverse-edit protocol keeps source, warm materialization, parity, and publication reviewable together"
    )]
    fn publish_managed_rectangle_placement(
        &mut self,
        declaration: &SemanticSymbol,
        alias: &geosolve_sketch_intent::IntentKey,
        changed_arguments: &BTreeSet<String>,
        candidate_editor: &ProjectionalEditorSession,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before dragging code-owned geometry"
                    .into(),
            );
        }
        let mut managed = self.project.managed.clone();
        for argument in changed_arguments {
            let position = managed_rectangle_argument_position(candidate_editor, alias, argument)
                .ok_or_else(|| {
                format!(
                    "managed rectangle `{}` has no exact `{argument}` Cartesian placement",
                    declaration.0
                )
            })?;
            if !position[0].is_finite() || !position[1].is_finite() {
                return Err("managed rectangle placement is not finite".into());
            }
            let current = managed
                .program
                .declarations
                .iter()
                .find(|candidate| &candidate.symbol == declaration)
                .and_then(|candidate| {
                    managed_object_path(&candidate.arguments, std::slice::from_ref(argument))
                })
                .ok_or_else(|| {
                    format!(
                        "managed rectangle `{}` no longer owns `{argument}`",
                        declaration.0
                    )
                })?;
            let value = managed_point_value_like(current, position)?;
            let plan = plan_managed_edit(
                &managed,
                ManagedEdit::SetInvocationArgument {
                    declaration: declaration.clone(),
                    path: vec![argument.clone()],
                    value,
                },
            )
            .map_err(|error| error.to_string())?;
            managed = apply_managed_edit(&managed, &plan).map_err(|error| error.to_string())?;
        }

        let mut candidate_project = self.project.clone();
        candidate_project.managed = managed;
        candidate_project
            .validate()
            .map_err(|error| error.to_string())?;
        if candidate_project.custom_files != self.project.custom_files
            || candidate_project.artifacts != self.project.artifacts
        {
            return Err("managed placement attempted to change custom project authority".into());
        }
        let desired =
            required_generated_members(&candidate_project).map_err(|error| error.to_string())?;
        let plan = self
            .session
            .plan_structural_reconciliation(self.session.identity(), desired, &BTreeSet::new())
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &candidate_project,
            plan.staged(),
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let staged_alias = managed_rectangle_alias(&candidate_project, &expansion, declaration)
            .ok_or_else(|| {
                format!(
                    "managed rectangle `{}` disappeared during source expansion",
                    declaration.0
                )
            })?;
        for argument in changed_arguments {
            let live = managed_rectangle_argument_position(candidate_editor, alias, argument)
                .ok_or_else(|| "live managed rectangle placement disappeared".to_owned())?;
            let staged =
                managed_rectangle_argument_position(&materialized.editor, &staged_alias, argument)
                    .ok_or_else(|| "staged managed rectangle placement disappeared".to_owned())?;
            if pair_bits(live) != pair_bits(staged) {
                return Err(format!(
                    "managed rectangle `{}` `{argument}` drag failed exact source/native parity",
                    declaration.0
                ));
            }
        }
        validate_terminal_native_parity(
            candidate_editor,
            &materialized.editor,
            &materialized.expansion,
        )?;
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_edit_from_plan(
                self.session.identity(),
                candidate_project.clone(),
                plan,
                expansion,
                checkpoint,
                label,
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.project = candidate_project;
        self.managed_draft = self.session.snapshot().managed.source.clone();
        self.draft_diagnostic = None;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    pub(crate) fn demo_key(&self) -> Option<&'static str> {
        self.origin.demo_key()
    }

    pub(crate) fn select_file(&mut self, path: &str) -> Result<(), String> {
        if path == MANAGED_FILE {
            self.selected_file = SelectedCodeFile::Managed;
            return Ok(());
        }
        if self.project.custom_files.contains_key(path) {
            self.selected_file = SelectedCodeFile::Custom(path.to_owned());
            return Ok(());
        }
        Err(format!("code-project file `{path}` is unavailable"))
    }

    pub(crate) fn set_managed_draft(&mut self, draft: String) {
        self.managed_draft = draft;
        // A prior diagnostic authenticates different draft bytes and must not
        // continue to claim line ownership while the user edits.
        self.draft_diagnostic = None;
    }

    pub(crate) fn apply_managed_draft(&mut self) -> Result<CodeApplyOutcome, String> {
        let managed = match parse_managed_source(&self.managed_draft) {
            Ok(managed) => managed,
            Err(error) => {
                self.draft_diagnostic = Some(error.diagnostic.clone());
                return Err(error.to_string());
            }
        };
        let mut candidate_project = self.project.clone();
        candidate_project.managed = managed;
        candidate_project
            .validate()
            .map_err(|error| error.to_string())?;
        let desired_members = match required_generated_members(&candidate_project) {
            Ok(members) => members,
            Err(error) => {
                return self.retain_candidate_failure(
                    candidate_project,
                    None,
                    None,
                    "structural expansion",
                    error.to_string(),
                );
            }
        };
        let plan = match self.session.plan_structural_reconciliation(
            self.session.identity(),
            desired_members,
            &BTreeSet::new(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return self.retain_candidate_failure(
                    candidate_project,
                    None,
                    None,
                    "keyed reconciliation",
                    error.to_string(),
                );
            }
        };
        self.ensure_materialized_cache()?;
        match materialize_code_project_incremental(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &candidate_project,
            plan.staged(),
        )
        .map_err(|error| error.to_string())
        {
            Ok(materialized) => {
                let expansion = materialized.expansion.clone();
                let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
                let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
                let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
                let prepared = self
                    .session
                    .prepare_project_edit_from_plan(
                        self.session.identity(),
                        candidate_project.clone(),
                        plan,
                        expansion,
                        checkpoint,
                        "Apply managed source",
                    )
                    .map_err(|error| error.to_string())?;
                let receipt = self
                    .session
                    .apply_prepared(prepared)
                    .map_err(|error| error.to_string())?;
                self.project = candidate_project;
                self.managed_draft = self.session.snapshot().managed.source.clone();
                self.draft_diagnostic = None;
                self.materialized = Some(candidate_cache);
                self.last_receipt = Some(receipt.clone());
                Ok(CodeApplyOutcome::Accepted(AcceptedCodePublication {
                    editor: delegated_editor,
                    receipt,
                }))
            }
            Err(diagnostic) => {
                let retained_expansion =
                    expansion_for_retained_failure(&candidate_project, plan.staged());
                self.retain_candidate_failure(
                    candidate_project,
                    Some(plan),
                    retained_expansion,
                    "native materialization",
                    diagnostic,
                )
            }
        }
    }

    /// Applies one artifact-declared scalar edit lens through the exact same
    /// managed-source/native transaction as an explicit source Apply.
    pub(crate) fn apply_scalar_lens(
        &mut self,
        declaration: &str,
        path: &str,
        value: f64,
    ) -> Result<CodeApplyOutcome, String> {
        if !value.is_finite() {
            return Err("edit-lens value must be finite".into());
        }
        let path = path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if path.is_empty() {
            return Err("edit-lens argument path is empty".into());
        }
        let declaration = SemanticSymbol(declaration.to_owned());
        let current = self
            .project
            .managed
            .program
            .declarations
            .iter()
            .find(|candidate| candidate.symbol == declaration)
            .and_then(|candidate| managed_object_path(&candidate.arguments, &path))
            .ok_or_else(|| "edit-lens target is unavailable in managed source".to_owned())?;
        let replacement = match current {
            ManagedValue::Unit(unit) => ManagedValue::Unit(UnitLiteral {
                unit: unit.unit.clone(),
                value,
            }),
            ManagedValue::Number(_) => ManagedValue::Number(value),
            _ => return Err("this edit lens does not target a scalar literal".into()),
        };
        let plan = plan_managed_edit(
            &self.project.managed,
            ManagedEdit::SetInvocationArgument {
                declaration,
                path,
                value: replacement,
            },
        )
        .map_err(|error| error.to_string())?;
        let managed =
            apply_managed_edit(&self.project.managed, &plan).map_err(|error| error.to_string())?;
        self.set_managed_draft(managed.source);
        self.apply_managed_draft()
    }

    fn retain_candidate_failure(
        &mut self,
        candidate_project: CodeProject,
        plan: Option<geosolve_sketch_code::KeyedReconcilePlan>,
        expansion: Option<geosolve_sketch_code::ExpandedCodeProject>,
        stage: &str,
        diagnostic: String,
    ) -> Result<CodeApplyOutcome, String> {
        let prepared = self
            .session
            .prepare_project_retained_failure(
                self.session.identity(),
                candidate_project.clone(),
                plan,
                expansion,
                self.session.snapshot().accepted_editor_checkpoint.clone(),
                stage,
                diagnostic.clone(),
                "Apply managed source (retained failure)",
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.project = candidate_project;
        self.managed_draft = self.session.snapshot().managed.source.clone();
        self.draft_diagnostic = None;
        self.last_receipt = Some(receipt.clone());
        Ok(CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        })
    }

    pub(crate) fn revert_managed_draft(&mut self) -> bool {
        let changed = self.managed_draft != self.session.snapshot().managed.source
            || self.draft_diagnostic.is_some();
        self.managed_draft = self.session.snapshot().managed.source.clone();
        self.draft_diagnostic = None;
        changed
    }

    #[cfg(test)]
    pub(crate) fn apply_override(
        &mut self,
        address: &GeneratedMemberAddress,
        value: ManagedValue,
    ) -> Result<AcceptedCodePublication, String> {
        let mut generated = self.session.snapshot().generated.clone();
        generated
            .set_override(address, value.clone())
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &generated,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_override(
                self.session.identity(),
                address,
                value,
                expansion,
                checkpoint,
                "Place generated override",
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        })
    }

    pub(crate) fn reset_override(
        &mut self,
        display_path: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let address = self
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| address.display_path() == display_path)
            .cloned()
            .ok_or_else(|| format!("generated member `{display_path}` is unavailable"))?;
        if self
            .session
            .snapshot()
            .generated
            .override_for(&address)
            .is_none()
        {
            return Ok(None);
        }
        let mut generated = self.session.snapshot().generated.clone();
        generated
            .reset_to_code(&address)
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &generated,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_reset_to_code(
                self.session.identity(),
                &address,
                expansion,
                checkpoint,
                "Reset generated override",
            )
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "generated override disappeared before publication".to_owned())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    pub(crate) fn step_history(
        &mut self,
        undo: bool,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if (undo && !self.session.can_undo()) || (!undo && !self.session.can_redo()) {
            return Ok(None);
        }
        // Restore and independently validate the nested authority before
        // replacing the live code session, so corrupt opaque checkpoint bytes
        // cannot leave history half-stepped.
        let mut candidate_session = self.session.clone();
        let receipt = if undo {
            candidate_session.undo()
        } else {
            candidate_session.redo()
        }
        .map_err(|error| error.to_string())?;
        let Some(receipt) = receipt else {
            return Ok(None);
        };
        let editor = restore_editor_checkpoint(candidate_session.pointer_frame_checkpoint())?;
        let expansion = candidate_session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code history restored no accepted expansion authority".to_owned())?;
        let candidate_cache = rehydrate_materialized_code_project(
            restore_editor_checkpoint(candidate_session.pointer_frame_checkpoint())?,
            expansion,
        )
        .map_err(|error| error.to_string())?;
        let project = candidate_session
            .snapshot()
            .code_project
            .clone()
            .ok_or_else(|| "code history restored incomplete project authority".to_owned())?;
        self.session = candidate_session;
        self.project = project;
        self.managed_draft = self.project.managed.source.clone();
        self.draft_diagnostic = None;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication { editor, receipt }))
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.managed_draft != self.session.snapshot().managed.source
    }

    #[cfg(test)]
    pub(crate) fn managed_source(&self) -> &str {
        &self.session.snapshot().managed.source
    }

    pub(crate) fn panel_markup(&self) -> String {
        let mut markup = String::new();
        markup.push_str("<div class=\"wb-code-project\">");
        self.write_project_header(&mut markup);
        self.write_file_tabs(&mut markup);
        self.write_source_surface(&mut markup);
        self.write_artifact_status(&mut markup);
        self.write_edit_lenses(&mut markup);
        self.write_generated_members(&mut markup);
        markup.push_str("</div>");
        markup
    }

    fn write_project_header(&self, markup: &mut String) {
        let revision = self.session.identity().revision;
        let dirty = if self.is_dirty() {
            " · unsaved draft"
        } else {
            ""
        };
        let _ = write!(
            markup,
            concat!(
                "<header class=\"wb-code-project-header\"><div>",
                "<span class=\"wb-code-eyebrow\">Code project</span>",
                "<strong>{}</strong><small>managed-v1 · revision {}{}</small>",
                "</div><span class=\"wb-code-runtime-badge\">Rust runtime · data only</span></header>"
            ),
            escape_html(self.origin.title()),
            revision,
            dirty,
        );
    }

    fn write_file_tabs(&self, markup: &mut String) {
        markup.push_str(
            "<div class=\"wb-code-file-tabs\" role=\"tablist\" aria-label=\"Code project files\">",
        );
        write_file_tab(
            markup,
            MANAGED_FILE,
            "Managed",
            matches!(self.selected_file, SelectedCodeFile::Managed),
        );
        for path in self.project.custom_files.keys() {
            write_file_tab(
                markup,
                path,
                "Custom · read-only",
                self.selected_file.path() == path,
            );
        }
        markup.push_str("</div>");
    }

    fn write_source_surface(&self, markup: &mut String) {
        match &self.selected_file {
            SelectedCodeFile::Managed => {
                let _ = write!(
                    markup,
                    concat!(
                        "<section class=\"wb-code-editor\" data-code-file-kind=\"managed\">",
                        "<div class=\"wb-code-editor-toolbar\"><div><strong>sketch.ts</strong>",
                        "<span>GUI-managed subset</span></div><div>",
                        "<button type=\"button\" data-code-action=\"revert\"{}>Revert</button>",
                        "<button type=\"button\" data-code-action=\"apply\"{}>Apply</button>",
                        "</div></div>",
                        "<textarea id=\"wb-code-managed-source\" spellcheck=\"false\" ",
                        "autocomplete=\"off\" autocapitalize=\"off\" aria-label=\"Managed sketch TypeScript\">{}</textarea>"
                    ),
                    if self.is_dirty() || self.draft_diagnostic.is_some() {
                        ""
                    } else {
                        " disabled"
                    },
                    if self.is_dirty() { "" } else { " disabled" },
                    escape_html(&self.managed_draft),
                );
                if let Some(diagnostic) = &self.draft_diagnostic {
                    let _ = write!(
                        markup,
                        concat!(
                            "<div class=\"wb-code-diagnostic\" role=\"alert\" data-line=\"{}\">",
                            "<strong>Line {}, column {}</strong><span>{}</span></div>"
                        ),
                        diagnostic.line,
                        diagnostic.line,
                        diagnostic.column,
                        escape_html(&diagnostic.message),
                    );
                } else {
                    markup.push_str(
                        "<p class=\"wb-code-editor-note\">Apply reparses and validates the complete candidate before one publication. Comments and unowned formatting stay byte-identical.</p>",
                    );
                }
                markup.push_str("</section>");
            }
            SelectedCodeFile::Custom(path) => {
                let file = &self.project.custom_files[path];
                let _ = write!(
                    markup,
                    concat!(
                        "<section class=\"wb-code-editor\" data-code-file-kind=\"custom\">",
                        "<div class=\"wb-code-editor-toolbar\"><div><strong>{}</strong>",
                        "<span>User / AI owned</span></div><span class=\"wb-code-readonly\">Read-only in demo</span></div>",
                        "<pre tabindex=\"0\" aria-label=\"Read-only custom patch source\"><code>{}</code></pre>",
                        "<p class=\"wb-code-editor-note\">GeoSolve never rewrites or evaluates this file in Rust/WASM. The caller-owned Node build emits the pinned artifact below.</p>",
                        "</section>"
                    ),
                    escape_html(path),
                    escape_html(&file.contents),
                );
            }
        }
    }

    fn write_artifact_status(&self, markup: &mut String) {
        let artifact_count = self.project.artifacts.len();
        let lens_count = self
            .project
            .artifacts
            .values()
            .filter_map(|value| serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok())
            .map(|artifact| artifact.edit_lenses.len())
            .sum::<usize>();
        let _ = write!(
            markup,
            concat!(
                "<section class=\"wb-code-artifact-status\"><div>",
                "<span class=\"wb-code-status-dot\" aria-hidden=\"true\"></span>",
                "<div><strong>Artifacts ready</strong><small>{} pinned module{} · {} edit lens{}</small></div>",
                "</div><span>Offline · ABI v1</span></section>"
            ),
            artifact_count,
            if artifact_count == 1 { "" } else { "s" },
            lens_count,
            if lens_count == 1 { "" } else { "es" },
        );
    }

    fn write_edit_lenses(&self, markup: &mut String) {
        let lenses = self
            .project
            .artifacts
            .values()
            .filter_map(|value| serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok())
            .flat_map(|artifact| {
                let module = artifact.module_specifier;
                let export = artifact.export_name;
                artifact
                    .edit_lenses
                    .into_iter()
                    .map(move |lens| (module.clone(), export.clone(), lens))
            })
            .collect::<Vec<_>>();
        if lenses.is_empty() {
            return;
        }
        markup.push_str(
            "<section class=\"wb-code-lenses\"><header><strong>Edit lenses</strong><span>Generated values with managed-source controls</span></header>",
        );
        for (module, export, lens) in lenses {
            let output = managed_path_text(&lens.output.0);
            let argument = lens.invocation_argument.join(".");
            let scalar = self
                .project
                .managed
                .program
                .declarations
                .iter()
                .find(|declaration| {
                    declaration
                        .patch
                        .as_ref()
                        .is_some_and(|patch| patch.module_binding == export)
                })
                .and_then(|declaration| {
                    managed_object_path(&declaration.arguments, &lens.invocation_argument)
                        .and_then(|value| match value {
                            ManagedValue::Number(value) => Some((*value, None)),
                            ManagedValue::Unit(value) => {
                                Some((value.value, Some(value.unit.as_str())))
                            }
                            _ => None,
                        })
                        .map(|value| (declaration, value))
                });
            let _ = write!(
                markup,
                concat!(
                    "<div class=\"wb-code-lens\"><div><strong>{}</strong>",
                    "<small>{} → invocation.{}</small></div>"
                ),
                escape_html(&output),
                escape_html(&module),
                escape_html(&argument),
            );
            if let Some((declaration, (value, unit))) = scalar {
                let _ = write!(
                    markup,
                    concat!(
                        "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">Edit {}</span>",
                        "<input type=\"number\" step=\"any\" value=\"{}\" ",
                        "data-code-lens-declaration=\"{}\" data-code-lens-path=\"{}\" />",
                        "<small>{}</small></label>"
                    ),
                    escape_html(&output),
                    value,
                    escape_attribute(&declaration.symbol.0),
                    escape_attribute(&argument),
                    escape_html(unit.unwrap_or("value")),
                );
            } else {
                markup.push_str(
                    "<button type=\"button\" data-code-action=\"open-managed-lens\">Edit managed input</button>",
                );
            }
            markup.push_str("</div>");
        }
        markup.push_str("</section>");
    }

    fn write_generated_members(&self, markup: &mut String) {
        let members = self.session.snapshot().generated.ordered_members();
        let mut groups = BTreeMap::<&str, Vec<_>>::new();
        for member in &members {
            groups
                .entry(member.address.invocation.as_str())
                .or_default()
                .push(member);
        }
        markup.push_str("<section class=\"wb-code-generated\"><header><div><strong>Generated ownership</strong><span>Stable semantic keys, not wire IDs</span></div>");
        let _ = write!(
            markup,
            "<small>{} member{}</small></header>",
            members.len(),
            if members.len() == 1 { "" } else { "s" },
        );
        for (invocation, group) in groups {
            let _ = write!(
                markup,
                "<div class=\"wb-code-member-group\"><h3>{}</h3>",
                escape_html(invocation),
            );
            for member in group {
                let overridden = self
                    .session
                    .snapshot()
                    .generated
                    .override_for(&member.address)
                    .is_some();
                let path = member.address.display_path();
                let member_key = member.address.member_key.join(" / ");
                let _ = write!(
                    markup,
                    concat!(
                        "<div class=\"wb-code-member\" data-code-ownership=\"{}\">",
                        "<div><strong>{}</strong><small>{}</small></div>",
                        "<span class=\"wb-code-ownership-badge\">{}</span>"
                    ),
                    if overridden { "override" } else { "generated" },
                    escape_html(&member_key),
                    escape_html(&path),
                    if overridden { "Override" } else { "Code-owned" },
                );
                if overridden {
                    let _ = write!(
                        markup,
                        "<button type=\"button\" data-code-action=\"reset-override\" data-code-member=\"{}\">Reset to code</button>",
                        escape_attribute(&path),
                    );
                }
                markup.push_str("</div>");
            }
            markup.push_str("</div>");
        }
        if members.is_empty() {
            markup.push_str("<p class=\"wb-code-empty\">This project has no structurally generated members.</p>");
        }
        if let Some(receipt) = &self.last_receipt {
            let _ = write!(
                markup,
                "<p class=\"wb-code-history-note\">Latest unified action: <strong>{}</strong> · revision {}</p>",
                escape_html(&receipt.label),
                receipt.after.revision,
            );
        }
        markup.push_str("</section>");
    }
}

/// Read-only managed-v1 projection of one complete ordinary GUI workspace.
/// The project is deliberately rebuilt and revalidated when Promote is
/// clicked; these source bytes never become authority by being rendered.
pub(crate) struct OrdinaryCodePreview {
    source: String,
    declaration_count: usize,
}

impl OrdinaryCodePreview {
    pub(crate) fn from_editor(editor: &ProjectionalEditorSession) -> Result<Self, String> {
        let project = projected_code_project(editor)?;
        Ok(Self {
            source: project.managed.source,
            declaration_count: project.managed.program.declarations.len(),
        })
    }

    pub(crate) fn panel_markup(&self) -> String {
        format!(
            concat!(
                "<div class=\"wb-code-project wb-code-preview\">",
                "<header class=\"wb-code-project-header\"><div>",
                "<span class=\"wb-code-eyebrow\">Managed code preview</span>",
                "<strong>Complete ordinary sketch</strong><small>{} declaration{}</small>",
                "</div><span class=\"wb-code-runtime-badge\">Read-only · not authority</span></header>",
                "<section class=\"wb-code-editor\" data-code-file-kind=\"managed-preview\">",
                "<div class=\"wb-code-editor-toolbar\"><div><strong>sketch.ts</strong>",
                "<span>Lexical, typed feature references</span></div>",
                "<button type=\"button\" data-code-action=\"promote-ordinary\">Promote to code project</button></div>",
                "<pre tabindex=\"0\" aria-label=\"Read-only managed sketch TypeScript preview\"><code>{}</code></pre>",
                "<p class=\"wb-code-editor-note\">Promotion reparses, cold-materializes, and independently validates the complete candidate before replacing the ordinary workspace. Unsupported declarations are never omitted.</p>",
                "</section></div>"
            ),
            self.declaration_count,
            if self.declaration_count == 1 { "" } else { "s" },
            escape_html(&self.source),
        )
    }
}

/// Produces one artifact-free project from every ordinary declaration in its
/// current presentation order. The optional layer owns the naming policy;
/// canonical node/port IDs never appear in managed source.
fn projected_code_project(editor: &ProjectionalEditorSession) -> Result<CodeProject, String> {
    let intent = editor.coordinator().intent();
    let projection = editor.workbench_projection();
    let ordered = projection
        .outline
        .iter()
        .flat_map(|cell| &cell.declarations)
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        return Err("the ordinary sketch has no declarations to promote".into());
    }
    if ordered.len() != intent.graph().nodes().len() {
        return Err(
            "the complete ordinary declaration graph is not available for managed promotion".into(),
        );
    }

    let mut base_counts = BTreeMap::<String, usize>::new();
    let mut declarations = Vec::with_capacity(ordered.len());
    for projected in ordered {
        let node = intent
            .graph()
            .node(projected.node)
            .ok_or_else(|| "an ordinary declaration disappeared during code preview".to_owned())?;
        let base = match node.kind {
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
            } => "frame",
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            } if node.inputs.len() >= 2
                && node.inputs.values().all(|input| {
                    intent.graph().node(input.node).is_some_and(|owner| {
                        matches!(
                            owner.kind,
                            IntentNodeKind::Geometry {
                                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
                            }
                        )
                    })
                }) =>
            {
                "diagonal"
            }
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            } => "line",
            _ => "declaration",
        };
        let ordinal = base_counts.entry(base.to_owned()).or_default();
        *ordinal = ordinal.saturating_add(1);
        let symbol = if *ordinal == 1 {
            base.to_owned()
        } else {
            format!("{base}{ordinal}")
        };
        declarations.push(EditorBootstrapDeclaration::new(
            node.id,
            SemanticSymbol(symbol),
        ));
    }
    initialize_code_project_from_editor(
        editor,
        ProjectKey("gui-promoted-sketch".into()),
        &declarations,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn inactive_panel_markup() -> &'static str {
    concat!(
        "<div class=\"wb-code-project-empty\"><span class=\"wb-code-eyebrow\">Optional authoring layer</span>",
        "<strong>No code project open</strong>",
        "<p>Choose a sample under <em>Code &amp; reusable patches</em> to inspect managed source, a pinned custom module, and stable generated ownership. Ordinary sketches remain plain projectional workspaces.</p></div>"
    )
}

pub(crate) fn sample_group_markup(selected: Option<CodeProjectDemoId>) -> String {
    let mut markup = String::from(
        "<li class=\"wb-sample-branch\"><button type=\"button\" data-sample-group-trigger aria-haspopup=\"menu\" aria-expanded=\"false\">Code &amp; reusable patches<span aria-hidden=\"true\">›</span></button><ul class=\"wb-sample-flyout\">",
    );
    for demo in bundled_code_project_demos() {
        let _ = write!(
            markup,
            "<li><button type=\"button\" data-code-sample-id=\"{}\"{}><span class=\"wb-code-sample-mark\" aria-hidden=\"true\">TS</span>{}</button></li>",
            demo.id.key(),
            if selected == Some(demo.id) {
                " aria-current=\"true\""
            } else {
                ""
            },
            escape_html(demo.title),
        );
    }
    markup.push_str("</ul></li>");
    markup
}

fn write_file_tab(markup: &mut String, path: &str, ownership: &str, selected: bool) {
    let _ = write!(
        markup,
        concat!(
            "<button type=\"button\" role=\"tab\" data-code-file=\"{}\" ",
            "aria-selected=\"{}\"><span>{}</span><small>{}</small></button>"
        ),
        escape_attribute(path),
        selected,
        escape_html(path.rsplit('/').next().unwrap_or(path)),
        escape_html(ownership),
    );
}

fn artifact_digests(project: &CodeProject) -> Result<BTreeMap<String, String>, String> {
    let modules = project
        .lock
        .get("modules")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "code-project lock has no module pins".to_owned())?;
    modules
        .iter()
        .map(|(module, pin)| {
            pin.get("artifact")
                .and_then(serde_json::Value::as_str)
                .map(|digest| (module.clone(), digest.to_owned()))
                .ok_or_else(|| format!("code-project module `{module}` has no artifact digest"))
        })
        .collect()
}

fn validate_generated_members(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    authority: &str,
) -> Result<(), String> {
    let desired = required_generated_members(project).map_err(|error| error.to_string())?;
    let actual = generated
        .ordered_members()
        .into_iter()
        .map(|member| member.address)
        .collect::<Vec<_>>();
    if actual == desired {
        Ok(())
    } else {
        Err(format!(
            "{authority} code-session generated provenance does not match managed source"
        ))
    }
}

fn materialize_candidate(
    project: &CodeProject,
    generated: &KeyedReconcileState,
) -> Result<MaterializedCodeProject, String> {
    let (intent, document) = next_materialization_ids()?;
    materialize_code_project_cold(
        project,
        generated,
        intent,
        document,
        CODE_PROJECT_MODEL_SCALE,
    )
    .map_err(|error| error.to_string())
}

fn expansion_for_retained_failure(
    project: &CodeProject,
    generated: &KeyedReconcileState,
) -> Option<geosolve_sketch_code::ExpandedCodeProject> {
    let (intent, _) = next_materialization_ids().ok()?;
    let session = IntentSession::with_id(intent).ok()?;
    expand_code_project(project, generated, session.identity()).ok()
}

fn next_materialization_ids() -> Result<(IntentSessionId, DocumentId), String> {
    let ordinal = NEXT_CODE_MATERIALIZATION.fetch_add(1, Ordering::Relaxed);
    if ordinal == u64::MAX {
        return Err("code-project materialization identity is exhausted".into());
    }
    let raw = 0x84_0000_0000_0000_u128 | u128::from(ordinal);
    Ok((
        IntentSessionId::from_raw(raw),
        DocumentId(PersistentId::from_u128(raw)),
    ))
}

fn encode_editor_checkpoint(
    editor: &ProjectionalEditorSession,
) -> Result<serde_json::Value, String> {
    let ordinary = super::persistence::WorkspaceSnapshot::from_projectional_editor(editor)?;
    let delegated = super::persistence::WorkspaceSnapshot::from_delegated_projectional_editor(
        editor,
        ordinary.computed_evaluation_high_water(),
        ordinary.revisions,
    )?;
    delegated.validate_delegated_intent_checkpoint()?;
    delegated.encode().map(serde_json::Value::String)
}

fn validate_editor_checkpoint(checkpoint: &serde_json::Value) -> Result<(), String> {
    restore_editor_checkpoint(checkpoint).map(|_| ())
}

fn restore_editor_checkpoint(
    checkpoint: &serde_json::Value,
) -> Result<Box<ProjectionalEditorSession>, String> {
    let encoded = checkpoint
        .as_str()
        .ok_or_else(|| "code-project editor checkpoint is not encoded text".to_owned())?;
    let snapshot = super::persistence::WorkspaceSnapshot::decode(encoded)?;
    snapshot.validate_delegated_intent_checkpoint()?;
    let editor = Box::new(super::persistence::projectional_editor_from_snapshot(
        &snapshot,
    )?);
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code-project editor checkpoint has no accepted native scene".to_owned())?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err("code-project editor checkpoint failed independent validation".into());
    }
    Ok(editor)
}

fn rehydrate_editor_checkpoint(
    checkpoint: &serde_json::Value,
    expansion: ExpandedCodeProject,
) -> Result<Box<MaterializedCodeProject>, String> {
    rehydrate_materialized_code_project(restore_editor_checkpoint(checkpoint)?, expansion)
        .map_err(|error| error.to_string())
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed mutation classifier audits every permitted code-owned leaf and rejects all other canvas changes in one place"
)]
fn classify_code_owned_editor_change(
    accepted: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<Option<CodeOwnedEditorChange>, String> {
    let accepted_intent = accepted.coordinator().intent();
    let candidate_intent = candidate.coordinator().intent();
    let accepted_code_nodes = accepted_intent
        .graph()
        .nodes()
        .values()
        .filter(|node| node.symbol.as_str().starts_with("code."))
        .map(|node| (node.symbol.clone(), node.id))
        .collect::<BTreeMap<_, _>>();
    let candidate_code_symbols = candidate_intent
        .graph()
        .nodes()
        .values()
        .filter(|node| node.symbol.as_str().starts_with("code."))
        .map(|node| node.symbol.clone())
        .collect::<BTreeSet<_>>();
    if accepted_code_nodes.keys().cloned().collect::<BTreeSet<_>>() != candidate_code_symbols {
        return Err(
            "code-owned declarations cannot be created or deleted from the canvas; edit managed source"
                .into(),
        );
    }

    let mut allowed_leaves = generated_point_leaves(accepted, expansion)?
        .into_iter()
        .map(|(leaf, address)| (leaf, WritableCodeLeaf::GeneratedPoint(address)))
        .collect::<BTreeMap<_, _>>();
    for (leaf, owner) in managed_rectangle_leaves(accepted, project, expansion)? {
        if allowed_leaves.insert(leaf, owner).is_some() {
            return Err("code-owned writable leaves have ambiguous source ownership".into());
        }
    }
    let mut changed_address = None::<GeneratedMemberAddress>;
    let mut changed_rectangle = None::<(
        SemanticSymbol,
        geosolve_sketch_intent::IntentKey,
        BTreeSet<String>,
    )>;
    for (symbol, accepted_node_id) in &accepted_code_nodes {
        let accepted_node = accepted_intent
            .graph()
            .node(*accepted_node_id)
            .ok_or_else(|| format!("accepted code declaration `{symbol}` disappeared"))?;
        let candidate_node = candidate_intent
            .graph()
            .node_by_symbol(symbol)
            .ok_or_else(|| format!("candidate code declaration `{symbol}` disappeared"))?;
        if accepted_node != candidate_node {
            return Err(
                "this code-owned definition is read-only on canvas; edit managed source or an exposed lens"
                    .into(),
            );
        }
        let accepted_name = accepted_intent
            .organization()
            .node_names()
            .get(accepted_node_id);
        let candidate_name = candidate_intent
            .organization()
            .node_names()
            .get(&candidate_node.id);
        if accepted_name != candidate_name {
            return Err(
                "code-owned declaration names must be changed through managed source".into(),
            );
        }
        let leaves = accepted_intent
            .instance()
            .values()
            .keys()
            .filter(|leaf| leaf.node == *accepted_node_id)
            .chain(
                candidate_intent
                    .instance()
                    .values()
                    .keys()
                    .filter(|leaf| leaf.node == candidate_node.id),
            )
            .copied()
            .collect::<BTreeSet<_>>();
        for leaf in leaves {
            let candidate_leaf = LeafRef {
                node: candidate_node.id,
                ..leaf
            };
            if accepted_intent.instance().values().get(&leaf)
                == candidate_intent.instance().values().get(&candidate_leaf)
            {
                continue;
            }
            let owner = allowed_leaves.get(&leaf).ok_or_else(|| {
                "this code-owned placement is read-only on canvas; edit managed source or an exposed lens"
                    .to_owned()
            })?;
            match owner {
                WritableCodeLeaf::GeneratedPoint(address) => {
                    if changed_rectangle.is_some() {
                        return Err(
                            "one canvas action cannot mix managed and generated placements".into(),
                        );
                    }
                    if changed_address
                        .as_ref()
                        .is_some_and(|changed| changed != address)
                    {
                        return Err(
                            "one canvas action cannot override multiple generated members".into(),
                        );
                    }
                    changed_address = Some(address.clone());
                }
                WritableCodeLeaf::ManagedRectangle {
                    declaration,
                    alias,
                    argument,
                } => {
                    if changed_address.is_some() {
                        return Err(
                            "one canvas action cannot mix managed and generated placements".into(),
                        );
                    }
                    match &mut changed_rectangle {
                        Some((existing, owner, arguments)) => {
                            if existing != declaration || owner != alias {
                                return Err(
                                    "one canvas action cannot rewrite multiple managed declarations"
                                        .into(),
                                );
                            }
                            arguments.insert(argument.clone());
                        }
                        None => {
                            changed_rectangle = Some((
                                declaration.clone(),
                                alias.clone(),
                                BTreeSet::from([argument.clone()]),
                            ));
                        }
                    }
                }
            }
        }
    }
    validate_code_organization(accepted, candidate, &accepted_code_nodes)?;

    if let Some(address) = changed_address {
        let position =
            generated_point_instance(candidate, expansion, &address).ok_or_else(|| {
                "generated point override has no exact writable Cartesian leaves".to_owned()
            })?;
        if !position[0].is_finite() || !position[1].is_finite() {
            return Err("generated point override is not finite".into());
        }
        return Ok(Some(CodeOwnedEditorChange::GeneratedPoint {
            address,
            position,
        }));
    }
    Ok(
        changed_rectangle.map(|(declaration, alias, changed_arguments)| {
            CodeOwnedEditorChange::ManagedRectangle {
                declaration,
                alias,
                changed_arguments,
            }
        }),
    )
}

fn validate_code_organization(
    accepted: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
    code_nodes: &BTreeMap<geosolve_sketch_intent::IntentKey, NodeId>,
) -> Result<(), String> {
    let code_ids = code_nodes.values().copied().collect::<BTreeSet<_>>();
    let signature = |editor: &ProjectionalEditorSession| {
        let organization = editor.coordinator().intent().organization();
        organization
            .cell_order()
            .iter()
            .filter_map(|cell_id| {
                let cell = organization.cells().get(cell_id)?;
                let declarations = cell
                    .declarations
                    .iter()
                    .filter(|node| code_ids.contains(node))
                    .copied()
                    .collect::<Vec<_>>();
                (!declarations.is_empty()).then_some((cell.name.clone(), declarations))
            })
            .collect::<Vec<_>>()
    };
    if signature(accepted) == signature(candidate) {
        Ok(())
    } else {
        Err("code-owned declaration organization must be changed through managed source".into())
    }
}

fn generated_point_leaves(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeMap<LeafRef, GeneratedMemberAddress>, String> {
    let mut leaves = BTreeMap::new();
    for (address, provenance) in &expansion.generated_provenance {
        if !supported_point_override_address(address) {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            continue;
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&port.alias)
            .ok_or_else(|| {
                format!(
                    "generated point `{}` has no intent owner",
                    address.display_path()
                )
            })?;
        let output = node.port_by_selector(port.selector).ok_or_else(|| {
            format!(
                "generated point `{}` has no stable output",
                address.display_path()
            )
        })?;
        for field in [LeafField::X, LeafField::Y] {
            let leaf = LeafRef {
                node: node.id,
                port: output.id,
                field,
            };
            if leaves.insert(leaf, address.clone()).is_some() {
                return Err("generated point leaves are not uniquely owned".into());
            }
        }
    }
    Ok(leaves)
}

fn managed_rectangle_alias(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    declaration: &SemanticSymbol,
) -> Option<geosolve_sketch_intent::IntentKey> {
    let managed = project
        .managed
        .program
        .declarations
        .iter()
        .find(|candidate| {
            candidate.symbol == *declaration
                && candidate.patch.is_none()
                && candidate.builder_path == ["geometry", "rectangle"]
        })?;
    expansion.semantic_outputs.values().find_map(|output| {
        if output.reference.declaration != managed.symbol || !output.reference.output.0.is_empty() {
            return None;
        }
        match &output.target {
            ExpandedSemanticTarget::Declaration {
                alias,
                kind: FeatureKind::Feature,
            } => Some(alias.clone()),
            _ => None,
        }
    })
}

fn managed_rectangle_leaves(
    editor: &ProjectionalEditorSession,
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeMap<LeafRef, WritableCodeLeaf>, String> {
    let mut leaves = BTreeMap::new();
    for declaration in &project.managed.program.declarations {
        let Some(alias) = managed_rectangle_alias(project, expansion, &declaration.symbol) else {
            continue;
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .ok_or_else(|| {
                format!(
                    "managed rectangle `{}` has no intent declaration",
                    declaration.symbol.0
                )
            })?;
        if !matches!(
            node.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
            }
        ) {
            return Err(format!(
                "managed rectangle `{}` has the wrong intent recipe",
                declaration.symbol.0
            ));
        }
        for (index, argument) in [(0, "lowerLeft"), (2, "upperRight")] {
            let port = node
                .port_by_selector(IntentPortSelector::Node {
                    role: IntentPortRole::Corner,
                    index,
                })
                .ok_or_else(|| {
                    format!(
                        "managed rectangle `{}` has no writable `{argument}` port",
                        declaration.symbol.0
                    )
                })?;
            for field in [LeafField::X, LeafField::Y] {
                let leaf = LeafRef {
                    node: node.id,
                    port: port.id,
                    field,
                };
                let owner = WritableCodeLeaf::ManagedRectangle {
                    declaration: declaration.symbol.clone(),
                    alias: alias.clone(),
                    argument: argument.into(),
                };
                if leaves.insert(leaf, owner).is_some() {
                    return Err("managed rectangle leaves are not uniquely owned".into());
                }
            }
        }
    }
    Ok(leaves)
}

fn managed_rectangle_points(
    editor: &ProjectionalEditorSession,
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeMap<geosolve_sketch::DesignPointId, (SemanticSymbol, String)>, String> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
    let mut points = BTreeMap::new();
    for owner in managed_rectangle_leaves(editor, project, expansion)?.into_values() {
        let WritableCodeLeaf::ManagedRectangle {
            declaration,
            alias,
            argument,
        } = owner
        else {
            continue;
        };
        let index = match argument.as_str() {
            "lowerLeft" => 0,
            "upperRight" => 2,
            _ => continue,
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .ok_or_else(|| format!("managed rectangle `{}` disappeared", declaration.0))?;
        let port = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index,
            })
            .ok_or_else(|| format!("managed rectangle `{}` lost `{argument}`", declaration.0))?;
        let Some(IntentNativeBinding::Point(point)) = accepted.ownership.port(port.as_ref(node.id))
        else {
            return Err(format!(
                "managed rectangle `{}` `{argument}` is not a native point",
                declaration.0
            ));
        };
        if let Some(previous) = points.insert(point, (declaration.clone(), argument.clone()))
            && previous != (declaration, argument)
        {
            return Err("managed rectangle points are not uniquely owned".into());
        }
    }
    Ok(points)
}

fn managed_rectangle_argument_position(
    editor: &ProjectionalEditorSession,
    alias: &geosolve_sketch_intent::IntentKey,
    argument: &str,
) -> Option<[f64; 2]> {
    let index = match argument {
        "lowerLeft" => 0,
        "upperRight" => 2,
        _ => return None,
    };
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(alias)?;
    let port = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Corner,
        index,
    })?;
    let coordinate = |field| match intent.instance().values().get(&LeafRef {
        node: node.id,
        port: port.id,
        field,
    })? {
        IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        } => Some(*value),
        _ => None,
    };
    Some([coordinate(LeafField::X)?, coordinate(LeafField::Y)?])
}

fn managed_point_value_like(
    current: &ManagedValue,
    position: [f64; 2],
) -> Result<ManagedValue, String> {
    let ManagedValue::Array(current) = current else {
        return Err("managed point placement is not an array".into());
    };
    if current.len() != 2 {
        return Err("managed point placement does not have two coordinates".into());
    }
    let coordinate = |current: &ManagedValue, value| match current {
        ManagedValue::Number(_) => Ok(ManagedValue::Number(value)),
        ManagedValue::Unit(unit) => Ok(ManagedValue::Unit(UnitLiteral {
            unit: unit.unit.clone(),
            value,
        })),
        _ => Err("managed point coordinate is not a numeric literal".to_owned()),
    };
    Ok(ManagedValue::Array(vec![
        coordinate(&current[0], position[0])?,
        coordinate(&current[1], position[1])?,
    ]))
}

fn generated_point_instance(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    address: &GeneratedMemberAddress,
) -> Option<[f64; 2]> {
    let target = &expansion.generated_provenance.get(address)?.target;
    let ExpandedSemanticTarget::Port { port } = target else {
        return None;
    };
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&port.alias)?;
    let output = node.port_by_selector(port.selector)?;
    let value = |field| {
        let leaf = LeafRef {
            node: node.id,
            port: output.id,
            field,
        };
        match intent.instance().values().get(&leaf)? {
            IntentLiteral::Quantity {
                value,
                unit: IntentUnit::Length,
            } => Some(*value),
            _ => None,
        }
    };
    Some([value(LeafField::X)?, value(LeafField::Y)?])
}

fn generated_point_position(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    address: &GeneratedMemberAddress,
) -> Option<[f64; 2]> {
    let target = &expansion.generated_provenance.get(address)?.target;
    let point = expanded_target_point(editor, target)?;
    editor
        .coordinator()
        .accepted_materialization()?
        .session
        .design_document()
        .point(point)
        .map(|value| value.position)
}

fn expanded_target_point(
    editor: &ProjectionalEditorSession,
    target: &ExpandedSemanticTarget,
) -> Option<geosolve_sketch::DesignPointId> {
    let ExpandedSemanticTarget::Port { port } = target else {
        return None;
    };
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&port.alias)?;
    let output = node.port_by_selector(port.selector)?;
    let accepted = editor.coordinator().accepted_materialization()?;
    match accepted.ownership.port(output.as_ref(node.id))? {
        IntentNativeBinding::Point(point) => Some(point),
        _ => None,
    }
}

fn feature_documents_match_for_terminal_parity(
    terminal: &ComputedFeatureDocument,
    staged: &ComputedFeatureDocument,
) -> bool {
    let terminal_identity = terminal.identity();
    let staged_identity = staged.identity();
    terminal_identity.document == staged_identity.document
        && terminal_identity.sketch_document == staged_identity.sketch_document
        && terminal.allocator_high_water() == staged.allocator_high_water()
        && terminal.features().len() == staged.features().len()
        && terminal
            .features()
            .iter()
            .zip(staged.features())
            .all(|(terminal, staged)| {
                if terminal.id != staged.id
                    || terminal.label != staged.label
                    || terminal.suppressed != staged.suppressed
                {
                    return false;
                }
                let (
                    ComputedFeatureDefinition::FilletSet(terminal),
                    ComputedFeatureDefinition::FilletSet(staged),
                ) = (&terminal.definition, &staged.definition);
                terminal.radius.to_bits() == staged.radius.to_bits()
                    && terminal.corners.len() == staged.corners.len()
                    && terminal
                        .corners
                        .iter()
                        .zip(&staged.corners)
                        .all(|(terminal, staged)| {
                            terminal.id == staged.id
                                && terminal.endpoint_order == staged.endpoint_order
                                && terminal.sweep == staged.sweep
                                && [
                                    (terminal.first, staged.first),
                                    (terminal.second, staged.second),
                                ]
                                .into_iter()
                                .all(|(terminal, staged)| {
                                    // `picked_parameter` is a recomputable
                                    // seed, not a branch cell. Every durable
                                    // owner, winding, neighborhood, normal,
                                    // endpoint and periodic anchor stays exact.
                                    terminal.source == staged.source
                                        && terminal.winding == staged.winding
                                        && terminal.neighborhood == staged.neighborhood
                                        && terminal.normal_side == staged.normal_side
                                        && terminal.retained_endpoint == staged.retained_endpoint
                                        && terminal.periodic_anchor == staged.periodic_anchor
                                })
                        })
            })
}

fn computed_snapshots_match_for_terminal_parity(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
) -> bool {
    terminal.edges().len() == staged.edges().len()
        && terminal
            .edges()
            .iter()
            .zip(staged.edges())
            .all(|(terminal, staged)| {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.role == staged.role
                    && terminal.geometry == staged.geometry
                    && terminal.provenance == staged.provenance
            })
        && terminal.construction_fragments().len() == staged.construction_fragments().len()
        && terminal
            .construction_fragments()
            .iter()
            .zip(staged.construction_fragments())
            .all(|(terminal, staged)| {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.source == staged.source
                    && terminal.interval == staged.interval
                    && terminal.source_role == staged.source_role
                    && terminal.provenance == staged.provenance
            })
        && terminal.replaced_sources() == staged.replaced_sources()
        && terminal.feature_evaluations().len() == staged.feature_evaluations().len()
        && terminal
            .feature_evaluations()
            .iter()
            .zip(staged.feature_evaluations())
            .all(|(terminal, staged)| {
                if terminal.feature != staged.feature {
                    return false;
                }
                match (&terminal.state, &staged.state) {
                    (
                        ComputedFeatureEvaluationState::Current {
                            corner_edges: terminal,
                        },
                        ComputedFeatureEvaluationState::Current {
                            corner_edges: staged,
                        },
                    ) => {
                        terminal.len() == staged.len()
                            && terminal.iter().zip(staged).all(
                                |(
                                    (terminal_corner, terminal_edge),
                                    (staged_corner, staged_edge),
                                )| {
                                    terminal_corner == staged_corner
                                        && terminal_edge.ordinal == staged_edge.ordinal
                                },
                            )
                    }
                    (
                        ComputedFeatureEvaluationState::Failed { failure: terminal },
                        ComputedFeatureEvaluationState::Failed { failure: staged },
                    ) => terminal == staged,
                    (
                        ComputedFeatureEvaluationState::Suppressed,
                        ComputedFeatureEvaluationState::Suppressed,
                    ) => true,
                    _ => false,
                }
            })
}

fn validate_terminal_native_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<(), String> {
    let terminal_authority = terminal
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "staged code drag has no accepted native authority".to_owned())?;
    let mut recomputable = recomputable_code_line_branches(terminal, expansion)?;
    recomputable.extend(recomputable_code_line_branches(staged, expansion)?);
    let terminal_accepted = terminal_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal code drag has no current accepted document".to_owned())?
        .document();
    let staged_accepted = staged_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "staged code drag has no current accepted document".to_owned())?
        .document();
    let same_documents = terminal_authority
        .session
        .design_document()
        .exact_except_recomputable_line_branches(
            staged_authority.session.design_document(),
            &recomputable,
        )
        && terminal_accepted
            .exact_except_recomputable_line_branches(staged_accepted, &recomputable);
    let same_features = feature_documents_match_for_terminal_parity(
        &terminal_authority.features,
        &staged_authority.features,
    ) && computed_snapshots_match_for_terminal_parity(
        &terminal_authority.computed,
        &staged_authority.computed,
    );
    // Revision/digest stamps and Fillet pick seeds can refresh when staged
    // source is canonically rematerialized. Durable branch cells, complete
    // evaluated geometry, ownership rows and persistent IDs stay exact.
    let same_ownership = terminal_authority.ownership.nodes == staged_authority.ownership.nodes
        && terminal_authority.ownership.ports == staged_authority.ownership.ports
        && terminal_authority.ownership.reservations == staged_authority.ownership.reservations
        && terminal_authority.ownership.writable_leaves
            == staged_authority.ownership.writable_leaves
        && terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates;
    let mut differences = Vec::new();
    if !same_documents {
        differences.push("sketch documents");
    }
    if !same_features {
        differences.push("computed features");
    }
    if !same_ownership {
        differences.push("native ownership");
    }
    if terminal_authority.feature_lifecycle_high_water.allocator
        != staged_authority.feature_lifecycle_high_water.allocator
    {
        differences.push("feature allocator");
    }
    if terminal_authority.session.persistent_identity_high_water()
        != staged_authority.session.persistent_identity_high_water()
    {
        differences.push("sketch allocator");
    }
    if !differences.is_empty() {
        return Err(format!(
            "terminal code drag differs from its independently staged native authority in {}",
            differences.join(", ")
        ));
    }
    Ok(())
}

fn recomputable_code_line_branches(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeSet<geosolve_sketch::CurveId>, String> {
    let intent = editor.coordinator().intent();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
    let expansion_owned_segments = expansion
        .patch
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::CreateNode { draft, .. }
                if matches!(
                    draft.kind,
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment
                    }
                ) =>
            {
                Some(draft.symbol.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut curves = BTreeSet::new();
    for node in intent.graph().nodes().values() {
        let IntentNodeKind::Geometry { recipe } = node.kind else {
            continue;
        };
        // M83 Segment branches remain explicit. Only the exact current code
        // expansion proves that a Segment came from a managed/artifact
        // declaration whose branch is source-derived. An ordinary GUI Segment
        // living beside a code project must still compare bit-for-bit.
        let source_derived_segment = recipe == GeometryRecipeKind::Segment
            && expansion_owned_segments.contains(&node.symbol);
        if !source_derived_segment
            && !matches!(
                recipe,
                GeometryRecipeKind::Polyline
                    | GeometryRecipeKind::TwoPointAlignedRectangle
                    | GeometryRecipeKind::ThreePointCornerRectangle
                    | GeometryRecipeKind::CenterRectangle
                    | GeometryRecipeKind::ThreePointCenterRectangle
            )
        {
            continue;
        }
        if let Some(ownership) = accepted.ownership.node(node.id) {
            curves.extend(ownership.owned.iter().filter_map(|binding| match binding {
                IntentNativeBinding::Curve(curve) => Some(*curve),
                _ => None,
            }));
        }
    }
    for (address, provenance) in &expansion.generated_provenance {
        if address.template != ["polyline", "segment"] {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            continue;
        };
        let node = intent
            .graph()
            .node_by_symbol(&port.alias)
            .ok_or_else(|| format!("generated segment `{}` disappeared", address.display_path()))?;
        let output = node.port_by_selector(port.selector).ok_or_else(|| {
            format!(
                "generated segment `{}` lost its output",
                address.display_path()
            )
        })?;
        match accepted.ownership.port(output.as_ref(node.id)) {
            Some(IntentNativeBinding::CurveSpan(span)) => {
                curves.insert(span.curve);
            }
            Some(IntentNativeBinding::Curve(curve)) => {
                curves.insert(curve);
            }
            _ => {
                return Err(format!(
                    "generated segment `{}` has no native curve",
                    address.display_path()
                ));
            }
        }
    }
    Ok(curves)
}

fn supported_point_override_address(address: &GeneratedMemberAddress) -> bool {
    address.template == ["polyline", "vertex"] && address.output == ["point"]
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    [value[0].to_bits(), value[1].to_bits()]
}

fn managed_path_text(path: &[geosolve_sketch_code::ManagedPathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            geosolve_sketch_code::ManagedPathSegment::Field(field) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(field);
            }
            geosolve_sketch_code::ManagedPathSegment::Index(index) => {
                let _ = write!(text, "[{index}]");
            }
            geosolve_sketch_code::ManagedPathSegment::Member { member } => {
                let _ = write!(text, "[{member}]");
            }
        }
    }
    if text.is_empty() {
        "value".into()
    } else {
        text
    }
}

fn managed_object_path<'a>(value: &'a ManagedValue, path: &[String]) -> Option<&'a ManagedValue> {
    path.iter().try_fold(value, |value, field| {
        let ManagedValue::Object(object) = value else {
            return None;
        };
        object.get(field)
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attribute(value: &str) -> String {
    escape_html(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary_rectangle_diagonal() -> ProjectionalEditorSession {
        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_f003)).unwrap();
        let mut editor = ProjectionalEditorSession::restore(
            intent,
            DocumentId(PersistentId::from_u128(0x84_f003)),
            1.0,
        )
        .unwrap();
        let selector = |role, index| IntentPortSelector::Node { role, index };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let frame_alias = geosolve_sketch_intent::IntentKey::new("gui-frame").unwrap();
        let frame = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
            },
            geosolve_sketch_intent::IntentKey::new("gui.frame").unwrap(),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 0),
            LeafField::X,
            length(-4.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 0),
            LeafField::Y,
            length(-3.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 2),
            LeafField::X,
            length(8.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 2),
            LeafField::Y,
            length(5.0),
        );
        let frame = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: frame_alias.clone(),
                    draft: Box::new(frame),
                    cell: None,
                }],
            ))
            .unwrap();
        let frame_node = frame.aliases.node(&frame_alias).unwrap();
        let lower_left = editor
            .coordinator()
            .intent()
            .graph()
            .node(frame_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Corner, 0))
            .unwrap()
            .as_ref(frame_node);
        let upper_right = editor
            .coordinator()
            .intent()
            .graph()
            .node(frame_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Corner, 2))
            .unwrap()
            .as_ref(frame_node);
        let diagonal_alias = geosolve_sketch_intent::IntentKey::new("gui-diagonal").unwrap();
        let diagonal = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.diagonal").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 0),
            geosolve_sketch_intent::PatchPortRef::Stable { port: lower_left },
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 1),
            geosolve_sketch_intent::PatchPortRef::Stable { port: upper_right },
        );
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: diagonal_alias,
                    draft: Box::new(diagonal),
                    cell: None,
                }],
            ))
            .unwrap();
        editor
    }

    fn open_with_editor(key: &str) -> (CodeProjectWorkbench, Box<ProjectionalEditorSession>) {
        CodeProjectWorkbench::open_key(key).expect("code project")
    }

    fn open(key: &str) -> CodeProjectWorkbench {
        open_with_editor(key).0
    }

    fn open_boxed(key: &str) -> (Box<CodeProjectWorkbench>, Box<ProjectionalEditorSession>) {
        let (workbench, editor) = open_with_editor(key);
        (Box::new(workbench), editor)
    }

    fn code_node_ids(
        editor: &ProjectionalEditorSession,
    ) -> BTreeMap<geosolve_sketch_intent::IntentKey, NodeId> {
        editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter(|node| node.symbol.as_str().starts_with("code."))
            .map(|node| (node.symbol.clone(), node.id))
            .collect()
    }

    fn generated_native_bindings(
        editor: &ProjectionalEditorSession,
        expansion: &ExpandedCodeProject,
    ) -> BTreeMap<GeneratedMemberAddress, (NodeId, IntentNativeBinding)> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .expect("generated native authority");
        expansion
            .generated_provenance
            .iter()
            .filter_map(|(address, provenance)| {
                let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                    return None;
                };
                let node = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node_by_symbol(&port.alias)
                    .expect("generated declaration");
                let output = node
                    .port_by_selector(port.selector)
                    .expect("generated output");
                Some((
                    address.clone(),
                    (
                        node.id,
                        accepted
                            .ownership
                            .port(output.as_ref(node.id))
                            .expect("generated native binding"),
                    ),
                ))
            })
            .collect()
    }

    fn generated_fillet_owners(
        workbench: &CodeProjectWorkbench,
    ) -> BTreeMap<GeneratedMemberAddress, Vec<geosolve_constraint_editor::ComputedCornerRef>> {
        workbench
            .materialized
            .as_deref()
            .expect("warm code authority")
            .host_outputs
            .iter()
            .map(|(address, outputs)| {
                (
                    address.clone(),
                    outputs.iter().map(|output| output.owner).collect(),
                )
            })
            .collect()
    }

    fn native_binding_for_node(
        editor: &ProjectionalEditorSession,
        node: NodeId,
        predicate: impl Fn(IntentNativeBinding) -> bool,
    ) -> IntentNativeBinding {
        editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted native authority")
            .ownership
            .node(node)
            .expect("native declaration owner")
            .owned
            .iter()
            .copied()
            .find(|binding| predicate(*binding))
            .expect("requested native binding")
    }

    fn curve_definition_for_binding(
        editor: &ProjectionalEditorSession,
        binding: IntentNativeBinding,
    ) -> geosolve_sketch::CurveDefinition {
        let curve = match binding {
            IntentNativeBinding::Curve(curve) => curve,
            IntentNativeBinding::CurveSpan(span) => span.curve,
            _ => panic!("binding is not native curve geometry"),
        };
        editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted curve authority")
            .session
            .design_document()
            .curve(curve)
            .expect("owned native curve")
            .definition
            .clone()
    }

    fn assert_warm_cache_matches_session(workbench: &CodeProjectWorkbench) {
        let materialized = workbench.materialized.as_deref().expect("warm authority");
        assert_eq!(
            &materialized.expansion,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .expect("accepted expansion"),
        );
        assert_eq!(
            encode_editor_checkpoint(&materialized.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
    }

    fn add_gui_horizontal_on_generated_span(
        editor: &mut ProjectionalEditorSession,
        expansion: &ExpandedCodeProject,
        member_key: &str,
        symbol: &str,
        suppressed: bool,
    ) -> NodeId {
        let (_, provenance) = expansion
            .generated_provenance
            .iter()
            .find(|(address, _)| {
                address.template == ["polyline", "segment"] && address.member_key == [member_key]
            })
            .expect("generated Polyline span");
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            panic!("generated Polyline segment must own one stable span port")
        };
        let owner = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&port.alias)
            .expect("generated span declaration");
        let span = owner
            .port_by_selector(port.selector)
            .expect("generated span output")
            .as_ref(owner.id);
        let mut draft = geosolve_sketch_intent::IntentNodeDraft::new(
            geosolve_sketch_intent::IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            },
            geosolve_sketch_intent::IntentKey::new(symbol).unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Stable { port: span },
        );
        draft.suppressed = suppressed;
        let alias = geosolve_sketch_intent::IntentKey::new("gui-dependent").unwrap();
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: alias.clone(),
                    draft: Box::new(draft),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI dependent");
        outcome.aliases.node(&alias).expect("ordinary GUI node")
    }

    #[test]
    fn all_four_samples_open_with_nonempty_independently_validated_native_canvases() {
        for demo in bundled_code_project_demos() {
            let (workbench, editor) = open_with_editor(demo.id.key());
            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .expect("code sample owns accepted native authority");
            let design = accepted.session.design_document();
            assert!(
                !design.points().is_empty() && !design.curves().is_empty(),
                "{} opened an empty native canvas",
                demo.id.key(),
            );
            assert!(accepted.validation.hard_residuals_validated);
            assert!(accepted.validation.all_active_features_current);
            assert_eq!(editor.coordinator().intent().undo_len(), 0);
            assert_eq!(editor.coordinator().intent().redo_len(), 0);
            assert!(
                accepted
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
            );
            assert!(workbench.session.snapshot().expansion.is_some());
            assert_eq!(
                workbench.session.snapshot().accepted_editor_checkpoint,
                encode_editor_checkpoint(&editor).unwrap(),
            );
        }
    }

    #[test]
    fn menu_owns_one_distinct_code_group_and_four_genuine_project_leaves() {
        let markup = sample_group_markup(None);
        assert!(markup.contains("Code &amp; reusable patches"));
        assert_eq!(markup.matches("data-code-sample-id=").count(), 4);
        for demo in bundled_code_project_demos() {
            assert_eq!(
                markup
                    .matches(&format!("data-code-sample-id=\"{}\"", demo.id.key()))
                    .count(),
                1,
            );
        }
        assert!(!markup.contains("data-sample-id="));
    }

    #[test]
    fn managed_and_custom_files_have_truthful_distinct_ownership_surfaces() {
        let mut workbench = open("rounded-polyline");
        let managed = workbench.panel_markup();
        assert!(managed.contains("data-code-file-kind=\"managed\""));
        assert!(managed.contains("data-code-action=\"apply\""));
        assert!(managed.contains("GUI-managed subset"));
        assert!(managed.contains("Edit lenses"));
        assert!(managed.contains("data-code-lens-declaration=\"rounded\""));
        assert!(managed.contains("data-code-lens-path=\"radius\""));
        assert!(!managed.contains("Read-only in demo"));

        workbench
            .select_file("patches/round-every-corner.patch.ts")
            .unwrap();
        let custom = workbench.panel_markup();
        assert!(custom.contains("data-code-file-kind=\"custom\""));
        assert!(custom.contains("Read-only in demo"));
        assert!(custom.contains("p.each"));
        assert!(!custom.contains("id=\"wb-code-managed-source\""));
    }

    #[test]
    fn scalar_edit_lens_rewrites_one_leaf_and_publishes_one_outer_history_entry() {
        let mut workbench = open("rounded-polyline");
        let before = workbench.managed_source().to_owned();
        let before_revision = workbench.session.identity().revision;
        let CodeApplyOutcome::Accepted(publication) = workbench
            .apply_scalar_lens("rounded", "radius", 0.75)
            .unwrap()
        else {
            panic!("valid scalar lens must acquire native authority")
        };
        assert_eq!(publication.receipt.before.revision, before_revision);
        assert_eq!(publication.receipt.after.revision, before_revision + 1);
        assert!(workbench.managed_source().contains("radius: mm(0.75)"));
        assert_eq!(
            workbench
                .managed_source()
                .replace("radius: mm(0.75)", "radius: mm(0.4)"),
            before,
        );
        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
    }

    #[test]
    fn adaptive_managed_apply_preserves_unaffected_keys_and_adds_crest() {
        let (mut workbench, before_editor) = open_with_editor("rounded-polyline");
        let before = workbench.session.snapshot().generated.active().clone();
        let before_source = workbench.managed_source().to_owned();
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_native = generated_native_bindings(
            &before_editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        let before_fillets = generated_fillet_owners(&workbench);
        let before_points = before_editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len();
        let draft = workbench.managed_source().replace(
            "{ key: \"end\", position: [65, 10] },",
            "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
        );
        workbench.set_managed_draft(draft);
        let publication = match workbench.apply_managed_draft().unwrap() {
            CodeApplyOutcome::Accepted(publication) => publication,
            CodeApplyOutcome::RetainedFailure { diagnostic, .. } => {
                panic!("valid crest insertion was retained as failed: {diagnostic}")
            }
        };
        let after = workbench.session.snapshot().generated.active();
        assert_eq!(before.len(), 15);
        assert_eq!(after.len(), 18);
        for (address, identity) in &before {
            assert_eq!(
                after.get(address),
                Some(identity),
                "{}",
                address.display_path()
            );
        }
        assert!(
            after.keys().any(|address| {
                address.member_key == ["crest"] && address.template == ["fillet"]
            })
        );
        assert_ne!(workbench.managed_source(), before_source);
        assert_ne!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            publication.receipt.after,
            workbench.session.identity().clone()
        );
        let after_native = generated_native_bindings(
            &publication.editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        for (address, identity) in before_native {
            assert_eq!(
                after_native.get(&address),
                Some(&identity),
                "native identity changed for {}",
                address.display_path(),
            );
        }
        let after_fillets = generated_fillet_owners(&workbench);
        for (address, owners) in before_fillets {
            assert_eq!(
                after_fillets.get(&address),
                Some(&owners),
                "Fillet owner changed for {}",
                address.display_path(),
            );
        }
        let after_points = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len();
        assert!(after_points > before_points);
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the integration regression keeps GUI cell, declaration, native identity, Apply, and Undo assertions in one scenario"
    )]
    fn gui_cell_and_declaration_survive_managed_apply_with_native_identity() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let cell_alias = geosolve_sketch_intent::IntentKey::new("gui-notes-cell").unwrap();
        let node_alias = geosolve_sketch_intent::IntentKey::new("gui-marker").unwrap();
        let marker = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            geosolve_sketch_intent::IntentKey::new("gui.marker").unwrap(),
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: -5.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: -4.0,
                unit: IntentUnit::Length,
            },
        );
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![
                    geosolve_sketch_intent::IntentPatchOperation::CreateCell {
                        alias: cell_alias.clone(),
                        name: geosolve_sketch_intent::IntentKey::new("GUI notes").unwrap(),
                        before: None,
                    },
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: node_alias.clone(),
                        draft: Box::new(marker),
                        cell: Some(geosolve_sketch_intent::CellTarget::Alias {
                            alias: cell_alias.clone(),
                        }),
                    },
                ],
            ))
            .unwrap();
        let cell = outcome.aliases.cell(&cell_alias).unwrap();
        let node = outcome.aliases.node(&node_alias).unwrap();
        let marker_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(node)
            .unwrap()
            .clone();
        let point_before = native_binding_for_node(&editor, node, |binding| {
            matches!(binding, IntentNativeBinding::Point(_))
        });
        let published = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI notes marker",
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            published
                .editor
                .coordinator()
                .intent()
                .organization()
                .cells()
                .get(&cell)
                .unwrap()
                .declarations,
            vec![node],
        );

        workbench.set_managed_draft(
            workbench
                .managed_source()
                .replace("radius: mm(0.4)", "radius: mm(0.6)"),
        );
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid managed Apply unexpectedly retained a failure")
        };
        assert_eq!(
            applied.editor.coordinator().intent().graph().node(node),
            Some(&marker_before),
        );
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .organization()
                .cells()
                .get(&cell)
                .unwrap()
                .declarations,
            vec![node],
        );
        assert_eq!(
            native_binding_for_node(&applied.editor, node, |binding| {
                matches!(binding, IntentNativeBinding::Point(_))
            }),
            point_before,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn gui_horizontal_on_unaffected_start_span_survives_crest_insertion_exactly() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let constraint = add_gui_horizontal_on_generated_span(
            &mut editor,
            &expansion,
            "start",
            "gui.constraint.start-horizontal",
            false,
        );
        let node_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(constraint)
            .unwrap()
            .clone();
        let native_before = native_binding_for_node(&editor, constraint, |binding| {
            matches!(binding, IntentNativeBinding::Constraint(_))
        });
        workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Constrain generated start span",
            )
            .unwrap()
            .unwrap();
        workbench.set_managed_draft(workbench.managed_source().replace(
            "{ key: \"end\", position: [65, 10] },",
            "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
        ));
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid crest insertion unexpectedly retained a failure")
        };
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(constraint),
            Some(&node_before),
        );
        assert_eq!(
            native_binding_for_node(&applied.editor, constraint, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            native_before,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn persistence_rehydrate_then_crest_apply_preserves_all_unaffected_identities() {
        let workbench = open("rounded-polyline");
        let json = workbench.to_persistence_json().unwrap();
        let mut restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        let before_editor = restored.restore_accepted_editor().unwrap();
        let generated_before = restored.session.snapshot().generated.active().clone();
        let native_before = generated_native_bindings(
            &before_editor,
            restored
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        let fillets_before = generated_fillet_owners(&restored);
        restored.set_managed_draft(restored.managed_source().replace(
            "{ key: \"end\", position: [65, 10] },",
            "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
        ));
        let CodeApplyOutcome::Accepted(applied) = restored.apply_managed_draft().unwrap() else {
            panic!("valid post-restore crest insertion unexpectedly retained a failure")
        };
        for (address, identity) in generated_before {
            assert_eq!(
                restored.session.snapshot().generated.active().get(&address),
                Some(&identity),
                "semantic identity changed after reload for {}",
                address.display_path(),
            );
        }
        let native_after = generated_native_bindings(
            &applied.editor,
            restored
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        for (address, binding) in native_before {
            assert_eq!(native_after.get(&address), Some(&binding));
        }
        let fillets_after = generated_fillet_owners(&restored);
        for (address, owners) in fillets_before {
            assert_eq!(fillets_after.get(&address), Some(&owners));
        }
        assert_warm_cache_matches_session(&restored);
    }

    #[test]
    fn valid_but_impossible_apply_retains_source_over_the_prior_native_canvas() {
        let (mut workbench, editor) = open_with_editor("rounded-polyline");
        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let accepted_design = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .clone();
        workbench.set_managed_draft(workbench.managed_source().replace("mm(0.4)", "mm(400)"));
        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("impossible Fillet radius unexpectedly acquired native authority")
        };
        assert!(receipt.retained_failure);
        assert!(!diagnostic.is_empty());
        assert_eq!(workbench.accepted_editor_checkpoint(), &accepted_checkpoint,);
        assert!(workbench.session.snapshot().failure.is_some());
        assert!(workbench.managed_source().contains("mm(400)"));
        assert_eq!(
            editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document(),
            &accepted_design,
        );
    }

    #[test]
    fn parsed_structural_failure_retains_code_intent_without_touching_native_authority() {
        let mut workbench = open("rounded-polyline");
        let before_source = workbench.managed_source().to_owned();
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(before_source.replace(
            "{ key: \"ridge\", position: [40, 18] },",
            "{ key: \"shoulder\", position: [40, 18] },",
        ));

        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("duplicate semantic key unexpectedly acquired native authority")
        };
        assert!(receipt.retained_failure);
        assert!(diagnostic.contains("duplicate Polyline key `shoulder`"));
        assert_eq!(
            workbench.session.snapshot().failure.as_ref().unwrap().stage,
            "structural expansion",
        );
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_ne!(workbench.managed_source(), before_source);

        workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before_source);
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
    }

    #[test]
    fn retained_structural_failure_round_trips_with_accepted_canvas_authority() {
        let mut workbench = open("rounded-polyline");
        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(workbench.managed_source().replace(
            "{ key: \"ridge\", position: [40, 18] },",
            "{ key: \"shoulder\", position: [40, 18] },",
        ));
        assert!(matches!(
            workbench.apply_managed_draft().unwrap(),
            CodeApplyOutcome::RetainedFailure { .. }
        ));

        let json = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert!(restored.session.snapshot().failure.is_some());
        assert!(restored.managed_source().contains("key: \"shoulder\""));
        assert_eq!(restored.accepted_editor_checkpoint(), &accepted_checkpoint);
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    fn undo_redo_restore_source_expansion_and_native_scene_as_one_checkpoint() {
        let (mut workbench, _) = open_with_editor("rounded-polyline");
        let before_source = workbench.managed_source().to_owned();
        let before_expansion = workbench.session.snapshot().expansion.clone();
        let draft = before_source.replace("mm(0.4)", "mm(0.7)");
        workbench.set_managed_draft(draft.clone());
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid radius edit must acquire native authority")
        };
        let applied_checkpoint = workbench.accepted_editor_checkpoint().clone();
        assert_eq!(workbench.managed_source(), draft);
        assert_eq!(
            encode_editor_checkpoint(&applied.editor).unwrap(),
            applied_checkpoint,
        );
        assert_warm_cache_matches_session(&workbench);

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before_source);
        assert_eq!(workbench.session.snapshot().expansion, before_expansion);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
        assert_warm_cache_matches_session(&workbench);

        let redone = workbench.step_history(false).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), draft);
        assert_eq!(workbench.accepted_editor_checkpoint(), &applied_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&redone.editor).unwrap(),
            applied_checkpoint,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn direct_gui_edit_publishes_once_into_outer_history_without_nested_history() {
        let (mut workbench, mut editor) = open_boxed("typed-panel");
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_revision = workbench.session.identity().revision;
        let patch = geosolve_sketch_intent::IntentPatch::new(
            editor.coordinator().intent().identity(),
            geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
            vec![geosolve_sketch_intent::IntentPatchOperation::CreateCell {
                alias: geosolve_sketch_intent::IntentKey::new("gui-notes-cell").unwrap(),
                name: geosolve_sketch_intent::IntentKey::new("GUI notes").unwrap(),
                before: None,
            }],
        );
        editor.apply_patch(patch).unwrap();
        assert_eq!(editor.coordinator().intent().undo_len(), 1);

        let checkpoint = encode_editor_checkpoint(&editor).unwrap();
        let delegated = restore_editor_checkpoint(&checkpoint).unwrap();
        assert_eq!(delegated.coordinator().intent().undo_len(), 0);
        assert_eq!(delegated.coordinator().intent().redo_len(), 0);
        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint.clone(), "Add GUI cell")
            .unwrap()
            .expect("one outer publication");
        let receipt = publication.receipt;
        assert_eq!(receipt.before.revision, before_revision);
        assert_eq!(receipt.after.revision, before_revision + 1);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint);
        assert!(
            workbench
                .publish_delegated_editor_checkpoint(checkpoint, "Duplicate save")
                .unwrap()
                .is_none()
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            before_checkpoint,
        );
    }

    #[test]
    fn generated_polyline_point_edit_becomes_one_explicit_outer_override() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["shoulder"]
            })
            .unwrap()
            .clone();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let leaves = generated_point_leaves(&editor, expansion).unwrap();
        let mut operations = leaves
            .iter()
            .filter(|(_, owner)| *owner == &address)
            .map(|(leaf, _)| {
                let value = match leaf.field {
                    LeafField::X => 26.0,
                    LeafField::Y => 13.0,
                    _ => unreachable!("generated point owns only Cartesian leaves"),
                };
                geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: *leaf,
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                }
            })
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                leaf.field
            }
            _ => unreachable!(),
        });
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let checkpoint = encode_editor_checkpoint(&editor).unwrap();
        let revision = workbench.session.identity().revision;

        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint.clone(), "Drag generated point")
            .unwrap()
            .expect("terminal point drag publishes once");
        let receipt = publication.receipt;
        assert_eq!(receipt.before.revision, revision);
        assert_eq!(receipt.after.revision, revision + 1);
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &encode_editor_checkpoint(&publication.editor).unwrap()
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .map(|value| &value.value),
            Some(&ManagedValue::Array(vec![
                ManagedValue::Number(26.0),
                ManagedValue::Number(13.0),
            ])),
        );
        assert_eq!(
            restore_editor_checkpoint(&checkpoint)
                .unwrap()
                .coordinator()
                .intent()
                .undo_len(),
            0,
        );
        assert!(
            workbench
                .panel_markup()
                .contains("data-code-ownership=\"override\"")
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the braced-frame acceptance scenario proves the complete GUI-to-code-to-GUI transaction and exact Undo/Redo authority"
    )]
    fn braced_frame_corner_drag_rewrites_managed_source_and_preserves_custom_code() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let declaration = SemanticSymbol("frame".into());
        let alias = managed_rectangle_alias(&workbench.project, &expansion, &declaration).unwrap();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .unwrap();
        let corner = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 2,
            })
            .unwrap();
        let rectangle_node = node.id;
        let rectangle_corner = corner.id;

        // An ordinary GUI-authored dependent shares the moved code-owned
        // corner. Warm source publication must retain its declaration/native
        // owner and let the existing solver update its endpoint.
        let dependent_alias =
            geosolve_sketch_intent::IntentKey::new("gui-frame-dependent").unwrap();
        let dependent = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.frame-dependent").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 0),
            geosolve_sketch_intent::PatchPortRef::Stable {
                port: corner.as_ref(rectangle_node),
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: 78.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 42.0,
                unit: IntentUnit::Length,
            },
        );
        let dependent_outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: dependent_alias.clone(),
                    draft: Box::new(dependent),
                    cell: None,
                }],
            ))
            .unwrap();
        let dependent_node = dependent_outcome.aliases.node(&dependent_alias).unwrap();
        let dependent_definition = editor
            .coordinator()
            .intent()
            .graph()
            .node(dependent_node)
            .unwrap()
            .clone();
        let dependent_curve = native_binding_for_node(&editor, dependent_node, |binding| {
            matches!(binding, IntentNativeBinding::Curve(_))
        });
        let gui_publication = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI frame dependent",
            )
            .unwrap()
            .unwrap();
        editor = gui_publication.editor;

        let source_before = workbench.managed_source().to_owned();
        let custom_before = workbench.project.custom_files.clone();
        let code_ids_before = code_node_ids(&editor);
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let generated_before = generated_native_bindings(&editor, &expansion);
        let (brace_address, brace_identity) = generated_before
            .iter()
            .find(|(address, _)| address.template == ["diagonals", "rising"])
            .map(|(address, identity)| (address.clone(), *identity))
            .expect("generated rising brace");
        let brace_before = curve_definition_for_binding(&editor, brace_identity.1);
        let operations = [(LeafField::X, 64.0), (LeafField::Y, 38.0)]
            .into_iter()
            .map(
                |(field, value)| geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: LeafRef {
                        node: rectangle_node,
                        port: rectangle_corner,
                        field,
                    },
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .collect();
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let revision = workbench.session.identity().revision;
        let publication = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Drag managed frame corner",
            )
            .unwrap()
            .expect("one managed-source placement publication");

        assert_eq!(publication.receipt.before.revision, revision);
        assert_eq!(publication.receipt.after.revision, revision + 1);
        assert!(workbench.managed_source().contains("upperRight: [64, 38]"));
        assert_ne!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&publication.editor), code_ids_before);
        assert_eq!(
            publication
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_eq!(
            native_binding_for_node(&publication.editor, dependent_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            dependent_curve,
        );
        let generated_after = generated_native_bindings(
            &publication.editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        assert_eq!(generated_after.get(&brace_address), Some(&brace_identity));
        let brace_after = curve_definition_for_binding(&publication.editor, brace_identity.1);
        assert_ne!(
            brace_after, brace_before,
            "brace geometry did not follow frame"
        );
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &encode_editor_checkpoint(&publication.editor).unwrap(),
        );
        let after_checkpoint = workbench.accepted_editor_checkpoint().clone();
        assert_eq!(publication.editor.coordinator().intent().undo_len(), 0,);

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&undone.editor), code_ids_before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            before_checkpoint,
        );
        assert_eq!(
            curve_definition_for_binding(&undone.editor, brace_identity.1),
            brace_before,
        );
        assert_eq!(
            undone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_warm_cache_matches_session(&workbench);
        let redone = workbench.step_history(false).unwrap().unwrap();
        assert!(workbench.managed_source().contains("upperRight: [64, 38]"));
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&redone.editor), code_ids_before);
        assert_eq!(
            encode_editor_checkpoint(&redone.editor).unwrap(),
            after_checkpoint,
        );
        assert_eq!(
            curve_definition_for_binding(&redone.editor, brace_identity.1),
            brace_after,
        );
        assert_eq!(
            redone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the focused generated-reference regression keeps source/native identity, measured value, managed rewrite, exact history, and continued GUI editing together"
    )]
    fn braced_frame_gui_reference_dimension_survives_managed_rewrite_and_history() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let (_, brace_provenance) = expansion
            .generated_provenance
            .iter()
            .find(|(address, _)| address.template == ["diagonals", "rising"])
            .expect("generated rising brace provenance");
        let ExpandedSemanticTarget::Port { port: brace_port } = &brace_provenance.target else {
            panic!("generated rising brace must expose one stable span port")
        };
        let brace_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&brace_port.alias)
            .expect("generated rising brace declaration");
        let brace_node_id = brace_node.id;
        let brace_span_port = brace_node
            .port_by_selector(brace_port.selector)
            .expect("generated rising brace span")
            .as_ref(brace_node.id);
        let brace_binding = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(brace_span_port)
            .expect("generated rising brace native span");
        let IntentNativeBinding::CurveSpan(brace_span) = brace_binding else {
            panic!("generated rising brace port must own one native curve span")
        };

        let dimension_alias = geosolve_sketch_intent::IntentKey::new("gui-brace-length").unwrap();
        let dimension = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Dimension {
                dimension: geosolve_sketch_intent::DimensionKind::CurveLength,
            },
            geosolve_sketch_intent::IntentKey::new("Rising brace length").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Stable {
                port: brace_span_port,
            },
        )
        .with_field(
            geosolve_sketch_intent::IntentFieldKey(
                geosolve_sketch_intent::IntentKey::new("mode").unwrap(),
            ),
            IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("reference").unwrap()),
        );
        let dimension_outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: dimension_alias,
                    draft: Box::new(dimension),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI reference dimension");
        let dimension_node = dimension_outcome
            .aliases
            .node(&geosolve_sketch_intent::IntentKey::new("gui-brace-length").unwrap())
            .expect("GUI reference dimension node");
        let IntentNativeBinding::Dimension(dimension_id) =
            native_binding_for_node(&editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            })
        else {
            unreachable!("dimension predicate returned a non-dimension binding")
        };
        let dimension_source = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimension(dimension_id)
            .expect("native GUI reference dimension")
            .source_id;
        let published_dimension = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add rising brace reference dimension",
            )
            .unwrap()
            .expect("GUI reference dimension publishes once");
        editor = published_dimension.editor;

        let reference_value = |editor: &ProjectionalEditorSession| {
            let authority = editor
                .coordinator()
                .accepted_materialization()
                .expect("accepted native authority");
            let accepted = authority
                .session
                .accepted_state_for_current_input()
                .expect("current accepted scene");
            let value = accepted
                .reference_value(dimension_id)
                .expect("finite reference measurement");
            assert!(value.is_finite());
            let dimension = accepted
                .document()
                .dimension(dimension_id)
                .expect("accepted native reference dimension");
            assert_eq!(dimension.source_id, dimension_source);
            assert_eq!(
                dimension.mode,
                geosolve_sketch::DocumentDimensionMode::Reference
            );
            let geosolve_sketch::DocumentDimensionDefinition::CurveLength { curve, .. } =
                dimension.definition
            else {
                panic!("GUI reference dimension changed kind")
            };
            assert_eq!(curve, brace_span);
            let geosolve_sketch::CurveDefinition::Line { start, end, .. } = &accepted
                .document()
                .curve(curve.curve)
                .expect("accepted brace curve")
                .definition
            else {
                panic!("rising brace changed geometry family")
            };
            let start = accepted.document().point(*start).unwrap().position;
            let end = accepted.document().point(*end).unwrap().position;
            let measured = (end[0] - start[0]).hypot(end[1] - start[1]);
            assert!((value - measured).abs() <= 1.0e-9);
            value
        };
        let value_before = reference_value(&editor);
        let source_before = workbench.managed_source().to_owned();
        let custom_before = workbench.project.custom_files.clone();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let brace_before = curve_definition_for_binding(&editor, brace_binding);

        let frame_alias = managed_rectangle_alias(
            &workbench.project,
            &expansion,
            &SemanticSymbol("frame".into()),
        )
        .expect("managed frame declaration");
        let frame = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&frame_alias)
            .expect("managed frame node");
        let upper_right = frame
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 2,
            })
            .expect("managed upper-right corner");
        let operations = [(LeafField::X, 64.0), (LeafField::Y, 38.0)]
            .into_iter()
            .map(
                |(field, value)| geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: LeafRef {
                        node: frame.id,
                        port: upper_right.id,
                        field,
                    },
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .collect();
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .expect("managed frame corner edit");
        let rewritten = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Drag managed frame corner with GUI reference",
            )
            .unwrap()
            .expect("managed frame rewrite publishes once");
        let value_after = reference_value(&rewritten.editor);
        let brace_after = curve_definition_for_binding(&rewritten.editor, brace_binding);
        let source_after = workbench.managed_source().to_owned();
        let checkpoint_after = workbench.accepted_editor_checkpoint().clone();

        assert_ne!(source_after, source_before);
        assert!(source_after.contains("upperRight: [64, 38]"));
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_ne!(brace_after, brace_before);
        assert_ne!(value_after.to_bits(), value_before.to_bits());
        assert_eq!(
            rewritten
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&brace_port.alias)
                .unwrap()
                .id,
            brace_node_id,
        );
        assert_eq!(
            rewritten
                .editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .ownership
                .port(brace_span_port),
            Some(brace_binding),
        );
        assert_eq!(
            native_binding_for_node(&rewritten.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        let validation = &rewritten
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .validation;
        assert!(validation.hard_residuals_validated);
        assert!(validation.all_active_features_current);
        assert!(
            validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            checkpoint_before,
        );
        assert_eq!(
            curve_definition_for_binding(&undone.editor, brace_binding),
            brace_before
        );
        assert_eq!(
            reference_value(&undone.editor).to_bits(),
            value_before.to_bits()
        );
        assert_eq!(
            native_binding_for_node(&undone.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_warm_cache_matches_session(&workbench);

        let mut redone = workbench.step_history(false).unwrap().unwrap().editor;
        assert_eq!(workbench.managed_source(), source_after);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(encode_editor_checkpoint(&redone).unwrap(), checkpoint_after);
        assert_eq!(
            curve_definition_for_binding(&redone, brace_binding),
            brace_after
        );
        assert_eq!(reference_value(&redone).to_bits(), value_after.to_bits());
        assert_eq!(
            native_binding_for_node(&redone, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_warm_cache_matches_session(&workbench);

        redone
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                redone.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::RenameNode {
                    node: dimension_node,
                    name: geosolve_sketch_intent::IntentKey::new("Verified rising length").unwrap(),
                }],
            ))
            .expect("ordinary GUI dimension remains editable");
        let renamed = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&redone).unwrap(),
                "Rename rising brace reference dimension",
            )
            .unwrap()
            .expect("GUI dimension rename publishes once");
        assert_eq!(
            renamed
                .editor
                .coordinator()
                .intent()
                .organization()
                .node_names()
                .get(&dimension_node)
                .unwrap()
                .as_str(),
            "Verified rising length",
        );
        assert_eq!(
            native_binding_for_node(&renamed.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_eq!(
            reference_value(&renamed.editor).to_bits(),
            value_after.to_bits()
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn gui_dependent_survives_warm_apply_reload_and_blocks_source_removal() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let dependent = add_gui_horizontal_on_generated_span(
            &mut editor,
            &expansion,
            "shoulder",
            "gui.constraint.shoulder",
            true,
        );
        let dependent_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(dependent)
            .unwrap()
            .clone();
        let published = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI constraint",
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            published
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );

        workbench.set_managed_draft(
            workbench
                .managed_source()
                .replace("radius: mm(0.4)", "radius: mm(0.55)"),
        );
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid warm radius edit unexpectedly retained a failure")
        };
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );
        let persisted = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        assert_eq!(
            restored
                .restore_accepted_editor()
                .unwrap()
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );

        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(workbench.managed_source().replacen(
            "      { key: \"shoulder\", position: [24, 12] },\n",
            "",
            1,
        ));
        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("removing a generated source with an outside dependent was accepted")
        };
        assert!(receipt.retained_failure);
        assert!(diagnostic.contains("outside dependent"));
        assert_eq!(workbench.accepted_editor_checkpoint(), &accepted_checkpoint);
        assert_eq!(
            workbench
                .restore_accepted_editor()
                .unwrap()
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );
        assert!(workbench.step_history(true).unwrap().is_some());
        assert!(workbench.session.snapshot().failure.is_none());
        assert!(workbench.managed_source().contains("key: \"shoulder\""));
    }

    #[test]
    fn unmanaged_canvas_edit_of_code_declaration_is_rejected_without_divergence() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_revision = workbench.session.identity().revision;
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .find(|node| node.symbol.as_str().starts_with("code."))
            .unwrap()
            .id;
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::RenameNode {
                    node,
                    name: geosolve_sketch_intent::IntentKey::new("unmanaged GUI rename").unwrap(),
                }],
            ))
            .unwrap();
        let Err(diagnostic) = workbench.publish_delegated_editor_checkpoint(
            encode_editor_checkpoint(&editor).unwrap(),
            "Rename code declaration",
        ) else {
            panic!("unmanaged code declaration edit unexpectedly published")
        };
        assert!(diagnostic.contains("managed source"));
        assert_eq!(workbench.session.identity().revision, before_revision);
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&workbench.restore_accepted_editor().unwrap()).unwrap(),
            before_checkpoint,
        );
    }

    #[test]
    fn invalid_managed_text_is_only_a_diagnostic_draft_and_revert_is_exact() {
        let mut workbench = open("typed-panel");
        let canonical = workbench.managed_source().to_owned();
        workbench
            .set_managed_draft(canonical.replace("export default", "while (true) export default"));
        assert!(workbench.apply_managed_draft().is_err());
        assert_eq!(workbench.managed_source(), canonical);
        let markup = workbench.panel_markup();
        assert!(markup.contains("wb-code-diagnostic"));
        assert!(markup.contains("Line "));
        assert!(workbench.revert_managed_draft());
        assert_eq!(workbench.managed_draft, canonical);
        assert!(workbench.draft_diagnostic.is_none());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the override/reset regression keeps the unrelated GUI geometry and constraint identity oracle together"
    )]
    fn override_then_reset_preserves_unrelated_gui_geometry_and_constraint_ids() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let segment_alias = geosolve_sketch_intent::IntentKey::new("gui-segment").unwrap();
        let horizontal_alias =
            geosolve_sketch_intent::IntentKey::new("gui-segment-horizontal").unwrap();
        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let coordinate = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let segment = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.fixture.segment").unwrap(),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Start),
            LeafField::X,
            coordinate(-8.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Start),
            LeafField::Y,
            coordinate(-8.0),
        )
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, coordinate(8.0))
        .with_instance_leaf(
            selector(IntentPortRole::End),
            LeafField::Y,
            coordinate(-8.0),
        );
        let horizontal = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            },
            geosolve_sketch_intent::IntentKey::new("gui.fixture.horizontal").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Alias {
                node: segment_alias.clone(),
                selector: selector(IntentPortRole::Span),
            },
        );
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: segment_alias.clone(),
                        draft: Box::new(segment),
                        cell: None,
                    },
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: horizontal_alias.clone(),
                        draft: Box::new(horizontal),
                        cell: None,
                    },
                ],
            ))
            .unwrap();
        let segment = outcome.aliases.node(&segment_alias).unwrap();
        let horizontal = outcome.aliases.node(&horizontal_alias).unwrap();
        let segment_node = editor
            .coordinator()
            .intent()
            .graph()
            .node(segment)
            .unwrap()
            .clone();
        let horizontal_node = editor
            .coordinator()
            .intent()
            .graph()
            .node(horizontal)
            .unwrap()
            .clone();
        let curve = native_binding_for_node(&editor, segment, |binding| {
            matches!(binding, IntentNativeBinding::Curve(_))
        });
        let constraint = native_binding_for_node(&editor, horizontal, |binding| {
            matches!(binding, IntentNativeBinding::Constraint(_))
        });
        workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add unrelated GUI fixture",
            )
            .unwrap()
            .unwrap();
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["rise"]
            })
            .unwrap()
            .clone();
        let overridden = workbench
            .apply_override(
                &address,
                ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
            )
            .unwrap();
        assert_eq!(
            overridden
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(segment),
            Some(&segment_node),
        );
        assert_eq!(
            overridden
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(horizontal),
            Some(&horizontal_node),
        );
        assert_eq!(
            native_binding_for_node(&overridden.editor, segment, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            curve,
        );
        assert_eq!(
            native_binding_for_node(&overridden.editor, horizontal, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            constraint,
        );
        let reset = workbench
            .reset_override(&address.display_path())
            .unwrap()
            .unwrap();
        assert_eq!(
            reset.editor.coordinator().intent().graph().node(segment),
            Some(&segment_node),
        );
        assert_eq!(
            reset.editor.coordinator().intent().graph().node(horizontal),
            Some(&horizontal_node),
        );
        assert_eq!(
            native_binding_for_node(&reset.editor, segment, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            curve,
        );
        assert_eq!(
            native_binding_for_node(&reset.editor, horizontal, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            constraint,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn override_badge_reset_and_unified_undo_restore_one_code_session() {
        let mut workbench = open("rounded-polyline");
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["rise"]
            })
            .unwrap()
            .clone();
        workbench
            .apply_override(
                &address,
                ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
            )
            .unwrap();
        let overridden = workbench.panel_markup();
        assert!(overridden.contains("data-code-ownership=\"override\""));
        assert!(overridden.contains("Reset to code"));
        assert!(
            workbench
                .reset_override(&address.display_path())
                .unwrap()
                .is_some()
        );
        assert!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .is_none()
        );
        assert!(workbench.step_history(true).unwrap().is_some());
        assert!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .is_some()
        );
    }

    #[test]
    fn ordinary_inactive_surface_does_not_claim_a_code_project() {
        let markup = inactive_panel_markup();
        assert!(markup.contains("No code project open"));
        assert!(markup.contains("Ordinary sketches remain plain"));
        assert!(!markup.contains("wb-code-managed-source"));
    }

    #[test]
    fn ordinary_rectangle_diagonal_preview_uses_lexical_feature_references() {
        let editor = ordinary_rectangle_diagonal();
        let preview = OrdinaryCodePreview::from_editor(&editor).unwrap();
        assert!(
            preview
                .source
                .contains("const frame = $.geometry.rectangle")
        );
        assert!(preview.source.contains("const diagonal = $.geometry.line"));
        assert!(preview.source.contains("start: frame.corners.lowerLeft"));
        assert!(preview.source.contains("end: frame.corners.upperRight"));
        assert!(!preview.source.contains("{\"declaration\":"));

        let markup = preview.panel_markup();
        assert!(markup.contains("Managed code preview"));
        assert!(markup.contains("data-code-action=\"promote-ordinary\""));
        assert!(markup.contains("Read-only · not authority"));
        assert!(!markup.contains("<textarea"));
    }

    #[test]
    fn ordinary_promotion_creates_one_real_project_and_round_trips_without_demo_identity() {
        let editor = ordinary_rectangle_diagonal();
        let (workbench, promoted_editor) =
            CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        assert_eq!(workbench.demo_key(), None);
        assert_eq!(
            workbench.session.snapshot().code_project.as_ref(),
            Some(&workbench.project)
        );
        assert_eq!(
            workbench.session.snapshot().accepted_code_project.as_ref(),
            Some(&workbench.project)
        );
        assert!(
            promoted_editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .validation
                .hard_residuals_validated
        );

        let json = workbench.to_persistence_json().unwrap();
        let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["origin"]["kind"], "promoted");
        assert!(wire.get("demo").is_none());
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert_eq!(restored.demo_key(), None);
        assert_eq!(restored.managed_source(), workbench.managed_source());
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end promotion regression audits source rewrite, code history, semantic dependency and native movement together"
    )]
    fn promoted_rectangle_edit_rewrites_source_and_moves_lexically_dependent_line() {
        let editor = ordinary_rectangle_diagonal();
        let (mut workbench, mut promoted) =
            CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let frame = SemanticSymbol("frame".into());
        let frame_alias = managed_rectangle_alias(&workbench.project, &expansion, &frame).unwrap();
        let mut operations = managed_rectangle_leaves(&promoted, &workbench.project, &expansion)
            .unwrap()
            .into_iter()
            .filter_map(|(leaf, owner)| match owner {
                WritableCodeLeaf::ManagedRectangle { argument, .. } if argument == "upperRight" => {
                    Some(
                        geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                            leaf,
                            value: IntentLiteral::Quantity {
                                value: match leaf.field {
                                    LeafField::X => 10.0,
                                    LeafField::Y => 6.0,
                                    _ => unreachable!("rectangle placement owns Cartesian leaves"),
                                },
                                unit: IntentUnit::Length,
                            },
                        },
                    )
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                leaf.field
            }
            _ => unreachable!(),
        });
        promoted
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                promoted.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let checkpoint = encode_editor_checkpoint(&promoted).unwrap();
        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint, "Drag promoted frame")
            .unwrap()
            .unwrap();
        assert!(workbench.managed_source().contains("upperRight: [10, 6]"));
        assert_eq!(
            managed_rectangle_argument_position(&publication.editor, &frame_alias, "upperRight")
                .unwrap()
                .map(f64::to_bits),
            [10.0, 6.0].map(f64::to_bits),
        );

        let accepted_expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let line_alias = accepted_expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("diagonal".into())
                    && output.reference.output.0.is_empty())
                .then(|| match &output.target {
                    ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                    _ => None,
                })
                .flatten()
            })
            .unwrap();
        let line = publication
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&line_alias)
            .unwrap();
        let end = line
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            })
            .unwrap();
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let IntentNativeBinding::Point(end) = accepted.ownership.port(end.as_ref(line.id)).unwrap()
        else {
            panic!("managed line end must resolve to a native point")
        };
        assert_eq!(
            accepted
                .session
                .design_document()
                .point(end)
                .unwrap()
                .position
                .map(f64::to_bits),
            [10.0, 6.0].map(f64::to_bits),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the regression proves both expansion-owned inclusion and ordinary explicit-branch exclusion with one shared native authority"
    )]
    fn recomputable_code_line_branches_exclude_ordinary_gui_segments() {
        let editor = ordinary_rectangle_diagonal();
        let (workbench, mut promoted) = CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let managed_line_alias = expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("diagonal".into()))
                    .then(|| match &output.target {
                        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                        _ => None,
                    })
                    .flatten()
            })
            .expect("managed line declaration");
        let managed_line_node = promoted
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&managed_line_alias)
            .expect("managed line node")
            .id;
        let IntentNativeBinding::Curve(managed_line_curve) =
            native_binding_for_node(&promoted, managed_line_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            })
        else {
            panic!("managed line must own one native curve")
        };

        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let gui_alias = geosolve_sketch_intent::IntentKey::new("gui-parity-segment").unwrap();
        let gui_segment = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.parity-segment").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::X, length(20.0))
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::Y, length(20.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(30.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(20.0))
        .with_field(
            geosolve_sketch_intent::IntentFieldKey(
                geosolve_sketch_intent::IntentKey::new("branch_direction").unwrap(),
            ),
            IntentLiteral::Point([1.0, 0.0]),
        );
        let outcome = promoted
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                promoted.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: gui_alias.clone(),
                    draft: Box::new(gui_segment),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI Segment");
        let gui_node = outcome.aliases.node(&gui_alias).expect("ordinary GUI node");
        let IntentNativeBinding::Curve(gui_curve) =
            native_binding_for_node(&promoted, gui_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            })
        else {
            panic!("ordinary GUI Segment must own one native curve")
        };

        let recomputable = recomputable_code_line_branches(&promoted, &expansion).unwrap();
        assert!(
            recomputable.contains(&managed_line_curve),
            "the expansion-owned managed line remains source-derived"
        );
        assert!(
            !recomputable.contains(&gui_curve),
            "an ordinary GUI Segment must retain exact explicit branch authority"
        );

        let document = promoted
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .clone();
        let current = match &document.curve(gui_curve).unwrap().definition {
            geosolve_sketch::CurveDefinition::Line {
                branch_direction, ..
            } => *branch_direction,
            _ => panic!("ordinary GUI Segment must materialize as a line"),
        };
        let (sine, cosine) = 1.0e-10_f64.sin_cos();
        let replacement = [
            cosine * current[0] - sine * current[1],
            sine * current[0] + cosine * current[1],
        ];
        let mut same_cell_mismatch = document.clone();
        same_cell_mismatch
            .set_curve_branch(geosolve_sketch::CurveSpan::line(gui_curve), replacement)
            .expect("same-cell explicit branch mismatch remains a valid document");

        let mut intentionally_overbroad = recomputable.clone();
        intentionally_overbroad.insert(gui_curve);
        assert!(
            document.exact_except_recomputable_line_branches(
                &same_cell_mismatch,
                &intentionally_overbroad,
            ),
            "the fixture must exercise the normalization leak"
        );
        assert!(
            !document.exact_except_recomputable_line_branches(&same_cell_mismatch, &recomputable,),
            "the exact expansion-owned set must reject an ordinary Segment mismatch"
        );
    }

    #[test]
    fn static_workbench_has_one_hidden_optional_code_projection_and_bounded_editor_styles() {
        let html = include_str!("../../index.html");
        let css = include_str!("../../styles.css");
        assert_eq!(html.matches("id=\"wb-design-tab-code\"").count(), 1);
        assert_eq!(html.matches("id=\"wb-design-code\"").count(), 1);
        assert!(html.contains("data-wb-design-tab=\"code\""));
        assert!(html.contains("id=\"wb-design-tab-code\"") && html.contains("hidden>Code"));
        assert!(html.contains(">Intent IR</button>"));
        assert!(!html.contains(">Structured source</button>"));
        for selector in [
            ".wb-code-project",
            ".wb-code-file-tabs",
            ".wb-code-editor textarea",
            ".wb-code-artifact-status",
            ".wb-code-lenses",
            ".wb-code-member",
        ] {
            assert!(css.contains(selector), "missing `{selector}`");
        }
        assert!(css.contains("max-height: 27rem"));
    }

    #[test]
    fn wasm_adapter_routes_code_projects_only_at_durable_boundaries() {
        let source = include_str!("mod.rs");
        for route in [
            "data-code-sample-id",
            "open_projectional_code_project",
            "promote_projectional_code_project",
            "\"promote-ordinary\" =>",
            "apply_managed_draft()",
            "reset_override(&path)",
            "to_persistence_json()",
            "from_persistence_json(&decoded)",
        ] {
            assert!(source.contains(route), "missing durable route `{route}`");
        }
        let transient = source
            .split("fn render_projectional_canvas(")
            .nth(1)
            .and_then(|source| source.split("fn save_projectional(").next())
            .expect("transient projectional renderer");
        for forbidden in [
            "parse_managed_source",
            "apply_managed_draft",
            "panel_markup",
            "to_persistence_json",
        ] {
            assert!(
                !transient.contains(forbidden),
                "pointer-frame renderer admitted `{forbidden}`"
            );
        }
        let runtime_evaluation_call = ["ev", "al("].concat();
        assert!(!include_str!("code_projects.rs").contains(&runtime_evaluation_call));
    }

    #[test]
    fn complete_offline_project_session_draft_and_file_selection_round_trip() {
        let mut workbench = open("mounting-plate");
        workbench
            .select_file("patches/mounting-plate.patch.ts")
            .unwrap();
        let custom_before = workbench
            .project
            .custom_files
            .values()
            .map(|file| (file.path.clone(), file.contents.clone()))
            .collect::<BTreeMap<_, _>>();
        workbench.set_managed_draft(format!(
            "{}\n// unapplied presentation draft",
            workbench.managed_source()
        ));
        let json = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert_eq!(
            restored.selected_file.path(),
            "patches/mounting-plate.patch.ts"
        );
        assert!(
            restored
                .managed_draft
                .ends_with("unapplied presentation draft")
        );
        assert_eq!(
            restored
                .project
                .custom_files
                .values()
                .map(|file| (file.path.clone(), file.contents.clone()))
                .collect::<BTreeMap<_, _>>(),
            custom_before,
        );
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    fn historical_bundled_demo_wire_migrates_to_explicit_origin() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["origin"]["kind"], "bundled");
        assert_eq!(wire["origin"]["demo"], "braced-frame");
        wire.as_object_mut().unwrap().remove("origin");
        wire["demo"] = serde_json::Value::String("braced-frame".into());

        let restored = CodeProjectWorkbench::from_persistence_json(&wire.to_string()).unwrap();
        assert_eq!(restored.demo_key(), Some("braced-frame"));
        let migrated: serde_json::Value =
            serde_json::from_str(&restored.to_persistence_json().unwrap()).unwrap();
        assert_eq!(migrated["origin"]["kind"], "bundled");
        assert!(migrated.get("demo").is_none());
    }

    #[test]
    fn tampered_nested_code_session_rejects_before_restore() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        let session = wire["session"].as_str().unwrap();
        wire["session"] = serde_json::Value::String(
            session.replace("m84-demo-braced-frame", "m84-demo-tampered-frame"),
        );
        assert!(CodeProjectWorkbench::from_persistence_json(&wire.to_string()).is_err());
    }
}
