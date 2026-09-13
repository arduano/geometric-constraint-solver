// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only source Inspector authority shared by native and browser hosts.

use crate::{
    CodeGeneratedChildAddress, CodeOwnerAddress, CodeSessionSnapshot, ManagedControl,
    ManagedControlAccess, ManagedControlConsumerTarget, ManagedControlManifest,
    ManagedControlReadOnlyReason, ManagedPathSegment, ManagedSpan, MaterializedCodeProject,
    SemanticOutputPath, SemanticSymbol,
};
use geosolve_constraint_editor::{
    IntentInspectorEditTarget, IntentInspectorField, IntentInspectorProjection,
    IntentWorkbenchProjection, ProjectionalEditorSession,
};
use geosolve_sketch_intent::{
    IntentDefinitionFieldDescriptor, IntentFieldKey, IntentNodeKind, IntentOutputDescriptor,
    IntentPortKind, IntentProjectionPath, LeafRef,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Borrows accepted source and materialized native authority without owning publication.
/// A missing materialization preserves source navigation but refuses Inspector authority.
#[derive(Debug)]
pub struct ManagedSourceInspector<'a> {
    snapshot: &'a CodeSessionSnapshot,
    materialized: Option<&'a MaterializedCodeProject>,
}

impl<'source> ManagedSourceInspector<'source> {
    pub fn new(
        snapshot: &'source CodeSessionSnapshot,
        materialized: Option<&'source MaterializedCodeProject>,
    ) -> Self {
        Self {
            snapshot,
            materialized,
        }
    }

    /// Resolves the currently selected intent declaration back to the exact
    /// managed semantic owner published by code expansion.
    ///
    /// The implementation deliberately does not decode the hashed `code.*`
    /// developer symbol. An ordinary GUI-owned declaration is represented by
    /// `None`; every expansion-owned declaration must be present in the
    /// authenticated provenance map.
    ///
    /// # Errors
    /// Rejects unavailable declarations, expansions, or managed-source provenance.
    pub fn selected_managed_declaration(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<SemanticSymbol>, String> {
        let Some(node_id) = editor.selected_declaration() else {
            return Ok(None);
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(node_id)
            .ok_or_else(|| "selected declaration is absent from current intent".to_owned())?;
        let snapshot = self.snapshot;
        let expansion = snapshot
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        if let Some(child) = expansion.generated_child_for_alias(&node.symbol)
            && let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address
        {
            let declaration = SemanticSymbol(address.invocation.clone());
            if snapshot
                .managed
                .program
                .declarations
                .iter()
                .any(|candidate| candidate.symbol == declaration)
            {
                return Ok(Some(declaration));
            }
            return Err(
                "selected generated declaration has no authenticated managed-source invocation"
                    .into(),
            );
        }
        match expansion.declaration_for_alias(&node.symbol) {
            Some(declaration) => Ok(Some(declaration.clone())),
            None if node.symbol.as_str().starts_with("code.") => Err(
                "selected code-owned declaration has no authenticated managed-source provenance"
                    .into(),
            ),
            None => Ok(None),
        }
    }

    /// Exact accepted `sketch.ts` statement owned by one managed declaration.
    ///
    /// This is intentionally separate from individual managed-control spans:
    /// a source-backed declaration such as a direct Fillet owns several
    /// independently editable leaves, while selection-level navigation needs
    /// one honest common source region.
    ///
    /// # Errors
    /// Rejects declarations absent from the accepted managed source.
    pub fn managed_declaration_source_span(
        &self,
        declaration: &SemanticSymbol,
    ) -> Result<ManagedSpan, String> {
        let snapshot = self.snapshot;
        let managed = snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&snapshot.managed, |project| &project.managed);
        managed
            .program
            .declarations
            .iter()
            .find(|candidate| &candidate.symbol == declaration)
            .map(|candidate| candidate.statement_span)
            .ok_or_else(|| {
                format!(
                    "accepted managed declaration `{}` has no authenticated source statement",
                    declaration.0,
                )
            })
    }

    /// Uses the caller's one durable-render projection and managed manifest.
    /// The Code panel and Inspector therefore share the same transient control
    /// authority without either rebuilding it or rescanning complete fan-out.
    ///
    /// # Errors
    /// Rejects stale native projections, missing accepted authority, unauthenticated
    /// code ownership, missing schema paths, or ambiguous source control routes.
    pub fn inspector_parameter_presentations(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
        descriptors: &InspectorDescriptorIndex<'_>,
        manifest: Result<&ManagedControlManifest, &str>,
    ) -> Result<Vec<InspectorParameterPresentation>, String> {
        let Some(context) =
            self.managed_inspector_context(editor, projection, inspector, manifest)?
        else {
            return Ok(Vec::new());
        };
        let targets = std::iter::once(IntentInspectorEditTarget::Suppressed).chain(
            inspector.fields.iter().map(|field| match field {
                IntentInspectorField::Definition { definition, .. } => {
                    IntentInspectorEditTarget::Definition {
                        field: definition.clone(),
                    }
                }
                IntentInspectorField::Instance { leaf, .. } => {
                    IntentInspectorEditTarget::Instance { leaf: *leaf }
                }
            }),
        );
        targets
            .map(|target| {
                let authority =
                    match Self::resolve_managed_inspector_property(&context, descriptors, &target)?
                    {
                        ManagedInspectorPropertyResolution::ModifiableSource(control) => {
                            let path = managed_source_path(control);
                            let generated_consumer_count = control
                                .consumers
                                .iter()
                                .filter(|consumer| {
                                    matches!(
                                        consumer.target,
                                        ManagedControlConsumerTarget::Generated { .. }
                                    )
                                })
                                .count();
                            InspectorParameterAuthority::ModifiableSource {
                                control_id: control.id.0.clone(),
                                source_start: control.source.span.start,
                                source_end: control.source.span.end,
                                source_path: path,
                                source_text: control.source.source_text.clone(),
                                consumer_count: control.consumers.len(),
                                generated_consumer_count,
                            }
                        }
                        ManagedInspectorPropertyResolution::ModifiableInstance => {
                            InspectorParameterAuthority::ModifiableInstance
                        }
                        ManagedInspectorPropertyResolution::Encoded { reason } => {
                            InspectorParameterAuthority::Encoded { reason }
                        }
                        ManagedInspectorPropertyResolution::Blocked { reason } => {
                            InspectorParameterAuthority::Blocked { reason }
                        }
                    };
                Ok(InspectorParameterPresentation { target, authority })
            })
            .collect()
    }

    fn managed_inspector_scope(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
    ) -> Result<Option<ManagedInspectorScope>, String> {
        if projection.identity != inspector.identity
            || editor.coordinator().intent().identity() != inspector.identity
            || editor.selected_declaration() != Some(inspector.node)
        {
            return Err("the Inspector belongs to a stale code-project projection".into());
        }
        if self
            .materialized
            .map(|materialized| materialized.editor.coordinator().intent().identity())
            != Some(editor.coordinator().intent().identity())
        {
            return Err("the Inspector does not match accepted code-project authority".into());
        }
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(inspector.node)
            .ok_or_else(|| "the selected Inspector declaration disappeared".to_owned())?;
        if node.symbol != inspector.symbol {
            return Err("the selected Inspector symbol no longer matches its declaration".into());
        }
        let owner = {
            let expansion = self
                .snapshot
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
            if let Some(child) = expansion.generated_child_for_alias(&node.symbol) {
                Some(ManagedInspectorOwner::Generated(child.address.clone()))
            } else {
                expansion
                    .declaration_for_alias(&node.symbol)
                    .cloned()
                    .map(ManagedInspectorOwner::Declaration)
            }
        };
        let Some(owner) = owner else {
            if node.symbol.as_str().starts_with("code.") {
                return Err(
                    "selected code-owned declaration has no authenticated managed-source provenance"
                        .into(),
                );
            }
            return Ok(None);
        };
        Ok(Some(ManagedInspectorScope {
            owner,
            is_dimension: matches!(node.kind, IntentNodeKind::Dimension { .. }),
        }))
    }

    fn managed_inspector_context<'a>(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
        manifest: Result<&'a ManagedControlManifest, &str>,
    ) -> Result<Option<ManagedInspectorContext<'a>>, String> {
        let Some(scope) = self.managed_inspector_scope(editor, projection, inspector)? else {
            return Ok(None);
        };
        let blocked_reason = self.snapshot.failure.as_ref().map_or_else(
            || manifest.as_ref().err().map(|reason| (*reason).to_owned()),
            |_| {
                Some(
                    "Resolve or Undo the retained code failure before modifying this parameter"
                        .into(),
                )
            },
        );
        Ok(Some(indexed_managed_inspector_context(
            scope.owner,
            scope.is_dimension,
            manifest.ok(),
            blocked_reason,
        )))
    }

    fn managed_inspector_property_path(
        descriptors: &InspectorDescriptorIndex<'_>,
        target: &IntentInspectorEditTarget,
    ) -> Result<(SemanticOutputPath, bool), String> {
        Ok(match target {
            IntentInspectorEditTarget::Suppressed => (
                SemanticOutputPath(vec![ManagedPathSegment::Field("suppressed".into())]),
                false,
            ),
            IntentInspectorEditTarget::Definition { field } => descriptors
                .definition(field)
                .map(|descriptor| (semantic_output_path(&descriptor.path), false))
                .ok_or_else(|| {
                    "the managed Inspector definition has no current schema path".to_owned()
                })?,
            IntentInspectorEditTarget::Instance { leaf } => {
                let (descriptor, path) = descriptors.instance(*leaf).ok_or_else(|| {
                    "the managed Inspector instance leaf has no current output descriptor"
                        .to_owned()
                })?;
                (
                    semantic_output_path(path),
                    descriptor.kind == IntentPortKind::Point,
                )
            }
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed resolver authenticates every Inspector target against semantic owner, manifest consumer, schema path, access token, and truthful fallback authority"
    )]
    fn resolve_managed_inspector_property<'a>(
        context: &'a ManagedInspectorContext<'a>,
        descriptors: &InspectorDescriptorIndex<'_>,
        target: &IntentInspectorEditTarget,
    ) -> Result<ManagedInspectorPropertyResolution<'a>, String> {
        let (property, solver_instance_fallback) =
            Self::managed_inspector_property_path(descriptors, target)?;
        if let Some(reason) = &context.blocked_reason {
            return Ok(ManagedInspectorPropertyResolution::Blocked {
                reason: reason.clone(),
            });
        }
        if solver_instance_fallback {
            return Ok(ManagedInspectorPropertyResolution::ModifiableInstance);
        }
        if let Some(matching) = context.routes.get(&property) {
            let [index] = matching.as_slice() else {
                return Err(
                    "the managed Inspector property resolves to more than one source control"
                        .into(),
                );
            };
            let control = context.controls[*index];
            return Ok(match &control.access {
                ManagedControlAccess::Editable { .. } => {
                    ManagedInspectorPropertyResolution::ModifiableSource(control)
                }
                ManagedControlAccess::ReadOnly { reason, navigation } => {
                    ManagedInspectorPropertyResolution::Encoded {
                        reason: managed_read_only_reason(*reason, navigation.as_ref()),
                    }
                }
            });
        }
        if context.families.len() > 1 {
            return Err("one Inspector owner resolves to inconsistent generated families".into());
        }
        let family = context
            .families
            .first()
            .map(|family| managed_family_label(family));
        Ok(ManagedInspectorPropertyResolution::Encoded {
            reason: match (&context.owner, family) {
                (ManagedInspectorOwner::Generated(_), Some(family)) => {
                    format!("Generated {family} state · not declared in sketch.ts")
                }
                (ManagedInspectorOwner::Generated(_), None) => {
                    "Generated native state · not declared in sketch.ts".into()
                }
                (ManagedInspectorOwner::Declaration(_), Some(family)) => {
                    format!("Code-owned {family} state · not declared in sketch.ts")
                }
                (ManagedInspectorOwner::Declaration(_), None) => {
                    "Code-owned native state · not declared in sketch.ts".into()
                }
            },
        })
    }
}

/// Explicit authority shown beside a code-owned Inspector parameter. These
/// rows are presentation metadata only: the mutation adapter independently
/// re-resolves the same target against a fresh managed-control manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectorParameterAuthority {
    ModifiableSource {
        control_id: String,
        source_start: usize,
        source_end: usize,
        source_path: String,
        source_text: String,
        consumer_count: usize,
        generated_consumer_count: usize,
    },
    ModifiableInstance,
    Encoded {
        reason: String,
    },
    Blocked {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectorParameterPresentation {
    pub target: IntentInspectorEditTarget,
    pub authority: InspectorParameterAuthority,
}

/// One bounded target-to-schema index shared by managed-parameter authority
/// derivation and Inspector markup. The central descriptor remains the source
/// of truth; this view only prevents repeated linear scans of it for every
/// projected field.
#[derive(Debug)]
pub struct InspectorDescriptorIndex<'a> {
    definitions: BTreeMap<IntentFieldKey, &'a IntentDefinitionFieldDescriptor>,
    instances: BTreeMap<LeafRef, (&'a IntentOutputDescriptor, IntentProjectionPath)>,
}

impl<'a> InspectorDescriptorIndex<'a> {
    /// Indexes the editor's unique central schema descriptors.
    ///
    /// # Panics
    /// Panics if a descriptor repeats a definition or writable leaf, or declares
    /// a writable leaf outside its output. Native central descriptors are valid.
    pub fn new(inspector: &'a IntentInspectorProjection) -> Self {
        let mut definitions = BTreeMap::new();
        for descriptor in &inspector.descriptor.fields {
            let replaced = definitions.insert(descriptor.schema.field.clone(), descriptor);
            assert!(
                replaced.is_none(),
                "central Inspector definition descriptors must be unique"
            );
        }

        let mut instances = BTreeMap::new();
        for output in &inspector.descriptor.outputs {
            for field in &output.writable {
                let leaf = LeafRef {
                    node: output.port.node,
                    port: output.port.port,
                    field: *field,
                };
                let path = output
                    .path_for_leaf(leaf)
                    .expect("descriptor output owns its declared writable leaf");
                let replaced = instances.insert(leaf, (output, path));
                assert!(
                    replaced.is_none(),
                    "central Inspector writable-leaf descriptors must be unique"
                );
            }
        }
        Self {
            definitions,
            instances,
        }
    }

    pub fn definition(
        &self,
        field: &IntentFieldKey,
    ) -> Option<&'a IntentDefinitionFieldDescriptor> {
        self.definitions.get(field).copied()
    }

    pub fn instance(
        &self,
        leaf: LeafRef,
    ) -> Option<(&'a IntentOutputDescriptor, &IntentProjectionPath)> {
        self.instances
            .get(&leaf)
            .map(|(output, path)| (*output, path))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ManagedInspectorPropertyResolution<'a> {
    ModifiableSource(&'a ManagedControl),
    ModifiableInstance,
    Encoded { reason: String },
    Blocked { reason: String },
}

#[derive(Clone, Debug)]
enum ManagedInspectorOwner {
    Generated(CodeGeneratedChildAddress),
    Declaration(SemanticSymbol),
}

#[derive(Clone, Debug)]
struct ManagedInspectorContext<'a> {
    owner: ManagedInspectorOwner,
    controls: Vec<&'a ManagedControl>,
    routes: BTreeMap<SemanticOutputPath, Vec<usize>>,
    families: BTreeSet<String>,
    blocked_reason: Option<String>,
}

struct ManagedInspectorScope {
    owner: ManagedInspectorOwner,
    is_dimension: bool,
}

fn managed_consumer_matches_inspector_owner(
    consumer: &ManagedControlConsumerTarget,
    owner: &ManagedInspectorOwner,
) -> bool {
    match (consumer, owner) {
        (
            ManagedControlConsumerTarget::Generated {
                address, identity, ..
            },
            ManagedInspectorOwner::Generated(child),
        ) => {
            matches!(
                &child.owner.address,
                CodeOwnerAddress::GeneratedMember { address: child_address }
                    if child_address == address
            ) && child.owner.allocation == identity.allocation
                && child.owner.generation == identity.generation
        }
        (
            ManagedControlConsumerTarget::Declaration {
                declaration: candidate,
                ..
            },
            ManagedInspectorOwner::Declaration(declaration),
        ) => candidate == declaration,
        _ => false,
    }
}

fn indexed_managed_inspector_context(
    owner: ManagedInspectorOwner,
    is_dimension: bool,
    manifest: Option<&ManagedControlManifest>,
    blocked_reason: Option<String>,
) -> ManagedInspectorContext<'_> {
    let mut controls = Vec::new();
    let mut routes = BTreeMap::<SemanticOutputPath, Vec<usize>>::new();
    let mut families = BTreeSet::new();
    for control in manifest.into_iter().flat_map(|manifest| &manifest.controls) {
        let mut properties = BTreeSet::new();
        for consumer in &control.consumers {
            if !managed_consumer_matches_inspector_owner(&consumer.target, &owner) {
                continue;
            }
            let family = match &consumer.target {
                ManagedControlConsumerTarget::Declaration { family, .. }
                | ManagedControlConsumerTarget::Generated { family, .. } => family,
            };
            families.insert(family.clone());
            properties.insert(consumer.property.clone());

            // Direct dimension source spells the driving literal `target`,
            // while the central Inspector schema presents it as `value`.
            if is_dimension
                && matches!(
                    &consumer.target,
                    ManagedControlConsumerTarget::Declaration { .. }
                )
                && family.starts_with("dimension.")
                && matches!(
                    control.source.path.0.as_slice(),
                    [ManagedPathSegment::Field(source)] if source == "target"
                )
            {
                properties.insert(SemanticOutputPath(vec![ManagedPathSegment::Field(
                    "value".into(),
                )]));
            }
        }
        if properties.is_empty() {
            continue;
        }
        let index = controls.len();
        controls.push(control);
        for property in properties {
            routes.entry(property).or_default().push(index);
        }
    }
    ManagedInspectorContext {
        owner,
        controls,
        routes,
        families,
        blocked_reason,
    }
}

fn semantic_output_path(path: &geosolve_sketch_intent::IntentProjectionPath) -> SemanticOutputPath {
    SemanticOutputPath(
        path.segments()
            .iter()
            .map(|segment| match segment {
                geosolve_sketch_intent::IntentProjectionPathSegment::Field(field) => {
                    ManagedPathSegment::Field(field.as_str().to_owned())
                }
                geosolve_sketch_intent::IntentProjectionPathSegment::Index(index) => {
                    ManagedPathSegment::Index(usize::from(*index))
                }
            })
            .collect(),
    )
}

/// Formats an exact managed path using declaration field and member notation.
pub fn managed_path_text(path: &[ManagedPathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            ManagedPathSegment::Field(field) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(field);
            }
            ManagedPathSegment::Index(index) => {
                let _ = write!(text, "[{index}]");
            }
            ManagedPathSegment::Member { member } => {
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

/// Formats the declaration-qualified path of one source control.
pub fn managed_source_path(control: &ManagedControl) -> String {
    let path = managed_path_text(&control.source.path.0);
    if control.source.path.0.is_empty() {
        control.source.declaration.0.clone()
    } else {
        format!("{}.{}", control.source.declaration.0, path)
    }
}

fn managed_navigation_path(navigation: &crate::ManagedControlNavigation) -> String {
    if navigation.path.0.is_empty() {
        navigation.declaration.0.clone()
    } else {
        format!(
            "{}.{}",
            navigation.declaration.0,
            managed_path_text(&navigation.path.0),
        )
    }
}

fn managed_read_only_reason(
    reason: ManagedControlReadOnlyReason,
    navigation: Option<&crate::ManagedControlNavigation>,
) -> String {
    let reason = match reason {
        ManagedControlReadOnlyReason::Structure => "Structural source value",
        ManagedControlReadOnlyReason::Reference => "Source reference",
        ManagedControlReadOnlyReason::StructuralIdentity => "Structural identity",
        ManagedControlReadOnlyReason::SolverInstance => "Solver-owned instance value",
        ManagedControlReadOnlyReason::Null => "Null source value",
        ManagedControlReadOnlyReason::Absent => "Absent source value",
        ManagedControlReadOnlyReason::IncompatibleSchemas => "Incompatible consumer schemas",
        ManagedControlReadOnlyReason::UnprovenTransform => "Unproven source transform",
    };
    navigation.map_or_else(
        || format!("{reason} · no directly writable sketch.ts property"),
        |navigation| {
            format!(
                "{reason} · navigate to {} in sketch.ts",
                managed_navigation_path(navigation),
            )
        },
    )
}

fn managed_family_label(family: &str) -> String {
    let leaf = family.rsplit('.').next().unwrap_or(family);
    let mut characters = leaf.chars();
    characters.next().map_or_else(
        || "native".into(),
        |first| {
            let mut label = first.to_uppercase().collect::<String>();
            label.extend(characters);
            label
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey, SketchCodeSession,
    };

    fn fixture() -> (
        SketchCodeSession,
        MaterializedCodeProject,
        ManagedControlManifest,
    ) {
        let compiled = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-two-circles.json"
        )))
        .unwrap();
        let project =
            CodeProject::managed(ProjectKey("source-inspector".into()), compiled).unwrap();
        let generated = KeyedReconcileState::empty()
            .plan(
                crate::required_generated_members(&project).unwrap(),
                &BTreeSet::new(),
            )
            .unwrap()
            .into_staged();
        let mut materialized = crate::materialize_code_project_cold(
            &project,
            &generated,
            geosolve_sketch_intent::IntentSessionId::from_raw(0x99_1a01),
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x99_1a01)),
            1.0,
        )
        .unwrap();
        materialized.editor = materialized
            .editor
            .into_delegated_accepted_authority()
            .unwrap();
        let manifest = crate::managed_control_manifest(&project, &materialized.expansion).unwrap();
        let session = SketchCodeSession::new_project(
            project,
            generated,
            materialized.expansion.clone(),
            crate::encode_editor_checkpoint(&materialized.editor).unwrap(),
        )
        .unwrap();
        (session, materialized, manifest)
    }

    fn select_circle(materialized: &MaterializedCodeProject) -> ProjectionalEditorSession {
        let mut editor = materialized.editor.fork_accepted_authority().unwrap();
        let alias = materialized
            .expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| (declaration.0 == "geometry1").then_some(alias))
            .unwrap();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .find(|node| &node.symbol == alias)
            .unwrap()
            .id;
        editor.set_selected_declaration(Some(node));
        editor
    }

    fn inspector(editor: &ProjectionalEditorSession) -> IntentInspectorProjection {
        IntentInspectorProjection::from_session(
            editor.coordinator().intent(),
            editor.selected_declaration().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn accepted_inspector_preserves_source_token_span_and_native_point_authority() {
        let (session, materialized, manifest) = fixture();
        let editor = select_circle(&materialized);
        let basis = session.snapshot().clone();
        let native = std::ptr::from_ref(editor.coordinator().accepted_materialization().unwrap());
        let source = ManagedSourceInspector::new(session.snapshot(), Some(&materialized));
        let symbol = source
            .selected_managed_declaration(&editor)
            .unwrap()
            .unwrap();
        assert_eq!(symbol.0, "geometry1");
        let span = source.managed_declaration_source_span(&symbol).unwrap();
        assert!(basis.managed.source[span.start..span.end].contains("centerRadiusCircle"));
        let inspector = inspector(&editor);
        let presentations = source
            .inspector_parameter_presentations(
                &editor,
                &editor.workbench_projection(),
                &inspector,
                &InspectorDescriptorIndex::new(&inspector),
                Ok(&manifest),
            )
            .unwrap();
        let radius = manifest
            .controls
            .iter()
            .find(|control| managed_source_path(control) == "geometry1.radius")
            .unwrap();
        assert!(presentations.iter().any(|presentation| {
            matches!(&presentation.authority, InspectorParameterAuthority::ModifiableSource {
                control_id, source_start, source_end, source_path, source_text,
                consumer_count, generated_consumer_count,
            } if control_id == &radius.id.0 && source_start == &radius.source.span.start
                && source_end == &radius.source.span.end && source_path == "geometry1.radius"
                && source_text == &radius.source.source_text && *consumer_count == radius.consumers.len()
                && *generated_consumer_count == 0)
        }));
        assert!(presentations.iter().any(|presentation| {
            matches!(
                presentation.target,
                IntentInspectorEditTarget::Instance { .. }
            ) && presentation.authority == InspectorParameterAuthority::ModifiableInstance
        }));
        assert_eq!(source.snapshot, &basis);
        assert_eq!(
            std::ptr::from_ref(editor.coordinator().accepted_materialization().unwrap()),
            native
        );
    }

    #[test]
    fn accepted_inspector_refuses_stale_native_identity_and_ambiguous_source_routes() {
        let (session, materialized, manifest) = fixture();
        let editor = select_circle(&materialized);
        let source = ManagedSourceInspector::new(session.snapshot(), Some(&materialized));
        let inspector = inspector(&editor);
        let projection = editor.workbench_projection();
        let descriptors = InspectorDescriptorIndex::new(&inspector);
        let without_materialization = ManagedSourceInspector::new(session.snapshot(), None);
        assert_eq!(
            without_materialization
                .inspector_parameter_presentations(
                    &editor,
                    &projection,
                    &inspector,
                    &descriptors,
                    Ok(&manifest),
                )
                .unwrap_err(),
            "the Inspector does not match accepted code-project authority"
        );
        let mut stale = inspector.clone();
        stale.identity.session = geosolve_sketch_intent::IntentSessionId::from_raw(0x99_1aff);
        assert_eq!(
            source
                .inspector_parameter_presentations(
                    &editor,
                    &projection,
                    &stale,
                    &descriptors,
                    Ok(&manifest),
                )
                .unwrap_err(),
            "the Inspector belongs to a stale code-project projection"
        );
        let mut ambiguous = manifest.clone();
        let radius = ambiguous
            .controls
            .iter()
            .find(|control| managed_source_path(control) == "geometry1.radius")
            .unwrap()
            .clone();
        ambiguous.controls.push(radius);
        assert_eq!(
            source
                .inspector_parameter_presentations(
                    &editor,
                    &projection,
                    &inspector,
                    &descriptors,
                    Ok(&ambiguous),
                )
                .unwrap_err(),
            "the managed Inspector property resolves to more than one source control"
        );
        let blocked = source
            .inspector_parameter_presentations(
                &editor,
                &projection,
                &inspector,
                &descriptors,
                Err("Apply or Revert draft"),
            )
            .unwrap();
        assert!(blocked.iter().all(|presentation| presentation.authority
            == InspectorParameterAuthority::Blocked {
                reason: "Apply or Revert draft".into()
            }));
    }
}
