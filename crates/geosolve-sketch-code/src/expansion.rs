// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic, equation-free lowering from managed code projects into the
//! ordinary design-intent patch vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ConicConstructionOptions, GeometryToolVariant, NurbsConstructionOptions,
    PreparedIntentOperationOutputRole, PreparedIntentOperationPathSegment,
    PreparedIntentOperationPlan, PreparedIntentOperationSourceRef, ProjectionalGeometrySamples,
    ProjectionalTangentArc, projectional_geometry_draft_from_plan,
    projectional_geometry_plan_from_samples, projectional_tangent_arc_draft,
};
use geosolve_sketch::{
    ContactAdmissibleRange, ContactNeighborhood, DocumentArcSweep, DocumentBSplineForm,
    GeometryRole, TangentOrientation,
};
use geosolve_sketch_intent::{
    AggregateKind, ComputedFeatureKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentFieldDefault, IntentFieldKey, IntentKey, IntentKeyError, IntentLiteral,
    IntentLiteralSchema, IntentNodeDraft, IntentNodeKind, IntentOperationOutputKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortKind, IntentPortRole, IntentPortSelector,
    IntentProjectionPath, IntentProjectionPathSegment, IntentSessionIdentity, IntentUnit,
    LeafField, OperationKind, PatchPortRef, intent_content_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::declaration_catalog::{
    CodeAuthoringArgumentKind, CodeAuthoringAvailability, CodeAuthoringCollectionMember,
    CodeAuthoringDeclarationDescriptor, CodeAuthoringDeclarationKind, CodeAuthoringDynamicChildren,
    CodeAuthoringInputBinding, CodeAuthoringResultPolicy, code_authoring_family,
    resolve_code_authoring_declaration,
};
use crate::managed::{CompiledManagedSource, ManagedSuppressionProjection};
use crate::{
    ArtifactValidationError, AuthoringDeclaration, CodeDraftProvenance, CodeDraftValue,
    CodeGeneratedChildAddress, CodeInteractionOverlay, CodeOverlayError, CodeOwnerAddress,
    CodeProject, CodeProjectError, CodeWritableAddress, CollectionRule, FeatureKind,
    GeneratedMemberAddress, GeneratedMemberIdentity, KeyedReconcileError, KeyedReconcileState,
    ManagedPathSegment, ManagedValue, OutputRef, PatchModuleArtifact, PatchTemplateNode,
    ProjectKey, SemanticOutputPath, SemanticSymbol, TemplateArgument, TemplateBinding, UnitLiteral,
    ValidatedPatchModuleArtifact,
};

const MAX_EXPANDED_NODES: usize = 65_536;
const MAX_EXPANDED_OUTPUTS: usize = 65_536;
const MAX_HOST_REQUESTS: usize = 65_536;
const MAX_OPERATION_PLAN_PREFIX_ALIASES: usize = 65_536;

/// A transaction-local, typed ordinary intent port. No allocated node or port
/// identity is exposed to managed/custom code.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedPort {
    pub alias: IntentKey,
    pub selector: IntentPortSelector,
    pub kind: IntentPortKind,
}

impl ExpandedPort {
    fn patch_ref(&self) -> PatchPortRef {
        PatchPortRef::Alias {
            node: self.alias.clone(),
            selector: self.selector,
        }
    }
}

/// Semantic corner information required by the existing native Fillet
/// authoring path. Branch/contact parameters are deliberately absent: the
/// host must derive and authenticate them through its ordinary authoring API.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedFeatureCorner {
    pub point: ExpandedPort,
    pub incoming: ExpandedPort,
    pub outgoing: ExpandedPort,
}

/// Honest target of a code-facing semantic result. Feature roots and dynamic
/// collections are not misrepresented as one arbitrary native port.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpandedSemanticTarget {
    Declaration {
        alias: IntentKey,
        kind: FeatureKind,
    },
    Port {
        port: ExpandedPort,
    },
    FeatureCorner {
        corner: ExpandedFeatureCorner,
    },
    Collection {
        members: BTreeMap<String, Box<Self>>,
    },
    HostOutput {
        address: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        kind: FeatureKind,
    },
}

/// One final named output of the managed program.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedSemanticOutput {
    pub reference: OutputRef,
    pub target: ExpandedSemanticTarget,
}

/// Exact source-to-generated-intent audit row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedIntentProvenance {
    pub identity: GeneratedMemberIdentity,
    pub declaration: SemanticSymbol,
    pub artifact_digest: Option<String>,
    pub target: ExpandedSemanticTarget,
}

/// The source form from which one writable point gets its base seed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum CodePointSeedSource {
    Literal,
    Reference {
        declaration: SemanticSymbol,
        path: SemanticOutputPath,
    },
    /// A generated template input which aliases another semantic point.
    /// Unlike an independent generated seed, dragging this consumer detaches
    /// only the generated owner from its producer.
    GeneratedReference,
    Generated,
}

impl CodePointSeedSource {
    #[must_use]
    pub const fn is_reference(&self) -> bool {
        matches!(self, Self::Reference { .. } | Self::GeneratedReference)
    }
}

/// Canonical rectangle corner roles used by the family-owned point codec.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeRectangleCorner {
    LowerLeft,
    LowerRight,
    UpperRight,
    UpperLeft,
}

/// How one visible point handle updates semantic draft seeds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case", deny_unknown_fields)]
pub enum CodePointEdit {
    Point {
        address: CodeWritableAddress,
    },
    RectangleCorner {
        lower_left: CodeWritableAddress,
        upper_right: CodeWritableAddress,
        corner: CodeRectangleCorner,
        effective_lower_left: [f64; 2],
        effective_upper_right: [f64; 2],
    },
}

/// Exact semantic writable provenance for one visible intent point port.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedWritablePoint {
    pub handle: ExpandedPort,
    pub source: CodePointSeedSource,
    pub edit: CodePointEdit,
}

/// Exact generated host-child provenance used by semantic selection and
/// reversible suppression. `alias` is the actual durable host declaration
/// symbol; callers never derive or decode it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpandedGeneratedChild {
    pub alias: IntentKey,
    pub declaration: SemanticSymbol,
    pub address: CodeGeneratedChildAddress,
    pub suppressed: bool,
}

/// One keyed corner passed to native Fillet authoring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyedFilletHostRequest {
    pub invocation: SemanticSymbol,
    pub member_key: Vec<String>,
    pub output: GeneratedMemberAddress,
    pub identity: GeneratedMemberIdentity,
    pub radius: UnitLiteral,
    pub corner: ExpandedFeatureCorner,
    pub artifact_digest: String,
    #[serde(default)]
    pub suppressed: bool,
}

/// Host-owned work which cannot honestly be expressed without existing
/// native feature authoring and explicit branch state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum CodeHostRequest {
    FilletAtCorner(KeyedFilletHostRequest),
    RoundedRectangleProfile {
        invocation: SemanticSymbol,
        output: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        radius: UnitLiteral,
        corners: BTreeMap<String, ExpandedFeatureCorner>,
        #[serde(default)]
        suppressed_children: BTreeSet<String>,
        artifact_digest: String,
    },
}

/// Fully validated structural lowering product. The caller plans `patch`
/// through its ordinary `IntentSession`, then resolves host requests through
/// existing native authoring before publishing accepted scene authority.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpandedCodeProject {
    pub patch: IntentPatch,
    pub semantic_outputs: BTreeMap<String, ExpandedSemanticOutput>,
    pub generated_provenance: BTreeMap<GeneratedMemberAddress, GeneratedIntentProvenance>,
    /// Every emitted code-owned alias mapped to its managed declaration.
    /// Consumers must use this map rather than decode implementation aliases.
    pub declaration_provenance: BTreeMap<IntentKey, SemanticSymbol>,
    /// Stable point edit lenses keyed by visible semantic port.
    pub writable_points: Vec<ExpandedWritablePoint>,
    /// Generation-authenticated host children which may be suppressed without
    /// deleting their owning managed invocation.
    pub generated_children: Vec<ExpandedGeneratedChild>,
    pub host_requests: Vec<CodeHostRequest>,
    /// Source-relative native operation plans used to reconstruct clean
    /// semantic result paths. Restored composition authenticates every row
    /// again against the exact operation prefix before exposing references.
    pub operation_plans: Vec<PreparedCodeOperationPlan>,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedCodeOperationPlan {
    pub symbol: IntentKey,
    /// Exact source-order create-node prefix supplied to native planning,
    /// including this operation as its final alias. `IntentPatch` array order
    /// is canonical and therefore cannot reconstruct this authority.
    pub prefix_aliases: Vec<IntentKey>,
    pub plan: PreparedIntentOperationPlan,
}

impl ExpandedCodeProject {
    /// Resolves one emitted intent alias to its owning managed declaration.
    #[must_use]
    pub fn declaration_for_alias(&self, alias: &IntentKey) -> Option<&SemanticSymbol> {
        self.declaration_provenance.get(alias)
    }

    /// Returns every semantic edit lens attached to one point port. More than
    /// one row is deliberate when a producer and a dependent consumer share
    /// the same native point before the consumer is detached.
    pub fn writable_points_for_port(
        &self,
        alias: &IntentKey,
        selector: IntentPortSelector,
    ) -> impl Iterator<Item = &ExpandedWritablePoint> {
        self.writable_points
            .iter()
            .filter(move |point| point.handle.alias == *alias && point.handle.selector == selector)
    }

    /// Resolves an exact generated host declaration to its semantic child.
    #[must_use]
    pub fn generated_child_for_alias(&self, alias: &IntentKey) -> Option<&ExpandedGeneratedChild> {
        self.generated_children
            .iter()
            .find(|child| child.alias == *alias)
    }

    /// Prunes a prior overlay to the writable owners present in this exact
    /// structural expansion. Explicit structural publications use this after
    /// an overlay-free preflight; arbitrary stale persisted overlays continue
    /// to fail closed in ordinary expansion.
    #[must_use]
    pub fn retained_overlay(&self, current: &CodeInteractionOverlay) -> CodeInteractionOverlay {
        let writable = self
            .writable_points
            .iter()
            .flat_map(|point| {
                let provenance = point.draft_provenance();
                writable_addresses(point)
                    .into_iter()
                    .cloned()
                    .map(move |address| (address, provenance))
            })
            .collect::<BTreeSet<_>>();
        let children = self
            .generated_children
            .iter()
            .map(|child| child.address.clone())
            .collect::<BTreeSet<_>>();
        let mut retained = current.clone();
        retained.retain(|address, draft| writable.contains(&(address.clone(), draft.provenance)));
        retained.retain_generated_children(|address, _| children.contains(address));
        retained
    }
}

impl ExpandedWritablePoint {
    /// Returns a staged overlay containing one complete terminal point edit.
    /// A referenced consumer is detached locally; its producer is unchanged.
    ///
    /// # Errors
    ///
    /// Returns a finite-value, address, resource-limit, or same-tier conflict
    /// error without mutating `current`.
    pub fn stage_drag(
        &self,
        current: &CodeInteractionOverlay,
        target: [f64; 2],
    ) -> Result<CodeInteractionOverlay, CodeOverlayError> {
        stage_point_drags(current, [(self, target)])
    }

    #[must_use]
    pub fn draft_provenance(&self) -> CodeDraftProvenance {
        match self.source {
            CodePointSeedSource::Reference { .. } | CodePointSeedSource::GeneratedReference => {
                CodeDraftProvenance::DetachedReference
            }
            CodePointSeedSource::Generated => CodeDraftProvenance::GeneratedOverride,
            CodePointSeedSource::Literal => match &self.edit {
                CodePointEdit::Point { address } => match address.owner.address {
                    CodeOwnerAddress::DirectDeclaration { .. } => CodeDraftProvenance::CanvasDrag,
                    CodeOwnerAddress::GeneratedMember { .. } => {
                        CodeDraftProvenance::GeneratedOverride
                    }
                },
                CodePointEdit::RectangleCorner { lower_left, .. } => {
                    match lower_left.owner.address {
                        CodeOwnerAddress::DirectDeclaration { .. } => {
                            CodeDraftProvenance::CanvasDrag
                        }
                        CodeOwnerAddress::GeneratedMember { .. } => {
                            CodeDraftProvenance::GeneratedOverride
                        }
                    }
                }
            },
        }
    }
}

/// Stages one complete terminal drag bundle atomically. This is the sole
/// generalized same-tier conflict boundary for coupled semantic point edits.
///
/// # Errors
///
/// Returns a finite-value, address, resource-limit, or same-tier component
/// conflict error without mutating `current`.
pub fn stage_point_drags<'a>(
    current: &CodeInteractionOverlay,
    drags: impl IntoIterator<Item = (&'a ExpandedWritablePoint, [f64; 2])>,
) -> Result<CodeInteractionOverlay, CodeOverlayError> {
    let mut bases = BTreeMap::<CodeWritableAddress, ([f64; 2], CodeDraftProvenance)>::new();
    let mut components =
        BTreeMap::<(CodeWritableAddress, CartesianComponent), (f64, CodeDraftProvenance)>::new();
    for (point, target) in drags {
        let provenance = point.draft_provenance();
        for (address, base) in point.edit.point_bases(target) {
            if let Some((previous, previous_provenance)) = bases.get_mut(&address) {
                if point_seed_bits(*previous) != point_seed_bits(base) {
                    return Err(CodeOverlayError::ConflictingDraft(address.display_path()));
                }
                *previous_provenance = canonical_point_provenance(*previous_provenance, provenance);
            } else {
                bases.insert(address, (base, provenance));
            }
        }
        for update in point.edit.component_updates(target) {
            let key = (update.address.clone(), update.component);
            if let Some((previous, previous_provenance)) = components.get_mut(&key) {
                if previous.to_bits() != update.value.to_bits() {
                    return Err(CodeOverlayError::ConflictingDraft(
                        update.address.display_path(),
                    ));
                }
                *previous_provenance = canonical_point_provenance(*previous_provenance, provenance);
            } else {
                components.insert(key, (update.value, provenance));
            }
        }
    }
    let mut updates = bases;
    for ((address, component), (value, provenance)) in components {
        let Some(entry) = updates.get_mut(&address) else {
            return Err(CodeOverlayError::ConflictingDraft(address.display_path()));
        };
        match component {
            CartesianComponent::X => entry.0[0] = value,
            CartesianComponent::Y => entry.0[1] = value,
        }
        entry.1 = canonical_point_provenance(entry.1, provenance);
    }
    let mut staged = current.clone();
    staged.set_point_drafts_atomically(
        updates
            .into_iter()
            .map(|(address, (value, provenance))| (address, value, provenance)),
    )?;
    Ok(staged)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CartesianComponent {
    X,
    Y,
}

#[derive(Clone, Debug)]
struct PointComponentUpdate {
    address: CodeWritableAddress,
    component: CartesianComponent,
    value: f64,
}

fn canonical_point_provenance(
    left: CodeDraftProvenance,
    right: CodeDraftProvenance,
) -> CodeDraftProvenance {
    // Provenance is audit state rather than constraint priority. Identical
    // component writes use one stable label independent of event ordering.
    left.max(right)
}

impl CodePointEdit {
    /// Expands one dragged visible position into canonical semantic point
    /// drafts. Rectangle side/corner coupling is owned here, not in the web
    /// adapter.
    #[must_use]
    pub fn point_updates(&self, target: [f64; 2]) -> Vec<(CodeWritableAddress, [f64; 2])> {
        match self {
            Self::Point { address } => vec![(address.clone(), target)],
            Self::RectangleCorner {
                lower_left,
                upper_right,
                corner,
                effective_lower_left,
                effective_upper_right,
            } => {
                let mut next_lower = *effective_lower_left;
                let mut next_upper = *effective_upper_right;
                match corner {
                    CodeRectangleCorner::LowerLeft => next_lower = target,
                    CodeRectangleCorner::LowerRight => {
                        next_upper[0] = target[0];
                        next_lower[1] = target[1];
                    }
                    CodeRectangleCorner::UpperRight => next_upper = target,
                    CodeRectangleCorner::UpperLeft => {
                        next_lower[0] = target[0];
                        next_upper[1] = target[1];
                    }
                }
                vec![
                    (lower_left.clone(), next_lower),
                    (upper_right.clone(), next_upper),
                ]
            }
        }
    }

    fn component_updates(&self, target: [f64; 2]) -> Vec<PointComponentUpdate> {
        let components = |address: &CodeWritableAddress, values: &[(CartesianComponent, f64)]| {
            values
                .iter()
                .map(|(component, value)| PointComponentUpdate {
                    address: address.clone(),
                    component: *component,
                    value: *value,
                })
                .collect::<Vec<_>>()
        };
        match self {
            Self::Point { address } => components(
                address,
                &[
                    (CartesianComponent::X, target[0]),
                    (CartesianComponent::Y, target[1]),
                ],
            ),
            Self::RectangleCorner {
                lower_left,
                upper_right,
                corner,
                ..
            } => match corner {
                CodeRectangleCorner::LowerLeft => components(
                    lower_left,
                    &[
                        (CartesianComponent::X, target[0]),
                        (CartesianComponent::Y, target[1]),
                    ],
                ),
                CodeRectangleCorner::LowerRight => {
                    components(upper_right, &[(CartesianComponent::X, target[0])])
                        .into_iter()
                        .chain(components(
                            lower_left,
                            &[(CartesianComponent::Y, target[1])],
                        ))
                        .collect()
                }
                CodeRectangleCorner::UpperRight => components(
                    upper_right,
                    &[
                        (CartesianComponent::X, target[0]),
                        (CartesianComponent::Y, target[1]),
                    ],
                ),
                CodeRectangleCorner::UpperLeft => {
                    components(lower_left, &[(CartesianComponent::X, target[0])])
                        .into_iter()
                        .chain(components(
                            upper_right,
                            &[(CartesianComponent::Y, target[1])],
                        ))
                        .collect()
                }
            },
        }
    }

    fn point_bases(&self, target: [f64; 2]) -> Vec<(CodeWritableAddress, [f64; 2])> {
        match self {
            Self::Point { address } => vec![(address.clone(), target)],
            Self::RectangleCorner {
                lower_left,
                upper_right,
                effective_lower_left,
                effective_upper_right,
                ..
            } => vec![
                (lower_left.clone(), *effective_lower_left),
                (upper_right.clone(), *effective_upper_right),
            ],
        }
    }

    /// Exact draft-address bundle owned by one visible edit lens.
    #[must_use]
    pub fn writable_addresses(&self) -> Vec<CodeWritableAddress> {
        match self {
            Self::Point { address } => vec![address.clone()],
            Self::RectangleCorner {
                lower_left,
                upper_right,
                ..
            } => vec![lower_left.clone(), upper_right.clone()],
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpandedCodeProjectWire {
    patch: IntentPatch,
    semantic_outputs: BTreeMap<String, ExpandedSemanticOutput>,
    generated_provenance: Vec<(GeneratedMemberAddress, GeneratedIntentProvenance)>,
    declaration_provenance: Vec<(IntentKey, SemanticSymbol)>,
    writable_points: Vec<ExpandedWritablePoint>,
    #[serde(default)]
    generated_children: Vec<ExpandedGeneratedChild>,
    host_requests: Vec<CodeHostRequest>,
    #[serde(default)]
    operation_plans: Vec<PreparedCodeOperationPlan>,
    digest: String,
}

impl Serialize for ExpandedCodeProject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ExpandedCodeProjectWire {
            patch: self.patch.clone(),
            semantic_outputs: self.semantic_outputs.clone(),
            generated_provenance: self
                .generated_provenance
                .iter()
                .map(|(address, provenance)| (address.clone(), provenance.clone()))
                .collect(),
            declaration_provenance: self
                .declaration_provenance
                .iter()
                .map(|(alias, declaration)| (alias.clone(), declaration.clone()))
                .collect(),
            writable_points: self.writable_points.clone(),
            generated_children: self.generated_children.clone(),
            host_requests: self.host_requests.clone(),
            operation_plans: self.operation_plans.clone(),
            digest: self.digest.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExpandedCodeProject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ExpandedCodeProjectWire::deserialize(deserializer)?;
        let provenance_len = wire.generated_provenance.len();
        let generated_provenance: BTreeMap<GeneratedMemberAddress, GeneratedIntentProvenance> =
            wire.generated_provenance.into_iter().collect();
        if generated_provenance.len() != provenance_len {
            return Err(serde::de::Error::custom(
                "duplicate generated expansion provenance address",
            ));
        }
        let declaration_len = wire.declaration_provenance.len();
        let declaration_provenance: BTreeMap<IntentKey, SemanticSymbol> =
            wire.declaration_provenance.into_iter().collect();
        if declaration_provenance.len() != declaration_len {
            return Err(serde::de::Error::custom(
                "duplicate declaration expansion provenance alias",
            ));
        }
        Ok(Self {
            patch: wire.patch,
            semantic_outputs: wire.semantic_outputs,
            generated_provenance,
            declaration_provenance,
            writable_points: wire.writable_points,
            generated_children: wire.generated_children,
            host_requests: wire.host_requests,
            operation_plans: wire.operation_plans,
            digest: wire.digest,
        })
    }
}

/// Fail-closed managed/artifact/reconciliation expansion diagnostic.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CodeExpansionError {
    #[error(transparent)]
    Project(#[from] CodeProjectError),
    #[error(transparent)]
    Artifact(#[from] ArtifactValidationError),
    #[error(transparent)]
    Reconciliation(#[from] KeyedReconcileError),
    #[error(transparent)]
    Key(#[from] IntentKeyError),
    #[error("managed declaration `{declaration}` is invalid: {message}")]
    InvalidDeclaration {
        declaration: String,
        message: String,
    },
    #[error("patch binding `{binding}` has no exact imported artifact")]
    MissingArtifact { binding: String },
    #[error("patch binding `{binding}` resolves to more than one artifact")]
    AmbiguousArtifact { binding: String },
    #[error("semantic reference `{reference}` cannot be resolved")]
    UnresolvedReference { reference: String },
    #[error("semantic reference `{reference}` is {actual:?}, not {expected:?}")]
    KindMismatch {
        reference: String,
        expected: FeatureKind,
        actual: FeatureKind,
    },
    #[error("unsupported code lowering: {0}")]
    Unsupported(String),
    #[error("generated address `{0}` was produced more than once")]
    DuplicateGeneratedAddress(String),
    #[error("keyed reconciliation does not exactly match expansion: {0}")]
    ReconciliationMismatch(String),
    #[error("generated override `{address}` is not supported by an exact edit lens")]
    UnsupportedOverride { address: String },
    #[error("generated override `{address}` is not a finite two-coordinate point")]
    InvalidPointOverride { address: String },
    #[error(transparent)]
    Overlay(#[from] CodeOverlayError),
    #[error("UX draft `{address}` does not name an active writable seed")]
    UnknownDraft { address: String },
    #[error("UX draft `{address}` has a stale owner generation")]
    StaleDraftGeneration { address: String },
    #[error("UX draft `{address}` is not a point placement")]
    DraftTypeMismatch { address: String },
    #[error("conflicting UX drafts `{first}` and `{second}` target one instance seed")]
    ConflictingDrafts { first: String, second: String },
    #[error("expansion resource `{resource}` is {actual}; the limit is {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("expanded result could not be encoded: {0}")]
    Encoding(String),
    #[error("native operation planning for `{declaration}` failed: {message}")]
    OperationPlanning {
        declaration: String,
        message: String,
    },
}

/// Internal native-authority seam for topology-dependent operation results.
///
/// Pure source expansion deliberately has no implementation which guesses an
/// operation's result inventory. Cold composition supplies the exact document
/// and model-scale authority through this seam instead.
pub(crate) trait CodeOperationPlanner {
    fn prepare(
        &mut self,
        prefix: &[IntentPatchOperation],
        alias: &IntentKey,
        provisional: &IntentNodeDraft,
    ) -> Result<PreparedIntentOperationPlan, CodeExpansionError>;
}

struct UnavailableOperationPlanner;

impl CodeOperationPlanner for UnavailableOperationPlanner {
    fn prepare(
        &mut self,
        _prefix: &[IntentPatchOperation],
        _alias: &IntentKey,
        provisional: &IntentNodeDraft,
    ) -> Result<PreparedIntentOperationPlan, CodeExpansionError> {
        Err(CodeExpansionError::OperationPlanning {
            declaration: provisional.symbol.to_string(),
            message: "an authenticated native document and model scale are required".into(),
        })
    }
}

struct RetainedOperationPlanner<'a> {
    plans: &'a [PreparedCodeOperationPlan],
    next: usize,
}

impl CodeOperationPlanner for RetainedOperationPlanner<'_> {
    fn prepare(
        &mut self,
        prefix: &[IntentPatchOperation],
        alias: &IntentKey,
        provisional: &IntentNodeDraft,
    ) -> Result<PreparedIntentOperationPlan, CodeExpansionError> {
        let retained =
            self.plans
                .get(self.next)
                .ok_or_else(|| CodeExpansionError::OperationPlanning {
                    declaration: provisional.symbol.to_string(),
                    message: "canonical replay is missing an authenticated operation plan".into(),
                })?;
        if retained.symbol != provisional.symbol {
            return Err(CodeExpansionError::OperationPlanning {
                declaration: provisional.symbol.to_string(),
                message: format!(
                    "canonical replay expected operation `{}`, not `{}`",
                    retained.symbol, provisional.symbol
                ),
            });
        }
        let IntentNodeKind::Operation { operation } = provisional.kind else {
            return Err(CodeExpansionError::OperationPlanning {
                declaration: provisional.symbol.to_string(),
                message: "canonical replay target is not an operation".into(),
            });
        };
        if retained.plan.operation != operation {
            return Err(CodeExpansionError::OperationPlanning {
                declaration: provisional.symbol.to_string(),
                message: "canonical replay operation kind disagrees with source".into(),
            });
        }
        let mut prefix_aliases = prefix
            .iter()
            .map(|operation| match operation {
                IntentPatchOperation::CreateNode { alias, .. } => Ok(alias.clone()),
                _ => Err(CodeExpansionError::OperationPlanning {
                    declaration: provisional.symbol.to_string(),
                    message: "canonical replay planning prefix contains a non-create operation"
                        .into(),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        prefix_aliases.push(alias.clone());
        if retained.prefix_aliases != prefix_aliases {
            return Err(CodeExpansionError::OperationPlanning {
                declaration: provisional.symbol.to_string(),
                message: "canonical replay planning prefix disagrees with source order".into(),
            });
        }
        self.next += 1;
        Ok(retained.plan.clone())
    }
}

impl RetainedOperationPlanner<'_> {
    fn finish(self) -> Result<(), CodeExpansionError> {
        if let Some(unconsumed) = self.plans.get(self.next) {
            return Err(CodeExpansionError::OperationPlanning {
                declaration: unconsumed.symbol.to_string(),
                message: "canonical replay contains an unexpected authenticated operation plan"
                    .into(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SemanticDeclaration {
    root: SemanticValue,
    paths: BTreeMap<SemanticOutputPath, SemanticValue>,
}

type CollectionPlan = BTreeMap<Vec<String>, Vec<(String, SemanticValue)>>;
type TemplateOutputKey = (Vec<String>, Vec<String>, SemanticOutputPath);
type GeneratedAddressPlan = BTreeMap<TemplateOutputKey, GeneratedMemberAddress>;
type TemplateOutputPlan = BTreeMap<TemplateOutputKey, SemanticValue>;
type PlannedTemplateOutput = (
    SemanticOutputPath,
    FeatureKind,
    GeneratedMemberAddress,
    GeneratedMemberIdentity,
);

#[derive(Clone, Debug)]
struct GeneratedTemplateLowering {
    declaration: SemanticSymbol,
    invocation: SemanticSymbol,
    outputs: Vec<PlannedTemplateOutput>,
}
type SemanticPathMap = BTreeMap<SemanticOutputPath, SemanticValue>;
type SemanticMemberMap = BTreeMap<String, SemanticValue>;
type InvocationCollectionPublication = (SemanticPathMap, SemanticMemberMap);

#[derive(Clone, Debug)]
struct ResolvedTemplateBindings {
    arguments: ManagedValue,
    direct: BTreeMap<String, SemanticValue>,
    temporary: Vec<SemanticSymbol>,
}

#[derive(Clone, Debug)]
enum SemanticValue {
    Declaration {
        alias: IntentKey,
        kind: FeatureKind,
    },
    Port(ExpandedPort),
    Corner(ExpandedFeatureCorner),
    Feature {
        alias: IntentKey,
        members: BTreeMap<String, SemanticValue>,
    },
    Collection(BTreeMap<String, SemanticValue>),
    HostOutput {
        address: GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        kind: FeatureKind,
    },
    PointLiteral([f64; 2]),
    ScalarLiteral(UnitLiteral),
}

impl SemanticValue {
    fn kind(&self) -> FeatureKind {
        match self {
            Self::Declaration { kind, .. } | Self::HostOutput { kind, .. } => *kind,
            Self::Port(port) => feature_kind_for_port(port.kind),
            Self::Corner(_) => FeatureKind::FeatureCorner,
            Self::Feature { .. } => FeatureKind::Feature,
            Self::Collection(_) => FeatureKind::Collection,
            Self::PointLiteral(_) => FeatureKind::Point,
            Self::ScalarLiteral(_) => FeatureKind::Scalar,
        }
    }

    fn public_target(&self) -> ExpandedSemanticTarget {
        match self {
            Self::Declaration { alias, kind } => ExpandedSemanticTarget::Declaration {
                alias: alias.clone(),
                kind: *kind,
            },
            Self::Port(port) => ExpandedSemanticTarget::Port { port: port.clone() },
            Self::Corner(corner) => ExpandedSemanticTarget::FeatureCorner {
                corner: corner.clone(),
            },
            Self::Feature { alias, .. } => ExpandedSemanticTarget::Declaration {
                alias: alias.clone(),
                kind: FeatureKind::Feature,
            },
            Self::Collection(members) => ExpandedSemanticTarget::Collection {
                members: members
                    .iter()
                    .map(|(key, value)| (key.clone(), Box::new(value.public_target())))
                    .collect(),
            },
            Self::HostOutput {
                address,
                identity,
                kind,
            } => ExpandedSemanticTarget::HostOutput {
                address: address.clone(),
                identity: *identity,
                kind: *kind,
            },
            Self::PointLiteral(_) | Self::ScalarLiteral(_) => {
                unreachable!("literal helpers never become public outputs")
            }
        }
    }

    fn as_port(
        &self,
        expected: IntentPortKind,
        reference: &str,
    ) -> Result<ExpandedPort, CodeExpansionError> {
        let candidate = match self {
            Self::Port(port) => Some(port),
            Self::Corner(corner) if expected == IntentPortKind::Point => Some(&corner.point),
            _ => None,
        };
        let Some(port) = candidate else {
            return Err(CodeExpansionError::KindMismatch {
                reference: reference.to_owned(),
                expected: feature_kind_for_port(expected),
                actual: self.kind(),
            });
        };
        if port.kind != expected {
            return Err(CodeExpansionError::KindMismatch {
                reference: reference.to_owned(),
                expected: feature_kind_for_port(expected),
                actual: feature_kind_for_port(port.kind),
            });
        }
        Ok(port.clone())
    }

    fn as_corner(&self, reference: &str) -> Result<ExpandedFeatureCorner, CodeExpansionError> {
        match self {
            Self::Corner(corner) => Ok(corner.clone()),
            _ => Err(CodeExpansionError::KindMismatch {
                reference: reference.to_owned(),
                expected: FeatureKind::FeatureCorner,
                actual: self.kind(),
            }),
        }
    }
}

#[derive(Clone)]
struct PinnedArtifact {
    digest: String,
    artifact: ValidatedPatchModuleArtifact,
}

#[derive(Clone)]
struct InvocationPlan {
    declaration: AuthoringDeclaration,
    pinned: PinnedArtifact,
    collections: CollectionPlan,
    addresses: GeneratedAddressPlan,
    synthetic_outputs: TemplateOutputPlan,
}

struct ExpansionBuilder {
    project: ProjectKey,
    source_suppressions: ManagedSuppressionProjection,
    operations: Vec<IntentPatchOperation>,
    declarations: BTreeMap<SemanticSymbol, SemanticDeclaration>,
    provenance: BTreeMap<GeneratedMemberAddress, GeneratedIntentProvenance>,
    declaration_provenance: BTreeMap<IntentKey, SemanticSymbol>,
    writable_points: Vec<ExpandedWritablePoint>,
    generated_children: Vec<ExpandedGeneratedChild>,
    host_requests: Vec<CodeHostRequest>,
    operation_plans: Vec<PreparedCodeOperationPlan>,
    operation_plan_prefix_aliases: usize,
    point_seeds: BTreeMap<(IntentKey, IntentPortSelector), [f64; 2]>,
    generated_template: Option<GeneratedTemplateLowering>,
}

#[derive(Clone, Debug)]
struct KeyedPolylineDefinition {
    vertices: Vec<(String, ManagedValue)>,
    closed: bool,
    representation: DirectPolylineRepresentation,
    role: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectPolylineRepresentation {
    /// Historical managed Polyline lowering: one point and line declaration
    /// per keyed member plus an aggregate root.
    Composite,
    /// Canvas-authored Polyline lowering: one native Polyline recipe node
    /// whose child ports retain the same keyed public result surface.
    SingleCurve,
}

impl ExpansionBuilder {
    fn new(project: ProjectKey, source_suppressions: ManagedSuppressionProjection) -> Self {
        Self {
            project,
            source_suppressions,
            operations: Vec::new(),
            declarations: BTreeMap::new(),
            provenance: BTreeMap::new(),
            declaration_provenance: BTreeMap::new(),
            writable_points: Vec::new(),
            generated_children: Vec::new(),
            host_requests: Vec::new(),
            operation_plans: Vec::new(),
            operation_plan_prefix_aliases: 0,
            point_seeds: BTreeMap::new(),
            generated_template: None,
        }
    }

    fn lowering_alias(
        &self,
        scope: &str,
        declaration: &SemanticSymbol,
        suffix: &[&str],
    ) -> Result<IntentKey, CodeExpansionError> {
        if let Some(context) = self
            .generated_template
            .as_ref()
            .filter(|context| &context.declaration == declaration)
        {
            let (_, _, address, identity) = context.outputs.first().ok_or_else(|| {
                CodeExpansionError::Unsupported(format!(
                    "generated declaration `{}` has no planned output owner",
                    declaration.0
                ))
            })?;
            return generated_scoped_alias(address, *identity, scope, suffix);
        }
        semantic_alias(scope, &self.project, declaration, suffix)
    }

    fn lowering_output_owner(
        &self,
        declaration: &SemanticSymbol,
        output: &SemanticOutputPath,
    ) -> Option<(&GeneratedMemberAddress, GeneratedMemberIdentity)> {
        self.generated_template
            .as_ref()
            .filter(|context| &context.declaration == declaration)?
            .outputs
            .iter()
            .find(|(path, _, _, _)| path == output)
            .map(|(_, _, address, identity)| (address, *identity))
    }

    fn lowering_point_address(
        &self,
        declaration: &AuthoringDeclaration,
        direct_family: &str,
        reconciliation: Option<&KeyedReconcileState>,
        output: SemanticOutputPath,
    ) -> Option<CodeWritableAddress> {
        if let Some((address, identity)) = self.lowering_output_owner(&declaration.symbol, &output)
        {
            return Some(CodeWritableAddress::generated_point(
                self.project.clone(),
                address.clone(),
                identity,
                output,
            ));
        }
        let identity = direct_owner_identity(declaration, direct_family, reconciliation)?;
        Some(direct_point_address(
            &self.project,
            &declaration.symbol,
            identity,
            output,
        ))
    }

    fn push_node(
        &mut self,
        declaration: &SemanticSymbol,
        alias: IntentKey,
        mut draft: IntentNodeDraft,
    ) -> Result<(), CodeExpansionError> {
        if self.operations.len() >= MAX_EXPANDED_NODES {
            return Err(CodeExpansionError::ResourceLimit {
                resource: "intent nodes",
                actual: self.operations.len().saturating_add(1),
                limit: MAX_EXPANDED_NODES,
            });
        }
        let owner = self
            .generated_template
            .as_ref()
            .filter(|context| &context.declaration == declaration)
            .map_or_else(|| declaration.clone(), |context| context.invocation.clone());
        if let Some(previous) = self
            .declaration_provenance
            .insert(alias.clone(), owner.clone())
        {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: declaration.0.clone(),
                message: format!(
                    "expanded alias is already owned by declaration `{}`",
                    previous.0
                ),
            });
        }
        if self.source_suppressions.declarations.contains(&owner) {
            draft.suppressed = true;
        }
        self.operations.push(IntentPatchOperation::CreateNode {
            alias,
            draft: Box::new(draft),
            cell: None,
        });
        Ok(())
    }

    fn suppresses_declaration(&self, declaration: &SemanticSymbol) -> bool {
        self.source_suppressions.declarations.contains(declaration)
    }

    fn suppresses_generated_member(&self, address: &GeneratedMemberAddress) -> bool {
        self.source_suppressions.generated_members.contains(address)
    }

    fn suppress_lowered_member_since(&mut self, operation_start: usize, host_request_start: usize) {
        for operation in &mut self.operations[operation_start..] {
            if let IntentPatchOperation::CreateNode { draft, .. } = operation {
                draft.suppressed = true;
            }
        }
        for request in &mut self.host_requests[host_request_start..] {
            suppress_host_request(request);
        }
    }

    fn add_writable_point(&mut self, point: ExpandedWritablePoint) {
        self.writable_points.push(point);
    }

    #[allow(
        clippy::float_cmp,
        reason = "duplicate semantic point seeds must match their exact persisted IEEE values"
    )]
    fn add_point_seed(
        &mut self,
        port: &ExpandedPort,
        position: [f64; 2],
    ) -> Result<(), CodeExpansionError> {
        let key = (port.alias.clone(), port.selector);
        if let Some(previous) = self.point_seeds.insert(key, position)
            && previous != position
        {
            return Err(CodeExpansionError::Unsupported(format!(
                "point seed for `{}:{:?}` was emitted more than once",
                port.alias, port.selector
            )));
        }
        Ok(())
    }

    fn point_seed(&self, value: &SemanticValue) -> Option<[f64; 2]> {
        let port = match value {
            SemanticValue::PointLiteral(position) => return Some(*position),
            SemanticValue::Port(port) => port,
            SemanticValue::Corner(corner) => &corner.point,
            _ => return None,
        };
        self.point_seeds
            .get(&(port.alias.clone(), port.selector))
            .copied()
    }

    fn insert_declaration(
        &mut self,
        symbol: SemanticSymbol,
        declaration: SemanticDeclaration,
    ) -> Result<(), CodeExpansionError> {
        if self
            .declarations
            .insert(symbol.clone(), declaration)
            .is_some()
        {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: symbol.0,
                message: "duplicate lowered declaration".into(),
            });
        }
        Ok(())
    }

    fn resolve(
        &self,
        declaration: &SemanticSymbol,
        path: &SemanticOutputPath,
    ) -> Result<SemanticValue, CodeExpansionError> {
        let Some(value) = self.declarations.get(declaration) else {
            return Err(CodeExpansionError::UnresolvedReference {
                reference: reference_text(declaration, path),
            });
        };
        if path.0.is_empty() {
            return Ok(value.root.clone());
        }
        if let Some(value) = value.paths.get(path) {
            return Ok(value.clone());
        }
        // JavaScript property access records an ordinary string segment even
        // when the native result catalog later authenticates that coordinate
        // as a keyed member. Resolve that spelling only when it identifies one
        // unique structured path; a real Field/Member collision remains
        // ambiguous and fails closed.
        let mut equivalent = value
            .paths
            .iter()
            .filter(|(candidate, _)| semantic_reference_paths_equivalent(candidate, path));
        if let Some((_, resolved)) = equivalent.next()
            && equivalent.next().is_none()
        {
            return Ok(resolved.clone());
        }
        // Every public shorthand is emitted from compiler-recorded exact
        // output provenance. Never infer one from canonical map order.
        // `mapRecord`/`each` collections retain their exact TypeScript return
        // field even when the invocation root itself is a collection. The
        // flattened root remains an internal keyed view, never a substitute
        // for the public `result.fillets[key]` path.
        if let Some((ManagedPathSegment::Field(field), suffix)) = path.0.split_first()
            && let Some(SemanticValue::Collection(collection)) =
                value.paths.get(&fields_path(&[field.as_str()]))
        {
            return resolve_value_path(&SemanticValue::Collection(collection.clone()), suffix)
                .ok_or_else(|| CodeExpansionError::UnresolvedReference {
                    reference: reference_text(declaration, path),
                });
        }
        resolve_collection_path(&value.root, &path.0).ok_or_else(|| {
            CodeExpansionError::UnresolvedReference {
                reference: reference_text(declaration, path),
            }
        })
    }

    fn resolve_managed(
        &self,
        value: &ManagedValue,
        path: &SemanticOutputPath,
        label: &str,
    ) -> Result<SemanticValue, CodeExpansionError> {
        match value {
            ManagedValue::Reference {
                declaration,
                path: base,
            } => self.resolve(declaration, &join_paths(base, path)),
            ManagedValue::Array(values) if path.0.is_empty() => Ok(SemanticValue::PointLiteral(
                point_from_array(values, label)?,
            )),
            ManagedValue::Unit(value) if path.0.is_empty() => {
                Ok(SemanticValue::ScalarLiteral(value.clone()))
            }
            ManagedValue::Number(value) if path.0.is_empty() => {
                Ok(SemanticValue::ScalarLiteral(UnitLiteral {
                    unit: "model".into(),
                    value: *value,
                }))
            }
            _ => Err(CodeExpansionError::UnresolvedReference {
                reference: label.to_owned(),
            }),
        }
    }

    fn add_provenance(
        &mut self,
        address: &GeneratedMemberAddress,
        identity: GeneratedMemberIdentity,
        declaration: &SemanticSymbol,
        artifact_digest: Option<String>,
        value: &SemanticValue,
    ) -> Result<(), CodeExpansionError> {
        let target = value.public_target();
        let row = GeneratedIntentProvenance {
            identity,
            declaration: declaration.clone(),
            artifact_digest,
            target,
        };
        if self.provenance.insert(address.clone(), row).is_some() {
            return Err(CodeExpansionError::DuplicateGeneratedAddress(
                address.display_path(),
            ));
        }
        Ok(())
    }
}

fn semantic_reference_paths_equivalent(
    left: &SemanticOutputPath,
    right: &SemanticOutputPath,
) -> bool {
    left.0.len() == right.0.len()
        && left
            .0
            .iter()
            .zip(&right.0)
            .all(|(left, right)| match (left, right) {
                (ManagedPathSegment::Index(left), ManagedPathSegment::Index(right)) => {
                    left == right
                }
                (
                    ManagedPathSegment::Field(left) | ManagedPathSegment::Member { member: left },
                    ManagedPathSegment::Field(right) | ManagedPathSegment::Member { member: right },
                ) => left == right,
                _ => false,
            })
}

/// Computes the exact keyed member set which must be reconciled before
/// expansion. This performs the same bounded source/artifact analysis as the
/// lowering path and publishes no patch.
///
/// # Errors
///
/// Returns a project, artifact, reference, kind, duplicate or resource error.
pub fn required_generated_members(
    project: &CodeProject,
) -> Result<Vec<GeneratedMemberAddress>, CodeExpansionError> {
    let mut addresses = direct_generated_members(project)?;
    if project
        .managed
        .program
        .declarations
        .iter()
        .all(|declaration| declaration.patch.is_none())
    {
        return Ok(addresses);
    }
    let (builder, plans) = prepare(
        project,
        None,
        &CodeInteractionOverlay::empty(),
        &mut UnavailableOperationPlanner,
    )?;
    for plan in plans {
        addresses.extend(plan.addresses.into_values());
    }
    if addresses.len() > MAX_EXPANDED_OUTPUTS {
        return Err(CodeExpansionError::ResourceLimit {
            resource: "generated outputs",
            actual: addresses.len(),
            limit: MAX_EXPANDED_OUTPUTS,
        });
    }
    let mut seen = BTreeSet::new();
    for address in &addresses {
        if !seen.insert(address.clone()) {
            return Err(CodeExpansionError::DuplicateGeneratedAddress(
                address.display_path(),
            ));
        }
    }
    drop(builder);
    Ok(addresses)
}

fn direct_generated_members(
    project: &CodeProject,
) -> Result<Vec<GeneratedMemberAddress>, CodeExpansionError> {
    let mut addresses = Vec::new();
    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_some() {
            continue;
        }
        let Some((family, namespace, method)) = named_family_parts(declaration) else {
            return Err(CodeExpansionError::Unsupported(format!(
                "unknown managed declaration family `{}`",
                declaration.builder_path.join(".")
            )));
        };
        match family.declaration {
            CodeAuthoringDeclarationKind::Geometry(GeometryRecipeKind::Polyline) => {}
            CodeAuthoringDeclarationKind::Constraint(_)
            | CodeAuthoringDeclarationKind::Dimension(_)
            | CodeAuthoringDeclarationKind::Operation(_)
            | CodeAuthoringDeclarationKind::Aggregate(_) => continue,
            CodeAuthoringDeclarationKind::Geometry(recipe) => {
                let owner_family = match recipe {
                    GeometryRecipeKind::Segment => "line",
                    GeometryRecipeKind::CenterRadiusCircle => "circle",
                    _ => method,
                };
                addresses.push(direct_declaration_owner_address(
                    &declaration.symbol,
                    owner_family,
                ));
                continue;
            }
            CodeAuthoringDeclarationKind::ComputedFeature(ComputedFeatureKind::FilletSet) => {
                for key in direct_fillet_set_keys(declaration)? {
                    addresses.extend(["arc", "corner"].map(|output| {
                        direct_fillet_set_address(&declaration.symbol, &key, output)
                    }));
                }
                continue;
            }
        }
        debug_assert_eq!(namespace, "geometry");
        let definition = keyed_polyline_definition(declaration)?;
        addresses.extend(
            definition.vertices.iter().map(|(key, _)| {
                direct_polyline_address(&declaration.symbol, "vertex", key, "point")
            }),
        );
        let segment_count = if definition.closed {
            definition.vertices.len()
        } else {
            definition.vertices.len() - 1
        };
        addresses.extend(
            definition.vertices[..segment_count].iter().map(|(key, _)| {
                direct_polyline_address(&declaration.symbol, "segment", key, "span")
            }),
        );
    }
    Ok(addresses)
}

fn direct_declaration_owner_address(
    declaration: &SemanticSymbol,
    family: &str,
) -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(
        declaration.0.clone(),
        ["direct", family],
        ["self"],
        ["owner"],
    )
}

fn direct_polyline_address(
    declaration: &SemanticSymbol,
    member_kind: &str,
    key: &str,
    output: &str,
) -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(
        declaration.0.clone(),
        ["polyline", member_kind],
        [key],
        [output],
    )
}

fn direct_fillet_set_address(
    declaration: &SemanticSymbol,
    key: &str,
    output: &str,
) -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(
        declaration.0.clone(),
        ["computed", "filletSet"],
        [key],
        [output],
    )
}

fn direct_fillet_set_keys(
    declaration: &AuthoringDeclaration,
) -> Result<Vec<String>, CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    array(
        required(arguments, "corners", &declaration.symbol.0)?,
        "FilletSet corners",
    )?
    .iter()
    .map(|corner| {
        let corner = object(corner, "FilletSet corner")?;
        Ok(string(
            required(corner, "key", "FilletSet corner")?,
            "FilletSet corner key",
        )?
        .to_owned())
    })
    .collect()
}

/// Lowers a validated code project and exact keyed reconciliation state into
/// one ordinary, unordered `IntentPatch` plus semantic/host provenance.
///
/// # Errors
///
/// Fails before publishing a patch for invalid pins, unsupported families,
/// dangling/kind-mismatched references, reconciliation drift or bounds.
pub fn expand_code_project(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    expected: IntentSessionIdentity,
) -> Result<ExpandedCodeProject, CodeExpansionError> {
    expand_code_project_with_overlay(
        project,
        reconciliation,
        &CodeInteractionOverlay::empty(),
        expected,
    )
}

/// Lowers a project with one generation-authenticated GUI seed overlay.
/// Draft values remain instance seeds and never add constraints/equations.
///
/// # Errors
///
/// In addition to ordinary expansion failures, rejects non-finite, stale,
/// unknown, type-mismatched or conflicting drafts before a patch is returned.
#[allow(
    clippy::too_many_lines,
    reason = "the expansion transaction keeps validation, lowering order, provenance, and its authenticated digest in one auditable pipeline"
)]
pub fn expand_code_project_with_overlay(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    expected: IntentSessionIdentity,
) -> Result<ExpandedCodeProject, CodeExpansionError> {
    expand_code_project_with_overlay_and_planner(
        project,
        reconciliation,
        overlay,
        expected,
        &mut UnavailableOperationPlanner,
    )
}

/// Reconstructs one expansion using its retained source-relative native
/// operation inventories. This path is for canonical source/session replay;
/// restored composition must additionally regenerate every inventory against
/// native document authority before exposing the expansion.
pub(crate) fn expand_code_project_with_overlay_and_retained_operation_plans(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    expected: IntentSessionIdentity,
    retained: &[PreparedCodeOperationPlan],
) -> Result<ExpandedCodeProject, CodeExpansionError> {
    let mut planner = RetainedOperationPlanner {
        plans: retained,
        next: 0,
    };
    let expansion = expand_code_project_with_overlay_and_planner(
        project,
        reconciliation,
        overlay,
        expected,
        &mut planner,
    )?;
    planner.finish()?;
    Ok(expansion)
}

#[allow(
    clippy::too_many_lines,
    reason = "one expansion transaction authenticates reconciliation, lowering, provenance, and its digest"
)]
pub(crate) fn expand_code_project_with_overlay_and_planner(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    expected: IntentSessionIdentity,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<ExpandedCodeProject, CodeExpansionError> {
    reconciliation.validate()?;
    overlay.validate()?;
    validate_suppression_authority(project, overlay)?;
    validate_supported_overrides(reconciliation)?;
    let (mut builder, plans) = prepare(project, Some(reconciliation), overlay, operation_planner)?;
    let required = direct_generated_members(project)?
        .into_iter()
        .chain(
            plans
                .iter()
                .flat_map(|plan| plan.addresses.values().cloned()),
        )
        .collect::<BTreeSet<_>>();
    let active = reconciliation
        .active()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required != active {
        let missing = required.difference(&active).count();
        let unexpected = active.difference(&required).count();
        return Err(CodeExpansionError::ReconciliationMismatch(format!(
            "{missing} missing and {unexpected} unexpected active members"
        )));
    }

    for plan in plans {
        lower_invocation(
            &mut builder,
            &plan,
            reconciliation,
            overlay,
            operation_planner,
        )?;
    }
    lower_remaining_named_declarations(
        project,
        &mut builder,
        reconciliation,
        overlay,
        operation_planner,
    )?;

    let mut semantic_outputs = BTreeMap::new();
    for output in &project.managed.program.outputs {
        let value = builder.resolve(&output.declaration, &output.path)?;
        if semantic_outputs.len() >= MAX_EXPANDED_OUTPUTS {
            return Err(CodeExpansionError::ResourceLimit {
                resource: "managed outputs",
                actual: semantic_outputs.len().saturating_add(1),
                limit: MAX_EXPANDED_OUTPUTS,
            });
        }
        let generation = semantic_generation(&value);
        semantic_outputs.insert(
            output.name.clone(),
            ExpandedSemanticOutput {
                reference: OutputRef {
                    project: project.project.clone(),
                    declaration: output.declaration.clone(),
                    output: output.path.clone(),
                    expected_kind: value.kind(),
                    generation,
                },
                target: value.public_target(),
            },
        );
    }
    if builder.host_requests.len() > MAX_HOST_REQUESTS {
        return Err(CodeExpansionError::ResourceLimit {
            resource: "host requests",
            actual: builder.host_requests.len(),
            limit: MAX_HOST_REQUESTS,
        });
    }

    let patch = IntentPatch::new(
        expected,
        IntentPatchPolicy::RequireAccepted,
        builder.operations,
    );
    // `GeneratedMemberAddress` is a structured semantic key, not a JSON object
    // property. Canonicalize the already sorted map as rows so digesting never
    // relies on lossy/stringified map keys.
    validate_overlay_coverage(overlay, &builder.writable_points)?;
    validate_draft_conflicts(overlay, &builder.writable_points)?;
    apply_host_interaction_overlay(
        &builder.project,
        &mut builder.declaration_provenance,
        &mut builder.generated_children,
        &mut builder.host_requests,
        overlay,
    )?;
    let provenance_rows = builder.provenance.iter().collect::<Vec<_>>();
    let declaration_rows = builder.declaration_provenance.iter().collect::<Vec<_>>();
    let digest_bytes = serde_json::to_vec(&(
        &patch,
        &semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &builder.writable_points,
        &builder.generated_children,
        &builder.host_requests,
        &builder.operation_plans,
    ))
    .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    Ok(ExpandedCodeProject {
        patch,
        semantic_outputs,
        generated_provenance: builder.provenance,
        declaration_provenance: builder.declaration_provenance,
        writable_points: builder.writable_points,
        generated_children: builder.generated_children,
        host_requests: builder.host_requests,
        operation_plans: builder.operation_plans,
        digest: intent_content_digest(&digest_bytes).to_string(),
    })
}

/// Expands an explicit structural edit while deterministically pruning the
/// prior accepted interaction overlay to still-present, provenance-compatible
/// writable owners. Ordinary expansion remains strict and never prunes stale
/// persisted payloads implicitly.
///
/// # Errors
///
/// Returns the ordinary deterministic expansion or overlay-authentication
/// error without changing the project, reconciliation state, or overlay.
pub fn expand_code_project_for_structural_edit(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    current: &CodeInteractionOverlay,
    expected: IntentSessionIdentity,
) -> Result<(ExpandedCodeProject, CodeInteractionOverlay), CodeExpansionError> {
    let overlay_free = expand_code_project_with_overlay(
        project,
        reconciliation,
        &CodeInteractionOverlay::empty(),
        expected,
    )?;
    let retained = overlay_free.retained_overlay(current);
    if retained == CodeInteractionOverlay::empty() {
        Ok((overlay_free, retained))
    } else {
        let expansion =
            expand_code_project_with_overlay(project, reconciliation, &retained, expected)?;
        Ok((expansion, retained))
    }
}

fn apply_host_interaction_overlay(
    project: &ProjectKey,
    provenance: &mut BTreeMap<IntentKey, SemanticSymbol>,
    children: &mut Vec<ExpandedGeneratedChild>,
    requests: &mut [CodeHostRequest],
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    for request in requests {
        match request {
            CodeHostRequest::FilletAtCorner(request) => {
                let alias = host_intent_alias(&request.output, request.identity, None)?;
                let address = CodeGeneratedChildAddress::new(
                    project.clone(),
                    request.output.clone(),
                    request.identity,
                    SemanticOutputPath::default(),
                );
                request.suppressed |= overlay
                    .generated_child_suppression(&address)
                    .unwrap_or(false);
                insert_host_provenance(provenance, &alias, &request.invocation)?;
                children.push(ExpandedGeneratedChild {
                    alias,
                    declaration: request.invocation.clone(),
                    address,
                    suppressed: request.suppressed,
                });
            }
            CodeHostRequest::RoundedRectangleProfile {
                invocation,
                output,
                identity,
                corners,
                suppressed_children,
                ..
            } => {
                for key in corners.keys() {
                    let alias = host_intent_alias(output, *identity, Some(key))?;
                    let address = CodeGeneratedChildAddress::new(
                        project.clone(),
                        output.clone(),
                        *identity,
                        member_path(&[], key, &[]),
                    );
                    let suppressed = suppressed_children.contains(key)
                        || overlay
                            .generated_child_suppression(&address)
                            .unwrap_or(false);
                    if suppressed {
                        suppressed_children.insert(key.clone());
                    }
                    insert_host_provenance(provenance, &alias, invocation)?;
                    children.push(ExpandedGeneratedChild {
                        alias,
                        declaration: invocation.clone(),
                        address,
                        suppressed,
                    });
                }
            }
        }
    }
    children.sort_by(|left, right| left.alias.cmp(&right.alias));
    validate_generated_child_overlay_coverage(overlay, children)?;
    Ok(())
}

fn validate_suppression_authority(
    project: &CodeProject,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    if project.managed.has_compiled_authority() && !overlay.suppressed_children().is_empty() {
        return Err(CodeExpansionError::Unsupported(
            "managed generated suppression must be explicit source authority".into(),
        ));
    }
    Ok(())
}

fn suppress_host_request(request: &mut CodeHostRequest) {
    match request {
        CodeHostRequest::FilletAtCorner(request) => request.suppressed = true,
        CodeHostRequest::RoundedRectangleProfile {
            corners,
            suppressed_children,
            ..
        } => suppressed_children.extend(corners.keys().cloned()),
    }
}

fn insert_host_provenance(
    provenance: &mut BTreeMap<IntentKey, SemanticSymbol>,
    alias: &IntentKey,
    declaration: &SemanticSymbol,
) -> Result<(), CodeExpansionError> {
    if let Some(previous) = provenance.insert(alias.clone(), declaration.clone()) {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: declaration.0.clone(),
            message: format!(
                "generated host alias is already owned by declaration `{}`",
                previous.0
            ),
        });
    }
    Ok(())
}

fn validate_generated_child_overlay_coverage(
    overlay: &CodeInteractionOverlay,
    children: &[ExpandedGeneratedChild],
) -> Result<(), CodeExpansionError> {
    let known = children
        .iter()
        .map(|child| &child.address)
        .collect::<BTreeSet<_>>();
    for address in overlay.suppressed_children().keys() {
        if known.contains(address) {
            continue;
        }
        let same_semantic_owner = children.iter().any(|child| {
            child.address.project == address.project
                && child.address.owner.address == address.owner.address
                && child.address.child == address.child
        });
        if same_semantic_owner {
            return Err(CodeExpansionError::StaleDraftGeneration {
                address: address.display_path(),
            });
        }
        return Err(CodeExpansionError::UnknownDraft {
            address: address.display_path(),
        });
    }
    Ok(())
}

pub(crate) fn host_intent_alias(
    address: &GeneratedMemberAddress,
    identity: GeneratedMemberIdentity,
    suffix: Option<&str>,
) -> Result<IntentKey, CodeExpansionError> {
    let bytes = serde_json::to_vec(&(address, identity, suffix))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    IntentKey::new(format!("code.host.{}", intent_content_digest(&bytes))).map_err(Into::into)
}

fn prepare(
    project: &CodeProject,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<(ExpansionBuilder, Vec<InvocationPlan>), CodeExpansionError> {
    project.validate()?;
    let artifacts = pinned_artifacts(project)?;
    let source_suppressions = project
        .managed
        .compiled
        .as_deref()
        .map_or_else(
            || Ok(ManagedSuppressionProjection::default()),
            CompiledManagedSource::projected_suppressions,
        )
        .map_err(CodeProjectError::from)?;
    let mut builder = ExpansionBuilder::new(project.project.clone(), source_suppressions);

    for binding in &project.managed.program.scalar_bindings {
        lower_scalar_binding(&mut builder, binding)?;
    }
    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_some()
            || !managed_references_are_ready(&declaration.arguments, &builder.declarations)
        {
            continue;
        }
        lower_named_declaration(
            &mut builder,
            declaration,
            reconciliation,
            overlay,
            operation_planner,
        )?;
    }
    let mut plans = Vec::new();
    for declaration in &project.managed.program.declarations {
        let Some(patch) = &declaration.patch else {
            continue;
        };
        let pinned = resolve_pinned_artifact(project, &artifacts, &patch.module_binding)?;
        let collections = plan_collections(&builder, declaration, pinned.artifact.artifact())?;
        let (addresses, synthetic_outputs) =
            plan_invocation_addresses(declaration, pinned.artifact.artifact(), &collections)?;
        plans.push(InvocationPlan {
            declaration: declaration.clone(),
            pinned,
            collections,
            addresses,
            synthetic_outputs,
        });
    }
    Ok((builder, plans))
}

fn managed_references_are_ready(
    value: &ManagedValue,
    declarations: &BTreeMap<SemanticSymbol, SemanticDeclaration>,
) -> bool {
    match value {
        ManagedValue::Reference { declaration, .. } => declarations.contains_key(declaration),
        ManagedValue::Array(values) => values
            .iter()
            .all(|value| managed_references_are_ready(value, declarations)),
        ManagedValue::Object(fields) => fields
            .values()
            .all(|value| managed_references_are_ready(value, declarations)),
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_) => true,
    }
}

fn lower_scalar_binding(
    builder: &mut ExpansionBuilder,
    binding: &crate::ManagedScalarBinding,
) -> Result<(), CodeExpansionError> {
    let value = match &binding.value {
        ManagedValue::Number(value) if value.is_finite() => UnitLiteral {
            unit: "model".into(),
            value: *value,
        },
        ManagedValue::Unit(value) if value.value.is_finite() => value.clone(),
        _ => {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: binding.symbol.0.clone(),
                message: "lexical managed binding must own one finite numeric or unit literal"
                    .into(),
            });
        }
    };
    builder.insert_declaration(
        binding.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::ScalarLiteral(value),
            paths: BTreeMap::new(),
        },
    )
}

fn named_family_parts(
    declaration: &AuthoringDeclaration,
) -> Option<(
    &'static crate::declaration_catalog::CodeAuthoringFamilyDescriptor,
    &str,
    &str,
)> {
    let [namespace, method] = declaration.builder_path.as_slice() else {
        return None;
    };
    code_authoring_family(namespace, method)
        .map(|family| (family, namespace.as_str(), method.as_str()))
}

fn named_dynamic_children(
    declaration: &AuthoringDeclaration,
    policy: CodeAuthoringDynamicChildren,
) -> Result<u16, CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let count = match policy {
        CodeAuthoringDynamicChildren::None => 0,
        CodeAuthoringDynamicChildren::PolylineVertices => array(
            required(arguments, "vertices", &declaration.symbol.0)?,
            "Polyline vertices",
        )?
        .len(),
        CodeAuthoringDynamicChildren::SplineControls => array(
            required(arguments, "controls", &declaration.symbol.0)?,
            "NURBS controls",
        )?
        .len(),
        CodeAuthoringDynamicChildren::FilletCorners => array(
            required(arguments, "corners", &declaration.symbol.0)?,
            "Fillet corners",
        )?
        .len(),
        CodeAuthoringDynamicChildren::PatternInstances => {
            let value = finite_number(
                required(arguments, "instances", &declaration.symbol.0)?,
                "pattern instances",
            )?;
            if value < 0.0 || value.fract() != 0.0 {
                return invalid_declaration(
                    declaration,
                    "pattern instances must be a nonnegative integer".into(),
                );
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the exact u16 bound is checked immediately below"
            )]
            {
                if value > f64::from(u16::MAX) {
                    return invalid_declaration(
                        declaration,
                        "pattern instances exceed the native child bound".into(),
                    );
                }
                return Ok(value as u16);
            }
        }
    };
    u16::try_from(count).map_err(|_| CodeExpansionError::InvalidDeclaration {
        declaration: declaration.symbol.0.clone(),
        message: "dynamic declaration members exceed the native child bound".into(),
    })
}

fn lower_named_declaration(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<(), CodeExpansionError> {
    let Some((family, namespace, method)) = named_family_parts(declaration) else {
        return Err(CodeExpansionError::Unsupported(format!(
            "unknown managed declaration family `{}`",
            declaration.builder_path.join(".")
        )));
    };
    if family.availability == CodeAuthoringAvailability::RequiresHostSnapshot {
        return invalid_declaration(
            declaration,
            format!(
                "`{namespace}.{method}` requires immutable host snapshot authority and cannot replay standalone"
            ),
        );
    }
    let dynamic_children = named_dynamic_children(declaration, family.dynamic_children)?;
    let descriptor = resolve_code_authoring_declaration(namespace, method, dynamic_children)
        .map_err(|error| CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: error.to_string(),
        })?;

    match descriptor.declaration {
        CodeAuthoringDeclarationKind::Geometry(_) => {
            lower_named_geometry(builder, declaration, &descriptor, reconciliation, overlay)
        }
        CodeAuthoringDeclarationKind::Operation(_) => {
            if descriptor.result_policy == CodeAuthoringResultPolicy::AuthenticatedOperationPlan {
                lower_named_operation(builder, declaration, &descriptor, operation_planner)
            } else {
                lower_named_native(builder, declaration, &descriptor)
            }
        }
        CodeAuthoringDeclarationKind::ComputedFeature(ComputedFeatureKind::FilletSet) => {
            lower_named_fillet_set(builder, declaration, &descriptor, reconciliation)
        }
        CodeAuthoringDeclarationKind::Constraint(_)
        | CodeAuthoringDeclarationKind::Dimension(_)
        | CodeAuthoringDeclarationKind::Aggregate(_) => {
            lower_named_native(builder, declaration, &descriptor)
        }
    }
}

/// Publishes computed Fillet outputs as generation-authenticated host values.
///
/// The Intent node owns the durable Fillet definition, but its evaluated arcs
/// and corners are computed-feature products rather than native Intent ports.
/// Keeping them as [`SemanticValue::HostOutput`] prevents a source consumer
/// from laundering an evaluated arc into an ordinary native span input.
fn lower_named_fillet_set(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    reconciliation: Option<&KeyedReconcileState>,
) -> Result<(), CodeExpansionError> {
    lower_named_native(builder, declaration, descriptor)?;

    let alias = match builder
        .declarations
        .get(&declaration.symbol)
        .map(|lowered| &lowered.root)
    {
        Some(SemanticValue::Declaration { alias, .. }) => alias.clone(),
        _ => {
            return invalid_declaration(
                declaration,
                "FilletSet did not lower to one computed-feature declaration".into(),
            );
        }
    };
    let keys = direct_fillet_set_keys(declaration)?;
    let planning_identity = GeneratedMemberIdentity {
        allocation: 0,
        generation: 0,
    };
    let mut paths = BTreeMap::new();
    let mut fillets = BTreeMap::new();
    let mut provenance = Vec::new();
    for key in keys {
        let mut members = BTreeMap::new();
        for (output, kind) in [
            ("arc", FeatureKind::CurveSpan),
            ("corner", FeatureKind::FeatureCorner),
        ] {
            let path = member_path(&["fillets"], &key, &[output]);
            let generated = builder
                .lowering_output_owner(&declaration.symbol, &path)
                .map(|(address, identity)| (address.clone(), identity));
            let address = generated.as_ref().map_or_else(
                || direct_fillet_set_address(&declaration.symbol, &key, output),
                |(address, _)| address.clone(),
            );
            let identity = generated.as_ref().map_or_else(
                || {
                    reconciliation
                        .and_then(|state| state.active().get(&address).copied())
                        .unwrap_or(planning_identity)
                },
                |(_, identity)| *identity,
            );
            let value = SemanticValue::HostOutput {
                address: address.clone(),
                identity,
                kind,
            };
            paths.insert(path, value.clone());
            members.insert(output.to_owned(), value.clone());
            if reconciliation.is_some() && generated.is_none() {
                provenance.push((address, identity, value));
            }
        }
        fillets.insert(
            key,
            SemanticValue::Feature {
                alias: alias.clone(),
                members,
            },
        );
    }
    let collection = SemanticValue::Collection(fillets);
    paths.insert(fields_path(&["fillets"]), collection);
    builder
        .declarations
        .get_mut(&declaration.symbol)
        .expect("named FilletSet was inserted above")
        .paths = paths;
    for (address, identity, value) in provenance {
        builder.add_provenance(&address, identity, &declaration.symbol, None, &value)?;
    }
    Ok(())
}

fn lower_named_geometry(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let CodeAuthoringDeclarationKind::Geometry(recipe) = descriptor.declaration else {
        unreachable!("named geometry lowering receives a geometry descriptor")
    };
    match recipe {
        GeometryRecipeKind::Segment => {
            lower_direct_line(builder, declaration, reconciliation, overlay)
        }
        GeometryRecipeKind::CenterRadiusCircle => {
            lower_direct_circle(builder, declaration, reconciliation, overlay)
        }
        GeometryRecipeKind::Polyline => {
            lower_direct_polyline(builder, declaration, reconciliation, overlay)
        }
        GeometryRecipeKind::MidpointLine
        | GeometryRecipeKind::TwoPointAlignedRectangle
        | GeometryRecipeKind::ThreePointCornerRectangle
        | GeometryRecipeKind::CenterRectangle
        | GeometryRecipeKind::ThreePointCenterRectangle
        | GeometryRecipeKind::TwoPointDiameterCircle
        | GeometryRecipeKind::ThreePointCircle
        | GeometryRecipeKind::CenterArc
        | GeometryRecipeKind::ThreePointArc
        | GeometryRecipeKind::CenterAxesEllipse
        | GeometryRecipeKind::AxisEndpointsEllipse
        | GeometryRecipeKind::CenterAxesEllipticalArc
        | GeometryRecipeKind::AxisEndpointsEllipticalArc => {
            lower_named_geometry_samples(builder, declaration, descriptor, reconciliation, overlay)
        }
        GeometryRecipeKind::TangentArc => {
            lower_named_tangent_arc(builder, declaration, reconciliation, overlay)
        }
        GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs => {
            lower_named_nurbs(builder, declaration, descriptor, reconciliation, overlay)
        }
        _ => {
            lower_named_geometry_generic(builder, declaration, descriptor, reconciliation, overlay)
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedNamedGeometryPoint {
    slot: InputSlot,
    semantic_path: SemanticOutputPath,
    position: [f64; 2],
    resolved: SemanticValue,
    source: CodePointSeedSource,
    address: Option<CodeWritableAddress>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one sample-derived geometry boundary keeps source samples distinct from native stored points"
)]
fn lower_named_geometry_samples(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let CodeAuthoringDeclarationKind::Geometry(recipe) = descriptor.declaration else {
        unreachable!("sample geometry lowering receives a geometry descriptor")
    };
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let alias = builder.lowering_alias("geometry", &declaration.symbol, &[])?;
    let kind = IntentNodeKind::Geometry { recipe };
    let mut definition_fields = BTreeMap::new();
    for field in &descriptor.fields {
        let value = named_projection_value(arguments, &field.path);
        let literal = match value {
            Some(value) => Some(named_literal(
                builder,
                &value,
                field.schema.literal,
                &format!(
                    "{}.{}",
                    declaration.symbol.0,
                    path_text(&semantic_output_path(&field.path))
                ),
            )?),
            None => match &field.default {
                IntentFieldDefault::Literal(value) => Some(value.clone()),
                IntentFieldDefault::Contextual | IntentFieldDefault::Conditional => None,
                IntentFieldDefault::Required => {
                    return invalid_declaration(
                        declaration,
                        format!(
                            "missing required geometry field `{}`",
                            path_text(&semantic_output_path(&field.path))
                        ),
                    );
                }
            },
        };
        if let Some(literal) = literal {
            definition_fields.insert(field.schema.field.clone(), literal);
        }
    }

    let mut input_probe = IntentNodeDraft::new(kind.clone(), alias.clone())
        .with_dynamic_children(descriptor.dynamic_children.count);
    for (field, value) in &definition_fields {
        input_probe = input_probe.with_field(field.clone(), value.clone());
    }
    let input_probe = named_geometry_probe(&input_probe, descriptor)?;
    let input_outputs = input_probe.output_descriptors().map_err(|error| {
        CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: format!("named geometry input probe is invalid: {error}"),
        }
    })?;
    let mut points = Vec::new();
    for input in &descriptor.inputs {
        let (CodeAuthoringInputBinding::Slot { slot }, CodeAuthoringArgumentKind::Point) =
            (&input.binding, &input.kind)
        else {
            return invalid_declaration(
                declaration,
                format!(
                    "sample-derived geometry input `{}` is not one point slot",
                    input.name
                ),
            );
        };
        let value = required(arguments, &input.name, &declaration.symbol.0)?;
        let label = format!("{}.{}", declaration.symbol.0, input.name);
        let semantic_path = kind
            .draft_input_projection_path(*slot, descriptor.dynamic_children.count)
            .map_or_else(
                || fields_path(&[input.name.as_str()]),
                |path| semantic_output_path(&path),
            );
        let native_output = input_outputs
            .iter()
            .find(|output| output.alias_input == Some(*slot));
        let address = native_output.and_then(|output| {
            (output.kind == IntentPortKind::Point).then(|| {
                builder.lowering_point_address(
                    declaration,
                    &declaration.builder_path[1],
                    reconciliation,
                    semantic_output_path(&output.path),
                )
            })?
        });
        let resolved = if let Some(position) = address
            .as_ref()
            .and_then(|address| overlay_point(overlay, address))
        {
            SemanticValue::PointLiteral(position)
        } else {
            builder.resolve_managed(value, &SemanticOutputPath::default(), &label)?
        };
        let position = builder.point_seed(&resolved).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("point input `{}` has no deterministic seed", input.name),
            }
        })?;
        let source = match value {
            ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
                declaration: declaration.clone(),
                path: path.clone(),
            },
            _ => CodePointSeedSource::Literal,
        };
        points.push(ResolvedNamedGeometryPoint {
            slot: *slot,
            semantic_path,
            position,
            resolved,
            source,
            address,
        });
    }

    let mut stage_positions = points
        .iter()
        .map(|point| point.position)
        .collect::<Vec<_>>();
    if recipe == GeometryRecipeKind::ThreePointArc {
        let [first, through, end] = stage_positions.as_slice() else {
            return invalid_declaration(
                declaration,
                "Three-Point Arc requires first, second and third samples".into(),
            );
        };
        // The clean API names geometric order (Start, Through, End), while
        // the interaction recipe stages Start, End, Through.
        stage_positions = vec![*first, *end, *through];
    }
    if recipe == GeometryRecipeKind::ThreePointCenterRectangle {
        let [center, corner] = stage_positions.as_slice() else {
            return invalid_declaration(
                declaration,
                "Three-Point Center Rectangle requires center and corner point inputs".into(),
            );
        };
        let side_midpoint =
            match definition_fields.get(&IntentFieldKey(IntentKey::new("side_midpoint")?)) {
                Some(IntentLiteral::Point(position)) => *position,
                None => [center[0], corner[1]],
                _ => {
                    return invalid_declaration(
                        declaration,
                        "Three-Point Center Rectangle side midpoint must be a point".into(),
                    );
                }
            };
        stage_positions = vec![*center, side_midpoint, *corner];
    }
    let role = match definition_fields.get(&IntentFieldKey(IntentKey::new("role")?)) {
        Some(IntentLiteral::Enum(value)) if value.as_str() == "profile" => GeometryRole::Profile,
        Some(IntentLiteral::Enum(value)) if value.as_str() == "construction" => {
            GeometryRole::Construction
        }
        _ => {
            return invalid_declaration(
                declaration,
                "geometry role must be `profile` or `construction`".into(),
            );
        }
    };
    let arc_sweep = match definition_fields.get(&IntentFieldKey(IntentKey::new("sweep")?)) {
        Some(IntentLiteral::Enum(value)) if value.as_str() == "clockwise" => {
            DocumentArcSweep::Clockwise
        }
        Some(IntentLiteral::Enum(value)) if value.as_str() == "counter_clockwise" => {
            DocumentArcSweep::CounterClockwise
        }
        None => DocumentArcSweep::CounterClockwise,
        _ => {
            return invalid_declaration(
                declaration,
                "geometry sweep must be `clockwise` or `counterClockwise`".into(),
            );
        }
    };
    let samples = ProjectionalGeometrySamples {
        points: stage_positions,
        role,
        regularized: false,
        closed: false,
        conic_options: ConicConstructionOptions {
            arc_sweep,
            ..ConicConstructionOptions::default()
        },
        nurbs_options: NurbsConstructionOptions::default(),
    };
    let variant = GeometryToolVariant::from_intent_recipe(recipe);
    let plan = projectional_geometry_plan_from_samples(variant, &samples).map_err(|error| {
        CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: format!("geometry construction samples are invalid: {error}"),
        }
    })?;
    let projected =
        projectional_geometry_draft_from_plan(alias.clone(), variant, &plan).map_err(|error| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("geometry native recipe state is invalid: {error}"),
            }
        })?;
    let mut draft = projected.draft;
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "geometry label")?)?);
    }
    for (field, expected) in &definition_fields {
        if draft.fields.get(field) != Some(expected) {
            return invalid_declaration(
                declaration,
                format!(
                    "geometry construction samples disagree with definition field `{}`",
                    field.0.as_str()
                ),
            );
        }
    }

    draft
        .output_descriptors()
        .map_err(|error| CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: format!("sample-derived geometry draft is invalid: {error}"),
        })?;
    let seeds = projected.point_seeds;
    let mut writable = Vec::new();
    for point in points {
        let Some(output) = input_outputs
            .iter()
            .find(|output| output.alias_input == Some(point.slot))
        else {
            // A coordinate-only construction sample intentionally has no
            // persistent point output or writable native handle.
            continue;
        };
        if output.kind != IntentPortKind::Point {
            return invalid_declaration(
                declaration,
                format!(
                    "geometry point input path `{}` changed native kind",
                    path_text(&point.semantic_path)
                ),
            );
        }
        if let SemanticValue::Port(port) = point.resolved {
            draft.initial_instance.remove(&output.selector);
            draft = draft.with_input(point.slot, port.patch_ref());
        }
        if let Some(address) = point.address {
            writable.push(ExpandedWritablePoint {
                handle: port(&alias, output.selector, output.kind),
                source: point.source,
                edit: CodePointEdit::Point { address },
            });
        }
    }

    let mut outputs = draft_semantic_outputs(&draft, &alias)?;
    if matches!(
        recipe,
        GeometryRecipeKind::AxisEndpointsEllipse | GeometryRecipeKind::AxisEndpointsEllipticalArc
    ) {
        let major = outputs
            .get(&fields_path(&["majorAxisStart"]))
            .cloned()
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: "axis-endpoint ellipse has no native major-axis pole".into(),
            })?;
        outputs.insert(fields_path(&["majorAxisPoint"]), major);
    }
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    for (selector, position) in seeds {
        builder.add_point_seed(&port(&alias, selector, IntentPortKind::Point), position)?;
    }
    for point in writable {
        builder.add_writable_point(point);
    }
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths: outputs,
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the tangent-arc boundary decodes one complete explicit source contact cell before delegating native derivation"
)]
fn lower_named_tangent_arc(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let alias = builder.lowering_alias("geometry", &declaration.symbol, &[])?;
    let resolve_point = |builder: &ExpansionBuilder,
                         name: &str|
     -> Result<
        ([f64; 2], CodePointSeedSource, Option<CodeWritableAddress>),
        CodeExpansionError,
    > {
        let value = required(arguments, name, &declaration.symbol.0)?;
        let path = fields_path(&[name]);
        let address = builder.lowering_point_address(
            declaration,
            &declaration.builder_path[1],
            reconciliation,
            path,
        );
        let resolved = if let Some(position) = address
            .as_ref()
            .and_then(|address| overlay_point(overlay, address))
        {
            SemanticValue::PointLiteral(position)
        } else {
            builder.resolve_managed(
                value,
                &SemanticOutputPath::default(),
                &format!("{}.{}", declaration.symbol.0, name),
            )?
        };
        let position = builder.point_seed(&resolved).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("Tangent Arc `{name}` has no deterministic point seed"),
            }
        })?;
        let source = match value {
            ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
                declaration: declaration.clone(),
                path: path.clone(),
            },
            _ => CodePointSeedSource::Literal,
        };
        Ok((position, source, address))
    };
    let (center, center_source, center_address) = resolve_point(builder, "center")?;
    let (start, start_source, start_address) = resolve_point(builder, "start")?;
    let (end, end_source, end_address) = resolve_point(builder, "end")?;
    let source = object(
        required(arguments, "source", &declaration.symbol.0)?,
        "Tangent Arc source",
    )?;
    if source
        .keys()
        .any(|key| !matches!(key.as_str(), "span" | "contact"))
    {
        return invalid_declaration(
            declaration,
            "Tangent Arc source accepts only `span` and optional `contact`".into(),
        );
    }
    let source_span = resolve_named_reference(
        builder,
        required(source, "span", "Tangent Arc source")?,
        &CodeAuthoringArgumentKind::Reference(IntentPortKind::CurveSpan),
        &format!("{}.source.span", declaration.symbol.0),
    )?;

    let orientation = arguments
        .get("orientation")
        .map(|value| tangent_orientation(value, "Tangent Arc orientation"))
        .transpose()?
        .unwrap_or(TangentOrientation::Aligned);
    let (
        source_parameter,
        source_winding,
        source_supporting_line,
        source_range,
        source_neighborhood,
    ) = if let Some(contact) = source.get("contact") {
        let contact = object(contact, "Tangent Arc source contact")?;
        if contact.keys().any(|key| {
            !matches!(
                key.as_str(),
                "parameter" | "winding" | "support" | "range" | "neighborhood" | "orientation"
            )
        }) || !["parameter", "winding", "neighborhood", "orientation"]
            .into_iter()
            .all(|key| contact.contains_key(key))
        {
            return invalid_declaration(
                    declaration,
                    "Tangent Arc source contact requires `parameter`, `winding`, `neighborhood`, and `orientation`, and accepts only optional `support` and `range`".into(),
                );
        }
        let contact_orientation = tangent_orientation(
            required(contact, "orientation", "Tangent Arc source contact")?,
            "Tangent Arc source contact orientation",
        )?;
        if contact_orientation != orientation {
            return invalid_declaration(
                declaration,
                "Tangent Arc source-contact orientation must match `orientation`".into(),
            );
        }
        (
            finite_number(
                required(contact, "parameter", "Tangent Arc source contact")?,
                "Tangent Arc source parameter",
            )?,
            integer_value(
                required(contact, "winding", "Tangent Arc source contact")?,
                "Tangent Arc source winding",
            )?,
            contact
                .get("support")
                .map(|value| {
                    if string(value, "Tangent Arc source support")? == "supportingLine" {
                        Ok(true)
                    } else {
                        Err(CodeExpansionError::Unsupported(
                            "Tangent Arc source support must be `supportingLine`".into(),
                        ))
                    }
                })
                .transpose()?
                .unwrap_or(false),
            contact
                .get("range")
                .map(tangent_contact_range)
                .transpose()?,
            tangent_contact_neighborhood(required(
                contact,
                "neighborhood",
                "Tangent Arc source contact",
            )?)?,
        )
    } else {
        (1.0, 0, false, None, ContactNeighborhood::End)
    };
    let sweep = arguments
        .get("sweep")
        .map(|value| document_arc_sweep(value, "Tangent Arc sweep"))
        .transpose()?
        .unwrap_or(DocumentArcSweep::CounterClockwise);
    let role = arguments
        .get("role")
        .map(|value| geometry_role(value, "Tangent Arc role"))
        .transpose()?
        .unwrap_or(GeometryRole::Profile);
    let definition = ProjectionalTangentArc {
        center,
        start,
        end,
        source: source_span.patch_ref(),
        source_supporting_line,
        source_range,
        source_parameter,
        source_winding,
        source_neighborhood,
        sweep,
        orientation,
        role,
    };
    let mut draft =
        projectional_tangent_arc_draft(alias.clone(), &definition).map_err(|error| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("Tangent Arc native recipe state is invalid: {error}"),
            }
        })?;
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "Tangent Arc label")?)?);
    }
    let outputs = draft_semantic_outputs(&draft, &alias)?;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    for (selector, position) in [
        (node_selector(IntentPortRole::Center, 0), center),
        (node_selector(IntentPortRole::Start, 0), start),
        (node_selector(IntentPortRole::End, 0), end),
    ] {
        builder.add_point_seed(&port(&alias, selector, IntentPortKind::Point), position)?;
    }
    for (name, source, address) in [
        ("center", center_source, center_address),
        ("start", start_source, start_address),
        ("end", end_source, end_address),
    ] {
        if let Some(address) = address {
            let output = outputs
                .get(&fields_path(&[name]))
                .and_then(|value| match value {
                    SemanticValue::Port(port) => Some(port.clone()),
                    _ => None,
                })
                .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                    declaration: declaration.symbol.0.clone(),
                    message: format!("Tangent Arc `{name}` output is not one point port"),
                })?;
            builder.add_writable_point(ExpandedWritablePoint {
                handle: output,
                source,
                edit: CodePointEdit::Point { address },
            });
        }
    }
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths: outputs,
        },
    )
}

fn tangent_orientation(
    value: &ManagedValue,
    label: &str,
) -> Result<TangentOrientation, CodeExpansionError> {
    match string(value, label)? {
        "aligned" => Ok(TangentOrientation::Aligned),
        "opposed" => Ok(TangentOrientation::Opposed),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be `aligned` or `opposed`"
        ))),
    }
}

fn document_arc_sweep(
    value: &ManagedValue,
    label: &str,
) -> Result<DocumentArcSweep, CodeExpansionError> {
    match string(value, label)? {
        "clockwise" => Ok(DocumentArcSweep::Clockwise),
        "counterClockwise" => Ok(DocumentArcSweep::CounterClockwise),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be `clockwise` or `counterClockwise`"
        ))),
    }
}

fn geometry_role(value: &ManagedValue, label: &str) -> Result<GeometryRole, CodeExpansionError> {
    match string(value, label)? {
        "profile" => Ok(GeometryRole::Profile),
        "construction" => Ok(GeometryRole::Construction),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be `profile` or `construction`"
        ))),
    }
}

fn tangent_contact_range(
    value: &ManagedValue,
) -> Result<ContactAdmissibleRange, CodeExpansionError> {
    let values = object(value, "Tangent Arc source range")?;
    exact_object_keys(values, &["lower", "upper"], "Tangent Arc source range")?;
    let lower = finite_number(
        required(values, "lower", "Tangent Arc source range")?,
        "Tangent Arc source range lower",
    )?;
    let upper = finite_number(
        required(values, "upper", "Tangent Arc source range")?,
        "Tangent Arc source range upper",
    )?;
    if lower > upper {
        return Err(CodeExpansionError::Unsupported(
            "Tangent Arc source range must be ordered".into(),
        ));
    }
    Ok(ContactAdmissibleRange { lower, upper })
}

fn tangent_contact_neighborhood(
    value: &ManagedValue,
) -> Result<ContactNeighborhood, CodeExpansionError> {
    let values = object(value, "Tangent Arc source neighborhood")?;
    match string(
        required(values, "kind", "Tangent Arc source neighborhood")?,
        "Tangent Arc source neighborhood kind",
    )? {
        "interior" => {
            exact_object_keys(values, &["kind"], "Tangent Arc source neighborhood")?;
            Ok(ContactNeighborhood::Interior)
        }
        "start" => {
            exact_object_keys(values, &["kind"], "Tangent Arc source neighborhood")?;
            Ok(ContactNeighborhood::Start)
        }
        "end" => {
            exact_object_keys(values, &["kind"], "Tangent Arc source neighborhood")?;
            Ok(ContactNeighborhood::End)
        }
        "local" => {
            exact_object_keys(
                values,
                &["kind", "lower", "upper"],
                "Tangent Arc source neighborhood",
            )?;
            Ok(ContactNeighborhood::Local {
                lower: finite_number(
                    required(values, "lower", "Tangent Arc source neighborhood")?,
                    "Tangent Arc source neighborhood lower",
                )?,
                upper: finite_number(
                    required(values, "upper", "Tangent Arc source neighborhood")?,
                    "Tangent Arc source neighborhood upper",
                )?,
            })
        }
        _ => Err(CodeExpansionError::Unsupported(
            "Tangent Arc source neighborhood kind is invalid".into(),
        )),
    }
}

#[derive(Clone, Debug)]
struct ResolvedNurbsControl {
    key: String,
    position: [f64; 2],
    resolved: SemanticValue,
    source: CodePointSeedSource,
    address: Option<CodeWritableAddress>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one family-owned lowering keeps keyed controls, projective gauge normalization, native topology, and result paths together"
)]
fn lower_named_nurbs(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    _descriptor: &CodeAuthoringDeclarationDescriptor,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let controls = array(
        required(arguments, "controls", &declaration.symbol.0)?,
        "NURBS controls",
    )?;
    let mut resolved_controls = Vec::with_capacity(controls.len());
    let mut authored_weights = Vec::with_capacity(controls.len());
    let mut seen_keys = BTreeSet::new();
    for (ordinal, control) in controls.iter().enumerate() {
        let control = object(control, "NURBS control")?;
        exact_object_keys(control, &["key", "position", "weight"], "NURBS control")?;
        let key = string(
            required(control, "key", "NURBS control")?,
            "NURBS control key",
        )?;
        if key.is_empty() || !seen_keys.insert(key.to_owned()) {
            return invalid_declaration(
                declaration,
                format!("NURBS control key `{key}` is empty or duplicated"),
            );
        }
        let source = required(control, "position", "NURBS control")?;
        let label = format!("{}.controls[{ordinal}].position", declaration.symbol.0);
        let output = member_path(&["controls"], key, &["position"]);
        let address = builder.lowering_point_address(
            declaration,
            &declaration.builder_path[1],
            reconciliation,
            output,
        );
        let resolved = if let Some(position) = address
            .as_ref()
            .and_then(|address| overlay_point(overlay, address))
        {
            SemanticValue::PointLiteral(position)
        } else {
            builder.resolve_managed(source, &SemanticOutputPath::default(), &label)?
        };
        let position = builder.point_seed(&resolved).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("NURBS control `{key}` has no deterministic point seed"),
            }
        })?;
        let weight = finite_number(
            required(control, "weight", "NURBS control")?,
            "NURBS control weight",
        )?;
        if weight <= 0.0 {
            return invalid_declaration(
                declaration,
                format!("NURBS control `{key}` weight must be positive"),
            );
        }
        resolved_controls.push(ResolvedNurbsControl {
            key: key.to_owned(),
            position,
            resolved,
            source: match source {
                ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
                    declaration: declaration.clone(),
                    path: path.clone(),
                },
                _ => CodePointSeedSource::Literal,
            },
            address,
        });
        authored_weights.push(weight);
    }
    let degree_value = finite_number(
        required(arguments, "degree", &declaration.symbol.0)?,
        "NURBS degree",
    )?;
    if degree_value.fract() != 0.0 || !(1.0..=f64::from(u32::MAX)).contains(&degree_value) {
        return invalid_declaration(
            declaration,
            "NURBS degree must be a positive u32 integer".into(),
        );
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the exact positive u32 range and integrality are checked immediately above"
    )]
    let degree = degree_value as u32;
    let gauge = string(
        required(arguments, "gauge", &declaration.symbol.0)?,
        "NURBS gauge",
    )?;
    let gauge_index = resolved_controls
        .iter()
        .position(|control| control.key == gauge)
        .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: format!("NURBS gauge `{gauge}` does not name one control"),
        })?;
    let gauge_weight = authored_weights[gauge_index];
    let weights = authored_weights
        .iter()
        .map(|weight| weight / gauge_weight)
        .collect::<Vec<_>>();
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight <= 0.0)
    {
        return invalid_declaration(
            declaration,
            "NURBS projective gauge normalization produced an invalid weight".into(),
        );
    }
    let recipe = match declaration.builder_path.get(1).map(String::as_str) {
        Some("openControlNurbs") => GeometryRecipeKind::OpenControlNurbs,
        Some("periodicControlNurbs") => GeometryRecipeKind::PeriodicControlNurbs,
        _ => unreachable!("NURBS lowering receives one closed named family"),
    };
    let form = if recipe == GeometryRecipeKind::OpenControlNurbs {
        DocumentBSplineForm::Clamped
    } else {
        DocumentBSplineForm::Periodic
    };
    let role = arguments
        .get("role")
        .map(|value| geometry_role(value, "NURBS role"))
        .transpose()?
        .unwrap_or(GeometryRole::Profile);
    let variant = GeometryToolVariant::from_intent_recipe(recipe);
    let samples = ProjectionalGeometrySamples {
        points: resolved_controls
            .iter()
            .map(|control| control.position)
            .collect(),
        role,
        regularized: false,
        closed: false,
        conic_options: ConicConstructionOptions::default(),
        nurbs_options: NurbsConstructionOptions {
            form,
            degree,
            weights,
            gauge_index,
        },
    };
    let plan = projectional_geometry_plan_from_samples(variant, &samples).map_err(|error| {
        CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: format!("NURBS construction controls are invalid: {error}"),
        }
    })?;
    let alias = builder.lowering_alias("geometry", &declaration.symbol, &[])?;
    let projected =
        projectional_geometry_draft_from_plan(alias.clone(), variant, &plan).map_err(|error| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("NURBS native recipe state is invalid: {error}"),
            }
        })?;
    let mut draft = projected.draft;
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "NURBS label")?)?);
    }
    for (ordinal, control) in resolved_controls.iter().enumerate() {
        let ordinal =
            u16::try_from(ordinal).map_err(|_| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: "NURBS control count exceeds the native child bound".into(),
            })?;
        let selector = child_selector(ordinal, IntentPortRole::Control);
        if let SemanticValue::Port(source) = &control.resolved
            && source.kind == IntentPortKind::Point
        {
            draft.initial_instance.remove(&selector);
            draft = draft.with_input(
                InputSlot::new(InputRole::Point, ordinal),
                source.patch_ref(),
            );
        }
    }
    // Validate the exact keyed-independent native result shape before the
    // ergonomic keyed projection below is published.
    let native_outputs = draft_semantic_outputs(&draft, &alias)?;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;

    let mut paths = BTreeMap::new();
    paths.insert(
        fields_path(&["curve"]),
        native_outputs
            .get(&fields_path(&["curve"]))
            .cloned()
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: "NURBS recipe has no native curve output".into(),
            })?,
    );
    let mut control_members = BTreeMap::new();
    for (ordinal, control) in resolved_controls.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).expect("NURBS child count checked above");
        let position = port(
            &alias,
            child_selector(ordinal, IntentPortRole::Control),
            IntentPortKind::Point,
        );
        let weight = port(
            &alias,
            child_selector(ordinal, IntentPortRole::Target),
            IntentPortKind::Scalar,
        );
        builder.add_point_seed(&position, control.position)?;
        if let Some(address) = &control.address {
            builder.add_writable_point(ExpandedWritablePoint {
                handle: position.clone(),
                source: control.source.clone(),
                edit: CodePointEdit::Point {
                    address: address.clone(),
                },
            });
        }
        paths.insert(
            member_path(&["controls"], &control.key, &["position"]),
            SemanticValue::Port(position.clone()),
        );
        paths.insert(
            fields_path(&["controls", "byKey", &control.key, "position"]),
            SemanticValue::Port(position.clone()),
        );
        paths.insert(
            member_path(&["controls"], &control.key, &["weight"]),
            SemanticValue::Port(weight.clone()),
        );
        paths.insert(
            fields_path(&["controls", "byKey", &control.key, "weight"]),
            SemanticValue::Port(weight.clone()),
        );
        control_members.insert(
            control.key.clone(),
            SemanticValue::Collection(BTreeMap::from([
                ("position".into(), SemanticValue::Port(position)),
                ("weight".into(), SemanticValue::Port(weight)),
            ])),
        );
    }
    paths.insert(
        fields_path(&["controls"]),
        SemanticValue::Collection(control_members),
    );
    let degree = usize::try_from(degree).map_err(|_| CodeExpansionError::InvalidDeclaration {
        declaration: declaration.symbol.0.clone(),
        message: "NURBS degree does not fit this platform".into(),
    })?;
    let span_count = match form {
        DocumentBSplineForm::Clamped => {
            resolved_controls.len().checked_sub(degree).ok_or_else(|| {
                CodeExpansionError::InvalidDeclaration {
                    declaration: declaration.symbol.0.clone(),
                    message: "NURBS degree exceeds its control count".into(),
                }
            })?
        }
        DocumentBSplineForm::Periodic => resolved_controls.len(),
    };
    let mut span_members = BTreeMap::new();
    for (ordinal, control) in resolved_controls[..span_count].iter().enumerate() {
        let span = port(
            &alias,
            node_selector(
                IntentPortRole::Span,
                u16::try_from(ordinal).expect("NURBS span count is child-bounded"),
            ),
            IntentPortKind::CurveSpan,
        );
        paths.insert(
            member_path(&["spans"], &control.key, &[]),
            SemanticValue::Port(span.clone()),
        );
        paths.insert(
            fields_path(&["spans", "byKey", &control.key]),
            SemanticValue::Port(span.clone()),
        );
        span_members.insert(control.key.clone(), SemanticValue::Port(span));
    }
    paths.insert(
        fields_path(&["spans"]),
        SemanticValue::Collection(span_members),
    );
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths,
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one catalog-driven geometry lowering keeps native fields, point provenance, and output authority together"
)]
fn lower_named_geometry_generic(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let CodeAuthoringDeclarationKind::Geometry(recipe) = descriptor.declaration else {
        unreachable!("named geometry lowering receives a geometry descriptor")
    };
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let alias = builder.lowering_alias("geometry", &declaration.symbol, &[])?;
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Geometry { recipe }, alias.clone())
        .with_dynamic_children(descriptor.dynamic_children.count);
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "geometry label")?)?);
    }

    for field in &descriptor.fields {
        let Some(value) = named_projection_value(arguments, &field.path) else {
            if let IntentFieldDefault::Literal(value) = &field.default {
                draft = draft.with_field(field.schema.field.clone(), value.clone());
            } else if field.default == IntentFieldDefault::Required {
                return invalid_declaration(
                    declaration,
                    format!(
                        "missing required geometry field `{}`",
                        path_text(&semantic_output_path(&field.path))
                    ),
                );
            }
            continue;
        };
        draft = draft.with_field(
            field.schema.field.clone(),
            named_literal(
                builder,
                &value,
                field.schema.literal,
                &format!(
                    "{}.{}",
                    declaration.symbol.0,
                    path_text(&semantic_output_path(&field.path))
                ),
            )?,
        );
    }

    let probe = named_geometry_probe(&draft, descriptor)?;
    let probe_outputs =
        probe
            .output_descriptors()
            .map_err(|error| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("named geometry probe is invalid: {error}"),
            })?;
    let mut seeds = BTreeMap::<IntentPortSelector, [f64; 2]>::new();
    let mut writable = Vec::new();
    for input in &descriptor.inputs {
        match (&input.binding, &input.kind) {
            (CodeAuthoringInputBinding::Slot { slot }, CodeAuthoringArgumentKind::Point) => {
                let value = required(arguments, &input.name, &declaration.symbol.0)?;
                let label = format!("{}.{}", declaration.symbol.0, input.name);
                let semantic_path = IntentNodeKind::Geometry { recipe }
                    .draft_input_projection_path(*slot, descriptor.dynamic_children.count)
                    .map_or_else(
                        || fields_path(&[input.name.as_str()]),
                        |path| semantic_output_path(&path),
                    );
                let output = probe_outputs
                    .iter()
                    .find(|output| output.alias_input == Some(*slot))
                    .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!("point input `{}` has no native output", input.name),
                    })?;
                let source = match value {
                    ManagedValue::Reference { declaration, path } => {
                        CodePointSeedSource::Reference {
                            declaration: declaration.clone(),
                            path: path.clone(),
                        }
                    }
                    _ => CodePointSeedSource::Literal,
                };
                let address = builder.lowering_point_address(
                    declaration,
                    &declaration.builder_path[1],
                    reconciliation,
                    semantic_path.clone(),
                );
                let overridden = address
                    .as_ref()
                    .and_then(|address| overlay_point(overlay, address));
                let resolved = if let Some(position) = overridden {
                    SemanticValue::PointLiteral(position)
                } else {
                    builder.resolve_managed(value, &SemanticOutputPath::default(), &label)?
                };
                let position = builder.point_seed(&resolved).ok_or_else(|| {
                    CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!("point input `{}` has no deterministic seed", input.name),
                    }
                })?;
                match resolved {
                    SemanticValue::PointLiteral(position) => {
                        draft = draft
                            .with_instance_leaf(output.selector, LeafField::X, length(position[0]))
                            .with_instance_leaf(output.selector, LeafField::Y, length(position[1]));
                    }
                    value => {
                        let port = value.as_port(IntentPortKind::Point, &label)?;
                        draft = draft.with_input(*slot, port.patch_ref());
                    }
                }
                seeds.insert(output.selector, position);
                if let Some(address) = address {
                    writable.push(ExpandedWritablePoint {
                        handle: port(&alias, output.selector, output.kind),
                        source,
                        edit: CodePointEdit::Point { address },
                    });
                }
            }
            (CodeAuthoringInputBinding::Slot { slot }, kind) => {
                let value = required(arguments, &input.name, &declaration.symbol.0)?;
                let port = resolve_named_reference(
                    builder,
                    value,
                    kind,
                    &format!("{}.{}", declaration.symbol.0, input.name),
                )?;
                draft = draft.with_input(*slot, port.patch_ref());
            }
            _ => {
                return invalid_declaration(
                    declaration,
                    format!(
                        "geometry input `{}` requires family-owned dynamic lowering",
                        input.name
                    ),
                );
            }
        }
    }

    for value in &descriptor.values {
        let path = semantic_output_path(&value.path);
        let source = named_projection_value(arguments, &value.path).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("missing geometry value `{}`", path_text(&path)),
            }
        })?;
        let output = probe_outputs
            .iter()
            .find(|output| semantic_output_path(&output.path) == path)
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("geometry value `{}` has no native output", path_text(&path)),
            })?;
        let [leaf] = output.writable.as_slice() else {
            return invalid_declaration(
                declaration,
                format!(
                    "geometry value `{}` is not one writable scalar",
                    path_text(&path)
                ),
            );
        };
        draft = draft.with_instance_leaf(
            output.selector,
            *leaf,
            named_literal(
                builder,
                &source,
                value.literal,
                &format!("{}.{}", declaration.symbol.0, path_text(&path)),
            )?,
        );
    }

    let outputs = draft_semantic_outputs(&draft, &alias)?;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    for (selector, position) in seeds {
        builder.add_point_seed(&port(&alias, selector, IntentPortKind::Point), position)?;
    }
    for point in writable {
        builder.add_writable_point(point);
    }
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths: outputs,
        },
    )
}

fn named_geometry_probe(
    draft: &IntentNodeDraft,
    descriptor: &CodeAuthoringDeclarationDescriptor,
) -> Result<IntentNodeDraft, CodeExpansionError> {
    let mut probe = draft.clone();
    let mut ordinal = 0_u16;
    for input in &descriptor.inputs {
        if let CodeAuthoringInputBinding::Slot { slot } = input.binding {
            let expected = match input.kind {
                CodeAuthoringArgumentKind::Point => IntentPortKind::Point,
                CodeAuthoringArgumentKind::Reference(kind) => kind,
                CodeAuthoringArgumentKind::ReferenceChoice(ref kinds) => {
                    kinds.first().copied().unwrap_or(IntentPortKind::Feature)
                }
                CodeAuthoringArgumentKind::Collection(_) => continue,
            };
            probe = probe.with_input(
                slot,
                PatchPortRef::Alias {
                    node: IntentKey::new(format!("namedProbe{ordinal}"))?,
                    selector: node_selector(IntentPortRole::Primary, 0),
                },
            );
            let _ = expected;
            ordinal = ordinal.saturating_add(1);
        }
    }
    Ok(probe)
}

#[allow(
    clippy::too_many_lines,
    reason = "one catalog-driven native lowering keeps inputs, definition fields, values, and semantic results auditable together"
)]
fn lower_named_native(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
) -> Result<(), CodeExpansionError> {
    let (alias, draft) = build_named_native_draft(builder, declaration, descriptor)?;
    publish_named_native(builder, declaration, descriptor, &alias, draft)
}

#[allow(
    clippy::too_many_lines,
    reason = "one catalog-driven draft builder keeps inputs, definition fields, and authored values auditable together"
)]
fn build_named_native_draft(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
) -> Result<(IntentKey, IntentNodeDraft), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let alias = builder.lowering_alias(descriptor.namespace, &declaration.symbol, &[])?;
    let kind = descriptor.declaration.intent_kind();
    let mut draft = IntentNodeDraft::new(kind, alias.clone())
        .with_dynamic_children(descriptor.dynamic_children.count);
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "declaration label")?)?);
    }
    draft.suppressed = arguments
        .get("suppressed")
        .map(|value| boolean(value, "suppressed"))
        .transpose()?
        .unwrap_or(false);

    for input in &descriptor.inputs {
        lower_named_input(builder, declaration, &mut draft, arguments, input)?;
    }
    for field in &descriptor.fields {
        let value = named_projection_value(arguments, &field.path);
        let literal = match value {
            Some(value) => Some(named_literal(
                builder,
                &value,
                field.schema.literal,
                &format!(
                    "{}.{}",
                    declaration.symbol.0,
                    path_text(&semantic_output_path(&field.path))
                ),
            )?),
            None => match &field.default {
                IntentFieldDefault::Literal(value) => Some(value.clone()),
                IntentFieldDefault::Contextual | IntentFieldDefault::Conditional => None,
                IntentFieldDefault::Required => {
                    return invalid_declaration(
                        declaration,
                        format!(
                            "missing required named field `{}`",
                            path_text(&semantic_output_path(&field.path))
                        ),
                    );
                }
            },
        };
        if let Some(literal) = literal {
            draft = draft.with_field(field.schema.field.clone(), literal);
        }
    }

    let provisional_outputs =
        draft
            .output_descriptors()
            .map_err(|error| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("named declaration draft is invalid: {error}"),
            })?;
    for value in &descriptor.values {
        let path = semantic_output_path(&value.path);
        let source = named_projection_value(arguments, &value.path).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("missing required authored value `{}`", path_text(&path)),
            }
        })?;
        let output = provisional_outputs
            .iter()
            .find(|output| semantic_output_path(&output.path) == path)
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("authored value `{}` has no native output", path_text(&path)),
            })?;
        let [leaf] = output.writable.as_slice() else {
            return invalid_declaration(
                declaration,
                format!(
                    "authored value `{}` is not one writable scalar",
                    path_text(&path)
                ),
            );
        };
        draft = draft.with_instance_leaf(
            output.selector,
            *leaf,
            named_literal(
                builder,
                &source,
                value.literal,
                &format!("{}.{}", declaration.symbol.0, path_text(&path)),
            )?,
        );
    }

    Ok((alias, draft))
}

fn publish_named_native(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    alias: &IntentKey,
    draft: IntentNodeDraft,
) -> Result<(), CodeExpansionError> {
    let outputs = draft_semantic_outputs(&draft, alias)?;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    let root = named_declaration_root(descriptor.declaration, alias, &outputs);
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root,
            paths: outputs,
        },
    )
}

fn lower_named_operation(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    descriptor: &CodeAuthoringDeclarationDescriptor,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<(), CodeExpansionError> {
    let CodeAuthoringDeclarationKind::Operation(operation) = descriptor.declaration else {
        return invalid_declaration(
            declaration,
            "authenticated operation lowering received a non-operation descriptor".into(),
        );
    };
    let (alias, provisional) = build_named_native_draft(builder, declaration, descriptor)?;
    if !provisional.operation_outputs.is_empty() {
        return invalid_declaration(
            declaration,
            "provisional operation draft unexpectedly claims native outputs".into(),
        );
    }
    let mut prefix_aliases = builder
        .operations
        .iter()
        .map(|operation| match operation {
            IntentPatchOperation::CreateNode { alias, .. } => Ok(alias.clone()),
            _ => Err(CodeExpansionError::OperationPlanning {
                declaration: declaration.symbol.0.clone(),
                message: "native planning prefix contains a non-create operation".into(),
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    prefix_aliases.push(alias.clone());
    let total_prefix_aliases = builder
        .operation_plan_prefix_aliases
        .saturating_add(prefix_aliases.len());
    if total_prefix_aliases > MAX_OPERATION_PLAN_PREFIX_ALIASES {
        return Err(CodeExpansionError::ResourceLimit {
            resource: "operation planning prefix aliases",
            actual: total_prefix_aliases,
            limit: MAX_OPERATION_PLAN_PREFIX_ALIASES,
        });
    }
    let plan = operation_planner.prepare(&builder.operations, &alias, &provisional)?;
    if plan.operation != operation {
        return Err(CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message: "native planner returned a different operation kind".into(),
        });
    }
    let operation_symbol = provisional.symbol.clone();
    let draft = provisional
        .with_operation_outputs(plan.outputs.iter().map(|output| output.output).collect());
    let outputs =
        operation_semantic_outputs(builder, declaration, operation, &alias, &draft, &plan)?;
    builder.operation_plans.push(PreparedCodeOperationPlan {
        symbol: operation_symbol,
        prefix_aliases,
        plan,
    });
    builder.operation_plan_prefix_aliases = total_prefix_aliases;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    let root = named_declaration_root(descriptor.declaration, &alias, &outputs);
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root,
            paths: outputs,
        },
    )
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one operation result projector keeps native plan order, clean paths, aliased topology, and span cardinality adjacent"
)]
fn operation_semantic_outputs(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    operation: OperationKind,
    alias: &IntentKey,
    draft: &IntentNodeDraft,
    plan: &PreparedIntentOperationPlan,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    let native =
        draft
            .output_descriptors()
            .map_err(|error| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("authenticated operation result descriptor is invalid: {error}"),
            })?;
    let mut outputs = BTreeMap::new();
    insert_path(
        &mut outputs,
        fields_path(&["operation"]),
        SemanticValue::Port(port(
            alias,
            node_selector(IntentPortRole::Operation, 0),
            IntentPortKind::Operation,
        )),
        declaration,
    )?;

    if matches!(
        operation,
        OperationKind::Split | OperationKind::Break | OperationKind::Trim | OperationKind::Extend
    ) {
        publish_retained_operation_topology(builder, declaration, operation, &mut outputs)?;
    }

    let mut span_index = 0_u16;
    for (ordinal, prepared) in plan.outputs.iter().enumerate() {
        let result_index =
            u16::try_from(ordinal).map_err(|_| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: "operation output count exceeds the native selector bound".into(),
            })?;
        let result = native
            .iter()
            .find(|output| output.selector == node_selector(IntentPortRole::Result, result_index))
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("operation output {ordinal} has no native result selector"),
            })?;
        let expected_kind = operation_output_port_kind(prepared.output.kind);
        if !prepared.role.accepts_output_kind(prepared.output.kind) {
            return invalid_declaration(
                declaration,
                format!(
                    "operation output {ordinal} has a semantic role incompatible with {:?}",
                    prepared.output.kind
                ),
            );
        }
        if result.kind != expected_kind {
            return invalid_declaration(
                declaration,
                format!(
                    "operation output {ordinal} is {:?}, not the authenticated {:?}",
                    result.kind, expected_kind
                ),
            );
        }
        let path = clean_operation_output_path(
            builder,
            declaration,
            operation,
            &prepared.path,
            prepared.role,
        )?;
        insert_path(
            &mut outputs,
            path.clone(),
            SemanticValue::Port(port(alias, result.selector, result.kind)),
            declaration,
        )?;

        if prepared.output.kind == IntentOperationOutputKind::Curve {
            for local_span in 0..prepared.output.curve_span_count {
                let selector = node_selector(IntentPortRole::Span, span_index);
                let span = native
                    .iter()
                    .find(|output| output.selector == selector)
                    .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!(
                            "operation curve output {ordinal} is missing native span {local_span}"
                        ),
                    })?;
                if span.kind != IntentPortKind::CurveSpan {
                    return invalid_declaration(
                        declaration,
                        format!("operation span selector {span_index} has the wrong kind"),
                    );
                }
                let clean_span = clean_operation_span_path(
                    operation,
                    &path,
                    local_span,
                    prepared.output.curve_span_count,
                );
                insert_path(
                    &mut outputs,
                    clean_span,
                    SemanticValue::Port(port(alias, selector, IntentPortKind::CurveSpan)),
                    declaration,
                )?;
                span_index = span_index.checked_add(1).ok_or_else(|| {
                    CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: "operation span count exceeds the native selector bound".into(),
                    }
                })?;
            }
        }
    }
    if native.iter().any(|output| {
        matches!(
            output.selector,
            IntentPortSelector::Node {
                role: IntentPortRole::Result,
                ..
            }
        ) && !plan.outputs.iter().enumerate().any(|(ordinal, _)| {
            u16::try_from(ordinal)
                .is_ok_and(|index| output.selector == node_selector(IntentPortRole::Result, index))
        })
    }) {
        return invalid_declaration(
            declaration,
            "native operation descriptor contains an unauthenticated result".into(),
        );
    }
    let span_descriptors = native
        .iter()
        .filter(|output| {
            matches!(
                output.selector,
                IntentPortSelector::Node {
                    role: IntentPortRole::Span,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    if span_descriptors.len() != usize::from(span_index)
        || span_descriptors.iter().any(|output| {
            let IntentPortSelector::Node { index, .. } = output.selector else {
                unreachable!("span descriptor filter admits only node selectors")
            };
            index >= span_index || output.kind != IntentPortKind::CurveSpan
        })
    {
        return invalid_declaration(
            declaration,
            "native operation descriptor contains an unauthenticated span".into(),
        );
    }
    Ok(outputs)
}

const fn operation_output_port_kind(kind: IntentOperationOutputKind) -> IntentPortKind {
    match kind {
        IntentOperationOutputKind::Point => IntentPortKind::Point,
        IntentOperationOutputKind::Scalar => IntentPortKind::Scalar,
        IntentOperationOutputKind::Curve => IntentPortKind::Curve,
        IntentOperationOutputKind::Contact => IntentPortKind::Contact,
        IntentOperationOutputKind::Constraint => IntentPortKind::Constraint,
        IntentOperationOutputKind::Dimension => IntentPortKind::Dimension,
        IntentOperationOutputKind::Parameter => IntentPortKind::Parameter,
        IntentOperationOutputKind::ExternalBinding => IntentPortKind::ExternalBinding,
    }
}

fn publish_retained_operation_topology(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    operation: OperationKind,
    outputs: &mut BTreeMap<SemanticOutputPath, SemanticValue>,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let source = required(arguments, "source", &declaration.symbol.0)?;
    let span = builder
        .resolve_managed(
            source,
            &SemanticOutputPath::default(),
            &format!("{}.source", declaration.symbol.0),
        )?
        .as_port(
            IntentPortKind::CurveSpan,
            &format!("{}.source", declaration.symbol.0),
        )?;
    let span_value = SemanticValue::Port(span.clone());
    match operation {
        OperationKind::Split => {
            insert_path(
                outputs,
                fields_path(&["before"]),
                span_value.clone(),
                declaration,
            )?;
            insert_path(outputs, fields_path(&["after"]), span_value, declaration)?;
        }
        OperationKind::Break => {
            insert_path(
                outputs,
                fields_path(&["before"]),
                span_value.clone(),
                declaration,
            )?;
            insert_path(
                outputs,
                fields_path(&["middle"]),
                span_value.clone(),
                declaration,
            )?;
            insert_path(outputs, fields_path(&["after"]), span_value, declaration)?;
        }
        OperationKind::Trim => {
            insert_path(outputs, fields_path(&["retained"]), span_value, declaration)?;
        }
        OperationKind::Extend => {
            let curve = source_curve_for_span(builder, source, &span, declaration)?;
            insert_path(
                outputs,
                fields_path(&["curve"]),
                SemanticValue::Port(curve),
                declaration,
            )?;
            insert_path(outputs, fields_path(&["span"]), span_value, declaration)?;
        }
        OperationKind::Mirror
        | OperationKind::Chamfer
        | OperationKind::AssociativeFillet
        | OperationKind::Rectangle
        | OperationKind::RegularPolygon
        | OperationKind::Slot
        | OperationKind::LinearPattern
        | OperationKind::ProfileOffset => {}
    }
    Ok(())
}

fn source_curve_for_span(
    builder: &ExpansionBuilder,
    source: &ManagedValue,
    span: &ExpandedPort,
    declaration: &AuthoringDeclaration,
) -> Result<ExpandedPort, CodeExpansionError> {
    let ManagedValue::Reference {
        declaration: owner, ..
    } = source
    else {
        return invalid_declaration(
            declaration,
            "retained operation source span has no semantic owner".into(),
        );
    };
    let Some(owner) = builder.declarations.get(owner) else {
        return invalid_declaration(
            declaration,
            "retained operation source declaration is unavailable".into(),
        );
    };
    let mut matches = owner.paths.values().filter_map(|value| match value {
        SemanticValue::Port(port)
            if port.kind == IntentPortKind::Curve && port.alias == span.alias =>
        {
            Some(port.clone())
        }
        _ => None,
    });
    let Some(curve) = matches.next() else {
        return invalid_declaration(
            declaration,
            "retained operation source span has no corresponding curve result".into(),
        );
    };
    if matches.any(|candidate| candidate != curve) {
        return invalid_declaration(
            declaration,
            "retained operation source span has ambiguous curve ownership".into(),
        );
    }
    Ok(curve)
}

fn clean_operation_output_path(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    operation: OperationKind,
    native: &[PreparedIntentOperationPathSegment],
    role: PreparedIntentOperationOutputRole,
) -> Result<SemanticOutputPath, CodeExpansionError> {
    let mut clean = Vec::with_capacity(native.len());
    let mut index = 0;
    while index < native.len() {
        if operation == OperationKind::Mirror
            && matches!(native.get(index), Some(PreparedIntentOperationPathSegment::Field(name)) if name == "curves")
            && matches!(
                native.get(index + 1),
                Some(PreparedIntentOperationPathSegment::SourceCurve { .. })
            )
        {
            clean.push(ManagedPathSegment::Field("curve".into()));
            index += 2;
            continue;
        }
        match &native[index] {
            PreparedIntentOperationPathSegment::Field(name) => {
                clean.push(ManagedPathSegment::Field(
                    if role == PreparedIntentOperationOutputRole::DimensionTarget
                        && name == "target"
                    {
                        "value".into()
                    } else {
                        name.clone()
                    },
                ));
            }
            PreparedIntentOperationPathSegment::Index(native_index) => {
                let public_index = if operation == OperationKind::LinearPattern
                    && matches!(clean.last(), Some(ManagedPathSegment::Field(name)) if name == "instances")
                {
                    native_index.checked_sub(1).ok_or_else(|| {
                        CodeExpansionError::OperationPlanning {
                            declaration: declaration.symbol.0.clone(),
                            message: "native pattern instance numbering must begin at one".into(),
                        }
                    })?
                } else {
                    *native_index
                };
                clean.push(ManagedPathSegment::Index(public_index));
            }
            PreparedIntentOperationPathSegment::SourceControl { source } => {
                clean.push(ManagedPathSegment::Member {
                    member: operation_source_member_key(
                        builder,
                        declaration,
                        source,
                        &[IntentPortKind::Point],
                    )?,
                });
            }
            PreparedIntentOperationPathSegment::SourceCurve { source } => {
                clean.push(ManagedPathSegment::Member {
                    member: operation_source_member_key(
                        builder,
                        declaration,
                        source,
                        &[IntentPortKind::Curve],
                    )?,
                });
            }
            PreparedIntentOperationPathSegment::SourceSpan { source } => {
                clean.push(ManagedPathSegment::Member {
                    member: operation_source_member_key(
                        builder,
                        declaration,
                        source,
                        &[IntentPortKind::CurveSpan],
                    )?,
                });
            }
            PreparedIntentOperationPathSegment::SourceJunction { source } => {
                clean.push(ManagedPathSegment::Member {
                    member: operation_source_member_key(
                        builder,
                        declaration,
                        source,
                        &[IntentPortKind::Point, IntentPortKind::Constraint],
                    )?,
                });
            }
            _ => {
                return Err(CodeExpansionError::OperationPlanning {
                    declaration: declaration.symbol.0.clone(),
                    message: "native planner returned an unsupported source-relative path segment"
                        .into(),
                });
            }
        }
        index += 1;
    }
    if clean.is_empty() {
        return invalid_declaration(
            declaration,
            "native operation produced an empty clean result path".into(),
        );
    }
    Ok(SemanticOutputPath(clean))
}

fn operation_source_member_key(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    source: &PreparedIntentOperationSourceRef,
    expected: &[IntentPortKind],
) -> Result<String, CodeExpansionError> {
    if !expected.contains(&source.kind) {
        return Err(CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message: format!(
                "native operation result source is {:?}, not one of {expected:?}",
                source.kind
            ),
        });
    }
    let owner = builder
        .declaration_provenance
        .get(&source.declaration)
        .ok_or_else(|| CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message: format!(
                "native operation result source `{}` has no managed declaration owner",
                source.declaration
            ),
        })?;
    let lowered =
        builder
            .declarations
            .get(owner)
            .ok_or_else(|| CodeExpansionError::OperationPlanning {
                declaration: declaration.symbol.0.clone(),
                message: format!(
                    "native operation result source `{}` is not yet available",
                    source.declaration
                ),
            })?;
    let mut paths = lowered
        .paths
        .iter()
        .filter_map(|(path, value)| match value {
            SemanticValue::Port(port)
                if port.alias == source.declaration
                    && port.selector == source.selector
                    && port.kind == source.kind =>
            {
                Some(path)
            }
            _ => None,
        });
    let path = paths
        .next()
        .ok_or_else(|| CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message: format!(
                "native operation result source `{}` has no exact managed output path",
                source.declaration
            ),
        })?;
    if paths.next().is_some() {
        return Err(CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message: format!(
                "native operation result source `{}` has ambiguous managed output paths",
                source.declaration
            ),
        });
    }
    let key = structured_reference_text(owner, path);
    if key.len() > 256 {
        return Err(CodeExpansionError::OperationPlanning {
            declaration: declaration.symbol.0.clone(),
            message:
                "native operation result source reference exceeds the managed member-key bound"
                    .into(),
        });
    }
    Ok(key)
}

fn structured_reference_text(declaration: &SemanticSymbol, path: &SemanticOutputPath) -> String {
    if is_reference_identifier(&declaration.0)
        && path.0.iter().all(|segment| {
            matches!(segment, ManagedPathSegment::Field(field) if is_reference_identifier(field))
        })
    {
        return std::iter::once(declaration.0.as_str())
            .chain(path.0.iter().filter_map(|segment| match segment {
                ManagedPathSegment::Field(field) => Some(field.as_str()),
                ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. } => None,
            }))
            .collect::<Vec<_>>()
            .join(".");
    }

    // This reversible tagged spelling is display only; the structured source
    // reference remains the authenticated identity. Segment tags keep a user
    // member named `a.b` distinct from fields `a` then `b` without opaque
    // hashes or allocation ordinals.
    let mut parts = vec![format!("d-{}", encode_reference_component(&declaration.0))];
    parts.extend(path.0.iter().map(|segment| match segment {
        ManagedPathSegment::Field(field) => {
            format!("f-{}", encode_reference_component(field))
        }
        ManagedPathSegment::Index(value) => format!("i-{value}"),
        ManagedPathSegment::Member { member } => {
            format!("m-{}", encode_reference_component(member))
        }
    }));
    format!("ref.{}", parts.join("."))
}

fn encode_reference_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => encoded.push(char::from(byte)),
            b'_' => encoded.push_str("_u"),
            b'-' => encoded.push_str("_h"),
            b'.' => encoded.push_str("_d"),
            _ => {
                use std::fmt::Write as _;
                write!(&mut encoded, "_x{byte:02x}").expect("writing into a String is infallible");
            }
        }
    }
    encoded
}

fn is_reference_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn clean_operation_span_path(
    operation: OperationKind,
    curve: &SemanticOutputPath,
    ordinal: u16,
    count: u16,
) -> SemanticOutputPath {
    let mut path = curve.clone();
    match operation {
        OperationKind::Mirror | OperationKind::Chamfer | OperationKind::AssociativeFillet => {
            path = fields_path(&["span"]);
        }
        OperationKind::Rectangle | OperationKind::RegularPolygon => {
            if matches!(path.0.first(), Some(ManagedPathSegment::Field(name)) if name == "edges") {
                path.0[0] = ManagedPathSegment::Field("spans".into());
            }
        }
        OperationKind::Slot => {
            if matches!(path.0.first(), Some(ManagedPathSegment::Field(name)) if name == "edges") {
                path.0[0] = ManagedPathSegment::Field("spans".into());
            } else if matches!(path.0.last(), Some(ManagedPathSegment::Field(name)) if name == "curve")
            {
                *path.0.last_mut().expect("nonempty curve path") =
                    ManagedPathSegment::Field("span".into());
            }
        }
        OperationKind::LinearPattern | OperationKind::ProfileOffset => {
            if matches!(path.0.last(), Some(ManagedPathSegment::Field(name)) if name == "curve") {
                *path.0.last_mut().expect("nonempty curve path") =
                    ManagedPathSegment::Field("span".into());
            } else {
                path.0.push(ManagedPathSegment::Field("span".into()));
            }
        }
        OperationKind::Split
        | OperationKind::Break
        | OperationKind::Trim
        | OperationKind::Extend => {
            path.0.push(ManagedPathSegment::Field("span".into()));
        }
    }
    if count > 1 {
        if let Some(ManagedPathSegment::Field(name)) = path.0.last_mut()
            && name == "span"
        {
            *name = "spans".into();
        }
        path.0.push(ManagedPathSegment::Index(usize::from(ordinal)));
    }
    path
}

fn named_declaration_root(
    declaration: CodeAuthoringDeclarationKind,
    alias: &IntentKey,
    outputs: &BTreeMap<SemanticOutputPath, SemanticValue>,
) -> SemanticValue {
    let preferred = match declaration {
        CodeAuthoringDeclarationKind::Constraint(_) => Some("constraint"),
        CodeAuthoringDeclarationKind::Dimension(_) => Some("dimension"),
        CodeAuthoringDeclarationKind::Operation(_) => Some("operation"),
        CodeAuthoringDeclarationKind::Aggregate(AggregateKind::OpenChain) => Some("chain"),
        CodeAuthoringDeclarationKind::Aggregate(AggregateKind::ClosedProfile) => Some("profile"),
        CodeAuthoringDeclarationKind::Geometry(_)
        | CodeAuthoringDeclarationKind::ComputedFeature(_) => None,
    };
    if let Some(value) = preferred.and_then(|path| outputs.get(&fields_path(&[path]))) {
        return value.clone();
    }
    SemanticValue::Declaration {
        alias: alias.clone(),
        kind: FeatureKind::Feature,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed binding dispatch keeps every catalog-owned input mode auditable"
)]
fn lower_named_input(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    draft: &mut IntentNodeDraft,
    arguments: &BTreeMap<String, ManagedValue>,
    input: &crate::declaration_catalog::CodeAuthoringInputDescriptor,
) -> Result<(), CodeExpansionError> {
    let Some(value) = arguments.get(&input.name) else {
        if input.minimum == 0 {
            return Ok(());
        }
        return invalid_declaration(
            declaration,
            format!("missing required named input `{}`", input.name),
        );
    };
    match (&input.binding, &input.kind) {
        (CodeAuthoringInputBinding::Slot { slot }, kind) => {
            let port = resolve_named_reference(
                builder,
                value,
                kind,
                &format!("{}.{}", declaration.symbol.0, input.name),
            )?;
            *draft = draft.clone().with_input(*slot, port.patch_ref());
        }
        (
            CodeAuthoringInputBinding::Role {
                role: InputRole::Span,
            },
            CodeAuthoringArgumentKind::Collection(CodeAuthoringCollectionMember::FilletParent),
        ) => {
            let parents = array(value, "associative Fillet parents")?;
            if parents.len() != 2 {
                return invalid_declaration(
                    declaration,
                    "associative Fillet requires exactly two parents".into(),
                );
            }
            for (index, parent) in parents.iter().enumerate() {
                let parent = object(parent, "associative Fillet parent")?;
                let span = resolve_named_reference(
                    builder,
                    required(parent, "span", "associative Fillet parent")?,
                    &CodeAuthoringArgumentKind::Reference(IntentPortKind::CurveSpan),
                    &format!("{}.parents[{index}].span", declaration.symbol.0),
                )?;
                let index = u16::try_from(index).expect("two Fillet parents fit u16");
                *draft = draft
                    .clone()
                    .with_input(InputSlot::new(InputRole::Span, index), span.patch_ref());
            }
        }
        (
            CodeAuthoringInputBinding::Role { role },
            CodeAuthoringArgumentKind::Collection(member),
        ) => {
            let values = array(value, &format!("{} collection", input.name))?;
            if values.len() < usize::from(input.minimum)
                || values.len() > usize::from(input.maximum)
            {
                return invalid_declaration(
                    declaration,
                    format!("named input `{}` has invalid cardinality", input.name),
                );
            }
            for (index, value) in values.iter().enumerate() {
                let kind = collection_reference_kind(member).ok_or_else(|| {
                    CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!(
                            "named input `{}` is not a reference collection",
                            input.name
                        ),
                    }
                })?;
                let port = resolve_named_reference(
                    builder,
                    value,
                    &kind,
                    &format!("{}.{}[{index}]", declaration.symbol.0, input.name),
                )?;
                let index =
                    u16::try_from(index).map_err(|_| CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!("named input `{}` exceeds the slot bound", input.name),
                    })?;
                *draft = draft
                    .clone()
                    .with_input(InputSlot::new(*role, index), port.patch_ref());
            }
        }
        (CodeAuthoringInputBinding::Roles { roles }, kind) => {
            let values: Vec<&ManagedValue> = match kind {
                CodeAuthoringArgumentKind::Collection(_) => {
                    array(value, &format!("{} collection", input.name))?
                        .iter()
                        .collect()
                }
                _ => vec![value],
            };
            let mut role_counts = BTreeMap::<InputRole, u16>::new();
            for (index, value) in values.into_iter().enumerate() {
                let element_kind = match kind {
                    CodeAuthoringArgumentKind::Collection(member) => {
                        collection_reference_kind(member).ok_or_else(|| {
                            CodeExpansionError::InvalidDeclaration {
                                declaration: declaration.symbol.0.clone(),
                                message: format!(
                                    "named input `{}` is not a reference collection",
                                    input.name
                                ),
                            }
                        })?
                    }
                    other => other.clone(),
                };
                let port = resolve_named_reference(
                    builder,
                    value,
                    &element_kind,
                    &format!("{}.{}[{index}]", declaration.symbol.0, input.name),
                )?;
                let role = roles
                    .iter()
                    .copied()
                    .find(|role| role.expected_kind() == Some(port.kind))
                    .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!(
                            "named input `{}` has no role for {:?}",
                            input.name, port.kind
                        ),
                    })?;
                let slot_index = role_counts.entry(role).or_default();
                *draft = draft
                    .clone()
                    .with_input(InputSlot::new(role, *slot_index), port.patch_ref());
                *slot_index = slot_index.checked_add(1).ok_or_else(|| {
                    CodeExpansionError::InvalidDeclaration {
                        declaration: declaration.symbol.0.clone(),
                        message: format!("named input `{}` exceeds the slot bound", input.name),
                    }
                })?;
            }
        }
        (
            CodeAuthoringInputBinding::DynamicChildren {
                schema: geosolve_sketch_intent::IntentChildSchema::FilletCorner,
            },
            CodeAuthoringArgumentKind::Collection(CodeAuthoringCollectionMember::FilletCorner),
        ) => {
            let corners = array(value, "Fillet corners")?;
            for (corner_index, corner) in corners.iter().enumerate() {
                let corner = object(corner, "Fillet corner")?;
                let parents = array(
                    required(corner, "parents", "Fillet corner")?,
                    "Fillet parents",
                )?;
                let [first, second] = parents else {
                    return invalid_declaration(
                        declaration,
                        "each Fillet corner must have exactly two parents".into(),
                    );
                };
                for (parent_index, parent) in [first, second].into_iter().enumerate() {
                    let parent = object(parent, "Fillet parent")?;
                    let span = required(parent, "span", "Fillet parent")?;
                    let port = resolve_named_reference(
                        builder,
                        span,
                        &CodeAuthoringArgumentKind::Reference(IntentPortKind::CurveSpan),
                        &format!(
                            "{}.corners[{corner_index}].parents[{parent_index}].span",
                            declaration.symbol.0
                        ),
                    )?;
                    let slot = corner_index
                        .checked_mul(2)
                        .and_then(|value| value.checked_add(parent_index))
                        .and_then(|value| u16::try_from(value).ok())
                        .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                            declaration: declaration.symbol.0.clone(),
                            message: "Fillet parent slot exceeds the native bound".into(),
                        })?;
                    *draft = draft
                        .clone()
                        .with_input(InputSlot::new(InputRole::Span, slot), port.patch_ref());
                }
            }
        }
        (CodeAuthoringInputBinding::DynamicChildren { .. }, _) => {
            return invalid_declaration(
                declaration,
                format!(
                    "named dynamic input `{}` requires family-owned lowering",
                    input.name
                ),
            );
        }
        _ => {
            return invalid_declaration(
                declaration,
                format!(
                    "named input `{}` disagrees with the Rust catalog",
                    input.name
                ),
            );
        }
    }
    Ok(())
}

fn collection_reference_kind(
    member: &CodeAuthoringCollectionMember,
) -> Option<CodeAuthoringArgumentKind> {
    match member {
        CodeAuthoringCollectionMember::Reference(kind) => {
            Some(CodeAuthoringArgumentKind::Reference(*kind))
        }
        CodeAuthoringCollectionMember::ReferenceChoice(kinds) => {
            Some(CodeAuthoringArgumentKind::ReferenceChoice(kinds.clone()))
        }
        CodeAuthoringCollectionMember::Point
        | CodeAuthoringCollectionMember::SplineControl
        | CodeAuthoringCollectionMember::FilletCorner
        | CodeAuthoringCollectionMember::FilletParent => None,
    }
}

fn resolve_named_reference(
    builder: &ExpansionBuilder,
    value: &ManagedValue,
    kind: &CodeAuthoringArgumentKind,
    label: &str,
) -> Result<ExpandedPort, CodeExpansionError> {
    let resolved = builder.resolve_managed(value, &SemanticOutputPath::default(), label)?;
    match kind {
        CodeAuthoringArgumentKind::Reference(kind) => resolved.as_port(*kind, label),
        CodeAuthoringArgumentKind::ReferenceChoice(kinds) => kinds
            .iter()
            .find_map(|kind| resolved.as_port(*kind, label).ok())
            .ok_or_else(|| CodeExpansionError::KindMismatch {
                reference: label.to_owned(),
                expected: kinds
                    .first()
                    .copied()
                    .map_or(FeatureKind::Feature, feature_kind_for_port),
                actual: resolved.kind(),
            }),
        CodeAuthoringArgumentKind::Point => resolved.as_port(IntentPortKind::Point, label),
        CodeAuthoringArgumentKind::Collection(_) => Err(CodeExpansionError::InvalidDeclaration {
            declaration: label.to_owned(),
            message: "a collection cannot bind one native input slot".into(),
        }),
    }
}

fn named_projection_value(
    arguments: &BTreeMap<String, ManagedValue>,
    path: &IntentProjectionPath,
) -> Option<ManagedValue> {
    let mut current = ManagedValue::Object(arguments.clone());
    for segment in path.segments() {
        current = match (segment, current) {
            (IntentProjectionPathSegment::Field(field), ManagedValue::Object(values)) => {
                let name = field.as_str();
                if let Some(value) = values.get(name) {
                    value.clone()
                } else if name == "name" {
                    values.get("label")?.clone()
                } else if name == "anchor" {
                    values.get("periodicAnchor")?.clone()
                } else if name == "enabled" {
                    match values.get("kind")? {
                        ManagedValue::String(kind) if kind == "anchor" => ManagedValue::Bool(true),
                        ManagedValue::String(kind) if kind == "none" => ManagedValue::Bool(false),
                        _ => return None,
                    }
                } else {
                    return None;
                }
            }
            (IntentProjectionPathSegment::Index(index), ManagedValue::Array(values)) => {
                values.get(usize::from(*index))?.clone()
            }
            (IntentProjectionPathSegment::Index(index), ManagedValue::Object(values)) => {
                let key = match index {
                    0 => "first",
                    1 => "second",
                    _ => return None,
                };
                values.get(key)?.clone()
            }
            _ => return None,
        };
    }
    Some(current)
}

fn named_literal(
    builder: &ExpansionBuilder,
    value: &ManagedValue,
    schema: IntentLiteralSchema,
    label: &str,
) -> Result<IntentLiteral, CodeExpansionError> {
    match schema {
        IntentLiteralSchema::Boolean => Ok(IntentLiteral::Boolean(boolean(value, label)?)),
        IntentLiteralSchema::Integer => {
            let value = finite_number(value, label)?;
            if value.fract() != 0.0
                || !(-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&value)
            {
                return Err(CodeExpansionError::Unsupported(format!(
                    "`{label}` must be an exactly representable source integer"
                )));
            }
            #[allow(
                clippy::cast_possible_truncation,
                reason = "integrality and the exact i64 range are checked immediately above"
            )]
            Ok(IntentLiteral::Integer(value as i64))
        }
        IntentLiteralSchema::Natural => {
            let value = finite_number(value, label)?;
            if value.fract() != 0.0 || !(0.0..=9_007_199_254_740_991.0).contains(&value) {
                return Err(CodeExpansionError::Unsupported(format!(
                    "`{label}` must be an exactly representable source natural number"
                )));
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "integrality and the exact u64 range are checked immediately above"
            )]
            Ok(IntentLiteral::Natural(value as u64))
        }
        IntentLiteralSchema::Text => {
            Ok(IntentLiteral::Text(IntentKey::new(string(value, label)?)?))
        }
        IntentLiteralSchema::Enum => Ok(IntentLiteral::Enum(IntentKey::new(camel_to_snake(
            string(value, label)?,
        ))?)),
        IntentLiteralSchema::Point => Ok(IntentLiteral::Point(point(value, label)?)),
        IntentLiteralSchema::Quantity(unit) => {
            let scalar = match value {
                ManagedValue::Reference { .. } => {
                    let resolved =
                        builder.resolve_managed(value, &SemanticOutputPath::default(), label)?;
                    let SemanticValue::ScalarLiteral(value) = resolved else {
                        return Err(CodeExpansionError::KindMismatch {
                            reference: label.to_owned(),
                            expected: FeatureKind::Scalar,
                            actual: resolved.kind(),
                        });
                    };
                    value
                }
                ManagedValue::Unit(value) => value.clone(),
                ManagedValue::Number(value) if value.is_finite() => UnitLiteral {
                    unit: "model".into(),
                    value: *value,
                },
                _ => {
                    return Err(CodeExpansionError::Unsupported(format!(
                        "`{label}` must be a finite typed quantity"
                    )));
                }
            };
            Ok(IntentLiteral::Quantity {
                value: convert_named_unit(&scalar, unit, label)?,
                unit,
            })
        }
    }
}

fn convert_named_unit(
    value: &UnitLiteral,
    expected: IntentUnit,
    label: &str,
) -> Result<f64, CodeExpansionError> {
    let converted = match (expected, value.unit.as_str()) {
        (IntentUnit::Length, "model" | "mm")
        | (IntentUnit::Angle, "model" | "rad")
        | (IntentUnit::Dimensionless, "model") => value.value,
        (IntentUnit::Length, "cm") => value.value * 10.0,
        (IntentUnit::Length, "m") => value.value * 1_000.0,
        (IntentUnit::Length, "inch") => value.value * 25.4,
        (IntentUnit::Angle, "deg") => value.value.to_radians(),
        _ => {
            return Err(CodeExpansionError::Unsupported(format!(
                "`{label}` has unit `{}` incompatible with {expected:?}",
                value.unit
            )));
        }
    };
    if !converted.is_finite() {
        return Err(CodeExpansionError::Unsupported(format!(
            "`{label}` converts to a non-finite quantity"
        )));
    }
    Ok(converted)
}

fn camel_to_snake(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(character.to_ascii_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

fn semantic_output_path(path: &IntentProjectionPath) -> SemanticOutputPath {
    SemanticOutputPath(
        path.segments()
            .iter()
            .map(|segment| match segment {
                IntentProjectionPathSegment::Field(field) => {
                    ManagedPathSegment::Field(field.as_str().to_owned())
                }
                IntentProjectionPathSegment::Index(index) => {
                    ManagedPathSegment::Index(usize::from(*index))
                }
            })
            .collect(),
    )
}

fn lower_remaining_named_declarations(
    project: &CodeProject,
    builder: &mut ExpansionBuilder,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<(), CodeExpansionError> {
    for declaration in &project.managed.program.declarations {
        if declaration.patch.is_some() || builder.declarations.contains_key(&declaration.symbol) {
            continue;
        }
        lower_named_declaration(
            builder,
            declaration,
            Some(reconciliation),
            overlay,
            operation_planner,
        )?;
    }
    Ok(())
}

fn pinned_artifacts(project: &CodeProject) -> Result<Vec<PinnedArtifact>, CodeExpansionError> {
    project
        .artifacts
        .iter()
        .map(|(digest, value)| {
            let artifact: PatchModuleArtifact = serde_json::from_value(value.clone())
                .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
            Ok(PinnedArtifact {
                digest: digest.clone(),
                artifact: artifact.validate()?,
            })
        })
        .collect()
}

fn resolve_pinned_artifact(
    project: &CodeProject,
    artifacts: &[PinnedArtifact],
    binding: &str,
) -> Result<PinnedArtifact, CodeExpansionError> {
    let modules = project
        .managed
        .imports
        .iter()
        .filter(|import| import.bindings.iter().any(|candidate| candidate == binding))
        .map(|import| import.module.as_str())
        .collect::<BTreeSet<_>>();
    let matches = artifacts
        .iter()
        .filter(|candidate| {
            candidate.artifact.artifact().export_name == binding
                && modules.contains(candidate.artifact.artifact().module_specifier.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(CodeExpansionError::MissingArtifact {
            binding: binding.to_owned(),
        }),
        [only] => Ok(only.clone()),
        _ => Err(CodeExpansionError::AmbiguousArtifact {
            binding: binding.to_owned(),
        }),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one lowering keeps endpoint references, detached seeds, aliases, and writable provenance auditable together"
)]
fn lower_direct_line(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let alias = builder.lowering_alias("decl", &declaration.symbol, &[])?;
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        alias.clone(),
    );
    if let Some(role) = arguments.get("role") {
        let role = string(role, "line role")?;
        if !matches!(role, "profile" | "construction") {
            return invalid_declaration(
                declaration,
                "line role must be `profile` or `construction`".into(),
            );
        }
        draft = draft.with_field(field_key("role")?, enum_value(role)?);
    }
    let mut endpoint_positions = [None, None];
    let mut writable = Vec::new();
    for (index, (name, role)) in [
        ("start", IntentPortRole::Start),
        ("end", IntentPortRole::End),
    ]
    .into_iter()
    .enumerate()
    {
        let value = required(arguments, name, &declaration.symbol.0)?;
        let reference = format!("{}.{}", declaration.symbol.0, name);
        let address = builder.lowering_point_address(
            declaration,
            "line",
            reconciliation,
            fields_path(&[name]),
        );
        let source = match value {
            ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
                declaration: declaration.clone(),
                path: path.clone(),
            },
            _ => CodePointSeedSource::Literal,
        };
        let drafted = address
            .as_ref()
            .and_then(|address| overlay_point(overlay, address));
        let resolved = if let Some(position) = drafted {
            SemanticValue::PointLiteral(position)
        } else {
            builder.resolve_managed(value, &SemanticOutputPath::default(), &reference)?
        };
        endpoint_positions[index] = builder.point_seed(&resolved);
        match resolved {
            SemanticValue::PointLiteral(position) => {
                let selector = node_selector(role, 0);
                draft = draft
                    .with_instance_leaf(selector, LeafField::X, length(position[0]))
                    .with_instance_leaf(selector, LeafField::Y, length(position[1]));
            }
            value => {
                let point = value.as_port(IntentPortKind::Point, &reference)?;
                draft = draft.with_input(
                    InputSlot::new(
                        InputRole::Point,
                        u16::try_from(index).expect("two endpoints"),
                    ),
                    point.patch_ref(),
                );
            }
        }
        if let Some(address) = address {
            writable.push((
                ExpandedPort {
                    alias: alias.clone(),
                    selector: node_selector(role, 0),
                    kind: IntentPortKind::Point,
                },
                source,
                address,
            ));
        }
    }
    let [Some(start), Some(end)] = endpoint_positions else {
        return Err(CodeExpansionError::Unsupported(format!(
            "line `{}` has no deterministic endpoint seed for explicit branch direction",
            declaration.symbol.0
        )));
    };
    let direction =
        unit_direction(start, end).ok_or_else(|| CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: "line endpoints must be finite and distinct".into(),
        })?;
    draft = draft.with_field(
        field_key("branch_direction")?,
        IntentLiteral::Point(direction),
    );
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    for (role, position) in [IntentPortRole::Start, IntentPortRole::End]
        .into_iter()
        .zip([start, end])
    {
        builder.add_point_seed(
            &port(&alias, node_selector(role, 0), IntentPortKind::Point),
            position,
        )?;
    }
    for (handle, source, address) in writable {
        builder.add_writable_point(ExpandedWritablePoint {
            handle,
            source,
            edit: CodePointEdit::Point { address },
        });
    }

    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias: alias.clone(),
                kind: FeatureKind::Feature,
            },
            paths: BTreeMap::from([
                (
                    fields_path(&["curve"]),
                    SemanticValue::Port(port(
                        &alias,
                        node_selector(IntentPortRole::Curve, 0),
                        IntentPortKind::Curve,
                    )),
                ),
                (
                    fields_path(&["end"]),
                    SemanticValue::Port(port(
                        &alias,
                        node_selector(IntentPortRole::End, 0),
                        IntentPortKind::Point,
                    )),
                ),
                (
                    fields_path(&["span"]),
                    SemanticValue::Port(port(
                        &alias,
                        node_selector(IntentPortRole::Span, 0),
                        IntentPortKind::CurveSpan,
                    )),
                ),
                (
                    fields_path(&["start"]),
                    SemanticValue::Port(port(
                        &alias,
                        node_selector(IntentPortRole::Start, 0),
                        IntentPortKind::Point,
                    )),
                ),
            ]),
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "direct circle lowering keeps center-reference, overlay, seed, and writable provenance together"
)]
fn lower_direct_circle(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    if arguments
        .keys()
        .any(|key| !matches!(key.as_str(), "center" | "radius" | "label" | "role"))
        || !arguments.contains_key("center")
        || !arguments.contains_key("radius")
    {
        return Err(CodeExpansionError::Unsupported(format!(
            "`{}` requires `center` and `radius` and accepts only optional `label` and `role`",
            declaration.symbol.0
        )));
    }
    let radius = resolved_direct_length_value(
        builder,
        required(arguments, "radius", &declaration.symbol.0)?,
        "circle radius",
    )?;
    if radius <= 0.0 {
        return invalid_declaration(
            declaration,
            "circle radius must be finite and positive".into(),
        );
    }

    let center_value = required(arguments, "center", &declaration.symbol.0)?;
    let center_reference = format!("{}.center", declaration.symbol.0);
    let center_address = builder.lowering_point_address(
        declaration,
        "circle",
        reconciliation,
        fields_path(&["center"]),
    );
    let center_source = match center_value {
        ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
            declaration: declaration.clone(),
            path: path.clone(),
        },
        _ => CodePointSeedSource::Literal,
    };
    let drafted_center = center_address
        .as_ref()
        .and_then(|address| overlay_point(overlay, address));
    let resolved_center = if let Some(position) = drafted_center {
        SemanticValue::PointLiteral(position)
    } else {
        builder.resolve_managed(
            center_value,
            &SemanticOutputPath::default(),
            &center_reference,
        )?
    };
    let center_seed = builder.point_seed(&resolved_center).ok_or_else(|| {
        CodeExpansionError::Unsupported(format!(
            "circle `{}` has no deterministic center seed",
            declaration.symbol.0
        ))
    })?;

    let alias = builder.lowering_alias("decl", &declaration.symbol, &[])?;
    let center_selector = node_selector(IntentPortRole::Center, 0);
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::CenterRadiusCircle,
        },
        alias.clone(),
    )
    .with_instance_leaf(
        node_selector(IntentPortRole::Target, 0),
        LeafField::Value,
        length(radius),
    );
    if let Some(label) = arguments.get("label") {
        draft = draft.with_display_name(IntentKey::new(string(label, "circle label")?)?);
    }
    if let Some(role) = arguments.get("role") {
        let role = geometry_role(role, "circle role")?;
        draft = draft.with_field(
            field_key("role")?,
            enum_value(match role {
                GeometryRole::Profile => "profile",
                GeometryRole::Construction => "construction",
            })?,
        );
    }
    match resolved_center {
        SemanticValue::PointLiteral(position) => {
            draft = draft
                .with_instance_leaf(center_selector, LeafField::X, length(position[0]))
                .with_instance_leaf(center_selector, LeafField::Y, length(position[1]));
        }
        value => {
            let center = value.as_port(IntentPortKind::Point, &center_reference)?;
            draft = draft.with_input(InputSlot::new(InputRole::Point, 0), center.patch_ref());
        }
    }
    let outputs = draft_semantic_outputs(&draft, &alias)?;
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    let center = port(&alias, center_selector, IntentPortKind::Point);
    builder.add_point_seed(&center, center_seed)?;
    if let Some(address) = center_address {
        builder.add_writable_point(ExpandedWritablePoint {
            handle: center.clone(),
            source: center_source,
            edit: CodePointEdit::Point { address },
        });
    }

    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias: alias.clone(),
                kind: FeatureKind::Feature,
            },
            paths: outputs,
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one keyed Polyline lowering keeps point, span, aggregate and typed corner paths auditable together"
)]
fn lower_direct_polyline(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let definition = keyed_polyline_definition(declaration)?;
    if definition.representation == DirectPolylineRepresentation::SingleCurve {
        return lower_direct_single_curve_polyline(
            builder,
            declaration,
            &definition,
            reconciliation,
            overlay,
        );
    }
    let segment_count = if definition.closed {
        definition.vertices.len()
    } else {
        definition.vertices.len() - 1
    };
    let effective_vertices = definition
        .vertices
        .iter()
        .map(|(key, position)| {
            let position = point(position, "Polyline vertex position")?;
            let address = direct_polyline_address(&declaration.symbol, "vertex", key, "point");
            let identity = reconciliation.and_then(|state| state.active().get(&address).copied());
            let writable = identity.map(|identity| {
                CodeWritableAddress::generated_point(
                    builder.project.clone(),
                    address.clone(),
                    identity,
                    member_path(&["vertices"], key, &["position"]),
                )
            });
            let legacy = overridden_point(reconciliation, &address, position)?;
            let effective = writable
                .as_ref()
                .and_then(|writable| overlay_point(overlay, writable))
                .unwrap_or(legacy);
            Ok((key.clone(), effective, writable))
        })
        .collect::<Result<Vec<_>, CodeExpansionError>>()?;
    let mut points = Vec::with_capacity(effective_vertices.len());
    for (key, position, writable) in &effective_vertices {
        let address = direct_polyline_address(&declaration.symbol, "vertex", key, "point");
        let (alias, identity) = direct_generated_alias(&address, reconciliation)?;
        let selector = node_selector(IntentPortRole::Primary, 0);
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            alias.clone(),
        )
        .with_instance_leaf(selector, LeafField::X, length(position[0]))
        .with_instance_leaf(selector, LeafField::Y, length(position[1]));
        builder.push_node(&declaration.symbol, alias.clone(), draft)?;
        let point = SemanticValue::Port(port(&alias, selector, IntentPortKind::Point));
        if let Some(identity) = identity {
            builder.add_provenance(&address, identity, &declaration.symbol, None, &point)?;
        }
        let SemanticValue::Port(point) = point else {
            unreachable!("keyed Polyline point is a port")
        };
        builder.add_point_seed(&point, *position)?;
        if let Some(address) = writable {
            builder.add_writable_point(ExpandedWritablePoint {
                handle: point.clone(),
                source: CodePointSeedSource::Literal,
                edit: CodePointEdit::Point {
                    address: address.clone(),
                },
            });
        }
        points.push(point);
    }

    let mut spans = Vec::with_capacity(segment_count);
    for index in 0..segment_count {
        let (key, start_position, _) = &effective_vertices[index];
        let end_index = (index + 1) % effective_vertices.len();
        let end_position = effective_vertices[end_index].1;
        let address = direct_polyline_address(&declaration.symbol, "segment", key, "span");
        let (alias, identity) = direct_generated_alias(&address, reconciliation)?;
        let direction = unit_direction(*start_position, end_position).ok_or_else(|| {
            CodeExpansionError::InvalidDeclaration {
                declaration: declaration.symbol.0.clone(),
                message: format!("Polyline segment starting at `{key}` must be finite and nonzero"),
            }
        })?;
        let draft = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            alias.clone(),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            points[index].patch_ref(),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 1),
            points[end_index].patch_ref(),
        )
        .with_field(
            field_key("branch_direction")?,
            IntentLiteral::Point(direction),
        );
        builder.push_node(&declaration.symbol, alias.clone(), draft)?;
        let span = SemanticValue::Port(port(
            &alias,
            node_selector(IntentPortRole::Span, 0),
            IntentPortKind::CurveSpan,
        ));
        if let Some(identity) = identity {
            builder.add_provenance(&address, identity, &declaration.symbol, None, &span)?;
        }
        let SemanticValue::Port(span) = span else {
            unreachable!("keyed Polyline span is a port")
        };
        spans.push(span);
    }

    let alias = semantic_alias(
        if definition.closed {
            "polyline.profile"
        } else {
            "polyline.chain"
        },
        &builder.project,
        &declaration.symbol,
        &[],
    )?;
    let aggregate = if definition.closed {
        AggregateKind::ClosedProfile
    } else {
        AggregateKind::OpenChain
    };
    let mut draft = IntentNodeDraft::new(IntentNodeKind::Aggregate { aggregate }, alias.clone());
    for (index, span) in spans.iter().enumerate() {
        let index = u16::try_from(index).expect("checked Polyline segment count");
        draft = draft.with_input(InputSlot::new(InputRole::Span, index), span.patch_ref());
    }
    builder.push_node(&declaration.symbol, alias.clone(), draft)?;

    let mut paths = BTreeMap::new();
    let mut vertices = BTreeMap::new();
    let mut segments = BTreeMap::new();
    let mut corners = BTreeMap::new();
    for (ordinal, ((key, _, _), point)) in effective_vertices.iter().zip(&points).enumerate() {
        vertices.insert(key.clone(), SemanticValue::Port(point.clone()));
        insert_path(
            &mut paths,
            member_path(&["vertices"], key, &["position"]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        insert_path(
            &mut paths,
            index_path(&["vertices"], ordinal, &["position"]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        insert_path(
            &mut paths,
            fields_path(&["vertices", "byKey", key.as_str()]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        if ordinal < segment_count {
            segments.insert(key.clone(), SemanticValue::Port(spans[ordinal].clone()));
            insert_path(
                &mut paths,
                member_path(&["segments"], key, &[]),
                SemanticValue::Port(spans[ordinal].clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                index_path(&["segments"], ordinal, &[]),
                SemanticValue::Port(spans[ordinal].clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                fields_path(&["segments", "byKey", key.as_str()]),
                SemanticValue::Port(spans[ordinal].clone()),
                declaration,
            )?;
        }
        let is_corner =
            definition.closed || (ordinal > 0 && ordinal + 1 < definition.vertices.len());
        if is_corner {
            let incoming_index = if ordinal == 0 {
                segment_count - 1
            } else {
                ordinal - 1
            };
            let outgoing_index = ordinal % segment_count;
            let corner = ExpandedFeatureCorner {
                point: point.clone(),
                incoming: spans[incoming_index].clone(),
                outgoing: spans[outgoing_index].clone(),
            };
            corners.insert(key.clone(), SemanticValue::Corner(corner.clone()));
            insert_path(
                &mut paths,
                member_path(&["filletableCorners"], key, &[]),
                SemanticValue::Corner(corner.clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                fields_path(&["filletableCorners", "byKey", key.as_str()]),
                SemanticValue::Corner(corner),
                declaration,
            )?;
        }
    }
    paths.insert(
        fields_path(&["vertices"]),
        SemanticValue::Collection(vertices),
    );
    paths.insert(
        fields_path(&["segments"]),
        SemanticValue::Collection(segments),
    );
    paths.insert(
        fields_path(&["filletableCorners"]),
        SemanticValue::Collection(corners),
    );
    paths.insert(
        fields_path(&["curve"]),
        SemanticValue::Port(port(
            &alias,
            node_selector(IntentPortRole::Curve, 0),
            IntentPortKind::Curve,
        )),
    );
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths,
        },
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one native Polyline lowering keeps its keyed points, spans, corners, writable seeds, and reconciliation provenance visibly together"
)]
fn lower_direct_single_curve_polyline(
    builder: &mut ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    definition: &KeyedPolylineDefinition,
    reconciliation: Option<&KeyedReconcileState>,
    overlay: &CodeInteractionOverlay,
) -> Result<(), CodeExpansionError> {
    let segment_count = if definition.closed {
        definition.vertices.len()
    } else {
        definition.vertices.len() - 1
    };
    let child_count = u16::try_from(definition.vertices.len()).map_err(|_| {
        CodeExpansionError::InvalidDeclaration {
            declaration: declaration.symbol.0.clone(),
            message: "Polyline vertex count exceeds the intent child bound".into(),
        }
    })?;
    let alias = builder.lowering_alias("decl", &declaration.symbol, &[])?;
    let mut draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Polyline,
        },
        alias.clone(),
    )
    .with_dynamic_children(child_count)
    .with_field(
        field_key("closed")?,
        IntentLiteral::Boolean(definition.closed),
    );
    if let Some(role) = &definition.role {
        draft = draft.with_field(field_key("role")?, enum_value(role)?);
    }

    let mut positions = Vec::with_capacity(definition.vertices.len());
    let mut point_rows = Vec::with_capacity(definition.vertices.len());
    for (ordinal, (key, authored)) in definition.vertices.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).expect("Polyline child count checked above");
        let selector = child_selector(ordinal, IntentPortRole::Corner);
        let output_path = member_path(&["vertices"], key, &[]);
        let generated_owner = builder
            .lowering_output_owner(&declaration.symbol, &output_path)
            .map(|(address, identity)| (address.clone(), identity));
        let address = generated_owner.as_ref().map_or_else(
            || direct_polyline_address(&declaration.symbol, "vertex", key, "point"),
            |(address, _)| address.clone(),
        );
        if generated_owner.is_none() && builder.suppresses_generated_member(&address) {
            return Err(CodeExpansionError::Unsupported(format!(
                "single-curve Polyline member `{}` cannot be suppressed independently",
                address.display_path()
            )));
        }
        let identity = match generated_owner {
            Some((_, identity)) => Some(identity),
            None => direct_generated_alias(&address, reconciliation)?.1,
        };
        let writable = identity.map(|identity| {
            if builder.generated_template.is_some() {
                CodeWritableAddress::generated_point(
                    builder.project.clone(),
                    address.clone(),
                    identity,
                    output_path.clone(),
                )
            } else {
                CodeWritableAddress::generated_point(
                    builder.project.clone(),
                    address.clone(),
                    identity,
                    member_path(&["vertices"], key, &["position"]),
                )
            }
        });
        let source = match authored {
            ManagedValue::Reference { declaration, path } => CodePointSeedSource::Reference {
                declaration: declaration.clone(),
                path: path.clone(),
            },
            _ => CodePointSeedSource::Literal,
        };
        let override_position = reconciliation
            .and_then(|state| state.override_for(&address))
            .map(|value| {
                point_override(&value.value).ok_or_else(|| {
                    CodeExpansionError::InvalidPointOverride {
                        address: address.display_path(),
                    }
                })
            })
            .transpose()?;
        let drafted_position = writable
            .as_ref()
            .and_then(|address| overlay_point(overlay, address));
        let resolved = if let Some(position) = drafted_position.or(override_position) {
            SemanticValue::PointLiteral(position)
        } else {
            builder.resolve_managed(
                authored,
                &SemanticOutputPath::default(),
                &format!("{}.vertices[{ordinal}].position", declaration.symbol.0),
            )?
        };
        let position = builder.point_seed(&resolved).ok_or_else(|| {
            CodeExpansionError::Unsupported(format!(
                "Polyline vertex `{key}` has no deterministic point seed"
            ))
        })?;
        match resolved {
            SemanticValue::PointLiteral(position) => {
                draft = draft
                    .with_instance_leaf(selector, LeafField::X, length(position[0]))
                    .with_instance_leaf(selector, LeafField::Y, length(position[1]));
            }
            value => {
                let point = value.as_port(
                    IntentPortKind::Point,
                    &format!("{}.vertices[{ordinal}].position", declaration.symbol.0),
                )?;
                draft =
                    draft.with_input(InputSlot::new(InputRole::Point, ordinal), point.patch_ref());
            }
        }
        positions.push(position);
        point_rows.push((
            key,
            selector,
            address,
            identity.filter(|_| builder.generated_template.is_none()),
            writable,
            source,
        ));
    }

    builder.push_node(&declaration.symbol, alias.clone(), draft)?;
    let mut point_ports = Vec::with_capacity(point_rows.len());
    for ((_, position), (key, selector, address, identity, writable, source)) in
        definition.vertices.iter().zip(&positions).zip(point_rows)
    {
        let point = port(&alias, selector, IntentPortKind::Point);
        builder.add_point_seed(&point, *position)?;
        let value = SemanticValue::Port(point.clone());
        if let Some(identity) = identity {
            builder.add_provenance(&address, identity, &declaration.symbol, None, &value)?;
        }
        if let Some(address) = writable {
            builder.add_writable_point(ExpandedWritablePoint {
                handle: point.clone(),
                source,
                edit: CodePointEdit::Point { address },
            });
        }
        point_ports.push((key.clone(), point));
    }

    let mut span_ports = Vec::with_capacity(segment_count);
    for (ordinal, (key, _)) in definition.vertices[..segment_count].iter().enumerate() {
        let ordinal = u16::try_from(ordinal).expect("Polyline span count is bounded by children");
        let output_path = member_path(&["segments"], key, &[]);
        let generated_owner = builder
            .lowering_output_owner(&declaration.symbol, &output_path)
            .map(|(address, identity)| (address.clone(), identity));
        let address = generated_owner.as_ref().map_or_else(
            || direct_polyline_address(&declaration.symbol, "segment", key, "span"),
            |(address, _)| address.clone(),
        );
        if generated_owner.is_none() && builder.suppresses_generated_member(&address) {
            return Err(CodeExpansionError::Unsupported(format!(
                "single-curve Polyline member `{}` cannot be suppressed independently",
                address.display_path()
            )));
        }
        let identity = match generated_owner {
            Some((_, _)) => None,
            None => direct_generated_alias(&address, reconciliation)?.1,
        };
        let span = port(
            &alias,
            child_selector(ordinal, IntentPortRole::Span),
            IntentPortKind::CurveSpan,
        );
        let value = SemanticValue::Port(span.clone());
        if let Some(identity) = identity {
            builder.add_provenance(&address, identity, &declaration.symbol, None, &value)?;
        }
        span_ports.push((key.clone(), span));
    }

    let mut paths = BTreeMap::new();
    let mut vertices = BTreeMap::new();
    let mut segments = BTreeMap::new();
    let mut corners = BTreeMap::new();
    for (ordinal, (key, point)) in point_ports.iter().enumerate() {
        vertices.insert(key.clone(), SemanticValue::Port(point.clone()));
        insert_path(
            &mut paths,
            member_path(&["vertices"], key, &["position"]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        insert_path(
            &mut paths,
            index_path(&["vertices"], ordinal, &["position"]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        insert_path(
            &mut paths,
            fields_path(&["vertices", "byKey", key.as_str()]),
            SemanticValue::Port(point.clone()),
            declaration,
        )?;
        if ordinal < segment_count {
            let span = &span_ports[ordinal].1;
            segments.insert(key.clone(), SemanticValue::Port(span.clone()));
            insert_path(
                &mut paths,
                member_path(&["segments"], key, &[]),
                SemanticValue::Port(span.clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                index_path(&["segments"], ordinal, &[]),
                SemanticValue::Port(span.clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                fields_path(&["segments", "byKey", key.as_str()]),
                SemanticValue::Port(span.clone()),
                declaration,
            )?;
        }
        let is_corner =
            definition.closed || (ordinal > 0 && ordinal + 1 < definition.vertices.len());
        if is_corner {
            let incoming_index = if ordinal == 0 {
                segment_count - 1
            } else {
                ordinal - 1
            };
            let outgoing_index = ordinal % segment_count;
            let corner = ExpandedFeatureCorner {
                point: point.clone(),
                incoming: span_ports[incoming_index].1.clone(),
                outgoing: span_ports[outgoing_index].1.clone(),
            };
            corners.insert(key.clone(), SemanticValue::Corner(corner.clone()));
            insert_path(
                &mut paths,
                member_path(&["filletableCorners"], key, &[]),
                SemanticValue::Corner(corner.clone()),
                declaration,
            )?;
            insert_path(
                &mut paths,
                fields_path(&["filletableCorners", "byKey", key.as_str()]),
                SemanticValue::Corner(corner),
                declaration,
            )?;
        }
    }
    paths.insert(
        fields_path(&["vertices"]),
        SemanticValue::Collection(vertices),
    );
    paths.insert(
        fields_path(&["segments"]),
        SemanticValue::Collection(segments),
    );
    paths.insert(
        fields_path(&["filletableCorners"]),
        SemanticValue::Collection(corners),
    );
    paths.insert(
        fields_path(&["curve"]),
        SemanticValue::Port(port(
            &alias,
            node_selector(IntentPortRole::Curve, 0),
            IntentPortKind::Curve,
        )),
    );
    builder.insert_declaration(
        declaration.symbol.clone(),
        SemanticDeclaration {
            root: SemanticValue::Declaration {
                alias,
                kind: FeatureKind::Feature,
            },
            paths,
        },
    )
}

fn validate_supported_overrides(
    reconciliation: &KeyedReconcileState,
) -> Result<(), CodeExpansionError> {
    for address in reconciliation.active().keys() {
        let Some(value) = reconciliation.override_for(address) else {
            continue;
        };
        if address.template != ["polyline", "vertex"] || address.output != ["point"] {
            return Err(CodeExpansionError::UnsupportedOverride {
                address: address.display_path(),
            });
        }
        if point_override(&value.value).is_none() {
            return Err(CodeExpansionError::InvalidPointOverride {
                address: address.display_path(),
            });
        }
    }
    Ok(())
}

fn overridden_point(
    reconciliation: Option<&KeyedReconcileState>,
    address: &GeneratedMemberAddress,
    authored: [f64; 2],
) -> Result<[f64; 2], CodeExpansionError> {
    let Some(value) = reconciliation.and_then(|state| state.override_for(address)) else {
        return Ok(authored);
    };
    point_override(&value.value).ok_or_else(|| CodeExpansionError::InvalidPointOverride {
        address: address.display_path(),
    })
}

fn point_override(value: &ManagedValue) -> Option<[f64; 2]> {
    let ManagedValue::Array(values) = value else {
        return None;
    };
    let [ManagedValue::Number(x), ManagedValue::Number(y)] = values.as_slice() else {
        return None;
    };
    (x.is_finite() && y.is_finite()).then_some([*x, *y])
}

fn direct_owner_identity(
    declaration: &AuthoringDeclaration,
    family: &str,
    reconciliation: Option<&KeyedReconcileState>,
) -> Option<GeneratedMemberIdentity> {
    let reconciliation = reconciliation?;
    let address = direct_declaration_owner_address(&declaration.symbol, family);
    reconciliation.active().get(&address).copied()
}

fn direct_point_address(
    project: &ProjectKey,
    declaration: &SemanticSymbol,
    identity: GeneratedMemberIdentity,
    output: SemanticOutputPath,
) -> CodeWritableAddress {
    CodeWritableAddress::direct_point(project.clone(), declaration.clone(), identity, output)
}

fn overlay_point(
    overlay: &CodeInteractionOverlay,
    address: &CodeWritableAddress,
) -> Option<[f64; 2]> {
    let draft = overlay.draft(address)?;
    match draft.value {
        CodeDraftValue::Point(point) => Some(point),
    }
}

fn writable_addresses(point: &ExpandedWritablePoint) -> Vec<&CodeWritableAddress> {
    match &point.edit {
        CodePointEdit::Point { address } => vec![address],
        CodePointEdit::RectangleCorner {
            lower_left,
            upper_right,
            ..
        } => vec![lower_left, upper_right],
    }
}

fn validate_overlay_coverage(
    overlay: &CodeInteractionOverlay,
    writable_points: &[ExpandedWritablePoint],
) -> Result<(), CodeExpansionError> {
    let mut known = BTreeMap::<CodeWritableAddress, BTreeSet<CodeDraftProvenance>>::new();
    for point in writable_points {
        let provenance = point.draft_provenance();
        for address in writable_addresses(point) {
            known.entry(address.clone()).or_default().insert(provenance);
        }
    }
    if let Some((address, _)) = known.iter().find(|(_, provenances)| provenances.len() != 1) {
        return Err(CodeExpansionError::Unsupported(format!(
            "writable seed `{}` has ambiguous draft provenance",
            address.display_path()
        )));
    }
    for (address, draft) in overlay.drafts() {
        if let Some(expected) = known.get(address) {
            if !expected.contains(&draft.provenance) {
                return Err(CodeExpansionError::Unsupported(format!(
                    "draft `{}` has {:?} provenance, expected {:?}",
                    address.display_path(),
                    draft.provenance,
                    expected.first().expect("known provenance is nonempty")
                )));
            }
            continue;
        }
        let same_semantic_owner = known.keys().any(|known| {
            known.project == address.project
                && known.owner.address == address.owner.address
                && known.output == address.output
                && known.field == address.field
        });
        if same_semantic_owner {
            return Err(CodeExpansionError::StaleDraftGeneration {
                address: address.display_path(),
            });
        }
        return Err(CodeExpansionError::UnknownDraft {
            address: address.display_path(),
        });
    }
    Ok(())
}

fn validate_draft_conflicts(
    overlay: &CodeInteractionOverlay,
    writable_points: &[ExpandedWritablePoint],
) -> Result<(), CodeExpansionError> {
    let mut by_handle =
        BTreeMap::<(IntentKey, IntentPortSelector), (&CodeWritableAddress, [f64; 2])>::new();
    for point in writable_points {
        let CodePointEdit::Point { address } = &point.edit else {
            continue;
        };
        let Some(CodeDraftValue::Point(value)) = overlay.draft(address).map(|draft| draft.value)
        else {
            continue;
        };
        let key = (point.handle.alias.clone(), point.handle.selector);
        if let Some((known_address, known_value)) = by_handle.insert(key, (address, value))
            && point_seed_bits(known_value) != point_seed_bits(value)
        {
            return Err(CodeExpansionError::ConflictingDrafts {
                first: known_address.display_path(),
                second: address.display_path(),
            });
        }
    }
    Ok(())
}

fn point_seed_bits(point: [f64; 2]) -> [u64; 2] {
    [point[0].to_bits(), point[1].to_bits()]
}

fn keyed_polyline_definition(
    declaration: &AuthoringDeclaration,
) -> Result<KeyedPolylineDefinition, CodeExpansionError> {
    let arguments = object(&declaration.arguments, &declaration.symbol.0)?;
    let vertices = array(
        required(arguments, "vertices", &declaration.symbol.0)?,
        "vertices",
    )?;
    if !(2..=usize::from(u16::MAX)).contains(&vertices.len()) {
        return invalid_declaration(
            declaration,
            format!(
                "Polyline vertex count {} is outside 2..={}",
                vertices.len(),
                u16::MAX
            ),
        );
    }
    let closed = arguments
        .get("closed")
        .map(|value| boolean(value, "closed"))
        .transpose()?
        .unwrap_or(false);
    let representation = match arguments.get("representation") {
        // The clean authoring API is one native Polyline declaration. The
        // historical composite was an adapter implementation detail and
        // cannot authenticate the public `curve` result promised by the Rust
        // catalog.
        None => DirectPolylineRepresentation::SingleCurve,
        Some(value) if string(value, "Polyline representation")? == "singleCurve" => {
            DirectPolylineRepresentation::SingleCurve
        }
        Some(_) => {
            return invalid_declaration(
                declaration,
                "Polyline representation must be `singleCurve` when present".into(),
            );
        }
    };
    let role = arguments
        .get("role")
        .map(|value| string(value, "Polyline role").map(str::to_owned))
        .transpose()?;
    if role
        .as_deref()
        .is_some_and(|role| !matches!(role, "profile" | "construction"))
    {
        return invalid_declaration(
            declaration,
            "Polyline role must be `profile` or `construction`".into(),
        );
    }
    if representation == DirectPolylineRepresentation::Composite && role.is_some() {
        return invalid_declaration(
            declaration,
            "Polyline role requires `representation: \"singleCurve\"`".into(),
        );
    }
    let mut keyed = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let value = object(vertex, "Polyline vertex")?;
        let key = string(required(value, "key", "Polyline vertex")?, "vertex key")?;
        if keyed.iter().any(|(existing, _)| existing == key) {
            return invalid_declaration(declaration, format!("duplicate Polyline key `{key}`"));
        }
        let position = required(value, "position", "Polyline vertex")?;
        if representation == DirectPolylineRepresentation::Composite
            || !matches!(position, ManagedValue::Reference { .. })
        {
            point(position, "position")?;
        }
        keyed.push((key.to_owned(), position.clone()));
    }
    Ok(KeyedPolylineDefinition {
        vertices: keyed,
        closed,
        representation,
        role,
    })
}

#[allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "exactly equal authored coordinates are the explicit zero-width/height invalid geometry boundary"
)]
fn build_rectangle(
    builder: &mut ExpansionBuilder,
    symbol: &SemanticSymbol,
    generated: Option<(&GeneratedMemberAddress, GeneratedMemberIdentity)>,
    lower_left: [f64; 2],
    upper_right: [f64; 2],
) -> Result<SemanticDeclaration, CodeExpansionError> {
    if lower_left[0] == upper_right[0] || lower_left[1] == upper_right[1] {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: symbol.0.clone(),
            message: "rectangle corners must define nonzero width and height".into(),
        });
    }
    let alias = match generated {
        Some((address, identity)) => generated_alias(address, identity)?,
        None => semantic_alias("decl", &builder.project, symbol, &[])?,
    };
    let first = node_selector(IntentPortRole::Corner, 0);
    let third = node_selector(IntentPortRole::Corner, 2);
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
        },
        alias.clone(),
    )
    .with_instance_leaf(first, LeafField::X, length(lower_left[0]))
    .with_instance_leaf(first, LeafField::Y, length(lower_left[1]))
    .with_instance_leaf(third, LeafField::X, length(upper_right[0]))
    .with_instance_leaf(third, LeafField::Y, length(upper_right[1]));
    builder.push_node(symbol, alias.clone(), draft)?;

    for (selector, position) in [
        (node_selector(IntentPortRole::Corner, 0), lower_left),
        (
            node_selector(IntentPortRole::Corner, 1),
            [upper_right[0], lower_left[1]],
        ),
        (node_selector(IntentPortRole::Corner, 2), upper_right),
        (
            node_selector(IntentPortRole::Corner, 3),
            [lower_left[0], upper_right[1]],
        ),
    ] {
        builder.add_point_seed(&port(&alias, selector, IntentPortKind::Point), position)?;
    }

    let profile_alias = semantic_alias("profile", &builder.project, symbol, &[])?;
    let mut profile = IntentNodeDraft::new(
        IntentNodeKind::Aggregate {
            aggregate: AggregateKind::ClosedProfile,
        },
        profile_alias.clone(),
    );
    for index in 0..4 {
        profile = profile.with_input(
            InputSlot::new(InputRole::Span, index),
            port(
                &alias,
                node_selector(IntentPortRole::Span, index),
                IntentPortKind::CurveSpan,
            )
            .patch_ref(),
        );
    }
    builder.push_node(symbol, profile_alias.clone(), profile)?;

    let corner_names = ["lowerLeft", "lowerRight", "upperRight", "upperLeft"];
    let edge_names = ["bottom", "right", "top", "left"];
    let mut paths = BTreeMap::new();
    for index in 0..4_u16 {
        let point = port(
            &alias,
            node_selector(IntentPortRole::Corner, index),
            IntentPortKind::Point,
        );
        let corner = ExpandedFeatureCorner {
            point,
            incoming: port(
                &alias,
                node_selector(IntentPortRole::Span, (index + 3) % 4),
                IntentPortKind::CurveSpan,
            ),
            outgoing: port(
                &alias,
                node_selector(IntentPortRole::Span, index),
                IntentPortKind::CurveSpan,
            ),
        };
        paths.insert(
            fields_path(&["corners", corner_names[usize::from(index)]]),
            SemanticValue::Corner(corner),
        );
        paths.insert(
            fields_path(&["edges", edge_names[usize::from(index)]]),
            SemanticValue::Port(port(
                &alias,
                node_selector(IntentPortRole::Span, index),
                IntentPortKind::CurveSpan,
            )),
        );
    }
    paths.insert(
        fields_path(&["profile"]),
        SemanticValue::Port(port(
            &profile_alias,
            node_selector(IntentPortRole::Profile, 0),
            IntentPortKind::Profile,
        )),
    );
    Ok(SemanticDeclaration {
        root: SemanticValue::Declaration {
            alias,
            kind: FeatureKind::Feature,
        },
        paths,
    })
}

fn plan_collections(
    builder: &ExpansionBuilder,
    declaration: &AuthoringDeclaration,
    artifact: &PatchModuleArtifact,
) -> Result<CollectionPlan, CodeExpansionError> {
    let invocation = declaration
        .patch
        .as_ref()
        .expect("invocation planner only receives patch declarations");
    let arguments = object(&invocation.arguments, &declaration.symbol.0)?;
    let mut result = BTreeMap::new();
    for rule in &artifact.collections {
        let (path, input) = match rule {
            CollectionRule::Each { path, input, .. }
            | CollectionRule::MapRecord { path, input, .. } => (path, input),
        };
        let members = if let Some(value) = arguments.get(input) {
            collection_from_managed(
                builder,
                value,
                &format!("{}.{}", declaration.symbol.0, input),
            )?
        } else if input == "corners" {
            let polyline = required(arguments, "polyline", &declaration.symbol.0)?;
            let ManagedValue::Reference {
                declaration: owner,
                path: base,
            } = polyline
            else {
                return Err(CodeExpansionError::UnresolvedReference {
                    reference: format!("{}.polyline", declaration.symbol.0),
                });
            };
            let corners = builder.resolve(
                owner,
                &join_paths(base, &fields_path(&["filletableCorners"])),
            )?;
            match corners {
                SemanticValue::Collection(values) => values.into_iter().collect(),
                other => {
                    return Err(CodeExpansionError::KindMismatch {
                        reference: format!("{}.polyline.filletableCorners", declaration.symbol.0),
                        expected: FeatureKind::Collection,
                        actual: other.kind(),
                    });
                }
            }
        } else if input == "holeKeys" {
            derived_mounting_centers(arguments, &declaration.symbol.0)?
                .into_iter()
                .map(|(key, point)| (key, SemanticValue::PointLiteral(point)))
                .collect()
        } else {
            return Err(CodeExpansionError::UnresolvedReference {
                reference: format!("{}.{}", declaration.symbol.0, input),
            });
        };
        if members.len() > MAX_EXPANDED_OUTPUTS {
            return Err(CodeExpansionError::ResourceLimit {
                resource: "collection members",
                actual: members.len(),
                limit: MAX_EXPANDED_OUTPUTS,
            });
        }
        result.insert(path.clone(), members);
    }
    Ok(result)
}

fn collection_from_managed(
    builder: &ExpansionBuilder,
    value: &ManagedValue,
    label: &str,
) -> Result<Vec<(String, SemanticValue)>, CodeExpansionError> {
    match value {
        ManagedValue::Object(values) => values
            .iter()
            .map(|(key, value)| {
                let resolved = match value {
                    ManagedValue::Reference { declaration, path } => {
                        builder.resolve(declaration, path)?
                    }
                    ManagedValue::Array(values) => {
                        SemanticValue::PointLiteral(point_from_array(values, label)?)
                    }
                    _ => {
                        return Err(CodeExpansionError::UnresolvedReference {
                            reference: format!("{label}.{key}"),
                        });
                    }
                };
                Ok((key.clone(), resolved))
            })
            .collect(),
        ManagedValue::Reference { declaration, path } => {
            match builder.resolve(declaration, path)? {
                SemanticValue::Collection(values) => Ok(values.into_iter().collect()),
                other => Err(CodeExpansionError::KindMismatch {
                    reference: label.to_owned(),
                    expected: FeatureKind::Collection,
                    actual: other.kind(),
                }),
            }
        }
        _ => Err(CodeExpansionError::KindMismatch {
            reference: label.to_owned(),
            expected: FeatureKind::Collection,
            actual: managed_kind(value),
        }),
    }
}

fn plan_invocation_addresses(
    declaration: &AuthoringDeclaration,
    artifact: &PatchModuleArtifact,
    collections: &CollectionPlan,
) -> Result<(GeneratedAddressPlan, TemplateOutputPlan), CodeExpansionError> {
    let mut template_collections = BTreeMap::<Vec<String>, Vec<(Vec<String>, String)>>::new();
    for rule in &artifact.collections {
        let (path, templates) = match rule {
            CollectionRule::Each {
                path, templates, ..
            }
            | CollectionRule::MapRecord {
                path, templates, ..
            } => (path, templates),
        };
        let members =
            collections
                .get(path)
                .ok_or_else(|| CodeExpansionError::UnresolvedReference {
                    reference: format!("{} collection {}", declaration.symbol.0, path.join(".")),
                })?;
        for template in templates {
            template_collections.insert(
                template.clone(),
                members
                    .iter()
                    .map(|(key, _)| (vec![key.clone()], key.clone()))
                    .collect(),
            );
        }
    }
    let mut addresses = BTreeMap::new();
    for template in &artifact.templates {
        let members = template_collections
            .get(&template.path)
            .cloned()
            .unwrap_or_else(|| vec![(vec!["self".into()], "self".into())]);
        for (member_key, _) in members {
            for output in &template.outputs {
                let address = GeneratedMemberAddress::new(
                    declaration.symbol.0.clone(),
                    template.path.clone(),
                    member_key.clone(),
                    generated_output_address(&output.path),
                );
                let key = (
                    template.path.clone(),
                    member_key.clone(),
                    output.path.clone(),
                );
                if addresses.insert(key, address.clone()).is_some() {
                    return Err(CodeExpansionError::DuplicateGeneratedAddress(
                        address.display_path(),
                    ));
                }
            }
        }
    }

    // Direct keyed geometry owns its own addresses. Patch invocations only
    // reconcile members actually generated by their artifact templates.
    Ok((addresses, BTreeMap::new()))
}

fn lower_invocation(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<(), CodeExpansionError> {
    for (key, value) in &plan.synthetic_outputs {
        let address = plan
            .addresses
            .get(key)
            .expect("synthetic output owns address");
        let identity = reconciliation
            .active()
            .get(address)
            .copied()
            .ok_or_else(|| {
                CodeExpansionError::ReconciliationMismatch(format!(
                    "missing active `{}`",
                    address.display_path()
                ))
            })?;
        builder.add_provenance(address, identity, &plan.declaration.symbol, None, value)?;
    }
    let artifact = plan.pinned.artifact.artifact();
    let mut template_outputs = TemplateOutputPlan::new();
    let mut pending = artifact
        .templates
        .iter()
        .map(|template| (template.path.clone(), template))
        .collect::<BTreeMap<_, _>>();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .filter(|(_, template)| {
                template_argument_bindings(&template.arguments)
                    .into_iter()
                    .all(|binding| match binding {
                        TemplateBinding::TemplateOutput { template, path, .. } => artifact
                            .templates
                            .iter()
                            .find(|candidate| &candidate.path == template)
                            .is_some_and(|dependency| {
                                dependency.outputs.iter().any(|output| {
                                    output.path == *path
                                        && template_outputs.keys().any(
                                            |(candidate_template, _, candidate_path)| {
                                                candidate_template == template
                                                    && candidate_path == path
                                            },
                                        )
                                })
                            }),
                        TemplateBinding::Input { .. }
                        | TemplateBinding::CollectionMember { .. } => true,
                    })
            })
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(CodeExpansionError::Unsupported(format!(
                "artifact `{}` template DAG could not be scheduled",
                artifact.export_name
            )));
        }
        for path in ready {
            let template = pending
                .remove(&path)
                .expect("ready template remains pending");
            lower_template(
                builder,
                plan,
                template,
                reconciliation,
                overlay,
                operation_planner,
                &mut template_outputs,
            )?;
        }
    }
    publish_invocation_declaration(builder, plan, &template_outputs)
}

fn lower_template(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
    template_outputs: &mut TemplateOutputPlan,
) -> Result<(), CodeExpansionError> {
    let member_values = collection_members_for_template(plan, &template.path);
    for (member_key, member_value) in member_values {
        let outputs = template
            .outputs
            .iter()
            .map(|output| {
                let key = (
                    template.path.clone(),
                    member_key.clone(),
                    output.path.clone(),
                );
                let address = plan.addresses.get(&key).ok_or_else(|| {
                    CodeExpansionError::UnresolvedReference {
                        reference: format!("generated {}", template.path.join(".")),
                    }
                })?;
                let identity = reconciliation
                    .active()
                    .get(address)
                    .copied()
                    .ok_or_else(|| {
                        CodeExpansionError::ReconciliationMismatch(format!(
                            "missing active `{}`",
                            address.display_path()
                        ))
                    })?;
                Ok((output.path.clone(), output.kind, address.clone(), identity))
            })
            .collect::<Result<Vec<_>, CodeExpansionError>>()?;

        let bindings = resolve_template_bindings(
            builder,
            plan,
            template,
            &member_key,
            member_value.as_ref(),
            template_outputs,
        )?;
        let suppressed_output_count = outputs
            .iter()
            .filter(|(_, _, address, _)| builder.suppresses_generated_member(address))
            .count();
        if suppressed_output_count != 0 && suppressed_output_count != outputs.len() {
            return Err(CodeExpansionError::Unsupported(format!(
                "generated template member `{}` shares one native declaration across independently suppressed outputs",
                template.path.join("."),
            )));
        }
        let suppress_member = (!outputs.is_empty() && suppressed_output_count == outputs.len())
            || builder.suppresses_declaration(&plan.declaration.symbol);
        let operation_start = builder.operations.len();
        let host_request_start = builder.host_requests.len();
        let lowered = lower_template_family(
            builder,
            plan,
            template,
            &member_key,
            &outputs,
            &bindings,
            overlay,
            operation_planner,
        )?;
        if suppress_member {
            builder.suppress_lowered_member_since(operation_start, host_request_start);
        }
        for (output, _, address, identity) in outputs {
            let value = lowered.get(&output).cloned().ok_or_else(|| {
                CodeExpansionError::Unsupported(format!(
                    "family `{}` did not lower output `{}`",
                    template.declaration_family,
                    path_text(&output),
                ))
            })?;
            builder.add_provenance(
                &address,
                identity,
                &plan.declaration.symbol,
                Some(plan.pinned.digest.clone()),
                &value,
            )?;
            template_outputs.insert((template.path.clone(), member_key.clone(), output), value);
        }
    }
    Ok(())
}

fn collection_members_for_template(
    plan: &InvocationPlan,
    template_path: &[String],
) -> Vec<(Vec<String>, Option<SemanticValue>)> {
    for rule in &plan.pinned.artifact.artifact().collections {
        let (path, templates) = match rule {
            CollectionRule::Each {
                path, templates, ..
            }
            | CollectionRule::MapRecord {
                path, templates, ..
            } => (path, templates),
        };
        if templates.iter().any(|candidate| candidate == template_path) {
            return plan
                .collections
                .get(path)
                .into_iter()
                .flatten()
                .map(|(key, value)| (vec![key.clone()], Some(value.clone())))
                .collect();
        }
    }
    vec![(vec!["self".into()], None)]
}

fn resolve_template_bindings(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    member_value: Option<&SemanticValue>,
    template_outputs: &TemplateOutputPlan,
) -> Result<ResolvedTemplateBindings, CodeExpansionError> {
    let arguments = object(
        &plan
            .declaration
            .patch
            .as_ref()
            .expect("patch plan")
            .arguments,
        &plan.declaration.symbol.0,
    )?;
    let TemplateArgument::Object(arguments_tree) = &template.arguments else {
        return Err(CodeExpansionError::Unsupported(format!(
            "template `{}` arguments must be an object",
            template.path.join(".")
        )));
    };
    let mut resolved = BTreeMap::new();
    let mut direct = BTreeMap::new();
    let mut temporary = Vec::new();
    for (name, argument) in arguments_tree {
        let value = resolve_template_argument_bindings(
            builder,
            plan,
            template,
            arguments,
            name,
            argument,
            member_key,
            member_value,
            template_outputs,
            Some(name),
            &mut direct,
            &mut temporary,
        )?;
        if resolved.insert(name.clone(), value).is_some() {
            return Err(CodeExpansionError::Unsupported(format!(
                "template argument `{name}` is duplicated"
            )));
        }
    }
    Ok(ResolvedTemplateBindings {
        arguments: ManagedValue::Object(resolved),
        direct,
        temporary,
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "recursive template binding keeps all authenticated argument forms in one closed dispatch"
)]
fn resolve_template_argument_bindings(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    invocation_arguments: &BTreeMap<String, ManagedValue>,
    path: &str,
    argument: &TemplateArgument,
    member_key: &[String],
    member_value: Option<&SemanticValue>,
    template_outputs: &TemplateOutputPlan,
    capture_direct: Option<&str>,
    direct: &mut BTreeMap<String, SemanticValue>,
    temporary: &mut Vec<SemanticSymbol>,
) -> Result<ManagedValue, CodeExpansionError> {
    match argument {
        TemplateArgument::Binding(binding) => {
            let value = match binding {
                TemplateBinding::Input {
                    name: input,
                    path,
                    expected_kind,
                } => {
                    let argument =
                        required(invocation_arguments, input, &plan.declaration.symbol.0)?;
                    let value = builder.resolve_managed(argument, path, input)?;
                    ensure_feature_kind(value, *expected_kind, input)?
                }
                TemplateBinding::CollectionMember {
                    input,
                    path,
                    expected_kind,
                } => {
                    let value = member_value.cloned().ok_or_else(|| {
                        CodeExpansionError::UnresolvedReference {
                            reference: format!(
                                "{}.{}[{}]",
                                plan.declaration.symbol.0,
                                input,
                                member_key.join("/")
                            ),
                        }
                    })?;
                    let value = resolve_value_path(&value, &path.0).ok_or_else(|| {
                        CodeExpansionError::UnresolvedReference {
                            reference: format!("collection member {} path", path_text(path)),
                        }
                    })?;
                    ensure_feature_kind(value, *expected_kind, input)?
                }
                TemplateBinding::TemplateOutput {
                    template,
                    path: output_path,
                    expected_kind,
                } => {
                    let exact = (template.clone(), member_key.to_vec(), output_path.clone());
                    let fallback = (template.clone(), Vec::new(), output_path.clone());
                    let value = template_outputs
                        .get(&exact)
                        .or_else(|| template_outputs.get(&fallback))
                        .cloned()
                        .ok_or_else(|| CodeExpansionError::UnresolvedReference {
                            reference: format!(
                                "template {}.{}",
                                template.join("."),
                                path_text(output_path),
                            ),
                        })?;
                    ensure_feature_kind(value, *expected_kind, &path_text(output_path))?
                }
            };
            if let Some(name) = capture_direct
                && direct.insert(name.to_owned(), value.clone()).is_some()
            {
                return Err(CodeExpansionError::Unsupported(format!(
                    "template argument binding `{name}` is duplicated"
                )));
            }
            let symbol = template_binding_symbol(plan, template, member_key, path)?;
            builder.insert_declaration(
                symbol.clone(),
                SemanticDeclaration {
                    root: value,
                    paths: BTreeMap::new(),
                },
            )?;
            temporary.push(symbol.clone());
            Ok(ManagedValue::Reference {
                declaration: symbol,
                path: SemanticOutputPath::default(),
            })
        }
        TemplateArgument::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                resolve_template_argument_bindings(
                    builder,
                    plan,
                    template,
                    invocation_arguments,
                    &format!("{path}.{index}"),
                    value,
                    member_key,
                    member_value,
                    template_outputs,
                    None,
                    direct,
                    temporary,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(ManagedValue::Array),
        TemplateArgument::Object(values) => values
            .iter()
            .map(|(name, value)| {
                resolve_template_argument_bindings(
                    builder,
                    plan,
                    template,
                    invocation_arguments,
                    &format!("{path}.{name}"),
                    value,
                    member_key,
                    member_value,
                    template_outputs,
                    None,
                    direct,
                    temporary,
                )
                .map(|resolved| (name.clone(), resolved))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(ManagedValue::Object),
        TemplateArgument::Literal(value) => Ok(value.clone()),
    }
}

fn template_binding_symbol(
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    path: &str,
) -> Result<SemanticSymbol, CodeExpansionError> {
    let bytes = serde_json::to_vec(&(&plan.declaration.symbol, &template.path, member_key, path))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    Ok(SemanticSymbol(format!(
        "__patch_binding_{}",
        intent_content_digest(&bytes)
    )))
}

fn template_argument_bindings(argument: &TemplateArgument) -> Vec<&TemplateBinding> {
    let mut bindings = Vec::new();
    collect_template_argument_bindings(argument, &mut bindings);
    bindings
}

fn collect_template_argument_bindings<'a>(
    argument: &'a TemplateArgument,
    bindings: &mut Vec<&'a TemplateBinding>,
) {
    match argument {
        TemplateArgument::Binding(binding) => bindings.push(binding),
        TemplateArgument::Array(values) => {
            for value in values {
                collect_template_argument_bindings(value, bindings);
            }
        }
        TemplateArgument::Object(values) => {
            for value in values.values() {
                collect_template_argument_bindings(value, bindings);
            }
        }
        TemplateArgument::Literal(_) => {}
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "patch ownership, resolved bindings, interaction state, and native operation planning stay explicit"
)]
fn lower_template_family(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    outputs: &[PlannedTemplateOutput],
    bindings: &ResolvedTemplateBindings,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    let lowered = match template.declaration_family.as_str() {
        "computed.roundedRectangleProfile" => {
            lower_generated_rectangle(builder, plan, outputs, overlay)
        }
        "computed.fillet" => {
            lower_generated_fillet(builder, plan, member_key, outputs, &bindings.direct)
        }
        _ => lower_catalog_template(
            builder,
            plan,
            template,
            member_key,
            outputs,
            &bindings.arguments,
            overlay,
            operation_planner,
        ),
    };
    for symbol in &bindings.temporary {
        builder.declarations.remove(symbol);
    }
    lowered
}

#[allow(
    clippy::too_many_arguments,
    reason = "catalog template lowering keeps authenticated ownership beside its reconstructed source declaration"
)]
fn lower_catalog_template(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
    outputs: &[PlannedTemplateOutput],
    arguments: &ManagedValue,
    overlay: &CodeInteractionOverlay,
    operation_planner: &mut dyn CodeOperationPlanner,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    let mut family = template.declaration_family.split('.');
    let (Some(namespace), Some(method), None) = (family.next(), family.next(), family.next())
    else {
        return Err(CodeExpansionError::Unsupported(format!(
            "artifact declaration family `{}` is not one named catalog method",
            template.declaration_family
        )));
    };
    code_authoring_family(namespace, method).ok_or_else(|| {
        CodeExpansionError::Unsupported(format!(
            "artifact declaration family `{}` is not in the clean authoring catalog",
            template.declaration_family
        ))
    })?;
    let symbol = template_declaration_symbol(plan, template, member_key)?;
    let declaration = AuthoringDeclaration {
        variable: symbol.0.clone(),
        symbol: symbol.clone(),
        builder_path: vec![namespace.to_owned(), method.to_owned()],
        arguments: arguments.clone(),
        patch: None,
        statement_span: plan.declaration.statement_span,
        symbol_span: plan.declaration.symbol_span,
        arguments_span: plan.declaration.arguments_span,
    };
    builder.generated_template = Some(GeneratedTemplateLowering {
        declaration: symbol.clone(),
        invocation: plan.declaration.symbol.clone(),
        outputs: outputs.to_vec(),
    });
    let result = lower_named_declaration(builder, &declaration, None, overlay, operation_planner);
    builder.generated_template = None;
    result?;
    let lowered = builder.declarations.remove(&symbol).ok_or_else(|| {
        CodeExpansionError::Unsupported(format!(
            "catalog family `{}` did not publish its semantic declaration",
            template.declaration_family
        ))
    })?;
    select_planned_outputs(&lowered.paths, outputs, &template.declaration_family)
}

fn template_declaration_symbol(
    plan: &InvocationPlan,
    template: &PatchTemplateNode,
    member_key: &[String],
) -> Result<SemanticSymbol, CodeExpansionError> {
    let bytes = serde_json::to_vec(&(&plan.declaration.symbol, &template.path, member_key))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    Ok(SemanticSymbol(format!(
        "__patch_declaration_{}",
        intent_content_digest(&bytes)
    )))
}

#[allow(
    clippy::too_many_lines,
    reason = "one generated-rectangle lowering keeps coupled seeds, outputs, and provenance auditable together"
)]
fn lower_generated_rectangle(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &[PlannedTemplateOutput],
    overlay: &CodeInteractionOverlay,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    let width = invocation_number(plan, "width")?;
    let height = invocation_number(plan, "height")?;
    if width <= 0.0 || height <= 0.0 {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "rectangle width and height must be positive".into(),
        });
    }
    let (_, _, address, identity) = outputs
        .iter()
        .find(|(path, _, _, _)| *path == fields_path(&["profile"]))
        .or_else(|| outputs.first())
        .ok_or_else(|| {
            CodeExpansionError::Unsupported("rectangle template has no output".into())
        })?;
    let lower_address = CodeWritableAddress::generated_point(
        builder.project.clone(),
        address.clone(),
        *identity,
        fields_path(&["corners", "lowerLeft"]),
    );
    let upper_address = CodeWritableAddress::generated_point(
        builder.project.clone(),
        address.clone(),
        *identity,
        fields_path(&["corners", "upperRight"]),
    );
    let authored_lower = [-width / 2.0, -height / 2.0];
    let authored_upper = [width / 2.0, height / 2.0];
    let lower_left = overlay_point(overlay, &lower_address).unwrap_or(authored_lower);
    let upper_right = overlay_point(overlay, &upper_address).unwrap_or(authored_upper);
    let rectangle = build_rectangle(
        builder,
        &plan.declaration.symbol,
        Some((address, *identity)),
        lower_left,
        upper_right,
    )?;
    let profile = rectangle
        .paths
        .get(&fields_path(&["profile"]))
        .cloned()
        .expect("built rectangle owns a profile");
    for (name, corner) in [
        ("lowerLeft", CodeRectangleCorner::LowerLeft),
        ("lowerRight", CodeRectangleCorner::LowerRight),
        ("upperRight", CodeRectangleCorner::UpperRight),
        ("upperLeft", CodeRectangleCorner::UpperLeft),
    ] {
        let point = rectangle
            .paths
            .get(&fields_path(&["corners", name]))
            .expect("built generated rectangle owns every corner")
            .as_corner(name)?
            .point;
        builder.add_writable_point(ExpandedWritablePoint {
            handle: point,
            source: CodePointSeedSource::Generated,
            edit: CodePointEdit::RectangleCorner {
                lower_left: lower_address.clone(),
                upper_right: upper_address.clone(),
                corner,
                effective_lower_left: lower_left,
                effective_upper_right: upper_right,
            },
        });
    }
    if let Ok(radius) = invocation_unit(plan, &["cornerRadius"]) {
        let corners = ["lowerLeft", "lowerRight", "upperRight", "upperLeft"]
            .into_iter()
            .map(|name| {
                let corner = rectangle
                    .paths
                    .get(&fields_path(&["corners", name]))
                    .expect("built rectangle owns named corners")
                    .as_corner(name)?;
                Ok((name.to_owned(), corner))
            })
            .collect::<Result<BTreeMap<_, _>, CodeExpansionError>>()?;
        builder
            .host_requests
            .push(CodeHostRequest::RoundedRectangleProfile {
                invocation: plan.declaration.symbol.clone(),
                output: address.clone(),
                identity: *identity,
                radius,
                corners,
                suppressed_children: BTreeSet::new(),
                artifact_digest: plan.pinned.digest.clone(),
            });
    }
    // Mounting outputs are independent authored points, not aliases of the
    // rounded profile's rectangle corners. Besides matching the public
    // `mounts.{nw,ne,se,sw}` type, this keeps hole centers free of the
    // profile's endpoint topology and gives every mount its own reconciled
    // native point identity.
    let arguments = object(
        &plan
            .declaration
            .patch
            .as_ref()
            .expect("rectangle template belongs to a patch invocation")
            .arguments,
        &plan.declaration.symbol.0,
    )?;
    let mounting_centers = derived_mounting_centers(arguments, &plan.declaration.symbol.0)?;
    outputs
        .iter()
        .map(|(path, _, address, identity)| {
            if *path == fields_path(&["profile"]) {
                return Ok((path.clone(), profile.clone()));
            }
            let Some(name) = path.0.last().and_then(|segment| match segment {
                ManagedPathSegment::Field(name) => Some(name.as_str()),
                ManagedPathSegment::Index(_) | ManagedPathSegment::Member { .. } => None,
            }) else {
                return Err(CodeExpansionError::Unsupported(format!(
                    "rectangle template output `{}` is not a named point",
                    path_text(path),
                )));
            };
            let Some(position) = mounting_centers.get(name) else {
                return Err(CodeExpansionError::Unsupported(format!(
                    "rectangle template output `{name}`"
                )));
            };
            let alias = generated_alias(address, *identity)?;
            let selector = node_selector(IntentPortRole::Primary, 0);
            let writable_address = CodeWritableAddress::generated_point(
                builder.project.clone(),
                address.clone(),
                *identity,
                SemanticOutputPath::default(),
            );
            let position = overlay_point(overlay, &writable_address).unwrap_or(*position);
            let draft = IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::SketchPoint,
                },
                alias.clone(),
            )
            .with_instance_leaf(selector, LeafField::X, length(position[0]))
            .with_instance_leaf(selector, LeafField::Y, length(position[1]));
            builder.push_node(&plan.declaration.symbol, alias.clone(), draft)?;
            builder.add_point_seed(&port(&alias, selector, IntentPortKind::Point), position)?;
            builder.add_writable_point(ExpandedWritablePoint {
                handle: port(&alias, selector, IntentPortKind::Point),
                source: CodePointSeedSource::Generated,
                edit: CodePointEdit::Point {
                    address: writable_address,
                },
            });
            Ok((
                path.clone(),
                SemanticValue::Port(port(&alias, selector, IntentPortKind::Point)),
            ))
        })
        .collect()
}

fn lower_generated_fillet(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    member_key: &[String],
    outputs: &[PlannedTemplateOutput],
    bindings: &BTreeMap<String, SemanticValue>,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    let corner = bindings
        .get("corner")
        .or_else(|| {
            bindings
                .values()
                .find(|value| value.kind() == FeatureKind::FeatureCorner)
        })
        .ok_or_else(|| CodeExpansionError::UnresolvedReference {
            reference: format!("{}.fillet.corner", plan.declaration.symbol.0),
        })?
        .as_corner("fillet corner")?;
    let radius = bindings
        .get("radius")
        .and_then(|value| match value {
            SemanticValue::ScalarLiteral(value) => Some(value.clone()),
            _ => None,
        })
        .map_or_else(|| invocation_unit(plan, &["radius", "cornerRadius"]), Ok)?;
    if radius.value <= 0.0 {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "Fillet radius must be positive".into(),
        });
    }
    let mut result = BTreeMap::new();
    for (output, expected_kind, address, identity) in outputs {
        builder
            .host_requests
            .push(CodeHostRequest::FilletAtCorner(KeyedFilletHostRequest {
                invocation: plan.declaration.symbol.clone(),
                member_key: member_key.to_vec(),
                output: address.clone(),
                identity: *identity,
                radius: radius.clone(),
                corner: corner.clone(),
                artifact_digest: plan.pinned.digest.clone(),
                suppressed: false,
            }));
        result.insert(
            output.clone(),
            SemanticValue::HostOutput {
                address: address.clone(),
                identity: *identity,
                kind: *expected_kind,
            },
        );
    }
    Ok(result)
}

fn publish_invocation_declaration(
    builder: &mut ExpansionBuilder,
    plan: &InvocationPlan,
    outputs: &TemplateOutputPlan,
) -> Result<(), CodeExpansionError> {
    let (mut paths, mut root_members) = publish_invocation_collections(plan, outputs)?;
    for template in &plan.pinned.artifact.artifact().templates {
        let Some(result_path) = &template.result_path else {
            continue;
        };
        let value = template_public_result(template, &["self".into()], outputs, plan)?;
        let semantic_result_path = SemanticOutputPath(
            result_path
                .iter()
                .cloned()
                .map(ManagedPathSegment::Field)
                .collect(),
        );
        insert_template_public_paths(
            &mut paths,
            &semantic_result_path,
            template,
            &["self".into()],
            outputs,
            &value,
        )?;
        insert_nested_root_member(
            &mut root_members,
            result_path,
            value,
            &plan.declaration.symbol,
        )?;
    }
    let artifact = plan.pinned.artifact.artifact();
    for (name, expected_kind) in &artifact.outputs {
        let path = fields_path(&[name]);
        let value = paths
            .get(&path)
            .cloned()
            .or_else(|| root_members.get(name).cloned())
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: plan.declaration.symbol.0.clone(),
                message: format!("executed patch result `{name}` has no lowered semantic value"),
            })?;
        if value.kind() != *expected_kind {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: plan.declaration.symbol.0.clone(),
                message: format!(
                    "executed patch result `{name}` lowered as {:?}, not {expected_kind:?}",
                    value.kind(),
                ),
            });
        }
        paths.entry(path).or_insert(value);
    }
    let root_kind = if artifact.outputs.len() == 1
        && artifact.outputs.values().next() == Some(&FeatureKind::Collection)
    {
        FeatureKind::Collection
    } else {
        FeatureKind::Feature
    };
    let root = if root_kind == FeatureKind::Collection {
        SemanticValue::Collection(root_members)
    } else {
        let alias = semantic_alias("patch", &builder.project, &plan.declaration.symbol, &[])?;
        SemanticValue::Declaration {
            alias,
            kind: FeatureKind::Feature,
        }
    };
    builder.insert_declaration(
        plan.declaration.symbol.clone(),
        SemanticDeclaration { root, paths },
    )
}

fn template_public_result(
    template: &PatchTemplateNode,
    member_key: &[String],
    outputs: &TemplateOutputPlan,
    plan: &InvocationPlan,
) -> Result<SemanticValue, CodeExpansionError> {
    let matching = outputs
        .iter()
        .filter(|((candidate, member, _), _)| candidate == &template.path && member == member_key)
        .map(|((_, _, path), value)| (path, value))
        .collect::<Vec<_>>();
    if let Some(selected) = &template.result_output {
        return matching
            .iter()
            .find_map(|(path, value)| (*path == selected).then(|| (*value).clone()))
            .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
                declaration: plan.declaration.symbol.0.clone(),
                message: format!(
                    "template `{}` did not lower selected result `{}`",
                    template.path.join("."),
                    path_text(selected),
                ),
            });
    }
    if matching.is_empty() {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: format!(
                "template `{}` has no lowered result",
                template.path.join(".")
            ),
        });
    }
    let mut members = BTreeMap::new();
    for (path, value) in matching {
        insert_semantic_member_path(&mut members, &path.0, value.clone(), plan)?;
    }
    // Every output of one template member is backed by one native declaration.
    // Recover its already-lowered alias from the first leaf when possible;
    // otherwise derive it from the reconciled identity recorded in provenance.
    let alias =
        matching_feature_alias(&members).ok_or_else(|| CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: format!(
                "template `{}` whole result has no native declaration identity",
                template.path.join("."),
            ),
        })?;
    Ok(SemanticValue::Feature { alias, members })
}

fn matching_feature_alias(members: &BTreeMap<String, SemanticValue>) -> Option<IntentKey> {
    members.values().find_map(|value| match value {
        SemanticValue::Declaration { alias, .. } | SemanticValue::Feature { alias, .. } => {
            Some(alias.clone())
        }
        SemanticValue::Port(port) => Some(port.alias.clone()),
        SemanticValue::Collection(children) => matching_feature_alias(children),
        SemanticValue::HostOutput {
            address, identity, ..
        } => host_intent_alias(address, *identity, None).ok(),
        SemanticValue::Corner(_)
        | SemanticValue::PointLiteral(_)
        | SemanticValue::ScalarLiteral(_) => None,
    })
}

fn insert_semantic_member_path(
    members: &mut BTreeMap<String, SemanticValue>,
    path: &[ManagedPathSegment],
    value: SemanticValue,
    plan: &InvocationPlan,
) -> Result<(), CodeExpansionError> {
    let Some((head, tail)) = path.split_first() else {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "template output path cannot be empty".into(),
        });
    };
    let key = match head {
        ManagedPathSegment::Field(field) | ManagedPathSegment::Member { member: field } => {
            field.clone()
        }
        ManagedPathSegment::Index(index) => index.to_string(),
    };
    if tail.is_empty() {
        if members.insert(key, value).is_some() {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: plan.declaration.symbol.0.clone(),
                message: "template result paths overlap".into(),
            });
        }
        return Ok(());
    }
    let entry = members
        .entry(key)
        .or_insert_with(|| SemanticValue::Collection(BTreeMap::new()));
    let SemanticValue::Collection(children) = entry else {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: "template result paths overlap".into(),
        });
    };
    insert_semantic_member_path(children, tail, value, plan)
}

fn insert_template_public_paths(
    paths: &mut BTreeMap<SemanticOutputPath, SemanticValue>,
    base: &SemanticOutputPath,
    template: &PatchTemplateNode,
    member_key: &[String],
    outputs: &TemplateOutputPlan,
    value: &SemanticValue,
) -> Result<(), CodeExpansionError> {
    if paths.insert(base.clone(), value.clone()).is_some() {
        return Err(CodeExpansionError::Unsupported(format!(
            "duplicate patch result path `{}`",
            path_text(base),
        )));
    }

    // A selected callback result is already one exact leaf. For a callback
    // returning a whole typed feature, publish its authenticated output
    // inventory with the original Field/Index/Member segments. Rebuilding
    // these paths from the feature's string-keyed convenience view would
    // collapse indices and keyed members into ordinary fields.
    if template.result_output.is_some() {
        return Ok(());
    }
    for ((candidate, member, output_path), output_value) in outputs {
        if candidate != &template.path || member != member_key || output_path.0.is_empty() {
            continue;
        }
        let path = SemanticOutputPath(base.0.iter().chain(&output_path.0).cloned().collect());
        if paths.insert(path.clone(), output_value.clone()).is_some() {
            return Err(CodeExpansionError::Unsupported(format!(
                "duplicate patch result path `{}`",
                path_text(&path),
            )));
        }
    }
    Ok(())
}

fn publish_invocation_collections(
    plan: &InvocationPlan,
    outputs: &TemplateOutputPlan,
) -> Result<InvocationCollectionPublication, CodeExpansionError> {
    let mut paths = BTreeMap::new();
    let mut root_members = BTreeMap::new();
    for rule in &plan.pinned.artifact.artifact().collections {
        let (collection_path, templates) = match rule {
            CollectionRule::Each {
                path, templates, ..
            }
            | CollectionRule::MapRecord {
                path, templates, ..
            } => (path, templates),
        };
        let mut members = BTreeMap::new();
        for (key, _) in plan.collections.get(collection_path).into_iter().flatten() {
            let mut member_outputs = BTreeMap::new();
            for template in templates {
                let template_node = plan
                    .pinned
                    .artifact
                    .artifact()
                    .templates
                    .iter()
                    .find(|candidate| candidate.path == *template)
                    .expect("collection rule references one authenticated template");
                for ((_, _, output), value) in outputs.iter().filter(|((path, member, _), _)| {
                    path == template && member == std::slice::from_ref(key)
                }) {
                    if template_node.result_output.as_ref() == Some(output) {
                        member_outputs.insert(path_text(output), value.clone());
                    }
                }
                if template_node.result_output.is_none() {
                    let value = template_public_result(
                        template_node,
                        std::slice::from_ref(key),
                        outputs,
                        plan,
                    )?;
                    member_outputs.insert(template_node.path.join("."), value);
                }
            }
            if member_outputs.is_empty() {
                return Err(CodeExpansionError::InvalidDeclaration {
                    declaration: plan.declaration.symbol.0.clone(),
                    message: format!(
                        "collection member `{key}` has no compiler-recorded result output"
                    ),
                });
            }
            let value = if member_outputs.len() == 1 {
                member_outputs.pop_first().expect("one output").1
            } else {
                SemanticValue::Collection(member_outputs)
            };
            members.insert(key.clone(), value.clone());
            paths.insert(
                SemanticOutputPath(
                    collection_path
                        .iter()
                        .cloned()
                        .map(ManagedPathSegment::Field)
                        .chain(std::iter::once(ManagedPathSegment::Member {
                            member: key.clone(),
                        }))
                        .collect(),
                ),
                value.clone(),
            );
            // A patch returning `mapRecord(...)` exposes its exact record keys
            // directly; returning `{ fillets: ... }` also exposes the named
            // collection path. Both are deterministic semantic aliases.
            paths.entry(fields_path(&[key])).or_insert(value);
        }
        let collection = SemanticValue::Collection(members.clone());
        paths.insert(
            SemanticOutputPath(
                collection_path
                    .iter()
                    .cloned()
                    .map(ManagedPathSegment::Field)
                    .collect(),
            ),
            collection.clone(),
        );
        root_members.extend(members);
    }
    Ok((paths, root_members))
}

fn insert_nested_root_member(
    members: &mut BTreeMap<String, SemanticValue>,
    path: &[String],
    value: SemanticValue,
    declaration: &SemanticSymbol,
) -> Result<(), CodeExpansionError> {
    let Some((head, tail)) = path.split_first() else {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: declaration.0.clone(),
            message: "patch result path cannot be empty".into(),
        });
    };
    if tail.is_empty() {
        if members.insert(head.clone(), value).is_some() {
            return Err(CodeExpansionError::InvalidDeclaration {
                declaration: declaration.0.clone(),
                message: format!("duplicate patch result path `{}`", path.join(".")),
            });
        }
        return Ok(());
    }
    let entry = members
        .entry(head.clone())
        .or_insert_with(|| SemanticValue::Collection(BTreeMap::new()));
    let SemanticValue::Collection(children) = entry else {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: declaration.0.clone(),
            message: format!("overlapping patch result path `{}`", path.join(".")),
        });
    };
    insert_nested_root_member(children, tail, value, declaration)
}

fn invocation_number(plan: &InvocationPlan, name: &str) -> Result<f64, CodeExpansionError> {
    let arguments = object(
        &plan
            .declaration
            .patch
            .as_ref()
            .expect("patch plan")
            .arguments,
        &plan.declaration.symbol.0,
    )?;
    let value = required(arguments, name, &plan.declaration.symbol.0)?;
    match value {
        ManagedValue::Number(value) => Ok(*value),
        ManagedValue::Unit(value) => Ok(value.value),
        _ => Err(CodeExpansionError::InvalidDeclaration {
            declaration: plan.declaration.symbol.0.clone(),
            message: format!("`{name}` must be a finite length"),
        }),
    }
}

fn invocation_unit(
    plan: &InvocationPlan,
    names: &[&str],
) -> Result<UnitLiteral, CodeExpansionError> {
    let arguments = object(
        &plan
            .declaration
            .patch
            .as_ref()
            .expect("patch plan")
            .arguments,
        &plan.declaration.symbol.0,
    )?;
    for name in names {
        if let Some(value) = arguments.get(*name) {
            return match value {
                ManagedValue::Unit(value) => Ok(value.clone()),
                ManagedValue::Number(value) => Ok(UnitLiteral {
                    unit: "model".into(),
                    value: *value,
                }),
                _ => Err(CodeExpansionError::InvalidDeclaration {
                    declaration: plan.declaration.symbol.0.clone(),
                    message: format!("`{name}` must be a finite length"),
                }),
            };
        }
    }
    Err(CodeExpansionError::UnresolvedReference {
        reference: format!("{}.{}", plan.declaration.symbol.0, names.join("|")),
    })
}

fn derived_mounting_centers(
    arguments: &BTreeMap<String, ManagedValue>,
    declaration: &str,
) -> Result<BTreeMap<String, [f64; 2]>, CodeExpansionError> {
    let width = scalar_value(required(arguments, "width", declaration)?, "width")?;
    let height = scalar_value(required(arguments, "height", declaration)?, "height")?;
    let inset = arguments
        .get("cornerRadius")
        .map(|value| scalar_value(value, "cornerRadius"))
        .transpose()?
        .unwrap_or(0.0)
        .max(0.0);
    let x = width / 2.0 - inset;
    let y = height / 2.0 - inset;
    if !x.is_finite() || !y.is_finite() || x <= 0.0 || y <= 0.0 {
        return Err(CodeExpansionError::InvalidDeclaration {
            declaration: declaration.to_owned(),
            message: "mounting centers require positive dimensions beyond the corner inset".into(),
        });
    }
    Ok(BTreeMap::from([
        ("nw".into(), [-x, y]),
        ("ne".into(), [x, y]),
        ("se".into(), [x, -y]),
        ("sw".into(), [-x, -y]),
    ]))
}

fn ensure_feature_kind(
    value: SemanticValue,
    expected: FeatureKind,
    reference: &str,
) -> Result<SemanticValue, CodeExpansionError> {
    if value.kind() == expected
        || (expected == FeatureKind::Point && matches!(value, SemanticValue::Corner(_)))
        || (expected == FeatureKind::Feature
            && matches!(value, SemanticValue::Corner(_) | SemanticValue::Port(_)))
    {
        Ok(value)
    } else {
        Err(CodeExpansionError::KindMismatch {
            reference: reference.to_owned(),
            expected,
            actual: value.kind(),
        })
    }
}

fn resolve_collection_path(
    value: &SemanticValue,
    path: &[ManagedPathSegment],
) -> Option<SemanticValue> {
    resolve_value_path(value, path)
}

fn resolve_value_path(value: &SemanticValue, path: &[ManagedPathSegment]) -> Option<SemanticValue> {
    if path.is_empty() {
        return Some(value.clone());
    }
    let members = match value {
        SemanticValue::Collection(members) | SemanticValue::Feature { members, .. } => members,
        SemanticValue::Declaration { .. }
        | SemanticValue::Port(_)
        | SemanticValue::Corner(_)
        | SemanticValue::HostOutput { .. }
        | SemanticValue::PointLiteral(_)
        | SemanticValue::ScalarLiteral(_) => return None,
    };
    let key = match &path[0] {
        ManagedPathSegment::Field(key) | ManagedPathSegment::Member { member: key } => key,
        ManagedPathSegment::Index(index) => &index.to_string(),
    };
    resolve_value_path(members.get(key)?, &path[1..])
}

fn semantic_alias(
    prefix: &str,
    project: &ProjectKey,
    symbol: &SemanticSymbol,
    suffix: &[&str],
) -> Result<IntentKey, CodeExpansionError> {
    let bytes = serde_json::to_vec(&(prefix, project, symbol, suffix))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    IntentKey::new(format!("code.{prefix}.{}", intent_content_digest(&bytes))).map_err(Into::into)
}

/// Returns the durable Intent symbol used by one direct named declaration.
///
/// Projectional hosts use this narrow identity seam when several declarations
/// are born in one native gesture. Intent allocation is deliberately
/// independent of patch-array order, so assigning human-facing generated
/// names in this same symbol order preserves the already accepted native
/// declaration identities without reproducing the alias hash outside this
/// crate.
///
/// # Errors
///
/// Returns an unsupported-namespace or key-encoding error.
pub fn direct_declaration_intent_symbol(
    project: &ProjectKey,
    namespace: &str,
    symbol: &SemanticSymbol,
) -> Result<IntentKey, CodeExpansionError> {
    if !matches!(
        namespace,
        "geometry" | "constraint" | "dimension" | "operation" | "aggregate" | "computed"
    ) {
        return Err(CodeExpansionError::Unsupported(format!(
            "unknown direct declaration namespace `{namespace}`"
        )));
    }
    semantic_alias(namespace, project, symbol, &[])
}

fn generated_alias(
    address: &GeneratedMemberAddress,
    identity: GeneratedMemberIdentity,
) -> Result<IntentKey, CodeExpansionError> {
    // Allocation is the never-reused identity coordinate. Generation alone
    // is insufficient: two unrelated active members normally both begin at
    // generation zero and must never collapse to the same graph alias.
    let bytes = serde_json::to_vec(&(address, identity))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    IntentKey::new(format!("code.generated.{}", intent_content_digest(&bytes))).map_err(Into::into)
}

fn generated_scoped_alias(
    address: &GeneratedMemberAddress,
    identity: GeneratedMemberIdentity,
    scope: &str,
    suffix: &[&str],
) -> Result<IntentKey, CodeExpansionError> {
    let bytes = serde_json::to_vec(&(address, identity, scope, suffix))
        .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
    IntentKey::new(format!("code.generated.{}", intent_content_digest(&bytes))).map_err(Into::into)
}

fn direct_generated_alias(
    address: &GeneratedMemberAddress,
    reconciliation: Option<&KeyedReconcileState>,
) -> Result<(IntentKey, Option<GeneratedMemberIdentity>), CodeExpansionError> {
    let Some(reconciliation) = reconciliation else {
        // Preliminary artifact planning needs typed semantic paths but does
        // not publish this alias. Keep it deterministic and distinct from all
        // allocated identities.
        let bytes = serde_json::to_vec(&("direct-preflight", address))
            .map_err(|error| CodeExpansionError::Encoding(error.to_string()))?;
        return Ok((
            IntentKey::new(format!("code.preflight.{}", intent_content_digest(&bytes)))?,
            None,
        ));
    };
    let identity = reconciliation
        .active()
        .get(address)
        .copied()
        .ok_or_else(|| {
            CodeExpansionError::ReconciliationMismatch(format!(
                "missing active `{}`",
                address.display_path()
            ))
        })?;
    Ok((generated_alias(address, identity)?, Some(identity)))
}

const fn feature_kind_for_port(kind: IntentPortKind) -> FeatureKind {
    match kind {
        IntentPortKind::Point | IntentPortKind::HandlePoint => FeatureKind::Point,
        IntentPortKind::Scalar
        | IntentPortKind::Parameter
        | IntentPortKind::ParameterBinding
        | IntentPortKind::ParameterOutput
        | IntentPortKind::ExternalBinding
        | IntentPortKind::SemanticCatalog
        | IntentPortKind::Source
        | IntentPortKind::Annotation => FeatureKind::Scalar,
        IntentPortKind::Contact => FeatureKind::Contact,
        IntentPortKind::Curve => FeatureKind::Curve,
        IntentPortKind::CurveSpan => FeatureKind::CurveSpan,
        IntentPortKind::Constraint => FeatureKind::Constraint,
        IntentPortKind::Dimension => FeatureKind::Dimension,
        IntentPortKind::Profile => FeatureKind::Profile,
        IntentPortKind::Chain => FeatureKind::Chain,
        IntentPortKind::Operation => FeatureKind::Operation,
        IntentPortKind::Feature => FeatureKind::Feature,
        IntentPortKind::FeatureCorner => FeatureKind::FeatureCorner,
        IntentPortKind::Collection => FeatureKind::Collection,
    }
}

fn semantic_generation(value: &SemanticValue) -> u32 {
    match value {
        SemanticValue::HostOutput { identity, .. } => identity.generation,
        _ => 0,
    }
}

fn managed_kind(value: &ManagedValue) -> FeatureKind {
    match value {
        ManagedValue::Array(_) | ManagedValue::Object(_) => FeatureKind::Collection,
        ManagedValue::Reference { .. } => FeatureKind::Feature,
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_) => FeatureKind::Scalar,
    }
}

fn port(alias: &IntentKey, selector: IntentPortSelector, kind: IntentPortKind) -> ExpandedPort {
    ExpandedPort {
        alias: alias.clone(),
        selector,
        kind,
    }
}

fn draft_semantic_outputs(
    draft: &IntentNodeDraft,
    alias: &IntentKey,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    draft
        .output_descriptors()
        .map_err(|error| CodeExpansionError::InvalidDeclaration {
            declaration: draft.symbol.as_str().to_owned(),
            message: format!("native result descriptor rejected the named declaration: {error}"),
        })?
        .into_iter()
        .map(|output| {
            let path = semantic_output_path(&output.path);
            let value = SemanticValue::Port(port(alias, output.selector, output.kind));
            Ok((path, value))
        })
        .collect()
}

fn select_planned_outputs(
    all: &BTreeMap<SemanticOutputPath, SemanticValue>,
    planned: &[PlannedTemplateOutput],
    family: &str,
) -> Result<BTreeMap<SemanticOutputPath, SemanticValue>, CodeExpansionError> {
    planned
        .iter()
        .map(|(path, expected, _, _)| {
            let value = all.get(path).cloned().ok_or_else(|| {
                CodeExpansionError::Unsupported(format!(
                    "family `{family}` did not materialize output `{}`",
                    path_text(path),
                ))
            })?;
            ensure_feature_kind(value, *expected, &path_text(path))
                .map(|value| (path.clone(), value))
        })
        .collect()
}

const fn node_selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

const fn child_selector(ordinal: u16, role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::InitialChild {
        ordinal,
        role,
        index: 0,
    }
}

fn field_key(value: &str) -> Result<IntentFieldKey, CodeExpansionError> {
    Ok(IntentFieldKey(IntentKey::new(value)?))
}

const fn length(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn enum_value(value: &str) -> Result<IntentLiteral, CodeExpansionError> {
    Ok(IntentLiteral::Enum(IntentKey::new(value)?))
}

fn unit_direction(start: [f64; 2], end: [f64; 2]) -> Option<[f64; 2]> {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let magnitude = direction[0].hypot(direction[1]);
    (magnitude.is_finite() && magnitude > f64::EPSILON)
        .then_some([direction[0] / magnitude, direction[1] / magnitude])
}

fn object<'a>(
    value: &'a ManagedValue,
    label: &str,
) -> Result<&'a BTreeMap<String, ManagedValue>, CodeExpansionError> {
    match value {
        ManagedValue::Object(value) => Ok(value),
        _ => Err(CodeExpansionError::InvalidDeclaration {
            declaration: label.to_owned(),
            message: "arguments must be an object".into(),
        }),
    }
}

fn array<'a>(
    value: &'a ManagedValue,
    label: &str,
) -> Result<&'a [ManagedValue], CodeExpansionError> {
    match value {
        ManagedValue::Array(value) => Ok(value),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be an array"
        ))),
    }
}

fn required<'a>(
    values: &'a BTreeMap<String, ManagedValue>,
    key: &str,
    declaration: &str,
) -> Result<&'a ManagedValue, CodeExpansionError> {
    values
        .get(key)
        .ok_or_else(|| CodeExpansionError::InvalidDeclaration {
            declaration: declaration.to_owned(),
            message: format!("missing `{key}`"),
        })
}

fn point(value: &ManagedValue, label: &str) -> Result<[f64; 2], CodeExpansionError> {
    let values = array(value, label)?;
    point_from_array(values, label)
}

fn point_from_array(values: &[ManagedValue], label: &str) -> Result<[f64; 2], CodeExpansionError> {
    if values.len() != 2 {
        return Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must contain exactly two coordinates"
        )));
    }
    Ok([
        scalar_value(&values[0], label)?,
        scalar_value(&values[1], label)?,
    ])
}

fn scalar_value(value: &ManagedValue, label: &str) -> Result<f64, CodeExpansionError> {
    match value {
        ManagedValue::Number(value) => Ok(*value),
        ManagedValue::Unit(value) => Ok(value.value),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be a finite number"
        ))),
    }
}

fn finite_number(value: &ManagedValue, label: &str) -> Result<f64, CodeExpansionError> {
    match value {
        ManagedValue::Number(value) if value.is_finite() => Ok(*value),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be a finite dimensionless number"
        ))),
    }
}

fn length_value(value: &ManagedValue, label: &str) -> Result<f64, CodeExpansionError> {
    match value {
        ManagedValue::Number(value) if value.is_finite() => Ok(*value),
        ManagedValue::Unit(UnitLiteral { unit, value }) if unit == "mm" && value.is_finite() => {
            Ok(*value)
        }
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be a finite model-unit number or millimetre literal"
        ))),
    }
}

/// Resolves only the scalar-binding indirection admitted by managed before
/// applying the same direct-family length policy as an inline literal. The
/// binding was already finite-validated and lowered into `SemanticValue`, so
/// this never evaluates source syntax or guesses through a declaration result.
fn resolved_direct_length_value(
    builder: &ExpansionBuilder,
    value: &ManagedValue,
    label: &str,
) -> Result<f64, CodeExpansionError> {
    let ManagedValue::Reference { path, .. } = value else {
        return length_value(value, label);
    };
    if !path.0.is_empty() {
        return length_value(value, label);
    }
    match builder.resolve_managed(value, &SemanticOutputPath::default(), label)? {
        SemanticValue::ScalarLiteral(UnitLiteral { unit, value })
            if matches!(unit.as_str(), "model" | "mm") && value.is_finite() =>
        {
            Ok(value)
        }
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must reference one finite model-unit or millimetre scalar binding"
        ))),
    }
}

fn integer_value(value: &ManagedValue, label: &str) -> Result<i32, CodeExpansionError> {
    let value = finite_number(value, label)?;
    if value.fract() != 0.0 || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be an integer within i32 range"
        )));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "integrality and the exact i32 range are checked immediately above"
    )]
    Ok(value as i32)
}

fn exact_object_keys(
    value: &BTreeMap<String, ManagedValue>,
    expected: &[&str],
    label: &str,
) -> Result<(), CodeExpansionError> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(CodeExpansionError::Unsupported(format!(
            "`{label}` fields must be exactly {}",
            expected.into_iter().collect::<Vec<_>>().join(", ")
        )))
    }
}

fn boolean(value: &ManagedValue, label: &str) -> Result<bool, CodeExpansionError> {
    match value {
        ManagedValue::Bool(value) => Ok(*value),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be boolean"
        ))),
    }
}

fn string<'a>(value: &'a ManagedValue, label: &str) -> Result<&'a str, CodeExpansionError> {
    match value {
        ManagedValue::String(value) => Ok(value),
        _ => Err(CodeExpansionError::Unsupported(format!(
            "`{label}` must be a string"
        ))),
    }
}

fn invalid_declaration<T>(
    declaration: &AuthoringDeclaration,
    message: String,
) -> Result<T, CodeExpansionError> {
    Err(CodeExpansionError::InvalidDeclaration {
        declaration: declaration.symbol.0.clone(),
        message,
    })
}

fn insert_path(
    paths: &mut BTreeMap<SemanticOutputPath, SemanticValue>,
    path: SemanticOutputPath,
    value: SemanticValue,
    declaration: &AuthoringDeclaration,
) -> Result<(), CodeExpansionError> {
    if paths.insert(path, value).is_some() {
        return invalid_declaration(declaration, "duplicate semantic output path".into());
    }
    Ok(())
}

fn fields_path(fields: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        fields
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).to_owned()))
            .collect(),
    )
}

fn member_path(prefix: &[&str], member: &str, suffix: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        prefix
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).to_owned()))
            .chain(std::iter::once(ManagedPathSegment::Member {
                member: member.to_owned(),
            }))
            .chain(
                suffix
                    .iter()
                    .map(|field| ManagedPathSegment::Field((*field).to_owned())),
            )
            .collect(),
    )
}

fn index_path(prefix: &[&str], index: usize, suffix: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        prefix
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).to_owned()))
            .chain(std::iter::once(ManagedPathSegment::Index(index)))
            .chain(
                suffix
                    .iter()
                    .map(|field| ManagedPathSegment::Field((*field).to_owned())),
            )
            .collect(),
    )
}

fn join_paths(first: &SemanticOutputPath, second: &SemanticOutputPath) -> SemanticOutputPath {
    SemanticOutputPath(first.0.iter().chain(&second.0).cloned().collect())
}

fn reference_text(declaration: &SemanticSymbol, path: &SemanticOutputPath) -> String {
    if path.0.is_empty() {
        return declaration.0.clone();
    }
    format!("{}.{}", declaration.0, path_text(path))
}

fn path_text(path: &SemanticOutputPath) -> String {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => field.clone(),
            ManagedPathSegment::Index(index) => index.to_string(),
            ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn generated_output_address(path: &SemanticOutputPath) -> Vec<String> {
    path.0
        .iter()
        .map(|segment| match segment {
            ManagedPathSegment::Field(field) => format!("field:{field}"),
            ManagedPathSegment::Index(index) => format!("index:{index}"),
            ManagedPathSegment::Member { member } => format!("member:{member}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(
        symbol: &str,
        builder_path: &[&str],
        arguments: ManagedValue,
    ) -> AuthoringDeclaration {
        AuthoringDeclaration {
            variable: symbol.into(),
            symbol: SemanticSymbol(symbol.into()),
            builder_path: builder_path.iter().map(|part| (*part).into()).collect(),
            arguments,
            patch: None,
            statement_span: crate::ManagedSpan::new(0, 1),
            symbol_span: crate::ManagedSpan::new(0, 1),
            arguments_span: crate::ManagedSpan::new(0, 1),
        }
    }

    fn rectangle_draft(
        alias: &IntentKey,
        outputs: Vec<geosolve_sketch_intent::IntentOperationOutput>,
    ) -> IntentNodeDraft {
        IntentNodeDraft::new(
            IntentNodeKind::Operation {
                operation: OperationKind::Rectangle,
            },
            alias.clone(),
        )
        .with_field(
            field_key("origin").unwrap(),
            IntentLiteral::Point([0.0, 0.0]),
        )
        .with_field(field_key("width").unwrap(), length(4.0))
        .with_field(field_key("height").unwrap(), length(3.0))
        .with_field(field_key("role").unwrap(), enum_value("profile").unwrap())
        .with_operation_outputs(outputs)
    }

    fn prepared_output(
        output: geosolve_sketch_intent::IntentOperationOutput,
        path: Vec<PreparedIntentOperationPathSegment>,
        role: PreparedIntentOperationOutputRole,
    ) -> geosolve_constraint_editor::PreparedIntentOperationOutput {
        geosolve_constraint_editor::PreparedIntentOperationOutput { output, path, role }
    }

    fn source_ref(
        alias: &IntentKey,
        selector: IntentPortSelector,
        kind: IntentPortKind,
    ) -> PreparedIntentOperationSourceRef {
        PreparedIntentOperationSourceRef {
            declaration: alias.clone(),
            selector,
            kind,
        }
    }

    fn builder_with_source_outputs() -> (ExpansionBuilder, IntentKey) {
        let mut builder = ExpansionBuilder::new(
            ProjectKey("operation-source-key-test".into()),
            ManagedSuppressionProjection::default(),
        );
        let owner = SemanticSymbol("west".into());
        let alias = IntentKey::new("sourceWest").unwrap();
        let outputs = BTreeMap::from([
            (
                fields_path(&["end"]),
                SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::End, 0),
                    IntentPortKind::Point,
                )),
            ),
            (
                fields_path(&["curve"]),
                SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Curve, 0),
                    IntentPortKind::Curve,
                )),
            ),
            (
                fields_path(&["span"]),
                SemanticValue::Port(port(
                    &alias,
                    node_selector(IntentPortRole::Span, 0),
                    IntentPortKind::CurveSpan,
                )),
            ),
        ]);
        builder
            .declaration_provenance
            .insert(alias.clone(), owner.clone());
        builder.declarations.insert(
            owner,
            SemanticDeclaration {
                root: SemanticValue::Declaration {
                    alias: alias.clone(),
                    kind: FeatureKind::Feature,
                },
                paths: outputs,
            },
        );
        (builder, alias)
    }

    fn single_curve_polyline_declaration() -> AuthoringDeclaration {
        let vertex = |key: &str, position: [f64; 2]| {
            ManagedValue::Object(BTreeMap::from([
                ("key".into(), ManagedValue::String(key.into())),
                (
                    "position".into(),
                    ManagedValue::Array(position.into_iter().map(ManagedValue::Number).collect()),
                ),
            ]))
        };
        AuthoringDeclaration {
            variable: "geometry1".into(),
            symbol: SemanticSymbol("geometry1".into()),
            builder_path: vec!["geometry".into(), "polyline".into()],
            arguments: ManagedValue::Object(BTreeMap::from([
                ("closed".into(), ManagedValue::Bool(false)),
                (
                    "representation".into(),
                    ManagedValue::String("singleCurve".into()),
                ),
                ("role".into(), ManagedValue::String("profile".into())),
                (
                    "vertices".into(),
                    ManagedValue::Array(vec![
                        vertex("v0", [0.0, 0.0]),
                        vertex("v1", [20.0, 0.0]),
                        vertex("v2", [20.0, 10.0]),
                    ]),
                ),
            ])),
            patch: None,
            statement_span: crate::ManagedSpan::new(0, 1),
            symbol_span: crate::ManagedSpan::new(0, 1),
            arguments_span: crate::ManagedSpan::new(0, 1),
        }
    }

    #[test]
    fn direct_circle_accepts_and_lowers_canvas_presentation_fields() {
        let declaration = declaration(
            "geometry1",
            &["geometry", "centerRadiusCircle"],
            ManagedValue::Object(BTreeMap::from([
                (
                    "center".into(),
                    ManagedValue::Array(vec![
                        ManagedValue::Number(-0.6),
                        ManagedValue::Number(2.6),
                    ]),
                ),
                (
                    "label".into(),
                    ManagedValue::String("center-radius-circle-0000000000000001".into()),
                ),
                (
                    "radius".into(),
                    ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: 1.8,
                    }),
                ),
                ("role".into(), ManagedValue::String("construction".into())),
            ])),
        );
        let mut builder = ExpansionBuilder::new(
            ProjectKey("direct-circle-presentation-test".into()),
            ManagedSuppressionProjection::default(),
        );

        lower_direct_circle(
            &mut builder,
            &declaration,
            None,
            &CodeInteractionOverlay::empty(),
        )
        .expect("canvas-authored direct circle presentation fields lower");

        let [IntentPatchOperation::CreateNode { draft, .. }] = builder.operations.as_slice() else {
            panic!("direct circle must lower to one native node")
        };
        assert_eq!(draft.name.as_str(), "center-radius-circle-0000000000000001");
        assert_eq!(
            draft.fields.get(&field_key("role").unwrap()),
            Some(&enum_value("construction").unwrap())
        );
    }

    #[test]
    fn compact_polyline_lowers_to_one_native_recipe_with_keyed_child_ports() {
        let declaration = single_curve_polyline_declaration();
        let mut builder = ExpansionBuilder::new(
            ProjectKey("compact-polyline-test".into()),
            ManagedSuppressionProjection::default(),
        );
        lower_direct_polyline(
            &mut builder,
            &declaration,
            None,
            &CodeInteractionOverlay::empty(),
        )
        .expect("compact Polyline lowering");

        let [IntentPatchOperation::CreateNode { alias, draft, .. }] = builder.operations.as_slice()
        else {
            panic!("compact source must lower to exactly one native Polyline node")
        };
        assert!(matches!(
            draft.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Polyline
            }
        ));
        assert_eq!(draft.dynamic_children, 3);
        assert_eq!(
            draft.fields.get(&field_key("closed").unwrap()),
            Some(&IntentLiteral::Boolean(false))
        );
        assert_eq!(
            draft.fields.get(&field_key("role").unwrap()),
            Some(&enum_value("profile").unwrap())
        );
        assert_eq!(builder.point_seeds.len(), 3);
        let semantic = builder
            .declarations
            .get(&declaration.symbol)
            .expect("Polyline semantic declaration");
        assert!(matches!(
            &semantic.root,
            SemanticValue::Declaration { alias: root, .. } if root == alias
        ));
        for (key, ordinal) in [("v0", 0), ("v1", 1)] {
            let Some(SemanticValue::Port(span)) = semantic
                .paths
                .get(&fields_path(&["segments", "byKey", key]))
            else {
                panic!("compact Polyline must expose keyed segment `{key}`")
            };
            assert_eq!(span.alias, *alias);
            assert_eq!(span.selector, child_selector(ordinal, IntentPortRole::Span));
            assert_eq!(span.kind, IntentPortKind::CurveSpan);
        }
    }

    #[test]
    fn pure_expansion_refuses_to_guess_an_operation_result_shape() {
        let declaration = declaration(
            "rectangle",
            &["operation", "rectangle"],
            ManagedValue::Object(BTreeMap::from([
                (
                    "origin".into(),
                    ManagedValue::Array(vec![ManagedValue::Number(0.0), ManagedValue::Number(0.0)]),
                ),
                (
                    "width".into(),
                    ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: 4.0,
                    }),
                ),
                (
                    "height".into(),
                    ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: 3.0,
                    }),
                ),
                ("role".into(), ManagedValue::String("profile".into())),
            ])),
        );
        let descriptor = resolve_code_authoring_declaration("operation", "rectangle", 0).unwrap();
        let mut builder = ExpansionBuilder::new(
            ProjectKey("source-free-operation-test".into()),
            ManagedSuppressionProjection::default(),
        );

        let error = lower_named_operation(
            &mut builder,
            &declaration,
            &descriptor,
            &mut UnavailableOperationPlanner,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CodeExpansionError::OperationPlanning { message, .. }
                if message.contains("authenticated native document")
        ));
        assert!(builder.operations.is_empty());
        assert!(builder.declarations.is_empty());
    }

    #[test]
    fn source_free_operation_outputs_need_no_fabricated_source_key() {
        let alias = IntentKey::new("rectangleOperation").unwrap();
        let output =
            geosolve_sketch_intent::IntentOperationOutput::native(IntentOperationOutputKind::Point);
        let draft = rectangle_draft(&alias, vec![output]);
        let plan = PreparedIntentOperationPlan {
            operation: OperationKind::Rectangle,
            outputs: vec![prepared_output(
                output,
                vec![
                    PreparedIntentOperationPathSegment::Field("corners".into()),
                    PreparedIntentOperationPathSegment::Field("bottomLeft".into()),
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )],
        };
        let declaration = declaration(
            "rectangle",
            &["operation", "rectangle"],
            ManagedValue::Object(BTreeMap::new()),
        );
        let builder = ExpansionBuilder::new(
            ProjectKey("source-free-output-test".into()),
            ManagedSuppressionProjection::default(),
        );

        let outputs = operation_semantic_outputs(
            &builder,
            &declaration,
            OperationKind::Rectangle,
            &alias,
            &draft,
            &plan,
        )
        .unwrap();

        assert!(outputs.contains_key(&fields_path(&["corners", "bottomLeft"])));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table-style regression compares all source-keyed operation path variants"
    )]
    fn operation_source_members_use_readable_semantic_references_and_public_indices() {
        let (builder, source_alias) = builder_with_source_outputs();
        let declaration = declaration(
            "derived",
            &["operation", "mirror"],
            ManagedValue::Object(BTreeMap::new()),
        );
        let point = source_ref(
            &source_alias,
            node_selector(IntentPortRole::End, 0),
            IntentPortKind::Point,
        );
        let curve = source_ref(
            &source_alias,
            node_selector(IntentPortRole::Curve, 0),
            IntentPortKind::Curve,
        );
        let span = source_ref(
            &source_alias,
            node_selector(IntentPortRole::Span, 0),
            IntentPortKind::CurveSpan,
        );

        assert_eq!(
            clean_operation_output_path(
                &builder,
                &declaration,
                OperationKind::Mirror,
                &[
                    PreparedIntentOperationPathSegment::Field("controls".into()),
                    PreparedIntentOperationPathSegment::SourceControl { source: point },
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )
            .unwrap(),
            member_path(&["controls"], "west.end", &[]),
        );
        assert_eq!(
            clean_operation_output_path(
                &builder,
                &declaration,
                OperationKind::Mirror,
                &[
                    PreparedIntentOperationPathSegment::Field("curves".into()),
                    PreparedIntentOperationPathSegment::SourceCurve {
                        source: curve.clone(),
                    },
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )
            .unwrap(),
            fields_path(&["curve"]),
        );
        assert_eq!(
            clean_operation_output_path(
                &builder,
                &declaration,
                OperationKind::LinearPattern,
                &[
                    PreparedIntentOperationPathSegment::Field("instances".into()),
                    PreparedIntentOperationPathSegment::Index(1),
                    PreparedIntentOperationPathSegment::Field("sources".into()),
                    PreparedIntentOperationPathSegment::SourceCurve { source: curve },
                    PreparedIntentOperationPathSegment::Field("curve".into()),
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )
            .unwrap(),
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("instances".into()),
                ManagedPathSegment::Index(0),
                ManagedPathSegment::Field("sources".into()),
                ManagedPathSegment::Member {
                    member: "west.curve".into(),
                },
                ManagedPathSegment::Field("curve".into()),
            ]),
        );
        assert_eq!(
            clean_operation_output_path(
                &builder,
                &declaration,
                OperationKind::ProfileOffset,
                &[
                    PreparedIntentOperationPathSegment::Field("operand".into()),
                    PreparedIntentOperationPathSegment::Field("chain".into()),
                    PreparedIntentOperationPathSegment::Field("edges".into()),
                    PreparedIntentOperationPathSegment::SourceSpan { source: span },
                    PreparedIntentOperationPathSegment::Field("curve".into()),
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )
            .unwrap(),
            SemanticOutputPath(vec![
                ManagedPathSegment::Field("operand".into()),
                ManagedPathSegment::Field("chain".into()),
                ManagedPathSegment::Field("edges".into()),
                ManagedPathSegment::Member {
                    member: "west.span".into(),
                },
                ManagedPathSegment::Field("curve".into()),
            ]),
        );
    }

    #[test]
    fn operation_outputs_reject_role_kind_tampering_and_surplus_native_spans() {
        let alias = IntentKey::new("tamperedRectangle").unwrap();
        let declaration = declaration(
            "rectangle",
            &["operation", "rectangle"],
            ManagedValue::Object(BTreeMap::new()),
        );
        let builder = ExpansionBuilder::new(
            ProjectKey("operation-tamper-test".into()),
            ManagedSuppressionProjection::default(),
        );
        let curve_one_span = geosolve_sketch_intent::IntentOperationOutput::curve(1);
        let wrong_role_plan = PreparedIntentOperationPlan {
            operation: OperationKind::Rectangle,
            outputs: vec![prepared_output(
                curve_one_span,
                vec![PreparedIntentOperationPathSegment::Field("edge".into())],
                PreparedIntentOperationOutputRole::Contact,
            )],
        };
        let draft = rectangle_draft(&alias, vec![curve_one_span]);
        assert!(matches!(
            operation_semantic_outputs(
                &builder,
                &declaration,
                OperationKind::Rectangle,
                &alias,
                &draft,
                &wrong_role_plan,
            ),
            Err(CodeExpansionError::InvalidDeclaration { message, .. })
                if message.contains("semantic role incompatible")
        ));

        let two_span_draft = rectangle_draft(
            &alias,
            vec![geosolve_sketch_intent::IntentOperationOutput::curve(2)],
        );
        let one_span_plan = PreparedIntentOperationPlan {
            operation: OperationKind::Rectangle,
            outputs: vec![prepared_output(
                curve_one_span,
                vec![
                    PreparedIntentOperationPathSegment::Field("edges".into()),
                    PreparedIntentOperationPathSegment::Field("bottom".into()),
                ],
                PreparedIntentOperationOutputRole::Geometry,
            )],
        };
        assert!(matches!(
            operation_semantic_outputs(
                &builder,
                &declaration,
                OperationKind::Rectangle,
                &alias,
                &two_span_draft,
                &one_span_plan,
            ),
            Err(CodeExpansionError::InvalidDeclaration { message, .. })
                if message.contains("unauthenticated span")
        ));
    }
}
